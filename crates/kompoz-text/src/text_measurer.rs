//! Laying text out outside a Text (Compose's TextMeasurer and its TextLayoutCache): measure text to draw it on a
//! canvas, the last few layouts kept.

use crate::annotated_string::{AnnotatedString, Range as AnnotatedRange};
use crate::font::FontFamilyResolver;
use crate::multi_paragraph::{MultiParagraph, MultiParagraphIntrinsics};
use crate::paragraph::DefaultMaxLines;
use crate::placeholder::Placeholder;
use crate::style::TextOverflow;
use crate::text_layout_result::{TextLayoutInput, TextLayoutResult};
use crate::text_style::TextStyle;
use kompoz_graphics::{Constraints, Density, IntSize, LayoutDirection};
use kompoz_runtime::named;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// How many layouts a measurer keeps by default (Compose's DefaultCacheSize).
pub const DefaultCacheSize: usize = 8;

/// The last layouts made (Compose's TextLayoutCache): looked up by what lays text out (a style differing only in
/// what paints still finds its layout), the oldest dropped past its capacity, one whose fonts went stale (an async
/// font loaded) not found.
pub struct TextLayoutCache {
    capacity: usize,
    entries: Vec<TextLayoutResult>,
}

impl TextLayoutCache {
    pub fn new(capacity: usize) -> TextLayoutCache {
        TextLayoutCache { capacity: capacity.max(1), entries: Vec::new() }
    }

    /// The layout kept for `key`, made the most recent.
    pub fn get(&mut self, key: &TextLayoutInput) -> Option<TextLayoutResult> {
        let i = self.entries.iter().position(|r| r.layout_input.lays_out_like(key))?;
        let found = self.entries.remove(i);
        if found.multi_paragraph.intrinsics().has_stale_resolved_fonts() {
            return None;
        }
        self.entries.push(found.clone());
        Some(found)
    }

    /// Keeps `value` (for its input).
    pub fn put(&mut self, value: TextLayoutResult) {
        self.entries.retain(|r| !r.layout_input.lays_out_like(&value.layout_input));
        self.entries.push(value);
        if self.entries.len() > self.capacity {
            self.entries.remove(0);
        }
    }
}

/// Lays text out (Compose's TextMeasurer): with its defaults — the resolver of fonts, the density, the layout
/// direction — where a call gives none, keeping its last `cache_size` layouts (none: 0).
pub struct TextMeasurer {
    default_font_family_resolver: Rc<dyn FontFamilyResolver>,
    default_density: Density,
    default_layout_direction: LayoutDirection,
    cache: Option<RefCell<TextLayoutCache>>,
}

impl TextMeasurer {
    pub fn new(
        default_font_family_resolver: Rc<dyn FontFamilyResolver>,
        default_density: Density,
        default_layout_direction: LayoutDirection,
        cache_size: usize,
    ) -> TextMeasurer {
        TextMeasurer {
            default_font_family_resolver,
            default_density,
            default_layout_direction,
            cache: (cache_size > 0).then(|| RefCell::new(TextLayoutCache::new(cache_size))),
        }
    }

    /// Its resolver of fonts.
    pub fn default_font_family_resolver(&self) -> &Rc<dyn FontFamilyResolver> {
        &self.default_font_family_resolver
    }
    /// Its density.
    pub fn default_density(&self) -> Density {
        self.default_density
    }
    /// Its layout direction.
    pub fn default_layout_direction(&self) -> LayoutDirection {
        self.default_layout_direction
    }

    /// Lays `text` out with every parameter given (Compose's measure); see [`measure`](TextMeasurer::measure).
    #[allow(clippy::too_many_arguments)]
    pub fn measure_with(
        &self,
        text: impl Into<AnnotatedString>,
        style: TextStyle,
        overflow: TextOverflow,
        soft_wrap: bool,
        max_lines: usize,
        placeholders: Vec<AnnotatedRange<Placeholder>>,
        constraints: Constraints,
        layout_direction: LayoutDirection,
        density: Density,
        font_family_resolver: Rc<dyn FontFamilyResolver>,
        skip_cache: bool,
    ) -> TextLayoutResult {
        let input = Rc::new(TextLayoutInput {
            text: text.into(),
            style,
            placeholders,
            max_lines,
            soft_wrap,
            overflow,
            density,
            layout_direction,
            font_family_resolver,
            constraints,
        });
        let cached = match (&self.cache, skip_cache) {
            (Some(cache), false) => cache.borrow_mut().get(&input),
            _ => None,
        };
        if let Some(r) = cached {
            let mp = &r.multi_paragraph;
            let size = constraints.constrain_size(IntSize::new(ceil(mp.width()), ceil(mp.height())));
            return r.copy(input, size);
        }
        let result = layout(input);
        if let Some(cache) = &self.cache {
            cache.borrow_mut().put(result.clone());
        }
        result
    }
}

#[named]
impl TextMeasurer {
    /// Lays `text` out (Compose's TextMeasurer.measure): in `style` (its defaults resolved in the layout's
    /// direction), at most `max_lines` lines, wrapping where `soft_wrap` says, cut as `overflow` says, within
    /// `constraints` — as wide as its max intrinsic width within them (all of their width where they fix it),
    /// unbounded when it neither wraps nor ellipsizes, one line when an ellipsis cuts text that doesn't wrap — its
    /// size whole pixels within them; the measurer's density, direction and resolver unless given. A layout kept
    /// for the same input (`skip_cache` not set) is given again, at the size these constraints make of it.
    #[allow(clippy::too_many_arguments)]
    pub fn measure(
        &self,
        text: impl Into<AnnotatedString>,
        #[default(TextStyle::Default)] style: TextStyle,
        #[default(TextOverflow::Clip)] overflow: TextOverflow,
        #[default(true)] soft_wrap: bool,
        #[default(DefaultMaxLines)] max_lines: usize,
        #[default(Vec::new())] placeholders: Vec<AnnotatedRange<Placeholder>>,
        #[default(Constraints::default())] constraints: Constraints,
        #[default(self.default_layout_direction)] layout_direction: LayoutDirection,
        #[default(self.default_density)] density: Density,
        #[default(self.default_font_family_resolver.clone())] font_family_resolver: Rc<dyn FontFamilyResolver>,
        #[default(false)] skip_cache: bool,
    ) -> TextLayoutResult {
        self.measure_with(
            text,
            style,
            overflow,
            soft_wrap,
            max_lines,
            placeholders,
            constraints,
            layout_direction,
            density,
            font_family_resolver,
            skip_cache,
        )
    }
}

impl fmt::Debug for TextMeasurer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TextMeasurer")
            .field("default_density", &self.default_density)
            .field("default_layout_direction", &self.default_layout_direction)
            .finish_non_exhaustive()
    }
}

/// `v` rounded up to a whole pixel (saturating).
fn ceil(v: f32) -> i32 {
    v.ceil() as i32
}

/// Whether `overflow` ellipsizes.
fn is_ellipsis(overflow: TextOverflow) -> bool {
    matches!(overflow, TextOverflow::Ellipsis | TextOverflow::StartEllipsis | TextOverflow::MiddleEllipsis)
}

/// Lays text out as Compose's TextMeasurer.layout does.
fn layout(input: Rc<TextLayoutInput>) -> TextLayoutResult {
    let c = input.constraints;
    let style = input.style.resolve_defaults(input.layout_direction);
    let intrinsics = Rc::new(MultiParagraphIntrinsics::new(
        &input.text,
        &style,
        &input.placeholders,
        input.density,
        &input.font_family_resolver,
        input.soft_wrap,
    ));
    let width_matters = input.soft_wrap || is_ellipsis(input.overflow);
    let max_width = if width_matters && c.has_bounded_width() { c.max_width } else { Constraints::Infinity };
    // Text that doesn't wrap but is ellipsized: each line would be; native layout ellipsizes one, so one line.
    let max_lines = if !input.soft_wrap && is_ellipsis(input.overflow) { 1 } else { input.max_lines };
    let width = if c.min_width == max_width {
        max_width
    } else {
        ceil(intrinsics.max_intrinsic_width()).clamp(c.min_width, max_width)
    };
    let multi_paragraph = MultiParagraph::new(
        intrinsics,
        Constraints::fit_prioritizing_width(0, width, 0, c.max_height),
        max_lines,
        input.overflow,
    );
    let size = c.constrain_size(IntSize::new(ceil(multi_paragraph.width()), ceil(multi_paragraph.height())));
    TextLayoutResult::new(input, Rc::new(multi_paragraph), size)
}

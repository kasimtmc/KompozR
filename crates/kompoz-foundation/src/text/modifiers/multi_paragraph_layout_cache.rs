//! An annotated string's layout, kept (Compose's MultiParagraphLayoutCache): what [`TextAnnotatedStringNode`] lays
//! out with, auto sizing included.
//!
//! [`TextAnnotatedStringNode`]: super::text_annotated_string::TextAnnotatedStringNode

use super::layout_utils::{ceil_to_int_px, final_constraints, final_max_lines};
use super::min_lines_constrainer::MinLinesConstrainer;
use crate::text::auto_size::{TextAutoSize, TextAutoSizeLayoutScope};
use kompoz_graphics::{Constraints, Density, IntSize, LayoutDirection, TextUnit};
use kompoz_text::span_style::DefaultFontSize;
use kompoz_text::{
    AnnotatedString, FontFamilyResolver, MultiParagraph, MultiParagraphIntrinsics, Placeholder, Range, TextLayoutInput,
    TextLayoutResult, TextOverflow, TextStyle,
};
use std::rc::Rc;

/// Lays an annotated string out as a [`MultiParagraph`], keeping what it can (Compose's
/// MultiParagraphLayoutCache): its intrinsics while its text, style and direction stay; its layout while new
/// constraints can't reflow it; its height at the last width asked for. With an auto size, the size it picks is
/// laid out in.
pub(crate) struct MultiParagraphLayoutCache {
    text: AnnotatedString,
    style: TextStyle,
    font_family_resolver: Rc<dyn FontFamilyResolver>,
    overflow: TextOverflow,
    soft_wrap: bool,
    max_lines: usize,
    min_lines: usize,
    placeholders: Option<Vec<Range<Placeholder>>>,
    auto_size: Option<Rc<dyn TextAutoSize>>,
    min_lines_constrainer: Option<MinLinesConstrainer>,
    /// The density it lays out in (none until a measure gives one).
    density: Option<Density>,
    paragraph_intrinsics: Option<Rc<MultiParagraphIntrinsics>>,
    intrinsics_layout_direction: Option<LayoutDirection>,
    /// The last layout.
    layout_cache: Option<TextLayoutResult>,
    cached_intrinsic_height_input_width: i32,
    cached_intrinsic_height: i32,
    /// The last layout an auto size made (Compose's fontSizeSearchScope.lastLayoutResult).
    auto_size_last_layout: Option<TextLayoutResult>,
}

impl MultiParagraphLayoutCache {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        text: AnnotatedString,
        style: TextStyle,
        font_family_resolver: Rc<dyn FontFamilyResolver>,
        overflow: TextOverflow,
        soft_wrap: bool,
        max_lines: usize,
        min_lines: usize,
        placeholders: Option<Vec<Range<Placeholder>>>,
        auto_size: Option<Rc<dyn TextAutoSize>>,
    ) -> MultiParagraphLayoutCache {
        MultiParagraphLayoutCache {
            text,
            style,
            font_family_resolver,
            overflow,
            soft_wrap,
            max_lines,
            min_lines,
            placeholders,
            auto_size,
            min_lines_constrainer: None,
            density: None,
            paragraph_intrinsics: None,
            intrinsics_layout_direction: None,
            layout_cache: None,
            cached_intrinsic_height_input_width: -1,
            cached_intrinsic_height: -1,
            auto_size_last_layout: None,
        }
    }

    /// The density it lays out in.
    pub(crate) fn density(&self) -> Option<Density> {
        self.density
    }

    /// Lays out in `density` from now on: another one than before drops what it kept (Compose's density setter).
    pub(crate) fn set_density(&mut self, density: Option<Density>) {
        let Some(old) = self.density else {
            self.density = density;
            return;
        };
        if density != Some(old) {
            self.density = density;
            self.mark_dirty();
        }
    }

    /// The style it lays out in (an auto size's font size in it once laid out).
    pub(crate) fn style(&self) -> &TextStyle {
        &self.style
    }

    /// Takes `style`, dropping what depends on it where it lays text out otherwise (Compose's style setter).
    fn set_style(&mut self, style: TextStyle) {
        let same = style.has_same_layout_affecting_attributes(&self.style);
        self.style = style;
        if !same {
            self.mark_style_affected_dirty();
        }
    }

    /// The last layout; panics when there is none (Compose's textLayoutResult, read when drawing).
    #[track_caller]
    pub(crate) fn text_layout_result(&self) -> &TextLayoutResult {
        self.layout_cache.as_ref().expect(
            "kompoz: Internal Error: MultiParagraphLayoutCache could not provide TextLayoutResult during the draw \
             phase.",
        )
    }

    /// The last layout, if any (Compose's layoutOrNull).
    pub(crate) fn layout_or_none(&self) -> Option<&TextLayoutResult> {
        self.layout_cache.as_ref()
    }

    /// Lays the text out in `constraints` (Compose's layoutWithConstraints), its min lines made a min height, at
    /// its auto size's size if it has one; whether the layout changed. Constraints that can't reflow it keep the
    /// layout (its input made again where they differ).
    pub(crate) fn layout_with_constraints(
        &mut self,
        constraints: Constraints,
        layout_direction: LayoutDirection,
    ) -> bool {
        let final_constraints = if self.min_lines > 1 {
            self.use_min_lines_constrainer(constraints, layout_direction)
        } else {
            constraints
        };
        if !self.new_layout_will_be_different(final_constraints, layout_direction) {
            let kept = self.layout_cache.clone().expect("kompoz: a layout");
            if final_constraints == kept.layout_input.constraints {
                return false;
            }
            // The same layout, its input (the constraints) made again.
            self.layout_cache =
                Some(self.text_layout_result_of(layout_direction, final_constraints, kept.multi_paragraph.clone()));
            return true;
        }
        if let Some(auto_size) = self.auto_size.clone() {
            self.intrinsics_layout_direction = Some(layout_direction);
            let font_size_before_layout = self.style.font_size;
            let density = self.density.expect("kompoz: a density to lay text out in");
            let text = self.text.clone();
            let mut optimal = {
                let mut scope = TextAutoSizeLayoutScope { cache: self, density };
                auto_size.get_font_size(&mut scope, constraints, &text)
            };
            if optimal.is_em() {
                optimal = times_em(font_size_before_layout, optimal);
            }
            // A layout the search made in that size is the one.
            if let Some(last) = &self.auto_size_last_layout
                && optimal == last.layout_input.style.font_size
                && last.layout_input.overflow == self.overflow
            {
                self.layout_cache = Some(last.clone());
                return true;
            }
            let style = TextStyle { font_size: optimal, ..self.style.clone() };
            self.set_style(style);
        }
        let multi_paragraph = Rc::new(self.layout_text(final_constraints, layout_direction));
        self.layout_cache = Some(self.text_layout_result_of(layout_direction, final_constraints, multi_paragraph));
        true
    }

    /// The text laid out at `font_size` (an em size of the style's) for an auto size to judge (Compose's
    /// TextAutoSizeLayoutScopeImpl.performLayout): its style as it was afterwards.
    pub(crate) fn perform_auto_size_layout(
        &mut self,
        constraints: Constraints,
        font_size: TextUnit,
    ) -> TextLayoutResult {
        let style_before_layout = self.style.clone();
        let scaled = if font_size.is_em() { times_em(self.style.font_size, font_size) } else { font_size };
        if scaled != self.style.font_size {
            self.set_style(TextStyle { font_size: scaled, ..self.style.clone() });
        }
        let direction = self.intrinsics_layout_direction.expect("kompoz: a layout direction to lay text out in");
        let layout_constraints =
            if self.min_lines > 1 { self.use_min_lines_constrainer(constraints, direction) } else { constraints };
        let multi_paragraph = Rc::new(self.layout_text(layout_constraints, direction));
        let result = self.text_layout_result_of(direction, layout_constraints, multi_paragraph);
        self.auto_size_last_layout = Some(result.clone());
        self.set_style(style_before_layout);
        result
    }

    fn use_min_lines_constrainer(
        &mut self,
        constraints: Constraints,
        layout_direction: LayoutDirection,
    ) -> Constraints {
        let density = self.density.expect("kompoz: a density to lay text out in");
        let constrainer = MinLinesConstrainer::from(
            self.min_lines_constrainer.as_ref(),
            layout_direction,
            &self.style,
            density,
            &self.font_family_resolver,
        );
        let coerced = constrainer.coerce_min_lines(constraints, self.min_lines);
        self.min_lines_constrainer = Some(constrainer);
        coerced
    }

    /// A [`TextLayoutResult`] of `multi_paragraph` (Compose's textLayoutResult): its size its unwrapped width (at
    /// most its width) and its height, whole pixels within the constraints.
    fn text_layout_result_of(
        &self,
        layout_direction: LayoutDirection,
        final_constraints: Constraints,
        multi_paragraph: Rc<MultiParagraph>,
    ) -> TextLayoutResult {
        let layout_width = multi_paragraph.intrinsics().max_intrinsic_width().min(multi_paragraph.width());
        let size = final_constraints
            .constrain_size(IntSize::new(ceil_to_int_px(layout_width), ceil_to_int_px(multi_paragraph.height())));
        let input = TextLayoutInput {
            text: self.text.clone(),
            style: self.style.clone(),
            placeholders: self.placeholders.clone().unwrap_or_default(),
            max_lines: self.max_lines,
            soft_wrap: self.soft_wrap,
            overflow: self.overflow,
            density: self.density.expect("kompoz: a density to lay text out in"),
            layout_direction,
            font_family_resolver: self.font_family_resolver.clone(),
            constraints: final_constraints,
        };
        TextLayoutResult::new(Rc::new(input), multi_paragraph, size)
    }

    /// How tall the text is laid out `width` px wide in `layout_direction` (Compose's intrinsicHeight): its min
    /// lines' height at least; the last width's kept.
    pub(crate) fn intrinsic_height(&mut self, width: i32, layout_direction: LayoutDirection) -> i32 {
        if width == self.cached_intrinsic_height_input_width && self.cached_intrinsic_height_input_width != -1 {
            return self.cached_intrinsic_height;
        }
        let constraints = Constraints::new(0, width, 0, Constraints::Infinity);
        let final_constraints = if self.min_lines > 1 {
            self.use_min_lines_constrainer(constraints, layout_direction)
        } else {
            constraints
        };
        let height = ceil_to_int_px(self.layout_text(final_constraints, layout_direction).height())
            .max(final_constraints.min_height);
        self.cached_intrinsic_height_input_width = width;
        self.cached_intrinsic_height = height;
        height
    }

    /// Takes new arguments, dropping what it kept (Compose's update).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn update(
        &mut self,
        text: AnnotatedString,
        style: TextStyle,
        font_family_resolver: Rc<dyn FontFamilyResolver>,
        overflow: TextOverflow,
        soft_wrap: bool,
        max_lines: usize,
        min_lines: usize,
        placeholders: Option<Vec<Range<Placeholder>>>,
        auto_size: Option<Rc<dyn TextAutoSize>>,
    ) {
        self.text = text;
        self.set_style(style);
        self.font_family_resolver = font_family_resolver;
        self.overflow = overflow;
        self.soft_wrap = soft_wrap;
        self.max_lines = max_lines;
        self.min_lines = min_lines;
        self.placeholders = placeholders;
        self.auto_size = auto_size;
        self.mark_dirty();
    }

    /// The text measured in `layout_direction`: kept while the direction stays and its faces are fresh
    /// (Compose's setLayoutDirection).
    fn set_layout_direction(&mut self, layout_direction: LayoutDirection) -> Rc<MultiParagraphIntrinsics> {
        if let Some(i) = &self.paragraph_intrinsics
            && self.intrinsics_layout_direction == Some(layout_direction)
            && !i.has_stale_resolved_fonts()
        {
            return i.clone();
        }
        self.intrinsics_layout_direction = Some(layout_direction);
        let intrinsics = Rc::new(MultiParagraphIntrinsics::new(
            &self.text,
            &self.style.resolve_defaults(layout_direction),
            self.placeholders.as_deref().unwrap_or_default(),
            self.density.expect("kompoz: a density to lay text out in"),
            &self.font_family_resolver,
            self.soft_wrap,
        ));
        self.paragraph_intrinsics = Some(intrinsics.clone());
        intrinsics
    }

    /// The text laid out in `constraints` (Compose's layoutText): as close to its unwrapped width as they let it.
    fn layout_text(&mut self, constraints: Constraints, layout_direction: LayoutDirection) -> MultiParagraph {
        let intrinsics = self.set_layout_direction(layout_direction);
        let max_intrinsic_width = intrinsics.max_intrinsic_width();
        MultiParagraph::new(
            intrinsics,
            final_constraints(constraints, self.soft_wrap, self.overflow, max_intrinsic_width),
            final_max_lines(self.soft_wrap, self.overflow, self.max_lines),
            self.overflow,
        )
    }

    /// Whether laying out in `constraints` and `layout_direction` could come out other than the last layout
    /// (Compose's newLayoutWillBeDifferent).
    fn new_layout_will_be_different(&self, constraints: Constraints, layout_direction: LayoutDirection) -> bool {
        let Some(last) = &self.layout_cache else {
            return true;
        };
        if last.multi_paragraph.intrinsics().has_stale_resolved_fonts() {
            return true;
        }
        let input = &last.layout_input;
        if layout_direction != input.layout_direction {
            return true;
        }
        if constraints == input.constraints {
            return false;
        }
        if constraints.max_width != input.constraints.max_width || constraints.min_width != input.constraints.min_width
        {
            return true;
        }
        // The width stays; the height may cut it off.
        (constraints.max_height as f32) < last.multi_paragraph.height() || last.multi_paragraph.did_exceed_max_lines()
    }

    fn mark_dirty(&mut self) {
        self.paragraph_intrinsics = None;
        self.layout_cache = None;
        self.cached_intrinsic_height = -1;
        self.cached_intrinsic_height_input_width = -1;
        self.auto_size_last_layout = None;
    }

    fn mark_style_affected_dirty(&mut self) {
        self.paragraph_intrinsics = None;
        self.layout_cache = None;
        self.cached_intrinsic_height = -1;
        self.cached_intrinsic_height_input_width = -1;
    }

    /// The width past which the text gets no shorter (Compose's maxIntrinsicWidth).
    pub(crate) fn max_intrinsic_width(&mut self, layout_direction: LayoutDirection) -> i32 {
        ceil_to_int_px(self.set_layout_direction(layout_direction).max_intrinsic_width())
    }

    /// The narrowest the text can be without breaking inside a word (Compose's minIntrinsicWidth).
    pub(crate) fn min_intrinsic_width(&mut self, layout_direction: LayoutDirection) -> i32 {
        ceil_to_int_px(self.set_layout_direction(layout_direction).min_intrinsic_width())
    }
}

/// `size` times an em `factor` (Compose's TextUnit.times(TextUnit)): of the default 14 sp where `size` is
/// unspecified; an em `size` panics.
#[track_caller]
fn times_em(size: TextUnit, factor: TextUnit) -> TextUnit {
    assert!(factor.is_em(), "kompoz: The multiplier must be in em, but was {factor:?}.");
    assert!(
        !size.is_em(),
        "kompoz: Cannot convert Em to Px when style.fontSize is Em ({factor:?}). Please declare the style.fontSize \
         with Sp units instead."
    );
    if size.is_unspecified() { DefaultFontSize * factor.value() } else { size * factor.value() }
}

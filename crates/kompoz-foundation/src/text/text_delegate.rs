//! What lays a value-based field's shown text out (Compose's TextDelegate.kt): its text and everything its layout
//! depends on, the intrinsics kept per layout direction, and a layout reusing the last one where it can.

use super::modifiers::layout_utils::ceil_to_int_px;
use kompoz_graphics::{Constraints, Density, IntSize, LayoutDirection};
use kompoz_text::{
    AnnotatedString, FontFamilyResolver, MultiParagraph, MultiParagraphIntrinsics, Placeholder, TextLayoutInput,
    TextLayoutResult, TextOverflow, TextStyle,
};
use std::cell::RefCell;
use std::rc::Rc;

type AnnotatedRange<T> = kompoz_text::Range<T>;

/// What lays a field's text out (Compose's TextDelegate): immutable but for its intrinsics, made once per layout
/// direction. Cheap to clone: clones are the same delegate ([`same_as`](Self::same_as)).
#[derive(Clone)]
pub struct TextDelegate(Rc<Inner>);

struct Inner {
    text: AnnotatedString,
    style: TextStyle,
    max_lines: usize,
    min_lines: usize,
    soft_wrap: bool,
    overflow: TextOverflow,
    density: Density,
    font_family_resolver: Rc<dyn FontFamilyResolver>,
    placeholders: Vec<AnnotatedRange<Placeholder>>,
    paragraph_intrinsics: RefCell<Option<(Rc<MultiParagraphIntrinsics>, LayoutDirection)>>,
}

impl TextDelegate {
    /// One laying `text` out in `style`; panics where the min lines exceed the max ones, or either is 0 (Compose's).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        text: AnnotatedString,
        style: TextStyle,
        max_lines: usize,
        min_lines: usize,
        soft_wrap: bool,
        overflow: TextOverflow,
        density: Density,
        font_family_resolver: Rc<dyn FontFamilyResolver>,
        placeholders: Vec<AnnotatedRange<Placeholder>>,
    ) -> TextDelegate {
        assert!(max_lines > 0, "no maxLines");
        assert!(min_lines > 0, "no minLines");
        assert!(min_lines <= max_lines, "minLines greater than maxLines");
        TextDelegate(Rc::new(Inner {
            text,
            style,
            max_lines,
            min_lines,
            soft_wrap,
            overflow,
            density,
            font_family_resolver,
            placeholders,
            paragraph_intrinsics: RefCell::new(None),
        }))
    }

    /// One with Compose's defaults: any number of lines, wrapping, clipped, no placeholders.
    pub fn with_defaults(
        text: AnnotatedString,
        style: TextStyle,
        soft_wrap: bool,
        density: Density,
        font_family_resolver: Rc<dyn FontFamilyResolver>,
    ) -> TextDelegate {
        TextDelegate::new(
            text,
            style,
            usize::MAX,
            1,
            soft_wrap,
            TextOverflow::Clip,
            density,
            font_family_resolver,
            Vec::new(),
        )
    }

    /// Whether `other` is this same delegate.
    pub fn same_as(&self, other: &TextDelegate) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub fn text(&self) -> &AnnotatedString {
        &self.0.text
    }
    pub fn style(&self) -> &TextStyle {
        &self.0.style
    }
    pub fn max_lines(&self) -> usize {
        self.0.max_lines
    }
    pub fn min_lines(&self) -> usize {
        self.0.min_lines
    }
    pub fn soft_wrap(&self) -> bool {
        self.0.soft_wrap
    }
    pub fn overflow(&self) -> TextOverflow {
        self.0.overflow
    }
    pub fn density(&self) -> Density {
        self.0.density
    }
    pub fn font_family_resolver(&self) -> &Rc<dyn FontFamilyResolver> {
        &self.0.font_family_resolver
    }
    pub fn placeholders(&self) -> &[AnnotatedRange<Placeholder>] {
        &self.0.placeholders
    }

    fn intrinsics(&self) -> Rc<MultiParagraphIntrinsics> {
        self.0
            .paragraph_intrinsics
            .borrow()
            .as_ref()
            .map(|(i, _)| i.clone())
            .expect("layoutIntrinsics must be called first")
    }

    /// The text's narrowest width, its longest word's (Compose's `minIntrinsicWidth`); after
    /// [`layout_intrinsics`](Self::layout_intrinsics).
    pub fn min_intrinsic_width(&self) -> i32 {
        ceil_to_int_px(self.intrinsics().min_intrinsic_width())
    }

    /// The text's unwrapped width (Compose's `maxIntrinsicWidth`); after [`layout_intrinsics`](Self::layout_intrinsics).
    pub fn max_intrinsic_width(&self) -> i32 {
        ceil_to_int_px(self.intrinsics().max_intrinsic_width())
    }

    /// Shapes the text for `layout_direction` unless it was, its fonts not changed since (Compose's
    /// `layoutIntrinsics`).
    pub fn layout_intrinsics(&self, layout_direction: LayoutDirection) {
        let mut cached = self.0.paragraph_intrinsics.borrow_mut();
        let stale = match cached.as_ref() {
            None => true,
            Some((intrinsics, direction)) => *direction != layout_direction || intrinsics.has_stale_resolved_fonts(),
        };
        if stale {
            let inner = &self.0;
            let intrinsics = MultiParagraphIntrinsics::new(
                &inner.text,
                &inner.style.resolve_defaults(layout_direction),
                &inner.placeholders,
                inner.density,
                &inner.font_family_resolver,
                inner.soft_wrap,
            );
            *cached = Some((Rc::new(intrinsics), layout_direction));
        }
    }

    /// The text laid out in `constraints` (Compose's `layoutText`): as wide as its unwrapped width within them.
    fn layout_text(&self, constraints: Constraints, layout_direction: LayoutDirection) -> MultiParagraph {
        self.layout_intrinsics(layout_direction);
        let inner = &self.0;
        let min_width = constraints.min_width;
        let width_matters = inner.soft_wrap || inner.overflow == TextOverflow::Ellipsis;
        let max_width = if width_matters && constraints.has_bounded_width() {
            constraints.max_width
        } else {
            Constraints::Infinity
        };
        // (one line where text that doesn't wrap is ellipsized: a layout ellipsizes one line, not each)
        let overwrite_max_lines = !inner.soft_wrap && inner.overflow == TextOverflow::Ellipsis;
        let final_max_lines = if overwrite_max_lines { 1 } else { inner.max_lines };
        let width =
            if min_width == max_width { max_width } else { self.max_intrinsic_width().clamp(min_width, max_width) };
        MultiParagraph::new(
            self.intrinsics(),
            Constraints::fit_prioritizing_width(0, width, 0, constraints.max_height),
            final_max_lines,
            inner.overflow,
        )
    }

    /// The text laid out in `constraints` and `layout_direction` (Compose's `layout`): `prev_result` again (in
    /// these constraints, with this style) when it can be reused.
    pub fn layout(
        &self,
        constraints: Constraints,
        layout_direction: LayoutDirection,
        prev_result: Option<&TextLayoutResult>,
    ) -> TextLayoutResult {
        let inner = &self.0;
        if let Some(prev) = prev_result
            && can_reuse(prev, self, layout_direction, constraints)
        {
            let input = &prev.layout_input;
            let layout_input = TextLayoutInput {
                text: input.text.clone(),
                style: inner.style.clone(),
                placeholders: input.placeholders.clone(),
                max_lines: input.max_lines,
                soft_wrap: input.soft_wrap,
                overflow: input.overflow,
                density: input.density,
                layout_direction: input.layout_direction,
                font_family_resolver: input.font_family_resolver.clone(),
                constraints,
            };
            let size = constraints.constrain_size(IntSize::new(
                ceil_to_int_px(prev.multi_paragraph.width()),
                ceil_to_int_px(prev.multi_paragraph.height()),
            ));
            return prev.copy(Rc::new(layout_input), size);
        }
        let multi_paragraph = self.layout_text(constraints, layout_direction);
        let size = constraints.constrain_size(IntSize::new(
            ceil_to_int_px(multi_paragraph.width()),
            ceil_to_int_px(multi_paragraph.height()),
        ));
        let layout_input = TextLayoutInput {
            text: inner.text.clone(),
            style: inner.style.clone(),
            placeholders: inner.placeholders.clone(),
            max_lines: inner.max_lines,
            soft_wrap: inner.soft_wrap,
            overflow: inner.overflow,
            density: inner.density,
            layout_direction,
            font_family_resolver: inner.font_family_resolver.clone(),
            constraints,
        };
        TextLayoutResult::new(Rc::new(layout_input), Rc::new(multi_paragraph), size)
    }
}

/// Whether `result` is what laying `delegate`'s text out in `constraints` would give (Compose's
/// `TextLayoutResult.canReuse`): its fonts unchanged, the same inputs (a style laying out alike), the same min
/// width, and where the width matters the same max width and height.
fn can_reuse(
    result: &TextLayoutResult,
    delegate: &TextDelegate,
    layout_direction: LayoutDirection,
    constraints: Constraints,
) -> bool {
    let input = &result.layout_input;
    if result.multi_paragraph.intrinsics().has_stale_resolved_fonts() {
        return false;
    }
    let d = &delegate.0;
    let same_resolver = std::ptr::eq(
        Rc::as_ptr(&input.font_family_resolver) as *const (),
        Rc::as_ptr(&d.font_family_resolver) as *const (),
    );
    if !(input.text == d.text
        && input.style.has_same_layout_affecting_attributes(&d.style)
        && input.placeholders == d.placeholders
        && input.max_lines == d.max_lines
        && input.soft_wrap == d.soft_wrap
        && input.overflow == d.overflow
        && input.density == d.density
        && input.layout_direction == layout_direction
        && same_resolver)
    {
        return false;
    }
    if constraints.min_width != input.constraints.min_width {
        return false;
    }
    if !(d.soft_wrap || d.overflow == TextOverflow::Ellipsis) {
        // (the width doesn't matter: the same layout)
        return true;
    }
    constraints.max_width == input.constraints.max_width && constraints.max_height == input.constraints.max_height
}

/// `current` if it lays out what is given, else a new delegate of it (Compose's `updateTextDelegate`).
#[allow(clippy::too_many_arguments)]
pub fn update_text_delegate(
    current: &TextDelegate,
    text: &AnnotatedString,
    style: &TextStyle,
    density: Density,
    font_family_resolver: &Rc<dyn FontFamilyResolver>,
    soft_wrap: bool,
    overflow: TextOverflow,
    max_lines: usize,
    min_lines: usize,
    placeholders: &[AnnotatedRange<Placeholder>],
) -> TextDelegate {
    let c = &current.0;
    let same_resolver =
        std::ptr::eq(Rc::as_ptr(&c.font_family_resolver) as *const (), Rc::as_ptr(font_family_resolver) as *const ());
    if c.text != *text
        || c.style != *style
        || c.soft_wrap != soft_wrap
        || c.overflow != overflow
        || c.max_lines != max_lines
        || c.min_lines != min_lines
        || c.density != density
        || c.placeholders != placeholders
        || !same_resolver
    {
        TextDelegate::new(
            text.clone(),
            style.clone(),
            max_lines,
            min_lines,
            soft_wrap,
            overflow,
            density,
            font_family_resolver.clone(),
            placeholders.to_vec(),
        )
    } else {
        current.clone()
    }
}

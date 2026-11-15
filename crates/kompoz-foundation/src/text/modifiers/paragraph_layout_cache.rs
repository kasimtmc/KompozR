//! A plain string's layout, kept (Compose's ParagraphLayoutCache): what [`TextStringSimpleNode`] lays out with.
//!
//! [`TextStringSimpleNode`]: super::text_string_simple::TextStringSimpleNode

use super::layout_utils::{ceil_to_int_px, final_constraints, final_max_lines};
use super::min_lines_constrainer::MinLinesConstrainer;
use kompoz_graphics::{Constraints, Density, IntSize, LayoutDirection};
use kompoz_text::{
    AnnotatedString, FontFamilyResolver, MultiParagraph, MultiParagraphIntrinsics, Paragraph, ParagraphIntrinsics,
    TextLayoutInput, TextLayoutResult, TextOverflow, TextStyle,
};
use std::rc::Rc;

/// Lays a plain string out as a [`Paragraph`], keeping what it can (Compose's ParagraphLayoutCache): its
/// intrinsics while its text, style and direction stay; its paragraph while new constraints can't reflow it;
/// its height at the last width asked for.
pub(crate) struct ParagraphLayoutCache {
    text: String,
    style: TextStyle,
    font_family_resolver: Rc<dyn FontFamilyResolver>,
    overflow: TextOverflow,
    soft_wrap: bool,
    max_lines: usize,
    min_lines: usize,
    /// The density it lays out in (none until a measure gives one).
    density: Option<Density>,
    /// The last paragraph laid out.
    pub(crate) paragraph: Option<Rc<Paragraph>>,
    /// Whether the text overflows the size it was given (and doesn't show what overflows).
    pub(crate) did_overflow: bool,
    /// The last size it came out (what a TextLayoutResult would say).
    pub(crate) layout_size: IntSize,
    min_lines_constrainer: Option<MinLinesConstrainer>,
    paragraph_intrinsics: Option<Rc<ParagraphIntrinsics>>,
    intrinsics_layout_direction: Option<LayoutDirection>,
    prev_constraints: Constraints,
    cached_intrinsic_height_input_width: i32,
    cached_intrinsic_height: i32,
}

impl ParagraphLayoutCache {
    pub(crate) fn new(
        text: String,
        style: TextStyle,
        font_family_resolver: Rc<dyn FontFamilyResolver>,
        overflow: TextOverflow,
        soft_wrap: bool,
        max_lines: usize,
        min_lines: usize,
    ) -> ParagraphLayoutCache {
        ParagraphLayoutCache {
            text,
            style,
            font_family_resolver,
            overflow,
            soft_wrap,
            max_lines,
            min_lines,
            density: None,
            paragraph: None,
            did_overflow: false,
            layout_size: IntSize::new(0, 0),
            min_lines_constrainer: None,
            paragraph_intrinsics: None,
            intrinsics_layout_direction: None,
            prev_constraints: Constraints::fixed(0, 0),
            cached_intrinsic_height_input_width: -1,
            cached_intrinsic_height: -1,
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

    /// Reads whether a face it laid out in has changed (an async font loaded), so a measure reading it runs
    /// again when it does (Compose's observeFontChanges).
    pub(crate) fn observe_font_changes(&self) {
        if let Some(i) = &self.paragraph_intrinsics {
            i.has_stale_resolved_fonts();
        }
    }

    /// Lays the text out in `constraints` (Compose's layoutWithConstraints), its min lines made a min height;
    /// whether the layout changed. Constraints that can't reflow it keep the paragraph, its size and overflow
    /// worked out again.
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
            if final_constraints != self.prev_constraints {
                // Its size and overflow in these constraints.
                let paragraph = self.paragraph.clone().expect("kompoz: a paragraph laid out");
                let layout_width = paragraph.max_intrinsic_width().min(paragraph.width());
                let size = final_constraints
                    .constrain_size(IntSize::new(ceil_to_int_px(layout_width), ceil_to_int_px(paragraph.height())));
                self.layout_size = size;
                self.did_overflow = self.overflow != TextOverflow::Visible
                    && ((size.width as f32) < paragraph.width() || (size.height as f32) < paragraph.height());
                self.prev_constraints = final_constraints;
            }
            return false;
        }
        let paragraph = Rc::new(self.layout_text(final_constraints, layout_direction));
        self.prev_constraints = final_constraints;
        let size = final_constraints
            .constrain_size(IntSize::new(ceil_to_int_px(paragraph.width()), ceil_to_int_px(paragraph.height())));
        self.layout_size = size;
        self.did_overflow = self.overflow != TextOverflow::Visible
            && ((size.width as f32) < paragraph.width() || (size.height as f32) < paragraph.height());
        self.paragraph = Some(paragraph);
        true
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
        text: String,
        style: TextStyle,
        font_family_resolver: Rc<dyn FontFamilyResolver>,
        overflow: TextOverflow,
        soft_wrap: bool,
        max_lines: usize,
        min_lines: usize,
    ) {
        self.text = text;
        self.style = style;
        self.font_family_resolver = font_family_resolver;
        self.overflow = overflow;
        self.soft_wrap = soft_wrap;
        self.max_lines = max_lines;
        self.min_lines = min_lines;
        self.mark_dirty();
    }

    /// The text measured in `layout_direction`: kept while the direction stays and its faces are fresh
    /// (Compose's setLayoutDirection).
    fn set_layout_direction(&mut self, layout_direction: LayoutDirection) -> Rc<ParagraphIntrinsics> {
        if let Some(i) = &self.paragraph_intrinsics
            && self.intrinsics_layout_direction == Some(layout_direction)
            && !i.has_stale_resolved_fonts()
        {
            return i.clone();
        }
        self.intrinsics_layout_direction = Some(layout_direction);
        let intrinsics = Rc::new(ParagraphIntrinsics::new(
            &self.text,
            &self.style.resolve_defaults(layout_direction),
            &[],
            &[],
            self.density.expect("kompoz: a density to lay text out in"),
            &self.font_family_resolver,
            self.soft_wrap,
        ));
        self.paragraph_intrinsics = Some(intrinsics.clone());
        intrinsics
    }

    /// The text laid out in `constraints` (Compose's layoutText): as close to its unwrapped width as they let it.
    pub(crate) fn layout_text(&mut self, constraints: Constraints, layout_direction: LayoutDirection) -> Paragraph {
        let intrinsics = self.set_layout_direction(layout_direction);
        let max_intrinsic_width = intrinsics.max_intrinsic_width();
        Paragraph::new(
            intrinsics,
            final_constraints(constraints, self.soft_wrap, self.overflow, max_intrinsic_width),
            final_max_lines(self.soft_wrap, self.overflow, self.max_lines),
            self.overflow,
        )
    }

    /// Whether laying out in `constraints` and `layout_direction` could come out other than the last layout
    /// (Compose's newLayoutWillBeDifferent).
    fn new_layout_will_be_different(&self, constraints: Constraints, layout_direction: LayoutDirection) -> bool {
        let (Some(paragraph), Some(intrinsics)) = (&self.paragraph, &self.paragraph_intrinsics) else {
            return true;
        };
        if intrinsics.has_stale_resolved_fonts() {
            return true;
        }
        if Some(layout_direction) != self.intrinsics_layout_direction {
            return true;
        }
        if constraints == self.prev_constraints {
            return false;
        }
        if constraints.max_width != self.prev_constraints.max_width
            || constraints.min_width != self.prev_constraints.min_width
        {
            return true;
        }
        // The width stays; the height may cut it off.
        (constraints.max_height as f32) < paragraph.height() || paragraph.did_exceed_max_lines()
    }

    fn mark_dirty(&mut self) {
        self.paragraph = None;
        self.paragraph_intrinsics = None;
        self.intrinsics_layout_direction = None;
        self.cached_intrinsic_height_input_width = -1;
        self.cached_intrinsic_height = -1;
        self.prev_constraints = Constraints::fixed(0, 0);
        self.layout_size = IntSize::new(0, 0);
        self.did_overflow = false;
    }

    /// The layout as a [`TextLayoutResult`] in `style`, laid out again as a [`MultiParagraph`] (Compose's
    /// slowCreateTextLayoutResultOrNull, for semantics' GetTextLayoutResult): none before it was laid out. Its
    /// input's constraints are the last ones' max dimensions, as Compose's; it is laid out in the width and lines
    /// the paragraph drawn was (Compose's lays it out in the max width, so a centered line moves).
    pub(crate) fn slow_create_text_layout_result_or_none(&self, style: TextStyle) -> Option<TextLayoutResult> {
        let layout_direction = self.intrinsics_layout_direction?;
        let density = self.density?;
        let drawn = self.paragraph.as_ref()?;
        self.paragraph_intrinsics.as_ref()?;
        let annotated = AnnotatedString::new(self.text.as_str());
        let input_constraints = self.prev_constraints.copy_max_dimensions();
        let intrinsics = Rc::new(MultiParagraphIntrinsics::new(
            &annotated,
            &style.resolve_defaults(layout_direction),
            &[],
            density,
            &self.font_family_resolver,
            self.soft_wrap,
        ));
        let multi_paragraph = MultiParagraph::new(intrinsics, drawn.constraints(), drawn.max_lines(), self.overflow);
        let input = TextLayoutInput {
            text: annotated,
            style,
            placeholders: Vec::new(),
            max_lines: self.max_lines,
            soft_wrap: self.soft_wrap,
            overflow: self.overflow,
            density,
            layout_direction,
            font_family_resolver: self.font_family_resolver.clone(),
            constraints: input_constraints,
        };
        Some(TextLayoutResult::new(Rc::new(input), Rc::new(multi_paragraph), self.layout_size))
    }

    /// The narrowest the text can be without breaking inside a word (Compose's minIntrinsicWidth).
    pub(crate) fn min_intrinsic_width(&mut self, layout_direction: LayoutDirection) -> i32 {
        ceil_to_int_px(self.set_layout_direction(layout_direction).min_intrinsic_width())
    }

    /// The width past which the text gets no shorter (Compose's maxIntrinsicWidth).
    pub(crate) fn max_intrinsic_width(&mut self, layout_direction: LayoutDirection) -> i32 {
        ceil_to_int_px(self.set_layout_direction(layout_direction).max_intrinsic_width())
    }
}

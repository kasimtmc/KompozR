//! Drawing laid out text (Compose's TextPainter: DrawScope.drawText): a [`TextLayoutResult`] at a top left, in a
//! color or a brush of the draw call's (spread over all of the text) where the text's spans gave none of their
//! own, with a shadow, a decoration and a draw style of the call's or the text's, blended, clipped to its size where
//! it overflows and doesn't show what overflows; or text measured by a [`TextMeasurer`] and drawn at once; or a
//! [`Paragraph`] or [`MultiParagraph`] painted as it is (Compose's Paragraph.paint, what a text node draws). Each
//! run is drawn as the paragraph's draw list has it (built once per layout), through the [`GraphicsContext`]'s
//! text calls.
//!
//! [`GraphicsContext`]: crate::graphics_context::GraphicsContext

use super::scope::faded;
use super::{DefaultBlendMode, DrawScope, DrawStyle};
use crate::graphics_context::CornerRadii;
use kompoz_graphics::{
    BlendMode, Brush, Color, Constraints, Offset, Path, PathDirection, PathStroke, Rect, Shadow, Size,
};
use kompoz_runtime::named;
use kompoz_text::{
    AnnotatedString, MultiParagraph, Paragraph, Placeholder, TextDecoration, TextForegroundStyle, TextLayoutResult,
    TextMeasurer, TextOverflow, TextPaint, TextPaintOp, TextPaintStyle, TextStyle,
};

impl<'a> DrawScope<'a> {
    /// Draws `ops` (in a layout's coordinates) from `top_left`, clipped to `clip` (from there) if given, blended
    /// by `blend_mode`.
    fn paint_text_ops(&mut self, ops: &[TextPaintOp], top_left: Offset, clip: Option<Size>, blend_mode: BlendMode) {
        let at = self.origin + top_left;
        if let Some(size) = clip {
            self.canvas.push_clip(Rect::from_offset_size(at, size));
        }
        let blended = blend_mode != BlendMode::SrcOver;
        if blended {
            self.canvas.push_blend_mode(blend_mode);
        }
        for op in ops {
            match op {
                TextPaintOp::Background { rect, color } => {
                    self.canvas.fill_round_rect(rect.translate(at), CornerRadii::Zero, *color);
                }
                TextPaintOp::Shadow { text, color, blur_radius } => {
                    self.canvas.draw_text_shadow(&text.at(text.baseline + at), *color, *blur_radius);
                }
                TextPaintOp::Text { text, paint } => {
                    let moved = text.at(text.baseline + at);
                    match paint {
                        TextPaint::Color(c) => self.canvas.draw_text(&moved, *c),
                        TextPaint::Brush { brush, area, alpha } => {
                            let b = if alpha.is_nan() { brush.clone() } else { brush.faded(*alpha) };
                            self.canvas.draw_text_brush(&moved, &b, area.translate(at));
                        }
                    }
                }
                TextPaintOp::Line { from, to, thickness, paint } => match paint {
                    TextPaint::Color(c) => self.canvas.draw_line(*from + at, *to + at, *thickness, *c),
                    TextPaint::Brush { brush, area, alpha } => {
                        let half = thickness / 2.0;
                        let mut path = Path::new();
                        path.add_rect(
                            Rect::new(from.x - half, from.y - half, to.x + half, to.y + half),
                            PathDirection::Clockwise,
                        );
                        let b = if alpha.is_nan() { brush.clone() } else { brush.faded(*alpha) };
                        self.canvas.draw_path(&path, at, &b, area.translate(at), &PathStroke::default());
                    }
                },
            }
        }
        if blended {
            self.canvas.pop_blend_mode();
        }
        if clip.is_some() {
            self.canvas.pop_clip();
        }
    }

    /// `result` painted with `paint` at `top_left` (Compose's drawText's body): clipped to its size where it
    /// overflows and its overflow isn't Visible.
    fn paint_text_layout(
        &mut self,
        result: &TextLayoutResult,
        paint: &TextPaintStyle,
        top_left: Offset,
        blend_mode: BlendMode,
    ) {
        let clip = (result.has_visual_overflow() && result.layout_input.overflow != TextOverflow::Visible)
            .then(|| result.size.to_size());
        let ops = result.multi_paragraph.paint_ops(paint);
        self.paint_text_ops(&ops, top_left, clip, blend_mode);
    }

    /// Paints `paragraph` from the scope's top left with `paint` (Compose's Paragraph.paint(canvas, color or
    /// brush, …)): `paint`'s color or brush (a brush spread over the paragraph), shadow, decoration and draw style
    /// where its spans gave none, blended by `blend_mode`; not clipped (its caller clips what overflows).
    pub fn paint_paragraph(&mut self, paragraph: &Paragraph, paint: &TextPaintStyle, blend_mode: BlendMode) {
        let area = Rect::new(0.0, 0.0, paragraph.width(), paragraph.height());
        let ops = paragraph.paint_ops(paint, area, 0.0);
        self.paint_text_ops(&ops, Offset::Zero, None, blend_mode);
    }

    /// Paints `multi_paragraph` the same way (Compose's MultiParagraph.paint): its pieces one under another, a
    /// brush `paint` gives spread over all of it.
    pub fn paint_multi_paragraph(
        &mut self,
        multi_paragraph: &MultiParagraph,
        paint: &TextPaintStyle,
        blend_mode: BlendMode,
    ) {
        let ops = multi_paragraph.paint_ops(paint);
        self.paint_text_ops(&ops, Offset::Zero, None, blend_mode);
    }

    /// The constraints text drawn here is measured in (Compose's textLayoutConstraints): `size` where given (its
    /// sides fixed), else what is left of the scope from `top_left`.
    fn text_layout_constraints(&self, size: Size, top_left: Offset) -> Constraints {
        let (min_width, max_width) = if size.is_unspecified() || size.width.is_nan() {
            (0, ((self.size.width - top_left.x).ceil().round() as i32).max(0))
        } else {
            let w = size.width.ceil().round() as i32;
            (w, w)
        };
        let (min_height, max_height) = if size.is_unspecified() || size.height.is_nan() {
            (0, ((self.size.height - top_left.y).ceil().round() as i32).max(0))
        } else {
            let h = size.height.ceil().round() as i32;
            (h, h)
        };
        Constraints::new(min_width, max_width, min_height, max_height)
    }

    /// [`draw_text`](Self::draw_text) with every parameter, for plain code.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_with(
        &mut self,
        text_layout_result: &TextLayoutResult,
        color: Color,
        top_left: Offset,
        alpha: f32,
        shadow: Option<Shadow>,
        text_decoration: Option<TextDecoration>,
        draw_style: Option<DrawStyle>,
        blend_mode: BlendMode,
    ) {
        let style = &text_layout_result.layout_input.style;
        let foreground = match &style.brush {
            Some(brush) if color.is_unspecified() => {
                TextForegroundStyle::from_brush(Some(brush), if alpha.is_nan() { style.alpha } else { alpha })
            }
            _ => {
                let c = if color.is_specified() { color } else { style.color };
                TextForegroundStyle::from_color(if c.is_specified() { faded(c, alpha) } else { c })
            }
        };
        let paint = TextPaintStyle {
            foreground,
            shadow: shadow.or_else(|| style.shadow.clone()),
            decoration: text_decoration.or(style.text_decoration),
            draw_style: draw_style.or_else(|| style.draw_style.clone()),
        };
        self.paint_text_layout(text_layout_result, &paint, top_left, blend_mode);
    }

    /// [`draw_text_brush`](Self::draw_text_brush) with every parameter, for plain code.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_brush_with(
        &mut self,
        text_layout_result: &TextLayoutResult,
        brush: impl Into<Brush>,
        top_left: Offset,
        alpha: f32,
        shadow: Option<Shadow>,
        text_decoration: Option<TextDecoration>,
        draw_style: Option<DrawStyle>,
        blend_mode: BlendMode,
    ) {
        let style = &text_layout_result.layout_input.style;
        let brush = brush.into();
        let paint = TextPaintStyle {
            foreground: TextForegroundStyle::from_brush(Some(&brush), if alpha.is_nan() { style.alpha } else { alpha }),
            shadow: shadow.or_else(|| style.shadow.clone()),
            decoration: text_decoration.or(style.text_decoration),
            draw_style: draw_style.or_else(|| style.draw_style.clone()),
        };
        self.paint_text_layout(text_layout_result, &paint, top_left, blend_mode);
    }

    /// [`draw_text_measurer`](Self::draw_text_measurer) with every parameter, for plain code.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_measurer_with(
        &mut self,
        text_measurer: &TextMeasurer,
        text: impl Into<AnnotatedString>,
        top_left: Offset,
        style: TextStyle,
        overflow: TextOverflow,
        soft_wrap: bool,
        max_lines: usize,
        placeholders: Vec<kompoz_text::Range<Placeholder>>,
        size: Size,
        blend_mode: BlendMode,
    ) {
        let constraints = self.text_layout_constraints(size, top_left);
        let result = text_measurer.measure_with(
            text,
            style,
            overflow,
            soft_wrap,
            max_lines,
            placeholders,
            constraints,
            self.layout_direction,
            self.density,
            text_measurer.default_font_family_resolver().clone(),
            false,
        );
        // Painted in the style it was measured in (a layout kept for another style that lays out alike is given
        // this one's colors, shadow, decoration and draw style).
        self.draw_text_with(&result, Color::Unspecified, top_left, f32::NAN, None, None, None, blend_mode);
    }
}

#[named]
impl<'a> DrawScope<'a> {
    /// Draws laid out text from `top_left` (Compose's drawText(textLayoutResult, color, …)): in `color` (faded by
    /// `alpha`) where its spans gave none of their own — unspecified, the text's own color, or its brush if it has
    /// one (at `alpha`, or its own) — with `shadow`, `text_decoration` and `draw_style` where its spans gave none
    /// (the text style's where these are None), blended by `blend_mode`; clipped to its size where it overflows
    /// (unless its overflow is Visible).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(
        &mut self,
        text_layout_result: &TextLayoutResult,
        #[default(Color::Unspecified)] color: Color,
        #[default(Offset::Zero)] top_left: Offset,
        #[default(f32::NAN)] alpha: f32,
        #[default(None)] shadow: Option<Shadow>,
        #[default(None)] text_decoration: Option<TextDecoration>,
        #[default(None)] draw_style: Option<DrawStyle>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        self.draw_text_with(
            text_layout_result,
            color,
            top_left,
            alpha,
            shadow,
            text_decoration,
            draw_style,
            blend_mode,
        );
    }

    /// Draws laid out text from `top_left` painted with `brush` spread over all of it (Compose's
    /// drawText(textLayoutResult, brush, …)), where its spans gave no color or brush of their own; otherwise as
    /// [`draw_text`](Self::draw_text).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_brush(
        &mut self,
        text_layout_result: &TextLayoutResult,
        brush: impl Into<Brush>,
        #[default(Offset::Zero)] top_left: Offset,
        #[default(f32::NAN)] alpha: f32,
        #[default(None)] shadow: Option<Shadow>,
        #[default(None)] text_decoration: Option<TextDecoration>,
        #[default(None)] draw_style: Option<DrawStyle>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        self.draw_text_brush_with(
            text_layout_result,
            brush,
            top_left,
            alpha,
            shadow,
            text_decoration,
            draw_style,
            blend_mode,
        );
    }

    /// Lays `text` out with `text_measurer` and draws it from `top_left` (Compose's drawText(textMeasurer, text,
    /// …)): in `style`, cut as `overflow` says, wrapping where `soft_wrap` says, at most `max_lines` lines, in
    /// `size` (unspecified: what is left of the scope from `top_left`), blended by `blend_mode`; the scope's density
    /// and layout direction.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_measurer(
        &mut self,
        text_measurer: &TextMeasurer,
        text: impl Into<AnnotatedString>,
        #[default(Offset::Zero)] top_left: Offset,
        #[default(TextStyle::Default)] style: TextStyle,
        #[default(TextOverflow::Clip)] overflow: TextOverflow,
        #[default(true)] soft_wrap: bool,
        #[default(usize::MAX)] max_lines: usize,
        #[default(Vec::new())] placeholders: Vec<kompoz_text::Range<Placeholder>>,
        #[default(Size::Unspecified)] size: Size,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        self.draw_text_measurer_with(
            text_measurer,
            text,
            top_left,
            style,
            overflow,
            soft_wrap,
            max_lines,
            placeholders,
            size,
            blend_mode,
        );
    }
}

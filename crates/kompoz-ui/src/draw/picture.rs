//! A recording of drawing (Android's Picture, what a RenderNode's display list holds): a [`GraphicsContext`] that
//! keeps every call, to replay it into another as often as wanted. A GraphicsLayer records into one, and so does a
//! canvas over a bitmap until the platform's renderer draws it.
//!
//! (C++ recorded both into a DisplayList. That is a frame's recording for damage: it drops what falls outside its
//! viewport and answers `quick_reject` by it, where a layer's recording has to keep what it draws past its bounds
//! (a RenderNode clips only when asked) and be replayed wherever it is drawn; it lines frames up by the nodes drawn
//! but doesn't replay the node hooks, which a layer drawn into the host's display list must hand on so the nodes
//! in it are kept or drawn as theirs; and it works out screen bounds for each call, which a recording replayed
//! elsewhere never uses.)

use crate::graphics_context::{
    ClipOp, CornerRadii, ElevationShadow, GraphicsContext, LayerTransform, Shadow, ShadowLight, TextDraw,
};
use kompoz_graphics::{
    BlendMode, Brush, Color, ColorFilter, Density, FilterQuality, ImageBitmap, LayoutDirection, Matrix, Offset,
    Outline, Path, PathStroke, Rect, RenderEffect, Shape, Size, StretchEffect, Vertices,
};

/// One call recorded, with its arguments.
#[derive(Clone, Debug)]
enum Op {
    FillRoundRect(Rect, CornerRadii, Color),
    StrokeRoundRect(Rect, CornerRadii, f32, Color),
    Line(Offset, Offset, f32, Color),
    Text(TextDraw, Color),
    TextBrush(TextDraw, Brush, Rect),
    TextShadow(TextDraw, Color, f32),
    Arc(Rect, f32, f32, f32, Color, bool),
    Image(ImageBitmap, Rect, Rect, f32, FilterQuality),
    ImageMask(ImageBitmap, Rect, Rect, Brush, Rect, f32, FilterQuality),
    PushLayer(Rect, CornerRadii, f32, LayerTransform),
    PopLayer,
    PushClip(Rect),
    PushClipRoundRect(Rect, CornerRadii, ClipOp),
    PushClipPath(Path, Offset, ClipOp),
    PopClip,
    PushTranslation(Offset),
    PopTranslation,
    PushTransform(Matrix),
    PopTransform,
    PushEffectLayer(Rect, RenderEffect, f32),
    PushStretchLayer(Rect, StretchEffect, Option<RenderEffect>, f32),
    Backdrop(Rect, RenderEffect, f32),
    PushBlendMode(BlendMode),
    PopBlendMode,
    PushAntiAlias(bool),
    PopAntiAlias,
    PushColorFilter(ColorFilter),
    PopColorFilter,
    FillGradient(Rect, CornerRadii, Brush, Rect),
    Vertices(Vertices, Offset, BlendMode, Brush, Rect, f32),
    Path(Path, Offset, Brush, Rect, PathStroke),
    Shadow(Rect, CornerRadii, Shadow, bool),
    ShadowPath(Path, Offset, Shadow, bool),
    ElevationShadow(Outline, Offset, ElevationShadow),
    OutlineLayer(Rect, Path, f32, LayerTransform),
    ShapeShadow(Rect, Outline, LayoutDirection, Density, Shadow, bool),
    BeginNode(u64),
    EndNode,
}

/// What was drawn into it, in order; replayed into any context ([`replay`](Picture::replay)), whose light,
/// translations and clips it draws under then (an elevation shadow is cast from the light of the context replayed
/// into, as a RenderNode's is). It rejects nothing.
#[derive(Clone, Debug, Default)]
pub struct Picture {
    ops: Vec<Op>,
}

impl Picture {
    pub fn new() -> Picture {
        Picture::default()
    }

    /// How many calls it holds.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Forgets what it holds.
    pub fn clear(&mut self) {
        self.ops.clear();
    }

    /// Draws what it holds into `into`.
    pub fn replay(&self, into: &mut dyn GraphicsContext) {
        for op in &self.ops {
            match op {
                Op::FillRoundRect(r, radii, c) => into.fill_round_rect(*r, *radii, *c),
                Op::StrokeRoundRect(r, radii, w, c) => into.stroke_round_rect(*r, *radii, *w, *c),
                Op::Line(a, b, w, c) => into.draw_line(*a, *b, *w, *c),
                Op::Text(t, c) => into.draw_text(t, *c),
                Op::TextBrush(t, b, area) => into.draw_text_brush(t, b, *area),
                Op::TextShadow(t, c, blur) => into.draw_text_shadow(t, *c, *blur),
                Op::Arc(oval, start, sweep, w, c, round) => into.draw_arc(*oval, *start, *sweep, *w, *c, *round),
                Op::Image(image, src, dst, alpha, q) => into.draw_image(image, *src, *dst, *alpha, *q),
                Op::ImageMask(image, src, dst, brush, area, alpha, q) => {
                    into.draw_image_mask(image, *src, *dst, brush, *area, *alpha, *q)
                }
                Op::PushLayer(b, radii, alpha, t) => into.push_layer(*b, *radii, *alpha, *t),
                Op::PopLayer => into.pop_layer(),
                Op::PushClip(r) => into.push_clip(*r),
                Op::PushClipRoundRect(r, radii, clip_op) => into.push_clip_round_rect(*r, *radii, *clip_op),
                Op::PushClipPath(p, origin, clip_op) => into.push_clip_path(p, *origin, *clip_op),
                Op::PopClip => into.pop_clip(),
                Op::PushTranslation(by) => into.push_translation(*by),
                Op::PopTranslation => into.pop_translation(),
                Op::PushTransform(m) => into.push_transform(m),
                Op::PopTransform => into.pop_transform(),
                Op::PushEffectLayer(b, effect, alpha) => into.push_effect_layer(*b, effect, *alpha),
                Op::PushStretchLayer(b, stretch, effect, alpha) => {
                    into.push_stretch_layer(*b, stretch, effect.as_ref(), *alpha)
                }
                Op::Backdrop(b, effect, alpha) => into.draw_backdrop(*b, effect, *alpha),
                Op::PushBlendMode(mode) => into.push_blend_mode(*mode),
                Op::PopBlendMode => into.pop_blend_mode(),
                Op::PushAntiAlias(on) => into.push_anti_alias(*on),
                Op::PopAntiAlias => into.pop_anti_alias(),
                Op::PushColorFilter(f) => into.push_color_filter(f),
                Op::PopColorFilter => into.pop_color_filter(),
                Op::FillGradient(r, radii, brush, area) => into.fill_gradient(*r, *radii, brush, *area),
                Op::Vertices(v, origin, mode, brush, area, alpha) => {
                    into.draw_vertices(v, *origin, *mode, brush, *area, *alpha)
                }
                Op::Path(p, origin, brush, area, stroke) => into.draw_path(p, *origin, brush, *area, stroke),
                Op::Shadow(r, radii, s, inner) => into.draw_shadow(*r, *radii, s, *inner),
                Op::ShadowPath(p, origin, s, inner) => into.draw_shadow_path(p, *origin, s, *inner),
                Op::ElevationShadow(o, origin, s) => into.draw_elevation_shadow(o, *origin, s),
                Op::OutlineLayer(b, p, alpha, t) => into.push_outline_layer(*b, p, *alpha, *t),
                Op::ShapeShadow(r, outline, ld, density, s, inner) => {
                    into.draw_shape_shadow(*r, &RecordedOutline(outline.clone()), *ld, density, s, *inner)
                }
                Op::BeginNode(id) => into.begin_node(*id),
                Op::EndNode => into.end_node(),
            }
        }
    }
}

/// A shape's outline as it was where a shape's shadow was recorded: the same outline again over the same rect.
#[derive(Clone, PartialEq, Debug)]
struct RecordedOutline(Outline);

impl Shape for RecordedOutline {
    fn create_outline(&self, _: Size, _: LayoutDirection, _: &Density) -> Outline {
        self.0.clone()
    }
}

impl GraphicsContext for Picture {
    fn fill_round_rect(&mut self, rect: Rect, radii: CornerRadii, color: Color) {
        self.ops.push(Op::FillRoundRect(rect, radii, color));
    }
    fn stroke_round_rect(&mut self, rect: Rect, radii: CornerRadii, width: f32, color: Color) {
        self.ops.push(Op::StrokeRoundRect(rect, radii, width, color));
    }
    fn draw_line(&mut self, from: Offset, to: Offset, width: f32, color: Color) {
        self.ops.push(Op::Line(from, to, width, color));
    }
    fn draw_text(&mut self, text: &TextDraw, color: Color) {
        self.ops.push(Op::Text(text.clone(), color));
    }
    fn draw_text_brush(&mut self, text: &TextDraw, brush: &Brush, brush_area: Rect) {
        self.ops.push(Op::TextBrush(text.clone(), brush.clone(), brush_area));
    }
    fn draw_text_shadow(&mut self, text: &TextDraw, color: Color, blur_radius: f32) {
        self.ops.push(Op::TextShadow(text.clone(), color, blur_radius));
    }
    fn draw_arc(&mut self, oval: Rect, start: f32, sweep: f32, stroke_width: f32, color: Color, round_cap: bool) {
        self.ops.push(Op::Arc(oval, start, sweep, stroke_width, color, round_cap));
    }
    fn draw_image(&mut self, image: &ImageBitmap, src: Rect, dst: Rect, alpha: f32, quality: FilterQuality) {
        self.ops.push(Op::Image(image.clone(), src, dst, alpha, quality));
    }
    fn draw_image_mask(
        &mut self,
        image: &ImageBitmap,
        src: Rect,
        dst: Rect,
        brush: &Brush,
        brush_area: Rect,
        alpha: f32,
        quality: FilterQuality,
    ) {
        self.ops.push(Op::ImageMask(image.clone(), src, dst, brush.clone(), brush_area, alpha, quality));
    }
    fn push_layer(&mut self, bounds: Rect, radii: CornerRadii, alpha: f32, transform: LayerTransform) {
        self.ops.push(Op::PushLayer(bounds, radii, alpha, transform));
    }
    fn pop_layer(&mut self) {
        self.ops.push(Op::PopLayer);
    }
    fn push_clip(&mut self, rect: Rect) {
        self.ops.push(Op::PushClip(rect));
    }
    fn push_clip_round_rect(&mut self, rect: Rect, radii: CornerRadii, op: ClipOp) {
        self.ops.push(Op::PushClipRoundRect(rect, radii, op));
    }
    fn push_clip_path(&mut self, path: &Path, origin: Offset, op: ClipOp) {
        self.ops.push(Op::PushClipPath(path.clone(), origin, op));
    }
    fn pop_clip(&mut self) {
        self.ops.push(Op::PopClip);
    }
    fn push_translation(&mut self, by: Offset) {
        self.ops.push(Op::PushTranslation(by));
    }
    fn pop_translation(&mut self) {
        self.ops.push(Op::PopTranslation);
    }
    fn push_transform(&mut self, matrix: &Matrix) {
        self.ops.push(Op::PushTransform(*matrix));
    }
    fn pop_transform(&mut self) {
        self.ops.push(Op::PopTransform);
    }
    fn push_effect_layer(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        self.ops.push(Op::PushEffectLayer(bounds, effect.clone(), alpha));
    }
    fn push_stretch_layer(&mut self, bounds: Rect, stretch: &StretchEffect, effect: Option<&RenderEffect>, alpha: f32) {
        self.ops.push(Op::PushStretchLayer(bounds, *stretch, effect.cloned(), alpha));
    }
    fn draw_backdrop(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        self.ops.push(Op::Backdrop(bounds, effect.clone(), alpha));
    }
    fn push_blend_mode(&mut self, mode: BlendMode) {
        self.ops.push(Op::PushBlendMode(mode));
    }
    fn pop_blend_mode(&mut self) {
        self.ops.push(Op::PopBlendMode);
    }
    fn push_anti_alias(&mut self, on: bool) {
        self.ops.push(Op::PushAntiAlias(on));
    }
    fn pop_anti_alias(&mut self) {
        self.ops.push(Op::PopAntiAlias);
    }
    fn push_color_filter(&mut self, filter: &ColorFilter) {
        self.ops.push(Op::PushColorFilter(*filter));
    }
    fn pop_color_filter(&mut self) {
        self.ops.push(Op::PopColorFilter);
    }
    fn fill_gradient(&mut self, rect: Rect, radii: CornerRadii, brush: &Brush, brush_area: Rect) {
        self.ops.push(Op::FillGradient(rect, radii, brush.clone(), brush_area));
    }
    fn draw_vertices(
        &mut self,
        vertices: &Vertices,
        origin: Offset,
        blend_mode: BlendMode,
        brush: &Brush,
        brush_area: Rect,
        alpha: f32,
    ) {
        self.ops.push(Op::Vertices(vertices.clone(), origin, blend_mode, brush.clone(), brush_area, alpha));
    }
    fn draw_path(&mut self, path: &Path, origin: Offset, brush: &Brush, brush_area: Rect, stroke: &PathStroke) {
        self.ops.push(Op::Path(path.clone(), origin, brush.clone(), brush_area, stroke.clone()));
    }
    fn draw_shadow(&mut self, rect: Rect, radii: CornerRadii, shadow: &Shadow, inner: bool) {
        self.ops.push(Op::Shadow(rect, radii, shadow.clone(), inner));
    }
    fn draw_shadow_path(&mut self, outline: &Path, origin: Offset, shadow: &Shadow, inner: bool) {
        self.ops.push(Op::ShadowPath(outline.clone(), origin, shadow.clone(), inner));
    }
    fn light(&self) -> ShadowLight {
        ShadowLight::default()
    }
    fn draw_elevation_shadow(&mut self, outline: &Outline, origin: Offset, shadow: &ElevationShadow) {
        self.ops.push(Op::ElevationShadow(outline.clone(), origin, *shadow));
    }
    fn push_outline_layer(&mut self, bounds: Rect, outline: &Path, alpha: f32, transform: LayerTransform) {
        self.ops.push(Op::OutlineLayer(bounds, outline.clone(), alpha, transform));
    }
    fn draw_shape_shadow(
        &mut self,
        rect: Rect,
        shape: &dyn Shape,
        layout_direction: LayoutDirection,
        density: &Density,
        shadow: &Shadow,
        inner: bool,
    ) {
        let outline = shape.create_outline(rect.size(), layout_direction, density);
        self.ops.push(Op::ShapeShadow(rect, outline, layout_direction, *density, shadow.clone(), inner));
    }
    fn begin_node(&mut self, node: u64) {
        self.ops.push(Op::BeginNode(node));
    }
    fn end_node(&mut self) {
        self.ops.push(Op::EndNode);
    }
}

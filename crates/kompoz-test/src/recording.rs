//! A canvas that records what reaches it (C++'s testing::RecordingCanvas), so tests can check what was drawn and
//! where: translations are applied to where things land; layer transforms and `push_transform`'s matrices aren't
//! (they are recorded).

use kompoz_graphics::{
    BlendMode, Brush, Color, ColorFilter, Density, FilterQuality, ImageBitmap, LayoutDirection, Matrix, Offset,
    Outline, Path, PathStroke, Rect, RenderEffect, Shape, StretchEffect, Vertices, path_effected,
};
use kompoz_ui::draw::vector_drawn_into;
use kompoz_ui::graphics_context::{
    ClipOp, CornerRadii, ElevationShadow, GraphicsContext, LayerContent, LayerTransform, Shadow, ShadowLight, TextDraw,
    TextRunStyle, default_draw_shape_shadow,
};
use std::rc::Rc;

/// A `draw_shadow` call (drop_shadow, inner_shadow): where, its corners, the shadow, and the blend mode it was drawn
/// with.
#[derive(Clone, Debug)]
pub struct RecordedShapeShadow {
    pub rect: Rect,
    pub radii: CornerRadii,
    pub shadow: Shadow,
    pub inner: bool,
    pub blend: BlendMode,
}

/// A `draw_shadow_path` call (an outline shape's): the outline's bounds where it is.
#[derive(Clone, Debug)]
pub struct RecordedPathShadow {
    pub bounds: Rect,
    pub shadow: Shadow,
    pub inner: bool,
}

/// A `draw_elevation_shadow` call: the outline's bounds where it is, the outline, the shadow.
#[derive(Clone, Debug)]
pub struct RecordedElevationShadow {
    pub bounds: Rect,
    pub outline: Outline,
    pub shadow: ElevationShadow,
}

impl RecordedElevationShadow {
    /// How high (a flat one's).
    pub fn elevation(&self) -> f32 {
        self.shadow.plane[2]
    }
}

/// A `draw_backdrop` call.
#[derive(Clone, Debug)]
pub struct RecordedBackdrop {
    pub bounds: Rect,
    pub effect: RenderEffect,
    pub alpha: f32,
}

/// A `draw_arc` call.
#[derive(Clone, Copy, Debug)]
pub struct RecordedArc {
    pub oval: Rect,
    pub start: f32,
    pub sweep: f32,
    pub stroke_width: f32,
    pub color: Color,
}

/// A `push_clip_round_rect` or `push_clip_path` call: its bounds (where it is), corners, path (around the origin it
/// was clipped at) and op.
#[derive(Clone, Debug)]
pub struct RecordedClipShape {
    pub bounds: Rect,
    pub radii: CornerRadii,
    pub path: Option<Path>,
    pub op: ClipOp,
}

/// A `draw_vertices` call, with the blend mode and the filters in effect.
#[derive(Clone, Debug)]
pub struct RecordedMesh {
    pub vertices: Vertices,
    pub origin: Offset,
    pub blend_mode: BlendMode,
    pub brush: Brush,
    pub brush_area: Rect,
    pub alpha: f32,
    pub paint_blend: BlendMode,
    pub filters: Vec<ColorFilter>,
}

/// Records what is drawn (C++'s RecordingCanvas): each kind of call in lists tests read, `order` the fills as they
/// came ('r' a rect, 'p' a path, 's' a shadow, 'e' an elevation shadow, 'b' a backdrop, 'v' a mesh), `stack` what
/// was pushed and popped (a layer L l, a clip C c, a transform T t, a translation M m, a color filter F f, a blend
/// mode B b, anti-aliasing A a). Colors are recorded as the filters in effect make them.
#[derive(Default)]
pub struct RecordingCanvas {
    pub texts: Vec<String>,
    /// Each drawn text itself.
    pub text_draws: Vec<TextDraw>,
    /// The style each text was drawn in (C++'s `fonts`): its family, weight, style, size, letter spacing, line
    /// height, features, draw style, geometric transform.
    pub text_styles: Vec<Rc<TextRunStyle>>,
    pub colors: Vec<Color>,
    /// Where each text starts.
    pub baselines: Vec<Offset>,
    /// Whether its paragraph goes right to left.
    pub rtl: Vec<bool>,
    /// The ids of the vectors drawn (the bitmaps painters drew them into, drawn).
    pub vectors: Vec<u64>,
    pub images: Vec<u64>,
    pub drawn_images: Vec<ImageBitmap>,
    pub image_sources: Vec<Rect>,
    pub image_destinations: Vec<Rect>,
    pub layers: i32,
    pub depth: i32,
    pub max_depth: i32,
    pub transformed_layers: i32,
    pub gradients: i32,
    pub layer_bounds: Vec<Rect>,
    pub layer_alphas: Vec<f32>,
    pub layer_transforms: Vec<LayerTransform>,
    /// Bounds of drawn paths (where they land).
    pub paths: Vec<Rect>,
    /// Filled (rounded) rects.
    pub rects: Vec<Rect>,
    pub rect_colors: Vec<Color>,
    /// Each filled rect's top left corner radius, and all four.
    pub rect_radii: Vec<f32>,
    pub rect_corners: Vec<CornerRadii>,
    pub stroke_corners: Vec<CornerRadii>,
    pub layer_corners: Vec<CornerRadii>,
    /// `push_clip` calls.
    pub clips: Vec<Rect>,
    /// `key_layer`'s: each layer's key, whether it is the same.
    pub layer_keys: Vec<(u64, bool)>,
    /// What `layer_content` says.
    pub layer_answer: LayerContent,
    /// Stroked (rounded) rects: borders, focus rings.
    pub strokes: Vec<Rect>,
    pub stroke_colors: Vec<Color>,
    pub order: String,
    pub stack: String,
    /// Each `push_translation`'s.
    pub translations: Vec<Offset>,
    /// The translation in effect now (their sum).
    pub translation: Offset,
    /// Each `push_transform`'s.
    pub transforms: Vec<Matrix>,
    /// `push_transform`s in effect now.
    pub transform_depth: i32,
    /// `push_color_filter`s in effect, the innermost last.
    pub filter_stack: Vec<ColorFilter>,
    /// Each drawn image's filters (as `filter_stack` was).
    pub image_filters: Vec<Vec<ColorFilter>>,
    /// `push_blend_mode`s in effect.
    pub blend_stack: Vec<BlendMode>,
    /// Each filled rect's (and gradient's) blend mode, and how many layers it is drawn in.
    pub rect_blends: Vec<BlendMode>,
    pub rect_depths: Vec<i32>,
    pub path_blends: Vec<BlendMode>,
    pub anti_alias_stack: Vec<bool>,
    /// Whether each path was drawn anti-aliased.
    pub path_anti_aliased: Vec<bool>,
    /// Drawn lines, where they land.
    pub lines: Vec<(Offset, Offset)>,
    pub line_colors: Vec<Color>,
    /// Texts and their shadows as they came: 't' a text, 's' a shadow.
    pub text_order: String,
    pub shadow_texts: Vec<String>,
    pub shadow_baselines: Vec<Offset>,
    pub shadow_colors: Vec<Color>,
    pub shadow_blurs: Vec<f32>,
    pub shadows: Vec<RecordedShapeShadow>,
    pub path_shadows: Vec<RecordedPathShadow>,
    /// `draw_shape_shadow` calls with a brush (not one color): where the shape is.
    pub brushed_shadows: Vec<Rect>,
    /// Each text painted with a brush (in `texts` too, in the color at the area's center).
    pub brush_areas: Vec<Rect>,
    /// Each vector drawn: its color (a one-color one's mask paint as the filters make it; one in its own colors,
    /// white as they make it), faded by its alpha, and where it went.
    pub vector_colors: Vec<Color>,
    pub vector_bounds: Vec<Rect>,
    pub arcs: Vec<RecordedArc>,
    pub image_qualities: Vec<FilterQuality>,
    pub image_alphas: Vec<f32>,
    pub image_blends: Vec<BlendMode>,
    /// Each mask drawn (an Alpha8 image: in `images` too): its paint as the filters make it, and its alpha.
    pub mask_colors: Vec<Color>,
    pub mask_alphas: Vec<f32>,
    /// Each outline layer's outline, from its top left.
    pub layer_outlines: Vec<Path>,
    pub elevation_shadows: Vec<RecordedElevationShadow>,
    pub backdrops: Vec<RecordedBackdrop>,
    /// Each layer's effect (`push_effect_layer`, `push_stretch_layer`).
    pub layer_effects: Vec<Option<RenderEffect>>,
    /// Each layer's stretch (`push_stretch_layer`).
    pub layer_stretches: Vec<Option<StretchEffect>>,
    pub clip_shapes: Vec<RecordedClipShape>,
    /// Set: what lies outside isn't drawn (as a real screen culls it).
    pub visible: Option<Rect>,
    /// Each drawn path's color (a gradient's at the path's center), stroke width (0: filled, unless a hairline),
    /// all of its stroke, and the path itself (around the origin it was drawn at).
    pub path_colors: Vec<Color>,
    pub path_strokes: Vec<f32>,
    pub path_stroke_styles: Vec<PathStroke>,
    pub path_shapes: Vec<Path>,
    pub meshes: Vec<RecordedMesh>,
    /// The light elevation shadows have.
    pub light: ShadowLight,
    saved_translations: Vec<Offset>,
}

impl RecordingCanvas {
    pub fn new() -> RecordingCanvas {
        RecordingCanvas::default()
    }

    /// A color as the filters in effect make it (the innermost first): what a backend would show.
    pub fn filtered(&self, mut c: Color) -> Color {
        for f in self.filter_stack.iter().rev() {
            c = f.apply(c);
        }
        c
    }

    /// The blend mode in effect.
    pub fn blend(&self) -> BlendMode {
        self.blend_stack.last().copied().unwrap_or(BlendMode::SrcOver)
    }

    /// Whether drawing is anti-aliased now.
    pub fn anti_aliased(&self) -> bool {
        self.anti_alias_stack.last().copied().unwrap_or(true)
    }

    fn moved(&self, r: Rect) -> Rect {
        r.translate(self.translation)
    }

    fn vector_drawn(&mut self, image: &ImageBitmap, c: Color, alpha: f32, dst: Rect) {
        if let Some(v) = vector_drawn_into(image) {
            self.vectors.push(v);
            self.vector_colors.push(Color::new_in(c.red(), c.green(), c.blue(), c.alpha() * alpha, c.color_space()));
            self.vector_bounds.push(self.moved(dst));
        }
    }

    fn image(&mut self, image: &ImageBitmap, src: Rect, dst: Rect, alpha: f32, quality: FilterQuality) {
        self.images.push(image.id());
        self.drawn_images.push(image.clone());
        self.image_alphas.push(alpha);
        self.image_blends.push(self.blend());
        self.image_qualities.push(quality);
        self.image_filters.push(self.filter_stack.clone());
        self.image_sources.push(src);
        self.image_destinations.push(self.moved(dst));
    }

    fn rect(&mut self, r: Rect, radii: CornerRadii, color: Color) {
        self.order.push('r');
        self.rect_depths.push(self.depth);
        self.rect_blends.push(self.blend());
        self.rects.push(self.moved(r));
        self.rect_colors.push(self.filtered(color));
        self.rect_radii.push(radii.top_left);
        self.rect_corners.push(radii);
    }
}

impl GraphicsContext for RecordingCanvas {
    fn fill_round_rect(&mut self, rect: Rect, radii: CornerRadii, color: Color) {
        self.rect(rect, radii, color);
    }
    fn stroke_round_rect(&mut self, rect: Rect, radii: CornerRadii, _: f32, color: Color) {
        self.strokes.push(self.moved(rect));
        self.stroke_colors.push(self.filtered(color));
        self.stroke_corners.push(radii);
    }
    fn draw_line(&mut self, from: Offset, to: Offset, _: f32, color: Color) {
        self.lines.push((from + self.translation, to + self.translation));
        self.line_colors.push(color);
    }
    fn draw_text(&mut self, text: &TextDraw, color: Color) {
        self.text_order.push('t');
        self.texts.push(text.text.to_string());
        self.text_draws.push(text.clone());
        self.text_styles.push(text.style.clone());
        self.colors.push(self.filtered(color));
        self.baselines.push(text.baseline + self.translation);
        self.rtl.push(text.rtl);
    }
    fn draw_text_brush(&mut self, text: &TextDraw, brush: &Brush, brush_area: Rect) {
        self.brush_areas.push(self.moved(brush_area));
        self.draw_text(text, brush.color_at(brush_area.center(), brush_area));
    }
    fn draw_text_shadow(&mut self, text: &TextDraw, color: Color, blur_radius: f32) {
        self.text_order.push('s');
        self.shadow_texts.push(text.text.to_string());
        self.shadow_baselines.push(text.baseline + self.translation);
        self.shadow_colors.push(color);
        self.shadow_blurs.push(blur_radius);
    }
    fn draw_arc(&mut self, oval: Rect, start: f32, sweep: f32, stroke_width: f32, color: Color, _: bool) {
        self.arcs.push(RecordedArc { oval: self.moved(oval), start, sweep, stroke_width, color });
    }
    fn draw_image(&mut self, image: &ImageBitmap, src: Rect, dst: Rect, alpha: f32, quality: FilterQuality) {
        let white = self.filtered(Color::White);
        self.vector_drawn(image, white, alpha, dst);
        self.image(image, src, dst, alpha, quality);
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
        let c = self.filtered(brush.color_at(brush_area.center(), brush_area));
        self.vector_drawn(image, c, alpha, dst);
        self.mask_colors.push(c);
        self.mask_alphas.push(alpha);
        self.image(image, src, dst, alpha, quality);
    }
    fn push_layer(&mut self, bounds: Rect, radii: CornerRadii, alpha: f32, transform: LayerTransform) {
        self.layer_effects.push(None);
        self.layer_stretches.push(None);
        self.layer_bounds.push(self.moved(bounds));
        self.layer_corners.push(radii);
        self.layer_alphas.push(alpha);
        self.layer_transforms.push(transform);
        if !transform.is_identity() {
            self.transformed_layers += 1;
        }
        self.stack.push('L');
        self.layers += 1;
        self.depth += 1;
        self.max_depth = self.max_depth.max(self.depth);
    }
    fn pop_layer(&mut self) {
        self.stack.push('l');
        self.depth -= 1;
    }
    fn push_clip(&mut self, rect: Rect) {
        self.stack.push('C');
        self.clips.push(self.moved(rect));
    }
    fn push_clip_round_rect(&mut self, rect: Rect, radii: CornerRadii, op: ClipOp) {
        self.stack.push('C');
        self.clip_shapes.push(RecordedClipShape { bounds: self.moved(rect), radii, path: None, op });
    }
    fn push_clip_path(&mut self, path: &Path, origin: Offset, op: ClipOp) {
        self.stack.push('C');
        let b = path.get_bounds().translate(origin);
        self.clip_shapes.push(RecordedClipShape {
            bounds: self.moved(b),
            radii: CornerRadii::Zero,
            path: Some(path.clone()),
            op,
        });
    }
    fn pop_clip(&mut self) {
        self.stack.push('c');
    }
    fn push_translation(&mut self, by: Offset) {
        self.stack.push('M');
        self.translations.push(by);
        self.saved_translations.push(self.translation);
        self.translation = self.translation + by;
    }
    fn pop_translation(&mut self) {
        self.stack.push('m');
        if let Some(t) = self.saved_translations.pop() {
            self.translation = t;
        }
    }
    fn push_transform(&mut self, matrix: &Matrix) {
        self.stack.push('T');
        self.transforms.push(*matrix);
        self.transform_depth += 1;
    }
    fn pop_transform(&mut self) {
        self.stack.push('t');
        self.transform_depth -= 1;
    }
    fn push_effect_layer(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        self.push_layer(bounds, CornerRadii::Zero, alpha, LayerTransform::Identity);
        if let Some(last) = self.layer_effects.last_mut() {
            *last = Some(effect.clone());
        }
    }
    fn push_stretch_layer(&mut self, bounds: Rect, stretch: &StretchEffect, effect: Option<&RenderEffect>, alpha: f32) {
        self.push_layer(bounds, CornerRadii::Zero, alpha, LayerTransform::Identity);
        if let Some(last) = self.layer_effects.last_mut() {
            *last = effect.cloned();
        }
        if let Some(last) = self.layer_stretches.last_mut() {
            *last = Some(*stretch);
        }
    }
    fn draw_backdrop(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        self.order.push('b');
        self.backdrops.push(RecordedBackdrop { bounds: self.moved(bounds), effect: effect.clone(), alpha });
    }
    fn push_blend_mode(&mut self, mode: BlendMode) {
        self.stack.push('B');
        self.blend_stack.push(mode);
    }
    fn pop_blend_mode(&mut self) {
        self.stack.push('b');
        self.blend_stack.pop();
    }
    fn push_anti_alias(&mut self, on: bool) {
        self.stack.push('A');
        self.anti_alias_stack.push(on);
    }
    fn pop_anti_alias(&mut self) {
        self.stack.push('a');
        self.anti_alias_stack.pop();
    }
    fn push_color_filter(&mut self, filter: &ColorFilter) {
        self.stack.push('F');
        self.filter_stack.push(*filter);
    }
    fn pop_color_filter(&mut self) {
        self.stack.push('f');
        self.filter_stack.pop();
    }
    fn quick_reject(&self, rect: Rect) -> bool {
        let r = self.moved(rect);
        self.visible.is_some_and(|v| r.right <= v.left || r.bottom <= v.top || r.left >= v.right || r.top >= v.bottom)
    }
    fn fill_gradient(&mut self, rect: Rect, radii: CornerRadii, brush: &Brush, brush_area: Rect) {
        self.gradients += 1;
        self.rect(rect, radii, brush.color_at(rect.center(), brush_area)); // (a gradient's at the rect's center)
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
        self.order.push('v');
        self.meshes.push(RecordedMesh {
            vertices: vertices.clone(),
            origin: origin + self.translation,
            blend_mode,
            brush: brush.clone(),
            brush_area,
            alpha,
            paint_blend: self.blend(),
            filters: self.filter_stack.clone(),
        });
    }
    /// A path effect is applied as a backend applies it (here a unit a pixel): the path and stroke recorded are what
    /// it made.
    fn draw_path(&mut self, source: &Path, origin: Offset, brush: &Brush, brush_area: Rect, given: &PathStroke) {
        let mut stroke = given.clone();
        let had_effect = stroke.path_effect.is_some();
        let path = path_effected(source, &mut stroke, 1.0).into_owned();
        if had_effect && path.is_empty() {
            return; // (nothing left to draw)
        }
        self.order.push('p');
        self.path_blends.push(self.blend());
        self.path_anti_aliased.push(self.anti_aliased());
        let b = path.get_bounds().translate(origin);
        self.path_colors.push(self.filtered(brush.color_at(b.center(), brush_area)));
        self.paths.push(self.moved(b));
        self.path_strokes.push(stroke.width);
        self.path_stroke_styles.push(stroke);
        self.path_shapes.push(path);
    }
    fn draw_shadow(&mut self, rect: Rect, radii: CornerRadii, shadow: &Shadow, inner: bool) {
        self.order.push('s');
        self.shadows.push(RecordedShapeShadow {
            rect: self.moved(rect),
            radii,
            shadow: shadow.clone(),
            inner,
            blend: self.blend(),
        });
    }
    fn draw_shadow_path(&mut self, outline: &Path, origin: Offset, shadow: &Shadow, inner: bool) {
        let b = outline.get_bounds().translate(origin);
        self.path_shadows.push(RecordedPathShadow { bounds: self.moved(b), shadow: shadow.clone(), inner });
    }
    fn light(&self) -> ShadowLight {
        self.light
    }
    fn draw_elevation_shadow(&mut self, outline: &Outline, origin: Offset, shadow: &ElevationShadow) {
        self.order.push('e');
        let b = outline.bounds().translate(origin);
        self.elevation_shadows.push(RecordedElevationShadow {
            bounds: self.moved(b),
            outline: outline.clone(),
            shadow: *shadow,
        });
    }
    fn push_outline_layer(&mut self, bounds: Rect, outline: &Path, alpha: f32, transform: LayerTransform) {
        self.layer_outlines.push(outline.clone());
        self.push_layer(bounds, CornerRadii::Zero, alpha, transform);
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
        if shadow.brush.as_ref().is_some_and(|b| !b.is_solid()) {
            self.brushed_shadows.push(self.moved(rect));
        }
        default_draw_shape_shadow(self, rect, shape, layout_direction, density, shadow, inner);
    }
    fn key_layer(&mut self, key: u64, same: bool) {
        self.layer_keys.push((key, same));
    }
    fn layer_content(&self) -> LayerContent {
        self.layer_answer
    }
}

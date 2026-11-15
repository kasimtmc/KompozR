//! Compose's `androidx.compose.ui.graphics.Canvas` and `Paint` (C++'s `graphics::Canvas`): a transform and a clip on
//! a stack (save, restore, saveLayer) over a [`GraphicsContext`], draws in a [`Paint`]. A DrawScope has one
//! (`draw_into_canvas`, `canvas()`); [`Canvas::new`] makes one over any context, [`Canvas::from_image`] one drawing
//! into a bitmap.

use super::bitmap::draw_into_bitmap;
use super::{DrawScope, PointMode, Stroke};
use crate::graphics_context::{ClipOp, CornerRadii, GraphicsContext, LayerTransform};
use kompoz_graphics::{
    BlendMode, Brush, Color, ColorFilter, CornerRadius, DefaultFilterQuality, Density, FilterQuality, ImageBitmap,
    ImageBitmapConfig, IntOffset, IntSize, LayoutDirection, Matrix, Offset, Path, PathEffect, PathStroke, Rect, Shader,
    ShaderBrush, Size, SolidColor, StrokeCap, StrokeJoin, Vertices,
};
use kompoz_runtime::named;
use std::rc::Rc;

/// How a paint paints a shape (Compose's PaintingStyle): filled, or outlined.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PaintingStyle {
    #[default]
    Fill,
    Stroke,
}

/// What a [`Canvas`] draws with (Compose's Paint): a color (its alpha the paint's: [`alpha`](Paint::alpha),
/// [`set_alpha`](Paint::set_alpha)), or a shader (in the canvas's coordinates) faded by that alpha; filled or
/// stroked (a stroke width of 0: a hairline) with its cap, join and miter limit; its path through `path_effect`
/// first (a fill's too, but for dashes: Skia dashes only strokes); its colors through `color_filter`; onto what is
/// there by `blend_mode`; images sampled as `filter_quality` says; not anti-aliased where `is_anti_alias` is false.
/// Equal by value.
#[derive(Clone, PartialEq, Debug)]
pub struct Paint {
    pub color: Color,
    pub is_anti_alias: bool,
    pub blend_mode: BlendMode,
    pub style: PaintingStyle,
    pub stroke_width: f32,
    pub stroke_cap: StrokeCap,
    pub stroke_join: StrokeJoin,
    pub stroke_miter_limit: f32,
    pub filter_quality: FilterQuality,
    pub shader: Option<Shader>,
    pub color_filter: Option<ColorFilter>,
    pub path_effect: Option<PathEffect>,
}

/// Compose's: black, anti-aliased, SrcOver, filled, a hairline's width, butt, mitered at 4, sampled Low.
impl Default for Paint {
    fn default() -> Paint {
        Paint {
            color: Color::Black,
            is_anti_alias: true,
            blend_mode: BlendMode::SrcOver,
            style: PaintingStyle::Fill,
            stroke_width: 0.0,
            stroke_cap: StrokeCap::Butt,
            stroke_join: StrokeJoin::Miter,
            stroke_miter_limit: Stroke::DefaultMiter,
            filter_quality: DefaultFilterQuality,
            shader: None,
            color_filter: None,
            path_effect: None,
        }
    }
}

/// Compose's `Paint()`.
pub fn Paint() -> Paint {
    Paint::default()
}

impl Paint {
    /// Its color's alpha.
    pub fn alpha(&self) -> f32 {
        self.color.alpha()
    }

    /// Its color's alpha, kept to 0..1.
    pub fn set_alpha(&mut self, alpha: f32) {
        let c = self.color;
        self.color = Color::new_in(c.red(), c.green(), c.blue(), alpha.clamp(0.0, 1.0), c.color_space());
    }

    /// What it paints with: its shader (in the canvas's coordinates) or its color, opaque (its alpha goes apart).
    fn brush(&self) -> Brush {
        match &self.shader {
            Some(shader) => ShaderBrush(shader.clone()),
            None => {
                let c = self.color;
                SolidColor(Color::new_in(c.red(), c.green(), c.blue(), 1.0, c.color_space()))
            }
        }
    }

    /// How it draws a shape: its stroke, or a fill through its path effect.
    fn path_stroke(&self) -> PathStroke {
        match self.style {
            PaintingStyle::Stroke => Stroke::new(
                self.stroke_width,
                self.stroke_miter_limit,
                self.stroke_cap,
                self.stroke_join,
                self.path_effect.clone(),
            )
            .path_stroke(),
            PaintingStyle::Fill => PathStroke { path_effect: self.path_effect.clone(), ..PathStroke::default() },
        }
    }
}

/// What a canvas pushed onto its context and pops when it is restored.
#[derive(Clone, Copy, Debug)]
enum Saved {
    Save,
    /// A layer, and whether a color filter and a blend mode were pushed with it.
    Layer {
        filtered: bool,
        blended: bool,
    },
    Transform,
    Clip,
}

/// A canvas's stack: its origin in its context, its density (how big a hairline's dot is), what it pushed.
#[derive(Debug)]
pub struct CanvasState {
    origin: Offset,
    density: Density,
    stack: Vec<Saved>,
}

impl CanvasState {
    pub(crate) fn new(origin: Offset, density: Density) -> CanvasState {
        CanvasState { origin, density, stack: Vec::new() }
    }

    /// Its origin in its context.
    pub fn origin(&self) -> Offset {
        self.origin
    }

    pub(crate) fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Pops what was pushed past `depth`.
    pub(crate) fn restore_to(&mut self, context: &mut dyn GraphicsContext, depth: usize) {
        while self.stack.len() > depth {
            match self.stack.pop().expect("a saved entry") {
                Saved::Save => {}
                Saved::Transform => context.pop_transform(),
                Saved::Clip => context.pop_clip(),
                Saved::Layer { filtered, blended } => {
                    context.pop_layer();
                    if blended {
                        context.pop_blend_mode();
                    }
                    if filtered {
                        context.pop_color_filter();
                    }
                }
            }
        }
    }
}

/// Where a canvas keeps its stack: its own, or a draw scope's (what it pushes stays there until the scope's block
/// ends).
pub(crate) enum CanvasSlot<'a> {
    Own(Option<CanvasState>),
    Of(&'a mut CanvasState),
}

impl CanvasSlot<'_> {
    pub(crate) fn get(&mut self) -> Option<&mut CanvasState> {
        match self {
            CanvasSlot::Own(s) => s.as_mut(),
            CanvasSlot::Of(s) => Some(&mut **s),
        }
    }
}

/// What a canvas draws into: a context, or a recording handed to a bitmap's renderer as it goes.
enum Target<'a> {
    Context(&'a mut dyn GraphicsContext),
    Bitmap { picture: super::Picture, image: ImageBitmap },
}

/// Drawing as Compose's Canvas does (Android's): a transform and a clip on a stack (save, restore, saveLayer), draws
/// in a [`Paint`]. Its coordinates are px from its origin in its context; what it draws goes through the context's
/// transforms and clips as well as its own. What is still saved when it goes is restored (a draw scope's canvas
/// keeps it until the scope's block ends). Compose's draws taking left, top, right and bottom are the `_ltrb` forms.
pub struct Canvas<'a> {
    target: Target<'a>,
    state: StateRef<'a>,
}

enum StateRef<'a> {
    Own(CanvasState),
    Of(&'a mut CanvasState),
}

impl<'a> Canvas<'a> {
    /// A canvas over `context`, its origin there `origin`.
    pub fn new(context: &'a mut dyn GraphicsContext, origin: Offset) -> Canvas<'a> {
        Canvas { target: Target::Context(context), state: StateRef::Own(CanvasState::new(origin, Density::default())) }
    }

    /// A canvas over a context keeping its stack in `state` (a draw scope's).
    pub(crate) fn of(context: &'a mut dyn GraphicsContext, state: &'a mut CanvasState) -> Canvas<'a> {
        Canvas { target: Target::Context(context), state: StateRef::Of(state) }
    }

    /// Drawing into `image` (Compose's Canvas(ImageBitmap)), a unit a pixel: what it draws is drawn into it by the
    /// platform's renderer as the canvas goes (kept for one to come while there is none: see
    /// [`set_bitmap_renderer`](super::set_bitmap_renderer)). An Alpha8 bitmap keeps the alpha, an Rgb565 one or one
    /// without alpha what is drawn over its black.
    pub fn from_image(image: &ImageBitmap) -> Canvas<'static> {
        Canvas {
            target: Target::Bitmap { picture: super::Picture::new(), image: image.clone() },
            state: StateRef::Own(CanvasState::new(Offset::Zero, Density::default())),
        }
    }

    /// The context it draws into, and its stack.
    pub(crate) fn parts(&mut self) -> (&mut dyn GraphicsContext, &mut CanvasState) {
        let context: &mut dyn GraphicsContext = match &mut self.target {
            Target::Context(c) => &mut **c,
            Target::Bitmap { picture, .. } => picture,
        };
        let state = match &mut self.state {
            StateRef::Own(s) => s,
            StateRef::Of(s) => &mut **s,
        };
        (context, state)
    }

    /// The context it draws into.
    pub fn context(&mut self) -> &mut dyn GraphicsContext {
        self.parts().0
    }

    /// Its origin in its context.
    pub fn origin(&mut self) -> Offset {
        self.parts().1.origin
    }

    fn depth(&mut self) -> usize {
        self.parts().1.depth()
    }

    fn restore_to(&mut self, depth: usize) {
        let (context, state) = self.parts();
        state.restore_to(context, depth);
    }

    fn push_saved(&mut self, saved: Saved) {
        self.parts().1.stack.push(saved);
    }

    /// `r` in the context's coordinates, sorted (as Skia sorts a rect).
    fn at(&mut self, r: Rect) -> Rect {
        let origin = self.origin();
        sorted(r).translate(origin)
    }

    /// A draw scope over its context at its origin, of no size (what its draws go through).
    fn scope(&mut self) -> DrawScope<'_> {
        let (context, state) = self.parts();
        let (origin, density) = (state.origin, state.density);
        DrawScope::at(context, origin, Size::Zero, LayoutDirection::Ltr, density)
    }

    /// The transform and clip saved; [`restore`](Canvas::restore) takes them back.
    pub fn save(&mut self) {
        self.push_saved(Saved::Save);
    }

    /// Takes back what was pushed since the last save (and composites a saveLayer's layer, faded by its paint's
    /// alpha, through its color filter, by its blend mode). With nothing saved it does nothing.
    pub fn restore(&mut self) {
        let at = self.parts().1.stack.iter().rposition(|s| matches!(s, Saved::Save | Saved::Layer { .. }));
        if let Some(i) = at {
            self.restore_to(i);
        }
    }

    /// What is drawn until the restore goes to a layer over `bounds` (sorted), composited faded by `paint`'s alpha,
    /// through its color filter, by its blend mode.
    pub fn save_layer(&mut self, bounds: Rect, paint: &Paint) {
        let filtered = paint.color_filter.is_some();
        let blended = paint.blend_mode != BlendMode::SrcOver;
        let at = self.at(bounds);
        let context = self.context();
        if let Some(f) = &paint.color_filter {
            context.push_color_filter(f);
        }
        if blended {
            context.push_blend_mode(paint.blend_mode);
        }
        context.push_layer(at, CornerRadii::Zero, paint.alpha(), LayerTransform::Identity);
        self.push_saved(Saved::Layer { filtered, blended });
    }

    /// `block` drawn saved, restored after it (Compose's withSave).
    pub fn with_save(&mut self, block: impl FnOnce(&mut Canvas<'a>)) {
        let depth = self.depth();
        self.save();
        block(self);
        self.restore_to(depth);
    }

    /// `block` drawn in a layer, restored after it (Compose's withSaveLayer).
    pub fn with_save_layer(&mut self, bounds: Rect, paint: &Paint, block: impl FnOnce(&mut Canvas<'a>)) {
        let depth = self.depth();
        self.save_layer(bounds, paint);
        block(self);
        self.restore_to(depth);
    }

    /// Pushes `matrix` (from what is drawn next to this canvas's coordinates) in the context's: from there to this
    /// canvas's, through it, and back.
    fn push(&mut self, matrix: &Matrix) {
        let origin = self.origin();
        let m = around(origin, matrix);
        self.context().push_transform(&m);
        self.push_saved(Saved::Transform);
    }

    /// Each transform goes before what is there (what is drawn next goes through it first).
    pub fn translate(&mut self, dx: f32, dy: f32) {
        let mut m = Matrix::new();
        m.translate(dx, dy, 0.0);
        self.push(&m);
    }

    pub fn scale(&mut self, sx: f32, sy: f32) {
        let mut m = Matrix::new();
        m.scale(sx, sy, 1.0);
        self.push(&m);
    }

    /// Degrees, clockwise.
    pub fn rotate(&mut self, degrees: f32) {
        let mut m = Matrix::new();
        m.rotate_z(degrees);
        self.push(&m);
    }

    /// Skewed by factors (x' = x + sx y, y' = sy x + y), as Android's Canvas and Compose's take them (Compose
    /// documents degrees).
    pub fn skew(&mut self, sx: f32, sy: f32) {
        let mut m = Matrix::new();
        m.set(1, 0, sx);
        m.set(0, 1, sy);
        self.push(&m);
    }

    /// Compose's skewRad: degrees(radians) handed on as [`skew`](Canvas::skew)'s factors.
    pub fn skew_rad(&mut self, sx_rad: f32, sy_rad: f32) {
        self.skew(sx_rad.to_degrees(), sy_rad.to_degrees());
    }

    /// Through `matrix`, its z row and column left out (Android's 3x3).
    pub fn concat(&mut self, matrix: &Matrix) {
        let mut m = Matrix::new();
        for r in [0, 1, 3] {
            for c in [0, 1, 3] {
                m.set(r, c, matrix.get(r, c));
            }
        }
        self.push(&m);
    }

    /// Turned about (`pivot_x`, `pivot_y`); nothing for 0 degrees (as Compose's).
    pub fn rotate_about(&mut self, degrees: f32, pivot_x: f32, pivot_y: f32) {
        if degrees == 0.0 {
            return;
        }
        self.translate(pivot_x, pivot_y);
        self.rotate(degrees);
        self.translate(-pivot_x, -pivot_y);
    }

    /// Radians, about a pivot (Compose's rotateRad).
    pub fn rotate_rad(&mut self, radians: f32, pivot_x: f32, pivot_y: f32) {
        self.rotate_about(radians.to_degrees(), pivot_x, pivot_y);
    }

    /// Scaled about (`pivot_x`, `pivot_y`); nothing for a scale of 1 (as Compose's).
    pub fn scale_about(&mut self, sx: f32, sy: f32, pivot_x: f32, pivot_y: f32) {
        if sx == 1.0 && sy == 1.0 {
            return;
        }
        self.translate(pivot_x, pivot_y);
        self.scale(sx, sy);
        self.translate(-pivot_x, -pivot_y);
    }

    /// Within the clip there is and `rect` (sorted), or outside it (Difference).
    pub fn clip_rect(&mut self, rect: Rect, op: ClipOp) {
        let r = self.at(rect);
        let context = self.context();
        match op {
            ClipOp::Intersect => context.push_clip(r),
            ClipOp::Difference => context.push_clip_round_rect(r, CornerRadii::Zero, op),
        }
        self.push_saved(Saved::Clip);
    }

    pub fn clip_rect_ltrb(&mut self, left: f32, top: f32, right: f32, bottom: f32, op: ClipOp) {
        self.clip_rect(Rect::new(left, top, right, bottom), op);
    }

    /// Within (or outside) `path`, by its fill type.
    pub fn clip_path(&mut self, path: &Path, op: ClipOp) {
        let origin = self.origin();
        self.context().push_clip_path(path, origin, op);
        self.push_saved(Saved::Clip);
    }

    /// Anti-aliasing off around a draw, for a paint that says so.
    fn aliased(&mut self, paint: &Paint, draw: impl FnOnce(&mut Canvas<'a>)) {
        let off = !paint.is_anti_alias;
        if off {
            self.context().push_anti_alias(false);
        }
        draw(self);
        if off {
            self.context().pop_anti_alias();
        }
    }

    /// A line, stroked whatever the paint's style (as Android's).
    pub fn draw_line(&mut self, p1: Offset, p2: Offset, paint: &Paint) {
        self.aliased(paint, |c| {
            c.scope().paint_line(
                &paint.brush(),
                p1,
                p2,
                paint.stroke_width,
                paint.stroke_cap,
                paint.path_effect.clone(),
                paint.alpha(),
                paint.color_filter.as_ref(),
                paint.blend_mode,
            )
        });
    }

    /// A rect (sorted).
    pub fn draw_rect(&mut self, rect: Rect, paint: &Paint) {
        self.aliased(paint, |c| {
            let r = c.at(rect);
            c.scope().paint_round_rect(
                r,
                CornerRadius::Zero,
                &paint.brush(),
                paint.alpha(),
                &paint.path_stroke(),
                paint.color_filter.as_ref(),
                paint.blend_mode,
                false,
            )
        });
    }

    pub fn draw_rect_ltrb(&mut self, left: f32, top: f32, right: f32, bottom: f32, paint: &Paint) {
        self.draw_rect(Rect::new(left, top, right, bottom), paint);
    }

    /// A rounded rect, its corners quarter ellipses (none if either radius is 0 or less).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_round_rect(
        &mut self,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        radius_x: f32,
        radius_y: f32,
        paint: &Paint,
    ) {
        self.aliased(paint, |c| {
            let r = c.at(Rect::new(left, top, right, bottom));
            c.scope().paint_round_rect(
                r,
                CornerRadius::new(radius_x, radius_y),
                &paint.brush(),
                paint.alpha(),
                &paint.path_stroke(),
                paint.color_filter.as_ref(),
                paint.blend_mode,
                false,
            )
        });
    }

    /// The oval in a rect (sorted).
    pub fn draw_oval(&mut self, rect: Rect, paint: &Paint) {
        self.aliased(paint, |c| {
            let r = c.at(rect);
            c.scope().paint_oval(
                r,
                &paint.brush(),
                paint.alpha(),
                &paint.path_stroke(),
                paint.color_filter.as_ref(),
                paint.blend_mode,
            )
        });
    }

    pub fn draw_oval_ltrb(&mut self, left: f32, top: f32, right: f32, bottom: f32, paint: &Paint) {
        self.draw_oval(Rect::new(left, top, right, bottom), paint);
    }

    pub fn draw_circle(&mut self, center: Offset, radius: f32, paint: &Paint) {
        self.aliased(paint, |c| {
            let r = Rect::from_center(center, radius).translate(c.origin());
            c.scope().paint_round_rect(
                r,
                CornerRadius::circular(radius),
                &paint.brush(),
                paint.alpha(),
                &paint.path_stroke(),
                paint.color_filter.as_ref(),
                paint.blend_mode,
                true,
            )
        });
    }

    /// Part of the oval in a rect (sorted): from `start_angle` through `sweep_angle` (degrees, 0 at 3 o'clock,
    /// clockwise), through its center with `use_center`.
    pub fn draw_arc(&mut self, rect: Rect, start_angle: f32, sweep_angle: f32, use_center: bool, paint: &Paint) {
        self.aliased(paint, |c| {
            let r = c.at(rect);
            c.scope().paint_arc(
                r,
                start_angle,
                sweep_angle,
                use_center,
                &paint.brush(),
                paint.alpha(),
                &paint.path_stroke(),
                paint.color_filter.as_ref(),
                paint.blend_mode,
            )
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_arc_ltrb(
        &mut self,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        start_angle: f32,
        sweep_angle: f32,
        use_center: bool,
        paint: &Paint,
    ) {
        self.draw_arc(Rect::new(left, top, right, bottom), start_angle, sweep_angle, use_center, paint);
    }

    /// The same in radians (Compose's drawArcRad).
    pub fn draw_arc_rad(&mut self, rect: Rect, start_rad: f32, sweep_rad: f32, use_center: bool, paint: &Paint) {
        self.draw_arc(rect, start_rad.to_degrees(), sweep_rad.to_degrees(), use_center, paint);
    }

    pub fn draw_path(&mut self, path: &Path, paint: &Paint) {
        self.aliased(paint, |c| {
            c.scope().paint_path(
                path,
                &paint.brush(),
                paint.alpha(),
                &paint.path_stroke(),
                paint.color_filter.as_ref(),
                paint.blend_mode,
            )
        });
    }

    /// An image at its pixels as px from `top_left`, faded by the paint's alpha (an Alpha8 one in the paint's color
    /// or shader).
    pub fn draw_image(&mut self, image: &ImageBitmap, top_left: Offset, paint: &Paint) {
        let (w, h) = (image.width() as f32, image.height() as f32);
        self.image(image, Rect::new(0.0, 0.0, w, h), Rect::from_offset_size(top_left, Size::new(w, h)), paint);
    }

    /// An image (its paint's alpha; an Alpha8 one in the paint's color or shader), `dst` in this canvas's
    /// coordinates.
    fn image(&mut self, image: &ImageBitmap, src: Rect, dst: Rect, paint: &Paint) {
        if paint.alpha() <= 0.0 {
            return;
        }
        let origin = self.origin();
        let brush = paint.brush();
        self.aliased(paint, |c| {
            let context = c.context();
            with_paint(context, paint.color_filter.as_ref(), paint.blend_mode, |g| {
                paint_image(
                    g,
                    image,
                    src,
                    dst.translate(origin),
                    paint.alpha(),
                    paint.filter_quality,
                    &brush,
                    Rect::from_offset_size(origin, Size::Zero),
                )
            });
        });
    }

    /// The pixels from `src_offset`, `src_size` of `image` into the rect from `dst_offset`, `dst_size`.
    pub fn draw_image_rect_with(
        &mut self,
        image: &ImageBitmap,
        src_offset: IntOffset,
        src_size: IntSize,
        dst_offset: IntOffset,
        dst_size: IntSize,
        paint: &Paint,
    ) {
        let src = Rect::from_offset_size(src_offset.to_offset(), src_size.to_size());
        let dst = Rect::from_offset_size(dst_offset.to_offset(), dst_size.to_size());
        self.image(image, src, dst, paint);
    }

    /// Points as DrawScope's drawPoints draws them.
    pub fn draw_points(&mut self, point_mode: PointMode, points: &[Offset], paint: &Paint) {
        self.aliased(paint, |c| {
            c.scope().paint_points(
                points,
                point_mode,
                &paint.brush(),
                paint.stroke_width,
                paint.stroke_cap,
                paint.path_effect.clone(),
                paint.alpha(),
                paint.color_filter.as_ref(),
                paint.blend_mode,
            )
        });
    }

    /// The same from x, y pairs (an odd last one left out).
    pub fn draw_raw_points(&mut self, point_mode: PointMode, points: &[f32], paint: &Paint) {
        let pairs: Vec<Offset> = points.as_chunks::<2>().0.iter().map(|&[x, y]| Offset::new(x, y)).collect();
        self.draw_points(point_mode, &pairs, paint);
    }

    /// A triangle mesh, as Skia draws one (see [`GraphicsContext::draw_vertices`]): the paint's shader where each
    /// vertex's texture coordinate puts it (in the canvas's coordinates), or its color opaque, through the vertex
    /// colors by `blend_mode`, faded by the paint's alpha, its color filter and blend mode as any draw's. (Compose on
    /// Android leaves `blend_mode` out, using Modulate; Compose documents it, and its other platforms use it.)
    pub fn draw_vertices(&mut self, vertices: &Vertices, blend_mode: BlendMode, paint: &Paint) {
        if paint.alpha() <= 0.0 {
            return;
        }
        let origin = self.origin();
        let brush = paint.brush();
        let context = self.context();
        with_paint(context, paint.color_filter.as_ref(), paint.blend_mode, |g| {
            g.draw_vertices(
                vertices,
                origin,
                blend_mode,
                &brush,
                Rect::from_offset_size(origin, Size::Zero),
                paint.alpha(),
            )
        });
    }

    /// Android lets layers drawn after enableZ reorder by their elevation; KompozR draws them in order (z order is
    /// the layout's: `z_index`), so these do nothing.
    pub fn enable_z(&mut self) {}
    pub fn disable_z(&mut self) {}
}

#[named]
impl<'a> Canvas<'a> {
    /// Part of `image` into a rect (Compose's drawImageRect): from `src_offset`, `src_size` (its size by default)
    /// into the rect from `dst_offset`, `dst_size` (the source's size by default).
    pub fn draw_image_rect(
        &mut self,
        image: &ImageBitmap,
        #[default(IntOffset::Zero)] src_offset: IntOffset,
        #[default(UNSET_SIZE)] src_size: IntSize,
        #[default(IntOffset::Zero)] dst_offset: IntOffset,
        #[default(UNSET_SIZE)] dst_size: IntSize,
        paint: &Paint,
    ) {
        let src_size = if src_size == UNSET_SIZE { IntSize::new(image.width(), image.height()) } else { src_size };
        let dst_size = if dst_size == UNSET_SIZE { src_size } else { dst_size };
        self.draw_image_rect_with(image, src_offset, src_size, dst_offset, dst_size, paint);
    }
}

impl Drop for Canvas<'_> {
    fn drop(&mut self) {
        if let StateRef::Own(_) = self.state {
            self.restore_to(0);
        }
        if let Target::Bitmap { picture, image } = &mut self.target
            && !picture.is_empty()
        {
            draw_into_bitmap(image, Rc::new(std::mem::take(picture)));
        }
    }
}

/// A size not given (a default worked out from other parameters).
pub(crate) const UNSET_SIZE: IntSize = IntSize::new(-1, -1);

/// A rect sorted (a negative size reaching back), as Skia sorts a shape's rect.
pub(crate) fn sorted(r: Rect) -> Rect {
    Rect::new(r.left.min(r.right), r.top.min(r.bottom), r.left.max(r.right), r.top.max(r.bottom))
}

/// `matrix` (from a drawing's coordinates to those of something at `origin`) in the coordinates `origin` is in.
pub(crate) fn around(origin: Offset, matrix: &Matrix) -> Matrix {
    let mut m = Matrix::new();
    m.translate(-origin.x, -origin.y, 0.0);
    m.times_assign(matrix);
    let mut back = Matrix::new();
    back.translate(origin.x, origin.y, 0.0);
    m.times_assign(&back);
    m
}

/// `draw` with a color filter and a blend mode other than SrcOver pushed around it.
pub(crate) fn with_paint(
    context: &mut dyn GraphicsContext,
    color_filter: Option<&ColorFilter>,
    blend_mode: BlendMode,
    draw: impl FnOnce(&mut dyn GraphicsContext),
) {
    let blended = blend_mode != BlendMode::SrcOver;
    if blended {
        context.push_blend_mode(blend_mode);
    }
    if let Some(f) = color_filter {
        context.push_color_filter(f);
    }
    draw(context);
    if color_filter.is_some() {
        context.pop_color_filter();
    }
    if blended {
        context.pop_blend_mode();
    }
}

/// An image: as it is, or an Alpha8 one as a mask (as Android draws one): `brush` (its paint's color or shader; a
/// DrawScope's: black, as Compose's) where it is, as opaque as it is.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_image(
    context: &mut dyn GraphicsContext,
    image: &ImageBitmap,
    src: Rect,
    dst: Rect,
    alpha: f32,
    quality: FilterQuality,
    brush: &Brush,
    brush_area: Rect,
) {
    if image.config() == ImageBitmapConfig::Alpha8 {
        context.draw_image_mask(image, src, dst, brush, brush_area, alpha, quality);
    } else {
        context.draw_image(image, src, dst, alpha, quality);
    }
}

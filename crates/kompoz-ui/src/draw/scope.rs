//! DrawScope's drawing (Compose's DrawScope over the [`GraphicsContext`], as the C++ Kompoz draws it): shapes,
//! lines, points, paths, images and outlines in a color or a brush (a gradient spread over the whole area), faded,
//! filled or stroked, through a color filter, by a blend mode; text; transforms of what a block draws (translate,
//! inset, clip, rotate, scale, `with_transform`); its canvas.
//!
//! Each call with Compose's defaults takes them by name in a kompozable body (`d.draw_rect(red, alpha = 0.5)`); in
//! plain code the call takes the parameters without a default, and its `…_with` form all of them. A color is a
//! brush (`impl Into<Brush>`), so the Color and Brush overloads of Compose are one call each.

use super::canvas::{CanvasSlot, CanvasState, around, paint_image, sorted, with_paint};
use super::{Canvas, ContentDrawScope, DefaultBlendMode, DrawScope, DrawStyle, Fill, PointMode, Stroke};
use crate::graphics_context::{ClipOp, CornerRadii, GraphicsContext, TextDraw};
use kompoz_graphics::{
    BlendMode, Brush, Color, ColorFilter, CornerRadius, DefaultFilterQuality, Density, FilterQuality, ImageBitmap,
    IntOffset, IntSize, LayoutDirection, Matrix, Offset, Outline, Path, PathDirection, PathEffect, PathStroke, Rect,
    RoundRect, Size, StrokeCap, StrokeJoin,
};
use kompoz_runtime::named;

/// The color with another alpha, in its color space.
pub(crate) fn with_alpha(c: Color, alpha: f32) -> Color {
    Color::new_in(c.red(), c.green(), c.blue(), alpha, c.color_space())
}

/// A color faded by an alpha.
pub(crate) fn faded(c: Color, alpha: f32) -> Color {
    if alpha < 1.0 { with_alpha(c, c.alpha() * alpha.max(0.0)) } else { c }
}

/// The path Skia draws for an arc (SkPathPriv::CreateDrawArcPath): a whole oval for a filled sweep of a turn or more;
/// else through the center first if `use_center` (and back to it), the sweep in half turns while it is a turn or
/// more (`arc_to` goes round only once).
fn arc_path(oval: Rect, mut start: f32, mut sweep: f32, use_center: bool, fill_no_effect: bool) -> Path {
    let mut p = Path::new();
    if fill_no_effect && sweep.abs() >= 360.0 {
        p.add_oval(oval, PathDirection::Clockwise);
        return p;
    }
    if use_center {
        let c = oval.center();
        p.move_to(c.x, c.y);
    }
    let mut force_move_to = !use_center;
    for half in [-180.0f32, 180.0] {
        while if half < 0.0 { sweep <= -360.0 } else { sweep >= 360.0 } {
            p.arc_to(oval, start, half, force_move_to);
            start += half;
            p.arc_to(oval, start, half, false);
            start += half;
            force_move_to = false;
            sweep -= 2.0 * half;
        }
    }
    p.arc_to(oval, start, sweep, force_move_to);
    if use_center {
        p.close();
    }
    p
}

/// A size not given: to the area's far corner (or its default worked out).
const UNSET: Size = Size::Unspecified;

// ---- What the drawing calls draw through ------------------------------------------------------------------------

impl<'a> DrawScope<'a> {
    /// What brushes are spread over: the whole area, where it is in the context.
    pub(crate) fn area(&self) -> Rect {
        Rect::from_offset_size(self.origin, self.size)
    }

    /// The rect from `top_left` of `size` (unspecified: to the area's bottom right), in the context.
    fn local(&self, top_left: Offset, size: Size) -> Rect {
        let s = if size.width.is_nan() || size.height.is_nan() {
            Size::new(self.size.width - top_left.x, self.size.height - top_left.y)
        } else {
            size
        };
        Rect::from_offset_size(self.origin + top_left, s)
    }

    /// A path (this scope's coordinates) in a brush, faded, filled or stroked (through the stroke's path effect
    /// first), its colors filtered, blended.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_path(
        &mut self,
        path: &Path,
        brush: &Brush,
        alpha: f32,
        stroke: &PathStroke,
        color_filter: Option<&ColorFilter>,
        blend_mode: BlendMode,
    ) {
        if path.is_empty() || alpha <= 0.0 {
            return;
        }
        let b = brush.faded(alpha);
        let (origin, area) = (self.origin, self.area());
        with_paint(&mut *self.canvas, color_filter, blend_mode, |g| g.draw_path(path, origin, &b, area, stroke));
    }

    /// A rounded rect (in the context; `oval`: a circle), as the backend draws one where it can: filled, or stroked
    /// along it (half each side) where its corners are circles and the stroke's joins can't show (curved corners, or
    /// square ones mitered); else as the path Android's canvas makes of it (Skia's defaults: clockwise, an oval from
    /// its right, a rect from its top left, a round rect from its sixth point).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_round_rect(
        &mut self,
        r: Rect,
        corner: CornerRadius,
        brush: &Brush,
        alpha: f32,
        stroke: &PathStroke,
        color_filter: Option<&ColorFilter>,
        blend_mode: BlendMode,
        oval: bool,
    ) {
        if alpha <= 0.0 {
            return;
        }
        let circular = corner.x == corner.y;
        let radius = corner.x.max(0.0);
        let strokes = stroke.strokes();
        let plain = stroke.path_effect.is_none()
            && (!strokes
                || (stroke.width > 0.0 && !stroke.hairline && (radius > 0.0 || stroke.join == StrokeJoin::Miter)));
        if !circular || !plain || (strokes && !brush.is_solid()) {
            let at = r.translate(-self.origin);
            let mut outline = Path::new();
            if oval {
                outline.add_oval(at, PathDirection::Clockwise);
            } else if corner.x <= 0.0 || corner.y <= 0.0 {
                outline.add_rect(at, PathDirection::Clockwise);
            } else {
                outline.add_round_rect(&RoundRect::from_rect_radius(at, corner), PathDirection::Clockwise);
            }
            return self.paint_path(&outline, brush, alpha, stroke, color_filter, blend_mode);
        }
        let area = self.area();
        with_paint(&mut *self.canvas, color_filter, blend_mode, |g| {
            if strokes {
                let half = stroke.width / 2.0;
                let radii = if radius > 0.0 { radius + half } else { 0.0 };
                let color = brush.solid_color().unwrap_or(Color::Transparent);
                g.stroke_round_rect(r.inflate(half), CornerRadii::all(radii), stroke.width, faded(color, alpha));
            } else if let Some(color) = brush.solid_color() {
                g.fill_round_rect(r, CornerRadii::all(radius), faded(color, alpha));
            } else {
                g.fill_gradient(r, CornerRadii::all(radius), &brush.faded(alpha), area);
            }
        });
    }

    /// An oval in a rect (in the context): a circle as a rounded rect, any other as its path.
    pub(crate) fn paint_oval(
        &mut self,
        r: Rect,
        brush: &Brush,
        alpha: f32,
        stroke: &PathStroke,
        color_filter: Option<&ColorFilter>,
        blend_mode: BlendMode,
    ) {
        if r.width() == r.height() {
            let corner = CornerRadius::circular(r.width() / 2.0);
            return self.paint_round_rect(r, corner, brush, alpha, stroke, color_filter, blend_mode, true);
        }
        let mut oval = Path::new();
        oval.add_oval(r.translate(-self.origin), PathDirection::Clockwise);
        self.paint_path(&oval, brush, alpha, stroke, color_filter, blend_mode);
    }

    /// A line: square capped in a color as the backend draws one (its ends square); else (butt or round ends,
    /// a brush, a path effect) the line's path stroked.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_line(
        &mut self,
        brush: &Brush,
        start: Offset,
        end: Offset,
        stroke_width: f32,
        cap: StrokeCap,
        path_effect: Option<PathEffect>,
        alpha: f32,
        color_filter: Option<&ColorFilter>,
        blend_mode: BlendMode,
    ) {
        if let Some(color) = brush.solid_color()
            && stroke_width > 0.0
            && cap == StrokeCap::Square
            && path_effect.is_none()
        {
            if alpha <= 0.0 {
                return;
            }
            let (a, b) = (self.origin + start, self.origin + end);
            with_paint(&mut *self.canvas, color_filter, blend_mode, |g| {
                g.draw_line(a, b, stroke_width, faded(color, alpha))
            });
            return;
        }
        let mut line = Path::new();
        line.move_to(start.x, start.y);
        line.line_to(end.x, end.y);
        let stroke = Stroke::new(stroke_width, Stroke::DefaultMiter, cap, StrokeJoin::Miter, path_effect).path_stroke();
        self.paint_path(&line, brush, alpha, &stroke, color_filter, blend_mode);
    }

    /// Part of an oval (in the context): an arc of a circle stroked butt or round in a color as the backend draws
    /// one, any other as Skia's path of it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_arc(
        &mut self,
        oval: Rect,
        start: f32,
        sweep: f32,
        use_center: bool,
        brush: &Brush,
        alpha: f32,
        stroke: &PathStroke,
        color_filter: Option<&ColorFilter>,
        blend_mode: BlendMode,
    ) {
        let strokes = stroke.strokes();
        if let Some(color) = brush.solid_color()
            && strokes
            && !use_center
            && stroke.width > 0.0
            && !stroke.hairline
            && stroke.path_effect.is_none()
            && stroke.cap != StrokeCap::Square
            && oval.width() == oval.height()
            && sweep.abs() < 360.0
        {
            if alpha <= 0.0 {
                return;
            }
            let round = stroke.cap == StrokeCap::Round;
            with_paint(&mut *self.canvas, color_filter, blend_mode, |g| {
                g.draw_arc(oval, start, sweep, stroke.width, faded(color, alpha), round)
            });
            return;
        }
        let fill_no_effect = !strokes && stroke.path_effect.is_none();
        let path = arc_path(oval.translate(-self.origin), start, sweep, use_center, fill_no_effect);
        self.paint_path(&path, brush, alpha, stroke, color_filter, blend_mode);
    }

    /// As Skia's drawPoints: dots as wide as the stroke (a hairline's a pixel), round with a round cap, square
    /// otherwise; lines between each two; one line through them all.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_points(
        &mut self,
        points: &[Offset],
        point_mode: PointMode,
        brush: &Brush,
        stroke_width: f32,
        cap: StrokeCap,
        path_effect: Option<PathEffect>,
        alpha: f32,
        color_filter: Option<&ColorFilter>,
        blend_mode: BlendMode,
    ) {
        if points.is_empty() {
            return;
        }
        let mut path = Path::new();
        if point_mode == PointMode::Points {
            let w = if stroke_width > 0.0 { stroke_width } else { 1.0 };
            let r = w / 2.0;
            for p in points {
                let dot = Rect::new(p.x - r, p.y - r, p.x + r, p.y + r);
                if cap == StrokeCap::Round {
                    path.add_oval(dot, PathDirection::CounterClockwise);
                } else {
                    path.add_rect(dot, PathDirection::CounterClockwise);
                }
            }
            return self.paint_path(&path, brush, alpha, &PathStroke::default(), color_filter, blend_mode);
        }
        for (i, p) in points.iter().enumerate() {
            let start = if point_mode == PointMode::Polygon { i == 0 } else { i % 2 == 0 };
            if point_mode == PointMode::Lines && start && i + 1 == points.len() {
                break; // (an odd one out has no partner)
            }
            if start {
                path.move_to(p.x, p.y);
            } else {
                path.line_to(p.x, p.y);
            }
        }
        let stroke = Stroke::new(stroke_width, Stroke::DefaultMiter, cap, StrokeJoin::Miter, path_effect).path_stroke();
        self.paint_path(&path, brush, alpha, &stroke, color_filter, blend_mode);
    }

    /// An image's `src` pixels into `dst` (in this scope's coordinates); an Alpha8 one as a mask in black (as
    /// Compose's DrawScope draws one).
    #[allow(clippy::too_many_arguments)]
    fn paint_image_rect(
        &mut self,
        image: &ImageBitmap,
        src: Rect,
        dst: Rect,
        alpha: f32,
        color_filter: Option<&ColorFilter>,
        blend_mode: BlendMode,
        filter_quality: FilterQuality,
    ) {
        if alpha <= 0.0 {
            return;
        }
        let (dst, area) = (dst.translate(self.origin), self.area());
        with_paint(&mut *self.canvas, color_filter, blend_mode, |g| {
            paint_image(g, image, src, dst, alpha.min(1.0), filter_quality, &Brush::from(Color::Black), area)
        });
    }

    /// An outline (in this scope's coordinates): a rect as a rect, a rounded rect with one radius as a rounded rect,
    /// any other as its path.
    pub(crate) fn paint_outline(
        &mut self,
        outline: &Outline,
        brush: &Brush,
        alpha: f32,
        stroke: &PathStroke,
        cf: Option<&ColorFilter>,
        blend_mode: BlendMode,
    ) {
        match outline {
            Outline::Rectangle(r) => {
                let r = sorted(r.translate(self.origin));
                self.paint_round_rect(r, CornerRadius::Zero, brush, alpha, stroke, cf, blend_mode, false);
            }
            Outline::Rounded(rr) if rr.is_simple() => {
                let r = sorted(rr.bounding_rect().translate(self.origin));
                let radius = CornerRadius::circular(rr.bottom_left_corner_radius.x);
                self.paint_round_rect(r, radius, brush, alpha, stroke, cf, blend_mode, false);
            }
            Outline::Rounded(rr) => {
                let mut p = Path::new();
                p.add_round_rect(rr, PathDirection::CounterClockwise);
                self.paint_path(&p, brush, alpha, stroke, cf, blend_mode);
            }
            Outline::Generic(p) => self.paint_path(p, brush, alpha, stroke, cf, blend_mode),
        }
    }

    /// Its canvas's stack, made when first asked for (at its origin, its density).
    pub(crate) fn canvas_state_mut(&mut self) -> &mut CanvasState {
        let (origin, density) = (self.origin, self.density);
        if let CanvasSlot::Own(s) = &mut self.canvas_state
            && s.is_none()
        {
            *s = Some(CanvasState::new(origin, density));
        }
        self.canvas_state.get().expect("a canvas state")
    }

    /// How deep its canvas's stack is (0 without one).
    fn canvas_depth(&mut self) -> usize {
        self.canvas_state.get().map_or(0, |s| s.depth())
    }

    /// Pops what its canvas pushed past `depth`.
    fn restore_canvas_to(&mut self, depth: usize) {
        if let Some(state) = self.canvas_state.get() {
            state.restore_to(&mut *self.canvas, depth);
        }
    }
}

// ---- The drawing calls ------------------------------------------------------------------------------------------

#[named]
impl<'a> DrawScope<'a> {
    /// A line from `start` to `end` (Compose's drawLine), `stroke_width` px wide (0: a hairline), its ends `cap`,
    /// through `path_effect`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_line(
        &mut self,
        brush: impl Into<Brush>,
        start: Offset,
        end: Offset,
        #[default(Stroke::HairlineWidth)] stroke_width: f32,
        #[default(Stroke::DefaultCap)] cap: StrokeCap,
        #[default(None)] path_effect: Option<PathEffect>,
        #[default(1.0)] alpha: f32,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        let brush = brush.into();
        self.paint_line(&brush, start, end, stroke_width, cap, path_effect, alpha, color_filter.as_ref(), blend_mode);
    }

    /// A rect from `top_left` of `size` (unspecified: to the area's bottom right), sorted as Skia sorts it (Compose's
    /// drawRect).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_rect(
        &mut self,
        brush: impl Into<Brush>,
        #[default(Offset::Zero)] top_left: Offset,
        #[default(UNSET)] size: Size,
        #[default(1.0)] alpha: f32,
        #[default(Fill)] style: DrawStyle,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        let r = sorted(self.local(top_left, size));
        let brush = brush.into();
        let stroke = style.path_stroke();
        self.paint_round_rect(r, CornerRadius::Zero, &brush, alpha, &stroke, color_filter.as_ref(), blend_mode, false);
    }

    /// A rounded rect, its corners quarter ellipses of `corner_radius` (Compose's drawRoundRect).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_round_rect(
        &mut self,
        brush: impl Into<Brush>,
        #[default(Offset::Zero)] top_left: Offset,
        #[default(UNSET)] size: Size,
        #[default(CornerRadius::Zero)] corner_radius: CornerRadius,
        #[default(1.0)] alpha: f32,
        #[default(Fill)] style: DrawStyle,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        let r = sorted(self.local(top_left, size));
        let brush = brush.into();
        let stroke = style.path_stroke();
        self.paint_round_rect(r, corner_radius, &brush, alpha, &stroke, color_filter.as_ref(), blend_mode, false);
    }

    /// A circle of `radius` (the largest that fits by default) about `center` (the middle by default) (Compose's
    /// drawCircle).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_circle(
        &mut self,
        brush: impl Into<Brush>,
        #[default(self.size.min_dimension() / 2.0)] radius: f32,
        #[default(self.center())] center: Offset,
        #[default(1.0)] alpha: f32,
        #[default(Fill)] style: DrawStyle,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        let r = Rect::from_center(self.origin + center, radius);
        let brush = brush.into();
        let stroke = style.path_stroke();
        let corner = CornerRadius::circular(radius);
        self.paint_round_rect(r, corner, &brush, alpha, &stroke, color_filter.as_ref(), blend_mode, true);
    }

    /// The oval in a rect (Compose's drawOval).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_oval(
        &mut self,
        brush: impl Into<Brush>,
        #[default(Offset::Zero)] top_left: Offset,
        #[default(UNSET)] size: Size,
        #[default(1.0)] alpha: f32,
        #[default(Fill)] style: DrawStyle,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        let r = sorted(self.local(top_left, size));
        let brush = brush.into();
        let stroke = style.path_stroke();
        self.paint_oval(r, &brush, alpha, &stroke, color_filter.as_ref(), blend_mode);
    }

    /// Part of the oval in a rect: from `start_angle` through `sweep_angle` (degrees, 0 at 3 o'clock, clockwise);
    /// `use_center` closes it through the center (a pie slice), else straight across its ends (filled) or not at
    /// all (outlined) (Compose's drawArc).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_arc(
        &mut self,
        brush: impl Into<Brush>,
        start_angle: f32,
        sweep_angle: f32,
        use_center: bool,
        #[default(Offset::Zero)] top_left: Offset,
        #[default(UNSET)] size: Size,
        #[default(1.0)] alpha: f32,
        #[default(Fill)] style: DrawStyle,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        let oval = sorted(self.local(top_left, size));
        let brush = brush.into();
        let stroke = style.path_stroke();
        let cf = color_filter.as_ref();
        self.paint_arc(oval, start_angle, sweep_angle, use_center, &brush, alpha, &stroke, cf, blend_mode);
    }

    /// A path (Compose's drawPath).
    pub fn draw_path(
        &mut self,
        path: &Path,
        brush: impl Into<Brush>,
        #[default(1.0)] alpha: f32,
        #[default(Fill)] style: DrawStyle,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        let brush = brush.into();
        self.paint_path(path, &brush, alpha, &style.path_stroke(), color_filter.as_ref(), blend_mode);
    }

    /// Points (Compose's drawPoints): each a dot (round with a round cap, else square), every two a line, or all
    /// of them one line.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_points(
        &mut self,
        points: &[Offset],
        point_mode: PointMode,
        brush: impl Into<Brush>,
        #[default(Stroke::HairlineWidth)] stroke_width: f32,
        #[default(StrokeCap::Butt)] cap: StrokeCap,
        #[default(None)] path_effect: Option<PathEffect>,
        #[default(1.0)] alpha: f32,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        let brush = brush.into();
        let cf = color_filter.as_ref();
        self.paint_points(points, point_mode, &brush, stroke_width, cap, path_effect, alpha, cf, blend_mode);
    }

    /// An image at its size, its pixels as px, from `top_left` (Compose's drawImage); a stroke style draws it as a
    /// fill does (as Android's).
    pub fn draw_image(
        &mut self,
        image: &ImageBitmap,
        #[default(Offset::Zero)] top_left: Offset,
        #[default(1.0)] alpha: f32,
        #[default(Fill)] style: DrawStyle,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        let _ = style; // (an image is drawn the same either way, as on Android)
        let (w, h) = (image.width() as f32, image.height() as f32);
        let src = Rect::new(0.0, 0.0, w, h);
        let dst = Rect::from_offset_size(top_left, Size::new(w, h));
        self.paint_image_rect(image, src, dst, alpha, color_filter.as_ref(), blend_mode, DefaultFilterQuality);
    }

    /// The pixels of `image` from `src_offset`, `src_size` (all of it by default, as Compose's) into the rect from
    /// `dst_offset`, `dst_size` (the source's size by default), sampled as `filter_quality` says (Compose's
    /// drawImage with a source and a destination).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_image_rect(
        &mut self,
        image: &ImageBitmap,
        #[default(IntOffset::Zero)] src_offset: IntOffset,
        #[default(IntSize::new(-1, -1))] src_size: IntSize,
        #[default(IntOffset::Zero)] dst_offset: IntOffset,
        #[default(IntSize::new(-1, -1))] dst_size: IntSize,
        #[default(1.0)] alpha: f32,
        #[default(Fill)] style: DrawStyle,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
        #[default(DefaultFilterQuality)] filter_quality: FilterQuality,
    ) {
        let _ = style;
        let unset = IntSize::new(-1, -1);
        let src_size = if src_size == unset { IntSize::new(image.width(), image.height()) } else { src_size };
        let dst_size = if dst_size == unset { src_size } else { dst_size };
        let src = Rect::from_offset_size(src_offset.to_offset(), src_size.to_size());
        let dst = Rect::from_offset_size(dst_offset.to_offset(), dst_size.to_size());
        self.paint_image_rect(image, src, dst, alpha, color_filter.as_ref(), blend_mode, filter_quality);
    }

    /// An outline (Compose's drawOutline): a rect as a rect, a rounded rect with one radius as a rounded rect, any
    /// other as its path.
    pub fn draw_outline(
        &mut self,
        outline: &Outline,
        brush: impl Into<Brush>,
        #[default(1.0)] alpha: f32,
        #[default(Fill)] style: DrawStyle,
        #[default(None)] color_filter: Option<ColorFilter>,
        #[default(DefaultBlendMode)] blend_mode: BlendMode,
    ) {
        self.paint_outline(outline, &brush.into(), alpha, &style.path_stroke(), color_filter.as_ref(), blend_mode);
    }
}

/// The `…_with` forms: every parameter given, for plain code.
impl<'a> DrawScope<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn draw_line_with(
        &mut self,
        brush: impl Into<Brush>,
        start: Offset,
        end: Offset,
        stroke_width: f32,
        cap: StrokeCap,
        path_effect: Option<PathEffect>,
        alpha: f32,
        color_filter: Option<ColorFilter>,
        blend_mode: BlendMode,
    ) {
        let brush = brush.into();
        self.paint_line(&brush, start, end, stroke_width, cap, path_effect, alpha, color_filter.as_ref(), blend_mode);
    }

    /// `size` unspecified: to the area's bottom right.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_rect_with(
        &mut self,
        brush: impl Into<Brush>,
        top_left: Offset,
        size: Size,
        alpha: f32,
        style: DrawStyle,
        color_filter: Option<ColorFilter>,
        blend_mode: BlendMode,
    ) {
        let r = sorted(self.local(top_left, size));
        let brush = brush.into();
        let stroke = style.path_stroke();
        self.paint_round_rect(r, CornerRadius::Zero, &brush, alpha, &stroke, color_filter.as_ref(), blend_mode, false);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_round_rect_with(
        &mut self,
        brush: impl Into<Brush>,
        top_left: Offset,
        size: Size,
        corner_radius: CornerRadius,
        alpha: f32,
        style: DrawStyle,
        color_filter: Option<ColorFilter>,
        blend_mode: BlendMode,
    ) {
        let r = sorted(self.local(top_left, size));
        let brush = brush.into();
        let stroke = style.path_stroke();
        self.paint_round_rect(r, corner_radius, &brush, alpha, &stroke, color_filter.as_ref(), blend_mode, false);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_circle_with(
        &mut self,
        brush: impl Into<Brush>,
        radius: f32,
        center: Offset,
        alpha: f32,
        style: DrawStyle,
        color_filter: Option<ColorFilter>,
        blend_mode: BlendMode,
    ) {
        let r = Rect::from_center(self.origin + center, radius);
        let brush = brush.into();
        let stroke = style.path_stroke();
        let corner = CornerRadius::circular(radius);
        self.paint_round_rect(r, corner, &brush, alpha, &stroke, color_filter.as_ref(), blend_mode, true);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_oval_with(
        &mut self,
        brush: impl Into<Brush>,
        top_left: Offset,
        size: Size,
        alpha: f32,
        style: DrawStyle,
        color_filter: Option<ColorFilter>,
        blend_mode: BlendMode,
    ) {
        let r = sorted(self.local(top_left, size));
        let brush = brush.into();
        self.paint_oval(r, &brush, alpha, &style.path_stroke(), color_filter.as_ref(), blend_mode);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_arc_with(
        &mut self,
        brush: impl Into<Brush>,
        start_angle: f32,
        sweep_angle: f32,
        use_center: bool,
        top_left: Offset,
        size: Size,
        alpha: f32,
        style: DrawStyle,
        color_filter: Option<ColorFilter>,
        blend_mode: BlendMode,
    ) {
        let oval = sorted(self.local(top_left, size));
        let brush = brush.into();
        let stroke = style.path_stroke();
        let cf = color_filter.as_ref();
        self.paint_arc(oval, start_angle, sweep_angle, use_center, &brush, alpha, &stroke, cf, blend_mode);
    }

    pub fn draw_path_with(
        &mut self,
        path: &Path,
        brush: impl Into<Brush>,
        alpha: f32,
        style: DrawStyle,
        color_filter: Option<ColorFilter>,
        blend_mode: BlendMode,
    ) {
        let brush = brush.into();
        self.paint_path(path, &brush, alpha, &style.path_stroke(), color_filter.as_ref(), blend_mode);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_points_with(
        &mut self,
        points: &[Offset],
        point_mode: PointMode,
        brush: impl Into<Brush>,
        stroke_width: f32,
        cap: StrokeCap,
        path_effect: Option<PathEffect>,
        alpha: f32,
        color_filter: Option<ColorFilter>,
        blend_mode: BlendMode,
    ) {
        let brush = brush.into();
        let cf = color_filter.as_ref();
        self.paint_points(points, point_mode, &brush, stroke_width, cap, path_effect, alpha, cf, blend_mode);
    }

    pub fn draw_image_with(
        &mut self,
        image: &ImageBitmap,
        top_left: Offset,
        alpha: f32,
        style: DrawStyle,
        color_filter: Option<ColorFilter>,
        blend_mode: BlendMode,
    ) {
        let _ = style;
        let (w, h) = (image.width() as f32, image.height() as f32);
        let src = Rect::new(0.0, 0.0, w, h);
        let dst = Rect::from_offset_size(top_left, Size::new(w, h));
        self.paint_image_rect(image, src, dst, alpha, color_filter.as_ref(), blend_mode, DefaultFilterQuality);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_image_rect_with(
        &mut self,
        image: &ImageBitmap,
        src_offset: IntOffset,
        src_size: IntSize,
        dst_offset: IntOffset,
        dst_size: IntSize,
        alpha: f32,
        style: DrawStyle,
        color_filter: Option<ColorFilter>,
        blend_mode: BlendMode,
        filter_quality: FilterQuality,
    ) {
        let _ = style;
        let src = Rect::from_offset_size(src_offset.to_offset(), src_size.to_size());
        let dst = Rect::from_offset_size(dst_offset.to_offset(), dst_size.to_size());
        self.paint_image_rect(image, src, dst, alpha, color_filter.as_ref(), blend_mode, filter_quality);
    }

    pub fn draw_outline_with(
        &mut self,
        outline: &Outline,
        brush: impl Into<Brush>,
        alpha: f32,
        style: DrawStyle,
        color_filter: Option<ColorFilter>,
        blend_mode: BlendMode,
    ) {
        self.paint_outline(outline, &brush.into(), alpha, &style.path_stroke(), color_filter.as_ref(), blend_mode);
    }

    /// A laid out run of text from its baseline (in this scope's coordinates), in `color`: the low-level call
    /// Compose's drawText (a TextLayoutResult's or a measurer's, the text pipeline's) goes through per run.
    pub fn draw_text_run(&mut self, text: &TextDraw, color: Color) {
        let moved = self.moved_text(text);
        self.canvas.draw_text(&moved, color);
    }

    /// The same painted with `brush` spread over `brush_area` (this scope's coordinates).
    pub fn draw_text_run_brush(&mut self, text: &TextDraw, brush: &Brush, brush_area: Rect) {
        let moved = self.moved_text(text);
        let area = brush_area.translate(self.origin);
        self.canvas.draw_text_brush(&moved, brush, area);
    }

    /// Its shadow: its glyphs in `color`, blurred by `blur_radius` px.
    pub fn draw_text_run_shadow(&mut self, text: &TextDraw, color: Color, blur_radius: f32) {
        let moved = self.moved_text(text);
        self.canvas.draw_text_shadow(&moved, color, blur_radius);
    }

    fn moved_text(&self, text: &TextDraw) -> TextDraw {
        let mut moved = text.clone();
        moved.baseline = moved.baseline + self.origin;
        moved
    }

    /// Its canvas (Compose's drawContext.canvas), its origin this scope's top left: what it saves, transforms and
    /// clips holds for what is drawn after it here, until a block it is in ends (translate, clip_rect,
    /// with_transform…) or the scope does, as Compose's restores it.
    pub fn canvas(&mut self) -> Canvas<'_> {
        self.canvas_state_mut();
        let state = self.canvas_state.get().expect("a canvas state");
        Canvas::of(&mut *self.canvas, state)
    }

    /// `block` given its canvas (Compose's drawIntoCanvas).
    pub fn draw_into_canvas(&mut self, block: impl FnOnce(&mut Canvas<'_>)) {
        let mut canvas = self.canvas();
        block(&mut canvas);
    }

    /// [`draw_into_canvas`](Self::draw_into_canvas) whose `block` suspends (Kotlin's inline drawIntoCanvas: its
    /// lambda, run in place, suspends with the coroutine it is called in).
    #[doc(hidden)]
    pub async fn draw_into_canvas_async(&mut self, block: impl AsyncFnOnce(&mut Canvas<'_>)) {
        let mut canvas = self.canvas();
        block(&mut canvas).await;
    }

    /// What it draws with and in (Compose's drawContext).
    pub fn draw_context(&mut self) -> DrawContext<'_, 'a> {
        DrawContext { scope: self }
    }
}

/// What a draw scope draws with and in (Compose's DrawContext): its canvas, size, density and layout direction.
pub struct DrawContext<'s, 'a> {
    scope: &'s mut DrawScope<'a>,
}

impl DrawContext<'_, '_> {
    pub fn canvas(&mut self) -> Canvas<'_> {
        self.scope.canvas()
    }
    pub fn size(&self) -> Size {
        self.scope.size
    }
    pub fn density(&self) -> Density {
        self.scope.density
    }
    pub fn layout_direction(&self) -> LayoutDirection {
        self.scope.layout_direction
    }
    /// The context drawn into.
    pub fn graphics_context(&mut self) -> &mut dyn GraphicsContext {
        &mut *self.scope.canvas
    }
}

// ---- Transforms -------------------------------------------------------------------------------------------------

/// What both scopes are: a draw scope, or one with content (their transforms give the block the scope it was).
pub(crate) trait AsDrawScope<'a> {
    fn draw_scope(&mut self) -> &mut DrawScope<'a>;
}

impl<'a> AsDrawScope<'a> for DrawScope<'a> {
    fn draw_scope(&mut self) -> &mut DrawScope<'a> {
        self
    }
}

impl<'a> AsDrawScope<'a> for ContentDrawScope<'a> {
    fn draw_scope(&mut self) -> &mut DrawScope<'a> {
        &mut self.scope
    }
}

/// What entering a transform of a block's drawing changed, for leaving it after the block: each transform is pushed
/// as the block starts and popped as it ends (what the block left saved on the canvas restored first), the block's
/// drawing between (plain, or suspending: the transforms' suspending twins).
enum Entered<'a> {
    /// Moved: the canvas's depth inside it.
    Translated(usize),
    /// A transform pushed (a clip, a matrix): the canvas's depth inside it.
    Clipped(usize),
    Transformed(usize),
    /// An inset scope: what the scope was.
    Inset {
        origin: Offset,
        size: Size,
        outer: CanvasSlot<'a>,
    },
}

fn enter_translation<'a, S: AsDrawScope<'a>>(s: &mut S, left: f32, top: f32) -> Entered<'a> {
    let ds = s.draw_scope();
    ds.canvas.push_translation(Offset::new(left, top)); // (the content too: draw_content inside it)
    Entered::Translated(ds.canvas_depth())
}

/// The block in a scope of its own, inset: its origin moved, its size less the insets, a canvas of its own (what it
/// leaves pushed popped as the block ends).
fn enter_inset<'a, S: AsDrawScope<'a>>(s: &mut S, left: f32, top: f32, right: f32, bottom: f32) -> Entered<'a> {
    let ds = s.draw_scope();
    let (origin, size) = (ds.origin, ds.size);
    let outer = std::mem::replace(&mut ds.canvas_state, CanvasSlot::Own(None));
    ds.origin = origin + Offset::new(left, top);
    ds.size = Size::new((size.width - left - right).max(0.0), (size.height - top - bottom).max(0.0));
    Entered::Inset { origin, size, outer }
}

fn enter_clip_rect<'a, S: AsDrawScope<'a>>(s: &mut S, rect: Rect, op: ClipOp) -> Entered<'a> {
    let ds = s.draw_scope();
    let r = sorted(rect.translate(ds.origin));
    match op {
        ClipOp::Intersect => ds.canvas.push_clip(r),
        ClipOp::Difference => ds.canvas.push_clip_round_rect(r, CornerRadii::Zero, op),
    }
    Entered::Clipped(ds.canvas_depth())
}

fn enter_clip_path<'a, S: AsDrawScope<'a>>(s: &mut S, path: &Path, op: ClipOp) -> Entered<'a> {
    let ds = s.draw_scope();
    let origin = ds.origin;
    ds.canvas.push_clip_path(path, origin, op);
    Entered::Clipped(ds.canvas_depth())
}

/// Through `matrix`, which is in the scope's coordinates (its top left the origin): what the block draws, its content
/// too, goes through it, not clipped to the area.
fn enter_transform<'a, S: AsDrawScope<'a>>(s: &mut S, matrix: &Matrix) -> Entered<'a> {
    let ds = s.draw_scope();
    let m = around(ds.origin, matrix);
    ds.canvas.push_transform(&m);
    Entered::Transformed(ds.canvas_depth())
}

/// Leaves what [`Entered`] says was entered, after the block.
fn leave<'a, S: AsDrawScope<'a>>(s: &mut S, entered: Entered<'a>) {
    let ds = s.draw_scope();
    match entered {
        Entered::Translated(depth) => {
            ds.restore_canvas_to(depth);
            ds.canvas.pop_translation();
        }
        Entered::Clipped(depth) => {
            ds.restore_canvas_to(depth);
            ds.canvas.pop_clip();
        }
        Entered::Transformed(depth) => {
            ds.restore_canvas_to(depth);
            ds.canvas.pop_transform();
        }
        Entered::Inset { origin, size, outer } => {
            ds.restore_canvas_to(0);
            ds.canvas_state = outer;
            ds.origin = origin;
            ds.size = size;
        }
    }
}

pub(crate) fn translate_in<'a, S: AsDrawScope<'a>>(s: &mut S, left: f32, top: f32, block: impl FnOnce(&mut S)) {
    let e = enter_translation(s, left, top);
    block(s);
    leave(s, e);
}

/// `block` through `matrix`, which is in the scope's coordinates (its top left the origin): what it draws, its
/// content too, goes through it, not clipped to the area.
pub(crate) fn transformed_in<'a, S: AsDrawScope<'a>>(s: &mut S, matrix: &Matrix, block: impl FnOnce(&mut S)) {
    let e = enter_transform(s, matrix);
    block(s);
    leave(s, e);
}

/// `block` drawn in what `entered` entered, left after it.
fn entered_in<'a, S: AsDrawScope<'a>>(s: &mut S, entered: Entered<'a>, block: impl FnOnce(&mut S)) {
    block(s);
    leave(s, entered);
}

/// [`entered_in`] whose `block` suspends: what the transforms' suspending twins draw (Kotlin's inline transforms:
/// their lambdas, run in place, suspend with the coroutine they are called in).
async fn entered_in_async<'a, S: AsDrawScope<'a>>(s: &mut S, entered: Entered<'a>, block: impl AsyncFnOnce(&mut S)) {
    block(s).await;
    leave(s, entered);
}

/// Turning `degrees` clockwise about `pivot`.
fn rotation(degrees: f32, pivot: Offset) -> Matrix {
    let mut m = Matrix::new(); // (each step before the one above it)
    m.translate(pivot.x, pivot.y, 0.0);
    m.rotate_z(degrees);
    m.translate(-pivot.x, -pivot.y, 0.0);
    m
}

/// Scaling about `pivot`.
fn scaling(sx: f32, sy: f32, pivot: Offset) -> Matrix {
    let mut m = Matrix::new();
    m.translate(pivot.x, pivot.y, 0.0);
    m.scale(sx, sy, 1.0);
    m.translate(-pivot.x, -pivot.y, 0.0);
    m
}

/// One step of a [`DrawTransform`].
#[derive(Clone, Debug)]
enum Step {
    Translate(f32, f32),
    Rotate(f32, Offset),
    Scale(f32, f32, Offset),
    Transform(Matrix),
    Inset(f32, f32, f32, f32),
    ClipRect(Rect, ClipOp),
    ClipPath(Path, ClipOp),
}

/// What `with_transform` does before the drawing, in order, each inside the one before (Compose's DrawTransform):
/// `d.with_transform(|t| { t.translate(10.0, 0.0); t.rotate(45.0) }, |d| …)`. Its size is the scope's as the
/// steps so far leave it (an inset makes it smaller); a pivot left out is its middle then.
pub struct DrawTransform {
    steps: Vec<Step>,
    size: Size,
}

#[named]
impl DrawTransform {
    pub fn size(&self) -> Size {
        self.size
    }
    pub fn center(&self) -> Offset {
        self.size.center()
    }
    pub fn translate(&mut self, #[default(0.0)] left: f32, #[default(0.0)] top: f32) {
        self.steps.push(Step::Translate(left, top));
    }
    /// Degrees, clockwise, about `pivot`.
    pub fn rotate(&mut self, degrees: f32, #[default(self.center())] pivot: Offset) {
        self.steps.push(Step::Rotate(degrees, pivot));
    }
    pub fn rotate_rad(&mut self, radians: f32, #[default(self.center())] pivot: Offset) {
        self.steps.push(Step::Rotate(radians.to_degrees(), pivot));
    }
    pub fn scale(&mut self, scale_x: f32, scale_y: f32, #[default(self.center())] pivot: Offset) {
        self.steps.push(Step::Scale(scale_x, scale_y, pivot));
    }
    /// The same both ways.
    pub fn scale_uniform(&mut self, scale: f32, #[default(self.center())] pivot: Offset) {
        self.steps.push(Step::Scale(scale, scale, pivot));
    }
    /// Through `matrix` (from the drawing's coordinates to the scope's, its top left the origin).
    pub fn transform(&mut self, matrix: Matrix) {
        self.steps.push(Step::Transform(matrix));
    }
    pub fn inset(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        self.steps.push(Step::Inset(left, top, right, bottom));
        self.size = Size::new((self.size.width - left - right).max(0.0), (self.size.height - top - bottom).max(0.0));
    }
    pub fn inset_hv(&mut self, #[default(0.0)] horizontal: f32, #[default(0.0)] vertical: f32) {
        self.inset(horizontal, vertical, horizontal, vertical);
    }
    pub fn inset_all(&mut self, inset: f32) {
        self.inset(inset, inset, inset, inset);
    }
    /// Within (or outside) the rect from (`left`, `top`) to (`right`, `bottom`) (the whole size by default).
    pub fn clip_rect(
        &mut self,
        #[default(0.0)] left: f32,
        #[default(0.0)] top: f32,
        #[default(self.size.width)] right: f32,
        #[default(self.size.height)] bottom: f32,
        #[default(ClipOp::Intersect)] clip_op: ClipOp,
    ) {
        self.steps.push(Step::ClipRect(Rect::new(left, top, right, bottom), clip_op));
    }
    pub fn clip_path(&mut self, path: Path, #[default(ClipOp::Intersect)] clip_op: ClipOp) {
        self.steps.push(Step::ClipPath(path, clip_op));
    }
}

/// The `…_with` forms of [`DrawTransform`]'s steps: every parameter given, for plain code.
impl DrawTransform {
    pub fn translate_with(&mut self, left: f32, top: f32) {
        self.steps.push(Step::Translate(left, top));
    }
    pub fn rotate_with(&mut self, degrees: f32, pivot: Offset) {
        self.steps.push(Step::Rotate(degrees, pivot));
    }
    pub fn rotate_rad_with(&mut self, radians: f32, pivot: Offset) {
        self.steps.push(Step::Rotate(radians.to_degrees(), pivot));
    }
    pub fn scale_with(&mut self, scale_x: f32, scale_y: f32, pivot: Offset) {
        self.steps.push(Step::Scale(scale_x, scale_y, pivot));
    }
    pub fn clip_rect_with(&mut self, left: f32, top: f32, right: f32, bottom: f32, clip_op: ClipOp) {
        self.steps.push(Step::ClipRect(Rect::new(left, top, right, bottom), clip_op));
    }
    pub fn clip_path_with(&mut self, path: Path, clip_op: ClipOp) {
        self.steps.push(Step::ClipPath(path, clip_op));
    }
}

/// Enters a step of a [`DrawTransform`].
fn enter_step<'a, S: AsDrawScope<'a>>(s: &mut S, step: &Step) -> Entered<'a> {
    match step {
        Step::Translate(l, t) => enter_translation(s, *l, *t),
        Step::Rotate(d, p) => enter_transform(s, &rotation(*d, *p)),
        Step::Scale(x, y, p) => enter_transform(s, &scaling(*x, *y, *p)),
        Step::Transform(m) => enter_transform(s, m),
        Step::Inset(l, t, r, b) => enter_inset(s, *l, *t, *r, *b),
        Step::ClipRect(r, op) => enter_clip_rect(s, *r, *op),
        Step::ClipPath(p, op) => enter_clip_path(s, p, *op),
    }
}

/// The steps `t` says entered, each inside the one before (the first outermost): the block goes innermost, then
/// [`leave_steps`] leaves them, the last first.
fn enter_steps<'a, S: AsDrawScope<'a>>(s: &mut S, t: &DrawTransform) -> Vec<Entered<'a>> {
    t.steps.iter().map(|step| enter_step(s, step)).collect()
}

fn leave_steps<'a, S: AsDrawScope<'a>>(s: &mut S, entered: Vec<Entered<'a>>) {
    for e in entered.into_iter().rev() {
        leave(s, e);
    }
}

fn with_transform_in<'a, S: AsDrawScope<'a>>(
    s: &mut S,
    transform: impl FnOnce(&mut DrawTransform),
    block: impl FnOnce(&mut S),
) {
    let mut t = DrawTransform { steps: Vec::new(), size: s.draw_scope().size };
    transform(&mut t);
    let entered = enter_steps(s, &t);
    block(s);
    leave_steps(s, entered);
}

/// [`with_transform_in`] whose lambdas suspend (Kotlin's inline withTransform takes both inline).
async fn with_transform_in_async<'a, S: AsDrawScope<'a>>(
    s: &mut S,
    transform: impl AsyncFnOnce(&mut DrawTransform),
    block: impl AsyncFnOnce(&mut S),
) {
    let mut t = DrawTransform { steps: Vec::new(), size: s.draw_scope().size };
    transform(&mut t).await;
    let entered = enter_steps(s, &t);
    block(s).await;
    leave_steps(s, entered);
}

/// The transforms of a scope, each giving its block the scope itself (a content scope's block can draw the content).
macro_rules! transforms {
    ($ty:ident) => {
        #[named]
        impl<'a> $ty<'a> {
            /// What `block` draws, its content too, moved by (`left`, `top`) (Compose's translate).
            pub fn translate(
                &mut self,
                #[default(0.0)] left: f32,
                #[default(0.0)] top: f32,
                block: impl FnOnce(&mut $ty<'a>),
            ) {
                translate_in(self, left, top, block)
            }

            /// [`translate`](Self::translate) with every parameter, for plain code.
            pub fn translate_with(&mut self, left: f32, top: f32, block: impl FnOnce(&mut $ty<'a>)) {
                translate_in(self, left, top, block)
            }

            /// `block` in a scope inset by each side (Compose's inset(left, top, right, bottom)): its origin moved,
            /// its size smaller.
            pub fn inset(&mut self, left: f32, top: f32, right: f32, bottom: f32, block: impl FnOnce(&mut $ty<'a>)) {
                let e = enter_inset(self, left, top, right, bottom);
                entered_in(self, e, block)
            }

            /// Inset by `horizontal` at the left and the right, `vertical` at the top and the bottom (Compose's
            /// inset(horizontal, vertical)).
            pub fn inset_hv(
                &mut self,
                #[default(0.0)] horizontal: f32,
                #[default(0.0)] vertical: f32,
                block: impl FnOnce(&mut $ty<'a>),
            ) {
                let e = enter_inset(self, horizontal, vertical, horizontal, vertical);
                entered_in(self, e, block)
            }

            /// Inset by `inset` all round (Compose's inset(inset)).
            pub fn inset_all(&mut self, inset: f32, block: impl FnOnce(&mut $ty<'a>)) {
                let e = enter_inset(self, inset, inset, inset, inset);
                entered_in(self, e, block)
            }

            /// What `block` draws, only inside the rect from (`left`, `top`) to (`right`, `bottom`) (sorted; the
            /// whole area by default), or outside it with ClipOp::Difference (Compose's clipRect).
            pub fn clip_rect(
                &mut self,
                #[default(0.0)] left: f32,
                #[default(0.0)] top: f32,
                #[default(self.size().width)] right: f32,
                #[default(self.size().height)] bottom: f32,
                #[default(ClipOp::Intersect)] clip_op: ClipOp,
                block: impl FnOnce(&mut $ty<'a>),
            ) {
                let e = enter_clip_rect(self, Rect::new(left, top, right, bottom), clip_op);
                entered_in(self, e, block)
            }

            /// [`clip_rect`](Self::clip_rect) with every parameter, for plain code.
            pub fn clip_rect_with(
                &mut self,
                left: f32,
                top: f32,
                right: f32,
                bottom: f32,
                clip_op: ClipOp,
                block: impl FnOnce(&mut $ty<'a>),
            ) {
                let e = enter_clip_rect(self, Rect::new(left, top, right, bottom), clip_op);
                entered_in(self, e, block)
            }

            /// What `block` draws, only inside `path` (or outside it: ClipOp::Difference), by its fill type, its
            /// edge anti-aliased (Compose's clipPath).
            pub fn clip_path(
                &mut self,
                path: &Path,
                #[default(ClipOp::Intersect)] clip_op: ClipOp,
                block: impl FnOnce(&mut $ty<'a>),
            ) {
                let e = enter_clip_path(self, path, clip_op);
                entered_in(self, e, block)
            }

            pub fn clip_path_with(&mut self, path: &Path, clip_op: ClipOp, block: impl FnOnce(&mut $ty<'a>)) {
                let e = enter_clip_path(self, path, clip_op);
                entered_in(self, e, block)
            }

            /// What `block` draws turned `degrees` clockwise about `pivot` (the middle by default) (Compose's
            /// rotate).
            pub fn rotate(
                &mut self,
                degrees: f32,
                #[default(self.center())] pivot: Offset,
                block: impl FnOnce(&mut $ty<'a>),
            ) {
                let e = enter_transform(self, &rotation(degrees, pivot));
                entered_in(self, e, block)
            }

            pub fn rotate_with(&mut self, degrees: f32, pivot: Offset, block: impl FnOnce(&mut $ty<'a>)) {
                let e = enter_transform(self, &rotation(degrees, pivot));
                entered_in(self, e, block)
            }

            /// The same in radians (Compose's rotateRad).
            pub fn rotate_rad(
                &mut self,
                radians: f32,
                #[default(self.center())] pivot: Offset,
                block: impl FnOnce(&mut $ty<'a>),
            ) {
                let e = enter_transform(self, &rotation(radians.to_degrees(), pivot));
                entered_in(self, e, block)
            }

            pub fn rotate_rad_with(&mut self, radians: f32, pivot: Offset, block: impl FnOnce(&mut $ty<'a>)) {
                let e = enter_transform(self, &rotation(radians.to_degrees(), pivot));
                entered_in(self, e, block)
            }

            /// What `block` draws scaled about `pivot` (the middle by default) (Compose's scale(scaleX, scaleY)).
            pub fn scale(
                &mut self,
                scale_x: f32,
                scale_y: f32,
                #[default(self.center())] pivot: Offset,
                block: impl FnOnce(&mut $ty<'a>),
            ) {
                let e = enter_transform(self, &scaling(scale_x, scale_y, pivot));
                entered_in(self, e, block)
            }

            pub fn scale_with(&mut self, scale_x: f32, scale_y: f32, pivot: Offset, block: impl FnOnce(&mut $ty<'a>)) {
                let e = enter_transform(self, &scaling(scale_x, scale_y, pivot));
                entered_in(self, e, block)
            }

            /// The same both ways (Compose's scale(scale)).
            pub fn scale_uniform(
                &mut self,
                scale: f32,
                #[default(self.center())] pivot: Offset,
                block: impl FnOnce(&mut $ty<'a>),
            ) {
                let e = enter_transform(self, &scaling(scale, scale, pivot));
                entered_in(self, e, block)
            }

            /// Several transforms at once (Compose's withTransform): `transform` says them, each inside the one
            /// before, `block` draws through them all.
            pub fn with_transform(
                &mut self,
                transform: impl FnOnce(&mut DrawTransform),
                block: impl FnOnce(&mut $ty<'a>),
            ) {
                with_transform_in(self, transform, block)
            }

            // The suspending twins of Compose's inline transforms (Kotlin's inline functions: their lambdas, run in
            // place, suspend with the coroutine they are called in): future-giving `fn`s, as `#[named]` takes them.
            // The `…_with` forms are Rust's: none.

            #[doc(hidden)]
            pub fn translate_async(
                &mut self,
                #[default(0.0)] left: f32,
                #[default(0.0)] top: f32,
                block: impl AsyncFnOnce(&mut $ty<'a>),
            ) -> impl Future<Output = ()> {
                async move {
                    let e = enter_translation(self, left, top);
                    entered_in_async(self, e, block).await
                }
            }

            #[doc(hidden)]
            pub fn inset_async(
                &mut self,
                left: f32,
                top: f32,
                right: f32,
                bottom: f32,
                block: impl AsyncFnOnce(&mut $ty<'a>),
            ) -> impl Future<Output = ()> {
                async move {
                    let e = enter_inset(self, left, top, right, bottom);
                    entered_in_async(self, e, block).await
                }
            }

            #[doc(hidden)]
            pub fn inset_hv_async(
                &mut self,
                #[default(0.0)] horizontal: f32,
                #[default(0.0)] vertical: f32,
                block: impl AsyncFnOnce(&mut $ty<'a>),
            ) -> impl Future<Output = ()> {
                async move {
                    let e = enter_inset(self, horizontal, vertical, horizontal, vertical);
                    entered_in_async(self, e, block).await
                }
            }

            #[doc(hidden)]
            pub fn inset_all_async(
                &mut self,
                inset: f32,
                block: impl AsyncFnOnce(&mut $ty<'a>),
            ) -> impl Future<Output = ()> {
                async move {
                    let e = enter_inset(self, inset, inset, inset, inset);
                    entered_in_async(self, e, block).await
                }
            }

            #[doc(hidden)]
            pub fn clip_rect_async(
                &mut self,
                #[default(0.0)] left: f32,
                #[default(0.0)] top: f32,
                #[default(self.size().width)] right: f32,
                #[default(self.size().height)] bottom: f32,
                #[default(ClipOp::Intersect)] clip_op: ClipOp,
                block: impl AsyncFnOnce(&mut $ty<'a>),
            ) -> impl Future<Output = ()> {
                async move {
                    let e = enter_clip_rect(self, Rect::new(left, top, right, bottom), clip_op);
                    entered_in_async(self, e, block).await
                }
            }

            #[doc(hidden)]
            pub fn clip_path_async(
                &mut self,
                path: &Path,
                #[default(ClipOp::Intersect)] clip_op: ClipOp,
                block: impl AsyncFnOnce(&mut $ty<'a>),
            ) -> impl Future<Output = ()> {
                async move {
                    let e = enter_clip_path(self, path, clip_op);
                    entered_in_async(self, e, block).await
                }
            }

            #[doc(hidden)]
            pub fn rotate_async(
                &mut self,
                degrees: f32,
                #[default(self.center())] pivot: Offset,
                block: impl AsyncFnOnce(&mut $ty<'a>),
            ) -> impl Future<Output = ()> {
                async move {
                    let e = enter_transform(self, &rotation(degrees, pivot));
                    entered_in_async(self, e, block).await
                }
            }

            #[doc(hidden)]
            pub fn rotate_rad_async(
                &mut self,
                radians: f32,
                #[default(self.center())] pivot: Offset,
                block: impl AsyncFnOnce(&mut $ty<'a>),
            ) -> impl Future<Output = ()> {
                async move {
                    let e = enter_transform(self, &rotation(radians.to_degrees(), pivot));
                    entered_in_async(self, e, block).await
                }
            }

            #[doc(hidden)]
            pub fn scale_async(
                &mut self,
                scale_x: f32,
                scale_y: f32,
                #[default(self.center())] pivot: Offset,
                block: impl AsyncFnOnce(&mut $ty<'a>),
            ) -> impl Future<Output = ()> {
                async move {
                    let e = enter_transform(self, &scaling(scale_x, scale_y, pivot));
                    entered_in_async(self, e, block).await
                }
            }

            #[doc(hidden)]
            pub fn scale_uniform_async(
                &mut self,
                scale: f32,
                #[default(self.center())] pivot: Offset,
                block: impl AsyncFnOnce(&mut $ty<'a>),
            ) -> impl Future<Output = ()> {
                async move {
                    let e = enter_transform(self, &scaling(scale, scale, pivot));
                    entered_in_async(self, e, block).await
                }
            }

            /// (Kotlin's withTransform takes both lambdas inline: either may suspend)
            #[doc(hidden)]
            pub fn with_transform_async(
                &mut self,
                transform: impl AsyncFnOnce(&mut DrawTransform),
                block: impl AsyncFnOnce(&mut $ty<'a>),
            ) -> impl Future<Output = ()> {
                async move { with_transform_in_async(self, transform, block).await }
            }
        }
    };
}

transforms!(DrawScope);
transforms!(ContentDrawScope);

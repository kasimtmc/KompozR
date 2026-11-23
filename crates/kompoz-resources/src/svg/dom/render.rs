//! An SVG document drawn (Skia's SkSVGRenderContext and its nodes' onRender): each element through its transform,
//! its presentation attributes inherited (SkSVGPresentationContext), a layer for a group's opacity (a shape's with
//! a fill or a stroke but not both fades its paint instead), its clip-path; shapes filled then stroked, in a
//! color or a gradient.

use super::values::{Affine, AspectRatio, Length, SvgColor, SvgPaint, Unit, view_box_transform};
use super::{Gradient, Kind, Node, Props, SvgDom, UseRef, Viewport};
use kompoz_graphics::{
    Color, CornerRadius, LinearGradientShader, Offset, Path, PathCommand, PathDirection, PathEffect, PathFillType,
    PathOperation, RadialGradientShader, Rect, RoundRect, StrokeCap, StrokeJoin, TileMode,
};
use kompoz_ui::draw::{Canvas, DrawScope, Paint, PaintingStyle};
use kompoz_ui::graphics_context::ClipOp;

/// How deep `use` references (and gradient and clip references) go before they are taken as a cycle.
const MAX_DEPTH: usize = 32;

/// Which way a percentage length goes (SkSVGLengthContext::LengthType).
#[derive(Clone, Copy)]
enum Dir {
    H,
    V,
    Other,
}

/// What percentages are of: the nearest viewport's size (SkSVGLengthContext).
#[derive(Clone, Copy, Debug)]
pub(super) struct Lctx {
    w: f32,
    h: f32,
}

impl Lctx {
    fn resolve(&self, l: Length, dir: Dir) -> f32 {
        if l.unit == Unit::Percentage {
            let size = match dir {
                Dir::H => self.w,
                Dir::V => self.h,
                Dir::Other => ((self.w * self.w + self.h * self.h) / 2.0).sqrt(),
            };
            return l.value * size / 100.0;
        }
        super::values::absolute_px(l).unwrap_or(0.0)
    }
}

/// The inherited properties where an element is drawn (SkSVGPresentationContext's fInherited), the initial ones
/// SVG's.
#[derive(Clone, Debug)]
pub(super) struct Inherited {
    fill: SvgPaint,
    fill_opacity: f32,
    fill_rule: PathFillType,
    clip_rule: PathFillType,
    stroke: SvgPaint,
    stroke_opacity: f32,
    stroke_width: Length,
    cap: StrokeCap,
    join: StrokeJoin,
    miter: f32,
    dash: Option<Vec<Length>>,
    dash_offset: Length,
    color: u32,
    hidden: bool,
}

impl Inherited {
    fn initial() -> Inherited {
        Inherited {
            fill: SvgPaint::Color(SvgColor::Argb(0xFF00_0000)),
            fill_opacity: 1.0,
            fill_rule: PathFillType::NonZero,
            clip_rule: PathFillType::NonZero,
            stroke: SvgPaint::None,
            stroke_opacity: 1.0,
            stroke_width: Length::number(1.0),
            cap: StrokeCap::Butt,
            join: StrokeJoin::Miter,
            miter: 4.0,
            dash: None,
            dash_offset: Length::number(0.0),
            color: 0xFF00_0000,
            hidden: false,
        }
    }

    /// These with an element's presentation attributes over them.
    fn with(&self, p: &Props) -> Inherited {
        let mut i = self.clone();
        if let Some(v) = &p.fill {
            i.fill = v.clone();
        }
        if let Some(v) = p.fill_opacity {
            i.fill_opacity = v;
        }
        if let Some(v) = p.fill_rule {
            i.fill_rule = v;
        }
        if let Some(v) = p.clip_rule {
            i.clip_rule = v;
        }
        if let Some(v) = &p.stroke {
            i.stroke = v.clone();
        }
        if let Some(v) = p.stroke_opacity {
            i.stroke_opacity = v;
        }
        if let Some(v) = p.stroke_width {
            i.stroke_width = v;
        }
        if let Some(v) = p.stroke_linecap {
            i.cap = v;
        }
        if let Some(v) = p.stroke_linejoin {
            i.join = v;
        }
        if let Some(v) = p.stroke_miterlimit {
            i.miter = v;
        }
        if let Some(v) = &p.stroke_dasharray {
            i.dash = v.clone();
        }
        if let Some(v) = p.stroke_dashoffset {
            i.dash_offset = v;
        }
        // (`color: currentColor` is the parent's)
        if let Some(SvgColor::Argb(c)) = p.color {
            i.color = c;
        }
        if let Some(v) = p.visibility_hidden {
            i.hidden = v;
        }
        i
    }

    fn resolve_color(&self, c: SvgColor) -> u32 {
        match c {
            SvgColor::Argb(c) => c,
            SvgColor::Current => self.color,
        }
    }
}

/// `argb` with its alpha times `opacity`.
fn color_of(argb: u32, opacity: f32) -> Color {
    let a = ((argb >> 24) as f32 / 255.0 * opacity).clamp(0.0, 1.0);
    let c = Color::from_argb(argb | 0xFF00_0000);
    Color::new(c.red(), c.green(), c.blue(), a)
}

/// A paint of `argb` faded by `opacity`.
fn solid_paint(argb: u32, opacity: f32) -> Paint {
    Paint { color: color_of(argb, opacity), ..Paint::default() }
}

/// The path's tight bounds (Skia's computeTightBounds: its curves' extremes, not their control points); none for
/// an empty path.
pub(super) fn tight_bounds(path: &Path) -> Option<Rect> {
    let (mut l, mut t, mut r, mut b) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut add = |p: Offset| {
        l = l.min(p.x);
        t = t.min(p.y);
        r = r.max(p.x);
        b = b.max(p.y);
    };
    let (mut current, mut start) = (Offset::Zero, Offset::Zero);
    for c in path.commands() {
        match *c {
            PathCommand::Move(p) => {
                add(p);
                current = p;
                start = p;
            }
            PathCommand::Line(p) => {
                add(p);
                current = p;
            }
            PathCommand::Quadratic(c1, p) => {
                add(p);
                let at = |s: f32| {
                    let u = 1.0 - s;
                    Offset::new(
                        u * u * current.x + 2.0 * u * s * c1.x + s * s * p.x,
                        u * u * current.y + 2.0 * u * s * c1.y + s * s * p.y,
                    )
                };
                for (a, m, z) in [(current.x, c1.x, p.x), (current.y, c1.y, p.y)] {
                    let d = a - 2.0 * m + z;
                    if d != 0.0 {
                        let s = (a - m) / d;
                        if s > 0.0 && s < 1.0 {
                            add(at(s));
                        }
                    }
                }
                current = p;
            }
            PathCommand::Conic(c1, p, w) => {
                add(p);
                for k in 1..32 {
                    let s = k as f32 / 32.0;
                    let u = 1.0 - s;
                    let (k0, k1, k2) = (u * u, 2.0 * w * u * s, s * s);
                    let den = k0 + k1 + k2;
                    add(Offset::new(
                        (k0 * current.x + k1 * c1.x + k2 * p.x) / den,
                        (k0 * current.y + k1 * c1.y + k2 * p.y) / den,
                    ));
                }
                current = p;
            }
            PathCommand::Cubic(c1, c2, p) => {
                add(p);
                let at = |s: f32| {
                    let u = 1.0 - s;
                    let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * s, 3.0 * u * s * s, s * s * s);
                    Offset::new(
                        k0 * current.x + k1 * c1.x + k2 * c2.x + k3 * p.x,
                        k0 * current.y + k1 * c1.y + k2 * c2.y + k3 * p.y,
                    )
                };
                for (p0, p1, p2, p3) in [(current.x, c1.x, c2.x, p.x), (current.y, c1.y, c2.y, p.y)] {
                    // B'(s)/3 = a s^2 + b s + c
                    let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
                    let bb = 2.0 * (p0 - 2.0 * p1 + p2);
                    let cc = p1 - p0;
                    let mut roots = [f32::NAN; 2];
                    if a.abs() < 1e-12 {
                        if bb != 0.0 {
                            roots[0] = -cc / bb;
                        }
                    } else {
                        let disc = bb * bb - 4.0 * a * cc;
                        if disc >= 0.0 {
                            let q = disc.sqrt();
                            roots = [(-bb + q) / (2.0 * a), (-bb - q) / (2.0 * a)];
                        }
                    }
                    for s in roots {
                        if s > 0.0 && s < 1.0 {
                            add(at(s));
                        }
                    }
                }
                current = p;
            }
            PathCommand::Close => current = start,
        }
    }
    (l <= r && t <= b).then(|| Rect::new(l, t, r, b))
}

/// The document drawn into `scope` at `width` by `height` (see [`SvgDom::render`]).
pub(super) fn render(dom: &SvgDom, scope: &mut DrawScope<'_>, width: f32, height: f32) {
    if !(width > 0.0 && height > 0.0) {
        return;
    }
    let mut canvas = scope.canvas();
    let mut r = Renderer {
        dom,
        canvas: &mut canvas,
        ctm: Affine::IDENTITY,
        device: Rect::new(0.0, 0.0, width, height),
        active: Vec::new(),
    };
    r.render_node(0, &Inherited::initial(), Lctx { w: width, h: height }, Via::Root(width, height));
}

/// How an element is reached: the root (drawn at a size), by a `use` (a `symbol` or an `svg` given its size), or
/// as a child.
#[derive(Clone, Copy)]
enum Via<'a> {
    Root(f32, f32),
    Use(&'a UseRef),
    Child,
}

struct Renderer<'d, 'c, 'x> {
    dom: &'d SvgDom,
    canvas: &'c mut Canvas<'x>,
    /// The transform from the user space drawn in now to where drawing started.
    ctm: Affine,
    /// What is drawn into, where drawing started.
    device: Rect,
    /// The elements being drawn through a `use` (and clip paths being resolved): not entered again.
    active: Vec<usize>,
}

impl Renderer<'_, '_, '_> {
    fn node(&self, id: usize) -> &Node {
        &self.dom.nodes[id]
    }

    fn save(&mut self) -> Affine {
        self.canvas.save();
        self.ctm
    }

    fn restore(&mut self, ctm: Affine) {
        self.canvas.restore();
        self.ctm = ctm;
    }

    fn concat(&mut self, m: Affine) {
        if !m.is_identity() {
            self.canvas.concat(&m.to_matrix());
            self.ctm = self.ctm.pre(m);
        }
    }

    /// A layer faded by `opacity` over what can be drawn into (SkCanvas::saveLayer with no bounds); false if
    /// nothing can be seen through the transform.
    fn push_layer(&mut self, opacity: f32) -> bool {
        let Some(back) = self.ctm.invert() else { return false };
        let mut paint = Paint::default();
        paint.set_alpha(opacity);
        self.canvas.save_layer(back.map_rect(self.device), &paint);
        true
    }

    /// Enters a viewport (SkSVGSVG::onPrepareToRender): clipped to it where `clip` (SVG's overflow: hidden), its
    /// viewBox mapped onto it; the lengths in it then of the viewBox's size (or its own). None: nothing is drawn
    /// in it (an empty size or viewBox).
    fn enter_viewport(&mut self, at: Rect, view_box: Option<Rect>, par: AspectRatio, clip: bool) -> Option<Lctx> {
        let (w, h) = (at.right - at.left, at.bottom - at.top);
        if !(w > 0.0 && h > 0.0) {
            return None;
        }
        if clip {
            self.canvas.clip_rect(at, ClipOp::Intersect);
        }
        let mut content = Affine::translate(at.left, at.top);
        let mut lctx = Lctx { w, h };
        if let Some(vb) = view_box {
            let (vw, vh) = (vb.right - vb.left, vb.bottom - vb.top);
            if !(vw > 0.0 && vh > 0.0) {
                return None;
            }
            content = content.pre(view_box_transform(vb, Rect::new(0.0, 0.0, w, h), par));
            lctx = Lctx { w: vw, h: vh };
        }
        self.concat(content);
        Some(lctx)
    }

    /// A viewport's rect where it is: its x and y (the root's ignored), its size (a `use`'s over its own; none:
    /// 100%).
    fn viewport_rect(vp: &Viewport, lctx: Lctx, size: Option<(Option<Length>, Option<Length>)>) -> Rect {
        let (uw, uh) = size.unwrap_or((None, None));
        let full = Length::percent(100.0);
        let x = lctx.resolve(vp.x, Dir::H);
        let y = lctx.resolve(vp.y, Dir::V);
        let w = lctx.resolve(uw.or(vp.width).unwrap_or(full), Dir::H);
        let h = lctx.resolve(uh.or(vp.height).unwrap_or(full), Dir::V);
        Rect::new(x, y, x + w, y + h)
    }

    /// Element `id` drawn (SkSVGNode::render): not if it is one that isn't drawn where it is, is `display: none`
    /// or (as Skia has it, with all in it) `visibility: hidden`.
    fn render_node(&mut self, id: usize, parent: &Inherited, lctx: Lctx, via: Via<'_>) {
        let dom = self.dom;
        let node = &dom.nodes[id];
        let drawn_here = match (&node.kind, via) {
            (Kind::Symbol(_), Via::Use(_)) => true,
            (k, _) => k.is_rendered(),
        };
        if !drawn_here || node.props.display_none || matches!(node.kind, Kind::Unsupported) {
            return;
        }
        let inh = parent.with(&node.props);
        if inh.hidden {
            return;
        }
        if node.props.opacity.is_some_and(|o| o <= 0.0) {
            return;
        }
        let saved = self.save();
        let mut lctx = lctx;
        let entered = match (&node.kind, via) {
            (Kind::Svg(vp), Via::Root(w, h)) => {
                self.enter_viewport(Rect::new(0.0, 0.0, w, h), vp.view_box, AspectRatio::NONE, false)
            }
            (Kind::Svg(vp) | Kind::Symbol(vp), Via::Use(u)) => {
                let at = Self::viewport_rect(vp, lctx, Some((u.width, u.height)));
                self.enter_viewport(at, vp.view_box, vp.par, true)
            }
            (Kind::Svg(vp), _) => self.enter_viewport(Self::viewport_rect(vp, lctx, None), vp.view_box, vp.par, true),
            _ => Some(lctx),
        };
        let Some(inner) = entered else {
            self.restore(saved);
            return;
        };
        lctx = inner;
        self.concat(node.transform);
        let mut deferred = 1.0;
        let mut layer = false;
        if let Some(o) = node.props.opacity
            && o < 1.0
        {
            let (fills, strokes) = (inh.fill != SvgPaint::None, inh.stroke != SvgPaint::None);
            if node.kind.is_shape() && (fills ^ strokes) {
                deferred = o; // (one draw: its paint faded, as Skia's applyOpacity does)
            } else if self.push_layer(o) {
                layer = true;
            } else {
                self.restore(saved);
                return;
            }
        }
        let clipped = match &node.props.clip_path {
            Some(cid) => match self.resolve_clip(cid, id, &inh, lctx) {
                None => true,
                Some(p) if p.is_empty() => false,
                Some(p) => {
                    self.canvas.clip_path(&p, ClipOp::Intersect);
                    true
                }
            },
            None => true,
        };
        if clipped {
            match &node.kind {
                Kind::Svg(_) | Kind::G | Kind::Symbol(_) => {
                    for &c in &node.children {
                        self.render_node(c, &inh, lctx, Via::Child);
                    }
                }
                Kind::Switch => {
                    let first = node.children.iter().copied().find(|&c| {
                        let n = &dom.nodes[c];
                        n.conditions_hold && n.kind.is_rendered()
                    });
                    if let Some(c) = first {
                        self.render_node(c, &inh, lctx, Via::Child);
                    }
                }
                Kind::Use(u) => self.render_use(u, &inh, lctx),
                _ => self.draw_shape(id, &inh, lctx, deferred),
            }
        }
        if layer {
            self.canvas.restore();
        }
        self.restore(saved);
    }

    /// The element a `use` refers to, moved by its x and y (SkSVGUse::onRender); not one being drawn through a
    /// `use` already.
    fn render_use(&mut self, u: &UseRef, inh: &Inherited, lctx: Lctx) {
        let Some(&target) = u.href.as_ref().and_then(|h| self.dom.ids.get(h)) else { return };
        if self.active.contains(&target) || self.active.len() >= MAX_DEPTH {
            return;
        }
        self.concat(Affine::translate(lctx.resolve(u.x, Dir::H), lctx.resolve(u.y, Dir::V)));
        self.active.push(target);
        self.render_node(target, inh, lctx, Via::Use(u));
        self.active.pop();
    }

    /// A shape's geometry in its user space (SkSVGShape's): none where it draws nothing (a rect or a circle of no
    /// size, a path of no data).
    fn shape_path(&self, id: usize, lctx: Lctx) -> Option<Path> {
        let r = |l: Length, d: Dir| lctx.resolve(l, d);
        let mut p = Path::new();
        match &self.node(id).kind {
            Kind::Path(path) => p = path.clone(),
            Kind::Rect { x, y, width, height, rx, ry } => {
                let (x, y, w, h) = (r(*x, Dir::H), r(*y, Dir::V), r(*width, Dir::H), r(*height, Dir::V));
                if !(w > 0.0 && h > 0.0) {
                    return None;
                }
                let rx = rx.map(|v| r(v, Dir::H)).filter(|v| *v >= 0.0);
                let ry = ry.map(|v| r(v, Dir::V)).filter(|v| *v >= 0.0);
                let (rx, ry) = match (rx, ry) {
                    (Some(a), Some(b)) => (a, b),
                    (Some(a), None) => (a, a),
                    (None, Some(b)) => (b, b),
                    (None, None) => (0.0, 0.0),
                };
                let (rx, ry) = (rx.min(w / 2.0), ry.min(h / 2.0));
                let rect = Rect::new(x, y, x + w, y + h);
                if rx > 0.0 && ry > 0.0 {
                    p.add_round_rect(
                        &RoundRect::from_rect_radius(rect, CornerRadius::new(rx, ry)),
                        PathDirection::Clockwise,
                    );
                } else {
                    p.add_rect(rect, PathDirection::Clockwise);
                }
            }
            Kind::Circle { cx, cy, r: radius } => {
                let (cx, cy, rr) = (r(*cx, Dir::H), r(*cy, Dir::V), r(*radius, Dir::Other));
                if rr.is_nan() || rr <= 0.0 {
                    return None;
                }
                p.add_oval(Rect::new(cx - rr, cy - rr, cx + rr, cy + rr), PathDirection::Clockwise);
            }
            Kind::Ellipse { cx, cy, rx, ry } => {
                let (cx, cy) = (r(*cx, Dir::H), r(*cy, Dir::V));
                let rx = rx.map(|v| r(v, Dir::H));
                let ry = ry.map(|v| r(v, Dir::V));
                let (rx, ry) = match (rx, ry) {
                    (Some(a), Some(b)) => (a, b),
                    (Some(a), None) => (a, a),
                    (None, Some(b)) => (b, b),
                    (None, None) => return None,
                };
                if !(rx > 0.0 && ry > 0.0) {
                    return None;
                }
                p.add_oval(Rect::new(cx - rx, cy - ry, cx + rx, cy + ry), PathDirection::Clockwise);
            }
            Kind::Line { x1, y1, x2, y2 } => {
                p.move_to(r(*x1, Dir::H), r(*y1, Dir::V));
                p.line_to(r(*x2, Dir::H), r(*y2, Dir::V));
            }
            Kind::Polyline(pts) | Kind::Polygon(pts) => {
                let (first, rest) = pts.split_first()?;
                p.move_to(first.x, first.y);
                for q in rest {
                    p.line_to(q.x, q.y);
                }
                if matches!(self.node(id).kind, Kind::Polygon(_)) {
                    p.close();
                }
            }
            _ => return None,
        }
        (!p.is_empty()).then_some(p)
    }

    /// A shape filled, then stroked (SkSVGShape::onRender), its paints faded by `deferred` (its opacity, where it
    /// takes it so).
    fn draw_shape(&mut self, id: usize, inh: &Inherited, lctx: Lctx, deferred: f32) {
        let Some(mut path) = self.shape_path(id, lctx) else { return };
        path.fill_type = inh.fill_rule;
        let bounds = tight_bounds(&path);
        if !matches!(self.node(id).kind, Kind::Line { .. })
            && let Some(paint) = self.paint(&inh.fill, inh.fill_opacity * deferred, inh, bounds, lctx)
        {
            self.canvas.draw_path(&path, &paint);
        }
        let width = lctx.resolve(inh.stroke_width, Dir::Other);
        if width > 0.0
            && let Some(mut paint) = self.paint(&inh.stroke, inh.stroke_opacity * deferred, inh, bounds, lctx)
        {
            paint.style = PaintingStyle::Stroke;
            paint.stroke_width = width;
            paint.stroke_cap = inh.cap;
            paint.stroke_join = inh.join;
            paint.stroke_miter_limit = inh.miter;
            paint.path_effect = Self::dash(inh, lctx);
            self.canvas.draw_path(&path, &paint);
        }
    }

    /// The stroke's dashes (SkSVGRenderContext's stroke paint): its lengths, given twice if there is an odd number
    /// of them, from its offset; none for none, a negative one or all 0 (a solid stroke).
    fn dash(inh: &Inherited, lctx: Lctx) -> Option<PathEffect> {
        let dashes = inh.dash.as_ref()?;
        let mut intervals: Vec<f32> = dashes.iter().map(|d| lctx.resolve(*d, Dir::Other)).collect();
        if intervals.is_empty() || intervals.iter().any(|v| *v < 0.0) || intervals.iter().sum::<f32>() <= 0.0 {
            return None;
        }
        if intervals.len() % 2 == 1 {
            intervals.extend_from_within(..);
        }
        Some(PathEffect::dash_path_effect(&intervals, lctx.resolve(inh.dash_offset, Dir::Other)))
    }

    /// The paint for a fill or a stroke (SkSVGRenderContext::commonPaint): none for `none`; a color; a gradient
    /// (over `bounds`, the shape's, where its units are the object's bounding box), or the fallback where the IRI
    /// isn't a gradient (none given: none); its alpha times `opacity`.
    fn paint(
        &self,
        paint: &SvgPaint,
        opacity: f32,
        inh: &Inherited,
        bounds: Option<Rect>,
        lctx: Lctx,
    ) -> Option<Paint> {
        let solid = |argb: u32| solid_paint(argb, opacity);
        match paint {
            SvgPaint::None => None,
            SvgPaint::Color(c) => Some(solid(inh.resolve_color(*c))),
            SvgPaint::Iri(id, fallback) => {
                match self.dom.ids.get(id).filter(|&&g| matches!(self.node(g).kind, Kind::Gradient(_))) {
                    Some(&g) => self.gradient_paint(g, opacity, bounds, lctx),
                    None => fallback.map(|c| solid(inh.resolve_color(c))),
                }
            }
        }
    }

    /// A gradient's attributes and stops, each taken from the first of it and the gradients its href leads to
    /// that has them (its coordinates only from gradients of its kind), and its stops' colors.
    fn resolve_gradient(&self, id: usize) -> (Gradient, Vec<(f32, u32)>) {
        let Kind::Gradient(first) = &self.node(id).kind else { unreachable!("a gradient") };
        let mut g = Gradient { radial: first.radial, ..Gradient::default() };
        let mut stops: Option<Vec<(f32, u32)>> = None;
        let mut chain = vec![id];
        let mut at = id;
        while let Kind::Gradient(h) = &self.node(at).kind {
            g.units_obb = g.units_obb.or(h.units_obb);
            g.transform = g.transform.or(h.transform);
            g.spread = g.spread.or(h.spread);
            if h.radial == g.radial {
                for (c, v) in g.coords.iter_mut().zip(h.coords) {
                    *c = c.or(v);
                }
            }
            if stops.is_none() {
                let own = self.stops_of(at);
                if !own.is_empty() {
                    stops = Some(own);
                }
            }
            match h.href.as_ref().and_then(|r| self.dom.ids.get(r)) {
                Some(&next) if !chain.contains(&next) && chain.len() < MAX_DEPTH => {
                    chain.push(next);
                    at = next;
                }
                _ => break,
            }
        }
        (g, stops.unwrap_or_default())
    }

    /// A gradient element's stops (SkSVGGradient::collectColorStops): each offset at least the one before, its
    /// color (`currentColor` the stop's `color`) faded by its stop-opacity.
    fn stops_of(&self, id: usize) -> Vec<(f32, u32)> {
        let gradient = self.node(id);
        let mut out: Vec<(f32, u32)> = Vec::new();
        for &c in &gradient.children {
            let n = self.node(c);
            let Kind::Stop { offset } = n.kind else { continue };
            let color = n.props.color.or(gradient.props.color);
            let current = match color {
                Some(SvgColor::Argb(a)) => a,
                _ => 0xFF00_0000,
            };
            let argb = match n.props.stop_color.unwrap_or(SvgColor::Argb(0xFF00_0000)) {
                SvgColor::Argb(a) => a,
                SvgColor::Current => current,
            };
            let alpha = ((argb >> 24) as f32 * n.props.stop_opacity.unwrap_or(1.0)).round() as u32;
            let offset = out.last().map_or(offset, |l| offset.max(l.0));
            out.push((offset, (argb & 0x00FF_FFFF) | alpha.min(255) << 24));
        }
        out
    }

    /// A gradient's paint (SkSVGGradient::onAsPaint): its shader through its gradientTransform, then (its units
    /// the object's bounding box) onto `bounds`; one stop (or ends at one place, or a radius of 0) its last color;
    /// no stops, or a bounding box of no size, nothing. A radial gradient's focal point isn't taken (KR's radial
    /// gradients have none): it goes from its center.
    fn gradient_paint(&self, id: usize, opacity: f32, bounds: Option<Rect>, lctx: Lctx) -> Option<Paint> {
        let (g, stops) = self.resolve_gradient(id);
        let last = stops.last()?.1;
        let solid = |argb: u32| solid_paint(argb, opacity);
        if stops.len() == 1 {
            return Some(solid(last));
        }
        let obb = g.units_obb.unwrap_or(true);
        let mut local = Affine::IDENTITY;
        let lctx = if obb {
            let b = bounds?;
            let (w, h) = (b.right - b.left, b.bottom - b.top);
            if !(w > 0.0 && h > 0.0) {
                return None;
            }
            local = Affine::translate(b.left, b.top).pre(Affine::scale(w, h));
            Lctx { w: 1.0, h: 1.0 }
        } else {
            lctx
        };
        local = local.pre(g.transform.unwrap_or(Affine::IDENTITY));
        local.invert()?;
        let tile = g.spread.unwrap_or(TileMode::Clamp);
        let colors: Vec<Color> = stops.iter().map(|s| Color::from_argb(s.1)).collect();
        let offsets: Vec<f32> = stops.iter().map(|s| s.0).collect();
        let pct = |v: f32| Length::percent(v);
        let shader = if g.radial {
            let [cx, cy, r, _, _] = g.coords;
            let center = Offset::new(
                lctx.resolve(cx.unwrap_or(pct(50.0)), Dir::H),
                lctx.resolve(cy.unwrap_or(pct(50.0)), Dir::V),
            );
            let r = lctx.resolve(r.unwrap_or(pct(50.0)), Dir::Other);
            if r < 0.0 {
                return None;
            }
            if r == 0.0 {
                return Some(solid(last));
            }
            RadialGradientShader(center, r, &colors, Some(&offsets), tile)
        } else {
            let [x1, y1, x2, y2, _] = g.coords;
            let from =
                Offset::new(lctx.resolve(x1.unwrap_or(pct(0.0)), Dir::H), lctx.resolve(y1.unwrap_or(pct(0.0)), Dir::V));
            let to = Offset::new(
                lctx.resolve(x2.unwrap_or(pct(100.0)), Dir::H),
                lctx.resolve(y2.unwrap_or(pct(0.0)), Dir::V),
            );
            if from == to {
                return Some(solid(last));
            }
            LinearGradientShader(from, to, &colors, Some(&offsets), tile)
        };
        let mut paint = Paint { shader: Some(shader.transformed(&local.to_matrix())), ..Paint::default() };
        paint.set_alpha(opacity);
        Some(paint)
    }

    /// A clip-path's path in the user space of element `id` (SkSVGClipPath::resolveClip): its children's paths
    /// (each by its clip-rule) unioned, through its transform, then (its units the object's bounding box) onto the
    /// element's bounding box, within its own clip-path. None: no clip (the IRI isn't a clipPath, or leads back to
    /// one being resolved); an empty path: all clipped.
    fn resolve_clip(&mut self, clip_id: &str, id: usize, inh: &Inherited, lctx: Lctx) -> Option<Path> {
        let dom = self.dom;
        let &cid = dom.ids.get(clip_id)?;
        let clip = &dom.nodes[cid];
        let Kind::ClipPath { obb } = clip.kind else { return None };
        if self.active.contains(&cid) || self.active.len() >= MAX_DEPTH {
            return None;
        }
        self.active.push(cid);
        let cinh = inh.with(&clip.props);
        let mut path = Path::new();
        for &c in &clip.children {
            let p = self.as_path(c, &cinh, lctx);
            path = union(path, p);
        }
        path.transform(&clip.transform.to_matrix());
        if obb {
            match self.object_bounding_box(id, lctx, 0) {
                Some(b) if b.right > b.left && b.bottom > b.top => {
                    let m = Affine::translate(b.left, b.top).pre(Affine::scale(b.right - b.left, b.bottom - b.top));
                    path.transform(&m.to_matrix());
                }
                _ => path = Path::new(),
            }
        }
        if let Some(own) = &clip.props.clip_path
            && let Some(p) = self.resolve_clip(own, id, inh, lctx)
        {
            path = intersect(path, &p);
        }
        self.active.pop();
        Some(path)
    }

    /// An element's geometry for a clip (SkSVGNode::asPath), in its parent's user space: a shape's by its
    /// clip-rule, a group's children's unioned, a `use`'s target's moved; within its own clip-path; nothing where
    /// it isn't displayed.
    fn as_path(&mut self, id: usize, parent: &Inherited, lctx: Lctx) -> Path {
        let dom = self.dom;
        let node = &dom.nodes[id];
        if node.props.display_none {
            return Path::new();
        }
        let inh = parent.with(&node.props);
        if inh.hidden {
            return Path::new();
        }
        let mut path = match &node.kind {
            k if k.is_shape() => match self.shape_path(id, lctx) {
                Some(mut p) => {
                    p.fill_type = inh.clip_rule;
                    p
                }
                None => Path::new(),
            },
            Kind::G => {
                let mut acc = Path::new();
                for &c in &node.children {
                    let p = self.as_path(c, &inh, lctx);
                    acc = union(acc, p);
                }
                acc
            }
            Kind::Use(u) => match u.href.as_ref().and_then(|h| dom.ids.get(h)) {
                Some(&t) if !self.active.contains(&t) && self.active.len() < MAX_DEPTH => {
                    self.active.push(t);
                    let mut p = self.as_path(t, &inh, lctx);
                    self.active.pop();
                    p.translate(Offset::new(lctx.resolve(u.x, Dir::H), lctx.resolve(u.y, Dir::V)));
                    p
                }
                _ => Path::new(),
            },
            _ => Path::new(),
        };
        if let Some(c) = &node.props.clip_path
            && let Some(clip) = self.resolve_clip(c, id, &inh, lctx)
        {
            path = intersect(path, &clip);
        }
        path.transform(&node.transform.to_matrix());
        path
    }

    /// An element's bounding box in its user space (SkSVGNode::objectBoundingBox): a shape's geometry's, a
    /// container's children's (through their transforms) together, a `use`'s target's moved; none where there is
    /// no geometry.
    fn object_bounding_box(&self, id: usize, lctx: Lctx, depth: usize) -> Option<Rect> {
        if depth > MAX_DEPTH {
            return None;
        }
        let node = self.node(id);
        match &node.kind {
            k if k.is_shape() => self.shape_path(id, lctx).as_ref().and_then(tight_bounds),
            Kind::G | Kind::Svg(_) | Kind::Switch | Kind::Symbol(_) => {
                let mut acc: Option<Rect> = None;
                for &c in &node.children {
                    let n = self.node(c);
                    if !n.kind.is_rendered() {
                        continue;
                    }
                    if let Some(b) = self.object_bounding_box(c, lctx, depth + 1) {
                        let b = n.transform.map_rect(b);
                        acc = Some(acc.map_or(b, |a| a.union(b)));
                    }
                }
                acc
            }
            Kind::Use(u) => {
                let &t = u.href.as_ref().and_then(|h| self.dom.ids.get(h))?;
                let b = self.object_bounding_box(t, lctx, depth + 1)?;
                let b = self.node(t).transform.map_rect(b);
                Some(b.translate(Offset::new(lctx.resolve(u.x, Dir::H), lctx.resolve(u.y, Dir::V))))
            }
            _ => None,
        }
    }
}

/// `a` and `b` together (Skia's kUnion_SkPathOp, each by its fill type); one of them where the other is empty.
fn union(a: Path, b: Path) -> Path {
    if a.is_empty() {
        b
    } else if b.is_empty() {
        a
    } else {
        Path::combine(PathOperation::Union, &a, &b)
    }
}

/// What is in both (Skia's kIntersect_SkPathOp).
fn intersect(a: Path, b: &Path) -> Path {
    if a.is_empty() || b.is_empty() { Path::new() } else { Path::combine(PathOperation::Intersect, &a, b) }
}

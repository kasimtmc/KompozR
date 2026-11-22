//! A shape built from lines and curves (Compose's Path, an Android Path: Skia's SkPath).

use super::curves::{self, Conic, NEARLY_ZERO};
use crate::geometry::{CornerRadius, Offset, Rect, RoundRect};
use crate::matrix::Matrix;

/// How a path's inside is worked out (Compose's PathFillType): where its outline winds round a point other than
/// zero times, or an odd number of times.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PathFillType {
    #[default]
    NonZero,
    EvenOdd,
}

/// Which way a shape added whole goes round (Compose's Path.Direction): its winding.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PathDirection {
    #[default]
    CounterClockwise,
    Clockwise,
}

/// One step of a path: a move, a line, a quadratic, a conic (a quadratic whose control point is weighed by its
/// weight: arcs and ovals, exactly), a cubic, or the close of a contour. Each holds its control points (as needed)
/// and its end point.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PathCommand {
    Move(Offset),
    Line(Offset),
    /// Its control point and end.
    Quadratic(Offset, Offset),
    /// Its control point, end and weight.
    Conic(Offset, Offset, f32),
    /// Its two control points and end.
    Cubic(Offset, Offset, Offset),
    Close,
}

impl PathCommand {
    /// Where it ends (none for a close).
    pub fn end(&self) -> Option<Offset> {
        match *self {
            PathCommand::Move(p)
            | PathCommand::Line(p)
            | PathCommand::Quadratic(_, p)
            | PathCommand::Conic(_, p, _)
            | PathCommand::Cubic(_, _, p) => Some(p),
            PathCommand::Close => None,
        }
    }

    /// Every point of it through `f`.
    pub(crate) fn map(&self, mut f: impl FnMut(Offset) -> Offset) -> PathCommand {
        match *self {
            PathCommand::Move(p) => PathCommand::Move(f(p)),
            PathCommand::Line(p) => PathCommand::Line(f(p)),
            PathCommand::Quadratic(c, p) => PathCommand::Quadratic(f(c), f(p)),
            PathCommand::Conic(c, p, w) => PathCommand::Conic(f(c), f(p), w),
            PathCommand::Cubic(c1, c2, p) => PathCommand::Cubic(f(c1), f(c2), f(p)),
            PathCommand::Close => PathCommand::Close,
        }
    }
}

/// As Compose's (an Android Path, which is Skia's SkPath): ovals, round rects and arcs are made of conics (quarter
/// circles exactly), a shape starts where Skia's does and goes round as asked, drawing after close() starts a new
/// contour from the last one's start.
#[derive(Clone, Debug, Default)]
pub struct Path {
    pub fill_type: PathFillType,
    commands: Vec<PathCommand>,
    current: Offset,
    start: Offset,
}

impl PartialEq for Path {
    /// The same fill type and commands.
    fn eq(&self, o: &Path) -> bool {
        self.fill_type == o.fill_type && self.commands == o.commands
    }
}

/// Skia's SK_ScalarRoot2Over2: a quarter circle's conic weight.
const ROOT2_OVER2: f32 = std::f32::consts::FRAC_1_SQRT_2;

impl Path {
    pub fn new() -> Path {
        Path::default()
    }

    /// A path of these commands as they are (a vector's).
    pub fn from_commands(commands: Vec<PathCommand>, fill_type: PathFillType) -> Path {
        let mut p = Path { fill_type, commands, current: Offset::Zero, start: Offset::Zero };
        for c in &p.commands {
            if let PathCommand::Move(to) = *c {
                p.start = to;
            }
            p.current = c.end().unwrap_or(p.start);
        }
        p
    }

    pub fn commands(&self) -> &[PathCommand] {
        &self.commands
    }

    /// One right after another takes its place.
    pub fn move_to(&mut self, x: f32, y: f32) {
        let to = Offset::new(x, y);
        match self.commands.last_mut() {
            Some(PathCommand::Move(p)) => *p = to,
            _ => self.commands.push(PathCommand::Move(to)),
        }
        self.current = to;
        self.start = to;
    }

    /// A move to the last start after close() (or to the origin), before drawing on.
    fn ensure_move(&mut self) {
        match self.commands.last() {
            None => self.move_to(0.0, 0.0),
            Some(PathCommand::Close) => self.move_to(self.start.x, self.start.y),
            _ => {}
        }
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        self.ensure_move();
        self.commands.push(PathCommand::Line(Offset::new(x, y)));
        self.current = Offset::new(x, y);
    }

    /// From the last point (after close(), from the contour's start).
    pub fn relative_move_to(&mut self, dx: f32, dy: f32) {
        self.move_to(self.current.x + dx, self.current.y + dy);
    }
    pub fn relative_line_to(&mut self, dx: f32, dy: f32) {
        self.line_to(self.current.x + dx, self.current.y + dy);
    }

    pub fn quadratic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        self.ensure_move();
        self.commands.push(PathCommand::Quadratic(Offset::new(x1, y1), Offset::new(x2, y2)));
        self.current = Offset::new(x2, y2);
    }

    /// A conic to (x2, y2), its control point (x1, y1) weighed by w (Skia's conicTo: 1 is a quadratic, cos(θ / 2) a
    /// circle's arc of θ; 0 or less a line).
    pub fn conic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, w: f32) {
        self.ensure_move();
        if w.is_nan() || w <= 0.0 {
            return self.line_to(x2, y2);
        }
        if w == 1.0 {
            return self.quadratic_to(x1, y1, x2, y2);
        }
        if !w.is_finite() {
            // (Skia's: two lines)
            self.line_to(x1, y1);
            return self.line_to(x2, y2);
        }
        self.commands.push(PathCommand::Conic(Offset::new(x1, y1), Offset::new(x2, y2), w));
        self.current = Offset::new(x2, y2);
    }

    pub fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
        self.ensure_move();
        self.commands.push(PathCommand::Cubic(Offset::new(x1, y1), Offset::new(x2, y2), Offset::new(x3, y3)));
        self.current = Offset::new(x3, y3);
    }

    pub fn relative_quadratic_to(&mut self, dx1: f32, dy1: f32, dx2: f32, dy2: f32) {
        let c = self.current;
        self.quadratic_to(c.x + dx1, c.y + dy1, c.x + dx2, c.y + dy2);
    }
    pub fn relative_conic_to(&mut self, dx1: f32, dy1: f32, dx2: f32, dy2: f32, w: f32) {
        let c = self.current;
        self.conic_to(c.x + dx1, c.y + dy1, c.x + dx2, c.y + dy2, w);
    }
    pub fn relative_cubic_to(&mut self, dx1: f32, dy1: f32, dx2: f32, dy2: f32, dx3: f32, dy3: f32) {
        let c = self.current;
        self.cubic_to(c.x + dx1, c.y + dy1, c.x + dx2, c.y + dy2, c.x + dx3, c.y + dy3);
    }

    pub fn close(&mut self) {
        if matches!(self.commands.last(), None | Some(PathCommand::Close)) {
            return;
        }
        self.commands.push(PathCommand::Close);
        self.current = self.start;
    }

    /// Emptied.
    pub fn reset(&mut self) {
        self.commands.clear();
        self.current = Offset::Zero;
        self.start = Offset::Zero;
    }

    /// Emptied, its memory kept (as reset() here).
    pub fn rewind(&mut self) {
        self.reset();
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Whether all its points are finite (Skia's SkPath::isFinite: a canvas draws nothing of one that isn't).
    pub fn is_finite(&self) -> bool {
        let finite = |p: Offset| p.x.is_finite() && p.y.is_finite();
        self.commands.iter().all(|c| match *c {
            PathCommand::Move(p) | PathCommand::Line(p) => finite(p),
            PathCommand::Quadratic(c1, p) | PathCommand::Conic(c1, p, _) => finite(c1) && finite(p),
            PathCommand::Cubic(c1, c2, p) => finite(c1) && finite(c2) && finite(p),
            PathCommand::Close => true,
        })
    }

    /// One contour that turns one way all round (Skia's: lines and points are convex too).
    pub fn is_convex(&self) -> bool {
        curves::convex(&self.commands)
    }

    /// The bounds of all its points, control points included (none: Rect::Zero).
    pub fn get_bounds(&self) -> Rect {
        if self.commands.is_empty() {
            return Rect::Zero;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        let mut add = |p: Offset| {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        };
        for c in &self.commands {
            match *c {
                PathCommand::Cubic(c1, c2, p) => {
                    add(c2);
                    add(c1);
                    add(p);
                }
                PathCommand::Quadratic(c1, p) | PathCommand::Conic(c1, p, _) => {
                    add(c1);
                    add(p);
                }
                PathCommand::Move(p) | PathCommand::Line(p) => add(p),
                PathCommand::Close => {}
            }
        }
        Rect::new(x0, y0, x1, y1)
    }

    /// Whether a point is inside, by its fill type, its curves exact (Skia's contains: open contours count as closed;
    /// a point on the outline is inside).
    pub fn contains(&self, p: Offset) -> bool {
        curves::contains(&self.commands, self.fill_type, p)
    }

    // ---- Whole shapes ----------------------------------------------------------------------------------------------

    /// A rect as a contour of its own from its top left, going round as `direction` says.
    pub fn add_rect(&mut self, rect: Rect, direction: PathDirection) {
        add_rect_from(self, rect, direction == PathDirection::Clockwise, 0);
    }

    /// An oval as a contour of its own from its right, four conics going round as `direction` says.
    pub fn add_oval(&mut self, oval: Rect, direction: PathDirection) {
        add_oval_from(self, oval, direction == PathDirection::Clockwise, 1);
    }

    /// A round rect as a contour of its own, its corners quarter ellipses (fitted as Skia fits them: all by one
    /// factor), from where its top left corner meets its left side (counterclockwise) or its bottom left corner
    /// meets it (clockwise). All corners round: an oval from its left; none: a rect.
    pub fn add_round_rect(&mut self, round_rect: &RoundRect, direction: PathDirection) {
        let clockwise = direction == PathDirection::Clockwise;
        add_round_rect_from(self, round_rect, clockwise, if clockwise { 6 } else { 7 });
    }

    /// Part of the oval in `oval` (Compose's arcTo): from `start_angle_degrees` (0 at the right, clockwise) through
    /// `sweep_angle_degrees` (a turn at most), joined to what is there by a line unless `force_move_to` (or it is the
    /// first thing) starts it on its own; as conics.
    ///
    /// As Skia's SkPathBuilder::arcTo: a lone point where it is one; the unit circle's arc from the start's direction
    /// to the end's as conics, scaled onto the oval; a sweep too small for those a line to its end.
    pub fn arc_to(&mut self, oval: Rect, start_angle_degrees: f32, sweep_angle_degrees: f32, force_move_to: bool) {
        if oval.width() < 0.0 || oval.height() < 0.0 {
            return;
        }
        let start_angle = start_angle_degrees % 360.0;
        let sweep = sweep_angle_degrees;
        let force_move_to = force_move_to || self.commands.is_empty();
        let cx = oval.left + oval.width() / 2.0;
        let cy = oval.top + oval.height() / 2.0;
        if sweep == 0.0 && (start_angle == 0.0 || start_angle == 360.0) {
            // A lone point (moving in and out of ovals).
            let q = Offset::new(oval.right, cy);
            return if force_move_to { self.move_to(q.x, q.y) } else { self.line_to(q.x, q.y) };
        }
        if oval.width() == 0.0 && oval.height() == 0.0 {
            let q = Offset::new(oval.right, oval.top);
            return if force_move_to { self.move_to(q.x, q.y) } else { self.line_to(q.x, q.y) };
        }
        const RADIANS: f32 = std::f32::consts::PI / 180.0;
        let start_rad = start_angle * RADIANS;
        let mut stop_rad = (start_angle + sweep) * RADIANS;
        let start_v = Offset::new(cos_snapped(start_rad), sin_snapped(start_rad));
        let mut stop_v = Offset::new(cos_snapped(stop_rad), sin_snapped(stop_rad));
        if start_v == stop_v {
            let sw = sweep.abs();
            if sw < 360.0 && sw > 359.0 {
                // Nearly a turn: nudged back until they differ.
                let delta = (1.0f32 / 512.0).copysign(sweep);
                loop {
                    stop_rad -= delta;
                    stop_v = Offset::new(cos_snapped(stop_rad), sin_snapped(stop_rad));
                    if start_v != stop_v {
                        break;
                    }
                }
            }
        }
        let clockwise = sweep > 0.0;
        let rx = oval.width() / 2.0;
        let ry = oval.height() / 2.0;
        if start_v == stop_v {
            // Too small a sweep: its end.
            let end = (start_angle + sweep) * RADIANS;
            return self.add_arc_point(Offset::new(cx + rx * end.cos(), cy + ry * end.sin()), force_move_to);
        }
        let conics = Conic::unit_arc(start_v, stop_v, clockwise, &[rx, 0.0, cx, 0.0, ry, cy]);
        if conics.is_empty() {
            return self.add_arc_point(Offset::new(rx * stop_v.x + cx, ry * stop_v.y + cy), force_move_to);
        }
        self.add_arc_point(conics[0].p[0], force_move_to);
        for k in &conics {
            self.conic_to(k.p[1].x, k.p[1].y, k.p[2].x, k.p[2].y, k.w);
        }
    }

    /// Where an arc starts: moved to, or joined by a line unless the pen is there already.
    fn add_arc_point(&mut self, q: Offset, force_move_to: bool) {
        if force_move_to {
            self.move_to(q.x, q.y);
        } else if (self.current.x - q.x).abs() > NEARLY_ZERO || (self.current.y - q.y).abs() > NEARLY_ZERO {
            self.line_to(q.x, q.y);
        }
    }

    pub fn arc_to_rad(&mut self, oval: Rect, start_angle_radians: f32, sweep_angle_radians: f32, force_move_to: bool) {
        let degrees = 180.0 / std::f32::consts::PI;
        self.arc_to(oval, start_angle_radians * degrees, sweep_angle_radians * degrees, force_move_to);
    }

    /// An arc as a contour of its own (a turn or more from a quarter's angle: the whole oval, from that quarter, as
    /// Skia's).
    pub fn add_arc(&mut self, oval: Rect, start_angle_degrees: f32, sweep_angle_degrees: f32) {
        if oval.width() <= 0.0 || oval.height() <= 0.0 || sweep_angle_degrees == 0.0 {
            return;
        }
        if sweep_angle_degrees.abs() >= 360.0 {
            let quarters = start_angle_degrees / 90.0;
            let whole = quarters.round();
            if (quarters - whole).abs() <= NEARLY_ZERO {
                let mut index = (whole + 1.0) % 4.0;
                if index < 0.0 {
                    index += 4.0;
                }
                return add_oval_from(self, oval, sweep_angle_degrees > 0.0, index as u32);
            }
        }
        self.arc_to(oval, start_angle_degrees, sweep_angle_degrees, true);
    }

    pub fn add_arc_rad(&mut self, oval: Rect, start_angle_radians: f32, sweep_angle_radians: f32) {
        let degrees = 180.0 / std::f32::consts::PI;
        self.add_arc(oval, start_angle_radians * degrees, sweep_angle_radians * degrees);
    }

    /// Its contours, moved by `offset`.
    pub fn add_path(&mut self, path: &Path, offset: Offset) {
        let mut moved = path.clone();
        moved.translate(offset);
        self.commands.extend_from_slice(&moved.commands);
        if !moved.commands.is_empty() {
            self.current = moved.current;
            self.start = moved.start;
        }
    }

    /// Every point moved.
    pub fn translate(&mut self, offset: Offset) {
        for c in &mut self.commands {
            *c = c.map(|p| p + offset);
        }
        self.current = self.current + offset;
        self.start = self.start + offset;
    }

    /// Every point through `matrix`'s plane (as Skia's SkPath::transform): affinely, every point; in perspective,
    /// first cut to what is in front of the eye, then quadratics and conics as conics (their weights through the map:
    /// exact), cubics in four (approximate), every point divided by its w.
    pub fn transform(&mut self, matrix: &Matrix) {
        if matrix.is_identity() || self.commands.is_empty() {
            return;
        }
        let v = &matrix.values;
        let m = [
            v[Matrix::ScaleX],
            v[Matrix::SkewX],
            v[Matrix::TranslateX],
            v[Matrix::SkewY],
            v[Matrix::ScaleY],
            v[Matrix::TranslateY],
            v[Matrix::Perspective0],
            v[Matrix::Perspective1],
            v[Matrix::Perspective2],
        ];
        let map = |p: Offset| {
            let w = m[6] * p.x + m[7] * p.y + m[8];
            Offset::new((m[0] * p.x + m[1] * p.y + m[2]) / w, (m[3] * p.x + m[4] * p.y + m[5]) / w)
        };
        let perspective = m[6] != 0.0 || m[7] != 0.0 || m[8] != 1.0;
        if !perspective {
            for c in &mut self.commands {
                *c = c.map(map);
            }
            self.current = map(self.current);
            self.start = map(self.start);
            return;
        }
        // Skia's kW0PlaneDistance.
        const W0_PLANE_DISTANCE: f32 = 1.0 / (1 << 14) as f32;
        let src = in_front(self, &m, W0_PLANE_DISTANCE);
        let fill = self.fill_type;
        self.reset();
        self.fill_type = fill;
        let mut current = Offset::Zero;
        let mut start = Offset::Zero;
        for c in &src.commands {
            match *c {
                PathCommand::Move(p) => {
                    let e = map(p);
                    self.move_to(e.x, e.y);
                    current = p;
                    start = p;
                    continue;
                }
                PathCommand::Line(p) => {
                    let e = map(p);
                    self.line_to(e.x, e.y);
                }
                PathCommand::Quadratic(c1, _) | PathCommand::Conic(c1, _, _) => {
                    let (p, w) = match *c {
                        PathCommand::Conic(_, p, w) => (p, w),
                        PathCommand::Quadratic(_, p) => (p, 1.0),
                        _ => unreachable!(),
                    };
                    let w = Conic::transformed_weight(&[current, c1, p], w, &m);
                    let control = map(c1);
                    let e = map(p);
                    self.conic_to(control.x, control.y, e.x, e.y, w);
                }
                PathCommand::Cubic(c1, c2, p) => {
                    let halves = curves::chop_cubic_at(&[current, c1, c2, p], 0.5);
                    for h in 0..2 {
                        let half = [halves[3 * h], halves[3 * h + 1], halves[3 * h + 2], halves[3 * h + 3]];
                        let quarters = curves::chop_cubic_at(&half, 0.5);
                        for q in 0..2 {
                            let k = &quarters[3 * q..];
                            let (a, b, e) = (map(k[1]), map(k[2]), map(k[3]));
                            self.cubic_to(a.x, a.y, b.x, b.y, e.x, e.y);
                        }
                    }
                }
                PathCommand::Close => {
                    self.close();
                    current = start;
                    continue;
                }
            }
            current = c.end().unwrap_or(current);
        }
    }

    /// Its outline added as a contour of its own (Compose's Path.addOutline): a rect's or rounded rect's
    /// (counterclockwise), or the path.
    pub fn add_outline(&mut self, outline: &super::outline::Outline) {
        use super::outline::Outline;
        match outline {
            Outline::Rectangle(rect) => self.add_rect(*rect, PathDirection::CounterClockwise),
            Outline::Rounded(round_rect) => self.add_round_rect(round_rect, PathDirection::CounterClockwise),
            Outline::Generic(path) => self.add_path(path, Offset::Zero),
        }
    }
}

// ---- Pieces of segments (for transforms' cuts and path operations) --------------------------------------------------

/// The point at t of segment `c` drawn from `from`.
pub(crate) fn piece_start(c: &PathCommand, from: Offset, t: f32) -> Offset {
    match *c {
        PathCommand::Line(p) => Offset::new(from.x + (p.x - from.x) * t, from.y + (p.y - from.y) * t),
        PathCommand::Quadratic(c1, p) => Conic::new([from, c1, p], 1.0).at(t),
        PathCommand::Conic(c1, p, w) => Conic::new([from, c1, p], w).at(t),
        PathCommand::Cubic(c1, c2, p) => curves::cubic_at(&[from, c1, c2, p], t),
        PathCommand::Move(p) => p,
        PathCommand::Close => Offset::Zero,
    }
}

/// The piece of segment `c` (from `from`) between t0 and t1, appended to `p` (which is at its start).
pub(crate) fn append_piece(p: &mut Path, c: &PathCommand, from: Offset, t0: f32, t1: f32) {
    match *c {
        PathCommand::Line(e) => {
            let e = Offset::new(from.x + (e.x - from.x) * t1, from.y + (e.y - from.y) * t1);
            p.line_to(e.x, e.y);
        }
        PathCommand::Quadratic(c1, e) | PathCommand::Conic(c1, e, _) => {
            let w = if let PathCommand::Conic(_, _, w) = *c { w } else { 1.0 };
            let k = Conic::new([from, c1, e], w).between(t0, t1);
            p.conic_to(k.p[1].x, k.p[1].y, k.p[2].x, k.p[2].y, k.w);
        }
        PathCommand::Cubic(c1, c2, e) => {
            let piece = curves::chop_cubic_between(&[from, c1, c2, e], t0, t1);
            p.cubic_to(piece[1].x, piece[1].y, piece[2].x, piece[2].y, piece[3].x, piece[3].y);
        }
        PathCommand::Move(_) | PathCommand::Close => {}
    }
}

/// Where along a segment from `from` a projective map's w crosses `edge`: the roots of w's Bernstein form (a conic's
/// numerator) in (0, 1), sorted.
fn w_crossings(c: &PathCommand, from: Offset, m: &[f32; 9], edge: f32) -> Vec<f32> {
    let v = |p: Offset| m[6] * p.x + m[7] * p.y + m[8] - edge;
    let mut out = Vec::new();
    let v0 = v(from);
    match *c {
        PathCommand::Line(p) => {
            let v_end = v(p);
            if (v0 < 0.0) != (v_end < 0.0) && v0 != v_end {
                out.push(v0 / (v0 - v_end));
            }
        }
        PathCommand::Quadratic(c1, p) | PathCommand::Conic(c1, p, _) => {
            let w = if let PathCommand::Conic(_, _, w) = *c { w } else { 1.0 };
            let v_end = v(p);
            let v1 = v(c1);
            out = curves::unit_quad_roots(v0 - 2.0 * w * v1 + v_end, 2.0 * (w * v1 - v0), v0);
        }
        PathCommand::Cubic(c1, c2, p) => {
            let (v1, v2, v_end) = (v(c1), v(c2), v(p));
            let at = |t: f32| {
                let u = 1.0 - t;
                v0 * u * u * u + 3.0 * v1 * u * u * t + 3.0 * v2 * u * t * t + v_end * t * t * t
            };
            const STEPS: i32 = 32;
            for k in 0..STEPS {
                let mut a = k as f32 / STEPS as f32;
                let mut b = (k + 1) as f32 / STEPS as f32;
                let fa = at(a);
                let fb = at(b);
                if (fa < 0.0) == (fb < 0.0) || fa == fb {
                    continue;
                }
                for _ in 0..40 {
                    let mid = (a + b) / 2.0;
                    if (at(mid) < 0.0) == (fa < 0.0) {
                        a = mid;
                    } else {
                        b = mid;
                    }
                }
                out.push((a + b) / 2.0);
            }
        }
        PathCommand::Move(_) | PathCommand::Close => {}
    }
    out.sort_by(f32::total_cmp);
    out
}

/// What of `path` is in front of the eye (w at least `edge`, Skia's kW0PlaneDistance): each segment cut where it
/// crosses, the pieces behind left out, the gaps bridged by lines.
fn in_front(path: &Path, m: &[f32; 9], edge: f32) -> Path {
    let mut out = Path { fill_type: path.fill_type, ..Path::default() };
    let mut current = Offset::Zero;
    let mut start = Offset::Zero;
    let mut begun = false; // the contour has a piece in front so far
    let front = |q: Offset| m[6] * q.x + m[7] * q.y + m[8] >= edge;
    for c in &path.commands {
        match *c {
            PathCommand::Move(p) => {
                current = p;
                start = p;
                begun = false;
            }
            PathCommand::Close => {
                if begun {
                    out.close();
                }
                current = start;
                begun = false;
            }
            _ => {
                let mut ts = w_crossings(c, current, m, edge);
                ts.insert(0, 0.0);
                ts.push(1.0);
                for k in 0..ts.len() - 1 {
                    let (t0, t1) = (ts[k], ts[k + 1]);
                    if t1 - t0 <= 0.0 || !front(piece_start(c, current, (t0 + t1) / 2.0)) {
                        continue;
                    }
                    let s0 = piece_start(c, current, t0);
                    if !begun {
                        out.move_to(s0.x, s0.y);
                    } else if t0 > 0.0 {
                        // Across what was behind (a piece from the start is where the pen is).
                        out.line_to(s0.x, s0.y);
                    }
                    begun = true;
                    append_piece(&mut out, c, current, t0, t1);
                }
                current = c.end().unwrap_or(current);
            }
        }
    }
    out
}

// ---- Skia's shapes ------------------------------------------------------------------------------------------------

/// The four corners (top left, top right, bottom right, bottom left) or an oval's four extremes (top, right, bottom,
/// left) from `start`, clockwise or not (Skia's SkPath_PointIterator).
struct Corners {
    points: [Offset; 4],
    at: usize,
    step: usize,
}

impl Corners {
    fn new(points: [Offset; 4], clockwise: bool, start: u32) -> Corners {
        Corners { points, at: (start % 4) as usize, step: if clockwise { 1 } else { 3 } }
    }
    fn current(&self) -> Offset {
        self.points[self.at]
    }
    fn next(&mut self) -> Offset {
        self.at = (self.at + self.step) % 4;
        self.points[self.at]
    }
}

fn corners_of(r: Rect) -> [Offset; 4] {
    [r.top_left(), r.top_right(), r.bottom_right(), r.bottom_left()]
}

fn extremes_of(r: Rect) -> [Offset; 4] {
    let cx = r.left + r.width() / 2.0;
    let cy = r.top + r.height() / 2.0;
    [Offset::new(cx, r.top), Offset::new(r.right, cy), Offset::new(cx, r.bottom), Offset::new(r.left, cy)]
}

fn add_rect_from(p: &mut Path, r: Rect, clockwise: bool, start: u32) {
    let mut c = Corners::new(corners_of(r), clockwise, start);
    let o = c.current();
    p.move_to(o.x, o.y);
    for _ in 0..3 {
        let o = c.next();
        p.line_to(o.x, o.y);
    }
    p.close();
}

fn add_oval_from(p: &mut Path, r: Rect, clockwise: bool, start: u32) {
    let mut oval = Corners::new(extremes_of(r), clockwise, start);
    let mut rect = Corners::new(corners_of(r), clockwise, start + if clockwise { 0 } else { 1 });
    let o = oval.current();
    p.move_to(o.x, o.y);
    for _ in 0..4 {
        let control = rect.next();
        let end = oval.next();
        p.conic_to(control.x, control.y, end.x, end.y, ROOT2_OVER2);
    }
    p.close();
}

/// What Skia's SkRRect is: a rect, an oval or a round rect.
#[derive(PartialEq)]
enum RRectKind {
    Rect,
    Oval,
    Rounded,
}

fn kind_of(f: &RoundRect) -> RRectKind {
    let r = [
        f.top_left_corner_radius,
        f.top_right_corner_radius,
        f.bottom_right_corner_radius,
        f.bottom_left_corner_radius,
    ];
    if r.iter().all(|c| c.x == 0.0 && c.y == 0.0) {
        return RRectKind::Rect;
    }
    if r.iter().all(|&c| c == r[0]) && r[0].x >= f.width() / 2.0 && r[0].y >= f.height() / 2.0 {
        return RRectKind::Oval;
    }
    RRectKind::Rounded
}

/// Skia's round rect contour from point `start` of its eight (where each corner meets each side, from the top left
/// one's top end, clockwise), clockwise or not: lines and quarter ellipses (conics), from a line (an odd start
/// clockwise or an even one counterclockwise starts with a conic instead).
fn add_rrect_from(p: &mut Path, f: &RoundRect, clockwise: bool, start: u32) {
    let (l, t, r, b) = (f.left, f.top, f.right, f.bottom);
    let (tl, tr, br, bl) = (
        f.top_left_corner_radius,
        f.top_right_corner_radius,
        f.bottom_right_corner_radius,
        f.bottom_left_corner_radius,
    );
    let pts = [
        Offset::new(l + tl.x, t),
        Offset::new(r - tr.x, t),
        Offset::new(r, t + tr.y),
        Offset::new(r, b - br.y),
        Offset::new(r - br.x, b),
        Offset::new(l + bl.x, b),
        Offset::new(l, b - bl.y),
        Offset::new(l, t + tl.y),
    ];
    let mut at = (start % 8) as usize;
    let step = if clockwise { 1 } else { 7 };
    let mut next = || {
        at = (at + step) % 8;
        pts[at]
    };
    let mut rect = Corners::new(corners_of(f.bounding_rect()), clockwise, start / 2 + if clockwise { 0 } else { 1 });
    let conic_first = (start & 1) == if clockwise { 1 } else { 0 };
    let o = pts[(start % 8) as usize];
    p.move_to(o.x, o.y);
    for i in 0..4 {
        if !conic_first {
            let o = next();
            p.line_to(o.x, o.y);
        }
        let control = rect.next();
        let end = next();
        p.conic_to(control.x, control.y, end.x, end.y, ROOT2_OVER2);
        if conic_first && i < 3 {
            // (the last line: close())
            let o = next();
            p.line_to(o.x, o.y);
        }
    }
    p.close();
}

/// Skia's SkRRect from a round rect's corner radii: a corner with either radius 0 or less square, the radii fitted
/// (all by the largest factor that fits every side's two corners in it, as Android draws a RoundRect).
fn skia_round_rect(rr: &RoundRect) -> RoundRect {
    let square = |c: CornerRadius| if c.x <= 0.0 || c.y <= 0.0 { CornerRadius::Zero } else { c };
    let mut f = RoundRect {
        top_left_corner_radius: square(rr.top_left_corner_radius),
        top_right_corner_radius: square(rr.top_right_corner_radius),
        bottom_right_corner_radius: square(rr.bottom_right_corner_radius),
        bottom_left_corner_radius: square(rr.bottom_left_corner_radius),
        ..*rr
    };
    let mut k = 1.0f32;
    let mut fit = |side: f32, a: f32, b: f32| {
        if a + b > side && a + b > 0.0 {
            k = k.min(side.max(0.0) / (a + b));
        }
    };
    let (w, h) = (f.width(), f.height());
    fit(w, f.top_left_corner_radius.x, f.top_right_corner_radius.x);
    fit(w, f.bottom_left_corner_radius.x, f.bottom_right_corner_radius.x);
    fit(h, f.top_left_corner_radius.y, f.bottom_left_corner_radius.y);
    fit(h, f.top_right_corner_radius.y, f.bottom_right_corner_radius.y);
    if k < 1.0 {
        for c in [
            &mut f.top_left_corner_radius,
            &mut f.top_right_corner_radius,
            &mut f.bottom_right_corner_radius,
            &mut f.bottom_left_corner_radius,
        ] {
            c.x *= k;
            c.y *= k;
        }
    }
    f
}

fn add_round_rect_from(p: &mut Path, rr: &RoundRect, clockwise: bool, start: u32) {
    let f = skia_round_rect(rr);
    match kind_of(&f) {
        // (SkPathPriv::SimplifyRRect)
        RRectKind::Rect => add_rect_from(p, f.bounding_rect(), clockwise, start.div_ceil(2)),
        RRectKind::Oval => add_oval_from(p, f.bounding_rect(), clockwise, start / 2),
        RRectKind::Rounded => add_rrect_from(p, &f, clockwise, start),
    }
}

fn sin_snapped(radians: f32) -> f32 {
    let v = radians.sin();
    if v.abs() <= NEARLY_ZERO { 0.0 } else { v }
}
fn cos_snapped(radians: f32) -> f32 {
    let v = radians.cos();
    if v.abs() <= NEARLY_ZERO { 0.0 } else { v }
}

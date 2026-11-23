//! Lengths along a path, as Skia's (SkContourMeasure and SkPathMeasure): Compose's PathMeasure.
//!
//! Skia's float arithmetic as written (Rust doesn't fuse multiply-adds), its SkGeometry forms where they differ from
//! the curves module's, so that lengths, positions and pieces come out as Skia's do.

use super::curves::{chop_cubic_at, chop_quad_at, cubic_tangent, quad_tangent};
use super::path::{Path, PathCommand};
use crate::geometry::Offset;

pub(super) fn finite(p: Offset) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// SkScalarInterp.
pub(super) fn interp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// SkPoint::Length: in double where the float squares overflow.
pub(super) fn length(dx: f32, dy: f32) -> f32 {
    let mag2 = dx * dx + dy * dy;
    if mag2.is_finite() {
        return mag2.sqrt();
    }
    let (xx, yy) = (dx as f64, dy as f64);
    (xx * xx + yy * yy).sqrt() as f32
}

pub(super) fn distance(a: Offset, b: Offset) -> f32 {
    length(b.x - a.x, b.y - a.y)
}

/// SkPoint::setLength(x, y, 1): the unit vector, (0, 0) if there is none.
fn normalized(x: f32, y: f32) -> Offset {
    let (xx, yy) = (x as f64, y as f64);
    let scale = 1.0 / (xx * xx + yy * yy).sqrt();
    let x = (x as f64 * scale) as f32;
    let y = (y as f64 * scale) as f32;
    if !x.is_finite() || !y.is_finite() || (x == 0.0 && y == 0.0) {
        return Offset::Zero;
    }
    Offset::new(x, y)
}

// ---- Skia's SkGeometry forms ---------------------------------------------------------------------------------------

/// SkQuadCoeff's evaluation.
fn eval_quad(p: &[Offset; 3], t: f32) -> Offset {
    let c = p[0];
    let b = (p[1] - c) * 2.0;
    let a = p[2] - p[1] * 2.0 + c;
    Offset::new((a.x * t + b.x) * t + c.x, (a.y * t + b.y) * t + c.y)
}

/// SkCubicCoeff's evaluation.
fn eval_cubic(p: &[Offset; 4], t: f32) -> Offset {
    let a = p[3] + (p[1] - p[2]) * 3.0 - p[0];
    let b = (p[2] - p[1] * 2.0 + p[0]) * 3.0;
    let c = (p[1] - p[0]) * 3.0;
    let d = p[0];
    Offset::new(((a.x * t + b.x) * t + c.x) * t + d.x, ((a.y * t + b.y) * t + c.y) * t + d.y)
}

/// SkConic, evaluated and chopped as Skia does.
#[derive(Clone, Copy, Debug)]
pub(super) struct SkConic {
    pub pts: [Offset; 3],
    pub w: f32,
}

impl SkConic {
    /// (SkConicCoeff) Its numerator's and denominator's quadratics at t.
    fn coeffs(&self, t: f32) -> (Offset, f32) {
        let p1w = self.pts[1] * self.w;
        let c = self.pts[0];
        let a = self.pts[2] - p1w * 2.0 + c;
        let b = (p1w - c) * 2.0;
        let numer = Offset::new((a.x * t + b.x) * t + c.x, (a.y * t + b.y) * t + c.y);
        let dc = 1.0f32;
        let db = 2.0 * (self.w - dc);
        let da = 0.0 - db;
        (numer, (da * t + db) * t + dc)
    }

    pub fn eval_at(&self, t: f32) -> Offset {
        let (n, d) = self.coeffs(t);
        Offset::new(n.x / d, n.y / d)
    }

    fn tangent_at(&self, t: f32) -> Offset {
        let p = &self.pts;
        if (t == 0.0 && p[0] == p[1]) || (t == 1.0 && p[1] == p[2]) {
            return p[2] - p[0];
        }
        let p20 = p[2] - p[0];
        let p10 = p[1] - p[0];
        let c = p10 * self.w;
        let a = p20 * self.w - p20;
        let b = p20 - c - c;
        Offset::new((a.x * t + b.x) * t + c.x, (a.y * t + b.y) * t + c.y)
    }

    /// Cut at t in two (None if that is not finite).
    fn chop_at(&self, t: f32) -> Option<[SkConic; 2]> {
        let p = &self.pts;
        let w = self.w;
        let tmp = [[p[0].x, p[0].y, 1.0], [p[1].x * w, p[1].y * w, w], [p[2].x, p[2].y, 1.0]];
        let mut out = [[0.0f32; 3]; 3];
        for k in 0..3 {
            // (p3d_interp)
            let ab = interp(tmp[0][k], tmp[1][k], t);
            let bc = interp(tmp[1][k], tmp[2][k], t);
            out[0][k] = ab;
            out[1][k] = interp(ab, bc, t);
            out[2][k] = bc;
        }
        let down = |v: [f32; 3]| Offset::new(v[0] / v[2], v[1] / v[2]);
        let root = out[1][2].sqrt();
        let first = SkConic { pts: [p[0], down(out[0]), down(out[1])], w: out[0][2] / root };
        let second = SkConic { pts: [first.pts[2], down(out[2]), p[2]], w: out[2][2] / root };
        for c in [&first, &second] {
            if !finite(c.pts[0]) || !finite(c.pts[1]) || !finite(c.pts[2]) || !c.w.is_finite() {
                return None;
            }
        }
        Some([first, second])
    }

    /// The piece from t1 to t2.
    fn chop_between(&self, t1: f32, t2: f32) -> SkConic {
        if t1 == 0.0 || t2 == 1.0 {
            if t1 == 0.0 && t2 == 1.0 {
                return *self;
            }
            if let Some(pair) = self.chop_at(if t1 != 0.0 { t1 } else { t2 }) {
                return pair[if t1 != 0.0 { 1 } else { 0 }];
            }
        }
        let (a_xy, a_z) = self.coeffs(t1);
        let (d_xy, d_z) = self.coeffs((t1 + t2) / 2.0);
        let (c_xy, c_z) = self.coeffs(t2);
        let b_xy = d_xy * 2.0 - (a_xy + c_xy) * 0.5;
        let b_z = d_z * 2.0 - (a_z + c_z) * 0.5;
        SkConic {
            pts: [
                Offset::new(a_xy.x / a_z, a_xy.y / a_z),
                Offset::new(b_xy.x / b_z, b_xy.y / b_z),
                Offset::new(c_xy.x / c_z, c_xy.y / c_z),
            ],
            w: b_z / (a_z * c_z).sqrt(),
        }
    }
}

/// The last point of a path (SkPath::getLastPt), if it has one.
fn last_point(path: &Path) -> Option<Offset> {
    path.commands().iter().rev().find_map(|c| c.end())
}

// ---- Skia's SkContourMeasure --------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum SegType {
    Line,
    Quad,
    Cubic,
    Conic,
}

const MAX_T_VALUE: u32 = 0x3FFF_FFFF;

fn t_value(t: u32) -> f32 {
    t as f32 * (1.0 / MAX_T_VALUE as f32)
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    distance: f32, // total distance up to this point
    pt_index: usize,
    t_value: u32,
    kind: SegType,
}

impl Segment {
    fn scalar_t(&self) -> f32 {
        t_value(self.t_value)
    }
}

/// One contour's measure (SkContourMeasure): its pieces' distances, and its points (a conic's weight in its first
/// point's x, then its two points).
#[derive(Clone, Debug, Default)]
pub(crate) struct ContourMeasure {
    segments: Vec<Segment>,
    pts: Vec<Offset>,
    pub length: f32,
    pub closed: bool,
}

impl ContourMeasure {
    /// The segment `distance` (within 0..length) falls in, and the t there.
    fn distance_to_segment(&self, distance: f32) -> (usize, f32) {
        let base = &self.segments;
        let count = base.len();
        // (SkTKSearch)
        let (mut lo, mut hi) = (0, count - 1);
        while lo < hi {
            let mid = (hi + lo) >> 1;
            if base[mid].distance < distance {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let mut index = hi;
        if base[hi].distance < distance {
            index = hi + 1;
        }
        if index >= count {
            index = count - 1; // (Skia's ~index xor gives the insertion point)
        }
        let seg = &base[index];
        let (mut start_t, mut start_d) = (0.0, 0.0);
        if index > 0 {
            start_d = base[index - 1].distance;
            if base[index - 1].pt_index == seg.pt_index {
                start_t = base[index - 1].scalar_t();
            }
        }
        (index, start_t + (seg.scalar_t() - start_t) * (distance - start_d) / (seg.distance - start_d))
    }

    fn quad_pts(&self, i: usize) -> [Offset; 3] {
        [self.pts[i], self.pts[i + 1], self.pts[i + 2]]
    }
    fn cubic_pts(&self, i: usize) -> [Offset; 4] {
        [self.pts[i], self.pts[i + 1], self.pts[i + 2], self.pts[i + 3]]
    }
    fn conic(&self, i: usize) -> SkConic {
        SkConic { pts: [self.pts[i], self.pts[i + 2], self.pts[i + 3]], w: self.pts[i + 1].x }
    }

    fn position(&self, seg: &Segment, t: f32) -> Offset {
        let i = seg.pt_index;
        match seg.kind {
            SegType::Line => {
                let (a, b) = (self.pts[i], self.pts[i + 1]);
                Offset::new(interp(a.x, b.x, t), interp(a.y, b.y, t))
            }
            SegType::Quad => eval_quad(&self.quad_pts(i), t),
            SegType::Conic => self.conic(i).eval_at(t),
            SegType::Cubic => eval_cubic(&self.cubic_pts(i), t),
        }
    }

    fn tangent(&self, seg: &Segment, t: f32) -> Offset {
        let i = seg.pt_index;
        let v = match seg.kind {
            SegType::Line => self.pts[i + 1] - self.pts[i],
            SegType::Quad => quad_tangent(&self.quad_pts(i), t),
            SegType::Conic => self.conic(i).tangent_at(t),
            SegType::Cubic => cubic_tangent(&self.cubic_pts(i), t),
        };
        normalized(v.x, v.y)
    }

    /// Where `distance` (kept within 0..length) lands and the unit vector the contour goes there (None for NaN).
    pub(super) fn get_pos_tan(&self, distance: f32) -> Option<(Offset, Offset)> {
        if distance.is_nan() {
            return None;
        }
        let distance = if distance < 0.0 {
            0.0
        } else if distance > self.length {
            self.length
        } else {
            distance
        };
        let (index, t) = self.distance_to_segment(distance);
        if t.is_nan() {
            return None;
        }
        let seg = &self.segments[index];
        Some((self.position(seg, t), self.tangent(seg, t)))
    }

    /// (SkContourMeasure_segTo) The piece of a segment's curve from start_t to stop_t.
    fn seg_to(&self, seg: &Segment, start_t: f32, stop_t: f32, dst: &mut Path) {
        let i = seg.pt_index;
        if start_t == stop_t {
            if let Some(last) = last_point(dst) {
                dst.line_to(last.x, last.y); // a zero-length dash: caps for it
            }
            return;
        }
        match seg.kind {
            SegType::Line => {
                let (a, b) = (self.pts[i], self.pts[i + 1]);
                if stop_t == 1.0 {
                    dst.line_to(b.x, b.y);
                } else {
                    dst.line_to(interp(a.x, b.x, stop_t), interp(a.y, b.y, stop_t));
                }
            }
            SegType::Quad => {
                let p = self.quad_pts(i);
                if start_t == 0.0 {
                    if stop_t == 1.0 {
                        dst.quadratic_to(p[1].x, p[1].y, p[2].x, p[2].y);
                    } else {
                        let tmp0 = chop_quad_at(&p, stop_t);
                        dst.quadratic_to(tmp0[1].x, tmp0[1].y, tmp0[2].x, tmp0[2].y);
                    }
                } else {
                    let tmp0 = chop_quad_at(&p, start_t);
                    if stop_t == 1.0 {
                        dst.quadratic_to(tmp0[3].x, tmp0[3].y, tmp0[4].x, tmp0[4].y);
                    } else {
                        let tmp1 = chop_quad_at(&[tmp0[2], tmp0[3], tmp0[4]], (stop_t - start_t) / (1.0 - start_t));
                        dst.quadratic_to(tmp1[1].x, tmp1[1].y, tmp1[2].x, tmp1[2].y);
                    }
                }
            }
            SegType::Conic => {
                let conic = self.conic(i);
                let add =
                    |dst: &mut Path, k: &SkConic| dst.conic_to(k.pts[1].x, k.pts[1].y, k.pts[2].x, k.pts[2].y, k.w);
                if start_t == 0.0 {
                    if stop_t == 1.0 {
                        add(dst, &conic);
                    } else if let Some(tmp) = conic.chop_at(stop_t) {
                        add(dst, &tmp[0]);
                    }
                } else if stop_t == 1.0 {
                    if let Some(tmp) = conic.chop_at(start_t) {
                        add(dst, &tmp[1]);
                    }
                } else {
                    add(dst, &conic.chop_between(start_t, stop_t));
                }
            }
            SegType::Cubic => {
                let p = self.cubic_pts(i);
                if start_t == 0.0 {
                    if stop_t == 1.0 {
                        dst.cubic_to(p[1].x, p[1].y, p[2].x, p[2].y, p[3].x, p[3].y);
                    } else {
                        let tmp0 = chop_cubic_at(&p, stop_t);
                        dst.cubic_to(tmp0[1].x, tmp0[1].y, tmp0[2].x, tmp0[2].y, tmp0[3].x, tmp0[3].y);
                    }
                } else {
                    let tmp0 = chop_cubic_at(&p, start_t);
                    if stop_t == 1.0 {
                        dst.cubic_to(tmp0[4].x, tmp0[4].y, tmp0[5].x, tmp0[5].y, tmp0[6].x, tmp0[6].y);
                    } else {
                        let tail = [tmp0[3], tmp0[4], tmp0[5], tmp0[6]];
                        let tmp1 = chop_cubic_at(&tail, (stop_t - start_t) / (1.0 - start_t));
                        dst.cubic_to(tmp1[1].x, tmp1[1].y, tmp1[2].x, tmp1[2].y, tmp1[3].x, tmp1[3].y);
                    }
                }
            }
        }
    }

    /// Adds the piece from start_d to stop_d (kept within 0..length) to `dst`: false if start_d isn't at most
    /// stop_d.
    pub(super) fn get_segment(
        &self,
        mut start_d: f32,
        mut stop_d: f32,
        dst: &mut Path,
        start_with_move_to: bool,
    ) -> bool {
        if start_d < 0.0 {
            start_d = 0.0;
        }
        if stop_d > self.length {
            stop_d = self.length;
        }
        if start_d.is_nan() || stop_d.is_nan() || start_d > stop_d || self.segments.is_empty() {
            return false; // (NaNs too)
        }
        let (mut seg, mut start_t) = self.distance_to_segment(start_d);
        if !start_t.is_finite() {
            return false;
        }
        let (stop_seg, stop_t) = self.distance_to_segment(stop_d);
        if !stop_t.is_finite() {
            return false;
        }
        if start_with_move_to {
            let p = self.position(&self.segments[seg], start_t);
            dst.move_to(p.x, p.y);
        }
        let segments = &self.segments;
        if segments[seg].pt_index == segments[stop_seg].pt_index {
            self.seg_to(&segments[seg], start_t, stop_t, dst);
        } else {
            loop {
                self.seg_to(&segments[seg], start_t, 1.0, dst);
                // (Segment::Next: the first segment of the next piece of the contour)
                let at = segments[seg].pt_index;
                seg += 1;
                while segments[seg].pt_index == at {
                    seg += 1;
                }
                start_t = 0.0;
                if segments[seg].pt_index >= segments[stop_seg].pt_index {
                    break;
                }
            }
            self.seg_to(&segments[seg], 0.0, stop_t, dst);
        }
        true
    }
}

/// CHEAP_DIST_LIMIT: how far a piece may be from a line, at one pixel a unit.
const CHEAP_DIST_LIMIT: f32 = 0.5;
const MAX_RECURSION_DEPTH: u32 = 8;

fn tspan_big_enough(tspan: u32) -> bool {
    (tspan >> 10) != 0
}

fn quad_too_curvy(p: &[Offset; 3], tolerance: f32) -> bool {
    let dx = (p[1].x / 2.0) - (((p[0].x + p[2].x) / 2.0) / 2.0);
    let dy = (p[1].y / 2.0) - (((p[0].y + p[2].y) / 2.0) / 2.0);
    dx.abs().max(dy.abs()) > tolerance
}

fn conic_too_curvy(first: Offset, mid: Offset, last: Offset, tolerance: f32) -> bool {
    let ends = (first + last) * 0.5;
    let d = mid - ends;
    d.x.abs().max(d.y.abs()) > tolerance
}

fn cheap_dist_exceeds_limit(p: Offset, x: f32, y: f32, tolerance: f32) -> bool {
    (x - p.x).abs().max((y - p.y).abs()) > tolerance
}

fn cubic_too_curvy(p: &[Offset; 4], tolerance: f32) -> bool {
    cheap_dist_exceeds_limit(p[1], interp(p[0].x, p[3].x, 1.0 / 3.0), interp(p[0].y, p[3].y, 1.0 / 3.0), tolerance)
        || cheap_dist_exceeds_limit(
            p[2],
            interp(p[0].x, p[3].x, 2.0 / 3.0),
            interp(p[0].y, p[3].y, 2.0 / 3.0),
            tolerance,
        )
}

/// (SkContourMeasureIter::Impl) Measures a path's contours one after another.
struct ContourBuilder<'a> {
    cmds: &'a [PathCommand],
    at: usize,
    tolerance: f32,
    force_closed: bool,
}

impl ContourBuilder<'_> {
    fn add_seg(cm: &mut ContourMeasure, distance: f32, pt_index: usize, kind: SegType, t: u32) {
        cm.segments.push(Segment { distance, pt_index, t_value: t, kind });
    }

    fn line_seg(cm: &mut ContourMeasure, p0: Offset, p1: Offset, mut distance: f32, pt_index: usize) -> f32 {
        let d = self::distance(p0, p1);
        let prev = distance;
        distance += d;
        if distance > prev {
            Self::add_seg(cm, distance, pt_index, SegType::Line, MAX_T_VALUE);
        }
        distance
    }

    #[allow(clippy::too_many_arguments)]
    fn quad_segs(
        &self,
        cm: &mut ContourMeasure,
        p: &[Offset; 3],
        mut distance: f32,
        mint: u32,
        maxt: u32,
        pt_index: usize,
        depth: u32,
    ) -> f32 {
        if depth < MAX_RECURSION_DEPTH && tspan_big_enough(maxt - mint) && quad_too_curvy(p, self.tolerance) {
            let tmp = chop_quad_at(p, 0.5);
            let half = (mint + maxt) >> 1;
            distance = self.quad_segs(cm, &[tmp[0], tmp[1], tmp[2]], distance, mint, half, pt_index, depth + 1);
            distance = self.quad_segs(cm, &[tmp[2], tmp[3], tmp[4]], distance, half, maxt, pt_index, depth + 1);
        } else {
            let d = self::distance(p[0], p[2]);
            let prev = distance;
            distance += d;
            if distance > prev {
                Self::add_seg(cm, distance, pt_index, SegType::Quad, maxt);
            }
        }
        distance
    }

    #[allow(clippy::too_many_arguments)]
    fn conic_segs(
        &self,
        cm: &mut ContourMeasure,
        conic: &SkConic,
        mut distance: f32,
        mint: u32,
        min_pt: Offset,
        maxt: u32,
        max_pt: Offset,
        pt_index: usize,
        depth: u32,
    ) -> f32 {
        let half = (mint + maxt) >> 1;
        let half_pt = conic.eval_at(t_value(half));
        if !finite(half_pt) {
            return distance;
        }
        if depth < MAX_RECURSION_DEPTH
            && tspan_big_enough(maxt - mint)
            && conic_too_curvy(min_pt, half_pt, max_pt, self.tolerance)
        {
            distance = self.conic_segs(cm, conic, distance, mint, min_pt, half, half_pt, pt_index, depth + 1);
            distance = self.conic_segs(cm, conic, distance, half, half_pt, maxt, max_pt, pt_index, depth + 1);
        } else {
            let d = self::distance(min_pt, max_pt);
            let prev = distance;
            distance += d;
            if distance > prev {
                Self::add_seg(cm, distance, pt_index, SegType::Conic, maxt);
            }
        }
        distance
    }

    #[allow(clippy::too_many_arguments)]
    fn cubic_segs(
        &self,
        cm: &mut ContourMeasure,
        p: &[Offset; 4],
        mut distance: f32,
        mint: u32,
        maxt: u32,
        pt_index: usize,
        depth: u32,
    ) -> f32 {
        if depth < MAX_RECURSION_DEPTH && tspan_big_enough(maxt - mint) && cubic_too_curvy(p, self.tolerance) {
            let tmp = chop_cubic_at(p, 0.5);
            let half = (mint + maxt) >> 1;
            distance =
                self.cubic_segs(cm, &[tmp[0], tmp[1], tmp[2], tmp[3]], distance, mint, half, pt_index, depth + 1);
            distance =
                self.cubic_segs(cm, &[tmp[3], tmp[4], tmp[5], tmp[6]], distance, half, maxt, pt_index, depth + 1);
        } else {
            let d = self::distance(p[0], p[3]);
            let prev = distance;
            distance += d;
            if distance > prev {
                Self::add_seg(cm, distance, pt_index, SegType::Cubic, maxt);
            }
        }
        distance
    }

    /// The next contour's measure: None if it has no length (or isn't finite).
    fn build(&mut self) -> Option<ContourMeasure> {
        let mut cm = ContourMeasure::default();
        let mut pt_index = 0usize;
        let mut distance = 0.0f32;
        let mut seen_close = self.force_closed;
        let mut seen_move = false;
        let mut last = Offset::Zero;
        while self.at < self.cmds.len() {
            let c = self.cmds[self.at];
            if seen_move && matches!(c, PathCommand::Move(_)) {
                break;
            }
            if !seen_move && !matches!(c, PathCommand::Move(_) | PathCommand::Close) {
                // Drawing with no move first (a path of commands as they are): from the origin, as a path's would.
                cm.pts.push(last);
                seen_move = true;
            }
            match c {
                PathCommand::Move(to) => {
                    cm.pts.push(to);
                    seen_move = true;
                }
                PathCommand::Line(to) => {
                    let prev = distance;
                    distance = Self::line_seg(&mut cm, last, to, distance, pt_index);
                    if distance > prev {
                        cm.pts.push(to);
                        pt_index += 1;
                    }
                }
                PathCommand::Quadratic(c1, to) => {
                    let p = [last, c1, to];
                    let prev = distance;
                    distance = self.quad_segs(&mut cm, &p, distance, 0, MAX_T_VALUE, pt_index, 0);
                    if distance > prev {
                        cm.pts.push(c1);
                        cm.pts.push(to);
                        pt_index += 2;
                    }
                }
                PathCommand::Conic(c1, to, w) => {
                    let conic = SkConic { pts: [last, c1, to], w };
                    let prev = distance;
                    distance = self.conic_segs(
                        &mut cm,
                        &conic,
                        distance,
                        0,
                        conic.pts[0],
                        MAX_T_VALUE,
                        conic.pts[2],
                        pt_index,
                        0,
                    );
                    if distance > prev {
                        cm.pts.push(Offset::new(w, 0.0));
                        cm.pts.push(c1);
                        cm.pts.push(to);
                        pt_index += 3;
                    }
                }
                PathCommand::Cubic(c1, c2, to) => {
                    let p = [last, c1, c2, to];
                    let prev = distance;
                    distance = self.cubic_segs(&mut cm, &p, distance, 0, MAX_T_VALUE, pt_index, 0);
                    if distance > prev {
                        cm.pts.push(c1);
                        cm.pts.push(c2);
                        cm.pts.push(to);
                        pt_index += 3;
                    }
                }
                PathCommand::Close => seen_close = true,
            }
            if let Some(to) = c.end() {
                last = to;
            }
            self.at += 1;
        }
        if !distance.is_finite() || cm.segments.is_empty() {
            return None;
        }
        if seen_close {
            let prev = distance;
            let first = cm.pts[0];
            let last = cm.pts[pt_index];
            distance = Self::line_seg(&mut cm, last, first, distance, pt_index);
            if distance > prev {
                cm.pts.push(first);
            }
        }
        cm.length = distance;
        cm.closed = seen_close;
        Some(cm)
    }
}

fn finite_path(path: &Path) -> bool {
    path.commands().iter().all(|c| {
        let mut ok = true;
        c.map(|p| {
            ok &= finite(p);
            p
        });
        ok && !matches!(c, PathCommand::Conic(_, _, w) if !w.is_finite())
    })
}

/// Every contour of a path with a length, measured (SkContourMeasureIter), its curves cut until each piece is within
/// half a unit over `res_scale` of a line.
pub(crate) fn measure_contours(path: &Path, force_closed: bool, res_scale: f32) -> Vec<ContourMeasure> {
    let mut out = Vec::new();
    if !finite_path(path) {
        return out;
    }
    let mut b =
        ContourBuilder { cmds: path.commands(), at: 0, tolerance: CHEAP_DIST_LIMIT * (1.0 / res_scale), force_closed };
    while b.at < b.cmds.len() {
        if let Some(cm) = b.build() {
            out.push(cm);
        }
    }
    out
}

// ---- PathMeasure (SkPathMeasure) ---------------------------------------------------------------------------------

/// Lengths along a path (Compose's PathMeasure, Android's: Skia's SkPathMeasure), contour by contour (those of no
/// length skipped), its curves measured as Skia measures them: cut in halves until each piece is within half a unit
/// of a line (half a unit over its res scale), positions and tangents on the curves themselves, pieces of them as
/// curves.
#[derive(Clone, Debug, Default)]
pub struct PathMeasure {
    contours: Vec<ContourMeasure>,
    current: usize,
}

impl PathMeasure {
    /// One with no path: no length.
    ///
    /// Kotlin: `PathMeasure()`
    pub fn new() -> PathMeasure {
        PathMeasure::default()
    }

    /// Measures `path` (each contour closed if `force_closed`), from its first contour.
    pub fn set_path(&mut self, path: &Path, force_closed: bool) {
        self.set_path_at_res_scale(path, force_closed, 1.0);
    }

    /// Measures `path` drawn at `res_scale` pixels a unit: its curves within half a pixel of the lines measured.
    pub fn set_path_at_res_scale(&mut self, path: &Path, force_closed: bool, res_scale: f32) {
        self.contours = measure_contours(path, force_closed, res_scale);
        self.current = 0;
    }

    /// The current contour's (the first, until next_contour()); 0 if none.
    pub fn length(&self) -> f32 {
        self.contours.get(self.current).map_or(0.0, |c| c.length)
    }

    /// The current contour ends with a close (or the path was measured force closed).
    pub fn is_closed(&self) -> bool {
        self.contours.get(self.current).is_some_and(|c| c.closed)
    }

    /// Moves on to the next contour, if there is one.
    pub fn next_contour(&mut self) -> bool {
        if self.current >= self.contours.len() {
            return false;
        }
        self.current += 1;
        self.current < self.contours.len()
    }

    /// Where `distance` (kept within 0..length) lands; unspecified (NaN) if there is no contour.
    pub fn get_position(&self, distance: f32) -> Offset {
        self.contours.get(self.current).and_then(|c| c.get_pos_tan(distance)).map_or(Offset::Unspecified, |(p, _)| p)
    }

    /// The unit vector the contour goes at `distance` (kept within 0..length); unspecified (NaN) if there is no
    /// contour.
    pub fn get_tangent(&self, distance: f32) -> Offset {
        self.contours.get(self.current).and_then(|c| c.get_pos_tan(distance)).map_or(Offset::Unspecified, |(_, t)| t)
    }

    /// Adds the piece from `start_distance` to `stop_distance` (kept within 0..length; false unless start is under
    /// stop) to `destination`: its curves cut at those places, beginning with a move (else going on from where it
    /// was).
    pub fn get_segment(
        &self,
        mut start_distance: f32,
        mut stop_distance: f32,
        destination: &mut Path,
        start_with_move_to: bool,
    ) -> bool {
        // (Android's PathMeasure: the distances kept within the contour, nothing unless start is under stop)
        let length = self.length();
        if start_distance < 0.0 {
            start_distance = 0.0;
        }
        if stop_distance > length {
            stop_distance = length;
        }
        if start_distance.is_nan() || stop_distance.is_nan() || start_distance >= stop_distance {
            return false;
        }
        self.contours[self.current].get_segment(start_distance, stop_distance, destination, start_with_move_to)
    }
}

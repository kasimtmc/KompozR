//! The curve geometry paths are built on, ported from Skia's (SkGeometry, SkCubicClipper, SkPathPriv's winding and
//! convexity), so that a Path is what an Android one is: quadratics, conics (rational quadratics: arcs and ovals
//! exactly), cubics; their roots, chops, conversions, and whether a point is inside.

use super::path::{PathCommand, PathFillType};
use crate::geometry::Offset;

/// Skia's SK_ScalarNearlyZero.
pub(crate) const NEARLY_ZERO: f32 = 1.0 / (1 << 12) as f32;

/// How many times a conic is halved at most to make quadratics of it (2^5 of them).
pub(crate) const MAX_CONIC_TO_QUAD_POW2: u32 = 5;

fn dot(a: Offset, b: Offset) -> f32 {
    a.x * b.x + a.y * b.y
}
fn cross(a: Offset, b: Offset) -> f32 {
    a.x * b.y - a.y * b.x
}
fn along(a: Offset, b: Offset, t: f32) -> Offset {
    Offset::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}
fn same(a: Offset, b: Offset) -> bool {
    a.x == b.x && a.y == b.y
}
fn finite(p: Offset) -> bool {
    p.x.is_finite() && p.y.is_finite()
}
fn nearly_equal(a: f32, b: f32) -> bool {
    (a - b).abs() <= NEARLY_ZERO
}
/// Points equal within a tolerance (SkPointPriv::EqualsWithinTolerance).
fn equal_within(a: Offset, b: Offset) -> bool {
    (a.x - b.x).abs() <= NEARLY_ZERO && (a.y - b.y).abs() <= NEARLY_ZERO
}

fn not_monotonic(a: f32, b: f32, c: f32) -> bool {
    let ab = a - b;
    let mut bc = b - c;
    if ab < 0.0 {
        bc = -bc;
    }
    ab == 0.0 || bc < 0.0
}

/// numer / denom where it lies in (0, 1).
fn unit_divide(mut numer: f32, mut denom: f32) -> Option<f32> {
    if numer < 0.0 {
        numer = -numer;
        denom = -denom;
    }
    if denom == 0.0 || numer == 0.0 || numer >= denom {
        return None;
    }
    let r = numer / denom;
    if r.is_nan() || r == 0.0 {
        return None;
    }
    Some(r)
}

/// The roots of A t^2 + B t + C in (0, 1), in order, a double root once (Numerical Recipes').
pub(crate) fn unit_quad_roots(a: f32, b: f32, c: f32) -> Vec<f32> {
    let mut roots = Vec::with_capacity(2);
    if a == 0.0 {
        roots.extend(unit_divide(-c, b));
        return roots;
    }
    let dr = b as f64 * b as f64 - 4.0 * a as f64 * c as f64;
    if dr < 0.0 {
        return roots;
    }
    let r = dr.sqrt() as f32;
    if !r.is_finite() {
        return roots;
    }
    let q = if b < 0.0 { -(b - r) / 2.0 } else { -(b + r) / 2.0 };
    roots.extend(unit_divide(q, a));
    roots.extend(unit_divide(c, q));
    if roots.len() == 2 {
        if roots[0] > roots[1] {
            roots.swap(0, 1);
        } else if roots[0] == roots[1] {
            roots.pop(); // a double root once
        }
    }
    roots
}

pub(crate) fn quad_tangent(p: &[Offset; 3], t: f32) -> Offset {
    if (t == 0.0 && same(p[0], p[1])) || (t == 1.0 && same(p[1], p[2])) {
        return p[2] - p[0];
    }
    let b = p[1] - p[0];
    let a = p[2] - p[1] - b;
    let d = a * t + b;
    d + d
}

/// Split at t (0 < t < 1): [0..=2] and [2..=4].
pub(crate) fn chop_quad_at(src: &[Offset; 3], t: f32) -> [Offset; 5] {
    let p01 = along(src[0], src[1], t);
    let p12 = along(src[1], src[2], t);
    [src[0], p01, along(p01, p12, t), p12, src[2]]
}

/// Split at its y extremum if it has one (made flat there): 0 for one quad (in dst[0..3]), 1 for two.
pub(crate) fn chop_quad_at_y_extrema(src: &[Offset; 3], dst: &mut [Offset; 5]) -> usize {
    let a = src[0].y;
    let mut b = src[1].y;
    let c = src[2].y;
    if not_monotonic(a, b, c) {
        if let Some(t) = unit_divide(a - b, a - b - b + c) {
            *dst = chop_quad_at(src, t);
            dst[1].y = dst[2].y; // flat at the extremum
            dst[3].y = dst[2].y;
            return 1;
        }
        b = if (a - b).abs() < (b - c).abs() { a } else { c }; // (underflow: made monotonic)
    }
    dst[0] = Offset::new(src[0].x, a);
    dst[1] = Offset::new(src[1].x, b);
    dst[2] = Offset::new(src[2].x, c);
    0
}

pub(crate) fn cubic_at(p: &[Offset; 4], t: f32) -> Offset {
    let ab = along(p[0], p[1], t);
    let bc = along(p[1], p[2], t);
    let cd = along(p[2], p[3], t);
    along(along(ab, bc, t), along(bc, cd, t), t)
}

pub(crate) fn cubic_tangent(p: &[Offset; 4], t: f32) -> Offset {
    if (t == 0.0 && same(p[0], p[1])) || (t == 1.0 && same(p[2], p[3])) {
        let d = if t == 0.0 { p[2] - p[0] } else { p[3] - p[1] };
        return if d.x == 0.0 && d.y == 0.0 { p[3] - p[0] } else { d };
    }
    let a = p[3] + (p[1] - p[2]) * 3.0 - p[0];
    let b = (p[2] - p[1] * 2.0 + p[0]) * 2.0;
    let c = p[1] - p[0];
    (a * t + b) * t + c
}

/// Split at t: [0..=3] and [3..=6].
pub(crate) fn chop_cubic_at(src: &[Offset; 4], t: f32) -> [Offset; 7] {
    if t == 1.0 {
        return [src[0], src[1], src[2], src[3], src[3], src[3], src[3]];
    }
    let mix = |a: Offset, b: Offset| (b - a) * t + a;
    let ab = mix(src[0], src[1]);
    let bc = mix(src[1], src[2]);
    let cd = mix(src[2], src[3]);
    let abc = mix(ab, bc);
    let bcd = mix(bc, cd);
    [src[0], ab, abc, mix(abc, bcd), bcd, cd, src[3]]
}

fn first4(p: &[Offset]) -> [Offset; 4] {
    [p[0], p[1], p[2], p[3]]
}

/// The piece between t0 and t1.
pub(crate) fn chop_cubic_between(src: &[Offset; 4], t0: f32, t1: f32) -> [Offset; 4] {
    let mut src = *src;
    if t0 > 0.0 {
        src = first4(&chop_cubic_at(&src, t0)[3..]);
    }
    let t = if t0 < 1.0 { (t1 - t0) / (1.0 - t0) } else { 1.0 };
    if t >= 1.0 {
        return src;
    }
    first4(&chop_cubic_at(&src, t.clamp(0.0, 1.0)))
}

fn cubic_extrema(a: f32, b: f32, c: f32, d: f32) -> Vec<f32> {
    unit_quad_roots(d - a + 3.0 * (b - c), 2.0 * (a - b - b + c), b - a)
}

/// Split at its y extrema: 0, 1 or 2 splits (dst[0..=3], [3..=6], [6..=9]), monotonic in y.
pub(crate) fn chop_cubic_at_y_extrema(src: &[Offset; 4], dst: &mut [Offset; 10]) -> usize {
    let t = cubic_extrema(src[0].y, src[1].y, src[2].y, src[3].y);
    if t.is_empty() {
        dst[..4].copy_from_slice(src);
        return 0;
    }
    dst[..7].copy_from_slice(&chop_cubic_at(src, t[0]));
    if t.len() == 2 {
        let tail = first4(&dst[3..]);
        dst[3..10].copy_from_slice(&chop_cubic_at(&tail, ((t[1] - t[0]) / (1.0 - t[0])).clamp(0.0, 1.0)));
    }
    // Flat at each extremum.
    dst[2].y = dst[3].y;
    dst[4].y = dst[3].y;
    if t.len() == 2 {
        dst[5].y = dst[6].y;
        dst[7].y = dst[6].y;
    }
    t.len()
}

/// Where a cubic monotonic in y crosses y (by bisection, as Skia's).
pub(crate) fn chop_mono_cubic_at_y(p: &[Offset; 4], y: f32) -> Option<f32> {
    let c = [p[0].y - y, p[1].y - y, p[2].y - y, p[3].y - y];
    let (mut neg, mut pos);
    if c[0] < 0.0 {
        if c[3] < 0.0 {
            return None;
        }
        neg = 0.0f32;
        pos = 1.0f32;
    } else if c[0] > 0.0 {
        if c[3] > 0.0 {
            return None;
        }
        neg = 1.0;
        pos = 0.0;
    } else {
        return Some(0.0);
    }
    let interp = |a: f32, b: f32, s: f32| a + (b - a) * s;
    let tolerance = 1.0 / 65536.0;
    loop {
        let mid = (pos + neg) / 2.0;
        let y01 = interp(c[0], c[1], mid);
        let y12 = interp(c[1], c[2], mid);
        let y23 = interp(c[2], c[3], mid);
        let y0123 = interp(interp(y01, y12, mid), interp(y12, y23, mid), mid);
        if y0123 == 0.0 {
            return Some(mid);
        }
        if y0123 < 0.0 {
            neg = mid;
        } else {
            pos = mid;
        }
        if (pos - neg).abs() <= tolerance {
            break;
        }
    }
    Some((neg + pos) / 2.0)
}

// ---- Conics ------------------------------------------------------------------------------------------------------

/// A rational quadratic: p[1] weighed by w (an arc of a circle's quarter: w = cos(45°)).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(crate) struct Conic {
    pub p: [Offset; 3],
    pub w: f32,
}

impl Conic {
    pub fn new(p: [Offset; 3], w: f32) -> Conic {
        Conic { p, w }
    }

    pub fn at(&self, t: f32) -> Offset {
        // (p0 (1-t)^2 + 2 w p1 t (1-t) + p2 t^2) / ((1-t)^2 + 2 w t (1-t) + t^2)
        let p = &self.p;
        let w = self.w;
        let n = p[0] * ((1.0 - t) * (1.0 - t)) + p[1] * (2.0 * w * t * (1.0 - t)) + p[2] * (t * t);
        let d = (1.0 - t) * (1.0 - t) + 2.0 * w * t * (1.0 - t) + t * t;
        n * (1.0 / d)
    }

    pub fn tangent(&self, t: f32) -> Offset {
        let p = &self.p;
        if (t == 0.0 && same(p[0], p[1])) || (t == 1.0 && same(p[1], p[2])) {
            return p[2] - p[0];
        }
        let p20 = p[2] - p[0];
        let p10 = p[1] - p[0];
        let c = p10 * self.w;
        let a = p20 * self.w - p20;
        let b = p20 - c - c;
        (a * t + b) * t + c
    }

    /// Split at t; None if a piece isn't finite.
    pub fn chop_at(&self, t: f32) -> Option<[Conic; 2]> {
        // In homogeneous coordinates: (p0, 1), (w p1, w), (p2, 1), split there as a quadratic.
        let p = &self.p;
        let w = self.w;
        let a = [p[0].x, p[0].y, 1.0];
        let b = [p[1].x * w, p[1].y * w, w];
        let c = [p[2].x, p[2].y, 1.0];
        let mix =
            |u: [f32; 3], v: [f32; 3]| [u[0] + (v[0] - u[0]) * t, u[1] + (v[1] - u[1]) * t, u[2] + (v[2] - u[2]) * t];
        let ab = mix(a, b);
        let bc = mix(b, c);
        let abc = mix(ab, bc);
        let down = |q: [f32; 3]| Offset::new(q[0] / q[2], q[1] / q[2]);
        let root = abc[2].sqrt();
        let first = Conic::new([p[0], down(ab), down(abc)], ab[2] / root);
        let second = Conic::new([first.p[2], down(bc), p[2]], bc[2] / root);
        let ok =
            first.p.iter().chain(second.p.iter()).all(|&q| finite(q)) && first.w.is_finite() && second.w.is_finite();
        ok.then_some([first, second])
    }

    /// The piece between t1 and t2.
    pub fn between(&self, t1: f32, t2: f32) -> Conic {
        if t1 == 0.0 && t2 == 1.0 {
            return *self;
        }
        if (t1 == 0.0 || t2 == 1.0)
            && let Some(pair) = self.chop_at(if t1 != 0.0 { t1 } else { t2 })
        {
            return pair[if t1 != 0.0 { 1 } else { 0 }];
        }
        // Numerators and denominators at t1, the middle and t2 (Skia's SkConic::chopAt(t1, t2)).
        let p = &self.p;
        let w = self.w;
        let numer = |t: f32| p[0] * ((1.0 - t) * (1.0 - t)) + p[1] * (2.0 * w * t * (1.0 - t)) + p[2] * (t * t);
        let denom = |t: f32| (1.0 - t) * (1.0 - t) + 2.0 * w * t * (1.0 - t) + t * t;
        let mid = (t1 + t2) / 2.0;
        let a_xy = numer(t1);
        let d_xy = numer(mid);
        let c_xy = numer(t2);
        let a_z = denom(t1);
        let d_z = denom(mid);
        let c_z = denom(t2);
        let b_xy = d_xy * 2.0 - (a_xy + c_xy) * 0.5;
        let b_z = d_z * 2.0 - (a_z + c_z) * 0.5;
        Conic::new([a_xy * (1.0 / a_z), b_xy * (1.0 / b_z), c_xy * (1.0 / c_z)], b_z / (a_z * c_z).sqrt())
    }

    /// Its halves.
    pub fn chop(&self) -> [Conic; 2] {
        let p = &self.p;
        let w = self.w;
        let scale = 1.0 / (1.0 + w);
        let t0 = p[0] * scale;
        let t1 = p[1] * (w * scale);
        let t2 = p[2] * scale;
        let q1 = t0 + t1;
        let q3 = t1 + t2;
        let q2 = t0 * 0.5 + t1 + t2 * 0.5;
        let half = (0.5 + w * 0.5).sqrt();
        [Conic::new([p[0], q1, q2], half), Conic::new([q2, q3, p[2]], half)]
    }

    pub fn find_y_extrema(&self) -> Option<f32> {
        // The derivative's numerator (its magnitude disregarded) as a quadratic.
        let p = &self.p;
        let p20 = p[2].y - p[0].y;
        let p10 = p[1].y - p[0].y;
        let wp10 = self.w * p10;
        let roots = unit_quad_roots(self.w * p20 - p20, p20 - 2.0 * wp10, wp10);
        if roots.len() == 1 { Some(roots[0]) } else { None }
    }

    pub fn chop_at_y_extrema(&self) -> Option<[Conic; 2]> {
        let t = self.find_y_extrema()?;
        let mut dst = self.chop_at(t)?;
        let y = dst[0].p[2].y; // flat at the extremum
        dst[0].p[1].y = y;
        dst[1].p[0].y = y;
        dst[1].p[1].y = y;
        Some(dst)
    }

    /// How many times to halve it (a power of 2) for quadratics within `tolerance` of it.
    pub fn quad_pow2(&self, tolerance: f32) -> u32 {
        let p = &self.p;
        if tolerance < 0.0
            || !tolerance.is_finite()
            || !finite(p[0])
            || !finite(p[1])
            || !finite(p[2])
            || self.w < 0.0
            || !self.w.is_finite()
        {
            return 0;
        }
        // Floater's error for the quadratic through the same points.
        let a = self.w - 1.0;
        let k = a / (4.0 * (2.0 + a));
        let x = k * (p[0].x - 2.0 * p[1].x + p[2].x);
        let y = k * (p[0].y - 2.0 * p[1].y + p[2].y);
        let mut error = (x * x + y * y).sqrt();
        let mut pow2 = 0;
        while pow2 < MAX_CONIC_TO_QUAD_POW2 {
            if error <= tolerance {
                break;
            }
            error *= 0.25;
            pow2 += 1;
        }
        pow2
    }

    /// The quadratics it is halved into `pow2` times: its start, then two points for each (2 * 2^pow2 + 1 points;
    /// fewer for an extreme weight that makes two lines).
    pub fn chop_into_quads_pow2(&self, mut pow2: u32) -> Vec<Offset> {
        if self.w < 0.0 || !self.w.is_finite() {
            pow2 = 0;
        }
        let mut pts = vec![self.p[0]];
        let mut done = false;
        if pow2 == MAX_CONIC_TO_QUAD_POW2 {
            // An extreme weight: two lines, if the first chop makes them.
            let dst = self.chop();
            if equal_within(dst[0].p[1], dst[0].p[2]) && equal_within(dst[1].p[0], dst[1].p[1]) {
                pts.extend([dst[0].p[1], dst[0].p[1], dst[0].p[1], dst[1].p[2]]);
                done = true;
            }
        }
        if !done {
            subdivide(self, &mut pts, pow2);
        }
        if !pts.iter().all(|&q| finite(q)) {
            let n = pts.len();
            for q in &mut pts[1..n - 1] {
                *q = self.p[1];
            }
        }
        pts
    }

    /// An arc of the unit circle from the unit vector `start` to `stop` (clockwise or not) as at most five conics,
    /// mapped by `m` (x' = m0 x + m1 y + m2, y' = m3 x + m4 y + m5): none if they coincide.
    pub fn unit_arc(start: Offset, stop: Offset, clockwise: bool, m: &[f32; 6]) -> Vec<Conic> {
        let x = dot(start, stop);
        let mut y = cross(start, stop);
        let abs_y = y.abs();
        if abs_y <= NEARLY_ZERO && x > 0.0 && ((y >= 0.0 && clockwise) || (y <= 0.0 && !clockwise)) {
            return Vec::new();
        }
        if !clockwise {
            y = -y;
        }
        // One conic a quadrant, then what is left of it (less than 90 degrees).
        let mut quadrant = 0;
        if y == 0.0 {
            quadrant = 2;
        } else if x == 0.0 {
            quadrant = if y > 0.0 { 1 } else { 3 };
        } else {
            if y < 0.0 {
                quadrant += 2;
            }
            if (x < 0.0) != (y < 0.0) {
                quadrant += 1;
            }
        }
        const QUADRANT_PTS: [Offset; 8] = [
            Offset::new(1.0, 0.0),
            Offset::new(1.0, 1.0),
            Offset::new(0.0, 1.0),
            Offset::new(-1.0, 1.0),
            Offset::new(-1.0, 0.0),
            Offset::new(-1.0, -1.0),
            Offset::new(0.0, -1.0),
            Offset::new(1.0, -1.0),
        ];
        let quadrant_weight = std::f32::consts::FRAC_1_SQRT_2;
        let mut dst: Vec<Conic> = (0..quadrant)
            .map(|i| {
                Conic::new(
                    [QUADRANT_PTS[i * 2], QUADRANT_PTS[i * 2 + 1], QUADRANT_PTS[(i * 2 + 2) % 8]],
                    quadrant_weight,
                )
            })
            .collect();
        let last = Offset::new(x, y);
        let last_q = QUADRANT_PTS[quadrant * 2];
        let d = dot(last_q, last);
        if d.is_nan() {
            return Vec::new();
        }
        if d < 1.0 {
            let mut off_curve = Offset::new(last_q.x + x, last_q.y + y);
            let cos_half = ((1.0 + d) / 2.0).sqrt();
            let len = off_curve.x.hypot(off_curve.y);
            if len > 0.0 {
                off_curve = off_curve * (1.0 / cos_half / len);
            }
            if !equal_within(last_q, off_curve) {
                dst.push(Conic::new([last_q, off_curve, last], cos_half));
            }
        }
        // Turned to start at `start` (mirrored in y going counterclockwise), then mapped.
        let s = start.y;
        let c = start.x;
        for conic in &mut dst {
            for q in &mut conic.p {
                let qy = if clockwise { q.y } else { -q.y };
                let r = Offset::new(c * q.x - s * qy, s * q.x + c * qy);
                *q = Offset::new(m[0] * r.x + m[1] * r.y + m[2], m[3] * r.x + m[4] * r.y + m[5]);
            }
        }
        dst
    }

    /// The weight of the conic `pts`, w, through a projective map (rows m0..m8).
    pub fn transformed_weight(pts: &[Offset; 3], w: f32, m: &[f32; 9]) -> f32 {
        let z = |q: Offset, qw: f32| {
            m[6] as f64 * q.x as f64 * qw as f64 + m[7] as f64 * q.y as f64 * qw as f64 + m[8] as f64 * qw as f64
        };
        let w0 = z(pts[0], 1.0);
        let w1 = z(pts[1], w);
        let w2 = z(pts[2], 1.0);
        (w1 * w1 / (w0 * w2)).sqrt() as f32
    }
}

fn between(a: f32, b: f32, c: f32) -> bool {
    (a - b) * (c - b) <= 0.0
}

/// The conic halved `level` times, each quadratic's last two points appended to `pts`.
fn subdivide(src: &Conic, pts: &mut Vec<Offset>, level: u32) {
    if level == 0 {
        pts.push(src.p[1]);
        pts.push(src.p[2]);
        return;
    }
    let mut dst = src.chop();
    let start_y = src.p[0].y;
    let end_y = src.p[2].y;
    if between(start_y, src.p[1].y, end_y) {
        // Keep the halves monotonic in y if it was.
        let mid_y = dst[0].p[2].y;
        if !between(start_y, mid_y, end_y) {
            let closer = if (mid_y - start_y).abs() < (mid_y - end_y).abs() { start_y } else { end_y };
            dst[0].p[2].y = closer;
            dst[1].p[0].y = closer;
        }
        if !between(start_y, dst[0].p[1].y, dst[0].p[2].y) {
            dst[0].p[1].y = start_y;
        }
        if !between(dst[1].p[0].y, dst[1].p[1].y, end_y) {
            dst[1].p[1].y = end_y;
        }
    }
    subdivide(&dst[0], pts, level - 1);
    subdivide(&dst[1], pts, level - 1);
}

// ---- Winding (Skia's SkPathPriv::Contains) -----------------------------------------------------------------------

fn on_curve(x: f32, y: f32, start: Offset, end: Offset) -> bool {
    if start.y == end.y {
        return between(start.x, x, end.x) && x != end.x;
    }
    x == start.x && y == start.y
}

fn winding_mono_cubic(p: &[Offset; 4], x: f32, y: f32, on_count: &mut i32) -> i32 {
    let mut y0 = p[0].y;
    let mut y3 = p[3].y;
    let mut dir = 1;
    if y0 > y3 {
        std::mem::swap(&mut y0, &mut y3);
        dir = -1;
    }
    if y < y0 || y > y3 {
        return 0;
    }
    if on_curve(x, y, p[0], p[3]) {
        *on_count += 1;
        return 0;
    }
    if y == y3 {
        return 0;
    }
    let lo = p[0].x.min(p[1].x).min(p[2].x).min(p[3].x);
    let hi = p[0].x.max(p[1].x).max(p[2].x).max(p[3].x);
    if x < lo {
        return 0;
    }
    if x > hi {
        return dir;
    }
    let Some(t) = chop_mono_cubic_at_y(p, y) else { return 0 };
    let xt = cubic_at(p, t).x;
    if nearly_equal(xt, x) && (x != p[3].x || y != p[3].y) {
        *on_count += 1;
        return 0;
    }
    if xt < x { dir } else { 0 }
}

fn winding_cubic(p: &[Offset; 4], x: f32, y: f32, on_count: &mut i32) -> i32 {
    let mut dst = [Offset::Zero; 10];
    let n = chop_cubic_at_y_extrema(p, &mut dst);
    (0..=n).map(|i| winding_mono_cubic(&first4(&dst[i * 3..]), x, y, on_count)).sum()
}

fn conic_numerator(p0: f32, p1: f32, p2: f32, w: f32, t: f32) -> f64 {
    let c = p0;
    let a = p2 - 2.0 * p1 * w + c;
    let b = 2.0 * (p1 * w - c);
    ((a * t + b) * t + c) as f64
}
fn conic_denominator(w: f32, t: f32) -> f64 {
    let b = 2.0 * (w - 1.0);
    let a = -b;
    ((a * t + b) * t + 1.0) as f64
}

fn winding_mono_conic(c: &Conic, x: f32, y: f32, on_count: &mut i32) -> i32 {
    let p = &c.p;
    let mut y0 = p[0].y;
    let mut y2 = p[2].y;
    let mut dir = 1;
    if y0 > y2 {
        std::mem::swap(&mut y0, &mut y2);
        dir = -1;
    }
    if y < y0 || y > y2 {
        return 0;
    }
    if on_curve(x, y, p[0], p[2]) {
        *on_count += 1;
        return 0;
    }
    if y == y2 {
        return 0;
    }
    let mut a = p[2].y;
    let mut b = p[1].y * c.w - y * c.w + y;
    let mut cc = p[0].y;
    a += cc - 2.0 * b;
    b -= cc;
    cc -= y;
    let roots = unit_quad_roots(a, 2.0 * b, cc);
    let xt = if roots.is_empty() {
        p[if dir == 1 { 0 } else { 2 }].x
    } else {
        (conic_numerator(p[0].x, p[1].x, p[2].x, c.w, roots[0]) / conic_denominator(c.w, roots[0])) as f32
    };
    if nearly_equal(xt, x) && (x != p[2].x || y != p[2].y) {
        *on_count += 1;
        return 0;
    }
    if xt < x { dir } else { 0 }
}

fn mono_quad(y0: f32, y1: f32, y2: f32) -> bool {
    if y0 == y1 {
        return true;
    }
    if y0 < y1 { y1 <= y2 } else { y1 >= y2 }
}

fn winding_conic(p: &[Offset; 3], w: f32, x: f32, y: f32, on_count: &mut i32) -> i32 {
    let c = Conic::new(*p, w);
    let chopped = if mono_quad(p[0].y, p[1].y, p[2].y) { None } else { c.chop_at_y_extrema() };
    match chopped {
        None => winding_mono_conic(&c, x, y, on_count),
        Some(halves) => winding_mono_conic(&halves[0], x, y, on_count) + winding_mono_conic(&halves[1], x, y, on_count),
    }
}

fn winding_mono_quad(p: &[Offset; 3], x: f32, y: f32, on_count: &mut i32) -> i32 {
    let mut y0 = p[0].y;
    let mut y2 = p[2].y;
    let mut dir = 1;
    if y0 > y2 {
        std::mem::swap(&mut y0, &mut y2);
        dir = -1;
    }
    if y < y0 || y > y2 {
        return 0;
    }
    if on_curve(x, y, p[0], p[2]) {
        *on_count += 1;
        return 0;
    }
    if y == y2 {
        return 0;
    }
    let roots = unit_quad_roots(p[0].y - 2.0 * p[1].y + p[2].y, 2.0 * (p[1].y - p[0].y), p[0].y - y);
    let xt = if roots.is_empty() {
        p[if dir == 1 { 0 } else { 2 }].x
    } else {
        let t = roots[0];
        let c = p[0].x;
        let a = p[2].x - 2.0 * p[1].x + c;
        let b = 2.0 * (p[1].x - c);
        (a * t + b) * t + c
    };
    if nearly_equal(xt, x) && (x != p[2].x || y != p[2].y) {
        *on_count += 1;
        return 0;
    }
    if xt < x { dir } else { 0 }
}

fn winding_quad(p: &[Offset; 3], x: f32, y: f32, on_count: &mut i32) -> i32 {
    if mono_quad(p[0].y, p[1].y, p[2].y) {
        return winding_mono_quad(p, x, y, on_count);
    }
    let mut dst = [Offset::Zero; 5];
    let n = chop_quad_at_y_extrema(p, &mut dst);
    let mut w = winding_mono_quad(&[dst[0], dst[1], dst[2]], x, y, on_count);
    if n > 0 {
        w += winding_mono_quad(&[dst[2], dst[3], dst[4]], x, y, on_count);
    }
    w
}

fn winding_line(p: &[Offset; 2], x: f32, y: f32, on_count: &mut i32) -> i32 {
    let x0 = p[0].x;
    let mut y0 = p[0].y;
    let x1 = p[1].x;
    let mut y1 = p[1].y;
    let dy = y1 - y0;
    let mut dir = 1;
    if y0 > y1 {
        std::mem::swap(&mut y0, &mut y1);
        dir = -1;
    }
    if y < y0 || y > y1 {
        return 0;
    }
    if on_curve(x, y, p[0], p[1]) {
        *on_count += 1;
        return 0;
    }
    if y == y1 {
        return 0;
    }
    let c = (x1 - x0) * (y - p[0].y) - dy * (x - x0);
    if c == 0.0 {
        if x != x1 || y != p[1].y {
            *on_count += 1;
        }
        dir = 0;
    } else if (if c > 0.0 { 1 } else { -1 }) == dir {
        dir = 0;
    }
    dir
}

fn tangent_cubic(p: &[Offset; 4], x: f32, y: f32, tangents: &mut Vec<Offset>) {
    if !between(p[0].y, y, p[1].y) && !between(p[1].y, y, p[2].y) && !between(p[2].y, y, p[3].y) {
        return;
    }
    if !between(p[0].x, x, p[1].x) && !between(p[1].x, x, p[2].x) && !between(p[2].x, x, p[3].x) {
        return;
    }
    let mut dst = [Offset::Zero; 10];
    let n = chop_cubic_at_y_extrema(p, &mut dst);
    for i in 0..=n {
        let c = first4(&dst[i * 3..]);
        let Some(t) = chop_mono_cubic_at_y(&c, y) else { continue };
        if !nearly_equal(x, cubic_at(&c, t).x) {
            continue;
        }
        tangents.push(cubic_tangent(&c, t));
    }
}

fn tangent_conic(p: &[Offset; 3], w: f32, x: f32, y: f32, tangents: &mut Vec<Offset>) {
    if !between(p[0].y, y, p[1].y) && !between(p[1].y, y, p[2].y) {
        return;
    }
    if !between(p[0].x, x, p[1].x) && !between(p[1].x, x, p[2].x) {
        return;
    }
    let mut a = p[2].y;
    let mut b = p[1].y * w - y * w + y;
    let mut c = p[0].y;
    a += c - 2.0 * b;
    b -= c;
    c -= y;
    for t in unit_quad_roots(a, 2.0 * b, c) {
        let xt = (conic_numerator(p[0].x, p[1].x, p[2].x, w, t) / conic_denominator(w, t)) as f32;
        if !nearly_equal(x, xt) {
            continue;
        }
        tangents.push(Conic::new(*p, w).tangent(t));
    }
}

fn tangent_quad(p: &[Offset; 3], x: f32, y: f32, tangents: &mut Vec<Offset>) {
    if !between(p[0].y, y, p[1].y) && !between(p[1].y, y, p[2].y) {
        return;
    }
    if !between(p[0].x, x, p[1].x) && !between(p[1].x, x, p[2].x) {
        return;
    }
    for t in unit_quad_roots(p[0].y - 2.0 * p[1].y + p[2].y, 2.0 * (p[1].y - p[0].y), p[0].y - y) {
        let c = p[0].x;
        let a = p[2].x - 2.0 * p[1].x + c;
        let b = 2.0 * (p[1].x - c);
        if !nearly_equal(x, (a * t + b) * t + c) {
            continue;
        }
        tangents.push(quad_tangent(p, t));
    }
}

fn tangent_line(p: &[Offset; 2], x: f32, y: f32, tangents: &mut Vec<Offset>) {
    if !between(p[0].y, y, p[1].y) || !between(p[0].x, x, p[1].x) {
        return;
    }
    let dx = p[1].x - p[0].x;
    let dy = p[1].y - p[0].y;
    if !nearly_equal((x - p[0].x) * dy, dx * (y - p[0].y)) {
        return;
    }
    tangents.push(Offset::new(dx, dy));
}

/// An edge of a path and its points (the one it starts from first).
enum Edge {
    Line([Offset; 2]),
    Quad([Offset; 3]),
    Conic([Offset; 3], f32),
    Cubic([Offset; 4]),
}

/// Each edge of the path, open contours closed by a line unless `close_open` is false (Skia's SkPathEdgeIter).
fn for_each_edge(commands: &[PathCommand], close_open: bool, mut edge: impl FnMut(Edge)) {
    let mut start = Offset::Zero;
    let mut current = Offset::Zero;
    let mut open = false;
    let close_contour = |current: &mut Offset, start: Offset, open: &mut bool, edge: &mut dyn FnMut(Edge)| {
        if close_open && *open && !same(*current, start) {
            edge(Edge::Line([*current, start]));
        }
        *open = false;
        *current = start;
    };
    for c in commands {
        match *c {
            PathCommand::Move(p) => {
                close_contour(&mut current, start, &mut open, &mut edge);
                start = p;
                current = p;
            }
            PathCommand::Line(p) => {
                edge(Edge::Line([current, p]));
                current = p;
                open = true;
            }
            PathCommand::Quadratic(c1, p) => {
                edge(Edge::Quad([current, c1, p]));
                current = p;
                open = true;
            }
            PathCommand::Conic(c1, p, w) => {
                edge(Edge::Conic([current, c1, p], w));
                current = p;
                open = true;
            }
            PathCommand::Cubic(c1, c2, p) => {
                edge(Edge::Cubic([current, c1, c2, p]));
                current = p;
                open = true;
            }
            PathCommand::Close => close_contour(&mut current, start, &mut open, &mut edge),
        }
    }
    close_contour(&mut current, start, &mut open, &mut edge);
}

fn edge_winding(e: &Edge, q: Offset, on_count: &mut i32) -> i32 {
    match e {
        Edge::Line(p) => winding_line(p, q.x, q.y, on_count),
        Edge::Quad(p) => winding_quad(p, q.x, q.y, on_count),
        Edge::Conic(p, w) => winding_conic(p, *w, q.x, q.y, on_count),
        Edge::Cubic(p) => winding_cubic(p, q.x, q.y, on_count),
    }
}

/// How many times the edges wind round `q` (crossing a ray from it; points on them don't count), open contours
/// closed by a line or not.
pub(crate) fn winding(commands: &[PathCommand], q: Offset, close_open: bool) -> i32 {
    let mut w = 0;
    let mut on_count = 0;
    for_each_edge(commands, close_open, |e| w += edge_winding(&e, q, &mut on_count));
    w
}

/// Whether `q` is inside `commands` by `fill`, its curves exact (Skia's SkPath::contains: a point on the outline is
/// inside; open contours count as closed).
pub(crate) fn contains(commands: &[PathCommand], fill: PathFillType, q: Offset) -> bool {
    if commands.is_empty() {
        return false;
    }
    // Inside its bounds (control points included), edges inclusive.
    let (mut l, mut t, mut r, mut b) = (1e30f32, 1e30f32, -1e30f32, -1e30f32);
    let mut add = |p: Offset| {
        l = l.min(p.x);
        t = t.min(p.y);
        r = r.max(p.x);
        b = b.max(p.y);
    };
    for c in commands {
        match *c {
            PathCommand::Move(p) | PathCommand::Line(p) => add(p),
            PathCommand::Quadratic(c1, p) | PathCommand::Conic(c1, p, _) => {
                add(p);
                add(c1);
            }
            PathCommand::Cubic(c1, c2, p) => {
                add(p);
                add(c1);
                add(c2);
            }
            PathCommand::Close => {}
        }
    }
    if !(l <= q.x && q.x <= r && t <= q.y && q.y <= b) {
        return false;
    }
    let mut w = 0;
    let mut on_count = 0;
    for_each_edge(commands, true, |e| w += edge_winding(&e, q, &mut on_count));
    let even_odd = fill == PathFillType::EvenOdd;
    if even_odd {
        w &= 1;
    }
    if w != 0 {
        return true;
    }
    if on_count <= 1 {
        return on_count != 0;
    }
    if (on_count & 1) != 0 || even_odd {
        return (on_count & 1) != 0;
    }
    // On an even number of edges, nonzero: inside unless they coincide (the same tangent, opposite ways).
    let mut tangents: Vec<Offset> = Vec::new();
    for_each_edge(commands, true, |e| {
        let before = tangents.len();
        match &e {
            Edge::Line(p) => tangent_line(p, q.x, q.y, &mut tangents),
            Edge::Quad(p) => tangent_quad(p, q.x, q.y, &mut tangents),
            Edge::Conic(p, w) => tangent_conic(p, *w, q.x, q.y, &mut tangents),
            Edge::Cubic(p) => tangent_cubic(p, q.x, q.y, &mut tangents),
        }
        if tangents.len() <= before {
            return;
        }
        let last = tangents.len() - 1;
        let tan = tangents[last];
        if dot(tan, tan).abs() <= NEARLY_ZERO {
            tangents.pop();
            return;
        }
        for i in 0..last {
            let test = tangents[i];
            if cross(test, tan).abs() <= NEARLY_ZERO && tan.x * test.x <= 0.0 && tan.y * test.y <= 0.0 {
                tangents.pop();
                let moved = tangents.pop().expect("the pair's other tangent");
                if i < tangents.len() {
                    tangents[i] = moved;
                }
                break;
            }
        }
    });
    !tangents.is_empty()
}

// ---- Convexity (Skia's Convexicator) -----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Change {
    Unknown,
    Left,
    Right,
    Straight,
    Backwards,
    Invalid,
}

struct Convexicator {
    first_pt: Offset,
    first_vec: Offset,
    last_pt: Offset,
    last_vec: Offset,
    expected: Change,
    reversals: i32,
}

impl Convexicator {
    fn new() -> Convexicator {
        Convexicator {
            first_pt: Offset::Zero,
            first_vec: Offset::Zero,
            last_pt: Offset::Zero,
            last_vec: Offset::Zero,
            expected: Change::Invalid,
            reversals: 0,
        }
    }
    fn set_move(&mut self, p: Offset) {
        self.first_pt = p;
        self.last_pt = p;
        self.expected = Change::Invalid;
    }
    fn add_pt(&mut self, p: Offset) -> bool {
        if same(self.last_pt, p) {
            return true;
        }
        if same(self.first_pt, self.last_pt)
            && self.expected == Change::Invalid
            && self.last_vec.x == 0.0
            && self.last_vec.y == 0.0
        {
            self.last_vec = p - self.last_pt;
            self.first_vec = self.last_vec;
        } else if !self.add_vec(p - self.last_pt) {
            return false;
        }
        self.last_pt = p;
        true
    }
    fn close(&mut self) -> bool {
        self.add_pt(self.first_pt) && self.add_vec(self.first_vec)
    }
    fn change(&self, v: Offset) -> Change {
        let c = cross(self.last_vec, v);
        if !c.is_finite() {
            return Change::Unknown;
        }
        if c == 0.0 {
            return if dot(self.last_vec, v) < 0.0 { Change::Backwards } else { Change::Straight };
        }
        if c > 0.0 { Change::Right } else { Change::Left }
    }
    fn add_vec(&mut self, v: Offset) -> bool {
        match self.change(v) {
            d @ (Change::Left | Change::Right) => {
                if self.expected == Change::Invalid {
                    self.expected = d;
                } else if d != self.expected {
                    return false;
                }
                self.last_vec = v;
            }
            Change::Straight | Change::Invalid => {}
            Change::Backwards => {
                self.last_vec = v;
                self.reversals += 1;
                return self.reversals < 3;
            }
            Change::Unknown => return false,
        }
        true
    }
    fn concave_by_sign(pts: &[Offset]) -> bool {
        if pts.len() <= 3 {
            return false;
        }
        let (mut dxes, mut dyes, mut last_sx, mut last_sy) = (0, 0, 2, 2);
        let mut curr = pts[0];
        let mut visit = |p: Offset| {
            let v = p - curr;
            if v.x != 0.0 || v.y != 0.0 {
                if !finite(v) {
                    return true;
                }
                let sx = (v.x < 0.0) as i32;
                let sy = (v.y < 0.0) as i32;
                dxes += (sx != last_sx) as i32;
                dyes += (sy != last_sy) as i32;
                if dxes > 3 || dyes > 3 {
                    return true;
                }
                last_sx = sx;
                last_sy = sy;
            }
            curr = p;
            false
        };
        for &p in &pts[1..] {
            if visit(p) {
                return true;
            }
        }
        visit(pts[0]) // round to the first again
    }
}

/// Skia's convexity: one contour (trailing moves aside) that turns one way all round.
pub(crate) fn convex(commands: &[PathCommand]) -> bool {
    let mut n = commands.len();
    while n > 0 && matches!(commands[n - 1], PathCommand::Move(_)) {
        n -= 1;
    }
    if n == 0 {
        return true;
    }
    let commands = &commands[..n];
    // All the points, control points too.
    let mut points = Vec::new();
    for c in commands {
        match *c {
            PathCommand::Move(p) | PathCommand::Line(p) => points.push(p),
            PathCommand::Quadratic(c1, p) | PathCommand::Conic(c1, p, _) => points.extend([c1, p]),
            PathCommand::Cubic(c1, c2, p) => points.extend([c1, c2, p]),
            PathCommand::Close => {}
        }
    }
    if Convexicator::concave_by_sign(&points) {
        return false;
    }
    let mut contours = 0;
    let mut needs_close = false;
    let mut state = Convexicator::new();
    for c in commands {
        if contours == 0 {
            if let PathCommand::Move(p) = *c {
                state.set_move(p);
            } else {
                contours += 1;
                needs_close = true;
            }
        }
        if contours == 1 {
            match *c {
                PathCommand::Close | PathCommand::Move(_) => {
                    if !state.close() {
                        return false;
                    }
                    needs_close = false;
                    contours += 1;
                }
                PathCommand::Line(p) => {
                    if !state.add_pt(p) {
                        return false;
                    }
                }
                PathCommand::Quadratic(c1, p) | PathCommand::Conic(c1, p, _) => {
                    if !state.add_pt(c1) || !state.add_pt(p) {
                        return false;
                    }
                }
                PathCommand::Cubic(c1, c2, p) => {
                    if !state.add_pt(c1) || !state.add_pt(c2) || !state.add_pt(p) {
                        return false;
                    }
                }
            }
        } else if contours > 1 && !matches!(c, PathCommand::Move(_)) {
            return false; // a second contour
        }
    }
    if needs_close && !state.close() {
        return false;
    }
    !(state.expected == Change::Invalid && state.reversals >= 3) // (no way round, but back and forth)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(x: f32, y: f32) -> Offset {
        Offset::new(x, y)
    }
    fn near(a: Offset, b: Offset, tolerance: f32) -> bool {
        (a.x - b.x).abs() <= tolerance && (a.y - b.y).abs() <= tolerance
    }

    #[test]
    fn unit_quad_roots_are_in_order_and_inside() {
        // (t - 0.25)(t - 0.75) = t^2 - t + 0.1875
        assert_eq!(unit_quad_roots(1.0, -1.0, 0.1875), vec![0.25, 0.75]);
        // A double root once; roots outside (0, 1) left out; a line's root.
        assert_eq!(unit_quad_roots(1.0, -1.0, 0.25), vec![0.5]);
        assert_eq!(unit_quad_roots(1.0, -3.0, 2.0), Vec::<f32>::new()); // 1 and 2
        assert_eq!(unit_quad_roots(0.0, 2.0, -1.0), vec![0.5]);
        assert!(unit_quad_roots(1.0, 0.0, 1.0).is_empty()); // none real
    }

    #[test]
    fn chops_meet_the_curve() {
        let q = [o(0.0, 0.0), o(50.0, 100.0), o(100.0, 0.0)];
        let halves = chop_quad_at(&q, 0.3);
        assert!(near(halves[2], Conic::new(q, 1.0).at(0.3), 1e-4));
        let c = [o(0.0, 0.0), o(0.0, 100.0), o(100.0, 100.0), o(100.0, 0.0)];
        let parts = chop_cubic_at(&c, 0.4);
        assert!(near(parts[3], cubic_at(&c, 0.4), 1e-4));
        // The piece between 0.2 and 0.6: its ends on the curve, its middle the curve's at 0.4.
        let piece = chop_cubic_between(&c, 0.2, 0.6);
        assert!(near(piece[0], cubic_at(&c, 0.2), 1e-4) && near(piece[3], cubic_at(&c, 0.6), 1e-4));
        assert!(near(cubic_at(&piece, 0.5), cubic_at(&c, 0.4), 1e-3));
    }

    #[test]
    fn y_extrema_split_curves_monotonic() {
        let q = [o(0.0, 0.0), o(50.0, 100.0), o(100.0, 0.0)];
        let mut dst = [Offset::Zero; 5];
        assert_eq!(chop_quad_at_y_extrema(&q, &mut dst), 1);
        assert!(dst[1].y == dst[2].y && dst[3].y == dst[2].y && (dst[2].y - 50.0).abs() < 1e-4);
        let c = [o(0.0, 50.0), o(30.0, -20.0), o(70.0, 120.0), o(100.0, 50.0)];
        let mut ten = [Offset::Zero; 10];
        assert_eq!(chop_cubic_at_y_extrema(&c, &mut ten), 2);
        for i in 0..3 {
            let piece = first4(&ten[i * 3..]);
            let ys: Vec<f32> = (0..=10).map(|k| cubic_at(&piece, k as f32 / 10.0).y).collect();
            assert!(ys.windows(2).all(|w| w[1] >= w[0] - 1e-3) || ys.windows(2).all(|w| w[1] <= w[0] + 1e-3));
        }
        let t = chop_mono_cubic_at_y(&first4(&ten[3..]), 50.0).unwrap();
        assert!((cubic_at(&first4(&ten[3..]), t).y - 50.0).abs() < 0.01);
    }

    #[test]
    fn conics_are_exact_arcs() {
        // A quarter of the unit circle: every point on it, its halves too.
        let k = Conic::new([o(1.0, 0.0), o(1.0, 1.0), o(0.0, 1.0)], std::f32::consts::FRAC_1_SQRT_2);
        for i in 0..=8 {
            assert!((k.at(i as f32 / 8.0).get_distance() - 1.0).abs() < 1e-6);
        }
        let [a, b] = k.chop();
        assert!((a.w - (std::f32::consts::PI / 8.0).cos()).abs() < 1e-6 && a.p[2] == b.p[0]);
        assert!((a.at(0.5).get_distance() - 1.0).abs() < 1e-6);
        let [c, d] = k.chop_at(0.25).unwrap();
        assert!(near(c.p[2], k.at(0.25), 1e-6) && near(d.p[2], o(0.0, 1.0), 0.0));
        let piece = k.between(0.25, 0.75);
        assert!(near(piece.p[0], k.at(0.25), 1e-5) && near(piece.p[2], k.at(0.75), 1e-5));
        assert!((piece.at(0.5).get_distance() - 1.0).abs() < 1e-5);
        // Its quadratics: more of them for a finer tolerance, their points on the circle.
        assert!(k.quad_pow2(0.25) < k.quad_pow2(0.0001));
        let pts = k.chop_into_quads_pow2(3);
        assert_eq!(pts.len(), 17);
        for i in (0..17).step_by(2) {
            assert!((pts[i].get_distance() - 1.0).abs() < 1e-5);
        }
        // Through a perspective map w = 1 + x / 100: the weight w1 / sqrt(w0 w2).
        let m = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.01, 0.0, 1.0];
        let w = Conic::transformed_weight(&[o(0.0, 0.0), o(50.0, 50.0), o(100.0, 0.0)], 1.0, &m);
        assert!((w - 1.5 / 2f32.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn unit_arcs_turn_from_start_to_stop() {
        let id = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        // A quarter clockwise (y down): one conic from (1, 0) to (0, 1).
        let q = Conic::unit_arc(o(1.0, 0.0), o(0.0, 1.0), true, &id);
        assert_eq!(q.len(), 1);
        assert!(near(q[0].p[2], o(0.0, 1.0), 1e-6) && near(q[0].p[1], o(1.0, 1.0), 1e-6));
        // Three quarters counterclockwise from the right: through the top.
        let three = Conic::unit_arc(o(1.0, 0.0), o(0.0, 1.0), false, &id);
        assert_eq!(three.len(), 3);
        assert!(near(three[0].p[2], o(0.0, -1.0), 1e-6) && near(three[2].p[2], o(0.0, 1.0), 1e-6));
        // The same vector: none.
        assert!(Conic::unit_arc(o(1.0, 0.0), o(1.0, 0.0), true, &id).is_empty());
        // Mapped onto an ellipse.
        let m = [10.0, 0.0, 5.0, 0.0, 20.0, 5.0];
        let e = Conic::unit_arc(o(1.0, 0.0), o(0.0, 1.0), true, &m);
        assert!(near(e[0].p[0], o(15.0, 5.0), 1e-5) && near(e[0].p[2], o(5.0, 25.0), 1e-5));
    }

    #[test]
    fn winding_counts_crossings() {
        let square = [
            PathCommand::Move(o(0.0, 0.0)),
            PathCommand::Line(o(10.0, 0.0)),
            PathCommand::Line(o(10.0, 10.0)),
            PathCommand::Line(o(0.0, 10.0)),
            PathCommand::Close,
        ];
        assert_eq!(winding(&square, o(5.0, 5.0), true).abs(), 1);
        assert_eq!(winding(&square, o(15.0, 5.0), true), 0);
        // Open, it isn't closed unless asked.
        let open = &square[..4];
        assert_eq!(winding(open, o(5.0, 5.0), false), 0);
        assert_eq!(winding(open, o(5.0, 5.0), true).abs(), 1);
        // A point on an edge: inside.
        assert!(contains(&square, PathFillType::NonZero, o(10.0, 5.0)));
        assert!(!contains(&square, PathFillType::NonZero, o(10.5, 5.0)));
    }

    #[test]
    fn convexity_is_skias() {
        let tri = [
            PathCommand::Move(o(0.0, 0.0)),
            PathCommand::Line(o(10.0, 0.0)),
            PathCommand::Line(o(0.0, 10.0)),
            PathCommand::Close,
            PathCommand::Move(o(5.0, 5.0)),
        ];
        assert!(convex(&tri)); // (a trailing move aside)
        let dent = [
            PathCommand::Move(o(0.0, 0.0)),
            PathCommand::Line(o(10.0, 0.0)),
            PathCommand::Line(o(5.0, 5.0)),
            PathCommand::Line(o(10.0, 10.0)),
            PathCommand::Line(o(0.0, 10.0)),
            PathCommand::Close,
        ];
        assert!(!convex(&dent));
    }
}

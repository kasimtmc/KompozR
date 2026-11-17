//! Roots and bounds of Bézier curves along one axis (Compose's Bezier.kt: evaluateCubic, findFirstCubicRoot,
//! computeCubicVerticalBounds, what Compose's CubicBezierEasing solves its curves with; and over a path's segment
//! evaluateY, findFirstRoot, computeHorizontalBounds, what its PathEasing solves with).

use super::iterator::{PathSegment, PathSegmentType};
use std::f64::consts::PI;

const Tau: f64 = PI * 2.0;
const Epsilon: f64 = 1e-7;
/// Roots this far outside 0..1 are taken as its ends (Compose's FloatEpsilon).
const FloatEpsilon: f32 = 1.05e-6;

fn close_to(a: f64, b: f64) -> bool {
    (a - b).abs() < Epsilon
}

/// Compose's fastCbrt: the cube root from the float's bits, two Newton-Raphson steps after.
fn fast_cbrt(x: f32) -> f32 {
    // (Kotlin's toRawBits().toLong() and 0x1ffffffffL: a negative one's sign carried into bit 32)
    let v = (x.to_bits() as i32 as i64) & 0x1_ffff_ffff;
    let mut estimate = f32::from_bits(0x2a51_0554i32.wrapping_add((v / 3) as i32) as u32);
    estimate -= (estimate - x / (estimate * estimate)) * (1.0 / 3.0);
    estimate -= (estimate - x / (estimate * estimate)) * (1.0 / 3.0);
    estimate
}

/// `r` if it is within 0..1 (within FloatEpsilon of it: kept to it), else NaN.
fn clamp_valid_root_in_unit_range(r: f32) -> f32 {
    // (NaN stays NaN, as with Kotlin's fastCoerceIn)
    let s = r.clamp(0.0, 1.0);
    if (s - r).abs() > FloatEpsilon { f32::NAN } else { s }
}

/// `r` written at `roots[index]` (NaN too); 1 if it is a root within 0..1, else 0.
fn write_valid_root_in_unit_range(r: f32, roots: &mut [f32], index: usize) -> usize {
    let v = clamp_valid_root_in_unit_range(r);
    roots[index] = v;
    if v.is_nan() { 0 } else { 1 }
}

fn find_line_root(p0: f32, p1: f32, roots: &mut [f32], index: usize) -> usize {
    write_valid_root_in_unit_range(-p0 / (p1 - p0), roots, index)
}

/// The roots within 0..1 of the quadratic Bézier of `p0`, `p1`, `p2`, sorted, an equal pair once.
fn find_quadratic_roots(p0: f32, p1: f32, p2: f32, roots: &mut [f32], index: usize) -> usize {
    let (a, b, c) = (p0 as f64, p1 as f64, p2 as f64);
    let d = a - 2.0 * b + c;
    let mut root_count = 0;
    if d != 0.0 {
        let v1 = -(b * b - a * c).sqrt();
        let v2 = -a + b;
        root_count += write_valid_root_in_unit_range((-(v1 + v2) / d) as f32, roots, index);
        root_count += write_valid_root_in_unit_range(((v1 - v2) / d) as f32, roots, index + root_count);
        if root_count > 1 {
            let (s, t) = (roots[index], roots[index + 1]);
            if s > t {
                roots[index] = t;
                roots[index + 1] = s;
            } else if s == t {
                root_count -= 1;
            }
        }
    } else if b != c {
        root_count += write_valid_root_in_unit_range(((2.0 * b - c) / (2.0 * b - 2.0 * c)) as f32, roots, index);
    }
    root_count
}

/// The cubic Bézier of `p0`..`p3` at `t`.
fn evaluate_cubic4(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let a = p3 + 3.0 * (p1 - p2) - p0;
    let b = 3.0 * (p2 - 2.0 * p1 + p0);
    let c = 3.0 * (p1 - p0);
    ((a * t + b) * t + c) * t + p0
}

/// The cubic Bézier from 0 to 1 whose control points are `p1` and `p2`, at `t` (Compose's evaluateCubic).
pub fn evaluate_cubic(p1: f32, p2: f32, t: f32) -> f32 {
    let a = 1.0 / 3.0 + (p1 - p2);
    let b = p2 - 2.0 * p1;
    let c = p1;
    3.0 * ((a * t + b) * t + c) * t
}

/// The first root within 0..1 of the cubic Bézier of `p0`..`p3` (one axis's coordinates), NaN if there is none
/// (Compose's findFirstCubicRoot: Cardano's method, as "A Primer on Bézier Curves" describes it).
pub fn find_first_cubic_root(p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    let mut a = 3.0 * (p0 as f64 - 2.0 * p1 as f64 + p2 as f64);
    let mut b = 3.0 * (p1 - p0) as f64;
    let mut c = p0 as f64;
    let d = -p0 as f64 + 3.0 * (p1 - p2) as f64 + p3 as f64;

    // Not a cubic.
    if close_to(d, 0.0) {
        // Not a quadratic.
        if close_to(a, 0.0) {
            if close_to(b, 0.0) {
                return f32::NAN;
            }
            return clamp_valid_root_in_unit_range((-c / b) as f32);
        }
        let q = (b * b - 4.0 * a * c).sqrt();
        let a2 = 2.0 * a;
        let root = clamp_valid_root_in_unit_range(((q - b) / a2) as f32);
        if !root.is_nan() {
            return root;
        }
        return clamp_valid_root_in_unit_range(((-b - q) / a2) as f32);
    }

    a /= d;
    b /= d;
    c /= d;
    let o3 = (3.0 * b - a * a) / 9.0;
    let q2 = (2.0 * a * a * a - 9.0 * a * b + 27.0 * c) / 54.0;
    let discriminant = q2 * q2 + o3 * o3 * o3;
    let a3 = a / 3.0;

    if discriminant < 0.0 {
        let mp33 = -(o3 * o3 * o3);
        let r = mp33.sqrt();
        let t = -q2 / r;
        let cos_phi = t.clamp(-1.0, 1.0);
        let phi = cos_phi.acos();
        let t1 = 2.0f32 * fast_cbrt(r as f32);
        let root = clamp_valid_root_in_unit_range((t1 as f64 * (phi / 3.0).cos() - a3) as f32);
        if !root.is_nan() {
            return root;
        }
        let root = clamp_valid_root_in_unit_range((t1 as f64 * ((phi + Tau) / 3.0).cos() - a3) as f32);
        if !root.is_nan() {
            return root;
        }
        return clamp_valid_root_in_unit_range((t1 as f64 * ((phi + 2.0 * Tau) / 3.0).cos() - a3) as f32);
    } else if discriminant == 0.0 {
        let u1 = -fast_cbrt(q2 as f32);
        let root = clamp_valid_root_in_unit_range(2.0 * u1 - a3 as f32);
        if !root.is_nan() {
            return root;
        }
        return clamp_valid_root_in_unit_range(-u1 - a3 as f32);
    }

    let sd = discriminant.sqrt();
    let u1 = fast_cbrt((-q2 + sd) as f32);
    let v1 = fast_cbrt((q2 + sd) as f32);
    clamp_valid_root_in_unit_range(((u1 - v1) as f64 - a3) as f32)
}

/// The least and greatest of the cubic Bézier of `p0y`..`p3y` (one axis's coordinates) over 0..1 (Compose's
/// computeCubicVerticalBounds, its roots found in an array of its own).
pub fn compute_cubic_vertical_bounds(p0y: f32, p1y: f32, p2y: f32, p3y: f32) -> (f32, f32) {
    let mut roots = [0.0f32; 5];
    // The derivative, a quadratic.
    let d0 = 3.0 * (p1y - p0y);
    let d1 = 3.0 * (p2y - p1y);
    let d2 = 3.0 * (p3y - p2y);
    let mut count = find_quadratic_roots(d0, d1, d2, &mut roots, 0);
    // The second derivative, a line.
    let dd0 = 2.0 * (d1 - d0);
    let dd1 = 2.0 * (d2 - d1);
    count += find_line_root(dd0, dd1, &mut roots, count);

    let mut min_y = p0y.min(p3y);
    let mut max_y = p0y.max(p3y);
    for &t in &roots[..count] {
        let y = evaluate_cubic4(p0y, p1y, p2y, p3y, t);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    (min_y, max_y)
}

// ---- A path segment along one axis (Compose's Bezier.kt over PathSegment: what PathEasing solves with) ----------

/// The coordinate `axis` (0: x, 1: y) of a segment's point `i`.
fn coord(segment: &PathSegment, i: usize, axis: usize) -> f32 {
    let p = segment.points[i];
    if axis == 0 { p.x } else { p.y }
}

/// The index of a segment's last point (Compose's endX/endY: 0 for a move, a close, a done).
fn end_index(segment: &PathSegment) -> usize {
    match segment.r#type {
        PathSegmentType::Line => 1,
        PathSegmentType::Quadratic | PathSegmentType::Conic => 2,
        PathSegmentType::Cubic => 3,
        _ => 0,
    }
}

fn evaluate_line(p0: f32, p1: f32, t: f32) -> f32 {
    (p1 - p0) * t + p0
}

fn evaluate_quadratic(p0: f32, p1: f32, p2: f32, t: f32) -> f32 {
    let by = 2.0 * (p1 - p0);
    let ay = p2 - 2.0 * p1 + p0;
    (ay * t + by) * t + p0
}

/// A segment's coordinate `axis` at `t` (Compose's evaluateX, evaluateY): NaN for a conic, a close, a done.
fn evaluate_axis(segment: &PathSegment, t: f32, axis: usize) -> f32 {
    let c = |i| coord(segment, i, axis);
    match segment.r#type {
        PathSegmentType::Move => c(0),
        PathSegmentType::Line => evaluate_line(c(0), c(1), t),
        PathSegmentType::Quadratic => evaluate_quadratic(c(0), c(1), c(2), t),
        PathSegmentType::Cubic => evaluate_cubic4(c(0), c(1), c(2), c(3), t),
        _ => f32::NAN,
    }
}

/// The y coordinate of `segment`'s curve at `t` (Compose's evaluateY): NaN for a conic, a close, a done.
pub fn evaluate_y(segment: &PathSegment, t: f32) -> f32 {
    evaluate_axis(segment, t, 1)
}

/// The first root within 0..1 of a quadratic Bézier (Compose's findFirstQuadraticRoot), NaN if none.
fn find_first_quadratic_root(p0: f32, p1: f32, p2: f32) -> f32 {
    let (a, b, c) = (p0 as f64, p1 as f64, p2 as f64);
    let d = a - 2.0 * b + c;
    if d != 0.0 {
        let v1 = -(b * b - a * c).sqrt();
        let v2 = -a + b;
        let root = clamp_valid_root_in_unit_range((-(v1 + v2) / d) as f32);
        if !root.is_nan() {
            return root;
        }
        return clamp_valid_root_in_unit_range(((v1 - v2) / d) as f32);
    } else if b != c {
        return clamp_valid_root_in_unit_range(((2.0 * b - c) / (2.0 * b - 2.0 * c)) as f32);
    }
    f32::NAN
}

/// Where along `segment` (0..1) its x is `fraction`, the first such place; NaN if there is none (Compose's
/// findFirstRoot: conics, closes and dones have none).
pub fn find_first_root(segment: &PathSegment, fraction: f32) -> f32 {
    let x = |i| coord(segment, i, 0) - fraction;
    match segment.r#type {
        PathSegmentType::Line => clamp_valid_root_in_unit_range(-x(0) / (x(1) - x(0))),
        PathSegmentType::Quadratic => find_first_quadratic_root(x(0), x(1), x(2)),
        PathSegmentType::Cubic => find_first_cubic_root(x(0), x(1), x(2), x(3)),
        _ => f32::NAN,
    }
}

/// The roots within 0..1 of a segment's derivative along `axis` (and, for a cubic, of its second derivative),
/// written into `roots` from `index`; how many (Compose's findDerivativeRoots).
fn find_derivative_roots(segment: &PathSegment, axis: usize, roots: &mut [f32], index: usize) -> usize {
    let c = |i| coord(segment, i, axis);
    match segment.r#type {
        PathSegmentType::Quadratic => {
            let d0 = 2.0 * (c(1) - c(0));
            let d1 = 2.0 * (c(2) - c(1));
            find_line_root(d0, d1, roots, index)
        }
        PathSegmentType::Cubic => {
            let d0 = 3.0 * (c(1) - c(0));
            let d1 = 3.0 * (c(2) - c(1));
            let d2 = 3.0 * (c(3) - c(2));
            let count = find_quadratic_roots(d0, d1, d2, roots, index);
            let dd0 = 2.0 * (d1 - d0);
            let dd1 = 2.0 * (d2 - d1);
            count + find_line_root(dd0, dd1, roots, index + count)
        }
        _ => 0,
    }
}

/// The least and greatest x of `segment` (Compose's computeHorizontalBounds; `roots`, at least 5 long, is scratch).
pub fn compute_horizontal_bounds(segment: &PathSegment, roots: &mut [f32]) -> (f32, f32) {
    let count = find_derivative_roots(segment, 0, roots, 0);
    let (start, end) = (coord(segment, 0, 0), coord(segment, end_index(segment), 0));
    let mut min_x = start.min(end);
    let mut max_x = start.max(end);
    for &t in &roots[..count] {
        let x = evaluate_axis(segment, t, 0);
        min_x = min_x.min(x);
        max_x = max_x.max(x);
    }
    (min_x, max_x)
}

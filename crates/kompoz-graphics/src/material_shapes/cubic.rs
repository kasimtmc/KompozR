//! Cubic Bézier curves (androidx.graphics.shapes' Cubic and MutableCubic) and the point transformers that move
//! them.

use super::utils::*;
use std::fmt;
use std::ops::{Add, Deref, Div, Mul};

/// What a [`PointTransformer`] gives: the point moved (Compose's TransformResult, a float pair).
pub type TransformResult = (f32, f32);

/// What moves points (Compose's PointTransformer): rotates, scales, translates… Any `Fn(f32, f32) -> (f32, f32)`
/// is one.
pub trait PointTransformer {
    fn transform(&self, x: f32, y: f32) -> TransformResult;
}

impl<F: Fn(f32, f32) -> TransformResult> PointTransformer for F {
    fn transform(&self, x: f32, y: f32) -> TransformResult {
        self(x, y)
    }
}

/// A point that can be changed (Compose's MutablePoint).
pub trait MutablePoint {
    fn x(&self) -> f32;
    fn set_x(&mut self, x: f32);
    fn y(&self) -> f32;
    fn set_y(&mut self, y: f32);
}

/// One cubic Bézier curve (Compose's Cubic): from its first anchor, pulled toward its two control points, to its
/// second anchor. Equal by its points.
#[derive(Clone, Copy, PartialEq)]
pub struct Cubic {
    pub(crate) points: [f32; 8],
}

/// A cubic of these anchor and control points (Compose's Cubic(…)).
#[allow(clippy::too_many_arguments)]
pub fn Cubic(
    anchor0_x: f32,
    anchor0_y: f32,
    control0_x: f32,
    control0_y: f32,
    control1_x: f32,
    control1_y: f32,
    anchor1_x: f32,
    anchor1_y: f32,
) -> Cubic {
    Cubic { points: [anchor0_x, anchor0_y, control0_x, control0_y, control1_x, control1_y, anchor1_x, anchor1_y] }
}

impl Cubic {
    /// Its 8 values: anchor 0, control 0, control 1, anchor 1, x and y each.
    pub const fn from_points(points: [f32; 8]) -> Cubic {
        Cubic { points }
    }
    pub(crate) fn of_points(anchor0: Point, control0: Point, control1: Point, anchor1: Point) -> Cubic {
        Cubic(anchor0.x, anchor0.y, control0.x, control0.y, control1.x, control1.y, anchor1.x, anchor1.y)
    }

    pub fn points(&self) -> &[f32; 8] {
        &self.points
    }
    pub fn anchor0_x(&self) -> f32 {
        self.points[0]
    }
    pub fn anchor0_y(&self) -> f32 {
        self.points[1]
    }
    pub fn control0_x(&self) -> f32 {
        self.points[2]
    }
    pub fn control0_y(&self) -> f32 {
        self.points[3]
    }
    pub fn control1_x(&self) -> f32 {
        self.points[4]
    }
    pub fn control1_y(&self) -> f32 {
        self.points[5]
    }
    pub fn anchor1_x(&self) -> f32 {
        self.points[6]
    }
    pub fn anchor1_y(&self) -> f32 {
        self.points[7]
    }

    /// The point `t` of the way along it (0 its first anchor, 1 its second).
    pub fn point_on_curve(&self, t: f32) -> Point {
        let u = 1.0 - t;
        Point::new(
            self.anchor0_x() * (u * u * u)
                + self.control0_x() * (3.0 * t * u * u)
                + self.control1_x() * (3.0 * t * t * u)
                + self.anchor1_x() * (t * t * t),
            self.anchor0_y() * (u * u * u)
                + self.control0_y() * (3.0 * t * u * u)
                + self.control1_y() * (3.0 * t * t * u)
                + self.anchor1_y() * (t * t * t),
        )
    }

    pub(crate) fn zero_length(&self) -> bool {
        (self.anchor0_x() - self.anchor1_x()).abs() < DISTANCE_EPSILON
            && (self.anchor0_y() - self.anchor1_y()).abs() < DISTANCE_EPSILON
    }

    pub(crate) fn convex_to(&self, next: &Cubic) -> bool {
        let prev_vertex = Point::new(self.anchor0_x(), self.anchor0_y());
        let curr_vertex = Point::new(self.anchor1_x(), self.anchor1_y());
        let next_vertex = Point::new(next.anchor1_x(), next.anchor1_y());
        convex(prev_vertex, curr_vertex, next_vertex)
    }

    /// Its bounds, left, top, right, bottom: its own (where its derivative is 0), or (`approximate`) its anchors'
    /// and controls'.
    pub fn calculate_bounds(&self, approximate: bool) -> [f32; 4] {
        // (a curve of no length: the point)
        if self.zero_length() {
            return [self.anchor0_x(), self.anchor0_y(), self.anchor0_x(), self.anchor0_y()];
        }
        let mut min_x = self.anchor0_x().min(self.anchor1_x());
        let mut min_y = self.anchor0_y().min(self.anchor1_y());
        let mut max_x = self.anchor0_x().max(self.anchor1_x());
        let mut max_y = self.anchor0_y().max(self.anchor1_y());
        if approximate {
            return [
                min_x.min(self.control0_x().min(self.control1_x())),
                min_y.min(self.control0_y().min(self.control1_y())),
                max_x.max(self.control0_x().max(self.control1_x())),
                max_y.max(self.control0_y().max(self.control1_y())),
            ];
        }
        // The derivative is a quadratic: its roots by the quadratic formula (or Muller's where a is 0).
        let roots = |a: f32, b: f32, c: f32| -> Vec<f32> {
            if a.abs() < DISTANCE_EPSILON {
                if b != 0.0 { vec![2.0 * c / (-2.0 * b)] } else { vec![] }
            } else {
                let s = b * b - 4.0 * a * c;
                if s >= 0.0 { vec![(-b + s.sqrt()) / (2.0 * a), (-b - s.sqrt()) / (2.0 * a)] } else { vec![] }
            }
        };
        let (a0x, c0x, c1x, a1x) = (self.anchor0_x(), self.control0_x(), self.control1_x(), self.anchor1_x());
        let xa = -a0x + 3.0 * c0x - 3.0 * c1x + a1x;
        let xb = 2.0 * a0x - 4.0 * c0x + 2.0 * c1x;
        let xc = -a0x + c0x;
        for t in roots(xa, xb, xc) {
            if (0.0..=1.0).contains(&t) {
                let x = self.point_on_curve(t).x;
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
            }
        }
        let (a0y, c0y, c1y, a1y) = (self.anchor0_y(), self.control0_y(), self.control1_y(), self.anchor1_y());
        let ya = -a0y + 3.0 * c0y - 3.0 * c1y + a1y;
        let yb = 2.0 * a0y - 4.0 * c0y + 2.0 * c1y;
        let yc = -a0y + c0y;
        for t in roots(ya, yb, yc) {
            if (0.0..=1.0).contains(&t) {
                let y = self.point_on_curve(t).y;
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
        [min_x, min_y, max_x, max_y]
    }

    /// Two curves, this one split `t` of the way along.
    pub fn split(&self, t: f32) -> (Cubic, Cubic) {
        let u = 1.0 - t;
        let p = self.point_on_curve(t);
        let (a0x, a0y, c0x, c0y) = (self.anchor0_x(), self.anchor0_y(), self.control0_x(), self.control0_y());
        let (c1x, c1y, a1x, a1y) = (self.control1_x(), self.control1_y(), self.anchor1_x(), self.anchor1_y());
        (
            Cubic(
                a0x,
                a0y,
                a0x * u + c0x * t,
                a0y * u + c0y * t,
                a0x * (u * u) + c0x * (2.0 * u * t) + c1x * (t * t),
                a0y * (u * u) + c0y * (2.0 * u * t) + c1y * (t * t),
                p.x,
                p.y,
            ),
            Cubic(
                p.x,
                p.y,
                c0x * (u * u) + c1x * (2.0 * u * t) + a1x * (t * t),
                c0y * (u * u) + c1y * (2.0 * u * t) + a1y * (t * t),
                c1x * u + a1x * t,
                c1y * u + a1y * t,
                a1x,
                a1y,
            ),
        )
    }

    /// The same curve the other way.
    pub fn reverse(&self) -> Cubic {
        Cubic(
            self.anchor1_x(),
            self.anchor1_y(),
            self.control1_x(),
            self.control1_y(),
            self.control0_x(),
            self.control0_y(),
            self.anchor0_x(),
            self.anchor0_y(),
        )
    }

    /// Its points moved by `f`.
    pub fn transformed(&self, f: &dyn PointTransformer) -> Cubic {
        let mut c = MutableCubic { cubic: *self };
        c.transform(f);
        c.cubic
    }

    /// A straight line from (x0, y0) to (x1, y1), its controls a third of the way from each end.
    pub fn straight_line(x0: f32, y0: f32, x1: f32, y1: f32) -> Cubic {
        Cubic(
            x0,
            y0,
            interpolate(x0, x1, 1.0 / 3.0),
            interpolate(y0, y1, 1.0 / 3.0),
            interpolate(x0, x1, 2.0 / 3.0),
            interpolate(y0, y1, 2.0 / 3.0),
            x1,
            y1,
        )
    }

    /// Close to the shorter circular arc round the center from (x0, y0) to (x1, y1) (equally far from it).
    pub fn circular_arc(center_x: f32, center_y: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> Cubic {
        let p0d = direction_vector(x0 - center_x, y0 - center_y);
        let p1d = direction_vector(x1 - center_x, y1 - center_y);
        let rotated_p0 = rotate90(p0d);
        let rotated_p1 = rotate90(p1d);
        let clockwise = dot(rotated_p0, Point::new(x1 - center_x, y1 - center_y)) >= 0.0;
        let cosa = dot(p0d, p1d);
        // (p0 ~= p1)
        if cosa > 0.999 {
            return Cubic::straight_line(x0, y0, x1, y1);
        }
        let k = distance(x0 - center_x, y0 - center_y) * 4.0 / 3.0
            * ((2.0 * (1.0 - cosa)).sqrt() - (1.0 - cosa * cosa).sqrt())
            / (1.0 - cosa)
            * if clockwise { 1.0 } else { -1.0 };
        Cubic(
            x0,
            y0,
            x0 + rotated_p0.x * k,
            y0 + rotated_p0.y * k,
            x1 - rotated_p1.x * k,
            y1 - rotated_p1.y * k,
            x1,
            y1,
        )
    }

    /// A curve of no length at (x0, y0).
    pub(crate) fn empty(x0: f32, y0: f32) -> Cubic {
        Cubic(x0, y0, x0, y0, x0, y0, x0, y0)
    }
}

impl Add for Cubic {
    type Output = Cubic;
    fn add(self, o: Cubic) -> Cubic {
        Cubic { points: std::array::from_fn(|i| self.points[i] + o.points[i]) }
    }
}
impl Mul<f32> for Cubic {
    type Output = Cubic;
    fn mul(self, x: f32) -> Cubic {
        Cubic { points: self.points.map(|p| p * x) }
    }
}
impl Mul<i32> for Cubic {
    type Output = Cubic;
    fn mul(self, x: i32) -> Cubic {
        self * x as f32
    }
}
impl Div<f32> for Cubic {
    type Output = Cubic;
    fn div(self, x: f32) -> Cubic {
        self * (1.0 / x)
    }
}
impl Div<i32> for Cubic {
    type Output = Cubic;
    fn div(self, x: i32) -> Cubic {
        self / x as f32
    }
}

/// Compose's toString.
impl fmt::Debug for Cubic {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "anchor0: ({:?}, {:?}) control0: ({:?}, {:?}), control1: ({:?}, {:?}), anchor1: ({:?}, {:?})",
            self.anchor0_x(),
            self.anchor0_y(),
            self.control0_x(),
            self.control0_y(),
            self.control1_x(),
            self.control1_y(),
            self.anchor1_x(),
            self.anchor1_y()
        )
    }
}

/// A cubic changed in place (Compose's MutableCubic), as a morph hands out each of its curves in turn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MutableCubic {
    cubic: Cubic,
}

impl Default for MutableCubic {
    fn default() -> MutableCubic {
        MutableCubic::new()
    }
}

impl MutableCubic {
    /// All its points at the origin.
    pub fn new() -> MutableCubic {
        MutableCubic { cubic: Cubic { points: [0.0; 8] } }
    }
    /// Its points moved by `f`.
    pub fn transform(&mut self, f: &dyn PointTransformer) {
        for ix in [0, 2, 4, 6] {
            let (x, y) = f.transform(self.cubic.points[ix], self.cubic.points[ix + 1]);
            self.cubic.points[ix] = x;
            self.cubic.points[ix + 1] = y;
        }
    }
    /// `progress` of the way from `c1` to `c2`, point by point.
    pub fn interpolate(&mut self, c1: &Cubic, c2: &Cubic, progress: f32) {
        for i in 0..8 {
            self.cubic.points[i] = interpolate(c1.points[i], c2.points[i], progress);
        }
    }
}

impl Deref for MutableCubic {
    type Target = Cubic;
    fn deref(&self) -> &Cubic {
        &self.cubic
    }
}

//! Outline representation and helpers for autohinting.

use super::super::raster::{FixedOutline, FixedPoint, PointTag};
use super::metrics::Scale;
use super::unscaled::clamp_coord;
use skrifa::raw::limits::MAX_OUTLINE_POINTS;
use skrifa::raw::model::glyph::outline::UnscaledOutlineSink;
use skrifa::raw::tables::glyf::{PointFlags, PointMarker};
use std::ops::Range;

/// Hinting directions.
///
/// The values are such that `dir1 + dir2 == 0` when the directions are
/// opposite.
///
// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.h#L45>
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[repr(i8)]
pub enum Direction {
    /// Undetermined direction.
    #[default]
    None = 4,
    /// Toward the right.
    Right = 1,
    /// Toward the left.
    Left = -1,
    /// Toward the top.
    Up = 2,
    /// Toward the bottom.
    Down = -2,
}

impl Direction {
    /// Computes a direction from a vector.
    ///
    /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L751>
    pub fn new(dx: i32, dy: i32) -> Self {
        let (dir, long_arm, short_arm) = if dy >= dx {
            if dy >= -dx { (Direction::Up, dy, dx) } else { (Direction::Left, -dx, dy) }
        } else if dy >= -dx {
            (Direction::Right, dx, dy)
        } else {
            (Direction::Down, -dy, dx)
        };
        // Return no direction if arm lengths do not differ enough.
        // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L789>
        if long_arm <= 14 * short_arm.abs() { Direction::None } else { dir }
    }

    pub fn is_opposite(self, other: Self) -> bool {
        self as i8 + other as i8 == 0
    }

    pub fn is_same_axis(self, other: Self) -> bool {
        (self as i8).abs() == (other as i8).abs()
    }

    pub(crate) fn normalize(self) -> Self {
        // FreeType uses absolute value for this.
        match self {
            Self::Left => Self::Right,
            Self::Down => Self::Up,
            _ => self,
        }
    }
}

/// The overall orientation of an outline.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Orientation {
    Clockwise,
    CounterClockwise,
}

/// Outline point with a lot of context for hinting.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.h#L239>
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub(crate) struct Point {
    /// Describes the type and hinting state of the point.
    pub flags: PointFlags,
    /// X coordinate in font units.
    pub fx: i32,
    /// Y coordinate in font units.
    pub fy: i32,
    /// Scaled X coordinate.
    pub ox: i32,
    /// Scaled Y coordinate.
    pub oy: i32,
    /// Hinted X coordinate.
    pub x: i32,
    /// Hinted Y coordinate.
    pub y: i32,
    /// Direction of inwards vector.
    pub in_dir: Direction,
    /// Direction of outwards vector.
    pub out_dir: Direction,
    /// Context dependent coordinate.
    pub u: i32,
    /// Context dependent coordinate.
    pub v: i32,
    /// Index of next point in contour.
    pub next_ix: u16,
    /// Index of previous point in contour.
    pub prev_ix: u16,
    /// Left out of segments (FreeType 2.14's `AF_FLAG_IGNORE`): a point of a tilde, or of what lies beyond it,
    /// that the hinter mustn't fit.
    pub ignore: bool,
}

impl Point {
    pub fn is_on_curve(&self) -> bool {
        self.flags.is_on_curve()
    }

    /// Returns the index of the next point in the contour.
    pub fn next(&self) -> usize {
        self.next_ix as usize
    }

    /// Returns the index of the previous point in the contour.
    pub fn prev(&self) -> usize {
        self.prev_ix as usize
    }
}

#[derive(Default)]
pub(crate) struct Outline {
    pub units_per_em: i32,
    pub orientation: Option<Orientation>,
    pub points: Vec<Point>,
    pub contours: Vec<Contour>,
    pub advance: i32,
}

impl Outline {
    /// Fills the outline using `draw`.
    ///
    /// `draw` writes the glyph's points in font units and returns its advance width (None: the glyph can't be
    /// drawn). None when it can't, or has more points than the autohinter takes.
    pub fn fill_with(&mut self, units_per_em: i32, draw: impl FnOnce(&mut Self) -> Option<i32>) -> Option<()> {
        self.clear();
        self.units_per_em = units_per_em;
        self.advance = draw(self)?;
        self.analyze_and_validate()
    }

    fn analyze_and_validate(&mut self) -> Option<()> {
        if self.points.len() > MAX_OUTLINE_POINTS || self.contours.len() > MAX_OUTLINE_POINTS {
            return None;
        }
        // Heuristic value
        let near_limit = 20 * self.units_per_em / 2048;
        self.link_points();
        self.mark_near_points(near_limit);
        self.compute_directions(near_limit);
        self.simplify_topology();
        self.check_remaining_weak_points(is_corner_flat);
        self.compute_orientation();
        Some(())
    }

    /// Applies dimension specific scaling factors and deltas to each
    /// point in the outline.
    pub fn scale(&mut self, scale: &Scale) {
        use super::metrics::fixed_mul;
        for point in &mut self.points {
            let x = fixed_mul(point.fx, scale.x_scale) + scale.x_delta;
            let y = fixed_mul(point.fy, scale.y_scale) + scale.y_delta;
            point.ox = x;
            point.x = x;
            point.oy = y;
            point.y = y;
        }
    }

    pub fn clear(&mut self) {
        self.units_per_em = 0;
        self.points.clear();
        self.contours.clear();
        self.advance = 0;
    }

    /// The hinted outline as FreeType's outline holds it (26.6, its points and their tags as the glyph has them),
    /// `overlap` its FT_OUTLINE_OVERLAP flag.
    pub fn to_fixed(&self, overlap: bool) -> FixedOutline {
        let mut out = FixedOutline::new();
        for contour in &self.contours {
            let Some(points) = self.points.get(contour.range()) else {
                continue;
            };
            for point in points {
                out.points.push(FixedPoint::new(point.x, point.y));
                out.tags.push(if point.flags.is_on_curve() {
                    PointTag::On
                } else if point.flags.is_off_curve_cubic() {
                    PointTag::Cubic
                } else {
                    PointTag::Conic
                });
            }
            out.contours.push(out.points.len() - 1);
        }
        out.overlap = overlap;
        out
    }
}

impl Outline {
    /// Sets next and previous indices for each point.
    fn link_points(&mut self) {
        let points = self.points.as_mut_slice();
        for contour in &self.contours {
            let Some(points) = points.get_mut(contour.range()) else {
                continue;
            };
            let first_ix = contour.first();
            let mut prev_ix = contour.last() as u16;
            for (ix, point) in points.iter_mut().enumerate() {
                let ix = (ix + first_ix) as u16;
                point.prev_ix = prev_ix;
                prev_ix = ix;
                // This can only potentially wrap for the last point in an
                // outline and will be patched up the line below that sets
                // the last point's next_ix to the first_ix.
                point.next_ix = ix.wrapping_add(1);
            }
            points.last_mut().unwrap().next_ix = first_ix as u16;
        }
    }

    /// Computes the near flag for each contour.
    ///
    /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L1017>
    fn mark_near_points(&mut self, near_limit: i32) {
        let points = self.points.as_mut_slice();
        for contour in &self.contours {
            let mut prev_ix = contour.last();
            for ix in contour.range() {
                let point = points[ix];
                let prev = &mut points[prev_ix];
                // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L1017>
                let out_x = point.fx - prev.fx;
                let out_y = point.fy - prev.fy;
                if out_x.abs() + out_y.abs() < near_limit {
                    prev.flags.set_marker(PointMarker::NEAR);
                }
                prev_ix = ix;
            }
        }
    }

    /// Compute directions of in and out vectors.
    ///
    /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L1064>
    fn compute_directions(&mut self, near_limit: i32) {
        let near_limit2 = 2 * near_limit - 1;
        let points = self.points.as_mut_slice();
        for contour in &self.contours {
            // Walk backward to find the first non-near point.
            let mut first_ix = contour.first();
            let mut ix = first_ix;
            let mut prev_ix = contour.prev(first_ix);
            let mut point = points[first_ix];
            while prev_ix != first_ix {
                let prev = points[prev_ix];
                let out_x = point.fx - prev.fx;
                let out_y = point.fy - prev.fy;
                // <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L1102>
                if out_x.abs() + out_y.abs() >= near_limit2 {
                    break;
                }
                point = prev;
                ix = prev_ix;
                prev_ix = contour.prev(prev_ix);
            }
            first_ix = ix;
            // Abuse u and v fields to store deltas to the next and previous
            // non-near points, respectively.
            let first = &mut points[first_ix];
            first.u = first_ix as _;
            first.v = first_ix as _;
            let mut next_ix = first_ix;
            let mut ix = first_ix;
            // Now loop over all points in the contour to compute in and
            // out directions
            let mut out_x = 0;
            let mut out_y = 0;
            loop {
                let point_ix = next_ix;
                next_ix = contour.next(point_ix);
                let point = points[point_ix];
                let next = &mut points[next_ix];
                // Accumulate the deltas until we surpass near_limit
                out_x += next.fx - point.fx;
                out_y += next.fy - point.fy;
                if out_x.abs() + out_y.abs() < near_limit {
                    next.flags.set_marker(PointMarker::WEAK_INTERPOLATION);
                    // The original code is a do-while loop, so make
                    // sure we keep this condition before the continue
                    if next_ix == first_ix {
                        break;
                    }
                    continue;
                }
                let out_dir = Direction::new(out_x, out_y);
                next.in_dir = out_dir;
                next.v = ix as _;
                let cur = &mut points[ix];
                cur.u = next_ix as _;
                cur.out_dir = out_dir;
                // Adjust directions for all intermediate points
                let mut inter_ix = contour.next(ix);
                while inter_ix != next_ix {
                    let point = &mut points[inter_ix];
                    point.in_dir = out_dir;
                    point.out_dir = out_dir;
                    inter_ix = contour.next(inter_ix);
                }
                ix = next_ix;
                points[ix].u = first_ix as _;
                points[first_ix].v = ix as _;
                out_x = 0;
                out_y = 0;
                if next_ix == first_ix {
                    break;
                }
            }
        }
    }

    /// Simplify so that we can identify local extrema more reliably.
    ///
    /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L1181>
    fn simplify_topology(&mut self) {
        let points = self.points.as_mut_slice();
        for i in 0..points.len() {
            let point = points[i];
            if point.flags.has_marker(PointMarker::WEAK_INTERPOLATION) {
                continue;
            }
            if point.in_dir == Direction::None && point.out_dir == Direction::None {
                let u_index = point.u as usize;
                let v_index = point.v as usize;
                let next_u = points[u_index];
                let prev_v = points[v_index];
                let in_x = point.fx - prev_v.fx;
                let in_y = point.fy - prev_v.fy;
                let out_x = next_u.fx - point.fx;
                let out_y = next_u.fy - point.fy;
                if (in_x ^ out_x) >= 0 && (in_y ^ out_y) >= 0 {
                    // Both vectors point into the same quadrant
                    points[i].flags.set_marker(PointMarker::WEAK_INTERPOLATION);
                    points[v_index].u = u_index as _;
                    points[u_index].v = v_index as _;
                }
            }
        }
    }

    /// Check for remaining weak points.
    ///
    /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afhints.c#L1226>
    fn check_remaining_weak_points(&mut self, is_corner_flat: impl Fn(i32, i32, i32, i32) -> bool) {
        let points = self.points.as_mut_slice();
        for i in 0..points.len() {
            let point = points[i];
            let mut make_weak = false;
            if point.flags.has_marker(PointMarker::WEAK_INTERPOLATION) {
                // Already weak
                continue;
            }
            if !point.flags.is_on_curve() {
                // Control points are always weak
                make_weak = true;
            } else if point.out_dir == point.in_dir {
                if point.out_dir != Direction::None {
                    // Point lies on a vertical or horizontal segment but
                    // not at start or end
                    make_weak = true;
                } else {
                    let u_index = point.u as usize;
                    let v_index = point.v as usize;
                    let next_u = points[u_index];
                    let prev_v = points[v_index];
                    if is_corner_flat(
                        point.fx - prev_v.fx,
                        point.fy - prev_v.fy,
                        next_u.fx - point.fx,
                        next_u.fy - point.fy,
                    ) {
                        // One of the vectors is more dominant
                        make_weak = true;
                        points[v_index].u = u_index as _;
                        points[u_index].v = v_index as _;
                    }
                }
            } else if point.in_dir.is_opposite(point.out_dir) {
                // Point forms a "spike"
                make_weak = true;
            }
            if make_weak {
                points[i].flags.set_marker(PointMarker::WEAK_INTERPOLATION);
            }
        }
    }

    /// Computes the overall winding order of the outline.
    ///
    /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/base/ftoutln.c#L1049>
    fn compute_orientation(&mut self) {
        self.orientation = None;
        let points = self.points.as_slice();
        if points.is_empty() {
            return;
        }
        fn point_to_i64(point: &Point) -> (i64, i64) {
            (point.fx as i64, point.fy as i64)
        }
        let mut area = 0i64;
        for contour in &self.contours {
            let last_ix = contour.last();
            let first_ix = contour.first();
            let (mut prev_x, mut prev_y) = point_to_i64(&points[last_ix]);
            for point in &points[first_ix..=last_ix] {
                let (x, y) = point_to_i64(point);
                area += (y - prev_y) * (x + prev_x);
                (prev_x, prev_y) = (x, y);
            }
        }
        use core::cmp::Ordering;
        self.orientation = match area.cmp(&0) {
            Ordering::Less => Some(Orientation::CounterClockwise),
            Ordering::Greater => Some(Orientation::Clockwise),
            Ordering::Equal => None,
        };
    }
}

/// Runtime or "just in time" hinted version from FT.
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/base/ftcalc.c#L1026>
fn is_corner_flat(in_x: i32, in_y: i32, out_x: i32, out_y: i32) -> bool {
    let ax = in_x + out_x;
    let ay = in_y + out_y;
    fn hypot(x: i32, y: i32) -> i32 {
        let x = x.abs();
        let y = y.abs();
        if x > y { x + ((3 * y) >> 3) } else { y + ((3 * x) >> 3) }
    }
    let d_in = hypot(in_x, in_y);
    let d_out = hypot(out_x, out_y);
    let d_hypot = hypot(ax, ay);
    (d_in + d_out - d_hypot) < (d_hypot >> 4)
}

#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct Contour {
    first_ix: u16,
    last_ix: u16,
}

impl Contour {
    pub fn first(self) -> usize {
        self.first_ix as usize
    }

    pub fn last(self) -> usize {
        self.last_ix as usize
    }

    pub fn next(self, index: usize) -> usize {
        if index >= self.last_ix as usize { self.first_ix as usize } else { index + 1 }
    }

    pub fn prev(self, index: usize) -> usize {
        if index <= self.first_ix as usize { self.last_ix as usize } else { index - 1 }
    }

    pub fn range(self) -> Range<usize> {
        self.first()..self.last() + 1
    }
}

impl UnscaledOutlineSink for Outline {
    fn start_contour(&mut self) {
        // The contour is empty until its first point arrives; one that ends
        // still empty is dropped again.
        let ix = self.points.len() as u16;
        self.contours.push(Contour { first_ix: ix, last_ix: ix });
    }

    fn point(&mut self, x: i32, y: i32, flags: PointFlags) {
        // Indices are sixteen bits; a glyph with more points than that fails
        // the point count check once it is filled.
        let ix = self.points.len() as u16;
        match self.contours.last_mut() {
            // The first point of a contour is already its range.
            Some(contour) if contour.first_ix == ix => {}
            Some(contour) => contour.last_ix = ix,
            // A point outside any contour starts one.
            None => self.contours.push(Contour { first_ix: ix, last_ix: ix }),
        }
        self.points.push(Point { flags, fx: clamp_coord(x), fy: clamp_coord(y), ..Default::default() });
    }

    fn end_contour(&mut self) {
        if self.contours.last().is_some_and(|contour| contour.first_ix as usize == self.points.len()) {
            self.contours.pop();
        }
    }
}

//! Android's PathInterpolator (android.view.animation.PathInterpolator, Android 37): a path from (0, 0) to (1, 1)
//! approximated by lines (Android's native `Path.approximate(0.002)`: curves halved until a line is within the
//! error of them, a cubic's halves checked again at their quarter for an inflection), its y read off by x between
//! the two points around it.

use kompoz_graphics::{Offset, Path, PathSegmentType};

/// How close the lines keep to the path (Android's PRECISION).
const PRECISION: f32 = 0.002;

/// Android's PathInterpolator: the path's points' x and y.
#[derive(Clone, Debug)]
pub(crate) struct PathInterpolator {
    x: Vec<f32>,
    y: Vec<f32>,
}

/// Why a path can't be one (Android's IllegalArgumentException messages).
pub(crate) type PathInterpolatorError = &'static str;

impl PathInterpolator {
    /// Android's `PathInterpolator(path)`: the path must start at (0, 0), end at (1, 1), never go back along x nor
    /// jump along it.
    pub(crate) fn of_path(path: &Path) -> Result<PathInterpolator, PathInterpolatorError> {
        let points = approximate(path, PRECISION);
        let (first, last) = (points[0], points[points.len() - 1]);
        if first.1 != 0.0 || first.2 != 0.0 || last.1 != 1.0 || last.2 != 1.0 {
            return Err("The Path must start at (0,0) and end at (1,1)");
        }
        let mut x = Vec::with_capacity(points.len());
        let mut y = Vec::with_capacity(points.len());
        let (mut prev_x, mut prev_fraction) = (0.0f32, 0.0f32);
        for &(fraction, px, py) in &points {
            if fraction == prev_fraction && px != prev_x {
                return Err("The Path cannot have discontinuity in the X axis.");
            }
            if px < prev_x {
                return Err("The Path cannot loop back on itself.");
            }
            x.push(px);
            y.push(py);
            prev_x = px;
            prev_fraction = fraction;
        }
        Ok(PathInterpolator { x, y })
    }

    /// Android's `PathInterpolator(controlX, controlY)`: a quadratic Bézier from (0, 0) to (1, 1).
    pub(crate) fn quad(control_x: f32, control_y: f32) -> Result<PathInterpolator, PathInterpolatorError> {
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.quadratic_to(control_x, control_y, 1.0, 1.0);
        PathInterpolator::of_path(&path)
    }

    /// Android's `getInterpolation(t)`: 0 at or before 0, 1 at or past 1, else the line's y at x `t`.
    pub(crate) fn get_interpolation(&self, t: f32) -> f32 {
        if t <= 0.0 {
            return 0.0;
        } else if t >= 1.0 {
            return 1.0;
        }
        let mut start_index = 0usize;
        let mut end_index = self.x.len() - 1;
        while end_index - start_index > 1 {
            let mid_index = (start_index + end_index) / 2;
            if t < self.x[mid_index] {
                end_index = mid_index;
            } else {
                start_index = mid_index;
            }
        }
        let x_range = self.x[end_index] - self.x[start_index];
        if x_range == 0.0 {
            return self.y[start_index];
        }
        let t_in_range = t - self.x[start_index];
        let fraction = t_in_range / x_range;
        let start_y = self.y[start_index];
        let end_y = self.y[end_index];
        start_y + (fraction * (end_y - start_y))
    }
}

/// A point of a Bézier at `t` (Android's quadratic and cubic BezierCalculation).
type Bezier = fn(f32, &[Offset]) -> Offset;

fn cubic_coordinate(t: f32, p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    let one_minus_t = 1.0 - t;
    let one_minus_t_squared = one_minus_t * one_minus_t;
    let one_minus_t_cubed = one_minus_t_squared * one_minus_t;
    let t_squared = t * t;
    let t_cubed = t_squared * t;
    (one_minus_t_cubed * p0)
        + (3.0 * one_minus_t_squared * t * p1)
        + (3.0 * one_minus_t * t_squared * p2)
        + (t_cubed * p3)
}

fn cubic_bezier(t: f32, p: &[Offset]) -> Offset {
    Offset::new(
        cubic_coordinate(t, p[0].x, p[1].x, p[2].x, p[3].x),
        cubic_coordinate(t, p[0].y, p[1].y, p[2].y, p[3].y),
    )
}

fn quadratic_coordinate(t: f32, p0: f32, p1: f32, p2: f32) -> f32 {
    let one_minus_t = 1.0 - t;
    one_minus_t * ((one_minus_t * p0) + (t * p1)) + t * ((one_minus_t * p1) + (t * p2))
}

fn quadratic_bezier(t: f32, p: &[Offset]) -> Offset {
    Offset::new(quadratic_coordinate(t, p[0].x, p[1].x, p[2].x), quadratic_coordinate(t, p[0].y, p[1].y, p[2].y))
}

/// The points and their lengths along the path so far (Android's segmentPoints and lengths).
struct Segments {
    points: Vec<Offset>,
    lengths: Vec<f32>,
}

impl Segments {
    fn add_move(&mut self, point: Offset) {
        let length = self.lengths.last().copied().unwrap_or(0.0);
        self.points.push(point);
        self.lengths.push(length);
    }

    /// Android's addLine: from (0, 0) where there is nothing yet; nothing where it goes nowhere.
    fn add_line(&mut self, to: Offset) {
        match self.points.last() {
            None => {
                self.points.push(Offset::new(0.0, 0.0));
                self.lengths.push(0.0);
            }
            // (an empty line)
            Some(&last) if last == to => return,
            Some(_) => {}
        }
        let last = self.points[self.points.len() - 1];
        let (dx, dy) = (to.x - last.x, to.y - last.y);
        let length = self.lengths[self.lengths.len() - 1] + (dx * dx + dy * dy).sqrt();
        self.points.push(to);
        self.lengths.push(length);
    }

    /// Android's addBezier: the curve halved, each part again until the line across it is within `error_squared`
    /// of the curve at its middle (a cubic's checked at its quarter too, until one inflection is found).
    fn add_bezier(&mut self, points: &[Offset], bezier: Bezier, error_squared: f32, double_check_division: bool) {
        let mut double_check = double_check_division;
        let mut t_to_point: Vec<(f32, Offset)> = vec![(0.0, bezier(0.0, points)), (1.0, bezier(1.0, points))];
        let mut iter = 0usize;
        while iter + 1 < t_to_point.len() {
            loop {
                let (t0, p0) = t_to_point[iter];
                let (t1, p1) = t_to_point[iter + 1];
                let (mid_t, mid_point, mut needs_subdivision) =
                    subdivide_points(points, bezier, t0, p0, t1, p1, error_squared);
                if !needs_subdivision && double_check {
                    let (_, _, quarter_needs) =
                        subdivide_points(points, bezier, t0, p0, mid_t, mid_point, error_squared);
                    needs_subdivision = quarter_needs;
                    if needs_subdivision {
                        // (an inflection point found: no need to check again)
                        double_check = false;
                    }
                }
                if needs_subdivision {
                    t_to_point.insert(iter + 1, (mid_t, mid_point));
                } else {
                    break;
                }
            }
            iter += 1;
        }
        for (_, p) in t_to_point {
            self.add_line(p);
        }
    }
}

/// Android's subdividePoints: the middle `t` and its point, and whether the line from `p0` to `p1` misses it by
/// more than the error.
fn subdivide_points(
    points: &[Offset],
    bezier: Bezier,
    t0: f32,
    p0: Offset,
    t1: f32,
    p1: Offset,
    error_squared: f32,
) -> (f32, Offset, bool) {
    let mid_t = (t1 + t0) / 2.0;
    let mid_x = (p1.x + p0.x) / 2.0;
    let mid_y = (p1.y + p0.y) / 2.0;
    let mid_point = bezier(mid_t, points);
    let x_error = mid_point.x - mid_x;
    let y_error = mid_point.y - mid_y;
    let mid_error_squared = (x_error * x_error) + (y_error * y_error);
    (mid_t, mid_point, mid_error_squared > error_squared)
}

/// Android's `Path.approximate(acceptableError)`: each point along the path as (its fraction of the path's length,
/// x, y); a move adds a point at the same length.
fn approximate(path: &Path, acceptable_error: f32) -> Vec<(f32, f32, f32)> {
    let mut s = Segments { points: Vec::new(), lengths: Vec::new() };
    let error_squared = acceptable_error * acceptable_error;
    let error_conic = acceptable_error / 2.0;
    let mut move_to = Offset::new(0.0, 0.0);
    let mut verbs = 0usize;
    let mut first_point = None;
    for segment in path.iterator() {
        let p = &segment.points;
        verbs += 1;
        match segment.r#type {
            PathSegmentType::Move => {
                move_to = p[0];
                first_point.get_or_insert(p[0]);
                s.add_move(p[0]);
            }
            PathSegmentType::Close => s.add_line(move_to),
            PathSegmentType::Line => s.add_line(p[1]),
            PathSegmentType::Quadratic => s.add_bezier(p, quadratic_bezier, error_squared, false),
            PathSegmentType::Cubic => s.add_bezier(p, cubic_bezier, error_squared, true),
            // (the iterator gives conics as quadratics: Skia's within the conic's error)
            PathSegmentType::Conic => s.add_bezier(p, quadratic_bezier, error_conic, false),
            PathSegmentType::Done => {}
        }
    }
    if s.points.is_empty() {
        match (verbs, first_point) {
            (1, Some(p)) => s.add_move(p),
            // (an invalid or empty path: (0, 0))
            _ => s.add_move(Offset::new(0.0, 0.0)),
        }
    }
    let mut total_length = s.lengths[s.lengths.len() - 1];
    if total_length == 0.0 {
        // (a lone move still animates, at the same value)
        let last = s.points[s.points.len() - 1];
        s.points.push(last);
        s.lengths.push(1.0);
        total_length = 1.0;
    }
    s.points.iter().zip(&s.lengths).map(|(p, l)| (l / total_length, p.x, p.y)).collect()
}

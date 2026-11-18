//! Keyframes joined by quarter ellipses (Compose's internal ArcSpline, from ConstraintLayout's motion library):
//! each pair of dimensions moves along an arc between two keyframes, at a constant speed along it.

use std::f64::consts::PI;

pub(crate) const ArcSplineArcStartLinear: i32 = 0;
pub(crate) const ArcSplineArcStartVertical: i32 = 1;
pub(crate) const ArcSplineArcStartHorizontal: i32 = 2;
pub(crate) const ArcSplineArcStartFlip: i32 = 3;
pub(crate) const ArcSplineArcBelow: i32 = 4;
pub(crate) const ArcSplineArcAbove: i32 = 5;

const StartVertical: i32 = 1;
const StartHorizontal: i32 = 2;
const StartLinear: i32 = 3;
const DownArc: i32 = 4;
const UpArc: i32 = 5;
const LutSize: usize = 101;
const Epsilon: f32 = 0.001;
const HalfPi: f32 = (PI * 0.5) as f32;

/// Kotlin's float functions: in doubles, back to a float.
fn sinf(x: f32) -> f32 {
    (x as f64).sin() as f32
}
fn cosf(x: f32) -> f32 {
    (x as f64).cos() as f32
}
fn hypotf(x: f32, y: f32) -> f32 {
    (x as f64).hypot(y as f64) as f32
}

/// Java's `Arrays.binarySearch(float[], float)`: the index of `key`, or `-(insertion point) - 1` (floats compared
/// as Java's `Float.compare` does where they are equal).
pub(crate) fn java_float_binary_search(a: &[f32], key: f32) -> i32 {
    let (mut low, mut high) = (0i32, a.len() as i32 - 1);
    while low <= high {
        let mid = ((low + high) as u32 >> 1) as i32;
        let mid_val = a[mid as usize];
        if mid_val < key {
            low = mid + 1;
        } else if mid_val > key {
            high = mid - 1;
        } else {
            let mid_bits = java_float_bits(mid_val);
            let key_bits = java_float_bits(key);
            if mid_bits == key_bits {
                return mid;
            } else if mid_bits < key_bits {
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }
    }
    -(low + 1)
}

/// Java's `Float.floatToIntBits` (one NaN for all).
fn java_float_bits(f: f32) -> i32 {
    if f.is_nan() { 0x7fc0_0000 } else { f.to_bits() as i32 }
}

/// A stretch between two keyframes of a pair of dimensions.
pub(crate) struct Arc {
    pub(crate) time1: f32,
    pub(crate) time2: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    arc_distance: f32,
    lut: [f32; LutSize],
    one_over_delta_time: f32,
    arc_velocity: f32,
    vertical: f32,
    ellipse_a: f32,
    ellipse_b: f32,
    pub(crate) is_linear: bool,
    ellipse_center_x: f32,
    ellipse_center_y: f32,
}

impl Arc {
    fn new(mode: i32, time1: f32, time2: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> Arc {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let is_vertical = match mode {
            StartVertical => true,
            UpArc => dy < 0.0,
            DownArc => dy > 0.0,
            _ => false,
        };
        let vertical = if is_vertical { -1.0 } else { 1.0 };
        let one_over_delta_time = 1.0 / (time2 - time1);
        let mut arc = Arc {
            time1,
            time2,
            x1,
            y1,
            x2,
            y2,
            arc_distance: 0.0,
            lut: [0.0; LutSize],
            one_over_delta_time,
            arc_velocity: 0.0,
            vertical,
            ellipse_a: 0.0,
            ellipse_b: 0.0,
            is_linear: false,
            ellipse_center_x: 0.0,
            ellipse_center_y: 0.0,
        };
        let mut is_linear = mode == StartLinear;
        if is_linear || dx.abs() < Epsilon || dy.abs() < Epsilon {
            is_linear = true;
            arc.arc_distance = hypotf(dy, dx);
            arc.arc_velocity = arc.arc_distance * one_over_delta_time;
            // (the slope cached in the unused center)
            arc.ellipse_center_x = dx * one_over_delta_time;
            arc.ellipse_center_y = dy * one_over_delta_time;
            arc.ellipse_a = f32::NAN;
            arc.ellipse_b = f32::NAN;
        } else {
            arc.ellipse_a = dx * vertical;
            arc.ellipse_b = dy * -vertical;
            arc.ellipse_center_x = if is_vertical { x2 } else { x1 };
            arc.ellipse_center_y = if is_vertical { y1 } else { y2 };
            arc.build_table(x1, y1, x2, y2);
            arc.arc_velocity = arc.arc_distance * one_over_delta_time;
        }
        arc.is_linear = is_linear;
        arc
    }

    pub(crate) fn linear_dx(&self) -> f32 {
        self.ellipse_center_x
    }
    pub(crate) fn linear_dy(&self) -> f32 {
        self.ellipse_center_y
    }

    /// The sine and cosine of the angle at `time` (Compose's setPoint).
    pub(crate) fn set_point(&self, time: f32) -> (f32, f32) {
        let angle = self.calc_angle(time);
        (sinf(angle), cosf(angle))
    }

    fn calc_angle(&self, time: f32) -> f32 {
        let percent =
            (if self.vertical == -1.0 { self.time2 - time } else { time - self.time1 }) * self.one_over_delta_time;
        HalfPi * self.lookup(percent)
    }

    pub(crate) fn calc_x(&self, (sin, _): (f32, f32)) -> f32 {
        self.ellipse_center_x + self.ellipse_a * sin
    }
    pub(crate) fn calc_y(&self, (_, cos): (f32, f32)) -> f32 {
        self.ellipse_center_y + self.ellipse_b * cos
    }
    pub(crate) fn calc_dx(&self, (sin, cos): (f32, f32)) -> f32 {
        let vx = self.ellipse_a * cos;
        let vy = -self.ellipse_b * sin;
        let norm = self.arc_velocity / hypotf(vx, vy);
        vx * self.vertical * norm
    }
    pub(crate) fn calc_dy(&self, (sin, cos): (f32, f32)) -> f32 {
        let vx = self.ellipse_a * cos;
        let vy = -self.ellipse_b * sin;
        let norm = self.arc_velocity / hypotf(vx, vy);
        vy * self.vertical * norm
    }
    pub(crate) fn get_linear_x(&self, time: f32) -> f32 {
        let t = (time - self.time1) * self.one_over_delta_time;
        self.x1 + t * (self.x2 - self.x1)
    }
    pub(crate) fn get_linear_y(&self, time: f32) -> f32 {
        let t = (time - self.time1) * self.one_over_delta_time;
        self.y1 + t * (self.y2 - self.y1)
    }

    fn lookup(&self, v: f32) -> f32 {
        if v <= 0.0 {
            return 0.0;
        }
        if v >= 1.0 {
            return 1.0;
        }
        let pos = v * (LutSize - 1) as f32;
        let iv = pos as i32 as usize;
        let off = pos - (pos as i32) as f32;
        self.lut[iv] + off * (self.lut[iv + 1] - self.lut[iv])
    }

    fn build_table(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        let a = x2 - x1;
        let b = y1 - y2;
        let mut lx = 0.0f32;
        let mut ly = b;
        let mut dist = 0.0f32;
        // (Compose's OurPercentCache, 91 long, its first always 0)
        let mut our_percent = [0.0f32; 91];
        let last_index = our_percent.len() - 1;
        let last_index_float = last_index as f32;
        for (i, percent) in our_percent.iter_mut().enumerate().skip(1) {
            let angle = ((90.0 * i as f64 / last_index as f64) * (PI / 180.0)) as f32;
            let s = sinf(angle);
            let c = cosf(angle);
            let px = a * s;
            let py = b * c;
            dist += hypotf(px - lx, py - ly);
            *percent = dist;
            lx = px;
            ly = py;
        }
        self.arc_distance = dist;
        for p in our_percent.iter_mut().skip(1) {
            *p /= dist;
        }
        let lut_last_index = (LutSize - 1) as f32;
        for (i, lut) in self.lut.iter_mut().enumerate().take(LutSize) {
            let pos = i as f32 / lut_last_index;
            let index = java_float_binary_search(&our_percent, pos);
            if index >= 0 {
                *lut = index as f32 / last_index_float;
            } else if index == -1 {
                *lut = 0.0;
            } else {
                let p1 = (-index - 2) as usize;
                let p2 = (-index - 1) as usize;
                *lut = (p1 as f32 + (pos - our_percent[p1]) / (our_percent[p2] - our_percent[p1])) / last_index_float;
            }
        }
    }
}

/// Keyframes of pairs of dimensions joined by arcs (Compose's ArcSpline).
pub(crate) struct ArcSpline {
    arcs: Vec<Vec<Arc>>,
}

impl ArcSpline {
    /// `arc_modes` and `time_points` per keyframe; `y`, a keyframe's floats (an even count).
    pub(crate) fn new(arc_modes: &[i32], time_points: &[f32], y: &[Vec<f32>]) -> ArcSpline {
        let mut mode = StartVertical;
        let mut last = StartVertical;
        let arcs = (0..time_points.len() - 1)
            .map(|i| {
                match arc_modes[i] {
                    ArcSplineArcStartVertical => {
                        mode = StartVertical;
                        last = mode;
                    }
                    ArcSplineArcStartHorizontal => {
                        mode = StartHorizontal;
                        last = mode;
                    }
                    ArcSplineArcStartFlip => {
                        mode = if last == StartVertical { StartHorizontal } else { StartVertical };
                        last = mode;
                    }
                    ArcSplineArcStartLinear => mode = StartLinear,
                    ArcSplineArcAbove => mode = UpArc,
                    ArcSplineArcBelow => mode = DownArc,
                    _ => {}
                }
                let (y_array, y_array1) = (&y[i], &y[i + 1]);
                let (time, time1) = (time_points[i], time_points[i + 1]);
                let dim = y_array.len() / 2 + y_array.len() % 2;
                (0..dim)
                    .map(|j| {
                        let k = j * 2;
                        Arc::new(mode, time, time1, y_array[k], y_array[k + 1], y_array1[k], y_array1[k + 1])
                    })
                    .collect()
            })
            .collect();
        ArcSpline { arcs }
    }

    /// The position at `time` into `v` (Compose's getPos; outside the keyframes' times, extrapolated along the
    /// first or last arc's tangent).
    pub(crate) fn get_pos(&self, time: f32, v: &mut [f32]) {
        let t = time;
        let arcs = &self.arcs;
        let last_index = arcs.len() - 1;
        let start = arcs[0][0].time1;
        let end = arcs[last_index][0].time2;
        let size = v.len();
        if t < start || t > end {
            let (p, t0) = if t > end { (last_index, end) } else { (0, start) };
            let dt = t - t0;
            let (mut i, mut j) = (0usize, 0usize);
            while i + 1 < size {
                let arc = &arcs[p][j];
                if arc.is_linear {
                    v[i] = arc.get_linear_x(t0) + dt * arc.linear_dx();
                    v[i + 1] = arc.get_linear_y(t0) + dt * arc.linear_dy();
                } else {
                    let point = arc.set_point(t0);
                    v[i] = arc.calc_x(point) + dt * arc.calc_dx(point);
                    v[i + 1] = arc.calc_y(point) + dt * arc.calc_dy(point);
                }
                i += 2;
                j += 1;
            }
            return;
        }
        let mut populated = false;
        for arcs_i in arcs {
            let (mut k, mut j) = (0usize, 0usize);
            while j + 1 < size {
                let arc = &arcs_i[k];
                if t <= arc.time2 {
                    if arc.is_linear {
                        v[j] = arc.get_linear_x(t);
                        v[j + 1] = arc.get_linear_y(t);
                    } else {
                        let point = arc.set_point(t);
                        v[j] = arc.calc_x(point);
                        v[j + 1] = arc.calc_y(point);
                    }
                    populated = true;
                }
                j += 2;
                k += 1;
            }
            if populated {
                return;
            }
        }
    }

    /// The velocity at `time` into `v` (Compose's getSlope; within the keyframes' times).
    pub(crate) fn get_slope(&self, time: f32, v: &mut [f32]) {
        let arcs = &self.arcs;
        let t = crate::easing::coerce_in(time, arcs[0][0].time1, arcs[arcs.len() - 1][0].time2);
        let size = v.len();
        let mut populated = false;
        for arcs_i in arcs {
            let (mut j, mut k) = (0usize, 0usize);
            while j + 1 < size {
                let arc = &arcs_i[k];
                if t <= arc.time2 {
                    if arc.is_linear {
                        v[j] = arc.linear_dx();
                        v[j + 1] = arc.linear_dy();
                    } else {
                        let point = arc.set_point(t);
                        v[j] = arc.calc_dx(point);
                        v[j + 1] = arc.calc_dy(point);
                    }
                    populated = true;
                }
                j += 2;
                k += 1;
            }
            if populated {
                return;
            }
        }
    }
}

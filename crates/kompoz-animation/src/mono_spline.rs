//! A monotonic cubic spline through keyframes (Compose's internal MonoSpline, Fritsch–Carlson tangents): what
//! `keyframesWithSpline` animates along.

use crate::easing::coerce_in;
use crate::vector::AnimationVector;

fn hypotf(x: f32, y: f32) -> f32 {
    (x as f64).hypot(y as f64) as f32
}

/// Values at times, with tangents keeping each stretch monotonic; extrapolated past its ends along their slopes.
pub(crate) struct MonoSpline {
    time_points: Vec<f32>,
    values: Vec<Vec<f32>>,
    tangents: Vec<Vec<f32>>,
}

impl MonoSpline {
    /// `time` per keyframe, `y` its floats; `periodic_bias` (NaN: none) makes the ends' slopes one, weighted
    /// between the last stretch's (0) and the first's (1).
    pub(crate) fn new(time: Vec<f32>, y: Vec<Vec<f32>>, periodic_bias: f32) -> MonoSpline {
        let n = time.len();
        let dim = y[0].len();
        let mut slope = vec![vec![0.0f32; dim]; n - 1];
        let mut tangent = vec![vec![0.0f32; dim]; n];
        for j in 0..dim {
            for i in 0..n - 1 {
                let dt = time[i + 1] - time[i];
                slope[i][j] = (y[i + 1][j] - y[i][j]) / dt;
                if i == 0 {
                    tangent[i][j] = slope[i][j];
                } else {
                    tangent[i][j] = (slope[i - 1][j] + slope[i][j]) * 0.5;
                }
            }
            tangent[n - 1][j] = slope[n - 2][j];
        }
        if !periodic_bias.is_nan() {
            for j in 0..dim {
                let adjusted_slope = (slope[n - 2][j] * (1.0 - periodic_bias)) + (slope[0][j] * periodic_bias);
                slope[0][j] = adjusted_slope;
                slope[n - 2][j] = adjusted_slope;
                tangent[n - 1][j] = adjusted_slope;
                tangent[0][j] = adjusted_slope;
            }
        }
        for i in 0..n - 1 {
            for j in 0..dim {
                if slope[i][j] == 0.0 {
                    tangent[i][j] = 0.0;
                    tangent[i + 1][j] = 0.0;
                } else {
                    let a = tangent[i][j] / slope[i][j];
                    let b = tangent[i + 1][j] / slope[i][j];
                    let h = hypotf(a, b);
                    if h as f64 > 9.0 {
                        let t = 3.0 / h;
                        tangent[i][j] = t * a * slope[i][j];
                        tangent[i + 1][j] = t * b * slope[i][j];
                    }
                }
            }
        }
        MonoSpline { time_points: time, values: y, tangents: tangent }
    }

    /// The position at `time` into `v`, searching from keyframe `index` (Compose's getPos into a vector).
    pub(crate) fn get_pos<V: AnimationVector>(&self, time: f32, v: &mut V, index: usize) {
        let n = self.time_points.len();
        let dim = self.values[0].len();
        let k = if time <= self.time_points[0] {
            Some(0)
        } else if time >= self.time_points[n - 1] {
            Some(n - 1)
        } else {
            None
        };
        if let Some(k) = k {
            let mut slope_temp = vec![0.0f32; dim];
            self.get_slope_array(self.time_points[k], &mut slope_temp);
            for (j, slope) in slope_temp.iter().enumerate() {
                v.set(j, self.values[k][j] + (time - self.time_points[k]) * slope);
            }
            return;
        }
        for i in index..n - 1 {
            if time == self.time_points[i] {
                for (j, &value) in self.values[i].iter().enumerate().take(dim) {
                    v.set(j, value);
                }
                return;
            }
            if time < self.time_points[i + 1] {
                let h = self.time_points[i + 1] - self.time_points[i];
                let x = (time - self.time_points[i]) / h;
                for j in 0..dim {
                    let y1 = self.values[i][j];
                    let y2 = self.values[i + 1][j];
                    let t1 = self.tangents[i][j];
                    let t2 = self.tangents[i + 1][j];
                    v.set(j, hermite_interpolate(h, x, y1, y2, t1, t2));
                }
                return;
            }
        }
    }

    fn get_slope_array(&self, time: f32, v: &mut [f32]) {
        let dim = self.values[0].len();
        let n = self.time_points.len();
        let t = coerce_in(time, self.time_points[0], self.time_points[n - 1]);
        if v.len() < dim {
            return;
        }
        for i in 0..n - 1 {
            if t <= self.time_points[i + 1] {
                let h = self.time_points[i + 1] - self.time_points[i];
                let x = (t - self.time_points[i]) / h;
                for (j, out) in v.iter_mut().enumerate().take(dim) {
                    let y1 = self.values[i][j];
                    let y2 = self.values[i + 1][j];
                    let t1 = self.tangents[i][j];
                    let t2 = self.tangents[i + 1][j];
                    *out = hermite_differential(h, x, y1, y2, t1, t2) / h;
                }
                break;
            }
        }
    }

    /// The velocity at `time` into `v`, searching from keyframe `index` (Compose's getSlope into a vector: the
    /// end's tangent past an end).
    pub(crate) fn get_slope<V: AnimationVector>(&self, time: f32, v: &mut V, index: usize) {
        let n = self.time_points.len();
        let dim = self.values[0].len();
        let tangent_index = if time <= self.time_points[0] {
            Some(0)
        } else if time >= self.time_points[n - 1] {
            Some(n - 1)
        } else {
            None
        };
        if let Some(ti) = tangent_index {
            let tangent = &self.tangents[ti];
            if tangent.len() < dim {
                return;
            }
            for (j, t) in tangent.iter().enumerate().take(dim) {
                v.set(j, *t);
            }
            return;
        }
        for i in index..n - 1 {
            if time <= self.time_points[i + 1] {
                let h = self.time_points[i + 1] - self.time_points[i];
                let x = (time - self.time_points[i]) / h;
                for j in 0..dim {
                    let y1 = self.values[i][j];
                    let y2 = self.values[i + 1][j];
                    let t1 = self.tangents[i][j];
                    let t2 = self.tangents[i + 1][j];
                    v.set(j, hermite_differential(h, x, y1, y2, t1, t2) / h);
                }
                break;
            }
        }
    }
}

/// The Hermite curve through (0, y1) and (1, y2) with tangents t1, t2 over a stretch of length h, at x.
pub(crate) fn hermite_interpolate(h: f32, x: f32, y1: f32, y2: f32, t1: f32, t2: f32) -> f32 {
    let x2 = x * x;
    let x3 = x2 * x;
    h * t1 * (x - 2.0 * x2 + x3) + h * t2 * (x3 - x2) + y1 - (3.0 * x2 - 2.0 * x3) * (y1 - y2)
}

/// Its derivative.
pub(crate) fn hermite_differential(h: f32, x: f32, y1: f32, y2: f32, t1: f32, t2: f32) -> f32 {
    let x2 = x * x;
    h * (t1 - 2.0 * x * (2.0 * t1 + t2) + 3.0 * (t1 + t2) * x2) - 6.0 * (x - x2) * (y1 - y2)
}

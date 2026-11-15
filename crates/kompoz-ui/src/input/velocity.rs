//! How fast a pointer moves, as Compose's `VelocityTracker.kt`: positions fed as they come, the velocity a least
//! squares fit of the last 100 ms (Compose's Lsq2 strategy; Impulse for differential data), nothing if the pointer
//! stood still for 40 ms at the end.

use super::PointerInputChange;
use crate::unit::Velocity;
use kompoz_graphics::Offset;

/// The samples a tracker keeps (Compose's `HistorySize`).
const HISTORY_SIZE: usize = 20;
/// How far back the samples count (Compose's `HorizonMilliseconds`).
const HORIZON_MILLISECONDS: i64 = 100;
/// A pause this long ends the motion (Compose's `AssumePointerMoveStoppedMilliseconds`).
const ASSUME_POINTER_MOVE_STOPPED_MILLISECONDS: i64 = 40;

/// How a 1D tracker works its velocity out (Compose's VelocityTracker1D.Strategy).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Strategy {
    /// A least squares fit of a second degree polynomial.
    Lsq2,
    /// The kinetic energy the samples' impulses add up to (Android's impulse strategy).
    Impulse,
}

/// The velocity of one value over time (Compose's VelocityTracker1D): positions, or with `is_data_differential`
/// the changes between them.
#[derive(Clone)]
pub struct VelocityTracker1D {
    is_data_differential: bool,
    strategy: Strategy,
    samples: [Option<(i64, f32)>; HISTORY_SIZE],
    index: usize,
}

impl VelocityTracker1D {
    /// A tracker of positions, or with `is_data_differential` of the changes between them, by the impulse strategy
    /// (Compose's public `VelocityTracker1D(isDataDifferential)`; its 2D tracker fits each axis by Lsq2).
    pub fn new(is_data_differential: bool) -> VelocityTracker1D {
        VelocityTracker1D::with_strategy(is_data_differential, Strategy::Impulse)
    }

    fn with_strategy(is_data_differential: bool, strategy: Strategy) -> VelocityTracker1D {
        assert!(
            !(is_data_differential && strategy == Strategy::Lsq2),
            "Lsq2 not (yet) supported for differential axes"
        );
        VelocityTracker1D { is_data_differential, strategy, samples: [None; HISTORY_SIZE], index: 0 }
    }

    pub fn is_data_differential(&self) -> bool {
        self.is_data_differential
    }

    fn min_sample_size(&self) -> usize {
        match self.strategy {
            Strategy::Impulse => 2,
            Strategy::Lsq2 => 3,
        }
    }

    /// A value at a time (Compose's `addDataPoint`).
    pub fn add_data_point(&mut self, time_millis: i64, data_point: f32) {
        self.index = (self.index + 1) % HISTORY_SIZE;
        self.samples[self.index] = Some((time_millis, data_point));
    }

    /// The velocity, units per second (Compose's `calculateVelocity`).
    pub fn calculate_velocity(&self) -> f32 {
        let mut data_points = [0f32; HISTORY_SIZE];
        let mut time = [0f32; HISTORY_SIZE];
        let mut sample_count = 0;
        let mut index = self.index;
        let Some(newest) = self.samples[index] else { return 0.0 };
        let mut previous = newest;
        while let Some(sample) = self.samples[index] {
            let age = (newest.0 - sample.0) as f32;
            let delta = (sample.0 - previous.0).abs();
            previous = if self.strategy == Strategy::Lsq2 || self.is_data_differential { sample } else { newest };
            if age > HORIZON_MILLISECONDS as f32 || delta > ASSUME_POINTER_MOVE_STOPPED_MILLISECONDS {
                break;
            }
            data_points[sample_count] = sample.1;
            time[sample_count] = -age;
            index = if index == 0 { HISTORY_SIZE } else { index } - 1;
            sample_count += 1;
            if sample_count >= HISTORY_SIZE {
                break;
            }
        }
        if sample_count >= self.min_sample_size() {
            let v = match self.strategy {
                Strategy::Impulse => {
                    calculate_impulse_velocity(&data_points, &time, sample_count, self.is_data_differential)
                }
                Strategy::Lsq2 => poly_fit_least_squares(&time, &data_points, sample_count, 2).map_or(0.0, |c| c[1]),
            };
            return v * 1000.0; // units/ms to units/s
        }
        0.0
    }

    /// The velocity, no faster than `maximum_velocity` either way (Compose's `calculateVelocity(maximumVelocity)`).
    pub fn calculate_velocity_max(&self, maximum_velocity: f32) -> f32 {
        assert!(maximum_velocity > 0.0, "maximumVelocity should be a positive value. You specified={maximum_velocity}");
        let v = self.calculate_velocity();
        if v == 0.0 || v.is_nan() {
            0.0
        } else if v > 0.0 {
            v.min(maximum_velocity)
        } else {
            v.max(-maximum_velocity)
        }
    }

    /// Forgets the samples.
    pub fn reset_tracking(&mut self) {
        self.samples = [None; HISTORY_SIZE];
        self.index = 0;
    }
}

/// The coefficients of the polynomial of `degree` best fitting the samples, lowest first (Compose's
/// `polyFitLeastSquares`, a QR decomposition by Gram-Schmidt); `None` with no samples.
fn poly_fit_least_squares(x: &[f32], y: &[f32], sample_count: usize, degree: usize) -> Option<Vec<f32>> {
    if degree < 1 || sample_count == 0 {
        return None;
    }
    let degree = if degree >= sample_count { sample_count - 1 } else { degree };
    let m = sample_count;
    let n = degree + 1;
    let mut a = vec![vec![0f32; m]; n];
    for h in 0..m {
        a[0][h] = 1.0;
        for i in 1..n {
            a[i][h] = a[i - 1][h] * x[h];
        }
    }
    let dot = |p: &[f32], q: &[f32]| -> f32 { p.iter().zip(q).map(|(a, b)| a * b).sum() };
    let mut q = vec![vec![0f32; m]; n];
    let mut r = vec![vec![0f32; n]; n];
    for j in 0..n {
        let mut w = a[j].clone();
        for qi in q.iter().take(j) {
            let d = dot(&w, qi);
            for h in 0..m {
                w[h] -= d * qi[h];
            }
        }
        let inverse_norm = 1.0 / dot(&w, &w).sqrt().max(1e-6);
        for v in w.iter_mut() {
            *v *= inverse_norm;
        }
        for i in 0..n {
            r[j][i] = if i < j { 0.0 } else { dot(&w, &a[i]) };
        }
        q[j] = w;
    }
    let mut coefficients = vec![0f32; n];
    for i in (0..n).rev() {
        let mut c = dot(&q[i], &y[..m]);
        for j in (i + 1..n).rev() {
            c -= r[i][j] * coefficients[j];
        }
        coefficients[i] = c / r[i][i];
    }
    Some(coefficients)
}

/// Compose's `calculateImpulseVelocity`.
fn calculate_impulse_velocity(data_points: &[f32], time: &[f32], sample_count: usize, differential: bool) -> f32 {
    let kinetic_energy_to_velocity = |e: f32| e.signum() * (2.0 * e.abs()).sqrt();
    let mut work = 0f32;
    let start = sample_count - 1;
    let mut next_time = time[start];
    for i in (1..=start).rev() {
        let current_time = next_time;
        next_time = time[i - 1];
        if current_time == next_time {
            continue;
        }
        let delta = if differential { -data_points[i - 1] } else { data_points[i] - data_points[i - 1] };
        let v_curr = delta / (current_time - next_time);
        let v_prev = kinetic_energy_to_velocity(work);
        work += (v_curr - v_prev) * v_curr.abs();
        if i == start {
            work *= 0.5;
        }
    }
    kinetic_energy_to_velocity(work)
}

/// How fast a pointer moves (Compose's VelocityTracker, its Lsq2 strategy on each axis): feed it positions (or the
/// changes of a gesture: [`add_pointer_input_change`](VelocityTracker::add_pointer_input_change)) and ask for the
/// velocity when the pointer lifts.
#[derive(Clone)]
pub struct VelocityTracker {
    x: VelocityTracker1D,
    y: VelocityTracker1D,
    last_move_event_time_stamp: i64,
}

impl Default for VelocityTracker {
    fn default() -> Self {
        VelocityTracker::new()
    }
}

impl VelocityTracker {
    pub fn new() -> VelocityTracker {
        VelocityTracker {
            x: VelocityTracker1D::with_strategy(false, Strategy::Lsq2),
            y: VelocityTracker1D::with_strategy(false, Strategy::Lsq2),
            last_move_event_time_stamp: 0,
        }
    }

    /// A position at a time (Compose's `addPosition`).
    pub fn add_position(&mut self, time_millis: i64, position: Offset) {
        self.x.add_data_point(time_millis, position.x);
        self.y.add_data_point(time_millis, position.y);
    }

    /// The velocity, px/s (Compose's `calculateVelocity`).
    pub fn calculate_velocity(&self) -> Velocity {
        self.calculate_velocity_max(Velocity::new(f32::MAX, f32::MAX))
    }

    /// The velocity, no faster than `maximum_velocity` along either axis.
    pub fn calculate_velocity_max(&self, maximum_velocity: Velocity) -> Velocity {
        assert!(
            maximum_velocity.x > 0.0 && maximum_velocity.y > 0.0,
            "maximumVelocity should be a positive value. You specified={maximum_velocity:?}"
        );
        Velocity::new(
            self.x.calculate_velocity_max(maximum_velocity.x),
            self.y.calculate_velocity_max(maximum_velocity.y),
        )
    }

    /// Forgets the samples.
    pub fn reset_tracking(&mut self) {
        self.x.reset_tracking();
        self.y.reset_tracking();
        self.last_move_event_time_stamp = 0;
    }

    /// A change of a gesture, where it is in the window (so layout moving under the pointer doesn't count): a press
    /// starts anew; a release after a 40 ms pause forgets the motion (Compose's `addPointerInputChange`).
    pub fn add_pointer_input_change(&mut self, change: &PointerInputChange) {
        self.add_pointer_input_change_offset(change, Offset::Zero);
    }

    /// The same, the positions moved by `offset`.
    pub fn add_pointer_input_change_offset(&mut self, change: &PointerInputChange, offset: Offset) {
        if change.changed_to_down_ignore_consumed() {
            self.reset_tracking();
        }
        if !change.changed_to_up_ignore_consumed() {
            for h in change.historical() {
                self.add_position(h.uptime_millis, h.original_event_position + offset);
            }
            self.add_position(change.uptime_millis, change.original_event_position + offset);
        }
        if change.changed_to_up_ignore_consumed()
            && change.uptime_millis - self.last_move_event_time_stamp > ASSUME_POINTER_MOVE_STOPPED_MILLISECONDS
        {
            self.reset_tracking();
        }
        self.last_move_event_time_stamp = change.uptime_millis;
    }
}

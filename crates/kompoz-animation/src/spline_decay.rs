//! Android's fling curve (Compose animation's SplineBasedDecay.kt, FlingCalculator.kt,
//! SplineBasedFloatDecayAnimationSpec.kt): OverScroller's spline, how far and how long a fling goes at a velocity
//! for the screen's density, and the decay of flings along it.

use crate::MillisToNanos;
use crate::decay::{FloatDecayAnimationSpec, FloatDecaySpecOf, generate_decay_animation_spec};
use kompoz_graphics::Density;
use kompoz_runtime::{kompozable, remember_keyed};
use kompoz_ui::platform::LocalDensity;
use std::sync::OnceLock;

const Inflection: f32 = 0.35; // (the tension lines cross at (Inflection, 1))
const StartTension: f32 = 0.5;
const EndTension: f32 = 1.0;
const P1: f32 = StartTension * Inflection;
const P2: f32 = 1.0 - EndTension * (1.0 - Inflection);

/// The spline's position and time at each of `nb_samples` (Compose's computeSplineInfo).
fn compute_spline_info(spline_positions: &mut [f32], spline_times: &mut [f32], nb_samples: usize) {
    let mut x_min = 0.0f32;
    let mut y_min = 0.0f32;
    for i in 0..nb_samples {
        let alpha = i as f32 / nb_samples as f32;
        let mut x_max = 1.0f32;
        let mut x;
        let mut coef;
        loop {
            x = x_min + (x_max - x_min) / 2.0;
            coef = 3.0 * x * (1.0 - x);
            let tx = coef * ((1.0 - x) * P1 + x * P2) + x * x * x;
            // (Kotlin's `abs(tx - alpha) < 1E-5`: a float against a double)
            if ((tx - alpha).abs() as f64) < 1e-5 {
                break;
            }
            if tx > alpha {
                x_max = x;
            } else {
                x_min = x;
            }
        }
        spline_positions[i] = coef * ((1.0 - x) * StartTension + x) + x * x * x;
        let mut y_max = 1.0f32;
        let mut y;
        loop {
            y = y_min + (y_max - y_min) / 2.0;
            coef = 3.0 * y * (1.0 - y);
            let dy = coef * ((1.0 - y) * StartTension + y) + y * y * y;
            if ((dy - alpha).abs() as f64) < 1e-5 {
                break;
            }
            if dy > alpha {
                y_max = y;
            } else {
                y_min = y;
            }
        }
        spline_times[i] = coef * ((1.0 - y) * P1 + y * P2) + y * y * y;
    }
    spline_times[nb_samples] = 1.0;
    spline_positions[nb_samples] = spline_times[nb_samples];
}

/// Where a fling is at a fraction of its time (Compose's AndroidFlingSpline.FlingResult): the fraction of its
/// distance, and how fast that grows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FlingResult {
    pub distance_coefficient: f32,
    pub velocity_coefficient: f32,
}

/// Android's OverScroller spline (Compose's internal AndroidFlingSpline).
pub struct AndroidFlingSpline;

const NbSamples: usize = 100;

/// The spline's positions at each hundredth of its time (Compose's SplinePositions; its SplineTimes, computed
/// along, aren't used).
fn spline_positions() -> &'static [f32; NbSamples + 1] {
    static POSITIONS: OnceLock<[f32; NbSamples + 1]> = OnceLock::new();
    POSITIONS.get_or_init(|| {
        let mut positions = [0.0; NbSamples + 1];
        let mut times = [0.0; NbSamples + 1];
        compute_spline_info(&mut positions, &mut times, NbSamples);
        positions
    })
}

impl AndroidFlingSpline {
    /// Where a fling is at `time`, a fraction (0..1) of its duration.
    pub fn fling_position(time: f32) -> FlingResult {
        let clamped_time = time.clamp(0.0, 1.0);
        let index = (NbSamples as f32 * clamped_time) as i32 as usize;
        let mut distance_coef = 1.0;
        let mut velocity_coef = 0.0;
        if index < NbSamples {
            let positions = spline_positions();
            let t_inf = index as f32 / NbSamples as f32;
            let t_sup = (index + 1) as f32 / NbSamples as f32;
            let d_inf = positions[index];
            let d_sup = positions[index + 1];
            velocity_coef = (d_sup - d_inf) / (t_sup - t_inf);
            distance_coef = d_inf + (clamped_time - t_inf) * velocity_coef;
        }
        FlingResult { distance_coefficient: distance_coef, velocity_coefficient: velocity_coef }
    }

    /// The log of a fling's deceleration at `velocity` against `friction` (Compose's deceleration).
    pub fn deceleration(velocity: f32, friction: f32) -> f64 {
        ((Inflection * velocity.abs()) as f64 / friction as f64).ln()
    }
}

const GravityEarth: f32 = 9.80665;
const InchesPerMeter: f32 = 39.37;

/// Kotlin's `(ln(0.78) / ln(0.9)).toFloat()`.
fn deceleration_rate() -> f32 {
    (0.78f64.ln() / 0.9f64.ln()) as f32
}

fn compute_deceleration(friction: f32, density: f32) -> f32 {
    GravityEarth * InchesPerMeter * density * 160.0 * friction
}

/// Kotlin's `sign(Float)` (Math.signum: a zero keeps its sign, NaN stays NaN).
fn signf(x: f32) -> f32 {
    if x.is_nan() || x == 0.0 { x } else { x.signum() }
}

/// How far and how long a fling goes (Compose's internal FlingCalculator): Android's, for a scroll friction and
/// the screen's density (px per dp).
#[derive(Clone, Copy, Debug)]
pub struct FlingCalculator {
    friction: f32,
    pub density: Density,
    magic_physical_coefficient: f32,
}

/// A fling at a velocity (Compose's FlingCalculator.FlingInfo): its distance and duration (ms).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FlingInfo {
    pub initial_velocity: f32,
    pub distance: f32,
    pub duration: i64,
}

impl FlingInfo {
    /// How far it has gone at `time` ms.
    pub fn position(&self, time: i64) -> f32 {
        let spline_pos = if self.duration > 0 { time as f32 / self.duration as f32 } else { 1.0 };
        self.distance
            * signf(self.initial_velocity)
            * AndroidFlingSpline::fling_position(spline_pos).distance_coefficient
    }

    /// How fast it goes at `time` ms (px per s).
    pub fn velocity(&self, time: i64) -> f32 {
        let spline_pos = if self.duration > 0 { time as f32 / self.duration as f32 } else { 1.0 };
        AndroidFlingSpline::fling_position(spline_pos).velocity_coefficient
            * signf(self.initial_velocity)
            * self.distance
            / self.duration as f32
            * 1000.0
    }
}

impl FlingCalculator {
    pub fn new(friction: f32, density: Density) -> FlingCalculator {
        FlingCalculator { friction, density, magic_physical_coefficient: compute_deceleration(0.84, density.density) }
    }

    fn get_spline_deceleration(&self, velocity: f32) -> f64 {
        AndroidFlingSpline::deceleration(velocity, self.friction * self.magic_physical_coefficient)
    }

    /// How long (ms) a fling at `velocity` goes.
    pub fn fling_duration(&self, velocity: f32) -> i64 {
        let l = self.get_spline_deceleration(velocity);
        let decel_minus_one = deceleration_rate() as f64 - 1.0;
        (1000.0 * (l / decel_minus_one).exp()) as i64
    }

    /// How far (px) a fling at `velocity` goes, unsigned.
    pub fn fling_distance(&self, velocity: f32) -> f32 {
        let l = self.get_spline_deceleration(velocity);
        let decel_minus_one = deceleration_rate() as f64 - 1.0;
        ((self.friction * self.magic_physical_coefficient) as f64
            * (deceleration_rate() as f64 / decel_minus_one * l).exp()) as f32
    }

    /// The fling at `velocity`.
    pub fn fling_info(&self, velocity: f32) -> FlingInfo {
        let l = self.get_spline_deceleration(velocity);
        let decel_minus_one = deceleration_rate() as f64 - 1.0;
        FlingInfo {
            initial_velocity: velocity,
            distance: ((self.friction * self.magic_physical_coefficient) as f64
                * (deceleration_rate() as f64 / decel_minus_one * l).exp()) as f32,
            duration: (1000.0 * (l / decel_minus_one).exp()) as i64,
        }
    }
}

/// The platform's scroll friction (Compose's platformFlingScrollFriction: Android's
/// `ViewConfiguration.getScrollFriction()`, 0.015, which Compose for desktop uses too).
pub const PlatformFlingScrollFriction: f32 = 0.015;

/// Flings along Android's curve (Compose's SplineBasedFloatDecayAnimationSpec): a faster fling goes much further
/// (its distance grows as the velocity to the power 2.36…); it has no velocity threshold.
#[derive(Clone, Copy, Debug)]
pub struct SplineBasedFloatDecayAnimationSpec {
    fling_calculator: FlingCalculator,
}

impl SplineBasedFloatDecayAnimationSpec {
    /// The fling of a screen of `density`.
    pub fn new(density: Density) -> SplineBasedFloatDecayAnimationSpec {
        SplineBasedFloatDecayAnimationSpec {
            fling_calculator: FlingCalculator::new(PlatformFlingScrollFriction, density),
        }
    }

    fn fling_distance(&self, start_velocity: f32) -> f32 {
        self.fling_calculator.fling_distance(start_velocity) * signf(start_velocity)
    }
}

impl FloatDecayAnimationSpec for SplineBasedFloatDecayAnimationSpec {
    fn abs_velocity_threshold(&self) -> f32 {
        0.0
    }
    fn get_target_value(&self, initial_value: f32, initial_velocity: f32) -> f32 {
        initial_value + self.fling_distance(initial_velocity)
    }
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: f32, initial_velocity: f32) -> f32 {
        let play_time_millis = play_time_nanos / 1_000_000;
        initial_value + self.fling_calculator.fling_info(initial_velocity).position(play_time_millis)
    }
    fn get_duration_nanos(&self, _: f32, initial_velocity: f32) -> i64 {
        self.fling_calculator.fling_duration(initial_velocity).wrapping_mul(MillisToNanos)
    }
    fn get_velocity_from_nanos(&self, play_time_nanos: i64, _: f32, initial_velocity: f32) -> f32 {
        let play_time_millis = play_time_nanos / 1_000_000;
        self.fling_calculator.fling_info(initial_velocity).velocity(play_time_millis)
    }
}

/// Android's fling as a decay of any type, for a screen of `density` (Compose's `splineBasedDecay(density)`).
pub fn spline_based_decay<T>(density: Density) -> FloatDecaySpecOf<T> {
    generate_decay_animation_spec(SplineBasedFloatDecayAnimationSpec::new(density))
}

/// Android's fling for this screen's density (Compose's rememberSplineBasedDecay: the host's `LocalDensity`), kept
/// while the density stays.
#[kompozable]
pub fn remember_spline_based_decay<T: 'static>() -> FloatDecaySpecOf<T> {
    let density = LocalDensity.current();
    remember_keyed(density.density.to_bits(), || spline_based_decay(density))
}

/// The default decay of flings (Compose's deprecated defaultDecayAnimationSpec: rememberSplineBasedDecay of floats).
#[kompozable]
pub fn default_decay_animation_spec() -> FloatDecaySpecOf<f32> {
    remember_spline_based_decay::<f32>()
}

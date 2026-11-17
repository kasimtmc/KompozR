//! A damped spring (Compose's SpringSimulation.kt), its constants (Spring) and how long it takes to settle
//! (SpringEstimation.kt).

use kompoz_runtime::kt::panic_kt;

/// A spring's common stiffnesses and damping ratios (Compose's `Spring` object).
pub struct Spring;

impl Spring {
    /// Stiff: quick.
    pub const StiffnessHigh: f32 = 10_000.0;
    /// The default.
    pub const StiffnessMedium: f32 = 1500.0;
    pub const StiffnessMediumLow: f32 = 400.0;
    pub const StiffnessLow: f32 = 200.0;
    /// Soft: slow.
    pub const StiffnessVeryLow: f32 = 50.0;
    pub const DampingRatioHighBouncy: f32 = 0.2;
    pub const DampingRatioMediumBouncy: f32 = 0.5;
    pub const DampingRatioLowBouncy: f32 = 0.75;
    /// Critically damped: no bounce (the default).
    pub const DampingRatioNoBouncy: f32 = 1.0;
    /// How close to its target a spring of floats is close enough (0.01).
    pub const DefaultDisplacementThreshold: f32 = 0.01;
}

/// A value and its velocity (Compose's internal Motion, packed in a Long there).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Motion {
    pub(crate) value: f32,
    pub(crate) velocity: f32,
}

/// A spring of unit mass toward `final_position` (Compose's internal SpringSimulation), in closed form.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpringSimulation {
    pub(crate) final_position: f32,
    natural_freq: f64,
    damping_ratio: f32,
}

impl SpringSimulation {
    pub(crate) fn new(final_position: f32) -> SpringSimulation {
        SpringSimulation {
            final_position,
            natural_freq: (Spring::StiffnessVeryLow as f64).sqrt(),
            damping_ratio: Spring::DampingRatioNoBouncy,
        }
    }

    pub(crate) fn stiffness(&self) -> f32 {
        (self.natural_freq * self.natural_freq) as f32
    }

    /// Sets the stiffness. (Compose checks the stiffness it had, not the new one, to be positive: so does this.)
    #[track_caller]
    pub(crate) fn set_stiffness(&mut self, value: f32) {
        if self.stiffness() <= 0.0 {
            panic_kt("IllegalArgumentException", "Spring stiffness constant must be positive.");
        }
        self.natural_freq = (value as f64).sqrt();
    }

    pub(crate) fn damping_ratio(&self) -> f32 {
        self.damping_ratio
    }

    #[track_caller]
    pub(crate) fn set_damping_ratio(&mut self, value: f32) {
        if value < 0.0 {
            panic_kt("IllegalArgumentException", "Damping ratio must be non-negative");
        }
        self.damping_ratio = value;
    }

    /// Where it is and how fast it goes `time_elapsed` ms after it was at `last_displacement` going at
    /// `last_velocity` (Compose's updateValues).
    pub(crate) fn update_values(&self, last_displacement: f32, last_velocity: f32, time_elapsed: i64) -> Motion {
        let adjusted_displacement = (last_displacement - self.final_position) as f64;
        let last_velocity = last_velocity as f64;
        let delta_t = time_elapsed as f64 / 1000.0;
        let damping_ratio = self.damping_ratio as f64;
        let damping_ratio_squared = damping_ratio * damping_ratio;
        let natural_freq = self.natural_freq;
        let r = -damping_ratio * natural_freq;
        let displacement: f64;
        let current_velocity: f64;
        if self.damping_ratio > 1.0 {
            let s = natural_freq * (damping_ratio_squared - 1.0).sqrt();
            let gamma_plus = r + s;
            let gamma_minus = r - s;
            let coeff_b = (gamma_minus * adjusted_displacement - last_velocity) / (gamma_minus - gamma_plus);
            let coeff_a = adjusted_displacement - coeff_b;
            displacement = coeff_a * (gamma_minus * delta_t).exp() + coeff_b * (gamma_plus * delta_t).exp();
            current_velocity = coeff_a * gamma_minus * (gamma_minus * delta_t).exp()
                + coeff_b * gamma_plus * (gamma_plus * delta_t).exp();
        } else if self.damping_ratio == 1.0 {
            let coeff_a = adjusted_displacement;
            let coeff_b = last_velocity + natural_freq * adjusted_displacement;
            let n_f_d_t = -natural_freq * delta_t;
            displacement = (coeff_a + coeff_b * delta_t) * n_f_d_t.exp();
            current_velocity =
                ((coeff_a + coeff_b * delta_t) * n_f_d_t.exp() * (-natural_freq)) + coeff_b * n_f_d_t.exp();
        } else {
            let damped_freq = natural_freq * (1.0 - damping_ratio_squared).sqrt();
            let cos_coeff = adjusted_displacement;
            let sin_coeff = (1.0 / damped_freq) * ((-r * adjusted_displacement) + last_velocity);
            let d_f_d_t = damped_freq * delta_t;
            displacement = (r * delta_t).exp() * (cos_coeff * d_f_d_t.cos() + sin_coeff * d_f_d_t.sin());
            current_velocity = displacement * r
                + ((r * delta_t).exp()
                    * (-damped_freq * cos_coeff * d_f_d_t.sin() + damped_freq * sin_coeff * d_f_d_t.cos()));
        }
        let new_value = (displacement + self.final_position as f64) as f32;
        let new_velocity = current_velocity as f32;
        Motion { value: new_value, velocity: new_velocity }
    }
}

// ---- How long a spring takes (Compose's SpringEstimation.kt) --------------------------------------------------

const MAX_LONG_MILLIS: i64 = i64::MAX / 1_000_000;

/// How long (ms) a spring of that stiffness and damping ratio takes from that displacement and velocity to stay
/// within `delta` of its target (Compose's estimateAnimationDurationMillis, of floats; restricted to its library
/// there): forever (`Long.MAX_VALUE / 1_000_000`) for an undamped one.
pub fn estimate_animation_duration_millis(
    stiffness: f32,
    damping_ratio: f32,
    initial_velocity: f32,
    initial_displacement: f32,
    delta: f32,
) -> i64 {
    if damping_ratio == 0.0 {
        return MAX_LONG_MILLIS;
    }
    estimate_animation_duration_millis_f64(
        stiffness as f64,
        damping_ratio as f64,
        initial_velocity as f64,
        initial_displacement as f64,
        delta as f64,
    )
}

/// The same in doubles (Compose's overload of Doubles).
/// Kotlin: `estimateAnimationDurationMillis(stiffness: Double, dampingRatio: Double, initialVelocity: Double, initialDisplacement: Double, delta: Double)`
pub fn estimate_animation_duration_millis_f64(
    stiffness: f64,
    damping_ratio: f64,
    initial_velocity: f64,
    initial_displacement: f64,
    delta: f64,
) -> i64 {
    let damping_coefficient = 2.0 * damping_ratio * stiffness.sqrt();
    let partial_root = damping_coefficient * damping_coefficient - 4.0 * stiffness;
    let partial_root_real = if partial_root < 0.0 { 0.0 } else { partial_root.sqrt() };
    let partial_root_imaginary = if partial_root < 0.0 { partial_root.abs().sqrt() } else { 0.0 };
    let first_root_real = (-damping_coefficient + partial_root_real) * 0.5;
    let first_root_imaginary = partial_root_imaginary * 0.5;
    let second_root_real = (-damping_coefficient - partial_root_real) * 0.5;
    estimate_duration_internal(
        first_root_real,
        first_root_imaginary,
        second_root_real,
        damping_ratio,
        initial_velocity,
        initial_displacement,
        delta,
    )
}

/// The same of a spring given by its constant, damping coefficient and mass (Compose's overload of those).
/// Kotlin: `estimateAnimationDurationMillis(springConstant: Double, dampingCoefficient: Double, mass: Double, initialVelocity: Double, initialDisplacement: Double, delta: Double)`
pub fn estimate_animation_duration_millis_of_mass(
    spring_constant: f64,
    damping_coefficient: f64,
    mass: f64,
    initial_velocity: f64,
    initial_displacement: f64,
    delta: f64,
) -> i64 {
    let critical_damping = 2.0 * (spring_constant * mass).sqrt();
    let damping_ratio = damping_coefficient / critical_damping;
    let partial_root = damping_coefficient * damping_coefficient - 4.0 * mass * spring_constant;
    let divisor = 1.0 / (2.0 * mass);
    let partial_root_real = if partial_root < 0.0 { 0.0 } else { partial_root.sqrt() };
    let partial_root_imaginary = if partial_root < 0.0 { partial_root.abs().sqrt() } else { 0.0 };
    let first_root_real = (-damping_coefficient + partial_root_real) * divisor;
    let first_root_imaginary = partial_root_imaginary * divisor;
    let second_root_real = (-damping_coefficient - partial_root_real) * divisor;
    estimate_duration_internal(
        first_root_real,
        first_root_imaginary,
        second_root_real,
        damping_ratio,
        initial_velocity,
        initial_displacement,
        delta,
    )
}

fn estimate_under_damped(first_root_real: f64, first_root_imaginary: f64, p0: f64, v0: f64, delta: f64) -> f64 {
    let r = first_root_real;
    let c1 = p0;
    let c2 = (v0 - r * c1) / first_root_imaginary;
    let c = (c1 * c1 + c2 * c2).sqrt();
    (delta / c).ln() / r
}

fn estimate_critically_damped(first_root_real: f64, p0: f64, v0: f64, delta: f64) -> f64 {
    let r = first_root_real;
    let c1 = p0;
    let c2 = v0 - r * c1;
    let t1 = (delta / c1).abs().ln() / r;
    let t2 = {
        let guess = (delta / c2).abs().ln();
        let mut t = guess;
        for _ in 0..=5 {
            t = guess - (t / r).abs().ln();
        }
        t
    } / r;
    let mut t_curr = if !t1.is_finite() {
        t2
    } else if !t2.is_finite() {
        t1
    } else {
        kotlin_max(t1, t2)
    };
    let t_inflection = -(r * c1 + c2) / (r * c2);
    let x_inflection = c1 * (r * t_inflection).exp() + c2 * t_inflection * (r * t_inflection).exp();
    let signed_delta = if t_inflection.is_nan() || t_inflection <= 0.0 {
        -delta
    } else if t_inflection > 0.0 && -x_inflection < delta {
        if c2 < 0.0 && c1 > 0.0 {
            t_curr = 0.0;
        }
        -delta
    } else {
        t_curr = -(2.0 / r) - (c1 / c2);
        delta
    };
    let mut t_delta = f64::MAX;
    let mut iterations = 0;
    while t_delta > 0.001 && iterations < 100 {
        iterations += 1;
        let t_last = t_curr;
        t_curr = iterate_newtons_method(
            t_curr,
            |t| (c1 + c2 * t) * (r * t).exp() + signed_delta,
            |t| (c2 * (r * t + 1.0) + c1 * r) * (r * t).exp(),
        );
        t_delta = (t_last - t_curr).abs();
    }
    t_curr
}

fn estimate_over_damped(first_root_real: f64, second_root_real: f64, p0: f64, v0: f64, delta: f64) -> f64 {
    let r1 = first_root_real;
    let r2 = second_root_real;
    let c2 = (r1 * p0 - v0) / (r1 - r2);
    let c1 = p0 - c2;
    let t1 = (delta / c1).abs().ln() / r1;
    let t2 = (delta / c2).abs().ln() / r2;
    let mut t_curr = if !t1.is_finite() {
        t2
    } else if !t2.is_finite() {
        t1
    } else {
        kotlin_max(t1, t2)
    };
    let t_inflection = ((c1 * r1) / (-c2 * r2)).ln() / (r2 - r1);
    let x_inflection = || c1 * (r1 * t_inflection).exp() + c2 * (r2 * t_inflection).exp();
    let signed_delta = if t_inflection.is_nan() || t_inflection <= 0.0 {
        -delta
    } else if t_inflection > 0.0 && -x_inflection() < delta {
        if c2 > 0.0 && c1 < 0.0 {
            t_curr = 0.0;
        }
        -delta
    } else {
        t_curr = (-(c2 * r2 * r2) / (c1 * r1 * r1)).ln() / (r1 - r2);
        delta
    };
    if (c1 * r1 * (r1 * t_curr).exp() + c2 * r2 * (r2 * t_curr).exp()).abs() < 0.0001 {
        return t_curr;
    }
    let mut t_delta = f64::MAX;
    let mut iterations = 0;
    while t_delta > 0.001 && iterations < 100 {
        iterations += 1;
        let t_last = t_curr;
        t_curr = iterate_newtons_method(
            t_curr,
            |t| c1 * (r1 * t).exp() + c2 * (r2 * t).exp() + signed_delta,
            |t| c1 * r1 * (r1 * t).exp() + c2 * r2 * (r2 * t).exp(),
        );
        t_delta = (t_last - t_curr).abs();
    }
    t_curr
}

/// Kotlin's `max(a, b)` of doubles (Java's Math.max: a NaN wins).
fn kotlin_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) }
}

fn estimate_duration_internal(
    first_root_real: f64,
    first_root_imaginary: f64,
    second_root_real: f64,
    damping_ratio: f64,
    initial_velocity: f64,
    initial_position: f64,
    delta: f64,
) -> i64 {
    if initial_position == 0.0 && initial_velocity == 0.0 {
        return 0;
    }
    let v0 = if initial_position < 0.0 { -initial_velocity } else { initial_velocity };
    let p0 = initial_position.abs();
    let seconds = if damping_ratio > 1.0 {
        estimate_over_damped(first_root_real, second_root_real, p0, v0, delta)
    } else if damping_ratio < 1.0 {
        estimate_under_damped(first_root_real, first_root_imaginary, p0, v0, delta)
    } else {
        estimate_critically_damped(first_root_real, p0, v0, delta)
    };
    // (Kotlin's Double.toLong(): toward zero, saturated, NaN to 0 — Rust's `as`)
    (seconds * 1000.0) as i64
}

fn iterate_newtons_method(x: f64, f: impl Fn(f64) -> f64, f_prime: impl Fn(f64) -> f64) -> f64 {
    x - f(x) / f_prime(x)
}

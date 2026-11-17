//! Specs of one float (Compose's FloatAnimationSpec.kt): a spring and a tween.

use crate::MillisToNanos;
use crate::easing::{Easing, FastOutSlowInEasing, easing_eq};
use crate::spec::AnimationConstants;
use crate::spec::animation_spec_impls;
use crate::spring_simulation::{Spring, SpringSimulation, estimate_animation_duration_millis};
use crate::vectorized::{VectorizedFloatAnimationSpec, lerp};
use std::sync::Arc;

/// An animation of a float (Compose's FloatAnimationSpec, an `AnimationSpec<Float>` itself: on vectors, it runs
/// on each float).
pub trait FloatAnimationSpec: Send + Sync + 'static {
    /// The value `play_time_nanos` into it.
    fn get_value_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: f32,
        target_value: f32,
        initial_velocity: f32,
    ) -> f32;

    /// The velocity `play_time_nanos` into it.
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: f32,
        target_value: f32,
        initial_velocity: f32,
    ) -> f32;

    /// The velocity at its end.
    fn get_end_velocity(&self, initial_value: f32, target_value: f32, initial_velocity: f32) -> f32 {
        self.get_velocity_from_nanos(
            self.get_duration_nanos(initial_value, target_value, initial_velocity),
            initial_value,
            target_value,
            initial_velocity,
        )
    }

    /// How long it takes.
    fn get_duration_nanos(&self, initial_value: f32, target_value: f32, initial_velocity: f32) -> i64;
}

/// A spring of a float (Compose's FloatSpringSpec): it ends where it stays within `visibility_threshold` of its
/// target.
#[derive(Clone, Debug)]
pub struct FloatSpringSpec {
    pub damping_ratio: f32,
    pub stiffness: f32,
    visibility_threshold: f32,
    spring: SpringSimulation,
}

impl FloatSpringSpec {
    #[track_caller]
    pub fn new(damping_ratio: f32, stiffness: f32, visibility_threshold: f32) -> FloatSpringSpec {
        let mut spring = SpringSimulation::new(1.0);
        spring.set_damping_ratio(damping_ratio);
        spring.set_stiffness(stiffness);
        FloatSpringSpec { damping_ratio, stiffness, visibility_threshold, spring }
    }
}

impl Default for FloatSpringSpec {
    /// Compose's defaults: no bounce, medium stiffness, floats' threshold.
    fn default() -> Self {
        FloatSpringSpec::new(
            Spring::DampingRatioNoBouncy,
            Spring::StiffnessMedium,
            Spring::DefaultDisplacementThreshold,
        )
    }
}

impl PartialEq for FloatSpringSpec {
    /// (Compose's FloatSpringSpec has no `equals`: one equals itself only; here, one of the same parameters.)
    fn eq(&self, other: &Self) -> bool {
        self.damping_ratio == other.damping_ratio
            && self.stiffness == other.stiffness
            && self.visibility_threshold == other.visibility_threshold
    }
}

impl FloatAnimationSpec for FloatSpringSpec {
    fn get_value_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: f32,
        target_value: f32,
        initial_velocity: f32,
    ) -> f32 {
        let play_time_millis = play_time_nanos / MillisToNanos;
        let mut spring = self.spring;
        spring.final_position = target_value;
        spring.update_values(initial_value, initial_velocity, play_time_millis).value
    }
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: f32,
        target_value: f32,
        initial_velocity: f32,
    ) -> f32 {
        let play_time_millis = play_time_nanos / MillisToNanos;
        let mut spring = self.spring;
        spring.final_position = target_value;
        spring.update_values(initial_value, initial_velocity, play_time_millis).velocity
    }
    fn get_end_velocity(&self, _: f32, _: f32, _: f32) -> f32 {
        0.0
    }
    fn get_duration_nanos(&self, initial_value: f32, target_value: f32, initial_velocity: f32) -> i64 {
        estimate_animation_duration_millis(
            self.spring.stiffness(),
            self.spring.damping_ratio(),
            initial_velocity / self.visibility_threshold,
            (initial_value - target_value) / self.visibility_threshold,
            1.0,
        )
        .wrapping_mul(MillisToNanos)
    }
}

/// A tween of a float (Compose's FloatTweenSpec): `duration` ms after `delay` ms, along `easing`.
#[derive(Clone)]
pub struct FloatTweenSpec {
    pub duration: i32,
    pub delay: i32,
    easing: Arc<dyn Easing>,
    duration_nanos: i64,
    delay_nanos: i64,
}

impl FloatTweenSpec {
    pub fn new(duration: i32, delay: i32, easing: Arc<dyn Easing>) -> FloatTweenSpec {
        FloatTweenSpec {
            duration,
            delay,
            easing,
            duration_nanos: duration as i64 * MillisToNanos,
            delay_nanos: delay as i64 * MillisToNanos,
        }
    }

    fn clamp_play_time_nanos(&self, play_time_nanos: i64) -> i64 {
        crate::vectorized::clamp_i64(play_time_nanos - self.delay_nanos, 0, self.duration_nanos)
    }
}

impl Default for FloatTweenSpec {
    /// Compose's defaults: 300 ms, no delay, FastOutSlowIn.
    fn default() -> Self {
        FloatTweenSpec::new(AnimationConstants::DefaultDurationMillis, 0, Arc::new(FastOutSlowInEasing))
    }
}

impl PartialEq for FloatTweenSpec {
    /// (Compose's has no `equals`; here, one of the same parameters.)
    fn eq(&self, other: &Self) -> bool {
        self.duration == other.duration && self.delay == other.delay && easing_eq(&self.easing, &other.easing)
    }
}

impl std::fmt::Debug for FloatTweenSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "FloatTweenSpec(duration={}, delay={})", self.duration, self.delay)
    }
}

impl FloatAnimationSpec for FloatTweenSpec {
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: f32, target_value: f32, _: f32) -> f32 {
        let clamped_play_time_nanos = self.clamp_play_time_nanos(play_time_nanos);
        let raw_fraction =
            if self.duration == 0 { 1.0 } else { clamped_play_time_nanos as f32 / self.duration_nanos as f32 };
        let fraction = self.easing.transform(raw_fraction);
        lerp(initial_value, target_value, fraction)
    }
    fn get_duration_nanos(&self, _: f32, _: f32, _: f32) -> i64 {
        self.delay_nanos + self.duration_nanos
    }
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: f32,
        target_value: f32,
        initial_velocity: f32,
    ) -> f32 {
        let clamped_play_time_nanos = self.clamp_play_time_nanos(play_time_nanos);
        if clamped_play_time_nanos == 0 {
            return initial_velocity;
        }
        let start_num = self.get_value_from_nanos(
            clamped_play_time_nanos - MillisToNanos,
            initial_value,
            target_value,
            initial_velocity,
        );
        let end_num = self.get_value_from_nanos(clamped_play_time_nanos, initial_value, target_value, initial_velocity);
        (end_num - start_num) * 1000.0
    }
}

impl FloatSpringSpec {
    fn vectorize_into<V: crate::AnimationVector>(
        &self,
        _: &crate::TwoWayConverter<f32, V>,
    ) -> VectorizedFloatAnimationSpec<V> {
        VectorizedFloatAnimationSpec::new(Arc::new(self.clone()))
    }
}

impl FloatTweenSpec {
    fn vectorize_into<V: crate::AnimationVector>(
        &self,
        _: &crate::TwoWayConverter<f32, V>,
    ) -> VectorizedFloatAnimationSpec<V> {
        VectorizedFloatAnimationSpec::new(Arc::new(self.clone()))
    }
}

animation_spec_impls!(any [] FloatSpringSpec, f32);
animation_spec_impls!(any [] FloatTweenSpec, f32);

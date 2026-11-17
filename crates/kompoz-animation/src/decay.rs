//! How something flung slows down (Compose's FloatDecayAnimationSpec.kt, DecayAnimationSpec.kt,
//! VectorizedDecayAnimationSpec.kt): an exponential decay, any float decay on vectors, the rest a fling comes to.

use crate::MillisToNanos;
use crate::converter::{TwoWayConverter, VectorConverter};
use crate::vector::{AnimationVector, AnimationVector1D, AnimationVector2D, AnimationVector3D, AnimationVector4D};
use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;

/// A decay of one float (Compose's FloatDecayAnimationSpec): from a value and a velocity, where it is and how fast
/// it goes over time, how long until it is slower than `abs_velocity_threshold`, and where it comes to rest.
pub trait FloatDecayAnimationSpec: Send + Sync + 'static {
    /// The speed under which it has ended.
    fn abs_velocity_threshold(&self) -> f32;
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: f32, initial_velocity: f32) -> f32;
    fn get_duration_nanos(&self, initial_value: f32, initial_velocity: f32) -> i64;
    fn get_velocity_from_nanos(&self, play_time_nanos: i64, initial_value: f32, initial_velocity: f32) -> f32;
    /// Where it comes to rest.
    fn get_target_value(&self, initial_value: f32, initial_velocity: f32) -> f32;
}

const ExponentialDecayFriction: f32 = -4.2;

/// Kotlin's `exp(Float)`: in doubles.
fn expf(x: f32) -> f32 {
    (x as f64).exp() as f32
}
/// Kotlin's `ln(Float)`: in doubles.
fn lnf(x: f32) -> f32 {
    (x as f64).ln() as f32
}

/// The velocity falls off exponentially, `e^(-4.2 · frictionMultiplier · t)` (Compose's FloatExponentialDecaySpec);
/// it ends once slower than `abs_velocity_threshold`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FloatExponentialDecaySpec {
    abs_velocity_threshold: f32,
    friction: f32,
}

impl FloatExponentialDecaySpec {
    /// Compose's `FloatExponentialDecaySpec(frictionMultiplier = 1f, absVelocityThreshold = 0.1f)` (a threshold of
    /// at least 1e-7, a multiplier of at least 1e-4).
    pub fn new(friction_multiplier: f32, abs_velocity_threshold: f32) -> FloatExponentialDecaySpec {
        FloatExponentialDecaySpec {
            abs_velocity_threshold: 0.000_000_1f32.max(abs_velocity_threshold.abs()),
            friction: ExponentialDecayFriction * 0.0001f32.max(friction_multiplier),
        }
    }
}

impl Default for FloatExponentialDecaySpec {
    fn default() -> Self {
        FloatExponentialDecaySpec::new(1.0, 0.1)
    }
}

impl FloatDecayAnimationSpec for FloatExponentialDecaySpec {
    fn abs_velocity_threshold(&self) -> f32 {
        self.abs_velocity_threshold
    }
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: f32, initial_velocity: f32) -> f32 {
        let play_time_millis = play_time_nanos / MillisToNanos;
        initial_value - initial_velocity / self.friction
            + initial_velocity / self.friction * expf(self.friction * play_time_millis as f32 / 1000.0)
    }
    fn get_velocity_from_nanos(&self, play_time_nanos: i64, _: f32, initial_velocity: f32) -> f32 {
        let play_time_millis = play_time_nanos / MillisToNanos;
        initial_velocity * expf((play_time_millis as f32 / 1000.0) * self.friction)
    }
    fn get_duration_nanos(&self, _: f32, initial_velocity: f32) -> i64 {
        // (Kotlin's Float.toLong(): toward zero, saturated; times a million, wrapping as Kotlin's Long does)
        ((1000.0 * lnf(self.abs_velocity_threshold / initial_velocity.abs()) / self.friction) as i64)
            .wrapping_mul(MillisToNanos)
    }
    fn get_target_value(&self, initial_value: f32, initial_velocity: f32) -> f32 {
        if initial_velocity.abs() <= self.abs_velocity_threshold {
            return initial_value;
        }
        let duration: f64 =
            ((self.abs_velocity_threshold / initial_velocity).abs() as f64).ln() / self.friction as f64 * 1000.0;
        initial_value - initial_velocity / self.friction
            + initial_velocity / self.friction * (self.friction as f64 * duration / 1000.0).exp() as f32
    }
}

/// A decay on vectors (Compose's VectorizedDecayAnimationSpec).
pub trait VectorizedDecayAnimationSpec<V: AnimationVector> {
    fn abs_velocity_threshold(&self) -> f32;
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, initial_velocity: V) -> V;
    fn get_duration_nanos(&self, initial_value: V, initial_velocity: V) -> i64;
    fn get_velocity_from_nanos(&self, play_time_nanos: i64, initial_value: V, initial_velocity: V) -> V;
    fn get_target_value(&self, initial_value: V, initial_velocity: V) -> V;
}

/// A decay of any type (Compose's DecayAnimationSpec): it becomes one on a converter's vectors. Shared as
/// `Arc<dyn DecayAnimationSpec<T>>`.
///
/// Implementing one: give the four `vectorize_decay_*d` (one per vector size) and `as_any`.
pub trait DecayAnimationSpec<T>: Send + Sync + 'static {
    #[doc(hidden)]
    fn vectorize_decay_1d(
        &self,
        c: &TwoWayConverter<T, AnimationVector1D>,
    ) -> Box<dyn VectorizedDecayAnimationSpec<AnimationVector1D>>;
    #[doc(hidden)]
    fn vectorize_decay_2d(
        &self,
        c: &TwoWayConverter<T, AnimationVector2D>,
    ) -> Box<dyn VectorizedDecayAnimationSpec<AnimationVector2D>>;
    #[doc(hidden)]
    fn vectorize_decay_3d(
        &self,
        c: &TwoWayConverter<T, AnimationVector3D>,
    ) -> Box<dyn VectorizedDecayAnimationSpec<AnimationVector3D>>;
    #[doc(hidden)]
    fn vectorize_decay_4d(
        &self,
        c: &TwoWayConverter<T, AnimationVector4D>,
    ) -> Box<dyn VectorizedDecayAnimationSpec<AnimationVector4D>>;
    #[doc(hidden)]
    fn as_any(&self) -> &dyn Any;

    /// The decay on `type_converter`'s vectors (Compose's vectorize).
    fn vectorize<V: AnimationVector>(
        &self,
        type_converter: &TwoWayConverter<T, V>,
    ) -> Box<dyn VectorizedDecayAnimationSpec<V>>
    where
        Self: Sized,
    {
        V::vectorize_decay(self, type_converter)
    }

    /// Where a fling of `T`s from `initial_value` at `initial_velocity` comes to rest (Compose's
    /// calculateTargetValue).
    fn calculate_target_value<V: AnimationVector>(
        &self,
        type_converter: &TwoWayConverter<T, V>,
        initial_value: T,
        initial_velocity: T,
    ) -> T
    where
        Self: Sized,
    {
        calculate_target_value_of(self, type_converter, initial_value, initial_velocity)
    }
}

impl<T: 'static> dyn DecayAnimationSpec<T> {
    /// The decay on `type_converter`'s vectors (Compose's vectorize).
    pub fn vectorize<V: AnimationVector>(
        &self,
        type_converter: &TwoWayConverter<T, V>,
    ) -> Box<dyn VectorizedDecayAnimationSpec<V>> {
        V::vectorize_decay(self, type_converter)
    }

    /// Where a fling comes to rest (Compose's calculateTargetValue).
    pub fn calculate_target_value<V: AnimationVector>(
        &self,
        type_converter: &TwoWayConverter<T, V>,
        initial_value: T,
        initial_velocity: T,
    ) -> T {
        calculate_target_value_of(self, type_converter, initial_value, initial_velocity)
    }
}

fn calculate_target_value_of<T, V: AnimationVector>(
    spec: &(impl DecayAnimationSpec<T> + ?Sized),
    type_converter: &TwoWayConverter<T, V>,
    initial_value: T,
    initial_velocity: T,
) -> T {
    let vectorized_spec = V::vectorize_decay(spec, type_converter);
    let target_vector = vectorized_spec.get_target_value(
        type_converter.convert_to_vector(initial_value),
        type_converter.convert_to_vector(initial_velocity),
    );
    type_converter.convert_from_vector(target_vector)
}

/// Where a float's fling comes to rest (Compose's `DecayAnimationSpec<Float>.calculateTargetValue(initialValue,
/// initialVelocity)`).
pub trait FloatDecayTarget {
    /// Kotlin: `calculateTargetValue(initialValue: Float, initialVelocity: Float): Float`
    fn calculate_float_target_value(&self, initial_value: f32, initial_velocity: f32) -> f32;
}

impl<S: DecayAnimationSpec<f32> + ?Sized> FloatDecayTarget for S {
    fn calculate_float_target_value(&self, initial_value: f32, initial_velocity: f32) -> f32 {
        let vectorized_spec = AnimationVector1D::vectorize_decay(self, &f32::VectorConverter);
        vectorized_spec.get_target_value(AnimationVector1D(initial_value), AnimationVector1D(initial_velocity)).value
    }
}

impl<T: 'static> DecayAnimationSpec<T> for Arc<dyn DecayAnimationSpec<T>> {
    fn vectorize_decay_1d(
        &self,
        c: &TwoWayConverter<T, AnimationVector1D>,
    ) -> Box<dyn VectorizedDecayAnimationSpec<AnimationVector1D>> {
        (**self).vectorize_decay_1d(c)
    }
    fn vectorize_decay_2d(
        &self,
        c: &TwoWayConverter<T, AnimationVector2D>,
    ) -> Box<dyn VectorizedDecayAnimationSpec<AnimationVector2D>> {
        (**self).vectorize_decay_2d(c)
    }
    fn vectorize_decay_3d(
        &self,
        c: &TwoWayConverter<T, AnimationVector3D>,
    ) -> Box<dyn VectorizedDecayAnimationSpec<AnimationVector3D>> {
        (**self).vectorize_decay_3d(c)
    }
    fn vectorize_decay_4d(
        &self,
        c: &TwoWayConverter<T, AnimationVector4D>,
    ) -> Box<dyn VectorizedDecayAnimationSpec<AnimationVector4D>> {
        (**self).vectorize_decay_4d(c)
    }
    fn as_any(&self) -> &dyn Any {
        (**self).as_any()
    }
}

/// A float decay on each of a vector's floats, for any type (Compose's internal DecayAnimationSpecImpl and its
/// VectorizedFloatDecaySpec): what [`generate_decay_animation_spec`] makes.
pub struct FloatDecaySpecOf<T> {
    float_decay_spec: Arc<dyn FloatDecayAnimationSpec>,
    _t: PhantomData<fn() -> T>,
}

impl<T> Clone for FloatDecaySpecOf<T> {
    fn clone(&self) -> Self {
        FloatDecaySpecOf { float_decay_spec: self.float_decay_spec.clone(), _t: PhantomData }
    }
}

impl<T> FloatDecaySpecOf<T> {
    /// The float decay it runs.
    pub fn float_decay_spec(&self) -> &Arc<dyn FloatDecayAnimationSpec> {
        &self.float_decay_spec
    }
}

macro_rules! decay_vectorize {
    ($($name:ident: $v:ty),*) => {$(
        fn $name(&self, _: &TwoWayConverter<T, $v>) -> Box<dyn VectorizedDecayAnimationSpec<$v>> {
            Box::new(VectorizedFloatDecaySpec { float_decay_spec: self.float_decay_spec.clone() })
        }
    )*};
}

impl<T: 'static> DecayAnimationSpec<T> for FloatDecaySpecOf<T> {
    decay_vectorize!(
        vectorize_decay_1d: AnimationVector1D,
        vectorize_decay_2d: AnimationVector2D,
        vectorize_decay_3d: AnimationVector3D,
        vectorize_decay_4d: AnimationVector4D
    );
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A float decay as a decay of any type (Compose's `FloatDecayAnimationSpec.generateDecayAnimationSpec()`).
pub fn generate_decay_animation_spec<T>(float_decay_spec: impl FloatDecayAnimationSpec) -> FloatDecaySpecOf<T> {
    let any: &dyn Any = &float_decay_spec;
    let shared = match any.downcast_ref::<Arc<dyn FloatDecayAnimationSpec>>() {
        Some(shared) => shared.clone(),
        None => Arc::new(float_decay_spec),
    };
    FloatDecaySpecOf { float_decay_spec: shared, _t: PhantomData }
}

impl FloatDecayAnimationSpec for Arc<dyn FloatDecayAnimationSpec> {
    fn abs_velocity_threshold(&self) -> f32 {
        (**self).abs_velocity_threshold()
    }
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: f32, initial_velocity: f32) -> f32 {
        (**self).get_value_from_nanos(play_time_nanos, initial_value, initial_velocity)
    }
    fn get_duration_nanos(&self, initial_value: f32, initial_velocity: f32) -> i64 {
        (**self).get_duration_nanos(initial_value, initial_velocity)
    }
    fn get_velocity_from_nanos(&self, play_time_nanos: i64, initial_value: f32, initial_velocity: f32) -> f32 {
        (**self).get_velocity_from_nanos(play_time_nanos, initial_value, initial_velocity)
    }
    fn get_target_value(&self, initial_value: f32, initial_velocity: f32) -> f32 {
        (**self).get_target_value(initial_value, initial_velocity)
    }
}

/// Compose's `exponentialDecay(frictionMultiplier = 1f, absVelocityThreshold = 0.1f)`.
#[kompoz_runtime::named]
pub fn exponential_decay<T>(
    #[default(1.0)] friction_multiplier: f32,
    #[default(0.1)] abs_velocity_threshold: f32,
) -> FloatDecaySpecOf<T> {
    generate_decay_animation_spec(FloatExponentialDecaySpec::new(friction_multiplier, abs_velocity_threshold))
}

/// A float decay on each float of a vector (Compose's private VectorizedFloatDecaySpec).
struct VectorizedFloatDecaySpec {
    float_decay_spec: Arc<dyn FloatDecayAnimationSpec>,
}

impl<V: AnimationVector> VectorizedDecayAnimationSpec<V> for VectorizedFloatDecaySpec {
    fn abs_velocity_threshold(&self) -> f32 {
        self.float_decay_spec.abs_velocity_threshold()
    }
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, initial_velocity: V) -> V {
        let mut value = initial_value.new_instance();
        for i in 0..V::SIZE {
            value.set(
                i,
                self.float_decay_spec.get_value_from_nanos(
                    play_time_nanos,
                    initial_value.get(i),
                    initial_velocity.get(i),
                ),
            );
        }
        value
    }
    fn get_duration_nanos(&self, initial_value: V, initial_velocity: V) -> i64 {
        let mut max_duration = 0i64;
        for i in 0..V::SIZE {
            max_duration = max_duration
                .max(self.float_decay_spec.get_duration_nanos(initial_value.get(i), initial_velocity.get(i)));
        }
        max_duration
    }
    fn get_velocity_from_nanos(&self, play_time_nanos: i64, initial_value: V, initial_velocity: V) -> V {
        let mut velocity = initial_value.new_instance();
        for i in 0..V::SIZE {
            velocity.set(
                i,
                self.float_decay_spec.get_velocity_from_nanos(
                    play_time_nanos,
                    initial_value.get(i),
                    initial_velocity.get(i),
                ),
            );
        }
        velocity
    }
    fn get_target_value(&self, initial_value: V, initial_velocity: V) -> V {
        let mut target = initial_value.new_instance();
        for i in 0..V::SIZE {
            target.set(i, self.float_decay_spec.get_target_value(initial_value.get(i), initial_velocity.get(i)));
        }
        target
    }
}

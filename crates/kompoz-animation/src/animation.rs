//! An animation's whole course from a start (Compose's Animation.kt): to a target along a spec, or a decay.

use crate::MillisToNanos;
use crate::converter::{TwoWayConverter, VectorConverter};
use crate::decay::{
    DecayAnimationSpec, FloatDecayAnimationSpec, VectorizedDecayAnimationSpec, generate_decay_animation_spec,
};
use crate::spec::AnimationSpec;
use crate::vector::{AnimationVector, AnimationVector1D};
use crate::vectorized::VectorizedAnimationSpec;
use kompoz_runtime::kt::panic_kt;
use std::cell::{Cell, RefCell};

/// An animation from its start to its end, asked for its value and velocity at a play time (Compose's
/// Animation): stateless, what `AnimationState`s and `Animatable`s run frame by frame.
pub trait Animation<T, V: AnimationVector> {
    /// How long it runs (`i64::MAX`: forever).
    fn duration_nanos(&self) -> i64;
    fn type_converter(&self) -> &TwoWayConverter<T, V>;
    /// Where it ends.
    fn target_value(&self) -> T;
    /// Whether it never ends.
    fn is_infinite(&self) -> bool;
    fn get_value_from_nanos(&self, play_time_nanos: i64) -> T;
    fn get_velocity_vector_from_nanos(&self, play_time_nanos: i64) -> V;
    /// Whether it has ended at that play time.
    fn is_finished_from_nanos(&self, play_time_nanos: i64) -> bool {
        play_time_nanos >= self.duration_nanos()
    }
    /// Its velocity at a play time as a value (Compose's getVelocityFromNanos).
    fn get_velocity_from_nanos(&self, play_time_nanos: i64) -> T {
        self.type_converter().convert_from_vector(self.get_velocity_vector_from_nanos(play_time_nanos))
    }
    /// How long it runs, in ms (Compose's internal durationMillis).
    fn duration_millis(&self) -> i64 {
        self.duration_nanos() / MillisToNanos
    }
}

/// From a start to a target along a spec, with a start velocity (Compose's TargetBasedAnimation). Its start and
/// target can be changed (what Transition does); it caches its duration and end velocity until then.
pub struct TargetBasedAnimation<T, V: AnimationVector> {
    animation_spec: Box<dyn VectorizedAnimationSpec<V>>,
    type_converter: TwoWayConverter<T, V>,
    initial_value: RefCell<T>,
    target_value: RefCell<T>,
    initial_value_vector: Cell<V>,
    target_value_vector: Cell<V>,
    initial_velocity_vector: V,
    duration_nanos: Cell<i64>,
    end_velocity: Cell<Option<V>>,
}

impl<T: Clone + PartialEq + 'static, V: AnimationVector> TargetBasedAnimation<T, V> {
    /// From `initial_value` to `target_value` along `animation_spec`, from `initial_velocity_vector` (`None`:
    /// still) (Compose's constructor of an `AnimationSpec`).
    pub fn new(
        animation_spec: &(impl AnimationSpec<T> + ?Sized),
        type_converter: TwoWayConverter<T, V>,
        initial_value: T,
        target_value: T,
        initial_velocity_vector: Option<V>,
    ) -> TargetBasedAnimation<T, V> {
        let vectorized = V::vectorize(animation_spec, &type_converter);
        TargetBasedAnimation::of_vectorized(
            vectorized,
            type_converter,
            initial_value,
            target_value,
            initial_velocity_vector,
        )
    }

    /// The same with the start velocity as a value (Compose's `TargetBasedAnimation(animationSpec, typeConverter,
    /// initialValue, targetValue, initialVelocity)`).
    pub fn with_velocity(
        animation_spec: &(impl AnimationSpec<T> + ?Sized),
        type_converter: TwoWayConverter<T, V>,
        initial_value: T,
        target_value: T,
        initial_velocity: T,
    ) -> TargetBasedAnimation<T, V> {
        let v = type_converter.convert_to_vector(initial_velocity);
        TargetBasedAnimation::new(animation_spec, type_converter, initial_value, target_value, Some(v))
    }

    /// Along a spec on vectors already (Compose's internal constructor).
    pub fn of_vectorized(
        animation_spec: Box<dyn VectorizedAnimationSpec<V>>,
        type_converter: TwoWayConverter<T, V>,
        initial_value: T,
        target_value: T,
        initial_velocity_vector: Option<V>,
    ) -> TargetBasedAnimation<T, V> {
        let initial_value_vector = type_converter.convert_to_vector(initial_value.clone());
        let target_value_vector = type_converter.convert_to_vector(target_value.clone());
        let initial_velocity_vector = initial_velocity_vector
            .unwrap_or_else(|| type_converter.convert_to_vector(initial_value.clone()).new_instance());
        TargetBasedAnimation {
            animation_spec,
            type_converter,
            initial_value: RefCell::new(initial_value),
            target_value: RefCell::new(target_value),
            initial_value_vector: Cell::new(initial_value_vector),
            target_value_vector: Cell::new(target_value_vector),
            initial_velocity_vector,
            duration_nanos: Cell::new(-1),
            end_velocity: Cell::new(None),
        }
    }

    /// Where it starts.
    pub fn initial_value(&self) -> T {
        self.initial_value.borrow().clone()
    }

    /// Its start velocity.
    pub fn initial_velocity_vector(&self) -> V {
        self.initial_velocity_vector
    }

    /// Changes its target (Compose's internal `mutableTargetValue`): its duration and end velocity found again.
    pub fn set_target_value(&self, value: T) {
        if *self.target_value.borrow() != value {
            self.target_value_vector.set(self.type_converter.convert_to_vector(value.clone()));
            *self.target_value.borrow_mut() = value;
            self.end_velocity.set(None);
            self.duration_nanos.set(-1);
        }
    }

    /// Changes its start (Compose's internal `mutableInitialValue`).
    pub fn set_initial_value(&self, value: T) {
        if *self.initial_value.borrow() != value {
            self.initial_value_vector.set(self.type_converter.convert_to_vector(value.clone()));
            *self.initial_value.borrow_mut() = value;
            self.end_velocity.set(None);
            self.duration_nanos.set(-1);
        }
    }

    fn end_velocity(&self) -> V {
        if let Some(v) = self.end_velocity.get() {
            return v;
        }
        let v = self.animation_spec.get_end_velocity(
            self.initial_value_vector.get(),
            self.target_value_vector.get(),
            self.initial_velocity_vector,
        );
        self.end_velocity.set(Some(v));
        v
    }
}

impl<T: Clone + PartialEq + 'static, V: AnimationVector> Animation<T, V> for TargetBasedAnimation<T, V> {
    fn duration_nanos(&self) -> i64 {
        if self.duration_nanos.get() < 0 {
            self.duration_nanos.set(self.animation_spec.get_duration_nanos(
                self.initial_value_vector.get(),
                self.target_value_vector.get(),
                self.initial_velocity_vector,
            ));
        }
        self.duration_nanos.get()
    }
    fn type_converter(&self) -> &TwoWayConverter<T, V> {
        &self.type_converter
    }
    fn target_value(&self) -> T {
        self.target_value.borrow().clone()
    }
    fn is_infinite(&self) -> bool {
        self.animation_spec.is_infinite()
    }
    #[track_caller]
    fn get_value_from_nanos(&self, play_time_nanos: i64) -> T {
        if !self.is_finished_from_nanos(play_time_nanos) {
            let v = self.animation_spec.get_value_from_nanos(
                play_time_nanos,
                self.initial_value_vector.get(),
                self.target_value_vector.get(),
                self.initial_velocity_vector,
            );
            for i in 0..V::SIZE {
                if v.get(i).is_nan() {
                    panic_kt(
                        "IllegalStateException",
                        &format!("AnimationVector cannot contain a NaN. {v}. playTimeNanos: {play_time_nanos}"),
                    );
                }
            }
            self.type_converter.convert_from_vector(v)
        } else {
            self.target_value()
        }
    }
    fn get_velocity_vector_from_nanos(&self, play_time_nanos: i64) -> V {
        if !self.is_finished_from_nanos(play_time_nanos) {
            self.animation_spec.get_velocity_from_nanos(
                play_time_nanos,
                self.initial_value_vector.get(),
                self.target_value_vector.get(),
                self.initial_velocity_vector,
            )
        } else {
            self.end_velocity()
        }
    }
}

/// A fling from a start at a velocity, slowing down along a decay (Compose's DecayAnimation): its target is
/// where it comes to rest; its end velocity is kept within the decay's threshold.
pub struct DecayAnimation<T, V: AnimationVector> {
    animation_spec: Box<dyn VectorizedDecayAnimationSpec<V>>,
    type_converter: TwoWayConverter<T, V>,
    initial_value: T,
    initial_value_vector: V,
    initial_velocity_vector: V,
    end_velocity: V,
    target_value: T,
    duration_nanos: i64,
}

impl<T: Clone + 'static, V: AnimationVector> DecayAnimation<T, V> {
    /// From `initial_value` at `initial_velocity_vector` along `animation_spec` (Compose's constructor).
    pub fn new(
        animation_spec: &(impl DecayAnimationSpec<T> + ?Sized),
        type_converter: TwoWayConverter<T, V>,
        initial_value: T,
        initial_velocity_vector: V,
    ) -> DecayAnimation<T, V> {
        let vectorized = V::vectorize_decay(animation_spec, &type_converter);
        DecayAnimation::of_vectorized(vectorized, type_converter, initial_value, initial_velocity_vector)
    }

    /// The same with the velocity as a value.
    pub fn with_velocity(
        animation_spec: &(impl DecayAnimationSpec<T> + ?Sized),
        type_converter: TwoWayConverter<T, V>,
        initial_value: T,
        initial_velocity: T,
    ) -> DecayAnimation<T, V> {
        let v = type_converter.convert_to_vector(initial_velocity);
        DecayAnimation::new(animation_spec, type_converter, initial_value, v)
    }

    /// Along a decay on vectors already (Compose's primary constructor).
    pub fn of_vectorized(
        animation_spec: Box<dyn VectorizedDecayAnimationSpec<V>>,
        type_converter: TwoWayConverter<T, V>,
        initial_value: T,
        initial_velocity_vector: V,
    ) -> DecayAnimation<T, V> {
        let initial_value_vector = type_converter.convert_to_vector(initial_value.clone());
        let target_value = type_converter
            .convert_from_vector(animation_spec.get_target_value(initial_value_vector, initial_velocity_vector));
        let duration_nanos = animation_spec.get_duration_nanos(initial_value_vector, initial_velocity_vector);
        let mut end_velocity =
            animation_spec.get_velocity_from_nanos(duration_nanos, initial_value_vector, initial_velocity_vector);
        let threshold = animation_spec.abs_velocity_threshold();
        for i in 0..V::SIZE {
            end_velocity.set(i, crate::easing::coerce_in(end_velocity.get(i), -threshold, threshold));
        }
        DecayAnimation {
            animation_spec,
            type_converter,
            initial_value,
            initial_value_vector,
            initial_velocity_vector,
            end_velocity,
            target_value,
            duration_nanos,
        }
    }

    pub fn initial_value(&self) -> T {
        self.initial_value.clone()
    }

    pub fn initial_velocity_vector(&self) -> V {
        self.initial_velocity_vector
    }
}

impl<T: Clone + 'static, V: AnimationVector> Animation<T, V> for DecayAnimation<T, V> {
    fn duration_nanos(&self) -> i64 {
        self.duration_nanos
    }
    fn type_converter(&self) -> &TwoWayConverter<T, V> {
        &self.type_converter
    }
    fn target_value(&self) -> T {
        self.target_value.clone()
    }
    fn is_infinite(&self) -> bool {
        false
    }
    fn get_value_from_nanos(&self, play_time_nanos: i64) -> T {
        if !self.is_finished_from_nanos(play_time_nanos) {
            self.type_converter.convert_from_vector(self.animation_spec.get_value_from_nanos(
                play_time_nanos,
                self.initial_value_vector,
                self.initial_velocity_vector,
            ))
        } else {
            self.target_value()
        }
    }
    fn get_velocity_vector_from_nanos(&self, play_time_nanos: i64) -> V {
        if !self.is_finished_from_nanos(play_time_nanos) {
            self.animation_spec.get_velocity_from_nanos(
                play_time_nanos,
                self.initial_value_vector,
                self.initial_velocity_vector,
            )
        } else {
            self.end_velocity
        }
    }
}

/// Compose's `DecayAnimation(animationSpec: FloatDecayAnimationSpec, initialValue, initialVelocity = 0f)`: a float's.
/// Kotlin: `DecayAnimation(animationSpec: FloatDecayAnimationSpec, initialValue: Float, initialVelocity: Float = 0f)`
#[kompoz_runtime::named]
pub fn float_decay_animation(
    animation_spec: impl FloatDecayAnimationSpec,
    initial_value: f32,
    #[default(0.0)] initial_velocity: f32,
) -> DecayAnimation<f32, AnimationVector1D> {
    DecayAnimation::new(
        &generate_decay_animation_spec::<f32>(animation_spec),
        f32::VectorConverter,
        initial_value,
        AnimationVector1D(initial_velocity),
    )
}

/// A spec on vectors as an animation of vectors (Compose's `VectorizedAnimationSpec.createAnimation`).
pub fn create_animation<V: AnimationVector>(
    spec: Box<dyn VectorizedAnimationSpec<V>>,
    initial_value: V,
    target_value: V,
    initial_velocity: V,
) -> TargetBasedAnimation<V, V> {
    TargetBasedAnimation::of_vectorized(
        spec,
        TwoWayConverter(|it: V| it, |it: V| it),
        initial_value,
        target_value,
        Some(initial_velocity),
    )
}

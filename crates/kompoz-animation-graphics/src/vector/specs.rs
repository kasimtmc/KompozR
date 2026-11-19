//! The specs an animated vector's properties play with (Compose's AnimatorAnimationSpecs.kt): a spec played
//! backwards over a duration (`reversed`), and specs one after the other, each from its start time (`combined`).

use kompoz_animation::{
    AnimationSpec, AnimationVector, AnimationVector1D, AnimationVector2D, AnimationVector3D, AnimationVector4D,
    FiniteAnimationSpec, TwoWayConverter, VectorizedAnimationSpec, VectorizedFiniteAnimationSpec,
};
use std::any::Any;
use std::sync::Arc;

/// Nanoseconds in a millisecond (Compose's MillisToNanos).
const MillisToNanos: i64 = 1_000_000;

/// `spec` played backwards over `duration_millis` (Compose's `FiniteAnimationSpec.reversed(durationMillis)`): at
/// `t` it is `spec` at `duration - t` from the target to the start.
pub(crate) fn reversed<T: 'static>(
    spec: Arc<dyn FiniteAnimationSpec<T>>,
    duration_millis: i32,
) -> Arc<dyn FiniteAnimationSpec<T>> {
    Arc::new(ReversedSpec { spec, duration_millis })
}

/// `specs` each played from its start time in ms (Compose's `combined(specs)`): at `t` the last one started by
/// then (the first before any), `t` less its start into it.
pub(crate) fn combined<T: 'static>(
    specs: Vec<(i32, Arc<dyn FiniteAnimationSpec<T>>)>,
) -> Arc<dyn FiniteAnimationSpec<T>> {
    Arc::new(CombinedSpec { specs })
}

/// The trait impls of a finite spec type with an inherent `vectorize_into<V>`; equal only to itself (Compose's
/// classes have no `equals`).
macro_rules! finite_spec_impls {
    ($ty:ident) => {
        impl<T: 'static> AnimationSpec<T> for $ty<T> {
            fn vectorize_1d(
                &self,
                c: &TwoWayConverter<T, AnimationVector1D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector1D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_2d(
                &self,
                c: &TwoWayConverter<T, AnimationVector2D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector2D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_3d(
                &self,
                c: &TwoWayConverter<T, AnimationVector3D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector3D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_4d(
                &self,
                c: &TwoWayConverter<T, AnimationVector4D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector4D>> {
                Box::new(self.vectorize_into(c))
            }
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn equals(&self, other: &dyn Any) -> bool {
                other.downcast_ref::<Self>().is_some_and(|o| std::ptr::eq(o, self))
            }
        }
        impl<T: 'static> FiniteAnimationSpec<T> for $ty<T> {
            fn vectorize_finite_1d(
                &self,
                c: &TwoWayConverter<T, AnimationVector1D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector1D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_finite_2d(
                &self,
                c: &TwoWayConverter<T, AnimationVector2D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector2D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_finite_3d(
                &self,
                c: &TwoWayConverter<T, AnimationVector3D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector3D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_finite_4d(
                &self,
                c: &TwoWayConverter<T, AnimationVector4D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector4D>> {
                Box::new(self.vectorize_into(c))
            }
        }
    };
}

// ---- Reversed -----------------------------------------------------------------------------------------------------

/// Compose's ReversedSpec.
pub(crate) struct ReversedSpec<T> {
    spec: Arc<dyn FiniteAnimationSpec<T>>,
    duration_millis: i32,
}

impl<T: 'static> ReversedSpec<T> {
    fn vectorize_into<V: AnimationVector>(&self, converter: &TwoWayConverter<T, V>) -> VectorizedReversedSpec<V> {
        VectorizedReversedSpec {
            animation: V::vectorize_finite(&*self.spec, converter),
            duration_nanos: self.duration_millis as i64 * MillisToNanos,
        }
    }
}

finite_spec_impls!(ReversedSpec);

/// Compose's VectorizedReversedSpec.
struct VectorizedReversedSpec<V: AnimationVector> {
    animation: Box<dyn VectorizedFiniteAnimationSpec<V>>,
    duration_nanos: i64,
}

impl<V: AnimationVector> VectorizedAnimationSpec<V> for VectorizedReversedSpec<V> {
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, initial_velocity: V) -> V {
        self.animation.get_value_from_nanos(
            self.duration_nanos - play_time_nanos,
            target_value,
            initial_value,
            initial_velocity,
        )
    }
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V {
        negated(self.animation.get_velocity_from_nanos(
            self.duration_nanos - play_time_nanos,
            target_value,
            initial_value,
            initial_velocity,
        ))
    }
    fn get_duration_nanos(&self, _: V, _: V, _: V) -> i64 {
        self.duration_nanos
    }
}

impl<V: AnimationVector> VectorizedFiniteAnimationSpec<V> for VectorizedReversedSpec<V> {}

/// Compose's `V.reversed()`: each of its floats times -1.
fn negated<V: AnimationVector>(v: V) -> V {
    let mut out = v.new_instance();
    for i in 0..v.size() {
        out.set(i, -v.get(i));
    }
    out
}

// ---- Combined -----------------------------------------------------------------------------------------------------

/// Compose's CombinedSpec.
pub(crate) struct CombinedSpec<T> {
    specs: Vec<(i32, Arc<dyn FiniteAnimationSpec<T>>)>,
}

impl<T: 'static> CombinedSpec<T> {
    fn vectorize_into<V: AnimationVector>(&self, converter: &TwoWayConverter<T, V>) -> VectorizedCombinedSpec<V> {
        VectorizedCombinedSpec {
            animations: self
                .specs
                .iter()
                .map(|(time_millis, spec)| {
                    (*time_millis as i64 * MillisToNanos, V::vectorize_finite(&**spec, converter))
                })
                .collect(),
        }
    }
}

finite_spec_impls!(CombinedSpec);

/// Compose's VectorizedCombinedSpec.
struct VectorizedCombinedSpec<V: AnimationVector> {
    animations: Vec<(i64, Box<dyn VectorizedFiniteAnimationSpec<V>>)>,
}

impl<V: AnimationVector> VectorizedCombinedSpec<V> {
    /// The last one started by `play_time_nanos`, else the first.
    fn choose_animation(&self, play_time_nanos: i64) -> &(i64, Box<dyn VectorizedFiniteAnimationSpec<V>>) {
        self.animations
            .iter()
            .rev()
            .find(|(time_nanos, _)| *time_nanos <= play_time_nanos)
            .or_else(|| self.animations.first())
            .unwrap_or_else(|| kompoz_runtime::kt::panic_kt("NoSuchElementException", "List is empty."))
    }
}

impl<V: AnimationVector> VectorizedAnimationSpec<V> for VectorizedCombinedSpec<V> {
    fn get_value_from_nanos(&self, play_time_nanos: i64, initial_value: V, target_value: V, initial_velocity: V) -> V {
        let (time_nanos, animation) = self.choose_animation(play_time_nanos);
        animation.get_value_from_nanos(play_time_nanos - time_nanos, initial_value, target_value, initial_velocity)
    }
    fn get_velocity_from_nanos(
        &self,
        play_time_nanos: i64,
        initial_value: V,
        target_value: V,
        initial_velocity: V,
    ) -> V {
        let (time_nanos, animation) = self.choose_animation(play_time_nanos);
        animation.get_velocity_from_nanos(play_time_nanos - time_nanos, initial_value, target_value, initial_velocity)
    }
    fn get_duration_nanos(&self, initial_value: V, target_value: V, initial_velocity: V) -> i64 {
        let Some((time_nanos, animation)) = self.animations.last() else {
            kompoz_runtime::kt::panic_kt("NoSuchElementException", "List is empty.")
        };
        time_nanos + animation.get_duration_nanos(initial_value, target_value, initial_velocity)
    }
}

impl<V: AnimationVector> VectorizedFiniteAnimationSpec<V> for VectorizedCombinedSpec<V> {}

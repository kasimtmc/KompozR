//! What an animation does over time, of any type (Compose's AnimationSpec.kt): tween, spring, snap, keyframes
//! (with arcs, or along a spline), repeats, an arc, a start delay; vectorized for a type's converter.

use crate::converter::TwoWayConverter;
use crate::easing::{Easing, FastOutSlowInEasing, LinearEasing, arc_easing, easing_eq};
use crate::mono_spline_keyframes::VectorizedMonoSplineKeyframesSpec;
use crate::spring_simulation::Spring;
use crate::vector::{AnimationVector, AnimationVector1D, AnimationVector2D, AnimationVector3D, AnimationVector4D};
use crate::vectorized::{
    ArcMode, StartDelayVectorizedAnimationSpec, VectorizedAnimationSpec, VectorizedDurationBasedAnimationSpec,
    VectorizedFiniteAnimationSpec, VectorizedInfiniteRepeatableSpec, VectorizedKeyframeSpecElementInfo,
    VectorizedKeyframesSpec, VectorizedRepeatableSpec, VectorizedSnapSpec, VectorizedSpringSpec, VectorizedTweenSpec,
    keyframe_timestamps,
};
use kompoz_runtime::kt::panic_kt;
use kompoz_runtime::{KotlinProperties, named};
use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// Compose's AnimationConstants.
pub struct AnimationConstants;

impl AnimationConstants {
    /// A tween's default duration: 300 ms.
    pub const DefaultDurationMillis: i32 = 300;
    /// No time (`Long.MIN_VALUE`): a frame time not known yet.
    pub const UnspecifiedTime: i64 = i64::MIN;
}

/// What an animation of a `T` does over time (Compose's AnimationSpec): it becomes a spec on the vectors a
/// converter makes of `T`s ([`vectorize`](AnimationSpec::vectorize)). Specs are shared as `Arc<dyn
/// AnimationSpec<T>>`; two are equal as Compose's are (a tween to another of the same duration, delay and easing;
/// keyframes only to themselves).
///
/// Implementing one: give the four `vectorize_*d` (one per vector size; [`AnimationVector`] is sealed), `as_any`
/// and `equals`.
pub trait AnimationSpec<T>: Send + Sync + 'static {
    #[doc(hidden)]
    fn vectorize_1d(
        &self,
        c: &TwoWayConverter<T, AnimationVector1D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector1D>>;
    #[doc(hidden)]
    fn vectorize_2d(
        &self,
        c: &TwoWayConverter<T, AnimationVector2D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector2D>>;
    #[doc(hidden)]
    fn vectorize_3d(
        &self,
        c: &TwoWayConverter<T, AnimationVector3D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector3D>>;
    #[doc(hidden)]
    fn vectorize_4d(
        &self,
        c: &TwoWayConverter<T, AnimationVector4D>,
    ) -> Box<dyn VectorizedAnimationSpec<AnimationVector4D>>;
    #[doc(hidden)]
    fn as_any(&self) -> &dyn Any;
    /// Whether `other` is a spec equal to this one (Compose's `equals`).
    #[doc(hidden)]
    fn equals(&self, other: &dyn Any) -> bool;

    /// The spec on `converter`'s vectors (Compose's vectorize).
    fn vectorize<V: AnimationVector>(&self, converter: &TwoWayConverter<T, V>) -> Box<dyn VectorizedAnimationSpec<V>>
    where
        Self: Sized,
    {
        V::vectorize(self, converter)
    }
}

impl<T: 'static> dyn AnimationSpec<T> {
    /// The spec on `converter`'s vectors (Compose's vectorize).
    pub fn vectorize<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<T, V>,
    ) -> Box<dyn VectorizedAnimationSpec<V>> {
        V::vectorize(self, converter)
    }
}

impl<T: 'static> PartialEq for dyn AnimationSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other.as_any())
    }
}

/// A spec that ends (Compose's FiniteAnimationSpec): all but infiniteRepeatable.
pub trait FiniteAnimationSpec<T>: AnimationSpec<T> {
    #[doc(hidden)]
    fn vectorize_finite_1d(
        &self,
        c: &TwoWayConverter<T, AnimationVector1D>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector1D>>;
    #[doc(hidden)]
    fn vectorize_finite_2d(
        &self,
        c: &TwoWayConverter<T, AnimationVector2D>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector2D>>;
    #[doc(hidden)]
    fn vectorize_finite_3d(
        &self,
        c: &TwoWayConverter<T, AnimationVector3D>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector3D>>;
    #[doc(hidden)]
    fn vectorize_finite_4d(
        &self,
        c: &TwoWayConverter<T, AnimationVector4D>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector4D>>;
}

impl<T: 'static> dyn FiniteAnimationSpec<T> {
    /// The spec on `converter`'s vectors (Compose's vectorize of a finite spec).
    pub fn vectorize<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<T, V>,
    ) -> Box<dyn VectorizedFiniteAnimationSpec<V>> {
        V::vectorize_finite(self, converter)
    }
}

impl<T: 'static> PartialEq for dyn FiniteAnimationSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other.as_any())
    }
}

/// A spec of a fixed duration (Compose's DurationBasedAnimationSpec): tween, snap, keyframes, an arc — what
/// repeatable and infiniteRepeatable repeat.
pub trait DurationBasedAnimationSpec<T>: FiniteAnimationSpec<T> {
    #[doc(hidden)]
    fn vectorize_duration_based_1d(
        &self,
        c: &TwoWayConverter<T, AnimationVector1D>,
    ) -> Box<dyn VectorizedDurationBasedAnimationSpec<AnimationVector1D>>;
    #[doc(hidden)]
    fn vectorize_duration_based_2d(
        &self,
        c: &TwoWayConverter<T, AnimationVector2D>,
    ) -> Box<dyn VectorizedDurationBasedAnimationSpec<AnimationVector2D>>;
    #[doc(hidden)]
    fn vectorize_duration_based_3d(
        &self,
        c: &TwoWayConverter<T, AnimationVector3D>,
    ) -> Box<dyn VectorizedDurationBasedAnimationSpec<AnimationVector3D>>;
    #[doc(hidden)]
    fn vectorize_duration_based_4d(
        &self,
        c: &TwoWayConverter<T, AnimationVector4D>,
    ) -> Box<dyn VectorizedDurationBasedAnimationSpec<AnimationVector4D>>;
}

impl<T: 'static> dyn DurationBasedAnimationSpec<T> {
    /// The spec on `converter`'s vectors (Compose's vectorize of a duration-based spec).
    pub fn vectorize<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<T, V>,
    ) -> Box<dyn VectorizedDurationBasedAnimationSpec<V>> {
        V::vectorize_duration_based(self, converter)
    }
}

impl<T: 'static> PartialEq for dyn DurationBasedAnimationSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other.as_any())
    }
}

/// The trait impls of a spec type with an inherent `vectorize_into<V>(&self, &TwoWayConverter<T, V>) -> X<V>`
/// (`X<V>` a vectorized spec of the level given: `any`, `finite`, `duration_based`) and `PartialEq`.
macro_rules! animation_spec_impls {
    (any [$($g:tt)*] $ty:ty, $t:ty) => {
        impl<$($g)*> $crate::AnimationSpec<$t> for $ty {
            fn vectorize_1d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector1D>,
            ) -> Box<dyn $crate::VectorizedAnimationSpec<$crate::AnimationVector1D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_2d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector2D>,
            ) -> Box<dyn $crate::VectorizedAnimationSpec<$crate::AnimationVector2D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_3d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector3D>,
            ) -> Box<dyn $crate::VectorizedAnimationSpec<$crate::AnimationVector3D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_4d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector4D>,
            ) -> Box<dyn $crate::VectorizedAnimationSpec<$crate::AnimationVector4D>> {
                Box::new(self.vectorize_into(c))
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn equals(&self, other: &dyn std::any::Any) -> bool {
                other.downcast_ref::<Self>().is_some_and(|o| o == self)
            }
        }
    };
    (finite [$($g:tt)*] $ty:ty, $t:ty) => {
        $crate::spec::animation_spec_impls!(any [$($g)*] $ty, $t);
        impl<$($g)*> $crate::FiniteAnimationSpec<$t> for $ty {
            fn vectorize_finite_1d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector1D>,
            ) -> Box<dyn $crate::VectorizedFiniteAnimationSpec<$crate::AnimationVector1D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_finite_2d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector2D>,
            ) -> Box<dyn $crate::VectorizedFiniteAnimationSpec<$crate::AnimationVector2D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_finite_3d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector3D>,
            ) -> Box<dyn $crate::VectorizedFiniteAnimationSpec<$crate::AnimationVector3D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_finite_4d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector4D>,
            ) -> Box<dyn $crate::VectorizedFiniteAnimationSpec<$crate::AnimationVector4D>> {
                Box::new(self.vectorize_into(c))
            }
        }
    };
    (duration_based [$($g:tt)*] $ty:ty, $t:ty) => {
        $crate::spec::animation_spec_impls!(finite [$($g)*] $ty, $t);
        impl<$($g)*> $crate::DurationBasedAnimationSpec<$t> for $ty {
            fn vectorize_duration_based_1d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector1D>,
            ) -> Box<dyn $crate::VectorizedDurationBasedAnimationSpec<$crate::AnimationVector1D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_duration_based_2d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector2D>,
            ) -> Box<dyn $crate::VectorizedDurationBasedAnimationSpec<$crate::AnimationVector2D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_duration_based_3d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector3D>,
            ) -> Box<dyn $crate::VectorizedDurationBasedAnimationSpec<$crate::AnimationVector3D>> {
                Box::new(self.vectorize_into(c))
            }
            fn vectorize_duration_based_4d(
                &self,
                c: &$crate::TwoWayConverter<$t, $crate::AnimationVector4D>,
            ) -> Box<dyn $crate::VectorizedDurationBasedAnimationSpec<$crate::AnimationVector4D>> {
                Box::new(self.vectorize_into(c))
            }
        }
    };
}
pub(crate) use animation_spec_impls;

/// The shared specs are specs too: `Arc<dyn …>` where a spec is taken.
macro_rules! shared_spec_impls {
    ($dyn:ident, any) => {
        impl<T: 'static> AnimationSpec<T> for Arc<dyn $dyn<T>> {
            fn vectorize_1d(
                &self,
                c: &TwoWayConverter<T, AnimationVector1D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector1D>> {
                (**self).vectorize_1d(c)
            }
            fn vectorize_2d(
                &self,
                c: &TwoWayConverter<T, AnimationVector2D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector2D>> {
                (**self).vectorize_2d(c)
            }
            fn vectorize_3d(
                &self,
                c: &TwoWayConverter<T, AnimationVector3D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector3D>> {
                (**self).vectorize_3d(c)
            }
            fn vectorize_4d(
                &self,
                c: &TwoWayConverter<T, AnimationVector4D>,
            ) -> Box<dyn VectorizedAnimationSpec<AnimationVector4D>> {
                (**self).vectorize_4d(c)
            }
            fn as_any(&self) -> &dyn Any {
                (**self).as_any()
            }
            fn equals(&self, other: &dyn Any) -> bool {
                (**self).equals(other)
            }
        }
    };
    ($dyn:ident, finite) => {
        shared_spec_impls!($dyn, any);
        impl<T: 'static> FiniteAnimationSpec<T> for Arc<dyn $dyn<T>> {
            fn vectorize_finite_1d(
                &self,
                c: &TwoWayConverter<T, AnimationVector1D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector1D>> {
                (**self).vectorize_finite_1d(c)
            }
            fn vectorize_finite_2d(
                &self,
                c: &TwoWayConverter<T, AnimationVector2D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector2D>> {
                (**self).vectorize_finite_2d(c)
            }
            fn vectorize_finite_3d(
                &self,
                c: &TwoWayConverter<T, AnimationVector3D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector3D>> {
                (**self).vectorize_finite_3d(c)
            }
            fn vectorize_finite_4d(
                &self,
                c: &TwoWayConverter<T, AnimationVector4D>,
            ) -> Box<dyn VectorizedFiniteAnimationSpec<AnimationVector4D>> {
                (**self).vectorize_finite_4d(c)
            }
        }
    };
}
shared_spec_impls!(AnimationSpec, any);
shared_spec_impls!(FiniteAnimationSpec, finite);
shared_spec_impls!(DurationBasedAnimationSpec, finite);

impl<T: 'static> DurationBasedAnimationSpec<T> for Arc<dyn DurationBasedAnimationSpec<T>> {
    fn vectorize_duration_based_1d(
        &self,
        c: &TwoWayConverter<T, AnimationVector1D>,
    ) -> Box<dyn VectorizedDurationBasedAnimationSpec<AnimationVector1D>> {
        (**self).vectorize_duration_based_1d(c)
    }
    fn vectorize_duration_based_2d(
        &self,
        c: &TwoWayConverter<T, AnimationVector2D>,
    ) -> Box<dyn VectorizedDurationBasedAnimationSpec<AnimationVector2D>> {
        (**self).vectorize_duration_based_2d(c)
    }
    fn vectorize_duration_based_3d(
        &self,
        c: &TwoWayConverter<T, AnimationVector3D>,
    ) -> Box<dyn VectorizedDurationBasedAnimationSpec<AnimationVector3D>> {
        (**self).vectorize_duration_based_3d(c)
    }
    fn vectorize_duration_based_4d(
        &self,
        c: &TwoWayConverter<T, AnimationVector4D>,
    ) -> Box<dyn VectorizedDurationBasedAnimationSpec<AnimationVector4D>> {
        (**self).vectorize_duration_based_4d(c)
    }
}

// ---- Tween ----------------------------------------------------------------------------------------------------

/// From the start to the target in `duration_millis` after `delay`, along `easing` (Compose's TweenSpec).
pub struct TweenSpec<T> {
    pub duration_millis: i32,
    pub delay: i32,
    pub easing: Arc<dyn Easing>,
    _t: PhantomData<fn() -> T>,
}

impl<T> TweenSpec<T> {
    pub fn new(duration_millis: i32, delay: i32, easing: impl Easing) -> TweenSpec<T> {
        TweenSpec { duration_millis, delay, easing: arc_easing(easing), _t: PhantomData }
    }

    fn vectorize_into<V: AnimationVector>(&self, _: &TwoWayConverter<T, V>) -> VectorizedTweenSpec<V> {
        VectorizedTweenSpec::new(self.duration_millis, self.delay, self.easing.clone())
    }
}

impl<T> Clone for TweenSpec<T> {
    fn clone(&self) -> Self {
        TweenSpec {
            duration_millis: self.duration_millis,
            delay: self.delay,
            easing: self.easing.clone(),
            _t: PhantomData,
        }
    }
}

impl<T> PartialEq for TweenSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.duration_millis == other.duration_millis
            && self.delay == other.delay
            && easing_eq(&self.easing, &other.easing)
    }
}

impl<T> std::fmt::Debug for TweenSpec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "TweenSpec(durationMillis={}, delay={})", self.duration_millis, self.delay)
    }
}

animation_spec_impls!(duration_based [T: 'static] TweenSpec<T>, T);

/// Compose's `tween(durationMillis = 300, delayMillis = 0, easing = FastOutSlowInEasing)`.
#[named]
pub fn tween<T>(
    #[default(AnimationConstants::DefaultDurationMillis)] duration_millis: i32,
    #[default(0)] delay_millis: i32,
    #[default(Arc::new(FastOutSlowInEasing))] easing: Arc<dyn Easing>,
) -> TweenSpec<T> {
    TweenSpec { duration_millis, delay: delay_millis, easing, _t: PhantomData }
}

// ---- Spring ---------------------------------------------------------------------------------------------------

/// A spring toward the target (Compose's SpringSpec): its damping ratio, stiffness, and how close to the target
/// counts as there (`None`: the type's converter's floats at 0.01).
pub struct SpringSpec<T> {
    pub damping_ratio: f32,
    pub stiffness: f32,
    pub visibility_threshold: Option<T>,
}

impl<T: Clone> SpringSpec<T> {
    pub fn new(damping_ratio: f32, stiffness: f32, visibility_threshold: Option<T>) -> SpringSpec<T> {
        SpringSpec { damping_ratio, stiffness, visibility_threshold }
    }

    fn vectorize_into<V: AnimationVector>(&self, converter: &TwoWayConverter<T, V>) -> VectorizedSpringSpec<V> {
        VectorizedSpringSpec::new(
            self.damping_ratio,
            self.stiffness,
            self.visibility_threshold.clone().map(|t| converter.convert_to_vector(t)),
        )
    }
}

impl<T> Default for SpringSpec<T> {
    /// Compose's defaults: no bounce, medium stiffness, no threshold of its own.
    fn default() -> Self {
        SpringSpec {
            damping_ratio: Spring::DampingRatioNoBouncy,
            stiffness: Spring::StiffnessMedium,
            visibility_threshold: None,
        }
    }
}

impl<T: Clone> Clone for SpringSpec<T> {
    fn clone(&self) -> Self {
        SpringSpec::new(self.damping_ratio, self.stiffness, self.visibility_threshold.clone())
    }
}

impl<T: PartialEq> PartialEq for SpringSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.damping_ratio == other.damping_ratio
            && self.stiffness == other.stiffness
            && self.visibility_threshold == other.visibility_threshold
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for SpringSpec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "SpringSpec(dampingRatio={}, stiffness={}, visibilityThreshold={:?})",
            self.damping_ratio, self.stiffness, self.visibility_threshold
        )
    }
}

animation_spec_impls!(finite [T: Clone + PartialEq + Send + Sync + 'static] SpringSpec<T>, T);

/// Compose's `spring(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessMedium,
/// visibilityThreshold = null)`.
#[named]
pub fn spring<T: Clone>(
    #[default(Spring::DampingRatioNoBouncy)] damping_ratio: f32,
    #[default(Spring::StiffnessMedium)] stiffness: f32,
    #[default(None)] visibility_threshold: Option<T>,
) -> SpringSpec<T> {
    SpringSpec::new(damping_ratio, stiffness, visibility_threshold)
}

// ---- Arc ------------------------------------------------------------------------------------------------------

/// From the start to the target along a quarter of an ellipse, for 2D values (Compose's experimental
/// ArcAnimationSpec): above or below the line (`ArcMode::ArcBelow` by default), in `duration_millis` after
/// `delay_millis`, along `easing`; other sizes animate their pairs of floats so (and the last alone in a line).
pub struct ArcAnimationSpec<T> {
    pub mode: ArcMode,
    pub duration_millis: i32,
    pub delay_millis: i32,
    pub easing: Arc<dyn Easing>,
    _t: PhantomData<fn() -> T>,
}

/// Compose's `ArcAnimationSpec(mode, durationMillis, delayMillis, easing)`.
/// Kotlin: `ArcAnimationSpec(mode: ArcMode = ArcMode.ArcBelow, durationMillis: Int = 300, delayMillis: Int = 0, easing: Easing = FastOutSlowInEasing)`
pub fn ArcAnimationSpec<T>(
    mode: ArcMode,
    duration_millis: i32,
    delay_millis: i32,
    easing: impl Easing,
) -> ArcAnimationSpec<T> {
    ArcAnimationSpec { mode, duration_millis, delay_millis, easing: arc_easing(easing), _t: PhantomData }
}

impl<T> ArcAnimationSpec<T> {
    fn vectorize_into<V: AnimationVector>(&self, _: &TwoWayConverter<T, V>) -> VectorizedKeyframesSpec<V> {
        VectorizedKeyframesSpec::internal(
            vec![0, self.duration_millis],
            HashMap::new(),
            self.duration_millis,
            self.delay_millis,
            self.easing.clone(),
            self.mode,
        )
    }
}

impl<T> PartialEq for ArcAnimationSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode
            && self.duration_millis == other.duration_millis
            && self.delay_millis == other.delay_millis
            && easing_eq(&self.easing, &other.easing)
    }
}

animation_spec_impls!(duration_based [T: 'static] ArcAnimationSpec<T>, T);

// ---- Start offsets and repeats ----------------------------------------------------------------------------------

/// How a repeat's start offset applies (Compose's StartOffsetType): a delay, or a fast-forward into it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct StartOffsetType(i32);

impl StartOffsetType {
    /// The animation starts that much later.
    pub const Delay: StartOffsetType = StartOffsetType(-1);
    /// The animation starts that far into it.
    pub const FastForward: StartOffsetType = StartOffsetType(1);
}

/// Where a repeated animation starts (Compose's StartOffset): delayed by, or fast-forwarded by, `offset_millis`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct StartOffset {
    pub(crate) value: i64,
}

/// Compose's `StartOffset(offsetMillis, offsetType = StartOffsetType.Delay)`.
#[named(type)]
pub fn StartOffset(offset_millis: i32, #[default(StartOffsetType::Delay)] offset_type: StartOffsetType) -> StartOffset {
    StartOffset::new(offset_millis, offset_type)
}

impl StartOffset {
    /// An offset of `offset_millis`, a delay or a fast-forward.
    pub const fn new(offset_millis: i32, offset_type: StartOffsetType) -> StartOffset {
        StartOffset { value: offset_millis as i64 * offset_type.0 as i64 }
    }

    /// How long the offset is.
    pub fn offset_millis(&self) -> i32 {
        (self.value as i32).wrapping_abs()
    }
    /// A delay or a fast-forward (a zero offset is a delay).
    pub fn offset_type(&self) -> StartOffsetType {
        if self.value > 0 { StartOffsetType::FastForward } else { StartOffsetType::Delay }
    }
}

/// Whether a repeat starts over or goes back (Compose's RepeatMode).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RepeatMode {
    /// Each time from the start.
    Restart,
    /// Back and forth.
    Reverse,
}

/// A duration-based spec `iterations` times (Compose's RepeatableSpec).
pub struct RepeatableSpec<T> {
    pub iterations: i32,
    pub animation: Arc<dyn DurationBasedAnimationSpec<T>>,
    pub repeat_mode: RepeatMode,
    pub initial_start_offset: StartOffset,
}

impl<T: 'static> RepeatableSpec<T> {
    pub fn new(
        iterations: i32,
        animation: impl DurationBasedAnimationSpec<T>,
        repeat_mode: RepeatMode,
        initial_start_offset: StartOffset,
    ) -> RepeatableSpec<T> {
        RepeatableSpec { iterations, animation: shared_duration_based(animation), repeat_mode, initial_start_offset }
    }

    #[track_caller]
    fn vectorize_into<V: AnimationVector>(&self, converter: &TwoWayConverter<T, V>) -> VectorizedRepeatableSpec<V> {
        VectorizedRepeatableSpec::new(
            self.iterations,
            V::vectorize_duration_based(&*self.animation, converter),
            self.repeat_mode,
            self.initial_start_offset,
        )
    }
}

/// The spec shared, an `Arc<dyn DurationBasedAnimationSpec<T>>` as it is if it is one.
fn shared_duration_based<T: 'static>(
    spec: impl DurationBasedAnimationSpec<T>,
) -> Arc<dyn DurationBasedAnimationSpec<T>> {
    let any: &dyn Any = &spec;
    if let Some(shared) = any.downcast_ref::<Arc<dyn DurationBasedAnimationSpec<T>>>() {
        return shared.clone();
    }
    Arc::new(spec)
}

impl<T: 'static> PartialEq for RepeatableSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.iterations == other.iterations
            && *self.animation == *other.animation
            && self.repeat_mode == other.repeat_mode
            && self.initial_start_offset == other.initial_start_offset
    }
}

animation_spec_impls!(finite [T: 'static] RepeatableSpec<T>, T);

/// Compose's `repeatable(iterations, animation, repeatMode = RepeatMode.Restart, initialStartOffset =
/// StartOffset(0))`: fewer than one iteration panics where it is used, as Compose's throws then.
#[named]
pub fn repeatable<T: 'static>(
    iterations: i32,
    animation: impl DurationBasedAnimationSpec<T>,
    #[default(RepeatMode::Restart)] repeat_mode: RepeatMode,
    #[default(StartOffset::default())] initial_start_offset: StartOffset,
) -> RepeatableSpec<T> {
    RepeatableSpec::new(iterations, animation, repeat_mode, initial_start_offset)
}

/// A duration-based spec forever (Compose's InfiniteRepeatableSpec): one of no duration panics, as Compose's
/// throws.
pub struct InfiniteRepeatableSpec<T> {
    pub animation: Arc<dyn DurationBasedAnimationSpec<T>>,
    pub repeat_mode: RepeatMode,
    pub initial_start_offset: StartOffset,
}

impl<T: 'static> InfiniteRepeatableSpec<T> {
    #[track_caller]
    pub fn new(
        animation: impl DurationBasedAnimationSpec<T>,
        repeat_mode: RepeatMode,
        initial_start_offset: StartOffset,
    ) -> InfiniteRepeatableSpec<T> {
        let animation = shared_duration_based(animation);
        let any = animation.as_any();
        let is_zero_duration = if let Some(a) = any.downcast_ref::<TweenSpec<T>>() {
            a.duration_millis == 0 && a.delay == 0
        } else if let Some(a) = any.downcast_ref::<SnapSpec<T>>() {
            a.delay == 0
        } else if let Some(a) = any.downcast_ref::<KeyframesSpec<T>>() {
            a.config.duration_millis == 0 && a.config.delay_millis == 0
        } else if let Some(a) = any.downcast_ref::<KeyframesWithSplineSpec<T>>() {
            a.config.duration_millis == 0 && a.config.delay_millis == 0
        } else if let Some(a) = any.downcast_ref::<ArcAnimationSpec<T>>() {
            a.duration_millis == 0 && a.delay_millis == 0
        } else {
            false
        };
        if is_zero_duration {
            panic_kt("IllegalArgumentException", "Animation to be infinitely repeated cannot have a 0-duration");
        }
        InfiniteRepeatableSpec { animation, repeat_mode, initial_start_offset }
    }

    fn vectorize_into<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<T, V>,
    ) -> VectorizedInfiniteRepeatableSpec<V> {
        VectorizedInfiniteRepeatableSpec::new(
            V::vectorize_duration_based(&*self.animation, converter),
            self.repeat_mode,
            self.initial_start_offset,
        )
    }
}

impl<T: 'static> PartialEq for InfiniteRepeatableSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        *self.animation == *other.animation
            && self.repeat_mode == other.repeat_mode
            && self.initial_start_offset == other.initial_start_offset
    }
}

animation_spec_impls!(any [T: 'static] InfiniteRepeatableSpec<T>, T);

/// Compose's `infiniteRepeatable(animation, repeatMode = RepeatMode.Restart, initialStartOffset = StartOffset(0))`.
#[named]
pub fn infinite_repeatable<T: 'static>(
    animation: impl DurationBasedAnimationSpec<T>,
    #[default(RepeatMode::Restart)] repeat_mode: RepeatMode,
    #[default(StartOffset::default())] initial_start_offset: StartOffset,
) -> InfiniteRepeatableSpec<T> {
    InfiniteRepeatableSpec::new(animation, repeat_mode, initial_start_offset)
}

// ---- Snap -----------------------------------------------------------------------------------------------------

/// The target at once, after `delay` ms (Compose's SnapSpec).
pub struct SnapSpec<T> {
    pub delay: i32,
    _t: PhantomData<fn() -> T>,
}

impl<T> SnapSpec<T> {
    pub fn new(delay: i32) -> SnapSpec<T> {
        SnapSpec { delay, _t: PhantomData }
    }
    fn vectorize_into<V: AnimationVector>(&self, _: &TwoWayConverter<T, V>) -> VectorizedSnapSpec<V> {
        VectorizedSnapSpec::new(self.delay)
    }
}

impl<T> Clone for SnapSpec<T> {
    fn clone(&self) -> Self {
        SnapSpec::new(self.delay)
    }
}

impl<T> PartialEq for SnapSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.delay == other.delay
    }
}

animation_spec_impls!(duration_based [T: 'static] SnapSpec<T>, T);

/// Compose's `snap(delayMillis = 0)`.
#[named]
pub fn snap<T>(#[default(0)] delay_millis: i32) -> SnapSpec<T> {
    SnapSpec::new(delay_millis)
}

// ---- Keyframes --------------------------------------------------------------------------------------------------

/// A value at a time in a keyframes block (Compose's KeyframesSpec.KeyframeEntity): the easing of the stretch
/// that starts there (`using`), and for keyframes with arcs, how that stretch arcs.
#[derive(Clone)]
pub struct KeyframeEntity<T> {
    pub(crate) value: T,
    pub(crate) easing: Arc<dyn Easing>,
    pub(crate) arc_mode: ArcMode,
}

impl<T> KeyframeEntity<T> {
    fn new(value: T) -> KeyframeEntity<T> {
        KeyframeEntity { value, easing: Arc::new(LinearEasing), arc_mode: ArcMode::ArcLinear }
    }

    /// The easing from this keyframe to the next (Compose's infix `using`): `config.at(0.4, 75).using(FastOutSlowInEasing)`.
    pub fn using(&mut self, easing: impl Easing) -> &mut KeyframeEntity<T> {
        self.easing = arc_easing(easing);
        self
    }

    /// How the stretch from this keyframe to the next arcs, for 2D values (Compose's infix `using(arcMode)`).
    /// Kotlin: `using(arcMode: ArcMode)`
    pub fn using_arc_mode(&mut self, arc_mode: ArcMode) -> &mut KeyframeEntity<T> {
        self.arc_mode = arc_mode;
        self
    }

    /// The easing from this keyframe (Compose's deprecated infix `with`, `using` now).
    pub fn with(&mut self, easing: impl Easing) {
        self.easing = arc_easing(easing);
    }
}

impl<T: PartialEq> PartialEq for KeyframeEntity<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && easing_eq(&self.easing, &other.easing) && self.arc_mode == other.arc_mode
    }
}

/// What a `keyframes { … }` block sets (Compose's KeyframesSpec.KeyframesSpecConfig): the duration (300 ms by
/// default), a delay, and the keyframes.
#[derive(Clone, KotlinProperties)]
pub struct KeyframesSpecConfig<T> {
    #[kotlin(set)]
    pub duration_millis: i32,
    #[kotlin(set)]
    pub delay_millis: i32,
    pub(crate) keyframes: HashMap<i32, KeyframeEntity<T>>,
}
impl<T> KeyframesSpecConfig<T> {
    pub fn new() -> KeyframesSpecConfig<T> {
        KeyframesSpecConfig {
            duration_millis: AnimationConstants::DefaultDurationMillis,
            delay_millis: 0,
            keyframes: HashMap::new(),
        }
    }

    /// `value` at `time_stamp` ms (Compose's infix `at`: `0.4f at 75` in a `keyframes { }` block); the keyframe, to give
    /// it an easing.
    pub fn at(&mut self, value: T, time_stamp: i32) -> &mut KeyframeEntity<T> {
        self.keyframes.insert(time_stamp, KeyframeEntity::new(value));
        self.keyframes.get_mut(&time_stamp).unwrap()
    }

    /// `value` at `fraction` of the duration (Compose's infix `atFraction`), rounded to a millisecond.
    pub fn at_fraction(&mut self, value: T, fraction: f32) -> &mut KeyframeEntity<T> {
        let time_stamp = crate::converter::fast_round_to_int(self.duration_millis as f32 * fraction);
        self.at(value, time_stamp)
    }
}

impl<T> Default for KeyframesSpecConfig<T> {
    fn default() -> Self {
        KeyframesSpecConfig::new()
    }
}

/// What a `keyframesWithSpline { … }` block sets (Compose's KeyframesWithSplineSpec.KeyframesWithSplineSpecConfig).
#[derive(Clone, KotlinProperties)]
pub struct KeyframesWithSplineSpecConfig<T> {
    #[kotlin(set)]
    pub duration_millis: i32,
    #[kotlin(set)]
    pub delay_millis: i32,
    pub(crate) keyframes: HashMap<i32, KeyframeEntity<T>>,
}
impl<T> KeyframesWithSplineSpecConfig<T> {
    pub fn new() -> KeyframesWithSplineSpecConfig<T> {
        KeyframesWithSplineSpecConfig {
            duration_millis: AnimationConstants::DefaultDurationMillis,
            delay_millis: 0,
            keyframes: HashMap::new(),
        }
    }

    /// `value` at `time_stamp` ms (Compose's infix `at`: `0.4f at 75` in a `keyframesWithSpline { }` block); the keyframe, to give
    /// it an easing.
    pub fn at(&mut self, value: T, time_stamp: i32) -> &mut KeyframeEntity<T> {
        self.keyframes.insert(time_stamp, KeyframeEntity::new(value));
        self.keyframes.get_mut(&time_stamp).unwrap()
    }

    /// `value` at `fraction` of the duration (Compose's infix `atFraction`), rounded to a millisecond.
    pub fn at_fraction(&mut self, value: T, fraction: f32) -> &mut KeyframeEntity<T> {
        let time_stamp = crate::converter::fast_round_to_int(self.duration_millis as f32 * fraction);
        self.at(value, time_stamp)
    }
}

impl<T> Default for KeyframesWithSplineSpecConfig<T> {
    fn default() -> Self {
        KeyframesWithSplineSpecConfig::new()
    }
}

/// Values at given times, eased between, the start and the target at 0 and the duration unless keyframes are
/// there (Compose's KeyframesSpec). Equal only to itself.
pub struct KeyframesSpec<T> {
    pub config: KeyframesSpecConfig<T>,
}

impl<T: Clone> KeyframesSpec<T> {
    pub fn new(config: KeyframesSpecConfig<T>) -> KeyframesSpec<T> {
        KeyframesSpec { config }
    }

    fn vectorize_into<V: AnimationVector>(&self, converter: &TwoWayConverter<T, V>) -> VectorizedKeyframesSpec<V> {
        let keyframes = &self.config.keyframes;
        let timestamps =
            keyframe_timestamps(keyframes.keys().copied(), |t| keyframes.contains_key(&t), self.config.duration_millis);
        let infos = keyframes
            .iter()
            .map(|(t, k)| {
                (
                    *t,
                    VectorizedKeyframeSpecElementInfo {
                        vector_value: converter.convert_to_vector(k.value.clone()),
                        easing: k.easing.clone(),
                        arc_mode: k.arc_mode,
                    },
                )
            })
            .collect();
        VectorizedKeyframesSpec::internal(
            timestamps,
            infos,
            self.config.duration_millis,
            self.config.delay_millis,
            Arc::new(LinearEasing),
            ArcMode::ArcLinear,
        )
    }
}

impl<T> PartialEq for KeyframesSpec<T> {
    /// (Compose's has no `equals`: the same spec only.)
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

animation_spec_impls!(duration_based [T: Clone + Send + Sync + 'static] KeyframesSpec<T>, T);

/// Compose's `keyframes { durationMillis = 375; 0.4f at 75 using FastOutSlowInEasing }`:
/// `keyframes(|k| { k.duration_millis = 375; k.at(0.4, 75).using(FastOutSlowInEasing); })`.
pub fn keyframes<T: Clone>(init: impl FnOnce(&mut KeyframesSpecConfig<T>)) -> KeyframesSpec<T> {
    let mut config = KeyframesSpecConfig::new();
    init(&mut config);
    KeyframesSpec::new(config)
}

/// Keyframes joined by a monotonic spline (Compose's KeyframesWithSplineSpec), smooth through all of them; with
/// a periodic bias (0..1), its ends' slopes made one for a seamless repeat. Equal only to itself.
pub struct KeyframesWithSplineSpec<T> {
    pub config: KeyframesWithSplineSpecConfig<T>,
    periodic_bias: f32,
}

impl<T: Clone> KeyframesWithSplineSpec<T> {
    pub fn new(config: KeyframesWithSplineSpecConfig<T>) -> KeyframesWithSplineSpec<T> {
        KeyframesWithSplineSpec { config, periodic_bias: f32::NAN }
    }

    /// With a periodic bias.
    pub fn new_periodic(config: KeyframesWithSplineSpecConfig<T>, periodic_bias: f32) -> KeyframesWithSplineSpec<T> {
        KeyframesWithSplineSpec { config, periodic_bias }
    }

    fn vectorize_into<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<T, V>,
    ) -> VectorizedMonoSplineKeyframesSpec<V> {
        let keyframes = &self.config.keyframes;
        let timestamps =
            keyframe_timestamps(keyframes.keys().copied(), |t| keyframes.contains_key(&t), self.config.duration_millis);
        let map = keyframes
            .iter()
            .map(|(t, k)| (*t, (converter.convert_to_vector(k.value.clone()), k.easing.clone())))
            .collect();
        VectorizedMonoSplineKeyframesSpec::new(
            timestamps,
            map,
            self.config.duration_millis,
            self.config.delay_millis,
            self.periodic_bias,
        )
    }
}

impl<T> PartialEq for KeyframesWithSplineSpec<T> {
    /// (Compose's has no `equals`: the same spec only.)
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

animation_spec_impls!(duration_based [T: Clone + Send + Sync + 'static] KeyframesWithSplineSpec<T>, T);

/// Compose's `keyframesWithSpline { … }`.
pub fn keyframes_with_spline<T: Clone>(
    init: impl FnOnce(&mut KeyframesWithSplineSpecConfig<T>),
) -> KeyframesWithSplineSpec<T> {
    let mut config = KeyframesWithSplineSpecConfig::new();
    init(&mut config);
    KeyframesWithSplineSpec::new(config)
}

/// Compose's `keyframesWithSpline(periodicBias) { … }`: the ends' slopes made one, weighted between the last
/// stretch's (0) and the first's (1).
/// Kotlin: `keyframesWithSpline(periodicBias: Float, init: KeyframesWithSplineSpecConfig<T>.() -> Unit)`
pub fn keyframes_with_spline_periodic<T: Clone>(
    periodic_bias: f32,
    init: impl FnOnce(&mut KeyframesWithSplineSpecConfig<T>),
) -> KeyframesWithSplineSpec<T> {
    let mut config = KeyframesWithSplineSpecConfig::new();
    init(&mut config);
    KeyframesWithSplineSpec::new_periodic(config, periodic_bias)
}

// ---- A start delay ----------------------------------------------------------------------------------------------

/// `animation_spec` started `start_delay_nanos` late: the start value and velocity until then (Compose's internal
/// `delayed`, what Transition delays an animation with).
pub fn delayed<T: 'static>(
    animation_spec: Arc<dyn AnimationSpec<T>>,
    start_delay_nanos: i64,
) -> StartDelayAnimationSpec<T> {
    StartDelayAnimationSpec { animation_spec, start_delay_nanos }
}

/// A spec started late (Compose's internal StartDelayAnimationSpec).
pub struct StartDelayAnimationSpec<T> {
    pub animation_spec: Arc<dyn AnimationSpec<T>>,
    pub start_delay_nanos: i64,
}

impl<T: 'static> StartDelayAnimationSpec<T> {
    fn vectorize_into<V: AnimationVector>(
        &self,
        converter: &TwoWayConverter<T, V>,
    ) -> StartDelayVectorizedAnimationSpec<V> {
        StartDelayVectorizedAnimationSpec {
            vectorized_animation_spec: V::vectorize(&*self.animation_spec, converter),
            start_delay_nanos: self.start_delay_nanos,
        }
    }
}

impl<T: 'static> PartialEq for StartDelayAnimationSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.start_delay_nanos == other.start_delay_nanos && *self.animation_spec == *other.animation_spec
    }
}

animation_spec_impls!(any [T: 'static] StartDelayAnimationSpec<T>, T);

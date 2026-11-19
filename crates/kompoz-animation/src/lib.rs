//! KompozR's animation value layer, as Compose's `androidx.compose.animation.core` (and `animation`'s decays and
//! color converter): values that animate ([`Animatable`], [`AnimationState`], [`animate`]) frame by frame on the
//! host's frame clock in a coroutine, along specs ([`spring`], [`tween`], [`snap`], [`keyframes`],
//! [`keyframes_with_spline`], [`repeatable`], [`infinite_repeatable`]) with [`Easing`]s, on vectors of one to four
//! floats a [`TwoWayConverter`] makes of a value; flings slowing down along a decay ([`exponential_decay`], Android's
//! [`spline_based_decay`]); one mutation at a time ([`MutatorMutex`]).
//!
//! ```no_run
//! # use kompoz_animation::*;
//! # use kompoz_coroutines::*;
//! # use kompoz_runtime::*;
//! #[kompozable]
//! fn Pulse() {
//!     let scale = remember(|| Animatable(1.0));
//!     let s = scale.clone();
//!     LaunchedEffect((), async move {
//!         s.animate_to(1.2).await;
//!     });
//! }
//! ```
//!
//! Time is the frame clock's (`with_frame_nanos`); a [`MotionDurationScale`](kompoz_ui::MotionDurationScale) in the
//! coroutine's context scales durations, an [`InfiniteAnimationPolicy`](kompoz_ui::InfiniteAnimationPolicy) decides
//! what infinite ones do. Vectors are values here (Compose reuses its vector objects); where that is observable
//! (a keyframes spec's velocity), Compose's numbers are kept.

#![allow(non_upper_case_globals, non_snake_case)]

extern crate self as kompoz_animation;

mod animatable;
mod animation;
mod arc_spline;
mod color;
mod converter;
mod decay;
mod deferred_target;
mod easing;
mod float_spec;
mod mono_spline;
mod mono_spline_keyframes;
mod mutator;
mod spec;
mod spline_decay;
mod spring_simulation;
mod state;
mod suspend;
mod vector;
mod vectorized;

pub use animatable::{Animatable, AnimatableArgs, AnimationEndReason, AnimationResult};
pub use animation::{Animation, DecayAnimation, TargetBasedAnimation, create_animation, float_decay_animation};
pub use color::ColorVectorConverter;
pub use converter::{TwoWayConverter, VectorConverter, VisibilityThreshold, visibility_threshold_of};
pub use decay::{
    DecayAnimationSpec, FloatDecayAnimationSpec, FloatDecaySpecOf, FloatDecayTarget, FloatExponentialDecaySpec,
    VectorizedDecayAnimationSpec, exponential_decay, generate_decay_animation_spec,
};
pub use deferred_target::DeferredTargetAnimation;
pub use easing::{
    CubicBezierEasing, Ease, EaseIn, EaseInBack, EaseInBounce, EaseInCirc, EaseInCubic, EaseInElastic, EaseInExpo,
    EaseInOut, EaseInOutBack, EaseInOutBounce, EaseInOutCirc, EaseInOutCubic, EaseInOutElastic, EaseInOutExpo,
    EaseInOutQuad, EaseInOutQuart, EaseInOutQuint, EaseInOutSine, EaseInQuad, EaseInQuart, EaseInQuint, EaseInSine,
    EaseOut, EaseOutBack, EaseOutBounce, EaseOutCirc, EaseOutCubic, EaseOutElastic, EaseOutExpo, EaseOutQuad,
    EaseOutQuart, EaseOutQuint, EaseOutSine, Easing, FastOutLinearInEasing, FastOutSlowInEasing, LinearEasing,
    LinearOutSlowInEasing, PathEasing, arc_easing,
};
pub use float_spec::{FloatAnimationSpec, FloatSpringSpec, FloatTweenSpec};
pub use mutator::{MutatePriority, MutatorMutex};
pub use spec::{
    AnimationConstants, AnimationSpec, ArcAnimationSpec, DurationBasedAnimationSpec, FiniteAnimationSpec,
    InfiniteRepeatableSpec, KeyframeEntity, KeyframesSpec, KeyframesSpecConfig, KeyframesWithSplineSpec,
    KeyframesWithSplineSpecConfig, RepeatMode, RepeatableSpec, SnapSpec, SpringSpec, StartDelayAnimationSpec,
    StartOffset, StartOffsetType, TweenSpec, delayed, infinite_repeatable, keyframes, keyframes_with_spline,
    keyframes_with_spline_periodic, repeatable, snap, spring, tween,
};
pub use spline_decay::{
    AndroidFlingSpline, FlingCalculator, FlingInfo, FlingResult, PlatformFlingScrollFriction,
    SplineBasedFloatDecayAnimationSpec, default_decay_animation_spec, remember_spline_based_decay, spline_based_decay,
};
pub use spring_simulation::{
    Spring, estimate_animation_duration_millis, estimate_animation_duration_millis_f64,
    estimate_animation_duration_millis_of_mass,
};
pub use state::{AnimationScope, AnimationState, AnimationValue, float_animation_state};
pub use suspend::{
    animate, animate_decay, animate_value, with_infinite_animation_frame_millis, with_infinite_animation_frame_nanos,
};
pub use vector::{
    AnimationVector, AnimationVector1D, AnimationVector2D, AnimationVector3D, AnimationVector4D, AnimationVectorArgs,
};
pub use vectorized::{
    ArcMode, VectorizedAnimationSpec, VectorizedDurationBasedAnimationSpec, VectorizedFiniteAnimationSpec,
    VectorizedFloatAnimationSpec, VectorizedInfiniteRepeatableSpec, VectorizedKeyframesSpec, VectorizedRepeatableSpec,
    VectorizedSnapSpec, VectorizedSpringSpec, VectorizedTweenSpec,
};

/// Nanoseconds in a millisecond (Compose's internal MillisToNanos).
pub(crate) const MillisToNanos: i64 = 1_000_000;
/// Milliseconds in a second (Compose's internal SecondsToMillis).
pub(crate) const SecondsToMillis: i64 = 1_000;

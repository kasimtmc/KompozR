//! KompozR's animation value layer, as Compose's `androidx.compose.animation.core` (and `animation`'s decays and
//! color converter): values that animate ([`Animatable`], [`AnimationState`], [`animate`]) frame by frame on the
//! host's frame clock in a coroutine, along specs ([`spring`], [`tween`], [`snap`], [`keyframes`],
//! [`keyframes_with_spline`], [`repeatable`], [`infinite_repeatable`]) with [`Easing`]s, on vectors of one to four
//! floats a [`TwoWayConverter`] makes of a value; flings slowing down along a decay ([`exponential_decay`], Android's
//! [`spline_based_decay`]); one mutation at a time ([`MutatorMutex`]). On it, what composables animate with: values
//! animated as they change ([`animate_float_as_state`], … [`animate_color_as_state`]), transitions of a state
//! ([`update_transition`], [`remember_transition`] of a [`MutableTransitionState`], [`SeekableTransitionState`] or
//! [`DeferredTransitionState`]; a [`Transition`]'s `animate_float`, … `animate_color`, child transitions), and
//! animations that never end ([`remember_infinite_transition`]).
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
mod animate_as_state;
mod animated_content;
mod animated_visibility;
mod animation;
mod arc_spline;
mod bounds;
mod color;
mod content_size;
mod converter;
mod crossfade;
mod decay;
mod deferred_enter_exit;
mod deferred_target;
mod easing;
mod enter_exit;
mod float_spec;
mod infinite_transition;
mod lookahead_debug;
mod mono_spline;
mod mono_spline_keyframes;
mod mutator;
mod seekable;
mod skip_to_lookahead_size;
mod spec;
mod spline_decay;
mod spring_simulation;
mod state;
mod suspend;
#[cfg(test)]
mod tests;
mod transition;
mod vector;
mod vectorized;

pub use animatable::{Animatable, AnimatableArgs, AnimationEndReason, AnimationResult};
pub use animate_as_state::{
    animate_color_as_state, animate_dp_as_state, animate_float_as_state, animate_int_as_state,
    animate_int_offset_as_state, animate_int_size_as_state, animate_offset_as_state, animate_rect_as_state,
    animate_size_as_state, animate_value_as_state,
};
pub use animated_content::{
    AnimatedContent, AnimatedContentScope, AnimatedContentTransitionScope, ContentKey, ContentTransform, SizeTransform,
    SlideDirection, default_content_transform, default_size_animation_spec,
};
pub use animated_visibility::{
    AnimatedVisibility, AnimatedVisibilityScope, AnimatedVisibilityWithState, ColumnAnimatedVisibility,
    ColumnAnimatedVisibilityWithState, RowAnimatedVisibility, RowAnimatedVisibilityWithState,
};
pub use animation::{Animation, DecayAnimation, TargetBasedAnimation, create_animation, float_decay_animation};
pub use bounds::{BoundsModifiers, BoundsTransform};
pub use color::ColorVectorConverter;
pub use content_size::{ContentSizeModifiers, SizeAnimationFinishedListener};
pub use converter::{TwoWayConverter, VectorConverter, VisibilityThreshold, visibility_threshold_of};
pub use crossfade::Crossfade;
pub use decay::{
    DecayAnimationSpec, FloatDecayAnimationSpec, FloatDecaySpecOf, FloatDecayTarget, FloatExponentialDecaySpec,
    VectorizedDecayAnimationSpec, exponential_decay, generate_decay_animation_spec,
};
pub use deferred_enter_exit::{
    MutableContentTransform, MutableTransform, MutableTransformBlock, MutableTransformScope,
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
pub use enter_exit::{
    EnterExitState, EnterTransition, ExitTransition, expand_horizontally, expand_in, expand_vertically, fade_in,
    fade_out, scale_in, scale_out, shrink_horizontally, shrink_out, shrink_vertically, slide_in, slide_in_horizontally,
    slide_in_vertically, slide_out, slide_out_horizontally, slide_out_vertically, unveil_in, veil_out,
};
pub use float_spec::{FloatAnimationSpec, FloatSpringSpec, FloatTweenSpec};
pub use infinite_transition::{
    AnyInfiniteTransitionAnimationState, InfiniteTransition, InfiniteTransitionAnimationState,
    remember_infinite_transition,
};
pub use lookahead_debug::{CustomizedLookaheadAnimationVisualDebugging, LookaheadAnimationVisualDebugging};
pub use mutator::{MutatePriority, MutatorMutex};
pub use seekable::{SeekableTransitionState, SeekingAnimationState};
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
pub use transition::{
    AnyTransition, AnyTransitionAnimationState, DeferredAnimation, DeferredTransition, DeferredTransitionState,
    MutableTransitionState, Segment, Transition, TransitionAnimationState, TransitionState,
    remember_deferred_transition, remember_transition, update_transition, update_transition_of_state,
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

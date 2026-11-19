//! Animations run in a coroutine, frame by frame on the frame clock (Compose's SuspendAnimation.kt,
//! InfiniteAnimationPolicy.kt): `animate`, `animate_decay`, `AnimationState::animate_to` and `animate_decay`.

use crate::animation::{Animation, DecayAnimation, TargetBasedAnimation};
use crate::converter::{TwoWayConverter, VectorConverter};
use crate::decay::{DecayAnimationSpec, FloatDecayAnimationSpec, generate_decay_animation_spec};
use crate::spec::{AnimationConstants, AnimationSpec, spring};
use crate::state::{AnimationScope, AnimationState, AnimationValue};
use crate::vector::{AnimationVector, AnimationVector1D};
use kompoz_coroutines::{current_coroutine_context, with_frame_nanos};
use kompoz_runtime::kt::panic_kt;
use kompoz_runtime::named;
use kompoz_ui::{InfiniteAnimationPolicy, MotionDurationScale};
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;
use std::task::Poll;

/// Compose's AnimationDebugDurationScale: frame times are divided by it (1).
const AnimationDebugDurationScale: i64 = 1;

/// A frame of an infinite animation (Compose's withInfiniteAnimationFrameNanos): through the coroutine context's
/// [`InfiniteAnimationPolicy`] if it has one (a test's cancels infinite animations while its clock advances on its
/// own), else a plain frame.
pub async fn with_infinite_animation_frame_nanos<R>(on_frame: impl FnOnce(i64) -> R) -> R {
    match current_coroutine_context().get::<InfiniteAnimationPolicy>() {
        None => with_frame_nanos(on_frame).await,
        Some(policy) => {
            // (the frame awaited through the policy; the callback run in the frame, as soon as the frame's time is
            // in: Compose's `policy.onInfiniteOperation { withFrameNanos(onFrame) }`)
            let frame_time: Rc<Cell<Option<i64>>> = Rc::default();
            let in_frame = frame_time.clone();
            let mut operation = std::pin::pin!(policy.on_infinite_operation(with_frame_nanos(move |time| {
                in_frame.set(Some(time));
                time
            })));
            let mut on_frame = Some(on_frame);
            let mut result = None;
            std::future::poll_fn(move |cx| {
                let polled = operation.as_mut().poll(cx);
                if let Some(time) = frame_time.take() {
                    result = on_frame.take().map(|f| f(time));
                }
                match polled {
                    Poll::Ready(_) => Poll::Ready(result.take().expect("the frame's callback ran with its time")),
                    Poll::Pending => Poll::Pending,
                }
            })
            .await
        }
    }
}

/// The same in milliseconds (Compose's withInfiniteAnimationFrameMillis).
pub async fn with_infinite_animation_frame_millis<R>(on_frame: impl FnOnce(i64) -> R) -> R {
    with_infinite_animation_frame_nanos(|nanos| on_frame(nanos / 1_000_000)).await
}

/// The coroutine context's motion duration scale (Compose's internal `CoroutineContext.durationScale`): 1 without
/// a [`MotionDurationScale`]; a negative one panics, as Compose's throws.
#[track_caller]
pub(crate) fn duration_scale() -> f32 {
    let scale = current_coroutine_context().get::<MotionDurationScale>().map(|s| s.scale_factor()).unwrap_or(1.0);
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    if !(scale >= 0.0) {
        panic_kt("IllegalStateException", "negative scale factor");
    }
    scale
}

/// A frame for `animation` (Compose's callWithFrameNanos): an infinite one's through the policy.
async fn call_with_frame_nanos<T, V: AnimationVector, R>(
    animation: &impl Animation<T, V>,
    on_frame: impl FnOnce(i64) -> R,
) -> R {
    if animation.is_infinite() {
        with_infinite_animation_frame_nanos(on_frame).await
    } else {
        with_frame_nanos(|t| on_frame(t / AnimationDebugDurationScale)).await
    }
}

/// Runs `animation` on `state`, one frame each frame (Compose's internal `AnimationState.animate`): from
/// `start_time_nanos` (`UnspecifiedTime`: the next frame), `block` called each frame with the scope; until it
/// finishes or the block cancels it. Cancelled (the coroutine), the state stops running unless another animation
/// took it over.
pub(crate) async fn animate_state<T: AnimationValue, V: AnimationVector>(
    state: &AnimationState<T, V>,
    animation: &impl Animation<T, V>,
    start_time_nanos: i64,
    mut block: impl FnMut(&AnimationScope<T, V>),
) {
    let initial_value = animation.get_value_from_nanos(0);
    let initial_velocity_vector = animation.get_velocity_vector_from_nanos(0);
    let late_init_scope: Rc<RefCell<Option<Rc<AnimationScope<T, V>>>>> = Rc::new(RefCell::new(None));

    /// What Compose's `catch (e: CancellationException)` does, run when the animation is dropped before its end.
    struct OnCancel<T: AnimationValue, V: AnimationVector> {
        scope: Rc<RefCell<Option<Rc<AnimationScope<T, V>>>>>,
        state: AnimationState<T, V>,
        armed: bool,
    }
    impl<T: AnimationValue, V: AnimationVector> Drop for OnCancel<T, V> {
        fn drop(&mut self) {
            // (a failure isn't a cancellation: Compose catches only the latter)
            if !self.armed || std::thread::panicking() {
                return;
            }
            let scope = self.scope.borrow().clone();
            if let Some(scope) = &scope {
                scope.set_is_running(false);
            }
            if scope.as_ref().map(|s| s.last_frame_time_nanos()) == Some(self.state.last_frame_time_nanos()) {
                // (no other animation since)
                self.state.set_is_running(false);
            }
        }
    }
    let mut guard = OnCancel { scope: late_init_scope.clone(), state: state.clone(), armed: true };

    let new_scope = |frame_time: i64| {
        let on_cancel_state = state.clone();
        Rc::new(AnimationScope::new(
            initial_value.clone(),
            animation.type_converter().clone(),
            initial_velocity_vector,
            frame_time,
            animation.target_value(),
            frame_time,
            true,
            move || on_cancel_state.set_is_running(false),
        ))
    };
    if start_time_nanos == AnimationConstants::UnspecifiedTime {
        let scale = duration_scale();
        let scope = call_with_frame_nanos(animation, |t| {
            let scope = new_scope(t);
            *late_init_scope.borrow_mut() = Some(scope.clone());
            do_animation_frame_with_scale(&scope, t, scale, animation, state, &mut block);
            scope
        })
        .await;
        drop(scope);
    } else {
        let scope = new_scope(start_time_nanos);
        *late_init_scope.borrow_mut() = Some(scope.clone());
        do_animation_frame_with_scale(&scope, start_time_nanos, duration_scale(), animation, state, &mut block);
    }
    // Later frames.
    loop {
        let scope = late_init_scope.borrow().clone().expect("the animation's scope");
        if !scope.peek_is_running() {
            break;
        }
        let scale = duration_scale();
        call_with_frame_nanos(animation, |t| {
            do_animation_frame_with_scale(&scope, t, scale, animation, state, &mut block)
        })
        .await;
    }
    guard.armed = false;
}

/// A frame at `frame_time_nanos`, its play time scaled (Compose's doAnimationFrameWithScale: a scale of 0 ends it).
fn do_animation_frame_with_scale<T: AnimationValue, V: AnimationVector>(
    scope: &AnimationScope<T, V>,
    frame_time_nanos: i64,
    duration_scale: f32,
    animation: &impl Animation<T, V>,
    state: &AnimationState<T, V>,
    block: &mut impl FnMut(&AnimationScope<T, V>),
) {
    let play_time_nanos = if duration_scale == 0.0 {
        animation.duration_nanos()
    } else {
        // (Kotlin's Long / Float: in floats; then toLong)
        ((frame_time_nanos - scope.start_time_nanos()) as f32 / duration_scale) as i64
    };
    do_animation_frame(scope, frame_time_nanos, play_time_nanos, animation, state, block);
}

/// A frame (Compose's doAnimationFrame): the scope's value and velocity at the play time, finished there if the
/// animation is, copied into the state, then the block.
fn do_animation_frame<T: AnimationValue, V: AnimationVector>(
    scope: &AnimationScope<T, V>,
    frame_time_nanos: i64,
    play_time_nanos: i64,
    animation: &impl Animation<T, V>,
    state: &AnimationState<T, V>,
    block: &mut impl FnMut(&AnimationScope<T, V>),
) {
    scope.set_last_frame_time_nanos(frame_time_nanos);
    scope.set_value(animation.get_value_from_nanos(play_time_nanos));
    scope.set_velocity_vector(animation.get_velocity_vector_from_nanos(play_time_nanos));
    if animation.is_finished_from_nanos(play_time_nanos) {
        scope.set_finished_time_nanos(scope.last_frame_time_nanos());
        scope.set_is_running(false);
    }
    scope.update_state(state);
    block(scope);
}

/// Animates a float from `initial_value` to `target_value` along `animation_spec` (Compose's `animate`), calling
/// `block` with the value and velocity each frame; done when the animation is.
#[named]
pub fn animate(
    initial_value: f32,
    target_value: f32,
    #[default(0.0)] initial_velocity: f32,
    #[default(Arc::new(spring::<f32>()))] animation_spec: Arc<dyn AnimationSpec<f32>>,
    block: impl FnMut(f32, f32) + 'static,
) -> impl Future<Output = ()> + 'static {
    animate_value_of(f32::VectorConverter, initial_value, target_value, Some(initial_velocity), animation_spec, block)
}

/// Animates a value of any type through its converter (Compose's `animate(typeConverter, …)`): from a velocity
/// of `None`, still.
/// Kotlin: `animate(typeConverter: TwoWayConverter<T, V>, initialValue: T, targetValue: T, initialVelocity: T?, animationSpec: AnimationSpec<T>, block: (T, T) -> Unit)`
#[named]
pub fn animate_value<T: AnimationValue, V: AnimationVector>(
    type_converter: TwoWayConverter<T, V>,
    initial_value: T,
    target_value: T,
    #[default(None)] initial_velocity: Option<T>,
    #[default(Arc::new(spring::<T>()))] animation_spec: Arc<dyn AnimationSpec<T>>,
    block: impl FnMut(T, T) + 'static,
) -> impl Future<Output = ()> + 'static {
    animate_value_of(type_converter, initial_value, target_value, initial_velocity, animation_spec, block)
}

async fn animate_value_of<T: AnimationValue, V: AnimationVector>(
    type_converter: TwoWayConverter<T, V>,
    initial_value: T,
    target_value: T,
    initial_velocity: Option<T>,
    animation_spec: Arc<dyn AnimationSpec<T>>,
    mut block: impl FnMut(T, T) + 'static,
) {
    let initial_velocity_vector = match initial_velocity {
        Some(v) => type_converter.convert_to_vector(v),
        None => type_converter.convert_to_vector(initial_value.clone()).new_instance(),
    };
    let anim = TargetBasedAnimation::new(
        &*animation_spec,
        type_converter.clone(),
        initial_value.clone(),
        target_value,
        Some(initial_velocity_vector),
    );
    let state = AnimationState::new(
        type_converter.clone(),
        initial_value,
        Some(initial_velocity_vector),
        AnimationConstants::UnspecifiedTime,
        AnimationConstants::UnspecifiedTime,
        false,
    );
    animate_state(&state, &anim, AnimationConstants::UnspecifiedTime, |scope| {
        block(scope.value(), type_converter.convert_from_vector(scope.velocity_vector()))
    })
    .await;
}

/// A float flung from `initial_value` at `initial_velocity`, slowing down along `animation_spec` (Compose's
/// `animateDecay`), `block` called with the value and velocity each frame.
pub fn animate_decay(
    initial_value: f32,
    initial_velocity: f32,
    animation_spec: impl FloatDecayAnimationSpec,
    mut block: impl FnMut(f32, f32) + 'static,
) -> impl Future<Output = ()> + 'static {
    let spec = generate_decay_animation_spec::<f32>(animation_spec);
    async move {
        let anim = DecayAnimation::new(&spec, f32::VectorConverter, initial_value, AnimationVector1D(initial_velocity));
        let state = AnimationState::new(
            f32::VectorConverter,
            initial_value,
            Some(AnimationVector1D(initial_velocity)),
            AnimationConstants::UnspecifiedTime,
            AnimationConstants::UnspecifiedTime,
            false,
        );
        animate_state(&state, &anim, AnimationConstants::UnspecifiedTime, |scope| {
            block(scope.value(), scope.velocity_vector().value)
        })
        .await;
    }
}

#[named]
impl<T: AnimationValue, V: AnimationVector> AnimationState<T, V> {
    /// Animates this state from its value and velocity to `target_value` along `animation_spec` (Compose's
    /// `AnimationState.animateTo`): `sequential_animation` starts it at the last one's last frame (seamless
    /// after it), else at the next frame; `block` sees each frame's scope.
    pub fn animate_to(
        &self,
        target_value: T,
        #[default(Arc::new(spring::<T>()))] animation_spec: Arc<dyn AnimationSpec<T>>,
        #[default(false)] sequential_animation: bool,
        #[default(Box::new(|_: &AnimationScope<T, V>| {}))] block: Box<dyn for<'a> FnMut(&'a AnimationScope<T, V>)>,
    ) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            let anim = TargetBasedAnimation::new(
                &*animation_spec,
                state.type_converter().clone(),
                state.peek(),
                target_value,
                Some(state.velocity_vector()),
            );
            let start =
                if sequential_animation { state.last_frame_time_nanos() } else { AnimationConstants::UnspecifiedTime };
            animate_state(&state, &anim, start, block).await;
        }
    }

    /// Flings this state from its value at its velocity along `animation_spec` (Compose's
    /// `AnimationState.animateDecay`).
    pub fn animate_decay(
        &self,
        animation_spec: impl DecayAnimationSpec<T>,
        #[default(false)] sequential_animation: bool,
        #[default(Box::new(|_: &AnimationScope<T, V>| {}))] block: Box<dyn for<'a> FnMut(&'a AnimationScope<T, V>)>,
    ) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            let anim = DecayAnimation::new(
                &animation_spec,
                state.type_converter().clone(),
                state.peek(),
                state.velocity_vector(),
            );
            let start =
                if sequential_animation { state.last_frame_time_nanos() } else { AnimationConstants::UnspecifiedTime };
            animate_state(&state, &anim, start, block).await;
        }
    }
}

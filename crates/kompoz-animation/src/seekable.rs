//! A transition state seeked or animated by hand (Compose's SeekableTransitionState, in Transition.kt): `seek_to` a
//! fraction of the way to a target, `animate_to` it (linearly over the transition's duration, or along a spec of the
//! fraction), `snap_to` a state. Its transition's total duration is followed through a [`SnapshotStateObserver`]
//! (Compose's), the transition's composition awaited when the target changes.

use crate::MillisToNanos;
use crate::converter::VectorConverter;
use crate::easing::coerce_in;
use crate::mutator::MutatorMutex;
use crate::spec::{AnimationConstants, FiniteAnimationSpec};
use crate::state::AnimationValue;
use crate::suspend::duration_scale;
use crate::transition::{
    ResetAnimationSnap, ResetAnimationSnapCurrent, ResetAnimationSnapTarget, Transition, TransitionInner,
    round_to_long, used_elsewhere,
};
use crate::vector::{AnimationVector, AnimationVector1D};
use crate::vectorized::VectorizedAnimationSpec;
use kompoz_coroutines::{CoroutineScope, cancel_current, coroutine_scope, with_frame_nanos};
use kompoz_runtime::kt::panic_kt;
use kompoz_runtime::{MutableState, SnapshotStateObserver, mutable_confined_state_of, mutable_state_of, named};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};

/// An animation of a seekable transition's fraction (Compose's internal `SeekableTransitionState.SeekingAnimationState`):
/// the running `animate_to`, or an initial value animation (a transition redirected before it finished: its start
/// values animated toward where they were going).
pub struct SeekingAnimationState {
    /// The progress, of its spec if it has one, else of `duration_nanos`.
    pub(crate) progress_nanos: Cell<i64>,
    /// Its spec; none: linear over `duration_nanos`.
    pub(crate) animation_spec: RefCell<Option<Rc<dyn VectorizedAnimationSpec<AnimationVector1D>>>>,
    /// An initial value animation goes on while this is false.
    pub(crate) is_complete: Cell<bool>,
    /// The fraction now.
    pub(crate) value: Cell<f32>,
    pub(crate) start: Cell<AnimationVector1D>,
    pub(crate) initial_velocity: Cell<Option<AnimationVector1D>>,
    /// The transition's total duration (an initial value animation's: when it was made).
    pub(crate) duration_nanos: Cell<i64>,
    /// Its spec's duration, kept (a spring's takes time to find).
    pub(crate) animation_spec_duration: Cell<i64>,
}

impl SeekingAnimationState {
    fn new() -> SeekingAnimationState {
        SeekingAnimationState {
            progress_nanos: Cell::new(0),
            animation_spec: RefCell::new(None),
            is_complete: Cell::new(false),
            value: Cell::new(0.0),
            start: Cell::new(AnimationVector1D(0.0)),
            initial_velocity: Cell::new(None),
            duration_nanos: Cell::new(0),
            animation_spec_duration: Cell::new(0),
        }
    }
}

impl fmt::Display for SeekingAnimationState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "progress nanos: {}, animationSpec: {}, isComplete: {}, value: {}, start: {}, initialVelocity: {:?}, \
             durationNanos: {}, animationSpecDuration: {}",
            self.progress_nanos.get(),
            if self.animation_spec.borrow().is_some() { "set" } else { "null" },
            self.is_complete.get(),
            self.value.get(),
            self.start.get(),
            self.initial_velocity.get(),
            self.duration_nanos.get(),
            self.animation_spec_duration.get()
        )
    }
}

/// AnimationVector1D with 0: a still start.
const ZeroVelocity: AnimationVector1D = AnimationVector1D(0.0);
/// AnimationVector1D with 1: the fraction's target.
const Target1: AnimationVector1D = AnimationVector1D(1.0);

/// Where a coroutine waiting for the composition is resumed with the target state it composed (Compose's
/// `compositionContinuation`).
struct Continuation<S> {
    value: RefCell<Option<S>>,
    waker: RefCell<Option<Waker>>,
}

/// The scope a seekable state's total duration is observed under (Compose's `this`): the same state.
struct ObservedScope<S>(Weak<SeekableInner<S>>);

impl<S> PartialEq for ObservedScope<S> {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.0, &other.0)
    }
}

pub(crate) struct SeekableInner<S> {
    target_state: MutableState<S>,
    current_state: MutableState<S>,
    is_running: MutableState<bool>,
    /// The target state the composition knows of: seeking waits until it is the target state.
    composed_target_state: RefCell<S>,
    /// The one transition it is used with.
    transition: RefCell<Option<Weak<TransitionInner<S>>>>,
    /// The transition's total duration, kept for the fraction's sake.
    total_duration_nanos: Cell<i64>,
    snapshot_state_observer: RefCell<Option<SnapshotStateObserver>>,
    fraction: MutableState<f32>,
    composition_continuation: RefCell<Option<Rc<Continuation<S>>>>,
    /// One of `snap_to`, `seek_to`, `animate_to` at a time.
    mutator_mutex: MutatorMutex,
    /// The last frame's time while an animation runs, else unspecified.
    last_frame_time_nanos: Cell<i64>,
    /// The initial value animations: none once `seek_to`, `snap_to` or `animate_to` completes.
    initial_value_animations: RefCell<Vec<Rc<SeekingAnimationState>>>,
    /// What `animate_to` animates; none while seeking or after `snap_to`.
    current_animation: RefCell<Option<Rc<SeekingAnimationState>>>,
    /// The duration scale of the frame being animated.
    duration_scale: Cell<f32>,
    /// What a change of the total duration calls (one callback: the observer's scopes are kept by it).
    on_total_duration_changed: Rc<dyn Fn(&ObservedScope<S>)>,
    this: Weak<SeekableInner<S>>,
}

/// A [`Transition`]'s state moved by hand (Compose's SeekableTransitionState): [`seek_to`](Self::seek_to) a
/// fraction of the way from its current state to a target, [`animate_to`](Self::animate_to) a target (the rest of
/// the way), [`snap_to`](Self::snap_to) one. Interrupted on its way, the values it had animate on toward where they
/// were going while the new run starts. One transition only ([`remember_transition`](crate::remember_transition) of
/// it). Clones are the same state; `remember` one.
pub struct SeekableTransitionState<S> {
    pub(crate) inner: Rc<SeekableInner<S>>,
}

impl<S> Clone for SeekableTransitionState<S> {
    fn clone(&self) -> Self {
        SeekableTransitionState { inner: self.inner.clone() }
    }
}

impl<S> PartialEq for SeekableTransitionState<S> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

/// Compose's `SeekableTransitionState(initialState)`: [`SeekableTransitionState::new`].
///
/// Kotlin: `SeekableTransitionState(initialState: S)`
pub fn SeekableTransitionState<S: AnimationValue>(initial_state: S) -> SeekableTransitionState<S> {
    SeekableTransitionState::new(initial_state)
}

impl<S: AnimationValue> SeekableTransitionState<S> {
    /// A state at `initial_state`.
    pub fn new(initial_state: S) -> SeekableTransitionState<S> {
        let inner = Rc::new_cyclic(|this: &Weak<SeekableInner<S>>| SeekableInner {
            target_state: mutable_confined_state_of(initial_state.clone()),
            current_state: mutable_confined_state_of(initial_state.clone()),
            is_running: mutable_state_of(false),
            composed_target_state: RefCell::new(initial_state),
            transition: RefCell::new(None),
            total_duration_nanos: Cell::new(0),
            snapshot_state_observer: RefCell::new(None),
            fraction: mutable_state_of(0.0),
            composition_continuation: RefCell::new(None),
            mutator_mutex: MutatorMutex::new(),
            last_frame_time_nanos: Cell::new(AnimationConstants::UnspecifiedTime),
            initial_value_animations: RefCell::new(Vec::new()),
            current_animation: RefCell::new(None),
            duration_scale: Cell::new(0.0),
            on_total_duration_changed: Rc::new(|scope: &ObservedScope<S>| {
                if let Some(s) = scope.0.upgrade() {
                    s.on_total_duration_changed();
                }
            }),
            this: this.clone(),
        });
        SeekableTransitionState { inner }
    }

    /// The state its transition is in (a state read).
    pub fn current_state(&self) -> S {
        self.inner.current_state.get()
    }

    /// Where its transition goes (a state read).
    pub fn target_state(&self) -> S {
        self.inner.target_state.get()
    }

    /// How far from its current state to its target state it is, of the transition's whole duration (0 when they
    /// are the same; a state read).
    pub fn fraction(&self) -> f32 {
        self.inner.fraction.get()
    }

    pub(crate) fn set_current_state(&self, state: S) {
        self.inner.current_state.set(state);
    }

    pub(crate) fn is_running_state(&self) -> &MutableState<bool> {
        &self.inner.is_running
    }

    pub(crate) fn transition_configured(&self, transition: &Rc<TransitionInner<S>>) {
        let previous = self.inner.transition.borrow().as_ref().and_then(|t| t.upgrade());
        if let Some(p) = previous
            && !Rc::ptr_eq(&p, transition)
        {
            used_elsewhere(&format!("{:p}", Rc::as_ptr(&p)), &format!("{:p}", Rc::as_ptr(transition)));
        }
        *self.inner.transition.borrow_mut() = Some(Rc::downgrade(transition));
    }

    pub(crate) fn transition_removed(&self) {
        *self.inner.transition.borrow_mut() = None;
        if let Some(o) = self.inner.snapshot_state_observer.borrow().clone() {
            o.clear(&self.inner.scope());
        }
    }

    /// Gives it the observer its transition's total duration is followed through (rememberTransition's), the old
    /// one stopped.
    pub(crate) fn set_snapshot_state_observer(&self, value: Option<SnapshotStateObserver>) {
        let old = self.inner.snapshot_state_observer.borrow().clone();
        let same = match (&old, &value) {
            (Some(a), Some(b)) => a == b,
            (None, None) => true,
            _ => false,
        };
        if !same {
            if let Some(o) = &old {
                o.clear(&self.inner.scope());
                o.stop();
            }
            *self.inner.snapshot_state_observer.borrow_mut() = value.clone();
            if let Some(v) = &value {
                v.start();
            }
            self.inner.observe_total_duration();
        }
    }

    pub(crate) fn observe_total_duration(&self) {
        self.inner.observe_total_duration();
    }

    /// The composition composed its current target state: who waits for it goes on (rememberTransition's effect).
    pub(crate) fn on_composed(&self) {
        let target = self.inner.target_state.get();
        *self.inner.composed_target_state.borrow_mut() = target.clone();
        let continuation = self.inner.composition_continuation.borrow_mut().take();
        if let Some(c) = continuation {
            *c.value.borrow_mut() = Some(target);
            if let Some(w) = c.waker.borrow_mut().take() {
                w.wake();
            }
        }
    }
}

impl<S: AnimationValue> SeekableInner<S> {
    fn rc(&self) -> Rc<SeekableInner<S>> {
        self.this.upgrade().expect("a seekable transition state in use")
    }

    fn scope(&self) -> ObservedScope<S> {
        ObservedScope(self.this.clone())
    }

    fn transition(&self) -> Option<Transition<S>> {
        self.transition.borrow().as_ref().and_then(|t| t.upgrade()).map(|inner| Transition { inner })
    }

    fn observe_total_duration(&self) {
        let observer = self.snapshot_state_observer.borrow().clone();
        if let Some(o) = observer {
            let this = self.rc();
            o.observe_reads(self.scope(), self.on_total_duration_changed.clone(), move || {
                this.total_duration_nanos.set(this.transition().map(|t| t.total_duration_nanos()).unwrap_or(0));
            });
        }
    }

    fn on_total_duration_changed(&self) {
        let previous_total_duration_nanos = self.total_duration_nanos.get();
        self.observe_total_duration();
        let total = self.total_duration_nanos.get();
        if previous_total_duration_nanos != total {
            let animation = self.current_animation.borrow().clone();
            if let Some(animation) = animation {
                if animation.progress_nanos.get() > total {
                    self.end_all_animations();
                } else {
                    animation.duration_nanos.set(total);
                    if animation.animation_spec.borrow().is_none() {
                        animation
                            .animation_spec_duration
                            .set(round_to_long((1.0 - animation.start.get().value as f64) * total as f64));
                    }
                }
            } else if total != 0 {
                // (seeked to a fraction: the values follow it; a running animation moves them itself)
                self.seek_to_fraction();
            }
        }
    }

    fn seek_to_fraction(&self) {
        let Some(transition) = self.transition() else { return };
        let play_time_nanos = round_to_long(self.fraction.get() as f64 * transition.total_duration_nanos() as f64);
        transition.seek_animations(play_time_nanos);
    }

    /// Stops every animation, the initial value ones too; the fraction 1 (Compose's `endAllAnimations`).
    fn end_all_animations(&self) {
        if let Some(t) = self.transition() {
            t.clear_initial_animations();
        }
        self.initial_value_animations.borrow_mut().clear();
        let current = self.current_animation.borrow_mut().take();
        if current.is_some() {
            self.fraction.set(1.0);
            self.seek_to_fraction();
        }
    }

    /// Runs the animation and the initial value ones until they are done; one frame first to take the start time
    /// if none ran (Compose's `runAnimations`).
    async fn run_animations(self: Rc<Self>) {
        if self.initial_value_animations.borrow().is_empty() && self.current_animation.borrow().is_none() {
            return;
        }
        if duration_scale() == 0.0 {
            self.end_all_animations();
            self.last_frame_time_nanos.set(AnimationConstants::UnspecifiedTime);
            return;
        }
        if self.last_frame_time_nanos.get() == AnimationConstants::UnspecifiedTime {
            let this = self.clone();
            with_frame_nanos(move |t| this.last_frame_time_nanos.set(t)).await;
        }
        while !self.initial_value_animations.borrow().is_empty() || self.current_animation.borrow().is_some() {
            self.clone().animate_one_frame().await;
        }
        self.last_frame_time_nanos.set(AnimationConstants::UnspecifiedTime);
    }

    /// One frame: the start time taken if no animation ran, else a frame of them (Compose's `doOneFrame`).
    async fn do_one_frame(self: Rc<Self>) {
        if self.last_frame_time_nanos.get() == AnimationConstants::UnspecifiedTime {
            let this = self.clone();
            with_frame_nanos(move |t| this.last_frame_time_nanos.set(t)).await;
        } else {
            self.animate_one_frame().await;
        }
    }

    async fn animate_one_frame(self: Rc<Self>) {
        let duration_scale = duration_scale();
        if duration_scale <= 0.0 {
            self.end_all_animations();
        } else {
            self.duration_scale.set(duration_scale);
            let this = self.clone();
            with_frame_nanos(move |t| this.animate_one_frame_at(t)).await;
        }
    }

    /// The frame at `frame_time_nanos` (Compose's `animateOneFrameLambda`).
    fn animate_one_frame_at(&self, frame_time_nanos: i64) {
        let delta = frame_time_nanos - self.last_frame_time_nanos.get();
        self.last_frame_time_nanos.set(frame_time_nanos);
        let delta_play_time_nanos = round_to_long(delta as f64 / self.duration_scale.get() as f64);
        let initial = self.initial_value_animations.borrow().clone();
        if !initial.is_empty() {
            for animation in &initial {
                // (update_initial_values sets it back if it isn't done)
                self.recalculate_animation_value(animation, delta_play_time_nanos);
                animation.is_complete.set(true);
            }
            if let Some(t) = self.transition() {
                t.update_initial_values();
            }
            self.initial_value_animations.borrow_mut().retain(|a| !a.is_complete.get());
        }
        let current = self.current_animation.borrow().clone();
        if let Some(current) = current {
            current.duration_nanos.set(self.total_duration_nanos.get());
            self.recalculate_animation_value(&current, delta_play_time_nanos);
            self.fraction.set(current.value.get());
            if current.value.get() == 1.0 {
                *self.current_animation.borrow_mut() = None; // all done
            }
            self.seek_to_fraction();
        }
    }

    /// The animation's value `delta_play_time_nanos` on: along its spec, or linear over its duration (Compose's
    /// `recalculateAnimationValue`; no duration scale here).
    fn recalculate_animation_value(&self, animation: &SeekingAnimationState, delta_play_time_nanos: i64) {
        let play_time_nanos = animation.progress_nanos.get() + delta_play_time_nanos;
        animation.progress_nanos.set(play_time_nanos);
        let duration_nanos = animation.animation_spec_duration.get();
        if play_time_nanos >= duration_nanos {
            animation.value.set(1.0);
        } else {
            let spec = animation.animation_spec.borrow().clone();
            match spec {
                Some(spec) => {
                    let v = spec.get_value_from_nanos(
                        play_time_nanos,
                        animation.start.get(),
                        Target1,
                        animation.initial_velocity.get().unwrap_or(ZeroVelocity),
                    );
                    animation.value.set(coerce_in(v.value, 0.0, 1.0));
                }
                None => {
                    let fraction = play_time_nanos as f32 / duration_nanos as f32;
                    animation.value.set(lerp(animation.start.get().value, 1.0, fraction));
                }
            }
        }
    }

    /// Waits for the composition of the target state when it isn't composed yet (Compose's
    /// `waitForCompositionAfterTargetStateChange`): cancelled if another was composed.
    async fn wait_for_composition_after_target_state_change(self: Rc<Self>) {
        let expected_state = self.target_state.get();
        if expected_state == *self.composed_target_state.borrow() {
            return;
        }
        let state = self.clone().await_composition().await;
        if state != expected_state {
            self.last_frame_time_nanos.set(AnimationConstants::UnspecifiedTime);
            // (Compose: CancellationException("snapTo() was canceled because state was changed to …"))
            cancel_current::<()>().await;
        }
    }

    /// Waits for a composition whatever the target state (Compose's `waitForComposition`: the current state is what
    /// changes).
    async fn wait_for_composition(self: Rc<Self>) {
        let expected_state = self.target_state.get();
        let state = self.clone().await_composition().await;
        if state != expected_state {
            self.last_frame_time_nanos.set(AnimationConstants::UnspecifiedTime);
            // (Compose: CancellationException("targetState while waiting for composition"))
            cancel_current::<()>().await;
        }
    }

    /// Suspends until the transition's composition resumes it with the target state it composed.
    async fn await_composition(self: Rc<Self>) -> S {
        let continuation = Rc::new(Continuation { value: RefCell::new(None), waker: RefCell::new(None) });
        *self.composition_continuation.borrow_mut() = Some(continuation.clone());
        std::future::poll_fn(move |cx| match continuation.value.borrow_mut().take() {
            Some(v) => Poll::Ready(v),
            None => {
                *continuation.waker.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }

    /// The running animation made an initial value animation, if it is to be one (Compose's
    /// `moveAnimationToInitialState`).
    fn move_animation_to_initial_state(&self) {
        let Some(transition) = self.transition() else { return };
        let total = self.total_duration_nanos.get();
        let fraction = self.fraction.get();
        let current = self.current_animation.borrow().clone();
        let animation = current.or_else(|| {
            if total <= 0 || fraction == 1.0 || self.current_state.get() == self.target_state.get() {
                None
            } else {
                let a = SeekingAnimationState::new();
                a.value.set(fraction);
                a.duration_nanos.set(total);
                a.animation_spec_duration.set(round_to_long(total as f64 * (1.0 - fraction as f64)));
                a.start.set(AnimationVector1D(fraction));
                Some(Rc::new(a))
            }
        });
        if let Some(animation) = animation {
            animation.duration_nanos.set(total);
            self.initial_value_animations.borrow_mut().push(animation.clone());
            transition.set_initial_animations(&animation);
        }
        *self.current_animation.borrow_mut() = None;
    }
}

/// Kotlin's `lerp(start, stop, fraction)` of floats.
fn lerp(start: f32, stop: f32, fraction: f32) -> f32 {
    (1.0 - fraction) * start + fraction * stop
}

#[named]
impl<S: AnimationValue> SeekableTransitionState<S> {
    /// Puts it at `target_state`, current and target, every value there at once, no animation left (Compose's
    /// `snapTo`): unlike [`seek_to`](Self::seek_to), no further seeking toward it. A state other than the current and
    /// target ones waits for the composition to set its values up.
    pub fn snap_to(&self, target_state: S) -> impl Future<Output = ()> + 'static {
        let this = self.inner.clone();
        async move {
            let Some(transition) = this.transition() else { return };
            if this.current_state.get() == target_state && this.target_state.get() == target_state {
                return; // nothing to change
            }
            let mutex = this.mutator_mutex.clone();
            let body = async move {
                this.end_all_animations();
                this.last_frame_time_nanos.set(AnimationConstants::UnspecifiedTime);
                this.fraction.set(0.0);
                let fraction = if target_state == this.current_state.get() {
                    ResetAnimationSnapCurrent
                } else if target_state == this.target_state.get() {
                    ResetAnimationSnapTarget
                } else {
                    ResetAnimationSnap
                };
                transition.update_target(target_state.clone());
                transition.set_play_time_nanos(0);
                this.target_state.set(target_state.clone());
                this.fraction.set(0.0);
                this.current_state.set(target_state);
                transition.reset_animation_fraction(fraction);
                if fraction == ResetAnimationSnap {
                    // (all changed: the composition sets the right values up)
                    this.clone().wait_for_composition_after_target_state_change().await;
                }
                transition.on_transition_end();
            };
            mutex.mutate(body).await;
        }
    }

    /// Seeks toward `target_state` (its target by default), `fraction` of the way (Compose's `seekTo`): to its target
    /// already, it stops the running animation and puts the fraction there; to another, its current state becomes its
    /// former target, and the values it had animate on toward it meanwhile. Done once those initial values arrived and
    /// the fraction is reached. A fraction outside 0..1 panics (Kotlin's IllegalArgumentException).
    #[track_caller]
    pub fn seek_to(
        &self,
        fraction: f32,
        #[default(self.target_state())] target_state: S,
    ) -> impl Future<Output = ()> + 'static {
        if !(0.0..=1.0).contains(&fraction) {
            panic_kt("IllegalArgumentException", &format!("Expecting fraction between 0 and 1. Got {fraction}"));
        }
        let this = self.inner.clone();
        async move {
            let Some(transition) = this.transition() else { return };
            let old_target_state = this.target_state.get();
            let mutex = this.mutator_mutex.clone();
            let body = coroutine_scope(move |scope: CoroutineScope| async move {
                if target_state != old_target_state {
                    this.move_animation_to_initial_state();
                } else {
                    *this.current_animation.borrow_mut() = None;
                    if this.current_state.get() == target_state {
                        return; // (no seeking where the current state is the target)
                    }
                }
                if target_state != old_target_state {
                    // (the target and the fraction)
                    transition.update_target(target_state.clone());
                    transition.set_play_time_nanos(0);
                    this.target_state.set(target_state);
                    transition.reset_animation_fraction(fraction);
                }
                this.fraction.set(fraction);
                if !this.initial_value_animations.borrow().is_empty() {
                    scope.launch(this.clone().run_animations());
                } else {
                    this.last_frame_time_nanos.set(AnimationConstants::UnspecifiedTime);
                }
                this.clone().wait_for_composition_after_target_state_change().await;
                this.seek_to_fraction();
            });
            mutex.mutate(body).await;
        }
    }

    /// Animates to `target_state` (its target by default), the rest of the way (Compose's `animateTo`): the
    /// fraction along `animation_spec` (none: linearly over the transition's duration). To a new target, an
    /// interrupted run's values animate on toward where they were going. Done, its current state is the target.
    pub fn animate_to(
        &self,
        #[default(self.target_state())] target_state: S,
        #[default(None)] animation_spec: Option<Arc<dyn FiniteAnimationSpec<f32>>>,
    ) -> impl Future<Output = ()> + 'static {
        let this = self.inner.clone();
        async move {
            let Some(transition) = this.transition() else { return };
            let mutex = this.mutator_mutex.clone();
            let body = coroutine_scope(move |_scope: CoroutineScope| async move {
                let old_target_state = this.target_state.get();
                if target_state != old_target_state {
                    this.move_animation_to_initial_state();
                    this.fraction.set(0.0);
                    transition.update_target(target_state.clone());
                    transition.set_play_time_nanos(0);
                    this.current_state.set(old_target_state);
                    this.target_state.set(target_state.clone());
                }
                let composed_target_state = this.composed_target_state.borrow().clone();
                if target_state != composed_target_state {
                    // (a frame while waiting for the composition, so none is skipped then)
                    this.clone().do_one_frame().await;
                    this.clone().wait_for_composition_after_target_state_change().await;
                }
                if this.current_state.get() != target_state {
                    if this.fraction.get() < 1.0 {
                        let running_animation = this.current_animation.borrow().clone();
                        let new_spec: Option<Rc<dyn VectorizedAnimationSpec<AnimationVector1D>>> =
                            animation_spec.as_ref().map(|s| {
                                let v: Box<dyn VectorizedAnimationSpec<AnimationVector1D>> =
                                    AnimationVector1D::vectorize_finite(&**s, &f32::VectorConverter);
                                Rc::from(v)
                            });
                        // (a spec vectorized anew is never the running one's: equal only when neither has one)
                        let same_spec = running_animation
                            .as_ref()
                            .is_some_and(|r| new_spec.is_none() && r.animation_spec.borrow().is_none());
                        if !same_spec {
                            let total = this.total_duration_nanos.get();
                            let old_spec = running_animation.as_ref().and_then(|r| r.animation_spec.borrow().clone());
                            let old_velocity = match (&running_animation, old_spec) {
                                (Some(running), Some(old_spec)) => old_spec.get_velocity_from_nanos(
                                    running.progress_nanos.get(),
                                    running.start.get(),
                                    Target1,
                                    running.initial_velocity.get().unwrap_or(ZeroVelocity),
                                ),
                                (None, _) => ZeroVelocity,
                                (Some(running), None) if running.progress_nanos.get() == 0 => ZeroVelocity,
                                (Some(running), None) => {
                                    let old_duration_nanos = running.duration_nanos.get();
                                    let old_duration = if old_duration_nanos == AnimationConstants::UnspecifiedTime {
                                        total
                                    } else {
                                        old_duration_nanos
                                    } as f32
                                        / (1000.0 * MillisToNanos as f32);
                                    if old_duration <= 0.0 {
                                        ZeroVelocity
                                    } else {
                                        AnimationVector1D(1.0 / old_duration)
                                    }
                                }
                            };
                            let new_animation =
                                running_animation.unwrap_or_else(|| Rc::new(SeekingAnimationState::new()));
                            let fraction = this.fraction.get();
                            *new_animation.animation_spec.borrow_mut() = new_spec.clone();
                            new_animation.is_complete.set(false);
                            new_animation.value.set(fraction);
                            new_animation.start.set(AnimationVector1D(fraction));
                            new_animation.duration_nanos.set(total);
                            new_animation.progress_nanos.set(0);
                            new_animation.initial_velocity.set(Some(old_velocity));
                            new_animation.animation_spec_duration.set(match &new_spec {
                                Some(spec) => spec.get_duration_nanos(new_animation.start.get(), Target1, old_velocity),
                                None => round_to_long(total as f64 * (1.0 - fraction as f64)),
                            });
                            *this.current_animation.borrow_mut() = Some(new_animation);
                        }
                    }
                    this.clone().run_animations().await;
                    this.current_state.set(target_state);
                    this.fraction.set(0.0);
                    transition.on_transition_end();
                    this.clone().wait_for_composition().await;
                }
            });
            mutex.mutate(body).await;
        }
    }
}

impl<S: AnimationValue + fmt::Debug> fmt::Debug for SeekableTransitionState<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "SeekableTransitionState({:?} → {:?}, fraction {})",
            self.inner.current_state.peek(),
            self.inner.target_state.peek(),
            self.inner.fraction.peek()
        )
    }
}

/// The observer a seekable state's total duration is followed through (rememberTransition's): changes applied on
/// this thread delivered at once; from another, through a coroutine of `scope` (Compose's
/// `coroutineScope.launch { it() }`).
pub(crate) fn seekable_observer(scope: &CoroutineScope) -> SnapshotStateObserver {
    type Delivery = Box<dyn FnOnce() + Send>;
    let thread = std::thread::current().id();
    let queue: Arc<Mutex<(Vec<Delivery>, Option<Waker>)>> = Arc::new(Mutex::new((Vec::new(), None)));
    let q = queue.clone();
    scope.launch(async move {
        loop {
            let deliveries = std::future::poll_fn(|cx| {
                let mut g = q.lock().unwrap_or_else(|e| e.into_inner());
                if g.0.is_empty() {
                    g.1 = Some(cx.waker().clone());
                    Poll::Pending
                } else {
                    Poll::Ready(std::mem::take(&mut g.0))
                }
            })
            .await;
            for d in deliveries {
                d();
            }
        }
    });
    SnapshotStateObserver::new(move |deliver| {
        if std::thread::current().id() == thread {
            deliver();
        } else {
            let waker = {
                let mut g = queue.lock().unwrap_or_else(|e| e.into_inner());
                g.0.push(deliver);
                g.1.take()
            };
            if let Some(w) = waker {
                w.wake();
            }
        }
    })
}

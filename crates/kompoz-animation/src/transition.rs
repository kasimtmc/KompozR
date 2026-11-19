//! Transitions (Compose's animation-core Transition.kt, and animation's `Transition.animateColor`): animations run
//! together from one state to another — [`update_transition`], [`remember_transition`] of a [`TransitionState`]
//! ([`MutableTransitionState`], [`SeekableTransitionState`], [`DeferredTransitionState`]), the values a
//! [`Transition`] animates (`animate_float`, `animate_dp`, … `animate_color`, `animate_value`), its child transitions
//! (`create_child_transition`) and the animations set up after composition (`create_deferred_animation`).
//!
//! ```no_run
//! # use kompoz_animation::*;
//! # use kompoz_runtime::*;
//! # use kompoz_graphics::Dp;
//! #[kompozable]
//! fn Panel(open: bool) {
//!     let transition = update_transition(open, Some("panel".to_string()));
//!     let width = transition.animate_dp(|_| std::sync::Arc::new(tween(300)), "width".into(), |o: &bool| {
//!         if *o { Dp(200.0) } else { Dp(80.0) }
//!     });
//!     let _ = width.value();
//! }
//! ```

use crate::animation::{Animation, TargetBasedAnimation};
use crate::color::ColorVectorConverter;
use crate::converter::{TwoWayConverter, VectorConverter, VisibilityThreshold, visibility_threshold_of};
use crate::seekable::{SeekableTransitionState, SeekingAnimationState};
use crate::spec::{AnimationConstants, AnimationSpec, FiniteAnimationSpec, SpringSpec, delayed, spring};
use crate::state::AnimationValue;
use crate::suspend::duration_scale;
use crate::vector::{AnimationVector, AnimationVector4D};
use kompoz_coroutines::with_frame_nanos;
use kompoz_graphics::{Color, Dp, IntOffset, IntSize, Offset, Rect, Size};
use kompoz_runtime::kt::panic_kt;
use kompoz_runtime::{
    DerivedState, DisposableEffect, MutableState, State, StateSource, derived_state_of, group, kompozable,
    kompozable_block, mutable_state_of, named, on_dispose, remember_keyed, without_read_observation,
};
use std::any::Any;
use std::cell::{Cell, OnceCell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// Compose's internal AnimationDebugDurationScale: frame times are divided by it (1).
const AnimationDebugDurationScale: i64 = 1;

/// When a TransitionAnimation doesn't need to be reset.
pub(crate) const NoReset: f32 = -1.0;
/// When the animation needs to be changed because of a target update.
pub(crate) const ResetNoSnap: f32 = -2.0;
/// When the animation should be reset to have the same start and end value.
pub(crate) const ResetAnimationSnap: f32 = -3.0;
/// Snap to the current state and set the initial and target values to the same thing.
pub(crate) const ResetAnimationSnapCurrent: f32 = -4.0;
/// Snap to the target state and set the initial and target values to the same thing.
pub(crate) const ResetAnimationSnapTarget: f32 = -5.0;

/// Kotlin's `Double.roundToLong()` (Math.round: half up).
pub(crate) fn round_to_long(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

// ---- Transition states --------------------------------------------------------------------------------------------

/// A [`Transition`]'s state for [`remember_transition`] (Compose's sealed TransitionState): a
/// [`MutableTransitionState`] targeted by setting its target, a [`SeekableTransitionState`] seeked or animated by
/// hand, or a [`DeferredTransitionState`] with a deferred phase. Clones are the same state.
pub enum TransitionState<S> {
    Mutable(MutableTransitionState<S>),
    Seekable(SeekableTransitionState<S>),
    Deferred(DeferredTransitionState<S>),
}

impl<S> Clone for TransitionState<S> {
    fn clone(&self) -> Self {
        match self {
            TransitionState::Mutable(s) => TransitionState::Mutable(s.clone()),
            TransitionState::Seekable(s) => TransitionState::Seekable(s.clone()),
            TransitionState::Deferred(s) => TransitionState::Deferred(s.clone()),
        }
    }
}

impl<S> PartialEq for TransitionState<S> {
    /// The same state.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TransitionState::Mutable(a), TransitionState::Mutable(b)) => a == b,
            (TransitionState::Seekable(a), TransitionState::Seekable(b)) => a == b,
            (TransitionState::Deferred(a), TransitionState::Deferred(b)) => a == b,
            _ => false,
        }
    }
}

impl<S: AnimationValue> TransitionState<S> {
    /// The state the transition is in; while a transition runs, the one it started from (Compose's `currentState`).
    pub fn current_state(&self) -> S {
        match self {
            TransitionState::Mutable(s) => s.current_state(),
            TransitionState::Seekable(s) => s.current_state(),
            TransitionState::Deferred(s) => s.current_state(),
        }
    }

    /// The state the transition goes to; the current one when none runs (Compose's `targetState`).
    pub fn target_state(&self) -> S {
        match self {
            TransitionState::Mutable(s) => s.target_state(),
            TransitionState::Seekable(s) => s.target_state(),
            TransitionState::Deferred(s) => s.target_state(),
        }
    }

    pub(crate) fn set_current_state(&self, state: S) {
        match self {
            TransitionState::Mutable(s) => s.inner.current_state.set(state),
            TransitionState::Seekable(s) => s.set_current_state(state),
            TransitionState::Deferred(s) => s.inner.current_state.set(state),
        }
    }

    pub(crate) fn set_is_running(&self, running: bool) {
        match self {
            TransitionState::Mutable(s) => s.inner.is_running.set(running),
            TransitionState::Seekable(s) => s.is_running_state().set(running),
            TransitionState::Deferred(s) => s.inner.is_running.set(running),
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        match self {
            TransitionState::Mutable(s) => s.inner.is_running.get(),
            TransitionState::Seekable(s) => s.is_running_state().get(),
            TransitionState::Deferred(s) => s.inner.is_running.get(),
        }
    }

    /// Whether the transition sets its current state as it ends (Compose's `is MutableTransitionState || is
    /// DeferredTransitionState`).
    fn sets_current_state(&self) -> bool {
        !matches!(self, TransitionState::Seekable(_))
    }

    fn transition_configured(&self, transition: &Rc<TransitionInner<S>>) {
        if let TransitionState::Seekable(s) = self {
            s.transition_configured(transition);
        }
    }

    fn transition_removed(&self) {
        if let TransitionState::Seekable(s) = self {
            s.transition_removed();
        }
    }
}

impl<S> From<MutableTransitionState<S>> for TransitionState<S> {
    fn from(s: MutableTransitionState<S>) -> Self {
        TransitionState::Mutable(s)
    }
}
impl<S> From<SeekableTransitionState<S>> for TransitionState<S> {
    fn from(s: SeekableTransitionState<S>) -> Self {
        TransitionState::Seekable(s)
    }
}
impl<S> From<DeferredTransitionState<S>> for TransitionState<S> {
    fn from(s: DeferredTransitionState<S>) -> Self {
        TransitionState::Deferred(s)
    }
}

struct MutableTransitionStateInner<S> {
    current_state: MutableState<S>,
    target_state: MutableState<S>,
    is_running: MutableState<bool>,
}

/// A transition's state as two states, its current one and its target (Compose's MutableTransitionState): its
/// current state is what it is made with until the [`Transition`] made of it ([`remember_transition`]) arrives
/// elsewhere; set its target to start a transition (a target other than the initial state at once: a transition
/// as it enters the composition). Clones are the same state; `remember` one.
pub struct MutableTransitionState<S> {
    inner: Arc<MutableTransitionStateInner<S>>,
}

impl<S> Clone for MutableTransitionState<S> {
    fn clone(&self) -> Self {
        MutableTransitionState { inner: self.inner.clone() }
    }
}

impl<S> PartialEq for MutableTransitionState<S> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

/// Compose's `MutableTransitionState(initialState)`: [`MutableTransitionState::new`].
///
/// Kotlin: `MutableTransitionState(initialState: S)`
pub fn MutableTransitionState<S: AnimationValue>(initial_state: S) -> MutableTransitionState<S> {
    MutableTransitionState::new(initial_state)
}

impl<S: AnimationValue> MutableTransitionState<S> {
    /// A state whose current and target states are `initial_state`.
    pub fn new(initial_state: S) -> MutableTransitionState<S> {
        MutableTransitionState {
            inner: Arc::new(MutableTransitionStateInner {
                current_state: mutable_state_of(initial_state.clone()),
                target_state: mutable_state_of(initial_state),
                is_running: mutable_state_of(false),
            }),
        }
    }

    /// The state it is in: the initial state until its transition arrives at another (a state read).
    pub fn current_state(&self) -> S {
        self.inner.current_state.get()
    }

    /// Where its transition goes (a state read).
    pub fn target_state(&self) -> S {
        self.inner.target_state.get()
    }

    /// Sets where its transition goes: the transition made of it animates there.
    pub fn set_target_state(&self, target_state: S) {
        self.inner.target_state.set(target_state);
    }

    /// Whether its transition has finished (Compose's `isIdle`): at its target, not running. Setting another target
    /// makes it not idle at once.
    pub fn is_idle(&self) -> bool {
        self.current_state() == self.target_state() && !self.inner.is_running.get()
    }
}

impl<S: AnimationValue + fmt::Debug> fmt::Debug for MutableTransitionState<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MutableTransitionState({:?} → {:?})",
            self.inner.current_state.peek(),
            self.inner.target_state.peek()
        )
    }
}

struct DeferredTransitionStateInner<S> {
    current_state: MutableState<S>,
    target_state: MutableState<S>,
    pending_target_state: MutableState<Option<S>>,
    is_running: MutableState<bool>,
}

/// A transition's state with a deferred phase before its automatic transition (Compose's experimental
/// DeferredTransitionState): [`defer`](Self::defer) holds the new target as [`pending_target_state`](Self::
/// pending_target_state) while the target stays (a predictive back gesture's components set their values by hand
/// meanwhile); [`animate_to`](Self::animate_to) ends the phase and starts the transition. Clones are the same
/// state; `remember` one, and make a [`DeferredTransition`] of it with [`remember_deferred_transition`].
pub struct DeferredTransitionState<S> {
    inner: Arc<DeferredTransitionStateInner<S>>,
}

impl<S> Clone for DeferredTransitionState<S> {
    fn clone(&self) -> Self {
        DeferredTransitionState { inner: self.inner.clone() }
    }
}

impl<S> PartialEq for DeferredTransitionState<S> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

/// Compose's `DeferredTransitionState(initialState)`: [`DeferredTransitionState::new`].
///
/// Kotlin: `DeferredTransitionState(initialState: S)`
pub fn DeferredTransitionState<S: AnimationValue>(initial_state: S) -> DeferredTransitionState<S> {
    DeferredTransitionState::new(initial_state)
}

impl<S: AnimationValue> DeferredTransitionState<S> {
    /// A state whose current and target states are `initial_state`, nothing pending.
    pub fn new(initial_state: S) -> DeferredTransitionState<S> {
        DeferredTransitionState {
            inner: Arc::new(DeferredTransitionStateInner {
                current_state: mutable_state_of(initial_state.clone()),
                target_state: mutable_state_of(initial_state),
                pending_target_state: mutable_state_of(None),
                is_running: mutable_state_of(false),
            }),
        }
    }

    /// The state it is in (a state read).
    pub fn current_state(&self) -> S {
        self.inner.current_state.get()
    }

    /// Where its transition goes (a state read).
    pub fn target_state(&self) -> S {
        self.inner.target_state.get()
    }

    /// Where its transition will go once the deferred phase ends: set by [`defer`](Self::defer), none after
    /// [`animate_to`](Self::animate_to) (a state read).
    pub fn pending_target_state(&self) -> Option<S> {
        self.inner.pending_target_state.get()
    }

    /// Starts the deferred phase toward `target_state` (none pending if it is the target already): the target stays,
    /// a running transition goes on to it, the components see the pending state.
    pub fn defer(&self, target_state: S) {
        if self.inner.target_state.get() == target_state {
            self.inner.pending_target_state.set(None);
        } else {
            self.inner.pending_target_state.set(Some(target_state));
        }
    }

    /// Ends the deferred phase: nothing pending, `target_state` the target (another state than the one deferred to
    /// goes straight there).
    pub fn animate_to(&self, target_state: S) {
        self.inner.pending_target_state.set(None);
        self.inner.target_state.set(target_state);
    }
}

// ---- Segment ------------------------------------------------------------------------------------------------------

/// The beginning and the end of a transition (Compose's `Transition.Segment`): what an animation's spec is picked by
/// (`transition_spec`).
///
/// Kotlin: `Transition.Segment`
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Segment<S> {
    initial_state: S,
    target_state: S,
}

impl<S: Clone + PartialEq> Segment<S> {
    /// A segment from `initial_state` to `target_state`.
    pub fn new(initial_state: S, target_state: S) -> Segment<S> {
        Segment { initial_state, target_state }
    }

    /// The state the transition starts from.
    pub fn initial_state(&self) -> S {
        self.initial_state.clone()
    }

    /// The state the transition ends on.
    pub fn target_state(&self) -> S {
        self.target_state.clone()
    }

    /// Whether `state` is its initial state and `target_state` its target (Compose's `S.isTransitioningTo(targetState)`,
    /// infix: `false isTransitioningTo true`).
    pub fn is_transitioning_to(&self, state: S, target_state: S) -> bool {
        state == self.initial_state && target_state == self.target_state
    }
}

// ---- Type-erased views ----------------------------------------------------------------------------------------------

/// A [`Transition`] of any state's type (Compose's `Transition<*>`): what [`Transition::transitions`] lists. Downcast
/// with [`Transition::from_any`].
pub trait AnyTransition {
    /// Its label (tools show it).
    fn label(&self) -> Option<String>;
    /// Whether any of its animations runs.
    fn is_running(&self) -> bool;
    /// Whether it is seeked (tools').
    fn is_seeking(&self) -> bool;
    /// How long its animations and child transitions take.
    fn total_duration_nanos(&self) -> i64;
    /// Its play time.
    fn play_time_nanos(&self) -> i64;
    /// Its animations.
    fn animations(&self) -> Vec<Rc<dyn AnyTransitionAnimationState>>;
    /// Its child transitions.
    fn transitions(&self) -> Vec<Rc<dyn AnyTransition>>;
    /// Whether a seekable transition animates the initial values of it or of a child (Compose's internal
    /// `hasInitialValueAnimations`).
    fn has_initial_value_animations(&self) -> bool;
    /// Itself as `Any`, to downcast.
    fn into_any_rc(self: Rc<Self>) -> Rc<dyn Any>;

    #[doc(hidden)]
    fn calculate_total_duration_nanos(&self) -> i64;
    #[doc(hidden)]
    fn is_at_target(&self) -> bool;
    /// The transition it is a child of (Compose's library-restricted `parentTransition`).
    #[doc(hidden)]
    fn parent(&self) -> Option<Rc<dyn AnyTransition>>;
    #[doc(hidden)]
    fn on_frame(&self, scaled_play_time_nanos: i64, scale_to_end: bool);
    #[doc(hidden)]
    fn on_transition_end(&self);
    #[doc(hidden)]
    fn seek_own_states(&self, play_time_nanos: i64);
    #[doc(hidden)]
    fn seek_animations(&self, play_time_nanos: i64);
    #[doc(hidden)]
    fn set_initial_animations(&self, state: &Rc<SeekingAnimationState>);
    #[doc(hidden)]
    fn reset_animation_fraction(&self, fraction: f32);
    #[doc(hidden)]
    fn clear_initial_animations(&self);
    #[doc(hidden)]
    fn update_initial_values(&self);
    #[doc(hidden)]
    fn id(&self) -> *const ();
}

/// An animation of a [`Transition`] of any types (Compose's `TransitionAnimationState<*, *>`): what
/// [`Transition::animations`] lists. Downcast with [`TransitionAnimationState::from_any`].
pub trait AnyTransitionAnimationState {
    /// Its label (tools show it).
    fn label(&self) -> String;
    /// How long its animation takes.
    fn duration_nanos(&self) -> i64;
    /// Itself as `Any`, to downcast.
    fn into_any_rc(self: Rc<Self>) -> Rc<dyn Any>;

    #[doc(hidden)]
    fn is_finished(&self) -> bool;
    #[doc(hidden)]
    fn on_play_time_changed(&self, play_time_nanos: i64, scale_to_end: bool);
    #[doc(hidden)]
    fn seek_to(&self, play_time_nanos: i64);
    #[doc(hidden)]
    fn reset_animation(&self);
    #[doc(hidden)]
    fn reset_animation_value(&self, fraction: f32);
    #[doc(hidden)]
    fn set_initial_value_animation(&self, state: &Rc<SeekingAnimationState>);
    #[doc(hidden)]
    fn clear_initial_animation(&self);
    #[doc(hidden)]
    fn update_initial_value(&self);
    #[doc(hidden)]
    fn has_initial_value_state(&self) -> bool;
    #[doc(hidden)]
    fn id(&self) -> *const ();
}

/// A list a snapshot state's reads stand for (Compose's `mutableStateListOf`, of objects that can't be states here):
/// read through `get`, its readers told of each change.
struct ObservedList<T: ?Sized> {
    items: RefCell<Vec<Rc<T>>>,
    version: MutableState<u64>,
}

impl<T: ?Sized> ObservedList<T> {
    fn new() -> ObservedList<T> {
        ObservedList { items: RefCell::new(Vec::new()), version: mutable_state_of(0) }
    }
    /// Its items, the read recorded.
    fn get(&self) -> Vec<Rc<T>> {
        let _ = self.version.get();
        self.items.borrow().clone()
    }
    fn add(&self, item: Rc<T>) {
        self.items.borrow_mut().push(item);
        self.version.set(self.version.peek().wrapping_add(1));
    }
    fn remove(&self, id: *const (), id_of: impl Fn(&T) -> *const ()) {
        let removed = {
            let mut items = self.items.borrow_mut();
            let before = items.len();
            if let Some(i) = items.iter().position(|it| id_of(it) == id) {
                items.remove(i);
            }
            before != items.len()
        };
        if removed {
            self.version.set(self.version.peek().wrapping_add(1));
        }
    }
}

// ---- Transition ---------------------------------------------------------------------------------------------------

pub(crate) struct TransitionInner<S> {
    transition_state: TransitionState<S>,
    parent_transition: Option<Weak<dyn AnyTransition>>,
    label: Option<String>,
    target_state: MutableState<S>,
    pending_target_state: MutableState<Option<S>>,
    segment: MutableState<Segment<S>>,
    play_time_nanos: MutableState<i64>,
    /// In real frame time for the root transition, scaled frame time for child transitions (as offset from the
    /// root's start).
    start_time_nanos: MutableState<i64>,
    update_children_needed: MutableState<bool>,
    animations: ObservedList<dyn AnyTransitionAnimationState>,
    transitions: ObservedList<dyn AnyTransition>,
    is_seeking: MutableState<bool>,
    last_seeked_time_nanos: Cell<i64>,
    total_duration_nanos: OnceCell<DerivedState<i64>>,
}

/// Animations run together from one state to another (Compose's Transition): made by [`update_transition`] or
/// [`remember_transition`]; its animations (`animate_float`, `animate_dp`, … `animate_value`, `animate_color`) each
/// map a state to a value and animate toward the target state's value along a spec picked by the
/// [`Segment`]; when the target changes they all set off toward its values. It is running until every animation (and
/// child transition) is done, then its current state becomes the target. Clones are the same transition.
pub struct Transition<S> {
    pub(crate) inner: Rc<TransitionInner<S>>,
}

impl<S> Clone for Transition<S> {
    fn clone(&self) -> Self {
        Transition { inner: self.inner.clone() }
    }
}

impl<S> PartialEq for Transition<S> {
    /// The same transition.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<S: AnimationValue + fmt::Debug> fmt::Debug for Transition<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Transition({:?}: {:?} → {:?})",
            self.inner.label,
            self.inner.transition_state.current_state(),
            self.inner.target_state.peek()
        )
    }
}

impl<S: AnimationValue> fmt::Display for Transition<S> {
    /// Compose's `toString`: its animations' values.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Transition animation values: ")?;
        for a in self.inner.animations.items.borrow().iter() {
            write!(f, "{}, ", a.label())?;
        }
        Ok(())
    }
}

impl<S: AnimationValue> Transition<S> {
    /// A transition of `transition_state` (Compose's TransitionInstance), a child of `parent_transition` if given.
    pub(crate) fn new(
        transition_state: TransitionState<S>,
        parent_transition: Option<Weak<dyn AnyTransition>>,
        label: Option<String>,
    ) -> Transition<S> {
        let current = transition_state.current_state();
        let inner = Rc::new(TransitionInner {
            transition_state,
            parent_transition,
            label,
            target_state: mutable_state_of(current.clone()),
            pending_target_state: mutable_state_of(None),
            segment: mutable_state_of(Segment::new(current.clone(), current)),
            play_time_nanos: mutable_state_of(0),
            start_time_nanos: mutable_state_of(AnimationConstants::UnspecifiedTime),
            update_children_needed: mutable_state_of(false),
            animations: ObservedList::new(),
            transitions: ObservedList::new(),
            is_seeking: mutable_state_of(false),
            last_seeked_time_nanos: Cell::new(0),
            total_duration_nanos: OnceCell::new(),
        });
        let weak = Rc::downgrade(&inner);
        let _ = inner
            .total_duration_nanos
            .set(derived_state_of(move || weak.upgrade().map(|i| i.calculate_total_duration_nanos()).unwrap_or(0)));
        inner.transition_state.transition_configured(&inner);
        Transition { inner }
    }

    /// The transition `any` is, if it is one of `S`s.
    pub fn from_any(any: Rc<dyn AnyTransition>) -> Option<Transition<S>> {
        any.into_any_rc().downcast::<TransitionInner<S>>().ok().map(|inner| Transition { inner })
    }

    /// Itself as a transition of any state type.
    pub fn as_any(&self) -> Rc<dyn AnyTransition> {
        self.inner.clone()
    }

    /// The state it is in: its initial state until it finishes, then its target (a state read).
    pub fn current_state(&self) -> S {
        self.inner.current_state()
    }

    /// Where it goes: what its animations read their targets from (a state read).
    pub fn target_state(&self) -> S {
        self.inner.target_state.get()
    }

    /// The state a deferred update waits to animate to (Compose's experimental `pendingTargetState`): none unless
    /// one is in progress (a state read).
    pub fn pending_target_state(&self) -> Option<S> {
        self.inner.pending_target_state.get()
    }

    /// Its initial and target states now (a state read).
    pub fn segment(&self) -> Segment<S> {
        self.inner.segment.get()
    }

    /// Its label (tools show it; a child's is its parent's and its own).
    pub fn label(&self) -> Option<String> {
        self.inner.label.clone()
    }

    /// Which transition it is: the same for its clones (what a key of it is).
    pub(crate) fn identity(&self) -> usize {
        Rc::as_ptr(&self.inner) as *const () as usize
    }

    /// The transition this one is a child of (Compose's library-restricted `parentTransition`).
    pub fn parent_transition(&self) -> Option<Rc<dyn AnyTransition>> {
        self.inner.parent_transition.as_ref().and_then(|p| p.upgrade())
    }

    /// Whether any of its animations runs (a state read).
    pub fn is_running(&self) -> bool {
        self.inner.is_running()
    }

    /// Its play time: from 0 at its start until its animations are done; a child's is its parent's (Compose's
    /// library-restricted `playTimeNanos`; a state read).
    pub fn play_time_nanos(&self) -> i64 {
        self.inner.play_time_nanos()
    }

    /// Sets its play time (a root's; a child's follows its parent's).
    pub fn set_play_time_nanos(&self, value: i64) {
        self.inner.set_play_time_nanos(value);
    }

    /// Whether tools seek it (Compose's library-restricted `isSeeking`; a state read).
    pub fn is_seeking(&self) -> bool {
        self.inner.is_seeking.get()
    }

    /// Its child transitions (Compose's `transitions`; a state read).
    pub fn transitions(&self) -> Vec<Rc<dyn AnyTransition>> {
        self.inner.transitions.get()
    }

    /// Its animations (Compose's `animations`; a state read).
    pub fn animations(&self) -> Vec<Rc<dyn AnyTransitionAnimationState>> {
        self.inner.animations.get()
    }

    /// Whether a seekable transition animates the initial values of an animation of it or of a child (Compose's
    /// `hasInitialValueAnimations`, internal-animation API).
    pub fn has_initial_value_animations(&self) -> bool {
        self.inner.has_initial_value_animations()
    }

    /// How long its animations and child transitions take (Compose's `totalDurationNanos`, a derived state): it
    /// changes as animations are added, best read after they are set up.
    pub fn total_duration_nanos(&self) -> i64 {
        self.inner.total_duration_nanos()
    }

    /// Puts it at `play_time_nanos` between `initial_state` and `target_state`, its animations and seeking children
    /// there too (Compose's tools-only `setPlaytimeAfterInitialAndTargetStateEstablished`, `seek` from Java): from
    /// then on it doesn't run on its own. New states take a composition before the animations follow.
    pub fn set_playtime_after_initial_and_target_state_established(
        &self,
        initial_state: S,
        target_state: S,
        play_time_nanos: i64,
    ) {
        self.inner.set_playtime_after_initial_and_target_state_established(
            initial_state,
            target_state,
            play_time_nanos,
        );
    }

    pub(crate) fn update_target(&self, target_state: S) {
        self.inner.update_target(target_state);
    }

    pub(crate) fn update_pending_target(&self, value: Option<S>) {
        self.inner.update_pending_target(value);
    }

    pub(crate) fn on_transition_end(&self) {
        self.inner.on_transition_end();
    }

    pub(crate) fn on_disposed(&self) {
        self.inner.on_transition_end();
        self.inner.transition_state.transition_removed();
    }

    pub(crate) fn seek_animations(&self, play_time_nanos: i64) {
        self.inner.seek_animations(play_time_nanos);
    }

    pub(crate) fn set_initial_animations(&self, state: &Rc<SeekingAnimationState>) {
        self.inner.set_initial_animations(state);
    }

    pub(crate) fn reset_animation_fraction(&self, fraction: f32) {
        self.inner.reset_animation_fraction(fraction);
    }

    pub(crate) fn clear_initial_animations(&self) {
        self.inner.clear_initial_animations();
    }

    pub(crate) fn update_initial_values(&self) {
        self.inner.update_initial_values();
    }

    pub(crate) fn last_seeked_time_nanos(&self) -> i64 {
        self.inner.last_seeked_time_nanos.get()
    }

    pub(crate) fn add_animation(&self, animation: Rc<dyn AnyTransitionAnimationState>) {
        self.inner.animations.add(animation);
    }

    pub(crate) fn remove_animation(&self, id: *const ()) {
        self.inner.animations.remove(id, |a| a.id());
    }

    /// Runs its frames while it is targeted elsewhere, running or its animations changed (Compose's composable
    /// `animateTo`, for a transition whose play time comes from the frame clock).
    #[track_caller]
    pub(crate) fn animate_to_target(&self, target_state: S) {
        let this = self.clone();
        group(move || {
            if !this.is_seeking() {
                this.update_target(target_state);
                // (target != current adds the effect in the frame the target changes)
                let t = this.clone();
                let run_frame_loop = remember_keyed(this.clone(), move || {
                    derived_state_of(move || {
                        t.target_state() != t.current_state() || t.is_running() || t.inner.update_children_needed.get()
                    })
                });
                if run_frame_loop.get() {
                    group(|| {
                        let coroutine_scope = kompoz_coroutines::remember_coroutine_scope();
                        let (scope, transition) = (coroutine_scope.clone(), this.clone());
                        kompozable_block!({
                            DisposableEffect((coroutine_scope, this.clone()), move || {
                                // (it runs at once until its first frame: the state is set up in this frame)
                                scope.launch(async move {
                                    let duration_scale = duration_scale();
                                    loop {
                                        let t = transition.clone();
                                        with_frame_nanos(move |frame_time_nanos| {
                                            // (seeking may have started between the composition and this frame)
                                            if !t.is_seeking() {
                                                t.inner.on_frame_time(
                                                    frame_time_nanos / AnimationDebugDurationScale,
                                                    duration_scale,
                                                );
                                            }
                                        })
                                        .await;
                                    }
                                });
                                on_dispose(|| {})
                            });
                        });
                    });
                }
            }
        });
    }
}

impl<S: AnimationValue> TransitionInner<S> {
    fn current_state(&self) -> S {
        self.transition_state.current_state()
    }

    fn is_running(&self) -> bool {
        self.start_time_nanos.get() != AnimationConstants::UnspecifiedTime
    }

    fn play_time_nanos(&self) -> i64 {
        match self.parent_transition.as_ref().and_then(|p| p.upgrade()) {
            Some(parent) => parent.play_time_nanos(),
            None => self.play_time_nanos.get(),
        }
    }

    fn set_play_time_nanos(&self, value: i64) {
        if self.parent_transition.is_none() {
            self.play_time_nanos.set(value);
        }
    }

    fn total_duration_nanos(&self) -> i64 {
        self.total_duration_nanos.get().map(|d| d.get()).unwrap_or(0)
    }

    fn calculate_total_duration_nanos(&self) -> i64 {
        let mut max_duration_nanos = 0;
        for a in self.animations.get() {
            max_duration_nanos = max_duration_nanos.max(a.duration_nanos());
        }
        for t in self.transitions.get() {
            max_duration_nanos = max_duration_nanos.max(t.calculate_total_duration_nanos());
        }
        max_duration_nanos
    }

    fn has_initial_value_animations(&self) -> bool {
        self.animations.get().iter().any(|a| a.has_initial_value_state())
            || self.transitions.get().iter().any(|t| t.has_initial_value_animations())
    }

    fn update_pending_target(&self, value: Option<S>) {
        let previous_pending = self.pending_target_state.get();
        let was_pending_cleared =
            previous_pending.is_some() && value.is_none() && self.target_state.get() == self.current_state();
        self.pending_target_state.set(value);
        if was_pending_cleared {
            let previous_pending = previous_pending.expect("a pending state");
            self.segment.set(Segment::new(previous_pending.clone(), self.target_state.get()));
            // (a deferred phase interrupted by a move back to where it was: from the pending state, so what was set by
            // hand meanwhile animates back; nothing set, it settles at once)
            self.transition_state.set_current_state(previous_pending);
            if !self.is_running() {
                self.update_children_needed.set(true);
            }
            for a in self.animations.items.borrow().clone() {
                a.reset_animation();
            }
        }
    }

    /// A frame of a root transition at `frame_time_nanos` (Compose's `onFrame(frameTimeNanos, durationScale)`).
    fn on_frame_time(&self, frame_time_nanos: i64, duration_scale: f32) {
        if self.start_time_nanos.peek() == AnimationConstants::UnspecifiedTime {
            self.on_transition_start(frame_time_nanos);
        }
        let delta_t = frame_time_nanos - self.start_time_nanos.peek();
        let scaled_play_time_nanos =
            if duration_scale == 0.0 { delta_t } else { round_to_long(delta_t as f64 / duration_scale as f64) };
        self.set_play_time_nanos(scaled_play_time_nanos);
        self.on_frame(scaled_play_time_nanos, duration_scale == 0.0);
    }

    fn on_frame(&self, scaled_play_time_nanos: i64, scale_to_end: bool) {
        if self.start_time_nanos.peek() == AnimationConstants::UnspecifiedTime {
            self.on_transition_start(scaled_play_time_nanos);
        } else if !self.transition_state.is_running() {
            self.transition_state.set_is_running(true);
        }
        self.update_children_needed.set(false);

        let mut all_finished = true;
        for a in self.animations.items.borrow().clone() {
            if !a.is_finished() {
                a.on_play_time_changed(scaled_play_time_nanos, scale_to_end);
            }
            if !a.is_finished() {
                all_finished = false;
            }
        }
        for t in self.transitions.items.borrow().clone() {
            if !t.is_at_target() {
                t.on_frame(scaled_play_time_nanos, scale_to_end);
            }
            if !t.is_at_target() {
                all_finished = false;
            }
        }
        if all_finished {
            self.on_transition_end();
        }
    }

    fn on_transition_start(&self, frame_time_nanos: i64) {
        self.start_time_nanos.set(frame_time_nanos);
        self.transition_state.set_is_running(true);
    }

    fn on_transition_end(&self) {
        self.start_time_nanos.set(AnimationConstants::UnspecifiedTime);
        if self.transition_state.sets_current_state() {
            self.transition_state.set_current_state(self.target_state.get());
        }
        self.set_play_time_nanos(0);
        self.transition_state.set_is_running(false);
        for t in self.transitions.items.borrow().clone() {
            t.on_transition_end();
        }
    }

    fn set_playtime_after_initial_and_target_state_established(
        &self,
        initial_state: S,
        target_state: S,
        play_time_nanos: i64,
    ) {
        self.start_time_nanos.set(AnimationConstants::UnspecifiedTime);
        self.transition_state.set_is_running(false);
        if !self.is_seeking.get() || self.current_state() != initial_state || self.target_state.get() != target_state {
            if self.current_state() != initial_state && self.transition_state.sets_current_state() {
                self.transition_state.set_current_state(initial_state.clone());
            }
            self.target_state.set(target_state.clone());
            self.is_seeking.set(true);
            self.segment.set(Segment::new(initial_state, target_state));
        }
        for t in self.transitions.items.borrow().clone() {
            if t.is_seeking() {
                t.seek_own_states(play_time_nanos);
            }
        }
        for a in self.animations.items.borrow().clone() {
            a.seek_to(play_time_nanos);
        }
        self.last_seeked_time_nanos.set(play_time_nanos);
    }

    /// Changes the target (Compose's `updateTarget`; written in composition, it may be rolled back with it).
    fn update_target(&self, target_state: S) {
        if self.target_state.get() != target_state {
            // (its child animations update their specs by the segment)
            self.segment.set(Segment::new(self.target_state.get(), target_state.clone()));
            if self.current_state() != self.target_state.get() {
                self.transition_state.set_current_state(self.target_state.get());
            }
            self.target_state.set(target_state);
            if !self.is_running() {
                self.update_children_needed.set(true);
            }
            // (each animation made again at the next frame toward its new target value)
            for a in self.animations.items.borrow().clone() {
                a.reset_animation();
            }
        }
    }

    fn seek_animations(&self, play_time_nanos: i64) {
        if self.start_time_nanos.peek() == AnimationConstants::UnspecifiedTime {
            self.start_time_nanos.set(play_time_nanos);
        }
        self.set_play_time_nanos(play_time_nanos);
        self.update_children_needed.set(false);
        for a in self.animations.items.borrow().clone() {
            a.seek_to(play_time_nanos);
        }
        for t in self.transitions.items.borrow().clone() {
            if !t.is_at_target() {
                t.seek_animations(play_time_nanos);
            }
        }
    }

    fn set_initial_animations(&self, state: &Rc<SeekingAnimationState>) {
        for a in self.animations.items.borrow().clone() {
            a.set_initial_value_animation(state);
        }
        for t in self.transitions.items.borrow().clone() {
            t.set_initial_animations(state);
        }
    }

    fn reset_animation_fraction(&self, fraction: f32) {
        for a in self.animations.items.borrow().clone() {
            a.reset_animation_value(fraction);
        }
        for t in self.transitions.items.borrow().clone() {
            t.reset_animation_fraction(fraction);
        }
    }

    fn clear_initial_animations(&self) {
        for a in self.animations.items.borrow().clone() {
            a.clear_initial_animation();
        }
        for t in self.transitions.items.borrow().clone() {
            t.clear_initial_animations();
        }
    }

    fn update_initial_values(&self) {
        for a in self.animations.items.borrow().clone() {
            a.update_initial_value();
        }
        for t in self.transitions.items.borrow().clone() {
            t.update_initial_values();
        }
    }

    fn on_child_animation_updated(&self) {
        self.update_children_needed.set(true);
        if self.is_seeking.get() {
            // (the total duration follows; the animations are put where the last seek was)
            for a in self.animations.items.borrow().clone() {
                a.seek_to(self.last_seeked_time_nanos.get());
            }
            self.update_children_needed.set(false);
        }
    }
}

impl<S: AnimationValue> AnyTransition for TransitionInner<S> {
    fn label(&self) -> Option<String> {
        self.label.clone()
    }
    fn is_running(&self) -> bool {
        TransitionInner::is_running(self)
    }
    fn is_seeking(&self) -> bool {
        self.is_seeking.get()
    }
    fn total_duration_nanos(&self) -> i64 {
        TransitionInner::total_duration_nanos(self)
    }
    fn play_time_nanos(&self) -> i64 {
        TransitionInner::play_time_nanos(self)
    }
    fn animations(&self) -> Vec<Rc<dyn AnyTransitionAnimationState>> {
        self.animations.get()
    }
    fn transitions(&self) -> Vec<Rc<dyn AnyTransition>> {
        self.transitions.get()
    }
    fn has_initial_value_animations(&self) -> bool {
        TransitionInner::has_initial_value_animations(self)
    }
    fn into_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
    fn calculate_total_duration_nanos(&self) -> i64 {
        TransitionInner::calculate_total_duration_nanos(self)
    }
    fn is_at_target(&self) -> bool {
        self.target_state.get() == self.current_state()
    }
    fn parent(&self) -> Option<Rc<dyn AnyTransition>> {
        self.parent_transition.as_ref().and_then(|p| p.upgrade())
    }
    fn on_frame(&self, scaled_play_time_nanos: i64, scale_to_end: bool) {
        TransitionInner::on_frame(self, scaled_play_time_nanos, scale_to_end);
    }
    fn on_transition_end(&self) {
        TransitionInner::on_transition_end(self);
    }
    fn seek_own_states(&self, play_time_nanos: i64) {
        let (initial, target) = (self.current_state(), self.target_state.get());
        self.set_playtime_after_initial_and_target_state_established(initial, target, play_time_nanos);
    }
    fn seek_animations(&self, play_time_nanos: i64) {
        TransitionInner::seek_animations(self, play_time_nanos);
    }
    fn set_initial_animations(&self, state: &Rc<SeekingAnimationState>) {
        TransitionInner::set_initial_animations(self, state);
    }
    fn reset_animation_fraction(&self, fraction: f32) {
        TransitionInner::reset_animation_fraction(self, fraction);
    }
    fn clear_initial_animations(&self) {
        TransitionInner::clear_initial_animations(self);
    }
    fn update_initial_values(&self) {
        TransitionInner::update_initial_values(self);
    }
    fn id(&self) -> *const () {
        self as *const TransitionInner<S> as *const ()
    }
}

// ---- A transition's animations --------------------------------------------------------------------------------------

struct AnimationInner<S, T, V: AnimationVector> {
    transition: Weak<TransitionInner<S>>,
    type_converter: TwoWayConverter<T, V>,
    label: String,
    /// Changed during composition (may be rolled back with it).
    target_value: MutableState<T>,
    animation_spec: MutableState<Arc<dyn FiniteAnimationSpec<T>>>,
    /// What animates it from the current state to the target state (Compose's `animation`, a state: its reads are
    /// the version's).
    animation: RefCell<Rc<TargetBasedAnimation<T, V>>>,
    animation_version: MutableState<u64>,
    initial_value_state: RefCell<Option<Rc<SeekingAnimationState>>>,
    initial_value_animation: RefCell<Option<Rc<TargetBasedAnimation<T, V>>>>,
    is_finished: MutableState<bool>,
    reset_snap_value: MutableState<f32>,
    /// The target state changed but not the target value: the initial value animation alone gives the value.
    use_only_initial_value: Cell<bool>,
    /// Changed during animation (no rollback to fear).
    value: MutableState<T>,
    velocity_vector: Cell<V>,
    duration_nanos: MutableState<i64>,
    is_seeking: Cell<bool>,
    interruption_spec: Arc<dyn FiniteAnimationSpec<T>>,
}

/// An animation of a [`Transition`] (Compose's `Transition.TransitionAnimationState`): what `animate_float`,
/// `animate_value`, … make; a state of its value. Clones are the same animation.
///
/// Kotlin: `Transition.TransitionAnimationState`
pub struct TransitionAnimationState<S, T, V: AnimationVector> {
    inner: Rc<AnimationInner<S, T, V>>,
}

impl<S, T, V: AnimationVector> Clone for TransitionAnimationState<S, T, V> {
    fn clone(&self) -> Self {
        TransitionAnimationState { inner: self.inner.clone() }
    }
}

impl<S, T, V: AnimationVector> PartialEq for TransitionAnimationState<S, T, V> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<S: AnimationValue, T: AnimationValue, V: AnimationVector> TransitionAnimationState<S, T, V> {
    fn new(
        transition: &Transition<S>,
        initial_value: T,
        initial_velocity_vector: V,
        type_converter: TwoWayConverter<T, V>,
        label: String,
    ) -> TransitionAnimationState<S, T, V> {
        let default_spring: Arc<dyn FiniteAnimationSpec<T>> = Arc::new(spring::<T>());
        let animation = Rc::new(TargetBasedAnimation::new(
            &*default_spring,
            type_converter.clone(),
            initial_value.clone(),
            initial_value.clone(),
            Some(initial_velocity_vector),
        ));
        let visibility_threshold = visibility_threshold_of(&type_converter).map(|t| {
            let mut vector = type_converter.convert_to_vector(initial_value.clone());
            for id in 0..V::SIZE {
                vector.set(id, t);
            }
            type_converter.convert_from_vector(vector)
        });
        let interruption_spec: Arc<dyn FiniteAnimationSpec<T>> =
            Arc::new(SpringSpec::new(spring::<T>().damping_ratio, spring::<T>().stiffness, visibility_threshold));
        let duration = animation.duration_nanos();
        TransitionAnimationState {
            inner: Rc::new(AnimationInner {
                transition: Rc::downgrade(&transition.inner),
                type_converter,
                label,
                target_value: mutable_state_of(initial_value.clone()),
                animation_spec: mutable_state_of(default_spring),
                animation: RefCell::new(animation),
                animation_version: mutable_state_of(0),
                initial_value_state: RefCell::new(None),
                initial_value_animation: RefCell::new(None),
                is_finished: mutable_state_of(true),
                reset_snap_value: mutable_state_of(NoReset),
                use_only_initial_value: Cell::new(false),
                value: mutable_state_of(initial_value),
                velocity_vector: Cell::new(initial_velocity_vector),
                duration_nanos: mutable_state_of(duration),
                is_seeking: Cell::new(false),
                interruption_spec,
            }),
        }
    }

    /// The animation `any` is, if it is one of these types.
    pub fn from_any(any: Rc<dyn AnyTransitionAnimationState>) -> Option<TransitionAnimationState<S, T, V>> {
        any.into_any_rc().downcast::<AnimationInner<S, T, V>>().ok().map(|inner| TransitionAnimationState { inner })
    }

    /// Its value (a state read).
    pub fn value(&self) -> T {
        self.inner.value.get()
    }

    /// Its value as a read-only state.
    pub fn as_state(&self) -> State<T> {
        State::from_source(self.clone())
    }

    pub fn type_converter(&self) -> &TwoWayConverter<T, V> {
        &self.inner.type_converter
    }

    /// Its label (tools show it).
    pub fn label(&self) -> String {
        self.inner.label.clone()
    }

    /// The spec of its current run (a state read): it changes as the target state does.
    pub fn animation_spec(&self) -> Arc<dyn FiniteAnimationSpec<T>> {
        self.inner.animation_spec.get()
    }

    /// What animates it from the current state to the target state: initial and target values, velocity, spec (a
    /// state read).
    pub fn animation(&self) -> Rc<TargetBasedAnimation<T, V>> {
        self.inner.animation()
    }
}

impl<S: AnimationValue, T: AnimationValue + fmt::Display, V: AnimationVector> fmt::Display
    for TransitionAnimationState<S, T, V>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "current value: {}, target: {}", self.inner.value.peek(), self.inner.target_value.peek())
    }
}

impl<S: AnimationValue, T: AnimationValue, V: AnimationVector> StateSource<T> for TransitionAnimationState<S, T, V> {
    fn with_value(&self, subscribe: bool, f: &mut dyn FnMut(&T)) {
        if subscribe { self.inner.value.with(|v| f(v)) } else { self.inner.value.peek_with(|v| f(v)) }
    }
    fn identity(&self) -> *const () {
        Rc::as_ptr(&self.inner) as *const ()
    }
}

impl<S: AnimationValue, T: AnimationValue, V: AnimationVector> AnimationInner<S, T, V> {
    fn transition(&self) -> Rc<TransitionInner<S>> {
        self.transition.upgrade().expect("the transition of an animation")
    }

    fn animation(&self) -> Rc<TargetBasedAnimation<T, V>> {
        let _ = self.animation_version.get();
        self.animation.borrow().clone()
    }

    fn set_animation(&self, animation: TargetBasedAnimation<T, V>) {
        *self.animation.borrow_mut() = Rc::new(animation);
        self.animation_version.set(self.animation_version.peek().wrapping_add(1));
    }

    fn new_animation(&self, spec: &(impl AnimationSpec<T> + ?Sized), initial: T, target: T, velocity: V) {
        self.set_animation(TargetBasedAnimation::new(
            spec,
            self.type_converter.clone(),
            initial,
            target,
            Some(velocity),
        ));
    }

    fn update_animation(&self, initial_value: T, is_interrupted: bool) {
        let initial_target = self.initial_value_animation.borrow().as_ref().map(|a| a.target_value());
        if initial_target == Some(self.target_value.get()) {
            // (the target value didn't change: the initial value animation takes care of it)
            let zero = self.velocity_vector.get().new_instance();
            self.new_animation(&*self.interruption_spec, initial_value.clone(), initial_value, zero);
            self.use_only_initial_value.set(true);
            self.duration_nanos.set(self.animation().duration_nanos());
            return;
        }
        let animation_spec = self.animation_spec.get();
        let spec_without_delay: Arc<dyn FiniteAnimationSpec<T>> = if is_interrupted && !self.is_seeking.get() {
            // (interrupted: the default spring, unless the spec is a spring too)
            if animation_spec.as_any().downcast_ref::<SpringSpec<T>>().is_some() {
                animation_spec
            } else {
                self.interruption_spec.clone()
            }
        } else {
            animation_spec
        };
        let play_time_nanos = self.transition().play_time_nanos();
        let target = self.target_value.get();
        let velocity = self.velocity_vector.get();
        if play_time_nanos <= 0 {
            self.new_animation(&*spec_without_delay, initial_value, target, velocity);
        } else {
            let spec: Arc<dyn AnimationSpec<T>> = spec_without_delay;
            self.new_animation(&delayed(spec, play_time_nanos), initial_value, target, velocity);
        }
        self.duration_nanos.set(self.animation().duration_nanos());
        self.use_only_initial_value.set(false);
        self.transition().on_child_animation_updated();
    }

    /// Called during composition.
    fn update_target_value(
        &self,
        target_value: T,
        animation_spec: Arc<dyn FiniteAnimationSpec<T>>,
        forced_initial_value: Option<T>,
        forced_initial_velocity: Option<V>,
    ) {
        let initial_target = self.initial_value_animation.borrow().as_ref().map(|a| a.target_value());
        if self.use_only_initial_value.get() && Some(&target_value) == initial_target.as_ref() {
            // (animating to the target value through the initial value already)
            return;
        }
        if self.target_value.get() == target_value
            && self.reset_snap_value.get() == NoReset
            && forced_initial_value.as_ref().is_none_or(|f| *f == self.animation().initial_value())
        {
            // (nothing changed: the animation goes on)
            return;
        }
        self.target_value.set(target_value.clone());
        self.animation_spec.set(animation_spec);
        let reset_snap_value = self.reset_snap_value.get();
        let initial_value = match &forced_initial_value {
            Some(v) => v.clone(),
            None if reset_snap_value == ResetAnimationSnap => target_value.clone(),
            None => self.value.get(),
        };
        if forced_initial_value.is_some() {
            self.value.set(initial_value.clone());
            if let Some(v) = forced_initial_velocity {
                self.velocity_vector.set(v);
            }
        }
        self.update_animation(initial_value, !self.is_finished.get());
        let reset_snap_value = self.reset_snap_value.get();
        self.is_finished.set(reset_snap_value == ResetAnimationSnap);
        // (the target may change in the middle of a transition)
        if reset_snap_value >= 0.0 {
            let animation = self.animation();
            let duration = animation.duration_nanos();
            self.value.set(animation.get_value_from_nanos((duration as f32 * reset_snap_value) as i64));
        } else if reset_snap_value == ResetAnimationSnap {
            self.value.set(target_value);
        }
        self.use_only_initial_value.set(false);
        self.reset_snap_value.set(NoReset);
    }

    /// Called during composition, seeking.
    fn update_initial_and_target_value(
        &self,
        initial_value: T,
        target_value: T,
        animation_spec: Arc<dyn FiniteAnimationSpec<T>>,
    ) {
        self.target_value.set(target_value.clone());
        self.animation_spec.set(animation_spec);
        let animation = self.animation();
        if animation.initial_value() == initial_value && animation.target_value() == target_value {
            return;
        }
        self.update_animation(initial_value, false);
    }
}

impl<S: AnimationValue, T: AnimationValue, V: AnimationVector> AnyTransitionAnimationState for AnimationInner<S, T, V> {
    fn label(&self) -> String {
        self.label.clone()
    }
    fn duration_nanos(&self) -> i64 {
        self.duration_nanos.get()
    }
    fn into_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
    fn is_finished(&self) -> bool {
        self.is_finished.get()
    }
    fn on_play_time_changed(&self, play_time_nanos: i64, scale_to_end: bool) {
        let animation = self.animation();
        let play_time = if scale_to_end { animation.duration_nanos() } else { play_time_nanos };
        self.value.set(animation.get_value_from_nanos(play_time));
        self.velocity_vector.set(animation.get_velocity_vector_from_nanos(play_time));
        if animation.is_finished_from_nanos(play_time) {
            self.is_finished.set(true);
        }
    }
    fn seek_to(&self, play_time_nanos: i64) {
        if self.reset_snap_value.get() != NoReset {
            return;
        }
        // (a SeekableTransitionState doesn't use the interrupted animation's spec)
        self.is_seeking.set(true);
        let animation = self.animation();
        if animation.target_value() == animation.initial_value() {
            // (an interrupted animation whose initial value changes, its target not: the initial value animation
            // alone changes the value)
            self.value.set(animation.target_value());
        } else {
            self.value.set(animation.get_value_from_nanos(play_time_nanos));
            self.velocity_vector.set(animation.get_velocity_vector_from_nanos(play_time_nanos));
        }
    }
    fn reset_animation(&self) {
        self.reset_snap_value.set(ResetNoSnap);
    }
    fn reset_animation_value(&self, fraction: f32) {
        if fraction == ResetAnimationSnapCurrent || fraction == ResetAnimationSnapTarget {
            let initial_animation = self.initial_value_animation.borrow().clone();
            let animation = self.animation();
            if let Some(init) = initial_animation {
                animation.set_initial_value(init.target_value());
                *self.initial_value_state.borrow_mut() = None;
                *self.initial_value_animation.borrow_mut() = None;
            }
            let animation_value = if fraction == ResetAnimationSnapCurrent {
                animation.initial_value()
            } else {
                animation.target_value()
            };
            animation.set_initial_value(animation_value.clone());
            animation.set_target_value(animation_value.clone());
            self.value.set(animation_value);
            self.duration_nanos.set(animation.duration_nanos());
        } else {
            self.reset_snap_value.set(fraction);
        }
    }
    fn set_initial_value_animation(&self, state: &Rc<SeekingAnimationState>) {
        let animation = self.animation();
        if animation.target_value() != animation.initial_value() {
            // (it goes on from where it is to its target)
            *self.initial_value_animation.borrow_mut() = Some(animation);
            *self.initial_value_state.borrow_mut() = Some(state.clone());
        }
        let value = self.value.get();
        let zero = self.velocity_vector.get().new_instance();
        self.new_animation(&*self.interruption_spec, value.clone(), value, zero);
        self.duration_nanos.set(self.animation().duration_nanos());
        self.use_only_initial_value.set(true);
    }
    fn clear_initial_animation(&self) {
        *self.initial_value_animation.borrow_mut() = None;
        *self.initial_value_state.borrow_mut() = None;
        self.use_only_initial_value.set(false);
    }
    fn update_initial_value(&self) {
        let Some(anim_state) = self.initial_value_state.borrow().clone() else { return };
        let Some(animation) = self.initial_value_animation.borrow().clone() else { return };
        // (in doubles: nanoseconds have more digits than a float keeps)
        let initial_play_time_nanos =
            round_to_long(anim_state.duration_nanos.get() as f64 * anim_state.value.get() as f64);
        let initial_value = animation.get_value_from_nanos(initial_play_time_nanos);
        let current = self.animation();
        if self.use_only_initial_value.get() {
            current.set_target_value(initial_value.clone());
        }
        current.set_initial_value(initial_value.clone());
        self.duration_nanos.set(current.duration_nanos());
        if self.reset_snap_value.get() == ResetNoSnap || self.use_only_initial_value.get() {
            self.value.set(initial_value);
        } else {
            self.seek_to(self.transition().play_time_nanos());
        }
        if initial_play_time_nanos >= anim_state.duration_nanos.get() {
            *self.initial_value_state.borrow_mut() = None;
            *self.initial_value_animation.borrow_mut() = None;
        } else {
            anim_state.is_complete.set(false);
        }
    }
    fn has_initial_value_state(&self) -> bool {
        self.initial_value_state.borrow().is_some()
    }
    fn id(&self) -> *const () {
        self as *const AnimationInner<S, T, V> as *const ()
    }
}

// ---- Deferred animations ------------------------------------------------------------------------------------------

/// A transition spec shared: `segment -> spec`.
type TransitionSpecFn<S, T> = Rc<dyn Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<T>>>;
/// A target value by state shared.
type TargetValueFn<S, T> = Rc<dyn Fn(&S) -> T>;

struct DeferredAnimationData<S, T, V: AnimationVector> {
    transition: Weak<TransitionInner<S>>,
    animation: TransitionAnimationState<S, T, V>,
    transition_spec: RefCell<TransitionSpecFn<S, T>>,
    target_value_by_state: RefCell<TargetValueFn<S, T>>,
}

impl<S: AnimationValue, T: AnimationValue, V: AnimationVector> DeferredAnimationData<S, T, V> {
    fn update_animation_states(
        &self,
        segment: &Segment<S>,
        forced_initial_value: Option<T>,
        forced_initial_velocity: Option<V>,
    ) {
        let target_by_state = self.target_value_by_state.borrow().clone();
        let transition_spec = self.transition_spec.borrow().clone();
        let target_value = target_by_state(&segment.target_state);
        let seeking = self.transition.upgrade().is_some_and(|t| t.is_seeking.get());
        if seeking {
            // (seeking: the initial value too)
            let initial_value = target_by_state(&segment.initial_state);
            self.animation.inner.update_initial_and_target_value(initial_value, target_value, transition_spec(segment));
        } else {
            self.animation.inner.update_target_value(
                target_value,
                transition_spec(segment),
                forced_initial_value,
                forced_initial_velocity,
            );
        }
    }
}

/// The state [`DeferredAnimation::animate`] gives: its value is the animation's, brought up to date with the
/// transition's segment as it is read.
struct DeferredAnimationValue<S, T, V: AnimationVector>(Rc<DeferredAnimationData<S, T, V>>);

impl<S: AnimationValue, T: AnimationValue, V: AnimationVector> StateSource<T> for DeferredAnimationValue<S, T, V> {
    fn with_value(&self, subscribe: bool, f: &mut dyn FnMut(&T)) {
        let update = || {
            if let Some(t) = self.0.transition.upgrade() {
                self.0.update_animation_states(&t.segment.get(), None, None);
            }
        };
        if subscribe {
            update();
            self.0.animation.inner.value.with(|v| f(v));
        } else {
            without_read_observation(update);
            self.0.animation.inner.value.peek_with(|v| f(v));
        }
    }
    fn identity(&self) -> *const () {
        Rc::as_ptr(&self.0) as *const ()
    }
}

struct DeferredAnimationInner<S, T, V: AnimationVector> {
    transition: Weak<TransitionInner<S>>,
    type_converter: TwoWayConverter<T, V>,
    label: String,
    /// Compose's `data`, a state: its reads are the version's.
    data: RefCell<Option<Rc<DeferredAnimationData<S, T, V>>>>,
    data_version: MutableState<u64>,
}

/// An animation of a [`Transition`] set up after composition (Compose's library-restricted
/// `Transition.DeferredAnimation`): its target values are known only later (a size, a position, as a layout
/// measures). Made by [`Transition::create_deferred_animation`]; [`animate`](Self::animate) sets it up and gives its
/// value as a state. Clones are the same animation.
///
/// Kotlin: `Transition.DeferredAnimation`
pub struct DeferredAnimation<S, T, V: AnimationVector> {
    inner: Rc<DeferredAnimationInner<S, T, V>>,
}

impl<S, T, V: AnimationVector> Clone for DeferredAnimation<S, T, V> {
    fn clone(&self) -> Self {
        DeferredAnimation { inner: self.inner.clone() }
    }
}

impl<S, T, V: AnimationVector> PartialEq for DeferredAnimation<S, T, V> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<S: AnimationValue, T: AnimationValue, V: AnimationVector> DeferredAnimation<S, T, V> {
    pub fn type_converter(&self) -> &TwoWayConverter<T, V> {
        &self.inner.type_converter
    }

    /// Its label (tools show it).
    pub fn label(&self) -> String {
        self.inner.label.clone()
    }

    /// The animation it set up, once [`animate`](Self::animate) did (a state read).
    pub fn animation(&self) -> Option<TransitionAnimationState<S, T, V>> {
        let _ = self.inner.data_version.get();
        self.inner.data.borrow().as_ref().map(|d| d.animation.clone())
    }

    fn data(&self) -> Option<Rc<DeferredAnimationData<S, T, V>>> {
        let _ = self.inner.data_version.get();
        self.inner.data.borrow().clone()
    }

    pub(crate) fn setup_seeking(&self) {
        if let Some(data) = self.data()
            && let Some(t) = self.inner.transition.upgrade()
        {
            let segment = t.segment.get();
            let target_by_state = data.target_value_by_state.borrow().clone();
            let transition_spec = data.transition_spec.borrow().clone();
            data.animation.inner.update_initial_and_target_value(
                target_by_state(&segment.initial_state),
                target_by_state(&segment.target_state),
                transition_spec(&segment),
            );
        }
    }
}

impl<S: AnimationValue, T: AnimationValue, V: AnimationVector> DeferredAnimation<S, T, V> {
    /// Sets the animation up (the first time) or updates it (Compose's `animate(transitionSpec,
    /// targetValueByState)`): `transition_spec` maps the transition's segment to a spec, `target_value_by_state` a
    /// state to a target value, as `Transition::animate_value`'s do. Its value as a state, brought up to date with the
    /// transition as it is read.
    pub fn animate(
        &self,
        transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<T>> + 'static,
        target_value_by_state: impl Fn(&S) -> T + 'static,
    ) -> State<T> {
        self.animate_with_forced_initial(transition_spec, None, None, target_value_by_state)
    }

    /// [`animate`](Self::animate) from a forced initial value and velocity, if given (Compose's `animate` overload
    /// taking them): a manual deferred phase's values handed over to the automatic transition.
    ///
    /// Kotlin: `animate(transitionSpec: Segment<S>.() -> FiniteAnimationSpec<T>, forcedInitialValue: T? = null, forcedInitialVelocity: V? = null, targetValueByState: (state: S) -> T)`
    pub fn animate_with_forced_initial(
        &self,
        transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<T>> + 'static,
        forced_initial_value: Option<T>,
        forced_initial_velocity: Option<V>,
        target_value_by_state: impl Fn(&S) -> T + 'static,
    ) -> State<T> {
        let transition = self.inner.transition.upgrade().expect("the transition of a deferred animation");
        let transition_spec: TransitionSpecFn<S, T> = Rc::new(transition_spec);
        let target_value_by_state: TargetValueFn<S, T> = Rc::new(target_value_by_state);
        let data = match self.data() {
            Some(d) => d,
            None => {
                let current = transition.current_state();
                let initial = target_value_by_state(&current);
                let t = Transition { inner: transition.clone() };
                let animation = TransitionAnimationState::new(
                    &t,
                    initial.clone(),
                    self.inner.type_converter.create_zero_vector_from(initial),
                    self.inner.type_converter.clone(),
                    self.inner.label.clone(),
                );
                let data = Rc::new(DeferredAnimationData {
                    transition: Rc::downgrade(&transition),
                    animation: animation.clone(),
                    transition_spec: RefCell::new(transition_spec.clone()),
                    target_value_by_state: RefCell::new(target_value_by_state.clone()),
                });
                *self.inner.data.borrow_mut() = Some(data.clone());
                self.inner.data_version.set(self.inner.data_version.peek().wrapping_add(1));
                t.add_animation(animation.inner.clone());
                data
            }
        };
        // (the latest mappings)
        *data.target_value_by_state.borrow_mut() = target_value_by_state;
        *data.transition_spec.borrow_mut() = transition_spec;
        data.update_animation_states(&transition.segment.get(), forced_initial_value, forced_initial_velocity);
        State::from_source(DeferredAnimationValue(data))
    }
}

// ---- Composables ------------------------------------------------------------------------------------------------

/// A [`Transition`] targeted at `target_state` (Compose's `updateTransition(targetState, label)`): when the target
/// changes, its animations run toward the new state's values. `label` tells it apart in tools.
#[kompozable]
pub fn update_transition<S: AnimationValue>(target_state: S, #[default(None)] label: Option<String>) -> Transition<S> {
    let initial = target_state.clone();
    let transition = kompoz_runtime::remember(move || {
        Transition::new(TransitionState::Mutable(MutableTransitionState::new(initial)), None, label)
    });
    transition.animate_to_target(target_state);
    let t = transition.clone();
    DisposableEffect(transition.clone(), move || {
        // (cleaned up on the way out: no observer stuck in between)
        on_dispose(move || t.on_disposed())
    });
    transition
}

/// A [`Transition`] of `transition_state` (Compose's `rememberTransition(transitionState, label)`), put in its
/// current state: it animates toward the state's target as it changes (a [`MutableTransitionState`]'s, a
/// [`DeferredTransitionState`]'s), or as a [`SeekableTransitionState`] seeks or animates it. Remember the state: a new
/// one makes a new transition (snapping to it). (A deferred one's [`DeferredTransition`]:
/// [`remember_deferred_transition`].)
///
/// Kotlin: `rememberTransition(transitionState: TransitionState<T>, label: String? = null)`
#[kompozable]
pub fn remember_transition<S: AnimationValue>(
    transition_state: impl Into<TransitionState<S>>,
    #[default(None)] label: Option<String>,
) -> Transition<S> {
    remember_transition_of(transition_state.into(), label)
}

/// A [`DeferredTransition`] of `transition_state` (Compose's experimental `rememberTransition(transitionState:
/// DeferredTransitionState<T>, label)`): see [`remember_transition`].
///
/// Kotlin: `rememberTransition(transitionState: DeferredTransitionState<T>, label: String? = null)`
#[kompozable]
pub fn remember_deferred_transition<S: AnimationValue>(
    transition_state: DeferredTransitionState<S>,
    #[default(None)] label: Option<String>,
) -> DeferredTransition<S> {
    DeferredTransition(remember_transition_of(TransitionState::Deferred(transition_state), label))
}

#[kompozable]
fn remember_transition_of<S: AnimationValue>(
    transition_state: TransitionState<S>,
    label: Option<String>,
) -> Transition<S> {
    let state = transition_state.clone();
    let transition = remember_keyed(transition_state.clone(), move || {
        // (remember observes what its calculation reads: not wanted here)
        without_read_observation(move || Transition::new(state, None, label))
    });
    if let TransitionState::Seekable(seekable) = &transition_state {
        let coroutine_scope = kompoz_coroutines::remember_coroutine_scope();
        let (s, scope) = (seekable.clone(), coroutine_scope.clone());
        DisposableEffect(coroutine_scope, move || {
            s.set_snapshot_state_observer(Some(crate::seekable::seekable_observer(&scope)));
            on_dispose(move || s.set_snapshot_state_observer(None))
        });
        let s = seekable.clone();
        kompoz_coroutines::LaunchedEffect((seekable.current_state(), seekable.target_state()), async move {
            s.observe_total_duration();
            s.on_composed();
        });
    } else {
        transition.animate_to_target(transition_state.target_state());
        if let TransitionState::Deferred(deferred) = &transition_state {
            transition.update_pending_target(deferred.pending_target_state());
        }
    }
    let t = transition.clone();
    DisposableEffect(transition.clone(), move || on_dispose(move || t.on_disposed()));
    transition
}

/// Compose's deprecated `updateTransition(transitionState: MutableTransitionState<T>, label)`: [`remember_transition`].
///
/// Kotlin: `updateTransition(transitionState: MutableTransitionState<T>, label: String? = null)`
#[kompozable]
pub fn update_transition_of_state<S: AnimationValue>(
    transition_state: MutableTransitionState<S>,
    #[default(None)] label: Option<String>,
) -> Transition<S> {
    remember_transition_of(TransitionState::Mutable(transition_state), label)
}

/// A [`Transition`] with a deferred phase (Compose's experimental DeferredTransition), made by
/// [`remember_transition`] of a [`DeferredTransitionState`]: while the state defers, the transition's
/// `pending_target_state` tells its components what is pending, the target staying. It is a [`Transition`].
pub struct DeferredTransition<S>(Transition<S>);

impl<S> Clone for DeferredTransition<S> {
    fn clone(&self) -> Self {
        DeferredTransition(self.0.clone())
    }
}

impl<S> PartialEq for DeferredTransition<S> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<S> std::ops::Deref for DeferredTransition<S> {
    type Target = Transition<S>;
    fn deref(&self) -> &Transition<S> {
        &self.0
    }
}

impl<S> DeferredTransition<S> {
    /// It as the transition it is.
    pub fn transition(&self) -> &Transition<S> {
        &self.0
    }
}

impl<S> From<DeferredTransition<S>> for Transition<S> {
    fn from(t: DeferredTransition<S>) -> Transition<S> {
        t.0
    }
}

/// A [`Transition`] animation's default transition spec: `spring()`.
fn default_spring_spec<S, T: AnimationValue>(_: &Segment<S>) -> Arc<dyn FiniteAnimationSpec<T>> {
    Arc::new(spring::<T>())
}

/// A [`Transition`] animation's default transition spec of a type with a visibility threshold: `spring(visibilityThreshold
/// = T.VisibilityThreshold)` (`IntOffset(1, 1)`, `IntSize(1, 1)`, `1` are those of IntOffset, IntSize, Int).
fn threshold_spring_spec<S, T: AnimationValue + VisibilityThreshold>(
    _: &Segment<S>,
) -> Arc<dyn FiniteAnimationSpec<T>> {
    let s = spring::<T>();
    Arc::new(SpringSpec::new(s.damping_ratio, s.stiffness, Some(T::VisibilityThreshold)))
}

/// The equality of a restartable scope's arguments.
fn args_equal<A: PartialEq>(a: &A, b: &A) -> bool {
    a == b
}

#[named]
impl<S: AnimationValue> Transition<S> {
    /// An animation of `T`s in this transition (Compose's `Transition.animateValue`): `target_value_by_state` maps a
    /// state to its value (composable: it may read states, locals), `transition_spec` the [`Segment`] to its spec
    /// (any finite one; a spring by default). Its value starts at the current state's (added while the transition
    /// runs, it joins in) and animates toward the target state's; a target value changing while the transition is at
    /// rest runs the transition again. Its value as a state.
    #[track_caller]
    pub fn animate_value<T: AnimationValue, V: AnimationVector>(
        &self,
        type_converter: TwoWayConverter<T, V>,
        #[default(default_spring_spec)] transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<T>>,
        #[default("ValueAnimation".to_string())] label: String,
        target_value_by_state: impl Fn(&S) -> T,
    ) -> State<T> {
        self.animate_value_of(type_converter, transition_spec, label, target_value_by_state)
    }

    /// A float animation in this transition (Compose's `Transition.animateFloat`): see [`animate_value`](Self::
    /// animate_value).
    #[track_caller]
    pub fn animate_float(
        &self,
        #[default(default_spring_spec)] transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<f32>>,
        #[default("FloatAnimation".to_string())] label: String,
        target_value_by_state: impl Fn(&S) -> f32,
    ) -> State<f32> {
        self.animate_value_of(f32::VectorConverter, transition_spec, label, target_value_by_state)
    }

    /// A [`Dp`] animation in this transition (Compose's `Transition.animateDp`): a spring of its visibility threshold
    /// by default.
    #[track_caller]
    pub fn animate_dp(
        &self,
        #[default(threshold_spring_spec)] transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<Dp>>,
        #[default("DpAnimation".to_string())] label: String,
        target_value_by_state: impl Fn(&S) -> Dp,
    ) -> State<Dp> {
        self.animate_value_of(Dp::VectorConverter, transition_spec, label, target_value_by_state)
    }

    /// An [`Offset`] animation in this transition (Compose's `Transition.animateOffset`).
    #[track_caller]
    pub fn animate_offset(
        &self,
        #[default(threshold_spring_spec)] transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<Offset>>,
        #[default("OffsetAnimation".to_string())] label: String,
        target_value_by_state: impl Fn(&S) -> Offset,
    ) -> State<Offset> {
        self.animate_value_of(Offset::VectorConverter, transition_spec, label, target_value_by_state)
    }

    /// A [`Size`] animation in this transition (Compose's `Transition.animateSize`).
    #[track_caller]
    pub fn animate_size(
        &self,
        #[default(threshold_spring_spec)] transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<Size>>,
        #[default("SizeAnimation".to_string())] label: String,
        target_value_by_state: impl Fn(&S) -> Size,
    ) -> State<Size> {
        self.animate_value_of(Size::VectorConverter, transition_spec, label, target_value_by_state)
    }

    /// An [`IntOffset`] animation in this transition (Compose's `Transition.animateIntOffset`).
    #[track_caller]
    pub fn animate_int_offset(
        &self,
        #[default(threshold_spring_spec)] transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<IntOffset>>,
        #[default("IntOffsetAnimation".to_string())] label: String,
        target_value_by_state: impl Fn(&S) -> IntOffset,
    ) -> State<IntOffset> {
        self.animate_value_of(IntOffset::VectorConverter, transition_spec, label, target_value_by_state)
    }

    /// An `Int` animation in this transition (Compose's `Transition.animateInt`).
    #[track_caller]
    pub fn animate_int(
        &self,
        #[default(threshold_spring_spec)] transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<i32>>,
        #[default("IntAnimation".to_string())] label: String,
        target_value_by_state: impl Fn(&S) -> i32,
    ) -> State<i32> {
        self.animate_value_of(i32::VectorConverter, transition_spec, label, target_value_by_state)
    }

    /// An [`IntSize`] animation in this transition (Compose's `Transition.animateIntSize`).
    #[track_caller]
    pub fn animate_int_size(
        &self,
        #[default(threshold_spring_spec)] transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<IntSize>>,
        #[default("IntSizeAnimation".to_string())] label: String,
        target_value_by_state: impl Fn(&S) -> IntSize,
    ) -> State<IntSize> {
        self.animate_value_of(IntSize::VectorConverter, transition_spec, label, target_value_by_state)
    }

    /// A [`Rect`] animation in this transition (Compose's `Transition.animateRect`).
    #[track_caller]
    pub fn animate_rect(
        &self,
        #[default(threshold_spring_spec)] transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<Rect>>,
        #[default("RectAnimation".to_string())] label: String,
        target_value_by_state: impl Fn(&S) -> Rect,
    ) -> State<Rect> {
        self.animate_value_of(Rect::VectorConverter, transition_spec, label, target_value_by_state)
    }

    /// A [`Color`] animation in this transition, in Oklab (Compose animation's `Transition.animateColor`): converted
    /// back into the target state's color's color space.
    #[track_caller]
    pub fn animate_color(
        &self,
        #[default(default_spring_spec)] transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<Color>>,
        #[default("ColorAnimation".to_string())] label: String,
        target_value_by_state: impl Fn(&S) -> Color,
    ) -> State<Color> {
        let this = self.clone();
        group(move || {
            let color_space = group(|| target_value_by_state(&this.target_state())).color_space();
            let type_converter: TwoWayConverter<Color, AnimationVector4D> =
                remember_keyed(color_space.id(), move || Color::VectorConverter(color_space));
            this.animate_value_of(type_converter, transition_spec, label, target_value_by_state)
        })
    }

    /// A child transition (Compose's experimental `Transition.createChildTransition`): its state is
    /// `transform_to_child_state` of this one's (composable), so this transition knows of its animations (it runs
    /// until they are done too), and what is below sees a simpler state.
    #[track_caller]
    pub fn create_child_transition<T: AnimationValue>(
        &self,
        #[default("ChildTransition".to_string())] label: String,
        transform_to_child_state: impl Fn(&S) -> T,
    ) -> Transition<T> {
        let this = self.clone();
        group(move || {
            let t = this.clone();
            let initial_parent_state = remember_keyed(this.clone(), move || t.current_state());
            let initial_state = group(|| {
                if this.is_seeking() {
                    transform_to_child_state(&this.current_state())
                } else {
                    transform_to_child_state(&initial_parent_state)
                }
            });
            let target_state = group(|| transform_to_child_state(&this.target_state()));
            let child_transition = this.create_child_transition_internal(initial_state, target_state, label);
            if !this.is_seeking() {
                let pending_target_state = this.pending_target_state().map(|p| group(|| transform_to_child_state(&p)));
                child_transition.update_pending_target(pending_target_state);
            }
            child_transition
        })
    }

    /// The child transition of [`create_child_transition`](Self::create_child_transition), of its initial and target
    /// states (Compose's published-internal `createChildTransitionInternal`).
    #[track_caller]
    pub fn create_child_transition_internal<T: AnimationValue>(
        &self,
        initial_state: T,
        target_state: T,
        child_label: String,
    ) -> Transition<T> {
        let this = self.clone();
        group(move || {
            let parent: Rc<dyn AnyTransition> = this.inner.clone();
            let parent = Rc::downgrade(&parent);
            let label = format!("{} > {}", this.inner.label.as_deref().unwrap_or("null"), child_label);
            let initial = initial_state.clone();
            let transition = remember_keyed(this.clone(), move || {
                Transition::new(
                    TransitionState::Mutable(MutableTransitionState::new(initial)),
                    Some(parent),
                    Some(label),
                )
            });
            let (p, child) = (this.clone(), transition.clone());
            kompozable_block!({
                DisposableEffect(transition.clone(), move || {
                    p.inner.transitions.add(child.inner.clone());
                    on_dispose(move || {
                        let id = Rc::as_ptr(&child.inner) as *const ();
                        p.inner.transitions.remove(id, |t| t.id());
                    })
                });
            });
            if this.is_seeking() {
                transition.set_playtime_after_initial_and_target_state_established(
                    initial_state,
                    target_state,
                    this.last_seeked_time_nanos(),
                );
            } else {
                transition.update_target(target_state);
                transition.inner.is_seeking.set(false);
            }
            transition
        })
    }

    /// An animation set up later (Compose's library-restricted `Transition.createDeferredAnimation`): it doesn't
    /// animate until [`DeferredAnimation::animate`] sets it up; then it animates from the current state to the target
    /// (none if the transition has arrived already).
    #[track_caller]
    pub fn create_deferred_animation<T: AnimationValue, V: AnimationVector>(
        &self,
        type_converter: TwoWayConverter<T, V>,
        #[default("DeferredAnimation".to_string())] label: String,
    ) -> DeferredAnimation<S, T, V> {
        let this = self.clone();
        group(move || {
            let weak = Rc::downgrade(&this.inner);
            let lazy_anim = remember_keyed(this.clone(), move || DeferredAnimation {
                inner: Rc::new(DeferredAnimationInner {
                    transition: weak,
                    type_converter,
                    label,
                    data: RefCell::new(None),
                    data_version: mutable_state_of(0),
                }),
            });
            let (t, anim) = (this.clone(), lazy_anim.clone());
            kompozable_block!({
                DisposableEffect(lazy_anim.clone(), move || {
                    on_dispose(move || {
                        if let Some(a) = anim.inner.data.borrow().as_ref() {
                            t.remove_animation(Rc::as_ptr(&a.animation.inner) as *const ());
                        }
                    })
                });
            });
            if this.is_seeking() {
                lazy_anim.setup_seeking();
            }
            lazy_anim
        })
    }
}

impl<S: AnimationValue> Transition<S> {
    /// [`animate_value`](Self::animate_value)'s body, every argument given.
    #[track_caller]
    fn animate_value_of<T: AnimationValue, V: AnimationVector>(
        &self,
        type_converter: TwoWayConverter<T, V>,
        transition_spec: impl Fn(&Segment<S>) -> Arc<dyn FiniteAnimationSpec<T>>,
        label: String,
        target_value_by_state: impl Fn(&S) -> T,
    ) -> State<T> {
        let this = self.clone();
        group(move || {
            let initial_state = if !this.is_seeking() {
                // (read once, as the animation is made)
                let t = this.clone();
                group(|| remember_keyed(this.clone(), move || without_read_observation(|| t.current_state())))
            } else {
                this.current_state()
            };
            let initial_value = group(|| target_value_by_state(&initial_state));
            // (through derived states: no recomposition when the value stays the same)
            let t = this.clone();
            let target_state = remember_keyed(this.clone(), move || derived_state_of(move || t.target_state()));
            let target_value = group(|| target_value_by_state(&target_state.get()));
            let t = this.clone();
            let segment = remember_keyed(this.clone(), move || derived_state_of(move || t.segment()));
            let animation_spec = group(|| transition_spec(&segment.get()));
            this.create_transition_animation(initial_value, target_value, animation_spec, type_converter, label)
        })
    }

    /// Compose's published-internal `createTransitionAnimation`: the animation remembered, its initial and target
    /// values updated in a scope of its own (Compose's `UpdateInitialAndTargetValues`: the reads of the animation's
    /// states there recompose that scope alone), added to the transition while it is composed.
    #[track_caller]
    pub fn create_transition_animation<T: AnimationValue, V: AnimationVector>(
        &self,
        initial_value: T,
        target_value: T,
        animation_spec: Arc<dyn FiniteAnimationSpec<T>>,
        type_converter: TwoWayConverter<T, V>,
        label: String,
    ) -> State<T> {
        let this = self.clone();
        group(move || {
            let (t, initial, target) = (this.clone(), initial_value.clone(), target_value.clone());
            let transition_animation = remember_keyed(this.clone(), move || {
                // (remember observes what its calculation reads: not wanted here)
                without_read_observation(move || {
                    // (from the initial state's value: added during a run, it joins in)
                    let zero = type_converter.create_zero_vector_from(target);
                    TransitionAnimationState::new(&t, initial, zero, type_converter, label)
                })
            });
            kompoz_runtime::restartable(
                (this.clone(), transition_animation.clone(), initial_value, target_value, animation_spec),
                Some(args_equal),
                |(transition, animation, initial_value, target_value, animation_spec)| {
                    if transition.is_seeking() {
                        animation.inner.update_initial_and_target_value(
                            initial_value.clone(),
                            target_value.clone(),
                            animation_spec.clone(),
                        );
                    } else {
                        animation.inner.update_target_value(target_value.clone(), animation_spec.clone(), None, None);
                    }
                },
            );
            let (t, anim) = (this.clone(), transition_animation.clone());
            kompozable_block!({
                DisposableEffect(transition_animation.clone(), move || {
                    t.add_animation(anim.inner.clone());
                    on_dispose(move || t.remove_animation(Rc::as_ptr(&anim.inner) as *const ()))
                });
            });
            transition_animation.as_state()
        })
    }
}

/// What panics when a transition state is given to another transition (Compose's checkPrecondition).
#[track_caller]
pub(crate) fn used_elsewhere(previous: &str, new: &str) -> ! {
    panic_kt(
        "IllegalStateException",
        &format!(
            "An instance of SeekableTransitionState has been used in different Transitions. Previous instance: \
             {previous}, new instance: {new}"
        ),
    )
}

//! An animation's state frame to frame (Compose's AnimationState.kt): its value (a state), velocity, last frame
//! time, when it finished, whether it runs; and the scope a running animation's frames see.

use crate::converter::{TwoWayConverter, VectorConverter};
use crate::spec::AnimationConstants;
use crate::vector::{AnimationVector, AnimationVector1D};
use kompoz_runtime::kt::KtToString;
use kompoz_runtime::{MutableState, SharedState, State, StateSource, mutable_state_of, named};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

/// The values an animation's state holds: compared to tell changes, shared with other threads' snapshots.
pub trait AnimationValue: Clone + PartialEq + Send + Sync + 'static {}
impl<T: Clone + PartialEq + Send + Sync + 'static> AnimationValue for T {}

struct AnimationStateInner<T, V> {
    type_converter: TwoWayConverter<T, V>,
    value: MutableState<T>,
    velocity_vector: Cell<V>,
    last_frame_time_nanos: Cell<i64>,
    finished_time_nanos: Cell<i64>,
    is_running: Cell<bool>,
}

/// An animation's state (Compose's AnimationState, a `State<T>`): its value (read in a composition, it recomposes
/// the reader as the animation runs), velocity, the time of its last frame, when it finished, and whether it runs.
/// [`animate_to`](AnimationState::animate_to) and [`animate_decay`](AnimationState::animate_decay) run animations
/// on it, each from where the last left it. Clones are the same state.
pub struct AnimationState<T, V>(Rc<AnimationStateInner<T, V>>);

impl<T, V> Clone for AnimationState<T, V> {
    fn clone(&self) -> Self {
        AnimationState(self.0.clone())
    }
}

impl<T: AnimationValue, V: AnimationVector> AnimationState<T, V> {
    /// Compose's `AnimationState(typeConverter, initialValue, initialVelocityVector = null, lastFrameTimeNanos =
    /// UnspecifiedTime, finishedTimeNanos = UnspecifiedTime, isRunning = false)` (a velocity of `None`: still).
    pub fn new(
        type_converter: TwoWayConverter<T, V>,
        initial_value: T,
        initial_velocity_vector: Option<V>,
        last_frame_time_nanos: i64,
        finished_time_nanos: i64,
        is_running: bool,
    ) -> AnimationState<T, V> {
        let velocity =
            initial_velocity_vector.unwrap_or_else(|| type_converter.create_zero_vector_from(initial_value.clone()));
        AnimationState(Rc::new(AnimationStateInner {
            type_converter,
            value: mutable_state_of(initial_value),
            velocity_vector: Cell::new(velocity),
            last_frame_time_nanos: Cell::new(last_frame_time_nanos),
            finished_time_nanos: Cell::new(finished_time_nanos),
            is_running: Cell::new(is_running),
        }))
    }

    /// A still state of `initial_value` (the rest of Compose's constructor's defaults).
    pub fn of(type_converter: TwoWayConverter<T, V>, initial_value: T) -> AnimationState<T, V> {
        AnimationState::new(
            type_converter,
            initial_value,
            None,
            AnimationConstants::UnspecifiedTime,
            AnimationConstants::UnspecifiedTime,
            false,
        )
    }

    /// A state moving at `initial_velocity`, a value (Compose's `AnimationState(typeConverter, initialValue,
    /// initialVelocity: T, …)`).
    pub fn with_velocity(
        type_converter: TwoWayConverter<T, V>,
        initial_value: T,
        initial_velocity: T,
        last_frame_time_nanos: i64,
        finished_time_nanos: i64,
        is_running: bool,
    ) -> AnimationState<T, V> {
        let v = type_converter.convert_to_vector(initial_velocity);
        AnimationState::new(
            type_converter,
            initial_value,
            Some(v),
            last_frame_time_nanos,
            finished_time_nanos,
            is_running,
        )
    }

    pub fn type_converter(&self) -> &TwoWayConverter<T, V> {
        &self.0.type_converter
    }

    /// Its value (a state read: it subscribes the composable reading it).
    pub fn value(&self) -> T {
        self.0.value.get()
    }

    /// Its value without subscribing.
    pub fn peek(&self) -> T {
        self.0.value.peek()
    }

    pub(crate) fn set_value(&self, value: T) {
        self.0.value.set(value);
    }

    /// Its velocity as a vector.
    pub fn velocity_vector(&self) -> V {
        self.0.velocity_vector.get()
    }

    pub(crate) fn set_velocity_vector(&self, v: V) {
        self.0.velocity_vector.set(v);
    }

    /// Its velocity as a value.
    pub fn velocity(&self) -> T {
        self.0.type_converter.convert_from_vector(self.velocity_vector())
    }

    /// When its last frame was (`AnimationConstants::UnspecifiedTime` before its first).
    pub fn last_frame_time_nanos(&self) -> i64 {
        self.0.last_frame_time_nanos.get()
    }

    pub(crate) fn set_last_frame_time_nanos(&self, t: i64) {
        self.0.last_frame_time_nanos.set(t);
    }

    /// When its animation finished (`UnspecifiedTime` unless it did).
    pub fn finished_time_nanos(&self) -> i64 {
        self.0.finished_time_nanos.get()
    }

    pub(crate) fn set_finished_time_nanos(&self, t: i64) {
        self.0.finished_time_nanos.set(t);
    }

    /// Whether an animation runs on it.
    pub fn is_running(&self) -> bool {
        self.0.is_running.get()
    }

    pub(crate) fn set_is_running(&self, running: bool) {
        self.0.is_running.set(running);
    }

    /// Whether its last animation finished (Compose's `isFinished`).
    pub fn is_finished(&self) -> bool {
        self.finished_time_nanos() != AnimationConstants::UnspecifiedTime
    }

    /// Its value as a read-only state (it is one in Compose).
    pub fn as_state(&self) -> State<T> {
        State::from_source(self.clone())
    }

    /// Its value as a read-only state shareable across threads (Compose's state of it, read by what must be
    /// `Send + Sync`: a shape drawn by it, say).
    pub fn as_shared_state(&self) -> SharedState<T> {
        self.0.value.as_shared_state()
    }
}

#[named]
impl<T: AnimationValue, V: AnimationVector> AnimationState<T, V> {
    /// A new state of these, the rest this one's (Compose's `AnimationState.copy`).
    pub fn copy(
        &self,
        #[default(self.peek())] value: T,
        #[default(Some(self.velocity_vector()))] velocity_vector: Option<V>,
        #[default(self.last_frame_time_nanos())] last_frame_time_nanos: i64,
        #[default(self.finished_time_nanos())] finished_time_nanos: i64,
        #[default(self.is_running())] is_running: bool,
    ) -> AnimationState<T, V> {
        AnimationState::new(
            self.0.type_converter.clone(),
            value,
            velocity_vector,
            last_frame_time_nanos,
            finished_time_nanos,
            is_running,
        )
    }
}

#[named]
impl AnimationState<f32, AnimationVector1D> {
    /// A new state of these, a float velocity, the rest this one's (Compose's `AnimationState<Float,
    /// AnimationVector1D>.copy(value, velocity, …)`).
    /// Kotlin: `copy(value: Float, velocity: Float, lastFrameTimeNanos: Long, finishedTimeNanos: Long, isRunning: Boolean)`
    pub fn copy_float(
        &self,
        #[default(self.peek())] value: f32,
        #[default(self.velocity_vector().value)] velocity: f32,
        #[default(self.last_frame_time_nanos())] last_frame_time_nanos: i64,
        #[default(self.finished_time_nanos())] finished_time_nanos: i64,
        #[default(self.is_running())] is_running: bool,
    ) -> AnimationState<f32, AnimationVector1D> {
        AnimationState::new(
            f32::VectorConverter,
            value,
            Some(AnimationVector1D(velocity)),
            last_frame_time_nanos,
            finished_time_nanos,
            is_running,
        )
    }
}

/// Compose's `AnimationState(initialValue: Float, initialVelocity = 0f, lastFrameTimeNanos = UnspecifiedTime,
/// finishedTimeNanos = UnspecifiedTime, isRunning = false)`: a float's.
/// Kotlin: `AnimationState(initialValue: Float, initialVelocity: Float, lastFrameTimeNanos: Long, finishedTimeNanos: Long, isRunning: Boolean)`
#[named]
pub fn float_animation_state(
    initial_value: f32,
    #[default(0.0)] initial_velocity: f32,
    #[default(AnimationConstants::UnspecifiedTime)] last_frame_time_nanos: i64,
    #[default(AnimationConstants::UnspecifiedTime)] finished_time_nanos: i64,
    #[default(false)] is_running: bool,
) -> AnimationState<f32, AnimationVector1D> {
    AnimationState::new(
        f32::VectorConverter,
        initial_value,
        Some(AnimationVector1D(initial_velocity)),
        last_frame_time_nanos,
        finished_time_nanos,
        is_running,
    )
}

/// Its value as a state's source: `State::from(state)`.
impl<T: AnimationValue, V: AnimationVector> StateSource<T> for AnimationState<T, V> {
    fn with_value(&self, subscribe: bool, f: &mut dyn FnMut(&T)) {
        if subscribe { self.0.value.with(|v| f(v)) } else { self.0.value.peek_with(|v| f(v)) }
    }
    fn identity(&self) -> *const () {
        Rc::as_ptr(&self.0) as *const ()
    }
}

impl<T: AnimationValue + KtToString, V: AnimationVector> fmt::Display for AnimationState<T, V>
where
    T: Clone,
{
    /// As Compose's `toString()`.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "AnimationState(value={}, velocity={}, isRunning={}, lastFrameTimeNanos={}, finishedTimeNanos={})",
            self.peek().kt_to_string(),
            self.velocity().kt_to_string(),
            self.is_running(),
            self.last_frame_time_nanos(),
            self.finished_time_nanos()
        )
    }
}

impl<T: AnimationValue + fmt::Debug, V: AnimationVector> fmt::Debug for AnimationState<T, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AnimationState")
            .field("value", &self.peek())
            .field("velocity_vector", &self.velocity_vector())
            .field("is_running", &self.is_running())
            .field("last_frame_time_nanos", &self.last_frame_time_nanos())
            .field("finished_time_nanos", &self.finished_time_nanos())
            .finish()
    }
}

/// What a running animation's frames see (Compose's AnimationScope, the receiver of `animate`'s block): its
/// value (a state) and velocity at this frame, its target, when it started and its last frame was, whether it
/// runs; [`cancel_animation`](AnimationScope::cancel_animation) stops it after this frame.
pub struct AnimationScope<T, V> {
    type_converter: TwoWayConverter<T, V>,
    target_value: T,
    start_time_nanos: i64,
    value: MutableState<T>,
    velocity_vector: Cell<V>,
    last_frame_time_nanos: Cell<i64>,
    finished_time_nanos: Cell<i64>,
    is_running: MutableState<bool>,
    on_cancel: Box<dyn Fn()>,
}

impl<T: AnimationValue, V: AnimationVector> AnimationScope<T, V> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        initial_value: T,
        type_converter: TwoWayConverter<T, V>,
        initial_velocity_vector: V,
        last_frame_time_nanos: i64,
        target_value: T,
        start_time_nanos: i64,
        is_running: bool,
        on_cancel: impl Fn() + 'static,
    ) -> AnimationScope<T, V> {
        AnimationScope {
            type_converter,
            target_value,
            start_time_nanos,
            value: mutable_state_of(initial_value),
            velocity_vector: Cell::new(initial_velocity_vector),
            last_frame_time_nanos: Cell::new(last_frame_time_nanos),
            finished_time_nanos: Cell::new(AnimationConstants::UnspecifiedTime),
            is_running: mutable_state_of(is_running),
            on_cancel: Box::new(on_cancel),
        }
    }

    pub fn type_converter(&self) -> &TwoWayConverter<T, V> {
        &self.type_converter
    }
    /// Where it is headed.
    pub fn target_value(&self) -> T {
        self.target_value.clone()
    }
    /// When it started.
    pub fn start_time_nanos(&self) -> i64 {
        self.start_time_nanos
    }
    /// Its value at this frame (a state read).
    pub fn value(&self) -> T {
        self.value.get()
    }
    pub(crate) fn peek(&self) -> T {
        self.value.peek()
    }
    pub(crate) fn set_value(&self, value: T) {
        self.value.set(value);
    }
    pub fn velocity_vector(&self) -> V {
        self.velocity_vector.get()
    }
    pub(crate) fn set_velocity_vector(&self, v: V) {
        self.velocity_vector.set(v);
    }
    /// Its velocity at this frame, as a value.
    pub fn velocity(&self) -> T {
        self.type_converter.convert_from_vector(self.velocity_vector())
    }
    pub fn last_frame_time_nanos(&self) -> i64 {
        self.last_frame_time_nanos.get()
    }
    pub(crate) fn set_last_frame_time_nanos(&self, t: i64) {
        self.last_frame_time_nanos.set(t);
    }
    pub fn finished_time_nanos(&self) -> i64 {
        self.finished_time_nanos.get()
    }
    pub(crate) fn set_finished_time_nanos(&self, t: i64) {
        self.finished_time_nanos.set(t);
    }
    /// Whether it runs (a state read).
    pub fn is_running(&self) -> bool {
        self.is_running.get()
    }
    pub(crate) fn peek_is_running(&self) -> bool {
        self.is_running.peek()
    }
    pub(crate) fn set_is_running(&self, running: bool) {
        self.is_running.set(running);
    }

    /// Stops the animation: this is its last frame (Compose's cancelAnimation; the animation ends normally, not
    /// cancelled).
    pub fn cancel_animation(&self) {
        self.is_running.set(false);
        (self.on_cancel)();
    }

    /// Its state now as an `AnimationState` (Compose's toAnimationState).
    pub fn to_animation_state(&self) -> AnimationState<T, V> {
        AnimationState::new(
            self.type_converter.clone(),
            self.peek(),
            Some(self.velocity_vector()),
            self.last_frame_time_nanos(),
            self.finished_time_nanos(),
            self.peek_is_running(),
        )
    }

    /// Copies this frame into `state` (Compose's internal updateState).
    pub(crate) fn update_state(&self, state: &AnimationState<T, V>) {
        state.set_value(self.peek());
        state.set_velocity_vector(self.velocity_vector());
        state.set_finished_time_nanos(self.finished_time_nanos());
        state.set_last_frame_time_nanos(self.last_frame_time_nanos());
        state.set_is_running(self.peek_is_running());
    }
}

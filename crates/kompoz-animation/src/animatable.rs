//! A value that animates (Compose's Animatable.kt, AnimationEndReason.kt).

use crate::animation::{Animation, DecayAnimation, TargetBasedAnimation};
use crate::converter::{TwoWayConverter, VectorConverter};
use crate::decay::DecayAnimationSpec;
use crate::mutator::MutatorMutex;
use crate::spec::{AnimationConstants, AnimationSpec, SpringSpec};
use crate::spring_simulation::Spring;
use crate::state::{AnimationState, AnimationValue};
use crate::suspend::animate_state;
use crate::vector::{AnimationVector, AnimationVector1D};
use kompoz_runtime::kt::{KtToString, panic_kt};
use kompoz_runtime::{
    KotlinProperties, MutableState, SharedState, State, mutable_confined_state_of, mutable_state_of, named,
};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

/// Why an animation ended (Compose's AnimationEndReason): it reached its target or rested, or it hit a bound.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AnimationEndReason {
    /// It hit a bound of its Animatable before its end: its value is the bound, its velocity what it had there.
    BoundReached,
    /// It ran to its end.
    Finished,
}

impl fmt::Display for AnimationEndReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// How an animation of an Animatable ended (Compose's AnimationResult): where and how fast (its end state), and
/// why.
#[derive(KotlinProperties)]
pub struct AnimationResult<T, V> {
    pub end_state: AnimationState<T, V>,
    pub end_reason: AnimationEndReason,
}

impl<T: AnimationValue + KtToString, V: AnimationVector> fmt::Display for AnimationResult<T, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AnimationResult(endReason={}, endState={})", self.end_reason, self.end_state)
    }
}

impl<T: AnimationValue + fmt::Debug, V: AnimationVector> fmt::Debug for AnimationResult<T, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AnimationResult")
            .field("end_reason", &self.end_reason)
            .field("end_state", &self.end_state)
            .finish()
    }
}

struct Inner<T, V> {
    type_converter: TwoWayConverter<T, V>,
    visibility_threshold: Option<T>,
    label: String,
    internal_state: AnimationState<T, V>,
    is_running: MutableState<bool>,
    target_value: MutableState<T>,
    lower_bound: RefCell<Option<T>>,
    upper_bound: RefCell<Option<T>>,
    negative_infinity_bounds: V,
    positive_infinity_bounds: V,
    lower_bound_vector: Cell<V>,
    upper_bound_vector: Cell<V>,
    mutator_mutex: MutatorMutex,
    default_spring_spec: Arc<SpringSpec<T>>,
}

/// A value that animates (Compose's Animatable): [`animate_to`](Animatable::animate_to) a target along a spec,
/// [`animate_decay`](Animatable::animate_decay) from a velocity, [`snap_to`](Animatable::snap_to) a value,
/// [`stop`](Animatable::stop); one at a time, a new one cancelling the running one (that one's coroutine is
/// cancelled where it awaits it, as Compose's throws CancellationException there), starting from its value and
/// velocity. Optional bounds stop an animation where they are hit. Its value, `is_running` and `target_value` are
/// states: a composable reading them recomposes as it animates. Clones are the same Animatable; `remember` one:
/// `remember(|| Animatable(0.0))`.
pub struct Animatable<T, V> {
    inner: Rc<Inner<T, V>>,
}

impl<T, V> Clone for Animatable<T, V> {
    fn clone(&self) -> Self {
        Animatable { inner: self.inner.clone() }
    }
}

impl<T, V> PartialEq for Animatable<T, V> {
    /// The same Animatable.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

fn infinity_bounds<V: AnimationVector>(value: f32) -> V {
    let mut v = V::default();
    for i in 0..V::SIZE {
        v.set(i, value);
    }
    v
}

impl<T: AnimationValue, V: AnimationVector> Animatable<T, V> {
    /// Compose's `Animatable(initialValue, typeConverter, visibilityThreshold = null, label = "Animatable")`: a
    /// default spring stops within `visibility_threshold` of its target (`None`: the converter's floats at 0.01).
    pub fn new(
        initial_value: T,
        type_converter: TwoWayConverter<T, V>,
        visibility_threshold: Option<T>,
        label: impl Into<String>,
    ) -> Animatable<T, V> {
        let negative_infinity_bounds = infinity_bounds::<V>(f32::NEG_INFINITY);
        let positive_infinity_bounds = infinity_bounds::<V>(f32::INFINITY);
        Animatable {
            inner: Rc::new(Inner {
                internal_state: AnimationState::of(type_converter.clone(), initial_value.clone()),
                default_spring_spec: Arc::new(SpringSpec::new(
                    Spring::DampingRatioNoBouncy,
                    Spring::StiffnessMedium,
                    visibility_threshold.clone(),
                )),
                type_converter,
                visibility_threshold,
                label: label.into(),
                is_running: mutable_state_of(false),
                target_value: mutable_confined_state_of(initial_value),
                lower_bound: RefCell::new(None),
                upper_bound: RefCell::new(None),
                negative_infinity_bounds,
                positive_infinity_bounds,
                lower_bound_vector: Cell::new(negative_infinity_bounds),
                upper_bound_vector: Cell::new(positive_infinity_bounds),
                mutator_mutex: MutatorMutex::new(),
            }),
        }
    }

    pub fn type_converter(&self) -> &TwoWayConverter<T, V> {
        &self.inner.type_converter
    }

    /// Its label (tools show it).
    pub fn label(&self) -> &str {
        &self.inner.label
    }

    /// The threshold it was made with.
    pub fn visibility_threshold(&self) -> Option<T> {
        self.inner.visibility_threshold.clone()
    }

    /// Its value (a state read).
    pub fn value(&self) -> T {
        self.inner.internal_state.value()
    }

    /// Its value without subscribing the reader.
    pub fn peek(&self) -> T {
        self.inner.internal_state.peek()
    }

    /// Its velocity as a vector (0 when no animation runs).
    pub fn velocity_vector(&self) -> V {
        self.inner.internal_state.velocity_vector()
    }

    /// Its velocity as a value.
    pub fn velocity(&self) -> T {
        self.inner.type_converter.convert_from_vector(self.velocity_vector())
    }

    /// Whether an animation runs (a state read).
    pub fn is_running(&self) -> bool {
        self.inner.is_running.get()
    }

    /// Where the running animation is headed, or its value (a state read).
    pub fn target_value(&self) -> T {
        self.inner.target_value.get()
    }

    /// Its lower bound, if any.
    pub fn lower_bound(&self) -> Option<T> {
        self.inner.lower_bound.borrow().clone()
    }

    /// Its upper bound, if any.
    pub fn upper_bound(&self) -> Option<T> {
        self.inner.upper_bound.borrow().clone()
    }

    /// Its value as a read-only state (Compose's asState).
    pub fn as_state(&self) -> State<T> {
        self.inner.internal_state.as_state()
    }

    /// Its value as a read-only state that is `Send + Sync` as the value is (Compose's `asState()`, held by what
    /// must be `Send + Sync`: a shape drawn by the animated value, say — Compose's shapes read an `Animatable` as
    /// they make their outline). Its value is read on the thread the animation runs on, as the animation's own.
    pub fn as_shared_state(&self) -> SharedState<T>
    where
        T: Send + Sync,
    {
        self.inner.internal_state.as_shared_state()
    }

    /// The spring it animates with by default: no bounce, medium stiffness, its visibility threshold.
    pub fn default_spring_spec(&self) -> Arc<dyn AnimationSpec<T>> {
        self.inner.default_spring_spec.clone()
    }

    fn clamp_to_bounds(&self, value: T) -> T {
        let (lower, upper) = (self.inner.lower_bound_vector.get(), self.inner.upper_bound_vector.get());
        if lower == self.inner.negative_infinity_bounds && upper == self.inner.positive_infinity_bounds {
            return value;
        }
        let mut value_vector = self.inner.type_converter.convert_to_vector(value.clone());
        let mut clamped = false;
        for i in 0..V::SIZE {
            let v = value_vector.get(i);
            if v < lower.get(i) || v > upper.get(i) {
                clamped = true;
                value_vector.set(i, kotlin_coerce_in(v, lower.get(i), upper.get(i)));
            }
        }
        if clamped { self.inner.type_converter.convert_from_vector(value_vector) } else { value }
    }

    /// Ends the animation's state: no velocity, no last frame, not running (Compose's endAnimation).
    fn end_animation(&self) {
        let state = &self.inner.internal_state;
        state.set_velocity_vector(state.velocity_vector().new_instance());
        state.set_last_frame_time_nanos(AnimationConstants::UnspecifiedTime);
        self.inner.is_running.set(false);
    }

    /// Runs `animation` as a mutation (Compose's runAnimation): from the last frame of the one it interrupts.
    fn run_animation(
        &self,
        animation: impl Animation<T, V> + 'static,
        initial_velocity: T,
        mut block: Option<Box<dyn for<'a> FnMut(&'a Animatable<T, V>)>>,
    ) -> impl Future<Output = AnimationResult<T, V>> + 'static {
        let this = self.clone();
        // (before the interrupted one's cancellation resets it)
        let start_time = this.inner.internal_state.last_frame_time_nanos();
        let mutex = this.inner.mutator_mutex.clone();
        let mutation = async move {
            /// Compose's `catch (e: CancellationException) { endAnimation(); throw e }`.
            struct EndOnCancel<T: AnimationValue, V: AnimationVector>(Option<Animatable<T, V>>);
            impl<T: AnimationValue, V: AnimationVector> Drop for EndOnCancel<T, V> {
                fn drop(&mut self) {
                    if let Some(a) = self.0.take()
                        && !std::thread::panicking()
                    {
                        a.end_animation();
                    }
                }
            }
            let mut guard = EndOnCancel(Some(this.clone()));
            let state = &this.inner.internal_state;
            state.set_velocity_vector(this.inner.type_converter.convert_to_vector(initial_velocity));
            this.inner.target_value.set(animation.target_value());
            this.inner.is_running.set(true);
            let end_state = named_copy_finished(state);
            let clamping_needed = Rc::new(Cell::new(false));
            let clamping = clamping_needed.clone();
            let (a, end) = (this.clone(), end_state.clone());
            animate_state(&end_state, &animation, start_time, move |scope| {
                scope.update_state(&a.inner.internal_state);
                let value = scope.value();
                let clamped = a.clamp_to_bounds(value.clone());
                if clamped != value {
                    a.inner.internal_state.set_value(clamped.clone());
                    end.set_value(clamped);
                    if let Some(block) = &mut block {
                        block(&a);
                    }
                    scope.cancel_animation();
                    clamping.set(true);
                } else if let Some(block) = &mut block {
                    block(&a);
                }
            })
            .await;
            let end_reason =
                if clamping_needed.get() { AnimationEndReason::BoundReached } else { AnimationEndReason::Finished };
            guard.0 = None;
            this.end_animation();
            AnimationResult { end_state, end_reason }
        };
        async move { mutex.mutate(mutation).await }
    }
}

/// `internalState.copy(finishedTimeNanos = UnspecifiedTime)`.
fn named_copy_finished<T: AnimationValue, V: AnimationVector>(state: &AnimationState<T, V>) -> AnimationState<T, V> {
    AnimationState::new(
        state.type_converter().clone(),
        state.peek(),
        Some(state.velocity_vector()),
        state.last_frame_time_nanos(),
        AnimationConstants::UnspecifiedTime,
        state.is_running(),
    )
}

/// Kotlin's coerceIn of floats (an empty range throws).
#[track_caller]
fn kotlin_coerce_in(v: f32, min: f32, max: f32) -> f32 {
    if min > max {
        panic_kt(
            "IllegalArgumentException",
            &format!("Cannot coerce value to an empty range: maximum {max} is less than minimum {min}."),
        );
    }
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

#[named]
impl<T: AnimationValue, V: AnimationVector> Animatable<T, V> {
    /// Sets its bounds (Compose's updateBounds; `None`: unbounded that way; each by default as it is): a lower one
    /// above the upper one in any dimension panics. With no animation running, its value is moved within them.
    #[track_caller]
    pub fn update_bounds(
        &self,
        #[default(self.lower_bound())] lower_bound: Option<T>,
        #[default(self.upper_bound())] upper_bound: Option<T>,
    ) {
        let converter = &self.inner.type_converter;
        let lower_bound_vector =
            lower_bound.clone().map(|b| converter.convert_to_vector(b)).unwrap_or(self.inner.negative_infinity_bounds);
        let upper_bound_vector =
            upper_bound.clone().map(|b| converter.convert_to_vector(b)).unwrap_or(self.inner.positive_infinity_bounds);
        for i in 0..V::SIZE {
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            if !(lower_bound_vector.get(i) <= upper_bound_vector.get(i)) {
                panic_kt(
                    "IllegalStateException",
                    &format!(
                        "Lower bound must be no greater than upper bound on *all* dimensions. The provided lower \
                         bound: {lower_bound_vector} is greater than upper bound {upper_bound_vector} on index {i}"
                    ),
                );
            }
        }
        self.inner.lower_bound_vector.set(lower_bound_vector);
        self.inner.upper_bound_vector.set(upper_bound_vector);
        *self.inner.upper_bound.borrow_mut() = upper_bound;
        *self.inner.lower_bound.borrow_mut() = lower_bound;
        if !self.inner.is_running.peek() {
            let value = self.peek();
            let clamped_value = self.clamp_to_bounds(value.clone());
            if clamped_value != value {
                self.inner.internal_state.set_value(clamped_value);
            }
        }
    }

    /// Animates to `target_value` along `animation_spec` (its default spring unless given), from its value and
    /// `initial_velocity` (its velocity unless given), `block` called with it each frame (Compose's animateTo).
    /// Done when the animation is: the result says where it ended and why. A running animation is cancelled (its
    /// coroutine), this one starting from its value and velocity; cancelled itself, it ends there (no velocity).
    pub fn animate_to(
        &self,
        target_value: T,
        #[default(self.default_spring_spec())] animation_spec: Arc<dyn AnimationSpec<T>>,
        #[default(self.velocity())] initial_velocity: T,
        #[default(None)] block: Option<Box<dyn for<'a> FnMut(&'a Animatable<T, V>)>>,
    ) -> impl Future<Output = AnimationResult<T, V>> + 'static {
        let anim = TargetBasedAnimation::with_velocity(
            &*animation_spec,
            self.inner.type_converter.clone(),
            self.peek(),
            target_value,
            initial_velocity.clone(),
        );
        self.run_animation(anim, initial_velocity, block)
    }

    /// Flings it from its value at `initial_velocity`, slowing down along `animation_spec` (Compose's
    /// animateDecay); a bound hit ends it there.
    pub fn animate_decay(
        &self,
        initial_velocity: T,
        animation_spec: impl DecayAnimationSpec<T>,
        #[default(None)] block: Option<Box<dyn for<'a> FnMut(&'a Animatable<T, V>)>>,
    ) -> impl Future<Output = AnimationResult<T, V>> + 'static {
        let anim = DecayAnimation::new(
            &animation_spec,
            self.inner.type_converter.clone(),
            self.peek(),
            self.inner.type_converter.convert_to_vector(initial_velocity.clone()),
        );
        self.run_animation(anim, initial_velocity, block)
    }
}

impl<T: AnimationValue, V: AnimationVector> Animatable<T, V> {
    /// Sets its value at once (within its bounds), the running animation cancelled (Compose's snapTo).
    pub fn snap_to(&self, target_value: T) -> impl Future<Output = ()> + 'static {
        let this = self.clone();
        let mutex = this.inner.mutator_mutex.clone();
        let mutation = async move {
            this.end_animation();
            let clamped_value = this.clamp_to_bounds(target_value);
            this.inner.internal_state.set_value(clamped_value.clone());
            this.inner.target_value.set(clamped_value);
        };
        async move { mutex.mutate(mutation).await }
    }

    /// Stops the running animation where it is (Compose's stop): cancelled, its velocity gone.
    pub fn stop(&self) -> impl Future<Output = ()> + 'static {
        let this = self.clone();
        let mutex = this.inner.mutator_mutex.clone();
        let mutation = async move {
            this.end_animation();
        };
        async move { mutex.mutate(mutation).await }
    }
}

impl<T: AnimationValue + fmt::Debug, V: AnimationVector> fmt::Debug for Animatable<T, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Animatable")
            .field("label", &self.inner.label)
            .field("value", &self.peek())
            .field("is_running", &self.inner.is_running.peek())
            .finish()
    }
}

/// Compose's `Animatable(…)`, its overloads by their arguments: `Animatable(0.0)` (a float, the visibility
/// threshold 0.01), `Animatable((0.0, 0.5))` (and a threshold), `Animatable((value, converter))`, `Animatable((value,
/// converter, threshold))`, `Animatable((value, converter, threshold, label))`, `Animatable(color)`.
pub fn Animatable<A: AnimatableArgs>(args: A) -> A::Animatable {
    args.animatable()
}

/// What [`Animatable()`](fn@Animatable) is given: Kotlin's arguments, one as itself, several as a tuple.
pub trait AnimatableArgs {
    type Animatable;
    fn animatable(self) -> Self::Animatable;
}

/// Kotlin: `Animatable(initialValue: Float)`
impl AnimatableArgs for f32 {
    type Animatable = Animatable<f32, AnimationVector1D>;
    fn animatable(self) -> Self::Animatable {
        Animatable::new(self, f32::VectorConverter, Some(Spring::DefaultDisplacementThreshold), "Animatable")
    }
}
/// Kotlin: `Animatable(initialValue: Float, visibilityThreshold: Float)`
impl AnimatableArgs for (f32, f32) {
    type Animatable = Animatable<f32, AnimationVector1D>;
    fn animatable(self) -> Self::Animatable {
        Animatable::new(self.0, f32::VectorConverter, Some(self.1), "Animatable")
    }
}
/// Kotlin: `Animatable(initialValue: T, typeConverter: TwoWayConverter<T, V>)`
impl<T: AnimationValue, V: AnimationVector> AnimatableArgs for (T, TwoWayConverter<T, V>) {
    type Animatable = Animatable<T, V>;
    fn animatable(self) -> Self::Animatable {
        Animatable::new(self.0, self.1, None, "Animatable")
    }
}
/// Kotlin: `Animatable(initialValue: T, typeConverter: TwoWayConverter<T, V>, visibilityThreshold: T?)`
impl<T: AnimationValue, V: AnimationVector> AnimatableArgs for (T, TwoWayConverter<T, V>, Option<T>) {
    type Animatable = Animatable<T, V>;
    fn animatable(self) -> Self::Animatable {
        Animatable::new(self.0, self.1, self.2, "Animatable")
    }
}
/// Kotlin: `Animatable(initialValue: T, typeConverter: TwoWayConverter<T, V>, visibilityThreshold: T?, label: String)`
impl<T: AnimationValue, V: AnimationVector> AnimatableArgs for (T, TwoWayConverter<T, V>, Option<T>, &str) {
    type Animatable = Animatable<T, V>;
    fn animatable(self) -> Self::Animatable {
        Animatable::new(self.0, self.1, self.2, self.3)
    }
}
/// Kotlin: `Animatable(initialValue: T, typeConverter: TwoWayConverter<T, V>, visibilityThreshold: T?, label: String)`
impl<T: AnimationValue, V: AnimationVector> AnimatableArgs for (T, TwoWayConverter<T, V>, Option<T>, String) {
    type Animatable = Animatable<T, V>;
    fn animatable(self) -> Self::Animatable {
        Animatable::new(self.0, self.1, self.2, self.3)
    }
}

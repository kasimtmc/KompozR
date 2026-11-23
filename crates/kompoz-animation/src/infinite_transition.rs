//! Animations that never end (Compose's animation-core InfiniteTransition.kt, and animation's
//! `InfiniteTransition.animateColor`): [`remember_infinite_transition`] and the values it runs from as they enter the
//! composition until they leave it, frame by frame through the coroutine context's
//! [`InfiniteAnimationPolicy`](kompoz_ui::InfiniteAnimationPolicy) (a test's cancels them while its clock advances
//! on its own).

use crate::animation::{Animation, TargetBasedAnimation};
use crate::color::ColorVectorConverter;
use crate::converter::{TwoWayConverter, VectorConverter};
use crate::spec::{AnimationConstants, AnimationSpec, InfiniteRepeatableSpec};
use crate::state::AnimationValue;
use crate::suspend::{duration_scale, with_infinite_animation_frame_nanos};
use crate::vector::{AnimationVector, AnimationVector4D};
use kompoz_coroutines::{LaunchedEffect, snapshot_flow};
use kompoz_graphics::Color;
use kompoz_runtime::{
    DisposableEffect, MutableState, SideEffect, State, StateSource, group, kompozable, kompozable_block,
    mutable_confined_state_of, mutable_state_of, named, on_dispose, remember,
};
use kompoz_ui::MotionDurationScale;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// An [`InfiniteTransition`] (Compose's `rememberInfiniteTransition(label)`): its animations (`animate_float`,
/// `animate_color`, `animate_value`) start as they enter the composition and run until they leave it.
#[kompozable]
pub fn remember_infinite_transition(#[default("InfiniteTransition".to_string())] label: String) -> InfiniteTransition {
    let infinite_transition = remember(move || InfiniteTransition::new(label));
    infinite_transition.run();
    infinite_transition
}

/// An animation of an [`InfiniteTransition`] of any types (Compose's `InfiniteTransition.TransitionAnimationState<*, *>`).
pub trait AnyInfiniteTransitionAnimationState {
    /// Its label (tools show it).
    fn label(&self) -> String;
    #[doc(hidden)]
    fn is_finished(&self) -> bool;
    #[doc(hidden)]
    fn on_play_time_changed(&self, play_time_nanos: i64);
    #[doc(hidden)]
    fn skip_to_end(&self);
    #[doc(hidden)]
    fn reset(&self);
    #[doc(hidden)]
    fn id(&self) -> *const ();
}

struct Inner {
    label: String,
    animations: RefCell<Vec<Rc<dyn AnyInfiniteTransitionAnimationState>>>,
    refresh_child_needed: MutableState<bool>,
    start_time_nanos: Cell<i64>,
    is_running: MutableState<bool>,
}

/// Runs animations that never end (Compose's InfiniteTransition): made by [`remember_infinite_transition`]; each
/// animation (`animate_float`, `animate_color`, `animate_value`) runs from its initial value to its target along an
/// infinitely repeated spec as long as it is composed. Clones are the same transition.
#[derive(Clone)]
pub struct InfiniteTransition {
    inner: Rc<Inner>,
}

impl PartialEq for InfiniteTransition {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for InfiniteTransition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InfiniteTransition({}, {} animations)", self.inner.label, self.inner.animations.borrow().len())
    }
}

impl InfiniteTransition {
    pub(crate) fn new(label: String) -> InfiniteTransition {
        InfiniteTransition {
            inner: Rc::new(Inner {
                label,
                animations: RefCell::new(Vec::new()),
                refresh_child_needed: mutable_state_of(false),
                start_time_nanos: Cell::new(AnimationConstants::UnspecifiedTime),
                is_running: mutable_state_of(true),
            }),
        }
    }

    /// Its label (tools show it).
    pub fn label(&self) -> String {
        self.inner.label.clone()
    }

    /// Its animations.
    pub fn animations(&self) -> Vec<Rc<dyn AnyInfiniteTransitionAnimationState>> {
        self.inner.animations.borrow().clone()
    }

    fn add_animation(&self, animation: Rc<dyn AnyInfiniteTransitionAnimationState>) {
        self.inner.animations.borrow_mut().push(animation);
        self.inner.refresh_child_needed.set(true);
    }

    fn remove_animation(&self, id: *const ()) {
        self.inner.animations.borrow_mut().retain(|a| a.id() != id);
    }

    /// Its frames, while an animation runs or one changed (Compose's composable `run`): restarted when the duration
    /// scale changes, the animations ended at once with a scale of 0 (waiting for another then).
    #[track_caller]
    fn run(&self) {
        let this = self.clone();
        group(move || {
            if this.inner.is_running.get() || this.inner.refresh_child_needed.get() {
                let t = this.clone();
                kompozable_block!({
                    LaunchedEffect(this.clone(), async move {
                        // (restarted each time the duration scale changes)
                        let duration_scale_now = Rc::new(Cell::new(1.0f32));
                        loop {
                            let (tt, scale) = (t.clone(), duration_scale_now.clone());
                            with_infinite_animation_frame_nanos(move |frame_time_nanos| {
                                let inner = &tt.inner;
                                let current = duration_scale();
                                if inner.start_time_nanos.get() == AnimationConstants::UnspecifiedTime
                                    || scale.get() != current
                                {
                                    inner.start_time_nanos.set(frame_time_nanos);
                                    for a in inner.animations.borrow().clone() {
                                        a.reset();
                                    }
                                    scale.set(current);
                                }
                                if scale.get() == 0.0 {
                                    // (done at once)
                                    for a in inner.animations.borrow().clone() {
                                        a.skip_to_end();
                                    }
                                } else {
                                    let play_time_nanos =
                                        ((frame_time_nanos - inner.start_time_nanos.get()) as f32 / scale.get()) as i64;
                                    tt.on_frame(play_time_nanos);
                                }
                            })
                            .await;
                            // (suspended until the duration scale isn't 0)
                            if duration_scale_now.get() == 0.0 {
                                let context = kompoz_coroutines::current_coroutine_context();
                                let positive = snapshot_flow(move || {
                                    context.get::<MotionDurationScale>().map(|s| s.scale_factor()).unwrap_or(1.0)
                                })
                                .filter(|s| *s > 0.0)
                                .take(1);
                                positive.collect(|_| {}).await;
                            }
                        }
                    });
                });
            }
        });
    }

    fn on_frame(&self, play_time_nanos: i64) {
        let mut all_finished = true;
        for a in self.inner.animations.borrow().clone() {
            if !a.is_finished() {
                a.on_play_time_changed(play_time_nanos);
            }
            if !a.is_finished() {
                all_finished = false;
            }
        }
        self.inner.is_running.set(!all_finished);
    }
}

struct AnimationInner<T, V: AnimationVector> {
    transition: Weak<Inner>,
    initial_value: RefCell<T>,
    target_value: RefCell<T>,
    type_converter: TwoWayConverter<T, V>,
    label: String,
    value: MutableState<T>,
    animation_spec: RefCell<Arc<dyn AnimationSpec<T>>>,
    animation: RefCell<Rc<TargetBasedAnimation<T, V>>>,
    /// (a hint to the transition in a normal run)
    is_finished: Cell<bool>,
    /// Refreshed during a run: the new animation starts at the next frame.
    start_on_the_next_frame: Cell<bool>,
    /// A changed animation starts from play time 0 again: the transition's play time offset by this.
    play_time_nanos_offset: Cell<i64>,
}

/// An animation of an [`InfiniteTransition`] (Compose's `InfiniteTransition.TransitionAnimationState`): a state of
/// its value. Clones are the same animation.
///
/// Kotlin: `InfiniteTransition.TransitionAnimationState`
pub struct InfiniteTransitionAnimationState<T, V: AnimationVector> {
    inner: Rc<AnimationInner<T, V>>,
}

impl<T, V: AnimationVector> Clone for InfiniteTransitionAnimationState<T, V> {
    fn clone(&self) -> Self {
        InfiniteTransitionAnimationState { inner: self.inner.clone() }
    }
}

impl<T, V: AnimationVector> PartialEq for InfiniteTransitionAnimationState<T, V> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: AnimationValue, V: AnimationVector> InfiniteTransitionAnimationState<T, V> {
    fn new(
        transition: &InfiniteTransition,
        initial_value: T,
        target_value: T,
        type_converter: TwoWayConverter<T, V>,
        animation_spec: Arc<dyn AnimationSpec<T>>,
        label: String,
    ) -> InfiniteTransitionAnimationState<T, V> {
        let animation = Rc::new(TargetBasedAnimation::new(
            &*animation_spec,
            type_converter.clone(),
            initial_value.clone(),
            target_value.clone(),
            None,
        ));
        InfiniteTransitionAnimationState {
            inner: Rc::new(AnimationInner {
                transition: Rc::downgrade(&transition.inner),
                value: mutable_confined_state_of(initial_value.clone()),
                initial_value: RefCell::new(initial_value),
                target_value: RefCell::new(target_value),
                type_converter,
                label,
                animation_spec: RefCell::new(animation_spec),
                animation: RefCell::new(animation),
                is_finished: Cell::new(false),
                start_on_the_next_frame: Cell::new(false),
                play_time_nanos_offset: Cell::new(0),
            }),
        }
    }

    /// Its value (a state read).
    pub fn value(&self) -> T {
        self.inner.value.get()
    }

    pub fn type_converter(&self) -> &TwoWayConverter<T, V> {
        &self.inner.type_converter
    }

    /// The spec of its run.
    pub fn animation_spec(&self) -> Arc<dyn AnimationSpec<T>> {
        self.inner.animation_spec.borrow().clone()
    }

    /// What animates it from its initial value to its target.
    pub fn animation(&self) -> Rc<TargetBasedAnimation<T, V>> {
        self.inner.animation.borrow().clone()
    }

    /// Its initial and target values changed (rarely): a new animation, started at the next frame (no continuity).
    fn update_values(&self, initial_value: T, target_value: T, animation_spec: Arc<dyn AnimationSpec<T>>) {
        let inner = &self.inner;
        *inner.initial_value.borrow_mut() = initial_value.clone();
        *inner.target_value.borrow_mut() = target_value.clone();
        *inner.animation.borrow_mut() = Rc::new(TargetBasedAnimation::new(
            &*animation_spec,
            inner.type_converter.clone(),
            initial_value,
            target_value,
            None,
        ));
        *inner.animation_spec.borrow_mut() = animation_spec;
        if let Some(t) = inner.transition.upgrade() {
            t.refresh_child_needed.set(true);
        }
        inner.is_finished.set(false);
        inner.start_on_the_next_frame.set(true);
    }
}

impl<T: AnimationValue, V: AnimationVector> AnyInfiniteTransitionAnimationState for AnimationInner<T, V> {
    fn label(&self) -> String {
        self.label.clone()
    }
    fn is_finished(&self) -> bool {
        self.is_finished.get()
    }
    fn on_play_time_changed(&self, play_time_nanos: i64) {
        if let Some(t) = self.transition.upgrade() {
            t.refresh_child_needed.set(false);
        }
        if self.start_on_the_next_frame.get() {
            self.start_on_the_next_frame.set(false);
            self.play_time_nanos_offset.set(play_time_nanos);
        }
        let play_time = play_time_nanos - self.play_time_nanos_offset.get();
        let animation = self.animation.borrow().clone();
        self.value.set(animation.get_value_from_nanos(play_time));
        self.is_finished.set(animation.is_finished_from_nanos(play_time));
    }
    fn skip_to_end(&self) {
        self.value.set(self.animation.borrow().target_value());
        self.start_on_the_next_frame.set(true);
    }
    fn reset(&self) {
        self.start_on_the_next_frame.set(true);
    }
    fn id(&self) -> *const () {
        self as *const AnimationInner<T, V> as *const ()
    }
}

impl<T: AnimationValue, V: AnimationVector> StateSource<T> for InfiniteTransitionAnimationState<T, V> {
    fn with_value(&self, subscribe: bool, f: &mut dyn FnMut(&T)) {
        if subscribe { self.inner.value.with(|v| f(v)) } else { self.inner.value.peek_with(|v| f(v)) }
    }
    fn identity(&self) -> *const () {
        Rc::as_ptr(&self.inner) as *const ()
    }
}

impl InfiniteTransition {
    /// [`animate_value`](Self::animate_value)'s body, every argument given.
    #[track_caller]
    fn animate_value_of<T: AnimationValue, V: AnimationVector>(
        &self,
        initial_value: T,
        target_value: T,
        type_converter: TwoWayConverter<T, V>,
        animation_spec: InfiniteRepeatableSpec<T>,
        label: String,
    ) -> State<T> {
        let this = self.clone();
        let animation_spec: Arc<dyn AnimationSpec<T>> = Arc::new(animation_spec);
        group(move || {
            let (t, initial, target, spec) =
                (this.clone(), initial_value.clone(), target_value.clone(), animation_spec.clone());
            let transition_animation = remember(move || {
                InfiniteTransitionAnimationState::new(&t, initial, target, type_converter, spec, label)
            });
            let animation = transition_animation.clone();
            kompozable_block!({
                SideEffect(move || {
                    let changed = initial_value != *animation.inner.initial_value.borrow()
                        || target_value != *animation.inner.target_value.borrow();
                    if changed {
                        animation.update_values(initial_value, target_value, animation_spec);
                    }
                });
            });
            let (t, animation) = (this.clone(), transition_animation.clone());
            kompozable_block!({
                DisposableEffect(transition_animation.clone(), move || {
                    t.add_animation(animation.inner.clone());
                    on_dispose(move || t.remove_animation(Rc::as_ptr(&animation.inner) as *const ()))
                });
            });
            State::from_source(transition_animation)
        })
    }
}

#[named]
impl InfiniteTransition {
    /// A value animated from `initial_value` to `target_value` and again, forever, along `animation_spec` (its
    /// repeat mode: from the start each time, or back and forth) as part of this transition (Compose's
    /// `InfiniteTransition.animateValue`): new initial or target values restart it (no continuity). A state of it.
    #[track_caller]
    pub fn animate_value<T: AnimationValue, V: AnimationVector>(
        &self,
        initial_value: T,
        target_value: T,
        type_converter: TwoWayConverter<T, V>,
        animation_spec: InfiniteRepeatableSpec<T>,
        #[default("ValueAnimation".to_string())] label: String,
    ) -> State<T> {
        self.animate_value_of(initial_value, target_value, type_converter, animation_spec, label)
    }

    /// A float animated forever (Compose's `InfiniteTransition.animateFloat`): see [`animate_value`](Self::
    /// animate_value).
    #[track_caller]
    pub fn animate_float(
        &self,
        initial_value: f32,
        target_value: f32,
        animation_spec: InfiniteRepeatableSpec<f32>,
        #[default("FloatAnimation".to_string())] label: String,
    ) -> State<f32> {
        self.animate_value_of(initial_value, target_value, f32::VectorConverter, animation_spec, label)
    }

    /// A color animated forever, in Oklab (Compose animation's `InfiniteTransition.animateColor`): converted back
    /// into the target's color space.
    #[track_caller]
    pub fn animate_color(
        &self,
        initial_value: Color,
        target_value: Color,
        animation_spec: InfiniteRepeatableSpec<Color>,
        #[default("ColorAnimation".to_string())] label: String,
    ) -> State<Color> {
        let this = self.clone();
        group(move || {
            let color_space = target_value.color_space();
            let converter: TwoWayConverter<Color, AnimationVector4D> =
                remember(move || Color::VectorConverter(color_space));
            this.animate_value_of(initial_value, target_value, converter, animation_spec, label)
        })
    }
}

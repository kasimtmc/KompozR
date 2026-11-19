//! A shared element's bounds animation, as Compose's internal `BoundsAnimation.kt`: an animation of a [`Rect`] set
//! up later on a `Transition<Boolean>` (the shared element's visibility: its child transition of the
//! AnimatedVisibilityScope's, or one of its own for a visibility the caller manages), from where it is shown to its
//! target, along the spec its bounds transform gives when it starts.

use crate::bounds::BoundsTransform;
use crate::shared_transition::{Observed, SharedTransitionScopeImpl};
use crate::transition::{DeferredAnimation, Transition};
use crate::{AnimationVector4D, FiniteAnimationSpec, Spring, VisibilityThreshold, spring};
use kompoz_graphics::{Offset, Rect};
use kompoz_runtime::{State, named_args};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// The deferred animation of a shared element's bounds (Compose's `Transition<Boolean>.DeferredAnimation<Rect,
/// AnimationVector4D>`).
pub(crate) type BoundsDeferredAnimation = DeferredAnimation<bool, Rect, AnimationVector4D>;

/// The spring bounds animate along until a bounds transform gives a spec (Compose's private
/// `DefaultBoundsAnimation`): medium-low stiff, there within a pixel each way.
fn default_bounds_animation() -> Arc<dyn FiniteAnimationSpec<Rect>> {
    Arc::new(named_args!(spring(
        stiffness = Spring::StiffnessMediumLow,
        visibility_threshold = Some(Rect::VisibilityThreshold)
    )))
}

/// Compose's internal BoundsAnimation: the shared element's bounds animated along its visibility transition while
/// the scope's transition is active.
pub(crate) struct BoundsAnimation {
    transition_scope: Weak<SharedTransitionScopeImpl>,
    pub(crate) transition: Transition<bool>,
    /// Compose's `animation`, a state.
    animation: Observed<BoundsDeferredAnimation>,
    /// Compose's `boundsTransform`, a state.
    bounds_transform: Observed<Rc<dyn BoundsTransform>>,
    /// The offset a fling's momentum adds (the shared element's).
    momentum_offset: Rc<dyn Fn() -> Offset>,
    /// The spec the animation goes along (Compose's `animationSpec`): its bounds transform's, asked as it starts.
    animation_spec: RefCell<Arc<dyn FiniteAnimationSpec<Rect>>>,
    /// The animation's value once set up (Compose's `animationState`, a state: who read it unset is told when it is
    /// set).
    animation_state: Observed<Option<State<Rect>>>,
    current_bounds_for_lambda: Cell<Option<Rect>>,
    target_bounds_for_lambda: Cell<Option<Rect>>,
    weak_self: Weak<BoundsAnimation>,
}

impl BoundsAnimation {
    pub(crate) fn new(
        transition_scope: &Rc<SharedTransitionScopeImpl>,
        transition: Transition<bool>,
        animation: BoundsDeferredAnimation,
        bounds_transform: Rc<dyn BoundsTransform>,
        momentum_offset: Rc<dyn Fn() -> Offset>,
    ) -> Rc<BoundsAnimation> {
        Rc::new_cyclic(|weak_self| BoundsAnimation {
            transition_scope: Rc::downgrade(transition_scope),
            transition,
            animation: Observed::new(animation),
            bounds_transform: Observed::new(bounds_transform),
            momentum_offset,
            animation_spec: RefCell::new(default_bounds_animation()),
            animation_state: Observed::new(None),
            current_bounds_for_lambda: Cell::new(None),
            target_bounds_for_lambda: Cell::new(None),
            weak_self: weak_self.clone(),
        })
    }

    fn is_transition_active(&self) -> bool {
        self.transition_scope.upgrade().is_some_and(|s| s.is_transition_active())
    }

    /// Takes a new deferred animation (made again as the scope's transition starts or ends) and bounds transform
    /// (Compose's `updateAnimation`): another animation starts over from the default spec.
    pub(crate) fn update_animation(
        &self,
        animation: BoundsDeferredAnimation,
        bounds_transform: Rc<dyn BoundsTransform>,
    ) {
        if self.animation.peek() != animation {
            self.animation.set(animation);
            self.animation_state.set(None);
            *self.animation_spec.borrow_mut() = default_bounds_animation();
        }
        self.bounds_transform.set(bounds_transform);
    }

    /// Whether the transitions it is part of are on their way (Compose's `isRunning`): its root transition's
    /// current state isn't its target.
    pub(crate) fn is_running(&self) -> bool {
        let mut parent = self.transition.as_any();
        while let Some(p) = parent.parent() {
            parent = p;
        }
        !parent.is_at_target()
    }

    /// The spec it goes along (Compose's `animationSpec`).
    pub(crate) fn animation_spec(&self) -> Arc<dyn FiniteAnimationSpec<Rect>> {
        self.animation_spec.borrow().clone()
    }

    /// Whether it was set up (Compose's `animationState != null`; a state read).
    pub(crate) fn has_animation_state(&self) -> bool {
        self.animation_state.get().is_some()
    }

    /// Its value while the scope's transition is active (Compose's `value`), moved by a fling's momentum; none
    /// otherwise or before it is set up.
    pub(crate) fn value(&self) -> Option<Rect> {
        if !self.is_transition_active() {
            return None;
        }
        self.animation_state.get().map(|state| {
            let value = state.get();
            let offset = (self.momentum_offset)();
            if offset != Offset::Zero { value.translate(offset) } else { value }
        })
    }

    /// Animates from `current_bounds` to `target_bounds` while the scope's transition is active (Compose's
    /// `animate`): the bounds transform (or `forced_bounds_transform`) gives the spec as it is set up (it doesn't take
    /// part in an interruption's animation); a manual deferred phase hands over its value and velocity.
    pub(crate) fn animate(
        &self,
        current_bounds: Rect,
        target_bounds: Rect,
        forced_bounds_transform: Option<Rc<dyn BoundsTransform>>,
        forced_initial_value: Option<Rect>,
        forced_initial_velocity: Option<AnimationVector4D>,
    ) {
        if !self.is_transition_active() {
            return;
        }
        self.current_bounds_for_lambda.set(Some(current_bounds));
        self.target_bounds_for_lambda.set(Some(target_bounds));
        if self.animation_state.get().is_none() {
            let transform = forced_bounds_transform.unwrap_or_else(|| self.bounds_transform.get());
            *self.animation_spec.borrow_mut() = transform.create_animation_spec(current_bounds, target_bounds);
        }
        let (spec_of, value_of) = (self.weak_self.clone(), self.weak_self.clone());
        let state = self.animation.get().animate_with_forced_initial(
            move |_| spec_of.upgrade().map_or_else(default_bounds_animation, |b| b.animation_spec()),
            forced_initial_value,
            forced_initial_velocity,
            move |state: &bool| {
                let this = value_of.upgrade().expect("a bounds animation's own lambda");
                if *state == this.transition.target_state() {
                    this.target_bounds_for_lambda.get().expect("its target bounds")
                } else {
                    this.current_bounds_for_lambda.get().expect("its current bounds")
                }
            },
        );
        self.animation_state.set(Some(state));
    }

    /// Whether its shared element is becoming visible (Compose's `target`): its transition's target state.
    pub(crate) fn target(&self) -> bool {
        self.transition.target_state()
    }
}

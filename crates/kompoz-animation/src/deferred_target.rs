//! `DeferredTargetAnimation`, as Compose's `androidx.compose.animation.core.DeferredTargetAnimation`: an animation
//! whose target isn't known when it is made — a size or a position set up in composition or as a modifier node is
//! made, known only as the layout measures and places (an approach layout's lookahead size).

use crate::{Animatable, AnimationSpec, AnimationValue, AnimationVector, FiniteAnimationSpec, TwoWayConverter, spring};
use kompoz_coroutines::CoroutineScope;
use kompoz_runtime::{MutableState, mutable_confined_state_of, named, named_args};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// An animation to a target known later (Compose's DeferredTargetAnimation): each
/// [`update_target`](Self::update_target) — from a measure, a placement, an approach node — sets the animation up the
/// first time (at the target, as it is) and animates to a new target after, in the given scope, giving the value
/// now. Clones are the same animation; `remember` one, or keep it in a node.
pub struct DeferredTargetAnimation<T, V> {
    inner: Rc<Inner<T, V>>,
}

struct Inner<T, V> {
    vector_converter: TwoWayConverter<T, V>,
    /// The target of the last `update_target` (Compose's `_pendingTarget`, a state).
    pending_target: MutableState<Option<T>>,
    animatable: RefCell<Option<Animatable<T, V>>>,
}

/// Compose's `DeferredTargetAnimation(vectorConverter)`: [`DeferredTargetAnimation::new`].
///
/// Kotlin: `DeferredTargetAnimation(vectorConverter: TwoWayConverter<T, V>)`
pub fn DeferredTargetAnimation<T: AnimationValue, V: AnimationVector>(
    vector_converter: TwoWayConverter<T, V>,
) -> DeferredTargetAnimation<T, V> {
    DeferredTargetAnimation::new(vector_converter)
}

impl<T, V> Clone for DeferredTargetAnimation<T, V> {
    fn clone(&self) -> Self {
        DeferredTargetAnimation { inner: self.inner.clone() }
    }
}

impl<T, V> PartialEq for DeferredTargetAnimation<T, V> {
    /// The same animation.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: AnimationValue, V: AnimationVector> DeferredTargetAnimation<T, V> {
    /// An animation of values `vector_converter` converts (Compose's `DeferredTargetAnimation(vectorConverter)`):
    /// `DeferredTargetAnimation::new(Size::VectorConverter)`.
    pub fn new(vector_converter: TwoWayConverter<T, V>) -> DeferredTargetAnimation<T, V> {
        DeferredTargetAnimation {
            inner: Rc::new(Inner {
                vector_converter,
                pending_target: mutable_confined_state_of(None),
                animatable: RefCell::new(None),
            }),
        }
    }

    /// The target of the last [`update_target`](Self::update_target) (Compose's `pendingTarget`), none before the
    /// first. Read in composition, layout or drawing, it is observed.
    pub fn pending_target(&self) -> Option<T> {
        self.inner.pending_target.get()
    }

    /// The target of the animation (Compose's private `target`).
    fn target(&self) -> Option<T> {
        self.inner.animatable.borrow().as_ref().map(|a| a.target_value())
    }

    /// Whether it rests at the last target given (Compose's `isIdle`): that target is its animation's, and it isn't
    /// running.
    pub fn is_idle(&self) -> bool {
        let running = self.inner.animatable.borrow().as_ref().is_some_and(|a| a.is_running());
        self.inner.pending_target.get() == self.target() && !running
    }
}

#[named]
impl<T: AnimationValue, V: AnimationVector> DeferredTargetAnimation<T, V> {
    /// Sets the animation up at `target` the first time; after, animates it to `target` along `animation_spec` in
    /// `coroutine_scope` when that isn't its target already (a new one taking over from where it is). Its value now
    /// (Compose's `updateTarget`).
    pub fn update_target(
        &self,
        target: T,
        coroutine_scope: &CoroutineScope,
        #[default(Arc::new(spring()))] animation_spec: Arc<dyn FiniteAnimationSpec<T>>,
    ) -> T {
        self.inner.pending_target.set(Some(target.clone()));
        let animatable = {
            let mut slot = self.inner.animatable.borrow_mut();
            slot.get_or_insert_with(|| {
                Animatable::new(target.clone(), self.inner.vector_converter.clone(), None, "Animatable")
            })
            .clone()
        };
        let (anim, pending) = (animatable.clone(), self.inner.pending_target.clone());
        let spec: Arc<dyn AnimationSpec<T>> = animation_spec;
        coroutine_scope.launch(async move {
            if Some(anim.target_value()) != pending.get() {
                named_args!(anim.animate_to(target, animation_spec = spec)).await;
            }
        });
        animatable.value()
    }
}

impl<T: AnimationValue + std::fmt::Debug, V: AnimationVector> std::fmt::Debug for DeferredTargetAnimation<T, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "DeferredTargetAnimation(pending: {:?}, idle: {})", self.pending_target(), self.is_idle())
    }
}

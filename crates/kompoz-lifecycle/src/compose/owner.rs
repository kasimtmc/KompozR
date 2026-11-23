//! lifecycle-runtime-compose's DropUnlessLifecycle.kt (`dropUnlessStarted`, `dropUnlessResumed`) and
//! RememberLifecycleOwner.kt (`rememberLifecycleOwner`).

use crate::compose::LocalLifecycleOwner;
use crate::lifecycle::{Lifecycle, LifecycleEvent, LifecycleObserver, LifecycleOwner, LifecycleState};
use crate::registry::LifecycleRegistry;
use kompoz_coroutines::LaunchedEffect;
use kompoz_runtime::{DisposableEffect, kompozable, on_dispose, remember_keyed};
use std::cell::Cell;
use std::rc::{Rc, Weak};

/// Kotlin's private `dropUnlessStateIsAtLeast`.
fn drop_unless_state_is_at_least(
    state: LifecycleState,
    lifecycle_owner: Rc<dyn LifecycleOwner>,
    block: impl Fn() + 'static,
) -> impl Fn() + Clone + 'static {
    assert!(
        state != LifecycleState::Destroyed,
        "Target state is not allowed to be `Lifecycle.State.DESTROYED` because Compose disposes of the composition \
         before `Lifecycle.Event.ON_DESTROY` observers are invoked."
    );
    let block = Rc::new(block);
    move || {
        if lifecycle_owner.lifecycle().current_state().is_at_least(state) {
            block();
        }
    }
}

/// `block` as a callback that runs only while `lifecycle_owner`'s lifecycle is at least `STARTED`, dropped
/// otherwise (Compose's `dropUnlessStarted`: a click on a screen going away, after navigating, does nothing).
///
/// ```no_run
/// # use kompoz_lifecycle::*;
/// # use kompoz_runtime::kompozable;
/// #[kompozable]
/// fn Next(navigate: impl Fn() + 'static) {
///     let on_click = drop_unless_started(block = navigate);
///     # let _ = on_click;
/// }
/// ```
#[kompozable]
#[must_use]
pub fn drop_unless_started(
    #[default(LocalLifecycleOwner.current())] lifecycle_owner: Rc<dyn LifecycleOwner>,
    block: impl Fn() + 'static,
) -> impl Fn() + Clone + 'static {
    drop_unless_state_is_at_least(LifecycleState::Started, lifecycle_owner, block)
}

/// The same while it is at least `RESUMED` (Compose's `dropUnlessResumed`).
#[kompozable]
#[must_use]
pub fn drop_unless_resumed(
    #[default(LocalLifecycleOwner.current())] lifecycle_owner: Rc<dyn LifecycleOwner>,
    block: impl Fn() + 'static,
) -> impl Fn() + Clone + 'static {
    drop_unless_state_is_at_least(LifecycleState::Resumed, lifecycle_owner, block)
}

/// The owner [`remember_lifecycle_owner`] makes (Compose's private `ComposeLifecycleOwner`): its parent's state,
/// capped at its maximum.
struct ComposeLifecycleOwner {
    registry: LifecycleRegistry,
    parent_lifecycle_state: Cell<LifecycleState>,
    max_lifecycle_state: Cell<LifecycleState>,
}

impl LifecycleOwner for ComposeLifecycleOwner {
    fn lifecycle(&self) -> Lifecycle {
        self.registry.lifecycle()
    }
}

impl ComposeLifecycleOwner {
    fn new() -> Rc<ComposeLifecycleOwner> {
        Rc::new_cyclic(|me: &Weak<ComposeLifecycleOwner>| ComposeLifecycleOwner {
            registry: LifecycleRegistry::new(me.clone()),
            parent_lifecycle_state: Cell::new(LifecycleState::Initialized),
            max_lifecycle_state: Cell::new(LifecycleState::Initialized),
        })
    }

    fn set_max_lifecycle_state(&self, value: LifecycleState) {
        self.max_lifecycle_state.set(value);
        self.update_lifecycle_state();
    }

    fn handle_lifecycle_event(&self, event: LifecycleEvent) {
        self.parent_lifecycle_state.set(event.target_state());
        self.update_lifecycle_state();
    }

    fn update_lifecycle_state(&self) {
        let target = self.parent_lifecycle_state.get().min(self.max_lifecycle_state.get());
        if self.registry.current_state() == LifecycleState::Initialized && target == LifecycleState::Destroyed {
            return;
        }
        self.registry.set_current_state(target);
    }
}

/// A lifecycle owner of its own for the content below (Compose's `rememberLifecycleOwner`): following `parent`'s
/// lifecycle (the nearest owner's by default) but never above `max_lifecycle`; with no parent, as if it were
/// resumed; destroyed when it leaves the composition. Provide it as [`LocalLifecycleOwner`] to the content it caps.
///
/// ```no_run
/// # use kompoz_lifecycle::*;
/// # use kompoz_runtime::*;
/// #[kompozable]
/// fn Capped(content: impl Fn() + 'static) {
///     let owner = remember_lifecycle_owner(max_lifecycle = LifecycleState::Started);
///     KompozitionLocalProvider(&[LocalLifecycleOwner.provides(owner)], || content());
/// }
/// ```
#[kompozable]
pub fn remember_lifecycle_owner(
    #[default(LifecycleState::Resumed)] max_lifecycle: LifecycleState,
    #[default(Some(LocalLifecycleOwner.current()))] parent: Option<Rc<dyn LifecycleOwner>>,
) -> Rc<dyn LifecycleOwner> {
    let local = remember_keyed(parent.clone(), ComposeLifecycleOwner::new);
    let (owner, p) = (local.clone(), parent.clone());
    let key: Rc<dyn LifecycleOwner> = local.clone();
    DisposableEffect((key.clone(), parent), move || {
        let o = owner.clone();
        let observer: Rc<dyn LifecycleObserver> =
            Rc::new(move |_: &dyn LifecycleOwner, event: LifecycleEvent| o.handle_lifecycle_event(event));
        if let Some(parent) = &p {
            parent.lifecycle().add_observer(observer.clone());
        } else {
            owner.handle_lifecycle_event(LifecycleEvent::OnResume);
        }
        on_dispose(move || {
            if let Some(parent) = &p {
                parent.lifecycle().remove_observer(observer.clone());
            }
            owner.handle_lifecycle_event(LifecycleEvent::OnDestroy);
        })
    });
    let o = local.clone();
    LaunchedEffect((key.clone(), max_lifecycle), async move {
        o.set_max_lifecycle_state(max_lifecycle);
    });
    key
}

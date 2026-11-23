//! lifecycle-runtime-compose's LifecycleEffect.kt: `LifecycleEventEffect`, `LifecycleStartEffect` and
//! `LifecycleResumeEffect` with their scopes and results.

use crate::compose::LocalLifecycleOwner;
use crate::lifecycle::{Lifecycle, LifecycleEvent, LifecycleObserver, LifecycleOwner};
use kompoz_runtime::{DisposableEffect, State, kompozable, on_dispose, remember_keyed, remember_updated_state};
use std::cell::RefCell;
use std::rc::Rc;

/// Runs `on_event` each time the owner's lifecycle has `event` (Compose's `LifecycleEventEffect`): the latest
/// `on_event` given; first composed, also the events that bring an observer up to the lifecycle's state (`ON_RESUME`
/// runs at once on a resumed screen). Left the composition: no more.
///
/// # Panics
/// With `ON_DESTROY` (Compose disposes of the composition before `ON_DESTROY` observers are told).
///
/// ```no_run
/// # use kompoz_lifecycle::*;
/// # use kompoz_runtime::kompozable;
/// #[kompozable]
/// fn Analytics(track: impl Fn(&str) + 'static) {
///     LifecycleEventEffect(LifecycleEvent::OnResume, on_event = move || track("screen1"));
/// }
/// ```
#[kompozable]
pub fn LifecycleEventEffect(
    event: LifecycleEvent,
    #[default(LocalLifecycleOwner.current())] lifecycle_owner: Rc<dyn LifecycleOwner>,
    on_event: impl Fn() + 'static,
) {
    if event == LifecycleEvent::OnDestroy {
        panic!(
            "LifecycleEventEffect cannot be used to listen for Lifecycle.Event.ON_DESTROY, since Compose disposes of \
             the composition before ON_DESTROY observers are invoked."
        );
    }
    let current_on_event: State<Rc<dyn Fn()>> = remember_updated_state!(Rc::new(on_event) as Rc<dyn Fn()>);
    let owner = lifecycle_owner.clone();
    DisposableEffect(lifecycle_owner, move || {
        let observer: Rc<dyn LifecycleObserver> = Rc::new(move |_: &dyn LifecycleOwner, e: LifecycleEvent| {
            if e == event {
                (current_on_event.get())();
            }
        });
        let lifecycle = owner.lifecycle();
        lifecycle.add_observer(observer.clone());
        on_dispose(move || lifecycle.remove_observer(observer))
    });
}

/// What a [`LifecycleStartEffect`]'s block gives back: run when the lifecycle stops, or the effect leaves (Compose's
/// `LifecycleStopOrDisposeEffectResult`); made by [`LifecycleStartStopEffectScope::on_stop_or_dispose`].
pub trait LifecycleStopOrDisposeEffectResult {
    fn run_stop_or_dispose_effect(&self);
}

/// What a [`LifecycleResumeEffect`]'s block gives back: run when the lifecycle pauses, or the effect leaves
/// (Compose's `LifecyclePauseOrDisposeEffectResult`); made by [`LifecycleResumePauseEffectScope::on_pause_or_dispose`].
pub trait LifecyclePauseOrDisposeEffectResult {
    fn run_pause_or_on_dispose_effect(&self);
}

/// An `Rc` compared by identity: an effect keyed by the scope remembered for its keys (a new one when they change).
struct Same<T>(Rc<T>);

impl<T> PartialEq for Same<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// A block given its scope (Kotlin's `LifecycleOwner.() -> Unit` called with the scope as its receiver), run once.
struct OnceWithScope<S, F> {
    scope: S,
    effect: RefCell<Option<F>>,
}

impl<S, F: FnOnce(&S)> OnceWithScope<S, F> {
    fn run(&self) {
        let effect = self.effect.borrow_mut().take();
        if let Some(effect) = effect {
            effect(&self.scope);
        }
    }
}

impl<F: FnOnce(&LifecycleStartStopEffectScope)> LifecycleStopOrDisposeEffectResult
    for OnceWithScope<LifecycleStartStopEffectScope, F>
{
    fn run_stop_or_dispose_effect(&self) {
        self.run();
    }
}

impl<F: FnOnce(&LifecycleResumePauseEffectScope)> LifecyclePauseOrDisposeEffectResult
    for OnceWithScope<LifecycleResumePauseEffectScope, F>
{
    fn run_pause_or_on_dispose_effect(&self) {
        self.run();
    }
}

/// The receiver of a [`LifecycleStartEffect`]'s block (Compose's `LifecycleStartStopEffectScope`): the lifecycle's
/// owner, and `on_stop_or_dispose { … }`, what the block ends with.
#[derive(Clone)]
pub struct LifecycleStartStopEffectScope {
    lifecycle: Lifecycle,
}

impl LifecycleOwner for LifecycleStartStopEffectScope {
    fn lifecycle(&self) -> Lifecycle {
        self.lifecycle.clone()
    }
}

impl LifecycleStartStopEffectScope {
    /// A scope of `lifecycle`.
    pub fn new(lifecycle: Lifecycle) -> LifecycleStartStopEffectScope {
        LifecycleStartStopEffectScope { lifecycle }
    }

    /// What to do when the lifecycle stops, or the effect leaves the composition (the scope its receiver: Kotlin's
    /// `LifecycleOwner.() -> Unit`, the scope the owner).
    pub fn on_stop_or_dispose(
        &self,
        on_stop_or_dispose_effect: impl FnOnce(&LifecycleStartStopEffectScope) + 'static,
    ) -> Box<dyn LifecycleStopOrDisposeEffectResult> {
        Box::new(OnceWithScope { scope: self.clone(), effect: RefCell::new(Some(on_stop_or_dispose_effect)) })
    }
}

/// The receiver of a [`LifecycleResumeEffect`]'s block (Compose's `LifecycleResumePauseEffectScope`): the
/// lifecycle's owner, and `on_pause_or_dispose { … }`, what the block ends with.
#[derive(Clone)]
pub struct LifecycleResumePauseEffectScope {
    lifecycle: Lifecycle,
}

impl LifecycleOwner for LifecycleResumePauseEffectScope {
    fn lifecycle(&self) -> Lifecycle {
        self.lifecycle.clone()
    }
}

impl LifecycleResumePauseEffectScope {
    /// A scope of `lifecycle`.
    pub fn new(lifecycle: Lifecycle) -> LifecycleResumePauseEffectScope {
        LifecycleResumePauseEffectScope { lifecycle }
    }

    /// What to do when the lifecycle pauses, or the effect leaves the composition (the scope its receiver: Kotlin's
    /// `LifecycleOwner.() -> Unit`, the scope the owner).
    pub fn on_pause_or_dispose(
        &self,
        on_pause_or_dispose_effect: impl FnOnce(&LifecycleResumePauseEffectScope) + 'static,
    ) -> Box<dyn LifecyclePauseOrDisposeEffectResult> {
        Box::new(OnceWithScope { scope: self.clone(), effect: RefCell::new(Some(on_pause_or_dispose_effect)) })
    }
}

/// Runs `effects` each time the owner's lifecycle starts (`ON_START`, and at once when it is started already), and
/// what it gives back ([`on_stop_or_dispose`](LifecycleStartStopEffectScope::on_stop_or_dispose)) each time it stops
/// or the effect leaves the composition (Compose's `LifecycleStartEffect`). Keyed as `DisposableEffect` by `keys`
/// (Kotlin's `key1`, `key1, key2`, … and `vararg keys`: a tuple) and the owner: changed, the last result runs and it
/// starts over; the `effects` of the composition that started it is the one that runs (as Compose's).
///
/// ```no_run
/// # use kompoz_lifecycle::*;
/// # use kompoz_runtime::kompozable;
/// #[kompozable]
/// fn Tracked(screen: &'static str) {
///     LifecycleStartEffect(screen, effects = move |scope: &LifecycleStartStopEffectScope| {
///         println!("{screen} started");
///         scope.on_stop_or_dispose(move |_| println!("{screen} stopped"))
///     });
/// }
/// ```
#[kompozable]
pub fn LifecycleStartEffect<K: PartialEq + Clone + 'static>(
    keys: K,
    #[default(LocalLifecycleOwner.current())] lifecycle_owner: Rc<dyn LifecycleOwner>,
    effects: impl Fn(&LifecycleStartStopEffectScope) -> Box<dyn LifecycleStopOrDisposeEffectResult> + 'static,
) {
    let owner = lifecycle_owner.clone();
    let scope = remember_keyed((keys, lifecycle_owner.clone()), || {
        Rc::new(LifecycleStartStopEffectScope::new(owner.lifecycle()))
    });
    let s = scope.clone();
    DisposableEffect((lifecycle_owner.clone(), Same(scope)), move || {
        let result: Rc<RefCell<Option<Box<dyn LifecycleStopOrDisposeEffectResult>>>> = Rc::new(RefCell::new(None));
        let r = result.clone();
        let observer: Rc<dyn LifecycleObserver> =
            Rc::new(move |_: &dyn LifecycleOwner, event: LifecycleEvent| match event {
                LifecycleEvent::OnStart => {
                    let made = effects(&s);
                    *r.borrow_mut() = Some(made);
                }
                LifecycleEvent::OnStop => {
                    let last = r.borrow_mut().take();
                    if let Some(last) = last {
                        last.run_stop_or_dispose_effect();
                    }
                }
                _ => {}
            });
        let lifecycle = lifecycle_owner.lifecycle();
        lifecycle.add_observer(observer.clone());
        on_dispose(move || {
            lifecycle.remove_observer(observer);
            let last = result.borrow_mut().take();
            if let Some(last) = last {
                last.run_stop_or_dispose_effect();
            }
        })
    });
}

/// Runs `effects` each time the owner's lifecycle resumes (`ON_RESUME`, and at once when it is resumed already),
/// and what it gives back ([`on_pause_or_dispose`](LifecycleResumePauseEffectScope::on_pause_or_dispose)) each time
/// it pauses or the effect leaves the composition (Compose's `LifecycleResumeEffect`); keyed as
/// [`LifecycleStartEffect`].
#[kompozable]
pub fn LifecycleResumeEffect<K: PartialEq + Clone + 'static>(
    keys: K,
    #[default(LocalLifecycleOwner.current())] lifecycle_owner: Rc<dyn LifecycleOwner>,
    effects: impl Fn(&LifecycleResumePauseEffectScope) -> Box<dyn LifecyclePauseOrDisposeEffectResult> + 'static,
) {
    let owner = lifecycle_owner.clone();
    let scope = remember_keyed((keys, lifecycle_owner.clone()), || {
        Rc::new(LifecycleResumePauseEffectScope::new(owner.lifecycle()))
    });
    let s = scope.clone();
    DisposableEffect((lifecycle_owner.clone(), Same(scope)), move || {
        let result: Rc<RefCell<Option<Box<dyn LifecyclePauseOrDisposeEffectResult>>>> = Rc::new(RefCell::new(None));
        let r = result.clone();
        let observer: Rc<dyn LifecycleObserver> =
            Rc::new(move |_: &dyn LifecycleOwner, event: LifecycleEvent| match event {
                LifecycleEvent::OnResume => {
                    let made = effects(&s);
                    *r.borrow_mut() = Some(made);
                }
                LifecycleEvent::OnPause => {
                    let last = r.borrow_mut().take();
                    if let Some(last) = last {
                        last.run_pause_or_on_dispose_effect();
                    }
                }
                _ => {}
            });
        let lifecycle = lifecycle_owner.lifecycle();
        lifecycle.add_observer(observer.clone());
        on_dispose(move || {
            lifecycle.remove_observer(observer);
            let last = result.borrow_mut().take();
            if let Some(last) = last {
                last.run_pause_or_on_dispose_effect();
            }
        })
    });
}

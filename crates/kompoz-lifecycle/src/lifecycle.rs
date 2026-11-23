//! lifecycle-common: [`Lifecycle`] with its [`LifecycleState`]s and [`LifecycleEvent`]s, its observers
//! ([`LifecycleObserver`], [`LifecycleEventObserver`], [`DefaultLifecycleObserver`]), [`LifecycleOwner`], and the
//! extensions of `Lifecycle.kt` (`currentStateFlow`, `coroutineScope`, `eventFlow`, `addObserver { }`).

use kompoz_coroutines::{
    BufferOverflow, Channel, CoroutineContext, CoroutineScope, CoroutineStart, Dispatcher, Flow, Job, MutableStateFlow,
    StateFlow, flow,
};
use std::cell::RefCell;
use std::fmt;
use std::future::Future;
use std::rc::{Rc, Weak};

// ---- State and Event ----

/// A lifecycle's state (Compose's `Lifecycle.State`): the nodes of its graph, ordered — a state is "at least"
/// those before it ([`is_at_least`](Self::is_at_least), `>=`).
///
/// Kotlin: `Lifecycle.State`
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum LifecycleState {
    /// Destroyed: it dispatches no more events (an activity's state right before `onDestroy`).
    ///
    /// Kotlin: `DESTROYED`
    Destroyed,
    /// Made, not created yet (an activity's state before `onCreate`).
    ///
    /// Kotlin: `INITIALIZED`
    Initialized,
    /// Created, not on the screen (after `onCreate`, after `onStop`).
    ///
    /// Kotlin: `CREATED`
    Created,
    /// On the screen, not in front (after `onStart`, after `onPause`).
    ///
    /// Kotlin: `STARTED`
    Started,
    /// In front, taking input (after `onResume`).
    ///
    /// Kotlin: `RESUMED`
    Resumed,
}

impl LifecycleState {
    /// Whether it is `state` or after it.
    pub fn is_at_least(self, state: LifecycleState) -> bool {
        self >= state
    }

    /// Its Kotlin name (`"RESUMED"`), what it prints as.
    pub fn name(self) -> &'static str {
        match self {
            LifecycleState::Destroyed => "DESTROYED",
            LifecycleState::Initialized => "INITIALIZED",
            LifecycleState::Created => "CREATED",
            LifecycleState::Started => "STARTED",
            LifecycleState::Resumed => "RESUMED",
        }
    }

    /// Its place in the order, from 0 (`DESTROYED`).
    pub fn ordinal(self) -> i32 {
        self as i32
    }
}

impl fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A step of a lifecycle from one state to the next (Compose's `Lifecycle.Event`): what its observers are told.
///
/// Kotlin: `Lifecycle.Event`
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LifecycleEvent {
    /// Kotlin: `ON_CREATE`
    OnCreate,
    /// Kotlin: `ON_START`
    OnStart,
    /// Kotlin: `ON_RESUME`
    OnResume,
    /// Kotlin: `ON_PAUSE`
    OnPause,
    /// Kotlin: `ON_STOP`
    OnStop,
    /// Kotlin: `ON_DESTROY`
    OnDestroy,
    /// Any of them (an observer's wildcard; never dispatched).
    ///
    /// Kotlin: `ON_ANY`
    OnAny,
}

impl LifecycleEvent {
    /// The state the lifecycle is in right after it.
    ///
    /// # Panics
    /// For `ON_ANY` ("ON_ANY has no target state", Kotlin's IllegalArgumentException).
    pub fn target_state(self) -> LifecycleState {
        match self {
            LifecycleEvent::OnCreate | LifecycleEvent::OnStop => LifecycleState::Created,
            LifecycleEvent::OnStart | LifecycleEvent::OnPause => LifecycleState::Started,
            LifecycleEvent::OnResume => LifecycleState::Resumed,
            LifecycleEvent::OnDestroy => LifecycleState::Destroyed,
            LifecycleEvent::OnAny => panic!("{self} has no target state"),
        }
    }

    /// The event leaving `state` for the state below it: none from `DESTROYED` or `INITIALIZED`.
    pub fn down_from(state: LifecycleState) -> Option<LifecycleEvent> {
        match state {
            LifecycleState::Created => Some(LifecycleEvent::OnDestroy),
            LifecycleState::Started => Some(LifecycleEvent::OnStop),
            LifecycleState::Resumed => Some(LifecycleEvent::OnPause),
            _ => None,
        }
    }

    /// The event entering `state` from the state above it: none into `INITIALIZED` or `RESUMED`.
    pub fn down_to(state: LifecycleState) -> Option<LifecycleEvent> {
        match state {
            LifecycleState::Destroyed => Some(LifecycleEvent::OnDestroy),
            LifecycleState::Created => Some(LifecycleEvent::OnStop),
            LifecycleState::Started => Some(LifecycleEvent::OnPause),
            _ => None,
        }
    }

    /// The event leaving `state` for the state above it: none from `DESTROYED` or `RESUMED`.
    pub fn up_from(state: LifecycleState) -> Option<LifecycleEvent> {
        match state {
            LifecycleState::Initialized => Some(LifecycleEvent::OnCreate),
            LifecycleState::Created => Some(LifecycleEvent::OnStart),
            LifecycleState::Started => Some(LifecycleEvent::OnResume),
            _ => None,
        }
    }

    /// The event entering `state` from the state below it: none into `DESTROYED` or `INITIALIZED`.
    pub fn up_to(state: LifecycleState) -> Option<LifecycleEvent> {
        match state {
            LifecycleState::Created => Some(LifecycleEvent::OnCreate),
            LifecycleState::Started => Some(LifecycleEvent::OnStart),
            LifecycleState::Resumed => Some(LifecycleEvent::OnResume),
            _ => None,
        }
    }

    /// Its Kotlin name (`"ON_RESUME"`), what it prints as.
    pub fn name(self) -> &'static str {
        match self {
            LifecycleEvent::OnCreate => "ON_CREATE",
            LifecycleEvent::OnStart => "ON_START",
            LifecycleEvent::OnResume => "ON_RESUME",
            LifecycleEvent::OnPause => "ON_PAUSE",
            LifecycleEvent::OnStop => "ON_STOP",
            LifecycleEvent::OnDestroy => "ON_DESTROY",
            LifecycleEvent::OnAny => "ON_ANY",
        }
    }

    /// Its place in the order, from 0 (`ON_CREATE`).
    pub fn ordinal(self) -> i32 {
        self as i32
    }
}

impl fmt::Display for LifecycleEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ---- Observers ----

/// What observes a lifecycle (Compose's marker interface `LifecycleObserver`): a [`LifecycleEventObserver`] (told
/// each event), a [`DefaultLifecycleObserver`] (a method for each), or both — what it is told it says through its
/// two methods (Kotlin's `Lifecycling` checks the observer's interfaces): a closure `|source, event| …` is an event
/// observer already; a type of one's own implementing either says so:
///
/// ```
/// # use kompoz_lifecycle::*;
/// struct Logger;
/// impl DefaultLifecycleObserver for Logger {
///     fn on_start(&self, _owner: &dyn LifecycleOwner) { println!("on the screen"); }
/// }
/// impl LifecycleObserver for Logger {
///     fn as_default_lifecycle_observer(&self) -> Option<&dyn DefaultLifecycleObserver> { Some(self) }
/// }
/// ```
///
/// An observer is told apart from another by its identity (the `Rc` it is added as), as Kotlin's objects.
pub trait LifecycleObserver: 'static {
    /// The event observer it is, if it is one (told each event, after the [`DefaultLifecycleObserver`] method).
    fn as_lifecycle_event_observer(&self) -> Option<&dyn LifecycleEventObserver> {
        None
    }

    /// The default observer it is, if it is one (a method for each event).
    fn as_default_lifecycle_observer(&self) -> Option<&dyn DefaultLifecycleObserver> {
        None
    }
}

/// An observer told each event with the owner whose lifecycle has it (Compose's fun interface
/// `LifecycleEventObserver`): a closure `|source, event| …` is one.
pub trait LifecycleEventObserver: LifecycleObserver {
    fn on_state_changed(&self, source: &dyn LifecycleOwner, event: LifecycleEvent);
}

impl<F: Fn(&dyn LifecycleOwner, LifecycleEvent) + 'static> LifecycleEventObserver for F {
    fn on_state_changed(&self, source: &dyn LifecycleOwner, event: LifecycleEvent) {
        self(source, event)
    }
}

impl<F: Fn(&dyn LifecycleOwner, LifecycleEvent) + 'static> LifecycleObserver for F {
    fn as_lifecycle_event_observer(&self) -> Option<&dyn LifecycleEventObserver> {
        Some(self)
    }
}

/// An observer with a method for each event (Compose's `DefaultLifecycleObserver`), each doing nothing unless
/// written; told before its [`LifecycleEventObserver`] side, if it has one.
pub trait DefaultLifecycleObserver: LifecycleObserver {
    /// After the owner's `onCreate`.
    fn on_create(&self, owner: &dyn LifecycleOwner) {
        let _ = owner;
    }
    /// After the owner's `onStart`.
    fn on_start(&self, owner: &dyn LifecycleOwner) {
        let _ = owner;
    }
    /// After the owner's `onResume`.
    fn on_resume(&self, owner: &dyn LifecycleOwner) {
        let _ = owner;
    }
    /// Before the owner's `onPause`.
    fn on_pause(&self, owner: &dyn LifecycleOwner) {
        let _ = owner;
    }
    /// Before the owner's `onStop`.
    fn on_stop(&self, owner: &dyn LifecycleOwner) {
        let _ = owner;
    }
    /// Before the owner's `onDestroy`.
    fn on_destroy(&self, owner: &dyn LifecycleOwner) {
        let _ = owner;
    }
}

/// Tells `observer` `event` as Kotlin's `Lifecycling.lifecycleEventObserver(observer)` would (its
/// `DefaultLifecycleObserverAdapter`): the default observer's method first, then the event observer.
pub(crate) fn dispatch_to(observer: &dyn LifecycleObserver, source: &dyn LifecycleOwner, event: LifecycleEvent) {
    if let Some(default) = observer.as_default_lifecycle_observer() {
        match event {
            LifecycleEvent::OnCreate => default.on_create(source),
            LifecycleEvent::OnStart => default.on_start(source),
            LifecycleEvent::OnResume => default.on_resume(source),
            LifecycleEvent::OnPause => default.on_pause(source),
            LifecycleEvent::OnStop => default.on_stop(source),
            LifecycleEvent::OnDestroy => default.on_destroy(source),
            LifecycleEvent::OnAny => panic!("ON_ANY must not been send by anybody"),
        }
    }
    if let Some(events) = observer.as_lifecycle_event_observer() {
        events.on_state_changed(source, event);
    }
}

/// Whether two observers are the same one (Kotlin's identity of objects).
pub(crate) fn same_observer(a: &Rc<dyn LifecycleObserver>, b: &Rc<dyn LifecycleObserver>) -> bool {
    std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b))
}

// ---- Lifecycle ----

/// What a [`Lifecycle`] is made of: Kotlin's abstract members of `Lifecycle`, which [`LifecycleRegistry`] (or a
/// lifecycle of one's own) implements.
///
/// [`LifecycleRegistry`]: crate::LifecycleRegistry
pub trait LifecycleImpl: 'static {
    /// Adds `observer`, brought up to the current state at once (told the events up to it).
    fn add_observer(&self, observer: Rc<dyn LifecycleObserver>);
    /// Removes `observer` (no more events; none told now).
    fn remove_observer(&self, observer: Rc<dyn LifecycleObserver>);
    /// The state now.
    fn current_state(&self) -> LifecycleState;
    /// Its state as a flow, if it keeps one ([`Lifecycle::current_state_flow`]'s; none: a flow of an observer's).
    fn current_state_flow(&self) -> Option<StateFlow<LifecycleState>> {
        None
    }
}

struct LifecycleData {
    lifecycle: Rc<dyn LifecycleImpl>,
    /// The scope of [`Lifecycle::coroutine_scope`], once made (Kotlin's `internalScopeRef`).
    internal_scope: RefCell<Option<LifecycleCoroutineScope>>,
}

/// What has a lifecycle (an app, a window, a navigation entry) goes through: it is in a [`LifecycleState`] and goes
/// up and down from one to the next by [`LifecycleEvent`]s, which its observers are told (Compose's `Lifecycle`).
/// Cheap to clone (the same lifecycle); equal when the same one.
///
/// ```text
/// INITIALIZED ─ON_CREATE→ CREATED ─ON_START→ STARTED ─ON_RESUME→ RESUMED
/// DESTROYED ←ON_DESTROY─ CREATED ←ON_STOP─ STARTED ←ON_PAUSE─ RESUMED
/// ```
///
/// Kotlin: `Lifecycle`
#[derive(Clone)]
pub struct Lifecycle(Rc<LifecycleData>);

impl PartialEq for Lifecycle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for Lifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Lifecycle({})", self.current_state())
    }
}

impl Lifecycle {
    /// A lifecycle made of `lifecycle` (a lifecycle of one's own, Kotlin's subclass of `Lifecycle`).
    pub fn new(lifecycle: Rc<dyn LifecycleImpl>) -> Lifecycle {
        Lifecycle(Rc::new(LifecycleData { lifecycle, internal_scope: RefCell::new(None) }))
    }

    /// Adds `observer`: told the events that bring it up to the current state at once (from `INITIALIZED`; one
    /// added while `RESUMED` hears `ON_CREATE`, `ON_START`, `ON_RESUME`), then each event as it happens. Added
    /// already: nothing.
    pub fn add_observer(&self, observer: Rc<dyn LifecycleObserver>) {
        self.0.lifecycle.add_observer(observer);
    }

    /// Adds an event observer of `action` (Kotlin's inline `addObserver { source, event -> }`, the observer its
    /// receiver): the observer, to remove it with.
    ///
    /// Kotlin: `addObserver(action: LifecycleObserver.(source: LifecycleOwner, event: Lifecycle.Event) -> Unit): LifecycleObserver`
    pub fn add_observer_with(
        &self,
        action: impl Fn(&dyn LifecycleObserver, &dyn LifecycleOwner, LifecycleEvent) + 'static,
    ) -> Rc<dyn LifecycleObserver> {
        struct ActionObserver<F>(F);
        impl<F: Fn(&dyn LifecycleObserver, &dyn LifecycleOwner, LifecycleEvent) + 'static> LifecycleObserver
            for ActionObserver<F>
        {
            fn as_lifecycle_event_observer(&self) -> Option<&dyn LifecycleEventObserver> {
                Some(self)
            }
        }
        impl<F: Fn(&dyn LifecycleObserver, &dyn LifecycleOwner, LifecycleEvent) + 'static> LifecycleEventObserver
            for ActionObserver<F>
        {
            fn on_state_changed(&self, source: &dyn LifecycleOwner, event: LifecycleEvent) {
                (self.0)(self, source, event)
            }
        }
        let observer: Rc<dyn LifecycleObserver> = Rc::new(ActionObserver(action));
        self.add_observer(observer.clone());
        observer
    }

    /// Removes `observer` (added as this `Rc`, or a clone of it): it is told no more events.
    pub fn remove_observer(&self, observer: Rc<dyn LifecycleObserver>) {
        self.0.lifecycle.remove_observer(observer);
    }

    /// The state now (not observed: a composable reads [`current_state_as_state`](Lifecycle::
    /// current_state_as_state)).
    pub fn current_state(&self) -> LifecycleState {
        self.0.lifecycle.current_state()
    }

    /// Its state as a flow: a [`LifecycleRegistry`](crate::LifecycleRegistry)'s own, else one an observer added
    /// now keeps (Compose's open `currentStateFlow`).
    pub fn current_state_flow(&self) -> StateFlow<LifecycleState> {
        if let Some(flow) = self.0.lifecycle.current_state_flow() {
            return flow;
        }
        let state = MutableStateFlow::new(self.current_state());
        let s = state.clone();
        let observer: Rc<dyn LifecycleObserver> =
            Rc::new(move |_: &dyn LifecycleOwner, event: LifecycleEvent| s.set_value(event.target_state()));
        self.add_observer(observer);
        state.as_state_flow()
    }

    /// The coroutine scope tied to it (Compose's `Lifecycle.coroutineScope`): cancelled once it is destroyed; its
    /// coroutines on the main dispatcher ([`Dispatcher::main`]), failing on their own (a supervisor). The same scope
    /// each time.
    pub fn coroutine_scope(&self) -> LifecycleCoroutineScope {
        if let Some(scope) = self.0.internal_scope.borrow().clone() {
            return scope;
        }
        let scope = LifecycleCoroutineScope {
            scope: CoroutineScope::new_supervisor(&Dispatcher::main(), CoroutineContext::new()),
            lifecycle: Rc::downgrade(&self.0),
        };
        // (made on a lifecycle destroyed already: cancelled at once)
        if self.current_state() == LifecycleState::Destroyed {
            scope.scope.cancel();
        }
        *self.0.internal_scope.borrow_mut() = Some(scope.clone());
        scope.register();
        scope
    }

    /// Its events as a flow (Compose's `Lifecycle.eventFlow`): those bringing a new collector up to the current state
    /// first, each event after; done after `ON_DESTROY`.
    pub fn event_flow(&self) -> Flow<LifecycleEvent> {
        let lifecycle = self.clone();
        flow(move |collector| {
            let lifecycle = lifecycle.clone();
            async move {
                // (Kotlin's callbackFlow: a channel the observer sends to, buffered, closed after ON_DESTROY)
                let channel = Channel::<LifecycleEvent>::new(64, BufferOverflow::Suspend);
                let ch = channel.clone();
                let observer: Rc<dyn LifecycleObserver> =
                    Rc::new(move |_: &dyn LifecycleOwner, event: LifecycleEvent| {
                        ch.try_send(event);
                        if event == LifecycleEvent::OnDestroy {
                            ch.close();
                        }
                    });
                lifecycle.add_observer(observer.clone());
                let _remove = RemoveObserver(lifecycle.clone(), Some(observer));
                while let Some(event) = channel.receive().await {
                    collector.emit(event).await;
                }
            }
        })
    }
}

/// Removes an observer when dropped (Kotlin's `finally { removeObserver(…) }`, `awaitClose { … }`).
pub(crate) struct RemoveObserver(pub(crate) Lifecycle, pub(crate) Option<Rc<dyn LifecycleObserver>>);

impl Drop for RemoveObserver {
    fn drop(&mut self) {
        if let Some(observer) = self.1.take() {
            self.0.remove_observer(observer);
        }
    }
}

// ---- LifecycleOwner ----

/// What has a [`Lifecycle`] (Compose's `LifecycleOwner`): the host's (the app's, following its window), a
/// navigation entry's, [`remember_lifecycle_owner`](crate::remember_lifecycle_owner)'s. Shared as
/// `Rc<dyn LifecycleOwner>` ([`LocalLifecycleOwner`](crate::LocalLifecycleOwner)), told apart by identity.
pub trait LifecycleOwner {
    /// Its lifecycle.
    fn lifecycle(&self) -> Lifecycle;
}

/// Owners are the same when they are the same object (Kotlin's identity): an `Rc<dyn LifecycleOwner>` compares so
/// (what keys an effect).
impl PartialEq for dyn LifecycleOwner {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// An owner of `lifecycle` (Compose's `LifecycleOwner(lifecycle)` factory).
#[allow(non_snake_case)]
pub fn LifecycleOwner(lifecycle: Lifecycle) -> Rc<dyn LifecycleOwner> {
    struct Owner(Lifecycle);
    impl LifecycleOwner for Owner {
        fn lifecycle(&self) -> Lifecycle {
            self.0.clone()
        }
    }
    Rc::new(Owner(lifecycle))
}

// ---- LifecycleCoroutineScope ----

/// The coroutine scope tied to a lifecycle (Compose's `LifecycleCoroutineScope`, [`Lifecycle::coroutine_scope`],
/// an owner's `lifecycleScope`): cancelled once the lifecycle is destroyed. It is a [`CoroutineScope`] (`launch`,
/// `async`…), and has the deprecated `launchWhen…` forms.
#[derive(Clone)]
pub struct LifecycleCoroutineScope {
    scope: CoroutineScope,
    lifecycle: Weak<LifecycleData>,
}

impl PartialEq for LifecycleCoroutineScope {
    fn eq(&self, other: &Self) -> bool {
        self.scope == other.scope
    }
}

impl std::ops::Deref for LifecycleCoroutineScope {
    type Target = CoroutineScope;
    fn deref(&self) -> &CoroutineScope {
        &self.scope
    }
}

impl LifecycleCoroutineScope {
    /// The lifecycle it is tied to (gone: none).
    pub fn lifecycle(&self) -> Option<Lifecycle> {
        self.lifecycle.upgrade().map(Lifecycle)
    }

    /// Kotlin's `register()`: on the main dispatcher at once, the scope observes its lifecycle (cancelled with it).
    fn register(&self) {
        let this = self.clone();
        kompoz_runtime::named_args!(self.scope.launch(start = CoroutineStart::UNDISPATCHED, async move {
            let Some(lifecycle) = this.lifecycle() else { return };
            if lifecycle.current_state() >= LifecycleState::Initialized {
                let scope = this.scope.clone();
                let weak = this.lifecycle.clone();
                // (Kotlin's LifecycleCoroutineScopeImpl is the observer: it holds the lifecycle weakly here)
                let observer =
                    Rc::new_cyclic(|me: &Weak<ScopeObserver>| ScopeObserver { me: me.clone(), scope, lifecycle: weak });
                lifecycle.add_observer(observer);
            } else {
                this.scope.cancel();
            }
        }));
    }

    /// Launches `block` once the lifecycle is at least `CREATED`, suspended while it is below (deprecated as
    /// Compose's: it suspends rather than cancels — prefer `repeat_on_lifecycle`).
    #[deprecated(note = "launchWhenCreated is deprecated as it can lead to wasted resources in some cases. Replace \
                         with suspending repeatOnLifecycle to run the block whenever the Lifecycle state is at least \
                         Lifecycle.State.CREATED.")]
    pub fn launch_when_created<Fut: Future<Output = ()> + 'static>(
        &self,
        block: impl FnOnce(CoroutineScope) -> Fut + 'static,
    ) -> Job {
        self.launch_when(LifecycleState::Created, block)
    }

    /// The same, at least `STARTED`.
    #[deprecated(note = "launchWhenStarted is deprecated as it can lead to wasted resources in some cases. Replace \
                         with suspending repeatOnLifecycle to run the block whenever the Lifecycle state is at least \
                         Lifecycle.State.STARTED.")]
    pub fn launch_when_started<Fut: Future<Output = ()> + 'static>(
        &self,
        block: impl FnOnce(CoroutineScope) -> Fut + 'static,
    ) -> Job {
        self.launch_when(LifecycleState::Started, block)
    }

    /// The same, at least `RESUMED`.
    #[deprecated(note = "launchWhenResumed is deprecated as it can lead to wasted resources in some cases. Replace \
                         with suspending repeatOnLifecycle to run the block whenever the Lifecycle state is at least \
                         Lifecycle.State.RESUMED.")]
    pub fn launch_when_resumed<Fut: Future<Output = ()> + 'static>(
        &self,
        block: impl FnOnce(CoroutineScope) -> Fut + 'static,
    ) -> Job {
        self.launch_when(LifecycleState::Resumed, block)
    }

    fn launch_when<Fut: Future<Output = ()> + 'static>(
        &self,
        state: LifecycleState,
        block: impl FnOnce(CoroutineScope) -> Fut + 'static,
    ) -> Job {
        let lifecycle = self.lifecycle();
        self.scope.launch(async move {
            if let Some(lifecycle) = lifecycle {
                crate::pausing::when_state_at_least(lifecycle, state, block).await;
            }
        })
    }
}

/// The observer a lifecycle's coroutine scope adds: cancels it once the lifecycle is destroyed.
struct ScopeObserver {
    me: Weak<ScopeObserver>,
    scope: CoroutineScope,
    lifecycle: Weak<LifecycleData>,
}

impl LifecycleObserver for ScopeObserver {
    fn as_lifecycle_event_observer(&self) -> Option<&dyn LifecycleEventObserver> {
        Some(self)
    }
}

impl LifecycleEventObserver for ScopeObserver {
    fn on_state_changed(&self, _source: &dyn LifecycleOwner, _event: LifecycleEvent) {
        let Some(lifecycle) = self.lifecycle.upgrade().map(Lifecycle) else { return };
        if lifecycle.current_state() <= LifecycleState::Destroyed {
            if let Some(me) = self.me.upgrade() {
                lifecycle.remove_observer(me);
            }
            self.scope.cancel();
        }
    }
}

/// What lifecycle-runtime adds to an owner (Compose's `LifecycleOwner` extensions): its lifecycle's scope, and the
/// suspending calls of its lifecycle (`repeatOnLifecycle`, `withStarted`, …) on the owner.
pub trait LifecycleOwnerExtensions: LifecycleOwner {
    /// Its lifecycle's coroutine scope ([`Lifecycle::coroutine_scope`]).
    fn lifecycle_scope(&self) -> LifecycleCoroutineScope {
        self.lifecycle().coroutine_scope()
    }

    /// [`Lifecycle::repeat_on_lifecycle`] of its lifecycle.
    fn repeat_on_lifecycle<Fut: Future<Output = ()> + 'static>(
        &self,
        state: LifecycleState,
        block: impl Fn(CoroutineScope) -> Fut + 'static,
    ) -> LocalFuture<()> {
        Box::pin(self.lifecycle().repeat_on_lifecycle(state, block))
    }

    /// [`Lifecycle::with_state_at_least`] of its lifecycle.
    fn with_state_at_least<R: 'static>(
        &self,
        state: LifecycleState,
        block: impl FnOnce() -> R + 'static,
    ) -> LocalFuture<R> {
        Box::pin(self.lifecycle().with_state_at_least(state, block))
    }

    /// [`Lifecycle::with_created`] of its lifecycle.
    fn with_created<R: 'static>(&self, block: impl FnOnce() -> R + 'static) -> LocalFuture<R> {
        Box::pin(self.lifecycle().with_created(block))
    }

    /// [`Lifecycle::with_started`] of its lifecycle.
    fn with_started<R: 'static>(&self, block: impl FnOnce() -> R + 'static) -> LocalFuture<R> {
        Box::pin(self.lifecycle().with_started(block))
    }

    /// [`Lifecycle::with_resumed`] of its lifecycle.
    fn with_resumed<R: 'static>(&self, block: impl FnOnce() -> R + 'static) -> LocalFuture<R> {
        Box::pin(self.lifecycle().with_resumed(block))
    }

    /// [`Lifecycle::when_created`] of its lifecycle (deprecated as Compose's).
    #[deprecated(note = "whenCreated has been deprecated because it runs the block on a pausing dispatcher that \
                         suspends, rather than cancels work when the lifecycle state goes below the given state. Use \
                         withCreated for non-suspending work that needs to run only once when the Lifecycle changes.")]
    fn when_created<T: 'static, Fut: Future<Output = T> + 'static>(
        &self,
        block: impl FnOnce(CoroutineScope) -> Fut + 'static,
    ) -> LocalFuture<T> {
        Box::pin(crate::pausing::when_state_at_least(self.lifecycle(), LifecycleState::Created, block))
    }

    /// [`Lifecycle::when_started`] of its lifecycle (deprecated as Compose's).
    #[deprecated(note = "whenStarted has been deprecated because it runs the block on a pausing dispatcher that \
                         suspends, rather than cancels work when the lifecycle state goes below the given state. Use \
                         withStarted for non-suspending work that needs to run only once when the Lifecycle changes.")]
    fn when_started<T: 'static, Fut: Future<Output = T> + 'static>(
        &self,
        block: impl FnOnce(CoroutineScope) -> Fut + 'static,
    ) -> LocalFuture<T> {
        Box::pin(crate::pausing::when_state_at_least(self.lifecycle(), LifecycleState::Started, block))
    }

    /// [`Lifecycle::when_resumed`] of its lifecycle (deprecated as Compose's).
    #[deprecated(note = "whenResumed has been deprecated because it runs the block on a pausing dispatcher that \
                         suspends, rather than cancels work when the lifecycle state goes below the given state. Use \
                         withResumed for non-suspending work that needs to run only once when the Lifecycle changes.")]
    fn when_resumed<T: 'static, Fut: Future<Output = T> + 'static>(
        &self,
        block: impl FnOnce(CoroutineScope) -> Fut + 'static,
    ) -> LocalFuture<T> {
        Box::pin(crate::pausing::when_state_at_least(self.lifecycle(), LifecycleState::Resumed, block))
    }
}

impl<O: LifecycleOwner + ?Sized> LifecycleOwnerExtensions for O {}

/// A future of the UI thread, boxed (what an owner's suspending calls give: a trait's own can't be named).
pub type LocalFuture<T> = std::pin::Pin<Box<dyn Future<Output = T>>>;

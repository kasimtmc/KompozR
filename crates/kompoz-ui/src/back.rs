//! Back handling, as activity-compose 1.13's (androidx.activity.compose's `BackHandler` and
//! `PredictiveBackHandler`, on navigationevent's dispatcher): handlers registered with the nearest
//! `LocalNavigationEventDispatcherOwner`'s dispatcher (the host's, at the root) while composed — the latest
//! registered enabled one takes back, a focusable popup's or a dialog's (registered as it opens, before its
//! content's) before the content under it —, enabled while the lifecycle is at least started. A back gesture goes to
//! the handler that takes back when it starts, through its steps to its end or its cancellation. And androidx's
//! `BackEventCompat`, the gesture's step as these handlers have it.
//!
//! The legacy `OnBackPressedDispatcher` (activity's, which activity-compose falls back to without a navigation event
//! dispatcher owner) isn't KompozR's: the host always provides an owner.

use kompoz_coroutines::{Channel, CoroutineScope, Flow, Job, flow, remember_coroutine_scope};
use kompoz_lifecycle::{LifecycleStartEffect, LifecycleStartStopEffectScope};
use kompoz_navigationevent::{
    LocalNavigationEventDispatcherOwner, NavigationEvent, NavigationEventDispatcher, NavigationEventDispatcherOwner,
    NavigationEventHandler, NavigationEventHandlerCallbacks, NavigationEventInfo,
};
use kompoz_runtime::{
    DisposableEffect, SideEffect, current_composite_key_hash, kompozable, on_dispose, remember_keyed,
};
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

/// The edge a back gesture started from (androidx's `BackEventCompat.SwipeEdge`, its `EDGE_LEFT`, `EDGE_RIGHT` and
/// `EDGE_NONE`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum SwipeEdge {
    #[default]
    Left,
    Right,
    /// Not a swipe from an edge (a button's).
    None,
}

impl SwipeEdge {
    /// Its navigation event's `swipeEdge` (`NavigationEvent.EDGE_…`).
    pub fn to_int(self) -> i32 {
        match self {
            SwipeEdge::Left => NavigationEvent::EDGE_LEFT,
            SwipeEdge::Right => NavigationEvent::EDGE_RIGHT,
            SwipeEdge::None => NavigationEvent::EDGE_NONE,
        }
    }

    /// The edge of a navigation event's `swipeEdge`.
    pub fn from_int(swipe_edge: i32) -> SwipeEdge {
        match swipe_edge {
            NavigationEvent::EDGE_LEFT => SwipeEdge::Left,
            NavigationEvent::EDGE_RIGHT => SwipeEdge::Right,
            _ => SwipeEdge::None,
        }
    }
}

/// A step of a back gesture (androidx's `BackEventCompat`): where the touch is, how far along it is (0 to 1), the
/// edge it came from, its frame time (ms). Kotlin reads its fields as properties (`backEvent.progress`).
#[derive(Clone, Copy, PartialEq, Debug, Default, kompoz_runtime::KotlinProperties)]
pub struct BackEventCompat {
    pub touch_x: f32,
    pub touch_y: f32,
    pub progress: f32,
    pub swipe_edge: SwipeEdge,
    pub frame_time_millis: i64,
}

impl BackEventCompat {
    /// The event as navigationevent has it (Kotlin's `toNavigationEvent()`).
    pub fn to_navigation_event(&self) -> NavigationEvent {
        NavigationEvent {
            touch_x: self.touch_x,
            touch_y: self.touch_y,
            progress: self.progress,
            swipe_edge: self.swipe_edge.to_int(),
            frame_time_millis: self.frame_time_millis,
        }
    }
}

/// Kotlin's `BackEventCompat(navigationEvent)`.
impl From<NavigationEvent> for BackEventCompat {
    fn from(event: NavigationEvent) -> Self {
        BackEventCompat {
            touch_x: event.touch_x,
            touch_y: event.touch_y,
            progress: event.progress,
            swipe_edge: SwipeEdge::from_int(event.swipe_edge),
            frame_time_millis: event.frame_time_millis,
        }
    }
}

/// A back handler's info (activity-compose's private `BackHandlerInfo` / `PredictiveBackHandlerInfo`: the owner
/// and the composition's place).
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct BackHandlerInfo {
    owner: usize,
    composite_key: u64,
    predictive: bool,
}

impl NavigationEventInfo for BackHandlerInfo {}

type Handler = NavigationEventHandler<BackHandlerInfo>;

/// What a back handler does (activity-compose's internal `BackHandlerCompat`'s members), one place for the
/// navigation event handler to call; each is given that handler (Kotlin's `isBackEnabled` is its).
trait BackHandlerCompat: 'static {
    fn on_back_started(&self, handler: &Handler, event: BackEventCompat) {
        let _ = (handler, event);
    }
    fn on_back_progressed(&self, handler: &Handler, event: BackEventCompat) {
        let _ = (handler, event);
    }
    fn on_back_completed(&self, handler: &Handler);
    fn on_back_cancelled(&self, handler: &Handler) {
        let _ = handler;
    }
}

/// The navigation event handler of a [`BackHandlerCompat`] (its `navigationEventHandler`).
struct CompatCallbacks<C>(Rc<C>);

impl<C: BackHandlerCompat> NavigationEventHandlerCallbacks<BackHandlerInfo> for CompatCallbacks<C> {
    fn on_back_started(&self, handler: &Handler, event: NavigationEvent) {
        self.0.on_back_started(handler, BackEventCompat::from(event));
    }
    fn on_back_progressed(&self, handler: &Handler, event: NavigationEvent) {
        self.0.on_back_progressed(handler, BackEventCompat::from(event));
    }
    fn on_back_completed(&self, handler: &Handler) {
        self.0.on_back_completed(handler);
    }
    fn on_back_cancelled(&self, handler: &Handler) {
        self.0.on_back_cancelled(handler);
    }
}

/// A back handler as composed: what it does and its navigation event handler (disabled until its lifecycle effect
/// enables it). Equal by identity (it keys the effects).
struct Composed<C> {
    compat: Rc<C>,
    handler: NavigationEventHandler<BackHandlerInfo>,
}

impl<C> Clone for Composed<C> {
    fn clone(&self) -> Self {
        Composed { compat: self.compat.clone(), handler: self.handler.clone() }
    }
}

impl<C> PartialEq for Composed<C> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.compat, &other.compat)
    }
}

impl<C: BackHandlerCompat> Composed<C> {
    fn new(compat: C, info: BackHandlerInfo) -> Composed<C> {
        let compat = Rc::new(compat);
        let handler = NavigationEventHandler::new(info, false, CompatCallbacks(compat.clone()));
        Composed { compat, handler }
    }
}

/// The owner's dispatcher (activity-compose's `BackHandlerDispatcherCompat`, its navigation event side), compared
/// by identity.
#[derive(Clone, PartialEq)]
struct DispatcherOf(NavigationEventDispatcher);

/// An owner compared by identity.
struct SameOwner(Rc<dyn NavigationEventDispatcherOwner>);

impl PartialEq for SameOwner {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The nearest owner.
///
/// # Panics
/// None provided (activity-compose's error, its legacy owner named too).
#[track_caller]
fn owner() -> Rc<dyn NavigationEventDispatcherOwner> {
    LocalNavigationEventDispatcherOwner.current().unwrap_or_else(|| {
        panic!(
            "No NavigationEventDispatcherOwner was provided via LocalNavigationEventDispatcherOwner and no \
             OnBackPressedDispatcherOwner was provided via LocalOnBackPressedDispatcherOwner. Please provide one of \
             the two."
        )
    })
}

/// Registers a composed handler as activity-compose does (`isOnBackPressedLifecycleOrderMaintained`, its default):
/// added to the dispatcher while composed, enabled as `enabled` says while the lifecycle is at least started.
#[kompozable(inline)]
fn register<C: BackHandlerCompat>(
    dispatcher: NavigationEventDispatcher,
    composed: Composed<C>,
    enabled: bool,
    set_enabled: fn(&Composed<C>, bool),
) {
    let c = composed.clone();
    LifecycleStartEffect(
        (enabled, composed.clone()),
        effects = move |scope: &LifecycleStartStopEffectScope| {
            set_enabled(&c, enabled);
            let c = c.clone();
            scope.on_stop_or_dispose(move |_| set_enabled(&c, false))
        },
    );
    DisposableEffect((DispatcherOf(dispatcher.clone()), composed.clone()), move || {
        dispatcher.add_handler(&composed.handler);
        on_dispose(move || composed.handler.remove())
    });
}

/// A plain back handler (activity-compose's private `ComposeBackHandler`): its `on_back` the latest given.
struct ComposeBackHandler {
    current_on_back_completed: RefCell<Rc<dyn Fn()>>,
}

impl BackHandlerCompat for ComposeBackHandler {
    fn on_back_completed(&self, _: &Handler) {
        let f = self.current_on_back_completed.borrow().clone();
        f();
    }
}

/// Handles back presses while `enabled` (Compose's `BackHandler`, activity-compose's): `on_back` runs when back
/// completes (a press, or a gesture's end). The handler composed last among the enabled ones takes it; it is
/// enabled only while the lifecycle is at least started. Call it unconditionally and use `enabled` (a conditional
/// call changes the order of composition).
///
/// # Panics
/// No `LocalNavigationEventDispatcherOwner` (outside a host).
#[kompozable]
pub fn BackHandler(#[default(true)] enabled: bool, on_back: impl Fn() + 'static) {
    let owner = owner();
    let o = owner.clone();
    let dispatcher = remember_keyed(SameOwner(owner.clone()), move || o.navigation_event_dispatcher());
    let composite_key = current_composite_key_hash();
    let info = BackHandlerInfo { owner: Rc::as_ptr(&owner) as *const () as usize, composite_key, predictive: false };
    let handler = remember_keyed((DispatcherOf(dispatcher.clone()), composite_key), move || {
        Composed::new(ComposeBackHandler { current_on_back_completed: RefCell::new(Rc::new(|| {})) }, info)
    });
    let h = handler.clone();
    SideEffect(move || *h.compat.current_on_back_completed.borrow_mut() = Rc::new(on_back));
    register(dispatcher, handler, enabled, |c, enabled| c.handler.set_is_back_enabled(enabled));
}

type OnBack = Rc<dyn Fn(Flow<BackEventCompat>) -> Pin<Box<dyn Future<Output = ()>>>>;

/// A predictive back handler (activity-compose's private `ComposePredictiveBackHandler`): one gesture at a time, each
/// with a channel of its steps and a coroutine of `on_back` collecting them (in the composition's scope); the
/// gesture's end closes the channel (the collection ends and the coroutine goes on), its cancellation cancels both.
struct ComposePredictiveBackHandler {
    scope: CoroutineScope,
    current_on_back: RefCell<OnBack>,
    active_channel: RefCell<Option<Channel<BackEventCompat>>>,
    active_job: RefCell<Option<Job>>,
    is_predictive_back: Cell<bool>,
}

impl ComposePredictiveBackHandler {
    /// Kotlin's overridden `isBackEnabled` setter: disabled with a gesture's coroutine done, what is left of it
    /// goes.
    fn set_is_back_enabled(&self, handler: &Handler, value: bool) {
        let finished = self.active_job.borrow().as_ref().is_some_and(|j| !j.is_active());
        if !value && handler.is_back_enabled() && finished {
            self.on_back_cancelled(handler);
        }
        handler.set_is_back_enabled(value);
    }

    /// A gesture's run: a fresh channel, and `on_back` launched with its flow (which it must collect).
    fn launch_new_gesture(&self, handler: &Handler) {
        let channel = Channel::buffered();
        let steps = channel.receive_as_flow();
        *self.active_channel.borrow_mut() = Some(channel);
        let (handler, on_back) = (handler.clone(), self.current_on_back.borrow().clone());
        let job = self.scope.launch(async move {
            if handler.is_back_enabled() {
                let completed = Rc::new(Cell::new(false));
                let c = completed.clone();
                // (Kotlin's `onCompletion { completed = true }`)
                let progress = flow(move |down| {
                    let (steps, c) = (steps.clone(), c.clone());
                    async move {
                        steps.collect_async(move |v| down.emit(v)).await;
                        c.set(true);
                    }
                });
                on_back(progress).await;
                assert!(completed.get(), "You must collect the progress flow");
            }
        });
        *self.active_job.borrow_mut() = Some(job);
    }
}

impl BackHandlerCompat for ComposePredictiveBackHandler {
    fn on_back_started(&self, handler: &Handler, _: BackEventCompat) {
        // (a gesture before not cleaned up: called off first)
        self.on_back_cancelled(handler);
        if handler.is_back_enabled() {
            self.is_predictive_back.set(true);
            self.launch_new_gesture(handler);
        }
    }

    fn on_back_progressed(&self, _: &Handler, event: BackEventCompat) {
        // (not suspending: a full buffer drops the step)
        if let Some(c) = self.active_channel.borrow().as_ref() {
            c.try_send(event);
        }
    }

    fn on_back_completed(&self, handler: &Handler) {
        // (a press: delivered through a fresh run)
        let has_channel = self.active_channel.borrow().is_some();
        if has_channel && !self.is_predictive_back.get() {
            self.on_back_cancelled(handler);
        }
        if self.active_channel.borrow().is_none() {
            self.is_predictive_back.set(false);
            self.launch_new_gesture(handler);
        }
        // (closed: the collection ends, completed)
        if let Some(c) = self.active_channel.borrow().as_ref() {
            c.close();
        }
        self.is_predictive_back.set(false);
    }

    fn on_back_cancelled(&self, _: &Handler) {
        let channel = self.active_channel.borrow_mut().take();
        if let Some(c) = channel {
            c.cancel();
        }
        let job = self.active_job.borrow_mut().take();
        if let Some(j) = job {
            j.cancel();
        }
        self.is_predictive_back.set(false);
    }
}

/// Handles back with the gesture's progress while `enabled` (Compose's `PredictiveBackHandler`, activity-compose's):
/// `on_back` runs in a coroutine from the gesture's start, its flow the gesture's steps; the gesture done, the flow
/// ends and the coroutine goes on to its end; called off, the coroutine is cancelled (its `CancellationException`).
/// A press runs it with a flow that ends at once. `on_back` must collect the flow. The handler composed last among
/// the enabled ones takes it; enabled only while the lifecycle is at least started. (A gesture may start in a frame
/// before an `enabled` change is applied, and still come here; disabled during a gesture, the gesture goes on here.)
///
/// # Panics
/// No `LocalNavigationEventDispatcherOwner` (outside a host).
#[kompozable]
pub fn PredictiveBackHandler<Fut: Future<Output = ()> + 'static>(
    #[default(true)] enabled: bool,
    on_back: impl Fn(Flow<BackEventCompat>) -> Fut + 'static,
) {
    let owner = owner();
    let o = owner.clone();
    let dispatcher = remember_keyed(SameOwner(owner.clone()), move || o.navigation_event_dispatcher());
    let scope = remember_coroutine_scope();
    let composite_key = current_composite_key_hash();
    let info = BackHandlerInfo { owner: Rc::as_ptr(&owner) as *const () as usize, composite_key, predictive: true };
    let handler = remember_keyed((DispatcherOf(dispatcher.clone()), composite_key), move || {
        let state = ComposePredictiveBackHandler {
            scope,
            current_on_back: RefCell::new(Rc::new(|_| Box::pin(async {}))),
            active_channel: RefCell::new(None),
            active_job: RefCell::new(None),
            is_predictive_back: Cell::new(false),
        };
        Composed::new(state, info)
    });
    let h = handler.clone();
    SideEffect(move || *h.compat.current_on_back.borrow_mut() = Rc::new(move |flow| Box::pin(on_back(flow))));
    register(dispatcher, handler, enabled, |c, enabled| c.compat.set_is_back_enabled(&c.handler, enabled));
}

//! Back handling (C++'s back handlers; Compose's `BackHandler` and `PredictiveBackHandler` on Android's
//! OnBackPressedDispatcher): handlers registered by composables, the most recently composed enabled one taking
//! back; while a focusable popup or a dialog is on top, only the handlers composed in it (then its dismissal). A
//! back gesture goes to the handler that would take back when it starts: its steps, then its end or its
//! cancellation.

use kompoz_coroutines::{Channel, CoroutineScope, Flow, LocalDispatcher};
use kompoz_runtime::{DisposableEffect, KompozitionLocal, kompozable, on_dispose, remember};
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::{Rc, Weak};

/// The edge a back gesture started from (androidx's BackEventCompat.SwipeEdge).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum SwipeEdge {
    #[default]
    Left,
    Right,
    /// Not a swipe from an edge (a button's).
    None,
}

/// A step of a back gesture (androidx's BackEventCompat): where the touch is, how far along it is (0 to 1), the
/// edge it came from.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct BackEventCompat {
    pub touch_x: f32,
    pub touch_y: f32,
    pub progress: f32,
    pub swipe_edge: SwipeEdge,
}

/// The window a back handler belongs to: a focusable popup's or dialog's (its handlers take back while it is on
/// top), none for the host's. Equal when it is the same one.
#[derive(Clone)]
pub struct BackScope(pub(crate) Rc<()>);

impl BackScope {
    pub(crate) fn new() -> BackScope {
        BackScope(Rc::new(()))
    }
}

impl PartialEq for BackScope {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for BackScope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "BackScope({:p})", Rc::as_ptr(&self.0))
    }
}

/// The back scope of the focusable popup or dialog being composed (none outside one).
pub static LocalBackScope: KompozitionLocal<Option<BackScope>> = KompozitionLocal::new(|| None);

/// The host's back dispatcher, as composables reach it (Compose's LocalOnBackPressedDispatcherOwner).
pub static LocalBackDispatcher: KompozitionLocal<Option<BackDispatcherRef>> = KompozitionLocal::new(|| None);

/// A handle of a host's back dispatcher (weak).
#[derive(Clone)]
pub struct BackDispatcherRef(pub(crate) Weak<BackDispatcher>);

impl PartialEq for BackDispatcherRef {
    fn eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }
}

impl BackDispatcherRef {
    pub fn get(&self) -> Option<Rc<BackDispatcher>> {
        self.0.upgrade()
    }
}

type EventFn = Rc<dyn Fn(BackEventCompat)>;

/// A back handler (androidx's OnBackPressedCallback): what back does while it is enabled, what a gesture's start,
/// steps and cancellation do, and the window it belongs to.
pub struct BackCallback {
    on_back: RefCell<Rc<dyn Fn()>>,
    on_started: RefCell<Option<EventFn>>,
    on_progressed: RefCell<Option<EventFn>>,
    on_cancelled: RefCell<Option<Rc<dyn Fn()>>>,
    enabled: Cell<bool>,
    scope: RefCell<Option<BackScope>>,
}

impl BackCallback {
    /// A handler of the host's window doing `on_back`.
    pub fn new(on_back: impl Fn() + 'static) -> Rc<BackCallback> {
        Rc::new(BackCallback {
            on_back: RefCell::new(Rc::new(on_back)),
            on_started: RefCell::new(None),
            on_progressed: RefCell::new(None),
            on_cancelled: RefCell::new(None),
            enabled: Cell::new(true),
            scope: RefCell::new(None),
        })
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }
    pub fn set_on_back(&self, on_back: impl Fn() + 'static) {
        *self.on_back.borrow_mut() = Rc::new(on_back);
    }
    /// What a gesture's start, steps and cancellation do (a predictive handler's).
    pub fn set_gesture(
        &self,
        on_started: impl Fn(BackEventCompat) + 'static,
        on_progressed: impl Fn(BackEventCompat) + 'static,
        on_cancelled: impl Fn() + 'static,
    ) {
        *self.on_started.borrow_mut() = Some(Rc::new(on_started));
        *self.on_progressed.borrow_mut() = Some(Rc::new(on_progressed));
        *self.on_cancelled.borrow_mut() = Some(Rc::new(on_cancelled));
    }
    /// The window it belongs to (none: the host's).
    pub fn set_scope(&self, scope: Option<BackScope>) {
        *self.scope.borrow_mut() = scope;
    }
    pub fn scope(&self) -> Option<BackScope> {
        self.scope.borrow().clone()
    }

    fn back(&self) {
        let f = self.on_back.borrow().clone();
        f();
    }
    fn started(&self, event: BackEventCompat) {
        let f = self.on_started.borrow().clone();
        if let Some(f) = f {
            f(event);
        }
    }
    fn progressed(&self, event: BackEventCompat) {
        let f = self.on_progressed.borrow().clone();
        if let Some(f) = f {
            f(event);
        }
    }
    fn cancelled(&self) {
        let f = self.on_cancelled.borrow().clone();
        if let Some(f) = f {
            f();
        }
    }
}

/// The host's back handlers, oldest first, and the one a gesture under way goes to. Nothing is borrowed while a
/// handler runs.
#[derive(Default)]
pub struct BackDispatcher {
    handlers: RefCell<Vec<Weak<BackCallback>>>,
    gesture: RefCell<Option<Rc<BackCallback>>>,
}

impl BackDispatcher {
    /// Adds a handler (held by its owner: gone with it); the latest added comes first.
    pub fn add(&self, callback: &Rc<BackCallback>) {
        self.handlers.borrow_mut().push(Rc::downgrade(callback));
    }

    /// Removes a handler; a gesture going to it is cancelled.
    pub fn remove(&self, callback: &Rc<BackCallback>) {
        let w = Rc::downgrade(callback);
        self.handlers.borrow_mut().retain(|x| !x.ptr_eq(&w));
        let gesture = self.gesture.borrow().clone();
        if gesture.is_some_and(|g| Rc::ptr_eq(&g, callback)) {
            self.gesture.borrow_mut().take();
            callback.cancelled();
        }
    }

    /// The handler that would take back now (C++'s `backTarget`): the most recent enabled one of `scope`'s window
    /// (none: the host's).
    pub fn target(&self, scope: Option<&BackScope>) -> Option<Rc<BackCallback>> {
        self.handlers.borrow_mut().retain(|w| w.strong_count() > 0);
        let handlers: Vec<Rc<BackCallback>> = self.handlers.borrow().iter().filter_map(Weak::upgrade).collect();
        handlers.into_iter().rev().find(|h| h.enabled.get() && h.scope.borrow().as_ref() == scope)
    }

    /// A gesture started: it goes to the handler that would take back (a gesture never ended is cancelled first).
    pub fn started(&self, scope: Option<&BackScope>, event: BackEventCompat) {
        self.cancelled();
        let target = self.target(scope);
        *self.gesture.borrow_mut() = target.clone();
        if let Some(t) = target {
            t.started(event);
        }
    }

    pub fn progressed(&self, event: BackEventCompat) {
        let gesture = self.gesture.borrow().clone();
        if let Some(g) = gesture {
            g.progressed(event);
        }
    }

    /// The gesture under way was called off.
    pub fn cancelled(&self) {
        let gesture = self.gesture.borrow_mut().take();
        if let Some(g) = gesture {
            g.cancelled();
        }
    }

    /// The gesture under way ended: whether its handler took back (it is still the one that would; else it is
    /// cancelled and the caller goes back the plain way).
    pub(crate) fn gesture_invoked(&self, scope: Option<&BackScope>) -> bool {
        let Some(g) = self.gesture.borrow_mut().take() else { return false };
        if self.target(scope).is_some_and(|t| Rc::ptr_eq(&t, &g)) {
            g.back();
            return true;
        }
        g.cancelled();
        false
    }

    /// Back pressed (not a gesture): the handler that would take it runs; whether there was one.
    pub(crate) fn pressed(&self, scope: Option<&BackScope>) -> bool {
        match self.target(scope) {
            Some(t) => {
                t.back();
                true
            }
            None => false,
        }
    }
}

/// Handles back while `enabled` (Compose's `BackHandler`): the most recently composed enabled handler takes it;
/// in a focusable popup or a dialog, it belongs to that window.
#[kompozable]
pub fn BackHandler(#[default(true)] enabled: bool, on_back: impl Fn() + 'static) {
    let callback = remember(|| BackCallback::new(|| {}));
    callback.set_on_back(on_back);
    callback.set_enabled(enabled);
    callback.set_scope(LocalBackScope.current());
    RegisterBackCallback(callback);
}

/// Registers a handler with the host's dispatcher while it is in the composition.
#[kompozable(inline)]
fn RegisterBackCallback(callback: Rc<BackCallback>) {
    let dispatcher = LocalBackDispatcher.current();
    DisposableEffect(dispatcher.clone(), move || {
        let d = dispatcher.and_then(|d| d.get());
        if let Some(d) = &d {
            d.add(&callback);
        }
        on_dispose(move || {
            if let Some(d) = d {
                d.remove(&callback);
            }
        })
    });
}

/// One run of a predictive handler's `on_back` (androidx's OnBackInstance): its coroutine and the channel its
/// flow reads the gesture's steps from.
struct PredictiveRun {
    on_back: RefCell<Rc<dyn Fn(Flow<BackEventCompat>) -> std::pin::Pin<Box<dyn Future<Output = ()>>>>>,
    scope: RefCell<Option<CoroutineScope>>,
    steps: RefCell<Option<Channel<BackEventCompat>>>,
    predictive: Cell<bool>,
}

impl PredictiveRun {
    fn start(&self, dispatcher: &kompoz_coroutines::Dispatcher, gesture: bool) {
        self.cancel();
        let steps = Channel::buffered();
        let scope = CoroutineScope::new(dispatcher);
        let on_back = self.on_back.borrow().clone();
        let flow = steps.receive_as_flow();
        *self.steps.borrow_mut() = Some(steps);
        *self.scope.borrow_mut() = Some(scope.clone());
        self.predictive.set(gesture);
        scope.launch(on_back(flow));
    }

    fn cancel(&self) {
        if let Some(s) = self.scope.borrow_mut().take() {
            s.cancel();
        }
        if let Some(c) = self.steps.borrow_mut().take() {
            c.cancel();
        }
        self.predictive.set(false);
    }
}

/// Handles back with the gesture's progress (Compose's `PredictiveBackHandler`): `on_back` runs in a coroutine
/// from the gesture's start, its flow the gesture's steps; the gesture done, the flow ends and the coroutine goes
/// on to its end; called off, the coroutine is cancelled. A plain press runs it with a flow that ends at once.
#[kompozable]
pub fn PredictiveBackHandler<Fut: Future<Output = ()> + 'static>(
    #[default(true)] enabled: bool,
    on_back: impl Fn(Flow<BackEventCompat>) -> Fut + 'static,
) {
    let dispatcher = LocalDispatcher.current().expect("kompoz: no dispatcher provided (LocalDispatcher)");
    let run = remember(|| {
        Rc::new(PredictiveRun {
            on_back: RefCell::new(Rc::new(|_| Box::pin(async {}))),
            scope: RefCell::new(None),
            steps: RefCell::new(None),
            predictive: Cell::new(false),
        })
    });
    *run.on_back.borrow_mut() = Rc::new(move |flow| Box::pin(on_back(flow)));
    let callback = remember(|| BackCallback::new(|| {}));
    let (r, d) = (run.clone(), dispatcher.clone());
    callback.set_on_back(move || {
        if !r.predictive.get() {
            r.start(&d, false);
        }
        if let Some(steps) = r.steps.borrow().as_ref() {
            steps.close();
        }
        r.predictive.set(false);
    });
    let (r1, r2, r3, d1) = (run.clone(), run.clone(), run.clone(), dispatcher);
    callback.set_gesture(
        move |_| r1.start(&d1, true),
        move |event| {
            if let Some(steps) = r2.steps.borrow().as_ref() {
                steps.try_send(event);
            }
        },
        move || r3.cancel(),
    );
    callback.set_enabled(enabled);
    callback.set_scope(LocalBackScope.current());
    RegisterBackCallback(callback);
    let r = run.clone();
    DisposableEffect((), move || on_dispose(move || r.cancel()));
}

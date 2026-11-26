//! NavigationEventHandler.kt: a handler's state (its infos, its transition state, whether it takes back and
//! forward) and what it does (Kotlin's open `on…` methods: [`NavigationEventHandlerCallbacks`]).

use crate::dispatcher::{DispatcherCore, NavigationEventDispatcher};
use crate::event::{NavigationEvent, NavigationEventInfo, NavigationEventTransitionState};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

/// What a handler does at each step of a gesture (Kotlin's open members of the abstract class
/// `NavigationEventHandler`, a subclass's overrides): each is given the handler (Kotlin's `this`: its
/// `transition_state` is already updated when it is called). A completion it doesn't take panics, as Kotlin's
/// default `onBackCompleted` throws: a handler that can be completed must say what it does. The Kotlin syntax's
/// `object : NavigationEventHandler<T>(info, isBackEnabled = true) { override fun onBackCompleted() { … } }` is a
/// [`NavigationEventHandler`] of an implementation of it.
///
/// Kotlin: `abstract class NavigationEventHandler<T>(initialInfo: T, isBackEnabled: Boolean, isForwardEnabled: Boolean)`
pub trait NavigationEventHandlerCallbacks<T: NavigationEventInfo + Clone>: 'static {
    /// A back gesture started (predictive: a press goes straight to its completion).
    fn on_back_started(&self, handler: &NavigationEventHandler<T>, event: NavigationEvent) {
        let _ = (handler, event);
    }
    /// A step of a back gesture.
    fn on_back_progressed(&self, handler: &NavigationEventHandler<T>, event: NavigationEvent) {
        let _ = (handler, event);
    }
    /// Back done: navigate.
    ///
    /// # Panics
    /// Not overridden ("A handler that receives a 'backCompleted' event must override 'onBackCompleted()' to handle
    /// the callback.", Kotlin's UnsupportedOperationException).
    fn on_back_completed(&self, handler: &NavigationEventHandler<T>) {
        let _ = handler;
        panic!(
            "A handler that receives a 'backCompleted' event must override 'onBackCompleted()' to handle the callback."
        );
    }
    /// Back called off: the UI goes back to how it was.
    fn on_back_cancelled(&self, handler: &NavigationEventHandler<T>) {
        let _ = handler;
    }
    /// A forward gesture started.
    fn on_forward_started(&self, handler: &NavigationEventHandler<T>, event: NavigationEvent) {
        let _ = (handler, event);
    }
    /// A step of a forward gesture.
    fn on_forward_progressed(&self, handler: &NavigationEventHandler<T>, event: NavigationEvent) {
        let _ = (handler, event);
    }
    /// Forward done.
    ///
    /// # Panics
    /// Not overridden (Kotlin's UnsupportedOperationException).
    fn on_forward_completed(&self, handler: &NavigationEventHandler<T>) {
        let _ = handler;
        panic!(
            "A handler that receives a 'forwardCompleted' event must override 'onForwardCompleted()' to handle the \
             callback."
        );
    }
    /// Forward called off.
    fn on_forward_cancelled(&self, handler: &NavigationEventHandler<T>) {
        let _ = handler;
    }
}

pub(crate) struct HandlerCore<T: NavigationEventInfo + Clone> {
    current_info: RefCell<Rc<T>>,
    back_info: RefCell<Vec<Rc<T>>>,
    forward_info: RefCell<Vec<Rc<T>>>,
    transition_state: Cell<NavigationEventTransitionState>,
    is_back_enabled: Cell<bool>,
    is_forward_enabled: Cell<bool>,
    dispatcher: RefCell<Weak<DispatcherCore>>,
    callbacks: Box<dyn NavigationEventHandlerCallbacks<T>>,
}

/// A handler of navigation gestures (Compose's abstract class `NavigationEventHandler<T>`): what it tells of the
/// app's navigation — the active destination ([`current_info`](Self::current_info)), what back and forward lead to
/// ([`back_info`](Self::back_info), [`forward_info`](Self::forward_info), nearest first: not stacks, what renders
/// the affordances) —, its gesture's [`transition_state`](Self::transition_state), whether it takes back and
/// forward gestures, and what it does with them (its [`NavigationEventHandlerCallbacks`]). Registered with a
/// [`NavigationEventDispatcher`] (`add_handler`) it gets the events while it and its dispatcher are enabled. A cheap
/// handle: clones are the same handler (equal by identity).
pub struct NavigationEventHandler<T: NavigationEventInfo + Clone> {
    pub(crate) core: Rc<HandlerCore<T>>,
}

impl<T: NavigationEventInfo + Clone> Clone for NavigationEventHandler<T> {
    fn clone(&self) -> Self {
        NavigationEventHandler { core: self.core.clone() }
    }
}

impl<T: NavigationEventInfo + Clone> PartialEq for NavigationEventHandler<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.core, &other.core)
    }
}

impl<T: NavigationEventInfo + Clone> fmt::Debug for NavigationEventHandler<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NavigationEventHandler@{:p}", Rc::as_ptr(&self.core))
    }
}

impl<T: NavigationEventInfo + Clone> NavigationEventHandler<T> {
    /// A handler of back gestures (forward disabled; Kotlin's `NavigationEventHandler(initialInfo, isBackEnabled)`).
    pub fn new(
        initial_info: T,
        is_back_enabled: bool,
        callbacks: impl NavigationEventHandlerCallbacks<T>,
    ) -> NavigationEventHandler<T> {
        NavigationEventHandler::with_forward(initial_info, is_back_enabled, false, callbacks)
    }

    /// A handler of back and forward gestures (Kotlin's `NavigationEventHandler(initialInfo, isBackEnabled,
    /// isForwardEnabled)`).
    pub fn with_forward(
        initial_info: T,
        is_back_enabled: bool,
        is_forward_enabled: bool,
        callbacks: impl NavigationEventHandlerCallbacks<T>,
    ) -> NavigationEventHandler<T> {
        NavigationEventHandler {
            core: Rc::new(HandlerCore {
                current_info: RefCell::new(Rc::new(initial_info)),
                back_info: RefCell::new(Vec::new()),
                forward_info: RefCell::new(Vec::new()),
                transition_state: Cell::new(NavigationEventTransitionState::Idle),
                is_back_enabled: Cell::new(is_back_enabled),
                is_forward_enabled: Cell::new(is_forward_enabled),
                dispatcher: RefCell::new(Weak::new()),
                callbacks: Box::new(callbacks),
            }),
        }
    }

    /// The active destination's info.
    pub fn current_info(&self) -> T {
        T::clone(&self.core.current_info.borrow())
    }

    /// What back leads to (nearest first; empty: nothing).
    pub fn back_info(&self) -> Vec<T> {
        self.core.back_info.borrow().iter().map(|i| T::clone(i)).collect()
    }

    /// What forward leads to (nearest first; empty: nothing).
    pub fn forward_info(&self) -> Vec<T> {
        self.core.forward_info.borrow().iter().map(|i| T::clone(i)).collect()
    }

    /// Its gesture's state: set by the dispatcher before the `on…` callbacks are called.
    pub fn transition_state(&self) -> NavigationEventTransitionState {
        self.core.transition_state.get()
    }

    /// Whether it takes back gestures: false while its dispatcher is disabled, whatever it was set to.
    pub fn is_back_enabled(&self) -> bool {
        if self.dispatcher().is_some_and(|d| !d.is_enabled()) { false } else { self.core.is_back_enabled.get() }
    }

    /// Sets whether it takes back gestures (its own setting); its dispatcher reconsiders which handler is active.
    pub fn set_is_back_enabled(&self, value: bool) {
        if self.core.is_back_enabled.get() == value {
            return;
        }
        self.core.is_back_enabled.set(value);
        if let Some(d) = self.dispatcher() {
            d.core.shared_processor.refresh_enabled_handlers();
        }
    }

    /// Whether it takes forward gestures: false while its dispatcher is disabled, whatever it was set to.
    pub fn is_forward_enabled(&self) -> bool {
        if self.dispatcher().is_some_and(|d| !d.is_enabled()) { false } else { self.core.is_forward_enabled.get() }
    }

    /// Sets whether it takes forward gestures; its dispatcher reconsiders which handler is active.
    pub fn set_is_forward_enabled(&self, value: bool) {
        if self.core.is_forward_enabled.get() == value {
            return;
        }
        self.core.is_forward_enabled.set(value);
        if let Some(d) = self.dispatcher() {
            d.core.shared_processor.refresh_enabled_handlers();
        }
    }

    /// The dispatcher it is registered with.
    pub(crate) fn dispatcher(&self) -> Option<NavigationEventDispatcher> {
        self.core.dispatcher.borrow().upgrade().map(|core| NavigationEventDispatcher { core })
    }

    /// Removes it from the dispatcher it is registered with (nothing if it isn't).
    pub fn remove(&self) {
        if let Some(d) = self.dispatcher() {
            d.remove_handler(&self.erased());
        }
    }

    /// Sets what it tells of the navigation: the active destination, what back and forward lead to (nearest first;
    /// empty: no affordance that way). The history changes if it is the active handler.
    pub fn set_info(&self, current_info: T, back_info: Vec<T>, forward_info: Vec<T>) {
        *self.core.current_info.borrow_mut() = Rc::new(current_info);
        *self.core.back_info.borrow_mut() = back_info.into_iter().map(Rc::new).collect();
        *self.core.forward_info.borrow_mut() = forward_info.into_iter().map(Rc::new).collect();
        if let Some(d) = self.dispatcher() {
            d.core.shared_processor.update_enabled_handler_info(Some(&self.erased()));
        }
    }

    /// It as the dispatcher keeps it.
    pub(crate) fn erased(&self) -> Rc<dyn AnyHandler> {
        Rc::new(self.clone())
    }
}

/// A handler of any info type, as the dispatcher and its processor keep it (Kotlin's `NavigationEventHandler<*>`).
pub(crate) trait AnyHandler {
    /// Its identity.
    fn id(&self) -> *const ();
    fn is_back_enabled(&self) -> bool;
    fn is_forward_enabled(&self) -> bool;
    fn current_info(&self) -> Rc<dyn NavigationEventInfo>;
    fn back_info(&self) -> Vec<Rc<dyn NavigationEventInfo>>;
    fn forward_info(&self) -> Vec<Rc<dyn NavigationEventInfo>>;
    fn has_dispatcher(&self) -> bool;
    fn set_dispatcher(&self, dispatcher: Option<&NavigationEventDispatcher>);
    fn remove(&self);
    fn do_on_back_started(&self, event: NavigationEvent);
    fn do_on_back_progressed(&self, event: NavigationEvent);
    fn do_on_back_completed(&self);
    fn do_on_back_cancelled(&self);
    fn do_on_forward_started(&self, event: NavigationEvent);
    fn do_on_forward_progressed(&self, event: NavigationEvent);
    fn do_on_forward_completed(&self);
    fn do_on_forward_cancelled(&self);
}

impl dyn AnyHandler {
    pub(crate) fn same(&self, other: &dyn AnyHandler) -> bool {
        self.id() == other.id()
    }
}

impl<T: NavigationEventInfo + Clone> AnyHandler for NavigationEventHandler<T> {
    fn id(&self) -> *const () {
        Rc::as_ptr(&self.core) as *const ()
    }
    fn is_back_enabled(&self) -> bool {
        NavigationEventHandler::is_back_enabled(self)
    }
    fn is_forward_enabled(&self) -> bool {
        NavigationEventHandler::is_forward_enabled(self)
    }
    fn current_info(&self) -> Rc<dyn NavigationEventInfo> {
        self.core.current_info.borrow().clone()
    }
    fn back_info(&self) -> Vec<Rc<dyn NavigationEventInfo>> {
        self.core.back_info.borrow().iter().map(|i| i.clone() as Rc<dyn NavigationEventInfo>).collect()
    }
    fn forward_info(&self) -> Vec<Rc<dyn NavigationEventInfo>> {
        self.core.forward_info.borrow().iter().map(|i| i.clone() as Rc<dyn NavigationEventInfo>).collect()
    }
    fn has_dispatcher(&self) -> bool {
        self.core.dispatcher.borrow().strong_count() > 0
    }
    fn set_dispatcher(&self, dispatcher: Option<&NavigationEventDispatcher>) {
        *self.core.dispatcher.borrow_mut() = dispatcher.map(|d| Rc::downgrade(&d.core)).unwrap_or_default();
    }
    fn remove(&self) {
        NavigationEventHandler::remove(self);
    }
    fn do_on_back_started(&self, event: NavigationEvent) {
        self.core.transition_state.set(NavigationEventTransitionState::in_progress(
            event,
            NavigationEventTransitionState::TRANSITIONING_BACK,
        ));
        self.core.callbacks.on_back_started(self, event);
    }
    fn do_on_back_progressed(&self, event: NavigationEvent) {
        self.core.transition_state.set(NavigationEventTransitionState::in_progress(
            event,
            NavigationEventTransitionState::TRANSITIONING_BACK,
        ));
        self.core.callbacks.on_back_progressed(self, event);
    }
    fn do_on_back_completed(&self) {
        self.core.transition_state.set(NavigationEventTransitionState::Idle);
        self.core.callbacks.on_back_completed(self);
    }
    fn do_on_back_cancelled(&self) {
        self.core.transition_state.set(NavigationEventTransitionState::Idle);
        self.core.callbacks.on_back_cancelled(self);
    }
    fn do_on_forward_started(&self, event: NavigationEvent) {
        self.core.transition_state.set(NavigationEventTransitionState::in_progress(
            event,
            NavigationEventTransitionState::TRANSITIONING_FORWARD,
        ));
        self.core.callbacks.on_forward_started(self, event);
    }
    fn do_on_forward_progressed(&self, event: NavigationEvent) {
        self.core.transition_state.set(NavigationEventTransitionState::in_progress(
            event,
            NavigationEventTransitionState::TRANSITIONING_FORWARD,
        ));
        self.core.callbacks.on_forward_progressed(self, event);
    }
    fn do_on_forward_completed(&self) {
        self.core.transition_state.set(NavigationEventTransitionState::Idle);
        self.core.callbacks.on_forward_completed(self);
    }
    fn do_on_forward_cancelled(&self) {
        self.core.transition_state.set(NavigationEventTransitionState::Idle);
        self.core.callbacks.on_forward_cancelled(self);
    }
}

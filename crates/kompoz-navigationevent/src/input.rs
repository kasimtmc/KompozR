//! NavigationEventInput.kt and DirectNavigationEventInput.kt: what turns a platform's signals into navigation
//! events.

use crate::dispatcher::{DispatcherCore, NavigationEventDispatcher};
use crate::event::{NavigationEvent, NavigationEventHistory, NavigationEventTransitionState};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

const BACK: i32 = NavigationEventTransitionState::TRANSITIONING_BACK;
const FORWARD: i32 = NavigationEventTransitionState::TRANSITIONING_FORWARD;

/// What an input hears of its dispatcher (Kotlin's open members of the abstract class `NavigationEventInput`, a
/// subclass's overrides); each is given the input (Kotlin's `this`: what its `dispatch_on…` calls are made on).
pub trait NavigationEventInputCallbacks: 'static {
    /// Added to `dispatcher` (an input is added to one dispatcher at a time).
    fn on_added(&self, input: &NavigationEventInput, dispatcher: &NavigationEventDispatcher) {
        let _ = (input, dispatcher);
    }
    /// Removed from its dispatcher (`remove_input`, or the dispatcher disposed).
    fn on_removed(&self, input: &NavigationEventInput) {
        let _ = input;
    }
    /// Whether the dispatcher has enabled handlers of the input's priority (all of them for an input added without
    /// one) changed: a system back input registers with the platform only while it has (told at once when added).
    fn on_has_enabled_handlers_changed(&self, input: &NavigationEventInput, has_enabled_handlers: bool) {
        let _ = (input, has_enabled_handlers);
    }
    /// The dispatcher's history changed (told at once when added).
    fn on_history_changed(&self, input: &NavigationEventInput, history: &NavigationEventHistory) {
        let _ = (input, history);
    }
}

/// An input that only dispatches (Kotlin's subclass overriding nothing).
impl NavigationEventInputCallbacks for () {}

pub(crate) struct InputCore {
    pub(crate) dispatcher: RefCell<Weak<DispatcherCore>>,
    is_predictive_back_in_progress: Cell<bool>,
    is_predictive_forward_in_progress: Cell<bool>,
    callbacks: Box<dyn NavigationEventInputCallbacks>,
}

/// A source of navigation events (Compose's abstract class `NavigationEventInput`): turns a platform's signals —
/// a back gesture, a key, a button — into events sent to the [`NavigationEventDispatcher`] it is added to
/// (`add_input`), with its `dispatch_on…` calls; hears the dispatcher through its
/// [`NavigationEventInputCallbacks`]. A press (a completion without a start) is sent as Compose sends it: started
/// without an event (nothing told to the handler), then completed. A cheap handle: clones are the same input.
#[derive(Clone)]
pub struct NavigationEventInput {
    pub(crate) core: Rc<InputCore>,
}

impl PartialEq for NavigationEventInput {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.core, &other.core)
    }
}

impl fmt::Debug for NavigationEventInput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NavigationEventInput@{:p}", Rc::as_ptr(&self.core))
    }
}

impl NavigationEventInput {
    /// An input hearing its dispatcher through `callbacks` (`()`: hearing nothing).
    pub fn new(callbacks: impl NavigationEventInputCallbacks) -> NavigationEventInput {
        NavigationEventInput {
            core: Rc::new(InputCore {
                dispatcher: RefCell::new(Weak::new()),
                is_predictive_back_in_progress: Cell::new(false),
                is_predictive_forward_in_progress: Cell::new(false),
                callbacks: Box::new(callbacks),
            }),
        }
    }

    /// The dispatcher it is added to.
    pub(crate) fn dispatcher(&self) -> Option<NavigationEventDispatcher> {
        self.core.dispatcher.borrow().upgrade().map(|core| NavigationEventDispatcher { core })
    }

    fn checked_dispatcher(&self) -> NavigationEventDispatcher {
        self.dispatcher().unwrap_or_else(|| panic!("This input is not added to any dispatcher."))
    }

    pub(crate) fn do_on_added(&self, dispatcher: &NavigationEventDispatcher) {
        self.core.callbacks.on_added(self, dispatcher);
    }
    pub(crate) fn do_on_removed(&self) {
        self.core.callbacks.on_removed(self);
    }
    pub(crate) fn do_on_has_enabled_handlers_changed(&self, has_enabled_handlers: bool) {
        self.core.callbacks.on_has_enabled_handlers_changed(self, has_enabled_handlers);
    }
    pub(crate) fn do_on_history_changed(&self, history: &NavigationEventHistory) {
        self.core.callbacks.on_history_changed(self, history);
    }

    /// A back gesture started (the dispatcher's transition state goes `InProgress`); ignored while one from this
    /// input is under way. (Kotlin's protected `dispatchOnBackStarted`: for the input's own code.)
    ///
    /// # Panics
    /// Not added to a dispatcher, or the dispatcher disposed.
    pub fn dispatch_on_back_started(&self, event: NavigationEvent) {
        let dispatcher = self.checked_dispatcher();
        if !self.core.is_predictive_back_in_progress.get() {
            dispatcher.dispatch_on_started(self, BACK, Some(event));
            self.core.is_predictive_back_in_progress.set(true);
        }
    }

    /// A step of the back gesture under way (ignored without one).
    ///
    /// # Panics
    /// Not added to a dispatcher, or the dispatcher disposed.
    pub fn dispatch_on_back_progressed(&self, event: NavigationEvent) {
        let dispatcher = self.checked_dispatcher();
        if self.core.is_predictive_back_in_progress.get() {
            dispatcher.dispatch_on_progressed(self, BACK, event);
        }
    }

    /// The back gesture was called off (terminal: the transition state goes `Idle`); without a gesture, a press
    /// called off.
    ///
    /// # Panics
    /// Not added to a dispatcher, or the dispatcher disposed.
    pub fn dispatch_on_back_cancelled(&self) {
        let dispatcher = self.checked_dispatcher();
        if !self.core.is_predictive_back_in_progress.get() {
            dispatcher.dispatch_on_started(self, BACK, None);
        }
        dispatcher.dispatch_on_cancelled(self, BACK);
        self.core.is_predictive_back_in_progress.set(false);
    }

    /// Back completed (terminal: navigate): the gesture under way's end, or a press.
    ///
    /// # Panics
    /// Not added to a dispatcher, or the dispatcher disposed.
    pub fn dispatch_on_back_completed(&self) {
        let dispatcher = self.checked_dispatcher();
        if !self.core.is_predictive_back_in_progress.get() {
            dispatcher.dispatch_on_started(self, BACK, None);
        }
        dispatcher.dispatch_on_completed(self, BACK);
        self.core.is_predictive_back_in_progress.set(false);
    }

    /// A forward gesture started; ignored while one from this input is under way.
    ///
    /// # Panics
    /// Not added to a dispatcher, or the dispatcher disposed.
    pub fn dispatch_on_forward_started(&self, event: NavigationEvent) {
        let dispatcher = self.checked_dispatcher();
        if !self.core.is_predictive_forward_in_progress.get() {
            dispatcher.dispatch_on_started(self, FORWARD, Some(event));
            self.core.is_predictive_forward_in_progress.set(true);
        }
    }

    /// A step of the forward gesture under way.
    ///
    /// # Panics
    /// Not added to a dispatcher, or the dispatcher disposed.
    pub fn dispatch_on_forward_progressed(&self, event: NavigationEvent) {
        let dispatcher = self.checked_dispatcher();
        if self.core.is_predictive_forward_in_progress.get() {
            dispatcher.dispatch_on_progressed(self, FORWARD, event);
        }
    }

    /// The forward gesture was called off (or a press, without one).
    ///
    /// # Panics
    /// Not added to a dispatcher, or the dispatcher disposed.
    pub fn dispatch_on_forward_cancelled(&self) {
        let dispatcher = self.checked_dispatcher();
        if !self.core.is_predictive_forward_in_progress.get() {
            dispatcher.dispatch_on_started(self, FORWARD, None);
        }
        dispatcher.dispatch_on_cancelled(self, FORWARD);
        self.core.is_predictive_forward_in_progress.set(false);
    }

    /// Forward completed: the gesture under way's end, or a press.
    ///
    /// # Panics
    /// Not added to a dispatcher, or the dispatcher disposed.
    pub fn dispatch_on_forward_completed(&self) {
        let dispatcher = self.checked_dispatcher();
        if !self.core.is_predictive_forward_in_progress.get() {
            dispatcher.dispatch_on_started(self, FORWARD, None);
        }
        dispatcher.dispatch_on_completed(self, FORWARD);
        self.core.is_predictive_forward_in_progress.set(false);
    }
}

/// An input to send events with directly, without a type of one's own (Compose's `DirectNavigationEventInput`):
/// what tests and simple hosts use. Derefs to its [`NavigationEventInput`] (what `add_input` takes).
#[derive(Clone, PartialEq, Debug)]
pub struct DirectNavigationEventInput {
    input: NavigationEventInput,
}

impl Default for DirectNavigationEventInput {
    fn default() -> Self {
        DirectNavigationEventInput::new()
    }
}

impl std::ops::Deref for DirectNavigationEventInput {
    type Target = NavigationEventInput;
    fn deref(&self) -> &NavigationEventInput {
        &self.input
    }
}

impl DirectNavigationEventInput {
    /// Kotlin: `DirectNavigationEventInput()`
    pub fn new() -> DirectNavigationEventInput {
        DirectNavigationEventInput { input: NavigationEventInput::new(()) }
    }

    pub fn back_started(&self, event: NavigationEvent) {
        self.input.dispatch_on_back_started(event);
    }
    pub fn back_progressed(&self, event: NavigationEvent) {
        self.input.dispatch_on_back_progressed(event);
    }
    pub fn back_cancelled(&self) {
        self.input.dispatch_on_back_cancelled();
    }
    pub fn back_completed(&self) {
        self.input.dispatch_on_back_completed();
    }
    pub fn forward_started(&self, event: NavigationEvent) {
        self.input.dispatch_on_forward_started(event);
    }
    pub fn forward_progressed(&self, event: NavigationEvent) {
        self.input.dispatch_on_forward_progressed(event);
    }
    pub fn forward_cancelled(&self) {
        self.input.dispatch_on_forward_cancelled();
    }
    pub fn forward_completed(&self) {
        self.input.dispatch_on_forward_completed();
    }
}

/// Kotlin: `DirectNavigationEventInput()`
pub fn DirectNavigationEventInput() -> DirectNavigationEventInput {
    DirectNavigationEventInput::new()
}

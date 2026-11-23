//! NavigationEventDispatcher.kt, NavigationEventDispatcherOwner.kt, OnBackCompletedFallback.kt,
//! OnForwardCompletedFallback.kt.

use crate::NavigationEventInfo;
use crate::event::{NavigationEvent, NavigationEventHistory, NavigationEventTransitionState};
use crate::handler::{AnyHandler, NavigationEventHandler};
use crate::input::NavigationEventInput;
use crate::processor::NavigationEventProcessor;
use kompoz_coroutines::StateFlow;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fmt;
use std::rc::{Rc, Weak};

/// The last resort of a back completion no handler takes (Compose's fun interface `OnBackCompletedFallback`: an
/// activity's finish); a closure is one. Not called when back is called off.
pub trait OnBackCompletedFallback {
    fn on_back_completed_fallback(&self);
}

impl<F: Fn()> OnBackCompletedFallback for F {
    fn on_back_completed_fallback(&self) {
        self()
    }
}

/// The last resort of a forward completion no handler takes (Compose's fun interface `OnForwardCompletedFallback`);
/// a closure is one.
pub trait OnForwardCompletedFallback {
    fn on_forward_completed_fallback(&self);
}

impl<F: Fn()> OnForwardCompletedFallback for F {
    fn on_forward_completed_fallback(&self) {
        self()
    }
}

/// What has a [`NavigationEventDispatcher`] to register handlers with (Compose's `NavigationEventDispatcherOwner`):
/// the host's (the app's root dispatcher, which its inputs send to), or one of
/// `remember_navigation_event_dispatcher_owner`'s.
pub trait NavigationEventDispatcherOwner {
    fn navigation_event_dispatcher(&self) -> NavigationEventDispatcher;
}

/// Owners compare by identity (Kotlin's objects): an `Rc<dyn NavigationEventDispatcherOwner>` keys effects.
impl PartialEq for dyn NavigationEventDispatcherOwner {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const Self as *const (), other as *const Self as *const ())
    }
}

pub(crate) struct DispatcherCore {
    parent: RefCell<Option<NavigationEventDispatcher>>,
    on_back_completed_fallback: Option<Box<dyn OnBackCompletedFallback>>,
    on_forward_completed_fallback: Option<Box<dyn OnForwardCompletedFallback>>,
    is_disposed: Cell<bool>,
    is_enabled: Cell<bool>,
    pub(crate) shared_processor: Rc<NavigationEventProcessor>,
    child_dispatchers: RefCell<Vec<Weak<DispatcherCore>>>,
    handlers: RefCell<Vec<Rc<dyn AnyHandler>>>,
    inputs: RefCell<Vec<NavigationEventInput>>,
}

/// A dispatcher of navigation events, in a tree (Compose's `NavigationEventDispatcher`): where handlers
/// ([`add_handler`](Self::add_handler)) and inputs ([`add_input`](Self::add_input)) of a part of the UI are
/// registered. A child shares its root's processor — one order of handlers for the whole tree: overlay ones before
/// default ones, the latest added first — and is disabled while its parent is. [`dispose`](Self::dispose) it when
/// its owner goes (its handlers and inputs, and its children's, are removed). A cheap handle: clones are the same
/// dispatcher (equal by identity).
#[derive(Clone)]
pub struct NavigationEventDispatcher {
    pub(crate) core: Rc<DispatcherCore>,
}

impl PartialEq for NavigationEventDispatcher {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.core, &other.core)
    }
}

impl fmt::Debug for NavigationEventDispatcher {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NavigationEventDispatcher@{:p}", Rc::as_ptr(&self.core))
    }
}

impl Default for NavigationEventDispatcher {
    fn default() -> Self {
        NavigationEventDispatcher::new()
    }
}

/// Kotlin's overloaded constructors of `NavigationEventDispatcher`, as their arguments.
pub trait NavigationEventDispatcherArgs {
    fn navigation_event_dispatcher(self) -> NavigationEventDispatcher;
}

/// Kotlin: `NavigationEventDispatcher()`
impl NavigationEventDispatcherArgs for () {
    fn navigation_event_dispatcher(self) -> NavigationEventDispatcher {
        NavigationEventDispatcher::new()
    }
}

/// Kotlin: `NavigationEventDispatcher(onBackCompletedFallback: OnBackCompletedFallback)`
impl<F: Fn() + 'static> NavigationEventDispatcherArgs for F {
    fn navigation_event_dispatcher(self) -> NavigationEventDispatcher {
        NavigationEventDispatcher::with_fallback(self)
    }
}

/// Kotlin: `NavigationEventDispatcher(onBackCompletedFallback: OnBackCompletedFallback, onForwardCompletedFallback: OnForwardCompletedFallback)`
impl<B: Fn() + 'static, F: Fn() + 'static> NavigationEventDispatcherArgs for (B, F) {
    fn navigation_event_dispatcher(self) -> NavigationEventDispatcher {
        NavigationEventDispatcher::with_fallbacks(self.0, self.1)
    }
}

/// Kotlin: `NavigationEventDispatcher(parent: NavigationEventDispatcher)`
impl NavigationEventDispatcherArgs for NavigationEventDispatcher {
    fn navigation_event_dispatcher(self) -> NavigationEventDispatcher {
        NavigationEventDispatcher::with_parent(&self)
    }
}

/// Kotlin: `NavigationEventDispatcher(parent: NavigationEventDispatcher)`
impl NavigationEventDispatcherArgs for &NavigationEventDispatcher {
    fn navigation_event_dispatcher(self) -> NavigationEventDispatcher {
        NavigationEventDispatcher::with_parent(self)
    }
}

/// A dispatcher made as Kotlin's constructors make one: `()` a root, a closure a root with a back fallback, two a
/// root with back and forward fallbacks, a dispatcher a child of it.
pub fn NavigationEventDispatcher(args: impl NavigationEventDispatcherArgs) -> NavigationEventDispatcher {
    args.navigation_event_dispatcher()
}

impl NavigationEventDispatcher {
    /// Highest priority: overlays (dialogs, sheets, drawers) get the events before the content.
    pub const PRIORITY_OVERLAY: i32 = 0;
    /// The content's priority (the default).
    pub const PRIORITY_DEFAULT: i32 = 1;

    fn make(
        parent: Option<&NavigationEventDispatcher>,
        on_back_completed_fallback: Option<Box<dyn OnBackCompletedFallback>>,
        on_forward_completed_fallback: Option<Box<dyn OnForwardCompletedFallback>>,
    ) -> NavigationEventDispatcher {
        let shared_processor = match parent {
            Some(p) => p.core.shared_processor.clone(),
            None => Rc::new(NavigationEventProcessor::new()),
        };
        let dispatcher = NavigationEventDispatcher {
            core: Rc::new(DispatcherCore {
                parent: RefCell::new(parent.cloned()),
                on_back_completed_fallback,
                on_forward_completed_fallback,
                is_disposed: Cell::new(false),
                is_enabled: Cell::new(true),
                shared_processor,
                child_dispatchers: RefCell::new(Vec::new()),
                handlers: RefCell::new(Vec::new()),
                inputs: RefCell::new(Vec::new()),
            }),
        };
        if let Some(p) = parent {
            p.core.child_dispatchers.borrow_mut().push(Rc::downgrade(&dispatcher.core));
        }
        dispatcher
    }

    /// A root dispatcher (a navigation hierarchy of its own; inputs send to it): a completion no handler takes
    /// does nothing.
    pub fn new() -> NavigationEventDispatcher {
        NavigationEventDispatcher::make(None, None, None)
    }

    /// A root dispatcher whose back completions no handler takes go to `on_back_completed_fallback` (not its
    /// cancellations).
    pub fn with_fallback(
        on_back_completed_fallback: impl OnBackCompletedFallback + 'static,
    ) -> NavigationEventDispatcher {
        NavigationEventDispatcher::make(None, Some(Box::new(on_back_completed_fallback)), None)
    }

    /// A root dispatcher with back and forward fallbacks.
    pub fn with_fallbacks(
        on_back_completed_fallback: impl OnBackCompletedFallback + 'static,
        on_forward_completed_fallback: impl OnForwardCompletedFallback + 'static,
    ) -> NavigationEventDispatcher {
        NavigationEventDispatcher::make(
            None,
            Some(Box::new(on_back_completed_fallback)),
            Some(Box::new(on_forward_completed_fallback)),
        )
    }

    /// A child of `parent`: its processor (one stream of events), disabled while `parent` is, disposed with it.
    pub fn with_parent(parent: &NavigationEventDispatcher) -> NavigationEventDispatcher {
        NavigationEventDispatcher::make(Some(parent), None, None)
    }

    fn is_disposed(&self) -> bool {
        let parent = self.core.parent.borrow().clone();
        if parent.is_some_and(|p| p.is_disposed()) { true } else { self.core.is_disposed.get() }
    }

    /// Whether it processes events: its own setting (true at first), false while its parent isn't enabled.
    pub fn is_enabled(&self) -> bool {
        let parent = self.core.parent.borrow().clone();
        if parent.is_some_and(|p| !p.is_enabled()) { false } else { self.core.is_enabled.get() }
    }

    /// Sets its own setting (the effective one is also its ancestors').
    ///
    /// # Panics
    /// Disposed.
    pub fn set_is_enabled(&self, value: bool) {
        self.check_invariants();
        if self.core.is_enabled.get() == value {
            return;
        }
        self.core.is_enabled.set(value);
        self.core.shared_processor.refresh_enabled_handlers();
    }

    /// The gesture's state, for the whole tree (`Idle` or `InProgress`, of the active handler's gesture): what
    /// system UI and animations follow, apart from the history.
    pub fn transition_state(&self) -> StateFlow<NavigationEventTransitionState> {
        self.core.shared_processor.transition_state.as_state_flow()
    }

    /// The history, for the whole tree: stable during a gesture, changed when the active handler or its info does.
    pub fn history(&self) -> StateFlow<NavigationEventHistory> {
        self.core.shared_processor.history.as_state_flow()
    }

    /// Adds `handler` at the default priority (Kotlin's `addHandler(handler)`).
    ///
    /// # Panics
    /// The handler registered with another dispatcher; this one disposed.
    pub fn add_handler<T: NavigationEventInfo + Clone>(&self, handler: &NavigationEventHandler<T>) {
        self.add_handler_with_priority(handler, NavigationEventDispatcher::PRIORITY_DEFAULT);
    }

    /// Adds `handler` to get the events at `priority`: all overlay handlers come before the default ones, and
    /// within a priority the latest added comes first. Removed with its `remove()`.
    ///
    /// # Panics
    /// The handler registered with another dispatcher; `priority` not one of the `PRIORITY_` constants; this one
    /// disposed.
    ///
    /// Kotlin: `addHandler(handler: NavigationEventHandler<*>, priority: Int)`
    pub fn add_handler_with_priority<T: NavigationEventInfo + Clone>(
        &self,
        handler: &NavigationEventHandler<T>,
        priority: i32,
    ) {
        self.check_invariants();
        let erased = handler.erased();
        let added = {
            let mut handlers = self.core.handlers.borrow_mut();
            if handlers.iter().any(|h| h.same(&*erased)) {
                false
            } else {
                handlers.push(erased.clone());
                true
            }
        };
        if added {
            self.core.shared_processor.add_handler(self, erased, priority);
        }
    }

    /// `NavigationEventHandler.remove()`.
    pub(crate) fn remove_handler(&self, handler: &Rc<dyn AnyHandler>) {
        let removed = {
            let mut handlers = self.core.handlers.borrow_mut();
            let before = handlers.len();
            handlers.retain(|h| !h.same(&**handler));
            handlers.len() != before
        };
        if removed {
            self.core.shared_processor.remove_handler(handler);
        }
    }

    /// Adds `input` without a priority: it hears whether any handler is enabled (Kotlin's `addInput(input)`). Its
    /// `on_added` is told at once, then the history and whether handlers are enabled; it is removed when this
    /// dispatcher is disposed.
    ///
    /// # Panics
    /// The input added to a dispatcher already; this one disposed.
    pub fn add_input(&self, input: &NavigationEventInput) {
        self.check_invariants();
        if self.track_input(input) {
            self.core.shared_processor.add_input(self, input, -1);
        }
    }

    /// Adds `input` with a priority: it hears whether handlers of that priority are enabled.
    ///
    /// # Panics
    /// `priority` not one of the `PRIORITY_` constants; the input added to a dispatcher already; this one disposed.
    ///
    /// Kotlin: `addInput(input: NavigationEventInput, priority: Int)`
    pub fn add_input_with_priority(&self, input: &NavigationEventInput, priority: i32) {
        self.check_invariants();
        assert!(
            priority == NavigationEventDispatcher::PRIORITY_DEFAULT
                || priority == NavigationEventDispatcher::PRIORITY_OVERLAY,
            "Unsupported priority value: {priority}"
        );
        if self.track_input(input) {
            self.core.shared_processor.add_input(self, input, priority);
        }
    }

    fn track_input(&self, input: &NavigationEventInput) -> bool {
        let mut inputs = self.core.inputs.borrow_mut();
        if inputs.contains(input) {
            return false;
        }
        inputs.push(input.clone());
        true
    }

    /// Removes `input` (its `on_removed` is told).
    ///
    /// # Panics
    /// This one disposed.
    pub fn remove_input(&self, input: &NavigationEventInput) {
        self.check_invariants();
        let removed = {
            let mut inputs = self.core.inputs.borrow_mut();
            let before = inputs.len();
            inputs.retain(|i| i != input);
            inputs.len() != before
        };
        if removed {
            self.core.shared_processor.remove_input(input);
        }
    }

    pub(crate) fn dispatch_on_started(
        &self,
        input: &NavigationEventInput,
        direction: i32,
        event: Option<NavigationEvent>,
    ) {
        self.check_invariants();
        if !self.is_enabled() {
            return;
        }
        self.core.shared_processor.dispatch_on_started(input, direction, event);
    }

    pub(crate) fn dispatch_on_progressed(&self, input: &NavigationEventInput, direction: i32, event: NavigationEvent) {
        self.check_invariants();
        if !self.is_enabled() {
            return;
        }
        self.core.shared_processor.dispatch_on_progressed(input, direction, event);
    }

    pub(crate) fn dispatch_on_completed(&self, input: &NavigationEventInput, direction: i32) {
        self.check_invariants();
        if !self.is_enabled() {
            return;
        }
        self.core.shared_processor.dispatch_on_completed(
            input,
            direction,
            self.core.on_back_completed_fallback.as_deref(),
            self.core.on_forward_completed_fallback.as_deref(),
        );
    }

    pub(crate) fn dispatch_on_cancelled(&self, input: &NavigationEventInput, direction: i32) {
        self.check_invariants();
        if !self.is_enabled() {
            return;
        }
        self.core.shared_processor.dispatch_on_cancelled(input, direction);
    }

    /// Takes it and its descendants out of the hierarchy (terminal): each one's inputs removed (told
    /// `on_removed`), its handlers removed, it unlinked from its parent.
    ///
    /// # Panics
    /// Disposed already.
    pub fn dispose(&self) {
        self.check_invariants();
        self.core.is_disposed.set(true);
        let mut to_dispose: VecDeque<NavigationEventDispatcher> = VecDeque::from([self.clone()]);
        while let Some(current) = to_dispose.pop_front() {
            current.core.is_disposed.set(true);
            let children: Vec<NavigationEventDispatcher> = current
                .core
                .child_dispatchers
                .borrow()
                .iter()
                .filter_map(|w| w.upgrade().map(|core| NavigationEventDispatcher { core }))
                .collect();
            to_dispose.extend(children);
            let inputs = current.core.inputs.borrow().clone();
            for input in &inputs {
                current.core.shared_processor.remove_input(input);
            }
            current.core.inputs.borrow_mut().clear();
            let handlers = current.core.handlers.borrow().clone();
            for handler in &handlers {
                handler.remove();
            }
            current.core.handlers.borrow_mut().clear();
            current.core.child_dispatchers.borrow_mut().clear();
            let parent = current.core.parent.borrow_mut().take();
            if let Some(parent) = parent {
                parent
                    .core
                    .child_dispatchers
                    .borrow_mut()
                    .retain(|w| !std::ptr::eq(w.as_ptr(), Rc::as_ptr(&current.core)));
            }
        }
    }

    fn check_invariants(&self) {
        assert!(!self.is_disposed(), "This NavigationEventDispatcher has already been disposed and cannot be used.");
    }
}

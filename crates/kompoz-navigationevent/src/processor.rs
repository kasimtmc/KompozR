//! NavigationEventProcessor.kt: the handlers and inputs of a dispatcher tree, in order, and the dispatch of its
//! events. Nothing is borrowed while a handler or an input is told something (they may add, remove, enable and
//! set infos as they are told).

use crate::dispatcher::{NavigationEventDispatcher, OnBackCompletedFallback, OnForwardCompletedFallback};
use crate::event::{NavigationEvent, NavigationEventHistory, NavigationEventInfo, NavigationEventTransitionState};
use crate::handler::AnyHandler;
use crate::input::NavigationEventInput;
use kompoz_coroutines::MutableStateFlow;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

const UNKNOWN: i32 = NavigationEventTransitionState::TRANSITIONING_UNKNOWN;
const BACK: i32 = NavigationEventTransitionState::TRANSITIONING_BACK;
const FORWARD: i32 = NavigationEventTransitionState::TRANSITIONING_FORWARD;

pub(crate) struct NavigationEventProcessor {
    pub(crate) transition_state: MutableStateFlow<NavigationEventTransitionState>,
    pub(crate) history: MutableStateFlow<NavigationEventHistory>,
    /// Overlay handlers, the latest added first.
    overlay_handlers: RefCell<VecDeque<Rc<dyn AnyHandler>>>,
    /// Default handlers, the latest added first.
    default_handlers: RefCell<VecDeque<Rc<dyn AnyHandler>>>,
    /// The handler of the event under way (its terminal event goes to it alone; removed, it is cancelled).
    in_progress_handler: RefCell<Option<Rc<dyn AnyHandler>>>,
    in_progress_direction: Cell<i32>,
    /// The input the event under way came from (another's events are ignored).
    in_progress_input: RefCell<Option<NavigationEventInput>>,
    unspecified_inputs: RefCell<Vec<NavigationEventInput>>,
    default_inputs: RefCell<Vec<NavigationEventInput>>,
    overlay_inputs: RefCell<Vec<NavigationEventInput>>,
    has_enabled_default_handlers: Cell<bool>,
    has_enabled_overlay_handlers: Cell<bool>,
    has_enabled_any_handlers: Cell<bool>,
}

impl NavigationEventProcessor {
    pub(crate) fn new() -> NavigationEventProcessor {
        NavigationEventProcessor {
            transition_state: MutableStateFlow::new(NavigationEventTransitionState::Idle),
            history: MutableStateFlow::new(NavigationEventHistory::empty()),
            overlay_handlers: RefCell::new(VecDeque::new()),
            default_handlers: RefCell::new(VecDeque::new()),
            in_progress_handler: RefCell::new(None),
            in_progress_direction: Cell::new(UNKNOWN),
            in_progress_input: RefCell::new(None),
            unspecified_inputs: RefCell::new(Vec::new()),
            default_inputs: RefCell::new(Vec::new()),
            overlay_inputs: RefCell::new(Vec::new()),
            has_enabled_default_handlers: Cell::new(false),
            has_enabled_overlay_handlers: Cell::new(false),
            has_enabled_any_handlers: Cell::new(false),
        }
    }

    fn handlers_of(queue: &RefCell<VecDeque<Rc<dyn AnyHandler>>>) -> Vec<Rc<dyn AnyHandler>> {
        queue.borrow().iter().cloned().collect()
    }

    /// Recomputes whether each priority has enabled handlers, tells the inputs of a priority whose answer changed,
    /// then brings the history up to the active handler.
    pub(crate) fn refresh_enabled_handlers(&self) {
        let enabled = |h: &Rc<dyn AnyHandler>| h.is_back_enabled() || h.is_forward_enabled();
        let new_overlay_enabled = Self::handlers_of(&self.overlay_handlers).iter().any(enabled);
        let new_default_enabled = Self::handlers_of(&self.default_handlers).iter().any(enabled);
        let new_any_enabled = new_overlay_enabled || new_default_enabled;

        let overlay_enabled_changed = self.has_enabled_overlay_handlers.get() != new_overlay_enabled;
        let default_enabled_changed = self.has_enabled_default_handlers.get() != new_default_enabled;
        let any_enabled_changed = self.has_enabled_any_handlers.get() != new_any_enabled;

        if overlay_enabled_changed {
            let inputs = self.overlay_inputs.borrow().clone();
            for input in inputs {
                input.do_on_has_enabled_handlers_changed(new_overlay_enabled);
            }
        }
        if default_enabled_changed {
            let inputs = self.default_inputs.borrow().clone();
            for input in inputs {
                input.do_on_has_enabled_handlers_changed(new_default_enabled);
            }
        }
        if any_enabled_changed {
            let inputs = self.unspecified_inputs.borrow().clone();
            for input in inputs {
                input.do_on_has_enabled_handlers_changed(new_any_enabled);
            }
        }

        // (committed after the notifications: changes compare against what was published)
        self.has_enabled_overlay_handlers.set(new_overlay_enabled);
        self.has_enabled_default_handlers.set(new_default_enabled);
        self.has_enabled_any_handlers.set(new_any_enabled);

        let active = self.active_handler();
        self.update_enabled_handler_info(active.as_ref());
    }

    /// The handler allowed to set the history now: the one in progress, else the first enabled one.
    fn active_handler(&self) -> Option<Rc<dyn AnyHandler>> {
        let in_progress = self.in_progress_handler.borrow().clone();
        in_progress.or_else(|| self.resolve_enabled_handler(UNKNOWN))
    }

    /// `handler`'s info changed (or the active handler may have): the history is updated if it is the active
    /// handler (a lower one can't overwrite it); the inputs are told at once.
    pub(crate) fn update_enabled_handler_info(&self, handler: Option<&Rc<dyn AnyHandler>>) {
        let active = self.active_handler();
        let same = match (&active, handler) {
            (None, None) => true,
            (Some(a), Some(h)) => a.same(&**h),
            _ => false,
        };
        if !same {
            return;
        }
        let new_history = match &active {
            None => NavigationEventHistory::empty(),
            Some(active) => NavigationEventHistory::new(
                active.current_info(),
                self.resolve_combined_back_info(),
                active.forward_info(),
            ),
        };
        if self.history.value() == new_history {
            return;
        }
        self.history.set_value(new_history.clone());
        let inputs: Vec<NavigationEventInput> = self
            .overlay_inputs
            .borrow()
            .iter()
            .chain(self.default_inputs.borrow().iter())
            .chain(self.unspecified_inputs.borrow().iter())
            .cloned()
            .collect();
        for input in inputs {
            input.do_on_history_changed(&new_history);
        }
    }

    pub(crate) fn add_handler(
        &self,
        dispatcher: &NavigationEventDispatcher,
        handler: Rc<dyn AnyHandler>,
        priority: i32,
    ) {
        assert!(!handler.has_dispatcher(), "Handler '{:?}' is already registered with a dispatcher", handler.id());
        match priority {
            NavigationEventDispatcher::PRIORITY_OVERLAY => {
                self.overlay_handlers.borrow_mut().push_front(handler.clone())
            }
            NavigationEventDispatcher::PRIORITY_DEFAULT => {
                self.default_handlers.borrow_mut().push_front(handler.clone())
            }
            _ => panic!("Unsupported priority value: {priority}"),
        }
        handler.set_dispatcher(Some(dispatcher));
        self.refresh_enabled_handlers();
    }

    pub(crate) fn remove_handler(&self, handler: &Rc<dyn AnyHandler>) {
        let in_progress = self.in_progress_handler.borrow().clone();
        if in_progress.is_some_and(|h| h.same(&**handler)) {
            match self.in_progress_direction.get() {
                BACK => handler.do_on_back_cancelled(),
                FORWARD => handler.do_on_forward_cancelled(),
                _ => {}
            }
            *self.in_progress_handler.borrow_mut() = None;
            self.in_progress_direction.set(UNKNOWN);
            *self.in_progress_input.borrow_mut() = None;
        }
        self.overlay_handlers.borrow_mut().retain(|h| !h.same(&**handler));
        self.default_handlers.borrow_mut().retain(|h| !h.same(&**handler));
        handler.set_dispatcher(None);
        self.refresh_enabled_handlers();
    }

    pub(crate) fn add_input(
        &self,
        dispatcher: &NavigationEventDispatcher,
        input: &NavigationEventInput,
        priority: i32,
    ) {
        assert!(
            input.dispatcher().is_none(),
            "Input '{input:?}' is already added to dispatcher {:?}.",
            input.dispatcher()
        );
        let inputs = match priority {
            NavigationEventDispatcher::PRIORITY_OVERLAY => &self.overlay_inputs,
            NavigationEventDispatcher::PRIORITY_DEFAULT => &self.default_inputs,
            _ => &self.unspecified_inputs,
        };
        {
            let mut inputs = inputs.borrow_mut();
            if !inputs.contains(input) {
                inputs.push(input.clone());
            }
        }
        *input.core.dispatcher.borrow_mut() = Rc::downgrade(&dispatcher.core);
        input.do_on_added(dispatcher);
        // (the initial state at once: nothing missed)
        input.do_on_history_changed(&self.history.value());
        let has_enabled_handlers = match priority {
            NavigationEventDispatcher::PRIORITY_OVERLAY => self.has_enabled_overlay_handlers.get(),
            NavigationEventDispatcher::PRIORITY_DEFAULT => self.has_enabled_default_handlers.get(),
            _ => self.has_enabled_any_handlers.get(),
        };
        input.do_on_has_enabled_handlers_changed(has_enabled_handlers);
    }

    pub(crate) fn remove_input(&self, input: &NavigationEventInput) {
        self.overlay_inputs.borrow_mut().retain(|i| i != input);
        self.default_inputs.borrow_mut().retain(|i| i != input);
        self.unspecified_inputs.borrow_mut().retain(|i| i != input);
        *input.core.dispatcher.borrow_mut() = std::rc::Weak::new();
        input.do_on_removed();
    }

    fn is_in_progress_from(&self, input: &NavigationEventInput, direction: i32) -> bool {
        self.in_progress_input.borrow().as_ref() == Some(input) && self.in_progress_direction.get() == direction
    }

    /// Starts an event: a predictive gesture with `event` (its handler told `on_…_started`, the transition state
    /// `InProgress`), a press without (nothing told until it completes or is cancelled). Ignored while one is
    /// under way.
    pub(crate) fn dispatch_on_started(
        &self,
        input: &NavigationEventInput,
        direction: i32,
        event: Option<NavigationEvent>,
    ) {
        if self.in_progress_direction.get() != UNKNOWN {
            return;
        }
        let handler = self.resolve_enabled_handler(direction);
        // (set before the handler runs: removed in its `on_…_started`, it is cancelled)
        *self.in_progress_handler.borrow_mut() = handler.clone();
        self.in_progress_direction.set(direction);
        *self.in_progress_input.borrow_mut() = Some(input.clone());
        if let Some(event) = event {
            match direction {
                BACK => {
                    if let Some(h) = &handler {
                        h.do_on_back_started(event);
                    }
                }
                FORWARD => {
                    if let Some(h) = &handler {
                        h.do_on_forward_started(event);
                    }
                }
                _ => {}
            }
            self.transition_state.set_value(NavigationEventTransitionState::in_progress(event, direction));
        }
    }

    /// A step of the predictive gesture under way (from its input, in its direction): to its handler.
    pub(crate) fn dispatch_on_progressed(&self, input: &NavigationEventInput, direction: i32, event: NavigationEvent) {
        if !self.is_in_progress_from(input, direction) {
            return;
        }
        let in_progress = self.in_progress_handler.borrow().clone();
        let handler = in_progress.or_else(|| self.resolve_enabled_handler(direction));
        match direction {
            BACK => {
                if let Some(h) = &handler {
                    h.do_on_back_progressed(event);
                }
            }
            FORWARD => {
                if let Some(h) = &handler {
                    h.do_on_forward_progressed(event);
                }
            }
            _ => {}
        }
        self.transition_state.set_value(NavigationEventTransitionState::in_progress(event, direction));
    }

    /// The event under way completed (terminal): its handler told, or the fallback without one; back to `Idle`.
    pub(crate) fn dispatch_on_completed(
        &self,
        input: &NavigationEventInput,
        direction: i32,
        on_back_completed_fallback: Option<&dyn OnBackCompletedFallback>,
        on_forward_completed_fallback: Option<&dyn OnForwardCompletedFallback>,
    ) {
        if !self.is_in_progress_from(input, direction) {
            return;
        }
        let in_progress = self.in_progress_handler.borrow_mut().take();
        let handler = in_progress.or_else(|| self.resolve_enabled_handler(direction));
        self.in_progress_direction.set(UNKNOWN);
        *self.in_progress_input.borrow_mut() = None;
        match direction {
            BACK => match &handler {
                None => {
                    if let Some(f) = on_back_completed_fallback {
                        f.on_back_completed_fallback();
                    }
                }
                Some(h) => h.do_on_back_completed(),
            },
            FORWARD => match &handler {
                None => {
                    if let Some(f) = on_forward_completed_fallback {
                        f.on_forward_completed_fallback();
                    }
                }
                Some(h) => h.do_on_forward_completed(),
            },
            _ => {}
        }
        self.transition_state.set_value(NavigationEventTransitionState::Idle);
    }

    /// The event under way was called off (terminal): its handler told; back to `Idle`.
    pub(crate) fn dispatch_on_cancelled(&self, input: &NavigationEventInput, direction: i32) {
        if !self.is_in_progress_from(input, direction) {
            return;
        }
        let in_progress = self.in_progress_handler.borrow_mut().take();
        let handler = in_progress.or_else(|| self.resolve_enabled_handler(direction));
        self.in_progress_direction.set(UNKNOWN);
        *self.in_progress_input.borrow_mut() = None;
        match direction {
            BACK => {
                if let Some(h) = &handler {
                    h.do_on_back_cancelled();
                }
            }
            FORWARD => {
                if let Some(h) = &handler {
                    h.do_on_forward_cancelled();
                }
            }
            _ => {}
        }
        self.transition_state.set_value(NavigationEventTransitionState::Idle);
    }

    /// The first handler enabled for `direction` (`UNKNOWN`: either): the overlay ones, then the default ones, the
    /// latest added first.
    fn resolve_enabled_handler(&self, direction: i32) -> Option<Rc<dyn AnyHandler>> {
        let predicate: fn(&Rc<dyn AnyHandler>) -> bool = match direction {
            UNKNOWN => |h| h.is_back_enabled() || h.is_forward_enabled(),
            BACK => |h| h.is_back_enabled(),
            FORWARD => |h| h.is_forward_enabled(),
            _ => panic!("Unsupported direction: '{direction}'."),
        };
        Self::handlers_of(&self.overlay_handlers)
            .into_iter()
            .find(predicate)
            .or_else(|| Self::handlers_of(&self.default_handlers).into_iter().find(predicate))
    }

    /// The back infos of the enabled handlers, overlay ones first.
    fn resolve_combined_back_info(&self) -> Vec<Rc<dyn NavigationEventInfo>> {
        let mut combined = Vec::new();
        for handler in
            Self::handlers_of(&self.overlay_handlers).iter().chain(Self::handlers_of(&self.default_handlers).iter())
        {
            if handler.is_back_enabled() {
                combined.extend(handler.back_info());
            }
        }
        combined
    }
}

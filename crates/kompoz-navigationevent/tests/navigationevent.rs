//! navigationevent's behaviour, as Compose's semantics say (androidx.navigationevent 1.1's KDoc and code): the
//! dispatcher's order (overlay before default, the latest added first), enabling (a handler's, its dispatcher's, an
//! ancestor's), predictive gestures and presses, the fallbacks, a handler removed or an input another's during a
//! gesture, the inputs' hooks by priority, the history and the transition state, disposal; the composables
//! (NavigationEventHandler / NavigationBackHandler / NavigationForwardHandler with their state,
//! rememberNavigationEventDispatcherOwner) in a host, whose root owner gets the keys' and the platform's back.

#![allow(non_snake_case)]

use kompoz_navigationevent::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

#[derive(Clone, PartialEq, Debug)]
struct Screen(&'static str);

impl NavigationEventInfo for Screen {}

/// A handler logging each callback as `name event` (`name started 0.1`, `name completed`).
struct Logged {
    name: &'static str,
    log: Log,
}

impl NavigationEventHandlerCallbacks<Screen> for Logged {
    fn on_back_started(&self, _: &NavigationEventHandler<Screen>, event: NavigationEvent) {
        self.log.borrow_mut().push(format!("{} started {}", self.name, event.progress));
    }
    fn on_back_progressed(&self, _: &NavigationEventHandler<Screen>, event: NavigationEvent) {
        self.log.borrow_mut().push(format!("{} progressed {}", self.name, event.progress));
    }
    fn on_back_completed(&self, _: &NavigationEventHandler<Screen>) {
        self.log.borrow_mut().push(format!("{} completed", self.name));
    }
    fn on_back_cancelled(&self, _: &NavigationEventHandler<Screen>) {
        self.log.borrow_mut().push(format!("{} cancelled", self.name));
    }
    fn on_forward_started(&self, _: &NavigationEventHandler<Screen>, event: NavigationEvent) {
        self.log.borrow_mut().push(format!("{} forward started {}", self.name, event.progress));
    }
    fn on_forward_progressed(&self, _: &NavigationEventHandler<Screen>, event: NavigationEvent) {
        self.log.borrow_mut().push(format!("{} forward progressed {}", self.name, event.progress));
    }
    fn on_forward_completed(&self, _: &NavigationEventHandler<Screen>) {
        self.log.borrow_mut().push(format!("{} forward completed", self.name));
    }
    fn on_forward_cancelled(&self, _: &NavigationEventHandler<Screen>) {
        self.log.borrow_mut().push(format!("{} forward cancelled", self.name));
    }
}

fn handler(name: &'static str, log: &Log) -> NavigationEventHandler<Screen> {
    NavigationEventHandler::with_forward(Screen(name), true, true, Logged { name, log: log.clone() })
}

fn event(progress: f32) -> NavigationEvent {
    NavigationEvent { progress, ..NavigationEvent::default() }
}

fn handlers_are_tried_overlay_first_then_the_latest_added() {
    let log = log();
    let dispatcher = NavigationEventDispatcher::new();
    let input = DirectNavigationEventInput::new();
    dispatcher.add_input(&input);
    let (a, b, overlay) = (handler("a", &log), handler("b", &log), handler("overlay", &log));
    dispatcher.add_handler_with_priority(&overlay, NavigationEventDispatcher::PRIORITY_OVERLAY);
    dispatcher.add_handler(&a);
    dispatcher.add_handler(&b);
    input.back_completed();
    assert_eq!(taken(&log), ["overlay completed"]);
    overlay.set_is_back_enabled(false);
    input.back_completed();
    assert_eq!(taken(&log), ["b completed"]); // the latest added default one
    b.set_is_back_enabled(false);
    input.back_completed();
    assert_eq!(taken(&log), ["a completed"]);
    // Added twice to it: nothing (a set); to another dispatcher: an error.
    dispatcher.add_handler(&a);
    let other = NavigationEventDispatcher::new();
    let other_add = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| other.add_handler(&a)));
    assert!(other_add.is_err());
    // A priority not one of the constants: an error.
    let c = handler("c", &log);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatcher.add_handler_with_priority(&c, 7))).is_err()
    );
    // Removed: no more events; removed again, nothing.
    a.remove();
    a.remove();
    input.back_completed();
    assert!(taken(&log).is_empty());
}

fn a_disabled_dispatcher_disables_its_handlers_and_its_childrens() {
    let log = log();
    let root = NavigationEventDispatcher::new();
    let child = NavigationEventDispatcher::with_parent(&root);
    let grandchild = NavigationEventDispatcher(&child);
    let input = DirectNavigationEventInput::new();
    root.add_input(&input);
    let (a, b) = (handler("a", &log), handler("b", &log));
    root.add_handler(&a);
    grandchild.add_handler(&b);
    input.back_completed();
    assert_eq!(taken(&log), ["b completed"]);
    child.set_is_enabled(false);
    assert!(!grandchild.is_enabled() && !b.is_back_enabled());
    input.back_completed();
    assert_eq!(taken(&log), ["a completed"]);
    child.set_is_enabled(true);
    assert!(b.is_back_enabled());
    // The root disabled: events from its inputs go nowhere.
    root.set_is_enabled(false);
    input.back_completed();
    assert!(taken(&log).is_empty());
}

fn a_predictive_gesture_goes_to_its_handler_and_a_press_only_completes() {
    let log = log();
    let dispatcher = NavigationEventDispatcher::new();
    let input = DirectNavigationEventInput::new();
    dispatcher.add_input(&input);
    let (a, b) = (handler("a", &log), handler("b", &log));
    dispatcher.add_handler(&a);
    dispatcher.add_handler(&b);
    let transition = dispatcher.transition_state();
    input.back_started(event(0.0));
    assert_eq!(
        b.transition_state(),
        NavigationEventTransitionState::in_progress(event(0.0), NavigationEventTransitionState::TRANSITIONING_BACK)
    );
    // A handler added during the gesture doesn't get it: the gesture is its handler's to its end.
    let c = handler("c", &log);
    dispatcher.add_handler(&c);
    input.back_progressed(event(0.5));
    assert_eq!(transition.value().latest_event(), Some(event(0.5)));
    // A second start from the same input during the gesture: ignored.
    input.back_started(event(0.9));
    input.back_completed();
    assert_eq!(taken(&log), ["b started 0", "b progressed 0.5", "b completed"]);
    assert!(transition.value().is_idle() && b.transition_state().is_idle());
    // A press: only completed, by the first enabled handler now.
    input.back_completed();
    assert_eq!(taken(&log), ["c completed"]);
    // Cancelled: the handler told; a cancellation without a gesture is a press cancelled.
    input.back_started(event(0.1));
    input.back_cancelled();
    input.back_cancelled();
    assert_eq!(taken(&log), ["c started 0.1", "c cancelled", "c cancelled"]);
    // A progress without a gesture: nothing.
    input.back_progressed(event(0.3));
    assert!(taken(&log).is_empty());
    // Forward: the same, to the handlers enabled for it.
    b.set_is_forward_enabled(false);
    c.set_is_forward_enabled(false);
    input.forward_started(event(0.2));
    input.forward_progressed(event(0.4));
    input.forward_completed();
    assert_eq!(taken(&log), ["a forward started 0.2", "a forward progressed 0.4", "a forward completed"]);
    input.forward_cancelled();
    assert_eq!(taken(&log), ["a forward cancelled"]);
}

fn completions_no_handler_takes_go_to_the_fallbacks() {
    let log = log();
    let (l1, l2) = (log.clone(), log.clone());
    let dispatcher = NavigationEventDispatcher::with_fallbacks(
        move || l1.borrow_mut().push("back fallback".into()),
        move || l2.borrow_mut().push("forward fallback".into()),
    );
    let child = NavigationEventDispatcher::with_parent(&dispatcher);
    let input = DirectNavigationEventInput::new();
    dispatcher.add_input(&input);
    input.back_completed();
    input.forward_completed();
    // Cancelled: no fallback.
    input.back_started(event(0.0));
    input.back_cancelled();
    assert_eq!(taken(&log), ["back fallback", "forward fallback"]);
    // A child's input completing goes to the child's own fallback (none).
    let child_input = DirectNavigationEventInput::new();
    child.add_input(&child_input);
    child_input.back_completed();
    assert!(taken(&log).is_empty());
    // The one-fallback root: Kotlin's constructor of a closure.
    let l = log.clone();
    let root = NavigationEventDispatcher(move || l.borrow_mut().push("only back".into()));
    let input = DirectNavigationEventInput::new();
    root.add_input(&input);
    input.back_completed();
    input.forward_completed();
    assert_eq!(taken(&log), ["only back"]);
}

fn a_handler_removed_during_its_gesture_is_cancelled_and_other_inputs_wait() {
    let log = log();
    let dispatcher = NavigationEventDispatcher::new();
    let (first, second) = (DirectNavigationEventInput::new(), DirectNavigationEventInput::new());
    dispatcher.add_input(&first);
    dispatcher.add_input(&second);
    let (a, b) = (handler("a", &log), handler("b", &log));
    dispatcher.add_handler(&a);
    dispatcher.add_handler(&b);
    first.back_started(event(0.0));
    // Another input's events during the gesture: ignored (its press too).
    second.back_progressed(event(0.7));
    second.back_completed();
    assert_eq!(taken(&log), ["b started 0"]);
    // Its handler removed: cancelled; the gesture's end then goes nowhere.
    b.remove();
    first.back_progressed(event(0.5));
    first.back_completed();
    assert_eq!(taken(&log), ["b cancelled"]);
    // A handler removing itself as its gesture starts: cancelled at once.
    struct RemovesItself(Log);
    impl NavigationEventHandlerCallbacks<Screen> for RemovesItself {
        fn on_back_started(&self, handler: &NavigationEventHandler<Screen>, _: NavigationEvent) {
            self.0.borrow_mut().push("started".into());
            handler.remove();
        }
        fn on_back_cancelled(&self, _: &NavigationEventHandler<Screen>) {
            self.0.borrow_mut().push("cancelled".into());
        }
    }
    let r = NavigationEventHandler::new(Screen("r"), true, RemovesItself(log.clone()));
    dispatcher.add_handler(&r);
    first.back_started(event(0.0));
    assert_eq!(taken(&log), ["started", "cancelled"]);
    first.back_cancelled();
    first.back_completed();
    assert_eq!(taken(&log), ["a completed"]);
}

/// An input logging what it hears.
struct Hearing(&'static str, Log);

impl NavigationEventInputCallbacks for Hearing {
    fn on_added(&self, _: &NavigationEventInput, _: &NavigationEventDispatcher) {
        self.1.borrow_mut().push(format!("{} added", self.0));
    }
    fn on_removed(&self, _: &NavigationEventInput) {
        self.1.borrow_mut().push(format!("{} removed", self.0));
    }
    fn on_has_enabled_handlers_changed(&self, _: &NavigationEventInput, has_enabled_handlers: bool) {
        self.1.borrow_mut().push(format!("{} enabled {has_enabled_handlers}", self.0));
    }
    fn on_history_changed(&self, _: &NavigationEventInput, history: &NavigationEventHistory) {
        self.1.borrow_mut().push(format!("{} history {}", self.0, history.current_index()));
    }
}

fn inputs_hear_whether_handlers_of_their_priority_are_enabled() {
    let log = log();
    let handlers = log.clone();
    let dispatcher = NavigationEventDispatcher::new();
    let any = NavigationEventInput::new(Hearing("any", log.clone()));
    let overlay = NavigationEventInput::new(Hearing("overlay", log.clone()));
    let default = NavigationEventInput::new(Hearing("default", log.clone()));
    // Added: told at once, then the history and whether handlers are enabled.
    dispatcher.add_input(&any);
    dispatcher.add_input_with_priority(&overlay, NavigationEventDispatcher::PRIORITY_OVERLAY);
    dispatcher.add_input_with_priority(&default, NavigationEventDispatcher::PRIORITY_DEFAULT);
    assert_eq!(
        taken(&log),
        [
            "any added",
            "any history -1",
            "any enabled false",
            "overlay added",
            "overlay history -1",
            "overlay enabled false",
            "default added",
            "default history -1",
            "default enabled false"
        ]
    );
    let a = handler("a", &handlers);
    dispatcher.add_handler(&a);
    assert_eq!(
        taken(&log),
        ["default enabled true", "any enabled true", "overlay history 0", "default history 0", "any history 0"]
    );
    let o = handler("o", &handlers);
    dispatcher.add_handler_with_priority(&o, NavigationEventDispatcher::PRIORITY_OVERLAY);
    // (an overlay handler is the active one now: its history)
    assert_eq!(taken(&log), ["overlay enabled true", "overlay history 0", "default history 0", "any history 0"]);
    a.set_is_back_enabled(false);
    a.set_is_forward_enabled(false);
    assert_eq!(taken(&log), ["default enabled false"]);
    // A priority not one of the constants: an error; added twice: nothing; to another dispatcher: an error.
    let x = NavigationEventInput::new(());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatcher.add_input_with_priority(&x, 5))).is_err()
    );
    dispatcher.add_input(&any);
    let other = NavigationEventDispatcher::new();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| other.add_input(&any))).is_err());
    assert!(taken(&log).is_empty());
    dispatcher.remove_input(&default);
    assert_eq!(taken(&log), ["default removed"]);
    // Not added: dispatching is an error.
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| default.dispatch_on_back_completed())).is_err());
}

fn the_history_is_the_active_handlers_and_stays_during_a_gesture() {
    let log = log();
    let dispatcher = NavigationEventDispatcher::new();
    let input = DirectNavigationEventInput::new();
    dispatcher.add_input(&input);
    let history = dispatcher.history();
    assert_eq!(history.value().current_index(), -1);
    assert!(history.value().merged_history().is_empty());
    let list = handler("list", &log);
    list.set_info(Screen("list"), vec![Screen("home")], vec![]);
    dispatcher.add_handler(&list);
    let detail = handler("detail", &log);
    detail.set_info(Screen("detail"), vec![Screen("list"), Screen("home")], vec![Screen("next")]);
    dispatcher.add_handler(&detail);
    let names = |h: &NavigationEventHistory| -> Vec<&'static str> {
        h.merged_history().iter().map(|i| i.downcast_ref::<Screen>().unwrap().0).collect()
    };
    // The back infos of every enabled handler (overlay first, then the latest added), then the active one's current
    // and forward infos.
    assert_eq!(names(&history.value()), ["list", "home", "home", "detail", "next"]);
    assert_eq!(history.value().current_index(), 3);
    // A lower handler's info changing doesn't change it.
    list.set_info(Screen("list2"), vec![], vec![]);
    assert_eq!(names(&history.value()), ["list", "home", "home", "detail", "next"]);
    // The active one's does.
    detail.set_info(Screen("detail"), vec![], vec![]);
    assert_eq!(names(&history.value()), ["detail"]);
    // During a gesture: the handler in progress keeps it, even disabled.
    input.back_started(event(0.0));
    detail.set_is_back_enabled(false);
    detail.set_is_forward_enabled(false);
    assert_eq!(names(&history.value()), ["detail"]);
    input.back_completed();
    // (a completion doesn't recompute it: the next change of the active handler does)
    assert_eq!(names(&history.value()), ["detail"]);
    list.set_info(Screen("list3"), vec![], vec![]);
    assert_eq!(names(&history.value()), ["list3"]);
    // None enabled: empty again.
    list.set_is_back_enabled(false);
    list.set_is_forward_enabled(false);
    assert_eq!(history.value().current_index(), -1);
    assert_eq!(taken(&log), ["detail started 0", "detail completed"]);
}

fn disposing_a_dispatcher_takes_its_subtree_out() {
    let log = log();
    let root = NavigationEventDispatcher::new();
    let child = NavigationEventDispatcher::with_parent(&root);
    let grandchild = NavigationEventDispatcher::with_parent(&child);
    let input = DirectNavigationEventInput::new();
    root.add_input(&input);
    let child_input = NavigationEventInput::new(Hearing("child input", log.clone()));
    grandchild.add_input(&child_input);
    let (a, b, c) = (handler("a", &log), handler("b", &log), handler("c", &log));
    root.add_handler(&a);
    child.add_handler(&b);
    grandchild.add_handler(&c);
    taken(&log);
    input.back_started(event(0.0));
    child.dispose();
    // The subtree's inputs removed, then its handlers (the gesture's: cancelled).
    assert_eq!(taken(&log), ["c started 0", "child input removed", "c cancelled"]);
    input.back_cancelled();
    input.back_completed();
    assert_eq!(taken(&log), ["a completed"]);
    // Disposed (and its children): used again, an error.
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| child.dispose())).is_err());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| grandchild.add_handler(&b))).is_err());
    // A handler of a disposed dispatcher can be added to another.
    root.add_handler(&b);
    input.back_completed();
    assert_eq!(taken(&log), ["b completed"]);
}

fn a_completion_not_overridden_is_an_error() {
    struct Nothing;
    impl NavigationEventHandlerCallbacks<NavigationEventInfoNone> for Nothing {}
    let dispatcher = NavigationEventDispatcher::new();
    let input = DirectNavigationEventInput::new();
    dispatcher.add_input(&input);
    let h = NavigationEventHandler::new(NavigationEventInfoNone, true, Nothing);
    dispatcher.add_handler(&h);
    input.back_started(event(0.0));
    input.back_cancelled(); // (cancellations needn't be overridden)
    let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| input.back_completed()));
    assert!(completed.is_err());
}

// ---- The composables --------------------------------------------------------------------------------------------

fn navigation_back_handlers_take_back_in_composition_order_with_their_state() {
    let log = log();
    let inner = mutable_state_of(true);
    let captured: Rc<RefCell<Option<NavigationEventState<Screen>>>> = Rc::new(RefCell::new(None));
    let (l, i, c) = (log.clone(), inner.clone(), captured.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l1, l2, l3) = (l.clone(), l.clone(), l.clone());
        let outer = remember_navigation_event_state(Screen("outer"));
        NavigationBackHandler(outer, on_back_completed = move || l1.borrow_mut().push("outer".into()));
        let state = remember_navigation_event_state(Screen("inner"), back_info = vec![Screen("outer")]);
        *c.borrow_mut() = Some(state.clone());
        NavigationBackHandler(
            state,
            is_back_enabled = i.get(),
            on_back_cancelled = move || l2.borrow_mut().push("inner cancelled".into()),
            on_back_completed = move || l3.borrow_mut().push("inner".into()),
        );
    }));
    let host = rule.host();
    let dispatcher = host.navigation_event_dispatcher();
    let state = captured.borrow().clone().unwrap();
    // The last composed enabled one: its history.
    let names: Vec<&'static str> =
        dispatcher.history().value().merged_history().iter().map(|i| i.downcast_ref::<Screen>().unwrap().0).collect();
    assert_eq!(names, ["outer", "inner"]);
    // A gesture: its state follows it, then Idle.
    host.back_started(kompoz_ui::back::BackEventCompat { progress: 0.0, ..Default::default() });
    host.back_progressed(kompoz_ui::back::BackEventCompat { progress: 0.4, ..Default::default() });
    assert_eq!(state.transition_state().latest_event().map(|e| e.progress), Some(0.4));
    host.back_cancelled();
    assert!(state.transition_state().is_idle());
    assert_eq!(taken(&log), ["inner cancelled"]);
    assert!(host.back_invoked());
    assert_eq!(taken(&log), ["inner"]);
    inner.set(false);
    rule.wait_for_idle();
    assert!(host.back_invoked());
    assert_eq!(taken(&log), ["outer"]);
    // A state given to two handlers: an error.
    let twice = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _rule = KompozTestRule::new(kompozable_lambda!(|| {
            let state = remember_navigation_event_state(NavigationEventInfoNone);
            NavigationBackHandler(state.clone(), on_back_completed = || {});
            NavigationBackHandler(state, on_back_completed = || {});
        }));
    }));
    assert!(twice.is_err());
}

fn navigation_event_handlers_take_forward_and_back_and_leave_with_the_composition() {
    let log = log();
    let shown = mutable_state_of(true);
    let (l, s) = (log.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        if s.get() {
            let (l1, l2, l3) = (l.clone(), l.clone(), l.clone());
            let state = remember_navigation_event_state(Screen("page"), forward_info = vec![Screen("next")]);
            NavigationEventHandler(
                state,
                on_forward_completed = move || l1.borrow_mut().push("forward".into()),
                on_back_completed = move || l2.borrow_mut().push("back".into()),
            );
            let other = remember_navigation_event_state(Screen("only forward"));
            NavigationForwardHandler(
                other,
                is_forward_enabled = false,
                on_forward_completed = move || l3.borrow_mut().push("never".into()),
            );
        }
    }));
    let dispatcher = rule.host().navigation_event_dispatcher();
    let input = DirectNavigationEventInput::new();
    dispatcher.add_input(&input);
    input.forward_completed();
    input.back_completed();
    assert_eq!(taken(&log), ["forward", "back"]);
    let names: Vec<&'static str> =
        dispatcher.history().value().merged_history().iter().map(|i| i.downcast_ref::<Screen>().unwrap().0).collect();
    assert_eq!(names, ["page", "next"]);
    shown.set(false);
    rule.wait_for_idle();
    input.back_completed();
    assert!(taken(&log).is_empty());
    assert_eq!(dispatcher.history().value().current_index(), -1);
    dispatcher.remove_input(&input);
}

fn remembered_owners_scope_their_content_and_are_disposed_with_it() {
    let log = log();
    let enabled = mutable_state_of(true);
    let shown = mutable_state_of(true);
    let (l, e, s) = (log.clone(), enabled.clone(), shown.clone());
    let owner_seen: Rc<RefCell<Option<Rc<dyn NavigationEventDispatcherOwner>>>> = Rc::new(RefCell::new(None));
    let o = owner_seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l1 = l.clone();
        let root_state = remember_navigation_event_state(NavigationEventInfoNone);
        NavigationBackHandler(root_state, on_back_completed = move || l1.borrow_mut().push("root".into()));
        if s.get() {
            let new_owner = remember_navigation_event_dispatcher_owner(enabled = e.get());
            *o.borrow_mut() = Some(new_owner.clone());
            let l2 = l.clone();
            KompozitionLocalProvider(&[LocalNavigationEventDispatcherOwner.provides(new_owner)], move || {
                let l2 = l2.clone();
                let state = remember_navigation_event_state(NavigationEventInfoNone);
                NavigationBackHandler(state, on_back_completed = move || l2.borrow_mut().push("scoped".into()));
            });
        }
    }));
    let host = rule.host();
    // The scope's dispatcher is a child of the host's: one order.
    assert!(host.back_invoked());
    assert_eq!(taken(&log), ["scoped"]);
    let scoped = owner_seen.borrow().clone().unwrap();
    // Disabled (a launched effect: after the frame), its handlers are skipped.
    enabled.set(false);
    rule.wait_for_idle();
    assert!(!scoped.navigation_event_dispatcher().is_enabled());
    assert!(host.back_invoked());
    assert_eq!(taken(&log), ["root"]);
    enabled.set(true);
    rule.wait_for_idle();
    // Gone: disposed with its handlers.
    shown.set(false);
    rule.wait_for_idle();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scoped.navigation_event_dispatcher().dispose()))
            .is_err()
    );
    assert!(host.back_invoked());
    assert_eq!(taken(&log), ["root"]);
    // A root of its own (parent None): the host's inputs don't reach it.
    let reached = Rc::new(Cell::new(false));
    let r = reached.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let own = remember_navigation_event_dispatcher_owner(parent = None);
        let r = r.clone();
        KompozitionLocalProvider(&[LocalNavigationEventDispatcherOwner.provides(own)], move || {
            let r = r.clone();
            let state = remember_navigation_event_state(NavigationEventInfoNone);
            NavigationBackHandler(state, on_back_completed = move || r.set(true));
        });
    }));
    assert!(!rule.host().back_invoked() && !reached.get());
}

fn the_hosts_root_owner_takes_back_keys_while_a_handler_is_enabled() {
    use kompoz_ui::input::{Key, KeyModifiers, PointerButton};
    let backs = Rc::new(Cell::new(0));
    let enabled = mutable_state_of(true);
    let (b, e) = (backs.clone(), enabled.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let b = b.clone();
        let state = remember_navigation_event_state(NavigationEventInfoNone);
        NavigationBackHandler(state, is_back_enabled = e.get(), on_back_completed = move || b.set(b.get() + 1));
    }));
    let host = rule.host();
    assert!(host.wants_back());
    assert!(host.key_down(Key::Escape, KeyModifiers::None));
    assert!(host.key_down(Key::Back, KeyModifiers::None));
    // The mouse's back button: the Back key after its pointer event (Android's input reader).
    host.mouse_button(kompoz_graphics::Offset::new(1.0, 1.0), -1, PointerButton::Back, true);
    host.mouse_button(kompoz_graphics::Offset::new(1.0, 1.0), -1, PointerButton::Back, false);
    assert_eq!(backs.get(), 3);
    // None enabled: the key goes on, the platform's back is its own (leaving the app).
    enabled.set(false);
    rule.wait_for_idle();
    assert!(!host.wants_back());
    assert!(!host.key_down(Key::Escape, KeyModifiers::None));
    assert!(!host.back_invoked());
    assert_eq!(backs.get(), 3);
}

/// The host's root owner is its architecture components owner (CMP's `DefaultArchitectureComponentsOwner`: the
/// lifecycle, ViewModel store, saved state registry and navigation event dispatcher owner in one), its dispatcher
/// the window's; made again from a saved state (`restore_state`), the owner is new and the dispatcher the same,
/// what registered with it still taking back.
fn the_hosts_root_owner_is_its_architecture_components_owner() {
    use kompoz_ui::input::{Key, KeyModifiers};
    let backs = Rc::new(Cell::new(0));
    let owners = Rc::new(RefCell::new(Vec::<Rc<dyn NavigationEventDispatcherOwner>>::new()));
    let (b, o) = (backs.clone(), owners.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let b = b.clone();
        o.borrow_mut().push(LocalNavigationEventDispatcherOwner.current().expect("the host's owner"));
        let state = remember_navigation_event_state(NavigationEventInfoNone);
        NavigationBackHandler(state, on_back_completed = move || b.set(b.get() + 1));
    }));
    let host = rule.host();
    let address = |o: &Rc<dyn NavigationEventDispatcherOwner>| Rc::as_ptr(o) as *const ();
    let root = owners.borrow().last().cloned().expect("composed");
    assert_eq!(address(&root), address(&host.navigation_event_dispatcher_owner()));
    assert_eq!(address(&root), Rc::as_ptr(&host.lifecycle_owner()) as *const ());
    assert!(root.navigation_event_dispatcher() == host.navigation_event_dispatcher());
    let dispatcher = host.navigation_event_dispatcher();
    let bytes = host.save_state();
    host.restore_state(&bytes);
    rule.wait_for_idle();
    let again = owners.borrow().last().cloned().expect("composed");
    assert_ne!(address(&again), address(&root));
    assert_eq!(address(&again), address(&host.navigation_event_dispatcher_owner()));
    assert!(again.navigation_event_dispatcher() == dispatcher && host.navigation_event_dispatcher() == dispatcher);
    assert!(host.key_down(Key::Escape, KeyModifiers::None));
    assert_eq!(backs.get(), 1);
}

fn main() {
    let tests: &[(&str, fn())] = &[
        (
            "the_hosts_root_owner_is_its_architecture_components_owner",
            the_hosts_root_owner_is_its_architecture_components_owner,
        ),
        (
            "handlers_are_tried_overlay_first_then_the_latest_added",
            handlers_are_tried_overlay_first_then_the_latest_added,
        ),
        (
            "a_disabled_dispatcher_disables_its_handlers_and_its_childrens",
            a_disabled_dispatcher_disables_its_handlers_and_its_childrens,
        ),
        (
            "a_predictive_gesture_goes_to_its_handler_and_a_press_only_completes",
            a_predictive_gesture_goes_to_its_handler_and_a_press_only_completes,
        ),
        ("completions_no_handler_takes_go_to_the_fallbacks", completions_no_handler_takes_go_to_the_fallbacks),
        (
            "a_handler_removed_during_its_gesture_is_cancelled_and_other_inputs_wait",
            a_handler_removed_during_its_gesture_is_cancelled_and_other_inputs_wait,
        ),
        (
            "inputs_hear_whether_handlers_of_their_priority_are_enabled",
            inputs_hear_whether_handlers_of_their_priority_are_enabled,
        ),
        (
            "the_history_is_the_active_handlers_and_stays_during_a_gesture",
            the_history_is_the_active_handlers_and_stays_during_a_gesture,
        ),
        ("disposing_a_dispatcher_takes_its_subtree_out", disposing_a_dispatcher_takes_its_subtree_out),
        ("a_completion_not_overridden_is_an_error", a_completion_not_overridden_is_an_error),
        (
            "navigation_back_handlers_take_back_in_composition_order_with_their_state",
            navigation_back_handlers_take_back_in_composition_order_with_their_state,
        ),
        (
            "navigation_event_handlers_take_forward_and_back_and_leave_with_the_composition",
            navigation_event_handlers_take_forward_and_back_and_leave_with_the_composition,
        ),
        (
            "remembered_owners_scope_their_content_and_are_disposed_with_it",
            remembered_owners_scope_their_content_and_are_disposed_with_it,
        ),
        (
            "the_hosts_root_owner_takes_back_keys_while_a_handler_is_enabled",
            the_hosts_root_owner_takes_back_keys_while_a_handler_is_enabled,
        ),
    ];
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = Vec::new();
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        if std::panic::catch_unwind(test).is_err() {
            failed.push(*name);
            println!("FAILED {name}");
        } else {
            println!("ok     {name}");
        }
    }
    println!("\n{} run, {} failed", tests.len(), failed.len());
    if !failed.is_empty() {
        std::process::exit(1);
    }
}

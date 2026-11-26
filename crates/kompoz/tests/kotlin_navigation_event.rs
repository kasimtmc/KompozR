//! Back handling in Kotlin's syntax, as a Compose user writes it: activity-compose 1.13's samples (BackHandler,
//! PredictiveBack) and navigationevent-compose 1.1's (rememberNavigationEventDispatcherOwner) from kt/navigation_event.kt
//! (kompoz_file!), and navigationevent's composables as an app uses them (NavigationBackHandler with a hoisted
//! `rememberNavigationEventState` whose transition state is read, NavigationEventHandler with forward, a scope of its
//! own), and a handler of the app's own as activity-compose's BackHandlerCompat writes one (an object expression of
//! `NavigationEventHandler<NavigationEventInfo>`). The same behaviour crates/kompoz-navigationevent/tests/
//! navigationevent.rs and kompoz-ui's windows.rs check in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::input::KeyModifiers;
use std::cell::RefCell;
use std::rc::Rc;

kompoz_file!("tests/kt/navigation_event.kt");

// ---- the rest of the API as an app writes it ------------------------------------------------------------------------

kompoz! {
    // An input of the app's own (a button sending a whole gesture) added to the nearest dispatcher, with a priority.
    @Composable
    fun OwnInput(onBack: () -> Unit, seen: (String) -> Unit) {
        val dispatcher = LocalNavigationEventDispatcherOwner.current!!.navigationEventDispatcher
        val input = remember { DirectNavigationEventInput() }
        DisposableEffect(dispatcher) {
            dispatcher.addInput(input, NavigationEventDispatcher.PRIORITY_DEFAULT)
            onDispose { dispatcher.removeInput(input) }
        }
        val state = rememberNavigationEventState(NavigationEventInfo.None)
        NavigationBackHandler(state = state, onBackCompleted = onBack)
        Button(
            onClick = {
                input.backStarted(NavigationEvent(swipeEdge = NavigationEvent.EDGE_LEFT, progress = 0.1f))
                input.backProgressed(NavigationEvent(progress = 0.6f, touchX = 12f))
                seen("ilerleme ${dispatcher.history.value.currentIndex}")
                input.backCompleted()
            },
            modifier = Modifier.testTag("geri"),
        ) { Text("geri") }
    }
}

// ---- tests ----------------------------------------------------------------------------------------------------------

fn escape(rule: &KompozTestRule) -> bool {
    let taken = rule.host().key_down(Key::Escape, KeyModifiers::None);
    rule.host().key_up(Key::Escape, KeyModifiers::None);
    rule.wait_for_idle();
    taken
}

fn step(progress: f32) -> BackEventCompat {
    BackEventCompat { progress, ..BackEventCompat::default() }
}

/// activity-compose's BackHandler and PredictiveBack samples: enabled while there is text; back clears it (the
/// BackHandler sample's "handle back event"), the predictive one follows the gesture to its pop.
fn back_handlers_as_activity_compose_samples_write_them() {
    let rule = KompozTestRule::new(BackHandlerSample);
    assert!(!escape(&rule)); // (no text: nothing takes back)
    rule.on_node_with_tag("metin").perform_text_input("yazı");
    rule.wait_for_idle();
    assert!(rule.host().wants_back());
    assert!(escape(&rule));
    rule.on_node_with_text("yazı").assert_does_not_exist();
    assert!(!rule.host().wants_back());

    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let note = |text: &'static str| {
        let log = log.clone();
        move || log.borrow_mut().push(text.to_string())
    };
    let l = log.clone();
    let callback = SampleCallbackHelper(
        note("prepare"),
        move |progress: f32| l.borrow_mut().push(format!("progress {progress}")),
        note("pop"),
        note("cancel"),
    );
    let taken = || std::mem::take(&mut *log.borrow_mut());
    let c = callback.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || PredictiveBack(c.clone())));
    let host = rule.host();
    assert!(!host.wants_back());
    rule.on_node_with_tag("metin").perform_text_input("yazı");
    rule.wait_for_idle();
    host.back_started(step(0.0));
    host.back_progressed(step(0.25));
    rule.wait_for_idle();
    host.back_progressed(step(0.5));
    rule.wait_for_idle();
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert_eq!(taken(), ["prepare", "progress 0.25", "progress 0.5", "pop"]);
    // Called off: the coroutine is cancelled, the sample's catch cancels the pop.
    host.back_started(step(0.0));
    host.back_progressed(step(0.1));
    rule.wait_for_idle();
    host.back_cancelled();
    rule.wait_for_idle();
    assert_eq!(taken(), ["prepare", "progress 0.1", "cancel"]);
    // A press: the flow ends at once.
    assert!(escape(&rule));
    assert_eq!(taken(), ["prepare", "pop"]);
}

/// navigationevent-compose's RememberNavigationEventDispatcherOwner sample: a scope of its own for its content.
fn remember_navigation_event_dispatcher_owner_as_its_sample_writes_it() {
    let rule = KompozTestRule::new(RememberNavigationEventDispatcherOwner);
    rule.on_node_with_text("kapsam").assert_exists();
    assert!(!rule.host().wants_back());
}

/// NavigationBackHandler with a hoisted state: the gesture's progress read from its transition state, back closing
/// the screen; NavigationEventHandler with forward and back, and a scope of its own (disabled: skipped).
fn navigation_event_handlers_as_an_app_writes_them() {
    let open = mutable_state_of(true);
    let o = open.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        if o.get() {
            let o = o.clone();
            Detail(move || o.set(false));
        }
    }));
    let host = rule.host();
    let history: Vec<Screen> = host
        .navigation_event_dispatcher()
        .history()
        .value()
        .merged_history()
        .iter()
        .map(|i| i.downcast_ref::<Screen>().unwrap().clone())
        .collect();
    assert_eq!(history, [Screen("liste"), Screen("detay")]);
    host.back_started(step(0.0));
    host.back_progressed(step(0.3));
    rule.wait_for_idle();
    rule.on_node_with_text("detay kapanıyor 0.3").assert_exists();
    host.back_cancelled();
    rule.wait_for_idle();
    rule.on_node_with_text("detay").assert_exists();
    assert!(escape(&rule));
    rule.on_node_with_text("detay").assert_does_not_exist();

    let page = mutable_state_of(0);
    let scoped = mutable_state_of(false);
    let (p, s) = (page.clone(), scoped.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let p2 = p.clone();
        Pages(p.get(), move |to: i32| p2.set(to), s.get());
    }));
    let host = rule.host();
    let input = DirectNavigationEventInput::new();
    host.navigation_event_dispatcher().add_input(&input);
    // Back disabled on the first page, the scope disabled: no handler takes back (the key is the dispatcher's while a
    // handler is enabled, forward's here: it goes nowhere).
    assert!(escape(&rule));
    rule.on_node_with_text("sayfa 0").assert_exists();
    input.forward_completed();
    rule.wait_for_idle();
    rule.on_node_with_text("sayfa 1").assert_exists();
    input.forward_completed();
    rule.wait_for_idle();
    input.forward_completed(); // (no forward from the last page)
    rule.wait_for_idle();
    rule.on_node_with_text("sayfa 2").assert_exists();
    assert!(escape(&rule));
    rule.on_node_with_text("sayfa 1").assert_exists();
    // The scope enabled: its handler (composed later) comes first.
    scoped.set(true);
    rule.wait_for_idle();
    assert!(escape(&rule));
    assert_eq!(page.get(), -1);
}

/// An input of the app's own sending a gesture to the nearest dispatcher (`addInput(input, priority)`,
/// `NavigationEvent(…)` with Compose's named arguments, the history's index).
fn navigation_event_inputs_as_an_app_writes_them() {
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let (l1, l2) = (log.clone(), log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l1, l2) = (l1.clone(), l2.clone());
        OwnInput(move || l1.borrow_mut().push("geri".into()), move |s: String| l2.borrow_mut().push(s));
    }));
    rule.on_node_with_tag("geri").perform_click();
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["ilerleme 0", "geri"]);
}

/// A handler of the app's own (kt/navigation_event.kt's loggingHandler, as activity-compose's BackHandlerCompat writes
/// its one): infos of any type, back disabled until enabled, its overrides told the gesture; its info in the history.
fn navigation_event_handler_objects_as_activity_compose_writes_them() {
    use kompoz::navigationevent::{NavigationEvent, NavigationEventDispatcher};
    let log = mutable_state_of(String::new());
    let info: Rc<dyn NavigationEventInfo> = Rc::new(Screen("sayfa"));
    let handler = logging_handler(info, log.clone());
    let dispatcher = NavigationEventDispatcher::new();
    dispatcher.add_handler(&handler);
    let input = DirectNavigationEventInput::new();
    dispatcher.add_input(&input);
    input.back_started(NavigationEvent::default());
    input.back_completed();
    assert!(log.get().is_empty()); // (back disabled)
    handler.set_is_back_enabled(true);
    input.back_started(NavigationEvent { progress: 0.25, ..NavigationEvent::default() });
    input.back_progressed(NavigationEvent { progress: 0.5, ..NavigationEvent::default() });
    input.back_cancelled();
    input.back_started(NavigationEvent::default());
    input.back_completed();
    assert_eq!(log.get(), "başla 0.25 ilerle 0.5 vazgeçti başla 0.0 bitti ");
    let history = dispatcher.history().value().merged_history();
    assert_eq!(
        history.iter().map(|i| i.downcast_ref::<Screen>().cloned()).collect::<Vec<_>>(),
        [Some(Screen("sayfa"))]
    );
}

fn main() {
    common::run(&[
        ("back_handlers_as_activity_compose_samples_write_them", back_handlers_as_activity_compose_samples_write_them),
        (
            "remember_navigation_event_dispatcher_owner_as_its_sample_writes_it",
            remember_navigation_event_dispatcher_owner_as_its_sample_writes_it,
        ),
        ("navigation_event_handlers_as_an_app_writes_them", navigation_event_handlers_as_an_app_writes_them),
        ("navigation_event_inputs_as_an_app_writes_them", navigation_event_inputs_as_an_app_writes_them),
        (
            "navigation_event_handler_objects_as_activity_compose_writes_them",
            navigation_event_handler_objects_as_activity_compose_writes_them,
        ),
    ]);
}

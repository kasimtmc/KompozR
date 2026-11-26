//! Lifecycles in Kotlin's syntax, as a Compose user writes them: the C++ Kompoz's KitchenForms (kompozable_tests.cpp
//! `state_forms_as_kotlin_writes_them`: its lifecycle lines — `collectAsStateWithLifecycle()` as a member,
//! `LocalLifecycleOwner.current.lifecycle.currentStateAsState()`, `Lifecycle.State.RESUMED` — and the rest of it) and
//! lifecycle-runtime-compose's samples from kt/lifecycle.kt (kompoz_file!); the rest of the API as an app writes it
//! (LifecycleForms: the effects with their scopes' `onStopOrDispose { }`/`onPauseOrDispose { }`, `dropUnless…`,
//! `LifecycleEventObserver { source, event -> }`, `Lifecycle.Event.upTo`, `repeatOnLifecycle`, `withStarted`,
//! `flowWithLifecycle`, `lifecycleScope`). The same behaviour crates/kompoz-lifecycle/tests/lifecycle.rs checks in
//! Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
// (what KitchenForms' `semantics { contentType = ContentType.Username }` names: Compose's androidx.compose.ui.autofill)
use kompoz::ui::semantics::ContentType;
use std::cell::RefCell;
use std::rc::Rc;

// ---- what the tests log to ----------------------------------------------------------------------------------------

/// The time ScreenAnalytics tracks (as the samples' `TimeAnalytics`): noted in the log.
#[derive(Clone)]
pub struct LoggedTime(Rc<RefCell<Vec<String>>>);

impl LoggedTime {
    pub fn stop_time_tracking(&self) -> LoggedTime {
        self.0.borrow_mut().push("stop".into());
        self.clone()
    }
}

/// What ScreenAnalytics reports to (as the samples' `DataAnalytics`): a log of what it is told.
#[derive(Clone)]
pub struct LoggedAnalytics(Rc<RefCell<Vec<String>>>);

impl PartialEq for LoggedAnalytics {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl LoggedAnalytics {
    pub fn track_screen_view(&self, screen_name: impl std::fmt::Display) {
        self.0.borrow_mut().push(format!("view {screen_name}"));
    }
    pub fn start_time_tracking(&self) -> LoggedTime {
        self.0.borrow_mut().push("start".into());
        LoggedTime(self.0.clone())
    }
    #[allow(dead_code)]
    pub fn send_disposal_analytics(&self, _time_analytics: LoggedTime) {
        self.0.borrow_mut().push("disposal".into());
    }
    fn taken(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}

fn analytics() -> LoggedAnalytics {
    LoggedAnalytics(Rc::default())
}

kompoz_file!("tests/kt/lifecycle.kt");

// ---- the rest of the API as an app writes it ------------------------------------------------------------------------

kompoz! {
    // The effects as the samples' local Analytics write them (kt/lifecycle.kt: functions of their samples, called by
    // none), in one composable of the app's, reporting to a log of Rust's.
    @Composable
    fun ScreenAnalytics(dataAnalytics: LoggedAnalytics) {
        LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { dataAnalytics.trackScreenView("screen1") }
        LifecycleStartEffect(dataAnalytics) {
            val timeTracker = dataAnalytics.startTimeTracking()
            onStopOrDispose { timeTracker.stopTimeTracking() }
        }
        LifecycleResumeEffect(dataAnalytics, LocalLifecycleOwner.current) {
            dataAnalytics.trackScreenView("resumed at ${lifecycle.currentState}")
            onPauseOrDispose { dataAnalytics.trackScreenView("paused") }
        }
    }

    @Composable
    fun Navigating(navigate: (String) -> Unit) {
        Column {
            Button(onClick = dropUnlessStarted { navigate("başladı") }, modifier = Modifier.testTag("başladıysa")) { Text("ileri") }
            Button(onClick = dropUnlessResumed { navigate("önde") }, modifier = Modifier.testTag("öndeyse")) { Text("ileri") }
        }
    }

    // A flow of one's own (`flow { emit(…) }`), collected while started.
    fun countdown(from: Int): Flow<Int> = flow {
        for (i in from downTo 1) emit(i)
    }

    @Composable
    fun Countdown() {
        val flow = remember { countdown(3) }
        val left by flow.collectAsStateWithLifecycle(initialValue = 0)
        Text("geri sayım $left")
    }

    // An observer of one's own, the event a state is reached by, the runtime's suspending calls in effects.
    @Composable
    fun Observing(seen: (String) -> Unit, ticks: StateFlow<Int>) {
        val owner = LocalLifecycleOwner.current
        DisposableEffect(owner) {
            val observer = LifecycleEventObserver { source, event ->
                if (event == Lifecycle.Event.upTo(Lifecycle.State.RESUMED)) seen("gözlem ${source.lifecycle.currentState}")
            }
            owner.lifecycle.addObserver(observer)
            onDispose { owner.lifecycle.removeObserver(observer) }
        }
        LaunchedEffect(owner) {
            owner.lifecycle.repeatOnLifecycle(Lifecycle.State.RESUMED) {
                seen("tekrar")
            }
        }
        LaunchedEffect(Unit) {
            val value = owner.lifecycle.withStarted { 42 }
            seen("başladı $value")
            ticks.asFlow().flowWithLifecycle(owner.lifecycle, Lifecycle.State.RESUMED).collect { seen("tik $it") }
        }
    }
}

// ---- tests ----------------------------------------------------------------------------------------------------------

/// C++ `state_forms_as_kotlin_writes_them` (kompozable_tests.cpp 3228), whole: orders sent without waiting (one more
/// than the channel holds refused) and served by a suspending consumer, a producer's countdown, KitchenForms' state
/// open while the app is resumed or started, closed in the background; its ticks the TickViewModel's (the host's
/// ViewModel), collected while started.
fn state_forms_as_kotlin_writes_them() {
    let rule = KompozTestRule::with_viewport(kompozable_lambda!(|| KitchenForms()), IntSize::new(400, 800));
    rule.set_lifecycle_state(LifecycleState::Resumed);
    rule.wait_for_idle();
    rule.on_node_with_text("servis 0 · ret 0 · ").assert_exists();
    rule.on_node_with_text("durum açık · 400 ms · tik var").assert_exists();
    rule.on_node_with_tag("ver").perform_click();
    rule.advance_time_by(50);
    rule.wait_for_idle();
    rule.on_node_with_text("servis 1 · ret 0 · ").assert_exists();
    rule.on_node_with_text("say").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("servis 1 · ret 0 · 321!").assert_exists();
    let alan = rule.on_node_with_tag("alan").get_unclipped_bounds_in_root();
    assert!((alan.left - 12.0).abs() < 0.5 && (alan.top - 8.0).abs() < 0.5, "{alan:?}"); // PaddingValues(12.dp, 8.dp)
    // (in the background: closed — the lifecycle's state as state)
    rule.set_lifecycle_state(LifecycleState::Created);
    rule.wait_for_idle();
    rule.on_node_with_text("durum kapalı · 400 ms · tik var").assert_exists();
}

/// lifecycle-runtime-compose's samples: a StateFlow's and a Flow's values (of their local classes) while started,
/// dropUnlessStarted and dropUnlessResumed, rememberLifecycleOwner capped and detached (the effects' samples'
/// Analytics are local functions they don't call: their effects as ScreenAnalytics, in
/// lifecycle_effects_as_compose_writes_them).
fn lifecycle_runtime_compose_samples() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                StateFlowCollectAsStateWithLifecycle();
                FlowCollectAsStateWithLifecycle();
                DropUnlessStarted();
                DropUnlessResumed();
                ComposableWithMaxLifecycle();
                ComposableWithParentNull();
            },
        );
    }));
    rule.on_node_with_text("0").assert_exists();
    rule.advance_time_by(1000);
    rule.on_node_with_text("1").assert_exists();
    // Capped at STARTED, the app resumed; detached from it: as capped.
    assert_eq!(rule.on_all_nodes_with_text("Current lifecycle state is STARTED").fetch_semantics_nodes().len(), 2);
    rule.set_lifecycle_state(LifecycleState::Created);
    rule.wait_for_idle();
    rule.on_node_with_text("Current lifecycle state is CREATED").assert_exists(); // (the capped one follows the app)
    rule.on_node_with_text("Current lifecycle state is STARTED").assert_exists(); // (the detached one doesn't)
    rule.advance_time_by(3000);
    rule.on_node_with_text("1").assert_exists(); // (not collected in the background)
    rule.set_lifecycle_state(LifecycleState::Resumed);
    rule.main_clock().run_current();
    rule.on_node_with_text("0").assert_exists(); // (collected again: a new collection of the cold flow)
    assert_eq!(rule.on_all_nodes_with_text("Navigate to next screen").fetch_semantics_nodes().len(), 2);
}

/// The effects of the samples' Analytics (ScreenAnalytics), with their scopes: the time tracked while started and resumed, stopped as
/// the app goes to the background and as they leave; the screen view on each resume. dropUnless… drop the clicks
/// below their states.
fn lifecycle_effects_as_compose_writes_them() {
    let data = analytics();
    let shown = mutable_state_of(true);
    let navigated = Rc::new(RefCell::new(Vec::<String>::new()));
    let (d, sh, n) = (data.clone(), shown.clone(), navigated.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        if sh.get() {
            ScreenAnalytics(d.clone());
        }
        let n = n.clone();
        Navigating(move |to: String| n.borrow_mut().push(to));
    }));
    assert_eq!(data.taken(), ["view screen1", "start", "view resumed at RESUMED"]);
    rule.set_lifecycle_state(LifecycleState::Started);
    assert_eq!(data.taken(), ["view paused"]);
    rule.on_node_with_tag("başladıysa").perform_click();
    rule.on_node_with_tag("öndeyse").perform_click();
    assert_eq!(navigated.borrow_mut().drain(..).collect::<Vec<_>>(), ["başladı"]);
    rule.set_lifecycle_state(LifecycleState::Created);
    assert_eq!(data.taken(), ["stop"]);
    rule.on_node_with_tag("başladıysa").perform_click();
    assert!(navigated.borrow().is_empty());
    rule.set_lifecycle_state(LifecycleState::Resumed);
    assert_eq!(data.taken(), ["view screen1", "start", "view resumed at RESUMED"]);
    rule.on_node_with_tag("öndeyse").perform_click();
    assert_eq!(navigated.borrow_mut().drain(..).collect::<Vec<_>>(), ["önde"]);
    shown.set(false);
    rule.wait_for_idle();
    let mut left = data.taken();
    left.sort();
    assert_eq!(left, ["stop", "view paused"]);
}

/// An observer of one's own (a SAM-constructed `LifecycleEventObserver`), `Lifecycle.Event.upTo`, `repeatOnLifecycle`,
/// `withStarted` and `flowWithLifecycle` in effects.
fn lifecycle_observers_and_suspending_calls_as_compose_writes_them() {
    let seen = Rc::new(RefCell::new(Vec::<String>::new()));
    let ticks = MutableStateFlow::new(0);
    let (s, t) = (seen.clone(), ticks.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Observing(move |v: String| s.borrow_mut().push(v), t.as_state_flow());
    }));
    let taken = || {
        let mut v = std::mem::take(&mut *seen.borrow_mut());
        v.sort();
        v
    };
    assert_eq!(taken(), ["başladı 42", "gözlem RESUMED", "tekrar", "tik 0"]);
    rule.set_lifecycle_state(LifecycleState::Started);
    rule.wait_for_idle();
    ticks.set_value(1);
    rule.wait_for_idle();
    assert!(taken().is_empty()); // (paused: no ticks)
    rule.set_lifecycle_state(LifecycleState::Resumed);
    rule.wait_for_idle();
    assert_eq!(taken(), ["gözlem RESUMED", "tekrar", "tik 1"]);
    // (a Kotlin flow of one's own)
    let rule = KompozTestRule::new(Countdown);
    rule.on_node_with_text("geri sayım 1").assert_exists();
}

fn main() {
    common::run(&[
        ("state_forms_as_kotlin_writes_them", state_forms_as_kotlin_writes_them),
        ("lifecycle_runtime_compose_samples", lifecycle_runtime_compose_samples),
        ("lifecycle_effects_as_compose_writes_them", lifecycle_effects_as_compose_writes_them),
        (
            "lifecycle_observers_and_suspending_calls_as_compose_writes_them",
            lifecycle_observers_and_suspending_calls_as_compose_writes_them,
        ),
    ]);
}

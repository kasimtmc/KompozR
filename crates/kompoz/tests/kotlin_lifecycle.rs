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
use std::time::Duration;

// ---- Rust twins of the Kotlin classes (stage 14) -------------------------------------------------------------------

/// LifecycleComposeSamples' `class ExampleState { private val _uiState = MutableStateFlow(""); val uiState:
/// StateFlow<String> = _uiState.asStateFlow() }` (StateFlowCollectAsStateWithLifecycle's).
#[derive(Clone)]
pub struct ExampleStateClass {
    #[allow(dead_code)]
    ui_state_source: MutableStateFlow<String>,
    ui_state: StateFlow<String>,
}

impl ExampleStateClass {
    pub fn ui_state(&self) -> StateFlow<String> {
        self.ui_state.clone()
    }
}

pub fn ExampleState() -> ExampleStateClass {
    let source = MutableStateFlow::new(String::new());
    ExampleStateClass { ui_state: source.as_state_flow(), ui_state_source: source }
}

/// LifecycleComposeSamples' second `class ExampleState { val counter = flow { … } }` (FlowCollectAsStateWithLifecycle's).
#[derive(Clone)]
pub struct CounterStateClass {
    counter: Flow<i32>,
}

impl CounterStateClass {
    pub fn counter(&self) -> Flow<i32> {
        self.counter.clone()
    }
}

pub fn CounterState() -> CounterStateClass {
    CounterStateClass {
        counter: flow(|emit| async move {
            let mut count = 0;
            loop {
                emit.emit(count).await;
                count += 1;
                delay_ms(1000).await;
            }
        }),
    }
}

/// KitchenForms' `class TickViewModel : ViewModel() { val ticks: StateFlow<Int> = flow<Int> { var i = 0; while (true)
/// { emit(i++); delay(100) } }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), 0) }`: its ticks, in the
/// scope given (its `viewModelScope`).
fn tick_view_model_ticks(view_model_scope: &CoroutineScope) -> StateFlow<i32> {
    flow(|emit| async move {
        let mut i = 0;
        loop {
            emit.emit(i).await;
            i += 1;
            delay_ms(100).await;
        }
    })
    .state_in(view_model_scope, SharingStarted::WhileSubscribed(Duration::from_millis(5000)), 0)
}

/// The samples' `private interface TimeAnalytics { fun stopTimeTracking(): TimeAnalytics }`: here one that logs.
#[derive(Clone)]
pub struct TimeAnalytics(Rc<RefCell<Vec<String>>>);

impl TimeAnalytics {
    pub fn stop_time_tracking(&self) -> TimeAnalytics {
        self.0.borrow_mut().push("stop".into());
        self.clone()
    }
}

/// The samples' `private interface DataAnalytics`: here one that logs what it is told.
#[derive(Clone)]
pub struct DataAnalytics(Rc<RefCell<Vec<String>>>);

impl PartialEq for DataAnalytics {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl DataAnalytics {
    pub fn track_screen_view(&self, screen_name: impl std::fmt::Display) {
        self.0.borrow_mut().push(format!("view {screen_name}"));
    }
    pub fn start_time_tracking(&self) -> TimeAnalytics {
        self.0.borrow_mut().push("start".into());
        TimeAnalytics(self.0.clone())
    }
    pub fn send_disposal_analytics(&self, _time_analytics: TimeAnalytics) {
        self.0.borrow_mut().push("disposal".into());
    }
    fn taken(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}

fn analytics() -> DataAnalytics {
    DataAnalytics(Rc::default())
}

kompoz_file!("tests/kt/lifecycle.kt");

// ---- the rest of the API as an app writes it ------------------------------------------------------------------------

kompoz! {
    // The samples' Analytics, each an effect (their nested functions here at the top).
    @Composable
    fun ScreenAnalytics(dataAnalytics: DataAnalytics) {
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

/// C++ `state_forms_as_kotlin_writes_them` (kompozable_tests.cpp 3228), its lifecycle lines: KitchenForms' state
/// open while the app is resumed or started, closed in the background; its ticks those of TickViewModel's flow
/// (the ViewModel itself is stage 13 WP1's), collected while started.
fn state_forms_as_kotlin_writes_them() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let scope = remember_coroutine_scope();
            let ticks = remember(|| tick_view_model_ticks(&scope));
            KitchenForms(ticks);
        }),
        IntSize::new(400, 800),
    );
    rule.set_lifecycle_state(LifecycleState::Resumed);
    rule.wait_for_idle();
    rule.on_node_with_text("durum açık · 400 ms · tik var").assert_exists();
    let alan = rule.on_node_with_tag("alan").get_unclipped_bounds_in_root();
    assert!((alan.left - 12.0).abs() < 0.5 && (alan.top - 8.0).abs() < 0.5, "{alan:?}"); // PaddingValues(12.dp, 8.dp)
    // (in the background: closed — the lifecycle's state as state)
    rule.set_lifecycle_state(LifecycleState::Created);
    rule.wait_for_idle();
    rule.on_node_with_text("durum kapalı · 400 ms · tik var").assert_exists();
}

/// lifecycle-runtime-compose's samples: a StateFlow's and a Flow's values while started, dropUnlessStarted and
/// dropUnlessResumed, rememberLifecycleOwner capped and detached (the effects' samples: ScreenAnalytics, in
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

/// The effects of the samples' Analytics, with their scopes: the time tracked while started and resumed, stopped as
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

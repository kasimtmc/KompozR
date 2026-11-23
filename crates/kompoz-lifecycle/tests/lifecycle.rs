//! The lifecycles' tests: the C++ Kompoz's lifecycle tests by name (in Compose's semantics where the C++'s differ:
//! said at each), and Compose's behaviour of lifecycle-common, lifecycle-runtime and lifecycle-runtime-compose —
//! the registry's order and reentrance, `DefaultLifecycleObserver`, the coroutine scope, `repeatOnLifecycle`,
//! `withStateAtLeast`, the deprecated pausing `whenStarted`, the flows, `rememberLifecycleOwner`, `dropUnless…`.

#![allow(non_snake_case, deprecated)]

use kompoz_coroutines::{AsFlow, CoroutineScope, Dispatcher, MutableStateFlow, delay_ms};
use kompoz_lifecycle::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, Text};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

/// An owner of a registry of its own (a screen's).
struct TestOwner {
    registry: LifecycleRegistry,
}

impl LifecycleOwner for TestOwner {
    fn lifecycle(&self) -> Lifecycle {
        self.registry.lifecycle()
    }
}

fn owner() -> Rc<TestOwner> {
    Rc::new_cyclic(|me: &Weak<TestOwner>| TestOwner { registry: LifecycleRegistry::new(me.clone()) })
}

/// An observer logging each event as `label EVENT`.
fn logging(log: &Log, label: &'static str) -> Rc<dyn LifecycleObserver> {
    let log = log.clone();
    Rc::new(move |_: &dyn LifecycleOwner, e: LifecycleEvent| log.borrow_mut().push(format!("{label} {e}")))
}

/// What `f` panics with (its message), the panic hook quiet meanwhile; none if it doesn't.
fn thrown<R>(f: impl FnOnce() -> R) -> Option<String> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(hook);
    let e = result.err()?;
    Some(
        e.downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default(),
    )
}

// ---- C++ tests ------------------------------------------------------------------------------------------------

/// C++ 23215. Compose's LifecycleRegistry where the C++'s differs: leaving DESTROYED, and going from INITIALIZED
/// straight to DESTROYED, are errors (IllegalStateException; the C++'s ignored them and told nothing), and so is
/// stepping back to INITIALIZED (the C++'s "no way back: it stays created" — Compose's tells its observers ON_DESTROY
/// and then fails finding no event up from DESTROYED).
fn lifecycle_steps_through_its_events_and_new_observers_catch_up() {
    use LifecycleEvent::*;
    use LifecycleState as S;
    let owner = owner();
    let lifecycle = owner.lifecycle();
    let (first, second) = (Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())));
    let f = first.clone();
    let a: Rc<dyn LifecycleObserver> = Rc::new(move |_: &dyn LifecycleOwner, e| f.borrow_mut().push(e));
    lifecycle.add_observer(a.clone());
    assert!(first.borrow().is_empty() && lifecycle.current_state() == S::Initialized);
    owner.registry.set_current_state(S::Resumed); // one step at a time
    assert_eq!(*first.borrow(), [OnCreate, OnStart, OnResume]);
    assert_eq!(lifecycle.current_state_flow().value(), S::Resumed);
    owner.registry.set_current_state(S::Created);
    assert_eq!(*first.borrow(), [OnCreate, OnStart, OnResume, OnPause, OnStop]);
    let s = second.clone();
    lifecycle.add_observer(Rc::new(move |_: &dyn LifecycleOwner, e| s.borrow_mut().push(e))); // brought up to now
    assert_eq!(*second.borrow(), [OnCreate]);
    lifecycle.remove_observer(a); // removed
    owner.registry.set_current_state(S::Started);
    assert_eq!(first.borrow().len(), 5);
    assert_eq!(*second.borrow(), [OnCreate, OnStart]);
    owner.registry.set_current_state(S::Destroyed);
    assert_eq!(*second.borrow(), [OnCreate, OnStart, OnStop, OnDestroy]);
    assert_eq!(lifecycle.current_state(), S::Destroyed);
    // Destroyed is for good: leaving it is an error.
    let error = thrown(|| owner.registry.set_current_state(S::Resumed)).expect("an error");
    assert!(error.contains("State is 'DESTROYED' and cannot be moved to `RESUMED`"), "{error}");
    let late = Rc::new(RefCell::new(Vec::new()));
    let l = late.clone();
    lifecycle.add_observer(Rc::new(move |_: &dyn LifecycleOwner, e| l.borrow_mut().push(e)));
    assert!(lifecycle.current_state() == S::Destroyed && second.borrow().len() == 4 && late.borrow().is_empty());
    // Never created: destroying it is an error too.
    let never = self::owner();
    let l = late.clone();
    never.lifecycle().add_observer(Rc::new(move |_: &dyn LifecycleOwner, e| l.borrow_mut().push(e)));
    let error = thrown(|| never.registry.set_current_state(S::Destroyed)).expect("an error");
    assert!(error.contains("State must be at least 'CREATED' to be moved to 'DESTROYED'"), "{error}");
    assert!(late.borrow().is_empty() && never.lifecycle().current_state() == S::Initialized);
    // Back to INITIALIZED: told ON_DESTROY, then no event up from DESTROYED.
    let back = self::owner();
    let events = log();
    back.lifecycle().add_observer(logging(&events, "o"));
    back.registry.set_current_state(S::Created);
    let error = thrown(|| back.registry.set_current_state(S::Initialized)).expect("an error");
    assert!(error.contains("no event up from DESTROYED"), "{error}");
    assert_eq!(taken(&events), ["o ON_CREATE", "o ON_DESTROY"]);
}

/// C++ 23245, Compose's way where the C++'s differs: `LifecycleStartEffect`/`LifecycleResumeEffect` keep the
/// `effects` of the composition that started them while their keys stay (a DisposableEffect's block: the C++'s ran
/// the latest, "start b"); `LifecycleEventEffect` runs the latest `on_event` (`rememberUpdatedState`), as both have it.
fn lifecycle_effects_follow_the_app_lifecycle() {
    let log = log();
    let resumed = Rc::new(Cell::new(0));
    let (shown, label) = (mutable_state_of(true), mutable_state_of("a".to_string()));
    let (lg, r, sh, lb) = (log.clone(), resumed.clone(), shown.clone(), label.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let state = LocalLifecycleOwner.current().lifecycle().current_state_as_state().get();
        Text(state.name().to_lowercase());
        if !sh.get() {
            return;
        }
        let l = lb.get();
        let r = r.clone();
        LifecycleEventEffect(LifecycleEvent::OnResume, on_event = move || r.set(r.get() + 1));
        let (log1, l1) = (lg.clone(), l.clone());
        LifecycleStartEffect(
            0,
            effects = move |scope: &LifecycleStartStopEffectScope| {
                log1.borrow_mut().push(format!("start {l1}"));
                let (log, l) = (log1.clone(), l1.clone());
                scope.on_stop_or_dispose(move |_| log.borrow_mut().push(format!("stop {l}")))
            },
        );
        let (log2, l2) = (lg.clone(), l.clone());
        LifecycleResumeEffect(
            0,
            effects = move |scope: &LifecycleResumePauseEffectScope| {
                log2.borrow_mut().push(format!("resume {l2}"));
                let (log, l) = (log2.clone(), l2.clone());
                scope.on_pause_or_dispose(move |_| log.borrow_mut().push(format!("pause {l}")))
            },
        );
    }));
    rule.on_node_with_text("resumed").assert_exists();
    assert_eq!((resumed.get(), taken(&log)), (1, vec!["start a".to_string(), "resume a".to_string()]));
    rule.set_lifecycle_state(LifecycleState::Started); // told at once, not on the next frame
    assert_eq!((resumed.get(), log.borrow().clone()), (1, vec!["pause a".to_string()]));
    rule.set_lifecycle_state(LifecycleState::Created);
    assert_eq!(*log.borrow(), ["pause a", "stop a"]);
    rule.wait_for_idle();
    rule.on_node_with_text("created").assert_exists();
    label.set("b".into()); // keyed the same: not started again
    rule.wait_for_idle();
    taken(&log);
    rule.set_lifecycle_state(LifecycleState::Resumed);
    assert_eq!(taken(&log), ["start a", "resume a"]); // (the effects that started them: Compose's)
    assert_eq!(resumed.get(), 2); // (the latest on_event)
    shown.set(false); // leaving the composition while resumed: stopped
    rule.wait_for_idle();
    let left = taken(&log);
    assert!(left.len() == 2 && left.contains(&"pause a".to_string()) && left.contains(&"stop a".to_string()));
    rule.set_lifecycle_state(LifecycleState::Started);
    assert!(log.borrow().is_empty() && resumed.get() == 2);
}

/// C++ 23287: Compose's `collectAsStateWithLifecycle` (`repeatOnLifecycle` in `produceState`).
fn collect_as_state_with_lifecycle_stops_collecting_in_the_background() {
    let flow = MutableStateFlow::new(1);
    let f = flow.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Text(format!("değer {}", f.as_state_flow().collect_as_state_with_lifecycle().get()));
        let front = f.as_state_flow().collect_as_state_with_lifecycle(min_active_state = LifecycleState::Resumed);
        Text(format!("ön {}", front.get()));
    }));
    rule.on_node_with_text("değer 1").assert_exists();
    assert_eq!(flow.subscription_count(), 2);
    flow.set_value(2);
    rule.wait_for_idle();
    rule.on_node_with_text("değer 2").assert_exists();
    rule.on_node_with_text("ön 2").assert_exists();
    rule.set_lifecycle_state(LifecycleState::Started); // paused: the RESUMED one lets go
    assert_eq!(flow.subscription_count(), 1);
    flow.set_value(3);
    rule.wait_for_idle();
    rule.on_node_with_text("değer 3").assert_exists();
    rule.on_node_with_text("ön 2").assert_exists();
    rule.set_lifecycle_state(LifecycleState::Created); // in the background: neither collects
    assert_eq!(flow.subscription_count(), 0);
    flow.set_value(4);
    rule.wait_for_idle();
    rule.on_node_with_text("değer 3").assert_exists();
    rule.on_node_with_text("ön 2").assert_exists();
    rule.set_lifecycle_state(LifecycleState::Resumed); // back: they catch up
    rule.wait_for_idle();
    rule.on_node_with_text("değer 4").assert_exists();
    rule.on_node_with_text("ön 4").assert_exists();
    assert_eq!(flow.subscription_count(), 2);
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

struct Recorder(Log);

impl DefaultLifecycleObserver for Recorder {
    fn on_create(&self, _: &dyn LifecycleOwner) {
        self.0.borrow_mut().push("onCreate".into());
    }
    fn on_resume(&self, _: &dyn LifecycleOwner) {
        self.0.borrow_mut().push("onResume".into());
    }
    fn on_destroy(&self, _: &dyn LifecycleOwner) {
        self.0.borrow_mut().push("onDestroy".into());
    }
}

impl LifecycleEventObserver for Recorder {
    fn on_state_changed(&self, _: &dyn LifecycleOwner, event: LifecycleEvent) {
        self.0.borrow_mut().push(format!("event {event}"));
    }
}

impl LifecycleObserver for Recorder {
    fn as_default_lifecycle_observer(&self) -> Option<&dyn DefaultLifecycleObserver> {
        Some(self)
    }
    fn as_lifecycle_event_observer(&self) -> Option<&dyn LifecycleEventObserver> {
        Some(self)
    }
}

/// Up in the order observers were added (each brought all the way before the next), down in the reverse; an
/// observer added from an event isn't brought past the observer being told (Compose's parent states: it is brought
/// up after);
/// a DefaultLifecycleObserver's method comes before its event observer side; `addObserver { }` gives the observer
/// it made, and the registry counts them.
fn the_registry_tells_its_observers_in_order() {
    let owner = owner();
    let lifecycle = owner.lifecycle();
    let events = log();
    lifecycle.add_observer(logging(&events, "a"));
    let (l, e) = (lifecycle.clone(), events.clone());
    let added = Rc::new(Cell::new(false));
    let ad = added.clone();
    // (b adds c as it is created: c stays where b was until the pass reaches it)
    lifecycle.add_observer(Rc::new(move |_: &dyn LifecycleOwner, event: LifecycleEvent| {
        e.borrow_mut().push(format!("b {event}"));
        if event == LifecycleEvent::OnCreate && !ad.replace(true) {
            assert_eq!(l.current_state(), LifecycleState::Resumed);
            l.add_observer(logging(&e, "c"));
        }
    }));
    owner.registry.set_current_state(LifecycleState::Resumed);
    assert_eq!(
        taken(&events),
        [
            "a ON_CREATE",
            "a ON_START",
            "a ON_RESUME",
            "b ON_CREATE",
            "b ON_START",
            "b ON_RESUME",
            "c ON_CREATE",
            "c ON_START",
            "c ON_RESUME"
        ]
    );
    owner.registry.set_current_state(LifecycleState::Created);
    assert_eq!(taken(&events), ["c ON_PAUSE", "c ON_STOP", "b ON_PAUSE", "b ON_STOP", "a ON_PAUSE", "a ON_STOP"]);
    assert_eq!(owner.registry.observer_count(), 3);
    let recorded = log();
    lifecycle.add_observer(Rc::new(Recorder(recorded.clone())));
    assert_eq!(taken(&recorded), ["onCreate", "event ON_CREATE"]);
    let seen = log();
    let s = seen.clone();
    let observer = lifecycle.add_observer_with(move |_, _, event| s.borrow_mut().push(event.to_string()));
    assert_eq!(taken(&seen), ["ON_CREATE"]);
    lifecycle.remove_observer(observer);
    owner.registry.handle_lifecycle_event(LifecycleEvent::OnResume);
    assert!(seen.borrow().is_empty());
    assert_eq!(taken(&recorded), ["event ON_START", "onResume", "event ON_RESUME"]);
    owner.registry.set_current_state(LifecycleState::Destroyed);
    assert_eq!(taken(&recorded), ["event ON_PAUSE", "event ON_STOP", "onDestroy", "event ON_DESTROY"]);
    assert_eq!(owner.registry.observer_count(), 0); // (let go once destroyed)
    assert_eq!(LifecycleEvent::OnPause.target_state(), LifecycleState::Started);
    assert_eq!(thrown(|| LifecycleEvent::OnAny.target_state()).as_deref(), Some("ON_ANY has no target state"));
    assert!(LifecycleState::Resumed.is_at_least(LifecycleState::Started) && LifecycleState::Created.ordinal() == 2);
    assert_eq!(LifecycleEvent::up_to(LifecycleState::Started), Some(LifecycleEvent::OnStart));
    assert_eq!(LifecycleEvent::down_from(LifecycleState::Destroyed), None);
}

/// `repeatOnLifecycle(STARTED)` runs its block at each `ON_START` and cancels it at each `ON_STOP`, done once the
/// lifecycle is destroyed; `withStarted` runs its block once the state is reached (at once if it is), a destroyed
/// lifecycle cancels the waiting caller; the lifecycle's coroutine scope is cancelled with it; its event flow gives
/// what brings a collector up to now, then each event; `flowWithLifecycle` collects the upstream only while started.
fn the_runtimes_suspending_calls_follow_the_lifecycle() {
    // (what independent coroutines did, in no particular order)
    fn sorted(log: &Log) -> Vec<String> {
        let mut v = taken(log);
        v.sort();
        v
    }
    let rule = KompozTestRule::new(kompozable_lambda!(|| {}));
    let owner = owner();
    let lifecycle = owner.lifecycle();
    let scope = CoroutineScope::new(&Dispatcher::main());
    let events = log();
    let (l, e) = (lifecycle.clone(), events.clone());
    let repeating = scope.launch(async move {
        let e2 = e.clone();
        l.repeat_on_lifecycle(LifecycleState::Started, move |_| {
            let e = e2.clone();
            async move {
                e.borrow_mut().push("run".into());
                struct Ends(Log);
                impl Drop for Ends {
                    fn drop(&mut self) {
                        self.0.borrow_mut().push("cancelled".into());
                    }
                }
                let _ends = Ends(e.clone());
                std::future::pending::<()>().await;
            }
        })
        .await;
        e.borrow_mut().push("done".into());
    });
    let (l, e) = (lifecycle.clone(), events.clone());
    scope.launch(async move {
        let v = l.with_started(|| 7).await;
        e.borrow_mut().push(format!("started {v}"));
    });
    let (l, e) = (lifecycle.clone(), events.clone());
    let flowing = scope.launch(async move {
        let e2 = e.clone();
        l.event_flow().collect(move |event| e2.borrow_mut().push(format!("flow {event}"))).await;
        e.borrow_mut().push("flow done".into());
    });
    rule.wait_for_idle();
    assert!(taken(&events).is_empty());
    owner.registry.set_current_state(LifecycleState::Started);
    rule.wait_for_idle();
    assert_eq!(sorted(&events), ["flow ON_CREATE", "flow ON_START", "run", "started 7"]);
    owner.registry.set_current_state(LifecycleState::Created);
    assert_eq!(taken(&events), ["cancelled"]); // (at once)
    rule.wait_for_idle();
    assert_eq!(taken(&events), ["flow ON_STOP"]);
    owner.registry.set_current_state(LifecycleState::Resumed);
    rule.wait_for_idle();
    assert_eq!(sorted(&events), ["flow ON_RESUME", "flow ON_START", "run"]);
    // (already started: at once)
    let (l, e) = (lifecycle.clone(), events.clone());
    scope.launch(async move {
        let v = l.with_resumed(|| 8).await;
        e.borrow_mut().push(format!("again {v}"));
    });
    rule.wait_for_idle();
    assert_eq!(taken(&events), ["again 8"]);
    // A waiting caller of a lifecycle destroyed is cancelled; the lifecycle's own scope too.
    owner.registry.set_current_state(LifecycleState::Created);
    let (l, e) = (lifecycle.clone(), events.clone());
    let waiting = scope.launch(async move {
        l.with_resumed(|| ()).await;
        e.borrow_mut().push("never".into());
    });
    let tied = lifecycle.coroutine_scope();
    assert!(tied == lifecycle.coroutine_scope()); // (the same scope each time)
    let e = events.clone();
    let tied_job = tied.launch(async move {
        std::future::pending::<()>().await;
        e.borrow_mut().push("never".into());
    });
    rule.wait_for_idle();
    taken(&events);
    owner.registry.set_current_state(LifecycleState::Destroyed);
    rule.wait_for_idle();
    assert_eq!(sorted(&events), ["done", "flow ON_DESTROY", "flow done"]);
    assert!(repeating.is_completed() && flowing.is_completed() && waiting.is_cancelled());
    assert!(tied_job.is_cancelled() && !tied.is_active());
    // flowWithLifecycle: the upstream collected only while started.
    let source = MutableStateFlow::new(1);
    let owner = self::owner();
    let collected = log();
    let (src, c, l) = (source.clone(), collected.clone(), owner.lifecycle());
    scope.launch(async move {
        let c2 = c.clone();
        AsFlow::as_flow(&src.as_state_flow())
            .flow_with_lifecycle(l) // (STARTED: its default)
            .collect(move |v: i32| c2.borrow_mut().push(v.to_string()))
            .await;
        c.borrow_mut().push("end".into());
    });
    rule.wait_for_idle();
    assert!(taken(&collected).is_empty() && source.subscription_count() == 0);
    owner.registry.set_current_state(LifecycleState::Started);
    rule.wait_for_idle();
    assert_eq!((taken(&collected), source.subscription_count()), (vec!["1".to_string()], 1));
    source.set_value(2);
    rule.wait_for_idle();
    owner.registry.set_current_state(LifecycleState::Created);
    rule.wait_for_idle();
    source.set_value(3);
    rule.wait_for_idle();
    assert_eq!((taken(&collected), source.subscription_count()), (vec!["2".to_string()], 0));
    owner.registry.set_current_state(LifecycleState::Destroyed);
    rule.wait_for_idle();
    assert_eq!(taken(&collected), ["end"]);
    scope.cancel();
}

/// The deprecated `whenStarted`: its block runs while the lifecycle is started, suspended (not cancelled) while it
/// is below — the coroutines it launches in its scope too — and the caller is cancelled once it is destroyed.
fn when_started_suspends_its_block_below_the_state() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {}));
    let owner = owner();
    let scope = CoroutineScope::new(&Dispatcher::main());
    let steps = log();
    let (o, s) = (owner.clone(), steps.clone());
    let job = scope.launch(async move {
        let s2 = s.clone();
        let v = o
            .when_started(move |inner: CoroutineScope| async move {
                let s3 = s2.clone();
                inner.launch(async move {
                    for i in 0..3 {
                        s3.borrow_mut().push(format!("child {i}"));
                        delay_ms(100).await;
                    }
                });
                for i in 0..3 {
                    s2.borrow_mut().push(format!("step {i}"));
                    delay_ms(100).await;
                }
                5
            })
            .await;
        s.borrow_mut().push(format!("value {v}"));
    });
    rule.wait_for_idle();
    assert!(taken(&steps).is_empty()); // (not started yet)
    owner.registry.set_current_state(LifecycleState::Started);
    rule.advance_time_by(50);
    assert_eq!(taken(&steps), ["step 0", "child 0"]);
    owner.registry.set_current_state(LifecycleState::Created); // suspended, not cancelled
    rule.advance_time_by(500);
    assert!(taken(&steps).is_empty() && !job.is_completed());
    owner.registry.set_current_state(LifecycleState::Resumed);
    rule.wait_for_idle();
    assert_eq!(taken(&steps), ["step 1", "child 1", "step 2", "child 2", "value 5"]);
    // Destroyed while it waits: the caller cancelled.
    let (o, s) = (owner.clone(), steps.clone());
    owner.registry.set_current_state(LifecycleState::Created);
    let waiting = scope.launch(async move {
        o.lifecycle().when_resumed(|_| async {}).await;
        s.borrow_mut().push("never".into());
    });
    rule.wait_for_idle();
    owner.registry.set_current_state(LifecycleState::Destroyed);
    rule.wait_for_idle();
    assert!(waiting.is_cancelled() && taken(&steps).is_empty());
    scope.cancel();
}

/// `rememberLifecycleOwner`: its parent's state, capped at its maximum (the cap changed, it follows); with no parent,
/// resumed (capped); destroyed as it leaves the composition. `dropUnlessResumed`/`dropUnlessStarted` drop calls below
/// their state.
fn remembered_owners_follow_their_parent_capped_and_drop_unless_drops() {
    let (cap, shown, parentless) =
        (mutable_state_of(LifecycleState::Started), mutable_state_of(true), mutable_state_of(false));
    let owners: Rc<RefCell<Vec<Rc<dyn LifecycleOwner>>>> = Rc::default();
    let clicks = log();
    let actions: Rc<RefCell<Vec<Rc<dyn Fn()>>>> = Rc::default();
    let (c, sh, pl, ow, cl, ac) =
        (cap.clone(), shown.clone(), parentless.clone(), owners.clone(), clicks.clone(), actions.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (cl1, cl2) = (cl.clone(), cl.clone());
        let started = drop_unless_started(block = move || cl1.borrow_mut().push("started".into()));
        let resumed = drop_unless_resumed(block = move || cl2.borrow_mut().push("resumed".into()));
        *ac.borrow_mut() = vec![Rc::new(started), Rc::new(resumed)];
        if !sh.get() {
            return;
        }
        let owner = if pl.get() {
            remember_lifecycle_owner(max_lifecycle = c.get(), parent = None)
        } else {
            remember_lifecycle_owner(max_lifecycle = c.get())
        };
        ow.borrow_mut().push(owner.clone());
        KompozitionLocalProvider(&[LocalLifecycleOwner.provides(owner)], || {
            let state = LocalLifecycleOwner.current().lifecycle().current_state_as_state();
            Text(format!("içerde {}", state.get()));
        });
    }));
    let current = || owners.borrow().last().unwrap().lifecycle().current_state();
    rule.on_node_with_text("içerde STARTED").assert_exists();
    let click = |i: usize| (actions.borrow()[i].clone())();
    click(0);
    click(1);
    assert_eq!(taken(&clicks), ["started", "resumed"]);
    cap.set(LifecycleState::Resumed);
    rule.wait_for_idle();
    assert_eq!(current(), LifecycleState::Resumed);
    rule.on_node_with_text("içerde RESUMED").assert_exists();
    rule.set_lifecycle_state(LifecycleState::Started); // the parent below the cap: the parent's
    assert_eq!(current(), LifecycleState::Started);
    click(0);
    click(1);
    assert_eq!(taken(&clicks), ["started"]);
    rule.set_lifecycle_state(LifecycleState::Created);
    click(0);
    assert!(taken(&clicks).is_empty());
    rule.set_lifecycle_state(LifecycleState::Resumed);
    // No parent: resumed, as capped.
    parentless.set(true);
    rule.wait_for_idle();
    let detached = owners.borrow().last().unwrap().clone();
    assert_eq!(detached.lifecycle().current_state(), LifecycleState::Resumed);
    rule.set_lifecycle_state(LifecycleState::Created);
    assert_eq!(detached.lifecycle().current_state(), LifecycleState::Resumed); // (it follows no one)
    rule.set_lifecycle_state(LifecycleState::Resumed);
    // Leaving the composition: destroyed.
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(detached.lifecycle().current_state(), LifecycleState::Destroyed);
    // (ON_DESTROY isn't an event an effect can follow: Compose disposes of the composition first)
    let error = thrown(|| {
        KompozTestRule::new(kompozable_lambda!(|| LifecycleEventEffect(LifecycleEvent::OnDestroy, on_event = || {})))
    });
    assert!(error.is_some_and(|e| e.contains("cannot be used to listen for Lifecycle.Event.ON_DESTROY")));
}

/// The host's lifecycle owner is the root's LocalLifecycleOwner, resumed from the start, and destroyed when the
/// host goes (paused, stopped, destroyed, told while the content is still there).
fn the_hosts_lifecycle_is_the_apps() {
    let events = log();
    let e = events.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let e = e.clone();
        let owner = LocalLifecycleOwner.current();
        DisposableEffect((), move || {
            owner.lifecycle().add_observer(logging(&e, "kök"));
            on_dispose(|| {})
        });
    }));
    assert!(rule.host().lifecycle_owner().lifecycle().current_state() == LifecycleState::Resumed);
    assert_eq!(taken(&events), ["kök ON_CREATE", "kök ON_START", "kök ON_RESUME"]);
    drop(rule);
    assert_eq!(taken(&events), ["kök ON_PAUSE", "kök ON_STOP", "kök ON_DESTROY"]);
}

fn main() {
    set_ui_thread();
    let tests: &[(&str, fn())] = &[
        (
            "lifecycle_steps_through_its_events_and_new_observers_catch_up",
            lifecycle_steps_through_its_events_and_new_observers_catch_up,
        ),
        ("lifecycle_effects_follow_the_app_lifecycle", lifecycle_effects_follow_the_app_lifecycle),
        (
            "collect_as_state_with_lifecycle_stops_collecting_in_the_background",
            collect_as_state_with_lifecycle_stops_collecting_in_the_background,
        ),
        ("the_registry_tells_its_observers_in_order", the_registry_tells_its_observers_in_order),
        ("the_runtimes_suspending_calls_follow_the_lifecycle", the_runtimes_suspending_calls_follow_the_lifecycle),
        ("when_started_suspends_its_block_below_the_state", when_started_suspends_its_block_below_the_state),
        (
            "remembered_owners_follow_their_parent_capped_and_drop_unless_drops",
            remembered_owners_follow_their_parent_capped_and_drop_unless_drops,
        ),
        ("the_hosts_lifecycle_is_the_apps", the_hosts_lifecycle_is_the_apps),
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

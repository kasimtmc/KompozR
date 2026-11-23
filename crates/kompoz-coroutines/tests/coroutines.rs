//! The coroutines' tests: launching, jobs, scopes, timeouts, background work, flows, channels and their place in
//! the composition (the C++ Kompoz's coroutine, flow and channel tests).

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-runtime/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_coroutines::*;
use kompoz_runtime::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const MS: i64 = 1_000_000;

/// Composes content with a dispatcher, running frames as a host does.
struct CoRule {
    root: Rc<TestNode>,
    composition: Option<Composition>,
    dispatcher: Dispatcher,
    time: Cell<i64>,
}

impl CoRule {
    fn new(content: impl Content + 'static) -> CoRule {
        let root = TestNode::new("root");
        let composition = Composition::new(Some(root.clone()));
        let dispatcher = Dispatcher::new();
        let d = dispatcher.clone();
        let content = Rc::new(content);
        composition.set_content(kompozable_lambda!(move || KompozitionLocalProvider(
            &[LocalDispatcher.provides(Some(d.clone()))],
            || content.compose()
        )));
        let rule = CoRule { root, composition: Some(composition), dispatcher, time: Cell::new(0) };
        rule.frame();
        rule
    }

    fn composition(&self) -> &Composition {
        self.composition.as_ref().unwrap()
    }

    /// A frame at the current time: timers and frame waiters, then recomposition and what it woke, until settled.
    fn frame(&self) {
        let t = self.time.get();
        Snapshot::send_apply_notifications();
        self.dispatcher.run(t);
        for _ in 0..100 {
            Snapshot::send_apply_notifications();
            self.composition().invalidate_frame_scopes(t);
            let busy = self.composition().recompose(t);
            self.dispatcher.run_ready();
            Snapshot::send_apply_notifications();
            if !busy
                && !self.composition().has_invalidations()
                && !self.dispatcher.has_ready()
                && !global_snapshot_changed()
            {
                return;
            }
        }
        panic!("a frame that never settles");
    }

    /// Frames, 16 ms apart, until `ms` later (the last one at that time).
    fn advance_time_by(&self, ms: i64) {
        let target = self.time.get() + ms * MS;
        while self.time.get() < target {
            self.time.set((self.time.get() + 16 * MS).min(target));
            self.frame();
        }
    }

    /// Frames until nothing is left to do: no coroutine woken or waiting for a frame or a timer, no background
    /// work, nothing to recompose. Time jumps to the next timer when only timers are left.
    fn wait_for_idle(&self) {
        for _ in 0..100_000 {
            self.frame();
            if background_work_in_flight() > 0 {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            if self.dispatcher.has_ready() || self.composition().has_invalidations() || global_snapshot_changed() {
                continue;
            }
            let next = if self.dispatcher.has_frame_waiters() || self.composition().has_frame_scopes() {
                Some(self.time.get() + 16 * MS)
            } else {
                self.dispatcher.next_deadline()
            };
            match next {
                Some(t) => self.time.set(t.max(self.time.get() + MS)),
                None => return,
            }
        }
        panic!("never idle");
    }

    fn names(&self) -> Vec<String> {
        self.root.names()
    }

    fn has(&self, name: &str) -> bool {
        self.names().iter().any(|n| n == name)
    }

    fn has_pending_work(&self) -> bool {
        self.dispatcher.has_ready() || self.dispatcher.has_timers() || self.dispatcher.has_frame_waiters()
    }
}

type Log = Rc<RefCell<Vec<String>>>;

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn push(log: &Log, s: impl Into<String>) {
    log.borrow_mut().push(s.into());
}

/// Logs when a coroutine's future is dropped.
struct Guard(Log, &'static str);
impl Drop for Guard {
    fn drop(&mut self) {
        push(&self.0, format!("{} destroyed", self.1));
    }
}

/// Runs `f`, which is expected to panic, without the panic printed.
fn quietly<R>(f: impl FnOnce() -> R) -> R {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = f();
    std::panic::set_hook(hook);
    r
}

fn scope_of(rule_scope: &Rc<RefCell<Option<CoroutineScope>>>) -> CoroutineScope {
    rule_scope.borrow().clone().unwrap()
}

/// A rule whose content only remembers a coroutine scope; the scope.
fn scope_rule() -> (CoRule, CoroutineScope) {
    let cell = Rc::new(RefCell::new(None));
    let c = cell.clone();
    let rule = CoRule::new(kompozable_lambda!(move || *c.borrow_mut() = Some(remember_coroutine_scope())));
    let scope = scope_of(&cell);
    (rule, scope)
}

// ---------------------------------------------------------------------------------------------------------------

fn launched_effect_ticks_and_is_cancelled_on_leave() {
    let show = mutable_state_of(true);
    let ticks = mutable_state_of(0);
    let log = new_log();
    let (s, t, l) = (show.clone(), ticks.clone(), log.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        if s.get() {
            let (t, l) = (t.clone(), l.clone());
            LaunchedEffect(0, async move {
                let _guard = Guard(l, "ticker");
                loop {
                    delay_ms(1000).await;
                    t.set(t.peek() + 1);
                }
            });
        }
        Text(format!("ticks={t}"));
    }));
    rule.advance_time_by(3500);
    assert!(rule.has("ticks=3"), "{:?}", rule.names());
    show.set(false);
    rule.advance_time_by(2000);
    assert!(rule.has("ticks=3")); // no more ticks
    assert_eq!(*log.borrow(), ["ticker destroyed"]);
    assert!(!rule.dispatcher.has_timers());
}

async fn doubled_later(x: i32) -> i32 {
    delay_ms(100).await;
    x * 2
}

/// One awaiting another; a negative one is an error.
async fn described(x: i32) -> Result<String, String> {
    let d = doubled_later(x).await;
    if d < 0 {
        return Err("negative".into());
    }
    Ok(format!("twice {d}"))
}

fn coroutines_give_back_values_and_work_in_the_background() {
    let key = mutable_state_of(1);
    let shown = mutable_state_of("-".to_string());
    let failure = mutable_state_of(String::new());
    let worker = Arc::new(parking_lot::Mutex::new(None::<std::thread::ThreadId>));
    let from_worker = mutable_state_of(0);
    let summed = mutable_state_of(0);
    let slow_key = mutable_state_of(1);
    let slow_results = Rc::new(RefCell::new(Vec::<i32>::new()));
    let (k, sh, fa, w, fw, su, sk, sr) = (
        key.clone(),
        shown.clone(),
        failure.clone(),
        worker.clone(),
        from_worker.clone(),
        summed.clone(),
        slow_key.clone(),
        slow_results.clone(),
    );
    let rule = CoRule::new(kompozable_lambda!(move || {
        let kv = k.get();
        let (sh, fa) = (sh.clone(), fa.clone());
        LaunchedEffect(kv, async move {
            match described(kv).await {
                Ok(s) => sh.set(s),
                Err(e) => fa.set(e),
            }
        });
        let skv = sk.get();
        let sr = sr.clone();
        LaunchedEffect(skv, async move {
            // cancelled while its work runs: what it gives is dropped
            let r = with_background(move || {
                std::thread::sleep(Duration::from_millis(30));
                skv * 10
            })
            .await;
            sr.borrow_mut().push(r);
        });
        let (w, fw, su) = (w.clone(), fw.clone(), su.clone());
        LaunchedEffect(0, async move {
            let sum = with_background(move || {
                *w.lock() = Some(std::thread::current().id());
                (1..=100).sum::<i32>()
            })
            .await;
            fw.set(sum); // back on this thread
            su.set(sum);
            with_background(|| {}).await; // nothing to give back
            let failed = run_catching(with_background(|| panic!("empty (expected in this test)"))).await;
            if failed.is_err() {
                fw.set(-1);
            }
        });
    }));
    quietly(|| rule.wait_for_idle());
    assert!(shown.peek() == "twice 2" && failure.peek().is_empty());
    assert_eq!(summed.peek(), 5050);
    let w = worker.lock().unwrap();
    assert!(w != std::thread::current().id());
    assert_eq!(*slow_results.borrow(), [10]);
    slow_key.set(2);
    rule.frame(); // its work started
    slow_key.set(3);
    rule.wait_for_idle();
    assert_eq!(*slow_results.borrow(), [10, 30]);
    // A key change cancels the one waiting; the new one gives its own value; an error reaches the caller.
    key.set(5);
    rule.advance_time_by(48);
    key.set(-3);
    rule.wait_for_idle();
    assert!(shown.peek() == "twice 2" && failure.peek() == "negative");
    key.set(7);
    rule.wait_for_idle();
    assert_eq!(shown.peek(), "twice 14");
}

fn background_failures_reach_the_awaiting_coroutine() {
    let (rule, scope) = scope_rule();
    let seen = new_log();
    let s = seen.clone();
    scope.launch(async move {
        match run_catching(with_background(|| -> i32 { panic!("behind (expected in this test)") })).await {
            Ok(_) => push(&s, "ok"),
            Err(f) => push(&s, f.message()),
        }
    });
    quietly(|| rule.wait_for_idle());
    assert_eq!(*seen.borrow(), ["behind (expected in this test)"]);
}

fn launch_gives_a_job_and_async_a_deferred() {
    let (rule, scope) = scope_rule();
    let log = new_log();
    let l = log.clone();
    let slow = scope.launch(async move {
        delay_ms(300).await;
        push(&l, "slow");
    });
    let (l, s) = (log.clone(), slow.clone());
    scope.launch(async move {
        s.join().await; // waits for the other to end
        push(&l, "joined");
    });
    assert!(slow.is_active() && !slow.is_completed());
    rule.advance_time_by(400);
    assert!(*log.borrow() == ["slow", "joined"] && slow.is_completed() && !slow.is_cancelled());
    // Cancelled, a job is done too: who joins it goes on.
    let forever = scope.launch(async { delay(Duration::from_secs(3600)).await });
    let (l, f) = (log.clone(), forever.clone());
    scope.launch(async move {
        f.join().await;
        push(&l, "after cancel");
    });
    forever.cancel();
    rule.frame();
    assert!(forever.is_cancelled() && forever.is_completed() && log.borrow().last().unwrap() == "after cancel");
    // Two asyncs at once: both values after the longer one, not after both one after the other.
    let sum = Rc::new(Cell::new(0));
    let (sm, sc) = (sum.clone(), scope.clone());
    scope.launch(async move {
        let a = sc.r#async(async {
            delay_ms(200).await;
            2
        });
        let b = sc.r#async(async {
            delay_ms(300).await;
            3
        });
        sm.set(a.await.unwrap() + b.await.unwrap());
    });
    rule.advance_time_by(250);
    assert_eq!(sum.get(), 0);
    rule.advance_time_by(100);
    assert_eq!(sum.get(), 5);
    // What an async panics with panics where it's awaited, not reported.
    let caught = new_log();
    let (c, sc) = (caught.clone(), scope.clone());
    scope.launch(async move {
        let failing = sc.r#async(async {
            delay_ms(10).await;
            panic!("none (expected in this test)");
        });
        if let Err(f) = run_catching(failing.into_future()).await {
            push(&c, f.message());
        }
    });
    quietly(|| rule.advance_time_by(50));
    assert_eq!(*caught.borrow(), ["none (expected in this test)"]);
    // An async cancelled before it gave its value: Err(Cancelled).
    let result = Rc::new(RefCell::new(None));
    let (r, sc) = (result.clone(), scope.clone());
    let deferred = sc.r#async(async {
        delay_ms(100).await;
        1
    });
    let d = deferred.clone();
    scope.launch(async move {
        *r.borrow_mut() = Some(d.await);
    });
    deferred.job().cancel();
    rule.frame();
    assert_eq!(*result.borrow(), Some(Err(Cancelled)));
}

use std::future::IntoFuture;

fn coroutine_scopes_wait_for_their_children_and_fail_with_them() {
    let (rule, scope) = scope_rule();
    let log = new_log();
    // coroutine_scope: on once its children are done, with the block's value.
    let l = log.clone();
    scope.launch(async move {
        let l2 = l.clone();
        let n = coroutine_scope(move |s| async move {
            s.launch(async move {
                delay_ms(300).await;
                push(&l2, "child");
            });
            7
        })
        .await;
        push(&l, format!("scope {n}"));
    });
    rule.advance_time_by(200);
    assert!(log.borrow().is_empty());
    rule.advance_time_by(200);
    assert_eq!(*log.borrow(), ["child", "scope 7"]);
    // A child failing cancels its siblings, and the await fails with it.
    log.borrow_mut().clear();
    let l = log.clone();
    scope.launch(async move {
        let l2 = l.clone();
        let r = run_catching(coroutine_scope(move |s| async move {
            s.launch(async move {
                let _guard = Guard(l2, "sibling");
                delay(Duration::from_secs(3600)).await;
            });
            s.launch(async {
                delay_ms(100).await;
                panic!("broke (expected in this test)");
            });
        }))
        .await;
        if let Err(f) = r {
            push(&l, format!("caught {}", f.message()));
        }
    });
    quietly(|| rule.advance_time_by(200));
    assert_eq!(*log.borrow(), ["sibling destroyed", "caught broke (expected in this test)"]);
    // The awaiting coroutine cancelled: the scope's children too.
    log.borrow_mut().clear();
    let l = log.clone();
    let outer = scope.launch(async move {
        let l2 = l.clone();
        coroutine_scope(move |s| async move {
            s.launch(async move {
                let _guard = Guard(l2, "inner");
                delay(Duration::from_secs(3600)).await;
            });
        })
        .await;
        push(&l, "never");
    });
    rule.advance_time_by(100);
    outer.cancel();
    assert_eq!(*log.borrow(), ["inner destroyed"]);
    // supervisor_scope: a child's failure is its own.
    log.borrow_mut().clear();
    let l = log.clone();
    scope.launch(async move {
        let l2 = l.clone();
        supervisor_scope(move |s| async move {
            s.launch(async {
                delay_ms(50).await;
                panic!("one child (expected in this test)");
            });
            s.launch(async move {
                delay_ms(100).await;
                push(&l2, "other child");
            });
        })
        .await;
        push(&l, "supervisor done");
    });
    quietly(|| rule.advance_time_by(200));
    assert_eq!(*log.borrow(), ["other child", "supervisor done"]);
    assert!(!rule.dispatcher.has_timers());
}

fn with_timeout_cancels_what_runs_too_long() {
    let (rule, scope) = scope_rule();
    let log = new_log();
    let l = log.clone();
    scope.launch(async move {
        let l2 = l.clone();
        let r = with_timeout(Duration::from_millis(100), move |_| async move {
            let _guard = Guard(l2, "slow");
            delay(Duration::from_secs(1)).await;
        })
        .await;
        if r == Err(TimeoutCancellation) {
            push(&l, "timed out");
        }
        let quick = with_timeout_or_null(Duration::from_millis(100), |_| async {
            delay_ms(50).await;
            4
        })
        .await;
        let late = with_timeout_or_null(Duration::from_millis(100), |_| async {
            delay(Duration::from_secs(1)).await;
            5
        })
        .await;
        let completed =
            with_timeout_or_null(Duration::from_millis(100), |_| delay(Duration::from_secs(1))).await.is_some();
        push(&l, format!("{} {} {}", quick.unwrap_or(-1), late.unwrap_or(-1), if completed { "yes" } else { "no" }));
    });
    rule.advance_time_by(1000);
    assert_eq!(*log.borrow(), ["slow destroyed", "timed out", "4 -1 no"]);
    // A coroutine ending with a timeout is done, nothing left waiting.
    scope.launch(async {
        let _ = with_timeout(Duration::from_millis(10), |_| delay(Duration::from_secs(1))).await;
    });
    rule.advance_time_by(100);
    assert!(!rule.dispatcher.has_timers());
}

fn launched_effect_restarts_when_key_changes() {
    let k = mutable_state_of(1);
    let log = new_log();
    let (kk, l) = (k.clone(), log.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        let key = kk.get();
        let l = l.clone();
        LaunchedEffect(key, async move {
            let _guard = Guard(l.clone(), if key == 1 { "run1" } else { "run2" });
            push(&l, format!("start{key}"));
            delay(Duration::from_secs(10)).await;
        });
    }));
    k.set(2);
    rule.advance_time_by(100);
    assert_eq!(*log.borrow(), ["start1", "run1 destroyed", "start2"]);
}

async fn wait_and_add(value: MutableState<i32>, amount: i32) {
    delay_ms(100).await;
    value.set(value.peek() + amount);
}

fn coroutine_scope_launches_from_a_click() {
    let value = mutable_state_of(0);
    let scope_cell = Rc::new(RefCell::new(None));
    let (v, sc) = (value.clone(), scope_cell.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        *sc.borrow_mut() = Some(remember_coroutine_scope());
        Text(format!("value={v}"));
    }));
    // What a button's click does:
    let v = value.clone();
    scope_of(&scope_cell).launch(async move {
        delay_ms(500).await;
        v.set(1);
        wait_and_add(v.clone(), 10).await; // awaiting another coroutine
        v.set(v.peek() + 100);
    });
    rule.advance_time_by(300);
    assert!(rule.has("value=0"));
    rule.advance_time_by(250); // 550 ms: past the first delay, inside the child's
    assert!(rule.has("value=1"));
    rule.wait_for_idle();
    assert!(rule.has("value=111"));
}

fn coroutine_failure_is_reported_not_fatal() {
    let after = mutable_state_of(0);
    let a = after.clone();
    let rule = CoRule::new(kompozable_lambda!(move || {
        LaunchedEffect(0, async {
            delay_ms(10).await;
            panic!("boom (expected in this test)");
        });
        let a = a.clone();
        LaunchedEffect(1, async move {
            delay_ms(50).await;
            a.set(1);
        });
    }));
    quietly(|| rule.advance_time_by(100));
    assert_eq!(after.peek(), 1); // other coroutines keep running
}

fn frames_are_awaited_and_give_their_time() {
    let (rule, scope) = scope_rule();
    let times = Rc::new(RefCell::new(Vec::new()));
    let t = times.clone();
    scope.launch(async move {
        for _ in 0..3 {
            let ms = with_frame_millis(|ms| ms).await;
            t.borrow_mut().push(ms);
        }
    });
    rule.advance_time_by(48);
    assert_eq!(*times.borrow(), [16, 32, 48]);
    assert!(!rule.dispatcher.has_frame_waiters());
}

fn snapshot_flow_reports_each_new_value() {
    let a = mutable_state_of(1);
    let b = mutable_state_of(10);
    let running = mutable_state_of(true);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (aa, bb, r, s) = (a.clone(), b.clone(), running.clone(), seen.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        if !r.get() {
            return;
        }
        let (aa, bb, s) = (aa.clone(), bb.clone(), s.clone());
        LaunchedEffect(0, async move {
            snapshot_flow(move || aa.get() + bb.get() / 10).collect(move |v| s.borrow_mut().push(v)).await;
        });
    }));
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [2]); // the value it starts with
    a.set(2);
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [2, 3]);
    a.set(3);
    b.set(0); // 3 + 0: the same as before, so nothing new
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [2, 3]);
    b.set(20);
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [2, 3, 5]);
    running.set(false); // the effect leaves: no more values
    rule.wait_for_idle();
    a.set(100);
    rule.wait_for_idle();
    assert!(*seen.borrow() == [2, 3, 5] && !rule.has_pending_work());
}

/// A screen reading a ViewModel's StateFlow.
#[derive(Clone)]
struct DownloadViewModel {
    progress: MutableStateFlow<i32>,
    status: MutableStateFlow<String>,
}

thread_local! {
    static FLOW_READER_RUNS: Cell<u32> = const { Cell::new(0) };
}

#[kompozable]
fn FlowReader(vm: DownloadViewModel) {
    FLOW_READER_RUNS.with(|r| r.set(r.get() + 1));
    let progress = vm.progress.collect_as_state();
    Text(format!("{} · %{progress}", vm.status.collect_as_state()));
}

fn state_flow_is_collected_as_state() {
    FLOW_READER_RUNS.with(|r| r.set(0));
    let vm = DownloadViewModel { progress: MutableStateFlow::new(0), status: MutableStateFlow::new("waiting".into()) };
    let v = vm.clone();
    let rule = CoRule::new(kompozable_lambda!(move || FlowReader(v.clone())));
    let runs = || FLOW_READER_RUNS.with(|r| r.get());
    assert!(rule.has("waiting · %0"));
    vm.progress.set_value(40);
    rule.wait_for_idle();
    assert!(rule.has("waiting · %40") && runs() == 2, "{} {:?}", runs(), rule.names());
    vm.progress.set_value(40); // the value it has: nothing to do
    rule.wait_for_idle();
    assert!(runs() == 2 && !rule.dispatcher.has_ready());
    // From a worker thread; values it sets faster than frames are conflated to the latest.
    let v = vm.clone();
    let (progress, status) = (v.progress.clone(), v.status.clone());
    std::thread::spawn(move || {
        for i in 41..=100 {
            progress.set_value(i);
        }
        status.set_value("done".into());
    })
    .join()
    .unwrap();
    rule.wait_for_idle();
    assert!(rule.has("done · %100") && runs() == 3, "{} {:?}", runs(), rule.names());
    // update() is atomic: four threads adding a thousand each lose none.
    vm.progress.set_value(0);
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let p = vm.progress.clone();
            std::thread::spawn(move || {
                for _ in 0..1000 {
                    p.update(|n| n + 1);
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(vm.progress.value(), 4000);
    assert!(
        vm.progress.compare_and_set(&4000, 7) && !vm.progress.compare_and_set(&4000, 8) && vm.progress.value() == 7
    );
    rule.wait_for_idle();
    assert!(rule.has("done · %7"));
}

/// Counts the futures alive of a flow's producer (dropped with it).
struct ProducerAlive(Rc<Cell<i32>>);
impl ProducerAlive {
    fn new(c: &Rc<Cell<i32>>) -> ProducerAlive {
        c.set(c.get() + 1);
        ProducerAlive(c.clone())
    }
}
impl Drop for ProducerAlive {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

fn numbers(alive: &Rc<Cell<i32>>, start: i32) -> Flow<i32> {
    let alive = alive.clone();
    flow(move |emit| {
        let alive = alive.clone();
        async move {
            let _guard = ProducerAlive::new(&alive);
            let mut i = start;
            loop {
                emit.emit(i).await;
                delay_ms(100).await;
                i += 1;
            }
        }
    })
}

fn flows_produce_on_demand_and_operators_shape_them() {
    let alive = Rc::new(Cell::new(0));
    let numbers = numbers(&alive, 1);
    let seen = new_log();
    let (n, s) = (numbers.clone(), seen.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        let (n, s) = (n.clone(), s.clone());
        LaunchedEffect(0, async move {
            let s2 = s.clone();
            n.filter(|v| v % 2 == 1)
                .map(|v| (v * 10).to_string())
                .on_each(move |v| push(&s2, format!("before {v}")))
                .take(3)
                .collect({
                    let s = s.clone();
                    move |v| push(&s, v)
                })
                .await;
            push(&s, "done"); // take stopped the endless producer
        });
    }));
    assert!(alive.get() == 1 && *seen.borrow() == ["before 10", "10"]); // the first at once, the rest as produced
    rule.advance_time_by(1000);
    assert_eq!(*seen.borrow(), ["before 10", "10", "before 30", "30", "before 50", "50", "done"]);
    assert_eq!(alive.get(), 0); // its future is gone
    // Synchronous producers run through at once; transform makes any number of values of each.
    let out = Rc::new(RefCell::new(Vec::new()));
    let scope = CoroutineScope::new(&rule.dispatcher);
    let o = out.clone();
    flow_of([1, 2, 2, 3])
        .distinct_until_changed()
        .transform(|v, emit| async move {
            emit.emit(v).await;
            emit.emit(v * 10).await;
        })
        .on_each(move |v| o.borrow_mut().push(*v))
        .launch_in(&scope);
    let o = out.clone();
    empty_flow::<i32>().on_each(move |v| o.borrow_mut().push(*v)).launch_in(&scope);
    // (`launchIn` is a dispatched `launch`, as Kotlin's: nothing ran yet; they run when the dispatcher next runs what
    // is ready)
    assert!(out.borrow().is_empty() && scope.active_jobs() == 2);
    rule.dispatcher.run_ready();
    assert!(*out.borrow() == [1, 10, 2, 20, 3, 30] && scope.active_jobs() == 0);
    // A collector that suspends holds the producer back: it emits the next when this one is taken.
    let log = new_log();
    let l = log.clone();
    scope.launch(async move {
        let l2 = l.clone();
        flow(move |emit| {
            let l2 = l2.clone();
            async move {
                for i in 1..=3 {
                    push(&l2, format!("make {i}"));
                    emit.emit(i).await;
                }
            }
        })
        .collect_async(move |v| {
            let l = l.clone();
            async move {
                delay_ms(100).await;
                push(&l, format!("take {v}"));
            }
        })
        .await;
    });
    rule.advance_time_by(150);
    assert_eq!(*log.borrow(), ["make 1", "take 1", "make 2"]);
    rule.advance_time_by(300);
    assert_eq!(*log.borrow(), ["make 1", "take 1", "make 2", "take 2", "make 3", "take 3"]);
}

fn flow_collection_stops_with_its_coroutine() {
    let alive = Rc::new(Cell::new(0));
    let ticks = numbers(&alive, 0);
    let thirds = ticks.map(|i| i / 3).distinct_until_changed();
    // Operators that collect in coroutines of their own: those go too.
    let t2 = ticks.clone();
    let sums = combine(&ticks, &ticks.debounce(Duration::from_millis(50)), |a, b| a + b)
        .flat_map_latest(move |sum| t2.map(move |i| sum + i));
    let shown = mutable_state_of(true);
    let compositions = Rc::new(Cell::new(0));
    let totals = Rc::new(RefCell::new(Vec::new()));
    let (s, c, t) = (shown.clone(), compositions.clone(), totals.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        if !s.get() {
            return;
        }
        let (th, c) = (thirds.clone(), c.clone());
        scoped(move || {
            c.set(c.get() + 1);
            Text(format!("thirds {}", th.collect_as_state(-1)));
        });
        let (sums, t) = (sums.clone(), t.clone());
        LaunchedEffect(0, async move { sums.collect(move |v| t.borrow_mut().push(v)).await });
    }));
    assert!(rule.has("thirds 0") && alive.get() == 3, "{} {:?}", alive.get(), rule.names()); // the thirds', and combine's two
    rule.advance_time_by(100);
    assert!(!totals.borrow().is_empty() && alive.get() == 4, "{}", alive.get()); // and flat_map_latest's current one
    let before = compositions.get();
    rule.advance_time_by(150); // 1 and 2 are thirds 0: nothing new
    assert!(rule.has("thirds 0") && compositions.get() == before);
    rule.advance_time_by(100);
    assert!(rule.has("thirds 1"));
    shown.set(false); // its collectors leave with the composition, and the producers with them
    rule.frame();
    assert_eq!(alive.get(), 0);
}

/// A search as Compose apps write it: the query debounced, combined with a filter, each one searched (the
/// previous search cancelled), into a StateFlow for the screen.
struct SearchModel {
    query: MutableStateFlow<String>,
    exact: MutableStateFlow<bool>,
    searches: Rc<Cell<i32>>,
    cancelled: Rc<Cell<i32>>,
    results: StateFlow<String>,
    _scope: CoroutineScope,
}

fn search(q: String, exactly: bool, searches: Rc<Cell<i32>>, cancelled: Rc<Cell<i32>>) -> Flow<String> {
    flow(move |emit| {
        let (q, searches, cancelled) = (q.clone(), searches.clone(), cancelled.clone());
        async move {
            searches.set(searches.get() + 1);
            struct Watch(Rc<Cell<i32>>, bool);
            impl Drop for Watch {
                fn drop(&mut self) {
                    if !self.1 {
                        self.0.set(self.0.get() + 1);
                    }
                }
            }
            let mut watch = Watch(cancelled, false);
            emit.emit(format!("searching: {q}")).await;
            delay_ms(500).await;
            emit.emit(format!("result: {q}{}", if exactly { " (exact)" } else { "" })).await;
            watch.1 = true;
        }
    })
}

impl SearchModel {
    fn new(dispatcher: &Dispatcher) -> SearchModel {
        let scope = CoroutineScope::new(dispatcher);
        let query = MutableStateFlow::new(String::new());
        let exact = MutableStateFlow::new(false);
        let (searches, cancelled) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
        let (s, c) = (searches.clone(), cancelled.clone());
        let results = combine(&query.debounce(Duration::from_millis(300)), &exact.as_flow(), |q, e| (q.clone(), *e))
            .flat_map_latest(move |(q, e)| search(q, e, s.clone(), c.clone()))
            .state_in(&scope, SharingStarted::Eagerly, "-".into());
        SearchModel { query, exact, searches, cancelled, results, _scope: scope }
    }
}

fn flows_combine_debounce_and_follow_the_latest_into_a_state_flow() {
    let model: Rc<RefCell<Option<Rc<SearchModel>>>> = Rc::new(RefCell::new(None));
    let m = model.clone();
    let rule = CoRule::new(kompozable_lambda!(move || {
        let d = LocalDispatcher.current().unwrap();
        let vm = remember(|| Rc::new(SearchModel::new(&d)));
        *m.borrow_mut() = Some(vm.clone());
        Text(vm.results.collect_as_state());
    }));
    let model = model.borrow().clone().unwrap();
    assert!(rule.has("-")); // the initial value, until the debounced query is searched
    rule.advance_time_by(1000);
    assert!(rule.has("result: ") && model.searches.get() == 1, "{:?}", rule.names());
    model.query.set_value("k".into()); // typing: only the query it rests on is searched
    rule.advance_time_by(100);
    model.query.set_value("ko".into());
    rule.advance_time_by(100);
    model.query.set_value("kom".into());
    rule.advance_time_by(250);
    assert_eq!(model.searches.get(), 1); // not yet: 300 ms since the last key
    rule.advance_time_by(150);
    assert!(rule.has("searching: kom") && model.searches.get() == 2, "{:?}", rule.names());
    model.exact.set_value(true); // the filter changes mid-search: that search is cancelled
    rule.advance_time_by(100);
    assert!(model.searches.get() == 3 && model.cancelled.get() == 1 && rule.has("searching: kom"));
    rule.advance_time_by(600);
    assert!(rule.has("result: kom (exact)") && model.cancelled.get() == 1, "{:?}", rule.names());
}

fn shared_flows_hand_every_value_to_every_collector() {
    let events = MutableSharedFlow::<String>::new(1); // the last one replayed to a newcomer
    let (first, second) = (new_log(), new_log());
    let late = mutable_state_of(false);
    let gone = mutable_state_of(false);
    let (e, f, s, la, g) = (events.clone(), first.clone(), second.clone(), late.clone(), gone.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        if !g.get() {
            let (e, f) = (e.clone(), f.clone());
            LaunchedEffect(0, async move { e.collect(move |v| push(&f, v)).await });
        }
        if la.get() {
            let (e, s) = (e.clone(), s.clone());
            LaunchedEffect(0, async move { e.collect(move |v| push(&s, v)).await });
        }
    }));
    assert_eq!(events.subscription_count(), 1);
    events.try_emit("a".into());
    let e = events.clone();
    std::thread::spawn(move || {
        e.try_emit("b".into());
    })
    .join()
    .unwrap(); // from another thread
    rule.wait_for_idle();
    assert!(*first.borrow() == ["a", "b"] && second.borrow().is_empty());
    late.set(true); // gets "b" (replayed), then what comes
    rule.wait_for_idle();
    events.try_emit("c".into());
    rule.wait_for_idle();
    assert!(*second.borrow() == ["b", "c"] && *first.borrow() == ["a", "b", "c"]);
    assert!(events.subscription_count() == 2 && events.replay_cache() == ["c"]);
    gone.set(true); // that collector leaves: it gets nothing more
    rule.wait_for_idle();
    events.try_emit("d".into());
    rule.wait_for_idle();
    assert!(first.borrow().len() == 3 && second.borrow().last().unwrap() == "d" && events.subscription_count() == 1);
}

fn hot_flows_take_flows_operators_on_their_values() {
    let events = MutableSharedFlow::<i32>::new(0);
    let count = MutableStateFlow::new(0);
    let (evens, seen, first_two) =
        (Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())));
    let (e, c, ev, se, ft) = (events.clone(), count.clone(), evens.clone(), seen.clone(), first_two.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        let (e1, ev) = (e.clone(), ev.clone());
        LaunchedEffect(0, async move { e1.filter(|v| v % 2 == 0).collect(move |v| ev.borrow_mut().push(v)).await });
        let (e2, se) = (e.clone(), se.clone());
        LaunchedEffect(1, async move { e2.on_each(move |v| se.borrow_mut().push(*v)).collect(|_| {}).await });
        let (c, ft) = (c.clone(), ft.clone());
        LaunchedEffect(2, async move { c.take(2).collect(move |v| ft.borrow_mut().push(v)).await });
    }));
    for i in 1..=4 {
        events.try_emit(i);
        count.set_value(i);
        rule.wait_for_idle();
    }
    assert_eq!(*evens.borrow(), [2, 4]);
    assert_eq!(*seen.borrow(), [1, 2, 3, 4]);
    assert_eq!(*first_two.borrow(), [0, 1]);
}

fn rendezvous_channels_hand_each_element_from_sender_to_receiver() {
    let channel = Channel::<i32>::rendezvous(); // no buffer
    let log = new_log();
    let receiving = mutable_state_of(false);
    let (ch, l, r) = (channel.clone(), log.clone(), receiving.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        let (ch1, l1) = (ch.clone(), l.clone());
        LaunchedEffect(0, async move {
            for i in 1..=3 {
                let sent = ch1.send(i).await;
                push(&l1, format!("sent {i}{}", if sent { "" } else { " (closed)" }));
            }
            ch1.close();
        });
        if r.get() {
            let (ch2, l2) = (ch.clone(), l.clone());
            LaunchedEffect(1, async move {
                while let Some(v) = ch2.receive().await {
                    push(&l2, format!("received {v}"));
                }
                push(&l2, "done");
            });
        }
    }));
    // Nobody receives yet: the first send waits.
    assert!(log.borrow().is_empty() && !channel.is_empty() && !channel.try_send(9));
    receiving.set(true);
    rule.wait_for_idle();
    // In order, each send done only once its element was taken: never more than one ahead of what the receiver
    // got (handed straight to a waiting receiver, a send needn't wait for it to run).
    let (mut sent, mut received) = (Vec::new(), Vec::new());
    for l in log.borrow().iter() {
        if l.starts_with("sent") {
            sent.push(l.clone());
            assert!(received.len() + 1 >= sent.len());
        }
        if l.starts_with("received") {
            received.push(l.clone());
        }
    }
    assert_eq!(received, ["received 1", "received 2", "received 3"]);
    assert_eq!(sent, ["sent 1", "sent 2", "sent 3"]);
    assert!(log.borrow().len() == 7 && log.borrow().last().unwrap() == "done");
    assert!(channel.is_closed_for_receive() && !channel.try_send(4));
}

fn buffered_channels_wait_only_when_full() {
    let channel = Channel::<i32>::new(2, BufferOverflow::Suspend);
    let sent = Rc::new(Cell::new(0));
    let (ch, s) = (channel.clone(), sent.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        let (ch, s) = (ch.clone(), s.clone());
        LaunchedEffect(0, async move {
            for i in 1..=4 {
                ch.send(i).await;
                s.set(s.get() + 1);
            }
        });
    }));
    assert_eq!(sent.get(), 2); // two fit; the third waits for room
    assert_eq!(channel.try_receive(), Some(1));
    rule.wait_for_idle();
    assert_eq!(sent.get(), 3); // it moved into the room
    assert!(channel.try_receive() == Some(2) && channel.try_receive() == Some(3));
    rule.wait_for_idle();
    assert!(
        sent.get() == 4 && channel.try_receive() == Some(4) && channel.try_receive().is_none() && channel.is_empty()
    );
}

fn conflated_and_dropping_channels_keep_what_their_overflow_says() {
    let conflated = Channel::<i32>::conflated();
    let oldest = Channel::<i32>::new(2, BufferOverflow::DropOldest);
    let latest = Channel::<i32>::new(2, BufferOverflow::DropLatest);
    for i in 1..=4 {
        assert!(conflated.try_send(i) && oldest.try_send(i) && latest.try_send(i));
    }
    assert!(conflated.try_receive() == Some(4) && conflated.try_receive().is_none());
    assert!(oldest.try_receive() == Some(3) && oldest.try_receive() == Some(4));
    assert!(latest.try_receive() == Some(1) && latest.try_receive() == Some(2) && latest.try_receive().is_none());
    let full = Channel::<i32>::new(1, BufferOverflow::Suspend);
    assert!(full.try_send(1) && !full.try_send(2)); // Suspend: try_send fails when full
}

fn closed_channels_give_what_is_left_and_cancelled_ones_drop_it() {
    let channel = Channel::<String>::unlimited();
    channel.try_send("a".into());
    channel.try_send("b".into());
    assert!(channel.close() && !channel.close() && channel.is_closed_for_send() && !channel.is_closed_for_receive());
    assert!(!channel.try_send("c".into()));
    assert!(channel.try_receive().as_deref() == Some("a") && channel.try_receive().as_deref() == Some("b"));
    assert!(channel.is_closed_for_receive());
    // Cancelled: buffered elements and a waiting sender's go; the sender hears false.
    let cancelled = Channel::<i32>::new(1, BufferOverflow::Suspend);
    let result = Rc::new(Cell::new(None));
    let (ch, r) = (cancelled.clone(), result.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        let (ch, r) = (ch.clone(), r.clone());
        LaunchedEffect(0, async move {
            ch.send(1).await;
            r.set(Some(ch.send(2).await)); // waits: full
        });
    }));
    assert!(result.get().is_none());
    cancelled.cancel();
    rule.wait_for_idle();
    assert!(result.get() == Some(false) && cancelled.try_receive().is_none() && cancelled.is_closed_for_receive());
}

fn channel_receivers_share_its_elements_and_a_cancelled_one_takes_nothing() {
    let channel = Channel::<i32>::unlimited();
    let (first, second) = (Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())));
    let both = mutable_state_of(true);
    let waiter = mutable_state_of(true);
    let lost = Rc::new(Cell::new(None));
    let (ch, f, s, b, w, lo) =
        (channel.clone(), first.clone(), second.clone(), both.clone(), waiter.clone(), lost.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        if b.get() {
            let (c1, f) = (ch.clone(), f.clone());
            LaunchedEffect(1, async move { c1.receive_as_flow().collect(move |v| f.borrow_mut().push(v)).await });
            let (c2, s) = (ch.clone(), s.clone());
            LaunchedEffect(2, async move { c2.receive_as_flow().collect(move |v| s.borrow_mut().push(v)).await });
        }
        if w.get() {
            let (c3, lo) = (ch.clone(), lo.clone());
            LaunchedEffect(3, async move { lo.set(c3.receive().await) });
        }
    }));
    for i in 1..=6 {
        channel.try_send(i);
    }
    rule.wait_for_idle();
    // Each element went to one of them, none twice.
    let mut all: Vec<i32> = first.borrow().iter().chain(second.borrow().iter()).copied().collect();
    all.extend(lost.get());
    all.sort();
    assert!(all == [1, 2, 3, 4, 5, 6] && !first.borrow().is_empty() && !second.borrow().is_empty(), "{all:?}");
    // Gone while waiting: the next element waits in the channel for whoever comes.
    both.set(false);
    waiter.set(false);
    rule.wait_for_idle();
    channel.try_send(7);
    rule.wait_for_idle();
    assert_eq!(channel.try_receive(), Some(7));
}

fn produce_sends_from_a_coroutine_and_consume_each_takes_it_all() {
    let got = Rc::new(RefCell::new(Vec::new()));
    let numbers: Rc<RefCell<Option<Channel<i32>>>> = Rc::new(RefCell::new(None));
    let (g, n) = (got.clone(), numbers.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        let scope = remember_coroutine_scope();
        let (g, n) = (g.clone(), n.clone());
        LaunchedEffect(0, async move {
            let ch = produce(&scope, Channel::<i32>::RENDEZVOUS, |out| async move {
                for i in 1..=5 {
                    out.send(i * i).await;
                }
            });
            *n.borrow_mut() = Some(ch.clone());
            ch.consume_each(move |v| g.borrow_mut().push(v)).await;
        });
    }));
    rule.wait_for_idle();
    assert_eq!(*got.borrow(), [1, 4, 9, 16, 25]);
    assert!(numbers.borrow().as_ref().unwrap().is_closed_for_receive());
    // From another thread, received on the UI thread.
    let events = Channel::<i32>::buffered();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let where_ = Rc::new(Cell::new(None));
    let (e, s, w) = (events.clone(), seen.clone(), where_.clone());
    let other = CoRule::new(kompozable_lambda!(move || {
        let (e, s, w) = (e.clone(), s.clone(), w.clone());
        LaunchedEffect(0, async move {
            e.consume_each(move |v| {
                w.set(Some(std::thread::current().id()));
                s.borrow_mut().push(v);
            })
            .await;
        });
    }));
    let e = events.clone();
    std::thread::spawn(move || {
        e.try_send(42);
    })
    .join()
    .unwrap();
    other.wait_for_idle();
    assert!(*seen.borrow() == [42] && where_.get() == Some(std::thread::current().id()));
}

fn closed_channels_answer_at_once_and_consumers_and_producers_end_them() {
    // Closed: a send says no and a receive gives nothing, neither waiting.
    let closed = Channel::<i32>::rendezvous();
    closed.close();
    let sent = Rc::new(Cell::new(None));
    let received = Rc::new(Cell::new(None));
    // A producer done before anyone reads: closed, what it sent still there.
    let early: Rc<RefCell<Option<Channel<i32>>>> = Rc::new(RefCell::new(None));
    // A consumer that stops: its channel cancelled.
    let consumed = Channel::<i32>::unlimited();
    let consuming = mutable_state_of(true);
    let got = Rc::new(RefCell::new(Vec::new()));
    let (cl, se, re, ea, co, cg, g) = (
        closed.clone(),
        sent.clone(),
        received.clone(),
        early.clone(),
        consumed.clone(),
        consuming.clone(),
        got.clone(),
    );
    let rule = CoRule::new(kompozable_lambda!(move || {
        let scope = remember_coroutine_scope();
        let (cl, se, re, ea) = (cl.clone(), se.clone(), re.clone(), ea.clone());
        LaunchedEffect(0, async move {
            se.set(Some(cl.send(1).await));
            re.set(Some(cl.receive().await));
            *ea.borrow_mut() = Some(produce(&scope, Channel::<i32>::UNLIMITED, |out| async move {
                for i in 1..=3 {
                    out.send(i).await;
                }
            }));
        });
        if cg.get() {
            let (co, g) = (co.clone(), g.clone());
            LaunchedEffect(1, async move { co.consume_each(move |v| g.borrow_mut().push(v)).await });
        }
    }));
    assert!(sent.get() == Some(false) && received.get() == Some(None));
    let e = early.borrow().clone().unwrap();
    assert!(
        e.is_closed_for_send()
            && e.try_receive() == Some(1)
            && e.try_receive() == Some(2)
            && e.try_receive() == Some(3)
    );
    // A producer cancelled while it waits: its channel cancelled.
    let producers = CoroutineScope::new(&rule.dispatcher);
    let stuck = produce(&producers, Channel::<i32>::RENDEZVOUS, |out| async move {
        out.send(1).await; // nobody receives
    });
    rule.wait_for_idle();
    assert!(!stuck.is_closed_for_send());
    producers.cancel();
    assert!(stuck.is_closed_for_send() && stuck.is_empty());
    consumed.try_send(5);
    rule.wait_for_idle();
    assert!(*got.borrow() == [5] && !consumed.is_closed_for_send());
    consuming.set(false); // stops consuming
    rule.wait_for_idle();
    assert!(consumed.is_closed_for_send() && !consumed.try_send(6));
}

fn sharing_started_while_subscribed_collects_only_while_collected() {
    let alive = Rc::new(Cell::new(0));
    let starts = Rc::new(Cell::new(0));
    let (a, s) = (alive.clone(), starts.clone());
    let ticks = flow(move |emit| {
        let (a, s) = (a.clone(), s.clone());
        async move {
            let _guard = ProducerAlive::new(&a);
            s.set(s.get() + 1);
            let mut i = 1;
            loop {
                emit.emit(i).await;
                delay_ms(100).await;
                i += 1;
            }
        }
    });
    let shown = mutable_state_of(false);
    // A ViewModel's scope, say: the sharing lives as long as it.
    let shared: Rc<RefCell<Option<(StateFlow<i32>, SharedFlow<i32>, CoroutineScope)>>> = Rc::new(RefCell::new(None));
    let (t, sh, sd) = (ticks.clone(), shown.clone(), shared.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        if sd.borrow().is_none() {
            let scope = CoroutineScope::new(&LocalDispatcher.current().unwrap());
            let state = t.state_in(&scope, SharingStarted::WhileSubscribed(Duration::from_millis(1000)), 0);
            let lazy = t.share_in(&scope, SharingStarted::Lazily, 1);
            *sd.borrow_mut() = Some((state, lazy, scope));
        }
        if sh.get() {
            let state = sd.borrow().as_ref().unwrap().0.clone();
            Text(format!("value {}", state.collect_as_state()));
        }
    }));
    let (state, lazy, _scope) = shared.borrow().clone().unwrap();
    rule.advance_time_by(300);
    assert!(alive.get() == 0 && starts.get() == 0 && state.value() == 0); // nobody collects it: not started
    shown.set(true);
    rule.advance_time_by(250);
    let v = state.value(); // the screen shows it from the next frame
    assert!(alive.get() == 1 && starts.get() == 1 && v >= 2, "{} {} {v}", alive.get(), starts.get());
    assert!(rule.has(&format!("value {v}")) || rule.has(&format!("value {}", v - 1)), "{:?}", rule.names());
    shown.set(false); // gone, back within the timeout: still the same collection
    rule.advance_time_by(500);
    assert!(alive.get() == 1 && state.subscription_count() == 0);
    shown.set(true);
    rule.advance_time_by(200);
    assert!(alive.get() == 1 && starts.get() == 1);
    shown.set(false);
    rule.advance_time_by(1200); // past the timeout: stopped, the value kept
    let kept = state.value();
    assert!(alive.get() == 0 && kept >= 9, "{} {kept}", alive.get());
    rule.advance_time_by(300);
    assert_eq!(state.value(), kept);
    shown.set(true); // collected again: started again
    rule.advance_time_by(150);
    assert!(alive.get() == 1 && starts.get() == 2);
    // Lazily: started by its first collector, never stopped.
    let seen = Rc::new(RefCell::new(Vec::new()));
    let collector = CoroutineScope::new(&rule.dispatcher);
    let (lz, se) = (lazy.clone(), seen.clone());
    collector.launch(async move { lz.collect(move |v| se.borrow_mut().push(v)).await });
    rule.advance_time_by(150);
    assert!(alive.get() == 2 && starts.get() == 3 && !seen.borrow().is_empty());
    collector.cancel();
    rule.advance_time_by(2000);
    assert!(alive.get() == 2 && lazy.replay_cache().len() == 1);
}

fn state_flow_values_arrive_after_input_stamped_ahead_of_frames() {
    // An input event stamped later than the next frame's time (another clock: a script's virtual one) mustn't
    // leave the collector waiting for the frames to catch up.
    let vm = DownloadViewModel { progress: MutableStateFlow::new(0), status: MutableStateFlow::new("waiting".into()) };
    let v = vm.clone();
    let rule = CoRule::new(kompozable_lambda!(move || FlowReader(v.clone())));
    rule.dispatcher.advance_to(rule.time.get() + 60_000 * MS); // a minute ahead
    vm.progress.set_value(30);
    rule.wait_for_idle();
    assert!(rule.has("waiting · %30"));
    vm.progress.set_value(31);
    rule.wait_for_idle();
    assert!(rule.has("waiting · %31"));
}

fn state_flow_collect_follows_the_latest_value_until_cancelled() {
    let (a, b) = (MutableStateFlow::new(1), MutableStateFlow::new(100));
    let source = mutable_state_of(0);
    let running = mutable_state_of(true);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let shown = Rc::new(Cell::new(-1));
    let (aa, bb, so, ru, se, sh) = (a.clone(), b.clone(), source.clone(), running.clone(), seen.clone(), shown.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        if !ru.get() {
            return;
        }
        let flow = if so.get() == 0 { aa.as_state_flow() } else { bb.as_state_flow() };
        sh.set(flow.collect_as_state()); // another flow: the new one's value at once
        let (aa, se) = (aa.clone(), se.clone());
        LaunchedEffect(0, async move { aa.collect(move |v| se.borrow_mut().push(v)).await });
    }));
    rule.wait_for_idle();
    assert!(*seen.borrow() == [1] && shown.get() == 1);
    a.set_value(2);
    a.set_value(3); // before the collector runs: it sees 3 only
    rule.wait_for_idle();
    assert!(*seen.borrow() == [1, 3] && shown.get() == 3);
    a.set_value(4);
    a.set_value(3); // and back: nothing new
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [1, 3]);
    source.set(1);
    rule.wait_for_idle();
    assert_eq!(shown.get(), 100);
    a.set_value(50); // no longer shown
    b.set_value(101);
    rule.wait_for_idle();
    assert!(shown.get() == 101 && *seen.borrow() == [1, 3, 50]);
    running.set(false); // the collectors leave with the composition
    rule.wait_for_idle();
    a.set_value(60);
    b.set_value(102);
    rule.wait_for_idle();
    assert!(*seen.borrow() == [1, 3, 50] && !rule.has_pending_work());
}

fn remember_updated_state_gives_effects_the_latest_value() {
    let label = mutable_state_of("first".to_string());
    let seen = Rc::new(RefCell::new(String::new()));
    let (l, s) = (label.clone(), seen.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        let latest = remember_updated_state!(l.get());
        let s = s.clone();
        LaunchedEffect(0, async move {
            // key 0: never restarts
            delay(Duration::from_secs(1)).await;
            *s.borrow_mut() = latest.get();
        });
    }));
    label.set("last".into());
    rule.frame();
    rule.advance_time_by(1100);
    assert_eq!(*seen.borrow(), "last");
}

fn produce_state_from_a_coroutine() {
    let rule = CoRule::new(kompozable_lambda!(|| {
        let value = produce_state(0, 0, |state| async move {
            for i in 1..=3 {
                delay_ms(100).await;
                state.set(i);
            }
        });
        Text(format!("produced={value}"));
    }));
    assert!(rule.has("produced=0"));
    rule.advance_time_by(250);
    assert!(rule.has("produced=2"));
    rule.wait_for_idle();
    assert!(rule.has("produced=3"));
}

fn launched_effects_and_produce_state_take_several_keys() {
    let (a, b, c) = (mutable_state_of(1), mutable_state_of(1), mutable_state_of(1));
    let log = new_log();
    let produced = Rc::new(Cell::new(0));
    let (aa, bb, cc, l, p) = (a.clone(), b.clone(), c.clone(), log.clone(), produced.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        let (x, y, z) = (aa.get(), bb.get(), cc.get());
        let l = l.clone();
        LaunchedEffect((y, z), async move { push(&l, "launched") });
        let p = p.clone();
        let v = produce_state(0, (x, z), |state| async move {
            p.set(p.get() + 1);
            state.set(p.get());
        });
        Text(format!("p={v}"));
    }));
    rule.wait_for_idle();
    assert!(*log.borrow() == ["launched"] && rule.has("p=1"));
    log.borrow_mut().clear();
    c.set(2); // in both keys
    rule.wait_for_idle();
    assert!(*log.borrow() == ["launched"] && rule.has("p=2"));
    log.borrow_mut().clear();
    b.set(2); // only the LaunchedEffect's
    rule.wait_for_idle();
    assert!(*log.borrow() == ["launched"] && produced.get() == 2);
}

/// A value handed to whoever waits for it, waking them (what a pointer event is to a pointer input coroutine).
#[derive(Clone, Default)]
struct Signal(Rc<RefCell<(Option<i32>, Option<std::task::Waker>)>>);

impl Signal {
    fn fire(&self, v: i32) {
        let waker = {
            let mut s = self.0.borrow_mut();
            s.0 = Some(v);
            s.1.take()
        };
        if let Some(w) = waker {
            w.wake();
        }
    }
    async fn wait(&self) -> i32 {
        std::future::poll_fn(|cx| {
            let mut s = self.0.borrow_mut();
            match s.0.take() {
                Some(v) => std::task::Poll::Ready(v),
                None => {
                    s.1 = Some(cx.waker().clone());
                    std::task::Poll::Pending
                }
            }
        })
        .await
    }
}

fn run_woken_by_resumes_what_it_wakes_before_returning() {
    let d = Dispatcher::new();
    let scope = CoroutineScope::new(&d);
    let log = new_log();
    let (a, b, c) = (Signal::default(), Signal::default(), Signal::default());
    for (name, signal) in [("a", a.clone()), ("b", b.clone())] {
        let (log, c) = (log.clone(), c.clone());
        scope.launch(async move {
            loop {
                let v = signal.wait().await;
                push(&log, format!("{name} {v}"));
                if name == "b" {
                    c.fire(v * 10); // wakes the third one in turn
                }
            }
        });
    }
    let (l, third) = (log.clone(), c.clone());
    scope.launch(async move {
        loop {
            let v = third.wait().await;
            push(&l, format!("c {v}"));
        }
    });
    // (launched dispatched: they wait for their signals once the dispatcher has run them)
    d.run_ready();
    // Woken before: it waits for the dispatcher's next run; what the call wakes (and what that wakes) runs in it.
    a.fire(1);
    let r = d.run_woken_by(|| {
        b.fire(2);
        "done"
    });
    assert_eq!(r, "done");
    assert!(*log.borrow() == ["b 2", "c 20"], "{:?}", log.borrow());
    assert!(d.has_ready());
    d.run_ready();
    assert!(*log.borrow() == ["b 2", "c 20", "a 1"]);
    // A plain wake waits for the run.
    b.fire(3);
    assert_eq!(log.borrow().len(), 3);
    d.run_ready();
    assert_eq!(log.borrow().len(), 5);
    scope.cancel();
}

fn run_woken_by_from_inside_a_coroutine_keeps_its_own_wake() {
    // A coroutine that wakes itself and, while being polled, has something resumed in place: its own wake isn't
    // lost (it can't be polled while it runs), and it goes on at the next run.
    let d = Dispatcher::new();
    let scope = CoroutineScope::new(&d);
    let log = new_log();
    let other = Signal::default();
    let (l, o) = (log.clone(), other.clone());
    scope.launch(async move {
        loop {
            let v = o.wait().await;
            push(&l, format!("other {v}"));
        }
    });
    // (dispatched: waiting for its signal once run)
    d.run_ready();
    let (l, o, dd) = (log.clone(), other.clone(), d.clone());
    // (started at once, in the test: its first step resumes the other in place)
    named_args!(scope.launch(start = CoroutineStart::UNDISPATCHED, async move {
        let mut yielded = false;
        std::future::poll_fn(|cx| {
            if yielded {
                return std::task::Poll::Ready(());
            }
            yielded = true;
            cx.waker().wake_by_ref();
            dd.run_woken_by(|| o.fire(7));
            std::task::Poll::Pending
        })
        .await;
        push(&l, "self resumed");
    }));
    assert!(*log.borrow() == ["other 7"]);
    d.run_ready();
    assert!(*log.borrow() == ["other 7", "self resumed"]);
    scope.cancel();
}

/// An element of a coroutine's context (any `Clone + 'static` value, found by its type).
#[derive(Clone, PartialEq, Debug)]
struct Tag(&'static str);

impl From<Tag> for CoroutineContext {
    fn from(tag: Tag) -> CoroutineContext {
        CoroutineContext::of(tag)
    }
}

fn coroutine_contexts_reach_the_block_and_what_it_launches_in_its_scopes() {
    let (rule, scope) = scope_rule();
    let log = new_log();
    let tag = || current_coroutine_context().get::<Tag>().map(|t| t.0).unwrap_or("none");
    let (l, outer) = (log.clone(), scope.clone());
    scope.launch(async move {
        l.borrow_mut().push(format!("before: {}", tag()));
        with_context(Tag("a"), async {
            l.borrow_mut().push(format!("in: {}", tag()));
            delay_ms(10).await;
            l.borrow_mut().push(format!("after a delay: {}", tag()));
            with_context(Tag("b"), async { l.borrow_mut().push(format!("inner: {}", tag())) }).await;
            let l2 = l.clone();
            coroutine_scope(|inner| async move {
                // What a scope made inside launches sees it; what the outer scope launches doesn't.
                let l3 = l2.clone();
                inner.launch(async move { l3.borrow_mut().push(format!("child: {}", tag())) });
                let l4 = l2.clone();
                outer.launch(async move { l4.borrow_mut().push(format!("outer's: {}", tag())) });
            })
            .await;
        })
        .await;
        l.borrow_mut().push(format!("out: {}", tag()));
    });
    rule.advance_time_by(20);
    assert_eq!(
        *log.borrow(),
        ["before: none", "in: a", "after a delay: a", "inner: b", "child: a", "outer's: none", "out: none"]
    );
    // A scope's own context; elements by type, the newer one replacing the older.
    let own = CoroutineScope::new_with_context(&rule.dispatcher, CoroutineContext::of(Tag("own")));
    let seen = Rc::new(RefCell::new(None));
    let s = seen.clone();
    own.launch(async move { *s.borrow_mut() = current_coroutine_context().get::<Tag>() });
    rule.dispatcher.run_ready(); // (dispatched)
    assert_eq!(*seen.borrow(), Some(Tag("own")));
    let both = CoroutineContext::of(Tag("x")).plus_element(7i32) + CoroutineContext::of(Tag("y"));
    assert_eq!((both.get::<Tag>(), both.get::<i32>()), (Some(Tag("y")), Some(7)));
    assert_eq!(both.minus_key::<Tag>().get::<Tag>(), None);
    assert!(CoroutineContext::new().is_empty() && current_coroutine_context().is_empty());
    // (equal: a context and its clones; any two empty ones, as Kotlin's one EmptyCoroutineContext — an effect keyed
    // by `CoroutineContext::new()` isn't restarted at each composition)
    assert!(both == both.clone() && CoroutineContext::new() == CoroutineContext::new());
    assert!(both != CoroutineContext::of(Tag("y")) && CoroutineContext::new() != both);
}

fn a_cancelled_coroutine_scope_cancels_who_awaits_it() {
    let (rule, scope) = scope_rule();
    let log = new_log();
    // The block cancels its scope (Kotlin's `coroutineScope { cancel() }`): the CancellationException goes on out,
    // what follows the scope doesn't run, the coroutine is cancelled.
    let l = log.clone();
    let job = scope.launch(async move {
        coroutine_scope(|s| async move {
            s.cancel();
            delay_ms(10).await;
        })
        .await;
        l.borrow_mut().push("after the scope".to_string());
    });
    rule.advance_time_by(50);
    assert!(log.borrow().is_empty() && job.is_cancelled() && job.is_completed());
    // cancel_current(): Kotlin's `throw CancellationException()` where nothing catches it.
    let l = log.clone();
    let job = scope.launch(async move {
        l.borrow_mut().push("before".to_string());
        cancel_current::<()>().await;
        l.borrow_mut().push("after".to_string());
    });
    rule.dispatcher.run_ready(); // (dispatched)
    assert_eq!(*log.borrow(), ["before"]);
    assert!(job.is_cancelled());
    // Through nested scopes and a timeout, out to the launched coroutine.
    let l = log.clone();
    let job = scope.launch(async move {
        let _ = with_timeout(Duration::from_secs(1), |_| async {
            coroutine_scope(|_| async { cancel_current::<()>().await }).await;
        })
        .await;
        l.borrow_mut().push("after the timeout".to_string());
    });
    rule.advance_time_by(50);
    assert_eq!(*log.borrow(), ["before"]);
    assert!(job.is_cancelled());
    // An async's: awaiting it gives Err(Cancelled).
    let got = Rc::new(RefCell::new(None));
    let (g, s) = (got.clone(), scope.clone());
    scope.launch(async move {
        let d = s.r#async(async { coroutine_scope(|_| async { cancel_current::<i32>().await }).await });
        *g.borrow_mut() = Some(d.await);
    });
    rule.advance_time_by(20);
    assert_eq!(*got.borrow(), Some(Err(Cancelled)));
}

fn snapshot_flow_follows_a_derived_state_made_in_the_composition() {
    // A derived state made in the composition (its inputs observed there) tells a snapshot flow reading it when
    // its value changes, as Compose's derived states report their dependencies to the readers outside.
    let count = mutable_state_of(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (c, s) = (count.clone(), seen.clone());
    let rule = CoRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        let big = remember(|| derived_state_of(move || c.get() > 3));
        let s = s.clone();
        LaunchedEffect(0, async move {
            snapshot_flow(move || big.get()).collect(move |v| s.borrow_mut().push(v)).await;
        });
    }));
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [false]);
    count.set(2); // still not big: nothing new
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [false]);
    count.set(5);
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [false, true]);
    count.set(1);
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [false, true, false]);
}

fn catch_cancellation_gives_the_cancellation_to_the_block_that_catches_it() {
    // Kotlin's `try { … } catch (e: CancellationException) { … }` around a block whose own scope is cancelled:
    // the catcher goes on; its value when it isn't.
    let (rule, scope) = scope_rule();
    let log = new_log();
    let l = log.clone();
    scope.launch(async move {
        let caught = catch_cancellation(|inner: CoroutineScope| async move {
            inner.cancel();
            delay_ms(100).await; // a suspension: where the cancellation is thrown
            1
        })
        .await;
        push(&l, format!("caught {}", caught.is_err()));
        let done = catch_cancellation(|_| async { 2 }).await;
        push(&l, format!("done {:?}", done.ok()));
    });
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["caught true", "done Some(2)"]);
}

fn await_frame_end_resumes_after_the_frames_work() {
    // A coroutine resumed by a frame runs before the frame's composition and layout; awaiting the frame's end, it
    // goes on after them (where Compose's frame continuations run), at the frame's time.
    let dispatcher = Dispatcher::new();
    let log = new_log();
    let l = log.clone();
    CoroutineScope::new(&dispatcher).launch(async move {
        let at = await_frame().await;
        push(&l, format!("frame {at}"));
        let end = await_frame_end().await;
        push(&l, format!("end {end}"));
    });
    dispatcher.run(16 * MS);
    push(&log, "laid out");
    assert!(dispatcher.has_frame_waiters());
    dispatcher.run_frame_end();
    assert_eq!(*log.borrow(), [format!("frame {}", 16 * MS), "laid out".to_string(), format!("end {}", 16 * MS)]);
    assert!(!dispatcher.has_frame_waiters());
}

/// What the log holds, emptied.
fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

// ---- Kotlin's CoroutineStart, a scope's completion, a frame's two halves -------------------------------------------

fn launch_starts_as_kotlins_coroutine_start_says() {
    let d = Dispatcher::new();
    let scope = CoroutineScope::new(&d);
    let log = new_log();
    // DEFAULT: dispatched — queued, it runs when the dispatcher next runs what is ready, in launch order.
    let l = log.clone();
    let first = scope.launch(async move { push(&l, "default") });
    // UNDISPATCHED: at once, up to its first suspension, then dispatched as usual.
    let (l, signal) = (log.clone(), Signal::default());
    let s = signal.clone();
    named_args!(scope.launch(start = CoroutineStart::UNDISPATCHED, async move {
        push(&l, "undispatched");
        let v = s.wait().await;
        push(&l, format!("resumed {v}"));
    }));
    // LAZY: nothing until started; not active meanwhile.
    let l = log.clone();
    let lazy = named_args!(scope.launch(start = CoroutineStart::LAZY, async move { push(&l, "lazy") }));
    assert_eq!(*log.borrow(), ["undispatched"]);
    assert!(first.is_active() && !lazy.is_active() && !lazy.is_completed() && !lazy.is_cancelled());
    signal.fire(1);
    assert!(lazy.start() && !lazy.start()); // (true only for the call that started it)
    d.run_ready();
    assert_eq!(taken(&log), ["undispatched", "default", "resumed 1", "lazy"]);
    // A DEFAULT one cancelled before it ran never runs; an ATOMIC one runs, cancelled, up to its first suspension.
    let l = log.clone();
    let never = scope.launch(async move { push(&l, "never") });
    let l = log.clone();
    let atomic = named_args!(scope.launch(start = CoroutineStart::ATOMIC, async move {
        push(&l, "atomic");
        delay_ms(10).await;
        push(&l, "after its suspension");
    }));
    never.cancel();
    atomic.cancel();
    assert!(never.is_completed() && !atomic.is_completed());
    d.run_ready();
    assert_eq!(taken(&log), ["atomic"]);
    assert!(atomic.is_cancelled() && atomic.is_completed());
    // A LAZY one cancelled before it started never runs; join and await start one.
    let l = log.clone();
    let lazy = named_args!(scope.launch(start = CoroutineStart::LAZY, async move { push(&l, "cancelled lazy") }));
    lazy.cancel();
    assert!(lazy.is_completed() && !lazy.start());
    let l = log.clone();
    let joined = named_args!(scope.launch(start = CoroutineStart::LAZY, async move { push(&l, "joined") }));
    let deferred = named_args!(scope.r#async(start = CoroutineStart::LAZY, async { 7 }));
    let l = log.clone();
    scope.launch(async move {
        joined.join().await;
        let v = deferred.await.unwrap();
        push(&l, format!("awaited {v}"));
    });
    d.run_ready();
    assert_eq!(taken(&log), ["joined", "awaited 7"]);
    // In a cancelled scope: DEFAULT never runs; UNDISPATCHED runs, cancelled, up to its first suspension.
    scope.cancel();
    let l = log.clone();
    let gone = scope.launch(async move { push(&l, "in a cancelled scope") });
    let l = log.clone();
    named_args!(scope.launch(start = CoroutineStart::UNDISPATCHED, async move {
        push(&l, "undispatched, cancelled");
        delay_ms(10).await;
        push(&l, "never after");
    }));
    d.run_ready();
    assert!(gone.is_cancelled() && gone.is_completed());
    assert_eq!(taken(&log), ["undispatched, cancelled"]);
}

fn a_scope_tells_its_completion_unless_disposed() {
    // Kotlin's `coroutineContext[Job]?.invokeOnCompletion { … }` on a scope's job: told once the scope is cancelled
    // and its coroutines are done; at once if it is already; not if disposed (Compose's emitWithFallback: a
    // launched emit that never ran is tried at the scope's end).
    let d = Dispatcher::new();
    let scope = CoroutineScope::new(&d);
    let log = new_log();
    let l = log.clone();
    let handler = scope.invoke_on_completion(move || push(&l, "fallback"));
    let l = log.clone();
    scope.launch(async move {
        push(&l, "emitted");
        handler.dispose();
    });
    let l = log.clone();
    let kept = scope.invoke_on_completion(move || push(&l, "end"));
    d.run_ready();
    assert_eq!(taken(&log), ["emitted"]);
    // Cancelled before the launched one ran: it never runs, the handler not disposed is told.
    let l = log.clone();
    let handler = scope.invoke_on_completion(move || push(&l, "fallback"));
    let l = log.clone();
    scope.launch(async move {
        push(&l, "emitted");
        handler.dispose();
    });
    scope.cancel();
    assert_eq!(taken(&log), ["end", "fallback"]);
    let _ = kept;
    let l = log.clone();
    scope.invoke_on_completion(move || push(&l, "at once"));
    assert_eq!(taken(&log), ["at once"]);
}

fn a_job_tells_its_completion_unless_disposed() {
    // Kotlin's `scope.launch { … }.invokeOnCompletion { … }` (material3's ModalBottomSheetSample: the sheet hidden,
    // then its flag cleared): told once the job ends — completed, or cancelled (at once if it never ran) —; at once
    // if it has ended already; not if disposed.
    let d = Dispatcher::new();
    let scope = CoroutineScope::new(&d);
    let log = new_log();
    let l = log.clone();
    let job = scope.launch(async move {
        push(&l, "ran");
        r#yield().await;
        push(&l, "resumed");
    });
    let l = log.clone();
    job.invoke_on_completion(move || push(&l, "done"));
    let l = log.clone();
    job.invoke_on_completion(move || push(&l, "disposed")).dispose();
    d.run_ready();
    assert_eq!(taken(&log), ["ran", "resumed", "done"]);
    let l = log.clone();
    job.invoke_on_completion(move || push(&l, "at once"));
    assert_eq!(taken(&log), ["at once"]);
    // Cancelled before it ran: told when cancelled.
    let l = log.clone();
    let never = scope.launch(async move { push(&l, "never") });
    let l = log.clone();
    never.invoke_on_completion(move || push(&l, "cancelled"));
    never.cancel();
    d.run_ready();
    assert_eq!(taken(&log), ["cancelled"]);
    // Launched in a cancelled scope: ended already, told at once.
    scope.cancel();
    let l = log.clone();
    let gone = scope.launch(async move { push(&l, "never") });
    let l = log.clone();
    gone.invoke_on_completion(move || push(&l, "gone"));
    assert_eq!(taken(&log), ["gone"]);
}

fn a_frame_runs_its_callbacks_then_its_continuations() {
    // Compose's frame: the awaiters' `onFrame` run in the frame (BroadcastFrameClock.sendFrame), what they do after
    // is dispatched: it runs after the frame's composition and layout — before the coroutines the frame launched.
    let d = Dispatcher::new();
    let scope = CoroutineScope::new(&d);
    let log = new_log();
    let l = log.clone();
    scope.launch(async move {
        let t = with_frame_nanos(|t| {
            push(&l, format!("on frame {}", t / MS));
            t
        })
        .await;
        push(&l, format!("after the frame {}", t / MS));
        let t = await_frame().await;
        push(&l, format!("awaited {}", t / MS));
    });
    d.run_ready();
    d.send_frame(16 * MS);
    assert_eq!(taken(&log), ["on frame 16"]);
    // (the frame's composition launches a coroutine: it runs after the frame's continuations)
    let l = log.clone();
    scope.launch(async move { push(&l, "launched in the frame") });
    assert!(d.has_ready());
    d.run_frame_end();
    assert_eq!(taken(&log), ["after the frame 16", "launched in the frame"]);
    d.run(32 * MS);
    assert_eq!(taken(&log), ["awaited 32"]);
}

// (WP5: snackbars — kotlinx.coroutines.sync's Mutex)
/// kotlinx's Mutex: one holder at a time, the waiting coroutines served in the order they came; a coroutine
/// cancelled while waiting leaves the queue, `with_lock` unlocks however its action ends, owners are checked.
fn mutexes_hand_themselves_to_coroutines_in_the_order_they_came() {
    use kompoz_coroutines::sync::{Mutex, MutexOwner};
    let (rule, scope) = scope_rule();
    let log = new_log();
    let mutex = Mutex(false);
    assert!(!mutex.is_locked());
    let mut jobs = Vec::new();
    for name in ["a", "b", "c", "d"] {
        let l = log.clone();
        jobs.push(scope.launch(mutex.with_lock(None, move || async move {
            push(&l, format!("{name} in"));
            delay_ms(100).await;
            push(&l, format!("{name} out"));
        })));
    }
    rule.frame();
    assert!(mutex.is_locked() && *log.borrow() == ["a in"]);
    jobs[2].cancel(); // c leaves the queue
    rule.advance_time_by(100);
    assert!(*log.borrow() == ["a in", "a out", "b in"]);
    jobs[1].cancel(); // b cancelled holding it: with_lock lets go, d follows
    rule.frame();
    assert!(*log.borrow() == ["a in", "a out", "b in", "d in"]);
    rule.advance_time_by(100);
    assert!(*log.borrow() == ["a in", "a out", "b in", "d in", "d out"] && !mutex.is_locked());
    // try_lock, owners, unlock by hand.
    let owner: MutexOwner = Rc::new("sahip");
    assert!(mutex.try_lock(Some(owner.clone())) && mutex.holds_lock(&owner) && !mutex.try_lock(None));
    let again = || mutex.try_lock(Some(owner.clone()));
    assert!(quietly(|| std::panic::catch_unwind(std::panic::AssertUnwindSafe(again))).is_err());
    let other: MutexOwner = Rc::new(1);
    let wrong_owner = || mutex.unlock(Some(other.clone()));
    assert!(quietly(|| std::panic::catch_unwind(std::panic::AssertUnwindSafe(wrong_owner))).is_err());
    let (l, m, o) = (log.clone(), mutex.clone(), other.clone());
    scope.launch(async move {
        m.lock(Some(o)).await;
        push(&l, "other");
    });
    rule.frame();
    assert!(log.borrow().last().unwrap() == "d out");
    mutex.unlock(Some(owner.clone()));
    rule.frame();
    assert!(log.borrow().last().unwrap() == "other" && mutex.holds_lock(&other) && !mutex.holds_lock(&owner));
    mutex.unlock(None);
    assert!(!mutex.is_locked() && Mutex(true).is_locked() && mutex == mutex.clone() && mutex != Mutex(false));
    let not_locked = || mutex.unlock(None);
    assert!(quietly(|| std::panic::catch_unwind(std::panic::AssertUnwindSafe(not_locked))).is_err());
}

// (WP5: snackbars — an indefinite snackbar's delay)
/// kotlinx's `delay(Long.MAX_VALUE)` waits forever without a timer (nothing for a test's idling to jump to).
fn a_delay_of_long_max_value_waits_forever_without_a_timer() {
    let (rule, scope) = scope_rule();
    let log = new_log();
    let l = log.clone();
    let job = scope.launch(async move {
        delay(i64::MAX).await;
        push(&l, "never");
    });
    rule.frame();
    assert!(!rule.dispatcher.has_timers() && job.is_active());
    rule.wait_for_idle();
    assert!(log.borrow().is_empty() && job.is_active());
    job.cancel();
    rule.frame();
    assert!(job.is_cancelled());
}

tests!(
    launched_effect_ticks_and_is_cancelled_on_leave,
    coroutines_give_back_values_and_work_in_the_background,
    background_failures_reach_the_awaiting_coroutine,
    launch_gives_a_job_and_async_a_deferred,
    coroutine_scopes_wait_for_their_children_and_fail_with_them,
    with_timeout_cancels_what_runs_too_long,
    launched_effect_restarts_when_key_changes,
    coroutine_scope_launches_from_a_click,
    coroutine_failure_is_reported_not_fatal,
    frames_are_awaited_and_give_their_time,
    snapshot_flow_reports_each_new_value,
    state_flow_is_collected_as_state,
    flows_produce_on_demand_and_operators_shape_them,
    flow_collection_stops_with_its_coroutine,
    flows_combine_debounce_and_follow_the_latest_into_a_state_flow,
    shared_flows_hand_every_value_to_every_collector,
    hot_flows_take_flows_operators_on_their_values,
    rendezvous_channels_hand_each_element_from_sender_to_receiver,
    buffered_channels_wait_only_when_full,
    conflated_and_dropping_channels_keep_what_their_overflow_says,
    closed_channels_give_what_is_left_and_cancelled_ones_drop_it,
    channel_receivers_share_its_elements_and_a_cancelled_one_takes_nothing,
    produce_sends_from_a_coroutine_and_consume_each_takes_it_all,
    closed_channels_answer_at_once_and_consumers_and_producers_end_them,
    sharing_started_while_subscribed_collects_only_while_collected,
    state_flow_values_arrive_after_input_stamped_ahead_of_frames,
    state_flow_collect_follows_the_latest_value_until_cancelled,
    remember_updated_state_gives_effects_the_latest_value,
    produce_state_from_a_coroutine,
    launched_effects_and_produce_state_take_several_keys,
    run_woken_by_resumes_what_it_wakes_before_returning,
    run_woken_by_from_inside_a_coroutine_keeps_its_own_wake,
    coroutine_contexts_reach_the_block_and_what_it_launches_in_its_scopes,
    a_cancelled_coroutine_scope_cancels_who_awaits_it,
    snapshot_flow_follows_a_derived_state_made_in_the_composition,
    catch_cancellation_gives_the_cancellation_to_the_block_that_catches_it,
    await_frame_end_resumes_after_the_frames_work,
    launch_starts_as_kotlins_coroutine_start_says,
    a_scope_tells_its_completion_unless_disposed,
    a_job_tells_its_completion_unless_disposed,
    a_frame_runs_its_callbacks_then_its_continuations,
    mutexes_hand_themselves_to_coroutines_in_the_order_they_came,
    a_delay_of_long_max_value_waits_forever_without_a_timer,
);

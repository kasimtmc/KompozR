//! The runtime's tests: composition, recomposition, remember, effects, locals, derived states, collections,
//! movable content (the C++ Kompoz's runtime tests, with test nodes in place of its layout nodes).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_runtime::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

thread_local! {
    static COUNTER_RUNS: Cell<u32> = const { Cell::new(0) };
    static STATIC_RUNS: Cell<u32> = const { Cell::new(0) };
    static MIDDLE_RUNS: Cell<u32> = const { Cell::new(0) };
    static READER_RUNS: Cell<u32> = const { Cell::new(0) };
    static SHOWN_RUNS: Cell<u32> = const { Cell::new(0) };
}

/// Runs `f`, which is expected to panic, without the panic printed.
fn quietly<R>(f: impl FnOnce() -> R) -> std::thread::Result<R> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(hook);
    r
}

fn bump(c: &'static std::thread::LocalKey<Cell<u32>>) {
    c.with(|c| c.set(c.get() + 1));
}
fn runs(c: &'static std::thread::LocalKey<Cell<u32>>) -> u32 {
    c.with(|c| c.get())
}
fn reset(c: &'static std::thread::LocalKey<Cell<u32>>) {
    c.with(|c| c.set(0));
}

#[kompozable]
fn CounterView(count: MutableState<i32>) {
    bump(&COUNTER_RUNS);
    Text(format!("count={count}"));
}

#[kompozable]
fn StaticView() {
    bump(&STATIC_RUNS);
    Text("static");
}

/// A row that remembers its own click count.
#[kompozable(inline)]
fn Item(id: i32, clicks_out: Rc<RefCell<Vec<(i32, MutableState<i32>)>>>) {
    let clicks = remember(|| mutable_state_of(0));
    clicks_out.borrow_mut().push((id, clicks.clone()));
    Text(format!("item{id}:{clicks}"));
}

fn click(handles: &Rc<RefCell<Vec<(i32, MutableState<i32>)>>>, id: i32) {
    let c = handles.borrow().iter().rev().find(|(i, _)| *i == id).unwrap().1.clone();
    c.set(c.peek() + 1);
}

fn nodes_are_retained_across_recompositions() {
    let label = mutable_state_of("a".to_string());
    let l = label.clone();
    let rule = Rule::new(kompozable_lambda!(move || Text(l.get())));
    let before = rule.root.children.borrow()[0].clone();
    label.set("b".into());
    rule.wait_for_idle();
    let after = rule.root.children.borrow()[0].clone();
    assert!(Rc::ptr_eq(&before, &after));
    assert_eq!(rule.names(), ["b"]);
}

fn key_keeps_state_with_its_item() {
    let ids = mutable_state_of(vec![1, 2, 3]);
    let handles = Rc::new(RefCell::new(Vec::new()));
    let (i, h) = (ids.clone(), handles.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let (i, h) = (i.clone(), h.clone());
        Column("column", move || {
            for id in i.get() {
                key(&id, || Item(id, h.clone()));
            }
        });
    }));
    click(&handles, 2);
    rule.wait_for_idle();
    assert!(rule.has("item2:1"));
    ids.update(|v| {
        v.remove(0);
    }); // item 1 removed
    rule.wait_for_idle();
    assert!(rule.has("item2:1")); // the state moved with item 2
    assert!(rule.has("item3:0"));
}

fn without_key_state_is_positional() {
    let ids = mutable_state_of(vec![1, 2]);
    let handles = Rc::new(RefCell::new(Vec::new()));
    let (i, h) = (ids.clone(), handles.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let (i, h) = (i.clone(), h.clone());
        Column("column", move || {
            for id in i.get() {
                Item(id, h.clone());
            }
        });
    }));
    click(&handles, 1);
    ids.update(|v| {
        v.remove(0);
    });
    rule.wait_for_idle();
    assert!(rule.has("item2:1")); // as in Compose without key()
}

fn disposable_effect_runs_and_cleans_up() {
    let show = mutable_state_of(true);
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let (s, l) = (show.clone(), log.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        if s.get() {
            let l = l.clone();
            DisposableEffect(0, move || {
                l.borrow_mut().push("start".into());
                on_dispose(move || l.borrow_mut().push("stop".into()))
            });
        }
    }));
    assert_eq!(*log.borrow(), ["start"]);
    show.set(false);
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["start", "stop"]);
}

fn disposable_effect_restarts_when_key_changes() {
    let k = mutable_state_of(1);
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let (kk, l) = (k.clone(), log.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let key = kk.get();
        let l = l.clone();
        DisposableEffect(key, move || {
            l.borrow_mut().push(format!("start{key}"));
            on_dispose(move || l.borrow_mut().push(format!("stop{key}")))
        });
    }));
    k.set(2);
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["start1", "stop1", "start2"]);
}

/// Takes a state it only reads: a mutable or a derived one.
#[kompozable]
fn Shown(value: State<i32>) {
    bump(&SHOWN_RUNS);
    Text(format!("value={value}"));
}

fn read_only_states_come_from_mutable_and_derived_ones() {
    reset(&SHOWN_RUNS);
    let count = mutable_state_of(1);
    let other = mutable_state_of(0);
    let (c, o) = (count.clone(), other.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        o.get(); // the parent runs again, the same states given: they're skipped
        let cc = c.clone();
        let doubled = remember(|| derived_state_of(move || cc.get() * 2));
        let c = c.clone();
        Column("column", move || {
            Shown(c.clone().into());
            Shown(doubled.clone().into());
        });
    }));
    assert!(rule.has("value=1") && rule.has("value=2") && runs(&SHOWN_RUNS) == 2);
    other.set(1);
    rule.wait_for_idle();
    assert_eq!(runs(&SHOWN_RUNS), 2);
    count.set(5); // each read its state: both again
    rule.wait_for_idle();
    assert!(rule.has("value=5") && rule.has("value=10"), "{:?}", rule.names());
    assert_eq!(runs(&SHOWN_RUNS), 4);
    let plain: State<i32> = count.clone().into();
    assert!(plain.peek() == 5 && plain.get() == 5 && plain == State::from(&count) && plain != State::from(&other));
}

fn snapshots_see_states_as_they_were_and_apply_their_writes_together() {
    let a = mutable_state_of(1);
    let b = mutable_state_of(10);
    let taken = Snapshot::take_snapshot(None); // what is written after isn't seen in it
    a.set(2);
    assert!(taken.enter(|| a.get()) == 1 && a.peek() == 2);
    taken.dispose();
    // A mutable snapshot's writes: seen once it is applied, together.
    let both = Snapshot::take_mutable_snapshot(None, None);
    both.enter(|| {
        a.set(3);
        b.set(30);
    });
    assert!(a.peek() == 2 && b.peek() == 10 && both.enter(|| a.peek() + b.peek()) == 33);
    assert!(both.apply() == SnapshotApplyResult::Success && a.peek() == 3 && b.peek() == 30);
    both.dispose();
    // Written meanwhile elsewhere to another value: it fails; to the same one, it applies.
    let late = Snapshot::take_mutable_snapshot(None, None);
    late.enter(|| a.set(4));
    a.set(5);
    assert!(late.apply() == SnapshotApplyResult::Failure && a.peek() == 5);
    late.dispose();
    let same = Snapshot::take_mutable_snapshot(None, None);
    same.enter(|| a.set(6));
    a.set(6);
    assert!(same.apply() == SnapshotApplyResult::Success && a.peek() == 6);
    same.dispose();
    let conflicted = Snapshot::with_mutable_snapshot(|| {
        a.set(7);
        Snapshot::with_mutable_snapshot(|| a.set(8)).unwrap(); // (nested: applied to the outer one, which then sees 8)
        assert_eq!(a.peek(), 8);
        let a2 = a.clone();
        std::thread::spawn(move || a2.set(9)).join().unwrap(); // another thread writes it meanwhile
    });
    assert!(conflicted.is_err() && a.peek() == 9);
    // Nested: applied to its parent, seen outside once the parent is.
    let outer = Snapshot::take_mutable_snapshot(None, None);
    outer.enter(|| {
        let inner = Snapshot::take_mutable_snapshot(None, None);
        inner.enter(|| b.set(40));
        assert_eq!(b.peek(), 30);
        assert!(inner.apply() == SnapshotApplyResult::Success && b.peek() == 40);
        inner.dispose();
    });
    assert!(b.peek() == 30 && outer.apply() == SnapshotApplyResult::Success && b.peek() == 40);
    outer.dispose();
    assert!(
        Snapshot::with_mutable_snapshot(|| {
            b.set(41);
            b.peek()
        }) == Ok(41)
            && b.peek() == 41
    );
    // A read observer hears what is read in it (not peeked); a read-only snapshot can't be written.
    let reads = Arc::new(Mutex::new(Vec::new()));
    let r = reads.clone();
    let observed = Snapshot::take_snapshot(Some(Arc::new(move |k| r.lock().unwrap().push(k))));
    observed.enter(|| {
        a.get();
        b.peek();
    });
    assert_eq!(*reads.lock().unwrap(), [a.key()]);
    let refused = quietly(|| observed.enter(|| a.set(0))).is_err();
    assert!(refused && a.peek() == 9);
    observed.dispose();
}

use std::sync::{Arc, Mutex};

fn snapshots_keep_only_the_records_someone_reads() {
    let state = mutable_state_of(0);
    for i in 1..=50 {
        state.set(i);
        Snapshot::send_apply_notifications();
    }
    assert!(state.get() == 50 && state.record_count() <= 2, "{}", state.record_count());
    let pinned = Snapshot::take_snapshot(None); // reads 50 as long as it is open
    for i in 51..=100 {
        state.set(i);
        Snapshot::send_apply_notifications();
    }
    assert!(pinned.enter(|| state.get()) == 50 && state.get() == 100 && state.record_count() <= 3);
    pinned.dispose();
    state.set(101);
    Snapshot::send_apply_notifications();
    assert!(state.get() == 101 && state.record_count() <= 2);
}

fn a_composition_sees_other_threads_writes_whole_and_once_applied() {
    let first = mutable_state_of(0);
    let second = mutable_state_of(0);
    let seen = Rc::new(RefCell::new(Vec::<(i32, i32)>::new()));
    let (f, s, sn) = (first.clone(), second.clone(), seen.clone());
    let rule = Rule::new(kompozable_lambda!(move || sn.borrow_mut().push((f.get(), s.get()))));
    Snapshot::with_mutable_snapshot(|| {
        first.set(1);
        second.set(1);
    })
    .unwrap();
    rule.wait_for_idle();
    assert!(seen.borrow().len() == 2 && *seen.borrow().last().unwrap() == (1, 1)); // composed once for both
    // Another thread writing both together, the composition going on meanwhile: never one without the other.
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (f, s, d) = (first.clone(), second.clone(), done.clone());
    let writer = std::thread::spawn(move || {
        for i in 2..=300 {
            Snapshot::with_mutable_snapshot(|| {
                f.set(i);
                s.set(i);
            })
            .unwrap();
        }
        d.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    while !done.load(std::sync::atomic::Ordering::SeqCst) {
        rule.wait_for_idle();
    }
    writer.join().unwrap();
    rule.wait_for_idle();
    assert_eq!(*seen.borrow().last().unwrap(), (300, 300));
    assert!(seen.borrow().iter().all(|(a, b)| a == b));
    // Written outside a snapshot from another thread, read there in one: as written; on the UI thread after a
    // frame.
    let f = first.clone();
    let read = std::thread::spawn(move || {
        f.set(400);
        let snapshot = Snapshot::take_snapshot(None);
        let v = snapshot.enter(|| f.peek());
        snapshot.dispose();
        v
    })
    .join()
    .unwrap();
    rule.wait_for_idle();
    assert!(read == 400 && seen.borrow().last().unwrap().0 == 400);
}

fn states_take_a_mutation_policy() {
    let same = mutable_state_of(1); // ==: the same value changes nothing
    let every = mutable_state_of_with_policy(1, never_equal_policy()); // every set() recomposes
    let sign = mutable_state_of_with_policy(1, SnapshotMutationPolicy::new(|a: &i32, b: &i32| (*a < 0) == (*b < 0)));
    let counts = Rc::new([Cell::new(0), Cell::new(0), Cell::new(0), Cell::new(0)]);
    let (s, e, g, c) = (same.clone(), every.clone(), sign.clone(), counts.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let (s, e, g, c) = (s.clone(), e.clone(), g.clone(), c.clone());
        let c1 = c.clone();
        let s1 = s.clone();
        scoped(move || {
            s1.get();
            c1[0].set(c1[0].get() + 1);
        });
        let c2 = c.clone();
        scoped(move || {
            e.get();
            c2[1].set(c2[1].get() + 1);
        });
        let c3 = c.clone();
        scoped(move || {
            g.get();
            c3[2].set(c3[2].get() + 1);
        });
        // Its readers recompose whenever it's computed again, the same value too.
        let tens = remember(|| derived_state_of_with_policy(never_equal_policy(), move || s.get() / 10));
        scoped(move || {
            tens.get();
            c[3].set(c[3].get() + 1);
        });
    }));
    let n = |i: usize| counts[i].get();
    same.set(1);
    every.set(1);
    sign.set(5);
    rule.wait_for_idle();
    assert_eq!((n(0), n(1), n(2), n(3)), (1, 2, 1, 1));
    sign.set(-1);
    same.set(2); // 2 / 10 is 0 again, but the derived state's policy says it changed
    rule.wait_for_idle();
    assert!(n(0) == 2 && n(2) == 2 && sign.peek() == -1 && n(3) == 2, "{:?}", (n(0), n(1), n(2), n(3)));
}

fn remember_and_effects_take_two_or_three_keys() {
    let (a, b, c) = (mutable_state_of(1), mutable_state_of(1), mutable_state_of(1));
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let made = Rc::new([Cell::new(0), Cell::new(0)]);
    let (aa, bb, cc, l, m) = (a.clone(), b.clone(), c.clone(), log.clone(), made.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let (x, y, z) = (aa.get(), bb.get(), cc.get());
        remember_keyed((x, y), || {
            m[0].set(m[0].get() + 1);
            m[0].get()
        });
        remember_keyed((x, y, z), || {
            m[1].set(m[1].get() + 1);
            m[1].get()
        });
        let l = l.clone();
        DisposableEffect((x, y), move || {
            l.borrow_mut().push(format!("+{x}{y}"));
            on_dispose(move || l.borrow_mut().push("-".into()))
        });
    }));
    assert!(made[0].get() == 1 && made[1].get() == 1 && *log.borrow() == ["+11"]);
    log.borrow_mut().clear();
    c.set(2); // only in the three-key one
    rule.wait_for_idle();
    assert!(made[0].get() == 1 && made[1].get() == 2 && log.borrow().is_empty());
    b.set(2);
    rule.wait_for_idle();
    assert!(made[0].get() == 2 && made[1].get() == 3 && *log.borrow() == ["-", "+12"]);
}

/// Tells what it is told, as Compose's RememberObserver.
struct Remembered {
    log: Rc<RefCell<Vec<String>>>,
    name: String,
}
impl RememberObserver for Remembered {
    fn on_remembered(&self) {
        self.log.borrow_mut().push(format!("+{}", self.name));
    }
    fn on_forgotten(&self) {
        self.log.borrow_mut().push(format!("-{}", self.name));
    }
}

#[kompozable(inline)]
fn effect(log: &Rc<RefCell<Vec<String>>>, name: &'static str, k: i32) {
    let log = log.clone();
    DisposableEffect(k, move || {
        log.borrow_mut().push(format!("+{name}{k}"));
        on_dispose(move || log.borrow_mut().push(format!("-{name}{k}")))
    });
}

fn effects_run_in_composes_order() {
    // As Compose applies a composition: what left it is forgotten first, in the reverse of the order it was
    // composed in, then what entered it is remembered in order, then the side effects.
    let key = mutable_state_of(1);
    let show = mutable_state_of(true);
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let (k, s, l) = (key.clone(), show.clone(), log.clone());
    let mut rule = Rule::new(kompozable_lambda!(move || {
        let kv = k.get();
        let l1 = l.clone();
        SideEffect(move || l1.borrow_mut().push("side".into()));
        effect(&l, "a", kv);
        effect(&l, "b", kv);
        let lr = l.clone();
        remember_observer_keyed(kv, move || Remembered { log: lr, name: format!("r{kv}") });
        let (s2, l3) = (s.clone(), l.clone());
        Column("column", move || {
            if s2.get() {
                effect(&l3, "x", 0);
                effect(&l3, "y", 0);
                effect(&l3, "z", 0);
            }
        });
        Text("later");
        if !s.get() {
            l.borrow_mut().push("composed".into());
        }
    }));
    assert_eq!(*log.borrow(), ["+a1", "+b1", "+r1", "+x0", "+y0", "+z0", "side"]);
    log.borrow_mut().clear();
    key.set(2); // a key changed: the old ones go, all of them before the new ones come
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["-r1", "-b1", "-a1", "+a2", "+b2", "+r2", "side"]);
    log.borrow_mut().clear();
    show.set(false); // left: after the composition is over, last composed first
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["composed", "-z0", "-y0", "-x0", "side"]);
    log.borrow_mut().clear();
    rule.dispose(); // the composition disposed of: all of it, last composed first
    assert_eq!(*log.borrow(), ["-r2", "-b2", "-a2"]);
}

static LocalName: KompozitionLocal<String> = KompozitionLocal::new(|| "default".into());

fn kompozition_local_is_scoped() {
    let rule = Rule::new(kompozable_lambda!(|| {
        Column("column", || {
            Text(format!("a={}", LocalName.current()));
            KompozitionLocalProvider(&[LocalName.provides("inner".into())], || {
                Text(format!("b={}", LocalName.current()))
            });
            Text(format!("c={}", LocalName.current()));
        });
    }));
    assert_eq!(rule.names(), ["column", "a=default", "b=inner", "c=default"]);
}

thread_local! {
    static MADE: Cell<u32> = const { Cell::new(0) };
}
static LocalLazy: KompozitionLocal<i32> = kompozition_local_of(|| {
    MADE.with(|m| m.set(m.get() + 1));
    7
});
static LocalNeeded: KompozitionLocal<String> = kompozition_local_of(|| panic!("not provided"));
static LocalTheme: KompozitionLocal<String> = static_kompozition_local_of(|| "light".into());

fn kompozition_locals_from_factories_defaults_and_contexts() {
    let tick = mutable_state_of(0);
    let mode = mutable_state_of("dark".to_string());
    #[derive(Default)]
    struct Seen {
        lazy: i32,
        lazy_again: i32,
        threw: bool,
        plain: String,
        kept: String,
        own: String,
        elsewhere: String,
    }
    let seen = Rc::new(RefCell::new(Seen::default()));
    let here = Rc::new(RefCell::new(KompozitionLocalContext::default()));
    let (t, m, sn, h) = (tick.clone(), mode.clone(), seen.clone(), here.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        t.get();
        sn.borrow_mut().lazy = LocalLazy.current();
        sn.borrow_mut().lazy_again = LocalLazy.current();
        let threw = quietly(|| LocalNeeded.current()).is_err();
        sn.borrow_mut().threw = threw;
        sn.borrow_mut().plain = LocalTheme.current();
        let (sn2, h2) = (sn.clone(), h.clone());
        KompozitionLocalProvider(&[LocalTheme.provides(m.get()), LocalNeeded.provides("given".into())], || {
            KompozitionLocalProvider(&[LocalTheme.provides_default("light".into())], || {
                sn2.borrow_mut().kept = LocalTheme.current(); // provided above
            });
            let h3 = h2.clone();
            scoped(move || *h3.borrow_mut() = current_kompozition_local_context()); // recomposed as they change
        });
        KompozitionLocalProvider(&[LocalTheme.provides_default("default".into())], || {
            sn.borrow_mut().own = LocalTheme.current(); // nothing above
        });
        let context = h.borrow().clone();
        KompozitionLocalProviderContext(&context, || {
            sn.borrow_mut().elsewhere = format!("{}/{}", LocalNeeded.current(), LocalTheme.current());
        });
    }));
    {
        let s = seen.borrow();
        assert!(s.lazy == 7 && s.lazy_again == 7 && MADE.with(|m| m.get()) == 1 && s.threw && s.plain == "light");
        assert!(
            s.kept == "dark" && s.own == "default" && s.elsewhere == "given/dark",
            "{} {} {}",
            s.kept,
            s.own,
            s.elsewhere
        );
    }
    tick.set(1); // composed again: the default stays the one made
    rule.wait_for_idle();
    assert!(MADE.with(|m| m.get()) == 1 && seen.borrow().lazy == 7);
    mode.set("night".into());
    rule.wait_for_idle();
    assert_eq!(seen.borrow().elsewhere, "given/night");
}

fn only_readers_of_a_state_recompose() {
    reset(&COUNTER_RUNS);
    reset(&STATIC_RUNS);
    let count = mutable_state_of(0);
    let root_runs = Rc::new(Cell::new(0));
    let (c, r) = (count.clone(), root_runs.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        r.set(r.get() + 1);
        let c = c.clone();
        Column("column", move || {
            CounterView(c.clone());
            StaticView();
        });
    }));
    count.set(1);
    rule.wait_for_idle();
    assert!(rule.has("count=1"));
    assert_eq!(root_runs.get(), 1); // the parent didn't read `count`
    assert_eq!(runs(&COUNTER_RUNS), 2);
    assert_eq!(runs(&STATIC_RUNS), 1);
}

fn parent_recomposition_skips_unchanged_children() {
    reset(&STATIC_RUNS);
    let tick = mutable_state_of(0);
    let t = tick.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        let t = t.clone();
        Column("column", move || {
            Text(format!("tick={t}"));
            Text("fixed");
            StaticView();
        });
    }));
    let skipped_before = rule.composition().stats().scopes_skipped;
    tick.set(1);
    rule.wait_for_idle();
    assert!(rule.has("tick=1"));
    assert_eq!(runs(&STATIC_RUNS), 1);
    // Text("fixed") and StaticView were skipped: equal arguments, nothing read changed.
    assert!(rule.composition().stats().scopes_skipped >= skipped_before + 2);
    assert_eq!(rule.root.find("fixed").unwrap().compositions.get(), 1);
}

fn changed_argument_reruns_child() {
    reset(&COUNTER_RUNS);
    let (a, b) = (mutable_state_of(0), mutable_state_of(0));
    let which = mutable_state_of(true);
    let (aa, bb, w) = (a.clone(), b.clone(), which.clone());
    let rule = Rule::new(kompozable_lambda!(move || CounterView(if w.get() { aa.clone() } else { bb.clone() })));
    assert_eq!(runs(&COUNTER_RUNS), 1);
    which.set(false); // a different state object: the argument changed
    rule.wait_for_idle();
    assert_eq!(runs(&COUNTER_RUNS), 2);
    b.set(7); // now subscribed to b
    rule.wait_for_idle();
    assert!(rule.has("count=7"));
    a.set(9); // no longer read by anyone
    rule.wait_for_idle();
    assert_eq!(runs(&COUNTER_RUNS), 3);
}

static LocalLevel: KompozitionLocal<i32> = KompozitionLocal::new(|| 0);

#[kompozable]
fn Middle() {
    bump(&MIDDLE_RUNS);
    Reader();
}

#[kompozable]
fn Reader() {
    bump(&READER_RUNS);
    Text(format!("level={}", LocalLevel.current()));
}

fn local_change_reaches_reader_under_skipped_parent() {
    reset(&MIDDLE_RUNS);
    reset(&READER_RUNS);
    let level = mutable_state_of(1);
    let l = level.clone();
    let rule =
        Rule::new(kompozable_lambda!(move || KompozitionLocalProvider(&[LocalLevel.provides(l.get())], || Middle())));
    assert!(rule.has("level=1"));
    level.set(2);
    rule.wait_for_idle();
    assert!(rule.has("level=2"));
    assert_eq!(runs(&MIDDLE_RUNS), 1); // skipped: it doesn't read the local
    assert_eq!(runs(&READER_RUNS), 2);
}

static LocalSpeed: KompozitionLocal<i32> = kompozition_local_of(|| 0);
static LocalUnit: KompozitionLocal<String> = static_kompozition_local_of(|| "m".into());
/// Values the same when they round to the same ten.
static LocalRounded: KompozitionLocal<i32> =
    kompozition_local_of_with_policy(SnapshotMutationPolicy::new(|a, b| a / 10 == b / 10), || 0);

thread_local! {
    static KEEPER_RUNS: Cell<u32> = const { Cell::new(0) };
    static SPEED_RUNS: Cell<u32> = const { Cell::new(0) };
    static KEPT: RefCell<KompozitionLocalContext> = RefCell::new(KompozitionLocalContext::default());
}

/// Keeps the locals where it is as a layout node does.
#[kompozable]
fn Keeper() {
    bump(&KEEPER_RUNS);
    let map = current_kompozition_local_map();
    KEPT.with(|k| *k.borrow_mut() = map);
}

#[kompozable]
fn SpeedReader() {
    bump(&SPEED_RUNS);
    Text(format!("speed={}", LocalSpeed.current()));
}

fn kept(f: impl FnOnce(&KompozitionLocalContext) -> String) -> String {
    KEPT.with(|k| f(&k.borrow()))
}

fn dynamic_locals_are_read_live_and_static_ones_compose_their_keepers_again() {
    // Compose's DynamicValueHolder: a dynamic local's provider sets the value in its holder; what kept the locals
    // reads it as it is now, observed as a state read is (a layout's measure runs again, nothing composes); its
    // readers in the composition compose again once. A static local's new value comes in a holder of its own: what
    // kept the locals composes again to keep it.
    reset(&KEEPER_RUNS);
    reset(&SPEED_RUNS);
    let (speed, unit, rounded) = (mutable_state_of(1), mutable_state_of("m".to_string()), mutable_state_of(11));
    let (s, u, r) = (speed.clone(), unit.clone(), rounded.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        KompozitionLocalProvider(
            &[LocalSpeed.provides(s.get()), LocalUnit.provides(u.get()), LocalRounded.provides(r.get())],
            || {
                Keeper();
                SpeedReader();
            },
        );
    }));
    assert_eq!((runs(&KEEPER_RUNS), runs(&SPEED_RUNS)), (1, 1));
    // Read outside the composition: observed (the key told to the observer), as it is now.
    let read_keys = Rc::new(RefCell::new(Vec::<StateKey>::new()));
    let rk = read_keys.clone();
    let observed =
        observe_reads(Rc::new(move |k| rk.borrow_mut().push(k)), || kept(|k| k.get(&LocalSpeed).to_string()));
    assert!(observed == "1" && read_keys.borrow().len() == 1);
    let changed = std::sync::Arc::new(parking_lot::Mutex::new(Vec::<StateKey>::new()));
    let c = changed.clone();
    let _observer = Snapshot::register_apply_observer(move |keys| c.lock().extend_from_slice(keys));
    speed.set(2);
    rule.wait_for_idle();
    assert!(rule.has("speed=2"));
    assert_eq!((runs(&KEEPER_RUNS), runs(&SPEED_RUNS)), (1, 2)); // (the reader once, the keeper not at all)
    assert_eq!(kept(|k| k.get(&LocalSpeed).to_string()), "2");
    assert!(changed.lock().contains(&read_keys.borrow()[0])); // (the measure that read it would run again)
    // A static local: the keeper composes again and keeps the new holder.
    unit.set("km".into());
    rule.wait_for_idle();
    assert_eq!(runs(&KEEPER_RUNS), 2);
    assert_eq!(kept(|k| k.get(&LocalUnit)), "km");
    // A static local read from what was kept is read by no one.
    let static_keys = Rc::new(RefCell::new(Vec::<StateKey>::new()));
    let sk = static_keys.clone();
    observe_reads(Rc::new(move |k| sk.borrow_mut().push(k)), || kept(|k| k.get(&LocalUnit)));
    assert!(static_keys.borrow().is_empty());
    // The local's mutation policy: a value it takes as the same sets nothing.
    changed.lock().clear();
    rounded.set(15);
    rule.wait_for_idle();
    assert_eq!(kept(|k| k.get(&LocalRounded).to_string()), "11");
    rounded.set(25);
    rule.wait_for_idle();
    assert_eq!(kept(|k| k.get(&LocalRounded).to_string()), "25");
    assert_eq!(runs(&KEEPER_RUNS), 2);
}

// ---- Computed locals (Compose 1.7's providesComputed, compositionLocalWithComputedDefaultOf) ----------------------

static LocalBase: KompozitionLocal<i32> = KompozitionLocal::new(|| 1);
static LocalTwice: KompozitionLocal<i32> = KompozitionLocal::new(|| 0);
static LocalThrice: KompozitionLocal<i32> =
    kompozition_local_with_computed_default_of(|scope| scope.current_value(&LocalBase) * 3);

fn computed_locals_are_computed_where_they_are_read() {
    // Compose's ComputedValueHolder: the computation reads the other locals as its reader sees them (a provider
    // below the computed one's counts); a computed default the same; a value provided for either takes its place.
    reset(&KEEPER_RUNS);
    let rule = Rule::new(kompozable_lambda!(|| {
        Text(format!("default={}/{}", LocalTwice.current(), LocalThrice.current()));
        KompozitionLocalProvider(&[LocalTwice.provides_computed(|scope| scope.current_value(&LocalBase) * 2)], || {
            Text(format!("outer={}/{}", LocalTwice.current(), LocalThrice.current()));
            KompozitionLocalProvider(&[LocalBase.provides(10)], || {
                Text(format!("inner={}/{}", LocalTwice.current(), LocalThrice.current()));
                Keeper();
                KompozitionLocalProvider(&[LocalThrice.provides(7), LocalTwice.provides(5)], || {
                    Text(format!("given={}/{}", LocalTwice.current(), LocalThrice.current()));
                });
            });
        });
    }));
    assert!(rule.has("default=0/3") && rule.has("outer=2/3"), "{:?}", rule.names());
    assert!(rule.has("inner=20/30") && rule.has("given=5/7"), "{:?}", rule.names());
    // Read from the locals a layout keeps: computed there, from what they hold.
    assert_eq!(kept(|k| format!("{}/{}", k.get(&LocalTwice), k.peek(&LocalThrice))), "20/30");
}

static LocalTint: KompozitionLocal<i32> = KompozitionLocal::new(|| 0);

thread_local! {
    static PROVIDER_RUNS: Cell<u32> = const { Cell::new(0) };
    static TINT_RUNS: Cell<u32> = const { Cell::new(0) };
}

#[kompozable]
fn TintReader() {
    bump(&TINT_RUNS);
    Text(format!("tint={}", LocalTint.current()));
}

#[kompozable]
fn TintedContent() {
    TintReader();
    Keeper();
}

fn a_computed_local_is_followed_by_its_readers_alone() {
    // What the computation reads is its reader's read: a state it reads changing composes the reader again (and
    // tells whoever read it from kept locals), not the provider. The provider composing again with a closure that
    // captures the same values keeps it (Compose's memoized lambda, the holder kept): no reader is told; one that
    // captures another value is another value: its readers, and the layouts keeping it, compose again.
    reset(&PROVIDER_RUNS);
    reset(&TINT_RUNS);
    reset(&KEEPER_RUNS);
    let (tint, other, offset) = (mutable_state_of(1), mutable_state_of(0), mutable_state_of(0));
    let (t, o, f) = (tint.clone(), other.clone(), offset.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        bump(&PROVIDER_RUNS);
        let _ = o.get();
        let add = f.get();
        let tint = t.clone();
        KompozitionLocalProvider(&[LocalTint.provides_computed(move |_| tint.get() * 10 + add)], || TintedContent());
    }));
    let runs_now = || (runs(&PROVIDER_RUNS), runs(&TINT_RUNS), runs(&KEEPER_RUNS));
    assert!(rule.has("tint=10") && runs_now() == (1, 1, 1));
    tint.set(2);
    rule.wait_for_idle();
    assert!(rule.has("tint=20"), "{:?}", rule.names());
    assert_eq!(runs_now(), (1, 2, 1)); // the reader alone
    // Read from the kept locals: computed there, its state read observed (a measure reading it would run again).
    let read_keys = Rc::new(RefCell::new(Vec::<StateKey>::new()));
    let rk = read_keys.clone();
    let observed = observe_reads(Rc::new(move |k| rk.borrow_mut().push(k)), || kept(|k| k.get(&LocalTint).to_string()));
    assert!(observed == "20" && read_keys.borrow().len() == 1);
    let changed = std::sync::Arc::new(parking_lot::Mutex::new(Vec::<StateKey>::new()));
    let c = changed.clone();
    let _observer = Snapshot::register_apply_observer(move |keys| c.lock().extend_from_slice(keys));
    tint.set(3);
    rule.wait_for_idle();
    assert!(changed.lock().contains(&read_keys.borrow()[0]));
    assert!(rule.has("tint=30") && runs_now() == (1, 3, 1));
    // The provider composed again for something else, its closure capturing the same: its readers stay.
    other.set(1);
    rule.wait_for_idle();
    assert_eq!(runs_now(), (2, 3, 1));
    // Another value captured: another computation, read again (the keeper keeps the new one).
    offset.set(5);
    rule.wait_for_idle();
    assert!(rule.has("tint=35"), "{:?}", rule.names());
    assert_eq!(runs_now(), (3, 4, 2));
    assert_eq!(kept(|k| k.get(&LocalTint).to_string()), "35");
    // (the new computation followed as the old one was)
    tint.set(4);
    rule.wait_for_idle();
    assert!(rule.has("tint=45") && runs_now() == (3, 5, 2));
}

#[kompozable]
fn ContextUser() {
    Text(format!("ctx={}/{}", LocalSpeed.current(), LocalUnit.current()));
}

fn a_context_provided_again_follows_its_provider() {
    // A context provided elsewhere shares its dynamic holders: what reads them there follows the provider here
    // (Compose's DynamicValueHolder.toProvided keeps the state).
    let speed = mutable_state_of(3);
    let s = speed.clone();
    let context = Rc::new(RefCell::new(None::<KompozitionLocalContext>));
    let c = context.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        let c2 = c.clone();
        KompozitionLocalProvider(&[LocalSpeed.provides(s.get()), LocalUnit.provides("km".into())], move || {
            let map = current_kompozition_local_map();
            c2.borrow_mut().get_or_insert(map);
        });
    }));
    let taken = context.borrow().clone().unwrap();
    let elsewhere = Rule::new(kompozable_lambda!(move || {
        KompozitionLocalProviderContext(&taken, || ContextUser());
    }));
    assert!(elsewhere.has("ctx=3/km"));
    speed.set(4);
    rule.wait_for_idle();
    elsewhere.wait_for_idle();
    assert!(elsewhere.has("ctx=4/km"), "{:?}", elsewhere.names());
}

#[kompozable]
fn Items(n: MutableState<i32>) {
    for i in 0..n.get() {
        Text(format!("item{i}"));
    }
}

fn partial_recomposition_keeps_node_order() {
    let n = mutable_state_of(1);
    let nn = n.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        let nn = nn.clone();
        Column("col", move || {
            Text("header");
            Items(nn.clone());
            Text("footer");
        });
    }));
    let col = rule.root.find("col").unwrap();
    assert_eq!(col.child_names(), ["header", "item0", "footer"]);
    n.set(3);
    rule.wait_for_idle();
    assert_eq!(col.child_names(), ["header", "item0", "item1", "item2", "footer"]);
    n.set(0);
    rule.wait_for_idle();
    assert_eq!(col.child_names(), ["header", "footer"]);
}

#[derive(Clone)]
struct Held(Rc<i32>);
impl PartialEq for Held {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
static LocalHeld: KompozitionLocal<Option<Held>> = KompozitionLocal::new(|| None);

fn a_restarted_scope_leaves_nothing_provided_held() {
    // A scope run on its own gets the locals provided above it; once it ran, the composer keeps none of them (a
    // nav entry's ViewModelStore lived on, past the host's dispatcher).
    let held = Rc::new(7);
    let weak = Rc::downgrade(&held);
    let holder = Rc::new(RefCell::new(Some(held)));
    let count = mutable_state_of(0);
    let shown = mutable_state_of(true);
    let (h, c, s) = (holder.clone(), count.clone(), shown.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        if s.get() {
            let held = h.borrow().clone().map(Held);
            let c = c.clone();
            KompozitionLocalProvider(&[LocalHeld.provides(held)], move || CounterView(c.clone()));
        }
    }));
    holder.borrow_mut().take();
    count.set(1); // only CounterView runs again
    rule.wait_for_idle();
    assert!(rule.has("count=1") && weak.upgrade().is_some());
    shown.set(false);
    rule.wait_for_idle();
    assert!(weak.upgrade().is_none());
}

#[kompozable]
fn Toggle(show: MutableState<bool>, log: Rc<RefCell<Vec<String>>>) {
    if show.get() {
        let log = log.clone();
        DisposableEffect(0, move || {
            log.borrow_mut().push("on".into());
            on_dispose(move || log.borrow_mut().push("off".into()))
        });
    }
    Text(if show.get() { "shown" } else { "hidden" });
}

fn restarted_scope_disposes_what_it_no_longer_calls() {
    let show = mutable_state_of(true);
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let root_runs = Rc::new(Cell::new(0));
    let (s, l, r) = (show.clone(), log.clone(), root_runs.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        r.set(r.get() + 1);
        Toggle(s.clone(), l.clone());
    }));
    show.set(false);
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["on", "off"]);
    assert!(rule.has("hidden"));
    assert_eq!(root_runs.get(), 1);
}

fn state_set_from_another_thread() {
    let count = mutable_state_of(0);
    let c = count.clone();
    let rule = Rule::new(kompozable_lambda!(move || CounterView(c.clone())));
    let c = count.clone();
    std::thread::spawn(move || c.set(5)).join().unwrap();
    rule.wait_for_idle();
    assert!(rule.has("count=5"));
}

fn frame_scopes_recompose_only_themselves() {
    // What an animation does: its scope asks for the next frames until it is done.
    let root_runs = Rc::new(Cell::new(0));
    let frames = Rc::new(Cell::new(0));
    let (r, f) = (root_runs.clone(), frames.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        r.set(r.get() + 1);
        let f = f.clone();
        scoped(move || {
            f.set(f.get() + 1);
            if f.get() < 5 {
                request_frame();
            }
            Text(if f.get() >= 5 { "done" } else { "moving" });
        });
    }));
    assert!(rule.has("done"));
    assert_eq!(root_runs.get(), 1);
    assert_eq!(frames.get(), 5);
}

fn derived_state_recomposes_only_when_its_value_changes() {
    let count = mutable_state_of(0);
    let runs = Rc::new(Cell::new(0));
    let (c, r) = (count.clone(), runs.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let c = c.clone();
        let big = remember(|| derived_state_of(move || c.get() > 3));
        let r = r.clone();
        scoped(move || {
            r.set(r.get() + 1);
            Text(if big.get() { "big" } else { "small" });
        });
    }));
    for i in 1..=3 {
        count.set(i);
        rule.wait_for_idle();
    }
    assert!(runs.get() == 1 && rule.has("small"));
    count.set(4);
    rule.wait_for_idle();
    assert!(runs.get() == 2 && rule.has("big"));
    count.set(5);
    rule.wait_for_idle();
    assert_eq!(runs.get(), 2);
}

/// Compose's DerivedSnapshotState: a derived state made in a composition, read right after a write (before the
/// composition took it), from a coroutine or outside any reader, is computed again then — and only then: read
/// again with nothing written it isn't, nor when the composition takes the write it already saw.
fn derived_state_read_after_a_write_is_up_to_date() {
    let count = mutable_state_of(1);
    let computed = Rc::new(Cell::new(0));
    let runs = Rc::new(Cell::new(0));
    let held: Rc<RefCell<Option<DerivedState<i32>>>> = Rc::new(RefCell::new(None));
    let (c, k, r, h) = (count.clone(), computed.clone(), runs.clone(), held.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let (c, k) = (c.clone(), k.clone());
        let doubled = remember(|| {
            derived_state_of(move || {
                k.set(k.get() + 1);
                c.get() * 2
            })
        });
        *h.borrow_mut() = Some(doubled.clone());
        let r = r.clone();
        scoped(move || {
            r.set(r.get() + 1);
            Text(format!("doubled={}", doubled.get()));
        });
    }));
    let doubled = held.borrow().clone().unwrap();
    assert!(rule.has("doubled=2"));
    assert_eq!((computed.get(), runs.get()), (1, 1));
    // Read again, nothing written: kept.
    assert_eq!(doubled.get(), 2);
    assert_eq!(computed.get(), 1);
    // Written: the next read sees it at once (computed once), the reader still told.
    count.set(5);
    assert_eq!(doubled.get(), 10);
    assert_eq!(doubled.peek(), 10);
    assert_eq!(computed.get(), 2);
    rule.wait_for_idle();
    assert!(rule.has("doubled=10"));
    // The composition took the write the read had seen: not computed again, its reader recomposed once.
    assert_eq!((computed.get(), runs.get()), (2, 2));
    // A write equal in value changes nothing for it; another state written doesn't make it compute.
    let other = mutable_state_of(0);
    other.set(1);
    assert_eq!(doubled.get(), 10);
    assert_eq!(computed.get(), 2);
    // Written in a snapshot: seen there, not outside until applied.
    let snapshot = Snapshot::take_mutable_snapshot(None, None);
    snapshot.enter(|| {
        count.set(7);
        assert_eq!(doubled.get(), 14);
    });
    assert_eq!(doubled.get(), 10);
    assert_eq!(snapshot.apply(), SnapshotApplyResult::Success);
    snapshot.dispose();
    assert_eq!(doubled.get(), 14);
    rule.wait_for_idle();
    assert!(rule.has("doubled=14"));
}

/// A derived state reading another: both brought up to date by one read after a write; a derived state of a
/// policy that never counts values the same tells its readers once per change, not again when the composition
/// takes it; one made outside a composition computes at every read, and a derived state reading it follows what
/// it reads.
fn derived_states_of_derived_states_follow_writes() {
    let count = mutable_state_of(1);
    let changes = Rc::new(Cell::new(0));
    let held: Rc<RefCell<Option<(DerivedState<i32>, DerivedState<i32>, DerivedState<i32>)>>> =
        Rc::new(RefCell::new(None));
    let (c, ch, h) = (count.clone(), changes.clone(), held.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let c = c.clone();
        let (inner, outer, every) = remember(|| {
            let c2 = c.clone();
            let inner = derived_state_of(move || c2.get() + 1);
            let i = inner.clone();
            let outer = derived_state_of(move || i.get() * 10);
            let o = outer.clone();
            let every = derived_state_of_with_policy(never_equal_policy(), move || o.get() / 10);
            (inner, outer, every)
        });
        *h.borrow_mut() = Some((inner.clone(), outer.clone(), every.clone()));
        let ch = ch.clone();
        scoped(move || {
            ch.set(ch.get() + 1);
            Text(format!("every={}", every.get()));
        });
    }));
    let (inner, outer, every) = held.borrow().clone().unwrap();
    assert_eq!((inner.get(), outer.get(), every.get()), (2, 20, 2));
    count.set(4);
    assert_eq!(outer.get(), 50);
    assert_eq!(inner.peek(), 5);
    assert_eq!(every.get(), 5);
    rule.wait_for_idle();
    assert!(rule.has("every=5"));
    assert_eq!(changes.get(), 2);
    rule.wait_for_idle();
    assert_eq!(changes.get(), 2);

    // Made outside a composition: computed at every read; one made in a composition reading it follows it.
    let base = mutable_state_of(3);
    let b = base.clone();
    let loose = derived_state_of(move || b.get() * 3);
    assert_eq!(loose.get(), 9);
    let l = loose.clone();
    let tracked: Rc<RefCell<Option<DerivedState<i32>>>> = Rc::new(RefCell::new(None));
    let t = tracked.clone();
    let _rule2 = Rule::new(kompozable_lambda!(move || {
        let l = l.clone();
        let d = remember(|| derived_state_of(move || l.get() + 1));
        *t.borrow_mut() = Some(d.clone());
        Text(format!("tracked={}", d.get()));
    }));
    let tracked = tracked.borrow().clone().unwrap();
    assert_eq!(tracked.get(), 10);
    base.set(4);
    assert_eq!(loose.get(), 12);
    assert_eq!(tracked.get(), 13);
}

fn state_list_recomposes_on_changes() {
    let list = mutable_state_list_of(["a".to_string()]);
    let l = list.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        for item in l.to_vec() {
            Text(item);
        }
    }));
    list.add("b".into());
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["a", "b"]);
    list.add_at(0, "z".into());
    list.remove_at(2);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["z", "a"]);
    list.set(1, "y".into());
    list.retain(|s| s != "z");
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["y"]);
    list.clear();
    rule.wait_for_idle();
    assert!(rule.names().is_empty() && list.is_empty());
}

fn state_maps_recompose_their_readers() {
    let likes = mutable_state_map_of::<String, i32>([]);
    let runs = Rc::new(Cell::new(0));
    let (l, r) = (likes.clone(), runs.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        r.set(r.get() + 1);
        Text(format!("likes={}", l.get_or_default(&"a".to_string(), 0)));
    }));
    assert_eq!(likes.put("a".into(), 1), None);
    rule.wait_for_idle();
    assert!(rule.has("likes=1") && runs.get() == 2);
    assert_eq!(likes.put("a".into(), 1), Some(1)); // the same value: nobody recomposes
    rule.wait_for_idle();
    assert_eq!(runs.get(), 2);
    assert_eq!(likes.remove(&"b".into()), None); // nothing there: nobody recomposes
    rule.wait_for_idle();
    assert_eq!(runs.get(), 2);
    likes.put_all([("b".to_string(), 2), ("a".to_string(), 3)]);
    rule.wait_for_idle();
    assert!(rule.has("likes=3") && likes.len() == 2 && likes.contains_key(&"b".into()));
    assert_eq!(likes.remove(&"a".into()), Some(3));
    rule.wait_for_idle();
    assert!(rule.has("likes=0"));
}

fn remember_updated_state_gives_effects_the_latest_value() {
    let value = mutable_state_of(1);
    let latest = Rc::new(RefCell::new(None::<State<i32>>));
    let starts = Rc::new(Cell::new(0));
    let (v, l, s) = (value.clone(), latest.clone(), starts.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let updated = remember_updated_state!(v.get());
        let (l, s) = (l.clone(), s.clone());
        DisposableEffect((), move || {
            // started once, reads the newest value later
            s.set(s.get() + 1);
            *l.borrow_mut() = Some(updated);
            on_dispose(|| {})
        });
    }));
    value.set(2);
    rule.wait_for_idle();
    value.set(3);
    rule.wait_for_idle();
    assert!(starts.get() == 1 && latest.borrow().as_ref().unwrap().get() == 3);
}

#[kompozable]
fn UpdatedReader(value: State<i32>, runs: Rc<Cell<u32>>) {
    runs.set(runs.get() + 1);
    Text(format!("read {}", value.get()));
}

/// Compose's `rememberUpdatedState` is a `mutableStateOf` set at every composition: a composition reading it (a
/// child scope, the scope that set it) and a drawing's or layout's observer (a SnapshotStateObserver) are told when
/// it is set to an unequal value, not when the composition that sets it runs again with an equal one; equality is
/// Kotlin's `==` (a callback `Rc` equal only to itself).
fn remember_updated_state_tells_its_readers_of_unequal_values_only() {
    let (value, other) = (mutable_state_of(1), mutable_state_of(0));
    let shared: Rc<dyn Fn() -> i32> = Rc::new(|| 7);
    let use_shared = mutable_state_of(false);
    type Kept = (State<i32>, State<Rc<dyn Fn() -> i32>>);
    let kept = Rc::new(RefCell::new(None::<Kept>));
    let (reader_runs, writer_runs) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (v, o, u, sh, k, rr, wr) = (
        value.clone(),
        other.clone(),
        use_shared.clone(),
        shared.clone(),
        kept.clone(),
        reader_runs.clone(),
        writer_runs.clone(),
    );
    let rule = Rule::new(kompozable_lambda!(move || {
        wr.set(wr.get() + 1);
        o.get(); // (what makes it compose again with an equal value)
        let updated = remember_updated_state!(v.get());
        let callback = if u.get() { sh.clone() } else { Rc::new(|| 0) as Rc<dyn Fn() -> i32> };
        let latest_callback = remember_updated_state!(callback);
        *k.borrow_mut() = Some((updated.clone(), latest_callback));
        Text(format!("set {}", updated.get())); // the scope that set it reads it too
        UpdatedReader(updated, rr.clone());
    }));
    let (updated, latest_callback) = kept.borrow().clone().unwrap();
    // A drawing's reads, observed as a layout's or a drawing's are.
    let observer = SnapshotStateObserver::new(|deliver| deliver());
    observer.start();
    let told = Rc::new(RefCell::new(Vec::<&'static str>::new()));
    let t = told.clone();
    let on_changed: Rc<dyn Fn(&&'static str)> = Rc::new(move |scope| t.borrow_mut().push(*scope));
    let draw = || {
        observer.observe_reads("value", on_changed.clone(), || updated.get());
        observer.observe_reads("callback", on_changed.clone(), || latest_callback.get()());
    };
    draw();
    assert!(rule.has("read 1") && rule.has("set 1") && reader_runs.get() == 1);

    // An unequal value: the readers told, the child read again, the scope that set it settled.
    value.set(2);
    rule.wait_for_idle();
    assert!(rule.has("read 2") && rule.has("set 2") && reader_runs.get() == 2);
    assert!(told.borrow().contains(&"value"));
    told.borrow_mut().clear();
    draw();

    // The same value again (the setter composed again): no one told.
    let writes = writer_runs.get();
    other.set(1);
    rule.wait_for_idle();
    assert!(writer_runs.get() > writes && reader_runs.get() == 2);
    assert!(!told.borrow().contains(&"value"));
    // (a new callback instance at each composition: never equal to the last one, as an unequal lambda in Compose)
    assert!(told.borrow().contains(&"callback"));

    // The same callback instance each time: equal to itself.
    use_shared.set(true);
    rule.wait_for_idle();
    told.borrow_mut().clear();
    draw();
    assert_eq!((latest_callback.peek())(), 7);
    other.set(2);
    rule.wait_for_idle();
    assert!(told.borrow().is_empty());
    observer.stop();
}

#[kompozable]
fn ReadsItsCallback(callback: Rc<dyn Fn() -> i32>, tick: MutableState<i32>, runs: Rc<Cell<u32>>) {
    runs.set(runs.get() + 1);
    tick.get();
    let latest = remember_updated_state!(callback);
    Text(format!("callback {}", latest.get()()));
}

/// A callback taken as an `Rc` parameter is the same instance when its scope restarts by itself (as a memoized
/// lambda is in Compose): the updated state stays equal, and the scope reading it where it sets it settles.
fn an_updated_callback_parameter_stays_equal_as_its_scope_restarts() {
    let tick = mutable_state_of(0);
    let runs = Rc::new(Cell::new(0));
    let (t, r) = (tick.clone(), runs.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        ReadsItsCallback(Rc::new(|| 5), t.clone(), r.clone());
    }));
    assert!(rule.has("callback 5") && runs.get() == 1);
    tick.set(1);
    rule.wait_for_idle();
    assert_eq!(runs.get(), 2);
}

/// A derived state reading an updated state follows it (the state is a dependency it tracks).
fn derived_state_follows_an_updated_state() {
    let value = mutable_state_of(1);
    let v = value.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        let updated = remember_updated_state!(v.get() * 10);
        let doubled = remember(move || derived_state_of(move || updated.get() * 2));
        Text(format!("doubled {}", doubled.get()));
    }));
    assert!(rule.has("doubled 20"));
    value.set(2);
    rule.wait_for_idle();
    assert!(rule.has("doubled 40"));
}

fn movable_content_keeps_its_state_where_it_moves() {
    let wide = mutable_state_of(false);
    let handles = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let (w, h, l) = (wide.clone(), handles.clone(), log.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let (h, l) = (h.clone(), l.clone());
        let video = remember(move || {
            movable_content_of(move || {
                Item(1, h.clone());
                effect(&l, "video", 0);
            })
        });
        if w.get() {
            let v = video.clone();
            Column("row", move || {
                Text("details");
                v.call();
            });
        } else {
            let v = video.clone();
            Column("column", move || v.call());
        }
    }));
    click(&handles, 1);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["column", "item1:1"]);
    wide.set(true);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["row", "details", "item1:1"]); // its remembered count came along
    assert_eq!(*log.borrow(), ["+video0"]); // and its effect kept running
    wide.set(false);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["column", "item1:1"]);
    assert_eq!(*log.borrow(), ["+video0"]);
}

fn movable_content_leaves_a_composition_it_isnt_called_in() {
    let show = mutable_state_of(true);
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let (s, l) = (show.clone(), log.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let l = l.clone();
        let content = remember(move || movable_content_of(move || effect(&l, "m", 0)));
        if s.get() {
            content.call();
        }
    }));
    show.set(false);
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["+m0", "-m0"]);
}

static LocalHost: KompozitionLocal<Option<String>> = KompozitionLocal::new(|| None);

/// A scope inside movable content that reads the host's local (as LaunchedEffect reads LocalDispatcher).
#[kompozable]
fn HostReader(tone: MutableState<i32>) {
    let host = LocalHost.current().expect("the host's local, provided at the root");
    Text(format!("{host}:{tone}"));
}

/// Movable content made anew (`remember(content) { movableContentOf(content) }` with another content) leaves its
/// old group parked; a scope in that group invalidated in the same pass isn't run on its own, without the locals
/// above it (the WP9 showcase's NavigationSuiteScaffold, whose items' content changes with the theme).
fn movable_content_made_anew_doesnt_run_the_old_ones_scopes() {
    let version = mutable_state_of(0);
    let tone = mutable_state_of(0);
    let (v, t) = (version.clone(), tone.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let (v, t) = (v.clone(), t.clone());
        KompozitionLocalProvider(&[LocalHost.provides(Some("host".into()))], move || {
            let n = v.get();
            let t = t.clone();
            let content = remember_keyed(n, move || movable_content_of(move || HostReader(t.clone())));
            Column("box", move || content.call());
        });
    }));
    assert_eq!(rule.names(), ["box", "host:0"]);
    version.set(1);
    tone.set(1);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["box", "host:1"]);
}

fn movable_content_called_twice_makes_a_second_instance() {
    let handles = Rc::new(RefCell::new(Vec::new()));
    let h = handles.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        let h = h.clone();
        let content = remember(move || movable_content_with_param_of(move |id: &i32| Item(*id, h.clone())));
        content.call(1);
        content.call(2);
    }));
    click(&handles, 2);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["item1:0", "item2:1"]);
}

fn remember_saveable_comes_back_with_its_holders_key() {
    let tab = mutable_state_of("a".to_string());
    let handles = Rc::new(RefCell::new(Vec::<(String, MutableState<i32>)>::new()));
    let (t, h) = (tab.clone(), handles.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let holder = remember_saveable_state_holder();
        let key = t.get();
        let h = h.clone();
        holder.SaveableStateProvider(&key.clone(), || {
            let count = remember_saveable(|| mutable_state_of(0));
            h.borrow_mut().push((key.clone(), count.clone()));
            Text(format!("{key}:{count}"));
        });
    }));
    let count_of = |k: &str| handles.borrow().iter().rev().find(|(t, _)| t == k).unwrap().1.clone();
    count_of("a").set(5);
    rule.wait_for_idle();
    assert!(rule.has("a:5"));
    tab.set("b".into());
    rule.wait_for_idle();
    assert!(rule.has("b:0"));
    tab.set("a".into()); // back: the value it had
    rule.wait_for_idle();
    assert!(rule.has("a:5"), "{:?}", rule.names());
}

/// The same composable in both hosts: its saved values are found by their place in it.
#[kompozable]
fn SavedText(handles: Rc<RefCell<Vec<MutableState<String>>>>) {
    let text = remember_saveable(|| mutable_state_of(String::new()));
    handles.borrow_mut().push(text.clone());
    Text(format!("text={text}"));
}

fn saved_state_outlives_the_host() {
    let host = |saved: Option<&SavedValue>, handles: Rc<RefCell<Vec<MutableState<String>>>>| {
        let registry = SaveableStateRegistry::new(saved);
        let r = registry.clone();
        (
            registry,
            Rule::new(kompozable_lambda!(move || ProvideSaveableStateRegistry(&r, || SavedText(handles.clone())))),
        )
    };
    let handles = Rc::new(RefCell::new(Vec::new()));
    let saved = {
        let (registry, rule) = host(None, handles.clone());
        handles.borrow()[0].set("hello".into());
        rule.wait_for_idle();
        assert!(rule.has("text=hello"));
        registry.save().encode()
    };
    // A host made again from what the one before saved (the process ended meanwhile).
    let restored = SavedValue::decode(&saved).unwrap();
    let (_, rule) = host(Some(&restored), Rc::new(RefCell::new(Vec::new())));
    assert!(rule.has("text=hello"), "{:?}", rule.names());
}

fn remember_saveable_starts_again_when_its_inputs_change() {
    let input = mutable_state_of(1);
    let handles = Rc::new(RefCell::new(Vec::<MutableState<i32>>::new()));
    let (i, h) = (input.clone(), handles.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let holder = remember_saveable_state_holder();
        let (i, h) = (i.clone(), h.clone());
        holder.SaveableStateProvider("screen", move || {
            let count = remember_saveable_keyed(i.get(), || mutable_state_of(0));
            h.borrow_mut().push(count.clone());
            Text(format!("count={count}"));
        });
    }));
    handles.borrow().last().unwrap().set(3);
    rule.wait_for_idle();
    assert!(rule.has("count=3"));
    input.set(2);
    rule.wait_for_idle();
    assert!(rule.has("count=0"), "{:?}", rule.names());
}

#[derive(Clone, PartialEq, Debug)]
struct City {
    name: String,
    country: String,
}

#[kompozable]
fn CityText(saver: Saver<MutableState<City>>) {
    let city =
        remember_saveable_with(&saver, || mutable_state_of(City { name: "Madrid".into(), country: "Spain".into() }));
    Text(format!("{}/{}", city.get().name, city.get().country));
    if city.peek().name == "Madrid" {
        city.set(City { name: "Izmir".into(), country: "Türkiye".into() });
    }
}

fn remember_saveable_with_a_saver_of_its_own() {
    let saver = map_saver(
        |c: &City| {
            vec![("name".to_string(), c.name.as_str().into()), ("country".to_string(), c.country.as_str().into())]
        },
        |m| Some(City { name: m.get("name")?.as_str()?.into(), country: m.get("country")?.as_str()?.into() }),
    )
    .for_state();
    let host = |saved: Option<&SavedValue>, saver: Saver<MutableState<City>>| {
        let registry = SaveableStateRegistry::new(saved);
        let r = registry.clone();
        (registry, Rule::new(kompozable_lambda!(move || ProvideSaveableStateRegistry(&r, || CityText(saver.clone())))))
    };
    let (registry, rule) = host(None, saver.clone());
    assert!(rule.has("Izmir/Türkiye"));
    let saved = registry.save();
    drop(rule);
    let (_, rule) = host(Some(&saved), saver);
    assert!(rule.has("Izmir/Türkiye"), "{:?}", rule.names());
}

fn values_registered_by_key_are_saved_and_given_back_once() {
    // Compose's SaveableStateRegistry provider API: what isn't a composable's registers a provider by key, asked for
    // its value as the registry saves; a registry restored gives each value back once; an entry unregistered is
    // saved no more; several providers of one key are saved in order.
    let registry = SaveableStateRegistry::new(None);
    let value = Rc::new(Cell::new(7));
    let v = value.clone();
    let entry = registry.register_provider("seçili", move || Some(SavedValue::Int(v.get() as i64)));
    let _none = registry.register_provider("boş", || None);
    let first = registry.register_provider("çift", || Some(SavedValue::String("bir".into())));
    let _second = registry.register_provider("çift", || Some(SavedValue::String("iki".into())));
    value.set(9);
    let saved = registry.perform_save();
    assert!(registry.can_be_saved(&SavedValue::Int(1)));
    let restored = SaveableStateRegistry::new(Some(&saved));
    assert_eq!(restored.consume_restored("seçili"), Some(SavedValue::Int(9)));
    assert_eq!(restored.consume_restored("seçili"), None); // (once)
    assert_eq!(restored.consume_restored("boş"), None); // (a provider giving nothing saved nothing)
    assert_eq!(restored.consume_restored("çift"), Some(SavedValue::String("bir".into())));
    // What wasn't asked for yet is saved again as it was.
    let again = SaveableStateRegistry::new(Some(&restored.save()));
    assert_eq!(again.consume_restored("çift"), Some(SavedValue::String("iki".into())));
    // Unregistered: no longer saved.
    entry.unregister();
    first.unregister();
    let after = SaveableStateRegistry::new(Some(&registry.save()));
    assert_eq!(after.consume_restored("seçili"), None);
    assert_eq!(after.consume_restored("çift"), Some(SavedValue::String("iki".into())));
    // A registry saving only what its check takes.
    let strict = SaveableStateRegistry::with_can_be_saved(None, |v| v.as_int().is_some());
    assert!(strict.can_be_saved(&SavedValue::Int(3)) && !strict.can_be_saved(&SavedValue::Bool(true)));
    // The registry where `remember_saveable` keeps its values is the one provided (Compose's
    // LocalSaveableStateRegistry).
    let host = SaveableStateRegistry::new(None);
    let seen = Rc::new(RefCell::new(None));
    let (h, s) = (host.clone(), seen.clone());
    let _rule = Rule::new(kompozable_lambda!(move || {
        let s = s.clone();
        ProvideSaveableStateRegistry(&h, move || *s.borrow_mut() = LocalSaveableStateRegistry.current());
    }));
    assert!(seen.borrow().as_ref() == Some(&host));
    assert!(quietly(|| host.register_provider("  ", || None)).is_err());
}

/// A saved state list (Compose's `rememberSaveable { mutableStateListOf(…) }`, as the M15 sample
/// SegmentedListItemWithExpansionSample keeps its children's checks).
#[kompozable]
fn SavedChecks(handles: Rc<RefCell<Vec<SnapshotStateList<bool>>>>) {
    let checks = remember_saveable(|| mutable_state_list_of([false, false, false]));
    handles.borrow_mut().push(checks.clone());
    Text(format!("checks={:?}", checks.to_vec()));
}

fn a_state_list_is_saved_and_given_back() {
    let host = |saved: Option<&SavedValue>, handles: Rc<RefCell<Vec<SnapshotStateList<bool>>>>| {
        let registry = SaveableStateRegistry::new(saved);
        let r = registry.clone();
        (
            registry,
            Rule::new(kompozable_lambda!(move || ProvideSaveableStateRegistry(&r, || SavedChecks(handles.clone())))),
        )
    };
    let handles = Rc::new(RefCell::new(Vec::new()));
    let saved = {
        let (registry, rule) = host(None, handles.clone());
        handles.borrow()[0].set(1, true);
        rule.wait_for_idle();
        assert!(rule.has("checks=[false, true, false]"), "{:?}", rule.names());
        registry.save().encode()
    };
    let restored = SavedValue::decode(&saved).unwrap();
    let handles = Rc::new(RefCell::new(Vec::new()));
    let (_, rule) = host(Some(&restored), handles.clone());
    assert!(rule.has("checks=[false, true, false]"), "{:?}", rule.names());
    // (a list of its own again: a change to it recomposes its readers)
    handles.borrow()[0].set(2, true);
    rule.wait_for_idle();
    assert!(rule.has("checks=[false, true, true]"), "{:?}", rule.names());
}

tests!(
    a_state_written_while_composing_reaches_its_readers_further_on_in_the_same_pass,
    a_derived_state_written_while_composing_reaches_its_readers_further_on_in_the_same_pass,
    a_derived_states_readers_further_on_skip_when_its_value_stays,
    confined_states_hold_values_that_arent_send,
    a_confined_state_list_holds_items_that_arent_send,
    a_snapshot_writing_a_confined_state_applies_on_its_thread_only,
    a_confined_states_values_are_freed_on_its_thread,
    an_impl_fn_parameter_is_one_rc_while_its_identity_stays,
    nodes_are_retained_across_recompositions,
    key_keeps_state_with_its_item,
    without_key_state_is_positional,
    disposable_effect_runs_and_cleans_up,
    disposable_effect_restarts_when_key_changes,
    read_only_states_come_from_mutable_and_derived_ones,
    snapshots_see_states_as_they_were_and_apply_their_writes_together,
    snapshots_keep_only_the_records_someone_reads,
    a_composition_sees_other_threads_writes_whole_and_once_applied,
    states_take_a_mutation_policy,
    remember_and_effects_take_two_or_three_keys,
    effects_run_in_composes_order,
    kompozition_local_is_scoped,
    kompozition_locals_from_factories_defaults_and_contexts,
    only_readers_of_a_state_recompose,
    parent_recomposition_skips_unchanged_children,
    changed_argument_reruns_child,
    local_change_reaches_reader_under_skipped_parent,
    dynamic_locals_are_read_live_and_static_ones_compose_their_keepers_again,
    computed_locals_are_computed_where_they_are_read,
    a_computed_local_is_followed_by_its_readers_alone,
    a_context_provided_again_follows_its_provider,
    values_registered_by_key_are_saved_and_given_back_once,
    partial_recomposition_keeps_node_order,
    a_restarted_scope_leaves_nothing_provided_held,
    restarted_scope_disposes_what_it_no_longer_calls,
    state_set_from_another_thread,
    frame_scopes_recompose_only_themselves,
    derived_state_recomposes_only_when_its_value_changes,
    derived_state_read_after_a_write_is_up_to_date,
    derived_states_of_derived_states_follow_writes,
    state_list_recomposes_on_changes,
    state_maps_recompose_their_readers,
    remember_updated_state_gives_effects_the_latest_value,
    remember_updated_state_tells_its_readers_of_unequal_values_only,
    an_updated_callback_parameter_stays_equal_as_its_scope_restarts,
    derived_state_follows_an_updated_state,
    movable_content_keeps_its_state_where_it_moves,
    movable_content_leaves_a_composition_it_isnt_called_in,
    movable_content_made_anew_doesnt_run_the_old_ones_scopes,
    movable_content_called_twice_makes_a_second_instance,
    remember_saveable_comes_back_with_its_holders_key,
    saved_state_outlives_the_host,
    remember_saveable_starts_again_when_its_inputs_change,
    remember_saveable_with_a_saver_of_its_own,
    a_state_list_is_saved_and_given_back,
    a_shared_state_reads_its_state_from_anywhere,
    a_jump_out_of_composable_content_ends_the_groups_it_leaves,
    a_loop_jump_out_of_composable_content_keeps_the_composition_whole,
    a_jump_out_of_providers_and_keys_leaves_their_locals,
);

/// `MutableState::as_shared_state`: a read-only view that is `Send + Sync` as the state is (what a shape or a theme
/// value holds to read a state another part sets): it reads what the state holds, a read in a composition
/// subscribes it, it is equal to another view of the same state only.
fn a_shared_state_reads_its_state_from_anywhere() {
    fn send_sync<T: Send + Sync>(_: &T) {}
    let state = mutable_state_of(1);
    let shared = state.as_shared_state();
    send_sync(&shared);
    assert!(shared.get() == 1 && shared.peek() == 1 && shared.with(|v| *v + 1) == 2);
    assert!(shared == state.as_shared_state() && shared != mutable_state_of(1).as_shared_state());
    let s = shared.clone();
    let rule = Rule::new(kompozable_lambda!(move || Text(format!("v={}", s.get()))));
    assert_eq!(rule.names(), ["v=1"]);
    state.set(5);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["v=5"]);
    assert!(shared.as_state().get() == 5 && format!("{shared:?}") == "SharedState(5)");
}

/// Compose's `mutableStateOf` holds any object: `mutable_state_of!` keeps a value that isn't `Send` (an `Rc`, a
/// `Job`, an `Animatable`) in a state confined to this thread — kept per snapshot as any state, its readers
/// recomposed when it changes, equal values (Kotlin's `==`: an `Rc`'s instance) changing nothing.
fn confined_states_hold_values_that_arent_send() {
    let shared = mutable_state_of!(1);
    let label = mutable_state_of!(Rc::new("a".to_string()));
    let optional = mutable_state_of!(None::<Rc<Cell<i32>>>);
    assert!(!shared.is_confined() && label.is_confined() && optional.is_confined());
    let runs = Rc::new(Cell::new(0));
    let (l, r) = (label.clone(), runs.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        r.set(r.get() + 1);
        Text(format!("label={}", l.get()));
    }));
    assert!(rule.has("label=a"));
    let b = Rc::new("b".to_string());
    label.set(b.clone());
    rule.wait_for_idle();
    assert!(rule.has("label=b") && runs.get() == 2);
    label.set(b); // the same instance: no change
    rule.wait_for_idle();
    assert_eq!(runs.get(), 2);
    // A snapshot's writes are its own until applied, and dropped with it.
    let cell = Rc::new(Cell::new(7));
    let snapshot = Snapshot::take_mutable_snapshot(None, None);
    snapshot.enter(|| optional.set(Some(cell.clone())));
    assert!(optional.get().is_none() && snapshot.enter(|| optional.get().is_some()));
    snapshot.dispose();
    assert!(optional.get().is_none());
    Snapshot::with_mutable_snapshot(|| optional.set(Some(cell.clone()))).unwrap();
    assert_eq!(optional.get().map(|c| c.get()), Some(7));
    // A policy given: a state of `Rc`s compared by value.
    let by_value = mutable_state_of!(Rc::new(1), SnapshotMutationPolicy::new(|a: &Rc<i32>, b| a == b));
    by_value.set(Rc::new(1));
    assert_eq!(by_value.record_count(), 1);
}

/// Compose's `mutableStateListOf` holds any objects: a list confined to this thread holds items that aren't `Send`
/// (`Rc`s), its readers recomposed on each change, a snapshot's changes its own until applied.
fn a_confined_state_list_holds_items_that_arent_send() {
    let list = mutable_confined_state_list_of([Rc::new("a".to_string())]);
    assert!(list.is_confined() && !mutable_state_list_of([1]).is_confined());
    let l = list.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        for item in l.to_vec() {
            Text((*item).clone());
        }
    }));
    assert_eq!(rule.names(), ["a"]);
    list.add(Rc::new("b".to_string()));
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["a", "b"]);
    let snapshot = Snapshot::take_mutable_snapshot(None, None);
    snapshot.enter(|| list.remove_at(0));
    assert!(list.len() == 2 && snapshot.enter(|| list.len()) == 1);
    assert!(snapshot.apply() == SnapshotApplyResult::Success);
    snapshot.dispose();
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["b"]);
}

/// A confined state's values stay on its thread: a snapshot that wrote one can't be applied on another (it would
/// copy or compare them there) — it panics there; the thread that wrote it applies it.
fn a_snapshot_writing_a_confined_state_applies_on_its_thread_only() {
    let state = mutable_confined_state_of(Rc::new(0));
    let other = mutable_state_of(0);
    let snapshot = Snapshot::take_mutable_snapshot(None, None);
    snapshot.enter(|| state.set(Rc::new(1)));
    // (written meanwhile in the global snapshot: applying compares the values)
    let one = Rc::new(1);
    state.set(one.clone());
    let elsewhere = snapshot.clone();
    let applied = std::thread::spawn(move || elsewhere.apply()).join();
    assert!(applied.is_err(), "applied on another thread");
    // Another thread may still apply a snapshot of shared states only.
    let shared_only = Snapshot::take_mutable_snapshot(None, None);
    shared_only.enter(|| other.set(5));
    let applied = std::thread::spawn(move || shared_only.apply()).join().unwrap();
    assert!(applied == SnapshotApplyResult::Success && other.get() == 5);
    snapshot.dispose();
    assert!(Rc::ptr_eq(&state.get(), &one));
}

/// What drops a value, and on which thread.
struct DropTracer(Rc<RefCell<Vec<std::thread::ThreadId>>>);
impl Clone for DropTracer {
    fn clone(&self) -> Self {
        DropTracer(self.0.clone())
    }
}
impl PartialEq for DropTracer {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}
impl Drop for DropTracer {
    fn drop(&mut self) {
        self.0.borrow_mut().push(std::thread::current().id());
    }
}

/// The records of a confined state no one reads any more are freed between frames on its thread, and when its last
/// handle goes on another thread (a snapshot's list of what it wrote), its values wait there for its thread.
fn a_confined_states_values_are_freed_on_its_thread() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let me = std::thread::current().id();
    let tracer = DropTracer(drops.clone());
    let state = mutable_confined_state_of(tracer.clone());
    for _ in 0..3 {
        state.set(tracer.clone());
        Snapshot::send_apply_notifications();
    }
    assert!(state.record_count() <= 2, "{} records", state.record_count());
    assert!(drops.borrow().iter().all(|t| *t == me));
    // The last handle in a snapshot dropped on another thread.
    let snapshot = Snapshot::take_mutable_snapshot(None, None);
    snapshot.enter(|| state.set(tracer.clone()));
    drop(state);
    let before = drops.borrow().len();
    std::thread::spawn(move || drop(snapshot)).join().unwrap();
    assert_eq!(drops.borrow().len(), before, "values dropped on another thread");
    Snapshot::send_apply_notifications();
    assert!(drops.borrow().len() > before && drops.borrow().iter().all(|t| *t == me));
}

#[kompozable]
fn ShowsShared(label: &'static str, shared: MutableState<i32>, runs: Rc<Cell<u32>>) {
    runs.set(runs.get() + 1);
    Text(format!("{label} {}", shared.get()));
}

#[kompozable]
fn SetsShared(value: i32, shared: MutableState<i32>) {
    shared.set(value);
}

/// A state written while composing (a menu setting the transition state its popup's content reads, Compose's
/// `recordWriteOf`): its readers further on — skippable ones too — are composed in the same pass with what was
/// written (Compose's imminent invalidations), once; those composed (or skipped) before it in the pass compose
/// again in the next (Compose's: the next frame).
fn a_state_written_while_composing_reaches_its_readers_further_on_in_the_same_pass() {
    let (target, shared) = (mutable_state_of(0), mutable_state_of(0));
    let (before, after) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (t, sh, b, a) = (target.clone(), shared.clone(), before.clone(), after.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        ShowsShared("before", sh.clone(), b.clone());
        SetsShared(t.get(), sh.clone());
        ShowsShared("after", sh.clone(), a.clone());
    }));
    assert!(rule.has("before 0") && rule.has("after 0"));
    assert_eq!((before.get(), after.get()), (1, 1));
    target.set(1);
    Snapshot::send_apply_notifications();
    assert!(rule.composition().recompose(0)); // one pass
    assert!(rule.has("after 1"), "{:?}", rule.names()); // further on: in this pass
    assert!(rule.has("before 0"), "{:?}", rule.names()); // skipped before the write: the next pass
    assert_eq!((before.get(), after.get()), (1, 2));
    assert!(rule.composition().has_invalidations() || global_snapshot_changed());
    rule.wait_for_idle();
    assert!(rule.has("before 1") && rule.has("after 1"), "{:?}", rule.names());
    assert_eq!((before.get(), after.get()), (2, 2)); // (the one further on not again: it composed with it)
}

#[kompozable]
fn ShowsDerived<T: std::fmt::Display + Clone + 'static>(
    label: &'static str,
    derived: DerivedState<T>,
    runs: Rc<Cell<u32>>,
) {
    runs.set(runs.get() + 1);
    Text(format!("{label} {}", derived.get()));
}

/// A derived state whose input is written while composing (Compose's `recordWriteOf`: `derivedStates.forEachScopeOf`
/// the value written): its readers further on are invalidated conditionally at once — composed in the same pass
/// when its value changed, a derived state of a derived state too —; one composed before the write composes again
/// with the new value, once.
fn a_derived_state_written_while_composing_reaches_its_readers_further_on_in_the_same_pass() {
    let (target, shared) = (mutable_state_of(0), mutable_state_of(0));
    let (before, after, nested) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (t, sh, b, a, n) = (target.clone(), shared.clone(), before.clone(), after.clone(), nested.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let s = sh.clone();
        let positive = remember(|| derived_state_of(move || s.get() > 0));
        let p = positive.clone();
        let tens = remember(|| derived_state_of(move || if p.get() { 10 } else { 0 }));
        ShowsDerived("before", positive.clone(), b.clone());
        SetsShared(t.get(), sh.clone());
        ShowsDerived("after", positive.clone(), a.clone());
        ShowsDerived("tens", tens.clone(), n.clone());
    }));
    assert!(rule.has("before false") && rule.has("after false") && rule.has("tens 0"));
    target.set(1);
    Snapshot::send_apply_notifications();
    assert!(rule.composition().recompose(0)); // one pass
    assert!(rule.has("after true"), "{:?}", rule.names()); // further on: in this pass
    assert!(rule.has("tens 10"), "{:?}", rule.names()); // (through the derived state it reads)
    assert_eq!((after.get(), nested.get()), (2, 2));
    rule.wait_for_idle();
    assert!(rule.has("before true") && rule.has("after true") && rule.has("tens 10"), "{:?}", rule.names());
    assert_eq!((before.get(), after.get(), nested.get()), (2, 2, 2)); // each once more
}

/// Readers of a derived state whose input is written while composing, its value the same: invalidated only on the
/// condition it changed (Compose's conditional scopes, `isInvalidFor`), they are skipped — nor are they composed
/// again when the pass is applied.
fn a_derived_states_readers_further_on_skip_when_its_value_stays() {
    let (target, shared) = (mutable_state_of(0), mutable_state_of(0));
    let (before, after) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (t, sh, b, a) = (target.clone(), shared.clone(), before.clone(), after.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let s = sh.clone();
        let non_negative = remember(|| derived_state_of(move || s.get() >= 0));
        ShowsDerived("before", non_negative.clone(), b.clone());
        SetsShared(t.get(), sh.clone());
        ShowsDerived("after", non_negative.clone(), a.clone());
    }));
    assert!(rule.has("before true") && rule.has("after true"));
    target.set(1);
    Snapshot::send_apply_notifications();
    assert!(rule.composition().recompose(0));
    rule.wait_for_idle();
    assert_eq!(shared.get(), 1);
    assert!(!rule.composition().has_invalidations());
    assert_eq!((before.get(), after.get()), (1, 1)); // neither composed again
}

#[kompozable]
fn ReadsItsImplCallback(callback: impl Fn() -> i32, tick: i32, runs: Rc<Cell<u32>>, reads: MutableState<i32>) {
    runs.set(runs.get() + 1);
    reads.get();
    // (`Rc::new` of the parameter is the one `Rc` it is kept as while its identity stays the same)
    let latest = remember_updated_state!(Rc::new(callback) as Rc<dyn Fn() -> i32>);
    Text(format!("callback {} at {tick}", latest.get()()));
}

/// An `impl Fn` parameter wrapped in an `Rc` in the body (what a `remember_updated_state!` or a node is given) is
/// one `Rc` while the closure's identity stays the same — the scope restarting by itself, or its caller composing it
/// again with a closure capturing the same values (Compose's memoized lambda, the same instance) —: the updated
/// state read where it is set doesn't change, and composing settles (it used to recompose forever: "never idle");
/// a closure capturing another value is a new one.
fn an_impl_fn_parameter_is_one_rc_while_its_identity_stays() {
    let (tick, value, reads) = (mutable_state_of(0), mutable_state_of(5), mutable_state_of(0));
    let runs = Rc::new(Cell::new(0));
    let (t, v, rd, r) = (tick.clone(), value.clone(), reads.clone(), runs.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let v = v.get();
        ReadsItsImplCallback(move || v, t.get(), r.clone(), rd.clone());
    }));
    assert!(rule.has("callback 5 at 0") && runs.get() == 1);
    reads.set(1); // restarts by itself
    rule.wait_for_idle();
    assert_eq!(runs.get(), 2);
    tick.set(1); // its caller again, the same closure (it captures the same value)
    rule.wait_for_idle();
    assert!(rule.has("callback 5 at 1") && runs.get() == 3, "{}", runs.get());
    value.set(6); // another closure: a change of the state its scope read, which runs it once more (Compose's
    // composer counts a scope being composed as invalidated "imminently": not again)
    rule.wait_for_idle();
    assert!(rule.has("callback 6 at 1") && runs.get() == 5, "{}", runs.get());
}

// ---- Jumps out of composable lambdas (Compose's `currentMarker` / `endToMarker`) ----------------------------------

/// A node whose content runs in its caller's scope (an inline composable, as Compose's `Column`).
#[kompozable(inline)]
fn Boxed(name: &'static str, content: impl FnOnce()) {
    emit(|| TestNode::new(name), |n| n.compositions.set(n.compositions.get() + 1), content);
}

/// Kotlin's `Box("box") { remember a; CounterView; if (stop) return; remember b; Text("second") }; Text("tail")`,
/// as the Kotlin syntax writes it: `kt::leave` in the lambda, `kt::leaving` around the call.
#[kompozable]
fn LeavesItsBox(stop: bool, count: MutableState<i32>, log: Rc<RefCell<Vec<String>>>) {
    let slot = &kt::Leave::new();
    match kt::leaving(slot, || {
        Boxed("box", || {
            let la = log.clone();
            remember_observer(move || Remembered { log: la, name: "a".into() });
            Text("first");
            CounterView(count.clone());
            if stop {
                kt::leave(slot, kt::Jump::Return(()));
            }
            let lb = log.clone();
            remember_observer(move || Remembered { log: lb, name: "b".into() });
            Text("second");
        })
    }) {
        Ok(()) => {}
        Err(kt::Jump::Return(())) => return,
        Err(_) => unreachable!(),
    }
    Text("tail");
}

/// A `return` out of an inline composable's content (Kotlin's `Column { if (…) return … }`): the groups it left are
/// ended where it lands, as Compose's `endToMarker` ends them — the node keeps what was composed before the jump, the
/// values remembered after it are forgotten (and remembered anew when the jump isn't taken again), those before it
/// kept; the composition stays whole: the parent's calls after it, a scope inside restarting on its own.
fn a_jump_out_of_composable_content_ends_the_groups_it_leaves() {
    let stop = mutable_state_of(false);
    let count = mutable_state_of(0);
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    reset(&COUNTER_RUNS);
    let (s, c, l) = (stop.clone(), count.clone(), log.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        LeavesItsBox(s.get(), c.clone(), l.clone());
        Text("sibling");
    }));
    assert_eq!(rule.names(), ["box", "first", "count=0", "second", "tail", "sibling"]);
    assert_eq!(*log.borrow(), ["+a", "+b"]);
    log.borrow_mut().clear();

    stop.set(true);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["box", "first", "count=0", "sibling"]);
    assert_eq!(rule.root.find("box").unwrap().child_names(), ["first", "count=0"]);
    assert_eq!(*log.borrow(), ["-b"]);
    log.borrow_mut().clear();

    // (the scope before the jump runs again on its own; then the parent again, jumping again)
    count.set(1);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["box", "first", "count=1", "sibling"]);
    assert_eq!(runs(&COUNTER_RUNS), 2);
    stop.set(true);
    count.set(2);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["box", "first", "count=2", "sibling"]);
    assert!(log.borrow().is_empty());

    stop.set(false);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["box", "first", "count=2", "second", "tail", "sibling"]);
    assert_eq!(*log.borrow(), ["+b"]); // `a` kept all along
    assert!(!rule.composition().has_invalidations());
}

/// Kotlin's `for (i in 0 until n) { Box("row") { if (i >= limit) break; if (i == skip) continue; Text("r$i") } }`.
#[kompozable]
fn BreaksItsRows(n: i32, limit: i32, skip: i32) {
    for i in 0..n {
        let slot = &kt::Leave::<()>::new();
        match kt::leaving(slot, || {
            Boxed("row", || {
                if i >= limit {
                    kt::leave(slot, kt::Jump::Loop(0));
                }
                if i == skip {
                    kt::leave(slot, kt::Jump::Loop(1));
                }
                Text(format!("r{i}"));
            })
        }) {
            Ok(()) => {}
            Err(kt::Jump::Loop(0)) => break,
            Err(kt::Jump::Loop(1)) => continue,
            Err(_) => unreachable!(),
        }
    }
    Text("end");
}

/// `break`/`continue` out of a node's content in a loop: the node the jump left keeps what it composed (none here),
/// the loop goes on or stops where Kotlin's does, and as the limit changes the rows come and go positionally.
fn a_loop_jump_out_of_composable_content_keeps_the_composition_whole() {
    let limit = mutable_state_of(2);
    let skip = mutable_state_of(-1);
    let (l, s) = (limit.clone(), skip.clone());
    let rule = Rule::new(kompozable_lambda!(move || BreaksItsRows(4, l.get(), s.get())));
    assert_eq!(rule.names(), ["row", "r0", "row", "r1", "row", "end"]);
    limit.set(3);
    skip.set(1);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["row", "r0", "row", "row", "r2", "row", "end"]);
    limit.set(1);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["row", "r0", "row", "end"]);
    limit.set(4);
    skip.set(-1);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["row", "r0", "row", "r1", "row", "r2", "row", "r3", "end"]);
}

/// A jump out of what provides composition locals and reuses content (`CompositionLocalProvider`, `key`,
/// `ReusableContent`, nested node contents): where it lands the locals provided and the reuse begun inside are left
/// too, the code after the call reading the locals of its own place.
fn a_jump_out_of_providers_and_keys_leaves_their_locals() {
    let stop = mutable_state_of(true);
    let s = stop.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        let slot = &kt::Leave::<()>::new();
        let stop = s.get();
        let r = kt::leaving(slot, || {
            Boxed("outer", || {
                KompozitionLocalProvider(&[LocalName.provides("inner".into())], || {
                    key(&7, || {
                        ReusableContent(stop, || {
                            Boxed("inner", || {
                                Text(format!("in {}", LocalName.current()));
                                if stop {
                                    kt::leave(slot, kt::Jump::Loop(0));
                                }
                                Text("rest");
                            });
                        });
                    });
                });
            });
        });
        Text(format!("left={} at {}", r.is_err(), LocalName.current()));
    }));
    assert_eq!(rule.names(), ["outer", "inner", "in inner", "left=true at default"]);
    stop.set(false);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["outer", "inner", "in inner", "rest", "left=false at default"]);
    stop.set(true);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["outer", "inner", "in inner", "left=true at default"]);
}

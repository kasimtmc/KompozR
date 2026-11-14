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

#[track_caller]
fn CounterView(count: MutableState<i32>) {
    restartable(count, Some(MutableState::eq), |count| {
        bump(&COUNTER_RUNS);
        Text(format!("count={count}"));
    });
}

#[track_caller]
fn StaticView() {
    restartable((), Some(|_, _| true), |_| {
        bump(&STATIC_RUNS);
        Text("static");
    });
}

/// A row that remembers its own click count.
#[track_caller]
fn Item(id: i32, clicks_out: Rc<RefCell<Vec<(i32, MutableState<i32>)>>>) {
    group(|| {
        let clicks = remember(|| mutable_state_of(0));
        clicks_out.borrow_mut().push((id, clicks.clone()));
        Text(format!("item{id}:{clicks}"));
    });
}

fn click(handles: &Rc<RefCell<Vec<(i32, MutableState<i32>)>>>, id: i32) {
    let c = handles.borrow().iter().rev().find(|(i, _)| *i == id).unwrap().1.clone();
    c.set(c.peek() + 1);
}

fn nodes_are_retained_across_recompositions() {
    let label = mutable_state_of("a".to_string());
    let l = label.clone();
    let rule = Rule::new(move || Text(l.get()));
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
    let rule = Rule::new(move || {
        let (i, h) = (i.clone(), h.clone());
        Column("column", move || {
            for id in i.get() {
                key(&id, || Item(id, h.clone()));
            }
        });
    });
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
    let rule = Rule::new(move || {
        let (i, h) = (i.clone(), h.clone());
        Column("column", move || {
            for id in i.get() {
                Item(id, h.clone());
            }
        });
    });
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
    let rule = Rule::new(move || {
        if s.get() {
            let l = l.clone();
            DisposableEffect(0, move || {
                l.borrow_mut().push("start".into());
                on_dispose(move || l.borrow_mut().push("stop".into()))
            });
        }
    });
    assert_eq!(*log.borrow(), ["start"]);
    show.set(false);
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["start", "stop"]);
}

fn disposable_effect_restarts_when_key_changes() {
    let k = mutable_state_of(1);
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let (kk, l) = (k.clone(), log.clone());
    let rule = Rule::new(move || {
        let key = kk.get();
        let l = l.clone();
        DisposableEffect(key, move || {
            l.borrow_mut().push(format!("start{key}"));
            on_dispose(move || l.borrow_mut().push(format!("stop{key}")))
        });
    });
    k.set(2);
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["start1", "stop1", "start2"]);
}

/// Takes a state it only reads: a mutable or a derived one.
#[track_caller]
fn Shown(value: State<i32>) {
    restartable(value, Some(State::eq), |value| {
        bump(&SHOWN_RUNS);
        Text(format!("value={value}"));
    });
}

fn read_only_states_come_from_mutable_and_derived_ones() {
    reset(&SHOWN_RUNS);
    let count = mutable_state_of(1);
    let other = mutable_state_of(0);
    let (c, o) = (count.clone(), other.clone());
    let rule = Rule::new(move || {
        o.get(); // the parent runs again, the same states given: they're skipped
        let cc = c.clone();
        let doubled = remember(|| derived_state_of(move || cc.get() * 2));
        let c = c.clone();
        Column("column", move || {
            Shown(c.clone().into());
            Shown(doubled.clone().into());
        });
    });
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
    let rule = Rule::new(move || sn.borrow_mut().push((f.get(), s.get())));
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
    let rule = Rule::new(move || {
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
    });
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
    let rule = Rule::new(move || {
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
    });
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

#[track_caller]
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
    let mut rule = Rule::new(move || {
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
    });
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
    let rule = Rule::new(|| {
        Column("column", || {
            Text(format!("a={}", LocalName.current()));
            KompozitionLocalProvider(&[LocalName.provides("inner".into())], || {
                Text(format!("b={}", LocalName.current()))
            });
            Text(format!("c={}", LocalName.current()));
        });
    });
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
    let rule = Rule::new(move || {
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
    });
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
    let rule = Rule::new(move || {
        r.set(r.get() + 1);
        let c = c.clone();
        Column("column", move || {
            CounterView(c.clone());
            StaticView();
        });
    });
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
    let rule = Rule::new(move || {
        let t = t.clone();
        Column("column", move || {
            Text(format!("tick={t}"));
            Text("fixed");
            StaticView();
        });
    });
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
    let rule = Rule::new(move || CounterView(if w.get() { aa.clone() } else { bb.clone() }));
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

#[track_caller]
fn Middle() {
    restartable((), Some(|_, _| true), |_| {
        bump(&MIDDLE_RUNS);
        Reader();
    });
}

#[track_caller]
fn Reader() {
    restartable((), Some(|_, _| true), |_| {
        bump(&READER_RUNS);
        Text(format!("level={}", LocalLevel.current()));
    });
}

fn local_change_reaches_reader_under_skipped_parent() {
    reset(&MIDDLE_RUNS);
    reset(&READER_RUNS);
    let level = mutable_state_of(1);
    let l = level.clone();
    let rule = Rule::new(move || KompozitionLocalProvider(&[LocalLevel.provides(l.get())], Middle));
    assert!(rule.has("level=1"));
    level.set(2);
    rule.wait_for_idle();
    assert!(rule.has("level=2"));
    assert_eq!(runs(&MIDDLE_RUNS), 1); // skipped: it doesn't read the local
    assert_eq!(runs(&READER_RUNS), 2);
}

#[track_caller]
fn Items(n: MutableState<i32>) {
    restartable(n, Some(MutableState::eq), |n| {
        for i in 0..n.get() {
            Text(format!("item{i}"));
        }
    });
}

fn partial_recomposition_keeps_node_order() {
    let n = mutable_state_of(1);
    let nn = n.clone();
    let rule = Rule::new(move || {
        let nn = nn.clone();
        Column("col", move || {
            Text("header");
            Items(nn.clone());
            Text("footer");
        });
    });
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
    let rule = Rule::new(move || {
        if s.get() {
            let held = h.borrow().clone().map(Held);
            let c = c.clone();
            KompozitionLocalProvider(&[LocalHeld.provides(held)], move || CounterView(c.clone()));
        }
    });
    holder.borrow_mut().take();
    count.set(1); // only CounterView runs again
    rule.wait_for_idle();
    assert!(rule.has("count=1") && weak.upgrade().is_some());
    shown.set(false);
    rule.wait_for_idle();
    assert!(weak.upgrade().is_none());
}

#[track_caller]
fn Toggle(show: MutableState<bool>, log: Rc<RefCell<Vec<String>>>) {
    restartable((show, log), None, |(show, log)| {
        if show.get() {
            let log = log.clone();
            DisposableEffect(0, move || {
                log.borrow_mut().push("on".into());
                on_dispose(move || log.borrow_mut().push("off".into()))
            });
        }
        Text(if show.get() { "shown" } else { "hidden" });
    });
}

fn restarted_scope_disposes_what_it_no_longer_calls() {
    let show = mutable_state_of(true);
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let root_runs = Rc::new(Cell::new(0));
    let (s, l, r) = (show.clone(), log.clone(), root_runs.clone());
    let rule = Rule::new(move || {
        r.set(r.get() + 1);
        Toggle(s.clone(), l.clone());
    });
    show.set(false);
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["on", "off"]);
    assert!(rule.has("hidden"));
    assert_eq!(root_runs.get(), 1);
}

fn state_set_from_another_thread() {
    let count = mutable_state_of(0);
    let c = count.clone();
    let rule = Rule::new(move || CounterView(c.clone()));
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
    let rule = Rule::new(move || {
        r.set(r.get() + 1);
        let f = f.clone();
        scoped(move || {
            f.set(f.get() + 1);
            if f.get() < 5 {
                request_frame();
            }
            Text(if f.get() >= 5 { "done" } else { "moving" });
        });
    });
    assert!(rule.has("done"));
    assert_eq!(root_runs.get(), 1);
    assert_eq!(frames.get(), 5);
}

fn derived_state_recomposes_only_when_its_value_changes() {
    let count = mutable_state_of(0);
    let runs = Rc::new(Cell::new(0));
    let (c, r) = (count.clone(), runs.clone());
    let rule = Rule::new(move || {
        let c = c.clone();
        let big = remember(|| derived_state_of(move || c.get() > 3));
        let r = r.clone();
        scoped(move || {
            r.set(r.get() + 1);
            Text(if big.get() { "big" } else { "small" });
        });
    });
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

fn state_list_recomposes_on_changes() {
    let list = mutable_state_list_of(["a".to_string()]);
    let l = list.clone();
    let rule = Rule::new(move || {
        for item in l.to_vec() {
            Text(item);
        }
    });
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
    let rule = Rule::new(move || {
        r.set(r.get() + 1);
        Text(format!("likes={}", l.get_or_default(&"a".to_string(), 0)));
    });
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
    let latest = Rc::new(RefCell::new(None::<UpdatedState<i32>>));
    let starts = Rc::new(Cell::new(0));
    let (v, l, s) = (value.clone(), latest.clone(), starts.clone());
    let rule = Rule::new(move || {
        let updated = remember_updated_state(v.get());
        let (l, s) = (l.clone(), s.clone());
        DisposableEffect((), move || {
            // started once, reads the newest value later
            s.set(s.get() + 1);
            *l.borrow_mut() = Some(updated);
            on_dispose(|| {})
        });
    });
    value.set(2);
    rule.wait_for_idle();
    value.set(3);
    rule.wait_for_idle();
    assert!(starts.get() == 1 && latest.borrow().as_ref().unwrap().get() == 3);
}

fn movable_content_keeps_its_state_where_it_moves() {
    let wide = mutable_state_of(false);
    let handles = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let (w, h, l) = (wide.clone(), handles.clone(), log.clone());
    let rule = Rule::new(move || {
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
    });
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
    let rule = Rule::new(move || {
        let l = l.clone();
        let content = remember(move || movable_content_of(move || effect(&l, "m", 0)));
        if s.get() {
            content.call();
        }
    });
    show.set(false);
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["+m0", "-m0"]);
}

fn movable_content_called_twice_makes_a_second_instance() {
    let handles = Rc::new(RefCell::new(Vec::new()));
    let h = handles.clone();
    let rule = Rule::new(move || {
        let h = h.clone();
        let content = remember(move || movable_content_with_param_of(move |id: &i32| Item(*id, h.clone())));
        content.call(1);
        content.call(2);
    });
    click(&handles, 2);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["item1:0", "item2:1"]);
}

fn remember_saveable_comes_back_with_its_holders_key() {
    let tab = mutable_state_of("a".to_string());
    let handles = Rc::new(RefCell::new(Vec::<(String, MutableState<i32>)>::new()));
    let (t, h) = (tab.clone(), handles.clone());
    let rule = Rule::new(move || {
        let holder = remember_saveable_state_holder();
        let key = t.get();
        let h = h.clone();
        holder.SaveableStateProvider(&key.clone(), || {
            let count = remember_saveable(|| mutable_state_of(0));
            h.borrow_mut().push((key.clone(), count.clone()));
            Text(format!("{key}:{count}"));
        });
    });
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
#[track_caller]
fn SavedText(handles: Rc<RefCell<Vec<MutableState<String>>>>) {
    let text = remember_saveable(|| mutable_state_of(String::new()));
    handles.borrow_mut().push(text.clone());
    Text(format!("text={text}"));
}

fn saved_state_outlives_the_host() {
    let host = |saved: Option<&SavedValue>, handles: Rc<RefCell<Vec<MutableState<String>>>>| {
        let registry = SaveableStateRegistry::new(saved);
        let r = registry.clone();
        (registry, Rule::new(move || ProvideSaveableStateRegistry(&r, || SavedText(handles.clone()))))
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
    let rule = Rule::new(move || {
        let holder = remember_saveable_state_holder();
        let (i, h) = (i.clone(), h.clone());
        holder.SaveableStateProvider("screen", move || {
            let count = remember_saveable_keyed(i.get(), || mutable_state_of(0));
            h.borrow_mut().push(count.clone());
            Text(format!("count={count}"));
        });
    });
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

#[track_caller]
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
        (registry, Rule::new(move || ProvideSaveableStateRegistry(&r, || CityText(saver.clone()))))
    };
    let (registry, rule) = host(None, saver.clone());
    assert!(rule.has("Izmir/Türkiye"));
    let saved = registry.save();
    drop(rule);
    let (_, rule) = host(Some(&saved), saver);
    assert!(rule.has("Izmir/Türkiye"), "{:?}", rule.names());
}

tests!(
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
    partial_recomposition_keeps_node_order,
    a_restarted_scope_leaves_nothing_provided_held,
    restarted_scope_disposes_what_it_no_longer_calls,
    state_set_from_another_thread,
    frame_scopes_recompose_only_themselves,
    derived_state_recomposes_only_when_its_value_changes,
    state_list_recomposes_on_changes,
    state_maps_recompose_their_readers,
    remember_updated_state_gives_effects_the_latest_value,
    movable_content_keeps_its_state_where_it_moves,
    movable_content_leaves_a_composition_it_isnt_called_in,
    movable_content_called_twice_makes_a_second_instance,
    remember_saveable_comes_back_with_its_holders_key,
    saved_state_outlives_the_host,
    remember_saveable_starts_again_when_its_inputs_change,
    remember_saveable_with_a_saver_of_its_own,
);

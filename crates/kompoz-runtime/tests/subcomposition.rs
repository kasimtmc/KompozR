//! Child compositions, reusable content and paused compositions (Compose's subcomposition, `ReusableComposition`,
//! `ReusableContent`/`ReusableContentHost` and `PausableComposition`), on test nodes.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_runtime::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

/// Tells what it is told, as Compose's RememberObserver.
struct Told {
    log: Log,
    name: &'static str,
}

impl RememberObserver for Told {
    fn on_remembered(&self) {
        self.log.borrow_mut().push(format!("remembered {}", self.name));
    }
    fn on_forgotten(&self) {
        self.log.borrow_mut().push(format!("forgotten {}", self.name));
    }
    fn on_abandoned(&self) {
        self.log.borrow_mut().push(format!("abandoned {}", self.name));
    }
}

/// A context taken where the parent composes, kept for the test to make child compositions from.
type Kept = Rc<RefCell<Option<CompositionContext>>>;

fn kept(k: &Kept) -> CompositionContext {
    k.borrow().clone().expect("a context was taken")
}

static LocalTone: KompozitionLocal<String> = KompozitionLocal::new(|| "none".into());
static LocalSize: KompozitionLocal<i32> = static_kompozition_local_of(|| 0);
static LocalExtra: KompozitionLocal<String> = KompozitionLocal::new(|| "plain".into());

fn child_composition_sees_its_parents_locals() {
    let (tone, size, extra) = (mutable_state_of("warm".to_string()), mutable_state_of(1), mutable_state_of(false));
    let k: Kept = Rc::new(RefCell::new(None));
    let (t, s, e, kk) = (tone.clone(), size.clone(), extra.clone(), k.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let mut values = vec![LocalTone.provides(t.get()), LocalSize.provides(s.get())];
        if e.get() {
            values.push(LocalExtra.provides("given".into()));
        }
        let kk = kk.clone();
        KompozitionLocalProvider(&values, move || {
            *kk.borrow_mut() = Some(remember_kompozition_context());
        });
    }));
    let root = TestNode::new("child");
    let child = Composition::with_parent(Some(root.clone()), &kept(&k));
    assert!(child.is_child());
    child.set_content(kompozable_lambda!(|| {
        Text(format!("{}/{}/{}", LocalTone.current(), LocalSize.current(), LocalExtra.current()))
    }));
    // (composed at once, as a child composition's content is)
    assert_eq!(root.names(), ["warm/1/plain"]);
    tone.set("cool".into()); // a dynamic local: its holder set, read as any state
    rule.wait_for_idle();
    assert_eq!(root.names(), ["cool/1/plain"]);
    size.set(2); // a static local: another holder, its readers in the child told
    rule.wait_for_idle();
    assert_eq!(root.names(), ["cool/2/plain"]);
    extra.set(true); // provided now where it wasn't: the child's reader of its default told
    rule.wait_for_idle();
    assert_eq!(root.names(), ["cool/2/given"]);
    drop(child);
    drop(rule);
}

fn parents_recompose_before_their_children() {
    let n = mutable_state_of(0);
    let order = log();
    let k: Kept = Rc::new(RefCell::new(None));
    let (nn, o, kk) = (n.clone(), order.clone(), k.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        o.borrow_mut().push(format!("parent {}", nn.get()));
        *kk.borrow_mut() = Some(remember_kompozition_context());
    }));
    let root = TestNode::new("child");
    let child = Composition::with_parent(Some(root.clone()), &kept(&k));
    let (nn, o) = (n.clone(), order.clone());
    child.set_content(kompozable_lambda!(move || {
        o.borrow_mut().push(format!("child {}", nn.get()));
        // a grandchild, from a context taken in the child
        let kk = remember_kompozition_context();
        let (nn, o) = (nn.clone(), o.clone());
        let grandchild = remember(move || {
            let g = Rc::new(Composition::with_parent(None, &kk));
            g.set_content(move || o.borrow_mut().push(format!("grandchild {}", nn.get())));
            g
        });
        let _ = grandchild;
    }));
    assert_eq!(taken(&order), ["parent 0", "child 0", "grandchild 0"]);
    n.set(1);
    rule.wait_for_idle();
    assert_eq!(taken(&order), ["parent 1", "child 1", "grandchild 1"]);
    // Deactivated, a child isn't recomposed (nor what was made from it)...
    child.deactivate();
    n.set(2);
    rule.wait_for_idle();
    assert_eq!(taken(&order), ["parent 2"]);
    assert!(!rule.composition().has_invalidations());
    // ...disposed of, it leaves its parent's recomposer.
    child.dispose();
    assert!(child.is_disposed());
    n.set(3);
    rule.wait_for_idle();
    assert_eq!(taken(&order), ["parent 3"]);
}

/// A state one composition writes while composing reaches the compositions recomposed after it in the same frame
/// (Compose's Recomposer: the compositions observing what a composition modified are composed in that frame,
/// unless composed in it already): the parent's write reaches its child at once; the child's write reaches the
/// parent, composed before it, at the next frame.
fn a_state_a_composition_writes_reaches_the_compositions_after_it_in_the_same_frame() {
    let (target, down, up) = (mutable_state_of(0), mutable_state_of(0), mutable_state_of(0));
    let k: Kept = Rc::new(RefCell::new(None));
    let (t, d, u, kk) = (target.clone(), down.clone(), up.clone(), k.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        d.set(t.get()); // written while composing
        Text(format!("parent sees {}", u.get()));
        *kk.borrow_mut() = Some(remember_kompozition_context());
    }));
    let root = TestNode::new("child");
    let child = Composition::with_parent(Some(root.clone()), &kept(&k));
    let (d, u) = (down.clone(), up.clone());
    child.set_content(kompozable_lambda!(move || {
        let v = d.get();
        u.set(v * 10); // written while composing too, read by the parent
        Text(format!("child sees {v}"));
    }));
    rule.wait_for_idle();
    assert_eq!(root.names(), ["child sees 0"]);
    target.set(1);
    // One frame: the parent, then the child with what the parent wrote.
    Snapshot::send_apply_notifications();
    assert!(rule.composition().recompose(0));
    assert_eq!(root.names(), ["child sees 1"]);
    assert!(rule.has("parent sees 0"), "{:?}", rule.names());
    // The next: the parent with what the child wrote.
    Snapshot::send_apply_notifications();
    assert!(rule.composition().recompose(16_000_000));
    assert!(rule.has("parent sees 10"), "{:?}", rule.names());
    rule.wait_for_idle();
    assert_eq!(root.names(), ["child sees 1"]);
    drop(child);
    drop(rule);
}

/// Content of the same shape in two child compositions: the keys continue each one's context, and their saveable
/// values are each their own, saved and given back in the order they were made.
fn child_compositions_continue_their_parents_keys() {
    fn compose(registry: &SaveableStateRegistry, next: Rc<Cell<i32>>) -> (Rule, Vec<Composition>, Vec<Rc<TestNode>>) {
        let k: Rc<RefCell<Vec<CompositionContext>>> = Rc::new(RefCell::new(Vec::new()));
        let (reg, kk) = (registry.clone(), k.clone());
        let rule = Rule::new(kompozable_lambda!(move || {
            let kk = kk.clone();
            ProvideSaveableStateRegistry(&reg, move || {
                kk.borrow_mut().clear();
                for i in 0..2 {
                    key(&i, || kk.borrow_mut().push(remember_kompozition_context()));
                }
            });
        }));
        let contexts = k.borrow().clone();
        let mut children = Vec::new();
        let mut roots = Vec::new();
        // two from the first context, one from the second: all the same shape
        for ctx in [&contexts[0], &contexts[0], &contexts[1]] {
            let root = TestNode::new("child");
            let child = Composition::with_parent(Some(root.clone()), ctx);
            let next = next.clone();
            child.set_content(kompozable_lambda!(move || {
                let next = next.clone();
                let v = remember_saveable(move || {
                    next.set(next.get() + 1);
                    next.get()
                });
                Text(format!("{v}@{:x}", current_composite_key_hash()));
            }));
            children.push(child);
            roots.push(root);
        }
        (rule, children, roots)
    }
    let registry = SaveableStateRegistry::new(None);
    let (rule, children, roots) = compose(&registry, Rc::new(Cell::new(0)));
    let texts: Vec<String> = roots.iter().map(|r| r.names()[0].clone()).collect();
    let values: Vec<&str> = texts.iter().map(|t| t.split('@').next().unwrap()).collect();
    let keys: Vec<&str> = texts.iter().map(|t| t.split('@').nth(1).unwrap()).collect();
    assert_eq!(values, ["1", "2", "3"]);
    assert_eq!(keys[0], keys[1]); // the same place under the same context
    assert_ne!(keys[0], keys[2]); // under another one
    // Recomposed, they keep their keys.
    rule.composition().invalidate_all();
    rule.wait_for_idle();
    let again: Vec<String> = roots.iter().map(|r| r.names()[0].clone()).collect();
    assert_eq!(again, texts);
    // Written out and given back: each its own value.
    let saved = registry.save();
    drop(children);
    drop(rule);
    let restored = SaveableStateRegistry::new(Some(&saved));
    let (_rule, _children, roots) = compose(&restored, Rc::new(Cell::new(100)));
    let values: Vec<String> = roots.iter().map(|r| r.names()[0].split('@').next().unwrap().to_string()).collect();
    assert_eq!(values, ["1", "2", "3"]);
}

fn reusable_content_starts_its_state_over_and_reuses_its_nodes() {
    let key = mutable_state_of(1);
    let events = log();
    let made = Rc::new(Cell::new(0));
    let (k, ev, m) = (key.clone(), events.clone(), made.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let (ev, m) = (ev.clone(), m.clone());
        ReusableContent(k.get(), move || {
            let n = remember(|| {
                m.set(m.get() + 1);
                m.get()
            });
            let ev = ev.clone();
            DisposableEffect((), move || {
                ev.borrow_mut().push(format!("+{n}"));
                on_dispose(move || ev.borrow_mut().push(format!("-{n}")))
            });
            Text(format!("n={n}"));
        });
    }));
    let node = rule.root.children.borrow()[0].clone();
    assert_eq!(rule.names(), ["n=1"]);
    assert_eq!(taken(&events), ["+1"]);
    key.set(1); // the same key: as it was
    rule.wait_for_idle();
    assert_eq!((rule.names(), node.reuses.get()), (vec!["n=1".to_string()], 0));
    key.set(2);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["n=2"]);
    assert_eq!(taken(&events), ["-1", "+2"]);
    let after = rule.root.children.borrow()[0].clone();
    assert!(Rc::ptr_eq(&node, &after), "the node is reused");
    assert_eq!((node.reuses.get(), node.releases.get()), (1, 0));
}

/// Reusable content keyed by `key` saving a value made by `made`, under `registry`.
fn saved_in_reused_content(registry: &SaveableStateRegistry, key: MutableState<i32>, made: Rc<Cell<i32>>) -> Rule {
    let reg = registry.clone();
    Rule::new(kompozable_lambda!(move || {
        let m = made.clone();
        let k = key.get();
        ProvideSaveableStateRegistry(&reg, move || {
            let m = m.clone();
            ReusableContent(k, move || {
                let m = m.clone();
                let n = remember_saveable(move || {
                    m.set(m.get() + 1);
                    m.get()
                });
                Text(format!("n={n}"));
            });
        });
    }))
}

/// A saveable value in reused content starts over too, and what is saved is the new one.
fn reusable_content_starts_its_saveable_state_over() {
    let registry = SaveableStateRegistry::new(None);
    let key = mutable_state_of(1);
    let rule = saved_in_reused_content(&registry, key.clone(), Rc::new(Cell::new(0)));
    assert_eq!(rule.names(), ["n=1"]);
    key.set(2);
    rule.wait_for_idle();
    assert_eq!(rule.names(), ["n=2"]);
    // Written out and given back: the new value.
    let saved = registry.save();
    drop(rule);
    let restored = SaveableStateRegistry::new(Some(&saved));
    let rule = saved_in_reused_content(&restored, mutable_state_of(2), Rc::new(Cell::new(100)));
    assert_eq!(rule.names(), ["n=2"]);
}

fn reusable_content_host_deactivates_and_reuses() {
    let active = mutable_state_of(true);
    let events = log();
    let (a, ev) = (active.clone(), events.clone());
    let rule = Rule::new(kompozable_lambda!(move || {
        let ev = ev.clone();
        ReusableContentHost(a.get(), move || {
            let ev2 = ev.clone();
            remember_observer(move || Told { log: ev2, name: "value" });
            let ev = ev.clone();
            DisposableEffect((), move || {
                ev.borrow_mut().push("+effect".into());
                on_dispose(move || ev.borrow_mut().push("-effect".into()))
            });
            Text("shown");
        });
    }));
    assert_eq!(taken(&events), ["remembered value", "+effect"]);
    let node = rule.root.children.borrow()[0].clone();
    active.set(false);
    rule.wait_for_idle();
    // Its state and effects gone, its node kept, deactivated.
    let mut events_now = taken(&events);
    events_now.sort();
    assert_eq!(events_now, ["-effect", "forgotten value"]);
    assert_eq!(rule.names(), ["shown"]);
    assert_eq!((node.deactivations.get(), node.releases.get()), (1, 0));
    active.set(false);
    rule.wait_for_idle();
    assert!(taken(&events).is_empty());
    active.set(true);
    rule.wait_for_idle();
    assert_eq!(taken(&events), ["remembered value", "+effect"]);
    assert!(Rc::ptr_eq(&node, &rule.root.children.borrow()[0]));
    assert_eq!((node.reuses.get(), node.deactivations.get()), (1, 1));
}

fn a_deactivated_composition_is_composed_again_reusing_its_nodes() {
    let k: Kept = Rc::new(RefCell::new(None));
    let kk = k.clone();
    let _rule = Rule::new(kompozable_lambda!(move || *kk.borrow_mut() = Some(remember_kompozition_context())));
    let events = log();
    let made = Rc::new(Cell::new(0));
    let root = TestNode::new("slot");
    let child = Composition::with_parent(Some(root.clone()), &kept(&k));
    let content = {
        let (ev, m) = (events.clone(), made.clone());
        move || {
            let (ev, m) = (ev.clone(), m.clone());
            kompozable_block!({
                remember_observer(move || Told { log: ev, name: "state" });
                let n = remember(move || {
                    m.set(m.get() + 1);
                    m.get()
                });
                Text(format!("n={n}"));
            });
        }
    };
    child.set_content(content.clone());
    assert_eq!(taken(&events), ["remembered state"]);
    let node = root.children.borrow()[0].clone();
    child.deactivate();
    assert!(child.is_deactivated());
    assert_eq!(taken(&events), ["forgotten state"]);
    assert_eq!(node.deactivations.get(), 1);
    child.set_content_with_reuse(content);
    assert!(!child.is_deactivated());
    assert_eq!(taken(&events), ["remembered state"]);
    assert_eq!(root.names(), ["n=2"]);
    assert!(Rc::ptr_eq(&node, &root.children.borrow()[0]));
    assert_eq!((node.reuses.get(), node.releases.get()), (1, 0));
}

fn nodes_leaving_are_released() {
    let show = mutable_state_of(true);
    let s = show.clone();
    let rule = Rule::new(kompozable_lambda!(move || {
        if s.get() {
            Text("here");
        }
    }));
    let node = rule.root.children.borrow()[0].clone();
    show.set(false);
    rule.wait_for_idle();
    assert_eq!((node.releases.get(), node.deactivations.get()), (1, 0));
}

fn a_paused_composition_composes_a_piece_at_a_time() {
    let k: Kept = Rc::new(RefCell::new(None));
    let kk = k.clone();
    let rule = Rule::new(kompozable_lambda!(move || *kk.borrow_mut() = Some(remember_kompozition_context())));
    let events = log();
    let label = mutable_state_of("c".to_string());
    let root = TestNode::new("slot");
    let child = Composition::with_parent(Some(root.clone()), &kept(&k));
    let (ev, l) = (events.clone(), label.clone());
    let paused = child.set_pausable_content(kompozable_lambda!(move || {
        let ev2 = ev.clone();
        remember_observer(move || Told { log: ev2, name: "state" });
        Text("a");
        Text("b");
        Label(l.clone());
        let ev = ev.clone();
        SideEffect(move || ev.borrow_mut().push("side".into()));
    }));
    assert!(!paused.is_complete() && !paused.is_applied());
    // Nothing is composed before it is resumed.
    assert!(root.names().is_empty());
    // Asked to pause at every restartable call: the content runs, its three Texts wait.
    assert!(!paused.resume(|| true));
    assert!(root.names().is_empty());
    // Each later resume composes one of them before it pauses.
    assert!(!paused.resume(|| true));
    assert_eq!(root.names(), ["a"]);
    assert!(!paused.resume(|| true));
    assert_eq!(root.names(), ["a", "b"]);
    // (the Label, then the Text in it)
    assert!(!paused.resume(|| true));
    assert!(paused.resume(|| true));
    assert_eq!(root.names(), ["a", "b", "c"]);
    // Nothing it remembered entered, nor did its effects run, nor does the host recompose it.
    assert!(taken(&events).is_empty());
    label.set("d".into());
    rule.wait_for_idle();
    assert_eq!(root.names(), ["a", "b", "c"]);
    // What it read changed: not complete until resumed again.
    assert!(!paused.is_complete());
    assert!(paused.resume(|| false));
    assert_eq!(root.names(), ["a", "b", "d"]);
    paused.apply();
    assert!(paused.is_applied());
    assert_eq!(taken(&events), ["remembered state", "side"]);
    // Applied, it is recomposed as any other.
    label.set("e".into());
    rule.wait_for_idle();
    assert_eq!(root.names(), ["a", "b", "e"]);
}

fn a_paused_composition_cannot_be_applied_before_it_is_complete() {
    let k: Kept = Rc::new(RefCell::new(None));
    let kk = k.clone();
    let _rule = Rule::new(kompozable_lambda!(move || *kk.borrow_mut() = Some(remember_kompozition_context())));
    let child = Composition::with_parent(Some(TestNode::new("slot")), &kept(&k));
    let paused = child.set_pausable_content(kompozable_lambda!(|| {
        Text("a");
    }));
    paused.resume(|| true);
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| paused.apply()));
    std::panic::set_hook(hook);
    assert!(r.is_err());
}

fn a_cancelled_paused_composition_abandons_what_it_remembered() {
    let k: Kept = Rc::new(RefCell::new(None));
    let kk = k.clone();
    let _rule = Rule::new(kompozable_lambda!(move || *kk.borrow_mut() = Some(remember_kompozition_context())));
    let events = log();
    let child = Composition::with_parent(Some(TestNode::new("slot")), &kept(&k));
    let ev = events.clone();
    let paused = child.set_pausable_content(kompozable_lambda!(move || {
        let ev2 = ev.clone();
        remember_observer(move || Told { log: ev2, name: "state" });
        let ev = ev.clone();
        DisposableEffect((), move || {
            ev.borrow_mut().push("+effect".into());
            on_dispose(move || ev.borrow_mut().push("-effect".into()))
        });
        Text("a");
    }));
    assert!(paused.resume(|| false));
    paused.cancel();
    assert!(paused.is_cancelled() && !paused.is_complete());
    child.dispose();
    assert_eq!(taken(&events), ["abandoned state"]);
}

fn a_paused_composition_reusing_a_deactivated_one_reuses_its_nodes() {
    let k: Kept = Rc::new(RefCell::new(None));
    let kk = k.clone();
    let _rule = Rule::new(kompozable_lambda!(move || *kk.borrow_mut() = Some(remember_kompozition_context())));
    let root = TestNode::new("slot");
    let child = Composition::with_parent(Some(root.clone()), &kept(&k));
    let made = Rc::new(Cell::new(0));
    let content = {
        let m = made.clone();
        move || {
            let m = m.clone();
            kompozable_block!({
                Item(m);
            });
        }
    };
    child.set_content(content.clone());
    let node = root.children.borrow()[0].clone();
    assert_eq!(root.names(), ["item 1"]);
    child.deactivate();
    let paused = child.set_pausable_content_with_reuse(content);
    // The item pauses at its restartable call, and is composed in reuse when resumed (its Text pausing too).
    let mut resumes = 1;
    while !paused.resume(|| true) {
        resumes += 1;
    }
    assert_eq!(resumes, 3);
    paused.apply();
    assert_eq!(root.names(), ["item 2"]);
    assert!(Rc::ptr_eq(&node, &root.children.borrow()[0]));
    assert_eq!(node.reuses.get(), 1);
}

#[kompozable]
fn Label(text: MutableState<String>) {
    Text(text.get());
}

#[kompozable]
fn Item(made: Rc<Cell<i32>>) {
    let n = remember(move || {
        made.set(made.get() + 1);
        made.get()
    });
    Text(format!("item {n}"));
}

tests!(
    child_composition_sees_its_parents_locals,
    a_state_a_composition_writes_reaches_the_compositions_after_it_in_the_same_frame,
    parents_recompose_before_their_children,
    child_compositions_continue_their_parents_keys,
    reusable_content_starts_its_state_over_and_reuses_its_nodes,
    reusable_content_starts_its_saveable_state_over,
    reusable_content_host_deactivates_and_reuses,
    a_deactivated_composition_is_composed_again_reusing_its_nodes,
    nodes_leaving_are_released,
    a_paused_composition_composes_a_piece_at_a_time,
    a_paused_composition_cannot_be_applied_before_it_is_complete,
    a_cancelled_paused_composition_abandons_what_it_remembered,
    a_paused_composition_reusing_a_deactivated_one_reuses_its_nodes,
);

//! SnapshotStateObserver, as Compose's: what blocks read, by scope; a scope told when what it read changes in an
//! applied snapshot (the global one, or one applied on another thread through the executor); scopes cleared, reads
//! nested and paused, the observer stopped.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz_runtime::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// An observer delivering at once (its executor runs what it is given, on the thread it is called on), and what its
/// one callback was told.
fn immediate() -> (SnapshotStateObserver, Rc<RefCell<Vec<&'static str>>>, Rc<dyn Fn(&&'static str)>) {
    let observer = SnapshotStateObserver::new(|deliver| deliver());
    let told = Rc::new(RefCell::new(Vec::new()));
    let t = told.clone();
    let on_changed: Rc<dyn Fn(&&'static str)> = Rc::new(move |scope| t.borrow_mut().push(*scope));
    (observer, told, on_changed)
}

fn a_scope_is_told_when_what_it_read_changes() {
    let (observer, told, on_changed) = immediate();
    observer.start();
    let (a, b) = (mutable_state_of(1), mutable_state_of(10));
    let read = observer.observe_reads("a", on_changed.clone(), || a.get());
    assert_eq!(read, 1);
    observer.observe_reads("b", on_changed.clone(), || b.get());
    a.set(2);
    Snapshot::send_apply_notifications();
    assert_eq!(*told.borrow(), ["a"]);
    // Observed again: what it reads now replaces what it read.
    told.borrow_mut().clear();
    observer.observe_reads("a", on_changed.clone(), || b.get());
    a.set(3);
    Snapshot::send_apply_notifications();
    assert!(told.borrow().is_empty());
    b.set(11);
    Snapshot::send_apply_notifications();
    let mut seen = told.borrow().clone();
    seen.sort();
    assert_eq!(seen, ["a", "b"]);
    // Cleared: told no more.
    told.borrow_mut().clear();
    observer.clear(&"a");
    b.set(12);
    Snapshot::send_apply_notifications();
    assert_eq!(*told.borrow(), ["b"]);
    observer.clear_if(|scope| scope.downcast_ref::<&str>() == Some(&"b"));
    told.borrow_mut().clear();
    b.set(13);
    Snapshot::send_apply_notifications();
    assert!(told.borrow().is_empty());
    // Stopped: nothing told.
    observer.observe_reads("a", on_changed.clone(), || a.get());
    observer.stop();
    a.set(4);
    Snapshot::send_apply_notifications();
    assert!(told.borrow().is_empty());
    // Told by hand what changed (Compose's notifyChanges).
    observer.notify_changes(&[a.key()]);
    assert_eq!(*told.borrow(), ["a"]);
}

fn nested_reads_go_to_the_inner_scope_and_paused_ones_nowhere() {
    let (observer, told, on_changed) = immediate();
    observer.start();
    let (outer, inner, paused) = (mutable_state_of(0), mutable_state_of(0), mutable_state_of(0));
    observer.observe_reads("dış", on_changed.clone(), || {
        outer.get();
        observer.observe_reads("iç", on_changed.clone(), || inner.get());
        observer.with_no_observations(|| paused.get());
    });
    inner.set(1);
    Snapshot::send_apply_notifications();
    assert_eq!(*told.borrow(), ["iç"]);
    told.borrow_mut().clear();
    paused.set(1);
    Snapshot::send_apply_notifications();
    assert!(told.borrow().is_empty());
    outer.set(1);
    Snapshot::send_apply_notifications();
    assert_eq!(*told.borrow(), ["dış"]);
    // A callback of its own: its scopes apart.
    let other = Rc::new(RefCell::new(Vec::new()));
    let o = other.clone();
    let on_other: Rc<dyn Fn(&i32)> = Rc::new(move |scope| o.borrow_mut().push(*scope));
    observer.observe_reads(7, on_other, || outer.get());
    told.borrow_mut().clear();
    outer.set(2);
    Snapshot::send_apply_notifications();
    assert_eq!((told.borrow().clone(), other.borrow().clone()), (vec!["dış"], vec![7]));
}

fn a_snapshot_applied_on_another_thread_comes_through_the_executor() {
    // The executor posts (as a host's handle would) what the applying thread gives it; run here, it delivers.
    let posted: Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>> = Arc::default();
    let p = posted.clone();
    let observer = SnapshotStateObserver::new(move |deliver| p.lock().unwrap().push(deliver));
    let told = Rc::new(RefCell::new(Vec::new()));
    let t = told.clone();
    let on_changed: Rc<dyn Fn(&&'static str)> = Rc::new(move |scope| t.borrow_mut().push(*scope));
    observer.start();
    let state = mutable_state_of(0);
    observer.observe_reads("uzak", on_changed, || state.get());
    let s = state.clone();
    std::thread::spawn(move || {
        let snapshot = Snapshot::take_mutable_snapshot(None, None);
        snapshot.enter(|| s.set(5));
        let _ = snapshot.apply();
        snapshot.dispose();
    })
    .join()
    .unwrap();
    assert!(told.borrow().is_empty());
    // A delivery run on another thread does nothing (the change waits); run on its own, one tells.
    let deliveries = std::mem::take(&mut *posted.lock().unwrap());
    assert!(!deliveries.is_empty());
    std::thread::spawn(move || {
        for deliver in deliveries {
            deliver();
        }
    })
    .join()
    .unwrap();
    assert!(told.borrow().is_empty());
    let s = state.clone();
    std::thread::spawn(move || {
        let snapshot = Snapshot::take_mutable_snapshot(None, None);
        snapshot.enter(|| s.set(6));
        let _ = snapshot.apply();
        snapshot.dispose();
    })
    .join()
    .unwrap();
    for deliver in std::mem::take(&mut *posted.lock().unwrap()) {
        deliver();
    }
    assert_eq!(*told.borrow(), ["uzak"]);
}

tests!(
    a_scope_is_told_when_what_it_read_changes,
    nested_reads_go_to_the_inner_scope_and_paused_ones_nowhere,
    a_snapshot_applied_on_another_thread_comes_through_the_executor,
);

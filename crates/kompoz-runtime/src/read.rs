//! Who is told about a read: the innermost observer this thread installed (a composition's running scope, a
//! layout's measure, a drawing), as reads happen; and what a derived state computing reads (its dependencies,
//! Compose's `DerivedSnapshotState`'s), to tell later whether its value still stands.

use crate::snapshot::{StateKey, StateObject};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};
use std::sync::Arc;

thread_local! {
    static OBSERVERS: RefCell<Vec<Option<Rc<dyn Fn(StateKey)>>>> = const { RefCell::new(Vec::new()) };
    /// The derived states computing, innermost last (`None`: reads not observed there, which aren't theirs).
    static DEPENDENCIES: RefCell<Vec<Option<Dependencies>>> = const { RefCell::new(Vec::new()) };
}

/// Tells the innermost read observer that `key` was read. States call it as they are read (not peeked).
pub fn record_read(key: StateKey) {
    note_read(key);
    tell_observer(key);
}

/// Tells the innermost read observer that `key` was read, not as a dependency of the derived state computing (a
/// derived state computed at every read: what it read is).
pub(crate) fn tell_observer(key: StateKey) {
    let top = OBSERVERS.with(|o| o.borrow().last().cloned().flatten());
    if let Some(observer) = top {
        observer(key);
    }
}

struct Pop;
impl Drop for Pop {
    fn drop(&mut self) {
        OBSERVERS.with(|o| o.borrow_mut().pop());
    }
}

/// Runs `f` with what it reads told to `observer` (and not to the observers outside it).
pub fn observe_reads<R>(observer: Rc<dyn Fn(StateKey)>, f: impl FnOnce() -> R) -> R {
    OBSERVERS.with(|o| o.borrow_mut().push(Some(observer)));
    let _pop = Pop;
    f()
}

/// Runs `f` with what it reads told to no one (Compose's Snapshot.withoutReadObservation): not to the read
/// observers, nor as a dependency of the derived state computing.
pub fn without_read_observation<R>(f: impl FnOnce() -> R) -> R {
    OBSERVERS.with(|o| o.borrow_mut().push(None));
    DEPENDENCIES.with(|d| d.borrow_mut().push(None));
    let _pop = Pop;
    let _pop_dependencies = PopDependencies;
    f()
}

// ---- Dependencies of derived states ----

/// A derived state, as one reading it depends on it: brought up to date, the version of its value (one more at
/// each change).
pub(crate) trait DerivedDependency {
    fn validated_version(&self) -> u64;
}

/// Something a derived state's calculation read, as it was then.
#[derive(Clone)]
pub(crate) enum Dependency {
    /// A state, and the stamp of the record it read.
    State(Arc<dyn StateObject>, u64),
    /// Another derived state (made in a composition: its value kept), and its value's version then.
    Derived(Weak<dyn DerivedDependency>, u64),
}

impl Dependency {
    /// Whether what this reads now isn't what it read then (the derived state it is brought up to date).
    pub(crate) fn changed(&self) -> bool {
        match self {
            Dependency::State(object, stamp) => crate::snapshot::readable_stamp(&**object) != *stamp,
            Dependency::Derived(derived, version) => {
                derived.upgrade().is_none_or(|d| d.validated_version() != *version)
            }
        }
    }
}

/// What a calculation read: the dependencies it can check, and every key it read (some it can't: a local's).
#[derive(Default)]
pub(crate) struct Dependencies {
    pub(crate) list: Vec<Dependency>,
    tracked: HashSet<StateKey>,
    read: HashSet<StateKey>,
}

impl Dependencies {
    /// Whether every key read is one of its dependencies (a change of what it read shows in them).
    pub(crate) fn all_tracked(&self) -> bool {
        self.read.iter().all(|k| self.tracked.contains(k))
    }
}

struct PopDependencies;
impl Drop for PopDependencies {
    fn drop(&mut self) {
        DEPENDENCIES.with(|d| d.borrow_mut().pop());
    }
}

/// Runs `f`, collecting what it reads (a derived state's calculation).
pub(crate) fn collect_dependencies<R>(f: impl FnOnce() -> R) -> (R, Dependencies) {
    DEPENDENCIES.with(|d| d.borrow_mut().push(Some(Dependencies::default())));
    let pop = PopDependencies; // (popped by it if `f` unwinds)
    let r = f();
    std::mem::forget(pop);
    let dependencies = DEPENDENCIES.with(|d| d.borrow_mut().pop()).flatten().unwrap_or_default();
    (r, dependencies)
}

/// Whether a derived state is computing here (its reads collected).
pub(crate) fn collecting_dependencies() -> bool {
    DEPENDENCIES.with(|d| matches!(d.borrow().last(), Some(Some(_))))
}

/// A dependency of the derived state computing: `key` read as `dependency` says.
pub(crate) fn record_dependency(key: StateKey, dependency: Dependency) {
    DEPENDENCIES.with(|d| {
        if let Some(Some(top)) = d.borrow_mut().last_mut()
            && top.tracked.insert(key)
        {
            top.list.push(dependency);
        }
    });
}

/// `key` read by the derived state computing, if one is (Compose's dependencies count every state object read).
pub(crate) fn note_read(key: StateKey) {
    DEPENDENCIES.with(|d| {
        if let Some(Some(top)) = d.borrow_mut().last_mut() {
            top.read.insert(key);
        }
    });
}

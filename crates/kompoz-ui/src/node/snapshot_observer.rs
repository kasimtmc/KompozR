//! Which layouts read which states while measuring, placing, drawing, computing their layers and semantics, and
//! what to invalidate when those states change (Compose's OwnerSnapshotObserver over a SnapshotStateObserver).

use kompoz_runtime::{ObserverHandle, Snapshot, StateKey, observe_reads};
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// One thing whose reads are observed (a layout's measure, its placement, a draw, a node's `observe_reads`): what
/// it read last time, and what to do when one of those changes.
pub(crate) struct ReaderScope {
    reads: RefCell<Vec<StateKey>>,
    on_changed: Box<dyn Fn()>,
    observer: Weak<SnapshotObserver>,
    /// What `observe_reads` is given: records into this scope.
    recorder: RefCell<Option<Rc<dyn Fn(StateKey)>>>,
}

impl Drop for ReaderScope {
    fn drop(&mut self) {
        let reads = std::mem::take(&mut *self.reads.borrow_mut());
        if let Some(o) = self.observer.upgrade() {
            o.forget(self as *const ReaderScope, &reads);
        }
    }
}

/// The states read, by scope, and the changes applied since they were last looked at.
pub(crate) struct SnapshotObserver {
    readers: RefCell<HashMap<StateKey, Vec<Weak<ReaderScope>>>>,
    /// What applied snapshots changed, from any thread.
    pending: Arc<Mutex<Vec<StateKey>>>,
    _registration: ObserverHandle,
}

impl SnapshotObserver {
    pub(crate) fn new() -> Rc<SnapshotObserver> {
        let pending = Arc::new(Mutex::new(Vec::new()));
        let sink = pending.clone();
        let registration = Snapshot::register_apply_observer(move |changed| sink.lock().extend_from_slice(changed));
        Rc::new(SnapshotObserver { readers: RefCell::new(HashMap::new()), pending, _registration: registration })
    }

    /// A scope that calls `on_changed` when what it read changes.
    pub(crate) fn scope(self: &Rc<Self>, on_changed: impl Fn() + 'static) -> Rc<ReaderScope> {
        let scope = Rc::new(ReaderScope {
            reads: RefCell::new(Vec::new()),
            on_changed: Box::new(on_changed),
            observer: Rc::downgrade(self),
            recorder: RefCell::new(None),
        });
        let weak = Rc::downgrade(&scope);
        let observer = Rc::downgrade(self);
        let recorder: Rc<dyn Fn(StateKey)> = Rc::new(move |key| {
            if let (Some(scope), Some(observer)) = (weak.upgrade(), observer.upgrade()) {
                observer.record(&scope, key);
            }
        });
        *scope.recorder.borrow_mut() = Some(recorder);
        scope
    }

    fn record(&self, scope: &Rc<ReaderScope>, key: StateKey) {
        {
            let mut reads = scope.reads.borrow_mut();
            if reads.contains(&key) {
                return;
            }
            reads.push(key);
        }
        self.readers.borrow_mut().entry(key).or_default().push(Rc::downgrade(scope));
    }

    /// Runs `f` with what it reads recorded into `scope`, forgetting what it read before.
    pub(crate) fn observe<R>(&self, scope: &Rc<ReaderScope>, f: impl FnOnce() -> R) -> R {
        self.clear(scope);
        let recorder = scope.recorder.borrow().clone().expect("a reader scope without its recorder");
        observe_reads(recorder, f)
    }

    /// Forgets what `scope` read.
    pub(crate) fn clear(&self, scope: &Rc<ReaderScope>) {
        let reads = std::mem::take(&mut *scope.reads.borrow_mut());
        self.forget(Rc::as_ptr(scope), &reads);
    }

    fn forget(&self, scope: *const ReaderScope, reads: &[StateKey]) {
        let Ok(mut readers) = self.readers.try_borrow_mut() else { return };
        for key in reads {
            if let Some(list) = readers.get_mut(key) {
                list.retain(|w| w.as_ptr() != scope && w.strong_count() > 0);
                if list.is_empty() {
                    readers.remove(key);
                }
            }
        }
    }

    /// Whether changes wait to be looked at.
    pub(crate) fn has_pending(&self) -> bool {
        !self.pending.lock().is_empty()
    }

    /// Tells the scopes that read what changed (each once). True if any was told.
    pub(crate) fn process_changes(&self) -> bool {
        let changed = std::mem::take(&mut *self.pending.lock());
        if changed.is_empty() {
            return false;
        }
        let mut told: Vec<Rc<ReaderScope>> = Vec::new();
        {
            let readers = self.readers.borrow();
            for key in &changed {
                if let Some(list) = readers.get(key) {
                    for w in list {
                        if let Some(s) = w.upgrade()
                            && !told.iter().any(|t| Rc::ptr_eq(t, &s))
                        {
                            told.push(s);
                        }
                    }
                }
            }
        }
        let any = !told.is_empty();
        for scope in told {
            (scope.on_changed)();
        }
        any
    }
}

//! `SnapshotStateObserver`, as Compose's `androidx.compose.runtime.snapshots.SnapshotStateObserver`: what states
//! blocks read, by scope, and a callback per scope when one of those changes (what a layout's measure is to its
//! host, what a seekable transition's animation is to the states its duration reads).

use crate::read::observe_reads;
use crate::snapshot::{ObserverHandle, Snapshot, StateKey};
use parking_lot::Mutex;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::ThreadId;

/// Runs what a [`SnapshotStateObserver`] gives it — the delivery of the changes of a snapshot applied, from the
/// thread that applied it — where the observer lives (Compose's `onChangedExecutor`): at once when that is the
/// thread it is called on, else posted there (a host's `HostHandle::post`). A delivery run on another thread does
/// nothing (the changes wait for the next one).
pub type OnChangedExecutor = Arc<dyn Fn(Box<dyn FnOnce() + Send>) + Send + Sync>;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

thread_local! {
    /// The observers of this thread, by id: what a delivery finds its observer by.
    static OBSERVERS: RefCell<HashMap<u64, Weak<Inner>>> = RefCell::new(HashMap::new());
}

/// Observes what blocks read, by scope, and calls a scope's `on_value_changed_for_scope` when one of the states it
/// read is changed by an applied snapshot (Compose's SnapshotStateObserver). [`start`](Self::start) it to be told,
/// [`stop`](Self::stop) it when done. It lives on the thread it was made on (its scopes and callbacks are that
/// thread's); the changes of a snapshot applied anywhere come through its executor. Clones are the same observer.
#[derive(Clone)]
pub struct SnapshotStateObserver {
    inner: Rc<Inner>,
}

struct Inner {
    id: u64,
    thread: ThreadId,
    executor: OnChangedExecutor,
    /// The keys applied snapshots changed, not yet delivered (from any thread).
    pending: Arc<Mutex<Vec<StateKey>>>,
    /// One map of scopes per `on_value_changed_for_scope` callback (Compose's `observedScopeMaps`).
    maps: RefCell<Vec<Rc<ScopeMap>>>,
    /// Reads not recorded (Compose's `isPaused`, `withNoObservations`).
    paused: Cell<bool>,
    registration: RefCell<Option<ObserverHandle>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let id = self.id;
        let _ = OBSERVERS.try_with(|o| o.borrow_mut().remove(&id));
    }
}

/// A scope, compared by value (Compose's scope objects compared by `equals`).
trait ScopeValue: Any {
    fn eq_dyn(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: PartialEq + 'static> ScopeValue for T {
    fn eq_dyn(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| o == self)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A scope and what it read last.
struct ScopeEntry {
    scope: Rc<dyn ScopeValue>,
    reads: RefCell<Vec<StateKey>>,
}

/// The scopes observed with one callback (Compose's ObservedScopeMap).
struct ScopeMap {
    /// The callback's identity (the address of what it points to).
    callback: *const (),
    on_changed: Box<dyn Fn(&dyn Any)>,
    scopes: RefCell<Vec<Rc<ScopeEntry>>>,
    /// The scopes reading each key.
    readers: RefCell<HashMap<StateKey, Vec<Weak<ScopeEntry>>>>,
    /// The scopes a change reached, waiting for their callback.
    invalidated: RefCell<Vec<Rc<dyn ScopeValue>>>,
}

impl ScopeMap {
    fn entry_for(&self, scope: &dyn ScopeValue) -> Option<Rc<ScopeEntry>> {
        self.scopes.borrow().iter().find(|e| e.scope.eq_dyn(scope.as_any())).cloned()
    }

    /// Forgets what `entry` read.
    fn clear_reads(&self, entry: &Rc<ScopeEntry>) {
        let reads = std::mem::take(&mut *entry.reads.borrow_mut());
        let mut readers = self.readers.borrow_mut();
        for key in reads {
            if let Some(list) = readers.get_mut(&key) {
                list.retain(|w| w.upgrade().is_some_and(|e| !Rc::ptr_eq(&e, entry)));
                if list.is_empty() {
                    readers.remove(&key);
                }
            }
        }
    }

    fn record(&self, entry: &Rc<ScopeEntry>, key: StateKey) {
        {
            let mut reads = entry.reads.borrow_mut();
            if reads.contains(&key) {
                return;
            }
            reads.push(key);
        }
        self.readers.borrow_mut().entry(key).or_default().push(Rc::downgrade(entry));
    }

    fn remove_where(&self, predicate: &dyn Fn(&dyn Any) -> bool) {
        let removed: Vec<Rc<ScopeEntry>> =
            self.scopes.borrow().iter().filter(|e| predicate(e.scope.as_any())).cloned().collect();
        for entry in &removed {
            self.clear_reads(entry);
        }
        self.scopes.borrow_mut().retain(|e| !removed.iter().any(|r| Rc::ptr_eq(r, e)));
        self.invalidated.borrow_mut().retain(|s| !predicate(s.as_any()));
    }

    /// The scopes that read one of `changed`, noted as invalidated (Compose's `recordInvalidation`). Whether any.
    fn record_invalidation(&self, changed: &[StateKey]) -> bool {
        let mut any = false;
        let readers = self.readers.borrow();
        let mut invalidated = self.invalidated.borrow_mut();
        for key in changed {
            let Some(list) = readers.get(key) else { continue };
            for entry in list.iter().filter_map(Weak::upgrade) {
                if !invalidated.iter().any(|s| Rc::ptr_eq(s, &entry.scope)) {
                    invalidated.push(entry.scope.clone());
                }
                any = true;
            }
        }
        any
    }
}

impl SnapshotStateObserver {
    /// An observer whose deliveries `on_changed_executor` runs (Compose's `SnapshotStateObserver(onChangedExecutor)`):
    /// `|callback| callback()` where snapshots are only applied on this thread.
    pub fn new(
        on_changed_executor: impl Fn(Box<dyn FnOnce() + Send>) + Send + Sync + 'static,
    ) -> SnapshotStateObserver {
        let inner = Rc::new(Inner {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            thread: std::thread::current().id(),
            executor: Arc::new(on_changed_executor),
            pending: Arc::new(Mutex::new(Vec::new())),
            maps: RefCell::new(Vec::new()),
            paused: Cell::new(false),
            registration: RefCell::new(None),
        });
        OBSERVERS.with(|o| o.borrow_mut().insert(inner.id, Rc::downgrade(&inner)));
        SnapshotStateObserver { inner }
    }

    /// Runs `block`, recording what it reads into `scope` (what it read before forgotten): when one of those states
    /// changes, `on_value_changed_for_scope` is called with the scope (Compose's `observeReads`). The scopes of one
    /// callback are told together; a scope is the same as another equal to it. What an `observe_reads` nested in
    /// `block` reads is that one's alone.
    pub fn observe_reads<T: PartialEq + 'static, R>(
        &self,
        scope: T,
        on_value_changed_for_scope: Rc<dyn Fn(&T)>,
        block: impl FnOnce() -> R,
    ) -> R {
        let map = self.map_for(on_value_changed_for_scope);
        let scope: Rc<dyn ScopeValue> = Rc::new(scope);
        let entry = match map.entry_for(&*scope) {
            Some(e) => {
                map.clear_reads(&e);
                e
            }
            None => {
                let e = Rc::new(ScopeEntry { scope, reads: RefCell::new(Vec::new()) });
                map.scopes.borrow_mut().push(e.clone());
                e
            }
        };
        let inner = Rc::downgrade(&self.inner);
        let weak_map = Rc::downgrade(&map);
        let weak_entry = Rc::downgrade(&entry);
        let recorder: Rc<dyn Fn(StateKey)> = Rc::new(move |key| {
            let (Some(inner), Some(map), Some(entry)) = (inner.upgrade(), weak_map.upgrade(), weak_entry.upgrade())
            else {
                return;
            };
            if !inner.paused.get() {
                map.record(&entry, key);
            }
        });
        let was_paused = self.inner.paused.replace(false);
        let r = observe_reads(recorder, block);
        self.inner.paused.set(was_paused);
        r
    }

    /// The map of `on_changed`'s scopes, made the first time.
    fn map_for<T: 'static>(&self, on_changed: Rc<dyn Fn(&T)>) -> Rc<ScopeMap> {
        let callback = Rc::as_ptr(&on_changed) as *const ();
        if let Some(m) = self.inner.maps.borrow().iter().find(|m| m.callback == callback) {
            return m.clone();
        }
        let map = Rc::new(ScopeMap {
            callback,
            on_changed: Box::new(move |scope: &dyn Any| {
                if let Some(scope) = scope.downcast_ref::<T>() {
                    on_changed(scope);
                }
            }),
            scopes: RefCell::new(Vec::new()),
            readers: RefCell::new(HashMap::new()),
            invalidated: RefCell::new(Vec::new()),
        });
        self.inner.maps.borrow_mut().push(map.clone());
        map
    }

    /// Runs `block` with what it reads not recorded (Compose's deprecated `withNoObservations`; `Snapshot
    /// .withoutReadObservation` is what to use now, [`without_read_observation`](crate::without_read_observation)).
    pub fn with_no_observations<R>(&self, block: impl FnOnce() -> R) -> R {
        let was = self.inner.paused.replace(true);
        let r = block();
        self.inner.paused.set(was);
        r
    }

    /// Forgets what `scope` read, for every callback (Compose's `clear(scope)`).
    pub fn clear<T: PartialEq + 'static>(&self, scope: &T) {
        self.clear_if(|s| s.downcast_ref::<T>().is_some_and(|s| s == scope));
    }

    /// Forgets what the scopes `predicate` holds for read (Compose's `clearIf`).
    pub fn clear_if(&self, predicate: impl Fn(&dyn Any) -> bool) {
        let maps = self.inner.maps.borrow().clone();
        for map in maps {
            map.remove_where(&predicate);
        }
    }

    /// Forgets everything (Compose's `clear()`).
    pub fn clear_all(&self) {
        self.inner.maps.borrow_mut().clear();
    }

    /// Starts being told of applied snapshots (Compose's `start`).
    pub fn start(&self) {
        if self.inner.registration.borrow().is_some() {
            return;
        }
        let pending = self.inner.pending.clone();
        let executor = self.inner.executor.clone();
        let (id, thread) = (self.inner.id, self.inner.thread);
        let registration = Snapshot::register_apply_observer(move |changed| {
            if changed.is_empty() {
                return;
            }
            pending.lock().extend_from_slice(changed);
            // (on its own thread, only when a scope read what changed: Compose's `drainChanges` first)
            if std::thread::current().id() == thread && !has_invalidations(id) {
                return;
            }
            executor(Box::new(move || deliver(id, thread)));
        });
        *self.inner.registration.borrow_mut() = Some(registration);
    }

    /// Stops being told (Compose's `stop`).
    pub fn stop(&self) {
        self.inner.registration.borrow_mut().take();
    }

    /// Tells the scopes that read one of `changes` (Compose's `notifyChanges`): what an apply of them does.
    pub fn notify_changes(&self, changes: &[StateKey]) {
        self.inner.pending.lock().extend_from_slice(changes);
        self.inner.send_notifications();
    }
}

impl Inner {
    /// The pending keys' scopes noted as invalidated; whether any.
    fn drain_changes(&self) -> bool {
        let changed = std::mem::take(&mut *self.pending.lock());
        if changed.is_empty() {
            return false;
        }
        let maps = self.maps.borrow().clone();
        let mut any = false;
        for map in maps {
            any |= map.record_invalidation(&changed);
        }
        any
    }

    /// The invalidated scopes told, a callback's together (Compose's `sendNotifications`).
    fn send_notifications(&self) {
        self.drain_changes();
        let maps = self.maps.borrow().clone();
        for map in maps {
            let scopes = std::mem::take(&mut *map.invalidated.borrow_mut());
            for scope in scopes {
                (map.on_changed)(scope.as_any());
            }
        }
    }
}

/// Whether the changes waiting for observer `id` reach a scope (noted invalidated if so).
fn has_invalidations(id: u64) -> bool {
    let inner = OBSERVERS.with(|o| o.borrow().get(&id).and_then(Weak::upgrade));
    inner.is_some_and(|i| i.drain_changes() || i.maps.borrow().iter().any(|m| !m.invalidated.borrow().is_empty()))
}

/// The delivery an executor runs: the observer's invalidated scopes told, on its thread.
fn deliver(id: u64, thread: ThreadId) {
    if std::thread::current().id() != thread {
        return;
    }
    let inner = OBSERVERS.with(|o| o.borrow().get(&id).and_then(Weak::upgrade));
    if let Some(inner) = inner {
        inner.send_notifications();
    }
}

impl std::fmt::Debug for SnapshotStateObserver {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "SnapshotStateObserver({} callbacks)", self.inner.maps.borrow().len())
    }
}

//! Snapshots, as Compose's snapshot system: every state keeps its value in records, one per snapshot that wrote
//! it, and code sees the states as of the snapshot it runs in. Outside any, code runs in the global snapshot,
//! whose writes are told to whoever listens (the hosts: their readers recompose) when it is applied
//! ([`Snapshot::send_apply_notifications`], at every frame). A composition runs in a snapshot of its own, applied
//! when it is over: it sees one consistent state however other threads write meanwhile.
//!
//! ```
//! # use kompoz_runtime::*;
//! # let (first, second) = (mutable_state_of(0), mutable_state_of(0));
//! Snapshot::with_mutable_snapshot(|| {  // both written at once: no one sees one without the other
//!     first.set(1);
//!     second.set(2);
//! }).unwrap();
//! ```
//!
//! Threads: on the UI thread (the one the hosts run on, [`set_ui_thread`]) states are read without a lock.
//! Any other thread may write them (each write applied at once, or with others in `with_mutable_snapshot`) and
//! reads them under the lock, or in a snapshot of its own.
//!
//! Memory: records no snapshot reads any more are unlinked between frames on the UI thread
//! (`send_apply_notifications`), and freed once no snapshot that was open when they were unlinked is left (one
//! may still have been walking through them).

use parking_lot::{Mutex, ReentrantMutex, ReentrantMutexGuard};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicPtr, AtomicU64, Ordering::*};
use std::sync::{Arc, OnceLock, Weak};
use std::thread::ThreadId;

/// Who a read is recorded under: a state, or any object whose readers are told when it changes
/// ([`state_changed`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct StateKey(u64);

impl StateKey {
    /// A key no other object has.
    pub fn new() -> StateKey {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        StateKey(NEXT.fetch_add(1, Relaxed))
    }
}

impl Default for StateKey {
    fn default() -> Self {
        StateKey::new()
    }
}

const INVALID_SNAPSHOT: i64 = 0; // a record no one reads (dropped)
const PREEXISTING_SNAPSHOT: i64 = 1; // a state's first record, seen in every snapshot

/// Snapshot ids, as sorted ranges that don't touch: those a snapshot doesn't see.
#[derive(Clone, Default, Debug)]
struct IdSet {
    ranges: Vec<(i64, i64)>, // [first, last]
}

impl IdSet {
    fn contains(&self, id: i64) -> bool {
        let i = self.ranges.partition_point(|r| r.0 <= id);
        i > 0 && self.ranges[i - 1].1 >= id
    }
    fn add_range(&mut self, mut first: i64, mut last: i64) {
        if first > last {
            return;
        }
        let mut out = Vec::with_capacity(self.ranges.len() + 1);
        let mut placed = false;
        for &r in &self.ranges {
            if r.1 + 1 < first {
                out.push(r);
            } else if last + 1 < r.0 {
                if !placed {
                    out.push((first, last));
                    placed = true;
                }
                out.push(r);
            } else {
                first = first.min(r.0);
                last = last.max(r.1);
            }
        }
        if !placed {
            out.push((first, last));
        }
        self.ranges = out;
    }
    fn add(&mut self, id: i64) {
        self.add_range(id, id);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Records and state objects

/// What every record starts with: the snapshot that wrote it, the next (older) one, and its stamp: a number no
/// other value of any state had, given anew whenever the record is written (in place too). A derived state
/// keeps the stamps of what it read, to tell later whether what a snapshot sees of them is still that.
#[repr(C)]
pub(crate) struct RecordHead {
    id: AtomicI64,
    next: AtomicPtr<RecordHead>,
    stamp: AtomicU64,
}

/// A stamp no record had (see [`RecordHead`]).
fn next_stamp() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Relaxed)
}

/// Writes of any state in any snapshot so far (a record made, written in place, or applied to a parent): when it
/// and the snapshot read in are as they were, no state a derived state read can have another value now.
static WRITES: AtomicU64 = AtomicU64::new(0);

/// One value of a state, written in the snapshot `head.id`.
#[repr(C)]
struct Record<T> {
    head: RecordHead,
    value: T,
}

fn new_record<T>(value: T, id: i64) -> *mut RecordHead {
    Box::into_raw(Box::new(Record {
        head: RecordHead {
            id: AtomicI64::new(id),
            next: AtomicPtr::new(ptr::null_mut()),
            stamp: AtomicU64::new(next_stamp()),
        },
        value,
    })) as *mut RecordHead
}

/// Frees a record made by `new_record::<T>`.
unsafe fn drop_record<T>(r: *mut RecordHead) {
    drop(unsafe { Box::from_raw(r as *mut Record<T>) });
}

/// A state whose value is kept per snapshot (Compose's StateObject), as the snapshot system handles it whatever
/// its value's type.
pub(crate) trait StateObject: Send + Sync + 'static {
    fn key(&self) -> StateKey;
    /// Its records, newest first.
    fn first(&self) -> &AtomicPtr<RecordHead>;
    /// A new record (not linked) with a copy of `r`'s value.
    unsafe fn copy_record(&self, r: *const RecordHead, id: i64) -> *mut RecordHead;
    /// Whether `applied`, written in a snapshot being applied, may stand where `current` was written meanwhile
    /// by another (Compose's mergeRecords: its policy's equivalent); otherwise the apply fails.
    unsafe fn mergeable(&self, current: *const RecordHead, applied: *const RecordHead) -> bool;
    /// How a record of it is freed.
    fn dropper(&self) -> unsafe fn(*mut RecordHead);
    /// The thread its values are confined to (a state of a value that isn't `Send + Sync`), if one.
    fn owner(&self) -> Option<ThreadId>;
}

/// Panics unless this runs on `owner` (a confined state's values touched elsewhere).
fn check_owner(owner: Option<ThreadId>, what: &str) {
    if let Some(owner) = owner {
        assert!(
            owner == std::thread::current().id(),
            "kompoz: {what} a state confined to another thread (one of a value that isn't Send + Sync)"
        );
    }
}

/// The newest record of `object` written at `id` or before and not by a snapshot in `invalid`.
fn find(object: &dyn StateObject, id: i64, invalid: &IdSet) -> *mut RecordHead {
    let mut best = ptr::null_mut();
    let mut best_id = 0;
    let mut r = object.first().load(Acquire);
    while !r.is_null() {
        // SAFETY: a record linked in, or unlinked but not freed while a snapshot that may see it is open.
        let head = unsafe { &*r };
        let rid = head.id.load(Acquire);
        if rid != INVALID_SNAPSHOT && rid <= id && rid > best_id && !invalid.contains(rid) {
            best = r;
            best_id = rid;
        }
        r = head.next.load(Acquire);
    }
    debug_assert!(!best.is_null(), "a state with no record this snapshot sees");
    best
}

fn prepend(object: &dyn StateObject, record: *mut RecordHead, id: i64) {
    // SAFETY: a fresh record, not linked yet; under the lock.
    let head = unsafe { &*record };
    head.id.store(id, Relaxed);
    head.next.store(object.first().load(Relaxed), Relaxed);
    object.first().store(record, Release);
}

/// A state's records and policy: the value of a `MutableState` (and of what is built on one).
pub(crate) struct StateCell<T> {
    key: StateKey,
    first: AtomicPtr<RecordHead>,
    equivalent: Option<fn(&T, &T) -> bool>,
    /// The thread its values are confined to: a state of a value that isn't `Send + Sync` (`None`: T is).
    owner: Option<ThreadId>,
    _value: PhantomData<T>,
}

// SAFETY: the records are values of T, shared between threads as the snapshot system says (T: Send + Sync).
unsafe impl<T: Send + Sync> Send for StateCell<T> {}
unsafe impl<T: Send + Sync> Sync for StateCell<T> {}

/// A cell as the snapshot system holds it, whatever its value: the system keeps it in lists every thread reaches,
/// and touches the values of a confined one (its owner set) only on its owner thread — `check_owner` where a value
/// is copied or compared, the frees deferred to the owner (`Retired::owner`, `ORPHANS`) — so a confined cell is
/// sound there although its T isn't `Send + Sync`. (A cell of a `Send + Sync` T is one as it is.)
#[repr(transparent)]
struct Erased<T>(StateCell<T>);

// SAFETY: see above; the handles of a confined cell (`MutableState<T>`, not Send for such a T) stay on its thread.
unsafe impl<T> Send for Erased<T> {}
unsafe impl<T> Sync for Erased<T> {}

/// Records of a confined cell dropped (its last handle) on another thread, freed by their owner.
struct Orphan {
    record: *mut RecordHead,
    drop: unsafe fn(*mut RecordHead),
    owner: ThreadId,
}
// SAFETY: unlinked records no one can reach any more (the cell is gone); freed on their owner thread.
unsafe impl Send for Orphan {}

static ORPHANS: Mutex<Vec<Orphan>> = Mutex::new(Vec::new());

/// Frees the orphaned records confined to this thread.
fn free_orphans() {
    let me = std::thread::current().id();
    let mine: Vec<Orphan> = {
        let mut all = ORPHANS.lock();
        if all.is_empty() {
            return;
        }
        let (mine, others) = std::mem::take(&mut *all).into_iter().partition(|o| o.owner == me);
        *all = others;
        mine
    };
    for o in mine {
        // SAFETY: on its owner thread; no one reaches it.
        unsafe { (o.drop)(o.record) };
    }
}

impl<T> Drop for StateCell<T> {
    fn drop(&mut self) {
        // Nothing holds it in a snapshot's writes (they hold it alive) or in the prune list (weakly); its records
        // that were unlinked are in the retired list, freed from there.
        let mut r = *self.first.get_mut();
        if let Some(owner) = self.owner
            && owner != std::thread::current().id()
        {
            // (a confined cell's last handle dropped elsewhere — a snapshot's write list, say: its values are freed
            // on their thread)
            let mut orphans = ORPHANS.lock();
            while !r.is_null() {
                // SAFETY: linked records are this cell's; no reader can be left (no Arc).
                let next = unsafe { (*r).next.load(Relaxed) };
                orphans.push(Orphan { record: r, drop: drop_record::<T>, owner });
                r = next;
            }
            return;
        }
        while !r.is_null() {
            // SAFETY: linked records are this cell's, made by new_record::<T>; no reader can be left (no Arc).
            let next = unsafe { (*r).next.load(Relaxed) };
            unsafe { drop_record::<T>(r) };
            r = next;
        }
    }
}

impl<T: Clone + 'static> StateObject for Erased<T> {
    fn key(&self) -> StateKey {
        self.0.key
    }
    fn first(&self) -> &AtomicPtr<RecordHead> {
        &self.0.first
    }
    unsafe fn copy_record(&self, r: *const RecordHead, id: i64) -> *mut RecordHead {
        check_owner(self.0.owner, "a snapshot applied on this thread copies");
        let value = unsafe { &(*(r as *const Record<T>)).value };
        new_record(value.clone(), id)
    }
    unsafe fn mergeable(&self, current: *const RecordHead, applied: *const RecordHead) -> bool {
        check_owner(self.0.owner, "a snapshot applied on this thread compares");
        let (a, b) = unsafe { (&(*(current as *const Record<T>)).value, &(*(applied as *const Record<T>)).value) };
        self.0.equivalent.is_some_and(|eq| eq(a, b))
    }
    fn dropper(&self) -> unsafe fn(*mut RecordHead) {
        drop_record::<T>
    }
    fn owner(&self) -> Option<ThreadId> {
        self.0.owner
    }
}

impl<T: Clone + 'static> StateCell<T> {
    /// A cell; `owner`: the thread its values are confined to (`None` only for a `Send + Sync` T).
    pub(crate) fn new(value: T, equivalent: Option<fn(&T, &T) -> bool>, owner: Option<ThreadId>) -> Arc<StateCell<T>> {
        Arc::new(StateCell {
            key: StateKey::new(),
            first: AtomicPtr::new(new_record(value, PREEXISTING_SNAPSHOT)),
            equivalent,
            owner,
            _value: PhantomData,
        })
    }

    pub(crate) fn owner(&self) -> Option<ThreadId> {
        self.owner
    }

    /// It as the snapshot system reads it.
    fn erased(&self) -> &Erased<T> {
        // SAFETY: `Erased<T>` is a transparent wrapper of `StateCell<T>`.
        unsafe { &*(self as *const StateCell<T> as *const Erased<T>) }
    }

    /// It as the snapshot system holds it.
    fn object(self: &Arc<Self>) -> Arc<dyn StateObject> {
        // SAFETY: `Erased<T>` is a transparent wrapper of `StateCell<T>` (the same layout); the same allocation.
        let erased: Arc<Erased<T>> = unsafe { Arc::from_raw(Arc::into_raw(self.clone()) as *const Erased<T>) };
        erased
    }

    pub(crate) fn key(&self) -> StateKey {
        self.key
    }

    /// Runs `f` with the value the snapshot this thread runs in sees (told to its read observer if `observe`,
    /// and then a dependency of the derived state computing, if one is).
    pub(crate) fn with<R>(self: &Arc<Self>, observe: bool, f: impl FnOnce(&T) -> R) -> R {
        check_owner(self.owner, "read");
        with_readable(self.erased(), observe, |r| {
            if observe && crate::read::collecting_dependencies() {
                // SAFETY: a record this snapshot reads (see with_readable).
                let stamp = unsafe { (*r).stamp.load(Acquire) };
                crate::read::record_dependency(self.key, crate::read::Dependency::State(self.object(), stamp));
            }
            f(unsafe { &(*(r as *const Record<T>)).value })
        })
    }

    /// Sets the value, unless it is equivalent to the one it has (its policy).
    pub(crate) fn set(self: &Arc<Self>, value: T) {
        check_owner(self.owner, "written");
        let this = self.object();
        write(&this, |current| {
            // SAFETY: `current` is this cell's record, made by new_record::<T>; others may be reading it.
            let current = unsafe { &(*(current as *const Record<T>)).value };
            if self.equivalent.is_some_and(|eq| eq(current, &value)) {
                return Write::Nothing;
            }
            Write::Value(Box::new(move |own: Option<*mut RecordHead>, id| match own {
                Some(own) => {
                    unsafe { (*(own as *mut Record<T>)).value = value };
                    None
                }
                None => Some(new_record(value, id)),
            }))
        });
    }

    /// Changes the value in place (a copy of it, if this snapshot doesn't have one of its own yet).
    pub(crate) fn update(self: &Arc<Self>, mutate: impl FnOnce(&mut T)) {
        check_owner(self.owner, "written");
        let this = self.object();
        write(&this, |current| {
            let current = current as *mut Record<T>;
            Write::Value(Box::new(move |own: Option<*mut RecordHead>, id| match own {
                Some(own) => {
                    mutate(unsafe { &mut (*(own as *mut Record<T>)).value });
                    None
                }
                None => {
                    let mut value = unsafe { (*current).value.clone() };
                    mutate(&mut value);
                    Some(new_record(value, id))
                }
            }))
        });
    }

    pub(crate) fn records(&self) -> usize {
        let mut n = 0;
        let mut r = self.first.load(Acquire);
        while !r.is_null() {
            n += 1;
            r = unsafe { (*r).next.load(Acquire) };
        }
        n
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Global state

/// The global snapshot as the UI thread reads it without the lock: replaced (never changed) under it, the old
/// one freed by the UI thread between frames.
struct GlobalView {
    id: i64,
    invalid: IdSet,
}

pub type ReadObserver = Arc<dyn Fn(StateKey) + Send + Sync>;
pub type WriteObserver = Arc<dyn Fn(StateKey) + Send + Sync>;
/// Told the states a snapshot changed as it is applied (the global one at `send_apply_notifications`), on the
/// thread applying it.
pub type ApplyObserver = Arc<dyn Fn(&[StateKey]) + Send + Sync>;

/// A record unlinked from its state, freed once no snapshot open when it was unlinked is left.
struct Retired {
    record: *mut RecordHead,
    drop: unsafe fn(*mut RecordHead),
    epoch: i64,
    /// A confined state's: freed only on this thread.
    owner: Option<ThreadId>,
}
// SAFETY: a record of a Send + Sync value; freed on the UI thread.
unsafe impl Send for Retired {}

struct RetiredView(*mut GlobalView);
// SAFETY: a view no one reads any more; freed on the UI thread.
unsafe impl Send for RetiredView {}

/// Writes of states, by key, in the order they were first written.
#[derive(Default)]
struct Modified {
    objects: Vec<Arc<dyn StateObject>>,
    keys: HashSet<StateKey>,
}

impl Modified {
    fn add(&mut self, object: &Arc<dyn StateObject>) {
        if self.keys.insert(object.key()) {
            self.objects.push(object.clone());
        }
    }
    fn clear(&mut self) {
        self.objects.clear();
        self.keys.clear();
    }
}

#[derive(Default)]
struct Globals {
    next_id: i64,
    /// Snapshots with ids of their own, not applied or disposed: their ids and what they don't see.
    open: Vec<(i64, IdSet)>,
    retired_views: Vec<RetiredView>,
    /// The global snapshot's writes since it was last applied.
    global_modified: Modified,
    global_changed: Vec<StateKey>,
    apply_observers: Vec<Weak<dyn Fn(&[StateKey]) + Send + Sync>>,
    /// What may hold records no one reads any more.
    to_prune: HashMap<StateKey, Weak<dyn StateObject>>,
    retired: Vec<Retired>,
}

fn globals() -> &'static ReentrantMutex<RefCell<Globals>> {
    static G: OnceLock<ReentrantMutex<RefCell<Globals>>> = OnceLock::new();
    G.get_or_init(|| {
        ReentrantMutex::new(RefCell::new(Globals { next_id: PREEXISTING_SNAPSHOT + 2, ..Default::default() }))
    })
}

fn lock() -> ReentrantMutexGuard<'static, RefCell<Globals>> {
    globals().lock()
}

static VIEW: AtomicPtr<GlobalView> = AtomicPtr::new(ptr::null_mut()); // null: first_view()
static GLOBAL_DIRTY: AtomicBool = AtomicBool::new(false);
static UI_THREAD: OnceLock<ThreadId> = OnceLock::new();

fn global_write_observer() -> &'static Mutex<Option<Arc<dyn Fn() + Send + Sync>>> {
    static O: Mutex<Option<Arc<dyn Fn() + Send + Sync>>> = Mutex::new(None);
    &O
}

thread_local! {
    /// The snapshot this thread runs in (null: the global one); kept alive by the `Entered` that set it.
    static CURRENT: Cell<*const SnapshotCore> = const { Cell::new(ptr::null()) };
    /// How many record values this thread is reading (borrowed by a `with`): nothing it may be reading is freed
    /// or written in place meanwhile.
    static READING: Cell<u32> = const { Cell::new(0) };
}

fn first_view() -> &'static GlobalView {
    static FIRST: OnceLock<GlobalView> = OnceLock::new();
    FIRST.get_or_init(|| GlobalView { id: PREEXISTING_SNAPSHOT + 1, invalid: IdSet::default() })
}

fn load_view(order: std::sync::atomic::Ordering) -> &'static GlobalView {
    let v = VIEW.load(order);
    // SAFETY: a view is freed only on the UI thread between frames, when none of its readers is left (the UI
    // thread's own reads are over; other threads read it under the lock).
    if v.is_null() { first_view() } else { unsafe { &*v } }
}

fn on_ui_thread() -> bool {
    UI_THREAD.get().is_none_or(|t| *t == std::thread::current().id())
}

/// Makes this thread the UI thread: the one the hosts run on, which reads states without the lock and frees
/// what no one reads. The first call decides; later ones do nothing.
pub fn set_ui_thread() {
    let _g = lock();
    let _ = UI_THREAD.set(std::thread::current().id());
}

/// Whether the global snapshot has writes not applied yet (a frame is wanted to tell them).
pub fn global_snapshot_changed() -> bool {
    GLOBAL_DIRTY.load(Acquire)
}

/// Called (outside the lock, on the writing thread) at the first write to the global snapshot after it was
/// applied: the hosts ask for a frame.
pub fn set_global_write_observer(observer: impl Fn() + Send + Sync + 'static) {
    *global_write_observer().lock() = Some(Arc::new(observer));
}

fn open_ids(g: &Globals) -> IdSet {
    let mut ids = IdSet::default();
    for (id, _) in &g.open {
        ids.add(*id);
    }
    ids
}

/// A new global view (under the lock): its invalid set the open snapshots', at `id`.
fn replace_view(g: &mut Globals, id: i64) {
    WRITES.fetch_add(1, Release); // (what the global snapshot sees may differ: a snapshot applied, say)
    let fresh = Box::into_raw(Box::new(GlobalView { id, invalid: open_ids(g) }));
    let old = VIEW.swap(fresh, Release);
    if !old.is_null() {
        g.retired_views.push(RetiredView(old));
    }
}

/// Takes out the records of `object` no snapshot reads (under the lock, on the UI thread): what the global
/// snapshot reads is what every snapshot taken from now on will, and the open ones keep what they read.
fn prune(g: &mut Globals, object: &dyn StateObject) {
    let view = load_view(Relaxed);
    let mut read = vec![find(object, view.id, &view.invalid)];
    for (id, invalid) in &g.open {
        read.push(find(object, *id, invalid));
    }
    let mut previous: *mut RecordHead = ptr::null_mut();
    let mut r = object.first().load(Relaxed);
    while !r.is_null() {
        let next = unsafe { (*r).next.load(Relaxed) };
        if read.contains(&r) {
            previous = r;
        } else {
            let link = if previous.is_null() { object.first() } else { unsafe { &(*previous).next } };
            link.store(next, Release);
            g.retired.push(Retired { record: r, drop: object.dropper(), epoch: g.next_id, owner: object.owner() });
        }
        r = next;
    }
}

/// Between frames, on the UI thread (under the lock): what no one can be reading any more freed.
fn collect(g: &mut Globals) {
    for (_, object) in std::mem::take(&mut g.to_prune) {
        if let Some(object) = object.upgrade() {
            prune(g, &*object);
        }
    }
    free_retired(g, true);
    for v in g.retired_views.drain(..) {
        // SAFETY: replaced; the UI thread isn't reading it (between frames), others read under the lock.
        drop(unsafe { Box::from_raw(v.0) });
    }
}

/// Frees the retired records no snapshot can reach any more that may be freed on this thread (a confined state's
/// only on its owner thread; off the UI thread only those of the states confined to it, as the UI thread may be
/// reading the others without the lock).
fn free_retired(g: &mut Globals, ui_thread: bool) {
    let me = std::thread::current().id();
    let oldest_open = g.open.iter().map(|(id, _)| *id).min().unwrap_or(i64::MAX);
    g.retired.retain(|r| {
        let ours = match r.owner {
            Some(o) => o == me,
            None => ui_thread,
        };
        if r.epoch > oldest_open || !ours {
            return true;
        }
        // SAFETY: unlinked, and every snapshot that could still reach it is closed.
        unsafe { (r.drop)(r.record) };
        false
    });
}

fn notify(changed: &[StateKey]) {
    if changed.is_empty() {
        return;
    }
    let observers: Vec<ApplyObserver> = {
        let g = lock();
        let mut g = g.borrow_mut();
        let mut live = Vec::new();
        g.apply_observers.retain(|w| match w.upgrade() {
            Some(o) => {
                live.push(o);
                true
            }
            None => false,
        });
        live
    };
    for o in observers {
        o(changed);
    }
}

/// While it lives, this thread reads record values (see READING).
struct Reading;
impl Reading {
    fn new() -> Reading {
        READING.with(|r| r.set(r.get() + 1));
        Reading
    }
}
impl Drop for Reading {
    fn drop(&mut self) {
        READING.with(|r| r.set(r.get() - 1));
    }
}

fn reading() -> bool {
    READING.with(|r| r.get() > 0)
}

/// Runs `f` with the record of `object` the snapshot this thread runs in sees (told to its read observer if
/// `observe`).
fn with_readable<R>(object: &dyn StateObject, observe: bool, f: impl FnOnce(*mut RecordHead) -> R) -> R {
    let current = CURRENT.with(|c| c.get());
    if !current.is_null() {
        // SAFETY: the Entered that set it holds it.
        let s = unsafe { &*current };
        if observe && let Some(o) = &s.read_observer {
            o(object.key());
        }
        let r = find(object, s.id, &s.invalid);
        let _reading = Reading::new();
        return f(r);
    }
    if !on_ui_thread() {
        // A read meant for a snapshot: at least not torn by a write.
        let _g = lock();
        let view = load_view(Relaxed);
        let r = find(object, view.id, &view.invalid);
        return f(r);
    }
    let view = load_view(Acquire);
    let r = find(object, view.id, &view.invalid);
    let _reading = Reading::new();
    f(r)
}

/// What a write does once it saw the current record: nothing, or set a value — in place in the record given
/// (this snapshot's own), or in a new record it returns (made with the id given).
enum Write<'a> {
    Nothing,
    Value(Box<dyn FnOnce(Option<*mut RecordHead>, i64) -> Option<*mut RecordHead> + 'a>),
}

/// Writes `object` in the snapshot this thread runs in. `decide` sees the record it sees now.
fn write<'a>(object: &Arc<dyn StateObject>, decide: impl FnOnce(*mut RecordHead) -> Write<'a>) {
    let guard = lock();
    let current_snapshot = CURRENT.with(|c| c.get());
    let mut first_global_write = false;
    let observer;
    if !current_snapshot.is_null() {
        // SAFETY: the Entered that set it holds it.
        let s = unsafe { &*current_snapshot };
        assert!(!s.read_only, "kompoz: a state written in a read-only snapshot");
        let current = find(&**object, s.id, &s.invalid);
        let Write::Value(set) = decide(current) else { return };
        let own = unsafe { (*current).id.load(Relaxed) } == s.id && !reading();
        match set(own.then_some(current), s.id) {
            Some(fresh) => prepend(&**object, fresh, s.id),
            None => unsafe { (*current).stamp.store(next_stamp(), Release) },
        }
        WRITES.fetch_add(1, Release);
        s.state.lock().modified.add(object);
        observer = s.write_observer.clone();
    } else {
        let view = load_view(Relaxed);
        let current = find(&**object, view.id, &view.invalid);
        let Write::Value(set) = decide(current) else { return };
        if !on_ui_thread() {
            // An id of its own below the global snapshot's next one: seen by it (and by what is taken after),
            // not by the snapshots open now; never in place (the UI thread may be reading it).
            let id = {
                let mut g = guard.borrow_mut();
                g.next_id += 1;
                g.next_id - 1
            };
            if let Some(fresh) = set(None, id) {
                prepend(&**object, fresh, id);
            }
            WRITES.fetch_add(1, Release);
            let mut g = guard.borrow_mut();
            let next = g.next_id;
            g.next_id += 1;
            replace_view(&mut g, next);
        } else {
            let own = unsafe { (*current).id.load(Relaxed) } == view.id && !reading();
            match set(own.then_some(current), view.id) {
                Some(fresh) => prepend(&**object, fresh, view.id),
                None => unsafe { (*current).stamp.store(next_stamp(), Release) },
            }
            WRITES.fetch_add(1, Release);
        }
        guard.borrow_mut().global_modified.add(object);
        first_global_write = !GLOBAL_DIRTY.swap(true, AcqRel);
        observer = None;
    }
    drop(guard);
    if let Some(o) = observer {
        o(object.key());
    }
    if first_global_write {
        let o = global_write_observer().lock().clone();
        if let Some(o) = o {
            o();
        }
    }
}

/// A plain object that isn't kept per snapshot (an animation's, a lazy list's) changed: its readers are told as
/// the snapshot it changed in is applied.
pub fn state_changed(key: StateKey) {
    let first = {
        let g = lock();
        let current = CURRENT.with(|c| c.get());
        // SAFETY: the Entered that set it holds it.
        if let Some(s) = unsafe { current.as_ref() }
            && !s.read_only
        {
            s.state.lock().changed.push(key);
            false
        } else {
            g.borrow_mut().global_changed.push(key);
            !GLOBAL_DIRTY.swap(true, AcqRel)
        }
    };
    if first {
        let o = global_write_observer().lock().clone();
        if let Some(o) = o {
            o();
        }
    }
}

/// The stamp of the record of `object` the snapshot this thread runs in sees (read by no observer): what a
/// derived state compares with the stamp it saw when it computed.
pub(crate) fn readable_stamp(object: &dyn StateObject) -> u64 {
    // SAFETY: a record this snapshot reads (see with_readable).
    with_readable(object, false, |r| unsafe { (*r).stamp.load(Acquire) })
}

/// Where this thread reads (the snapshot it runs in, or the global one as it is now) and how many writes there
/// were so far: the same pair later says no state read here can have changed meanwhile.
pub(crate) fn read_epoch() -> (i64, u64) {
    let writes = WRITES.load(Acquire);
    let current = CURRENT.with(|c| c.get());
    // SAFETY: the Entered that set it holds it.
    let id = match unsafe { current.as_ref() } {
        Some(s) => s.id,
        None if !on_ui_thread() => {
            let _g = lock();
            load_view(Relaxed).id
        }
        None => load_view(Acquire).id,
    };
    (id, writes)
}

// ---------------------------------------------------------------------------------------------------------------
// Snapshots

#[derive(Default)]
struct CoreState {
    applied: bool,
    disposed: bool,
    open: bool,
    modified: Modified,
    changed: Vec<StateKey>,
}

struct SnapshotCore {
    id: i64,
    invalid: IdSet,
    read_only: bool,
    /// A nested mutable snapshot's: what it applies to.
    parent: Option<Arc<SnapshotCore>>,
    read_observer: Option<ReadObserver>,
    write_observer: Option<WriteObserver>,
    state: Mutex<CoreState>,
}

impl Drop for SnapshotCore {
    fn drop(&mut self) {
        // Dropped without dispose(): disposed.
        if !self.state.get_mut().disposed {
            let g = lock();
            dispose_locked(&mut g.borrow_mut(), self);
        }
    }
}

fn close(g: &mut Globals, s: &SnapshotCore, st: &mut CoreState) {
    g.open.retain(|(id, _)| *id != s.id);
    st.open = false;
}

fn dispose_locked(g: &mut Globals, s: &SnapshotCore) {
    let mut st = s.state.lock();
    if st.disposed {
        return;
    }
    st.disposed = true;
    if !st.applied && !s.read_only {
        // Its writes dropped.
        for object in &st.modified.objects {
            let mut r = object.first().load(Relaxed);
            while !r.is_null() {
                let head = unsafe { &*r };
                if head.id.load(Relaxed) == s.id {
                    head.id.store(INVALID_SNAPSHOT, Release);
                }
                r = head.next.load(Relaxed);
            }
            g.to_prune.insert(object.key(), Arc::downgrade(object));
        }
        st.modified.clear();
    }
    if st.open {
        close(g, s, &mut st);
        if s.parent.is_none() {
            let id = load_view(Relaxed).id;
            replace_view(g, id);
        }
    }
}

/// What applying a mutable snapshot gave: `Failure` if a state it wrote was written meanwhile by another
/// snapshot to a value its policy doesn't count as the same.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnapshotApplyResult {
    Success,
    Failure,
}

/// `with_mutable_snapshot`'s apply failed (see [`SnapshotApplyResult`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SnapshotApplyConflict;

impl std::fmt::Display for SnapshotApplyConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a state written in the snapshot was written meanwhile elsewhere")
    }
}

impl std::error::Error for SnapshotApplyConflict {}

/// A view of the states as they were when it was taken (Compose's Snapshot): what runs in it with
/// [`enter`](Snapshot::enter) reads them so, whatever is written meanwhile. Disposed with
/// [`dispose`](Snapshot::dispose), or when its last handle is dropped.
#[derive(Clone)]
pub struct Snapshot {
    core: Arc<SnapshotCore>,
}

/// A snapshot to write in too; its writes are seen by others once it is [applied](MutableSnapshot::apply).
#[derive(Clone)]
pub struct MutableSnapshot {
    snapshot: Snapshot,
}

impl std::ops::Deref for MutableSnapshot {
    type Target = Snapshot;
    fn deref(&self) -> &Snapshot {
        &self.snapshot
    }
}

/// Told what every applied snapshot changed while it lives (or until [`dispose`](ObserverHandle::dispose)).
pub struct ObserverHandle {
    _registration: Option<ApplyObserver>,
}

impl ObserverHandle {
    pub fn dispose(mut self) {
        self._registration = None;
    }
}

impl Snapshot {
    /// A read-only snapshot of what this thread sees now (inside a snapshot: of that one).
    pub fn take_snapshot(read_observer: Option<ReadObserver>) -> Snapshot {
        let g = lock();
        let mut g = g.borrow_mut();
        let current = CURRENT.with(|c| c.get());
        // SAFETY: the Entered that set it holds it.
        if let Some(parent) = unsafe { current.as_ref() } {
            // What that one sees.
            return Snapshot {
                core: Arc::new(SnapshotCore {
                    id: parent.id,
                    invalid: parent.invalid.clone(),
                    read_only: true,
                    parent: None,
                    read_observer,
                    write_observer: None,
                    state: Mutex::new(CoreState::default()),
                }),
            };
        }
        let id = g.next_id;
        g.next_id += 1;
        let invalid = open_ids(&g);
        g.open.push((id, invalid.clone()));
        let next = g.next_id;
        g.next_id += 1;
        replace_view(&mut g, next); // (the global snapshot's writes from now on: newer than it)
        Snapshot {
            core: Arc::new(SnapshotCore {
                id,
                invalid,
                read_only: true,
                parent: None,
                read_observer,
                write_observer: None,
                state: Mutex::new(CoreState { open: true, ..Default::default() }),
            }),
        }
    }

    /// A snapshot to write in too; its writes are seen by others once it is applied. Taken in a mutable
    /// snapshot, it sees what that one sees and applies to it.
    pub fn take_mutable_snapshot(
        read_observer: Option<ReadObserver>,
        write_observer: Option<WriteObserver>,
    ) -> MutableSnapshot {
        let g = lock();
        let mut g = g.borrow_mut();
        let current = CURRENT.with(|c| c.get());
        let id = g.next_id;
        g.next_id += 1;
        let (parent, invalid) = match unsafe { current.as_ref() } {
            Some(p) => {
                assert!(!p.read_only, "kompoz: a mutable snapshot taken in a read-only one");
                // Sees what its parent sees, not what was written since it was taken.
                let mut invalid = p.invalid.clone();
                invalid.add_range(p.id + 1, id - 1);
                // SAFETY: `p` is held by the Entered that set it, as an Arc: one more for the child.
                let parent = unsafe {
                    Arc::increment_strong_count(current);
                    Arc::from_raw(current)
                };
                (Some(parent), invalid)
            }
            None => (None, open_ids(&g)),
        };
        g.open.push((id, invalid.clone()));
        if parent.is_none() {
            let next = g.next_id;
            g.next_id += 1;
            replace_view(&mut g, next);
        }
        MutableSnapshot {
            snapshot: Snapshot {
                core: Arc::new(SnapshotCore {
                    id,
                    invalid,
                    read_only: false,
                    parent,
                    read_observer,
                    write_observer,
                    state: Mutex::new(CoreState { open: true, ..Default::default() }),
                }),
            },
        }
    }

    /// Runs `block` with what it reads told to no one (Compose's companion `Snapshot.withoutReadObservation`; the
    /// free [`without_read_observation`](crate::without_read_observation) is the same).
    pub fn without_read_observation<R>(block: impl FnOnce() -> R) -> R {
        crate::read::without_read_observation(block)
    }

    /// Runs `block` in a new mutable snapshot and applies it: its writes are seen together, or not at all.
    pub fn with_mutable_snapshot<R>(block: impl FnOnce() -> R) -> Result<R, SnapshotApplyConflict> {
        let snapshot = Snapshot::take_mutable_snapshot(None, None);
        let result = snapshot.enter(block);
        let applied = snapshot.apply();
        snapshot.dispose();
        match applied {
            SnapshotApplyResult::Success => Ok(result),
            SnapshotApplyResult::Failure => Err(SnapshotApplyConflict),
        }
    }

    /// Applies the global snapshot: what was written outside any snapshot is told to the apply observers. The
    /// hosts do it at every frame; on the UI thread it also frees what no one reads any more (on any thread, what
    /// no one reads of the states confined to it).
    pub fn send_apply_notifications() {
        let mut changed = Vec::new();
        {
            let g = lock();
            let mut g = g.borrow_mut();
            if GLOBAL_DIRTY.swap(false, AcqRel) {
                let modified = std::mem::take(&mut g.global_modified);
                for object in &modified.objects {
                    changed.push(object.key());
                    g.to_prune.insert(object.key(), Arc::downgrade(object));
                }
                changed.append(&mut g.global_changed);
                let next = g.next_id;
                g.next_id += 1;
                replace_view(&mut g, next); // written from now on: new records
            }
            if CURRENT.with(|c| c.get().is_null()) && !reading() {
                if on_ui_thread() {
                    collect(&mut g);
                } else {
                    free_retired(&mut g, false); // (the states confined to this thread)
                }
                free_orphans();
            }
        }
        notify(&changed);
    }

    /// Calls `observer` with what every applied snapshot changed, until the handle is dropped.
    pub fn register_apply_observer(observer: impl Fn(&[StateKey]) + Send + Sync + 'static) -> ObserverHandle {
        let observer: ApplyObserver = Arc::new(observer);
        let g = lock();
        g.borrow_mut().apply_observers.push(Arc::downgrade(&observer));
        ObserverHandle { _registration: Some(observer) }
    }

    /// Runs `block` in this snapshot (inside it, states are read and written as it sees them).
    pub fn enter<R>(&self, block: impl FnOnce() -> R) -> R {
        let _entered = Entered::new(self);
        block()
    }

    pub fn dispose(&self) {
        let g = lock();
        dispose_locked(&mut g.borrow_mut(), &self.core);
    }

    pub fn read_only(&self) -> bool {
        self.core.read_only
    }
}

impl MutableSnapshot {
    /// Makes its writes seen by others (the global snapshot's, or its parent's if it was taken in another
    /// mutable snapshot), unless one conflicts. It can't be applied twice.
    pub fn apply(&self) -> SnapshotApplyResult {
        self.apply_telling(true)
    }

    /// Applies it without telling the apply observers what it changed: its writes are seen from now on, but no
    /// reader is invalidated by them (what the layout check does with what its measures write again: the nodes it
    /// laid out stay as it left them, and so must what they share with the states — a lazy list's measured items —
    /// without a change that would lay its readers out, and check them, again).
    #[doc(hidden)]
    pub fn apply_unobserved(&self) -> SnapshotApplyResult {
        self.apply_telling(false)
    }

    fn apply_telling(&self, tell: bool) -> SnapshotApplyResult {
        let s = &self.snapshot.core;
        let mut changed = Vec::new();
        {
            let g = lock();
            let mut g = g.borrow_mut();
            let mut st = s.state.lock();
            assert!(!st.applied && !st.disposed, "kompoz: a snapshot applied twice, or after it was disposed");
            let (into_id, into) = match &s.parent {
                Some(p) => (p.id, p.invalid.clone()),
                None => {
                    let v = load_view(Relaxed);
                    (v.id, v.invalid.clone())
                }
            };
            let mut before = s.invalid.clone(); // what it saw before it wrote
            before.add(s.id);
            for object in &st.modified.objects {
                let current = find(&**object, into_id, &into);
                if current == find(&**object, s.id, &before) {
                    continue; // not written meanwhile
                }
                let applied = find(&**object, s.id, &s.invalid);
                if !unsafe { object.mergeable(current, applied) } {
                    return SnapshotApplyResult::Failure;
                }
            }
            st.applied = true;
            close(&mut g, s, &mut st);
            if let Some(p) = &s.parent {
                // Its writes become its parent's.
                let mut pst = p.state.lock();
                for object in &st.modified.objects {
                    let applied = find(&**object, s.id, &s.invalid);
                    let copy = unsafe { object.copy_record(applied, p.id) };
                    prepend(&**object, copy, p.id);
                    WRITES.fetch_add(1, Release);
                    unsafe { (*applied).id.store(INVALID_SNAPSHOT, Release) };
                    pst.modified.add(object);
                    g.to_prune.insert(object.key(), Arc::downgrade(object));
                }
                let mut more = std::mem::take(&mut st.changed);
                pst.changed.append(&mut more);
                return SnapshotApplyResult::Success;
            }
            let id = load_view(Relaxed).id;
            replace_view(&mut g, id); // its writes seen from now on
            for object in &st.modified.objects {
                changed.push(object.key());
                g.to_prune.insert(object.key(), Arc::downgrade(object));
            }
            changed.append(&mut st.changed);
        }
        if tell {
            notify(&changed);
        }
        SnapshotApplyResult::Success
    }
}

/// In a snapshot while it lives (what `enter` does around its block).
struct Entered {
    previous: *const SnapshotCore,
    _held: Arc<SnapshotCore>,
}

impl Entered {
    fn new(snapshot: &Snapshot) -> Entered {
        let held = snapshot.core.clone();
        let previous = CURRENT.with(|c| c.replace(Arc::as_ptr(&held)));
        Entered { previous, _held: held }
    }
}

impl Drop for Entered {
    fn drop(&mut self) {
        CURRENT.with(|c| c.set(self.previous));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_sets_merge_ranges() {
        let mut s = IdSet::default();
        s.add(5);
        s.add(7);
        assert!(s.contains(5) && !s.contains(6) && s.contains(7));
        s.add(6);
        assert_eq!(s.ranges, vec![(5, 7)]);
        s.add_range(1, 3);
        s.add_range(10, 12);
        assert_eq!(s.ranges, vec![(1, 3), (5, 7), (10, 12)]);
        s.add_range(2, 11);
        assert_eq!(s.ranges, vec![(1, 12)]);
        assert!(!s.contains(0) && s.contains(1) && s.contains(12) && !s.contains(13));
    }
}

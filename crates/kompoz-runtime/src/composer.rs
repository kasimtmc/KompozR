//! The composition: a tree of groups (one per composable call; the call site plus the order among the parent's
//! calls is a group's identity), scopes that record what their bodies read, and smart recomposition — when a
//! state changes only the scopes that read it run again, and a scope whose arguments equal last time's is
//! skipped when its parent runs.
//!
//! A group's children are kept in call order. Composing a group again walks its old children alongside the new
//! calls: the next old child is taken if its key matches (the common case, no lookup), else the key is looked
//! up among those left; whatever is left at the end has left the composition.

use crate::context::{CompositionContext, ContextData, PausedComposition, Recomposer};
use crate::read::observe_reads;
use crate::snapshot::{ObserverHandle, Snapshot, SnapshotApplyResult, StateKey};
use parking_lot::Mutex;
use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::panic::Location;
use std::rc::{Rc, Weak};
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// Keys

pub(crate) fn mix_key(a: u64, b: u64) -> u64 {
    let mut h = a ^ (b.wrapping_add(0x9e3779b97f4a7c15).wrapping_add(a << 6).wrapping_add(a >> 2));
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h
}

/// FNV-1a: stable across runs and builds (saved state is found again by the keys made from it).
pub(crate) struct StableHasher(u64);

impl Default for StableHasher {
    fn default() -> Self {
        StableHasher(0xcbf29ce484222325)
    }
}

impl std::hash::Hasher for StableHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ *b as u64).wrapping_mul(0x100000001b3);
        }
    }
}

pub(crate) fn stable_hash<K: std::hash::Hash + ?Sized>(k: &K) -> u64 {
    let mut h = StableHasher::default();
    k.hash(&mut h);
    std::hash::Hasher::finish(&h)
}

/// A call site's key: the file, line and column.
pub(crate) fn key_of(site: &Location<'_>) -> u64 {
    let mut h = StableHasher::default();
    std::hash::Hasher::write(&mut h, site.file().as_bytes());
    mix_key(h.0, ((site.line() as u64) << 32) | site.column() as u64)
}

// ---------------------------------------------------------------------------------------------------------------
// Nodes

/// What a composable emits (a layout node): the composition keeps it in its group and tells it which nodes are
/// below it, in call order, whenever they change.
pub trait Node: Any {
    /// The nodes emitted below it (looking through the groups that emit none), in call order.
    fn set_children(&self, children: &[Rc<dyn Node>]);

    /// Its group is composed again for other content (Compose's `ComposeNodeLifecycleCallback.onReuse`: reusable
    /// content whose key changed, a reused composition): it is about to be updated as a new node would be.
    fn on_reuse(&self) {}

    /// Its content was deactivated (`onDeactivate`: a `ReusableContentHost` turned off, a composition
    /// deactivated): kept for reuse, told after the composition, in the order its group's values are forgotten.
    fn on_deactivate(&self) {}

    /// It left the composition for good (`onRelease`), told after the composition as its group's values are
    /// forgotten: the content it owns (a subcomposing layout's compositions) goes with it.
    fn on_release(&self) {}
}

// ---------------------------------------------------------------------------------------------------------------
// Groups and scopes

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct GroupId(u32);
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ScopeId(u32);

struct Slab<T> {
    items: Vec<Option<T>>,
    free: Vec<u32>,
}

impl<T> Default for Slab<T> {
    fn default() -> Self {
        Slab { items: Vec::new(), free: Vec::new() }
    }
}

impl<T> Slab<T> {
    fn insert(&mut self, item: T) -> u32 {
        match self.free.pop() {
            Some(i) => {
                self.items[i as usize] = Some(item);
                i
            }
            None => {
                self.items.push(Some(item));
                (self.items.len() - 1) as u32
            }
        }
    }
    fn remove(&mut self, i: u32) -> T {
        let item = self.items[i as usize].take().expect("removed twice");
        self.free.push(i);
        item
    }
    fn get(&self, i: u32) -> &T {
        self.items[i as usize].as_ref().expect("gone")
    }
    fn get_mut(&mut self, i: u32) -> &mut T {
        self.items[i as usize].as_mut().expect("gone")
    }
    fn try_get(&self, i: u32) -> Option<&T> {
        self.items.get(i as usize).and_then(|x| x.as_ref())
    }
}

/// What `remember` keeps in a group.
pub(crate) struct SlotValue {
    pub(crate) value: Box<dyn Any>,
    pub(crate) observer: Option<Observed>,
}

/// A remembered value's observer, and whether it was told it entered (else it is abandoned, not forgotten).
pub(crate) struct Observed {
    observer: Rc<dyn RememberObserver>,
    entered: Rc<std::cell::Cell<bool>>,
}

/// What a remembered value is told as it enters and leaves the composition (Compose's RememberObserver; see
/// [`remember_observer`](crate::remember_observer)). Both after the composition: what left forgotten first (in
/// the reverse of the order it was composed in), then what entered remembered (in order), before the side
/// effects. A remembered value's `Drop` runs where `on_forgotten` would. One remembered by a composition that
/// never got to enter (a paused composition cancelled, or disposed before it was applied) is told
/// `on_abandoned` instead of either.
pub trait RememberObserver {
    fn on_remembered(&self) {}
    fn on_forgotten(&self) {}
    fn on_abandoned(&self) {}
}

/// A movable content's identity: its latest content and the group it is composed in (none while it isn't).
pub(crate) struct MovableData {
    pub(crate) content: RefCell<Option<Rc<dyn Fn()>>>,
    pub(crate) group: std::cell::Cell<Option<GroupId>>,
}

/// What a provider gives a local (Compose's ValueHolder): its value, and the key its readers are told under. A
/// dynamic local's provider keeps one holder and sets the value in it when it gives another (Compose's
/// DynamicValueHolder): whoever holds the holder (a layout node keeping the locals where it was composed) reads
/// the value as it is now, and whoever read it (a composable, a measure, a drawing) is told.
pub(crate) struct ValueHolder {
    pub(crate) key: StateKey,
    value: RefCell<Rc<dyn Any>>,
}

impl ValueHolder {
    pub(crate) fn new(value: Rc<dyn Any>) -> Rc<ValueHolder> {
        Rc::new(ValueHolder { key: StateKey::new(), value: RefCell::new(value) })
    }

    /// The value, the read told to whoever observes reads.
    pub(crate) fn read(&self) -> Rc<dyn Any> {
        crate::read::record_read(self.key);
        self.peek()
    }

    /// The value, read by no one.
    pub(crate) fn peek(&self) -> Rc<dyn Any> {
        self.value.borrow().clone()
    }

    /// Holds `value` from now on (the old one dropped outside the borrow).
    fn set(&self, value: Rc<dyn Any>) {
        let old = self.value.replace(value);
        drop(old);
    }
}

/// A value given to a composition local by a provider.
#[derive(Clone)]
pub(crate) struct Provided {
    /// Which local (its address).
    pub(crate) local: usize,
    pub(crate) holder: Rc<ValueHolder>,
    /// When two values are the same (the local's mutation policy).
    pub(crate) equals: Rc<dyn Fn(&dyn Any, &dyn Any) -> bool>,
    /// `provides_default`: only where nothing above provides one.
    pub(crate) if_missing: bool,
    /// A dynamic local's (Compose's compositionLocalOf): another value is set in the provider's holder; a static
    /// local's (staticCompositionLocalOf) comes in a holder of its own, and the scopes that read the old one (and
    /// the layouts that kept it) compose again.
    pub(crate) dynamic: bool,
    /// From a context provided again: the holder is the context's, shared (another one is a holder of its own).
    pub(crate) shared: bool,
}

impl Provided {
    /// Whether a layout keeping it has to be composed again to see another value (a static local's, a context's:
    /// their holders are replaced, not set).
    pub(crate) fn replaced_when_changed(&self) -> bool {
        !self.dynamic || self.shared
    }
}

struct Group {
    parent: Option<GroupId>,
    depth: u32,
    /// Its key under the parent (call site and occurrence).
    key: u64,
    /// Children in call order.
    children: Vec<GroupId>,
    slot: Option<SlotValue>,
    node: Option<Rc<dyn Node>>,
    /// The nodes it last told `node` were below it.
    node_children: Vec<*const ()>,
    scope: Option<ScopeId>,
    provided: Vec<Provided>,
    /// The cleanup a DisposableEffect registered.
    on_forget: Option<Box<dyn FnOnce()>>,
    /// When it was last called, among all calls.
    seq: u64,
    movable: Option<Rc<MovableData>>,
    /// The composition pass it was last called in.
    composed_in: u64,
    /// Matched while content was reused (its remembered values forgotten): the node it emits is told so
    /// ([`Node::on_reuse`]) before it is updated.
    reused: bool,
}

impl Group {
    fn new(parent: Option<GroupId>, depth: u32, key: u64) -> Group {
        Group {
            parent,
            depth,
            key,
            children: Vec::new(),
            slot: None,
            node: None,
            node_children: Vec::new(),
            scope: None,
            provided: Vec::new(),
            on_forget: None,
            seq: 0,
            movable: None,
            composed_in: 0,
            reused: false,
        }
    }
}

/// A restartable composable: its body (with what it takes), what the body read last time, and whether it has
/// to run again. A scope without a group observes reads for something else (a derived state): when one of them
/// changes it calls `on_invalidated` instead of recomposing.
struct Scope {
    group: Option<GroupId>,
    body: Option<Rc<dyn Fn(&dyn Any)>>,
    args: Option<Rc<dyn Any>>,
    on_invalidated: Option<Rc<dyn Fn()>>,
    /// States and composition locals read, each with its place among the key's readers.
    reads: Vec<(StateKey, u32)>,
    invalid: bool,
    /// Paused (or set) while content was being reused: run again in reuse, its old content's values forgotten
    /// and its nodes reused.
    reuse_on_restart: bool,
}

/// Who read each key, in the order they did (a scope gone leaves a hole, filled in as they mount up: removal is
/// quick and the order stays).
#[derive(Default)]
struct Readers {
    scopes: Vec<Option<ScopeId>>,
    holes: u32,
}

/// A group being composed: its old children not matched yet, and the new ones so far.
struct Frame {
    group: GroupId,
    old: Vec<Option<GroupId>>,
    cursor: usize,
    /// Built at the first call whose key isn't the next old child's: key → index in `old`.
    lookup: Option<HashMap<u64, usize>>,
    /// How many times each call site was called so far.
    occurrences: Vec<(u64, u32)>,
    children: Vec<GroupId>,
}

/// How many scopes ran and were skipped.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct RecompositionStats {
    /// Frames that recomposed anything.
    pub recompositions: u64,
    pub scopes_run: u64,
    pub scopes_skipped: u64,
}

pub(crate) struct ComposerState {
    groups: Slab<Group>,
    scopes: Slab<Scope>,
    frames: Vec<Frame>,
    locals: Vec<Provided>,
    after: Vec<Box<dyn FnOnce()>>,
    side: Vec<Box<dyn FnOnce()>>,
    /// What leaves the composition, told (in reverse) once it is over.
    forgets: Vec<Box<dyn FnOnce()>>,
    /// What leaves holding values of the app's, dropped after the forgets.
    garbage: Vec<Box<dyn Any>>,
    seq: u64,
    current_scope: Option<ScopeId>,
    readers: HashMap<StateKey, Readers>,
    invalid: Vec<ScopeId>,
    frame_scopes: Vec<ScopeId>,
    timed_scopes: Vec<(i64, ScopeId)>,
    /// Movable contents whose places left: waiting until the composition is over, one may be called further on.
    parked: Vec<GroupId>,
    /// The holders of dynamic locals its providers set: their readers here were invalidated as they were set, so
    /// their applied changes aren't taken again.
    set_holders: Vec<StateKey>,
    /// The derived states made here: they invalidate their readers here as they change, so the change they tell
    /// the apply observers (for their readers elsewhere) isn't taken again.
    derived_keys: HashSet<StateKey>,
    /// The readers a state written while composing had that the composition hadn't reached yet (Compose's
    /// imminent invalidations, its `observationsProcessed`): invalidated at once, composed in the same pass after
    /// the write, so the applied change isn't taken again for them.
    imminent: Vec<(StateKey, ScopeId)>,
    pass: u64,
    frame_time: i64,
    stats: RecompositionStats,
    root: Option<GroupId>,
    composing: bool,
    /// How deep in reused content this composes (Compose's `reusing`): a group matched there has its
    /// remembered values forgotten, its scope run, its node reused.
    reusing: u32,
    /// Deactivated (its values forgotten, its groups and nodes kept for reuse): not recomposed until its
    /// content is set again with reuse.
    deactivated: bool,
    /// A paused composition of it is under way (set, not applied yet): what it composes is held until it is
    /// applied, and the recomposer leaves it alone.
    paused: bool,
    /// Its paused composition was cancelled: it is to be disposed of, not composed.
    cancelled: bool,
    /// While a paused composition resumes: whether to pause at the next restartable call.
    pausing: Option<Rc<dyn Fn() -> bool>>,
    /// The scopes paused in this resume (composed by the next).
    paused_scopes: Vec<ScopeId>,
}

/// One composition's state, shared by what refers to it (derived states, the scope handles).
pub(crate) struct Composer {
    pub(crate) state: RefCell<ComposerState>,
    /// What applied snapshots changed, from any thread: invalidated before the next recomposition.
    pending: Arc<Mutex<Vec<StateKey>>>,
    reader: Rc<dyn Fn(StateKey)>,
    /// Unique while the program runs (what a group of it is told apart from another composition's by).
    pub(crate) id: u64,
    /// The context it was made from (a child composition's: its parent's locals, key and recomposer).
    pub(crate) parent: Option<CompositionContext>,
    /// The contexts taken in it, which child compositions are made from (weakly: gone with their groups).
    pub(crate) contexts: RefCell<Vec<Weak<ContextData>>>,
    /// The recomposer it is registered with: its root composition's.
    pub(crate) recomposer: Rc<Recomposer>,
}

/// A group of a composition: where saveable positions are counted from (a registry's provider).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Boundary {
    pub(crate) composer: u64,
    pub(crate) group: GroupId,
}

thread_local! {
    static CURRENT: RefCell<Vec<Rc<Composer>>> = const { RefCell::new(Vec::new()) };
}

struct PopComposer;
impl Drop for PopComposer {
    fn drop(&mut self) {
        CURRENT.with(|c| c.borrow_mut().pop());
    }
}

fn with_current<R>(c: &Rc<Composer>, f: impl FnOnce() -> R) -> R {
    CURRENT.with(|s| s.borrow_mut().push(c.clone()));
    let _pop = PopComposer;
    f()
}

impl Composer {
    /// The composition this thread is composing.
    pub(crate) fn current() -> Option<Rc<Composer>> {
        CURRENT.with(|c| c.borrow().last().cloned())
    }

    /// The composition of id `id`, if this thread composes it now.
    fn composing(id: u64) -> Option<Rc<Composer>> {
        CURRENT.with(|c| c.borrow().iter().rev().find(|c| c.id == id).cloned())
    }

    #[track_caller]
    pub(crate) fn require() -> Rc<Composer> {
        Composer::current().expect("kompoz: a composable called outside of a composition")
    }

    fn new(parent: Option<CompositionContext>) -> Rc<Composer> {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let recomposer = parent.as_ref().map_or_else(|| Rc::new(Recomposer::default()), |p| p.0.recomposer.clone());
        Rc::new_cyclic(|weak: &Weak<Composer>| {
            let w = weak.clone();
            let reader: Rc<dyn Fn(StateKey)> = Rc::new(move |key| {
                if let Some(c) = w.upgrade() {
                    c.record_read(key);
                }
            });
            Composer {
                state: RefCell::new(ComposerState {
                    groups: Slab::default(),
                    scopes: Slab::default(),
                    frames: Vec::new(),
                    locals: Vec::new(),
                    after: Vec::new(),
                    side: Vec::new(),
                    forgets: Vec::new(),
                    garbage: Vec::new(),
                    seq: 0,
                    current_scope: None,
                    readers: HashMap::new(),
                    invalid: Vec::new(),
                    frame_scopes: Vec::new(),
                    timed_scopes: Vec::new(),
                    parked: Vec::new(),
                    set_holders: Vec::new(),
                    derived_keys: HashSet::new(),
                    imminent: Vec::new(),
                    pass: 0,
                    frame_time: 0,
                    stats: RecompositionStats::default(),
                    root: None,
                    composing: false,
                    reusing: 0,
                    deactivated: false,
                    paused: false,
                    cancelled: false,
                    pausing: None,
                    paused_scopes: Vec::new(),
                }),
                pending: Arc::new(Mutex::new(Vec::new())),
                reader,
                id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                parent,
                contexts: RefCell::new(Vec::new()),
                recomposer,
            }
        })
    }

    // ---- Reads and invalidation ----

    /// Records that the running scope read `key`.
    /// A local's holder read by the running scope (and by the derived state computing, if one is: a key it can't
    /// check, which reaches it through the composition).
    fn record_local_read(&self, key: StateKey) {
        crate::read::note_read(key);
        self.record_read(key);
    }

    pub(crate) fn record_read(&self, key: StateKey) {
        let mut st = self.state.borrow_mut();
        let st = &mut *st;
        let Some(s) = st.current_scope else { return };
        let scope = st.scopes.get_mut(s.0);
        if scope.reads.iter().any(|r| r.0 == key) {
            return;
        }
        let readers = st.readers.entry(key).or_default();
        scope.reads.push((key, readers.scopes.len() as u32));
        readers.scopes.push(Some(s));
    }

    fn clear_reads(st: &mut ComposerState, s: ScopeId) {
        let reads = std::mem::take(&mut st.scopes.get_mut(s.0).reads);
        for (key, slot) in &reads {
            let Some(readers) = st.readers.get_mut(key) else { continue };
            readers.scopes[*slot as usize] = None;
            readers.holes += 1;
            if readers.holes as usize == readers.scopes.len() {
                st.readers.remove(key);
            } else if readers.holes > 16 && 2 * readers.holes as usize > readers.scopes.len() {
                // Mostly holes: closed up.
                readers.scopes.retain(|s| s.is_some());
                readers.holes = 0;
                let scopes = readers.scopes.clone();
                for (i, reader) in scopes.iter().enumerate() {
                    for r in &mut st.scopes.get_mut(reader.unwrap().0).reads {
                        if r.0 == *key {
                            r.1 = i as u32;
                        }
                    }
                }
            }
        }
    }

    /// Queues `scope` to run again (an observer: recomputes instead).
    pub(crate) fn invalidate_scope(&self, s: ScopeId) {
        let on_invalidated = {
            let mut st = self.state.borrow_mut();
            let Some(scope) = st.scopes.try_get(s.0) else { return };
            if let Some(f) = &scope.on_invalidated {
                Some(f.clone())
            } else {
                let scope = st.scopes.get_mut(s.0);
                if !scope.invalid {
                    scope.invalid = true;
                    st.invalid.push(s);
                }
                None
            }
        };
        if let Some(f) = on_invalidated {
            f();
        }
    }

    /// Queues the scopes that read `key` to run again.
    pub(crate) fn invalidate_readers(&self, key: StateKey) {
        let scopes: Vec<ScopeId> = {
            let st = self.state.borrow();
            match st.readers.get(&key) {
                Some(r) => r.scopes.iter().flatten().copied().collect(),
                None => return,
            }
        };
        for s in scopes {
            self.invalidate_scope(s);
        }
    }

    fn take_pending(&self) {
        let keys = std::mem::take(&mut *self.pending.lock());
        let (set_holders, imminent) = {
            let mut st = self.state.borrow_mut();
            (std::mem::take(&mut st.set_holders), std::mem::take(&mut st.imminent))
        };
        for key in keys {
            let derived = self.state.borrow().derived_keys.contains(&key);
            if set_holders.contains(&key) || derived {
                continue;
            }
            if imminent.is_empty() {
                self.invalidate_readers(key);
                continue;
            }
            let scopes: Vec<ScopeId> = {
                let st = self.state.borrow();
                match st.readers.get(&key) {
                    Some(r) => r.scopes.iter().flatten().copied().collect(),
                    None => continue,
                }
            };
            for s in scopes {
                if !imminent.contains(&(key, s)) {
                    self.invalidate_scope(s);
                }
            }
        }
    }

    /// A state written while this composition composes (in its snapshot: Compose's `recordWriteOf`): its readers
    /// the composition hasn't reached yet are invalidated at once — composed later in this pass, seeing the value
    /// written (Compose's imminent invalidation: a scope further on in the composition) —; those it has (composed
    /// or skipped in this pass, running now, or before where it composes) are told when the pass's snapshot is
    /// applied, and compose again in the next pass. A derived state's readers are told as it is (when applied).
    fn record_write_of(&self, key: StateKey) {
        let mut st = self.state.borrow_mut();
        let st = &mut *st;
        if !st.composing {
            return;
        }
        let Some(readers) = st.readers.get(&key) else { return };
        let ahead: Vec<ScopeId> = readers
            .scopes
            .iter()
            .flatten()
            .copied()
            .filter(|s| {
                st.scopes.try_get(s.0).is_some_and(|scope| {
                    scope.on_invalidated.is_none() && scope.group.is_some_and(|g| Self::is_ahead(st, g))
                })
            })
            .collect();
        for s in ahead {
            let scope = st.scopes.get_mut(s.0);
            if !scope.invalid {
                scope.invalid = true;
                st.invalid.push(s);
            }
            st.imminent.push((key, s));
        }
    }

    /// Whether the group `g` is further on than where the composition composes now (Compose's `location >=
    /// reader.currentGroup`): not composed (nor skipped) in this pass, nor running, and after the current position
    /// in the composition's order — below a group composing now, among the children it hasn't reached yet, or
    /// outside the scope restarted now, after it.
    fn is_ahead(st: &ComposerState, g: GroupId) -> bool {
        let on_stack = |x: GroupId| st.frames.iter().any(|f| f.group == x);
        if on_stack(g) || st.groups.get(g.0).composed_in == st.pass {
            return false; // running, or composed (or skipped) in this pass
        }
        let mut path = vec![g];
        let mut x = st.groups.get(g.0).parent;
        while let Some(p) = x {
            if on_stack(p) {
                return true; // below a group composing now, not reached yet
            }
            if st.groups.get(p.0).composed_in == st.pass {
                return false; // inside what was composed or skipped in this pass
            }
            path.push(p);
            x = st.groups.get(p.0).parent;
        }
        // Outside the scope restarted now: after it in the composition's order?
        let Some(restarted) = st.frames.first().map(|f| f.group) else { return false };
        let mut other = vec![restarted];
        let mut x = st.groups.get(restarted.0).parent;
        while let Some(p) = x {
            other.push(p);
            x = st.groups.get(p.0).parent;
        }
        // (both paths end at the root group: the first difference from the top is where they part)
        let (mut a, mut b) = (path.iter().rev().peekable(), other.iter().rev().peekable());
        let mut parent = None;
        while let (Some(x), Some(y)) = (a.peek(), b.peek()) {
            if x != y {
                break;
            }
            parent = Some(**x);
            a.next();
            b.next();
        }
        match (parent, a.next(), b.next()) {
            (Some(parent), Some(x), Some(y)) => {
                let children = &st.groups.get(parent.0).children;
                let index = |c: &GroupId| children.iter().position(|k| k == c);
                matches!((index(x), index(y)), (Some(i), Some(j)) if i > j)
            }
            _ => false,
        }
    }

    /// A derived state made here (its key), or gone.
    pub(crate) fn set_derived_key(&self, key: StateKey, made: bool) {
        let mut st = self.state.borrow_mut();
        if made {
            st.derived_keys.insert(key);
        } else {
            st.derived_keys.remove(&key);
        }
    }

    // ---- Groups ----

    /// Opens the group of a call with key `key` under the group being composed (not entered).
    fn begin_group(st: &mut ComposerState, key: u64) -> GroupId {
        let frame = st.frames.last_mut().expect("kompoz: a group outside of a composition");
        let occurrence = match frame.occurrences.iter_mut().find(|o| o.0 == key) {
            Some(o) => {
                o.1 += 1;
                o.1 - 1
            }
            None => {
                frame.occurrences.push((key, 1));
                0
            }
        };
        let id = mix_key(key, occurrence as u64);
        let mut found = None;
        if let Some(Some(next)) = frame.old.get(frame.cursor)
            && st.groups.get(next.0).key == id
        {
            found = Some(*next);
            frame.old[frame.cursor] = None;
            frame.cursor += 1;
        }
        if found.is_none() && frame.old.len() > frame.cursor {
            let groups = &st.groups;
            let old = &frame.old;
            let lookup = frame.lookup.get_or_insert_with(|| {
                old.iter().enumerate().filter_map(|(i, g)| g.map(|g| (groups.get(g.0).key, i))).collect()
            });
            if let Some(i) = lookup.remove(&id) {
                found = frame.old[i].take();
            }
        }
        let parent = frame.group;
        let g = match found {
            Some(g) => {
                if st.reusing > 0 {
                    Self::strip(st, g, false);
                }
                g
            }
            None => {
                let depth = st.groups.get(parent.0).depth + 1;
                GroupId(st.groups.insert(Group::new(Some(parent), depth, id)))
            }
        };
        st.frames.last_mut().unwrap().children.push(g);
        st.seq += 1;
        let (seq, pass) = (st.seq, st.pass);
        let group = st.groups.get_mut(g.0);
        group.seq = seq;
        group.composed_in = pass;
        g
    }

    /// Composes `g`'s children from now on.
    fn enter(st: &mut ComposerState, g: GroupId) {
        let old: Vec<Option<GroupId>> =
            std::mem::take(&mut st.groups.get_mut(g.0).children).into_iter().map(Some).collect();
        st.frames.push(Frame { group: g, old, cursor: 0, lookup: None, occurrences: Vec::new(), children: Vec::new() });
    }

    pub(crate) fn start_group(&self, key: u64) {
        let mut st = self.state.borrow_mut();
        let g = Self::begin_group(&mut st, key);
        Self::enter(&mut st, g);
    }

    /// The current group's position below `boundary`: the keys of the groups in between, from this one up to
    /// it. A child composition's continues in its parent from where its context was taken (Compose's
    /// `compositeKeyHash` of a subcomposition), up to `boundary` in whichever composition it is (none: the top).
    pub(crate) fn key_below(&self, boundary: Option<Boundary>) -> u64 {
        let start = self.state.borrow().frames.last().map(|f| f.group);
        self.key_from(start, boundary)
    }

    /// [`key_below`](Self::key_below) from `start`.
    pub(crate) fn key_from(&self, start: Option<GroupId>, boundary: Option<Boundary>) -> u64 {
        let (mut key, mut reached) = self.fold_up(1469598103934665603, start, boundary);
        let mut context = self.parent.clone();
        while !reached && let Some(ctx) = context {
            let Some(parent) = ctx.0.composer.upgrade() else { break };
            (key, reached) = parent.fold_up(key, ctx.0.group.get(), boundary);
            context = parent.parent.clone();
        }
        key
    }

    /// Mixes the keys of `start` and the groups above it into `key`, up to `boundary` if it is one of this
    /// composition's (and whether it was reached).
    fn fold_up(&self, mut key: u64, start: Option<GroupId>, boundary: Option<Boundary>) -> (u64, bool) {
        let st = self.state.borrow();
        let stop = boundary.filter(|b| b.composer == self.id).map(|b| b.group);
        let mut g = start;
        while let Some(id) = g {
            if Some(id) == stop {
                return (key, true);
            }
            let Some(group) = st.groups.try_get(id.0) else { break };
            key = mix_key(key, group.key);
            g = group.parent;
        }
        (key, false)
    }

    /// The group composing now, as a saveable registry's boundary.
    pub(crate) fn boundary_here(&self) -> Boundary {
        Boundary { composer: self.id, group: self.current_group() }
    }

    /// Whether what runs now is composed in a group (and so can remember): not a derived state computed outside
    /// a composition, not an effect run after one.
    pub(crate) fn in_group(&self) -> bool {
        let st = self.state.borrow();
        st.composing && !st.frames.is_empty()
    }

    pub(crate) fn current_group(&self) -> GroupId {
        self.state.borrow().frames.last().expect("kompoz: no group").group
    }

    /// Closes the group being composed: whatever it didn't call this time has left the composition (skipped
    /// scopes count as called: they keep their slots and nodes), and a node it emitted is told its children.
    pub(crate) fn end_group(&self) {
        let update = {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            let frame = st.frames.pop().expect("kompoz: end_group without a group");
            let mut gone: Vec<GroupId> = frame.old.into_iter().flatten().collect();
            st.groups.get_mut(frame.group.0).children = frame.children;
            for g in std::mem::take(&mut gone) {
                if let Some(g) = Self::park(st, g) {
                    gone.push(g);
                }
            }
            Self::destroy_in_call_order(st, gone);
            Self::node_update(st, frame.group)
        };
        if let Some((node, children)) = update {
            node.set_children(&children);
        }
    }

    /// If `g` emits a node whose children changed: the node and its children now.
    fn node_update(st: &mut ComposerState, g: GroupId) -> Option<(Rc<dyn Node>, Vec<Rc<dyn Node>>)> {
        let node = st.groups.get(g.0).node.clone()?;
        let mut children = Vec::new();
        Self::collect_nodes(st, g, &mut children);
        let ids: Vec<*const ()> = children.iter().map(|n| Rc::as_ptr(n) as *const ()).collect();
        let group = st.groups.get_mut(g.0);
        if group.node_children == ids {
            return None;
        }
        group.node_children = ids;
        Some((node, children))
    }

    /// The nodes directly below `g`, in call order, looking through groups that don't emit one.
    fn collect_nodes(st: &ComposerState, g: GroupId, out: &mut Vec<Rc<dyn Node>>) {
        for &child in &st.groups.get(g.0).children {
            match &st.groups.get(child.0).node {
                Some(n) => out.push(n.clone()),
                None => Self::collect_nodes(st, child, out),
            }
        }
    }

    /// The node `g`'s nodes go into: the nearest above that emits one, if its children changed.
    fn parent_node_update(st: &mut ComposerState, g: GroupId) -> Option<(Rc<dyn Node>, Vec<Rc<dyn Node>>)> {
        let mut p = st.groups.get(g.0).parent;
        while let Some(pg) = p {
            if st.groups.get(pg.0).node.is_some() {
                return Self::node_update(st, pg);
            }
            p = st.groups.get(pg.0).parent;
        }
        None
    }

    /// Whether `g` is in a movable content's group that left its place in this pass (parked: composed again where it
    /// is called next, else destroyed): its scopes aren't run on their own meanwhile (Compose's removed groups'
    /// scopes aren't either), as they would compose without the composition above them — its locals.
    fn in_parked(st: &ComposerState, g: GroupId) -> bool {
        let mut top = g;
        while let Some(p) = st.groups.get(top.0).parent {
            top = p;
        }
        st.parked.contains(&top)
    }

    /// A group leaving the composition: a movable content in it (or it) waits in `parked` until the composition
    /// is over; returns what is to be destroyed.
    fn park(st: &mut ComposerState, g: GroupId) -> Option<GroupId> {
        let group = st.groups.get(g.0);
        if let Some(m) = &group.movable
            && m.group.get() == Some(g)
        {
            st.groups.get_mut(g.0).parent = None;
            st.parked.push(g);
            return None;
        }
        let children = std::mem::take(&mut st.groups.get_mut(g.0).children);
        let mut kept = Vec::new();
        for c in children {
            if let Some(c) = Self::park(st, c) {
                kept.push(c);
            }
        }
        st.groups.get_mut(g.0).children = kept;
        Some(g)
    }

    /// Groups in the order they were called (seq), for what they hold to be forgotten in it.
    fn destroy_in_call_order(st: &mut ComposerState, mut groups: Vec<GroupId>) {
        groups.sort_by_key(|g| st.groups.get(g.0).seq);
        for g in groups {
            Self::destroy(st, g);
        }
    }

    /// `g` and what is below it leave: what they hold is forgotten (told once the composition is over, in the
    /// reverse of the order it was composed in).
    fn destroy(st: &mut ComposerState, g: GroupId) {
        let mut group = st.groups.remove(g.0);
        if let Some(f) = group.on_forget.take() {
            st.forgets.push(f);
        }
        if let Some(slot) = group.slot.take() {
            Self::forget_slot_value(st, slot);
        }
        let children = std::mem::take(&mut group.children);
        Self::destroy_in_call_order(st, children);
        if let Some(s) = group.scope.take() {
            Self::remove_scope(st, s);
        }
        if let Some(m) = group.movable.take()
            && m.group.get() == Some(g)
        {
            m.group.set(None);
        }
        // Its node released (the composition's own root isn't its to release), told before what its group and
        // the groups below it remembered are forgotten (the forgets run last pushed first), as Compose's node
        // callbacks go in the queue of forgets.
        if let Some(node) = group.node.take() {
            if Some(g) != st.root {
                st.forgets.push(Box::new(move || node.on_release()));
            } else {
                st.garbage.push(Box::new(node));
            }
        }
        st.garbage.push(Box::new(group.provided));
    }

    fn forget_slot_value(st: &mut ComposerState, slot: SlotValue) {
        // The value dropped (and its observer told) after the composition: forgotten if it entered, else abandoned.
        st.forgets.push(Box::new(move || {
            if let Some(o) = &slot.observer {
                if o.entered.get() {
                    o.observer.on_forgotten();
                } else {
                    o.observer.on_abandoned();
                }
            }
            drop(slot);
        }));
    }

    /// `g`'s remembered values forgotten and its effects' cleanups run (after the composition), its scope's reads
    /// dropped and its arguments with them (so it runs, not skips, the next time it is called), as content reused
    /// or deactivated has (Compose's reuse and `deactivateToEndGroup`). Its children too with `deep`, and the
    /// nodes then deactivated; else its node is reused when its emit runs again.
    fn strip(st: &mut ComposerState, g: GroupId, deep: bool) {
        let group = st.groups.get_mut(g.0);
        let slot = group.slot.take();
        let on_forget = group.on_forget.take();
        let scope = group.scope;
        let node = group.node.clone();
        group.reused = true;
        if let Some(f) = on_forget {
            st.forgets.push(f);
        }
        if let Some(slot) = slot {
            Self::forget_slot_value(st, slot);
        }
        if let Some(s) = scope {
            Self::clear_reads(st, s);
            st.invalid.retain(|x| *x != s);
            st.paused_scopes.retain(|x| *x != s);
            st.frame_scopes.retain(|x| *x != s);
            st.timed_scopes.retain(|x| x.1 != s);
            let scope = st.scopes.get_mut(s.0);
            scope.invalid = false;
            let old = scope.args.take();
            st.garbage.push(Box::new(old));
        }
        if deep {
            for c in st.groups.get(g.0).children.clone() {
                Self::strip(st, c, true);
            }
            if let Some(node) = node {
                st.forgets.push(Box::new(move || node.on_deactivate()));
            }
        }
    }

    /// The current group's old children (those not composed yet) deactivated: their values forgotten, their
    /// nodes deactivated, all kept for reuse (Compose's `deactivateToEndGroup`).
    pub(crate) fn deactivate_rest(&self) {
        let mut st = self.state.borrow_mut();
        let st = &mut *st;
        let old: Vec<GroupId> = st.frames.last().expect("kompoz: no group").old.iter().flatten().copied().collect();
        for g in old {
            Self::strip(st, g, true);
        }
    }

    /// The current group keeps the children it has without composing them (Compose's `skipToGroupEnd`).
    pub(crate) fn skip_rest(&self) {
        let mut st = self.state.borrow_mut();
        let frame = st.frames.last_mut().expect("kompoz: no group");
        let old = std::mem::take(&mut frame.old);
        frame.children.extend(old.into_iter().flatten());
        frame.lookup = None;
        frame.cursor = 0;
    }

    /// What is composed from now on is reused content (Compose's reusing), until [`end_reuse`](Self::end_reuse).
    pub(crate) fn begin_reuse(&self) {
        self.state.borrow_mut().reusing += 1;
    }

    pub(crate) fn end_reuse(&self) {
        self.state.borrow_mut().reusing -= 1;
    }

    fn remove_scope(st: &mut ComposerState, s: ScopeId) {
        Self::clear_reads(st, s);
        st.invalid.retain(|x| *x != s);
        st.frame_scopes.retain(|x| *x != s);
        st.timed_scopes.retain(|x| x.1 != s);
        if st.current_scope == Some(s) {
            st.current_scope = None;
        }
        let scope = st.scopes.remove(s.0);
        st.garbage.push(Box::new((scope.body, scope.args, scope.on_invalidated)));
    }

    // ---- Remember ----

    /// Takes what the current group remembered out (put back with `put_slot`): the app's code (a key's `==`, a
    /// value's `clone`) runs on it with nothing of the composer's borrowed.
    pub(crate) fn take_slot(&self) -> Option<SlotValue> {
        let mut st = self.state.borrow_mut();
        let g = st.frames.last().expect("kompoz: no group").group;
        st.groups.get_mut(g.0).slot.take()
    }

    pub(crate) fn put_slot(&self, slot: SlotValue) {
        let mut st = self.state.borrow_mut();
        let g = st.frames.last().expect("kompoz: no group").group;
        st.groups.get_mut(g.0).slot = Some(slot);
    }

    /// Remembers `value` in the current group, forgetting what it held; an observer is told it entered once the
    /// composition is over.
    pub(crate) fn set_remembered(&self, value: Box<dyn Any>, observer: Option<Rc<dyn RememberObserver>>) {
        let mut st = self.state.borrow_mut();
        let st = &mut *st;
        let g = st.frames.last().expect("kompoz: no group").group;
        if let Some(old) = st.groups.get_mut(g.0).slot.take() {
            Self::forget_slot_value(st, old);
        }
        let observer = observer.map(|observer| {
            let entered = Rc::new(std::cell::Cell::new(false));
            let (o, e) = (observer.clone(), entered.clone());
            st.after.push(Box::new(move || {
                e.set(true);
                o.on_remembered();
            }));
            Observed { observer, entered }
        });
        st.groups.get_mut(g.0).slot = Some(SlotValue { value, observer });
    }

    /// The cleanup of the current group's DisposableEffect: the old one handed over to be told after the
    /// composition, before any new effect runs.
    pub(crate) fn replace_on_forget(&self) {
        let mut st = self.state.borrow_mut();
        let st = &mut *st;
        let g = st.frames.last().expect("kompoz: no group").group;
        if let Some(f) = st.groups.get_mut(g.0).on_forget.take() {
            st.forgets.push(f);
        }
    }

    /// Sets `g`'s cleanup (a DisposableEffect's), after the composition: `g` may be gone by then.
    pub(crate) fn set_on_forget(&self, g: GroupId, f: Box<dyn FnOnce()>) {
        let gone = {
            let mut st = self.state.borrow_mut();
            match st.groups.items.get_mut(g.0 as usize).and_then(|x| x.as_mut()) {
                Some(group) => {
                    group.on_forget = Some(f);
                    None
                }
                None => Some(f),
            }
        };
        if let Some(f) = gone {
            f(); // (gone meanwhile: told at once)
        }
    }

    pub(crate) fn after_composition(&self, effect: Box<dyn FnOnce()>) {
        self.state.borrow_mut().after.push(effect);
    }

    pub(crate) fn side_effect(&self, effect: Box<dyn FnOnce()>) {
        self.state.borrow_mut().side.push(effect);
    }

    // ---- Nodes ----

    /// Emits a node in a group of its own: made by `factory` the first time, configured by `update`, its children
    /// emitted by `content`.
    pub(crate) fn emit(
        &self,
        key: u64,
        factory: impl FnOnce() -> Rc<dyn Node>,
        update: impl FnOnce(&Rc<dyn Node>),
        content: impl FnOnce(),
    ) {
        self.start_group(key);
        let (node, reused) = {
            let mut st = self.state.borrow_mut();
            let g = st.frames.last().unwrap().group;
            let group = st.groups.get_mut(g.0);
            (group.node.clone(), std::mem::take(&mut group.reused))
        };
        let node = match node {
            Some(n) => {
                if reused {
                    n.on_reuse(); // (reset before it is updated as a new one)
                }
                n
            }
            None => {
                let n = factory();
                let mut st = self.state.borrow_mut();
                let g = st.frames.last().unwrap().group;
                st.groups.get_mut(g.0).node = Some(n.clone());
                n
            }
        };
        update(&node);
        content();
        self.end_group();
    }

    // ---- Scopes ----

    /// A restartable call: its group, and its body run unless `args` equal the last ones (`equals`) and nothing
    /// it read changed.
    pub(crate) fn restartable(
        &self,
        key: u64,
        args: Rc<dyn Any>,
        equals: Option<fn(&dyn Any, &dyn Any) -> bool>,
        body: Rc<dyn Fn(&dyn Any)>,
    ) {
        let s = {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            let g = Self::begin_group(st, key);
            match st.groups.get(g.0).scope {
                Some(s) => {
                    let scope = st.scopes.get(s.0);
                    if !scope.invalid
                        && let (Some(eq), Some(last)) = (equals, &scope.args)
                        && eq(&**last, &*args)
                    {
                        st.stats.scopes_skipped += 1; // the same arguments, nothing it read changed: as it is
                        return;
                    }
                    let scope = st.scopes.get_mut(s.0);
                    let old = (scope.args.replace(args), scope.body.replace(body));
                    st.garbage.push(Box::new(old));
                    s
                }
                None => {
                    let s = ScopeId(st.scopes.insert(Scope {
                        group: Some(g),
                        body: Some(body),
                        args: Some(args),
                        on_invalidated: None,
                        reads: Vec::new(),
                        invalid: false,
                        reuse_on_restart: false,
                    }));
                    st.groups.get_mut(g.0).scope = Some(s);
                    s
                }
            }
        };
        // A paused composition pauses here, at a restartable call (Compose's restart group boundary), when asked:
        // the scope keeps what it had and is composed by the next resume.
        let pausing = self.state.borrow().pausing.clone();
        if let Some(should_pause) = pausing
            && should_pause()
        {
            let mut st = self.state.borrow_mut();
            let reusing = st.reusing > 0;
            let scope = st.scopes.get_mut(s.0);
            scope.invalid = true;
            scope.reuse_on_restart |= reusing;
            st.paused_scopes.push(s);
            return;
        }
        self.run(s);
    }

    /// Runs `s`'s body in its group, its reads recorded.
    fn run(&self, s: ScopeId) {
        let (outer, body, args, g) = {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            let outer = st.current_scope.replace(s);
            Self::clear_reads(st, s);
            st.timed_scopes.retain(|t| t.1 != s); // (asked again if still due)
            let scope = st.scopes.get_mut(s.0);
            scope.invalid = false;
            let (body, args, g) = (scope.body.clone(), scope.args.clone(), scope.group.expect("an observer run"));
            st.groups.get_mut(g.0).composed_in = st.pass; // (a restarted scope's: composed in this pass)
            Self::enter(st, g);
            (outer, body, args, g)
        };
        let _ = g;
        observe_reads(self.reader.clone(), || {
            if let (Some(body), Some(args)) = (body, args) {
                body(&*args);
            }
        });
        self.end_group();
        let mut st = self.state.borrow_mut();
        st.current_scope = outer;
        st.stats.scopes_run += 1;
    }

    /// Runs a scope on its own, without its parent: the composition locals come from the providers above it, and
    /// the node it emits into is told its children.
    fn restart(&self, s: ScopeId) {
        let context_locals = self.parent.as_ref().map(|p| p.0.locals.borrow().clone()).unwrap_or_default();
        let reuse = {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            let g = st.scopes.get(s.0).group.unwrap();
            st.frames.clear();
            st.locals = context_locals;
            let above = Self::locals_above(st, g);
            st.locals.extend(above);
            st.current_scope = None;
            let reuse = std::mem::take(&mut st.scopes.get_mut(s.0).reuse_on_restart);
            if reuse {
                st.reusing += 1;
            }
            reuse
        };
        self.run(s);
        let update = {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            if reuse {
                st.reusing -= 1;
            }
            let locals = std::mem::take(&mut st.locals); // (what they hold isn't the composer's to keep)
            st.garbage.push(Box::new(locals));
            let g = st.scopes.try_get(s.0).and_then(|s| s.group);
            g.and_then(|g| Self::parent_node_update(st, g))
        };
        if let Some((node, children)) = update {
            node.set_children(&children);
        }
    }

    /// A group-less scope recording reads for `on_invalidated` (a derived state's).
    pub(crate) fn new_observer(&self, on_invalidated: Rc<dyn Fn()>) -> ScopeId {
        let mut st = self.state.borrow_mut();
        ScopeId(st.scopes.insert(Scope {
            group: None,
            body: None,
            args: None,
            on_invalidated: Some(on_invalidated),
            reads: Vec::new(),
            invalid: false,
            reuse_on_restart: false,
        }))
    }

    pub(crate) fn remove_observer(&self, s: ScopeId) {
        let garbage = {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            Self::remove_scope(st, s);
            if st.composing { Vec::new() } else { std::mem::take(&mut st.garbage) }
        };
        drop(garbage);
    }

    /// Runs `f` with its reads recorded into the observer `s`.
    pub(crate) fn observe_into<R>(self: &Rc<Self>, s: ScopeId, f: impl FnOnce() -> R) -> R {
        let outer = {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            Self::clear_reads(st, s);
            st.current_scope.replace(s)
        };
        let r = with_current(self, || observe_reads(self.reader.clone(), f));
        self.state.borrow_mut().current_scope = outer;
        r
    }

    /// The running scope composed again at the next frame (an animation's).
    pub(crate) fn request_frame(&self) {
        let mut st = self.state.borrow_mut();
        if let Some(s) = st.current_scope
            && !st.frame_scopes.contains(&s)
        {
            st.frame_scopes.push(s);
        }
    }

    /// The running scope composed again at the frame at `frame_time_nanos` (or the first after), not on those
    /// before: an animation read elsewhere telling its scope when it is done.
    pub(crate) fn request_frame_at(&self, frame_time_nanos: i64) {
        let mut st = self.state.borrow_mut();
        let Some(s) = st.current_scope else { return };
        if let Some(t) = st.timed_scopes.iter_mut().find(|t| t.1 == s) {
            t.0 = t.0.min(frame_time_nanos);
        } else {
            st.timed_scopes.push((frame_time_nanos, s));
        }
    }

    pub(crate) fn frame_time(&self) -> i64 {
        self.state.borrow().frame_time
    }

    // ---- Composition locals ----

    fn locals_above(st: &ComposerState, g: GroupId) -> Vec<Provided> {
        let mut providers = Vec::new();
        let mut p = st.groups.get(g.0).parent;
        while let Some(pg) = p {
            let group = st.groups.get(pg.0);
            if !group.provided.is_empty() {
                providers.push(pg);
            }
            p = group.parent;
        }
        providers.iter().rev().flat_map(|pg| st.groups.get(pg.0).provided.iter().cloned()).collect()
    }

    /// Composes `content` with `values` provided, in the current group (the provider's), as Compose's
    /// `updatedStateOf`: a dynamic local given another value has it set in the provider's holder (its readers
    /// here are composed again now, and whoever else read it, a layout's measure or drawing, is told as the
    /// composition's snapshot is applied); a static local given another value, or a context's holder given in
    /// place of another, comes in a holder of its own, and those that read the old one are composed again.
    pub(crate) fn provide(&self, mut values: Vec<Provided>, content: impl FnOnce()) {
        values.retain(|v| !(v.if_missing && self.find_local(v.local).is_some())); // provided above already
        let (changed, set, mark) = {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            let g = st.frames.last().unwrap().group;
            let mut changed = Vec::new();
            let mut set = Vec::new();
            for v in &mut values {
                let Some(old) = st.groups.get(g.0).provided.iter().find(|p| p.local == v.local) else { continue };
                if v.shared || old.shared {
                    if !Rc::ptr_eq(&old.holder, &v.holder) {
                        changed.push(old.holder.key);
                    }
                    continue;
                }
                let (old_value, value) = (old.holder.peek(), v.holder.peek());
                let same = (v.equals)(&*old_value, &*value);
                if v.dynamic && old.dynamic {
                    if !same {
                        set.push((old.holder.clone(), value));
                    }
                    v.holder = old.holder.clone();
                } else if same && v.dynamic == old.dynamic {
                    v.holder = old.holder.clone();
                } else {
                    changed.push(old.holder.key);
                }
            }
            st.set_holders.extend(set.iter().map(|(h, _)| h.key));
            let old = std::mem::replace(&mut st.groups.get_mut(g.0).provided, values.clone());
            st.garbage.push(Box::new(old));
            let mark = st.locals.len();
            st.locals.extend(values);
            (changed, set, mark)
        };
        for (holder, value) in set {
            holder.set(value);
            crate::snapshot::state_changed(holder.key);
            self.invalidate_readers(holder.key);
        }
        for key in changed {
            self.invalidate_readers(key);
        }
        content();
        let dropped = {
            let mut st = self.state.borrow_mut();
            st.locals.split_off(mark)
        };
        drop(dropped);
    }

    /// The value provided for `local` nearest above, if any.
    pub(crate) fn find_local(&self, local: usize) -> Option<Rc<dyn Any>> {
        let st = self.state.borrow();
        st.locals.iter().rev().find(|p| p.local == local).map(|p| p.holder.peek())
    }

    /// The same, the running scope subscribed to the holder giving it (recomposed when that holds another value,
    /// or its provider gives another holder).
    pub(crate) fn read_local(&self, local: usize) -> Option<Rc<dyn Any>> {
        let holder = {
            let st = self.state.borrow();
            st.locals.iter().rev().find(|p| p.local == local).map(|p| p.holder.clone())
        };
        let Some(holder) = holder else {
            // (a child composition's reader of a default: told when its parent provides one)
            self.record_context_locals_read();
            return None;
        };
        self.record_local_read(holder.key);
        Some(holder.peek())
    }

    /// A child composition's running scope subscribed to which locals its context gives (told when its parent
    /// provides others, or no longer one).
    fn record_context_locals_read(&self) {
        if let Some(p) = &self.parent {
            self.record_local_read(p.0.locals_key);
        }
    }

    /// Everything provided where this runs, outermost first, read by no one.
    pub(crate) fn locals_unobserved(&self) -> Vec<Provided> {
        self.state.borrow().locals.clone()
    }

    /// Everything provided where this runs, outermost first, as a layout keeps it (Compose's
    /// `currentCompositionLocalMap`): the running scope subscribed to the holders that are replaced rather than
    /// set (static locals', contexts'), so it composes again, and keeps the new holder, when one is; the dynamic
    /// ones are read later, live.
    pub(crate) fn locals_kept(&self) -> Vec<Provided> {
        let locals = self.state.borrow().locals.clone();
        for p in locals.iter().filter(|p| p.replaced_when_changed()) {
            self.record_local_read(p.holder.key);
        }
        self.record_context_locals_read();
        locals
    }

    /// Everything provided where this runs, outermost first (each read, so the reader follows them).
    pub(crate) fn locals(&self) -> Vec<Provided> {
        let locals = self.state.borrow().locals.clone();
        for p in &locals {
            self.record_local_read(p.holder.key);
        }
        self.record_context_locals_read();
        locals
    }

    // ---- Movable content ----

    /// Composes a movable content here: its group (state, nodes, effects) comes along from where it was composed
    /// last, unless that place called it already in this composition.
    pub(crate) fn compose_movable(&self, data: &Rc<MovableData>) {
        let key = mix_key(0x6d6f7661626c6521, Rc::as_ptr(data) as usize as u64);
        let old_update = {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            // Is there a group of this content at this place already?
            let frame = st.frames.last().unwrap();
            let occurrence = frame.occurrences.iter().find(|o| o.0 == key).map_or(0, |o| o.1);
            let id = mix_key(key, occurrence as u64);
            let here = frame.old.iter().flatten().any(|g| st.groups.get(g.0).key == id);
            let mut update = None;
            if !here && let Some(moved) = Self::take_movable(st, data, &mut update) {
                // Brought here: put where an old child of this key would be found.
                let frame = st.frames.last_mut().unwrap();
                st.groups.get_mut(moved.0).key = id;
                frame.old.push(Some(moved));
                frame.lookup = None;
                let parent = frame.group;
                st.groups.get_mut(moved.0).parent = Some(parent);
                let depth = st.groups.get(parent.0).depth + 1;
                Self::set_depth(st, moved, depth);
            }
            update
        };
        if let Some((node, children)) = old_update {
            node.set_children(&children);
        }
        {
            let mut st = self.state.borrow_mut();
            let g = Self::begin_group(&mut st, key);
            let group = st.groups.get_mut(g.0);
            if group.movable.is_none() && data.group.get().is_none() {
                group.movable = Some(data.clone());
                data.group.set(Some(g));
            }
            Self::enter(&mut st, g);
        }
        let content = data.content.borrow().clone(); // (it may be replaced while it runs)
        if let Some(content) = content {
            content();
        }
        self.end_group();
    }

    fn set_depth(st: &mut ComposerState, g: GroupId, depth: u32) {
        st.groups.get_mut(g.0).depth = depth;
        for c in st.groups.get(g.0).children.clone() {
            Self::set_depth(st, c, depth + 1);
        }
    }

    /// The movable content's group from where it was: parked, or still in its old place if that wasn't
    /// composed yet (taken out, its old place's node told).
    fn take_movable(
        st: &mut ComposerState,
        data: &MovableData,
        update: &mut Option<(Rc<dyn Node>, Vec<Rc<dyn Node>>)>,
    ) -> Option<GroupId> {
        let g = data.group.get()?;
        if st.groups.get(g.0).composed_in == st.pass {
            return None; // composed already in this composition: a new one here
        }
        let Some(old) = st.groups.get(g.0).parent else {
            let i = st.parked.iter().position(|p| *p == g)?;
            st.parked.remove(i);
            return Some(g);
        };
        // Still in its old place. If that is being composed now, it is an old child there: taken from the frame.
        for frame in st.frames.iter_mut() {
            if frame.group == old
                && let Some(slot) = frame.old.iter_mut().find(|x| **x == Some(g))
            {
                *slot = None;
                frame.lookup = None;
                return Some(g);
            }
        }
        st.groups.get_mut(old.0).children.retain(|c| *c != g);
        st.groups.get_mut(g.0).parent = None;
        *update = Self::parent_node_update_from(st, old);
        Some(g)
    }

    fn parent_node_update_from(st: &mut ComposerState, g: GroupId) -> Option<(Rc<dyn Node>, Vec<Rc<dyn Node>>)> {
        if st.groups.get(g.0).node.is_some() {
            return Self::node_update(st, g);
        }
        Self::parent_node_update(st, g)
    }

    // ---- Composing ----

    /// Runs every invalid scope, outermost first, in a snapshot of its own; then, outside of it, what left is
    /// forgotten (last composed first), what entered is remembered and its effects run, then the side effects.
    /// False if there was nothing to do.
    fn recompose(self: &Rc<Self>, frame_time_nanos: i64) -> bool {
        if !self.compose_pass(frame_time_nanos, None) {
            return false;
        }
        self.finish_composition();
        self.apply_changes();
        true
    }

    /// The invalid scopes run, outermost first, in a snapshot of its own applied after; false if none was. With
    /// `should_pause` (a paused composition resuming), a restartable call pauses when it says so (as does the
    /// next scope to run, after the first): those scopes stay invalid for the next resume, and what was
    /// composed is held (not forgotten, remembered or applied) until the paused composition is applied.
    fn compose_pass(self: &Rc<Self>, frame_time_nanos: i64, should_pause: Option<Rc<dyn Fn() -> bool>>) -> bool {
        self.take_pending();
        {
            let mut st = self.state.borrow_mut();
            if st.invalid.is_empty() {
                return false;
            }
            st.frame_time = frame_time_nanos;
            st.stats.recompositions += 1;
            st.pass += 1;
            st.composing = true;
            st.pausing = should_pause.clone();
        }
        let id = self.id;
        let observer: crate::snapshot::WriteObserver = Arc::new(move |key| {
            if let Some(c) = Composer::composing(id) {
                c.record_write_of(key);
            }
        });
        let snapshot = Snapshot::take_mutable_snapshot(None, Some(observer));
        snapshot.enter(|| {
            with_current(self, || {
                let mut first = true;
                loop {
                    if !first && should_pause.as_ref().is_some_and(|pause| pause()) {
                        break;
                    }
                    // Outermost first: running a scope also runs (or skips) the scopes inside it, which clears their
                    // invalid flag, so they are not run twice.
                    let next = {
                        let mut st = self.state.borrow_mut();
                        let st = &mut *st;
                        let best = st
                            .invalid
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, s)| st.groups.get(st.scopes.get(s.0).group.unwrap().0).depth)
                            .map(|(i, _)| i);
                        best.map(|i| st.invalid.remove(i))
                    };
                    let Some(s) = next else { break };
                    let still = {
                        let st = self.state.borrow();
                        st.scopes
                            .try_get(s.0)
                            .is_some_and(|x| x.invalid && !x.group.is_some_and(|g| Self::in_parked(&st, g)))
                    };
                    if still {
                        first = false;
                        self.restart(s);
                    }
                }
            })
        });
        let applied = snapshot.apply() == SnapshotApplyResult::Success;
        snapshot.dispose();
        {
            let mut st = self.state.borrow_mut();
            st.composing = false;
            st.pausing = None;
            let paused = std::mem::take(&mut st.paused_scopes);
            st.invalid.extend(paused);
        }
        assert!(applied, "kompoz: a state the composition wrote was written meanwhile on another thread");
        true
    }
    /// Movable contents not called again leave.
    fn finish_composition(&self) {
        let mut st = self.state.borrow_mut();
        let st = &mut *st;
        let parked = std::mem::take(&mut st.parked);
        Self::destroy_in_call_order(st, parked);
    }

    /// After a composition: forgets (reversed), saveable values whose composables (not providers) left gone,
    /// remembered and effects, side effects, then the garbage.
    fn apply_changes(&self) {
        let (forgets, after, side, garbage) = {
            let mut st = self.state.borrow_mut();
            (
                std::mem::take(&mut st.forgets),
                std::mem::take(&mut st.after),
                std::mem::take(&mut st.side),
                std::mem::take(&mut st.garbage),
            )
        };
        for f in forgets.into_iter().rev() {
            f();
        }
        crate::saveable::settle_saveables();
        for f in after {
            f();
        }
        for f in side {
            f();
        }
        drop(garbage);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Composition

/// What a composition composes: a closure, or a `#[kompozable]` function taking no arguments (or only defaulted
/// ones).
pub trait Content {
    fn compose(&self);
}

impl<F: Fn()> Content for F {
    fn compose(&self) {
        self()
    }
}

/// A composition: content composed into a tree of groups, emitting into `root`'s node, recomposed where what it
/// read changed (Compose's `Composition`, which is also its `ReusableComposition` and `PausableComposition`).
///
/// A root composition ([`new`](Composition::new)) is driven by its host: [`recompose`](Composition::recompose)
/// at every frame (after [`Snapshot::send_apply_notifications`]) runs what has to run again, in it and then in
/// the child compositions made from its contexts (each after the one it was made from, as Compose's
/// Recomposer has parents recompose before their children). A child composition
/// ([`with_parent`](Composition::with_parent): a [`SubcomposeLayout`]'s slot, content composed elsewhere) sees
/// its parent's composition locals where the context was taken, continues its composite key there, and composes
/// at once when its content is set.
///
/// It can be deactivated ([`deactivate`](Composition::deactivate): its remembered values forgotten, its groups
/// and nodes kept) and composed again reusing them ([`set_content_with_reuse`](Composition::set_content_with_reuse):
/// state starts over, nodes are reused), and its content can be composed a piece at a time
/// ([`set_pausable_content`](Composition::set_pausable_content)) before it is applied. Dropped, it is disposed of.
///
/// [`SubcomposeLayout`]: ../kompoz_ui/layout/fn.SubcomposeLayout.html
pub struct Composition {
    composer: Rc<Composer>,
    _observer: ObserverHandle,
    disposed: std::cell::Cell<bool>,
}

/// Compose's `ReusableComposition`: a composition that can be deactivated and composed again reusing its nodes
/// (every [`Composition`] can).
pub type ReusableComposition = Composition;

/// Compose's `PausableComposition`: a composition whose content can be composed a piece at a time (every
/// [`Composition`] can: [`set_pausable_content`](Composition::set_pausable_content)).
pub type PausableComposition = Composition;

impl Composition {
    /// A root composition emitting into `root` (none: nodes emitted at the top are kept by no one).
    pub fn new(root: Option<Rc<dyn Node>>) -> Composition {
        Self::make(root, None)
    }

    /// A child composition of the composition `parent` was taken in, emitting into `root` (Compose's
    /// `Composition(applier, parent)`): it sees the locals provided where `parent` was taken, its composite keys
    /// continue that position's, and it is recomposed with its parent's, after it.
    pub fn with_parent(root: Option<Rc<dyn Node>>, parent: &CompositionContext) -> Composition {
        let composition = Self::make(root, Some(parent.clone()));
        parent.0.children.borrow_mut().push(Rc::downgrade(&composition.composer));
        composition
    }

    fn make(root: Option<Rc<dyn Node>>, parent: Option<CompositionContext>) -> Composition {
        let composer = Composer::new(parent);
        {
            let mut st = composer.state.borrow_mut();
            let mut g = Group::new(None, 0, 0);
            g.node = root;
            let id = GroupId(st.groups.insert(g));
            st.root = Some(id);
        }
        let pending = composer.pending.clone();
        let observer = Snapshot::register_apply_observer(move |changed| pending.lock().extend_from_slice(changed));
        Composition { composer, _observer: observer, disposed: std::cell::Cell::new(false) }
    }

    /// Whether it was made from a context (a child composition).
    pub fn is_child(&self) -> bool {
        self.composer.parent.is_some()
    }

    /// The root scope composing `content`, made the first time (not composed yet).
    fn set_root_content(&self, content: impl Content + 'static, reuse: bool) -> ScopeId {
        assert!(!self.disposed.get(), "kompoz: the composition was disposed of");
        let mut st = self.composer.state.borrow_mut();
        let st = &mut *st;
        assert!(!st.paused, "kompoz: a paused composition of it is under way: apply or cancel it first");
        let root = st.root.unwrap();
        let body: Rc<dyn Fn(&dyn Any)> = Rc::new(move |_| content.compose());
        let reuse = std::mem::take(&mut st.deactivated) || reuse;
        let s = match st.groups.get(root.0).scope {
            Some(s) => {
                let old = st.scopes.get_mut(s.0).body.replace(body);
                st.garbage.push(Box::new(old));
                s
            }
            None => {
                let s = ScopeId(st.scopes.insert(Scope {
                    group: Some(root),
                    body: Some(body),
                    args: Some(Rc::new(())),
                    on_invalidated: None,
                    reads: Vec::new(),
                    invalid: false,
                    reuse_on_restart: false,
                }));
                st.groups.get_mut(root.0).scope = Some(s);
                s
            }
        };
        st.scopes.get_mut(s.0).reuse_on_restart |= reuse;
        s
    }

    /// Sets what it composes (Compose's `setContent`): a closure, or a `#[kompozable]` taking no arguments. A root
    /// composition composes it at the next `recompose`; a child one at once. One deactivated composes it reusing
    /// what it kept, as with [`set_content_with_reuse`](Composition::set_content_with_reuse).
    pub fn set_content(&self, content: impl Content + 'static) {
        let s = self.set_root_content(content, false);
        self.composer.invalidate_scope(s);
        self.compose_child_now();
    }

    /// Sets what it composes reusing what it composed before (Compose's `ReusableComposition.setContentWithReuse`):
    /// the groups that match are composed as new ones (their remembered values forgotten, their scopes run) and
    /// the nodes they emit reused ([`Node::on_reuse`]), not made again. A root composition composes it at the
    /// next `recompose`; a child one at once.
    pub fn set_content_with_reuse(&self, content: impl Content + 'static) {
        let s = self.set_root_content(content, true);
        self.composer.invalidate_scope(s);
        self.compose_child_now();
    }

    /// A child composition composes what waits at once (the frame time its recomposer last composed at).
    fn compose_child_now(&self) {
        if self.is_child() {
            let t = self.composer.recomposer.frame_time.get();
            self.composer.recompose(t);
        }
    }

    /// Sets what it composes without composing it (Compose's `PausableComposition.setPausableContent`): the
    /// [`PausedComposition`] it gives composes it a piece at a time as it is resumed, and applies it once
    /// complete. Until then nothing it composes is remembered or run, and it isn't recomposed by its host.
    pub fn set_pausable_content(&self, content: impl Content + 'static) -> PausedComposition {
        self.set_pausable(content, false)
    }

    /// The same reusing what it composed before (Compose's `setPausableContentWithReuse`).
    pub fn set_pausable_content_with_reuse(&self, content: impl Content + 'static) -> PausedComposition {
        self.set_pausable(content, true)
    }

    fn set_pausable(&self, content: impl Content + 'static, reuse: bool) -> PausedComposition {
        let s = self.set_root_content(content, reuse);
        self.composer.invalidate_scope(s);
        {
            let mut st = self.composer.state.borrow_mut();
            st.paused = true;
            st.cancelled = false;
        }
        PausedComposition::new(self.composer.clone())
    }

    /// Deactivates it (Compose's `ReusableComposition.deactivate`): what it remembered is forgotten and its effects
    /// disposed of, its groups and nodes kept ([`Node::on_deactivate`]) to be reused by the next
    /// [`set_content_with_reuse`](Composition::set_content_with_reuse); until then nothing in it recomposes.
    pub fn deactivate(&self) {
        {
            let mut st = self.composer.state.borrow_mut();
            let st = &mut *st;
            assert!(!st.composing, "kompoz: a composition deactivated while it composes");
            st.paused = false;
            let root = st.root.unwrap();
            for c in st.groups.get(root.0).children.clone() {
                Composer::strip(st, c, true);
            }
            if let Some(s) = st.groups.get(root.0).scope {
                Composer::clear_reads(st, s);
                st.invalid.retain(|x| *x != s);
                st.scopes.get_mut(s.0).invalid = false;
            }
            st.invalid.clear();
            st.paused_scopes.clear();
            st.frame_scopes.clear();
            st.timed_scopes.clear();
            let parked = std::mem::take(&mut st.parked);
            Composer::destroy_in_call_order(st, parked);
            st.deactivated = true;
        }
        self.composer.pending.lock().clear();
        self.composer.apply_changes();
    }

    /// Whether it is deactivated (and not composed again since).
    pub fn is_deactivated(&self) -> bool {
        self.composer.state.borrow().deactivated
    }

    /// Runs what has to run again (what read states changed since), then does the same in the child compositions
    /// made from its contexts, each after the one it was made from (Compose's Recomposer: parents before
    /// children); those deactivated, paused or disposed of are left alone. False if there was nothing to do.
    pub fn recompose(&self, frame_time_nanos: i64) -> bool {
        if self.disposed.get() || !self.composer.recomposes() {
            return false;
        }
        if !self.is_child() {
            self.composer.recomposer.frame_time.set(frame_time_nanos);
        }
        let mut any = self.composer.recompose(frame_time_nanos);
        for child in self.composer.descendants() {
            if child.recomposes() {
                any |= child.recompose(frame_time_nanos);
            }
        }
        any
    }

    /// Whether something waits to be recomposed in it or its child compositions (states changed and applied,
    /// not recomposed yet).
    pub fn has_invalidations(&self) -> bool {
        self.composer.has_invalidations()
            || self.composer.descendants().iter().any(|c| c.recomposes() && c.has_invalidations())
    }

    /// Whether a scope in it or its child compositions asked to be composed again at a frame (an animation running).
    pub fn has_frame_scopes(&self) -> bool {
        std::iter::once(self.composer.clone()).chain(self.composer.descendants()).any(|c| {
            let st = c.state.borrow();
            c.recomposes() && (!st.frame_scopes.is_empty() || !st.timed_scopes.is_empty())
        })
    }

    /// The scopes that asked for this frame (the next, or one by `frame_time_nanos`) queued to run again, in it
    /// and its child compositions.
    pub fn invalidate_frame_scopes(&self, frame_time_nanos: i64) {
        for c in std::iter::once(self.composer.clone()).chain(self.composer.descendants()) {
            c.invalidate_frame_scopes(frame_time_nanos);
        }
    }

    /// Everything composed again (the host's environment changed): its content's scope, and its child
    /// compositions'.
    pub fn invalidate_all(&self) {
        for c in std::iter::once(self.composer.clone()).chain(self.composer.descendants()) {
            if c.recomposes() {
                c.invalidate_root();
            }
        }
    }

    /// How many scopes ran and were skipped in it (not its child compositions).
    pub fn stats(&self) -> RecompositionStats {
        self.composer.state.borrow().stats
    }

    /// Disposes of it (Compose's `dispose`): all of it leaves, what it held forgotten last composed first (or
    /// abandoned, if it never entered). Dropping it does the same.
    pub fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        {
            let mut st = self.composer.state.borrow_mut();
            let st = &mut *st;
            st.paused = false;
            if let Some(root) = st.root {
                Composer::destroy(st, root);
                st.root = None;
            }
            let parked = std::mem::take(&mut st.parked);
            Composer::destroy_in_call_order(st, parked);
            // (what a paused composition would have told never is: its values are abandoned as they go)
            st.after.clear();
            st.side.clear();
        }
        if let Some(parent) = &self.composer.parent {
            parent.0.children.borrow_mut().retain(|c| c.upgrade().is_some_and(|c| !Rc::ptr_eq(&c, &self.composer)));
        }
        self.composer.apply_changes();
    }

    /// Whether it was disposed of.
    pub fn is_disposed(&self) -> bool {
        self.disposed.get()
    }
}

impl Drop for Composition {
    /// The composition disposed of.
    fn drop(&mut self) {
        self.dispose();
    }
}

impl Composer {
    /// Whether its recomposer composes it: not deactivated, nor waiting on a paused composition, nor cancelled.
    pub(crate) fn recomposes(&self) -> bool {
        let st = self.state.borrow();
        !st.deactivated && !st.paused && !st.cancelled && st.root.is_some()
    }

    fn has_invalidations(&self) -> bool {
        !self.pending.lock().is_empty() || !self.state.borrow().invalid.is_empty()
    }

    fn invalidate_frame_scopes(&self, frame_time_nanos: i64) {
        let due: Vec<ScopeId> = {
            let mut st = self.state.borrow_mut();
            let mut due = std::mem::take(&mut st.frame_scopes);
            st.timed_scopes.retain(|t| {
                if t.0 <= frame_time_nanos {
                    due.push(t.1);
                    false
                } else {
                    true
                }
            });
            due
        };
        for s in due {
            self.invalidate_scope(s);
        }
    }

    fn invalidate_root(&self) {
        let root = {
            let st = self.state.borrow();
            st.root.and_then(|r| st.groups.get(r.0).scope)
        };
        if let Some(s) = root {
            self.invalidate_scope(s);
        }
    }

    /// The compositions made from the contexts taken in it, and from theirs, each before those made from it.
    pub(crate) fn descendants(&self) -> Vec<Rc<Composer>> {
        let mut out = Vec::new();
        self.collect_descendants(&mut out);
        out
    }

    fn collect_descendants(&self, out: &mut Vec<Rc<Composer>>) {
        let contexts: Vec<Rc<ContextData>> = {
            let mut contexts = self.contexts.borrow_mut();
            contexts.retain(|c| c.strong_count() > 0);
            contexts.iter().filter_map(|c| c.upgrade()).collect()
        };
        for ctx in contexts {
            let children: Vec<Rc<Composer>> = ctx.children.borrow().iter().filter_map(|c| c.upgrade()).collect();
            for child in children {
                out.push(child.clone());
                child.collect_descendants(out);
            }
        }
    }

    /// A paused composition's next piece: what is invalid composed (pausing where `should_pause` says), held
    /// until applied. Whether it is complete.
    pub(crate) fn resume_paused(self: &Rc<Self>, should_pause: Rc<dyn Fn() -> bool>) -> bool {
        let t = self.recomposer.frame_time.get();
        self.compose_pass(t, Some(should_pause));
        self.paused_complete()
    }

    /// Whether nothing is left for a paused composition to compose (nothing it read changed since, either).
    pub(crate) fn paused_complete(&self) -> bool {
        self.take_pending();
        self.state.borrow().invalid.is_empty()
    }

    /// A complete paused composition applied: what it held is done as after any composition, and it composes as
    /// any other from now on.
    pub(crate) fn apply_paused(&self) {
        self.state.borrow_mut().paused = false;
        self.finish_composition();
        self.apply_changes();
    }

    /// A paused composition cancelled: what it remembered is abandoned when the composition is disposed of (what
    /// it would have run never runs); what left meanwhile is forgotten.
    pub(crate) fn cancel_paused(&self) {
        let (after, side) = {
            let mut st = self.state.borrow_mut();
            st.paused = false;
            st.cancelled = true;
            (std::mem::take(&mut st.after), std::mem::take(&mut st.side))
        };
        drop((after, side));
        self.apply_changes();
    }
}

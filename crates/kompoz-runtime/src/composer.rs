//! The composition: a tree of groups (one per composable call; the call site plus the order among the parent's
//! calls is a group's identity), scopes that record what their bodies read, and smart recomposition — when a
//! state changes only the scopes that read it run again, and a scope whose arguments equal last time's is
//! skipped when its parent runs.
//!
//! A group's children are kept in call order. Composing a group again walks its old children alongside the new
//! calls: the next old child is taken if its key matches (the common case, no lookup), else the key is looked
//! up among those left; whatever is left at the end has left the composition.

use crate::read::observe_reads;
use crate::snapshot::{ObserverHandle, Snapshot, SnapshotApplyResult, StateKey};
use parking_lot::Mutex;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
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
    pub(crate) observer: Option<Rc<dyn RememberObserver>>,
}

/// What a remembered value is told as it enters and leaves the composition (Compose's RememberObserver; see
/// [`remember_observer`](crate::remember_observer)). Both after the composition: what left forgotten first (in
/// the reverse of the order it was composed in), then what entered remembered (in order), before the side
/// effects. A remembered value's `Drop` runs where `on_forgotten` would.
pub trait RememberObserver {
    fn on_remembered(&self) {}
    fn on_forgotten(&self) {}
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
    pass: u64,
    frame_time: i64,
    stats: RecompositionStats,
    root: Option<GroupId>,
    composing: bool,
}

/// One composition's state, shared by what refers to it (derived states, the scope handles).
pub(crate) struct Composer {
    pub(crate) state: RefCell<ComposerState>,
    /// What applied snapshots changed, from any thread: invalidated before the next recomposition.
    pending: Arc<Mutex<Vec<StateKey>>>,
    reader: Rc<dyn Fn(StateKey)>,
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

    #[track_caller]
    pub(crate) fn require() -> Rc<Composer> {
        Composer::current().expect("kompoz: a composable called outside of a composition")
    }

    fn new() -> Rc<Composer> {
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
                    pass: 0,
                    frame_time: 0,
                    stats: RecompositionStats::default(),
                    root: None,
                    composing: false,
                }),
                pending: Arc::new(Mutex::new(Vec::new())),
                reader,
            }
        })
    }

    // ---- Reads and invalidation ----

    /// Records that the running scope read `key`.
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
        let set_holders = std::mem::take(&mut self.state.borrow_mut().set_holders);
        for key in keys {
            if !set_holders.contains(&key) {
                self.invalidate_readers(key);
            }
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
            Some(g) => g,
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

    /// The current group's position below `boundary`: the keys of the groups in between.
    pub(crate) fn key_below(&self, boundary: Option<GroupId>) -> u64 {
        let st = self.state.borrow();
        let mut key = 1469598103934665603;
        let mut g = st.frames.last().map(|f| f.group);
        while let Some(id) = g
            && Some(id) != boundary
        {
            let group = st.groups.get(id.0);
            key = mix_key(key, group.key);
            g = group.parent;
        }
        key
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
        st.garbage.push(Box::new(group.node.take()));
        st.garbage.push(Box::new(group.provided));
    }

    fn forget_slot_value(st: &mut ComposerState, slot: SlotValue) {
        // The value dropped (and its observer told) after the composition.
        st.forgets.push(Box::new(move || {
            if let Some(o) = &slot.observer {
                o.on_forgotten();
            }
            drop(slot);
        }));
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
        if let Some(o) = &observer {
            let o = o.clone();
            st.after.push(Box::new(move || o.on_remembered()));
        }
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
        let node = {
            let st = self.state.borrow();
            let g = st.frames.last().unwrap().group;
            st.groups.get(g.0).node.clone()
        };
        let node = match node {
            Some(n) => n,
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
                    }));
                    st.groups.get_mut(g.0).scope = Some(s);
                    s
                }
            }
        };
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
        {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
            let g = st.scopes.get(s.0).group.unwrap();
            st.frames.clear();
            st.locals = Self::locals_above(st, g);
            st.current_scope = None;
        }
        self.run(s);
        let update = {
            let mut st = self.state.borrow_mut();
            let st = &mut *st;
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
            st.locals.iter().rev().find(|p| p.local == local)?.holder.clone()
        };
        self.record_read(holder.key);
        Some(holder.peek())
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
            self.record_read(p.holder.key);
        }
        locals
    }

    /// Everything provided where this runs, outermost first (each read, so the reader follows them).
    pub(crate) fn locals(&self) -> Vec<Provided> {
        let locals = self.state.borrow().locals.clone();
        for p in &locals {
            self.record_read(p.holder.key);
        }
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
        }
        let snapshot = Snapshot::take_mutable_snapshot(None, None);
        snapshot.enter(|| {
            with_current(self, || {
                loop {
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
                    let still = self.state.borrow().scopes.try_get(s.0).is_some_and(|x| x.invalid);
                    if still {
                        self.restart(s);
                    }
                }
                self.finish_composition();
            })
        });
        let applied = snapshot.apply() == SnapshotApplyResult::Success;
        snapshot.dispose();
        self.state.borrow_mut().composing = false;
        assert!(applied, "kompoz: a state the composition wrote was written meanwhile on another thread");
        self.apply_changes();
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
/// read changed. The host drives it: [`recompose`](Composition::recompose) at every frame (after
/// [`Snapshot::send_apply_notifications`]).
pub struct Composition {
    composer: Rc<Composer>,
    _observer: ObserverHandle,
}

impl Composition {
    /// A composition emitting into `root` (none: nodes emitted at the top are kept by no one).
    pub fn new(root: Option<Rc<dyn Node>>) -> Composition {
        let composer = Composer::new();
        {
            let mut st = composer.state.borrow_mut();
            let mut g = Group::new(None, 0, 0);
            g.node = root;
            let id = GroupId(st.groups.insert(g));
            st.root = Some(id);
        }
        let pending = composer.pending.clone();
        let observer = Snapshot::register_apply_observer(move |changed| pending.lock().extend_from_slice(changed));
        Composition { composer, _observer: observer }
    }

    /// Sets what it composes (composed at the next `recompose`): a closure, or a `#[kompozable]` taking no
    /// arguments.
    pub fn set_content(&self, content: impl Content + 'static) {
        let s = {
            let mut st = self.composer.state.borrow_mut();
            let st = &mut *st;
            let root = st.root.unwrap();
            let body: Rc<dyn Fn(&dyn Any)> = Rc::new(move |_| content.compose());
            match st.groups.get(root.0).scope {
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
                    }));
                    st.groups.get_mut(root.0).scope = Some(s);
                    s
                }
            }
        };
        self.composer.invalidate_scope(s);
    }

    /// Runs what has to run again (what read states changed since). False if there was nothing to do.
    pub fn recompose(&self, frame_time_nanos: i64) -> bool {
        self.composer.recompose(frame_time_nanos)
    }

    /// Whether something waits to be recomposed (states changed and applied, not recomposed yet).
    pub fn has_invalidations(&self) -> bool {
        !self.composer.pending.lock().is_empty() || !self.composer.state.borrow().invalid.is_empty()
    }

    /// Whether a scope asked to be composed again at a frame (an animation running).
    pub fn has_frame_scopes(&self) -> bool {
        let st = self.composer.state.borrow();
        !st.frame_scopes.is_empty() || !st.timed_scopes.is_empty()
    }

    /// The scopes that asked for this frame (the next, or one by `frame_time_nanos`) queued to run again.
    pub fn invalidate_frame_scopes(&self, frame_time_nanos: i64) {
        let due: Vec<ScopeId> = {
            let mut st = self.composer.state.borrow_mut();
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
            self.composer.invalidate_scope(s);
        }
    }

    /// Everything composed again (the host's environment changed).
    pub fn invalidate_all(&self) {
        let root = self.composer.state.borrow().root.and_then(|r| self.composer.state.borrow().groups.get(r.0).scope);
        if let Some(s) = root {
            self.composer.invalidate_scope(s);
        }
    }

    pub fn stats(&self) -> RecompositionStats {
        self.composer.state.borrow().stats
    }
}

impl Drop for Composition {
    /// The composition disposed of: all of it leaves, what it held forgotten last composed first.
    fn drop(&mut self) {
        {
            let mut st = self.composer.state.borrow_mut();
            let st = &mut *st;
            if let Some(root) = st.root.take() {
                Composer::destroy(st, root);
            }
            let parked = std::mem::take(&mut st.parked);
            Composer::destroy_in_call_order(st, parked);
        }
        self.composer.apply_changes();
    }
}

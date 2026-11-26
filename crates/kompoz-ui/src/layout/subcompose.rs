//! `SubcomposeLayout`, as Compose's `androidx.compose.ui.layout.SubcomposeLayout`: a layout that composes its
//! content as it measures — each slot ([`SubcomposeMeasureScope::subcompose`]) in a child composition of its own,
//! made from a context taken where the layout is composed — so what the content is can depend on the constraints,
//! or on another slot's size. Its node's children are owned by its slots (Compose's
//! `LayoutNodeSubcompositionsState`): the slots used in the last measure in the order they were asked for, then
//! those kept for reuse (by a [`SubcomposeSlotReusePolicy`]: deactivated, their state gone, their nodes kept for a
//! compatible slot), then those precomposed ([`SubcomposeLayoutState::precompose`], or a piece at a time with
//! [`SubcomposeLayoutState::create_paused_precomposition`]) ahead of being asked for.
//!
//! Intrinsic measurements of it panic, as Compose's throw: what it is made of isn't known until it measures.

use crate::layout::{
    IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasurePolicy, MeasureResult, MeasureScope, PlacementScope,
};
use crate::modifier::{Modifier, NodeCx, TraversalDecision};
use crate::node::{LayoutNode, LayoutState, UsageByParent};
use crate::platform::{LocalDensity, LocalLayoutDirection};
use crate::unit::Constraints;
use kompoz_graphics::IntSize;
use kompoz_runtime::{
    Composition, CompositionContext, MutableState, Node, PausedComposition, ReusableContentHost, ShouldPauseCallback,
    SideEffect, Snapshot, current_composite_key_hash, current_kompozition_local_map, emit, kompozable,
    kompozable_block, mutable_state_of, remember, remember_kompozition_context, without_read_observation,
};
use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::{Rc, Weak};

// ---------------------------------------------------------------------------------------------------------------
// Slot ids

/// What a slot id can be: any value with `Hash` and `Eq` (Compose's `Any?` slot ids, compared by `equals`), held
/// by value.
pub trait SlotId: Any {
    /// Whether `other` is the same id (of the same type, and equal).
    fn slot_eq(&self, other: &dyn SlotId) -> bool;
    /// Hashes it (with its type).
    fn slot_hash(&self, state: &mut dyn Hasher);
    fn as_any(&self) -> &dyn Any;
    /// What it is, for messages: its value if it is a text or a number, else its type.
    fn describe(&self) -> String;
}

impl<T: Any + Hash + Eq> SlotId for T {
    fn slot_eq(&self, other: &dyn SlotId) -> bool {
        other.as_any().downcast_ref::<T>().is_some_and(|o| o == self)
    }
    fn slot_hash(&self, mut state: &mut dyn Hasher) {
        TypeId::of::<T>().hash(&mut state);
        self.hash(&mut state);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn describe(&self) -> String {
        let any = self as &dyn Any;
        macro_rules! shown {
            ($($t:ty),*) => {
                $(if let Some(v) = any.downcast_ref::<$t>() {
                    return v.to_string();
                })*
            };
        }
        shown!(String, &'static str, char, bool, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);
        format!("a {}", std::any::type_name::<T>())
    }
}

/// A slot's id, held by value (Compose's `Any?` slot id): made from any `Hash + Eq` value. Two are equal when
/// their values are (of the same type).
#[derive(Clone)]
pub struct SlotKey(Rc<dyn SlotId>);

impl SlotKey {
    pub fn new<T: Hash + Eq + 'static>(id: T) -> SlotKey {
        SlotKey(Rc::new(id))
    }

    /// Its value, if it is a `T`.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.0.as_any().downcast_ref::<T>()
    }

    /// Whether its value is `id`.
    pub fn is<T: Hash + Eq + 'static>(&self, id: &T) -> bool {
        self.downcast_ref::<T>().is_some_and(|v| v == id)
    }
}

impl PartialEq for SlotKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || self.0.slot_eq(&*other.0)
    }
}

impl Eq for SlotKey {}

impl Hash for SlotKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.slot_hash(state);
    }
}

impl std::fmt::Debug for SlotKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.describe())
    }
}

/// Kotlin's `toString()` of the `Any?` it holds (a content key's: `newStack.toString()`): a text's or a number's
/// value, else its type.
impl kompoz_runtime::kt::KtToString for SlotKey {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&self.0.describe());
    }
}

/// The id a slot kept for reuse gets once its layout itself was reused or deactivated (Compose's
/// `ReusedSlotId`): any slot can take it.
#[derive(Hash, PartialEq, Eq)]
struct ReusedSlotId;

// ---------------------------------------------------------------------------------------------------------------
// Reuse policies

/// The slot ids available to reuse, in the order they were added (Compose's `SlotIdsSet`), for
/// [`SubcomposeSlotReusePolicy::get_slots_to_retain`] to take those not to keep out of.
#[derive(Default)]
pub struct SlotIdsSet {
    ids: Vec<SlotKey>,
}

impl SlotIdsSet {
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn contains(&self, slot_id: &SlotKey) -> bool {
        self.ids.contains(slot_id)
    }

    /// Whether it has all of `slot_ids` (Compose's `containsAll`).
    pub fn contains_all(&self, slot_ids: &[SlotKey]) -> bool {
        slot_ids.iter().all(|id| self.contains(id))
    }

    /// The ids, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = &SlotKey> {
        self.ids.iter()
    }

    pub(crate) fn add(&mut self, slot_id: SlotKey) {
        if !self.contains(&slot_id) {
            self.ids.push(slot_id);
        }
    }

    /// Takes `slot_id` out; whether it was in.
    pub fn remove(&mut self, slot_id: &SlotKey) -> bool {
        let before = self.ids.len();
        self.ids.retain(|id| id != slot_id);
        before != self.ids.len()
    }

    /// Takes all of `slot_ids` out (Compose's `removeAll(slotIds)`); whether any was in.
    pub fn remove_all(&mut self, slot_ids: &[SlotKey]) -> bool {
        let before = self.ids.len();
        self.ids.retain(|id| !slot_ids.contains(id));
        before != self.ids.len()
    }

    /// Takes the ids `predicate` holds for out (Compose's `removeAll(predicate)`); whether any was.
    pub fn remove_if(&mut self, predicate: impl Fn(&SlotKey) -> bool) -> bool {
        let before = self.ids.len();
        self.ids.retain(|id| !predicate(id));
        before != self.ids.len()
    }

    /// Keeps only the ids in `slot_ids` (Compose's `retainAll(slotIds)`); whether any was taken out.
    pub fn retain_all(&mut self, slot_ids: &[SlotKey]) -> bool {
        let before = self.ids.len();
        self.ids.retain(|id| slot_ids.contains(id));
        before != self.ids.len()
    }

    /// Keeps only the ids `predicate` holds for (Compose's `retainAll(predicate)`); whether any was taken out.
    pub fn retain_if(&mut self, predicate: impl Fn(&SlotKey) -> bool) -> bool {
        let before = self.ids.len();
        self.ids.retain(|id| predicate(id));
        before != self.ids.len()
    }

    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// Takes ids out, the last added first, until it has `max_slots_to_retain_for_reuse` (Compose's `trimToSize`).
    pub fn trim_to_size(&mut self, max_slots_to_retain_for_reuse: usize) {
        self.ids.truncate(max_slots_to_retain_for_reuse);
    }
}

/// Which slots a [`SubcomposeLayout`] keeps for reuse once they aren't used (Compose's `SubcomposeSlotReusePolicy`):
/// kept slots lose their state but keep their nodes, and a new slot compatible with one is composed into it rather
/// than made anew.
pub trait SubcomposeSlotReusePolicy {
    /// Takes the slots not to keep out of `slot_ids` (those available to reuse).
    fn get_slots_to_retain(&self, slot_ids: &mut SlotIdsSet);
    /// Whether a slot kept with `reusable_slot_id` can be reused for `slot_id` (their content is alike).
    fn are_compatible(&self, slot_id: &SlotKey, reusable_slot_id: &SlotKey) -> bool;
}

/// A policy keeping up to `max_slots_to_retain_for_reuse` slots, any compatible with any other (Compose's
/// `SubcomposeSlotReusePolicy(maxSlotsToRetainForReuse)`).
pub fn SubcomposeSlotReusePolicy(max_slots_to_retain_for_reuse: usize) -> Rc<dyn SubcomposeSlotReusePolicy> {
    Rc::new(FixedCountSubcomposeSlotReusePolicy { max_slots_to_retain_for_reuse })
}

struct FixedCountSubcomposeSlotReusePolicy {
    max_slots_to_retain_for_reuse: usize,
}

impl SubcomposeSlotReusePolicy for FixedCountSubcomposeSlotReusePolicy {
    fn get_slots_to_retain(&self, slot_ids: &mut SlotIdsSet) {
        if slot_ids.len() > self.max_slots_to_retain_for_reuse {
            slot_ids.trim_to_size(self.max_slots_to_retain_for_reuse);
        }
    }
    fn are_compatible(&self, _: &SlotKey, _: &SlotKey) -> bool {
        true
    }
}

/// Keeps nothing (Compose's `NoOpSubcomposeSlotReusePolicy`, a state's default).
struct NoOpSubcomposeSlotReusePolicy;

impl SubcomposeSlotReusePolicy for NoOpSubcomposeSlotReusePolicy {
    fn get_slots_to_retain(&self, slot_ids: &mut SlotIdsSet) {
        slot_ids.clear();
    }
    fn are_compatible(&self, _: &SlotKey, _: &SlotKey) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The state

/// The state of a [`SubcomposeLayout`] (Compose's `SubcomposeLayoutState`): its reuse policy, and what composes
/// slots ahead of the measure ([`precompose`](Self::precompose),
/// [`create_paused_precomposition`](Self::create_paused_precomposition)). Clones are the same state.
#[derive(Clone)]
pub struct SubcomposeLayoutState {
    data: Rc<StateData>,
}

/// What a state holds: its policy, and the slots of the layout it was given to.
struct StateData {
    slot_reuse_policy: Rc<dyn SubcomposeSlotReusePolicy>,
    state: RefCell<Option<Rc<LayoutNodeSubcompositionsState>>>,
}

impl Default for SubcomposeLayoutState {
    /// A state keeping no slot for reuse (Compose's `SubcomposeLayoutState()`).
    fn default() -> Self {
        Self::new(Rc::new(NoOpSubcomposeSlotReusePolicy))
    }
}

impl PartialEq for SubcomposeLayoutState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }
}

impl SubcomposeLayoutState {
    /// A state keeping the slots `slot_reuse_policy` says for reuse (Compose's `SubcomposeLayoutState(policy)`).
    pub fn new(slot_reuse_policy: Rc<dyn SubcomposeSlotReusePolicy>) -> Self {
        SubcomposeLayoutState { data: Rc::new(StateData { slot_reuse_policy, state: RefCell::new(None) }) }
    }

    /// A state keeping up to `max_slots_to_retain_for_reuse` slots (Compose's deprecated
    /// `SubcomposeLayoutState(maxSlotsToRetainForReuse)`).
    pub fn with_max_slots_to_retain_for_reuse(max_slots_to_retain_for_reuse: usize) -> Self {
        Self::new(SubcomposeSlotReusePolicy(max_slots_to_retain_for_reuse))
    }

    fn state(&self) -> Rc<LayoutNodeSubcompositionsState> {
        self.data.state.borrow().clone().expect("kompoz: SubcomposeLayoutState is not attached to SubcomposeLayout")
    }

    /// Given to `node` (Compose's `setRoot`): its slots' state, made the first time, takes this policy.
    fn set_root(&self, node: &LayoutNode) -> Rc<LayoutNodeSubcompositionsState> {
        let state = node.subcompositions.borrow().clone();
        let state = state.unwrap_or_else(|| {
            let state = LayoutNodeSubcompositionsState::new(node, self.data.slot_reuse_policy.clone());
            *node.subcompositions.borrow_mut() = Some(state.clone());
            state
        });
        *self.data.state.borrow_mut() = Some(state.clone());
        state.make_sure_state_is_consistent();
        state.set_slot_reuse_policy(self.data.slot_reuse_policy.clone());
        state
    }

    /// Composes `slot_id`'s `content` now (Compose's `precompose`), so the measure that asks for it finds it
    /// composed. Dispose of it through the handle if it turns out not to be needed.
    pub fn precompose<K: Hash + Eq + 'static>(
        &self,
        slot_id: K,
        content: impl Fn() + 'static,
    ) -> PrecomposedSlotHandle {
        self.precompose_shared(slot_id, Rc::new(content))
    }

    /// [`precompose`](Self::precompose) with content kept by its instance: the same `Rc` is the same content, not
    /// composed again for itself (a lazy layout's item content, kept by its key).
    pub fn precompose_shared<K: Hash + Eq + 'static>(
        &self,
        slot_id: K,
        content: Rc<dyn Fn()>,
    ) -> PrecomposedSlotHandle {
        self.state().precompose(SlotKey::new(slot_id), content)
    }

    /// A precomposition of `slot_id` made a piece at a time (Compose's `createPausedPrecomposition`): resume it
    /// until it is complete (between frames), then apply it; the measure that asks for the slot meanwhile completes
    /// it.
    pub fn create_paused_precomposition<K: Hash + Eq + 'static>(
        &self,
        slot_id: K,
        content: impl Fn() + 'static,
    ) -> PausedPrecomposition {
        self.create_paused_precomposition_shared(slot_id, Rc::new(content))
    }

    /// [`create_paused_precomposition`](Self::create_paused_precomposition) with content kept by its instance.
    pub fn create_paused_precomposition_shared<K: Hash + Eq + 'static>(
        &self,
        slot_id: K,
        content: Rc<dyn Fn()>,
    ) -> PausedPrecomposition {
        self.state().precompose_paused(SlotKey::new(slot_id), content)
    }

    pub(crate) fn force_recompose_children(&self) {
        self.state().force_recompose_children();
    }
}

/// A state keeping the slots `slot_reuse_policy` says for reuse (Compose's `SubcomposeLayoutState(slotReusePolicy)`;
/// [`SubcomposeLayoutState::default`] is its `SubcomposeLayoutState()`, keeping none).
///
/// Kotlin: `SubcomposeLayoutState(slotReusePolicy: SubcomposeSlotReusePolicy = SubcomposeSlotReusePolicy(0))`
pub fn SubcomposeLayoutState(slot_reuse_policy: Rc<dyn SubcomposeSlotReusePolicy>) -> SubcomposeLayoutState {
    SubcomposeLayoutState::new(slot_reuse_policy)
}

/// A slot composed ahead of the measure ([`SubcomposeLayoutState::precompose`]; Compose's
/// `PrecomposedSlotHandle`).
pub struct PrecomposedSlotHandle {
    /// None when the layout wasn't attached (nothing was composed).
    state: Option<Weak<LayoutNodeSubcompositionsState>>,
    slot_id: SlotKey,
    /// The indices premeasured through it.
    premeasured: RefCell<Vec<usize>>,
}

impl PrecomposedSlotHandle {
    /// Disposes of the slot's content (Compose's `dispose`), unless a measure used the slot since.
    pub fn dispose(&self) {
        if let Some(state) = self.state.as_ref().and_then(Weak::upgrade) {
            state.dispose_precomposed_slot(&self.slot_id);
        }
    }

    fn slot(&self) -> Option<Rc<Slot>> {
        let state = self.state.as_ref()?.upgrade()?;
        state.precompose_map.borrow().get(&self.slot_id).cloned()
    }

    /// How many layouts its content emitted (Compose's `placeablesCount`).
    pub fn placeables_count(&self) -> usize {
        self.slot().map_or(0, |s| s.root.nodes.borrow().len())
    }

    /// Measures the `index`th layout of it within `constraints` now (Compose's `premeasure`). Panics on an index
    /// out of bounds.
    pub fn premeasure(&self, index: usize, constraints: Constraints) {
        let Some(state) = self.state.as_ref().and_then(Weak::upgrade) else { return };
        let Some(slot) = self.slot() else { return };
        let nodes = slot.root.nodes.borrow().clone();
        let Some(root) = state.root.upgrade().filter(|r| r.is_attached()) else { return };
        assert!(index < nodes.len(), "kompoz: Index ({index}) is out of bound of [0, {})", nodes.len());
        let node = &nodes[index];
        assert!(!node.is_placed(), "kompoz: Pre-measure called on node that is not placed");
        if let Some(owner) = root.owner() {
            root.ignoring_remeasure_requests(|| owner.measure_and_layout_node(node, constraints));
        }
        let mut premeasured = self.premeasured.borrow_mut();
        if !premeasured.contains(&index) {
            premeasured.push(index);
        }
    }

    /// Visits the [`TraversableNode`](crate::modifier::TraversableNode)s with `key` in its content, depth first,
    /// each telling whether to go on (Compose's `traverseDescendants`).
    pub fn traverse_descendants(&self, key: &dyn Any, block: &mut dyn FnMut(&NodeCx) -> TraversalDecision) {
        let Some(slot) = self.slot() else { return };
        let nodes: Vec<Rc<LayoutNode>> = slot.root.nodes.borrow().iter().filter(|n| n.is_attached()).cloned().collect();
        crate::modifier::traverse_descendants_of(nodes, key, block);
    }

    /// The size the `index`th layout of it was premeasured to (Compose's `getSize`); zero before it was.
    pub fn get_size(&self, index: usize) -> IntSize {
        let Some(slot) = self.slot() else { return IntSize::Zero };
        let nodes = slot.root.nodes.borrow();
        assert!(index < nodes.len(), "kompoz: Index ({index}) is out of bound of [0, {})", nodes.len());
        if self.premeasured.borrow().contains(&index) && nodes[index].is_attached() {
            return nodes[index].measured_size.get();
        }
        IntSize::Zero
    }
}

/// A slot's composition made a piece at a time ([`SubcomposeLayoutState::create_paused_precomposition`]; Compose's
/// `PausedPrecomposition`): [`resume`](Self::resume) it until it is complete, then [`apply`](Self::apply) it.
pub struct PausedPrecomposition {
    /// None when the layout wasn't attached (nothing to compose: complete at once).
    state: Option<Weak<LayoutNodeSubcompositionsState>>,
    slot_id: SlotKey,
}

impl PausedPrecomposition {
    fn slot(&self) -> Option<Rc<Slot>> {
        let state = self.state.as_ref()?.upgrade()?;
        state.precompose_map.borrow().get(&self.slot_id).cloned()
    }

    /// Whether nothing is left to compose (Compose's `isComplete`).
    pub fn is_complete(&self) -> bool {
        match self.slot() {
            Some(slot) => slot.paused.borrow().as_ref().is_none_or(|p| p.is_complete()),
            None => true,
        }
    }

    /// Composes what is left, pausing when `should_pause` says so (Compose's `resume`); whether it is complete.
    pub fn resume(&self, should_pause: impl ShouldPauseCallback + 'static) -> bool {
        let Some(slot) = self.slot() else { return true };
        let complete = slot.paused.borrow().as_ref().is_none_or(|p| p.is_complete());
        if complete {
            return true;
        }
        without_read_observation(|| {
            let paused = slot.paused.borrow();
            paused.as_ref().unwrap().resume(should_pause)
        })
    }

    /// Applies it (Compose's `apply`): its content goes into the layout, kept as precomposed. The handle disposes
    /// of it, premeasures it.
    pub fn apply(&self) -> PrecomposedSlotHandle {
        let state = self.state.as_ref().and_then(Weak::upgrade);
        if let (Some(state), Some(slot)) = (&state, self.slot()) {
            state.apply_paused_precomposition(&slot, false);
        }
        PrecomposedSlotHandle {
            state: self.state.clone(),
            slot_id: self.slot_id.clone(),
            premeasured: RefCell::new(Vec::new()),
        }
    }

    /// Cancels it (Compose's `cancel`): the slot is disposed of, if its composition was still waiting to be applied.
    pub fn cancel(&self) {
        let Some(state) = self.state.as_ref().and_then(Weak::upgrade) else { return };
        if self.slot().is_some_and(|s| s.paused.borrow().is_some()) {
            state.dispose_precomposed_slot(&self.slot_id);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Slots

/// What a slot's composition emits into (Compose's virtual LayoutNode): the layouts it emitted, which the
/// subcomposing layout takes as children in the slot's place.
struct SlotRoot {
    nodes: RefCell<Vec<Rc<LayoutNode>>>,
    state: Weak<LayoutNodeSubcompositionsState>,
}

impl Node for SlotRoot {
    fn set_children(&self, children: &[Rc<dyn Node>]) {
        let nodes = children
            .iter()
            .map(|c| (c.clone() as Rc<dyn Any>).downcast::<LayoutNode>().expect("kompoz: a node of another kind"))
            .collect();
        *self.nodes.borrow_mut() = nodes;
        if let Some(state) = self.state.upgrade() {
            state.sync_children();
        }
    }
}

/// A slot (Compose's `NodeState` with its virtual node): its id, its content, its composition.
struct Slot {
    root: Rc<SlotRoot>,
    slot_id: RefCell<SlotKey>,
    content: RefCell<Option<Rc<dyn Fn()>>>,
    composition: RefCell<Option<Composition>>,
    force_recompose: Cell<bool>,
    force_reuse: Cell<bool>,
    paused: RefCell<Option<PausedComposition>>,
    /// Whether its content is active (read by its `ReusableContentHost`): a new state each time it is reused or
    /// deactivated by force, so the change isn't told to anyone.
    active: RefCell<MutableState<bool>>,
    composed_with_reusable_content_host: Cell<bool>,
    /// The layouts the layout has as this slot's (what it emitted, as of the last sync while not paused).
    synced: RefCell<Vec<Rc<LayoutNode>>>,
}

impl Slot {
    fn active(&self) -> bool {
        self.active.borrow().peek()
    }

    fn nodes(&self) -> Vec<Rc<LayoutNode>> {
        self.root.nodes.borrow().clone()
    }
}

/// A subcomposing layout node's slots and their compositions (Compose's `LayoutNodeSubcompositionsState`), kept by
/// the node: its children are the slots' layouts — the active slots (used in the last measure), then
/// `reusable_count` kept for reuse, then `precomposed_count` precomposed.
pub(crate) struct LayoutNodeSubcompositionsState {
    this: Weak<LayoutNodeSubcompositionsState>,
    root: Weak<LayoutNode>,
    composition_context: RefCell<Option<CompositionContext>>,
    slot_reuse_policy: RefCell<Rc<dyn SubcomposeSlotReusePolicy>>,
    current_index: Cell<usize>,
    /// The slots, in their children's order (Compose's `foldedChildren`).
    folded: RefCell<Vec<Rc<Slot>>>,
    /// The active slots by id (not the precomposed or reusable ones).
    slot_id_to_node: RefCell<HashMap<SlotKey, Rc<Slot>>>,
    precompose_map: RefCell<HashMap<SlotKey, Rc<Slot>>>,
    reusable_slot_ids: RefCell<SlotIdsSet>,
    reusable_count: Cell<usize>,
    precomposed_count: Cell<usize>,
    /// In a lookahead scope, how many slots the approach measure asked for so far (Compose's `currentApproachIndex`).
    current_approach_index: Cell<usize>,
    /// The slots the approach pass precomposed, which it disposes of once neither pass needs them (Compose's
    /// `approachPrecomposeSlotHandleMap`).
    approach_precompose_slot_handle_map: RefCell<Vec<(SlotKey, PrecomposedSlotHandle)>>,
    /// The ids of the slots the approach pass needs, in the order it asked for them; none past
    /// `current_approach_index` (Compose's `slotIdsOfCompositionsNeededInApproach`).
    slot_ids_of_compositions_needed_in_approach: RefCell<Vec<Option<SlotKey>>>,
}

/// What asking a subcomposing layout for an intrinsic size panics with (Compose's `NoIntrinsicsMessage`).
pub const NoIntrinsicsMessage: &str = "Asking for intrinsic measurements of SubcomposeLayout layouts is not \
    supported. This includes components that are built on top of SubcomposeLayout, such as lazy lists, \
    BoxWithConstraints, TabRow, etc. To mitigate this:\n- if intrinsic measurements are used to achieve 'match \
    parent' sizing, consider replacing the parent of the component with a custom layout which controls the order \
    in which children are measured, making intrinsic measurement not needed\n- adding a size modifier to the \
    component, in order to fast return the queried intrinsic measurement.";

impl LayoutNodeSubcompositionsState {
    fn new(root: &LayoutNode, slot_reuse_policy: Rc<dyn SubcomposeSlotReusePolicy>) -> Rc<Self> {
        Rc::new_cyclic(|this| LayoutNodeSubcompositionsState {
            this: this.clone(),
            root: root.this.clone(),
            composition_context: RefCell::new(None),
            slot_reuse_policy: RefCell::new(slot_reuse_policy),
            current_index: Cell::new(0),
            folded: RefCell::new(Vec::new()),
            slot_id_to_node: RefCell::new(HashMap::new()),
            precompose_map: RefCell::new(HashMap::new()),
            reusable_slot_ids: RefCell::new(SlotIdsSet::default()),
            reusable_count: Cell::new(0),
            precomposed_count: Cell::new(0),
            current_approach_index: Cell::new(0),
            approach_precompose_slot_handle_map: RefCell::new(Vec::new()),
            slot_ids_of_compositions_needed_in_approach: RefCell::new(Vec::new()),
        })
    }

    fn root(&self) -> Rc<LayoutNode> {
        self.root.upgrade().expect("kompoz: a subcomposing layout's state outlived its node")
    }

    /// Another policy: applied after the next measure (its slots all available to reuse till then).
    fn set_slot_reuse_policy(&self, policy: Rc<dyn SubcomposeSlotReusePolicy>) {
        let same = Rc::ptr_eq(&*self.slot_reuse_policy.borrow(), &policy);
        if !same {
            *self.slot_reuse_policy.borrow_mut() = policy;
            self.mark_active_nodes_as_reused(false);
            self.root().request_remeasure(false);
        }
    }

    pub(crate) fn on_reuse(&self) {
        self.mark_active_nodes_as_reused(false);
    }

    pub(crate) fn on_deactivate(&self) {
        self.mark_active_nodes_as_reused(true);
    }

    pub(crate) fn on_release(&self) {
        self.dispose_current_nodes();
    }

    /// The layout's children made the slots' layouts, in the slots' order (a paused slot's as they were before it
    /// was paused: what it composes joins once applied).
    fn sync_children(&self) {
        let Some(root) = self.root.upgrade() else { return };
        let mut children = Vec::new();
        for slot in self.folded.borrow().iter() {
            if slot.paused.borrow().is_none() {
                *slot.synced.borrow_mut() = slot.nodes();
            }
            children.extend(slot.synced.borrow().iter().cloned());
        }
        let changed = root.ignoring_remeasure_requests(|| {
            let before = root.children();
            root.set_layout_children(children);
            let after = root.children();
            before.len() != after.len() || before.iter().zip(&after).any(|(a, b)| !Rc::ptr_eq(a, b))
        });
        // (a slot's composition recomposed on its own, outside the layout's measure — its content read a state —
        // changed the layouts: measured again, as Compose's virtual node's insertAt/removeAt invalidate its parent;
        // while measuring, what subcompose gives is measured now)
        let measuring = matches!(
            root.layout_state.get(),
            LayoutState::Measuring
                | LayoutState::LayingOut
                | LayoutState::LookaheadMeasuring
                | LayoutState::LookaheadLayingOut
        );
        if changed && !measuring {
            root.invalidate_measurements();
        }
    }

    /// Composes `slot_id`'s `content` while measuring (Compose's `subcompose`): the slot used before, a
    /// precomposed one, one kept for reuse, or a new one; moved to its place among the active slots. Its layouts,
    /// to measure (in lookahead, their lookahead pass delegates).
    fn subcompose(&self, slot_id: SlotKey, content: Rc<dyn Fn()>) -> Vec<Rc<dyn Measurable>> {
        self.make_sure_state_is_consistent();
        let root = self.root();
        let layout_state = root.layout_state.get();
        assert!(
            matches!(
                layout_state,
                LayoutState::Measuring
                    | LayoutState::LayingOut
                    | LayoutState::LookaheadMeasuring
                    | LayoutState::LookaheadLayingOut
            ),
            "kompoz: subcompose can only be used inside the measure or layout blocks"
        );
        let existing = self.slot_id_to_node.borrow().get(&slot_id).cloned();
        let slot = match existing {
            Some(slot) => slot,
            None => {
                let precomposed = self.precompose_map.borrow_mut().remove(&slot_id);
                let slot = match precomposed {
                    Some(slot) => {
                        assert!(self.precomposed_count.get() > 0);
                        self.precomposed_count.set(self.precomposed_count.get() - 1);
                        slot
                    }
                    None => self
                        .take_node_from_reusables(&slot_id)
                        .unwrap_or_else(|| self.create_node_at(self.current_index.get(), slot_id.clone())),
                };
                self.slot_id_to_node.borrow_mut().insert(slot_id.clone(), slot.clone());
                slot
            }
        };
        let current = self.current_index.get();
        let at_current = self.folded.borrow().get(current).is_some_and(|s| Rc::ptr_eq(s, &slot));
        if !at_current {
            let item_index = self.folded.borrow().iter().position(|s| Rc::ptr_eq(s, &slot)).unwrap();
            assert!(
                item_index >= current,
                "kompoz: Key \"{slot_id:?}\" was already used. If you are using LazyColumn/Row please make sure you \
                 provide a unique key for each item."
            );
            if current != item_index {
                self.move_slots(item_index, current, 1);
            }
        }
        self.current_index.set(current + 1);
        self.subcompose_slot(&slot, false, content);
        if layout_state == LayoutState::Measuring || layout_state == LayoutState::LayingOut {
            slot.nodes().into_iter().map(|n| n as Rc<dyn Measurable>).collect()
        } else {
            slot.nodes().into_iter().map(|n| n.lookahead_pass_delegate() as Rc<dyn Measurable>).collect()
        }
    }

    /// The approach measure's `subcompose` (Compose's ApproachMeasureScopeImpl `subcompose`): the layouts the lookahead
    /// pass composed for `slot_id`, or, the lookahead not having asked for it, the slot composed for the approach.
    fn subcompose_in_approach(&self, slot_id: SlotKey, content: Rc<dyn Fn()>) -> Vec<Rc<dyn Measurable>> {
        let node_in_slot = self.slot_id_to_node.borrow().get(&slot_id).cloned();
        if let Some(slot) = node_in_slot {
            let index = self.folded.borrow().iter().position(|s| Rc::ptr_eq(s, &slot));
            if index.is_some_and(|i| i < self.current_index.get()) {
                return slot.nodes().into_iter().map(|n| n as Rc<dyn Measurable>).collect();
            }
        }
        self.approach_subcompose(slot_id, content)
    }

    /// A slot the approach needs but the lookahead didn't compose (Compose's `approachSubcompose`): precomposed for
    /// the approach (or one the lookahead composed and no longer needs taken over), its layouts measured and placed
    /// in lookahead by their own approach (they missed their parent's lookahead).
    fn approach_subcompose(&self, slot_id: SlotKey, content: Rc<dyn Fn()>) -> Vec<Rc<dyn Measurable>> {
        let index = self.current_approach_index.get();
        {
            let mut needed = self.slot_ids_of_compositions_needed_in_approach.borrow_mut();
            assert!(
                needed.len() >= index,
                "Error: currentApproachIndex cannot be greater than the size of theapproachComposedSlotIds list."
            );
            if needed.len() == index {
                needed.push(Some(slot_id.clone()));
            } else {
                needed[index] = Some(slot_id.clone());
            }
        }
        self.current_approach_index.set(index + 1);
        let node_for_slot = self.slot_id_to_node.borrow().get(&slot_id).cloned();
        let precomposed = self.precompose_map.borrow().contains_key(&slot_id);
        if !precomposed && node_for_slot.is_none() {
            // Not composed in lookahead nor precomposed: precomposed for the approach, which disposes of it once
            // it needs it no longer.
            let handle = self.precompose(slot_id.clone(), content);
            self.approach_precompose_slot_handle_map.borrow_mut().push((slot_id.clone(), handle));
        } else {
            if let (false, Some(slot)) = (precomposed, &node_for_slot) {
                // Composed in lookahead, not needed there any more: the approach takes it over (moved to the
                // precomposed ones at the end), to dispose of it once it needs it no longer.
                let index = self.folded.borrow().iter().position(|s| Rc::ptr_eq(s, slot)).unwrap();
                let len = self.folded.borrow().len();
                self.move_slots(index, len - 1, 1);
                self.precomposed_count.set(self.precomposed_count.get() + 1);
                self.slot_id_to_node.borrow_mut().remove(&slot_id);
                self.precompose_map.borrow_mut().insert(slot_id.clone(), slot.clone());
                let handle = self.create_precomposed_slot_handle(slot_id.clone());
                self.approach_precompose_slot_handle_map.borrow_mut().push((slot_id.clone(), handle));
                if self.root().is_attached() {
                    self.make_sure_state_is_consistent();
                }
            }
            let slot = self.precompose_map.borrow().get(&slot_id).cloned();
            if let Some(slot) = slot {
                if slot.force_recompose.get() {
                    self.subcompose_slot(&slot, false, content);
                }
                if slot.paused.borrow().is_some() {
                    self.apply_paused_precomposition(&slot, true);
                }
            }
        }
        let slot = self.precompose_map.borrow().get(&slot_id).cloned();
        match slot {
            Some(slot) => slot
                .nodes()
                .into_iter()
                .map(|n| {
                    // (Compose's `markDetachedFromParentLookaheadPass`)
                    n.detached_from_parent_lookahead_pass.set(true);
                    n as Rc<dyn Measurable>
                })
                .collect(),
            None => Vec::new(),
        }
    }

    /// The slots the approach owns and no pass needs any more disposed of (Compose's
    /// `disposeUnusedSlotsInApproach`).
    fn dispose_unused_slots_in_approach(&self) {
        let entries = std::mem::take(&mut *self.approach_precompose_slot_handle_map.borrow_mut());
        let mut kept = Vec::new();
        for (slot_id, handle) in entries {
            let position = self
                .slot_ids_of_compositions_needed_in_approach
                .borrow()
                .iter()
                .position(|s| s.as_ref() == Some(&slot_id));
            match position {
                Some(i) if i < self.current_approach_index.get() => kept.push((slot_id, handle)),
                _ => {
                    if let Some(i) = position {
                        self.slot_ids_of_compositions_needed_in_approach.borrow_mut()[i] = None;
                    }
                    if self.precompose_map.borrow().contains_key(&slot_id) {
                        handle.dispose();
                    }
                }
            }
        }
        let mut map = self.approach_precompose_slot_handle_map.borrow_mut();
        kept.append(&mut map);
        *map = kept;
    }

    /// Composes a slot if its content changed, it has invalidations or is to be composed again (Compose's
    /// `subcompose(node, slotId, pausable, content)`): a paused composition of other content is cancelled, of the
    /// same content completed and applied (or, pausable, left).
    fn subcompose_slot(&self, slot: &Rc<Slot>, pausable: bool, content: Rc<dyn Fn()>) {
        let content_changed = slot.content.borrow().as_ref().is_none_or(|c| !Rc::ptr_eq(c, &content));
        if slot.paused.borrow().is_some() {
            if content_changed {
                self.cancel_paused_precomposition(slot);
            } else if pausable {
                return;
            } else {
                self.apply_paused_precomposition(slot, true);
            }
        }
        let has_pending_changes = slot.composition.borrow().as_ref().is_none_or(|c| c.has_invalidations());
        if content_changed || has_pending_changes || slot.force_recompose.get() {
            *slot.content.borrow_mut() = Some(content);
            self.compose_slot(slot, pausable);
            slot.force_recompose.set(false);
        }
    }

    /// Sets the slot's content in its composition (made the first time, from the layout's context), inside a
    /// `ReusableContentHost` driven by the slot's active state, pausable or not, reusing what it had when the slot
    /// was taken for reuse.
    fn compose_slot(&self, slot: &Rc<Slot>, pausable: bool) {
        assert!(slot.paused.borrow().is_none(), "kompoz: new subcompose call while paused composition is still active");
        let root = self.root();
        without_read_observation(|| {
            root.ignoring_remeasure_requests(|| {
                let disposed = slot.composition.borrow().as_ref().is_none_or(|c| c.is_disposed());
                if disposed {
                    let context = self
                        .composition_context
                        .borrow()
                        .clone()
                        .expect("kompoz: parent composition reference not set");
                    let composition = Composition::with_parent(Some(slot.root.clone() as Rc<dyn Node>), &context);
                    *slot.composition.borrow_mut() = Some(composition);
                }
                let content = slot.content.borrow().clone().expect("a slot's content");
                slot.composed_with_reusable_content_host.set(true);
                let weak = Rc::downgrade(slot);
                let composable = move || {
                    let active = weak.upgrade().is_some_and(|s| s.active.borrow().get());
                    let content = content.clone();
                    kompozable_block!({
                        ReusableContentHost(active, move || content());
                    });
                };
                let composition = slot.composition.borrow();
                let composition = composition.as_ref().unwrap();
                let reuse = slot.force_reuse.get();
                if pausable {
                    let paused = if reuse {
                        composition.set_pausable_content_with_reuse(composable)
                    } else {
                        composition.set_pausable_content(composable)
                    };
                    *slot.paused.borrow_mut() = Some(paused);
                } else if reuse {
                    composition.set_content_with_reuse(composable);
                } else {
                    composition.set_content(composable);
                }
                slot.force_reuse.set(false);
            })
        });
    }

    /// After the measure placed what it used: the slots from `start_index` on (not used) are kept for reuse, as
    /// many as the policy says (their content deactivated), or disposed of (Compose's
    /// `disposeOrReuseStartingFromIndex`).
    fn dispose_or_reuse_starting_from_index(&self, start_index: usize) {
        self.reusable_count.set(0);
        let len = self.folded.borrow().len();
        let precomposed = self.precomposed_count.get();
        let mut need_apply_notification = false;
        if start_index + precomposed < len {
            let last_reusable_index = len - precomposed - 1;
            {
                let mut ids = self.reusable_slot_ids.borrow_mut();
                ids.clear();
                for i in start_index..=last_reusable_index {
                    let id = self.folded.borrow()[i].slot_id.borrow().clone();
                    ids.add(id);
                }
            }
            let policy = self.slot_reuse_policy.borrow().clone();
            policy.get_slots_to_retain(&mut self.reusable_slot_ids.borrow_mut());
            without_read_observation(|| {
                for i in (start_index..=last_reusable_index).rev() {
                    let slot = self.folded.borrow()[i].clone();
                    let slot_id = slot.slot_id.borrow().clone();
                    if self.reusable_slot_ids.borrow().contains(&slot_id) {
                        self.reusable_count.set(self.reusable_count.get() + 1);
                        if slot.active() {
                            reset_layout_state(&slot);
                            self.reuse_composition(&slot, false);
                            if slot.composed_with_reusable_content_host.get() {
                                need_apply_notification = true;
                            }
                        }
                    } else {
                        let root = self.root();
                        root.ignoring_remeasure_requests(|| {
                            self.folded.borrow_mut().remove(i);
                            let composition = slot.composition.borrow_mut().take();
                            if let Some(c) = composition {
                                c.dispose();
                            }
                            self.sync_children();
                        });
                    }
                    // (not active any more)
                    let mut active = self.slot_id_to_node.borrow_mut();
                    if active.get(&slot_id).is_some_and(|s| Rc::ptr_eq(s, &slot)) {
                        active.remove(&slot_id);
                    }
                }
            });
        }
        if need_apply_notification {
            Snapshot::send_apply_notifications();
        }
        self.make_sure_state_is_consistent();
    }

    /// All the slots kept for reuse, their ids forgotten (Compose's `markActiveNodesAsReused`): the layout itself
    /// reused (or deactivated, `deactivate`: their compositions deactivated now).
    fn mark_active_nodes_as_reused(&self, deactivate: bool) {
        self.precomposed_count.set(0);
        self.precompose_map.borrow_mut().clear();
        let count = self.folded.borrow().len();
        if self.reusable_count.get() != count {
            self.reusable_count.set(count);
            without_read_observation(|| {
                let slots = self.folded.borrow().clone();
                for slot in slots {
                    if slot.active() {
                        reset_layout_state(&slot);
                        self.reuse_composition(&slot, deactivate);
                        *slot.slot_id.borrow_mut() = SlotKey::new(ReusedSlotId);
                    }
                }
            });
            self.slot_id_to_node.borrow_mut().clear();
        }
        self.make_sure_state_is_consistent();
    }

    /// Every slot disposed of, the layout's children with them (Compose's `disposeCurrentNodes`: the layout left).
    fn dispose_current_nodes(&self) {
        let slots = std::mem::take(&mut *self.folded.borrow_mut());
        for slot in &slots {
            slot.paused.borrow_mut().take();
            let composition = slot.composition.borrow_mut().take();
            if let Some(c) = composition {
                c.dispose();
            }
        }
        if let Some(root) = self.root.upgrade() {
            root.ignoring_remeasure_requests(|| root.set_layout_children(Vec::new()));
        }
        self.slot_id_to_node.borrow_mut().clear();
        self.precompose_map.borrow_mut().clear();
        self.precomposed_count.set(0);
        self.reusable_count.set(0);
        self.make_sure_state_is_consistent();
    }

    fn make_sure_state_is_consistent(&self) {
        let count = self.folded.borrow().len();
        let (reusable, precomposed) = (self.reusable_count.get(), self.precomposed_count.get());
        assert!(
            count >= reusable + precomposed,
            "kompoz: Incorrect state. Total children {count}. Reusable children {reusable}. Precomposed children \
             {precomposed}"
        );
        let map = self.precompose_map.borrow().len();
        assert!(map == precomposed, "kompoz: Incorrect state. Precomposed children {precomposed}. Map size {map}");
    }

    /// A kept slot for `slot_id`: one with the same id, else the last compatible one (Compose's
    /// `takeNodeFromReusables`), moved to the start of the kept ones and made active, to be composed again in
    /// reuse.
    fn take_node_from_reusables(&self, slot_id: &SlotKey) -> Option<Rc<Slot>> {
        let reusable = self.reusable_count.get();
        if reusable == 0 {
            return None;
        }
        let end = self.folded.borrow().len() - self.precomposed_count.get();
        let start = end - reusable;
        let slots = self.folded.borrow().clone();
        let mut chosen = (start..end).rev().find(|&i| *slots[i].slot_id.borrow() == *slot_id);
        if chosen.is_none() {
            let policy = self.slot_reuse_policy.borrow().clone();
            chosen = (start..end).rev().find(|&i| {
                let id = slots[i].slot_id.borrow().clone();
                id.is(&ReusedSlotId) || policy.are_compatible(slot_id, &id)
            });
            if let Some(i) = chosen {
                *slots[i].slot_id.borrow_mut() = slot_id.clone();
            }
        }
        let index = chosen?;
        if index != start {
            self.move_slots(index, start, 1);
        }
        self.reusable_count.set(reusable - 1);
        let slot = self.folded.borrow()[start].clone();
        // (a new state: the change isn't told to anyone)
        *slot.active.borrow_mut() = mutable_state_of(true);
        slot.force_reuse.set(true);
        slot.force_recompose.set(true);
        Some(slot)
    }

    /// A new slot for `slot_id` at `index` (Compose's `createNodeAt`, and its node state).
    fn create_node_at(&self, index: usize, slot_id: SlotKey) -> Rc<Slot> {
        let slot = Rc::new(Slot {
            root: Rc::new(SlotRoot { nodes: RefCell::new(Vec::new()), state: self.this.clone() }),
            slot_id: RefCell::new(slot_id),
            content: RefCell::new(None),
            composition: RefCell::new(None),
            force_recompose: Cell::new(false),
            force_reuse: Cell::new(false),
            paused: RefCell::new(None),
            active: RefCell::new(mutable_state_of(true)),
            composed_with_reusable_content_host: Cell::new(false),
            synced: RefCell::new(Vec::new()),
        });
        self.folded.borrow_mut().insert(index, slot.clone());
        slot
    }

    /// Moves `count` slots from `from` to `to` (Compose's `move`).
    fn move_slots(&self, from: usize, to: usize, count: usize) {
        {
            let mut folded = self.folded.borrow_mut();
            let moved: Vec<Rc<Slot>> = folded.drain(from..from + count).collect();
            let to = to.min(folded.len());
            for (k, slot) in moved.into_iter().enumerate() {
                folded.insert(to + k, slot);
            }
        }
        self.sync_children();
    }

    /// The measure policy running `block` with a scope that subcomposes (Compose's `createMeasurePolicy`): the
    /// slots it asked for in order; once it has placed, those it no longer asks for kept for reuse or disposed
    /// of. Its intrinsics panic.
    fn create_measure_policy(&self, block: Rc<SubcomposeMeasureBlock>) -> Rc<dyn MeasurePolicy> {
        Rc::new(SubcomposeMeasurePolicy { state: self.this.clone(), block })
    }

    fn precompose(&self, slot_id: SlotKey, content: Rc<dyn Fn()>) -> PrecomposedSlotHandle {
        self.precompose_slot(slot_id.clone(), content, false);
        self.create_precomposed_slot_handle(slot_id)
    }

    /// Compose's `precompose(slotId, content, pausable)`: unless the slot is active, its precomposed slot (one
    /// kept for reuse moved to the end, or a new one at the end) composed.
    fn precompose_slot(&self, slot_id: SlotKey, content: Rc<dyn Fn()>, pausable: bool) {
        if !self.root.upgrade().is_some_and(|r| r.is_attached()) {
            return;
        }
        self.make_sure_state_is_consistent();
        if self.slot_id_to_node.borrow().contains_key(&slot_id) {
            return;
        }
        // (the approach yields what it precomposed of the slot to whoever precomposes it now)
        self.approach_precompose_slot_handle_map.borrow_mut().retain(|(id, _)| *id != slot_id);
        let existing = self.precompose_map.borrow().get(&slot_id).cloned();
        let slot = match existing {
            Some(slot) => slot,
            None => {
                let slot = match self.take_node_from_reusables(&slot_id) {
                    Some(reused) => {
                        // to the end, where the precomposed slots are
                        let index = self.folded.borrow().iter().position(|s| Rc::ptr_eq(s, &reused)).unwrap();
                        let len = self.folded.borrow().len();
                        self.move_slots(index, len - 1, 1);
                        reused
                    }
                    None => {
                        let len = self.folded.borrow().len();
                        self.create_node_at(len, slot_id.clone())
                    }
                };
                self.precomposed_count.set(self.precomposed_count.get() + 1);
                self.precompose_map.borrow_mut().insert(slot_id.clone(), slot.clone());
                slot
            }
        };
        *slot.slot_id.borrow_mut() = slot_id;
        self.subcompose_slot(&slot, pausable, content);
    }

    /// Lets a slot go (Compose's `reuseComposition`): its content deactivated through its host (or, forced or
    /// without one, its composition deactivated); a paused composition of it cancelled.
    fn reuse_composition(&self, slot: &Rc<Slot>, force_deactivate: bool) {
        if !force_deactivate && slot.composed_with_reusable_content_host.get() {
            slot.active.borrow().set(false);
        } else {
            *slot.active.borrow_mut() = mutable_state_of(false);
        }
        if slot.paused.borrow().is_some() {
            self.cancel_paused_precomposition(slot);
        } else if force_deactivate || !slot.composed_with_reusable_content_host.get() {
            let composition = slot.composition.borrow();
            if let Some(c) = composition.as_ref() {
                c.deactivate();
            }
        }
    }

    /// Compose's `cancelPausedPrecomposition`: cancelled, its composition disposed of.
    fn cancel_paused_precomposition(&self, slot: &Rc<Slot>) {
        let paused = slot.paused.borrow_mut().take();
        if let Some(p) = paused {
            p.cancel();
            let composition = slot.composition.borrow_mut().take();
            if let Some(c) = composition {
                c.dispose();
            }
            self.sync_children();
        }
    }

    /// A precomposed slot let go (Compose's `disposePrecomposedSlot`): moved to the kept ones, then kept or
    /// disposed of by the policy.
    fn dispose_precomposed_slot(&self, slot_id: &SlotKey) {
        self.make_sure_state_is_consistent();
        let slot = self.precompose_map.borrow_mut().remove(slot_id);
        if let Some(slot) = slot {
            assert!(self.precomposed_count.get() > 0, "kompoz: No pre-composed items to dispose");
            let item_index = self.folded.borrow().iter().position(|s| Rc::ptr_eq(s, &slot)).unwrap();
            let len = self.folded.borrow().len();
            assert!(item_index >= len - self.precomposed_count.get(), "kompoz: Item is not in pre-composed item range");
            self.reusable_count.set(self.reusable_count.get() + 1);
            self.precomposed_count.set(self.precomposed_count.get() - 1);
            self.cancel_paused_precomposition(&slot);
            let reusable_start = len - self.precomposed_count.get() - self.reusable_count.get();
            self.move_slots(item_index, reusable_start, 1);
            self.dispose_or_reuse_starting_from_index(reusable_start);
        }
        // (a slot the approach still needs, disposed of by another: the approach measures again to compose it)
        if self.slot_ids_of_compositions_needed_in_approach.borrow().iter().any(|s| s.as_ref() == Some(slot_id)) {
            self.root().request_remeasure(true);
        }
    }

    fn create_precomposed_slot_handle(&self, slot_id: SlotKey) -> PrecomposedSlotHandle {
        let attached = self.root.upgrade().is_some_and(|r| r.is_attached());
        PrecomposedSlotHandle {
            state: attached.then(|| self.this.clone()),
            slot_id,
            premeasured: RefCell::new(Vec::new()),
        }
    }

    fn precompose_paused(&self, slot_id: SlotKey, content: Rc<dyn Fn()>) -> PausedPrecomposition {
        if !self.root.upgrade().is_some_and(|r| r.is_attached()) {
            return PausedPrecomposition { state: None, slot_id };
        }
        self.precompose_slot(slot_id.clone(), content, true);
        PausedPrecomposition { state: Some(self.this.clone()), slot_id }
    }

    /// Every slot composed again at the next measure (Compose's `forceRecomposeChildren`: the layout composed again).
    fn force_recompose_children(&self) {
        let count = self.folded.borrow().len();
        if self.reusable_count.get() != count {
            for slot in self.folded.borrow().iter() {
                slot.force_recompose.set(true);
            }
            let root = self.root();
            if root.lookahead_root.borrow().is_some() {
                // (in a lookahead scope its lookahead composes the slots again)
                if !root.lookahead_measure_pending.get() {
                    root.request_lookahead_remeasure(false, true);
                }
            } else if !root.measure_pending.get() {
                root.request_remeasure(false);
            }
        }
    }

    /// A slot's paused composition (completed first with `should_complete`) applied: its layouts join the layout's
    /// children.
    fn apply_paused_precomposition(&self, slot: &Rc<Slot>, should_complete: bool) {
        let paused = slot.paused.borrow_mut().take();
        let Some(paused) = paused else { return };
        let root = self.root();
        without_read_observation(|| {
            root.ignoring_remeasure_requests(|| {
                if should_complete {
                    while !paused.is_complete() {
                        paused.resume(|| false);
                    }
                }
                paused.apply();
            })
        });
        self.sync_children();
    }
}

/// A slot not used: its layouts' use by the parent forgotten (Compose's `resetLayoutState`).
fn reset_layout_state(slot: &Slot) {
    for node in slot.nodes() {
        node.measured_by_parent.set(UsageByParent::NotUsed);
        if let Some(d) = node.lookahead_pass() {
            d.measured_by_parent.set(UsageByParent::NotUsed);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Measuring

/// A subcomposing layout's measure block: `|scope, constraints| scope.layout(…)`.
pub type SubcomposeMeasureBlock = dyn Fn(&SubcomposeMeasureScope, Constraints) -> MeasureResult;

/// What a [`SubcomposeLayout`]'s measure block measures in (Compose's `SubcomposeMeasureScope`): a measure scope
/// (it derefs to one: `scope.layout(…)`, `scope.round_to_px(…)`) that also composes slots. A clone is the same scope
/// (what a slot's content keeps of it: its density).
#[derive(Clone)]
pub struct SubcomposeMeasureScope {
    scope: MeasureScope,
    state: Rc<LayoutNodeSubcompositionsState>,
    /// The approach measure's (Compose's ApproachMeasureScopeImpl): in a lookahead scope, the main pass.
    approach: bool,
}

impl std::ops::Deref for SubcomposeMeasureScope {
    type Target = MeasureScope;
    fn deref(&self) -> &MeasureScope {
        &self.scope
    }
}

impl SubcomposeMeasureScope {
    /// Composes `content` into the slot `slot_id` now, as the layout measures (Compose's `subcompose`): composed
    /// the first time it is asked for, and again when it changes (a closure here is new content each time it is
    /// given, as a Kotlin lambda is), when what it read changed or the layout composed again. The layouts it
    /// emits, to measure. Slot ids are any `Hash + Eq` values (an enum for fixed slots, a key for list items);
    /// one asked for twice in a measure panics.
    pub fn subcompose<K: Hash + Eq + 'static>(
        &self,
        slot_id: K,
        content: impl Fn() + 'static,
    ) -> Vec<Rc<dyn Measurable>> {
        self.subcompose_key(SlotKey::new(slot_id), Rc::new(content))
    }

    fn subcompose_key(&self, slot_id: SlotKey, content: Rc<dyn Fn()>) -> Vec<Rc<dyn Measurable>> {
        if self.approach {
            self.state.subcompose_in_approach(slot_id, content)
        } else {
            self.state.subcompose(slot_id, content)
        }
    }

    /// [`subcompose`](Self::subcompose) with content kept by its instance: the same `Rc` is the same content, not
    /// composed again for itself (as Compose compares a lambda by its instance: a lazy layout's item content, kept
    /// by its key).
    pub fn subcompose_shared<K: Hash + Eq + 'static>(
        &self,
        slot_id: K,
        content: Rc<dyn Fn()>,
    ) -> Vec<Rc<dyn Measurable>> {
        self.subcompose_key(SlotKey::new(slot_id), content)
    }

    /// The measure scope it is.
    pub fn measure_scope(&self) -> &MeasureScope {
        &self.scope
    }
}

/// A subcomposing layout's policy (Compose's `NoIntrinsicsMeasurePolicy` that `createMeasurePolicy` makes).
struct SubcomposeMeasurePolicy {
    state: Weak<LayoutNodeSubcompositionsState>,
    block: Rc<SubcomposeMeasureBlock>,
}

impl MeasurePolicy for SubcomposeMeasurePolicy {
    fn measure(&self, scope: &MeasureScope, _: &[&dyn Measurable], constraints: Constraints) -> MeasureResult {
        let state = self.state.upgrade().expect("kompoz: a subcomposing layout's state is gone");
        let in_lookahead_scope = state.root().lookahead_root.borrow().is_some();
        let weak = self.state.clone();
        if !scope.is_looking_ahead() && in_lookahead_scope {
            // The approach: the slots the lookahead composed, and those only the approach needs.
            state.current_approach_index.set(0);
            let sub = SubcomposeMeasureScope { scope: scope.clone(), state: state.clone(), approach: true };
            let result = (self.block)(&sub, constraints);
            let index_after_measure = state.current_approach_index.get();
            let placed = result.clone();
            return result.with_placement(Rc::new(move |p: &PlacementScope| {
                let Some(state) = weak.upgrade() else { return };
                state.current_approach_index.set(index_after_measure);
                placed.place_children(p);
                state.dispose_unused_slots_in_approach();
                state.dispose_or_reuse_starting_from_index(state.current_index.get());
            }));
        }
        // The lookahead, or the main pass out of a lookahead scope.
        state.current_index.set(0);
        let sub = SubcomposeMeasureScope { scope: scope.clone(), state: state.clone(), approach: false };
        let result = (self.block)(&sub, constraints);
        let index_after_measure = state.current_index.get();
        let placed = result.clone();
        result.with_placement(Rc::new(move |p: &PlacementScope| {
            let Some(state) = weak.upgrade() else { return };
            state.current_index.set(index_after_measure);
            placed.place_children(p);
            // (in a lookahead scope the approach's placement disposes, once it had the chance to take slots over)
            if state.root().lookahead_root.borrow().is_none() {
                state.dispose_or_reuse_starting_from_index(state.current_index.get());
            }
        }))
    }

    fn min_intrinsic_width(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], _: i32) -> i32 {
        panic!("{NoIntrinsicsMessage}")
    }
    fn min_intrinsic_height(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], _: i32) -> i32 {
        panic!("{NoIntrinsicsMessage}")
    }
    fn max_intrinsic_width(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], _: i32) -> i32 {
        panic!("{NoIntrinsicsMessage}")
    }
    fn max_intrinsic_height(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], _: i32) -> i32 {
        panic!("{NoIntrinsicsMessage}")
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The composable

/// A layout that composes its content as it measures (Compose's `SubcomposeLayout`): `measure_policy` gets the
/// constraints and a scope whose [`subcompose`](SubcomposeMeasureScope::subcompose) composes a slot then and there
/// and gives its layouts to measure — content that depends on the constraints (`BoxWithConstraints`), on another
/// slot's size, or composed only as far as it shows (lazy lists). `state` (Compose's overload taking one: none, a
/// remembered state keeping no slot for reuse) sets the reuse policy and precomposes. Its slots see the
/// composition locals provided where it is composed. Asking it for an intrinsic size panics, as Compose's throws.
#[kompozable]
pub fn SubcomposeLayout(
    #[default(None)] state: Option<SubcomposeLayoutState>,
    #[default(Modifier)] modifier: Modifier,
    measure_policy: impl Fn(&SubcomposeMeasureScope, Constraints) -> MeasureResult + 'static,
) {
    let state = match state {
        Some(state) => state,
        None => remember(SubcomposeLayoutState::default),
    };
    let composite_key = current_composite_key_hash();
    let context = remember_kompozition_context();
    let density = LocalDensity.current();
    let layout_direction = LocalLayoutDirection.current();
    let locals = current_kompozition_local_map();
    let block: Rc<SubcomposeMeasureBlock> = Rc::new(measure_policy);
    let s = state.clone();
    emit(
        LayoutNode::new,
        move |node: &LayoutNode| {
            let subcompositions = s.set_root(node);
            *subcompositions.composition_context.borrow_mut() = Some(context);
            node.set_measure_policy(subcompositions.create_measure_policy(block));
            node.set_density(density);
            node.set_layout_direction(layout_direction);
            node.set_locals(locals);
            node.modifier_set(modifier);
            node.set_composite_key(composite_key);
            node.note_composed();
        },
        || {},
    );
    // (it composed again, not skipped: its slots composed again at the next measure)
    SideEffect(move || state.force_recompose_children());
}

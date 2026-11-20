//! A modifier node's cell (the node and what the chain knows of it), the context a node is given and the handle it
//! keeps, node kinds, delegation, traversal and observed reads.

use super::ModifierNode;
use crate::layout::LayoutCoordinates;
use crate::node::{LayoutNode, NodeCoordinator, ReaderScope};
use kompoz_coroutines::CoroutineScope;
use kompoz_graphics::{Density, LayoutDirection};
use kompoz_runtime::KompozitionLocal;
use std::any::Any;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::rc::{Rc, Weak};

/// What a node is: a set of [`NodeKind`] bits, worked out from its capabilities (Compose's NodeKind/Nodes).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeKind(pub u32);

impl NodeKind {
    pub const Any: NodeKind = NodeKind(1);
    pub const Layout: NodeKind = NodeKind(1 << 1);
    pub const Draw: NodeKind = NodeKind(1 << 2);
    pub const Semantics: NodeKind = NodeKind(1 << 3);
    pub const PointerInput: NodeKind = NodeKind(1 << 4);
    pub const Locals: NodeKind = NodeKind(1 << 5);
    pub const ParentData: NodeKind = NodeKind(1 << 6);
    pub const OnRemeasured: NodeKind = NodeKind(1 << 7);
    pub const GlobalPositionAware: NodeKind = NodeKind(1 << 8);
    pub const ApproachMeasure: NodeKind = NodeKind(1 << 9);
    pub const FocusTarget: NodeKind = NodeKind(1 << 10);
    pub const FocusProperties: NodeKind = NodeKind(1 << 11);
    pub const FocusEvent: NodeKind = NodeKind(1 << 12);
    pub const KeyInput: NodeKind = NodeKind(1 << 13);
    pub const RotaryInput: NodeKind = NodeKind(1 << 14);
    pub const CompositionLocalConsumer: NodeKind = NodeKind(1 << 15);
    pub const SoftKeyboardKeyInput: NodeKind = NodeKind(1 << 16);
    pub const Traversable: NodeKind = NodeKind(1 << 17);
    pub const BringIntoView: NodeKind = NodeKind(1 << 18);
    pub const Unplaced: NodeKind = NodeKind(1 << 19);
    pub const IndirectPointerInput: NodeKind = NodeKind(1 << 20);
    pub const OnPlaced: NodeKind = NodeKind(1 << 21);
    pub const BeyondBoundsLayout: NodeKind = NodeKind(1 << 22);
    /// KompozR's: a node whose observed reads tell it when they change ([`ObserverModifierNode`]).
    pub const Observer: NodeKind = NodeKind(1 << 23);

    pub const fn mask(self) -> u32 {
        self.0
    }
    pub const fn or(self, other: NodeKind) -> NodeKind {
        NodeKind(self.0 | other.0)
    }
    /// Whether `set` has any of these bits.
    pub const fn is_in(self, set: u32) -> bool {
        set & self.0 != 0
    }
    /// Whether a coordinator visits its own layout modifier node for these kinds (Compose's
    /// `NodeKind.includeSelfInTraversal`): only for `OnRemeasured` and `OnPlaced`. A layout modifier node's other
    /// kinds (its drawing, its pointer input, …) are visited by the coordinator it wraps, in that one's space.
    pub const fn include_self_in_traversal(self) -> bool {
        self.0 & (NodeKind::OnRemeasured.0 | NodeKind::OnPlaced.0) != 0
    }
}

/// The kinds of a node, from the capabilities it has (Compose's `calculateNodeKindSetFrom`).
pub(crate) fn kinds_of(node: &mut dyn ModifierNode) -> u32 {
    fn bit(has: bool, kind: NodeKind) -> u32 {
        if has { kind.0 } else { 0 }
    }
    let mut k = NodeKind::Any.0;
    k |= bit(node.as_layout().is_some(), NodeKind::Layout);
    k |= bit(node.as_approach_layout().is_some(), NodeKind::ApproachMeasure);
    k |= bit(node.as_draw().is_some(), NodeKind::Draw);
    k |= bit(node.as_semantics().is_some(), NodeKind::Semantics);
    k |= bit(node.as_pointer_input().is_some(), NodeKind::PointerInput);
    k |= bit(node.as_parent_data().is_some(), NodeKind::ParentData);
    if let Some(aware) = node.as_layout_aware() {
        // A node that only cares about its placement isn't remeasured for it (Compose's OnPlacedNode).
        if aware.observes_remeasure() {
            k |= NodeKind::OnRemeasured.0;
        }
        k |= NodeKind::OnPlaced.0;
    }
    k |= bit(node.as_global_position_aware().is_some(), NodeKind::GlobalPositionAware);
    k |= bit(node.as_unplaced_aware().is_some(), NodeKind::Unplaced);
    k |= bit(node.as_focus_target().is_some(), NodeKind::FocusTarget);
    k |= bit(node.as_focus_properties().is_some(), NodeKind::FocusProperties);
    k |= bit(node.as_focus_event().is_some(), NodeKind::FocusEvent);
    k |= bit(node.as_key_input().is_some(), NodeKind::KeyInput);
    k |= bit(node.as_rotary_input().is_some(), NodeKind::RotaryInput);
    k |= bit(node.as_soft_keyboard_interception().is_some(), NodeKind::SoftKeyboardKeyInput);
    k |= bit(node.as_traversable().is_some(), NodeKind::Traversable);
    k |= bit(node.as_modifier_local().is_some(), NodeKind::Locals);
    k |= bit(node.as_bring_into_view().is_some(), NodeKind::BringIntoView);
    k |= bit(node.as_beyond_bounds_layout().is_some(), NodeKind::BeyondBoundsLayout);
    k |= bit(node.as_indirect_pointer_input().is_some(), NodeKind::IndirectPointerInput);
    k |= bit(node.as_observer().is_some(), NodeKind::Observer);
    k
}

/// What the chain knows of a node: its kinds, where it is, whether it is attached; its delegates; its coroutine
/// scope and observed reads. All of it apart from the node's own `RefCell`, so it can be read while the node runs.
pub(crate) struct NodeMeta {
    /// Its kinds and its delegates' (Compose's kindSet).
    pub(crate) kind_set: Cell<u32>,
    /// Its own kinds only (Compose's DelegatingNode.selfKindSet).
    pub(crate) self_kind_set: Cell<u32>,
    /// Its kinds and those of the nodes after it in the chain (Compose's aggregateChildKindSet).
    pub(crate) aggregate_child_kind_set: Cell<u32>,
    pub(crate) attached: Cell<bool>,
    attach_run_expected: Cell<bool>,
    detach_run_expected: Cell<bool>,
    pub(crate) coordinator: RefCell<Weak<NodeCoordinator>>,
    pub(crate) layout_node: RefCell<Weak<LayoutNode>>,
    scope: RefCell<Option<CoroutineScope>>,
    /// Its delegates, in the order it delegated to them.
    pub(crate) delegates: RefCell<Vec<NodeRc>>,
    /// The node it is a delegate of.
    pub(crate) delegator: RefCell<Option<Weak<NodeCell<dyn ModifierNode>>>>,
    reads: RefCell<Option<Rc<ReaderScope>>>,
    /// Made while its layout was detached: its insertion invalidations wait for the attach.
    pub(crate) inserted_awaiting_attach: Cell<bool>,
    /// Told when it is detached, before its `on_detach` (Compose's `detachedListener`: a hit path drops it).
    pub(crate) detached_listener: RefCell<Option<Rc<dyn Fn()>>>,
    /// The modifier locals it provides as last read (made, updated, attached, or read since): what a node below
    /// reads of it while it is busy in one of its own callbacks (measuring what a subcomposition attaches below it).
    pub(crate) provided_locals: RefCell<Option<super::ModifierLocalMap>>,
}

/// A node and what the chain knows of it (opaque outside the crate).
#[doc(hidden)]
pub struct NodeCell<N: ?Sized> {
    pub(crate) meta: NodeMeta,
    pub(crate) node: RefCell<N>,
}

#[doc(hidden)]
pub type NodeRc = Rc<NodeCell<dyn ModifierNode>>;

impl<N: ModifierNode> NodeCell<N> {
    /// A cell for a new node, its kinds worked out.
    pub(crate) fn new_typed(mut node: N) -> Rc<NodeCell<N>> {
        let kinds = kinds_of(&mut node);
        let cell = Rc::new(NodeCell {
            meta: NodeMeta {
                kind_set: Cell::new(kinds),
                self_kind_set: Cell::new(kinds),
                aggregate_child_kind_set: Cell::new(0),
                attached: Cell::new(false),
                attach_run_expected: Cell::new(false),
                detach_run_expected: Cell::new(false),
                coordinator: RefCell::new(Weak::new()),
                layout_node: RefCell::new(Weak::new()),
                scope: RefCell::new(None),
                delegates: RefCell::new(Vec::new()),
                delegator: RefCell::new(None),
                reads: RefCell::new(None),
                inserted_awaiting_attach: Cell::new(true),
                detached_listener: RefCell::new(None),
                provided_locals: RefCell::new(None),
            },
            node: RefCell::new(node),
        });
        cell.refresh_provided_locals();
        cell
    }

    pub(crate) fn new_rc(node: N) -> NodeRc {
        NodeCell::new_typed(node)
    }
}

impl<N: ModifierNode + ?Sized> NodeCell<N> {
    /// Keeps a copy of the modifier locals it provides now, if it isn't busy (see `NodeMeta::provided_locals`).
    pub(crate) fn refresh_provided_locals(&self) {
        if let Ok(mut node) = self.node.try_borrow_mut() {
            let map = node.as_modifier_local().map(|n| n.provided_values().clone());
            *self.meta.provided_locals.borrow_mut() = map;
        }
    }
}

impl NodeCell<dyn ModifierNode> {
    pub(crate) fn is_kind(&self, kind: NodeKind) -> bool {
        kind.is_in(self.meta.kind_set.get())
    }

    pub(crate) fn coordinator(&self) -> Option<Rc<NodeCoordinator>> {
        self.meta.coordinator.borrow().upgrade()
    }

    pub(crate) fn layout_node(&self) -> Option<Rc<LayoutNode>> {
        self.meta.layout_node.borrow().upgrade()
    }

    /// Sets where it is (and its delegates'), as Compose's `updateCoordinator`.
    pub(crate) fn set_place(&self, coordinator: Option<&Rc<NodeCoordinator>>, layout_node: &Weak<LayoutNode>) {
        *self.meta.coordinator.borrow_mut() = coordinator.map_or_else(Weak::new, Rc::downgrade);
        *self.meta.layout_node.borrow_mut() = layout_node.clone();
        for d in self.meta.delegates.borrow().iter() {
            d.set_place(coordinator, layout_node);
        }
    }

    /// The delegates, newest first (Compose's `forEachImmediateDelegate` order), copied out.
    pub(crate) fn immediate_delegates(&self) -> Vec<NodeRc> {
        self.meta.delegates.borrow().iter().rev().cloned().collect()
    }

    /// Compose's `markAsAttached` (its delegates' too).
    pub(crate) fn mark_as_attached(&self) {
        assert!(!self.meta.attached.get(), "kompoz: a node attached twice");
        self.meta.attached.set(true);
        self.meta.attach_run_expected.set(true);
        for d in self.immediate_delegates() {
            if !d.meta.attached.get() {
                d.mark_as_attached();
            }
        }
    }

    /// Compose's `runAttachLifecycle`: the delegates' `on_attach`, newest first, then its own.
    pub(crate) fn run_attach_lifecycle(self: &Rc<Self>) {
        for d in self.immediate_delegates() {
            d.run_attach_lifecycle();
        }
        assert!(self.meta.attach_run_expected.get(), "kompoz: on_attach run twice");
        self.meta.attach_run_expected.set(false);
        let cx = NodeCx::new(self);
        self.node.borrow_mut().on_attach(&cx);
        self.refresh_provided_locals();
        self.meta.detach_run_expected.set(true);
    }

    /// Compose's `runDetachLifecycle`: its own `on_detach`, then its delegates'.
    pub(crate) fn run_detach_lifecycle(self: &Rc<Self>) {
        assert!(self.meta.detach_run_expected.get(), "kompoz: on_detach without on_attach");
        self.meta.detach_run_expected.set(false);
        let listener = self.meta.detached_listener.borrow_mut().take();
        if let Some(l) = listener {
            l();
        }
        let cx = NodeCx::new(self);
        self.node.borrow_mut().on_detach(&cx);
        for d in self.immediate_delegates() {
            d.run_detach_lifecycle();
        }
    }

    /// Compose's `markAsDetached`: its delegates', then itself; its coroutine scope cancelled and its observed reads
    /// forgotten.
    pub(crate) fn mark_as_detached(&self) {
        for d in self.immediate_delegates() {
            d.mark_as_detached();
        }
        assert!(self.meta.attached.get(), "kompoz: a node detached twice");
        self.meta.attached.set(false);
        let scope = self.meta.scope.borrow_mut().take();
        if let Some(scope) = scope {
            scope.cancel();
        }
        let reads = self.meta.reads.borrow_mut().take();
        drop(reads);
    }

    /// Compose's `reset` (its delegates' too).
    pub(crate) fn reset(&self) {
        self.node.borrow_mut().on_reset();
        for d in self.immediate_delegates() {
            d.reset();
        }
    }
}

/// Calls `f` on the nodes of `kind` among `cell` and its delegates, as Compose's `dispatchForKind`: the node itself
/// if it is of that kind, else those of its delegates that are (each the same way).
pub(crate) fn dispatch_for_kind(cell: &NodeRc, kind: NodeKind, f: &mut dyn FnMut(&NodeRc)) {
    if kind.is_in(cell.meta.self_kind_set.get()) {
        f(cell);
        return;
    }
    if !cell.is_kind(kind) {
        return;
    }
    for d in cell.immediate_delegates() {
        if d.is_kind(kind) {
            dispatch_for_kind(&d, kind, f);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The context and the handle

/// What a node is given in its callbacks (Compose's DelegatableNode receiver): invalidate what depends on it,
/// read its layout's density, direction and composition locals, its coordinates, a coroutine scope, observed reads,
/// delegation. Keep [`handle`](NodeCx::handle) to do the same later.
#[derive(Clone)]
pub struct NodeCx {
    pub(crate) cell: NodeRc,
}

impl NodeCx {
    pub(crate) fn new(cell: &NodeRc) -> NodeCx {
        NodeCx { cell: cell.clone() }
    }

    /// What to keep to act later: weak, so it doesn't keep the node.
    pub fn handle(&self) -> NodeHandle {
        NodeHandle { cell: Rc::downgrade(&self.cell) }
    }

    /// Whether the node is attached (in a layout that is in a host).
    pub fn is_attached(&self) -> bool {
        self.cell.meta.attached.get()
    }

    /// Whether `other` is the context of the same node.
    pub fn is_same_node(&self, other: &NodeCx) -> bool {
        Rc::ptr_eq(&self.cell, &other.cell)
    }

    /// The layout node it is in.
    pub fn layout_node(&self) -> Option<Rc<LayoutNode>> {
        self.cell.layout_node()
    }

    /// Its layout measured again (Compose's `invalidateMeasurement`).
    pub fn invalidate_measurement(&self) {
        if let Some(n) = self.cell.layout_node() {
            n.invalidate_measurements();
        }
    }

    /// Its layout measured and placed again now (Compose's `remeasureSync`, a `Remeasurement`'s
    /// `forceRemeasure`).
    pub fn remeasure_sync(&self) {
        if let Some(n) = self.cell.layout_node() {
            n.force_remeasure();
        }
    }

    /// Its layout placed again, its placement blocks run again without measuring (Compose's
    /// `invalidatePlacement`).
    pub fn invalidate_placement(&self) {
        if let Some(n) = self.cell.layout_node() {
            n.request_relayout();
        }
    }

    /// Its layout drawn again (Compose's `invalidateDraw`).
    pub fn invalidate_draw(&self) {
        if let Some(n) = self.cell.layout_node() {
            n.invalidate_draw();
        }
    }

    /// Its layout's semantics worked out again (Compose's `invalidateSemantics`).
    pub fn invalidate_semantics(&self) {
        if let Some(n) = self.cell.layout_node() {
            n.invalidate_semantics();
        }
    }

    /// Its layout's parent data worked out again (Compose's `invalidateParentData`).
    pub fn invalidate_parent_data(&self) {
        if let Some(n) = self.cell.layout_node() {
            n.invalidate_parent_data();
        }
    }

    pub fn density(&self) -> Density {
        self.cell.layout_node().map_or_else(Density::default, |n| n.density())
    }

    pub fn layout_direction(&self) -> LayoutDirection {
        self.cell.layout_node().map_or(LayoutDirection::Ltr, |n| n.layout_direction())
    }

    /// Its layout's density, the node being in a layout (Compose's `DelegatableNode.requireDensity()`): panics
    /// otherwise.
    #[track_caller]
    pub fn require_density(&self) -> Density {
        self.require_layout_node().density()
    }

    /// Its layout's direction, the node being in a layout (Compose's `DelegatableNode.requireLayoutDirection()`):
    /// panics otherwise.
    #[track_caller]
    pub fn require_layout_direction(&self) -> LayoutDirection {
        self.require_layout_node().layout_direction()
    }

    /// The layout node it is in (Compose's `requireLayoutNode()`): panics if it is in none.
    #[track_caller]
    pub fn require_layout_node(&self) -> Rc<LayoutNode> {
        self.cell.layout_node().expect("Cannot obtain node coordinator. Is the Modifier.Node attached?")
    }

    /// The coordinates of the layout it belongs to (its coordinator: the layout node below it in the chain, or
    /// the layout's own).
    pub fn coordinates(&self) -> Option<LayoutCoordinates> {
        self.cell.coordinator().map(LayoutCoordinates::of)
    }

    /// `local`'s value where its layout was composed (Compose's CompositionLocalConsumerModifierNode.currentValueOf),
    /// subscribing no composable: a dynamic local's as it is now, read as a state is (in a measure, a placement, a
    /// drawing or `observe_reads`, that runs again when its provider gives another value); a static local's as
    /// it was when the layout was last composed (the layout composes again, and its nodes are invalidated, when
    /// it changes).
    pub fn current_value_of<T: Clone + 'static>(&self, local: &'static KompozitionLocal<T>) -> T {
        match self.cell.layout_node() {
            Some(n) => n.current_value_of(local),
            None => local.default_value(),
        }
    }

    /// The dispatcher of the host it is in, while it is in one: its clock (`now()`, the frame's time or a newer
    /// event's) is what Android's `AnimationUtils.currentAnimationTimeMillis` is to a view.
    pub fn dispatcher(&self) -> Option<kompoz_coroutines::Dispatcher> {
        self.cell.layout_node().and_then(|n| n.owner()).map(|o| o.dispatcher())
    }

    /// A scope for its coroutines: made when first asked for, cancelled when the node is detached (Compose's
    /// `Modifier.Node.coroutineScope`), running them in its host's effect context (Compose's
    /// `Owner.coroutineContext`). Panics on a node that isn't attached.
    #[track_caller]
    pub fn coroutine_scope(&self) -> CoroutineScope {
        if let Some(s) = self.cell.meta.scope.borrow().as_ref() {
            return s.clone();
        }
        assert!(self.is_attached(), "kompoz: coroutine_scope asked of a node that isn't attached");
        let owner = self.cell.layout_node().and_then(|n| n.owner()).expect("kompoz: a node outside of a host");
        let scope = CoroutineScope::new_with_context(&owner.dispatcher(), owner.effect_coroutine_context());
        *self.cell.meta.scope.borrow_mut() = Some(scope.clone());
        scope
    }

    /// Runs `f` with what it reads observed: when one of those states changes, the node's
    /// [`ObserverModifierNode::on_observed_reads_changed`] is called (Compose's `observeReads`). What an earlier
    /// call read is forgotten.
    pub fn observe_reads<R>(&self, f: impl FnOnce() -> R) -> R {
        let Some(owner) = self.cell.layout_node().and_then(|n| n.owner()) else { return f() };
        let scope = {
            let mut reads = self.cell.meta.reads.borrow_mut();
            reads
                .get_or_insert_with(|| {
                    let weak = Rc::downgrade(&self.cell);
                    owner.snapshot_observer().scope(move || {
                        if let Some(cell) = weak.upgrade()
                            && cell.meta.attached.get()
                        {
                            let mut node = cell.node.borrow_mut();
                            if let Some(o) = node.as_observer() {
                                o.on_observed_reads_changed();
                            }
                        }
                    })
                })
                .clone()
        };
        owner.snapshot_observer().observe(&scope, f)
    }

    /// Tells the platform its layout scrolled by `delta`, what a scrollable consumed (Compose's
    /// `DelegatableNode.dispatchOnScrollChanged`, which calls the owner's): Android's owner tells its view tree's
    /// scroll observers (stage 15's); a desktop owner, as Compose's default `Owner.dispatchOnScrollChanged`, has none
    /// to tell.
    pub fn dispatch_on_scroll_changed(&self, delta: kompoz_graphics::Offset) {
        let _ = delta;
    }

    /// Asks the platform's autofill to fill its layout (Compose's `DelegatableNode.requestAutofill`): the autofill
    /// manager tells the platform the layout's semantics id and bounds.
    pub fn request_autofill(&self) {
        let Some(node) = self.cell.layout_node() else { return };
        if let Some(autofill) = node.owner().and_then(|o| o.autofill()) {
            autofill.request_autofill(&node);
        }
    }

    /// Runs `effect` once the composition's changes are applied (Compose's `Modifier.Node.sideEffect`).
    pub fn side_effect(&self, effect: impl FnOnce() + 'static) {
        match self.cell.layout_node().and_then(|n| n.owner()) {
            Some(owner) => owner.register_on_end_apply_changes(Box::new(effect)),
            None => effect(),
        }
    }

    /// Makes `node` a delegate of this node (Compose's `DelegatingNode.delegate`): its kinds are this node's too,
    /// it gets the lifecycle with it (attached now if this node is), and traversals find it where this node is.
    pub fn delegate<T: ModifierNode>(&self, node: T) -> DelegatedNode<T> {
        let typed = NodeCell::new_typed(node);
        let cell: NodeRc = typed.clone();
        self.delegate_cell(cell);
        DelegatedNode { cell: typed }
    }

    /// Makes a node made apart from any chain a delegate of this one (Compose's `delegate` of a node another
    /// object hands out, as an OverscrollEffect's `node`): nothing if it is this node's delegate already. Panics
    /// if it is another node's delegate, or attached.
    #[track_caller]
    pub fn delegate_node(&self, node: &DelegatableNode) {
        let delegator = node.cell.meta.delegator.borrow().as_ref().and_then(Weak::upgrade);
        if let Some(d) = delegator {
            if Rc::ptr_eq(&d, &self.cell) {
                return;
            }
            panic!("Cannot delegate to an already delegated node");
        }
        assert!(!node.cell.meta.attached.get(), "Cannot delegate to an already attached node");
        self.delegate_cell(node.cell.clone());
    }

    fn delegate_cell(&self, cell: NodeRc) {
        *cell.meta.delegator.borrow_mut() = Some(Rc::downgrade(&self.cell));
        let before = self.cell.meta.kind_set.get();
        let delegated = cell.meta.kind_set.get();
        assert!(
            !(NodeKind::Layout.is_in(delegated) && NodeKind::Layout.is_in(before))
                || NodeKind::Layout.is_in(self.cell.meta.self_kind_set.get()),
            "kompoz: delegating to several layout nodes without the delegating node being a layout node itself"
        );
        self.cell.meta.delegates.borrow_mut().push(cell.clone());
        self.update_kinds();
        let layout_node = self.cell.meta.layout_node.borrow().clone();
        cell.set_place(self.cell.coordinator().as_ref(), &layout_node);
        if self.cell.meta.attached.get() {
            if NodeKind::Layout.is_in(delegated)
                && !NodeKind::Layout.is_in(before)
                && let Some(n) = layout_node.upgrade()
            {
                n.sync_coordinators();
            }
            cell.mark_as_attached();
            cell.run_attach_lifecycle();
            crate::modifier::chain::auto_invalidate(&cell, crate::modifier::chain::Phase::Inserted);
        }
    }

    /// Takes a delegate back (Compose's `undelegate`): detached if it was attached, its kinds no longer this
    /// node's.
    pub fn undelegate<T: ModifierNode>(&self, delegate: &DelegatedNode<T>) {
        let target: NodeRc = delegate.cell.clone();
        self.undelegate_cell(&target);
    }

    /// Takes back a node [`delegate_node`](NodeCx::delegate_node) delegated to: it can be delegated to again (by
    /// this node or another).
    pub fn undelegate_node(&self, node: &DelegatableNode) {
        self.undelegate_cell(&node.cell);
    }

    fn undelegate_cell(&self, target: &NodeRc) {
        let found = {
            let mut delegates = self.cell.meta.delegates.borrow_mut();
            let at = delegates.iter().position(|d| Rc::ptr_eq(d, target));
            at.map(|i| delegates.remove(i))
        };
        let Some(cell) = found else { panic!("kompoz: undelegate of a node that isn't a delegate of this one") };
        if cell.meta.attached.get() {
            crate::modifier::chain::auto_invalidate(&cell, crate::modifier::chain::Phase::Removed);
            cell.run_detach_lifecycle();
            cell.mark_as_detached();
        }
        *cell.meta.delegator.borrow_mut() = None;
        let before = self.cell.meta.kind_set.get();
        self.update_kinds();
        if self.cell.meta.attached.get()
            && NodeKind::Layout.is_in(before)
            && !NodeKind::Layout.is_in(self.cell.meta.kind_set.get())
            && let Some(n) = self.cell.layout_node()
        {
            n.sync_coordinators();
        }
    }

    /// Its kinds worked out again from its delegates', up to the chain's node, and the chain's aggregates.
    fn update_kinds(&self) {
        let mut cell = self.cell.clone();
        loop {
            let mut kinds = cell.meta.self_kind_set.get();
            for d in cell.meta.delegates.borrow().iter() {
                kinds |= d.meta.kind_set.get();
            }
            cell.meta.kind_set.set(kinds);
            let up = cell.meta.delegator.borrow().as_ref().and_then(Weak::upgrade);
            match up {
                Some(d) => cell = d,
                None => break,
            }
        }
        if let Some(n) = self.cell.layout_node() {
            n.sync_aggregate_kinds();
        }
    }

    /// Visits the nodes of `kind` above this one: earlier in its chain, then in its ancestors' chains, nearest
    /// first, until `visit` returns false (Compose's `visitAncestors`). With `include_self`, starting with it.
    pub fn visit_ancestors(&self, kind: NodeKind, include_self: bool, visit: &mut dyn FnMut(&NodeCx) -> bool) {
        let Some(layout_node) = self.cell.layout_node() else { return };
        let me = self.chain_node();
        let mut node: Option<Rc<LayoutNode>> = Some(layout_node);
        let mut first = true;
        while let Some(n) = node {
            let nodes = n.chain_nodes();
            let end = if first {
                match nodes.iter().position(|c| Rc::ptr_eq(c, &me)) {
                    Some(i) if include_self => i + 1,
                    Some(i) => i,
                    None => 0,
                }
            } else {
                nodes.len()
            };
            first = false;
            for cell in nodes[..end].iter().rev() {
                let mut go_on = true;
                dispatch_for_kind(cell, kind, &mut |c| {
                    if go_on {
                        go_on = visit(&NodeCx::new(c));
                    }
                });
                if !go_on {
                    return;
                }
            }
            node = n.parent();
        }
    }

    /// Visits the nodes of `kind` below this one: later in its chain, then in its descendants' chains (each child's
    /// chain from its head, depth first, in its children's order), until `visit` returns false (Compose's
    /// `visitSubtree`). `visit_children` instead stops going down at the first node of the kind on each path.
    pub fn visit_subtree(&self, kind: NodeKind, visit: &mut dyn FnMut(&NodeCx) -> bool) {
        self.visit_below(kind, false, visit);
    }

    /// The nearest nodes of `kind` below this one on each path (Compose's `visitChildren`), until `visit` returns
    /// false.
    pub fn visit_children(&self, kind: NodeKind, visit: &mut dyn FnMut(&NodeCx) -> bool) {
        self.visit_below(kind, true, visit);
    }

    fn visit_below(&self, kind: NodeKind, nearest_only: bool, visit: &mut dyn FnMut(&NodeCx) -> bool) {
        let Some(layout_node) = self.cell.layout_node() else { return };
        let me = self.chain_node();
        let nodes = layout_node.chain_nodes();
        let start = nodes.iter().position(|c| Rc::ptr_eq(c, &me)).map_or(0, |i| i + 1);
        let mut stop = false;
        // Nodes of this chain after it; a hit there ends the walk down for `nearest_only`.
        for cell in &nodes[start..] {
            let mut hit = false;
            dispatch_for_kind(cell, kind, &mut |c| {
                if !stop {
                    hit = true;
                    stop = !visit(&NodeCx::new(c));
                }
            });
            if stop || (hit && nearest_only) {
                return;
            }
        }
        visit_layout_children(&layout_node, kind, nearest_only, visit, &mut stop);
    }

    /// The node of the chain it is (or is a delegate of).
    fn chain_node(&self) -> NodeRc {
        let mut cell = self.cell.clone();
        loop {
            let up = cell.meta.delegator.borrow().as_ref().and_then(Weak::upgrade);
            match up {
                Some(d) => cell = d,
                None => return cell,
            }
        }
    }

    /// The node itself, borrowed as a `T` (if it is one).
    pub fn with<T: ModifierNode, R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        let mut node = self.cell.node.borrow_mut();
        let node: &mut dyn Any = &mut *node;
        node.downcast_mut::<T>().map(f)
    }

    /// Whether the node is a `T`.
    pub fn is<T: ModifierNode>(&self) -> bool {
        let node = self.cell.node.borrow();
        let node: &dyn Any = &*node;
        node.is::<T>()
    }

    /// Visits the [`TraversableNode`]s with `key` above this one, nearest first, until `visit` says to stop
    /// (Compose's `traverseAncestors`).
    pub fn traverse_ancestors(&self, key: &dyn Any, visit: &mut dyn FnMut(&NodeCx) -> bool) {
        self.visit_ancestors(NodeKind::Traversable, false, &mut |cx| {
            if cx.traverse_key_is(key) { visit(cx) } else { true }
        });
    }

    /// The nearest [`TraversableNode`] with `key` above this one (Compose's `findNearestAncestor`).
    pub fn find_nearest_ancestor(&self, key: &dyn Any) -> Option<NodeCx> {
        let mut found = None;
        self.traverse_ancestors(key, &mut |cx| {
            found = Some(cx.clone());
            false
        });
        found
    }

    /// The nearest [`TraversableNode`]s with `key` below this one (Compose's `traverseChildren`).
    pub fn traverse_children(&self, key: &dyn Any, visit: &mut dyn FnMut(&NodeCx) -> bool) {
        // Compose's: the nearest traversable nodes, those of other keys looked through.
        let mut stop = false;
        self.visit_children(NodeKind::Traversable, &mut |cx| {
            if cx.traverse_key_is(key) {
                stop = !visit(cx);
            } else {
                cx.traverse_children(key, &mut |c| {
                    stop = !visit(c);
                    !stop
                });
            }
            !stop
        });
    }

    /// The [`TraversableNode`]s with `key` below this one, depth first, each telling whether to go on, skip what is
    /// below it, or stop (Compose's `traverseDescendants`).
    pub fn traverse_descendants(&self, key: &dyn Any, visit: &mut dyn FnMut(&NodeCx) -> TraversalDecision) {
        let mut stop = false;
        self.visit_children(NodeKind::Traversable, &mut |cx| {
            let decision = if cx.traverse_key_is(key) { visit(cx) } else { TraversalDecision::ContinueTraversal };
            match decision {
                TraversalDecision::CancelTraversal => stop = true,
                TraversalDecision::SkipSubtreeAndContinueTraversal => {}
                TraversalDecision::ContinueTraversal => cx.traverse_descendants(key, &mut |c| {
                    let d = visit(c);
                    if d == TraversalDecision::CancelTraversal {
                        stop = true;
                    }
                    d
                }),
            }
            !stop
        });
    }

    fn traverse_key_is(&self, key: &dyn Any) -> bool {
        let mut node = self.cell.node.borrow_mut();
        node.as_traversable().is_some_and(|t| t.traverse_key_eq(key))
    }
}

/// The [`TraversableNode`]s with `key` in `nodes` and below them, depth first, as
/// [`NodeCx::traverse_descendants`] visits those below a node (what a subcomposing layout's precomposed slot
/// traverses from its virtual node: Compose's `PrecomposedSlotHandle.traverseDescendants`).
pub(crate) fn traverse_descendants_of(
    nodes: Vec<Rc<LayoutNode>>,
    key: &dyn Any,
    visit: &mut dyn FnMut(&NodeCx) -> TraversalDecision,
) {
    let mut stop = false;
    visit_layout_nodes(
        nodes,
        NodeKind::Traversable,
        true,
        &mut |cx| {
            let decision = if cx.traverse_key_is(key) { visit(cx) } else { TraversalDecision::ContinueTraversal };
            let mut cancelled = decision == TraversalDecision::CancelTraversal;
            if decision == TraversalDecision::ContinueTraversal {
                cx.traverse_descendants(key, &mut |c| {
                    let d = visit(c);
                    if d == TraversalDecision::CancelTraversal {
                        cancelled = true;
                    }
                    d
                });
            }
            !cancelled
        },
        &mut stop,
    );
}

/// Walks `node`'s children's chains for [`NodeCx::visit_below`].
fn visit_layout_children(
    node: &Rc<LayoutNode>,
    kind: NodeKind,
    nearest_only: bool,
    visit: &mut dyn FnMut(&NodeCx) -> bool,
    stop: &mut bool,
) {
    visit_layout_nodes(node.children(), kind, nearest_only, visit, stop);
}

/// The same over `nodes`.
fn visit_layout_nodes(
    nodes: Vec<Rc<LayoutNode>>,
    kind: NodeKind,
    nearest_only: bool,
    visit: &mut dyn FnMut(&NodeCx) -> bool,
    stop: &mut bool,
) {
    for child in nodes {
        if *stop {
            return;
        }
        if child.chain_aggregate_kinds() & kind.0 == 0 {
            // (nothing of the kind in its chain: on to its children)
            visit_layout_children(&child, kind, nearest_only, visit, stop);
            continue;
        }
        let mut hit = false;
        for cell in child.chain_nodes() {
            dispatch_for_kind(&cell, kind, &mut |c| {
                if !*stop {
                    hit = true;
                    *stop = !visit(&NodeCx::new(c));
                }
            });
            if *stop || (hit && nearest_only) {
                break;
            }
        }
        if !*stop && !(hit && nearest_only) {
            visit_layout_children(&child, kind, nearest_only, visit, stop);
        }
    }
}

/// What a node keeps to act later (from a coroutine, an animation, a callback): the same as [`NodeCx`], weak — it
/// does nothing once the node is gone.
#[derive(Clone)]
pub struct NodeHandle {
    pub(crate) cell: Weak<NodeCell<dyn ModifierNode>>,
}

impl NodeHandle {
    /// The context, while the node is alive.
    pub fn cx(&self) -> Option<NodeCx> {
        self.cell.upgrade().map(|cell| NodeCx { cell })
    }
    pub fn is_attached(&self) -> bool {
        self.cx().is_some_and(|c| c.is_attached())
    }
    pub fn invalidate_measurement(&self) {
        if let Some(c) = self.cx() {
            c.invalidate_measurement();
        }
    }
    pub fn invalidate_placement(&self) {
        if let Some(c) = self.cx() {
            c.invalidate_placement();
        }
    }
    pub fn invalidate_draw(&self) {
        if let Some(c) = self.cx() {
            c.invalidate_draw();
        }
    }
    pub fn invalidate_semantics(&self) {
        if let Some(c) = self.cx() {
            c.invalidate_semantics();
        }
    }
    pub fn layout_node(&self) -> Option<Rc<LayoutNode>> {
        self.cx().and_then(|c| c.layout_node())
    }
    pub fn remeasure_sync(&self) {
        if let Some(c) = self.cx() {
            c.remeasure_sync();
        }
    }
    pub fn density(&self) -> Density {
        self.cx().map_or_else(Density::default, |c| c.density())
    }
    pub fn layout_direction(&self) -> LayoutDirection {
        self.cx().map_or(LayoutDirection::Ltr, |c| c.layout_direction())
    }
    /// [`NodeCx::require_density`]: panics once the node is gone or outside a layout.
    #[track_caller]
    pub fn require_density(&self) -> Density {
        self.cx().expect("Cannot obtain node coordinator. Is the Modifier.Node attached?").require_density()
    }
    /// [`NodeCx::require_layout_direction`]: panics once the node is gone or outside a layout.
    #[track_caller]
    pub fn require_layout_direction(&self) -> LayoutDirection {
        self.cx().expect("Cannot obtain node coordinator. Is the Modifier.Node attached?").require_layout_direction()
    }
    pub fn coordinates(&self) -> Option<LayoutCoordinates> {
        self.cx().and_then(|c| c.coordinates())
    }
    pub fn current_value_of<T: Clone + 'static>(&self, local: &'static KompozitionLocal<T>) -> T {
        match self.cx() {
            Some(c) => c.current_value_of(local),
            None => local.default_value(),
        }
    }
    /// Its host's dispatcher (see [`NodeCx::dispatcher`]).
    pub fn dispatcher(&self) -> Option<kompoz_coroutines::Dispatcher> {
        self.cx().and_then(|c| c.dispatcher())
    }
    /// The node's coroutine scope (see [`NodeCx::coroutine_scope`]); none once it is gone or detached.
    pub fn coroutine_scope(&self) -> Option<CoroutineScope> {
        self.cx().filter(|c| c.is_attached()).map(|c| c.coroutine_scope())
    }
    pub fn observe_reads<R>(&self, f: impl FnOnce() -> R) -> R {
        match self.cx() {
            Some(c) => c.observe_reads(f),
            None => f(),
        }
    }
}

/// A node made apart from any chain, handed out by what owns it to be delegated to by whichever node takes it
/// (Compose's DelegatableNode an object hands out: an OverscrollEffect's `node`, which `Modifier.overscroll` or a
/// scrollable area delegates to): [`NodeCx::delegate_node`] attaches it where that node is,
/// [`NodeCx::undelegate_node`] gives it back. Clones are the same node.
#[derive(Clone)]
pub struct DelegatableNode {
    pub(crate) cell: NodeRc,
}

impl DelegatableNode {
    /// `node`, in no chain yet.
    pub fn new<T: ModifierNode>(node: T) -> DelegatableNode {
        DelegatableNode { cell: NodeCell::new_rc(node) }
    }

    /// Whether it is attached (delegated to by an attached node: Compose's `node.isAttached`).
    pub fn is_attached(&self) -> bool {
        self.cell.meta.attached.get()
    }

    /// Its context (what it invalidates and reads is where it is delegated to).
    pub fn cx(&self) -> NodeCx {
        NodeCx { cell: self.cell.clone() }
    }

    /// Whether `other` is the same node.
    pub fn ptr_eq(&self, other: &DelegatableNode) -> bool {
        Rc::ptr_eq(&self.cell, &other.cell)
    }

    /// Whether `cx`'s node delegates to it.
    pub fn is_delegate_of(&self, cx: &NodeCx) -> bool {
        self.cell.meta.delegator.borrow().as_ref().and_then(Weak::upgrade).is_some_and(|d| Rc::ptr_eq(&d, &cx.cell))
    }
}

impl std::fmt::Debug for DelegatableNode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "DelegatableNode({:p})", Rc::as_ptr(&self.cell))
    }
}

/// A delegate a node made ([`NodeCx::delegate`]): the delegating node keeps it to call it.
pub struct DelegatedNode<T: ModifierNode> {
    pub(crate) cell: Rc<NodeCell<T>>,
}

impl<T: ModifierNode> Clone for DelegatedNode<T> {
    fn clone(&self) -> Self {
        DelegatedNode { cell: self.cell.clone() }
    }
}

impl<T: ModifierNode> DelegatedNode<T> {
    pub fn borrow(&self) -> Ref<'_, T> {
        self.cell.node.borrow()
    }
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.cell.node.borrow_mut()
    }
    /// The delegate's own context (it invalidates and reads as the node it is part of).
    pub fn cx(&self) -> NodeCx {
        let cell: NodeRc = self.cell.clone();
        NodeCx { cell }
    }
}

/// A node others can find by its key, up or down the tree (Compose's TraversableNode).
pub trait TraversableNode {
    /// What it is known by: nodes of one kind share a key.
    fn traverse_key(&self) -> &dyn Any;
    /// Whether `key` is its key (by default: the same type and, for a `&'static str`, the same text).
    fn traverse_key_eq(&self, key: &dyn Any) -> bool {
        let mine = self.traverse_key();
        if let (Some(a), Some(b)) = (mine.downcast_ref::<&'static str>(), key.downcast_ref::<&'static str>()) {
            return a == b;
        }
        if let (Some(a), Some(b)) = (mine.downcast_ref::<std::any::TypeId>(), key.downcast_ref::<std::any::TypeId>()) {
            return a == b;
        }
        std::ptr::eq(mine as *const dyn Any as *const (), key as *const dyn Any as *const ())
    }
}

/// What a traversal does after a node (Compose's TraversableNode.Companion.TraverseDescendantsAction).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TraversalDecision {
    ContinueTraversal,
    SkipSubtreeAndContinueTraversal,
    CancelTraversal,
}

/// A node told when what it read in [`NodeCx::observe_reads`] changes (Compose's ObserverModifierNode).
pub trait ObserverModifierNode {
    fn on_observed_reads_changed(&mut self);
}

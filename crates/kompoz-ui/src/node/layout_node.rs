//! The layout node: what a layout composable emits (Compose's LayoutNode) — its children, its modifier's chain and
//! coordinators, its measure policy, and its place in the measure and layout passes.

use super::alignment_lines::{AlignmentLines, NOT_PLACED};
use super::{NodeCoordinator, Owner, ReaderScope};
use crate::layout::{LayerBlock, MeasurePolicy};
use crate::modifier::chain::NodeChain;
use crate::modifier::{Modifier, NodeKind, NodeRc, dispatch_for_kind};
use crate::semantics::SemanticsConfiguration;
use crate::unit::Constraints;
use kompoz_graphics::{Density, IntOffset, IntSize, LayoutDirection};
use kompoz_runtime::{KompozitionLocal, KompozitionLocalContext, Node};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

/// Where a layout node is in the passes (Compose's LayoutState, without lookahead).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LayoutState {
    Measuring,
    LayingOut,
    Idle,
}

/// How its parent used it (Compose's UsageByParent): measured in its measure block, in its placement block, or not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum UsageByParent {
    InMeasureBlock,
    InLayoutBlock,
    NotUsed,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A node of the layout tree (Compose's LayoutNode): emitted by `Layout`, it measures its children with its
/// measure policy through the coordinators of its modifier's layout nodes, places them, draws, takes input and
/// describes itself through its modifier's nodes. Held as `Rc`, its parent weakly.
pub struct LayoutNode {
    /// Unique while the program runs: its semantics id and what a display list knows it by.
    pub(crate) id: u64,
    pub(crate) this: Weak<LayoutNode>,
    parent: RefCell<Weak<LayoutNode>>,
    children: RefCell<Vec<Rc<LayoutNode>>>,
    owner: RefCell<Weak<Owner>>,
    depth: Cell<u32>,
    measure_policy: RefCell<Rc<dyn MeasurePolicy>>,
    density: Cell<Density>,
    layout_direction: Cell<LayoutDirection>,
    locals: RefCell<KompozitionLocalContext>,
    composite_key: Cell<u64>,
    pub(crate) pending_modifier: RefCell<Option<Modifier>>,
    pub(crate) modifier: RefCell<Modifier>,
    pub(crate) chain: RefCell<NodeChain>,
    pub(crate) aggregate_kinds: Cell<u32>,
    pub(crate) inner: Rc<NodeCoordinator>,
    pub(crate) outer: RefCell<Rc<NodeCoordinator>>,

    // The measure and layout passes (Compose's LayoutNodeLayoutDelegate and MeasurePassDelegate).
    pub(crate) layout_state: Cell<LayoutState>,
    pub(crate) measure_pending: Cell<bool>,
    pub(crate) layout_pending: Cell<bool>,
    pub(crate) layout_pending_for_alignment: Cell<bool>,
    pub(crate) measured_by_parent: Cell<UsageByParent>,
    pub(crate) intrinsics_usage_by_parent: Cell<UsageByParent>,
    pub(crate) previous_intrinsics_usage_by_parent: Cell<UsageByParent>,
    pub(crate) is_placed: Cell<bool>,
    pub(crate) is_placed_by_parent: Cell<bool>,
    pub(crate) place_order: Cell<i32>,
    pub(crate) previous_place_order: Cell<i32>,
    pub(crate) next_child_place_order: Cell<i32>,
    pub(crate) z_index: Cell<f32>,
    pub(crate) last_position: Cell<IntOffset>,
    pub(crate) last_z_index: Cell<f32>,
    pub(crate) last_layer_block: RefCell<Option<LayerBlock>>,
    pub(crate) last_constraints: Cell<Option<Constraints>>,
    pub(crate) measured_size: Cell<IntSize>,
    pub(crate) placed_once: Cell<bool>,
    pub(crate) relayout_without_parent_in_progress: Cell<bool>,
    pub(crate) on_node_placed_called: Cell<bool>,
    pub(crate) during_alignment_lines_query: Cell<bool>,
    pub(crate) laying_out_children: Cell<bool>,
    pub(crate) coordinates_accessed_during_placement: Cell<bool>,
    pub(crate) coordinates_accessed_during_modifier_placement: Cell<bool>,
    pub(crate) children_accessing_coordinates_during_placement: Cell<i32>,
    pub(crate) can_multi_measure: Cell<bool>,
    pub(crate) ignore_remeasure_requests: Cell<bool>,
    pub(crate) parent_data_cache: RefCell<Option<Rc<dyn Any>>>,
    pub(crate) parent_data_dirty: Cell<bool>,
    pub(crate) alignment_lines: AlignmentLines,
    z_sorted_dirty: Cell<bool>,
    z_sorted: RefCell<Vec<Rc<LayoutNode>>>,
    globally_positioned_observers: Cell<i32>,
    pub(crate) needs_on_positioned_dispatch: Cell<bool>,
    /// Drawn again at the next frame: what it or a node in it draws changed.
    pub(crate) draw_dirty: Cell<bool>,
    /// The same for the host's keep check, whose keeping list draws apart from the platform's (swapped with
    /// `draw_dirty` while it draws).
    pub(crate) keep_check_dirty: Cell<bool>,
    pub(crate) semantics_configuration: RefCell<Option<SemanticsConfiguration>>,
    pub(crate) semantics_invalidated: Cell<bool>,
    pub(crate) measure_reads: RefCell<Option<Rc<ReaderScope>>>,
    pub(crate) layout_reads: RefCell<Option<Rc<ReaderScope>>>,
    pub(crate) layout_modifier_reads: RefCell<Option<Rc<ReaderScope>>>,
    pub(crate) draw_reads: RefCell<Option<Rc<ReaderScope>>>,
    pub(crate) semantics_reads: RefCell<Option<Rc<ReaderScope>>>,
    /// How many times a `Layout` configured it (tests read it).
    pub(crate) compositions: Cell<u32>,
    /// A popup's root: what the popup is (the window package's).
    pub(crate) popup: RefCell<Option<Rc<crate::window::PopupSpec>>>,
    /// The roots of the popups composed in it, split out of its children (C++'s `overlays`).
    pub(crate) overlays: RefCell<Vec<Rc<LayoutNode>>>,
    /// A popup's root: the node it was composed in.
    pub(crate) anchor: RefCell<Weak<LayoutNode>>,
}

/// A measure policy measuring nothing, 0 by 0 (a node's until one is set: Compose's ErrorMeasurePolicy stands in
/// for a missing one).
fn empty_policy() -> Rc<dyn MeasurePolicy> {
    Rc::new(|scope: &crate::layout::MeasureScope, _: &[&dyn crate::layout::Measurable], c: Constraints| {
        scope.layout(c.min_width, c.min_height, |_| {})
    })
}

impl LayoutNode {
    /// A node with no children, no modifier and a policy measuring nothing.
    pub fn new() -> Rc<LayoutNode> {
        Rc::new_cyclic(|this: &Weak<LayoutNode>| {
            let inner = NodeCoordinator::new_inner(this.clone());
            LayoutNode {
                id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                this: this.clone(),
                parent: RefCell::new(Weak::new()),
                children: RefCell::new(Vec::new()),
                owner: RefCell::new(Weak::new()),
                depth: Cell::new(0),
                measure_policy: RefCell::new(empty_policy()),
                density: Cell::new(Density::default()),
                layout_direction: Cell::new(LayoutDirection::Ltr),
                locals: RefCell::new(KompozitionLocalContext::default()),
                composite_key: Cell::new(0),
                pending_modifier: RefCell::new(None),
                modifier: RefCell::new(Modifier),
                chain: RefCell::new(NodeChain::default()),
                aggregate_kinds: Cell::new(0),
                outer: RefCell::new(inner.clone()),
                inner,
                layout_state: Cell::new(LayoutState::Idle),
                measure_pending: Cell::new(false),
                layout_pending: Cell::new(false),
                layout_pending_for_alignment: Cell::new(false),
                measured_by_parent: Cell::new(UsageByParent::NotUsed),
                intrinsics_usage_by_parent: Cell::new(UsageByParent::NotUsed),
                previous_intrinsics_usage_by_parent: Cell::new(UsageByParent::NotUsed),
                is_placed: Cell::new(false),
                is_placed_by_parent: Cell::new(false),
                place_order: Cell::new(NOT_PLACED),
                previous_place_order: Cell::new(NOT_PLACED),
                next_child_place_order: Cell::new(0),
                z_index: Cell::new(0.0),
                last_position: Cell::new(IntOffset::Zero),
                last_z_index: Cell::new(0.0),
                last_layer_block: RefCell::new(None),
                last_constraints: Cell::new(None),
                measured_size: Cell::new(IntSize::Zero),
                placed_once: Cell::new(false),
                relayout_without_parent_in_progress: Cell::new(false),
                on_node_placed_called: Cell::new(false),
                during_alignment_lines_query: Cell::new(false),
                laying_out_children: Cell::new(false),
                coordinates_accessed_during_placement: Cell::new(false),
                coordinates_accessed_during_modifier_placement: Cell::new(false),
                children_accessing_coordinates_during_placement: Cell::new(0),
                can_multi_measure: Cell::new(false),
                ignore_remeasure_requests: Cell::new(false),
                parent_data_cache: RefCell::new(None),
                parent_data_dirty: Cell::new(true),
                alignment_lines: AlignmentLines::new(),
                z_sorted_dirty: Cell::new(true),
                z_sorted: RefCell::new(Vec::new()),
                globally_positioned_observers: Cell::new(0),
                needs_on_positioned_dispatch: Cell::new(false),
                draw_dirty: Cell::new(true),
                keep_check_dirty: Cell::new(true),
                semantics_configuration: RefCell::new(None),
                semantics_invalidated: Cell::new(false),
                measure_reads: RefCell::new(None),
                layout_reads: RefCell::new(None),
                layout_modifier_reads: RefCell::new(None),
                draw_reads: RefCell::new(None),
                semantics_reads: RefCell::new(None),
                compositions: Cell::new(0),
                popup: RefCell::new(None),
                overlays: RefCell::new(Vec::new()),
                anchor: RefCell::new(Weak::new()),
            }
        })
    }

    pub(crate) fn rc(&self) -> Rc<LayoutNode> {
        self.this.upgrade().expect("a layout node in use is alive")
    }

    /// Its id: unique while the program runs (its semantics node's id, what a display list knows it by).
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn parent(&self) -> Option<Rc<LayoutNode>> {
        self.parent.borrow().upgrade()
    }

    /// Its children, in the order they were emitted (copied out).
    pub fn children(&self) -> Vec<Rc<LayoutNode>> {
        self.children.borrow().clone()
    }

    pub fn depth(&self) -> u32 {
        self.depth.get()
    }

    pub(crate) fn owner(&self) -> Option<Rc<Owner>> {
        self.owner.borrow().upgrade()
    }

    /// Whether it is in a host.
    pub fn is_attached(&self) -> bool {
        self.owner.borrow().strong_count() > 0
    }

    /// Whether its parent placed it in the last layout (and so it is shown).
    pub fn is_placed(&self) -> bool {
        self.is_placed.get()
    }

    pub fn density(&self) -> Density {
        self.density.get()
    }

    pub fn layout_direction(&self) -> LayoutDirection {
        self.layout_direction.get()
    }

    pub fn modifier(&self) -> Modifier {
        self.modifier.borrow().clone()
    }

    /// The key of where it was composed (Compose's `compositeKeyHash`).
    pub fn composite_key(&self) -> u64 {
        self.composite_key.get()
    }

    /// How many times a `Layout` configured it.
    pub fn compositions(&self) -> u32 {
        self.compositions.get()
    }

    pub(crate) fn measure_policy(&self) -> Rc<dyn MeasurePolicy> {
        self.measure_policy.borrow().clone()
    }

    /// Its measured size (its outer coordinator's).
    pub fn size(&self) -> IntSize {
        self.outer_coordinator().measured_size.get()
    }

    /// The coordinates of its outermost coordinator: where it is, all its modifiers included.
    pub fn coordinates(&self) -> crate::layout::LayoutCoordinates {
        crate::layout::LayoutCoordinates::of(self.outer_coordinator())
    }

    /// The coordinates of its inner coordinator: where its content (its children) is.
    pub fn inner_coordinates(&self) -> crate::layout::LayoutCoordinates {
        crate::layout::LayoutCoordinates::of(self.inner.clone())
    }

    pub(crate) fn outer_coordinator(&self) -> Rc<NodeCoordinator> {
        self.outer.borrow().clone()
    }

    /// Its coordinators, outer to inner, the inner one excluded.
    pub(crate) fn coordinators(&self) -> Vec<Rc<NodeCoordinator>> {
        let mut out = Vec::new();
        let mut c = self.outer_coordinator();
        while !Rc::ptr_eq(&c, &self.inner) {
            out.push(c.clone());
            match c.wrapped() {
                Some(w) => c = w,
                None => break,
            }
        }
        out
    }

    /// Its chain's nodes, head first (copied out).
    pub(crate) fn chain_nodes(&self) -> Vec<NodeRc> {
        self.chain.borrow().nodes.clone()
    }

    pub(crate) fn chain_aggregate_kinds(&self) -> u32 {
        self.aggregate_kinds.get()
    }

    /// Whether a node of `kind` is in its chain.
    pub(crate) fn has_kind(&self, kind: NodeKind) -> bool {
        kind.is_in(self.aggregate_kinds.get())
    }

    /// Calls `f` on its chain's nodes of `kind`, head first (Compose's `nodes.headToTail(kind)`).
    pub(crate) fn head_to_tail(&self, kind: NodeKind, f: &mut dyn FnMut(&NodeRc)) {
        if !self.has_kind(kind) {
            return;
        }
        for node in self.chain_nodes() {
            if node.is_kind(kind) {
                dispatch_for_kind(&node, kind, f);
            }
        }
    }

    /// `local`'s value where it was composed (what a node's `current_value_of` reads): a dynamic local's as it is
    /// now, the read told to whoever observes reads (a measure, a placement, a drawing runs again when it changes);
    /// a static local's as it was when this was last composed.
    pub fn current_value_of<T: Clone + 'static>(&self, local: &'static KompozitionLocal<T>) -> T {
        self.locals.borrow().get(local)
    }

    // ---- What the composition sets (Compose's ComposeUiNode setters) ----

    /// Its measure policy, which measures it again unless it is the one it has (the same `Rc`: what `Layout`
    /// keeps while its closure policy is the same memoized closure, and composables keep of their own policies).
    pub fn set_measure_policy(&self, policy: Rc<dyn MeasurePolicy>) {
        let same = Rc::ptr_eq(&*self.measure_policy.borrow(), &policy);
        if !same {
            *self.measure_policy.borrow_mut() = policy;
            self.invalidate_measurements();
        }
    }

    pub fn set_density(&self, density: Density) {
        if self.density.get() != density {
            self.density.set(density);
            self.on_density_or_layout_direction_changed();
            if self.is_attached() {
                crate::input::on_density_changed(self);
            }
        }
    }

    pub fn set_layout_direction(&self, direction: LayoutDirection) {
        if self.layout_direction.get() != direction {
            self.layout_direction.set(direction);
            self.on_density_or_layout_direction_changed();
        }
    }

    fn on_density_or_layout_direction_changed(&self) {
        self.invalidate_measurements();
        if let Some(p) = self.parent() {
            p.invalidate_layer();
        }
        // (Compose's invalidateLayers: what it draws itself draws otherwise, which a canvas keeping nodes keeps.)
        self.invalidate_draw();
        self.invalidate_semantics();
        // (the inner coordinator's too: the innermost layer modifier's layer is on it)
        for c in self.coordinators().into_iter().chain(std::iter::once(self.inner.clone())) {
            if c.layer.borrow().is_some() {
                c.update_layer_parameters(true);
            }
        }
    }

    /// The composition locals where it is composed, for its nodes' `current_value_of` (Compose's
    /// `compositionLocalMap` setter): when they are others (a static local given another value), its nodes are
    /// invalidated as an update would, so what they read of them is read again.
    pub fn set_locals(&self, locals: KompozitionLocalContext) {
        if *self.locals.borrow() == locals {
            return;
        }
        let view_configuration = &crate::platform::LocalViewConfiguration;
        let before = self.locals.borrow().peek(view_configuration);
        let old = self.locals.replace(locals);
        drop(old);
        if !self.is_attached() {
            return;
        }
        // (Compose's `viewConfiguration` setter: pointer input under way is cancelled)
        if self.locals.borrow().peek(view_configuration) != before {
            crate::input::on_view_configuration_changed(self);
        }
        for node in self.chain_nodes() {
            if node.meta.attached.get() {
                crate::modifier::chain::auto_invalidate(&node, crate::modifier::chain::Phase::Updated);
            }
        }
    }

    pub fn set_composite_key(&self, key: u64) {
        self.composite_key.set(key);
    }

    pub(crate) fn note_composed(&self) {
        self.compositions.set(self.compositions.get() + 1);
    }

    // ---- The tree ----

    /// Inserts `child` at `index` (Compose's `insertAt`): attached if this node is.
    pub(crate) fn insert_at(self: &Rc<Self>, index: usize, child: Rc<LayoutNode>) {
        assert!(child.parent().is_none(), "kompoz: a layout node can't have two parents");
        assert!(!child.is_attached(), "kompoz: a layout node already attached elsewhere");
        *child.parent.borrow_mut() = Rc::downgrade(self);
        self.children.borrow_mut().insert(index, child.clone());
        self.on_z_sorted_children_invalidated();
        if child.globally_positioned_observers.get() > 0 {
            self.add_globally_positioned_observers(1);
        }
        if let Some(owner) = self.owner() {
            child.attach(&owner);
        }
        self.invalidate_measurements();
    }

    /// Removes the child at `index` (Compose's `removeAt`): detached if it was attached.
    pub(crate) fn remove_at(self: &Rc<Self>, index: usize) -> Rc<LayoutNode> {
        let child = self.children.borrow_mut().remove(index);
        self.on_child_removed(&child);
        child
    }

    fn on_child_removed(self: &Rc<Self>, child: &Rc<LayoutNode>) {
        if child.globally_positioned_observers.get() > 0 {
            self.add_globally_positioned_observers(-1);
        }
        if child.is_attached() {
            child.detach();
        }
        *child.parent.borrow_mut() = Weak::new();
        *child.outer_coordinator().wrapped_by.borrow_mut() = Weak::new();
        self.on_z_sorted_children_invalidated();
        self.invalidate_measurements();
    }

    /// Attaches it (and its subtree) to `owner` (Compose's `attach`).
    pub(crate) fn attach(self: &Rc<Self>, owner: &Rc<Owner>) {
        assert!(!self.is_attached(), "kompoz: a layout node attached twice");
        let parent = self.parent();
        if parent.is_none() {
            // The root: placed by the host.
            self.is_placed.set(true);
        }
        *self.outer_coordinator().wrapped_by.borrow_mut() =
            parent.as_ref().map_or_else(Weak::new, |p| Rc::downgrade(&p.inner));
        *self.owner.borrow_mut() = Rc::downgrade(owner);
        self.depth.set(parent.as_ref().map_or(0, |p| p.depth.get() + 1));
        let pending = self.pending_modifier.borrow_mut().take();
        if let Some(m) = pending {
            self.apply_modifier(m, false);
        }
        owner.on_attach(self);
        self.mark_chain_attached();
        for child in self.children() {
            child.attach(owner);
        }
        for popup in self.overlays() {
            popup.attach(owner);
        }
        self.run_chain_attach_lifecycle();
        self.invalidate_measurements();
        if let Some(p) = &parent {
            p.invalidate_measurements();
        }
        self.invalidate_parent_data();
        self.invalidate_draw();
        if self.has_kind(NodeKind::Semantics) {
            self.invalidate_semantics();
        }
        owner.semantics_changed();
    }

    /// Detaches it and its subtree (Compose's `detach`).
    pub(crate) fn detach(self: &Rc<Self>) {
        let owner = self.owner().expect("kompoz: detaching a layout node that isn't attached");
        if let Some(parent) = self.parent() {
            parent.invalidate_layer();
            parent.invalidate_measurements();
            self.measured_by_parent.set(UsageByParent::NotUsed);
        }
        self.alignment_lines.reset();
        let mut c = Some(self.outer_coordinator());
        while let Some(x) = c {
            x.release_layer();
            if self.is_placed.get() {
                x.on_unplaced();
            }
            c = x.wrapped();
        }
        self.run_chain_detach_lifecycle();
        self.ignore_remeasure_requests.set(true);
        for child in self.children() {
            child.detach();
        }
        for popup in self.overlays() {
            if popup.is_attached() {
                popup.detach();
            }
        }
        self.ignore_remeasure_requests.set(false);
        self.mark_chain_detached();
        owner.on_detach(self);
        *self.owner.borrow_mut() = Weak::new();
        self.depth.set(0);
        self.place_order.set(NOT_PLACED);
        self.previous_place_order.set(NOT_PLACED);
        self.is_placed.set(false);
        for scope in [
            &self.measure_reads,
            &self.layout_reads,
            &self.layout_modifier_reads,
            &self.draw_reads,
            &self.semantics_reads,
        ] {
            *scope.borrow_mut() = None;
        }
        if self.semantics_configuration.borrow_mut().take().is_some() {
            self.semantics_invalidated.set(false);
        }
        owner.semantics_changed();
    }

    /// The children sorted by z-index, then by the order their parent placed them (Compose's
    /// `zSortedChildren`): the order they are drawn in (and hit in reverse).
    pub fn z_sorted_children(&self) -> Vec<Rc<LayoutNode>> {
        if self.z_sorted_dirty.replace(false) {
            let mut sorted = self.children();
            sorted.sort_by(|a, b| {
                a.z_index
                    .get()
                    .partial_cmp(&b.z_index.get())
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.place_order.get().cmp(&b.place_order.get()))
            });
            *self.z_sorted.borrow_mut() = sorted;
        }
        self.z_sorted.borrow().clone()
    }

    pub(crate) fn on_z_sorted_children_invalidated(&self) {
        self.z_sorted_dirty.set(true);
    }

    // ---- Invalidation ----

    /// Measured again at the next layout (Compose's `requestRemeasure`); intrinsics users above too.
    pub(crate) fn request_remeasure(&self, force: bool) {
        if self.ignore_remeasure_requests.get() {
            return;
        }
        let Some(owner) = self.owner() else { return };
        owner.request_remeasure(&self.rc(), force);
        self.invalidate_intrinsics_parent(force);
    }

    /// Placed again at the next layout (Compose's `requestRelayout`).
    pub fn request_relayout(&self) {
        self.request_relayout_forced(false);
    }

    pub(crate) fn request_relayout_forced(&self, force: bool) {
        if let Some(owner) = self.owner() {
            owner.request_relayout(&self.rc(), force);
        }
    }

    /// Measured again (Compose's `invalidateMeasurements`).
    pub fn invalidate_measurements(&self) {
        self.request_remeasure(false);
    }

    /// The measure of everything in it again (Compose's `invalidateMeasurementForSubtree`).
    pub fn invalidate_measurement_for_subtree(&self) {
        self.request_remeasure(false);
        for c in self.children() {
            c.invalidate_measurement_for_subtree();
        }
    }

    /// Drawn again: it, and what it is drawn into (its ancestors, for what keeps their drawings). Every ancestor:
    /// a node drawn only into a graphics layer's recording is never kept and stays marked, so one marked already
    /// doesn't mean those above it are.
    pub fn invalidate_draw(&self) {
        let mut node = Some(self.rc());
        while let Some(n) = node {
            n.draw_dirty.set(true);
            n.keep_check_dirty.set(true);
            node = n.parent();
        }
        if let Some(owner) = self.owner() {
            owner.request_draw();
        }
    }

    /// Compose's `invalidateLayer`: what draws it draws again.
    pub(crate) fn invalidate_layer(&self) {
        self.invalidate_draw();
    }

    /// Its semantics worked out again (Compose's `invalidateSemantics`); the listeners told after the frame.
    pub fn invalidate_semantics(&self) {
        if let Some(owner) = self.owner() {
            owner.note_semantics_invalidated(self);
            owner.semantics_changed();
        }
        self.semantics_invalidated.set(true);
    }

    /// Its parent data worked out again, and its parent measured again if it changed (Compose's
    /// `invalidateParentData`).
    pub(crate) fn invalidate_parent_data(&self) {
        self.parent_data_dirty.set(true);
        if self.update_parent_data()
            && let Some(p) = self.parent()
        {
            p.request_remeasure(false);
        }
    }

    /// Compose's `MeasurePassDelegate.updateParentData`: whether it changed.
    pub(crate) fn update_parent_data(&self) -> bool {
        if !self.parent_data_dirty.get() {
            return false;
        }
        let new = self.compute_parent_data();
        let had = self.parent_data_cache.borrow().is_some();
        if !had && new.is_none() {
            self.parent_data_dirty.set(false);
            return false;
        }
        self.parent_data_dirty.set(false);
        *self.parent_data_cache.borrow_mut() = new;
        true
    }

    /// The chain's parent data nodes folded tail to head.
    fn compute_parent_data(&self) -> Option<Rc<dyn Any>> {
        if !self.has_kind(NodeKind::ParentData) {
            return None;
        }
        let density = self.density();
        let mut data: Option<Box<dyn Any>> = None;
        for node in self.chain_nodes().iter().rev() {
            if node.is_kind(NodeKind::ParentData) {
                dispatch_for_kind(node, NodeKind::ParentData, &mut |cell| {
                    if let Some(p) = cell.node.borrow_mut().as_parent_data() {
                        data = p.modify_parent_data(&density, data.take());
                    }
                });
            }
        }
        data.map(Rc::from)
    }

    /// What its parent data modifiers say to its parent.
    pub fn parent_data(&self) -> Option<Rc<dyn Any>> {
        if self.parent_data_dirty.get() {
            self.update_parent_data();
        }
        self.parent_data_cache.borrow().clone()
    }

    pub(crate) fn globally_positioned_observers(&self) -> i32 {
        self.globally_positioned_observers.get()
    }

    /// Counts nodes in its subtree told where they are after layout (Compose's `globallyPositionedObservers`).
    pub(crate) fn add_globally_positioned_observers(&self, delta: i32) {
        let before = self.globally_positioned_observers.get();
        let after = before + delta;
        self.globally_positioned_observers.set(after);
        if (before == 0) != (after == 0)
            && let Some(p) = self.parent()
        {
            p.add_globally_positioned_observers(if after > 0 { 1 } else { -1 });
        }
    }

    /// Its global position callbacks are due (Compose's `invalidateOnPositioned`).
    pub(crate) fn invalidate_on_positioned(&self) {
        if self.globally_positioned_observers.get() == 0
            || self.layout_pending.get()
            || self.measure_pending.get()
            || self.needs_on_positioned_dispatch.get()
        {
            return;
        }
        if let Some(owner) = self.owner() {
            owner.request_on_positioned_callback(&self.rc());
        }
    }

    /// Tells its global position nodes where it is (Compose's `dispatchOnPositionedCallbacks`).
    pub(crate) fn dispatch_on_positioned_callbacks(&self) {
        if self.layout_state.get() != LayoutState::Idle || self.layout_pending.get() || self.measure_pending.get() {
            return;
        }
        if !self.is_placed.get() {
            return;
        }
        self.head_to_tail(NodeKind::GlobalPositionAware, &mut |cell| {
            let Some(c) = cell.coordinator() else { return };
            let coordinates = crate::layout::LayoutCoordinates::of(c);
            if let Some(g) = cell.node.borrow_mut().as_global_position_aware() {
                g.on_globally_positioned(&coordinates);
            }
        });
    }

    /// One of its coordinators moved or resized (Compose's `onCoordinatorRectChanged`): placements that read
    /// coordinates run again.
    pub(crate) fn on_coordinator_rect_changed(&self, _coordinator: &NodeCoordinator) {
        self.request_layout_if_coordinates_are_used_and_notify_children();
    }

    /// Something that moves what is under it changed (Compose's `Owner.onLayoutChange`): the semantics bounds, the
    /// global position callbacks.
    pub(crate) fn on_layout_change(&self) {
        if let Some(owner) = self.owner() {
            owner.semantics_bounds_changed();
        }
    }

    /// The UI's `Layout` set the modifier.
    pub(crate) fn modifier_set(&self, modifier: Modifier) {
        let same = match &*self.pending_modifier.borrow() {
            Some(pending) => *pending == modifier,
            None => *self.modifier.borrow() == modifier,
        };
        if !same {
            self.rc().set_modifier(modifier);
        }
    }
}

impl Node for LayoutNode {
    /// Compose's applier: the children become `children` (moved, inserted, removed), attached and detached.
    fn set_children(&self, children: &[Rc<dyn Node>]) {
        let me = self.rc();
        let emitted: Vec<Rc<LayoutNode>> = children
            .iter()
            .map(|c| (c.clone() as Rc<dyn Any>).downcast::<LayoutNode>().expect("kompoz: a node of another kind"))
            .collect();
        // Popup roots go to its overlays.
        let new = me.split_overlays(emitted);
        // Those that left, then the new order with those that came.
        let old = self.children();
        for (i, child) in old.iter().enumerate().rev() {
            if !new.iter().any(|n| Rc::ptr_eq(n, child)) {
                me.remove_at(i);
            }
        }
        for (i, child) in new.iter().enumerate() {
            let at = self.children.borrow().iter().position(|c| Rc::ptr_eq(c, child));
            match at {
                Some(j) if j == i => {}
                Some(j) => {
                    // Moved: taken out and put back without leaving the tree (Compose's `move`).
                    let moved = self.children.borrow_mut().remove(j);
                    self.children.borrow_mut().insert(i, moved);
                    self.on_z_sorted_children_invalidated();
                    self.invalidate_measurements();
                }
                None => {
                    // Moved here from another node (movable content): taken out of the one it was in first, which
                    // may not have been told its new children yet, or may itself have left the tree.
                    if let Some(old_parent) = child.parent() {
                        let at = old_parent.children.borrow().iter().position(|c| Rc::ptr_eq(c, child));
                        if let Some(j) = at {
                            old_parent.remove_at(j);
                        }
                    }
                    me.insert_at(i, child.clone())
                }
            }
        }
    }
}

impl fmt::Debug for LayoutNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "LayoutNode#{}(children: {}, modifier: {:?})",
            self.id,
            self.children.borrow().len(),
            self.modifier.borrow()
        )
    }
}

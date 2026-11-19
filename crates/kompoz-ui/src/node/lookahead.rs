//! The lookahead pass (Compose's LookaheadPassDelegate and LookaheadDelegate): in a lookahead scope, a layout is
//! measured and placed first where it is going to be — every layout of the scope measured by the measure
//! policies and layout modifiers as usual, approach nodes by their plain `measure` — and only then by the main
//! pass, the "approach", in which an [`ApproachLayoutModifierNode`](crate::layout::ApproachLayoutModifierNode) sees
//! that destination and measures and places on its way to it.
//!
//! A layout node in a scope has a [`LookaheadPassDelegate`] (its lookahead constraints, size, position, place order
//! and alignment lines: what its parent's lookahead measures and places) and each of its coordinators a
//! [`LookaheadDelegate`] (its lookahead size, position, measure result, alignment lines and rulers). The node that
//! starts a lookahead — a scope's top layouts, a layout with an approach node outside any scope, or what a
//! subcomposing layout composes only in its approach — is a lookahead root: its main measure measures it in
//! lookahead first, its main placement places it in lookahead first.

use super::alignment_lines::{AlignmentLines, NOT_PLACED};
use super::{LayoutNode, LayoutState, NodeCoordinator, UsageByParent};
use crate::layout::{
    AlignmentLine, IntrinsicMeasurable, LayerBlock, LayoutCoordinates, Measurable, MeasureResult, PlaceTarget,
    Placeable, PlacementScope, VirtualLayoutNode,
};
use crate::unit::Constraints;
use kompoz_graphics::{IntOffset, IntSize, Offset};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

// ---------------------------------------------------------------------------------------------------------------
// Lookahead roots

/// The nearest lookahead root of a layout node (Compose's `LayoutNode.lookaheadRoot`): a `LookaheadScope`'s virtual
/// node, or a layout node itself (one with an approach node outside any scope).
#[derive(Clone)]
pub(crate) enum LookaheadRoot {
    Node(Weak<LayoutNode>),
    /// A `LookaheadScope` (Compose's `isVirtualLookaheadRoot`).
    Virtual(Weak<VirtualLayoutNode>),
}

impl PartialEq for LookaheadRoot {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LookaheadRoot::Node(a), LookaheadRoot::Node(b)) => a.ptr_eq(b),
            (LookaheadRoot::Virtual(a), LookaheadRoot::Virtual(b)) => a.ptr_eq(b),
            _ => false,
        }
    }
}

impl LookaheadRoot {
    /// The coordinates of the scope it roots (Compose's `lookaheadScopeCoordinates` of an approach measure scope):
    /// a `LookaheadScope`'s parent's inner coordinator (its first layout's outer one at the top of the tree), a
    /// layout node's outer coordinator.
    pub(crate) fn scope_coordinator(&self) -> Option<Rc<NodeCoordinator>> {
        match self {
            LookaheadRoot::Node(n) => n.upgrade().map(|n| n.outer_coordinator()),
            LookaheadRoot::Virtual(v) => {
                let v = v.upgrade()?;
                match v.parent() {
                    Some(p) => Some(p.inner.clone()),
                    None => v.nodes().first().map(|n| n.outer_coordinator()),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The lookahead pass of a layout node

/// Where a lookahead pass delegate was placed (Compose's `PlacedState`: `IsPlacedInLookahead`, …).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PlacedState {
    PlacedInLookahead,
    PlacedInApproach,
    NotPlaced,
}

/// A layout node measured and placed by its parent's lookahead (Compose's LookaheadPassDelegate): what a measure
/// policy in the lookahead pass gets for the node, measured within its lookahead constraints, placed where it is
/// going to be.
pub(crate) struct LookaheadPassDelegate {
    node: Weak<LayoutNode>,
    this: Weak<LookaheadPassDelegate>,
    pub(crate) measured_by_parent: Cell<UsageByParent>,
    pub(crate) place_order: Cell<i32>,
    previous_place_order: Cell<i32>,
    placed_state: Cell<PlacedState>,
    lookahead_constraints: Cell<Option<Constraints>>,
    /// Its lookahead size as its parent sees it (the outer lookahead delegate's, within its constraints).
    measured_size: Cell<IntSize>,
    measured_once: Cell<bool>,
    pub(crate) placed_once: Cell<bool>,
    last_position: Cell<IntOffset>,
    last_z_index: Cell<f32>,
    last_layer_block: RefCell<Option<LayerBlock>>,
    during_alignment_lines_query: Cell<bool>,
    pub(crate) laying_out_children: Cell<bool>,
    relayout_without_parent_in_progress: Cell<bool>,
    on_node_placed_called: Cell<bool>,
    parent_data_dirty: Cell<bool>,
    parent_data: RefCell<Option<Rc<dyn Any>>>,
    pub(crate) alignment_lines: AlignmentLines,
    pub(crate) placed_under_motion_frame_of_reference: Cell<bool>,
}

impl LookaheadPassDelegate {
    pub(crate) fn new(node: &LayoutNode) -> Rc<LookaheadPassDelegate> {
        Rc::new_cyclic(|this| LookaheadPassDelegate {
            node: node.this.clone(),
            this: this.clone(),
            measured_by_parent: Cell::new(UsageByParent::NotUsed),
            place_order: Cell::new(NOT_PLACED),
            previous_place_order: Cell::new(NOT_PLACED),
            placed_state: Cell::new(PlacedState::NotPlaced),
            lookahead_constraints: Cell::new(None),
            measured_size: Cell::new(IntSize::Zero),
            measured_once: Cell::new(false),
            placed_once: Cell::new(false),
            last_position: Cell::new(IntOffset::Zero),
            last_z_index: Cell::new(0.0),
            last_layer_block: RefCell::new(None),
            during_alignment_lines_query: Cell::new(false),
            laying_out_children: Cell::new(false),
            relayout_without_parent_in_progress: Cell::new(false),
            on_node_placed_called: Cell::new(false),
            parent_data_dirty: Cell::new(true),
            parent_data: RefCell::new(node.parent_data_cache.borrow().clone()),
            alignment_lines: AlignmentLines::new(),
            placed_under_motion_frame_of_reference: Cell::new(false),
        })
    }

    fn rc(&self) -> Rc<LookaheadPassDelegate> {
        self.this.upgrade().expect("a lookahead pass delegate in use is alive")
    }

    pub(crate) fn node(&self) -> Rc<LayoutNode> {
        self.node.upgrade().expect("a lookahead pass delegate of a layout node that is gone")
    }

    fn outer_lookahead(&self) -> Rc<LookaheadDelegate> {
        self.node()
            .outer_coordinator()
            .lookahead_delegate()
            .expect("kompoz: a lookahead node's coordinator without a lookahead delegate")
    }

    fn inner_lookahead(&self) -> Rc<LookaheadDelegate> {
        self.node().inner.lookahead_delegate().expect("kompoz: Expected lookahead delegate")
    }

    /// Whether it was placed, in its parent's lookahead or in an approach (Compose's `isPlaced`).
    pub(crate) fn is_placed(&self) -> bool {
        self.placed_state.get() != PlacedState::NotPlaced
    }

    /// The constraints it was last measured with in lookahead (Compose's `lastConstraints`).
    pub(crate) fn last_constraints(&self) -> Option<Constraints> {
        self.lookahead_constraints.get()
    }

    /// It wasn't placed by its parent's lookahead before its approach placement (Compose's `onApproachPlacement`):
    /// its lookahead placement comes in the approach then.
    pub(crate) fn on_approach_placement(&self) {
        if self.placed_state.get() == PlacedState::NotPlaced {
            let node = self.node();
            if node.is_out_most_lookahead_root() {
                return;
            }
            node.detached_from_parent_lookahead_placement.set(true);
        }
    }

    /// Whether its approach placement places it in lookahead first (Compose's `needsToBePlacedInApproach`): a
    /// lookahead root, or a node its parent's lookahead measured but didn't place.
    pub(crate) fn needs_to_be_placed_in_approach(&self) -> bool {
        let node = self.node();
        node.is_out_most_lookahead_root() || node.detached_from_parent_lookahead_placement.get()
    }

    /// The lookahead pass delegates of its children, measured by its measure policy in lookahead (Compose's
    /// `childDelegates`).
    pub(crate) fn child_delegates(&self) -> Vec<Rc<LookaheadPassDelegate>> {
        self.node().children().iter().map(|c| c.lookahead_pass_delegate()).collect()
    }

    // ---- Laying out ----

    /// Places its children in lookahead if that is pending, observing what the placement block reads, and works
    /// its lookahead alignment lines out if needed (Compose's `layoutChildren`).
    pub(crate) fn layout_children(&self) {
        let node = self.node();
        self.laying_out_children.set(true);
        if node.lookahead_layout_pending.get() {
            self.on_before_layout_children();
        }
        let inner = self.inner_lookahead();
        if node.lookahead_layout_pending_for_alignment.get()
            || (!self.during_alignment_lines_query.get()
                && !inner.placing_for_alignment.get()
                && node.lookahead_layout_pending.get())
        {
            node.lookahead_layout_pending.set(false);
            let old = node.layout_state.replace(LayoutState::LookaheadLayingOut);
            node.set_lookahead_coordinates_accessed(true, false);
            match node.owner() {
                Some(owner) => {
                    let scope =
                        node.reader_scope(&node.layout_lookahead_reads, |n| n.request_lookahead_relayout(false));
                    owner.snapshot_observer().observe(&scope, || self.layout_children_block());
                }
                None => self.layout_children_block(),
            }
            node.layout_state.set(old);
            if node.lookahead_coordinates_accessed_during_placement.get() && inner.placing_for_alignment.get() {
                node.request_lookahead_relayout(false);
            }
            node.lookahead_layout_pending_for_alignment.set(false);
        }
        if self.alignment_lines.used_during_parent_layout.get() {
            self.alignment_lines.previous_used_during_parent_layout.set(true);
        }
        if self.alignment_lines.dirty.get() && self.alignment_lines_required() {
            self.recalculate_alignment_lines();
        }
        self.laying_out_children.set(false);
    }

    fn layout_children_block(&self) {
        let node = self.node();
        self.clear_place_order();
        let children = node.children();
        for c in &children {
            c.lookahead_pass_delegate().alignment_lines.used_during_parent_layout.set(false);
        }
        let inner = self.inner_lookahead();
        let for_alignment = inner.placing_for_alignment.get();
        let were: Vec<Option<bool>> = children
            .iter()
            .map(|c| c.outer_coordinator().lookahead_delegate().map(|d| d.placing_for_alignment.replace(for_alignment)))
            .collect();
        if let Some(result) = inner.measure_result() {
            result.place_children(&node.inner.lookahead_placement_scope());
        }
        for (c, was) in children.iter().zip(were) {
            if let Some(d) = c.outer_coordinator().lookahead_delegate() {
                d.placing_for_alignment.set(was.unwrap_or(false));
            }
        }
        self.check_children_place_order_for_updates();
        for c in &children {
            let lines = &c.lookahead_pass_delegate().alignment_lines;
            lines.previous_used_during_parent_layout.set(lines.used_during_parent_layout.get());
        }
    }

    fn check_children_place_order_for_updates(&self) {
        for child in self.child_delegates() {
            if child.previous_place_order.get() != child.place_order.get() && child.place_order.get() == NOT_PLACED {
                child.mark_node_and_subtree_as_not_placed(true);
            }
        }
    }

    /// Marks it and its subtree not placed in lookahead (Compose's `markNodeAndSubtreeAsNotPlaced`): from the
    /// pass that places it (its parent's lookahead, or its approach when it is placed there).
    pub(crate) fn mark_node_and_subtree_as_not_placed(&self, in_lookahead: bool) {
        let in_approach = self.needs_to_be_placed_in_approach();
        if (in_lookahead && in_approach) || (!in_lookahead && !in_approach) {
            return;
        }
        self.placed_state.set(PlacedState::NotPlaced);
        for child in self.child_delegates() {
            child.mark_node_and_subtree_as_not_placed(true);
        }
    }

    /// Its lookahead alignment lines, worked out by placing its children for alignment in lookahead (Compose's
    /// `calculateAlignmentLines`).
    pub(crate) fn calculate_alignment_lines(&self) -> Vec<(AlignmentLine, i32)> {
        let node = self.node();
        if !self.during_alignment_lines_query.get() {
            if node.layout_state.get() == LayoutState::LookaheadMeasuring {
                self.alignment_lines.used_by_modifier_measurement.set(true);
                if self.alignment_lines.dirty.get() {
                    node.mark_lookahead_layout_pending();
                }
            } else {
                self.alignment_lines.used_by_modifier_layout.set(true);
            }
        }
        let inner = node.inner.lookahead_delegate();
        let was = inner.as_ref().map(|d| d.placing_for_alignment.replace(true));
        self.layout_children();
        if let Some(d) = &inner {
            d.placing_for_alignment.set(was.unwrap_or(false));
        }
        self.alignment_lines.last_calculation()
    }

    /// A child reading its lookahead coordinates while placing is placed again in lookahead (Compose's
    /// `notifyChildrenUsingLookaheadCoordinatesWhilePlacing`).
    pub(crate) fn notify_children_using_lookahead_coordinates_while_placing(&self) {
        let node = self.node();
        if node.children_accessing_lookahead_coordinates_during_placement.get() > 0 {
            for child in node.children() {
                let accessed = child.lookahead_coordinates_accessed_during_placement.get()
                    || child.lookahead_coordinates_accessed_during_modifier_placement.get();
                if accessed && !child.lookahead_layout_pending.get() {
                    child.request_lookahead_relayout(false);
                }
                if let Some(d) = child.lookahead_pass() {
                    d.notify_children_using_lookahead_coordinates_while_placing();
                }
            }
        }
    }

    // ---- Measuring ----

    fn measure_for_parent(&self, constraints: Constraints) -> Placeable {
        let node = self.node();
        if let Some(p) = node.parent()
            && matches!(p.layout_state.get(), LayoutState::LookaheadMeasuring | LayoutState::LookaheadLayingOut)
        {
            node.detached_from_parent_lookahead_pass.set(false);
        }
        self.track_lookahead_measurement_by_parent(&node);
        if node.intrinsics_usage_by_parent.get() == UsageByParent::NotUsed {
            node.clear_subtree_intrinsics_usage();
        }
        self.remeasure(constraints);
        Placeable::new(self.rc())
    }

    fn track_lookahead_measurement_by_parent(&self, node: &LayoutNode) {
        match node.parent() {
            Some(parent) => {
                assert!(
                    self.measured_by_parent.get() == UsageByParent::NotUsed || node.can_multi_measure.get(),
                    "kompoz: measure() may not be called multiple times on the same Measurable. If you want to get \
                     the content size of the Measurable before calculating the final constraints, please use \
                     methods like minIntrinsicWidth()/maxIntrinsicWidth() and minIntrinsicHeight()/\
                     maxIntrinsicHeight()"
                );
                let usage = match parent.layout_state.get() {
                    LayoutState::LookaheadMeasuring | LayoutState::Measuring => UsageByParent::InMeasureBlock,
                    LayoutState::LayingOut | LayoutState::LookaheadLayingOut => UsageByParent::InLayoutBlock,
                    state => panic!(
                        "kompoz: Measurable could be only measured from the parent's measure or layout block. \
                         Parents state is {state:?}"
                    ),
                };
                self.measured_by_parent.set(usage);
            }
            None => self.measured_by_parent.set(UsageByParent::NotUsed),
        }
    }

    /// Measures it in lookahead (Compose's `performMeasure`): through its outer coordinator's lookahead delegate,
    /// what it reads observed (a change measures it again in lookahead); its lookahead placement pending, and its
    /// main placement (a lookahead root's) or measure (another's) after it.
    pub(crate) fn perform_measure(&self, constraints: Constraints) {
        let node = self.node();
        node.layout_state.set(LayoutState::LookaheadMeasuring);
        node.lookahead_measure_pending.set(false);
        let outer = self.outer_lookahead();
        match node.owner() {
            Some(owner) => {
                let scope =
                    node.reader_scope(&node.measure_lookahead_reads, |n| n.request_lookahead_remeasure(false, true));
                owner.snapshot_observer().observe(&scope, || outer.measure_in_lookahead(constraints));
            }
            None => {
                outer.measure_in_lookahead(constraints);
            }
        }
        node.mark_lookahead_layout_pending();
        if node.is_out_most_lookahead_root() {
            node.mark_layout_pending();
        } else {
            node.mark_measure_pending();
        }
        node.layout_state.set(LayoutState::Idle);
    }

    /// Measures it again in lookahead if that is pending or the constraints changed, else measures what in it
    /// waits to be (Compose's `remeasure`). Whether its lookahead size changed.
    pub(crate) fn remeasure(&self, constraints: Constraints) -> bool {
        let node = self.node();
        if let Some(p) = node.parent() {
            node.can_multi_measure.set(node.can_multi_measure.get() || p.can_multi_measure.get());
        }
        if node.lookahead_measure_pending.get() || self.lookahead_constraints.get() != Some(constraints) {
            self.lookahead_constraints.set(Some(constraints));
            self.alignment_lines.used_by_modifier_measurement.set(false);
            for child in node.children() {
                if let Some(d) = child.lookahead_pass() {
                    d.alignment_lines.used_during_parent_measurement.set(false);
                }
            }
            let last =
                if self.measured_once.get() { self.measured_size.get() } else { IntSize::new(i32::MIN, i32::MIN) };
            self.measured_once.set(true);
            self.perform_measure(constraints);
            let outer = self.outer_lookahead();
            let size = IntSize::new(outer.width(), outer.height());
            self.measured_size.set(size);
            return last != size;
        }
        if let Some(owner) = node.owner() {
            owner.force_measure_the_subtree(&node, true);
        }
        node.reset_subtree_intrinsics_usage();
        false
    }

    // ---- Placing ----

    /// Placed by its parent's lookahead, or in its approach (Compose's `placeSelf`).
    fn place_self(&self, position: IntOffset, z_index: f32, layer: Option<LayerBlock>) {
        let node = self.node();
        if node.parent().is_some_and(|p| p.layout_state.get() == LayoutState::LookaheadLayingOut) {
            node.detached_from_parent_lookahead_placement.set(false);
        }
        node.layout_state.set(LayoutState::LookaheadLayingOut);
        self.placed_once.set(true);
        self.on_node_placed_called.set(false);
        if position != self.last_position.get() {
            if node.lookahead_coordinates_accessed_during_modifier_placement.get()
                || node.lookahead_coordinates_accessed_during_placement.get()
            {
                node.lookahead_layout_pending.set(true);
            }
            self.notify_children_using_lookahead_coordinates_while_placing();
        }
        self.last_position.set(position);
        if !node.lookahead_layout_pending.get() && self.is_placed() {
            self.outer_lookahead().place_self_apparent_to_real_offset(position);
            self.on_node_placed();
        } else {
            node.set_lookahead_coordinates_accessed(false, false);
            self.alignment_lines.used_by_modifier_layout.set(false);
            match node.owner() {
                Some(owner) => {
                    let scope = node
                        .reader_scope(&node.layout_modifier_lookahead_reads, |n| n.request_lookahead_relayout(false));
                    owner.snapshot_observer().observe(&scope, || self.layout_modifier_block(&node));
                }
                None => self.layout_modifier_block(&node),
            }
        }
        self.last_z_index.set(z_index);
        *self.last_layer_block.borrow_mut() = layer;
        node.layout_state.set(LayoutState::Idle);
    }

    /// Its outer coordinator's lookahead delegate placed where it was last placed, in its parent's lookahead
    /// placement scope (or, placed in its approach, in the main one, whose motion frame of reference it takes).
    fn layout_modifier_block(&self, node: &Rc<LayoutNode>) {
        let expects_lookahead_placement_from_parent =
            !node.is_out_most_lookahead_root() && !node.detached_from_parent_lookahead_placement.get();
        let parent = node.outer_coordinator().wrapped_by();
        let scope = match &parent {
            Some(p) if expects_lookahead_placement_from_parent && p.lookahead_delegate().is_some() => {
                p.lookahead_placement_scope()
            }
            Some(p) => p.placement_scope(),
            None => node
                .owner()
                .map(|o| o.root_placement_scope())
                .unwrap_or_else(|| PlacementScope::new(0, node.layout_direction())),
        };
        let outer = self.outer_lookahead();
        scope.place_at(&Placeable::new(outer), self.last_position.get(), 0.0);
    }

    /// Its outer lookahead delegate is placed under a motion frame of reference, or not (Compose's
    /// `updatePlacedUnderMotionFrameOfReference`).
    fn update_motion_frame_of_reference(&self, new: bool) {
        if let Some(outer) = self.node().outer_coordinator().lookahead_delegate()
            && outer.placed_under_motion_frame_of_reference.get() != new
        {
            outer.placed_under_motion_frame_of_reference.set(new);
        }
        self.placed_under_motion_frame_of_reference.set(new);
    }

    /// Its inner lookahead delegate was placed (Compose's `onNodePlaced`): it and its subtree marked placed, its
    /// lookahead place order noted, its children placed in lookahead.
    pub(crate) fn on_node_placed(&self) {
        self.on_node_placed_called.set(true);
        let node = self.node();
        let parent = node.parent();
        let detached = node.detached_from_parent_lookahead_placement.get();
        let state = self.placed_state.get();
        if (state != PlacedState::PlacedInLookahead && !detached)
            || (state != PlacedState::PlacedInApproach && detached)
        {
            self.mark_node_and_subtree_as_placed();
            if self.relayout_without_parent_in_progress.get()
                && let Some(p) = &parent
            {
                p.request_lookahead_relayout(false);
            }
        }
        match &parent {
            Some(p) => {
                if !self.relayout_without_parent_in_progress.get()
                    && matches!(p.layout_state.get(), LayoutState::LayingOut | LayoutState::LookaheadLayingOut)
                {
                    assert_eq!(
                        self.place_order.get(),
                        NOT_PLACED,
                        "kompoz: Place was called on a node which was placed already"
                    );
                    let order = p.next_child_lookahead_place_order.get();
                    self.place_order.set(order);
                    p.next_child_lookahead_place_order.set(order + 1);
                }
            }
            None => self.place_order.set(0),
        }
        self.layout_children();
    }

    fn clear_place_order(&self) {
        self.node().next_child_lookahead_place_order.set(0);
        for child in self.child_delegates() {
            child.previous_place_order.set(child.place_order.get());
            child.place_order.set(NOT_PLACED);
            if child.measured_by_parent.get() == UsageByParent::InLayoutBlock {
                child.measured_by_parent.set(UsageByParent::NotUsed);
            }
        }
    }

    fn mark_node_and_subtree_as_placed(&self) {
        let node = self.node();
        let previous = self.placed_state.get();
        self.placed_state.set(if node.detached_from_parent_lookahead_placement.get() {
            PlacedState::PlacedInApproach
        } else {
            PlacedState::PlacedInLookahead
        });
        if previous != PlacedState::PlacedInLookahead && node.lookahead_measure_pending.get() {
            node.request_lookahead_remeasure(true, true);
        }
        for child in node.children() {
            let d = child
                .lookahead_pass()
                .expect("Error: Child node's lookahead pass delegate cannot be null when in a lookahead scope.");
            if d.place_order.get() != NOT_PLACED {
                d.mark_node_and_subtree_as_placed();
                node.reschedule_remeasure_or_relayout(&child);
            }
        }
    }

    /// Children measured in its lookahead measure block and waiting to be measured again are, before it places
    /// them (Compose's `onBeforeLayoutChildren`).
    fn on_before_layout_children(&self) {
        let node = self.node();
        for child in node.children() {
            let Some(d) = child.lookahead_pass() else { continue };
            if child.lookahead_measure_pending.get()
                && d.measured_by_parent.get() == UsageByParent::InMeasureBlock
                && let Some(c) = d.last_constraints()
                && d.remeasure(c)
            {
                node.request_lookahead_remeasure(false, true);
            }
        }
    }

    /// Placed again in lookahead where it was, without its parent (Compose's `replace`).
    pub(crate) fn replace(&self) {
        self.relayout_without_parent_in_progress.set(true);
        assert!(self.placed_once.get(), "kompoz: replace() called on item that was not placed");
        self.on_node_placed_called.set(false);
        let was_placed = self.is_placed();
        let layer = self.last_layer_block.borrow().clone();
        self.place_self(self.last_position.get(), 0.0, layer);
        if was_placed
            && !self.on_node_placed_called.get()
            && let Some(p) = self.node().parent()
        {
            p.request_lookahead_relayout(false);
        }
        self.relayout_without_parent_in_progress.set(false);
    }

    pub(crate) fn on_attached_to_null_parent(&self) {
        self.placed_state.set(PlacedState::PlacedInLookahead);
    }

    // ---- Intrinsics ----

    fn on_intrinsics_queried(&self) {
        let node = self.node();
        node.request_lookahead_remeasure(false, true);
        if let Some(parent) = node.parent()
            && node.intrinsics_usage_by_parent.get() == UsageByParent::NotUsed
        {
            let usage = match parent.layout_state.get() {
                LayoutState::Measuring => UsageByParent::InMeasureBlock,
                LayoutState::LayingOut => UsageByParent::InLayoutBlock,
                _ => parent.intrinsics_usage_by_parent.get(),
            };
            node.intrinsics_usage_by_parent.set(usage);
        }
    }

    /// Its lookahead intrinsics changing measures (or lays out) the nearest parent that used them (Compose's
    /// `invalidateIntrinsicsParent`), in lookahead where that parent is in a scope.
    pub(crate) fn invalidate_intrinsics_parent(&self, force: bool) {
        let node = self.node();
        let usage = node.intrinsics_usage_by_parent.get();
        let Some(parent) = node.parent() else { return };
        if usage == UsageByParent::NotUsed {
            return;
        }
        let mut user = parent;
        while user.intrinsics_usage_by_parent.get() == usage {
            match user.parent() {
                Some(p) => user = p,
                None => break,
            }
        }
        let in_scope = user.lookahead_root.borrow().is_some();
        match usage {
            UsageByParent::InMeasureBlock if in_scope => user.request_lookahead_remeasure(force, true),
            UsageByParent::InMeasureBlock => user.request_remeasure(force),
            UsageByParent::InLayoutBlock if in_scope => user.request_lookahead_relayout(force),
            UsageByParent::InLayoutBlock => user.request_relayout_forced(force),
            UsageByParent::NotUsed => {}
        }
    }

    // ---- Parent data ----

    pub(crate) fn invalidate_parent_data(&self) {
        self.parent_data_dirty.set(true);
    }

    /// Compose's `updateParentData`: whether its parent data changed.
    pub(crate) fn update_parent_data(&self) -> bool {
        let node = self.node();
        let new = node.compute_parent_data();
        if self.parent_data.borrow().is_none() && new.is_none() {
            return false;
        }
        if !self.parent_data_dirty.get() {
            return false;
        }
        self.parent_data_dirty.set(false);
        *self.parent_data.borrow_mut() = new;
        true
    }

    // ---- Alignment lines ----

    /// Whether its lookahead alignment lines are needed (Compose's `AlignmentLines.required`).
    pub(crate) fn alignment_lines_required(&self) -> bool {
        if self.alignment_lines.queried() {
            return true;
        }
        self.node().parent().and_then(|p| p.lookahead_pass()).is_some_and(|p| p.alignment_lines_required())
    }

    /// Its lookahead lines may have changed (Compose's `onAlignmentsChanged` of LookaheadAlignmentLines): who used
    /// them measures or lays out again in lookahead, and so on up.
    pub(crate) fn on_alignments_changed(&self) {
        let lines = &self.alignment_lines;
        lines.dirty.set(true);
        let node = self.node();
        let Some(parent) = node.parent() else { return };
        let Some(parent_delegate) = parent.lookahead_pass() else { return };
        if lines.used_during_parent_measurement.get() {
            parent.request_lookahead_remeasure(false, true);
        } else if lines.previous_used_during_parent_layout.get() || lines.used_during_parent_layout.get() {
            parent.request_lookahead_relayout(false);
        }
        if lines.used_by_modifier_measurement.get() {
            node.request_lookahead_remeasure(false, true);
        }
        if lines.used_by_modifier_layout.get() {
            node.request_lookahead_relayout(false);
        }
        parent_delegate.on_alignments_changed();
    }

    /// Works its lookahead lines out again (Compose's `recalculate` of LookaheadAlignmentLines): each child placed
    /// in lookahead's, moved up by the lookahead positions of its coordinators, merged; then the lines its own
    /// measure policy gave in lookahead.
    fn recalculate_alignment_lines(&self) {
        let node = self.node();
        self.alignment_lines.clear();
        for child in node.children() {
            let Some(d) = child.lookahead_pass() else { continue };
            if d.place_order.get() == NOT_PLACED {
                continue;
            }
            if d.alignment_lines.dirty.get() {
                d.layout_children();
            }
            for (line, position) in d.alignment_lines.last_calculation() {
                self.add_alignment_line(&node, line, position, &child.inner);
            }
            let mut coordinator = child.inner.wrapped_by();
            while let Some(c) = coordinator {
                if Rc::ptr_eq(&c, &node.inner) {
                    break;
                }
                if let Some(ld) = c.lookahead_delegate() {
                    for line in ld.measure_result_lines() {
                        let position = ld.alignment_line(line);
                        self.add_alignment_line(&node, line, position, &c);
                    }
                }
                coordinator = c.wrapped_by();
            }
        }
        if let Some(inner) = node.inner.lookahead_delegate() {
            self.alignment_lines.put_all(inner.measure_result_line_values());
        }
        self.alignment_lines.dirty.set(false);
    }

    fn add_alignment_line(&self, owner: &LayoutNode, line: AlignmentLine, initial: i32, from: &Rc<NodeCoordinator>) {
        let mut position = Offset::new(initial as f32, initial as f32);
        let mut coordinator = from.clone();
        loop {
            let at = coordinator.lookahead_delegate().map_or(IntOffset::Zero, |d| d.position.get());
            position = at.to_offset() + position;
            let Some(up) = coordinator.wrapped_by() else { break };
            coordinator = up;
            if Rc::ptr_eq(&coordinator, &owner.inner) {
                break;
            }
            if let Some(ld) = coordinator.lookahead_delegate()
                && ld.provides_line(line)
            {
                let p = ld.alignment_line(line) as f32;
                position = Offset::new(p, p);
            }
        }
        let in_container = (if line.is_horizontal() { position.y } else { position.x }).round() as i32;
        self.alignment_lines.merge(line, in_container);
    }
}

impl IntrinsicMeasurable for LookaheadPassDelegate {
    fn parent_data(&self) -> Option<Rc<dyn Any>> {
        self.node().parent_data()
    }
    fn min_intrinsic_width(&self, height: i32) -> i32 {
        self.on_intrinsics_queried();
        self.outer_lookahead().min_intrinsic_width(height)
    }
    fn max_intrinsic_width(&self, height: i32) -> i32 {
        self.on_intrinsics_queried();
        self.outer_lookahead().max_intrinsic_width(height)
    }
    fn min_intrinsic_height(&self, width: i32) -> i32 {
        self.on_intrinsics_queried();
        self.outer_lookahead().min_intrinsic_height(width)
    }
    fn max_intrinsic_height(&self, width: i32) -> i32 {
        self.on_intrinsics_queried();
        self.outer_lookahead().max_intrinsic_height(width)
    }
}

impl Measurable for LookaheadPassDelegate {
    fn measure(&self, constraints: Constraints) -> Placeable {
        self.measure_for_parent(constraints)
    }
    fn retain(&self) -> Option<Rc<dyn Measurable>> {
        Some(self.rc())
    }
}

impl PlaceTarget for LookaheadPassDelegate {
    fn measured_size(&self) -> IntSize {
        self.measured_size.get()
    }
    fn measurement_constraints(&self) -> Constraints {
        self.lookahead_constraints.get().unwrap_or_default()
    }
    fn alignment_line(&self, line: AlignmentLine) -> i32 {
        let node = self.node();
        if let Some(p) = node.parent() {
            match p.layout_state.get() {
                LayoutState::LookaheadMeasuring => self.alignment_lines.used_during_parent_measurement.set(true),
                LayoutState::LookaheadLayingOut => self.alignment_lines.used_during_parent_layout.set(true),
                _ => {}
            }
        }
        self.during_alignment_lines_query.set(true);
        let v = self.outer_lookahead().alignment_line(line);
        self.during_alignment_lines_query.set(false);
        v
    }
    fn place_at(&self, position: IntOffset, z_index: f32, layer: Option<LayerBlock>) {
        self.place_self(position, z_index, layer);
    }
    fn parent_data(&self) -> Option<Rc<dyn Any>> {
        self.node().parent_data()
    }
    fn update_placed_under_motion_frame_of_reference(&self, new: bool) {
        self.update_motion_frame_of_reference(new);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The lookahead side of a coordinator

/// A coordinator in the lookahead pass (Compose's LookaheadDelegate): its lookahead measure result, size and
/// position, alignment lines and rulers — what a layout modifier's lookahead measure wraps, and what lookahead
/// coordinates read.
pub(crate) struct LookaheadDelegate {
    coordinator: Weak<NodeCoordinator>,
    this: Weak<LookaheadDelegate>,
    pub(crate) position: Cell<IntOffset>,
    measurement_constraints: Cell<Constraints>,
    measured_size: Cell<IntSize>,
    measure_result: RefCell<Option<MeasureResult>>,
    old_alignment_lines: RefCell<Option<Vec<(AlignmentLine, i32)>>>,
    /// The lines last worked out for it (Compose's `cachedAlignmentLinesMap`), which the main pass of a layout
    /// modifier's coordinator reads.
    cached_alignment_lines: RefCell<Vec<(AlignmentLine, i32)>>,
    pub(crate) placing_for_alignment: Cell<bool>,
    pub(crate) shallow_placing: Cell<bool>,
    pub(crate) placed_under_motion_frame_of_reference: Cell<bool>,
    /// Its placement scope's `with_motion_frame_of_reference_placement` flag.
    pub(crate) motion_frame_placement: Rc<Cell<bool>>,
    /// The rulers it provides in lookahead and who read them.
    pub(crate) rulers: RefCell<Option<Rc<super::rulers::CoordinatorRulers>>>,
}

impl LookaheadDelegate {
    pub(crate) fn new(coordinator: &NodeCoordinator) -> Rc<LookaheadDelegate> {
        Rc::new_cyclic(|this| LookaheadDelegate {
            coordinator: coordinator.this.clone(),
            this: this.clone(),
            position: Cell::new(IntOffset::Zero),
            measurement_constraints: Cell::new(Constraints::default()),
            measured_size: Cell::new(IntSize::Zero),
            measure_result: RefCell::new(None),
            old_alignment_lines: RefCell::new(None),
            cached_alignment_lines: RefCell::new(Vec::new()),
            placing_for_alignment: Cell::new(false),
            shallow_placing: Cell::new(false),
            placed_under_motion_frame_of_reference: Cell::new(false),
            motion_frame_placement: Rc::new(Cell::new(false)),
            rulers: RefCell::new(None),
        })
    }

    pub(crate) fn rc(&self) -> Rc<LookaheadDelegate> {
        self.this.upgrade().expect("a lookahead delegate in use is alive")
    }

    pub(crate) fn coordinator(&self) -> Rc<NodeCoordinator> {
        self.coordinator.upgrade().expect("a lookahead delegate of a coordinator that is gone")
    }

    pub(crate) fn has_measure_result(&self) -> bool {
        self.measure_result.borrow().is_some()
    }

    pub(crate) fn measure_result(&self) -> Option<MeasureResult> {
        self.measure_result.borrow().clone()
    }

    /// Its measured size (maybe outside its constraints).
    pub(crate) fn measured_size(&self) -> IntSize {
        self.measured_size.get()
    }

    /// Its width within the constraints it was measured with (Compose's Placeable `width`).
    pub(crate) fn width(&self) -> i32 {
        let c = self.measurement_constraints.get();
        self.measured_size.get().width.clamp(c.min_width, c.max_width)
    }

    pub(crate) fn height(&self) -> i32 {
        let c = self.measurement_constraints.get();
        self.measured_size.get().height.clamp(c.min_height, c.max_height)
    }

    /// Its lookahead size (Compose's LookaheadDelegate `size`: within its constraints).
    pub(crate) fn size(&self) -> IntSize {
        IntSize::new(self.width(), self.height())
    }

    /// The constraints it was measured with in lookahead.
    pub(crate) fn constraints(&self) -> Constraints {
        self.measurement_constraints.get()
    }

    fn apparent_to_real_offset(&self) -> IntOffset {
        let m = self.measured_size.get();
        IntOffset::new((self.width() - m.width) / 2, (self.height() - m.height) / 2)
    }

    /// Its lookahead coordinates (Compose's `lookaheadLayoutCoordinates`).
    pub(crate) fn coordinates(&self) -> LayoutCoordinates {
        LayoutCoordinates::lookahead_of(self.coordinator())
    }

    /// The lookahead delegate of the coordinator wrapping it (Compose's LookaheadDelegate `parent`).
    pub(crate) fn parent(&self) -> Option<Rc<LookaheadDelegate>> {
        self.coordinator().wrapped_by().and_then(|w| w.lookahead_delegate())
    }

    /// Measures it in lookahead (Compose's `measure` of its two kinds: a layout modifier with what it wraps in
    /// lookahead, the measure policy with the children's lookahead pass delegates).
    pub(crate) fn measure_in_lookahead(&self, constraints: Constraints) -> Placeable {
        self.measurement_constraints.set(constraints);
        let result = self.coordinator().measure_result_in_lookahead(constraints);
        self.set_measure_result(result);
        Placeable::new(self.rc())
    }

    /// Compose's `_measureResult` setter: its size, and new lines tell the node's lookahead alignment lines.
    fn set_measure_result(&self, result: MeasureResult) {
        self.measured_size.set(IntSize::new(result.width, result.height));
        let changed = self.measure_result.borrow().as_ref().is_none_or(|old| !old.same_result(&result));
        if changed {
            let old_lines = self.old_alignment_lines.borrow().clone();
            let differs = match &old_lines {
                Some(old) => !old.is_empty() || !result.alignment_lines().is_empty(),
                None => !result.alignment_lines().is_empty(),
            } && old_lines.as_deref() != Some(result.alignment_lines());
            if differs {
                if let Some(d) = self.coordinator().layout_node().and_then(|n| n.lookahead_pass()) {
                    d.on_alignments_changed();
                }
                *self.old_alignment_lines.borrow_mut() = Some(result.alignment_lines().to_vec());
            }
        }
        *self.measure_result.borrow_mut() = Some(result);
    }

    pub(crate) fn measure_result_lines(&self) -> Vec<AlignmentLine> {
        self.measure_result
            .borrow()
            .as_ref()
            .map_or_else(Vec::new, |r| r.alignment_lines().iter().map(|l| l.0).collect())
    }

    pub(crate) fn measure_result_line_values(&self) -> Vec<(AlignmentLine, i32)> {
        self.measure_result.borrow().as_ref().map_or_else(Vec::new, |r| r.alignment_lines().to_vec())
    }

    pub(crate) fn provides_line(&self, line: AlignmentLine) -> bool {
        self.measure_result.borrow().as_ref().is_some_and(|r| r.alignment_line(line).is_some())
    }

    // ---- Placing ----

    /// Places it in lookahead (Compose's LookaheadDelegate `placeAt`): itself, then (unless placing only itself) what
    /// it places.
    fn place_at_position(&self, position: IntOffset) {
        self.place_self(position);
        if self.shallow_placing.get() {
            return;
        }
        let coordinator = self.coordinator();
        if coordinator.is_inner() {
            // (Compose's LookaheadDelegateImpl `placeChildren`)
            if let Some(d) = coordinator.layout_node().and_then(|n| n.lookahead_pass()) {
                d.on_node_placed();
            }
        } else if let Some(result) = self.measure_result() {
            let wrapped = coordinator.wrapped().and_then(|w| w.lookahead_delegate());
            let was = wrapped.as_ref().map(|w| w.placing_for_alignment.replace(self.placing_for_alignment.get()));
            result.place_children(&coordinator.lookahead_placement_scope());
            if let (Some(w), Some(was)) = (wrapped, was) {
                w.placing_for_alignment.set(was);
            }
        }
    }

    fn place_self(&self, position: IntOffset) {
        let coordinator = self.coordinator();
        if self.position.get() != position {
            self.position.set(position);
            if let Some(n) = coordinator.layout_node() {
                if let Some(d) = n.lookahead_pass() {
                    d.notify_children_using_lookahead_coordinates_while_placing();
                }
                self.invalidate_alignment_lines_from_position_change(&coordinator, &n);
            }
        }
        if !self.placing_for_alignment.get() {
            let result = self.measure_result();
            coordinator.capture_rulers_if_needed(result.as_ref(), true);
        }
    }

    /// Moves it alone, the apparent to real offset added (Compose's `placeSelfApparentToRealOffset`).
    pub(crate) fn place_self_apparent_to_real_offset(&self, position: IntOffset) {
        self.place_self(position + self.apparent_to_real_offset());
    }

    /// Placed again where it was (Compose's LookaheadDelegate `replace`).
    pub(crate) fn replace(&self) {
        self.place_at_position(self.position.get());
    }

    /// Compose's `invalidateAlignmentLinesFromPositionChange`, in lookahead.
    fn invalidate_alignment_lines_from_position_change(&self, coordinator: &NodeCoordinator, node: &Rc<LayoutNode>) {
        let wrapped_same_node =
            coordinator.wrapped().and_then(|w| w.layout_node()).is_some_and(|w| Rc::ptr_eq(&w, node));
        if !wrapped_same_node {
            if let Some(d) = node.lookahead_pass() {
                d.on_alignments_changed();
            }
        } else if let Some(d) = node.parent().and_then(|p| p.lookahead_pass()) {
            d.on_alignments_changed();
        }
    }

    /// Placed under a motion frame of reference or not (Compose's LookaheadCapablePlaceable
    /// `updatePlacedUnderMotionFrameOfReference`): a coordinator inside its node always; its node's outer one only
    /// when its parent places it (a node placed again on its own keeps what its parent's placement said).
    fn update_motion_frame_of_reference(&self, new: bool) {
        let coordinator = self.coordinator();
        let node = coordinator.layout_node();
        let parent_node = self.parent().and_then(|p| p.coordinator().layout_node());
        let same = match (&parent_node, &node) {
            (Some(p), Some(n)) => Rc::ptr_eq(p, n),
            _ => false,
        };
        if same
            || parent_node.is_some_and(|p| {
                matches!(p.layout_state.get(), LayoutState::LayingOut | LayoutState::LookaheadLayingOut)
            })
        {
            self.placed_under_motion_frame_of_reference.set(new);
        }
    }

    // ---- Alignment lines ----

    /// Where `line` is in it in lookahead, from its real top left (Compose's LookaheadCapablePlaceable `get`).
    pub(crate) fn alignment_line(&self, line: AlignmentLine) -> i32 {
        if !self.has_measure_result() {
            return AlignmentLine::Unspecified;
        }
        let measured = self.calculate_alignment_line(line);
        if measured == AlignmentLine::Unspecified {
            return measured;
        }
        let offset = self.apparent_to_real_offset();
        measured + if line.is_horizontal() { offset.y } else { offset.x }
    }

    /// Compose's `calculateAlignmentLine` of its two kinds, cached.
    pub(crate) fn calculate_alignment_line(&self, line: AlignmentLine) -> i32 {
        let coordinator = self.coordinator();
        let value = if coordinator.is_inner() {
            let lines =
                coordinator.layout_node().and_then(|n| n.lookahead_pass()).map(|d| d.calculate_alignment_lines());
            lines
                .and_then(|l| l.iter().find(|(l2, _)| *l2 == line).map(|(_, v)| *v))
                .unwrap_or(AlignmentLine::Unspecified)
        } else {
            self.calculate_alignment_and_place_child_as_needed(&coordinator, line)
        };
        let mut cache = self.cached_alignment_lines.borrow_mut();
        match cache.iter_mut().find(|(l, _)| *l == line) {
            Some((_, v)) => *v = value,
            None => cache.push((line, value)),
        }
        value
    }

    /// The line last worked out for it, or [`AlignmentLine::Unspecified`] (Compose's `getCachedAlignmentLine`).
    pub(crate) fn cached_alignment_line(&self, line: AlignmentLine) -> i32 {
        self.cached_alignment_lines
            .borrow()
            .iter()
            .find(|(l, _)| *l == line)
            .map_or(AlignmentLine::Unspecified, |(_, v)| *v)
    }

    /// Compose's `calculateAlignmentAndPlaceChildAsNeeded`, in lookahead.
    fn calculate_alignment_and_place_child_as_needed(&self, coordinator: &NodeCoordinator, line: AlignmentLine) -> i32 {
        if let Some(v) = self.measure_result.borrow().as_ref().and_then(|r| r.alignment_line(line)) {
            return v;
        }
        let child = coordinator.wrapped().and_then(|w| w.lookahead_delegate());
        let child =
            child.expect("kompoz: Child of a lookahead delegate cannot be null when calculating alignment line");
        let in_wrapped = child.alignment_line(line);
        if in_wrapped == AlignmentLine::Unspecified {
            return AlignmentLine::Unspecified;
        }
        let was_shallow = child.shallow_placing.replace(true);
        let was_for_alignment = self.placing_for_alignment.replace(true);
        self.replace();
        child.shallow_placing.set(was_shallow);
        self.placing_for_alignment.set(was_for_alignment);
        let p = child.position.get();
        in_wrapped + if line.is_horizontal() { p.y } else { p.x }
    }

    // ---- Positions ----

    /// Its position in `ancestor` in lookahead, the offsets of what was placed under a motion frame of reference
    /// left out if `excluding_agnostic_offset` (Compose's `positionIn`).
    pub(crate) fn position_in(&self, ancestor: &LookaheadDelegate, excluding_agnostic_offset: bool) -> IntOffset {
        let mut aggregated = IntOffset::Zero;
        let mut delegate = self.rc();
        while !std::ptr::eq(&*delegate, ancestor) {
            if !delegate.placed_under_motion_frame_of_reference.get() || !excluding_agnostic_offset {
                aggregated = aggregated + delegate.position.get();
            }
            delegate = delegate
                .coordinator()
                .wrapped_by()
                .and_then(|w| w.lookahead_delegate())
                .expect("kompoz: lookahead delegates of one lookahead tree");
        }
        aggregated
    }

    /// The lookahead delegate at the top of its lookahead tree (Compose's `rootLookaheadDelegate`).
    pub(crate) fn root_lookahead_delegate(&self) -> Rc<LookaheadDelegate> {
        let mut root = self.coordinator().layout_node().expect("a coordinator of a layout node that is gone");
        loop {
            let Some(parent) = root.parent() else { break };
            let Some(parent_root) = parent.lookahead_root.borrow().clone() else { break };
            root = match parent_root {
                LookaheadRoot::Virtual(_) => parent,
                LookaheadRoot::Node(n) => n.upgrade().expect("a lookahead root that is gone"),
            };
        }
        root.outer_coordinator().lookahead_delegate().expect("kompoz: a lookahead root without a lookahead delegate")
    }

    // ---- Intrinsics ----

    pub(crate) fn min_intrinsic_width(&self, height: i32) -> i32 {
        self.coordinator().lookahead_intrinsic(crate::layout::MinMax::Min, crate::layout::Side::Width, height)
    }
    pub(crate) fn max_intrinsic_width(&self, height: i32) -> i32 {
        self.coordinator().lookahead_intrinsic(crate::layout::MinMax::Max, crate::layout::Side::Width, height)
    }
    pub(crate) fn min_intrinsic_height(&self, width: i32) -> i32 {
        self.coordinator().lookahead_intrinsic(crate::layout::MinMax::Min, crate::layout::Side::Height, width)
    }
    pub(crate) fn max_intrinsic_height(&self, width: i32) -> i32 {
        self.coordinator().lookahead_intrinsic(crate::layout::MinMax::Max, crate::layout::Side::Height, width)
    }
}

impl IntrinsicMeasurable for LookaheadDelegate {
    fn parent_data(&self) -> Option<Rc<dyn Any>> {
        self.coordinator().parent_data()
    }
    fn min_intrinsic_width(&self, height: i32) -> i32 {
        LookaheadDelegate::min_intrinsic_width(self, height)
    }
    fn max_intrinsic_width(&self, height: i32) -> i32 {
        LookaheadDelegate::max_intrinsic_width(self, height)
    }
    fn min_intrinsic_height(&self, width: i32) -> i32 {
        LookaheadDelegate::min_intrinsic_height(self, width)
    }
    fn max_intrinsic_height(&self, width: i32) -> i32 {
        LookaheadDelegate::max_intrinsic_height(self, width)
    }
}

impl Measurable for LookaheadDelegate {
    fn measure(&self, constraints: Constraints) -> Placeable {
        self.measure_in_lookahead(constraints)
    }
    fn retain(&self) -> Option<Rc<dyn Measurable>> {
        Some(self.rc())
    }
}

impl PlaceTarget for LookaheadDelegate {
    fn measured_size(&self) -> IntSize {
        self.measured_size.get()
    }
    fn measurement_constraints(&self) -> Constraints {
        self.measurement_constraints.get()
    }
    fn alignment_line(&self, line: AlignmentLine) -> i32 {
        LookaheadDelegate::alignment_line(self, line)
    }
    fn place_at(&self, position: IntOffset, _z_index: f32, _layer: Option<LayerBlock>) {
        self.place_at_position(position);
    }
    fn parent_data(&self) -> Option<Rc<dyn Any>> {
        self.coordinator().parent_data()
    }
    fn update_placed_under_motion_frame_of_reference(&self, new: bool) {
        self.update_motion_frame_of_reference(new);
    }
}

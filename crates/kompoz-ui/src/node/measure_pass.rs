//! A layout node measured and placed by its parent (Compose's MeasurePassDelegate): measured again only when
//! asked to or with other constraints, placed through its coordinators, its placement block run again when only
//! its placement changes, its children's place order kept for drawing, its alignment lines worked out.

use super::alignment_lines::NOT_PLACED;
use super::lookahead::{LookaheadPassDelegate, LookaheadRoot};
use super::{LayoutNode, LayoutState, UsageByParent};
use crate::layout::{
    AlignmentLine, IntrinsicMeasurable, LayerBlock, Measurable, MeasurePolicy, MinMax, PlaceTarget, Placeable, Side,
};
use crate::unit::Constraints;
use kompoz_graphics::{IntOffset, IntSize};
use std::any::Any;
use std::rc::Rc;

impl LayoutNode {
    /// Measures it within `constraints` for its parent (Compose's `MeasurePassDelegate.measure`): a lookahead root
    /// in lookahead first, then (all) in the main pass.
    fn measure_for_parent(&self, constraints: Constraints) -> Placeable {
        if self.intrinsics_usage_by_parent.get() == UsageByParent::NotUsed {
            self.clear_subtree_intrinsics_usage();
        }
        if self.is_out_most_lookahead_root() {
            let d = self.lookahead_pass_delegate();
            d.measured_by_parent.set(UsageByParent::NotUsed);
            d.measure(constraints);
        }
        self.track_measurement_by_parent();
        self.remeasure(constraints);
        Placeable::new(self.rc())
    }

    fn track_measurement_by_parent(&self) {
        match self.parent() {
            Some(parent) => {
                assert!(
                    self.measured_by_parent.get() == UsageByParent::NotUsed || self.can_multi_measure.get(),
                    "kompoz: measure() may not be called multiple times on the same Measurable. If you want to get \
                     the content size of the Measurable before calculating the final constraints, please use \
                     intrinsic measurements."
                );
                let usage = match parent.layout_state.get() {
                    LayoutState::Measuring => UsageByParent::InMeasureBlock,
                    LayoutState::LayingOut => UsageByParent::InLayoutBlock,
                    _ => {
                        panic!("kompoz: a Measurable can only be measured from its parent's measure or placement block")
                    }
                };
                self.measured_by_parent.set(usage);
            }
            None => self.measured_by_parent.set(UsageByParent::NotUsed),
        }
    }

    /// Measures it again if it is pending or the constraints changed, else measures what in it waits to be
    /// (Compose's `remeasure`). Whether its size changed.
    pub(crate) fn remeasure(&self, constraints: Constraints) -> bool {
        if let Some(p) = self.parent() {
            self.can_multi_measure.set(self.can_multi_measure.get() || p.can_multi_measure.get());
        }
        if self.measure_pending.get() || self.last_constraints.get() != Some(constraints) {
            self.alignment_lines.used_by_modifier_measurement.set(false);
            for child in self.children() {
                child.alignment_lines.used_during_parent_measurement.set(false);
            }
            let outer = self.outer_coordinator();
            let previous = outer.measured_size.get();
            let previous_measured = self.measured_size.get();
            self.last_constraints.set(Some(constraints));
            self.perform_measure(constraints);
            let outer = self.outer_coordinator();
            let size = outer.measured_size.get();
            let constrained = constraints.constrain_size(size);
            let changed = size != previous || constrained != constraints.constrain_size(previous_measured);
            self.measured_size.set(constrained);
            return changed;
        }
        if let Some(owner) = self.owner() {
            owner.force_measure_the_subtree(&self.rc(), false);
        }
        self.reset_subtree_intrinsics_usage();
        false
    }

    /// Measures it again with the constraints it was last measured with (Compose's `LayoutNode.remeasure()`).
    pub(crate) fn remeasure_with_last_constraints(&self) -> bool {
        match self.last_constraints.get() {
            Some(c) => {
                if self.intrinsics_usage_by_parent.get() == UsageByParent::NotUsed {
                    self.clear_subtree_intrinsics_usage();
                }
                self.remeasure(c)
            }
            None => false,
        }
    }

    /// Compose's `performMeasure`: through its coordinators, what it reads observed (a change measures it again).
    fn perform_measure(&self, constraints: Constraints) {
        assert_eq!(self.layout_state.get(), LayoutState::Idle, "kompoz: a layout measured while it is busy");
        self.layout_state.set(LayoutState::Measuring);
        self.measure_pending.set(false);
        let outer = self.outer_coordinator();
        match self.owner() {
            Some(owner) => {
                let scope = self.reader_scope(&self.measure_reads, |n| n.request_remeasure(false));
                owner.snapshot_observer().observe(&scope, || outer.perform_measure(constraints));
            }
            None => outer.perform_measure(constraints),
        }
        if self.layout_state.get() == LayoutState::Measuring {
            self.mark_layout_pending();
            self.layout_state.set(LayoutState::Idle);
        }
    }

    /// One of its reader scopes, made when first needed: `on_changed` is called on it when what it read changes.
    pub(crate) fn reader_scope(
        &self,
        slot: &std::cell::RefCell<Option<Rc<super::ReaderScope>>>,
        on_changed: impl Fn(&LayoutNode) + 'static,
    ) -> Rc<super::ReaderScope> {
        if let Some(s) = slot.borrow().as_ref() {
            return s.clone();
        }
        let owner = self.owner().expect("reader scopes are a host's");
        let weak = self.this.clone();
        let scope = owner.snapshot_observer().scope(move || {
            if let Some(n) = weak.upgrade()
                && n.is_attached()
            {
                on_changed(&n);
            }
        });
        *slot.borrow_mut() = Some(scope.clone());
        scope
    }

    pub(crate) fn mark_layout_pending(&self) {
        self.layout_pending.set(true);
        self.layout_pending_for_alignment.set(true);
    }

    pub(crate) fn mark_measure_pending(&self) {
        self.measure_pending.set(true);
    }

    // ---- Intrinsics ----

    fn on_intrinsics_queried(&self) {
        // As Compose's: the child measured again, and how its parent used its intrinsics noted.
        self.request_remeasure(false);
        if let Some(parent) = self.parent()
            && self.intrinsics_usage_by_parent.get() == UsageByParent::NotUsed
        {
            let usage = match parent.layout_state.get() {
                LayoutState::Measuring => UsageByParent::InMeasureBlock,
                LayoutState::LayingOut => UsageByParent::InLayoutBlock,
                _ => parent.intrinsics_usage_by_parent.get(),
            };
            self.intrinsics_usage_by_parent.set(usage);
        }
    }

    /// The node's own intrinsics, from its measure policy and children (Compose's IntrinsicsPolicy).
    pub(crate) fn policy_intrinsic(&self, min_max: MinMax, side: Side, size: i32) -> i32 {
        let children = self.children();
        let refs: Vec<&dyn IntrinsicMeasurable> = children.iter().map(|c| &**c as &dyn IntrinsicMeasurable).collect();
        Self::policy_intrinsic_of(&*self.measure_policy(), &self.inner.measure_scope(), &refs, min_max, side, size)
    }

    /// The same in lookahead (Compose's IntrinsicsPolicy `min/maxLookaheadIntrinsic…`): with its children's
    /// lookahead pass delegates. The scope is the main one, as Compose's (its outer coordinator).
    pub(crate) fn lookahead_policy_intrinsic(&self, min_max: MinMax, side: Side, size: i32) -> i32 {
        let children: Vec<Rc<LookaheadPassDelegate>> =
            self.children().iter().map(|c| c.lookahead_pass_delegate()).collect();
        let refs: Vec<&dyn IntrinsicMeasurable> = children.iter().map(|c| &**c as &dyn IntrinsicMeasurable).collect();
        let scope = self.outer_coordinator().measure_scope();
        Self::policy_intrinsic_of(&*self.measure_policy(), &scope, &refs, min_max, side, size)
    }

    fn policy_intrinsic_of(
        policy: &dyn MeasurePolicy,
        scope: &crate::layout::IntrinsicMeasureScope,
        refs: &[&dyn IntrinsicMeasurable],
        min_max: MinMax,
        side: Side,
        size: i32,
    ) -> i32 {
        match (min_max, side) {
            (MinMax::Min, Side::Width) => policy.min_intrinsic_width(scope, refs, size),
            (MinMax::Max, Side::Width) => policy.max_intrinsic_width(scope, refs, size),
            (MinMax::Min, Side::Height) => policy.min_intrinsic_height(scope, refs, size),
            (MinMax::Max, Side::Height) => policy.max_intrinsic_height(scope, refs, size),
        }
    }

    /// Its intrinsics changing measures (or lays out) the nearest parent that used them (Compose's
    /// `invalidateIntrinsicsParent`).
    pub(crate) fn invalidate_intrinsics_parent(&self, force: bool) {
        let usage = self.intrinsics_usage_by_parent.get();
        let Some(parent) = self.parent() else { return };
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
        match usage {
            UsageByParent::InMeasureBlock => user.request_remeasure(force),
            UsageByParent::InLayoutBlock => user.request_relayout_forced(force),
            UsageByParent::NotUsed => {}
        }
    }

    pub(crate) fn clear_subtree_intrinsics_usage(&self) {
        self.previous_intrinsics_usage_by_parent.set(self.intrinsics_usage_by_parent.get());
        self.intrinsics_usage_by_parent.set(UsageByParent::NotUsed);
        for child in self.children() {
            if child.intrinsics_usage_by_parent.get() != UsageByParent::NotUsed {
                child.clear_subtree_intrinsics_usage();
            }
        }
    }

    pub(crate) fn reset_subtree_intrinsics_usage(&self) {
        for child in self.children() {
            child.intrinsics_usage_by_parent.set(child.previous_intrinsics_usage_by_parent.get());
            if child.intrinsics_usage_by_parent.get() != UsageByParent::NotUsed {
                child.reset_subtree_intrinsics_usage();
            }
        }
    }

    // ---- Placing ----

    /// Placed by its parent (Compose's `placeSelf`): a node whose lookahead its approach places (a lookahead
    /// root, or one its parent's lookahead didn't place) placed in lookahead first, then in the main pass.
    fn place_for_parent(&self, position: IntOffset, z_index: f32, layer: Option<LayerBlock>) {
        self.is_placed_by_parent.set(true);
        let same_layer = match (&*self.last_layer_block.borrow(), &layer) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        };
        if (position != self.last_position.get() || !same_layer || self.needs_coordinates_update.get())
            && (self.coordinates_accessed_during_modifier_placement.get()
                || self.coordinates_accessed_during_placement.get()
                || self.needs_coordinates_update.get())
        {
            self.layout_pending.set(true);
            self.needs_coordinates_update.set(false);
        }
        self.place_in_lookahead_first(position);
        self.place_outer_coordinator(position, z_index, layer);
    }

    /// The lookahead placement its approach placement does first (Compose's `placeSelf`, before
    /// `placeOuterCoordinator`).
    fn place_in_lookahead_first(&self, position: IntOffset) {
        let Some(d) = self.lookahead_pass() else { return };
        d.on_approach_placement();
        if d.needs_to_be_placed_in_approach() {
            let scope = match self.outer_coordinator().wrapped_by() {
                Some(p) => p.placement_scope(),
                None => match self.owner() {
                    Some(o) => o.root_placement_scope(),
                    None => crate::layout::PlacementScope::new(0, self.layout_direction()),
                },
            };
            // (the root of its lookahead tree: no parent resets the place order for it)
            if let Some(p) = self.parent() {
                p.next_child_lookahead_place_order.set(0);
            }
            d.place_order.set(NOT_PLACED);
            scope.place(&Placeable::new(d.clone()), position.x, position.y);
        }
        assert!(d.placed_once.get(), "kompoz: Error: Placement happened before lookahead.");
    }

    /// Compose's `placeOuterCoordinator`: only moved when nothing in it changed; else placed through its
    /// coordinators, what its layout modifiers read while placing observed.
    pub(crate) fn place_outer_coordinator(&self, position: IntOffset, z_index: f32, layer: Option<LayerBlock>) {
        self.layout_state.set(LayoutState::LayingOut);
        self.last_position.set(position);
        self.last_z_index.set(z_index);
        *self.last_layer_block.borrow_mut() = layer.clone();
        self.on_node_placed_called.set(false);
        let outer = self.outer_coordinator();
        let at = position + outer.apparent_to_real_offset();
        if !self.layout_pending.get() && self.is_placed.get() {
            outer.place_self_only(at, z_index, layer);
            self.on_node_placed();
        } else {
            self.alignment_lines.used_by_modifier_layout.set(false);
            self.set_coordinates_accessed(false, false);
            match self.owner() {
                Some(owner) => {
                    let scope = self.reader_scope(&self.layout_modifier_reads, |n| n.request_relayout());
                    owner.snapshot_observer().observe(&scope, || outer.place_at(at, z_index, layer));
                }
                None => outer.place_at(at, z_index, layer),
            }
        }
        self.layout_state.set(LayoutState::Idle);
        if outer.placing_for_alignment.get()
            && (self.coordinates_accessed_during_modifier_placement.get()
                || self.coordinates_accessed_during_placement.get())
        {
            self.request_relayout();
        }
        self.placed_once.set(true);
    }

    /// Placed again where it was, without its parent (Compose's `replace`).
    pub(crate) fn replace(&self) {
        assert!(self.placed_once.get(), "kompoz: replace called on a node never placed");
        self.relayout_without_parent_in_progress.set(true);
        let was_placed = self.is_placed.get();
        let layer = self.last_layer_block.borrow().clone();
        self.place_outer_coordinator(self.last_position.get(), self.last_z_index.get(), layer);
        if was_placed
            && !self.on_node_placed_called.get()
            && let Some(p) = self.parent()
        {
            p.request_relayout();
        }
        self.relayout_without_parent_in_progress.set(false);
    }

    /// Placed at the top left, the root's (Compose's `LayoutNode.place`).
    pub(crate) fn place_root(&self) {
        self.place_in_lookahead_first(IntOffset::Zero);
        self.place_outer_coordinator(IntOffset::Zero, 0.0, None);
    }

    /// Its inner coordinator was placed (Compose's `onNodePlaced`): its z-index and place order noted, it and
    /// its subtree marked placed, its children placed.
    pub(crate) fn on_node_placed(&self) {
        self.on_node_placed_called.set(true);
        let parent = self.parent();
        let mut z = self.inner.z_index.get();
        for c in self.coordinators() {
            z += c.z_index.get();
        }
        if z != self.z_index.get() {
            self.z_index.set(z);
            if let Some(p) = &parent {
                p.on_z_sorted_children_invalidated();
                p.invalidate_layer();
            }
        }
        if !self.inner.placing_for_alignment.get() {
            let was_placed = self.is_placed.get();
            if !was_placed || self.alignment_lines.queried() {
                self.mark_node_and_subtree_as_placed();
            }
            if !was_placed {
                if let Some(p) = &parent {
                    p.invalidate_layer();
                    if self.relayout_without_parent_in_progress.get() {
                        p.request_relayout();
                    }
                }
            } else {
                self.inner.on_placed();
            }
        }
        match &parent {
            Some(p) => {
                if !self.relayout_without_parent_in_progress.get() && p.layout_state.get() == LayoutState::LayingOut {
                    assert_eq!(self.place_order.get(), NOT_PLACED, "kompoz: place was called on a node placed already");
                    let order = p.next_child_place_order.get();
                    self.place_order.set(order);
                    p.next_child_place_order.set(order + 1);
                }
            }
            None => self.place_order.set(0),
        }
        self.layout_children();
    }

    fn mark_node_and_subtree_as_placed(&self) {
        let was_placed = self.is_placed.replace(true);
        if !was_placed {
            self.inner.on_placed();
            if self.measure_pending.get() {
                self.request_remeasure(true);
            } else if self.lookahead_measure_pending.get() {
                self.request_lookahead_remeasure(true, true);
            }
            if let Some(owner) = self.owner() {
                owner.semantics_changed();
            }
        }
        for child in self.children() {
            if child.place_order.get() != NOT_PLACED {
                child.mark_node_and_subtree_as_placed();
                self.reschedule_remeasure_or_relayout(&child);
            }
        }
    }

    pub(crate) fn mark_subtree_as_not_placed(&self) {
        if self.is_placed.replace(false) {
            let mut c = Some(self.outer_coordinator());
            while let Some(x) = c {
                x.on_unplaced();
                x.release_layer();
                c = x.wrapped();
            }
            if let Some(owner) = self.owner() {
                owner.semantics_changed();
            }
            for child in self.children() {
                child.mark_subtree_as_not_placed();
            }
        }
    }

    /// A child scheduled to be measured or placed again while it wasn't placed is scheduled again (requests for nodes
    /// not placed are dropped: Compose's `rescheduleRemeasureOrRelayout`).
    pub(crate) fn reschedule_remeasure_or_relayout(&self, child: &LayoutNode) {
        if child.layout_state.get() == LayoutState::Idle {
            if child.lookahead_measure_pending.get() {
                child.request_lookahead_remeasure(true, true);
            } else {
                if child.lookahead_layout_pending.get() {
                    child.request_lookahead_relayout(true);
                }
                if child.measure_pending.get() {
                    child.request_remeasure(true);
                } else if child.layout_pending.get() {
                    child.request_relayout_forced(true);
                }
            }
        }
    }

    /// Places its children if its placement is pending, observing what the placement block reads, and works its
    /// alignment lines out if needed (Compose's `layoutChildren`).
    pub(crate) fn layout_children(&self) {
        self.laying_out_children.set(true);
        if self.layout_pending.get() {
            self.on_before_layout_children();
        }
        if self.layout_pending_for_alignment.get()
            || (!self.during_alignment_lines_query.get()
                && !self.inner.placing_for_alignment.get()
                && self.layout_pending.get())
        {
            self.layout_pending.set(false);
            let old = self.layout_state.replace(LayoutState::LayingOut);
            self.set_coordinates_accessed(true, false);
            match self.owner() {
                Some(owner) => {
                    let scope = self.reader_scope(&self.layout_reads, |n| n.request_relayout());
                    owner.snapshot_observer().observe(&scope, || self.layout_children_block());
                }
                None => self.layout_children_block(),
            }
            self.layout_state.set(old);
            self.layout_pending_for_alignment.set(false);
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
        self.clear_place_order();
        let children = self.children();
        for c in &children {
            c.alignment_lines.used_during_parent_layout.set(false);
        }
        let for_alignment = self.inner.placing_for_alignment.get();
        let were: Vec<bool> =
            children.iter().map(|c| c.outer_coordinator().placing_for_alignment.replace(for_alignment)).collect();
        if let Some(result) = self.inner.measure_result() {
            result.place_children(&self.inner.placement_scope());
        }
        for (c, was) in children.iter().zip(were) {
            c.outer_coordinator().placing_for_alignment.set(was);
        }
        self.check_children_place_order_for_updates();
        for c in &children {
            c.alignment_lines.previous_used_during_parent_layout.set(c.alignment_lines.used_during_parent_layout.get());
        }
    }

    fn clear_place_order(&self) {
        self.next_child_place_order.set(0);
        for child in self.children() {
            child.previous_place_order.set(child.place_order.get());
            child.place_order.set(NOT_PLACED);
            child.is_placed_by_parent.set(false);
            if child.measured_by_parent.get() == UsageByParent::InLayoutBlock {
                child.measured_by_parent.set(UsageByParent::NotUsed);
            }
        }
    }

    fn check_children_place_order_for_updates(&self) {
        for child in self.children() {
            if child.previous_place_order.get() != child.place_order.get() {
                self.on_z_sorted_children_invalidated();
                self.invalidate_layer();
                if child.place_order.get() == NOT_PLACED {
                    // (a child placed in lookahead by its approach isn't placed there either)
                    if (child.detached_from_parent_lookahead_placement.get() || child.is_out_most_lookahead_root())
                        && let Some(d) = child.lookahead_pass()
                    {
                        d.mark_node_and_subtree_as_not_placed(false);
                    }
                    child.mark_subtree_as_not_placed();
                }
            }
        }
    }

    /// Children measured in its measure block and waiting to be measured again are, before it places them
    /// (Compose's `onBeforeLayoutChildren`).
    fn on_before_layout_children(&self) {
        for child in self.children() {
            if child.measure_pending.get()
                && child.measured_by_parent.get() == UsageByParent::InMeasureBlock
                && child.remeasure_with_last_constraints()
            {
                self.request_remeasure(false);
            }
        }
    }

    /// Its alignment lines, worked out by placing its children for alignment (Compose's
    /// `calculateAlignmentLines`).
    pub(crate) fn calculate_alignment_lines(&self) {
        if !self.during_alignment_lines_query.get() {
            if self.layout_state.get() == LayoutState::Measuring {
                self.alignment_lines.used_by_modifier_measurement.set(true);
                if self.alignment_lines.dirty.get() {
                    self.mark_layout_pending();
                }
            } else {
                self.alignment_lines.used_by_modifier_layout.set(true);
            }
        }
        let was = self.inner.placing_for_alignment.replace(true);
        self.layout_children();
        self.inner.placing_for_alignment.set(was);
    }

    // ---- Coordinates read while placing ----

    /// Its coordinates were read (Compose's `LayoutNodeLayoutDelegate.onCoordinatesUsed`): read while it places
    /// its children or its modifiers place, it is placed again when it moves (in lookahead too, read there).
    pub(crate) fn on_coordinates_used(&self) {
        let state = self.layout_state.get();
        if state == LayoutState::LayingOut || state == LayoutState::LookaheadLayingOut {
            // (read placing its children, or as its modifiers place: each flag cleared as that placement runs again)
            self.set_coordinates_accessed(self.laying_out_children.get(), true);
        }
        if state == LayoutState::LookaheadLayingOut {
            let laying_out = self.lookahead_pass().is_some_and(|d| d.laying_out_children.get());
            self.set_lookahead_coordinates_accessed(laying_out, true);
        }
    }

    /// Sets one of the two "lookahead coordinates read while placing" flags, keeping the parent's count of such
    /// children (Compose's `lookaheadCoordinatesAccessedDuring(Modifier)Placement` setters).
    pub(crate) fn set_lookahead_coordinates_accessed(&self, during_placement: bool, value: bool) {
        let before = self.lookahead_coordinates_accessed_during_placement.get()
            || self.lookahead_coordinates_accessed_during_modifier_placement.get();
        if during_placement {
            self.lookahead_coordinates_accessed_during_placement.set(value);
        } else {
            self.lookahead_coordinates_accessed_during_modifier_placement.set(value);
        }
        let after = self.lookahead_coordinates_accessed_during_placement.get()
            || self.lookahead_coordinates_accessed_during_modifier_placement.get();
        if before != after {
            self.add_children_accessing_lookahead_coordinates(if after { 1 } else { -1 });
        }
    }

    fn add_children_accessing_lookahead_coordinates(&self, delta: i32) {
        let before = self.children_accessing_lookahead_coordinates_during_placement.get();
        let after = before + delta;
        self.children_accessing_lookahead_coordinates_during_placement.set(after);
        if (before == 0) != (after == 0)
            && let Some(p) = self.parent()
        {
            p.add_children_accessing_lookahead_coordinates(if after == 0 { -1 } else { 1 });
        }
    }

    pub(crate) fn on_coordinates_accessed_in_placement(&self, _coordinator: &super::NodeCoordinator) {
        self.on_coordinates_used();
    }

    /// Sets one of the two "coordinates read while placing" flags, keeping the parent's count of such children.
    fn set_coordinates_accessed(&self, during_placement: bool, value: bool) {
        let before = self.coordinates_accessed_during_placement.get()
            || self.coordinates_accessed_during_modifier_placement.get();
        if during_placement {
            self.coordinates_accessed_during_placement.set(value);
        } else {
            self.coordinates_accessed_during_modifier_placement.set(value);
        }
        let after = self.coordinates_accessed_during_placement.get()
            || self.coordinates_accessed_during_modifier_placement.get();
        if before != after
            && let Some(p) = self.parent()
        {
            let n = p.children_accessing_coordinates_during_placement.get();
            p.children_accessing_coordinates_during_placement.set(if after { n + 1 } else { n - 1 });
        }
    }

    /// A child that read its coordinates while placing is placed again (Compose's
    /// `requestLayoutIfCoordinatesAreUsedAndNotifyChildren`).
    pub(crate) fn request_layout_if_coordinates_are_used_and_notify_children(&self) {
        if !self.is_placed.get() || self.children_accessing_coordinates_during_placement.get() <= 0 {
            return;
        }
        for child in self.children() {
            let accessed = child.coordinates_accessed_during_placement.get()
                || child.coordinates_accessed_during_modifier_placement.get();
            if accessed && !child.layout_pending.get() {
                child.request_relayout();
            }
            child.request_layout_if_coordinates_are_used_and_notify_children();
        }
    }
}

impl LayoutNode {
    // ---- The lookahead pass (Compose's LayoutNode and LayoutNodeLayoutDelegate lookahead parts) ----

    /// Its lookahead pass, in a lookahead scope.
    pub(crate) fn lookahead_pass(&self) -> Option<Rc<LookaheadPassDelegate>> {
        self.lookahead_pass.borrow().clone()
    }

    /// Its lookahead pass, which a node in a lookahead scope has (its parent measures it in lookahead).
    pub(crate) fn lookahead_pass_delegate(&self) -> Rc<LookaheadPassDelegate> {
        self.lookahead_pass().expect("kompoz: a layout node in a lookahead without its lookahead pass delegate")
    }

    /// Whether it starts a lookahead (Compose's `isOutMostLookaheadRoot`): in a lookahead scope its parent isn't
    /// in, or composed only in its parent's approach.
    pub(crate) fn is_out_most_lookahead_root(&self) -> bool {
        self.lookahead_root.borrow().is_some()
            && (self.parent().is_none_or(|p| p.lookahead_root.borrow().is_none())
                || self.detached_from_parent_lookahead_pass.get())
    }

    /// Whether it was placed in lookahead (Compose's `isPlacedInLookahead`), none outside a lookahead scope.
    pub(crate) fn is_placed_in_lookahead(&self) -> Option<bool> {
        self.lookahead_pass().map(|d| d.is_placed())
    }

    /// Whether it is measured and laid out in a lookahead pass (it is in a lookahead scope, or has an approach node).
    pub fn is_in_lookahead_scope(&self) -> bool {
        self.lookahead_root.borrow().is_some()
    }

    /// The lookahead root it would take from where it was emitted (Compose's `_foldedParent?.lookaheadRoot`): its
    /// virtual parent's (a `LookaheadScope` is one), else its parent's.
    pub(crate) fn inherited_lookahead_root(&self) -> Option<LookaheadRoot> {
        if let Some(v) = self.virtual_parent.borrow().upgrade() {
            return v.lookahead_root();
        }
        self.parent().and_then(|p| p.lookahead_root.borrow().clone())
    }

    /// Its lookahead root becomes `root` (Compose's `lookaheadRoot` setter): in a scope, it and its coordinators get
    /// their lookahead sides; out of one, they drop them and what is pending in lookahead. Measured again.
    pub(crate) fn set_lookahead_root(&self, root: Option<LookaheadRoot>) {
        if *self.lookahead_root.borrow() == root {
            return;
        }
        let in_scope = root.is_some();
        *self.lookahead_root.borrow_mut() = root;
        if in_scope {
            if self.lookahead_pass.borrow().is_none() {
                *self.lookahead_pass.borrow_mut() = Some(LookaheadPassDelegate::new(self));
            }
            for c in self.coordinators().into_iter().chain(std::iter::once(self.inner.clone())) {
                c.ensure_lookahead_delegate();
            }
        } else {
            // (Compose's `onRemovedFromLookaheadScope`; the coordinators' sides go too)
            *self.lookahead_pass.borrow_mut() = None;
            self.lookahead_layout_pending.set(false);
            self.lookahead_measure_pending.set(false);
            for c in self.coordinators().into_iter().chain(std::iter::once(self.inner.clone())) {
                c.clear_lookahead_delegate();
            }
        }
        self.invalidate_measurements();
    }

    /// Its lookahead root worked out again from where it is now emitted (it moved between a scope and outside of
    /// it without leaving the tree), and its subtree's after it.
    pub(crate) fn refresh_lookahead_root(&self) {
        let mut root = self.inherited_lookahead_root();
        if root.is_none() && self.has_kind(crate::modifier::NodeKind::ApproachMeasure) {
            root = Some(LookaheadRoot::Node(self.this.clone()));
        }
        if *self.lookahead_root.borrow() != root {
            self.set_lookahead_root(root);
            for child in self.children() {
                child.refresh_lookahead_root();
            }
        }
    }

    pub(crate) fn mark_lookahead_layout_pending(&self) {
        self.lookahead_layout_pending.set(true);
        self.lookahead_layout_pending_for_alignment.set(true);
    }

    pub(crate) fn mark_lookahead_measure_pending(&self) {
        self.lookahead_measure_pending.set(true);
    }

    /// Measures it again in lookahead within `constraints`, or its last lookahead constraints (Compose's
    /// `lookaheadRemeasure`). Whether its lookahead size changed.
    pub(crate) fn lookahead_remeasure(&self, constraints: Option<Constraints>) -> bool {
        let Some(d) = self.lookahead_pass() else { return false };
        match constraints.or(d.last_constraints()) {
            Some(c) => d.remeasure(c),
            None => false,
        }
    }

    /// Placed again in lookahead where it was (Compose's `lookaheadReplace`).
    pub(crate) fn lookahead_replace(&self) {
        if self.intrinsics_usage_by_parent.get() == UsageByParent::NotUsed {
            self.clear_subtree_intrinsics_usage();
        }
        self.lookahead_pass_delegate().replace();
    }

    /// How its parent's lookahead measured it (Compose's `measuredByParentInLookahead`).
    pub(crate) fn measured_by_parent_in_lookahead(&self) -> UsageByParent {
        self.lookahead_pass().map_or(UsageByParent::NotUsed, |d| d.measured_by_parent.get())
    }

    /// Whether its lookahead alignment lines are needed.
    pub(crate) fn lookahead_alignment_lines_required(&self) -> bool {
        self.lookahead_pass().is_some_and(|d| d.alignment_lines_required())
    }

    /// Its main pass is placed under a motion frame of reference or not (Compose's MeasurePassDelegate
    /// `updatePlacedUnderMotionFrameOfReference`): its outer coordinator takes it, and its coordinates are updated.
    fn update_motion_frame_of_reference(&self, new: bool) {
        let outer = self.outer_coordinator();
        if outer.placed_under_motion_frame_of_reference.get() != new {
            outer.placed_under_motion_frame_of_reference.set(new);
            self.needs_coordinates_update.set(true);
        }
        self.placed_under_motion_frame_of_reference.set(new);
    }
}

impl IntrinsicMeasurable for LayoutNode {
    fn parent_data(&self) -> Option<Rc<dyn Any>> {
        LayoutNode::parent_data(self)
    }
    // (asked from above a lookahead root, its lookahead answers: Compose's MeasurePassDelegate intrinsics)
    fn min_intrinsic_width(&self, height: i32) -> i32 {
        if self.is_out_most_lookahead_root() {
            return self.lookahead_pass_delegate().min_intrinsic_width(height);
        }
        self.on_intrinsics_queried();
        self.outer_coordinator().min_intrinsic_width(height)
    }
    fn max_intrinsic_width(&self, height: i32) -> i32 {
        if self.is_out_most_lookahead_root() {
            return self.lookahead_pass_delegate().max_intrinsic_width(height);
        }
        self.on_intrinsics_queried();
        self.outer_coordinator().max_intrinsic_width(height)
    }
    fn min_intrinsic_height(&self, width: i32) -> i32 {
        if self.is_out_most_lookahead_root() {
            return self.lookahead_pass_delegate().min_intrinsic_height(width);
        }
        self.on_intrinsics_queried();
        self.outer_coordinator().min_intrinsic_height(width)
    }
    fn max_intrinsic_height(&self, width: i32) -> i32 {
        if self.is_out_most_lookahead_root() {
            return self.lookahead_pass_delegate().max_intrinsic_height(width);
        }
        self.on_intrinsics_queried();
        self.outer_coordinator().max_intrinsic_height(width)
    }
}

impl Measurable for LayoutNode {
    fn measure(&self, constraints: Constraints) -> Placeable {
        self.measure_for_parent(constraints)
    }

    fn retain(&self) -> Option<Rc<dyn Measurable>> {
        Some(self.rc())
    }
}

impl PlaceTarget for LayoutNode {
    fn measured_size(&self) -> IntSize {
        self.measured_size.get()
    }
    fn measurement_constraints(&self) -> Constraints {
        self.last_constraints.get().unwrap_or_default()
    }
    fn alignment_line(&self, line: AlignmentLine) -> i32 {
        if let Some(p) = self.parent() {
            match p.layout_state.get() {
                LayoutState::Measuring => self.alignment_lines.used_during_parent_measurement.set(true),
                LayoutState::LayingOut => self.alignment_lines.used_during_parent_layout.set(true),
                _ => {}
            }
        }
        self.during_alignment_lines_query.set(true);
        let v = self.outer_coordinator().alignment_line(line);
        self.during_alignment_lines_query.set(false);
        v
    }
    fn place_at(&self, position: IntOffset, z_index: f32, layer: Option<LayerBlock>) {
        self.place_for_parent(position, z_index, layer);
    }
    fn parent_data(&self) -> Option<Rc<dyn Any>> {
        LayoutNode::parent_data(self)
    }
    fn update_placed_under_motion_frame_of_reference(&self, new: bool) {
        self.update_motion_frame_of_reference(new);
    }
}

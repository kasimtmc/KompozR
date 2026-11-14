//! A layout node's chain of modifier nodes, as Compose's NodeChain: updated from a new modifier by comparing its
//! elements with the last ones position by position (the same element: the node kept; one of the same type: the
//! node kept and updated; otherwise Compose's structural diff, `MyersDiff`), its coordinators synced to its layout
//! nodes, its nodes attached and detached in Compose's order.

use super::diff::{DiffCallback, execute_diff};
use super::{AnyElement, Modifier, NodeKind, NodeRc, dispatch_for_kind};
use crate::node::{LayoutNode, NodeCoordinator};
use std::rc::{Rc, Weak};

/// The elements of a layout node's modifier and their nodes, outermost (the head) first.
#[derive(Default)]
pub(crate) struct NodeChain {
    pub(crate) elements: Vec<Rc<dyn AnyElement>>,
    pub(crate) nodes: Vec<NodeRc>,
}

/// What a change of a node is: invalidations run as Compose's `autoInvalidate…Node`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Phase {
    Updated,
    Inserted,
    Removed,
}

/// What comparing an old element with the new one at its place says (Compose's `actionForModifiers`).
#[derive(PartialEq, Eq)]
enum Action {
    Reuse,
    Update,
    Replace,
}

fn action_for(prev: &dyn AnyElement, next: &dyn AnyElement) -> Action {
    if prev.eq_dyn(next) {
        Action::Reuse
    } else if prev.same_type(next) {
        Action::Update
    } else {
        Action::Replace
    }
}

/// A step of the diff (Compose's DiffCallback calls), in its order: an old node kept for a new element (updated if
/// the element changed), a node made for a new element, an old node removed.
enum Op {
    Same(usize, usize),
    Insert(usize),
    Remove(usize),
}

/// Compose's NodeChain.Differ: the steps of the diff of what is left after `offset`.
struct Differ<'a> {
    before: &'a [Rc<dyn AnyElement>],
    after: &'a [Rc<dyn AnyElement>],
    offset: usize,
    ops: Vec<Op>,
}

impl DiffCallback for Differ<'_> {
    fn are_items_the_same(&mut self, old_index: usize, new_index: usize) -> bool {
        action_for(&*self.before[self.offset + old_index], &*self.after[self.offset + new_index]) != Action::Replace
    }
    fn insert(&mut self, new_index: usize) {
        self.ops.push(Op::Insert(self.offset + new_index));
    }
    fn remove(&mut self, _at_index: usize, old_index: usize) {
        self.ops.push(Op::Remove(self.offset + old_index));
    }
    fn same(&mut self, old_index: usize, new_index: usize) {
        self.ops.push(Op::Same(self.offset + old_index, self.offset + new_index));
    }
}

impl LayoutNode {
    /// Sets the modifier: applied now if the node is attached, else when it is (Compose's `modifier` setter).
    pub(crate) fn set_modifier(self: &Rc<Self>, modifier: Modifier) {
        if self.is_attached() {
            self.apply_modifier(modifier, true);
        } else {
            *self.pending_modifier.borrow_mut() = Some(modifier);
        }
    }

    /// Compose's `NodeChain.updateFrom`: nodes inserted are attached at once with `attach_inserted` (the node is
    /// attached), else with the node (it is being attached: Compose's `applyingModifierOnAttach`).
    ///
    /// The steps run in the diff's order — an update as its node is reached, a removed node detached, an inserted
    /// one made — with nothing of the chain borrowed (a node's callbacks may walk it); the inserted nodes are
    /// attached, in order, once the new chain and its coordinators are in place (Compose attaches each as it is
    /// inserted, its coordinator patched in; here the coordinators are synced once).
    pub(crate) fn apply_modifier(self: &Rc<Self>, modifier: Modifier, attach_inserted: bool) {
        let (before, old_nodes) = {
            let chain = self.chain.borrow();
            (chain.elements.clone(), chain.nodes.clone())
        };
        let after: Vec<Rc<dyn AnyElement>> = modifier.elements().to_vec();
        let mut ops = Vec::new();
        let mut structural = true;
        if after.len() == before.len() {
            // The common case: the same elements, maybe updated; a structural change from the first replaced one.
            let mut i = 0;
            while i < before.len() && action_for(&*before[i], &*after[i]) != Action::Replace {
                ops.push(Op::Same(i, i));
                i += 1;
            }
            if i < before.len() {
                ops.extend(Self::diff(i, &before, &after));
            } else {
                structural = false;
            }
        } else if !attach_inserted && before.is_empty() {
            // Made as the node is attached: all inserted, attached with it.
            ops.extend((0..after.len()).map(Op::Insert));
        } else if after.is_empty() {
            ops.extend((0..before.len()).map(Op::Remove));
        } else {
            ops.extend(Self::diff(0, &before, &after));
        }
        let mut nodes = Vec::with_capacity(after.len());
        let mut inserted = Vec::new();
        for op in ops {
            match op {
                Op::Same(old, new) => {
                    let node = old_nodes[old].clone();
                    if !before[old].eq_dyn(&*after[new]) {
                        after[new].update_node(&node);
                        if node.meta.attached.get() {
                            auto_invalidate(&node, Phase::Updated);
                        }
                    }
                    nodes.push(node);
                }
                Op::Remove(old) => {
                    let node = &old_nodes[old];
                    if node.meta.attached.get() {
                        auto_invalidate(node, Phase::Removed);
                        node.run_detach_lifecycle();
                        node.mark_as_detached();
                    }
                }
                Op::Insert(new) => {
                    let node = after[new].create_node();
                    nodes.push(node.clone());
                    inserted.push(node);
                }
            }
        }
        {
            let mut chain = self.chain.borrow_mut();
            chain.elements = after;
            chain.nodes = nodes;
        }
        *self.modifier.borrow_mut() = modifier;
        if structural {
            self.sync_aggregate_kinds();
            self.sync_coordinators();
        }
        if attach_inserted {
            for node in inserted {
                node.mark_as_attached();
                node.run_attach_lifecycle();
                node.meta.inserted_awaiting_attach.set(false);
                auto_invalidate(&node, Phase::Inserted);
            }
        }
        if structural {
            self.invalidate_parent_data();
        }
    }

    /// The steps of Compose's structural diff of the elements after `offset`.
    fn diff(offset: usize, before: &[Rc<dyn AnyElement>], after: &[Rc<dyn AnyElement>]) -> Vec<Op> {
        let mut differ = Differ { before, after, offset, ops: Vec::new() };
        execute_diff(before.len() - offset, after.len() - offset, &mut differ);
        differ.ops
    }

    /// Compose's `syncAggregateChildKindSet`: each node's kinds and those after it.
    pub(crate) fn sync_aggregate_kinds(&self) {
        let chain = self.chain.borrow();
        let mut aggregate = 0;
        for node in chain.nodes.iter().rev() {
            aggregate |= node.meta.kind_set.get();
            node.meta.aggregate_child_kind_set.set(aggregate);
        }
        self.aggregate_kinds.set(aggregate);
    }

    /// Compose's `syncCoordinators`: a coordinator for each layout node of the chain (the one it had kept), the
    /// other nodes given to the coordinator of the layout node after them (or the inner one), the coordinators
    /// linked outer to inner.
    pub(crate) fn sync_coordinators(self: &Rc<Self>) {
        let nodes = self.chain_nodes();
        let weak: Weak<LayoutNode> = Rc::downgrade(self);
        let old: Vec<Rc<NodeCoordinator>> = self.coordinators();
        let mut coordinator = self.inner.clone();
        let mut owned: Vec<NodeRc> = Vec::new();
        let mut kept: Vec<Rc<NodeCoordinator>> = Vec::new();
        for node in nodes.iter().rev() {
            if node.is_kind(NodeKind::Layout) {
                coordinator.set_nodes(std::mem::take(&mut owned));
                let mut layout_cell = None;
                dispatch_for_kind(node, NodeKind::Layout, &mut |c| {
                    if layout_cell.is_none() {
                        layout_cell = Some(c.clone());
                    }
                });
                let layout_cell = layout_cell.expect("a layout node without a layout delegate");
                let next = match old.iter().find(|c| c.entry().is_some_and(|e| Rc::ptr_eq(&e, node))) {
                    Some(c) => {
                        c.set_layout_cell(layout_cell);
                        c.clone()
                    }
                    None => NodeCoordinator::new_modifier(weak.clone(), node.clone(), layout_cell),
                };
                *coordinator.wrapped_by.borrow_mut() = Rc::downgrade(&next);
                *next.wrapped.borrow_mut() = Some(coordinator.clone());
                kept.push(next.clone());
                coordinator = next;
            }
            node.set_place(Some(&coordinator), &weak);
            owned.insert(0, node.clone());
        }
        coordinator.set_nodes(owned);
        *coordinator.wrapped_by.borrow_mut() = self.parent().map_or_else(Weak::new, |p| Rc::downgrade(&p.inner));
        for c in &old {
            if !kept.iter().any(|k| Rc::ptr_eq(k, c)) {
                c.on_release();
            }
        }
        *self.outer.borrow_mut() = coordinator;
    }

    /// Marks its nodes attached, then (once its children are attached) runs their `on_attach`, head first, with the
    /// invalidations of nodes inserted while it was detached (Compose's `markAsAttached` / `runAttachLifecycle`).
    pub(crate) fn mark_chain_attached(&self) {
        for node in self.chain_nodes() {
            node.mark_as_attached();
        }
    }

    pub(crate) fn run_chain_attach_lifecycle(&self) {
        for node in self.chain_nodes() {
            node.run_attach_lifecycle();
            if node.meta.inserted_awaiting_attach.replace(false) {
                auto_invalidate(&node, Phase::Inserted);
            }
        }
    }

    /// Runs its nodes' `on_detach`, tail first (Compose's `runDetachLifecycle`).
    pub(crate) fn run_chain_detach_lifecycle(&self) {
        for node in self.chain_nodes().iter().rev() {
            if node.meta.attached.get() {
                node.run_detach_lifecycle();
            }
        }
    }

    /// Marks them detached, tail first (Compose's `markAsDetached`).
    pub(crate) fn mark_chain_detached(&self) {
        for node in self.chain_nodes().iter().rev() {
            if node.meta.attached.get() {
                node.mark_as_detached();
            }
        }
    }

    /// Resets its nodes (tail first) and detaches them (Compose's `resetState`): its layout reused for other
    /// content.
    pub(crate) fn reset_chain(&self) {
        for node in self.chain_nodes().iter().rev() {
            if node.meta.attached.get() {
                node.reset();
            }
        }
        self.run_chain_detach_lifecycle();
        self.mark_chain_detached();
    }

    /// The node reused for other content (Compose's `onReuse`, what a reusing layout's pool does): its nodes reset
    /// and detached, then attached again, everything measured again.
    pub fn on_reuse(&self) {
        assert!(self.is_attached(), "kompoz: on_reuse is for an attached layout node");
        self.reset_chain();
        self.mark_chain_attached();
        self.run_chain_attach_lifecycle();
        if self.has_kind(NodeKind::Semantics) {
            self.invalidate_semantics();
        }
        self.invalidate_measurements();
        self.request_relayout();
    }
}

/// The invalidations a node's change calls for (Compose's `autoInvalidateNodeIncludingDelegates`): a delegating
/// node's own kinds, then its delegates' (those it doesn't have itself).
pub(crate) fn auto_invalidate(node: &NodeRc, phase: Phase) {
    including_delegates(node, !0, phase);
}

fn including_delegates(node: &NodeRc, remaining: u32, phase: Phase) {
    let delegates = node.immediate_delegates();
    if delegates.is_empty() {
        invalidate_self(node, node.meta.kind_set.get() & remaining, phase);
    } else {
        let own = node.meta.self_kind_set.get();
        invalidate_self(node, own & remaining, phase);
        for d in delegates {
            including_delegates(&d, remaining & !own, phase);
        }
    }
}

fn invalidate_self(node: &NodeRc, kinds: u32, phase: Phase) {
    if phase == Phase::Updated && !node.node.borrow().should_auto_invalidate() {
        return;
    }
    let Some(layout_node) = node.layout_node() else { return };
    if NodeKind::Layout.is_in(kinds) {
        layout_node.invalidate_measurements();
        if phase == Phase::Removed {
            layout_node.invalidate_draw();
        }
    }
    if NodeKind::OnRemeasured.is_in(kinds) && phase != Phase::Removed {
        layout_node.invalidate_measurements();
    }
    if NodeKind::OnPlaced.is_in(kinds) && phase != Phase::Removed {
        layout_node.request_relayout();
    }
    if NodeKind::GlobalPositionAware.is_in(kinds) {
        match phase {
            Phase::Inserted => layout_node.add_globally_positioned_observers(1),
            Phase::Removed => layout_node.add_globally_positioned_observers(-1),
            Phase::Updated => {}
        }
        if phase != Phase::Removed {
            layout_node.invalidate_on_positioned();
        }
    }
    if NodeKind::Draw.is_in(kinds) {
        layout_node.invalidate_draw();
    }
    if NodeKind::Semantics.is_in(kinds) {
        layout_node.invalidate_semantics();
    }
    if NodeKind::ParentData.is_in(kinds) {
        layout_node.invalidate_parent_data();
    }
    let focus = NodeKind::FocusTarget.or(NodeKind::FocusProperties).or(NodeKind::FocusEvent);
    if focus.is_in(kinds) {
        crate::focus::on_focus_node_changed(node, kinds, phase == Phase::Removed);
    }
    if NodeKind::IndirectPointerInput.is_in(kinds) && phase == Phase::Removed {
        crate::input::on_indirect_pointer_node_removed(node);
    }
}

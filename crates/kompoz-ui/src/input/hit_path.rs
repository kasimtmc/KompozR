//! The hit paths of the pointers down (and of a hovering mouse), as Compose's `HitPathTracker`: a tree of the
//! pointer input nodes hit, outermost at the top, each knowing its pointers; an event is cached per node (its
//! changes local to it, its Enter or Exit worked out), then dispatched — Initial from the top down and Main back up
//! in one walk, then Final from the top down — and the nodes whose pointers are gone are dropped.

use super::pointer::{
    HistoricalChange, InternalPointerEvent, PointerEvent, PointerEventPass, PointerEventType, PointerInputChange,
};
use super::{PointerId, PointerInputModifierNode, call_node};
use crate::layout::LayoutCoordinates;
use crate::modifier::{NodeKind, NodeRc, dispatch_for_kind};
use kompoz_graphics::{IntSize, Offset};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What a node of the tree caches for the event under way.
#[derive(Default)]
struct HitState {
    /// The changes of its pointers, local to it, by pointer id.
    relevant_changes: Vec<PointerInputChange>,
    coordinates: Option<LayoutCoordinates>,
    /// The event it got last (kept to tell whether a move moved anything).
    pointer_event: Option<PointerEvent>,
    was_in: bool,
    is_in: bool,
    has_exited: bool,
}

/// A pointer input node on a hit path (Compose's HitPathTracker.Node).
struct HitNode {
    modifier_node: NodeRc,
    pointer_ids: RefCell<Vec<PointerId>>,
    state: RefCell<HitState>,
    children: RefCell<Vec<Rc<HitNode>>>,
}

impl HitNode {
    fn new(modifier_node: NodeRc) -> Rc<HitNode> {
        Rc::new(HitNode {
            modifier_node,
            pointer_ids: RefCell::new(Vec::new()),
            state: RefCell::new(HitState { is_in: true, has_exited: true, ..Default::default() }),
            children: RefCell::new(Vec::new()),
        })
    }

    fn add_pointer(&self, id: PointerId) {
        let mut ids = self.pointer_ids.borrow_mut();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }

    fn remove_pointer(&self, id: PointerId) {
        self.pointer_ids.borrow_mut().retain(|p| *p != id);
    }

    fn has_pointer(&self, id: PointerId) -> bool {
        self.pointer_ids.borrow().contains(&id)
    }

    fn children(&self) -> Vec<Rc<HitNode>> {
        self.children.borrow().clone()
    }

    fn attached(&self) -> bool {
        self.modifier_node.meta.attached.get()
    }

    /// Calls `f` on its pointer input parts (it, or its delegates that are), none of the node borrowed while what
    /// they set off runs.
    fn dispatch(&self, f: &mut dyn FnMut(&mut dyn PointerInputModifierNode)) {
        let mut targets = Vec::new();
        dispatch_for_kind(&self.modifier_node, NodeKind::PointerInput, &mut |c| targets.push(c.clone()));
        for t in targets {
            call_node(|| {
                let mut node = t.node.borrow_mut();
                if let Some(p) = node.as_pointer_input() {
                    f(p);
                }
            });
        }
    }

    /// Compose's `removeInvalidPointerIdsAndChanges`: a pointer that changed to down drops the nodes it no longer
    /// hit.
    fn remove_invalid_pointer_ids_and_changes(&self, id: PointerId, hit_nodes: &[Rc<HitNode>]) {
        if self.has_pointer(id) && !hit_nodes.iter().any(|h| std::ptr::eq(&**h, self)) {
            self.remove_pointer(id);
            self.state.borrow_mut().relevant_changes.retain(|c| c.id != id);
        }
        for c in self.children() {
            c.remove_invalid_pointer_ids_and_changes(id, hit_nodes);
        }
    }

    /// Caches the event for it and its children; whether anything is to be dispatched (Compose's `buildCache`).
    fn build_cache(
        &self,
        changes: &[PointerInputChange],
        internal: &Rc<InternalPointerEvent>,
        in_bounds: bool,
    ) -> bool {
        let mut child_changed = false;
        for c in self.children() {
            child_changed = c.build_cache(changes, internal, in_bounds) || child_changed;
        }
        if !self.attached() {
            return true;
        }
        let mut coordinates = None;
        dispatch_for_kind(&self.modifier_node, NodeKind::PointerInput, &mut |c| {
            coordinates = pointer_coordinates(c);
        });
        let Some(coordinates) = coordinates.filter(|c| c.is_attached()) else { return true };
        let ids = self.pointer_ids.borrow().clone();
        let mut relevant = Vec::new();
        for change in changes {
            if !ids.contains(&change.id) {
                continue;
            }
            if change.previous_position.is_valid() && change.position.is_valid() {
                let historical = change
                    .historical()
                    .iter()
                    .filter(|h| h.position.is_valid())
                    .map(|h| HistoricalChange {
                        uptime_millis: h.uptime_millis,
                        position: window_to_local(&coordinates, h.position),
                        scale_factor: h.scale_factor,
                        pan_offset: h.pan_offset,
                        original_event_position: h.original_event_position,
                    })
                    .collect();
                relevant.push(change.moved_to(
                    window_to_local(&coordinates, change.previous_position),
                    window_to_local(&coordinates, change.position),
                    historical,
                ));
            }
        }
        if relevant.is_empty() {
            self.pointer_ids.borrow_mut().clear();
            self.children.borrow_mut().clear();
            return true; // not hit
        }
        self.pointer_ids.borrow_mut().retain(|id| changes.iter().any(|c| c.id == *id));
        let mut event = PointerEvent::of(relevant.clone(), internal);
        let mut state = self.state.borrow_mut();
        let active_hover = event.changes.iter().find(|c| internal.active_hover_event(c.id)).cloned();
        if let Some(hover) = active_hover {
            if !in_bounds {
                state.is_in = false;
            } else if !state.is_in && (hover.pressed || hover.previous_pressed) {
                state.is_in = !hover.is_outside(coordinates.size());
            }
            let t = event.r#type;
            if state.is_in != state.was_in
                && (t == PointerEventType::Move || t == PointerEventType::Enter || t == PointerEventType::Exit)
            {
                event.r#type = if state.is_in { PointerEventType::Enter } else { PointerEventType::Exit };
            } else if t == PointerEventType::Enter && state.was_in && !state.has_exited {
                event.r#type = PointerEventType::Move; // it knew it was in
            } else if t == PointerEventType::Exit && state.is_in && hover.pressed {
                event.r#type = PointerEventType::Move; // still in
            }
        }
        let changed = child_changed
            || event.r#type != PointerEventType::Move
            || has_position_changed(&state.pointer_event, &event);
        state.relevant_changes = relevant;
        state.coordinates = Some(coordinates);
        state.pointer_event = Some(event);
        changed
    }

    /// Whether there is something to dispatch to it (its changes, attached, its layout placed).
    fn can_dispatch(&self) -> Option<(PointerEvent, IntSize)> {
        let state = self.state.borrow();
        if state.relevant_changes.is_empty() || !self.attached() {
            return None;
        }
        let placed = self.modifier_node.layout_node().is_some_and(|n| n.is_placed());
        if !placed {
            return None;
        }
        let event = state.pointer_event.clone()?;
        let size = state.coordinates.as_ref().map_or(IntSize::Zero, |c| c.size());
        Some((event, size))
    }

    /// The Initial pass on it, its children's two passes, then the Main pass on it (Compose's
    /// `dispatchMainEventPass`).
    fn dispatch_main_event_pass(&self) -> bool {
        let Some((event, size)) = self.can_dispatch() else { return false };
        self.dispatch(&mut |p| p.on_pointer_event(&event, PointerEventPass::Initial, size));
        if self.attached() {
            for c in self.children() {
                c.dispatch_main_event_pass();
            }
        }
        if self.attached() {
            self.dispatch(&mut |p| p.on_pointer_event(&event, PointerEventPass::Main, size));
        }
        true
    }

    /// The Final pass on it, then on its children; what is gone dropped (Compose's `dispatchFinalEventPass`).
    fn dispatch_final_event_pass(&self, internal: &InternalPointerEvent) -> bool {
        let result = match self.can_dispatch() {
            Some((event, size)) => {
                self.dispatch(&mut |p| p.on_pointer_event(&event, PointerEventPass::Final, size));
                if self.attached() {
                    for c in self.children() {
                        c.dispatch_final_event_pass(internal);
                    }
                }
                true
            }
            None => false,
        };
        self.clean_up_hits(internal);
        let mut state = self.state.borrow_mut();
        state.relevant_changes.clear();
        state.coordinates = None;
        result
    }

    /// Compose's `cleanUpHits`: children without pointers dropped; pointers let go (out of a hover stream, or
    /// outside it) forgotten.
    fn clean_up_hits(&self, internal: &InternalPointerEvent) {
        self.children.borrow_mut().retain(|c| !c.pointer_ids.borrow().is_empty());
        let mut state = self.state.borrow_mut();
        let Some(event) = state.pointer_event.clone() else { return };
        state.was_in = state.is_in;
        let outside = !state.is_in;
        for change in &event.changes {
            let released = !change.pressed;
            let non_hover = !internal.active_hover_event(change.id);
            if (released && non_hover) || (released && outside) {
                self.remove_pointer(change.id);
            }
        }
        state.is_in = false;
        state.has_exited = event.r#type == PointerEventType::Exit;
    }

    /// Its children's cancel, then its own (Compose's `dispatchCancel`).
    fn dispatch_cancel(&self) {
        for c in self.children() {
            c.dispatch_cancel();
        }
        self.dispatch(&mut |p| p.on_cancel_pointer_input());
    }
}

/// The coordinates a pointer input node gets its events in (Compose's `requireCoordinator(PointerInput)`): its
/// coordinator's, or, for the layout node of a coordinator itself, what that measured.
pub(crate) fn pointer_coordinates(cell: &NodeRc) -> Option<LayoutCoordinates> {
    let coordinator = cell.coordinator()?;
    let is_tail = coordinator.entry().is_some_and(|e| Rc::ptr_eq(&e, cell));
    let c = if is_tail { coordinator.wrapped()? } else { coordinator };
    Some(LayoutCoordinates::of(c))
}

/// A position in the window, in `coordinates` (through the root of their tree, which the window places).
pub(crate) fn window_to_local(coordinates: &LayoutCoordinates, position: Offset) -> Offset {
    let root = coordinates.coordinator.find_root();
    let in_root = root.parent_to_local_position(position);
    coordinates.local_position_of(&LayoutCoordinates::of(root), in_root)
}

fn has_position_changed(old: &Option<PointerEvent>, new: &PointerEvent) -> bool {
    let Some(old) = old else { return true };
    if old.changes.len() != new.changes.len() {
        return true;
    }
    old.changes.iter().zip(&new.changes).any(|(a, b)| a.position != b.position)
}

/// The tree's top, and what is put off while an event is dispatched.
struct TrackerInner {
    children: RefCell<Vec<Rc<HitNode>>>,
    dispatching: Cell<bool>,
    dispatch_cancel_after: Cell<bool>,
    clear_cache_after: Cell<bool>,
    nodes_to_remove: RefCell<Vec<NodeRc>>,
}

/// The hit paths (Compose's HitPathTracker).
pub(crate) struct HitPathTracker {
    inner: Rc<TrackerInner>,
}

impl Default for HitPathTracker {
    fn default() -> Self {
        HitPathTracker {
            inner: Rc::new(TrackerInner {
                children: RefCell::new(Vec::new()),
                dispatching: Cell::new(false),
                dispatch_cancel_after: Cell::new(false),
                clear_cache_after: Cell::new(false),
                nodes_to_remove: RefCell::new(Vec::new()),
            }),
        }
    }
}

impl HitPathTracker {
    /// Adds a pointer's hit path, outermost first, merged into what is there from the top; a pointer that changed
    /// to down drops the nodes it no longer hits (Compose's `addHitPath`).
    pub(crate) fn add_hit_path(&self, pointer_id: PointerId, nodes: &[NodeRc], prune: bool) {
        let mut parent: Option<Rc<HitNode>> = None;
        let mut merging = true;
        let mut hit_nodes: Vec<Rc<HitNode>> = Vec::new();
        for cell in nodes {
            if !cell.meta.attached.get() {
                continue;
            }
            let (tracker, node) = (Rc::downgrade(&self.inner), Rc::downgrade(cell));
            *cell.meta.detached_listener.borrow_mut() = Some(Rc::new(move || {
                if let (Some(t), Some(n)) = (tracker.upgrade(), node.upgrade()) {
                    remove_pointer_input_modifier_node(&t, &n);
                }
            }));
            let siblings = match &parent {
                Some(p) => p.children(),
                None => self.inner.children.borrow().clone(),
            };
            if merging {
                if let Some(existing) = siblings.iter().find(|c| Rc::ptr_eq(&c.modifier_node, cell)) {
                    existing.state.borrow_mut().is_in = true; // Compose's `markIsIn`
                    existing.add_pointer(pointer_id);
                    if prune {
                        hit_nodes.push(existing.clone());
                    }
                    parent = Some(existing.clone());
                    continue;
                }
                merging = false;
            }
            let node = HitNode::new(cell.clone());
            node.add_pointer(pointer_id);
            if prune {
                hit_nodes.push(node.clone());
            }
            match &parent {
                Some(p) => p.children.borrow_mut().push(node.clone()),
                None => self.inner.children.borrow_mut().push(node.clone()),
            }
            parent = Some(node);
        }
        if prune {
            let top = self.inner.children.borrow().clone();
            for c in top {
                c.remove_invalid_pointer_ids_and_changes(pointer_id, &hit_nodes);
            }
        }
    }

    /// Dispatches an event to the hit paths; whether it went to a node (Compose's `dispatchChanges`).
    pub(crate) fn dispatch_changes(&self, internal: &Rc<InternalPointerEvent>, in_bounds: bool) -> bool {
        let top = self.inner.children.borrow().clone();
        let mut changed = false;
        for c in &top {
            changed = c.build_cache(&internal.changes, internal, in_bounds) || changed;
        }
        if !changed {
            return false;
        }
        self.inner.dispatching.set(true);
        /// Ends the dispatch however it ends.
        struct Dispatching<'a>(&'a TrackerInner);
        impl Drop for Dispatching<'_> {
            fn drop(&mut self) {
                self.0.dispatching.set(false);
            }
        }
        let guard = Dispatching(&self.inner);
        let top = self.inner.children.borrow().clone();
        let mut hit = false;
        for c in &top {
            hit = c.dispatch_main_event_pass() || hit;
        }
        let top = self.inner.children.borrow().clone();
        let mut final_hit = false;
        for c in &top {
            final_hit = c.dispatch_final_event_pass(internal) || final_hit;
        }
        self.inner.children.borrow_mut().retain(|c| !c.pointer_ids.borrow().is_empty());
        hit = final_hit || hit;
        drop(guard);
        let to_remove = std::mem::take(&mut *self.inner.nodes_to_remove.borrow_mut());
        for n in to_remove {
            remove_pointer_input_modifier_node(&self.inner, &n);
        }
        if self.inner.dispatch_cancel_after.replace(false) {
            self.process_cancel();
        }
        if self.inner.clear_cache_after.replace(false) {
            self.clear_previously_hit_modifier_node_cache();
        }
        hit
    }

    /// Forgets the paths (Compose's `clearPreviouslyHitModifierNodeCache`).
    pub(crate) fn clear_previously_hit_modifier_node_cache(&self) {
        if self.inner.dispatching.get() {
            self.inner.clear_cache_after.set(true);
            return;
        }
        self.inner.children.borrow_mut().clear();
    }

    /// Cancels the gesture: every node on a path told, the paths forgotten (Compose's `processCancel`).
    pub(crate) fn process_cancel(&self) {
        if self.inner.dispatching.get() {
            self.inner.dispatch_cancel_after.set(true);
            return;
        }
        let top = self.inner.children.borrow().clone();
        for c in top {
            c.dispatch_cancel();
        }
        self.clear_previously_hit_modifier_node_cache();
    }

    /// Whether a node is on a path (tests ask).
    pub(crate) fn contains(&self, cell: &NodeRc) -> bool {
        fn find(nodes: &[Rc<HitNode>], cell: &NodeRc) -> bool {
            nodes.iter().any(|n| Rc::ptr_eq(&n.modifier_node, cell) || find(&n.children(), cell))
        }
        find(&self.inner.children.borrow(), cell)
    }
}

/// A node on a path left its layout: dropped with what is under it, which is cancelled (Compose's
/// `removePointerInputModifierNode`); put off while an event is dispatched.
fn remove_pointer_input_modifier_node(tracker: &Rc<TrackerInner>, cell: &NodeRc) {
    if tracker.dispatching.get() {
        tracker.nodes_to_remove.borrow_mut().push(cell.clone());
        return;
    }
    let mut stack: Vec<Option<Rc<HitNode>>> = vec![None];
    while let Some(parent) = stack.pop() {
        let children = match &parent {
            Some(p) => p.children(),
            None => tracker.children.borrow().clone(),
        };
        for child in children {
            if Rc::ptr_eq(&child.modifier_node, cell) {
                match &parent {
                    Some(p) => p.children.borrow_mut().retain(|c| !Rc::ptr_eq(c, &child)),
                    None => tracker.children.borrow_mut().retain(|c| !Rc::ptr_eq(c, &child)),
                }
                child.dispatch_cancel();
            } else {
                stack.push(Some(child));
            }
        }
    }
}

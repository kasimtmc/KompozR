//! The host's focus (Compose's FocusOwnerImpl and FocusInvalidationManager): the active target, moving and
//! clearing the focus, focus search from the root of the window that has it, key and rotary events along the
//! focused path, and the invalidations worked off after each frame.

use super::search::{custom_focus_search, focus_search, scoped_active};
use super::target::{
    CustomDestinationResult, dispatch_focus_callbacks, fetch_focus_properties, focus_rect, focus_state,
    invalidate_focus, is_target, perform_custom_clear_focus, prepare_to_clear_focus, request_focus, send_focus_event,
};
use super::{FocusDirection, FocusState, ancestors};
use crate::input::{IndirectPointerEvent, KeyEvent, KeyEventType, PointerEventPass, RotaryScrollEvent};
use crate::modifier::{ModifierNode, NodeCell, NodeCx, NodeKind, NodeRc, dispatch_for_kind};
use crate::node::{LayoutNode, Owner};
use kompoz_graphics::{LayoutDirection, Rect};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

type WeakCell = Weak<NodeCell<dyn ModifierNode>>;

/// The host's focus state (Compose's FocusOwner): which target has the focus, whether it is captured, the targets
/// and event nodes whose focus is to be worked out again, the keys down. Each window's root (the host's, each
/// popup's) has a root target that never takes the focus; search starts at the root of the window on top that
/// takes the focus (a focusable popup's, a dialog's), else the host's.
#[derive(Default)]
pub struct FocusOwner {
    pub(crate) owner: RefCell<Weak<Owner>>,
    active: RefCell<Option<WeakCell>>,
    captured: Cell<bool>,
    pending_targets: RefCell<Vec<WeakCell>>,
    pending_events: RefCell<Vec<WeakCell>>,
    pending: Cell<bool>,
    keys_down: RefCell<Vec<u64>>,
}

impl FocusOwner {
    fn owner(&self) -> Option<Rc<Owner>> {
        self.owner.borrow().upgrade()
    }

    /// The target with the focus, if it is attached (Compose's `activeFocusTargetNode`).
    pub(crate) fn active_focus_target(&self) -> Option<NodeRc> {
        self.active.borrow().as_ref().and_then(Weak::upgrade).filter(|c| c.meta.attached.get())
    }

    /// The layout node of the target with the focus.
    pub fn focused_layout_node(&self) -> Option<Rc<LayoutNode>> {
        self.active_focus_target().and_then(|c| c.layout_node())
    }

    /// Whether a target has the focus.
    pub fn has_focus(&self) -> bool {
        self.active_focus_target().is_some()
    }

    /// Sets the active target (Compose's `activeFocusTargetNode` setter): a capture ends with it; the indirect
    /// pointer nodes the focus left are cancelled (Compose's AndroidComposeView focus listener).
    pub(crate) fn set_active(&self, target: Option<&NodeRc>) {
        let previous = self.active_focus_target();
        *self.active.borrow_mut() = target.map(Rc::downgrade);
        let same = match (&previous, target) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        };
        if target.is_none() || !same {
            self.captured.set(false);
        }
        if let Some(previous) = previous
            && !same
        {
            let now: Vec<NodeRc> = target.map(indirect_pointer_path).unwrap_or_default();
            for node in indirect_pointer_path(&previous) {
                if !now.iter().any(|n| Rc::ptr_eq(n, &node)) {
                    crate::input::call_node(|| {
                        let mut n = node.node.borrow_mut();
                        if let Some(i) = n.as_indirect_pointer_input() {
                            i.on_cancel_indirect_pointer_input();
                        }
                    });
                }
            }
        }
    }

    /// Whether the focus is captured (Compose's `isFocusCaptured`).
    pub fn is_focus_captured(&self) -> bool {
        self.captured.get()
    }

    pub(crate) fn set_captured(&self, captured: bool) {
        assert!(
            !captured || self.active_focus_target().is_some(),
            "Cannot capture focus when the active focus target node is unset"
        );
        self.captured.set(captured);
    }

    /// The root target search starts at (Compose's `rootFocusNode`): the top focusable popup's or dialog's, else
    /// the host window's.
    pub(crate) fn root_focus_node(&self) -> Option<NodeRc> {
        let owner = self.owner()?;
        match owner.top_focusable_popup() {
            Some(popup) => root_target_of(&popup),
            None => root_target_of(&owner.root()?),
        }
    }

    /// The root target of the window the active target is in.
    fn active_root(&self) -> Option<NodeRc> {
        let active = self.active_focus_target()?;
        Some(ancestors(&active, NodeKind::FocusTarget).pop().unwrap_or(active))
    }

    fn layout_direction(&self) -> LayoutDirection {
        self.owner().and_then(|o| o.root()).map_or(LayoutDirection::Ltr, |r| r.layout_direction())
    }

    /// The state of the root target search starts at (Compose's `rootState`).
    pub fn root_state(&self) -> FocusState {
        self.root_focus_node().map_or(FocusState::Inactive, |r| focus_state(&r))
    }

    /// Takes the focus from where it is (Compose's `clearFocus(force)`): not from a captured target unless forced,
    /// and not if an exit on the way cancels or redirects (unless forced). Whether it was cleared.
    pub fn clear_focus(&self, force: bool) -> bool {
        self.clear_focus_with(force, true, true, FocusDirection::Exit)
    }

    /// Compose's `clearFocus(force, refreshFocusEvents, clearOwnerFocus, focusDirection)`.
    pub(crate) fn clear_focus_with(
        &self,
        force: bool,
        refresh_focus_events: bool,
        _clear_owner_focus: bool,
        direction: FocusDirection,
    ) -> bool {
        if !force && let Some(root) = self.active_root() {
            match perform_custom_clear_focus(&root, direction) {
                CustomDestinationResult::None => {}
                _ => return false,
            }
        }
        self.clear_focus_internal(force, refresh_focus_events)
    }

    fn clear_focus_internal(&self, forced: bool, refresh_focus_events: bool) -> bool {
        let Some(previous) = self.active_focus_target() else { return true };
        if self.captured.get() && !forced {
            return false;
        }
        let was_captured = self.captured.get();
        self.set_active(None);
        if refresh_focus_events {
            dispatch_focus_callbacks(
                &previous,
                if was_captured { FocusState::Captured } else { FocusState::Active },
                FocusState::Inactive,
            );
            for a in ancestors(&previous, NodeKind::FocusTarget) {
                dispatch_focus_callbacks(&a, FocusState::ActiveParent, FocusState::Inactive);
            }
        }
        true
    }

    /// Clears the focus without asking (Compose's `releaseFocus`: the window lost it).
    pub fn release_focus(&self) {
        if let Some(root) = self.active_root() {
            prepare_to_clear_focus(&root, true, true);
        }
        if let Some(previous) = self.active_focus_target() {
            self.set_active(None);
            dispatch_focus_callbacks(&previous, FocusState::Active, FocusState::Inactive);
        }
    }

    /// Moves the focus `direction` (Compose's `moveFocus`), a 1D move wrapping around at the end; whether it
    /// moved.
    pub fn move_focus(&self, direction: FocusDirection) -> bool {
        self.move_focus_with(direction, true)
    }

    pub(crate) fn move_focus_with(&self, direction: FocusDirection, wrap_around: bool) -> bool {
        self.process_invalidations();
        let mut request_success: Option<bool> = Some(false);
        let before = self.active_focus_target();
        let search = self.focus_search(direction, None, &mut |t| {
            let r = request_focus(t, direction);
            request_success = Some(r);
            r
        });
        let after = self.active_focus_target();
        let moved = match (&before, &after) {
            (Some(a), Some(b)) => !Rc::ptr_eq(a, b),
            (None, Some(_)) => true,
            _ => false,
        };
        if search == Some(true) && moved {
            return true;
        }
        let (Some(search), Some(requested)) = (search, request_success) else { return false };
        if search && requested {
            return true;
        }
        if direction.is_1d() && wrap_around {
            let cleared = self.clear_focus_with(false, true, false, direction);
            return cleared && self.take_focus(direction, None);
        }
        false
    }

    /// Gives the focus to the first target `direction` finds from the root (Compose's `takeFocus`).
    pub(crate) fn take_focus(&self, direction: FocusDirection, previously_focused_rect: Option<Rect>) -> bool {
        self.focus_search(direction, previously_focused_rect, &mut |t| request_focus(t, direction)).unwrap_or(false)
    }

    /// Clears the focus, then gives it to the first target `direction` finds (Compose's `resetFocus`).
    pub fn reset_focus(&self, direction: FocusDirection) -> bool {
        if !self.clear_focus_with(false, true, false, direction) {
            return false;
        }
        self.focus_search(direction, None, &mut |t| request_focus(t, direction)).unwrap_or(false)
    }

    /// Searches `direction` (Compose's FocusOwnerImpl `focusSearch`): first the active target's custom
    /// destination (when it is in the window on top), then from the root; none if called off.
    pub(crate) fn focus_search(
        &self,
        direction: FocusDirection,
        focused_rect: Option<Rect>,
        on_found: &mut dyn FnMut(&NodeRc) -> bool,
    ) -> Option<bool> {
        let root = self.root_focus_node()?;
        let layout_direction = self.layout_direction();
        let source = scoped_active(&root);
        if let Some(source) = &source {
            let custom = custom_focus_search(source, direction, layout_direction);
            if custom == super::FocusRequester::Cancel {
                return None;
            } else if custom == super::FocusRequester::Redirect {
                return scoped_active(&root).map(|a| on_found(&a));
            } else if custom != super::FocusRequester::Default {
                return Some(custom.find_focus_target_for_search(on_found));
            }
        }
        focus_search(&root, direction, layout_direction, focused_rect, &mut |t| {
            if source.as_ref().is_some_and(|s| Rc::ptr_eq(s, t)) {
                false
            } else if Rc::ptr_eq(t, &root) {
                panic!("Focus search landed at the root.")
            } else {
                on_found(t)
            }
        })
    }

    /// The active target's focus rect in its root's coordinates (Compose's `getFocusRect`).
    pub fn focus_rect(&self) -> Option<Rect> {
        self.active_focus_target().map(|t| focus_rect(&t))
    }

    /// Whether a target below the root can take the focus (Compose's `hasFocusableContent`).
    pub fn has_focusable_content(&self) -> bool {
        let Some(root) = self.root_focus_node() else { return false };
        let mut found = false;
        NodeCx::new(&root).visit_subtree(NodeKind::FocusTarget, &mut |c| {
            if c.is_attached() && is_target(&c.cell) && fetch_focus_properties(&c.cell).can_focus {
                found = true;
                return false;
            }
            true
        });
        found
    }

    // ---- Keys ----

    /// Sends a key event along the focused path (Compose's `dispatchKeyEvent`): previews from the outermost key
    /// node down to the focused one, `on_focused_item`, then the events from the focused one up. Whether a node
    /// took it. A key up without its down is dropped.
    pub(crate) fn dispatch_key_event(&self, event: &KeyEvent, on_focused_item: &mut dyn FnMut() -> bool) -> bool {
        self.process_invalidations();
        if !self.validate_key_event(event) {
            return false;
        }
        let Some(root) = self.root_focus_node() else { return false };
        let target = scoped_active(&root);
        let key_node = target
            .as_ref()
            .and_then(last_local_key_input_node)
            .or_else(|| target.as_ref().and_then(|t| nearest_ancestor_including_self(t, NodeKind::KeyInput)))
            .or_else(|| super::nearest_ancestor(&root, NodeKind::KeyInput));
        let Some(key_node) = key_node else { return false };
        let outer = ancestors(&key_node, NodeKind::KeyInput);
        for node in outer.iter().rev() {
            if key_pre(node, event) {
                return true;
            }
        }
        if dispatch_kind(&key_node, NodeKind::KeyInput, &mut |n| key_pre(n, event)) {
            return true;
        }
        if on_focused_item() {
            return true;
        }
        if dispatch_kind(&key_node, NodeKind::KeyInput, &mut |n| key_post(n, event)) {
            return true;
        }
        for node in &outer {
            if key_post(node, event) {
                return true;
            }
        }
        false
    }

    /// Sends a hardware key a soft keyboard would see first along the focused path (Compose's
    /// `dispatchInterceptedSoftKeyboardEvent`).
    pub(crate) fn dispatch_intercepted_soft_keyboard_event(&self, event: &KeyEvent) -> bool {
        self.process_invalidations();
        let Some(root) = self.root_focus_node() else { return false };
        let Some(active) = scoped_active(&root) else { return false };
        let Some(node) = nearest_ancestor_including_self(&active, NodeKind::SoftKeyboardKeyInput) else { return false };
        let outer = ancestors(&node, NodeKind::SoftKeyboardKeyInput);
        let pre = |n: &NodeRc| {
            let mut x = n.node.borrow_mut();
            x.as_soft_keyboard_interception().is_some_and(|s| s.on_pre_intercept_key_before_soft_keyboard(event))
        };
        let post = |n: &NodeRc| {
            let mut x = n.node.borrow_mut();
            x.as_soft_keyboard_interception().is_some_and(|s| s.on_intercept_key_before_soft_keyboard(event))
        };
        for n in outer.iter().rev() {
            if pre(n) {
                return true;
            }
        }
        if dispatch_kind(&node, NodeKind::SoftKeyboardKeyInput, &mut |n| pre(n)) {
            return true;
        }
        if dispatch_kind(&node, NodeKind::SoftKeyboardKeyInput, &mut |n| post(n)) {
            return true;
        }
        outer.iter().any(post)
    }

    /// Sends a rotary scroll event along the focused path (Compose's `dispatchRotaryEvent`).
    pub(crate) fn dispatch_rotary_event(
        &self,
        event: &RotaryScrollEvent,
        on_focused_item: &mut dyn FnMut() -> bool,
    ) -> bool {
        self.process_invalidations();
        let Some(root) = self.root_focus_node() else { return false };
        let Some(active) = scoped_active(&root) else { return false };
        let Some(node) = nearest_ancestor_including_self(&active, NodeKind::RotaryInput) else { return false };
        let outer = ancestors(&node, NodeKind::RotaryInput);
        let pre = |n: &NodeRc| {
            let mut x = n.node.borrow_mut();
            x.as_rotary_input().is_some_and(|r| r.on_pre_rotary_scroll_event(event))
        };
        let post = |n: &NodeRc| {
            let mut x = n.node.borrow_mut();
            x.as_rotary_input().is_some_and(|r| r.on_rotary_scroll_event(event))
        };
        for n in outer.iter().rev() {
            if pre(n) {
                return true;
            }
        }
        if dispatch_kind(&node, NodeKind::RotaryInput, &mut |n| pre(n)) || on_focused_item() {
            return true;
        }
        if dispatch_kind(&node, NodeKind::RotaryInput, &mut |n| post(n)) {
            return true;
        }
        outer.iter().any(post)
    }

    /// Sends an indirect pointer event along the focused path (Compose's `dispatchIndirectPointerEvent`): the
    /// nearest indirect pointer node at or above the focused target and those above it, in the Initial pass from the
    /// root down, the Main pass from it up, the Final pass down. Whether a node consumed a change.
    pub(crate) fn dispatch_indirect_pointer_event(&self, event: &IndirectPointerEvent) -> bool {
        self.process_invalidations();
        let Some(node) = self.focused_indirect_pointer_node() else { return false };
        let outer = ancestors(&node, NodeKind::IndirectPointerInput);
        let send = |n: &NodeRc, pass: PointerEventPass| {
            dispatch_kind(n, NodeKind::IndirectPointerInput, &mut |c| {
                let mut x = c.node.borrow_mut();
                if let Some(i) = x.as_indirect_pointer_input() {
                    i.on_indirect_pointer_event(event, pass);
                }
                false
            });
        };
        for n in outer.iter().rev() {
            send(n, PointerEventPass::Initial);
        }
        send(&node, PointerEventPass::Initial);
        send(&node, PointerEventPass::Main);
        for n in &outer {
            send(n, PointerEventPass::Main);
        }
        for n in outer.iter().rev() {
            send(n, PointerEventPass::Final);
        }
        send(&node, PointerEventPass::Final);
        event.changes.iter().any(|c| c.is_consumed())
    }

    /// Cancels the indirect pointer stream along the focused path, from the nearest node up (Compose's
    /// `dispatchIndirectPointerCancel`).
    pub(crate) fn dispatch_indirect_pointer_cancel(&self) {
        let Some(node) = self.focused_indirect_pointer_node() else { return };
        let mut path = vec![node.clone()];
        path.extend(ancestors(&node, NodeKind::IndirectPointerInput));
        for n in &path {
            dispatch_kind(n, NodeKind::IndirectPointerInput, &mut |c| {
                let mut x = c.node.borrow_mut();
                if let Some(i) = x.as_indirect_pointer_input() {
                    i.on_cancel_indirect_pointer_input();
                }
                false
            });
        }
    }

    /// The nearest indirect pointer node at or above the focused target.
    fn focused_indirect_pointer_node(&self) -> Option<NodeRc> {
        let root = self.root_focus_node()?;
        let active = scoped_active(&root)?;
        nearest_ancestor_including_self(&active, NodeKind::IndirectPointerInput)
    }

    /// Compose's `validateKeyEvent`: a key up is passed on only if its down was.
    fn validate_key_event(&self, event: &KeyEvent) -> bool {
        let code = event.key.0;
        let mut down = self.keys_down.borrow_mut();
        match event.r#type {
            KeyEventType::KeyDown => {
                if !down.contains(&code) {
                    down.push(code);
                }
                true
            }
            KeyEventType::KeyUp => match down.iter().position(|k| *k == code) {
                Some(i) => {
                    down.remove(i);
                    true
                }
                None => false,
            },
            KeyEventType::Unknown => true,
        }
    }

    // ---- Invalidations (Compose's FocusInvalidationManager) ----

    pub(crate) fn schedule_target_invalidation(&self, cell: &NodeRc) {
        let w = Rc::downgrade(cell);
        let mut list = self.pending_targets.borrow_mut();
        if !list.iter().any(|x| x.ptr_eq(&w)) {
            list.push(w);
        }
        self.pending.set(true);
    }

    pub(crate) fn schedule_event_invalidation(&self, cell: &NodeRc) {
        let w = Rc::downgrade(cell);
        let mut list = self.pending_events.borrow_mut();
        if !list.iter().any(|x| x.ptr_eq(&w)) {
            list.push(w);
        }
        self.pending.set(true);
    }

    /// Whether invalidations wait to be worked off.
    pub fn has_pending_invalidation(&self) -> bool {
        self.pending.get()
    }

    /// Works off the invalidations (Compose's `invalidateNodes`): the active target's properties checked again,
    /// the event nodes on its path told its state (ActiveParent past the first target), the others Inactive.
    pub fn process_invalidations(&self) {
        if !self.pending.replace(false) {
            return;
        }
        let targets: Vec<WeakCell> = std::mem::take(&mut *self.pending_targets.borrow_mut());
        let mut events: Vec<NodeRc> =
            std::mem::take(&mut *self.pending_events.borrow_mut()).iter().filter_map(Weak::upgrade).collect();
        match self.active_focus_target() {
            None => {
                for e in &events {
                    send_focus_event(e, FocusState::Inactive);
                }
            }
            Some(active) => {
                if targets.iter().any(|t| t.upgrade().is_some_and(|t| Rc::ptr_eq(&t, &active))) {
                    invalidate_focus(&active);
                }
                let state = focus_state(&active);
                let mut traversed_targets = 0;
                NodeCx::new(&active).visit_ancestors(NodeKind::FocusTarget.or(NodeKind::FocusEvent), true, &mut |c| {
                    let self_kinds = c.cell.meta.self_kind_set.get();
                    if NodeKind::FocusTarget.is_in(self_kinds) {
                        traversed_targets += 1;
                    }
                    if NodeKind::FocusEvent.is_in(self_kinds)
                        && let Some(i) = events.iter().position(|e| Rc::ptr_eq(e, &c.cell))
                    {
                        let e = events.remove(i);
                        send_focus_event(&e, if traversed_targets <= 1 { state } else { FocusState::ActiveParent });
                    }
                    true
                });
                for e in &events {
                    send_focus_event(e, FocusState::Inactive);
                }
            }
        }
    }

    /// Forgets nodes that are gone.
    pub(crate) fn prune(&self) {
        self.pending_targets.borrow_mut().retain(|w| w.strong_count() > 0);
        self.pending_events.borrow_mut().retain(|w| w.strong_count() > 0);
        if self.active.borrow().as_ref().is_some_and(|w| w.strong_count() == 0) {
            *self.active.borrow_mut() = None;
        }
    }
}

/// The root target of the window `root` is the root layout of: the first focus target of its chain.
pub(crate) fn root_target_of(root: &Rc<LayoutNode>) -> Option<NodeRc> {
    let mut found = None;
    root.head_to_tail(NodeKind::FocusTarget, &mut |c| {
        if found.is_none() && is_target(c) {
            found = Some(c.clone());
        }
    });
    found
}

/// The key input node after the focused target in its own chain, before the next target (Compose's
/// `lastLocalKeyInputNode`).
fn last_local_key_input_node(target: &NodeRc) -> Option<NodeRc> {
    let layout = target.layout_node()?;
    let nodes = layout.chain_nodes();
    let me = chain_node_of(target);
    let start = nodes.iter().position(|n| Rc::ptr_eq(n, &me))? + 1;
    let mut found = None;
    for n in &nodes[start..] {
        if n.is_kind(NodeKind::FocusTarget) {
            break;
        }
        if n.is_kind(NodeKind::KeyInput) {
            found = Some(n.clone());
        }
    }
    found
}

/// The chain's node `cell` is (or is a delegate of).
fn chain_node_of(cell: &NodeRc) -> NodeRc {
    let mut c = cell.clone();
    loop {
        let up = c.meta.delegator.borrow().as_ref().and_then(Weak::upgrade);
        match up {
            Some(d) => c = d,
            None => return c,
        }
    }
}

fn nearest_ancestor_including_self(cell: &NodeRc, kind: NodeKind) -> Option<NodeRc> {
    let mut found = None;
    NodeCx::new(cell).visit_ancestors(kind, true, &mut |c| {
        found = Some(c.cell.clone());
        false
    });
    found
}

/// Calls `f` on the nodes of `kind` of `cell` (itself or its delegates) until one says true.
fn dispatch_kind(cell: &NodeRc, kind: NodeKind, f: &mut dyn FnMut(&NodeRc) -> bool) -> bool {
    let mut cells = Vec::new();
    dispatch_for_kind(cell, kind, &mut |c| cells.push(c.clone()));
    cells.iter().any(f)
}

/// A key node's preview of `event` (what it sets off run once it is released): whether it took it.
fn key_pre(cell: &NodeRc, event: &KeyEvent) -> bool {
    if let Some((pre, _)) = crate::input::key_handlers(cell) {
        return pre.is_some_and(|h| h(event));
    }
    let mut taken = false;
    crate::input::call_node(|| {
        let mut n = cell.node.borrow_mut();
        taken = n.as_key_input().is_some_and(|k| k.on_pre_key_event(event));
    });
    taken
}

/// A key node's `event` (what it sets off run once it is released): whether it took it.
fn key_post(cell: &NodeRc, event: &KeyEvent) -> bool {
    if let Some((_, post)) = crate::input::key_handlers(cell) {
        return post.is_some_and(|h| h(event));
    }
    let mut taken = false;
    crate::input::call_node(|| {
        let mut n = cell.node.borrow_mut();
        taken = n.as_key_input().is_some_and(|k| k.on_key_event(event));
    });
    taken
}

/// The indirect pointer nodes from `cell` up.
fn indirect_pointer_path(cell: &NodeRc) -> Vec<NodeRc> {
    let mut out = Vec::new();
    NodeCx::new(cell).visit_ancestors(NodeKind::IndirectPointerInput, true, &mut |c| {
        out.push(c.cell.clone());
        true
    });
    out
}

impl FocusDirection {
    /// Whether it is a 1D direction (Next, Previous).
    pub fn is_1d(self) -> bool {
        matches!(self, FocusDirection::Next | FocusDirection::Previous)
    }
}

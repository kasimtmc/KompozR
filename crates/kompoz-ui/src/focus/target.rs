//! The focus target node (Compose's FocusTargetNode, FocusTargetModifierNode and FocusTransactions): what can
//! take the focus, its state (worked out from the owner's active node), its properties (from the property nodes
//! above it up to the next target), and the transactions moving the focus to it or away.

use super::properties::{FocusEnterExitScope, FocusProperties};
use super::requester::FocusRequester;
use super::{
    FocusDirection, FocusOwner, FocusState, Focusability, ancestors, children, focus_owner_of, nearest_ancestor,
};
use crate::layout::LayoutCoordinates;
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeKind, NodeRc, ObserverModifierNode};
use crate::node_capabilities;
use kompoz_graphics::Rect;
use std::rc::Rc;

/// What a focus target's `on_focus_change` is told: its state before and after.
pub type FocusChangeCallback = Rc<dyn Fn(FocusState, FocusState)>;

/// A node that can take the focus (Compose's FocusTargetNode): `Modifier.focus_target()`'s, or one a node of its
/// own delegates to (`cx.delegate(FocusTargetNode::new(…))`, Compose's `FocusTargetModifierNode(…)` factory).
/// Its state, whether it can take the focus, moving the focus to it: [`NodeCx`]'s focus methods on its context
/// (`focus_state`, `request_focus`, `set_focusability`, …).
pub struct FocusTargetNode {
    pub(crate) focusability: Focusability,
    pub(crate) on_focus_change: Option<FocusChangeCallback>,
    pub(crate) is_processing_custom_exit: bool,
    pub(crate) is_processing_custom_enter: bool,
    /// The composite key of the child that had the focus when it left (Compose's `previouslyFocusedChildHash`):
    /// where a focus restorer brings it back.
    pub(crate) previously_focused_child_hash: Option<u64>,
    /// Where that child is saved with the host's state (Compose's `focusRestorationEntry`).
    pub(crate) focus_restoration_entry: Option<kompoz_runtime::SaveableStateRegistryEntry>,
    /// Its context, kept from its attach (its observed reads and detach work on it).
    cx: Option<NodeCx>,
}

impl FocusTargetNode {
    /// A focus target (Compose's `FocusTargetModifierNode(focusability, onFocusChange)`): `on_focus_change` is told
    /// its state before and after each change.
    pub fn new(focusability: Focusability, on_focus_change: Option<FocusChangeCallback>) -> FocusTargetNode {
        FocusTargetNode {
            focusability,
            on_focus_change,
            is_processing_custom_exit: false,
            is_processing_custom_enter: false,
            previously_focused_child_hash: None,
            focus_restoration_entry: None,
            cx: None,
        }
    }

    /// How it can take the focus.
    pub fn focusability(&self) -> Focusability {
        self.focusability
    }
}

impl Default for FocusTargetNode {
    fn default() -> Self {
        FocusTargetNode::new(Focusability::Always, None)
    }
}

impl ModifierNode for FocusTargetNode {
    node_capabilities!(focus_target, observer);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        self.cx = Some(cx.clone());
    }

    /// Leaving with the focus clears it (Compose's `onDetach`: the whole focus, the owner's own focus kept). Its
    /// own callback is told here (it is borrowed while the focus is cleared), then the event nodes above.
    fn on_detach(&mut self, cx: &NodeCx) {
        let cell = cx.cell.clone();
        let state = focus_state(&cell);
        if matches!(state, FocusState::Active | FocusState::Captured)
            && let Some(owner) = focus_owner_of(&cell)
        {
            if let Some(callback) = self.on_focus_change.clone() {
                callback(state, FocusState::Inactive);
            }
            owner.clear_focus_with(true, true, false, FocusDirection::Exit);
        }
        self.previously_focused_child_hash = None;
        if let Some(entry) = self.focus_restoration_entry.take() {
            entry.unregister();
        }
        self.cx = None;
    }

    /// The layout reused for other content with the focus in it: the focus cleared (Compose's `onReset`).
    fn on_reset(&mut self) {
        let Some(cx) = self.cx.clone() else { return };
        let cell = cx.cell.clone();
        let state = focus_state(&cell);
        if state.is_focused()
            && let Some(owner) = focus_owner_of(&cell)
        {
            if let Some(callback) = self.on_focus_change.clone() {
                callback(state, FocusState::Inactive);
            }
            owner.clear_focus_with(true, true, true, FocusDirection::Exit);
        }
    }
}

impl super::FocusTargetModifierNode for FocusTargetNode {
    fn focus_target_node(&mut self) -> &mut FocusTargetNode {
        self
    }
}

impl ObserverModifierNode for FocusTargetNode {
    /// What its focus properties read changed: worked out again (Compose's `invalidateFocus`), once the node is
    /// released.
    fn on_observed_reads_changed(&mut self) {
        if let Some(cx) = &self.cx {
            schedule_target_invalidation(&cx.cell);
        }
    }
}

/// `Modifier.focus_target()`'s element (Compose's FocusTargetElement).
#[derive(PartialEq)]
struct FocusTargetElement;

impl ModifierNodeElement for FocusTargetElement {
    type Node = FocusTargetNode;
    fn create(&self) -> FocusTargetNode {
        FocusTargetNode::default()
    }
    fn update(&self, _node: &mut FocusTargetNode) {}
    fn name(&self) -> &'static str {
        "focusTarget"
    }
}

/// The root of a window's focus (Compose's FocusOwnerImpl.rootFocusNode): a target that never takes the focus.
#[derive(PartialEq)]
pub(crate) struct RootFocusTargetElement;

impl ModifierNodeElement for RootFocusTargetElement {
    type Node = FocusTargetNode;
    fn create(&self) -> FocusTargetNode {
        FocusTargetNode::new(Focusability::Never, None)
    }
    fn update(&self, _node: &mut FocusTargetNode) {}
    fn name(&self) -> &'static str {
        "RootFocusTarget"
    }
}

impl Modifier {
    /// Makes the layout a focus target: it can take the focus, and its focus state reaches the focus event nodes
    /// above it (Compose's `Modifier.focusTarget`).
    pub fn focus_target(self) -> Modifier {
        self.then_element(FocusTargetElement)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The target's state and properties

/// Runs `f` on the focus target node `cell` is (a delegate's own cell); none if it is borrowed (it is the one
/// being detached or reset, which does its part itself).
pub(crate) fn with_target<R>(cell: &NodeRc, f: impl FnOnce(&mut FocusTargetNode) -> R) -> Option<R> {
    let mut node = cell.node.try_borrow_mut().ok()?;
    node.as_focus_target().map(|t| f(t.focus_target_node()))
}

/// Whether `cell` is a focus target node itself.
pub(crate) fn is_target(cell: &NodeRc) -> bool {
    NodeKind::FocusTarget.is_in(cell.meta.self_kind_set.get())
}

/// Its focus state (Compose's `focusState`): from the owner's active node, Active (or Captured) if it is that,
/// ActiveParent if it is above it.
pub(crate) fn focus_state(cell: &NodeRc) -> FocusState {
    if !cell.meta.attached.get() {
        return FocusState::Inactive;
    }
    let Some(owner) = focus_owner_of(cell) else { return FocusState::Inactive };
    let Some(active) = owner.active_focus_target() else { return FocusState::Inactive };
    if Rc::ptr_eq(&active, cell) {
        return if owner.is_focus_captured() { FocusState::Captured } else { FocusState::Active };
    }
    if active.meta.attached.get() && ancestors(&active, NodeKind::FocusTarget).iter().any(|a| Rc::ptr_eq(a, cell)) {
        return FocusState::ActiveParent;
    }
    FocusState::Inactive
}

/// Its focus properties (Compose's `fetchFocusProperties`): whether it can take the focus by its focusability,
/// then each property node from it up to the next target (the nearest last, so the outermost has the last word).
pub(crate) fn fetch_focus_properties(cell: &NodeRc) -> FocusProperties {
    let mut properties = FocusProperties::default();
    let focusability = with_target(cell, |t| t.focusability).unwrap_or(Focusability::Always);
    properties.can_focus = focusability.can_focus(&NodeCx::new(cell));
    let mut nodes = Vec::new();
    NodeCx::new(cell).visit_ancestors(NodeKind::FocusProperties.or(NodeKind::FocusTarget), true, &mut |c| {
        if !Rc::ptr_eq(&c.cell, cell) && c.cell.is_kind(NodeKind::FocusTarget) && is_target(&c.cell) {
            return false;
        }
        if NodeKind::FocusProperties.is_in(c.cell.meta.self_kind_set.get()) {
            nodes.push(c.cell.clone());
        }
        true
    });
    for node in nodes {
        let mut n = node.node.borrow_mut();
        if let Some(p) = n.as_focus_properties() {
            p.apply_focus_properties(&mut properties);
        }
    }
    properties
}

/// Its focus rect in `relative` (Compose's `fetchFocusRect`): the one its properties give, else its bounds.
pub(crate) fn fetch_focus_rect(cell: &NodeRc, relative: Option<&LayoutCoordinates>) -> Rect {
    let Some(me) = NodeCx::new(cell).coordinates() else { return Rect::Zero };
    if let Some(custom) = fetch_focus_properties(cell).focus_rect {
        return match relative {
            Some(r) => custom.translate(r.local_position_of(&me, kompoz_graphics::Offset::Zero)),
            None => custom,
        };
    }
    match relative {
        Some(r) => r.local_bounding_box_of(&me, false),
        None => Rect::from_offset_size(kompoz_graphics::Offset::Zero, me.size().to_size()),
    }
}

/// Its state told to it and to the focus event nodes from it up to the next target, stopping if the focus moves
/// meanwhile (Compose's `dispatchFocusCallbacks`).
pub(crate) fn dispatch_focus_callbacks(cell: &NodeRc, previous: FocusState, new: FocusState) {
    let Some(owner) = focus_owner_of(cell) else { return };
    let active = owner.active_focus_target();
    if previous != new
        && let Some(callback) = with_target(cell, |t| t.on_focus_change.clone()).flatten()
    {
        callback(previous, new);
    }
    let mut nodes = Vec::new();
    NodeCx::new(cell).visit_ancestors(NodeKind::FocusEvent.or(NodeKind::FocusTarget), true, &mut |c| {
        if !Rc::ptr_eq(&c.cell, cell) && is_target(&c.cell) {
            return false;
        }
        if NodeKind::FocusEvent.is_in(c.cell.meta.self_kind_set.get()) {
            nodes.push(c.cell.clone());
        }
        true
    });
    for node in nodes {
        let now = owner.active_focus_target();
        let same = match (&active, &now) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            return;
        }
        send_focus_event(&node, new);
    }
}

/// Tells a focus event node `state`, its user code run once it is released.
pub(crate) fn send_focus_event(cell: &NodeRc, state: FocusState) {
    crate::input::call_node(|| {
        let mut n = cell.node.borrow_mut();
        if let Some(e) = n.as_focus_event() {
            e.on_focus_event(state);
        }
    });
}

/// Its nearest descendant target that has the focus or is above it (Compose's `activeChild`).
pub(crate) fn active_child(cell: &NodeRc) -> Option<NodeRc> {
    if !cell.meta.attached.get() {
        return None;
    }
    children(cell, NodeKind::FocusTarget)
        .into_iter()
        .find(|c| c.meta.attached.get() && focus_state(c) != FocusState::Inactive)
}

/// The owner's active node, if it is attached (Compose's `findActiveFocusNode`).
pub(crate) fn find_active_focus_node(cell: &NodeRc) -> Option<NodeRc> {
    focus_owner_of(cell)?.active_focus_target()
}

/// Whether focus search may go to it: its layout placed and attached (Compose's `isEligibleForFocusSearch`).
pub(crate) fn is_eligible_for_focus_search(cell: &NodeRc) -> bool {
    // (not in a layout taking no input: KompozR's PointerInputModifierNode::blocks_input)
    cell.layout_node().is_some_and(|n| n.is_attached() && n.is_placed() && !crate::input::input_blocked_at(&n))
}

/// Its focus rect in its root's coordinates (Compose's `focusRect`).
pub(crate) fn focus_rect(cell: &NodeRc) -> Rect {
    if !cell.meta.attached.get() {
        return Rect::Zero;
    }
    let Some(me) = NodeCx::new(cell).coordinates() else { return Rect::Zero };
    let root = me.find_root_coordinates();
    if !root.is_attached() {
        return Rect::Zero;
    }
    fetch_focus_rect(cell, Some(&root))
}

/// The focus cleared if it is here and its properties no longer let it be (Compose's `invalidateFocus`).
pub(crate) fn invalidate_focus(cell: &NodeRc) {
    if matches!(focus_state(cell), FocusState::Active | FocusState::Captured) {
        let cx = NodeCx::new(cell);
        let properties = cx.observe_reads(|| fetch_focus_properties(cell));
        if !properties.can_focus
            && let Some(owner) = focus_owner_of(cell)
        {
            owner.clear_focus(true);
        }
    }
}

/// Its invalidation scheduled with its owner (Compose's `invalidateFocusTarget`).
pub(crate) fn schedule_target_invalidation(cell: &NodeRc) {
    if let Some(owner) = focus_owner_of(cell) {
        owner.schedule_target_invalidation(cell);
    }
}

/// Sets how it can take the focus; if it has the focus and no longer can, the focus is cleared (Compose's
/// `focusability` setter).
pub(crate) fn set_focusability(cell: &NodeRc, focusability: Focusability) {
    let changed = with_target(cell, |t| {
        let changed = t.focusability != focusability;
        t.focusability = focusability;
        changed
    })
    .unwrap_or(false);
    if !changed || !cell.meta.attached.get() {
        return;
    }
    let Some(owner) = focus_owner_of(cell) else { return };
    let is_active = owner.active_focus_target().is_some_and(|a| Rc::ptr_eq(&a, cell));
    if is_active && !focusability.can_focus(&NodeCx::new(cell)) && prepare_to_clear_focus(cell, true, true) {
        let previous = owner.active_focus_target();
        owner.set_active(None);
        if let Some(p) = previous {
            dispatch_focus_callbacks(&p, FocusState::Active, FocusState::Inactive);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Transactions (Compose's FocusTransactions)

/// Requests the focus for it (Compose's `FocusTargetNode.requestFocus`): itself if it can take it (its custom
/// enter and exit run first), else the child its enter search finds.
pub(crate) fn request_focus(cell: &NodeRc, direction: FocusDirection) -> bool {
    if fetch_focus_properties(cell).can_focus {
        assign_focus(cell, direction)
    } else {
        super::search::find_child_corresponding_to_focus_enter(cell, direction, &mut |c| assign_focus(c, direction))
    }
}

fn assign_focus(cell: &NodeRc, direction: FocusDirection) -> bool {
    match perform_custom_request_focus(cell, direction) {
        CustomDestinationResult::None => perform_request_focus(cell),
        CustomDestinationResult::Redirected => true,
        CustomDestinationResult::Cancelled | CustomDestinationResult::RedirectCancelled => false,
    }
}

/// Moves the focus here (Compose's `performRequestFocus`): the focus cleared from the node that had it (unless it
/// is above this one), this granted it, then the callbacks of each target whose state changed, outermost first;
/// the targets above it note where the focus went (for focus restorers).
pub(crate) fn perform_request_focus(cell: &NodeRc) -> bool {
    let Some(owner) = focus_owner_of(cell) else { return false };
    let previous_active = owner.active_focus_target();
    let previous_state = focus_state(cell);
    if previous_active.as_ref().is_some_and(|p| Rc::ptr_eq(p, cell)) {
        dispatch_focus_callbacks(cell, previous_state, previous_state);
        return true;
    }
    let mut previous_ancestors: Option<Vec<NodeRc>> =
        previous_active.as_ref().map(|p| ancestors(p, NodeKind::FocusTarget));
    let mut should_clear_previous = true;
    let mut common = Vec::new();
    let mut new_ancestors = Vec::new();
    for a in ancestors(cell, NodeKind::FocusTarget) {
        let removed = match previous_ancestors.as_mut() {
            Some(list) => match list.iter().position(|x| Rc::ptr_eq(x, &a)) {
                Some(i) => {
                    list.remove(i);
                    true
                }
                None => false,
            },
            None => false,
        };
        if removed {
            common.push(a.clone());
        } else {
            new_ancestors.push(a.clone());
        }
        if previous_active.as_ref().is_some_and(|p| Rc::ptr_eq(p, &a)) {
            should_clear_previous = false;
        }
    }
    if should_clear_previous
        && let Some(p) = &previous_active
        && !prepare_to_clear_focus(p, false, true)
    {
        return false;
    }
    grant_focus(cell, &owner);
    let is_this = |owner: &FocusOwner| owner.active_focus_target().is_some_and(|a| Rc::ptr_eq(&a, cell));
    if should_clear_previous && let Some(p) = &previous_active {
        dispatch_focus_callbacks(p, FocusState::Active, FocusState::Inactive);
    }
    if let Some(list) = &previous_ancestors {
        for a in list.iter().rev() {
            if !is_this(&owner) {
                return false;
            }
            dispatch_focus_callbacks(a, FocusState::ActiveParent, FocusState::Inactive);
        }
    }
    for a in new_ancestors.iter().rev() {
        if !is_this(&owner) {
            return false;
        }
        let before = if previous_active.as_ref().is_some_and(|p| Rc::ptr_eq(p, a)) {
            FocusState::Active
        } else {
            FocusState::Inactive
        };
        dispatch_focus_callbacks(a, before, FocusState::ActiveParent);
    }
    if !is_this(&owner) {
        return false;
    }
    dispatch_focus_callbacks(cell, previous_state, FocusState::Active);
    if !is_this(&owner) {
        return false;
    }
    // Where the focus went, noted by the targets above up to the nearest one the focus was already in.
    let closest_common = common.last().cloned();
    for a in ancestors(cell, NodeKind::FocusTarget) {
        super::restorer::save_focused_child(&a);
        if closest_common.as_ref().is_some_and(|c| Rc::ptr_eq(c, &a)) {
            break;
        }
    }
    true
}

/// Captures the focus here, if it has it (Compose's `captureFocus`).
pub(crate) fn capture_focus(cell: &NodeRc) -> bool {
    match focus_state(cell) {
        FocusState::Active => {
            if let Some(owner) = focus_owner_of(cell) {
                owner.set_captured(true);
            }
            dispatch_focus_callbacks(cell, FocusState::Active, FocusState::Captured);
            true
        }
        FocusState::Captured => true,
        _ => false,
    }
}

/// Frees a captured focus here (Compose's `freeFocus`).
pub(crate) fn free_focus(cell: &NodeRc) -> bool {
    match focus_state(cell) {
        FocusState::Captured => {
            if let Some(owner) = focus_owner_of(cell) {
                owner.set_captured(false);
            }
            dispatch_focus_callbacks(cell, FocusState::Captured, FocusState::Active);
            true
        }
        FocusState::Active => true,
        _ => false,
    }
}

/// Whether the focus can be taken away from here, its children's cleared (Compose's `prepareToClearFocus`).
pub(crate) fn prepare_to_clear_focus(cell: &NodeRc, forced: bool, refresh_focus_events: bool) -> bool {
    match focus_state(cell) {
        FocusState::Active | FocusState::Inactive => true,
        FocusState::Captured => forced,
        FocusState::ActiveParent => {
            let cleared = active_child(cell).is_none_or(|c| prepare_to_clear_focus(&c, forced, refresh_focus_events));
            if cleared && refresh_focus_events {
                dispatch_focus_callbacks(cell, FocusState::ActiveParent, FocusState::Inactive);
            }
            cleared
        }
    }
}

fn grant_focus(cell: &NodeRc, owner: &FocusOwner) {
    NodeCx::new(cell).observe_reads(|| fetch_focus_properties(cell));
    if matches!(focus_state(cell), FocusState::Inactive | FocusState::ActiveParent) {
        owner.set_active(Some(cell));
    }
}

/// What a custom enter or exit came to (Compose's CustomDestinationResult).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CustomDestinationResult {
    None,
    Cancelled,
    Redirected,
    RedirectCancelled,
}

/// Runs the custom exits and enters on the way to it (Compose's `performCustomRequestFocus`).
pub(crate) fn perform_custom_request_focus(cell: &NodeRc, direction: FocusDirection) -> CustomDestinationResult {
    match focus_state(cell) {
        FocusState::Active | FocusState::Captured => CustomDestinationResult::None,
        FocusState::ActiveParent => match active_child(cell) {
            Some(child) => perform_custom_clear_focus(&child, direction),
            None => panic!("ActiveParent with no focused child"),
        },
        FocusState::Inactive => {
            let Some(parent) = nearest_ancestor(cell, NodeKind::FocusTarget) else {
                return CustomDestinationResult::None;
            };
            match focus_state(&parent) {
                FocusState::Captured => CustomDestinationResult::Cancelled,
                FocusState::ActiveParent => perform_custom_request_focus(&parent, direction),
                FocusState::Active => perform_custom_enter(&parent, direction),
                FocusState::Inactive => match perform_custom_request_focus(&parent, direction) {
                    CustomDestinationResult::None => perform_custom_enter(&parent, direction),
                    other => other,
                },
            }
        }
    }
}

/// Runs the custom exits on the way out of it (Compose's `performCustomClearFocus`).
pub(crate) fn perform_custom_clear_focus(cell: &NodeRc, direction: FocusDirection) -> CustomDestinationResult {
    match focus_state(cell) {
        FocusState::Active | FocusState::Inactive => CustomDestinationResult::None,
        FocusState::Captured => CustomDestinationResult::Cancelled,
        FocusState::ActiveParent => {
            let child = active_child(cell).expect("ActiveParent with no focused child");
            match perform_custom_clear_focus(&child, direction) {
                CustomDestinationResult::None => perform_custom_exit(cell, direction),
                other => other,
            }
        }
    }
}

fn custom_destination(requester: FocusRequester) -> CustomDestinationResult {
    if requester == FocusRequester::Cancel {
        CustomDestinationResult::Cancelled
    } else if requester == FocusRequester::Redirect || requester.request_focus() {
        CustomDestinationResult::Redirected
    } else {
        CustomDestinationResult::RedirectCancelled
    }
}

fn perform_custom_enter(cell: &NodeRc, direction: FocusDirection) -> CustomDestinationResult {
    fetch_custom_enter(cell, direction).map_or(CustomDestinationResult::None, custom_destination)
}

fn perform_custom_exit(cell: &NodeRc, direction: FocusDirection) -> CustomDestinationResult {
    fetch_custom_exit(cell, direction).map_or(CustomDestinationResult::None, custom_destination)
}

/// Runs its `on_enter` (not while it runs already): Cancel if it cancelled, Redirect if it moved the focus, none
/// otherwise (Compose's `fetchCustomEnter`).
pub(crate) fn fetch_custom_enter(cell: &NodeRc, direction: FocusDirection) -> Option<FocusRequester> {
    let busy = with_target(cell, |t| std::mem::replace(&mut t.is_processing_custom_enter, true)).unwrap_or(true);
    if busy {
        return None;
    }
    let result = fetch_custom_enter_or_exit(cell, direction, true);
    with_target(cell, |t| t.is_processing_custom_enter = false);
    result
}

/// Runs its `on_exit` (Compose's `fetchCustomExit`).
pub(crate) fn fetch_custom_exit(cell: &NodeRc, direction: FocusDirection) -> Option<FocusRequester> {
    let busy = with_target(cell, |t| std::mem::replace(&mut t.is_processing_custom_exit, true)).unwrap_or(true);
    if busy {
        return None;
    }
    let result = fetch_custom_enter_or_exit(cell, direction, false);
    with_target(cell, |t| t.is_processing_custom_exit = false);
    result
}

fn fetch_custom_enter_or_exit(cell: &NodeRc, direction: FocusDirection, enter: bool) -> Option<FocusRequester> {
    let properties = fetch_focus_properties(cell);
    let owner = focus_owner_of(cell)?;
    let mut scope = FocusEnterExitScope::new(direction);
    let before = owner.active_focus_target();
    let block = if enter { properties.on_enter } else { properties.on_exit };
    if let Some(block) = block {
        block(&mut scope);
    }
    let after = owner.active_focus_target();
    if scope.is_canceled() {
        Some(FocusRequester::Cancel)
    } else if let Some(after) = after
        && before.as_ref().is_none_or(|b| !Rc::ptr_eq(b, &after))
    {
        Some(FocusRequester::Redirect)
    } else {
        None
    }
}

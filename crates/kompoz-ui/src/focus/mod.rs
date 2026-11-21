//! Focus, as Compose's `androidx.compose.ui.focus`: focus target nodes and their state, focus properties,
//! requesters, restorers, focus events, the host's focus owner and the focus manager the locals give, and focus
//! search — one-dimensional (Tab: placement order) and two-dimensional (arrows: Android's beam search).
//!
//! The focus is one target at a time, anywhere in the host: in its window, in a popup or a dialog. Each window's
//! root has a root target that never takes it; search (Tab, arrows, `move_focus`) starts at the root of the window
//! on top that takes the focus (a focusable popup's, a dialog's), else the host's, and keys go along the focused
//! path when the focus is in that window (else to its root's key node). A node's focus methods are on its
//! [`NodeCx`] (`request_focus`, `focus_state`, `capture_focus`, …: Compose's FocusRequesterModifierNode and
//! FocusTargetModifierNode functions).

mod beyond_bounds;
mod event;
mod owner;
mod properties;
mod relocation;
mod requester;
mod restorer;
mod search;
mod target;

pub use beyond_bounds::{BeyondBoundsDirection, BeyondBoundsLayout, BeyondBoundsScope};
pub use event::FocusEventNode;
pub use owner::FocusOwner;
pub use properties::{FocusEnterExitBlock, FocusEnterExitScope, FocusProperties, FocusPropertiesNode};
pub use relocation::{
    BoundsProvider, BringIntoViewFuture, BringIntoViewModifierNode, bring_into_view, bring_layout_into_view,
};
pub use requester::{FocusRequester, FocusRequesterNode};
pub use restorer::FocusRestorerNode;
pub use search::is_better_candidate;
pub use target::{FocusChangeCallback, FocusTargetNode};

pub(crate) use target::RootFocusTargetElement;

use crate::input::{Key, KeyEvent, KeyEventType, KeyInputModifierNode};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeCell, NodeCx, NodeKind, NodeRc};
use crate::node::{LayoutNode, Owner};
use crate::node_capabilities;
use crate::platform::{InputMode, LocalInputModeManager};
use kompoz_graphics::Rect;
use std::rc::{Rc, Weak};

/// Which way focus moves (Compose's FocusDirection).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FocusDirection {
    Next,
    Previous,
    Left,
    Right,
    Up,
    Down,
    /// Into the focused target's children.
    Enter,
    /// Out of the focused target, to the target around it.
    Exit,
}

/// A target's focus state (Compose's FocusState / FocusStateImpl).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FocusState {
    /// It has the focus.
    Active,
    /// A target below it has the focus.
    ActiveParent,
    /// It has the focus, which can't be taken away unless forced (a field with an invalid input).
    Captured,
    /// Neither it nor anything below it has the focus.
    Inactive,
}

impl FocusState {
    /// Whether it has the focus itself (Active or Captured).
    pub fn is_focused(self) -> bool {
        matches!(self, FocusState::Active | FocusState::Captured)
    }
    /// Whether it or a target below it has the focus.
    pub fn has_focus(self) -> bool {
        !matches!(self, FocusState::Inactive)
    }
    pub fn is_captured(self) -> bool {
        self == FocusState::Captured
    }
}

/// How a target can take the focus (Compose's Focusability).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Focusability {
    /// Always.
    Always,
    /// As the system says: not in touch mode (a button on a phone), yes with a keyboard.
    SystemDefined,
    /// Never (a group: its children can).
    Never,
}

impl Focusability {
    /// Whether a target `cx` is can take the focus now (Compose's `canFocus`).
    pub fn can_focus(self, cx: &NodeCx) -> bool {
        match self {
            Focusability::Always => true,
            Focusability::SystemDefined => cx.current_value_of(&LocalInputModeManager).input_mode() != InputMode::Touch,
            Focusability::Never => false,
        }
    }
}

/// A node that can take the focus (Compose's FocusTargetModifierNode): a [`FocusTargetNode`].
pub trait FocusTargetModifierNode {
    fn focus_target_node(&mut self) -> &mut FocusTargetNode;
}

/// A node that sets the focus properties of the targets below it, up to the next target (Compose's
/// FocusPropertiesModifierNode). A change of what it sets is told with
/// [`NodeCx::invalidate_focus_properties`].
pub trait FocusPropertiesModifierNode {
    fn apply_focus_properties(&mut self, properties: &mut FocusProperties);
}

/// A node told the focus state of the targets below it, up to the next one (Compose's FocusEventModifierNode).
pub trait FocusEventModifierNode {
    fn on_focus_event(&mut self, state: FocusState);
}

// ---------------------------------------------------------------------------------------------------------------
// Walking the tree for focus

/// The nodes of `kind` above `cell`, nearest first (Compose's `ancestors`).
pub(crate) fn ancestors(cell: &NodeRc, kind: NodeKind) -> Vec<NodeRc> {
    let mut out = Vec::new();
    NodeCx::new(cell).visit_ancestors(kind, false, &mut |c| {
        out.push(c.cell.clone());
        true
    });
    out
}

/// The nearest node of `kind` above `cell` (Compose's `nearestAncestor`).
pub(crate) fn nearest_ancestor(cell: &NodeRc, kind: NodeKind) -> Option<NodeRc> {
    let mut found = None;
    NodeCx::new(cell).visit_ancestors(kind, false, &mut |c| {
        found = Some(c.cell.clone());
        false
    });
    found
}

/// The nearest nodes of `kind` below `cell` (Compose's `visitChildren`).
pub(crate) fn children(cell: &NodeRc, kind: NodeKind) -> Vec<NodeRc> {
    let mut out = Vec::new();
    NodeCx::new(cell).visit_children(kind, &mut |c| {
        out.push(c.cell.clone());
        true
    });
    out
}

/// `cell` if it is of `kind` itself, else the nearest nodes of it below (Compose's `visitSelfAndChildren`).
fn self_and_children(cell: &NodeRc, kind: NodeKind) -> Vec<NodeRc> {
    if kind.is_in(cell.meta.self_kind_set.get()) { vec![cell.clone()] } else { children(cell, kind) }
}

/// The focus owner of the host `cell` is in.
pub(crate) fn focus_owner_of(cell: &NodeRc) -> Option<Rc<FocusOwner>> {
    cell.layout_node()?.owner()?.focus()
}

/// A focus node of a chain changed (Compose's autoInvalidate of the focus kinds): a property node that says
/// whether its targets can take the focus has them worked out again; an event node is told the state below it.
pub(crate) fn on_focus_node_changed(node: &NodeRc, kinds: u32, _removed: bool) {
    let Some(owner) = node.layout_node().and_then(|n| n.owner()).and_then(|o| o.focus()) else { return };
    if NodeKind::FocusProperties.is_in(kinds) && specifies_can_focus(node) {
        for t in children(node, NodeKind::FocusTarget) {
            owner.schedule_target_invalidation(&t);
        }
    }
    if NodeKind::FocusEvent.is_in(kinds) {
        owner.schedule_event_invalidation(node);
    }
    owner.prune();
}

/// Whether a property node sets `can_focus` (Compose's `specifiesCanFocusProperty`).
fn specifies_can_focus(node: &NodeRc) -> bool {
    let mut n = node.node.borrow_mut();
    let Some(p) = n.as_focus_properties() else { return false };
    let mut a = FocusProperties { can_focus: true, ..FocusProperties::default() };
    p.apply_focus_properties(&mut a);
    let mut b = FocusProperties { can_focus: false, ..FocusProperties::default() };
    p.apply_focus_properties(&mut b);
    a.can_focus == b.can_focus
}

impl Owner {
    /// The layout node of the target with the focus.
    pub(crate) fn focused_layout_node(&self) -> Option<Rc<LayoutNode>> {
        self.focus()?.focused_layout_node()
    }
}

/// Moves the focus to the first target of `layout` (its own chain's, else below it) that takes it (what
/// accessibility's Focus does to a node without a `RequestFocus` action).
pub(crate) fn request_focus_in_layout(layout: &Rc<LayoutNode>) -> bool {
    let mut targets = Vec::new();
    layout.head_to_tail(NodeKind::FocusTarget, &mut |c| targets.push(c.clone()));
    for t in targets {
        if target::fetch_focus_properties(&t).can_focus {
            return target::request_focus(&t, FocusDirection::Enter);
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------------------------
// A node's focus methods (Compose's FocusRequesterModifierNode, FocusTargetModifierNode and FocusEventModifierNode
// functions)

impl NodeCx {
    /// Moves the focus to this node if it is a target, else to the first target below it that takes it
    /// (Compose's `FocusRequesterModifierNode.requestFocus`); whether it moved.
    pub fn request_focus(&self) -> bool {
        self.request_focus_with(FocusDirection::Enter)
    }

    /// The same, moving `direction` (Compose's `FocusTargetModifierNode.requestFocus(focusDirection)`).
    pub fn request_focus_with(&self, direction: FocusDirection) -> bool {
        self_and_children(&self.cell, NodeKind::FocusTarget)
            .first()
            .is_some_and(|t| target::request_focus(t, direction))
    }

    /// Captures the focus here (or below), if it has it.
    pub fn capture_focus(&self) -> bool {
        self_and_children(&self.cell, NodeKind::FocusTarget).iter().any(target::capture_focus)
    }

    /// Frees a captured focus here (or below).
    pub fn free_focus(&self) -> bool {
        self_and_children(&self.cell, NodeKind::FocusTarget).iter().any(target::free_focus)
    }

    /// Notes which child of the target here (or below) has the focus (Compose's `saveFocusedChild`).
    pub fn save_focused_child(&self) -> bool {
        self_and_children(&self.cell, NodeKind::FocusTarget).iter().any(restorer::save_focused_child)
    }

    /// Moves the focus back to the child noted (Compose's `restoreFocusedChild`).
    pub fn restore_focused_child(&self) -> bool {
        self_and_children(&self.cell, NodeKind::FocusTarget).iter().any(restorer::restore_focused_child)
    }

    /// The focus state of this target, or (another node) of the first target below it that has the focus in it
    /// (Compose's `focusState` / `FocusEventModifierNode.getFocusState`).
    pub fn focus_state(&self) -> FocusState {
        for t in self_and_children(&self.cell, NodeKind::FocusTarget) {
            let s = target::focus_state(&t);
            if s != FocusState::Inactive {
                return s;
            }
        }
        FocusState::Inactive
    }

    /// How this target can take the focus (it is a [`FocusTargetNode`]).
    pub fn focusability(&self) -> Option<Focusability> {
        target::with_target(&self.cell, |t| t.focusability)
    }

    /// Sets how this target can take the focus; losing it clears the focus here (Compose's `focusability`
    /// setter).
    pub fn set_focusability(&self, focusability: Focusability) {
        target::set_focusability(&self.cell, focusability);
    }

    /// The focused rect in this node's coordinates, while the focus is here or below (Compose's
    /// `getFocusedRect`).
    pub fn focused_rect(&self) -> Option<Rect> {
        if !self.is_attached() {
            return None;
        }
        let state = self.focus_state();
        if !state.has_focus() {
            return None;
        }
        if state.is_focused() && target::is_target(&self.cell) {
            return Some(target::fetch_focus_rect(&self.cell, None));
        }
        let owner = focus_owner_of(&self.cell)?;
        let active = owner.active_focus_target()?;
        Some(target::fetch_focus_rect(&active, self.coordinates().as_ref()))
    }

    /// The focus properties of the targets below worked out again (Compose's `invalidateFocusProperties`).
    pub fn invalidate_focus_properties(&self) {
        let Some(owner) = focus_owner_of(&self.cell) else { return };
        for t in children(&self.cell, NodeKind::FocusTarget) {
            owner.schedule_target_invalidation(&t);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The focus manager

/// Moving and clearing the focus (Compose's FocusManager), as `LocalFocusManager` gives it.
#[derive(Clone, Default)]
pub struct FocusManager {
    pub(crate) owner: Option<Weak<FocusOwner>>,
}

impl PartialEq for FocusManager {
    fn eq(&self, other: &Self) -> bool {
        match (&self.owner, &other.owner) {
            (Some(a), Some(b)) => a.ptr_eq(b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl FocusManager {
    /// Takes the focus from where it is (Compose's `clearFocus(force)`).
    ///
    /// Kotlin: `clearFocus(force: Boolean = false)`
    pub fn clear_focus(&self, force: bool) {
        if let Some(o) = self.owner.as_ref().and_then(Weak::upgrade) {
            o.clear_focus(force);
        }
    }
    /// Moves the focus (Compose's `moveFocus`); whether it moved.
    pub fn move_focus(&self, direction: FocusDirection) -> bool {
        self.owner.as_ref().and_then(Weak::upgrade).is_some_and(|o| o.move_focus(direction))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A window's root

/// The modifier of a window's root (the host's, each popup's): its key node — what moves the focus by keys none
/// took — before its root focus target (Compose's AndroidComposeView root modifier: `keyInputModifier` then
/// `focusOwner.modifier`).
pub(crate) fn window_root_modifier() -> Modifier {
    Modifier.then_element(RootKeyInputElement).then_element(RootFocusTargetElement)
}

/// A popup's root modifiers with its key listeners (Compose for desktop's popup layer's `setKeyEventListener`):
/// `on_preview_key_event` sees the keys going down first, `on_key_event` those going up before the root's focus
/// moves.
pub(crate) fn window_root_modifier_with_keys(
    on_preview_key_event: Option<crate::input::KeyHandler>,
    on_key_event: Option<crate::input::KeyHandler>,
) -> Modifier {
    let mut m = Modifier;
    if let Some(preview) = on_preview_key_event {
        m = m.on_preview_key_event(move |e| preview(e));
    }
    m = m.then_element(RootKeyInputElement);
    if let Some(key) = on_key_event {
        m = m.on_key_event(move |e| key(e));
    }
    m.then_element(RootFocusTargetElement)
}

#[derive(PartialEq)]
struct RootKeyInputElement;

impl ModifierNodeElement for RootKeyInputElement {
    type Node = RootKeyInputNode;
    fn create(&self) -> RootKeyInputNode {
        RootKeyInputNode { cell: None }
    }
    fn update(&self, _node: &mut RootKeyInputNode) {}
    fn name(&self) -> &'static str {
        "keyInputModifier"
    }
}

/// A window root's key node (Compose's AndroidComposeView `keyInputModifier`): a key none took that is a focus
/// direction moves the focus that way; a 1D move with nothing more that way wraps around.
pub struct RootKeyInputNode {
    cell: Option<Weak<NodeCell<dyn ModifierNode>>>,
}

impl ModifierNode for RootKeyInputNode {
    node_capabilities!(key_input);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.cell = Some(Rc::downgrade(&cx.cell));
    }
    fn on_detach(&mut self, _cx: &NodeCx) {
        self.cell = None;
    }
}

impl KeyInputModifierNode for RootKeyInputNode {
    fn on_pre_key_event(&mut self, _event: &KeyEvent) -> bool {
        false
    }

    fn on_key_event(&mut self, event: &KeyEvent) -> bool {
        let Some(direction) = focus_direction_of(event) else { return false };
        if event.r#type != KeyEventType::KeyDown {
            return false;
        }
        let Some(owner) = self.cell.as_ref().and_then(Weak::upgrade).and_then(|c| focus_owner_of(&c)) else {
            return false;
        };
        let Some(moved) = owner.focus_search(direction, None, &mut |t| target::request_focus(t, direction)) else {
            return true;
        };
        if moved {
            return true;
        }
        if !direction.is_1d() {
            return false;
        }
        if !owner.clear_focus_with(false, true, false, direction) {
            return true;
        }
        owner.focus_search(direction, None, &mut |t| target::request_focus(t, direction)).unwrap_or(true)
    }
}

/// The focus direction a key event is (Compose's `KeyEvent.toFocusDirection`): Tab (Shift: back), the arrows
/// and page keys, Enter (into), Escape and Back (out).
pub fn focus_direction_of(event: &KeyEvent) -> Option<FocusDirection> {
    Some(match event.key {
        Key::NavigatePrevious => FocusDirection::Previous,
        Key::NavigateNext => FocusDirection::Next,
        Key::Tab if event.is_shift_pressed() => FocusDirection::Previous,
        Key::Tab => FocusDirection::Next,
        Key::DirectionRight => FocusDirection::Right,
        Key::DirectionLeft => FocusDirection::Left,
        Key::DirectionUp | Key::PageUp => FocusDirection::Up,
        Key::DirectionDown | Key::PageDown => FocusDirection::Down,
        Key::DirectionCenter | Key::Enter | Key::NumPadEnter => FocusDirection::Enter,
        Key::Back | Key::Escape => FocusDirection::Exit,
        _ => return None,
    })
}

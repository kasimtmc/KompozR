//! Focus, as Compose's `androidx.compose.ui.focus`: focus target nodes, focus properties, focus events, the focus
//! owner and the focus manager.
//!
//! What is here is the core's part: the capability traits nodes declare, the host's focus state, the focus
//! manager the locals give, and the hooks the chain calls. Focus search (1D and 2D), requesters, restorers and key
//! dispatch along the focused path are the focus package of this stage, which gives these their insides.

use crate::modifier::NodeRc;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// A node that can take the focus (Compose's FocusTargetNode). The focus package gives it its state and methods.
pub trait FocusTargetModifierNode {}

/// A node that sets the focus properties of the targets below it (Compose's FocusPropertiesModifierNode). The focus
/// package gives it `apply_focus_properties`.
pub trait FocusPropertiesModifierNode {}

/// A node told when the focus state below it changes (Compose's FocusEventModifierNode). The focus package gives it
/// `on_focus_event`.
pub trait FocusEventModifierNode {}

/// A node that brings a rect of its into view when asked (Compose's BringIntoViewModifierNode / responder). The
/// focus package gives it its methods.
pub trait BringIntoViewModifierNode {}

/// Which way focus moves (Compose's FocusDirection).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FocusDirection {
    Next,
    Previous,
    Left,
    Right,
    Up,
    Down,
    Enter,
    Exit,
}

/// The host's focus state (Compose's FocusOwner): which node has the focus, the nodes whose focus properties or
/// events changed since the last frame. The focus package keeps its search state in it.
#[derive(Default)]
pub struct FocusOwner {
    /// The focused target node.
    pub(crate) focused: Option<Weak<crate::modifier::NodeCell<dyn crate::modifier::ModifierNode>>>,
    /// Focus nodes whose properties or events changed: looked at after the frame's layout.
    pub(crate) invalidated: Vec<Weak<crate::modifier::NodeCell<dyn crate::modifier::ModifierNode>>>,
}

impl FocusOwner {
    /// Whether a node has the focus.
    pub fn has_focus(&self) -> bool {
        self.focused.as_ref().is_some_and(|w| w.strong_count() > 0)
    }

    /// The focus taken from where it is (Compose's `clearFocus`); whether it moved.
    pub fn clear_focus(&mut self, _force: bool) -> bool {
        self.focused.take().is_some()
    }

    /// The focus moved `direction` (Compose's `moveFocus`); the focus package searches. Whether it moved.
    pub fn move_focus(&mut self, _direction: FocusDirection) -> bool {
        false
    }

    /// Forgets nodes that are gone.
    pub(crate) fn prune(&mut self) {
        self.invalidated.retain(|w| w.strong_count() > 0);
        if self.focused.as_ref().is_some_and(|w| w.strong_count() == 0) {
            self.focused = None;
        }
    }
}

/// A focus node of a chain changed (Compose's autoInvalidate of the focus kinds): its focus state worked out again
/// after the frame's layout; a removed focused target gives the focus up.
pub(crate) fn on_focus_node_changed(node: &NodeRc, _kinds: u32, removed: bool) {
    let Some(owner) = node.layout_node().and_then(|n| n.owner()) else { return };
    let Some(focus) = owner.focus() else { return };
    let mut focus = focus.borrow_mut();
    let this = Rc::downgrade(node);
    if removed && focus.focused.as_ref().is_some_and(|f| f.ptr_eq(&this)) {
        focus.focused = None;
    }
    if !removed && !focus.invalidated.iter().any(|w| w.ptr_eq(&this)) {
        focus.invalidated.push(this);
    }
    focus.prune();
}

/// Moving and clearing the focus (Compose's FocusManager), as the locals give it.
#[derive(Clone, Default)]
pub struct FocusManager {
    pub(crate) owner: Option<Weak<RefCell<FocusOwner>>>,
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
    pub fn clear_focus(&self, force: bool) {
        if let Some(o) = self.owner.as_ref().and_then(Weak::upgrade) {
            o.borrow_mut().clear_focus(force);
        }
    }
    /// Moves the focus (Compose's `moveFocus`); whether it moved.
    pub fn move_focus(&self, direction: FocusDirection) -> bool {
        self.owner.as_ref().and_then(Weak::upgrade).is_some_and(|o| o.borrow_mut().move_focus(direction))
    }
}

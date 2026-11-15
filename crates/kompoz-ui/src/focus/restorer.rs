//! Focus restoration (Compose's FocusRestorer.kt): a target notes which child had the focus when the focus left
//! it (by the child's composite key), and `Modifier.focus_restorer` sends the focus back there when it comes in.

use super::requester::FocusRequester;
use super::target::{self, with_target};
use super::{FocusPropertiesModifierNode, children};
use crate::focus::properties::{FocusEnterExitScope, FocusProperties};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeCell, NodeCx, NodeKind, NodeRc};
use crate::node_capabilities;
use kompoz_runtime::named;
use std::rc::{Rc, Weak};

/// Notes, at `cell` (a target with the focus in it), the child target the focus is in (Compose's
/// `saveFocusedChild`).
pub(crate) fn save_focused_child(cell: &NodeRc) -> bool {
    if !target::focus_state(cell).has_focus() {
        return false;
    }
    for child in children(cell, NodeKind::FocusTarget) {
        if target::focus_state(&child).has_focus() {
            let Some(key) = child.layout_node().map(|n| n.composite_key()) else { return false };
            with_target(cell, |t| t.previously_focused_child_hash = Some(key));
            return true;
        }
    }
    false
}

/// Moves the focus back to the child target noted at `cell` (Compose's `restoreFocusedChild`).
pub(crate) fn restore_focused_child(cell: &NodeRc) -> bool {
    let Some(key) = with_target(cell, |t| t.previously_focused_child_hash).flatten() else { return false };
    for child in children(cell, NodeKind::FocusTarget) {
        if child.meta.attached.get() && child.layout_node().is_some_and(|n| n.composite_key() == key) {
            return target::request_focus(&child, super::FocusDirection::Enter);
        }
    }
    false
}

impl Modifier {
    /// [`focus_restorer`](Modifier::focus_restorer) with its fallback, for plain code.
    pub fn focus_restorer_with(self, fallback: FocusRequester) -> Modifier {
        self.then_element(FocusRestorerElement { fallback })
    }
}

#[named]
impl Modifier {
    /// Sends the focus, coming in, back to the child that had it when it left; the first time (or if that child is
    /// gone), to `fallback` (`FocusRequester::Default`: the usual search; `FocusRequester::Cancel`: nowhere)
    /// (Compose's `Modifier.focusRestorer(fallback)`).
    pub fn focus_restorer(self, #[default(FocusRequester::Default)] fallback: FocusRequester) -> Modifier {
        self.then_element(FocusRestorerElement { fallback })
    }
}

#[derive(PartialEq)]
struct FocusRestorerElement {
    fallback: FocusRequester,
}

impl ModifierNodeElement for FocusRestorerElement {
    type Node = FocusRestorerNode;
    fn create(&self) -> FocusRestorerNode {
        FocusRestorerNode { fallback: self.fallback.clone(), cell: None }
    }
    fn update(&self, node: &mut FocusRestorerNode) {
        node.fallback = self.fallback.clone();
    }
    fn name(&self) -> &'static str {
        "focusRestorer"
    }
}

/// `Modifier.focus_restorer`'s node (Compose's FocusRestorerNode): its enter restores, else goes to its fallback.
pub struct FocusRestorerNode {
    fallback: FocusRequester,
    cell: Option<Weak<NodeCell<dyn ModifierNode>>>,
}

impl ModifierNode for FocusRestorerNode {
    node_capabilities!(focus_properties);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.cell = Some(Rc::downgrade(&cx.cell));
    }
    fn on_detach(&mut self, _cx: &NodeCx) {
        self.cell = None;
    }
}

impl FocusPropertiesModifierNode for FocusRestorerNode {
    fn apply_focus_properties(&mut self, properties: &mut FocusProperties) {
        let cell = self.cell.clone();
        let fallback = self.fallback.clone();
        properties.on_enter = Some(Rc::new(move |scope: &mut FocusEnterExitScope| {
            let restored =
                cell.as_ref().and_then(Weak::upgrade).is_some_and(|c| NodeCx::new(&c).restore_focused_child());
            if !restored && fallback != FocusRequester::Default {
                if fallback == FocusRequester::Cancel {
                    scope.cancel_focus_change();
                } else {
                    fallback.request_focus();
                }
            }
        }));
        // (Compose's onExit saves the focused child only with focus restoration off; with it on, as here, each
        // focus change notes it.)
        properties.on_exit = None;
    }
}

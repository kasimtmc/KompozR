//! Focus events (Compose's `Modifier.onFocusEvent`, `Modifier.onFocusChanged` and FocusEventModifierNode): told the
//! focus state of the targets below, up to the next target above them.

use super::{FocusEventModifierNode, FocusState};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement};
use crate::node_capabilities;
use std::rc::Rc;

impl Modifier {
    /// `on_focus_event` told the focus state below each time it is worked out, changed or not (Compose's
    /// `Modifier.onFocusEvent`).
    pub fn on_focus_event(self, on_focus_event: impl Fn(FocusState) + 'static) -> Modifier {
        self.then_element(FocusEventElement { callback: Rc::new(on_focus_event), changes_only: false })
    }

    /// `on_focus_changed` told the focus state below when it changes (and its first one: Compose's
    /// `Modifier.onFocusChanged`).
    pub fn on_focus_changed(self, on_focus_changed: impl Fn(FocusState) + 'static) -> Modifier {
        self.then_element(FocusEventElement { callback: Rc::new(on_focus_changed), changes_only: true })
    }
}

struct FocusEventElement {
    callback: Rc<dyn Fn(FocusState)>,
    changes_only: bool,
}

impl PartialEq for FocusEventElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.callback, &other.callback) && self.changes_only == other.changes_only
    }
}

impl ModifierNodeElement for FocusEventElement {
    type Node = FocusEventNode;
    fn create(&self) -> FocusEventNode {
        FocusEventNode { callback: self.callback.clone(), changes_only: self.changes_only, last: None }
    }
    fn update(&self, node: &mut FocusEventNode) {
        node.callback = self.callback.clone();
        node.changes_only = self.changes_only;
    }
    fn name(&self) -> &'static str {
        if self.changes_only { "onFocusChanged" } else { "onFocusEvent" }
    }
}

/// The node of `on_focus_event` (Compose's FocusEventNode) and of `on_focus_changed` (FocusChangedNode, which
/// tells only changes). Its callback runs once the node is released.
pub struct FocusEventNode {
    callback: Rc<dyn Fn(FocusState)>,
    changes_only: bool,
    last: Option<FocusState>,
}

impl ModifierNode for FocusEventNode {
    node_capabilities!(focus_event);
}

impl FocusEventModifierNode for FocusEventNode {
    fn on_focus_event(&mut self, state: FocusState) {
        if self.changes_only && self.last == Some(state) {
            return;
        }
        self.last = Some(state);
        let callback = self.callback.clone();
        crate::input::when_node_released(move || callback(state));
    }
}

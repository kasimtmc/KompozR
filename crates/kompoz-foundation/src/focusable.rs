//! Focus groups (Compose's `Modifier.focusGroup` of Focusable.kt): a focus target that never takes the focus
//! itself, so its children are searched as one group (arrow keys stay in it first; from outside it is one target).

use kompoz_runtime::named;
use kompoz_ui::focus::{FocusTargetNode, Focusability};
use kompoz_ui::{Modifier, ModifierNodeElement};

/// The focus modifiers of foundation.
#[named(trait FocusableModifiers)]
impl Modifier {
    /// Makes the layout a focus group (Compose's `Modifier.focusGroup`): a target its children's focus goes
    /// through, never focused itself.
    fn focus_group(self) -> Modifier {
        self.then_element(FocusGroupElement)
    }
}

#[derive(PartialEq)]
struct FocusGroupElement;

impl ModifierNodeElement for FocusGroupElement {
    type Node = FocusTargetNode;
    fn create(&self) -> FocusTargetNode {
        FocusTargetNode::new(Focusability::Never, None)
    }
    fn update(&self, _node: &mut FocusTargetNode) {}
    fn name(&self) -> &'static str {
        "focusGroup"
    }
}

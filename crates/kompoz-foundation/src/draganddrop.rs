//! `Modifier.drag_and_drop_target`, as Compose's foundation DragAndDropTarget.kt: the layout takes drags (from
//! outside the app: files from a file manager, text from a browser) that `should_start_drag_and_drop` wants, told
//! to `target` (kompoz-ui's [`DragAndDropTargetNode`]).

use kompoz_runtime::named;
use kompoz_ui::draganddrop::{DragAndDropEvent, DragAndDropTarget, DragAndDropTargetNode};
use kompoz_ui::{Modifier, ModifierNodeElement};
use std::rc::Rc;

/// The drag and drop modifiers of foundation.
#[named(trait DragAndDropModifiers)]
impl Modifier {
    /// The layout takes the drags `should_start_drag_and_drop` wants (asked as each starts), told to `target` as
    /// they start, enter it, move over it, leave it, are dropped on it and end (Compose's
    /// `Modifier.dragAndDropTarget`).
    fn drag_and_drop_target(
        self,
        should_start_drag_and_drop: impl Fn(&DragAndDropEvent) -> bool + 'static,
        target: Rc<dyn DragAndDropTarget>,
    ) -> Modifier {
        self.then_element(DragAndDropTargetElement { should_start: Rc::new(should_start_drag_and_drop), target })
    }
}

struct DragAndDropTargetElement {
    should_start: Rc<dyn Fn(&DragAndDropEvent) -> bool>,
    target: Rc<dyn DragAndDropTarget>,
}

impl PartialEq for DragAndDropTargetElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.should_start, &other.should_start) && Rc::ptr_eq(&self.target, &other.target)
    }
}

impl ModifierNodeElement for DragAndDropTargetElement {
    type Node = DragAndDropTargetNode;
    fn create(&self) -> DragAndDropTargetNode {
        DragAndDropTargetNode::new(self.should_start.clone(), self.target.clone())
    }
    fn update(&self, node: &mut DragAndDropTargetNode) {
        node.update(self.should_start.clone(), self.target.clone());
    }
    fn name(&self) -> &'static str {
        "dragAndDropTarget"
    }
}

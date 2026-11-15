//! Gesture coordination, as Compose's GestureNode.kt: a node handling a high level gesture (a click, a drag, a
//! scroll) delegates to a [`GestureNode`] carrying its [`GestureConnection`], so the gestures above and below it can
//! ask how far along it is (a click inside a scroll container waits to show its press while the scroll is deciding).

use kompoz_ui::modifier::TraversableNode;
use kompoz_ui::{ModifierNode, NodeCx, node_capabilities};
use std::any::Any;
use std::rc::Rc;

/// How far a high level gesture is (Compose's GestureState): enabled and idle, waiting for what decides it (a touch
/// slop), or under way.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GestureState {
    Idle,
    Waiting,
    Recognized,
}

/// What a gesture handler tells the others (Compose's GestureConnection): its state, set as its recognition starts
/// (in the Initial or Main pass) and reset as it ends (in the Final pass).
pub trait GestureConnection {
    fn gesture_state(&self) -> GestureState;
}

/// What gesture nodes are found by (Compose's GestureNode.TraverseKey).
const GestureNodeTraverseKey: &str = "androidx.compose.foundation.GestureNode";

/// A node carrying a gesture handler's [`GestureConnection`] (Compose's GestureNode, made by `gestureNode(…)`): the
/// handler delegates to it while it is enabled, and takes it back when it is disabled.
pub struct GestureNode {
    connection: Rc<dyn GestureConnection>,
}

impl GestureNode {
    /// A node for `connection` (Compose's `gestureNode`).
    pub fn new(connection: Rc<dyn GestureConnection>) -> GestureNode {
        GestureNode { connection }
    }
}

impl ModifierNode for GestureNode {
    node_capabilities!(traversable);
}

impl TraversableNode for GestureNode {
    fn traverse_key(&self) -> &dyn Any {
        &GestureNodeTraverseKey
    }
}

/// Calls `block` with the gesture connections above `cx`'s node, nearest first, until it says false (Compose's
/// `traverseAncestorGestures`).
pub fn traverse_ancestor_gestures(cx: &NodeCx, block: &mut dyn FnMut(&dyn GestureConnection) -> bool) {
    cx.traverse_ancestors(&GestureNodeTraverseKey, &mut |node| {
        let connection = node.with(|g: &mut GestureNode| g.connection.clone());
        match connection {
            Some(c) => block(&*c),
            None => true,
        }
    });
}

/// Calls `block` with the nearest gesture connections below `cx`'s node, until it says false (Compose's
/// `traverseChildrenGestures`).
pub fn traverse_children_gestures(cx: &NodeCx, block: &mut dyn FnMut(&dyn GestureConnection) -> bool) {
    cx.traverse_children(&GestureNodeTraverseKey, &mut |node| {
        let connection = node.with(|g: &mut GestureNode| g.connection.clone());
        match connection {
            Some(c) => block(&*c),
            None => true,
        }
    });
}

/// Whether a gesture above `cx`'s node is waiting to be decided (Compose's `hasWaitingParent`).
pub fn has_waiting_parent(cx: &NodeCx) -> bool {
    let mut waiting = false;
    traverse_ancestor_gestures(cx, &mut |connection| {
        waiting = connection.gesture_state() == GestureState::Waiting;
        !waiting
    });
    waiting
}

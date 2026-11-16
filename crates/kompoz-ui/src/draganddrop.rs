//! Drag and drop targets (Compose's `androidx.compose.ui.draganddrop`: DragAndDropTarget, DragAndDropEvent, the
//! DragAndDropNode targets are made of, and the manager's part the host plays): a drag from outside the app (files
//! from a file manager, text from a browser), as the platform reports it, reaches the targets in the tree. When it
//! comes in, every target is asked whether it wants it (`should_start_drag_and_drop`); those that do hear it start,
//! enter and leave them as it moves (the innermost under it), the drop (the one under it), and its end.
//!
//! The host's entry points ([`KompozHost::external_drag_entered`](crate::KompozHost::external_drag_entered),
//! `external_drag_moved`, `external_drop`, `external_drag_left`) drive the tree's root node, as Compose's
//! DragAndDropManager drives its root DragAndDropNode. Positions are px in the window.

use crate::modifier::{ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, TraversableNode, TraversalDecision};
use crate::node_capabilities;
use kompoz_graphics::Offset;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

/// What a drag carries, as told at its drop (C++'s DragAndDropTransferData; Compose's is the platform's own): its
/// MIME types ("text/uri-list" for files, "text/plain"), its text (the files' `file:` URIs, a line each, for a
/// uri-list), and the files' paths.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DragAndDropTransferData {
    pub mime_types: Vec<String>,
    pub text: String,
    pub files: Vec<String>,
}

/// A drag's moment (Compose's DragAndDropEvent): what it may carry (its MIME types, all that is known before the
/// drop), where it is (px in the window), and at the drop what it carries.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct DragAndDropEvent {
    pub mime_types: Vec<String>,
    pub position_in_root: Offset,
    pub transfer_data: Option<DragAndDropTransferData>,
}

impl DragAndDropEvent {
    /// Its MIME types (Compose's `mimeTypes()`).
    pub fn mime_types(&self) -> &[String] {
        &self.mime_types
    }
}

/// What takes drags (Compose's DragAndDropTarget): told as one it wanted starts, enters it, moves over it, leaves
/// it, changes, is dropped on it (whether it took what it carries) and ends.
pub trait DragAndDropTarget {
    /// Dropped on it; whether it took what the drag carries.
    fn on_drop(&self, event: &DragAndDropEvent) -> bool;
    fn on_started(&self, event: &DragAndDropEvent) {
        let _ = event;
    }
    fn on_entered(&self, event: &DragAndDropEvent) {
        let _ = event;
    }
    fn on_moved(&self, event: &DragAndDropEvent) {
        let _ = event;
    }
    fn on_exited(&self, event: &DragAndDropEvent) {
        let _ = event;
    }
    fn on_changed(&self, event: &DragAndDropEvent) {
        let _ = event;
    }
    fn on_ended(&self, event: &DragAndDropEvent) {
        let _ = event;
    }
}

/// Whether a target wants a drag that starts (Compose's `shouldStartDragAndDrop`).
pub type ShouldStartDragAndDrop = Rc<dyn Fn(&DragAndDropEvent) -> bool>;

/// The key drag and drop nodes find each other by.
struct DragAndDropKey;
static KEY: DragAndDropKey = DragAndDropKey;

/// A drag and drop target's node, and the tree's root one (Compose's DragAndDropNode, its target part): a target
/// wanting the drag under way, and its child node the drag is over.
pub struct DragAndDropTargetNode {
    should_start: Option<ShouldStartDragAndDrop>,
    target: Option<Rc<dyn DragAndDropTarget>>,
    /// Its target while it takes part in the drag under way (Compose's `thisDragAndDropTarget`).
    this_target: Option<Rc<dyn DragAndDropTarget>>,
    /// The innermost child node the drag is over (Compose's `lastChildDragAndDropModifierNode`).
    last_child: Option<NodeHandle>,
    /// The root's: where the host finds it.
    root_slot: Option<Rc<RefCell<Option<NodeHandle>>>>,
}

impl DragAndDropTargetNode {
    /// A target's node (Compose's `DragAndDropTargetModifierNode`): `should_start` says whether it wants a drag,
    /// `target` takes it.
    pub fn new(should_start: ShouldStartDragAndDrop, target: Rc<dyn DragAndDropTarget>) -> DragAndDropTargetNode {
        DragAndDropTargetNode {
            should_start: Some(should_start),
            target: Some(target),
            this_target: None,
            last_child: None,
            root_slot: None,
        }
    }

    /// A new `should_start` and `target` (an element's update).
    pub fn update(&mut self, should_start: ShouldStartDragAndDrop, target: Rc<dyn DragAndDropTarget>) {
        self.should_start = Some(should_start);
        self.target = Some(target);
    }
}

impl ModifierNode for DragAndDropTargetNode {
    node_capabilities!(traversable);

    fn on_attach(&mut self, cx: &NodeCx) {
        if let Some(slot) = &self.root_slot {
            *slot.borrow_mut() = Some(cx.handle());
        }
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.this_target = None;
        self.last_child = None;
        if let Some(slot) = &self.root_slot {
            slot.borrow_mut().take();
        }
    }
}

impl TraversableNode for DragAndDropTargetNode {
    fn traverse_key(&self) -> &dyn Any {
        &KEY
    }
}

/// The root's element: the host's.
pub(crate) struct RootDragAndDropElement(pub(crate) Rc<RefCell<Option<NodeHandle>>>);

impl PartialEq for RootDragAndDropElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl ModifierNodeElement for RootDragAndDropElement {
    type Node = DragAndDropTargetNode;
    fn create(&self) -> DragAndDropTargetNode {
        DragAndDropTargetNode {
            should_start: None,
            target: None,
            this_target: None,
            last_child: None,
            root_slot: Some(self.0.clone()),
        }
    }
    fn update(&self, node: &mut DragAndDropTargetNode) {
        node.root_slot = Some(self.0.clone());
    }
    fn name(&self) -> &'static str {
        "rootDragAndDrop"
    }
}

/// The node's (this target, last child), copied out: no borrow is held while targets run.
fn parts(cx: &NodeCx) -> (Option<Rc<dyn DragAndDropTarget>>, Option<NodeHandle>) {
    cx.with(|n: &mut DragAndDropTargetNode| (n.this_target.clone(), n.last_child.clone())).unwrap_or((None, None))
}

fn set_last_child(cx: &NodeCx, child: Option<NodeHandle>) {
    cx.with(|n: &mut DragAndDropTargetNode| n.last_child = child);
}

/// Whether the node's layout is under `position` (px in the window).
fn contains(cx: &NodeCx, position: Offset) -> bool {
    cx.is_attached() && cx.coordinates().is_some_and(|c| c.bounds_in_root().contains(position))
}

fn same(a: &NodeHandle, b: &NodeHandle) -> bool {
    std::rc::Weak::ptr_eq(&a.cell, &b.cell)
}

/// A drag under way: the nodes that wanted it (Compose's manager's `interestedTargets`).
#[derive(Default)]
pub(crate) struct DragAndDropSession {
    interested: Vec<NodeHandle>,
}

impl DragAndDropSession {
    fn is_interested(&self, cx: &NodeCx) -> bool {
        self.interested.iter().any(|h| h.cx().is_some_and(|c| c.is_same_node(cx)))
    }
}

/// Every node at and below `root` asked whether its target wants the drag (Compose's `acceptDragAndDropTransfer`);
/// whether any did. Those that did are the session's.
pub(crate) fn accept(root: &NodeCx, event: &DragAndDropEvent, session: &mut DragAndDropSession) -> bool {
    let mut nodes = vec![root.clone()];
    root.traverse_descendants(&KEY, &mut |cx| {
        if !cx.is_attached() {
            return TraversalDecision::SkipSubtreeAndContinueTraversal;
        }
        nodes.push(cx.clone());
        TraversalDecision::ContinueTraversal
    });
    let mut accepted = false;
    for cx in nodes {
        let (should_start, target) =
            cx.with(|n: &mut DragAndDropTargetNode| (n.should_start.clone(), n.target.clone())).unwrap_or((None, None));
        let wanted = match (should_start, target) {
            (Some(should_start), Some(target)) => should_start(event).then_some(target),
            _ => None,
        };
        let wants = wanted.is_some();
        cx.with(|n: &mut DragAndDropTargetNode| n.this_target = wanted);
        if wants {
            session.interested.push(cx.handle());
        }
        accepted |= wants;
    }
    accepted
}

/// Each target that wanted the drag told it started (Compose's manager's `onStarted` to its interested targets).
pub(crate) fn started(session: &DragAndDropSession, event: &DragAndDropEvent) {
    for cx in session.interested.iter().filter_map(NodeHandle::cx) {
        let (this, _) = parts(&cx);
        if let Some(t) = this {
            t.on_started(event);
        }
    }
}

fn node_entered(cx: &NodeCx, event: &DragAndDropEvent) {
    let (this, child) = parts(cx);
    match (this, child) {
        (Some(t), _) => t.on_entered(event),
        (None, Some(c)) => {
            if let Some(c) = c.cx() {
                node_entered(&c, event);
            }
        }
        _ => {}
    }
}

/// A node entered: entered, then moved over (Compose's `dispatchEntered`).
fn node_dispatch_entered(cx: &NodeCx, event: &DragAndDropEvent, session: &DragAndDropSession) {
    node_entered(cx, event);
    moved(cx, event, session);
}

/// The drag moved over `cx`'s node (Compose's DragAndDropNode `onMoved`): into a child that wanted it, out of one
/// back to the node, from one child to another, or on within the same.
pub(crate) fn moved(cx: &NodeCx, event: &DragAndDropEvent, session: &DragAndDropSession) {
    let (this, current) = parts(cx);
    let position = event.position_in_root;
    let new_child = match &current {
        Some(c) if c.cx().is_some_and(|c| contains(&c, position)) => Some(c.clone()),
        _ => {
            let mut found = None;
            cx.traverse_descendants(&KEY, &mut |child| {
                if session.is_interested(child) && contains(child, position) {
                    found = Some(child.handle());
                    TraversalDecision::CancelTraversal
                } else {
                    TraversalDecision::ContinueTraversal
                }
            });
            found
        }
    };
    match (&new_child, &current) {
        (Some(new), None) => {
            if let Some(new) = new.cx() {
                node_dispatch_entered(&new, event, session);
            }
            if let Some(t) = &this {
                t.on_exited(event);
            }
        }
        (None, Some(old)) => {
            if let Some(t) = &this {
                t.on_entered(event);
                t.on_moved(event);
            }
            if let Some(old) = old.cx() {
                exited(&old, event);
            }
        }
        (Some(new), Some(old)) if !same(new, old) => {
            if let Some(new) = new.cx() {
                node_dispatch_entered(&new, event, session);
            }
            if let Some(old) = old.cx() {
                exited(&old, event);
            }
        }
        (Some(new), Some(_)) => {
            if let Some(new) = new.cx() {
                moved(&new, event, session);
            }
        }
        (None, None) => {
            if let Some(t) = &this {
                t.on_moved(event);
            }
        }
    }
    set_last_child(cx, new_child);
}

/// The drag left `cx`'s node (Compose's `onExited`).
pub(crate) fn exited(cx: &NodeCx, event: &DragAndDropEvent) {
    let (this, child) = parts(cx);
    if let Some(t) = this {
        t.on_exited(event);
    }
    if let Some(c) = child.and_then(|c| c.cx()) {
        exited(&c, event);
    }
    set_last_child(cx, None);
}

/// Dropped on `cx`'s node: the child it is over takes it, else the node's target (Compose's `onDrop`); whether it
/// was taken.
pub(crate) fn dropped(cx: &NodeCx, event: &DragAndDropEvent) -> bool {
    let (this, child) = parts(cx);
    match (child.and_then(|c| c.cx()), this) {
        (Some(c), _) => dropped(&c, event),
        (None, Some(t)) => t.on_drop(event),
        (None, None) => false,
    }
}

/// The drag ended: every node at and below `root` that took part told, and forgets it (Compose's `onEnded`).
pub(crate) fn ended(root: &NodeCx, event: &DragAndDropEvent) {
    let mut nodes = vec![root.clone()];
    root.traverse_descendants(&KEY, &mut |cx| {
        if !cx.is_attached() {
            return TraversalDecision::SkipSubtreeAndContinueTraversal;
        }
        nodes.push(cx.clone());
        TraversalDecision::ContinueTraversal
    });
    for cx in nodes {
        let this = cx
            .with(|n: &mut DragAndDropTargetNode| {
                n.last_child = None;
                n.this_target.take()
            })
            .flatten();
        if let Some(t) = this {
            t.on_ended(event);
        }
    }
}

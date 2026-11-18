//! What receivers and the text fields under them share (Compose's `foundation.content.internal`): a receiver's
//! configuration (its listener, the receivers' around it after it), how a node finds the nearest one
//! ([`get_receive_content_configuration`]: Compose's ModifierLocal `current`), and a receiver's drop node.

use super::{
    ModifierLocalReceiveContent, ReceiveContentListener, ReceiveContentNodeState, TransferableContent,
    TransferableContentSource,
};
use kompoz_ui::NodeCx;
use kompoz_ui::draganddrop::{DragAndDropEvent, DragAndDropNode, DragAndDropTarget, DragAndDropTargetModifierNode};
use std::cell::Cell;
use std::rc::Rc;

/// What content given to a receiver goes through (Compose's ReceiveContentConfiguration): its listener.
pub trait ReceiveContentConfiguration {
    fn receive_content_listener(&self) -> Rc<dyn ReceiveContentListener>;

    /// Content an input method committed given to the listener: whether it took any (Compose's `onCommitContent`:
    /// it gave back something else than it was given).
    fn on_commit_content(&self, transferable_content: TransferableContent) -> bool {
        let remaining = self.receive_content_listener().on_receive(transferable_content.clone());
        remaining.as_ref() != Some(&transferable_content)
    }
}

/// A configuration of a listener as it is (Compose's `ReceiveContentConfiguration(listener)`).
pub struct ReceiveContentConfigurationImpl(pub Rc<dyn ReceiveContentListener>);

impl ReceiveContentConfiguration for ReceiveContentConfigurationImpl {
    fn receive_content_listener(&self) -> Rc<dyn ReceiveContentListener> {
        self.0.clone()
    }
}

/// The nearest receiver's configuration around `cx`'s node, if it is attached (Compose's
/// `getReceiveContentConfiguration`: ModifierLocalReceiveContent's current value).
pub fn get_receive_content_configuration(cx: &NodeCx) -> Option<Rc<dyn ReceiveContentConfiguration>> {
    if !cx.is_attached() {
        return None;
    }
    cx.modifier_local_current(&ModifierLocalReceiveContent)
}

/// What a receiver provides (Compose's DynamicReceiveContentConfiguration): its listener now, then the receiver's
/// around it — content its listener gives back goes on to it, and its drag's enters and exits are told to it too
/// (counted: entering a field or receiver inside it isn't leaving it).
pub(crate) struct DynamicReceiveContentConfiguration {
    listener: Rc<dyn ReceiveContentListener>,
}

impl DynamicReceiveContentConfiguration {
    pub(crate) fn new(node: Rc<ReceiveContentNodeState>) -> DynamicReceiveContentConfiguration {
        DynamicReceiveContentConfiguration {
            listener: Rc::new(DynamicListener { node, node_enter_count: Cell::new(0) }),
        }
    }
}

impl ReceiveContentConfiguration for DynamicReceiveContentConfiguration {
    fn receive_content_listener(&self) -> Rc<dyn ReceiveContentListener> {
        self.listener.clone()
    }
}

/// The listener a receiver provides (Compose's object in DynamicReceiveContentConfiguration).
struct DynamicListener {
    node: Rc<ReceiveContentNodeState>,
    node_enter_count: Cell<i32>,
}

impl DynamicListener {
    /// The receiver's listener now (no borrow held while it runs).
    fn own(&self) -> Rc<dyn ReceiveContentListener> {
        self.node.receive_content_listener.borrow().clone()
    }

    /// The listener of the receiver around this one (Compose's `getParentReceiveContentListener`).
    fn parent(&self) -> Option<Rc<dyn ReceiveContentListener>> {
        let cx = self.node.handle.borrow().as_ref()?.cx()?;
        Some(get_receive_content_configuration(&cx)?.receive_content_listener())
    }
}

impl ReceiveContentListener for DynamicListener {
    fn on_drag_start(&self) {
        self.node_enter_count.set(0);
        self.own().on_drag_start();
    }

    fn on_drag_end(&self) {
        self.own().on_drag_end();
        self.node_enter_count.set(0);
    }

    fn on_drag_enter(&self) {
        self.node_enter_count.set(self.node_enter_count.get() + 1);
        if self.node_enter_count.get() == 1 {
            self.own().on_drag_enter();
        }
        if let Some(parent) = self.parent() {
            parent.on_drag_enter();
        }
    }

    fn on_drag_exit(&self) {
        let previous = self.node_enter_count.get();
        self.node_enter_count.set((previous - 1).max(0));
        if self.node_enter_count.get() == 0 && previous > 0 {
            self.own().on_drag_exit();
        }
        if let Some(parent) = self.parent() {
            parent.on_drag_exit();
        }
    }

    fn on_receive(&self, transferable_content: TransferableContent) -> Option<TransferableContent> {
        let remaining = self.own().on_receive(transferable_content)?;
        match self.parent() {
            Some(parent) => parent.on_receive(remaining),
            None => Some(remaining),
        }
    }
}

/// Asks the platform for permission to read what a drop carries (Compose's `dragAndDropRequestPermission`: Android
/// grants access to a drop's content URIs this way). The desktop platforms' drops are readable as they are: nothing
/// to ask (Compose desktop's actual).
pub(crate) fn drag_and_drop_request_permission(event: &DragAndDropEvent) {
    let _ = event;
}

/// A drop as content (Compose's `DragAndDropEvent.toTransferableContent`): what it carries, from a drag and drop.
pub(crate) fn to_transferable_content(event: &DragAndDropEvent) -> TransferableContent {
    let entry = event.clip_entry().cloned().unwrap_or_default();
    let metadata = entry.clip_metadata().clone();
    TransferableContent::new(entry, metadata, TransferableContentSource::DragAndDrop, None)
}

/// A receiver's drop node (Compose's ReceiveContentDragAndDropNode, Android's actual): it takes part in every drag
/// (what it takes is its listener's to say), its moments told to the configuration's listener, a drop given to it
/// as content — taken if the listener gave back anything else.
#[allow(non_snake_case)]
pub(crate) fn ReceiveContentDragAndDropNode(configuration: Rc<dyn ReceiveContentConfiguration>) -> DragAndDropNode {
    DragAndDropTargetModifierNode(|_: &DragAndDropEvent| true, Rc::new(ReceiveContentTarget(configuration)))
}

struct ReceiveContentTarget(Rc<dyn ReceiveContentConfiguration>);

impl DragAndDropTarget for ReceiveContentTarget {
    fn on_started(&self, _event: &DragAndDropEvent) {
        self.0.receive_content_listener().on_drag_start();
    }

    fn on_ended(&self, _event: &DragAndDropEvent) {
        self.0.receive_content_listener().on_drag_end();
    }

    fn on_entered(&self, _event: &DragAndDropEvent) {
        self.0.receive_content_listener().on_drag_enter();
    }

    fn on_exited(&self, _event: &DragAndDropEvent) {
        self.0.receive_content_listener().on_drag_exit();
    }

    fn on_drop(&self, event: &DragAndDropEvent) -> bool {
        drag_and_drop_request_permission(event);
        let original = to_transferable_content(event);
        let remaining = self.0.receive_content_listener().on_receive(original.clone());
        remaining.as_ref() != Some(&original)
    }
}

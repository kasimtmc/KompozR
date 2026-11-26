//! Receiving content (Compose's `androidx.compose.foundation.content`):
//! [`Modifier.content_receiver`](ContentReceiverModifiers::content_receiver) makes a layout and what is inside it a
//! receiver of [`TransferableContent`] — what another app or the user gives it by a paste ([`Source::Clipboard`]), a
//! drop ([`Source::DragAndDrop`]) or an input method's commit ([`Source::Keyboard`]) —, told to its
//! [`ReceiveContentListener`]: the nearest receiver gets content first, what it leaves goes to the receiver around
//! it, and what is left at the end goes to the text field the content came to (its plain text inserted).
//!
//! Compose for desktop declares the API but leaves it unimplemented (its desktop actuals print that it isn't
//! supported, its `MediaType`s are empty and its drop node throws); this is Compose's own — Android's — behaviour,
//! on KompozR's clipboards and drags: a receiver is a drop target for every drag (what it takes is its listener's
//! to say), its `MediaType`s are MIME types matched as Android's ClipDescription matches them, and a state-based
//! `BasicTextField` under it routes its paste, its drops and its input method's content through it.
//!
//! [`Source::Clipboard`]: TransferableContentSource::Clipboard
//! [`Source::DragAndDrop`]: TransferableContentSource::DragAndDrop
//! [`Source::Keyboard`]: TransferableContentSource::Keyboard

pub mod internal;

use self::internal::{DynamicReceiveContentConfiguration, ReceiveContentConfiguration, ReceiveContentDragAndDropNode};
use kompoz_runtime::named;
use kompoz_ui::clipboard::{ClipEntry, ClipItem, ClipMetadata};
use kompoz_ui::draganddrop::DragAndDropNode;
use kompoz_ui::modifier::{DelegatedNode, ModifierLocal, ModifierLocalMap, ModifierLocalModifierNode};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// Media types

/// A type of content transferred between apps (Compose's MediaType, Android's actual): a MIME type, matched with
/// wildcards (`text/*`, `*/*`).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MediaType {
    representation: Cow<'static, str>,
}

impl MediaType {
    /// Text of any kind: plain, HTML, styled (`text/*`).
    pub const Text: MediaType = MediaType { representation: Cow::Borrowed("text/*") };
    /// Plain text (`text/plain`).
    pub const PlainText: MediaType = MediaType { representation: Cow::Borrowed("text/plain") };
    /// HTML (`text/html`).
    pub const HtmlText: MediaType = MediaType { representation: Cow::Borrowed("text/html") };
    /// Any image: PNG, JPEG, GIF, … (`image/*`).
    pub const Image: MediaType = MediaType { representation: Cow::Borrowed("image/*") };
    /// Every type (`*/*`).
    pub const All: MediaType = MediaType { representation: Cow::Borrowed("*/*") };

    /// The type of `representation`, a MIME type (RFC 2045).
    pub fn new(representation: impl Into<String>) -> MediaType {
        MediaType { representation: Cow::Owned(representation.into()) }
    }

    /// Its MIME type.
    pub fn representation(&self) -> &str {
        &self.representation
    }
}

impl fmt::Debug for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MediaType(representation='{}')", self.representation)
    }
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// The type of `representation` (Compose's `MediaType(representation)`).
/// Kotlin: `MediaType(representation: String)`
#[allow(non_snake_case)]
pub fn MediaType(representation: impl Into<String>) -> MediaType {
    MediaType::new(representation)
}

// ---------------------------------------------------------------------------------------------------------------
// Transferable content

/// Where content comes from (Compose's `TransferableContent.Source`).
/// Kotlin: `TransferableContent.Source`
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransferableContentSource(i32);

impl TransferableContentSource {
    /// An input method (the soft keyboard) committed it.
    pub const Keyboard: TransferableContentSource = TransferableContentSource(0);
    /// The system's drag and drop dropped it.
    pub const DragAndDrop: TransferableContentSource = TransferableContentSource(1);
    /// A paste from the clipboard ("Paste" in a menu, Ctrl+V).
    pub const Clipboard: TransferableContentSource = TransferableContentSource(2);
}

impl fmt::Debug for TransferableContentSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            0 => f.write_str("Source.Keyboard"),
            1 => f.write_str("Source.DragAndDrop"),
            2 => f.write_str("Source.Clipboard"),
            v => write!(f, "Invalid ({v})"),
        }
    }
}

impl fmt::Display for TransferableContentSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// What a platform gives with content besides it (Compose's PlatformTransferableContent): the link an input
/// method's content points at (Android's `InputContentInfo.linkUri`), if it gave one.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PlatformTransferableContent {
    pub link_uri: Option<String>,
}

struct ContentData {
    clip_entry: ClipEntry,
    clip_metadata: ClipMetadata,
    source: TransferableContentSource,
    platform_transferable_content: Option<PlatformTransferableContent>,
}

/// Content transferred between apps or within one (Compose's TransferableContent): its entry (text, an image,
/// files, …), what describes it, where it came from, and what the platform gives besides. Cheap to clone; equal
/// when the same (Compose's is a class, compared by identity: a listener returning what it was given has taken
/// nothing).
#[derive(Clone)]
pub struct TransferableContent(Rc<ContentData>);

impl PartialEq for TransferableContent {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for TransferableContent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TransferableContent")
            .field("clip_entry", &self.0.clip_entry)
            .field("source", &self.0.source)
            .field("platform_transferable_content", &self.0.platform_transferable_content)
            .finish()
    }
}

impl TransferableContent {
    /// Content of `clip_entry` described by `clip_metadata`, from `source` (Compose's internal constructor: content
    /// comes from a paste, a drop or an input method).
    pub(crate) fn new(
        clip_entry: ClipEntry,
        clip_metadata: ClipMetadata,
        source: TransferableContentSource,
        platform_transferable_content: Option<PlatformTransferableContent>,
    ) -> TransferableContent {
        TransferableContent(Rc::new(ContentData { clip_entry, clip_metadata, source, platform_transferable_content }))
    }

    /// What it holds.
    pub fn clip_entry(&self) -> &ClipEntry {
        &self.0.clip_entry
    }

    /// What describes it.
    pub fn clip_metadata(&self) -> &ClipMetadata {
        &self.0.clip_metadata
    }

    /// Where it came from.
    pub fn source(&self) -> TransferableContentSource {
        self.0.source
    }

    /// What the platform gives with it.
    pub fn platform_transferable_content(&self) -> Option<&PlatformTransferableContent> {
        self.0.platform_transferable_content.as_ref()
    }

    /// Whether it can give an item of `media_type` (Compose's `hasMediaType`: its metadata has a type of it).
    pub fn has_media_type(&self, media_type: &MediaType) -> bool {
        self.0.clip_metadata.has_mime_type(media_type.representation())
    }

    /// Takes the items `predicate` says it took (true), giving what is left to pass on (Compose's Android
    /// `consume`): none if it took them all, itself if it took none, else content of the items left (with the same
    /// metadata, source and platform content).
    pub fn consume(&self, predicate: impl Fn(&ClipItem) -> bool) -> Option<TransferableContent> {
        let items = self.0.clip_entry.items();
        if items.len() == 1 {
            return (!predicate(&items[0])).then(|| self.clone());
        }
        let remaining: Vec<ClipItem> = items.iter().filter(|i| !predicate(i)).cloned().collect();
        if remaining.is_empty() {
            return None;
        }
        if remaining.len() == items.len() {
            return Some(self.clone());
        }
        let metadata = self.0.clip_metadata.clone();
        Some(TransferableContent::new(
            ClipEntry::with_metadata(remaining, metadata.clone()),
            metadata,
            self.0.source,
            self.0.platform_transferable_content.clone(),
        ))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The listener

/// What a content receiver does (Compose's ReceiveContentListener, a `fun interface`): told of drags of any content
/// over it, and given the content that comes to it. Any `Fn(TransferableContent) -> Option<TransferableContent>` is
/// one (Kotlin's `{ content -> … }`: its `onReceive`).
pub trait ReceiveContentListener {
    /// A drag and drop of any content started in the app (whether over this receiver or not).
    fn on_drag_start(&self) {}
    /// The drag and drop ended (dropped, or called off).
    fn on_drag_end(&self) {}
    /// The drag came over this receiver (once, whatever receivers or fields inside it it then goes over).
    fn on_drag_enter(&self) {}
    /// The drag left this receiver.
    fn on_drag_exit(&self) {}
    /// Content came: what it doesn't take, it gives back (`consume` helps) — none if it took it all. What is given
    /// back goes to the receiver around it, then to the text field it came to.
    fn on_receive(&self, transferable_content: TransferableContent) -> Option<TransferableContent>;
}

impl<F: Fn(TransferableContent) -> Option<TransferableContent>> ReceiveContentListener for F {
    fn on_receive(&self, transferable_content: TransferableContent) -> Option<TransferableContent> {
        self(transferable_content)
    }
}

/// A shared listener (Kotlin's reference to one: an `object : ReceiveContentListener { … }`) is one.
impl ReceiveContentListener for Rc<dyn ReceiveContentListener> {
    fn on_drag_start(&self) {
        (**self).on_drag_start()
    }
    fn on_drag_end(&self) {
        (**self).on_drag_end()
    }
    fn on_drag_enter(&self) {
        (**self).on_drag_enter()
    }
    fn on_drag_exit(&self) {
        (**self).on_drag_exit()
    }
    fn on_receive(&self, transferable_content: TransferableContent) -> Option<TransferableContent> {
        (**self).on_receive(transferable_content)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The modifier

/// The content receiving modifier of foundation.
#[named(trait ContentReceiverModifiers)]
impl Modifier {
    /// Makes the layout and what is inside it a receiver of content (Compose's `Modifier.contentReceiver`): pasted,
    /// dropped or committed by an input method, given to `receive_content_listener` — the nearest receiver first,
    /// what it gives back to the one around it, then to the text field it came to. Nothing is filtered by type: a
    /// listener checks what it is given (`has_media_type`, `clip_metadata`). Of the text fields, the state-based
    /// `BasicTextField` takes part (as Compose's).
    fn content_receiver(self, receive_content_listener: impl ReceiveContentListener + 'static) -> Modifier {
        self.then_element(ReceiveContentElement { receive_content_listener: Rc::new(receive_content_listener) })
    }
}

/// `content_receiver`'s element (Compose's ReceiveContentElement: equal by its listener).
struct ReceiveContentElement {
    receive_content_listener: Rc<dyn ReceiveContentListener>,
}

impl PartialEq for ReceiveContentElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.receive_content_listener, &other.receive_content_listener)
    }
}

impl ModifierNodeElement for ReceiveContentElement {
    type Node = ReceiveContentNode;
    fn create(&self) -> ReceiveContentNode {
        ReceiveContentNode::new(self.receive_content_listener.clone())
    }
    fn update(&self, node: &mut ReceiveContentNode) {
        node.update_node(self.receive_content_listener.clone());
    }
    fn name(&self) -> &'static str {
        "receiveContent"
    }
}

/// The configuration of the nearest receiver (Compose's ModifierLocalReceiveContent): what receivers provide to the
/// receivers and fields inside them.
#[allow(non_upper_case_globals)]
pub(crate) static ModifierLocalReceiveContent: ModifierLocal<Option<Rc<dyn ReceiveContentConfiguration>>> =
    ModifierLocal::new(|| None);

/// What a receiver's node and its configuration share: its listener now, and where it is.
pub(crate) struct ReceiveContentNodeState {
    pub(crate) receive_content_listener: RefCell<Rc<dyn ReceiveContentListener>>,
    pub(crate) handle: RefCell<Option<NodeHandle>>,
}

/// A receiver's node (Compose's ReceiveContentNode): it provides its configuration to what is inside it (its
/// listener, then the receivers' around it: Compose's ModifierLocalReceiveContent), and it is a drop target for
/// every drag (a delegate), its drops given to the configuration.
pub struct ReceiveContentNode {
    state: Rc<ReceiveContentNodeState>,
    provided_values: ModifierLocalMap,
    drag_and_drop_node: Option<DelegatedNode<DragAndDropNode>>,
    receive_content_configuration: Rc<dyn ReceiveContentConfiguration>,
}

impl ModifierLocalModifierNode for ReceiveContentNode {
    fn provided_values(&self) -> &ModifierLocalMap {
        &self.provided_values
    }
}

impl ReceiveContentNode {
    fn new(receive_content_listener: Rc<dyn ReceiveContentListener>) -> ReceiveContentNode {
        let state = Rc::new(ReceiveContentNodeState {
            receive_content_listener: RefCell::new(receive_content_listener),
            handle: RefCell::new(None),
        });
        let receive_content_configuration: Rc<dyn ReceiveContentConfiguration> =
            Rc::new(DynamicReceiveContentConfiguration::new(state.clone()));
        let provided_values =
            ModifierLocalMap::of(&ModifierLocalReceiveContent, Some(receive_content_configuration.clone()));
        ReceiveContentNode { state, provided_values, drag_and_drop_node: None, receive_content_configuration }
    }

    fn update_node(&mut self, receive_content_listener: Rc<dyn ReceiveContentListener>) {
        *self.state.receive_content_listener.borrow_mut() = receive_content_listener;
    }
}

impl ModifierNode for ReceiveContentNode {
    node_capabilities!(modifier_local);

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.state.handle.borrow_mut() = Some(cx.handle());
        // (Compose delegates to its drop node as it is made; a node has its context once attached)
        if self.drag_and_drop_node.is_none() {
            let node = ReceiveContentDragAndDropNode(self.receive_content_configuration.clone());
            self.drag_and_drop_node = Some(cx.delegate(node));
        }
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.state.handle.borrow_mut().take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(items: Vec<ClipItem>) -> TransferableContent {
        let entry = ClipEntry::new(items);
        let metadata = entry.clip_metadata().clone();
        TransferableContent::new(entry, metadata, TransferableContentSource::Clipboard, None)
    }

    #[test]
    fn media_types_match_their_mime_types() {
        let c = content(vec![ClipItem::with_html_text("a", "<b>a</b>")]);
        assert!(c.has_media_type(&MediaType::Text));
        assert!(c.has_media_type(&MediaType::HtmlText));
        assert!(c.has_media_type(&MediaType::PlainText));
        assert!(c.has_media_type(&MediaType::All));
        assert!(!c.has_media_type(&MediaType::Image));
        assert!(c.has_media_type(&MediaType("text/html")));
        assert_eq!(MediaType("image/*"), MediaType::Image);
        assert_eq!(MediaType::Image.to_string(), "MediaType(representation='image/*')");
        assert_eq!(TransferableContentSource::DragAndDrop.to_string(), "Source.DragAndDrop");
    }

    #[test]
    fn consume_takes_items_and_gives_what_is_left() {
        let one = content(vec![ClipItem::with_text("a")]);
        assert_eq!(one.consume(|_| true), None);
        assert_eq!(one.consume(|_| false), Some(one.clone())); // (itself)
        let three = content(vec![ClipItem::with_text("a"), ClipItem::with_uri("file:///b"), ClipItem::with_text("c")]);
        assert_eq!(three.consume(|_| false), Some(three.clone()));
        assert_eq!(three.consume(|_| true), None);
        let left = three.consume(|i| i.uri().is_some()).unwrap();
        assert_ne!(left, three);
        assert_eq!(left.clip_entry().item_count(), 2);
        assert_eq!(left.clip_entry().read_plain_text().as_deref(), Some("a\nc"));
        assert_eq!(left.clip_metadata(), three.clip_metadata()); // (Android's: its description copied)
        assert_eq!(left.source(), TransferableContentSource::Clipboard);
    }
}

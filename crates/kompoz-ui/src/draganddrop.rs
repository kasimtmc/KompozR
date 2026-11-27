//! Drag and drop, as Compose's `androidx.compose.ui.draganddrop`: [`DragAndDropNode`] (Compose's DragAndDropNode,
//! made by [`DragAndDropSourceModifierNode()`] and [`DragAndDropTargetModifierNode()`]) is both what starts a drag
//! ([`request_drag_and_drop_transfer`]: its `on_start_transfer` given a [`DragAndDropStartTransferScope`] whose
//! `start_drag_and_drop_transfer` hands the drag its [`DragAndDropTransferData`] and decoration) and what takes one:
//! when a drag starts, every target node is asked whether it wants it (`should_start_drag_and_drop`); those that do
//! hear it start, enter and leave them as it moves (the innermost under it), the drop (the one under it), and its
//! end. The tree's root node is the host's ([`DragAndDropManager`], Compose's DragAndDropManager/DragAndDropOwner).
//!
//! Drags come from two places:
//! - in the app (C++ Kompoz's own session, what Compose leaves to the platform): a source's transfer starts a drag
//!   the host runs itself — it takes the pointer that pressed (the gesture's other handlers are cancelled), moves
//!   with it, draws the decoration under it (faded: Android's drag shadow), drops where it is let go, is called
//!   off by Escape. One that leaves the window is handed to the platform when it can take it
//!   ([`PlatformCallbacks::start_drag_and_drop`](crate::platform::PlatformCallbacks::start_drag_and_drop): the
//!   outgoing drags of Wayland's data device and X11's XDND), as Compose's drags are the platform's from the start;
//!   coming back over the window it is the app's again, with what it carries (`local_state` too), and the platform
//!   tells its end through [`KompozHost::platform_drag_ended`](crate::KompozHost::platform_drag_ended).
//! - from outside (files from a file manager, text from a browser), as the platform reports them: the host's
//!   `external_drag_entered`, `external_drag_moved`, `external_drop`, `external_drag_left`.
//!
//! Positions are px in the window.

// (Compose's names: `DragAndDropSourceModifierNode(…)` and its siblings are functions)
#![allow(non_snake_case)]

use crate::clipboard::{
    ClipEntry, ClipItem, ClipMetadata, ClipValue, MIMETYPE_TEXT_PLAIN, MIMETYPE_TEXT_URILIST, compare_mime_types,
    file_path_of_uri, is_plain_text_type,
};
use crate::draw::{BitmapPainter, DrawScope, GraphicsLayer};
use crate::graphics_context::{CornerRadii, GraphicsContext, LayerTransform};
use crate::input::KeyModifiers;
use crate::layout::LayoutAwareModifierNode;
use crate::modifier::{ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, TraversableNode, TraversalDecision};
use crate::node_capabilities;
use crate::platform::PlatformHandle;
use kompoz_graphics::{
    Density, FilterQuality, ImageBitmap, IntOffset, IntSize, LayoutDirection, Matrix, Offset, Rect, Size,
};
use kompoz_runtime::named;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

// ---------------------------------------------------------------------------------------------------------------
// What a drag carries

/// What a drag does to what it carries (Compose desktop's DragAndDropTransferAction): copies, moves or links it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DragAndDropTransferAction(&'static str);

impl DragAndDropTransferAction {
    /// The target gets a copy (Compose's `Copy`).
    pub const Copy: DragAndDropTransferAction = DragAndDropTransferAction("Copy");
    /// It is moved: the source lets go of it (Compose's `Move`).
    pub const Move: DragAndDropTransferAction = DragAndDropTransferAction("Move");
    /// The target links to it (Compose's `Link`).
    pub const Link: DragAndDropTransferAction = DragAndDropTransferAction("Link");
}

impl fmt::Display for DragAndDropTransferAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl fmt::Debug for DragAndDropTransferAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// What a drag carries (Compose's DragAndDropTransferData, the platforms' own there: Android's ClipData with its
/// local state, the desktop's transferable with its actions): a [`ClipEntry`] — text, HTML, an image, files (as
/// `file://` URIs), any type of the app's —, described by its MIME types; within the app, anything (`local_state`,
/// Android's); the actions its source allows (the desktop's `supportedActions`, all three by default), where the
/// pointer is in its decoration (the desktop's `dragDecorationOffset`; none: its middle, Android's touch point) and
/// what its source is told as it ends (the desktop's `onTransferCompleted`: the action a target took it with, or
/// none). A drop from another app carries what that app offers, read as it is asked for.
#[derive(Clone)]
pub struct DragAndDropTransferData {
    pub clip_entry: ClipEntry,
    pub local_state: Option<Rc<dyn Any>>,
    pub supported_actions: Vec<DragAndDropTransferAction>,
    pub drag_decoration_offset: Option<Offset>,
    pub on_transfer_completed: Option<Rc<dyn Fn(Option<DragAndDropTransferAction>)>>,
}

impl Default for DragAndDropTransferData {
    fn default() -> Self {
        DragAndDropTransferData {
            clip_entry: ClipEntry::default(),
            local_state: None,
            supported_actions: vec![
                DragAndDropTransferAction::Copy,
                DragAndDropTransferAction::Move,
                DragAndDropTransferAction::Link,
            ],
            drag_decoration_offset: None,
            on_transfer_completed: None,
        }
    }
}

fn same_rc<T: ?Sized>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

impl PartialEq for DragAndDropTransferData {
    /// The same entry, local state and completion callback (by instance), actions and offset.
    fn eq(&self, o: &Self) -> bool {
        self.clip_entry == o.clip_entry
            && same_rc(&self.local_state, &o.local_state)
            && self.supported_actions == o.supported_actions
            && self.drag_decoration_offset == o.drag_decoration_offset
            && same_rc(&self.on_transfer_completed, &o.on_transfer_completed)
    }
}

impl fmt::Debug for DragAndDropTransferData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("DragAndDropTransferData")
            .field("clip_entry", &self.clip_entry)
            .field("local_state", &self.local_state.is_some())
            .field("supported_actions", &self.supported_actions)
            .field("drag_decoration_offset", &self.drag_decoration_offset)
            .finish()
    }
}

impl DragAndDropTransferData {
    /// Its MIME types (its entry's metadata).
    pub fn mime_types(&self) -> Vec<String> {
        self.clip_entry.clip_metadata().mime_types().to_vec()
    }

    /// Whether it is of `mime_type` (wildcards matched as Android's `hasMimeType`: `text/*`).
    pub fn has_mime_type(&self, mime_type: &str) -> bool {
        self.clip_entry.clip_metadata().has_mime_type(mime_type)
    }

    /// What it carries as text: its items' text (a line between each), else its URIs ([`ClipEntry::coerce_to_text`]:
    /// a file manager's drop).
    pub fn text(&self) -> String {
        self.clip_entry.read_plain_text().unwrap_or_else(|| self.clip_entry.coerce_to_text())
    }

    /// The local files it carries (its `file:` URIs' paths).
    pub fn files(&self) -> Vec<String> {
        self.clip_entry.files()
    }

    /// Its local state, if it is a `T`.
    pub fn local_state<T: Any>(&self) -> Option<&T> {
        self.local_state.as_ref().and_then(|s| s.downcast_ref::<T>())
    }
}

/// The entry of a drag given as text, its MIME types and files (KompozR's named form): an item holding `text` as
/// each of `mime_types` (the app's own types served as its text; `text/uri-list` left to the files when there are
/// some; `text/plain` too when there is text but no plain text type), then an item per file; described by
/// `mime_types` in the app's order.
fn entry_of(text: String, mime_types: Vec<String>, files: Vec<String>) -> ClipEntry {
    let mut items = Vec::new();
    let mut representations: Vec<(String, ClipValue)> = mime_types
        .iter()
        .filter(|m| files.is_empty() || *m != MIMETYPE_TEXT_URILIST)
        .cloned()
        .map(|m| (m, ClipValue::Text(text.clone())))
        .collect();
    if !text.is_empty() && !representations.iter().any(|(m, _)| is_plain_text_type(m)) {
        representations.push((MIMETYPE_TEXT_PLAIN.into(), ClipValue::Text(text)));
    }
    if !representations.is_empty() {
        items.push(ClipItem::of(representations));
    }
    items.extend(files.iter().map(|f| ClipItem::with_file(f)));
    // (described by its types as the app gave them, then those its items add: text/plain, text/uri-list)
    let mut described = mime_types;
    for t in ClipEntry::new(items.clone()).clip_metadata().mime_types() {
        if !described.iter().any(|d| compare_mime_types(d, t)) {
            described.push(t.clone());
        }
    }
    ClipEntry::with_metadata(items, ClipMetadata::new(described))
}

/// What a drag carries, by name (KompozR's: Compose's is each platform's own class): its `clip_entry` (Android's
/// ClipData), or one made of `text` (as each of `mime_types`, "text/plain" by default) and `files`.
/// Kotlin: `DragAndDropTransferData(clipEntry: ClipEntry? = null, text: String = "", mimeTypes: List<String> = listOf("text/plain"), files: List<String> = emptyList(), localState: Any? = null, supportedActions: List<DragAndDropTransferAction> = listOf(DragAndDropTransferAction.Copy, DragAndDropTransferAction.Move, DragAndDropTransferAction.Link), dragDecorationOffset: Offset? = null, onTransferCompleted: ((DragAndDropTransferAction?) -> Unit)? = null)`
#[named]
pub fn drag_and_drop_transfer_data(
    #[default(None)] clip_entry: Option<ClipEntry>,
    #[default(String::new())] text: String,
    #[default(vec!["text/plain".to_string()])] mime_types: Vec<String>,
    #[default(Vec::new())] files: Vec<String>,
    #[default(None)] local_state: Option<Rc<dyn Any>>,
    #[default(vec![DragAndDropTransferAction::Copy, DragAndDropTransferAction::Move, DragAndDropTransferAction::Link])]
    supported_actions: Vec<DragAndDropTransferAction>,
    #[default(None)] drag_decoration_offset: Option<Offset>,
    #[default(None)] on_transfer_completed: Option<Rc<dyn Fn(Option<DragAndDropTransferAction>)>>,
) -> DragAndDropTransferData {
    DragAndDropTransferData {
        clip_entry: clip_entry.unwrap_or_else(|| entry_of(text, mime_types, files)),
        local_state,
        supported_actions,
        drag_decoration_offset,
        on_transfer_completed,
    }
}

/// What a drag carries as the desktop reads it (Compose desktop's DragData, of `DragAndDropEvent.dragData()`):
/// files (local `file:` URIs), else an image, else text (of its best text type), else something else.
#[derive(Clone, PartialEq, Debug)]
pub enum DragData {
    FilesList(ClipEntry),
    Image(ClipEntry),
    Text { best_mime_type: String, entry: ClipEntry },
    Unknown,
}

impl DragData {
    /// What `entry` is, as the desktop sees it (Compose desktop's `Transferable.dragData`).
    pub fn of(entry: &ClipEntry) -> DragData {
        let metadata = entry.clip_metadata();
        if metadata.has_mime_type(MIMETYPE_TEXT_URILIST) && !entry.files().is_empty() {
            DragData::FilesList(entry.clone())
        } else if metadata.has_mime_type("image/*") {
            DragData::Image(entry.clone())
        } else if let Some(best) = metadata
            .mime_types()
            .iter()
            .find(|m| is_plain_text_type(m))
            .or_else(|| metadata.mime_types().iter().find(|m| compare_mime_types(m, "text/*")))
        {
            DragData::Text { best_mime_type: best.clone(), entry: entry.clone() }
        } else {
            DragData::Unknown
        }
    }

    /// The files' URIs (Compose desktop's `FilesList.readFiles`); none for another kind.
    pub fn read_files(&self) -> Vec<String> {
        match self {
            DragData::FilesList(entry) => entry.uris().into_iter().filter(|u| file_path_of_uri(u).is_some()).collect(),
            _ => Vec::new(),
        }
    }

    /// The image, painted (Compose desktop's `Image.readImage`); none for another kind, or one that can't be read.
    pub fn read_image(&self) -> Option<BitmapPainter> {
        match self {
            DragData::Image(entry) => entry.image().map(|i| {
                let size = IntSize::new(i.width(), i.height());
                BitmapPainter::new(i, IntOffset::Zero, size, FilterQuality::Low)
            }),
            _ => None,
        }
    }

    /// The text (Compose desktop's `Text.readText`); empty for another kind.
    pub fn read_text(&self) -> String {
        match self {
            DragData::Text { entry, .. } => entry.read_plain_text().unwrap_or_else(|| entry.coerce_to_text()),
            _ => String::new(),
        }
    }

    /// The text's best type (Compose desktop's `Text.bestMimeType`); empty for another kind.
    pub fn best_mime_type(&self) -> &str {
        match self {
            DragData::Text { best_mime_type, .. } => best_mime_type,
            _ => "",
        }
    }
}

/// A drag's moment (Compose's DragAndDropEvent): what it may carry (its MIME types: all that is known of a drag
/// from outside before its drop), where it is (px in the window), what it carries (a drag within the app: from its
/// start; from outside: at its drop, read as it is asked for), the action the user picks (the desktop's `action`:
/// within the app Move, Ctrl Copy, Ctrl+Shift Link, as AWT's, of those the source allows), and as it ends whether a
/// target took it (Android's `DragEvent.getResult`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct DragAndDropEvent {
    pub action: Option<DragAndDropTransferAction>,
    pub mime_types: Vec<String>,
    pub position_in_root: Offset,
    pub transfer_data: Option<DragAndDropTransferData>,
    pub result: bool,
}

impl DragAndDropEvent {
    /// Its MIME types (Compose's `mimeTypes()`).
    pub fn mime_types(&self) -> &[String] {
        &self.mime_types
    }

    /// Its MIME types as a description (Android's `DragEvent.getClipDescription`).
    pub fn clip_metadata(&self) -> ClipMetadata {
        ClipMetadata::new(self.mime_types.clone())
    }

    /// Whether it may be of `mime_type` (wildcards matched as Android's `hasMimeType`: `text/*`).
    pub fn has_mime_type(&self, mime_type: &str) -> bool {
        self.mime_types.iter().any(|m| compare_mime_types(m, mime_type))
    }

    /// What it carries, once known (Android's `DragEvent.getClipData`).
    pub fn clip_entry(&self) -> Option<&ClipEntry> {
        self.transfer_data.as_ref().map(|d| &d.clip_entry)
    }

    /// What it carries as text, once known (empty before).
    pub fn text(&self) -> String {
        self.transfer_data.as_ref().map(DragAndDropTransferData::text).unwrap_or_default()
    }

    /// What it carries as the desktop reads it (Compose desktop's `dragData()`): unknown before it is known.
    pub fn drag_data(&self) -> DragData {
        self.clip_entry().map_or(DragData::Unknown, DragData::of)
    }

    /// Its local state, if it carries a `T` (a drag within the app).
    pub fn local_state<T: Any>(&self) -> Option<&T> {
        self.transfer_data.as_ref().and_then(|d| d.local_state::<T>())
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

/// What draws a drag's decoration (Compose's `drawDragDecoration: DrawScope.() -> Unit`), in a scope of its size.
pub type DrawDragDecoration = Rc<dyn Fn(&mut DrawScope<'_>)>;

/// What a source starts its drag with (Compose's DragAndDropStartTransferScope).
pub trait DragAndDropStartTransferScope {
    /// Starts a drag carrying `transfer_data`, shown by `draw_drag_decoration` in `decoration_size` px (Compose's
    /// `startDragAndDropTransfer`): whether it started (not while another drag is under way).
    fn start_drag_and_drop_transfer(
        &self,
        transfer_data: DragAndDropTransferData,
        decoration_size: Size,
        draw_drag_decoration: DrawDragDecoration,
    ) -> bool;
}

/// Whether a target wants a drag that starts (Compose's `shouldStartDragAndDrop`).
pub type ShouldStartDragAndDrop = Rc<dyn Fn(&DragAndDropEvent) -> bool>;

/// What a source does when asked to start a drag, given where (local to it; unspecified when not told:
/// Compose's `onStartTransfer: DragAndDropStartTransferScope.(Offset) -> Unit`).
pub type OnStartTransfer = Rc<dyn Fn(&dyn DragAndDropStartTransferScope, Offset)>;

/// What a target node takes a drag with: its target if it wants it (Compose's `onDropTargetValidate`).
type OnDropTargetValidate = Rc<dyn Fn(&DragAndDropEvent) -> Option<Rc<dyn DragAndDropTarget>>>;

// ---------------------------------------------------------------------------------------------------------------
// The node

/// The key drag and drop nodes find each other by.
struct DragAndDropTraversableKey;
static KEY: DragAndDropTraversableKey = DragAndDropTraversableKey;

/// A drag and drop node (Compose's DragAndDropNode): a source starting drags (`on_start_transfer`), a target taking
/// them (`on_drop_target_validate`), and the tree's root one (the host's). It tracks, while a drag goes on, its own
/// target if it wanted the drag and its child node the drag is over (the traversal follows the last event's path).
pub struct DragAndDropNode {
    on_start_transfer: Option<OnStartTransfer>,
    on_drop_target_validate: Option<OnDropTargetValidate>,
    /// Its target while it takes part in the drag under way (Compose's `thisDragAndDropTarget`).
    this_target: Option<Rc<dyn DragAndDropTarget>>,
    /// The innermost child node the drag is over (Compose's `lastChildDragAndDropModifierNode`).
    last_child: Option<NodeHandle>,
    /// Its size as measured (Compose's `size`: not its coordinates', which padding may change).
    size: IntSize,
    /// The root's: where the host finds it.
    root_slot: Option<Rc<RefCell<Option<NodeHandle>>>>,
}

impl fmt::Debug for DragAndDropNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DragAndDropNode(source={}, target={})",
            self.on_start_transfer.is_some(),
            self.on_drop_target_validate.is_some()
        )
    }
}

/// A node starting drags (Compose's `DragAndDropSourceModifierNode(onStartTransfer)`): asked to start one
/// ([`request_drag_and_drop_transfer`]), `on_start_transfer` is given a scope to start it with and where (local to
/// the node).
pub fn DragAndDropSourceModifierNode(
    on_start_transfer: impl Fn(&dyn DragAndDropStartTransferScope, Offset) + 'static,
) -> DragAndDropNode {
    DragAndDropNode::of(Some(Rc::new(on_start_transfer)), None)
}

/// A node taking drags (Compose's `DragAndDropTargetModifierNode(shouldStartDragAndDrop, target)`):
/// `should_start_drag_and_drop` says, as each starts, whether `target` takes part.
pub fn DragAndDropTargetModifierNode(
    should_start_drag_and_drop: impl Fn(&DragAndDropEvent) -> bool + 'static,
    target: Rc<dyn DragAndDropTarget>,
) -> DragAndDropNode {
    let mut node = DragAndDropNode::of(None, None);
    node.update_target(Rc::new(should_start_drag_and_drop), target);
    node
}

/// Compose's deprecated `DragAndDropModifierNode()`: a node that starts drags with [`drag`] (use
/// [`DragAndDropSourceModifierNode()`]).
pub fn DragAndDropModifierNode() -> DragAndDropNode {
    DragAndDropNode::of(None, None)
}

impl DragAndDropNode {
    fn of(on_start_transfer: Option<OnStartTransfer>, on_drop_target_validate: Option<OnDropTargetValidate>) -> Self {
        DragAndDropNode {
            on_start_transfer,
            on_drop_target_validate,
            this_target: None,
            last_child: None,
            size: IntSize::Zero,
            root_slot: None,
        }
    }

    /// A new `on_start_transfer` (an element's update).
    pub fn set_on_start_transfer(&mut self, on_start_transfer: OnStartTransfer) {
        self.on_start_transfer = Some(on_start_transfer);
    }

    /// A new `should_start_drag_and_drop` and `target` (an element's update).
    pub fn update_target(&mut self, should_start: ShouldStartDragAndDrop, target: Rc<dyn DragAndDropTarget>) {
        self.on_drop_target_validate =
            Some(Rc::new(move |event: &DragAndDropEvent| should_start(event).then(|| target.clone())));
    }

    /// Whether a child the drag is over, or itself, takes a drop now (Compose's `hasEligibleDropTarget`).
    pub fn has_eligible_drop_target(&self) -> bool {
        self.last_child.is_some() || self.this_target.is_some()
    }
}

impl ModifierNode for DragAndDropNode {
    node_capabilities!(traversable, layout_aware);

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

impl LayoutAwareModifierNode for DragAndDropNode {
    fn on_remeasured(&mut self, size: IntSize) {
        self.size = size;
    }
}

impl TraversableNode for DragAndDropNode {
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
    type Node = DragAndDropNode;
    fn create(&self) -> DragAndDropNode {
        let mut node = DragAndDropNode::of(None, None);
        node.root_slot = Some(self.0.clone());
        node
    }
    fn update(&self, node: &mut DragAndDropNode) {
        node.root_slot = Some(self.0.clone());
    }
    fn name(&self) -> &'static str {
        "RootDragAndDropNode"
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Starting a drag

/// The host's drag and drop manager of a node's layout.
fn manager_of(cx: &NodeCx) -> Option<Rc<DragAndDropManager>> {
    cx.layout_node().and_then(|n| n.owner()).and_then(|o| o.drag_and_drop())
}

/// Whether a source must start its drags itself (Compose's `isRequestDragAndDropTransferRequired`): on every
/// KompozR platform it does (by a gesture of its own, as on Android and the desktop).
pub fn is_request_drag_and_drop_transfer_required(cx: &NodeCx) -> bool {
    manager_of(cx).is_some()
}

/// Asks the source node `cx` (a [`DragAndDropSourceModifierNode()`]) and the sources inside it under `offset` (px
/// local to it; unspecified: the first that starts one) to start a drag (Compose's `requestDragAndDropTransfer`):
/// each is given a scope to start it with, until one does.
pub fn request_drag_and_drop_transfer(cx: &NodeCx, offset: Offset) {
    let has_source = cx.with(|n: &mut DragAndDropNode| n.on_start_transfer.is_some()).unwrap_or(false);
    if !has_source {
        kompoz_runtime::kt::panic_kt("IllegalStateException", "Check failed.");
    }
    let Some(manager) = manager_of(cx) else { return };
    manager.request_drag_and_drop_transfer(cx, offset);
}

/// Starts a drag carrying `transfer_data` from the node `cx` (Compose's deprecated `DragAndDropModifierNode.drag`:
/// use a source node's [`request_drag_and_drop_transfer`]).
pub fn drag(
    cx: &NodeCx,
    transfer_data: DragAndDropTransferData,
    decoration_size: Size,
    draw_drag_decoration: DrawDragDecoration,
) {
    let had_source = cx.with(|n: &mut DragAndDropNode| n.on_start_transfer.is_some()).unwrap_or(true);
    if had_source {
        kompoz_runtime::kt::panic_kt("IllegalStateException", "Check failed.");
    }
    let data = RefCell::new(Some(transfer_data));
    let start: OnStartTransfer = Rc::new(move |scope: &dyn DragAndDropStartTransferScope, _| {
        if let Some(data) = data.borrow_mut().take() {
            scope.start_drag_and_drop_transfer(data, decoration_size, draw_drag_decoration.clone());
        }
    });
    cx.with(|n: &mut DragAndDropNode| n.on_start_transfer = Some(start));
    if let Some(manager) = manager_of(cx) {
        manager.request_drag_and_drop_transfer(cx, Offset::Unspecified);
    }
    cx.with(|n: &mut DragAndDropNode| n.on_start_transfer = None);
}

/// The source nodes at and below `cx` under `offset` (local to `cx`) given `scope`, until one starts a drag
/// (Compose's `DragAndDropStartTransferScope.startDragAndDropTransfer(offset, isTransferStarted)`).
fn start_transfer_traversal(
    cx: &NodeCx,
    scope: &dyn DragAndDropStartTransferScope,
    offset: Offset,
    is_transfer_started: &dyn Fn() -> bool,
) {
    let node_coordinates = cx.layout_node().map(|n| n.coordinates());
    let mut visit = |current: &NodeCx| -> TraversalDecision {
        if !current.is_attached() {
            return TraversalDecision::SkipSubtreeAndContinueTraversal;
        }
        let Some((on_start_transfer, size)) =
            current.with(|n: &mut DragAndDropNode| n.on_start_transfer.clone().map(|s| (s, n.size))).flatten()
        else {
            return TraversalDecision::ContinueTraversal;
        };
        if offset.is_specified() {
            let (Some(node_coordinates), Some(current_coordinates)) =
                (node_coordinates.as_ref(), current.layout_node().map(|n| n.coordinates()))
            else {
                return TraversalDecision::ContinueTraversal;
            };
            let local_position = current_coordinates.local_position_of(node_coordinates, offset);
            let bounds = Rect::new(0.0, 0.0, size.width as f32, size.height as f32);
            if !bounds.contains(local_position) {
                return TraversalDecision::ContinueTraversal;
            }
            on_start_transfer(scope, local_position);
        } else {
            on_start_transfer(scope, Offset::Unspecified);
        }
        if is_transfer_started() { TraversalDecision::CancelTraversal } else { TraversalDecision::ContinueTraversal }
    };
    // (itself first, then what is inside it: Compose's traverseSelfAndDescendants)
    if visit(cx) != TraversalDecision::ContinueTraversal {
        return;
    }
    cx.traverse_descendants(&KEY, &mut visit);
}

// ---------------------------------------------------------------------------------------------------------------
// The targets: Compose's DragAndDropNode's target part

/// The node's (this target, last child), copied out: no borrow is held while targets run.
fn parts(cx: &NodeCx) -> (Option<Rc<dyn DragAndDropTarget>>, Option<NodeHandle>) {
    cx.with(|n: &mut DragAndDropNode| (n.this_target.clone(), n.last_child.clone())).unwrap_or((None, None))
}

fn set_last_child(cx: &NodeCx, child: Option<NodeHandle>) {
    cx.with(|n: &mut DragAndDropNode| n.last_child = child);
}

/// Whether the node's layout, of its measured size, is under `position` (px in the window: Compose's `contains`).
fn contains(cx: &NodeCx, position: Offset) -> bool {
    if !cx.is_attached() {
        return false;
    }
    let Some(layout) = cx.layout_node() else { return false };
    let coordinates = layout.coordinates();
    if !coordinates.is_attached() {
        return false;
    }
    let p = coordinates.position_in_root();
    let size = cx.with(|n: &mut DragAndDropNode| n.size).unwrap_or(IntSize::Zero);
    position.x >= p.x
        && position.x <= p.x + size.width as f32
        && position.y >= p.y
        && position.y <= p.y + size.height as f32
}

fn same(a: &NodeHandle, b: &NodeHandle) -> bool {
    Weak::ptr_eq(&a.cell, &b.cell)
}

/// The nodes at and below `root`, attached ones.
fn self_and_descendants(root: &NodeCx) -> Vec<NodeCx> {
    let mut nodes = vec![root.clone()];
    root.traverse_descendants(&KEY, &mut |cx| {
        if !cx.is_attached() {
            return TraversalDecision::SkipSubtreeAndContinueTraversal;
        }
        nodes.push(cx.clone());
        TraversalDecision::ContinueTraversal
    });
    nodes
}

/// Every node at and below `root` asked whether its target wants the drag (Compose's `acceptDragAndDropTransfer`);
/// whether any did. Those that did are registered as interested.
fn accept(root: &NodeCx, event: &DragAndDropEvent, interested: &mut Vec<NodeHandle>) -> bool {
    let mut accepted = false;
    for cx in self_and_descendants(root) {
        let validate = cx.with(|n: &mut DragAndDropNode| n.on_drop_target_validate.clone()).flatten();
        let wanted = validate.and_then(|v| v(event));
        let wants = wanted.is_some();
        cx.with(|n: &mut DragAndDropNode| n.this_target = wanted);
        if wants {
            interested.push(cx.handle());
        }
        accepted |= wants;
    }
    accepted
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
fn node_dispatch_entered(cx: &NodeCx, event: &DragAndDropEvent, interested: &[NodeHandle]) {
    node_entered(cx, event);
    moved(cx, event, interested);
}

/// Whether `cx`'s node is among the interested.
fn is_interested(interested: &[NodeHandle], cx: &NodeCx) -> bool {
    interested.iter().any(|h| h.cx().is_some_and(|c| c.is_same_node(cx)))
}

/// The drag moved over `cx`'s node (Compose's DragAndDropNode `onMoved`): into a child that wanted it, out of one
/// back to the node, from one child to another, or on within the same.
fn moved(cx: &NodeCx, event: &DragAndDropEvent, interested: &[NodeHandle]) {
    let (this, current) = parts(cx);
    let position = event.position_in_root;
    let new_child = match &current {
        Some(c) if c.cx().is_some_and(|c| contains(&c, position)) => Some(c.clone()),
        _ => {
            let mut found = None;
            if cx.is_attached() {
                cx.traverse_descendants(&KEY, &mut |child| {
                    if is_interested(interested, child) && contains(child, position) {
                        found = Some(child.handle());
                        TraversalDecision::CancelTraversal
                    } else {
                        TraversalDecision::ContinueTraversal
                    }
                });
            }
            found
        }
    };
    match (&new_child, &current) {
        (Some(new), None) => {
            if let Some(new) = new.cx() {
                node_dispatch_entered(&new, event, interested);
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
                node_dispatch_entered(&new, event, interested);
            }
            if let Some(old) = old.cx() {
                exited(&old, event);
            }
        }
        (Some(new), Some(_)) => {
            if let Some(new) = new.cx() {
                moved(&new, event, interested);
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

/// The drag changed over `cx`'s node (Compose's `onChanged`): its target's, else the child's it is over.
fn changed(cx: &NodeCx, event: &DragAndDropEvent) {
    let (this, child) = parts(cx);
    match (this, child) {
        (Some(t), _) => t.on_changed(event),
        (None, Some(c)) => {
            if let Some(c) = c.cx() {
                changed(&c, event);
            }
        }
        _ => {}
    }
}

/// The drag left `cx`'s node (Compose's `onExited`).
fn exited(cx: &NodeCx, event: &DragAndDropEvent) {
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
fn dropped(cx: &NodeCx, event: &DragAndDropEvent) -> bool {
    let (this, child) = parts(cx);
    match (child.and_then(|c| c.cx()), this) {
        (Some(c), _) => dropped(&c, event),
        (None, Some(t)) => t.on_drop(event),
        (None, None) => false,
    }
}

/// The drag ended: every node at and below `root` that took part told, and forgets it (Compose's `onEnded`).
fn ended(root: &NodeCx, event: &DragAndDropEvent) {
    for cx in self_and_descendants(root) {
        let this = cx
            .with(|n: &mut DragAndDropNode| {
                n.last_child = None;
                n.this_target.take()
            })
            .flatten();
        if let Some(t) = this {
            t.on_ended(event);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The manager

/// A drag within the app the host runs (C++ Kompoz's session): what it carries, its decoration, where the pointer
/// is and which one it follows.
struct InAppDrag {
    data: DragAndDropTransferData,
    decoration_size: Size,
    draw_drag_decoration: DrawDragDecoration,
    /// Where the pointer is in the decoration.
    decoration_offset: Offset,
    pointer: Offset,
    /// The finger it follows (the host's pointer id), none for the mouse.
    pointer_id: Option<i64>,
    /// The host hasn't taken the pointer yet (the gesture's handlers cancelled, the pointer chosen).
    pending: bool,
    /// What draws the decoration.
    layer: Option<GraphicsLayer>,
}

/// A drag within the app handed to the platform as it left the window: what it carries, its decoration.
struct HandedOff {
    data: DragAndDropTransferData,
}

/// What a platform is given to start its own drag of a drag within the app that left the window (WP5b's: Wayland's
/// `wl_data_device.start_drag` with a data source of its MIME types and an icon surface of its decoration, X11's
/// XDND source): what it carries, its decoration (draw it with [`draw`](PlatformDragAndDrop::draw), `size` px at
/// the host's density) and where the pointer is in it, the pointer's position in the window. A platform may keep a
/// clone while its drag runs (to draw its icon again for a screen of another scale).
#[derive(Clone)]
pub struct PlatformDragAndDrop {
    pub transfer_data: DragAndDropTransferData,
    pub decoration_size: Size,
    pub decoration_offset: Offset,
    pub pointer: Offset,
    pub density: Density,
    draw_drag_decoration: DrawDragDecoration,
    /// The host's callbacks (what draws offscreen).
    platform: PlatformHandle,
}

impl PlatformDragAndDrop {
    /// Draws the decoration into `canvas`, from its top left (what a platform makes its drag icon of).
    pub fn draw(&self, canvas: &mut dyn GraphicsContext) {
        let mut scope = DrawScope::at(canvas, Offset::Zero, self.decoration_size, LayoutDirection::Ltr, self.density);
        (self.draw_drag_decoration)(&mut scope);
    }

    /// The decoration as the app shows it under the pointer (faded, as the drag within the app draws it) drawn into
    /// an image `scale` times its size (a screen of another scale than the window's: its own pixels), through the
    /// host's offscreen drawing ([`PlatformCallbacks::render_image`](crate::platform::PlatformCallbacks)): what a
    /// platform makes its drag icon of. None where nothing draws offscreen (or the decoration is empty).
    pub fn render_icon(&self, scale: f32) -> Option<ImageBitmap> {
        let size = Size::new(self.decoration_size.width * scale, self.decoration_size.height * scale);
        if !(size.width >= 1.0 && size.height >= 1.0) {
            return None;
        }
        self.platform.get().render_image(size, &|canvas| {
            let bounds = Rect::new(0.0, 0.0, size.width, size.height);
            canvas.push_layer(bounds, CornerRadii::Zero, DecorationAlpha, LayerTransform::Identity);
            let mut by = Matrix::IDENTITY;
            by.scale(scale, scale, 1.0);
            canvas.push_transform(&by);
            self.draw(canvas);
            canvas.pop_transform();
            canvas.pop_layer();
        })
    }
}

/// The alpha a drag's decoration is drawn with under the pointer: faded, as Android's drag shadow is unless opaque
/// (C++ Kompoz's 0.75).
const DecorationAlpha: f32 = 0.75;

/// The host's drag and drop (Compose's DragAndDropManager, as the desktop's DragAndDropOwner): the root node, the
/// targets interested in the drag under way, and that drag — one from outside (its MIME types), or one within the
/// app (run here; handed to the platform when it leaves the window).
pub struct DragAndDropManager {
    root: Rc<RefCell<Option<NodeHandle>>>,
    /// The nodes that wanted the drag under way (Compose's `interestedTargets`).
    interested: RefCell<Vec<NodeHandle>>,
    /// A drag from outside under way: what it may carry.
    external: RefCell<Option<Vec<String>>>,
    in_app: RefCell<Option<InAppDrag>>,
    handed_off: RefCell<Option<HandedOff>>,
    platform: PlatformHandle,
}

impl fmt::Debug for DragAndDropManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DragAndDropManager(in_app={}, external={})",
            self.in_app.borrow().is_some(),
            self.external.borrow().is_some()
        )
    }
}

impl DragAndDropManager {
    pub(crate) fn new(platform: PlatformHandle) -> Rc<DragAndDropManager> {
        Rc::new(DragAndDropManager {
            root: Rc::new(RefCell::new(None)),
            interested: RefCell::new(Vec::new()),
            external: RefCell::new(None),
            in_app: RefCell::new(None),
            handed_off: RefCell::new(None),
            platform,
        })
    }

    /// The root's element (Compose's `modifier`).
    pub(crate) fn root_element(&self) -> RootDragAndDropElement {
        RootDragAndDropElement(self.root.clone())
    }

    fn root(&self) -> Option<NodeCx> {
        self.root.borrow().as_ref().and_then(|h| h.cx())
    }

    /// Whether a drag within the app is under way (the host routes its pointer here).
    pub fn is_dragging(&self) -> bool {
        self.in_app.borrow().is_some()
    }

    /// Whether a drag of the app's own is with the platform (it left the window).
    pub fn is_handed_off(&self) -> bool {
        self.handed_off.borrow().is_some()
    }

    /// Compose's `requestDragAndDropTransfer`: the sources at `cx` under `offset` given a scope that starts a drag
    /// within the app.
    fn request_drag_and_drop_transfer(self: &Rc<Self>, cx: &NodeCx, offset: Offset) {
        let started = Rc::new(Cell::new(false));
        let start_position = match (offset.is_specified(), cx.layout_node()) {
            (true, Some(n)) => Some(n.coordinates().local_to_root(offset)),
            _ => None,
        };
        let scope = StartTransferScope { manager: Rc::downgrade(self), started: started.clone(), start_position };
        let s = started.clone();
        start_transfer_traversal(cx, &scope, offset, &move || s.get());
    }

    /// Starts a drag within the app (what a source's `start_drag_and_drop_transfer` does): not while one is under
    /// way. The host takes the pointer once the gesture that started it is handled.
    fn start_in_app(
        &self,
        data: DragAndDropTransferData,
        decoration_size: Size,
        draw_drag_decoration: DrawDragDecoration,
        start_position: Option<Offset>,
    ) -> bool {
        if self.in_app.borrow().is_some() || self.handed_off.borrow().is_some() || self.external.borrow().is_some() {
            return false;
        }
        let decoration_offset = data
            .drag_decoration_offset
            .unwrap_or(Offset::new(decoration_size.width / 2.0, decoration_size.height / 2.0));
        *self.in_app.borrow_mut() = Some(InAppDrag {
            data,
            decoration_size,
            draw_drag_decoration,
            decoration_offset,
            pointer: start_position.unwrap_or(Offset::Unspecified),
            pointer_id: None,
            pending: true,
            layer: None,
        });
        true
    }

    /// The drag within the app just started, once the gesture that started it is handled: the host gives it the
    /// pointer (`pointer_id`: a finger's, none for the mouse; `at`: where it is) — its other handlers were cancelled
    /// — and the targets are asked whether they want it and told it started and where it is.
    pub(crate) fn take_pointer(&self, pointer_id: Option<i64>, at: Offset) {
        let event = {
            let mut in_app = self.in_app.borrow_mut();
            let Some(drag) = in_app.as_mut().filter(|d| d.pending) else { return };
            drag.pending = false;
            drag.pointer_id = pointer_id;
            if drag.pointer.is_unspecified() {
                drag.pointer = at;
            }
            self.in_app_event(drag, KeyModifiers::None)
        };
        let Some(root) = self.root() else { return };
        self.interested.borrow_mut().clear();
        let mut interested = Vec::new();
        accept(&root, &event, &mut interested);
        *self.interested.borrow_mut() = interested.clone();
        for cx in interested.iter().filter_map(NodeHandle::cx) {
            let (this, _) = parts(&cx);
            if let Some(t) = this {
                t.on_started(&event);
            }
        }
        // (where it is: entered at once, as a drag coming in)
        moved(&root, &event, &interested);
    }

    /// Whether a drag within the app waits for the host to give it the pointer.
    pub(crate) fn is_pending(&self) -> bool {
        self.in_app.borrow().as_ref().is_some_and(|d| d.pending)
    }

    /// The pointer a drag within the app follows (a finger's host id; none: the mouse).
    pub(crate) fn pointer_id(&self) -> Option<i64> {
        self.in_app.borrow().as_ref().and_then(|d| d.pointer_id)
    }

    fn in_app_event(&self, drag: &InAppDrag, modifiers: KeyModifiers) -> DragAndDropEvent {
        DragAndDropEvent {
            action: drop_action(modifiers, &drag.data.supported_actions),
            mime_types: drag.data.mime_types(),
            position_in_root: drag.pointer,
            transfer_data: Some(drag.data.clone()),
            result: false,
        }
    }

    /// The pointer of the drag within the app moved to `position` (px): the targets it enters, moves over and
    /// leaves told; out of the window (`viewport`), handed to the platform if it takes it.
    pub(crate) fn move_in_app(&self, position: Offset, modifiers: KeyModifiers, viewport: IntSize, density: Density) {
        let event = {
            let mut in_app = self.in_app.borrow_mut();
            let Some(drag) = in_app.as_mut().filter(|d| !d.pending) else { return };
            drag.pointer = position;
            self.in_app_event(drag, modifiers)
        };
        let outside = position.x < 0.0
            || position.y < 0.0
            || position.x >= viewport.width as f32
            || position.y >= viewport.height as f32;
        if outside && self.hand_off(density) {
            return;
        }
        if let Some(root) = self.root() {
            let interested = self.interested.borrow().clone();
            moved(&root, &event, &interested);
        }
    }

    /// The drag within the app left the window: the platform asked to take it (its own drag from here on); if it
    /// does, the targets are told it left and ended (as a platform's drag leaving the window: AWT's dragExit).
    fn hand_off(&self, density: Density) -> bool {
        let drag = {
            let in_app = self.in_app.borrow();
            let Some(d) = in_app.as_ref() else { return false };
            PlatformDragAndDrop {
                transfer_data: d.data.clone(),
                decoration_size: d.decoration_size,
                decoration_offset: d.decoration_offset,
                pointer: d.pointer,
                density,
                draw_drag_decoration: d.draw_drag_decoration.clone(),
                platform: self.platform.clone(),
            }
        };
        let callbacks = self.platform.get();
        if !callbacks.start_drag_and_drop(&drag) {
            return false;
        }
        let Some(d) = self.in_app.borrow_mut().take() else { return true };
        if let Some(layer) = &d.layer {
            layer.release();
        }
        let event = DragAndDropEvent { position_in_root: d.pointer, ..self.in_app_event(&d, KeyModifiers::None) };
        *self.handed_off.borrow_mut() = Some(HandedOff { data: d.data });
        if let Some(root) = self.root() {
            exited(&root, &event);
            ended(&root, &event);
        }
        self.interested.borrow_mut().clear();
        true
    }

    /// A key changed the modifiers during the drag within the app: the targets told its action may have changed
    /// (Compose's `onChanged`).
    pub(crate) fn modifiers_changed_in_app(&self, modifiers: KeyModifiers) {
        let event = {
            let in_app = self.in_app.borrow();
            let Some(drag) = in_app.as_ref().filter(|d| !d.pending) else { return };
            self.in_app_event(drag, modifiers)
        };
        if let Some(root) = self.root() {
            changed(&root, &event);
        }
    }

    /// The drag within the app ends: dropped at `position` (`drop`), or called off. The target under it takes it
    /// (a drop), every target is told it ended (whether it was taken), the source what it was taken with.
    pub(crate) fn end_in_app(&self, drop_at: Option<(Offset, KeyModifiers)>) {
        let Some(mut drag) = self.in_app.borrow_mut().take() else { return };
        if let Some(layer) = drag.layer.take() {
            layer.release();
        }
        if drag.pending {
            // (it never took the pointer: nothing was told it started)
            complete(&drag.data, None);
            return;
        }
        let modifiers = drop_at.map_or(KeyModifiers::None, |(_, m)| m);
        if let Some((position, _)) = drop_at {
            drag.pointer = position;
        }
        let mut event = self.in_app_event(&drag, modifiers);
        let interested = std::mem::take(&mut *self.interested.borrow_mut());
        let Some(root) = self.root() else {
            complete(&drag.data, None);
            return;
        };
        let taken = match drop_at {
            Some(_) => {
                moved(&root, &event, &interested);
                dropped(&root, &event)
            }
            None => {
                exited(&root, &event);
                false
            }
        };
        event.result = taken;
        ended(&root, &event);
        complete(&drag.data, if taken { event.action } else { None });
    }

    /// Draws the decoration of the drag within the app under its pointer, faded (C++ Kompoz's; Android's drag
    /// shadow), through a layer of its own recorded the first time.
    pub(crate) fn draw(&self, canvas: &mut dyn GraphicsContext, size: Size, density: Density) {
        let mut in_app = self.in_app.borrow_mut();
        let Some(drag) = in_app.as_mut().filter(|d| !d.pending && d.pointer.is_specified()) else { return };
        if drag.layer.is_none() {
            // (drawn once, as the drag starts: a platform's drag image is made once, Android's shadow drawn once)
            let layer = GraphicsLayer::new(Some(self.platform.clone()));
            let decoration_size =
                IntSize::new(drag.decoration_size.width.round() as i32, drag.decoration_size.height.round() as i32);
            let draw = drag.draw_drag_decoration.clone();
            layer.record_flat(density, LayoutDirection::Ltr, decoration_size, |scope| draw(scope));
            drag.layer = Some(layer);
        }
        let layer = drag.layer.as_ref().expect("its layer");
        let top_left = drag.pointer - drag.decoration_offset;
        layer.set_top_left(kompoz_graphics::IntOffset::new(top_left.x.round() as i32, top_left.y.round() as i32));
        layer.set_alpha(DecorationAlpha);
        let layer = layer.clone();
        drop(in_app);
        let mut scope = DrawScope::at(canvas, Offset::Zero, size, LayoutDirection::Ltr, density);
        scope.draw_layer(&layer);
    }

    // ---- Drags from outside (and the app's own, back from the platform) ----

    /// A drag from outside came over the window carrying one of `mime_types`: the targets that want it told it
    /// started (a drag of the app's own the platform has, coming back: with what it carries). Whether any wants it.
    pub(crate) fn external_entered(&self, mime_types: Vec<String>) -> bool {
        self.external_end(None);
        let Some(root) = self.root() else { return false };
        let transfer_data = self.handed_off.borrow().as_ref().map(|h| h.data.clone());
        let event = DragAndDropEvent { mime_types: mime_types.clone(), transfer_data, ..DragAndDropEvent::default() };
        let mut interested = Vec::new();
        let accepted = accept(&root, &event, &mut interested);
        for cx in interested.iter().filter_map(NodeHandle::cx) {
            let (this, _) = parts(&cx);
            if let Some(t) = this {
                t.on_started(&event);
            }
        }
        *self.interested.borrow_mut() = interested;
        *self.external.borrow_mut() = Some(mime_types);
        accepted
    }

    /// It moved over the window to `position`: the targets it enters, moves over and leaves told. Whether it is over
    /// a target that wants it now (what a platform whose source asks as the drag moves tells it: OLE's effect).
    pub(crate) fn external_moved(&self, position: Offset) -> bool {
        let Some(mime_types) = self.external.borrow().clone() else { return false };
        let Some(root) = self.root() else { return false };
        let transfer_data = self.handed_off.borrow().as_ref().map(|h| h.data.clone());
        let event =
            DragAndDropEvent { mime_types, position_in_root: position, transfer_data, ..DragAndDropEvent::default() };
        let interested = self.interested.borrow().clone();
        moved(&root, &event, &interested);
        // (the root's own target, or the interested node it is in now: `moved` keeps the one it entered)
        is_interested(&interested, &root) || parts(&root).1.is_some()
    }

    /// It was dropped at `position` with `data` (the app's own drag back from the platform: with what it carries):
    /// the target under it takes it, every target is told it ended. Whether it was taken.
    pub(crate) fn external_drop(&self, data: DragAndDropTransferData, position: Offset) -> bool {
        if self.external.borrow().is_none() {
            self.external_entered(data.mime_types());
        }
        self.external_moved(position);
        let Some(root) = self.root() else { return false };
        let data = self.handed_off.borrow().as_ref().map_or(data, |h| h.data.clone());
        let mut event = DragAndDropEvent {
            mime_types: data.mime_types(),
            position_in_root: position,
            transfer_data: Some(data),
            ..DragAndDropEvent::default()
        };
        let taken = dropped(&root, &event);
        event.result = taken;
        self.external_end(Some(event));
        taken
    }

    /// It left the window, or ended without a drop: the target it was over told it left, every one that it ended.
    pub(crate) fn external_left(&self) {
        let Some(mime_types) = self.external.borrow().clone() else { return };
        let transfer_data = self.handed_off.borrow().as_ref().map(|h| h.data.clone());
        let event = DragAndDropEvent { mime_types, transfer_data, ..DragAndDropEvent::default() };
        if let Some(root) = self.root() {
            exited(&root, &event);
        }
        self.external_end(Some(event));
    }

    /// The drag from outside under way (if any) ended: every target that took part told.
    fn external_end(&self, event: Option<DragAndDropEvent>) {
        let Some(mime_types) = self.external.borrow_mut().take() else { return };
        let event = event.unwrap_or(DragAndDropEvent { mime_types, ..DragAndDropEvent::default() });
        if let Some(root) = self.root() {
            ended(&root, &event);
        }
        self.interested.borrow_mut().clear();
    }

    /// The platform's drag of the app's own ended (dropped with `action`, or not taken: none): its source told
    /// (the desktop's `onTransferCompleted`).
    pub(crate) fn platform_drag_ended(&self, action: Option<DragAndDropTransferAction>) {
        let Some(h) = self.handed_off.borrow_mut().take() else { return };
        complete(&h.data, action);
    }
}

/// The source told how its drag ended (the desktop's `onTransferCompleted`).
fn complete(data: &DragAndDropTransferData, action: Option<DragAndDropTransferAction>) {
    if let Some(f) = &data.on_transfer_completed {
        f(action);
    }
}

/// The action a drop within the app takes (AWT's user action): Move, Ctrl Copy, Ctrl+Shift Link — if the source
/// allows it, else the first it allows; none if it allows none.
fn drop_action(modifiers: KeyModifiers, supported: &[DragAndDropTransferAction]) -> Option<DragAndDropTransferAction> {
    let wanted = if modifiers.contains(KeyModifiers::Ctrl) && modifiers.contains(KeyModifiers::Shift) {
        DragAndDropTransferAction::Link
    } else if modifiers.contains(KeyModifiers::Ctrl) {
        DragAndDropTransferAction::Copy
    } else {
        DragAndDropTransferAction::Move
    };
    if supported.contains(&wanted) { Some(wanted) } else { supported.first().copied() }
}

/// The scope a source starts its drag in (Compose's desktop StartTransferScope of the drag and drop manager).
struct StartTransferScope {
    manager: Weak<DragAndDropManager>,
    started: Rc<Cell<bool>>,
    start_position: Option<Offset>,
}

impl DragAndDropStartTransferScope for StartTransferScope {
    fn start_drag_and_drop_transfer(
        &self,
        transfer_data: DragAndDropTransferData,
        decoration_size: Size,
        draw_drag_decoration: DrawDragDecoration,
    ) -> bool {
        let Some(manager) = self.manager.upgrade() else { return false };
        let started = manager.start_in_app(transfer_data, decoration_size, draw_drag_decoration, self.start_position);
        self.started.set(started);
        started
    }
}

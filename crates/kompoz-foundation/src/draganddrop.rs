//! Drag and drop modifiers, as Compose's foundation DragAndDropSource.kt and DragAndDropTarget.kt:
//! `Modifier.drag_and_drop_source` makes the layout a source of drags — a mouse's drag past the touch slop or a
//! finger's long press starts one carrying what `transfer_data` gives (none: no drag), shown under the pointer by
//! its decoration (by default a copy of how the layout looks: Compose's drag shadow, recorded into a graphics layer
//! as it draws); `Modifier.drag_and_drop_target` makes it take the drags `should_start_drag_and_drop` wants, told
//! to `target`. Both are kompoz-ui's [`DragAndDropNode`]s, delegated to (the host runs drags within the app and
//! takes those from outside: [`kompoz_ui::draganddrop`]).

use crate::gestures::{AwaitPointerEventScopeGestures, await_pointer_slop_or_cancellation};
use kompoz_graphics::{IntSize, Offset, Size};
use kompoz_runtime::named;
use kompoz_ui::draganddrop::{
    DragAndDropEvent, DragAndDropNode, DragAndDropSourceModifierNode, DragAndDropStartTransferScope, DragAndDropTarget,
    DragAndDropTargetModifierNode, DragAndDropTransferData, DrawDragDecoration,
    is_request_drag_and_drop_transfer_required, request_drag_and_drop_transfer,
};
use kompoz_ui::draw::{CacheDrawModifierNode, CacheDrawScope, DrawResult, DrawScope, GraphicsLayer};
use kompoz_ui::input::{
    AwaitPointerEventScope, PointerButton, PointerEvent, PointerEventPass, PointerEventType, PointerInputScope,
    PointerType, SuspendingPointerInputModifierNode,
};
use kompoz_ui::layout::LayoutAwareModifierNode;
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What gives a drag's transfer data, given where it started (local to the source): none, no drag (Compose's
/// `transferData: (Offset) -> DragAndDropTransferData?`).
pub type TransferData = Rc<dyn Fn(Offset) -> Option<DragAndDropTransferData>>;

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
        self.then_element(DropTargetElement { should_start: Rc::new(should_start_drag_and_drop), target })
    }

    /// The layout is a source of drags (Compose's `Modifier.dragAndDropSource(transferData)`): a mouse's primary
    /// drag past the touch slop, or a finger's long press, starts one carrying what `transfer_data` gives for
    /// where it started (none: no drag), shown under the pointer as the layout looks (its drag shadow).
    fn drag_and_drop_source(
        self,
        transfer_data: impl Fn(Offset) -> Option<DragAndDropTransferData> + 'static,
    ) -> Modifier {
        self.then_element(DragAndDropSourceWithDefaultShadowElement { transfer_data: Rc::new(transfer_data) })
    }

    /// The same, shown under the pointer by `draw_drag_decoration` in the layout's size (Compose's
    /// `Modifier.dragAndDropSource(drawDragDecoration, transferData)`).
    /// Kotlin: `dragAndDropSource(drawDragDecoration: DrawScope.() -> Unit, transferData: (Offset) -> DragAndDropTransferData?)`
    fn drag_and_drop_source_with_decoration(
        self,
        draw_drag_decoration: impl Fn(&mut DrawScope<'_>) + 'static,
        transfer_data: impl Fn(Offset) -> Option<DragAndDropTransferData> + 'static,
    ) -> Modifier {
        self.then_element(DragAndDropSourceElement {
            draw_drag_decoration: Rc::new(draw_drag_decoration),
            transfer_data: Rc::new(transfer_data),
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The target

struct DropTargetElement {
    should_start: Rc<dyn Fn(&DragAndDropEvent) -> bool>,
    target: Rc<dyn DragAndDropTarget>,
}

impl PartialEq for DropTargetElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.should_start, &other.should_start) && Rc::ptr_eq(&self.target, &other.target)
    }
}

impl ModifierNodeElement for DropTargetElement {
    type Node = DragAndDropTargetNode;
    fn create(&self) -> DragAndDropTargetNode {
        DragAndDropTargetNode {
            should_start: Rc::new(RefCell::new(self.should_start.clone())),
            target: self.target.clone(),
            drag_and_drop_node: None,
            handle: None,
        }
    }
    fn update(&self, node: &mut DragAndDropTargetNode) {
        node.update(self.should_start.clone(), self.target.clone());
    }
    fn name(&self) -> &'static str {
        "dropTarget"
    }
}

/// `drag_and_drop_target`'s node (Compose's foundation DragAndDropTargetNode): it delegates to a kompoz-ui target
/// node of its target (another target, another node), asking its latest `should_start`.
pub struct DragAndDropTargetNode {
    should_start: Rc<RefCell<Rc<dyn Fn(&DragAndDropEvent) -> bool>>>,
    target: Rc<dyn DragAndDropTarget>,
    drag_and_drop_node: Option<DelegatedNode<DragAndDropNode>>,
    handle: Option<NodeHandle>,
}

impl DragAndDropTargetNode {
    fn update(&mut self, should_start: Rc<dyn Fn(&DragAndDropEvent) -> bool>, target: Rc<dyn DragAndDropTarget>) {
        *self.should_start.borrow_mut() = should_start;
        if !Rc::ptr_eq(&self.target, &target) {
            self.target = target;
            if let Some(cx) = self.handle.as_ref().and_then(|h| h.cx()) {
                if let Some(old) = self.drag_and_drop_node.take() {
                    cx.undelegate(&old);
                }
                self.create_and_attach_drag_and_drop_modifier_node(&cx);
            }
        }
    }

    /// Compose's `createAndAttachDragAndDropModifierNode`: its latest `should_start` asked each time (updating it
    /// isn't destructive).
    fn create_and_attach_drag_and_drop_modifier_node(&mut self, cx: &NodeCx) {
        let should_start = self.should_start.clone();
        let node = DragAndDropTargetModifierNode(
            move |event: &DragAndDropEvent| {
                let should_start = should_start.borrow().clone();
                should_start(event)
            },
            self.target.clone(),
        );
        self.drag_and_drop_node = Some(cx.delegate(node));
    }
}

impl ModifierNode for DragAndDropTargetNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.create_and_attach_drag_and_drop_modifier_node(cx);
    }

    fn on_detach(&mut self, cx: &NodeCx) {
        if let Some(node) = self.drag_and_drop_node.take() {
            cx.undelegate(&node);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The source

/// What a source node and the drag start it gives share: the latest decoration and transfer data, its size.
struct SourceState {
    draw_drag_decoration: RefCell<DrawDragDecoration>,
    transfer_data: RefCell<TransferData>,
    size: Cell<IntSize>,
}

struct DragAndDropSourceElement {
    draw_drag_decoration: DrawDragDecoration,
    transfer_data: TransferData,
}

impl PartialEq for DragAndDropSourceElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.draw_drag_decoration, &other.draw_drag_decoration)
            && Rc::ptr_eq(&self.transfer_data, &other.transfer_data)
    }
}

impl ModifierNodeElement for DragAndDropSourceElement {
    type Node = DragAndDropSourceNode;
    fn create(&self) -> DragAndDropSourceNode {
        DragAndDropSourceNode::new(self.draw_drag_decoration.clone(), self.transfer_data.clone())
    }
    fn update(&self, node: &mut DragAndDropSourceNode) {
        *node.state.draw_drag_decoration.borrow_mut() = self.draw_drag_decoration.clone();
        *node.state.transfer_data.borrow_mut() = self.transfer_data.clone();
    }
    fn name(&self) -> &'static str {
        "dragSource"
    }
}

/// A drag and drop source's node (Compose's DragAndDropSourceNode): a kompoz-ui source node starting the drag with
/// the transfer data and decoration, and (the platforms have the app start its drags) the gesture starting it.
pub struct DragAndDropSourceNode {
    state: Rc<SourceState>,
    drag_and_drop_node: Option<DelegatedNode<DragAndDropNode>>,
    input_node: Option<DelegatedNode<SuspendingPointerInputModifierNode>>,
}

impl DragAndDropSourceNode {
    fn new(draw_drag_decoration: DrawDragDecoration, transfer_data: TransferData) -> DragAndDropSourceNode {
        DragAndDropSourceNode {
            state: Rc::new(SourceState {
                draw_drag_decoration: RefCell::new(draw_drag_decoration),
                transfer_data: RefCell::new(transfer_data),
                size: Cell::new(IntSize::Zero),
            }),
            drag_and_drop_node: None,
            input_node: None,
        }
    }
}

impl ModifierNode for DragAndDropSourceNode {
    node_capabilities!(layout_aware);

    fn on_attach(&mut self, cx: &NodeCx) {
        // (Compose delegates to its source node as it is made; a node has its context once attached)
        if self.drag_and_drop_node.is_none() {
            let state = self.state.clone();
            let node = DragAndDropSourceModifierNode(move |scope: &dyn DragAndDropStartTransferScope, offset| {
                let transfer_data = state.transfer_data.borrow().clone();
                if let Some(data) = transfer_data(offset) {
                    let size = state.size.get();
                    let draw = state.draw_drag_decoration.borrow().clone();
                    scope.start_drag_and_drop_transfer(data, size.to_size(), draw);
                }
            });
            self.drag_and_drop_node = Some(cx.delegate(node));
        }
        let source = self.drag_and_drop_node.as_ref().expect("its source node").cx();
        if is_request_drag_and_drop_transfer_required(&source) {
            let source = source.handle();
            let request: Rc<dyn Fn(Offset)> = Rc::new(move |offset| {
                if let Some(cx) = source.cx() {
                    request_drag_and_drop_transfer(&cx, offset);
                }
            });
            self.input_node = Some(cx.delegate(SuspendingPointerInputModifierNode::new(move |scope| {
                default_start_detector(scope, request.clone())
            })));
        }
    }

    fn on_detach(&mut self, cx: &NodeCx) {
        if let Some(node) = self.input_node.take() {
            cx.undelegate(&node);
        }
    }
}

impl LayoutAwareModifierNode for DragAndDropSourceNode {
    /// Its size, told to its source node too (Compose's: a node that is layout aware itself is told for its
    /// delegates).
    fn on_remeasured(&mut self, size: IntSize) {
        self.state.size.set(size);
        if let Some(node) = &self.drag_and_drop_node {
            node.borrow_mut().on_remeasured(size);
        }
    }

    fn on_placed(&mut self, coordinates: &kompoz_ui::layout::LayoutCoordinates) {
        if let Some(node) = &self.drag_and_drop_node {
            node.borrow_mut().on_placed(coordinates);
        }
    }
}

/// How a source's drag starts (Compose for desktop's `DragAndDropSourceDefaults.DefaultStartDetector`): a mouse's
/// primary drag past the touch slop (`detectDragGestures(matcher = PointerMatcher.mouse(Primary))`), a finger's
/// long press (`detectTapGestures(matcher = PointerMatcher.touch, onLongPress)`), each its own detector, at where it
/// went down. (Android's does the same in one loop: a mouse's drag, any other pointer's long press.)
async fn default_start_detector(scope: PointerInputScope, request_drag_and_drop_transfer: Rc<dyn Fn(Offset)>) {
    let request = request_drag_and_drop_transfer;
    kompoz_coroutines::coroutine_scope(move |cs| async move {
        let (s, r) = (scope.clone(), request.clone());
        cs.launch(async move { detect_mouse_drag_start(s, r).await });
        detect_touch_long_press(scope, request).await;
    })
    .await
}

/// A mouse's press of its primary button (the desktop's `PointerMatcher.mouse(PointerButton.Primary)`).
fn is_primary_mouse(event: &PointerEvent) -> bool {
    event.button == Some(PointerButton::Primary) && event.changes.iter().all(|c| c.r#type == PointerType::Mouse)
}

/// A finger's (the desktop's `PointerMatcher.touch`: no button, all touches).
fn is_touch(event: &PointerEvent) -> bool {
    event.button.is_none() && event.changes.iter().all(|c| c.r#type == PointerType::Touch)
}

/// Compose desktop's `isAllPressedDown`.
fn is_all_pressed_down(event: &PointerEvent, require_unconsumed: bool) -> bool {
    (event.r#type == PointerEventType::Press
        && event.changes.iter().all(|c| c.r#type == PointerType::Mouse && (!require_unconsumed || !c.is_consumed())))
        || event
            .changes
            .iter()
            .all(|c| if require_unconsumed { c.changed_to_down() } else { c.changed_to_down_ignore_consumed() })
}

/// Compose desktop's `isAllPressedUp`.
fn is_all_pressed_up(event: &PointerEvent, require_unconsumed: bool) -> bool {
    (event.r#type == PointerEventType::Release
        && event.changes.iter().all(|c| c.r#type == PointerType::Mouse && (!require_unconsumed || !c.is_consumed())))
        || event
            .changes
            .iter()
            .all(|c| if require_unconsumed { c.changed_to_up() } else { c.changed_to_up_ignore_consumed() })
}

/// Compose desktop's `awaitPress`: the next press `filter` takes.
async fn await_press(
    s: &AwaitPointerEventScope,
    filter: fn(&PointerEvent) -> bool,
    require_unconsumed: bool,
) -> PointerEvent {
    loop {
        let event = s.await_pointer_event().await;
        if is_all_pressed_down(&event, require_unconsumed) && filter(&event) {
            return event;
        }
    }
}

/// Compose desktop's `awaitReleaseOrCancelled`: the release `filter` takes; none if the pointer went out of
/// bounds or a change was consumed (in the final pass).
async fn await_release_or_cancelled(
    s: &AwaitPointerEventScope,
    consume_until_release: bool,
    filter: fn(&PointerEvent) -> bool,
) -> Option<PointerEvent> {
    loop {
        let event = s.await_pointer_event().await;
        if event.changes.iter().any(|c| c.is_out_of_bounds(s.size(), Size::Zero)) {
            return None;
        }
        let released = is_all_pressed_up(&event, true) && filter(&event);
        if consume_until_release {
            s.current_event().changes.iter().for_each(|c| c.consume());
        }
        // (cancelled by a consumption: the final pass after the main one looked at)
        let consume_check = s.await_pointer_event_with(PointerEventPass::Final).await;
        if consume_check.changes.iter().any(|c| c.is_consumed()) {
            return None;
        }
        if released {
            return Some(event);
        }
    }
}

/// The mouse's part (Compose desktop's `detectDragGestures(matcher)` with only `onDragStart`): a primary press
/// dragged past the touch slop starts the drag at the press.
async fn detect_mouse_drag_start(scope: PointerInputScope, request: Rc<dyn Fn(Offset)>) {
    loop {
        let request = request.clone();
        scope
            .await_pointer_event_scope(move |s| async move {
                let press = await_press(&s, is_primary_mouse, false).await;
                let down = press.changes[0].clone();
                // (Compose desktop's awaitDragStartOnSlop)
                let mut drag;
                loop {
                    drag = await_pointer_slop_or_cancellation(&s, down.id, down.r#type, None, Offset::Zero, |c, _| {
                        c.consume()
                    })
                    .await;
                    if drag.as_ref().is_none_or(|d| d.is_consumed()) {
                        break;
                    }
                }
                if drag.is_some() {
                    request(down.position);
                    s.drag(down.id, |c| c.consume()).await;
                }
            })
            .await;
    }
}

/// The finger's part (Compose desktop's `detectTapGestures(matcher = touch, onLongPress)`): a press nobody consumed
/// held for the long press timeout starts the drag at it; everything consumed until it lifts.
async fn detect_touch_long_press(scope: PointerInputScope, request: Rc<dyn Fn(Offset)>) {
    loop {
        let request = request.clone();
        scope
            .await_pointer_event_scope(move |s| async move {
                let down = await_press(&s, is_touch, true).await;
                down.changes[0].consume();
                let position = down.changes[0].position;
                let timeout = s.view_configuration().long_press_timeout_millis();
                let cancelled = Rc::new(Cell::new(false));
                let c = cancelled.clone();
                let first_release = s
                    .with_timeout_or_null(timeout, |s| async move {
                        let release = await_release_or_cancelled(&s, false, is_touch).await;
                        match &release {
                            Some(r) => r.changes.iter().for_each(|c| c.consume()),
                            None => c.set(true),
                        }
                        release
                    })
                    .await
                    .flatten();
                if cancelled.get() || first_release.is_some() {
                    return;
                }
                request(position);
                await_release_or_cancelled(&s, true, is_touch).await;
            })
            .await;
    }
}

// ---- The default shadow ----

struct DragAndDropSourceWithDefaultShadowElement {
    transfer_data: TransferData,
}

impl PartialEq for DragAndDropSourceWithDefaultShadowElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.transfer_data, &other.transfer_data)
    }
}

impl ModifierNodeElement for DragAndDropSourceWithDefaultShadowElement {
    type Node = DragSourceNodeWithDefaultPainter;
    fn create(&self) -> DragSourceNodeWithDefaultPainter {
        DragSourceNodeWithDefaultPainter::new(self.transfer_data.clone())
    }
    fn update(&self, node: &mut DragSourceNodeWithDefaultPainter) {
        if let Some(source) = &node.source {
            *source.borrow().state.transfer_data.borrow_mut() = self.transfer_data.clone();
        }
        node.transfer_data = self.transfer_data.clone();
    }
    fn name(&self) -> &'static str {
        "dragSourceWithDefaultPainter"
    }
}

/// What draws a source's default drag shadow (Compose's CacheDrawScopeDragShadowCallback, Android's): the content
/// recorded into a graphics layer as the layout draws, drawn again as the decoration.
#[derive(Default)]
struct CacheDrawScopeDragShadowCallback {
    graphics_layer: RefCell<Option<GraphicsLayer>>,
}

impl CacheDrawScopeDragShadowCallback {
    /// Compose's `drawDragShadow`: the recorded layer (it panics, as Compose throws, before the layout drew).
    fn draw_drag_shadow(&self, draw_scope: &mut DrawScope<'_>) {
        let layer = self.graphics_layer.borrow().clone();
        match layer {
            None => kompoz_runtime::kt::panic_kt(
                "IllegalArgumentException",
                "No cached drag shadow. Check if the drag source node was rendered first",
            ),
            Some(layer) => draw_scope.draw_layer(&layer),
        }
    }

    /// Compose's `cachePicture`: a layer of the content, drawn in its place.
    fn cache_picture(&self, scope: &mut CacheDrawScope) -> DrawResult {
        let layer = scope.obtain_graphics_layer();
        scope.record(&layer, |c| c.draw_content());
        *self.graphics_layer.borrow_mut() = Some(layer.clone());
        scope.on_draw_with_content(move |c| c.draw_layer(&layer))
    }
}

/// `drag_and_drop_source`'s node with the default shadow (Compose's DragSourceNodeWithDefaultPainter): it records
/// how the layout looks (a cache draw node) and is a source drawing that as its decoration.
pub struct DragSourceNodeWithDefaultPainter {
    transfer_data: TransferData,
    callback: Rc<CacheDrawScopeDragShadowCallback>,
    source: Option<DelegatedNode<DragAndDropSourceNode>>,
}

impl DragSourceNodeWithDefaultPainter {
    fn new(transfer_data: TransferData) -> DragSourceNodeWithDefaultPainter {
        DragSourceNodeWithDefaultPainter {
            transfer_data,
            callback: Rc::new(CacheDrawScopeDragShadowCallback::default()),
            source: None,
        }
    }
}

impl ModifierNode for DragSourceNodeWithDefaultPainter {
    fn on_attach(&mut self, cx: &NodeCx) {
        // (Compose delegates to both as it is made; a node has its context once attached)
        if self.source.is_none() {
            let callback = self.callback.clone();
            cx.delegate(CacheDrawModifierNode(move |scope: &mut CacheDrawScope| callback.cache_picture(scope)));
            let callback = self.callback.clone();
            let draw: DrawDragDecoration = Rc::new(move |scope: &mut DrawScope<'_>| callback.draw_drag_shadow(scope));
            self.source = Some(cx.delegate(DragAndDropSourceNode::new(draw, self.transfer_data.clone())));
        }
    }
}

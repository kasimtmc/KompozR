//! Dragging along an axis, as Compose's Draggable.kt: a [`DraggableState`] takes drag deltas inside a mutation
//! ([`DraggableState::drag_block`]); `Modifier.draggable` gives it the user's drags; [`DragGestureNode`] is the
//! drag detection both `draggable` and `scrollable` are made of (Compose's DragGestureNode: a state machine on the
//! node's pointer events — down, past the touch slop or picked up from a gesture another gave up, dragging — sending
//! the drag's events to a coroutine that opens the state's mutation for them).

// (Compose's parameter lists: the methods of a `#[named]` trait carry no attributes of their own)
#![allow(clippy::too_many_arguments)]

use super::drag::{TouchSlopDetector, pointer_slop};
use super::indirect_drag::IndirectPointerInputDragCycleDetector;
use super::scrollable_state::LocalFuture;
use super::{FIRST_DOWN_REFERS_TO_PRIMARY_MOUSE_BUTTON_ONLY, Orientation, is_changed_to_down};
use crate::gesture_node::{
    DraggableGestureConnection, GestureConnection, GestureNode, GestureState, traverse_parent_draggable_gestures,
};
use crate::interaction::{DragInteraction, MutableInteractionSource};
use kompoz_animation::{MutatePriority, MutatorMutex};
use kompoz_coroutines::{Channel, CoroutineScope, catch_cancellation};
use kompoz_graphics::{IntSize, Offset};
use kompoz_runtime::{kompozable, named, named_args, remember, remember_updated_state};
use kompoz_ui::input::{
    IndirectPointerEvent, IndirectPointerInputModifierNode, PointerEvent, PointerEventPass, PointerId,
    PointerInputChange, PointerInputModifierNode, PointerType, VelocityTracker,
};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::platform::LocalViewConfiguration;
use kompoz_ui::unit::Velocity;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::{Rc, Weak};

// ---------------------------------------------------------------------------------------------------------------
// The state

/// What a drag mutation drags with (Compose's DragScope).
pub trait DragScope {
    /// Drags by `pixels` (Compose's `dragBy`).
    fn drag_by(&self, pixels: f32);
}

/// A block dragging in a [`DragScope`] (Compose's `suspend DragScope.() -> Unit`).
pub type DragBlock = Box<dyn FnOnce(Rc<dyn DragScope>) -> LocalFuture<'static, ()>>;

/// A state that is dragged (Compose's DraggableState): `drag_block` runs a block taking deltas in its
/// [`DragScope`], one block at a time (a newer one of at least its priority cancels an older); `dispatch_raw_delta`
/// drags outside of that. A cheap handle, equal to its clones (what `draggable` compares it by).
pub trait DraggableState: Any + DraggableStateEq {
    /// Runs `block` as the mutation of `drag_priority` (Compose's `drag`). Through
    /// [`DraggableStateExtensions::drag`] with a closure.
    fn drag_block(&self, drag_priority: MutatePriority, block: DragBlock) -> LocalFuture<'_, ()>;

    /// Drags by `delta` at once, outside of any mutation (Compose's `dispatchRawDelta`).
    fn dispatch_raw_delta(&self, delta: f32);
}

/// How draggable states compare (a blanket impl for every `PartialEq` one).
#[doc(hidden)]
pub trait DraggableStateEq {
    fn draggable_state_eq(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: PartialEq + Any> DraggableStateEq for T {
    fn draggable_state_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PartialEq for dyn DraggableState {
    fn eq(&self, other: &dyn DraggableState) -> bool {
        self.draggable_state_eq(other.as_any())
    }
}

impl fmt::Debug for dyn DraggableState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DraggableState")
    }
}

/// A state shared as any kind of draggable state: its state's calls.
impl DraggableState for Rc<dyn DraggableState> {
    fn drag_block(&self, drag_priority: MutatePriority, block: DragBlock) -> LocalFuture<'_, ()> {
        (**self).drag_block(drag_priority, block)
    }
    fn dispatch_raw_delta(&self, delta: f32) {
        (**self).dispatch_raw_delta(delta)
    }
}

/// The calls of any [`DraggableState`] (Compose's `drag` with a lambda).
#[named(trait DraggableStateExtensions)]
impl<S: DraggableState> S {
    /// Runs `block` as the mutation of `drag_priority` (Compose's `drag { … }`).
    fn drag<F, Fut>(
        &self,
        #[default(MutatePriority::Default)] drag_priority: MutatePriority,
        block: F,
    ) -> impl Future<Output = ()>
    where
        F: FnOnce(Rc<dyn DragScope>) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        self.drag_block(drag_priority, Box::new(move |scope| Box::pin(block(scope))))
    }
}

/// A draggable state of a closure taking the deltas (Compose's DefaultDraggableState, made by
/// [`DraggableState()`]).
#[derive(Clone)]
pub struct DefaultDraggableState {
    inner: Rc<DefaultDraggableInner>,
}

struct DefaultDraggableInner {
    on_delta: Rc<dyn Fn(f32)>,
    scroll_mutex: MutatorMutex,
}

impl PartialEq for DefaultDraggableState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for DefaultDraggableState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DefaultDraggableState({:p})", Rc::as_ptr(&self.inner))
    }
}

struct DefaultDragScope(Rc<dyn Fn(f32)>);

impl DragScope for DefaultDragScope {
    fn drag_by(&self, pixels: f32) {
        (self.0)(pixels)
    }
}

impl DraggableState for DefaultDraggableState {
    fn drag_block(&self, drag_priority: MutatePriority, block: DragBlock) -> LocalFuture<'_, ()> {
        let scope: Rc<dyn DragScope> = Rc::new(DefaultDragScope(self.inner.on_delta.clone()));
        Box::pin(named_args!(self.inner.scroll_mutex.mutate_with(scope, drag_priority, block)))
    }

    fn dispatch_raw_delta(&self, delta: f32) {
        (self.inner.on_delta)(delta)
    }
}

/// A draggable state of `on_delta`, given each drag delta px (Compose's `DraggableState(onDelta)`).
pub fn DraggableState(on_delta: impl Fn(f32) + 'static) -> DefaultDraggableState {
    DefaultDraggableState {
        inner: Rc::new(DefaultDraggableInner { on_delta: Rc::new(on_delta), scroll_mutex: MutatorMutex::new() }),
    }
}

/// A [`DraggableState()`] remembered where it is called, calling the latest `on_delta` (Compose's
/// `rememberDraggableState`).
#[kompozable]
pub fn remember_draggable_state(on_delta: impl Fn(f32) + 'static) -> DefaultDraggableState {
    let on_delta_state = remember_updated_state::<Rc<dyn Fn(f32)>>(Rc::new(on_delta));
    remember(move || DraggableState(move |delta| (on_delta_state.get())(delta)))
}

// ---------------------------------------------------------------------------------------------------------------
// Modifier.draggable

/// What a drag start calls, in a coroutine of the node's (Compose's `onDragStarted: suspend CoroutineScope.(
/// startedPosition: Offset) -> Unit`).
pub type OnDragStarted = Rc<dyn Fn(CoroutineScope, Offset) -> LocalFuture<'static, ()>>;

/// What a drag's end calls with the velocity it was let go with along its axis, px/s (Compose's `onDragStopped`).
pub type OnDragStopped = Rc<dyn Fn(CoroutineScope, f32) -> LocalFuture<'static, ()>>;

/// The draggable modifier.
#[named(trait DraggableModifiers)]
impl Modifier {
    /// Gives `state` the user's drags along `orientation` (Compose's `Modifier.draggable`): after the touch slop
    /// (at once with `start_drag_immediately`), each delta px, in the finger's direction (the other way with
    /// `reverse_direction`); `on_drag_started` and `on_drag_stopped` (its velocity) are called in coroutines of
    /// its own; the drags are emitted to `interaction_source` as [`DragInteraction`]s. A mouse drags it too. Not
    /// `enabled`: nothing.
    fn draggable(
        self,
        state: impl DraggableState,
        orientation: Orientation,
        #[default(true)] enabled: bool,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(false)] start_drag_immediately: bool,
        #[default(None)] on_drag_started: Option<OnDragStarted>,
        #[default(None)] on_drag_stopped: Option<OnDragStopped>,
        #[default(false)] reverse_direction: bool,
    ) -> Modifier {
        self.then_element(DraggableElement {
            state: Rc::new(state),
            orientation,
            enabled,
            interaction_source,
            start_drag_immediately,
            on_drag_started,
            on_drag_stopped,
            reverse_direction,
        })
    }
}

struct DraggableElement {
    state: Rc<dyn DraggableState>,
    orientation: Orientation,
    enabled: bool,
    interaction_source: Option<MutableInteractionSource>,
    start_drag_immediately: bool,
    on_drag_started: Option<OnDragStarted>,
    on_drag_stopped: Option<OnDragStopped>,
    reverse_direction: bool,
}

fn same_rc<T: ?Sized>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

impl PartialEq for DraggableElement {
    fn eq(&self, o: &Self) -> bool {
        *self.state == *o.state
            && self.orientation == o.orientation
            && self.enabled == o.enabled
            && self.interaction_source == o.interaction_source
            && self.start_drag_immediately == o.start_drag_immediately
            && same_rc(&self.on_drag_started, &o.on_drag_started)
            && same_rc(&self.on_drag_stopped, &o.on_drag_stopped)
            && self.reverse_direction == o.reverse_direction
    }
}

/// `draggable`'s: any pointer drags it (Compose's DraggableElement.CanDrag).
fn can_drag_any(_: PointerType) -> bool {
    true
}

impl ModifierNodeElement for DraggableElement {
    type Node = DraggableNode;
    fn create(&self) -> DraggableNode {
        DraggableNode::new(self)
    }
    fn update(&self, node: &mut DraggableNode) {
        node.core.update(self);
    }
    fn name(&self) -> &'static str {
        "draggable"
    }
}

/// `draggable`'s node (Compose's DraggableNode): a drag gesture node opening its state's mutation for the drags.
pub struct DraggableNode {
    core: Rc<DraggableCore>,
}

struct DraggableCore {
    drag: Rc<DragGestureCore>,
    state: RefCell<Rc<dyn DraggableState>>,
    start_drag_immediately: Cell<bool>,
    on_drag_started: RefCell<Option<OnDragStarted>>,
    on_drag_stopped: RefCell<Option<OnDragStopped>>,
    reverse_direction: Cell<bool>,
}

impl DraggableNode {
    fn new(e: &DraggableElement) -> DraggableNode {
        let drag =
            DragGestureCore::new(Rc::new(can_drag_any), e.enabled, e.interaction_source.clone(), Some(e.orientation));
        let core = Rc::new(DraggableCore {
            drag: drag.clone(),
            state: RefCell::new(e.state.clone()),
            start_drag_immediately: Cell::new(e.start_drag_immediately),
            on_drag_started: RefCell::new(e.on_drag_started.clone()),
            on_drag_stopped: RefCell::new(e.on_drag_stopped.clone()),
            reverse_direction: Cell::new(e.reverse_direction),
        });
        let owner: Rc<dyn DragGestureOwner> = core.clone();
        drag.set_owner(&owner);
        DraggableNode { core }
    }
}

impl DraggableCore {
    fn reverse_if_needed(&self, o: Offset) -> Offset {
        if self.reverse_direction.get() { o * -1.0 } else { o }
    }

    /// Compose's DraggableNode.update.
    fn update(&self, e: &DraggableElement) {
        let mut reset_pointer_input_handling = false;
        if **self.state.borrow() != *e.state {
            *self.state.borrow_mut() = e.state.clone();
            reset_pointer_input_handling = true;
        }
        if self.reverse_direction.get() != e.reverse_direction {
            self.reverse_direction.set(e.reverse_direction);
            reset_pointer_input_handling = true;
        }
        *self.on_drag_started.borrow_mut() = e.on_drag_started.clone();
        *self.on_drag_stopped.borrow_mut() = e.on_drag_stopped.clone();
        self.start_drag_immediately.set(e.start_drag_immediately);
        self.drag.update(
            Rc::new(can_drag_any),
            e.enabled,
            e.interaction_source.clone(),
            Some(e.orientation),
            reset_pointer_input_handling,
        );
    }
}

fn offset_along(o: Offset, orientation: Orientation) -> f32 {
    if orientation == Orientation::Vertical { o.y } else { o.x }
}

impl DragGestureOwner for DraggableCore {
    fn drag(self: Rc<Self>, for_each_delta: ForEachDelta) -> LocalFuture<'static, ()> {
        Box::pin(async move {
            let Some(orientation) = self.drag.orientation.get() else { return };
            let state = self.state.borrow().clone();
            let me = self.clone();
            state
                .drag_block(
                    MutatePriority::UserInput,
                    Box::new(move |scope| {
                        Box::pin(async move {
                            let process: Rc<dyn Fn(DragDelta)> = Rc::new(move |d: DragDelta| {
                                scope.drag_by(offset_along(me.reverse_if_needed(d.delta), orientation));
                            });
                            for_each_delta(process).await;
                        })
                    }),
                )
                .await;
        })
    }

    fn on_drag_started(&self, started_position: Offset) {
        let Some(on_drag_started) = self.on_drag_started.borrow().clone() else { return };
        let Some(scope) = self.drag.scope() else { return };
        let s = scope.clone();
        scope.launch(async move { on_drag_started(s, started_position).await });
    }

    fn on_drag_stopped(&self, event: DragStopped) {
        let Some(on_drag_stopped) = self.on_drag_stopped.borrow().clone() else { return };
        let Some(orientation) = self.drag.orientation.get() else { return };
        let Some(scope) = self.drag.scope() else { return };
        let v = if self.reverse_direction.get() { event.velocity * -1.0 } else { event.velocity };
        let velocity = if orientation == Orientation::Vertical { v.y } else { v.x };
        let s = scope.clone();
        scope.launch(async move { on_drag_stopped(s, velocity).await });
    }

    fn start_drag_immediately(&self) -> bool {
        self.start_drag_immediately.get()
    }
}

impl ModifierNode for DraggableNode {
    node_capabilities!(pointer_input, indirect_pointer_input);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.core.drag.on_attach(cx);
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.core.drag.on_detach();
    }
}

impl PointerInputModifierNode for DraggableNode {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        self.core.drag.on_pointer_event(event, pass, bounds);
    }
    fn on_cancel_pointer_input(&mut self) {
        self.core.drag.on_cancel_pointer_input();
    }
}

impl IndirectPointerInputModifierNode for DraggableNode {
    fn on_indirect_pointer_event(&mut self, event: &IndirectPointerEvent, pass: PointerEventPass) {
        self.core.drag.on_indirect_pointer_event(event, pass);
    }
    fn on_cancel_indirect_pointer_input(&mut self) {
        self.core.drag.on_cancel_indirect_pointer_input();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The drag gesture node

/// A drag's delta (Compose's DragEvent.DragDelta).
#[derive(Clone, Copy, Debug)]
pub(crate) struct DragDelta {
    pub(crate) delta: Offset,
    pub(crate) is_indirect_pointer_event: bool,
}

/// A drag's end (Compose's DragEvent.DragStopped).
#[derive(Clone, Copy, Debug)]
pub(crate) struct DragStopped {
    pub(crate) velocity: Velocity,
    pub(crate) is_indirect_pointer_event: bool,
}

/// What the drag detection sends its coroutine (Compose's DragEvent, its variants' names Compose's).
#[derive(Clone, Copy, Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum DragEvent {
    DragStarted { start_point: Offset },
    DragStopped(DragStopped),
    DragCancelled,
    DragDelta(DragDelta),
}

/// The drag's deltas, handed one by one to what the owner's mutation does with them, until the drag ends
/// (Compose's `forEachDelta: suspend ((DragDelta) -> Unit) -> Unit`).
pub(crate) type ForEachDelta = Box<dyn FnOnce(Rc<dyn Fn(DragDelta)>) -> LocalFuture<'static, ()>>;

/// What a drag gesture node does with its drags (Compose's DragGestureNode's abstract members): `draggable`'s and
/// `scrollable`'s.
pub(crate) trait DragGestureOwner {
    /// Opens a mutation and hands `for_each_delta` what it does with each delta (Compose's `drag`).
    fn drag(self: Rc<Self>, for_each_delta: ForEachDelta) -> LocalFuture<'static, ()>;
    fn on_drag_started(&self, started_position: Offset);
    fn on_drag_stopped(&self, event: DragStopped);
    fn start_drag_immediately(&self) -> bool;
}

/// Whether a drag waits for the touch slop (Compose's DragDetectionState.AwaitDown.AwaitTouchSlop).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AwaitTouchSlop {
    Yes,
    No,
    NotInitialized,
}

/// Where a drag's detection is (Compose's DragDetectionState).
#[derive(Clone, Debug)]
enum DragDetectionState {
    AwaitDown { await_touch_slop: AwaitTouchSlop, consumed_on_initial: bool, has_seen_initial_event: bool },
    AwaitTouchSlop { initial_down: PointerInputChange, pointer_id: PointerId, verify_consumption_in_final_pass: bool },
    AwaitGesturePickup { initial_down: PointerInputChange, pointer_id: PointerId },
    Dragging { pointer_id: PointerId },
}

impl DragDetectionState {
    fn await_down() -> DragDetectionState {
        DragDetectionState::AwaitDown {
            await_touch_slop: AwaitTouchSlop::NotInitialized,
            consumed_on_initial: false,
            has_seen_initial_event: false,
        }
    }
}

/// The drag detection of `draggable` and `scrollable` (Compose's DragGestureNode): kept by their nodes, which
/// forward it their pointer events and lifecycle.
pub(crate) struct DragGestureCore {
    me: Weak<DragGestureCore>,
    owner: RefCell<Option<Weak<dyn DragGestureOwner>>>,
    pub(crate) can_drag: RefCell<Rc<dyn Fn(PointerType) -> bool>>,
    pub(crate) enabled: Cell<bool>,
    pub(crate) interaction_source: RefCell<Option<MutableInteractionSource>>,
    pub(crate) orientation: Cell<Option<Orientation>>,
    channel: RefCell<Option<Channel<DragEvent>>>,
    drag_interaction: Cell<Option<DragInteraction::Start>>,
    pub(crate) is_listening_for_events: Cell<bool>,
    is_listening_for_pointer_input_events: Cell<bool>,
    drag_accumulator: Cell<Offset>,
    pointer_input_gesture_node: RefCell<Option<DelegatedNode<GestureNode>>>,
    indirect_touch_gesture_node: RefCell<Option<DelegatedNode<GestureNode>>>,
    current_drag_state: RefCell<Option<DragDetectionState>>,
    velocity_tracker: RefCell<Option<VelocityTracker>>,
    touch_slop_detector: RefCell<Option<TouchSlopDetector>>,
    indirect_detector: RefCell<Option<Rc<IndirectPointerInputDragCycleDetector>>>,
    handle: RefCell<Option<NodeHandle>>,
}

impl DragGestureCore {
    pub(crate) fn new(
        can_drag: Rc<dyn Fn(PointerType) -> bool>,
        enabled: bool,
        interaction_source: Option<MutableInteractionSource>,
        orientation: Option<Orientation>,
    ) -> Rc<DragGestureCore> {
        Rc::new_cyclic(|me| DragGestureCore {
            me: me.clone(),
            owner: RefCell::new(None),
            can_drag: RefCell::new(can_drag),
            enabled: Cell::new(enabled),
            interaction_source: RefCell::new(interaction_source),
            orientation: Cell::new(orientation),
            channel: RefCell::new(None),
            drag_interaction: Cell::new(None),
            is_listening_for_events: Cell::new(false),
            is_listening_for_pointer_input_events: Cell::new(false),
            drag_accumulator: Cell::new(Offset::Zero),
            pointer_input_gesture_node: RefCell::new(None),
            indirect_touch_gesture_node: RefCell::new(None),
            current_drag_state: RefCell::new(None),
            velocity_tracker: RefCell::new(None),
            touch_slop_detector: RefCell::new(None),
            indirect_detector: RefCell::new(None),
            handle: RefCell::new(None),
        })
    }

    pub(crate) fn set_owner(&self, owner: &Rc<dyn DragGestureOwner>) {
        *self.owner.borrow_mut() = Some(Rc::downgrade(owner));
    }

    fn rc(&self) -> Rc<DragGestureCore> {
        self.me.upgrade().expect("kompoz: a drag node's state outlived it")
    }

    fn owner(&self) -> Option<Rc<dyn DragGestureOwner>> {
        self.owner.borrow().as_ref().and_then(Weak::upgrade)
    }

    pub(crate) fn cx(&self) -> Option<NodeCx> {
        self.handle.borrow().as_ref().and_then(|h| h.cx())
    }

    /// The node's coroutine scope, while attached.
    pub(crate) fn scope(&self) -> Option<CoroutineScope> {
        self.handle.borrow().as_ref().and_then(|h| h.coroutine_scope())
    }

    pub(crate) fn on_attach(&self, cx: &NodeCx) {
        *self.handle.borrow_mut() = Some(cx.handle());
    }

    /// Compose's `onDetach`.
    pub(crate) fn on_detach(&self) {
        self.is_listening_for_events.set(false);
        self.dispose_interaction_source();
        self.reset_gesture_nodes();
    }

    fn require_channel(&self) -> Channel<DragEvent> {
        self.channel.borrow().clone().expect("Events channel not initialized.")
    }

    /// Compose's `startListeningForEvents`: the coroutine taking the drag's events into the owner's mutation.
    fn start_listening_for_events(&self) {
        self.is_listening_for_events.set(true);
        if self.channel.borrow().is_none() {
            *self.channel.borrow_mut() = Some(Channel::unlimited());
        }
        let Some(scope) = self.scope() else { return };
        let me = self.rc();
        let channel = self.require_channel();
        scope.launch(async move {
            loop {
                let event = channel.receive().await;
                let Some(DragEvent::DragStarted { start_point }) = event else { continue };
                me.process_drag_start(start_point).await;
                let current: Rc<Cell<Option<DragEvent>>> = Rc::new(Cell::new(event));
                let Some(owner) = me.owner() else { continue };
                // (Kotlin's catch of the drag's cancellation: its mutation taken by another; dropped with the
                // node's scope, the drag is cancelled as it goes)
                let mut cancel_guard = CancelOnDrop(Some(me.clone()));
                let (c, ch) = (current.clone(), channel.clone());
                let for_each_delta: ForEachDelta = Box::new(move |process_delta| {
                    Box::pin(async move {
                        loop {
                            match c.get() {
                                Some(DragEvent::DragStopped(_)) | Some(DragEvent::DragCancelled) => break,
                                Some(DragEvent::DragDelta(d)) => process_delta(d),
                                _ => {}
                            }
                            c.set(ch.receive().await);
                        }
                    })
                });
                let dragged = catch_cancellation(move |_| owner.drag(for_each_delta)).await;
                cancel_guard.0 = None;
                match (dragged, current.get()) {
                    (Ok(()), Some(DragEvent::DragStopped(stopped))) => me.process_drag_stop(stopped).await,
                    (Ok(()), Some(DragEvent::DragCancelled)) => me.process_drag_cancel().await,
                    (Ok(()), _) => {}
                    (Err(_), _) => me.process_drag_cancel().await,
                }
            }
        });
    }

    /// Compose's `processDragStart`.
    async fn process_drag_start(&self, start_point: Offset) {
        let source = self.interaction_source.borrow().clone();
        if let Some(old) = self.drag_interaction.get()
            && let Some(source) = &source
        {
            source.emit(DragInteraction::Cancel::new(old)).await;
        }
        let interaction = DragInteraction::Start::new();
        if let Some(source) = &source {
            source.emit(interaction).await;
        }
        self.drag_interaction.set(Some(interaction));
        if let Some(owner) = self.owner() {
            owner.on_drag_started(start_point);
        }
    }

    /// Compose's `processDragStop`.
    async fn process_drag_stop(&self, event: DragStopped) {
        if let Some(interaction) = self.drag_interaction.take() {
            let source = self.interaction_source.borrow().clone();
            if let Some(source) = source {
                source.emit(DragInteraction::Stop::new(interaction)).await;
            }
        }
        if let Some(owner) = self.owner() {
            owner.on_drag_stopped(event);
        }
    }

    /// Compose's `processDragCancel`.
    async fn process_drag_cancel(&self) {
        if let Some(interaction) = self.drag_interaction.take() {
            let source = self.interaction_source.borrow().clone();
            if let Some(source) = source {
                source.emit(DragInteraction::Cancel::new(interaction)).await;
            }
        }
        if let Some(owner) = self.owner() {
            owner.on_drag_stopped(DragStopped { velocity: Velocity::Zero, is_indirect_pointer_event: false });
        }
    }

    /// The same where it can't wait: the coroutine dropped where it waited (Kotlin's catch running in a cancelled
    /// coroutine, whose `emit` doesn't suspend).
    fn process_drag_cancel_now(&self) {
        if let Some(interaction) = self.drag_interaction.take() {
            let source = self.interaction_source.borrow().clone();
            if let Some(source) = source {
                source.try_emit(DragInteraction::Cancel::new(interaction));
            }
        }
        if let Some(owner) = self.owner() {
            owner.on_drag_stopped(DragStopped { velocity: Velocity::Zero, is_indirect_pointer_event: false });
        }
    }

    /// Ends what it emitted at once (Compose's `disposeInteractionSource`).
    pub(crate) fn dispose_interaction_source(&self) {
        if let Some(interaction) = self.drag_interaction.take() {
            let source = self.interaction_source.borrow().clone();
            if let Some(source) = source {
                source.try_emit(DragInteraction::Cancel::new(interaction));
            }
        }
    }

    /// Compose's `update`.
    pub(crate) fn update(
        &self,
        can_drag: Rc<dyn Fn(PointerType) -> bool>,
        enabled: bool,
        interaction_source: Option<MutableInteractionSource>,
        orientation: Option<Orientation>,
        should_reset_pointer_input_handling: bool,
    ) {
        let mut reset_pointer_input_handling = should_reset_pointer_input_handling;
        *self.can_drag.borrow_mut() = can_drag;
        if self.enabled.get() != enabled {
            self.enabled.set(enabled);
            if !enabled {
                self.reset_gesture_nodes();
                self.dispose_interaction_source();
                *self.indirect_detector.borrow_mut() = None;
            }
            reset_pointer_input_handling = true;
        }
        if *self.interaction_source.borrow() != interaction_source {
            self.dispose_interaction_source();
            *self.interaction_source.borrow_mut() = interaction_source;
        }
        if self.orientation.get() != orientation {
            self.orientation.set(orientation);
            reset_pointer_input_handling = true;
        }
        if reset_pointer_input_handling {
            if self.is_listening_for_pointer_input_events.get() {
                self.reset_drag_detection_state();
            }
            let detector = self.indirect_detector.borrow().clone();
            if let Some(d) = detector {
                d.reset_drag_detection_state();
            }
        }
    }

    /// Compose's `initializePointerInputGestureCoordination`.
    pub(crate) fn initialize_pointer_input_gesture_coordination(&self) {
        if self.pointer_input_gesture_node.borrow().is_some() {
            return;
        }
        let Some(cx) = self.cx() else { return };
        let connection: Rc<dyn GestureConnection> = self.rc();
        let node = cx.delegate(GestureNode::new(connection));
        *self.pointer_input_gesture_node.borrow_mut() = Some(node);
    }

    /// Compose's `initializeIndirectTouchGestureCoordination`.
    fn initialize_indirect_touch_gesture_coordination(&self) {
        if self.indirect_touch_gesture_node.borrow().is_some() {
            return;
        }
        let Some(detector) = self.indirect_detector.borrow().clone() else { return };
        let Some(cx) = self.cx() else { return };
        let connection: Rc<dyn GestureConnection> = detector;
        let node = cx.delegate(GestureNode::new(connection));
        *self.indirect_touch_gesture_node.borrow_mut() = Some(node);
    }

    /// Compose's `resetGestureNodes`.
    fn reset_gesture_nodes(&self) {
        let cx = self.cx();
        let indirect = self.indirect_touch_gesture_node.borrow_mut().take();
        let pointer = self.pointer_input_gesture_node.borrow_mut().take();
        if let Some(cx) = cx {
            if let Some(n) = indirect {
                cx.undelegate(&n);
            }
            if let Some(n) = pointer {
                cx.undelegate(&n);
            }
        }
    }

    /// Compose's `onPointerEvent`.
    pub(crate) fn on_pointer_event(&self, event: &PointerEvent, pass: PointerEventPass, _bounds: IntSize) {
        self.is_listening_for_pointer_input_events.set(true);
        if self.enabled.get() {
            self.initialize_pointer_input_gesture_coordination();
            if self.current_drag_state.borrow().is_none() {
                *self.current_drag_state.borrow_mut() = Some(DragDetectionState::await_down());
            }
            self.process_raw_pointer_event(event, pass);
        }
    }

    /// Compose's `onIndirectPointerEvent`.
    pub(crate) fn on_indirect_pointer_event(&self, event: &IndirectPointerEvent, pass: PointerEventPass) {
        if self.enabled.get() {
            if self.indirect_detector.borrow().is_none() {
                *self.indirect_detector.borrow_mut() =
                    Some(IndirectPointerInputDragCycleDetector::new(self.me.clone()));
            }
            self.initialize_indirect_touch_gesture_coordination();
            let detector = self.indirect_detector.borrow().clone();
            if let Some(d) = detector {
                d.process_indirect_pointer_input_event(event, pass);
            }
        }
    }

    /// Compose's `onCancelIndirectPointerInput`.
    pub(crate) fn on_cancel_indirect_pointer_input(&self) {
        let detector = self.indirect_detector.borrow().clone();
        if let Some(d) = detector {
            d.reset_drag_detection_state();
        }
    }

    /// Compose's `onCancelPointerInput`.
    pub(crate) fn on_cancel_pointer_input(&self) {
        if self.is_listening_for_pointer_input_events.get() {
            self.reset_drag_detection_state();
        }
        self.is_listening_for_pointer_input_events.set(false);
    }

    /// Compose's `processRawPointerEvent` (its velocity tracker fix off, as Compose's flag is by default).
    fn process_raw_pointer_event(&self, event: &PointerEvent, pass: PointerEventPass) {
        let state = self.current_drag_state.borrow().clone().expect("currentDragState should not be null");
        match state {
            DragDetectionState::AwaitDown { await_touch_slop, consumed_on_initial, has_seen_initial_event } => self
                .process_initial_down_state(event, pass, await_touch_slop, consumed_on_initial, has_seen_initial_event),
            DragDetectionState::AwaitTouchSlop { initial_down, pointer_id, verify_consumption_in_final_pass } => {
                self.process_await_touch_slop(event, pass, initial_down, pointer_id, verify_consumption_in_final_pass)
            }
            DragDetectionState::AwaitGesturePickup { initial_down, pointer_id } => {
                self.process_await_gesture_pickup(event, pass, initial_down, pointer_id)
            }
            DragDetectionState::Dragging { pointer_id } => self.process_dragging_state(event, pass, pointer_id),
        }
    }

    /// Compose's `resetDragDetectionState`.
    pub(crate) fn reset_drag_detection_state(&self) {
        self.move_to_await_down_state();
        if self.is_listening_for_events.get() {
            self.send_drag_cancelled();
        }
        *self.velocity_tracker.borrow_mut() = None;
    }

    fn move_to_await_touch_slop_state(
        &self,
        initial_down: PointerInputChange,
        pointer_id: PointerId,
        initial_touch_slop_position_change: Offset,
        verify_consumption_in_final_pass: bool,
    ) {
        {
            let mut detector = self.touch_slop_detector.borrow_mut();
            match detector.as_mut() {
                None => *detector = Some(TouchSlopDetector::new(self.orientation.get(), Offset::Zero)),
                Some(d) => {
                    d.set_orientation(self.orientation.get());
                    d.reset_to(initial_touch_slop_position_change);
                }
            }
        }
        *self.current_drag_state.borrow_mut() =
            Some(DragDetectionState::AwaitTouchSlop { initial_down, pointer_id, verify_consumption_in_final_pass });
    }

    fn move_to_dragging_state(&self, pointer_id: PointerId) {
        *self.current_drag_state.borrow_mut() = Some(DragDetectionState::Dragging { pointer_id });
    }

    fn move_to_await_down_state(&self) {
        self.drag_accumulator.set(Offset::Zero);
        *self.current_drag_state.borrow_mut() = Some(DragDetectionState::await_down());
    }

    fn move_to_await_gesture_pickup_state(&self, initial_down: PointerInputChange, pointer_id: PointerId) {
        if let Some(d) = self.touch_slop_detector.borrow_mut().as_mut() {
            d.reset();
        }
        *self.current_drag_state.borrow_mut() =
            Some(DragDetectionState::AwaitGesturePickup { initial_down, pointer_id });
    }

    fn set_state(&self, state: DragDetectionState) {
        *self.current_drag_state.borrow_mut() = Some(state);
    }

    /// Compose's `processInitialDownState`.
    fn process_initial_down_state(
        &self,
        event: &PointerEvent,
        pass: PointerEventPass,
        await_touch_slop: AwaitTouchSlop,
        mut consumed_on_initial: bool,
        mut has_seen_initial_event: bool,
    ) {
        if event.changes.is_empty() {
            return;
        }
        if !is_changed_to_down(event, false, FIRST_DOWN_REFERS_TO_PRIMARY_MOUSE_BUTTON_ONLY) {
            return;
        }
        let first_down = event.changes[0].clone();
        let await_touch_slop = match await_touch_slop {
            AwaitTouchSlop::NotInitialized => {
                let immediately = self.owner().is_some_and(|o| o.start_drag_immediately());
                if !immediately { AwaitTouchSlop::Yes } else { AwaitTouchSlop::No }
            }
            other => other,
        };
        if pass == PointerEventPass::Initial {
            // Not waiting for the touch slop: taken at once (dispatched in the Main pass, as it was).
            if await_touch_slop == AwaitTouchSlop::No {
                first_down.consume();
                consumed_on_initial = true;
            }
            // It says it waits for the touch slop from the first down on.
            has_seen_initial_event = true;
        }
        self.set_state(DragDetectionState::AwaitDown { await_touch_slop, consumed_on_initial, has_seen_initial_event });
        if pass == PointerEventPass::Main {
            if await_touch_slop == AwaitTouchSlop::Yes {
                self.move_to_await_touch_slop_state(first_down.clone(), first_down.id, Offset::Zero, false);
            } else if consumed_on_initial {
                self.send_drag_start(&first_down, &first_down, Offset::Zero);
                self.send_drag_event(&first_down, Offset::Zero);
                self.move_to_dragging_state(first_down.id);
            }
        }
    }

    /// Compose's `processAwaitTouchSlop`.
    fn process_await_touch_slop(
        &self,
        event: &PointerEvent,
        pass: PointerEventPass,
        initial_down: PointerInputChange,
        mut pointer_id: PointerId,
        mut verify_consumption_in_final_pass: bool,
    ) {
        if pass == PointerEventPass::Initial {
            return;
        }
        let drag_event = match event.changes.iter().find(|c| c.id == pointer_id) {
            Some(c) => c.clone(),
            None => match event.changes.iter().find(|c| c.pressed) {
                None => {
                    // No other pointer down: reset.
                    self.move_to_await_down_state();
                    return;
                }
                Some(other) => {
                    pointer_id = other.id;
                    other.clone()
                }
            },
        };
        let mut next = DragDetectionState::AwaitTouchSlop {
            initial_down: initial_down.clone(),
            pointer_id,
            verify_consumption_in_final_pass,
        };
        if pass == PointerEventPass::Main {
            if !drag_event.is_consumed() {
                if drag_event.changed_to_up_ignore_consumed() {
                    match event.changes.iter().find(|c| c.pressed) {
                        None => {
                            self.move_to_await_down_state();
                            return;
                        }
                        Some(other) => pointer_id = other.id,
                    }
                    next = DragDetectionState::AwaitTouchSlop {
                        initial_down: initial_down.clone(),
                        pointer_id,
                        verify_consumption_in_final_pass,
                    };
                    self.set_state(next);
                } else {
                    // A move.
                    let vc = self.handle.borrow().as_ref().map_or_else(
                        || LocalViewConfiguration.default_value(),
                        |h| h.current_value_of(&LocalViewConfiguration),
                    );
                    let touch_slop = pointer_slop(&vc, drag_event.r#type);
                    let post_slop_offset = self
                        .touch_slop_detector
                        .borrow_mut()
                        .as_mut()
                        .expect("Touch slop detector not initialized.")
                        .get_post_slop_offset(drag_event.position_change_ignore_consumed(), touch_slop);
                    if post_slop_offset.is_specified() {
                        let delta = drag_event.position_change();
                        self.drag_accumulator.set(self.drag_accumulator.get() + delta);
                        let acc = self.drag_accumulator.get();
                        let gesture_angle = acc.y.abs().atan2(acc.x.abs()) * (180.0 / std::f32::consts::PI);
                        let is_drag_angle_aligned_with_self_orientation = match self.orientation.get() {
                            None => true,
                            Some(o) => is_drag_angle_aligned_with_orientation(o, gesture_angle),
                        };
                        let mut has_drag_angle_aligned_with_parent_orientation = false;
                        if let Some(cx) = self.cx() {
                            traverse_parent_draggable_gestures(&cx, &mut |coordinator| {
                                let is_parent_waiting = coordinator.gesture_state() == GestureState::Waiting;
                                let aligned = match coordinator.orientation() {
                                    None => false,
                                    Some(o) => is_drag_angle_aligned_with_orientation(o, gesture_angle),
                                };
                                has_drag_angle_aligned_with_parent_orientation =
                                    has_drag_angle_aligned_with_parent_orientation || is_parent_waiting && aligned;
                                !has_drag_angle_aligned_with_parent_orientation
                            });
                        }
                        if !is_drag_angle_aligned_with_self_orientation
                            && has_drag_angle_aligned_with_parent_orientation
                        {
                            verify_consumption_in_final_pass = true;
                            self.set_state(DragDetectionState::AwaitTouchSlop {
                                initial_down: initial_down.clone(),
                                pointer_id,
                                verify_consumption_in_final_pass,
                            });
                        } else {
                            drag_event.consume();
                            self.send_drag_start(&initial_down, &drag_event, post_slop_offset);
                            self.send_drag_event(&drag_event, post_slop_offset);
                            self.move_to_dragging_state(drag_event.id);
                            return;
                        }
                    } else {
                        verify_consumption_in_final_pass = true;
                        // The touch slop isn't crossed: the deltas before it add up.
                        self.drag_accumulator
                            .set(self.drag_accumulator.get() + drag_event.position_change_ignore_consumed());
                        self.set_state(DragDetectionState::AwaitTouchSlop {
                            initial_down: initial_down.clone(),
                            pointer_id,
                            verify_consumption_in_final_pass,
                        });
                    }
                }
            } else {
                // Someone else consumed it: this drag lost the gesture, it may pick it up later.
                self.move_to_await_gesture_pickup_state(initial_down, pointer_id);
                return;
            }
        } else {
            self.set_state(next);
        }
        if pass == PointerEventPass::Final && verify_consumption_in_final_pass {
            if drag_event.is_consumed() {
                self.move_to_await_gesture_pickup_state(initial_down, pointer_id);
            } else {
                self.set_state(DragDetectionState::AwaitTouchSlop {
                    initial_down,
                    pointer_id,
                    verify_consumption_in_final_pass: false,
                });
            }
        }
    }

    /// Compose's `processAwaitGesturePickup`.
    fn process_await_gesture_pickup(
        &self,
        event: &PointerEvent,
        pass: PointerEventPass,
        initial_down: PointerInputChange,
        pointer_id: PointerId,
    ) {
        if pass != PointerEventPass::Final {
            return;
        }
        let has_unconsumed_drag = event.changes.iter().all(|c| !c.is_consumed());
        let has_down_pointers = event.changes.iter().any(|c| c.pressed);
        if !has_down_pointers || event.changes.is_empty() {
            // All pointers up: reset.
            self.move_to_await_down_state();
        } else if has_unconsumed_drag {
            // Pointers down, unconsumed: a chance to pick the gesture up, from the first down (most likely past
            // the slop at once).
            let initial_position_change = event.changes[0].position - initial_down.position;
            self.move_to_await_touch_slop_state(initial_down, pointer_id, initial_position_change, false);
        }
    }

    /// Compose's `processDraggingState`.
    fn process_dragging_state(&self, event: &PointerEvent, pass: PointerEventPass, pointer_id: PointerId) {
        if pass != PointerEventPass::Main {
            return;
        }
        let Some(drag_event) = event.changes.iter().find(|c| c.id == pointer_id) else { return };
        if drag_event.changed_to_up_ignore_consumed() {
            match event.changes.iter().find(|c| c.pressed) {
                None => {
                    // The last up.
                    if !drag_event.is_consumed() && drag_event.changed_to_up_ignore_consumed() {
                        self.send_drag_stopped(drag_event);
                    } else {
                        self.send_drag_cancelled();
                    }
                    self.move_to_await_down_state();
                }
                Some(other) => self.move_to_dragging_state(other.id),
            }
        } else if drag_event.is_consumed() {
            self.send_drag_cancelled();
        } else {
            let motion_change = drag_event.position_change_ignore_consumed().get_distance();
            if motion_change != 0.0 {
                let position_change = drag_event.position_change();
                self.send_drag_event(drag_event, position_change);
                drag_event.consume();
            }
        }
    }

    /// Compose's `sendDragStart` (the tracker fed the first down: its velocity tracker fix off).
    fn send_drag_start(
        &self,
        down: &PointerInputChange,
        slop_trigger_change: &PointerInputChange,
        over_slop_offset: Offset,
    ) {
        self.velocity_tracker.borrow_mut().get_or_insert_with(VelocityTracker::new).add_pointer_input_change(down);
        let drag_started_offset = slop_trigger_change.position - over_slop_offset;
        let can_drag = self.can_drag.borrow().clone();
        if can_drag(down.r#type) {
            if !self.is_listening_for_events.get() {
                if self.channel.borrow().is_none() {
                    *self.channel.borrow_mut() = Some(Channel::unlimited());
                }
                self.start_listening_for_events();
            }
            self.require_channel().try_send(DragEvent::DragStarted { start_point: drag_started_offset });
        }
    }

    /// Compose's `sendDragEvent`.
    fn send_drag_event(&self, change: &PointerInputChange, drag_amount: Offset) {
        self.drag_accumulator.set(self.drag_accumulator.get() + drag_amount);
        if let Some(t) = self.velocity_tracker.borrow_mut().as_mut() {
            t.add_pointer_input_change(change);
        }
        if let Some(channel) = self.channel.borrow().as_ref() {
            channel.try_send(DragEvent::DragDelta(DragDelta { delta: drag_amount, is_indirect_pointer_event: false }));
        }
    }

    /// Compose's `sendDragStopped`.
    fn send_drag_stopped(&self, change: &PointerInputChange) {
        let maximum_velocity = self
            .handle
            .borrow()
            .as_ref()
            .map_or_else(|| LocalViewConfiguration.default_value(), |h| h.current_value_of(&LocalViewConfiguration))
            .maximum_fling_velocity;
        let velocity = {
            let mut tracker = self.velocity_tracker.borrow_mut();
            let t = tracker.get_or_insert_with(VelocityTracker::new);
            t.add_pointer_input_change(change);
            let v = t.calculate_velocity_max(Velocity::new(maximum_velocity, maximum_velocity));
            t.reset_tracking();
            v
        };
        if let Some(channel) = self.channel.borrow().as_ref() {
            channel.try_send(DragEvent::DragStopped(DragStopped {
                velocity: to_valid_velocity(velocity),
                is_indirect_pointer_event: false,
            }));
        }
        self.is_listening_for_pointer_input_events.set(false);
    }

    fn send_drag_cancelled(&self) {
        if let Some(channel) = self.channel.borrow().as_ref() {
            channel.try_send(DragEvent::DragCancelled);
        }
    }

    /// An event from the indirect pointer detection (Compose's `onDragEvent`).
    pub(crate) fn on_drag_event(&self, event: DragEvent) {
        if matches!(event, DragEvent::DragStarted { .. }) && !self.is_listening_for_events.get() {
            self.is_listening_for_events.set(true);
            self.start_listening_for_events();
        }
        if self.channel.borrow().is_none() {
            *self.channel.borrow_mut() = Some(Channel::unlimited());
        }
        self.require_channel().try_send(event);
    }

    /// The owner's start-at-once, for the indirect detection.
    pub(crate) fn start_drag_immediately(&self) -> bool {
        self.owner().is_some_and(|o| o.start_drag_immediately())
    }

    /// The view configuration where the node is.
    pub(crate) fn view_configuration(&self) -> kompoz_ui::platform::ViewConfiguration {
        self.handle
            .borrow()
            .as_ref()
            .map_or_else(|| LocalViewConfiguration.default_value(), |h| h.current_value_of(&LocalViewConfiguration))
    }

    /// Whether the node's pointer can drag it.
    pub(crate) fn can_drag(&self, pointer_type: PointerType) -> bool {
        let can_drag = self.can_drag.borrow().clone();
        can_drag(pointer_type)
    }
}

/// The drag cancelled where its coroutine waited: dropped with the node's scope (Kotlin's catch).
struct CancelOnDrop(Option<Rc<DragGestureCore>>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(core) = self.0.take() {
            core.process_drag_cancel_now();
        }
    }
}

impl GestureConnection for DragGestureCore {
    /// Compose's DragGestureNode `gestureState`.
    fn gesture_state(&self) -> GestureState {
        if !self.enabled.get() {
            return GestureState::Idle;
        }
        match &*self.current_drag_state.borrow() {
            Some(DragDetectionState::AwaitDown { has_seen_initial_event, .. }) => {
                if *has_seen_initial_event {
                    GestureState::Waiting
                } else {
                    GestureState::Idle
                }
            }
            Some(DragDetectionState::AwaitTouchSlop { .. }) => GestureState::Waiting,
            Some(DragDetectionState::AwaitGesturePickup { .. }) => GestureState::Waiting,
            Some(DragDetectionState::Dragging { .. }) => GestureState::Recognized,
            None => GestureState::Idle,
        }
    }

    fn as_draggable(&self) -> Option<&dyn DraggableGestureConnection> {
        Some(self)
    }
}

impl DraggableGestureConnection for DragGestureCore {
    fn orientation(&self) -> Option<Orientation> {
        self.orientation.get()
    }
}

/// A velocity with NaN made 0 (Compose's `toValidVelocity`).
pub(crate) fn to_valid_velocity(v: Velocity) -> Velocity {
    Velocity::new(if v.x.is_nan() { 0.0 } else { v.x }, if v.y.is_nan() { 0.0 } else { v.y })
}

const HorizontalAngleUpperBounds: f32 = 30.0;
const VerticalAngleUpperBounds: f32 = 90.0;

/// Whether a drag at `drag_angle` degrees from the horizontal (0 to 90) goes along `orientation` (Compose's
/// `isDragAngleAlignedWithOrientation`).
fn is_drag_angle_aligned_with_orientation(orientation: Orientation, drag_angle: f32) -> bool {
    if orientation == Orientation::Horizontal {
        drag_angle <= HorizontalAngleUpperBounds
    } else {
        drag_angle > HorizontalAngleUpperBounds && drag_angle <= VerticalAngleUpperBounds
    }
}

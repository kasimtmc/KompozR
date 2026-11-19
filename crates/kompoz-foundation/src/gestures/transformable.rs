//! Zooming, panning and turning with the user's gestures, as Compose's Transformable.kt: `Modifier.transformable`
//! gives a [`TransformableState`] what the fingers do together past the touch slop (two fingers spreading,
//! turning, moving — one finger panning when `can_pan` says so), and on a desktop what the mouse and trackpad do:
//! Ctrl with the wheel or a trackpad pan zooms (Chrome OS's curve: 545 px of scroll double it), a trackpad pan
//! pans, a trackpad pinch zooms.

use super::PointerEventTransforms;
use super::for_each::await_each_gesture;
use super::mouse_wheel::{ScrollConfig, platform_scroll_config};
use super::tap::await_first_down;
use super::transform::calculate_centroid;
use super::transformable_state::{TransformScope, TransformableState};
use kompoz_animation::MutatePriority;
use kompoz_coroutines::{Channel, CoroutineStart, catch_cancellation, coroutine_scope};
use kompoz_graphics::{IntSize, Offset};
use kompoz_runtime::{named, named_args};
use kompoz_ui::input::{
    AwaitPointerEventScope, PointerEvent, PointerEventPass, PointerEventType, PointerInputModifierNode,
    PointerInputScope, PointerType, SuspendingPointerInputModifierNode,
};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::f32::consts::PI;
use std::rc::{Rc, Weak};

/// What a pointer's moves say to the transformation under way (Compose's TransformEvent, its variants' names
/// Compose's).
#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(clippy::enum_variant_names)]
enum TransformEvent {
    TransformStarted,
    TransformStopped,
    TransformDelta { centroid: Offset, zoom_change: f32, pan_change: Offset, rotation_change: f32 },
}

/// The factor a mouse's scroll becomes a zoom by: every 545 px of scroll doubles it (Compose's SCROLL_FACTOR, from
/// curve fitting Chrome OS's zoom factors).
const SCROLL_FACTOR: f32 = 545.0;

/// The transformable modifier.
#[named(trait TransformableModifiers)]
impl Modifier {
    /// Gives `state` the user's zooms, pans and turns (Compose's `Modifier.transformable(state,
    /// lockRotationOnZoomPan, enabled)`): see [`transformable_with_can_pan`](TransformableModifiers::transformable_with_can_pan),
    /// one finger always panning.
    fn transformable(
        self,
        state: impl TransformableState,
        #[default(false)] lock_rotation_on_zoom_pan: bool,
        #[default(true)] enabled: bool,
    ) -> Modifier {
        self.then_element(TransformableElement {
            state: Rc::new(state),
            can_pan: Rc::new(|_| true),
            lock_rotation_on_zoom_pan,
            enabled,
        })
    }

    /// Gives `state` the user's zooms, pans and turns (Compose's `Modifier.transformable(state, canPan,
    /// lockRotationOnZoomPan, enabled)`): past the touch slop — a zoom's or a turn's, or a pan's that `can_pan`
    /// takes (it is given the pan; one finger left to a scroll around it when it says no) — each event's centroid,
    /// zoom, pan and turn (degrees clockwise; none once a gesture started as a zoom or a pan with
    /// `lock_rotation_on_zoom_pan`), in a user's mutation of the state. The mouse: Ctrl with the wheel zooms, a
    /// trackpad's pan pans (or zooms with Ctrl), its pinch zooms. Not `enabled`: nothing.
    /// Kotlin: `transformable(state: TransformableState, canPan: (Offset) -> Boolean, lockRotationOnZoomPan: Boolean = false, enabled: Boolean = true)`
    fn transformable_with_can_pan(
        self,
        state: impl TransformableState,
        can_pan: impl Fn(Offset) -> bool + 'static,
        #[default(false)] lock_rotation_on_zoom_pan: bool,
        #[default(true)] enabled: bool,
    ) -> Modifier {
        self.then_element(TransformableElement {
            state: Rc::new(state),
            can_pan: Rc::new(can_pan),
            lock_rotation_on_zoom_pan,
            enabled,
        })
    }
}

struct TransformableElement {
    state: Rc<dyn TransformableState>,
    can_pan: Rc<dyn Fn(Offset) -> bool>,
    lock_rotation_on_zoom_pan: bool,
    enabled: bool,
}

impl PartialEq for TransformableElement {
    fn eq(&self, o: &Self) -> bool {
        *self.state == *o.state
            && Rc::ptr_eq(&self.can_pan, &o.can_pan)
            && self.lock_rotation_on_zoom_pan == o.lock_rotation_on_zoom_pan
            && self.enabled == o.enabled
    }
}

impl ModifierNodeElement for TransformableElement {
    type Node = TransformableNode;
    fn create(&self) -> TransformableNode {
        TransformableNode::new(self)
    }
    fn update(&self, node: &mut TransformableNode) {
        node.core.update(self);
    }
    fn name(&self) -> &'static str {
        "transformable"
    }
}

/// `transformable`'s node (Compose's TransformableNode): its gesture block (and, once a mouse comes, the mouse's)
/// sending transform events to a coroutine that applies them in the state's mutation. As Compose's, it hands its
/// pointer events to those blocks itself.
pub struct TransformableNode {
    core: Rc<TransformableCore>,
}

struct TransformableCore {
    state: RefCell<Rc<dyn TransformableState>>,
    can_pan: RefCell<Rc<dyn Fn(Offset) -> bool>>,
    lock_rotation_on_zoom_pan: Cell<bool>,
    enabled: Cell<bool>,
    channel: Channel<TransformEvent>,
    scroll_config: RefCell<Option<Rc<dyn ScrollConfig>>>,
    pointer_input_node: RefCell<SuspendingPointerInputModifierNode>,
    pointer_input_modifier_mouse: RefCell<Option<SuspendingPointerInputModifierNode>>,
    handle: RefCell<Option<NodeHandle>>,
}

impl TransformableNode {
    fn new(e: &TransformableElement) -> TransformableNode {
        let core = Rc::new_cyclic(|me: &Weak<TransformableCore>| {
            let me = me.clone();
            TransformableCore {
                state: RefCell::new(e.state.clone()),
                can_pan: RefCell::new(e.can_pan.clone()),
                lock_rotation_on_zoom_pan: Cell::new(e.lock_rotation_on_zoom_pan),
                enabled: Cell::new(e.enabled),
                channel: Channel::unlimited(),
                scroll_config: RefCell::new(None),
                pointer_input_node: RefCell::new(SuspendingPointerInputModifierNode::new(
                    move |scope: PointerInputScope| pointer_input(scope, me.clone()),
                )),
                pointer_input_modifier_mouse: RefCell::new(None),
                handle: RefCell::new(None),
            }
        });
        TransformableNode { core }
    }
}

impl TransformableCore {
    /// Compose's TransformableNode `update`: a new state, `enabled` or lock restart the gesture block.
    fn update(&self, e: &TransformableElement) {
        *self.can_pan.borrow_mut() = e.can_pan.clone();
        let needs_reset = **self.state.borrow() != *e.state
            || self.enabled.get() != e.enabled
            || self.lock_rotation_on_zoom_pan.get() != e.lock_rotation_on_zoom_pan;
        if needs_reset {
            *self.state.borrow_mut() = e.state.clone();
            self.enabled.set(e.enabled);
            self.lock_rotation_on_zoom_pan.set(e.lock_rotation_on_zoom_pan);
            self.pointer_input_node.borrow().reset_pointer_input_handler();
        }
    }

    /// The `can_pan` of the latest update (Compose's `updatedCanPan`).
    fn updated_can_pan(&self, pan: Offset) -> bool {
        let can_pan = self.can_pan.borrow().clone();
        can_pan(pan)
    }
}

/// The gesture block (Compose's `pointerInputNode`'s): a coroutine applying the channel's transformations in the
/// state's mutation, and each gesture's zoom detection.
async fn pointer_input(scope: PointerInputScope, core: Weak<TransformableCore>) {
    let Some(c) = core.upgrade() else { return };
    if !c.enabled.get() {
        return;
    }
    let channel = c.channel.clone();
    drop(c);
    coroutine_scope(move |cs| async move {
        let (receiving, receiving_core) = (channel.clone(), core.clone());
        named_args!(cs.launch(start = CoroutineStart::UNDISPATCHED, async move {
            loop {
                let first = match receiving.receive().await {
                    Some(TransformEvent::TransformStarted) => TransformEvent::TransformStarted,
                    Some(_) => continue,
                    None => return,
                };
                let Some(c) = receiving_core.upgrade() else { return };
                let state = c.state.borrow().clone();
                drop(c);
                let ch = receiving.clone();
                // (a cancelled transformation — another took the state — ignored: it starts over)
                let _ = catch_cancellation(move |_| async move {
                    state
                        .transform_block(
                            MutatePriority::UserInput,
                            Box::new(move |transform_scope: Rc<dyn TransformScope>| {
                                Box::pin(async move {
                                    let mut event = Some(first);
                                    // (a closed channel, never: as stopped)
                                    while !matches!(event, Some(TransformEvent::TransformStopped) | None) {
                                        if let Some(TransformEvent::TransformDelta {
                                            centroid,
                                            zoom_change,
                                            pan_change,
                                            rotation_change,
                                        }) = event
                                        {
                                            transform_scope.transform_by_with_centroid(
                                                centroid,
                                                zoom_change,
                                                pan_change,
                                                rotation_change,
                                            );
                                        }
                                        event = ch.receive().await;
                                    }
                                })
                            }),
                        )
                        .await
                })
                .await;
            }
        }));
        await_each_gesture(&scope, move |s| {
            let (core, channel) = (core.clone(), channel.clone());
            async move {
                let _stopped = SendStoppedOnDrop(channel.clone());
                detect_zoom(&s, &core, &channel).await;
            }
        })
        .await;
    })
    .await;
}

/// Tells the transformation it stopped however the gesture ends (Compose's `finally { channel.trySend(
/// TransformStopped) }`).
struct SendStoppedOnDrop(Channel<TransformEvent>);

impl Drop for SendStoppedOnDrop {
    fn drop(&mut self) {
        self.0.try_send(TransformEvent::TransformStopped);
    }
}

/// Whether an event is a trackpad's pan or pinch.
fn is_pan(t: PointerEventType) -> bool {
    matches!(t, PointerEventType::PanStart | PointerEventType::PanMove | PointerEventType::PanEnd)
}

fn is_scale(t: PointerEventType) -> bool {
    matches!(t, PointerEventType::ScaleStart | PointerEventType::ScaleChange | PointerEventType::ScaleEnd)
}

/// One gesture's zooms, turns and pans past the touch slop (Compose's `detectZoom`): sent to the channel, the
/// moves consumed; a consumed event (or a trackpad's) ends it, as does one consumed in its final pass before the
/// slop (someone else took it).
async fn detect_zoom(s: &AwaitPointerEventScope, core: &Weak<TransformableCore>, channel: &Channel<TransformEvent>) {
    let pan_zoom_lock = core.upgrade().is_some_and(|c| c.lock_rotation_on_zoom_pan.get());
    let can_pan = |pan: Offset| core.upgrade().is_some_and(|c| c.updated_can_pan(pan));
    let mut rotation = 0.0f32;
    let mut zoom = 1.0f32;
    let mut pan = Offset::Zero;
    let mut past_touch_slop = false;
    let touch_slop = s.view_configuration().touch_slop;
    let mut locked_to_pan_zoom = false;
    await_first_down(s, false, PointerEventPass::Main).await;
    loop {
        let event = s.await_pointer_event().await;
        let canceled = event.changes.iter().any(|c| c.is_consumed()) || is_pan(event.r#type) || is_scale(event.r#type);
        if !canceled {
            let zoom_change = event.calculate_zoom();
            let rotation_change = event.calculate_rotation();
            let pan_change = event.calculate_pan();
            if !past_touch_slop {
                zoom *= zoom_change;
                rotation += rotation_change;
                pan = pan + pan_change;
                let centroid_size = event.calculate_centroid_size(false);
                let zoom_motion = (1.0 - zoom).abs() * centroid_size;
                let rotation_motion = (rotation * PI * centroid_size / 180.0).abs();
                let pan_motion = pan.get_distance();
                if zoom_motion > touch_slop
                    || rotation_motion > touch_slop
                    || (pan_motion > touch_slop && can_pan(pan_change))
                {
                    past_touch_slop = true;
                    locked_to_pan_zoom = pan_zoom_lock && rotation_motion < touch_slop;
                    channel.try_send(TransformEvent::TransformStarted);
                }
            }
            if past_touch_slop {
                let centroid = event.calculate_centroid(false);
                let effective_rotation = if locked_to_pan_zoom { 0.0 } else { rotation_change };
                if effective_rotation != 0.0
                    || zoom_change != 1.0
                    || (pan_change != Offset::Zero && can_pan(pan_change))
                {
                    channel.try_send(TransformEvent::TransformDelta {
                        centroid,
                        zoom_change,
                        pan_change,
                        rotation_change: effective_rotation,
                    });
                }
                event.changes.iter().filter(|c| c.position_changed()).for_each(|c| c.consume());
            }
        } else {
            channel.try_send(TransformEvent::TransformStopped);
        }
        let final_event = s.await_pointer_event_with(PointerEventPass::Final).await;
        // (someone consumed it while it waited for the touch slop)
        let finally_canceled = final_event.changes.iter().any(|c| c.is_consumed()) && !past_touch_slop;
        if canceled || finally_canceled || !event.changes.iter().any(|c| c.pressed) {
            break;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The mouse and the trackpad

/// The pans an event's change and its history make (Compose's `-panOffset` summed): the scroll the fingers make
/// turned back into their movement.
fn pan_delta_of(pointer: &PointerEvent) -> Offset {
    pointer
        .changes
        .first()
        .map_or(Offset::Zero, |c| c.historical().iter().fold(c.pan_offset * -1.0, |acc, h| acc - h.pan_offset))
}

/// Ctrl with the wheel or a trackpad's pan: the scroll it makes, consumed (Compose's
/// `consumePointerEventAsCtrlScrollOrNull`).
fn consume_pointer_event_as_ctrl_scroll_or_null(
    s: &AwaitPointerEventScope,
    pointer: &PointerEvent,
    scroll_config: &dyn ScrollConfig,
) -> Option<Offset> {
    if !pointer.keyboard_modifiers.is_ctrl_pressed()
        || (pointer.r#type != PointerEventType::Scroll && !is_pan(pointer.r#type))
    {
        return None;
    }
    let scroll_delta =
        scroll_config.calculate_mouse_wheel_scroll(s.density(), pointer, s.size()) + pan_delta_of(pointer);
    if scroll_delta == Offset::Zero {
        return None;
    }
    pointer.changes.iter().for_each(|c| c.consume());
    Some(scroll_delta)
}

/// A trackpad's pan: its movement, consumed (Compose's `consumePointerEventAsPanOrNull`).
fn consume_pointer_event_as_pan_or_null(pointer: &PointerEvent) -> Option<Offset> {
    if !is_pan(pointer.r#type) {
        return None;
    }
    let scroll_delta = pan_delta_of(pointer);
    if scroll_delta == Offset::Zero {
        return None;
    }
    pointer.changes.iter().for_each(|c| c.consume());
    Some(scroll_delta)
}

/// A trackpad's pinch: its scale, consumed (Compose's `consumePointerEventAsScaleOrNull`; its twist, KompozR's,
/// isn't a turn here, as Compose has none).
fn consume_pointer_event_as_scale_or_null(pointer: &PointerEvent) -> Option<f32> {
    if !is_scale(pointer.r#type) {
        return None;
    }
    let mut scale_delta = 1.0f32;
    for c in &pointer.changes {
        scale_delta *= c.scale_factor;
        for h in c.historical() {
            scale_delta *= h.scale_factor;
        }
    }
    if scale_delta == 1.0 {
        return None;
    }
    pointer.changes.iter().for_each(|c| c.consume());
    Some(scale_delta)
}

/// The mouse's and the trackpad's transformations (Compose's `detectNonTouchGestures`): Ctrl+scroll zooms, a pan
/// pans, a pinch zooms, each a transformation of its own while its events go on.
async fn detect_non_touch_gestures(
    scope: PointerInputScope,
    channel: Channel<TransformEvent>,
    scroll_config: Rc<dyn ScrollConfig>,
) {
    scope
        .await_pointer_event_scope(move |s| async move {
            loop {
                let _stopped = SendStoppedOnDrop(channel.clone());
                let mut pointer;
                let (mut zoom_offset, mut pan_offset, mut scale);
                loop {
                    pointer = s.await_pointer_event().await;
                    // (Ctrl's scroll first, then a pan, then a pinch)
                    zoom_offset = consume_pointer_event_as_ctrl_scroll_or_null(&s, &pointer, &*scroll_config);
                    pan_offset = consume_pointer_event_as_pan_or_null(&pointer);
                    scale = consume_pointer_event_as_scale_or_null(&pointer);
                    if zoom_offset.is_some() || pan_offset.is_some() || scale.is_some() {
                        break;
                    }
                }
                let centroid_of = |p: &PointerEvent| calculate_centroid(p, true, |_| true);
                if let Some(mut scroll_delta) = zoom_offset {
                    channel.try_send(TransformEvent::TransformStarted);
                    loop {
                        // (Chrome OS's Ctrl+scroll curve)
                        let zoom_change = 2.0f32.powf(scroll_delta.y / SCROLL_FACTOR);
                        channel.try_send(TransformEvent::TransformDelta {
                            centroid: centroid_of(&pointer),
                            zoom_change,
                            pan_change: Offset::Zero,
                            rotation_change: 0.0,
                        });
                        pointer = s.await_pointer_event().await;
                        match consume_pointer_event_as_ctrl_scroll_or_null(&s, &pointer, &*scroll_config) {
                            Some(d) => scroll_delta = d,
                            None => break,
                        }
                    }
                } else if let Some(mut pan_delta) = pan_offset {
                    channel.try_send(TransformEvent::TransformStarted);
                    loop {
                        channel.try_send(TransformEvent::TransformDelta {
                            centroid: centroid_of(&pointer),
                            zoom_change: 1.0,
                            pan_change: pan_delta,
                            rotation_change: 0.0,
                        });
                        pointer = s.await_pointer_event().await;
                        match consume_pointer_event_as_pan_or_null(&pointer) {
                            Some(d) => pan_delta = d,
                            None => break,
                        }
                    }
                } else {
                    let mut scale_delta = scale.expect("One of zoomOffset, panOffset and scaleDelta must be non-null");
                    channel.try_send(TransformEvent::TransformStarted);
                    loop {
                        channel.try_send(TransformEvent::TransformDelta {
                            centroid: centroid_of(&pointer),
                            zoom_change: scale_delta,
                            pan_change: Offset::Zero,
                            rotation_change: 0.0,
                        });
                        pointer = s.await_pointer_event().await;
                        match consume_pointer_event_as_scale_or_null(&pointer) {
                            Some(d) => scale_delta = d,
                            None => break,
                        }
                    }
                }
            }
        })
        .await
}

impl ModifierNode for TransformableNode {
    node_capabilities!(pointer_input);

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.core.handle.borrow_mut() = Some(cx.handle());
        // (its gesture blocks run in its own coroutine scope: it hands them its events)
        self.core.pointer_input_node.borrow_mut().on_attach(cx);
        if let Some(mouse) = self.core.pointer_input_modifier_mouse.borrow_mut().as_mut() {
            mouse.on_attach(cx);
        }
        *self.core.scroll_config.borrow_mut() = Some(platform_scroll_config(cx));
    }

    fn on_detach(&mut self, cx: &NodeCx) {
        self.core.pointer_input_node.borrow_mut().on_detach(cx);
        if let Some(mouse) = self.core.pointer_input_modifier_mouse.borrow_mut().as_mut() {
            mouse.on_detach(cx);
        }
    }
}

impl PointerInputModifierNode for TransformableNode {
    /// Compose's TransformableNode `onPointerEvent`: the mouse's block made at the first mouse event, then both
    /// given the event.
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        let core = &self.core;
        let scroll_config = core.scroll_config.borrow().clone();
        if core.enabled.get()
            && event.changes.iter().any(|c| c.r#type == PointerType::Mouse)
            && let Some(scroll_config) = scroll_config
            && core.pointer_input_modifier_mouse.borrow().is_none()
        {
            let channel = core.channel.clone();
            let mut node = SuspendingPointerInputModifierNode::new(move |scope: PointerInputScope| {
                detect_non_touch_gestures(scope, channel.clone(), scroll_config.clone())
            });
            if let Some(cx) = core.handle.borrow().as_ref().and_then(|h| h.cx()) {
                node.on_attach(&cx);
            }
            *core.pointer_input_modifier_mouse.borrow_mut() = Some(node);
        }
        core.pointer_input_node.borrow_mut().on_pointer_event(event, pass, bounds);
        if let Some(mouse) = core.pointer_input_modifier_mouse.borrow_mut().as_mut() {
            mouse.on_pointer_event(event, pass, bounds);
        }
    }

    fn on_cancel_pointer_input(&mut self) {
        self.core.pointer_input_node.borrow_mut().on_cancel_pointer_input();
        if let Some(mouse) = self.core.pointer_input_modifier_mouse.borrow_mut().as_mut() {
            mouse.on_cancel_pointer_input();
        }
    }
}

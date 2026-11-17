//! Drags of an indirect pointer (a touchpad not over the window, a remote's swipe surface), as Compose's
//! IndirectPointerInputDragCycleDetector.kt: the same detection as a pointer's, along the device's main axis
//! mapped onto the drag's, its moves and the velocity smoothed over the last few events.

use super::Orientation;
use super::drag::{TouchSlopDetector, pointer_slop};
use super::draggable::{DragDelta, DragEvent, DragGestureCore, DragStopped, to_valid_velocity};
use crate::gesture_node::{DraggableGestureConnection, GestureConnection, GestureState};
use kompoz_graphics::Offset;
use kompoz_ui::input::{
    IndirectPointerEvent, IndirectPointerEventPrimaryDirectionalMotionAxis, IndirectPointerInputChange,
    PointerEventPass, PointerId, PointerType, VelocityTracker,
};
use kompoz_ui::unit::Velocity;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// Below this a drag's move along its axis is left out (Compose's PixelSensibility).
const PixelSensibility: f32 = 2.0;

/// What a change of an indirect pointer was when the detection kept it.
#[derive(Clone, Copy, Debug)]
struct ChangeSnapshot {
    id: PointerId,
    uptime_millis: i64,
    position: Offset,
    pressed: bool,
    previous_pressed: bool,
}

impl ChangeSnapshot {
    fn of(c: &IndirectPointerInputChange) -> ChangeSnapshot {
        ChangeSnapshot {
            id: c.id,
            uptime_millis: c.uptime_millis,
            position: c.position,
            pressed: c.pressed,
            previous_pressed: c.previous_pressed,
        }
    }
    fn changed_to_up_ignore_consumed(&self) -> bool {
        self.previous_pressed && !self.pressed
    }
    fn changed_to_down_ignore_consumed(&self) -> bool {
        !self.previous_pressed && self.pressed
    }
}

/// Where the detection is (its DragDetectionState).
#[derive(Clone, Copy, Debug)]
enum State {
    AwaitDown { await_touch_slop: Option<bool>, consumed_on_initial: bool, has_seen_initial_event: bool },
    AwaitTouchSlop { initial_down: ChangeSnapshot, pointer_id: PointerId, verify_consumption_in_final_pass: bool },
    AwaitGesturePickup { initial_down: ChangeSnapshot, pointer_id: PointerId },
    Dragging { pointer_id: PointerId },
}

const AwaitDownState: State =
    State::AwaitDown { await_touch_slop: None, consumed_on_initial: false, has_seen_initial_event: false };

/// The drag detection of a drag node's indirect pointers (Compose's IndirectPointerInputDragCycleDetector): its
/// events go to the node's drag coroutine.
pub(crate) struct IndirectPointerInputDragCycleDetector {
    node: Weak<DragGestureCore>,
    current_drag_state: Cell<Option<State>>,
    velocity_tracker: RefCell<Option<VelocityTracker>>,
    touch_slop_detector: RefCell<Option<TouchSlopDetector>>,
    touch_smooth: RefCell<IndirectPointerInputEventSmoother>,
    offset_smoother: RefCell<OffsetSmoother>,
}

impl IndirectPointerInputDragCycleDetector {
    pub(crate) fn new(node: Weak<DragGestureCore>) -> Rc<IndirectPointerInputDragCycleDetector> {
        Rc::new(IndirectPointerInputDragCycleDetector {
            node,
            current_drag_state: Cell::new(None),
            velocity_tracker: RefCell::new(None),
            touch_slop_detector: RefCell::new(None),
            touch_smooth: RefCell::new(IndirectPointerInputEventSmoother::default()),
            offset_smoother: RefCell::new(OffsetSmoother::default()),
        })
    }

    fn orientation(&self) -> Option<Orientation> {
        self.node.upgrade().and_then(|n| n.orientation.get())
    }

    /// Compose's `processIndirectPointerInputEvent`.
    pub(crate) fn process_indirect_pointer_input_event(&self, event: &IndirectPointerEvent, pass: PointerEventPass) {
        if self.current_drag_state.get().is_none() {
            self.current_drag_state.set(Some(AwaitDownState));
        }
        match self.current_drag_state.get().expect("currentDragState should not be null") {
            State::AwaitDown { await_touch_slop, consumed_on_initial, has_seen_initial_event } => self
                .process_initial_down_state(event, pass, await_touch_slop, consumed_on_initial, has_seen_initial_event),
            State::AwaitTouchSlop { initial_down, pointer_id, verify_consumption_in_final_pass } => {
                self.process_await_touch_slop(event, pass, initial_down, pointer_id, verify_consumption_in_final_pass)
            }
            State::AwaitGesturePickup { initial_down, pointer_id } => {
                self.process_await_gesture_pickup(event, pass, initial_down, pointer_id)
            }
            State::Dragging { pointer_id } => self.process_dragging_state(event, pass, pointer_id),
        }
    }

    /// Compose's `resetDragDetectionState`.
    pub(crate) fn reset_drag_detection_state(&self) {
        self.move_to_await_down_state();
        if self.node.upgrade().is_some_and(|n| n.is_listening_for_events.get()) {
            self.send_drag_cancelled();
        }
        *self.velocity_tracker.borrow_mut() = None;
        self.offset_smoother.borrow_mut().reset();
    }

    fn move_to_await_touch_slop_state(
        &self,
        initial_down: ChangeSnapshot,
        pointer_id: PointerId,
        initial_touch_slop_position_change: Offset,
    ) {
        {
            let mut detector = self.touch_slop_detector.borrow_mut();
            match detector.as_mut() {
                None => *detector = Some(TouchSlopDetector::new(self.orientation(), Offset::Zero)),
                Some(d) => {
                    d.set_orientation(self.orientation());
                    d.reset_to(initial_touch_slop_position_change);
                }
            }
        }
        self.current_drag_state.set(Some(State::AwaitTouchSlop {
            initial_down,
            pointer_id,
            verify_consumption_in_final_pass: false,
        }));
    }

    fn move_to_dragging_state(&self, pointer_id: PointerId) {
        self.current_drag_state.set(Some(State::Dragging { pointer_id }));
    }

    fn move_to_await_down_state(&self) {
        self.current_drag_state.set(Some(AwaitDownState));
    }

    fn move_to_await_gesture_pickup_state(&self, initial_down: ChangeSnapshot, pointer_id: PointerId) {
        if let Some(d) = self.touch_slop_detector.borrow_mut().as_mut() {
            d.reset();
        }
        self.current_drag_state.set(Some(State::AwaitGesturePickup { initial_down, pointer_id }));
    }

    fn process_initial_down_state(
        &self,
        event: &IndirectPointerEvent,
        pass: PointerEventPass,
        await_touch_slop: Option<bool>,
        mut consumed_on_initial: bool,
        mut has_seen_initial_event: bool,
    ) {
        if event.changes.is_empty() {
            return;
        }
        if !event.changes.iter().all(|c| !c.previous_pressed && c.pressed) {
            return;
        }
        let first_down = &event.changes[0];
        let await_touch_slop =
            await_touch_slop.unwrap_or_else(|| !self.node.upgrade().is_some_and(|n| n.start_drag_immediately()));
        if pass == PointerEventPass::Initial {
            if !await_touch_slop {
                first_down.consume();
                consumed_on_initial = true;
            }
            has_seen_initial_event = true;
        }
        self.current_drag_state.set(Some(State::AwaitDown {
            await_touch_slop: Some(await_touch_slop),
            consumed_on_initial,
            has_seen_initial_event,
        }));
        if pass == PointerEventPass::Main {
            let down = ChangeSnapshot::of(first_down);
            if await_touch_slop {
                self.move_to_await_touch_slop_state(down, down.id, Offset::Zero);
            } else if consumed_on_initial {
                let axis = event.primary_directional_motion_axis;
                self.send_drag_start(down, down, axis, Offset::Zero);
                self.send_drag_event(down, axis, Offset::Zero);
                self.move_to_dragging_state(down.id);
            }
        }
    }

    fn process_await_touch_slop(
        &self,
        event: &IndirectPointerEvent,
        pass: PointerEventPass,
        initial_down: ChangeSnapshot,
        mut pointer_id: PointerId,
        mut verify_consumption_in_final_pass: bool,
    ) {
        if pass == PointerEventPass::Initial {
            return;
        }
        let drag_event = match event.changes.iter().find(|c| c.id == pointer_id) {
            Some(c) => c,
            None => match event.changes.iter().find(|c| c.pressed) {
                None => {
                    self.move_to_await_down_state();
                    return;
                }
                Some(other) => {
                    pointer_id = other.id;
                    other
                }
            },
        };
        let axis = event.primary_directional_motion_axis;
        let orientation = self.orientation();
        if pass == PointerEventPass::Main {
            if !drag_event.is_consumed() {
                if drag_event.previous_pressed && !drag_event.pressed {
                    match event.changes.iter().find(|c| c.pressed) {
                        None => {
                            self.move_to_await_down_state();
                            return;
                        }
                        Some(other) => pointer_id = other.id,
                    }
                } else {
                    let touch_slop =
                        self.node.upgrade().map_or(0.0, |n| pointer_slop(&n.view_configuration(), PointerType::Touch));
                    let post_slop_offset = self
                        .touch_slop_detector
                        .borrow_mut()
                        .as_mut()
                        .expect("Touch slop detector not initialized.")
                        .get_post_slop_offset(
                            position_change_internal(drag_event, orientation, axis, true),
                            touch_slop,
                        );
                    if post_slop_offset.is_specified() {
                        drag_event.consume();
                        let change = ChangeSnapshot::of(drag_event);
                        self.send_drag_start(initial_down, change, axis, post_slop_offset);
                        self.send_drag_event(change, axis, post_slop_offset);
                        self.move_to_dragging_state(drag_event.id);
                        return;
                    } else {
                        verify_consumption_in_final_pass = true;
                    }
                }
            } else {
                self.move_to_await_gesture_pickup_state(initial_down, pointer_id);
                return;
            }
        }
        self.current_drag_state.set(Some(State::AwaitTouchSlop {
            initial_down,
            pointer_id,
            verify_consumption_in_final_pass,
        }));
        if pass == PointerEventPass::Final && verify_consumption_in_final_pass {
            if drag_event.is_consumed() {
                self.move_to_await_gesture_pickup_state(initial_down, pointer_id);
            } else {
                self.current_drag_state.set(Some(State::AwaitTouchSlop {
                    initial_down,
                    pointer_id,
                    verify_consumption_in_final_pass: false,
                }));
            }
        }
    }

    fn process_await_gesture_pickup(
        &self,
        event: &IndirectPointerEvent,
        pass: PointerEventPass,
        initial_down: ChangeSnapshot,
        pointer_id: PointerId,
    ) {
        if pass != PointerEventPass::Final {
            return;
        }
        let has_unconsumed_drag = event.changes.iter().all(|c| !c.is_consumed());
        let has_down_pointers = event.changes.iter().any(|c| c.pressed);
        if !has_down_pointers || event.changes.is_empty() {
            self.move_to_await_down_state();
        } else if has_unconsumed_drag {
            let orientation = self.orientation();
            let axis = event.primary_directional_motion_axis;
            let initial_position_change = primary_axis(event.changes[0].position, orientation, axis)
                - primary_axis(initial_down.position, orientation, axis);
            self.move_to_await_touch_slop_state(initial_down, pointer_id, initial_position_change);
        }
    }

    fn process_dragging_state(&self, event: &IndirectPointerEvent, pass: PointerEventPass, pointer_id: PointerId) {
        if pass != PointerEventPass::Main {
            return;
        }
        let Some(drag_event) = event.changes.iter().find(|c| c.id == pointer_id) else { return };
        let axis = event.primary_directional_motion_axis;
        if drag_event.previous_pressed && !drag_event.pressed {
            match event.changes.iter().find(|c| c.pressed) {
                None => {
                    if !drag_event.is_consumed() {
                        self.send_drag_stopped(ChangeSnapshot::of(drag_event), axis);
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
            let orientation = self.orientation();
            let motion_change = position_change_internal(drag_event, orientation, axis, true).get_distance();
            if motion_change != 0.0 {
                let position_change = position_change_internal(drag_event, orientation, axis, false);
                self.send_drag_event(ChangeSnapshot::of(drag_event), axis, position_change);
                drag_event.consume();
            }
        }
    }

    fn add_to_velocity_tracker(&self, change: ChangeSnapshot, axis: IndirectPointerEventPrimaryDirectionalMotionAxis) {
        let orientation = self.orientation();
        let smoothed = primary_axis(self.touch_smooth.borrow_mut().smooth_event_position(change), orientation, axis);
        if let Some(t) = self.velocity_tracker.borrow_mut().as_mut() {
            t.add_position(change.uptime_millis, smoothed);
        }
    }

    fn send_drag_start(
        &self,
        down: ChangeSnapshot,
        slop_trigger_change: ChangeSnapshot,
        axis: IndirectPointerEventPrimaryDirectionalMotionAxis,
        over_slop_offset: Offset,
    ) {
        if self.velocity_tracker.borrow().is_none() {
            *self.velocity_tracker.borrow_mut() = Some(VelocityTracker::new());
        }
        self.add_to_velocity_tracker(down, axis);
        let orientation = self.orientation();
        let drag_started_offset = primary_axis(slop_trigger_change.position, orientation, axis) - over_slop_offset;
        if let Some(node) = self.node.upgrade()
            && node.can_drag(PointerType::Touch)
        {
            node.on_drag_event(DragEvent::DragStarted { start_point: drag_started_offset });
        }
        self.offset_smoother.borrow_mut().reset();
    }

    fn send_drag_event(
        &self,
        change: ChangeSnapshot,
        axis: IndirectPointerEventPrimaryDirectionalMotionAxis,
        drag_amount: Offset,
    ) {
        let Some(orientation) = self.orientation() else { return };
        let along = if orientation == Orientation::Vertical { drag_amount.y } else { drag_amount.x };
        if along.abs() > PixelSensibility {
            self.add_to_velocity_tracker(change, axis);
            let smoothed = self.offset_smoother.borrow_mut().smooth_event_position(drag_amount);
            if let Some(node) = self.node.upgrade() {
                node.on_drag_event(DragEvent::DragDelta(DragDelta {
                    delta: smoothed,
                    is_indirect_pointer_event: true,
                }));
            }
        }
    }

    fn send_drag_stopped(&self, change: ChangeSnapshot, axis: IndirectPointerEventPrimaryDirectionalMotionAxis) {
        self.add_to_velocity_tracker(change, axis);
        let Some(node) = self.node.upgrade() else { return };
        let maximum_velocity = node.view_configuration().maximum_fling_velocity;
        let velocity = {
            let mut tracker = self.velocity_tracker.borrow_mut();
            let t = tracker.get_or_insert_with(VelocityTracker::new);
            let v = t.calculate_velocity_max(Velocity::new(maximum_velocity, maximum_velocity));
            t.reset_tracking();
            v
        };
        node.on_drag_event(DragEvent::DragStopped(DragStopped {
            velocity: to_valid_velocity(velocity),
            is_indirect_pointer_event: true,
        }));
    }

    fn send_drag_cancelled(&self) {
        if let Some(node) = self.node.upgrade() {
            node.on_drag_event(DragEvent::DragCancelled);
        }
    }
}

impl GestureConnection for IndirectPointerInputDragCycleDetector {
    fn gesture_state(&self) -> GestureState {
        match self.current_drag_state.get() {
            Some(State::AwaitDown { has_seen_initial_event, .. }) => {
                if has_seen_initial_event {
                    GestureState::Waiting
                } else {
                    GestureState::Idle
                }
            }
            Some(State::AwaitTouchSlop { .. }) | Some(State::AwaitGesturePickup { .. }) => GestureState::Waiting,
            Some(State::Dragging { .. }) => GestureState::Recognized,
            None => GestureState::Idle,
        }
    }

    fn as_draggable(&self) -> Option<&dyn DraggableGestureConnection> {
        Some(self)
    }
}

impl DraggableGestureConnection for IndirectPointerInputDragCycleDetector {
    fn orientation(&self) -> Option<Orientation> {
        IndirectPointerInputDragCycleDetector::orientation(self)
    }
}

/// A change's move along the device's main axis put on the drag's (Compose's `positionChangeInternal`).
fn position_change_internal(
    change: &IndirectPointerInputChange,
    orientation: Option<Orientation>,
    axis: IndirectPointerEventPrimaryDirectionalMotionAxis,
    ignore_consumed: bool,
) -> Offset {
    let offset =
        primary_axis(change.position, orientation, axis) - primary_axis(change.previous_position, orientation, axis);
    if !ignore_consumed && change.is_consumed() { Offset::Zero } else { offset }
}

/// A position on the device put on the drag's axis: the device's main axis' coordinate along it (Compose's
/// `primaryAxisPosition`); as it is for a drag of no axis or a device of none.
fn primary_axis(
    position: Offset,
    orientation: Option<Orientation>,
    axis: IndirectPointerEventPrimaryDirectionalMotionAxis,
) -> Offset {
    let Some(orientation) = orientation else { return position };
    let delta = match axis {
        IndirectPointerEventPrimaryDirectionalMotionAxis::X => position.x,
        IndirectPointerEventPrimaryDirectionalMotionAxis::Y => position.y,
        IndirectPointerEventPrimaryDirectionalMotionAxis::None => return position,
    };
    if orientation == Orientation::Horizontal { Offset::new(delta, 0.0) } else { Offset::new(0.0, delta) }
}

/// How many events an indirect pointer's position and moves are averaged over (Compose's SmoothingFactor).
const SmoothingFactor: usize = 3;

/// An indirect pointer's position averaged over its last moves (Compose's IndirectPointerInputEventSmoother).
#[derive(Default)]
struct IndirectPointerInputEventSmoother {
    event_rotating_index: usize,
    event_rotating_array: Vec<Offset>,
}

impl IndirectPointerInputEventSmoother {
    fn smooth_event_position(&mut self, change: ChangeSnapshot) -> Offset {
        let mut position = change.position;
        if change.changed_to_down_ignore_consumed() {
            self.event_rotating_index = 0;
            self.event_rotating_array.clear();
        }
        if !change.changed_to_up_ignore_consumed() && !change.changed_to_down_ignore_consumed() {
            if self.event_rotating_array.len() == SmoothingFactor {
                self.event_rotating_array[self.event_rotating_index] = change.position;
                self.event_rotating_index += 1;
            } else {
                self.event_rotating_array.push(change.position);
            }
            if self.event_rotating_index == SmoothingFactor {
                self.event_rotating_index = 0;
            }
            position = average(&self.event_rotating_array);
        }
        position
    }
}

/// Moves averaged over the last few (Compose's OffsetSmoother).
#[derive(Default)]
struct OffsetSmoother {
    event_rotating_index: usize,
    event_rotating_array: Vec<Offset>,
}

impl OffsetSmoother {
    fn smooth_event_position(&mut self, offset: Offset) -> Offset {
        if self.event_rotating_array.len() == SmoothingFactor {
            self.event_rotating_array[self.event_rotating_index] = offset;
            self.event_rotating_index += 1;
        } else {
            self.event_rotating_array.push(offset);
        }
        if self.event_rotating_index == SmoothingFactor {
            self.event_rotating_index = 0;
        }
        average(&self.event_rotating_array)
    }

    fn reset(&mut self) {
        self.event_rotating_index = 0;
        self.event_rotating_array.clear();
    }
}

/// The average of each axis (Compose's `averageBy` of each).
fn average(points: &[Offset]) -> Offset {
    let n = points.len() as f32;
    let (mut x, mut y) = (0.0f32, 0.0f32);
    for p in points {
        x += p.x;
        y += p.y;
    }
    Offset::new(x / n, y / n)
}

//! Material's copy of foundation's touch slop detection, as Compose's internal DragGestureDetectorCopy.kt ("Copy-paste
//! version of DragGestureDetector.kt"): waiting for a pointer to move past the slop across (the slider's thumb, the
//! time picker's clock), the slop a mouse's fraction of a finger's. Its algorithm is the one foundation had when it
//! was copied: the moves along the axis added up until past the slop, then the change offered with what went past.

use kompoz_graphics::{Dp, Offset};
use kompoz_ui::input::{
    AwaitPointerEventScope, PointerEvent, PointerEventPass, PointerId, PointerInputChange, PointerType,
};
use kompoz_ui::platform::ViewConfiguration;

/// Waits for the pointer `pointer_id` to move past the slop across: [`await_pointer_slop_or_cancellation`] along x
/// (Compose's internal `awaitHorizontalPointerSlopOrCancellation`).
pub(crate) async fn await_horizontal_pointer_slop_or_cancellation(
    scope: &AwaitPointerEventScope,
    pointer_id: PointerId,
    pointer_type: PointerType,
    on_pointer_slop_reached: impl Fn(&PointerInputChange, f32),
) -> Option<PointerInputChange> {
    await_pointer_slop_or_cancellation(scope, pointer_id, pointer_type, on_pointer_slop_reached, |it| it.x).await
}

/// Waits for drag motion along one axis based on `get_drag_direction_value` to pass pointer slop, using
/// `pointer_id` as the pointer to examine (Compose's private `awaitPointerSlopOrCancellation`). If `pointer_id` is
/// raised, another pointer from those that are down will be chosen to lead the gesture, and if none are down, `None`
/// is returned. If `pointer_id` is not down when this is called, then `None` is returned.
///
/// When pointer slop is detected, `on_pointer_slop_reached` is called with the change and the distance beyond the
/// pointer slop. If it does not consume the position change, pointer slop will not have been considered detected
/// and the detection will continue or, if it is consumed, the change that was consumed will be returned. `None` if
/// all pointers are raised or the position change was consumed by another gesture detector.
async fn await_pointer_slop_or_cancellation(
    scope: &AwaitPointerEventScope,
    pointer_id: PointerId,
    pointer_type: PointerType,
    on_pointer_slop_reached: impl Fn(&PointerInputChange, f32),
    get_drag_direction_value: impl Fn(Offset) -> f32,
) -> Option<PointerInputChange> {
    if is_pointer_up(&scope.current_event(), pointer_id) {
        return None; // The pointer has already been lifted, so the gesture is canceled
    }
    let touch_slop = pointer_slop(&scope.view_configuration(), pointer_type);
    let mut pointer = pointer_id;
    let mut total_position_change = 0.0f32;

    loop {
        let event = scope.await_pointer_event().await;
        // (Kotlin's `!!`: the pointer followed is in each event)
        let drag_event =
            event.changes.iter().find(|c| c.id == pointer).cloned().expect("the pointer followed is in the event");
        if drag_event.is_consumed() {
            return None;
        } else if drag_event.changed_to_up_ignore_consumed() {
            match event.changes.iter().find(|c| c.pressed) {
                // This is the last "up"
                None => return None,
                Some(other_down) => pointer = other_down.id,
            }
        } else {
            let current_position = drag_event.position;
            let previous_position = drag_event.previous_position;
            let position_change =
                get_drag_direction_value(current_position) - get_drag_direction_value(previous_position);
            total_position_change += position_change;

            let in_direction = total_position_change.abs();
            if in_direction < touch_slop {
                // verify that nothing else consumed the drag event
                scope.await_pointer_event_with(PointerEventPass::Final).await;
                if drag_event.is_consumed() {
                    return None;
                }
            } else {
                on_pointer_slop_reached(
                    &drag_event,
                    total_position_change - (kotlin_sign(total_position_change) * touch_slop),
                );
                if drag_event.is_consumed() {
                    return Some(drag_event);
                } else {
                    total_position_change = 0.0;
                }
            }
        }
    }
}

/// Kotlin's `sign` of a float: -1, 0 or 1 (NaN for NaN).
fn kotlin_sign(value: f32) -> f32 {
    if value.is_nan() || value == 0.0 { value } else { value.signum() }
}

/// Whether the pointer isn't pressed in `event` (Compose's private `PointerEvent.isPointerUp`).
fn is_pointer_up(event: &PointerEvent, pointer_id: PointerId) -> bool {
    event.changes.iter().find(|c| c.id == pointer_id).is_none_or(|c| !c.pressed)
}

/// A mouse's slop (Compose's private `mouseSlop`).
const mouseSlop: Dp = Dp(0.125);
/// The default touch slop on Android devices (Compose's private `defaultTouchSlop`).
const defaultTouchSlop: Dp = Dp(18.0);
/// What a mouse's slop is of a finger's (Compose's private `mouseToTouchSlopRatio`).
const mouseToTouchSlopRatio: f32 = mouseSlop.0 / defaultTouchSlop.0;

/// How far a pointer of `pointer_type` moves before it drags (Compose's internal `ViewConfiguration.pointerSlop`): a
/// mouse's a fraction of a finger's.
pub(crate) fn pointer_slop(view_configuration: &ViewConfiguration, pointer_type: PointerType) -> f32 {
    match pointer_type {
        PointerType::Mouse => view_configuration.touch_slop * mouseToTouchSlopRatio,
        _ => view_configuration.touch_slop,
    }
}

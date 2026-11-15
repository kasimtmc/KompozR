//! Pans, zooms and rotations, as Compose's `TransformGestureDetector.kt`: `detect_transform_gestures` and what the
//! pointers of an event did together.

use super::PointerEventTransforms;
use super::for_each::await_each_gesture;
use super::tap::await_first_down;
use kompoz_graphics::Offset;
use kompoz_ui::input::{PointerEvent, PointerEventPass, PointerInputChange, PointerInputScope};
use std::f32::consts::PI;
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::rc::Rc;

/// Compose's `calculateCentroid` over the changes `matches` takes.
pub(crate) fn calculate_centroid(
    event: &PointerEvent,
    use_current: bool,
    matches: impl Fn(&PointerInputChange) -> bool,
) -> Offset {
    let mut centroid = Offset::Zero;
    let mut weight = 0;
    for c in event.changes.iter().filter(|c| matches(c)) {
        centroid = centroid + if use_current { c.position } else { c.previous_position };
        weight += 1;
    }
    if weight == 0 { Offset::Unspecified } else { centroid / weight as f32 }
}

/// Compose's `calculateCentroidSize`.
pub(crate) fn calculate_centroid_size(event: &PointerEvent, use_current: bool) -> f32 {
    let centroid = event.calculate_centroid(use_current);
    if centroid.is_unspecified() {
        return 0.0;
    }
    let mut distance = 0.0;
    let mut weight = 0;
    for c in event.changes.iter().filter(|c| c.pressed && c.previous_pressed) {
        let position = if use_current { c.position } else { c.previous_position };
        distance += (position - centroid).get_distance();
        weight += 1;
    }
    distance / weight as f32
}

/// Compose's `calculatePan`.
pub(crate) fn calculate_pan(event: &PointerEvent) -> Offset {
    let current = event.calculate_centroid(true);
    if current.is_unspecified() {
        return Offset::Zero;
    }
    current - event.calculate_centroid(false)
}

/// Compose's `calculateZoom`.
pub(crate) fn calculate_zoom(event: &PointerEvent) -> f32 {
    let current = event.calculate_centroid_size(true);
    let previous = event.calculate_centroid_size(false);
    if current == 0.0 || previous == 0.0 { 1.0 } else { current / previous }
}

/// Compose's angle of an offset: degrees, 0 straight down, clockwise on screen.
fn angle(o: Offset) -> f32 {
    if o.x == 0.0 && o.y == 0.0 { 0.0 } else { -o.x.atan2(o.y) * 180.0 / PI }
}

/// Compose's `calculateRotation`: the pointers' turns about the centroid, weighted by their distance from it.
pub(crate) fn calculate_rotation(event: &PointerEvent) -> f32 {
    let count = event.changes.iter().filter(|c| c.previous_pressed && c.pressed).count();
    if count < 2 {
        return 0.0;
    }
    let current_centroid = event.calculate_centroid(true);
    let previous_centroid = event.calculate_centroid(false);
    let mut rotation = 0.0;
    let mut rotation_weight = 0.0;
    for c in event.changes.iter().filter(|c| c.pressed && c.previous_pressed) {
        let previous_offset = c.previous_position - previous_centroid;
        let current_offset = c.position - current_centroid;
        let angle_diff = angle(current_offset) - angle(previous_offset);
        let weight = (current_offset + previous_offset).get_distance() / 2.0;
        let diff = if angle_diff > 180.0 {
            angle_diff - 360.0
        } else if angle_diff < -180.0 {
            angle_diff + 360.0
        } else {
            angle_diff
        };
        rotation += diff * weight;
        rotation_weight += weight;
    }
    if rotation_weight == 0.0 { 0.0 } else { rotation / rotation_weight }
}

/// `detect_transform_gestures`'s settings, then awaited (Compose's `detectTransformGestures(panZoomLock, …)`).
pub struct DetectTransformGestures {
    scope: PointerInputScope,
    pan_zoom_lock: bool,
    on_gesture: Rc<dyn Fn(Offset, Offset, f32, f32)>,
}

impl DetectTransformGestures {
    pub(crate) fn new(scope: PointerInputScope, on_gesture: impl Fn(Offset, Offset, f32, f32) + 'static) -> Self {
        DetectTransformGestures { scope, pan_zoom_lock: false, on_gesture: Rc::new(on_gesture) }
    }

    /// A gesture that starts as a pan or a zoom (not turning past the slop) doesn't rotate.
    pub fn pan_zoom_lock(mut self, pan_zoom_lock: bool) -> Self {
        self.pan_zoom_lock = pan_zoom_lock;
        self
    }
}

impl IntoFuture for DetectTransformGestures {
    type Output = ();
    type IntoFuture = Pin<Box<dyn Future<Output = ()>>>;

    fn into_future(self) -> Self::IntoFuture {
        let DetectTransformGestures { scope, pan_zoom_lock, on_gesture } = self;
        Box::pin(async move {
            await_each_gesture(&scope, move |s| {
                let on_gesture = on_gesture.clone();
                async move {
                    let mut rotation = 0.0f32;
                    let mut zoom = 1.0f32;
                    let mut pan = Offset::Zero;
                    let mut past_touch_slop = false;
                    let touch_slop = s.view_configuration().touch_slop;
                    let mut locked_to_pan_zoom = false;
                    await_first_down(&s, false, PointerEventPass::Main).await;
                    loop {
                        let event = s.await_pointer_event().await;
                        let canceled = event.changes.iter().any(|c| c.is_consumed());
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
                                if zoom_motion > touch_slop || rotation_motion > touch_slop || pan_motion > touch_slop {
                                    past_touch_slop = true;
                                    locked_to_pan_zoom = pan_zoom_lock && rotation_motion < touch_slop;
                                }
                            }
                            if past_touch_slop {
                                let centroid = event.calculate_centroid(false);
                                let effective_rotation = if locked_to_pan_zoom { 0.0 } else { rotation_change };
                                if effective_rotation != 0.0 || zoom_change != 1.0 || pan_change != Offset::Zero {
                                    on_gesture(centroid, pan_change, zoom_change, effective_rotation);
                                }
                                event.changes.iter().filter(|c| c.position_changed()).for_each(|c| c.consume());
                            }
                        }
                        if canceled || !event.changes.iter().any(|c| c.pressed) {
                            break;
                        }
                    }
                }
            })
            .await
        })
    }
}

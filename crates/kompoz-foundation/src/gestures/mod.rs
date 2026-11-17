//! Gesture detection, as Compose's `androidx.compose.foundation.gestures` detectors: built on `pointer_input`'s
//! scopes, taps (single, double, long, presses), drags (after the touch slop, after a long press, along one axis),
//! transforms (pan, zoom, rotation) and the blocks they are made of (`await_first_down`,
//! `wait_for_up_or_cancellation`, `await_each_gesture`, `await_touch_slop_or_cancellation`, `drag`, …).
//!
//! Compose's are extension functions of `PointerInputScope`, `AwaitPointerEventScope` and `PointerEvent`; here
//! they are methods of the extension traits [`PointerInputScopeGestures`], [`AwaitPointerEventScopeGestures`] and
//! [`PointerEventTransforms`] (in the prelude). A detector's optional callbacks are set on what it returns, which
//! is then awaited:
//!
//! ```no_run
//! # use kompoz_foundation::prelude::*;
//! # use kompoz_ui::{Modifier, input::PointerInputScope};
//! Modifier.pointer_input((), |scope: PointerInputScope| async move {
//!     scope.detect_tap_gestures().on_tap(|at| println!("tap at {at:?}")).on_double_tap(|_| println!("double")).await
//! });
//! ```

pub(crate) mod bring_into_view_spec;
mod content_in_view;
mod drag;
pub(crate) mod draggable;
pub(crate) mod fling;
mod for_each;
mod indirect_drag;
mod mouse_wheel;
pub(crate) mod scrollable;
pub(crate) mod scrollable_state;
mod tap;
mod transform;

pub use bring_into_view_spec::{
    BringIntoViewSpec, BringIntoViewSpecEq, DefaultBringIntoViewSpec, LocalBringIntoViewSpec,
    default_calculate_scroll_distance,
};
pub use drag::{DetectAxisDragGestures, DetectDragGestures, DetectDragGesturesAfterLongPress};
pub(crate) use drag::{await_all_pointers_up_with_slop_detection, pointer_slop};
pub use draggable::{
    DefaultDraggableState, DragBlock, DragScope, DraggableModifiers, DraggableNode, DraggableState, DraggableStateEq,
    DraggableStateExtensions, OnDragStarted, OnDragStopped, remember_draggable_state,
};
pub use fling::{DefaultFlingBehavior, FlingBehavior, FlingBehaviorEq, ScrollableDefaultFlingBehavior};
pub use mouse_wheel::{DefaultScrollConfig, LocalScrollConfig, ScrollConfig};
pub use scrollable::{NestedScrollScope, ScrollableDefaults, ScrollableModifiers, ScrollableNode};
pub use scrollable_state::{
    DefaultScrollableState, LocalFuture, ScrollBlock, ScrollIndicatorState, ScrollScope, ScrollableState,
    ScrollableStateEq, ScrollableStateExtensions, remember_scrollable_state,
};
pub use tap::{DetectTapGestures, GestureCancellation, PressGestureScope};
pub(crate) use tap::{
    FIRST_DOWN_REFERS_TO_PRIMARY_MOUSE_BUTTON_ONLY, LongPressResult, is_changed_to_down, wait_for_long_press,
};
pub use transform::DetectTransformGestures;

use kompoz_graphics::Offset;
use kompoz_ui::input::{
    AwaitPointerEventScope, PointerEvent, PointerEventPass, PointerId, PointerInputChange, PointerInputScope,
    PointerType,
};
use std::future::Future;

/// Which way something scrolls or drags (Compose's Orientation).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

/// The gesture detectors of a pointer input block (Compose's `PointerInputScope` extensions).
pub trait PointerInputScopeGestures {
    /// Runs `block` for each gesture, from its first down until all pointers are up again — the rest of a
    /// gesture `block` gives up on is waited out (Compose's `awaitEachGesture`). It never ends; the block's
    /// coroutine is cancelled with the pointer input.
    fn await_each_gesture<F, Fut>(&self, block: F) -> impl Future<Output = ()>
    where
        F: Fn(AwaitPointerEventScope) -> Fut,
        Fut: Future<Output = ()>;

    /// Taps, double taps, long presses and presses (Compose's `detectTapGestures`): set what to call on what
    /// this gives, then await it. Without a double tap callback a tap is reported at once; with one, after the
    /// double tap timeout. Positions are local to the layout.
    fn detect_tap_gestures(&self) -> DetectTapGestures;

    /// Drags past the touch slop (Compose's `detectDragGestures`): `on_drag` gets each move (the first one less
    /// the slop) with the change, which it should consume.
    fn detect_drag_gestures(&self, on_drag: impl Fn(&PointerInputChange, Offset) + 'static) -> DetectDragGestures;

    /// Drags after a long press, whole (no slop taken off: Compose's `detectDragGesturesAfterLongPress`).
    fn detect_drag_gestures_after_long_press(
        &self,
        on_drag: impl Fn(&PointerInputChange, Offset) + 'static,
    ) -> DetectDragGesturesAfterLongPress;

    /// Vertical drags past the touch slop (Compose's `detectVerticalDragGestures`).
    fn detect_vertical_drag_gestures(
        &self,
        on_vertical_drag: impl Fn(&PointerInputChange, f32) + 'static,
    ) -> DetectAxisDragGestures;

    /// Horizontal drags past the touch slop (Compose's `detectHorizontalDragGestures`).
    fn detect_horizontal_drag_gestures(
        &self,
        on_horizontal_drag: impl Fn(&PointerInputChange, f32) + 'static,
    ) -> DetectAxisDragGestures;

    /// Pans, zooms and rotations past the touch slop (Compose's `detectTransformGestures`): `on_gesture` gets
    /// the centroid, the pan, the zoom (a factor) and the rotation (degrees, clockwise) of each event.
    fn detect_transform_gestures(
        &self,
        on_gesture: impl Fn(Offset, Offset, f32, f32) + 'static,
    ) -> DetectTransformGestures;
}

impl PointerInputScopeGestures for PointerInputScope {
    fn await_each_gesture<F, Fut>(&self, block: F) -> impl Future<Output = ()>
    where
        F: Fn(AwaitPointerEventScope) -> Fut,
        Fut: Future<Output = ()>,
    {
        for_each::await_each_gesture(self, block)
    }

    fn detect_tap_gestures(&self) -> DetectTapGestures {
        DetectTapGestures::new(self.clone())
    }

    fn detect_drag_gestures(&self, on_drag: impl Fn(&PointerInputChange, Offset) + 'static) -> DetectDragGestures {
        DetectDragGestures::new(self.clone(), on_drag)
    }

    fn detect_drag_gestures_after_long_press(
        &self,
        on_drag: impl Fn(&PointerInputChange, Offset) + 'static,
    ) -> DetectDragGesturesAfterLongPress {
        DetectDragGesturesAfterLongPress::new(self.clone(), on_drag)
    }

    fn detect_vertical_drag_gestures(
        &self,
        on_vertical_drag: impl Fn(&PointerInputChange, f32) + 'static,
    ) -> DetectAxisDragGestures {
        DetectAxisDragGestures::new(self.clone(), Orientation::Vertical, on_vertical_drag)
    }

    fn detect_horizontal_drag_gestures(
        &self,
        on_horizontal_drag: impl Fn(&PointerInputChange, f32) + 'static,
    ) -> DetectAxisDragGestures {
        DetectAxisDragGestures::new(self.clone(), Orientation::Horizontal, on_horizontal_drag)
    }

    fn detect_transform_gestures(
        &self,
        on_gesture: impl Fn(Offset, Offset, f32, f32) + 'static,
    ) -> DetectTransformGestures {
        DetectTransformGestures::new(self.clone(), on_gesture)
    }
}

/// The gesture blocks of an [`AwaitPointerEventScope`] (Compose's `AwaitPointerEventScope` extensions).
pub trait AwaitPointerEventScopeGestures {
    /// The first press of a gesture not consumed (Compose's `awaitFirstDown()`).
    fn await_first_down(&self) -> impl Future<Output = PointerInputChange>;
    /// The first press, consumed ones too unless `require_unconsumed`, in `pass`.
    fn await_first_down_with(
        &self,
        require_unconsumed: bool,
        pass: PointerEventPass,
    ) -> impl Future<Output = PointerInputChange>;

    /// The release of all pointers, or none if the gesture was cancelled: consumed (in any pass) or out of
    /// bounds (Compose's `waitForUpOrCancellation()`).
    fn wait_for_up_or_cancellation(&self) -> impl Future<Output = Option<PointerInputChange>>;
    fn wait_for_up_or_cancellation_with(
        &self,
        pass: PointerEventPass,
    ) -> impl Future<Output = Option<PointerInputChange>>;

    /// Waits until the pointer moved past the touch slop; `on_touch_slop_reached` gets that change and how far
    /// past the slop it went, and consumes it to take the drag: that change, or none if it was consumed by
    /// another or lifted (Compose's `awaitTouchSlopOrCancellation`).
    fn await_touch_slop_or_cancellation(
        &self,
        pointer_id: PointerId,
        on_touch_slop_reached: impl Fn(&PointerInputChange, Offset),
    ) -> impl Future<Output = Option<PointerInputChange>>;
    /// The same vertically (Compose's `awaitVerticalTouchSlopOrCancellation`).
    fn await_vertical_touch_slop_or_cancellation(
        &self,
        pointer_id: PointerId,
        on_touch_slop_reached: impl Fn(&PointerInputChange, f32),
    ) -> impl Future<Output = Option<PointerInputChange>>;
    /// The same horizontally.
    fn await_horizontal_touch_slop_or_cancellation(
        &self,
        pointer_id: PointerId,
        on_touch_slop_reached: impl Fn(&PointerInputChange, f32),
    ) -> impl Future<Output = Option<PointerInputChange>>;
    /// The vertical one with the slop of a pointer of `pointer_type` (a mouse's is smaller: Compose's
    /// `awaitVerticalPointerSlopOrCancellation`).
    fn await_vertical_pointer_slop_or_cancellation(
        &self,
        pointer_id: PointerId,
        pointer_type: PointerType,
        on_pointer_slop_reached: impl Fn(&PointerInputChange, f32),
    ) -> impl Future<Output = Option<PointerInputChange>>;
    /// The horizontal one with the slop of a pointer of `pointer_type`.
    fn await_horizontal_pointer_slop_or_cancellation(
        &self,
        pointer_id: PointerId,
        pointer_type: PointerType,
        on_pointer_slop_reached: impl Fn(&PointerInputChange, f32),
    ) -> impl Future<Output = Option<PointerInputChange>>;

    /// The next move of the pointer (another one down if it lifts), none if it was consumed or all are up
    /// (Compose's `awaitDragOrCancellation`).
    fn await_drag_or_cancellation(&self, pointer_id: PointerId) -> impl Future<Output = Option<PointerInputChange>>;
    fn await_vertical_drag_or_cancellation(
        &self,
        pointer_id: PointerId,
    ) -> impl Future<Output = Option<PointerInputChange>>;
    fn await_horizontal_drag_or_cancellation(
        &self,
        pointer_id: PointerId,
    ) -> impl Future<Output = Option<PointerInputChange>>;

    /// Follows the drag until all pointers are up (true) or a move was consumed (false), `on_drag` getting each
    /// move (Compose's `drag`).
    fn drag(&self, pointer_id: PointerId, on_drag: impl Fn(&PointerInputChange)) -> impl Future<Output = bool>;
    fn vertical_drag(&self, pointer_id: PointerId, on_drag: impl Fn(&PointerInputChange))
    -> impl Future<Output = bool>;
    fn horizontal_drag(
        &self,
        pointer_id: PointerId,
        on_drag: impl Fn(&PointerInputChange),
    ) -> impl Future<Output = bool>;

    /// The pointer held for the long press timeout: the change then, or none if it moved out, was consumed or
    /// lifted first (Compose's `awaitLongPressOrCancellation`).
    fn await_long_press_or_cancellation(
        &self,
        pointer_id: PointerId,
    ) -> impl Future<Output = Option<PointerInputChange>>;
}

impl AwaitPointerEventScopeGestures for AwaitPointerEventScope {
    fn await_first_down(&self) -> impl Future<Output = PointerInputChange> {
        tap::await_first_down(self, true, PointerEventPass::Main)
    }
    fn await_first_down_with(
        &self,
        require_unconsumed: bool,
        pass: PointerEventPass,
    ) -> impl Future<Output = PointerInputChange> {
        tap::await_first_down(self, require_unconsumed, pass)
    }
    fn wait_for_up_or_cancellation(&self) -> impl Future<Output = Option<PointerInputChange>> {
        tap::wait_for_up_or_cancellation(self, PointerEventPass::Main)
    }
    fn wait_for_up_or_cancellation_with(
        &self,
        pass: PointerEventPass,
    ) -> impl Future<Output = Option<PointerInputChange>> {
        tap::wait_for_up_or_cancellation(self, pass)
    }
    fn await_touch_slop_or_cancellation(
        &self,
        pointer_id: PointerId,
        on_touch_slop_reached: impl Fn(&PointerInputChange, Offset),
    ) -> impl Future<Output = Option<PointerInputChange>> {
        drag::await_pointer_slop_or_cancellation(
            self,
            pointer_id,
            PointerType::Touch,
            None,
            Offset::Zero,
            on_touch_slop_reached,
        )
    }
    fn await_vertical_touch_slop_or_cancellation(
        &self,
        pointer_id: PointerId,
        on_touch_slop_reached: impl Fn(&PointerInputChange, f32),
    ) -> impl Future<Output = Option<PointerInputChange>> {
        drag::await_pointer_slop_or_cancellation(
            self,
            pointer_id,
            PointerType::Touch,
            Some(Orientation::Vertical),
            Offset::Zero,
            move |c, over| on_touch_slop_reached(c, over.y),
        )
    }
    fn await_horizontal_touch_slop_or_cancellation(
        &self,
        pointer_id: PointerId,
        on_touch_slop_reached: impl Fn(&PointerInputChange, f32),
    ) -> impl Future<Output = Option<PointerInputChange>> {
        drag::await_pointer_slop_or_cancellation(
            self,
            pointer_id,
            PointerType::Touch,
            Some(Orientation::Horizontal),
            Offset::Zero,
            move |c, over| on_touch_slop_reached(c, over.x),
        )
    }
    fn await_vertical_pointer_slop_or_cancellation(
        &self,
        pointer_id: PointerId,
        pointer_type: PointerType,
        on_pointer_slop_reached: impl Fn(&PointerInputChange, f32),
    ) -> impl Future<Output = Option<PointerInputChange>> {
        drag::await_pointer_slop_or_cancellation(
            self,
            pointer_id,
            pointer_type,
            Some(Orientation::Vertical),
            Offset::Zero,
            move |c, over| on_pointer_slop_reached(c, over.y),
        )
    }
    fn await_horizontal_pointer_slop_or_cancellation(
        &self,
        pointer_id: PointerId,
        pointer_type: PointerType,
        on_pointer_slop_reached: impl Fn(&PointerInputChange, f32),
    ) -> impl Future<Output = Option<PointerInputChange>> {
        drag::await_pointer_slop_or_cancellation(
            self,
            pointer_id,
            pointer_type,
            Some(Orientation::Horizontal),
            Offset::Zero,
            move |c, over| on_pointer_slop_reached(c, over.x),
        )
    }
    fn await_drag_or_cancellation(&self, pointer_id: PointerId) -> impl Future<Output = Option<PointerInputChange>> {
        drag::await_drag_or_cancellation(self, pointer_id, None)
    }
    fn await_vertical_drag_or_cancellation(
        &self,
        pointer_id: PointerId,
    ) -> impl Future<Output = Option<PointerInputChange>> {
        drag::await_drag_or_cancellation(self, pointer_id, Some(Orientation::Vertical))
    }
    fn await_horizontal_drag_or_cancellation(
        &self,
        pointer_id: PointerId,
    ) -> impl Future<Output = Option<PointerInputChange>> {
        drag::await_drag_or_cancellation(self, pointer_id, Some(Orientation::Horizontal))
    }
    fn drag(&self, pointer_id: PointerId, on_drag: impl Fn(&PointerInputChange)) -> impl Future<Output = bool> {
        drag::drag_until_up(self, pointer_id, on_drag)
    }
    fn vertical_drag(
        &self,
        pointer_id: PointerId,
        on_drag: impl Fn(&PointerInputChange),
    ) -> impl Future<Output = bool> {
        drag::axis_drag(self, pointer_id, Orientation::Vertical, on_drag)
    }
    fn horizontal_drag(
        &self,
        pointer_id: PointerId,
        on_drag: impl Fn(&PointerInputChange),
    ) -> impl Future<Output = bool> {
        drag::axis_drag(self, pointer_id, Orientation::Horizontal, on_drag)
    }
    fn await_long_press_or_cancellation(
        &self,
        pointer_id: PointerId,
    ) -> impl Future<Output = Option<PointerInputChange>> {
        drag::await_long_press_or_cancellation(self, pointer_id)
    }
}

/// What the pointers of an event did together (Compose's `calculateCentroid`, `calculatePan`, `calculateZoom`,
/// `calculateRotation`, `calculateCentroidSize` on PointerEvent), over the pointers pressed before and after it.
pub trait PointerEventTransforms {
    /// The pointers' average position, now or before; `Offset::Unspecified` if none was pressed both times.
    fn calculate_centroid(&self, use_current: bool) -> Offset;
    /// Their average distance from the centroid, now or before.
    fn calculate_centroid_size(&self, use_current: bool) -> f32;
    /// How far the centroid moved.
    fn calculate_pan(&self) -> Offset;
    /// How much they spread (a factor: 1 for none).
    fn calculate_zoom(&self) -> f32;
    /// How far they turned about the centroid, degrees (clockwise on screen).
    fn calculate_rotation(&self) -> f32;
}

impl PointerEventTransforms for PointerEvent {
    fn calculate_centroid(&self, use_current: bool) -> Offset {
        transform::calculate_centroid(self, use_current, |c| c.pressed && c.previous_pressed)
    }
    fn calculate_centroid_size(&self, use_current: bool) -> f32 {
        transform::calculate_centroid_size(self, use_current)
    }
    fn calculate_pan(&self) -> Offset {
        transform::calculate_pan(self)
    }
    fn calculate_zoom(&self) -> f32 {
        transform::calculate_zoom(self)
    }
    fn calculate_rotation(&self) -> f32 {
        transform::calculate_rotation(self)
    }
}

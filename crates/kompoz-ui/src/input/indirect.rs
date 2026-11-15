//! Indirect pointer input, as Compose's `androidx.compose.ui.input.indirect`: pointers that don't point at the
//! window (a touchpad not over it, a remote's swipe surface) go to the focused node's indirect pointer input nodes,
//! in the three passes, rather than to what is under them.

use super::{PointerEventPass, PointerId};
use kompoz_graphics::Offset;
use std::cell::Cell;
use std::fmt;

/// What an indirect pointer event is (Compose's IndirectPointerEventType).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum IndirectPointerEventType {
    #[default]
    Unknown,
    Press,
    Release,
    Move,
}

/// The axis an indirect pointer's motion is mostly meant along, where the device says (Compose's
/// IndirectPointerEventPrimaryDirectionalMotionAxis): a remote's swipe surface that pages one way.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum IndirectPointerEventPrimaryDirectionalMotionAxis {
    #[default]
    None,
    X,
    Y,
}

/// One pointer of an indirect pointer event (Compose's IndirectPointerInputChange): where it is on its device and
/// was, pressed or not, its pressure; consumed by whoever takes it (shared by every node that sees the event).
pub struct IndirectPointerInputChange {
    pub id: PointerId,
    pub uptime_millis: i64,
    /// On the device's surface, in its own units.
    pub position: Offset,
    pub pressed: bool,
    pub pressure: f32,
    pub previous_uptime_millis: i64,
    pub previous_position: Offset,
    pub previous_pressed: bool,
    consumed: Cell<bool>,
}

impl IndirectPointerInputChange {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: PointerId,
        uptime_millis: i64,
        position: Offset,
        pressed: bool,
        pressure: f32,
        previous_uptime_millis: i64,
        previous_position: Offset,
        previous_pressed: bool,
    ) -> IndirectPointerInputChange {
        IndirectPointerInputChange {
            id,
            uptime_millis,
            position,
            pressed,
            pressure,
            previous_uptime_millis,
            previous_position,
            previous_pressed,
            consumed: Cell::new(false),
        }
    }

    pub fn is_consumed(&self) -> bool {
        self.consumed.get()
    }

    /// Taken: the nodes after see it consumed (Compose's `consume`).
    pub fn consume(&self) {
        self.consumed.set(true);
    }
}

impl fmt::Debug for IndirectPointerInputChange {
    /// Compose's toString.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "IndirectPointerInputChange(id={:?}, uptimeMillis={}, position={:?}, pressed={}, pressure={}, \
             previousUptimeMillis={}, previousPosition={:?}, previousPressed={}, isConsumed={})",
            self.id,
            self.uptime_millis,
            self.position,
            self.pressed,
            self.pressure,
            self.previous_uptime_millis,
            self.previous_position,
            self.previous_pressed,
            self.is_consumed()
        )
    }
}

/// An event of indirect pointers (Compose's IndirectPointerEvent): their changes, what it is, the device's main
/// axis.
#[derive(Debug)]
pub struct IndirectPointerEvent {
    pub changes: Vec<IndirectPointerInputChange>,
    pub r#type: IndirectPointerEventType,
    pub primary_directional_motion_axis: IndirectPointerEventPrimaryDirectionalMotionAxis,
}

impl IndirectPointerEvent {
    pub fn new(
        changes: Vec<IndirectPointerInputChange>,
        r#type: IndirectPointerEventType,
        primary_directional_motion_axis: IndirectPointerEventPrimaryDirectionalMotionAxis,
    ) -> IndirectPointerEvent {
        IndirectPointerEvent { changes, r#type, primary_directional_motion_axis }
    }
}

/// A node that takes indirect pointer events, along the focused path (Compose's IndirectPointerInputModifierNode).
pub trait IndirectPointerInputModifierNode {
    /// An event, in `pass`: the focused one's nodes first in the Initial pass from the root down, then in the Main
    /// pass from it up, then the Final pass down again (Compose's `onIndirectPointerEvent`).
    fn on_indirect_pointer_event(&mut self, event: &IndirectPointerEvent, pass: PointerEventPass);

    /// Its stream of events was cancelled: the focus left it, it left the chain, the platform called it off
    /// (Compose's `onCancelIndirectPointerInput`).
    fn on_cancel_indirect_pointer_input(&mut self) {}
}

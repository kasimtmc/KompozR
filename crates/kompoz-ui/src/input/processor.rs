//! Turning what the platform reports into dispatched pointer events, as Compose's `PointerInputEventProcessor`:
//! the changes worked out from the pointers' last positions, hit testing (for a hover, every event; else where a
//! pointer goes down), the hit paths' dispatch, and what the event set off (whether a node took it, a move was
//! consumed).

use super::PointerId;
use super::hit_path::HitPathTracker;
use super::hit_test::{HitTestResult, hit_test_layout_node};
use super::pointer::{InternalPointerEvent, PointerInputChange, PointerInputEvent};
use crate::node::LayoutNode;
use kompoz_graphics::Offset;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What processing an event came to (Compose's ProcessResult).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ProcessResult {
    /// It went to a pointer input node.
    pub dispatched_to_a_pointer_input_modifier: bool,
    /// A change that moved was consumed.
    pub any_movement_consumed: bool,
    /// A change was consumed.
    pub any_change_consumed: bool,
}

/// A pointer's last report, down (Compose's PointerInputChangeEventProducer.PointerInputData).
#[derive(Clone, Copy)]
struct PointerInputData {
    id: PointerId,
    uptime: i64,
    position: Offset,
    down: bool,
}

/// Compose's PointerInputEventProcessor (and its PointerInputChangeEventProducer).
#[derive(Default)]
pub(crate) struct PointerInputEventProcessor {
    pub(crate) tracker: HitPathTracker,
    previous: RefCell<Vec<PointerInputData>>,
    is_processing: Cell<bool>,
}

impl PointerInputEventProcessor {
    /// Processes an event whose positions are in the host's coordinates, the roots hit tested in turn (the
    /// topmost window first, the first one hit taking it); `in_bounds` false when the pointer left the window.
    /// An event while one is processed is dropped, as in Compose.
    pub(crate) fn process(&self, event: PointerInputEvent, roots: &[Rc<LayoutNode>], in_bounds: bool) -> ProcessResult {
        if self.is_processing.get() {
            return ProcessResult::default();
        }
        self.is_processing.set(true);
        /// Ends the processing however it ends.
        struct Processing<'a>(&'a Cell<bool>);
        impl Drop for Processing<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let _guard = Processing(&self.is_processing);
        let internal = Rc::new(self.produce(event));
        let is_hover = internal.changes.iter().all(|c| !c.pressed && !c.previous_pressed);
        let mut hit = HitTestResult::new();
        for change in &internal.changes {
            if is_hover || change.changed_to_down_ignore_consumed() {
                for root in roots {
                    hit_test_layout_node(root, change.position, &mut hit, change.r#type, true);
                    if !hit.is_empty() {
                        break;
                    }
                }
                if !hit.is_empty() {
                    self.tracker.add_hit_path(change.id, hit.nodes(), change.changed_to_down_ignore_consumed());
                    hit.clear();
                }
            }
        }
        let dispatched = self.tracker.dispatch_changes(&internal, in_bounds);
        let any_movement_consumed = !internal.suppress_movement_consumption.get()
            && internal.changes.iter().any(|c| c.position_changed_ignore_consumed() && c.is_consumed());
        let any_change_consumed = internal.changes.iter().any(|c| c.is_consumed());
        ProcessResult { dispatched_to_a_pointer_input_modifier: dispatched, any_movement_consumed, any_change_consumed }
    }

    /// Cancels what is under way: every node on a hit path told (Compose's `processCancel`).
    pub(crate) fn process_cancel(&self) {
        if !self.is_processing.get() {
            self.previous.borrow_mut().clear();
            self.tracker.process_cancel();
        }
    }

    /// The changes of an event from the pointers' last reports (Compose's `produce`).
    fn produce(&self, event: PointerInputEvent) -> InternalPointerEvent {
        let mut changes = Vec::with_capacity(event.pointers.len());
        {
            let mut previous = self.previous.borrow_mut();
            for p in &event.pointers {
                let (previous_time, previous_position, previous_down) = match previous.iter().find(|d| d.id == p.id) {
                    Some(d) => (d.uptime, d.position, d.down),
                    None => (p.uptime, p.position, false),
                };
                let mut change = PointerInputChange::new(
                    p.id,
                    p.uptime,
                    p.position,
                    p.down,
                    previous_time,
                    previous_position,
                    previous_down,
                    false,
                )
                .with_type(p.r#type)
                .with_historical(p.historical.clone());
                change.pressure = p.pressure;
                change.scroll_delta = p.scroll_delta;
                change.scale_factor = p.scale_factor;
                change.rotation = p.rotation;
                change.set_original_event_position(p.position);
                changes.push(change);
                previous.retain(|d| d.id != p.id);
                if p.down {
                    previous.push(PointerInputData { id: p.id, uptime: p.uptime, position: p.position, down: true });
                }
            }
        }
        changes.sort_by_key(|c| c.id);
        InternalPointerEvent { changes, event, suppress_movement_consumption: Cell::new(false) }
    }
}

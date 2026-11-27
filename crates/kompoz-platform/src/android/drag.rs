//! Drags over the view as KompozDragAndDrop tells them (its `View.OnDragListener`, C++'s KompozView drag events,
//! Compose's AndroidDragAndDropManager): the system's `DragEvent` actions as the app core's drop events — a drag
//! entering the view begins one (with the MIME types its ClipDescription lists), moving over it moves it, leaving
//! it ends it with nothing, a drop gives what it carries (read from its ClipData as a target asks: the platform's
//! entry) or, for the app's own drag back over its window, what the host already has; the end of the app's own drag
//! (one KompozDragAndDrop started for it) tells its source whether a target took it. Pure: tested on the host.

use crate::app::Event;
use kompoz_ui::draganddrop::DragAndDropTransferAction;

/// `DragEvent.ACTION_*`.
pub mod action {
    pub const STARTED: i32 = 1;
    pub const LOCATION: i32 = 2;
    pub const DROP: i32 = 3;
    pub const ENDED: i32 = 4;
    pub const ENTERED: i32 = 5;
    pub const EXITED: i32 = 6;
}

/// A drag event: its action, where (px in the view), the types it carries, whether it is the app's own drag (its
/// local state KompozDragAndDrop's), and at its end whether a target took it (`DragEvent.getResult`).
pub struct DragEvent {
    pub action: i32,
    pub x: f32,
    pub y: f32,
    pub mime_types: Vec<String>,
    pub own: bool,
    pub result: bool,
}

/// What the app core is told of it. A drag starting anywhere is accepted (the view hears where it goes; whether a
/// target takes it is the drop's answer); a drop of another app's is the platform's entry to read (`DropData`), of
/// the app's own the host's (`DropOwn`); the app's own drag ending tells its source the action a target took it
/// with — the first its data offers, Android's drags having none of their own — or none.
pub fn events(e: &DragEvent, offered: &[DragAndDropTransferAction]) -> Vec<Event> {
    let (x, y) = (e.x, e.y);
    match e.action {
        action::ENTERED => vec![Event::DropBegin { x, y, mime_types: e.mime_types.clone() }, Event::DropMove { x, y }],
        action::LOCATION => vec![Event::DropMove { x, y }],
        action::EXITED => vec![Event::DropComplete { x, y }],
        action::DROP if e.own => vec![Event::DropMove { x, y }, Event::DropOwn { x, y }, Event::DropComplete { x, y }],
        action::DROP => vec![Event::DropMove { x, y }, Event::DropData { x, y }, Event::DropComplete { x, y }],
        action::ENDED if e.own => {
            let action = e.result.then(|| offered.first().copied().unwrap_or(DragAndDropTransferAction::Copy));
            vec![Event::OwnDragEnded { action }]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drag(action: i32, own: bool, result: bool) -> DragEvent {
        DragEvent { action, x: 10.0, y: 20.0, mime_types: vec!["text/plain".into()], own, result }
    }

    #[test]
    fn another_apps_drag_enters_moves_and_drops() {
        assert!(events(&drag(action::STARTED, false, false), &[]).is_empty());
        let entered = events(&drag(action::ENTERED, false, false), &[]);
        assert_eq!(entered[0], Event::DropBegin { x: 10.0, y: 20.0, mime_types: vec!["text/plain".into()] });
        assert_eq!(events(&drag(action::LOCATION, false, false), &[]), vec![Event::DropMove { x: 10.0, y: 20.0 }]);
        let dropped = events(&drag(action::DROP, false, false), &[]);
        assert!(matches!(dropped[1], Event::DropData { .. }) && matches!(dropped[2], Event::DropComplete { .. }));
        assert_eq!(events(&drag(action::EXITED, false, false), &[]), vec![Event::DropComplete { x: 10.0, y: 20.0 }]);
        // (Another app's drag ending is nothing of the app's.)
        assert!(events(&drag(action::ENDED, false, true), &[]).is_empty());
    }

    #[test]
    fn the_apps_own_drag_comes_back_and_ends() {
        let dropped = events(&drag(action::DROP, true, false), &[]);
        assert!(matches!(dropped[1], Event::DropOwn { .. }));
        let moved = [DragAndDropTransferAction::Move, DragAndDropTransferAction::Copy];
        assert_eq!(
            events(&drag(action::ENDED, true, true), &moved),
            vec![Event::OwnDragEnded { action: Some(DragAndDropTransferAction::Move) }]
        );
        assert_eq!(events(&drag(action::ENDED, true, false), &moved), vec![Event::OwnDragEnded { action: None }]);
    }
}

//! What text's drag gestures tell (Compose's LongPressTextDragObserver.kt and CoreTextField's Handle).

use crate::gestures::{AwaitPointerEventScopeGestures, PointerInputScopeGestures};
use crate::text::selection::SelectionAdjustment;
use kompoz_coroutines::{CoroutineStart, coroutine_scope};
use kompoz_graphics::Offset;
use kompoz_runtime::named_args;
use kompoz_ui::input::{PointerEventPass, PointerInputScope};
use std::rc::Rc;

/// A handle of text selection (Compose's Handle): a text field's cursor, or an end of a selection.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Handle {
    Cursor,
    SelectionStart,
    SelectionEnd,
}

/// What a text drag gesture tells (Compose's TextDragObserver).
pub trait TextDragObserver {
    /// A pointer went down (before a drag that may start); `on_up` follows when it lifts.
    fn on_down(&self, point: Offset);
    /// The pointer of `on_down` lifted.
    fn on_up(&self);
    /// A drag started (past the touch slop) at `start_point`, adjusted by `selection_adjustment`.
    fn on_start(&self, start_point: Offset, selection_adjustment: SelectionAdjustment);
    /// The drag moved by `delta`.
    fn on_drag(&self, delta: Offset);
    /// The drag ended.
    fn on_stop(&self);
    /// The drag was cancelled.
    fn on_cancel(&self);
}

/// Detects a [`TextDragObserver`]'s downs and drags (Compose's `detectDownAndDragGesturesWithObserver`): both at
/// once in `scope`.
pub async fn detect_down_and_drag_gestures_with_observer(scope: PointerInputScope, observer: Rc<dyn TextDragObserver>) {
    coroutine_scope(move |cs| async move {
        let (pre_drag_scope, pre_drag_observer) = (scope.clone(), observer.clone());
        named_args!(cs.launch(start = CoroutineStart::UNDISPATCHED, async move {
            detect_pre_drag_gestures_with_observer(pre_drag_scope, pre_drag_observer).await
        }));
        named_args!(cs.launch(start = CoroutineStart::UNDISPATCHED, async move {
            detect_drag_gestures_with_observer(scope, observer).await
        }));
    })
    .await
}

/// Each first down and the lift of its pointer (Compose's `detectPreDragGesturesWithObserver`).
async fn detect_pre_drag_gestures_with_observer(scope: PointerInputScope, observer: Rc<dyn TextDragObserver>) {
    scope
        .await_each_gesture(|s| {
            let observer = observer.clone();
            async move {
                let down = s.await_first_down_with(false, PointerEventPass::Main).await;
                observer.on_down(down.position);
                // Until that pointer lifts.
                loop {
                    let event = s.await_pointer_event().await;
                    if !event.changes.iter().any(|c| c.id == down.id && c.pressed) {
                        break;
                    }
                }
                observer.on_up();
            }
        })
        .await
}

/// Drags past the touch slop (Compose's `detectDragGesturesWithObserver`).
async fn detect_drag_gestures_with_observer(scope: PointerInputScope, observer: Rc<dyn TextDragObserver>) {
    let (on_end, on_start, on_cancel) = (observer.clone(), observer.clone(), observer.clone());
    scope
        .detect_drag_gestures(move |_, offset| observer.on_drag(offset))
        .on_drag_end(move || on_end.on_stop())
        .on_drag_start(move |at| on_start.on_start(at, SelectionAdjustment::None))
        .on_drag_cancel(move || on_cancel.on_cancel())
        .await
}

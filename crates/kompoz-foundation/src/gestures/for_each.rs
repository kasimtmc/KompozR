//! Gestures one after the other, as Compose's `ForEachGesture.kt`.

use kompoz_ui::input::{AwaitPointerEventScope, PointerEventPass, PointerInputScope};
use std::future::Future;

/// Whether no pointer is pressed in the current event (Compose's `allPointersUp`).
pub(crate) fn all_pointers_up(s: &AwaitPointerEventScope) -> bool {
    !s.current_event().changes.iter().any(|c| c.pressed)
}

/// Waits until no pointer is pressed (Compose's `awaitAllPointersUp`).
pub(crate) async fn await_all_pointers_up(s: &AwaitPointerEventScope, pass: PointerEventPass) {
    if !all_pointers_up(s) {
        loop {
            let event = s.await_pointer_event_with(pass).await;
            if !event.changes.iter().any(|c| c.pressed) {
                break;
            }
        }
    }
}

/// Compose's `awaitEachGesture`.
pub(crate) async fn await_each_gesture<F, Fut>(scope: &PointerInputScope, block: F)
where
    F: Fn(AwaitPointerEventScope) -> Fut,
    Fut: Future<Output = ()>,
{
    scope
        .await_pointer_event_scope(|s| async move {
            loop {
                block(s.clone()).await;
                await_all_pointers_up(&s, PointerEventPass::Final).await;
            }
        })
        .await
}

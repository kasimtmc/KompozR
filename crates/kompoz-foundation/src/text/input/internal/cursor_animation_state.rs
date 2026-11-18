//! A field's blinking cursor (Compose's CursorAnimationState): its alpha, 1 for half a second and 0 for the next,
//! restarted visible as the text or the cursor moves.

use kompoz_coroutines::{Job, coroutine_scope, delay_ms};
use kompoz_runtime::{MutableState, mutable_state_of};
use std::cell::RefCell;
use std::rc::Rc;

/// The cursor's blink (Compose's CursorAnimationState): `animate` false keeps it shown while it runs
/// (`LocalCursorBlinkEnabled`).
pub struct CursorAnimationState {
    animate: bool,
    animation_job: RefCell<Option<Job>>,
    cursor_alpha: MutableState<f32>,
}

impl CursorAnimationState {
    pub fn new(animate: bool) -> Rc<CursorAnimationState> {
        Rc::new(CursorAnimationState {
            animate,
            animation_job: RefCell::new(None),
            cursor_alpha: mutable_state_of(0.0),
        })
    }

    /// Whether it blinks.
    pub fn animate(&self) -> bool {
        self.animate
    }

    /// The alpha to draw the cursor with, 0 to 1 (read as a state).
    pub fn cursor_alpha(&self) -> f32 {
        self.cursor_alpha.get()
    }

    /// Shows the cursor at once and blinks it every 500 ms until cancelled (Compose's `snapToVisibleAndAnimate`):
    /// a blink running before is cancelled (and waited for) first. Cancelled, by [`cancel_and_hide`](
    /// Self::cancel_and_hide) or with the coroutine awaiting this, the cursor ends hidden.
    pub async fn snap_to_visible_and_animate(self: Rc<Self>) {
        coroutine_scope(move |scope| async move {
            let old_job = self.animation_job.borrow_mut().take();
            let this = self.clone();
            let job = scope.launch(async move {
                if let Some(old) = old_job {
                    old.cancel();
                    old.join().await;
                }
                // Hidden however it ends (Kotlin's `finally`).
                struct Hide(MutableState<f32>);
                impl Drop for Hide {
                    fn drop(&mut self) {
                        self.0.set(0.0);
                    }
                }
                let _hide = Hide(this.cursor_alpha.clone());
                this.cursor_alpha.set(1.0);
                if !this.animate {
                    std::future::pending::<()>().await;
                }
                loop {
                    delay_ms(500).await;
                    this.cursor_alpha.set(0.0);
                    delay_ms(500).await;
                    this.cursor_alpha.set(1.0);
                }
            });
            let mut current = self.animation_job.borrow_mut();
            if current.is_none() {
                *current = Some(job);
            }
        })
        .await;
    }

    /// Cancels the blink, hiding the cursor (Compose's `cancelAndHide`).
    pub fn cancel_and_hide(&self) {
        let job = self.animation_job.borrow_mut().take();
        if let Some(job) = job {
            job.cancel();
        }
    }
}

//! The scroll delta between a lazy layout's lookahead and approach passes, as Compose's
//! `LazyLayoutScrollDeltaBetweenPasses.kt`. The lookahead pass is where a lazy layout scrolls; its approach may not
//! take all of a scroll yet (its items not at their lookahead size: what lookahead scrolled to may be past the
//! approach's end, which then scrolls back). What it scrolled back is taken as a delta the approach measures with,
//! animated down to nothing, so the approach doesn't jump.

use kompoz_animation::{AnimationState, AnimationVector1D, Spring, VectorConverter, spring};
use kompoz_coroutines::{CoroutineScope, Job};
use kompoz_graphics::{Density, Dp};
use kompoz_runtime::{named_args, without_read_observation};
use std::cell::RefCell;
use std::sync::Arc;

/// Under this much scroll back (Compose's DeltaThresholdForScrollAnimation, 1 dp), the approach scrolls by it at once.
const DeltaThresholdForScrollAnimation: Dp = Dp(1.0);

/// A lazy layout's scroll delta between its passes (Compose's LazyLayoutScrollDeltaBetweenPasses).
pub(crate) struct LazyLayoutScrollDeltaBetweenPasses {
    job: RefCell<Option<Job>>,
    /// The delta, animating to nothing (Compose's `_scrollDeltaBetweenPasses`; its value a state the approach
    /// measure reads).
    state: RefCell<AnimationState<f32, AnimationVector1D>>,
}

impl LazyLayoutScrollDeltaBetweenPasses {
    pub(crate) fn new() -> LazyLayoutScrollDeltaBetweenPasses {
        LazyLayoutScrollDeltaBetweenPasses {
            job: RefCell::new(None),
            state: RefCell::new(AnimationState::of(f32::VectorConverter, 0.0)),
        }
    }

    /// The delta the approach measures with now (Compose's `scrollDeltaBetweenPasses`), observed where read.
    pub(crate) fn scroll_delta_between_passes(&self) -> f32 {
        let state = self.state.borrow().clone();
        state.value()
    }

    /// Whether there is a delta (Compose's `isActive`).
    pub(crate) fn is_active(&self) -> bool {
        let state = self.state.borrow().clone();
        state.value() != 0.0
    }

    /// Takes what the lookahead's approach would scroll back, `delta` (Compose's `updateScrollDeltaForApproach`):
    /// within the threshold, nothing (the approach scrolls by it); else the delta grows by it and animates down to
    /// nothing in `coroutine_scope`, a running one carrying on from where it is.
    pub(crate) fn update_scroll_delta_for_approach(
        &self,
        delta: f32,
        density: Density,
        coroutine_scope: &CoroutineScope,
    ) {
        if delta <= density.to_px(DeltaThresholdForScrollAnimation) {
            return;
        }
        // (updated in lookahead: no lookahead again when it changes)
        without_read_observation(|| {
            let current = self.state.borrow().clone();
            let current_delta = current.value();
            if let Some(job) = self.job.borrow_mut().take() {
                job.cancel();
            }
            let next = if current.is_running() {
                named_args!(current.copy_float(value = current_delta - delta))
            } else {
                AnimationState::of(f32::VectorConverter, -delta)
            };
            *self.state.borrow_mut() = next.clone();
            let spec = Arc::new(named_args!(spring(
                stiffness = Spring::StiffnessMediumLow,
                visibility_threshold = Some(0.5f32)
            )));
            let job = coroutine_scope.launch(async move {
                named_args!(next.animate_to(0.0, animation_spec = spec, sequential_animation = true)).await;
            });
            *self.job.borrow_mut() = Some(job);
        });
    }

    /// No delta any more (Compose's `stop`): the animation cancelled.
    pub(crate) fn stop(&self) {
        if let Some(job) = self.job.borrow_mut().take() {
            job.cancel();
        }
        *self.state.borrow_mut() = AnimationState::of(f32::VectorConverter, 0.0);
    }
}

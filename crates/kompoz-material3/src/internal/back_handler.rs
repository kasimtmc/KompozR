//! Back handling, as Compose's internal BackHandler.kt with Compose for desktop's actuals: the back event, the
//! handlers (on kompoz-ui's, which Compose for desktop's delegate to as well), and the easing material3's
//! predictive back animations take their progress through.
//!
//! Compose's `expect class BackEventCompat` is kompoz-ui's [`BackEventCompat`] (Compose for desktop's actual copies
//! ui's event field by field); its `swipeEdge` is an [`SwipeEdge`] there, Compose's `EDGE_LEFT` and `EDGE_RIGHT`
//! its `Left` and `Right`.

use kompoz_animation::{CubicBezierEasing, Easing};
use kompoz_coroutines::Flow;
use kompoz_runtime::kompozable;
use std::future::Future;

pub(crate) use kompoz_ui::back::{BackEventCompat, SwipeEdge};

/// Handles back while `enabled` (Compose's internal `BackHandler`, Compose for desktop's actual: ui's
/// `BackHandler`).
#[kompozable]
pub(crate) fn BackHandler(#[default(true)] enabled: bool, on_back: impl Fn() + 'static) {
    kompoz_ui::back::BackHandler(enabled, on_back);
}

/// Handles back with the gesture's progress while `enabled` (Compose's internal `PredictiveBackHandler`, Compose
/// for desktop's actual: ui's `PredictiveBackHandler`, its events as they are).
#[kompozable]
pub(crate) fn PredictiveBackHandler<Fut: Future<Output = ()> + 'static>(
    #[default(true)] enabled: bool,
    on_back: impl Fn(Flow<BackEventCompat>) -> Fut + 'static,
) {
    kompoz_ui::back::PredictiveBackHandler(enabled, on_back);
}

/// The easing of a predictive back's progress (Compose's private `PredictiveBackEasing`).
const PredictiveBackEasing: CubicBezierEasing = CubicBezierEasing::constant(0.1, 0.1, 0.0, 1.0);

/// Predictive back's progress as the components animate by it (Compose's internal `PredictiveBack`).
pub(crate) struct PredictiveBack;

impl PredictiveBack {
    /// `progress` through [`PredictiveBackEasing`] (Compose's `transform`).
    pub(crate) fn transform(progress: f32) -> f32 {
        PredictiveBackEasing.transform(progress)
    }
}

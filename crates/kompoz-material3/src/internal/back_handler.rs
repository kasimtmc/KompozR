//! Back handling, as Compose's internal BackHandler.kt with Android's actuals: the back event, the handlers
//! (activity-compose's, kompoz-ui's `back`, on navigationevent's dispatcher — Compose for desktop's delegate to its
//! ui's back handlers, which are on it as well), and the easing material3's predictive back animations take their
//! progress through.
//!
//! Compose's `expect class BackEventCompat` is kompoz-ui's [`BackEventCompat`] (Android's actual: a typealias of
//! androidx.activity's); its `swipeEdge` is an [`SwipeEdge`] there, Compose's `EDGE_LEFT` and `EDGE_RIGHT` its
//! `Left` and `Right`.

use kompoz_animation::{CubicBezierEasing, Easing};
use kompoz_coroutines::Flow;
use kompoz_runtime::kompozable;
use std::future::Future;

pub(crate) use kompoz_ui::back::{BackEventCompat, SwipeEdge};

/// Handles back while `enabled` (Compose's internal `BackHandler`, Android's actual: activity-compose's
/// `BackHandler`).
#[kompozable]
pub(crate) fn BackHandler(#[default(true)] enabled: bool, on_back: impl Fn() + 'static) {
    kompoz_ui::back::BackHandler(enabled, on_back);
}

/// Handles back with the gesture's progress while `enabled` (Compose's internal `PredictiveBackHandler`, Android's
/// actual: activity-compose's `PredictiveBackHandler`, its events as they are).
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

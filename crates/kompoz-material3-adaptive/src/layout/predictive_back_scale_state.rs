//! The scaffold's scale during a predictive back gesture, as Compose's adaptive-layout PredictiveBackScaleState.kt
//! (1.2.0, internal): the scale (from 1 down toward 0.95 as the gesture goes on), the modifier scaling the scaffold
//! by it, and the conversion of offsets under the scale the bounds animation places by.

use super::three_pane_scaffold_state::ThreePaneScaffoldState;
use kompoz_animation::{Animatable, AnimationVector1D};
use kompoz_coroutines::{LaunchedEffect, snapshot_flow};
use kompoz_graphics::{IntOffset, IntSize, Matrix, Offset};
use kompoz_runtime::kompozable;
use kompoz_ui::Modifier;
use kompoz_ui::layout::TransformOrigin;
use std::cell::Cell;
use std::rc::Rc;

/// The scaffold scaled by the predictive back scale (Compose's internal `Modifier.predictiveBackScale`).
pub(crate) fn predictive_back_scale(modifier: Modifier, state: Rc<PredictiveBackScaleState>) -> Modifier {
    modifier.graphics_layer_block(move |layer| {
        let scale = state.scale();
        layer.scale_x = scale;
        layer.scale_y = scale;
        layer.transform_origin = state.transform_origin;
    })
}

/// The predictive back scale (Compose's internal `PredictiveBackScaleState`).
pub(crate) struct PredictiveBackScaleState {
    scaffold_size: Box<dyn Fn() -> IntSize>,
    is_predictive_back_in_progress: Box<dyn Fn() -> bool>,
    pub(crate) scale_animatable: Animatable<f32, AnimationVector1D>,
    pub(crate) transform_origin: TransformOrigin,
}

impl PredictiveBackScaleState {
    /// The least scale a gesture reaches toward (Compose's `PredictiveBackMinScale`).
    pub(crate) const PredictiveBackMinScale: f32 = 0.95;

    pub(crate) fn new(
        scaffold_size: impl Fn() -> IntSize + 'static,
        is_predictive_back_in_progress: impl Fn() -> bool + 'static,
    ) -> PredictiveBackScaleState {
        PredictiveBackScaleState {
            scaffold_size: Box::new(scaffold_size),
            is_predictive_back_in_progress: Box::new(is_predictive_back_in_progress),
            scale_animatable: Animatable(1.0f32),
            transform_origin: TransformOrigin::Center,
        }
    }

    /// The scale (observable).
    pub(crate) fn scale(&self) -> f32 {
        self.scale_animatable.value()
    }

    /// Where `offset` is under the scale while a predictive back gesture is in progress (itself otherwise).
    pub(crate) fn convert(&self, offset: IntOffset) -> IntOffset {
        if (self.is_predictive_back_in_progress)() {
            let size = (self.scaffold_size)();
            let scale = self.scale_animatable.value();
            let mut matrix = Matrix::new();
            matrix.reset_to_pivoted_transform(
                size.width as f32 * self.transform_origin.pivot_fraction_x,
                size.height as f32 * self.transform_origin.pivot_fraction_y,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                scale,
                scale,
                1.0,
            );
            let mapped = matrix.map(Offset::new(offset.x as f32, offset.y as f32));
            IntOffset::new(round(mapped.x), round(mapped.y))
        } else {
            offset
        }
    }
}

/// Kotlin's `roundToInt` (Math.round).
fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// Follows the state's progress into the scale (Compose's internal `ThreePaneScaffoldState.CollectPredictiveBackScale`):
/// snapped along a decay curve while a predictive back gesture moves the state, animated back to 1 otherwise.
#[kompozable]
pub(crate) fn CollectPredictiveBackScale(
    scaffold_state: Rc<dyn ThreePaneScaffoldState>,
    predictive_back_scale_state: Rc<PredictiveBackScaleState>,
) {
    let previous_value = kompoz_runtime::remember(|| Rc::new(Cell::new(0.0f32)));
    let key = StateKey(scaffold_state.clone());
    LaunchedEffect(key, async move {
        let state = scaffold_state.clone();
        let progress = snapshot_flow(move || state.progress_fraction());
        let (state, scale) = (scaffold_state.clone(), predictive_back_scale_state.clone());
        progress
            .collect_async(move |value| {
                let (state, scale, previous_value) = (state.clone(), scale.clone(), previous_value.clone());
                async move {
                    if value == previous_value.get() {
                        return;
                    }
                    previous_value.set(value);
                    if state.is_predictive_back_in_progress() {
                        let target = convert_state_progress_to_predictive_back_scale(value);
                        scale.scale_animatable.snap_to(target).await;
                    } else {
                        let _ = kompoz_runtime::named_args!(scale.scale_animatable.animate_to(1.0)).await;
                    }
                }
            })
            .await;
    });
}

/// A scaffold state as a key: the same state (Compose's `LaunchedEffect(this)`).
struct StateKey(Rc<dyn ThreePaneScaffoldState>);

impl PartialEq for StateKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// A decay curve: 1 at 0, toward the least scale as the fraction goes to 1 (Compose's private
/// `convertStateProgressToPredictiveBackScale`).
fn convert_state_progress_to_predictive_back_scale(fraction: f32) -> f32 {
    let delta = 1.0 - PredictiveBackScaleState::PredictiveBackMinScale;
    let shift = delta / 2.0;
    let curve_scale = delta * delta / 2.0;
    curve_scale / (fraction + shift) + PredictiveBackScaleState::PredictiveBackMinScale
}

//! A lazy grid's snapping (Compose's LazyGridSnapLayoutInfoProvider.kt): the approach stops an item short of where
//! a decay would go; the snap goes to the item nearest its snap position (before or after it, by the velocity left).

use super::{
    SnapLayoutInfoProvider, SnapPosition, calculate_distance_to_desired_snap_position, calculate_final_offset,
    calculate_final_snapping_item, remember_snap_fling_behavior,
};
use crate::gestures::FlingBehavior;
use crate::lazy::grid::LazyGridState;
use kompoz_runtime::{kompozable, named_args, remember_keyed};
use std::rc::Rc;

/// The snap layout info of a lazy grid (Compose's `SnapLayoutInfoProvider(lazyGridState, snapPosition)`): items
/// snap to `snap_position`.
/// Kotlin: `SnapLayoutInfoProvider(lazyGridState: LazyGridState, snapPosition: SnapPosition = SnapPosition.Center)`
#[kompoz_runtime::named]
pub fn lazy_grid_snap_layout_info_provider(
    lazy_grid_state: &LazyGridState,
    #[default(SnapPosition::Center)] snap_position: SnapPosition,
) -> Rc<dyn SnapLayoutInfoProvider> {
    Rc::new(LazyGridSnapLayoutInfoProvider { lazy_grid_state: lazy_grid_state.clone(), snap_position })
}

struct LazyGridSnapLayoutInfoProvider {
    lazy_grid_state: LazyGridState,
    snap_position: SnapPosition,
}

impl LazyGridSnapLayoutInfoProvider {
    /// The average size of the items in view along the grid.
    fn average_item_size(&self) -> i32 {
        let layout_info = self.lazy_grid_state.layout_info();
        let items = layout_info.items();
        if items.is_empty() {
            0
        } else {
            items.iter().map(|i| i.size_on_main_axis(layout_info.orientation)).sum::<i32>() / items.len() as i32
        }
    }
}

impl SnapLayoutInfoProvider for LazyGridSnapLayoutInfoProvider {
    fn calculate_approach_offset(&self, _velocity: f32, decay_offset: f32) -> f32 {
        (decay_offset.abs() - self.average_item_size() as f32).max(0.0) * sign(decay_offset)
    }

    fn calculate_snap_offset(&self, velocity: f32) -> f32 {
        let layout_info = self.lazy_grid_state.layout_info();
        let mut distance_from_item_before_target = f32::NEG_INFINITY;
        let mut distance_from_item_after_target = f32::INFINITY;
        for item in layout_info.items() {
            let distance = calculate_distance_to_desired_snap_position(
                layout_info.single_axis_viewport_size(),
                layout_info.before_content_padding,
                layout_info.after_content_padding,
                item.size_on_main_axis(layout_info.orientation),
                item.offset_on_main_axis(layout_info.orientation),
                item.index,
                &self.snap_position,
                layout_info.total_items_count,
            );
            // The item closest to its place before it…
            if distance <= 0.0 && distance > distance_from_item_before_target {
                distance_from_item_before_target = distance;
            }
            // …and after it.
            if distance >= 0.0 && distance < distance_from_item_after_target {
                distance_from_item_after_target = distance;
            }
        }
        let density = self.lazy_grid_state.density();
        calculate_final_offset(
            calculate_final_snapping_item(&density, velocity),
            distance_from_item_before_target,
            distance_from_item_after_target,
        )
    }
}

/// A snapping fling of a lazy grid remembered for it (Compose's `rememberSnapFlingBehavior(lazyGridState,
/// snapPosition)`): its items snap to `snap_position`.
/// Kotlin: `rememberSnapFlingBehavior(lazyGridState: LazyGridState, snapPosition: SnapPosition = SnapPosition.Center)`
#[kompozable]
pub fn remember_snap_fling_behavior_for_grid(
    lazy_grid_state: &LazyGridState,
    #[default(SnapPosition::Center)] snap_position: SnapPosition,
) -> Rc<dyn FlingBehavior> {
    let state = lazy_grid_state.clone();
    let snapping_layout = remember_keyed(lazy_grid_state.clone(), move || {
        named_args!(lazy_grid_snap_layout_info_provider(&state, snap_position = snap_position))
    });
    remember_snap_fling_behavior(snapping_layout)
}

fn sign(v: f32) -> f32 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

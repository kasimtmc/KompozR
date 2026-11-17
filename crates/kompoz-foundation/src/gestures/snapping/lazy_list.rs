//! A lazy list's snapping (Compose's LazyListSnapLayoutInfoProvider.kt): the approach stops an item short of where
//! a decay would go; the snap goes to the item nearest its snap position (before or after it, by the velocity left).

use super::{
    SnapLayoutInfoProvider, SnapPosition, calculate_distance_to_desired_snap_position, calculate_final_offset,
    calculate_final_snapping_item, remember_snap_fling_behavior,
};
use crate::gestures::TargetedFlingBehavior;
use crate::lazy::LazyListState;
use kompoz_runtime::{kompozable, named_args, remember_keyed};
use std::rc::Rc;

/// The snap layout info of a lazy list (Compose's `SnapLayoutInfoProvider(lazyListState, snapPosition)`): items
/// snap to `snap_position`.
/// Kotlin: `SnapLayoutInfoProvider(lazyListState: LazyListState, snapPosition: SnapPosition = SnapPosition.Center)`
#[kompoz_runtime::named]
pub fn lazy_list_snap_layout_info_provider(
    lazy_list_state: &LazyListState,
    #[default(SnapPosition::Center)] snap_position: SnapPosition,
) -> Rc<dyn SnapLayoutInfoProvider> {
    Rc::new(LazyListSnapLayoutInfoProvider { lazy_list_state: lazy_list_state.clone(), snap_position })
}

struct LazyListSnapLayoutInfoProvider {
    lazy_list_state: LazyListState,
    snap_position: SnapPosition,
}

impl LazyListSnapLayoutInfoProvider {
    /// The average size of the items in view.
    fn average_item_size(&self) -> i32 {
        let layout_info = self.lazy_list_state.layout_info();
        let items = layout_info.items();
        if items.is_empty() { 0 } else { items.iter().map(|i| i.size).sum::<i32>() / items.len() as i32 }
    }
}

impl SnapLayoutInfoProvider for LazyListSnapLayoutInfoProvider {
    fn calculate_approach_offset(&self, _velocity: f32, decay_offset: f32) -> f32 {
        (decay_offset.abs() - self.average_item_size() as f32).max(0.0) * sign(decay_offset)
    }

    fn calculate_snap_offset(&self, velocity: f32) -> f32 {
        let layout_info = self.lazy_list_state.layout_info();
        let mut lower_bound_offset = f32::NEG_INFINITY;
        let mut upper_bound_offset = f32::INFINITY;
        for item in layout_info.items() {
            // (a sticking header doesn't snap)
            if item.non_scrollable {
                continue;
            }
            let offset = calculate_distance_to_desired_snap_position(
                layout_info.single_axis_viewport_size(),
                layout_info.before_content_padding,
                layout_info.after_content_padding,
                item.size,
                item.offset,
                item.index,
                &self.snap_position,
                layout_info.total_items_count,
            );
            // The item closest to its place before it…
            if offset <= 0.0 && offset > lower_bound_offset {
                lower_bound_offset = offset;
            }
            // …and after it.
            if offset >= 0.0 && offset < upper_bound_offset {
                upper_bound_offset = offset;
            }
        }
        let density = self.lazy_list_state.density();
        calculate_final_offset(
            calculate_final_snapping_item(&density, velocity),
            lower_bound_offset,
            upper_bound_offset,
        )
    }
}

/// A snapping fling of a lazy list remembered for it (Compose's `rememberSnapFlingBehavior(lazyListState,
/// snapPosition)`): its items snap to `snap_position`.
/// Kotlin: `rememberSnapFlingBehavior(lazyListState: LazyListState, snapPosition: SnapPosition = SnapPosition.Center)`
#[kompozable]
pub fn remember_snap_fling_behavior_for_list(
    lazy_list_state: &LazyListState,
    #[default(SnapPosition::Center)] snap_position: SnapPosition,
) -> Rc<dyn TargetedFlingBehavior> {
    let state = lazy_list_state.clone();
    let snapping_layout = remember_keyed(lazy_list_state.clone(), move || {
        named_args!(lazy_list_snap_layout_info_provider(&state, snap_position = snap_position))
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

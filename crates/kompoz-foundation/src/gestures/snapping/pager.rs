//! A pager's snapping (Compose's PagerSnapLayoutInfoProvider.kt): the approach goes as many pages as a decay would
//! carry it (within the snap distance) less one; the snap goes to the page before or after the snap position, by
//! the fling's velocity — or, slow, by how far the drag pulled the page (past the positional threshold: the next
//! one).

use super::{
    FinalSnappingItem, SnapLayoutInfoProvider, SnapPosition, calculate_distance_to_desired_snap_position,
    calculate_final_snapping_item,
};
use crate::gestures::{Orientation, ScrollableState};
use crate::pager::{PagerSnapDistance, PagerState};
use kompoz_graphics::LayoutDirection;
use std::rc::Rc;

/// The final snap bound of a fling (Compose's `calculateFinalSnappingBound` as passed in): its velocity, the
/// lower and upper bounds.
pub(crate) type CalculateFinalSnappingBound = Rc<dyn Fn(f32, f32, f32) -> f32>;

/// A pager's snap layout info (Compose's internal `SnapLayoutInfoProvider(pagerState, pagerSnapDistance,
/// calculateFinalSnappingBound)`).
pub(crate) fn pager_snap_layout_info_provider(
    pager_state: PagerState,
    pager_snap_distance: PagerSnapDistance,
    calculate_final_snapping_bound: CalculateFinalSnappingBound,
) -> Rc<dyn SnapLayoutInfoProvider> {
    Rc::new(PagerSnapLayoutInfoProvider { pager_state, pager_snap_distance, calculate_final_snapping_bound })
}

struct PagerSnapLayoutInfoProvider {
    pager_state: PagerState,
    pager_snap_distance: PagerSnapDistance,
    calculate_final_snapping_bound: CalculateFinalSnappingBound,
}

impl PagerSnapLayoutInfoProvider {
    /// The offsets of the pages closest to the snap position before and after it (Compose's
    /// `searchForSnappingBounds`): one for the other when missing, none towards an end it can't go past.
    fn search_for_snapping_bounds(&self, snap_position: &SnapPosition, velocity: f32) -> (f32, f32) {
        let layout_info = self.pager_state.layout_info();
        let mut lower_bound_offset = f32::NEG_INFINITY;
        let mut upper_bound_offset = f32::INFINITY;
        for page in layout_info.pages() {
            let offset = calculate_distance_to_desired_snap_position(
                layout_info.main_axis_viewport_size(),
                layout_info.before_content_padding,
                layout_info.after_content_padding,
                layout_info.page_size,
                page.offset,
                page.index,
                snap_position,
                self.pager_state.page_count(),
            );
            // The page closest to the snap position before it…
            if offset <= 0.0 && offset > lower_bound_offset {
                lower_bound_offset = offset;
            }
            // …and after it.
            if offset >= 0.0 && offset < upper_bound_offset {
                upper_bound_offset = offset;
            }
        }
        // (a bound missing: the other)
        if lower_bound_offset == f32::NEG_INFINITY {
            lower_bound_offset = upper_bound_offset;
        }
        if upper_bound_offset == f32::INFINITY {
            upper_bound_offset = lower_bound_offset;
        }
        // (at an end: no moving past it)
        if !self.pager_state.can_scroll_forward() {
            upper_bound_offset = 0.0;
            if is_scrolling_forward(&self.pager_state, velocity) {
                lower_bound_offset = 0.0;
            }
        }
        if !self.pager_state.can_scroll_backward() {
            lower_bound_offset = 0.0;
            if !is_scrolling_forward(&self.pager_state, velocity) {
                upper_bound_offset = 0.0;
            }
        }
        (lower_bound_offset, upper_bound_offset)
    }
}

impl SnapLayoutInfoProvider for PagerSnapLayoutInfoProvider {
    fn calculate_snap_offset(&self, velocity: f32) -> f32 {
        let snap_position = self.pager_state.layout_info().snap_position;
        let (lower_bound_offset, upper_bound_offset) = self.search_for_snapping_bounds(&snap_position, velocity);
        let final_distance = (self.calculate_final_snapping_bound)(velocity, lower_bound_offset, upper_bound_offset);
        assert!(
            final_distance == lower_bound_offset || final_distance == upper_bound_offset || final_distance == 0.0,
            "Final Snapping Offset Should Be one of {lower_bound_offset}, {upper_bound_offset} or 0.0"
        );
        if final_distance.is_finite() { final_distance } else { 0.0 }
    }

    fn calculate_approach_offset(&self, velocity: f32, decay_offset: f32) -> f32 {
        let state = &self.pager_state;
        let effective_page_size_px = state.page_size() + state.page_spacing();
        // (no page size: no approach)
        if effective_page_size_px == 0 {
            return 0.0;
        }
        let start_page = if velocity < 0.0 { state.first_visible_page() + 1 } else { state.first_visible_page() };
        // (the pages the decay would carry it)
        let pages_in_animation_offset = decay_offset / effective_page_size_px as f32;
        let target_page = (start_page + pages_in_animation_offset as i32).clamp(0, state.page_count().max(0));
        let corrected_target_page = self
            .pager_snap_distance
            .calculate_target_page(start_page, target_page, velocity, state.page_size(), state.page_spacing())
            .clamp(0, state.page_count().max(0));
        let proposed_fling_offset = (corrected_target_page - start_page) * effective_page_size_px;
        // (the approach ends a page short: the snap does the rest)
        let fling_approach_offset_px = (proposed_fling_offset.abs() - effective_page_size_px).max(0);
        if fling_approach_offset_px == 0 {
            fling_approach_offset_px as f32
        } else {
            fling_approach_offset_px as f32 * sign(velocity)
        }
    }
}

/// Whether a fling of `velocity` (or the drag, if there was one) goes forward (Compose's `isScrollingForward`).
fn is_scrolling_forward(state: &PagerState, velocity: f32) -> bool {
    let reverse_scroll_direction = state.layout_info().reverse_layout;
    let is_forward = if state.is_not_gesture_action() { -velocity } else { drag_gesture_delta(state) } > 0.0;
    (is_forward && reverse_scroll_direction) || (!is_forward && !reverse_scroll_direction)
}

/// How far the last gesture moved along the pager (Compose's `dragGestureDelta`).
fn drag_gesture_delta(state: &PagerState) -> f32 {
    let difference = state.inner.up_down_difference.get();
    if state.layout_info().orientation == Orientation::Horizontal { difference.x } else { difference.y }
}

/// The bound a pager's fling snaps to (Compose's `calculateFinalSnappingBound`): a fast fling the next or previous
/// page; a slow one past `snap_positional_threshold` of a page the next page its way, short of it but past the
/// position threshold (56 dp) back, else the closest.
pub(crate) fn calculate_final_snapping_bound(
    pager_state: &PagerState,
    layout_direction: LayoutDirection,
    snap_positional_threshold: f32,
    fling_velocity: f32,
    lower_bound_offset: f32,
    upper_bound_offset: f32,
) -> f32 {
    let is_scrolling_forward = is_scrolling_forward(pager_state, fling_velocity);
    let is_forward =
        if pager_state.layout_info().orientation == Orientation::Vertical || layout_direction == LayoutDirection::Ltr {
            is_scrolling_forward
        } else {
            !is_scrolling_forward
        };
    // (the pages the drag pulled)
    let page_size = pager_state.layout_info().page_size;
    let offset_from_snapped_position =
        if page_size == 0 { 0.0 } else { drag_gesture_delta(pager_state) / page_size as f32 };
    // (only its fraction)
    let offset_from_snapped_position_overflow =
        offset_from_snapped_position - (offset_from_snapped_position as i32) as f32;
    let final_snapping_item = calculate_final_snapping_item(&pager_state.density(), fling_velocity);
    match final_snapping_item {
        FinalSnappingItem::ClosestItem => {
            if offset_from_snapped_position_overflow.abs() > snap_positional_threshold {
                // (past the threshold: the next bound)
                if is_forward { upper_bound_offset } else { lower_bound_offset }
            } else if offset_from_snapped_position.abs() >= pager_state.position_threshold_fraction().abs() {
                // (short of it, but pulled: back)
                if is_forward { lower_bound_offset } else { upper_bound_offset }
            } else if lower_bound_offset.abs() < upper_bound_offset.abs() {
                // (hardly moved: the closest)
                lower_bound_offset
            } else {
                upper_bound_offset
            }
        }
        FinalSnappingItem::NextItem => upper_bound_offset,
        FinalSnappingItem::PreviousItem => lower_bound_offset,
    }
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

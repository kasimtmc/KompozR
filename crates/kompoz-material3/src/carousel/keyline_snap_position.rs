//! Where carousel items snap to (Compose's internal `KeylineSnapPosition.kt`).

use super::carousel::CarouselPageSize;
use super::strategy::Strategy;
use kompoz_foundation::gestures::snapping::{SnapPosition, SnapPositionCalculator};
use kompoz_runtime::kt::KtFloat;
use std::rc::Rc;

/// The offset from the container's start that snaps to the item at `item_index` (Compose's internal
/// `getSnapPositionOffset`): from the keyline list that has that item fully visible at a focal keyline — a start
/// step for the first items, an end step for the last ones, else the default keylines.
pub(crate) fn get_snap_position_offset(strategy: &Strategy, item_index: i32, item_count: i32) -> i32 {
    if !strategy.is_valid {
        return 0;
    }

    // By default the first focal keyline snaps to its resting place: every item needing no shift into the focal
    // range (an index past the start steps and before the end ones).
    let mut offset = (strategy.default_keylines.first_focal().unadjusted_offset - strategy.item_main_axis_size() / 2.0)
        .round_to_int();

    let start_last_index = strategy.start_keyline_steps.len() as i32 - 1;
    if item_index <= start_last_index {
        // The first items are focal at the step of their index; the steps go from the default one to the last start
        // step, so taken from the last.
        let step_index = (start_last_index - item_index).clamp(0, start_last_index);
        let start_keylines = &strategy.start_keyline_steps[step_index as usize];
        offset = (start_keylines.first_focal().unadjusted_offset - strategy.item_main_axis_size() / 2.0).round_to_int();
    }

    let last_item_index = item_count - 1;
    let end_last_index = strategy.end_keyline_steps.len() as i32 - 1;
    // (with every item on a focal keyline, no end steps)
    if item_index >= last_item_index - end_last_index && item_count > strategy.default_keylines.focal_count {
        // The item's distance from the end is its end step, taken from the last likewise.
        let step_index = (end_last_index - (last_item_index - item_index)).clamp(0, end_last_index);
        let end_keylines = &strategy.end_keyline_steps[step_index as usize];
        offset = (end_keylines.last_focal().unadjusted_offset - strategy.item_main_axis_size() / 2.0).round_to_int();
    }

    offset
}

/// The snap position of a carousel's pages: each item's keyline snap offset of the page size's strategy (Compose's
/// internal `KeylineSnapPosition(pageSize)`).
pub(crate) fn KeylineSnapPosition(page_size: Rc<CarouselPageSize>) -> SnapPosition {
    SnapPosition::Custom(Rc::new(KeylineSnapPositionCalculator { page_size }))
}

struct KeylineSnapPositionCalculator {
    page_size: Rc<CarouselPageSize>,
}

impl SnapPositionCalculator for KeylineSnapPositionCalculator {
    fn position(
        &self,
        _layout_size: i32,
        _item_size: i32,
        _before_content_padding: i32,
        _after_content_padding: i32,
        item_index: i32,
        item_count: i32,
    ) -> i32 {
        get_snap_position_offset(&self.page_size.strategy(), item_index, item_count)
    }
}

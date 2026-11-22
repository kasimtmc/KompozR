//! The keyline lists of the carousel configurations — multi-browse, uncontained, hero (Compose's internal
//! `Keylines.kt`).

use super::arrangement::Arrangement;
use super::carousel::{CarouselAlignment, CarouselDefaults};
use super::keyline_list::{KeylineList, keyline_list_of_alignment};
use kompoz_graphics::Density;

/// Keylines arranging items into a multi-browse configuration (Compose's internal `multiBrowseKeylineList`): the
/// nearest number of large items that fits an approved arrangement of large, medium and small items needing the least
/// size adjustment (large items may change size). `carousel_main_axis_size`, `preferred_item_size` (the large items'
/// wish), `item_spacing` in pixels; `min_small_item_size` / `max_small_item_size` None:
/// [`CarouselDefaults::MinSmallItemSize`] / [`CarouselDefaults::MaxSmallItemSize`] in pixels (Kotlin's defaults).
pub(crate) fn multi_browse_keyline_list(
    density: &Density,
    carousel_main_axis_size: f32,
    preferred_item_size: f32,
    item_spacing: f32,
    item_count: i32,
    min_small_item_size: Option<f32>,
    max_small_item_size: Option<f32>,
) -> KeylineList {
    let min_small_item_size = min_small_item_size.unwrap_or_else(|| density.to_px(CarouselDefaults::MinSmallItemSize));
    let max_small_item_size = max_small_item_size.unwrap_or_else(|| density.to_px(CarouselDefaults::MaxSmallItemSize));
    if carousel_main_axis_size == 0.0 || preferred_item_size == 0.0 {
        return KeylineList::empty();
    }

    let mut small_counts: Vec<i32> = vec![1];
    let medium_counts: Vec<i32> = vec![1, 0];

    let target_large_size = preferred_item_size.min(carousel_main_axis_size);
    // A balanced arrangement has small items a third of the large ones and medium ones between: the small target is
    // as close to a third as the min-max range allows.
    let target_small_size = (target_large_size / 3.0).clamp(min_small_item_size, max_small_item_size);
    let target_medium_size = (target_large_size + target_small_size) / 2.0;

    if carousel_main_axis_size < min_small_item_size * 2.0 {
        // Too small for a large item and a small one (a large item bigger than a small one): no small items.
        small_counts = vec![0];
    }

    // The least space left for large items after the most medium and small items: a plausible minimum large count.
    let min_available_large_space = carousel_main_axis_size
        - target_medium_size * max_of(&medium_counts) as f32
        - max_small_item_size * max_of(&small_counts) as f32;
    let min_large_count = 1.max((min_available_large_space / target_large_size).floor() as i32);
    let max_large_count = (carousel_main_axis_size / target_large_size).ceil() as i32;

    let large_counts: Vec<i32> = (0..(max_large_count - min_large_count + 1)).map(|it| max_large_count - it).collect();
    let anchor_size = density.to_px(CarouselDefaults::AnchorSize);
    let mut arrangement = Arrangement::find_lowest_cost_arrangement(
        carousel_main_axis_size,
        item_spacing,
        target_small_size,
        min_small_item_size,
        max_small_item_size,
        &small_counts,
        target_medium_size,
        &medium_counts,
        target_large_size,
        &large_counts,
    );

    if let Some(found) = arrangement
        && found.item_count() > item_count
    {
        let mut keyline_surplus = found.item_count() - item_count;
        let mut small_count = found.small_count;
        let mut medium_count = found.medium_count;
        while keyline_surplus > 0 {
            if small_count > 0 {
                small_count -= 1;
            } else if medium_count > 1 {
                // At least one medium item stays, so the large items don't fill the whole carousel.
                medium_count -= 1;
            }
            // Large items aren't removed even when a surplus: they are fully unmasked already.
            keyline_surplus -= 1;
        }
        arrangement = Arrangement::find_lowest_cost_arrangement(
            carousel_main_axis_size,
            item_spacing,
            target_small_size,
            min_small_item_size,
            max_small_item_size,
            &[small_count],
            target_medium_size,
            &[medium_count],
            target_large_size,
            &large_counts,
        );
    }

    let Some(arrangement) = arrangement else {
        return KeylineList::empty();
    };

    create_left_aligned_keyline_list(carousel_main_axis_size, item_spacing, anchor_size, anchor_size, &arrangement)
}

/// Keylines arranging items into the uncontained configuration (Compose's internal `uncontainedKeylineList`): as
/// many items of `item_size` as fit uncut, and in the space left a cut-off item sized to keep enough motion when
/// scrolling off screen.
pub(crate) fn uncontained_keyline_list(
    density: &Density,
    carousel_main_axis_size: f32,
    item_size: f32,
    item_spacing: f32,
) -> KeylineList {
    if carousel_main_axis_size == 0.0 || item_size == 0.0 {
        return KeylineList::empty();
    }

    let large_item_size = (item_size + item_spacing).min(carousel_main_axis_size);
    // The space left after as many large items as fit.
    let large_count = 1.max((carousel_main_axis_size / large_item_size).floor() as i32);
    let remaining_space = carousel_main_axis_size - large_count as f32 * large_item_size;

    let medium_count = if remaining_space > 0.0 { 1 } else { 0 };

    let default_anchor_size = density.to_px(CarouselDefaults::AnchorSize);
    let medium_item_size = calculate_medium_child_size(default_anchor_size, large_item_size, remaining_space);
    let arrangement = Arrangement::new(0, 0.0, 0, medium_item_size, medium_count, large_item_size, large_count);

    let x_small_size = default_anchor_size.min(item_size);
    // The start anchor half the cut-off item's size, so the motion at the start is closer to the end's (the cut-off).
    let left_anchor_size = x_small_size.max(medium_item_size * 0.5);
    create_left_aligned_keyline_list(
        carousel_main_axis_size,
        item_spacing,
        left_anchor_size,
        default_anchor_size,
        &arrangement,
    )
}

/// Keylines arranging items into a hero configuration (Compose's internal `heroKeylineList`). `max_item_size`:
/// None, one large item fills the viewport less the small items' room; else large items of at most that size are
/// added as space allows. `is_centered` (Kotlin's default false) centers the large item (start-aligned below three
/// items). The small sizes as [`multi_browse_keyline_list`]'s.
#[allow(clippy::too_many_arguments)]
pub(crate) fn hero_keyline_list(
    density: &Density,
    carousel_main_axis_size: f32,
    max_item_size: Option<f32>,
    item_spacing: f32,
    item_count: i32,
    is_centered: bool,
    min_small_item_size: Option<f32>,
    max_small_item_size: Option<f32>,
) -> KeylineList {
    let min_small_item_size = min_small_item_size.unwrap_or_else(|| density.to_px(CarouselDefaults::MinSmallItemSize));
    let max_small_item_size = max_small_item_size.unwrap_or_else(|| density.to_px(CarouselDefaults::MaxSmallItemSize));
    if carousel_main_axis_size == 0.0 {
        return KeylineList::empty();
    }

    // Arrangements of fewer than 3 items can't be centered.
    let should_center = is_centered && item_count >= 3;

    let mut small_counts: Vec<i32> = if item_count <= 1 {
        // One item: large, no small items.
        vec![0]
    } else if should_center {
        // Centered arrangements always need an even number of small items.
        vec![2]
    } else {
        // Otherwise a start-aligned hero carousel with one small item.
        vec![1]
    };

    let target_large_size = max_item_size.unwrap_or(carousel_main_axis_size).min(carousel_main_axis_size);
    // Visually balanced layouts aim at small items a third of the large ones.
    let target_small_size = (target_large_size / 3.0).clamp(min_small_item_size, max_small_item_size);

    // Room for the small items plus a large item at least 25% larger than them.
    let fullscreen_threshold = (min_small_item_size * max_of(&small_counts) as f32) + (min_small_item_size * 1.25);
    if carousel_main_axis_size < fullscreen_threshold {
        small_counts = vec![0];
    }

    let min_available_large_space = carousel_main_axis_size - (min_small_item_size * max_of(&small_counts) as f32);
    let min_large_count = 1.max((min_available_large_space / target_large_size).floor() as i32);
    let max_large_count = (carousel_main_axis_size / target_large_size).ceil() as i32;
    let large_counts: Vec<i32> = (0..(max_large_count - min_large_count + 1)).map(|it| max_large_count - it).collect();
    let anchor_size = density.to_px(CarouselDefaults::AnchorSize);
    let arrangement = Arrangement::find_lowest_cost_arrangement(
        carousel_main_axis_size,
        item_spacing,
        target_small_size,
        min_small_item_size,
        max_small_item_size,
        &small_counts,
        0.0,
        &[0],
        target_large_size,
        &large_counts,
    );

    let Some(arrangement) = arrangement else {
        return KeylineList::empty();
    };

    if should_center && item_count >= arrangement.item_count() {
        // Centered, with enough items to fill every keyline.
        create_center_aligned_keyline_list(
            carousel_main_axis_size,
            item_spacing,
            anchor_size,
            anchor_size,
            &arrangement,
        )
    } else {
        create_left_aligned_keyline_list(carousel_main_axis_size, item_spacing, anchor_size, anchor_size, &arrangement)
    }
}

/// Kotlin's `IntArray.max()` (throws on an empty array: panics).
fn max_of(counts: &[i32]) -> i32 {
    *counts.iter().max().expect("Array is empty.")
}

/// A start-aligned list: the start anchor, the large, medium and small items, the end anchor (Compose's internal
/// `createLeftAlignedKeylineList`).
pub(crate) fn create_left_aligned_keyline_list(
    carousel_main_axis_size: f32,
    item_spacing: f32,
    left_anchor_size: f32,
    right_anchor_size: f32,
    arrangement: &Arrangement,
) -> KeylineList {
    keyline_list_of_alignment(carousel_main_axis_size, item_spacing, CarouselAlignment::Start, |scope| {
        scope.add(left_anchor_size, true);
        for _ in 0..arrangement.large_count {
            scope.add(arrangement.large_size, false);
        }
        for _ in 0..arrangement.medium_count {
            scope.add(arrangement.medium_size, false);
        }
        for _ in 0..arrangement.small_count {
            scope.add(arrangement.small_size, false);
        }
        scope.add(right_anchor_size, true);
    })
}

/// A centered list: the start anchor, half the small and medium items, the large items, the other halves, the end
/// anchor (Compose's internal `createCenterAlignedKeylineList`).
pub(crate) fn create_center_aligned_keyline_list(
    carousel_main_axis_size: f32,
    item_spacing: f32,
    left_anchor_size: f32,
    right_anchor_size: f32,
    arrangement: &Arrangement,
) -> KeylineList {
    keyline_list_of_alignment(carousel_main_axis_size, item_spacing, CarouselAlignment::Center, |scope| {
        scope.add(left_anchor_size, true);
        for _ in 0..arrangement.small_count / 2 {
            scope.add(arrangement.small_size, false);
        }
        for _ in 0..arrangement.medium_count / 2 {
            scope.add(arrangement.medium_size, false);
        }
        for _ in 0..arrangement.large_count {
            scope.add(arrangement.large_size, false);
        }
        for _ in 0..arrangement.medium_count / 2 {
            scope.add(arrangement.medium_size, false);
        }
        for _ in 0..arrangement.small_count / 2 {
            scope.add(arrangement.small_size, false);
        }
        scope.add(right_anchor_size, true);
    })
}

/// A medium item size no bigger than the large one, small enough to differ from it and large enough to be cut off
/// by a good part (Compose's private `calculateMediumChildSize`).
fn calculate_medium_child_size(minimum_medium_size: f32, large_item_size: f32, remaining_space: f32) -> f32 {
    // Ideally a third of the medium item is cut off: 1.5 × the space left.
    let size_with_third_cut_off = remaining_space * 1.5;
    let mut medium_item_size = size_with_third_cut_off.max(minimum_medium_size);

    // A medium item over the threshold part of the large one is too similar to move enough between them.
    let large_item_threshold = large_item_size * CarouselDefaults::MediumLargeItemDiffThreshold;
    if medium_item_size > large_item_threshold {
        // The larger of the threshold and a size with a fifth cut off, never over the large size.
        let size_with_fifth_cut_off = remaining_space * 1.2;
        medium_item_size = large_item_threshold.max(size_with_fifth_cut_off).min(large_item_size);
    }
    medium_item_size
}

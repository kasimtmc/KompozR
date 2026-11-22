//! A carousel's keylines corrected for scroll offset and content padding (Compose's internal `Strategy.kt`).

use super::keyline_list::{Keyline, KeylineList, keyline_list_of_pivot, lerp_keyline_list};
use kompoz_graphics::lerp as lerp_float;
use std::sync::OnceLock;

/// An immutable supplier of the [`KeylineList`] a carousel uses at a scroll offset (Compose's internal `Strategy`).
/// From the default keylines (left to right / top to bottom) it derives the steps that move the focal range to the
/// container's start (`start_keyline_steps`, one keyline a step) and to its end (`end_keyline_steps`), so every item
/// becomes focal at the ends of the list without leaving the container's edges; content paddings shrink the last
/// steps.
#[derive(Clone, Debug)]
pub(crate) struct Strategy {
    pub(crate) default_keylines: KeylineList,
    pub(crate) start_keyline_steps: Vec<KeylineList>,
    pub(crate) end_keyline_steps: Vec<KeylineList>,
    pub(crate) available_space: f32,
    pub(crate) item_spacing: f32,
    pub(crate) before_content_padding: f32,
    pub(crate) after_content_padding: f32,
    /// The scroll distance through all start steps.
    start_shift_distance: f32,
    /// The scroll distance through all end steps.
    end_shift_distance: f32,
    /// For each start step, the part of `start_shift_distance` scrolled when it is used.
    start_shift_points: Vec<f32>,
    /// For each end step, the part of `end_shift_distance` scrolled when it is used.
    end_shift_points: Vec<f32>,
    /// Whether this strategy has a valid arrangement for a valid container.
    pub(crate) is_valid: bool,
    /// The last start step and the last end step, kept for carousels skipping the default keylines (Compose's
    /// `lastStartAndEndKeylineListSteps`, made once when first needed).
    last_start_and_end_keyline_list_steps: OnceLock<Vec<KeylineList>>,
}

impl Strategy {
    fn with_steps(
        default_keylines: KeylineList,
        start_keyline_steps: Vec<KeylineList>,
        end_keyline_steps: Vec<KeylineList>,
        available_space: f32,
        item_spacing: f32,
        before_content_padding: f32,
        after_content_padding: f32,
    ) -> Strategy {
        let start_shift_distance = get_start_shift_distance(&start_keyline_steps, before_content_padding);
        let end_shift_distance = get_end_shift_distance(&end_keyline_steps, after_content_padding);
        let start_shift_points = get_step_interpolation_points(start_shift_distance, &start_keyline_steps, true);
        let end_shift_points = get_step_interpolation_points(end_shift_distance, &end_keyline_steps, false);
        let is_valid =
            !default_keylines.is_empty() && available_space != 0.0 && default_keylines.first_focal().size != 0.0;
        Strategy {
            default_keylines,
            start_keyline_steps,
            end_keyline_steps,
            available_space,
            item_spacing,
            before_content_padding,
            after_content_padding,
            start_shift_distance,
            end_shift_distance,
            start_shift_points,
            end_shift_points,
            is_valid,
            last_start_and_end_keyline_list_steps: OnceLock::new(),
        }
    }

    /// A strategy for `default_keylines` in a container of `available_space` along its axis (Compose's public
    /// constructor `Strategy(defaultKeylines, availableSpace, itemSpacing, beforeContentPadding,
    /// afterContentPadding)`): it derives the start and end shifting steps.
    pub(crate) fn new(
        default_keylines: KeylineList,
        available_space: f32,
        item_spacing: f32,
        before_content_padding: f32,
        after_content_padding: f32,
    ) -> Strategy {
        let start_keyline_steps =
            get_start_keyline_steps(&default_keylines, available_space, item_spacing, before_content_padding);
        let end_keyline_steps =
            get_end_keyline_steps(&default_keylines, available_space, item_spacing, after_content_padding);
        Strategy::with_steps(
            default_keylines,
            start_keyline_steps,
            end_keyline_steps,
            available_space,
            item_spacing,
            before_content_padding,
            after_content_padding,
        )
    }

    /// The strategy of no keylines (Compose's `Strategy.Empty`).
    pub(crate) fn empty() -> Strategy {
        Strategy::with_steps(KeylineList::empty(), Vec::new(), Vec::new(), 0.0, 0.0, 0.0, 0.0)
    }

    /// The size of items when focal (fully unmasked).
    pub(crate) fn item_main_axis_size(&self) -> f32 {
        self.default_keylines.first_focal().size
    }

    /// The [`KeylineList`] for `scroll_offset` of `max_scroll_offset`: the default keylines between the shift
    /// ranges, else the steps interpolated (`round_to_nearest_step`, Kotlin's default false: the nearest whole step).
    pub(crate) fn get_keyline_list_for_scroll_offset(
        &self,
        scroll_offset: f32,
        max_scroll_offset: f32,
        round_to_nearest_step: bool,
    ) -> KeylineList {
        // Rounding can make the offset slightly negative.
        let positive_scroll_offset = 0.0f32.max(scroll_offset);
        let start_shift_offset = self.start_shift_distance;
        let end_shift_offset = 0.0f32.max(max_scroll_offset - self.end_shift_distance);

        // Within neither shift range: the default keylines.
        if (start_shift_offset..=end_shift_offset).contains(&positive_scroll_offset) {
            return self.default_keylines.clone();
        }

        let mut interpolation = lerp_range(1.0, 0.0, 0.0, start_shift_offset, positive_scroll_offset);
        let mut shift_points = &self.start_shift_points;
        let mut steps: &[KeylineList] = &self.start_keyline_steps;

        if positive_scroll_offset > end_shift_offset {
            interpolation = lerp_range(0.0, 1.0, end_shift_offset, max_scroll_offset, positive_scroll_offset);
            shift_points = &self.end_shift_points;
            steps = &self.end_keyline_steps;

            // End shifting from the list's start (< 0.01 for float imprecision): interpolate from the last start
            // step straight to the last end step, skipping the default one. An end steps' size of 2 always has shift
            // points [0, 1], so interpolation and points stay right.
            if end_shift_offset < 0.01 && self.start_keyline_steps.len() == 2 && self.end_keyline_steps.len() == 2 {
                steps = self.last_start_and_end_keyline_list_steps.get_or_init(|| {
                    vec![
                        self.start_keyline_steps.last().unwrap().clone(),
                        self.end_keyline_steps.last().unwrap().clone(),
                    ]
                });
            }
        }

        let shift_point_range = get_shift_point_range(steps.len() as i32, shift_points, interpolation);

        if round_to_nearest_step {
            // Kotlin's roundToInt (half up; the interpolation is within 0..1).
            let rounded_step_index = if shift_point_range.stepped_interpolation.round() as i32 == 0 {
                shift_point_range.from_step_index
            } else {
                shift_point_range.to_step_index
            };
            return steps[rounded_step_index as usize].clone();
        }

        lerp_keyline_list(
            &steps[shift_point_range.from_step_index as usize],
            &steps[shift_point_range.to_step_index as usize],
            shift_point_range.stepped_interpolation,
        )
    }
}

impl PartialEq for Strategy {
    fn eq(&self, other: &Strategy) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        // Two invalid strategies are equal.
        if !self.is_valid && !other.is_valid {
            return true;
        }
        // Only the default keylines are compared of the lists: the others derive from them.
        self.is_valid == other.is_valid
            && self.available_space == other.available_space
            && self.item_spacing == other.item_spacing
            && self.before_content_padding == other.before_content_padding
            && self.after_content_padding == other.after_content_padding
            && self.item_main_axis_size() == other.item_main_axis_size()
            && self.start_shift_distance == other.start_shift_distance
            && self.end_shift_distance == other.end_shift_distance
            && self.start_shift_points == other.start_shift_points
            && self.end_shift_points == other.end_shift_points
            && self.default_keylines == other.default_keylines
    }
}

/// The scroll offset through all start steps (at least the start padding).
fn get_start_shift_distance(start_keyline_steps: &[KeylineList], before_content_padding: f32) -> f32 {
    let (Some(first), Some(last)) = (start_keyline_steps.first(), start_keyline_steps.last()) else {
        return 0.0;
    };
    (last[0].unadjusted_offset - first[0].unadjusted_offset).max(before_content_padding)
}

/// The scroll offset through all end steps (at least the end padding).
fn get_end_shift_distance(end_keyline_steps: &[KeylineList], after_content_padding: f32) -> f32 {
    let (Some(first), Some(last)) = (end_keyline_steps.first(), end_keyline_steps.last()) else {
        return 0.0;
    };
    (first[first.len() - 1].unadjusted_offset - last[last.len() - 1].unadjusted_offset).max(after_content_padding)
}

/// The steps moving the focal range from its default place to the container's start, one keyline a step (Compose's
/// `getStartKeylineSteps`): each removes the keyline at the start and re-inserts it after the focal range beside a
/// keyline of its neighbour's size (keeping the visual balance). The first step is the default list, the last the
/// focal range at the start (shrunk by the start padding when there is one).
fn get_start_keyline_steps(
    default_keylines: &KeylineList,
    carousel_main_axis_size: f32,
    item_spacing: f32,
    before_content_padding: f32,
) -> Vec<KeylineList> {
    if default_keylines.is_empty() {
        return Vec::new();
    }

    let mut steps: Vec<KeylineList> = vec![default_keylines.clone()];

    if default_keylines.is_first_focal_item_at_start_of_container() {
        if before_content_padding != 0.0 {
            steps.push(create_shifted_keyline_list_for_content_padding(
                default_keylines,
                carousel_main_axis_size,
                item_spacing,
                before_content_padding,
                default_keylines.first_focal(),
                default_keylines.first_focal_index,
            ));
        }
        return steps;
    }

    let start_index = default_keylines.first_non_anchor_index;
    let end_index = default_keylines.first_focal_index;
    let number_of_steps = end_index - start_index;

    // No steps but a cutoff to account for: one list shifted for it.
    if number_of_steps <= 0 && default_keylines.first_focal().cutoff > 0.0 {
        steps.push(move_keyline_and_create_shifted_keyline_list(
            default_keylines,
            0,
            0,
            carousel_main_axis_size,
            item_spacing,
        ));
        return steps;
    }

    for i in 0..number_of_steps {
        let prev_step = steps.last().unwrap();
        let original_item_index = start_index + i;
        let mut dst_index = default_keylines.last_index();
        if original_item_index > 0 {
            let original_neighbor_before_size = default_keylines[(original_item_index - 1) as usize].size;
            dst_index = prev_step.first_index_after_focal_range_with_size(original_neighbor_before_size) - 1;
        }

        let step = move_keyline_and_create_shifted_keyline_list(
            prev_step,
            default_keylines.first_non_anchor_index,
            dst_index,
            carousel_main_axis_size,
            item_spacing,
        );
        steps.push(step);
    }

    if before_content_padding != 0.0 {
        let last = steps.last().unwrap();
        let shifted = create_shifted_keyline_list_for_content_padding(
            last,
            carousel_main_axis_size,
            item_spacing,
            before_content_padding,
            last.first_focal(),
            last.first_focal_index,
        );
        *steps.last_mut().unwrap() = shifted;
    }

    steps
}

/// The steps moving the focal range from its default place to the container's end (Compose's
/// `getEndKeylineSteps`): each removes the keyline at the end and re-inserts it before the focal range beside a
/// keyline of its neighbour's size. The first step is the default list, the last the focal range at the end (shrunk
/// by the end padding when there is one).
fn get_end_keyline_steps(
    default_keylines: &KeylineList,
    carousel_main_axis_size: f32,
    item_spacing: f32,
    after_content_padding: f32,
) -> Vec<KeylineList> {
    if default_keylines.is_empty() {
        return Vec::new();
    }
    let mut steps: Vec<KeylineList> = vec![default_keylines.clone()];

    if default_keylines.is_last_focal_item_at_end_of_container(carousel_main_axis_size) {
        if after_content_padding != 0.0 {
            steps.push(create_shifted_keyline_list_for_content_padding(
                default_keylines,
                carousel_main_axis_size,
                item_spacing,
                -after_content_padding,
                default_keylines.last_focal(),
                default_keylines.last_focal_index,
            ));
        }
        return steps;
    }

    let start_index = default_keylines.last_focal_index;
    let end_index = default_keylines.last_non_anchor_index;
    let number_of_steps = end_index - start_index;

    // No steps but a cutoff to account for: one list shifted for it.
    if number_of_steps <= 0 && default_keylines.last_focal().cutoff > 0.0 {
        steps.push(move_keyline_and_create_shifted_keyline_list(
            default_keylines,
            0,
            0,
            carousel_main_axis_size,
            item_spacing,
        ));
        return steps;
    }

    for i in 0..number_of_steps {
        let prev_step = steps.last().unwrap();
        let original_item_index = end_index - i;
        let mut dst_index = 0;

        if original_item_index < default_keylines.last_index() {
            let original_neighbor_after_size = default_keylines[(original_item_index + 1) as usize].size;
            dst_index = prev_step.last_index_before_focal_range_with_size(original_neighbor_after_size) + 1;
        }

        let keylines = move_keyline_and_create_shifted_keyline_list(
            prev_step,
            default_keylines.last_non_anchor_index,
            dst_index,
            carousel_main_axis_size,
            item_spacing,
        );
        steps.push(keylines);
    }

    if after_content_padding != 0.0 {
        let last = steps.last().unwrap();
        let shifted = create_shifted_keyline_list_for_content_padding(
            last,
            carousel_main_axis_size,
            item_spacing,
            -after_content_padding,
            last.last_focal(),
            last.last_focal_index,
        );
        *steps.last_mut().unwrap() = shifted;
    }

    steps
}

/// `from` with each non-anchor item shrunk by its share of `content_padding` and offsets moved by it, unadjusted
/// offsets kept (the pager still lays items out end to end at the original page size).
fn create_shifted_keyline_list_for_content_padding(
    from: &KeylineList,
    carousel_main_axis_size: f32,
    item_spacing: f32,
    content_padding: f32,
    pivot: Keyline,
    pivot_index: i32,
) -> KeylineList {
    let number_of_non_anchor_keylines = from.iter().filter(|k| !k.is_anchor).count();
    let size_reduction = content_padding / number_of_non_anchor_keylines as f32;
    // keyline_list_of makes the offsets for each item's reduced size.
    let new_keylines = keyline_list_of_pivot(
        carousel_main_axis_size,
        item_spacing,
        pivot_index,
        pivot.offset - (size_reduction / 2.0) + content_padding,
        |scope| {
            for k in from.iter() {
                scope.add(k.size - size_reduction.abs(), k.is_anchor);
            }
        },
    );

    // Each item's unadjusted offset back to the incoming list's.
    KeylineList::new(
        new_keylines
            .iter()
            .enumerate()
            .map(|(i, k)| Keyline { unadjusted_offset: from[i].unadjusted_offset, ..*k })
            .collect(),
    )
}

/// `from` with the keyline at `src_index` moved to `dst_index`, the pivot and offsets updated for the focal shift.
fn move_keyline_and_create_shifted_keyline_list(
    from: &KeylineList,
    src_index: i32,
    dst_index: i32,
    carousel_main_axis_size: f32,
    item_spacing: f32,
) -> KeylineList {
    // -1 when the pivot shifts towards the start, 1 towards the end.
    let pivot_dir = if src_index > dst_index { 1 } else { -1 };
    let src = from[src_index as usize];
    let pivot_delta = (src.size - src.cutoff + item_spacing) * pivot_dir as f32;
    let new_pivot_index = from.pivot_index + pivot_dir;
    let new_pivot_offset = from.pivot().offset + pivot_delta;
    keyline_list_of_pivot(carousel_main_axis_size, item_spacing, new_pivot_index, new_pivot_offset, |scope| {
        let mut moved: Vec<Keyline> = from.to_vec();
        let keyline = moved.remove(src_index as usize);
        moved.insert(dst_index as usize, keyline);
        for k in &moved {
            scope.add(k.size, k.is_anchor);
        }
    })
}

/// For each of `steps`, the point in 0..1 of an interpolation through them where it is shown (Compose's
/// `getStepInterpolationPoints`): spaced by how far keylines shift between steps over `total_shift_distance`, the
/// last 1. `is_shifting_left`: the start steps (else the end ones).
fn get_step_interpolation_points(total_shift_distance: f32, steps: &[KeylineList], is_shifting_left: bool) -> Vec<f32> {
    let mut points = vec![0.0f32];
    if total_shift_distance == 0.0 || steps.is_empty() {
        return points;
    }

    for i in 1..steps.len() {
        let prev_keylines = &steps[i - 1];
        let curr_keylines = &steps[i];
        let distance_shifted = if is_shifting_left {
            curr_keylines[0].unadjusted_offset - prev_keylines[0].unadjusted_offset
        } else {
            prev_keylines[prev_keylines.len() - 1].unadjusted_offset
                - curr_keylines[curr_keylines.len() - 1].unadjusted_offset
        };
        let step_percentage = distance_shifted / total_shift_distance;
        let point = if i == steps.len() - 1 { 1.0 } else { points[i - 1] + step_percentage };
        points.push(point);
    }
    points
}

struct ShiftPointRange {
    from_step_index: i32,
    to_step_index: i32,
    stepped_interpolation: f32,
}

fn get_shift_point_range(steps_count: i32, shift_point: &[f32], interpolation: f32) -> ShiftPointRange {
    let mut lower_bounds = shift_point[0];
    for i in 1..steps_count {
        let upper_bounds = shift_point[i as usize];
        if interpolation <= upper_bounds {
            return ShiftPointRange {
                from_step_index: i - 1,
                to_step_index: i,
                stepped_interpolation: lerp_range(0.0, 1.0, lower_bounds, upper_bounds, interpolation),
            };
        }
        lower_bounds = upper_bounds;
    }
    ShiftPointRange { from_step_index: 0, to_step_index: 0, stepped_interpolation: 0.0 }
}

/// `value` mapped from `input_min..input_max` onto `output_min..output_max`, clamped to the ends (Compose's internal
/// carousel `lerp(outputMin, outputMax, inputMin, inputMax, value)`).
pub(crate) fn lerp_range(output_min: f32, output_max: f32, input_min: f32, input_max: f32, value: f32) -> f32 {
    if value <= input_min {
        return output_min;
    } else if value >= input_max {
        return output_max;
    }
    lerp_float(output_min, output_max, (value - input_min) / (input_max - input_min))
}

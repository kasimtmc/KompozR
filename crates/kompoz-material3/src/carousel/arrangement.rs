//! A combination of large, medium and small items fitted to a carousel's space (Compose's internal
//! `carousel.Arrangement`).

/// Data about a combination of large, medium and small items: knows how to alter an arrangement to fit within an
/// available space and assesses the arrangement's desirability by a priority heuristic (Compose's internal
/// `Arrangement`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Arrangement {
    priority: i32,
    pub(crate) small_size: f32,
    pub(crate) small_count: i32,
    pub(crate) medium_size: f32,
    pub(crate) medium_count: i32,
    pub(crate) large_size: f32,
    pub(crate) large_count: i32,
}

/// A percentage of a medium item's size by which it can grow or shrink to help fit an arrangement into the
/// carousel's space.
const MediumItemFlexPercentage: f32 = 0.1;

impl Arrangement {
    pub(crate) fn new(
        priority: i32,
        small_size: f32,
        small_count: i32,
        medium_size: f32,
        medium_count: i32,
        large_size: f32,
        large_count: i32,
    ) -> Arrangement {
        Arrangement { priority, small_size, small_count, medium_size, medium_count, large_size, large_count }
    }

    fn is_valid(&self) -> bool {
        if self.large_count > 0 && self.small_count > 0 && self.medium_count > 0 {
            return self.large_size > self.medium_size && self.medium_size > self.small_size;
        } else if self.large_count > 0 && self.small_count > 0 {
            return self.large_size > self.small_size;
        }
        true
    }

    /// The cost of this arrangement — lower for a priority closer to 1 and a large size altered less from
    /// `target_large_size`; `f32::MAX` when invalid.
    fn cost(&self, target_large_size: f32) -> f32 {
        if !self.is_valid() {
            return f32::MAX;
        }
        (target_large_size - self.large_size).abs() * self.priority as f32
    }

    /// The number of items (keylines) in the arrangement.
    pub(crate) fn item_count(&self) -> i32 {
        self.large_count + self.medium_count + self.small_count
    }

    /// Fits every permutation of the counts (in priority order: large, then medium, then small counts) into
    /// `available_space` and gives the one of the lowest cost (Compose's `findLowestCostArrangement`). The result may
    /// still take more or less than the space: the caller fits it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn find_lowest_cost_arrangement(
        available_space: f32,
        item_spacing: f32,
        target_small_size: f32,
        min_small_size: f32,
        max_small_size: f32,
        small_counts: &[i32],
        target_medium_size: f32,
        medium_counts: &[i32],
        target_large_size: f32,
        large_counts: &[i32],
    ) -> Option<Arrangement> {
        let mut lowest_cost_arrangement: Option<Arrangement> = None;
        let mut priority = 1;
        for &large_count in large_counts {
            for &medium_count in medium_counts {
                for &small_count in small_counts {
                    let arrangement = Arrangement::fit(
                        priority,
                        available_space,
                        item_spacing,
                        small_count,
                        target_small_size,
                        min_small_size,
                        max_small_size,
                        medium_count,
                        target_medium_size,
                        large_count,
                        target_large_size,
                    );
                    let better = match &lowest_cost_arrangement {
                        None => true,
                        Some(lowest) => arrangement.cost(target_large_size) < lowest.cost(target_large_size),
                    };
                    if better {
                        lowest_cost_arrangement = Some(arrangement);
                        // A cost of 0 didn't alter the large size, and permutations come in priority order: none
                        // better follows.
                        if arrangement.cost(target_large_size) == 0.0 {
                            return lowest_cost_arrangement;
                        }
                    }
                    priority += 1;
                }
            }
        }
        lowest_cost_arrangement
    }

    /// An arrangement of the counts fitted into `available_space`: small items adjusted as much as allowed first,
    /// then the large size solved for, then the medium items flexed to bring it back towards its target (Compose's
    /// `fit`).
    #[allow(clippy::too_many_arguments)]
    fn fit(
        priority: i32,
        available_space: f32,
        item_spacing: f32,
        small_count: i32,
        small_size: f32,
        min_small_size: f32,
        max_small_size: f32,
        medium_count: i32,
        medium_size: f32,
        large_count: i32,
        large_size: f32,
    ) -> Arrangement {
        let total_item_count = large_count + medium_count + small_count;
        let available_space_without_spacing = available_space - ((total_item_count - 1) as f32 * item_spacing);
        // Kotlin's coerceIn: panics on an empty range as it throws.
        let mut arranged_small_size = small_size.clamp(min_small_size, max_small_size);
        let arranged_medium_size = medium_size;
        let arranged_large_size = large_size;

        let total_space_taken_by_arrangement = arranged_large_size * large_count as f32
            + arranged_medium_size * medium_count as f32
            + arranged_small_size * small_count as f32;
        let delta = available_space_without_spacing - total_space_taken_by_arrangement;
        // First, resize small items within their allowed range to try to fit the arrangement into the space.
        if small_count > 0 && delta > 0.0 {
            arranged_small_size += (delta / small_count as f32).min(max_small_size - arranged_small_size);
        } else if small_count > 0 && delta < 0.0 {
            arranged_small_size += (delta / small_count as f32).max(min_small_size - arranged_small_size);
        }

        // Zero out the small size if there are no small items.
        arranged_small_size = if small_count > 0 { arranged_small_size } else { 0.0 };
        let mut arranged_large_size = Arrangement::calculate_large_size(
            available_space_without_spacing,
            small_count,
            arranged_small_size,
            medium_count,
            large_count,
        );
        let mut arranged_medium_size = (arranged_large_size + arranged_small_size) / 2.0;

        // If the large size moved away from its target to fit, counter it as much as possible by flexing the medium
        // item within its range.
        if medium_count > 0 && arranged_large_size != large_size {
            let target_adjustment = (large_size - arranged_large_size) * large_count as f32;
            let available_medium_flex = arranged_medium_size * MediumItemFlexPercentage * medium_count as f32;
            let distribute = target_adjustment.abs().min(available_medium_flex);
            if target_adjustment > 0.0 {
                // Shrink the medium item and give it back to the large items.
                arranged_medium_size -= distribute / medium_count as f32;
                arranged_large_size += distribute / large_count as f32;
            } else {
                // Grow the medium item and take from the large items.
                arranged_medium_size += distribute / medium_count as f32;
                arranged_large_size -= distribute / large_count as f32;
            }
        }

        Arrangement::new(
            priority,
            arranged_small_size,
            small_count,
            arranged_medium_size,
            medium_count,
            arranged_large_size,
            large_count,
        )
    }

    /// The large size fitting `available_space` with the counts, the small size, and the medium size the mean of the
    /// large and small ones: `available = large·L + ((large + small) / 2)·M + small·S` solved for `large`.
    fn calculate_large_size(
        available_space: f32,
        small_count: i32,
        small_size: f32,
        medium_count: i32,
        large_count: i32,
    ) -> f32 {
        (available_space - (small_count as f32 + medium_count as f32 / 2.0) * small_size)
            / (large_count as f32 + medium_count as f32 / 2.0)
    }
}

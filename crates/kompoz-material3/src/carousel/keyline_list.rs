//! Keylines and keyline lists, the structure every carousel is built on (Compose's internal `KeylineList.kt`).

use super::carousel::CarouselAlignment;
use kompoz_graphics::lerp as lerp_float;
use std::ops::Deref;

/// A structure fixed at an `offset` along a scrolling axis defining an item's properties when its center is at that
/// offset (Compose's internal `Keyline`). A scrolled item is always between two keylines, and its distance between
/// them is the fraction its interpolated keyline (its size and translation) is made with.
///
/// - `size`: the size an item should be in pixels when its center is at `offset`;
/// - `offset`: where along the scrolling axis an item's center should be (usually translated to) when it is at
///   `unadjusted_offset` in the end-to-end scrolling model;
/// - `unadjusted_offset`: the keyline's location in the end-to-end model (all items at their full size, end to end);
/// - `is_focal`: whether an item at this keyline is focal ("fully viewable");
/// - `is_anchor`: whether this keyline can't be shifted within its list;
/// - `is_pivot`: whether this is the keyline all others' offsets were calculated from;
/// - `cutoff`: how far an item here bleeds beyond the container (0 when fully in or fully out).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Keyline {
    pub(crate) size: f32,
    pub(crate) offset: f32,
    pub(crate) unadjusted_offset: f32,
    pub(crate) is_focal: bool,
    pub(crate) is_anchor: bool,
    pub(crate) is_pivot: bool,
    pub(crate) cutoff: f32,
}

/// A list of [`Keyline`]s with what carousels ask of it (Compose's internal `KeylineList`, a `List<Keyline>`: here
/// it derefs to a slice). Its constructor is for interpolated lists; a strategy's or a shifted step's is made by
/// [`keyline_list_of_alignment`] / [`keyline_list_of_pivot`], which set every offset from a pivot keyline.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KeylineList {
    keylines: Vec<Keyline>,
    /// The index of the pivot keyline (-1 when none).
    pub(crate) pivot_index: i32,
    /// The index of the first non-anchor keyline, or -1.
    pub(crate) first_non_anchor_index: i32,
    /// The index of the last non-anchor keyline, or -1.
    pub(crate) last_non_anchor_index: i32,
    /// The index of the first focal keyline, or -1.
    pub(crate) first_focal_index: i32,
    /// The index of the last focal keyline, or -1.
    pub(crate) last_focal_index: i32,
    /// The number of focal keylines.
    pub(crate) focal_count: i32,
}

impl Deref for KeylineList {
    type Target = [Keyline];

    fn deref(&self) -> &[Keyline] {
        &self.keylines
    }
}

fn index_of_first(keylines: &[Keyline], f: impl Fn(&Keyline) -> bool) -> i32 {
    keylines.iter().position(f).map_or(-1, |i| i as i32)
}

fn index_of_last(keylines: &[Keyline], f: impl Fn(&Keyline) -> bool) -> i32 {
    keylines.iter().rposition(f).map_or(-1, |i| i as i32)
}

impl KeylineList {
    pub(crate) fn new(keylines: Vec<Keyline>) -> KeylineList {
        let pivot_index = index_of_first(&keylines, |k| k.is_pivot);
        let first_non_anchor_index = index_of_first(&keylines, |k| !k.is_anchor);
        let last_non_anchor_index = index_of_last(&keylines, |k| !k.is_anchor);
        let first_focal_index = index_of_first(&keylines, |k| k.is_focal);
        let last_focal_index = index_of_last(&keylines, |k| k.is_focal);
        KeylineList {
            keylines,
            pivot_index,
            first_non_anchor_index,
            last_non_anchor_index,
            first_focal_index,
            last_focal_index,
            focal_count: last_focal_index - first_focal_index + 1,
        }
    }

    /// The empty list (Compose's `KeylineList.Empty`, `emptyKeylineList()`).
    pub(crate) fn empty() -> KeylineList {
        KeylineList::new(Vec::new())
    }

    fn get_or_none(&self, index: i32) -> Option<Keyline> {
        usize::try_from(index).ok().and_then(|i| self.keylines.get(i).copied())
    }

    fn at(&self, index: i32) -> Keyline {
        assert!(
            index >= 0 && (index as usize) < self.keylines.len(),
            "Index {index} out of bounds for length {}",
            self.keylines.len()
        );
        self.keylines[index as usize]
    }

    /// The keyline all other offsets were calculated from.
    pub(crate) fn pivot(&self) -> Keyline {
        self.at(self.pivot_index)
    }

    /// The first non-anchor keyline (panics when there is none, as Compose's throws).
    pub(crate) fn first_non_anchor(&self) -> Keyline {
        self.at(self.first_non_anchor_index)
    }

    /// The last non-anchor keyline (panics when there is none).
    pub(crate) fn last_non_anchor(&self) -> Keyline {
        self.at(self.last_non_anchor_index)
    }

    /// The first focal keyline: every keyline list has one (panics otherwise, as Compose's throws).
    pub(crate) fn first_focal(&self) -> Keyline {
        match self.get_or_none(self.first_focal_index) {
            Some(k) => k,
            None => panic!("All KeylineLists must have at least one focal keyline"),
        }
    }

    /// The last focal keyline (panics when there is none).
    pub(crate) fn last_focal(&self) -> Keyline {
        match self.get_or_none(self.last_focal_index) {
            Some(k) => k,
            None => panic!("All KeylineLists must have at least one focal keyline"),
        }
    }

    /// The last index (Kotlin's `lastIndex`: -1 when empty).
    pub(crate) fn last_index(&self) -> i32 {
        self.keylines.len() as i32 - 1
    }

    /// Whether the first focal item's start is in the container and it is the first non-anchor keyline: the focal
    /// range can't shift further towards the start.
    pub(crate) fn is_first_focal_item_at_start_of_container(&self) -> bool {
        let first_focal = self.first_focal();
        let first_focal_left = first_focal.offset - (first_focal.size / 2.0);
        first_focal_left >= 0.0 && first_focal == self.first_non_anchor()
    }

    /// Whether the last focal item's end is in the container and it is the last non-anchor keyline: the focal range
    /// can't shift further towards the end.
    pub(crate) fn is_last_focal_item_at_end_of_container(&self, carousel_main_axis_size: f32) -> bool {
        let last_focal = self.last_focal();
        let last_focal_right = last_focal.offset + (last_focal.size / 2.0);
        last_focal_right <= carousel_main_axis_size && last_focal == self.last_non_anchor()
    }

    /// The index of the first keyline from the last focal one on whose size is `size`, else the last index (where a
    /// keyline moved across the focal range keeps the list's balance when shifting).
    pub(crate) fn first_index_after_focal_range_with_size(&self, size: f32) -> i32 {
        let from = self.last_focal_index;
        let to = self.last_index();
        (from..=to).find(|&i| self.at(i).size == size).unwrap_or(to)
    }

    /// The index of the last keyline before the focal range whose size is `size`, else 0.
    pub(crate) fn last_index_before_focal_range_with_size(&self, size: f32) -> i32 {
        let from = self.first_focal_index - 1;
        let to = 0;
        (to..=from).rev().find(|&i| self.at(i).size == size).unwrap_or(to)
    }

    /// The last keyline whose unadjusted offset is less than `unadjusted_offset`, else the first.
    pub(crate) fn get_keyline_before(&self, unadjusted_offset: f32) -> Keyline {
        for k in self.keylines.iter().rev() {
            if k.unadjusted_offset < unadjusted_offset {
                return *k;
            }
        }
        *self.keylines.first().expect("List is empty.")
    }

    /// The first keyline whose unadjusted offset is at least `unadjusted_offset`, else the last.
    pub(crate) fn get_keyline_after(&self, unadjusted_offset: f32) -> Keyline {
        match self.keylines.iter().find(|k| k.unadjusted_offset >= unadjusted_offset) {
            Some(k) => *k,
            None => *self.keylines.last().expect("List is empty."),
        }
    }
}

/// Adds keylines to a list made by [`keyline_list_of_alignment`] / [`keyline_list_of_pivot`] (Compose's internal
/// `KeylineListScope`).
pub(crate) trait KeylineListScope {
    /// Adds a keyline of an item `size` pixels big (in the order they appear); `is_anchor` for keylines never shifted
    /// — usually the first and last ones, fully off screen (Kotlin's default false).
    fn add(&mut self, size: f32, is_anchor: bool);
}

/// A [`KeylineList`] whose focal range is aligned in the container by `carousel_alignment` (Compose's
/// `keylineListOf(carouselMainAxisSize, itemSpacing, carouselAlignment, keylines)`).
pub(crate) fn keyline_list_of_alignment(
    carousel_main_axis_size: f32,
    item_spacing: f32,
    carousel_alignment: CarouselAlignment,
    keylines: impl FnOnce(&mut dyn KeylineListScope),
) -> KeylineList {
    let mut scope = KeylineListScopeImpl::default();
    keylines(&mut scope);
    scope.create_with_alignment(carousel_main_axis_size, item_spacing, carousel_alignment)
}

/// A [`KeylineList`] whose offsets are all calculated from the keyline at `pivot_index` placed at `pivot_offset`
/// (Compose's `keylineListOf(carouselMainAxisSize, itemSpacing, pivotIndex, pivotOffset, keylines)`).
pub(crate) fn keyline_list_of_pivot(
    carousel_main_axis_size: f32,
    item_spacing: f32,
    pivot_index: i32,
    pivot_offset: f32,
    keylines: impl FnOnce(&mut dyn KeylineListScope),
) -> KeylineList {
    let mut scope = KeylineListScopeImpl::default();
    keylines(&mut scope);
    scope.create_with_pivot(carousel_main_axis_size, item_spacing, pivot_index, pivot_offset)
}

#[derive(Clone, Copy)]
struct TmpKeyline {
    size: f32,
    is_anchor: bool,
}

struct KeylineListScopeImpl {
    first_focal_index: i32,
    focal_item_size: f32,
    tmp_keylines: Vec<TmpKeyline>,
}

impl Default for KeylineListScopeImpl {
    fn default() -> KeylineListScopeImpl {
        KeylineListScopeImpl { first_focal_index: -1, focal_item_size: 0.0, tmp_keylines: Vec::new() }
    }
}

impl KeylineListScope for KeylineListScopeImpl {
    fn add(&mut self, size: f32, is_anchor: bool) {
        self.tmp_keylines.push(TmpKeyline { size, is_anchor });
        // The first focal item is the first of the largest added; the last focal one is found at creation by walking
        // up from it while the size stays the focal size.
        if size > self.focal_item_size {
            self.first_focal_index = self.tmp_keylines.len() as i32 - 1;
            self.focal_item_size = size;
        }
    }
}

impl KeylineListScopeImpl {
    fn create_with_pivot(
        &self,
        carousel_main_axis_size: f32,
        item_spacing: f32,
        pivot_index: i32,
        pivot_offset: f32,
    ) -> KeylineList {
        let keylines = create_keylines_with_pivot(
            pivot_index,
            pivot_offset,
            self.first_focal_index,
            self.find_last_focal_index(),
            self.focal_item_size,
            carousel_main_axis_size,
            item_spacing,
            &self.tmp_keylines,
        );
        KeylineList::new(keylines)
    }

    fn create_with_alignment(
        &self,
        carousel_main_axis_size: f32,
        item_spacing: f32,
        carousel_alignment: CarouselAlignment,
    ) -> KeylineList {
        let last_focal_index = self.find_last_focal_index();
        let focal_item_count = last_focal_index - self.first_focal_index;
        let focal_item_size = self.focal_item_size;

        let pivot_index = self.first_focal_index;
        let pivot_offset = match carousel_alignment {
            CarouselAlignment::Center => {
                // With an even number of keylines the spacing is at the container's center: halve it before taking
                // the pivot item's center.
                let item_spacing_split =
                    if item_spacing == 0.0 || focal_item_count.rem_euclid(2) == 0 { 0.0 } else { item_spacing / 2.0 };
                // The spacings between the container's center and the first focal item.
                let item_space_counts = (focal_item_count / 2) as f32 * item_spacing;

                (carousel_main_axis_size / 2.0)
                    - ((focal_item_size / 2.0) * focal_item_count as f32)
                    - item_spacing_split
                    - item_space_counts
            }
            CarouselAlignment::End => carousel_main_axis_size - (focal_item_size / 2.0),
            // Start (Compose's `else`).
            _ => focal_item_size / 2.0,
        };

        let keylines = create_keylines_with_pivot(
            pivot_index,
            pivot_offset,
            self.first_focal_index,
            last_focal_index,
            focal_item_size,
            carousel_main_axis_size,
            item_spacing,
            &self.tmp_keylines,
        );
        KeylineList::new(keylines)
    }

    fn find_last_focal_index(&self) -> i32 {
        // The contiguous range of keylines of the focal size from the first focal one.
        let mut last_focal_index = self.first_focal_index;
        while last_focal_index < self.tmp_keylines.len() as i32 - 1
            && self.tmp_keylines[(last_focal_index + 1) as usize].size == self.focal_item_size
        {
            last_focal_index += 1;
        }
        last_focal_index
    }
}

/// The keylines of `tmp_keylines` with their offset, unadjusted offset and cutoff calculated from the pivot at
/// `pivot_index` placed at `pivot_offset` (where its center goes): those before it towards the start, those after it
/// towards the end, in their order. `item_main_axis_size` is the focal (unmasked) item's size.
#[allow(clippy::too_many_arguments)]
fn create_keylines_with_pivot(
    pivot_index: i32,
    pivot_offset: f32,
    first_focal_index: i32,
    last_focal_index: i32,
    item_main_axis_size: f32,
    carousel_main_axis_size: f32,
    item_spacing: f32,
    tmp_keylines: &[TmpKeyline],
) -> Vec<Keyline> {
    let pivot = tmp_keylines[pivot_index as usize];
    let focal_range = first_focal_index..=last_focal_index;
    let mut keylines: Vec<Keyline> = Vec::with_capacity(tmp_keylines.len());

    let pivot_cutoff = if is_cutoff_left(pivot.size, pivot_offset) {
        pivot_offset - (pivot.size / 2.0)
    } else if is_cutoff_right(pivot.size, pivot_offset, carousel_main_axis_size) {
        (pivot_offset + (pivot.size / 2.0)) - carousel_main_axis_size
    } else {
        0.0
    };
    keylines.push(Keyline {
        size: pivot.size,
        offset: pivot_offset,
        unadjusted_offset: pivot_offset,
        is_focal: focal_range.contains(&pivot_index),
        is_anchor: pivot.is_anchor,
        is_pivot: true,
        cutoff: pivot_cutoff,
    });

    // The keylines before the pivot, inserted at the front in their original order.
    let mut offset = pivot_offset - (item_main_axis_size / 2.0) - item_spacing;
    let mut unadjusted_offset = pivot_offset - (item_main_axis_size / 2.0) - item_spacing;
    for original_index in (0..pivot_index).rev() {
        let tmp = tmp_keylines[original_index as usize];
        let tmp_offset = offset - (tmp.size / 2.0);
        let tmp_unadjusted_offset = unadjusted_offset - (item_main_axis_size / 2.0);
        let cutoff = if is_cutoff_left(tmp.size, tmp_offset) { (tmp_offset - (tmp.size / 2.0)).abs() } else { 0.0 };
        keylines.insert(
            0,
            Keyline {
                size: tmp.size,
                offset: tmp_offset,
                unadjusted_offset: tmp_unadjusted_offset,
                is_focal: focal_range.contains(&original_index),
                is_anchor: tmp.is_anchor,
                is_pivot: false,
                cutoff,
            },
        );

        offset -= tmp.size + item_spacing;
        unadjusted_offset -= item_main_axis_size + item_spacing;
    }

    // The keylines after the pivot, appended in their order.
    offset = pivot_offset + (item_main_axis_size / 2.0) + item_spacing;
    unadjusted_offset = pivot_offset + (item_main_axis_size / 2.0) + item_spacing;
    for original_index in (pivot_index + 1)..tmp_keylines.len() as i32 {
        let tmp = tmp_keylines[original_index as usize];
        let tmp_offset = offset + (tmp.size / 2.0);
        let tmp_unadjusted_offset = unadjusted_offset + (item_main_axis_size / 2.0);
        let cutoff = if is_cutoff_right(tmp.size, tmp_offset, carousel_main_axis_size) {
            (tmp_offset + (tmp.size / 2.0)) - carousel_main_axis_size
        } else {
            0.0
        };
        keylines.push(Keyline {
            size: tmp.size,
            offset: tmp_offset,
            unadjusted_offset: tmp_unadjusted_offset,
            is_focal: focal_range.contains(&original_index),
            is_anchor: tmp.is_anchor,
            is_pivot: false,
            cutoff,
        });

        offset += tmp.size + item_spacing;
        unadjusted_offset += item_main_axis_size + item_spacing;
    }

    keylines
}

/// Whether an item of `size` centered at `offset` straddles the container's start (false when fully in or out).
fn is_cutoff_left(size: f32, offset: f32) -> bool {
    offset - (size / 2.0) < 0.0 && offset + (size / 2.0) > 0.0
}

/// Whether an item of `size` centered at `offset` straddles the container's end.
fn is_cutoff_right(size: f32, offset: f32, carousel_main_axis_size: f32) -> bool {
    offset - (size / 2.0) < carousel_main_axis_size && offset + (size / 2.0) > carousel_main_axis_size
}

/// The keyline between `start` and `end` at `fraction`: values interpolated, flags from the nearer one (Compose's
/// `lerp(start: Keyline, end: Keyline, fraction)`).
pub(crate) fn lerp_keyline(start: &Keyline, end: &Keyline, fraction: f32) -> Keyline {
    Keyline {
        size: lerp_float(start.size, end.size, fraction),
        offset: lerp_float(start.offset, end.offset, fraction),
        unadjusted_offset: lerp_float(start.unadjusted_offset, end.unadjusted_offset, fraction),
        is_focal: if fraction < 0.5 { start.is_focal } else { end.is_focal },
        is_anchor: if fraction < 0.5 { start.is_anchor } else { end.is_anchor },
        is_pivot: if fraction < 0.5 { start.is_pivot } else { end.is_pivot },
        cutoff: lerp_float(start.cutoff, end.cutoff, fraction),
    }
}

/// The list between `from` and `to` at `fraction`, keyline by keyline (Compose's `lerp(from: KeylineList, to,
/// fraction)`: unlike [`keyline_list_of_pivot`], no offsets recalculated from a pivot).
pub(crate) fn lerp_keyline_list(from: &KeylineList, to: &KeylineList, fraction: f32) -> KeylineList {
    let interpolated = from.iter().enumerate().map(|(i, k)| lerp_keyline(k, &to[i], fraction)).collect();
    KeylineList::new(interpolated)
}

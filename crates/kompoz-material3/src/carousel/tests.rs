//! The carousel's keyline math, checked as Compose's own unit tests check its internals (KeylineListTest,
//! StrategyTest, the arrangements of the keyline configurations): the values M15's code gives, worked out by hand
//! in the comments.

use super::arrangement::Arrangement;
use super::carousel::CarouselAlignment;
use super::keyline_list::{Keyline, KeylineList, keyline_list_of_alignment, lerp_keyline, lerp_keyline_list};
use super::keyline_snap_position::get_snap_position_offset;
use super::keylines::{hero_keyline_list, multi_browse_keyline_list, uncontained_keyline_list};
use super::strategy::{Strategy, lerp_range};
use kompoz_graphics::Density;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.01
}

fn sizes(list: &KeylineList) -> Vec<f32> {
    list.iter().map(|k| (k.size * 100.0).round() / 100.0).collect()
}

fn offsets(list: &KeylineList) -> Vec<f32> {
    list.iter().map(|k| (k.offset * 100.0).round() / 100.0).collect()
}

const D1: Density = Density { density: 1.0, font_scale: 1.0 };

/// The multi-browse keylines of 186 px items in 400 px, 8 apart.
fn browse() -> KeylineList {
    multi_browse_keyline_list(&D1, 400.0, 186.0, 8.0, 10, None, None)
}

#[test]
fn arrangements_fit_the_space_at_the_least_cost() {
    // Large 3, 2, 1 × medium 1, 0 × small 1: the one large, one medium, one small arrangement costs least (|186 −
    // 187.2| × priority 5): small 56 (grown by nothing: at its most), large (384 − 1.5 × 56) / 1.5 = 200, medium 128
    // flexed by its 10% (12.8) back towards the large target.
    let a = Arrangement::find_lowest_cost_arrangement(
        400.0,
        8.0,
        56.0,
        40.0,
        56.0,
        &[1],
        121.0,
        &[1, 0],
        186.0,
        &[3, 2, 1],
    )
    .unwrap();
    assert!(a.large_count == 1 && a.medium_count == 1 && a.small_count == 1 && a.item_count() == 3);
    assert!(near(a.large_size, 187.2) && near(a.medium_size, 140.8) && near(a.small_size, 56.0), "{a:?}");
    // A space the target sizes fill exactly costs nothing: found at once.
    let exact = Arrangement::find_lowest_cost_arrangement(250.0, 8.0, 56.0, 40.0, 56.0, &[1], 121.0, &[0], 186.0, &[1])
        .unwrap();
    assert!(near(exact.large_size, 186.0) && near(exact.small_size, 56.0) && exact.medium_count == 0);
    // Shrinking: small items go down to their least first.
    let tight =
        Arrangement::find_lowest_cost_arrangement(300.0, 8.0, 56.0, 40.0, 56.0, &[1], 0.0, &[0], 300.0, &[1]).unwrap();
    assert!(near(tight.small_size, 40.0) && near(tight.large_size, 252.0), "{tight:?}");
}

#[test]
fn multi_browse_keylines_place_large_medium_and_small_items_from_the_start() {
    let list = browse();
    assert_eq!(sizes(&list), vec![10.0, 187.2, 140.8, 56.0, 10.0]);
    // The first focal item's center at half its size; the others after it, 8 apart; the anchors off the ends.
    assert_eq!(offsets(&list), vec![-13.0, 93.6, 265.6, 372.0, 413.0]);
    let unadjusted: Vec<f32> = list.iter().map(|k| (k.unadjusted_offset * 10.0).round() / 10.0).collect();
    assert_eq!(unadjusted, vec![-101.6, 93.6, 288.8, 484.0, 679.2]);
    assert!(list[0].is_anchor && list[4].is_anchor && !list[1].is_anchor && list[1].is_pivot);
    assert!(list.first_focal_index == 1 && list.last_focal_index == 1 && list.focal_count == 1);
    assert!(list.iter().all(|k| k.cutoff == 0.0));
    assert!(list.is_first_focal_item_at_start_of_container() && !list.is_last_focal_item_at_end_of_container(400.0));
    // Fewer items than keylines: small ones, then medium ones beyond one, go (large ones stay): two items find two
    // large of 157.44 and a medium of 69.12 cheapest ((186 − 157.44) × priority 2).
    let two = multi_browse_keyline_list(&D1, 400.0, 186.0, 8.0, 2, None, None);
    assert_eq!(sizes(&two), vec![10.0, 157.44, 157.44, 69.12, 10.0]);
    // No space or no item size: no keylines; too narrow for a large and a small item: none small.
    assert!(multi_browse_keyline_list(&D1, 0.0, 186.0, 8.0, 10, None, None).is_empty());
    assert!(multi_browse_keyline_list(&D1, 400.0, 0.0, 8.0, 10, None, None).is_empty());
    let narrow = multi_browse_keyline_list(&D1, 70.0, 186.0, 0.0, 10, None, None);
    assert!(!narrow.is_empty() && narrow.iter().filter(|k| !k.is_anchor).all(|k| k.size >= 40.0), "{narrow:?}");
    // The small sizes given in px (Kotlin's defaults: 40 and 56 dp in the density).
    let dense =
        multi_browse_keyline_list(&Density { density: 2.0, font_scale: 1.0 }, 800.0, 372.0, 16.0, 10, None, None);
    assert_eq!(sizes(&dense), vec![20.0, 374.4, 281.6, 112.0, 20.0]);
}

#[test]
fn uncontained_keylines_keep_the_item_size_and_cut_one_off_at_the_end() {
    // 158 (the item and the spacing) fits twice in 400; the 84 left is a medium item of 126 (a third cut off), cut
    // off by 58 at the end; the start anchor half of it (63).
    let list = uncontained_keyline_list(&D1, 400.0, 150.0, 8.0);
    assert_eq!(sizes(&list), vec![63.0, 158.0, 158.0, 126.0, 10.0]);
    assert_eq!(offsets(&list), vec![-39.5, 79.0, 245.0, 395.0, 471.0]);
    assert!(near(list[3].cutoff, 58.0) && list.focal_count == 2);
    // A medium item too like the large one is held to 85% of it, or a fifth cut off, at most the large size.
    let similar = uncontained_keyline_list(&D1, 340.0, 192.0, 8.0);
    assert!(near(similar[2].size, 170.0), "{similar:?}");
    assert!(uncontained_keyline_list(&D1, 400.0, 0.0, 8.0).is_empty());
}

#[test]
fn hero_keylines_center_a_large_item_between_small_ones() {
    // Uncapped, the large target is the whole width: small items at their least (40), the large one 304, centered.
    let centered = hero_keyline_list(&D1, 400.0, None, 8.0, 6, true, None, None);
    assert_eq!(sizes(&centered), vec![10.0, 40.0, 304.0, 40.0, 10.0]);
    assert!(near(centered[2].offset, 200.0) && centered.focal_count == 1);
    // Capped at 200: two large items of 148 (the cheaper arrangement), centered as a pair around the spacing.
    let capped = hero_keyline_list(&D1, 400.0, Some(200.0), 8.0, 6, true, None, None);
    assert_eq!(sizes(&capped), vec![10.0, 40.0, 148.0, 148.0, 40.0, 10.0]);
    assert!(near(capped[2].offset, 122.0) && near(capped[3].offset, 278.0), "{capped:?}");
    // Fewer than three items aren't centered; one is alone and large.
    let pair = hero_keyline_list(&D1, 400.0, None, 8.0, 2, true, None, None);
    assert_eq!(sizes(&pair), vec![10.0, 352.0, 40.0, 10.0]);
    assert!(near(pair[1].offset, 176.0));
    let one = hero_keyline_list(&D1, 400.0, None, 8.0, 1, true, None, None);
    assert_eq!(sizes(&one), vec![10.0, 400.0, 10.0]);
    // Not centered: start-aligned with one small item.
    let start = hero_keyline_list(&D1, 400.0, None, 8.0, 6, false, None, None);
    assert_eq!(sizes(&start), vec![10.0, 352.0, 40.0, 10.0]);
    // Too narrow for the small items and a large one 25% larger: no small items.
    let narrow = hero_keyline_list(&D1, 120.0, None, 8.0, 6, true, None, None);
    assert_eq!(sizes(&narrow), vec![10.0, 120.0, 10.0]);
    assert!(hero_keyline_list(&D1, 0.0, None, 8.0, 6, true, None, None).is_empty());
}

#[test]
fn keyline_lists_align_and_answer_questions_about_themselves() {
    let make = |alignment| {
        keyline_list_of_alignment(300.0, 0.0, alignment, |s| {
            s.add(10.0, true);
            s.add(100.0, false);
            s.add(100.0, false);
            s.add(50.0, false);
            s.add(10.0, true);
        })
    };
    let start = make(CarouselAlignment::Start);
    assert_eq!(offsets(&start), vec![-5.0, 50.0, 150.0, 225.0, 255.0]);
    assert!(start.first_focal_index == 1 && start.last_focal_index == 2 && start.focal_count == 2);
    let end = make(CarouselAlignment::End);
    assert!(near(end.pivot().offset, 250.0) && end.pivot_index == 1, "{end:?}");
    // Centered: two focal items, the middle between them.
    let center = make(CarouselAlignment::Center);
    assert!(near(center[1].offset, 100.0) && near(center[2].offset, 200.0), "{center:?}");
    // Cut off at the end: the 50 item at 200..250 isn't; at the start the anchor is wholly out.
    assert!(start.iter().all(|k| k.cutoff == 0.0));
    // The keylines around an unadjusted offset: the last before it, the first at or after it.
    assert_eq!(start.get_keyline_before(120.0), start[1]);
    assert_eq!(start.get_keyline_after(120.0), start[2]);
    assert_eq!(start.get_keyline_before(-1000.0), start[0]);
    assert_eq!(start.get_keyline_after(1000.0), start[4]);
    // Where a keyline of a size is across the focal range (for shifting).
    assert_eq!(start.first_index_after_focal_range_with_size(50.0), 3);
    assert_eq!(start.first_index_after_focal_range_with_size(7.0), 4);
    assert_eq!(start.last_index_before_focal_range_with_size(10.0), 0);
    assert!(KeylineList::empty().is_empty() && KeylineList::empty().pivot_index == -1);
}

#[test]
fn keylines_interpolate_their_values_and_take_flags_from_the_nearer_one() {
    let a = Keyline {
        size: 10.0,
        offset: 0.0,
        unadjusted_offset: 0.0,
        is_focal: false,
        is_anchor: true,
        is_pivot: false,
        cutoff: 2.0,
    };
    let b = Keyline {
        size: 30.0,
        offset: 100.0,
        unadjusted_offset: 50.0,
        is_focal: true,
        is_anchor: false,
        is_pivot: true,
        cutoff: 0.0,
    };
    let quarter = lerp_keyline(&a, &b, 0.25);
    assert!(near(quarter.size, 15.0) && near(quarter.offset, 25.0) && near(quarter.unadjusted_offset, 12.5));
    assert!(!quarter.is_focal && quarter.is_anchor && !quarter.is_pivot && near(quarter.cutoff, 1.5));
    let half = lerp_keyline(&a, &b, 0.5);
    assert!(half.is_focal && !half.is_anchor && half.is_pivot);
    let lists = lerp_keyline_list(&KeylineList::new(vec![a, a]), &KeylineList::new(vec![b, b]), 1.0);
    assert!(lists.iter().all(|k| *k == b));
    // The clamped range lerp.
    assert!(near(lerp_range(1.0, 0.0, 0.0, 10.0, 2.5), 0.75) && lerp_range(0.0, 1.0, 5.0, 10.0, 1.0) == 0.0);
    assert!(lerp_range(0.0, 1.0, 5.0, 10.0, 11.0) == 1.0);
}

#[test]
fn strategies_shift_the_focal_range_to_the_ends() {
    let strategy = Strategy::new(browse(), 400.0, 8.0, 0.0, 0.0);
    assert!(strategy.is_valid && near(strategy.item_main_axis_size(), 187.2));
    // The focal range is at the start already: one start step; two end steps take it to the end, a keyline each.
    assert_eq!(strategy.start_keyline_steps.len(), 1);
    assert_eq!(strategy.end_keyline_steps.len(), 3);
    let last = strategy.end_keyline_steps.last().unwrap();
    assert_eq!(sizes(last), vec![10.0, 56.0, 140.8, 187.2, 10.0]);
    // (its focal item's end at 400 within float rounding: 2e-5 past it, so not "at the end" by the strict test)
    assert!(near(last.last_focal().offset + 93.6, 400.0) && last.pivot_index == 3, "{last:?}");
    // Ten items: the most it scrolls is 10 × 187.2 + 9 × 8 − 400.
    let max = 10.0 * 187.2 + 9.0 * 8.0 - 400.0;
    assert_eq!(strategy.get_keyline_list_for_scroll_offset(0.0, max, false), strategy.default_keylines);
    assert_eq!(strategy.get_keyline_list_for_scroll_offset(-0.1, max, false), strategy.default_keylines);
    assert_eq!(&strategy.get_keyline_list_for_scroll_offset(max, max, false), last);
    // Between the end steps: interpolated, or the nearest step when rounded.
    let first_end = &strategy.end_keyline_steps[1];
    let shift =
        strategy.end_keyline_steps[0].last().unwrap().unadjusted_offset - last.last().unwrap().unadjusted_offset;
    let from = max - shift;
    let between = strategy.get_keyline_list_for_scroll_offset(from + 1.0, max, false);
    assert!(between != strategy.default_keylines && &between != first_end);
    let rounded = strategy.get_keyline_list_for_scroll_offset(from + 1.0, max, true);
    assert_eq!(rounded, strategy.default_keylines);
    // Equal by their values; any two invalid ones are equal.
    assert_eq!(strategy, Strategy::new(browse(), 400.0, 8.0, 0.0, 0.0));
    assert_ne!(strategy, Strategy::new(browse(), 400.0, 8.0, 16.0, 0.0));
    assert_eq!(Strategy::empty(), Strategy::new(KeylineList::empty(), 300.0, 0.0, 0.0, 0.0));
    assert!(!Strategy::empty().is_valid);
}

#[test]
fn strategies_shrink_the_last_steps_for_the_content_padding() {
    // A start padding of 16 over three non-anchor items: each 16/3 smaller in an added start step, its unadjusted
    // offsets kept (the pager lays items out at their whole size).
    let strategy = Strategy::new(browse(), 400.0, 8.0, 16.0, 16.0);
    assert_eq!(strategy.start_keyline_steps.len(), 2);
    let padded = &strategy.start_keyline_steps[1];
    assert_eq!(sizes(padded), vec![4.67, 181.87, 135.47, 50.67, 4.67]);
    assert!(near(padded.first_focal().offset - padded.first_focal().size / 2.0, 16.0), "{padded:?}");
    let original: Vec<f32> = strategy.default_keylines.iter().map(|k| k.unadjusted_offset).collect();
    assert!(padded.iter().zip(original).all(|(k, u)| k.unadjusted_offset == u));
    // At scroll 0 the padded step is the one used.
    let max = 1000.0;
    assert_eq!(&strategy.get_keyline_list_for_scroll_offset(0.0, max, false), padded);
    // The end padding: the last end step's focal item ends 16 from the end.
    let last = strategy.end_keyline_steps.last().unwrap();
    assert!(near(last.last_focal().offset + last.last_focal().size / 2.0, 384.0), "{last:?}");
}

#[test]
fn items_snap_where_their_steps_put_them_in_focus() {
    let strategy = Strategy::new(browse(), 400.0, 8.0, 0.0, 0.0);
    // The first items and the middle ones: the first focal keyline at rest (0 from the start).
    for item in 0..=7 {
        assert_eq!(get_snap_position_offset(&strategy, item, 10), 0, "{item}");
    }
    // The last two: where the end steps put them in focus.
    let step = |i: usize| {
        let k = strategy.end_keyline_steps[i].last_focal();
        (k.unadjusted_offset - 93.6 + 0.5).floor() as i32
    };
    assert_eq!(get_snap_position_offset(&strategy, 8, 10), step(1));
    assert_eq!(get_snap_position_offset(&strategy, 9, 10), step(2));
    // All items focal at once: no end steps used.
    assert_eq!(get_snap_position_offset(&strategy, 0, 1), 0);
    assert_eq!(get_snap_position_offset(&Strategy::empty(), 3, 10), 0);
}

//! foundation.layout: Box, Row, Column and Spacer measured and placed, arrangements, the padding, size and scope
//! modifiers, intrinsics, alignment lines, right to left.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::layout::VerticalAlignmentLine;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};

// ---- Helpers ---------------------------------------------------------------------------------------------------

/// A node's outer bounds in the root: x, y, width, height (its test tag first in its modifier).
fn bounds(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    let b = rule.on_node_with_tag(tag).get_unclipped_bounds_in_root();
    [b.left, b.top, b.width(), b.height()]
}

/// Where a node's content is in the root (its inner coordinates: C++'s `contentBounds`): x, y, width, height.
fn content_bounds(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    let inner = rule.on_node_with_tag(tag).layout_node().inner_coordinates();
    let b = inner.find_root_coordinates().local_bounding_box_of(&inner, false);
    [b.left, b.top, b.width(), b.height()]
}

/// A node's measured size (for one of no area, whose bounds are zero).
fn size(rule: &KompozTestRule, tag: &str) -> IntSize {
    rule.on_node_with_tag(tag).layout_node().size()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

const DENSITY: Density = Density::new(1.0, 1.0);

fn horizontal(arrangement: &HorizontalArrangement, total: i32, sizes: &[i32], direction: LayoutDirection) -> Vec<i32> {
    let mut out = vec![0; sizes.len()];
    arrangement.arrange(&DENSITY, total, sizes, direction, &mut out);
    out
}

fn vertical(arrangement: &VerticalArrangement, total: i32, sizes: &[i32]) -> Vec<i32> {
    let mut out = vec![0; sizes.len()];
    arrangement.arrange(&DENSITY, total, sizes, &mut out);
    out
}

thread_local! {
    /// What a measuring layout saw of its child's intrinsic sizes.
    static SEEN: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    /// How many times a layout counting its measurements was measured.
    static MEASURES: Cell<u32> = const { Cell::new(0) };
}

fn seen() -> Vec<i32> {
    SEEN.with(|s| s.borrow().clone())
}

/// A layout that asks its child's intrinsic sizes — min and max width at `height`, min and max height at `width`
/// — keeps them in [`SEEN`], and measures and places it.
#[kompozable(inline)]
fn Intrinsics(height: i32, width: i32, content: impl FnOnce()) {
    Layout(
        content = content,
        measure_policy = measure_policy(move |scope, measurables, constraints| {
            let m = measurables[0];
            let values = vec![
                m.min_intrinsic_width(height),
                m.max_intrinsic_width(height),
                m.min_intrinsic_height(width),
                m.max_intrinsic_height(width),
            ];
            SEEN.with(|s| *s.borrow_mut() = values);
            let p = m.measure(constraints);
            scope.layout(p.width(), p.height(), move |s| s.place(&p, 0, 0))
        }),
    );
}

/// A leaf of a constant area: its intrinsic width at a height is the area over the height, and the other way
/// round; measured, its min constraints.
struct Area(i32);

impl Area {
    fn other_side(&self, side: i32) -> i32 {
        if side <= 0 || side == Constraints::Infinity { 0 } else { self.0 / side }
    }
}

impl MeasurePolicy for Area {
    fn measure(&self, scope: &MeasureScope, _: &[&dyn Measurable], c: Constraints) -> MeasureResult {
        scope.layout(c.min_width, c.min_height, |_| {})
    }
    fn min_intrinsic_width(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], height: i32) -> i32 {
        self.other_side(height)
    }
    fn max_intrinsic_width(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], height: i32) -> i32 {
        self.other_side(height)
    }
    fn min_intrinsic_height(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], width: i32) -> i32 {
        self.other_side(width)
    }
    fn max_intrinsic_height(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], width: i32) -> i32 {
        self.other_side(width)
    }
}

fn min_merge(a: i32, b: i32) -> i32 {
    a.min(b)
}

/// A vertical line a [`LineLeaf`] provides.
const Mark: VerticalAlignmentLine = VerticalAlignmentLine::new("Mark", min_merge);

/// A leaf `width` by 10 providing [`Mark`] at `mark`.
#[kompozable(inline)]
fn LineLeaf(width: i32, mark: i32, modifier: Modifier) {
    Layout(
        modifier = modifier,
        measure_policy =
            measure_policy(move |_, _, _| MeasureResult::new(width, 10, vec![(Mark.into(), mark)], |_| {})),
    );
}

/// A child of a C++ test: a box filling what it is given, tagged.
#[kompozable]
fn Child(tag: impl Into<String>) {
    Box(Modifier.test_tag(tag).fill_max_size());
}

fn measures() -> u32 {
    MEASURES.with(|m| m.get())
}

// ---- Ported from C++ -------------------------------------------------------------------------------------------

fn row_distributes_weight() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Row(Modifier.width(300.dp()), || {
            Box(Modifier.test_tag("a").weight(1.0).height(10.dp()));
            Box(Modifier.test_tag("b").weight(2.0).height(10.dp()));
        });
    }));
    let (a, b) = (bounds(&rule, "a"), bounds(&rule, "b"));
    assert!(near(a[2], 100.0));
    assert!(near(b[2], 200.0));
    assert!(near(b[0], 100.0));
}

fn padding_offsets_content() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(Modifier.padding(10.dp()), content = || Box(Modifier.test_tag("inner").size(50.dp())));
    }));
    let r = bounds(&rule, "inner");
    assert!(near(r[0], 10.0) && near(r[1], 10.0) && near(r[2], 50.0));
}

fn column_arrangement_and_alignment() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            Modifier.size_wh(200.dp(), 200.dp()),
            vertical_arrangement = Arrangement::spaced_by(10.dp()),
            horizontal_alignment = Alignment::CenterHorizontally,
            || {
                Box(Modifier.test_tag("first").size(50.dp()));
                Box(Modifier.test_tag("second").size_wh(100.dp(), 20.dp()));
            },
        );
    }));
    let (first, second) = (bounds(&rule, "first"), bounds(&rule, "second"));
    assert!(near(first[0], 75.0) && near(first[1], 0.0));
    assert!(near(second[0], 50.0) && near(second[1], 60.0));
}

fn box_alignment() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(Modifier.size(100.dp()), Alignment::Center, || {
            Box(Modifier.test_tag("centered").size(20.dp()));
            Box(Modifier.test_tag("corner").size(10.dp()).align(Alignment::BottomEnd));
        });
    }));
    assert!(near(bounds(&rule, "centered")[0], 40.0));
    let corner = bounds(&rule, "corner");
    assert!(near(corner[0], 90.0) && near(corner[1], 90.0));
}

fn required_size_ranges_ignore_the_incoming_constraints() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                // Wider than its 100: measured 160, centered over it.
                Box(
                    Modifier.test_tag("dar").size_wh(100.dp(), 40.dp()).required_width_in(150.dp(), max = 160.dp()),
                    content = || Child("dar-içi"),
                );
                // A max under the incoming min: 200 inside 300, centered.
                Box(
                    Modifier.test_tag("geniş").size_wh(300.dp(), 40.dp()).required_width_in(0.dp(), max = 200.dp()),
                    content = || Child("geniş-içi"),
                );
                // No max: the incoming one (all of its 300), at least its min.
                Box(
                    Modifier.size_wh(300.dp(), 40.dp()).required_width_in(min = 50.dp()),
                    content = || Child("serbest"),
                );
                // Heights too, and both at once.
                Box(
                    Modifier.test_tag("kutu").size_wh(100.dp(), 40.dp()).required_size_in(
                        120.dp(),
                        60.dp(),
                        130.dp(),
                        max_height = 70.dp(),
                    ),
                    content = || Child("kutu-içi"),
                );
                Box(
                    Modifier.size_wh(100.dp(), 100.dp()).required_height_in(10.dp(), max = 30.dp()),
                    content = || Child("alçak"),
                );
                Box(
                    Modifier.width_in(0.dp(), max = 80.dp()).height(20.dp()).required_width_in(min = 0.dp()),
                    content = || Child("sığar"),
                );
                // A min over the incoming max, no max given: the min; a min over its max: the max.
                Box(Modifier.size_wh(100.dp(), 20.dp()).required_width_in(min = 150.dp()), content = || Child("taşan"));
                Box(
                    Modifier.size_wh(300.dp(), 20.dp()).required_width_in(200.dp(), max = 100.dp()),
                    content = || Child("ters"),
                );
            },
        );
    }));
    let (narrow, inside) = (bounds(&rule, "dar"), bounds(&rule, "dar-içi"));
    assert!(near(inside[2], 160.0) && near(inside[0], narrow[0] - 30.0));
    let (wide, in_wide) = (bounds(&rule, "geniş"), bounds(&rule, "geniş-içi"));
    assert!(near(in_wide[2], 200.0) && near(in_wide[0], wide[0] + 50.0));
    assert!(near(bounds(&rule, "serbest")[2], 300.0));
    let (b, in_box) = (bounds(&rule, "kutu"), bounds(&rule, "kutu-içi"));
    assert!(
        near(in_box[2], 130.0) && near(in_box[3], 70.0) && near(in_box[0], b[0] - 15.0) && near(in_box[1], b[1] - 15.0)
    );
    assert!(near(bounds(&rule, "alçak")[3], 30.0) && near(bounds(&rule, "sığar")[2], 80.0));
    assert!(near(bounds(&rule, "taşan")[2], 150.0) && near(bounds(&rule, "ters")[2], 100.0));
}

fn size_in_and_default_min_size() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                // Empty: grows to the minimum.
                Box(Modifier.test_tag("small").size_in(50.dp(), 30.dp(), 100.dp(), max_height = 60.dp()));
                // Capped.
                Box(
                    Modifier.test_tag("big").size_in(50.dp(), 30.dp(), 100.dp(), max_height = 60.dp()),
                    content = || Box(Modifier.size_wh(300.dp(), 300.dp())),
                );
                // Nothing else says: the default.
                Box(Modifier.test_tag("default").default_min_size(80.dp(), min_height = 40.dp()));
                // A size from the caller wins.
                Box(Modifier
                    .test_tag("given")
                    .size_wh(20.dp(), 20.dp())
                    .default_min_size(80.dp(), min_height = 40.dp()));
            },
        );
    }));
    let size = |tag| {
        let b = bounds(&rule, tag);
        (b[2], b[3])
    };
    assert_eq!(size("small"), (50.0, 30.0));
    assert_eq!(size("big"), (100.0, 60.0));
    assert_eq!(size("default"), (80.0, 40.0));
    assert_eq!(size("given"), (20.0, 20.0));
}

fn rows_and_columns_with_weights_that_dont_fill_are_as_big_as_their_children() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            Modifier.size_wh(300.dp(), 500.dp()),
            content = || {
                Row(content = || {
                    // Not filling: each takes what it needs out of its share.
                    Column(
                        Modifier.test_tag("sütun"),
                        content = || {
                            Box(Modifier.size_wh(40.dp(), 60.dp()));
                            Box(Modifier.test_tag("ağır").weight(1.0, fill = false).size_wh(40.dp(), 50.dp()));
                        },
                    );
                    // Filling: all of the room.
                    Column(
                        Modifier.test_tag("dolu"),
                        content = || {
                            Box(Modifier.size_wh(40.dp(), 60.dp()));
                            Box(Modifier.test_tag("dolan").weight(1.0).width(40.dp()));
                        },
                    );
                    Row(
                        Modifier.test_tag("satır"),
                        content = || {
                            Box(Modifier.weight(1.0, fill = false).size_wh(30.dp(), 20.dp()));
                        },
                    );
                });
            },
        );
    }));
    assert!(near(bounds(&rule, "sütun")[3], 110.0) && near(bounds(&rule, "ağır")[3], 50.0));
    assert!(near(bounds(&rule, "dolu")[3], 500.0) && near(bounds(&rule, "dolan")[3], 440.0));
    assert!(near(bounds(&rule, "satır")[2], 30.0));
}

/// C++'s `aspect_ratio_wrap_content_and_required_size`, its wrap and required parts (aspect ratio is another
/// package's): a 200 by 100 box stands for its first, aspect-ratio box.
fn wrap_content_and_required_size() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Box(Modifier.size_wh(200.dp(), 100.dp()));
                Box(Modifier
                    .test_tag("wrapped")
                    .size_wh(300.dp(), 100.dp())
                    .wrap_content_size(align = Alignment::BottomEnd)
                    .size(50.dp()));
                Box(Modifier.test_tag("required").size(100.dp()).required_size(150.dp()));
            },
        );
    }));
    assert_eq!(bounds(&rule, "wrapped"), [0.0, 100.0, 300.0, 100.0]);
    assert_eq!(content_bounds(&rule, "wrapped"), [250.0, 150.0, 50.0, 50.0]);
    assert_eq!(bounds(&rule, "required"), [0.0, 200.0, 100.0, 100.0]);
    assert_eq!(content_bounds(&rule, "required"), [-25.0, 175.0, 150.0, 150.0]);
}

// ---- Arrangements ----------------------------------------------------------------------------------------------

fn arrangements_place_left_to_right_and_mirror_right_to_left() {
    use LayoutDirection::{Ltr, Rtl};
    let sizes = [10, 20, 30];
    let check = |a: &HorizontalArrangement, ltr: [i32; 3], rtl: [i32; 3]| {
        assert_eq!(horizontal(a, 100, &sizes, Ltr), ltr, "{a:?} left to right");
        assert_eq!(horizontal(a, 100, &sizes, Rtl), rtl, "{a:?} right to left");
    };
    check(&Arrangement::Start, [0, 10, 30], [90, 70, 40]);
    check(&Arrangement::End, [40, 50, 70], [50, 30, 0]);
    check(&Arrangement::Center, [20, 30, 50], [70, 50, 20]);
    check(&Arrangement::SpaceEvenly, [10, 30, 60], [80, 50, 10]);
    check(&Arrangement::SpaceBetween, [0, 30, 70], [90, 50, 0]);
    check(&Arrangement::SpaceAround, [7, 30, 63], [83, 50, 7]);
    check(&Arrangement::spaced_by(5.dp()), [0, 15, 40], [90, 65, 30]);
    check(&Arrangement::spaced_by_aligned(5.dp(), Alignment::End), [30, 45, 70], [60, 35, 0]);
    check(&Arrangement::spaced_by_aligned(5.dp(), Alignment::CenterHorizontally), [15, 30, 55], [75, 50, 15]);
    check(&Arrangement::aligned(Alignment::End), [40, 50, 70], [50, 30, 0]);
    // One child, spaced between: at the start.
    assert_eq!(horizontal(&Arrangement::SpaceBetween, 100, &[10], Ltr), [0]);
    assert_eq!(horizontal(&Arrangement::SpaceBetween, 100, &[10], Rtl), [90]);
    // The spacing is what intrinsic sizes count.
    assert_eq!(Arrangement::<Horizontal>::spaced_by(5.dp()).spacing(), 5.dp());
    assert_eq!(Arrangement::Start.spacing(), 0.dp());
}

fn absolute_arrangements_dont_mirror() {
    use LayoutDirection::{Ltr, Rtl};
    let sizes = [10, 20, 30];
    let check = |a: &HorizontalArrangement, expected: [i32; 3]| {
        assert_eq!(horizontal(a, 100, &sizes, Ltr), expected, "{a:?} left to right");
        assert_eq!(horizontal(a, 100, &sizes, Rtl), expected, "{a:?} right to left");
    };
    check(&AbsoluteArrangement::Left, [0, 10, 30]);
    check(&AbsoluteArrangement::Center, [20, 30, 50]);
    check(&AbsoluteArrangement::Right, [40, 50, 70]);
    check(&AbsoluteArrangement::SpaceBetween, [0, 30, 70]);
    check(&AbsoluteArrangement::SpaceEvenly, [10, 30, 60]);
    check(&AbsoluteArrangement::SpaceAround, [7, 30, 63]);
    check(&AbsoluteArrangement::spaced_by(5.dp()), [0, 15, 40]);
    check(&AbsoluteArrangement::spaced_by_aligned(5.dp(), AbsoluteAlignment::Right), [30, 45, 70]);
    check(&AbsoluteArrangement::aligned(AbsoluteAlignment::Right), [40, 50, 70]);
    // An alignment that mirrors still does: the end is the left right to left.
    let end = AbsoluteArrangement::spaced_by_aligned(5.dp(), Alignment::End);
    assert_eq!(horizontal(&end, 100, &sizes, Ltr), [30, 45, 70]);
    assert_eq!(horizontal(&end, 100, &sizes, Rtl), [0, 15, 40]);
}

fn vertical_arrangements() {
    let sizes = [10, 20, 30];
    assert_eq!(vertical(&Arrangement::Top, 100, &sizes), [0, 10, 30]);
    assert_eq!(vertical(&Arrangement::Bottom, 100, &sizes), [40, 50, 70]);
    assert_eq!(vertical(&Arrangement::Center, 100, &sizes), [20, 30, 50]);
    assert_eq!(vertical(&Arrangement::SpaceEvenly, 100, &sizes), [10, 30, 60]);
    assert_eq!(vertical(&Arrangement::SpaceBetween, 100, &sizes), [0, 30, 70]);
    assert_eq!(vertical(&Arrangement::SpaceAround, 100, &sizes), [7, 30, 63]);
    assert_eq!(vertical(&Arrangement::spaced_by(5.dp()), 100, &sizes), [0, 15, 40]);
    assert_eq!(vertical(&Arrangement::spaced_by_aligned(5.dp(), Alignment::Bottom), 100, &sizes), [30, 45, 70]);
    assert_eq!(vertical(&Arrangement::aligned(Alignment::CenterVertically), 100, &sizes), [20, 30, 50]);
    assert_eq!(vertical(&AbsoluteArrangement::spaced_by(5.dp()), 100, &sizes), [0, 15, 40]);
    // A custom one.
    let every_other = VerticalArrangement::vertical(3.dp(), |_, _, sizes, out| {
        let mut y = 0;
        for (i, s) in sizes.iter().enumerate() {
            out[i] = y;
            y += s * 2;
        }
    });
    assert_eq!(vertical(&every_other, 100, &sizes), [0, 20, 60]);
    assert_eq!(every_other.spacing(), 3.dp());
    assert_eq!(every_other, every_other.clone());
    assert_ne!(every_other, VerticalArrangement::vertical(3.dp(), |_, _, _, _| {}));
}

fn arrangements_in_less_space_than_their_children() {
    use LayoutDirection::{Ltr, Rtl};
    let sizes = [10, 20, 30];
    assert_eq!(horizontal(&Arrangement::Start, 50, &sizes, Ltr), [0, 10, 30]);
    assert_eq!(horizontal(&Arrangement::End, 50, &sizes, Ltr), [-10, 0, 20]);
    assert_eq!(horizontal(&Arrangement::Center, 50, &sizes, Ltr), [-5, 5, 25]);
    assert_eq!(horizontal(&Arrangement::SpaceBetween, 50, &sizes, Ltr), [0, 5, 20]);
    // Spaced: the space shrinks so the last child stays in.
    assert_eq!(horizontal(&Arrangement::spaced_by(5.dp()), 50, &sizes, Ltr), [0, 15, 20]);
    assert_eq!(horizontal(&Arrangement::spaced_by(5.dp()), 50, &sizes, Rtl), [40, 15, 0]);
}

fn arrangements_compare_by_how_they_were_made() {
    assert_eq!(HorizontalArrangement::Start, Arrangement::Start);
    assert_ne!(HorizontalArrangement::Start, Arrangement::End);
    assert_eq!(HorizontalArrangement::spaced_by(4.dp()), Arrangement::spaced_by(4.dp()));
    assert_ne!(HorizontalArrangement::spaced_by(4.dp()), Arrangement::spaced_by(5.dp()));
    assert_ne!(HorizontalArrangement::spaced_by(4.dp()), AbsoluteArrangement::spaced_by(4.dp()));
    assert_eq!(
        Arrangement::spaced_by_aligned(4.dp(), Alignment::End),
        HorizontalArrangement::spaced_by_aligned(4.dp(), Alignment::End)
    );
    assert_ne!(HorizontalArrangement::Center, AbsoluteArrangement::Center);
    assert_eq!(format!("{:?}", HorizontalArrangement::SpaceAround), "Arrangement#SpaceAround");
    assert_eq!(format!("{:?}", AbsoluteArrangement::Left), "AbsoluteArrangement#Left");
}

fn a_row_arranges_its_children_and_mirrors_right_to_left() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Row(
                    Modifier.width(100.dp()),
                    horizontal_arrangement = Arrangement::SpaceBetween,
                    content = || {
                        Box(Modifier.test_tag("a").size(10.dp()));
                        Box(Modifier.test_tag("b").size(20.dp()));
                        Box(Modifier.test_tag("c").size(30.dp()));
                    },
                );
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                    Row(
                        Modifier.test_tag("rtl").width(100.dp()),
                        content = || {
                            Box(Modifier.test_tag("x").size(10.dp()));
                            Box(Modifier.test_tag("y").size(20.dp()));
                        },
                    );
                    Row(
                        Modifier.test_tag("absolute").width(100.dp()),
                        horizontal_arrangement = AbsoluteArrangement::Left,
                        content = || {
                            Box(Modifier.test_tag("p").size(10.dp()));
                            Box(Modifier.test_tag("q").size(20.dp()));
                        },
                    );
                });
            },
        );
    }));
    assert_eq!([bounds(&rule, "a")[0], bounds(&rule, "b")[0], bounds(&rule, "c")[0]], [0.0, 30.0, 70.0]);
    let rtl = bounds(&rule, "rtl")[0];
    assert_eq!([bounds(&rule, "x")[0] - rtl, bounds(&rule, "y")[0] - rtl], [90.0, 70.0]);
    let absolute = bounds(&rule, "absolute")[0];
    assert_eq!([bounds(&rule, "p")[0] - absolute, bounds(&rule, "q")[0] - absolute], [0.0, 10.0]);
}

// ---- Row and Column --------------------------------------------------------------------------------------------

fn rows_and_columns_align_their_children_across() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Row(
                    Modifier.test_tag("row").height(50.dp()),
                    vertical_alignment = Alignment::CenterVertically,
                    content = || {
                        Box(Modifier.test_tag("middle").size(10.dp()));
                        Box(Modifier.test_tag("bottom").size(10.dp()).align(Alignment::Bottom));
                        Box(Modifier.test_tag("top").size(10.dp()).align(Alignment::Top));
                    },
                );
                Column(
                    Modifier.test_tag("column").width(50.dp()),
                    horizontal_alignment = Alignment::End,
                    content = || {
                        Box(Modifier.test_tag("end").size(10.dp()));
                        Box(Modifier.test_tag("start").size(10.dp()).align(Alignment::Start));
                        Box(Modifier.test_tag("center").size(10.dp()).align(Alignment::CenterHorizontally));
                    },
                );
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                    Column(
                        Modifier.test_tag("rtl").width(50.dp()),
                        horizontal_alignment = Alignment::End,
                        content = || {
                            Box(Modifier.test_tag("rtl-end").size(10.dp()));
                            Box(Modifier.test_tag("rtl-start").size(10.dp()).align(Alignment::Start));
                            Box(Modifier.test_tag("rtl-right").size(10.dp()).align(AbsoluteAlignment::Right));
                        },
                    );
                });
            },
        );
    }));
    let row = bounds(&rule, "row")[1];
    assert_eq!(
        [bounds(&rule, "middle")[1] - row, bounds(&rule, "bottom")[1] - row, bounds(&rule, "top")[1] - row],
        [20.0, 40.0, 0.0]
    );
    let column = bounds(&rule, "column")[0];
    assert_eq!(
        [bounds(&rule, "end")[0] - column, bounds(&rule, "start")[0] - column, bounds(&rule, "center")[0] - column],
        [40.0, 0.0, 20.0]
    );
    let rtl = bounds(&rule, "rtl")[0];
    assert_eq!(
        [bounds(&rule, "rtl-end")[0] - rtl, bounds(&rule, "rtl-start")[0] - rtl, bounds(&rule, "rtl-right")[0] - rtl],
        [0.0, 40.0, 40.0]
    );
}

fn children_align_by_baseline_and_the_row_grows_to_fit() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Row(
                    Modifier.test_tag("texts"),
                    content = || {
                        Text("a", font_size = 10.sp(), modifier = Modifier.test_tag("small").align_by_baseline());
                        Text("b", font_size = 20.sp(), modifier = Modifier.test_tag("big").align_by_baseline());
                    },
                );
                Row(
                    Modifier.test_tag("mixed"),
                    content = || {
                        Text("b", font_size = 20.sp(), modifier = Modifier.test_tag("text").align_by_baseline());
                        // A line at its bottom.
                        Box(Modifier.test_tag("box").size_wh(10.dp(), 30.dp()).align_by(|p: &Placeable| p.height()));
                        Box(Modifier.test_tag("plain").size(5.dp()));
                    },
                );
            },
        );
    }));
    // Baselines 8 (13 high) and 16 (25 high): the small one 8 down.
    let texts = bounds(&rule, "texts");
    assert_eq!(texts[3], 25.0);
    assert_eq!([bounds(&rule, "small")[1] - texts[1], bounds(&rule, "big")[1] - texts[1]], [8.0, 0.0]);
    // 30 above the line (the box), 9 under it (the text): 39 high.
    let mixed = bounds(&rule, "mixed");
    assert_eq!(mixed[3], 39.0);
    assert_eq!(
        [bounds(&rule, "text")[1] - mixed[1], bounds(&rule, "box")[1] - mixed[1], bounds(&rule, "plain")[1] - mixed[1]],
        [14.0, 0.0, 0.0]
    );
}

fn column_children_align_by_a_vertical_line_mirrored_right_to_left() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Column(
                    Modifier.test_tag("ltr"),
                    content = || {
                        LineLeaf(20, 5, Modifier.test_tag("a").align_by(Mark));
                        LineLeaf(10, 8, Modifier.test_tag("b").align_by(Mark));
                    },
                );
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                    Column(
                        Modifier.test_tag("rtl"),
                        content = || {
                            LineLeaf(20, 5, Modifier.test_tag("ra").align_by(Mark));
                            LineLeaf(10, 8, Modifier.test_tag("rb").align_by(Mark));
                        },
                    );
                });
            },
        );
    }));
    // 8 before the line, 15 after: 23 wide.
    let ltr = bounds(&rule, "ltr");
    assert_eq!(ltr[2], 23.0);
    assert_eq!([bounds(&rule, "a")[0] - ltr[0], bounds(&rule, "b")[0] - ltr[0]], [3.0, 0.0]);
    let rtl = bounds(&rule, "rtl");
    assert_eq!(rtl[2], 23.0);
    assert_eq!([bounds(&rule, "ra")[0] - rtl[0], bounds(&rule, "rb")[0] - rtl[0]], [0.0, 13.0]);
}

fn weights_share_what_is_left_rounding_spread_over_the_first() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Row(
            Modifier.width(100.dp()),
            horizontal_arrangement = Arrangement::spaced_by(5.dp()),
            content = || {
                Box(Modifier.test_tag("fixed").size(20.dp()));
                Box(Modifier.test_tag("w1").weight(1.0).height(10.dp()));
                Box(Modifier.test_tag("w2").weight(1.0).height(10.dp()));
                Box(Modifier.test_tag("w3").weight(1.0).height(10.dp()));
            },
        );
    }));
    // 100 - 20 - 3 * 5 = 65 for three: 21.67 each rounds to 22 (66), one too many: the first gives it back.
    let w = |t| bounds(&rule, t);
    assert_eq!([w("w1")[2], w("w2")[2], w("w3")[2]], [21.0, 22.0, 22.0]);
    assert_eq!([w("fixed")[0], w("w1")[0], w("w2")[0], w("w3")[0]], [0.0, 25.0, 51.0, 78.0]);
}

fn row_and_column_intrinsics_count_weights_and_spacing() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Intrinsics(40, 100, || {
            Row(
                horizontal_arrangement = Arrangement::spaced_by(4.dp()),
                content = || {
                    Text("aa bbbb", font_size = 10.sp());
                    Box(Modifier.size_wh(10.dp(), 30.dp()));
                    Layout(modifier = Modifier.weight(1.0), measure_policy = Area(1200));
                    Layout(modifier = Modifier.weight(2.0), measure_policy = Area(1200));
                },
            );
        });
    }));
    // Min width at 40: the text's widest word 20, the box 10, the weighted at 30 per unit of weight (the first
    // wants 30 for 1, the second 30 for 2: 15 a unit) times 3, three spacings of 4.
    // Max width: the whole text, 35. Heights at 100: 12 of spacing, the text 35, the box 10 leave 43 for 3
    // weights, 14 a unit: 1200 / 14 = 85 and 1200 / 28 = 42; the max at least that.
    assert_eq!(seen(), [20 + 10 + 90 + 12, 35 + 10 + 90 + 12, 85, 85]);
    drop(rule);

    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Intrinsics(100, 60, || {
            Column(
                vertical_arrangement = Arrangement::spaced_by(5.dp()),
                content = || {
                    Box(Modifier.size_wh(30.dp(), 10.dp()));
                    Layout(modifier = Modifier.weight(1.0), measure_policy = Area(1200));
                    Layout(modifier = Modifier.weight(3.0), measure_policy = Area(1200));
                },
            );
        });
    }));
    // Widths at 100: 10 of spacing and the box's 10 leave 80 for 4 weights, 20 a unit: 1200 / 20 = 60 and 1200 /
    // 60 = 20. Heights at 60: the box 10, the weighted 20 a unit (1200 / 60 for 1; 20 for 3, 7 a unit) times 4,
    // two spacings of 5.
    assert_eq!(seen(), [60, 60, 10 + 80 + 10, 10 + 80 + 10]);
    drop(rule);

    // Empty: nothing.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Intrinsics(100, 100, || Row(Modifier, Arrangement::spaced_by(5.dp()), content = || {}));
    }));
    assert_eq!(seen(), [0, 0, 0, 0]);
    drop(rule);
}

fn size_modifiers_answer_intrinsics() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Intrinsics(100, 100, || {
            Box(Modifier.width_in(20.dp(), max = 50.dp()), content = || Text("aa bbbb", font_size = 10.sp()));
        });
    }));
    // Min: the widest word 20; max: the whole text 35; heights the text's 13.
    assert_eq!(seen(), [20, 35, 13, 13]);
    drop(rule);

    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Intrinsics(100, 100, || {
            Box(
                Modifier.width_in(min = 30.dp(), max = 30.dp()).height_in(max = 10.dp()),
                content = || {
                    Text("aaaaaaaaaaaaaaaaaaaa", font_size = 10.sp());
                },
            );
        });
    }));
    // A fixed width: its own; a height capped at 10.
    assert_eq!(seen(), [30, 30, 10, 10]);
    drop(rule);

    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Intrinsics(100, 100, || Box(Modifier.default_min_size(80.dp(), min_height = 40.dp())));
    }));
    assert_eq!(seen(), [80, 80, 40, 40]);
    drop(rule);

    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Intrinsics(100, 100, || {
            Box(
                Modifier.padding_sides(start = 3.dp(), end = 4.dp(), top = 5.dp()).fill_max_width(),
                content = || {
                    Text("aa bbbb", font_size = 10.sp());
                },
            );
        });
    }));
    // The padding added.
    assert_eq!(seen(), [27, 42, 18, 18]);
    drop(rule);
}

// ---- Box -------------------------------------------------------------------------------------------------------

fn box_propagates_min_constraints_and_matches_its_size() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Box(
                    Modifier.size_in(min_width = 80.dp(), min_height = 40.dp()),
                    propagate_min_constraints = true,
                    content = || Box(Modifier.test_tag("propagated").size(20.dp())),
                );
                Box(
                    Modifier.size_in(min_width = 80.dp(), min_height = 40.dp()),
                    content = || Box(Modifier.test_tag("own").size(20.dp())),
                );
                Box(
                    Modifier.test_tag("outer"),
                    content = || {
                        Box(Modifier.size_wh(60.dp(), 30.dp()));
                        Box(Modifier.test_tag("matching").match_parent_size());
                        Box(Modifier.test_tag("bigger").match_parent_size().size(100.dp()));
                    },
                );
                Box(
                    Modifier.size_wh(70.dp(), 20.dp()),
                    content = || Box(Modifier.test_tag("only").match_parent_size()),
                );
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                    Box(
                        Modifier.test_tag("rtl").size(100.dp()),
                        content = || {
                            Box(Modifier.test_tag("top-end").size(10.dp()).align(Alignment::TopEnd));
                            Box(Modifier.test_tag("top-right").size(10.dp()).align(AbsoluteAlignment::TopRight));
                        },
                    );
                });
            },
        );
    }));
    assert_eq!(&bounds(&rule, "propagated")[2..], [80.0, 40.0]);
    assert_eq!(&bounds(&rule, "own")[2..], [20.0, 20.0]);
    let outer = bounds(&rule, "outer");
    assert_eq!(&outer[2..], [60.0, 30.0]);
    assert_eq!(bounds(&rule, "matching"), [outer[0], outer[1], 60.0, 30.0]);
    assert_eq!(bounds(&rule, "bigger"), [outer[0], outer[1], 60.0, 30.0]);
    assert_eq!(&bounds(&rule, "only")[2..], [70.0, 20.0]);
    let rtl = bounds(&rule, "rtl")[0];
    assert_eq!([bounds(&rule, "top-end")[0] - rtl, bounds(&rule, "top-right")[0] - rtl], [0.0, 90.0]);
}

// ---- Padding ---------------------------------------------------------------------------------------------------

fn padding_mirrors_start_and_end_right_to_left_but_not_left_and_right() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Box(
                    Modifier.test_tag("ltr").padding_sides(start = 10.dp(), top = 5.dp(), end = 20.dp()),
                    content = || {
                        Box(Modifier.test_tag("ltr-in").size(50.dp()));
                    },
                );
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                    Box(
                        Modifier.test_tag("rtl").padding_sides(start = 10.dp(), top = 5.dp(), end = 20.dp()),
                        content = || {
                            Box(Modifier.test_tag("rtl-in").size(50.dp()));
                        },
                    );
                    Box(
                        Modifier.test_tag("abs").absolute_padding(left = 10.dp(), right = 20.dp()),
                        content = || {
                            Box(Modifier.test_tag("abs-in").size(50.dp()));
                        },
                    );
                    Box(
                        Modifier.test_tag("values").padding_values(PaddingValues::new(
                            10.dp(),
                            5.dp(),
                            20.dp(),
                            0.dp(),
                        )),
                        content = || Box(Modifier.test_tag("values-in").size(50.dp())),
                    );
                    Box(
                        Modifier.test_tag("abs-values").padding_values(PaddingValues::absolute(
                            10.dp(),
                            0.dp(),
                            20.dp(),
                            0.dp(),
                        )),
                        content = || Box(Modifier.test_tag("abs-values-in").size(50.dp())),
                    );
                    Box(
                        Modifier.test_tag("hv").padding_hv(horizontal = 7.dp()),
                        content = || {
                            Box(Modifier.test_tag("hv-in").size(50.dp()));
                        },
                    );
                });
            },
        );
    }));
    let offset = |outer: &str, inner: &str| {
        let (o, i) = (bounds(&rule, outer), bounds(&rule, inner));
        (i[0] - o[0], i[1] - o[1], o[2], o[3])
    };
    assert_eq!(offset("ltr", "ltr-in"), (10.0, 5.0, 80.0, 55.0));
    assert_eq!(offset("rtl", "rtl-in"), (20.0, 5.0, 80.0, 55.0));
    assert_eq!(offset("abs", "abs-in"), (10.0, 0.0, 80.0, 50.0));
    assert_eq!(offset("values", "values-in"), (20.0, 5.0, 80.0, 55.0));
    assert_eq!(offset("abs-values", "abs-values-in"), (10.0, 0.0, 80.0, 50.0));
    assert_eq!(offset("hv", "hv-in"), (7.0, 0.0, 64.0, 50.0));
}

fn padding_values_as_compose() {
    use LayoutDirection::{Ltr, Rtl};
    let p = PaddingValues::new(1.dp(), 2.dp(), 3.dp(), 4.dp());
    assert_eq!((p.calculate_left_padding(Ltr), p.calculate_right_padding(Ltr)), (1.dp(), 3.dp()));
    assert_eq!((p.calculate_left_padding(Rtl), p.calculate_right_padding(Rtl)), (3.dp(), 1.dp()));
    assert_eq!((p.calculate_start_padding(Rtl), p.calculate_end_padding(Rtl)), (1.dp(), 3.dp()));
    assert_eq!((p.calculate_top_padding(), p.calculate_bottom_padding()), (2.dp(), 4.dp()));
    let a = PaddingValues::absolute(1.dp(), 2.dp(), 3.dp(), 4.dp());
    assert_eq!((a.calculate_left_padding(Rtl), a.calculate_start_padding(Rtl)), (1.dp(), 3.dp()));
    assert_eq!(PaddingValues::all(5.dp()), PaddingValues::new(5.dp(), 5.dp(), 5.dp(), 5.dp()));
    assert_eq!(PaddingValues::hv(5.dp(), 6.dp()), PaddingValues::new(5.dp(), 6.dp(), 5.dp(), 6.dp()));
    assert_ne!(PaddingValues::new(1.dp(), 2.dp(), 3.dp(), 4.dp()), a);
    let sum = p.clone() + a.clone();
    assert_eq!((sum.calculate_left_padding(Ltr), sum.calculate_bottom_padding()), (2.dp(), 8.dp()));
    let difference = PaddingValues::all(2.dp()) - p;
    assert_eq!((difference.calculate_left_padding(Ltr), difference.calculate_right_padding(Ltr)), (1.dp(), 0.dp()));
    assert_eq!(PaddingValues::Zero.calculate_top_padding(), 0.dp());
    assert_eq!(
        format!("{:?}", PaddingValues::all(1.dp())),
        format!("PaddingValues(start={0:?}, top={0:?}, end={0:?}, bottom={0:?})", 1.dp())
    );
    assert!(std::panic::catch_unwind(|| PaddingValues::all((-1).dp())).is_err());
    // One's own kind (Compose's PaddingValues interface implemented), told apart as Kotlin's `padding is Kind`.
    struct Growing(std::cell::Cell<f32>);
    impl CustomPaddingValues for Growing {
        fn calculate_left_padding(&self, _: LayoutDirection) -> Dp {
            Dp(self.0.get())
        }
        fn calculate_top_padding(&self) -> Dp {
            Dp(self.0.get())
        }
        fn calculate_right_padding(&self, _: LayoutDirection) -> Dp {
            Dp(0.0)
        }
        fn calculate_bottom_padding(&self) -> Dp {
            Dp(0.0)
        }
    }
    let custom = PaddingValues::custom(Growing(std::cell::Cell::new(1.0)));
    custom.custom_as::<Growing>().expect("a Growing").0.set(3.0);
    assert_eq!((custom.calculate_left_padding(Ltr), custom.calculate_top_padding()), (3.dp(), 3.dp()));
    assert!(PaddingValues::all(1.dp()).custom_as::<Growing>().is_none());
    assert_eq!(custom, custom.clone()); // (the same one)
    // Named in a body.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            Modifier.test_tag("p").padding_values(PaddingValues(top = 8.dp(), end = 2.dp())),
            content = || {
                Box(Modifier.size(10.dp()));
            },
        );
    }));
    assert_eq!(&bounds(&rule, "p")[2..], [12.0, 18.0]);
}

// ---- Size ------------------------------------------------------------------------------------------------------

fn fill_takes_a_fraction_of_the_max() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            Modifier.size_wh(200.dp(), 100.dp()),
            content = || {
                Box(Modifier.test_tag("half").fill_max_width(fraction = 0.5).height(10.dp()));
                Box(Modifier.test_tag("quarter").fill_max_height(fraction = 0.25).width(10.dp()));
                Box(Modifier.test_tag("both").fill_max_size(fraction = 0.3));
                Box(Modifier.test_tag("third").fill_max_width(fraction = 0.333).height(10.dp()));
                Box(Modifier.test_tag("whole").fill_max_size());
                // Within the min coming in.
                Box(Modifier.test_tag("min").width_in(min = 150.dp()).fill_max_width(fraction = 0.5).height(10.dp()));
            },
        );
    }));
    assert_eq!(&bounds(&rule, "half")[2..], [100.0, 10.0]);
    assert_eq!(&bounds(&rule, "quarter")[2..], [10.0, 25.0]);
    assert_eq!(&bounds(&rule, "both")[2..], [60.0, 30.0]);
    assert_eq!(&bounds(&rule, "third")[2..], [67.0, 10.0]);
    assert_eq!(&bounds(&rule, "whole")[2..], [200.0, 100.0]);
    assert_eq!(&bounds(&rule, "min")[2..], [150.0, 10.0]);
}

fn required_sizes_are_centered_over_what_the_parent_gave() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Box(Modifier.test_tag("big").size(100.dp()).required_size(150.dp()));
                Box(Modifier.test_tag("small").size(100.dp()).required_size_wh(40.dp(), 60.dp()));
                Box(Modifier.test_tag("wide").size(100.dp()).required_width(120.dp()));
                Box(Modifier.test_tag("low").size(100.dp()).required_height(20.dp()));
                Box(Modifier.test_tag("dp").size(100.dp()).required_size_dp(DpSize::new(20.dp(), 30.dp())));
            },
        );
    }));
    assert_eq!(content_bounds(&rule, "big"), [-25.0, -25.0, 150.0, 150.0]);
    assert_eq!(content_bounds(&rule, "small"), [30.0, 120.0, 40.0, 60.0]);
    assert_eq!(content_bounds(&rule, "wide"), [-10.0, 200.0, 120.0, 100.0]);
    assert_eq!(content_bounds(&rule, "low"), [0.0, 340.0, 100.0, 20.0]);
    assert_eq!(content_bounds(&rule, "dp"), [40.0, 435.0, 20.0, 30.0]);
    assert_eq!(bounds(&rule, "dp"), [0.0, 400.0, 100.0, 100.0]);
}

fn wrap_content_aligns_and_can_be_unbounded() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Box(Modifier.test_tag("bounded").size(50.dp()).wrap_content_size().size(80.dp()));
                Box(Modifier.test_tag("unbounded").size(50.dp()).wrap_content_size(unbounded = true).size(80.dp()));
                Box(Modifier
                    .test_tag("start")
                    .size(50.dp())
                    .wrap_content_width(align = Alignment::Start, unbounded = true)
                    .size_wh(80.dp(), 20.dp()));
                Box(Modifier
                    .test_tag("bottom")
                    .size(50.dp())
                    .wrap_content_height(align = Alignment::Bottom)
                    .size_wh(20.dp(), 10.dp()));
                Box(Modifier.test_tag("centered").size(50.dp()).wrap_content_size().size(10.dp()));
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                    Box(Modifier
                        .test_tag("rtl-start")
                        .size(50.dp())
                        .wrap_content_width(align = Alignment::Start, unbounded = true)
                        .size_wh(80.dp(), 20.dp()));
                });
            },
        );
    }));
    assert_eq!(content_bounds(&rule, "bounded"), [0.0, 0.0, 50.0, 50.0]);
    assert_eq!(content_bounds(&rule, "unbounded"), [-15.0, 35.0, 80.0, 80.0]);
    assert_eq!(content_bounds(&rule, "start"), [0.0, 100.0, 80.0, 50.0]);
    assert_eq!(content_bounds(&rule, "bottom"), [0.0, 190.0, 50.0, 10.0]);
    assert_eq!(content_bounds(&rule, "centered"), [20.0, 220.0, 10.0, 10.0]);
    let rtl = bounds(&rule, "rtl-start");
    let content = content_bounds(&rule, "rtl-start");
    assert_eq!(content[0] - rtl[0], -30.0);
}

fn default_min_size_yields_to_a_given_size() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Box(Modifier.test_tag("width").default_min_size(min_width = 80.dp()));
                Box(Modifier.test_tag("height").width(30.dp()).default_min_size(80.dp(), min_height = 40.dp()));
                Box(Modifier.test_tag("bigger").default_min_size(10.dp(), min_height = 10.dp()).size(30.dp()));
                Box(Modifier.test_tag("capped").width_in(max = 50.dp()).default_min_size(min_width = 80.dp()));
            },
        );
    }));
    // (no area: bounds are zero, as Compose's; the size tells)
    assert_eq!(size(&rule, "width"), IntSize::new(80, 0));
    assert_eq!(&bounds(&rule, "height")[2..], [30.0, 40.0]);
    assert_eq!(&bounds(&rule, "bigger")[2..], [30.0, 30.0]);
    assert_eq!(size(&rule, "capped"), IntSize::new(50, 0));
}

fn spacer_takes_its_modifiers_size() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Row(content = || {
                    Spacer(Modifier.width(30.dp()));
                    Box(Modifier.test_tag("after").size(10.dp()));
                });
                Row(
                    Modifier.width(100.dp()),
                    content = || {
                        Spacer(Modifier.weight(1.0));
                        Box(Modifier.test_tag("end").size(10.dp()));
                    },
                );
                Spacer(Modifier.test_tag("spacer").height(7.dp()));
            },
        );
    }));
    assert_eq!(bounds(&rule, "after")[0], 30.0);
    assert_eq!(bounds(&rule, "end")[0], 90.0);
    assert_eq!(size(&rule, "spacer"), IntSize::new(0, 7));
}

// ---- Elements and recomposition ---------------------------------------------------------------------------------

fn modifiers_of_equal_arguments_are_equal() {
    assert_eq!(Modifier.padding(4.dp()), Modifier.padding(4.dp()));
    assert_ne!(Modifier.padding(4.dp()), Modifier.padding(5.dp()));
    assert_ne!(
        Modifier.padding_sides_with(1.dp(), 0.dp(), 2.dp(), 0.dp()),
        Modifier.absolute_padding_with(1.dp(), 0.dp(), 2.dp(), 0.dp())
    );
    assert_eq!(Modifier.size_in(), Modifier.size_in());
    assert_eq!(Modifier.default_min_size(), Modifier.default_min_size());
    assert_eq!(Modifier.size(3.dp()), Modifier.size_wh(3.dp(), 3.dp()));
    assert_ne!(Modifier.size(3.dp()), Modifier.required_size(3.dp()));
    assert_eq!(Modifier.fill_max_width(), Modifier.fill_max_width_with(1.0));
    assert_ne!(Modifier.fill_max_width(), Modifier.fill_max_height());
    assert_eq!(Modifier.wrap_content_size(), Modifier.wrap_content_size_with(Alignment::Center, false));
    assert_ne!(Modifier.wrap_content_size(), Modifier.wrap_content_size_with(Alignment::Center, true));
    assert_eq!(Modifier.weight(1.0), Modifier.weight_with(1.0, true));
    assert_ne!(Modifier.weight(1.0), Modifier.weight_with(1.0, false));
    assert_eq!(Modifier.align(Alignment::Center), Modifier.align(Alignment::Center));
    assert_eq!(Modifier.align(Alignment::Top), Modifier.align(Alignment::Top));
    assert_eq!(Modifier.align_by(FirstBaseline), Modifier.align_by_baseline());
    assert_eq!(
        Modifier.padding_values(PaddingValues::all(2.dp())),
        Modifier.padding_values(PaddingValues::all(2.dp()))
    );
    assert_eq!(
        format!("{:?}", Modifier.padding(1.dp()).size(2.dp()).fill_max_width()),
        "Modifier.padding.size.fillMaxWidth"
    );
    assert!(std::panic::catch_unwind(|| Modifier.weight(0.0)).is_err());
    assert!(std::panic::catch_unwind(|| Modifier.padding((-1).dp())).is_err());
}

fn layouts_recomposed_with_equal_arguments_arent_measured_again() {
    MEASURES.with(|m| m.set(0));
    // Made once, so it stays equal: it counts the measurements of the layouts it decorates.
    let counting = Modifier.layout(|scope, measurable, constraints| {
        MEASURES.with(|m| m.set(m.get() + 1));
        let p = measurable.measure(constraints);
        scope.layout(p.width(), p.height(), move |s| s.place(&p, 0, 0))
    });
    let tick = mutable_state_of(0);
    let other = mutable_state_of(false);
    let (t, o) = (tick.clone(), other.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let _ = t.get();
        let other = o.get();
        let align = if other { Alignment::TopStart } else { Alignment::Center };
        let across = if other { Arrangement::SpaceBetween } else { Arrangement::spaced_by(4.dp()) };
        let down = if other { Arrangement::SpaceBetween } else { Arrangement::spaced_by(4.dp()) };
        Column(
            content = || {
                Box(
                    counting.clone().size(50.dp()).padding(2.dp()),
                    align.clone(),
                    content = || {
                        Box(Modifier.size(10.dp()));
                    },
                );
                Row(
                    counting.clone().fill_max_width().size_in(min_height = 10.dp()),
                    across.clone(),
                    Alignment::Bottom,
                    content = || Box(Modifier.size(10.dp())),
                );
                Column(counting.clone().wrap_content_size(), down.clone(), content = || Box(Modifier.size(10.dp())));
            },
        );
    }));
    rule.set_layout_check(false);
    let before = measures();
    assert!(before >= 3);
    // Composed again with the same arguments: nothing measured.
    tick.set(1);
    rule.wait_for_idle();
    assert_eq!(measures(), before);
    // Other ones: each measured again.
    other.set(true);
    rule.wait_for_idle();
    assert_eq!(measures(), before + 3);
}

tests!(
    row_distributes_weight,
    padding_offsets_content,
    column_arrangement_and_alignment,
    box_alignment,
    required_size_ranges_ignore_the_incoming_constraints,
    size_in_and_default_min_size,
    rows_and_columns_with_weights_that_dont_fill_are_as_big_as_their_children,
    wrap_content_and_required_size,
    arrangements_place_left_to_right_and_mirror_right_to_left,
    absolute_arrangements_dont_mirror,
    vertical_arrangements,
    arrangements_in_less_space_than_their_children,
    arrangements_compare_by_how_they_were_made,
    a_row_arranges_its_children_and_mirrors_right_to_left,
    rows_and_columns_align_their_children_across,
    children_align_by_baseline_and_the_row_grows_to_fit,
    column_children_align_by_a_vertical_line_mirrored_right_to_left,
    weights_share_what_is_left_rounding_spread_over_the_first,
    row_and_column_intrinsics_count_weights_and_spacing,
    size_modifiers_answer_intrinsics,
    box_propagates_min_constraints_and_matches_its_size,
    padding_mirrors_start_and_end_right_to_left_but_not_left_and_right,
    padding_values_as_compose,
    fill_takes_a_fraction_of_the_max,
    required_sizes_are_centered_over_what_the_parent_gave,
    wrap_content_aligns_and_can_be_unbounded,
    default_min_size_yields_to_a_given_size,
    spacer_takes_its_modifiers_size,
    modifiers_of_equal_arguments_are_equal,
    layouts_recomposed_with_equal_arguments_arent_measured_again,
);

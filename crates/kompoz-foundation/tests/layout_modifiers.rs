//! foundation.layout's other layout modifiers: offsets, aspect ratios, intrinsic sizes, padding from alignment
//! lines, and right to left.

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
use std::rc::Rc;

// ---- Helpers ---------------------------------------------------------------------------------------------------

/// A node's outer bounds in the root (its outer coordinates: C++'s `bounds`, wherever its test tag is in its
/// modifier): x, y, width, height.
fn bounds(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    let outer = rule.on_node_with_tag(tag).layout_node().coordinates();
    let b = outer.find_root_coordinates().local_bounding_box_of(&outer, false);
    [b.left, b.top, b.width(), b.height()]
}

/// Where a node's content is in the root (its inner coordinates: C++'s `contentBounds`): x, y, width, height.
fn content_bounds(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    let inner = rule.on_node_with_tag(tag).layout_node().inner_coordinates();
    let b = inner.find_root_coordinates().local_bounding_box_of(&inner, false);
    [b.left, b.top, b.width(), b.height()]
}

fn x(rule: &KompozTestRule, tag: &str) -> f32 {
    bounds(rule, tag)[0]
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// Where `line` is in a node, from its top (or left).
fn line(rule: &KompozTestRule, tag: &str, line: impl Into<AlignmentLine>) -> i32 {
    rule.on_node_with_tag(tag).layout_node().coordinates().get(line)
}

#[kompozable(inline)]
fn rtl(content: impl FnOnce()) {
    KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], content);
}

thread_local! {
    /// What a measuring layout saw of its child's intrinsic sizes.
    static SEEN: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
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
        2 * self.other_side(height)
    }
    fn min_intrinsic_height(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], width: i32) -> i32 {
        self.other_side(width)
    }
    fn max_intrinsic_height(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], width: i32) -> i32 {
        2 * self.other_side(width)
    }
}

fn min_merge(a: i32, b: i32) -> i32 {
    a.min(b)
}

/// A vertical line a [`VerticalLeaf`] provides.
const Mark: VerticalAlignmentLine = VerticalAlignmentLine::new("Mark", min_merge);

/// A leaf `width` by 10 providing [`Mark`] at `mark`.
#[kompozable(inline)]
fn VerticalLeaf(width: i32, mark: i32, modifier: Modifier) {
    Layout(
        modifier = modifier,
        measure_policy =
            measure_policy(move |_, _, _| MeasureResult::new(width, 10, vec![(Mark.into(), mark)], |_| {})),
    );
}

// ---- Aspect ratio ----------------------------------------------------------------------------------------------

fn aspect_ratio_wrap_content_and_required_size() {
    // C++'s aspect_ratio_wrap_content_and_required_size.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(Modifier.width(200.dp()).aspect_ratio(2.0).test_tag("ratio"));
            Box(Modifier
                .size_wh(300.dp(), 100.dp())
                .wrap_content_size(align = Alignment::BottomEnd)
                .size(50.dp())
                .test_tag("wrapped"));
            Box(Modifier.size(100.dp()).required_size(150.dp()).test_tag("required"));
            Box(Modifier.height(60.dp()).aspect_ratio(0.5).test_tag("tall"));
        });
    }));
    assert_eq!(bounds(&rule, "ratio"), [0.0, 0.0, 200.0, 100.0]);
    assert_eq!(bounds(&rule, "wrapped"), [0.0, 100.0, 300.0, 100.0]);
    assert_eq!(content_bounds(&rule, "wrapped"), [250.0, 150.0, 50.0, 50.0]);
    assert_eq!(bounds(&rule, "required"), [0.0, 200.0, 100.0, 100.0]);
    assert_eq!(content_bounds(&rule, "required"), [-25.0, 175.0, 150.0, 150.0]);
    assert_eq!(bounds(&rule, "tall"), [0.0, 300.0, 30.0, 60.0]);
}

fn aspect_ratio_tries_the_constraints_in_composes_order() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            // Bounded both ways: the max width first, unless the height goes first.
            Box(Modifier.size_in(max_width = 100.dp(), max_height = 80.dp()).aspect_ratio(2.0).test_tag("width"));
            Box(Modifier
                .size_in(max_width = 100.dp(), max_height = 80.dp())
                .aspect_ratio(2.0, match_height_constraints_first = true)
                .test_tag("height first"));
            // The max width would make it too tall: the max height then.
            Box(Modifier.size_in(max_width = 100.dp(), max_height = 20.dp()).aspect_ratio(2.0).test_tag("too tall"));
            // Nothing fits the constraints: the first that has a size, not enforced.
            Box(Modifier.size_wh(100.dp(), 10.dp()).aspect_ratio_with(1.0, false).test_tag("unfit"));
            // Unbounded: what it wraps measures.
            Row(Modifier.wrap_content_size(unbounded = true), || {
                Box(Modifier.aspect_ratio(3.0).test_tag("unbounded"), content = || Box(Modifier.size(12.dp())));
            });
        });
    }));
    assert_eq!(&bounds(&rule, "width")[2..], [100.0, 50.0]);
    // 80 high is 160 wide, too wide: the max width's 100 by 50 then.
    assert_eq!(&bounds(&rule, "height first")[2..], [100.0, 50.0]);
    assert_eq!(&bounds(&rule, "too tall")[2..], [40.0, 20.0]);
    // (measured 100 by 100, brought within the 100 by 10 it was given)
    assert_eq!(&bounds(&rule, "unfit")[2..], [100.0, 10.0]);
    assert_eq!(&content_bounds(&rule, "unfit")[2..], [100.0, 100.0]);
    assert_eq!(&bounds(&rule, "unbounded")[2..], [12.0, 12.0]);
    // Its intrinsic sizes: the side given through the ratio.
    let _rule = KompozTestRule::new(kompozable_lambda!(|| {
        Intrinsics(30, 60, || Box(Modifier.aspect_ratio(2.0)));
    }));
    assert_eq!(seen(), [60, 60, 30, 30]);
    let _rule = KompozTestRule::new(kompozable_lambda!(|| {
        Intrinsics(Constraints::Infinity, Constraints::Infinity, || {
            Layout(modifier = Modifier.aspect_ratio(2.0), measure_policy = Area(800));
        });
    }));
    assert_eq!(seen(), [0, 0, 0, 0]);
    // Not more than 0: refused.
    assert!(std::panic::catch_unwind(|| Modifier.aspect_ratio(0.0)).is_err());
    assert!(std::panic::catch_unwind(|| Modifier.aspect_ratio(-1.0)).is_err());
    assert_eq!(Modifier.aspect_ratio(1.5), Modifier.aspect_ratio_with(1.5, false));
    assert_ne!(Modifier.aspect_ratio(1.5), Modifier.aspect_ratio_with(1.5, true));
}

// ---- Intrinsic sizes -------------------------------------------------------------------------------------------

fn min_intrinsic_widths_are_the_widest_words() {
    // C++'s min_intrinsic_widths_are_the_widest_words, its parts the test Text can show (its styled part needs
    // BasicText). The FakeTextMeasurer at 16 sp: 8 px a code point. Compose's min intrinsic width is the widest
    // piece between line breaks with its trailing space ("kelimeler " 80, "bir " 32, "dört " 40; C++ left the
    // space out: 72, 24 + 32).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Column(Modifier.width_intrinsic(IntrinsicSize::Min).test_tag("dar"), || {
                Text("kısa kelimeler burada", font_size = 16.sp());
                // As wide as the column came out.
                Box(Modifier.fill_max_width().height(2.dp()).test_tag("çizgi"));
            });
            Row(Modifier.width_intrinsic(IntrinsicSize::Min).test_tag("satır"), || {
                Text("bir iki", font_size = 16.sp());
                Text("üç dört beş", font_size = 16.sp());
            });
            Column(Modifier.width_intrinsic(IntrinsicSize::Max).test_tag("geniş"), || {
                Text("kısa kelimeler burada", font_size = 16.sp());
            });
        });
    }));
    // "kelimeler "; "bir " (or "iki") and "dört " side by side; all of it on one line.
    assert!(near(bounds(&rule, "dar")[2], 10.0 * 8.0) && near(bounds(&rule, "çizgi")[2], 10.0 * 8.0));
    assert!(near(bounds(&rule, "satır")[2], 4.0 * 8.0 + 5.0 * 8.0));
    assert!(near(bounds(&rule, "geniş")[2], 21.0 * 8.0));
    // Laid out for real at that width afterwards: the narrow column's text wraps a word a line (three lines of
    // 20 and the 2 px line).
    assert!(near(bounds(&rule, "dar")[3], 3.0 * 20.0 + 2.0));
}

fn intrinsic_height_makes_a_divider_as_tall_as_the_row() {
    // C++'s intrinsic_height_and_flow_row, its intrinsic height part (FlowRow is a later stage's).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Row(Modifier.height_intrinsic(IntrinsicSize::Min).test_tag("row"), || {
                Box(Modifier.size_wh(40.dp(), 30.dp()));
                Box(Modifier.width(2.dp()).fill_max_height().test_tag("divider"));
                Box(Modifier.size_wh(40.dp(), 70.dp()));
            });
        });
    }));
    assert!(near(bounds(&rule, "row")[3], 70.0) && near(bounds(&rule, "divider")[3], 70.0));
}

fn intrinsic_size_modifiers_as_compose() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            // Min and max of a leaf whose max is twice its min, at the incoming max across.
            Box(
                Modifier.size_wh(200.dp(), 10.dp()),
                content = || {
                    Layout(
                        modifier = Modifier.width_intrinsic(IntrinsicSize::Min).height(10.dp()).test_tag("min"),
                        measure_policy = Area(100),
                    );
                },
            );
            Box(
                Modifier.size_wh(200.dp(), 10.dp()),
                content = || {
                    Layout(
                        modifier = Modifier.width_intrinsic(IntrinsicSize::Max).height(10.dp()).test_tag("max"),
                        measure_policy = Area(100),
                    );
                },
            );
            Box(
                Modifier.size_wh(20.dp(), 200.dp()),
                content = || {
                    Layout(
                        modifier = Modifier.height_intrinsic(IntrinsicSize::Max).width(20.dp()).test_tag("tall"),
                        measure_policy = Area(100),
                    );
                },
            );
            // Within the incoming constraints, or not (required): 50 wide in a 30 wide box, centered over it.
            Box(
                Modifier.width(30.dp()).height(10.dp()),
                content = || {
                    Layout(
                        modifier = Modifier.width_intrinsic(IntrinsicSize::Min).height(10.dp()).test_tag("kept"),
                        measure_policy = Area(500),
                    );
                },
            );
            Box(
                Modifier.width(30.dp()).height(10.dp()).test_tag("box"),
                content = || {
                    Layout(
                        modifier =
                            Modifier.required_width_intrinsic(IntrinsicSize::Min).height(10.dp()).test_tag("required"),
                        measure_policy = Area(500),
                    );
                },
            );
            Box(
                Modifier.width(10.dp()).height(30.dp()),
                content = || {
                    Layout(
                        modifier = Modifier
                            .required_height_intrinsic(IntrinsicSize::Max)
                            .width(10.dp())
                            .test_tag("required tall"),
                        measure_policy = Area(100),
                    );
                },
            );
        });
    }));
    assert_eq!(&bounds(&rule, "min")[2..], [10.0, 10.0]);
    assert_eq!(&bounds(&rule, "max")[2..], [20.0, 10.0]);
    assert_eq!(&bounds(&rule, "tall")[2..], [20.0, 10.0]);
    assert_eq!(&bounds(&rule, "kept")[2..], [30.0, 10.0]);
    assert_eq!(rule.on_node_with_tag("required").layout_node().size(), IntSize::new(50, 10));
    let (b, r) = (bounds(&rule, "box"), bounds(&rule, "required"));
    assert_eq!(r[0] - b[0], -10.0);
    assert_eq!(rule.on_node_with_tag("required tall").layout_node().size(), IntSize::new(10, 20));
    // Its own intrinsic size that way is its content's min or max, the other way its content's.
    let _rule = KompozTestRule::new(kompozable_lambda!(|| {
        Intrinsics(10, 10, || {
            Layout(modifier = Modifier.width_intrinsic(IntrinsicSize::Max), measure_policy = Area(100))
        });
    }));
    assert_eq!(seen(), [20, 20, 10, 20]);
    let _rule = KompozTestRule::new(kompozable_lambda!(|| {
        Intrinsics(10, 10, || {
            Layout(modifier = Modifier.height_intrinsic(IntrinsicSize::Min), measure_policy = Area(100))
        });
    }));
    assert_eq!(seen(), [10, 20, 10, 10]);
    assert_eq!(Modifier.width_intrinsic(IntrinsicSize::Min), Modifier.width_intrinsic(IntrinsicSize::Min));
    assert_ne!(Modifier.width_intrinsic(IntrinsicSize::Min), Modifier.required_width_intrinsic(IntrinsicSize::Min));
}

// ---- Padding from alignment lines ------------------------------------------------------------------------------

fn padding_from_baseline_puts_text_on_a_grid() {
    // C++'s alignment_lines_pass_up_and_set_padding, its paddingFromBaseline parts. 16 px text: 20 high, its
    // baseline 13 down.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Text(
                "ızgara",
                font_size = 16.sp(),
                modifier = Modifier.padding_from_baseline(top = 40.dp(), bottom = 20.dp()).test_tag("ızgara"),
            );
            // Only from the bottom, in a taller box: the content goes down to keep the last baseline 10 up.
            Text(
                "dipte",
                font_size = 16.sp(),
                modifier = Modifier.height(100.dp()).padding_from(LastBaseline, after = 10.dp()).test_tag("dipte"),
            );
        });
    }));
    // The baseline 40 from the top, 20 above the bottom.
    let grid = bounds(&rule, "ızgara");
    assert_eq!(line(&rule, "ızgara", FirstBaseline), 40);
    assert_eq!(grid[3] - 40.0, 20.0);
    let bottom = bounds(&rule, "dipte");
    assert_eq!(bottom[3], 100.0);
    assert_eq!(bottom[3] as i32 - line(&rule, "dipte", LastBaseline), 10);
}

fn padding_from_alignment_lines_as_compose() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            // Before only: the line moved down to it; after only: the space under it made up.
            Text(
                "a",
                font_size = 16.sp(),
                modifier = Modifier.padding_from(FirstBaseline, before = 30.dp()).test_tag("before"),
            );
            Text(
                "a",
                font_size = 16.sp(),
                modifier = Modifier.padding_from_with(FirstBaseline, Dp::Unspecified, 30.dp()).test_tag("after"),
            );
            // A line already further than asked: nothing added.
            Text(
                "a",
                font_size = 16.sp(),
                modifier = Modifier.padding_from(FirstBaseline, before = 5.dp()).test_tag("enough"),
            );
            // In sp, which follow the font scale.
            Text("a", font_size = 16.sp(), modifier = Modifier.padding_from_baseline_sp(top = 20.sp()).test_tag("sp"));
            Text(
                "a",
                font_size = 16.sp(),
                modifier =
                    Modifier.padding_from_sp_with(LastBaseline, TextUnit::Unspecified, 10.sp()).test_tag("sp after"),
            );
            // No such line: taken at the top.
            Box(
                Modifier.padding_from(FirstBaseline, before = 12.dp()).test_tag("none"),
                content = || Box(Modifier.size(10.dp())),
            );
            // Within the incoming max only.
            Box(
                Modifier.height(25.dp()),
                content = || {
                    Text(
                        "a",
                        font_size = 16.sp(),
                        modifier = Modifier.padding_from_baseline_with(40.dp(), Dp::Unspecified).test_tag("capped"),
                    );
                },
            );
            // A vertical line: from the start, mirrored right to left.
            Row(|| {
                VerticalLeaf(20, 4, Modifier.padding_from(Mark, before = 10.dp()).test_tag("vertical"));
            });
            rtl(|| {
                Row(Modifier.width(100.dp()), || {
                    VerticalLeaf(20, 4, Modifier.padding_from(Mark, before = 10.dp()).test_tag("vertical rtl"));
                });
            });
        });
    }));
    assert_eq!(line(&rule, "before", FirstBaseline), 30);
    assert_eq!(bounds(&rule, "before")[3], 37.0);
    assert_eq!(bounds(&rule, "after")[3], 43.0);
    assert_eq!(bounds(&rule, "after")[3] as i32 - line(&rule, "after", FirstBaseline), 30);
    assert_eq!(bounds(&rule, "enough")[3], 20.0);
    assert_eq!(line(&rule, "sp", FirstBaseline), 20);
    assert_eq!(bounds(&rule, "sp after")[3] as i32 - line(&rule, "sp after", LastBaseline), 10);
    assert_eq!(content_bounds(&rule, "none")[1] - bounds(&rule, "none")[1], 12.0);
    assert_eq!(bounds(&rule, "capped")[3], 25.0);
    assert_eq!(line(&rule, "capped", FirstBaseline), 13 + 5);
    assert_eq!(line(&rule, "vertical", Mark), 10);
    assert_eq!(bounds(&rule, "vertical")[2], 26.0);
    let v = rule.on_node_with_tag("vertical rtl");
    assert_eq!(v.layout_node().size().width, 26);
    // (placed from the start: the leaf at the right of its 26)
    assert_eq!(content_bounds(&rule, "vertical rtl")[0] - bounds(&rule, "vertical rtl")[0], 0.0);
    assert_eq!(bounds(&rule, "vertical rtl")[0], 74.0);
    // The font scale grows the sp ones.
    rule.host().set_font_scale(2.0);
    rule.wait_for_idle();
    assert_eq!(line(&rule, "sp", FirstBaseline), 40);
    // Negative distances are refused (in dp).
    assert!(
        std::panic::catch_unwind(|| Modifier.padding_from_with(FirstBaseline, (-1).dp(), Dp::Unspecified)).is_err()
    );
    // From the baselines: a padding from each.
    assert_eq!(
        Modifier.padding_from_baseline_with(1.dp(), 2.dp()),
        Modifier.padding_from_with(FirstBaseline, 1.dp(), Dp::Unspecified).padding_from_with(
            LastBaseline,
            Dp::Unspecified,
            2.dp()
        )
    );
    assert_eq!(Modifier.padding_from_baseline(), Modifier);
}

// ---- Offset ----------------------------------------------------------------------------------------------------

fn offsets_move_what_they_wrap_and_mirror_right_to_left() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(Modifier.width(100.dp()), || {
            Box(Modifier.size(20.dp()).offset(x = 10.dp(), y = 5.dp()).test_tag("offset"));
            Box(Modifier.size(20.dp()).test_tag("next"));
            Box(Modifier.size(20.dp()).absolute_offset_with(3.dp(), (-2).dp()).test_tag("absolute"));
            Box(Modifier.size(20.dp()).offset(y = 4.dp()).test_tag("down"));
            rtl(|| {
                Column(Modifier.fill_max_width(), || {
                    Box(Modifier.size(20.dp()).offset_with(10.dp(), 0.dp()).test_tag("offset rtl"));
                    Box(Modifier.size(20.dp()).absolute_offset(x = 10.dp()).test_tag("absolute rtl"));
                });
            });
        });
    }));
    // Its content moved; its place and size as they were, its siblings unmoved.
    assert_eq!(bounds(&rule, "offset"), [0.0, 0.0, 20.0, 20.0]);
    assert_eq!(content_bounds(&rule, "offset"), [10.0, 5.0, 20.0, 20.0]);
    assert_eq!(bounds(&rule, "next"), [0.0, 20.0, 20.0, 20.0]);
    assert_eq!(content_bounds(&rule, "absolute"), [3.0, 38.0, 20.0, 20.0]);
    assert_eq!(content_bounds(&rule, "down"), [0.0, 64.0, 20.0, 20.0]);
    // Right to left: towards the end is to the left; the absolute one to the right.
    assert_eq!(bounds(&rule, "offset rtl")[0], 80.0);
    assert_eq!(content_bounds(&rule, "offset rtl")[0], 70.0);
    assert_eq!(content_bounds(&rule, "absolute rtl")[0], 90.0);
    assert_eq!(Modifier.offset(), Modifier.offset_with(0.dp(), 0.dp()));
    assert_eq!(Modifier.offset_with(1.dp(), 2.dp()), Modifier.offset_with(1.dp(), 2.dp()));
    assert_ne!(Modifier.offset_with(1.dp(), 2.dp()), Modifier.absolute_offset_with(1.dp(), 2.dp()));
}

fn a_new_offset_places_again_without_measuring() {
    let measured = Rc::new(Cell::new(0));
    let m = measured.clone();
    // Made once, so it stays equal: it counts the measurements of what it decorates.
    let counting = Modifier.layout(move |scope, measurable, constraints| {
        m.set(m.get() + 1);
        let p = measurable.measure(constraints);
        scope.layout(p.width(), p.height(), move |s| s.place(&p, 0, 0))
    });
    let dx = mutable_state_of(0.0f32);
    let d = dx.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let counting = counting.clone();
        Box(
            Modifier.offset(x = d.get().dp()).test_tag("moved"),
            content = || {
                Box(counting.size(10.dp()));
            },
        );
    }));
    rule.set_layout_check(false);
    let before = measured.get();
    dx.set(15.0);
    rule.wait_for_idle();
    assert_eq!(content_bounds(&rule, "moved")[0], 15.0);
    assert_eq!(bounds(&rule, "moved")[0], 0.0);
    assert_eq!(measured.get(), before);
}

fn offset_px_places_again_without_measuring_or_composing() {
    let measured = Rc::new(Cell::new(0));
    let shift = mutable_state_of(0);
    let (m, s) = (measured.clone(), shift.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (m, s, s2) = (m.clone(), s.clone(), s.clone());
        Column(Modifier.width(100.dp()), || {
            Box(
                Modifier.size(20.dp()).offset_px(move |_| IntOffset::new(s.get(), 2 * s.get())).test_tag("moved"),
                content = || {
                    let m = m.clone();
                    Layout(
                        measure_policy = measure_policy(move |scope, _, c| {
                            m.set(m.get() + 1);
                            scope.layout(c.min_width, c.min_height, |_| {})
                        }),
                    );
                },
            );
            rtl(|| {
                Column(Modifier.fill_max_width(), || {
                    Box(Modifier
                        .size(20.dp())
                        .offset_px(move |d| IntOffset::new(d.round_to_px(5.dp()), 0))
                        .test_tag("rtl"));
                    Box(Modifier
                        .size(20.dp())
                        .absolute_offset_px(move |_| IntOffset::new(s2.get(), 0))
                        .test_tag("absolute"));
                });
            });
        });
    }));
    rule.set_layout_check(false);
    let (before, compositions) = (measured.get(), rule.host().stats().recompositions);
    assert_eq!(content_bounds(&rule, "moved"), [0.0, 0.0, 20.0, 20.0]);
    assert_eq!(content_bounds(&rule, "rtl")[0], 75.0);
    assert_eq!(content_bounds(&rule, "absolute")[0], 80.0);
    shift.set(7);
    rule.wait_for_idle();
    assert_eq!(content_bounds(&rule, "moved"), [7.0, 14.0, 20.0, 20.0]);
    assert_eq!(bounds(&rule, "moved"), [0.0, 0.0, 20.0, 20.0]);
    assert_eq!(content_bounds(&rule, "absolute")[0], 87.0);
    assert_eq!((measured.get(), rule.host().stats().recompositions), (before, compositions));
}

fn offsets_round_to_whole_pixels() {
    // C++'s layout_is_in_whole_pixels_as_composes, its offset part: at 2.625 px a dp, moved by 1.84 and 0.53
    // px: 2 and 1.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(Modifier.offset(x = 0.7.dp(), y = 0.2.dp()).size(5.dp()).test_tag("kaymış"));
        });
    }));
    rule.host().set_density(Density::new(2.625, 1.0));
    rule.wait_for_idle();
    let (b, c) = (bounds(&rule, "kaymış"), content_bounds(&rule, "kaymış"));
    assert_eq!((c[0] - b[0], c[1] - b[1]), (2.0, 1.0));
    assert_eq!(b[2], 13.0);
}

// ---- Right to left ---------------------------------------------------------------------------------------------

fn rtl_layout_mirrors_rows_alignment_padding_and_offset() {
    // C++'s rtl_layout_mirrors_rows_alignment_padding_and_offset.
    let is_rtl = mutable_state_of(true);
    let r = is_rtl.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], || {
            Column(Modifier.width(200.dp()), || {
                Row(|| {
                    Box(Modifier.size(10.dp()).test_tag("a"));
                    Box(Modifier.size(20.dp()).test_tag("b"));
                    Box(Modifier.size(30.dp()).test_tag("c"));
                });
                Box(
                    Modifier.fill_max_width().height(20.dp()),
                    Alignment::TopStart,
                    content = || {
                        Box(Modifier.size(20.dp()).test_tag("başta"));
                    },
                );
                Row(
                    Modifier.fill_max_width(),
                    horizontal_arrangement = Arrangement::End,
                    content = || {
                        Box(Modifier.size(20.dp()).test_tag("sonda"));
                    },
                );
                Box(
                    Modifier.fill_max_width().padding_sides(start = 16.dp(), end = 4.dp()),
                    content = || {
                        Box(Modifier.fill_max_width().height(10.dp()).test_tag("dolgu"));
                    },
                );
                Box(
                    Modifier.fill_max_width().absolute_padding(left = 16.dp(), right = 4.dp()),
                    content = || {
                        Box(Modifier.fill_max_width().height(10.dp()).test_tag("mutlak dolgu"));
                    },
                );
                Box(
                    Modifier.size(20.dp()).offset(x = 10.dp()),
                    content = || {
                        Box(Modifier.fill_max_size().test_tag("kayık"));
                    },
                );
                Box(
                    Modifier.size(20.dp()).absolute_offset(x = 10.dp()),
                    content = || {
                        Box(Modifier.fill_max_size().test_tag("mutlak kayık"));
                    },
                );
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Ltr)], || {
                    Row(Modifier.fill_max_width(), || Box(Modifier.size(10.dp()).test_tag("içte soldan")));
                });
            });
        });
    }));
    // Right to left: the row starts at the right (and the column puts it there), Start is the right.
    assert!(near(x(&rule, "a"), 190.0) && near(x(&rule, "b"), 170.0) && near(x(&rule, "c"), 140.0));
    assert!(near(x(&rule, "başta"), 180.0) && near(x(&rule, "sonda"), 0.0));
    // 16 at the start: the right.
    assert!(near(x(&rule, "dolgu"), 4.0) && near(bounds(&rule, "dolgu")[2], 180.0));
    assert!(near(x(&rule, "mutlak dolgu"), 16.0));
    // Offset towards the end: left.
    assert!(near(x(&rule, "kayık"), 170.0) && near(x(&rule, "mutlak kayık"), 190.0));
    // A left-to-right part inside.
    assert!(near(x(&rule, "içte soldan"), 0.0));
    is_rtl.set(false);
    rule.wait_for_idle();
    assert!(near(x(&rule, "a"), 0.0) && near(x(&rule, "b"), 10.0) && near(x(&rule, "c"), 30.0));
    assert!(near(x(&rule, "başta"), 0.0) && near(x(&rule, "sonda"), 180.0));
    assert!(near(x(&rule, "dolgu"), 16.0) && near(x(&rule, "mutlak dolgu"), 16.0));
    assert!(near(x(&rule, "kayık"), 10.0) && near(x(&rule, "mutlak kayık"), 10.0));
}

fn compose_forms_of_measuring_and_placing() {
    // C++'s compose_forms_of_measuring_and_placing: Modifier.layout and a Layout's placement block, place_relative
    // from the start (mirrored right to left), place from the left.
    fn moved(from_left: bool) -> Modifier {
        Modifier.layout(move |scope, m, c| {
            let p = m.measure(Constraints::new(0, c.max_width, 0, c.max_height));
            scope.layout(60, p.height(), move |s| {
                if from_left {
                    s.place(&p, 10, 0);
                } else {
                    s.place_relative(&p, 10, 0);
                }
            })
        })
    }
    for is_rtl in [false, true] {
        let rule = KompozTestRule::new(kompozable_lambda!(move || {
            let direction = if is_rtl { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], || {
                Column(|| {
                    Box(Modifier.test_tag("göreli").then(moved(false)).size(20.dp()));
                    Box(Modifier.test_tag("soldan").then(moved(true)).size(20.dp()));
                    Layout(
                        content = || Box(Modifier.size(20.dp()).test_tag("çocuk")),
                        modifier = Modifier.test_tag("yerleşim"),
                        measure_policy = measure_policy(|scope, ms, c| {
                            let p = ms[0].measure(Constraints::new(0, c.max_width, 0, c.max_height));
                            scope.layout(100, 20, move |s| s.place_relative(&p, 10, 0))
                        }),
                    );
                    Layout(
                        content = || Box(Modifier.size(20.dp()).test_tag("soldaki")),
                        modifier = Modifier.test_tag("soldan yerleşim"),
                        measure_policy = measure_policy(|scope, ms, c| {
                            let p = ms[0].measure(Constraints::new(0, c.max_width, 0, c.max_height));
                            scope.layout(100, 20, move |s| s.place(&p, 10, 0))
                        }),
                    );
                });
            });
        }));
        let relative = bounds(&rule, "göreli");
        assert!(near(relative[2], 60.0) && near(relative[3], 20.0));
        assert!(near(content_bounds(&rule, "göreli")[0] - relative[0], if is_rtl { 30.0 } else { 10.0 }));
        assert!(near(content_bounds(&rule, "soldan")[0] - bounds(&rule, "soldan")[0], 10.0));
        let (layout, child) = (bounds(&rule, "yerleşim"), bounds(&rule, "çocuk"));
        assert!(near(layout[2], 100.0) && near(child[0] - layout[0], if is_rtl { 70.0 } else { 10.0 }));
        let (left, left_child) = (bounds(&rule, "soldan yerleşim"), bounds(&rule, "soldaki"));
        assert!(near(left_child[0] - left[0], 10.0));
    }
}

tests!(
    aspect_ratio_wrap_content_and_required_size,
    aspect_ratio_tries_the_constraints_in_composes_order,
    min_intrinsic_widths_are_the_widest_words,
    intrinsic_height_makes_a_divider_as_tall_as_the_row,
    intrinsic_size_modifiers_as_compose,
    padding_from_baseline_puts_text_on_a_grid,
    padding_from_alignment_lines_as_compose,
    offsets_move_what_they_wrap_and_mirror_right_to_left,
    a_new_offset_places_again_without_measuring,
    offset_px_places_again_without_measuring_or_composing,
    offsets_round_to_whole_pixels,
    rtl_layout_mirrors_rows_alignment_padding_and_offset,
    compose_forms_of_measuring_and_placing,
);

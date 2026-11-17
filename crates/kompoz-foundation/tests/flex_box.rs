//! foundation.layout's FlexBox: growing, shrinking and wrapping as CSS's flexbox, lines and their alignment,
//! stretching and baselines, configs run as it measures (reading the constraints and states), intrinsics, right
//! to left, reversed directions, gaps, and the configs' values and combinations.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---- Helpers ---------------------------------------------------------------------------------------------------

/// A node's outer bounds in the root: x, y, width, height (its test tag first in its modifier).
fn bounds(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    let b = rule.on_node_with_tag(tag).get_unclipped_bounds_in_root();
    [b.left, b.top, b.width(), b.height()]
}

/// A node's bounds relative to its container's: x, y, width, height.
fn within(rule: &KompozTestRule, container: &str, tag: &str) -> [f32; 4] {
    let (c, r) = (bounds(rule, container), bounds(rule, tag));
    [r[0] - c[0], r[1] - c[1], r[2], r[3]]
}

thread_local! {
    /// What a measuring layout saw of its child's intrinsic sizes.
    static SEEN: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    /// How many times a test's content was composed.
    static COMPOSITIONS: Cell<u32> = const { Cell::new(0) };
    /// How many times a layout counting its measurements was measured.
    static MEASURES: Cell<u32> = const { Cell::new(0) };
}

fn measures() -> u32 {
    MEASURES.with(|m| m.get())
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

fn seen() -> Vec<i32> {
    SEEN.with(|s| s.borrow().clone())
}

// ---- Ported from C++ -------------------------------------------------------------------------------------------

fn flex_box_grows_shrinks_and_wraps_as_css() {
    // C++'s flex_box_grows_shrinks_and_wraps_as_css. The config reads a state as the FlexBox measures (Compose's
    // way to change it without composing again); the children change by composing.
    let step = mutable_state_of(0);
    let kids = mutable_state_of(0);
    let (s, k) = (step.clone(), kids.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let kids = k.get();
        FlexBox(
            modifier = Modifier.test_tag("kutu").width(300.dp()),
            config = |c: &mut FlexBoxConfigScope| match s.get() {
                0 => {}
                1 => c.column_gap(15.dp()),
                2 => {
                    c.justify_content(FlexJustifyContent::SpaceBetween);
                    c.align_items(FlexAlignItems::Center);
                }
                3 => c.justify_content(FlexJustifyContent::SpaceEvenly),
                4 => c.direction(FlexDirection::RowReverse),
                _ => {
                    c.direction(FlexDirection::Column);
                    c.row_gap(5.dp());
                }
            },
            content = || {
                if kids == 0 {
                    // Grow 1 and 2, a third that doesn't.
                    Box(Modifier.test_tag("a").size_wh(50.dp(), 20.dp()).flex(|f: &mut FlexConfigScope| f.grow(1.0)));
                    Box(Modifier.test_tag("b").size_wh(50.dp(), 40.dp()).flex(|f: &mut FlexConfigScope| f.grow(2.0)));
                    Box(Modifier.test_tag("c").size_wh(50.dp(), 30.dp()));
                } else {
                    // Three 50s, the last first by its order; one at the end of its line.
                    Box(Modifier.test_tag("a").size_wh(50.dp(), 20.dp()));
                    Box(Modifier
                        .test_tag("b")
                        .size_wh(50.dp(), 40.dp())
                        .flex(|f: &mut FlexConfigScope| f.align_self(FlexAlignSelf::End)));
                    Box(Modifier.test_tag("c").size_wh(50.dp(), 30.dp()).flex(|f: &mut FlexConfigScope| f.order(-1)));
                }
            },
        );
    }));
    let at = |tag: &str| within(&rule, "kutu", tag);
    // Grow: the 150 left shared 1:2; as tall as the tallest.
    assert_eq!(
        [at("a"), at("b"), at("c")],
        [[0.0, 0.0, 100.0, 20.0], [100.0, 0.0, 150.0, 40.0], [250.0, 0.0, 50.0, 30.0]]
    );
    assert_eq!(bounds(&rule, "kutu")[3], 40.0);
    // With gaps, what's left after them.
    step.set(1);
    rule.wait_for_idle();
    assert_eq!([at("a")[2], at("b")[0], at("b")[2], at("c")[0]], [90.0, 105.0, 130.0, 250.0]);
    // Justify and align: order first, SpaceBetween, Center across, End for one.
    kids.set(2);
    step.set(2);
    rule.wait_for_idle();
    assert_eq!([at("c")[0], at("a")[0], at("b")[0]], [0.0, 125.0, 250.0]);
    assert_eq!([at("c")[1], at("a")[1], at("b")[1]], [5.0, 10.0, 0.0]);
    // C++ expected 38, 125 and 213 (spaces of 37.5 at whole px); Compose's SpaceEvenly divides the 150 left in
    // whole px: 150 / 4 = 37 before, between and after.
    step.set(3);
    rule.wait_for_idle();
    assert_eq!([at("c")[0], at("a")[0], at("b")[0]], [37.0, 124.0, 211.0]);
    // RowReverse: from the end.
    step.set(4);
    rule.wait_for_idle();
    assert_eq!([at("c")[0], at("a")[0], at("b")[0]], [250.0, 200.0, 150.0]);
    // A column stacks them, the row gap between.
    step.set(5);
    rule.wait_for_idle();
    assert_eq!([at("c")[1], at("a")[1], at("b")[1]], [0.0, 35.0, 60.0]);
    assert_eq!(bounds(&rule, "kutu")[3], 100.0);
}

fn flex_box_lines_and_shrinking() {
    // C++'s flex_box_lines_and_shrinking.
    let wrap = mutable_state_of(FlexWrap::Wrap);
    let w = wrap.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(
            content = || {
                // Three 50s in 120: two lines.
                FlexBox(
                    modifier = Modifier.test_tag("satırlar").width(120.dp()),
                    config = |c: &mut FlexBoxConfigScope| {
                        c.wrap(w.get());
                        c.row_gap(5.dp());
                    },
                    content = || {
                        Box(Modifier.test_tag("x").size_wh(50.dp(), 20.dp()));
                        Box(Modifier.test_tag("y").size_wh(50.dp(), 20.dp()));
                        Box(Modifier.test_tag("z").size_wh(50.dp(), 20.dp()));
                    },
                );
                // Too wide for a line: on the first line on its own, not after an empty one.
                FlexBox(
                    modifier = Modifier.test_tag("geniş").width(120.dp()),
                    config = |c: &mut FlexBoxConfigScope| {
                        c.wrap(FlexWrap::Wrap);
                        c.row_gap(5.dp());
                    },
                    content = || {
                        Box(Modifier.test_tag("iri").size_wh(150.dp(), 20.dp()));
                        Box(Modifier.test_tag("ufak").size_wh(50.dp(), 20.dp()));
                    },
                );
                // 80 dp bases shrunk into 100; one with a 70 dp minimum.
                FlexBox(
                    modifier = Modifier.width(100.dp()),
                    config = DefaultFlexBoxConfig,
                    content = || {
                        Box(Modifier.test_tag("p").height(20.dp()).flex(|f: &mut FlexConfigScope| f.basis_dp(80.dp())));
                        Box(Modifier.test_tag("q").height(20.dp()).flex(|f: &mut FlexConfigScope| f.basis_dp(80.dp())));
                    },
                );
                FlexBox(
                    modifier = Modifier.width(100.dp()),
                    content = || {
                        Box(Modifier.test_tag("r").height(20.dp()).flex(|f: &mut FlexConfigScope| f.basis_dp(80.dp())));
                        Box(Modifier
                            .test_tag("s")
                            .width_in(min = 70.dp())
                            .height(20.dp())
                            .flex(|f: &mut FlexConfigScope| f.basis(FlexBasis::Dp(80.dp()))));
                    },
                );
                // Half the box, and a share of grow under 1 (only that share of the room).
                FlexBox(
                    modifier = Modifier.width(300.dp()),
                    content = || {
                        Box(Modifier
                            .test_tag("yarım")
                            .height(20.dp())
                            .flex(|f: &mut FlexConfigScope| f.basis(FlexBasis::Percent(0.5))));
                        Box(Modifier
                            .test_tag("az")
                            .size_wh(50.dp(), 20.dp())
                            .flex(|f: &mut FlexConfigScope| f.grow(0.5)));
                    },
                );
            },
        );
    }));
    let at = |tag: &str| within(&rule, "satırlar", tag);
    assert_eq!([at("x")[0], at("y")[0], at("z")[0], at("z")[1]], [0.0, 50.0, 0.0, 25.0]);
    assert_eq!(bounds(&rule, "satırlar")[3], 45.0);
    assert_eq!([within(&rule, "geniş", "iri")[1], within(&rule, "geniş", "ufak")[1]], [0.0, 25.0]);
    // WrapReverse: the first line at the bottom. C++ expected x 25 down and z at the top, in a box of 45; Compose
    // spreads the lines in the room the lines take without their gaps (40), so the gap is taken before the last
    // line: z 5 above the box, x 20 down, the box 40 high.
    wrap.set(FlexWrap::WrapReverse);
    rule.wait_for_idle();
    assert_eq!([at("x")[1], at("y")[1], at("z")[1]], [20.0, 20.0, -5.0]);
    assert_eq!(bounds(&rule, "satırlar")[3], 40.0);
    // Shrunk by their bases; one stops at its minimum, the other takes what's left.
    assert_eq!([bounds(&rule, "p")[2], bounds(&rule, "q")[2]], [50.0, 50.0]);
    assert_eq!([bounds(&rule, "s")[2], bounds(&rule, "r")[2]], [70.0, 30.0]);
    // Half of the 300; half of the 100 left.
    assert_eq!([bounds(&rule, "yarım")[2], bounds(&rule, "az")[2]], [150.0, 100.0]);
}

fn flex_box_stretches_aligns_lines_and_baselines() {
    // C++'s flex_box_stretches_aligns_lines_and_baselines.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Column(
                content = || {
                    // Stretch: a child without a height takes its line's.
                    FlexBox(
                        modifier = Modifier.width(200.dp()),
                        config = |c: &mut FlexBoxConfigScope| c.align_items(FlexAlignItems::Stretch),
                        content = || {
                            Box(Modifier.size_wh(50.dp(), 40.dp()));
                            Box(Modifier.test_tag("uzar").width(50.dp()));
                        },
                    );
                    // Two lines in 200 of height, spread by SpaceBetween.
                    FlexBox(
                        modifier = Modifier.test_tag("yayık").size_wh(100.dp(), 200.dp()),
                        config = |c: &mut FlexBoxConfigScope| {
                            c.wrap(FlexWrap::Wrap);
                            c.align_content(FlexAlignContent::SpaceBetween);
                        },
                        content = || {
                            Box(Modifier.test_tag("üst").size_wh(60.dp(), 20.dp()));
                            Box(Modifier.test_tag("alt").size_wh(60.dp(), 20.dp()));
                        },
                    );
                    // Texts of two sizes on one baseline.
                    FlexBox(
                        modifier = Modifier.width(300.dp()),
                        config = |c: &mut FlexBoxConfigScope| c.align_items(FlexAlignItems::Baseline),
                        content = || {
                            Text("küçük", font_size = 10.sp(), modifier = Modifier.test_tag("k"));
                            Text("büyük", font_size = 24.sp(), modifier = Modifier.test_tag("b"));
                        },
                    );
                    // The config by the constraints: wraps only when narrow.
                    FlexBox(
                        modifier = Modifier.test_tag("dar").width(150.dp()),
                        config = |c: &mut FlexBoxConfigScope| {
                            c.wrap(if c.constraints().max_width < 200 { FlexWrap::Wrap } else { FlexWrap::NoWrap })
                        },
                        content = || {
                            Box(Modifier.size_wh(100.dp(), 20.dp()));
                            Box(Modifier.test_tag("ikinci").size_wh(100.dp(), 20.dp()));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 1200),
    );
    assert_eq!(bounds(&rule, "uzar")[3], 40.0);
    assert_eq!([within(&rule, "yayık", "üst")[1], within(&rule, "yayık", "alt")[1]], [0.0, 180.0]);
    let baseline = |tag: &str| {
        bounds(&rule, tag)[1]
            + rule.on_node_with_tag(tag).fetch_semantics_node().get_alignment_line_position(FirstBaseline) as f32
    };
    assert_eq!(baseline("k"), baseline("b"));
    assert_ne!(bounds(&rule, "k")[1], bounds(&rule, "b")[1]);
    assert_eq!([within(&rule, "dar", "ikinci")[0], within(&rule, "dar", "ikinci")[1]], [0.0, 20.0]);
}

fn layouts_take_compose_arguments_and_config_scopes() {
    // The FlexBox part of C++'s layouts_take_compose_arguments_and_config_scopes: a config scope with a column gap
    // and a justification by the constraints, a child's `flex { grow(1) }`.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                FlexBox(
                    modifier = Modifier.test_tag("esnek").width(200.dp()),
                    config = |c: &mut FlexBoxConfigScope| {
                        c.column_gap(10.dp());
                        if c.constraints().max_width < 300 {
                            c.justify_content(FlexJustifyContent::Start);
                        }
                    },
                    content = || {
                        Box(Modifier.test_tag("sabit").size(40.dp()));
                        Box(Modifier.test_tag("büyüyen").height(40.dp()).flex(|f: &mut FlexConfigScope| f.grow(1.0)));
                    },
                );
            },
        );
    }));
    let grown = within(&rule, "esnek", "büyüyen");
    assert_eq!([grown[0], grown[2]], [50.0, 150.0]);
}

// ---- Compose's behaviours --------------------------------------------------------------------------------------

fn flex_box_intrinsics_break_lines_as_it_would() {
    let config = mutable_state_of((FlexDirection::Row, FlexWrap::Wrap));
    let c = config.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Intrinsics(50, 60, || {
            FlexBox(
                config = |s: &mut FlexBoxConfigScope| {
                    let (direction, wrap) = c.get();
                    s.direction(direction);
                    s.wrap(wrap);
                    s.gap_row_column(5.dp(), 10.dp());
                },
                content = || {
                    Box(Modifier.size_wh(30.dp(), 20.dp()));
                    Box(Modifier.size_wh(50.dp(), 40.dp()));
                },
            );
        });
    }));
    // A wrapping row: the widest child at least, all of them (and the gap) at most; across, two lines in 60
    // (20, the row gap, 40).
    assert_eq!(seen(), [50, 90, 65, 65]);
    // On one line: all of them both ways, as tall as the tallest.
    config.set((FlexDirection::Row, FlexWrap::NoWrap));
    rule.wait_for_idle();
    assert_eq!(seen(), [90, 90, 40, 40]);
    // A wrapping column: across, two lines in 50 (30, the column gap, 50); along, the tallest at least.
    config.set((FlexDirection::Column, FlexWrap::Wrap));
    rule.wait_for_idle();
    assert_eq!(seen(), [90, 90, 40, 65]);
    config.set((FlexDirection::ColumnReverse, FlexWrap::NoWrap));
    rule.wait_for_idle();
    assert_eq!(seen(), [50, 50, 65, 65]);
}

fn flex_box_mirrors_right_to_left() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
            Column(
                content = || {
                    FlexBox(
                        modifier = Modifier.test_tag("row").width(100.dp()),
                        content = || {
                            Box(Modifier.test_tag("a").size(10.dp()));
                            Box(Modifier.test_tag("b").size(20.dp()));
                        },
                    );
                    FlexBox(
                        modifier = Modifier.test_tag("reverse").width(100.dp()),
                        config = |c: &mut FlexBoxConfigScope| c.direction(FlexDirection::RowReverse),
                        content = || {
                            Box(Modifier.test_tag("ra").size(10.dp()));
                            Box(Modifier.test_tag("rb").size(20.dp()));
                        },
                    );
                    // Lines from the right in a column.
                    FlexBox(
                        modifier = Modifier.test_tag("column").size_wh(100.dp(), 30.dp()),
                        config = |c: &mut FlexBoxConfigScope| {
                            c.direction(FlexDirection::Column);
                            c.wrap(FlexWrap::Wrap);
                        },
                        content = || {
                            Box(Modifier.test_tag("ca").size(20.dp()));
                            Box(Modifier.test_tag("cb").size(20.dp()));
                        },
                    );
                },
            );
        });
    }));
    assert_eq!([within(&rule, "row", "a")[0], within(&rule, "row", "b")[0]], [90.0, 70.0]);
    assert_eq!([within(&rule, "reverse", "ra")[0], within(&rule, "reverse", "rb")[0]], [0.0, 10.0]);
    assert_eq!([within(&rule, "column", "ca")[0], within(&rule, "column", "cb")[0]], [80.0, 60.0]);
    assert_eq!([within(&rule, "column", "ca")[1], within(&rule, "column", "cb")[1]], [0.0, 0.0]);
}

fn flex_box_reverses_columns_and_their_lines() {
    let wrap = mutable_state_of(FlexWrap::Wrap);
    let w = wrap.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        FlexBox(
            modifier = Modifier.test_tag("box").size_wh(100.dp(), 50.dp()),
            config = |c: &mut FlexBoxConfigScope| {
                c.direction(FlexDirection::ColumnReverse);
                c.wrap(w.get());
            },
            content = || {
                Box(Modifier.test_tag("a").size_wh(30.dp(), 20.dp()));
                Box(Modifier.test_tag("b").size_wh(30.dp(), 20.dp()));
                Box(Modifier.test_tag("c").size_wh(30.dp(), 20.dp()));
            },
        );
    }));
    let at = |tag: &str| {
        let r = within(&rule, "box", tag);
        [r[0], r[1]]
    };
    // From the bottom up, a new line to the right.
    assert_eq!([at("a"), at("b"), at("c")], [[0.0, 30.0], [0.0, 10.0], [30.0, 30.0]]);
    // New lines to the left, from the right edge.
    wrap.set(FlexWrap::WrapReverse);
    rule.wait_for_idle();
    assert_eq!([at("a"), at("b"), at("c")], [[70.0, 30.0], [70.0, 10.0], [40.0, 30.0]]);
}

fn flex_box_spreads_lines_as_align_content_says_relaying_out_only() {
    COMPOSITIONS.with(|c| c.set(0));
    let align = mutable_state_of(FlexAlignContent::Start);
    let a = align.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        COMPOSITIONS.with(|c| c.set(c.get() + 1));
        FlexBox(
            modifier = Modifier.test_tag("box").size(100.dp()),
            config = |c: &mut FlexBoxConfigScope| {
                c.wrap(FlexWrap::Wrap);
                c.align_content(a.get());
            },
            content = || {
                Box(Modifier.test_tag("one").size_wh(60.dp(), 20.dp()));
                Box(Modifier.test_tag("two").size_wh(60.dp(), 20.dp()));
            },
        );
    }));
    let composed = COMPOSITIONS.with(|c| c.get());
    let lines = |content: FlexAlignContent| {
        align.set(content);
        rule.wait_for_idle();
        let (one, two) = (within(&rule, "box", "one"), within(&rule, "box", "two"));
        [one[1], one[3], two[1], two[3]]
    };
    assert_eq!(lines(FlexAlignContent::Start), [0.0, 20.0, 20.0, 20.0]);
    assert_eq!(lines(FlexAlignContent::End), [60.0, 20.0, 80.0, 20.0]);
    assert_eq!(lines(FlexAlignContent::Center), [30.0, 20.0, 50.0, 20.0]);
    assert_eq!(lines(FlexAlignContent::SpaceBetween), [0.0, 20.0, 80.0, 20.0]);
    // 60 left: 30 around each line, 15 at the ends.
    assert_eq!(lines(FlexAlignContent::SpaceAround), [15.0, 20.0, 65.0, 20.0]);
    // Each line 30 bigger; the children stretched to it (even over their size modifier's 20, within the fixed
    // constraints they are given).
    assert_eq!(lines(FlexAlignContent::Stretch), [0.0, 50.0, 50.0, 50.0]);
    // Read as it measures: laid out again, never composed again.
    assert_eq!(COMPOSITIONS.with(|c| c.get()), composed);
}

fn flex_box_gaps_and_baseline_lines() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                FlexBox(
                    modifier = Modifier.test_tag("all").width(100.dp()),
                    config = |c: &mut FlexBoxConfigScope| {
                        c.wrap(FlexWrap::Wrap);
                        c.gap(10.dp());
                    },
                    content = || {
                        Box(Modifier.test_tag("a1").size_wh(40.dp(), 10.dp()));
                        Box(Modifier.test_tag("a2").size_wh(40.dp(), 10.dp()));
                        Box(Modifier.test_tag("a3").size_wh(40.dp(), 10.dp()));
                    },
                );
                FlexBox(
                    modifier = Modifier.test_tag("each").width(100.dp()),
                    config = |c: &mut FlexBoxConfigScope| {
                        c.wrap(FlexWrap::Wrap);
                        c.gap_row_column(4.dp(), 6.dp());
                    },
                    content = || {
                        Box(Modifier.test_tag("e1").size_wh(40.dp(), 10.dp()));
                        Box(Modifier.test_tag("e2").size_wh(40.dp(), 10.dp()));
                        Box(Modifier.test_tag("e3").size_wh(40.dp(), 10.dp()));
                    },
                );
                // Boxes have no baseline: aligned by their bottoms; one by its middle.
                FlexBox(
                    modifier = Modifier.test_tag("lines"),
                    config = |c: &mut FlexBoxConfigScope| c.align_items_by_line(FirstBaseline),
                    content = || {
                        Box(Modifier.test_tag("low").size(10.dp()));
                        Box(Modifier.test_tag("high").size(30.dp()));
                        Box(Modifier
                            .test_tag("middle")
                            .size(20.dp())
                            .flex(|f: &mut FlexConfigScope| f.align_self_by(|p: &Placeable| p.height() / 2)));
                    },
                );
                // A block for all of them; one by a line of its own, one not aligned by a baseline.
                FlexBox(
                    modifier = Modifier.test_tag("block"),
                    config = |c: &mut FlexBoxConfigScope| c.align_items_by(|p: &Placeable| p.height() / 2),
                    content = || {
                        Box(Modifier.test_tag("k1").size(10.dp()));
                        Box(Modifier.test_tag("k2").size(30.dp()));
                        Box(Modifier
                            .test_tag("k3")
                            .size(20.dp())
                            .flex(|f: &mut FlexConfigScope| f.align_self_by_line(LastBaseline)));
                        Box(Modifier
                            .test_tag("k4")
                            .size(20.dp())
                            .flex(|f: &mut FlexConfigScope| f.align_self(FlexAlignSelf::End)));
                    },
                );
            },
        );
    }));
    let at = |container: &str, tag: &str| {
        let r = within(&rule, container, tag);
        [r[0], r[1]]
    };
    assert_eq!([at("all", "a1"), at("all", "a2"), at("all", "a3")], [[0.0, 0.0], [50.0, 0.0], [0.0, 20.0]]);
    assert_eq!([at("each", "e1"), at("each", "e2"), at("each", "e3")], [[0.0, 0.0], [46.0, 0.0], [0.0, 14.0]]);
    // 30 above the line (the high one), 10 below (the middle one's lower half).
    assert_eq!([at("lines", "low")[1], at("lines", "high")[1], at("lines", "middle")[1]], [20.0, 0.0, 20.0]);
    assert_eq!(bounds(&rule, "lines")[3], 40.0);
    // Middles at 20, and k3's bottom (it has no last baseline); k4 at the line's end (35 high: 20 above, 15 below).
    assert_eq!(
        [at("block", "k1")[1], at("block", "k2")[1], at("block", "k3")[1], at("block", "k4")[1]],
        [15.0, 5.0, 0.0, 15.0]
    );
    assert_eq!(bounds(&rule, "block")[3], 35.0);
}

fn flex_box_bases_orders_and_shrinking() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                // Unbounded along: a percent basis is the child's own size.
                FlexBox(
                    modifier = Modifier.wrap_content_width(unbounded = true),
                    content = || {
                        Box(Modifier
                            .test_tag("unbounded")
                            .size_wh(40.dp(), 10.dp())
                            .flex(|f: &mut FlexConfigScope| f.basis_percent(0.5)));
                    },
                );
                // Orders: ascending, the same ones as composed; one that doesn't shrink.
                FlexBox(
                    modifier = Modifier.test_tag("orders").width(100.dp()),
                    content = || {
                        Box(Modifier
                            .test_tag("o1")
                            .size_wh(40.dp(), 10.dp())
                            .flex(|f: &mut FlexConfigScope| f.order(1)));
                        Box(Modifier.test_tag("o2").size_wh(40.dp(), 10.dp()));
                        Box(Modifier.test_tag("o3").size_wh(40.dp(), 10.dp()).flex(|f: &mut FlexConfigScope| {
                            f.order(-1);
                            f.shrink(0.0);
                        }));
                        Box(Modifier.test_tag("o4").size_wh(40.dp(), 10.dp()));
                    },
                );
            },
        );
    }));
    assert_eq!(bounds(&rule, "unbounded")[2], 40.0);
    // 160 in 100: o3 keeps its 40, the three others (min intrinsic 40 too: their size modifiers) can't shrink.
    let x = |tag: &str| within(&rule, "orders", tag)[0];
    assert_eq!([x("o3"), x("o2"), x("o4"), x("o1")], [0.0, 40.0, 80.0, 120.0]);
    assert_eq!(bounds(&rule, "orders")[2], 100.0);
}

fn flex_box_keeps_its_policy_while_its_config_is_the_same() {
    MEASURES.with(|m| m.set(0));
    // Made once, so it stays equal: it counts the measurements of the FlexBoxes it decorates.
    let counting = Modifier.layout(|scope, measurable, constraints| {
        MEASURES.with(|m| m.set(m.get() + 1));
        let p = measurable.measure(constraints);
        scope.layout(p.width(), p.height(), move |s| s.place(&p, 0, 0))
    });
    let shared: Rc<dyn FlexBoxConfig> = Rc::new(|c: &mut FlexBoxConfigScope| c.wrap(FlexWrap::Wrap));
    let tick = mutable_state_of(0);
    let gap = mutable_state_of(4);
    let (t, g) = (tick.clone(), gap.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let _ = t.get();
        let gap = g.get();
        Column(
            content = || {
                FlexBox(
                    modifier = counting.clone(),
                    config = |c: &mut FlexBoxConfigScope| c.column_gap(gap.dp()),
                    content = || Box(Modifier.size(10.dp())),
                );
                FlexBox(modifier = counting.clone(), config = shared.clone(), content = || Box(Modifier.size(10.dp())));
                FlexBox(modifier = counting.clone(), content = || Box(Modifier.size(10.dp())));
            },
        );
    }));
    rule.set_layout_check(false);
    let before = measures();
    assert!(before >= 3);
    // Composed again with the same configs (a closure capturing the same values): none measured again.
    tick.set(1);
    rule.wait_for_idle();
    assert_eq!(measures(), before);
    // A closure capturing another value: a new policy, measured again.
    gap.set(8);
    rule.wait_for_idle();
    assert_eq!(measures(), before + 1);
}

fn flex_values_configs_and_their_combinations() {
    assert_eq!(FlexDirection::RowReverse.to_string(), "RowReverse");
    assert_eq!(format!("{:?}", FlexWrap::WrapReverse), "WrapReverse");
    assert_eq!(FlexAlignSelf::Auto.to_string(), "Auto");
    assert_eq!(FlexJustifyContent::SpaceEvenly.to_string(), "SpaceEvenly");
    assert_eq!(FlexBasis::Auto.to_string(), "FlexBasis.Auto");
    assert_eq!(FlexBasis::Dp(10.dp()).to_string(), "FlexBasis.Dp(10.0.dp)");
    assert_eq!(FlexBasis::Percent(0.25).to_string(), "FlexBasis.Percent(25%)");
    assert_eq!(FlexBasis::Dp(10.dp()), FlexBasis::Dp(10.dp()));
    assert_ne!(FlexBasis::Dp(10.dp()), FlexBasis::Percent(10.0));

    // The default gives the other; combinations of the same configs are equal; closures aren't.
    let grow: Rc<dyn FlexConfig> = Rc::new(|f: &mut FlexConfigScope| f.grow(1.0));
    let shrink: Rc<dyn FlexConfig> = Rc::new(|f: &mut FlexConfigScope| f.shrink(0.0));
    let default: Rc<dyn FlexConfig> = Rc::new(DefaultFlexConfig);
    assert!(Rc::ptr_eq(&DefaultFlexConfig.then(grow.clone()), &grow));
    assert!(Rc::ptr_eq(&grow.clone().then(DefaultFlexConfig), &grow));
    assert!(Rc::ptr_eq(&FlexConfig((DefaultFlexConfig, grow.clone(), DefaultFlexConfig)), &grow));
    assert!(Rc::ptr_eq(&FlexConfig([default.clone(), grow.clone()]), &grow));
    assert_eq!(Modifier.flex(FlexConfig([default.clone()])), Modifier.flex(DefaultFlexConfig));
    assert_eq!(Modifier.flex(grow.clone()), Modifier.flex(grow.clone()));
    assert_eq!(
        Modifier.flex(FlexConfig((grow.clone(), shrink.clone()))),
        Modifier.flex(grow.clone().then(shrink.clone()))
    );
    assert_eq!(
        Modifier.flex(FlexConfig((grow.clone(), shrink.clone(), grow.clone()))),
        Modifier.flex(FlexConfig([grow.clone(), shrink.clone(), grow.clone()]))
    );
    // (combined ones spread into three)
    assert_eq!(
        Modifier.flex(FlexConfig((FlexConfig((grow.clone(), shrink.clone())), grow.clone(), shrink.clone()))),
        Modifier.flex(FlexConfig([grow.clone(), shrink.clone(), grow.clone(), shrink.clone()]))
    );
    assert_ne!(Modifier.flex(grow.clone()), Modifier.flex(shrink.clone()));
    assert_ne!(
        Modifier.flex(|f: &mut FlexConfigScope| f.grow(1.0)),
        Modifier.flex(|f: &mut FlexConfigScope| f.grow(1.0))
    );

    // Combined container configs, later ones over earlier ones; the scope as Compose writes it.
    let written = Rc::new(RefCell::new(String::new()));
    let wr = written.clone();
    let wrap: Rc<dyn FlexBoxConfig> = Rc::new(|c: &mut FlexBoxConfigScope| {
        c.wrap(FlexWrap::Wrap);
        c.gap(4.dp());
    });
    let config = FlexBoxConfig((
        wrap,
        |c: &mut FlexBoxConfigScope| c.column_gap(6.dp()),
        move |c: &mut FlexBoxConfigScope| *wr.borrow_mut() = c.to_string(),
    ));
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        FlexBox(
            modifier = Modifier.test_tag("box").width(100.dp()),
            config = config.clone(),
            content = || {
                Box(Modifier.test_tag("a").size_wh(50.dp(), 10.dp()));
                Box(Modifier.test_tag("b").size_wh(50.dp(), 10.dp()));
            },
        );
    }));
    assert_eq!(within(&rule, "box", "b")[1], 14.0);
    assert_eq!(
        *written.borrow(),
        "FlexBoxConfig(\n    direction = Row,\n    wrap = Wrap,\n    justifyContent = Start,\n    alignItems = Start,\n    \
         alignContent = Start,\n    rowGap = 4.0.dp,\n    columnGap = 6.0.dp\n)"
    );
}

tests!(
    flex_box_grows_shrinks_and_wraps_as_css,
    flex_box_lines_and_shrinking,
    flex_box_stretches_aligns_lines_and_baselines,
    layouts_take_compose_arguments_and_config_scopes,
    flex_box_intrinsics_break_lines_as_it_would,
    flex_box_mirrors_right_to_left,
    flex_box_reverses_columns_and_their_lines,
    flex_box_spreads_lines_as_align_content_says_relaying_out_only,
    flex_box_gaps_and_baseline_lines,
    flex_box_bases_orders_and_shrinking,
    flex_box_keeps_its_policy_while_its_config_is_the_same,
    flex_values_configs_and_their_combinations,
);

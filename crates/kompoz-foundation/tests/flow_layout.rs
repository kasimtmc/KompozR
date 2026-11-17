//! FlowRow, FlowColumn and the contextual flow layouts, as Compose's: children wrapped into lines (C++'s FlowRow
//! tests ported onto Compose's), lines limited with their overflow clipped or replaced by indicators, a line's
//! weights and `fill_max_row_height`, alignments, right to left, intrinsics, and items composed knowing their
//! line.

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

/// A node's bounds in the root: x, y, width, height (px; the rule's density is 1).
fn bounds(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    let b = rule.on_node_with_tag(tag).get_unclipped_bounds_in_root();
    [b.left, b.top, b.width(), b.height()]
}

/// A node's top left relative to another's.
fn at(rule: &KompozTestRule, tag: &str, parent: &str) -> (f32, f32) {
    let (b, p) = (bounds(rule, tag), bounds(rule, parent));
    (b[0] - p[0], b[1] - p[1])
}

fn displayed(rule: &KompozTestRule, tag: &str) -> bool {
    rule.on_node_with_tag(tag).is_displayed()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

#[kompozable]
fn Item(tag: String, width: i32, height: i32) {
    Box(Modifier.size_wh(width.dp(), height.dp()).test_tag(tag));
}

// ---- C++ 12179: intrinsic_height_and_flow_row (its FlowRow half) ------------------------------------------------

fn flow_row_wraps_with_spacing_and_a_most_per_row() {
    // C++'s intrinsic_height_and_flow_row, its FlowRow half (the intrinsic height half is layout_modifiers.rs'):
    // 60 + 10 + 60 + 10 + 60 = 200 fits three, then two, the rows 5 apart; at most two to a row.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            FlowRow(
                modifier = Modifier.width(200.dp()).test_tag("flow"),
                horizontal_arrangement = Arrangement::spaced_by(10.dp()),
                vertical_arrangement = Arrangement::spaced_by(5.dp()),
                content = || {
                    for i in 0..5 {
                        Item(format!("chip{i}"), 60, 20);
                    }
                },
            );
            FlowRow(
                modifier = Modifier.width(400.dp()).test_tag("limited"),
                max_items_in_each_row = 2,
                content = || {
                    for i in 0..3 {
                        Item(format!("limited{i}"), 30, 30);
                    }
                },
            );
        });
    }));
    assert_eq!(at(&rule, "chip2", "flow"), (140.0, 0.0));
    assert_eq!(at(&rule, "chip3", "flow"), (0.0, 25.0));
    assert_eq!(bounds(&rule, "chip3")[2..], [60.0, 20.0]);
    assert!(near(bounds(&rule, "flow")[3], 45.0) && near(bounds(&rule, "flow")[2], 200.0));
    assert_eq!(at(&rule, "limited1", "limited"), (30.0, 0.0));
    assert_eq!(at(&rule, "limited2", "limited"), (0.0, 30.0));
}

// ---- C++ 11896: layouts_take_compose_arguments_and_config_scopes (its FlowRow part) -----------------------------

fn flow_row_takes_compose_arguments() {
    // C++'s layouts_take_compose_arguments_and_config_scopes, its FlowRow part: spacedBy(10) in 100 fits two 40s.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        FlowRow(
            modifier = Modifier.width(100.dp()).test_tag("akış"),
            horizontal_arrangement = Arrangement::spaced_by(10.dp()),
            content = || {
                for i in 0..3 {
                    Item(format!("akış{i}"), 40, 40);
                }
            },
        );
    }));
    assert_eq!(at(&rule, "akış1", "akış").0, 50.0);
    assert_eq!(at(&rule, "akış2", "akış"), (0.0, 40.0));
}

// ---- Lines and overflow ----------------------------------------------------------------------------------------

fn max_lines_clip_what_is_past_them() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        FlowRow(
            modifier = Modifier.width(100.dp()).test_tag("satır"),
            max_lines = 1,
            content = || {
                for i in 0..3 {
                    Item(format!("k{i}"), 40, 20);
                }
            },
        );
    }));
    assert!(displayed(&rule, "k0") && displayed(&rule, "k1") && !displayed(&rule, "k2"));
    assert_eq!(bounds(&rule, "satır")[3], 20.0);
}

fn visible_overflow_places_every_line() {
    // 20 tall: the second row doesn't fit across. Clipped (the default), it isn't placed; visible, it is, below,
    // at the size it was first measured.
    let visible = mutable_state_of(false);
    let v = visible.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        FlowRow(
            modifier = Modifier.width(100.dp()).height(20.dp()).test_tag("taşan"),
            overflow = if v.get() { FlowRowOverflow::Visible } else { FlowRowOverflow::Clip },
            content = || {
                for i in 0..3 {
                    Item(format!("v{i}"), 40, 20);
                }
            },
        );
    }));
    assert!(!rule.on_node_with_tag("v2").layout_node().is_placed());
    visible.set(true);
    rule.wait_for_idle();
    assert!(rule.on_node_with_tag("v2").layout_node().is_placed());
    assert_eq!(at(&rule, "v2", "taşan"), (0.0, 20.0));
    assert_eq!(bounds(&rule, "v2")[2..], [40.0, 20.0]); // (its first measure kept: Compose measures it once)
}

fn an_expand_indicator_takes_the_place_of_what_doesnt_fit() {
    // Four 40s in 100 on one line with a 30 wide indicator: 40 + 40 + 30 > 100, so one item and the indicator;
    // its scope knows the counts once the layout measured (read as it draws).
    let counts = Rc::new(Cell::new((0, 0)));
    let c = counts.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        let overflow = FlowRowOverflow::expand_indicator(move |scope: &FlowRowOverflowScope| {
            let (s, c) = (scope.clone(), c.clone());
            Box(Modifier
                .size_wh(30.dp(), 20.dp())
                .test_tag("daha")
                .draw_behind(move |_| c.set((s.total_item_count(), s.shown_item_count()))));
        });
        FlowRow(
            modifier = Modifier.width(100.dp()).test_tag("akış"),
            max_lines = 1,
            overflow = overflow,
            content = || {
                for i in 0..4 {
                    Item(format!("e{i}"), 40, 20);
                }
            },
        );
    }));
    rule.draw();
    assert!(displayed(&rule, "e0") && !displayed(&rule, "e1") && displayed(&rule, "daha"));
    assert_eq!(at(&rule, "daha", "akış"), (40.0, 0.0));
    assert_eq!(counts.get(), (4, 1));
}

fn shown_item_count_panics_before_the_layout_measured() {
    // Compose's lazy count throws when read as the indicator composes (before the measure).
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(|| {
        KompozTestRule::new(kompozable_lambda!(|| {
            let overflow = FlowRowOverflow::expand_indicator(|scope: &FlowRowOverflowScope| {
                Text(format!("+{}", scope.shown_item_count()));
            });
            FlowRow(max_lines = 1, overflow = overflow, content = || Item("x".to_string(), 40, 20));
        }))
    });
    std::panic::set_hook(hook);
    let message = r.err().and_then(|p| p.downcast_ref::<String>().cloned()).unwrap_or_default();
    assert!(message.contains("Accessing shownItemCount before it is set"), "{message}");
}

fn expand_or_collapse_indicators_switch_with_the_lines() {
    // One line: an item and the expand indicator. Every line: the four items in two rows, and the collapse
    // indicator on a line of its own (it doesn't fit after the last two).
    let max_lines = mutable_state_of(1);
    let m = max_lines.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let overflow = FlowRowOverflow::expand_or_collapse_indicator(
            |_: &FlowRowOverflowScope| Item("aç".to_string(), 30, 20),
            |_: &FlowRowOverflowScope| Item("kapat".to_string(), 30, 20),
            2,
            0.dp(),
        );
        FlowRow(
            modifier = Modifier.width(100.dp()).test_tag("akış"),
            max_lines = m.get(),
            overflow = overflow,
            content = || {
                for i in 0..4 {
                    Item(format!("o{i}"), 40, 20);
                }
            },
        );
    }));
    assert!(displayed(&rule, "aç") && !displayed(&rule, "kapat") && !displayed(&rule, "o1"));
    assert_eq!(at(&rule, "aç", "akış"), (40.0, 0.0));
    assert_eq!(bounds(&rule, "akış")[3], 20.0);
    max_lines.set(i32::MAX);
    rule.wait_for_idle();
    assert!(!displayed(&rule, "aç") && displayed(&rule, "kapat") && displayed(&rule, "o3"));
    assert_eq!(at(&rule, "o3", "akış"), (40.0, 20.0));
    assert_eq!(at(&rule, "kapat", "akış"), (0.0, 40.0));
    assert_eq!(bounds(&rule, "akış")[3], 60.0);
}

// ---- Within a line ---------------------------------------------------------------------------------------------

fn a_line_shares_weights_and_fills_its_height() {
    // The 40 tall item makes the row 40: `fill_max_row_height` is measured that tall; the weighted one takes what
    // the row leaves (200 - 50 - 50).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        FlowRow(
            modifier = Modifier.width(200.dp()).test_tag("akış"),
            content = || {
                Box(Modifier.size_wh(50.dp(), 40.dp()));
                Box(Modifier.width(50.dp()).fill_max_row_height(fraction = 1.0).test_tag("dolu"));
                Box(Modifier.height(20.dp()).weight(1.0).test_tag("ağırlık"));
            },
        );
    }));
    assert_eq!(bounds(&rule, "dolu")[2..], [50.0, 40.0]);
    assert_eq!(at(&rule, "ağırlık", "akış"), (100.0, 0.0));
    assert_eq!(bounds(&rule, "ağırlık")[2..], [100.0, 20.0]);
    assert_eq!(bounds(&rule, "akış")[2..], [200.0, 40.0]);
}

fn items_align_in_their_row_unless_they_say_otherwise() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        FlowRow(
            modifier = Modifier.width(200.dp()).test_tag("akış"),
            item_vertical_alignment = Alignment::CenterVertically,
            content = || {
                Item("uzun".to_string(), 40, 40);
                Item("orta".to_string(), 40, 20);
                Box(Modifier.size_wh(40.dp(), 20.dp()).align(Alignment::Bottom).test_tag("alt"));
            },
        );
    }));
    assert_eq!(at(&rule, "orta", "akış"), (40.0, 10.0));
    assert_eq!(at(&rule, "alt", "akış"), (80.0, 20.0));
}

fn flow_row_starts_from_the_right_in_rtl() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
            FlowRow(
                modifier = Modifier.width(100.dp()).test_tag("akış"),
                content = || {
                    for i in 0..3 {
                        Item(format!("r{i}"), 40, 20);
                    }
                },
            );
        });
    }));
    assert_eq!(at(&rule, "r0", "akış"), (60.0, 0.0));
    assert_eq!(at(&rule, "r1", "akış"), (20.0, 0.0));
    assert_eq!(at(&rule, "r2", "akış"), (60.0, 20.0));
}

fn flow_column_wraps_into_columns() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        FlowColumn(
            modifier = Modifier.height(100.dp()).test_tag("sütun"),
            vertical_arrangement = Arrangement::spaced_by(10.dp()),
            horizontal_arrangement = Arrangement::spaced_by(4.dp()),
            content = || {
                for i in 0..3 {
                    Item(format!("s{i}"), 40, 40);
                }
                Box(Modifier.size_wh(10.dp(), 20.dp()).fill_max_column_width(fraction = 0.5).test_tag("yarım"));
            },
        );
    }));
    assert_eq!(at(&rule, "s1", "sütun"), (0.0, 50.0));
    assert_eq!(at(&rule, "s2", "sütun"), (44.0, 0.0));
    // (in the second column, half its 40 wide; a fill-max child is measured at that width)
    assert_eq!(at(&rule, "yarım", "sütun"), (44.0, 50.0));
    assert_eq!(bounds(&rule, "yarım")[2], 20.0);
    assert_eq!(bounds(&rule, "sütun")[2..], [84.0, 100.0]);
}

fn flow_row_intrinsics_as_compose() {
    // Its max intrinsic width is its widest line's; its min the narrowest that keeps every item showing (a binary
    // search: one to a line here); its intrinsic height the lines at a width.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(
                Modifier.width_intrinsic(IntrinsicSize::Max).test_tag("en geniş"),
                content = || {
                    FlowRow(
                        content = || {
                            for _ in 0..3 {
                                Box(Modifier.size_wh(40.dp(), 20.dp()));
                            }
                        },
                    );
                },
            );
            Box(
                Modifier.width_intrinsic(IntrinsicSize::Min).test_tag("en dar"),
                content = || {
                    FlowRow(
                        content = || {
                            for _ in 0..3 {
                                Box(Modifier.size_wh(40.dp(), 20.dp()));
                            }
                        },
                    );
                },
            );
            Box(
                Modifier.width(100.dp()).height_intrinsic(IntrinsicSize::Min).test_tag("yükseklik"),
                content = || {
                    FlowRow(
                        content = || {
                            for _ in 0..3 {
                                Box(Modifier.size_wh(40.dp(), 20.dp()));
                            }
                        },
                    );
                },
            );
        });
    }));
    assert_eq!(bounds(&rule, "en geniş")[2], 120.0);
    assert_eq!(bounds(&rule, "en dar")[2..], [40.0, 60.0]);
    assert_eq!(bounds(&rule, "yükseklik")[3], 40.0);
}

// ---- Contextual flow layouts -----------------------------------------------------------------------------------

fn contextual_flow_row_composes_items_knowing_their_line() {
    // Two 40s to a 100 wide row, two rows: the fifth is composed to find it doesn't fit, the rest never are; each
    // knows its line and the room left as it was composed (Compose's numbers: the third is told line 0, being
    // composed before it is found to wrap).
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        ContextualFlowRow(
            item_count = 10,
            modifier = Modifier.width(100.dp()).test_tag("bağlam"),
            max_lines = 2,
            content = move |scope: &ContextualFlowRowScope, index: i32| {
                s.borrow_mut().push((index, scope.line_index(), scope.index_in_line(), scope.max_width_in_line()));
                Item(format!("ö{index}"), 40, 20);
            },
        );
    }));
    // (the first measure's)
    let seen = seen.borrow()[..5].to_vec();
    assert_eq!(
        seen,
        vec![(0, 0, 0, 100.dp()), (1, 0, 1, 60.dp()), (2, 0, 2, 20.dp()), (3, 1, 1, 60.dp()), (4, 1, 2, 20.dp()),]
    );
    assert_eq!(at(&rule, "ö2", "bağlam"), (0.0, 20.0));
    assert!(displayed(&rule, "ö3") && !displayed(&rule, "ö4"));
    rule.on_node_with_tag("ö5").assert_does_not_exist();
}

fn contextual_overflow_knows_the_shown_count_as_it_composes() {
    // The indicator is subcomposed for the count that shows: 10 items, one 40 fits with the 30 wide indicator.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let overflow = ContextualFlowRowOverflow::expand_indicator(|scope: &ContextualFlowRowOverflowScope| {
            let rest = scope.total_item_count() - scope.shown_item_count();
            Item(format!("+{rest}"), 30, 20);
        });
        ContextualFlowRow(
            item_count = 10,
            modifier = Modifier.width(100.dp()).test_tag("bağlam"),
            max_lines = 1,
            overflow = overflow,
            content = |_: &ContextualFlowRowScope, index: i32| Item(format!("c{index}"), 40, 20),
        );
    }));
    assert!(displayed(&rule, "+9"));
    assert_eq!(at(&rule, "+9", "bağlam"), (40.0, 0.0));
    assert!(displayed(&rule, "c0") && !displayed(&rule, "c1"));
}

fn contextual_flow_column_tells_its_column() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        ContextualFlowColumn(
            item_count = 3,
            modifier = Modifier.height(50.dp()).test_tag("sütun"),
            content = move |scope: &ContextualFlowColumnScope, index: i32| {
                s.borrow_mut().push((index, scope.line_index(), scope.max_height_in_line()));
                Item(format!("d{index}"), 30, 20);
            },
        );
    }));
    assert_eq!(seen.borrow()[..2], [(0, 0, 50.dp()), (1, 0, 30.dp())]);
    assert_eq!(at(&rule, "d2", "sütun"), (30.0, 0.0));
}

tests![
    flow_row_wraps_with_spacing_and_a_most_per_row,
    flow_row_takes_compose_arguments,
    max_lines_clip_what_is_past_them,
    visible_overflow_places_every_line,
    an_expand_indicator_takes_the_place_of_what_doesnt_fit,
    shown_item_count_panics_before_the_layout_measured,
    expand_or_collapse_indicators_switch_with_the_lines,
    a_line_shares_weights_and_fills_its_height,
    items_align_in_their_row_unless_they_say_otherwise,
    flow_row_starts_from_the_right_in_rtl,
    flow_column_wraps_into_columns,
    flow_row_intrinsics_as_compose,
    contextual_flow_row_composes_items_knowing_their_line,
    contextual_overflow_knows_the_shown_count_as_it_composes,
    contextual_flow_column_tells_its_column,
];

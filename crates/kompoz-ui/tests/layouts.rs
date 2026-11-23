//! The C++ layout-core tests that need foundation's layouts (the others are in kompoz-foundation's tests): numbers
//! in px, as the C++'s dp at density 1.

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::semantics::SemanticsProperties;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

fn near(a: Offset, b: Offset) -> bool {
    (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01
}

fn layout_coordinates_tell_their_parent_and_map_between_nodes() {
    // C++'s layout_coordinates_tell_their_parent_and_map_between_nodes, with Compose's parent coordinates: a
    // layout node's parent layout coordinates are its parent's inner coordinator (past the column's padding), and
    // the coordinates wrapping a modifier's are the next layout modifier out.
    let inner = Rc::new(RefCell::new(None::<LayoutCoordinates>));
    let other = Rc::new(RefCell::new(None::<LayoutCoordinates>));
    let (i, o) = (inner.clone(), other.clone());
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (i, o) = (i.clone(), o.clone());
        Column(modifier = Modifier.padding(10.dp()), || {
            Box(modifier = Modifier.size_wh(100.dp(), 50.dp()));
            Box(modifier = Modifier
                .padding_sides(start = 20.dp(), top = 5.dp())
                .size(40.dp())
                .on_globally_positioned(move |c| *i.borrow_mut() = Some(c.clone())));
            Box(modifier = Modifier.size(30.dp()).on_globally_positioned(move |c| *o.borrow_mut() = Some(c.clone())));
        });
    }));
    let inner = inner.borrow().clone().unwrap();
    let other = other.borrow().clone().unwrap();
    // inner: past its padding, at (30, 65); in the column's content (from 10, 10) at (20, 55).
    assert_eq!(inner.position_in_root(), Offset::new(30.0, 65.0));
    assert_eq!(inner.bounds_in_parent(), rect(20.0, 55.0, 40.0, 40.0));
    // Its parent: the column's content, 100 wide at (10, 10); its parent the root's, which has none.
    let parent = inner.parent_layout_coordinates().unwrap();
    assert_eq!(parent.position_in_root(), Offset::new(10.0, 10.0));
    assert_eq!(parent.size().width, 100);
    let root = parent.parent_layout_coordinates().unwrap();
    assert!(root.parent_layout_coordinates().is_none());
    // What wraps it: the size modifier's coordinates.
    assert_eq!(inner.parent_coordinates().unwrap().size(), IntSize::new(40, 40));
    // From other (at 10, 105): a point of inner's, and inner's bounds.
    assert!(near(other.local_position_of(&inner, Offset::new(5.0, 5.0)), Offset::new(25.0, -35.0)));
    assert_eq!(other.local_bounding_box_of(&inner, true), rect(20.0, -40.0, 40.0, 40.0));
}

fn layout_measures_again_only_what_changed() {
    // C++'s layout_measures_again_only_what_changed: `offset {}` reads the state while placing.
    let (measured_a, measured_b) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let width = mutable_state_of(50);
    let shift = mutable_state_of(0);
    let (a, b, w, s) = (measured_a.clone(), measured_b.clone(), width.clone(), shift.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (a, b, w, s) = (a.clone(), b.clone(), w.clone(), s.clone());
        Column(|| {
            let a = a.clone();
            Layout(
                measure_policy = measure_policy(move |scope, _, _| {
                    a.set(a.get() + 1);
                    scope.layout(10, 10, |_| {})
                }),
            );
            let w = w.clone();
            scoped(move || {
                Box(modifier = Modifier.width((w.get() as f32).dp()).height(10.dp()).test_tag("b"));
            });
            let s = s.clone();
            let moved = Modifier.offset_px(move |_| IntOffset::new(s.get(), 0));
            let b = b.clone();
            Box(modifier = moved, || {
                let b = b.clone();
                Layout(
                    modifier = Modifier.test_tag("içteki"),
                    measure_policy = measure_policy(move |scope, _, _| {
                        b.set(b.get() + 1);
                        scope.layout(10, 10, |_| {})
                    }),
                );
            });
        });
    }));
    // (the layout check measures everything once more)
    rule.set_layout_check(false);
    let (ma, mb) = (measured_a.get(), measured_b.get());
    width.set(80);
    rule.wait_for_idle();
    assert_eq!(rule.on_node_with_tag("b").get_unclipped_bounds_in_root().width(), 80.0);
    assert_eq!((measured_a.get(), measured_b.get()), (ma, mb));
    // Read as it lays out: that node placed again, not what it holds measured.
    shift.set(5);
    rule.wait_for_idle();
    assert_eq!(rule.on_node_with_tag("içteki").get_unclipped_bounds_in_root().left, 5.0);
    assert_eq!((measured_a.get(), measured_b.get()), (ma, mb));
}

#[kompozable]
fn Items(n: MutableState<i32>) {
    for i in 0..n.get() {
        Text(format!("item{i}"));
    }
}

fn child_texts(node: &LayoutNode) -> Vec<String> {
    node.children()
        .iter()
        .filter_map(|c| c.semantics_configuration().and_then(|s| s.get(SemanticsProperties::Text)))
        .map(|t| t.iter().map(|t| t.text()).collect::<String>())
        .collect()
}

fn partial_recomposition_keeps_node_order() {
    // C++'s partial_recomposition_keeps_node_order.
    let n = mutable_state_of(1);
    let m = n.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(modifier = Modifier.test_tag("col"), || {
            Text("header");
            Items(m.clone());
            Text("footer");
        });
    }));
    let col = rule.on_node_with_tag("col").layout_node();
    assert_eq!(child_texts(&col), ["header", "item0", "footer"]);
    n.set(3);
    rule.wait_for_idle();
    assert_eq!(child_texts(&col), ["header", "item0", "item1", "item2", "footer"]);
    n.set(0);
    rule.wait_for_idle();
    assert_eq!(child_texts(&col), ["header", "footer"]);
}

fn alignment_lines_pass_up_and_set_padding() {
    // C++'s alignment_lines_pass_up_and_set_padding: its column, box and custom layout parts (`paddingFromBaseline`
    // is the layout modifiers package's).
    let seen = Rc::new(Cell::new(0));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Column(|| {
            Column(modifier = Modifier.test_tag("sütun").padding_sides(top = 10.dp()), || {
                Text("ilk", font_size = 16.sp());
                Text("ikinci\nüçüncü", font_size = 16.sp());
            });
            Box(modifier = Modifier.test_tag("boş").size(20.dp()));
            let s = s.clone();
            Layout(
                content = || {
                    Text("ölçülen", font_size = 32.sp());
                    Text("yerleşen", font_size = 16.sp());
                },
                modifier = Modifier.test_tag("kendi"),
                measure_policy = measure_policy(move |scope, ms, c| {
                    let loose = Constraints::new(0, c.max_width, 0, c.max_height);
                    let _measured_only = ms[0].measure(loose);
                    let placed = ms[1].measure(loose);
                    s.set(placed.get(FirstBaseline));
                    scope.layout(100, 60, move |p| p.place(&placed, 0, 30))
                }),
            );
        });
    }));
    // 16 px text: 20 a line, its baseline 13 down. The topmost first baseline and the bottommost last one (the
    // second text's second line), the padding included; where the texts are drawn.
    let column = rule.on_node_with_tag("sütun").layout_node().coordinates();
    assert_eq!(column.get(FirstBaseline), 10 + 13);
    assert_eq!(column.get(LastBaseline), 10 + 20 + 20 + 13);
    let drawn = rule.draw();
    let top = rule.on_node_with_tag("sütun").get_unclipped_bounds_in_root().top;
    let baseline = |t: &str| drawn.baselines[drawn.texts.iter().position(|x| x == t).expect("drawn")].y - top;
    assert_eq!(baseline("ilk").round() as i32, column.get(FirstBaseline));
    assert_eq!(baseline("üçüncü").round() as i32, column.get(LastBaseline));
    assert!(baseline("üçüncü") > baseline("ikinci"));
    assert_eq!(rule.on_node_with_tag("boş").layout_node().coordinates().get(FirstBaseline), AlignmentLine::Unspecified);
    // A layout of its own: what it measured but didn't place passes nothing on.
    let own = rule.on_node_with_tag("kendi").layout_node().coordinates();
    assert_eq!(own.get(FirstBaseline), 30 + 13);
    assert_eq!(seen.get(), 13);
}

fn a_size_probe_measures_afresh() {
    // C++'s a_size_probe_measures_afresh: Compose measures a child once, so the narrowest it can be is its min
    // intrinsic width (a word a line), asked after measuring it for real. Compose's min intrinsic width keeps the
    // word's trailing space ("bir " is 32 px; C++'s 24 left it out).
    let full = Rc::new(Cell::new(0));
    let narrowest = Rc::new(Cell::new(0));
    let (f, n) = (full.clone(), narrowest.clone());
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (f, n) = (f.clone(), n.clone());
        Layout(
            content = || Text("bir iki üç", font_size = 16.sp()),
            measure_policy = measure_policy(move |scope, ms, _| {
                let placeable = ms[0].measure(Constraints::default());
                f.set(placeable.width());
                n.set(ms[0].min_intrinsic_width(Constraints::Infinity));
                scope.layout(placeable.width(), 20, move |p| p.place(&placeable, 0, 0))
            }),
        );
    }));
    assert!(full.get() > 0 && narrowest.get() > 0 && narrowest.get() < full.get() / 2);
    assert_eq!((full.get(), narrowest.get()), (80, 32));
}

fn recomposing_layouts_with_equal_arguments_measures_nothing_again() {
    // Box, Row and Column keep their measure policies: composed again alike, nothing is measured again.
    let measured = Rc::new(Cell::new(0));
    let tick = mutable_state_of(0);
    let (m, t) = (measured.clone(), tick.clone());
    let counted = Modifier.layout(move |scope, me, c| {
        m.set(m.get() + 1);
        let p = me.measure(c);
        scope.layout(p.width(), p.height(), move |pl| pl.place(&p, 0, 0))
    });
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let _ = t.get();
        let counted = counted.clone();
        Column(modifier = counted, || {
            Row(|| Box(modifier = Modifier.size(10.dp())));
            Box(content_alignment = Alignment::Center, || Text("x"));
        });
    }));
    rule.set_layout_check(false);
    let before = measured.get();
    tick.set(1);
    rule.wait_for_idle();
    assert_eq!(measured.get(), before);
}

fn a_closure_policy_capturing_the_same_values_measures_nothing_again() {
    // Compose's strong skipping remembers a measure policy lambda by what it captures: composed again with the
    // same values, the layout keeps its policy and isn't measured again; with others, it is.
    let measured = Rc::new(Cell::new(0));
    let (tick, width) = (mutable_state_of(0), mutable_state_of(10));
    let (m, t, w) = (measured.clone(), tick.clone(), width.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let _ = t.get();
        let (m, size) = (m.clone(), w.get());
        Layout(
            modifier = Modifier.test_tag("leaf"),
            measure_policy = measure_policy(move |scope, _, _| {
                m.set(m.get() + 1);
                scope.layout(size, size, |_| {})
            }),
        );
    }));
    rule.set_layout_check(false);
    let before = measured.get();
    tick.set(1);
    rule.wait_for_idle();
    tick.set(2);
    rule.wait_for_idle();
    assert_eq!(measured.get(), before);
    width.set(20);
    rule.wait_for_idle();
    assert_eq!(measured.get(), before + 1);
    assert_eq!(rule.on_node_with_tag("leaf").get_unclipped_bounds_in_root().width(), 20.0);
}

/// A test tag before a layout modifier: its semantics bounds are that modifier's coordinator's (Compose's: a
/// semantics node is on the coordinator of the next layout modifier), so when the modifier lays out to nothing
/// without measuring what it wraps, the bounds are 0×0 at once, not what it wrapped last measured.
fn a_tag_before_a_layout_modifier_that_lays_out_to_nothing_has_no_size() {
    let shown = mutable_state_of(true);
    let s = shown.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Row(move || {
                let s = s.clone();
                Box(modifier = Modifier
                    .test_tag("etiket")
                    .layout(move |scope, measurable, constraints| {
                        if s.get() {
                            let placeable = measurable.measure(constraints);
                            scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0))
                        } else {
                            scope.layout(0, 0, |_| {})
                        }
                    })
                    .size(56.dp()));
                Box(modifier = Modifier.size(10.dp()).test_tag("yan"));
            });
        }),
        IntSize::new(200, 200),
    );
    assert_eq!(rule.on_node_with_tag("etiket").get_unclipped_bounds_in_root(), rect(0.0, 0.0, 56.0, 56.0));
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(rule.on_node_with_tag("yan").get_unclipped_bounds_in_root().left, 0.0);
    let node = rule.on_node_with_tag("etiket");
    assert_eq!(node.get_unclipped_bounds_in_root().size(), Size::Zero);
    assert_eq!(node.fetch_semantics_node().size(), IntSize::Zero);
    shown.set(true);
    rule.wait_for_idle();
    assert_eq!(rule.on_node_with_tag("etiket").get_unclipped_bounds_in_root(), rect(0.0, 0.0, 56.0, 56.0));
    // A node merging what is below it (a clickable after the modifier: a FAB under animateFloatingActionButton)
    // has its bounds from the merging semantics node's coordinator (Compose's findSemanticsModifierNodeToGetBounds
    // prefers it), inside the modifier: as it was last measured, 56 dp, though the layout takes no room — what
    // Compose reports too.
    let shown = mutable_state_of(true);
    let s = shown.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Row(move || {
                let s = s.clone();
                Box(modifier = Modifier
                    .test_tag("birleşen")
                    .layout(move |scope, measurable, constraints| {
                        if s.get() {
                            let placeable = measurable.measure(constraints);
                            scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0))
                        } else {
                            scope.layout(0, 0, |_| {})
                        }
                    })
                    .clickable(|| {})
                    .size(56.dp()));
                Box(modifier = Modifier.size(10.dp()).test_tag("yan"));
            });
        }),
        IntSize::new(200, 200),
    );
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(rule.on_node_with_tag("yan").get_unclipped_bounds_in_root().left, 0.0);
    assert_eq!(rule.on_node_with_tag("birleşen").get_unclipped_bounds_in_root().size(), Size::new(56.0, 56.0));
}

fn a_placement_reading_its_coordinates_is_placed_again_when_an_ancestor_moves() {
    // Compose's childrenAccessingCoordinatesDuringPlacement: a node reading its coordinates while placing is counted
    // by every ancestor, so a move however far above it (here an `offset {}` two nodes up) places it again — this
    // one pins what it holds to the root's left edge.
    let shift = mutable_state_of(0);
    let s = shift.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Box(modifier = Modifier.offset_px(move |_| IntOffset::new(s.get(), 0)), || {
            Box(|| {
                let pinned = Modifier.layout(|scope, measurable, constraints| {
                    let placeable = measurable.measure(constraints);
                    scope.layout(placeable.width(), placeable.height(), move |p| {
                        let at = p.coordinates().map_or(Offset::Zero, |c| c.position_in_root());
                        p.place(&placeable, -at.x as i32, 0)
                    })
                });
                Box(modifier = pinned.size(10.dp()).test_tag("sabit"));
            });
        });
    }));
    let left = || rule.on_node_with_tag("sabit").get_unclipped_bounds_in_root().left;
    assert_eq!(left(), 0.0);
    shift.set(30);
    rule.wait_for_idle();
    assert_eq!(left(), 0.0);
}

tests!(
    a_placement_reading_its_coordinates_is_placed_again_when_an_ancestor_moves,
    a_tag_before_a_layout_modifier_that_lays_out_to_nothing_has_no_size,
    layout_coordinates_tell_their_parent_and_map_between_nodes,
    layout_measures_again_only_what_changed,
    partial_recomposition_keeps_node_order,
    alignment_lines_pass_up_and_set_padding,
    a_size_probe_measures_afresh,
    recomposing_layouts_with_equal_arguments_measures_nothing_again,
    a_closure_policy_capturing_the_same_values_measures_nothing_again,
);

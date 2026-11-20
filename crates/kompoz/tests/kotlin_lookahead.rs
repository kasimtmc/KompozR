//! Lookahead in Kotlin's syntax, as a Compose user writes it: the C++ Kompoz's LayoutForms approach part
//! (kompozable_tests.cpp 3281-3310: a `DeferredTargetAnimation` of sizes and `Modifier.approachLayout`, Compose's
//! spelling of them: the converter given, the spec at `updateTarget`, `approachMeasure` a `MeasureScope`-style block);
//! a `LookaheadScope { }` whose layouts read `isLookingAhead`, take their `lookaheadSize`, and map their coordinates
//! with `toLookaheadCoordinates()` / `localLookaheadPositionOf`; and in a `.kt` file (kompoz_file!) a scroller placing
//! its content on its own frame of reference (`withMotionFrameOfReferencePlacement`) and an approach placed by its
//! placement approach. The same behaviour crates/kompoz-ui/tests/lookahead.rs and crates/kompoz-animation/tests/
//! lookahead.rs check in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::cell::RefCell;
use std::rc::Rc;

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

// ---- LayoutForms' approach part (kompozable_tests.cpp 3281-3310) -------------------------------------------------

kompoz! {
    // (C++'s `DeferredTargetAnimation<Size>(glide)` and `approachMeasure = { s, measure, _, _ -> … }`: Compose takes the
    // converter, the spec at `updateTarget`, and an approach measure block as `MeasureScope.measure`)
    @Composable
    fun GlidingTag() {
        val glide = spring<Size>(Spring.DampingRatioLowBouncy, Spring.StiffnessLow)
        val tagSize = remember { DeferredTargetAnimation(Size.VectorConverter) }
        val scope = rememberCoroutineScope()
        Column {
            Box(modifier = Modifier
                    .approachLayout(
                        isMeasurementApproachInProgress = { target ->
                            tagSize.updateTarget(target.toSize(), scope, glide)
                            !tagSize.isIdle
                        },
                        approachMeasure = { measurable, _ ->
                            val now = tagSize.updateTarget(lookaheadSize.toSize(), scope, glide)
                            val placeable = measurable.measure(Constraints.fixed(now.width.roundToInt(), now.height.roundToInt()))
                            layout(placeable.width, placeable.height) { placeable.place(0, 0) }
                        })
                    .testTag("etiket")) { Text("ab".repeat(2)) }
        }
    }
}

fn layout_forms_approach_as_compose_writes_it() {
    let rule = KompozTestRule::new(GlidingTag);
    rule.wait_for_idle();
    rule.on_node_with_text("abab").assert_exists();
    // (its size is its text's, reached as it is: the first target is where it starts)
    let tag = bounds(&rule, "etiket");
    assert!(tag.width() > 0.0 && tag.height() > 0.0);
}

// ---- A LookaheadScope --------------------------------------------------------------------------------------------

kompoz! {
    @Composable
    fun LookingAhead(wide: MutableState<Boolean>, approaching: MutableState<Boolean>, seen: (Offset, Offset) -> Unit) {
        LookaheadScope {
            Column {
                Box(modifier = Modifier
                        .approachLayout(
                            isMeasurementApproachInProgress = { approaching.value },
                            approachMeasure = { measurable, _ ->
                                val placeable = measurable.measure(Constraints.fixed(lookaheadSize.width / 2, lookaheadSize.height))
                                layout(placeable.width, placeable.height) { placeable.place(0, 0) }
                            })
                        .testTag("yaklaşan")) {
                    Box(modifier = Modifier.size(if (wide.value) 200.dp else 100.dp, 20.dp))
                }
                Box(modifier = Modifier
                        .layout { measurable, constraints ->
                            val placeable = measurable.measure(constraints)
                            layout(placeable.width, placeable.height) {
                                placeable.place(if (isLookingAhead) 60 else 10, 0)
                            }
                        }
                        .size(20.dp)
                        .onPlaced { coordinates ->
                            val lookahead = coordinates.toLookaheadCoordinates()
                            val parent = coordinates.parentLayoutCoordinates!!
                            seen(parent.localLookaheadPositionOf(coordinates), lookahead.positionInRoot())
                        }
                        .testTag("öngören"))
            }
        }
    }
}

fn a_lookahead_scope_as_compose_writes_it() {
    let wide = mutable_state_of(false);
    let approaching = mutable_state_of(true);
    let seen = Rc::new(RefCell::new(Vec::<(Offset, Offset)>::new()));
    let (w, a, s) = (wide.clone(), approaching.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        LookingAhead(w.clone(), a.clone(), move |at, root| s.borrow_mut().push((at, root)));
    }));
    assert_eq!(bounds(&rule, "yaklaşan").width(), 50.0);
    approaching.set(false);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "yaklaşan").width(), 100.0);
    wide.set(true);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "yaklaşan").width(), 200.0);
    // Placed 10 right in the main pass, 60 in lookahead (under the first, 20 down): its lookahead position in its
    // parent, and in the root.
    assert_eq!(bounds(&rule, "öngören").left, 10.0);
    let (at, root) = *seen.borrow().last().unwrap();
    assert_eq!((at, root), (Offset::new(60.0, 20.0), Offset::new(60.0, 20.0)));
}

// ---- kompoz_file! ------------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/lookahead.kt");

fn a_motion_frame_of_reference_in_a_kt_file() {
    let scrolled = mutable_state_of(30);
    let seen = Rc::new(RefCell::new(Vec::<(Offset, Offset, bool)>::new()));
    let (s, o) = (scrolled.clone(), seen.clone());
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let o = o.clone();
        Scroller(s.clone(), move |with, without, introduces| o.borrow_mut().push((with, without, introduces)));
    }));
    let (with, without, introduces) = *seen.borrow().last().unwrap();
    assert!(introduces);
    assert_eq!((with, without), (Offset::new(0.0, 30.0), Offset::Zero));
}

fn a_placement_approach_in_a_kt_file() {
    let moving = mutable_state_of(true);
    let m = moving.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || Mover(m.clone())));
    assert_eq!(bounds(&rule, "taşınan").left, 30.0);
    moving.set(false);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "taşınan").left, 0.0);
}

tests!(
    layout_forms_approach_as_compose_writes_it,
    a_lookahead_scope_as_compose_writes_it,
    a_motion_frame_of_reference_in_a_kt_file,
    a_placement_approach_in_a_kt_file,
);

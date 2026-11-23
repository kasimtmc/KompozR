//! Lookahead animations of a layout's size and bounds in Kotlin's syntax, as a Compose user writes them:
//! `Modifier.animateContentSize()` (both overloads: a spec and a finished listener as a trailing lambda, an alignment
//! between), `LookaheadScope { Box(Modifier.animateBounds(this@LookaheadScope)) }` with a `BoundsTransform { initial,
//! target -> spring() }` (a fun interface's SAM constructor), `LookaheadAnimationVisualDebugging { }` and
//! `CustomizedLookaheadAnimationVisualDebugging(color) { }`; and in a `.kt` file (kompoz_file!) the same growing and
//! gliding. The same behaviour crates/kompoz-animation/tests/bounds.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::cell::RefCell;
use std::rc::Rc;

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

// ---- animateContentSize -------------------------------------------------------------------------------------------

kompoz! {
    @Composable
    fun Sized(big: Boolean, finished: (IntSize, IntSize) -> Unit) {
        Column {
            // (the overload without an alignment, its listener a trailing lambda)
            Box(modifier = Modifier.testTag("dinleyen").animateContentSize(tween(160, easing = LinearEasing)) { initial, target -> finished(initial, target) }) {
                Box(modifier = Modifier.size(if (big) 100.dp else 50.dp))
            }
            // (the one with an alignment: the content at the bottom end on the way)
            Box(modifier = Modifier.testTag("hizalı").animateContentSize(tween(160, easing = LinearEasing), Alignment.BottomEnd)) {
                Box(modifier = Modifier.size(if (big) 100.dp else 50.dp).testTag("hizalı-iç"))
            }
        }
    }
}

fn animate_content_size_as_compose_writes_it() {
    let big = mutable_state_of(false);
    let finished = Rc::new(RefCell::new(Vec::<(IntSize, IntSize)>::new()));
    let (b, f) = (big.clone(), finished.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let f = f.clone();
        Sized(b.get(), move |from, to| f.borrow_mut().push((from, to)));
    }));
    // (the tags before the modifier: Compose's semantics bounds are those of the layout the tag wraps)
    assert_eq!(bounds(&rule, "dinleyen").width(), 50.0);
    // (the clock by hand: the checks on the way see the animation running)
    rule.main_clock().set_auto_advance(false);
    big.set(true);
    rule.advance_time_by(80);
    let mid = bounds(&rule, "dinleyen").width();
    assert!(mid > 55.0 && mid < 95.0, "{mid}");
    let (outer, inner) = (bounds(&rule, "hizalı"), bounds(&rule, "hizalı-iç"));
    assert!((inner.right - outer.right).abs() < 0.5 && (inner.bottom - outer.bottom).abs() < 0.5);
    common::settle(&rule);
    assert_eq!(bounds(&rule, "dinleyen").width(), 100.0);
    assert_eq!(finished.borrow().last().copied(), Some((IntSize::new(50, 50), IntSize::new(100, 100))));
}

// ---- animateBounds ------------------------------------------------------------------------------------------------

kompoz! {
    @Composable
    fun Glide(right: Boolean, debugging: Boolean) {
        LookaheadAnimationVisualDebugging(isEnabled = debugging, inactiveElementColor = Color.Green) {
            CustomizedLookaheadAnimationVisualDebugging(Color.Blue) {
                LookaheadScope {
                    Box(modifier = Modifier.size(300.dp, 200.dp), contentAlignment = if (right) Alignment.BottomEnd else Alignment.TopStart) {
                        Box(modifier = Modifier
                            .animateBounds(this@LookaheadScope, boundsTransform = BoundsTransform { initial, target -> spring(stiffness = Spring.StiffnessLow) })
                            .size(40.dp)
                            .testTag("kutu"))
                    }
                }
            }
        }
    }
}

fn animate_bounds_as_compose_writes_it() {
    let right = mutable_state_of(false);
    let debugging = mutable_state_of(true);
    let (r, d) = (right.clone(), debugging.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || Glide(r.get(), d.get())));
    assert_eq!(bounds(&rule, "kutu").top_left(), Offset::Zero);
    // (at rest: outlined in the inactive color)
    assert!(rule.draw().stroke_colors.contains(&Color::Green));
    // (the clock by hand: the checks on the way see the animation running)
    rule.main_clock().set_auto_advance(false);
    right.set(true);
    rule.advance_time_by(160);
    let mid = bounds(&rule, "kutu").top_left();
    assert!(mid.x > 0.0 && mid.x < 260.0 && mid.y > 0.0 && mid.y < 160.0, "{mid:?}");
    // (on its way: outlined in its own color, here and at its target)
    assert_eq!(rule.draw().stroke_colors.iter().filter(|c| **c == Color::Blue).count(), 2);
    common::settle(&rule);
    assert_eq!(bounds(&rule, "kutu").top_left(), Offset::new(260.0, 160.0));
    debugging.set(false);
    common::settle(&rule);
    assert!(!rule.draw().stroke_colors.contains(&Color::Green));
}

// ---- kompoz_file! -------------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/bounds.kt");

fn growing_and_gliding_in_a_kt_file() {
    let big = mutable_state_of(false);
    let right = mutable_state_of(false);
    let (b, r) = (big.clone(), right.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (b, r) = (b.clone(), r.clone());
        // (the gliding one first: the growing one below it doesn't move it — the scope's coordinates are the
        // column's, and what moves in them animates)
        Column(move || {
            Gliding(r.clone());
            Growing(b.clone());
        });
    }));
    assert_eq!(bounds(&rule, "büyüyen").width(), 100.0);
    assert_eq!(bounds(&rule, "kayan").top_left(), Offset::Zero);
    // (the clock by hand: the checks on the way see the animation running)
    rule.main_clock().set_auto_advance(false);
    big.set(true);
    right.set(true);
    rule.advance_time_by(208);
    let grown = bounds(&rule, "büyüyen").width();
    assert!(grown > 100.0 && grown < 200.0, "{grown}");
    let at = bounds(&rule, "kayan");
    assert!(at.left > 100.0 && at.left < 160.0, "{at:?}");
    common::settle(&rule);
    assert_eq!(bounds(&rule, "büyüyen").width(), 200.0);
    assert_eq!(bounds(&rule, "kayan").top_left(), Offset::new(260.0, 160.0));
}

tests!(
    animate_content_size_as_compose_writes_it,
    animate_bounds_as_compose_writes_it,
    growing_and_gliding_in_a_kt_file
);

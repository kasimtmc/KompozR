//! Scrolling in Kotlin's syntax, as a Compose user writes it: a Column in `verticalScroll(rememberScrollState())`,
//! a scrollable of a `rememberScrollableState { delta -> … }`, an animated scroll from a LaunchedEffect, through
//! `kompoz!`; and in a `.kt` file (kompoz_file!) a scrollable with its overscroll effect, a draggable and a
//! horizontal scroll animated to a value. The same behaviour crates/kompoz-foundation/tests/scroll.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, Text};
use kompoz::ui::input::PointerType;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A node's bounds in the root, unclipped (px; the rule's density is 1).
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A finger from `from` to `to`, a move a frame, held `hold` frames, let go.
fn drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32, hold: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / frames as f32), -1, 0);
    }
    for _ in 0..hold {
        rule.advance_frame();
    }
    h.pointer_up(to, -1, 0);
    rule.wait_for_idle();
}

kompoz! {
    @Composable
    fun Feed() {
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).testTag("feed")) {
            for (i in 0 until 40) {
                Text("item $i", modifier = Modifier.height(50.dp).testTag("item $i"))
            }
        }
    }

    @Composable
    fun Wheel(seen: (Float) -> Unit) {
        var turned by remember { mutableStateOf(0f) }
        Box(
            Modifier.size(200.dp).testTag("wheel").scrollable(
                rememberScrollableState { delta ->
                    turned += delta
                    seen(turned)
                    delta
                },
                Orientation.Vertical,
            )
        )
    }

    @Composable
    fun JumpTo(value: Int, done: () -> Unit) {
        val state = rememberScrollState()
        LaunchedEffect(value) {
            state.animateScrollTo(value)
            done()
        }
        Column(Modifier.size(100.dp).verticalScroll(state).testTag("jump")) {
            Box(Modifier.size(100.dp, 1000.dp).testTag("tall"))
        }
    }
}

fn a_column_scrolls_with_the_finger() {
    let rule = KompozTestRule::new(Feed);
    rule.on_node_with_text("item 30").assert_is_not_displayed();
    drag(&rule, Offset::new(200.0, 700.0), Offset::new(200.0, 300.0), 10, 10);
    let top = bounds(&rule, "item 0").top;
    assert!(top < -350.0, "{top}"); // up with the finger, less the touch slop
    rule.on_node_with_tag("feed").perform_scroll_to_node(kompoz::test::has_text("item 30", false, false));
    rule.on_node_with_text("item 30").assert_is_displayed();
}

fn a_scrollable_state_hears_the_finger() {
    let seen = Rc::new(Cell::new(0.0f32));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Wheel(move |v| s.set(v));
    }));
    drag(&rule, Offset::new(100.0, 150.0), Offset::new(100.0, 50.0), 10, 10);
    let slop = kompoz::ui::platform::ViewConfiguration::default().touch_slop;
    assert_eq!(seen.get(), -(100.0 - slop)); // the finger's way, less the slop
}

fn an_animated_scroll_from_a_launched_effect() {
    let done = Rc::new(Cell::new(false));
    let d = done.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let d = d.clone();
        JumpTo(300, move || d.set(true));
    }));
    // (the rule composes it and runs until idle: the animation done)
    assert!(done.get() && bounds(&rule, "tall").top == -300.0, "{:?}", bounds(&rule, "tall"));
}

kompoz_file!("tests/kt/scroll.kt");

fn a_scrollable_with_its_overscroll_effect_from_a_kt_file() {
    let seen = Rc::new(Cell::new(0.0f32));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Pull(move |v| s.set(v));
    }));
    // Pulled down at its start: nothing scrolls, the overscroll stretches it.
    rule.host().pointer_down(Offset::new(100.0, 20.0), -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        rule.host().pointer_move(Offset::new(100.0, 20.0 + 15.0 * i as f32), -1, 0);
    }
    let canvas = rule.draw();
    assert!(
        seen.get() == 0.0 && canvas.layer_stretches.iter().any(|s| s.is_some_and(|s| s.stretch_direction().y > 0.0))
    );
    rule.host().pointer_up(Offset::new(100.0, 170.0), -1, 0);
    rule.wait_for_idle();
    // Up: it scrolls on (reversed: the finger up, forward).
    drag(&rule, Offset::new(100.0, 180.0), Offset::new(100.0, 80.0), 10, 10);
    assert!(seen.get() > 80.0, "{}", seen.get());
}

fn a_draggable_from_a_kt_file() {
    let (moved, stopped) = (Rc::new(Cell::new(0.0f32)), Rc::new(RefCell::new(Vec::new())));
    let (m, st) = (moved.clone(), stopped.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (m, st) = (m.clone(), st.clone());
        Slider(move |x| m.set(x), move |v| st.borrow_mut().push(v));
    }));
    drag(&rule, Offset::new(20.0, 30.0), Offset::new(220.0, 30.0), 10, 10);
    let slop = kompoz::ui::platform::ViewConfiguration::default().touch_slop;
    assert!(moved.get() == 200.0 - slop && *stopped.borrow() == [0.0], "{} {:?}", moved.get(), stopped.borrow());
}

fn a_scroll_animated_to_a_value_from_a_kt_file() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| Strip(250)));
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "cell 5").left, 0.0);
}

tests![
    a_column_scrolls_with_the_finger,
    a_scrollable_state_hears_the_finger,
    an_animated_scroll_from_a_launched_effect,
    a_scrollable_with_its_overscroll_effect_from_a_kt_file,
    a_draggable_from_a_kt_file,
    a_scroll_animated_to_a_value_from_a_kt_file,
];

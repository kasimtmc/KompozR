//! Transitions and values animated as state in Kotlin's syntax, as a Compose user writes them: the C++ Kompoz's
//! kompozable_tests.cpp 2436-2450 (`val scale by animateFloatAsState(…)`, `animateColorAsState`,
//! `updateTransition(targetState =, label =)` and its `animateDp(label =) { }` / `animateFloat { }` by delegate,
//! `rememberInfiniteTransition(label =).animateFloat(…)` by delegate) and 2935 (`animateFloatAsState(…, animationSpec =
//! tween(durationMillis =, easing =))`); a transition spec of a segment (`false isTransitioningTo true`), an
//! `animateColor`, a `MutableTransitionState` remembered; and in a `.kt` file (kompoz_file!) a SeekableTransitionState
//! seeked and animated from a coroutine, a child transition, a deferred transition state. The same behaviour
//! crates/kompoz-animation/tests checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn width(rule: &KompozTestRule, tag: &str) -> f32 {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root().width()
}

// ---- kompozable_tests.cpp 2436-2450 and 2935 ------------------------------------------------------------------

/// (Kotlin's `enum class Panel { Closed, Open }`: enum classes come with stage 14's Kotlin syntax.)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Panel {
    Closed,
    Open,
}

kompoz! {
    @Composable
    fun AnimatedValues(open: Boolean, seen: (Float, Float, Float) -> Unit) {
        val scale by animateFloatAsState(if (open) 1.2f else 1f, spring(Spring.DampingRatioMediumBouncy), label = "scale")
        val tint by animateColorAsState(if (open) Color.White else Color.Black, tween(300))
        val transition = updateTransition(targetState = if (open) Panel.Open else Panel.Closed, label = "panel")
        val size by transition.animateDp(label = "size") { if (it == Panel.Open) 200.dp else 80.dp }
        val share by transition.animateFloat { if (it == Panel.Open) 1f else 0f }
        val pulse by rememberInfiniteTransition(label = "pulse").animateFloat(
            initialValue = 0.85f, targetValue = 1f, animationSpec = infiniteRepeatable(tween(800), RepeatMode.Reverse), label = "scale")
        val alpha by animateFloatAsState(if (open) 1f else 0.5f, animationSpec = tween(durationMillis = 10, easing = LinearEasing))
        seen(share, alpha, tint.red)
        Column {
            Text("ölçek ${(scale * 10).roundToInt()}", modifier = Modifier.graphicsLayer { scaleX = pulse; scaleY = pulse })
            Box(modifier = Modifier.size(size).background(tint).testTag("kutu"))
        }
    }
}

fn animated_values_as_compose_writes_them() {
    let open = mutable_state_of(false);
    let seen = Rc::new(RefCell::new(Vec::<(f32, f32, f32)>::new()));
    let (o, s) = (open.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        AnimatedValues(o.get(), move |share, alpha, red| s.borrow_mut().push((share, alpha, red)));
    }));
    rule.on_node_with_text("ölçek 10").assert_exists();
    assert!((width(&rule, "kutu") - 80.0).abs() < 0.5);
    assert_eq!(*seen.borrow().last().unwrap(), (0.0, 0.5, 0.0));
    open.set(true);
    rule.advance_time_by(96);
    let w = width(&rule, "kutu");
    assert!(w > 80.0 && w < 200.0, "{w}");
    rule.wait_for_idle();
    rule.on_node_with_text("ölçek 12").assert_exists();
    assert!((width(&rule, "kutu") - 200.0).abs() < 0.5);
    assert_eq!(*seen.borrow().last().unwrap(), (1.0, 1.0, 1.0));
}

// ---- A segment's spec, animateColor, a MutableTransitionState ---------------------------------------------------

kompoz! {
    @Composable
    fun SegmentSpecs(on: Boolean, step: MutableTransitionState<Int>, seen: (Dp, Color, Float) -> Unit) {
        val transition = updateTransition(on, label = "açık")
        val dp by transition.animateDp(transitionSpec = { if (false isTransitioningTo true) tween(160, easing = LinearEasing) else snap() }) {
            if (it) 100.dp else 0.dp
        }
        val color by transition.animateColor(label = "renk") { if (it) Color.White else Color.Black }
        val steps = rememberTransition(step, label = "adım")
        val x by steps.animateFloat(transitionSpec = { tween(160) }) { it * 10f }
        seen(dp, color, x)
    }
}

fn segment_specs_as_compose_writes_them() {
    let on = mutable_state_of(false);
    let step = MutableTransitionState(0);
    let seen = Rc::new(RefCell::new(Vec::<(Dp, Color, f32)>::new()));
    let (o, st, s) = (on.clone(), step.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        SegmentSpecs(o.get(), st.clone(), move |dp, color, x| s.borrow_mut().push((dp, color, x)));
    }));
    on.set(true);
    step.set_target_state(2);
    rule.advance_time_by(96);
    let (dp, _, x) = *seen.borrow().last().unwrap();
    assert!(dp.0 > 20.0 && dp.0 < 80.0 && x > 0.0 && x < 20.0, "{dp:?} {x}");
    assert!(!step.is_idle());
    rule.wait_for_idle();
    assert_eq!(*seen.borrow().last().unwrap(), (100.dp(), Color::White, 20.0));
    assert!(step.is_idle() && step.current_state() == 2);
    // Back: a snap.
    on.set(false);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    assert_eq!(seen.borrow().last().unwrap().0, 0.dp());
}

// ---- kompoz_file! ------------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/transitions.kt");

fn a_seekable_transition_in_a_kt_file() {
    let state = SeekableTransitionState(false);
    let seen = Rc::new(Cell::new(-1.0f32));
    let go = mutable_state_of(0);
    let (st, s, g) = (state.clone(), seen.clone(), go.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Seeking(st.clone(), g.get(), move |x| s.set(x));
    }));
    go.set(1); // seek half way to true
    rule.wait_for_idle();
    assert!((seen.get() - 50.0).abs() < 0.01 && state.fraction() == 0.5, "{}", seen.get());
    go.set(2); // the rest of the way
    rule.wait_for_idle();
    assert_eq!((seen.get(), state.current_state()), (100.0, true));
    go.set(3); // snapped back
    rule.wait_for_idle();
    assert_eq!((seen.get(), state.current_state()), (0.0, false));
}

fn a_child_transition_in_a_kt_file() {
    let target = mutable_state_of(0);
    let seen = Rc::new(RefCell::new(Vec::<(f32, bool)>::new()));
    let (t, s) = (target.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Child(t.get(), move |x, running| s.borrow_mut().push((x, running)));
    }));
    target.set(1);
    rule.wait_for_idle();
    assert_eq!(*seen.borrow().last().unwrap(), (0.0, false));
    target.set(2);
    rule.advance_time_by(96);
    let (x, running) = *seen.borrow().last().unwrap();
    assert!(x > 0.0 && x < 1.0 && running, "{x} {running}");
    rule.wait_for_idle();
    assert_eq!(*seen.borrow().last().unwrap(), (1.0, false));
}

fn a_deferred_transition_in_a_kt_file() {
    let state = DeferredTransitionState(false);
    let seen = Rc::new(RefCell::new(Vec::<(Option<bool>, f32)>::new()));
    let (st, s) = (state.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Deferring(st.clone(), move |pending, x| s.borrow_mut().push((pending, x)));
    }));
    state.defer(true);
    rule.wait_for_idle();
    assert_eq!(*seen.borrow().last().unwrap(), (Some(true), 0.0));
    state.animate_to(true);
    rule.wait_for_idle();
    assert_eq!(*seen.borrow().last().unwrap(), (None, 1.0));
}

tests!(
    animated_values_as_compose_writes_them,
    segment_specs_as_compose_writes_them,
    a_seekable_transition_in_a_kt_file,
    a_child_transition_in_a_kt_file,
    a_deferred_transition_in_a_kt_file,
);

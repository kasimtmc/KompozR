//! Animations that never end (Compose's InfiniteTransition): the C++ Kompoz's infinite_transition_keeps_its_scope_
//! animating (3118), infinite_transition_takes_compose_named_arguments (3131), infinite_transition_states_are_read_
//! where_they_are_needed (16031), what_is_drawn_out_of_view_asks_for_no_frames (16078), and the infinite transition
//! part of colors_animate_in_oklab_as_composes_vector_converter (29876); Compose's behaviour of new values, the
//! duration scale, and the test clock's policy.
//!
//! Adapted (stage 11 design D3/D4): Compose's test rule cancels infinite animations while its clock advances on its
//! own, so these build an empty rule, turn auto advance off, then set the content. Compose's animations tick in their
//! coroutines whether what reads them is drawn or not: the C++ Kompoz's "no frames while not drawn / out of view, idle
//! until drawn" checks are its own model's (recorded deviations); here they check that frames go on while the
//! animation is composed, and stop as it leaves the composition.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz_animation::*;
use kompoz_coroutines::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::{Modifier, MotionDurationScale};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

fn linear<T: 'static>(ms: i32) -> TweenSpec<T> {
    TweenSpec::<T>::new(ms, 0, LinearEasing)
}

/// A rule whose clock advances only as the test says (infinite animations run), composing `content`.
fn running(content: impl Content + 'static) -> KompozTestRule {
    let rule = KompozTestRule::empty();
    rule.main_clock().set_auto_advance(false);
    rule.set_content(content);
    rule
}

// ---- C++ 3118 -----------------------------------------------------------------------------------------------------

fn infinite_transition_keeps_its_scope_animating() {
    let values = Rc::new(RefCell::new(Vec::<f32>::new()));
    let v = values.clone();
    let rule = running(kompozable_lambda!(move || {
        let transition = remember_infinite_transition();
        let spec = named_args!(infinite_repeatable(
            TweenSpec::<f32>::new(500, 0, FastOutSlowInEasing),
            repeat_mode = RepeatMode::Reverse
        ));
        v.borrow_mut().push(transition.animate_float(0.0, 1.0, spec).value());
    }));
    rule.main_clock().advance_time_by(1000);
    assert!(values.borrow().len() > 30, "{}", values.borrow().len());
    assert!(rule.host().has_pending_work());
    assert!(values.borrow().iter().cloned().fold(0.0f32, f32::max) > 0.9);
}

// ---- C++ 3131 -----------------------------------------------------------------------------------------------------

fn infinite_transition_takes_compose_named_arguments() {
    // animateFloat(initialValue = …, targetValue = …, animationSpec = …, label = …): a state, its value read.
    let seen = Rc::new(Cell::new(-1.0f32));
    let tint = Rc::new(Cell::new(Color::Transparent));
    let (s, ti) = (seen.clone(), tint.clone());
    let rule = running(kompozable_lambda!(move || {
        let t = remember_infinite_transition();
        let v = t.animate_float(
            initial_value = 2.0,
            target_value = 4.0,
            animation_spec = infinite_repeatable(linear(100)),
            label = "size".to_string(),
        );
        let c = t.animate_color(
            Color::Black,
            Color::White,
            animation_spec = infinite_repeatable(TweenSpec::new(100, 0, FastOutSlowInEasing)),
        );
        ti.set(c.value());
        s.set(v.value());
    }));
    let first = seen.get();
    assert!((2.0..=4.0).contains(&first));
    rule.main_clock().advance_time_by(30);
    assert!(seen.get() != first && (2.0..=4.0).contains(&seen.get()), "{first} {}", seen.get());
    assert!(tint.get().red() > 0.0);
}

// ---- C++ 16031 ----------------------------------------------------------------------------------------------------

fn infinite_transition_states_are_read_where_they_are_needed() {
    // As Compose's State from animateFloat: read while drawing, only drawn again; while laying out, only laid out
    // again; in a composition, composed again. (C++ let a spec that ends in: Compose's takes an infinite repeat.)
    let drawn = Rc::new(RefCell::new(Vec::<f32>::new()));
    let tints = Rc::new(RefCell::new(Vec::<Color>::new()));
    let (composed, labels) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (d, ti, c, l) = (drawn.clone(), tints.clone(), composed.clone(), labels.clone());
    let rule = running(kompozable_lambda!(move || {
        c.set(c.get() + 1);
        let t = remember_infinite_transition();
        let angle = t.animate_float(0.0, 100000.0, infinite_repeatable(linear(100000))); // a unit a millisecond
        let tint = t.animate_color(Color::Black, Color::White, infinite_repeatable(linear(1000)));
        let (d, ti, l) = (d.clone(), ti.clone(), l.clone());
        Column(move || {
            let (a, d) = (angle.clone(), d.clone());
            Box(modifier = Modifier.size(40.dp()).graphics_layer_block(move |layer| {
                layer.rotation_z = a.value();
                d.borrow_mut().push(layer.rotation_z);
            }));
            let (t, ti) = (tint.clone(), ti.clone());
            Box(modifier = Modifier.size(40.dp()).draw_behind(move |_| ti.borrow_mut().push(t.value())));
            let a = angle.clone();
            Box(modifier = Modifier
                .offset_px(move |_| IntOffset::new((a.value() / 100.0) as i32, 0))
                .size(10.dp())
                .test_tag("kayan"));
            // (composed again as it changes)
            let (a, l) = (angle.clone(), l.clone());
            scoped(move || {
                l.set(l.get() + 1);
                Text(format!("{}", a.value() as i32), modifier = Modifier.test_tag("açı"));
            });
        });
    }));
    rule.draw();
    assert!(rule.host().has_pending_work()); // the next frame is asked for
    let (composed_before, labels_before) = (composed.get(), labels.get());
    let x0 = rule.on_node_with_tag("kayan").get_unclipped_bounds_in_root().left;
    rule.main_clock().advance_time_by(250);
    rule.draw();
    assert_eq!(composed.get(), composed_before); // the section never composed again...
    assert!(labels.get() > labels_before && !rule.on_node_with_text("0").exists()); // ...but the text reading it did
    let (first, last) = (drawn.borrow()[0], *drawn.borrow().last().unwrap());
    assert!(drawn.borrow().len() >= 2 && last > first + 200.0, "{first} {last}");
    assert!(tints.borrow().len() >= 2 && tints.borrow().last() != tints.borrow().first());
    assert!(rule.on_node_with_tag("kayan").get_unclipped_bounds_in_root().left > x0 + 1.0); // laid out again there
    // Read only while drawing: frames are asked for whether it is drawn or not (Compose's; C++ asked for none until
    // it was drawn — a deviation).
    let drawn_only = running(kompozable_lambda!(|| {
        let angle = remember_infinite_transition().animate_float(0.0, 360.0, infinite_repeatable(linear(1000)));
        Box(modifier = Modifier.size(40.dp()).graphics_layer_block(move |layer| layer.rotation_z = angle.value()));
    }));
    assert!(drawn_only.host().has_pending_work());
}

// ---- C++ 16078 ----------------------------------------------------------------------------------------------------

fn what_is_drawn_out_of_view_goes_on_with_its_time() {
    // C++'s what_is_drawn_out_of_view_asks_for_no_frames, adapted (D4): out of view an endless animation still ticks
    // (Compose's), and drawn again in view it is where its time has taken it; one that leaves the composition asks for
    // no frames.
    let scroll = ScrollState::new(0);
    let drawn = Rc::new(RefCell::new(Vec::<f32>::new()));
    let (sc, d) = (scroll.clone(), drawn.clone());
    let rule = KompozTestRule::empty_with_viewport(IntSize::new(400, 800));
    rule.main_clock().set_auto_advance(false);
    rule.set_content(kompozable_lambda!(move || {
        let phase = remember_infinite_transition().animate_float(0.0, 1000.0, infinite_repeatable(linear(1000)));
        let d = d.clone();
        Column(
            modifier = Modifier.height(200.dp()).vertical_scroll(&sc),
            content = move || {
                Box(modifier = Modifier.size_wh(100.dp(), 300.dp()));
                let (p, d) = (phase.clone(), d.clone());
                // (at 300: out of view)
                Canvas(Modifier.size(40.dp()), move |scope| {
                    d.borrow_mut().push(p.value());
                    scope.draw_rect(Color::Green, Offset::new(p.value() / 100.0, 0.0), Size::new(10.0, 10.0));
                });
            },
        );
    }));
    rule.draw();
    assert!(rule.host().has_pending_work()); // (out of view: it ticks on)
    let at = *drawn.borrow().last().unwrap();
    rule.main_clock().advance_time_by(96);
    rule.draw();
    assert!(*drawn.borrow().last().unwrap() > at + 50.0);
    scroll.dispatch_raw_delta(150.0); // in view
    rule.main_clock().advance_time_by_frame();
    rule.draw();
    let at = *drawn.borrow().last().unwrap();
    rule.main_clock().advance_time_by(96);
    rule.draw();
    assert!(rule.host().has_pending_work() && *drawn.borrow().last().unwrap() != at);

    // One that never ends, gone from the composition, asks for none.
    let target = mutable_state_of(0.0f32);
    let shown = mutable_state_of(true);
    let (t, sh) = (target.clone(), shown.clone());
    let endless = running(kompozable_lambda!(move || {
        if sh.get() {
            let t = t.clone();
            group(move || {
                let spec: Arc<dyn AnimationSpec<f32>> = Arc::new(infinite_repeatable(linear(500)));
                let a = animate_float_as_state(t.get(), spec);
                Box(modifier = Modifier.size(40.dp()).draw_behind(move |_| {
                    let _ = a.value();
                }));
            });
        }
    }));
    target.set(1.0);
    endless.main_clock().advance_time_by_frame();
    endless.draw();
    assert!(endless.host().has_pending_work());
    shown.set(false);
    endless.main_clock().advance_time_by_frame();
    endless.wait_for_idle();
    assert!(!endless.host().has_pending_work());
}

// ---- C++ 29876, its infinite transition part ----------------------------------------------------------------------

fn infinite_colors_animate_in_oklab_past_their_end() {
    // Along an easing past its end, white to gray goes darker than gray: a state read in drawing and a value read in
    // composition.
    let beyond = CubicBezierEasing(0.3, 0.0, 0.4, 1.8);
    let gray = Color::from_argb(0xFF808080);
    let (state_darker, value_darker) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let (sd, vd) = (state_darker.clone(), value_darker.clone());
    let rule = running(kompozable_lambda!(move || {
        let t = remember_infinite_transition();
        let shade = t.animate_color(Color::White, gray, infinite_repeatable(TweenSpec::new(400, 0, beyond)));
        let value = t.animate_color(Color::White, gray, infinite_repeatable(TweenSpec::new(400, 0, beyond))).value();
        vd.set(vd.get() || value.red() < gray.red());
        let sd = sd.clone();
        Box(modifier =
            Modifier.size(10.dp()).draw_behind(move |_| sd.set(sd.get() || shade.value().red() < gray.red())));
    }));
    for _ in 0..30 {
        rule.main_clock().advance_time_by_frame();
        rule.draw();
    }
    assert!(state_darker.get() && value_darker.get());
}

// ---- Compose's behaviour ----------------------------------------------------------------------------------------

fn new_values_restart_an_infinite_animation() {
    // New initial or target values: a new animation from its start, at the next frame (no continuity).
    let to = mutable_state_of(100.0f32);
    let seen = Rc::new(Cell::new(-1.0f32));
    let (t, s) = (to.clone(), seen.clone());
    let rule = running(kompozable_lambda!(move || {
        let v = remember_infinite_transition(label = "nabız".to_string()).animate_float(
            0.0,
            t.get(),
            infinite_repeatable(linear(1000)),
        );
        s.set(v.value());
    }));
    rule.main_clock().advance_time_by(480);
    let before = seen.get();
    assert!(before > 30.0 && before < 60.0, "{before}");
    to.set(200.0);
    rule.main_clock().advance_time_by_frame(); // composed: the new values set
    rule.main_clock().advance_time_by_frame(); // its start
    assert!(seen.get() < 1.0, "{}", seen.get());
    rule.main_clock().advance_time_by(160);
    assert!(seen.get() > 20.0 && seen.get() < 40.0, "{}", seen.get());
}

fn a_duration_scale_of_zero_ends_them_at_once() {
    // With a MotionDurationScale of 0 in the effects' context, an infinite animation is at its target at once, then
    // waits for another scale.
    let scale = mutable_state_of(0.0f32);
    let seen = Rc::new(Cell::new(-1.0f32));
    let s = seen.clone();
    let sc = scale.clone();
    let rule = KompozTestRule::empty();
    rule.main_clock().set_auto_advance(false);
    rule.set_content(kompozable_lambda!(move || {
        let sc = sc.clone();
        let context = current_effect_coroutine_context().plus_element(MotionDurationScale::from_fn(move || sc.get()));
        let s = s.clone();
        KompozitionLocalProvider(&[LocalEffectCoroutineContext.provides_unequal(context)], move || {
            let v = remember_infinite_transition().animate_float(0.0, 10.0, infinite_repeatable(linear(1000)));
            s.set(v.value());
        });
    }));
    rule.main_clock().advance_time_by(48);
    assert_eq!(seen.get(), 10.0);
    scale.set(1.0);
    rule.main_clock().advance_time_by(200);
    assert!(seen.get() < 10.0, "{}", seen.get());
}

fn the_rules_policy_cancels_them_while_its_clock_advances_on_its_own() {
    // (Compose's test rule's InfiniteAnimationPolicy) With auto advance on, waiting for idle is possible: the
    // animation is cancelled at its first frame, its value where it started.
    let seen = Rc::new(Cell::new(-1.0f32));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v = remember_infinite_transition().animate_float(0.0, 1.0, infinite_repeatable(linear(100)));
        s.set(v.value());
    }));
    rule.main_clock().advance_time_by(160);
    assert_eq!(seen.get(), 0.0);
    rule.wait_for_idle();
}

tests!(
    infinite_transition_keeps_its_scope_animating,
    infinite_transition_takes_compose_named_arguments,
    infinite_transition_states_are_read_where_they_are_needed,
    what_is_drawn_out_of_view_goes_on_with_its_time,
    infinite_colors_animate_in_oklab_past_their_end,
    new_values_restart_an_infinite_animation,
    a_duration_scale_of_zero_ends_them_at_once,
    the_rules_policy_cancels_them_while_its_clock_advances_on_its_own,
);

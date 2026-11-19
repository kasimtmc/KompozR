//! Values animated as they change (Compose's animate*AsState and animateColorAsState): the C++ Kompoz's
//! animation_reaches_target (1485), animated_states_are_read_where_they_are_needed (15889),
//! a_local_provided_from_an_animated_state_is_followed_by_its_readers_alone (15976), the animateFloatAsState part of
//! keyframes_go_through_their_frames (24327) and the animateColorAsState parts of
//! colors_animate_in_oklab_as_composes_vector_converter (29876); Compose's behaviour of the others (every type,
//! the finished listener, the visibility threshold, a new target on the way).
//!
//! Timing: as Compose's, an animation set off by a composition takes its start time at the frame after it (its
//! coroutine waits for one): the C++ Kompoz started it in that frame, one frame earlier.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::Modifier;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

fn linear<T: 'static>(ms: i32) -> Arc<dyn AnimationSpec<T>> {
    Arc::new(TweenSpec::<T>::new(ms, 0, LinearEasing))
}

fn x_of(rule: &KompozTestRule, tag: &str) -> f32 {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root().left
}

// ---- C++ 1485 -----------------------------------------------------------------------------------------------------

fn animation_reaches_target() {
    let target = mutable_state_of(0.0f32);
    let seen = Rc::new(RefCell::new(Vec::<f32>::new()));
    let (t, s) = (target.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v = animate_float_as_state(t.get(), linear(160));
        s.borrow_mut().push(v.value());
    }));
    target.set(100.0);
    rule.advance_frame(); // the frame that sees the new target
    rule.advance_frame(); // the animation's first frame: its start
    rule.advance_frame();
    rule.advance_frame();
    let mid = *seen.borrow().last().unwrap();
    assert!(mid > 0.0 && mid < 100.0, "{mid}");
    rule.wait_for_idle();
    assert_eq!(*seen.borrow().last().unwrap(), 100.0);
}

// ---- C++ 15889 ----------------------------------------------------------------------------------------------------

fn animated_states_are_read_where_they_are_needed() {
    // As Compose's State from animateFloatAsState: read while drawing, only drawn again; while laying out, only laid
    // out again; in a composition, composed again. At rest, nothing is asked for.
    let on = mutable_state_of(false);
    let drawn = Rc::new(RefCell::new(Vec::<f32>::new()));
    let tints = Rc::new(RefCell::new(Vec::<Color>::new()));
    let (composed, labels) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (o, d, ti, c, l) = (on.clone(), drawn.clone(), tints.clone(), composed.clone(), labels.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        c.set(c.get() + 1);
        let movement = animate_float_as_state(if o.get() { 100.0 } else { 0.0 }, linear(320));
        let tint = animate_color_as_state(if o.get() { Color::White } else { Color::Black }, linear(320));
        let (d, ti, l) = (d.clone(), ti.clone(), l.clone());
        Column(move || {
            let (m, d) = (movement.clone(), d.clone());
            Box(modifier = Modifier.size(40.dp()).graphics_layer_block(move |layer| {
                layer.translation_x = m.value();
                d.borrow_mut().push(layer.translation_x);
            }));
            let (t, ti) = (tint.clone(), ti.clone());
            Box(modifier = Modifier.size(40.dp()).draw_behind(move |_| ti.borrow_mut().push(t.value())));
            let m = movement.clone();
            Box(modifier =
                Modifier.offset_px(move |_| IntOffset::new(m.value() as i32, 0)).size(10.dp()).test_tag("kayan"));
            // (composed again as it changes)
            let (m, l) = (movement.clone(), l.clone());
            scoped(move || {
                l.set(l.get() + 1);
                Text(format!("{}", m.value() as i32), modifier = Modifier.test_tag("değer"));
            });
        });
    }));
    rule.draw();
    assert!(!rule.host().has_pending_work() && *drawn.borrow().last().unwrap() == 0.0); // at rest
    on.set(true);
    rule.advance_frame(); // composed with the new target: on its way
    let (composed_before, labels_before) = (composed.get(), labels.get());
    rule.draw();
    assert!(rule.host().has_pending_work());
    rule.advance_time_by(160);
    rule.draw();
    assert_eq!(composed.get(), composed_before); // the section never composed again...
    assert!(labels.get() > labels_before && !rule.on_node_with_text("0").exists()); // ...but the text reading it did
    let (x, tint) = (*drawn.borrow().last().unwrap(), *tints.borrow().last().unwrap());
    assert!(x > 20.0 && x < 80.0 && tint != Color::Black && tint != Color::White, "{x} {tint:?}");
    assert!(x_of(&rule, "kayan") > 20.0); // laid out again where it is now
    rule.wait_for_idle();
    rule.draw();
    assert!(*drawn.borrow().last().unwrap() == 100.0 && *tints.borrow().last().unwrap() == Color::White);
    assert!(rule.on_node_with_text("100").exists());
    assert!(near(x_of(&rule, "kayan"), 100.0, 0.5) && composed.get() == composed_before);
    assert!(!rule.host().has_pending_work()); // arrived: nothing more asked for

    // Read only while drawing: what is drawn again is asked for, nothing is composed.
    let shown = mutable_state_of(false);
    let composed_only = Rc::new(Cell::new(0));
    let last_alpha = Rc::new(Cell::new(-1.0f32));
    let (sh, co, la) = (shown.clone(), composed_only.clone(), last_alpha.clone());
    let drawn_only = KompozTestRule::new(kompozable_lambda!(move || {
        co.set(co.get() + 1);
        let alpha = animate_float_as_state(if sh.get() { 1.0 } else { 0.0 }, linear(160));
        let la = la.clone();
        Box(modifier = Modifier.size(40.dp()).graphics_layer_block(move |layer| {
            layer.alpha = alpha.value();
            la.set(layer.alpha);
        }));
    }));
    shown.set(true);
    drawn_only.advance_frame();
    let before = composed_only.get();
    drawn_only.advance_frame(); // (its start)
    drawn_only.advance_frame();
    drawn_only.draw();
    assert!(drawn_only.host().has_pending_work() && last_alpha.get() > 0.0 && last_alpha.get() < 1.0);
    drawn_only.wait_for_idle(); // frames until it is done, drawn or not
    drawn_only.draw();
    assert!(last_alpha.get() == 1.0 && composed_only.get() == before && !drawn_only.host().has_pending_work());

    // A new target on the way goes on from where it is now, read or not since, and asks no frames of the composition
    // for it.
    let to = mutable_state_of(0.0f32);
    let composed_turn = Rc::new(Cell::new(0));
    let x: Rc<RefCell<Option<State<f32>>>> = Rc::default(); // read only here, by hand
    let (t, ct, xs) = (to.clone(), composed_turn.clone(), x.clone());
    let turned = KompozTestRule::new(kompozable_lambda!(move || {
        ct.set(ct.get() + 1);
        *xs.borrow_mut() = Some(animate_float_as_state(t.get(), linear(320)));
    }));
    let value = || x.borrow().as_ref().unwrap().peek();
    to.set(100.0);
    turned.advance_frame(); // composed with the new target
    turned.advance_frame(); // its start
    assert_eq!(value(), 0.0);
    turned.advance_time_by(80);
    assert_near(value(), 25.0, 0.01);
    turned.advance_time_by(80); // over half way, not read since a quarter of the way
    to.set(0.0);
    turned.advance_frame(); // composed with the new target (one more frame of the old animation)
    let composed_turned = composed_turn.get();
    let back = value();
    assert_near(back, 55.0, 0.01); // back from where it is (not from a quarter, where it was last read)
    turned.advance_frame(); // the new animation's start (from there)
    turned.advance_frame();
    turned.advance_frame();
    // (Compose's State has no target nor running flag: it is on its way down, frames asked for)
    assert!(composed_turn.get() == composed_turned && value() < back && turned.host().has_pending_work());
}

// ---- C++ 15976 ----------------------------------------------------------------------------------------------------

/// A color local, as Compose's LocalContentColor (C++ provided its AnimatedState to it, an overload Compose hasn't:
/// Compose's way is `LocalContentColor providesComputed { color.value }`, the State read where the local is).
static LocalTint: KompozitionLocal<Color> = KompozitionLocal::new(|| Color::Black);

/// A composable of its own, skipped when its caller composes again (its arguments the same: none), as Icon and Text
/// are: what it read alone composes it again.
#[track_caller]
fn reader(content: impl Fn() + 'static) {
    fn same(_: &(), _: &()) -> bool {
        true
    }
    restartable((), Some(same), move |_| content());
}

fn a_local_provided_from_an_animated_state_is_followed_by_its_readers_alone() {
    let on = mutable_state_of(false);
    let (composed, reads) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let seen = Rc::new(Cell::new(Color::Transparent));
    let (o, c, r, s) = (on.clone(), composed.clone(), reads.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        c.set(c.get() + 1);
        let color = animate_color_as_state(if o.get() { Color::White } else { Color::Black }, linear(160));
        let (r, s) = (r.clone(), s.clone());
        KompozitionLocalProvider(&[LocalTint.provides_computed(move |_| color.value())], move || {
            // (a reader of its own, as Icon and Text are)
            let (r, s) = (r.clone(), s.clone());
            reader(move || {
                r.set(r.get() + 1);
                s.set(LocalTint.current());
            });
        });
    }));
    assert_eq!(seen.get(), Color::Black);
    on.set(true);
    rule.advance_frame(); // the provider composed again with the new target...
    let (before, reads_before) = (composed.get(), reads.get());
    rule.advance_time_by(80);
    assert!(composed.get() == before && reads.get() > reads_before); // ...and from then on only the reader
    assert!(seen.get() != Color::Black && seen.get() != Color::White);
    rule.wait_for_idle();
    assert!(seen.get() == Color::White && composed.get() == before && !rule.host().has_pending_work());

    // The provider composed again for something else, giving the same state: its readers stay.
    let tick = mutable_state_of(0);
    let (ticks, tick_reads) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (tk, tc, tr) = (tick.clone(), ticks.clone(), tick_reads.clone());
    let same = KompozTestRule::new(kompozable_lambda!(move || {
        if tk.get() >= 0 {
            tc.set(tc.get() + 1);
        }
        let color = animate_color_as_state(Color::Red, Arc::new(snap()));
        let tr = tr.clone();
        KompozitionLocalProvider(&[LocalTint.provides_computed(move |_| color.value())], move || {
            let tr = tr.clone();
            reader(move || {
                tr.set(tr.get() + 1);
                let _ = LocalTint.current();
            });
        });
    }));
    let reads_once = tick_reads.get();
    tick.set(1);
    same.wait_for_idle();
    assert!(ticks.get() == 2 && tick_reads.get() == reads_once, "{} {} {}", ticks.get(), tick_reads.get(), reads_once);

    // Given a plain value (or another state's) again, its readers read that.
    let other = mutable_state_of(false);
    let (ot, s) = (other.clone(), seen.clone());
    let swapped = KompozTestRule::new(kompozable_lambda!(move || {
        let a = animate_color_as_state(Color::Red, Arc::new(snap()));
        let _b = animate_color_as_state(Color::Blue, Arc::new(snap()));
        let provided =
            if ot.get() { LocalTint.provides(Color::Green) } else { LocalTint.provides_computed(move |_| a.value()) };
        let s = s.clone();
        KompozitionLocalProvider(&[provided], move || {
            let s = s.clone();
            reader(move || s.set(LocalTint.current()));
        });
    }));
    assert_eq!(seen.get(), Color::Red);
    other.set(true);
    swapped.wait_for_idle();
    assert_eq!(seen.get(), Color::Green);
    other.set(false);
    swapped.wait_for_idle();
    assert_eq!(seen.get(), Color::Red);
}

// ---- C++ 24327, its animateFloatAsState part -----------------------------------------------------------------------

fn keyframes_go_through_their_frames_as_state() {
    // From 0 to 10 through 5 (not halfway's 50).
    let to = mutable_state_of(0.0f32);
    let (seen, peak) = (Rc::new(Cell::new(0.0f32)), Rc::new(Cell::new(0.0f32)));
    let (t, s, p) = (to.clone(), seen.clone(), peak.clone());
    let state = KompozTestRule::new(kompozable_lambda!(move || {
        let spec: Arc<dyn AnimationSpec<f32>> = Arc::new(keyframes(|k: &mut KeyframesSpecConfig<f32>| {
            k.duration_millis = 400;
            k.at(5.0, 100);
            k.at(5.0, 200);
        }));
        let v = animate_float_as_state(t.get(), spec).value();
        s.set(v);
        p.set(p.get().max(v));
    }));
    to.set(10.0);
    state.wait_for_idle();
    assert!(seen.get() == 10.0 && peak.get() == 10.0); // 5, 5, then up to 10
    to.set(0.0);
    state.advance_time_by(150);
    assert_near(seen.get(), 5.0, 0.3);
}

// ---- C++ 29876, its animateColorAsState parts ---------------------------------------------------------------------

/// The color `fraction` of the way from `from` to `to` in Oklab, as Compose's converter has it (C++'s
/// `detail::animatedColor`).
fn animated_color(from: Color, to: Color, fraction: f32) -> Color {
    let converter = Color::VectorConverter(to.color_space());
    let (a, b) = (converter.convert_to_vector(from), converter.convert_to_vector(to));
    let l = |x: f32, y: f32| x + (y - x) * fraction;
    converter.convert_from_vector(AnimationVector4D(l(a.v1, b.v1), l(a.v2, b.v2), l(a.v3, b.v3), l(a.v4, b.v4)))
}

fn same_color(a: Color, b: Color) -> bool {
    near(a.red(), b.red(), 1e-4) && near(a.green(), b.green(), 1e-4) && near(a.blue(), b.blue(), 1e-4)
}

fn colors_animate_in_oklab_as_state() {
    // animateColorAsState goes so: halfway along a linear tween, L halfway.
    let target = mutable_state_of(Color::Black);
    let seen = Rc::new(Cell::new(Color::Transparent));
    let (t, s) = (target.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        s.set(animate_color_as_state(t.get(), linear(32)).value());
    }));
    target.set(Color::White);
    rule.advance_frame(); // the frame that sees the new target
    rule.advance_frame(); // its start
    rule.advance_frame(); // 16 of 32 ms
    assert_eq!(seen.get().to_argb(), 0xFF636363);
    rule.wait_for_idle();
    assert_eq!(seen.get(), Color::White);

    // A spring past its target, each frame as its progress has it (not held at the target, as a lerp's fraction is):
    // white to gray goes darker than gray as it bounces.
    let bouncy: Arc<dyn AnimationSpec<Color>> =
        Arc::new(SpringSpec::new(Spring::DampingRatioHighBouncy, Spring::StiffnessMedium, None));
    let progress =
        VectorizedSpringSpec::<AnimationVector1D>::new(Spring::DampingRatioHighBouncy, Spring::StiffnessMedium, None);
    let gray = Color::from_argb(0xFF808080);
    let to = mutable_state_of(Color::White);
    let got = Rc::new(Cell::new(Color::Transparent));
    let (t, g, spec) = (to.clone(), got.clone(), bouncy.clone());
    let springy = KompozTestRule::new(kompozable_lambda!(move || {
        g.set(animate_color_as_state(t.get(), spec.clone()).value());
    }));
    to.set(gray);
    springy.advance_frame(); // the frame that sees the new target
    springy.advance_frame(); // its start
    let (mut as_progress, mut darker) = (true, false);
    let (zero, one) = (AnimationVector1D(0.0), AnimationVector1D(1.0));
    // (it ends as its four floats are within their threshold: sooner than the fraction's own spring)
    let converter = Color::VectorConverter(gray.color_space());
    let (from, to_vector) = (converter.convert_to_vector(Color::White), converter.convert_to_vector(gray));
    let end =
        VectorizedSpringSpec::<AnimationVector4D>::new(Spring::DampingRatioHighBouncy, Spring::StiffnessMedium, None)
            .get_duration_nanos(from, to_vector, AnimationVector4D::default());
    for frame in 1..60 {
        springy.advance_frame();
        let play_time_nanos = 16_000_000 * frame as i64;
        if play_time_nanos >= end {
            assert_eq!(got.get(), gray);
            break;
        }
        let p = progress.get_value_from_nanos(play_time_nanos, zero, one, zero).value;
        as_progress = as_progress && same_color(got.get(), animated_color(Color::White, gray, p));
        darker = darker || got.get().red() < gray.red();
    }
    assert!(as_progress && darker, "{as_progress} {darker}");
}

// ---- Compose's behaviour ----------------------------------------------------------------------------------------

fn every_type_animates_to_its_target() {
    let on = mutable_state_of(false);
    let seen = Rc::new(RefCell::new(String::new()));
    let (o, s) = (on.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v = o.get();
        let dp = animate_dp_as_state(if v { 10.dp() } else { 0.dp() });
        let size = animate_size_as_state(if v { Size::new(4.0, 6.0) } else { Size::Zero });
        let offset = animate_offset_as_state(if v { Offset::new(3.0, -3.0) } else { Offset::Zero });
        let rect = animate_rect_as_state(if v { Rect::new(1.0, 2.0, 3.0, 4.0) } else { Rect::Zero });
        let int = animate_int_as_state(if v { 7 } else { 0 });
        let int_offset = animate_int_offset_as_state(if v { IntOffset::new(5, 6) } else { IntOffset::Zero });
        let int_size = animate_int_size_as_state(if v { IntSize::new(8, 9) } else { IntSize::Zero });
        let value = animate_value_as_state(
            if v { DpOffset::new(1.dp(), 2.dp()) } else { DpOffset::Zero },
            DpOffset::VectorConverter,
        );
        *s.borrow_mut() = format!(
            "{} {:?} {:?} {:?} {} {:?} {:?} {:?}",
            dp.value().value(),
            size.value(),
            offset.value(),
            rect.value(),
            int.value(),
            int_offset.value(),
            int_size.value(),
            value.value()
        );
    }));
    let at_rest = seen.borrow().clone();
    on.set(true);
    rule.wait_for_idle();
    let arrived = format!(
        "10 {:?} {:?} {:?} 7 {:?} {:?} {:?}",
        Size::new(4.0, 6.0),
        Offset::new(3.0, -3.0),
        Rect::new(1.0, 2.0, 3.0, 4.0),
        IntOffset::new(5, 6),
        IntSize::new(8, 9),
        DpOffset::new(1.dp(), 2.dp())
    );
    assert_ne!(at_rest, arrived);
    assert_eq!(*seen.borrow(), arrived);
}

fn the_finished_listener_hears_where_it_arrived() {
    let to = mutable_state_of(0.0f32);
    let heard = Rc::new(RefCell::new(Vec::<f32>::new()));
    let (t, h) = (to.clone(), heard.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let h = h.clone();
        let listener: Rc<dyn Fn(f32)> = Rc::new(move |v| h.borrow_mut().push(v));
        animate_float_as_state(t.get(), linear(100), finished_listener = Some(listener), label = "alfa".to_string());
    }));
    to.set(5.0);
    rule.advance_time_by(48);
    // Interrupted on its way: no word of the first, the second arrives.
    to.set(9.0);
    rule.wait_for_idle();
    assert_eq!(*heard.borrow(), vec![9.0]);
}

fn a_visibility_threshold_ends_a_spring_early() {
    // The default spring stops within the threshold given (Compose's animateFloatAsState makes it of it): a coarse
    // one ends sooner than the default 0.01.
    fn frames_to_rest(threshold: Option<f32>) -> u32 {
        let to = mutable_state_of(0.0f32);
        let t = to.clone();
        let rule = KompozTestRule::new(kompozable_lambda!(move || {
            match threshold {
                Some(th) => {
                    animate_float_as_state(t.get(), visibility_threshold = th);
                }
                None => {
                    animate_float_as_state(t.get());
                }
            }
        }));
        to.set(1000.0);
        let mut frames = 0;
        rule.advance_frame();
        while rule.host().has_pending_work() {
            rule.advance_frame();
            frames += 1;
        }
        frames
    }
    let (default, coarse) = (frames_to_rest(None), frames_to_rest(Some(50.0)));
    assert!(coarse < default, "{coarse} {default}");
}

fn animatable_of_a_color_animates_in_oklab() {
    // Compose animation's `Animatable(Color)`: its converter is the color's space's.
    let anim = Animatable(Color::Black);
    assert_eq!(anim.value(), Color::Black);
    let red_p3 = Color::new_in(1.0, 0.0, 0.0, 1.0, &kompoz_graphics::colorspace::ColorSpaces::DisplayP3);
    let p3 = Animatable(red_p3);
    assert!(p3.value().color_space() == red_p3.color_space());
}

tests!(
    animation_reaches_target,
    animated_states_are_read_where_they_are_needed,
    a_local_provided_from_an_animated_state_is_followed_by_its_readers_alone,
    keyframes_go_through_their_frames_as_state,
    colors_animate_in_oklab_as_state,
    every_type_animates_to_its_target,
    the_finished_listener_hears_where_it_arrived,
    a_visibility_threshold_ends_a_spring_early,
    animatable_of_a_color_animates_in_oklab,
);

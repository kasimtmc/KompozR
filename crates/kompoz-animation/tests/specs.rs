//! animation-core's value layer without a composition: vectors and converters, easings, the spring and its
//! duration estimate, tween, snap, keyframes (with arcs, along a spline), repeats, decays and Android's fling
//! spline, against Compose's formulas; the C++ Kompoz's animation_spec_progress (3093) and the spec part of
//! keyframes_go_through_their_frames (24150), on Compose's API (where Compose's numbers differ from C++'s, said
//! where).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::*;
use kompoz_graphics::colorspace::ColorSpaces;
use kompoz_graphics::{Color, Density, Dp, DpOffset, DpSize, IntOffset, IntSize, Offset, Path, Rect, Size};
use std::collections::HashMap;
use std::sync::Arc;

const MS: i64 = 1_000_000;

/// A float animation of `spec` from `from` to `to` (Compose's TargetBasedAnimation, still at the start).
fn float_animation(
    spec: &(impl AnimationSpec<f32> + ?Sized),
    from: f32,
    to: f32,
) -> TargetBasedAnimation<f32, AnimationVector1D> {
    TargetBasedAnimation::new(spec, f32::VectorConverter, from, to, None)
}

/// Its value `ms` in, and whether it has finished there.
fn at(anim: &impl Animation<f32, AnimationVector1D>, ms: i64) -> (f32, bool) {
    (anim.get_value_from_nanos(ms * MS), anim.is_finished_from_nanos(ms * MS))
}

/// A cubic bezier from 0 to 1 solved by halving (to check Compose's root finding against).
fn bezier_by_halving(a: f32, b: f32, c: f32, d: f32, x: f32) -> f32 {
    let curve =
        |p1: f64, p2: f64, t: f64| 3.0 * (1.0 - t) * (1.0 - t) * t * p1 + 3.0 * (1.0 - t) * t * t * p2 + t * t * t;
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if curve(a as f64, c as f64, mid) < x as f64 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    curve(b as f64, d as f64, (lo + hi) / 2.0) as f32
}

// ---- Vectors and converters -----------------------------------------------------------------------------------

fn vectors_hold_their_floats_and_print_as_kotlin() {
    let mut v = AnimationVector((1.0, 2.0, 3.0, 4.0));
    assert_eq!(v, AnimationVector4D(1.0, 2.0, 3.0, 4.0));
    assert_eq!((v.get(0), v.get(3), v.get(4)), (1.0, 4.0, 0.0));
    v.set(2, 9.0);
    v.set(7, 9.0);
    assert_eq!(v, AnimationVector4D(1.0, 2.0, 9.0, 4.0));
    assert_eq!(v.new_instance(), AnimationVector4D(0.0, 0.0, 0.0, 0.0));
    v.reset();
    assert_eq!(v, AnimationVector4D::default());
    assert_eq!(AnimationVector(1.5).to_string(), "AnimationVector1D: value = 1.5");
    assert_eq!(AnimationVector((1.0, 2.0)).to_string(), "AnimationVector2D: v1 = 1.0, v2 = 2.0");
    assert_eq!((AnimationVector1D::SIZE, AnimationVector3D::SIZE), (1, 3));
}

fn converters_turn_values_into_vectors_and_back() {
    assert_eq!(f32::VectorConverter.convert_to_vector(2.5), AnimationVector1D(2.5));
    assert_eq!(i32::VectorConverter.convert_from_vector(AnimationVector1D(2.7)), 2); // (toInt: toward zero)
    assert_eq!(Dp::VectorConverter.convert_from_vector(AnimationVector1D(3.0)), Dp(3.0));
    let o = Offset::new(1.0, -2.0);
    assert_eq!(Offset::VectorConverter.convert_from_vector(Offset::VectorConverter.convert_to_vector(o)), o);
    let r = Rect::new(1.0, 2.0, 3.0, 4.0);
    assert_eq!(Rect::VectorConverter.convert_to_vector(r), AnimationVector4D(1.0, 2.0, 3.0, 4.0));
    assert_eq!(Size::VectorConverter.convert_to_vector(Size::new(5.0, 6.0)), AnimationVector2D(5.0, 6.0));
    let d = DpOffset::new(Dp(1.0), Dp(2.0));
    assert_eq!(DpOffset::VectorConverter.convert_from_vector(DpOffset::VectorConverter.convert_to_vector(d)), d);
    let s = DpSize::new(Dp(3.0), Dp(4.0));
    assert_eq!(DpSize::VectorConverter.convert_from_vector(DpSize::VectorConverter.convert_to_vector(s)), s);
    // IntOffset rounds half up (Math.round); IntSize also keeps itself at 0 or more.
    assert_eq!(IntOffset::VectorConverter.convert_from_vector(AnimationVector2D(1.5, -1.5)), IntOffset::new(2, -1));
    assert_eq!(IntSize::VectorConverter.convert_from_vector(AnimationVector2D(2.49, -3.0)), IntSize::new(2, 0));
    // Converters are equal when they are the same one.
    let (one, other) = (f32::VectorConverter, f32::VectorConverter);
    assert_eq!(one, other);
    assert_ne!(f32::VectorConverter.clone(), TwoWayConverter::new(AnimationVector1D, |v: AnimationVector1D| v.value));
    let custom = TwoWayConverter(|it: f32| AnimationVector1D(it * 2.0), |v: AnimationVector1D| v.value / 2.0);
    assert!(custom == custom.clone() && custom != TwoWayConverter(|it: f32| AnimationVector1D(it), |v| v.value));
    assert_eq!(custom.create_zero_vector_from(4.0), AnimationVector1D(0.0));
}

fn visibility_thresholds_are_composes() {
    assert_eq!(Offset::VisibilityThreshold, Offset::new(1.0, 1.0));
    assert_eq!(IntOffset::VisibilityThreshold, IntOffset::new(1, 1));
    assert_eq!(i32::VisibilityThreshold, 1);
    assert_eq!(Dp::VisibilityThreshold, Dp(0.4));
    assert_eq!(DpOffset::VisibilityThreshold, DpOffset::new(Dp(0.4), Dp(0.4)));
    assert_eq!(Size::VisibilityThreshold, Size::new(1.0, 1.0));
    assert_eq!(IntSize::VisibilityThreshold, IntSize::new(1, 1));
    assert_eq!(Rect::VisibilityThreshold, Rect::new(1.0, 1.0, 1.0, 1.0));
    // (the converters' map Transition and animate*AsState pick the default spring's threshold from)
    assert_eq!(visibility_threshold_of(&f32::VectorConverter), Some(0.01));
    assert_eq!(visibility_threshold_of(&IntSize::VectorConverter), Some(1.0));
    assert_eq!(visibility_threshold_of(&Dp::VectorConverter), Some(0.4));
    assert_eq!(visibility_threshold_of(&Rect::VectorConverter), Some(1.0));
    let custom = TwoWayConverter(|it: f32| AnimationVector1D(it), |v: AnimationVector1D| v.value);
    assert_eq!(visibility_threshold_of(&custom), None);
}

fn colors_animate_in_oklab() {
    let converter = Color::VectorConverter(&ColorSpaces::Srgb);
    let red = converter.convert_to_vector(Color::Red);
    assert_eq!(red.v1, 1.0); // alpha first, then L, a, b
    let back = converter.convert_from_vector(red);
    assert!((back.red() - 1.0).abs() < 1e-3 && back.green().abs() < 1e-3 && back.blue().abs() < 1e-3);
    // Out of Oklab's range (a spring's overshoot), kept within it.
    let over = converter.convert_from_vector(AnimationVector4D(1.5, 2.0, 0.0, 0.0));
    assert_eq!(over.alpha(), 1.0);
    assert!(over.red() > 0.99 && over.green() > 0.99);
    // Halfway from black to white: Oklab's middle gray, as Compose's ColorVectorConverter makes it.
    let (b, w) = (converter.convert_to_vector(Color::Black), converter.convert_to_vector(Color::White));
    let mid = AnimationVector4D(1.0, (b.v2 + w.v2) / 2.0, (b.v3 + w.v3) / 2.0, (b.v4 + w.v4) / 2.0);
    assert_eq!(converter.convert_from_vector(mid).to_argb(), 0xFF636363);
}

// ---- Easings ---------------------------------------------------------------------------------------------------

fn cubic_bezier_easings_solve_their_curves() {
    for (e, (a, b, c, d)) in [
        (FastOutSlowInEasing, (0.4, 0.0, 0.2, 1.0)),
        (LinearOutSlowInEasing, (0.0, 0.0, 0.2, 1.0)),
        (FastOutLinearInEasing, (0.4, 0.0, 1.0, 1.0)),
        (EaseInOut, (0.42, 0.0, 0.58, 1.0)),
        (EaseOutBack, (0.34, 1.56, 0.64, 1.0)),
        (EaseInBack, (0.36, 0.0, 0.66, -0.56)),
    ] {
        for i in 1..20 {
            let x = i as f32 / 20.0;
            assert_near(e.transform(x), bezier_by_halving(a, b, c, d, x), 2e-4);
        }
        assert_eq!((e.transform(0.0), e.transform(1.0)), (0.0, 1.0));
        // (outside 0..1, the fraction as it is)
        assert_eq!(e.transform(1.5), 1.5);
    }
    // Material's standard curve halfway: 0.7756.
    assert_near(FastOutSlowInEasing.transform(0.5), 0.7756, 1e-3);
    // An overshooting curve goes past 1, within its bounds.
    let peak = (1..100).map(|i| EaseOutBack.transform(i as f32 / 100.0)).fold(0.0, f32::max);
    assert!(peak > 1.05 && peak < 1.2);
    // Equal by their parameters; a constant and the constructor's alike.
    assert_eq!(CubicBezierEasing(0.4, 0.0, 0.2, 1.0), FastOutSlowInEasing);
    assert_eq!(CubicBezierEasing(0.4, 0.0, 0.2, 1.0).transform(0.3), FastOutSlowInEasing.transform(0.3));
    assert!(std::panic::catch_unwind(|| CubicBezierEasing(f32::NAN, 0.0, 1.0, 1.0)).is_err());
    assert_eq!(LinearEasing.transform(0.37), 0.37);
}

fn easing_functions_are_easings_nets() {
    // easeOutBounce at 0.5: in its third bounce stretch.
    let f: f32 = 0.5 - 1.5 / 2.75;
    assert_near(EaseOutBounce.transform(0.5), 7.5625 * f * f + 0.75, 1e-6);
    assert_near(EaseInBounce.transform(0.3), 1.0 - EaseOutBounce.transform(0.7), 1e-6);
    assert_near(EaseInOutBounce.transform(0.25), (1.0 - EaseOutBounce.transform(0.5)) / 2.0, 1e-6);
    let c4 = 2.0 * std::f64::consts::PI / 3.0;
    let expected = -(2f64.powf(10.0 * 0.5 - 10.0)) * ((0.5 * 10.0 - 10.75) * c4).sin();
    assert_near(EaseInElastic.transform(0.5), expected as f32, 1e-6);
    assert_eq!((EaseOutElastic.transform(0.0), EaseOutElastic.transform(1.0)), (0.0, 1.0));
    assert_near(EaseInOutElastic.transform(0.5), 0.5, 1e-6);
    // A closure is an easing too.
    let square = |x: f32| x * x;
    assert_eq!(square.transform(0.5), 0.25);
}

fn path_easings_follow_their_path() {
    // A straight path: linear.
    let mut line = Path::new();
    line.move_to(0.0, 0.0);
    line.line_to(1.0, 1.0);
    let linear = PathEasing(line);
    assert_near(linear.transform(0.3), 0.3, 1e-6);
    // FastOutSlowIn's curve as a path: the same easing.
    let mut curve = Path::new();
    curve.move_to(0.0, 0.0);
    curve.cubic_to(0.4, 0.0, 0.2, 1.0, 1.0, 1.0);
    let path = PathEasing(curve);
    for i in 1..10 {
        let x = i as f32 / 10.0;
        assert_near(path.transform(x), FastOutSlowInEasing.transform(x), 1e-4);
    }
    // Two segments: up to (0.5, 0.8), then to (1, 1).
    let mut bent = Path::new();
    bent.move_to(0.0, 0.0);
    bent.line_to(0.5, 0.8);
    bent.line_to(1.0, 1.0);
    let bent = PathEasing(bent);
    assert_near(bent.transform(0.25), 0.4, 1e-6);
    assert_near(bent.transform(0.75), 0.9, 1e-6);
    // One not reaching 1 along x is invalid (when used, as Compose's throws then).
    let mut short = Path::new();
    short.move_to(0.0, 0.0);
    short.line_to(0.5, 1.0);
    let short = PathEasing(short);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| short.transform(0.2))).is_err());
}

// ---- Spring ----------------------------------------------------------------------------------------------------

/// A spring's displacement and velocity from x0, v0 after t s, solved by hand (unit mass).
fn spring_by_hand(stiffness: f64, damping: f64, x0: f64, v0: f64, t: f64) -> (f64, f64) {
    let w = stiffness.sqrt();
    if damping < 1.0 {
        let wd = w * (1.0 - damping * damping).sqrt();
        let (a, b) = (x0, (v0 + damping * w * x0) / wd);
        let e = (-damping * w * t).exp();
        let x = e * (a * (wd * t).cos() + b * (wd * t).sin());
        let v = e
            * (-damping * w * (a * (wd * t).cos() + b * (wd * t).sin())
                + (-a * wd * (wd * t).sin() + b * wd * (wd * t).cos()));
        (x, v)
    } else if damping == 1.0 {
        let b = v0 + w * x0;
        let e = (-w * t).exp();
        (e * (x0 + b * t), e * (b - w * (x0 + b * t)))
    } else {
        let s = (damping * damping - 1.0).sqrt();
        let (r1, r2) = (-w * (damping - s), -w * (damping + s));
        let c2 = (r1 * x0 - v0) / (r1 - r2);
        let c1 = x0 - c2;
        (c1 * (r1 * t).exp() + c2 * (r2 * t).exp(), c1 * r1 * (r1 * t).exp() + c2 * r2 * (r2 * t).exp())
    }
}

fn springs_move_as_a_damped_spring() {
    for (damping, stiffness) in [
        (Spring::DampingRatioNoBouncy, Spring::StiffnessMedium),
        (Spring::DampingRatioHighBouncy, Spring::StiffnessLow),
        (Spring::DampingRatioMediumBouncy, Spring::StiffnessHigh),
        (2.0, Spring::StiffnessVeryLow),
    ] {
        let spec = FloatSpringSpec::new(damping, stiffness, 0.01);
        for ms in [0i64, 16, 50, 120, 333, 800] {
            let (x, v) = spring_by_hand(stiffness as f64, damping as f64, -10.0, 30.0, ms as f64 / 1000.0);
            assert_near(spec.get_value_from_nanos(ms * MS, -10.0, 0.0, 30.0), x as f32, 1e-3);
            assert_near(spec.get_velocity_from_nanos(ms * MS, -10.0, 0.0, 30.0), v as f32, 2e-2);
        }
        assert_eq!(spec.get_end_velocity(-10.0, 0.0, 30.0), 0.0);
    }
}

fn a_springs_duration_is_where_it_stays_within_its_threshold() {
    for (damping, stiffness, threshold) in [
        (Spring::DampingRatioNoBouncy, Spring::StiffnessMedium, 0.01f32),
        (Spring::DampingRatioLowBouncy, Spring::StiffnessMediumLow, 0.01),
        (Spring::DampingRatioHighBouncy, Spring::StiffnessMedium, 0.5),
        (3.0, Spring::StiffnessLow, 0.01),
    ] {
        let spec = FloatSpringSpec::new(damping, stiffness, threshold);
        let duration = spec.get_duration_nanos(0.0, 100.0, 0.0) / MS;
        // Within the threshold from there on (an underdamped one's envelope is: so is it).
        for ms in (duration..duration + 2000).step_by(7) {
            let x = spec.get_value_from_nanos(ms * MS, 0.0, 100.0, 0.0);
            assert!((x - 100.0).abs() <= threshold * 1.05, "{damping} {stiffness}: {x} at {ms} of {duration}");
        }
        // And not long before.
        let earlier = spec.get_value_from_nanos((duration * 3 / 4) * MS, 0.0, 100.0, 0.0);
        assert!((earlier - 100.0).abs() > threshold * 0.5 || damping < 1.0, "{damping} {stiffness}: settled early");
    }
    // Undamped: forever (Long.MAX_VALUE / 1_000_000 ms).
    assert_eq!(estimate_animation_duration_millis(100.0, 0.0, 0.0, 1.0, 1.0), i64::MAX / 1_000_000);
    // Already there: no time.
    assert_eq!(estimate_animation_duration_millis(100.0, 1.0, 0.0, 0.0, 1.0), 0);
    // The mass form agrees with the stiffness form for a unit mass.
    let (k, z) = (400.0f64, 0.5f64);
    let c = 2.0 * z * k.sqrt();
    assert_eq!(
        estimate_animation_duration_millis_of_mass(k, c, 1.0, 0.0, 100.0, 1.0),
        estimate_animation_duration_millis_f64(k, z, 0.0, 100.0, 1.0)
    );
}

// ---- The C++ Kompoz's animation_spec_progress (3093), on Compose's API ------------------------------------------

fn animation_spec_progress() {
    let t = float_animation(&TweenSpec::<f32>::new(100, 0, LinearEasing), 0.0, 1.0);
    let (v, done) = at(&t, 50);
    assert!(near(v, 0.5, 1e-3) && !done);
    assert_eq!(at(&t, 100), (1.0, true));
    let delayed = float_animation(&TweenSpec::<f32>::new(100, 50, LinearEasing), 0.0, 1.0);
    assert_eq!(at(&delayed, 40), (0.0, false));
    // Springs start at 0, settle at 1; bouncy ones overshoot.
    let stiff = float_animation(&spring::<f32>(), 0.0, 1.0);
    assert_eq!(at(&stiff, 0).0, 0.0);
    let bouncy = float_animation(
        &SpringSpec::<f32>::new(Spring::DampingRatioHighBouncy, Spring::StiffnessMedium, None),
        0.0,
        1.0,
    );
    let overshoot = (0..1000).step_by(5).map(|ms| at(&bouncy, ms).0).fold(0.0, f32::max);
    assert!(overshoot > 1.2);
    assert!(at(&stiff, 1000).1);
    // repeatable(3, Reverse): forward, backward, forward.
    let rep = float_animation(
        &RepeatableSpec::new(
            3,
            TweenSpec::<f32>::new(100, 0, LinearEasing),
            RepeatMode::Reverse,
            StartOffset::default(),
        ),
        0.0,
        1.0,
    );
    assert_near(at(&rep, 125).0, 0.75, 1e-3);
    assert_eq!(at(&rep, 300), (1.0, true));
    let forever = float_animation(
        &InfiniteRepeatableSpec::new(tween::<f32>(), RepeatMode::Restart, StartOffset::default()),
        0.0,
        1.0,
    );
    assert!(!at(&forever, 10_000_000).1 && forever.is_infinite());
}

fn tweens_ease_from_start_to_target() {
    let spec = tween::<f32>(); // 300 ms, FastOutSlowIn
    assert_eq!((spec.duration_millis, spec.delay), (300, 0));
    let anim = float_animation(&spec, 10.0, 20.0);
    for ms in [0i64, 30, 75, 150, 299] {
        let expected = 10.0 + 10.0 * FastOutSlowInEasing.transform(ms as f32 / 300.0);
        assert_near(at(&anim, ms).0, expected, 1e-4);
    }
    // Its velocity: the last millisecond's change, per second.
    let v = anim.get_velocity_vector_from_nanos(150 * MS).value;
    let expected = (anim.get_value_from_nanos(150 * MS) - anim.get_value_from_nanos(149 * MS)) * 1000.0;
    assert_near(v, expected, 1e-2);
    assert_eq!(anim.duration_nanos(), 300 * MS);
    // Equal by duration, delay and easing.
    assert!(TweenSpec::<f32>::new(300, 0, FastOutSlowInEasing) == tween::<f32>());
    let shared: Arc<dyn AnimationSpec<f32>> = Arc::new(tween::<f32>());
    assert!(*shared == *(Arc::new(TweenSpec::<f32>::new(300, 0, FastOutSlowInEasing)) as Arc<dyn AnimationSpec<f32>>));
    assert!(*shared != *(Arc::new(TweenSpec::<f32>::new(300, 0, LinearEasing)) as Arc<dyn AnimationSpec<f32>>));
}

fn snaps_jump_after_their_delay() {
    let anim = float_animation(&SnapSpec::<f32>::new(40), 0.0, 5.0);
    assert_eq!(at(&anim, 39).0, 0.0);
    assert_eq!(at(&anim, 40), (5.0, true));
    assert_eq!(float_animation(&snap::<f32>(), 0.0, 5.0).duration_nanos(), 0);
}

// ---- Keyframes (C++ 24150's spec part) ---------------------------------------------------------------------------

fn keyframes_of(duration: i32, delay: i32, frames: &[(i32, f32, Option<CubicBezierEasing>)]) -> KeyframesSpec<f32> {
    keyframes(|k| {
        k.duration_millis = duration;
        k.delay_millis = delay;
        for &(t, v, e) in frames {
            let entity = k.at(v, t);
            if let Some(e) = e {
                entity.using(e);
            }
        }
    })
}

fn keyframes_go_through_their_frames() {
    // (C++ took keyframes' values as fractions from 0 to 1 for non-floats; Compose's are values: the same from 0 to 1.)
    let k =
        float_animation(&keyframes_of(400, 0, &[(300, 1.2, None), (100, 0.5, Some(FastOutSlowInEasing))]), 0.0, 1.0);
    assert!(near(at(&k, 0).0, 0.0, 1e-3) && near(at(&k, 50).0, 0.25, 1e-3) && near(at(&k, 100).0, 0.5, 1e-3));
    let mid = at(&k, 200).0; // eased from 0.5 to 1.2: past the linear middle
    assert!(
        mid > 0.85 && mid < 1.2 && near(at(&k, 300).0, 1.2, 1e-3) && near(at(&k, 350).0, 1.1, 1e-3) && !at(&k, 350).1
    );
    assert_eq!(at(&k, 400), (1.0, true));
    // Frames at the ends replace the start and the target.
    let start = float_animation(&keyframes_of(200, 0, &[(0, 0.5, None), (200, 0.8, None)]), 0.0, 1.0);
    assert!(near(at(&start, 0).0, 0.5, 1e-3) && near(at(&start, 100).0, 0.65, 1e-3));
    // Repeated backwards: up, then the frames backwards.
    let back = float_animation(
        &RepeatableSpec::new(2, keyframes_of(100, 0, &[(50, 1.0, None)]), RepeatMode::Reverse, StartOffset::default()),
        0.0,
        1.0,
    );
    assert!(near(at(&back, 50).0, 1.0, 1e-3) && near(at(&back, 125).0, 1.0, 1e-3) && near(at(&back, 175).0, 0.5, 1e-3));
    // It waits its delay.
    assert!(near(at(&float_animation(&keyframes_of(100, 20, &[(50, 1.0, None)]), 0.0, 1.0), 10).0, 0.0, 1e-3));
    // For floats the values are values: a pulse from 1 to 1 through 1.3 and 0.9.
    let pulse = float_animation(&keyframes_of(400, 0, &[(100, 1.3, None), (250, 0.9, None)]), 1.0, 1.0);
    assert!(near(at(&pulse, 0).0, 1.0, 1e-3) && near(at(&pulse, 100).0, 1.3, 1e-3));
    assert!(near(at(&pulse, 175).0, 1.1, 1e-3) && near(at(&pulse, 325).0, 0.95, 1e-3) && !at(&pulse, 325).1);
    assert_eq!(at(&pulse, 500), (1.0, true));
    assert!(near(at(&float_animation(&TweenSpec::<f32>::new(100, 0, LinearEasing), 10.0, 20.0), 25).0, 12.5, 1e-3));
    let swing = float_animation(
        &RepeatableSpec::new(2, keyframes_of(100, 0, &[(50, 5.0, None)]), RepeatMode::Reverse, StartOffset::default()),
        0.0,
        10.0,
    );
    assert!(near(at(&swing, 25).0, 2.5, 1e-3) && near(at(&swing, 175).0, 2.5, 1e-3));
    assert!(near(at(&swing, 130).0, 7.0, 1e-3)); // back at 70 ms: 5 to 10, 2/5 on
    // Finished, Compose's animation is at its target (C++ ended an even reversed repeat at the start: 0).
    assert_eq!(at(&swing, 250), (10.0, true));
    // atFraction: of the duration, rounded.
    let fractions = float_animation(
        &keyframes(|k: &mut KeyframesSpecConfig<f32>| {
            k.duration_millis = 200;
            k.at_fraction(4.0, 0.25);
        }),
        0.0,
        8.0,
    );
    assert_eq!(at(&fractions, 50).0, 4.0);
    // Keyframes are equal only to themselves.
    let k1: Arc<dyn AnimationSpec<f32>> = Arc::new(keyframes_of(100, 0, &[]));
    let k2: Arc<dyn AnimationSpec<f32>> = Arc::new(keyframes_of(100, 0, &[]));
    let same = k1.clone();
    assert!(*k1 == *same && *k1 != *k2);
}

fn keyframes_velocity_is_composes() {
    // Compose takes a keyframes spec's velocity as the value a millisecond before minus the value now, times 1000,
    // and both are its one reused vector between keyframes: 0 there; next to a keyframe, minus the slope.
    let spec = keyframes_of(200, 0, &[(100, 10.0, None)]);
    let v = spec.vectorize(&f32::VectorConverter);
    let velocity = |ms: i64| {
        v.get_velocity_from_nanos(ms * MS, AnimationVector1D(0.0), AnimationVector1D(0.0), AnimationVector1D(0.0)).value
    };
    assert_eq!(velocity(50), 0.0);
    assert_near(velocity(101), (10.0 - 9.9) * 1000.0, 1e-1);
    assert_near(velocity(200), 0.1 * 1000.0, 1e-1);
}

fn arcs_go_along_a_quarter_ellipse() {
    let spec = ArcAnimationSpec::<Offset>(ArcMode::ArcBelow, 100, 0, LinearEasing);
    let anim = TargetBasedAnimation::new(
        &spec,
        Offset::VectorConverter,
        Offset::new(0.0, 0.0),
        Offset::new(100.0, 100.0),
        None,
    );
    for ms in [10i64, 30, 50, 70, 90] {
        let p = anim.get_value_from_nanos(ms * MS);
        // On the circle of radius 100 around (100, 0): below the line from the start to the end.
        let r = ((p.x - 100.0).powi(2) + p.y.powi(2)).sqrt();
        assert_near(r, 100.0, 0.5);
        assert!(p.y > p.x);
    }
    // Halfway along it in time, halfway along its length: 45°.
    let mid = anim.get_value_from_nanos(50 * MS);
    assert_near(mid.x, 100.0 - 70.71, 1.0);
    assert_near(mid.y, 70.71, 1.0);
    assert_eq!(anim.get_value_from_nanos(100 * MS), Offset::new(100.0, 100.0));
    // Its velocity is along the arc, at the arc's length per second.
    let v = anim.get_velocity_vector_from_nanos(50 * MS);
    assert_near((v.v1 * v.v1 + v.v2 * v.v2).sqrt(), 100.0 * std::f32::consts::FRAC_PI_2 / 0.1, 20.0);
    // ArcAbove bends the other way.
    let above = TargetBasedAnimation::new(
        &ArcAnimationSpec::<Offset>(ArcMode::ArcAbove, 100, 0, LinearEasing),
        Offset::VectorConverter,
        Offset::new(0.0, 0.0),
        Offset::new(100.0, 100.0),
        None,
    );
    let p = above.get_value_from_nanos(50 * MS);
    assert!(p.x > p.y);
    // Keyframes with an arc mode arc through them.
    let through = keyframes(|k: &mut KeyframesSpecConfig<Offset>| {
        k.duration_millis = 200;
        k.at(Offset::new(0.0, 0.0), 0).using_arc_mode(ArcMode::ArcBelow);
        k.at(Offset::new(100.0, 100.0), 100).using_arc_mode(ArcMode::ArcBelow);
    });
    let anim =
        TargetBasedAnimation::new(&through, Offset::VectorConverter, Offset::Zero, Offset::new(200.0, 0.0), None);
    assert_eq!(anim.get_value_from_nanos(100 * MS), Offset::new(100.0, 100.0));
    let q = anim.get_value_from_nanos(50 * MS);
    assert_near(((q.x - 100.0).powi(2) + q.y.powi(2)).sqrt(), 100.0, 0.5);
}

fn keyframes_with_a_spline_pass_smoothly_through_them() {
    let spec = keyframes_with_spline(|k: &mut KeyframesWithSplineSpecConfig<f32>| {
        k.duration_millis = 300;
        k.at(10.0, 100);
        k.at(30.0, 200);
    });
    let anim = float_animation(&spec, 0.0, 40.0);
    assert_eq!(at(&anim, 100).0, 10.0);
    assert_eq!(at(&anim, 200).0, 30.0);
    // Monotonic data, a monotonic curve: no overshoot between frames, and smooth through them.
    let values: Vec<f32> = (0..=300).map(|ms| at(&anim, ms).0).collect();
    assert!(values.windows(2).all(|w| w[1] >= w[0] - 1e-4));
    let before = values[100] - values[99];
    let after = values[101] - values[100];
    assert!((before - after).abs() < 0.05, "a kink at the keyframe: {before} then {after}");
    // Its velocity is the spline's slope (per second).
    let v = anim.get_velocity_vector_from_nanos(150 * MS).value;
    assert_near(v, (values[151] - values[149]) / 2.0 * 1000.0, 5.0);
    // A periodic bias makes the ends' slopes one.
    let periodic = keyframes_with_spline_periodic(0.5, |k: &mut KeyframesWithSplineSpecConfig<f32>| {
        k.duration_millis = 200;
        k.at(10.0, 100);
    });
    // (up 100 a second, then down as much: the ends' slope, half of each, is 0)
    let v = periodic.vectorize(&f32::VectorConverter);
    let slope = |ms: i64| {
        v.get_velocity_from_nanos(ms * MS, AnimationVector1D(0.0), AnimationVector1D(0.0), AnimationVector1D(0.0)).value
    };
    assert_eq!((slope(0), slope(200)), (0.0, 0.0));
    let plain = keyframes_with_spline(|k: &mut KeyframesWithSplineSpecConfig<f32>| {
        k.duration_millis = 200;
        k.at(10.0, 100);
    })
    .vectorize(&f32::VectorConverter);
    let plain_start =
        plain.get_velocity_from_nanos(0, AnimationVector1D(0.0), AnimationVector1D(0.0), AnimationVector1D(0.0)).value;
    assert_near(plain_start, 100.0, 1e-3);
}

fn repeats_offset_their_start() {
    let linear = TweenSpec::<f32>::new(100, 0, LinearEasing);
    // Fast-forwarded 30 ms: it starts at 0.3, and ends 30 ms sooner.
    let ff = float_animation(
        &RepeatableSpec::new(
            2,
            linear.clone(),
            RepeatMode::Restart,
            StartOffset::new(30, StartOffsetType::FastForward),
        ),
        0.0,
        1.0,
    );
    assert_near(at(&ff, 0).0, 0.3, 1e-3);
    assert_eq!(ff.duration_nanos(), 170 * MS);
    // Delayed 30 ms: still for 30 ms, ends 30 ms later.
    let delayed = float_animation(
        &RepeatableSpec::new(2, linear.clone(), RepeatMode::Restart, StartOffset::new(30, StartOffsetType::Delay)),
        0.0,
        1.0,
    );
    assert_eq!(at(&delayed, 20).0, 0.0);
    assert_near(at(&delayed, 80).0, 0.5, 1e-3);
    assert_eq!(delayed.duration_nanos(), 230 * MS);
    assert_eq!(
        (
            StartOffset::new(30, StartOffsetType::Delay).offset_millis(),
            StartOffset::new(30, StartOffsetType::FastForward).offset_type()
        ),
        (30, StartOffsetType::FastForward)
    );
    // Forever, reversing.
    let forever = float_animation(
        &InfiniteRepeatableSpec::new(linear.clone(), RepeatMode::Reverse, StartOffset::default()),
        0.0,
        1.0,
    );
    assert_near(at(&forever, 1250).0, 0.5, 1e-3);
    assert_near(at(&forever, 1150).0, 0.5, 1e-3);
    assert_near(at(&forever, 1130).0, 0.7, 1e-3);
    // No duration can't repeat forever; fewer than one iteration can't repeat.
    assert!(
        std::panic::catch_unwind(|| InfiniteRepeatableSpec::new(
            TweenSpec::<f32>::new(0, 0, LinearEasing),
            RepeatMode::Restart,
            StartOffset::default()
        ))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| float_animation(
            &RepeatableSpec::new(
                0,
                TweenSpec::<f32>::new(10, 0, LinearEasing),
                RepeatMode::Restart,
                StartOffset::default()
            ),
            0.0,
            1.0
        ))
        .is_err()
    );
}

fn a_start_delay_holds_the_start() {
    let spec = delayed(Arc::new(TweenSpec::<f32>::new(100, 0, LinearEasing)), 50 * MS);
    let anim = float_animation(&spec, 0.0, 1.0);
    assert_eq!(at(&anim, 40).0, 0.0);
    assert_near(at(&anim, 100).0, 0.5, 1e-3);
    assert_eq!(anim.duration_nanos(), 150 * MS);
}

// ---- Decays ---------------------------------------------------------------------------------------------------

fn exponential_decays_slow_down_exponentially() {
    // (C++ 24036's decay part, on Compose's API)
    let decay = FloatExponentialDecaySpec::default();
    assert_near(decay.get_velocity_from_nanos(0, 0.0, 420.0), 420.0, 1e-3);
    assert_near(decay.get_target_value(0.0, 420.0), 100.0, 0.05);
    assert_near((decay.get_duration_nanos(0.0, 420.0) / MS) as f32, 1987.0, 2.0);
    assert_near(decay.get_value_from_nanos(1_000_000 * MS, 10.0, -420.0), -90.0, 0.05);
    // Already slower than its threshold: Compose's duration comes out negative (C++ kept it at 0).
    assert!(decay.get_duration_nanos(0.0, 0.05) <= 0);
    // e^(-4.2 t): after a second, 1.5 % of the speed.
    assert_near(decay.get_velocity_from_nanos(1000 * MS, 0.0, 1000.0), 1000.0 * (-4.2f32).exp(), 1e-2);
    // A friction multiplier of 2: twice as quick to stop, half as far.
    let quick = FloatExponentialDecaySpec::new(2.0, 0.1);
    assert_near(quick.get_target_value(0.0, 420.0), 50.0, 0.05);
    // As a decay of any type: each float on its own.
    let offset = exponential_decay::<Offset>();
    let target =
        offset.calculate_target_value(&Offset::VectorConverter, Offset::new(0.0, 10.0), Offset::new(420.0, -420.0));
    assert!(near(target.x, 100.0, 0.05) && near(target.y, -90.0, 0.05));
    assert_near(exponential_decay::<f32>().calculate_float_target_value(0.0, 420.0), 100.0, 0.05);
    let anim = DecayAnimation::new(&exponential_decay::<f32>(), f32::VectorConverter, 0.0, AnimationVector1D(420.0));
    assert_near(anim.target_value(), 100.0, 0.05);
    // Its end velocity is within the threshold.
    assert!(anim.get_velocity_vector_from_nanos(anim.duration_nanos()).value.abs() <= 0.1);
}

fn flings_follow_androids_spline() {
    // (C++ 24036's spline part: in dp, density 1; the numbers OverScroller's)
    let fling = SplineBasedFloatDecayAnimationSpec::new(Density::new(1.0, 1.0));
    assert_near(fling.get_target_value(0.0, 3200.0), 1464.1, 1.0);
    assert_near((fling.get_duration_nanos(0.0, 3200.0) / MS) as f32, 1307.0, 2.0);
    assert_near(fling.get_velocity_from_nanos(0, 0.0, 3200.0), 3200.0, 10.0);
    assert_near(fling.get_value_from_nanos(1400 * MS, 0.0, 3200.0), 1464.1, 1.0);
    assert_near(fling.get_target_value(100.0, -2000.0), 100.0 - 647.4, 1.0);
    assert_eq!(fling.get_velocity_from_nanos(1400 * MS, 0.0, 3200.0), 0.0);
    let (x, v) =
        (fling.get_value_from_nanos(300 * MS, 0.0, 3200.0), fling.get_velocity_from_nanos(300 * MS, 0.0, 3200.0));
    assert!(x > 600.0 && v < 3200.0 && v > 0.0);
    let duration = fling.get_duration_nanos(0.0, 3200.0);
    assert_near(fling.get_value_from_nanos((duration as f64 * 0.995) as i64, 0.0, 3200.0), 1464.1, 2.0);
    assert!(fling.get_target_value(0.0, 8000.0) > 10.0 * fling.get_target_value(0.0, 2000.0));
    assert_near(fling.get_target_value(0.0, 1000.0), 194.3, 0.5);
    assert_eq!(fling.abs_velocity_threshold(), 0.0);
    // Compose's FlingCalculator: the distance is friction · coefficient · e^(r/(r-1) · ln(0.35 v / (friction ·
    // coefficient))), r = ln(0.78) / ln(0.9), the coefficient g · 39.37 · 160 · density · 0.84.
    let r = (0.78f64.ln() / 0.9f64.ln()) as f32 as f64;
    let coefficient = (9.80665f32 * 39.37 * 1.0 * 160.0 * 0.84) as f64;
    let friction = 0.015 * coefficient;
    let l = (0.35 * 3200.0 / friction).ln();
    assert_near(
        FlingCalculator::new(0.015, Density::new(1.0, 1.0)).fling_distance(3200.0),
        (friction * (r / (r - 1.0) * l).exp()) as f32,
        0.01,
    );
    assert_eq!(
        FlingCalculator::new(0.015, Density::new(1.0, 1.0)).fling_duration(3200.0),
        (1000.0 * (l / (r - 1.0)).exp()) as i64
    );
    // At twice the density a fling of twice as many px per second goes twice as many px, as long.
    let dense = SplineBasedFloatDecayAnimationSpec::new(Density::new(2.0, 1.0));
    assert_near(dense.get_target_value(0.0, 6400.0), 2.0 * fling.get_target_value(0.0, 3200.0), 0.5);
    assert_eq!(dense.get_duration_nanos(0.0, 6400.0), fling.get_duration_nanos(0.0, 3200.0));
    // OverScroller's spline: it starts at 0 (as near as its halving gets), ends at 1.
    assert_near(AndroidFlingSpline::fling_position(0.0).distance_coefficient, 0.0, 1e-4);
    assert_eq!(
        AndroidFlingSpline::fling_position(1.0),
        FlingResult { distance_coefficient: 1.0, velocity_coefficient: 0.0 }
    );
    // As a decay of any type.
    let decay = spline_based_decay::<f32>(Density::new(1.0, 1.0));
    assert_near(decay.calculate_float_target_value(0.0, 3200.0), 1464.1, 1.0);
}

// ---- Vectorized specs on several floats ------------------------------------------------------------------------

fn vectorized_specs_run_each_float() {
    let spring = VectorizedSpringSpec::new(1.0, Spring::StiffnessMedium, Some(AnimationVector2D(0.5, 0.01)));
    let (a, b) = (AnimationVector2D(0.0, 0.0), AnimationVector2D(100.0, 100.0));
    // The first float's threshold is bigger: it ends sooner; the spec's duration is the longest.
    let one = FloatSpringSpec::new(1.0, Spring::StiffnessMedium, 0.5).get_duration_nanos(0.0, 100.0, 0.0);
    let two = FloatSpringSpec::new(1.0, Spring::StiffnessMedium, 0.01).get_duration_nanos(0.0, 100.0, 0.0);
    assert!(one < two);
    assert_eq!(spring.get_duration_nanos(a, b, AnimationVector2D::default()), two);
    let tween = VectorizedTweenSpec::<AnimationVector3D>::new(100, 0, Arc::new(LinearEasing));
    let v = tween.get_value_from_nanos(
        25 * MS,
        AnimationVector3D(0.0, 10.0, 20.0),
        AnimationVector3D(4.0, 30.0, 0.0),
        AnimationVector3D::default(),
    );
    assert_eq!(v, AnimationVector3D(1.0, 15.0, 15.0));
    let keyframes = VectorizedKeyframesSpec::new(
        HashMap::from([(50, (AnimationVector1D(8.0), Arc::new(LinearEasing) as Arc<dyn Easing>))]),
        100,
        0,
    );
    assert_eq!(
        keyframes.get_value_from_nanos(25 * MS, AnimationVector1D(0.0), AnimationVector1D(0.0), AnimationVector1D(0.0)),
        AnimationVector1D(4.0)
    );
    let anim = create_animation(
        Box::new(tween),
        AnimationVector3D(0.0, 0.0, 0.0),
        AnimationVector3D(1.0, 1.0, 1.0),
        AnimationVector3D::default(),
    );
    assert_eq!(anim.get_value_from_nanos(50 * MS), AnimationVector3D(0.5, 0.5, 0.5));
}

tests!(
    vectors_hold_their_floats_and_print_as_kotlin,
    converters_turn_values_into_vectors_and_back,
    visibility_thresholds_are_composes,
    colors_animate_in_oklab,
    cubic_bezier_easings_solve_their_curves,
    easing_functions_are_easings_nets,
    path_easings_follow_their_path,
    springs_move_as_a_damped_spring,
    a_springs_duration_is_where_it_stays_within_its_threshold,
    animation_spec_progress,
    tweens_ease_from_start_to_target,
    snaps_jump_after_their_delay,
    keyframes_go_through_their_frames,
    keyframes_velocity_is_composes,
    arcs_go_along_a_quarter_ellipse,
    keyframes_with_a_spline_pass_smoothly_through_them,
    repeats_offset_their_start,
    a_start_delay_holds_the_start,
    exponential_decays_slow_down_exponentially,
    flings_follow_androids_spline,
    vectorized_specs_run_each_float,
);

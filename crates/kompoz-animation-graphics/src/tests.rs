//! Unit tests of what is crate-internal (as Compose's is): the combined and reversed specs, Android's path
//! interpolator, the pull parser's events (Compose's `forEachChildOf` over every depth), attribute values as aapt2
//! compiles them and TypedArray reads them, the holders Compose reads but doesn't play, path data's interpolation
//! (its quirks kept), Kotlin's Int arithmetic of durations.

use crate::res::path_interpolator::PathInterpolator;
use crate::vector::animator::{
    Animator, Keyframe, ObjectAnimator, PathTimestamp, PropertyValuesHolder, PropertyValuesHolder1D, Timestamp,
    interpolate_path,
};
use crate::vector::compat::animator_parser::{parse_animator_set, parse_object_animator};
use crate::vector::compat::resources::{NoXmlResources, Resource, Resources, attrs};
use crate::vector::compat::xml::{XmlEvent, XmlPullParser, read_document};
use crate::vector::specs::{combined, reversed};
use kompoz_animation::{
    AnimationVector1D, Easing, FiniteAnimationSpec, LinearEasing, RepeatMode, TweenSpec, VectorConverter,
};
use kompoz_graphics::{Path, PathNode, add_path_nodes};
use std::sync::Arc;

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn linear(ms: i32) -> Arc<dyn FiniteAnimationSpec<f32>> {
    Arc::new(TweenSpec::<f32>::new(ms, 0, LinearEasing))
}

/// The value of a float spec `t` ms in from `from` to `to`, and its velocity.
fn eval(spec: &Arc<dyn FiniteAnimationSpec<f32>>, t: i64, from: f32, to: f32) -> (f32, f32, i64) {
    let v = <AnimationVector1D as kompoz_animation::AnimationVector>::vectorize_finite(&**spec, &f32::VectorConverter);
    let (a, b, z) = (AnimationVector1D(from), AnimationVector1D(to), AnimationVector1D(0.0));
    (
        v.get_value_from_nanos(t * 1_000_000, a, b, z).value,
        v.get_velocity_from_nanos(t * 1_000_000, a, b, z).value,
        v.get_duration_nanos(a, b, z),
    )
}

#[test]
fn combined_specs_play_from_their_start_times_and_reversed_ones_backwards() {
    // 0..100 a tween 0 → 1 of 100; from 150 another of 50.
    let c = combined(vec![(0, linear(100)), (150, linear(50))]);
    assert_eq!(eval(&c, 50, 0.0, 10.0).0, 5.0);
    // (between them, the first, past its end)
    assert_eq!(eval(&c, 120, 0.0, 10.0).0, 10.0);
    assert_eq!(eval(&c, 175, 0.0, 10.0).0, 5.0);
    assert_eq!(eval(&c, 0, 0.0, 10.0).2, 200_000_000);
    // Before the first: the first, at a time before its start.
    let late = combined(vec![(100, linear(100))]);
    assert_eq!(eval(&late, 50, 0.0, 10.0).0, 0.0);
    // Reversed over 300: at t the spec at 300 - t, from the target to the start; its velocity negated; 300 long.
    let r = reversed(linear(300), 300);
    let (value, velocity, duration) = eval(&r, 100, 10.0, 0.0);
    assert!(near(value, 6.666_667, 1e-4) && duration == 300_000_000, "{value}");
    let (_, forward_velocity, _) = eval(&linear(300), 200, 0.0, 10.0);
    assert!(near(velocity, -forward_velocity, 1e-3), "{velocity} {forward_velocity}");
    // Equal only to themselves.
    let r2 = reversed(linear(300), 300);
    assert!(*r == *r && *r != *r2);
}

#[test]
fn androids_path_interpolator_approximates_its_path() {
    // Lines: their points.
    let mut lines = Path::new();
    lines.move_to(0.0, 0.0);
    lines.line_to(0.5, 0.2);
    lines.line_to(1.0, 1.0);
    let p = PathInterpolator::of_path(&lines).unwrap();
    assert!(p.get_interpolation(-1.0) == 0.0 && p.get_interpolation(2.0) == 1.0);
    assert!(near(p.get_interpolation(0.25), 0.1, 1e-6) && near(p.get_interpolation(0.75), 0.6, 1e-6));
    // A cubic: within Android's precision of the curve everywhere.
    let mut cubic = Path::new();
    cubic.move_to(0.0, 0.0);
    cubic.cubic_to(0.4, 0.0, 0.2, 1.0, 1.0, 1.0);
    let p = PathInterpolator::of_path(&cubic).unwrap();
    let easing = kompoz_animation::FastOutSlowInEasing;
    for i in 1..100 {
        let x = i as f32 / 100.0;
        assert!(near(p.get_interpolation(x), easing.transform(x), 0.004), "{x}");
    }
    // A quadratic of one control point.
    let q = PathInterpolator::quad(0.5, 0.0).unwrap();
    assert!(near(q.get_interpolation(0.5), 0.25, 0.003));
    // What can't be one.
    let mut short = Path::new();
    short.move_to(0.0, 0.0);
    short.line_to(1.0, 0.5);
    assert_eq!(PathInterpolator::of_path(&short).unwrap_err(), "The Path must start at (0,0) and end at (1,1)");
    // A lone move (Android: an empty path, (0, 0)): not from (0, 0) to (1, 1).
    assert!(PathInterpolator::of_path(&Path::new()).is_err());
}

#[test]
fn the_pull_parser_goes_through_every_tag() {
    let doc = read_document(
        r#"<?xml version="1.0"?><!-- c --><a xmlns:android="http://schemas.android.com/apk/res/android" android:x="1 &amp; 2"><b><c/></b>text<d/></a>"#,
    )
    .unwrap();
    assert_eq!(doc.attribute(crate::vector::compat::xml::ANDROID_NS, "x"), Some("1 & 2"));
    let mut p = XmlPullParser::new(doc);
    let mut seen = Vec::new();
    while p.next() != XmlEvent::EndDocument {
        seen.push((p.event_type(), p.name().to_string(), p.depth()));
    }
    use XmlEvent::{EndTag as E, StartTag as S};
    let expected: Vec<(XmlEvent, String, usize)> =
        [(S, "a", 1), (S, "b", 2), (S, "c", 3), (E, "c", 3), (E, "b", 2), (S, "d", 2), (E, "d", 2), (E, "a", 1)]
            .iter()
            .map(|(e, n, d)| (*e, n.to_string(), *d))
            .collect();
    assert_eq!(seen, expected);
    // Compose's forEachChildOf visits tags at every depth: an object animator inside an element a set doesn't
    // know is the set's child too.
    let res = Resources::new(&NoXmlResources);
    let mut p = XmlPullParser::new(
        read_document(r#"<set><wrapper><objectAnimator/></wrapper><objectAnimator/></set>"#).unwrap(),
    )
    .seek_to_start_tag()
    .unwrap();
    assert_eq!(parse_animator_set(&mut p, res).unwrap().animators.len(), 2);
}

#[test]
fn attribute_values_compile_as_aapt2_compiles_them() {
    let res = Resources::new(&NoXmlResources);
    let element = read_document(
        r##"<e xmlns:android="http://schemas.android.com/apk/res/android" android:valueFrom="#f00" android:valueTo="0x10"
        android:value="true" android:repeatCount="infinite" android:repeatMode="reverse" android:factor="12dp"
        android:duration="010" android:propertyName="1.5" android:fraction="50%"/>"##,
    )
    .unwrap();
    let p = XmlPullParser::new(element).seek_to_start_tag().unwrap();
    let a = res
        .obtain_attributes(
            &p,
            &[
                &attrs::VALUE_FROM,
                &attrs::VALUE_TO,
                &attrs::VALUE,
                &attrs::REPEAT_COUNT,
                &attrs::REPEAT_MODE,
                &attrs::VALUE_FROM,
                &attrs::DURATION,
                &attrs::PROPERTY_NAME,
                &attrs::INTERPOLATOR,
            ],
        )
        .unwrap();
    // A color: as an ARGB int, a float of it too (TypedArray.getFloat of an int).
    assert!(a.peek_value(0).unwrap().is_color() && a.get_color(0, 0).unwrap() as u32 == 0xFFFF0000);
    assert_eq!(a.get_float(0, 0.0).unwrap(), 0xFFFF0000u32 as i32 as f32);
    // Hex and decimal integers; a boolean (true: -1); enum names.
    assert_eq!(a.get_int(1, 0).unwrap(), 16);
    assert_eq!(a.get_int(2, 0).unwrap(), -1);
    assert!(a.get_int(3, 0).unwrap() == -1 && a.get_int(4, 0).unwrap() == 2);
    // A leading 0 is decimal to aapt2 (TypedArray holds 10).
    assert_eq!(a.get_int(6, 0).unwrap(), 10);
    // A string attribute keeps its text.
    assert_eq!(a.get_string(7).as_deref(), Some("1.5"));
    // Nothing there: the default, no resource.
    assert!(!a.has_value(8) && a.get_resource_id(8).is_none());
    // A float-only attribute refuses a dimension; a fraction isn't a float either.
    let bad = read_document(r#"<e xmlns:android="http://schemas.android.com/apk/res/android" android:factor="12dp"/>"#)
        .unwrap();
    let p = XmlPullParser::new(bad).seek_to_start_tag().unwrap();
    let e = res.obtain_attributes(&p, &[&attrs::FACTOR]).err().expect("incompatible");
    assert_eq!(e.message, "'12dp' is incompatible with attribute factor");
    // getColor of a float: Android's UnsupportedOperationException; getFloat of a dimension: NumberFormatException.
    let floats = read_document(
        r#"<e xmlns:android="http://schemas.android.com/apk/res/android" android:valueFrom="0.5" android:valueTo="3dp"/>"#,
    )
    .unwrap();
    let p = XmlPullParser::new(floats).seek_to_start_tag().unwrap();
    let a = res.obtain_attributes(&p, &[&attrs::VALUE_FROM, &attrs::VALUE_TO]).unwrap();
    assert_eq!(a.get_color(0, 0).unwrap_err().exception, "UnsupportedOperationException");
    assert_eq!(a.get_float(1, 0.0).unwrap_err().exception, "NumberFormatException");
    // A reference to the framework's interpolator: a file resource, not read until it is loaded.
    let r = read_document(
        r#"<e xmlns:android="http://schemas.android.com/apk/res/android" android:interpolator="@android:interpolator/linear"/>"#,
    )
    .unwrap();
    let p = XmlPullParser::new(r).seek_to_start_tag().unwrap();
    let a = res.obtain_attributes(&p, &[&attrs::INTERPOLATOR]).unwrap();
    assert!(matches!(a.get_resource_id(0), Some(Resource::Named(r)) if r.package.as_deref() == Some("android")));
    assert!(crate::vector::compat::resources::java_parse_float("1.5f").unwrap() == 1.5);
}

#[test]
fn holders_compose_doesnt_play_are_read() {
    let res = Resources::new(&NoXmlResources);
    let element = read_document(
        r#"<objectAnimator xmlns:android="http://schemas.android.com/apk/res/android" android:propertyXName="translateX"
        android:propertyYName="translateY" android:pathData="M0,0 L10,10" android:interpolator="@android:anim/linear_interpolator"/>"#,
    )
    .unwrap();
    let mut p = XmlPullParser::new(element).seek_to_start_tag().unwrap();
    let a = parse_object_animator(&mut p, res).unwrap();
    let PropertyValuesHolder::TwoD(h) = &a.holders[0] else { panic!("a 2D holder") };
    assert!(h.x_property_name == "translateX" && h.y_property_name == "translateY");
    assert_eq!(h.path_data, add_path_nodes("M0,0 L10,10"));
    assert_eq!(h.interpolator.transform(0.3), 0.3);
    let element = read_document(
        r#"<objectAnimator xmlns:android="http://schemas.android.com/apk/res/android" android:propertyName="x"
        android:valueType="intType" android:valueFrom="1" android:valueTo="7"/>"#,
    )
    .unwrap();
    let mut p = XmlPullParser::new(element).seek_to_start_tag().unwrap();
    let a = parse_object_animator(&mut p, res).unwrap();
    let PropertyValuesHolder::Int(h) = &a.holders[0] else { panic!("an Int holder") };
    assert_eq!(h.animator_keyframes.iter().map(|k| k.value).collect::<Vec<_>>(), [1, 7]);
}

fn path_holder(keyframes: &[(f32, &str)]) -> Arc<PropertyValuesHolder1D<Vec<PathNode>>> {
    let linear: Arc<dyn Easing> = Arc::new(LinearEasing);
    Arc::new(PropertyValuesHolder1D {
        property_name: "pathData".into(),
        animator_keyframes: keyframes
            .iter()
            .map(|(f, d)| Keyframe { fraction: *f, value: add_path_nodes(d), interpolator: linear.clone() })
            .collect(),
    })
}

#[test]
fn path_data_interpolates_as_compose_interpolates_it() {
    let holder = path_holder(&[(0.0, "M0,0"), (0.5, "M10,0"), (1.0, "M20,0")]);
    let once = |repeat_count, repeat_mode| {
        vec![PathTimestamp::of(&Timestamp {
            time_millis: 100,
            duration_millis: 100,
            repeat_count,
            repeat_mode,
            holder: holder.clone(),
        })]
    };
    let x = |nodes: Vec<PathNode>| match nodes[0] {
        PathNode::MoveTo { x, .. } => x,
        _ => f32::NAN,
    };
    let t = once(0, RepeatMode::Restart);
    assert_eq!(x(interpolate_path(&t, 50.0)), 0.0);
    assert_eq!(x(interpolate_path(&t, 175.0)), 15.0);
    // Past its end (its fraction past 1, no repeats): Compose's lookup finds no keyframe at or past it and takes
    // the first two (the middle keyframe's value), as Compose does.
    assert_eq!(x(interpolate_path(&t, 250.0)), 10.0);
    // Repeated back and forth: the second time backwards.
    let r = once(2, RepeatMode::Reverse);
    assert_eq!(x(interpolate_path(&r, 275.0)), 5.0);
    assert_eq!(x(interpolate_path(&r, 375.0)), 15.0);
    // So far past it that a float less one is itself: Compose's loop never ends; this one stops.
    assert!(!x(interpolate_path(&r, 3.0e9)).is_nan());
}

#[test]
fn durations_take_kotlins_int_arithmetic() {
    let animator = |duration, repeat_count| {
        Animator::Object(ObjectAnimator {
            duration,
            start_delay: 10,
            repeat_count,
            repeat_mode: RepeatMode::Restart,
            holders: Vec::new(),
        })
    };
    assert_eq!(animator(100, 2).total_duration(), 310);
    assert_eq!(animator(100, -1).total_duration(), i32::MAX);
    assert_eq!(animator(i32::MAX, 1).total_duration(), 10i32.wrapping_add(i32::MAX.wrapping_mul(2)));
}

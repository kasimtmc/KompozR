//! Animated vectors read from XML and seen as they play (`AnimatedImageVector::vector_at`: what the painter's
//! Transition gives each property): the C++ Kompoz's animated_vectors_play_their_animations (its data part) and
//! kompoz-graphics' stage-4 interpolator test, on Compose's model (where Compose's values differ from the C++
//! ones, Compose's are checked and the C++ ones named); Compose's timing (sets together and in sequence, start
//! delays, repeats, keyframes, path interpolators, color and path data holders), Android's interpolators, the
//! XML parsers over files modeled on Android's samples, resources from the caller, and what can't be read.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation_graphics::*;
use kompoz_graphics::*;
use std::collections::HashMap;
use std::f64::consts::PI;

// ---- Helpers ------------------------------------------------------------------------------------------------------

fn arc(a: f32, b: f32, theta: f32, large: bool, positive: bool, x: f32, y: f32) -> PathNode {
    PathNode::ArcTo {
        horizontal_ellipse_radius: a,
        vertical_ellipse_radius: b,
        theta,
        is_more_than_half: large,
        is_positive_arc: positive,
        arc_start_x: x,
        arc_start_y: y,
    }
}

fn same_rgb(a: Color, b: Color) -> bool {
    near(a.red(), b.red(), 0.01) && near(a.green(), b.green(), 0.01) && near(a.blue(), b.blue(), 0.01)
}

/// A vector's path with where its groups put it.
struct Placed {
    path: VectorPath,
    m: Matrix,
}

impl Placed {
    fn points(&self) -> Vec<Offset> {
        to_path(&self.path.path_data).commands().iter().filter_map(|c| c.end()).map(|p| self.m.map(p)).collect()
    }
    fn width(&self) -> f32 {
        let xs: Vec<f32> = self.points().iter().map(|p| p.x).collect();
        xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min)
    }
    fn fill(&self) -> Color {
        self.path.fill.as_ref().and_then(Brush::solid_color).expect("a solid fill")
    }
}

fn placed_in(g: &VectorGroup, outer: &Matrix, out: &mut Vec<Placed>) {
    let mut m = g.group_matrix();
    m.times_assign(outer);
    for n in g {
        match n {
            VectorNode::Path(p) => out.push(Placed { path: p.clone(), m }),
            VectorNode::Group(child) => placed_in(child, &m, out),
        }
    }
}

fn placed(v: &ImageVector) -> Vec<Placed> {
    let mut out = Vec::new();
    placed_in(v.root(), &Matrix::new(), &mut out);
    out
}

/// The first group named `name`.
fn group_named<'a>(g: &'a VectorGroup, name: &str) -> Option<&'a VectorGroup> {
    g.iter().filter_map(VectorNode::group).find_map(|c| if c.name == name { Some(c) } else { group_named(c, name) })
}

/// The first path named `name`.
fn path_named<'a>(g: &'a VectorGroup, name: &str) -> Option<&'a VectorPath> {
    g.iter().find_map(|n| match n {
        VectorNode::Path(p) if p.name == name => Some(p),
        VectorNode::Group(c) => path_named(c, name),
        _ => None,
    })
}

/// An `<animated-vector>` of a 24×24 drawable of `drawable` (its groups and paths) and `targets` (each a name and
/// its animation's element, inline).
fn avd_xml(drawable: &str, targets: &[(&str, &str)]) -> String {
    let mut s = String::from(
        r#"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt"><aapt:attr name="android:drawable"><vector android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24">"#,
    );
    s.push_str(drawable);
    s.push_str("</vector></aapt:attr>");
    for (name, animation) in targets {
        s.push_str(&format!(
            r#"<target android:name="{name}"><aapt:attr name="android:animation">{animation}</aapt:attr></target>"#
        ));
    }
    s.push_str("</animated-vector>");
    s
}

fn read(xml: &str) -> AnimatedImageVector {
    AnimatedImageVector::from_animated_vector_xml(xml).unwrap_or_else(|e| panic!("{e}"))
}

/// A group "g" with a path, its translateX animated by `animation`.
fn translate_x(animation: &str) -> AnimatedImageVector {
    read(&avd_xml(
        r##"<group android:name="g"><path android:fillColor="#000" android:pathData="M0,0h1v1z"/></group>"##,
        &[("g", animation)],
    ))
}

fn translate_x_at(avd: &AnimatedImageVector, t: f32) -> f32 {
    group_named(avd.vector_at(t).root(), "g").unwrap().translation_x
}

/// The message of the panic `f` panics with.
fn panic_message(f: impl FnOnce()) -> String {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(hook);
    let e = r.expect_err("a panic");
    e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
}

// ---- The C++ Kompoz's animated_vectors_play_their_animations (its data part) --------------------------------------

/// The C++ test's animated vector, as it was: its last target animates the vector's own `alpha` (a property of
/// Android's that Compose's StateVectorConfig hasn't: "Unknown propertyName").
const ANIMATED_CPP: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt">
    <aapt:attr name="android:drawable">
        <vector android:name="root" android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24">
            <group android:name="spin" android:pivotX="12" android:pivotY="12">
                <path android:name="dot" android:strokeColor="#000000" android:strokeWidth="1" android:pathData="M12,2L12,6"/>
            </group>
            <path android:name="bar" android:fillColor="#FF0000" android:pathData="M0,0h4v4h-4z"/>
            <path android:name="line" android:strokeColor="#000000" android:strokeWidth="1" android:pathData="M0,20h24"/>
        </vector>
    </aapt:attr>
    <target android:name="spin">
        <aapt:attr name="android:animation">
            <set>
                <objectAnimator android:propertyName="rotation" android:valueFrom="90" android:valueTo="180" android:startOffset="100"
                    android:duration="200" android:interpolator="@android:interpolator/linear"/>
                <objectAnimator android:propertyName="rotation" android:valueFrom="0" android:valueTo="90" android:duration="100"
                    android:interpolator="@android:interpolator/linear"/>
            </set>
        </aapt:attr>
    </target>
    <target android:name="bar">
        <aapt:attr name="android:animation">
            <set android:ordering="sequentially">
                <objectAnimator android:propertyName="fillColor" android:valueFrom="#FF0000" android:valueTo="#0000FF" android:duration="100"
                    android:interpolator="@android:interpolator/linear"/>
                <objectAnimator android:propertyName="pathData" android:valueType="pathType" android:valueFrom="M0,0h4v4h-4z"
                    android:valueTo="M0,0h8v8h-8z" android:duration="100" android:interpolator="@android:anim/linear_interpolator"/>
            </set>
        </aapt:attr>
    </target>
    <target android:name="line">
        <aapt:attr name="android:animation">
            <objectAnimator android:duration="60" android:startOffset="100" android:repeatCount="1" android:repeatMode="reverse"
                android:interpolator="@android:interpolator/linear">
                <propertyValuesHolder android:propertyName="trimPathEnd" android:valueFrom="0.8" android:valueTo="0"/>
                <propertyValuesHolder android:propertyName="strokeWidth" android:valueTo="3"/>
            </objectAnimator>
        </aapt:attr>
    </target>
    <target android:name="root">
        <aapt:attr name="android:animation">
            <objectAnimator android:propertyName="alpha" android:valueFrom="1" android:valueTo="0.5" android:duration="200"
                android:interpolator="@android:interpolator/accelerate_quad"/>
        </aapt:attr>
    </target>
</animated-vector>"##;

fn animated_vectors_play_their_animations() {
    let avd = read(ANIMATED);
    // The group's: its longer animator (together), 100 in and 200 long.
    assert_eq!(avd.total_duration(), 300);
    let at = |t: f32| placed(&avd.vector_at(t));
    // As it starts: the drawable as it is, but for what Compose starts at the first keyframe of each animated
    // property: the line's trimPathEnd at its animator's valueFrom (0.8) though its animator starts at 100, and its
    // strokeWidth at its only keyframe (3: valueTo without valueFrom; C++ and Android: the path's own, 1).
    let start = at(0.0);
    assert_eq!(start.len(), 3);
    assert!(same_rgb(start[1].fill(), Color::hex(0xFF0000)) && near(start[1].width(), 4.0, 1e-4));
    assert!(near(start[2].path.trim_path_end, 0.8, 1e-6) && start[2].path.stroke_line_width == 3.0);
    let own = placed(&avd.image_vector());
    assert!(start[0].path == own[0].path && start[0].m == own[0].m && start[1].path.path_data == own[1].path.path_data);
    // Its group turning (linear): 45° at 50; from 100 its second animator (started later, over the first): 112.5°
    // at 150.
    let (dot, later) = (at(50.0)[0].points()[0], at(150.0)[0].points()[0]);
    assert!(near(dot.x, 19.07, 0.05) && near(dot.y, 4.93, 0.05), "{dot:?}");
    assert!(near(later.x, 21.24, 0.05) && near(later.y, 15.83, 0.05), "{later:?}");
    // One after the other: the color first (halfway at 50, in Oklab as Compose animates it: ColorTest's red to blue
    // at 0.5; done at 100), then the shape (halfway at 150).
    assert!(same_rgb(at(50.0)[1].fill(), Color::hex(0x8C53A2)) && same_rgb(at(100.0)[1].fill(), Color::hex(0x0000FF)));
    assert!(near(at(100.0)[1].width(), 4.0, 0.01) && near(at(150.0)[1].width(), 6.0, 0.01));
    assert!(near(at(220.0)[1].width(), 8.0, 0.01));
    // From 100, there and back (repeatMode reverse). Before it starts: its first keyframe (C++: its own value, 1).
    // Once its animation is done (220), the Transition's animation is at its target: the last keyframe, 0 (Compose's
    // TargetBasedAnimation; C++ held where the reverse ends, 0.8). The holder without valueFrom: 3 all along (C++:
    // 1 → 3 → 1, 2 at 130).
    let line = |t: f32| at(t)[2].path.clone();
    assert!(near(line(99.0).trim_path_end, 0.8, 0.001) && near(line(115.0).trim_path_end, 0.6, 0.001));
    assert!(near(line(130.0).trim_path_end, 0.4, 0.001) && near(line(130.0).stroke_line_width, 3.0, 0.001));
    assert_near(line(175.0).trim_path_end, 0.2, 0.001);
    assert_near(line(219.0).trim_path_end, 0.8 * 59.0 / 60.0, 0.001);
    assert_near(line(230.0).trim_path_end, 0.0, 0.001);
    assert!(near(line(300.0).trim_path_end, 0.0, 0.001) && near(line(300.0).stroke_line_width, 3.0, 0.001));
    // The bar's fillAlpha (its second target, merged), accelerating (t²): a quarter of the way at the halfway mark
    // (C++: the vector's alpha, the same numbers on the bar).
    assert!(near(at(100.0)[1].path.fill_alpha, 0.875, 0.005) && near(at(200.0)[1].path.fill_alpha, 0.5, 0.005));
    assert!(AnimatedImageVector::from_animated_vector_xml("<vector/>").is_err());
    assert!(AnimatedImageVector::from_animated_vector_xml("not xml").is_err());
    // An arc morphed: its numbers in between; its flags the start's all along (Compose's lerp keeps them; C++ and
    // Android interpolate them as numbers: set at 50, the end's at 100).
    let arcs = read(
        r##"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android"
        xmlns:aapt="http://schemas.android.com/aapt"><aapt:attr name="android:drawable"><vector android:width="24dp" android:height="24dp"
        android:viewportWidth="24" android:viewportHeight="24"><path android:name="yay" android:strokeColor="#000" android:pathData="M0,0A5,5 0 0,1 10,0"/>
        </vector></aapt:attr><target android:name="yay"><aapt:attr name="android:animation"><objectAnimator android:propertyName="pathData"
        android:valueType="pathType" android:valueFrom="M0,0A5,5 0 0,1 10,0" android:valueTo="M0,0A5,8 0 1,0 10,0" android:duration="100"
        android:interpolator="@android:interpolator/linear"/></aapt:attr></target></animated-vector>"##,
    );
    let arc_at = |t: f32| arcs.vector_at(t).root()[0].path().unwrap().path_data[1];
    assert_eq!(arc_at(0.0), arc(5.0, 5.0, 0.0, false, true, 10.0, 0.0));
    assert_eq!(arc_at(50.0), arc(5.0, 6.5, 0.0, false, true, 10.0, 0.0));
    assert_eq!(arc_at(100.0), arc(5.0, 8.0, 0.0, false, true, 10.0, 0.0));
    // A vector drawable read the same way, whether in an animated one or not.
    assert_eq!(ImageVector::from_vector_xml(ANIMATED).unwrap(), avd.image_vector());
    // The same one; another read of it: another.
    assert!(avd.clone() == avd && read(ANIMATED) != avd);
}

/// The C++ test's vector as it was: read, but its `alpha` target can't be played (Compose's IllegalStateException
/// where its config is made).
fn the_vectors_own_alpha_isnt_a_property_compose_plays() {
    let avd = read(ANIMATED_CPP);
    assert_eq!(avd.total_duration(), 300);
    let message = panic_message(move || {
        avd.vector_at(50.0);
    });
    assert_eq!(message, "IllegalStateException: Unknown propertyName: alpha");
}

// ---- kompoz-graphics' animated_vectors_play_androids_interpolators -------------------------------------------------

/// One `<objectAnimator>` of `attributes` on a path's fillAlpha (0 to 1, 100 long unless they say).
fn fill_alpha_xml(attributes: &str) -> String {
    format!(
        r##"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android"
        xmlns:aapt="http://schemas.android.com/aapt"><aapt:attr name="android:drawable"><vector android:width="24dp"
        android:height="24dp" android:viewportWidth="24" android:viewportHeight="24"><path android:name="p" android:fillColor="#000"
        android:fillAlpha="0.5" android:pathData="M0,0h1v1z"/></vector></aapt:attr><target android:name="p"><aapt:attr
        name="android:animation"><objectAnimator android:propertyName="fillAlpha" android:valueFrom="0" android:valueTo="1"
        {attributes}/></aapt:attr></target></animated-vector>"##
    )
}

fn fill_alpha_animated(attributes: &str) -> AnimatedImageVector {
    read(&fill_alpha_xml(attributes))
}

fn fill_alpha_at(avd: &AnimatedImageVector, t: f32) -> f32 {
    avd.vector_at(t).root()[0].path().unwrap().fill_alpha
}

/// y of Compose's CubicBezierEasing's curve solved by halving.
fn bezier_at(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let curve =
        |p1: f64, p2: f64, t: f64| 3.0 * (1.0 - t) * (1.0 - t) * t * p1 + 3.0 * (1.0 - t) * t * t * p2 + t * t * t;
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if curve(x1 as f64, x2 as f64, mid) < x as f64 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    curve(y1 as f64, y2 as f64, (lo + hi) / 2.0) as f32
}

fn animated_vectors_play_androids_interpolators() {
    // Android's default (no interpolator): accelerate_decelerate, cos((t + 1)π) / 2 + 0.5; 300 long.
    let default = fill_alpha_animated("");
    assert_eq!(default.total_duration(), 300);
    assert_near(fill_alpha_at(&default, 75.0), 0.146_446_6, 1e-5);
    assert_near(fill_alpha_at(&default, 150.0), 0.5, 1e-5);
    let named = |name: &str, t: f32| {
        fill_alpha_at(&fill_alpha_animated(&format!(r#"android:duration="100" android:interpolator="{name}""#)), t)
    };
    assert_near(named("@android:anim/accelerate_decelerate_interpolator", 25.0), 0.146_446_6, 1e-5);
    assert_near(named("@android:anim/linear_interpolator", 30.0), 0.3, 1e-6);
    assert_near(named("@android:anim/accelerate_interpolator", 30.0), 0.09, 1e-6);
    assert_near(named("@android:interpolator/decelerate_quad", 30.0), 0.51, 1e-6);
    // One the framework hasn't: not found (C++: the default).
    let unknown = AnimatedImageVector::from_animated_vector_xml(&fill_alpha_xml(
        r#"android:interpolator="@android:interpolator/unknown""#,
    ));
    assert_eq!(unknown.unwrap_err().exception, "NotFoundException");
    // Compose's cubic easings: the curve's y where its x is the fraction.
    for (name, (x1, y1, x2, y2)) in [
        ("@android:interpolator/fast_out_slow_in", (0.4, 0.0, 0.2, 1.0)),
        ("@android:interpolator/fast_out_linear_in", (0.4, 0.0, 1.0, 1.0)),
        ("@android:interpolator/linear_out_slow_in", (0.0, 0.0, 0.2, 1.0)),
    ] {
        for t in [10.0, 25.0, 50.0, 75.0, 90.0] {
            assert!(near(named(name, t), bezier_at(x1, y1, x2, y2, t / 100.0), 1e-4), "{name} at {t}");
        }
        assert!(named(name, 0.0) == 0.0 && named(name, 100.0) == 1.0);
    }
    // Repeated (restarting) as many times again as it says, after a start offset; before it starts, its first
    // keyframe (C++: its own value, 0.5).
    let repeated = fill_alpha_animated(
        r#"android:duration="100" android:startOffset="50" android:repeatCount="2" android:interpolator="@android:interpolator/linear""#,
    );
    assert_eq!(repeated.total_duration(), 350);
    assert_near(fill_alpha_at(&repeated, 0.0), 0.0, 1e-6);
    assert_near(fill_alpha_at(&repeated, 75.0), 0.25, 1e-6);
    assert_near(fill_alpha_at(&repeated, 175.0), 0.25, 1e-6);
    assert_near(fill_alpha_at(&repeated, 400.0), 1.0, 1e-6);
    // An infinite repeat: Int.MAX_VALUE long, repeating (C++: played once).
    let infinite = fill_alpha_animated(
        r#"android:duration="100" android:repeatCount="infinite" android:interpolator="@android:interpolator/linear""#,
    );
    assert_eq!(infinite.total_duration(), i32::MAX);
    assert_near(fill_alpha_at(&infinite, 1_030.0), 0.3, 1e-4);
    assert_eq!(fill_alpha_animated(r#"android:duration="100" android:repeatCount="-1""#).total_duration(), i32::MAX);
    // No duration: at its end at once.
    let at_once = fill_alpha_animated(r#"android:duration="0""#);
    assert!(at_once.total_duration() == 0 && near(fill_alpha_at(&at_once, 0.0), 1.0, 1e-6));
}

// ---- Compose's timing --------------------------------------------------------------------------------------------

fn sets_play_together_and_one_after_another() {
    // In sequence, each after the whole of the one before (its start offset and repeats too): the first 50 in, 100
    // long; the second at 150.
    let seq = translate_x(
        r#"<set android:ordering="sequentially">
            <objectAnimator android:propertyName="translateX" android:valueFrom="0" android:valueTo="10" android:startOffset="50"
                android:duration="100" android:interpolator="@android:interpolator/linear"/>
            <objectAnimator android:propertyName="translateX" android:valueFrom="10" android:valueTo="30"
                android:duration="100" android:interpolator="@android:interpolator/linear"/>
        </set>"#,
    );
    assert_eq!(seq.total_duration(), 250);
    assert_near(translate_x_at(&seq, 25.0), 0.0, 1e-5); // (before the first: its first keyframe)
    assert_near(translate_x_at(&seq, 100.0), 5.0, 1e-4);
    assert_near(translate_x_at(&seq, 150.0), 10.0, 1e-4);
    assert_near(translate_x_at(&seq, 175.0), 15.0, 1e-4);
    assert_near(translate_x_at(&seq, 300.0), 30.0, 1e-4);
    // Together in a set in sequence: the inner set as long as its longest; a start offset on the inner set's
    // children counts from the set's start.
    let nested = translate_x(
        r#"<set android:ordering="sequentially">
            <set>
                <objectAnimator android:propertyName="translateY" android:valueFrom="0" android:valueTo="1" android:duration="40"/>
                <objectAnimator android:propertyName="translateY" android:valueFrom="1" android:valueTo="2" android:startOffset="20" android:duration="60"/>
            </set>
            <objectAnimator android:propertyName="translateX" android:valueFrom="0" android:valueTo="8" android:startOffset="20"
                android:duration="80" android:interpolator="@android:interpolator/linear"/>
        </set>"#,
    );
    assert_eq!(nested.total_duration(), 80 + 100);
    assert_near(translate_x_at(&nested, 100.0), 0.0, 1e-5);
    assert_near(translate_x_at(&nested, 140.0), 4.0, 1e-4);
    assert_near(translate_x_at(&nested, 180.0), 8.0, 1e-4);
    // translateY: the later-starting one (20) over the first from 20 on, accelerate-decelerating (the default).
    let y = |t: f32| group_named(nested.vector_at(t).root(), "g").unwrap().translation_y;
    let ad = |f: f64| ((f + 1.0) * PI).cos() / 2.0 + 0.5;
    assert_near(y(10.0), ad(0.25) as f32, 1e-5);
    assert_near(y(50.0), 1.0 + ad(0.5) as f32, 1e-5);
}

fn repeats_restart_and_reverse() {
    let anim = |mode: &str| {
        translate_x(&format!(
            r#"<objectAnimator android:propertyName="translateX" android:valueFrom="0" android:valueTo="100" android:duration="100"
                android:repeatCount="2" android:repeatMode="{mode}" android:interpolator="@android:interpolator/linear"/>"#
        ))
    };
    let (restart, reverse) = (anim("restart"), anim("reverse"));
    assert!(restart.total_duration() == 300 && reverse.total_duration() == 300);
    assert_near(translate_x_at(&restart, 150.0), 50.0, 1e-3);
    assert_near(translate_x_at(&restart, 230.0), 30.0, 1e-3);
    assert_near(translate_x_at(&reverse, 130.0), 70.0, 1e-3);
    assert_near(translate_x_at(&reverse, 230.0), 30.0, 1e-3);
    // After an odd number of reverses: at the end (the target: the last keyframe).
    assert_near(translate_x_at(&reverse, 300.0), 100.0, 1e-3);
}

fn keyframes_ease_as_compose_eases_them() {
    // Compose's keyframes: a keyframe's interpolator eases the stretch from it to the next (Android's: the stretch
    // to it). Here 0 at 0 (the holder's linear), 40 at 100 (accelerate_quad) and 100 at 400.
    let k = translate_x(
        r#"<objectAnimator android:duration="400" android:interpolator="@android:interpolator/linear">
            <propertyValuesHolder android:propertyName="translateX">
                <keyframe android:fraction="0" android:value="0"/>
                <keyframe android:fraction="0.25" android:value="40" android:interpolator="@android:interpolator/accelerate_quad"/>
                <keyframe android:fraction="1" android:value="100"/>
            </propertyValuesHolder>
        </objectAnimator>"#,
    );
    assert_eq!(k.total_duration(), 400);
    assert_near(translate_x_at(&k, 50.0), 20.0, 1e-3);
    assert_near(translate_x_at(&k, 100.0), 40.0, 1e-3);
    assert_near(translate_x_at(&k, 250.0), 55.0, 1e-3); // (Android's: 70)
    // Keyframes out of order are sorted; a fraction's time truncated to a millisecond (0.333 of 100: 33).
    let unsorted = translate_x(
        r#"<objectAnimator android:duration="100" android:interpolator="@android:interpolator/linear">
            <propertyValuesHolder android:propertyName="translateX">
                <keyframe android:fraction="1" android:value="10"/>
                <keyframe android:fraction="0" android:value="0"/>
                <keyframe android:fraction="0.333" android:value="3.3"/>
            </propertyValuesHolder>
        </objectAnimator>"#,
    );
    assert_near(translate_x_at(&unsorted, 33.0), 3.3, 1e-4);
    assert_near(translate_x_at(&unsorted, 66.0), 3.3 + 6.7 * (33.0 / 67.0), 1e-4);
    // (Compose's keyframes read whole milliseconds: 66.9 is 66)
    assert_eq!(translate_x_at(&unsorted, 66.9), translate_x_at(&unsorted, 66.0));
}

fn path_interpolators_follow_their_paths() {
    let with = |interpolator: &str| {
        translate_x(&format!(
            r#"<objectAnimator android:propertyName="translateX" android:valueFrom="0" android:valueTo="100" android:duration="100">
                <aapt:attr name="android:interpolator">{interpolator}</aapt:attr>
            </objectAnimator>"#
        ))
    };
    // Lines: exactly them.
    let lines = with(r#"<pathInterpolator android:pathData="M0,0 L0.5,0.2 L1,1"/>"#);
    assert_near(translate_x_at(&lines, 25.0), 10.0, 1e-3);
    assert_near(translate_x_at(&lines, 75.0), 60.0, 1e-3);
    // A quadratic (controlX1/Y1 alone): Android's approximation, within its precision of the curve.
    let quad = with(r#"<pathInterpolator android:controlX1="0.8" android:controlY1="0.1"/>"#);
    for t in [10.0f32, 30.0, 50.0, 70.0, 90.0] {
        // (x(s) = 2(1-s)s·0.8 + s², y(s) = 2(1-s)s·0.1 + s²: s solved by halving)
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        for _ in 0..60 {
            let s = (lo + hi) / 2.0;
            if 2.0 * (1.0 - s) * s * 0.8 + s * s < (t / 100.0) as f64 { lo = s } else { hi = s }
        }
        let s = (lo + hi) / 2.0;
        let y = 2.0 * (1.0 - s) * s * 0.1 + s * s;
        assert_near(translate_x_at(&quad, t), (y * 100.0) as f32, 0.3);
    }
    // Four controls: Compose's CubicBezierEasing.
    let cubic = with(
        r#"<pathInterpolator android:controlX1="0.3" android:controlY1="0" android:controlX2="0.1" android:controlY2="1"/>"#,
    );
    assert_near(translate_x_at(&cubic, 40.0), bezier_at(0.3, 0.0, 0.1, 1.0, 0.4) * 100.0, 1e-3);
    // The framework's fast_out_extra_slow_in (two cubics in path data), through Android's approximation.
    let extra = translate_x(
        r#"<objectAnimator android:propertyName="translateX" android:valueFrom="0" android:valueTo="100" android:duration="100"
            android:interpolator="@android:interpolator/fast_out_extra_slow_in"/>"#,
    );
    let first = bezier_at(0.05 / 0.166666, 0.0 / 0.4, 0.133333 / 0.166666, 0.06 / 0.4, 0.1 / 0.166666) * 0.4;
    assert_near(translate_x_at(&extra, 10.0), first * 100.0, 0.3);
    assert_near(translate_x_at(&extra, 100.0), 100.0, 1e-4);
    // Paths that aren't one (Android's IllegalArgumentException).
    let bad = |interpolator: &str| {
        AnimatedImageVector::from_animated_vector_xml(&avd_xml(r#"<group android:name="g"/>"#, &[(
            "g",
            &format!(
                r#"<objectAnimator android:propertyName="translateX" android:valueTo="1"><aapt:attr name="android:interpolator">{interpolator}</aapt:attr></objectAnimator>"#
            ),
        )]))
        .unwrap_err()
    };
    let e = bad(r#"<pathInterpolator android:pathData="M0,0 L1,0.5"/>"#);
    assert!(e.exception == "IllegalArgumentException" && e.message == "The Path must start at (0,0) and end at (1,1)");
    let e = bad(r#"<pathInterpolator android:pathData="M0,0 L0.6,0.5 L0.4,0.6 L1,1"/>"#);
    assert_eq!(e.message, "The Path cannot loop back on itself.");
    let e = bad(r#"<pathInterpolator android:pathData="M0,0 L0.5,0.5 M0.6,0.5 L1,1"/>"#);
    assert_eq!(e.message, "The Path cannot have discontinuity in the X axis.");
}

fn androids_interpolators_are_their_easings() {
    let at = |interpolator: &str, t: f32| {
        translate_x_at(
            &translate_x(&format!(
                r#"<objectAnimator android:propertyName="translateX" android:valueFrom="0" android:valueTo="1" android:duration="1000">
                    <aapt:attr name="android:interpolator">{interpolator}</aapt:attr>
                </objectAnimator>"#
            )),
            t * 1000.0,
        )
    };
    let x = 0.3f32;
    // Android's formulas (android.view.animation's Java).
    assert_near(at("<accelerateInterpolator/>", x), x * x, 1e-5);
    assert_near(at(r#"<accelerateInterpolator android:factor="1.5"/>"#, x), x.powf(3.0), 1e-5);
    assert_near(at(r#"<decelerateInterpolator android:factor="2.5"/>"#, x), 1.0 - (1.0 - x).powf(5.0), 1e-5);
    assert_near(at("<linearInterpolator/>", x), x, 1e-6);
    assert_near(at(r#"<cycleInterpolator android:cycles="0.5"/>"#, x), (PI * x as f64).sin() as f32, 1e-5);
    let anticipate = |t: f32, s: f32| t * t * ((s + 1.0) * t - s);
    assert_near(at("<anticipateInterpolator/>", x), anticipate(x, 2.0), 1e-5);
    assert_near(at(r#"<anticipateInterpolator android:tension="4"/>"#, x), anticipate(x, 4.0), 1e-5);
    let overshoot = |t: f32, s: f32| (t - 1.0) * (t - 1.0) * ((s + 1.0) * (t - 1.0) + s) + 1.0;
    assert_near(at("<overshootInterpolator/>", 0.8), overshoot(0.8, 2.0), 1e-5);
    // AnticipateOvershoot: tension × extraTension (2 × 1.5 by default).
    let o = |t: f32, s: f32| t * t * ((s + 1.0) * t + s);
    assert_near(at("<anticipateOvershootInterpolator/>", 0.2), 0.5 * anticipate(0.4, 3.0), 1e-5);
    assert_near(
        at(r#"<anticipateOvershootInterpolator android:tension="1" android:extraTension="2"/>"#, 0.8),
        0.5 * (o(0.8 * 2.0 - 2.0, 2.0) + 2.0),
        1e-5,
    );
    let bounce = |t: f32| t * t * 8.0;
    assert_near(at("<bounceInterpolator/>", 0.5), bounce(0.5 * 1.1226 - 0.54719) + 0.7, 1e-5);
    assert_near(at("<accelerateDecelerateInterpolator/>", x), (((x + 1.0) as f64 * PI).cos() / 2.0 + 0.5) as f32, 1e-5);
    // The framework's by name: their files (accelerate_cubic: factor 1.5).
    let named = |name: &str, t: f32| {
        translate_x_at(
            &translate_x(&format!(
                r#"<objectAnimator android:propertyName="translateX" android:valueFrom="0" android:valueTo="1" android:duration="1000"
                    android:interpolator="{name}"/>"#
            )),
            t * 1000.0,
        )
    };
    assert_near(named("@android:interpolator/accelerate_cubic", x), x.powf(3.0), 1e-5);
    assert_near(named("@android:interpolator/decelerate_quint", x), 1.0 - (1.0 - x).powf(5.0), 1e-5);
    assert_near(named("@android:anim/overshoot_interpolator", 0.8), overshoot(0.8, 2.0), 1e-5);
    assert_near(named("@android:anim/cycle_interpolator", 0.2), (2.0 * PI * 0.2).sin() as f32, 1e-5);
    assert_near(named("@*android:interpolator/bounce", 0.5), bounce(0.5 * 1.1226 - 0.54719) + 0.7, 1e-5);
    // An element that isn't one: Compose's "Unknown interpolator".
    let e = AnimatedImageVector::from_animated_vector_xml(&avd_xml(r#"<group android:name="g"/>"#, &[(
        "g",
        r#"<objectAnimator android:propertyName="translateX" android:valueTo="1"><aapt:attr name="android:interpolator"><springInterpolator/></aapt:attr></objectAnimator>"#,
    )]))
    .unwrap_err();
    assert!(e.exception == "RuntimeException" && e.message == "Unknown interpolator: springInterpolator");
}

fn colors_and_path_data_morph() {
    let drawable = r##"<path android:name="p" android:fillColor="#000" android:strokeColor="#000" android:pathData="M0,0 L10,0 L10,10z"/>"##;
    let avd = read(&avd_xml(
        drawable,
        &[(
            "p",
            r##"<objectAnimator android:duration="100" android:interpolator="@android:interpolator/linear">
            <propertyValuesHolder android:propertyName="fillColor" android:valueType="colorType">
                <keyframe android:fraction="0" android:value="#ff0000"/>
                <keyframe android:fraction="1" android:value="#0000ff"/>
            </propertyValuesHolder>
            <propertyValuesHolder android:propertyName="strokeColor" android:valueFrom="#f00" android:valueTo="#80ff0000"/>
            <propertyValuesHolder android:propertyName="pathData" android:valueType="pathType">
                <keyframe android:fraction="0" android:value="M0,0 L10,0 L10,10z"/>
                <keyframe android:fraction="0.5" android:value="M0,0 L20,0 L20,20z"/>
                <keyframe android:fraction="1" android:value="M10,10 L20,0 L20,20z"/>
            </propertyValuesHolder>
        </objectAnimator>"##,
        )],
    ));
    let p = |t: f32| path_named(avd.vector_at(t).root(), "p").unwrap().clone();
    // Colors in Oklab (their type from valueType; or from their values: both colors).
    assert!(same_rgb(p(50.0).fill.unwrap().solid_color().unwrap(), Color::hex(0x8C53A2)));
    let stroke = p(50.0).stroke.unwrap().solid_color().unwrap();
    assert!(same_rgb(stroke, Color::hex(0xFF0000)) && near(stroke.alpha(), 0.75, 0.01), "{stroke:?}");
    // Path data between the keyframes around the time (by the later one's interpolator): halfway to the middle
    // one at 25, from it to the last at 75.
    assert_eq!(p(25.0).path_data, add_path_nodes("M0,0 L15,0 L15,15z"));
    assert_eq!(p(75.0).path_data, add_path_nodes("M5,5 L20,0 L20,20z"));
    // Holders' types: a valueTo alone is a float though it is a color (Compose's: only both colors make one), so
    // a color property of it fails where it is played (Kotlin's cast).
    let valued = read(&avd_xml(
        drawable,
        &[(
            "p",
            r##"<objectAnimator android:propertyName="fillColor" android:valueTo="#0000ff" android:duration="100"/>"##,
        )],
    ));
    let message = panic_message(move || {
        valued.vector_at(50.0);
    });
    assert!(message.starts_with("ClassCastException"), "{message}");
    // Path data of different commands can't be morphed (Compose's `require`).
    let unlike = read(&avd_xml(
        drawable,
        &[(
            "p",
            r#"<objectAnimator android:propertyName="pathData" android:valueType="pathType" android:valueFrom="M0,0 L1,1" android:valueTo="M0,0 Q1,1 2,2"/>"#,
        )],
    ));
    let message = panic_message(move || {
        unlike.vector_at(150.0);
    });
    assert_eq!(message, "IllegalArgumentException: start and stop path nodes have different types");
}

fn two_dimensional_holders_are_read_not_played() {
    // A path's two properties (pathData, propertyXName/YName): read, and not played (Compose's TODO).
    let avd = translate_x(
        r#"<objectAnimator android:propertyXName="translateX" android:propertyYName="translateY" android:pathData="M0,0 L10,10" android:duration="100"/>"#,
    );
    assert_eq!(avd.total_duration(), 100);
    assert_eq!(translate_x_at(&avd, 50.0), 0.0);
    // Without its properties' names: Kotlin's `!!`.
    let e = AnimatedImageVector::from_animated_vector_xml(&avd_xml(
        r#"<group android:name="g"/>"#,
        &[("g", r#"<objectAnimator android:pathData="M0,0 L10,10"/>"#)],
    ))
    .unwrap_err();
    assert_eq!(e.exception, "NullPointerException");
}

fn infinite_repeats_take_kotlins_int() {
    // An infinite repeat is Int.MAX_VALUE long; one after it in a sequence starts past Int's end, wrapped (Kotlin's
    // Int arithmetic, as Compose sums them).
    let avd = translate_x(
        r#"<set android:ordering="sequentially">
            <objectAnimator android:propertyName="translateY" android:valueTo="1" android:repeatCount="infinite"/>
            <objectAnimator android:propertyName="translateX" android:valueTo="1" android:duration="100"/>
        </set>"#,
    );
    assert_eq!(avd.total_duration(), i32::MAX.wrapping_add(100));
}

// ---- XML read as Android reads it -----------------------------------------------------------------------------------

/// Android's AnimatedVectorDrawable sample shape (a drawable and animators in files of their own, an interpolator
/// resource, colors and a theme attribute).
const SEARCH_AVD: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<!-- Modeled on Android's AnimatedVectorDrawable sample (res/drawable/avd.xml) -->
<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" android:drawable="@drawable/vd_search">
    <target android:name="rotationGroup" android:animation="@animator/rotation"/>
    <target android:name="v" android:animation="@animator/path_morph"/>
</animated-vector>"#;

fn resources(r: &ResourceReference) -> Option<String> {
    let files: HashMap<(&str, &str), &str> = HashMap::from([
        (
            ("drawable", "vd_search"),
            r#"<vector xmlns:android="http://schemas.android.com/apk/res/android" android:height="64dp" android:width="64dp"
                android:viewportHeight="600" android:viewportWidth="600">
                <group android:name="rotationGroup" android:pivotX="300.0" android:pivotY="300.0" android:rotation="45.0">
                    <path android:name="v" android:fillColor="@color/brand" android:pathData="M300,70 l 0,-70 70,70 0,0 -70,70z"/>
                </group>
            </vector>"#,
        ),
        (
            ("animator", "rotation"),
            r#"<objectAnimator xmlns:android="http://schemas.android.com/apk/res/android" android:duration="@integer/turn"
                android:propertyName="rotation" android:valueFrom="0" android:valueTo="360"
                android:interpolator="@interpolator/ease"/>"#,
        ),
        (
            ("animator", "path_morph"),
            r#"<set xmlns:android="http://schemas.android.com/apk/res/android">
                <objectAnimator android:duration="3000" android:propertyName="pathData"
                    android:valueFrom="M300,70 l 0,-70 70,70 0,0   -70,70z" android:valueTo="M300,70 l 0,-70 70,0  0,140 -70,0 z"
                    android:valueType="pathType"/>
                <objectAnimator android:duration="3000" android:propertyName="fillColor" android:valueFrom="?attr/colorPrimary"
                    android:valueTo="@color/accent" android:valueType="colorType"/>
            </set>"#,
        ),
        (
            ("interpolator", "ease"),
            r#"<pathInterpolator xmlns:android="http://schemas.android.com/apk/res/android" android:pathData="M0,0 L0.5,0.8 L1,1"/>"#,
        ),
        (("integer", "turn"), "6000"),
        (("color", "brand"), "#FF3366CC"),
        (("color", "accent"), "@color/accent_base"),
        (("color", "accent_base"), "#00FF00"),
        (("attr", "colorPrimary"), "@color/primary"),
        (
            ("color", "primary"),
            r##"<selector xmlns:android="http://schemas.android.com/apk/res/android">
                <item android:state_enabled="false" android:color="#888888"/>
                <item android:color="#FF0000" android:alpha="0.5"/>
            </selector>"##,
        ),
    ]);
    files.get(&(r.r#type.as_str(), r.name.as_str())).map(|s| s.to_string())
}

fn resources_come_from_the_caller() {
    let avd =
        AnimatedImageVector::from_animated_vector_xml_with(SEARCH_AVD, &resources).unwrap_or_else(|e| panic!("{e}"));
    // (6000 from @integer/turn; the morph 3000)
    assert_eq!(avd.total_duration(), 6000);
    let v = avd.image_vector();
    assert!(v.viewport_width() == 600.0 && v.default_width() == Dp(64.0));
    // The drawable's color reference resolved.
    let own = path_named(v.root(), "v").unwrap();
    assert_eq!(own.fill.as_ref().and_then(Brush::solid_color), Some(Color::from_argb(0xFF3366CC)));
    // The interpolator resource: lines, 0.8 at the half.
    let at = |t: f32| avd.vector_at(t);
    assert_near(group_named(at(3000.0).root(), "rotationGroup").unwrap().rotation, 288.0, 1e-2);
    // Colors from a theme attribute (a color state list: its stateless item, its alpha) to a color reference to
    // another.
    let start = path_named(at(0.0).root(), "v").unwrap().fill.as_ref().and_then(Brush::solid_color).unwrap();
    assert!(same_rgb(start, Color::hex(0xFF0000)) && near(start.alpha(), 128.0 / 255.0, 1e-3), "{start:?}");
    let end = path_named(at(3000.0).root(), "v").unwrap().fill.as_ref().and_then(Brush::solid_color).unwrap();
    assert!(same_rgb(end, Color::hex(0x00FF00)) && end.alpha() == 1.0);
    assert_eq!(
        path_named(at(3000.0).root(), "v").unwrap().path_data,
        add_path_nodes("M300,70 l 0,-70 70,0  0,140 -70,0 z")
    );
    // What isn't there: Android's NotFoundException.
    let e = AnimatedImageVector::from_animated_vector_xml(SEARCH_AVD).unwrap_err();
    assert!(e.exception == "NotFoundException" && e.message.contains("@animator/rotation"), "{e}");
    let without_integer = |r: &ResourceReference| if r.r#type == "integer" { None } else { resources(r) };
    let e = AnimatedImageVector::from_animated_vector_xml_with(SEARCH_AVD, &without_integer).unwrap_err();
    assert_eq!(e.to_string(), "NotFoundException: Resource @integer/turn not found");
}

/// Android Studio's and Shape Shifter's way: everything inline (`<aapt:attr>`), entities in values, comments, a
/// clip path animated, a group's scale and translation, a path's trims and stroke.
fn inline_resources_as_android_studio_writes_them() {
    let avd = read(
        r##"<?xml version="1.0" encoding="utf-8"?>
<!-- Shape Shifter style -->
<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt">
  <aapt:attr name="android:drawable">
    <vector android:name="hourglass" android:width="24dp" android:height="24dp" android:viewportWidth="24" android:viewportHeight="24">
      <group android:name="frame" android:pivotX="12" android:pivotY="12">
        <clip-path android:name="mask" android:pathData="M0,0 L24,0 L24,12 L0,12 Z"/>
        <path android:name="fill" android:fillColor="#FF0000" android:pathData="M6,6 L18,6 L18,18 L6,18 Z"/>
        <path android:name="outline" android:strokeColor="#000" android:strokeWidth="2" android:pathData="M2,2 L22,2"/>
      </group>
    </vector>
  </aapt:attr>
  <target android:name="mask">
    <aapt:attr name="android:animation">
      <objectAnimator android:propertyName="pathData" android:duration="200" android:valueType="pathType"
        android:valueFrom="M0,0 L24,0 L24,12 L0,12 Z" android:valueTo="M0,12 L24,12 L24,24 L0,24 Z"
        android:interpolator="@android:interpolator/fast_out_slow_in"/>
    </aapt:attr>
  </target>
  <target android:name="frame">
    <aapt:attr name="android:animation">
      <set android:ordering="together">
        <objectAnimator android:propertyName="scaleX" android:valueFrom="1" android:valueTo="0.5" android:duration="200" android:interpolator="@android:interpolator/linear"/>
        <objectAnimator android:propertyName="translateY" android:valueFrom="0" android:valueTo="4" android:duration="200" android:interpolator="@android:interpolator/linear"/>
        <objectAnimator android:propertyName="pivotX" android:valueFrom="12" android:valueTo="6" android:duration="200" android:interpolator="@android:interpolator/linear"/>
      </set>
    </aapt:attr>
  </target>
  <target android:name="outline">
    <aapt:attr name="android:animation">
      <set>
        <objectAnimator android:propertyName="trimPathStart" android:valueFrom="0" android:valueTo="0.5" android:duration="100" android:interpolator="@android:interpolator/linear"/>
        <objectAnimator android:propertyName="trimPathOffset" android:valueFrom="0" android:valueTo="0.25" android:duration="100" android:interpolator="@android:interpolator/linear"/>
        <objectAnimator android:propertyName="strokeAlpha" android:valueFrom="1" android:valueTo="0" android:duration="100" android:interpolator="@android:interpolator/linear"/>
        <objectAnimator android:propertyName="strokeColor" android:valueFrom="#000000" android:valueTo="#fffff&#x66;" android:duration="100" android:interpolator="@android:interpolator/linear"/>
      </set>
    </aapt:attr>
  </target>
</animated-vector>"##,
    );
    assert_eq!(avd.total_duration(), 200);
    let mid = avd.vector_at(50.0);
    let frame = group_named(mid.root(), "frame").unwrap();
    assert!(
        near(frame.scale_x, 0.875, 1e-5) && near(frame.translation_y, 1.0, 1e-5) && near(frame.pivot_x, 10.5, 1e-5)
    );
    // The clip path (a group of its own in the vector) morphed by fast_out_slow_in.
    let mask = group_named(mid.root(), "mask").unwrap();
    let f = bezier_at(0.4, 0.0, 0.2, 1.0, 0.25);
    let PathNode::MoveTo { x, y } = mask.clip_path_data[0] else { panic!("a move") };
    assert!(x == 0.0 && near(y, 12.0 * f, 1e-3), "{y}");
    let outline = path_named(mid.root(), "outline").unwrap();
    assert!(near(outline.trim_path_start, 0.25, 1e-5) && near(outline.trim_path_offset, 0.125, 1e-5));
    assert!(near(outline.stroke_alpha, 0.5, 1e-5));
    // An entity in a value: #ffffff at the end.
    let end = avd.vector_at(100.0);
    let outline = path_named(end.root(), "outline").unwrap();
    assert_eq!(outline.stroke.as_ref().and_then(Brush::solid_color), Some(Color::hex(0xFFFFFF)));
    let fill = path_named(end.root(), "fill").unwrap();
    assert_eq!(fill.fill.as_ref().and_then(Brush::solid_color), Some(Color::hex(0xFF0000)));
}

fn xml_that_cant_be_read_is_an_error() {
    // Whatever can't be read: an error, never a panic.
    for xml in [
        "",
        "<",
        "<vector",
        "<vector android:width=>",
        "<vector android:width=\"24\"",
        "<!-- <vector/>",
        "<?xml version=\"1.0\"",
        "<<<>>>",
        "<vector>ğüşiöç</vector",
        "<animated-vector><target android:name=\"x\"/></animated-vector>",
        "<a></b>",
        "<a/><b/>",
        "<x:a/>",
    ] {
        assert!(AnimatedImageVector::from_animated_vector_xml(xml).is_err(), "{xml}");
    }
    let error = |targets: &[(&str, &str)]| {
        AnimatedImageVector::from_animated_vector_xml(&avd_xml(r#"<group android:name="g"/>"#, targets)).unwrap_err()
    };
    // An animator element Compose doesn't read.
    let e = error(&[("g", r#"<animator android:valueTo="1"/>"#)]);
    assert!(e.exception == "XmlPullParserException" && e.message == "Unknown tag: animator", "{e}");
    // A value of the wrong format for its attribute (aapt2 refuses it).
    let e = error(&[(
        "g",
        r#"<objectAnimator android:propertyName="translateX" android:valueTo="1" android:duration="long"/>"#,
    )]);
    assert_eq!(e.message, "'long' is incompatible with attribute duration");
    // A holder without its property's name.
    let e = error(&[("g", r#"<objectAnimator><propertyValuesHolder android:valueTo="1"/></objectAnimator>"#)]);
    assert_eq!(e.exception, "NullPointerException");
    // A dimension where a float is read (Compose's getFloat: Float.parseFloat("24.0dip")).
    let e = error(&[("g", r#"<objectAnimator android:propertyName="translateX" android:valueTo="24dp"/>"#)]);
    assert_eq!(e.exception, "NumberFormatException");
    // A target whose animation is none.
    let e = AnimatedImageVector::from_animated_vector_xml(
        r#"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android"><target android:name="g" android:animation="@null"/></animated-vector>"#,
    )
    .unwrap_err();
    assert_eq!(e.to_string(), "NotFoundException: Resource ID #0x0");
    // An inline resource of no element, or two.
    assert!(AnimatedImageVector::from_animated_vector_xml(
        r#"<animated-vector xmlns:aapt="http://schemas.android.com/aapt"><aapt:attr name="android:drawable"/></animated-vector>"#
    )
    .is_err());
    // An empty set animates nothing.
    let empty = read(&avd_xml(r#"<group android:name="g"/>"#, &[("g", "<set/>")]));
    assert_eq!(empty.total_duration(), 0);
}

tests!(
    animated_vectors_play_their_animations,
    the_vectors_own_alpha_isnt_a_property_compose_plays,
    animated_vectors_play_androids_interpolators,
    sets_play_together_and_one_after_another,
    repeats_restart_and_reverse,
    keyframes_ease_as_compose_eases_them,
    path_interpolators_follow_their_paths,
    androids_interpolators_are_their_easings,
    colors_and_path_data_morph,
    two_dimensional_holders_are_read_not_played,
    infinite_repeats_take_kotlins_int,
    resources_come_from_the_caller,
    inline_resources_as_android_studio_writes_them,
    xml_that_cant_be_read_is_an_error,
);

//! rememberAnimatedVectorPainter: the C++ Kompoz's animated_vectors_play_their_animations' painter half (an
//! `Image` of the painter, played to its end and back), the painter's vector frame by frame what `vector_at` says
//! of the same time (forward; backward its reversed specs), interrupted mid-way, and the painter's own vector
//! (size, viewport, tint, Compose's autoMirror).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation_graphics::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::Modifier;
use kompoz_ui::draw::VectorPainter;
use std::cell::RefCell;
use std::rc::Rc;

fn read(xml: &str) -> AnimatedImageVector {
    AnimatedImageVector::from_animated_vector_xml(xml).unwrap_or_else(|e| panic!("{e}"))
}

fn same(a: Color, b: Color) -> bool {
    near(a.red(), b.red(), 0.01)
        && near(a.green(), b.green(), 0.01)
        && near(a.blue(), b.blue(), 0.01)
        && near(a.alpha(), b.alpha(), 0.02)
}

/// Each path's fill and stroke color as drawn (its alpha times the path's).
fn path_colors(v: &ImageVector) -> Vec<Color> {
    fn walk(g: &VectorGroup, out: &mut Vec<Color>) {
        for n in g {
            match n {
                VectorNode::Path(p) => {
                    for (brush, alpha) in [(&p.fill, p.fill_alpha), (&p.stroke, p.stroke_alpha)] {
                        if let Some(c) = brush.as_ref().and_then(Brush::solid_color) {
                            out.push(Color::new(c.red(), c.green(), c.blue(), c.alpha() * alpha));
                        }
                    }
                }
                VectorNode::Group(c) => walk(c, out),
            }
        }
    }
    let mut out = Vec::new();
    walk(v.root(), &mut out);
    out
}

/// A rule of an `Image` of the painter of `avd` at `at_end`'s, the painter handed out.
fn painted(
    avd: AnimatedImageVector,
    at_end: MutableState<bool>,
) -> (KompozTestRule, Rc<RefCell<Option<VectorPainter>>>) {
    let seen: Rc<RefCell<Option<VectorPainter>>> = Rc::new(RefCell::new(None));
    let s = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let painter = remember_animated_vector_painter(avd.clone(), at_end.get());
            *s.borrow_mut() = Some(painter.clone());
            Image(Rc::new(painter), Some("canlı".to_string()), modifier = Modifier.size(24.dp()));
        }),
        IntSize::new(100, 100),
    );
    (rule, seen)
}

fn vector_of(seen: &Rc<RefCell<Option<VectorPainter>>>) -> ImageVector {
    seen.borrow().as_ref().expect("composed").vector.clone()
}

/// The play time of `avd` (one of `candidates`) whose `vector_at` the painter's vector is.
fn play_time_of(
    avd: &AnimatedImageVector,
    painted: &ImageVector,
    candidates: impl IntoIterator<Item = f32>,
) -> Option<f32> {
    candidates.into_iter().find(|t| avd.vector_at(*t).root() == painted.root())
}

/// The C++ test's painter half: at its start, played to its end, then back.
fn animated_vectors_play_their_animations() {
    let avd = read(ANIMATED);
    let at_end = mutable_state_of(false);
    let (rule, seen) = painted(avd.clone(), at_end.clone());
    let shows = |c: Color| {
        let v = vector_of(&seen);
        // (the Image draws the painter's vector)
        assert!(rule.draw().vectors.contains(&v.id()));
        path_colors(&v).iter().any(|d| same(*d, c))
    };
    let half_blue = Color::new(0.0, 0.0, 1.0, 0.5);
    assert!(shows(Color::hex(0xFF0000)));
    at_end.set(true);
    rule.advance_time_by(48);
    assert!(!shows(Color::hex(0xFF0000)) && !shows(half_blue)); // on its way
    rule.wait_for_idle();
    assert!(shows(half_blue)); // at its end (the bar half transparent)
    at_end.set(false);
    rule.wait_for_idle();
    assert!(shows(Color::hex(0xFF0000))); // played back to its start
}

/// Frame by frame what `vector_at` says of the same play time; going back, of the time from the end (its specs
/// reversed); at rest, its ends.
fn the_painter_plays_what_vector_at_says() {
    let avd = read(ANIMATED);
    let at_end = mutable_state_of(false);
    let (rule, seen) = painted(avd.clone(), at_end.clone());
    // At rest: each animated property at its first keyframe (Compose's), the rest as drawn.
    let start = vector_of(&seen);
    assert_eq!(path_colors(&start).len(), 3);
    assert!(same(path_colors(&start)[1], Color::hex(0xFF0000)));
    at_end.set(true);
    rule.advance_time_by(48);
    let frames: Vec<f32> = (0..=20).map(|i| i as f32 * 16.0).collect();
    let t = play_time_of(&avd, &vector_of(&seen), frames.clone()).expect("a frame vector_at gives");
    assert!(t > 0.0 && t <= 48.0, "{t}");
    // Frame after frame, the same, a frame (16 ms) on each time.
    let mut last = t;
    for _ in 0..12 {
        rule.advance_frame();
        let next = play_time_of(&avd, &vector_of(&seen), [last + 16.0]);
        assert_eq!(next, Some(last + 16.0), "a frame after {last}");
        last += 16.0;
    }
    rule.wait_for_idle();
    // At its end: the last keyframes.
    assert_eq!(vector_of(&seen).root(), avd.vector_at(300.0).root());
    // Back: its specs reversed over the whole 300 (the line's reversing trim, done at 220 forward, aside: at its
    // end the Transition has its target, the spec its own last value; past 100 back they agree).
    at_end.set(false);
    rule.advance_time_by(160);
    let back = vector_of(&seen);
    let t = play_time_of(&avd, &back, (0..=20).map(|i| 300.0 - i as f32 * 16.0)).expect("a reversed frame");
    assert!(t < 200.0 && t > 100.0, "{t}");
    rule.wait_for_idle();
    let rest = vector_of(&seen);
    assert!(path_colors(&rest).iter().zip(path_colors(&start)).all(|(a, b)| same(*a, b)));
}

/// Turned back mid-way: the Transition goes on from where it is (Compose's interruption: a spring from the value
/// it has), no jump.
fn turned_back_it_goes_on_from_where_it_is() {
    let avd = read(ANIMATED);
    let at_end = mutable_state_of(false);
    let (rule, seen) = painted(avd.clone(), at_end.clone());
    let rotation = || {
        let v = vector_of(&seen);
        v.root().iter().filter_map(VectorNode::group).find(|g| g.name == "spin").unwrap().rotation
    };
    at_end.set(true);
    rule.advance_time_by(96);
    let mid = rotation();
    assert!(mid > 30.0 && mid < 110.0, "{mid}");
    at_end.set(false);
    rule.advance_frame();
    let after = rotation();
    assert!((after - mid).abs() < 20.0, "{mid} then {after}");
    rule.wait_for_idle();
    assert_near(rotation(), 0.0, 0.01);
}

/// The painter's vector: the image vector's size, viewport, name and tint, mirrored right to left (Compose's
/// `autoMirror = true`).
fn the_painter_is_its_vectors() {
    let avd = read(
        r##"<animated-vector xmlns:android="http://schemas.android.com/apk/res/android" xmlns:aapt="http://schemas.android.com/aapt">
        <aapt:attr name="android:drawable"><vector android:width="48dp" android:height="32dp" android:viewportWidth="12"
        android:viewportHeight="8" android:tint="#80FF0000" android:tintMode="src_atop"><path android:name="p" android:fillColor="#000"
        android:pathData="M0,0h4v4z"/></vector></aapt:attr>
        </animated-vector>"##,
    );
    let (rule, seen) = painted(avd.clone(), mutable_state_of(false));
    let v = vector_of(&seen);
    let own = avd.image_vector();
    assert!(v.default_width() == Dp(48.0) && v.default_height() == Dp(32.0));
    assert!(v.viewport_width() == 12.0 && v.viewport_height() == 8.0);
    assert!(v.tint_color() == own.tint_color() && v.tint_blend_mode() == BlendMode::SrcAtop);
    assert!(v.auto_mirror() && !own.auto_mirror() && v.name() == own.name());
    assert_eq!(v.root(), own.root());
    assert_eq!(rule.on_node_with_content_description("canlı").get_unclipped_bounds_in_root().width(), 24.0);
}

tests!(
    animated_vectors_play_their_animations,
    the_painter_plays_what_vector_at_says,
    turned_back_it_goes_on_from_where_it_is,
    the_painter_is_its_vectors,
);

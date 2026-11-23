//! The C++ Kompoz's last Kotlin animation test, `animation_as_compose_writes_it` (kompozable_tests.cpp, its whole
//! `Animations` source in a `.kt` file of its own: kompoz_file!): `animateFloatAsState`/`animateColorAsState` by
//! delegate, `updateTransition(…).animateDp`, `rememberInfiniteTransition().animateFloat`, an Animatable shaken in a
//! LaunchedEffect, AnimatedVisibility with `fadeIn() + expandVertically()`, AnimatedContent with `togetherWith` and
//! `using SizeTransform(clip = false)`, Crossfade, and a shared element with `this@AnimatedContent` in a
//! SharedTransitionLayout. The C++ checks (where it starts, where it settles), and the way there as Compose runs it.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;

/// (The file's `enum class Panel { Closed, Open }`: enum classes come with stage 14's Kotlin syntax.)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Panel {
    Closed,
    Open,
}

kompoz_file!("tests/kt/animations_all.kt");

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn exists(rule: &KompozTestRule, text: &str) -> bool {
    !rule.on_all_nodes_with_text(text).fetch_semantics_nodes().is_empty()
}

fn tag_exists(rule: &KompozTestRule, tag: &str) -> bool {
    !rule.on_all_nodes_with_tag(tag).fetch_semantics_nodes().is_empty()
}

fn rule_of(open: &MutableState<bool>, count: &MutableState<i32>) -> KompozTestRule {
    let (o, c) = (open.clone(), count.clone());
    KompozTestRule::new(kompozable_lambda!(move || Animations(o.get(), c.get())))
}

fn animation_as_compose_writes_it() {
    let open = mutable_state_of(false);
    let count = mutable_state_of(0);
    let rule = rule_of(&open, &count);
    assert!(exists(&rule, "ölçek 10") && exists(&rule, "sayı 0") && exists(&rule, "kapalı"));
    assert!(!exists(&rule, "ayrıntılar") && tag_exists(&rule, "küçük"));
    assert!((bounds(&rule, "kutu").width() - 80.0).abs() < 0.5);
    open.set(true);
    count.set(1);
    rule.wait_for_idle();
    assert!(exists(&rule, "ölçek 12") && exists(&rule, "sayı 1") && exists(&rule, "açık"));
    assert!(exists(&rule, "ayrıntılar") && tag_exists(&rule, "büyük"));
    assert!((bounds(&rule, "kutu").width() - 200.0).abs() < 0.5);
    let ink = InkBlue.with(|c| *c);
    assert!(ink == Color::from_argb(0xFF1E88E5) && (ink.blue() - 0xE5 as f32 / 255.0).abs() < 0.01);
}

/// The way there, frame by frame: the transition's size grows between its ends, the Animatable shakes the box left
/// and right before it rests at 0, both texts are in while the content and the crossfade change, the details expand.
fn the_animations_run_their_course() {
    let open = mutable_state_of(false);
    let count = mutable_state_of(0);
    let rule = rule_of(&open, &count);
    // (the clock by hand: the checks frame by frame see the animations running)
    rule.main_clock().set_auto_advance(false);
    open.set(true);
    count.set(1);
    let mut widths = Vec::new();
    let mut lefts = Vec::new();
    let mut both_counts = false;
    let mut both_pages = false;
    for _ in 0..12 {
        rule.advance_frame();
        widths.push(bounds(&rule, "kutu").width());
        lefts.push(bounds(&rule, "kutu").left);
        both_counts |= exists(&rule, "sayı 0") && exists(&rule, "sayı 1");
        both_pages |= exists(&rule, "açık") && exists(&rule, "kapalı");
    }
    assert!(widths.iter().any(|w| *w > 80.5 && *w < 199.5), "{widths:?}");
    assert!(lefts.iter().any(|l| *l < -0.5) && lefts.iter().any(|l| *l > 0.5), "{lefts:?}");
    assert!(both_counts && both_pages);
    common::settle(&rule);
    assert_eq!(bounds(&rule, "kutu").left, 0.0);
    assert!(!exists(&rule, "sayı 0") && !exists(&rule, "kapalı") && !tag_exists(&rule, "küçük"));
    // Back: the details shrink away, the counter goes down the other way (its SizeTransform without a clip).
    open.set(false);
    count.set(0);
    common::settle(&rule);
    assert!(exists(&rule, "ölçek 10") && exists(&rule, "sayı 0") && exists(&rule, "kapalı"));
    assert!(!exists(&rule, "ayrıntılar") && tag_exists(&rule, "küçük"));
    assert!((bounds(&rule, "kutu").width() - 80.0).abs() < 0.5);
}

tests!(animation_as_compose_writes_it, the_animations_run_their_course);

//! animation-core's value layer in Kotlin's syntax, as a Compose user writes it: an Animatable animated to targets
//! in a LaunchedEffect (`anim.animateTo(1f, tween(300))`), along a spring, keyframes (`1.3f at 100 using …`) with a
//! block of the Animatable as its receiver, an infinite repeat; bounds and a decay; an Animatable of offsets given
//! as `Animatable<Offset, AnimationVector2D>`; the suspend `animate`; and in a `.kt` file (kompoz_file!) a color
//! Animatable, an AnimationState's decay, `animate` of offsets through their converter, a MutatorMutex. The same
//! behaviour crates/kompoz-animation/tests checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A node's size in the root (px; the rule's density is 1).
fn width(rule: &KompozTestRule, tag: &str) -> f32 {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root().width()
}

kompoz! {
    @Composable
    fun Grow(go: Boolean, done: () -> Unit) {
        val size = remember { Animatable(0f) }
        LaunchedEffect(go) {
            if (!go) return@LaunchedEffect
            size.animateTo(100f, tween(300, easing = LinearEasing))
            size.animateTo(40f, spring(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessMedium))
            done()
        }
        Box(Modifier.testTag("grow").size(size.value.dp))
    }

    @Composable
    fun Slide(offset: Animatable<Offset, AnimationVector2D>, go: Boolean) {
        LaunchedEffect(offset, go) {
            if (go) {
                offset.animateTo(Offset(100f, 50f), animationSpec = tween(durationMillis = 160, delayMillis = 0))
            }
        }
        Box(Modifier.offset(offset.value.x.dp, offset.value.y.dp).testTag("slide").size(10.dp))
    }

    @Composable
    fun Pulse(log: (Float) -> Unit) {
        val scale = remember { Animatable(1f) }
        val scope = rememberCoroutineScope()
        Box(Modifier.testTag("pulse").size(20.dp).clickable {
            scope.launch {
                scale.animateTo(
                    1f,
                    keyframes {
                        durationMillis = 400
                        1.3f at 100 using FastOutSlowInEasing
                        0.9f at 250
                    },
                ) {
                    log(value)
                }
            }
        })
    }

    @Composable
    fun Fling(target: (Float, Boolean) -> Unit) {
        val position = remember { Animatable(0f, Offset.VisibilityThreshold.x) }
        LaunchedEffect(Unit) {
            position.updateBounds(upperBound = 150f)
            val result = position.animateDecay(1000f, exponentialDecay())
            target(position.value, result.endReason == AnimationEndReason.BoundReached)
        }
    }

    @Composable
    fun Ticker(seen: (Float, Float) -> Unit) {
        LaunchedEffect(Unit) {
            animate(0f, 10f, animationSpec = tween(100, easing = LinearEasing)) { value, velocity ->
                seen(value, velocity)
            }
        }
    }

    @Composable
    fun Forever(go: Boolean, ticks: (Float) -> Unit) {
        val angle = remember { Animatable(0f) }
        LaunchedEffect(go) {
            if (!go) return@LaunchedEffect
            angle.animateTo(360f, infiniteRepeatable(tween(1000, easing = LinearEasing), RepeatMode.Restart)) {
                ticks(value)
            }
        }
    }
}

kompoz_file!("tests/kt/animation.kt");

// ---- kompoz! ----------------------------------------------------------------------------------------------------

/// (The rule runs what its content starts until it is idle: an animation is started after, by a state, to be seen
/// on its way.)
fn an_animatable_animates_in_a_launched_effect() {
    let done = Rc::new(Cell::new(false));
    let go = mutable_state_of(false);
    let (d, g) = (done.clone(), go.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let d = d.clone();
        Grow(g.value(), move || d.set(true));
    }));
    go.set(true);
    rule.advance_time_by(150);
    let w = width(&rule, "grow");
    assert!(w > 30.0 && w < 70.0, "{w}");
    // Up to 100 along the tween, then a spring back to 40.
    let mut widths = Vec::new();
    while !done.get() {
        rule.advance_frame();
        widths.push(width(&rule, "grow"));
    }
    let top = widths.iter().position(|w| *w == 100.0).expect("the tween's end");
    assert!(widths[..top].windows(2).all(|w| w[1] >= w[0]) && widths[top..].windows(2).all(|w| w[1] <= w[0]));
    assert_eq!(width(&rule, "grow"), 40.0);
}

fn an_animatable_of_offsets() {
    let offset = Animatable((Offset::Zero, Offset::VectorConverter));
    let go = mutable_state_of(false);
    let (o, g) = (offset.clone(), go.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || Slide(o.clone(), g.value())));
    go.set(true);
    rule.advance_time_by(80);
    let x = rule.on_node_with_tag("slide").get_unclipped_bounds_in_root().left;
    assert!(x > 20.0 && x < 80.0, "{x}");
    rule.wait_for_idle();
    let b = rule.on_node_with_tag("slide").get_unclipped_bounds_in_root();
    assert_eq!((b.left, b.top), (100.0, 50.0));
    assert_eq!(offset.peek(), Offset::new(100.0, 50.0));
}

fn keyframes_with_a_block_of_the_animatable() {
    let log: Rc<RefCell<Vec<f32>>> = Rc::default();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Pulse(move |v| l.borrow_mut().push(v));
    }));
    rule.on_node_with_tag("pulse").perform_click();
    rule.wait_for_idle();
    let log = log.borrow();
    let most = log.iter().cloned().fold(0.0, f32::max);
    assert!(most > 1.25 && *log.last().unwrap() == 1.0, "{log:?}");
    assert!(log.iter().any(|v| *v < 1.0)); // through 0.9
}

fn a_decay_ends_at_a_bound() {
    let got = Rc::new(Cell::new((0.0f32, false)));
    let g = got.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let g = g.clone();
        Fling(move |v, bound| g.set((v, bound)));
    }));
    rule.wait_for_idle();
    assert_eq!(got.get(), (150.0, true));
}

fn animate_gives_values_and_velocities() {
    let seen: Rc<RefCell<Vec<(f32, f32)>>> = Rc::default();
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Ticker(move |v, vel| s.borrow_mut().push((v, vel)));
    }));
    rule.wait_for_idle();
    let seen = seen.borrow();
    assert_eq!(seen.last().unwrap().0, 10.0);
    assert!(seen.iter().any(|(_, vel)| (vel - 100.0).abs() < 1.0));
}

fn an_infinite_repeat_goes_on() {
    let ticks: Rc<RefCell<Vec<f32>>> = Rc::default();
    let go = mutable_state_of(true);
    let (t, g) = (ticks.clone(), go.clone());
    // (Compose's way: the clock's auto advance off before composing — while it is on, the rule's
    // InfiniteAnimationPolicy cancels infinite animations, which would never let it be idle)
    let rule = KompozTestRule::empty();
    rule.main_clock().set_auto_advance(false);
    rule.set_content(kompozable_lambda!(move || {
        let t = t.clone();
        Forever(g.value(), move |v| t.borrow_mut().push(v));
    }));
    rule.advance_time_by(1500);
    let ticks = ticks.borrow();
    // Round and round: up to 360, then from 0 again.
    assert!(ticks.iter().any(|v| *v > 300.0));
    assert!(ticks.windows(2).any(|w| w[1] < w[0]));
    assert!(rule.host().has_pending_work());
}

// ---- kompoz_file! -------------------------------------------------------------------------------------------------

fn a_color_animatable_in_a_kt_file() {
    let seen: Rc<RefCell<Vec<Color>>> = Rc::default();
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Tint(move |c| s.borrow_mut().push(c));
    }));
    rule.wait_for_idle();
    let seen = seen.borrow();
    assert_eq!(seen.first().unwrap().to_argb(), Color::Black.to_argb());
    assert_eq!(seen.last().unwrap().to_argb(), Color::White.to_argb());
    assert!(seen.iter().any(|c| c.red() > 0.2 && c.red() < 0.8));
}

fn an_animation_states_decay_in_a_kt_file() {
    let got = Rc::new(Cell::new((0.0f32, 1.0f32)));
    let g = got.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let g = g.clone();
        Flung(move |v, vel| g.set((v, vel)));
    }));
    rule.wait_for_idle();
    let (v, vel) = got.get();
    assert!((v - 100.0).abs() < 0.05 && vel.abs() <= 0.1, "{v} {vel}");
}

fn animate_through_a_converter_in_a_kt_file() {
    let points: Rc<RefCell<Vec<Offset>>> = Rc::default();
    let p = points.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let p = p.clone();
        Path2D(move |o| p.borrow_mut().push(o));
    }));
    rule.wait_for_idle();
    let points = points.borrow();
    assert_eq!(*points.last().unwrap(), Offset::new(30.0, 60.0));
    assert!(points.iter().all(|o| (o.y - 2.0 * o.x).abs() < 1e-3));
}

fn a_mutator_mutex_in_a_kt_file() {
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Mutations(move |s: String| l.borrow_mut().push(s));
    }));
    rule.wait_for_idle();
    // The animation can't interrupt the user's drag: it is cancelled itself.
    assert_eq!(*log.borrow(), ["drag", "drag done"]);
}

tests![
    an_animatable_animates_in_a_launched_effect,
    an_animatable_of_offsets,
    keyframes_with_a_block_of_the_animatable,
    a_decay_ends_at_a_bound,
    animate_gives_values_and_velocities,
    an_infinite_repeat_goes_on,
    a_color_animatable_in_a_kt_file,
    an_animation_states_decay_in_a_kt_file,
    animate_through_a_converter_in_a_kt_file,
    a_mutator_mutex_in_a_kt_file,
];

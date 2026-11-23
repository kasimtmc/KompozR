//! Content coming in and going in Kotlin's syntax, as a Compose user writes it: the C++ Kompoz's kompozable_tests.cpp
//! 2452-2465 (`AnimatedVisibility(visible = …, enter = fadeIn() + expandVertically(), …)` in a Column — ColumnScope's —,
//! `AnimatedContent(transitionSpec = { if (targetState > initialState) slideInVertically { it } togetherWith … else
//! (…) using SizeTransform(clip = false) })`, `Crossfade`) and its GraphicsForms' stage-11 parts 3351-3380
//! (`remember { MutableTransitionState(false).apply { targetState = true } }`, `AnimatedVisibility(visibleState = …)`,
//! `Modifier.animateEnterExit(enter = …, exit = …)`); `AnimatedVisibility` in a Row (RowScope's); and in a `.kt` file
//! (kompoz_file!) every enter and exit transition, the forms of a transition, seeking, sliding into the container,
//! a ContentTransform made by hand, the deferred forms with MutableTransform blocks. The same behaviour
//! crates/kompoz-animation/tests/animated.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn exists(rule: &KompozTestRule, tag: &str) -> bool {
    !rule.on_all_nodes_with_tag(tag).fetch_semantics_nodes().is_empty()
}

fn text_exists(rule: &KompozTestRule, text: &str) -> bool {
    !rule.on_all_nodes_with_text(text).fetch_semantics_nodes().is_empty()
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

// ---- kompozable_tests.cpp 2452-2465 -----------------------------------------------------------------------------

kompoz! {
    @Composable
    fun Visibilities(open: Boolean, count: Int) {
        Column {
            AnimatedVisibility(visible = open, enter = fadeIn() + expandVertically(), exit = fadeOut() + shrinkVertically(), label = "details") {
                Text("ayrıntılar")
            }
            AnimatedContent(
                targetState = count,
                transitionSpec = {
                    if (targetState > initialState) slideInVertically { it } togetherWith slideOutVertically { -it }
                    else (slideInVertically { -it } togetherWith slideOutVertically { it }) using SizeTransform(clip = false)
                },
                label = "count",
            ) { n ->
                Text("sayı $n", modifier = Modifier.testTag("sayı $n"))
            }
            Crossfade(targetState = open, label = "page") { p -> Text(if (p) "açık" else "kapalı") }
            Row {
                // (RowScope's: across, the height whole)
                AnimatedVisibility(open, modifier = Modifier.testTag("satır")) { Box(modifier = Modifier.size(100.dp, 40.dp)) }
            }
        }
    }
}

fn animated_visibility_and_content_as_compose_writes_them() {
    let open = mutable_state_of(false);
    let count = mutable_state_of(0);
    let (o, c) = (open.clone(), count.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || Visibilities(o.get(), c.get())));
    assert!(text_exists(&rule, "sayı 0") && text_exists(&rule, "kapalı") && !text_exists(&rule, "ayrıntılar"));
    // (the clock by hand: the checks on the way see the animations running)
    rule.main_clock().set_auto_advance(false);
    open.set(true);
    count.set(1);
    rule.advance_time_by(96);
    // Up: the new number from below, the old one up and out; the row's box across.
    assert!(text_exists(&rule, "sayı 0") && text_exists(&rule, "sayı 1") && text_exists(&rule, "açık"));
    assert!(bounds(&rule, "sayı 1").top > bounds(&rule, "sayı 0").top);
    let row = bounds(&rule, "satır");
    assert!(row.width() > 1.0 && row.width() < 99.0 && near(row.height(), 40.0, 0.5), "{row:?}");
    common::settle(&rule);
    assert!(text_exists(&rule, "sayı 1") && !text_exists(&rule, "sayı 0") && text_exists(&rule, "ayrıntılar"));
    assert!(text_exists(&rule, "açık") && !text_exists(&rule, "kapalı"));
    // Down: the other way.
    count.set(0);
    rule.advance_time_by(96);
    assert!(bounds(&rule, "sayı 0").top < bounds(&rule, "sayı 1").top);
    open.set(false);
    common::settle(&rule);
    assert!(!text_exists(&rule, "ayrıntılar") && !exists(&rule, "satır") && text_exists(&rule, "kapalı"));
}

// ---- kompozable_tests.cpp 3351-3380 (GraphicsForms' stage-11 parts) ----------------------------------------------

kompoz! {
    @Composable
    fun Entering() {
        val shown = remember { MutableTransitionState(false).apply { targetState = true } }
        Column {
            AnimatedVisibility(visibleState = shown, enter = fadeIn(animationSpec = tween(100)) + expandVertically(animationSpec = tween(100)),
                               exit = fadeOut(animationSpec = tween(100))) {
                Box(modifier = Modifier.animateEnterExit(enter = slideInVertically(animationSpec = tween(100)) { it }, exit = scaleOut(targetScale = 0.5f))
                        .size(10.dp).testTag("giren"))
            }
            Text("durum ${shown.currentState}")
        }
    }
}

fn a_visible_state_comes_in_as_it_first_shows() {
    // (the clock by hand: the first composition seen before the entrance runs)
    let rule = KompozTestRule::empty();
    rule.main_clock().set_auto_advance(false);
    rule.set_content(kompozable_lambda!(move || Entering()));
    rule.on_node_with_text("durum false").assert_exists();
    rule.on_node_with_tag("giren").assert_exists();
    rule.main_clock().advance_time_by(48);
    // On its way: expanding from the bottom (the content held up), its child sliding up from a whole height down.
    let moved = rule.draw().translations;
    assert!(
        moved.iter().any(|o| o.y < -0.5 && o.y > -9.5) && moved.iter().any(|o| o.y > 0.5 && o.y < 9.5),
        "{moved:?}"
    );
    rule.main_clock().advance_time_by(200);
    rule.on_node_with_text("durum true").assert_exists();
    assert!(near(bounds(&rule, "giren").top, 0.0, 0.5));
}

// ---- kompoz_file! ------------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/animated.kt");

fn every_transition_in_a_kt_file() {
    let shown = mutable_state_of(false);
    let s = shown.clone();
    let rule =
        KompozTestRule::with_viewport(kompozable_lambda!(move || EveryTransition(s.get())), IntSize::new(600, 600));
    // (the clock by hand: the checks on the way see the transitions running)
    rule.main_clock().set_auto_advance(false);
    shown.set(true);
    rule.advance_frame();
    rule.advance_frame();
    let b = bounds(&rule, "kayan");
    assert!(near(b.left, 200.0, 0.5), "{b:?}"); // from two widths right
    rule.advance_time_by(160);
    let b = bounds(&rule, "büyüyen");
    assert!(near(b.width(), 75.0, 7.0) && near(b.height(), 37.5, 4.0), "{b:?}"); // from half its size
    common::settle(&rule);
    assert!(near(bounds(&rule, "kayan").left, 0.0, 0.5) && exists(&rule, "ölçekli"));
    shown.set(false);
    rule.advance_frame();
    rule.advance_time_by(160);
    let b = bounds(&rule, "büyüyen");
    assert!(near(b.width(), 62.5, 7.0), "{b:?}"); // half way to a quarter of its width
    common::settle(&rule);
    assert!(!exists(&rule, "kayan") && !exists(&rule, "büyüyen") && !exists(&rule, "ölçekli"));
}

fn a_transitions_forms_in_a_kt_file() {
    let step = mutable_state_of(0);
    let s = step.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || OfATransition(s.get())));
    // (the clock by hand: the checks on the way see the transition running)
    rule.main_clock().set_auto_advance(false);
    step.set(1);
    rule.advance_time_by(96);
    assert!(exists(&rule, "görünür") && text_exists(&rule, "adım 0") && text_exists(&rule, "adım 1"));
    assert!(text_exists(&rule, "geçiş 1") && !text_exists(&rule, "geçiş 0")); // one key: no crossing
    common::settle(&rule);
    step.set(2);
    rule.advance_time_by(96);
    assert!(text_exists(&rule, "geçiş 1") && text_exists(&rule, "geçiş 2"));
    step.set(0);
    common::settle(&rule);
    assert!(!exists(&rule, "görünür") && text_exists(&rule, "adım 0"));
}

fn sliding_into_the_container_in_a_kt_file() {
    let state = SeekableTransitionState(0);
    let scope = std::rc::Rc::new(std::cell::RefCell::new(None::<CoroutineScope>));
    let (st, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *sc.borrow_mut() = Some(remember_coroutine_scope());
        Sliding(st.clone());
    }));
    let scope = scope.borrow().clone().expect("composed");
    let left = |rule: &KompozTestRule, tag: &str| bounds(rule, tag).left - bounds(rule, "kap").left;
    // (the clock by hand: the checks on the way see the transition running)
    rule.main_clock().set_auto_advance(false);
    // Seeked half way to 1: from the right edge, half way in; the old one half way out to the left.
    scope.launch(named_args!(state.seek_to(0.5, target_state = 1)));
    common::settle(&rule);
    assert!(near(left(&rule, "parça 1"), 50.0, 1.0) && near(left(&rule, "parça 0"), -50.0, 1.0));
    scope.launch(state.animate_to());
    common::settle(&rule);
    assert!(!exists(&rule, "parça 0") && near(left(&rule, "parça 1"), 0.0, 0.5));
    // Back: by a ContentTransform made by hand (the old one kept until it is done, the new one under it).
    scope.launch(named_args!(state.animate_to(0)));
    rule.advance_time_by(160);
    assert!(exists(&rule, "parça 1") && exists(&rule, "parça 0"));
    common::settle(&rule);
    assert!(!exists(&rule, "parça 1"));
}

fn deferred_forms_in_a_kt_file() {
    let visible = DeferredTransitionState(true);
    let page = DeferredTransitionState(0);
    let drag = mutable_state_of(0);
    let (v, p, d) = (visible.clone(), page.clone(), drag.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || Deferred(v.clone(), p.clone(), d.get())),
        IntSize::new(400, 400),
    );
    visible.defer(false);
    page.defer(1);
    drag.set(60);
    rule.advance_frame();
    rule.advance_frame();
    // By hand while deferring: the visibility's content and both pages where the blocks put them.
    assert!(near(bounds(&rule, "sürüklenen").left, 60.0, 0.5));
    assert!(near(bounds(&rule, "sayfa 0").left, -60.0, 0.5) && near(bounds(&rule, "sayfa 1").left, 40.0, 0.5));
    // Then the transitions take over.
    visible.animate_to(false);
    page.animate_to(1);
    rule.wait_for_idle();
    assert!(!exists(&rule, "sürüklenen") && !exists(&rule, "sayfa 0") && near(bounds(&rule, "sayfa 1").left, 0.0, 0.5));
}

fn a_content_scope_given_on_in_a_kt_file() {
    let page = mutable_state_of(0);
    let p = page.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || Panes(p.get())));
    // (the clock by hand: the checks on the way see the slide running)
    rule.main_clock().set_auto_advance(false);
    // (0 and 1: one key, the content given the new page; 2: another, coming in by its own slide)
    page.set(1);
    common::settle(&rule);
    assert!(exists(&rule, "bölme 1") && !exists(&rule, "bölme 0"));
    page.set(2);
    rule.advance_frame();
    rule.advance_frame();
    assert!(exists(&rule, "bölme 1") && bounds(&rule, "bölme 2").left > 50.0);
    common::settle(&rule);
    assert!(!exists(&rule, "bölme 1") && near(bounds(&rule, "bölme 2").left, 0.0, 0.5));
}

tests!(
    a_content_scope_given_on_in_a_kt_file,
    animated_visibility_and_content_as_compose_writes_them,
    a_visible_state_comes_in_as_it_first_shows,
    every_transition_in_a_kt_file,
    a_transitions_forms_in_a_kt_file,
    sliding_into_the_container_in_a_kt_file,
    deferred_forms_in_a_kt_file,
);

//! Content coming in and going (Compose's EnterExitTransition.kt, AnimatedVisibility.kt, AnimatedContent.kt,
//! Crossfade.kt, DeferredEnterExitTransition.kt): the C++ Kompoz's tests of AnimatedVisibility (3313, 9178, 24491,
//! 24541, the AnimatedVisibility part of 10864, the animation half of 3468) and of AnimatedContent and Crossfade (3344,
//! 3365, 3511, 4621, 24372, 24426, 24610), by name, adapted where Compose's behaviour differs (each says how); and
//! Compose's behaviour: the transitions' parts and `+`, the Row and Column forms' defaults, lookahead, the deferred
//! forms.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::MutableTransformScope;
use kompoz_animation::*;
use kompoz_coroutines::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::LookaheadScope;
use kompoz_ui::input::PointerType;
use kompoz_ui::layout::{SlotKey, TransformOrigin};
use kompoz_ui::{Alignment, Modifier};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

fn linear<T: 'static>(ms: i32) -> Arc<dyn FiniteAnimationSpec<T>> {
    Arc::new(TweenSpec::<T>::new(ms, 0, LinearEasing))
}

fn eased<T: 'static>(ms: i32) -> Arc<dyn FiniteAnimationSpec<T>> {
    Arc::new(TweenSpec::<T>::new(ms, 0, FastOutSlowInEasing))
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn exists(rule: &KompozTestRule, tag: &str) -> bool {
    !rule.on_all_nodes_with_tag(tag).fetch_semantics_nodes().is_empty()
}

fn text_exists(rule: &KompozTestRule, text: &str) -> bool {
    !rule.on_all_nodes_with_text(text).fetch_semantics_nodes().is_empty()
}

fn center(rule: &KompozTestRule, tag: &str) -> Offset {
    bounds(rule, tag).center()
}

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.host().pointer_up(p, -1, 0);
}

// ---- C++ 3313 ---------------------------------------------------------------------------------------------------

fn animated_visibility_expands_then_disposes_on_exit() {
    // (Kotlin's `Column { AnimatedVisibility(visible) { … } }` is ColumnScope's: expanding and shrinking vertically)
    let visible = mutable_state_of(false);
    let log = Rc::new(RefCell::new(Vec::<&'static str>::new()));
    let (v, l) = (visible.clone(), log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, l) = (v.clone(), l.clone());
        Column(move || {
            let l = l.clone();
            ColumnAnimatedVisibility(
                v.get(),
                content = move |_| {
                    let l = l.clone();
                    DisposableEffect((), move || {
                        l.borrow_mut().push("shown");
                        let l = l.clone();
                        on_dispose(move || l.borrow_mut().push("gone"))
                    });
                    Box(modifier = Modifier.size(100.dp()).test_tag("panel"));
                },
            );
            Box(modifier = Modifier.size(10.dp()).test_tag("below"));
        });
    }));
    rule.main_clock().set_auto_advance(false);
    assert!(!exists(&rule, "panel"));
    visible.set(true);
    rule.advance_time_by(64);
    let mid = bounds(&rule, "below").top;
    assert!(mid > 0.0 && mid < 100.0, "{mid}");
    settle(&rule);
    assert_near(bounds(&rule, "below").top, 100.0, 0.5);
    visible.set(false);
    rule.advance_time_by(64);
    assert!(exists(&rule, "panel"), "still there while it shrinks");
    settle(&rule);
    assert!(!exists(&rule, "panel"));
    assert_near(bounds(&rule, "below").top, 0.0, 0.5);
    assert_eq!(*log.borrow(), ["shown", "gone"]);
}

// ---- C++ 9178 ---------------------------------------------------------------------------------------------------

fn scale_in_animates_through_a_transform() {
    let visible = mutable_state_of(false);
    let v = visible.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        AnimatedVisibility(v.get(), enter = scale_in(), exit = scale_out(), content = |_| Text("ölçek"));
    }));
    visible.set(true);
    rule.advance_time_by(64);
    let during = rule.draw();
    assert_eq!(during.texts, ["ölçek"]);
    assert!(during.transforms.len() == 1 && during.layers == 0, "{} {}", during.stack, during.layers);
    let scale = during.transforms[0].values[0];
    assert!(scale > 0.01 && scale < 0.99, "{scale}");
    rule.wait_for_idle();
    let after = rule.draw();
    assert!(after.transformed_layers == 0 && after.layers == 0 && after.transforms.is_empty(), "{}", after.stack);
}

// ---- C++ 24491 --------------------------------------------------------------------------------------------------

fn transitions_compose_their_caller_as_they_start_and_end() {
    // Adapted: Compose's AnimatedContent and AnimatedVisibility are restartable composables, their transitions'
    // states read in their own scopes: the caller is composed as the targets change, not as they start and end
    // (C++ composed it as each got there). The rest as C++: laid out and drawn as they go, each out at its end.
    let page = mutable_state_of(0);
    let shown = mutable_state_of(true);
    let composed = Rc::new(Cell::new(0));
    let (p, sh, c) = (page.clone(), shown.clone(), composed.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (p, sh, c) = (p.clone(), sh.clone(), c.clone());
        Column(move || {
            c.set(c.get() + 1);
            AnimatedContent(
                p.get(),
                transition_spec = |_| fade_in(eased(320)).together_with(fade_out(eased(200))),
                content = |_, p: i32| Box(modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag(format!("sayfa{p}"))),
            );
            ColumnAnimatedVisibility(
                sh.get(),
                enter = fade_in(eased(300)) + expand_vertically(eased(300)),
                exit = fade_out(eased(300)) + shrink_vertically(eased(300)),
                content = |_| Box(modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("kutu")),
            );
            Box(modifier = Modifier.size_wh(100.dp(), 10.dp()).test_tag("alt"));
        });
    }));
    rule.main_clock().set_auto_advance(false);
    let alphas = |rule: &KompozTestRule| rule.draw().layer_alphas;
    let below = bounds(&rule, "alt").top;
    page.set(1);
    shown.set(false);
    rule.advance_frame(); // composed with the new targets
    let before = composed.get();
    rule.advance_time_by(96);
    // On their way, laid out and drawn as they go, the caller not composed.
    let a = alphas(&rule);
    assert!(composed.get() == before && exists(&rule, "sayfa0") && exists(&rule, "sayfa1"));
    assert!(a.len() == 3 && a.iter().all(|x| *x > 0.05 && *x < 0.95), "{a:?}");
    let y = bounds(&rule, "alt").top;
    assert!(y < below - 5.0 && y > below - 35.0, "{y}"); // what is under the shrinking box comes up with it
    // (the page that left is out at 200 ms: its exit is done at a frame, its layout gone the frame after; the rest
    // still on their way)
    rule.advance_time_by(160);
    assert!(!exists(&rule, "sayfa0") && exists(&rule, "kutu"));
    settle(&rule);
    assert!(!exists(&rule, "sayfa0") && exists(&rule, "sayfa1") && !exists(&rule, "kutu"));
    assert!(alphas(&rule).is_empty() && near(bounds(&rule, "alt").top, below - 40.0, 0.5));
    assert_eq!(composed.get(), before);
    // Turned back on its way: out from where it turned (Compose: an interrupted transition goes on along its
    // interruption spring; C++ took the exit's whole spec from there, out later).
    shown.set(true);
    rule.advance_time_by(160);
    shown.set(false);
    rule.advance_frame();
    rule.advance_time_by(96);
    let a = alphas(&rule);
    assert!(exists(&rule, "kutu") && a.len() == 1 && a[0] < 0.7, "{a:?}");
    settle(&rule);
    assert!(!exists(&rule, "kutu"));
    assert_eq!(composed.get(), before + 2);
}

// ---- C++ 24541 --------------------------------------------------------------------------------------------------

fn animate_enter_exit_gives_children_their_own_transitions() {
    let open = mutable_state_of(false);
    let later = mutable_state_of(false);
    let (o, l) = (open.clone(), later.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, l) = (o.clone(), l.clone());
        Column(move || {
            ColumnAnimatedVisibility(
                o.get(),
                enter = EnterTransition::None,
                exit = ExitTransition::None,
                content = |s: &AnimatedVisibilityScope| {
                    let s = s.clone();
                    Column(move || {
                        let size = Modifier.size_wh(100.dp(), 40.dp());
                        let fade = s.animate_enter_exit(size, fade_in(eased(300)), fade_out(eased(300)));
                        Box(modifier = fade.test_tag("perde"));
                        let slide = s.animate_enter_exit(
                            Modifier,
                            slide_in_vertically(eased(600), |it| it),
                            slide_out_vertically(eased(600), |it| it),
                        );
                        Box(
                            modifier = slide.size_wh(100.dp(), 40.dp()),
                            content = || {
                                Box(modifier = Modifier.fill_max_size().test_tag("sayfa"));
                            },
                        );
                    });
                },
            );
            // Shown from the start (composed after the rule has settled, to see its first frames): no entrance.
            if l.get() {
                ColumnAnimatedVisibility(
                    true,
                    enter = EnterTransition::None,
                    exit = ExitTransition::None,
                    content = |s: &AnimatedVisibilityScope| {
                        let size = Modifier.size_wh(100.dp(), 40.dp());
                        let m = s.animate_enter_exit(size, fade_in(eased(300)), fade_out(eased(300)));
                        Box(modifier = m.test_tag("baştan"));
                    },
                );
            }
        });
    }));
    rule.main_clock().set_auto_advance(false);
    let alphas = |rule: &KompozTestRule| rule.draw().layer_alphas;
    later.set(true);
    rule.advance_frame();
    rule.advance_time_by(48);
    assert!(!exists(&rule, "perde") && exists(&rule, "baştan") && alphas(&rule).is_empty());
    open.set(true);
    rule.advance_frame();
    rule.advance_time_by(48);
    // The content came in at once (EnterTransition.None); its children come in their own ways.
    let a = alphas(&rule);
    assert!(a.len() == 1 && a[0] > 0.0 && a[0] < 0.5, "{a:?}");
    let rest = bounds(&rule, "perde").bottom; // where the sliding one ends up
    assert_near(bounds(&rule, "sayfa").top, rest + 40.0, 4.0); // a whole height down
    rule.advance_time_by(350);
    assert!(alphas(&rule).is_empty()); // the fade is done
    let y = bounds(&rule, "sayfa").top;
    assert!(y > rest + 1.0 && y < rest + 39.0, "{y}"); // the slide is not
    settle(&rule);
    assert_near(bounds(&rule, "sayfa").top, rest, 0.5);
    // Hidden: the content stays until both are out.
    open.set(false);
    rule.advance_frame();
    assert!(exists(&rule, "perde") && exists(&rule, "sayfa"));
    rule.advance_time_by(350);
    let y = bounds(&rule, "sayfa").top;
    assert!(exists(&rule, "perde") && y > rest + 1.0 && y < rest + 39.0, "{y}");
    settle(&rule);
    assert!(!exists(&rule, "perde") && !exists(&rule, "sayfa"));
    // Shown again half way out: they turn back from where they are.
    open.set(true);
    settle(&rule);
    open.set(false);
    rule.advance_frame();
    rule.advance_time_by(300);
    let turned = bounds(&rule, "sayfa").top;
    open.set(true);
    rule.advance_frame(); // turned at this frame, from where it is then
    let back = bounds(&rule, "sayfa").top;
    rule.advance_time_by(48);
    let y = bounds(&rule, "sayfa").top;
    assert!(back <= turned + 4.0 && y > rest && y < back, "{turned} {back} {y}");
    settle(&rule);
    assert!(exists(&rule, "perde"));
    assert_near(bounds(&rule, "sayfa").top, rest, 0.5);
}

// ---- C++ 10864's AnimatedVisibility part --------------------------------------------------------------------------

fn mutable_transition_states_animate_from_where_they_start() {
    // (the transition part is tests/transition.rs's; Kotlin's AnimatedVisibility in a Column is ColumnScope's)
    let open = mutable_state_of(false);
    let shown = MutableTransitionState(false);
    shown.set_target_state(true); // in as it first shows
    let (o, st) = (open.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, st) = (o.clone(), st.clone());
        Column(move || {
            if !o.get() {
                return;
            }
            ColumnAnimatedVisibilityWithState(
                st.clone(),
                modifier = Modifier.test_tag("ilk"),
                enter = expand_vertically(linear(320)),
                exit = shrink_vertically(linear(320)),
                content = |_| Box(modifier = Modifier.size_wh(50.dp(), 40.dp())),
            );
            ColumnAnimatedVisibility(
                true,
                modifier = Modifier.test_tag("düz"),
                enter = expand_vertically(linear(320)),
                exit = shrink_vertically(linear(320)),
                content = |_| Box(modifier = Modifier.size_wh(50.dp(), 40.dp())),
            );
        });
    }));
    rule.main_clock().set_auto_advance(false);
    assert!(!shown.current_state() && !shown.is_idle()); // not there yet
    open.set(true);
    rule.advance_time_by(160);
    // On its way from its current state; a plain visible one shown at once.
    let h = bounds(&rule, "ilk").height();
    assert!(h > 10.0 && h < 30.0, "{h}");
    assert_near(bounds(&rule, "düz").height(), 40.0, 0.5);
    assert!(!shown.current_state() && !shown.is_idle());
    settle(&rule);
    assert_near(bounds(&rule, "ilk").height(), 40.0, 0.5);
    assert!(shown.current_state() && shown.is_idle());
    // Hidden: still there on the way out, all gone once it says so.
    shown.set_target_state(false);
    rule.advance_time_by(160);
    let h = bounds(&rule, "ilk").height();
    assert!(h > 10.0 && h < 30.0 && shown.current_state() && !shown.is_idle(), "{h}");
    settle(&rule);
    assert!(!exists(&rule, "ilk") && !shown.current_state() && shown.is_idle());
    // Turned back on the way: not idle though back where it was.
    shown.set_target_state(true);
    rule.advance_time_by(96);
    shown.set_target_state(false);
    rule.advance_frame();
    // (Compose: the current state becomes the target it turned from — C++ kept the one it came from)
    assert!(
        shown.current_state() && !shown.target_state() && !shown.is_idle(),
        "{} {} {}",
        shown.current_state(),
        shown.target_state(),
        shown.is_idle()
    );
    settle(&rule);
    assert!(shown.is_idle() && !exists(&rule, "ilk"));
}

// ---- C++ 3468's animation half (kompoz-foundation's tests/clickable.rs has the rest) -------------------------------

fn leaving_content_takes_no_taps() {
    // D5, the user's choice (C++'s, not Compose 1.12's): content on its way out through AnimatedContent or
    // AnimatedVisibility takes no input.
    let selected = mutable_state_of(-1);
    let shown = mutable_state_of(true);
    let button_clicks = Rc::new(Cell::new(0));
    let (se, sh, bc) = (selected.clone(), shown.clone(), button_clicks.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (se, sh, bc) = (se.clone(), sh.clone(), bc.clone());
            Column(move || {
                let se2 = se.clone();
                AnimatedContent(
                    se.get(),
                    modifier = Modifier.height(300.dp()),
                    content = move |_, item: i32| {
                        if item >= 0 {
                            let padding = Modifier.padding_sides_with(0.dp(), 200.dp(), 0.dp(), 0.dp());
                            Text(format!("ayrıntı {item}"), modifier = padding);
                            return;
                        }
                        let se = se2.clone();
                        Column(move || {
                            for i in 0..3 {
                                let se = se.clone();
                                key(&i, || {
                                    Box(modifier = Modifier
                                        .size_wh(100.dp(), 40.dp())
                                        .test_tag(format!("card{i}"))
                                        .clickable(move || se.set(i)));
                                });
                            }
                        });
                    },
                );
                let bc = bc.clone();
                ColumnAnimatedVisibility(
                    sh.get(),
                    content = move |_| {
                        let bc = bc.clone();
                        let m = Modifier.size_wh(100.dp(), 40.dp()).test_tag("button");
                        Box(modifier = m.clickable(move || bc.set(bc.get() + 1)));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.main_clock().set_auto_advance(false);
    let (first, second) = (center(&rule, "card0"), center(&rule, "card1"));
    tap(&rule, first);
    rule.advance_frame(); // the list is fading out
    assert!(selected.peek() == 0 && exists(&rule, "card1"));
    tap(&rule, second);
    rule.advance_frame();
    assert_eq!(selected.peek(), 0); // the second card, on its way out, didn't take the tap
    settle(&rule);
    assert!(text_exists(&rule, "ayrıntı 0") && !exists(&rule, "card1"));
    // AnimatedVisibility hiding its content: no clicks on the way out either.
    let button = center(&rule, "button");
    shown.set(false);
    rule.advance_frame();
    assert!(exists(&rule, "button"));
    tap(&rule, button);
    settle(&rule);
    assert_eq!(button_clicks.get(), 0);
}

// ---- C++ 3344, 3365, 3511 ---------------------------------------------------------------------------------------

fn animated_content_shows_both_while_switching() {
    let n = mutable_state_of(1);
    let nn = n.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        AnimatedContent(nn.get(), content = |_, v: i32| Text(format!("n={v}")));
    }));
    rule.main_clock().set_auto_advance(false);
    assert!(text_exists(&rule, "n=1"));
    n.set(2);
    rule.advance_time_by(48);
    assert!(text_exists(&rule, "n=1") && text_exists(&rule, "n=2"));
    settle(&rule);
    assert!(!text_exists(&rule, "n=1") && text_exists(&rule, "n=2"));
    // Interrupted twice in a row: only the last one stays.
    n.set(3);
    rule.advance_time_by(32);
    n.set(4);
    settle(&rule);
    assert!(text_exists(&rule, "n=4") && !text_exists(&rule, "n=3") && !text_exists(&rule, "n=2"));
    assert!(!rule.host().has_pending_work());
}

fn animated_content_puts_the_entering_content_by_its_z_index() {
    let n = mutable_state_of(1);
    let z = mutable_state_of(0.0f32); // the entering content's z-index
    let (nn, zz) = (n.clone(), z.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let zz = zz.clone();
        AnimatedContent(
            nn.get(),
            // (both faded at once: both drawn from the start)
            transition_spec = move |_| {
                let t = fade_in(eased(300)).together_with(fade_out(eased(300)));
                t.set_target_content_z_index(zz.peek());
                t
            },
            content = |_, v: i32| Text(format!("n={v}")),
        );
    }));
    rule.main_clock().set_auto_advance(false);
    let order =
        |rule: &KompozTestRule| rule.draw().texts.into_iter().rfind(|t| t.starts_with("n=")).unwrap_or_default();
    n.set(2);
    rule.advance_time_by(48);
    assert_eq!(order(&rule), "n=2"); // by default the newer content is on top
    settle(&rule);
    z.set(-1.0); // going back: it comes in under the one leaving
    n.set(1);
    rule.advance_time_by(48);
    assert!(text_exists(&rule, "n=1") && text_exists(&rule, "n=2") && order(&rule) == "n=2");
    settle(&rule);
    assert_eq!(order(&rule), "n=1");
}

fn crossfade_between_states() {
    let on = mutable_state_of(false);
    let o = on.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Crossfade(o.get(), content = |v: bool| Text(if v { "açık" } else { "kapalı" }));
    }));
    rule.main_clock().set_auto_advance(false);
    on.set(true);
    rule.advance_time_by(96);
    assert!(text_exists(&rule, "açık") && text_exists(&rule, "kapalı"));
    settle(&rule);
    assert!(text_exists(&rule, "açık") && !text_exists(&rule, "kapalı"));
}

// ---- C++ 4621 -----------------------------------------------------------------------------------------------------

type Job = std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>;

/// Runs `job` in a composition's coroutine scope, then what it set off.
fn run(rule: &KompozTestRule, scope: &CoroutineScope, job: Job) {
    scope.launch(job);
    rule.wait_for_idle();
}

fn animated_content_seeks_a_transition_part_way() {
    // Adapted (D9): seeking is Compose's, a SeekableTransitionState given to rememberTransition, its transition's
    // AnimatedContent; the container's size is seeked along (C++ kept the shown one's).
    let state = SeekableTransitionState(1);
    let scope = Rc::new(RefCell::new(None::<CoroutineScope>));
    let (st, sc) = (state.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(remember_coroutine_scope());
            let transition = remember_transition(st.clone());
            transition.AnimatedContent(
                modifier = Modifier.test_tag("kutu"),
                transition_spec = |_| fade_in(linear(200)).together_with(fade_out(linear(200))),
                content = |_, n: i32| {
                    Box(modifier = Modifier.size((n as f32 * 100.0).dp()).test_tag(format!("içerik {n}")));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let width = |rule: &KompozTestRule| bounds(rule, "kutu").width();
    let scope = scope.borrow().clone().expect("composed");
    let run = |rule: &KompozTestRule, job: Job| run(rule, &scope, job);
    // Seeked half way to 2: both there, the box between their sizes.
    run(&rule, Box::pin(named_args!(state.seek_to(0.5, target_state = 2))));
    assert!(exists(&rule, "içerik 2") && exists(&rule, "içerik 1"));
    let w = width(&rule);
    assert!(w > 100.0 && w < 200.0, "{w}");
    // Gone there: on from half way, the box growing to its size.
    run(&rule, Box::pin(state.animate_to()));
    assert!(!exists(&rule, "içerik 1") && near(width(&rule), 200.0, 0.5));
    // Seeked back toward 1 and called off: 1 goes, 2 stays as it was.
    run(&rule, Box::pin(named_args!(state.seek_to(0.3, target_state = 1))));
    assert!(exists(&rule, "içerik 1") && exists(&rule, "içerik 2"));
    run(&rule, Box::pin(state.snap_to(2)));
    assert!(!exists(&rule, "içerik 1") && near(width(&rule), 200.0, 0.5));
}

// ---- C++ 24372, 24426, 24610 ------------------------------------------------------------------------------------

fn animated_content_picks_its_transition_from_the_states_and_animates_its_size() {
    let n = mutable_state_of(5);
    let big = mutable_state_of(false);
    let sized = mutable_state_of(true);
    let (nn, b, sz) = (n.clone(), big.clone(), sized.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (nn, b, sz) = (nn.clone(), b.clone(), sz.clone());
        Column(move || {
            // Up: faded in from 0.2; down: from 0.6 (to tell them apart).
            AnimatedContent(
                nn.get(),
                modifier = Modifier.test_tag("sayaç"),
                transition_spec = |s: &AnimatedContentTransitionScope<i32>| {
                    let from = if s.target_state() > s.initial_state() { 0.2 } else { 0.6 };
                    fade_in(eased(300), from).together_with(fade_out(eased(300)))
                },
                content = |_, v: i32| Text(format!("sayı {v}")),
            );
            let with_size = sz.get();
            AnimatedContent(
                b.get(),
                modifier = Modifier.test_tag("kutu"),
                transition_spec = move |s: &AnimatedContentTransitionScope<bool>| {
                    let t = fade_in(eased(300)).together_with(fade_out(eased(300)));
                    if with_size { s.using(t, Some(SizeTransform(true, |_, _| eased(320)))) } else { s.using(t, None) }
                },
                content = |_, b: bool| {
                    let (w, h) = if b { (300.0, 150.0) } else { (100.0, 50.0) };
                    Box(modifier = Modifier.size_wh(w.dp(), h.dp()));
                },
            );
        });
    }));
    rule.main_clock().set_auto_advance(false);
    let last_alpha = |rule: &KompozTestRule| rule.draw().layer_alphas.last().copied().unwrap_or(-1.0);
    n.set(6);
    rule.advance_frame();
    let up = last_alpha(&rule);
    settle(&rule);
    n.set(4);
    rule.advance_frame();
    let down = last_alpha(&rule);
    settle(&rule);
    assert!(up > 0.15 && up < 0.4 && down > 0.55 && down < 0.8, "{up} {down}");
    assert_eq!(bounds(&rule, "kutu").width(), 100.0);
    big.set(true);
    rule.advance_frame();
    rule.advance_time_by(160);
    let half = bounds(&rule, "kutu");
    let (w, h) = (half.width(), half.height());
    assert!(w > 110.0 && w < 290.0 && h > 55.0 && h < 145.0, "{half:?}");
    let clipped = rule.draw();
    assert!(
        clipped.clips.iter().any(|r| near(r.left, half.left, 0.5) && near(r.width(), half.width(), 0.5)),
        "{:?}",
        clipped.clips
    );
    settle(&rule);
    assert!(bounds(&rule, "kutu").width() == 300.0 && bounds(&rule, "kutu").height() == 150.0);
    sized.set(false); // no SizeTransform: as big as the contents shown (both, while they cross)
    big.set(false);
    rule.advance_frame();
    rule.advance_frame();
    assert_eq!(bounds(&rule, "kutu").width(), 300.0);
    settle(&rule);
    assert_eq!(bounds(&rule, "kutu").width(), 100.0);
}

/// C++'s Screen of 24426.
#[derive(Clone, PartialEq, Debug)]
struct Screen {
    kind: i32, // 0 loading, 1 loaded
    data: i32,
}

fn animated_content_aligns_its_contents_and_keeps_same_key_content() {
    let big = mutable_state_of(false);
    let screen = mutable_state_of(Screen { kind: 0, data: 0 });
    let composed_loaded = Rc::new(Cell::new(0));
    let (b, sc, cl) = (big.clone(), screen.clone(), composed_loaded.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (b, sc, cl) = (b.clone(), sc.clone(), cl.clone());
        Column(move || {
            AnimatedContent(
                b.get(),
                modifier = Modifier.test_tag("ortalı"),
                transition_spec = |s: &AnimatedContentTransitionScope<bool>| {
                    let t = fade_in(eased(300)).together_with(fade_out(eased(300)));
                    s.using(t, Some(SizeTransform(false, |_, _| eased(300))))
                },
                content_alignment = Alignment::Center,
                content = |_, b: bool| {
                    let (w, h) = if b { (200.0, 80.0) } else { (100.0, 40.0) };
                    Box(modifier = Modifier.size_wh(w.dp(), h.dp()).test_tag(if b { "büyük" } else { "küçük" }));
                },
            );
            let cl = cl.clone();
            AnimatedContent(
                sc.get(),
                modifier = Modifier.test_tag("ekran"),
                content_key = Some(Rc::new(|s: &Screen| SlotKey::new(s.kind)) as Rc<dyn Fn(&Screen) -> SlotKey>),
                content = move |_, s: Screen| {
                    if s.kind == 0 {
                        Text("Yükleniyor");
                        return;
                    }
                    let cl = cl.clone();
                    // (once per content shown)
                    let seen = remember(move || {
                        cl.set(cl.get() + 1);
                        cl.get()
                    });
                    Text(format!("veri {} ({seen})", s.data));
                },
            );
        });
    }));
    rule.main_clock().set_auto_advance(false);
    let b0 = bounds(&rule, "ortalı");
    assert!(near(b0.width(), 100.0, 0.5) && near(b0.height(), 40.0, 0.5));
    big.set(true);
    rule.advance_frame();
    rule.advance_time_by(150);
    // Half way: the box between the sizes, both contents centered in it (the big one sticks out).
    let bx = bounds(&rule, "ortalı");
    let (small, large) = (bounds(&rule, "küçük"), bounds(&rule, "büyük"));
    assert!(bx.width() > 110.0 && bx.width() < 190.0, "{bx:?}");
    let centered = |space: f32, size: f32| ((space - size) / 2.0 + 0.5).floor(); // (whole px, a half up)
    assert_eq!(bx.width(), bx.width().floor());
    assert_eq!(small.left - bx.left, centered(bx.width(), 100.0));
    assert_eq!(small.top - bx.top, centered(bx.height(), 40.0));
    assert_eq!(large.left - bx.left, centered(bx.width(), 200.0));
    assert_eq!(large.top - bx.top, centered(bx.height(), 80.0));
    settle(&rule);
    let bx = bounds(&rule, "ortalı");
    let large = bounds(&rule, "büyük");
    assert!(near(bx.width(), 200.0, 0.5) && near(large.left, bx.left, 0.5) && !exists(&rule, "küçük"));
    // Loading → loaded is new content (a transition); new data on the loaded screen is not.
    screen.set(Screen { kind: 1, data: 7 });
    rule.advance_frame();
    rule.advance_frame();
    assert!(text_exists(&rule, "Yükleniyor") && text_exists(&rule, "veri 7 (1)")); // crossing
    // New data half way: the loaded content takes it (composed once). Adapted: Compose's transition turning to a
    // new target makes the one it was going to its current state, so the loading screen, neither, leaves at once
    // (C++ kept it until its exit was done).
    screen.set(Screen { kind: 1, data: 8 });
    rule.advance_frame();
    assert!(text_exists(&rule, "veri 8 (1)") && !text_exists(&rule, "veri 8 (2)"));
    assert!(!text_exists(&rule, "Yükleniyor"));
    settle(&rule);
    assert!(!text_exists(&rule, "Yükleniyor") && text_exists(&rule, "veri 8 (1)"));
}

fn animated_content_waits_for_its_childrens_exits() {
    let page = mutable_state_of(0);
    let p = page.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        AnimatedContent(
            p.get(),
            transition_spec = |_| fade_in(eased(100)).together_with(fade_out(eased(100))),
            content = |s: &AnimatedContentScope, p: i32| {
                let m = s.animate_enter_exit(
                    Modifier,
                    slide_in_horizontally(eased(500), |it| it),
                    slide_out_horizontally(eased(500), |it| -it),
                );
                Box(
                    modifier = m.size_wh(100.dp(), 40.dp()),
                    content = move || {
                        Box(modifier = Modifier.fill_max_size().test_tag(format!("sayfa {p}")));
                    },
                );
            },
        );
    }));
    rule.main_clock().set_auto_advance(false);
    let rest = bounds(&rule, "sayfa 0").left;
    page.set(1);
    rule.advance_frame();
    rule.advance_frame();
    let x = bounds(&rule, "sayfa 1").left;
    assert!(x > rest + 90.0, "{x}"); // coming in from the right
    rule.advance_time_by(300);
    // The old page's own fade is over; its child still slides out, so it stays.
    assert!(exists(&rule, "sayfa 0") && bounds(&rule, "sayfa 0").left < rest - 1.0);
    settle(&rule);
    assert!(!exists(&rule, "sayfa 0") && near(bounds(&rule, "sayfa 1").left, rest, 0.5));
}

// ---- Compose's behaviour ------------------------------------------------------------------------------------------

fn transitions_combine_with_plus() {
    let slide = |it: IntSize| IntOffset::new(it.width, 0);
    let enter = named_args!(fade_in(linear(100), 0.3)) + named_args!(slide_in(linear(100), slide)) + expand_in();
    let over = enter.clone() + named_args!(fade_in(linear(200), 0.6));
    // (the right side's parts over the left's; equal when their parts are)
    assert!(over != enter && over == enter.clone() + named_args!(fade_in(linear(200), 0.6)));
    assert_eq!(enter.clone() + EnterTransition::None, enter);
    assert_eq!(EnterTransition::None.to_string(), "EnterTransition.None");
    assert!(over.to_string().starts_with("EnterTransition: Fade - Fade(alpha=0.6)"), "{over}");
    let exit = fade_out() + ExitTransition::KeepUntilTransitionsFinished;
    assert!(exit.to_string().ends_with("KeepUntilTransitionsFinished - true"), "{exit}");
    assert_eq!(ExitTransition::KeepUntilTransitionsFinished.to_string(), "ExitTransition.KeepUntilTransitionsFinished");
    assert_eq!(ExitTransition::None.to_string(), "ExitTransition.None");
    // (a hold stays through `+`, either side)
    assert!((ExitTransition::KeepUntilTransitionsFinished + shrink_out()).to_string().contains("Finished - true"));
    // Content transforms: the defaults, a z-index, a size transform, the deprecated `with`.
    let t = fade_in().together_with(fade_out());
    assert!(t.target_content_z_index() == 0.0 && t.size_transform().is_some_and(|s| s.clip()));
    let t2 = named_args!(ContentTransform(fade_in(), fade_out(), target_content_z_index = 2.0, size_transform = None));
    assert!(t2.target_content_z_index() == 2.0 && t2.size_transform().is_none());
    #[allow(deprecated)]
    let t3 = fade_in().with(fade_out());
    assert_eq!(t3.target_content_enter(), fade_in());
    assert_eq!(format!("{:?}", SlideDirection::Start), "Start");
    let st = SizeTransform(false, default_size_animation_spec);
    assert!(!st.clip());
}

fn row_and_column_forms_go_across_and_down() {
    // (Compose's defaults: in a Row, fading and expanding across; in a Column, down; the visibleState forms alike)
    let shown = mutable_state_of(false);
    let state = MutableTransitionState(false);
    let (s, st) = (shown.clone(), state.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, st) = (s.clone(), st.clone());
        Column(move || {
            let (s1, st1) = (s.clone(), st.clone());
            Row(move || {
                RowAnimatedVisibility(
                    s1.get(),
                    modifier = Modifier.test_tag("satır"),
                    content = |_| {
                        Box(modifier = Modifier.size_wh(100.dp(), 40.dp()));
                    },
                );
                RowAnimatedVisibilityWithState(
                    st1.clone(),
                    modifier = Modifier.test_tag("satır durum"),
                    content = |_| {
                        Box(modifier = Modifier.size_wh(100.dp(), 40.dp()));
                    },
                );
            });
            ColumnAnimatedVisibility(
                s.get(),
                modifier = Modifier.test_tag("sütun"),
                content = |_| {
                    Box(modifier = Modifier.size_wh(100.dp(), 40.dp()));
                },
            );
            ColumnAnimatedVisibilityWithState(
                st.clone(),
                modifier = Modifier.test_tag("sütun durum"),
                content = |_| {
                    Box(modifier = Modifier.size_wh(100.dp(), 40.dp()));
                },
            );
            AnimatedVisibility(
                s.get(),
                modifier = Modifier.test_tag("düz"),
                content = |_| {
                    Box(modifier = Modifier.size_wh(100.dp(), 40.dp()));
                },
            );
        });
    }));
    rule.main_clock().set_auto_advance(false);
    shown.set(true);
    state.set_target_state(true);
    rule.advance_time_by(64);
    for tag in ["satır", "satır durum"] {
        let b = bounds(&rule, tag);
        assert!(b.width() > 1.0 && b.width() < 99.0 && near(b.height(), 40.0, 0.5), "{tag} {b:?}");
    }
    for tag in ["sütun", "sütun durum"] {
        let b = bounds(&rule, tag);
        assert!(near(b.width(), 100.0, 0.5) && b.height() > 1.0 && b.height() < 39.0, "{tag} {b:?}");
    }
    // (the plain one expands both ways, from the bottom end)
    let b = bounds(&rule, "düz");
    assert!(b.width() > 1.0 && b.width() < 99.0 && b.height() > 1.0 && b.height() < 39.0, "{b:?}");
    settle(&rule);
    assert!(state.is_idle() && state.current_state());
    for tag in ["satır", "satır durum", "sütun", "sütun durum", "düz"] {
        let b = bounds(&rule, tag);
        assert!(near(b.width(), 100.0, 0.5) && near(b.height(), 40.0, 0.5), "{tag} {b:?}");
    }
    shown.set(false);
    state.set_target_state(false);
    rule.advance_time_by(64);
    let b = bounds(&rule, "satır");
    assert!(b.width() > 1.0 && b.width() < 99.0, "{b:?}");
    let b = bounds(&rule, "sütun durum");
    assert!(b.height() > 1.0 && b.height() < 39.0, "{b:?}");
    settle(&rule);
    for tag in ["satır", "satır durum", "sütun", "sütun durum", "düz"] {
        assert!(!exists(&rule, tag), "{tag}");
    }
}

fn a_transitions_forms_follow_it() {
    // Transition<T>.AnimatedVisibility of a state's visibility, Transition<S>.AnimatedContent, Transition<T>.Crossfade
    // (its content key): all along the one transition, which runs until they are done.
    let step = mutable_state_of(0);
    let transition = Rc::new(RefCell::new(None::<Transition<i32>>));
    let (s, tr) = (step.clone(), transition.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let t = update_transition(s.get());
        *tr.borrow_mut() = Some(t.clone());
        let t2 = t.clone();
        Column(move || {
            t2.AnimatedVisibility(
                visible = |s: &i32| *s > 0,
                modifier = Modifier.test_tag("görünür"),
                enter = fade_in(linear(200)),
                exit = fade_out(linear(200)),
                content = |_| Box(modifier = Modifier.size(20.dp())),
            );
            t2.AnimatedContent(
                transition_spec = |_| fade_in(linear(160)).together_with(fade_out(linear(160))),
                content = |_, s: i32| Text(format!("adım {s}")),
            );
            t2.Crossfade(
                animation_spec = linear(240),
                content_key = Some(Rc::new(|s: &i32| SlotKey::new(*s / 2)) as Rc<dyn Fn(&i32) -> SlotKey>),
                content = |s: i32| Text(format!("geçiş {s}")),
            );
        });
    }));
    rule.main_clock().set_auto_advance(false);
    let t = transition.borrow().clone().expect("composed");
    step.set(1);
    rule.advance_time_by(96);
    assert!(exists(&rule, "görünür") && text_exists(&rule, "adım 0") && text_exists(&rule, "adım 1"));
    // (0 and 1 are of one key: the crossfade's content takes the new state, no crossing)
    assert!(text_exists(&rule, "geçiş 1") && !text_exists(&rule, "geçiş 0"));
    assert!(t.is_running());
    rule.advance_time_by(160); // the content is there (160), the visibility at 200, the crossfade's at 240
    assert!(t.is_running());
    settle(&rule);
    assert!(!t.is_running() && !text_exists(&rule, "adım 0"));
    step.set(2);
    rule.advance_time_by(96);
    assert!(text_exists(&rule, "geçiş 1") && text_exists(&rule, "geçiş 2"));
    settle(&rule);
    assert!(!text_exists(&rule, "geçiş 1") && exists(&rule, "görünür"));
    step.set(0);
    settle(&rule);
    assert!(!exists(&rule, "görünür"));
}

fn slides_scales_and_sizes_follow_their_lambdas() {
    let shown = mutable_state_of(false);
    let s = shown.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Column(move || {
                // From 200 px right and 100 down of where it rests.
                AnimatedVisibility(
                    s.get(),
                    enter = slide_in(linear(320), |it: IntSize| IntOffset::new(it.width * 2, it.height * 2)),
                    exit = slide_out(linear(320), |it: IntSize| IntOffset::new(-it.width, 0)),
                    content = |_| Box(modifier = Modifier.size_wh(100.dp(), 50.dp()).test_tag("kayan")),
                );
                // From half its size, held at the top start, not clipped.
                AnimatedVisibility(
                    s.get(),
                    modifier = Modifier.test_tag("büyüyen"),
                    enter = named_args!(expand_in(
                        linear(320),
                        expand_from = Alignment::TopStart,
                        clip = false,
                        initial_size = |it: IntSize| IntSize::new(it.width / 2, it.height / 2)
                    )),
                    exit = named_args!(shrink_horizontally(linear(320), shrink_towards = Alignment::Start)),
                    content = |_| Box(modifier = Modifier.size_wh(100.dp(), 50.dp()).test_tag("içi")),
                );
                // Scaled from 0.5 about its top start.
                AnimatedVisibility(
                    s.get(),
                    enter = scale_in(linear(320), 0.5, TransformOrigin::new(0.0, 0.0)),
                    exit = scale_out(linear(320), 0.5),
                    content = |_| Box(modifier = Modifier.size_wh(100.dp(), 50.dp()).test_tag("ölçekli")),
                );
            });
        }),
        IntSize::new(600, 600),
    );
    rule.main_clock().set_auto_advance(false);
    shown.set(true);
    rule.advance_frame();
    rule.advance_frame(); // (on its way from the frame after the composition: its start)
    let b = bounds(&rule, "kayan");
    assert!(near(b.left, 200.0, 0.5) && near(b.top, 100.0, 0.5), "{b:?}");
    rule.advance_time_by(160);
    let b = bounds(&rule, "kayan");
    assert!(near(b.left, 100.0, 7.0) && near(b.top, 50.0, 4.0), "{b:?}");
    // (half way between 50 and 100 wide; its content at the top start, not clipped: whole)
    let outer = bounds(&rule, "büyüyen");
    assert!(near(outer.width(), 75.0, 7.0) && near(outer.height(), 37.5, 4.0), "{outer:?}");
    let inner = bounds(&rule, "içi");
    assert!(near(inner.left, outer.left, 0.5) && near(inner.top, outer.top, 0.5) && near(inner.width(), 100.0, 0.5));
    assert!(rule.draw().clips.is_empty());
    settle(&rule);
    let b = bounds(&rule, "kayan");
    assert!(near(b.left, 0.0, 0.5) && near(b.top, 0.0, 0.5), "{b:?}");
    shown.set(false);
    rule.advance_frame();
    rule.advance_time_by(160);
    let b = bounds(&rule, "kayan");
    assert!(near(b.left, -50.0, 7.0), "{b:?}"); // half way to a width left
    // (shrinking across toward the start, a full height)
    let outer = bounds(&rule, "büyüyen");
    assert!(near(outer.width(), 50.0, 7.0) && near(outer.height(), 50.0, 0.5), "{outer:?}");
    settle(&rule);
    assert!(!exists(&rule, "kayan") && !exists(&rule, "içi") && !exists(&rule, "ölçekli"));
}

fn veils_are_drawn_over_the_content() {
    let shown = mutable_state_of(false);
    let s = shown.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Box(
            modifier = Modifier.size(200.dp()),
            content = move || {
                AnimatedVisibility(
                    s.get(),
                    modifier = Modifier.offset(50.dp(), 50.dp()),
                    enter = unveil_in(eased(300), Color::Red),
                    exit = named_args!(veil_out(eased(300), target_color = Color::Blue, match_parent_size = true)),
                    content = |_| Box(modifier = Modifier.size(40.dp())),
                );
            },
        );
    }));
    let veils = |rule: &KompozTestRule| {
        let c = rule.draw();
        c.rects.into_iter().zip(c.rect_colors).filter(|(_, c)| c.alpha() > 0.0).collect::<Vec<_>>()
    };
    shown.set(true);
    rule.advance_frame();
    rule.advance_frame();
    // Unveiling: red over the content, from its color to transparent.
    let v = veils(&rule);
    assert!(v.len() == 1 && near(v[0].0.left, 50.0, 0.5) && near(v[0].0.width(), 40.0, 0.5), "{v:?}");
    assert!(v[0].1.red() > 0.99 && v[0].1.alpha() > 0.9, "{v:?}");
    rule.advance_time_by(160);
    let v = veils(&rule);
    assert!(v.len() == 1 && v[0].1.alpha() < 0.5, "{v:?}");
    rule.wait_for_idle();
    assert!(veils(&rule).is_empty());
    // Veiling, over the parent: blue, toward its color.
    shown.set(false);
    rule.advance_frame();
    rule.advance_time_by(160);
    let v = veils(&rule);
    assert!(v.len() == 1 && near(v[0].0.left, 0.0, 0.5) && near(v[0].0.width(), 200.0, 0.5), "{v:?}");
    assert!(v[0].1.blue() > 0.99 && v[0].1.alpha() > 0.1, "{v:?}");
    rule.wait_for_idle();
    assert!(veils(&rule).is_empty());
}

fn contents_slide_into_and_out_of_their_container() {
    let n = mutable_state_of(0);
    let nn = n.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        AnimatedContent(
            nn.get(),
            modifier = Modifier.test_tag("kap"),
            transition_spec = |s: &AnimatedContentTransitionScope<i32>| {
                let (into, out) = if s.target_state() > s.initial_state() {
                    (SlideDirection::Left, SlideDirection::Left)
                } else {
                    (SlideDirection::Down, SlideDirection::Down)
                };
                s.slide_into_container(into, linear(320)).together_with(s.slide_out_of_container(out, linear(320)))
            },
            content = |_, n: i32| Box(modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag(format!("parça {n}"))),
        );
    }));
    rule.main_clock().set_auto_advance(false);
    let left = |rule: &KompozTestRule, tag: &str| bounds(rule, tag).left - bounds(rule, "kap").left;
    let top = |rule: &KompozTestRule, tag: &str| bounds(rule, tag).top - bounds(rule, "kap").top;
    n.set(1);
    rule.advance_frame();
    rule.advance_frame();
    // Toward the left: the new one from the container's right edge, the old one out to its left.
    assert!(near(left(&rule, "parça 1"), 100.0, 0.5) && near(left(&rule, "parça 0"), 0.0, 0.5));
    rule.advance_time_by(160);
    assert!(near(left(&rule, "parça 1"), 50.0, 7.0) && near(left(&rule, "parça 0"), -50.0, 7.0));
    settle(&rule);
    assert!(!exists(&rule, "parça 0") && near(left(&rule, "parça 1"), 0.0, 0.5));
    // Down: from above the container, the old one down out of it.
    n.set(0);
    rule.advance_frame();
    rule.advance_frame();
    assert!(near(top(&rule, "parça 0"), -40.0, 0.5) && near(top(&rule, "parça 1"), 0.0, 0.5));
    rule.advance_time_by(160);
    assert!(near(top(&rule, "parça 0"), -20.0, 4.0) && near(top(&rule, "parça 1"), 20.0, 4.0));
    settle(&rule);
    assert!(!exists(&rule, "parça 1"));
}

fn kept_until_the_transitions_finish() {
    // ExitTransition.KeepUntilTransitionsFinished: the old content stays as it is until the change is done.
    let n = mutable_state_of(0);
    let nn = n.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        AnimatedContent(
            nn.get(),
            transition_spec = |_| fade_in(linear(300)).together_with(ExitTransition::KeepUntilTransitionsFinished),
            content = |_, n: i32| Text(format!("içerik {n}")),
        );
    }));
    rule.main_clock().set_auto_advance(false);
    n.set(1);
    rule.advance_time_by(240);
    assert!(text_exists(&rule, "içerik 0") && text_exists(&rule, "içerik 1"));
    // (drawn as it is: only the new one fades)
    assert_eq!(rule.draw().layer_alphas.len(), 1);
    settle(&rule);
    assert!(!text_exists(&rule, "içerik 0"));
}

/// The lookahead size of a layout as it is placed (a LookaheadScope's coordinates).
fn lookahead_size_of(slot: Rc<Cell<(IntSize, IntSize)>>) -> Modifier {
    Modifier.on_placed(move |c| slot.set((c.to_lookahead_coordinates().size(), c.size())))
}

fn in_a_lookahead_scope_they_are_where_they_are_going() {
    // In a lookahead scope: in the lookahead pass, content going has no size (its siblings are where they will be)
    // and an AnimatedContent is its target's size; the main pass animates to those.
    let shown = mutable_state_of(true);
    let big = mutable_state_of(false);
    let column = Rc::new(Cell::new((IntSize::Zero, IntSize::Zero)));
    let content = Rc::new(Cell::new((IntSize::Zero, IntSize::Zero)));
    let (s, b, c1, c2) = (shown.clone(), big.clone(), column.clone(), content.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, b, c1, c2) = (s.clone(), b.clone(), c1.clone(), c2.clone());
        LookaheadScope(move |_| {
            let (s, b, c1, c2) = (s.clone(), b.clone(), c1.clone(), c2.clone());
            Column(
                modifier = lookahead_size_of(c1),
                content = move || {
                    ColumnAnimatedVisibility(
                        s.get(),
                        exit = shrink_vertically(linear(320)),
                        content = |_| Box(modifier = Modifier.size_wh(100.dp(), 60.dp())),
                    );
                    AnimatedContent(
                        b.get(),
                        modifier = lookahead_size_of(c2.clone()),
                        transition_spec = |s: &AnimatedContentTransitionScope<bool>| {
                            let t = fade_in(linear(320)).together_with(fade_out(linear(320)));
                            s.using(t, Some(SizeTransform(true, |_, _| linear(320))))
                        },
                        content = |_, b: bool| {
                            let w = if b { 200.0 } else { 100.0 };
                            Box(modifier = Modifier.size_wh(w.dp(), 40.dp()));
                        },
                    );
                },
            );
        });
    }));
    assert_eq!(column.get(), (IntSize::new(100, 100), IntSize::new(100, 100)));
    shown.set(false);
    big.set(true);
    rule.advance_frame();
    rule.advance_time_by(160);
    let (lookahead, main) = column.get();
    assert_eq!(lookahead.height, 40, "{lookahead:?}"); // where it is going
    assert!(main.height > 50 && main.height < 90, "{main:?}"); // on its way
    let (lookahead, main) = content.get();
    assert_eq!(lookahead.width, 200, "{lookahead:?}");
    assert!(main.width > 110 && main.width < 190, "{main:?}");
    rule.wait_for_idle();
    assert_eq!(column.get().1, IntSize::new(200, 40));
    assert_eq!(content.get().1, IntSize::new(200, 40));
}

fn a_deferred_visibility_is_moved_by_hand_then_handed_over() {
    // DeferredAnimatedVisibility: while its state defers, the MutableTransform's block moves the content (here as
    // a gesture would, the offset and alpha set from outside); the transition then takes over from where it is.
    let state = DeferredTransitionState(true);
    let drag = mutable_state_of(0);
    let transform = MutableTransform();
    let d = drag.clone();
    transform.update(move |scope: &mut dyn MutableTransformScope, _: IntSize| {
        scope.set_offset(IntOffset::new(d.get(), 0));
        scope.set_alpha(1.0 - d.get() as f32 / 200.0);
    });
    let (st, tf) = (state.clone(), transform.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let t = remember_deferred_transition(st.clone());
        t.DeferredAnimatedVisibility(
            visible = |v: &bool| *v,
            enter = fade_in(linear(200)),
            exit = slide_out(linear(320), |it: IntSize| IntOffset::new(it.width * 2, 0)),
            mutable_transform = Some(tf.clone()),
            content = |_| Box(modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("sürüklenen")),
        );
    }));
    rule.main_clock().set_auto_advance(false);
    assert_near(bounds(&rule, "sürüklenen").left, 0.0, 0.5);
    state.defer(false);
    drag.set(60);
    rule.advance_frame();
    rule.advance_frame();
    // By hand: where the gesture put it, faded as it says.
    assert_near(bounds(&rule, "sürüklenen").left, 60.0, 0.5);
    let a = rule.draw().layer_alphas;
    assert!(a.len() == 1 && near(a[0], 0.7, 0.01), "{a:?}");
    drag.set(100);
    rule.advance_frame();
    assert_near(bounds(&rule, "sürüklenen").left, 100.0, 0.5);
    // Let go: the transition takes over from there, out to two widths right.
    state.animate_to(false);
    rule.advance_frame();
    rule.advance_frame();
    let x = bounds(&rule, "sürüklenen").left;
    assert!((95.0..140.0).contains(&x), "{x}");
    rule.advance_time_by(160);
    let x2 = bounds(&rule, "sürüklenen").left;
    assert!(x2 > x + 20.0, "{x} {x2}");
    settle(&rule);
    assert!(!exists(&rule, "sürüklenen"));
}

fn a_deferred_content_moves_both_contents_by_hand() {
    // DeferredAnimatedContent: while its state defers to the next page, the pending page is composed and both are
    // moved by the MutableContentTransform's blocks; then the transition takes the change over.
    let state = DeferredTransitionState(0);
    let progress = mutable_state_of(0);
    let (st, p) = (state.clone(), progress.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let t = remember_deferred_transition(st.clone());
            let p = p.clone();
            t.DeferredAnimatedContent(
                modifier = Modifier.test_tag("kap"),
                transition_spec = |_| fade_in(linear(200)).together_with(fade_out(linear(200))),
                mutable_transform_spec = move |_| {
                    let (p1, p2) = (p.clone(), p.clone());
                    Some(named_args!(MutableContentTransform(
                        block = move |m: &MutableContentTransform| {
                            let (p1, p2) = (p1.clone(), p2.clone());
                            m.initial_content_transform(move |s: &mut dyn MutableTransformScope, _| {
                                s.set_offset(IntOffset::new(-p1.get(), 0));
                            });
                            m.target_content_transform(move |s: &mut dyn MutableTransformScope, full: IntSize| {
                                s.set_offset(IntOffset::new(full.width - p2.get(), 0));
                            });
                        }
                    )))
                },
                content =
                    |_, n: i32| Box(modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag(format!("sayfa {n}"))),
            );
        }),
        IntSize::new(400, 400),
    );
    let left = |rule: &KompozTestRule, tag: &str| bounds(rule, tag).left - bounds(rule, "kap").left;
    state.defer(1);
    progress.set(30);
    rule.advance_frame();
    rule.advance_frame();
    assert!(exists(&rule, "sayfa 1"));
    assert!(near(left(&rule, "sayfa 0"), -30.0, 0.5) && near(left(&rule, "sayfa 1"), 70.0, 0.5));
    progress.set(50);
    rule.advance_frame();
    assert!(near(left(&rule, "sayfa 0"), -50.0, 0.5) && near(left(&rule, "sayfa 1"), 50.0, 0.5));
    state.animate_to(1);
    rule.wait_for_idle();
    assert!(!exists(&rule, "sayfa 0") && near(left(&rule, "sayfa 1"), 0.0, 0.5));
}

fn leaving_content_takes_no_focus_moves() {
    // D5's focus part (C++'s collectFocusables skipped leaving content): a focus move passes content on its way out.
    let shown = mutable_state_of(true);
    let s = shown.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Column(move || {
            Box(modifier = Modifier.size(20.dp()).test_tag("ilk").focusable());
            ColumnAnimatedVisibility(
                s.get(),
                exit = fade_out(linear(320)),
                content = |_| {
                    Box(modifier = Modifier.size(20.dp()).test_tag("giden").focusable());
                },
            );
            Box(modifier = Modifier.size(20.dp()).test_tag("son").focusable());
        });
    }));
    rule.main_clock().set_auto_advance(false);
    rule.on_node_with_tag("ilk").request_focus();
    settle(&rule);
    assert!(rule.host().move_focus(kompoz_ui::focus::FocusDirection::Next));
    settle(&rule);
    rule.on_node_with_tag("giden").assert_is_focused();
    rule.on_node_with_tag("ilk").request_focus();
    shown.set(false);
    rule.advance_frame();
    rule.advance_frame();
    assert!(exists(&rule, "giden"));
    assert!(rule.host().move_focus(kompoz_ui::focus::FocusDirection::Next));
    rule.advance_frame();
    rule.on_node_with_tag("son").assert_is_focused();
    assert!(rule.host().move_focus(kompoz_ui::focus::FocusDirection::Previous));
    rule.advance_frame();
    rule.on_node_with_tag("ilk").assert_is_focused();
}

tests!(
    transitions_combine_with_plus,
    row_and_column_forms_go_across_and_down,
    a_transitions_forms_follow_it,
    slides_scales_and_sizes_follow_their_lambdas,
    veils_are_drawn_over_the_content,
    contents_slide_into_and_out_of_their_container,
    kept_until_the_transitions_finish,
    in_a_lookahead_scope_they_are_where_they_are_going,
    a_deferred_visibility_is_moved_by_hand_then_handed_over,
    a_deferred_content_moves_both_contents_by_hand,
    leaving_content_takes_no_focus_moves,
    animated_visibility_expands_then_disposes_on_exit,
    scale_in_animates_through_a_transform,
    transitions_compose_their_caller_as_they_start_and_end,
    animate_enter_exit_gives_children_their_own_transitions,
    mutable_transition_states_animate_from_where_they_start,
    leaving_content_takes_no_taps,
    animated_content_shows_both_while_switching,
    animated_content_puts_the_entering_content_by_its_z_index,
    crossfade_between_states,
    animated_content_seeks_a_transition_part_way,
    animated_content_picks_its_transition_from_the_states_and_animates_its_size,
    animated_content_aligns_its_contents_and_keeps_same_key_content,
    animated_content_waits_for_its_childrens_exits,
);

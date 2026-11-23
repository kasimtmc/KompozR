//! Material's swipe to dismiss and pull to refresh as Compose's material3 (1.5.0-alpha17) has them: the C++
//! Kompoz's tests by name (kompoz_tests.cpp; M15's behaviour where the C++ differed, said at each), Compose's
//! behaviour (the state's values, constructors and savers, the directions allowed, the fling, the callback; the
//! pull's state, its taking and giving back of scrolls, the box, the indicators and their box, the defaults), and
//! the M15 samples of SwipeToDismissSamples.kt and PullToRefreshSamples.kt (+ LoadingIndicatorSamples.kt's
//! LoadingIndicatorPullToRefreshSample), each a Scaffold with a TopAppBar.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::{
    Animatable, AnimationVector1D, Easing, LinearOutSlowInEasing, Spring, VectorConverter, animate_color_as_state,
    spring,
};
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::gestures::LocalFuture;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_ui::input::PointerType;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::{ProgressBarRangeInfo, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    // (swipe to dismiss)
    swipe_to_dismiss_confirms_or_springs_back,
    rtl_drawer_and_swipe_to_dismiss_follow_the_mirror,
    swipe_to_dismiss_settles_by_its_threshold_and_the_fling,
    swipe_to_dismiss_goes_only_where_it_is_allowed,
    swipe_to_dismiss_box_states_move_from_code_and_save_their_value,
    swipe_to_dismiss_box_calls_on_dismiss_once_settled,
    swipe_to_dismiss_list_items_sample,
    // (pull to refresh)
    pull_to_refresh_refreshes_past_the_threshold,
    pull_to_refresh_goes_back_in_before_the_list_scrolls,
    pull_to_refresh_modifier_slows_past_the_threshold_and_can_be_turned_off,
    pull_to_refresh_box_can_be_turned_off,
    pull_to_refresh_ignores_pulls_while_its_state_animates,
    pull_to_refresh_lets_an_animation_the_app_starts_win_over_the_finger,
    pull_to_refresh_keeps_a_fling_down_from_the_list,
    pull_to_refresh_starts_where_is_refreshing_says,
    pull_to_refresh_states_animate_snap_and_save_their_fraction,
    pull_to_refresh_takes_a_pull_only_from_the_start_and_by_the_user,
    pull_to_refresh_indicator_comes_out_from_under_the_top_edge,
    pull_to_refresh_indicator_boxes_come_out_from_under_the_top_edge,
    pull_to_refresh_loading_indicator_morphs_with_the_pull_and_turns_past_it,
    pull_to_refresh_box_takes_an_indicator_of_its_own,
    pull_to_refresh_defaults_follow_the_tokens,
    // (M15's pull-to-refresh samples)
    pull_to_refresh_sample,
    pull_to_refresh_with_loading_indicator_sample,
    pull_to_refresh_view_model_sample,
    pull_to_refresh_scaling_sample,
    pull_to_refresh_linear_progress_indicator_sample,
    pull_to_refresh_sample_custom_state,
    pull_to_refresh_custom_indicator_with_default_transform,
    loading_indicator_pull_to_refresh_sample,
);

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

/// The C++ tests' `rule.swipe(from, to, frames)`: a move each frame, up at once (a fling).
fn swipe(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        let t = i as f32 / frames as f32;
        h.pointer_move(from + (to - from) * t, -1, 0);
    }
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

type Slot<T> = Rc<RefCell<Option<T>>>;

fn slot<T>() -> Slot<T> {
    Rc::new(RefCell::new(None))
}

fn get<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

fn launch(scope: &Slot<CoroutineScope>, f: impl std::future::Future<Output = ()> + 'static) {
    named_args!(get(scope).launch(f));
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn some(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
}

// ---- SwipeToDismissBox ----------------------------------------------------------------------------------------------

fn swipe_to_dismiss_confirms_or_springs_back() {
    // (the C++'s `rememberSwipeToDismissBoxState(confirmValueChange)`: M15's deprecated overload)
    let confirmed = Rc::new(RefCell::new(Vec::<SwipeToDismissBoxValue>::new()));
    let allow = Rc::new(Cell::new(true));
    let (state, scope) = (slot::<SwipeToDismissBoxState>(), slot::<CoroutineScope>());
    let rule = {
        let (confirmed, allow, state, scope) = (confirmed.clone(), allow.clone(), state.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let (confirmed, allow) = (confirmed.clone(), allow.clone());
                let s = remember_swipe_to_dismiss_box_state_confirming(
                    confirm_value_change = move |v| {
                        confirmed.borrow_mut().push(v);
                        allow.get()
                    },
                );
                *state.borrow_mut() = Some(s.clone());
                SwipeToDismissBox(
                    s,
                    || Box(modifier = Modifier.fill_max_size().test_tag("background")),
                    content = || Box(modifier = Modifier.fill_max_width().height(60.dp()).test_tag("row")),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let row_x = |rule: &KompozTestRule| bounds(rule, "row").left;
    // What was asked since `from` (an index into `confirmed`), but Settled.
    let asked_since = |from: usize| -> Vec<SwipeToDismissBoxValue> {
        confirmed.borrow()[from..].iter().copied().filter(|v| *v != SwipeToDismissBoxValue::Settled).collect()
    };
    // Covered at rest, the content's size (M15 composes the background always, behind the content: the C++ composed
    // it only while dragged).
    assert_eq!(bounds(&rule, "background"), Rect::new(0.0, 0.0, 400.0, 60.0));
    let h = rule.host();
    h.pointer_down(Offset::new(300.0, 30.0), -1, 0, PointerType::Touch);
    rule.advance_frame();
    h.pointer_move(Offset::new(280.0, 30.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "background"), Rect::new(0.0, 0.0, 400.0, 60.0)); // while dragged, the content's size
    assert!(row_x(&rule) < 0.0, "dragged with the finger: {}", row_x(&rule));
    h.pointer_up(Offset::new(280.0, 30.0), -1, 0);
    rule.wait_for_idle();
    // Slowly, short of 56 dp: back. (The C++ dragged 80 of 400: past M15's 56 dp threshold once the 8 dp slop is
    // taken, which settles away in Compose; 50 here.)
    swipe(&rule, Offset::new(300.0, 30.0), Offset::new(250.0, 30.0), 60);
    rule.wait_for_idle();
    assert!(asked_since(0).is_empty() && near(row_x(&rule), 0.0), "{:?} {}", confirmed.borrow(), row_x(&rule));
    // Far, then flung back: back in place. (Compose's state asks to confirm the value a drag passes half way to —
    // EndToStart here, 232 of 400 — and the one it settles on, Settled: the C++ asked for neither.)
    let from = confirmed.borrow().len();
    h.pointer_down(Offset::new(300.0, 30.0), -1, 0, PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(Offset::new(300.0 - 12.0 * i as f32, 30.0), -1, 0);
    }
    for i in 1..=3 {
        rule.advance_frame();
        h.pointer_move(Offset::new(60.0 + 20.0 * i as f32, 30.0), -1, 0);
    }
    h.pointer_up(Offset::new(120.0, 30.0), -1, 0);
    rule.wait_for_idle();
    assert!(near(row_x(&rule), 0.0), "{:?} {}", confirmed.borrow(), row_x(&rule));
    assert!(get(&state).current_value() == SwipeToDismissBoxValue::Settled);
    assert_eq!(asked_since(from), vec![SwipeToDismissBoxValue::EndToStart]);
    assert_eq!(confirmed.borrow().last(), Some(&SwipeToDismissBoxValue::Settled));
    // Far, but refused.
    allow.set(false);
    let from = confirmed.borrow().len();
    swipe(&rule, Offset::new(300.0, 30.0), Offset::new(60.0, 30.0), 20);
    rule.wait_for_idle();
    assert!(
        asked_since(from).iter().all(|v| *v == SwipeToDismissBoxValue::EndToStart) && !asked_since(from).is_empty()
    );
    assert!(get(&state).current_value() == SwipeToDismissBoxValue::Settled && near(row_x(&rule), 0.0));
    allow.set(true);
    let from = confirmed.borrow().len();
    swipe(&rule, Offset::new(300.0, 30.0), Offset::new(60.0, 30.0), 20);
    rule.wait_for_idle();
    assert_eq!(asked_since(from).last(), Some(&SwipeToDismissBoxValue::EndToStart));
    assert!(get(&state).current_value() == SwipeToDismissBoxValue::EndToStart && near(get(&state).progress(), 1.0));
    assert!(near(row_x(&rule), -400.0));
    // A short flick (60 of 400) away is enough, as in Compose (125 dp/s).
    let s = get(&state);
    launch(&scope, async move { s.reset().await });
    rule.wait_for_idle();
    let from = confirmed.borrow().len();
    swipe(&rule, Offset::new(100.0, 30.0), Offset::new(160.0, 30.0), 4);
    rule.wait_for_idle();
    let asked = asked_since(from);
    assert!(!asked.is_empty() && asked.iter().all(|v| *v == SwipeToDismissBoxValue::StartToEnd), "{asked:?}");
    assert!(get(&state).current_value() == SwipeToDismissBoxValue::StartToEnd && near(get(&state).progress(), 1.0));
    assert!(near(row_x(&rule), 400.0));
}

fn rtl_drawer_and_swipe_to_dismiss_follow_the_mirror() {
    // The swipe to dismiss half of the C++ test; its ModalNavigationDrawer half is WP4b's (navigation drawers).
    let swipe_state = slot::<SwipeToDismissBoxState>();
    let rule = {
        let swipe_state = swipe_state.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                    let s = remember_swipe_to_dismiss_box_state();
                    *swipe_state.borrow_mut() = Some(s.clone());
                    Column(
                        modifier = Modifier.padding_sides_with(0.dp(), 300.dp(), 0.dp(), 0.dp()),
                        content = || {
                            SwipeToDismissBox(
                                s.clone(),
                                || Box(modifier = Modifier.fill_max_size()),
                                content =
                                    || Box(modifier = Modifier.fill_max_width().height(60.dp()).test_tag("satır")),
                            );
                        },
                    );
                });
            }),
            IntSize::new(400, 800),
        )
    };
    // Swiped to the left in a right-to-left layout: from the start to the end.
    swipe(&rule, Offset::new(300.0, 330.0), Offset::new(40.0, 330.0), 20);
    rule.wait_for_idle();
    assert_eq!(get(&swipe_state).current_value(), SwipeToDismissBoxValue::StartToEnd);
    assert!(bounds(&rule, "satır").right <= 1.0, "{:?}", bounds(&rule, "satır"));
}

/// A SwipeToDismissBox of a 400 × 60 row (tagged "row", its background "background") at the top of a 400 × 800
/// window, its state made by `make` in the composition; the state and a coroutine scope kept in slots.
fn swipe_box(
    make: impl Fn() -> SwipeToDismissBoxState + 'static,
    start_to_end: MutableState<bool>,
    end_to_start: MutableState<bool>,
    gestures: MutableState<bool>,
) -> (KompozTestRule, Slot<SwipeToDismissBoxState>, Slot<CoroutineScope>) {
    let (state, scope) = (slot::<SwipeToDismissBoxState>(), slot::<CoroutineScope>());
    let make = Rc::new(make);
    let rule = {
        let (state, scope) = (state.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let s = make();
                *state.borrow_mut() = Some(s.clone());
                SwipeToDismissBox(
                    s,
                    || Box(modifier = Modifier.fill_max_size().test_tag("background")),
                    enable_dismiss_from_start_to_end = start_to_end.get(),
                    enable_dismiss_from_end_to_start = end_to_start.get(),
                    gestures_enabled = gestures.get(),
                    content = || Box(modifier = Modifier.fill_max_width().height(60.dp()).test_tag("row")),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    (rule, state, scope)
}

/// A slow drag (no fling) from `from` to `to` over 60 frames, held 200 ms, then up and settled.
fn slow_drag(rule: &KompozTestRule, from: Offset, to: Offset) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=60 {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / 60.0), -1, 0);
    }
    rule.advance_time_by(200);
    h.pointer_up(to, -1, 0);
    rule.wait_for_idle();
}

fn yes() -> MutableState<bool> {
    mutable_state_of(true)
}

fn swipe_to_dismiss_settles_by_its_threshold_and_the_fling() {
    // The primary constructor's state: the box's fling behavior (AnchoredDraggableDefaults.flingBehavior) with the
    // state's positional threshold, by default 56 dp whatever the distance.
    let (rule, state, scope) = swipe_box(|| remember_swipe_to_dismiss_box_state(), yes(), yes(), yes());
    let s = get(&state);
    assert!(s.current_value() == SwipeToDismissBoxValue::Settled);
    assert!(s.dismiss_direction() == SwipeToDismissBoxValue::Settled && near(s.require_offset(), 0.0));
    // Slowly 50 px (42 past the 8 px slop): back.
    slow_drag(&rule, Offset::new(300.0, 30.0), Offset::new(250.0, 30.0));
    assert!(s.current_value() == SwipeToDismissBoxValue::Settled && near(bounds(&rule, "row").left, 0.0));
    // Held 82 px out: dismissing that way, not half way yet.
    let h = rule.host();
    h.pointer_down(Offset::new(300.0, 30.0), -1, 0, PointerType::Touch);
    for i in 1..=30 {
        rule.advance_frame();
        h.pointer_move(Offset::new(300.0 - 3.0 * i as f32, 30.0), -1, 0);
    }
    let held = s.require_offset();
    assert!(held < -76.0 && held > -84.0, "82 px less the slop's: {held}");
    // (the target is the closest anchor, foundation's AnchoredDraggableState.targetValue: not yet the next one)
    assert!(s.target_value() == SwipeToDismissBoxValue::Settled);
    assert!(s.dismiss_direction() == SwipeToDismissBoxValue::EndToStart);
    assert_eq!(s.current_value(), SwipeToDismissBoxValue::Settled);
    assert_eq!(s.progress(), 1.0); // (from the settled value to the target: the same one)
    // Past half way: the target and the current value are the next one, the progress from the settled one to it.
    for i in 1..=50 {
        rule.advance_frame();
        h.pointer_move(Offset::new(210.0 - 3.0 * i as f32, 30.0), -1, 0);
    }
    let held = s.require_offset();
    assert!(
        s.target_value() == SwipeToDismissBoxValue::EndToStart
            && s.current_value() == SwipeToDismissBoxValue::EndToStart
    );
    assert!((s.progress() - held.abs() / 400.0).abs() < 0.001, "{} at {held}", s.progress());
    assert_eq!(s.settled_value(), SwipeToDismissBoxValue::Settled);
    rule.advance_time_by(200);
    h.pointer_up(Offset::new(60.0, 30.0), -1, 0);
    rule.wait_for_idle();
    assert!(s.current_value() == SwipeToDismissBoxValue::EndToStart);
    assert!(s.settled_value() == SwipeToDismissBoxValue::EndToStart && near(bounds(&rule, "row").left, -400.0));
    // Back from code; a quick flick of 30 px (22 past the slop) to the end is past 125 dp/s: dismissed that way.
    let back = s.clone();
    launch(&scope, async move { back.reset().await });
    rule.wait_for_idle();
    assert!(s.current_value() == SwipeToDismissBoxValue::Settled && near(bounds(&rule, "row").left, 0.0));
    swipe(&rule, Offset::new(100.0, 30.0), Offset::new(130.0, 30.0), 3);
    rule.wait_for_idle();
    assert!(s.current_value() == SwipeToDismissBoxValue::StartToEnd && near(bounds(&rule, "row").left, 400.0));
    assert_eq!(s.dismiss_direction(), SwipeToDismissBoxValue::StartToEnd);

    // A threshold of its own: half the distance (200 of 400).
    let (rule, state, _) = swipe_box(
        || {
            named_args!(remember_swipe_to_dismiss_box_state(
                positional_threshold = Rc::new(|d: f32| d / 2.0) as Rc<dyn Fn(f32) -> f32>
            ))
        },
        yes(),
        yes(),
        yes(),
    );
    slow_drag(&rule, Offset::new(300.0, 30.0), Offset::new(130.0, 30.0)); // 162: short of 200
    assert_eq!(get(&state).current_value(), SwipeToDismissBoxValue::Settled);
    slow_drag(&rule, Offset::new(300.0, 30.0), Offset::new(80.0, 30.0)); // 212: past it
    assert_eq!(get(&state).current_value(), SwipeToDismissBoxValue::EndToStart);
}

fn swipe_to_dismiss_goes_only_where_it_is_allowed() {
    let (end_to_start, gestures) = (mutable_state_of(false), yes());
    let (rule, state, _) =
        swipe_box(|| remember_swipe_to_dismiss_box_state(), yes(), end_to_start.clone(), gestures.clone());
    // Not from the end to the start: dragged that way, it doesn't move (its only anchors are at 0 and 400).
    swipe(&rule, Offset::new(300.0, 30.0), Offset::new(60.0, 30.0), 20);
    rule.wait_for_idle();
    assert!(get(&state).current_value() == SwipeToDismissBoxValue::Settled && near(bounds(&rule, "row").left, 0.0));
    // Allowed again (the anchors change with it); gestures off: no drag either way.
    end_to_start.set(true);
    gestures.set(false);
    rule.wait_for_idle();
    swipe(&rule, Offset::new(300.0, 30.0), Offset::new(60.0, 30.0), 20);
    swipe(&rule, Offset::new(60.0, 30.0), Offset::new(300.0, 30.0), 20);
    rule.wait_for_idle();
    assert!(get(&state).current_value() == SwipeToDismissBoxValue::Settled && near(bounds(&rule, "row").left, 0.0));
    gestures.set(true);
    rule.wait_for_idle();
    swipe(&rule, Offset::new(300.0, 30.0), Offset::new(60.0, 30.0), 20);
    rule.wait_for_idle();
    assert_eq!(get(&state).current_value(), SwipeToDismissBoxValue::EndToStart);
    // Dismissed: it takes no drags until it is settled again.
    swipe(&rule, Offset::new(100.0, 30.0), Offset::new(390.0, 30.0), 20);
    rule.wait_for_idle();
    assert!(get(&state).current_value() == SwipeToDismissBoxValue::EndToStart);
    assert!(near(bounds(&rule, "row").left, -400.0));
    // The direction it is dismissed in no longer allowed: the new anchors' target is Settled.
    end_to_start.set(false);
    rule.wait_for_idle();
    assert!(get(&state).current_value() == SwipeToDismissBoxValue::Settled && near(bounds(&rule, "row").left, 0.0));
}

fn swipe_to_dismiss_box_states_move_from_code_and_save_their_value() {
    let (rule, state, scope) = swipe_box(
        || named_args!(remember_swipe_to_dismiss_box_state(initial_value = SwipeToDismissBoxValue::EndToStart)),
        yes(),
        yes(),
        yes(),
    );
    // Placed where its initial value is.
    let s = get(&state);
    assert!(s.current_value() == SwipeToDismissBoxValue::EndToStart && near(s.require_offset(), -400.0));
    assert!(near(bounds(&rule, "row").left, -400.0) && s.dismiss_direction() == SwipeToDismissBoxValue::EndToStart);
    // Animated to the other side: on its way, the target is that side; done, settled there.
    let t = s.clone();
    launch(&scope, async move { t.dismiss(SwipeToDismissBoxValue::StartToEnd).await });
    rule.advance_frame();
    rule.advance_frame();
    assert_eq!(s.target_value(), SwipeToDismissBoxValue::StartToEnd);
    rule.wait_for_idle();
    assert!(s.settled_value() == SwipeToDismissBoxValue::StartToEnd && near(bounds(&rule, "row").left, 400.0));
    // Snapped: at once.
    let t = s.clone();
    launch(&scope, async move { t.snap_to(SwipeToDismissBoxValue::Settled).await });
    rule.advance_frame();
    assert!(s.current_value() == SwipeToDismissBoxValue::Settled && near(s.require_offset(), 0.0));
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "row").left, 0.0) && s.dismiss_direction() == SwipeToDismissBoxValue::Settled);

    // Saved as its current value; restored, a state of it (both savers).
    let threshold: Rc<dyn Fn(f32) -> f32> = Rc::new(|_| 10.0);
    let saver = SwipeToDismissBoxState::Saver(threshold.clone());
    let saved = saver.save(&SwipeToDismissBoxState(SwipeToDismissBoxValue::StartToEnd, threshold.clone()));
    assert_eq!(saver.restore(&saved).map(|s| s.current_value()), Some(SwipeToDismissBoxValue::StartToEnd));
    let density = Density::new(2.0, 1.0);
    let saver = SwipeToDismissBoxState::saver_confirming(|_| true, threshold.clone(), density);
    let deprecated =
        swipe_to_dismiss_box_state_confirming(SwipeToDismissBoxValue::EndToStart, density, threshold.clone());
    let restored = saver.restore(&saver.save(&deprecated)).map(|s| s.current_value());
    assert_eq!(restored, Some(SwipeToDismissBoxValue::EndToStart));
    for v in SwipeToDismissBoxValue::ENTRIES {
        assert_eq!(SwipeToDismissBoxValue::restore(&v.save()), Some(v));
    }
    // The states are handles: clones are the same state.
    assert!(s == s.clone() && s != SwipeToDismissBoxState(SwipeToDismissBoxValue::Settled, threshold));
    // Before it is laid out, its offset isn't known (Compose throws).
    let unplaced = SwipeToDismissBoxState(SwipeToDismissBoxValue::Settled, Rc::new(|_| 0.0));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unplaced.require_offset())).is_err());
}

fn swipe_to_dismiss_box_calls_on_dismiss_once_settled() {
    let dismissed = Rc::new(RefCell::new(Vec::<SwipeToDismissBoxValue>::new()));
    let (state, scope) = (slot::<SwipeToDismissBoxState>(), slot::<CoroutineScope>());
    let recompose = mutable_state_of(0);
    let threshold = slot::<f32>();
    let rule = {
        let (dismissed, state, scope, recompose, threshold) =
            (dismissed.clone(), state.clone(), scope.clone(), recompose.clone(), threshold.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                *threshold.borrow_mut() = Some(SwipeToDismissBoxDefaults::positional_threshold()(1000.0));
                let s = remember_swipe_to_dismiss_box_state();
                *state.borrow_mut() = Some(s.clone());
                Text(format!("{}", recompose.get()));
                let dismissed = dismissed.clone();
                SwipeToDismissBox(
                    s,
                    || {},
                    on_dismiss = move |direction| dismissed.borrow_mut().push(direction),
                    content = || Box(modifier = Modifier.fill_max_width().height(60.dp()).test_tag("row")),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    // (the default threshold: 56 dp whatever the distance)
    assert!(near(get(&threshold), 56.0 * rule.density().density));
    swipe(&rule, Offset::new(300.0, 30.0), Offset::new(60.0, 30.0), 20);
    rule.wait_for_idle();
    assert_eq!(*dismissed.borrow(), vec![SwipeToDismissBoxValue::EndToStart]);
    // Composed again with the same callback (by what it captures): not called again.
    recompose.set(1);
    rule.wait_for_idle();
    assert_eq!(dismissed.borrow().len(), 1);
    // Back from code: not called for Settled; dismissed the other way from code: called with it.
    let s = get(&state);
    launch(&scope, async move { s.reset().await });
    rule.wait_for_idle();
    assert_eq!(dismissed.borrow().len(), 1);
    let s = get(&state);
    launch(&scope, async move { s.dismiss(SwipeToDismissBoxValue::StartToEnd).await });
    rule.wait_for_idle();
    assert_eq!(*dismissed.borrow(), vec![SwipeToDismissBoxValue::EndToStart, SwipeToDismissBoxValue::StartToEnd]);
}

// ---- The M15 samples (SwipeToDismissSamples.kt) ---------------------------------------------------------------------

#[kompozable]
fn SwipeToDismissListItems() {
    let dismiss_state = remember_swipe_to_dismiss_box_state();
    let is_visible = remember(|| mutable_state_of(true));
    let scope = remember_coroutine_scope();
    if is_visible.get() {
        let background_state = dismiss_state.clone();
        let (state, visible) = (dismiss_state.clone(), is_visible.clone());
        SwipeToDismissBox(
            state = dismiss_state,
            background_content = move || {
                let color = animate_color_as_state(match background_state.target_value() {
                    SwipeToDismissBoxValue::Settled => Color::LightGray,
                    SwipeToDismissBoxValue::StartToEnd => Color::Green,
                    SwipeToDismissBoxValue::EndToStart => Color::Red,
                });
                Box(modifier = Modifier.fill_max_size().background(color.get()));
            },
            on_dismiss = move |direction| {
                if direction == SwipeToDismissBoxValue::EndToStart {
                    visible.set(false);
                } else {
                    let state = state.clone();
                    named_args!(scope.launch(async move { state.reset().await }));
                }
            },
            content = || {
                OutlinedCard(
                    shape = Arc::new(RectangleShape),
                    content = || {
                        ListItem(
                            headline_content = || Text("Cupcake"),
                            supporting_content = some(|| Text("Swipe me left or right!")),
                        );
                    },
                );
            },
        );
    }
}

/// Whether a rect of about `color` is drawn (an animated color comes back through its color space's floats).
fn drew_near(rule: &KompozTestRule, color: Color) -> bool {
    rule.draw().rect_colors.iter().any(|c| {
        (c.red() - color.red()).abs() < 0.01
            && (c.green() - color.green()).abs() < 0.01
            && (c.blue() - color.blue()).abs() < 0.01
            && (c.alpha() - color.alpha()).abs() < 0.01
    })
}

fn swipe_to_dismiss_list_items_sample() {
    let rule = KompozTestRule::with_viewport(SwipeToDismissListItems, IntSize::new(400, 800));
    let item = rule.on_node_with_text("Cupcake").get_unclipped_bounds_in_root();
    let y = item.center().y;
    // At rest: light gray behind (covered by the card).
    assert!(rule.draw().rect_colors.contains(&Color::LightGray));
    // Swiped right: green behind while it goes; dismissed that way, reset back.
    let h = rule.host();
    h.pointer_down(Offset::new(100.0, y), -1, 0, PointerType::Touch);
    for i in 1..=25 {
        rule.advance_frame();
        h.pointer_move(Offset::new(100.0 + 10.0 * i as f32, y), -1, 0);
    }
    rule.advance_time_by(600);
    assert!(drew_near(&rule, Color::Green)); // (past half way: the target is the end)
    h.pointer_up(Offset::new(350.0, y), -1, 0);
    rule.wait_for_idle();
    assert!(near(rule.on_node_with_text("Cupcake").get_unclipped_bounds_in_root().left, item.left));
    // Swiped left: red behind; dismissed that way, the item is gone.
    h.pointer_down(Offset::new(350.0, y), -1, 0, PointerType::Touch);
    for i in 1..=25 {
        rule.advance_frame();
        h.pointer_move(Offset::new(350.0 - 10.0 * i as f32, y), -1, 0);
    }
    rule.advance_time_by(600);
    assert!(drew_near(&rule, Color::Red));
    h.pointer_up(Offset::new(100.0, y), -1, 0);
    rule.wait_for_idle();
    rule.on_node_with_text("Cupcake").assert_does_not_exist();
}

// ---- Pull to refresh ------------------------------------------------------------------------------------------------

/// The touch slop (KompozR's and Compose's 8 dp; the C++'s was 10).
const Slop: f32 = 8.0;

/// Where the state is after a pull of `px` (past the slop) with a threshold of `threshold` px: half the pull, slowed
/// past the threshold, at most twice it (Compose's `calculateVerticalOffset`), as a fraction of the threshold.
fn pulled_fraction(px: f32, threshold: f32) -> f32 {
    let adjusted = px * 0.5;
    if adjusted <= threshold {
        return adjusted / threshold;
    }
    let tension = (adjusted / threshold - 1.0).clamp(0.0, 2.0);
    (threshold + threshold * (tension - tension * tension / 4.0)) / threshold
}

/// The C++ tests' list: 50 rows 50 dp tall.
#[kompozable]
fn Rows(state: Option<LazyListState>, tag: String) {
    LazyColumn(
        modifier = Modifier.fill_max_size().test_tag(tag.clone()),
        state = state,
        content = |this: &mut LazyListScope| {
            this.items(50, |i| Text(format!("Satır {i}"), modifier = Modifier.height(50.dp())));
        },
    );
}

/// Down at `from`, moved over 20 frames to `to`, held there: what the moves started run at the current time
/// (`runCurrent`: a drag's deltas are taken by its scrollable's coroutine, the pull's snaps by the node's, both
/// dispatched as Compose's; the C++ took them at once).
fn pull_to(rule: &KompozTestRule, from: Offset, to: Offset) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / 20.0), -1, 0);
    }
    rule.main_clock().run_current();
}

/// Held 200 ms, up at `at`, settled.
fn let_go(rule: &KompozTestRule, at: Offset) {
    rule.advance_time_by(200);
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
}

/// The C++ tests' `slowDrag` (20 frames, held 200 ms, up).
fn slow_drag20(rule: &KompozTestRule, from: Offset, to: Offset) {
    pull_to(rule, from, to);
    rule.advance_time_by(200);
    rule.host().pointer_up(to, -1, 0);
    rule.advance_frame();
}

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

/// Whether an endless progress is shown to accessibility (Compose's indeterminate CircularProgressIndicator: M15 gives
/// the refreshing indicator no "Refreshing" description, the C++'s, but an indeterminate ProgressBarRangeInfo).
fn endless_progress_shown(rule: &KompozTestRule) -> bool {
    rule.all_nodes(true)
        .iter()
        .any(|n| n.config().get(SemanticsProperties::ProgressBarRangeInfo) == Some(ProgressBarRangeInfo::Indeterminate))
}

/// A 400 × 800 window of a PullToRefreshBox around the C++ tests' list, refreshing as `refreshing` says, its
/// remembered state put in `state`, `on_refresh` called on a refresh.
fn pulled_box(
    refreshing: MutableState<bool>,
    list: LazyListState,
    state: Slot<Rc<dyn PullToRefreshState>>,
    on_refresh: impl Fn() + 'static,
) -> KompozTestRule {
    let on_refresh = Rc::new(on_refresh);
    KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = remember_pull_to_refresh_state();
            *state.borrow_mut() = Some(s.clone());
            let (on_refresh, list) = (on_refresh.clone(), list.clone());
            PullToRefreshBox(
                refreshing.get(),
                move || on_refresh(),
                modifier = Modifier.fill_max_size(),
                state = s,
                content = move || Rows(Some(list.clone()), "list".to_string()),
            );
        }),
        IntSize::new(400, 800),
    )
}

fn pull_to_refresh_refreshes_past_the_threshold() {
    let refreshing = mutable_state_of(false);
    let refreshes = Rc::new(Cell::new(0));
    let list = LazyListState::new(0, 0);
    let state = slot::<Rc<dyn PullToRefreshState>>();
    let rule = {
        let (r, refreshes) = (refreshing.clone(), refreshes.clone());
        pulled_box(refreshing.clone(), list.clone(), state.clone(), move || {
            refreshes.set(refreshes.get() + 1);
            r.set(true);
        })
    };
    let fraction = || get(&state).distance_fraction();
    // 100 dp of pull (92 past the slop, the indicator half that: 46 dp): short of 80 dp, it springs back.
    slow_drag20(&rule, at(200.0, 100.0), at(200.0, 200.0));
    rule.wait_for_idle();
    assert!(refreshes.get() == 0 && fraction() == 0.0, "{}", fraction());
    // 300 dp: past it. One refresh; the indicator waits at the threshold, spinning, until it's done.
    slow_drag20(&rule, at(200.0, 100.0), at(200.0, 400.0));
    assert!(refreshes.get() == 1 && refreshing.peek());
    rule.wait_for_idle();
    assert!((fraction() - 1.0).abs() < 0.001 && endless_progress_shown(&rule), "{}", fraction());
    refreshing.set(false);
    rule.wait_for_idle();
    assert!(fraction() == 0.0 && !endless_progress_shown(&rule));
    // Scrolled down, a drag down scrolls the list back instead.
    slow_drag20(&rule, at(200.0, 600.0), at(200.0, 100.0));
    rule.wait_for_idle();
    let index = list.first_visible_item_index();
    assert!(index > 5, "{index}");
    slow_drag20(&rule, at(200.0, 200.0), at(200.0, 350.0));
    rule.wait_for_idle();
    assert!(list.first_visible_item_index() < index && fraction() == 0.0 && refreshes.get() == 1);
}

fn pull_to_refresh_goes_back_in_before_the_list_scrolls() {
    let list = LazyListState::new(0, 0);
    let state = slot::<Rc<dyn PullToRefreshState>>();
    let refreshes = Rc::new(Cell::new(0));
    let rule = {
        let refreshes = refreshes.clone();
        pulled_box(mutable_state_of(false), list.clone(), state.clone(), move || refreshes.set(refreshes.get() + 1))
    };
    let fraction = || get(&state).distance_fraction();
    // Down 200 in steps of 10: the slop starts the drag, the indicator takes half of the 192 past it: 96 dp, past the
    // threshold, so the last 16 slow down (80 + 80 × (t - t² / 4), t = 16 / 80; the C++'s 94.3 with its 10 px slop).
    pull_to(&rule, at(200.0, 100.0), at(200.0, 300.0));
    let pulled = pulled_fraction(200.0 - Slop, 80.0) * 80.0;
    assert!((fraction() * 80.0 - pulled).abs() < 0.1, "{} {pulled}", fraction() * 80.0);
    assert!((pulled - (80.0 + 80.0 * (0.2 - 0.2 * 0.2 / 4.0))).abs() < 0.01);
    // Back up 100: the indicator goes in by 50 (what was pulled, 192 - 100, halved), the list stays.
    let h = rule.host();
    for i in 1..=10 {
        rule.advance_frame();
        h.pointer_move(at(200.0, 300.0 - 10.0 * i as f32), -1, 0);
    }
    rule.advance_frame();
    assert!((fraction() * 80.0 - 46.0).abs() < 0.01, "{}", fraction() * 80.0);
    assert!(list.first_visible_item_index() == 0 && list.first_visible_item_scroll_offset() == 0);
    // Short of the threshold: no refresh, even with refreshing left false.
    let_go(&rule, at(200.0, 200.0));
    assert!(refreshes.get() == 0 && fraction() == 0.0);
}

fn pull_to_refresh_modifier_slows_past_the_threshold_and_can_be_turned_off() {
    let refreshing = mutable_state_of(false);
    let enabled = mutable_state_of(true);
    let refreshes = Rc::new(Cell::new(0));
    let list = LazyListState::new(0, 0);
    let state = PullToRefreshState();
    let scope = slot::<CoroutineScope>();
    let rule = {
        let (refreshing, enabled, refreshes, list, state, scope) =
            (refreshing.clone(), enabled.clone(), refreshes.clone(), list.clone(), state.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let (refreshing_now, refreshes, refreshing) = (refreshing.get(), refreshes.clone(), refreshing.clone());
                let indicator_state = state.clone();
                Box(
                    modifier = Modifier.fill_max_size().pull_to_refresh(
                        refreshing_now,
                        state.clone(),
                        enabled = enabled.get(),
                        threshold = 100.dp(),
                        on_refresh = move || {
                            refreshes.set(refreshes.get() + 1);
                            refreshing.set(true);
                        },
                    ),
                    content = || {
                        Rows(Some(list.clone()), "list".to_string());
                        PullToRefreshDefaults.Indicator(
                            indicator_state.clone(),
                            refreshing_now,
                            modifier = Modifier.align(Alignment::TopCenter),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    // 200 down (192 past the slop): 96 of a 100 dp threshold. Let go short of it: no refresh.
    pull_to(&rule, at(200.0, 100.0), at(200.0, 300.0));
    assert!(
        (state.distance_fraction() - pulled_fraction(200.0 - Slop, 100.0)).abs() < 0.001,
        "{}",
        state.distance_fraction()
    );
    assert!((state.distance_fraction() - 0.96).abs() < 0.001);
    let_go(&rule, at(200.0, 300.0));
    assert!(refreshes.get() == 0 && state.distance_fraction() == 0.0);
    // 400 down (392 past the slop): 196, slowed to 100 + 100 × (0.96 - 0.96² / 4).
    pull_to(&rule, at(200.0, 100.0), at(200.0, 500.0));
    assert!(
        (state.distance_fraction() - (1.0 + 0.96 - 0.96 * 0.96 / 4.0)).abs() < 0.001,
        "{}",
        state.distance_fraction()
    );
    // Much further, it stops at twice the threshold.
    rule.advance_frame();
    rule.host().pointer_move(at(200.0, 790.0), -1, 0);
    rule.advance_frame();
    assert!((state.distance_fraction() - 2.0).abs() < 0.001);
    let_go(&rule, at(200.0, 790.0));
    assert!(refreshes.get() == 1 && (state.distance_fraction() - 1.0).abs() < 0.001);
    // Refreshing: a pull doesn't move the state (nor does letting go), scrolls go by to the list.
    slow_drag20(&rule, at(200.0, 100.0), at(200.0, 300.0));
    rule.wait_for_idle();
    assert!((state.distance_fraction() - 1.0).abs() < 0.001 && refreshes.get() == 1);
    slow_drag20(&rule, at(200.0, 600.0), at(200.0, 100.0));
    rule.wait_for_idle();
    assert!(list.first_visible_item_index() > 5 && (state.distance_fraction() - 1.0).abs() < 0.001);
    let l = list.clone();
    launch(&scope, async move { named_args!(l.scroll_to_item(0)).await });
    refreshing.set(false);
    rule.wait_for_idle();
    assert_eq!(state.distance_fraction(), 0.0);
    // Turned off: a pull does nothing.
    enabled.set(false);
    rule.wait_for_idle();
    pull_to(&rule, at(200.0, 100.0), at(200.0, 500.0));
    assert_eq!(state.distance_fraction(), 0.0);
    let_go(&rule, at(200.0, 500.0));
    assert!(refreshes.get() == 1 && state.distance_fraction() == 0.0);
}

/// A 400 × 800 window of the C++ tests' list in a Box pulling `state` (not refreshing, the callback counting).
fn pulled_list(state: Rc<dyn PullToRefreshState>, list: LazyListState) -> (KompozTestRule, Slot<CoroutineScope>) {
    let scope = slot::<CoroutineScope>();
    let rule = {
        let scope = scope.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                Box(
                    modifier = Modifier.fill_max_size().pull_to_refresh(false, state.clone(), on_refresh = || {}),
                    content = || Rows(Some(list.clone()), "list".to_string()),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    (rule, scope)
}

fn pull_to_refresh_ignores_pulls_while_its_state_animates() {
    let state = PullToRefreshState();
    let (rule, _) = pulled_list(state.clone(), LazyListState::new(0, 0));
    slow_drag20(&rule, at(200.0, 100.0), at(200.0, 200.0)); // 46 dp out, then going back in
    let mut before = state.distance_fraction();
    assert!(before > 0.3 && state.is_animating(), "{before}");
    let h = rule.host();
    h.pointer_down(at(200.0, 100.0), -1, 0, PointerType::Touch);
    for i in 1..=4 {
        // pulled again while it goes in: it keeps going in
        rule.advance_frame();
        h.pointer_move(at(200.0, 100.0 + 30.0 * i as f32), -1, 0);
        assert!(state.distance_fraction() < before, "{} after {before}", state.distance_fraction());
        before = state.distance_fraction();
    }
    h.pointer_up(at(200.0, 220.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(state.distance_fraction(), 0.0);
}

fn pull_to_refresh_lets_an_animation_the_app_starts_win_over_the_finger() {
    let state = PullToRefreshState();
    let list = LazyListState::new(0, 0);
    let (rule, scope) = pulled_list(state.clone(), list.clone());
    let h = rule.host();
    h.pointer_down(at(200.0, 100.0), -1, 0, PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(at(200.0, 100.0 + 10.0 * i as f32), -1, 0);
    }
    let s = state.clone();
    launch(&scope, async move { s.animate_to_hidden().await });
    for i in 1..=5 {
        // back up 50 while it goes: the list scrolls, the pull stays out of it
        rule.advance_frame();
        h.pointer_move(at(200.0, 300.0 - 10.0 * i as f32), -1, 0);
    }
    rule.advance_frame();
    let scrolled = list.first_visible_item_index() as f32 * 50.0 + list.first_visible_item_scroll_offset() as f32;
    assert!(state.is_animating() && scrolled > 40.0, "{scrolled}");
    h.pointer_up(at(200.0, 250.0), -1, 0);
    rule.wait_for_idle();
}

/// How far the node tagged `tag` is stretched now (C++'s `stretchOf`, both axes): the stretched layer drawn over it.
fn stretch_of(rule: &KompozTestRule, tag: &str) -> f32 {
    let center = bounds(rule, tag).center();
    let canvas = rule.draw();
    canvas
        .layer_stretches
        .iter()
        .zip(&canvas.layer_bounds)
        .filter_map(|(s, b)| s.filter(|s| !s.is_empty() && b.contains(center)).map(|s| s.stretch_direction()))
        .map(|d| d.x.abs() + d.y.abs())
        .sum()
}

fn pull_to_refresh_keeps_a_fling_down_from_the_list() {
    let state = PullToRefreshState();
    let (rule, _) = pulled_list(state.clone(), LazyListState::new(0, 0));
    let h = rule.host();
    h.pointer_down(at(200.0, 100.0), -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        h.pointer_move(at(200.0, 100.0 + 20.0 * i as f32), -1, 0);
    }
    h.pointer_up(at(200.0, 300.0), -1, 0); // still moving down: the pull takes the fling, the list doesn't bounce at its top
    let mut most = 0.0f32;
    for _ in 0..30 {
        rule.advance_frame();
        most = most.max(stretch_of(&rule, "list"));
    }
    assert_eq!(most, 0.0);
    rule.wait_for_idle();
    assert_eq!(state.distance_fraction(), 0.0);
}

fn pull_to_refresh_starts_where_is_refreshing_says() {
    let shown = mutable_state_of(false);
    let state = PullToRefreshState();
    let rule = {
        let (shown, state) = (shown.clone(), state.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                if shown.get() {
                    Box(modifier = Modifier.fill_max_size().pull_to_refresh(true, state.clone(), on_refresh = || {}));
                }
            }),
            IntSize::new(400, 800),
        )
    };
    shown.set(true);
    rule.advance_frame();
    assert!(state.distance_fraction() == 1.0 && !state.is_animating()); // put there, not animated
}

fn pull_to_refresh_states_animate_snap_and_save_their_fraction() {
    // remembered and saved: shown again under the same key, the fraction it had.
    let shown = mutable_state_of(true);
    let (state, scope) = (slot::<Rc<dyn PullToRefreshState>>(), slot::<CoroutineScope>());
    let rule = {
        let (shown, state, scope) = (shown.clone(), state.clone(), scope.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            *scope.borrow_mut() = Some(remember_coroutine_scope());
            let holder = remember_saveable_state_holder();
            if shown.get() {
                let state = state.clone();
                holder.SaveableStateProvider("çekme", move || {
                    *state.borrow_mut() = Some(remember_pull_to_refresh_state());
                });
            }
        }))
    };
    let s = get(&state);
    assert!(s.distance_fraction() == 0.0 && !s.is_animating());
    let t = s.clone();
    launch(&scope, async move { t.animate_to_threshold().await });
    rule.advance_frame();
    rule.advance_frame();
    assert!(s.is_animating() && s.distance_fraction() > 0.0 && s.distance_fraction() < 1.0);
    rule.wait_for_idle();
    assert!(!s.is_animating() && s.distance_fraction() == 1.0);
    let t = s.clone();
    launch(&scope, async move { t.snap_to(0.6).await });
    rule.advance_frame();
    assert!(!s.is_animating() && s.distance_fraction() == 0.6);
    shown.set(false);
    rule.wait_for_idle();
    shown.set(true);
    rule.wait_for_idle();
    let restored = get(&state);
    assert_eq!(restored.distance_fraction(), 0.6);
    let t = restored.clone();
    launch(&scope, async move { t.animate_to_hidden().await });
    rule.wait_for_idle();
    assert_eq!(restored.distance_fraction(), 0.0);
}

fn pull_to_refresh_takes_a_pull_only_from_the_start_and_by_the_user() {
    let state = PullToRefreshState();
    let list = LazyListState::new(0, 0);
    let (rule, scope) = pulled_list(state.clone(), list.clone());
    // Scrolled down, a drag down scrolls the list back first, then pulls.
    let l = list.clone();
    launch(&scope, async move { named_args!(l.scroll_to_item(2)).await });
    rule.wait_for_idle();
    pull_to(&rule, at(200.0, 100.0), at(200.0, 300.0));
    assert!(list.first_visible_item_index() == 0 && list.first_visible_item_scroll_offset() == 0);
    // (100 of the 192 past the slop scrolled the list back; the rest pulled)
    assert!((state.distance_fraction() - pulled_fraction(92.0, 80.0)).abs() < 0.01, "{}", state.distance_fraction());
    // Pushed back up: the pull gives it back first, the list stays.
    for i in 1..=4 {
        rule.advance_frame();
        rule.host().pointer_move(at(200.0, 300.0 - 10.0 * i as f32), -1, 0);
    }
    rule.advance_frame();
    assert!((state.distance_fraction() - pulled_fraction(52.0, 80.0)).abs() < 0.01, "{}", state.distance_fraction());
    assert!(list.first_visible_item_index() == 0 && list.first_visible_item_scroll_offset() == 0);
    let_go(&rule, at(200.0, 260.0));
    assert_eq!(state.distance_fraction(), 0.0);
    // A mouse wheel up at the top doesn't pull: Compose's wheel scrolling dispatches only what the list itself can
    // take (MouseWheelScrollingLogic's canConsumeDelta).
    rule.host().scroll(at(200.0, 200.0), at(0.0, -1.0));
    for _ in 0..10 {
        rule.advance_frame();
        assert_eq!(state.distance_fraction(), 0.0);
    }
    rule.wait_for_idle();
}

/// Whether a layer was drawn moved down by `y` (its transform's translation).
fn layer_moved_by(canvas: &kompoz_test::RecordingCanvas, y: f32) -> bool {
    canvas.transforms.iter().any(|m| (m.values[13] - y).abs() < 0.01)
}

fn pull_to_refresh_indicator_boxes_come_out_from_under_the_top_edge() {
    // IndicatorBox (what PullToRefreshDefaults.Indicator wraps its arrow and spinner in) with content of its own.
    let refreshing = mutable_state_of(false);
    let state = PullToRefreshState();
    let scope = slot::<CoroutineScope>();
    let rule = {
        let (refreshing, state, scope) = (refreshing.clone(), state.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let (state, refreshing) = (state.clone(), refreshing.get());
                Box(
                    modifier = Modifier.fill_max_size().padding_sides_with(0.dp(), 50.dp(), 0.dp(), 0.dp()),
                    content = move || {
                        PullToRefreshDefaults.IndicatorBox(
                            state.clone(),
                            refreshing,
                            modifier = Modifier.align(Alignment::TopCenter),
                            container_color = PullToRefreshDefaults::indicator_container_color(),
                            content =
                                || Box(modifier = Modifier.size(16.dp()).background(Color::Red).test_tag("içerik")),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let snap = |to: f32| {
        let s = state.clone();
        launch(&scope, async move { s.snap_to(to).await });
        rule.wait_for_idle();
    };
    let container = light_color_scheme().surface_container_high;
    // Hidden: 40 dp up, under the edge it rests at (y 50), no shadow, the container drawn in its color.
    let hidden = rule.draw();
    assert!(hidden.rect_colors.contains(&container) && hidden.elevation_shadows.is_empty());
    assert!(layer_moved_by(&hidden, -40.0), "{:?}", hidden.transforms);
    assert!(
        hidden.clips.iter().any(|c| near(c.top, 50.0) && c.left < -1000.0 && c.right > 1000.0),
        "{:?}",
        hidden.clips
    );
    // A quarter of the way: 20 of 80 out (the box 40 tall moves up by 20); a shadow of Elevation (3 dp).
    snap(0.25);
    let quarter = rule.draw();
    assert!(layer_moved_by(&quarter, 20.0 - 40.0), "{:?}", quarter.transforms);
    assert!(!quarter.elevation_shadows.is_empty());
    let content = bounds(&rule, "içerik");
    assert!(near(content.center().x, 200.0) && near(content.center().y, 50.0 - 20.0 + 20.0), "{content:?}");
    // Past the threshold: on down with the fraction.
    snap(1.5);
    assert!(layer_moved_by(&rule.draw(), 120.0 - 40.0));
    // Refreshing at 0: still a shadow (refreshing shows it).
    snap(0.0);
    refreshing.set(true);
    rule.wait_for_idle();
    assert!(!rule.draw().elevation_shadows.is_empty());
}

fn pull_to_refresh_box_can_be_turned_off() {
    let state = PullToRefreshState();
    let refreshes = Rc::new(Cell::new(0));
    let rule = {
        let (state, refreshes) = (state.clone(), refreshes.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let refreshes = refreshes.clone();
                PullToRefreshBox(
                    false,
                    move || refreshes.set(refreshes.get() + 1),
                    modifier = Modifier.fill_max_size(),
                    state = state.clone(),
                    enabled = false,
                    content = || Rows(None, "list".to_string()),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    slow_drag20(&rule, at(200.0, 100.0), at(200.0, 300.0));
    assert_eq!(state.distance_fraction(), 0.0);
    slow_drag20(&rule, at(200.0, 100.0), at(200.0, 500.0));
    rule.wait_for_idle();
    assert!(refreshes.get() == 0 && state.distance_fraction() == 0.0);
}

fn same_color(a: Color, b: Color) -> bool {
    (a.red() - b.red()).abs() < 0.003
        && (a.green() - b.green()).abs() < 0.003
        && (a.blue() - b.blue()).abs() < 0.003
        && (a.alpha() - b.alpha()).abs() < 0.003
}

/// The arcs drawn with a sweep and a color (Compose draws the arrow's arc and arrowhead at any progress: of nothing
/// at first; the endless spinner's track is transparent).
fn visible_arcs(canvas: &kompoz_test::RecordingCanvas) -> Vec<kompoz_test::RecordedArc> {
    canvas.arcs.iter().filter(|a| a.sweep != 0.0 && a.color.alpha() > 0.0).cloned().collect()
}

/// The paths drawn with an area (the arrowhead, scaled by the progress past 40 %).
fn visible_paths(canvas: &kompoz_test::RecordingCanvas) -> usize {
    canvas.paths.iter().filter(|p| p.width() > 0.0 || p.height() > 0.0).count()
}

/// The determinate progress told to accessibility, if any.
fn progress_info(rule: &KompozTestRule) -> Option<ProgressBarRangeInfo> {
    rule.all_nodes(true)
        .iter()
        .filter_map(|n| n.config().get(SemanticsProperties::ProgressBarRangeInfo))
        .find(|p| *p != ProgressBarRangeInfo::Indeterminate)
}

fn pull_to_refresh_indicator_comes_out_from_under_the_top_edge() {
    let refreshing = mutable_state_of(false);
    let state = PullToRefreshState();
    let scope = slot::<CoroutineScope>();
    let rule = {
        let (refreshing, state, scope) = (refreshing.clone(), state.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let (state, refreshing) = (state.clone(), refreshing.get());
                Box(
                    modifier = Modifier.fill_max_size().padding_sides_with(0.dp(), 50.dp(), 0.dp(), 0.dp()),
                    content = move || {
                        PullToRefreshDefaults.Indicator(
                            state.clone(),
                            refreshing,
                            modifier = Modifier.align(Alignment::TopCenter),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let snap = |to: f32| {
        let s = state.clone();
        launch(&scope, async move { s.snap_to(to).await });
        rule.wait_for_idle();
    };
    let cs = light_color_scheme();
    // Hidden: no shadow (only the circle's own color), no arc, no progress to accessibility.
    let hidden = rule.draw();
    assert!(visible_arcs(&hidden).is_empty() && visible_paths(&hidden) == 0 && !hidden.rect_colors.is_empty());
    assert!(hidden.rect_colors.iter().all(|c| same_color(*c, cs.surface_container_high)), "{:?}", hidden.rect_colors);
    assert!(hidden.elevation_shadows.is_empty() && progress_info(&rule).is_none());
    // A quarter of the way: 20 dp of 80 out (the circle 40 dp tall moves up by 20), clipped at the edge it rests
    // under, its arc not yet started (the first 40 % shows nothing). (The C++ described it as "Pull to refresh";
    // M15's arrow tells its progress instead.)
    snap(0.25);
    let quarter = rule.draw();
    assert!(layer_moved_by(&quarter, 20.0 - 40.0), "{:?}", quarter.transforms);
    assert!(quarter.clips.iter().any(|c| near(c.top, 50.0) && c.left < -1000.0 && c.right > 1000.0));
    assert!(visible_arcs(&quarter).is_empty());
    assert_eq!(progress_info(&rule), Some(ProgressBarRangeInfo::new(0.25, 0.0..=1.0, 0)));
    // Halfway past that, 70 % of the way: an arc of (0.7 - 0.4) × 5/3 × 0.8 of a turn, faint, with its arrowhead, in
    // onSurfaceVariant.
    snap(0.7);
    let pulled = rule.draw();
    let arcs = visible_arcs(&pulled);
    assert!(arcs.len() == 1 && visible_paths(&pulled) == 1, "{arcs:?} {:?}", pulled.paths);
    let a = &arcs[0];
    assert!((a.sweep - 0.4 * 360.0).abs() < 0.1 && (a.stroke_width - 2.5).abs() < 0.01, "{a:?}");
    assert!((a.oval.width() - 13.5).abs() < 0.01, "{a:?}");
    assert!(same_color(a.color, named_args!(cs.on_surface_variant.copy(alpha = 0.3))), "{:?}", a.color);
    // Its start turns with the pull: (-0.25 + 0.4 × 0.5) / 2 of a turn.
    assert!((a.start - (-0.025 * 360.0)).abs() < 0.01, "{}", a.start);
    // Past the threshold it shows in full: the whole arc (0.8 of a turn), turned on by the overshoot: (-0.25 + 0.4
    // + (0.2 - 0.2² / 4)) / 2. Out, with a shadow.
    snap(1.2);
    let past_canvas = rule.draw();
    let past = visible_arcs(&past_canvas);
    assert!(past.len() == 1 && same_color(past[0].color, cs.on_surface_variant), "{past:?}");
    assert!((past[0].sweep - 288.0).abs() < 0.1 && (past[0].start - 0.17 * 360.0).abs() < 0.1, "{past:?}");
    assert!(!past_canvas.elevation_shadows.is_empty());
    // Refreshing: a spinner in its place (crossfaded), at the threshold: one arc of the same stroke, no arrowhead, an
    // endless progress to accessibility (M15: no "Refreshing" description).
    refreshing.set(true);
    snap(1.0);
    rule.advance_time_by(1000);
    let spinning = rule.draw();
    let arcs = visible_arcs(&spinning);
    assert!(arcs.len() == 1 && visible_paths(&spinning) == 0, "{arcs:?} {:?}", spinning.paths);
    assert!((arcs[0].stroke_width - 2.5).abs() < 0.01 && same_color(arcs[0].color, cs.on_surface_variant));
    // (the spinner 16 dp: its arc's circle 2.5 dp strokes in, 13.5 across)
    assert!((arcs[0].oval.width() - 13.5).abs() < 0.01, "{:?}", arcs[0]);
    assert!(endless_progress_shown(&rule) && progress_info(&rule).is_none());
    // Done: the arrow again.
    refreshing.set(false);
    rule.wait_for_idle();
    assert!(!endless_progress_shown(&rule) && progress_info(&rule).is_some());
}

/// The angles (degrees) the drawn transforms turn by.
fn turns(canvas: &kompoz_test::RecordingCanvas) -> Vec<f32> {
    canvas.transforms.iter().map(|m| m.values[1].atan2(m.values[0]).to_degrees()).collect()
}

fn pull_to_refresh_loading_indicator_morphs_with_the_pull_and_turns_past_it() {
    // PullToRefreshDefaults.LoadingIndicator (Compose's): a 48 dp box of the contained loading indicator's colors,
    // without a shadow; pulled, the indicator morphing with the fraction; past the threshold the whole of it turning
    // by (fraction - 1) × 180°; refreshing, the endless one.
    let refreshing = mutable_state_of(false);
    let state = PullToRefreshState();
    let scope = slot::<CoroutineScope>();
    let rule = {
        let (refreshing, state, scope) = (refreshing.clone(), state.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let (state, refreshing) = (state.clone(), refreshing.get());
                Box(
                    modifier = Modifier.fill_max_size().padding_sides_with(0.dp(), 50.dp(), 0.dp(), 0.dp()),
                    content = move || {
                        PullToRefreshDefaults.LoadingIndicator(
                            state.clone(),
                            refreshing,
                            modifier = Modifier.align(Alignment::TopCenter).test_tag("yükleme"),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let snap = |to: f32| {
        let s = state.clone();
        launch(&scope, async move { s.snap_to(to).await });
        rule.wait_for_idle();
    };
    let cs = light_color_scheme();
    // Hidden: 48 dp, all of it up under the edge.
    let b = bounds(&rule, "yükleme");
    assert!(near(b.width(), 48.0) && near(b.height(), 48.0), "{b:?}");
    let hidden = rule.draw();
    assert!(layer_moved_by(&hidden, -48.0), "{:?}", hidden.transforms);
    // Half way: 40 of 80 out, the determinate indicator at 0.5, in its colors; no shadow (LoadingIndicatorElevation).
    snap(0.5);
    let half = rule.draw();
    assert!(layer_moved_by(&half, 40.0 - 48.0), "{:?}", half.transforms);
    assert_eq!(progress_info(&rule), Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0)));
    let drawn = |c: Color| half.rect_colors.contains(&c) || half.path_colors.contains(&c);
    assert!(drawn(cs.primary_container) && drawn(cs.on_primary_container));
    assert!(half.elevation_shadows.is_empty());
    // (the turns drawn: the layer's none, then the indicator's own by its coerced progress, -progress × 180°, and past
    // the threshold the whole one's by (fraction - 1) × 180°, between them)
    let turned = |to: f32| {
        snap(to);
        turns(&rule.draw()).iter().map(|t| t.round() as i32).collect::<Vec<_>>()
    };
    assert_eq!(turns(&half).iter().map(|t| t.round() as i32).collect::<Vec<_>>(), [0, -90]);
    assert_eq!(turned(1.0), [0, -180]);
    assert_eq!(turned(1.5), [0, -90, -180]);
    assert!(layer_moved_by(&rule.draw(), 120.0 - 48.0));
    // Refreshing: the endless one in its place.
    refreshing.set(true);
    snap(1.0);
    rule.advance_time_by(1000);
    assert!(endless_progress_shown(&rule) && progress_info(&rule).is_none());
}

fn pull_to_refresh_box_takes_an_indicator_of_its_own() {
    // PullToRefreshBox's indicator slot, its content's alignment and threshold: what Compose's PullToRefreshBox passes
    // on (a LoadingIndicator for the default Indicator, as M15's PullToRefreshWithLoadingIndicatorSample).
    let refreshing = mutable_state_of(false);
    let state = PullToRefreshState();
    let refreshes = Rc::new(Cell::new(0));
    let rule = {
        let (refreshing, state, refreshes) = (refreshing.clone(), state.clone(), refreshes.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (r, refreshes, indicator_state) = (refreshing.clone(), refreshes.clone(), state.clone());
                let refreshing_now = refreshing.get();
                PullToRefreshBox(
                    refreshing_now,
                    move || {
                        refreshes.set(refreshes.get() + 1);
                        r.set(true);
                    },
                    modifier = Modifier.fill_max_size(),
                    state = state.clone(),
                    content_alignment = Alignment::BottomEnd,
                    indicator = Some(Rc::new(move || {
                        PullToRefreshDefaults.LoadingIndicator(
                            indicator_state.clone(),
                            refreshing_now,
                            modifier = Modifier.align(Alignment::TopCenter).test_tag("yükleme"),
                        )
                    })),
                    threshold = 50.dp(),
                    // (a 200 dp column that scrolls: what a pull takes the scrolls of)
                    content = || {
                        Column(
                            modifier =
                                Modifier.size(200.dp()).vertical_scroll(&remember_scroll_state()).test_tag("içerik"),
                            content = || Box(modifier = Modifier.size(100.dp())),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    // The content at the bottom end, the indicator at the top center, the default spinner's arrow nowhere.
    let content = bounds(&rule, "içerik");
    assert!(near(content.right, 400.0) && near(content.bottom, 800.0), "{content:?}");
    assert!(near(bounds(&rule, "yükleme").center().x, 200.0));
    // 120 down on it (112 past the slop: 56, past 50 dp): a refresh; the loading indicator waits at the threshold.
    slow_drag20(&rule, at(300.0, 610.0), at(300.0, 730.0));
    rule.wait_for_idle();
    assert!(refreshes.get() == 1 && (state.distance_fraction() - 1.0).abs() < 0.001, "{}", state.distance_fraction());
    assert!(endless_progress_shown(&rule));
    // (out by its max distance, the default 80 dp whatever the threshold)
    assert!(layer_moved_by(&rule.draw(), 80.0 - 48.0));
}

fn pull_to_refresh_defaults_follow_the_tokens() {
    assert_eq!(PullToRefreshDefaults::PositionalThreshold, 80.dp());
    assert_eq!(PullToRefreshDefaults::IndicatorMaxDistance, 80.dp());
    assert_eq!(PullToRefreshDefaults::Elevation, 3.dp()); // (ElevationTokens.Level2)
    assert_eq!(PullToRefreshDefaults::LoadingIndicatorElevation, 0.dp()); // (Level0)
    assert!(PullToRefreshDefaults::indicator_shape() == CircleShape);
    assert!(PullToRefreshDefaults::shape() == CircleShape);
    let colors = slot::<[Color; 5]>();
    let rule = {
        let colors = colors.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            MaterialTheme(
                color_scheme = dark_color_scheme(),
                content = || {
                    *colors.borrow_mut() = Some([
                        PullToRefreshDefaults::indicator_container_color(),
                        PullToRefreshDefaults::container_color(),
                        PullToRefreshDefaults::indicator_color(),
                        PullToRefreshDefaults::loading_indicator_container_color(),
                        PullToRefreshDefaults::loading_indicator_color(),
                    ]);
                },
            );
        }))
    };
    rule.wait_for_idle();
    let dark = dark_color_scheme();
    assert_eq!(
        get(&colors),
        [
            dark.surface_container_high,
            dark.surface_container_high,
            dark.on_surface_variant,
            // (the contained loading indicator's: LoadingIndicatorTokens' ContainedContainerColor and
            // ContainedActiveColor)
            dark.primary_container,
            dark.on_primary_container,
        ]
    );
}

// ---- The M15 samples (PullToRefreshSamples.kt; LoadingIndicatorSamples.kt's LoadingIndicatorPullToRefreshSample) ----
// Each a Scaffold with a TopAppBar whose action refreshes too (the accessible alternative the samples give).

/// The samples' `onRefresh`: refreshing, `item_count` up by 5 after `ms` (the samples' fetch).
fn sample_on_refresh(
    item_count: &MutableState<i32>,
    is_refreshing: &MutableState<bool>,
    coroutine_scope: &CoroutineScope,
    ms: i64,
) -> Rc<dyn Fn()> {
    let (item_count, is_refreshing, coroutine_scope) =
        (item_count.clone(), is_refreshing.clone(), coroutine_scope.clone());
    Rc::new(move || {
        is_refreshing.set(true);
        let (item_count, is_refreshing) = (item_count.clone(), is_refreshing.clone());
        named_args!(coroutine_scope.launch(async move {
            kompoz_coroutines::delay(ms).await;
            item_count.set(item_count.get() + 5);
            is_refreshing.set(false);
        }));
    })
}

/// The samples' top bar: `title`, its action the refresh.
#[kompozable]
fn RefreshTopAppBar(title: &'static str, on_refresh: Rc<dyn Fn()>) {
    TopAppBar(
        title = || Text(title),
        // Provide an accessible alternative to trigger refresh.
        actions = || {
            let on_refresh = on_refresh.clone();
            IconButton(
                on_click = move || on_refresh(),
                content = || VectorIcon(Icons::Filled::Refresh(), Some("Trigger Refresh".to_string())),
            );
        },
    );
}

/// The samples' list: `item_count` list items, the newest first.
#[kompozable]
fn RefreshItems(item_count: MutableState<i32>) {
    LazyColumn(
        modifier = Modifier.fill_max_size(),
        content = move |scope: &mut LazyListScope| {
            let count = item_count.get();
            scope.items(count, move |it| {
                ListItem(headline_content = move || Text(text = format!("Item {}", count - it)))
            });
        },
    );
}

/// `PullToRefreshSample`.
#[kompozable]
fn PullToRefreshSample() {
    let item_count = remember(|| mutable_int_state_of(15));
    let is_refreshing = remember(|| mutable_state_of(false));
    let state = remember_pull_to_refresh_state();
    let coroutine_scope = remember_coroutine_scope();
    let on_refresh = sample_on_refresh(&item_count, &is_refreshing, &coroutine_scope, 5000);

    let bar_refresh = on_refresh.clone();
    Scaffold(
        top_bar = move || RefreshTopAppBar("Title", bar_refresh.clone()),
        content = move |it: PaddingValues| {
            let (on_refresh, item_count) = (on_refresh.clone(), item_count.clone());
            PullToRefreshBox(
                modifier = Modifier.padding_values(it),
                state = state.clone(),
                is_refreshing = is_refreshing.get(),
                on_refresh = move || on_refresh(),
                content = move || RefreshItems(item_count.clone()),
            );
        },
    );
}

/// `PullToRefreshWithLoadingIndicatorSample`.
#[kompozable]
fn PullToRefreshWithLoadingIndicatorSample() {
    let item_count = remember(|| mutable_int_state_of(15));
    let is_refreshing = remember(|| mutable_state_of(false));
    let state = remember_pull_to_refresh_state();
    let coroutine_scope = remember_coroutine_scope();
    let on_refresh = sample_on_refresh(&item_count, &is_refreshing, &coroutine_scope, 5000);

    let bar_refresh = on_refresh.clone();
    Scaffold(
        top_bar = move || RefreshTopAppBar("Title", bar_refresh.clone()),
        content = move |it: PaddingValues| {
            let (on_refresh, item_count, indicator_state) = (on_refresh.clone(), item_count.clone(), state.clone());
            let refreshing = is_refreshing.get();
            PullToRefreshBox(
                modifier = Modifier.padding_values(it),
                state = state.clone(),
                is_refreshing = refreshing,
                on_refresh = move || on_refresh(),
                indicator = Some(Rc::new(move || {
                    PullToRefreshDefaults.LoadingIndicator(
                        state = indicator_state.clone(),
                        is_refreshing = refreshing,
                        modifier = Modifier.align(Alignment::TopCenter),
                    );
                })),
                content = move || RefreshItems(item_count.clone()),
            );
        },
    );
}

/// `PullToRefreshViewModelSample`'s view model (Kotlin's `object : ViewModel()`, its `viewModelScope` a scope of its
/// own on the main dispatcher — the host's —, never cleared, as the sample remembers it).
struct RefreshViewModel {
    refresh_requests: kompoz_coroutines::Channel<()>,
    is_refreshing: MutableState<bool>,
    item_count: MutableState<i32>,
}

impl RefreshViewModel {
    fn new(view_model_scope: CoroutineScope) -> Rc<RefreshViewModel> {
        let view_model = Rc::new(RefreshViewModel {
            refresh_requests: kompoz_coroutines::Channel::new(1, kompoz_coroutines::BufferOverflow::Suspend),
            is_refreshing: mutable_state_of(false),
            item_count: mutable_int_state_of(15),
        });
        let (requests, is_refreshing, item_count) =
            (view_model.refresh_requests.clone(), view_model.is_refreshing.clone(), view_model.item_count.clone());
        named_args!(view_model_scope.launch(async move {
            while let Some(_r) = requests.receive().await {
                is_refreshing.set(true);
                // (Kotlin's try / finally: a guard sets it back however the block ends, cancelled too)
                let _finally = Finally(Some({
                    let is_refreshing = is_refreshing.clone();
                    move || is_refreshing.set(false)
                }));
                item_count.set(item_count.get() + 5);
                kompoz_coroutines::delay(5000).await; // simulate doing real work
            }
        }));
        view_model
    }

    fn is_refreshing(&self) -> bool {
        self.is_refreshing.get()
    }

    fn item_count(&self) -> i32 {
        self.item_count.get()
    }

    fn refresh(&self) {
        self.refresh_requests.try_send(());
    }
}

/// Runs its block when dropped (Kotlin's `finally`).
struct Finally<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> Drop for Finally<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

/// `PullToRefreshViewModelSample`.
#[kompozable]
fn PullToRefreshViewModelSample() {
    let dispatcher = kompoz_coroutines::LocalDispatcher.current().expect("the host's dispatcher");
    let view_model = remember(move || RefreshViewModel::new(CoroutineScope::new(&dispatcher)));

    let bar_view_model = view_model.clone();
    Scaffold(
        top_bar = move || {
            let view_model = bar_view_model.clone();
            TopAppBar(
                title = || Text("Title"),
                // Provide an accessible alternative to trigger refresh.
                actions = move || {
                    let view_model = view_model.clone();
                    IconButton(
                        enabled = !view_model.is_refreshing(),
                        on_click = move || view_model.refresh(),
                        content = || VectorIcon(Icons::Filled::Refresh(), Some("Trigger Refresh".to_string())),
                    );
                },
            );
        },
        content = move |it: PaddingValues| {
            let (refresh_view_model, list_view_model) = (view_model.clone(), view_model.clone());
            PullToRefreshBox(
                modifier = Modifier.padding_values(it),
                is_refreshing = view_model.is_refreshing(),
                on_refresh = move || refresh_view_model.refresh(),
                content = move || {
                    let view_model = list_view_model.clone();
                    LazyColumn(
                        modifier = Modifier.fill_max_size(),
                        content = move |scope: &mut LazyListScope| {
                            if !view_model.is_refreshing() {
                                let view_model = view_model.clone();
                                scope.items(view_model.item_count(), move |it| {
                                    let count = view_model.item_count();
                                    ListItem(headline_content = move || Text(text = format!("Item {}", count - it)));
                                });
                            }
                        },
                    );
                },
            );
        },
    );
}

/// `PullToRefreshScalingSample`.
#[kompozable]
fn PullToRefreshScalingSample() {
    let item_count = remember(|| mutable_int_state_of(15));
    let is_refreshing = remember(|| mutable_state_of(false));
    let state = remember_pull_to_refresh_state();
    let coroutine_scope = remember_coroutine_scope();
    // (fetch something)
    let on_refresh = sample_on_refresh(&item_count, &is_refreshing, &coroutine_scope, 5000);

    let scale_fraction = {
        let (is_refreshing, state) = (is_refreshing.clone(), state.clone());
        move || {
            if is_refreshing.get() {
                1.0
            } else {
                LinearOutSlowInEasing.transform(state.distance_fraction()).clamp(0.0, 1.0)
            }
        }
    };

    let (box_refresh, bar_refresh) = (on_refresh.clone(), on_refresh.clone());
    Scaffold(
        modifier = Modifier.pull_to_refresh(
            state = state.clone(),
            is_refreshing = is_refreshing.get(),
            on_refresh = move || box_refresh(),
        ),
        top_bar = move || RefreshTopAppBar("TopAppBar", bar_refresh.clone()),
        content = move |it: PaddingValues| {
            let (item_count, is_refreshing, state, scale_fraction) =
                (item_count.clone(), is_refreshing.clone(), state.clone(), scale_fraction.clone());
            Box(
                modifier = Modifier.padding_values(it),
                content = move || {
                    let item_count = item_count.clone();
                    let refreshing = is_refreshing.get();
                    LazyColumn(
                        modifier = Modifier.fill_max_size(),
                        content = move |scope: &mut LazyListScope| {
                            if !refreshing {
                                let count = item_count.get();
                                scope.items(count, move |it| {
                                    ListItem(headline_content = move || Text(text = format!("Item {}", count - it)));
                                });
                            }
                        },
                    );
                    let (scale_fraction, state) = (scale_fraction.clone(), state.clone());
                    Box(
                        modifier = Modifier.align(Alignment::TopCenter).graphics_layer_block(move |layer| {
                            layer.scale_x = scale_fraction();
                            layer.scale_y = scale_fraction();
                        }),
                        content =
                            move || PullToRefreshDefaults.Indicator(state = state.clone(), is_refreshing = refreshing),
                    );
                },
            );
        },
    );
}

/// `PullToRefreshLinearProgressIndicatorSample`.
#[kompozable]
fn PullToRefreshLinearProgressIndicatorSample() {
    let item_count = remember(|| mutable_int_state_of(15));
    let is_refreshing = remember(|| mutable_state_of(false));
    let state = remember_pull_to_refresh_state();
    let coroutine_scope = remember_coroutine_scope();
    // (fetch something)
    let on_refresh = sample_on_refresh(&item_count, &is_refreshing, &coroutine_scope, 5000);

    let (box_refresh, bar_refresh) = (on_refresh.clone(), on_refresh.clone());
    Scaffold(
        modifier = Modifier.pull_to_refresh(
            state = state.clone(),
            is_refreshing = is_refreshing.get(),
            on_refresh = move || box_refresh(),
        ),
        top_bar = move || RefreshTopAppBar("TopAppBar", bar_refresh.clone()),
        content = move |it: PaddingValues| {
            let (item_count, is_refreshing, state) = (item_count.clone(), is_refreshing.clone(), state.clone());
            Box(
                modifier = Modifier.padding_values(it),
                content = move || {
                    let item_count = item_count.clone();
                    let refreshing = is_refreshing.get();
                    LazyColumn(
                        modifier = Modifier.fill_max_size(),
                        content = move |scope: &mut LazyListScope| {
                            if !refreshing {
                                let count = item_count.get();
                                scope.items(count, move |it| {
                                    ListItem(headline_content = move || Text(text = format!("Item {}", count - it)));
                                });
                            }
                        },
                    );
                    if refreshing {
                        IndeterminateLinearProgressIndicator(modifier = Modifier.fill_max_width());
                    } else {
                        let state = state.clone();
                        LinearProgressIndicator(
                            modifier = Modifier.fill_max_width(),
                            progress = move || state.distance_fraction(),
                        );
                    }
                },
            );
        },
    );
}

/// `PullToRefreshSampleCustomState`'s state (Kotlin's `object : PullToRefreshState`): an Animatable of the
/// fraction, to the threshold with a high bouncy spring.
struct BouncyPullToRefreshState {
    anim: Animatable<f32, AnimationVector1D>,
}

impl PullToRefreshState for BouncyPullToRefreshState {
    fn distance_fraction(&self) -> f32 {
        self.anim.value()
    }

    fn is_animating(&self) -> bool {
        self.anim.is_running()
    }

    fn animate_to_threshold(&self) -> LocalFuture<'static, ()> {
        let animation = named_args!(
            self.anim
                .animate_to(1.0, animation_spec = Arc::new(spring(damping_ratio = Spring::DampingRatioHighBouncy)),)
        );
        Box::pin(async move {
            animation.await;
        })
    }

    fn animate_to_hidden(&self) -> LocalFuture<'static, ()> {
        let animation = named_args!(self.anim.animate_to(0.0));
        Box::pin(async move {
            animation.await;
        })
    }

    fn snap_to(&self, target_value: f32) -> LocalFuture<'static, ()> {
        Box::pin(self.anim.snap_to(target_value))
    }
}

/// `PullToRefreshSampleCustomState`.
#[kompozable]
fn PullToRefreshSampleCustomState() {
    let item_count = remember(|| mutable_int_state_of(15));
    let is_refreshing = remember(|| mutable_state_of(false));
    let coroutine_scope = remember_coroutine_scope();
    // (fetch something)
    let on_refresh = sample_on_refresh(&item_count, &is_refreshing, &coroutine_scope, 5000);

    let state: Rc<dyn PullToRefreshState> = remember(|| {
        Rc::new(BouncyPullToRefreshState { anim: Animatable((0.0, f32::VectorConverter)) })
            as Rc<dyn PullToRefreshState>
    });

    let bar_refresh = on_refresh.clone();
    Scaffold(
        top_bar = move || RefreshTopAppBar("TopAppBar", bar_refresh.clone()),
        content = move |it: PaddingValues| {
            let (on_refresh, item_count) = (on_refresh.clone(), item_count.clone());
            let refreshing = is_refreshing.get();
            PullToRefreshBox(
                modifier = Modifier.padding_values(it),
                is_refreshing = refreshing,
                on_refresh = move || on_refresh(),
                state = state.clone(),
                content = move || {
                    let item_count = item_count.clone();
                    LazyColumn(
                        modifier = Modifier.fill_max_size(),
                        content = move |scope: &mut LazyListScope| {
                            if !refreshing {
                                let count = item_count.get();
                                scope.items(count, move |it| {
                                    ListItem(headline_content = move || Text(text = format!("Item {}", count - it)));
                                });
                            }
                        },
                    );
                },
            );
        },
    );
}

/// `PullToRefreshCustomIndicatorWithDefaultTransform`.
#[kompozable]
fn PullToRefreshCustomIndicatorWithDefaultTransform() {
    let item_count = remember(|| mutable_int_state_of(15));
    let is_refreshing = remember(|| mutable_state_of(false));
    let state = remember_pull_to_refresh_state();
    let coroutine_scope = remember_coroutine_scope();
    let on_refresh = sample_on_refresh(&item_count, &is_refreshing, &coroutine_scope, 1500);

    let bar_refresh = on_refresh.clone();
    Scaffold(
        top_bar = move || RefreshTopAppBar("Title", bar_refresh.clone()),
        content = move |it: PaddingValues| {
            let (on_refresh, item_count, indicator_state) = (on_refresh.clone(), item_count.clone(), state.clone());
            let refreshing = is_refreshing.get();
            PullToRefreshBox(
                modifier = Modifier.padding_values(it),
                state = state.clone(),
                is_refreshing = refreshing,
                on_refresh = move || on_refresh(),
                indicator = Some(Rc::new(move || {
                    let progress_state = indicator_state.clone();
                    PullToRefreshDefaults.IndicatorBox(
                        state = indicator_state.clone(),
                        is_refreshing = refreshing,
                        modifier = Modifier.align(Alignment::TopCenter),
                        elevation = 0.dp(),
                        content = move || {
                            if refreshing {
                                IndeterminateCircularProgressIndicator();
                            } else {
                                let state = progress_state.clone();
                                CircularProgressIndicator(
                                    progress = move || state.distance_fraction(),
                                    track_color = ProgressIndicatorDefaults::circular_indeterminate_track_color(),
                                );
                            }
                        },
                    );
                })),
                content = move || RefreshItems(item_count.clone()),
            );
        },
    );
}

/// `LoadingIndicatorPullToRefreshSample` (LoadingIndicatorSamples.kt).
#[kompozable]
fn LoadingIndicatorPullToRefreshSample() {
    let item_count = remember(|| mutable_state_of(15));
    let is_refreshing = remember(|| mutable_state_of(false));
    let state = remember_pull_to_refresh_state();
    let coroutine_scope = remember_coroutine_scope();
    // (fetch something)
    let on_refresh = sample_on_refresh(&item_count, &is_refreshing, &coroutine_scope, 5000);

    let scale_fraction = {
        let (is_refreshing, state) = (is_refreshing.clone(), state.clone());
        move || {
            if is_refreshing.get() {
                1.0
            } else {
                LinearOutSlowInEasing.transform(state.distance_fraction()).clamp(0.0, 1.0)
            }
        }
    };

    let (box_refresh, bar_refresh) = (on_refresh.clone(), on_refresh.clone());
    Scaffold(
        modifier = Modifier.pull_to_refresh(
            state = state.clone(),
            is_refreshing = is_refreshing.get(),
            on_refresh = move || box_refresh(),
        ),
        top_bar = move || RefreshTopAppBar("TopAppBar", bar_refresh.clone()),
        content = move |it: PaddingValues| {
            let (item_count, is_refreshing, state, scale_fraction) =
                (item_count.clone(), is_refreshing.clone(), state.clone(), scale_fraction.clone());
            Box(
                modifier = Modifier.padding_values(it),
                content = move || {
                    let item_count = item_count.clone();
                    let refreshing = is_refreshing.get();
                    LazyColumn(
                        modifier = Modifier.fill_max_size(),
                        content = move |scope: &mut LazyListScope| {
                            if !refreshing {
                                let count = item_count.get();
                                scope.items(count, move |it| {
                                    ListItem(headline_content = move || Text(text = format!("Item {}", count - it)));
                                });
                            }
                        },
                    );
                    let (scale_fraction, state) = (scale_fraction.clone(), state.clone());
                    Box(
                        modifier = Modifier.align(Alignment::TopCenter).graphics_layer_block(move |layer| {
                            layer.scale_x = scale_fraction();
                            layer.scale_y = scale_fraction();
                        }),
                        content = move || {
                            PullToRefreshDefaults.LoadingIndicator(state = state.clone(), is_refreshing = refreshing)
                        },
                    );
                },
            );
        },
    );
}

/// A sample's window: 400 × 800, the 64 dp top bar over the list.
fn sample_rule(sample: impl Content + 'static) -> KompozTestRule {
    KompozTestRule::with_viewport(sample, IntSize::new(400, 800))
}

/// The first item's text, if the list shows one (the list under the 64 dp bar).
fn first_item(rule: &KompozTestRule) -> Option<String> {
    let items: Vec<_> = rule
        .all_nodes(true)
        .iter()
        .filter_map(|n| {
            let text = n.config().get(SemanticsProperties::Text)?.first()?.text().to_string();
            // (shown: the list composes and places past its viewport what it keeps around, clipped to nothing)
            let shown = n.bounds_in_root();
            (text.starts_with("Item ") && shown.height() > 0.0).then_some((shown.top, text))
        })
        .collect();
    let (top, text) = items.into_iter().min_by(|a, b| a.0.total_cmp(&b.0))?;
    assert!(top >= 64.0, "{text} at {top}");
    Some(text)
}

/// Pulled 300 down on the list, held, let go: past the 80 dp threshold.
fn pull_past(rule: &KompozTestRule) {
    slow_drag20(rule, at(200.0, 150.0), at(200.0, 450.0));
}

/// Pulled 88 down on the list (80 past the slop: the state half way, 40 of 80), held there.
fn pull_half(rule: &KompozTestRule) {
    pull_to(rule, at(200.0, 150.0), at(200.0, 238.0));
}

/// A click of the pointer on what `description` describes (`perform_click` waits until idle: the indicator's
/// animations would take the samples' fetch time with them).
fn click_without_waiting(rule: &KompozTestRule, description: &str) {
    let at = rule.on_node_with_content_description(description).get_unclipped_bounds_in_root().center();
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.host().pointer_up(at, -1, 0);
}

/// What the samples do: a pull past the threshold refreshes (an endless indicator shown) and 5 more items come after
/// `ms`; the bar's action refreshes too. `hides_list`: the list is gone while refreshing.
fn refreshes_by_pull_and_action(rule: &KompozTestRule, ms: i64, hides_list: bool) {
    assert_eq!(first_item(rule).as_deref(), Some("Item 15"));
    pull_past(rule);
    rule.advance_time_by(500);
    assert!(endless_progress_shown(rule));
    assert_eq!(first_item(rule).is_none(), hides_list);
    rule.advance_time_by(ms);
    rule.wait_for_idle();
    assert_eq!(first_item(rule).as_deref(), Some("Item 20"));
    assert!(!endless_progress_shown(rule));
    click_without_waiting(rule, "Trigger Refresh");
    rule.advance_time_by(500);
    assert!(endless_progress_shown(rule));
    rule.advance_time_by(ms);
    rule.wait_for_idle();
    assert_eq!(first_item(rule).as_deref(), Some("Item 25"));
    assert!(!endless_progress_shown(rule));
}

fn pull_to_refresh_sample() {
    let rule = sample_rule(PullToRefreshSample);
    refreshes_by_pull_and_action(&rule, 5000, false);
}

fn pull_to_refresh_with_loading_indicator_sample() {
    let rule = sample_rule(PullToRefreshWithLoadingIndicatorSample);
    // Half way: the loading indicator's determinate one at 0.5 (the default arrow tells no progress).
    pull_half(&rule);
    assert_eq!(progress_info(&rule), Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0)));
    let_go(&rule, at(200.0, 238.0));
    refreshes_by_pull_and_action(&rule, 5000, false);
}

fn pull_to_refresh_view_model_sample() {
    let rule = sample_rule(PullToRefreshViewModelSample);
    assert_eq!(first_item(&rule).as_deref(), Some("Item 15"));
    // The count goes up at once, the list hidden while refreshing; the action turned off meanwhile.
    pull_past(&rule);
    rule.advance_time_by(500);
    assert!(endless_progress_shown(&rule) && first_item(&rule).is_none());
    rule.on_node_with_content_description("Trigger Refresh").assert_is_not_enabled();
    rule.advance_time_by(5000);
    rule.wait_for_idle();
    assert_eq!(first_item(&rule).as_deref(), Some("Item 20"));
    rule.on_node_with_content_description("Trigger Refresh").assert_is_enabled().perform_click();
    rule.advance_time_by(500);
    assert!(endless_progress_shown(&rule));
    rule.advance_time_by(5000);
    rule.wait_for_idle();
    assert_eq!(first_item(&rule).as_deref(), Some("Item 25"));
}

/// Whether a layer was drawn scaled by `scale`.
fn layer_scaled_by(canvas: &kompoz_test::RecordingCanvas, scale: f32) -> bool {
    canvas.transforms.iter().any(|m| (m.values[0] - scale).abs() < 0.01 && (m.values[5] - scale).abs() < 0.01)
}

fn pull_to_refresh_scaling_sample() {
    let rule = sample_rule(PullToRefreshScalingSample);
    // Half way: the indicator scaled by LinearOutSlowIn of the fraction.
    pull_half(&rule);
    let scale = LinearOutSlowInEasing.transform(0.5);
    assert!(layer_scaled_by(&rule.draw(), scale), "{scale}");
    let_go(&rule, at(200.0, 238.0));
    refreshes_by_pull_and_action(&rule, 5000, true);
}

fn pull_to_refresh_linear_progress_indicator_sample() {
    let rule = sample_rule(PullToRefreshLinearProgressIndicatorSample);
    // Half way: the linear indicator at the fraction.
    pull_half(&rule);
    assert_eq!(progress_info(&rule), Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0)));
    let_go(&rule, at(200.0, 238.0));
    refreshes_by_pull_and_action(&rule, 5000, true);
}

fn pull_to_refresh_sample_custom_state() {
    let rule = sample_rule(PullToRefreshSampleCustomState);
    assert_eq!(first_item(&rule).as_deref(), Some("Item 15"));
    // Let go past the threshold, the state goes to it with its high bouncy spring: past it (the indicator box out
    // further than its 80 dp less its 40) and back.
    pull_to(&rule, at(200.0, 150.0), at(200.0, 450.0));
    rule.advance_time_by(200);
    rule.host().pointer_up(at(200.0, 450.0), -1, 0);
    let mut furthest = f32::MIN;
    for _ in 0..40 {
        rule.advance_frame();
        let canvas = rule.draw();
        furthest = canvas.transforms.iter().map(|m| m.values[13]).filter(|y| *y < 200.0).fold(furthest, f32::max);
    }
    assert!(furthest > 80.0 - 40.0 + 1.0, "{furthest}");
    assert!(endless_progress_shown(&rule) && first_item(&rule).is_none());
    rule.advance_time_by(5000);
    rule.wait_for_idle();
    assert_eq!(first_item(&rule).as_deref(), Some("Item 20"));
}

fn pull_to_refresh_custom_indicator_with_default_transform() {
    let rule = sample_rule(PullToRefreshCustomIndicatorWithDefaultTransform);
    // Half way: its circular indicator at the fraction, without a shadow (elevation 0).
    pull_half(&rule);
    assert_eq!(progress_info(&rule), Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0)));
    assert!(rule.draw().elevation_shadows.is_empty());
    let_go(&rule, at(200.0, 238.0));
    refreshes_by_pull_and_action(&rule, 1500, false);
}

fn loading_indicator_pull_to_refresh_sample() {
    let rule = sample_rule(LoadingIndicatorPullToRefreshSample);
    // Half way: the loading indicator scaled by LinearOutSlowIn of the fraction, its determinate one at 0.5.
    pull_half(&rule);
    assert!(layer_scaled_by(&rule.draw(), LinearOutSlowInEasing.transform(0.5)));
    assert_eq!(progress_info(&rule), Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0)));
    let_go(&rule, at(200.0, 238.0));
    refreshes_by_pull_and_action(&rule, 5000, true);
}

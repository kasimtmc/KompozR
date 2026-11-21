//! Material's swipe to dismiss and pull to refresh as Compose's material3 (1.5.0-alpha17) has them: the C++
//! Kompoz's tests by name (kompoz_tests.cpp; M15's behaviour where the C++ differed, said at each), Compose's
//! behaviour (the state's values, constructors and savers, the directions allowed, the fling, the callback; the
//! pull's state, its taking and giving back of scrolls, the indicator box, the defaults), and the M15 sample of
//! SwipeToDismissSamples.kt. (PullToRefreshSamples.kt's samples compose WP4a's Scaffold and TopAppBar and WP2's
//! progress indicators: they come with those packages.)

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::animate_color_as_state;
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_ui::input::PointerType;
use kompoz_ui::platform::LocalLayoutDirection;
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
    pull_to_refresh_modifier_slows_past_the_threshold_and_can_be_turned_off,
    pull_to_refresh_ignores_pulls_while_its_state_animates,
    pull_to_refresh_lets_an_animation_the_app_starts_win_over_the_finger,
    pull_to_refresh_keeps_a_fling_down_from_the_list,
    pull_to_refresh_starts_where_is_refreshing_says,
    pull_to_refresh_states_animate_snap_and_save_their_fraction,
    pull_to_refresh_takes_a_pull_only_from_the_start_and_by_the_user,
    pull_to_refresh_indicator_boxes_come_out_from_under_the_top_edge,
    pull_to_refresh_defaults_follow_the_tokens,
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
//
// (The C++'s pull_to_refresh_refreshes_past_the_threshold, pull_to_refresh_goes_back_in_before_the_list_scrolls,
// pull_to_refresh_box_can_be_turned_off and pull_to_refresh_indicator_comes_out_from_under_the_top_edge compose
// PullToRefreshBox / PullToRefreshDefaults.Indicator, whose refreshing branch is WP2's CircularProgressIndicator: they
// come with it. The indicator's arrow is checked by the unit test in src/pulltorefresh.rs.)

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

/// Down at `from`, moved over 20 frames to `to`, held there a frame: what the moves started run (a drag's deltas are
/// taken by its scrollable's coroutine, the pull's snaps by the node's, both dispatched as Compose's: the C++ took
/// them at once).
fn pull_to(rule: &KompozTestRule, from: Offset, to: Offset) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / 20.0), -1, 0);
    }
    rule.advance_frame();
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

fn pull_to_refresh_modifier_slows_past_the_threshold_and_can_be_turned_off() {
    // (The C++ drew PullToRefreshDefaults::Indicator beside the list, WP2's CircularProgressIndicator's: its checks
    // are all the state's, made here without it.)
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
                    content = || Rows(Some(list.clone()), "list".to_string()),
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

fn pull_to_refresh_defaults_follow_the_tokens() {
    assert_eq!(PullToRefreshDefaults::PositionalThreshold, 80.dp());
    assert_eq!(PullToRefreshDefaults::IndicatorMaxDistance, 80.dp());
    assert_eq!(PullToRefreshDefaults::Elevation, 3.dp()); // (ElevationTokens.Level2)
    assert_eq!(PullToRefreshDefaults::LoadingIndicatorElevation, 0.dp()); // (Level0)
    assert!(PullToRefreshDefaults::indicator_shape() == CircleShape);
    assert!(PullToRefreshDefaults::shape() == CircleShape);
    let colors = slot::<(Color, Color, Color)>();
    let rule = {
        let colors = colors.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            MaterialTheme(
                color_scheme = dark_color_scheme(),
                content = || {
                    *colors.borrow_mut() = Some((
                        PullToRefreshDefaults::indicator_container_color(),
                        PullToRefreshDefaults::container_color(),
                        PullToRefreshDefaults::indicator_color(),
                    ));
                },
            );
        }))
    };
    rule.wait_for_idle();
    let dark = dark_color_scheme();
    assert_eq!(get(&colors), (dark.surface_container_high, dark.surface_container_high, dark.on_surface_variant));
}

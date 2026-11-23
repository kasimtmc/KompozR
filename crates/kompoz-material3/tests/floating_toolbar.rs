//! Material's floating toolbars as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's test by name
//! (kompoz_tests.cpp; M15's behaviour and values where the C++ differed, said at each), Compose's behaviour (the
//! screen reader's actions, the FAB's places and sizes, the balanced padding, the vertical nested scroll's thresholds,
//! the exit directions right to left, the overrides, the defaults) and M15's FloatingToolbarSamples (all 11). Density
//! 1: dp are px; the viewport is 400 × 800 unless said.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::{exponential_decay, spring};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_extended::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_ui::input::PointerType;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::{CustomAccessibilityAction, SemanticsActions};
use kompoz_ui::tooling::preview::LoremIpsum;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

tests!(
    // (the C++ Kompoz's)
    floating_toolbar_shows_its_side_actions_when_expanded_and_leaves_on_scroll,
    // (Compose's behaviour)
    floating_toolbars_give_a_screen_reader_an_action_to_collapse_or_expand_them,
    fab_toolbars_grow_out_of_their_fab_at_either_end,
    a_toolbar_balances_its_contents_room_while_its_side_content_is_away,
    vertical_nested_scroll_expands_and_collapses_by_its_thresholds,
    exit_always_moves_towards_its_direction_and_mirrors_right_to_left,
    an_offscreen_toolbar_takes_no_keyboard_focus,
    floating_toolbar_overrides_make_the_toolbars_below,
    floating_toolbar_defaults_colors_and_values,
    // (M15's samples)
    expandable_horizontal_floating_toolbar_sample,
    overflowing_horizontal_floating_toolbar_sample,
    scrollable_horizontal_floating_toolbar_sample,
    expandable_vertical_floating_toolbar_sample,
    overflowing_vertical_floating_toolbar_sample,
    scrollable_vertical_floating_toolbar_sample,
    horizontal_floating_toolbar_with_fab_sample,
    centered_horizontal_floating_toolbar_with_fab_sample,
    vertical_floating_toolbar_with_fab_sample,
    centered_vertical_floating_toolbar_with_fab_sample,
    horizontal_floating_toolbar_as_scaffold_fab_sample,
);

/// Waits for idle with the clock advancing on its own (Compose's `mainClock.autoAdvance = true; waitForIdle()`):
/// what a test stepping the clock by hand does to let the animations run their course; its `auto_advance` put
/// back after.
fn finish_animations(rule: &KompozTestRule) {
    let clock = rule.main_clock();
    let was = clock.auto_advance();
    clock.set_auto_advance(true);
    rule.wait_for_idle();
    clock.set_auto_advance(was);
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left) && near(a.top, b.top) && near(a.right, b.right) && near(a.bottom, b.bottom)
}

/// The C++'s `{x, y, width, height}`.
fn xywh(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    named_args!(rule.on_node_with_tag(tag, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
}

fn exists(rule: &KompozTestRule, tag: &str) -> bool {
    named_args!(rule.on_all_nodes_with_tag(tag, use_unmerged_tree = true)).fetch_semantics_nodes().len() == 1
}

/// The radius and color of the rect drawn at `b` (the C++'s `fill`).
fn fill(rule: &KompozTestRule, b: Rect) -> Option<(f32, Color)> {
    let canvas = rule.draw();
    canvas
        .rects
        .iter()
        .zip(canvas.rect_radii.iter().zip(&canvas.rect_colors))
        .find(|(r, _)| same_rect(**r, b))
        .map(|(_, (radius, color))| (*radius, *color))
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

/// Lets frames run until no overscroll stretch is drawn (the swipe back at the top stretched the list): a stretch
/// springs back by its draws asking for the next frame (Android's EdgeEffect), which isn't work waiting for idle
/// waits for (Compose's idling resource counts recompositions, frame awaiters and layouts, not a redraw), so the
/// clock is advanced to it (Compose's `advanceTimeUntil`); a swipe while it is still stretched would take the
/// stretch back first.
fn spring_back(rule: &KompozTestRule) {
    let stretched = || rule.draw().layer_stretches.iter().any(|s| s.is_some_and(|s| !s.is_empty()));
    named_args!(rule.main_clock().advance_time_until(timeout_millis = 3_000, condition = || !stretched()));
}

/// A drag held still before it is let go (no fling).
fn held_drag(rule: &KompozTestRule, from: Offset, to: Offset) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / 10.0), -1, 0);
    }
    for _ in 0..10 {
        rule.advance_frame();
        h.pointer_move(to, -1, 0);
    }
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

/// Where what is in the node tagged `tag` is (its first child's bounds): a toolbar's tag sits outside the layout its
/// scroll behavior moves.
fn moved(rule: &KompozTestRule, tag: &str) -> Rect {
    named_args!(rule.on_node_with_tag(tag, use_unmerged_tree = true))
        .on_children()
        .on_first()
        .get_unclipped_bounds_in_root()
}

/// A 48 dp action tagged `tag` (the C++'s `action`).
#[kompozable]
fn Action(tag: &'static str) {
    Box(modifier = Modifier.size(48.dp()).test_tag(tag));
}

fn slot(content: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(content))
}

/// The screen reader's actions of the node whose child is tagged `tag` (the parent semantics of the toolbar's
/// content).
fn custom_actions(rule: &KompozTestRule, tag: &str) -> Vec<CustomAccessibilityAction> {
    named_args!(rule.on_node_with_tag(tag, use_unmerged_tree = true))
        .on_parent()
        .fetch_semantics_node()
        .config()
        .get(SemanticsActions::CustomActions)
        .unwrap_or_default()
}

// ---- Ported from C++ -------------------------------------------------------------------------------------------

/// The C++ test with M15's values: a FAB toolbar keeps its toolbar composed (zero wide, clipped) and its width the
/// expanded one's (M15's layout reserves the toolbar's room: the C++'s collapsed one was only the FAB); the FAB is 80
/// dp collapsed and 56 dp expanded (M15's `FabSizeRange` lerped by `1 - progress`; the C++ had 56 collapsed); the
/// standard FAB is in the primary container (M15's `fabContainerColor`; the C++ had the secondary container); the
/// state's offsets are negative (M15: 0 down to the offset limit; the C++ counted them positive).
fn floating_toolbar_shows_its_side_actions_when_expanded_and_leaves_on_scroll() {
    let expanded = mutable_state_of(false);
    let cs = light_color_scheme();
    let rule = {
        let expanded = expanded.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let e = expanded.get();
            Column(
                content = move || {
                    HorizontalFloatingToolbar(
                        e,
                        modifier = Modifier.test_tag("yatay"),
                        leading_content = slot(|| Action("baş")),
                        trailing_content = slot(|| Action("son")),
                        content = || Action("orta"),
                    );
                    VerticalFloatingToolbar(
                        e,
                        modifier = Modifier.test_tag("dikey"),
                        colors = FloatingToolbarDefaults.vibrant_floating_toolbar_colors(),
                        trailing_content = slot(|| Action("alt")),
                        content = || Action("dikey-orta"),
                    );
                    HorizontalFloatingToolbarWithFab(
                        e,
                        || {
                            FloatingToolbarDefaults.StandardFloatingActionButton(
                                || {},
                                modifier = Modifier.test_tag("fab"),
                                content = || {},
                            )
                        },
                        modifier = Modifier.test_tag("fablı"),
                        content = || Action("araç"),
                    );
                },
            );
        }))
    };
    let bar = bounds(&rule, "yatay");
    // Collapsed: the content, 8 dp around.
    assert!(same_rect(bar, xywh(0.0, 0.0, 64.0, 64.0)) && same_rect(bounds(&rule, "orta"), xywh(8.0, 8.0, 48.0, 48.0)));
    let (radius, color) = fill(&rule, bar).expect("the toolbar's container");
    assert!(near(radius, 32.0) && color == cs.surface_container, "a pill: {radius} {color:?}");
    assert!(!exists(&rule, "baş") && !exists(&rule, "son") && !exists(&rule, "alt"));
    let column = bounds(&rule, "dikey");
    assert!(same_rect(column, xywh(0.0, 64.0, 64.0, 64.0)), "{column:?}");
    assert_eq!(fill(&rule, column).map(|f| f.1), Some(cs.primary_container)); // vibrant
    // The FAB toolbar: as wide as expanded (toolbar 64 + 8 + the smallest FAB 56), 80 tall, the FAB 80 dp at its end.
    let (with_fab, fab) = (bounds(&rule, "fablı"), bounds(&rule, "fab"));
    assert!(same_rect(with_fab, xywh(0.0, 128.0, 128.0, 80.0)), "{with_fab:?}");
    assert!(same_rect(fab, xywh(48.0, 128.0, 80.0, 80.0)), "{fab:?}");
    assert!(exists(&rule, "araç")); // composed, the toolbar zero wide around it
    assert_eq!(fill(&rule, fab).map(|f| f.1), Some(cs.primary_container));

    expanded.set(true);
    rule.wait_for_idle();
    let bar = bounds(&rule, "yatay");
    assert!(near(bar.width(), 8.0 + 3.0 * 48.0 + 8.0), "{bar:?}");
    assert!(near(bounds(&rule, "baş").left, 8.0) && near(bounds(&rule, "orta").left, 56.0));
    assert!(near(bounds(&rule, "son").left, 104.0));
    let column = bounds(&rule, "dikey");
    assert!(near(column.height(), 8.0 + 2.0 * 48.0 + 8.0) && near(bounds(&rule, "alt").top, column.top + 56.0));
    let (tools, fab) = (bounds(&rule, "araç"), bounds(&rule, "fab"));
    // The toolbar (8 dp padding), 8 dp, then the FAB, now 56.
    assert!(near(fab.left - tools.right, 8.0 + 8.0) && near(fab.width(), 56.0), "{tools:?} {fab:?}");

    // Out of view as the content scrolls forwards, back as it scrolls back.
    let behavior: Rc<RefCell<Option<Rc<dyn FloatingToolbarScrollBehavior>>>> = Rc::new(RefCell::new(None));
    let page = {
        let behavior = behavior.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let kept = FloatingToolbarDefaults.exit_always_scroll_behavior(FloatingToolbarExitDirection::Bottom);
                let side = FloatingToolbarDefaults.exit_always_scroll_behavior(FloatingToolbarExitDirection::End);
                *behavior.borrow_mut() = Some(kept.clone());
                let (k, s) = (kept.clone(), side.clone());
                Box(
                    modifier = Modifier.size_wh(400.dp(), 600.dp()).nested_scroll(kept).nested_scroll(side),
                    content = move || {
                        LazyColumn(
                            modifier = Modifier.fill_max_size(),
                            content = |scope: &mut LazyListScope| {
                                scope.items(100, |_| Box(modifier = Modifier.fill_max_width().height(60.dp())));
                            },
                        );
                        HorizontalFloatingToolbar(
                            true,
                            modifier =
                                Modifier.align(Alignment::BottomCenter).offset(0.dp(), (-16).dp()).test_tag("araçlar"),
                            scroll_behavior = Some(k.clone()),
                            content = || Action("sayfa"),
                        );
                        VerticalFloatingToolbar(
                            true,
                            modifier = Modifier.align(Alignment::CenterEnd).offset((-16).dp(), 0.dp()),
                            scroll_behavior = Some(s.clone()),
                            content = || Action("yan"),
                        );
                    },
                );
            }),
            IntSize::new(400, 600),
        )
    };
    let state = || behavior.borrow().clone().expect("composed").state();
    // (the clock by hand: the checks after a gesture see the toolbar before it settles)
    page.main_clock().set_auto_advance(false);
    let moved = || bounds(&page, "sayfa"); // an action in it: where the toolbar's offsets put it
    let (shown, beside) = (moved(), bounds(&page, "yan"));
    assert!(near(shown.bottom, 600.0 - 16.0 - 8.0) && near(state().offset_limit(), -(64.0 + 16.0)), "{shown:?}");
    swipe(&page, Offset::new(200.0, 400.0), Offset::new(200.0, 370.0), 10); // 30 dp forwards (less the touch slop)
    page.advance_frame();
    let went = -state().offset();
    assert!(went > 5.0 && went < 30.5 && near(moved().top, shown.top + went), "{went} {:?}", moved());
    swipe(&page, Offset::new(200.0, 400.0), Offset::new(200.0, 300.0), 10); // past its limit: gone
    finish_animations(&page);
    assert!(near(state().offset(), -80.0) && moved().top >= 600.0 - 0.5, "{} {:?}", state().offset(), moved());
    let side = bounds(&page, "yan");
    assert!(near(side.left, beside.left + 80.0) && near(side.top, beside.top), "{side:?} {beside:?}"); // off the end
    // Back over half way, then held still and let go: it comes all the way back.
    let h = page.host();
    h.pointer_down(Offset::new(200.0, 300.0), -1, 0, PointerType::Touch);
    for i in 1..=12 {
        page.advance_frame();
        h.pointer_move(Offset::new(200.0, 300.0 + 5.0 * i as f32), -1, 0);
    }
    for _ in 0..10 {
        page.advance_frame();
        h.pointer_move(Offset::new(200.0, 360.0), -1, 0);
    }
    let halfway = -state().offset();
    h.pointer_up(Offset::new(200.0, 360.0), -1, 0);
    finish_animations(&page);
    assert!(halfway > 5.0 && halfway < 40.0, "{halfway}"); // (not by a fling: there was none)
    assert!(near(state().offset(), 0.0) && same_rect(moved(), shown), "{} {:?}", state().offset(), moved());
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

/// Each toolbar's content is a parent semantics node giving a screen reader one action: "Collapse toolbar" while
/// expanded, "Expand toolbar" while collapsed; collapsing forces it collapsed whatever `expanded` says, expanding
/// lets `expanded` say again.
fn floating_toolbars_give_a_screen_reader_an_action_to_collapse_or_expand_them() {
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(
            content = || {
                HorizontalFloatingToolbar(true, leading_content = slot(|| Action("baş")), content = || Action("orta"));
                VerticalFloatingToolbarWithFab(
                    true,
                    || Box(modifier = Modifier.fill_max_size().test_tag("fab")),
                    content = || Action("araç"),
                );
            },
        );
    }));
    let labels = |tag: &str| custom_actions(&rule, tag).iter().map(|a| a.label.clone()).collect::<Vec<_>>();
    assert_eq!(labels("orta"), ["Collapse toolbar"]);
    assert_eq!(labels("fab"), ["Collapse toolbar"]);
    assert!(exists(&rule, "baş"));
    assert!((custom_actions(&rule, "orta")[0].action)());
    rule.wait_for_idle();
    assert!(!exists(&rule, "baş"), "forced collapsed");
    assert_eq!(labels("orta"), ["Expand toolbar"]);
    assert!((custom_actions(&rule, "orta")[0].action)());
    rule.wait_for_idle();
    assert!(exists(&rule, "baş") && labels("orta") == ["Collapse toolbar"]);
    // The FAB toolbar's: collapsing shrinks its toolbar to nothing and grows the FAB to 80 dp.
    assert!(near(bounds(&rule, "fab").height(), 56.0));
    assert!((custom_actions(&rule, "fab")[0].action)());
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").height(), 80.0) && labels("fab") == ["Expand toolbar"]);
}

/// A FAB toolbar's FAB at its start (horizontal) or top (vertical), the toolbar growing out beside it along the
/// animation spec; the toolbar's layer casts the elevation lerped between the collapsed and expanded ones.
fn fab_toolbars_grow_out_of_their_fab_at_either_end() {
    let expanded = mutable_state_of(true);
    let rule = {
        let expanded = expanded.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let e = expanded.get();
            Column(
                content = move || {
                    HorizontalFloatingToolbarWithFab(
                        e,
                        || Box(modifier = Modifier.fill_max_size().test_tag("baştaki")),
                        modifier = Modifier.test_tag("yatay"),
                        floating_action_button_position = FloatingToolbarHorizontalFabPosition::Start,
                        content = || {
                            Action("a");
                            Action("b");
                        },
                    );
                    VerticalFloatingToolbarWithFab(
                        e,
                        || Box(modifier = Modifier.fill_max_size().test_tag("üstteki")),
                        modifier = Modifier.test_tag("dikey"),
                        floating_action_button_position = FloatingToolbarVerticalFabPosition::Top,
                        content = || {
                            Action("c");
                            Action("d");
                        },
                    );
                },
            );
        }))
    };
    // Across: the FAB (56) first, 8 dp, the toolbar (8 + 2 × 48 + 8 = 112): 176 wide, 80 tall.
    let row = bounds(&rule, "yatay");
    assert!(same_rect(row, xywh(0.0, 0.0, 176.0, 80.0)), "{row:?}");
    assert!(same_rect(bounds(&rule, "baştaki"), xywh(0.0, 12.0, 56.0, 56.0)));
    assert!(near(bounds(&rule, "a").left, 64.0 + 8.0) && near(bounds(&rule, "a").top, 8.0 + 8.0));
    // Down: the FAB on top, the toolbar under it.
    let column = bounds(&rule, "dikey");
    assert!(same_rect(column, xywh(0.0, 80.0, 80.0, 176.0)), "{column:?}");
    assert!(same_rect(bounds(&rule, "üstteki"), xywh(12.0, 80.0, 56.0, 56.0)));
    assert!(near(bounds(&rule, "c").top, 80.0 + 64.0 + 8.0) && near(bounds(&rule, "d").top, 80.0 + 64.0 + 56.0));
    // (the clock by hand: the check on the way sees the button growing)
    rule.main_clock().set_auto_advance(false);
    // Collapsing: half way through the FAB is between 56 and 80, the toolbar's room shrinking towards the FAB.
    expanded.set(false);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let fab = bounds(&rule, "baştaki").width();
    assert!(fab > 56.0 && fab < 80.0, "{fab}");
    finish_animations(&rule);
    assert!(near(bounds(&rule, "baştaki").width(), 80.0) && near(bounds(&rule, "üstteki").height(), 80.0));
    assert!(same_rect(bounds(&rule, "yatay"), row), "the layout keeps the expanded room");
}

/// While neither side content shows, the content is padded to balance the room its minimum interactive sizes leave
/// around it: a 64 × 40 dp icon button (48 dp tall with its room: 4 dp above and below, none at its sides) gets 4
/// dp more at each side; with side content shown the padding animates away (Compose's private
/// minimumInteractiveBalancedPadding).
fn a_toolbar_balances_its_contents_room_while_its_side_content_is_away() {
    let expanded = mutable_state_of(false);
    let rule = {
        let expanded = expanded.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            HorizontalFloatingToolbar(
                expanded.get(),
                modifier = Modifier.test_tag("bar"),
                leading_content = slot(|| Action("baş")),
                content = || {
                    FilledIconButton(|| {}, modifier = Modifier.width(64.dp()).test_tag("düğme"), content = || {});
                },
            );
        }))
    };
    let (bar, button) = (bounds(&rule, "bar"), bounds(&rule, "düğme"));
    assert!(near(bar.width(), 8.0 + 4.0 + 64.0 + 4.0 + 8.0) && near(bar.height(), 64.0), "{bar:?}");
    assert!(near(button.left, 12.0), "{button:?}");
    expanded.set(true);
    rule.wait_for_idle();
    let (bar, button) = (bounds(&rule, "bar"), bounds(&rule, "düğme"));
    assert!(near(bar.width(), 8.0 + 48.0 + 64.0 + 8.0) && near(button.left, 56.0), "{bar:?} {button:?}");
}

/// `Modifier.floating_toolbar_vertical_nested_scroll`: collapse once the content scrolled 40 dp forwards past where it
/// last expanded, expand once it scrolled 40 dp back past where it last collapsed; other thresholds as given; a
/// reversed layout flips the directions.
fn vertical_nested_scroll_expands_and_collapses_by_its_thresholds() {
    let expanded = mutable_state_of(true);
    let reverse = mutable_state_of(false);
    let calls = Rc::new(RefCell::new(Vec::<&str>::new()));
    let scroll = Rc::new(RefCell::new(None::<ScrollState>));
    let rule = {
        let (expanded, reverse, calls, scroll) = (expanded.clone(), reverse.clone(), calls.clone(), scroll.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let state = remember_scroll_state();
            *scroll.borrow_mut() = Some(state.clone());
            let (e1, e2, c1, c2) = (expanded.clone(), expanded.clone(), calls.clone(), calls.clone());
            Column(
                modifier = Modifier
                    .fill_max_size()
                    .floating_toolbar_vertical_nested_scroll(
                        expanded.get(),
                        move || {
                            c1.borrow_mut().push("aç");
                            e1.set(true);
                        },
                        move || {
                            c2.borrow_mut().push("kapa");
                            e2.set(false);
                        },
                        collapse_scroll_distance_threshold = 60.dp(),
                        reverse_layout = reverse.get(),
                    )
                    .vertical_scroll(&state, reverse_scrolling = reverse.get()),
                content = || {
                    for _ in 0..40 {
                        Box(modifier = Modifier.fill_max_width().height(100.dp()));
                    }
                },
            );
        }))
    };
    // Drags through the nested scroll chain (what a user's finger does): forwards is up.
    let drag = |from: f32, to: f32| held_drag(&rule, Offset::new(200.0, from), Offset::new(200.0, to));
    drag(500.0, 450.0); // 50 less the slop: under 60
    rule.wait_for_idle();
    assert!(calls.borrow().is_empty() && expanded.get());
    // Past 60 in all: collapsed. (Compose calls `onCollapse` again for each forward scroll past the threshold: it
    // moves the threshold ahead of the offset, which keeps going forwards.)
    drag(500.0, 420.0);
    rule.wait_for_idle();
    assert!(!calls.borrow().is_empty() && calls.borrow().iter().all(|c| *c == "kapa"), "{:?}", calls.borrow());
    assert!(!expanded.get());
    calls.borrow_mut().clear();
    drag(400.0, 430.0); // back 30 less the slop: under 40
    rule.wait_for_idle();
    assert!(calls.borrow().is_empty() && !expanded.get(), "{:?}", calls.borrow());
    drag(400.0, 470.0); // back 70 more: past 40
    rule.wait_for_idle();
    assert_eq!(calls.borrow().first(), Some(&"aç"));
    assert!(calls.borrow().iter().all(|c| *c == "aç") && expanded.get());
    // Reversed: a finger moving down scrolls forwards.
    calls.borrow_mut().clear();
    reverse.set(true);
    rule.wait_for_idle();
    drag(300.0, 420.0);
    rule.wait_for_idle();
    assert_eq!(calls.borrow().first(), Some(&"kapa"));
    assert!(!expanded.get());
}

/// The exit-always behavior towards the start and the top, right to left: towards the start a horizontal one moves
/// left left to right, right right to left; its drag follows the mirror too; the limit is what takes it out of its
/// parent that way.
fn exit_always_moves_towards_its_direction_and_mirrors_right_to_left() {
    let rtl = mutable_state_of(false);
    let behaviors: Rc<RefCell<Vec<Rc<dyn FloatingToolbarScrollBehavior>>>> = Rc::new(RefCell::new(Vec::new()));
    let rule = {
        let (rtl, behaviors) = (rtl.clone(), behaviors.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let direction = if rtl.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
                let behaviors = behaviors.clone();
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
                    let start =
                        FloatingToolbarDefaults.exit_always_scroll_behavior(FloatingToolbarExitDirection::Start);
                    let top = FloatingToolbarDefaults.exit_always_scroll_behavior(FloatingToolbarExitDirection::Top);
                    *behaviors.borrow_mut() = vec![start.clone(), top.clone()];
                    let (s, t) = (start.clone(), top.clone());
                    Box(
                        modifier = Modifier.fill_max_size().nested_scroll(start).nested_scroll(top),
                        content = move || {
                            LazyColumn(
                                modifier = Modifier.fill_max_size(),
                                content = |scope: &mut LazyListScope| {
                                    scope.items(100, |_| Box(modifier = Modifier.fill_max_width().height(60.dp())));
                                },
                            );
                            VerticalFloatingToolbar(
                                true,
                                modifier = Modifier.align(Alignment::CenterStart).offset(16.dp(), 0.dp()),
                                scroll_behavior = Some(s.clone()),
                                content = || Action("baştaki"),
                            );
                            HorizontalFloatingToolbar(
                                true,
                                modifier = Modifier.align(Alignment::TopCenter).offset(0.dp(), 16.dp()),
                                scroll_behavior = Some(t.clone()),
                                content = || Action("üstteki"),
                            );
                        },
                    );
                });
            }),
            IntSize::new(400, 600),
        )
    };
    let state = |i: usize| behaviors.borrow()[i].state();
    assert_eq!(behaviors.borrow()[0].exit_direction(), FloatingToolbarExitDirection::Start);
    // Left to right: the start one 16 + 64 from the left edge, the top one 16 + 64 from the top.
    assert!(near(state(0).offset_limit(), -80.0) && near(state(1).offset_limit(), -80.0));
    let (start, top) = (bounds(&rule, "baştaki"), bounds(&rule, "üstteki"));
    swipe(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 300.0), 10);
    rule.wait_for_idle();
    assert!(
        near(bounds(&rule, "baştaki").left, start.left - 80.0) && near(bounds(&rule, "üstteki").top, top.top - 80.0)
    );
    // Right to left the start is the right edge.
    rtl.set(true);
    rule.wait_for_idle();
    swipe(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 450.0), 10); // back
    rule.wait_for_idle();
    assert!(near(state(0).offset(), 0.0), "{}", state(0).offset());
    let start = bounds(&rule, "baştaki");
    assert!(near(start.right, 400.0 - 16.0 - 8.0), "{start:?}");
    assert!(near(state(0).offset_limit(), -80.0), "{}", state(0).offset_limit());
    swipe(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 300.0), 10);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "baştaki").left, start.left + 80.0), "{:?}", bounds(&rule, "baştaki"));
}

/// A toolbar moved out of view by its scroll behavior can't take the keyboard's focus (Compose's
/// `focusProperties { canFocus = false }` while its offset isn't 0); back at rest it can.
fn an_offscreen_toolbar_takes_no_keyboard_focus() {
    let state = FloatingToolbarState(-80.0, 0.0, 0.0);
    let modes: Rc<RefCell<Option<kompoz_ui::platform::InputModeManager>>> = Rc::new(RefCell::new(None));
    let rule = {
        let (state, m) = (state.clone(), modes.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            *m.borrow_mut() = Some(kompoz_ui::platform::LocalInputModeManager.current());
            let behavior = remember({
                let state = state.clone();
                move || -> Rc<dyn FloatingToolbarScrollBehavior> {
                    Rc::new(ExitAlwaysFloatingToolbarScrollBehavior::new(
                        FloatingToolbarExitDirection::Bottom,
                        state,
                        std::sync::Arc::new(spring::<f32>()),
                        std::sync::Arc::new(exponential_decay::<f32>()),
                    ))
                }
            });
            HorizontalFloatingToolbar(
                true,
                modifier = Modifier.test_tag("bar"),
                scroll_behavior = Some(behavior),
                content = || {
                    IconButton(|| {}, modifier = Modifier.test_tag("düğme"), content = || {});
                },
            );
        }))
    };
    let focus = |rule: &KompozTestRule| {
        rule.on_node_with_tag("düğme").request_focus();
        rule.wait_for_idle();
        rule.on_node_with_tag("düğme")
            .fetch_semantics_node()
            .config()
            .get(kompoz_ui::semantics::SemanticsProperties::Focused)
            == Some(true)
    };
    // (the keyboard's mode asked for, as a Compose test does through LocalInputModeManager: a clickable takes the
    // focus only out of touch mode)
    rule.run_on_idle(|| modes.borrow().as_ref().unwrap().request_input_mode(kompoz_ui::platform::InputMode::Keyboard));
    state.set_offset(-40.0);
    rule.wait_for_idle();
    assert!(!focus(&rule), "moved out: no focus");
    state.set_offset(0.0);
    rule.wait_for_idle();
    assert!(focus(&rule));
}

/// An override provided below makes the toolbars of its kind, given their arguments; the default override is what
/// makes them otherwise (Compose's experimental component override API).
fn floating_toolbar_overrides_make_the_toolbars_below() {
    struct Plain(Rc<RefCell<Vec<String>>>);
    impl HorizontalFloatingToolbarOverride for Plain {
        #[track_caller]
        fn HorizontalFloatingToolbar(&self, scope: &HorizontalFloatingToolbarOverrideScope) {
            self.0.borrow_mut().push(format!(
                "{} {:?} {}",
                scope.is_expanded(),
                scope.expanded_shadow_elevation(),
                scope.leading_content().is_some()
            ));
            let content = scope.content();
            kompozable_block!({
                Row(modifier = scope.modifier(), content = move || content());
            })
        }
    }
    impl VerticalFloatingToolbarWithFabOverride for Plain {
        #[track_caller]
        fn VerticalFloatingToolbarWithFab(&self, scope: &VerticalFloatingToolbarWithFabOverrideScope) {
            self.0.borrow_mut().push(format!(
                "{} {}",
                scope.floating_action_button_position(),
                scope.colors() == scope.colors()
            ));
            let (fab, content) = (scope.floating_action_button(), scope.content());
            kompozable_block!({
                Column(
                    modifier = scope.modifier(),
                    content = move || {
                        content();
                        fab();
                    },
                );
            })
        }
    }
    let seen = Rc::new(RefCell::new(Vec::new()));
    let rule = {
        let seen = seen.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let horizontal: Rc<dyn HorizontalFloatingToolbarOverride> = Rc::new(Plain(seen.clone()));
            let vertical: Rc<dyn VerticalFloatingToolbarWithFabOverride> = Rc::new(Plain(seen.clone()));
            KompozitionLocalProvider(
                &[
                    LocalHorizontalFloatingToolbarOverride.provides(horizontal),
                    LocalVerticalToolbarWithFabOverride.provides(vertical),
                ],
                || {
                    Column(
                        content = || {
                            HorizontalFloatingToolbar(
                                false,
                                modifier = Modifier.test_tag("yatay"),
                                content = || Action("a"),
                            );
                            VerticalFloatingToolbarWithFab(
                                true,
                                || Action("fab"),
                                modifier = Modifier.test_tag("dikey"),
                                content = || Action("b"),
                            );
                            // Not overridden: the default.
                            VerticalFloatingToolbar(
                                true,
                                modifier = Modifier.test_tag("öntanımlı"),
                                content = || Action("c"),
                            );
                        },
                    );
                },
            );
        }))
    };
    assert_eq!(*seen.borrow(), ["false 0.dp false", "FloatingToolbarVerticalFabPosition.Bottom true"]);
    assert!(same_rect(bounds(&rule, "yatay"), xywh(0.0, 0.0, 48.0, 48.0))); // the override's plain row
    assert!(near(bounds(&rule, "fab").top, 48.0 + 48.0));
    assert!(same_rect(bounds(&rule, "öntanımlı"), xywh(0.0, 144.0, 64.0, 64.0)));
}

/// FloatingToolbarDefaults' values from the tokens (M15), the colors (standard: surface container + primary container
/// FAB; vibrant: primary container + tertiary container FAB; each with its content color; a color given wins), the
/// colors' copy and equality, the value classes' names, the transitions and the scroll behavior's arguments.
fn floating_toolbar_defaults_colors_and_values() {
    assert_eq!(FloatingToolbarDefaults::ContainerSize, 64.dp());
    assert_eq!(FloatingToolbarDefaults::ContainerExpandedElevation, 0.dp());
    assert_eq!(FloatingToolbarDefaults::ContainerCollapsedElevation, 0.dp());
    assert_eq!(FloatingToolbarDefaults::ContainerExpandedElevationWithFab, 1.dp());
    assert_eq!(FloatingToolbarDefaults::ContainerCollapsedElevationWithFab, 0.dp());
    assert_eq!(FloatingToolbarDefaults::ScreenOffset, 16.dp());
    assert_eq!(FloatingToolbarDefaults::ScrollDistanceThreshold, 40.dp());
    assert_eq!(FloatingToolbarDefaults::ContentPadding(), PaddingValues::new(8.dp(), 8.dp(), 8.dp(), 8.dp()));
    assert_eq!(FloatingToolbarHorizontalFabPosition::Start.to_string(), "FloatingToolbarHorizontalFabPosition.Start");
    assert_eq!(FloatingToolbarHorizontalFabPosition::End.to_string(), "FloatingToolbarHorizontalFabPosition.End");
    assert_eq!(FloatingToolbarVerticalFabPosition::Top.to_string(), "FloatingToolbarVerticalFabPosition.Top");
    assert_eq!(
        format!("{:?}", FloatingToolbarVerticalFabPosition::Bottom),
        "FloatingToolbarVerticalFabPosition.Bottom"
    );
    let directions = [
        FloatingToolbarExitDirection::Bottom,
        FloatingToolbarExitDirection::Top,
        FloatingToolbarExitDirection::Start,
        FloatingToolbarExitDirection::End,
    ];
    let names: Vec<String> = directions.iter().map(|d| d.to_string()).collect();
    assert_eq!(
        names,
        [
            "FloatingToolbarExitDirection.Bottom",
            "FloatingToolbarExitDirection.Top",
            "FloatingToolbarExitDirection.Start",
            "FloatingToolbarExitDirection.End"
        ]
    );
    let colors = FloatingToolbarColors::new(Color::Red, Color::Green, Color::Blue, Color::White);
    assert_eq!(named_args!(colors.copy(fab_content_color = Color::Black)).fab_content_color, Color::Black);
    assert_eq!(named_args!(colors.copy(toolbar_container_color = Color::Unspecified)), colors);
    let seen = Rc::new(RefCell::new(None));
    let dark = dark_color_scheme();
    {
        let seen = seen.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let seen = seen.clone();
            MaterialTheme(
                color_scheme = dark,
                content = move || {
                    let behavior =
                        FloatingToolbarDefaults.exit_always_scroll_behavior(FloatingToolbarExitDirection::Top);
                    *seen.borrow_mut() = Some((
                        FloatingToolbarDefaults.standard_floating_toolbar_colors(),
                        FloatingToolbarDefaults.vibrant_floating_toolbar_colors(),
                        FloatingToolbarDefaults.vibrant_floating_toolbar_colors(toolbar_content_color = Color::Red),
                        behavior.exit_direction(),
                        behavior.state().offset_limit(),
                        behavior.state().offset(),
                    ));
                },
            );
        }));
    }
    let (standard, vibrant, given, exit_direction, limit, offset) = (*seen.borrow()).expect("composed");
    assert_eq!(
        standard,
        FloatingToolbarColors::new(
            dark.surface_container,
            dark.on_surface,
            dark.primary_container,
            dark.on_primary_container
        )
    );
    assert_eq!(
        vibrant,
        FloatingToolbarColors::new(
            dark.primary_container,
            dark.on_primary_container,
            dark.tertiary_container,
            dark.on_tertiary_container
        )
    );
    assert_eq!(given.toolbar_content_color, Color::Red);
    assert_eq!(given.toolbar_container_color, dark.primary_container);
    assert_eq!(exit_direction, FloatingToolbarExitDirection::Top);
    // The default state: no limit yet, at rest.
    assert_eq!(limit, -f32::MAX);
    assert_eq!(offset, 0.0);
    // A state keeps its offset within its limit.
    let state = FloatingToolbarState(-50.0, -10.0, 3.0);
    state.set_offset(-80.0);
    assert_eq!(state.offset(), -50.0);
    state.set_offset(20.0);
    assert_eq!(state.offset(), 0.0);
    assert_eq!(state.content_offset(), 3.0);
}

// ---- M15's samples ---------------------------------------------------------------------------------------------
// (as written, in Rust; Kotlin's twins in crates/kompoz/tests/kt/floating_toolbar.kt)

/// The tooltip box around each sample's buttons.
#[kompozable]
fn Tooltipped(content: impl Fn() + 'static) {
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
        tooltip = |scope: &dyn TooltipScope| scope.PlainTooltip(content = || Text("Localized description")),
        state = remember_tooltip_state(),
        content = content,
    );
}

#[kompozable]
fn SampleIcon(image: ImageVector) {
    VectorIcon(image, Some("Localized description".into()));
}

/// The samples' private `LeadingContent`.
#[kompozable]
fn LeadingContent() {
    Tooltipped(|| IconButton(|| {}, content = || SampleIcon(Icons::Filled::Check())));
    Tooltipped(|| IconButton(|| {}, content = || SampleIcon(Icons::Filled::Edit())));
}

/// The samples' private `TrailingContent`.
#[kompozable]
fn TrailingContent() {
    Tooltipped(|| IconButton(|| {}, content = || SampleIcon(Icons::Filled::Download())));
    Tooltipped(|| IconButton(|| {}, content = || SampleIcon(Icons::Filled::Favorite())));
}

/// The 76 numbered rows of the samples' lists.
fn sample_rows(scope: &mut LazyListScope) {
    let list: Vec<String> = (0..=75).map(|it| it.to_string()).collect();
    let n = list.len() as i32;
    scope.items(n, move |it| kompozable_block!({ SampleRow(list[it as usize].clone()) }));
}

#[kompozable]
fn SampleRow(text: String) {
    Text(
        text,
        style = MaterialTheme::typography().body_large,
        modifier = Modifier.fill_max_width().padding_hv(16.dp(), 0.dp()),
    );
}

#[kompozable]
fn ExpandableHorizontalFloatingToolbarSample() {
    let expanded = remember_saveable(|| mutable_state_of(true));
    Scaffold(
        content = move |inner_padding: PaddingValues| {
            let expanded = expanded.clone();
            let padding = inner_padding.clone();
            Box(
                modifier = Modifier.padding_values(inner_padding),
                content = move || {
                    // The toolbar should receive focus before the screen content, so place it first.
                    // Make sure to set its zIndex so it's above the screen content visually.
                    HorizontalFloatingToolbar(
                        modifier = Modifier
                            .align(Alignment::BottomCenter)
                            .offset(0.dp(), -FloatingToolbarDefaults::ScreenOffset)
                            .z_index(1.0),
                        expanded = expanded.get(),
                        leading_content = slot(|| LeadingContent()),
                        trailing_content = slot(|| TrailingContent()),
                        content = || {
                            Tooltipped(|| {
                                FilledIconButton(
                                    modifier = Modifier.width(64.dp()),
                                    on_click = || {},
                                    content = || SampleIcon(Icons::Filled::Add()),
                                )
                            })
                        },
                    );
                    let (e1, e2) = (expanded.clone(), expanded.clone());
                    LazyColumn(
                        // Apply a floatingToolbarVerticalNestedScroll Modifier toggle the expanded state of the
                        // HorizontalFloatingToolbar.
                        modifier = Modifier.floating_toolbar_vertical_nested_scroll(
                            expanded = expanded.get(),
                            on_expand = move || e1.set(true),
                            on_collapse = move || e2.set(false),
                        ),
                        state = remember_lazy_list_state(),
                        content_padding = padding.clone(),
                        vertical_arrangement = Arrangement::spaced_by(8.dp()),
                        content = sample_rows,
                    );
                },
            );
        },
    );
}

/// The descriptions of the icons shown (every icon of the samples has the same one).
fn shown_icons(rule: &KompozTestRule) -> usize {
    named_args!(rule.on_all_nodes_with_content_description("Localized description", use_unmerged_tree = true))
        .fetch_semantics_nodes()
        .len()
}

fn expandable_horizontal_floating_toolbar_sample() {
    let rule = KompozTestRule::new(ExpandableHorizontalFloatingToolbarSample);
    assert_eq!(shown_icons(&rule), 5); // leading 2, the content's, trailing 2
    swipe(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 300.0), 10); // forwards: collapses
    rule.wait_for_idle();
    assert_eq!(shown_icons(&rule), 1);
    swipe(&rule, Offset::new(200.0, 300.0), Offset::new(200.0, 400.0), 10); // back: expands
    rule.wait_for_idle();
    assert_eq!(shown_icons(&rule), 5);
}

#[kompozable]
fn OverflowingHorizontalFloatingToolbarSample() {
    Scaffold(
        content = move |inner_padding: PaddingValues| {
            let padding = inner_padding.clone();
            Box(
                modifier = Modifier.padding_values(inner_padding),
                content = move || {
                    HorizontalFloatingToolbar(
                        modifier = Modifier
                            .align(Alignment::BottomCenter)
                            .offset(0.dp(), -FloatingToolbarDefaults::ScreenOffset)
                            .z_index(1.0),
                        expanded = true,
                        leading_content = slot(|| LeadingContent()),
                        trailing_content = slot(|| {
                            AppBarRow(
                                content = |scope: &dyn AppBarRowScope| {
                                    scope.clickable_item(
                                        || {},
                                        || SampleIcon(Icons::Filled::Download()),
                                        "Download".into(),
                                    );
                                    scope.clickable_item(
                                        || {},
                                        || SampleIcon(Icons::Filled::Favorite()),
                                        "Favorite".into(),
                                    );
                                    scope.clickable_item(|| {}, || SampleIcon(Icons::Filled::Add()), "Add".into());
                                    scope.clickable_item(
                                        || {},
                                        || SampleIcon(Icons::Filled::Person()),
                                        "Person".into(),
                                    );
                                    scope.clickable_item(
                                        || {},
                                        || SampleIcon(Icons::Filled::ArrowUpward()),
                                        "ArrowUpward".into(),
                                    );
                                },
                            )
                        }),
                        content = || {
                            Tooltipped(|| {
                                FilledIconButton(
                                    modifier = Modifier.width(64.dp()),
                                    on_click = || {},
                                    content = || SampleIcon(Icons::Filled::Add()),
                                )
                            })
                        },
                    );
                    LazyColumn(
                        state = remember_lazy_list_state(),
                        content_padding = padding.clone(),
                        vertical_arrangement = Arrangement::spaced_by(8.dp()),
                        content = sample_rows,
                    );
                },
            );
        },
    );
}

fn overflowing_horizontal_floating_toolbar_sample() {
    // 400 dp wide: the leading content (96), the content (64) and the padding (16) leave the AppBarRow less than its
    // five 48 dp items: some go in its overflow menu, behind "More options".
    let rule = KompozTestRule::new(OverflowingHorizontalFloatingToolbarSample);
    let more = named_args!(rule.on_all_nodes_with_content_description("More options", use_unmerged_tree = true))
        .fetch_semantics_nodes()
        .len();
    assert_eq!(more, 1);
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("ArrowUpward").assert_exists();
}

#[kompozable]
fn ScrollableHorizontalFloatingToolbarSample() {
    let exit_always_scroll_behavior =
        FloatingToolbarDefaults.exit_always_scroll_behavior(exit_direction = FloatingToolbarExitDirection::Bottom);
    let behavior = exit_always_scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(exit_always_scroll_behavior.clone()).test_tag("iskele"),
        content = move |inner_padding: PaddingValues| {
            let padding = inner_padding.clone();
            let behavior = behavior.clone();
            Box(
                modifier = Modifier.padding_values(inner_padding),
                content = move || {
                    HorizontalFloatingToolbar(
                        modifier = Modifier
                            .align(Alignment::BottomCenter)
                            .offset(0.dp(), -FloatingToolbarDefaults::ScreenOffset)
                            .z_index(1.0)
                            .test_tag("araçlar"),
                        expanded = true,
                        leading_content = slot(|| LeadingContent()),
                        trailing_content = slot(|| TrailingContent()),
                        content = || {
                            Tooltipped(|| {
                                FilledIconButton(
                                    modifier = Modifier.width(64.dp()),
                                    on_click = || {},
                                    content = || SampleIcon(Icons::Filled::Add()),
                                )
                            })
                        },
                        scroll_behavior = Some(behavior.clone()),
                    );
                    LazyColumn(
                        state = remember_lazy_list_state(),
                        content_padding = padding.clone(),
                        vertical_arrangement = Arrangement::spaced_by(8.dp()),
                        content = sample_rows,
                    );
                },
            );
        },
    );
}

fn scrollable_horizontal_floating_toolbar_sample() {
    let rule = KompozTestRule::new(ScrollableHorizontalFloatingToolbarSample);
    let shown = bounds(&rule, "araçlar");
    assert!(near(shown.bottom, 800.0 - 16.0), "{shown:?}");
    let inner = moved(&rule, "araçlar");
    swipe(&rule, Offset::new(200.0, 400.0), Offset::new(200.0, 200.0), 10);
    rule.wait_for_idle();
    assert!(moved(&rule, "araçlar").top >= 800.0 - 0.5, "out of view: {:?}", moved(&rule, "araçlar"));
    swipe(&rule, Offset::new(200.0, 200.0), Offset::new(200.0, 400.0), 10);
    rule.wait_for_idle();
    assert!(same_rect(moved(&rule, "araçlar"), inner), "{:?} {inner:?}", moved(&rule, "araçlar"));
}

#[kompozable]
fn ExpandableVerticalFloatingToolbarSample() {
    let expanded = remember_saveable(|| mutable_state_of(true));
    Scaffold(
        content = move |inner_padding: PaddingValues| {
            let expanded = expanded.clone();
            let padding = inner_padding.clone();
            Box(
                modifier = Modifier.padding_values(inner_padding),
                content = move || {
                    // The toolbar should receive focus before the screen content for a11y, so place it first. Make sure to
                    // set its zIndex so it's above the screen content visually.
                    VerticalFloatingToolbar(
                        modifier = Modifier
                            .align(Alignment::CenterEnd)
                            .offset(-FloatingToolbarDefaults::ScreenOffset, 0.dp())
                            .z_index(1.0),
                        expanded = expanded.get(),
                        leading_content = slot(|| LeadingContent()),
                        trailing_content = slot(|| TrailingContent()),
                        content = || {
                            Tooltipped(|| {
                                FilledIconButton(
                                    modifier = Modifier.height(64.dp()),
                                    on_click = || {},
                                    content = || SampleIcon(Icons::Filled::Add()),
                                )
                            })
                        },
                    );
                    let (e1, e2) = (expanded.clone(), expanded.clone());
                    LazyColumn(
                        modifier = Modifier.floating_toolbar_vertical_nested_scroll(
                            expanded = expanded.get(),
                            on_expand = move || e1.set(true),
                            on_collapse = move || e2.set(false),
                        ),
                        state = remember_lazy_list_state(),
                        content_padding = padding.clone(),
                        vertical_arrangement = Arrangement::spaced_by(8.dp()),
                        content = sample_rows,
                    );
                },
            );
        },
    );
}

fn expandable_vertical_floating_toolbar_sample() {
    let rule = KompozTestRule::new(ExpandableVerticalFloatingToolbarSample);
    assert_eq!(shown_icons(&rule), 5);
    swipe(&rule, Offset::new(150.0, 400.0), Offset::new(150.0, 300.0), 10);
    rule.wait_for_idle();
    assert_eq!(shown_icons(&rule), 1);
    swipe(&rule, Offset::new(150.0, 300.0), Offset::new(150.0, 400.0), 10);
    rule.wait_for_idle();
    assert_eq!(shown_icons(&rule), 5);
}

#[kompozable]
fn OverflowingVerticalFloatingToolbarSample() {
    Scaffold(
        content = move |inner_padding: PaddingValues| {
            let padding = inner_padding.clone();
            Box(
                modifier = Modifier.padding_values(inner_padding),
                content = move || {
                    VerticalFloatingToolbar(
                        modifier = Modifier
                            .align(Alignment::CenterEnd)
                            .offset(-FloatingToolbarDefaults::ScreenOffset, 0.dp())
                            .z_index(1.0),
                        expanded = true,
                        leading_content = slot(|| LeadingContent()),
                        trailing_content = slot(|| {
                            AppBarColumn(
                                content = |scope: &dyn AppBarColumnScope| {
                                    scope.clickable_item(
                                        || {},
                                        || SampleIcon(Icons::Filled::Download()),
                                        "Download".into(),
                                    );
                                    scope.clickable_item(
                                        || {},
                                        || SampleIcon(Icons::Filled::Favorite()),
                                        "Favorite".into(),
                                    );
                                    scope.clickable_item(|| {}, || SampleIcon(Icons::Filled::Add()), "Add".into());
                                    scope.clickable_item(
                                        || {},
                                        || SampleIcon(Icons::Filled::Person()),
                                        "Person".into(),
                                    );
                                    scope.clickable_item(
                                        || {},
                                        || SampleIcon(Icons::Filled::ArrowUpward()),
                                        "ArrowUpward".into(),
                                    );
                                },
                            )
                        }),
                        content = || {
                            Tooltipped(|| {
                                FilledIconButton(
                                    modifier = Modifier.height(64.dp()),
                                    on_click = || {},
                                    content = || SampleIcon(Icons::Filled::Add()),
                                )
                            })
                        },
                    );
                    LazyColumn(
                        state = remember_lazy_list_state(),
                        content_padding = padding.clone(),
                        vertical_arrangement = Arrangement::spaced_by(8.dp()),
                        content = sample_rows,
                    );
                },
            );
        },
    );
}

fn overflowing_vertical_floating_toolbar_sample() {
    // 400 dp tall: the column's five 48 dp items don't all fit under the leading content and the content.
    let rule = KompozTestRule::with_viewport(OverflowingVerticalFloatingToolbarSample, IntSize::new(400, 400));
    rule.on_node_with_content_description("More options").assert_exists();
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("ArrowUpward").assert_exists();
}

#[kompozable]
fn ScrollableVerticalFloatingToolbarSample() {
    let exit_always_scroll_behavior =
        FloatingToolbarDefaults.exit_always_scroll_behavior(exit_direction = FloatingToolbarExitDirection::End);
    let behavior = exit_always_scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(exit_always_scroll_behavior.clone()),
        content = move |inner_padding: PaddingValues| {
            let padding = inner_padding.clone();
            let behavior = behavior.clone();
            Box(
                modifier = Modifier.padding_values(inner_padding),
                content = move || {
                    VerticalFloatingToolbar(
                        modifier = Modifier
                            .align(Alignment::CenterEnd)
                            .offset(-FloatingToolbarDefaults::ScreenOffset, 0.dp())
                            .z_index(1.0)
                            .test_tag("araçlar"),
                        expanded = true,
                        leading_content = slot(|| LeadingContent()),
                        trailing_content = slot(|| TrailingContent()),
                        content = || {
                            Tooltipped(|| {
                                FilledIconButton(
                                    modifier = Modifier.height(64.dp()),
                                    on_click = || {},
                                    content = || SampleIcon(Icons::Filled::Add()),
                                )
                            })
                        },
                        scroll_behavior = Some(behavior.clone()),
                    );
                    LazyColumn(
                        state = remember_lazy_list_state(),
                        content_padding = padding.clone(),
                        vertical_arrangement = Arrangement::spaced_by(8.dp()),
                        content = sample_rows,
                    );
                },
            );
        },
    );
}

fn scrollable_vertical_floating_toolbar_sample() {
    let rule = KompozTestRule::new(ScrollableVerticalFloatingToolbarSample);
    let shown = bounds(&rule, "araçlar");
    assert!(near(shown.right, 400.0 - 16.0), "{shown:?}");
    let inner = moved(&rule, "araçlar");
    swipe(&rule, Offset::new(150.0, 400.0), Offset::new(150.0, 200.0), 10);
    rule.wait_for_idle();
    assert!(moved(&rule, "araçlar").left >= 400.0 - 0.5, "out of the end: {:?}", moved(&rule, "araçlar"));
    swipe(&rule, Offset::new(150.0, 200.0), Offset::new(150.0, 400.0), 10);
    rule.wait_for_idle();
    assert!(same_rect(moved(&rule, "araçlar"), inner), "{:?} {inner:?}", moved(&rule, "araçlar"));
}

/// The FAB samples' four tooltipped icon buttons, not focusable while collapsed.
#[kompozable]
fn FabSampleButtons(expanded: bool) {
    for icon in [Icons::Filled::Person(), Icons::Filled::Edit(), Icons::Filled::Favorite(), Icons::Filled::MoreVert()] {
        Tooltipped(move || {
            let icon = icon.clone();
            IconButton(
                || {},
                Modifier.focus_properties(move |p| p.can_focus = expanded),
                content = move || SampleIcon(icon.clone()),
            )
        });
    }
}

/// The FAB samples' Column of a Lorem Ipsum.
#[kompozable]
fn LoremColumn(modifier: Modifier) {
    Column(
        modifier = modifier.vertical_scroll(&remember_scroll_state()),
        content = || {
            Text(text = remember(|| LoremIpsum().values()[0].clone()));
        },
    );
}

#[kompozable]
fn HorizontalFloatingToolbarWithFabSample() {
    let expanded = remember_saveable(|| mutable_state_of(true));
    let vibrant_colors = FloatingToolbarDefaults.vibrant_floating_toolbar_colors();
    Scaffold(
        content = move |inner_padding: PaddingValues| {
            let expanded = expanded.clone();
            Box(
                modifier = Modifier.padding_values(inner_padding),
                content = move || {
                    let toggle = expanded.clone();
                    let e = expanded.get();
                    HorizontalFloatingToolbarWithFab(
                        expanded = e,
                        floating_action_button = move || {
                            let toggle = toggle.clone();
                            Tooltipped(move || {
                                let toggle = toggle.clone();
                                // Match the FAB to the vibrantColors. See also StandardFloatingActionButton.
                                FloatingToolbarDefaults.VibrantFloatingActionButton(
                                    on_click = move || toggle.set(!toggle.get()),
                                    modifier = Modifier.test_tag("fab"),
                                    content = || SampleIcon(Icons::Filled::Add()),
                                )
                            })
                        },
                        modifier = Modifier
                            .align(Alignment::BottomEnd)
                            .offset(-FloatingToolbarDefaults::ScreenOffset, -FloatingToolbarDefaults::ScreenOffset)
                            .z_index(1.0)
                            .test_tag("araçlar"),
                        colors = vibrant_colors,
                        content = move || FabSampleButtons(e),
                    );
                    let (e1, e2) = (expanded.clone(), expanded.clone());
                    LoremColumn(
                        Modifier
                            .fill_max_width()
                            .padding_hv(16.dp(), 0.dp())
                            // Apply a floatingToolbarVerticalNestedScroll Modifier to the Column to toggle the expanded state
                            // of the HorizontalFloatingToolbar.
                            .floating_toolbar_vertical_nested_scroll(
                                expanded = expanded.get(),
                                on_expand = move || e1.set(true),
                                on_collapse = move || e2.set(false),
                            ),
                    );
                },
            );
        },
    );
}

fn horizontal_floating_toolbar_with_fab_sample() {
    let rule = KompozTestRule::new(HorizontalFloatingToolbarWithFabSample);
    let cs = light_color_scheme();
    let bar = bounds(&rule, "araçlar");
    // Bottom end, 16 dp in: the toolbar (8 + 4 × 48 + 8), 8 dp, the 56 dp FAB.
    assert!(near(bar.right, 400.0 - 16.0) && near(bar.bottom, 800.0 - 16.0) && near(bar.width(), 208.0 + 8.0 + 56.0));
    assert_eq!(fill(&rule, bounds(&rule, "fab")).map(|f| f.1), Some(cs.tertiary_container));
    rule.on_node_with_tag("fab").perform_click();
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").width(), 80.0)); // collapsed
    rule.on_node_with_tag("fab").perform_click();
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").width(), 56.0));
    // The Lorem Ipsum scrolled forwards collapses it.
    swipe(&rule, Offset::new(100.0, 400.0), Offset::new(100.0, 300.0), 10);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").width(), 80.0));
}

#[kompozable]
fn CenteredHorizontalFloatingToolbarWithFabSample() {
    let exit_always_scroll_behavior =
        FloatingToolbarDefaults.exit_always_scroll_behavior(exit_direction = FloatingToolbarExitDirection::Bottom);
    let vibrant_colors = FloatingToolbarDefaults.vibrant_floating_toolbar_colors();
    let behavior = exit_always_scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(exit_always_scroll_behavior.clone()),
        content = move |inner_padding: PaddingValues| {
            let behavior = behavior.clone();
            Box(
                modifier = Modifier.padding_values(inner_padding),
                content = move || {
                    HorizontalFloatingToolbarWithFab(
                        // Always expanded as the toolbar is bottom-centered. We will use a FloatingToolbarScrollBehavior
                        // to hide both the toolbar and its FAB on scroll.
                        expanded = true,
                        floating_action_button = || {
                            Tooltipped(|| {
                                FloatingToolbarDefaults.VibrantFloatingActionButton(
                                    on_click = || {},
                                    content = || SampleIcon(Icons::Filled::Add()),
                                )
                            })
                        },
                        modifier = Modifier
                            .align(Alignment::BottomCenter)
                            .offset(0.dp(), -FloatingToolbarDefaults::ScreenOffset)
                            .z_index(1.0)
                            .test_tag("araçlar"),
                        colors = vibrant_colors,
                        scroll_behavior = Some(behavior.clone()),
                        content = || {
                            Tooltipped(|| IconButton(|| {}, content = || SampleIcon(Icons::Filled::Person())));
                            Tooltipped(|| IconButton(|| {}, content = || SampleIcon(Icons::Filled::Edit())));
                            Tooltipped(|| IconButton(|| {}, content = || SampleIcon(Icons::Filled::Favorite())));
                            Tooltipped(|| IconButton(|| {}, content = || SampleIcon(Icons::Filled::MoreVert())));
                        },
                    );
                    LoremColumn(Modifier.fill_max_width().padding_hv(16.dp(), 0.dp()));
                },
            );
        },
    );
}

fn centered_horizontal_floating_toolbar_with_fab_sample() {
    let rule = KompozTestRule::new(CenteredHorizontalFloatingToolbarWithFabSample);
    let shown = bounds(&rule, "araçlar");
    assert!(near(shown.center().x, 200.0) && near(shown.bottom, 800.0 - 16.0), "{shown:?}");
    swipe(&rule, Offset::new(100.0, 400.0), Offset::new(100.0, 200.0), 10);
    rule.wait_for_idle();
    assert!(moved(&rule, "araçlar").top >= 800.0 - 0.5, "{:?}", moved(&rule, "araçlar"));
}

#[kompozable]
fn VerticalFloatingToolbarWithFabSample() {
    let expanded = remember_saveable(|| mutable_state_of(true));
    let vibrant_colors = FloatingToolbarDefaults.vibrant_floating_toolbar_colors();
    Scaffold(
        content = move |inner_padding: PaddingValues| {
            let expanded = expanded.clone();
            Box(
                modifier = Modifier.padding_values(inner_padding),
                content = move || {
                    let toggle = expanded.clone();
                    let e = expanded.get();
                    VerticalFloatingToolbarWithFab(
                        expanded = e,
                        floating_action_button = move || {
                            let toggle = toggle.clone();
                            Tooltipped(move || {
                                let toggle = toggle.clone();
                                FloatingToolbarDefaults.VibrantFloatingActionButton(
                                    on_click = move || toggle.set(!toggle.get()),
                                    modifier = Modifier.test_tag("fab"),
                                    content = || SampleIcon(Icons::Filled::Add()),
                                )
                            })
                        },
                        modifier = Modifier
                            .align(Alignment::BottomEnd)
                            .offset(-FloatingToolbarDefaults::ScreenOffset, -FloatingToolbarDefaults::ScreenOffset)
                            .z_index(1.0)
                            .test_tag("araçlar"),
                        colors = vibrant_colors,
                        content = move || FabSampleButtons(e),
                    );
                    let (e1, e2) = (expanded.clone(), expanded.clone());
                    LoremColumn(Modifier.fill_max_width().padding_hv(16.dp(), 0.dp()).then(
                        Modifier.floating_toolbar_vertical_nested_scroll(
                            expanded = expanded.get(),
                            on_expand = move || e1.set(true),
                            on_collapse = move || e2.set(false),
                        ),
                    ));
                },
            );
        },
    );
}

fn vertical_floating_toolbar_with_fab_sample() {
    let rule = KompozTestRule::new(VerticalFloatingToolbarWithFabSample);
    let bar = bounds(&rule, "araçlar");
    assert!(near(bar.bottom, 800.0 - 16.0) && near(bar.height(), 208.0 + 8.0 + 56.0), "{bar:?}");
    rule.on_node_with_tag("fab").perform_click();
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").height(), 80.0));
    swipe(&rule, Offset::new(100.0, 300.0), Offset::new(100.0, 400.0), 10); // back: expands
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").height(), 80.0), "nothing to scroll back to: stays");
    spring_back(&rule);
    swipe(&rule, Offset::new(100.0, 400.0), Offset::new(100.0, 300.0), 10);
    swipe(&rule, Offset::new(100.0, 300.0), Offset::new(100.0, 400.0), 10);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").height(), 56.0), "{:?}", bounds(&rule, "fab"));
}

#[kompozable]
fn CenteredVerticalFloatingToolbarWithFabSample() {
    let exit_always_scroll_behavior =
        FloatingToolbarDefaults.exit_always_scroll_behavior(exit_direction = FloatingToolbarExitDirection::End);
    let vibrant_colors = FloatingToolbarDefaults.vibrant_floating_toolbar_colors();
    let behavior = exit_always_scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(exit_always_scroll_behavior.clone()),
        content = move |inner_padding: PaddingValues| {
            let behavior = behavior.clone();
            Box(
                modifier = Modifier.padding_values(inner_padding),
                content = move || {
                    VerticalFloatingToolbarWithFab(
                        // Always expanded as the toolbar is right-centered. We will use a FloatingToolbarScrollBehavior to
                        // hide both the toolbar and its FAB on scroll.
                        expanded = true,
                        floating_action_button = || {
                            Tooltipped(|| {
                                FloatingToolbarDefaults.VibrantFloatingActionButton(
                                    on_click = || {},
                                    content = || SampleIcon(Icons::Filled::Add()),
                                )
                            })
                        },
                        modifier = Modifier
                            .align(Alignment::CenterEnd)
                            .offset(-FloatingToolbarDefaults::ScreenOffset, 0.dp())
                            .z_index(1.0)
                            .test_tag("araçlar"),
                        colors = vibrant_colors,
                        scroll_behavior = Some(behavior.clone()),
                        content = || {
                            Tooltipped(|| IconButton(|| {}, content = || SampleIcon(Icons::Filled::Person())));
                            Tooltipped(|| IconButton(|| {}, content = || SampleIcon(Icons::Filled::Edit())));
                            IconButton(|| {}, content = || SampleIcon(Icons::Filled::Favorite()));
                            Tooltipped(|| IconButton(|| {}, content = || SampleIcon(Icons::Filled::MoreVert())));
                        },
                    );
                    LoremColumn(Modifier.fill_max_width().padding_hv(16.dp(), 0.dp()));
                },
            );
        },
    );
}

fn centered_vertical_floating_toolbar_with_fab_sample() {
    let rule = KompozTestRule::new(CenteredVerticalFloatingToolbarWithFabSample);
    let shown = bounds(&rule, "araçlar");
    assert!(near(shown.center().y, 400.0) && near(shown.right, 400.0 - 16.0), "{shown:?}");
    swipe(&rule, Offset::new(100.0, 400.0), Offset::new(100.0, 200.0), 10);
    rule.wait_for_idle();
    assert!(moved(&rule, "araçlar").left >= 400.0 - 0.5, "{:?}", moved(&rule, "araçlar"));
}

#[kompozable]
fn HorizontalFloatingToolbarAsScaffoldFabSample() {
    let expanded = remember_saveable(|| mutable_state_of(true));
    let vibrant_colors = FloatingToolbarDefaults.vibrant_floating_toolbar_colors();
    let (fab_expanded, content_expanded) = (expanded.clone(), expanded.clone());
    Scaffold(
        floating_action_button = move || {
            let toggle = fab_expanded.clone();
            let e = fab_expanded.get();
            HorizontalFloatingToolbarWithFab(
                expanded = e,
                floating_action_button = move || {
                    let toggle = toggle.clone();
                    Tooltipped(move || {
                        let toggle = toggle.clone();
                        FloatingToolbarDefaults.VibrantFloatingActionButton(
                            on_click = move || toggle.set(!toggle.get()),
                            modifier = Modifier.test_tag("fab"),
                            content = || SampleIcon(Icons::Filled::Add()),
                        )
                    })
                },
                modifier = Modifier.test_tag("araçlar"),
                colors = vibrant_colors,
                content = move || FabSampleButtons(e),
            );
        },
        // When setting this to `FabPosition.Start` remember to set a
        // `floatingActionButtonPosition = FloatingToolbarHorizontalFabPosition.Start` at the
        // HorizontalFloatingToolbar as well.
        floating_action_button_position = FabPosition::End,
        content = move |inner_padding: PaddingValues| {
            let expanded = content_expanded.clone();
            Box(
                modifier = Modifier.padding_values(inner_padding),
                content = move || {
                    let (e1, e2) = (expanded.clone(), expanded.clone());
                    LoremColumn(Modifier.fill_max_width().padding_hv(16.dp(), 0.dp()).then(
                        Modifier.floating_toolbar_vertical_nested_scroll(
                            expanded = expanded.get(),
                            on_expand = move || e1.set(true),
                            on_collapse = move || e2.set(false),
                        ),
                    ));
                },
            );
        },
    );
}

fn horizontal_floating_toolbar_as_scaffold_fab_sample() {
    let rule = KompozTestRule::new(HorizontalFloatingToolbarAsScaffoldFabSample);
    // Where the Scaffold puts its FAB: 16 dp from the bottom end.
    let bar = bounds(&rule, "araçlar");
    assert!(near(bar.right, 400.0 - 16.0) && near(bar.bottom, 800.0 - 16.0), "{bar:?}");
    rule.on_node_with_tag("fab").perform_click();
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "fab").width(), 80.0));
    assert!(same_rect(bounds(&rule, "araçlar"), bar), "the layout keeps its room");
}

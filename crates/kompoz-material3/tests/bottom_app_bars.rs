//! Material's bottom app bars, AppBarRow and AppBarColumn as Compose's material3 (1.5.0-alpha17) has them: the C++
//! Kompoz's tests by name (kompoz_tests.cpp; M15's values and API where the C++ differed, said at each), Compose's
//! behaviour (the state, its coercion and saver; the exit-always behavior's connection and remembering; the bar's
//! drags and settling; the flexible bar's height; the defaults; the items, their menu and the overflow indicator; the
//! block run again by what it reads), and the bottom-bar samples of M15's AppBarSamples.kt. Density 1: dp are px; the
//! viewport is 400 × 800 unless said.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::layout::{PaddingValues, WindowInsets};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas, has_text};
use kompoz_ui::input::{NestedScrollSource, PointerType};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    // (the C++ Kompoz's)
    bottom_app_bar_as_compose_1_4,
    app_bar_rows_put_what_does_not_fit_in_a_menu,
    app_bar_columns_put_what_does_not_fit_in_a_menu_below,
    bottom_app_bars_spread_out_and_leave_as_the_content_scrolls_on,
    // (Compose's behaviour)
    bottom_app_bar_state_keeps_its_offset_in_its_limit_and_is_saved,
    exit_always_moves_the_bar_with_what_the_content_scrolls,
    dragging_a_bottom_app_bar_moves_it_and_it_settles,
    flexible_bottom_app_bar_takes_its_height_or_the_default,
    bottom_app_bar_defaults_as_compose,
    app_bar_items_of_their_own_disabled_items_and_overflow_indicators,
    app_bar_rows_run_their_block_again_when_what_it_reads_changes,
    // (M15's samples)
    simple_bottom_app_bar,
    bottom_app_bar_with_fab,
    exit_always_bottom_app_bar,
    exit_always_bottom_app_bar_spaced_around,
    exit_always_bottom_app_bar_spaced_between,
    exit_always_bottom_app_bar_spaced_evenly,
    exit_always_bottom_app_bar_fixed,
    exit_always_bottom_app_bar_fixed_vibrant,
    bottom_app_bar_with_overflow,
);

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A text's own bounds (the unmerged tree's).
fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with(has_text(text, false, false), true).get_unclipped_bounds_in_root()
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left) && near(a.top, b.top) && near(a.right, b.right) && near(a.bottom, b.bottom)
}

type Slot<T> = Rc<RefCell<Option<T>>>;

fn slot<T>() -> Slot<T> {
    Rc::new(RefCell::new(None))
}

fn get<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
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

/// Whether `icon` was drawn.
fn drawn(canvas: &RecordingCanvas, icon: &ImageVector) -> bool {
    canvas.vectors.contains(&icon.id())
}

/// Whether a vector other than `icons` was drawn: the overflow indicator's ⋮ (material3's own copy of the MoreVert
/// icon, as Compose's internal Icons: not the icons crate's).
fn drawn_other_than(canvas: &RecordingCanvas, icons: &[ImageVector]) -> bool {
    canvas.vectors.iter().any(|v| icons.iter().all(|i| i.id() != *v))
}

// ---- The C++ Kompoz's tests ----------------------------------------------------------------------------------------

/// The classic bottom app bar: 80 dp, its actions 4 dp in at the start and centered below the 4 dp at the top, the
/// FAB 12 dp down and 16 dp in from the end (M15's numbers are 1.4's).
fn bottom_app_bar_as_compose_1_4() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Box(
                modifier = Modifier.fill_max_size(),
                content_alignment = Alignment::BottomStart,
                content = || {
                    BottomAppBar(
                        actions = || Box(modifier = Modifier.size(48.dp()).test_tag("eylem")),
                        floating_action_button =
                            Some(Rc::new(|| Box(modifier = Modifier.size(56.dp()).test_tag("fab")))),
                        window_insets = WindowInsets(),
                        modifier = Modifier.test_tag("çubuk"),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let (bar, action, fab) = (bounds(&rule, "çubuk"), bounds(&rule, "eylem"), bounds(&rule, "fab"));
    // 4 dp in at the start and the top: the actions centered below that; the FAB 12 dp down, 16 from the end.
    assert!(near(bar.height(), 80.0) && near(action.left, bar.left + 4.0), "{bar:?} {action:?}");
    assert!(near(action.center().y, bar.top + 4.0 + 38.0), "{bar:?} {action:?}");
    assert!(near(fab.top, bar.top + 12.0) && near(fab.right, bar.right - 16.0), "{bar:?} {fab:?}");
}

/// An AppBarRow puts in what fits, keeping room for its ⋮ (which the last item may take), the rest in the ⋮'s menu;
/// its items run their actions from the menu and close it; `max_item_count` slots at most.
fn app_bar_rows_put_what_does_not_fit_in_a_menu() {
    let width = mutable_state_of(200.0f32);
    let max_items = mutable_state_of(i32::MAX);
    let clicked: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let starred = Rc::new(Cell::new(false));
    let names = ["Bir", "İki", "Üç", "Dört", "Beş"];
    let icons: Vec<ImageVector> = vec![
        Icons::Filled::Add(),
        Icons::Filled::Edit(),
        Icons::Filled::Share(),
        Icons::Filled::Delete(),
        Icons::Filled::Star(),
    ];
    let rule = {
        let (width, max_items, clicked, starred, icons) =
            (width.clone(), max_items.clone(), clicked.clone(), starred.clone(), icons.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (clicked, starred, icons) = (clicked.clone(), starred.clone(), icons.clone());
                let max_item_count = max_items.get();
                Box(
                    modifier = Modifier.width(width.get().dp()),
                    content = move || {
                        let (clicked, starred, icons) = (clicked.clone(), starred.clone(), icons.clone());
                        AppBarRow(
                            modifier = Modifier.test_tag("satır"),
                            max_item_count = max_item_count,
                            content = move |s: &dyn AppBarRowScope| {
                                for i in 0..4 {
                                    let (clicked, icon) = (clicked.clone(), icons[i].clone());
                                    s.clickable_item(
                                        move || clicked.borrow_mut().push(names[i].to_string()),
                                        move || {
                                            VectorIcon(icon.clone(), content_description = Some(names[i].to_string()))
                                        },
                                        format!("{} öğesi", names[i]), // the menu's label
                                    );
                                }
                                let (set, star) = (starred.clone(), icons[4].clone());
                                s.toggleable_item(
                                    starred.get(),
                                    move |v| set.set(v),
                                    move || VectorIcon(star.clone(), content_description = Some("Beş".to_string())),
                                    "Beş öğesi".to_string(),
                                );
                            },
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let shown = |name: &str| {
        // drawn in the row (the ones left out are composed, not placed)
        let canvas = rule.draw();
        match names.iter().position(|n| *n == name) {
            Some(i) => drawn(&canvas, &icons[i]),
            None => drawn_other_than(&canvas, &icons),
        }
    };
    // 200 dp: room kept for the ⋮ (48): three of them, the rest in its menu.
    assert!(shown("Bir") && shown("Üç") && !shown("Dört") && !shown("Beş") && shown("More options"));
    assert!(near(bounds(&rule, "satır").width(), 4.0 * 48.0));
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Dört öğesi").assert_exists();
    rule.on_node_with_text("Beş öğesi").assert_exists();
    rule.on_node_with_text("Üç öğesi").assert_does_not_exist();
    rule.on_node_with_text("Beş öğesi").perform_click(); // a toggle from the menu: flipped, the menu gone
    rule.wait_for_idle();
    assert!(starred.get());
    rule.on_node_with_text("Dört öğesi").assert_does_not_exist();
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Dört öğesi").perform_click();
    rule.wait_for_idle();
    assert_eq!(*clicked.borrow(), ["Dört"]);
    rule.on_node_with_text("Beş öğesi").assert_does_not_exist(); // and the menu closed
    width.set(240.0); // four fit, and the last one takes the button's room: no menu
    rule.wait_for_idle();
    assert!(shown("Beş") && !shown("More options") && near(bounds(&rule, "satır").width(), 5.0 * 48.0));
    max_items.set(3); // three slots: two, and the button
    rule.wait_for_idle();
    assert!(shown("İki") && !shown("Üç") && shown("More options"));
}

/// An AppBarColumn does the same downwards, its ⋮ at the bottom.
fn app_bar_columns_put_what_does_not_fit_in_a_menu_below() {
    let clicked = Rc::new(Cell::new(-1));
    let rule = {
        let clicked = clicked.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let clicked = clicked.clone();
                Box(
                    modifier = Modifier.height(150.dp()),
                    content = move || {
                        let clicked = clicked.clone();
                        AppBarColumn(
                            modifier = Modifier.test_tag("sütun"),
                            content = move |s: &dyn AppBarColumnScope| {
                                let items: [(ImageVector, &str); 4] = [
                                    (Icons::Filled::Add(), "Ekle"),
                                    (Icons::Filled::Edit(), "Düzenle"),
                                    (Icons::Filled::Share(), "Paylaş"),
                                    (Icons::Filled::Delete(), "Sil"),
                                ];
                                for (i, (icon, name)) in items.into_iter().enumerate() {
                                    let clicked = clicked.clone();
                                    s.clickable_item(
                                        move || clicked.set(i as i32),
                                        move || VectorIcon(icon.clone(), content_description = Some(name.to_string())),
                                        format!("{name} öğesi"),
                                    );
                                }
                            },
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    // 150 dp tall: 48 kept for the ⋮, two fit above it.
    let column = bounds(&rule, "sütun");
    assert!(near(column.height(), 3.0 * 48.0) && near(column.width(), 48.0), "{column:?}");
    let canvas = rule.draw();
    assert!(drawn(&canvas, &Icons::Filled::Add()) && drawn(&canvas, &Icons::Filled::Edit()));
    assert!(!drawn(&canvas, &Icons::Filled::Share()));
    assert!(drawn_other_than(
        &canvas,
        &[Icons::Filled::Add(), Icons::Filled::Edit(), Icons::Filled::Share(), Icons::Filled::Delete()]
    ));
    // At the bottom. (The node described is the icon button's, 40 dp in its 48 dp room: 4 dp down; the C++ measured
    // its 24 dp icon, 12 dp down.)
    let indicator = rule.on_node_with_content_description("More options").get_unclipped_bounds_in_root();
    assert!(near(indicator.top, column.top + 2.0 * 48.0 + 4.0), "{indicator:?}");
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Paylaş öğesi").assert_exists();
    rule.on_node_with_text("Sil öğesi").assert_exists();
    rule.on_node_with_text("Düzenle öğesi").assert_does_not_exist();
    rule.on_node_with_text("Sil öğesi").perform_click();
    assert_eq!(clicked.get(), 3);
}

/// The flexible bar spreads its content out across its 64 dp, 16 dp in from the sides; an exit-always behavior takes
/// it away as far as the content scrolls on, all of it past its height, and back; held a little over half way back
/// and let go, it settles all the way back. The classic bar leaves the same way, 80 dp of it. (The C++'s state told
/// positive offsets; M15's height offset and limit are negative.)
fn bottom_app_bars_spread_out_and_leave_as_the_content_scrolls_on() {
    let flexible = mutable_state_of(true);
    let behavior: Slot<Rc<dyn BottomAppBarScrollBehavior>> = slot();
    let rule = {
        let (flexible, behavior) = (flexible.clone(), behavior.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let kept = BottomAppBarDefaults.exit_always_scroll_behavior();
                *behavior.borrow_mut() = Some(kept.clone());
                let (flexible, bar_behavior) = (flexible.clone(), kept.clone());
                Scaffold(
                    modifier = Modifier.fill_max_size().nested_scroll(kept.nested_scroll_connection(), None),
                    bottom_bar = move || {
                        if flexible.get() {
                            FlexibleBottomAppBar(
                                modifier = Modifier.test_tag("çubuk"),
                                scroll_behavior = Some(bar_behavior.clone()),
                                window_insets = WindowInsets(),
                                content = || {
                                    for tag in ["a", "b", "c"] {
                                        Box(modifier = Modifier.size(48.dp()).test_tag(tag));
                                    }
                                },
                            );
                        } else {
                            BottomAppBar(
                                actions = || Box(modifier = Modifier.size(48.dp()).test_tag("a")),
                                window_insets = WindowInsets(),
                                scroll_behavior = Some(bar_behavior.clone()),
                                modifier = Modifier.test_tag("çubuk"),
                            );
                        }
                    },
                    content = |padding: PaddingValues| {
                        LazyColumn(
                            modifier = Modifier.fill_max_size(),
                            content_padding = padding,
                            content = |this: &mut LazyListScope| {
                                this.items(60, |_| Box(modifier = Modifier.fill_max_width().height(60.dp())));
                            },
                        );
                    },
                );
            }),
            IntSize::new(400, 600),
        )
    };
    let state = || get(&behavior).state();
    // Spread out across its 64 dp, 16 dp in from the sides.
    assert!(same_rect(bounds(&rule, "çubuk"), Rect::new(0.0, 536.0, 400.0, 600.0)), "{:?}", bounds(&rule, "çubuk"));
    assert!(near(bounds(&rule, "a").left, 16.0) && near(bounds(&rule, "b").left, 176.0));
    assert!(near(bounds(&rule, "c").left, 336.0));
    assert!(near(state().height_offset_limit(), -64.0));
    swipe(&rule, at(200.0, 300.0), at(200.0, 270.0), 10); // forwards: it slides down as far
    rule.advance_frame();
    let went = -state().height_offset();
    assert!(went > 20.0 && went < 30.5, "{went}");
    assert!(near(bounds(&rule, "çubuk").height(), 64.0 - went) && near(bounds(&rule, "a").top, 536.0 + went + 8.0));
    swipe(&rule, at(200.0, 400.0), at(200.0, 250.0), 10); // past it: gone
    rule.wait_for_idle();
    assert!(near(state().height_offset(), -64.0) && near(bounds(&rule, "çubuk").height(), 0.0));
    let h = rule.host(); // back a little over half way, held, let go: all the way back
    h.pointer_down(at(200.0, 300.0), -1, 0, PointerType::Touch);
    for i in 1..=8 {
        rule.advance_frame();
        h.pointer_move(at(200.0, 300.0 + 6.5 * i as f32), -1, 0);
    }
    for _ in 0..10 {
        rule.advance_frame();
        h.pointer_move(at(200.0, 352.0), -1, 0);
    }
    let halfway = -state().height_offset();
    h.pointer_up(at(200.0, 352.0), -1, 0);
    rule.wait_for_idle();
    assert!(halfway > 5.0 && halfway < 32.0, "{halfway}");
    assert!(near(state().height_offset(), 0.0) && near(bounds(&rule, "çubuk").height(), 64.0));

    flexible.set(false); // the classic one leaves the same way, 80 dp of it
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "çubuk").height(), 80.0) && near(state().height_offset_limit(), -80.0));
    swipe(&rule, at(200.0, 400.0), at(200.0, 250.0), 10);
    rule.wait_for_idle();
    assert!(near(state().height_offset(), -80.0) && near(bounds(&rule, "çubuk").height(), 0.0));
}

// ---- Compose's behaviour -------------------------------------------------------------------------------------------

/// The state keeps its height offset between its limit and 0, its collapsed fraction the offset over the limit (0
/// with a limit of 0); its saver writes the limit and the offsets and reads them back; remembered, it starts at
/// Compose's defaults.
fn bottom_app_bar_state_keeps_its_offset_in_its_limit_and_is_saved() {
    let state = BottomAppBarState(-100.0, -20.0, 5.0);
    assert_eq!((state.height_offset_limit(), state.height_offset(), state.content_offset()), (-100.0, -20.0, 5.0));
    assert!(near(state.collapsed_fraction(), 0.2));
    state.set_height_offset(-150.0);
    assert_eq!(state.height_offset(), -100.0);
    assert_eq!(state.collapsed_fraction(), 1.0);
    state.set_height_offset(10.0);
    assert_eq!(state.height_offset(), 0.0);
    state.set_height_offset(-50.0);
    let saver = <dyn BottomAppBarState>::Saver();
    let restored = saver.restore(&saver.save(&state)).expect("restored");
    assert_eq!(
        (restored.height_offset_limit(), restored.height_offset(), restored.content_offset()),
        (-100.0, -50.0, 5.0)
    );
    state.set_height_offset_limit(0.0);
    assert_eq!(state.collapsed_fraction(), 0.0);
    assert!(format!("{:?}", &*restored).starts_with("BottomAppBarState(heightOffsetLimit=-100"));

    let remembered: Slot<Rc<dyn BottomAppBarState>> = slot();
    let _rule = {
        let remembered = remembered.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            *remembered.borrow_mut() = Some(remember_bottom_app_bar_state());
        }))
    };
    let s = get(&remembered);
    assert_eq!((s.height_offset_limit(), s.height_offset(), s.content_offset()), (-f32::MAX, 0.0, 0.0));
}

/// The exit-always behavior: never pinned, its specs Compose's defaults; what the content scrolls (consumed) moves
/// the bar and the content offset the same way, while `can_scroll` says so; remembered while its state, `can_scroll`
/// and specs stay (another state: another behavior). (Its fling's settling: the C++ test above.)
fn exit_always_moves_the_bar_with_what_the_content_scrolls() {
    let can = Rc::new(Cell::new(true));
    let other_state = mutable_state_of(false);
    let recompose = mutable_state_of(0);
    let behavior: Slot<Rc<dyn BottomAppBarScrollBehavior>> = slot();
    let rule = {
        let (can, other_state, recompose, behavior) =
            (can.clone(), other_state.clone(), recompose.clone(), behavior.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            recompose.get();
            let can = can.clone();
            let own = remember_bottom_app_bar_state();
            let other = remember(|| BottomAppBarState(-80.0, 0.0, 0.0));
            let state = if other_state.get() { other } else { own };
            let b = named_args!(
                BottomAppBarDefaults.exit_always_scroll_behavior(state = state, can_scroll = move || can.get())
            );
            *behavior.borrow_mut() = Some(b);
        }))
    };
    let first = get(&behavior);
    assert!(!first.is_pinned() && first.snap_animation_spec().is_some() && first.fling_animation_spec().is_some());
    first.state().set_height_offset_limit(-80.0);
    let connection = first.nested_scroll_connection();
    let source = NestedScrollSource::UserInput;
    assert_eq!(connection.on_post_scroll(at(0.0, -30.0), at(0.0, 0.0), source), Offset::Zero);
    assert_eq!((first.state().height_offset(), first.state().content_offset()), (-30.0, -30.0));
    can.set(false);
    connection.on_post_scroll(at(0.0, -30.0), at(0.0, 0.0), source);
    assert_eq!(first.state().height_offset(), -30.0);
    can.set(true);
    connection.on_post_scroll(at(0.0, 10.0), at(0.0, 0.0), source);
    assert_eq!((first.state().height_offset(), first.state().content_offset()), (-20.0, -20.0));
    // Composed again with the same state and arguments: the same behavior.
    recompose.set(1);
    rule.wait_for_idle();
    assert!(Rc::ptr_eq(&first, &get(&behavior)));
    // Another state: another behavior.
    other_state.set(true);
    rule.wait_for_idle();
    let second = get(&behavior);
    assert!(!Rc::ptr_eq(&first, &second) && second.state().height_offset_limit() == -80.0);
    let again = second.clone();
    assert!(*first != *second && *second == *again);
    assert!(format!("{:?}", &*second).contains("isPinned=false"));
}

/// The bar itself is dragged (its scroll behavior's offset moved by the drag); let go, it settles: back when less
/// than half gone, away when more.
fn dragging_a_bottom_app_bar_moves_it_and_it_settles() {
    let behavior: Slot<Rc<dyn BottomAppBarScrollBehavior>> = slot();
    let rule = {
        let behavior = behavior.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let kept = BottomAppBarDefaults.exit_always_scroll_behavior();
                *behavior.borrow_mut() = Some(kept.clone());
                Box(
                    modifier = Modifier.fill_max_size(),
                    content_alignment = Alignment::BottomStart,
                    content = move || {
                        BottomAppBarWithContent(
                            modifier = Modifier.test_tag("çubuk"),
                            window_insets = WindowInsets(),
                            scroll_behavior = Some(kept.clone()),
                            content = || Box(modifier = Modifier.size(48.dp())),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let state = || get(&behavior).state();
    // Two moves a frame: a move leaving the pointer where it was on the bar (the bar having moved as far since the
    // last one) isn't dispatched to it (Compose's HitPathTracker `hasPositionChanged`, its default), so one move a
    // frame as far as the bar went in it would be every other one lost — as in Compose.
    let slow = |from: f32, to: f32| {
        let h = rule.host();
        h.pointer_down(at(200.0, from), -1, 0, PointerType::Touch);
        for i in 1..=20 {
            if i % 2 == 1 {
                rule.advance_frame();
            }
            h.pointer_move(at(200.0, from + (to - from) * i as f32 / 20.0), -1, 0);
        }
        for _ in 0..10 {
            rule.advance_frame();
            h.pointer_move(at(200.0, to), -1, 0);
        }
        let moved = state().height_offset();
        h.pointer_up(at(200.0, to), -1, 0);
        rule.wait_for_idle();
        moved
    };
    let moved = slow(740.0, 765.0); // 25 dp down (after the touch slop: less)
    assert!(moved < -5.0 && moved > -25.5, "{moved}");
    assert!(near(state().height_offset(), 0.0) && near(bounds(&rule, "çubuk").height(), 80.0));
    let moved = slow(730.0, 790.0);
    assert!(moved < -40.0, "{moved}");
    assert!(near(state().height_offset(), -80.0) && near(bounds(&rule, "çubuk").height(), 0.0));
}

/// The flexible bar is `expanded_height` tall, the default 64 dp for one not finite, unspecified or not above 0; its
/// container the scheme's surface container, its content colored to go on it, no tonal elevation; spread out by
/// default; a fixed arrangement puts its content 32 dp apart in the middle.
fn flexible_bottom_app_bar_takes_its_height_or_the_default() {
    let height = mutable_state_of(Dp(100.0));
    let fixed = mutable_state_of(false);
    let content_color: Slot<Color> = slot();
    let rule = {
        let (height, fixed, content_color) = (height.clone(), fixed.clone(), content_color.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let content_color = content_color.clone();
                let arrangement = if fixed.get() {
                    BottomAppBarDefaults::FlexibleFixedHorizontalArrangement()
                } else {
                    BottomAppBarDefaults::FlexibleHorizontalArrangement
                };
                FlexibleBottomAppBar(
                    modifier = Modifier.test_tag("çubuk"),
                    expanded_height = height.get(),
                    horizontal_arrangement = arrangement,
                    window_insets = WindowInsets(),
                    content = move || {
                        *content_color.borrow_mut() = Some(LocalContentColor.current());
                        for tag in ["a", "b", "c"] {
                            Box(modifier = Modifier.size(48.dp()).test_tag(tag));
                        }
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let scheme = light_color_scheme();
    assert!(near(bounds(&rule, "çubuk").height(), 100.0));
    assert_eq!(get(&content_color), scheme.on_surface);
    let canvas = rule.draw();
    assert!(canvas.rect_colors.contains(&scheme.surface_container), "{:?}", canvas.rect_colors);
    for h in [Dp::Infinity, Dp::Unspecified, Dp(-5.0), Dp(0.0)] {
        height.set(h);
        rule.wait_for_idle();
        assert!(near(bounds(&rule, "çubuk").height(), 64.0), "{h:?}");
    }
    assert!(near(bounds(&rule, "a").left, 16.0) && near(bounds(&rule, "c").right, 384.0));
    fixed.set(true);
    rule.wait_for_idle();
    // 3 × 48 + 2 × 32 = 208 in the middle of 400.
    assert!(near(bounds(&rule, "a").left, 96.0) && near(bounds(&rule, "b").left, 176.0));
    assert!(near(bounds(&rule, "c").left, 256.0));
}

/// The defaults are Compose's: the paddings, elevation, height, colors; the bars keep clear of (and consume) the
/// system bars at the sides and the bottom by default.
fn bottom_app_bar_defaults_as_compose() {
    assert_eq!(BottomAppBarDefaults::ContainerElevation, Dp(0.0));
    assert_eq!(BottomAppBarDefaults::FlexibleBottomAppBarHeight, Dp(64.0));
    let padding = BottomAppBarDefaults::ContentPadding();
    assert_eq!(
        (
            padding.calculate_start_padding(LayoutDirection::Ltr),
            padding.calculate_top_padding(),
            padding.calculate_bottom_padding()
        ),
        (Dp(4.0), Dp(4.0), Dp(0.0))
    );
    let flexible = BottomAppBarDefaults::FlexibleContentPadding();
    assert_eq!(
        (flexible.calculate_start_padding(LayoutDirection::Ltr), flexible.calculate_top_padding()),
        (Dp(16.0), Dp(0.0))
    );
    let colors: Slot<(Color, Color)> = slot();
    let rule = {
        let colors = colors.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *colors.borrow_mut() =
                    Some((BottomAppBarDefaults.container_color(), BottomAppBarDefaults.bottom_app_bar_fab_color()));
                Box(
                    modifier = Modifier.fill_max_size(),
                    content_alignment = Alignment::BottomStart,
                    content = || {
                        BottomAppBar(
                            actions = || Box(modifier = Modifier.size(48.dp()).test_tag("eylem")),
                            modifier = Modifier.test_tag("çubuk"),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let scheme = light_color_scheme();
    assert_eq!(get(&colors), (scheme.surface_container, scheme.secondary_container));
    let insets = kompoz_ui::platform::SystemInsets {
        navigation_bars: kompoz_ui::platform::Insets { left: 10, top: 0, right: 0, bottom: 30 },
        ..Default::default()
    };
    rule.host().set_system_insets(insets);
    rule.wait_for_idle();
    let (bar, action) = (bounds(&rule, "çubuk"), bounds(&rule, "eylem"));
    assert!(near(bar.height(), 110.0) && near(action.left, 14.0), "{bar:?} {action:?}");
    assert!(near(action.top, bar.top + 4.0 + 14.0), "{bar:?} {action:?}");
}

/// Items of their own (in the bar, and in the menu given its state to close it), disabled items (taking no clicks
/// in the bar), an overflow indicator of one's own given the menu's state, the default one showing and closing the
/// menu, and the menu state's handle.
fn app_bar_items_of_their_own_disabled_items_and_overflow_indicators() {
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let own_indicator = mutable_state_of(false);
    let rule = {
        let (log, own_indicator) = (log.clone(), own_indicator.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let log = log.clone();
                let own = own_indicator.get();
                let content = Rc::new(move |s: &dyn AppBarRowScope| {
                    let (l1, l2) = (log.clone(), log.clone());
                    named_args!(s.clickable_item(
                        move || l1.borrow_mut().push("kapalı".into()),
                        || VectorIcon(Icons::Filled::Add(), content_description = Some("Kapalı".to_string())),
                        "Kapalı öğe".to_string(),
                        enabled = false,
                    ));
                    s.custom_item(
                        || Box(modifier = Modifier.size(48.dp()).test_tag("özel")),
                        move |state: AppBarMenuState| {
                            let l2 = l2.clone();
                            DropdownMenuItem(
                                text = || Text("Özel öğe"),
                                on_click = move || {
                                    l2.borrow_mut().push("özel".into());
                                    state.dismiss();
                                },
                            );
                        },
                    );
                    s.clickable_item(
                        || {},
                        || VectorIcon(Icons::Filled::Edit(), content_description = None),
                        "Düzenle".to_string(),
                    );
                });
                Box(
                    modifier = Modifier.width(100.dp()),
                    content = move || {
                        let content = content.clone();
                        if own {
                            AppBarRow(
                                overflow_indicator = |state: AppBarMenuState| {
                                    let shown = state.is_showing();
                                    Text(
                                        if shown { "açık" } else { "kapalı" },
                                        modifier = Modifier.size(48.dp()).clickable(move || state.show()),
                                    );
                                },
                                content = move |s: &dyn AppBarRowScope| content(s),
                            );
                        } else {
                            AppBarRow(content = move |s: &dyn AppBarRowScope| content(s));
                        }
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    // 100 dp: one item and the ⋮; the disabled item takes no click in the bar.
    rule.on_node_with_content_description("Kapalı").assert_is_not_enabled().perform_click();
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Özel öğe").assert_exists();
    rule.on_node_with_text("Düzenle").assert_exists();
    rule.on_node_with_text("Kapalı öğe").assert_does_not_exist(); // (in the bar)
    rule.on_node_with_content_description("More options").perform_click(); // shown: closes it
    rule.wait_for_idle();
    rule.on_node_with_text("Özel öğe").assert_does_not_exist();
    rule.on_node_with_content_description("More options").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Özel öğe").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Özel öğe").assert_does_not_exist();
    assert_eq!(*log.borrow(), ["özel"]);
    own_indicator.set(true);
    rule.wait_for_idle();
    rule.on_node_with_text("kapalı").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("açık").assert_exists();
    rule.on_node_with_text("Özel öğe").assert_exists();

    let state = AppBarMenuState::new();
    let same = state.clone();
    state.show();
    assert!(same.is_showing() && state == same && state != AppBarMenuState::default());
    same.dismiss();
    assert!(!state.is_showing());
}

/// What the block reads runs it again when it changes, without composing the row (or what is around it) again.
fn app_bar_rows_run_their_block_again_when_what_it_reads_changes() {
    let count = mutable_state_of(1);
    let composed = Rc::new(Cell::new(0));
    let runs = Rc::new(Cell::new(0));
    let rule = {
        let (count, composed, runs) = (count.clone(), composed.clone(), runs.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                composed.set(composed.get() + 1);
                let (count, runs) = (count.clone(), runs.clone());
                AppBarRow(
                    modifier = Modifier.test_tag("satır"),
                    content = move |s: &dyn AppBarRowScope| {
                        runs.set(runs.get() + 1);
                        for i in 0..count.get() {
                            s.clickable_item(
                                || {},
                                move || {
                                    VectorIcon(Icons::Filled::Star(), content_description = Some(format!("yıldız {i}")))
                                },
                                format!("{i}"),
                            );
                        }
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    assert!(near(bounds(&rule, "satır").width(), 48.0));
    // (the block, a state the row sets as it runs, is read by the contents' own scopes: nothing runs again by itself)
    let settled = runs.get();
    rule.advance_frame();
    rule.wait_for_idle();
    assert_eq!(runs.get(), settled);
    count.set(3);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "satır").width(), 3.0 * 48.0));
    rule.on_node_with_content_description("yıldız 2").assert_exists();
    assert_eq!(composed.get(), 1);
    assert_eq!(runs.get(), settled + 1);
}

// ---- M15's samples ---------------------------------------------------------------------------------------------
// (as written, in Rust; Kotlin's twins in crates/kompoz/tests/kt/bottom_app_bars.kt)

/// The samples' icon buttons, each in a plain tooltip above it (their `TooltipBox(positionProvider =
/// TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above), tooltip = { PlainTooltip {
/// Text(tooltip) } }, state = rememberTooltipState()) { IconButton(onClick = { /* doSomething() */ }) { Icon(…) } }`).
#[kompozable]
fn TooltipIconButton(tooltip: &'static str, image_vector: ImageVector, content_description: &'static str) {
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
        tooltip = move |scope: &dyn TooltipScope| scope.PlainTooltip(content = move || Text(tooltip)),
        state = remember_tooltip_state(),
        content = move || {
            let image_vector = image_vector.clone();
            IconButton(
                on_click = || { /* doSomething() */ },
                content = move || {
                    VectorIcon(image_vector.clone(), content_description = Some(content_description.to_string()))
                },
            );
        },
    );
}

/// The samples' FAB in a plain tooltip: `FloatingActionButton(modifier, onClick = { /* do something */ },
/// containerColor = BottomAppBarDefaults.bottomAppBarFabColor, elevation =
/// FloatingActionButtonDefaults.bottomAppBarFabElevation()) { Icon(Icons.Filled.Add, "Add") }`.
#[kompozable]
fn AddFab(modifier: Modifier) {
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
        tooltip = |scope: &dyn TooltipScope| scope.PlainTooltip(content = || Text("Add")),
        state = remember_tooltip_state(),
        content = move || {
            FloatingActionButton(
                modifier = modifier.clone(),
                on_click = || { /* do something */ },
                container_color = BottomAppBarDefaults.bottom_app_bar_fab_color(),
                elevation = FloatingActionButtonDefaults.bottom_app_bar_fab_elevation(),
                content = || VectorIcon(Icons::Filled::Add(), Some("Add".to_string())),
            );
        },
    );
}

/// The exit-always samples' content: `LazyColumn(contentPadding = innerPadding, verticalArrangement =
/// Arrangement.spacedBy(8.dp))` of the 76 numbered rows.
#[kompozable]
fn SampleList(inner_padding: PaddingValues) {
    LazyColumn(
        content_padding = inner_padding,
        vertical_arrangement = Arrangement::spaced_by(8.dp()),
        content = |scope: &mut LazyListScope| {
            let list: Rc<Vec<String>> = Rc::new((0..=75).map(|it| it.to_string()).collect());
            let count = list.len() as i32;
            scope.items(count, move |it| {
                let list = list.clone();
                kompozable_block!({
                    Text(
                        text = list[it as usize].clone(),
                        style = MaterialTheme::typography().body_large,
                        modifier = Modifier.fill_max_width().padding_hv(16.dp(), 0.dp()),
                    )
                })
            });
        },
    );
}

/// The flexible samples' five buttons, the middle one a 56 dp wide filled icon button (`buttons.forEachIndexed {
/// index, button -> TooltipBox(…) { if (index == 2) FilledIconButton(modifier = Modifier.width(56.dp), …) { … } else
/// IconButton(…) { … } } }`).
#[kompozable]
fn SampleButtons() {
    let icons = [
        Icons::AutoMirrored::Filled::ArrowBack(),
        Icons::AutoMirrored::Filled::ArrowForward(),
        Icons::Filled::Add(),
        Icons::Filled::Check(),
        Icons::Filled::Edit(),
    ];
    let buttons = ["Back", "Forward", "Add", "Check", "Edit"];
    for (index, button) in buttons.into_iter().enumerate() {
        let icon = icons[index].clone();
        TooltipBox(
            position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
            tooltip = move |scope: &dyn TooltipScope| scope.PlainTooltip(content = move || Text(button)),
            state = remember_tooltip_state(),
            content = move || {
                let icon = icon.clone();
                if index == 2 {
                    FilledIconButton(
                        modifier = Modifier.width(56.dp()),
                        on_click = || { /* doSomething() */ },
                        content = move || VectorIcon(icon.clone(), content_description = Some(button.to_string())),
                    );
                } else {
                    IconButton(
                        on_click = || { /* doSomething() */ },
                        content = move || VectorIcon(icon.clone(), content_description = Some(button.to_string())),
                    );
                }
            },
        );
    }
}

#[kompozable]
fn SimpleBottomAppBar() {
    BottomAppBar(actions = || TooltipIconButton("Menu", Icons::Filled::Menu(), "Menu button"));
}

#[kompozable]
fn BottomAppBarWithFAB() {
    BottomAppBar(
        actions = || {
            TooltipIconButton("Check", Icons::Filled::Check(), "Check");
            TooltipIconButton("Edit", Icons::Filled::Edit(), "Edit");
        },
        floating_action_button = Some(Rc::new(|| AddFab(Modifier))),
    );
}

#[kompozable]
fn ExitAlwaysBottomAppBar() {
    let scroll_behavior = BottomAppBarDefaults.exit_always_scroll_behavior();
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        bottom_bar = move || {
            BottomAppBar(
                actions = || {
                    TooltipIconButton("Check", Icons::Filled::Check(), "Check");
                    TooltipIconButton("Edit", Icons::Filled::Edit(), "Edit");
                },
                scroll_behavior = Some(bar.clone()),
            );
        },
        floating_action_button = || AddFab(Modifier.offset(y = 4.dp())),
        floating_action_button_position = FabPosition::EndOverlay,
        content = |inner_padding: PaddingValues| SampleList(inner_padding),
    );
}

/// The flexible exit-always samples: `FlexibleBottomAppBar(horizontalArrangement, contentPadding?, scrollBehavior,
/// containerColor?) { the five buttons }` over the list.
#[kompozable]
fn ExitAlwaysFlexibleSample(
    horizontal_arrangement: HorizontalArrangement,
    content_padding: Option<PaddingValues>,
    container_color: Option<Color>,
) {
    let scroll_behavior = BottomAppBarDefaults.exit_always_scroll_behavior();
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        bottom_bar = move || {
            let content_padding = content_padding.clone().unwrap_or_else(BottomAppBarDefaults::FlexibleContentPadding);
            let container_color = container_color.unwrap_or_else(|| BottomAppBarDefaults.container_color());
            FlexibleBottomAppBar(
                horizontal_arrangement = horizontal_arrangement.clone(),
                content_padding = content_padding,
                scroll_behavior = Some(bar.clone()),
                container_color = container_color,
                content = || SampleButtons(),
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding),
    );
}

#[kompozable]
fn ExitAlwaysBottomAppBarSpacedAround() {
    ExitAlwaysFlexibleSample(Arrangement::SpaceAround, Some(PaddingValues::hv(0.dp(), 0.dp())), None);
}

#[kompozable]
fn ExitAlwaysBottomAppBarSpacedBetween() {
    ExitAlwaysFlexibleSample(Arrangement::SpaceBetween, None, None);
}

#[kompozable]
fn ExitAlwaysBottomAppBarSpacedEvenly() {
    ExitAlwaysFlexibleSample(Arrangement::SpaceEvenly, Some(PaddingValues::hv(0.dp(), 0.dp())), None);
}

#[kompozable]
fn ExitAlwaysBottomAppBarFixed() {
    ExitAlwaysFlexibleSample(BottomAppBarDefaults::FlexibleFixedHorizontalArrangement(), None, None);
}

#[kompozable]
fn ExitAlwaysBottomAppBarFixedVibrant() {
    ExitAlwaysFlexibleSample(
        BottomAppBarDefaults::FlexibleFixedHorizontalArrangement(),
        None,
        // TODO(b/356885344): tokens
        Some(MaterialTheme::color_scheme().primary_container),
    );
}

#[kompozable]
fn BottomAppBarWithOverflow() {
    let icons = vec![
        Icons::AutoMirrored::Filled::ArrowBack(),
        Icons::AutoMirrored::Filled::ArrowForward(),
        Icons::Filled::Add(),
        Icons::Filled::Check(),
        Icons::Filled::Edit(),
        Icons::Filled::Favorite(),
    ];
    let items = ["Back", "Forward", "Add", "Check", "Edit", "Favorite"];
    FlexibleBottomAppBar(
        content_padding = PaddingValues::hv(96.dp(), 0.dp()),
        horizontal_arrangement = BottomAppBarDefaults::FlexibleFixedHorizontalArrangement(),
        content = move || {
            let icons = icons.clone();
            AppBarRow(
                overflow_indicator = |menu_state: AppBarMenuState| {
                    TooltipBox(
                        position_provider =
                            TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                        tooltip = |scope: &dyn TooltipScope| scope.PlainTooltip(content = || Text("Overflow")),
                        state = remember_tooltip_state(),
                        content = move || {
                            let menu_state = menu_state.clone();
                            IconButton(
                                on_click = move || {
                                    if menu_state.is_showing() {
                                        menu_state.dismiss();
                                    } else {
                                        menu_state.show();
                                    }
                                },
                                content = || {
                                    VectorIcon(
                                        image_vector = Icons::Filled::MoreVert(),
                                        content_description = Some("Overflow".to_string()),
                                    )
                                },
                            );
                        },
                    );
                },
                content = move |scope: &dyn AppBarRowScope| {
                    for (index, item) in items.into_iter().enumerate() {
                        let icon = icons[index].clone();
                        scope.clickable_item(
                            on_click = || { /* doSomething() */ },
                            icon = move || VectorIcon(icon.clone(), content_description = Some(item.to_string())),
                            label = item.to_string(),
                        );
                    }
                },
            );
        },
    );
}

/// A long press on what `description` describes, and the frame after: its tooltip shows.
fn long_press(rule: &KompozTestRule, description: &str) {
    let host = rule.host();
    let center = center_of(rule, description);
    host.pointer_down(center, -1, 0, PointerType::Touch);
    rule.advance_time_by(600);
    host.pointer_up(center, -1, 0);
    host.pointer_exit();
    rule.advance_frame();
}

/// The bar drawn: the lowest rect as wide as the window and less than half its height, and its color.
fn bar(rule: &KompozTestRule) -> (Rect, Color) {
    let c = rule.draw();
    c.rects
        .iter()
        .zip(&c.rect_colors)
        .filter(|(r, _)| near(r.left, 0.0) && near(r.width(), 400.0) && r.height() < 400.0)
        .max_by(|(a, _), (b, _)| a.top.total_cmp(&b.top))
        .map(|(r, c)| (*r, *c))
        .unwrap_or((Rect::Zero, Color::Unspecified))
}

/// The bounds of what `description` describes.
fn described(rule: &KompozTestRule, description: &str) -> Rect {
    rule.on_node_with_content_description(description).get_unclipped_bounds_in_root()
}

fn center_of(rule: &KompozTestRule, description: &str) -> Offset {
    described(rule, description).center()
}

/// Scrolls the sample's content up by `by` (a slow drag in the window's middle) and lets it settle.
fn scroll_up(rule: &KompozTestRule, by: f32) {
    let h = rule.host();
    let (from, to) = (at(200.0, 500.0), at(200.0, 500.0 - by));
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / 20.0), -1, 0);
    }
    rule.advance_time_by(200);
    h.pointer_up(to, -1, 0);
    rule.wait_for_idle();
}

fn simple_bottom_app_bar() {
    let rule = KompozTestRule::new(SimpleBottomAppBar);
    // 80 dp in the surface container's color; its action 4 dp in and down, centered in the rest.
    assert_eq!(bar(&rule), (Rect::new(0.0, 0.0, 400.0, 80.0), light_color_scheme().surface_container));
    let menu = center_of(&rule, "Menu button");
    assert!(near(menu.x, 4.0 + 24.0) && near(menu.y, 4.0 + 38.0), "{menu:?}");
    // Its tooltip, on a long press, next to it (below: no room above at the window's top).
    long_press(&rule, "Menu button");
    let (tip, button) = (text_bounds(&rule, "Menu"), described(&rule, "Menu button"));
    assert!(tip.top >= button.bottom || tip.bottom <= button.top, "{tip:?} {button:?}");
}

fn bottom_app_bar_with_fab() {
    let rule = KompozTestRule::new(BottomAppBarWithFAB);
    assert_eq!(bar(&rule).0, Rect::new(0.0, 0.0, 400.0, 80.0));
    assert!(near(center_of(&rule, "Check").x, 28.0) && near(center_of(&rule, "Edit").x, 28.0 + 48.0));
    // The FAB 12 dp down, 16 dp in from the end, in the bar's FAB color.
    let fab = described(&rule, "Add");
    assert!(same_rect(fab, Rect::new(400.0 - 16.0 - 56.0, 12.0, 400.0 - 16.0, 12.0 + 56.0)), "{fab:?}");
    let c = rule.draw();
    let fab_color = c.rects.iter().zip(&c.rect_colors).find(|(r, _)| same_rect(**r, fab)).map(|(_, c)| *c);
    assert_eq!(fab_color, Some(light_color_scheme().secondary_container));
}

fn exit_always_bottom_app_bar() {
    let rule = KompozTestRule::new(ExitAlwaysBottomAppBar);
    assert_eq!(bar(&rule).0, Rect::new(0.0, 720.0, 400.0, 800.0));
    // The FAB over the bar (EndOverlay), 16 dp from the edges, 4 dp lower.
    let fab = described(&rule, "Add");
    let expected = Rect::new(400.0 - 16.0 - 56.0, 800.0 - 16.0 - 56.0 + 4.0, 400.0 - 16.0, 800.0 - 16.0 + 4.0);
    assert!(same_rect(fab, expected), "{fab:?}");
    assert!(near(text_bounds(&rule, "0").top, 0.0));
    // Scrolled on: the bar leaves (laid out 0 dp tall at the bottom, what it draws below the window); the FAB stays.
    scroll_up(&rule, 300.0);
    assert!(bar(&rule).0.top >= 799.5, "{:?}", bar(&rule));
    assert!(same_rect(described(&rule, "Add"), fab));
    // Scrolled back a little: it comes back first.
    let h = rule.host();
    h.pointer_down(at(200.0, 300.0), -1, 0, PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(at(200.0, 300.0 + 2.0 * i as f32), -1, 0);
    }
    assert!(bar(&rule).0.top < 780.0, "{:?}", bar(&rule));
}

/// A flexible sample: 64 dp at the bottom in `color`, its five buttons' centers at `centers` (the middle one 56 dp
/// wide); scrolled on, the bar leaves (what it draws below the window).
fn flexible_sample(rule: &KompozTestRule, color: Color, centers: [f32; 5]) {
    assert_eq!(bar(rule), (Rect::new(0.0, 736.0, 400.0, 800.0), color));
    for (name, x) in ["Back", "Forward", "Add", "Check", "Edit"].into_iter().zip(centers) {
        let c = center_of(rule, name);
        assert!(near(c.x, x) && near(c.y, 768.0), "{name}: {c:?}, not {x}");
    }
    assert!(near(described(rule, "Add").width(), 56.0));
    scroll_up(rule, 300.0);
    assert!(bar(rule).0.top >= 799.5, "{:?}", bar(rule));
}

fn exit_always_bottom_app_bar_spaced_around() {
    // No padding: the 152 dp left shared around the five, half of a share at the ends.
    let share = (400.0 - 4.0 * 48.0 - 56.0) / 5.0;
    let first = share / 2.0 + 24.0;
    let rule = KompozTestRule::new(ExitAlwaysBottomAppBarSpacedAround);
    let centers = [first, first + 48.0 + share, 200.0, 400.0 - first - 48.0 - share, 400.0 - first];
    flexible_sample(&rule, light_color_scheme().surface_container, centers);
}

fn exit_always_bottom_app_bar_spaced_between() {
    // 16 dp in: the first and the last at the sides, the 120 dp left between them.
    let gap = (400.0 - 32.0 - 4.0 * 48.0 - 56.0) / 4.0;
    let rule = KompozTestRule::new(ExitAlwaysBottomAppBarSpacedBetween);
    let centers = [40.0, 40.0 + 48.0 + gap, 200.0, 360.0 - 48.0 - gap, 360.0];
    flexible_sample(&rule, light_color_scheme().surface_container, centers);
}

fn exit_always_bottom_app_bar_spaced_evenly() {
    let gap = (400.0 - 4.0 * 48.0 - 56.0) / 6.0;
    let rule = KompozTestRule::new(ExitAlwaysBottomAppBarSpacedEvenly);
    let centers = [gap + 24.0, 2.0 * gap + 72.0, 200.0, 400.0 - 2.0 * gap - 72.0, 400.0 - gap - 24.0];
    flexible_sample(&rule, light_color_scheme().surface_container, centers);
}

/// The fixed arrangement: 32 dp between them in the 368 dp inside the padding; 376 dp asked for, the last one is
/// given the 40 dp left (a Row measures its children in turn with what is left).
fn fixed_centers() -> [f32; 5] {
    [40.0, 120.0, 204.0, 288.0, 364.0]
}

fn exit_always_bottom_app_bar_fixed() {
    let rule = KompozTestRule::new(ExitAlwaysBottomAppBarFixed);
    flexible_sample(&rule, light_color_scheme().surface_container, fixed_centers());
}

fn exit_always_bottom_app_bar_fixed_vibrant() {
    let rule = KompozTestRule::new(ExitAlwaysBottomAppBarFixedVibrant);
    flexible_sample(&rule, light_color_scheme().primary_container, fixed_centers());
}

fn bottom_app_bar_with_overflow() {
    let rule = KompozTestRule::new(BottomAppBarWithOverflow);
    // 96 dp in at each side: 208 dp, 48 of them kept for the indicator — three items and the indicator, centered.
    assert_eq!(bar(&rule), (Rect::new(0.0, 0.0, 400.0, 64.0), light_color_scheme().surface_container));
    let back = center_of(&rule, "Back");
    assert!(near(back.x, 104.0 + 24.0) && near(back.y, 32.0), "{back:?}");
    assert!(near(center_of(&rule, "Add").x, 104.0 + 2.0 * 48.0 + 24.0));
    assert!(near(center_of(&rule, "Overflow").x, 104.0 + 3.0 * 48.0 + 24.0));
    rule.on_node_with_text("Check").assert_does_not_exist();
    rule.on_node_with_content_description("Overflow").perform_click();
    rule.wait_for_idle();
    for label in ["Check", "Edit", "Favorite"] {
        rule.on_node_with_text(label).assert_exists();
    }
    rule.on_node_with_text("Back").assert_does_not_exist();
    // An item from the menu closes it; the indicator shows it again.
    rule.on_node_with_text("Edit").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Favorite").assert_does_not_exist();
    rule.on_node_with_content_description("Overflow").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Favorite").assert_exists();
}

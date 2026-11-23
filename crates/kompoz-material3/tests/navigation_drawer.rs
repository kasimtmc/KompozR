//! Material's navigation drawers as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's drawer tests by
//! name (kompoz_tests.cpp; M15's behaviour and values where the C++ differed, said at each), Compose's behaviour
//! (the state's values, savers and errors, the keyboard's focus and Escape, the defaults, the item's colors), and the
//! M15 samples of DrawerSamples.kt.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_coroutines::{CoroutineScope, LaunchedEffect, remember_coroutine_scope};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_extended::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::SemanticsMatcher;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::platform::{
    InputMode, InputModeManager, Insets, LocalInputModeManager, LocalLayoutDirection, SystemInsets,
};
use kompoz_ui::semantics::{Role, SemanticsActions, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    // (the C++ Kompoz's)
    permanent_and_dismissible_drawers,
    standard_drawer_sheets_pad_by_the_insets_left_to_them,
    drawer_sheets_follow_the_back_gesture,
    navigation_drawer_opens_closes_and_leaves_content_usable,
    modal_drawer_closes_by_its_own_width,
    navigation_drawer_items_as_compose_1_4,
    rtl_drawer_and_swipe_to_dismiss_follow_the_mirror,
    // (Compose's behaviour)
    drawer_states_move_from_code_and_save_their_value,
    drawers_take_the_keyboard_focus_and_close_by_escape,
    drawers_are_dismissed_by_accessibility,
    a_bouncy_opening_stretches_the_sheet_and_keeps_its_content,
    navigation_drawer_items_take_their_colors_icon_and_badge,
    drawer_defaults_follow_the_tokens,
    // (M15's DrawerSamples)
    modal_navigation_drawer_sample,
    permanent_navigation_drawer_sample,
    dismissible_navigation_drawer_sample,
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

fn near_within(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
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

/// The C++ tests' `tap`.
fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_frame();
}

/// Escape pressed: back from the root when nothing focused takes it.
fn escape(rule: &KompozTestRule) -> bool {
    rule.host().key_down(Key::Escape, KeyModifiers::default())
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

/// A node merged into its item's, by its tag.
fn unmerged(rule: &KompozTestRule, tag: &str) -> Rect {
    named_args!(rule.on_node_with_tag(tag, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
}

fn some(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with_text(text).get_unclipped_bounds_in_root()
}

/// How far out a drawer is, 0 (closed at -`width`) to 1 (the C++'s `openFraction`; Compose's drawers have their
/// scrim's `calculateFraction` of the offset).
fn open_fraction(state: &DrawerState, width: f32) -> f32 {
    ((state.current_offset() + width) / width).clamp(0.0, 1.0)
}

// ---- The C++ Kompoz's tests --------------------------------------------------------------------------------------

fn permanent_and_dismissible_drawers() {
    let (drawer, scope) = (slot::<DrawerState>(), slot::<CoroutineScope>());
    let rule = {
        let (drawer, scope) = (drawer.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let drawer = drawer.clone();
                Column(
                    content = move || {
                        PermanentNavigationDrawer(
                            || {
                                PermanentDrawerSheet(
                                    modifier = Modifier.width(250.dp()),
                                    content = || Text("Kalıcı menü"),
                                )
                            },
                            modifier = Modifier.height(200.dp()),
                            content = || Text("Kalıcı içerik", modifier = Modifier.test_tag("permanent")),
                        );
                        let s = remember_drawer_state(DrawerValue::Closed);
                        *drawer.borrow_mut() = Some(s.clone());
                        let sheet_state = s.clone();
                        // (M15's dismissible drawer is as big as its content, the C++'s filled its room: it fills
                        // it here so the drags below land on it; and M15's plain sheet doesn't handle back — the
                        // C++'s did —, its drawer state's form does)
                        DismissibleNavigationDrawer(
                            move || {
                                DismissibleDrawerSheetWithDrawerState(
                                    sheet_state.clone(),
                                    modifier = Modifier.width(250.dp()),
                                    content = || Text("Açılır menü", modifier = Modifier.test_tag("menu")),
                                )
                            },
                            modifier = Modifier.fill_max_size(),
                            drawer_state = Some(s),
                            content = || Text("İçerik", modifier = Modifier.test_tag("content")),
                        );
                    },
                );
            }),
            IntSize::new(600, 800),
        )
    };
    // Permanent: the content starts after the drawer.
    assert!(near(bounds(&rule, "permanent").left, 250.0));
    rule.on_node_with_text("Kalıcı menü").assert_exists();
    // Dismissible: closed, then open pushing the content over by the drawer's width.
    assert!(near(bounds(&rule, "content").left, 0.0));
    // (not placed while closed, as M15 does — so keyboard focus doesn't go there —; still composed, as in Compose)
    rule.on_node_with_tag("menu").assert_is_not_displayed();
    let s = get(&drawer);
    launch(&scope, s.snap_to(DrawerValue::Open));
    rule.wait_for_idle();
    // (M15's sheet has no padding of its own: the menu at its start; the C++'s padded it by 12 dp)
    assert!(near(bounds(&rule, "content").left, 250.0) && near(bounds(&rule, "menu").left, 0.0));
    swipe(&rule, Offset::new(400.0, 500.0), Offset::new(100.0, 500.0), 20); // dragged closed
    rule.wait_for_idle();
    assert!(!s.is_open() && near(bounds(&rule, "content").left, 0.0));
    swipe(&rule, Offset::new(50.0, 500.0), Offset::new(400.0, 500.0), 20); // and open
    rule.wait_for_idle();
    assert!(s.is_open() && near(bounds(&rule, "content").left, 250.0));
    assert!(escape(&rule)); // back closes it
    rule.wait_for_idle();
    assert!(!s.is_open());
}

fn standard_drawer_sheets_pad_by_the_insets_left_to_them() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Row(content = || {
                PermanentDrawerSheet(
                    modifier = Modifier.width(240.dp()),
                    content = || Text("Kenarda", modifier = Modifier.test_tag("edge")),
                );
                // A screen that already padded for the status bar.
                Box(
                    modifier = Modifier
                        .padding_sides_with(0.dp(), 24.dp(), 0.dp(), 0.dp())
                        .consume_window_insets_padding_values(PaddingValues::new(0.dp(), 24.dp(), 0.dp(), 0.dp())),
                    content = || {
                        PermanentDrawerSheet(
                            modifier = Modifier.width(240.dp()),
                            content = || Text("İçeride", modifier = Modifier.test_tag("inside")),
                        )
                    },
                );
            });
        }),
        IntSize::new(600, 800),
    );
    rule.host().set_system_insets(SystemInsets { status_bars: Insets::new(0, 24, 0, 0), ..Default::default() });
    rule.wait_for_idle();
    // (M15's sheet pads by the insets alone; the C++'s added 12 dp below them)
    assert!(near(bounds(&rule, "edge").top, 24.0), "{:?}", bounds(&rule, "edge"));
    assert!(near(bounds(&rule, "inside").top, 24.0), "{:?}", bounds(&rule, "inside"));
}

/// A predictive back gesture's transforms: the sheet's (the one lowering: scale Y) and its content's (scaled back
/// across only), and where each scales about, in the layer's own space.
#[derive(Debug)]
struct Scales {
    sheet_x: f32,
    sheet_y: f32,
    pivot_x: f32,
    pivot_y: f32,
    content_x: f32,
    content_pivot_x: f32,
}

fn scales(rule: &KompozTestRule) -> Scales {
    rule.advance_frame();
    let canvas: RecordingCanvas = rule.draw();
    let mut out = Scales {
        sheet_x: 1.0,
        sheet_y: 1.0,
        pivot_x: f32::NAN,
        pivot_y: f32::NAN,
        content_x: 1.0,
        content_pivot_x: f32::NAN,
    };
    // (a scaled graphics layer is drawn through a transform: scaled about its pivot, in the layer's own space)
    let pivot = |translation: f32, scale: f32| if scale != 1.0 { translation / (1.0 - scale) } else { f32::NAN };
    for m in &canvas.transforms {
        let (sx, sy, tx, ty) = (m.get(0, 0), m.get(1, 1), m.get(3, 0), m.get(3, 1));
        if sy != 1.0 {
            out = Scales { sheet_x: sx, sheet_y: sy, pivot_x: pivot(tx, sx), pivot_y: pivot(ty, sy), ..out };
        } else if sx != 1.0 {
            out.content_x = sx;
            out.content_pivot_x = pivot(tx, sx);
        }
    }
    out
}

fn back_event(touch_x: f32, progress: f32, swipe_edge: kompoz_ui::back::SwipeEdge) -> kompoz_ui::back::BackEventCompat {
    kompoz_ui::back::BackEventCompat { touch_x, touch_y: 400.0, progress, swipe_edge, frame_time_millis: 0 }
}

fn drawer_sheets_follow_the_back_gesture() {
    use kompoz_ui::back::SwipeEdge::{Left, Right};
    let dismissible = mutable_state_of(false);
    let rtl = mutable_state_of(false);
    let (drawer, scope) = (slot::<DrawerState>(), slot::<CoroutineScope>());
    let rule = {
        let (dismissible, rtl, drawer, scope) = (dismissible.clone(), rtl.clone(), drawer.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let state = remember_drawer_state(DrawerValue::Open);
                *drawer.borrow_mut() = Some(state.clone());
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let direction = if rtl.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
                let dismissible = dismissible.clone();
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
                    let sheet_state = state.clone();
                    // (M15's plain sheets don't take back: their drawer state's forms do, as the C++'s sheets did)
                    if dismissible.get() {
                        DismissibleNavigationDrawer(
                            move || {
                                DismissibleDrawerSheetWithDrawerState(
                                    sheet_state.clone(),
                                    modifier = Modifier.width(300.dp()),
                                    content = || Text("Gelen"),
                                )
                            },
                            drawer_state = Some(state.clone()),
                            content = || {},
                        );
                    } else {
                        // (M15's sheet is as wide as its content, within 240 to 360 dp: the content fills it here — the
                        // C++'s sheet was always 360 dp wide)
                        ModalNavigationDrawer(
                            move || {
                                ModalDrawerSheetWithDrawerState(
                                    sheet_state.clone(),
                                    content = || Text("Gelen", modifier = Modifier.fill_max_width()),
                                )
                            },
                            drawer_state = Some(state.clone()),
                            content = || PermanentDrawerSheet(content = || {}),
                        );
                    }
                });
            }),
            IntSize::new(400, 800),
        )
    };
    rule.wait_for_idle();
    let host = rule.host();
    let p =
        kompoz_animation::Easing::transform(&kompoz_animation::CubicBezierEasing::constant(0.1, 0.1, 0.0, 1.0), 0.5);
    // From the drawer's own edge: wider (up to 12 dp of the sheet's 360), lower (48 of 800), from the window's edge;
    // the content keeps its shape, from its top right.
    host.back_started(back_event(0.0, 0.0, Left));
    host.back_progressed(back_event(40.0, 0.5, Left));
    let a = scales(&rule);
    assert!(near_within(a.sheet_x, 1.0 + 12.0 * p / 360.0, 0.001), "{a:?}");
    assert!(near_within(a.sheet_y, 1.0 - 48.0 * p / 800.0, 0.001), "{a:?}");
    // (the window's edge, half way down)
    assert!(near_within(a.pivot_x, 0.0, 0.01) && near_within(a.pivot_y, 400.0, 0.01), "{a:?}");
    // The content's right edge: M15's content is the sheet's 360 wide (the C++'s 344 of a sheet reaching 16 dp past
    // the window's edge).
    assert!(
        near_within(a.content_x, a.sheet_y / a.sheet_x, 0.001) && near_within(a.content_pivot_x, 360.0, 0.01),
        "{a:?}"
    );
    host.back_cancelled(); // as it was, still open
    rule.wait_for_idle();
    let b = scales(&rule);
    let s = get(&drawer);
    assert!(b.sheet_x == 1.0 && b.sheet_y == 1.0 && b.content_x == 1.0 && s.is_open(), "{b:?}");
    // From the other edge: narrower (24 dp); done, it closes as it is.
    host.back_started(back_event(400.0, 0.0, Right));
    host.back_progressed(back_event(360.0, 0.5, Right));
    assert!(near_within(scales(&rule).sheet_x, 1.0 - 24.0 * p / 360.0, 0.001));
    assert!(host.back_invoked());
    rule.advance_time_by(48);
    let c = scales(&rule);
    // (closing: M15's `isOpen` is of the settled value, still open until the animation ends — the C++'s turned at
    // once; its target is closed)
    assert!(s.target_value() == DrawerValue::Closed && s.is_animation_running(), "{s:?}");
    assert!(near_within(c.sheet_x, 1.0 - 24.0 * p / 360.0, 0.001) && c.sheet_y < 1.0, "{c:?}");
    rule.wait_for_idle();
    assert!(open_fraction(&s, 360.0) == 0.0 && !host.wants_back());
    // Opened again: whole. From its edge, done: it goes back across as it closes, still lower.
    launch(&scope, s.open());
    rule.wait_for_idle();
    let d = scales(&rule);
    assert!(s.is_open() && d.sheet_x == 1.0 && d.sheet_y == 1.0, "{d:?}");
    host.back_started(back_event(0.0, 0.0, Left));
    host.back_progressed(back_event(40.0, 0.5, Left));
    assert!(host.back_invoked());
    rule.advance_time_by(150);
    let e = scales(&rule);
    assert!(e.sheet_x < 1.0 + 12.0 * p / 360.0 - 0.001 && e.sheet_y < 1.0, "{e:?}");
    rule.wait_for_idle();
    assert!(!s.is_open());
    // A dismissible one: from its edge, which is the window's.
    dismissible.set(true);
    launch(&scope, s.open());
    rule.wait_for_idle();
    host.back_started(back_event(0.0, 0.0, Left));
    host.back_progressed(back_event(40.0, 0.5, Left));
    let f = scales(&rule);
    assert!(near_within(f.sheet_x, 1.0 + 12.0 * p / 300.0, 0.001) && near_within(f.pivot_x, 0.0, 0.01), "{f:?}");
    assert!(near_within(f.sheet_y, 1.0 - 48.0 * p / 800.0, 0.001), "{f:?}");
    host.back_cancelled();
    // Right to left: the drawer's edge is the right one; mirrored. (In the layer's own space: the sheet's right
    // edge, 360, is the window's 400, the sheet at 40; its content's left edge, 0, the window's 40 — the C++'s 56
    // of its 344 wide content.)
    dismissible.set(false);
    rtl.set(true);
    rule.wait_for_idle();
    assert!(near(rule.on_node_with_text("Gelen").get_unclipped_bounds_in_root().left, 40.0));
    host.back_started(back_event(400.0, 0.0, Right));
    host.back_progressed(back_event(360.0, 0.5, Right));
    let g = scales(&rule);
    assert!(near_within(g.sheet_x, 1.0 + 12.0 * p / 360.0, 0.001) && near_within(g.pivot_x, 360.0, 0.01), "{g:?}");
    assert!(near_within(g.content_pivot_x, 0.0, 0.01), "{g:?}");
    host.back_cancelled();
}

fn navigation_drawer_opens_closes_and_leaves_content_usable() {
    let clicks = Rc::new(Cell::new(0));
    let (drawer, scope) = (slot::<DrawerState>(), slot::<CoroutineScope>());
    let rule = {
        let (clicks, drawer, scope) = (clicks.clone(), drawer.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let state = remember(|| DrawerState(DrawerValue::Closed));
                *drawer.borrow_mut() = Some(state.clone());
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let (clicks, sheet_state) = (clicks.clone(), state.clone());
                // (M15's plain sheet doesn't take back — the C++'s did —: its drawer state's form does)
                ModalNavigationDrawer(
                    move || {
                        ModalDrawerSheetWithDrawerState(
                            sheet_state.clone(),
                            modifier = Modifier.test_tag("sheet"),
                            content = || NavigationDrawerItem(|| Text("Gelen kutusu"), true, on_click = || {}),
                        )
                    },
                    drawer_state = Some(state),
                    content = move || {
                        let clicks = clicks.clone();
                        Button(
                            move || clicks.set(clicks.get() + 1),
                            modifier = Modifier.padding(100.dp()).test_tag("content"),
                            content = || Text("İçerik"),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    rule.on_node_with_text("İçerik").perform_click(); // closed: the content works
    assert_eq!(clicks.get(), 1);
    rule.on_node_with_text("Gelen kutusu").assert_is_not_displayed();
    let state = get(&drawer);
    launch(&scope, state.open());
    rule.wait_for_idle();
    assert!(state.is_open() && near(open_fraction(&state, 360.0), 1.0));
    // 400 wide: M15's sheet is its 360 dp (the C++'s 344 + 16 hidden past the edge); the item's text 16 dp in (the
    // C++'s items were 12 dp in by themselves: M15's take NavigationDrawerItemDefaults.ItemPadding if asked).
    let sheet = bounds(&rule, "sheet");
    assert!(near(sheet.width(), 360.0), "{sheet:?}");
    let text =
        named_args!(rule.on_node_with_text("Gelen kutusu", use_unmerged_tree = true)).get_unclipped_bounds_in_root();
    assert!(near(text.left, 16.0), "{text:?}");
    tap(&rule, Offset::new(380.0, 400.0)); // the scrim takes the tap and closes the drawer
    rule.wait_for_idle();
    assert!(clicks.get() == 1 && !state.is_open());
    rule.on_node_with_text("Gelen kutusu").assert_is_not_displayed();
    swipe(&rule, Offset::new(20.0, 400.0), Offset::new(300.0, 400.0), 12); // drag open from the content
    rule.wait_for_idle();
    assert!(state.is_open());
    assert!(escape(&rule)); // back closes it
    rule.wait_for_idle();
    assert!(!state.is_open());
    launch(&scope, state.open());
    rule.wait_for_idle();
    swipe(&rule, Offset::new(300.0, 400.0), Offset::new(40.0, 400.0), 12); // drag it shut
    rule.wait_for_idle();
    assert!(!state.is_open() && near(open_fraction(&state, 360.0), 0.0));
}

fn modal_drawer_closes_by_its_own_width() {
    // In a window narrower than the default sheet (360 dp): its offset goes by the width it got.
    let (drawer, scope) = (slot::<DrawerState>(), slot::<CoroutineScope>());
    let rule = {
        let (drawer, scope) = (drawer.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let state = remember_drawer_state(DrawerValue::Open);
                *drawer.borrow_mut() = Some(state.clone());
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                ModalNavigationDrawer(
                    || {
                        // (M15's sheet is as wide as its content, within 240 to 360 dp: the content fills it here,
                        // so the window narrower than 360 sets its width, as the C++'s did)
                        ModalDrawerSheet(
                            content = || Text("Gelen", modifier = Modifier.fill_max_width().test_tag("gelen")),
                        )
                    },
                    drawer_state = Some(state),
                    content = || {},
                );
            }),
            IntSize::new(300, 600),
        )
    };
    let open = bounds(&rule, "gelen").left;
    assert!(near(open, 0.0));
    // Held part of the way while the window narrows. (M15 keeps a held drawer where the finger has it: the new
    // anchors wait for the drag to end, Compose's `updateAnchors` setting its drag target as a drag holds it; the
    // C++ moved it by the sheet's new width.)
    // (the clock by hand: the checks while held and on the way see the drawer before it settles)
    rule.main_clock().set_auto_advance(false);
    let h = rule.host();
    h.pointer_down(Offset::new(150.0, 300.0), -1, 0, PointerType::Touch);
    for i in 1..=5 {
        h.pointer_move(Offset::new(150.0 - 20.0 * i as f32, 300.0), -1, 0);
    }
    rule.advance_time_by(100);
    let held = bounds(&rule, "gelen").left;
    h.set_viewport_size(IntSize::new(250, 600));
    rule.advance_frame();
    // (read as the frame laid it out, without the implicit wait: what is due after it — the drag's block restarted
    // with the new anchors — moves it on, see below)
    let narrowed = rule.run_without_implicit_wait(|| bounds(&rule, "gelen"));
    assert!(held < open && near(narrowed.left, held), "{open} {held} {narrowed:?}");
    // (Compose's DragGestureNode hands the restarted block the drag's current event again — its `drag { while (event
    // !is DragStopped …) { (event as? DragDelta)?.let(processDelta); event = channel.receive() } }` — so the last
    // move, 20 px, is applied once more as the anchors change: the C++ and the old harness, reading right after the
    // frame, never saw it)
    assert!(near(bounds(&rule, "gelen").left, held - 20.0), "{held} {:?}", bounds(&rule, "gelen"));
    h.pointer_up(Offset::new(50.0, 300.0), -1, 0);
    finish_animations(&rule);
    // Let go: settled on the new anchors (open, 100 of 250 from it), the sheet 250 wide.
    let s = get(&drawer);
    assert!(s.is_open() && near(s.current_offset(), 0.0) && near(open_fraction(&s, 250.0), 1.0));
    h.set_viewport_size(IntSize::new(300, 600));
    launch(&scope, s.snap_to(DrawerValue::Open));
    finish_animations(&rule);
    launch(&scope, s.close());
    rule.advance_time_by(48); // part of the way
    let moving = bounds(&rule, "gelen").left;
    assert!(moving < open && moving > open - 300.0, "{moving}");
    finish_animations(&rule);
    // Closed by its own width (300): out of sight, not placed.
    assert!(near(s.current_offset(), -300.0));
    rule.on_node_with_tag("gelen").assert_is_not_displayed();
}

fn navigation_drawer_items_as_compose_1_4() {
    let picked = mutable_state_of(0);
    let ambient = Rc::new(RefCell::new(None::<TextUnit>));
    let rule = {
        let (picked, ambient) = (picked.clone(), ambient.clone());
        // (in a theme: the text style around is its body style — outside one M15's is unspecified, its size
        // falling back to 14 sp)
        KompozTestRule::new(kompozable_lambda!(move || {
            let (picked, ambient) = (picked.clone(), ambient.clone());
            MaterialTheme(
                content = move || {
                    *ambient.borrow_mut() = Some(LocalTextStyle.current().font_size);
                    let picked = picked.clone();
                    Column(
                        modifier = Modifier.width(300.dp()),
                        content = move || {
                            let (p0, p1) = (picked.clone(), picked.clone());
                            NavigationDrawerItem(
                                || Text("Gelen"),
                                picked.get() == 0,
                                on_click = move || p0.set(0),
                                modifier = Modifier.test_tag("gelen"),
                            );
                            NavigationDrawerItem(
                                || Box(modifier = Modifier.size_wh(10.dp(), 70.dp())),
                                picked.get() == 1,
                                on_click = move || p1.set(1),
                                modifier = Modifier.test_tag("uzun"),
                            );
                        },
                    );
                },
            );
        }))
    };
    // The label in the text style around it; taller labels make it taller than 56.
    let canvas = rule.draw();
    let ambient = ambient.borrow().unwrap();
    assert_eq!(canvas.text_styles.len(), 1);
    assert!(near(canvas.text_styles[0].font_size, ambient.value()), "{:?}", canvas.text_styles[0].font_size);
    assert!(near(bounds(&rule, "gelen").height(), 56.0) && near(bounds(&rule, "uzun").height(), 70.0));
    // Picked: its pill at once (the scheme's secondary container, rounded all the way).
    picked.set(1);
    rule.advance_frame();
    let after = rule.draw();
    let tall = bounds(&rule, "uzun");
    let cs = light_color_scheme();
    let pill = after.rects.iter().zip(&after.rect_colors).zip(&after.rect_radii).any(|((r, c), radius)| {
        near(r.left, tall.left)
            && near(r.top, tall.top)
            && near(r.width(), tall.width())
            && near(r.height(), tall.height())
            && *c == cs.secondary_container
            && near(*radius, 35.0)
    });
    assert!(pill, "{:?} {:?} {:?}", after.rects, after.rect_colors, after.rect_radii);
    // Tabs to accessibility, the picked one selected.
    let tree = rule.host().accessibility_tree();
    let tabs: Vec<_> = tree.iter().filter(|n| n.role == Some(Role::Tab)).collect();
    assert_eq!(tabs.len(), 2);
    rule.on_node_with_tag("uzun").assert_is_selected();
    rule.on_node_with_tag("gelen").assert_is_not_selected();
}

fn rtl_drawer_and_swipe_to_dismiss_follow_the_mirror() {
    // The ModalNavigationDrawer half of the C++ test; its swipe to dismiss half is WP5's, in swipe_pull.rs (the same
    // name).
    let drawer = slot::<DrawerState>();
    let rule = {
        let drawer = drawer.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let drawer = drawer.clone();
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
                    let state = remember(|| DrawerState(DrawerValue::Closed));
                    *drawer.borrow_mut() = Some(state.clone());
                    ModalNavigationDrawer(
                        || {
                            // (M15's sheet is as wide as its content, within 240 to 360 dp: the content fills it
                            // here — the C++'s sheet was always 360 dp wide)
                            ModalDrawerSheet(
                                modifier = Modifier.test_tag("sheet"),
                                content = || Text("Gelen kutusu", modifier = Modifier.fill_max_width()),
                            )
                        },
                        drawer_state = Some(state),
                        content = || {
                            let s = remember_swipe_to_dismiss_box_state();
                            Column(
                                modifier = Modifier.padding_sides_with(0.dp(), 300.dp(), 0.dp(), 0.dp()),
                                content = move || {
                                    SwipeToDismissBox(
                                        s.clone(),
                                        || Box(modifier = Modifier.fill_max_size()),
                                        content = || {
                                            Box(modifier = Modifier.fill_max_width().height(60.dp()).test_tag("satır"))
                                        },
                                    );
                                },
                            );
                        },
                    );
                });
            }),
            IntSize::new(400, 800),
        )
    };
    // The drawer is at the right: dragged open to the left.
    swipe(&rule, Offset::new(380.0, 100.0), Offset::new(100.0, 100.0), 12);
    rule.wait_for_idle();
    let sheet = bounds(&rule, "sheet");
    let state = get(&drawer);
    // 400 wide: a 360 sheet at the right.
    assert!(state.is_open() && near(sheet.right, 400.0) && near(sheet.left, 40.0), "{sheet:?}");
    swipe(&rule, Offset::new(100.0, 100.0), Offset::new(380.0, 100.0), 12); // and shut to the right
    rule.wait_for_idle();
    assert!(!state.is_open());
}

// ---- Compose's behaviour -----------------------------------------------------------------------------------------

fn drawer_states_move_from_code_and_save_their_value() {
    let confirm = Rc::new(Cell::new(true));
    let asked = Rc::new(RefCell::new(Vec::<DrawerValue>::new()));
    let (drawer, scope) = (slot::<DrawerState>(), slot::<CoroutineScope>());
    let rule = {
        let (confirm, asked, drawer, scope) = (confirm.clone(), asked.clone(), drawer.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (confirm, asked) = (confirm.clone(), asked.clone());
                let state = remember_drawer_state(DrawerValue::Closed, move |value| {
                    asked.borrow_mut().push(value);
                    confirm.get()
                });
                *drawer.borrow_mut() = Some(state.clone());
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                ModalNavigationDrawer(
                    || ModalDrawerSheet(content = || Text("Menü", modifier = Modifier.fill_max_width())),
                    drawer_state = Some(state),
                    content = || {},
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let s = get(&drawer);
    assert!(s.is_closed() && s.current_value() == DrawerValue::Closed && s.target_value() == DrawerValue::Closed);
    assert!(near(s.current_offset(), -360.0) && near(s.offset().value(), -360.0));
    // Opened: the target at once, the value once it is there.
    launch(&scope, s.open());
    rule.advance_time_by(32);
    assert!(
        s.is_animation_running() && s.target_value() == DrawerValue::Open && s.current_value() == DrawerValue::Closed
    );
    let part = s.current_offset();
    assert!(part > -360.0 && part < 0.0, "{part}");
    rule.wait_for_idle();
    assert!(s.is_open() && !s.is_animation_running() && near(s.current_offset(), 0.0));
    // The deprecated animateTo by a spec of one's own; snapTo at once.
    launch(
        &scope,
        s.animate_to(DrawerValue::Closed, {
            let spec: kompoz_animation::TweenSpec<f32> = named_args!(kompoz_animation::tween(duration_millis = 100));
            Arc::new(spec)
        }),
    );
    rule.advance_time_by(48);
    assert!(s.current_offset() < 0.0 && s.current_offset() > -360.0);
    rule.wait_for_idle();
    assert!(s.is_closed());
    launch(&scope, s.snap_to(DrawerValue::Open));
    rule.advance_frame();
    assert!(s.is_open() && near(s.current_offset(), 0.0));
    // Vetoed: a drag back to closed settles open again.
    confirm.set(false);
    swipe(&rule, Offset::new(300.0, 400.0), Offset::new(20.0, 400.0), 12);
    rule.wait_for_idle();
    assert!(s.is_open() && asked.borrow().contains(&DrawerValue::Closed));
    // The scrim tapped asks too: not closed.
    tap(&rule, Offset::new(380.0, 400.0));
    rule.wait_for_idle();
    assert!(s.is_open());
    confirm.set(true);
    // Saved and restored by its value.
    let saver = DrawerState::Saver(|_| true);
    let saved = saver.save(&s);
    let restored = saver.restore(&saved).expect("restored");
    assert!(restored.is_open() && restored != s && restored.clone() == restored);
    assert!(DrawerState(DrawerValue::Closed).is_closed());
    assert_eq!(DrawerValue::ENTRIES, [DrawerValue::Closed, DrawerValue::Open]);
    assert_eq!(DrawerValue::Open.to_string(), "Open");
    // Not laid out: its offset is NaN.
    assert!(DrawerState(DrawerValue::Open).current_offset().is_nan());
}

fn drawers_take_the_keyboard_focus_and_close_by_escape() {
    let modes: Slot<InputModeManager> = slot();
    let (drawer, scope) = (slot::<DrawerState>(), slot::<CoroutineScope>());
    let rule = {
        let (modes, drawer, scope) = (modes.clone(), drawer.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *modes.borrow_mut() = Some(LocalInputModeManager.current());
                let state = remember_drawer_state(DrawerValue::Closed);
                *drawer.borrow_mut() = Some(state.clone());
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                ModalNavigationDrawer(
                    || {
                        ModalDrawerSheet(
                            content = || {
                                NavigationDrawerItem(
                                    || Text("Birinci"),
                                    true,
                                    on_click = || {},
                                    modifier = Modifier.test_tag("first"),
                                );
                                NavigationDrawerItem(|| Text("İkinci"), false, on_click = || {});
                            },
                        )
                    },
                    drawer_state = Some(state),
                    content = || {},
                );
            }),
            IntSize::new(400, 800),
        )
    };
    rule.run_on_idle(|| get(&modes).request_input_mode(InputMode::Keyboard));
    let s = get(&drawer);
    launch(&scope, s.open());
    rule.wait_for_idle();
    // Keyboard focus goes to the first element of the drawer when it opens.
    rule.on_node_with_tag("first").assert_is_focused();
    // Escape (as it goes up) closes it — a plain sheet has no back handler: Escape going down as back does nothing.
    assert!(!escape(&rule));
    assert!(rule.host().key_up(Key::Escape, KeyModifiers::default()));
    rule.wait_for_idle();
    assert!(s.is_closed());
}

fn drawers_are_dismissed_by_accessibility() {
    let (drawer, scope) = (slot::<DrawerState>(), slot::<CoroutineScope>());
    let rule = {
        let (drawer, scope) = (drawer.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let state = remember_drawer_state(DrawerValue::Closed);
                *drawer.borrow_mut() = Some(state.clone());
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                Column(
                    content = move || {
                        ModalNavigationDrawer(
                            || ModalDrawerSheet(content = || Text("Menü")),
                            modifier = Modifier.height(300.dp()),
                            drawer_state = Some(state.clone()),
                            content = || {},
                        );
                        Box(
                            modifier = Modifier.height(300.dp()),
                            content = || {
                                PermanentNavigationDrawer(
                                    || PermanentDrawerSheet(content = || Text("Kalıcı")),
                                    content = || {},
                                );
                            },
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let s = get(&drawer);
    // The panes told by their titles (both: the closed modal one composed, not placed); the modal one dismissible
    // only while open, and its scrim told (clickable) then.
    let titled = |rule: &KompozTestRule| {
        rule.host().accessibility_tree().iter().filter(|n| n.pane_title.as_deref() == Some("Navigation menu")).count()
    };
    let dismissible_pane = || {
        SemanticsMatcher::new("a dismissible navigation menu", |n| {
            n.config().get(SemanticsProperties::PaneTitle).as_deref() == Some("Navigation menu")
                && n.config().contains(SemanticsActions::Dismiss)
        })
    };
    assert_eq!(titled(&rule), 2);
    assert!(rule.on_all_nodes(dismissible_pane()).fetch_semantics_nodes().is_empty());
    rule.on_node_with_content_description("Close navigation menu").assert_does_not_exist();
    launch(&scope, s.open());
    rule.wait_for_idle();
    // Open: the scrim says what it does, and the pane can be dismissed.
    rule.on_node_with_content_description("Close navigation menu").assert(kompoz_test::has_click_action());
    let dismissible_pane = dismissible_pane();
    rule.on_node(dismissible_pane).perform_semantics_action(SemanticsActions::Dismiss);
    rule.wait_for_idle();
    assert!(s.is_closed());
}

fn a_bouncy_opening_stretches_the_sheet_and_keeps_its_content() {
    // Past the open anchor (an overshooting motion), the sheet is stretched across from the window's edge and its
    // content scaled back to keep its shape.
    let (drawer, scope) = (slot::<DrawerState>(), slot::<CoroutineScope>());
    let rule = {
        let (drawer, scope) = (drawer.clone(), scope.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let state = remember_drawer_state(DrawerValue::Closed);
                *drawer.borrow_mut() = Some(state.clone());
                *scope.borrow_mut() = Some(remember_coroutine_scope());
                let sheet_state = state.clone();
                ModalNavigationDrawer(
                    move || {
                        ModalDrawerSheetWithDrawerState(
                            sheet_state.clone(),
                            content = || Text("Menü", modifier = Modifier.fill_max_width()),
                        )
                    },
                    drawer_state = Some(state),
                    content = || {},
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let s = get(&drawer);
    launch(
        &scope,
        s.animate_to(DrawerValue::Open, {
            let spec: kompoz_animation::SpringSpec<f32> =
                named_args!(kompoz_animation::spring(damping_ratio = 0.2, stiffness = 200.0));
            Arc::new(spec)
        }),
    );
    let mut widest = (1.0f32, 1.0f32, f32::NAN);
    for _ in 0..60 {
        rule.advance_frame();
        let canvas = rule.draw();
        for (index, m) in canvas.transforms.iter().enumerate() {
            let sx = m.get(0, 0);
            if sx > widest.0 {
                // The content's transform follows the sheet's.
                let content = canvas.transforms.get(index + 1).map_or(1.0, |m| m.get(0, 0));
                widest = (sx, content, m.get(3, 0) / (1.0 - sx));
            }
        }
    }
    let (sheet_x, content_x, pivot_x) = widest;
    assert!(sheet_x > 1.0, "{widest:?}");
    assert!(near_within(content_x, 1.0 / sheet_x, 0.001), "{widest:?}");
    // About the sheet's right edge (the window's edge is where it stretches from, its left edge going past it).
    assert!(near_within(pivot_x, 360.0, 0.05), "{widest:?}");
    rule.wait_for_idle();
    assert!(s.is_open());
}

fn navigation_drawer_items_take_their_colors_icon_and_badge() {
    let selected = mutable_state_of(false);
    let rule = {
        let selected = selected.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let colors = named_args!(NavigationDrawerItemDefaults.colors(
                selected_container_color = Color::Red,
                unselected_container_color = Color::Yellow,
                selected_icon_color = Color::Blue,
                unselected_icon_color = Color::Cyan,
                selected_text_color = Color::Green,
                unselected_text_color = Color::Magenta,
                unselected_badge_color = Some(Color::Gray),
            ));
            Column(
                modifier = Modifier.width(300.dp()),
                content = || {
                    NavigationDrawerItem(
                        || Text("Etiket", modifier = Modifier.test_tag("label")),
                        selected.get(),
                        on_click = || {},
                        icon = some(|| Box(modifier = Modifier.size(24.dp()).test_tag("icon"))),
                        badge = some(|| Text("24", modifier = Modifier.test_tag("badge"))),
                        shape = Arc::new(RectangleShape),
                        colors = colors.clone(),
                        modifier = Modifier.test_tag("item"),
                    );
                },
            );
        }))
    };
    // The icon at 16 dp, the label 12 after it filling the room, the badge 12 before the end's 24.
    let (item, icon, label, badge) =
        (bounds(&rule, "item"), unmerged(&rule, "icon"), unmerged(&rule, "label"), unmerged(&rule, "badge"));
    assert!(near(icon.left, 16.0) && near(label.left, 16.0 + 24.0 + 12.0), "{icon:?} {label:?}");
    assert!(near(badge.right, item.right - 24.0), "{badge:?} {item:?}");
    assert!(near(icon.center().y, item.center().y));
    let colors_of = |rule: &KompozTestRule| {
        let canvas = rule.draw();
        (canvas.rect_colors.clone(), canvas.colors.clone())
    };
    let (rects, texts) = colors_of(&rule);
    assert!(
        rects.contains(&Color::Yellow) && texts.contains(&Color::Magenta) && texts.contains(&Color::Gray),
        "{rects:?} {texts:?}"
    );
    selected.set(true);
    rule.advance_frame();
    let (rects, texts) = colors_of(&rule);
    // (the selected badge in the selected text's color: Kotlin's default)
    assert!(
        rects.contains(&Color::Red) && texts.iter().filter(|c| **c == Color::Green).count() == 2,
        "{rects:?} {texts:?}"
    );
}

type Seen = (
    Color,
    Color,
    Color,
    Color,
    Arc<dyn Shape>,
    Rc<dyn NavigationDrawerItemColors>,
    Rc<dyn NavigationDrawerItemColors>,
);

/// Colors of one's own (Compose's interface implemented): black everywhere.
struct OwnColors;

impl NavigationDrawerItemColors for OwnColors {
    fn icon_color(&self, _selected: bool) -> State<Color> {
        State::from(mutable_state_of(Color::Black))
    }
    fn text_color(&self, _selected: bool) -> State<Color> {
        State::from(mutable_state_of(Color::Black))
    }
    fn badge_color(&self, _selected: bool) -> State<Color> {
        State::from(mutable_state_of(Color::Black))
    }
    fn container_color(&self, _selected: bool) -> State<Color> {
        State::from(mutable_state_of(Color::Black))
    }
}

fn drawer_defaults_follow_the_tokens() {
    let cs = light_color_scheme();
    let seen: Rc<RefCell<Option<Seen>>> = Rc::new(RefCell::new(None));
    let seen_again = |seen: &Rc<RefCell<Option<Seen>>>| {
        let colors = seen.borrow().as_ref().unwrap().6.clone();
        (colors, Rc::new(OwnColors) as Rc<dyn NavigationDrawerItemColors>)
    };
    let rule = {
        let seen = seen.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            *seen.borrow_mut() = Some((
                DrawerDefaults::scrim_color(),
                DrawerDefaults::standard_container_color(),
                DrawerDefaults::modal_container_color(),
                DrawerDefaults::container_color(),
                DrawerDefaults::shape(),
                NavigationDrawerItemDefaults.colors(),
                NavigationDrawerItemDefaults.colors(),
            ));
        }))
    };
    rule.wait_for_idle();
    let (scrim, standard, modal, deprecated, shape, colors, _) = seen.borrow().clone().unwrap();
    assert_eq!(scrim, named_args!(cs.scrim.copy(alpha = 0.32)));
    assert!(standard == cs.surface && deprecated == cs.surface && modal == cs.surface_container_low);
    // (the theme's large shape at the end only)
    assert!(shape == RoundedCornerShape::new(0.dp(), 16.dp(), 16.dp(), 0.dp()));
    assert_eq!(DrawerDefaults::ModalDrawerElevation, 0.dp());
    assert_eq!(DrawerDefaults::PermanentDrawerElevation, 0.dp());
    assert_eq!(DrawerDefaults::DismissibleDrawerElevation, 0.dp());
    assert_eq!(DrawerDefaults::MaximumDrawerWidth, 360.dp());
    assert!(NavigationDrawerItemDefaults::ItemPadding() == PaddingValues::hv(12.dp(), 0.dp()));
    // Material's colors: equal by their colors (Kotlin's data-like equals), another kind by identity.
    let (again, other) = seen_again(&seen);
    let other_again = other.clone();
    assert!(*colors == *again && !Rc::ptr_eq(&colors, &again) && *colors != *other && *other == *other_again);
    assert!(*other != *(Rc::new(OwnColors) as Rc<dyn NavigationDrawerItemColors>));
}

// ---- M15's DrawerSamples -----------------------------------------------------------------------------------------

/// The samples' destinations (icons to mimic drawer destinations).
fn sample_items() -> Vec<ImageVector> {
    use Icons::Default::*;
    vec![
        AccountCircle(),
        Bookmarks(),
        CalendarMonth(),
        Dashboard(),
        Email(),
        Favorite(),
        Group(),
        Headphones(),
        Image(),
        JoinFull(),
        Keyboard(),
        Laptop(),
        Map(),
        Navigation(),
        Outbox(),
        PushPin(),
        QrCode(),
        Radio(),
    ]
}

/// The name an item shows (Kotlin's `item.name.substringAfterLast(".")`).
fn item_name(item: &ImageVector) -> String {
    item.name().rsplit('.').next().unwrap_or_default().to_string()
}

#[kompozable]
fn ModalNavigationDrawerSample() {
    let drawer_state = remember_drawer_state(DrawerValue::Closed);
    let scope = remember_coroutine_scope();
    let focus_requester = remember(FocusRequester::new);
    // icons to mimic drawer destinations
    let items = sample_items();
    let selected_item = remember(|| mutable_state_of(items[0].clone()));
    let (sheet_state, items_state, item_scope) = (drawer_state.clone(), drawer_state.clone(), scope.clone());
    let (content_state, content_scope, button_focus) = (drawer_state.clone(), scope.clone(), focus_requester.clone());
    ModalNavigationDrawer(
        move || {
            let (items, selected_item, items_state, item_scope) =
                (items.clone(), selected_item.clone(), items_state.clone(), item_scope.clone());
            ModalDrawerSheetWithDrawerState(
                sheet_state.clone(),
                content = move || {
                    let (items, selected_item, items_state, item_scope) =
                        (items.clone(), selected_item.clone(), items_state.clone(), item_scope.clone());
                    Column(
                        modifier = Modifier.vertical_scroll(&remember_scroll_state()),
                        content = move || {
                            Spacer(Modifier.height(12.dp()));
                            for item in &items {
                                let (icon, name) = (item.clone(), item_name(item));
                                let (selected_item, drawer_state, scope, item) =
                                    (selected_item.clone(), items_state.clone(), item_scope.clone(), item.clone());
                                NavigationDrawerItem(
                                    move || Text(name.clone()),
                                    item == selected_item.get(),
                                    on_click = move || {
                                        let drawer_state = drawer_state.clone();
                                        named_args!(scope.launch(async move { drawer_state.close().await }));
                                        selected_item.set(item.clone());
                                    },
                                    icon = some(move || VectorIcon(icon.clone(), None)),
                                    modifier = Modifier.padding_values(NavigationDrawerItemDefaults::ItemPadding()),
                                );
                            }
                        },
                    );
                },
            );
        },
        drawer_state = Some(drawer_state.clone()),
        content = move || {
            let (content_state, content_scope, button_focus) =
                (content_state.clone(), content_scope.clone(), button_focus.clone());
            Column(
                modifier = Modifier.fill_max_size().padding(16.dp()),
                horizontal_alignment = Alignment::CenterHorizontally,
                content = move || {
                    Text(if content_state.is_closed() { ">>> Swipe >>>" } else { "<<< Swipe <<<" });
                    Spacer(Modifier.height(20.dp()));
                    let (drawer_state, scope) = (content_state.clone(), content_scope.clone());
                    Button(
                        move || {
                            let drawer_state = drawer_state.clone();
                            named_args!(scope.launch(async move { drawer_state.open().await }));
                        },
                        modifier = Modifier.focus_requester(&button_focus),
                        content = || Text("Click to open"),
                    );
                },
            );
        },
    );

    let is_closed = drawer_state.is_closed();
    LaunchedEffect(is_closed, async move {
        if is_closed {
            // Keyboard focus should go back to button once drawer closes.
            named_args!(focus_requester.request_focus());
        }
    });
}

fn modal_navigation_drawer_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| MaterialTheme(content = || ModalNavigationDrawerSample())),
        IntSize::new(400, 800),
    );
    rule.on_node_with_text(">>> Swipe >>>").assert_exists();
    rule.on_node_with_text("Email").assert_is_not_displayed();
    rule.on_node_with_text("Click to open").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("<<< Swipe <<<").assert_exists();
    rule.on_node_with_text("AccountCircle").assert_exists();
    // The first item selected; clicking another selects it and closes the drawer.
    rule.on_node_with_text("Email").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(">>> Swipe >>>").assert_exists();
    rule.on_node_with_text("Email").assert_is_not_displayed();
    // Swiped open again: Email is the one selected.
    swipe(&rule, Offset::new(20.0, 400.0), Offset::new(300.0, 400.0), 12);
    rule.wait_for_idle();
    rule.on_node_with_text("Email").assert_is_selected();
    rule.on_node_with_text("AccountCircle").assert_is_not_selected();
    // Its back handler: back closes it.
    assert!(escape(&rule));
    rule.wait_for_idle();
    rule.on_node_with_text(">>> Swipe >>>").assert_exists();
}

#[kompozable]
fn PermanentNavigationDrawerSample() {
    // icons to mimic drawer destinations
    let items = sample_items();
    let selected_item = remember(|| mutable_state_of(items[0].clone()));
    PermanentNavigationDrawer(
        move || {
            let (items, selected_item) = (items.clone(), selected_item.clone());
            PermanentDrawerSheet(
                modifier = Modifier.width(240.dp()),
                content = move || {
                    let (items, selected_item) = (items.clone(), selected_item.clone());
                    Column(
                        modifier = Modifier.vertical_scroll(&remember_scroll_state()),
                        content = move || {
                            Spacer(Modifier.height(12.dp()));
                            for item in &items {
                                let (icon, name, selected_item, item) =
                                    (item.clone(), item_name(item), selected_item.clone(), item.clone());
                                NavigationDrawerItem(
                                    move || Text(name.clone()),
                                    item == selected_item.get(),
                                    on_click = move || selected_item.set(item.clone()),
                                    icon = some(move || VectorIcon(icon.clone(), None)),
                                    modifier = Modifier.padding_hv(horizontal = 12.dp()),
                                );
                            }
                        },
                    );
                },
            );
        },
        content = || {
            Column(
                modifier = Modifier.fill_max_size().padding(16.dp()),
                horizontal_alignment = Alignment::CenterHorizontally,
                content = || Text("Application content"),
            );
        },
    );
}

fn permanent_navigation_drawer_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| MaterialTheme(content = || PermanentNavigationDrawerSample())),
        IntSize::new(800, 600),
    );
    rule.on_node_with_text("Application content").assert_exists();
    // The content beside the 240 dp sheet, centered in the rest.
    let content = text_bounds(&rule, "Application content");
    assert!(near(content.center().x, 240.0 + (800.0 - 240.0) / 2.0), "{content:?}");
    rule.on_node_with_text("AccountCircle").assert_is_selected();
    rule.on_node_with_text("Dashboard").perform_click();
    rule.on_node_with_text("Dashboard").assert_is_selected();
    rule.on_node_with_text("AccountCircle").assert_is_not_selected();
    // The items 12 dp in from the sheet's sides.
    let item = rule
        .on_node(kompoz_test::has_text("Dashboard", false, false).and(kompoz_test::has_role(Role::Tab)))
        .get_unclipped_bounds_in_root();
    assert!(near(item.left, 12.0) && near(item.width(), 240.0 - 24.0), "{item:?}");
}

#[kompozable]
fn DismissibleNavigationDrawerSample() {
    let drawer_state = remember_drawer_state(DrawerValue::Closed);
    let scope = remember_coroutine_scope();
    let focus_requester = remember(FocusRequester::new);
    // icons to mimic drawer destinations
    let items = sample_items();
    let selected_item = remember(|| mutable_state_of(items[0].clone()));

    let (sheet_state, items_state, item_scope) = (drawer_state.clone(), drawer_state.clone(), scope.clone());
    let (content_state, content_scope, button_focus) = (drawer_state.clone(), scope.clone(), focus_requester.clone());
    DismissibleNavigationDrawer(
        move || {
            let (items, selected_item, items_state, item_scope) =
                (items.clone(), selected_item.clone(), items_state.clone(), item_scope.clone());
            DismissibleDrawerSheetWithDrawerState(
                sheet_state.clone(),
                content = move || {
                    let (items, selected_item, items_state, item_scope) =
                        (items.clone(), selected_item.clone(), items_state.clone(), item_scope.clone());
                    Column(
                        modifier = Modifier.vertical_scroll(&remember_scroll_state()),
                        content = move || {
                            Spacer(Modifier.height(12.dp()));
                            for item in &items {
                                let (icon, name) = (item.clone(), item_name(item));
                                let (selected_item, drawer_state, scope, item) =
                                    (selected_item.clone(), items_state.clone(), item_scope.clone(), item.clone());
                                NavigationDrawerItem(
                                    move || Text(name.clone()),
                                    item == selected_item.get(),
                                    on_click = move || {
                                        let drawer_state = drawer_state.clone();
                                        named_args!(scope.launch(async move { drawer_state.close().await }));
                                        selected_item.set(item.clone());
                                    },
                                    icon = some(move || VectorIcon(icon.clone(), None)),
                                    modifier = Modifier.padding_hv(horizontal = 12.dp()),
                                );
                            }
                        },
                    );
                },
            );
        },
        drawer_state = Some(drawer_state.clone()),
        content = move || {
            let (content_state, content_scope, button_focus) =
                (content_state.clone(), content_scope.clone(), button_focus.clone());
            Column(
                modifier = Modifier.fill_max_size().padding(16.dp()),
                horizontal_alignment = Alignment::CenterHorizontally,
                content = move || {
                    Text(if content_state.is_closed() { ">>> Swipe >>>" } else { "<<< Swipe <<<" });
                    Spacer(Modifier.height(20.dp()));
                    let (drawer_state, scope) = (content_state.clone(), content_scope.clone());
                    Button(
                        move || {
                            let drawer_state = drawer_state.clone();
                            named_args!(scope.launch(async move { drawer_state.open().await }));
                        },
                        modifier = Modifier.focus_requester(&button_focus),
                        content = || Text("Click to open"),
                    );
                },
            );
        },
    );

    let is_closed = drawer_state.is_closed();
    LaunchedEffect(is_closed, async move {
        if is_closed {
            // Keyboard focus should go back to button once drawer closes.
            named_args!(focus_requester.request_focus());
        }
    });
}

fn dismissible_navigation_drawer_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| MaterialTheme(content = || DismissibleNavigationDrawerSample())),
        IntSize::new(800, 600),
    );
    let button = text_bounds(&rule, "Click to open");
    rule.on_node_with_text("Click to open").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("<<< Swipe <<<").assert_exists();
    // The content pushed aside by the sheet: as wide as it gets (360 dp).
    let pushed = text_bounds(&rule, "Click to open");
    assert!(near(pushed.left - button.left, 360.0), "{button:?} {pushed:?}");
    rule.on_node_with_text("Favorite").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(">>> Swipe >>>").assert_exists();
    assert!(near(text_bounds(&rule, "Click to open").left, button.left));
    swipe(&rule, Offset::new(20.0, 300.0), Offset::new(400.0, 300.0), 12);
    rule.wait_for_idle();
    rule.on_node_with_text("Favorite").assert_is_selected();
}

//! Material's bottom sheets as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's sheet tests by name
//! (kompoz_tests.cpp; M15's values where the C++ differed, said at each), Compose's behaviour (the sheet's
//! anchors, its state's values and errors, the defaults, the drag handle's semantics, the scaffold's layout, the
//! vertical drag handle), and the M15 samples of BottomSheetSamples.kt and DragHandleSamples.kt.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_coroutines::{CoroutineScope, LaunchedEffect, remember_coroutine_scope};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::semantics::SemanticsActions;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    bottom_sheet_slides_in_and_closes_by_scrim_drag_and_back,
    modal_bottom_sheet_state_opens_half_way_and_hides_from_code,
    modal_bottom_sheet_moves_as_compose_1_4,
    modal_bottom_sheet_follows_the_back_gesture,
    half_open_sheet_expands_before_its_list_scrolls,
    bottom_sheet_moves_with_what_its_list_leaves,
    scrims_take_the_schemes_scrim_color,
    bottom_sheet_scaffold_peeks_expands_and_follows_drags,
    a_dragged_bottom_sheet_moves_without_recomposing_the_scaffold,
    bottom_sheet_scaffold_moves_before_the_list_in_it_scrolls,
    bottom_sheet_scaffold_settles_with_the_flings_speed,
    bottom_sheet_scaffold_keeps_clear_of_the_system_bars_once,
    a_bottom_sheet_scaffolds_padding_follows_the_sheet_without_recomposing,
    sheet_states_start_where_told_and_refuse_what_they_skip,
    sheet_states_save_their_value,
    bottom_sheet_defaults_follow_the_tokens,
    drag_handles_expand_collapse_and_dismiss_the_sheet,
    sheets_are_at_most_their_maximum_width,
    vertical_drag_handles_grow_while_pressed,
    manual_modal_bottom_sheet_sample,
    simple_bottom_sheet_scaffold_sample,
    vertical_drag_handle_sample,
);

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn near_within(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

// A note on the drags below: Compose doesn't dispatch a move whose position in a pointer input node is where the
// last one was (ComposeUiFlags.isTriggerMoveEventsWhenLocationHasNotChangedEnabled, false by default). A sheet
// moved by whole pixels under a finger moving by the same whole pixels would so see every other move only (its
// drag node moves with it); real input doesn't move in whole steps, so the drags of something moving under the
// finger go by fractional steps here (the C++ moved by whole ones).

/// The C++ tests' `slowDrag`: down, moved over `frames` frames, held 200 ms, up.
fn slow_drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        let t = i as f32 / frames as f32;
        h.pointer_move(from + (to - from) * t, -1, 0);
    }
    rule.advance_time_by(200);
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

fn top_of(rule: &KompozTestRule, tag: &str) -> f32 {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root().top
}

// ---- ModalBottomSheet ----------------------------------------------------------------------------------------------

fn bottom_sheet_slides_in_and_closes_by_scrim_drag_and_back() {
    let open = mutable_state_of(false);
    let dismissed = Rc::new(Cell::new(0));
    let rule = {
        let (open, dismissed) = (open.clone(), dismissed.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            if open.get() {
                let (open, dismissed) = (open.clone(), dismissed.clone());
                ModalBottomSheet(
                    on_dismiss_request = move || {
                        dismissed.set(dismissed.get() + 1);
                        open.set(false);
                    },
                    content = || Text("Sayfa içeriği", modifier = Modifier.padding(16.dp()).test_tag("content")),
                );
            }
        }))
    };
    for way in 0..3 {
        open.set(true);
        rule.advance_time_by(32);
        let during = top_of(&rule, "content");
        rule.wait_for_idle();
        let settled = top_of(&rule, "content");
        assert!(settled < during, "slid up: {during} -> {settled}");
        assert!(settled > 600.0 && settled < 800.0, "resting at the bottom: {settled}");
        match way {
            0 => tap(&rule, Offset::new(200.0, 100.0)), // the scrim
            1 => swipe(&rule, Offset::new(200.0, settled), Offset::new(200.0, settled + 200.5), 4), // pulled down
            _ => assert!(escape(&rule)),                // back
        }
        rule.wait_for_idle();
        assert!(dismissed.get() == way + 1 && !open.get(), "way {way}: {}", dismissed.get());
        rule.on_node_with_tag("content").assert_does_not_exist();
    }
}

fn modal_bottom_sheet_state_opens_half_way_and_hides_from_code() {
    let open = mutable_state_of(true);
    let skip = mutable_state_of(false);
    let dismissed = Rc::new(Cell::new(0));
    let (state, skipping, scope) = (slot::<SheetState>(), slot::<SheetState>(), slot::<CoroutineScope>());
    let rule = {
        let (open, skip, dismissed, state, skipping, scope) =
            (open.clone(), skip.clone(), dismissed.clone(), state.clone(), skipping.clone(), scope.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let s = remember_modal_bottom_sheet_state();
            let k = remember_modal_bottom_sheet_state(skip_partially_expanded = true);
            *state.borrow_mut() = Some(s.clone());
            *skipping.borrow_mut() = Some(k.clone());
            *scope.borrow_mut() = Some(remember_coroutine_scope());
            if open.get() {
                let (open, dismissed) = (open.clone(), dismissed.clone());
                ModalBottomSheet(
                    on_dismiss_request = move || {
                        dismissed.set(dismissed.get() + 1);
                        open.set(false);
                    },
                    sheet_state = Some(if skip.get() { k } else { s }),
                    content = || Box(modifier = Modifier.fill_max_width().height(600.dp()).test_tag("içerik")),
                );
            }
        }))
    };
    rule.wait_for_idle();
    let s = get(&state);
    // 600 + its handle (48): 648 of the 800 it may take — over half: half way, at 400. (The C++'s sheet had a 28 dp
    // corner below its content and the window's insets around it: 676 of 752, at 300.)
    assert!(s.current_value() == SheetValue::PartiallyExpanded && s.is_visible() && s.has_partially_expanded_state());
    assert!(near(s.require_offset(), 400.0), "{}", s.require_offset());
    let half = top_of(&rule, "içerik");
    assert!(near(half, 448.0));
    // Up two thirds of the way: all the way up (to 800 - 648 = 152; the C++'s 300 up).
    slow_drag(&rule, Offset::new(200.0, half - 20.0), Offset::new(200.0, half - 220.5), 20);
    rule.wait_for_idle();
    assert!(s.current_value() == SheetValue::Expanded && near(top_of(&rule, "içerik"), half - 248.0));
    let ss = s.clone();
    launch(&scope, async move { ss.partial_expand().await });
    rule.wait_for_idle();
    assert!(s.current_value() == SheetValue::PartiallyExpanded && near(top_of(&rule, "içerik"), half));
    // Hidden from code and left there: not dismissed.
    let ss = s.clone();
    launch(&scope, async move { ss.hide().await });
    rule.wait_for_idle();
    assert!(!s.is_visible() && open.get() && dismissed.get() == 0);
    let ss = s.clone();
    launch(&scope, async move { ss.show().await }); // and back
    rule.wait_for_idle();
    assert!(s.current_value() == SheetValue::PartiallyExpanded && near(top_of(&rule, "içerik"), half));
    // From code: hidden, then the caller removes it.
    let (ss, o) = (s.clone(), open.clone());
    launch(&scope, async move {
        ss.hide().await;
        o.set(false);
    });
    rule.wait_for_idle();
    assert!(!s.is_visible() && !open.get() && dismissed.get() == 0);
    rule.on_node_with_tag("içerik").assert_does_not_exist();
    open.set(true); // again: it slides in again
    rule.advance_time_by(48);
    assert!(top_of(&rule, "içerik") > half);
    rule.wait_for_idle();
    assert!(s.current_value() == SheetValue::PartiallyExpanded && near(top_of(&rule, "içerik"), half));
    // Pulled down: hidden, onDismissRequest.
    slow_drag(&rule, Offset::new(200.0, half + 20.0), Offset::new(200.0, half + 260.5), 20);
    rule.wait_for_idle();
    assert!(dismissed.get() == 1 && !open.get(), "{}", dismissed.get());
    // skipPartiallyExpanded: all the way up at once.
    skip.set(true);
    open.set(true);
    rule.wait_for_idle();
    let k = get(&skipping);
    assert!(k.current_value() == SheetValue::Expanded && !k.has_partially_expanded_state());
    assert!(near(top_of(&rule, "içerik"), half - 248.0));
}

fn modal_bottom_sheet_moves_as_compose_1_4() {
    let (state, scope, cs) = (slot::<SheetState>(), slot::<CoroutineScope>(), slot::<ColorScheme>());
    let rule = {
        let (state, scope, cs) = (state.clone(), scope.clone(), cs.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            *cs.borrow_mut() = Some(MaterialTheme::color_scheme());
            let s = remember_modal_bottom_sheet_state(skip_partially_expanded = true);
            *state.borrow_mut() = Some(s.clone());
            *scope.borrow_mut() = Some(remember_coroutine_scope());
            ModalBottomSheet(
                on_dismiss_request = || {},
                sheet_state = Some(s),
                content = || Box(modifier = Modifier.fill_max_width().height(300.dp()).test_tag("içerik")),
            );
        }))
    };
    rule.wait_for_idle();
    let s = get(&state);
    let shown = s.require_offset();
    assert!(near(shown, 800.0 - 348.0), "{shown}");
    // Its handle in onSurfaceVariant, unfaded (a 32 × 4 rounded rect: the theme's extra large corners).
    let canvas = rule.draw();
    let on_surface_variant = get(&cs).on_surface_variant;
    let handle = canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .any(|(r, c)| near(r.width(), 32.0) && near(r.height(), 4.0) && *c == on_surface_variant);
    assert!(handle, "{:?} {:?}", canvas.rects, canvas.rect_colors);
    // Hidden on the fast effects spring: nearly all the way in 6 frames.
    let ss = s.clone();
    launch(&scope, async move { ss.hide().await });
    for _ in 0..6 {
        rule.advance_frame();
    }
    let hidden = s.require_offset();
    rule.wait_for_idle();
    let gone = s.require_offset();
    assert!((hidden - shown) / (gone - shown) > 0.9, "{shown} {hidden} {gone}");
    // Shown on the default spatial spring: about two thirds of the way in 6 frames.
    let ss = s.clone();
    launch(&scope, async move { ss.show().await });
    for _ in 0..6 {
        rule.advance_frame();
    }
    let coming = (gone - s.require_offset()) / (gone - shown);
    assert!(coming > 0.55 && coming < 0.75, "{coming}");
    rule.wait_for_idle();
}

/// The sheet's predictive back transform (narrower and lower), where it scales about (its pivot's y in the
/// sheet's own space), and its content's (scaled back along the height): the C++ test's `Scales`.
#[derive(Debug)]
struct Scales {
    sheet_x: f32,
    sheet_y: f32,
    pivot_y: f32,
    content_y: f32,
}

fn scales(rule: &KompozTestRule) -> Scales {
    rule.advance_frame();
    let canvas: RecordingCanvas = rule.draw();
    let mut out = Scales { sheet_x: 1.0, sheet_y: 1.0, pivot_y: f32::NAN, content_y: 1.0 };
    // (a scaled graphics layer is drawn through a transform: scaled about its pivot, in the layer's own space)
    for m in &canvas.transforms {
        let (sx, sy, ty) = (m.get(0, 0), m.get(1, 1), m.get(3, 1));
        if sx != 1.0 {
            out.sheet_x = sx;
            out.sheet_y = sy;
            out.pivot_y = if sy != 1.0 { ty / (1.0 - sy) } else { f32::NAN };
        } else if sy != 1.0 {
            out.content_y = sy;
        }
    }
    out
}

fn back_event(touch_x: f32, progress: f32) -> kompoz_ui::back::BackEventCompat {
    kompoz_ui::back::BackEventCompat { touch_x, touch_y: 400.0, progress, swipe_edge: kompoz_ui::back::SwipeEdge::Left }
}

fn modal_bottom_sheet_follows_the_back_gesture() {
    let open = mutable_state_of(true);
    let tall = mutable_state_of(false);
    let inner = mutable_state_of(false);
    let menu = mutable_state_of(false);
    let keep = Rc::new(Cell::new(false)); // onDismissRequest leaves it composed
    let (dismissed, inner_backs) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (state, scope) = (slot::<SheetState>(), slot::<CoroutineScope>());
    let field = FocusRequester::new();
    let f = field.clone();
    let rule = {
        let (open, tall, inner, menu, keep, dismissed, inner_backs, state, scope) = (
            open.clone(),
            tall.clone(),
            inner.clone(),
            menu.clone(),
            keep.clone(),
            dismissed.clone(),
            inner_backs.clone(),
            state.clone(),
            scope.clone(),
        );
        KompozTestRule::new(kompozable_lambda!(move || {
            let s = remember_modal_bottom_sheet_state();
            *state.borrow_mut() = Some(s.clone());
            *scope.borrow_mut() = Some(remember_coroutine_scope());
            if open.get() {
                let (open, keep, dismissed, inner_backs) =
                    (open.clone(), keep.clone(), dismissed.clone(), inner_backs.clone());
                let (inner, tall, menu, f) = (inner.clone(), tall.clone(), menu.clone(), f.clone());
                ModalBottomSheet(
                    on_dismiss_request = move || {
                        dismissed.set(dismissed.get() + 1);
                        open.set(keep.get());
                    },
                    sheet_state = Some(s),
                    content = move || {
                        let inner_backs = inner_backs.clone();
                        // One in the sheet comes first.
                        kompoz_ui::back::BackHandler(inner.get(), move || inner_backs.set(inner_backs.get() + 1));
                        let (f, menu) = (f.clone(), menu.clone());
                        Box(
                            modifier = Modifier.fill_max_width().height(if tall.get() { 600.dp() } else { 200.dp() }),
                            content = move || {
                                Box(modifier = Modifier.size(40.dp()).focus_requester(&f).focusable());
                                // (The C++ opened a DropdownMenu here, WP3's: its focusable popup dismissed by back is
                                // what this checks, so ui's Popup of those properties stands in for it)
                                if menu.get() {
                                    let m = menu.clone();
                                    kompoz_ui::window::Popup(
                                        on_dismiss_request = move || m.set(false),
                                        properties = kompoz_ui::window::PopupProperties {
                                            focusable: true,
                                            ..Default::default()
                                        },
                                        content = || Text("seçenek"),
                                    );
                                }
                            },
                        );
                    },
                );
            }
        }))
    };
    rule.wait_for_idle();
    let s = get(&state);
    assert_eq!(s.current_value(), SheetValue::Expanded);
    // 200 + its handle (48) high (the C++'s had a 28 dp corner below: 276), 400 wide: at most 48 dp narrower, 24
    // lower.
    let host = rule.host();
    host.back_started(back_event(0.0, 0.0));
    host.back_progressed(back_event(60.0, 0.5));
    let p =
        kompoz_animation::Easing::transform(&kompoz_animation::CubicBezierEasing::constant(0.1, 0.1, 0.0, 1.0), 0.5);
    let a = scales(&rule);
    assert!(near_within(a.sheet_x, 1.0 - 48.0 * p / 400.0, 0.001), "{a:?}");
    assert!(near_within(a.sheet_y, 1.0 - 24.0 * p / 248.0, 0.001), "{a:?}");
    // Compose scales the sheet about (its offset + its height) / its height of its own height: 552 + 248 = 800 down
    // from its top — its layer being inside the offset, at 552, that is 1352 down the window (the C++ scaled it
    // about the window's bottom, 800).
    assert!(near_within(a.pivot_y, 800.0, 0.05), "{a:?}");
    assert!(near_within(a.content_y, a.sheet_x / a.sheet_y, 0.001), "{a:?}");
    host.back_progressed(back_event(120.0, 1.0));
    assert!(near_within(scales(&rule).sheet_x, 1.0 - 48.0 / 400.0, 0.001));
    host.back_cancelled(); // grows back, stays
    rule.wait_for_idle();
    let b = scales(&rule);
    assert!(b.sheet_x == 1.0 && b.sheet_y == 1.0 && b.content_y == 1.0, "{b:?}");
    assert!(s.current_value() == SheetValue::Expanded && dismissed.get() == 0);
    // The focus in it: still the sheet's gesture. A menu open in it: back closes the menu.
    field.request_focus();
    rule.wait_for_idle();
    host.back_started(back_event(0.0, 0.0));
    host.back_progressed(back_event(60.0, 0.5));
    assert!(scales(&rule).sheet_x < 1.0);
    host.back_cancelled();
    rule.wait_for_idle();
    menu.set(true);
    rule.wait_for_idle();
    assert!(escape(&rule));
    rule.wait_for_idle();
    assert!(!menu.get() && s.current_value() == SheetValue::Expanded && dismissed.get() == 0);
    // Done: it hides as it is, then onDismissRequest.
    host.back_started(back_event(0.0, 0.0));
    host.back_progressed(back_event(60.0, 0.5));
    rule.wait_for_idle();
    assert!(host.back_invoked());
    rule.advance_time_by(48);
    assert!(near_within(scales(&rule).sheet_x, 1.0 - 48.0 * p / 400.0, 0.001));
    assert_eq!(s.target_value(), SheetValue::Hidden);
    rule.wait_for_idle();
    assert!(dismissed.get() == 1 && !open.get());
    // Expanded with a half way stop: back takes it there, grown back.
    tall.set(true);
    open.set(true);
    rule.wait_for_idle();
    let ss = s.clone();
    launch(&scope, async move { ss.expand().await });
    rule.wait_for_idle();
    assert_eq!(s.current_value(), SheetValue::Expanded);
    host.back_started(back_event(0.0, 0.0));
    host.back_progressed(back_event(60.0, 0.5));
    assert!(scales(&rule).sheet_x < 1.0);
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert!(s.current_value() == SheetValue::PartiallyExpanded && dismissed.get() == 1);
    assert_eq!(scales(&rule).sheet_x, 1.0);
    // A back handler in the sheet: back goes to it, the sheet stays.
    inner.set(true);
    rule.wait_for_idle();
    assert!(escape(&rule));
    rule.wait_for_idle();
    assert!(inner_backs.get() == 1 && s.current_value() == SheetValue::PartiallyExpanded);
    inner.set(false);
    rule.wait_for_idle();
    assert!(escape(&rule)); // half way: hidden
    rule.wait_for_idle();
    assert!(dismissed.get() == 2 && inner_backs.get() == 1);
    // Hidden by a gesture and left in the composition: shown again, it keeps the gesture's scale — Compose's sheet
    // takes its predictive back progress back to 0 only on a called-off gesture or a half way stop, not when it
    // hides (the C++ made it whole again).
    keep.set(true);
    open.set(true);
    rule.wait_for_idle();
    host.back_started(back_event(0.0, 0.0));
    host.back_progressed(back_event(60.0, 0.5));
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert!(dismissed.get() == 3 && open.get() && !s.is_visible());
    let ss = s.clone();
    launch(&scope, async move { ss.show().await });
    rule.wait_for_idle();
    assert!(s.is_visible() && near_within(scales(&rule).sheet_x, 1.0 - 48.0 * p / 400.0, 0.001));
    // Hidden from code and kept: the sheet takes no back while hidden, the dialog's own dismissal does (it hides
    // the sheet and calls onDismissRequest); shown again, the earlier scale is still there.
    let ss = s.clone();
    launch(&scope, async move { ss.hide().await });
    rule.wait_for_idle();
    host.back_started(back_event(0.0, 0.0));
    host.back_progressed(back_event(60.0, 0.5));
    host.back_invoked();
    rule.wait_for_idle();
    assert_eq!(dismissed.get(), 4);
    let ss = s.clone();
    launch(&scope, async move { ss.show().await });
    rule.wait_for_idle();
    assert!(s.is_visible() && near_within(scales(&rule).sheet_x, 1.0 - 48.0 * p / 400.0, 0.001));
}

fn half_open_sheet_expands_before_its_list_scrolls() {
    let list = LazyListState::new(0, 0);
    let state = slot::<SheetState>();
    let l = list.clone();
    let rule = {
        let state = state.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let s = remember_modal_bottom_sheet_state();
            *state.borrow_mut() = Some(s.clone());
            let l = l.clone();
            ModalBottomSheet(
                on_dismiss_request = || {},
                sheet_state = Some(s),
                content = move || {
                    LazyColumn(
                        modifier = Modifier.fill_max_width().height(600.dp()).test_tag("liste"),
                        state = Some(l.clone()),
                        content = |this: &mut LazyListScope| {
                            this.items(
                                60,
                                item_content = |i| Text(format!("Öğe {i}"), modifier = Modifier.height(48.dp())),
                            );
                        },
                    );
                },
            );
        }))
    };
    rule.wait_for_idle();
    assert_eq!(get(&state).current_value(), SheetValue::PartiallyExpanded);
    let half = top_of(&rule, "liste");
    // Pushed up on the list, past half way: the sheet rises, the list stays.
    slow_drag(&rule, Offset::new(200.0, half + 100.0), Offset::new(200.0, half - 120.5), 20);
    assert!(list.first_visible_item_index() == 0 && list.first_visible_item_scroll_offset() == 0);
    assert!(top_of(&rule, "liste") < half - 100.0);
    rule.wait_for_idle();
    assert_eq!(get(&state).current_value(), SheetValue::Expanded);
    let top = top_of(&rule, "liste");
    // All the way up: now the list scrolls.
    slow_drag(&rule, Offset::new(200.0, top + 300.0), Offset::new(200.0, top + 100.0), 20);
    assert!(list.first_visible_item_index() >= 3 && near(top_of(&rule, "liste"), top));
}

fn bottom_sheet_moves_with_what_its_list_leaves() {
    let open = mutable_state_of(true);
    let list = LazyListState::new(0, 0);
    let l = list.clone();
    let rule = {
        let open = open.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            if open.get() {
                let (open, l) = (open.clone(), l.clone());
                ModalBottomSheet(
                    on_dismiss_request = move || open.set(false),
                    content = move || {
                        LazyColumn(
                            modifier = Modifier.fill_max_width().height(300.dp()).test_tag("list"),
                            state = Some(l.clone()),
                            content = |this: &mut LazyListScope| {
                                this.items(
                                    50,
                                    item_content = |i| Text(format!("Öğe {i}"), modifier = Modifier.height(48.dp())),
                                );
                            },
                        );
                    },
                );
            }
        }))
    };
    rule.wait_for_idle();
    let at = rule.on_node_with_tag("list").get_unclipped_bounds_in_root();
    // Up: the list scrolls, the sheet stays.
    slow_drag(&rule, Offset::new(200.0, at.top + 250.0), Offset::new(200.0, at.top + 50.0), 20);
    assert!(list.first_visible_item_index() >= 3 && near(top_of(&rule, "list"), at.top));
    // Down a little more than that: the list goes back to its top, the sheet follows the rest while held and springs
    // back when let go.
    let h = rule.host();
    h.pointer_down(Offset::new(200.0, at.top + 50.0), -1, 0, PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(Offset::new(200.0, at.top + 50.0 + 12.5 * i as f32), -1, 0);
    }
    assert!(list.first_visible_item_index() == 0 && list.first_visible_item_scroll_offset() == 0);
    let pulled = top_of(&rule, "list") - at.top;
    assert!(pulled > 30.0 && pulled < 100.0, "{pulled}");
    rule.advance_time_by(200);
    h.pointer_up(Offset::new(200.0, at.top + 300.0), -1, 0);
    rule.wait_for_idle();
    assert!(open.get() && near(top_of(&rule, "list"), at.top));
    // Pulled far: it closes.
    slow_drag(&rule, Offset::new(200.0, at.top + 20.0), Offset::new(200.0, at.top + 280.5), 20);
    rule.wait_for_idle();
    assert!(!open.get());
}

fn scrims_take_the_schemes_scrim_color() {
    // WP0's theme.rs test checks the Scrim composable; the C++'s composed a ModalBottomSheet: its scrim, in the
    // scheme's scrim color at 32% (BottomSheetDefaults.ScrimColor), across the window.
    let mut scheme = light_color_scheme();
    scheme.scrim = Color::hex(0xFF0000);
    let scrim_color = slot::<Color>();
    let seen = scrim_color.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let seen = seen.clone();
        MaterialTheme(
            color_scheme = scheme,
            content = move || {
                *seen.borrow_mut() = Some(BottomSheetDefaults::ScrimColor());
                ModalBottomSheet(
                    on_dismiss_request = || {},
                    content = || Box(modifier = Modifier.fill_max_width().height(200.dp())),
                );
            },
        );
    }));
    rule.wait_for_idle();
    let canvas = rule.draw();
    let red = canvas.rects.iter().zip(&canvas.rect_colors).any(|(r, c)| {
        near(r.width(), 400.0)
            && c.red() == 1.0
            && c.green() == 0.0
            && c.blue() == 0.0
            && near_within(c.alpha(), 0.32, 0.01)
    });
    assert!(red, "{:?} {:?}", canvas.rects, canvas.rect_colors);
    assert_eq!(get(&scrim_color), named_args!(Color::hex(0xFF0000).copy(alpha = 0.32)));
}

// ---- BottomSheetScaffold ---------------------------------------------------------------------------------------

fn bottom_sheet_scaffold_peeks_expands_and_follows_drags() {
    let (sheet, scope) = (slot::<SheetState>(), slot::<CoroutineScope>());
    let seen = slot::<PaddingValues>();
    let content_clicks = Rc::new(Cell::new(0));
    let rule = {
        let (sheet, scope, seen, content_clicks) = (sheet.clone(), scope.clone(), seen.clone(), content_clicks.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            *scope.borrow_mut() = Some(remember_coroutine_scope());
            // (the C++'s `SheetState(PartiallyExpanded, false)`: it may hide too)
            let s = remember_standard_bottom_sheet_state(skip_hidden_state = false);
            *sheet.borrow_mut() = Some(s.clone());
            let (seen, content_clicks) = (seen.clone(), content_clicks.clone());
            BottomSheetScaffold(
                sheet_content = || {
                    Column(modifier = Modifier.fill_max_width().height(300.dp()), content = || Text("Ayrıntılar"));
                },
                modifier = Modifier.fill_max_size(),
                scaffold_state = Some(remember_bottom_sheet_scaffold_state(bottom_sheet_state = Some(s))),
                sheet_peek_height = 100.dp(),
                content = move |p: PaddingValues| {
                    *seen.borrow_mut() = Some(p.clone());
                    let content_clicks = content_clicks.clone();
                    Box(modifier =
                        Modifier.fill_max_size().clickable(move || content_clicks.set(content_clicks.get() + 1)));
                },
            );
        }))
    };
    // 48 of drag handle, then the content: 348 tall; 100 of it peeks out.
    let top = || rule.on_node_with_text("Ayrıntılar").get_unclipped_bounds_in_root().top - 48.0;
    let sheet = get(&sheet);
    assert!(near(top(), 700.0) && sheet.current_value() == SheetValue::PartiallyExpanded, "{}", top());
    assert!(near(get(&seen).calculate_bottom_padding().0, 100.0) && sheet.has_expanded_state());
    // The sheet takes the touch, not the content under it.
    let h = rule.host();
    h.pointer_down(Offset::new(200.0, 750.0), -1, 0, PointerType::Touch);
    h.pointer_up(Offset::new(200.0, 750.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(content_clicks.get(), 0);
    let s = sheet.clone();
    launch(&scope, async move { s.expand().await });
    for _ in 0..5 {
        rule.advance_frame();
    }
    assert!(top() < 700.0 && top() > 452.0, "{}", top());
    assert!(sheet.target_value() == SheetValue::Expanded && sheet.current_value() == SheetValue::PartiallyExpanded);
    rule.wait_for_idle();
    assert!(near(top(), 452.0) && sheet.current_value() == SheetValue::Expanded && near(sheet.require_offset(), 452.0));
    let s = sheet.clone();
    launch(&scope, async move { s.partial_expand().await });
    rule.wait_for_idle();
    assert!(near(top(), 700.0) && sheet.current_value() == SheetValue::PartiallyExpanded);
    // Dragged up slowly; flung up: all the way.
    h.pointer_down(Offset::new(200.0, 720.0), -1, 0, PointerType::Touch);
    for i in 1..=60 {
        rule.advance_frame();
        h.pointer_move(Offset::new(200.0, 720.0 - 100.0 * i as f32 / 60.0), -1, 0);
    }
    let touch_slop = kompoz_ui::platform::ViewConfiguration::default().touch_slop;
    // It followed the finger (placed at whole px).
    assert!(top() > 599.0 && top() <= 600.0 + touch_slop + 2.0, "{}", top());
    rule.advance_frame();
    h.pointer_up(Offset::new(200.0, 620.0), -1, 0);
    rule.wait_for_idle();
    // Let go still moving, slower than the velocity threshold: M15 settles past the sheet's positional threshold (56
    // dp from where the move started; it went 92) on the next anchor, expanded (the C++ went back to the peek, short
    // of half way).
    assert!(near(top(), 452.0) && sheet.current_value() == SheetValue::Expanded, "{}", top());
    let s = sheet.clone();
    launch(&scope, async move { s.partial_expand().await });
    rule.wait_for_idle();
    swipe(&rule, Offset::new(200.0, 720.0), Offset::new(200.0, 669.5), 4);
    rule.wait_for_idle();
    assert!(near(top(), 452.0) && sheet.current_value() == SheetValue::Expanded, "{}", top());
    swipe(&rule, Offset::new(200.0, 470.0), Offset::new(200.0, 520.5), 4); // flung down: to the peek, not further
    rule.wait_for_idle();
    assert!(near(top(), 700.0) && sheet.current_value() == SheetValue::PartiallyExpanded, "{}", top());
    swipe(&rule, Offset::new(200.0, 720.0), Offset::new(200.0, 159.5), 10); // no higher than its height
    rule.advance_frame();
    assert!(top() >= 452.0 - 0.5, "{}", top());
    rule.wait_for_idle();
    let s = sheet.clone();
    launch(&scope, async move { s.hide().await });
    rule.wait_for_idle();
    assert!(near(top(), 800.0) && sheet.current_value() == SheetValue::Hidden && !sheet.is_visible(), "{}", top());
}

fn a_dragged_bottom_sheet_moves_without_recomposing_the_scaffold() {
    let composed = Rc::new(Cell::new(0));
    let rule = {
        let composed = composed.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let composed = composed.clone();
            BottomSheetScaffold(
                sheet_content = || Box(modifier = Modifier.fill_max_width().height(400.dp()).test_tag("sayfa")),
                modifier = Modifier.fill_max_size(),
                sheet_peek_height = 100.dp(),
                content = move |_: PaddingValues| {
                    composed.set(composed.get() + 1);
                    Box(modifier = Modifier.fill_max_size());
                },
            );
        }))
    };
    let before = composed.get();
    let top = top_of(&rule, "sayfa");
    let h = rule.host();
    h.pointer_down(Offset::new(200.0, 750.0), -1, 0, PointerType::Touch);
    for i in 1..=30 {
        rule.advance_frame();
        h.pointer_move(Offset::new(200.0, 750.0 - 4.25 * i as f32), -1, 0);
    }
    rule.advance_frame();
    assert!(top_of(&rule, "sayfa") < top - 50.0, "it followed the finger: laid out again");
    assert_eq!(composed.get(), before, "not composed again");
    h.pointer_up(Offset::new(200.0, 622.5), -1, 0);
    rule.wait_for_idle();
}

fn bottom_sheet_scaffold_moves_before_the_list_in_it_scrolls() {
    let (sheet, scope) = (slot::<SheetState>(), slot::<CoroutineScope>());
    let list = LazyListState::new(0, 0);
    let rule = {
        let (sheet, scope, l) = (sheet.clone(), scope.clone(), list.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            *scope.borrow_mut() = Some(remember_coroutine_scope());
            let s = remember_standard_bottom_sheet_state();
            *sheet.borrow_mut() = Some(s.clone());
            let l = l.clone();
            BottomSheetScaffold(
                sheet_content = move || {
                    LazyColumn(
                        modifier = Modifier.fill_max_width().height(400.dp()),
                        state = Some(l.clone()),
                        content = |this: &mut LazyListScope| {
                            this.items(
                                50,
                                item_content = |_| Box(modifier = Modifier.fill_max_width().height(40.dp())),
                            );
                        },
                    );
                },
                modifier = Modifier.fill_max_size(),
                scaffold_state = Some(remember_bottom_sheet_scaffold_state(bottom_sheet_state = Some(s))),
                sheet_peek_height = 150.dp(),
                content = |_: PaddingValues| {},
            );
        }))
    };
    let sheet = get(&sheet);
    // 448 tall: peeks at 650, expanded at 352. Dragged up on the list: the sheet goes first...
    assert!(near(sheet.require_offset(), 650.0));
    let h = rule.host();
    h.pointer_down(Offset::new(200.0, 760.0), -1, 0, PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(Offset::new(200.0, 760.0 - 10.025 * i as f32), -1, 0);
    }
    rule.advance_frame();
    // (less the first step, to the slop)
    assert!(sheet.require_offset() > 449.0 && sheet.require_offset() < 461.0, "{}", sheet.require_offset());
    assert_eq!(list.first_visible_item_scroll_offset(), 0);
    // ...all the way up, and then the list scrolls.
    for i in 21..=40 {
        rule.advance_frame();
        h.pointer_move(Offset::new(200.0, 760.0 - 10.025 * i as f32), -1, 0);
    }
    rule.advance_frame();
    assert!(near(sheet.require_offset(), 352.0), "{}", sheet.require_offset());
    assert!(list.first_visible_item_index() > 0 || list.first_visible_item_scroll_offset() > 50);
    for _ in 0..30 {
        rule.advance_frame(); // held still: no fling
    }
    h.pointer_up(Offset::new(200.0, 359.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(sheet.current_value(), SheetValue::Expanded);
    // Down: the list back to its top first, then the sheet comes down with it.
    let l = list.clone();
    launch(&scope, async move { named_args!(l.scroll_to_item(0)).await });
    rule.wait_for_idle();
    swipe(&rule, Offset::new(200.0, 500.0), Offset::new(200.0, 700.5), 60);
    rule.wait_for_idle();
    assert!(sheet.current_value() == SheetValue::PartiallyExpanded && near(sheet.require_offset(), 650.0));
}

/// How far a spring of `damping_ratio` and `stiffness` takes a value from rest towards a target `x0` away in `t`
/// seconds (an underdamped one: 1 − e^(−ζωt)(cos ω_d t + ζω/ω_d sin ω_d t)).
fn spring_from_rest(x0: f32, t: f32, damping_ratio: f32, stiffness: f32) -> f32 {
    let w = stiffness.sqrt();
    let wd = w * (1.0 - damping_ratio * damping_ratio).sqrt();
    let decay = (-damping_ratio * w * t).exp();
    x0 * (1.0 - decay * ((wd * t).cos() + damping_ratio * w / wd * (wd * t).sin()))
}

fn bottom_sheet_scaffold_settles_with_the_flings_speed() {
    let sheet = slot::<SheetState>();
    let rule = {
        let sheet = sheet.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let s = remember_standard_bottom_sheet_state();
            *sheet.borrow_mut() = Some(s.clone());
            BottomSheetScaffold(
                sheet_content = || Box(modifier = Modifier.fill_max_width().height(2000.dp())),
                modifier = Modifier.fill_max_size(),
                scaffold_state = Some(remember_bottom_sheet_scaffold_state(bottom_sheet_state = Some(s))),
                sheet_peek_height = 100.dp(),
                content = |_: PaddingValues| {},
            );
        }))
    };
    let sheet = get(&sheet);
    // Flung up 100 dp in 4 frames (about 1560 dp/s): it goes on at that speed, not from rest.
    let h = rule.host();
    h.pointer_down(Offset::new(200.0, 720.0), -1, 0, PointerType::Touch);
    for i in 1..=4 {
        rule.advance_frame();
        h.pointer_move(Offset::new(200.0, 720.0 - 25.125 * i as f32), -1, 0);
    }
    h.pointer_up(Offset::new(200.0, 619.5), -1, 0);
    let released = sheet.require_offset();
    rule.advance_frame();
    rule.advance_frame();
    let moved = released - sheet.require_offset();
    // (Its first frame starts the clock: 16 ms of it.) From rest M15's default spatial spring (damping 0.9,
    // stiffness 700; the C++'s was critically damped at 400) goes `spring_from_rest`; the fling adds about
    // v·t·e^(−ζωt) (17 dp) to it.
    let from_rest = spring_from_rest(released, 0.016, 0.9, 700.0);
    assert!(moved > from_rest + 12.0, "{moved} {from_rest}");
    rule.wait_for_idle();
    // And not past the top.
    assert!(sheet.current_value() == SheetValue::Expanded && near(sheet.require_offset(), 0.0));
}

fn bottom_sheet_scaffold_keeps_clear_of_the_system_bars_once() {
    let sheet = slot::<SheetState>();
    let inside = mutable_state_of(false); // in a layout that already keeps clear of them
    let seen = slot::<PaddingValues>();
    let rule = {
        let (sheet, inside, seen) = (sheet.clone(), inside.clone(), seen.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let m = if inside.get() {
                Modifier.fill_max_size().window_insets_padding(WindowInsets::system_bars())
            } else {
                Modifier.fill_max_size()
            };
            let (sheet, seen) = (sheet.clone(), seen.clone());
            Box(
                modifier = m,
                content = move || {
                    let s = remember_standard_bottom_sheet_state();
                    *sheet.borrow_mut() = Some(s.clone());
                    let seen = seen.clone();
                    BottomSheetScaffold(
                        sheet_content = || Box(modifier = Modifier.fill_max_width().height(2000.dp())),
                        modifier = Modifier.fill_max_size(),
                        scaffold_state = Some(remember_bottom_sheet_scaffold_state(bottom_sheet_state = Some(s))),
                        sheet_peek_height = 100.dp(),
                        content = move |p: PaddingValues| *seen.borrow_mut() = Some(p),
                    );
                },
            );
        }))
    };
    rule.host().set_system_insets(kompoz_ui::platform::SystemInsets {
        status_bars: kompoz_ui::platform::Insets::new(0, 24, 0, 0),
        navigation_bars: kompoz_ui::platform::Insets::new(0, 0, 0, 48),
        ..Default::default()
    });
    rule.wait_for_idle();
    let sheet = get(&sheet);
    // M15's scaffold doesn't keep clear of the system bars (no window insets of its own; the C++'s did, once): 100
    // peeks out of the window's bottom, the content's padding is the peek height only, and all the way up it is at
    // the window's top (the C++: above the navigation bar, under the status bar, top padding 24, bottom 148).
    let padding = get(&seen);
    assert!(near(sheet.require_offset(), 700.0), "{}", sheet.require_offset());
    assert!(near(padding.calculate_top_padding().0, 0.0) && near(padding.calculate_bottom_padding().0, 100.0));
    swipe(&rule, Offset::new(200.0, 720.0), Offset::new(200.0, 639.5), 4);
    rule.wait_for_idle();
    assert!(sheet.current_value() == SheetValue::Expanded && near(sheet.require_offset(), 0.0));
    inside.set(true); // the bars are kept clear of around it (as in the C++: not again)
    rule.wait_for_idle();
    let padding = get(&seen);
    assert!(near(padding.calculate_bottom_padding().0, 100.0) && near(padding.calculate_top_padding().0, 0.0));
    assert!(near(sheet.require_offset(), 0.0));
}

fn a_bottom_sheet_scaffolds_padding_follows_the_sheet_without_recomposing() {
    let tall = mutable_state_of(40.0f32);
    let composed = Rc::new(Cell::new(0));
    let rule = {
        let (tall, composed) = (tall.clone(), composed.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let (tall, composed) = (tall.clone(), composed.clone());
            BottomSheetScaffold(
                sheet_content = move || {
                    let tall = tall.clone();
                    // As tall as `tall`, read as it is measured: less than the peek.
                    Box(modifier = Modifier.fill_max_width().layout(move |scope, measurable, c| {
                        let h = tall.get() as i32;
                        let placeable = measurable.measure(Constraints::fixed(c.max_width, h));
                        scope.layout(c.max_width, h, move |p| named_args!(p.place_relative(&placeable, 0, 0)))
                    }));
                },
                modifier = Modifier.fill_max_size(),
                sheet_peek_height = 300.dp(),
                content = move |padding: PaddingValues| {
                    composed.set(composed.get() + 1);
                    Box(modifier = Modifier.fill_max_size().padding_values(padding).test_tag("içerik"));
                },
            );
        }))
    };
    let bottom = || rule.on_node_with_tag("içerik").get_unclipped_bounds_in_root().bottom;
    let before = composed.get();
    // M15's sheet is at least the peek height tall (`requiredHeightIn(min = peekHeight)`) and the content's padding
    // is the peek height whatever the sheet (the C++'s padding followed the sheet's height): 500 either way, and
    // the content isn't composed again.
    let was = bottom();
    assert!(near(was, 800.0 - 300.0), "{was}");
    tall.set(70.0);
    rule.wait_for_idle();
    assert!(near(bottom(), was) && composed.get() == before);
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

/// The message of what `f` panics with (Kotlin's exception), none if it doesn't.
fn panic_message(f: impl FnOnce() + std::panic::UnwindSafe) -> Option<String> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(f);
    std::panic::set_hook(hook);
    result.err().map(|e| {
        e.downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default()
    })
}

fn sheet_states_start_where_told_and_refuse_what_they_skip() {
    let state = SheetState(false, || 56.0, || 125.0);
    assert!(state.current_value() == SheetValue::Hidden && state.target_value() == SheetValue::Hidden);
    assert!(!state.is_visible() && !state.is_animation_running() && !state.has_expanded_state());
    assert_eq!(state, state.clone());
    assert!(state != SheetState(false, || 56.0, || 125.0));
    let expanded = named_args!(SheetState(true, || 56.0, || 125.0, initial_value = SheetValue::Expanded));
    assert_eq!(expanded.current_value(), SheetValue::Expanded);
    // Kotlin's `require`s: a value it skips can't be where it starts.
    let message = panic_message(|| {
        drop(named_args!(SheetState(true, || 0.0, || 0.0, initial_value = SheetValue::PartiallyExpanded)))
    });
    assert!(message.unwrap().contains("must not be set to PartiallyExpanded if skipPartiallyExpanded"));
    let message = panic_message(|| drop(named_args!(SheetState(false, || 0.0, || 0.0, skip_hidden_state = true))));
    assert!(message.unwrap().contains("must not be set to Hidden if skipHiddenState"));
    // Read before it was laid out: Kotlin's IllegalStateException.
    let message = panic_message(|| {
        let _ = SheetState(false, || 0.0, || 0.0).require_offset();
    });
    assert!(message.unwrap().contains("The offset was read before being initialized"));
}

fn sheet_states_save_their_value() {
    let saver = SheetState::Saver(true, || 56.0, || 125.0, |_| true, false);
    let half = named_args!(SheetState(false, || 56.0, || 125.0, initial_value = SheetValue::PartiallyExpanded));
    let saved = saver.save(&half);
    // A half way value restored by a state that skips it: expanded.
    let restored = saver.restore(&saved).expect("restored");
    assert_eq!(restored.current_value(), SheetValue::Expanded);
    let keeping = SheetState::Saver(false, || 56.0, || 125.0, |_| true, false);
    assert_eq!(keeping.restore(&saved).unwrap().current_value(), SheetValue::PartiallyExpanded);
    for value in SheetValue::ENTRIES {
        assert_eq!(SheetValue::restore(&value.save()), Some(value));
    }
    assert_eq!(SheetValue::restore(&SavedValue::Int(3)), None);
    assert_eq!(format!("{:?}", SheetValue::PartiallyExpanded), "PartiallyExpanded");
}

fn bottom_sheet_defaults_follow_the_tokens() {
    let seen = slot::<(ColorScheme, Color, Color, Arc<dyn Shape>, Arc<dyn Shape>, WindowInsets, WindowInsets)>();
    let rule = {
        let seen = seen.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            *seen.borrow_mut() = Some((
                MaterialTheme::color_scheme(),
                BottomSheetDefaults::ContainerColor(),
                BottomSheetDefaults::ScrimColor(),
                BottomSheetDefaults::ExpandedShape(),
                BottomSheetDefaults::HiddenShape(),
                BottomSheetDefaults::standard_window_insets(),
                BottomSheetDefaults::modal_window_insets(),
            ));
        }))
    };
    rule.host().set_system_insets(kompoz_ui::platform::SystemInsets {
        status_bars: kompoz_ui::platform::Insets::new(0, 24, 0, 0),
        navigation_bars: kompoz_ui::platform::Insets::new(0, 0, 0, 48),
        ..Default::default()
    });
    rule.wait_for_idle();
    let (scheme, container, scrim, expanded, hidden, standard, modal) = get(&seen);
    assert!(container == scheme.surface_container_low && scrim == named_args!(scheme.scrim.copy(alpha = 0.32)));
    let density = Density::new(1.0, 1.0);
    let size = Size::new(400.0, 200.0);
    match expanded.create_outline(size, LayoutDirection::Ltr, &density) {
        Outline::Rounded(r) => {
            assert!(r.top_left_corner_radius.x == 28.0 && r.top_right_corner_radius.x == 28.0);
            assert!(r.bottom_left_corner_radius.x == 0.0 && r.bottom_right_corner_radius.x == 0.0);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(hidden.create_outline(size, LayoutDirection::Ltr, &density), Outline::Rectangle(_)));
    assert_eq!(BottomSheetDefaults::Elevation, 1.dp());
    assert!(BottomSheetDefaults::SheetPeekHeight == 56.dp() && BottomSheetDefaults::SheetMaxWidth == 640.dp());
    // The standard sheet's content keeps clear of the bottom's safe drawing insets, a modal one's of the top's too.
    assert!(standard.get_top(&density) == 0 && standard.get_bottom(&density) == 48);
    assert!(modal.get_top(&density) == 24 && modal.get_bottom(&density) == 48);
    assert_eq!(ModalBottomSheetDefaults::properties(), ModalBottomSheetProperties::default());
    assert!(ModalBottomSheetProperties::default().should_dismiss_on_back_press);
}

fn drag_handles_expand_collapse_and_dismiss_the_sheet() {
    let open = mutable_state_of(true);
    let dismissed = Rc::new(Cell::new(0));
    let state = slot::<SheetState>();
    let rule = {
        let (open, dismissed, state) = (open.clone(), dismissed.clone(), state.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let s = remember_modal_bottom_sheet_state();
            *state.borrow_mut() = Some(s.clone());
            if open.get() {
                let (open, dismissed) = (open.clone(), dismissed.clone());
                ModalBottomSheet(
                    on_dismiss_request = move || {
                        dismissed.set(dismissed.get() + 1);
                        open.set(false);
                    },
                    sheet_state = Some(s),
                    content = || Box(modifier = Modifier.fill_max_width().height(600.dp())),
                );
            }
        }))
    };
    rule.wait_for_idle();
    let s = get(&state);
    assert_eq!(s.current_value(), SheetValue::PartiallyExpanded);
    // Half way: it can be expanded and dismissed (not collapsed).
    let handle = || rule.on_node_with_content_description("Drag handle");
    handle().perform_semantics_action(SemanticsActions::Expand);
    rule.wait_for_idle();
    assert_eq!(s.current_value(), SheetValue::Expanded);
    // Expanded with a half way stop: collapsed.
    handle().perform_semantics_action(SemanticsActions::Collapse);
    rule.wait_for_idle();
    assert_eq!(s.current_value(), SheetValue::PartiallyExpanded);
    // Clicked half way: expanded; clicked expanded: hidden, dismissed.
    handle().perform_click();
    rule.wait_for_idle();
    assert_eq!(s.current_value(), SheetValue::Expanded);
    handle().perform_click();
    rule.wait_for_idle();
    assert!(dismissed.get() == 1 && !open.get() && !s.is_visible());
    // Dismissed by the action.
    open.set(true);
    rule.wait_for_idle();
    handle().perform_semantics_action(SemanticsActions::Dismiss);
    rule.wait_for_idle();
    assert!(dismissed.get() == 2 && !open.get());
}

fn sheets_are_at_most_their_maximum_width() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            BottomSheetScaffold(
                sheet_content = || Box(modifier = Modifier.fill_max_width().height(100.dp()).test_tag("standard")),
                sheet_max_width = 300.dp(),
                content = |_: PaddingValues| {},
            );
            ModalBottomSheet(
                on_dismiss_request = || {},
                content = || Box(modifier = Modifier.fill_max_width().height(100.dp()).test_tag("modal")),
            );
        }),
        IntSize::new(800, 800),
    );
    rule.wait_for_idle();
    // 640 dp at most, centered.
    let modal = rule.on_node_with_tag("modal").get_unclipped_bounds_in_root();
    assert!(near(modal.width(), 640.0) && near(modal.left, 80.0), "{modal:?}");
    let standard = rule.on_node_with_tag("standard").get_unclipped_bounds_in_root();
    assert!(near(standard.width(), 300.0) && near(standard.left, 250.0), "{standard:?}");
}

fn vertical_drag_handles_grow_while_pressed() {
    let defaults = slot::<(ColorScheme, DragHandleColors, DragHandleShapes, DragHandleSizes)>();
    let rule = {
        let defaults = defaults.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            *defaults.borrow_mut() = Some((
                MaterialTheme::color_scheme(),
                VerticalDragHandleDefaults.colors(),
                VerticalDragHandleDefaults.shapes(),
                VerticalDragHandleDefaults.sizes(),
            ));
            Box(
                modifier = Modifier.size(100.dp()),
                content_alignment = Alignment::Center,
                content = || {
                    VerticalDragHandle();
                },
            );
        }))
    };
    rule.wait_for_idle();
    let (scheme, colors, shapes, sizes) = get(&defaults);
    assert_eq!(colors, DragHandleColors(scheme.outline, scheme.on_surface, scheme.on_surface));
    assert_eq!(
        sizes,
        DragHandleSizes(DpSize::new(4.dp(), 48.dp()), DpSize::new(12.dp(), 52.dp()), DpSize::new(12.dp(), 52.dp()))
    );
    assert!(shapes == shapes.clone() && *shapes.pressed_shape == *shapes.dragged_shape);
    let drawn = |width: f32, height: f32, color: Color| {
        let canvas = rule.draw();
        canvas
            .rects
            .iter()
            .zip(&canvas.rect_colors)
            .any(|(r, c)| near(r.width(), width) && near(r.height(), height) && *c == color)
    };
    assert!(drawn(4.0, 48.0, scheme.outline));
    rule.host().pointer_down(Offset::new(50.0, 50.0), -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.advance_frame();
    assert!(drawn(12.0, 52.0, scheme.on_surface));
    rule.host().pointer_up(Offset::new(50.0, 50.0), -1, 0);
    rule.wait_for_idle();
    assert!(drawn(4.0, 48.0, scheme.outline));
    // Some given, the rest the defaults.
    let s = slot::<(DragHandleColors, DragHandleSizes)>();
    let rule = {
        let s = s.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            *s.borrow_mut() = Some((
                VerticalDragHandleDefaults.colors(pressed_color = Color::Red),
                VerticalDragHandleDefaults.sizes(size = DpSize::new(6.dp(), 30.dp())),
            ));
        }))
    };
    rule.wait_for_idle();
    let (c, z) = get(&s);
    assert!(c.pressed_color == Color::Red && c.color == colors.color && z.size == DpSize::new(6.dp(), 30.dp()));
    assert_eq!(z.pressed_size, sizes.pressed_size);
}

// ---- material3's samples (BottomSheetSamples.kt, DragHandleSamples.kt) --------------------------------------------
// (ModalBottomSheetSample composes a Checkbox (WP2) and an OutlinedTextField (WP3), BottomSheetScaffoldNestedScrollSample
// a TopAppBar and its scroll behavior (WP4a): they come with those packages.)

#[kompozable]
fn ManualModalBottomSheetSample() {
    let show_sheet = remember(|| mutable_state_of(false));
    let show = show_sheet.clone();
    Box(
        modifier = Modifier.fill_max_size(),
        content_alignment = Alignment::Center,
        content = move || {
            let show = show.clone();
            Button(on_click = move || show.set(true), content = || Text("Show Manual Sheet"));
        },
    );
    let sheet_state = remember_modal_bottom_sheet_state();

    if show_sheet.get() {
        let (s1, s2) = (show_sheet.clone(), show_sheet.clone());
        Scrim(Some("scrim".to_string()), on_click = Some(Rc::new(move || s1.set(false)) as Rc<dyn Fn()>));
        BottomSheet(
            modifier = Modifier,
            state = Some(sheet_state.clone()),
            on_dismiss_request = move || s2.set(false),
            content = || {
                LazyColumn(
                    content = |this: &mut LazyListScope| {
                        this.items(
                            25,
                            item_content = |it| {
                                ListItem(
                                    headline_content = move || Text(format!("Item {it}")),
                                    leading_content = Some(Rc::new(|| {
                                        VectorIcon(
                                            Icons::Default::Favorite(),
                                            Some("Localized description".to_string()),
                                        );
                                    }) as Rc<dyn Fn()>),
                                    colors =
                                        named_args!(ListItemDefaults.colors(
                                            container_color = MaterialTheme::color_scheme().surface_container_low
                                        )),
                                );
                            },
                        );
                    },
                );
            },
        );
    }
    // Handle sheet animations
    let shown = show_sheet.get();
    LaunchedEffect(shown, async move {
        if shown {
            sheet_state.show().await;
        } else {
            sheet_state.hide().await;
        }
    });
}

fn manual_modal_bottom_sheet_sample() {
    let rule = KompozTestRule::new(ManualModalBottomSheetSample);
    rule.on_node_with_text("Item 0").assert_does_not_exist();
    rule.on_node_with_text("Show Manual Sheet").perform_click();
    rule.wait_for_idle();
    // 25 items fill the window: the sheet stops half way, its items over the button.
    let item = rule.on_node_with_text("Item 0").get_unclipped_bounds_in_root();
    assert!(item.top > 400.0 && item.top < 800.0, "{item:?}");
    // (the items shown so far, each with its icon)
    assert!(
        named_args!(rule.on_all_nodes_with_content_description("Localized description")).fetch_semantics_nodes().len()
            > 5
    );
    // (the scrim's click: the sheet covers its middle, where a click would go)
    rule.on_node_with_content_description("scrim").perform_semantics_action(SemanticsActions::OnClick);
    rule.wait_for_idle();
    rule.on_node_with_text("Item 0").assert_does_not_exist();
    rule.on_node_with_content_description("scrim").assert_does_not_exist();
}

#[kompozable]
fn SimpleBottomSheetScaffoldSample() {
    let scope = remember_coroutine_scope();
    let scaffold_state = remember_bottom_sheet_scaffold_state();
    let state = scaffold_state.bottom_sheet_state.clone();

    BottomSheetScaffold(
        scaffold_state = Some(scaffold_state),
        sheet_peek_height = 128.dp(),
        sheet_content = move || {
            let (state, scope) = (state.clone(), scope.clone());
            Column(
                modifier = Modifier.fill_max_width(),
                horizontal_alignment = Alignment::CenterHorizontally,
                content = move || {
                    Box(
                        modifier = Modifier.fill_max_width().height(128.dp()),
                        content_alignment = Alignment::Center,
                        content = || Text("Swipe up to expand sheet"),
                    );
                    Text("Sheet content");
                    let (focus_state, click_state, scope) = (state.clone(), state.clone(), scope.clone());
                    Button(
                        modifier = named_args!(Modifier.padding_sides(bottom = 64.dp())).focus_properties(move |p| {
                            // Make sure the button is not keyboard focusable when it's offscreen.
                            p.can_focus = focus_state.current_value() == SheetValue::Expanded;
                        }),
                        on_click = move || {
                            let s = click_state.clone();
                            named_args!(scope.launch(async move { s.partial_expand().await }));
                        },
                        content = || Text("Click to collapse sheet"),
                    );
                },
            );
        },
        content = |inner_padding: PaddingValues| {
            Box(
                modifier = Modifier.fill_max_size().padding_values(inner_padding),
                content_alignment = Alignment::Center,
                content = || Text("Scaffold Content"),
            );
        },
    );
}

fn simple_bottom_sheet_scaffold_sample() {
    let rule = KompozTestRule::new(SimpleBottomSheetScaffoldSample);
    rule.wait_for_idle();
    // 128 peeks out: the drag handle (48) and the top of the swipe box (80 of it).
    let swipe_box = rule.on_node_with_text("Swipe up to expand sheet").get_unclipped_bounds_in_root();
    assert!(swipe_box.center().y > 800.0 - 80.0 && swipe_box.center().y < 800.0, "{swipe_box:?}");
    // The content centered above the peek.
    let content = rule.on_node_with_text("Scaffold Content").get_unclipped_bounds_in_root();
    assert!(near(content.center().y, (800.0 - 128.0) / 2.0), "{content:?}");
    swipe(&rule, Offset::new(200.0, 740.0), Offset::new(200.0, 600.5), 4);
    rule.wait_for_idle();
    let button = rule.on_node_with_text("Click to collapse sheet").get_unclipped_bounds_in_root();
    assert!(button.bottom < 800.0 - 64.0 + 1.0, "{button:?}");
    rule.on_node_with_text("Click to collapse sheet").perform_click();
    rule.wait_for_idle();
    let swipe_box = rule.on_node_with_text("Swipe up to expand sheet").get_unclipped_bounds_in_root();
    assert!(swipe_box.center().y > 800.0 - 80.0, "{swipe_box:?}");
}

#[kompozable]
fn VerticalDragHandleSample() {
    let offset_x = remember(|| mutable_state_of(0.0f32));
    let screen_size = remember(|| mutable_state_of(IntSize::Zero));
    let density = kompoz_ui::platform::LocalDensity.current();

    let (o, z) = (offset_x.clone(), screen_size.clone());
    Box(
        modifier = Modifier.fill_max_size().on_globally_positioned(move |layout_coordinates| {
            z.set(layout_coordinates.size());
            if o.get() == 0.0 {
                o.set(z.get().width as f32 / 2.0);
            }
        }),
        content = move || {
            let (offset_x, screen_size) = (offset_x.clone(), screen_size.clone());
            Surface(
                modifier = Modifier.width(density.to_dp(offset_x.get())).fill_max_height(),
                color = MaterialTheme::color_scheme().surface_container_highest,
                shape = Arc::new(RoundedCornerShape::new(0.dp(), 24.dp(), 24.dp(), 0.dp())) as Arc<dyn Shape>,
                content = move || {
                    let (offset_x, screen_size) = (offset_x.clone(), screen_size.clone());
                    Box(
                        modifier = Modifier.fill_max_size(),
                        content_alignment = Alignment::CenterEnd,
                        content = move || {
                            let (offset_x, screen_size) = (offset_x.clone(), screen_size.clone());
                            VerticalDragHandle(
                                modifier = named_args!(Modifier.draggable(
                                    state = remember_draggable_state(move |delta| {
                                        offset_x.set(
                                            (offset_x.get() + delta)
                                                .clamp(density.to_px(48.dp()), screen_size.get().width as f32),
                                        );
                                    }),
                                    orientation = Orientation::Horizontal,
                                ))
                                .system_gesture_exclusion(), // To avoid colliding with the back gesture
                            );
                        },
                    );
                },
            );
        },
    );
}

fn vertical_drag_handle_sample() {
    let rule = KompozTestRule::new(VerticalDragHandleSample);
    rule.wait_for_idle();
    let outline = light_color_scheme().outline;
    // The handle's left side: at the end of a pane half the window wide, in a 48 dp room.
    let handle_left = || {
        let canvas = rule.draw();
        canvas
            .rects
            .iter()
            .zip(&canvas.rect_colors)
            .find(|(r, c)| near(r.width(), 4.0) && near(r.height(), 48.0) && **c == outline)
            .map(|(r, _)| r.left)
    };
    assert!(near(handle_left().expect("drawn"), 200.0 - 48.0 + 22.0));
    // Dragged right: the pane follows.
    slow_drag(&rule, Offset::new(176.0, 400.0), Offset::new(276.5, 400.0), 20);
    rule.wait_for_idle();
    let left = handle_left().expect("drawn");
    assert!(left > 200.0 - 48.0 + 22.0 + 80.0 && left < 200.0 - 48.0 + 22.0 + 101.0, "{left}");
    assert!(!rule.host().system_gesture_exclusion().is_empty());
}

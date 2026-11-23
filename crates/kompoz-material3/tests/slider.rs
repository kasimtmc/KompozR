//! Material's sliders as Compose's material3 1.5 has them: Slider (its plain, slot and state forms), RangeSlider,
//! VerticalSlider, their states, SliderDefaults' thumbs and tracks — taps, drags (the nearer thumb of a range),
//! steps, what they tell their interaction sources, keys, RTL, what they tell accessibility, how they draw. The C++
//! Kompoz's tests by name (kompoz_tests.cpp: their Material halves where the rest is a lower crate's), and Compose's
//! behaviour.
//!
//! The C++ targeted Compose 1.4's slider (a 20 dp round thumb, its track 10 dp in) and the Expressive alphas; M15's
//! thumb is a 4 by 44 dp bar, so the track starts 2 dp in — and while it is pressed or dragged the bar is half as
//! wide, 2 dp, which is what a tap or a drag maps the touch by: x = 1 + 2 × value on a 202 dp slider of 0..100, the
//! C++ tests' sliders' width here (their drawing tests' 204: a 200 dp track from 2 dp in). Touches are at the 48 dp
//! room's middle (y = 24). Compose's draggable drags by what the finger moved past the touch slop (8 dp): a drag moves
//! the value by its length less the slop (a range slider's own gesture counts the slop in).

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::interaction::{DragInteraction, HoverInteraction, Interaction, PressInteraction};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material3::prelude::*;
use kompoz_runtime::kt::KtToString;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_ui::draw::DrawScope;
use kompoz_ui::input::{Key, PointerType};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::{ProgressBarRangeInfo, SemanticsAction, SemanticsActions};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    slider_tap_drag_and_steps,
    range_slider_moves_the_nearer_thumb,
    slider_tells_its_interaction_source_presses_drags_and_hovers,
    slider_takes_only_touches_not_taken_and_keeps_those_it_takes,
    range_slider_tells_each_thumbs_source,
    slider_thumbs_and_tracks_are_slots_given_its_state,
    remembered_slider_state_keeps_its_value_and_steps,
    material3_slider_track_keeps_apart_from_its_bar_thumb,
    range_slider_thumbs_and_track_are_slots,
    remembered_range_slider_state_and_its_material3_track,
    labels_show_over_their_content_while_held_or_always,
    vertical_slider_grows_upwards_and_follows_a_vertical_drag,
    semantics_ranges_live_regions_custom_actions_and_order,
    slider_keys_step_the_value_and_finish_on_release,
    range_slider_thumbs_take_their_own_keys_and_set_progress,
    rtl_sliders_drag_and_step_from_the_right,
    disabled_sliders_take_no_input_and_keep_their_set_progress,
    slider_state_snaps_to_its_steps_unless_told_not_to,
    slider_colors_come_from_the_scheme_and_copy_what_is_given,
    tracks_draw_their_stop_indicators_and_ticks_as_told,
    centered_and_corner_sized_tracks_as_compose,
    the_deprecated_positions_track_draws_lines_and_points,
    slider_states_are_saved_and_restored,
);

type Log = Rc<RefCell<Vec<&'static str>>>;

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

/// The bounds of the tagged layout (a slider's semantics node is wider: Compose's 10 dp more each side for a
/// screen reader; and it merges what is in it, so the unmerged tree's node).
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    let c = rule.on_node_with_tag_unmerged(tag).layout_node().coordinates();
    let (p, size) = (c.position_in_root(), c.size());
    Rect::new(p.x, p.y, p.x + size.width as f32, p.y + size.height as f32)
}

/// The touch slop (Compose's default view configuration: 8 dp).
const Slop: f32 = 8.0;

/// C++'s `tap`: down, a frame, up, a frame.
fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_frame();
}

/// C++'s `rule.swipe`: a finger from `from` to `to`, one move a frame in ten, let go, a frame.
fn swipe(rule: &KompozTestRule, from: Offset, to: Offset) {
    rule.host().pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        rule.host().pointer_move(from + (to - from) * (i as f32 / 10.0), -1, 0);
    }
    rule.host().pointer_up(to, -1, 0);
    rule.advance_frame();
}

/// C++'s `slowDrag`: a finger drag in `frames` moves that holds still before lifting (no fling).
fn slow_drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    rule.host().pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        rule.host().pointer_move(lerp_offset(from, to, i as f32 / frames as f32), -1, 0);
    }
    rule.main_clock().advance_time_by(200);
    rule.host().pointer_up(to, -1, 0);
    rule.advance_frame();
}

fn mouse_to(rule: &KompozTestRule, p: Offset) {
    rule.on_root().perform_mouse_input(|m| m.move_to(p));
}

/// A value-change callback setting `state` (Kotlin's `{ value = it }`).
fn set_to<T: PartialEq + Clone + Send + Sync + 'static>(state: &MutableState<T>) -> impl Fn(T) + 'static {
    let state = state.clone();
    move |v| state.set(v)
}

/// A callback counting its calls.
fn counting(count: &Rc<Cell<i32>>) -> Option<Rc<dyn Fn()>> {
    let count = count.clone();
    Some(Rc::new(move || count.set(count.get() + 1)))
}

/// A slot (Kotlin's `thumb = { … }`).
fn slot(f: impl Fn(SliderState) + 'static) -> Option<Rc<dyn Fn(SliderState)>> {
    Some(Rc::new(f))
}

/// A range slider's slot.
fn range_slot(f: impl Fn(RangeSliderState) + 'static) -> Option<Rc<dyn Fn(RangeSliderState)>> {
    Some(Rc::new(f))
}

fn interaction_name(i: &dyn Interaction) -> &'static str {
    if i.is::<PressInteraction::Press>() {
        "Press"
    } else if i.is::<PressInteraction::Release>() {
        "Release"
    } else if i.is::<PressInteraction::Cancel>() {
        "Cancel"
    } else if i.is::<DragInteraction::Start>() {
        "DragStart"
    } else if i.is::<DragInteraction::Stop>() {
        "DragStop"
    } else if i.is::<DragInteraction::Cancel>() {
        "DragCancel"
    } else if i.is::<HoverInteraction::Enter>() {
        "HoverEnter"
    } else if i.is::<HoverInteraction::Exit>() {
        "HoverExit"
    } else {
        "?"
    }
}

/// C++'s `LogInteractions`: what `source` tells, named, into `log`.
#[kompozable]
fn LogInteractions(source: MutableInteractionSource, log: Log) {
    let flow = source.interactions();
    kompoz_coroutines::LaunchedEffect(source.clone(), async move {
        flow.collect(move |i: std::sync::Arc<dyn Interaction>| log.borrow_mut().push(interaction_name(&*i))).await;
    });
}

fn taken(log: &Log) -> Vec<&'static str> {
    std::mem::take(&mut *log.borrow_mut())
}

/// Where `v` is on a held 202 dp slider of 0..100 (its 2 dp held thumb's middle there: x = 1 + 2v).
fn x_of(v: f32) -> f32 {
    1.0 + 2.0 * v
}

/// Where `v` is on a 204 dp range slider of 0..100, its thumbs not held (4 dp bars: x = 2 + 2v).
fn rx(v: f32) -> f32 {
    2.0 + 2.0 * v
}

/// The paths drawn of height `height` (the tracks' stretches: rounded rects drawn as paths), with their colors and
/// whether their start (left or top) corners and end corners are rounded by more than 2 px (a track's outside
/// corners, 8 px; its inside ones near the thumb, 2 px).
fn stretches(canvas: &RecordingCanvas, height: f32) -> Vec<(Rect, Color, bool, bool)> {
    let mut out = Vec::new();
    for (i, b) in canvas.paths.iter().enumerate() {
        if near(b.height(), height, 0.01) {
            let shape = &canvas.path_shapes[i];
            let r = shape.get_bounds();
            // (a point 1 px in from a corner: outside a corner of radius 8, inside one of radius 2)
            let start_round = !shape.contains(Offset::new(r.left + 1.0, r.top + 1.0));
            let end_round = !shape.contains(Offset::new(r.right - 1.0, r.bottom - 1.0));
            out.push((*b, canvas.path_colors[i], start_round, end_round));
        }
    }
    out
}

/// The dots drawn 4 px across (stop indicators and ticks): their centers' x.
fn dots(canvas: &RecordingCanvas) -> Vec<f32> {
    let mut out = Vec::new();
    for (i, b) in canvas.paths.iter().enumerate() {
        if near(b.width(), 4.0, 0.01) && near(b.height(), 4.0, 0.01) {
            let _ = i;
            out.push(b.center().x);
        }
    }
    for r in &canvas.rects {
        if near(r.width(), 4.0, 0.01) && near(r.height(), 4.0, 0.01) {
            out.push(r.center().x);
        }
    }
    out
}

/// The thumbs drawn: filled rects `width` by `height`.
fn bars(canvas: &RecordingCanvas, width: f32, height: f32) -> Vec<Rect> {
    canvas.rects.iter().filter(|r| near(r.width(), width, 0.01) && near(r.height(), height, 0.01)).copied().collect()
}

// ---- C++ slider_tap_drag_and_steps ----------------------------------------------------------------------------

fn slider_tap_drag_and_steps() {
    let value = mutable_state_of(0.0f32);
    let finished = Rc::new(Cell::new(0));
    let scroll = Rc::new(RefCell::new(None::<ScrollState>));
    let (v, f, sc) = (value.clone(), finished.clone(), scroll.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let state = remember_scroll_state();
        *sc.borrow_mut() = Some(state.clone());
        let (v, f) = (v.clone(), f.clone());
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&state),
            content = move || {
                Slider(
                    v.get(),
                    set_to(&v),
                    modifier = Modifier.width(202.dp()),
                    value_range = 0.0..=100.0,
                    on_value_change_finished = counting(&f),
                );
                Slider(
                    v.get(),
                    set_to(&v),
                    modifier = Modifier.width(202.dp()).test_tag("stepped"),
                    value_range = 0.0..=100.0,
                    steps = 3,
                );
                Box(Modifier.height(2000.dp()));
            },
        );
    }));
    let scroll = || scroll.borrow().clone().unwrap().value();
    // Held, the thumb is a 2 dp bar: x = 51 is a quarter.
    tap(&rule, at(x_of(25.0), 24.0));
    rule.wait_for_idle();
    assert!(near(value.peek(), 25.0, 0.01) && finished.get() == 1, "{} {}", value.peek(), finished.get());
    // A drag moves it by what the finger moved past the touch slop.
    swipe(&rule, at(x_of(25.0), 24.0), at(x_of(75.0), 24.0));
    rule.wait_for_idle();
    assert!(near(value.peek(), 75.0 - Slop / 2.0, 0.01) && finished.get() == 2, "{} {}", value.peek(), finished.get());
    assert_eq!(scroll(), 0);
    // 42.5% on the stepped slider: 3 steps allow 0/25/50/75/100, so 50.
    let stepped = bounds(&rule, "stepped");
    tap(&rule, at(x_of(42.5), stepped.top + 24.0));
    rule.wait_for_idle();
    assert!(near(value.peek(), 50.0, 0.01), "{}", value.peek());
    // A vertical swipe on the slider scrolls the page instead.
    swipe(&rule, at(160.0, 300.0), at(160.0, 100.0));
    rule.wait_for_idle();
    assert!(scroll() > 100, "{}", scroll());
}

// ---- C++ range_slider_moves_the_nearer_thumb ------------------------------------------------------------------

fn range_slider_moves_the_nearer_thumb() {
    let range = mutable_state_of(20.0f32..=80.0);
    let finished = Rc::new(Cell::new(0));
    let scroll = Rc::new(RefCell::new(None::<ScrollState>));
    let (r, f, sc) = (range.clone(), finished.clone(), scroll.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let state = remember_scroll_state();
        *sc.borrow_mut() = Some(state.clone());
        let (r, f) = (r.clone(), f.clone());
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&state),
            content = move || {
                RangeSlider(
                    r.get(),
                    set_to(&r),
                    modifier = Modifier.width(204.dp()).test_tag("range"),
                    value_range = 0.0..=100.0,
                    on_value_change_finished = counting(&f),
                );
                RangeSlider(
                    r.get(),
                    set_to(&r),
                    modifier = Modifier.width(204.dp()),
                    value_range = 0.0..=100.0,
                    steps = 9,
                );
                Box(Modifier.height(2000.dp()));
            },
        );
    }));
    let scroll = || scroll.borrow().clone().unwrap().value();
    let same = |a: f32, b: f32| {
        let r = range.peek();
        near(*r.start(), a, 0.5) && near(*r.end(), b, 0.5)
    };
    // Two 4 dp bar thumbs: the track runs from 2 to 202, x = 2 + 2 × value (a held thumb is 2 dp wide once its
    // press, launched, is told: a frame after the down — a value is within half a unit of that).
    tap(&rule, at(rx(30.0), 24.0)); // nearer the start
    rule.wait_for_idle();
    assert!(same(30.0, 80.0) && finished.get() == 1, "{:?} {}", range.peek(), finished.get());
    tap(&rule, at(rx(65.0), 24.0)); // nearer the end
    rule.wait_for_idle();
    assert!(same(30.0, 65.0), "{:?}", range.peek());
    // A drag moves the nearer thumb by what the finger moved (Compose's range slider counts the slop in); the start
    // stops at the end.
    swipe(&rule, at(rx(30.0), 24.0), at(rx(95.0), 24.0));
    rule.wait_for_idle();
    assert!(same(65.0, 65.0) && scroll() == 0, "{:?}", range.peek());
    // (C++'s one description "65% – 65%": Compose's thumbs have their own nodes and values)
    range.set(65.0..=65.0);
    rule.wait_for_idle();
    let thumbs: Vec<String> = rule
        .on_node_with_tag("range")
        .on_children()
        .fetch_semantics_nodes()
        .iter()
        .filter_map(|n| n.config().get(kompoz_ui::semantics::SemanticsProperties::StateDescription))
        .collect();
    assert_eq!(thumbs, ["65.0", "65.0"]);
    // Together: the press's side picks the thumb (Compose's: the start one where the press is before them, the end
    // one after; C++'s picked by the drag's way).
    range.set(65.0..=65.0);
    rule.wait_for_idle();
    swipe(&rule, at(rx(65.0) - 1.0, 24.0), at(rx(40.0), 24.0));
    rule.wait_for_idle();
    assert!(same(40.0, 65.0), "{:?}", range.peek());
    swipe(&rule, at(rx(60.0), 24.0), at(rx(90.0), 24.0)); // from nearer the end
    rule.wait_for_idle();
    assert!(same(40.0, 90.0), "{:?}", range.peek());
    let stepped_y = bounds(&rule, "range").bottom + 24.0;
    tap(&rule, at(rx(13.0), stepped_y)); // the stepped one: 13 is nearer 10 than 20
    rule.wait_for_idle();
    assert!(same(10.0, 90.0), "{:?}", range.peek());
    swipe(&rule, at(160.0, 300.0), at(160.0, 100.0)); // vertically: the page scrolls
    rule.wait_for_idle();
    assert!(scroll() > 100, "{}", scroll());
}

// ---- C++ slider_tells_its_interaction_source_presses_drags_and_hovers -----------------------------------------

#[kompozable]
fn PressedAndDragged(source: MutableInteractionSource) {
    let pressed = source.collect_is_pressed_as_state().get();
    let dragged = source.collect_is_dragged_as_state().get();
    Text(format!("basılı {pressed} sürükleniyor {dragged}"));
}

fn slider_tells_its_interaction_source_presses_drags_and_hovers() {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let off_log: Log = Rc::new(RefCell::new(Vec::new()));
    let value = mutable_state_of(0.0f32);
    let shown = mutable_state_of(true);
    let (l, ol, v, sh) = (log.clone(), off_log.clone(), value.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let source = remember(MutableInteractionSource::new);
        let off = remember(MutableInteractionSource::new);
        LogInteractions(source.clone(), l.clone());
        LogInteractions(off.clone(), ol.clone());
        let (v, sh) = (v.clone(), sh.clone());
        Column(
            content = move || {
                if sh.get() {
                    Slider(
                        v.get(),
                        set_to(&v),
                        modifier = Modifier.width(202.dp()).test_tag("kaydırıcı"),
                        value_range = 0.0..=100.0,
                        interaction_source = source.clone(),
                    );
                }
                Slider(
                    50.0,
                    |_| {},
                    modifier = Modifier.width(202.dp()).test_tag("kapalı"),
                    value_range = 0.0..=100.0,
                    enabled = false,
                    interaction_source = off.clone(),
                );
                PressedAndDragged(source.clone());
            },
        );
    }));
    // A press tells the press (Compose's: the value moves at the tap or at the drag, not at the press).
    rule.host().pointer_down(at(x_of(25.0), 24.0), -1, 0, PointerType::Touch);
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["Press"]);
    rule.on_node_with_text("basılı true sürükleniyor false").assert_exists();
    assert_eq!(value.peek(), 0.0);
    rule.host().pointer_move(at(x_of(25.0) + 3.0, 24.0), -1, 0); // under the touch slop: still a press
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["Press"]);
    let mut x = x_of(30.0);
    while x <= x_of(75.0) {
        rule.host().pointer_move(at(x, 24.0), -1, 0);
        x += 10.0;
    }
    rule.wait_for_idle();
    // Past the slop it drags: the drag is told, the press cancelled (the tap's gesture sees its moves taken).
    assert_eq!(*log.borrow(), ["Press", "Cancel", "DragStart"]);
    rule.on_node_with_text("basılı false sürükleniyor true").assert_exists();
    assert!(near(value.peek(), 75.0 - Slop / 2.0, 0.01), "{}", value.peek());
    rule.host().pointer_up(at(x_of(75.0), 24.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["Press", "Cancel", "DragStart", "DragStop"]);
    rule.on_node_with_text("basılı false sürükleniyor false").assert_exists();
    taken(&log);
    tap(&rule, at(x_of(50.0), 24.0)); // a tap: a press and its release, no drag
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["Press", "Release"]);
    assert!(near(value.peek(), 50.0, 0.01), "{}", value.peek());
    // Gone while dragged: both cancelled.
    rule.host().pointer_down(at(x_of(50.0), 24.0), -1, 0, PointerType::Touch);
    let mut x = x_of(55.0);
    while x <= x_of(75.0) {
        rule.host().pointer_move(at(x, 24.0), -1, 0);
        x += 10.0;
    }
    rule.wait_for_idle();
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["Press", "Cancel", "DragStart", "DragCancel"]);
    rule.on_node_with_text("basılı false sürükleniyor false").assert_exists();
    rule.host().pointer_up(at(x_of(75.0), 24.0), -1, 0);
    shown.set(true);
    rule.wait_for_idle();
    taken(&log);
    // The mouse over the thumb: hovered (M15 tells the hovering of its thumb only, the 4 dp bar).
    let thumb_x = 2.0 + (198.0 * value.peek() / 100.0 + 0.5).floor();
    mouse_to(&rule, at(thumb_x + 1.0, 24.0));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["HoverEnter"]);
    mouse_to(&rule, at(thumb_x + 40.0, 24.0)); // the slider, off its thumb
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["HoverExit"]);
    // A disabled one tells no press (only its thumb's hovering, which M15's thumb tells enabled or not).
    let disabled = bounds(&rule, "kapalı");
    tap(&rule, at(x_of(50.0), disabled.top + 24.0));
    rule.wait_for_idle();
    assert!(off_log.borrow().is_empty(), "{:?}", off_log.borrow());
    mouse_to(&rule, at(2.0 + 99.0 + 1.0, disabled.top + 24.0));
    rule.wait_for_idle();
    assert_eq!(taken(&off_log), ["HoverEnter"]);
}

// ---- C++ slider_takes_only_touches_not_taken_and_keeps_those_it_takes -----------------------------------------

/// A box over the thumb taking what `takes` says: 1 the moves, 2 the presses.
#[kompozable]
fn Taker(takes: i32) {
    Box(Modifier.size_wh(4.dp(), 44.dp()).pointer_input(
        takes,
        move |scope: kompoz_ui::input::PointerInputScope| async move {
            scope
                .await_pointer_event_scope(move |s| async move {
                    loop {
                        let e = s.await_pointer_event().await;
                        if (takes == 1 && e.r#type == kompoz_ui::input::PointerEventType::Move)
                            || (takes == 2 && e.r#type == kompoz_ui::input::PointerEventType::Press)
                        {
                            for c in &e.changes {
                                c.consume();
                            }
                        }
                    }
                })
                .await
        },
    ));
}

fn slider_takes_only_touches_not_taken_and_keeps_those_it_takes() {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let value = mutable_state_of(50.0f32);
    let thumb_takes = mutable_state_of(0);
    let outer_clicks = Rc::new(Cell::new(0));
    let scrolls = Rc::new(RefCell::new(None::<(ScrollState, ScrollState)>));
    let (l, v, tt, oc, sc) = (log.clone(), value.clone(), thumb_takes.clone(), outer_clicks.clone(), scrolls.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let source = remember(MutableInteractionSource::new);
        LogInteractions(source.clone(), l.clone());
        let (across, down) = (remember_scroll_state(), remember_scroll_state());
        *sc.borrow_mut() = Some((across.clone(), down.clone()));
        let (v, tt, oc) = (v.clone(), tt.clone(), oc.clone());
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&down),
            content = move || {
                let (v, tt, oc, source) = (v.clone(), tt.clone(), oc.clone(), source.clone());
                Row(
                    modifier = Modifier.fill_max_width().horizontal_scroll(&across),
                    content = move || {
                        let (v, tt, oc, source) = (v.clone(), tt.clone(), oc.clone(), source.clone());
                        Box(
                            modifier = Modifier.clickable(move || oc.set(oc.get() + 1)),
                            content = move || {
                                let (tt, thumb_source) = (tt.clone(), source.clone());
                                SlotSlider(
                                    v.get(),
                                    set_to(&v),
                                    modifier = Modifier.width(202.dp()),
                                    value_range = 0.0..=100.0,
                                    interaction_source = source.clone(),
                                    thumb = slot(move |_| {
                                        let takes = tt.get();
                                        let thumb_source = thumb_source.clone();
                                        Box(content = move || {
                                            SliderDefaults.Thumb(thumb_source.clone());
                                            Taker(takes);
                                        });
                                    }),
                                );
                            },
                        );
                        Box(Modifier.size_wh(1000.dp(), 48.dp()));
                    },
                );
                Box(Modifier.fill_max_width().height(2000.dp()));
            },
        );
    }));
    let across = || scrolls.borrow().clone().unwrap().0.value();
    let down = || scrolls.borrow().clone().unwrap().1.value();
    // (the thumb here: a 4 dp box, held or not — x = 2 + 1.98 × value)
    let px = |v: f32| 2.0 + 1.98 * v;
    // A drag along it moves the thumb, not the row it is in; a tap on it isn't the box's around it.
    slow_drag(&rule, at(px(25.0), 24.0), at(px(75.0), 24.0), 20);
    assert!(near(value.peek(), 75.0 - Slop / 1.98, 0.01) && across() == 0, "{} {}", value.peek(), across());
    tap(&rule, at(px(50.0), 24.0));
    rule.wait_for_idle();
    assert!(near(value.peek(), 50.0, 0.01) && outer_clicks.get() == 0, "{} {}", value.peek(), outer_clicks.get());
    // Moves the thumb itself takes (a box over M15's 4 dp bar thumb, at 101), it doesn't follow.
    thumb_takes.set(1);
    rule.wait_for_idle();
    slow_drag(&rule, at(101.0, 24.0), at(181.0, 24.0), 20);
    assert!(near(value.peek(), 50.0, 0.01), "{}", value.peek());
    // A press the thumb takes, the tap's gesture doesn't take: no press is told; Compose's draggable waits for a
    // down taken or not (its first down is the Initial pass's), so the drag still moves it, from where it was.
    thumb_takes.set(2);
    rule.wait_for_idle();
    taken(&log);
    slow_drag(&rule, at(101.0, 24.0), at(181.0, 24.0), 20);
    assert_eq!(taken(&log), ["DragStart", "DragStop"]);
    assert!(near(value.peek(), 50.0 + (80.0 - Slop) / 1.98, 0.01), "{}", value.peek());
    // A drag the column around it takes (down): the press is cancelled, not released.
    thumb_takes.set(0);
    rule.wait_for_idle();
    taken(&log);
    slow_drag(&rule, at(px(25.0), 24.0), at(px(25.0), 4.0), 20);
    assert!(down() > 0, "{}", down());
    assert_eq!(taken(&log), ["Press", "Cancel"]);
}

// ---- C++ range_slider_tells_each_thumbs_source ----------------------------------------------------------------

fn range_slider_tells_each_thumbs_source() {
    let (start_log, end_log): (Log, Log) = (Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())));
    let range = mutable_state_of(20.0f32..=80.0);
    let (sl, el, r) = (start_log.clone(), end_log.clone(), range.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let start = remember(MutableInteractionSource::new);
        let end = remember(MutableInteractionSource::new);
        LogInteractions(start.clone(), sl.clone());
        LogInteractions(end.clone(), el.clone());
        SlotRangeSlider(
            r.get(),
            set_to(&r),
            modifier = Modifier.width(204.dp()),
            value_range = 0.0..=100.0,
            start_interaction_source = start,
            end_interaction_source = end,
        );
    }));
    tap(&rule, at(rx(30.0), 24.0)); // nearer the start
    rule.wait_for_idle();
    assert_eq!(taken(&start_log), ["Press", "Release"]);
    assert!(end_log.borrow().is_empty());
    swipe(&rule, at(rx(70.0), 24.0), at(rx(90.0), 24.0)); // the end, dragged: its press turns into the drag
    rule.wait_for_idle();
    assert!(start_log.borrow().is_empty(), "{:?} {:?}", start_log.borrow(), end_log.borrow());
    assert_eq!(taken(&end_log), ["Press", "Cancel", "DragStart", "DragStop"]);
    let r = range.peek();
    assert!(near(*r.start(), 30.0, 0.5) && near(*r.end(), 90.0, 0.5), "{r:?}");
    // Set equal from code, the thumbs keep their pixel places (Compose's range slider moves them from its ends only
    // when they differ or its size changes): a press on them is the nearer one's by where it last was — the start
    // (at 30, the end at 90). (C++ told the press once the drag's way picked a thumb.)
    range.set(50.0..=50.0);
    rule.wait_for_idle();
    rule.host().pointer_down(at(rx(50.0), 24.0), -1, 0, PointerType::Touch);
    rule.wait_for_idle();
    assert_eq!(*start_log.borrow(), ["Press"]);
    assert!(end_log.borrow().is_empty());
    let mut d = 5.0;
    while d <= 40.0 {
        rule.host().pointer_move(at(rx(50.0) - d, 24.0), -1, 0);
        d += 5.0;
    }
    rule.wait_for_idle();
    assert_eq!(*start_log.borrow(), ["Press", "Cancel", "DragStart"]);
    assert!(end_log.borrow().is_empty());
    rule.host().pointer_up(at(rx(50.0) - 40.0, 24.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(taken(&start_log), ["Press", "Cancel", "DragStart", "DragStop"]);
    assert!(end_log.borrow().is_empty());
    let r = range.peek();
    assert!(near(*r.start(), 30.0, 0.5) && near(*r.end(), 50.0, 0.01), "{r:?}");
}

// ---- C++ slider_thumbs_and_tracks_are_slots_given_its_state ---------------------------------------------------

fn slider_thumbs_and_tracks_are_slots_given_its_state() {
    let value = mutable_state_of(25.0f32);
    let seen = Rc::new(RefCell::new(None::<SliderState>));
    let (v, se) = (value.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, se) = (v.clone(), se.clone());
        Column(
            content = move || {
                let source = remember(MutableInteractionSource::new);
                let (se, held) = (se.clone(), source.clone());
                SlotSlider(
                    v.get(),
                    set_to(&v),
                    modifier = Modifier.width(200.dp()).test_tag("kaydırıcı"),
                    value_range = 0.0..=100.0,
                    interaction_source = source,
                    // (M15's state has no isPressed: the thumb asks the slider's interaction source)
                    thumb = slot(move |s| {
                        *se.borrow_mut() = Some(s);
                        let pressed = held.collect_is_pressed_as_state().get();
                        Box(Modifier.size(if pressed { 20.dp() } else { 10.dp() }).test_tag("başparmak"));
                    }),
                    track = slot(|_| Box(Modifier.fill_max_width().height(6.dp()).test_tag("iz"))),
                );
                // (C++'s SliderDefaults::ValueThumb: M15 has none; a thumb showing the state's value)
                SlotSlider(
                    50.0,
                    |_| {},
                    modifier = Modifier.width(200.dp()),
                    value_range = 0.0..=100.0,
                    thumb = slot(|s| Text(s.value().kt_to_string())),
                );
            },
        );
    }));
    // The track: as long as the slider less the thumb, from half a thumb in; the thumb's middle at the value along it.
    let (slider, thumb, track) = (bounds(&rule, "kaydırıcı"), bounds(&rule, "başparmak"), bounds(&rule, "iz"));
    assert!(
        slider.height() == 48.0 && track.width() == 190.0 && track.left == slider.left + 5.0,
        "{slider:?} {track:?}"
    );
    assert!(near(track.top + 3.0, slider.top + 24.0, 0.01)); // 48 dp to touch
    assert!(thumb.left - slider.left == (190.0f32 * 0.25 + 0.5).floor() && thumb.width() == 10.0, "{thumb:?}"); // (at whole px)
    let s = seen.borrow().clone().unwrap();
    assert!(s.value() == 25.0 && s.value_range() == (0.0..=100.0) && near(s.coerced_value_as_fraction(), 0.25, 1e-4));
    rule.on_node_with_text("50.0").assert_exists(); // the thumb shows it
    // Held: the thumb knows it — and grows, which the tap's value is mapped by (M15 moves the value at the tap).
    let p = at(slider.left + 5.0 + 190.0 * 0.75, slider.top + 24.0);
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.wait_for_idle();
    assert!(value.peek() == 25.0 && !s.is_dragging() && bounds(&rule, "başparmak").width() == 20.0);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
    // (the 20 dp thumb: the track from 10 to 190)
    assert!(near(value.peek(), (147.5 - 10.0) / 180.0 * 100.0, 0.01), "{}", value.peek());
    assert_eq!(bounds(&rule, "başparmak").width(), 10.0);
}

// ---- C++ remembered_slider_state_keeps_its_value_and_steps ----------------------------------------------------

fn remembered_slider_state_keeps_its_value_and_steps() {
    let finished = Rc::new(Cell::new(0));
    let kept = Rc::new(RefCell::new(None::<SliderState>));
    let (f, k) = (finished.clone(), kept.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        // 1..5 in four parts
        let state = remember_slider_state(2.0, 3, counting(&f), 1.0..=5.0);
        *k.borrow_mut() = Some(state.clone());
        // (its range is the state's: Slider(state) takes none)
        StateSlider(state, modifier = Modifier.width(202.dp()).test_tag("puan"));
    }));
    let kept = kept.borrow().clone().unwrap();
    assert!(kept.value() == 2.0 && kept.steps() == 3 && kept.value_range() == (1.0..=5.0));
    let b = bounds(&rule, "puan");
    tap(&rule, at(b.left + 1.0 + 200.0 * 0.7, b.top + 24.0)); // nearest the step at 0.75
    rule.wait_for_idle();
    assert!(kept.value() == 4.0 && finished.get() == 1, "{} {}", kept.value(), finished.get());
    kept.set_value(9.0); // set from code: kept in range
    rule.wait_for_idle();
    assert_eq!(kept.value(), 5.0);
}

// ---- C++ material3_slider_track_keeps_apart_from_its_bar_thumb ------------------------------------------------

fn material3_slider_track_keeps_apart_from_its_bar_thumb() {
    let red = Color::new(1.0, 0.0, 0.0, 1.0);
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(
            content = move || {
                let source = remember(MutableInteractionSource::new);
                let thumb_source = source.clone();
                SlotSlider(
                    0.5,
                    |_| {},
                    modifier = Modifier.width(204.dp()).test_tag("m3"),
                    interaction_source = source,
                    thumb = slot(move |_| SliderDefaults.Thumb(thumb_source.clone())),
                    track = slot(|s| SliderDefaults.Track(s)),
                );
                let other = remember(MutableInteractionSource::new);
                let colors = SliderDefaults.colors(thumb_color = red);
                let thumb_other = other.clone();
                SlotSlider(
                    1.0,
                    |_| {},
                    modifier = Modifier.width(204.dp()).test_tag("adımlı"),
                    value_range = 0.0..=4.0,
                    steps = 3,
                    colors = colors,
                    interaction_source = other,
                    // (C++'s SliderDefaults::ShapeThumb(RectangleShape, 8 × 8): M15's thumb of an 8 by 8 size)
                    thumb = slot(move |_| {
                        SliderDefaults.Thumb(
                            thumb_other.clone(),
                            colors = colors,
                            thumb_size = DpSize::new(8.dp(), 8.dp()),
                        )
                    }),
                    track = slot(move |s| SliderDefaults.Track(s, colors = colors)),
                );
                let upright = remember(|| SliderState::new(0.5, 0, None, 0.0..=1.0));
                VerticalSlider(upright, modifier = Modifier.height(204.dp()).test_tag("dikey"));
            },
        );
    }));
    let canvas = rule.draw();
    // The first: a 4 dp bar thumb, a 200 dp track from 2 dp in; the value at 100: the active stretch ends 2 + 6
    // short of it, the inactive one starts 8 past it, a stop dot at its end.
    let m3 = bounds(&rule, "m3");
    let track_bars: Vec<_> =
        stretches(&canvas, 16.0).into_iter().filter(|s| near(s.0.top, m3.top + 16.0, 0.01)).collect();
    assert_eq!(track_bars.len(), 2, "{track_bars:?}");
    assert!(
        near(track_bars[0].0.left, m3.left + 2.0 + 108.0, 0.01) && near(track_bars[0].0.right, m3.left + 202.0, 0.01)
    ); // inactive
    assert!(near(track_bars[1].0.left, m3.left + 2.0, 0.01) && near(track_bars[1].0.right, m3.left + 2.0 + 92.0, 0.01)); // active
    assert!(!track_bars[0].2 && track_bars[0].3 && track_bars[1].2 && !track_bars[1].3, "{track_bars:?}"); // their corners: 2 by the thumb, 8 out
    let cs = light_color_scheme();
    assert!(track_bars[0].1 == cs.secondary_container && track_bars[1].1 == cs.primary);
    assert!(dots(&canvas).iter().any(|x| near(*x, m3.left + 202.0 - 8.0, 0.01)), "{:?}", dots(&canvas));
    // The second: on its first step the thumb stands on its tick, held 8 dp in from the track's ends.
    let stepped = bounds(&rule, "adımlı");
    let red_thumb = canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .any(|(r, c)| *c == red && near(r.left + 4.0, stepped.left + 4.0 + 8.0 + (196.0 - 16.0) * 0.25, 0.01));
    assert!(red_thumb);
    // Standing up, the bar lies down.
    let upright = bounds(&rule, "dikey");
    assert!(bars(&canvas, 44.0, 4.0).iter().any(|r| r.left >= upright.left - 0.5) && upright.width() == 48.0);
    // Held, the bar is half as thick.
    let p = at(m3.left + 150.0, m3.top + 24.0);
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.wait_for_idle();
    let held = rule.draw();
    assert!(bars(&held, 2.0, 44.0).iter().any(|r| near(r.top, m3.top + 2.0, 0.01)));
    rule.host().pointer_up(p, -1, 0);
}

// ---- C++ range_slider_thumbs_and_track_are_slots --------------------------------------------------------------

fn range_slider_thumbs_and_track_are_slots() {
    let range = mutable_state_of(20.0f32..=60.0);
    let seen = Rc::new(RefCell::new(None::<RangeSliderState>));
    // what each thumb was last given: its value, held
    let thumbs = Rc::new(RefCell::new([(f32::NAN, false); 2]));
    let (r, se, th) = (range.clone(), seen.clone(), thumbs.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (r, se, th) = (r.clone(), se.clone(), th.clone());
        Column(
            content = move || {
                let start = remember(MutableInteractionSource::new);
                let end = remember(MutableInteractionSource::new);
                let (th1, th2, se, s1, s2) = (th.clone(), th.clone(), se.clone(), start.clone(), end.clone());
                SlotRangeSlider(
                    r.get(),
                    set_to(&r),
                    modifier = Modifier.width(210.dp()).test_tag("aralık"),
                    value_range = 0.0..=100.0,
                    start_interaction_source = start,
                    end_interaction_source = end,
                    start_thumb = range_slot(move |s| {
                        th1.borrow_mut()[0] = (s.active_range_start(), s1.collect_is_pressed_as_state().get());
                        Box(Modifier.size(10.dp()).test_tag("baş"));
                    }),
                    end_thumb = range_slot(move |s| {
                        th2.borrow_mut()[1] = (s.active_range_end(), s2.collect_is_pressed_as_state().get());
                        Box(Modifier.size(10.dp()).test_tag("son"));
                    }),
                    track = range_slot(move |s| {
                        *se.borrow_mut() = Some(s);
                        Box(Modifier.fill_max_width().height(6.dp()).test_tag("iz"));
                    }),
                );
                // (C++'s second slider gave one `thumb` for both: M15 has a start and an end thumb only)
            },
        );
    }));
    let (slider, track) = (bounds(&rule, "aralık"), bounds(&rule, "iz"));
    assert!(track.left == slider.left + 5.0 && track.width() == 200.0, "{track:?}"); // half of each thumb off
    assert!(near(bounds(&rule, "baş").left, slider.left + 5.0 + 40.0 - 5.0, 0.01));
    assert!(near(bounds(&rule, "son").left, slider.left + 5.0 + 120.0 - 5.0, 0.01));
    let s = seen.borrow().clone().unwrap();
    assert!(s.active_range_start() == 20.0 && s.active_range_end() == 60.0);
    assert_eq!(*thumbs.borrow(), [(20.0, false), (60.0, false)]);
    // Nearer the end thumb: it is held (its press told a frame after the down), and moves where it is let go.
    let p = at(slider.left + 5.0 + 150.0, slider.top + 24.0);
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.wait_for_idle();
    assert_eq!(*thumbs.borrow(), [(20.0, false), (60.0, true)]);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
    assert!(near(*range.peek().end(), 75.0, 0.5) && !thumbs.borrow()[1].1, "{:?}", range.peek());
}

// ---- C++ remembered_range_slider_state_and_its_material3_track ------------------------------------------------

fn remembered_range_slider_state_and_its_material3_track() {
    let finished = Rc::new(Cell::new(0));
    let kept = Rc::new(RefCell::new(None::<RangeSliderState>));
    let (f, k) = (finished.clone(), kept.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let state = remember_range_slider_state(1.0, 3.0, 3, counting(&f), 0.0..=4.0);
        *k.borrow_mut() = Some(state.clone());
        // (M15's default thumbs and track: SliderDefaults.Thumb and Track(rangeSliderState))
        StateRangeSlider(state, modifier = Modifier.width(208.dp()).test_tag("m3"));
    }));
    // 4 dp bar thumbs: a 204 dp track from 2 dp in. On steps the values sit 8 dp in from its ends: 1 at
    // 8 + 188/4 = 55, 3 at 149; each stretch keeps 2 + 6 from a thumb.
    let b = bounds(&rule, "m3");
    let canvas = rule.draw();
    let bars3 = stretches(&canvas, 16.0);
    assert_eq!(bars3.len(), 3, "{bars3:?}");
    assert!(near(bars3[0].0.left, b.left + 2.0, 0.01) && near(bars3[0].0.right, b.left + 2.0 + 55.0 - 8.0, 0.01));
    assert!(bars3[0].2 && !bars3[0].3);
    assert!(near(bars3[1].0.left, b.left + 2.0 + 149.0 + 8.0, 0.01) && near(bars3[1].0.right, b.left + 206.0, 0.01));
    assert!(!bars3[1].2 && bars3[1].3);
    assert!(near(bars3[2].0.left, b.left + 2.0 + 55.0 + 8.0, 0.01));
    assert!(near(bars3[2].0.right, b.left + 2.0 + 149.0 - 8.0, 0.01));
    assert!(!bars3[2].2 && !bars3[2].3);
    let dot_at = |x: f32| dots(&canvas).iter().any(|d| near(*d, x, 0.01));
    // a stop dot at each end, the middle step's tick
    assert!(
        dot_at(b.left + 2.0 + 8.0) && dot_at(b.left + 206.0 - 8.0) && dot_at(b.left + 2.0 + 102.0),
        "{:?}",
        dots(&canvas)
    );
    let bar_at = |x: f32| bars(&canvas, 4.0, 44.0).iter().any(|r| near(r.left + 2.0, x, 0.01));
    assert!(bar_at(b.left + 2.0 + 55.0) && bar_at(b.left + 2.0 + 149.0)); // the thumbs on their steps' ticks
    let kept = kept.borrow().clone().unwrap();
    tap(&rule, at(b.left + 2.0 + 5.0, b.top + 24.0)); // by the start: the start thumb goes to the first step
    rule.wait_for_idle();
    assert!(kept.active_range_start() == 0.0 && kept.active_range_end() == 3.0 && finished.get() == 1);
    // From code: each end on steps, kept on its side of the other (C++ set both at once and ordered them).
    kept.set_active_range_end(3.6);
    kept.set_active_range_start(2.0);
    rule.wait_for_idle();
    assert!(kept.active_range_start() == 2.0 && kept.active_range_end() == 4.0);
    kept.set_active_range_start(4.5);
    rule.wait_for_idle();
    assert_eq!(kept.active_range_start(), 4.0);
}

// ---- C++ labels_show_over_their_content_while_held_or_always ----------------------------------------------------

/// A box of `width` by `height` dp tagged `tag`.
#[kompozable]
fn Tagged(tag: String, width: i32, height: i32) {
    Box(Modifier.size_wh(width.dp(), height.dp()).test_tag(tag));
}

/// (a label is in a popup: its tags are in the unmerged tree, its bounds in the root's space)
fn shown(rule: &KompozTestRule, tag: &str) -> Option<Rect> {
    let nodes = rule.on_all_nodes_with(kompoz_test::has_test_tag(tag), true).fetch_semantics_nodes();
    (!nodes.is_empty()).then(|| rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root())
}

fn labels_show_over_their_content_while_held_or_always() {
    // M15's slider has no label of its own: a thumb slot shows one as Compose's sample does, a Label of the
    // slider's interaction source around SliderDefaults.Thumb (C++'s SliderDefaults::Label(state, …)).
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Column(
            modifier = Modifier.padding_hv(0.dp(), 100.dp()),
            content = move || {
                Label(
                    |_| Tagged("etiket".into(), 40, 20),
                    modifier = Modifier.test_tag("içerik"),
                    interaction_source = Some(s.clone()),
                    content = || Tagged("içerik kutusu".into(), 100, 30),
                );
                Label(
                    |_| Tagged("hep".into(), 10, 10),
                    is_persistent = true,
                    content = || Tagged("hep kutusu".into(), 30, 30),
                );
                let slider_source = remember(MutableInteractionSource::new);
                let thumb_source = slider_source.clone();
                SlotSlider(
                    0.5,
                    |_| {},
                    modifier = Modifier.width(200.dp()).test_tag("kaydırıcı"),
                    interaction_source = slider_source,
                    thumb = slot(move |_| {
                        let thumb_source = thumb_source.clone();
                        Label(
                            |_| Tagged("değer".into(), 30, 16),
                            interaction_source = Some(thumb_source.clone()),
                            content = move || SliderDefaults.Thumb(thumb_source.clone()),
                        );
                    }),
                );
            },
        );
    }));
    assert!(shown(&rule, "etiket").is_none() && shown(&rule, "hep").is_some() && shown(&rule, "değer").is_none());
    let press = PressInteraction::Press::new(at(5.0, 5.0));
    source.try_emit(press); // pressed: above it, centered, 4 dp apart
    rule.wait_for_idle();
    let content = shown(&rule, "içerik").unwrap();
    let label = shown(&rule, "etiket").expect("shown while pressed");
    assert!(near(label.left, content.left + 30.0, 0.01) && near(label.bottom, content.top - 4.0, 0.01), "{label:?}");
    source.try_emit(PressInteraction::Release::new(press));
    rule.wait_for_idle();
    assert!(shown(&rule, "etiket").is_none());
    // A slider's thumb: while it is held, over the thumb (at 0.5 its middle is the slider's).
    let slider = bounds(&rule, "kaydırıcı");
    rule.host().pointer_down(at(slider.left + 100.0, slider.top + 24.0), -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.wait_for_idle();
    let value = shown(&rule, "değer").expect("shown while held");
    assert!(near(value.left + 15.0, slider.left + 100.0, 0.5) && value.bottom < slider.top + 4.0, "{value:?}");
    rule.host().pointer_up(at(slider.left + 100.0, slider.top + 24.0), -1, 0);
    rule.wait_for_idle();
    assert!(shown(&rule, "değer").is_none());
}

// ---- C++ vertical_slider_grows_upwards_and_follows_a_vertical_drag ----------------------------------------------

fn vertical_slider_grows_upwards_and_follows_a_vertical_drag() {
    // M15's VerticalSlider takes a state only, and its value grows from the top down — up from the bottom with
    // `reverse_direction` (the C++'s grew upwards, reversed downwards): the places are flipped here. Its thumb is a
    // 44 by 4 dp bar lying across it (2 dp thick while held), so its track runs from 2 dp in (1 dp held): a tap at y
    // is the fraction (y - 1) / 218 from the top. A press alone doesn't set the value (the C++'s did): Compose's tap
    // sets it as it ends, a drag by what the finger moved past the touch slop.
    let value = SliderState::new(0.25, 0, None, 0.0..=1.0);
    let reversed = SliderState::new(0.5, 3, None, 0.0..=1.0);
    let (v, r) = (value.clone(), reversed.clone());
    let cs = light_color_scheme();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (v, r) = (v.clone(), r.clone());
            Row(
                modifier = Modifier.height(220.dp()),
                content = move || {
                    VerticalSlider(v.clone(), modifier = Modifier.test_tag("dikey"));
                    VerticalSlider(r.clone(), modifier = Modifier.test_tag("ters"), reverse_direction = true);
                },
            );
        }),
        IntSize::new(400, 600),
    );
    // 48 dp to touch
    assert_eq!(bounds(&rule, "dikey"), Rect::new(0.0, 0.0, 48.0, 220.0));
    assert_eq!(bounds(&rule, "ters"), Rect::new(48.0, 0.0, 96.0, 220.0));
    // The bar thumb drawn in the column at x: its middle.
    let thumb_of = |x: f32| {
        let c = rule.draw();
        c.rects
            .iter()
            .zip(&c.rect_colors)
            .filter(|(r, color)| {
                near(r.width(), 44.0, 0.01)
                    && near(r.height(), 4.0, 0.01)
                    && near(r.center().x, x, 0.01)
                    && **color == cs.primary
            })
            .map(|(r, _)| r.center())
            .next_back()
            .unwrap_or(Offset::new(-1.0, -1.0))
    };
    // The track runs from 2 dp in at the top (the minimum) to 2 dp in at the bottom; reversed, the other way.
    assert!(near(thumb_of(24.0).y, 2.0 + 216.0 * 0.25, 0.01), "{:?}", thumb_of(24.0));
    assert!(near(thumb_of(72.0).y, 218.0 - 216.0 * 0.5, 0.01), "{:?}", thumb_of(72.0));
    let held_y = |fraction: f32| 1.0 + 218.0 * fraction;
    tap(&rule, at(22.0, held_y(0.75))); // a tap sets it: 3/4 of the way down
    rule.wait_for_idle();
    assert!(near(value.value(), 0.75, 0.01), "{}", value.value());
    // A vertical drag moves it.
    let from = held_y(0.25);
    rule.host().pointer_down(at(22.0, from), -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        rule.host().pointer_move(at(22.0, from + 10.0 * i as f32), -1, 0);
    }
    rule.host().pointer_up(at(22.0, from + 100.0), -1, 0);
    rule.wait_for_idle();
    let dragged = 0.25 + (100.0 - Slop) / 218.0;
    assert!(near(value.value(), dragged, 0.01), "{}", value.value());
    // Sideways does nothing (the press, let go off the slider, is no tap).
    rule.host().pointer_down(at(22.0, 100.0), -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        rule.host().pointer_move(at(22.0 + 10.0 * i as f32, 100.0), -1, 0);
    }
    rule.host().pointer_up(at(122.0, 100.0), -1, 0);
    rule.wait_for_idle();
    assert!(near(value.value(), dragged, 0.01), "{}", value.value());
    // Reversed, with steps: 0.3 up from the bottom is on the nearest of five, 0.25.
    tap(&rule, at(70.0, 219.0 - 218.0 * 0.3));
    rule.wait_for_idle();
    assert!(near(reversed.value(), 0.25, 0.01), "{}", reversed.value());
    // It grows upwards: a drag up from there by 100 dp less the slop is 0.67 of it, on its step 0.75.
    let from = 219.0 - 218.0 * 0.25;
    rule.host().pointer_down(at(70.0, from), -1, 0, PointerType::Touch);
    for i in 1..=10 {
        rule.advance_frame();
        rule.host().pointer_move(at(70.0, from - 10.0 * i as f32), -1, 0);
    }
    rule.host().pointer_up(at(70.0, from - 100.0), -1, 0);
    rule.wait_for_idle();
    assert!(near(reversed.value(), 0.75, 0.01), "{}", reversed.value());
}

// ---- C++ semantics_ranges_live_regions_custom_actions_and_order: its Material sliders ---------------------------

fn semantics_ranges_live_regions_custom_actions_and_order() {
    // (the rest is kompoz-ui's semantics.rs, its progress indicators the progress tests') Compose gives a slider
    // no role (accessibility tells it a seek bar by its set-progress action) and says its value as its state. A
    // disabled one keeps its set-progress action in its semantics, as Compose's does, but accessibility isn't
    // offered it and can't perform it (Compose's accessibility delegate; kompoz-ui's host does the same).
    let value = mutable_state_of(50.0f32);
    let finished = Rc::new(Cell::new(0));
    let (v, f) = (value.clone(), finished.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, f) = (v.clone(), f.clone());
        Column(move || {
            Slider(
                v.get(),
                set_to(&v),
                modifier = Modifier.test_tag("ayar"),
                value_range = 0.0..=100.0,
                steps = 3,
                on_value_change_finished = counting(&f),
            );
            Slider(0.5, |_| {}, modifier = Modifier.test_tag("kapalı"), enabled = false);
        });
    }));
    let node = |tag: &str| {
        rule.host()
            .accessibility_tree()
            .into_iter()
            .find(|n| n.test_tag.as_deref() == Some(tag))
            .unwrap_or_else(|| panic!("{tag}"))
    };
    // A slider: its value in its range with its steps; set to 70 it snaps to 75 (the steps are 25 apart).
    let slider = node("ayar");
    assert!(slider.role.is_none() && slider.settable && slider.enabled);
    assert_eq!(slider.range, Some(ProgressBarRangeInfo::new(50.0, 0.0..=100.0, 3)));
    assert_eq!(slider.state_description, "50.0");
    assert!(rule.host().perform_semantics_action(slider.id, SemanticsAction::SetProgress(70.0)));
    rule.wait_for_idle();
    assert!(value.peek() == 75.0 && finished.get() == 1);
    assert_eq!(node("ayar").range.map(|r| r.current), Some(75.0));
    // Set to what it is: not done (Android's AbsSeekBar's rule).
    assert!(!rule.host().perform_semantics_action(slider.id, SemanticsAction::SetProgress(74.0)));
    assert_eq!(finished.get(), 1);
    // Disabled: shown, not set.
    let off = node("kapalı");
    assert!(!off.settable && !off.enabled && off.range.is_some());
    assert!(rule.on_node_with_tag("kapalı").fetch_semantics_node().config().contains(SemanticsActions::SetProgress));
    assert!(!rule.host().perform_semantics_action(off.id, SemanticsAction::SetProgress(0.2)));
}

// ---- Compose's behaviour --------------------------------------------------------------------------------------

/// Presses `key` on what has the focus and lets go (Compose's `pressKey`).
fn press(rule: &KompozTestRule, key: Key) {
    rule.on_root().perform_key_input(|k| k.press_key_for(key, 50));
    rule.wait_for_idle();
}

fn slider_keys_step_the_value_and_finish_on_release() {
    // Compose's slideOnKeyEvents: focused, the arrows move a slider by 1% of its range (a step with steps), Page
    // Up/Down by a tenth of that many (at least one, at most ten), Home/End to its ends; a change finishes as the
    // key is let go. Right to left the arrows go the other way (Page Up/Down don't); standing up, Down grows it
    // (Up when reversed), Page Down too.
    let value = mutable_state_of(50.0f32);
    let stepped = mutable_state_of(50.0f32);
    let rtl = mutable_state_of(0.0f32);
    let finished = Rc::new(Cell::new(0));
    let upright = SliderState::new(0.5, 0, None, 0.0..=1.0);
    let reversed = SliderState::new(0.5, 0, None, 0.0..=1.0);
    let (v, st, r, f, u, rv) =
        (value.clone(), stepped.clone(), rtl.clone(), finished.clone(), upright.clone(), reversed.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, st, r, f, u, rv) = (v.clone(), st.clone(), r.clone(), f.clone(), u.clone(), rv.clone());
        Column(move || {
            Slider(
                v.get(),
                set_to(&v),
                modifier = Modifier.test_tag("düz"),
                value_range = 0.0..=100.0,
                on_value_change_finished = counting(&f),
            );
            Slider(st.get(), set_to(&st), modifier = Modifier.test_tag("adımlı"), value_range = 0.0..=100.0, steps = 3);
            let r = r.clone();
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
                Slider(r.get(), set_to(&r), modifier = Modifier.test_tag("sağdan"), value_range = 0.0..=100.0);
            });
            Row(
                modifier = Modifier.height(200.dp()),
                content = move || {
                    VerticalSlider(u.clone(), modifier = Modifier.test_tag("dikey"));
                    VerticalSlider(rv.clone(), modifier = Modifier.test_tag("ters"), reverse_direction = true);
                },
            );
        });
    }));
    rule.on_node_with_tag("düz").request_focus();
    press(&rule, Key::DirectionRight);
    assert!(value.peek() == 51.0 && finished.get() == 1, "{} {}", value.peek(), finished.get());
    press(&rule, Key::DirectionLeft);
    press(&rule, Key::DirectionLeft);
    assert_eq!(value.peek(), 49.0);
    press(&rule, Key::PageUp); // 100 steps of 1: a page is 10
    assert_eq!(value.peek(), 59.0);
    press(&rule, Key::PageDown);
    press(&rule, Key::PageDown);
    assert_eq!(value.peek(), 39.0);
    press(&rule, Key::MoveEnd);
    assert_eq!(value.peek(), 100.0);
    press(&rule, Key::DirectionRight); // held at its end
    assert_eq!(value.peek(), 100.0);
    press(&rule, Key::MoveHome);
    assert!(value.peek() == 0.0 && finished.get() == 9, "{}", finished.get());
    // A key that isn't its: no change, nothing finished.
    press(&rule, Key::DirectionUp);
    assert!(value.peek() == 0.0 && finished.get() == 9);
    // Steps: an arrow is a step (25), a page is one too (4 steps over ten is under one page).
    rule.on_node_with_tag("adımlı").request_focus();
    press(&rule, Key::DirectionRight);
    assert_eq!(stepped.peek(), 75.0);
    press(&rule, Key::PageDown);
    assert_eq!(stepped.peek(), 50.0);
    // Right to left: Right takes from it, Page Up still adds.
    rule.on_node_with_tag("sağdan").request_focus();
    press(&rule, Key::DirectionLeft);
    assert_eq!(rtl.peek(), 1.0);
    press(&rule, Key::DirectionRight);
    assert_eq!(rtl.peek(), 0.0);
    press(&rule, Key::PageUp);
    assert_eq!(rtl.peek(), 10.0);
    // Standing up: Down grows it, Up takes from it; reversed the other way; Left and Right aren't its.
    rule.on_node_with_tag("dikey").request_focus();
    press(&rule, Key::DirectionDown);
    assert!(near(upright.value(), 0.51, 1e-5), "{}", upright.value());
    press(&rule, Key::PageUp);
    assert!(near(upright.value(), 0.41, 1e-5), "{}", upright.value());
    press(&rule, Key::DirectionRight);
    assert!(near(upright.value(), 0.41, 1e-5));
    rule.on_node_with_tag("ters").request_focus();
    press(&rule, Key::DirectionUp);
    assert!(near(reversed.value(), 0.51, 1e-5), "{}", reversed.value());
    press(&rule, Key::PageDown);
    assert!(near(reversed.value(), 0.41, 1e-5), "{}", reversed.value());
}

fn range_slider_thumbs_take_their_own_keys_and_set_progress() {
    // Each of a range slider's thumbs is a node of its own to accessibility ("Range start", "Range end": its
    // value in the range up to the other thumb, set-progress keeping it on its side) and to the keyboard (its own
    // focus and arrows; right to left reversed).
    let range = mutable_state_of(20.0f32..=80.0);
    let finished = Rc::new(Cell::new(0));
    let (r, f) = (range.clone(), finished.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        RangeSlider(
            r.get(),
            set_to(&r),
            modifier = Modifier.width(204.dp()),
            value_range = 0.0..=100.0,
            on_value_change_finished = counting(&f),
        );
    }));
    let node = |label: &str| {
        rule.host().accessibility_tree().into_iter().find(|n| n.label == label).unwrap_or_else(|| panic!("{label}"))
    };
    let (start, end) = (node("Range start"), node("Range end"));
    assert_eq!(start.range, Some(ProgressBarRangeInfo::new(20.0, 0.0..=80.0, 0)));
    assert_eq!(end.range, Some(ProgressBarRangeInfo::new(80.0, 20.0..=100.0, 0)));
    assert!(start.settable && end.settable && start.state_description == "20.0");
    // Its bounds: the thumb's, 10 dp wider each side for a screen reader.
    assert!(near(start.bounds.width(), 4.0 + 20.0, 0.01), "{:?}", start.bounds);
    assert!(rule.host().perform_semantics_action(start.id, SemanticsAction::SetProgress(30.0)));
    rule.wait_for_idle();
    assert!(range.peek() == (30.0..=80.0) && finished.get() == 1, "{:?}", range.peek());
    // Past the end thumb: up to it.
    assert!(rule.host().perform_semantics_action(start.id, SemanticsAction::SetProgress(95.0)));
    rule.wait_for_idle();
    assert_eq!(range.peek(), 80.0..=80.0);
    assert_eq!(node("Range end").range, Some(ProgressBarRangeInfo::new(80.0, 80.0..=100.0, 0)));
    assert!(rule.host().perform_semantics_action(node("Range end").id, SemanticsAction::SetProgress(90.0)));
    rule.wait_for_idle();
    assert_eq!(range.peek(), 80.0..=90.0);
    // The keyboard: each thumb its own arrows.
    rule.on_node_with_content_description("Range start").request_focus();
    press(&rule, Key::DirectionLeft);
    assert_eq!(range.peek(), 79.0..=90.0);
    press(&rule, Key::DirectionRight);
    press(&rule, Key::DirectionRight); // (up to the end thumb at most)
    assert_eq!(range.peek(), 81.0..=90.0);
    rule.on_node_with_content_description("Range end").request_focus();
    press(&rule, Key::MoveEnd);
    assert_eq!(range.peek(), 81.0..=100.0);
    press(&rule, Key::MoveHome); // its start: the start thumb
    assert_eq!(range.peek(), 81.0..=81.0);
}

fn rtl_sliders_drag_and_step_from_the_right() {
    // Right to left a slider's track starts at the right (Compose's draggable reversed, `onPress` from the right):
    // a drag to the left grows it, its steps counted from the right; the active stretch is at the right.
    let value = mutable_state_of(0.0f32);
    let stepped = mutable_state_of(0.0f32);
    let (v, st) = (value.clone(), stepped.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, st) = (v.clone(), st.clone());
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
            let (v, st) = (v.clone(), st.clone());
            Column(move || {
                Slider(
                    v.get(),
                    set_to(&v),
                    modifier = Modifier.width(202.dp()).test_tag("a"),
                    value_range = 0.0..=100.0,
                );
                Slider(
                    st.get(),
                    set_to(&st),
                    modifier = Modifier.width(202.dp()).test_tag("b"),
                    value_range = 0.0..=100.0,
                    steps = 3,
                );
            });
        });
    }));
    let a = bounds(&rule, "a");
    // (held, x = right - 1 - 2v)
    let rx_of = |v: f32| a.right - 1.0 - 2.0 * v;
    swipe(&rule, at(rx_of(25.0), a.top + 24.0), at(rx_of(75.0), a.top + 24.0));
    rule.wait_for_idle();
    assert!(near(value.peek(), 75.0 - Slop / 2.0, 0.01), "{}", value.peek());
    swipe(&rule, at(rx_of(75.0), a.top + 24.0), at(rx_of(55.0), a.top + 24.0)); // to the right: less
    rule.wait_for_idle();
    assert!(near(value.peek(), 55.0 + Slop / 2.0, 0.01), "{}", value.peek());
    let b = bounds(&rule, "b");
    tap(&rule, at(b.right - 1.0 - 2.0 * 30.0, b.top + 24.0));
    rule.wait_for_idle();
    assert_eq!(stepped.peek(), 25.0);
    // The active stretch: from the track's right end (2 dp in) to 8 dp short of the thumb — on steps a value sits
    // 8 dp in from the 198 dp track's ends: 25 at 8 + 182 / 4 = 53.5 from its right end.
    let canvas = rule.draw();
    let cs = light_color_scheme();
    let active: Vec<_> = stretches(&canvas, 16.0)
        .into_iter()
        .filter(|s| s.1 == cs.primary && near(s.0.top, b.top + 16.0, 0.01))
        .collect();
    assert_eq!(active.len(), 1, "{active:?}");
    assert!(
        near(active[0].0.right, b.right - 2.0, 0.01) && near(active[0].0.left, b.right - 2.0 - 53.5 + 8.0, 0.01),
        "{active:?} {b:?}"
    );
    assert!(!active[0].2 && active[0].3, "{active:?}"); // its outside corner at the right
}

fn disabled_sliders_take_no_input_and_keep_their_set_progress() {
    // Not enabled: no taps, drags or keys (not focusable), the disabled colors; its set-progress stays in its
    // semantics (Compose's) but accessibility can't use it (the semantics test).
    let value = mutable_state_of(50.0f32);
    let range = mutable_state_of(20.0f32..=80.0);
    let colors = Rc::new(RefCell::new(None::<SliderColors>));
    let (v, r, c) = (value.clone(), range.clone(), colors.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, r) = (v.clone(), r.clone());
        *c.borrow_mut() = Some(SliderDefaults.colors());
        Column(move || {
            Slider(
                v.get(),
                set_to(&v),
                modifier = Modifier.width(202.dp()).test_tag("kapalı"),
                enabled = false,
                value_range = 0.0..=100.0,
            );
            RangeSlider(
                r.get(),
                set_to(&r),
                modifier = Modifier.width(204.dp()).test_tag("aralık"),
                enabled = false,
                value_range = 0.0..=100.0,
            );
        });
    }));
    let colors = colors.borrow().unwrap();
    let off = bounds(&rule, "kapalı");
    tap(&rule, at(off.left + x_of(10.0), off.top + 24.0));
    swipe(&rule, at(off.left + x_of(50.0), off.top + 24.0), at(off.left + x_of(90.0), off.top + 24.0));
    rule.wait_for_idle();
    assert_eq!(value.peek(), 50.0);
    let ranged = bounds(&rule, "aralık");
    tap(&rule, at(ranged.left + rx(10.0), ranged.top + 24.0));
    swipe(&rule, at(ranged.left + rx(80.0), ranged.top + 24.0), at(ranged.left + rx(95.0), ranged.top + 24.0));
    rule.wait_for_idle();
    assert_eq!(range.peek(), 20.0..=80.0);
    // Not focusable: nothing to take keys.
    let config = rule.on_node_with_tag("kapalı").fetch_semantics_node().config();
    assert!(!config.contains(SemanticsActions::RequestFocus));
    // Its colors: the disabled ones (the thumb, both stretches).
    let canvas = rule.draw();
    let thumbs = canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .filter(|(r, _)| near(r.width(), 4.0, 0.01) && near(r.height(), 44.0, 0.01));
    assert!(thumbs.clone().count() >= 3 && thumbs.clone().all(|(_, c)| *c == colors.disabled_thumb_color));
    let own: Vec<_> =
        stretches(&canvas, 16.0).into_iter().filter(|s| near(s.0.top, off.top + 16.0, 0.01)).map(|s| s.1).collect();
    assert_eq!(own, [colors.disabled_inactive_track_color, colors.disabled_active_track_color]);
    assert!(colors.disabled_thumb_color != colors.thumb_color);
}

fn slider_state_snaps_to_its_steps_unless_told_not_to() {
    // Compose's SliderState: its value set is coerced into the range and snapped to the nearest step — unless
    // `should_auto_snap` is off (a custom animation to the step); with `on_value_change` set, the slider gives it
    // the values instead of setting them; `is_dragging` while a drag lasts; `coerced_value_as_fraction`.
    let state = SliderState::new(0.0, 3, None, 0.0..=1.0);
    state.set_value(0.3);
    assert_eq!(state.value(), 0.25);
    state.set_value(7.0);
    assert_eq!(state.value(), 1.0);
    state.set_should_auto_snap(false);
    state.set_value(0.3);
    assert!(state.value() == 0.3 && !state.should_auto_snap());
    state.set_value(1.5); // (not coerced either: only snapping coerces)
    assert_eq!(state.coerced_value_as_fraction(), 1.0);
    state.set_should_auto_snap(true);
    state.set_value(0.6);
    assert_eq!(state.value(), 0.5);
    let given = Rc::new(RefCell::new(Vec::new()));
    let g = given.clone();
    let s = state.clone();
    state.set_on_value_change(Some(Rc::new(move |v| g.borrow_mut().push((v, s.is_dragging())))));
    let st = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        StateSlider(st.clone(), modifier = Modifier.width(202.dp()));
    }));
    swipe(&rule, at(x_of(50.0), 24.0), at(x_of(90.0), 24.0));
    rule.wait_for_idle();
    // (the drag: 80 dp less the slop is 0.86 of it, the nearest step 1.0 — given, not set)
    assert_eq!(state.value(), 0.5);
    let given = given.borrow().clone();
    assert!(!given.is_empty() && given.iter().all(|(v, dragging)| *dragging && (*v == 0.75 || *v == 1.0)), "{given:?}");
    assert!(!state.is_dragging());
    tap(&rule, at(x_of(0.0), 24.0));
    rule.wait_for_idle();
    assert_eq!(state.value(), 0.5);
}

fn slider_colors_come_from_the_scheme_and_copy_what_is_given() {
    // Compose's SliderDefaults.colors(): the scheme's colors by SliderTokens (an active tick in the secondary
    // container on the primary stretch, an inactive one in the primary — each stretch's ticks the other's color;
    // disabled the on-surface faded over the surface); colors(…) puts each given color in its place; copy keeps
    // what is unspecified.
    let seen = Rc::new(RefCell::new(None));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *s.borrow_mut() = Some((
            SliderDefaults.colors(),
            SliderDefaults.colors(thumb_color = Color::Red, inactive_tick_color = Color::Blue),
        ));
    }));
    let _ = &rule;
    let (default, given) = seen.borrow().unwrap();
    let cs = light_color_scheme();
    assert!(default.thumb_color == cs.primary && default.active_track_color == cs.primary);
    assert!(default.inactive_track_color == cs.secondary_container);
    assert!(default.active_tick_color == cs.secondary_container && default.inactive_tick_color == cs.primary);
    let faded_on_surface = |alpha: f32| named_args!(cs.on_surface.copy(alpha = alpha)).composite_over(cs.surface);
    assert_eq!(default.disabled_thumb_color, faded_on_surface(0.38));
    // (the stretches' not over the surface)
    assert_eq!(default.disabled_active_track_color, named_args!(cs.on_surface.copy(alpha = 0.38)));
    assert_eq!(default.disabled_inactive_track_color, named_args!(cs.on_surface.copy(alpha = 0.12)));
    assert!(given.thumb_color == Color::Red && given.inactive_tick_color == Color::Blue);
    assert!(
        given.active_track_color == default.active_track_color
            && given == named_args!(default.copy(thumb_color = Color::Red, inactive_tick_color = Color::Blue))
    );
    let copied = named_args!(default.copy(active_track_color = Color::Green));
    assert!(copied.active_track_color == Color::Green && copied.thumb_color == default.thumb_color);
}

/// What a track's slots were called with: (stop indicator offsets, tick offsets and colors).
type Drawn = Rc<RefCell<(Vec<Offset>, Vec<(Offset, Color)>)>>;

/// What they were called with, each once (each frame drawn calls them again), by x.
fn deduped(drawn: &Drawn) -> (Vec<Offset>, Vec<(Offset, Color)>) {
    let (mut stops, mut ticks) = drawn.borrow().clone();
    stops.sort_by(|a, b| a.x.total_cmp(&b.x));
    stops.dedup();
    ticks.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
    ticks.dedup();
    (stops, ticks)
}

fn tracks_draw_their_stop_indicators_and_ticks_as_told() {
    // Compose's drawTrack: a stop indicator at the inactive stretch's end, 8 dp in (its corner size); the ticks of
    // the steps between the corners, those on a stop indicator or in the thumb's gap left out, each in its
    // stretch's tick color. `draw_stop_indicator = null` draws none (and the end tick comes back); a tick drawn as
    // given.
    let drawn: Drawn = Rc::default();
    let colors = Rc::new(RefCell::new(None::<SliderColors>));
    let (d, c) = (drawn.clone(), colors.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *c.borrow_mut() = Some(SliderDefaults.colors());
        let d = d.clone();
        Column(move || {
            let d = d.clone();
            let state = remember(|| SliderState::new(0.5, 3, None, 0.0..=1.0));
            let (d1, d2) = (d.clone(), d.clone());
            SlotSlider(
                0.5,
                |_| {},
                modifier = Modifier.width(204.dp()).test_tag("adımlı"),
                steps = 3,
                track = slot(move |s| {
                    let (d1, d2) = (d1.clone(), d2.clone());
                    let stop: DrawStopIndicator =
                        Rc::new(move |_: &mut DrawScope, o: Offset| d1.borrow_mut().0.push(o));
                    let tick: DrawTick =
                        Rc::new(move |_: &mut DrawScope, o: Offset, c: Color| d2.borrow_mut().1.push((o, c)));
                    SliderDefaults.Track(s, draw_stop_indicator = Some(Some(stop)), draw_tick = tick);
                }),
            );
            let _ = state;
            StateSlider(
                remember(|| SliderState::new(0.5, 0, None, 0.0..=1.0)),
                modifier = Modifier.width(204.dp()).test_tag("yalın"),
                track = slot(|s| SliderDefaults.Track(s, draw_stop_indicator = Some(None))),
            );
        });
    }));
    let canvas = rule.draw();
    let colors = colors.borrow().unwrap();
    let (stops, ticks) = deduped(&drawn);
    // A 200 dp track, 8 dp corners: the ticks at 8 + 184 × (0, ¼, ½, ¾, 1); the thumb at ½ (100) takes 100 ± 8,
    // the stop indicator the last.
    assert_eq!(stops, [Offset::new(192.0, 8.0)]);
    let at_x: Vec<(f32, Color)> = ticks.iter().map(|(o, c)| (o.x, *c)).collect();
    assert_eq!(
        at_x,
        [(8.0, colors.active_tick_color), (54.0, colors.active_tick_color), (146.0, colors.inactive_tick_color)]
    );
    assert!(ticks.iter().all(|(o, _)| o.y == 8.0));
    // No stop indicator: no dot on the plain one.
    let plain = bounds(&rule, "yalın");
    let plain_dots = canvas
        .paths
        .iter()
        .chain(&canvas.rects)
        .filter(|p| {
            near(p.width(), 4.0, 0.01) && near(p.height(), 4.0, 0.01) && p.top > plain.top && p.bottom < plain.bottom
        })
        .count();
    assert_eq!(plain_dots, 0);
}

fn centered_and_corner_sized_tracks_as_compose() {
    // Compose's Expressive tracks. CenteredTrack: the active stretch from the middle to the thumb (an inactive one
    // each side, a stop indicator at each end). Track(trackCornerSize): its outside corners and its stop
    // indicator's place by that size.
    let drawn: Drawn = Rc::default();
    let d = drawn.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let d = d.clone();
        Column(move || {
            StateSlider(
                remember(|| SliderState::new(25.0, 0, None, -50.0..=50.0)),
                modifier = Modifier.width(204.dp()).test_tag("ortalı"),
                track = slot(|s| SliderDefaults.CenteredTrack(s)),
            );
            let d = d.clone();
            StateSlider(
                remember(|| SliderState::new(0.5, 0, None, 0.0..=1.0)),
                modifier = Modifier.width(204.dp()).test_tag("köşeli"),
                track = slot(move |s| {
                    let d = d.clone();
                    let stop: DrawStopIndicator = Rc::new(move |_: &mut DrawScope, o: Offset| d.borrow_mut().0.push(o));
                    SliderDefaults.CornerSizeTrack(
                        s,
                        12.dp(),
                        modifier = Modifier.height(36.dp()),
                        draw_stop_indicator = Some(Some(stop)),
                    );
                }),
            );
        });
    }));
    let canvas = rule.draw();
    let cs = light_color_scheme();
    let b = bounds(&rule, "ortalı");
    let centered: Vec<_> = stretches(&canvas, 16.0).into_iter().filter(|s| near(s.0.top, b.top + 16.0, 0.01)).collect();
    // A 200 dp track from 2 dp in; 25 of -50..50 is ¾: the thumb at 150. Inactive 0..94 (6 short of the middle:
    // a slider's start has no thumb, its gap only the 6 dp), active 100..142 (from the middle, 8 short of the
    // thumb), inactive 158..200.
    let spans: Vec<(f32, f32, bool)> =
        centered.iter().map(|s| (s.0.left - b.left - 2.0, s.0.right - b.left - 2.0, s.1 == cs.primary)).collect();
    assert_eq!(spans, [(0.0, 94.0, false), (158.0, 200.0, false), (100.0, 142.0, true)], "{centered:?}");
    assert!(!centered[2].2 && !centered[2].3); // the active one's corners: inside ones both
    assert!(centered[0].2 && centered[1].3); // the track's ends round
    // The corner-sized one: 36 dp high, its stop indicator 12 dp in from its end.
    let k = bounds(&rule, "köşeli");
    assert_eq!(deduped(&drawn).0, [Offset::new(200.0 - 12.0, 18.0)]);
    let tall = stretches(&canvas, 36.0);
    assert!(tall.len() == 2 && tall.iter().all(|s| s.0.top >= k.top), "{tall:?}");
}

#[allow(deprecated)]
fn the_deprecated_positions_track_draws_lines_and_points() {
    // Compose's deprecated Track(sliderPositions): the inactive line the whole width, the active one over the
    // active range (round caps), the ticks as points — inside the range in the active tick color, outside in the
    // inactive one.
    let colors = Rc::new(RefCell::new(None::<SliderColors>));
    let made = Rc::new(RefCell::new(None::<(SliderPositions, SliderPositions)>));
    let (c, m) = (colors.clone(), made.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *c.borrow_mut() = Some(SliderDefaults.colors());
        let positions = remember(|| SliderPositions(0.25..=0.75, vec![0.0, 0.5, 1.0]));
        *m.borrow_mut() = Some((positions.clone(), SliderPositions(0.25..=0.75, vec![0.0, 0.5, 1.0])));
        Box(Modifier.width(200.dp()), content = move || SliderDefaults.PositionsTrack(positions.clone()));
    }));
    let (positions, same) = made.borrow().clone().unwrap();
    let colors = colors.borrow().unwrap();
    let canvas = rule.draw();
    // (a line is drawn as a path, its bounds the line's; the points of a group as one path)
    let lines: Vec<(Rect, Color)> = canvas.paths.iter().copied().zip(canvas.path_colors.iter().copied()).collect();
    assert_eq!(
        lines,
        [
            (Rect::new(0.0, 8.0, 200.0, 8.0), colors.inactive_track_color),
            (Rect::new(50.0, 8.0, 150.0, 8.0), colors.active_track_color),
            // the ticks outside the range (0 and 1) first, as the first tick is; 4 dp round points
            (Rect::new(-2.0, 6.0, 202.0, 10.0), colors.inactive_tick_color),
            (Rect::new(98.0, 6.0, 102.0, 10.0), colors.active_tick_color),
        ]
    );
    assert!(positions == same);
    assert_eq!(positions.active_range(), 0.25..=0.75);
}

fn slider_states_are_saved_and_restored() {
    // rememberSliderState and rememberRangeSliderState are saved with the host (Compose's Savers: the value or
    // the ends, and the steps; the callback and the range given again).
    let saved = |value: f32, start: f32, end: f32| {
        let shown = mutable_state_of(true);
        let kept = Rc::new(RefCell::new(None::<(SliderState, RangeSliderState)>));
        let (sh, k) = (shown.clone(), kept.clone());
        let content = kompozable_lambda!(move || {
            if sh.get() {
                let one = remember_slider_state(0.0, 4, None, 0.0..=10.0);
                let two = remember_range_slider_state(1.0, 9.0, 0, None, 0.0..=10.0);
                *k.borrow_mut() = Some((one, two));
            }
        });
        (shown, kept, content, value, start, end)
    };
    let bytes = {
        let (_, kept, content, ..) = saved(0.0, 0.0, 0.0);
        let rule = KompozTestRule::new(content);
        let (one, two) = kept.borrow().clone().unwrap();
        one.set_value(6.0);
        two.set_active_range_start(3.0);
        two.set_active_range_end(7.5);
        rule.wait_for_idle();
        rule.host().save_state()
    };
    let (shown, kept, content, ..) = saved(0.0, 0.0, 0.0);
    shown.set(false);
    let rule = KompozTestRule::new(content);
    rule.host().restore_state(&bytes);
    shown.set(true);
    rule.wait_for_idle();
    let (one, two) = kept.borrow().clone().unwrap();
    assert!(one.value() == 6.0 && one.steps() == 4 && one.value_range() == (0.0..=10.0));
    assert!(two.active_range_start() == 3.0 && two.active_range_end() == 7.5 && two.steps() == 0);
}

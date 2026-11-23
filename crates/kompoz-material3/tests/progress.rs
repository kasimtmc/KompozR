//! Material's progress indicators as Compose's material3 1.5 has them: LinearProgressIndicator and
//! CircularProgressIndicator (a progress read as they draw, their endless forms, the deprecated value forms),
//! Expressive's wavy ones, ProgressIndicatorDefaults and WavyProgressIndicatorDefaults — how they draw (gaps to
//! their tracks, stop indicators, caps, right to left), how they animate (and what they ask for meanwhile), what
//! they tell accessibility. The C++ Kompoz's tests by name (kompoz_tests.cpp; the progress half of
//! `semantics_ranges_live_regions_custom_actions_and_order`, whose slider half is tests/slider.rs and the rest
//! kompoz-ui's), and Compose's behaviour.
//!
//! The recording canvas keeps each path's bounds as Skia's (its control points and a last lone move too) and draws
//! transforms as they come (a path inside a rotation is recorded unrotated), so the tests measure what a path draws
//! by walking it (`drawn`), turning it where the indicator turned it. A round-capped line is a path here (its
//! bounds the line, the caps half its width further): the C++ recorded the stroke's rect.
//!
//! The C++ targeted Compose 1.4's indicators and its own wavy ones; where M15 differs, the tests follow M15 and say
//! so. M15's wavy indicators run their wave whether drawn or not (the linear one's offset animation starts as its
//! node is attached), and Compose's test rule (KompozR's too) cancels endless animations while its clock advances on
//! its own: the tests that watch them turn that off first.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_ui::draw::DrawScope;
use kompoz_ui::platform::{LocalDensity, LocalLayoutDirection};
use kompoz_ui::semantics::{ProgressBarRangeInfo, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    circular_progress_keeps_a_gap_to_its_track_and_turns_in_six_seconds,
    rtl_linear_progress_fills_from_the_right,
    an_endless_indicator_asks_only_for_drawing,
    a_wavy_indicators_wave_rises_as_it_is_drawn,
    wavy_progress_waves_in_the_middle_and_keeps_a_gap_to_its_track,
    wavy_amplitude_rises_and_falls_on_their_own_curves,
    indeterminate_wavy_progress_chases_with_its_track_in_the_gaps,
    semantics_ranges_live_regions_custom_actions_and_order_progress,
    a_wavy_wave_rising_composes_nothing_around_it,
    determinate_progress_is_kept_between_0_and_1,
    the_gap_and_the_stop_indicator_follow_the_stroke_cap,
    a_custom_stop_indicator_or_none,
    circular_gap_without_round_caps_and_other_widths,
    indeterminate_linear_progress_chases_with_two_lines,
    progress_indicator_defaults_as_compose,
    the_deprecated_value_forms_draw_as_the_lambda_ones,
    wavy_progress_indicator_defaults_as_compose,
    wavy_indicators_skip_transparent_parts_and_take_their_strokes,
    progress_indicators_semantics_bounds_and_ranges,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn same_rect(a: Rect, b: Rect, tolerance: f32) -> bool {
    near(a.left, b.left, tolerance)
        && near(a.top, b.top, tolerance)
        && near(a.width(), b.width(), tolerance)
        && near(a.height(), b.height(), tolerance)
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

/// Each contour of the recorded path `i` as what it draws (sampled along it, not its control points), in the
/// canvas's coordinates (unturned).
fn drawn_contours(c: &RecordingCanvas, i: usize) -> Vec<Rect> {
    let shape = &c.path_shapes[i];
    let b = shape.get_bounds();
    let shift = Offset::new(c.paths[i].left - b.left, c.paths[i].top - b.top);
    let mut measure = PathMeasure::new();
    measure.set_path(shape, false);
    let mut contours = Vec::new();
    loop {
        let length = measure.length();
        if length > 0.0 {
            let mut r: Option<Rect> = None;
            for k in 0..=400 {
                let p = measure.get_position(length * k as f32 / 400.0) + shift;
                r = Some(match r {
                    None => Rect::new(p.x, p.y, p.x, p.y),
                    Some(r) => Rect::new(r.left.min(p.x), r.top.min(p.y), r.right.max(p.x), r.bottom.max(p.y)),
                });
            }
            contours.extend(r);
        }
        if !measure.next_contour() {
            break;
        }
    }
    contours
}

/// What the recorded path `i` draws, all its contours.
fn drawn(c: &RecordingCanvas, i: usize) -> Rect {
    drawn_contours(c, i)
        .into_iter()
        .reduce(|a, b| Rect::new(a.left.min(b.left), a.top.min(b.top), a.right.max(b.right), a.bottom.max(b.bottom)))
        .unwrap_or(Rect::Zero)
}

/// A rect turned half round about `pivot_x` across (what a 180° rotation about the middle of a line does to it).
fn mirrored(r: Rect, pivot_x: f32) -> Rect {
    Rect::new(2.0 * pivot_x - r.right, r.top, 2.0 * pivot_x - r.left, r.bottom)
}

/// What the recorded path `i` draws turned `degrees` about `pivot`.
fn drawn_turned(c: &RecordingCanvas, i: usize, pivot: Offset, degrees: f32) -> Rect {
    let shape = &c.path_shapes[i];
    let b = shape.get_bounds();
    let shift = Offset::new(c.paths[i].left - b.left, c.paths[i].top - b.top);
    let (sin, cos) = degrees.to_radians().sin_cos();
    let mut measure = PathMeasure::new();
    measure.set_path(shape, false);
    let length = measure.length();
    let mut r: Option<Rect> = None;
    for k in 0..=400 {
        let p = measure.get_position(length * k as f32 / 400.0) + shift - pivot;
        let q = Offset::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos) + pivot;
        r = Some(match r {
            None => Rect::new(q.x, q.y, q.x, q.y),
            Some(r) => Rect::new(r.left.min(q.x), r.top.min(q.y), r.right.max(q.x), r.bottom.max(q.y)),
        });
    }
    r.unwrap_or(Rect::Zero)
}

/// The angle (degrees) a recorded rotation turns by.
fn rotation_of(m: &Matrix) -> f32 {
    m.get(0, 1).atan2(m.get(0, 0)).to_degrees()
}

/// A rule whose clock moves only when the test moves it: endless animations run (Compose's
/// `mainClock.autoAdvance = false`).
fn rule_with_animations(content: impl Content + 'static) -> KompozTestRule {
    let rule = KompozTestRule::empty();
    rule.main_clock().set_auto_advance(false);
    rule.set_content(content);
    rule
}

fn range(rule: &KompozTestRule, tag: &str) -> Option<ProgressBarRangeInfo> {
    rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::ProgressBarRangeInfo)
}

// ---- The C++ Kompoz's tests --------------------------------------------------------------------------------------

fn circular_progress_keeps_a_gap_to_its_track_and_turns_in_six_seconds() {
    // (the C++'s `CircularProgressIndicator(0.25f)`: M15's value form is deprecated and keeps the deprecated
    // transparent track, so the lambda form, whose track is the secondary container)
    let spinning = mutable_state_of(false);
    let s = spinning.clone();
    let rule = rule_with_animations(kompozable_lambda!(move || {
        let s = s.clone();
        Column(
            content = move || {
                CircularProgressIndicator(progress = || 0.25);
                if s.get() {
                    IndeterminateCircularProgressIndicator();
                }
            },
        );
    }));
    let cs = light_color_scheme();
    let canvas = rule.draw();
    // Compose's: from 12 o'clock a quarter, then a gap (4 dp and the round caps: 8 dp of a 40 dp circle, 22.9°),
    // the track round to the gap before the start. (Compose's angles: 12 o'clock is 270°.)
    let gap = 8.0 / (std::f32::consts::PI * 40.0) * 360.0;
    assert_eq!(canvas.arcs.len(), 2);
    let (track, arc) = (&canvas.arcs[0], &canvas.arcs[1]);
    assert!(track.color == cs.secondary_container && near(track.start, 270.0 + 90.0 + gap, 0.1));
    assert!(near(track.sweep, 270.0 - 2.0 * gap, 0.1));
    assert!(arc.color == cs.primary && near(arc.start, 270.0, 0.1) && near(arc.sweep, 90.0, 0.1));
    // The arc's circle is the stroke's middle: 2 dp in.
    assert!(same_rect(arc.oval, rect(2.0, 2.0, 36.0, 36.0), 0.01));

    // Indeterminate: its track transparent; its arc 10% long at the start, growing to 87% by 3 s; turned 1080° in
    // 6 s and 90° more in 300 ms every 1.5 s (a rotation of the drawing, as Compose's `rotate`).
    spinning.set(true);
    rule.advance_frame(); // (an endless animation: never idle)
    rule.advance_frame();
    let turn = |c: &RecordingCanvas| rotation_of(c.transforms.last().expect("the spinner's rotation"));
    let first = rule.draw();
    let first_arc = *first.arcs.last().unwrap();
    assert!(first_arc.color == cs.primary && near(first_arc.sweep, 36.0, 3.0));
    assert!(first.arcs[first.arcs.len() - 2].color == Color::Transparent);
    // Frame by frame (a turn's angle known only within a round), past the first jump.
    let mut turned = 0.0;
    let mut last = turn(&first);
    for _ in 0..100 {
        rule.advance_frame();
        let now = turn(&rule.draw());
        turned += (now - last + 540.0).rem_euclid(360.0) - 180.0;
        last = now;
    }
    let later = rule.draw();
    // 1600 ms: 288° of the global turn and 120° of the second quarter jump (from 90° at 1.5 s), less what the first
    // frames had turned.
    let expected = 1600.0 / 6000.0 * 1080.0 + 120.0;
    assert!(turned > expected - 40.0 && turned < expected + 5.0, "{turned}");
    assert!(later.arcs.last().unwrap().sweep > first_arc.sweep);
}

fn rtl_linear_progress_fills_from_the_right() {
    let spinning = mutable_state_of(false);
    let s = spinning.clone();
    let rule = rule_with_animations(kompozable_lambda!(move || {
        let s = s.clone();
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
            let s = s.clone();
            Column(
                Modifier.width(200.dp()),
                content = move || {
                    LinearProgressIndicator(progress = || 0.25, modifier = Modifier.fill_max_width());
                    if s.get() {
                        IndeterminateLinearProgressIndicator(Modifier.fill_max_width());
                    }
                },
            );
        });
    }));
    let cs = light_color_scheme();
    let canvas = rule.draw();
    // Compose's: the track from 29% (past a 4 dp gap and the round caps) to the left end, a quarter at the right (the
    // lines' ends 2 dp in: their round caps reach the ends; the C++'s rects were the strokes', {0, 0, 144, 4} and
    // {148, 0, 52, 4}), the stop at the left end (drawn at the right, mirrored).
    assert_eq!(canvas.paths.len(), 2);
    assert!(
        same_rect(canvas.paths[0], rect(2.0, 2.0, 140.0, 0.0), 0.01) && canvas.path_colors[0] == cs.secondary_container
    );
    assert!(same_rect(canvas.paths[1], rect(150.0, 2.0, 48.0, 0.0), 0.01) && canvas.path_colors[1] == cs.primary);
    assert!(canvas.path_strokes.iter().all(|&w| w == 4.0));
    assert!(canvas.path_stroke_styles.iter().all(|s| s.cap == StrokeCap::Round));
    assert_eq!(canvas.rects.len(), 1);
    let mirror = canvas.transforms.last().expect("the stop's mirroring");
    assert!(same_rect(mirror.map_rect(canvas.rects[0]), rect(0.0, 0.0, 4.0, 4.0), 0.01));
    assert!(canvas.rect_colors[0] == cs.primary && near(canvas.rect_radii[0], 2.0, 0.01));

    spinning.set(true);
    rule.advance_time_by(500); // line 1's head on its way from the right, its tail just off
    let moving = rule.draw();
    let line = (0..moving.paths.len()).any(|i| {
        let p = moving.paths[i];
        p.top >= 3.5 && moving.path_colors[i] == cs.primary && p.right > 190.0 && p.left > 100.0
    });
    assert!(line);
}

fn an_endless_indicator_asks_only_for_drawing() {
    // M15's spinner is Compose's rememberInfiniteTransition: its animation runs from the start whether it is drawn
    // or not (the C++'s asked for frames only once drawn), the values read as it draws — it is neither composed nor
    // laid out again.
    let composed = Rc::new(Cell::new(0));
    let c = composed.clone();
    let rule = rule_with_animations(kompozable_lambda!(move || {
        c.set(c.get() + 1);
        IndeterminateCircularProgressIndicator(Modifier.test_tag("dönen"));
    }));
    let scopes = rule.host().stats().scopes_run;
    assert!(rule.host().has_pending_work()); // running, drawn or not (the C++: nothing until drawn)
    for _ in 0..3 {
        let _ = rule.draw();
        assert!(rule.host().has_pending_work()); // drawn: the next frame draws it on
        rule.advance_frame();
    }
    let relaid_out = rule.host().relaid_out_count();
    rule.advance_frame();
    assert_eq!(rule.host().relaid_out_count(), relaid_out); // ...nor laid out
    assert_eq!(composed.get(), 1); // neither composed again...
    // (the transition's own scope composes once more as it starts running, as Compose's InfiniteTransition.run)
    assert!(rule.host().stats().scopes_run <= scopes + 1);
    rule.on_node_with_tag("dönen").assert_exists();
}

fn a_wavy_indicators_wave_rises_as_it_is_drawn() {
    // (the C++ gave the progress as a value read in composition; M15 takes a lambda, read as it draws: setting the
    // progress composes nothing at all) The wave's amplitude animates in the node, read as it draws: nothing is
    // composed as it rises.
    let progress = mutable_state_of(0.0f32);
    let p = progress.clone();
    let rule = rule_with_animations(kompozable_lambda!(move || {
        let p = p.clone();
        LinearWavyProgressIndicator(progress = move || p.get());
    }));
    let _ = rule.draw();
    progress.set(0.5); // (a wave from none)
    rule.advance_frame();
    let _ = rule.draw();
    let ran = rule.host().stats().scopes_run;
    let height = |c: &RecordingCanvas| (0..c.paths.len()).map(|i| drawn(c, i).height()).fold(0.0f32, f32::max);
    let mut heights = Vec::new();
    for _ in 0..10 {
        rule.advance_frame();
        heights.push(height(&rule.draw()));
    }
    assert_eq!(rule.host().stats().scopes_run, ran); // not even the indicator composed again
    assert!(heights.first().unwrap() < heights.last().unwrap() && *heights.last().unwrap() > 1.0);
}

fn wavy_progress_waves_in_the_middle_and_keeps_a_gap_to_its_track() {
    let progress = mutable_state_of(0.0f32);
    let rtl = mutable_state_of(false);
    let (p, r) = (progress.clone(), rtl.clone());
    let rule = rule_with_animations(kompozable_lambda!(move || {
        let (p, r) = (p.clone(), r.clone());
        let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
            let p = p.clone();
            Column(
                Modifier.width(200.dp()),
                content = move || {
                    let (p1, p2) = (p.clone(), p.clone());
                    LinearWavyProgressIndicator(progress = move || p1.get(), modifier = Modifier.fill_max_width()); // 200 x 10
                    CircularWavyProgressIndicator(progress = move || p2.get()); // 48 x 48 below
                },
            );
        });
    }));
    let cs = light_color_scheme();
    let (primary, track) = (cs.primary, cs.secondary_container);
    let flat = rule.draw();
    assert_eq!(flat.paths.len(), 2); // the tracks alone (an empty progress draws nothing)
    assert!(same_rect(drawn(&flat, 0), rect(2.0, 5.0, 196.0, 0.0), 0.01) && flat.path_colors[0] == track);
    assert_eq!(flat.path_strokes[0], 4.0);
    // A whole flat circle: M15 scales the shapes to the size less the stroke by their bounds (a polygon's control
    // points a little outside its curve: 42.9 dp across; C++: 40.8, less its wave too).
    let ring = drawn(&flat, 1);
    assert!(
        flat.path_colors[1] == track && near(ring.width(), 42.9, 0.1) && near(ring.height(), 42.9, 0.1),
        "{ring:?}"
    );
    assert!(near(ring.center().x, 24.0, 0.1) && near(ring.center().y, 34.0, 0.1));
    assert!(flat.rects.len() == 1 && same_rect(flat.rects[0], rect(196.0, 3.0, 4.0, 4.0), 0.01)); // the stop
    assert!(flat.rect_colors[0] == primary);
    // M15's linear indicator moves its wave from the start, flat or not (the C++: nothing waves, no work).
    assert!(rule.host().has_pending_work());

    progress.set(0.5);
    rule.advance_frame();
    let _ = rule.draw(); // the amplitude's new target is seen as it draws
    rule.advance_time_by(100);
    let rising = rule.draw();
    assert_eq!(rising.paths.len(), 4);
    let rising_height = drawn(&rising, 1).height();
    assert!(rising_height > 0.5 && rising_height < 5.0, "{rising_height}"); // the wave grows in
    rule.advance_time_by(500);
    let half = rule.draw();
    assert_eq!(half.paths.len(), 4);
    // The track past the middle, 8 dp after the progress's end (4 dp of gap, the caps' 2 dp each).
    let track_parts = drawn_contours(&half, 0);
    assert!(track_parts.len() == 1 && same_rect(track_parts[0], rect(108.0, 5.0, 90.0, 0.0), 0.01));
    assert!(half.path_colors[0] == track);
    // The progress from 2 dp in to the middle (C++: to 98, its caps short of the middle) waving 3 dp up and down
    // (the height less the stroke, halved by the quadratic curves).
    let wave = drawn(&half, 1);
    assert!(near(wave.left, 2.0, 0.5) && near(wave.right, 100.0, 0.5) && half.path_colors[1] == primary, "{wave:?}");
    assert!(wave.height() > 5.5 && wave.height() < 6.01, "{wave:?}");
    // The circle: its track the left half, its progress the right half (from 12 o'clock at its middle, 24).
    let (circle_track, circle_progress) = (drawn(&half, 2), drawn(&half, 3));
    assert!(half.path_colors[2] == track && circle_track.right < 20.0 && near(circle_track.left, 2.55, 0.1));
    assert!(half.path_colors[3] == primary && circle_progress.left > 22.0 && circle_progress.right > 44.6);
    assert!(circle_progress.right < 48.01, "{circle_progress:?}");
    assert!(half.rects.len() == 1 && same_rect(half.rects[0], rect(196.0, 3.0, 4.0, 4.0), 0.01));
    // (the C++ found "50%" texts: Android's accessibility says the percentage; the semantics give the range)
    let half_range = kompoz_test::has_progress_bar_range_info(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0));
    assert_eq!(rule.on_all_nodes_with(half_range, false).fetch_semantics_nodes().len(), 2);
    rule.advance_time_by(100);
    let later = rule.draw();
    assert!(later.paths.len() == 4 && later.path_shapes[1] != half.path_shapes[1]); // the wave moves along
    assert!(near(drawn(&later, 1).left, 2.0, 0.5));
    assert!(later.path_shapes[0] == half.path_shapes[0]); // the track doesn't

    progress.set(0.97);
    rule.advance_frame();
    let _ = rule.draw();
    rule.advance_time_by(600);
    let full = rule.draw();
    // No room for the linear track nor the stop (the progress past them); the circle's track a sliver.
    // (the track's path only moves there: Compose draws it, and nothing shows)
    let linear: Vec<usize> =
        (0..full.paths.len()).filter(|&i| full.paths[i].top < 10.0 && !drawn_contours(&full, i).is_empty()).collect();
    assert_eq!(linear.len(), 1, "{:?}", full.paths);
    let line = drawn(&full, linear[0]);
    assert!(near(line.height(), 0.0, 0.01) && near(line.right, 194.0, 0.01), "{line:?}"); // flat again
    // The stop still whole at 97% (M15's shrinks as the progress's end, its cap's room in, passes its start; the
    // C++ had none): smaller past it, gone at the end.
    assert!(full.rects.len() == 1 && same_rect(full.rects[0], rect(196.0, 3.0, 4.0, 4.0), 0.01));
    let ends = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                for p in [0.985f32, 0.99] {
                    LinearWavyProgressIndicator(progress = move || p, modifier = Modifier.width(200.dp()));
                }
            },
        );
    }));
    let ends = ends.draw();
    assert!(ends.rects.len() == 1 && same_rect(ends.rects[0], rect(199.0, 4.5, 1.0, 1.0), 0.01), "{:?}", ends.rects);
    let circle_progress = (0..full.paths.len()).find(|&i| full.paths[i].top > 10.0 && full.path_colors[i] == primary);
    assert!(near(drawn(&full, circle_progress.unwrap()).width(), 42.9, 0.2));

    progress.set(0.5);
    rtl.set(true);
    rule.advance_frame();
    let _ = rule.draw();
    rule.advance_time_by(600);
    let mirrored_canvas = rule.draw();
    assert_eq!(mirrored_canvas.paths.len(), 4);
    // Right to left the linear one is drawn turned half round about its middle (x = 100).
    let turned = mirrored_canvas.transforms.iter().any(|m| near(rotation_of(m).abs(), 180.0, 0.01));
    assert!(turned);
    assert!(same_rect(mirrored(drawn(&mirrored_canvas, 0), 100.0), rect(2.0, 5.0, 90.0, 0.0), 0.01)); // the track on the left
    let wave = mirrored(drawn(&mirrored_canvas, 1), 100.0);
    assert!(near(wave.left, 100.0, 0.5) && near(wave.right, 198.0, 0.5)); // from the right (C++: from 102)
    let circle_progress = drawn(&mirrored_canvas, 3);
    assert!(circle_progress.left > 22.0 && circle_progress.right > 44.6); // the circle still goes clockwise
    assert!(same_rect(mirrored(mirrored_canvas.rects[0], 100.0), rect(0.0, 3.0, 4.0, 4.0), 0.01));

    // Its own amplitude, wavelength and no speed: half the wave, standing; a whole number of waves round the circle.
    let custom = rule_with_animations(kompozable_lambda!(|| {
        Column(
            content = || {
                LinearWavyProgressIndicator(
                    progress = || 1.0,
                    modifier = Modifier.width(200.dp()),
                    amplitude = Rc::new(|_: f32| 0.5f32) as AmplitudeFunction,
                    wavelength = 20.dp(),
                    wave_speed = 0.dp(),
                );
                CircularWavyProgressIndicator(
                    progress = || 1.0,
                    amplitude = Rc::new(|_: f32| 1.0f32) as AmplitudeFunction,
                    wave_speed = 0.dp(),
                );
            },
        );
    }));
    let still = custom.draw();
    let visible: Vec<usize> = (0..still.paths.len()).filter(|&i| !drawn_contours(&still, i).is_empty()).collect();
    assert_eq!(visible.len(), 2); // (the linear track's path only moves)
    let wave = drawn(&still, visible[0]);
    assert!(near(wave.height(), 3.0, 0.05), "{wave:?}"); // half the wave
    // Standing: its first drawing sets the amplitude it starts at (Compose's cache sets `amplitudeState` as it is
    // built: a frame more); then nothing moves.
    custom.wait_for_idle();
    custom.draw();
    assert!(!custom.host().has_pending_work());
    // The ring's progress at full amplitude: the star of as many points as 15 dp waves fill (2π·22 / 15 → 9).
    let star = drawn(&still, visible[1]);
    assert!(star.width() > 42.0 && star.width() <= 44.01, "{star:?}");
    // A whole number of waves round: the whole progress (a segment of the star, so not closed) ends where it
    // began, at the top.
    let turn = still.path_shapes[visible[1]].commands();
    let (first, last) = (turn[0].end().unwrap(), turn.last().unwrap().end().unwrap());
    assert!(turn.len() > 3 && near(first.x, last.x, 0.01) && near(first.y, last.y, 0.01), "{first:?} {last:?}");
}

fn wavy_amplitude_rises_and_falls_on_their_own_curves() {
    let big = mutable_state_of(false);
    let b = big.clone();
    let rule = rule_with_animations(kompozable_lambda!(move || {
        let b = b.clone();
        LinearWavyProgressIndicator(
            progress = || 0.5,
            modifier = Modifier.width(200.dp()),
            amplitude = Rc::new(move |_: f32| if b.get() { 1.0f32 } else { 0.0 }) as AmplitudeFunction,
        );
    }));
    // The wave's height: twice its amplitude (3 dp at most).
    let wave = |rule: &KompozTestRule| {
        let c = rule.draw();
        (0..c.paths.len()).map(|i| drawn(&c, i).height()).fold(0.0f32, f32::max)
    };
    let _ = wave(&rule);
    big.set(true);
    rule.advance_frame();
    let _ = wave(&rule); // (the amplitude function is read as the indicator draws: the new target seen there)
    rule.advance_time_by(125);
    let rising = wave(&rule); // the standard easing: well on its way at a quarter
    rule.advance_time_by(600);
    big.set(false);
    rule.advance_frame();
    let _ = wave(&rule);
    rule.advance_time_by(125);
    let falling = wave(&rule); // the emphasized accelerating one: hardly gone at a quarter
    assert!(rising > 2.5 && rising < 5.0 && falling > 5.3, "{rising} {falling}");
}

fn indeterminate_wavy_progress_chases_with_its_track_in_the_gaps() {
    let shown = mutable_state_of(false);
    let s = shown.clone();
    let rule = rule_with_animations(kompozable_lambda!(move || {
        if s.get() {
            Column(
                Modifier.width(200.dp()),
                content = || {
                    IndeterminateLinearWavyProgressIndicator(Modifier.fill_max_width());
                    IndeterminateCircularWavyProgressIndicator();
                },
            );
        }
    }));
    let cs = light_color_scheme();
    let (primary, track) = (cs.primary, cs.secondary_container);
    shown.set(true);
    let (mut two_lines, mut track_frames, mut arc_frames) = (0, 0, 0);
    let mut arc_starts = std::collections::BTreeSet::new();
    for _ in 0..110 {
        // a whole 1750 ms round
        rule.advance_frame();
        let c = rule.draw();
        let turn = c.transforms.last().map_or(0.0, rotation_of);
        let (mut lines, mut tracks, mut arcs, mut arc_tracks) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for k in 0..c.paths.len() {
            let linear = c.paths[k].top < 10.0; // (a wave's bounds reach its control points, 1 dp past the bar)
            let active = c.path_colors[k] == primary;
            assert!(active || c.path_colors[k] == track);
            match (linear, active) {
                (true, true) => lines.push(drawn(&c, k)),
                (true, false) => tracks.extend(drawn_contours(&c, k)),
                (false, true) => arcs.push(drawn_turned(&c, k, Offset::new(24.0, 34.0), turn)),
                (false, false) => arc_tracks.push(k),
            }
        }
        two_lines += (lines.len() == 2) as i32;
        track_frames += (!tracks.is_empty()) as i32;
        for l in &lines {
            if l.width() > 40.0 {
                assert!(l.height() > 5.5, "{l:?}"); // waving
            }
            for t in &tracks {
                // Round ends 2 dp each, a 4 dp gap: 8 dp — less as a line's head just comes in from the start (M15
                // shortens the gap to the head less the cap's room there; the C++ kept 8).
                let apart = (t.left - l.right).max(l.left - t.right);
                assert!(apart >= 8.0f32.min(l.right + 2.0) - 0.6, "{t:?} {l:?}");
            }
        }
        assert!(lines.len() + tracks.len() > 0 && lines.len() <= 2);
        if arcs.len() == 1 && arc_tracks.len() == 1 {
            arc_frames += 1;
            arc_starts.insert((arcs[0].left * 10.0).round() as i32);
        }
    }
    assert!(two_lines > 5 && track_frames > 100, "{two_lines} {track_frames}");
    assert!(arc_frames == 110 && arc_starts.len() > 20, "{arc_frames} {}", arc_starts.len()); // the arc always has its track, and turns
}

fn semantics_ranges_live_regions_custom_actions_and_order_progress() {
    // The progress half (the slider half is tests/slider.rs, the rest kompoz-ui's semantics.rs): a determinate
    // indicator tells its progress in 0..1, an endless one an indeterminate progress.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                LinearProgressIndicator(progress = || 0.3, modifier = Modifier.test_tag("yükleme"));
                IndeterminateCircularProgressIndicator(Modifier.test_tag("bekleme"));
            },
        );
    }));
    let node = |tag: &str| {
        rule.host()
            .accessibility_tree()
            .into_iter()
            .find(|n| n.test_tag.as_deref() == Some(tag))
            .unwrap_or_else(|| panic!("{tag}"))
    };
    assert_eq!(node("yükleme").range, Some(ProgressBarRangeInfo::new(0.3, 0.0..=1.0, 0)));
    assert_eq!(node("bekleme").range, Some(ProgressBarRangeInfo::Indeterminate));
    // Neither can be set.
    assert!(!node("yükleme").settable && !node("bekleme").settable);
}

fn a_wavy_wave_rising_composes_nothing_around_it() {
    // The wavy indicator's part of C++ overlays_fields_and_indicators_animate_composing_around_them_only_as_they_turn (whole in
    // cross_area.rs; the wave's heights checked here): its wave rises from none composing nothing around it. (The C++ gave `o ? 0.5f : 0.f` as a value read in composition;
    // M15 takes a lambda, read as it draws: turning it composes nothing at all.)
    let on = mutable_state_of(false);
    let composed = Rc::new(Cell::new(0));
    let (o, c) = (on.clone(), composed.clone());
    let rule = rule_with_animations(kompozable_lambda!(move || {
        c.set(c.get() + 1);
        let o = o.clone();
        Column(
            content = move || {
                let o = o.clone();
                LinearWavyProgressIndicator(progress = move || if o.get() { 0.5 } else { 0.0 });
            },
        );
    }));
    let _ = rule.draw();
    let before = composed.get();
    on.set(true);
    let mut heights = Vec::new();
    for _ in 0..40 {
        rule.advance_frame();
        let c = rule.draw();
        heights.push((0..c.paths.len()).map(|i| drawn(&c, i).height()).fold(0.0f32, f32::max));
    }
    assert_eq!(composed.get(), before); // not on the way
    assert!(heights[2] < 1.0 && *heights.last().unwrap() > 5.5, "{heights:?}"); // the wave rose
}

// (C++ graphics_forms_take_compose_named_arguments: its wavy indicators, with the rest of its Material part, in both
// syntaxes in crates/kompoz/tests/named_forms.rs)

// ---- Compose's behaviour ------------------------------------------------------------------------------------------

fn determinate_progress_is_kept_between_0_and_1() {
    // Compose's: the progress coerced into 0..1 as it draws and as it is told; NaN told as 0 (the range would throw).
    let value = mutable_state_of(1.5f32);
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v = v.clone();
        Column(
            content = move || {
                let (a, b, c, d) = (v.clone(), v.clone(), v.clone(), v.clone());
                LinearProgressIndicator(
                    progress = move || a.get(),
                    modifier = Modifier.width(200.dp()).test_tag("çizgi"),
                );
                CircularProgressIndicator(progress = move || b.get(), modifier = Modifier.test_tag("halka"));
                LinearWavyProgressIndicator(progress = move || c.get(), modifier = Modifier.test_tag("dalga"));
                CircularWavyProgressIndicator(
                    progress = move || d.get(),
                    modifier = Modifier.test_tag("dalgalı halka"),
                );
            },
        );
    }));
    let all = ["çizgi", "halka", "dalga", "dalgalı halka"];
    for tag in all {
        assert_eq!(range(&rule, tag), Some(ProgressBarRangeInfo::new(1.0, 0.0..=1.0, 0)), "{tag}");
    }
    let canvas = rule.draw();
    // Over: the whole bar, no track (no room past the end); the whole ring (a path: a whole circle isn't an arc
    // to the backend), its track's sweep less than none (Compose draws it backwards, under the ring).
    assert!(same_rect(canvas.paths[0], rect(2.0, 2.0, 196.0, 0.0), 0.01));
    assert!(near(drawn(&canvas, 1).width(), 36.0, 0.1) && canvas.arcs.len() == 1 && canvas.arcs[0].sweep < 0.0);
    value.set(-0.5);
    rule.wait_for_idle();
    for tag in all {
        assert_eq!(range(&rule, tag), Some(ProgressBarRangeInfo::new(0.0, 0.0..=1.0, 0)), "{tag}");
    }
    value.set(f32::NAN);
    rule.wait_for_idle();
    for tag in all {
        assert_eq!(range(&rule, tag), Some(ProgressBarRangeInfo::new(0.0, 0.0..=1.0, 0)), "{tag}");
    }
}

fn the_gap_and_the_stop_indicator_follow_the_stroke_cap() {
    // Compose's: with butt caps the gap is the gap alone (with round ones the caps' room is added), the stop a square;
    // with round ones a dot. A track taller than wide falls back to butt lines.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                LinearProgressIndicator(
                    progress = || 0.5,
                    modifier = Modifier.width(200.dp()),
                    stroke_cap = StrokeCap::Butt,
                );
                LinearProgressIndicator(
                    progress = || 0.5,
                    modifier = Modifier.width(200.dp()).height(8.dp()),
                    gap_size = 10.dp(),
                    stroke_cap = StrokeCap::Square,
                );
                CircularProgressIndicator(progress = || 0.25, stroke_cap = StrokeCap::Butt, gap_size = 0.dp());
            },
        );
    }));
    let cs = light_color_scheme();
    let canvas = rule.draw();
    // Butt: the track from 50% + 4 dp to the very end, the progress from the start to the middle, a 4 dp square at
    // the end.
    assert!(
        same_rect(canvas.paths[0], rect(104.0, 2.0, 96.0, 0.0), 0.01)
            && canvas.path_colors[0] == cs.secondary_container
    );
    assert!(same_rect(canvas.paths[1], rect(0.0, 2.0, 100.0, 0.0), 0.01) && canvas.path_colors[1] == cs.primary);
    assert!(canvas.path_stroke_styles[0].cap == StrokeCap::Butt);
    assert!(same_rect(canvas.rects[0], rect(196.0, 0.0, 4.0, 4.0), 0.01) && canvas.rect_radii[0] == 0.0);
    // Square caps, 8 dp tall (the backend's lines): the gap is 10 dp and the stroke's 8, the lines 4 dp in (half
    // the stroke); the stop is a 4 dp square 2 dp from the end (as far as from the top), in the middle.
    let ((t0, t1), (l0, l1)) = (canvas.lines[0], canvas.lines[1]);
    assert!(near(t0.x, 118.0, 0.01) && near(t1.x, 196.0, 0.01) && near(t0.y, 8.0, 0.01));
    assert!(near(l0.x, 4.0, 0.01) && near(l1.x, 100.0, 0.01) && canvas.line_colors[1] == cs.primary);
    assert!(same_rect(canvas.rects[1], rect(194.0, 6.0, 4.0, 4.0), 0.01) && canvas.rect_radii[1] == 0.0);
    // The ring of butt caps with no gap: the track from where the quarter ends round to 12 o'clock.
    let arcs = &canvas.arcs;
    assert!(near(arcs[0].start, 360.0, 0.01) && near(arcs[0].sweep, 270.0, 0.01));
}

fn a_custom_stop_indicator_or_none() {
    // Compose's `drawStopIndicator` slot: any drawing (here a 6 dp square at the start), or none at all.
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        Column(
            content = move || {
                let c = c.clone();
                LinearProgressIndicator(
                    progress = || 0.5,
                    modifier = Modifier.width(200.dp()),
                    draw_stop_indicator = Some(Rc::new(move |d: &mut DrawScope| {
                        c.set(c.get() + 1);
                        ProgressIndicatorDefaults.draw_stop_indicator(d, 6.dp(), Color::Red, StrokeCap::Butt);
                    }) as DrawStopIndicatorLambda),
                );
                LinearProgressIndicator(
                    progress = || 0.5,
                    modifier = Modifier.width(200.dp()),
                    draw_stop_indicator = Some(Rc::new(|_: &mut DrawScope| {}) as DrawStopIndicatorLambda),
                );
            },
        );
    }));
    let before = calls.get();
    let canvas = rule.draw();
    assert_eq!(calls.get(), before + 1);
    // The stop is no taller than the track: 4 dp, red, square.
    assert!(canvas.rects.len() == 1 && same_rect(canvas.rects[0], rect(196.0, 0.0, 4.0, 4.0), 0.01));
    assert!(canvas.rect_colors[0] == Color::Red);
}

fn circular_gap_without_round_caps_and_other_widths() {
    // Compose's: the gap's angle is the gap (and the stroke width with round caps) over the circumference; a wider
    // stroke draws its circle further in.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                CircularProgressIndicator(progress = || 0.5, stroke_width = 8.dp(), stroke_cap = StrokeCap::Butt);
                CircularProgressIndicator(
                    progress = || 0.5,
                    modifier = Modifier.size(80.dp()),
                    stroke_width = 8.dp(),
                    track_color = Color::Green,
                    color = Color::Blue,
                );
            },
        );
    }));
    let arcs = rule.draw().arcs;
    let gap = |dp: f32, size: f32| dp / (std::f32::consts::PI * size) * 360.0;
    assert!(
        near(arcs[0].start, 270.0 + 180.0 + gap(4.0, 40.0), 0.01)
            && near(arcs[0].sweep, 180.0 - 2.0 * gap(4.0, 40.0), 0.01)
    );
    assert!(same_rect(arcs[0].oval, rect(4.0, 4.0, 32.0, 32.0), 0.01) && arcs[0].stroke_width == 8.0);
    assert!(near(arcs[2].start, 270.0 + 180.0 + gap(12.0, 80.0), 0.01) && arcs[2].color == Color::Green);
    assert!(arcs[3].color == Color::Blue && same_rect(arcs[3].oval, rect(4.0, 44.0, 72.0, 72.0), 0.01));
}

fn indeterminate_linear_progress_chases_with_two_lines() {
    // Compose's keyframes: line 1's head from 0 to 1 in the first second (emphasized accelerating), its tail 250 ms
    // later; line 2's head from 650 ms, its tail from 900 ms (850 ms each); a round 1.75 s. The track in the gaps.
    let rule = rule_with_animations(kompozable_lambda!(|| {
        IndeterminateLinearProgressIndicator(Modifier.width(200.dp()));
    }));
    let cs = light_color_scheme();
    let mut saw_two_lines = false;
    let mut saw_one_line = false;
    for _ in 0..110 {
        rule.advance_frame();
        let c = rule.draw();
        let lines: Vec<Rect> =
            (0..c.paths.len()).filter(|&i| c.path_colors[i] == cs.primary).map(|i| c.paths[i]).collect();
        let tracks: Vec<Rect> =
            (0..c.paths.len()).filter(|&i| c.path_colors[i] == cs.secondary_container).map(|i| c.paths[i]).collect();
        assert!(lines.len() <= 2 && !tracks.is_empty());
        saw_two_lines |= lines.len() == 2;
        saw_one_line |= lines.len() == 1;
        // The track keeps 8 dp from each line (4 dp of gap, the caps' 2 dp each), wherever both are drawn whole.
        for l in &lines {
            for t in &tracks {
                // (a line or a track whose ends both lie within a cap's room of an end is drawn at the room's edge,
                // the other placed by its unkept ends)
                let squeezed = |r: &Rect| r.right <= 2.01 || r.left >= 197.99;
                let apart = (t.left - l.right).max(l.left - t.right);
                assert!(apart >= 8.0 - 0.01 || squeezed(l) || squeezed(t), "{t:?} {l:?}");
            }
        }
    }
    assert!(saw_two_lines && saw_one_line);
    // It tells accessibility an indeterminate progress.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        IndeterminateLinearProgressIndicator(Modifier.test_tag("belirsiz"));
    }));
    assert_eq!(range(&rule, "belirsiz"), Some(ProgressBarRangeInfo::Indeterminate));
}

fn progress_indicator_defaults_as_compose() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        s.borrow_mut().extend([
            ProgressIndicatorDefaults::linear_color(),
            ProgressIndicatorDefaults::circular_color(),
            ProgressIndicatorDefaults::linear_track_color(),
            ProgressIndicatorDefaults::circular_determinate_track_color(),
            ProgressIndicatorDefaults::circular_indeterminate_track_color(),
            #[allow(deprecated)]
            ProgressIndicatorDefaults::circular_track_color(),
        ]);
    }));
    let cs = light_color_scheme();
    assert_eq!(
        *seen.borrow(),
        [
            cs.primary,
            cs.primary,
            cs.secondary_container,
            cs.secondary_container,
            Color::Transparent,
            Color::Transparent
        ]
    );
    assert_eq!(ProgressIndicatorDefaults::CircularStrokeWidth, 4.dp());
    assert_eq!(ProgressIndicatorDefaults::LinearStrokeCap, StrokeCap::Round);
    assert_eq!(ProgressIndicatorDefaults::CircularDeterminateStrokeCap, StrokeCap::Round);
    assert_eq!(ProgressIndicatorDefaults::CircularIndeterminateStrokeCap, StrokeCap::Round);
    assert_eq!(ProgressIndicatorDefaults::LinearTrackStopIndicatorSize, 4.dp());
    assert_eq!(ProgressIndicatorDefaults::LinearIndicatorTrackGapSize, 4.dp());
    assert_eq!(ProgressIndicatorDefaults::CircularIndicatorTrackGapSize, 4.dp());
    let spec = ProgressIndicatorDefaults::ProgressAnimationSpec();
    assert!(spec.damping_ratio == 1.0 && spec.stiffness == 50.0 && spec.visibility_threshold == Some(0.001));
    // In a dark scheme its colors follow.
    let dark = Rc::new(Cell::new(Color::Unspecified));
    let d = dark.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let d = d.clone();
        MaterialTheme(
            color_scheme = dark_color_scheme(),
            content = move || d.set(ProgressIndicatorDefaults::linear_track_color()),
        );
    }));
    assert_eq!(dark.get(), dark_color_scheme().secondary_container);
}

#[allow(deprecated)]
fn the_deprecated_value_forms_draw_as_the_lambda_ones() {
    // Compose's deprecated `LinearProgressIndicator(progress: Float, …)` is the lambda form with the default gap and
    // stop; the circular one keeps the deprecated transparent track.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                FloatLinearProgressIndicator(0.25, modifier = Modifier.width(200.dp()).test_tag("eski"));
                LinearProgressIndicator(progress = || 0.25, modifier = Modifier.width(200.dp()));
                FloatCircularProgressIndicator(0.25, modifier = Modifier.test_tag("eski halka"));
            },
        );
    }));
    let canvas = rule.draw();
    assert!(same_rect(canvas.paths[0], canvas.paths[2].translate(Offset::new(0.0, -4.0)), 0.01));
    assert!(same_rect(canvas.paths[1], canvas.paths[3].translate(Offset::new(0.0, -4.0)), 0.01));
    assert_eq!(canvas.rects.len(), 2);
    let track = &canvas.arcs[0];
    assert!(track.color == Color::Transparent || canvas.arcs.len() == 1);
    assert_eq!(range(&rule, "eski"), Some(ProgressBarRangeInfo::new(0.25, 0.0..=1.0, 0)));
    assert_eq!(range(&rule, "eski halka"), Some(ProgressBarRangeInfo::new(0.25, 0.0..=1.0, 0)));
}

fn wavy_progress_indicator_defaults_as_compose() {
    let strokes = Rc::new(RefCell::new(Vec::new()));
    let colors = Rc::new(RefCell::new(Vec::new()));
    let (s, c) = (strokes.clone(), colors.clone());
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, c) = (s.clone(), c.clone());
        KompozitionLocalProvider(&[LocalDensity.provides(Density::new(2.0, 1.0))], move || {
            s.borrow_mut().extend([
                WavyProgressIndicatorDefaults::linear_indicator_stroke(),
                WavyProgressIndicatorDefaults::circular_indicator_stroke(),
                WavyProgressIndicatorDefaults::linear_track_stroke(),
                WavyProgressIndicatorDefaults::circular_track_stroke(),
            ]);
            c.borrow_mut().extend([
                WavyProgressIndicatorDefaults::indicator_color(),
                WavyProgressIndicatorDefaults::track_color(),
            ]);
        });
    }));
    // 4 dp round strokes, in px at the density.
    assert!(strokes.borrow().iter().all(|s| s.width == 8.0 && s.cap == StrokeCap::Round));
    let cs = light_color_scheme();
    assert_eq!(*colors.borrow(), [cs.primary, cs.secondary_container]);
    assert_eq!(WavyProgressIndicatorDefaults::LinearDeterminateWavelength, 40.dp());
    assert_eq!(WavyProgressIndicatorDefaults::LinearIndeterminateWavelength, 20.dp());
    assert_eq!(WavyProgressIndicatorDefaults::LinearContainerHeight, 10.dp());
    assert_eq!(WavyProgressIndicatorDefaults::LinearContainerWidth, 240.dp());
    assert_eq!(WavyProgressIndicatorDefaults::LinearTrackStopIndicatorSize, 4.dp());
    assert_eq!(WavyProgressIndicatorDefaults::CircularContainerSize, 48.dp());
    assert_eq!(WavyProgressIndicatorDefaults::CircularWavelength, 15.dp());
    assert_eq!(WavyProgressIndicatorDefaults::LinearIndicatorTrackGapSize, 4.dp());
    assert_eq!(WavyProgressIndicatorDefaults::CircularIndicatorTrackGapSize, 4.dp());
    // The amplitude: a wave from past 10% to short of 95%; one function (an element given it twice is equal).
    let amplitude = WavyProgressIndicatorDefaults::indicator_amplitude();
    let values: Vec<f32> = [0.0, 0.1, 0.11, 0.5, 0.94, 0.95, 1.0].iter().map(|&p| amplitude(p)).collect();
    assert_eq!(values, [0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
    assert!(Rc::ptr_eq(&amplitude, &WavyProgressIndicatorDefaults::indicator_amplitude()));
    // The progress spec: 500 ms linear.
    let spec = WavyProgressIndicatorDefaults::ProgressAnimationSpec();
    use kompoz_animation::{Animation, TargetBasedAnimation, VectorConverter};
    let animation = TargetBasedAnimation::new(&*spec, f32::VectorConverter, 0.0, 1.0, None);
    assert_eq!(animation.duration_nanos(), 500_000_000);
    assert!(near(animation.get_value_from_nanos(250_000_000), 0.5, 0.001));
}

fn wavy_indicators_skip_transparent_parts_and_take_their_strokes() {
    // Compose's: the circular one doesn't draw a transparent (or unspecified) track or progress; the strokes given
    // are drawn with; the indeterminate ones' amplitude is kept in 0..1.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                CircularWavyProgressIndicator(progress = || 0.5, track_color = Color::Transparent);
                CircularWavyProgressIndicator(
                    progress = || 0.5,
                    color = Color::Unspecified,
                    track_stroke = Stroke::new(6.0, Stroke::DefaultMiter, StrokeCap::Butt, StrokeJoin::Miter, None),
                );
                LinearWavyProgressIndicator(
                    progress = || 0.5,
                    modifier = Modifier.width(200.dp()),
                    stroke = Stroke::new(2.0, Stroke::DefaultMiter, StrokeCap::Butt, StrokeJoin::Miter, None),
                    track_stroke = Stroke::new(2.0, Stroke::DefaultMiter, StrokeCap::Butt, StrokeJoin::Miter, None),
                    stop_size = 0.dp(),
                );
            },
        );
    }));
    let cs = light_color_scheme();
    let canvas = rule.draw();
    let circle1: Vec<usize> = (0..canvas.paths.len()).filter(|&i| canvas.paths[i].bottom <= 48.5).collect();
    assert!(circle1.len() == 1 && canvas.path_colors[circle1[0]] == cs.primary);
    let circle2: Vec<usize> =
        (0..canvas.paths.len()).filter(|&i| canvas.paths[i].top > 48.0 && canvas.paths[i].bottom <= 96.5).collect();
    assert!(circle2.len() == 1 && canvas.path_colors[circle2[0]] == cs.secondary_container);
    assert!(canvas.path_strokes[circle2[0]] == 6.0 && canvas.path_stroke_styles[circle2[0]].cap == StrokeCap::Butt);
    // Butt caps: no room kept for them — the track from the progress's end plus the gap to the very end; no stop.
    let linear: Vec<usize> =
        (0..canvas.paths.len()).filter(|&i| canvas.paths[i].bottom > 96.0 && canvas.paths[i].top > 92.0).collect();
    assert_eq!(linear.len(), 2);
    let track = drawn_contours(&canvas, linear[0]);
    assert!(same_rect(track[0], rect(104.0, 101.0, 96.0, 0.0), 0.01), "{track:?}");
    // The wave from the start to the middle, 4 dp up and down (the height less the 2 dp stroke, halved).
    let wave = drawn(&canvas, linear[1]);
    assert!(same_rect(wave, rect(0.0, 97.0, 100.0, 8.0), 0.05), "{wave:?}");
    assert!(canvas.rects.is_empty());
}

fn progress_indicators_semantics_bounds_and_ranges() {
    // Compose's: the linear ones' semantics 10 dp taller at the top and the bottom (a screen reader's minimum), their
    // layouts as they were; the circular ones' their own size; the indeterminate circular wavy one tells it from a
    // spacer of its own over the drawing (b/347736702).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                LinearProgressIndicator(progress = || 0.5, modifier = Modifier.test_tag("çizgi"));
                LinearWavyProgressIndicator(progress = || 0.5, modifier = Modifier.test_tag("dalga"));
                IndeterminateLinearWavyProgressIndicator(Modifier.test_tag("belirsiz dalga"));
                CircularWavyProgressIndicator(progress = || 0.5, modifier = Modifier.test_tag("halka"));
                IndeterminateCircularWavyProgressIndicator(Modifier.test_tag("belirsiz halka"));
            },
        );
    }));
    let bounds = |tag: &str| rule.on_node_with_tag(tag).get_unclipped_bounds_in_root();
    assert!(same_rect(bounds("çizgi"), rect(0.0, -10.0, 240.0, 24.0), 0.01), "{:?}", bounds("çizgi"));
    assert!(same_rect(bounds("dalga"), rect(0.0, -6.0, 240.0, 30.0), 0.01), "{:?}", bounds("dalga"));
    assert!(same_rect(bounds("halka"), rect(0.0, 24.0, 48.0, 48.0), 0.01), "{:?}", bounds("halka"));
    assert_eq!(range(&rule, "dalga"), Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0)));
    assert_eq!(range(&rule, "belirsiz dalga"), Some(ProgressBarRangeInfo::Indeterminate));
    assert_eq!(range(&rule, "halka"), Some(ProgressBarRangeInfo::new(0.5, 0.0..=1.0, 0)));
    // (the Box tagged, its second spacer telling)
    let spacers = rule.on_node_with_tag("belirsiz halka").on_children().fetch_semantics_nodes();
    let ranges: Vec<_> =
        spacers.iter().filter_map(|n| n.config().get(SemanticsProperties::ProgressBarRangeInfo)).collect();
    assert_eq!(ranges, [ProgressBarRangeInfo::Indeterminate]);
}

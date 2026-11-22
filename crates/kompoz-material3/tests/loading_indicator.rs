//! Material Expressive's loading indicators as Compose's material3 1.5 has them: LoadingIndicator and
//! ContainedLoadingIndicator, morphing by a progress or on their own, and LoadingIndicatorDefaults — the shapes they
//! morph through and how far they turn, their sizes and containers, their timing (and that they compose nothing as
//! they move), what they tell accessibility. The C++ Kompoz's tests by name (kompoz_tests.cpp; the loading indicator
//! halves of `containers_take_compose_named_arguments` and `graphics_forms_take_compose_named_arguments`, whose rest
//! is other packages'), and Compose's behaviour.
//!
//! The recording canvas keeps a path as drawn before the rotation it is drawn in (its bounds where it lands
//! unturned): the tests turn its points by the recorded transform (`drawn_points`). The C++ drew its own outlines
//! (distances from the center blended angle by angle, 120 points, its own timing); M15 morphs Material's shapes
//! (RoundedPolygon / Morph) scaled so that they keep inside 38 dp as they turn: where they differ the tests follow
//! M15 and say so. Compose's test rule (KompozR's too) cancels endless animations while its clock advances on its
//! own: the tests that watch the endless ones turn that off first.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_ui::semantics::{ProgressBarRangeInfo, SemanticsProperties};
use kompoz_ui::*;
use std::cell::Cell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

tests!(
    loading_indicator_morphs_and_turns_and_grows_into_a_burst,
    containers_take_compose_named_arguments_loading_indicator,
    graphics_forms_take_compose_named_arguments_loading_indicator,
    determinate_loading_indicators_morph_through_their_polygons_by_progress,
    loading_indicators_need_two_polygons,
    indeterminate_loading_indicators_morph_every_650_ms_and_turn,
    indeterminate_loading_indicators_start_again_with_new_polygons,
    endless_loading_indicators_keep_still_while_the_test_clock_runs_on_its_own,
    contained_loading_indicators_draw_in_their_container,
    loading_indicators_take_the_size_they_are_given,
    loading_indicator_defaults_as_compose,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// The angle (degrees) a recorded rotation turns by.
fn rotation_of(m: &Matrix) -> f32 {
    m.get(0, 1).atan2(m.get(0, 0)).to_degrees()
}

/// `degrees` brought into -180..180.
fn wrapped(degrees: f32) -> f32 {
    degrees - 360.0 * (degrees / 360.0).round()
}

/// Points along what the recorded path `i` draws (each contour sampled along its length), turned by the transform
/// it was drawn in (each indicator draws its path in a rotation of its own: the `i`th transform), in the canvas's
/// coordinates.
fn drawn_points(c: &RecordingCanvas, i: usize) -> Vec<Offset> {
    assert_eq!(c.transforms.len(), c.paths.len()); // (one rotation a path)
    let shape = &c.path_shapes[i];
    let b = shape.get_bounds();
    let shift = Offset::new(c.paths[i].left - b.left, c.paths[i].top - b.top);
    let m = c.transforms[i];
    let mut measure = PathMeasure::new();
    measure.set_path(shape, false);
    let mut points = Vec::new();
    loop {
        let length = measure.length();
        points.extend((0..240).map(|k| m.map(measure.get_position(length * k as f32 / 240.0)) + shift));
        if !measure.next_contour() {
            break;
        }
    }
    points
}

/// The nearest and the farthest of `points` from `center`.
fn reach(points: &[Offset], center: Offset) -> (f32, f32) {
    points.iter().fold((f32::MAX, 0.0f32), |(nearest, farthest), p| {
        let d = (*p - center).get_distance();
        (nearest.min(d), farthest.max(d))
    })
}

/// How far an outline moved from one frame to the next: the farthest any point of `now` is from `before`'s outline.
fn moved_by(before: &[Offset], now: &[Offset]) -> f32 {
    now.iter().map(|p| before.iter().map(|q| (*p - *q).get_distance()).fold(f32::MAX, f32::min)).fold(0.0, f32::max)
}

/// The middle of an outline: the mean of its points (spaced evenly along it, as `drawn_points` and `outline_of`
/// take them), wherever it is centered or turned.
fn middle(points: &[Offset]) -> Offset {
    let sum = points.iter().fold(Offset::Zero, |s, p| s + *p);
    Offset::new(sum.x / points.len() as f32, sum.y / points.len() as f32)
}

/// How round an outline is, its nearest point over its farthest from its middle (what tells the shapes apart
/// whatever their size and turn).
fn roundness(points: &[Offset]) -> f32 {
    let (nearest, farthest) = reach(points, middle(points));
    nearest / farthest
}

/// How many lobes (corners, bumps, ends) an outline has: the times its distance from its middle turns from rising
/// to falling (by more than a 50th of its reach, so that a corner's rounding counts once).
fn lobes(points: &[Offset]) -> usize {
    let center = middle(points);
    let distances: Vec<f32> = points.iter().map(|p| (*p - center).get_distance()).collect();
    let step = reach(points, center).1 / 50.0;
    // (from the nearest point: the walk starts rising)
    let start = (0..distances.len()).min_by(|a, b| distances[*a].total_cmp(&distances[*b])).unwrap();
    let (mut lobes, mut rising, mut extreme) = (0, true, distances[start]);
    for k in 1..=distances.len() {
        let d = distances[(start + k) % distances.len()];
        if (rising && d > extreme) || (!rising && d < extreme) {
            extreme = d;
        } else if rising && d < extreme - step {
            lobes += 1;
            (rising, extreme) = (false, d);
        } else if !rising && d > extreme + step {
            (rising, extreme) = (true, d);
        }
    }
    lobes
}

/// A polygon's outline, points spaced evenly along it (the polygon made 100 across first: a path is measured in
/// pieces of about a pixel).
fn outline_of(polygon: &RoundedPolygon) -> Vec<Offset> {
    let mut path = polygon.to_path(0);
    let mut scale = Matrix::new();
    scale.scale(100.0, 100.0, 1.0);
    path.transform(&scale);
    let mut measure = PathMeasure::new();
    measure.set_path(&path, false);
    let length = measure.length();
    (0..240).map(|k| measure.get_position(length * k as f32 / 240.0)).collect()
}

fn roundness_of(polygon: &RoundedPolygon) -> f32 {
    roundness(&outline_of(polygon))
}

/// The roundness of what the recorded path `i` draws.
fn drawn_roundness(c: &RecordingCanvas, i: usize) -> f32 {
    roundness(&drawn_points(c, i))
}

/// The lobes of what the recorded path `i` draws.
fn drawn_lobes(c: &RecordingCanvas, i: usize) -> usize {
    lobes(&drawn_points(c, i))
}

/// The default endless shapes' lobes: SoftBurst 10, Cookie9Sided 9, Pentagon 5, Pill 2, Sunny 8, Cookie4Sided 4,
/// Oval 2.
const DefaultLobes: [usize; 7] = [10, 9, 5, 2, 8, 4, 2];

fn range(rule: &KompozTestRule, tag: &str) -> Option<ProgressBarRangeInfo> {
    rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::ProgressBarRangeInfo)
}

fn determinate(current: f32) -> ProgressBarRangeInfo {
    ProgressBarRangeInfo::new(current, 0.0..=1.0, 0)
}

/// A rule whose clock moves only when the test moves it: endless animations run (Compose's
/// `mainClock.autoAdvance = false`).
fn rule_with_animations(content: impl Content + 'static) -> KompozTestRule {
    let rule = KompozTestRule::empty();
    rule.main_clock().set_auto_advance(false);
    rule.set_content(content);
    rule
}

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

// ---- The C++ Kompoz's tests --------------------------------------------------------------------------------------

fn loading_indicator_morphs_and_turns_and_grows_into_a_burst() {
    // (the C++ gave the progress as a value; M15 takes a lambda, read as it draws) M15's shapes are Material's: the
    // circle 37.4 dp across (scaled so that the burst keeps inside 38 dp as it turns; the C++'s 38), the burst's
    // lobes 18.69 dp out and 14.72 dp in (the C++'s 19 and 14..17). The C++'s "0%"/"100%"/"loading" texts are what
    // Android's accessibility says of M15's progress ranges, checked here. Compose's test rule cancels endless
    // animations while its clock advances on its own: the clock moves only as the test says.
    let shown = mutable_state_of(false);
    let progress = mutable_state_of(0.0f32);
    let cs = light_color_scheme();
    let (s, p) = (shown.clone(), progress.clone());
    let rule = rule_with_animations(kompozable_lambda!(move || {
        let (s, p) = (s.clone(), p.clone());
        Column(
            content = move || {
                let (a, b) = (p.clone(), p.clone());
                // 48 dp: a 38 dp shape in the middle
                LoadingIndicator(progress = move || a.get(), modifier = Modifier.test_tag("belirli"));
                // in a circle
                ContainedLoadingIndicator(progress = move || b.get(), modifier = Modifier.test_tag("kaplı"));
                if s.get() {
                    IndeterminateLoadingIndicator(Modifier.test_tag("belirsiz"));
                    IndeterminateContainedLoadingIndicator(Modifier.test_tag("kaplı-belirsiz"));
                }
            },
        );
    }));
    let center = Offset::new(24.0, 24.0);
    let round = rule.draw();
    assert!(round.paths.len() == 2 && round.path_colors[0] == cs.primary);
    let (nearest, farthest) = reach(&drawn_points(&round, 0), center);
    assert!(near(nearest, 18.695, 0.01) && near(farthest, 18.695, 0.01), "{nearest} {farthest}"); // a circle at 0
    assert!(round.rects.len() == 1 && round.rects[0] == Rect::new(0.0, 48.0, 48.0, 96.0));
    assert!(round.rect_colors[0] == cs.primary_container && round.path_colors[1] == cs.on_primary_container);
    let clip = &round.clip_shapes[1]; // (the container cut to a circle)
    assert!(clip.bounds == Rect::new(0.0, 48.0, 48.0, 96.0) && near(clip.radii.top_left, 24.0, 0.01));
    assert!(range(&rule, "belirli") == Some(determinate(0.0)) && range(&rule, "kaplı") == Some(determinate(0.0)));
    assert!(!rule.host().has_pending_work()); // a determinate one only moves with its progress

    progress.set(1.0);
    rule.advance_frame();
    let burst = rule.draw();
    assert!(burst.paths.len() == 2 && burst.path_shapes[0] != round.path_shapes[0]);
    let (nearest, farthest) = reach(&drawn_points(&burst, 0), center); // its lobes: the outline comes and goes
    assert!(near(farthest, 18.69, 0.01) && nearest > 14.0 && nearest < 15.0, "{nearest} {farthest}");
    assert!(near(rotation_of(&burst.transforms[0]).abs(), 180.0, 0.01)); // turned half way round on the way
    assert!(range(&rule, "belirli") == Some(determinate(1.0)) && range(&rule, "kaplı") == Some(determinate(1.0)));

    shown.set(true);
    rule.advance_frame();
    let box_center = Offset::new(24.0, 96.0 + 24.0);
    let mut last: Option<(Vec<Offset>, f32)> = None;
    let (mut jump, mut turned, mut least_turn) = (0.0f32, 0.0f32, f32::MAX);
    let mut outlines = BTreeSet::new();
    let frames = 4 * 7 * 650 / 16 + 20; // a whole round of morphs (650 ms each, M15's MorphIntervalMillis), and its wrap
    for _ in 0..frames {
        rule.advance_frame();
        let c = rule.draw();
        assert_eq!(c.paths.len(), 4);
        let now = drawn_points(&c, 2);
        let (_, farthest) = reach(&now, box_center);
        // inside the 38 dp (the spring's overshoot morphs a little past the next shape)
        assert!(farthest < 19.2 && c.path_colors[2] == cs.primary, "{farthest}");
        assert!(c.path_colors[3] == cs.on_primary_container && c.rects.len() == 2);
        assert_eq!(c.rect_colors[1], cs.primary_container);
        let angle = rotation_of(&c.transforms[2]);
        if let Some((before, last_angle)) = &last {
            jump = jump.max(moved_by(before, &now));
            let step = wrapped(angle - last_angle);
            least_turn = least_turn.min(step.abs());
            turned += step;
        }
        let b = c.paths[2];
        outlines.insert(((b.width() * 4.0) as i32, (b.height() * 4.0) as i32));
        last = Some((now, angle));
    }
    // always on the move (a turn every frame), never a leap (not even where a morph ends and the next starts)
    assert!(jump > 0.2 && jump < 4.0 && least_turn > 0.5, "{jump} {least_turn}");
    assert!(outlines.len() > 10); // and its outline changes
    let turns = turned / 360.0;
    // a quarter turn a morph (28: 7 turns) on top of a turn every 4.666 s
    assert!(turns > 10.0 && turns < 12.0, "{turns}");
    assert_eq!(range(&rule, "belirsiz"), Some(ProgressBarRangeInfo::Indeterminate));
}

fn containers_take_compose_named_arguments_loading_indicator() {
    // The loading indicators' part (its lazy list, badges, segmented list items, extended FAB and bottom app bar are
    // other packages'): by name. (The C++'s `LoadingIndicator(progress = 0.5f, …)`: M15 takes a lambda.)
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                IndeterminateLoadingIndicator(modifier = Modifier.size(48.dp()).test_tag("yükleniyor"));
                LoadingIndicator(progress = || 0.5, modifier = Modifier.test_tag("yarım"));
            },
        );
    }));
    rule.on_node_with_tag("yükleniyor").assert_exists();
    rule.on_node_with_tag("yarım").assert_exists().assert_range_info_equals(determinate(0.5));
}

fn graphics_forms_take_compose_named_arguments_loading_indicator() {
    // The contained loading indicators' part (the wavy ones are tests/progress.rs', the rest the lower crates'): by
    // name and by place, 48 dp wide. (The C++'s `ContainedLoadingIndicator(progress = 0.3f, …)`: M15 takes a
    // lambda.)
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                ContainedLoadingIndicator(
                    progress = || 0.3,
                    container_color = Color::Yellow,
                    modifier = Modifier.test_tag("kap"),
                );
                IndeterminateContainedLoadingIndicator(Modifier.test_tag("dönen"));
            },
        );
    }));
    let width = |tag: &str| rule.on_node_with_tag(tag).get_unclipped_bounds_in_root().width();
    assert!(near(width("kap"), 48.0, 0.01) && near(width("dönen"), 48.0, 0.01));
    assert!(rule.draw().rect_colors.contains(&Color::Yellow));
}

// ---- Compose's behaviour ------------------------------------------------------------------------------------------

fn determinate_loading_indicators_morph_through_their_polygons_by_progress() {
    // Compose's: n polygons make n - 1 morphs, the progress split among them (the last one whole at 1); the shape
    // turns counterclockwise half way round as the progress goes from 0 to 1; the progress kept in 0..1, NaN told as
    // 0 (and drawn as nothing: a path of points not finite, as Skia's).
    let progress = mutable_state_of(0.0f32);
    let p = progress.clone();
    let polygons =
        vec![MaterialShapes::Square().clone(), MaterialShapes::Circle().clone(), MaterialShapes::Square().clone()];
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let p = p.clone();
        LoadingIndicator(progress = move || p.get(), polygons = polygons.clone(), modifier = Modifier.test_tag("üç"));
    }));
    let square = roundness_of(&MaterialShapes::Square().normalized());
    let at = |value: f32| {
        progress.set(value);
        rule.wait_for_idle();
        let c = rule.draw();
        (drawn_roundness(&c, 0), rotation_of(&c.transforms[0]))
    };
    let (roundness, turn) = at(0.0);
    assert!(near(roundness, square, 0.01) && near(turn, 0.0, 0.01), "{roundness} {square}"); // the square
    let (roundness, turn) = at(0.25);
    assert!(roundness > square + 0.02 && roundness < 0.99 && near(turn, -45.0, 0.01)); // half way to the circle
    let (roundness, turn) = at(0.5);
    assert!(roundness > 0.999 && near(turn, -90.0, 0.01), "{roundness}"); // the circle: the second morph at 0
    let (roundness, turn) = at(0.75);
    assert!(roundness > square + 0.02 && roundness < 0.99 && near(turn, -135.0, 0.01));
    let (roundness, turn) = at(1.0);
    assert!(near(roundness, square, 0.01) && near(turn.abs(), 180.0, 0.01)); // the last morph whole
    let (roundness, turn) = at(1.5); // (kept at 1)
    assert!(near(roundness, square, 0.01) && near(turn.abs(), 180.0, 0.01));
    assert_eq!(range(&rule, "üç"), Some(determinate(1.0)));
    let (_, turn) = at(-0.5); // (kept at 0)
    assert!(near(turn, 0.0, 0.01) && range(&rule, "üç") == Some(determinate(0.0)));
    progress.set(f32::NAN);
    rule.wait_for_idle();
    assert!(rule.draw().paths.is_empty() && range(&rule, "üç") == Some(determinate(0.0)));
}

fn loading_indicators_need_two_polygons() {
    // Kotlin's `require(indicatorPolygons.size > 1)`, determinate or endless.
    for endless in [false, true] {
        let message = panic_message(move || {
            let polygons = vec![MaterialShapes::Heart().clone()];
            let _rule = KompozTestRule::new(kompozable_lambda!(move || {
                if endless {
                    IndeterminateLoadingIndicator(polygons = polygons.clone());
                } else {
                    LoadingIndicator(progress = || 0.5, polygons = polygons.clone());
                }
            }));
        });
        let message = message.expect("a single polygon is refused");
        assert!(
            message.contains("IllegalArgumentException")
                && message.contains("indicatorPolygons should have, at least, two RoundedPolygons"),
            "{message}"
        );
    }
}

fn indeterminate_loading_indicators_morph_every_650_ms_and_turn() {
    // Compose's: it starts as the first shape turned a quarter round; every 650 ms a spring (damping 0.6, stiffness
    // 200, done within 0.1) morphs it into the next, turning it a quarter more, back to the first after the last; on
    // top of that it turns round every 4.666 s. All of it is read as it draws: nothing is composed (nor laid out) as
    // it moves.
    let composed = Rc::new(Cell::new(0));
    let c = composed.clone();
    let rule = rule_with_animations(kompozable_lambda!(move || {
        c.set(c.get() + 1);
        IndeterminateLoadingIndicator(Modifier.test_tag("dönen"));
    }));
    let shapes: Vec<f32> =
        LoadingIndicatorDefaults::IndeterminateIndicatorPolygons().iter().map(roundness_of).collect();
    let first = rule.draw();
    // the soft burst
    let roundness = drawn_roundness(&first, 0);
    assert!(drawn_lobes(&first, 0) == 10 && near(roundness, shapes[0], 0.01), "{roundness} {shapes:?}");
    assert!(near(rotation_of(&first.transforms[0]), 90.0, 0.01)); // a quarter round
    assert_eq!(range(&rule, "dönen"), Some(ProgressBarRangeInfo::Indeterminate));
    let start = rule.main_clock().current_time();
    let relaid_out = rule.host().relaid_out_count();
    // Each morph done (its spring at rest in under 400 ms), the next shape at rest: the k-th after 650 k ms.
    for k in 1..=9usize {
        let rest = start + 650 * k as i64 - 200;
        while rule.main_clock().current_time() < rest {
            rule.advance_frame();
        }
        let c = rule.draw();
        let (lobes, roundness) = (drawn_lobes(&c, 0), drawn_roundness(&c, 0));
        assert!(lobes == DefaultLobes[k % 7] && near(roundness, shapes[k % 7], 0.01), "{k}: {lobes} {roundness}");
        // A quarter turn a morph on top of a turn in 4.666 s (from the first frame it runs, 16 ms in).
        let global = 360.0 * (rule.main_clock().current_time() - start - 16) as f32 / 4666.0;
        let expected = 90.0 * (k + 1) as f32 + global;
        let turn = rotation_of(&c.transforms[0]);
        assert!(wrapped(turn - expected).abs() < 1.5, "{k}: {turn} {expected}");
    }
    // Between morphs only the global turn moves it: 360° in 4666 ms, a frame's 1.23°.
    let before = rotation_of(&rule.draw().transforms[0]);
    rule.advance_frame();
    let after = rotation_of(&rule.draw().transforms[0]);
    assert!(near(wrapped(after - before), 360.0 * 16.0 / 4666.0, 0.01), "{before} {after}");
    assert_eq!(composed.get(), 1);
    assert_eq!(rule.host().relaid_out_count(), relaid_out);
}

fn indeterminate_loading_indicators_start_again_with_new_polygons() {
    // Compose's: the morphs and their effect are keyed by the polygons: new ones start again from the first shape
    // (the turn goes on from where it was). Two shapes morph into each other and back.
    let polygons = mutable_state_of(LoadingIndicatorDefaults::IndeterminateIndicatorPolygons());
    let p = polygons.clone();
    let rule = rule_with_animations(kompozable_lambda!(move || {
        IndeterminateLoadingIndicator(polygons = p.get());
    }));
    let start = rule.main_clock().current_time();
    while rule.main_clock().current_time() < start + 650 * 2 - 200 {
        rule.advance_frame();
    }
    assert_eq!(drawn_lobes(&rule.draw(), 0), 5); // the third shape, the pentagon
    polygons.set(vec![MaterialShapes::Square().clone(), MaterialShapes::Circle().clone()]);
    rule.advance_frame();
    let square = roundness_of(MaterialShapes::Square());
    let c = rule.draw();
    assert!(drawn_lobes(&c, 0) == 4 && near(drawn_roundness(&c, 0), square, 0.02)); // the first again
    let restart = rule.main_clock().current_time();
    for k in 1..=3usize {
        while rule.main_clock().current_time() < restart + 650 * k as i64 - 200 {
            rule.advance_frame();
        }
        let roundness = drawn_roundness(&rule.draw(), 0);
        if k % 2 == 1 {
            assert!(roundness > 0.999, "{k}: {roundness}"); // the circle
        } else {
            assert!(near(roundness, square, 0.02), "{k}: {roundness}"); // and back
        }
    }
}

fn endless_loading_indicators_keep_still_while_the_test_clock_runs_on_its_own() {
    // Compose's: the morphs and the turn run through the coroutine's InfiniteAnimationPolicy; the test rule's
    // cancels them while its clock advances on its own: the first shape stays, a quarter round.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        IndeterminateContainedLoadingIndicator(Modifier.test_tag("dönen"));
    }));
    let first = rule.draw();
    rule.main_clock().advance_time_by(2000);
    rule.wait_for_idle();
    let later = rule.draw();
    assert!(later.path_shapes[0] == first.path_shapes[0] && near(rotation_of(&later.transforms[0]), 90.0, 0.01));
    assert!(!rule.host().has_pending_work());
}

fn contained_loading_indicators_draw_in_their_container() {
    // Compose's: the container is cut to its shape (a circle by default) and filled; an uncontained one's container
    // is unspecified: nothing drawn behind it. Colors and shapes as given.
    let cs = light_color_scheme();
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                LoadingIndicator(progress = || 0.5);
                IndeterminateLoadingIndicator();
                ContainedLoadingIndicator(
                    progress = || 0.5,
                    container_color = Color::Yellow,
                    indicator_color = Color::Red,
                    container_shape = Arc::new(RoundedCornerShape(8.dp())),
                );
                IndeterminateContainedLoadingIndicator(
                    container_color = Color::Blue,
                    indicator_color = Color::Green,
                    container_shape = Arc::new(RectangleShape),
                );
                LoadingIndicator(progress = || 0.5, color = Color::Magenta);
            },
        );
    }));
    let c = rule.draw();
    assert_eq!(c.path_colors, [cs.primary, cs.primary, Color::Red, Color::Green, Color::Magenta]);
    assert_eq!(c.rects, [Rect::new(0.0, 96.0, 48.0, 144.0), Rect::new(0.0, 144.0, 48.0, 192.0)]);
    assert_eq!(c.rect_colors, [Color::Yellow, Color::Blue]);
    let rounded: Vec<f32> = c.clip_shapes.iter().map(|s| s.radii.top_left).collect();
    assert!(rounded.iter().filter(|r| near(**r, 24.0, 0.01)).count() >= 3, "{rounded:?}"); // the circles
    assert!(rounded.iter().any(|r| near(*r, 8.0, 0.01)), "{rounded:?}");
}

fn loading_indicators_take_the_size_they_are_given() {
    // Compose's: 48 dp unless the modifier sizes it; the shape keeps square (the height first) in the middle, scaled
    // with the size (38 of each 48 dp, the turn's room kept).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                LoadingIndicator(progress = || 0.0, modifier = Modifier.test_tag("olağan"));
                LoadingIndicator(progress = || 0.0, modifier = Modifier.size(96.dp()).test_tag("büyük"));
                ContainedLoadingIndicator(progress = || 0.0, modifier = Modifier.width(100.dp()).test_tag("geniş"));
            },
        );
    }));
    let bounds = |tag: &str| rule.on_node_with_tag(tag).get_unclipped_bounds_in_root();
    assert_eq!(bounds("olağan"), Rect::new(0.0, 0.0, 48.0, 48.0));
    assert_eq!(bounds("büyük"), Rect::new(0.0, 48.0, 96.0, 144.0));
    assert_eq!(bounds("geniş"), Rect::new(0.0, 144.0, 100.0, 192.0));
    let c = rule.draw();
    let circle = |i: usize, center: Offset| reach(&drawn_points(&c, i), center);
    let (nearest, farthest) = circle(1, Offset::new(48.0, 96.0));
    assert!(near(nearest, 18.695 * 2.0, 0.02) && near(farthest, 18.695 * 2.0, 0.02), "{nearest} {farthest}");
    let (nearest, farthest) = circle(2, Offset::new(50.0, 168.0)); // a 48 dp square in the middle
    assert!(near(nearest, 18.695, 0.01) && near(farthest, 18.695, 0.01), "{nearest} {farthest}");
    assert_eq!(c.rects, [Rect::new(0.0, 144.0, 100.0, 192.0)]); // the container fills the width
    let clip = c.clip_shapes.last().unwrap();
    assert!(clip.bounds == Rect::new(0.0, 144.0, 100.0, 192.0) && near(clip.radii.top_left, 24.0, 0.01));
}

fn loading_indicator_defaults_as_compose() {
    assert_eq!(LoadingIndicatorDefaults::ContainerWidth, 48.dp());
    assert_eq!(LoadingIndicatorDefaults::ContainerHeight, 48.dp());
    assert_eq!(LoadingIndicatorDefaults::IndicatorSize, 38.dp());
    let indeterminate = LoadingIndicatorDefaults::IndeterminateIndicatorPolygons();
    let expected = [
        MaterialShapes::SoftBurst(),
        MaterialShapes::Cookie9Sided(),
        MaterialShapes::Pentagon(),
        MaterialShapes::Pill(),
        MaterialShapes::Sunny(),
        MaterialShapes::Cookie4Sided(),
        MaterialShapes::Oval(),
    ];
    assert!(indeterminate.len() == 7 && indeterminate.iter().zip(expected).all(|(a, b)| a == b));
    let determinate = LoadingIndicatorDefaults::DeterminateIndicatorPolygons();
    let mut turn = Matrix::new();
    turn.rotate_z(18.0); // (360 / 20: the circle turned to morph smoothly into the burst, turned so too)
    assert!(determinate.len() == 2 && determinate[1] == *MaterialShapes::SoftBurst());
    assert!(
        determinate[0] == MaterialShapes::Circle().transformed_by(&turn) && determinate[0] != *MaterialShapes::Circle()
    );
    let cs = light_color_scheme();
    let seen = Rc::new(Cell::new(None));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let shape = LoadingIndicatorDefaults::container_shape();
        let outline = shape.create_outline(Size::new(48.0, 48.0), LayoutDirection::Ltr, &Density::new(1.0, 1.0));
        let circle = CircleShape.create_outline(Size::new(48.0, 48.0), LayoutDirection::Ltr, &Density::new(1.0, 1.0));
        s.set(Some((
            LoadingIndicatorDefaults::indicator_color(),
            LoadingIndicatorDefaults::contained_indicator_color(),
            LoadingIndicatorDefaults::contained_container_color(),
            outline == circle,
        )));
    }));
    assert_eq!(seen.get(), Some((cs.primary, cs.on_primary_container, cs.primary_container, true)));
}

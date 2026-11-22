//! Material's progress indicators in Kotlin's syntax, as a Compose user writes them (kt/progress.kt through
//! `kompoz_file!`): the progress lines of the C++ Kompoz's MaterialForms and GraphicsForms (kompozable_tests.cpp
//! `material_forms_as_compose_writes_them`, `graphics_forms_as_compose_writes_them`: what the C++ asserted of them;
//! the rest of those sources is other packages' tests'), material3 1.5's ProgressIndicatorSamples (determinate,
//! endless, wavy, thick, legacy) and the deprecated value forms and the defaults by name; the loading indicators of
//! the C++'s Containers and GraphicsForms (`containers_as_compose_writes_them`, `graphics_forms_as_compose_writes_them`)
//! and material3 1.5's LoadingIndicatorSamples, and the loading indicators by name.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, RecordingCanvas};
use kompoz::ui::semantics::{ProgressBarRangeInfo, SemanticsActions, SemanticsNode, SemanticsProperties};

kompoz_file!("tests/kt/progress.kt");

/// The progress indicators' nodes (a progress, not settable: a slider's is), in order.
fn indicators(rule: &KompozTestRule) -> Vec<SemanticsNode> {
    rule.on_root()
        .fetch_semantics_node()
        .subtree()
        .into_iter()
        .filter(|n| {
            n.config().contains(SemanticsProperties::ProgressBarRangeInfo)
                && !n.config().contains(SemanticsActions::SetProgress)
        })
        .collect()
}

fn progress_of(rule: &KompozTestRule) -> Vec<ProgressBarRangeInfo> {
    indicators(rule).iter().filter_map(|n| n.config().get(SemanticsProperties::ProgressBarRangeInfo)).collect()
}

fn determinate(current: f32) -> ProgressBarRangeInfo {
    ProgressBarRangeInfo::new(current, 0.0..=1.0, 0)
}

/// Sets the sample's slider as accessibility does.
fn set_slider(rule: &KompozTestRule, value: f32) {
    let slider =
        rule.on_node_with(kompoz::test::SemanticsMatcher::key_is_defined(SemanticsActions::SetProgress), false);
    slider.perform_semantics_action_with(SemanticsActions::SetProgress, |f| {
        f(value);
    });
    rule.wait_for_idle();
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn colored(canvas: &RecordingCanvas, color: Color) -> Vec<usize> {
    (0..canvas.paths.len()).filter(|&i| canvas.path_colors[i] == color).collect()
}

fn material_forms_progress() {
    // MaterialForms' `LinearProgressIndicator(progress = { 0.4f }, …)`: there, telling 40%.
    let rule = KompozTestRule::new(MaterialFormsProgress);
    rule.on_node_with_tag("bar").assert_exists().assert_range_info_equals(determinate(0.4));
}

fn graphics_forms_progress() {
    // GraphicsForms' wavy indicators: the linear one 100 dp wide (C++'s check), its wave half the most (a lambda
    // amplitude), no stop; the endless circular one's track gray.
    let rule = KompozTestRule::new(GraphicsFormsProgress);
    let bounds = rule.on_node_with_tag("dalga").get_unclipped_bounds_in_root();
    assert!(near(bounds.width(), 100.0, 0.5), "{bounds:?}");
    rule.on_node_with_tag("dalga").assert_range_info_equals(determinate(0.5));
    let canvas = rule.draw();
    assert!(canvas.rects.is_empty()); // stopSize = 0.dp
    let wave = colored(&canvas, light_color_scheme().primary); // (no theme around: Material's default scheme)
    assert!(!wave.is_empty() && canvas.paths[wave[0]].right <= 100.0);
    assert!(!colored(&canvas, Color::Gray).is_empty());
    assert_eq!(progress_of(&rule), [determinate(0.5), ProgressBarRangeInfo::Indeterminate]);
}

fn determinate_samples() {
    // Linear, wavy linear, circular, wavy circular, their thick forms and the legacy ones: 10% to start, the
    // slider's value animated to (a spring) as it is set.
    let samples: [(&str, fn() -> KompozTestRule); 8] = [
        ("LinearProgressIndicatorSample", || KompozTestRule::new(LinearProgressIndicatorSample)),
        ("LinearWavyProgressIndicatorSample", || KompozTestRule::new(LinearWavyProgressIndicatorSample)),
        ("LinearThickWavyProgressIndicatorSample", || KompozTestRule::new(LinearThickWavyProgressIndicatorSample)),
        ("CircularProgressIndicatorSample", || KompozTestRule::new(CircularProgressIndicatorSample)),
        ("CircularWavyProgressIndicatorSample", || KompozTestRule::new(CircularWavyProgressIndicatorSample)),
        ("CircularThickWavyProgressIndicatorSample", || KompozTestRule::new(CircularThickWavyProgressIndicatorSample)),
        ("LegacyLinearProgressIndicatorSample", || KompozTestRule::new(LegacyLinearProgressIndicatorSample)),
        ("LegacyCircularProgressIndicatorSample", || KompozTestRule::new(LegacyCircularProgressIndicatorSample)),
    ];
    for (name, make) in samples {
        let rule = make();
        rule.on_node_with_text("Set progress:").assert_exists();
        assert_eq!(progress_of(&rule), [determinate(0.1)], "{name}");
        set_slider(&rule, 0.6);
        let now = progress_of(&rule)[0].current;
        assert!(near(now, 0.6, 0.002), "{name}: {now}");
    }
}

fn thick_wavy_samples() {
    // An 8 dp round stroke for the progress and the track; the linear one 14 dp tall, the circular one 52 dp.
    let thick: [fn() -> KompozTestRule; 2] = [
        || KompozTestRule::new(LinearThickWavyProgressIndicatorSample),
        || KompozTestRule::new(CircularThickWavyProgressIndicatorSample),
    ];
    for make in thick {
        let rule = make();
        set_slider(&rule, 0.5);
        let canvas = rule.draw();
        let cs = light_color_scheme();
        // (the indicator's paths: the sample's slider below is drawn in those colors too)
        let drawn: Vec<usize> = colored(&canvas, cs.primary)
            .into_iter()
            .chain(colored(&canvas, cs.secondary_container))
            .filter(|&i| canvas.paths[i].top < 56.0)
            .collect();
        assert!(drawn.len() >= 2);
        assert!(
            drawn
                .iter()
                .all(|&i| canvas.path_strokes[i] == 8.0 && canvas.path_stroke_styles[i].cap == StrokeCap::Round)
        );
    }
    let rule = KompozTestRule::new(LinearThickWavyProgressIndicatorSample);
    let node = &indicators(&rule)[0];
    // (its semantics 10 dp taller each way)
    assert!(near(node.bounds_in_root().height(), 34.0, 0.01), "{:?}", node.bounds_in_root());
    let rule = KompozTestRule::new(CircularThickWavyProgressIndicatorSample);
    assert!(near(indicators(&rule)[0].bounds_in_root().width(), 52.0, 0.01));
}

fn indeterminate_samples() {
    // The endless ones: an indeterminate progress each; the legacy ones' butt caps and surface variant track.
    let endless: [fn() -> KompozTestRule; 6] = [
        || KompozTestRule::new(IndeterminateLinearProgressIndicatorSample),
        || KompozTestRule::new(IndeterminateLinearWavyProgressIndicatorSample),
        || KompozTestRule::new(IndeterminateCircularProgressIndicatorSample),
        || KompozTestRule::new(IndeterminateCircularWavyProgressIndicatorSample),
        || KompozTestRule::new(LegacyIndeterminateLinearProgressIndicatorSample),
        || KompozTestRule::new(LegacyIndeterminateCircularProgressIndicatorSample),
    ];
    for make in endless {
        let rule = make();
        assert_eq!(progress_of(&rule), [ProgressBarRangeInfo::Indeterminate]);
    }
    let cs = light_color_scheme();
    let rule = KompozTestRule::new(LegacyIndeterminateLinearProgressIndicatorSample);
    let canvas = rule.draw();
    let track = colored(&canvas, cs.surface_variant);
    assert!(!track.is_empty() && canvas.path_stroke_styles[track[0]].cap == StrokeCap::Butt);
    let rule = KompozTestRule::new(LegacyIndeterminateCircularProgressIndicatorSample);
    let canvas = rule.draw();
    let arc = canvas.arcs.iter().find(|a| a.color == cs.primary).expect("the spinner's arc");
    assert!(arc.stroke_width == 4.0);
}

fn legacy_samples() {
    // LegacyLinearProgressIndicatorSample: butt caps, no gap, no stop, a surface variant track; the circular one's
    // transparent track, butt caps, no gap.
    let cs = light_color_scheme();
    let rule = KompozTestRule::new(LegacyLinearProgressIndicatorSample);
    let canvas = rule.draw();
    assert!(canvas.rects.iter().all(|r| r.top > 10.0)); // (the slider's below)
    let progress: Vec<usize> =
        colored(&canvas, cs.primary).into_iter().filter(|&i| canvas.paths[i].top < 10.0).collect();
    let track = colored(&canvas, cs.surface_variant);
    assert!(track.len() == 1 && progress.len() == 1);
    assert!(near(canvas.paths[track[0]].left, canvas.paths[progress[0]].right, 0.01)); // no gap
    assert!(canvas.path_stroke_styles[progress[0]].cap == StrokeCap::Butt);
    let rule = KompozTestRule::new(LegacyCircularProgressIndicatorSample);
    let canvas = rule.draw();
    let (track, arc) = (&canvas.arcs[0], &canvas.arcs[1]);
    assert!(track.color == Color::Transparent && arc.color == cs.primary);
    assert!(near(track.start, arc.start + arc.sweep, 0.01)); // no gap
}

fn progress_by_name() {
    // The deprecated value forms; a stop drawn by `ProgressIndicatorDefaults.drawStopIndicator(drawScope = this, …)`;
    // the wavy ones' defaults by name.
    let rule = KompozTestRule::new(ProgressByName);
    rule.on_node_with_tag("eski").assert_range_info_equals(determinate(0.25));
    rule.on_node_with_tag("eski halka").assert_range_info_equals(determinate(0.75));
    rule.on_node_with_tag("durak").assert_range_info_equals(determinate(0.5));
    rule.on_node_with_tag("belirsiz dalga").assert_range_info_equals(ProgressBarRangeInfo::Indeterminate);
    rule.on_node_with_tag("dalgalı halka").assert_range_info_equals(determinate(0.5));
    let canvas = rule.draw();
    let red: Vec<usize> = (0..canvas.rects.len()).filter(|&i| canvas.rect_colors[i] == Color::Red).collect();
    assert!(red.len() == 1 && canvas.rect_radii[red[0]] == 0.0); // a square: StrokeCap.Butt
    // The deprecated circular one: a 2 dp stroke, its track transparent.
    let halka =
        canvas.arcs.iter().find(|a| a.stroke_width == 2.0 && a.color != Color::Transparent).expect("eski halka");
    assert!(near(halka.sweep, 270.0, 0.01));
}

fn containers_loading_indicators() {
    // Containers' `LoadingIndicator(modifier = …)` and `LoadingIndicator(progress = { 0.5f }, …)` (the C++'s
    // `progress = 0.5f`): there (C++'s check), an endless one and one half way.
    let rule = KompozTestRule::new(ContainersLoadingIndicators);
    rule.on_node_with_tag("yükleniyor").assert_exists().assert_range_info_equals(ProgressBarRangeInfo::Indeterminate);
    rule.on_node_with_tag("yarım").assert_exists().assert_range_info_equals(determinate(0.5));
}

fn graphics_forms_loading_indicator() {
    // GraphicsForms' `ContainedLoadingIndicator(progress = { 0.3f }, …, containerColor = Color.Yellow)`: 48 dp wide
    // (C++'s check), in a yellow container.
    let rule = KompozTestRule::new(GraphicsFormsLoadingIndicator);
    let bounds = rule.on_node_with_tag("kap").get_unclipped_bounds_in_root();
    assert!(near(bounds.width(), 48.0, 0.5), "{bounds:?}");
    rule.on_node_with_tag("kap").assert_range_info_equals(determinate(0.3));
    let canvas = rule.draw();
    assert!(canvas.rect_colors == [Color::Yellow] && canvas.path_colors == [light_color_scheme().on_primary_container]);
}

fn loading_indicator_samples() {
    // The endless ones: 48 dp, the shape in the primary color (contained: on primary container, in a primary
    // container circle).
    let cs = light_color_scheme();
    let rule = KompozTestRule::new(LoadingIndicatorSample);
    assert_eq!(progress_of(&rule), [ProgressBarRangeInfo::Indeterminate]);
    let canvas = rule.draw();
    assert!(canvas.path_colors == [cs.primary] && canvas.rects.is_empty());
    assert!(near(rule.on_root().get_unclipped_bounds_in_root().height(), 48.0, 0.01));
    let rule = KompozTestRule::new(ContainedLoadingIndicatorSample);
    assert_eq!(progress_of(&rule), [ProgressBarRangeInfo::Indeterminate]);
    let canvas = rule.draw();
    assert!(canvas.path_colors == [cs.on_primary_container] && canvas.rect_colors == [cs.primary_container]);
    // The determinate ones: none to start, the slider's value animated to (a spring) as it is set.
    let determinate_samples: [fn() -> KompozTestRule; 2] = [
        || KompozTestRule::new(DeterminateLoadingIndicatorSample),
        || KompozTestRule::new(DeterminateContainedLoadingIndicatorSample),
    ];
    for make in determinate_samples {
        let rule = make();
        rule.on_node_with_text("Set loading progress:").assert_exists();
        assert_eq!(progress_of(&rule), [determinate(0.0)]);
        set_slider(&rule, 0.6);
        let now = progress_of(&rule)[0].current;
        assert!(near(now, 0.6, 0.002), "{now}");
    }
}

fn loading_indicators_by_name() {
    // Colors, a container's shape (8 dp corners) and polygons by name: the defaults' lists and a list of two of the
    // endless shapes.
    let rule = KompozTestRule::new(LoadingIndicatorsByName);
    rule.on_node_with_tag("patlama").assert_range_info_equals(determinate(1.0));
    rule.on_node_with_tag("dönen").assert_range_info_equals(ProgressBarRangeInfo::Indeterminate);
    rule.on_node_with_tag("kare").assert_range_info_equals(ProgressBarRangeInfo::Indeterminate);
    rule.on_node_with_text("48.0.dp 38.0.dp 7").assert_exists();
    let canvas = rule.draw();
    assert_eq!(canvas.path_colors, [Color::Red, Color::Magenta, Color::Green]);
    assert_eq!(canvas.rect_colors, [Color::Blue]);
    assert!(canvas.clip_shapes.iter().any(|s| near(s.radii.top_left, 8.0, 0.01)));
}

fn main() {
    common::run(&[
        ("material_forms_progress", material_forms_progress),
        ("graphics_forms_progress", graphics_forms_progress),
        ("determinate_samples", determinate_samples),
        ("thick_wavy_samples", thick_wavy_samples),
        ("indeterminate_samples", indeterminate_samples),
        ("legacy_samples", legacy_samples),
        ("progress_by_name", progress_by_name),
        ("containers_loading_indicators", containers_loading_indicators),
        ("graphics_forms_loading_indicator", graphics_forms_loading_indicator),
        ("loading_indicator_samples", loading_indicator_samples),
        ("loading_indicators_by_name", loading_indicators_by_name),
    ]);
}

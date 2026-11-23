//! Material's selection controls in Kotlin's syntax, as a Compose user writes them (the C++ Kompoz's NamedHeads, with
//! its Checkbox, is kotlin_buttons.rs' `required_arguments_by_name`): material3's samples for Checkbox,
//! TriStateCheckbox, RadioButton and Switch (with an icon in its thumb) from a `.kt` file
//! (kompoz_file!), with colors given through `CheckboxDefaults.colors(…)`, `RadioButtonDefaults.colors(…)` and
//! `SwitchDefaults.colors(…)`; the sliders (kt/slider.kt): the slider half of the C++'s Expressive source
//! (kompozable_tests.cpp `expressive_buttons_and_sliders_as_compose_writes_them`) as M15 accepts it and material3's
//! SliderSamples.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::InjectionScope;
use kompoz::test::{KompozTestRule, has_role, is_off, is_on, is_toggleable};
use kompoz::ui::semantics::{Role, SemanticsActions, SemanticsProperties, ToggleableState};

// ---- material3's samples (kt/controls.kt) -----------------------------------------------------------------------

kompoz_file!("tests/kt/controls.kt");

/// The toggleable (merged) nodes and their states, in order.
fn toggle_states(rule: &KompozTestRule) -> Vec<Option<ToggleableState>> {
    rule.on_all_nodes_with(is_toggleable(), false)
        .fetch_semantics_nodes()
        .iter()
        .map(|n| n.config().get(SemanticsProperties::ToggleableState))
        .collect()
}

fn checkbox_samples() {
    // CheckboxSample: checked, a click unchecks it.
    let rule = KompozTestRule::new(CheckboxSample);
    let checkbox = rule.on_node_with(is_toggleable(), false);
    checkbox.assert_is_on().assert(has_role(Role::Checkbox)).perform_click();
    rule.on_node_with(is_toggleable(), false).assert_is_off();
    // CheckboxWithTextSample: the row is the toggleable (the checkbox shares its interaction source), the text in
    // it; a tap on the text toggles.
    let rule = KompozTestRule::new(CheckboxWithTextSample);
    rule.on_node_with_text("Option selection").perform_click();
    assert_eq!(toggle_states(&rule).first().copied().flatten(), Some(ToggleableState::Off));
    // The checkbox inside takes its own clicks too (it has a callback): both are toggleables.
    assert_eq!(toggle_states(&rule).len(), 2);
    // CheckboxRoundedStrokesSample: its check mark drawn with round caps and joins.
    let rule = KompozTestRule::new(CheckboxRoundedStrokesSample);
    rule.wait_for_idle();
    let canvas = rule.draw();
    let stroke = canvas.path_stroke_styles.last().expect("a check mark");
    assert!(stroke.cap == StrokeCap::Round && stroke.join == StrokeJoin::Round && stroke.width == 2.0);
    rule.on_node_with(is_on(), false).perform_click();
    rule.on_node_with(is_off(), false).assert(has_role(Role::Checkbox));
}

fn tri_state_checkbox_samples() {
    for rule in [KompozTestRule::new(TriStateCheckboxSample), KompozTestRule::new(TriStateCheckboxRoundedStrokesSample)]
    {
        // (the rows are the toggleables: the checkboxes inside take no clicks)
        assert_eq!(toggle_states(&rule), vec![Some(ToggleableState::On); 3]);
        rule.on_node_with_text("Daily").perform_click();
        assert_eq!(
            toggle_states(&rule),
            vec![Some(ToggleableState::Indeterminate), Some(ToggleableState::Off), Some(ToggleableState::On)]
        );
        // Partly checked: the parent's click checks them all.
        rule.on_node_with_text("Receive Emails").perform_click();
        assert_eq!(toggle_states(&rule), vec![Some(ToggleableState::On); 3]);
        rule.on_node_with_text("Receive Emails").perform_click();
        assert_eq!(toggle_states(&rule), vec![Some(ToggleableState::Off); 3]);
        // The parent's dash drawn: a straight line across once partly checked.
        rule.on_node_with_text("Weekly").perform_click();
        rule.wait_for_idle();
        let canvas = rule.draw();
        assert!(canvas.paths.iter().any(|p| p.height() < 0.01 && p.width() > 10.0), "{:?}", canvas.paths);
    }
}

fn radio_button_samples() {
    // RadioButtonSample: two buttons in a selectable group, one selected; a click selects the other.
    let rule = KompozTestRule::new(RadioButtonSample);
    let buttons = rule.on_all_nodes_with_content_description("Localized Description");
    buttons.assert_count_equals(2);
    buttons.get(0).assert_is_selected().assert(has_role(Role::RadioButton));
    buttons.get(1).assert_is_not_selected().perform_click();
    let buttons = rule.on_all_nodes_with_content_description("Localized Description");
    buttons.get(0).assert_is_not_selected();
    buttons.get(1).assert_is_selected();
    // RadioGroupSample: the rows are the selectables, the first selected; a tap on a text selects its row.
    let rule = KompozTestRule::new(RadioGroupSample);
    rule.on_node_with_text("Calls").assert_is_selected();
    rule.on_node_with_text("Friends").perform_click();
    rule.on_node_with_text("Friends").assert_is_selected();
    rule.on_node_with_text("Calls").assert_is_not_selected();
    let group = rule.all_nodes(false).into_iter().filter(|n| n.config().contains(SemanticsProperties::SelectableGroup));
    assert_eq!(group.count(), 1);
}

fn switch_samples() {
    // SwitchSample: on, described; a click turns it off.
    let rule = KompozTestRule::new(SwitchSample);
    rule.on_node_with_content_description("Demo").assert_is_on().assert(has_role(Role::Switch)).perform_click();
    rule.on_node_with_content_description("Demo").assert_is_off();
    // SwitchWithThumbIconSample: the check icon in the thumb while on (in the icon color: the primary container's
    // content color), none once off.
    let rule = KompozTestRule::new(SwitchWithThumbIconSample);
    rule.wait_for_idle();
    let canvas = rule.draw();
    let scheme = light_color_scheme();
    assert!(canvas.vector_colors == vec![scheme.on_primary_container], "{:?}", canvas.vector_colors);
    // (16 dp in the 24 dp thumb at the track's end)
    let switch = rule.on_node_with_content_description("Demo with icon").get_unclipped_bounds_in_root();
    let icon = canvas.vector_bounds[0];
    assert!((icon.width() - 16.0).abs() < 0.01 && (icon.right - (switch.right - 8.0)).abs() < 0.01, "{icon:?}");
    rule.on_node_with_content_description("Demo with icon").perform_click();
    rule.wait_for_idle();
    assert!(rule.draw().vector_colors.is_empty());
}

fn control_colors_by_name() {
    let enabled = mutable_state_of(true);
    let e = enabled.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || ColoredControls(e.get())));
    rule.wait_for_idle();
    let canvas = rule.draw();
    // The checkbox blue with a yellow tick; the partly checked one green (its box's and border's colors given by
    // part); the radio button magenta; the switch's track red, its thumb white.
    assert!(canvas.rect_colors.contains(&Color::Blue) && canvas.path_colors.contains(&Color::Yellow));
    assert!(canvas.rect_colors.contains(&Color::Green));
    assert!(canvas.stroke_colors.contains(&Color::Magenta) && canvas.rect_colors.contains(&Color::Magenta));
    assert!(canvas.rect_colors.contains(&Color::Red) && canvas.rect_colors.contains(&Color::White));
    rule.on_node_with_text("aynı").assert_exists();
    // Disabled: the radio button in its disabled selected color.
    enabled.set(false);
    rule.wait_for_idle();
    assert!(rule.draw().stroke_colors.contains(&Color::Gray));
    rule.on_node_with_tag("kutu").assert_is_not_enabled();
}

// ---- the sliders (kt/slider.kt) ---------------------------------------------------------------------------------

kompoz_file!("tests/kt/slider.kt");

/// Sets a slider's value as accessibility does (its `SetProgress` action).
fn set_progress(node: kompoz::test::SemanticsNodeInteraction<'_>, value: f32) {
    node.perform_semantics_action_with(SemanticsActions::SetProgress, |f| {
        f(value);
    });
}

/// What is drawn in red (the stop indicators the Kotlin source draws while bold).
fn red_drawn(rule: &KompozTestRule) -> bool {
    let canvas = rule.draw();
    canvas.path_colors.contains(&Color::Red) || canvas.rect_colors.contains(&Color::Red)
}

fn expressive_sliders() {
    // The slider half of the C++ Expressive: a value in a valueRange, a Label over the thumb while it is held (a
    // PlainTooltip of the value), a track whose stop indicator is a lambda or null; a RangeSlider of a float
    // range value with steps; remembered slider states; `price.start`, `.endInclusive`, `500f in price`.
    let rule = KompozTestRule::new(ExpressiveSliders);
    rule.on_node_with_text("200–800 · 3 · 17 · 40 · içinde").assert_exists();
    rule.on_node_with_text("bitti: 0").assert_exists(); // (a `var finished: Int? by …`)
    set_progress(rule.on_node_with_tag("ses"), 75.0);
    rule.on_node_with_text("200–800 · 3 · 17 · 75 · içinde").assert_exists();
    rule.on_node_with_text("bitti: 1").assert_exists();
    // Held, its value over the thumb; let go, the tap in its middle sets it to 50.
    rule.on_node_with_tag("değer").assert_does_not_exist();
    rule.on_node_with_tag("ses").perform_touch_input(|t| t.down(t.center()));
    rule.advance_frame();
    rule.wait_for_idle_within(0);
    rule.on_node_with_tag("değer").assert_exists().assert_text_equals(&["75"]);
    rule.on_node_with_tag("ses").perform_touch_input(|t| t.up());
    rule.advance_frame();
    rule.on_node_with_tag("değer").assert_does_not_exist();
    // The range's start, on its steps (50 apart): 600 leaves 500 out.
    set_progress(rule.on_all_nodes_with_content_description("Range start").get(0), 610.0);
    rule.on_node_with_text("600–800 · 3 · 17 · 50 · dışında").assert_exists();
    rule.on_node_with_text("bitti: 2").assert_exists();
    // The remembered states: the stars on their steps, the hours' end.
    set_progress(rule.on_node_with_tag("yıldız"), 4.4);
    set_progress(rule.on_all_nodes_with_content_description("Range end").get(1), 20.0);
    rule.on_node_with_text("600–800 · 4 · 20 · 50 · dışında").assert_exists();
    // The stop indicator: none while not bold (null), the red dot when bold.
    assert!(!red_drawn(&rule));
    rule.on_node_with_tag("kalın").perform_click();
    assert!(red_drawn(&rule));
}

fn slider_samples() {
    // SliderSample, LegacySliderSample (a 20 dp thumb with its own ripple, a 4 dp track without gaps or stop
    // indicators), StepsSliderSample, SliderWithCustomTrackAndThumbSample (red), CenteredSliderSample.
    let rule = KompozTestRule::new(SliderSample);
    rule.on_node_with_text("0.00").assert_exists();
    set_progress(
        rule.on_node_with(kompoz::test::SemanticsMatcher::key_is_defined(SemanticsActions::SetProgress), false),
        0.5,
    );
    rule.on_node_with_text("0.50").assert_exists();

    let rule = KompozTestRule::new(LegacySliderSample);
    let canvas = rule.draw();
    assert!(canvas.rects.iter().any(|r| r.width() == 20.0 && r.height() == 20.0), "{:?}", canvas.rects);
    assert!(canvas.paths.iter().any(|p| p.height() == 4.0 && p.width() > 100.0));
    set_progress(
        rule.on_node_with(kompoz::test::SemanticsMatcher::key_is_defined(SemanticsActions::SetProgress), false),
        0.25,
    );
    rule.on_node_with_text("0.25").assert_exists();

    let rule = KompozTestRule::new(StepsSliderSample);
    set_progress(
        rule.on_node_with(kompoz::test::SemanticsMatcher::key_is_defined(SemanticsActions::SetProgress), false),
        33.0,
    );
    rule.on_node_with_text("30.00").assert_exists();

    let rule = KompozTestRule::new(SliderWithCustomTrackAndThumbSample);
    let canvas = rule.draw();
    assert!(canvas.rect_colors.contains(&Color::Red)); // the thumb
    set_progress(
        rule.on_node_with(kompoz::test::SemanticsMatcher::key_is_defined(SemanticsActions::SetProgress), false),
        50.0,
    );
    assert!(rule.draw().path_colors.contains(&Color::Red)); // the active stretch
    rule.on_node_with_text("50.00").assert_exists();

    let rule = KompozTestRule::new(CenteredSliderSample);
    rule.on_node_with_text("0.00").assert_exists();
    set_progress(
        rule.on_node_with(kompoz::test::SemanticsMatcher::key_is_defined(SemanticsActions::SetProgress), false),
        -25.0,
    );
    rule.on_node_with_text("-25.00").assert_exists();
}

/// Down on `node`'s center, held until its label shows.
fn hold(rule: &KompozTestRule, node: kompoz::test::SemanticsNodeInteraction<'_>) {
    node.perform_touch_input(|t| t.down(t.center()));
    rule.advance_frame();
    rule.wait_for_idle_within(0);
}

/// The slider found by its set-progress action.
fn the_slider(rule: &KompozTestRule) -> kompoz::test::SemanticsNodeInteraction<'_> {
    rule.on_node_with(kompoz::test::SemanticsMatcher::key_is_defined(SemanticsActions::SetProgress), false)
}

fn sliders_with_custom_thumbs_and_labels() {
    // SliderWithCustomThumbSample: a red Favorite icon for the thumb; held, its value in a PlainTooltip over it.
    let rule = KompozTestRule::new(SliderWithCustomThumbSample);
    let favorite = Icons::Filled::Favorite().id();
    let canvas = rule.draw();
    let at = canvas.vectors.iter().position(|v| *v == favorite).expect("the thumb's icon");
    assert_eq!(canvas.vector_colors[at], Color::Red);
    assert_eq!(canvas.vector_bounds[at].width(), 18.0); // (ButtonDefaults.IconSize)
    set_progress(the_slider(&rule), 50.0);
    rule.on_node_with_text("50.00").assert_does_not_exist();
    hold(&rule, the_slider(&rule));
    let tooltip = rule.on_node_with_text("50.00").assert_exists().get_unclipped_bounds_in_root();
    // (centered on the thumb, above it or, the slider at the window's top, below: the tooltip position provider's)
    let thumb = rule.draw().vector_bounds[at];
    assert!(
        tooltip.top >= thumb.bottom && (tooltip.center().x - thumb.center().x).abs() < 1.0,
        "{tooltip:?} {thumb:?}"
    );
    the_slider(&rule).perform_touch_input(|t| t.up());
    rule.advance_frame();
    rule.on_node_with_text("50.00").assert_does_not_exist();

    // RangeSliderWithCustomComponents: a blue start thumb and a green end one, the active stretch red; each thumb held
    // shows its end's value in a PlainTooltip.
    let rule = KompozTestRule::new(RangeSliderWithCustomComponents);
    let canvas = rule.draw();
    assert!(canvas.rect_colors.contains(&Color::Blue) && canvas.rect_colors.contains(&Color::Green));
    assert!(canvas.path_colors.contains(&Color::Red) || canvas.rect_colors.contains(&Color::Red));
    set_progress(rule.on_node_with_content_description("Range start"), 25.0);
    hold(&rule, rule.on_node_with_content_description("Range start"));
    rule.on_node_with_text("25.00").assert_exists();
    rule.on_node_with_text("100.00").assert_does_not_exist();
    rule.on_node_with_content_description("Range start").perform_touch_input(|t| t.up());
    rule.advance_frame();
    rule.on_node_with_text("25.00").assert_does_not_exist();
}

fn slider_with_track_icons_sample() {
    // SliderWithTrackIconsSample: vector painters of MusicNote and MusicOff drawn at the ends of each stretch of the
    // track wide enough for them (Painter's `DrawScope.draw` in `with(painter) { … }`), the active one's in the active
    // tick color, the inactive one's in the inactive one (M15's: the inactive track's color and the active one's).
    let rule = KompozTestRule::new(SliderWithTrackIconsSample);
    let (note, off) = (Icons::Filled::MusicNote().id(), Icons::Filled::MusicOff().id());
    let icons = |rule: &KompozTestRule| {
        let canvas = rule.draw();
        canvas
            .vectors
            .iter()
            .zip(&canvas.vector_colors)
            .filter(|(v, _)| **v == note || **v == off)
            .map(|(v, c)| (*v == note, *c))
            .collect::<Vec<_>>()
    };
    let colors = light_color_scheme();
    let (active, inactive) = (colors.secondary_container, colors.primary);
    // At 0 only the inactive stretch has room: its two icons.
    rule.on_node_with_text("0.00").assert_exists();
    assert_eq!(icons(&rule), [(true, inactive), (false, inactive)]);
    // At 50 both stretches: four.
    set_progress(the_slider(&rule), 50.0);
    rule.on_node_with_text("50.00").assert_exists();
    assert_eq!(icons(&rule), [(true, active), (false, active), (true, inactive), (false, inactive)]);
    // (36 dp tall, 20 dp icons in its middle)
    let canvas = rule.draw();
    let at = canvas.vectors.iter().position(|v| *v == note).expect("drawn");
    assert_eq!(canvas.vector_bounds[at].width(), 20.0);
}

fn vertical_sliders() {
    // VerticalSliderSample and VerticalCenteredSliderSample as M15 writes them (reversed: they grow upwards; the
    // auto-snap off, the state's `onValueChange` and `onValueChangeFinished` set; a drag sets the value and cancels
    // the snap animation running, its end snaps it by an animation kept in a `Job?` state).
    // (where a drag starts, from the slider's bounds; the values shown at the start, after a drag 148 px up, and after
    // the two drags below)
    type From = fn(Rect) -> f32;
    let samples: [(fn() -> KompozTestRule, From, [&str; 3]); 2] = [
        (|| KompozTestRule::new(VerticalSliderSample), |b| b.bottom - 2.0, ["0.00", "50.00", "0.00"]),
        (|| KompozTestRule::new(VerticalCenteredSliderSample), |b| b.center().y, ["0.00", "50.00", "-20.00"]),
    ];
    for (make, from, [start, up, end]) in samples {
        let rule = make();
        rule.on_node_with_text(start).assert_exists();
        // Half way up: 142 of 298 px past the slop, the nearest step 50 more.
        let b = the_slider(&rule).get_unclipped_bounds_in_root();
        let (x, from) = (b.center().x, from(b));
        let drag = |rule: &KompozTestRule, by: f32| {
            rule.host().pointer_down(Offset::new(x, from), -1, 0, kompoz::ui::input::PointerType::Touch);
            for i in 1..=10 {
                rule.advance_frame();
                rule.host().pointer_move(Offset::new(x, from - by / 10.0 * i as f32), -1, 0);
            }
            rule.host().pointer_up(Offset::new(x, from - by), -1, 0);
        };
        drag(&rule, 148.0);
        rule.wait_for_idle();
        rule.on_node_with_text(up).assert_exists();
        // A press sets where a drag starts: a quarter up, then down past it while the first snap animation runs —
        // the new drag cancels its job (the value follows the drag alone) and its end snaps it.
        rule.main_clock().set_auto_advance(false);
        drag(&rule, 74.0);
        rule.advance_frame();
        drag(&rule, -74.0);
        rule.main_clock().set_auto_advance(true);
        rule.wait_for_idle();
        rule.on_node_with_text(end).assert_exists();
    }
}

fn range_slider_samples() {
    // RangeSliderSample, LegacyRangeSliderSample (20 dp thumbs, a 4 dp track), StepRangeSliderSample.
    let rule = KompozTestRule::new(RangeSliderSample);
    rule.on_node_with_text("0.00 .. 100.00").assert_exists();
    set_progress(rule.on_node_with_content_description("Range start"), 25.0);
    rule.on_node_with_text("25.00 .. 100.00").assert_exists();

    let rule = KompozTestRule::new(LegacyRangeSliderSample);
    let canvas = rule.draw();
    assert_eq!(canvas.rects.iter().filter(|r| r.width() == 20.0 && r.height() == 20.0).count(), 2);
    set_progress(rule.on_node_with_content_description("Range end"), 60.0);
    rule.on_node_with_text("0.00 .. 60.00").assert_exists();

    let rule = KompozTestRule::new(StepRangeSliderSample);
    rule.on_node_with_text("0 .. 100").assert_exists();
    set_progress(rule.on_node_with_content_description("Range start"), 33.0);
    rule.on_node_with_text("30 .. 100").assert_exists();
}

fn main() {
    common::run(&[
        ("checkbox_samples", checkbox_samples),
        ("tri_state_checkbox_samples", tri_state_checkbox_samples),
        ("radio_button_samples", radio_button_samples),
        ("switch_samples", switch_samples),
        ("control_colors_by_name", control_colors_by_name),
        ("expressive_sliders", expressive_sliders),
        ("slider_samples", slider_samples),
        ("sliders_with_custom_thumbs_and_labels", sliders_with_custom_thumbs_and_labels),
        ("slider_with_track_icons_sample", slider_with_track_icons_sample),
        ("vertical_sliders", vertical_sliders),
        ("range_slider_samples", range_slider_samples),
    ]);
}

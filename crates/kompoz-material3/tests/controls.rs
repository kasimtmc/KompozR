//! Material's selection controls as Compose's material3 has them: Checkbox and TriStateCheckbox, RadioButton,
//! Switch — how they draw and animate, their colors, their ripple, the room they keep, what they tell
//! accessibility. The C++ Kompoz's tests by name (kompoz_tests.cpp: their Material halves where the rest is a lower
//! crate's), and Compose's behaviour. (The Checkboxes of C++ test_rule_assertions and
//! semantics_tree_merges_buttons_and_performs_actions are in buttons_and_containers.rs' tests of those names, with
//! their Buttons.)

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::interaction::{Interaction, PressInteraction};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, has_no_click_action, has_role};
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::{Role, SemanticsProperties, ToggleableState};
use kompoz_ui::*;
use std::cell::Cell;
use std::rc::Rc;

tests!(
    checkbox_toggles,
    checkbox_draws_as_compose_1_4,
    checkbox_styling_fix_as_compose_1_5,
    tri_state_checkbox_shows_a_partly_checked_group,
    radio_button_draws_as_compose_1_4,
    switch_colors_and_press_as_compose_1_4,
    switch_shows_an_icon_in_its_thumb_and_grows_it_while_held,
    disabled_switch_and_checkbox_ignore_taps,
    checkboxes_and_radio_buttons_ripple_and_hover_in_a_40_dp_circle,
    chips_radio_buttons_and_segments_select,
    selectable_and_toggleable_rows_take_the_tap_for_their_controls,
    controls_tell_accessibility_their_role_and_state,
    controls_keep_48_dp_of_room_only_when_they_take_clicks,
    control_colors_come_from_the_scheme_and_copy_what_is_given,
    controls_keep_48_dp_of_room_and_take_taps_anywhere_in_it,
    rtl_mirrors_arrows_switches_and_sliders,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left, 0.01)
        && near(a.top, b.top, 0.01)
        && near(a.width(), b.width(), 0.01)
        && near(a.height(), b.height(), 0.01)
}

/// C++'s `Rect{x, y, width, height}`.
fn xywh(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(x, y, x + width, y + height)
}

fn same_color(a: Color, b: Color) -> bool {
    near(a.red(), b.red(), 0.01)
        && near(a.green(), b.green(), 0.01)
        && near(a.blue(), b.blue(), 0.01)
        && near(a.alpha(), b.alpha(), 0.01)
}

fn faded(c: Color, alpha: f32) -> Color {
    named_args!(c.copy(alpha = alpha))
}

/// A checked-change callback (Kotlin's `{ checked = it }`).
fn on_change(f: impl Fn(bool) + 'static) -> Option<Rc<dyn Fn(bool)>> {
    Some(Rc::new(f))
}

/// A click callback.
fn on_click(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
}

/// Composable content given as an optional slot (Kotlin's `thumbContent = { … }`).
fn composable(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
}

/// The frames after a state changes that an animation has moved by: Compose's — the frame of the change composes,
/// the animation starts at the next one (its value still the start's), it moves at the one after (the C++ Kompoz's
/// animations moved at the second frame).
fn after_change(rule: &KompozTestRule) {
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
}

fn tap(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.host().pointer_up(at, -1, 0);
}

fn mouse_to(rule: &KompozTestRule, p: Offset) {
    rule.on_root().perform_mouse_input(|m| m.move_to(p));
}

/// The nodes in a toggleable `state`.
fn count_in(rule: &KompozTestRule, state: ToggleableState) -> usize {
    rule.all_nodes(false).iter().filter(|n| n.config().get(SemanticsProperties::ToggleableState) == Some(state)).count()
}

// ---- C++ checkbox_toggles -------------------------------------------------------------------------------------

fn checkbox_toggles() {
    let checked = mutable_state_of(false);
    let c = checked.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        let c2 = c.clone();
        Checkbox(c.get(), on_change(move |v| c2.set(v)), modifier = Modifier.test_tag("cb"));
    }));
    rule.on_node_with_tag("cb").perform_click();
    assert!(checked.peek());
    // (C++'s text "checked": Compose's toggleable state)
    rule.on_node_with_tag("cb").assert_is_on();
}

// ---- C++ checkbox_draws_as_compose_1_4 --------------------------------------------------------------------------

fn checkbox_draws_as_compose_1_4() {
    // material3 1.5 (M15) changed Checkbox.kt and CheckboxTokens.kt from 1.4: the corrected styling behind
    // ComposeMaterial3Flags.isCheckboxStylingFixEnabled (off by default: checkbox_styling_fix_as_compose_1_5), a
    // disabledCheckmarkColor (used only with the fix), CheckboxTokens.ContainerHeight/Width → ContainerSize (18 dp
    // both). With the flag at its default, a checkbox draws as 1.4's: these are the C++'s values unchanged.
    let state = mutable_state_of(ToggleableState::Off);
    let enabled = mutable_state_of(true);
    let (s, e) = (state.clone(), enabled.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        TriStateCheckbox(s.get(), on_click(|| {}), modifier = Modifier.test_tag("kutu"), enabled = e.get());
    }));
    let cs = light_color_scheme();
    // (the clock by hand: the drawings on the way see the tick turning)
    rule.main_clock().set_auto_advance(false);
    let drawn = |rule: &KompozTestRule| {
        let canvas = rule.draw();
        let c = bounds(rule, "kutu").center();
        (canvas, xywh(c.x - 10.0, c.y - 10.0, 20.0, 20.0))
    };
    // Off: a 2 dp outline inside the 20 dp box, in the surface's variant text color; no tick.
    let (off, b) = drawn(&rule);
    assert!(off.strokes.len() == 1 && same_rect(off.strokes[0], b), "{:?} {b:?}", off.strokes);
    assert!(same_color(off.stroke_colors[0], cs.on_surface_variant) && off.paths.is_empty(), "{:?}", off.paths);
    assert!(near(off.stroke_corners[0].top_left, 2.0, 0.01));
    // Ticked: drawn along, then all of it: one 20 dp box in the primary color, the tick in its text color.
    state.set(ToggleableState::On);
    after_change(&rule);
    let (growing, _) = drawn(&rule);
    assert!(growing.paths.len() == 1 && growing.paths[0].width() < 11.0, "{:?}", growing.paths);
    rule.advance_time_by(1000);
    let (on, b) = drawn(&rule);
    assert!(on.strokes.is_empty() && on.rects.len() == 1 && same_rect(on.rects[0], b), "{:?}", on.rects);
    assert!(same_color(on.rect_colors[0], cs.primary) && near(on.rect_radii[0], 2.0, 0.01));
    assert!(
        on.paths.len() == 1 && same_rect(on.paths[0], xywh(b.left + 4.0, b.top + 6.0, 12.0, 8.0)),
        "{:?}",
        on.paths
    );
    assert!(same_color(on.path_colors[0], cs.on_primary) && near(on.path_strokes[0], 2.0, 0.01));
    // Indeterminate from ticked: bent into a dash across the middle.
    state.set(ToggleableState::Indeterminate);
    after_change(&rule);
    let (bending, _) = drawn(&rule);
    assert!(bending.paths.len() == 1 && bending.paths[0].height() > 0.5 && bending.paths[0].height() < 7.5);
    rule.advance_time_by(1000);
    let (dash, b) = drawn(&rule);
    assert!(dash.paths.len() == 1 && same_rect(dash.paths[0], xywh(b.left + 4.0, b.top + 10.0, 12.0, 0.0)));
    // Unticked: the tick whole for 100 ms (only fading), then gone at once.
    state.set(ToggleableState::Off);
    after_change(&rule);
    let (fading, _) = drawn(&rule);
    // Faded by the fast effects spring (most of it within 50 ms), still a straight dash.
    rule.advance_frame();
    rule.advance_frame();
    let (faded_out, _) = drawn(&rule);
    assert!(
        faded_out.paths.len() == 1
            && near(faded_out.paths[0].height(), 0.0, 0.05)
            && faded_out.rect_colors[0].alpha() < 0.3
    );
    assert!(
        fading.paths.len() == 1 && near(fading.paths[0].width(), 12.0, 0.01) && fading.path_colors[0].alpha() < 0.99
    );
    rule.advance_time_by(1000);
    assert!(drawn(&rule).0.paths.is_empty());
    // Indeterminate from off: a dash from the start, drawn along.
    state.set(ToggleableState::Indeterminate);
    after_change(&rule);
    let (dashing, _) = drawn(&rule);
    assert!(dashing.paths.len() == 1 && near(dashing.paths[0].height(), 0.0, 0.01) && dashing.paths[0].width() < 11.0);
    rule.advance_time_by(1000);
    // Disabled: the surface's text faded, at once; the tick still in the primary's text color.
    state.set(ToggleableState::On);
    rule.advance_time_by(1000);
    enabled.set(false);
    rule.advance_frame();
    let (disabled_on, _) = drawn(&rule);
    assert!(disabled_on.rects.len() == 1 && same_color(disabled_on.rect_colors[0], faded(cs.on_surface, 0.38)));
    assert!(disabled_on.paths.len() == 1 && same_color(disabled_on.path_colors[0], cs.on_primary));
    state.set(ToggleableState::Off);
    rule.advance_time_by(1000);
    let (disabled_off, b) = drawn(&rule);
    assert!(disabled_off.strokes.len() == 1 && same_color(disabled_off.stroke_colors[0], faded(cs.on_surface, 0.38)));
    assert!(
        disabled_off.rects.len() == 1
            && disabled_off.rect_colors[0].alpha() == 0.0
            && same_rect(disabled_off.rects[0], xywh(b.left + 2.0, b.top + 2.0, 16.0, 16.0))
    );
}

fn checkbox_styling_fix_as_compose_1_5() {
    // material3 1.5's ComposeMaterial3Flags.isCheckboxStylingFixEnabled: the box 18 dp (CheckboxTokens.ContainerSize)
    // without its 2 dp of padding, the tick from 25% to 75% across (its cross at 65% down), the disabled tick in
    // its own color (SelectedDisabledIconColor: the surface), the ripple in the box's color.
    ComposeMaterial3Flags::set_is_checkbox_styling_fix_enabled(true);
    struct Off;
    impl Drop for Off {
        fn drop(&mut self) {
            ComposeMaterial3Flags::set_is_checkbox_styling_fix_enabled(false);
        }
    }
    let _off = Off;
    let enabled = mutable_state_of(true);
    let e = enabled.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let e = e.clone();
        Column(move || {
            Checkbox(true, on_change(|_| {}), modifier = Modifier.test_tag("kutu"), enabled = e.get());
            Checkbox(true, None, modifier = Modifier.test_tag("sade"));
        });
    }));
    let cs = light_color_scheme();
    // (no padding: 18 dp on its own)
    assert!(same_rect(bounds(&rule, "sade"), xywh(0.0, 48.0, 18.0, 18.0)), "{:?}", bounds(&rule, "sade"));
    rule.wait_for_idle();
    let c = bounds(&rule, "kutu").center();
    let canvas = rule.draw();
    let b = xywh(c.x - 9.0, c.y - 9.0, 18.0, 18.0);
    assert!(same_rect(canvas.rects[0], b) && same_color(canvas.rect_colors[0], cs.primary), "{:?}", canvas.rects);
    // 0.25 × 18 = 4.5 to 0.75 × 18 = 13.5 across, 0.3 × 18 = 5.4 to 0.65 × 18 = 11.7 down
    assert!(same_rect(canvas.paths[0], xywh(b.left + 4.5, b.top + 5.4, 9.0, 6.3)), "{:?}", canvas.paths);
    // Pressed: the ripple in the box's (primary) color.
    rule.host().pointer_down(c, -1, 0, PointerType::Touch);
    rule.advance_time_by(100);
    let pressed = rule.draw();
    let ripple = pressed
        .rect_colors
        .iter()
        .zip(&pressed.rects)
        .find(|(color, r)| color.alpha() > 0.0 && color.alpha() < 1.0 && near(r.center().x, c.x, 0.5));
    assert!(
        ripple.is_some_and(|(color, _)| same_color(named_args!(color.copy(alpha = 1.0)), cs.primary)),
        "{ripple:?}"
    );
    rule.host().pointer_up(c, -1, 0);
    rule.wait_for_idle();
    // Disabled: the tick in the surface's color (the disabled checkmark color).
    enabled.set(false);
    rule.wait_for_idle();
    let disabled = rule.draw();
    assert!(same_color(disabled.path_colors[0], cs.surface), "{:?}", disabled.path_colors);
}

// ---- C++ tri_state_checkbox_shows_a_partly_checked_group --------------------------------------------------------

fn tri_state_checkbox_shows_a_partly_checked_group() {
    let a = mutable_state_of(false);
    let b = mutable_state_of(false);
    let (a2, b2) = (a.clone(), b.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (a, b) = (a2.clone(), b2.clone());
            Column(move || {
                let all = if a.get() && b.get() {
                    ToggleableState::On
                } else if a.get() || b.get() {
                    ToggleableState::Indeterminate
                } else {
                    ToggleableState::Off
                };
                let (a1, b1) = (a.clone(), b.clone());
                TriStateCheckbox(
                    all,
                    on_click(move || {
                        let on = all != ToggleableState::On; // partly or not at all: all of them
                        a1.set(on);
                        b1.set(on);
                    }),
                    modifier = Modifier.test_tag("hepsi"),
                );
                let (a1, b1) = (a.clone(), b.clone());
                Checkbox(a.get(), on_change(move |v| a1.set(v)), modifier = Modifier.test_tag("a"));
                Checkbox(b.get(), on_change(move |v| b1.set(v)), modifier = Modifier.test_tag("b"));
            });
        }),
        IntSize::new(400, 800),
    );
    // (C++'s texts "unchecked", "indeterminate", "checked": Compose's toggleable states)
    assert_eq!(count_in(&rule, ToggleableState::Off), 3);
    rule.on_node_with_tag("a").perform_click();
    assert!(count_in(&rule, ToggleableState::Indeterminate) == 1 && count_in(&rule, ToggleableState::On) == 1);
    rule.on_node_with_tag("hepsi").perform_click();
    assert!(a.peek() && b.peek() && count_in(&rule, ToggleableState::On) == 3);
    rule.on_node_with_tag("hepsi").perform_click();
    assert!(!a.peek() && !b.peek() && count_in(&rule, ToggleableState::Off) == 3);
}

// ---- C++ radio_button_draws_as_compose_1_4 ----------------------------------------------------------------------

fn radio_button_draws_as_compose_1_4() {
    let selected = mutable_state_of(false);
    let enabled = mutable_state_of(true);
    let (s, e) = (selected.clone(), enabled.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        RadioButton(s.get(), on_click(|| {}), modifier = Modifier.test_tag("radyo"), enabled = e.get());
    }));
    let cs = light_color_scheme();
    let c = bounds(&rule, "radyo").center();
    // A 20 dp ring, 2 dp wide, in the surface's variant text color; no dot.
    let off = rule.draw();
    assert!(
        off.strokes.len() == 1
            && same_rect(off.strokes[0], xywh(c.x - 10.0, c.y - 10.0, 20.0, 20.0))
            && off.rects.is_empty()
    );
    assert!(same_color(off.stroke_colors[0], cs.on_surface_variant));
    // Selected: the dot grows to 5 dp across 10, both in the primary color, the color eased in.
    selected.set(true);
    after_change(&rule);
    let easing = rule.draw();
    assert!(easing.stroke_colors.len() == 1 && !same_color(easing.stroke_colors[0], cs.primary));
    rule.advance_time_by(1000);
    let on = rule.draw();
    assert!(on.rects.len() == 1 && same_rect(on.rects[0], xywh(c.x - 5.0, c.y - 5.0, 10.0, 10.0)), "{:?}", on.rects);
    assert!(same_color(on.rect_colors[0], cs.primary) && same_color(on.stroke_colors[0], cs.primary));
    // Unselected: the dot shrinks away (none of it drawn once under the ring's half width).
    selected.set(false);
    let (mut shrinking, mut inside) = (false, true);
    for _ in 0..30 {
        rule.advance_frame();
        for r in rule.draw().rects {
            shrinking |= r.width() > 0.5 && r.width() < 9.5;
            inside &= r.width() > 0.0;
        }
    }
    assert!(shrinking && inside);
    selected.set(true);
    rule.advance_time_by(1000);
    // Disabled: the surface's text faded, at once.
    enabled.set(false);
    rule.advance_frame();
    let disabled = rule.draw();
    assert!(
        same_color(disabled.stroke_colors[0], faded(cs.on_surface, 0.38))
            && same_color(disabled.rect_colors[0], faded(cs.on_surface, 0.38))
    );
    selected.set(false);
    rule.advance_time_by(1000);
    let disabled_off = rule.draw();
    assert!(same_color(disabled_off.stroke_colors[0], faded(cs.on_surface, 0.38)) && disabled_off.rects.is_empty());
}

// ---- C++ switch_colors_and_press_as_compose_1_4 -----------------------------------------------------------------

fn switch_colors_and_press_as_compose_1_4() {
    let on = mutable_state_of(false);
    let o = on.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let o = o.clone();
        Column(move || {
            let o2 = o.clone();
            Switch(o.get(), on_change(move |v| o2.set(v)), modifier = Modifier.test_tag("anahtar"));
            Switch(true, None, enabled = false);
            Switch(false, None, enabled = false);
        });
    }));
    let cs = light_color_scheme();
    let shapes = |rule: &KompozTestRule| {
        let canvas = rule.draw();
        canvas.rects.iter().copied().zip(canvas.rect_colors.iter().copied()).collect::<Vec<_>>()
    };
    // (SwitchDefaults': over the surface)
    let faded_over = |c: Color, a: f32| faded(c, a).composite_over(cs.surface);
    let has =
        |all: &[(Rect, Color)], w: f32, c: Color| all.iter().any(|(r, color)| near(r.width(), w, 0.01) && *color == c);
    let off = shapes(&rule);
    // Off: a surfaceContainerHighest track, a 16 dp outline thumb; disabled, faded over the surface.
    assert!(has(&off, 52.0, cs.surface_container_highest) && has(&off, 16.0, cs.outline), "{off:?}");
    assert!(
        has(&off, 52.0, faded_over(cs.on_surface, 0.12))
            && has(&off, 24.0, cs.surface)
            && has(&off, 52.0, faded_over(cs.surface_container_highest, 0.12))
            && has(&off, 16.0, faded_over(cs.on_surface, 0.38))
    );
    // Pressed: 28 dp at once.
    let at = bounds(&rule, "anahtar").center();
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.advance_frame();
    assert!(has(&shapes(&rule), 28.0, cs.outline), "{:?}", shapes(&rule));
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
    assert!(has(&shapes(&rule), 24.0, cs.on_primary) && has(&shapes(&rule), 52.0, cs.primary));
}

// ---- C++ switch_shows_an_icon_in_its_thumb_and_grows_it_while_held ---------------------------------------------

fn switch_shows_an_icon_in_its_thumb_and_grows_it_while_held() {
    let on = mutable_state_of(false);
    let icon_color = Rc::new(Cell::new(Color::Unspecified));
    let pressed = Rc::new(Cell::new(false));
    let source = MutableInteractionSource::new();
    let (o, ic, p, s) = (on.clone(), icon_color.clone(), pressed.clone(), source.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, ic, p, s) = (o.clone(), ic.clone(), p.clone(), s.clone());
        Column(move || {
            p.set(s.collect_is_pressed_as_state().get());
            let (o2, ic) = (o.clone(), ic.clone());
            Switch(
                o.get(),
                on_change(move |v| o2.set(v)),
                modifier = Modifier.test_tag("simgeli"),
                interaction_source = Some(s.clone()),
                thumb_content = composable(move || {
                    ic.set(LocalContentColor.current());
                    // (an icon's size, as Compose's sample sizes its Icon: content of no intrinsic size, the C++'s
                    // fillMaxSize Box, doesn't count as content to Compose's thumb)
                    Box(modifier = Modifier.size(SwitchDefaults::IconSize).test_tag("simge"));
                }),
            );
            Switch(false, on_change(|_| {}), modifier = Modifier.test_tag("düz"));
        });
    }));
    let cs = light_color_scheme();
    // The round square drawn in the switch, smaller than its track.
    let thumb_in = |rule: &KompozTestRule, area: Rect| {
        let c = rule.draw();
        let mut found = Rect::Zero;
        for (r, radius) in c.rects.iter().zip(&c.rect_radii) {
            if near(r.width(), r.height(), 0.01)
                && r.width() < 30.0
                && r.width() > 1.0
                && r.left >= area.left - 0.5
                && r.right <= area.right + 0.5
                && r.top >= area.top - 0.5
                && r.bottom <= area.bottom + 0.5
                && near(*radius, r.width() / 2.0, 0.01)
            {
                found = *r;
            }
        }
        found
    };
    // 52 x 32 in the middle of 48 dp of room to touch it. (Compose's semantics bounds: the switch's own, the room
    // around; C++'s were the room's.)
    let (with_icon, plain) = (bounds(&rule, "simgeli"), bounds(&rule, "düz"));
    assert!(
        same_rect(with_icon, xywh(0.0, 8.0, 52.0, 32.0)) && same_rect(plain, xywh(0.0, 56.0, 52.0, 32.0)),
        "{with_icon:?} {plain:?}"
    );
    assert!(same_rect(thumb_in(&rule, with_icon), xywh(4.0, 12.0, 24.0, 24.0))); // off, but an icon keeps it large
    assert!(same_rect(thumb_in(&rule, plain), xywh(8.0, 64.0, 16.0, 16.0))); // off and plain: small
    assert!(
        same_rect(rule.on_node_with_tag_unmerged("simge").get_unclipped_bounds_in_root(), xywh(8.0, 16.0, 16.0, 16.0)),
        "{:?}",
        rule.on_node_with_tag_unmerged("simge").get_unclipped_bounds_in_root()
    );
    assert_eq!(icon_color.get(), cs.surface_container_highest);
    tap(&rule, with_icon.center());
    rule.wait_for_idle();
    assert!(on.peek() && same_rect(thumb_in(&rule, with_icon), xywh(24.0, 12.0, 24.0, 24.0)));
    // (with the thumb)
    assert!(
        same_rect(rule.on_node_with_tag_unmerged("simge").get_unclipped_bounds_in_root(), xywh(28.0, 16.0, 16.0, 16.0))
            && icon_color.get() == cs.on_primary_container
    );
    rule.host().pointer_down(with_icon.center(), -1, 0, PointerType::Touch);
    rule.advance_time_by(300);
    // held: larger
    assert!(
        pressed.get() && same_rect(thumb_in(&rule, with_icon), xywh(22.0, 10.0, 28.0, 28.0)),
        "{:?}",
        thumb_in(&rule, with_icon)
    );
    rule.host().pointer_up(with_icon.center(), -1, 0);
    rule.wait_for_idle();
    assert!(!on.peek() && same_rect(thumb_in(&rule, with_icon), xywh(4.0, 12.0, 24.0, 24.0)));
}

// ---- C++ disabled_switch_and_checkbox_ignore_taps ---------------------------------------------------------------

fn disabled_switch_and_checkbox_ignore_taps() {
    let on = mutable_state_of(false);
    let o = on.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let o = o.clone();
        Column(move || {
            let (o1, o2) = (o.clone(), o.clone());
            Switch(o.get(), on_change(move |v| o1.set(v)), modifier = Modifier.test_tag("switch"), enabled = false);
            Checkbox(o.get(), on_change(move |v| o2.set(v)), modifier = Modifier.test_tag("box"), enabled = false);
        });
    }));
    rule.on_node_with_tag("switch").perform_click();
    rule.on_node_with_tag("box").perform_click();
    assert!(!on.peek());
    // (taps too, not only the semantics action)
    tap(&rule, bounds(&rule, "switch").center());
    tap(&rule, bounds(&rule, "box").center());
    rule.wait_for_idle();
    assert!(!on.peek());
    rule.on_node_with_tag("switch").assert_is_not_enabled();
    rule.on_node_with_tag("box").assert_is_not_enabled();
}

// ---- C++ checkboxes_and_radio_buttons_ripple_and_hover_in_a_40_dp_circle --------------------------------------

fn checkboxes_and_radio_buttons_ripple_and_hover_in_a_40_dp_circle() {
    let source = MutableInteractionSource::new();
    let radio_source = MutableInteractionSource::new();
    let hovered = Rc::new(Cell::new((false, false)));
    let (s, rs, h) = (source.clone(), radio_source.clone(), hovered.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, rs, h) = (s.clone(), rs.clone(), h.clone());
        Column(move || {
            h.set((s.collect_is_hovered_as_state().get(), rs.collect_is_hovered_as_state().get()));
            Checkbox(
                false,
                on_change(|_| {}),
                modifier = Modifier.test_tag("kutu"),
                interaction_source = Some(s.clone()),
            );
            RadioButton(
                false,
                on_click(|| {}),
                modifier = Modifier.test_tag("radyo"),
                interaction_source = Some(rs.clone()),
            );
        });
    }));
    let mut b = bounds(&rule, "radyo");
    // The round fills 40 dp wide centred on the box.
    let circles = |rule: &KompozTestRule, b: Rect| {
        let c = rule.draw();
        c.rects
            .iter()
            .zip(&c.rect_radii)
            .filter(|(r, radius)| {
                near(r.width(), 40.0, 0.01)
                    && near(**radius, 20.0, 0.01)
                    && near(r.center().x, b.center().x, 0.01)
                    && near(r.center().y, b.center().y, 0.01)
            })
            .count()
    };
    assert_eq!(circles(&rule, b), 0);
    mouse_to(&rule, b.center()); // the mouse over the radio button
    rule.wait_for_idle();
    assert!(hovered.get().1 && circles(&rule, b) == 1);
    b = bounds(&rule, "kutu");
    mouse_to(&rule, b.center()); // and over the checkbox
    rule.wait_for_idle();
    assert!(hovered.get().0 && circles(&rule, b) == 1);
    // The mouse pressed near the corner: the ripple still spreads from the middle, up to 40 dp (over the hover's
    // circle).
    let corner = Offset::new(b.left + 2.0, b.top + 2.0);
    rule.on_root().perform_mouse_input(|m| {
        m.move_to(corner);
        m.press();
    });
    rule.advance_time_by(1000);
    assert_eq!(circles(&rule, b), 2);
    rule.on_root().perform_mouse_input(|m| m.release());
    rule.wait_for_idle();
    // Focused from the keyboard: that circle at 10%, and no ring (Compose's Material 3 shows focus by its state
    // layer only).
    mouse_to(&rule, Offset::new(300.0, 300.0));
    rule.host().key_down(Key::Tab, KeyModifiers::None);
    rule.wait_for_idle();
    let focused = rule.draw();
    let layer = focused.rects.iter().zip(&focused.rect_radii).zip(&focused.rect_colors).any(|((r, radius), color)| {
        near(r.width(), 40.0, 0.01)
            && near(*radius, 20.0, 0.01)
            && near(color.alpha(), 0.1, 0.01)
            && near(r.center().x, b.center().x, 0.01)
    });
    // (besides the box's own outline)
    let ring = focused.strokes.iter().any(|r| near(r.width(), 40.0, 0.5));
    assert!(layer && !ring, "{:?} {:?}", focused.rects, focused.strokes);
}

// ---- C++ chips_radio_buttons_and_segments_select: its radio buttons ---------------------------------------------

fn chips_radio_buttons_and_segments_select() {
    // The C++ test's radio buttons (rows of a RadioButton and a Text, the button tapped). Its FilterChip is
    // tests/containers.rs' and its SingleChoiceSegmentedButtonRow the segmented buttons' (tests/segmented_buttons.rs, same
    // name).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let selected = remember(|| mutable_state_of("Kahve".to_string()));
        Column(move || {
            for option in ["Kahve", "Çay"] {
                let selected = selected.clone();
                Row(
                    modifier = Modifier.test_tag(option),
                    content = move || {
                        let s = selected.clone();
                        RadioButton(selected.get() == option, on_click(move || s.set(option.to_string())));
                        Text(option);
                    },
                );
            }
            Text(format!("seçim: {}", selected.get()));
        });
    }));
    rule.on_node_with_text("seçim: Kahve").assert_exists();
    // (the button, the row's first child)
    let tea = bounds(&rule, "Çay");
    tap(&rule, Offset::new(tea.left + 24.0, tea.top + tea.height() / 2.0));
    rule.wait_for_idle();
    rule.on_node_with_text("seçim: Çay").assert_exists();
}

// ---- C++ selectable_and_toggleable_rows_take_the_tap_for_their_controls ----------------------------------------

fn selectable_and_toggleable_rows_take_the_tap_for_their_controls() {
    // C++'s test with Material's controls (kompoz-foundation's clickable.rs has it with plain boxes): a control
    // without a callback takes no tap, the row around it does.
    let choice = mutable_state_of(0);
    let on = mutable_state_of(false);
    let tri = mutable_state_of(ToggleableState::Off);
    let (ch, o, t) = (choice.clone(), on.clone(), tri.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (ch, o, t) = (ch.clone(), o.clone(), t.clone());
        Column(
            modifier = Modifier.selectable_group().test_tag("grup"),
            content = move || {
                for i in 0..3 {
                    let ch = ch.clone();
                    let selected = ch.get() == i;
                    Row(
                        modifier = Modifier
                            .fill_max_width()
                            .height(56.dp())
                            .selectable(selected, role = Some(Role::RadioButton), on_click = move || ch.set(i))
                            .test_tag(format!("satır {i}")),
                        vertical_alignment = Alignment::CenterVertically,
                        content = move || {
                            RadioButton(selected, None, modifier = Modifier.test_tag(format!("düğme {i}")));
                            Text(format!("seçenek {i}"));
                        },
                    );
                }
                let o = o.clone();
                let value = o.get();
                Row(
                    modifier = Modifier
                        .fill_max_width()
                        .height(56.dp())
                        .toggleable(value, role = Some(Role::Switch), on_value_change = move |v| o.set(v))
                        .test_tag("anahtar satırı"),
                    vertical_alignment = Alignment::CenterVertically,
                    content = move || {
                        Text("Bildirimler");
                        Switch(value, None, modifier = Modifier.test_tag("anahtar"));
                    },
                );
                let t = t.clone();
                let state = t.get();
                let next = move || {
                    t.set(match t.peek() {
                        ToggleableState::Off => ToggleableState::Indeterminate,
                        ToggleableState::Indeterminate => ToggleableState::On,
                        ToggleableState::On => ToggleableState::Off,
                    })
                };
                Row(
                    modifier = Modifier
                        .fill_max_width()
                        .height(56.dp())
                        .tri_state_toggleable(state, role = Some(Role::Checkbox), on_click = next)
                        .test_tag("üç"),
                    content = move || TriStateCheckbox(state, None, modifier = Modifier.test_tag("kutu")),
                );
            },
        );
    }));
    rule.on_node_with_tag("satır 0").assert_is_selected();
    rule.on_node_with_tag_unmerged("düğme 2").perform_click(); // the control has no callback: the row takes the tap
    assert_eq!(choice.peek(), 2);
    rule.on_node_with_tag("satır 2").assert_is_selected();
    rule.on_node_with_tag("satır 0").assert_is_not_selected();
    rule.on_node_with_text("seçenek 1").perform_click();
    assert_eq!(choice.peek(), 1);
    rule.on_node_with_tag_unmerged("anahtar").perform_click();
    assert!(on.peek());
    rule.on_node_with_tag("anahtar satırı").assert_is_on();
    rule.on_node_with_tag_unmerged("kutu").perform_click();
    assert_eq!(tri.peek(), ToggleableState::Indeterminate);
    let s = rule.on_node_with_tag("üç").fetch_semantics_node().config();
    // (C++'s state description "Partially checked": Compose's toggleable state)
    assert!(
        s.get(SemanticsProperties::ToggleableState) == Some(ToggleableState::Indeterminate)
            && s.get(SemanticsProperties::Role) == Some(Role::Checkbox)
    );
    assert!(
        rule.on_node_with_tag("grup").fetch_semantics_node().config().contains(SemanticsProperties::SelectableGroup)
    );
    rule.on_node_with_tag_unmerged("düğme 0").assert(has_no_click_action()); // no callback, not clickable
}

// ---- Compose's behaviour ----------------------------------------------------------------------------------------

fn controls_tell_accessibility_their_role_and_state() {
    // Compose's: a Checkbox is a toggleable of the Checkbox role, a TriStateCheckbox tells its three states, a
    // RadioButton is a selectable of the RadioButton role, a Switch a toggleable of the Switch role; disabled ones
    // say so; one without a callback has no click and no role (a row around it tells them).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Checkbox(true, on_change(|_| {}), modifier = Modifier.test_tag("kutu"));
            TriStateCheckbox(ToggleableState::Indeterminate, on_click(|| {}), modifier = Modifier.test_tag("üç"));
            RadioButton(true, on_click(|| {}), modifier = Modifier.test_tag("radyo"));
            RadioButton(false, on_click(|| {}), modifier = Modifier.test_tag("boş radyo"), enabled = false);
            Switch(false, on_change(|_| {}), modifier = Modifier.test_tag("anahtar"));
            Checkbox(true, None, modifier = Modifier.test_tag("sade"));
        });
    }));
    rule.on_node_with_tag("kutu")
        .assert_is_toggleable()
        .assert_is_on()
        .assert(has_role(Role::Checkbox))
        .assert_has_click_action();
    let tri = rule.on_node_with_tag("üç").fetch_semantics_node().config();
    assert!(
        tri.get(SemanticsProperties::ToggleableState) == Some(ToggleableState::Indeterminate)
            && tri.get(SemanticsProperties::Role) == Some(Role::Checkbox)
    );
    rule.on_node_with_tag("radyo").assert_is_selectable().assert_is_selected().assert(has_role(Role::RadioButton));
    rule.on_node_with_tag("boş radyo").assert_is_not_selected().assert_is_not_enabled();
    rule.on_node_with_tag("anahtar").assert_is_toggleable().assert_is_off().assert(has_role(Role::Switch));
    let plain = rule.on_node_with_tag("sade").fetch_semantics_node().config();
    assert!(
        plain.get(SemanticsProperties::Role).is_none() && plain.get(SemanticsProperties::ToggleableState).is_none()
    );
    rule.on_node_with_tag("sade").assert_has_no_click_action();
}

fn controls_keep_48_dp_of_room_only_when_they_take_clicks() {
    // Compose's: a control taking clicks keeps the minimum interactive size (48 dp; LocalMinimumInteractiveComponentSize)
    // of room around itself, one without a callback only its own size (a checkbox or radio button 20 dp in 2 dp of
    // padding, a switch 52 x 32). Their semantics bounds are their own size either way (the toggleable's or
    // selectable's node is in the layout of what is inside the room, as Compose's): the room is the Box's around.
    let room = mutable_state_of(Dp(48.0));
    let r = room.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        KompozitionLocalProvider(&[LocalMinimumInteractiveComponentSize.provides(r.get())], || {
            Column(|| {
                Box(
                    modifier = Modifier.test_tag("kutu odası"),
                    content = || {
                        Checkbox(false, on_change(|_| {}), modifier = Modifier.test_tag("kutu"));
                    },
                );
                Box(
                    modifier = Modifier.test_tag("radyo odası"),
                    content = || {
                        RadioButton(false, on_click(|| {}), modifier = Modifier.test_tag("radyo"));
                    },
                );
                Box(
                    modifier = Modifier.test_tag("anahtar odası"),
                    content = || {
                        Switch(false, on_change(|_| {}), modifier = Modifier.test_tag("anahtar"));
                    },
                );
                Box(
                    modifier = Modifier.test_tag("sade kutu odası"),
                    content = || {
                        Checkbox(false, None);
                    },
                );
                Box(
                    modifier = Modifier.test_tag("sade radyo odası"),
                    content = || {
                        RadioButton(false, None);
                    },
                );
                Box(
                    modifier = Modifier.test_tag("sade anahtar odası"),
                    content = || {
                        Switch(false, None);
                    },
                );
            });
        });
    }));
    let size = |tag: &str| {
        let b = rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root();
        (b.width(), b.height())
    };
    assert!(size("kutu odası") == (48.0, 48.0) && size("radyo odası") == (48.0, 48.0));
    assert_eq!(size("anahtar odası"), (52.0, 48.0));
    assert!(size("kutu") == (24.0, 24.0) && size("radyo") == (24.0, 24.0) && size("anahtar") == (52.0, 32.0));
    // (centred in their room)
    assert!(same_rect(bounds(&rule, "kutu"), xywh(12.0, 12.0, 24.0, 24.0)), "{:?}", bounds(&rule, "kutu"));
    assert!(size("sade kutu odası") == (24.0, 24.0) && size("sade radyo odası") == (24.0, 24.0));
    assert_eq!(size("sade anahtar odası"), (52.0, 32.0));
    // Where a touch hits them (Compose's ui-test `touchBoundsInRoot`): those taking clicks their 48 dp room, those
    // without their own size.
    rule.on_node_with_tag("kutu").assert_touch_width_is_equal_to(Dp(48.0)).assert_touch_height_is_equal_to(Dp(48.0));
    assert!(same_rect(rule.on_node_with_tag("kutu").get_touch_bounds_in_root(), xywh(0.0, 0.0, 48.0, 48.0)));
    rule.on_node_with_tag("anahtar").assert_touch_width_is_equal_to(Dp(52.0)).assert_touch_height_is_equal_to(Dp(48.0));
    let plain = rule.on_node_with_tag_unmerged("sade kutu odası");
    assert!(same_rect(plain.get_touch_bounds_in_root(), plain.get_unclipped_bounds_in_root()));
    // No minimum provided: their own sizes; still touched in 48 dp (Compose's CheckboxTest
    // `checkBoxTest_MaterialSize_WhenEnforcementDisabled`: the view configuration's minimum touch target).
    room.set(Dp(0.0));
    rule.wait_for_idle();
    assert!(size("kutu odası") == (24.0, 24.0) && size("anahtar odası") == (52.0, 32.0));
    rule.on_node_with_tag("kutu").assert_width_is_equal_to(Dp(24.0)).assert_touch_width_is_equal_to(Dp(48.0));
    rule.on_node_with_tag("kutu").assert_touch_height_is_equal_to(Dp(48.0));
}

fn control_colors_come_from_the_scheme_and_copy_what_is_given() {
    // Compose's XDefaults.colors(): the scheme's colors by the tokens; colors(…) puts each given color in its
    // places; copy keeps what is unspecified; the deprecated CheckboxColors constructor's disabled checkmark is its
    // checked one.
    let seen = Rc::new(std::cell::RefCell::new(None));
    let s = seen.clone();
    let scheme = named_args!(dark_color_scheme(primary = Color::Red));
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            color_scheme = scheme,
            content = move || {
                let checkbox = CheckboxDefaults.colors();
                let checked_red =
                    named_args!(CheckboxDefaults.colors(checked_color = Color::Blue, checkmark_color = Color::Green));
                let parts = named_args!(CheckboxDefaults.colors_of_parts(unchecked_box_color = Color::Yellow));
                let radio = RadioButtonDefaults.colors();
                let radio_given = named_args!(RadioButtonDefaults.colors(unselected_color = Color::Blue));
                let switch = SwitchDefaults.colors();
                let switch_given = named_args!(SwitchDefaults.colors(checked_track_color = Color::Blue));
                *s.borrow_mut() = Some((checkbox, checked_red, parts, radio, radio_given, switch, switch_given));
                Checkbox(true, on_change(|_| {}), colors = checked_red, modifier = Modifier.test_tag("kutu"));
            },
        );
    }));
    let (checkbox, checked_red, parts, radio, radio_given, switch, switch_given) = seen.borrow().unwrap();
    let cs = scheme;
    assert!(checkbox.checked_box_color == Color::Red && checkbox.checked_checkmark_color == cs.on_primary);
    assert!(
        checkbox.unchecked_border_color == cs.on_surface_variant && checkbox.unchecked_box_color == Color::Transparent
    );
    assert_eq!(checkbox.disabled_checked_box_color, faded(cs.on_surface, 0.38));
    assert_eq!(checkbox.disabled_checkmark_color, cs.surface);
    assert!(checked_red.checked_box_color == Color::Blue && checked_red.checked_border_color == Color::Blue);
    assert!(
        checked_red.checked_checkmark_color == Color::Green && checked_red.disabled_checkmark_color == Color::Green
    );
    assert_eq!(checked_red.unchecked_border_color, cs.on_surface_variant);
    assert!(parts.unchecked_box_color == Color::Yellow && parts.checked_box_color == Color::Red);
    assert_eq!(named_args!(parts.copy(checked_box_color = Color::Unspecified)), parts);
    assert_eq!(named_args!(parts.copy(checked_box_color = Color::Blue)).checked_box_color, Color::Blue);
    #[allow(deprecated)]
    let old = CheckboxColorsWithoutDisabledCheckmark(
        Color::Red,
        Color::Transparent,
        Color::Red,
        Color::Transparent,
        Color::Gray,
        Color::Transparent,
        Color::Gray,
        Color::Red,
        Color::Black,
        Color::Gray,
        Color::Gray,
        Color::Gray,
    );
    assert_eq!(old.disabled_checkmark_color, Color::Red);
    assert!(radio.selected_color == Color::Red && radio.unselected_color == cs.on_surface_variant);
    assert_eq!(radio.disabled_selected_color, faded(cs.on_surface, 0.38));
    assert!(radio_given.unselected_color == Color::Blue && radio_given.selected_color == Color::Red);
    assert!(switch.checked_track_color == Color::Red && switch.checked_thumb_color == cs.on_primary);
    assert!(switch.unchecked_border_color == cs.outline && switch.checked_border_color == Color::Transparent);
    assert_eq!(
        switch.disabled_unchecked_track_color,
        faded(cs.surface_container_highest, 0.12).composite_over(cs.surface)
    );
    assert!(switch_given.checked_track_color == Color::Blue && switch_given.unchecked_thumb_color == cs.outline);
    assert_eq!(named_args!(switch.copy(checked_icon_color = Color::Green)).checked_icon_color, Color::Green);
    // The given colors drawn: the box and its border blue, the tick green.
    rule.wait_for_idle();
    let canvas = rule.draw();
    assert!(canvas.rect_colors.contains(&Color::Blue) && canvas.path_colors.contains(&Color::Green));
}

// ---- C++ controls_keep_48_dp_of_room_and_take_taps_anywhere_in_it ---------------------------------------------

type Log = Rc<std::cell::RefCell<Vec<&'static str>>>;

/// What `source` tells, named (presses only), into `log`.
#[kompozable]
fn LogPresses(source: MutableInteractionSource, log: Log) {
    let flow = source.interactions();
    kompoz_coroutines::LaunchedEffect(source.clone(), async move {
        flow.collect(move |i: std::sync::Arc<dyn Interaction>| {
            let name = if i.is::<PressInteraction::Press>() {
                "Press"
            } else if i.is::<PressInteraction::Release>() {
                "Release"
            } else if i.is::<PressInteraction::Cancel>() {
                "Cancel"
            } else {
                return;
            };
            log.borrow_mut().push(name);
        })
        .await;
    });
}

/// `content` in a Box tagged `tag` (the room a control keeps is the Box's size: the control's own node is its
/// clickable's, inside the room, as Compose's).
#[kompozable]
fn Room(tag: String, content: impl Fn() + 'static) {
    Box(modifier = Modifier.test_tag(tag), content = content);
}

fn controls_keep_48_dp_of_room_and_take_taps_anywhere_in_it() {
    // Compose's: a control taking clicks keeps 48 dp of room (LocalMinimumInteractiveComponentSize) and the
    // touches in it are its (the room's node hands them to the control inside; a press let go within the
    // minimum touch target still clicks); without a callback only its own size. (Their own nodes are their own
    // size: the rooms are measured on Boxes around them.)
    let clicks = Rc::new(Cell::new(0));
    let (checked, selected, on) = (mutable_state_of(false), mutable_state_of(false), mutable_state_of(false));
    let room = mutable_state_of(Dp(48.0));
    let log: Log = Rc::default();
    let (c, ch, se, o, r, l) =
        (clicks.clone(), checked.clone(), selected.clone(), on.clone(), room.clone(), log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let button_source = remember(MutableInteractionSource::new);
        LogPresses(button_source.clone(), l.clone());
        let (c, ch, se, o) = (c.clone(), ch.clone(), se.clone(), o.clone());
        KompozitionLocalProvider(&[LocalMinimumInteractiveComponentSize.provides(r.get())], move || {
            let (c, ch, se, o, button_source) = (c.clone(), ch.clone(), se.clone(), o.clone(), button_source.clone());
            Column(move || {
                let (c, ch, se, o, button_source) =
                    (c.clone(), ch.clone(), se.clone(), o.clone(), button_source.clone());
                Room("düğme".into(), move || {
                    let c = c.clone();
                    IconButton(
                        move || c.set(c.get() + 1),
                        interaction_source = Some(button_source.clone()),
                        content = || VectorIcon(Icons::Filled::Add(), Some("Ekle".to_string())),
                    );
                });
                Room("kutu".into(), move || {
                    let ch2 = ch.clone();
                    Checkbox(ch.get(), on_change(move |v| ch2.set(v)));
                });
                Room("radyo".into(), move || {
                    let se2 = se.clone();
                    RadioButton(se.get(), on_click(move || se2.set(!se2.peek())));
                });
                Room("anahtar".into(), move || {
                    let o2 = o.clone();
                    Switch(o.get(), on_change(move |v| o2.set(v)));
                });
                // Not taking taps themselves (a row around them would): just their own size.
                Room("sade kutu".into(), || Checkbox(false, None));
                Room("sade radyo".into(), || RadioButton(false, None));
                Room("sade anahtar".into(), || Switch(false, None));
            });
        });
    }));
    let size = |tag: &str| {
        let b = rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root();
        (b.width(), b.height())
    };
    assert_eq!(size("düğme"), (48.0, 48.0));
    assert!(size("kutu") == (48.0, 48.0) && size("radyo") == (48.0, 48.0));
    assert_eq!(size("anahtar"), (52.0, 48.0));
    assert!(size("sade kutu") == (24.0, 24.0) && size("sade radyo") == (24.0, 24.0));
    assert_eq!(size("sade anahtar").1, 32.0);
    // A tap 2 dp from the room's corner, well off the 40 dp circle (or the 24 dp box), still lands.
    let near_corner = |tag: &str| {
        let b = rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root();
        Offset::new(b.left + 2.0, b.top + 2.0)
    };
    for tag in ["düğme", "kutu", "radyo", "anahtar"] {
        tap(&rule, near_corner(tag));
        rule.wait_for_idle();
    }
    assert!(clicks.get() == 1 && checked.peek() && selected.peek() && on.peek());
    // So does a press in the middle let go near the room's edge (a release); one moved past the room doesn't click
    // (a cancel).
    let b = rule.on_node_with_tag_unmerged("düğme").get_unclipped_bounds_in_root();
    log.borrow_mut().clear();
    rule.host().pointer_down(b.center(), -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(Offset::new(b.right - 2.0, b.center().y), -1, 0);
    rule.wait_for_idle();
    assert!(clicks.get() == 2 && *log.borrow() == ["Press", "Release"], "{:?}", log.borrow());
    // (C++ let go past it at once: Compose's tap takes an up as the release wherever it is — moved there first,
    // it is cancelled)
    rule.host().pointer_down(b.center(), -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_move(Offset::new(b.right + 6.0, b.center().y), -1, 0);
    rule.advance_frame();
    rule.host().pointer_up(Offset::new(b.right + 6.0, b.center().y), -1, 0);
    rule.wait_for_idle();
    assert!(clicks.get() == 2 && *log.borrow() == ["Press", "Release", "Press", "Cancel"], "{:?}", log.borrow());
    // With no room asked for (0), they are as big as they look.
    room.set(Dp(0.0));
    rule.wait_for_idle();
    assert!(size("düğme").0 == 40.0 && size("kutu").0 == 24.0 && size("anahtar").1 == 32.0);
    tap(&rule, near_corner("kutu")); // the 24 dp box's own corner
    rule.wait_for_idle();
    assert!(!checked.peek());
}

// ---- C++ rtl_mirrors_arrows_switches_and_sliders --------------------------------------------------------------

fn rtl_mirrors_arrows_switches_and_sliders() {
    // M15's slider thumbs are 4 dp bars (2 dp held): its track starts 2 dp in (C++: 10). A range slider picks the thumb nearer the touch by where they last were (no drag-way pick
    // of two together, the C++'s): the second drag is the start thumb's from apart ends.
    let rtl = mutable_state_of(true);
    let value = mutable_state_of(0.0f32);
    let range = mutable_state_of(20.0f32..=80.0);
    let (rt, v, r) = (rtl.clone(), value.clone(), range.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, r) = (v.clone(), r.clone());
        let direction = if rt.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
            let (v, r) = (v.clone(), r.clone());
            Column(
                modifier = Modifier.width(220.dp()),
                content = move || {
                    Row(|| {
                        VectorIcon(Icons::AutoMirrored::Filled::ArrowBack(), Some("Geri".to_string()));
                        VectorIcon(Icons::Filled::Star(), Some("Yıldız".to_string()));
                    });
                    Switch(true, on_change(|_| {}), modifier = Modifier.test_tag("açık"));
                    Switch(false, on_change(|_| {}), modifier = Modifier.test_tag("kapalı"));
                    let v2 = v.clone();
                    Slider(
                        v.get(),
                        move |x| v2.set(x),
                        modifier = Modifier.width(220.dp()).test_tag("kaydırıcı"),
                        value_range = 0.0..=100.0,
                    );
                    let r2 = r.clone();
                    RangeSlider(
                        r.get(),
                        move |x| r2.set(x),
                        modifier = Modifier.width(220.dp()).test_tag("aralık"),
                        value_range = 0.0..=100.0,
                    );
                },
            );
        });
    }));
    // The arrow is drawn mirrored (about its middle); a star is a star. The arrow that mirrors is a vector of its
    // own, as Compose's Icons.AutoMirrored are.
    let back = Icons::AutoMirrored::Filled::ArrowBack();
    #[allow(deprecated)]
    let plain_back = Icons::Filled::ArrowBack();
    assert!(back.auto_mirror() && !plain_back.auto_mirror() && back.name() == "AutoMirrored.Filled.ArrowBack");
    assert!(plain_back.name() == "Filled.ArrowBack" && Icons::Filled::Star().name() == "Filled.Star");
    let mirrored_at = |c: &kompoz_test::RecordingCanvas| -> Vec<f32> {
        c.transforms
            .iter()
            .filter(|m| near(m.values[Matrix::ScaleX], -1.0, 0.001))
            .map(|m| m.map(Offset::new(0.0, 0.0)).x / 2.0)
            .collect()
    };
    let canvas = rule.draw();
    assert_eq!(canvas.vectors, [back.id(), Icons::Filled::Star().id()]);
    let mirrored = mirrored_at(&canvas);
    // (the mirroring is in the icon's own space: about its 24 dp middle; the arrow first at the right)
    let arrow = canvas.vector_bounds[0];
    assert!(mirrored.len() == 1 && near(mirrored[0], arrow.width() / 2.0, 0.01), "{mirrored:?}");
    assert!(near(arrow.right, 220.0, 0.01) && near(arrow.width(), 24.0, 0.01), "{arrow:?}");
    // A switch that is on has its thumb at the end: the left.
    let thumb_x = |tag: &str| {
        let b = bounds(&rule, tag);
        let mut x = -1.0;
        for r in &canvas.rects {
            if near(r.width(), r.height(), 0.01)
                && r.width() < 30.0
                && r.left >= b.left
                && r.right <= b.right
                && r.top >= b.top
                && r.bottom <= b.bottom
            {
                x = r.center().x - b.left;
            }
        }
        x
    };
    assert!(thumb_x("açık") > 0.0 && thumb_x("açık") < 26.0 && thumb_x("kapalı") > 26.0);
    // The slider's track starts at the right: a tap a quarter in from the right is 25.
    let slider = layout_bounds(&rule, "kaydırıcı");
    // (a tap at once: the thumb not yet thin, its track 2 dp in)
    tap(&rule, Offset::new(slider.right - 2.0 - 216.0 * 0.25, slider.top + 24.0));
    rule.wait_for_idle();
    assert!(near(value.peek(), 25.0, 0.01), "{}", value.peek());
    let cs = light_color_scheme();
    let primary_stretches = |c: &kompoz_test::RecordingCanvas, b: Rect| -> Vec<Rect> {
        c.paths
            .iter()
            .zip(&c.path_colors)
            .filter(|(p, col)| **col == cs.primary && near(p.height(), 16.0, 0.01) && near(p.top, b.top + 16.0, 0.01))
            .map(|(p, _)| *p)
            .collect()
    };
    // (its active stretch: from 2 dp in at the right to 8 dp short of the thumb, 2 + 54 dp from the right)
    let slid = primary_stretches(&rule.draw(), slider);
    assert!(
        slid.len() == 1
            && near(slid[0].right, slider.right - 2.0, 0.01)
            && near(slid[0].left, slider.right - 48.0, 0.01),
        "{slid:?}"
    );
    // The range's start thumb is at the right: a tap near it moves it (its thumbs not held: x = right - 2 - 2.16v).
    let rb = layout_bounds(&rule, "aralık");
    let rx = |v: f32| rb.right - 2.0 - 2.16 * v;
    tap(&rule, Offset::new(rx(10.0), rb.top + 24.0));
    rule.wait_for_idle();
    let got = range.peek();
    assert!(near(*got.start(), 10.0, 0.5) && near(*got.end(), 80.0, 0.01), "{got:?}");
    // The active stretch between 80 (left) and 10 (right), as far from each thumb.
    let between = primary_stretches(&rule.draw(), rb);
    assert!(between.len() == 1 && near(between[0].center().x, (rx(*got.start()) + rx(80.0)) / 2.0, 0.5), "{between:?}");
    // A drag of the start thumb to the right: less.
    range.set(40.0..=60.0);
    rule.wait_for_idle();
    swipe(&rule, Offset::new(rx(40.0), rb.top + 24.0), Offset::new(rx(20.0), rb.top + 24.0));
    rule.wait_for_idle();
    let got = range.peek();
    assert!(near(*got.start(), 20.0, 0.5) && near(*got.end(), 60.0, 0.01), "{got:?}");
    // Left to right again: the arrow as it is.
    rtl.set(false);
    rule.wait_for_idle();
    let ltr = rule.draw();
    assert!(ltr.vectors.first() == Some(&back.id()) && mirrored_at(&ltr).is_empty());
}

/// The tagged layout's bounds (a slider's semantics node is 10 dp wider each side, for a screen reader).
fn layout_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    let c = rule.on_node_with_tag_unmerged(tag).layout_node().coordinates();
    let (p, size) = (c.position_in_root(), c.size());
    Rect::new(p.x, p.y, p.x + size.width as f32, p.y + size.height as f32)
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

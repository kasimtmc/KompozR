//! KompozR's title bar (C++'s title_bar.cpp) in a test rule: the window's frame (the bar 40 dp across the top, the
//! content in what it leaves; none without a bar, the content kept as the bar comes and goes), the buttons where the
//! desktop's layout puts them (mirrored right to left), what the bar asks of the window (its buttons; a drag moves
//! it, a double click maximizes it, the secondary button opens the menu; a click alone nothing), the buttons'
//! labels and roles in the app's language, and its colors from the system's theme and palette.

#![allow(non_snake_case)]

use kompoz_foundation::prelude::*;
use kompoz_graphics::material_color::palettes::{DynamicTonalPalette, palette_tone};
use kompoz_graphics::*;
use kompoz_platform::{
    TitleBar, TitleBarColors, WindowAction, WindowActions, WindowButton, WindowFrame, button_action, button_icon,
};
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::Modifier;
use kompoz_ui::input::PointerButton;
use kompoz_ui::semantics::Role;
use kompoz_ui::strings::Strings;
use std::cell::RefCell;
use std::rc::Rc;

type Acts = Rc<RefCell<Vec<(WindowAction, Offset)>>>;

/// GNOME's layout (Close alone, on the right) and KDE's (Minimize, Maximize, Close on the right; here Close on the
/// left too, to see both sides).
fn both_sides(maximized: bool) -> TitleBar {
    TitleBar {
        start: vec![WindowButton::Close],
        end: vec![WindowButton::Minimize, WindowButton::Maximize, WindowButton::Close],
        maximized,
    }
}

/// A window's frame around a content of a tag of its own (with a count its clicks add to), the bar as `bar` says;
/// what the bar asks, logged.
fn framed(bar: MutableState<Option<TitleBar>>, acts: &Acts) -> KompozTestRule {
    let log = acts.clone();
    let act: WindowActions = Rc::new(move |action, at| log.borrow_mut().push((action, at)));
    let content: Rc<dyn Content> = Rc::new(kompozable_lambda!(|| {
        let count = remember(|| mutable_state_of(0));
        let shown = count.clone();
        Box(
            modifier =
                Modifier.fill_max_size().test_tag("content").clickable(on_click = move || count.set(count.get() + 1)),
            content = move || BasicText(format!("count {}", shown.get())),
        );
    }));
    KompozTestRule::new(kompozable_lambda!(move || {
        WindowFrame(bar.get(), "Sayaç".to_string(), act.clone(), content.clone());
    }))
}

fn taken(acts: &Acts) -> Vec<(WindowAction, Offset)> {
    std::mem::take(&mut *acts.borrow_mut())
}

fn bounds(node: SemanticsNodeInteraction<'_>) -> Rect {
    node.get_bounds_in_root()
}

fn button<'a>(rule: &'a KompozTestRule, label: &str) -> SemanticsNodeInteraction<'a> {
    rule.on_node_with(has_content_description(label, false, false).and(has_role(Role::Button)), false)
}

fn rect(left: f32, top: f32, width: f32, height: f32) -> Rect {
    Rect::new(left, top, left + width, top + height)
}

// ---------------------------------------------------------------------------------------------------------------

fn the_bar_is_above_the_content() {
    let acts = Acts::default();
    let bar = mutable_state_of(Some(both_sides(false)));
    let rule = framed(bar.clone(), &acts);
    // 40 dp across the top; the content in the rest, from below it.
    assert_eq!(bounds(rule.on_node_with_tag("content")), rect(0.0, 40.0, 400.0, 760.0));
    // The title in the middle, in the bar.
    let title = bounds(rule.on_node_with_text("Sayaç"));
    assert!(title.top >= 0.0 && title.bottom <= 40.0, "{title:?}");
    assert!((title.center().x - 200.0).abs() <= 0.5, "{title:?}");
    // The content's count, kept as the bar goes (the whole window then) and comes back.
    rule.on_node_with_tag("content").perform_click();
    rule.on_node_with_text("count 1").assert_exists();
    bar.set(None);
    rule.wait_for_idle();
    assert_eq!(bounds(rule.on_node_with_tag("content")), rect(0.0, 0.0, 400.0, 800.0));
    rule.on_node_with_text("Sayaç").assert_does_not_exist();
    rule.on_node_with_text("count 1").assert_exists();
    bar.set(Some(both_sides(false)));
    rule.wait_for_idle();
    rule.on_node_with_text("count 1").assert_exists();
    assert_eq!(bounds(rule.on_node_with_tag("content")), rect(0.0, 40.0, 400.0, 760.0));
    // At 2 px a dp: 80 px.
    rule.host().set_density(Density::new(2.0, 1.0));
    rule.wait_for_idle();
    assert_eq!(bounds(rule.on_node_with_tag("content")), rect(0.0, 80.0, 400.0, 720.0));
    assert_eq!(bounds(button(&rule, "Minimize")), rect(400.0 - 3.0 * 72.0, 12.0, 56.0, 56.0));
    assert!(taken(&acts).is_empty());
}

fn the_buttons_are_where_the_desktop_puts_them() {
    let acts = Acts::default();
    let rule = framed(mutable_state_of(Some(both_sides(false))), &acts);
    // 28 dp circles 8 dp apart, 8 dp from the sides, in the middle of the bar's height.
    assert_eq!(bounds(button(&rule, "Minimize")), rect(292.0, 6.0, 28.0, 28.0));
    assert_eq!(bounds(button(&rule, "Maximize")), rect(328.0, 6.0, 28.0, 28.0));
    let closes = rule.on_all_nodes_with(has_content_description("Close", false, false), false).fetch_semantics_nodes();
    let mut lefts: Vec<f32> = closes.iter().map(|n| n.bounds_in_root().left).collect();
    lefts.sort_by(f32::total_cmp);
    assert_eq!(lefts, [8.0, 364.0]);
    // GNOME's: Close alone on the right; the title clear of it on both sides.
    let rule = framed(
        mutable_state_of(Some(TitleBar { start: vec![], end: vec![WindowButton::Close], maximized: false })),
        &acts,
    );
    assert_eq!(bounds(button(&rule, "Close")), rect(364.0, 6.0, 28.0, 28.0));
    button(&rule, "Minimize").assert_does_not_exist();
    button(&rule, "Maximize").assert_does_not_exist();
    // Maximized: Maximize is Restore.
    let rule = framed(mutable_state_of(Some(both_sides(true))), &acts);
    button(&rule, "Maximize").assert_does_not_exist();
    assert_eq!(bounds(button(&rule, "Restore")), rect(328.0, 6.0, 28.0, 28.0));
    assert_eq!(button_icon(WindowButton::Maximize, true).name(), "Filled.FilterNone");
    assert_eq!(button_icon(WindowButton::Maximize, false).name(), "Filled.CropSquare");
    assert_eq!(button_icon(WindowButton::Minimize, false).name(), "Filled.Minimize");
    assert_eq!(button_icon(WindowButton::Close, true).name(), "Filled.Close");
    assert!(taken(&acts).is_empty());
}

fn right_to_left_the_sides_are_mirrored() {
    let acts = Acts::default();
    let rule = framed(mutable_state_of(Some(both_sides(false))), &acts);
    rule.host().set_layout_direction(LayoutDirection::Rtl);
    rule.wait_for_idle();
    // The end on the left, in order from the right; the start on the right.
    assert_eq!(bounds(button(&rule, "Minimize")), rect(80.0, 6.0, 28.0, 28.0));
    assert_eq!(bounds(button(&rule, "Maximize")), rect(44.0, 6.0, 28.0, 28.0));
    let closes = rule.on_all_nodes_with(has_content_description("Close", false, false), false).fetch_semantics_nodes();
    let mut lefts: Vec<f32> = closes.iter().map(|n| n.bounds_in_root().left).collect();
    lefts.sort_by(f32::total_cmp);
    assert_eq!(lefts, [8.0, 364.0]);
    let title = bounds(rule.on_node_with_text("Sayaç"));
    assert!((title.center().x - 200.0).abs() <= 0.5, "{title:?}");
    assert_eq!(bounds(rule.on_node_with_tag("content")), rect(0.0, 40.0, 400.0, 760.0));
}

fn the_buttons_and_gestures_ask_the_window() {
    let acts = Acts::default();
    let rule = framed(mutable_state_of(Some(both_sides(false))), &acts);
    // The buttons: each its action, nothing else.
    button(&rule, "Minimize").perform_click();
    rule.wait_for_idle();
    assert_eq!(taken(&acts), [(WindowAction::Minimize, Offset::Zero)]);
    button(&rule, "Maximize").perform_click();
    rule.wait_for_idle();
    assert_eq!(taken(&acts), [(WindowAction::ToggleMaximize, Offset::Zero)]);
    rule.on_root().perform_mouse_input(|m| m.click_with(Offset::new(378.0, 20.0), PointerButton::Primary));
    rule.wait_for_idle();
    assert_eq!(taken(&acts), [(WindowAction::Close, Offset::Zero)]);
    for b in [WindowButton::Minimize, WindowButton::Maximize, WindowButton::Close] {
        assert_eq!(
            button_action(b),
            [WindowAction::Minimize, WindowAction::ToggleMaximize, WindowAction::Close][b as usize]
        );
    }
    // A click on the bar alone: nothing (once the double click's time is over).
    rule.on_root().perform_mouse_input(|m| m.click_with(Offset::new(150.0, 20.0), PointerButton::Primary));
    rule.main_clock().advance_time_by(500);
    rule.wait_for_idle();
    assert!(taken(&acts).is_empty());
    // A double click (the mouse's, a finger's): maximized, or restored.
    rule.on_root().perform_mouse_input(|m| m.double_click_with(Offset::new(150.0, 20.0), PointerButton::Primary));
    rule.wait_for_idle();
    assert_eq!(taken(&acts), [(WindowAction::ToggleMaximize, Offset::Zero)]);
    rule.main_clock().advance_time_by(500);
    rule.on_root().perform_touch_input(|t| t.double_click_with(Offset::new(120.0, 30.0), 100));
    rule.wait_for_idle();
    assert_eq!(taken(&acts), [(WindowAction::ToggleMaximize, Offset::Zero)]);
    rule.main_clock().advance_time_by(500);
    // A drag (the mouse's, a finger's): moved, from where it passed the slop.
    rule.on_root().perform_mouse_input(|m| {
        m.move_to(Offset::new(150.0, 20.0));
        m.press_button(PointerButton::Primary);
        m.move_to(Offset::new(190.0, 25.0));
        m.move_to(Offset::new(230.0, 25.0));
        m.release_button(PointerButton::Primary);
    });
    rule.wait_for_idle();
    assert_eq!(taken(&acts), [(WindowAction::Move, Offset::new(190.0, 25.0))]);
    rule.main_clock().advance_time_by(500);
    rule.on_root().perform_touch_input(|t| {
        t.down(Offset::new(100.0, 10.0));
        t.move_to(Offset::new(100.0, 35.0));
        t.up();
    });
    rule.wait_for_idle();
    assert_eq!(taken(&acts), [(WindowAction::Move, Offset::new(100.0, 35.0))]);
    rule.main_clock().advance_time_by(500);
    // The secondary button: the menu where it was pressed (and no double click of it).
    rule.on_root().perform_mouse_input(|m| m.click_with(Offset::new(140.0, 30.0), PointerButton::Secondary));
    rule.on_root().perform_mouse_input(|m| m.click_with(Offset::new(140.0, 30.0), PointerButton::Secondary));
    rule.wait_for_idle();
    assert_eq!(taken(&acts), [(WindowAction::Menu, Offset::new(140.0, 30.0)); 2]);
    rule.main_clock().advance_time_by(500);
    // Below the bar: the content's, not the window's.
    rule.on_root().perform_mouse_input(|m| m.double_click_with(Offset::new(150.0, 200.0), PointerButton::Primary));
    rule.on_root().perform_mouse_input(|m| m.click_with(Offset::new(150.0, 200.0), PointerButton::Secondary));
    rule.main_clock().advance_time_by(500);
    rule.wait_for_idle();
    assert!(taken(&acts).is_empty());
}

fn the_buttons_are_labelled_in_the_apps_language() {
    let acts = Acts::default();
    let rule = framed(mutable_state_of(Some(both_sides(false))), &acts);
    for label in ["Minimize", "Maximize"] {
        button(&rule, label).assert_has_click_action();
    }
    rule.on_all_nodes_with(has_content_description("Close", false, false).and(has_role(Role::Button)), false)
        .assert_count_equals(2);
    rule.host().set_strings(Strings::turkish());
    rule.wait_for_idle();
    button(&rule, "Simge durumuna küçült").assert_exists();
    button(&rule, "Ekranı kapla").assert_exists();
    rule.on_all_nodes_with(has_content_description("Kapat", false, false).and(has_role(Role::Button)), false)
        .assert_count_equals(2);
    button(&rule, "Minimize").assert_does_not_exist();
    let rule = framed(mutable_state_of(Some(both_sides(true))), &acts);
    rule.host().set_strings(Strings::turkish());
    rule.wait_for_idle();
    button(&rule, "Önceki boyutuna getir").assert_exists();
    assert!(taken(&acts).is_empty());
}

/// The colors drawn: the bar's (the rect across it), the buttons' (their circles), the icons', the title's.
fn drawn_colors(rule: &KompozTestRule) -> (Color, Vec<Color>, Vec<Color>, Color) {
    let d = rule.draw();
    let bar = d.rects.iter().zip(&d.rect_colors).find(|(r, _)| **r == rect(0.0, 0.0, 400.0, 40.0)).map(|(_, c)| *c);
    let circles: Vec<Color> = d
        .rects
        .iter()
        .zip(&d.rect_colors)
        .filter(|(r, _)| r.width() == 28.0 && r.height() == 28.0)
        .map(|(_, c)| *c)
        .collect();
    let title = d.texts.iter().zip(&d.colors).find(|(t, _)| t.as_str() == "Sayaç").map(|(_, c)| *c);
    (bar.expect("the bar's rect"), circles, d.vector_colors.clone(), title.expect("the title"))
}

fn the_colors_are_the_systems() {
    // Material 3's baseline schemes without a palette (C++'s lightColorScheme and darkColorScheme).
    let light = TitleBarColors::of(false, None);
    assert_eq!(light.surface_container, Color::from_argb(0xFFF3EDF7));
    assert_eq!(light.surface_container_highest, Color::from_argb(0xFFE6E0E9));
    assert_eq!(light.on_surface, Color::from_argb(0xFF1D1B20));
    assert_eq!(light.on_surface_variant, Color::from_argb(0xFF49454F));
    let dark = TitleBarColors::of(true, None);
    assert_eq!(dark.surface_container, Color::from_argb(0xFF211F26));
    assert_eq!(dark.surface_container_highest, Color::from_argb(0xFF36343B));
    assert_eq!(dark.on_surface, Color::from_argb(0xFFE6E0E9));
    assert_eq!(dark.on_surface_variant, Color::from_argb(0xFFCAC4D0));
    // From a seed (the desktop's accent): the tones Compose's dynamic schemes give the roles.
    let palette = DynamicTonalPalette::from_seed(Color::from_argb(0xff3f51b5));
    let tone = |key: &[Color; 13], t: f32| palette_tone(key, t);
    let seeded_light = TitleBarColors::of(false, Some(&palette));
    assert_eq!(seeded_light.surface_container, tone(&palette.neutral1, 94.0));
    assert_eq!(seeded_light.surface_container_highest, tone(&palette.neutral1, 90.0));
    assert_eq!(seeded_light.surface_container_highest, palette.neutral1[3]); // (a tone the system gives)
    assert_eq!(seeded_light.on_surface, palette.neutral1[11]);
    assert_eq!(seeded_light.on_surface_variant, palette.neutral2[9]);
    let seeded_dark = TitleBarColors::of(true, Some(&palette));
    assert_eq!(seeded_dark.surface_container, tone(&palette.neutral1, 12.0));
    assert_eq!(seeded_dark.surface_container_highest, tone(&palette.neutral1, 22.0));
    assert_eq!(seeded_dark.on_surface, palette.neutral1[3]);
    assert_eq!(seeded_dark.on_surface_variant, palette.neutral2[4]);
    assert_ne!(seeded_light.surface_container, light.surface_container);
    assert!(seeded_dark.surface_container.luminance() < 0.05 && seeded_light.surface_container.luminance() > 0.8);
    // Drawn as the system's theme and palette say, the title dimmed while the window isn't focused.
    let acts = Acts::default();
    let rule = framed(mutable_state_of(Some(both_sides(false))), &acts);
    let check = |c: TitleBarColors, title: Color| {
        let (bar, circles, icons, drawn_title) = drawn_colors(&rule);
        assert_eq!(bar, c.surface_container);
        assert_eq!(circles, [c.surface_container_highest; 4]);
        assert_eq!(icons, [c.on_surface; 4]);
        assert_eq!(drawn_title, title);
    };
    check(light, light.on_surface);
    rule.host().set_system_dark_theme(true);
    rule.wait_for_idle();
    check(dark, dark.on_surface);
    rule.host().set_dynamic_tonal_palette(Some(palette));
    rule.wait_for_idle();
    check(seeded_dark, seeded_dark.on_surface);
    rule.host().set_system_dark_theme(false);
    rule.host().set_window_focused(false);
    rule.wait_for_idle();
    check(seeded_light, seeded_light.on_surface_variant);
}

fn main() {
    kompoz_runtime::set_ui_thread();
    let tests: &[(&str, fn())] = &[
        ("the_bar_is_above_the_content", the_bar_is_above_the_content),
        ("the_buttons_are_where_the_desktop_puts_them", the_buttons_are_where_the_desktop_puts_them),
        ("right_to_left_the_sides_are_mirrored", right_to_left_the_sides_are_mirrored),
        ("the_buttons_and_gestures_ask_the_window", the_buttons_and_gestures_ask_the_window),
        ("the_buttons_are_labelled_in_the_apps_language", the_buttons_are_labelled_in_the_apps_language),
        ("the_colors_are_the_systems", the_colors_are_the_systems),
    ];
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = 0;
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        let ok = std::panic::catch_unwind(test).is_ok();
        failed += usize::from(!ok);
        println!("{} {name}", if ok { "ok    " } else { "FAILED" });
    }
    if failed > 0 {
        std::process::exit(1);
    }
}

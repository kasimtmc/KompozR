//! Material's text fields as Compose's material3 has them (TextField.kt, OutlinedTextField.kt, SecureTextField.kt,
//! TextFieldDefaults.kt, internal/TextFieldImpl.kt): the C++ Kompoz's text field tests by name (kompoz_tests.cpp:
//! their Material halves where the editing is foundation's), and Compose's behaviour and numbers (56 dp tall, 16 dp
//! padded, the label 8 dp from the top, …) where the C++ tests don't reach.
//!
//! In a MaterialTheme unless said (Compose's fields take the theme's body large style; outside one the text is
//! Material's 14 sp `DefaultTextStyle`, where the C++ fields had 16). The fake text backend: a character is half its
//! size wide plus the letter spacing, rounded to whole px in still text (body large: 9 px). Density 1: dp are px.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_extended::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_text::TextRange;
use kompoz_text::input::{TextFieldValue, VisualTransformation};
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::graphics_context::ClipOp;
use kompoz_ui::input::{Key, KeyModifiers, PointerType};
use kompoz_ui::platform::{LocalLayoutDirection, PlatformCallbacks};
use kompoz_ui::semantics::SemanticsProperties;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    text_field_colors_follow_focus_and_error,
    disabled_text_fields_take_no_focus_and_fade,
    text_field_labels_go_above_or_stay_up,
    outlined_fields_keep_room_for_the_floated_label,
    text_field_shapes_round_the_container_and_the_outline,
    text_field_prefix_and_suffix_sit_around_the_text,
    text_fields_take_composable_labels_and_text_forms,
    a_tap_on_a_text_fields_icon_focuses_it,
    provide_text_style_and_text_without_a_direction_follow_the_layout,
    draw_reaches_canvas,
    overlays_fields_and_indicators_animate_composing_around_them_only_as_they_turn,
    fields_of_both_engines_edit_their_text,
    secure_fields_obfuscate_their_text_on_one_line,
    attached_labels_move_between_their_alignments_and_tell_their_progress,
    the_placeholder_fades_in_as_the_field_is_focused,
    the_indicator_line_follows_focus_error_and_enabled,
    the_outline_is_cut_where_the_minimized_label_sits,
    supporting_text_sits_below_and_errors_are_told,
    fields_answer_intrinsic_sizes,
    right_to_left_fields_mirror_their_parts,
    text_field_defaults_and_colors,
    decorators_decoration_boxes_and_containers_on_basic_fields,
    the_label_position_is_compared_and_printed_as_compose_s,
);

// ---- Helpers --------------------------------------------------------------------------------------------------

fn text_slot(s: &'static str) -> Option<TextFieldSlot> {
    Some(Rc::new(kompozable_lambda!(move || {
        Text(s);
    })))
}

fn label_slot(s: &'static str) -> Option<TextFieldLabel> {
    Some(Rc::new(move |_: &dyn TextFieldLabelScope| {
        kompozable_block!({
            Text(s);
        })
    }))
}

/// A slot recording the content color it is composed in.
fn color_slot(into: &Rc<Cell<Color>>) -> Option<TextFieldSlot> {
    let into = into.clone();
    Some(Rc::new(move || into.set(LocalContentColor.current())))
}

fn slot(f: impl Fn() + 'static) -> Option<TextFieldSlot> {
    Some(Rc::new(f))
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    named_args!(rule.on_node_with_text(text, use_unmerged_tree = true)).get_unclipped_bounds_in_root()
}

/// Each place `text` was drawn: its start, size, color.
fn drawn(canvas: &RecordingCanvas, text: &str) -> Vec<(Offset, f32, Color)> {
    (0..canvas.texts.len())
        .filter(|&i| canvas.texts[i] == text)
        .map(|i| (canvas.baselines[i], canvas.text_styles[i].font_size, canvas.colors[i]))
        .collect()
}

fn color_of(canvas: &RecordingCanvas, text: &str) -> Color {
    drawn(canvas, text).first().map_or(Color::Unspecified, |d| d.2)
}

fn has(colors: &[Color], color: Color) -> bool {
    colors.contains(&color)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

fn key(rule: &KompozTestRule, key: Key, modifiers: KeyModifiers) {
    rule.host().key_down(key, modifiers);
    rule.host().key_up(key, modifiers);
    rule.wait_for_idle();
}

fn type_text(rule: &KompozTestRule, text: &str) {
    rule.host().text_input(text);
    rule.wait_for_idle();
}

/// Lets the field's animations end (the theme's springs settle well within this).
fn settle(rule: &KompozTestRule) {
    rule.advance_time_by(600);
    rule.wait_for_idle();
}

/// A platform telling whether an input session is active.
#[derive(Default)]
struct InputPlatform {
    active: Cell<bool>,
}

impl PlatformCallbacks for InputPlatform {
    fn text_input(
        &self,
        active: bool,
        _: Rect,
        _: &kompoz_ui::platform::TextEditState,
        _: &kompoz_ui::platform::ImeOptions,
    ) {
        self.active.set(active);
    }
}

/// The light scheme's (what the theme gives).
fn scheme() -> ColorScheme {
    light_color_scheme()
}

/// The filled container's paths: the fills of `color` and their shapes.
fn path_shapes(canvas: &RecordingCanvas, color: Color) -> Vec<(Rect, Path)> {
    (0..canvas.paths.len())
        .filter(|&i| canvas.path_colors[i] == color && canvas.path_strokes[i] == 0.0)
        .map(|i| (canvas.paths[i], canvas.path_shapes[i].clone()))
        .collect()
}

fn contains(path: &Path, p: Offset) -> bool {
    PathHitTester::with_path(path, 0.1).contains(p)
}

// ---- The C++ tests ----------------------------------------------------------------------------------------------

/// C++ `text_field_colors_follow_focus_and_error`: each part in its color of the field's state — unfocused, focused,
/// in error (over focus) —, what isn't given the theme's; the selection's and the cursor's colors, a password
/// field's too. The C++ `TextFieldColors{…}` of designated colors is `TextFieldDefaults.colors(…)` (the rest the
/// theme's, as there). Compose's filled container and indicator are paths (its top-rounded shape), the cursor a path
/// too.
fn text_field_colors_follow_focus_and_error() {
    let error = mutable_state_of(false);
    let value = mutable_state_of("ali".to_string());
    let (leading, trailing, error_trailing) = (
        Rc::new(Cell::new(Color::Unspecified)),
        Rc::new(Cell::new(Color::Unspecified)),
        Rc::new(Cell::new(Color::Unspecified)),
    );
    let c = |r: f32| Color::new(r, 0.5, 0.5, 1.0);
    let password = TextFieldState::new("gizli");
    let (e, v, l, t, et, p) =
        (error.clone(), value.clone(), leading.clone(), trailing.clone(), error_trailing.clone(), password.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (e, v, l, t, et, p) = (e.clone(), v.clone(), l.clone(), t.clone(), et.clone(), p.clone());
        MaterialTheme(
            content = move || {
                let (e, v, l, t, et, p) = (e.clone(), v.clone(), l.clone(), t.clone(), et.clone(), p.clone());
                Column(
                    content = move || {
                        let colors = TextFieldDefaults.colors(
                            focused_text_color = c(0.01),
                            unfocused_text_color = c(0.02),
                            error_text_color = c(0.03),
                            focused_container_color = c(0.04),
                            unfocused_container_color = c(0.05),
                            error_container_color = c(0.06),
                            cursor_color = c(0.07),
                            error_cursor_color = c(0.08),
                            selection_colors = Some(TextSelectionColors::new(c(0.09), c(0.10))),
                            focused_indicator_color = c(0.11),
                            unfocused_indicator_color = c(0.12),
                            error_indicator_color = c(0.13),
                            focused_leading_icon_color = c(0.14),
                            unfocused_leading_icon_color = c(0.15),
                            error_trailing_icon_color = c(0.16),
                            focused_label_color = c(0.17),
                            unfocused_label_color = c(0.18),
                            error_label_color = c(0.19),
                            unfocused_supporting_text_color = c(0.20),
                            error_supporting_text_color = c(0.21),
                            focused_prefix_color = c(0.22),
                            unfocused_prefix_color = c(0.23),
                            focused_suffix_color = c(0.24),
                        );
                        let v2 = v.clone();
                        StringTextField(
                            v.get(),
                            move |it| v2.set(it),
                            modifier = Modifier.test_tag("ad"),
                            label = text_slot("Ad"),
                            prefix = text_slot("@"),
                            suffix = text_slot("!"),
                            leading_icon = color_slot(&l),
                            trailing_icon = color_slot(&t),
                            supporting_text = text_slot("ipucu"),
                            is_error = e.get(),
                            colors = colors,
                        );
                        // (Material's own colors)
                        StringOutlinedTextField(
                            "",
                            |_| {},
                            modifier = Modifier.test_tag("boş"),
                            placeholder = text_slot("yaz"),
                            trailing_icon = color_slot(&et),
                            is_error = true,
                        );
                        // (one line, its colors passed on)
                        OutlinedSecureTextField(
                            p.clone(),
                            modifier = Modifier.test_tag("parola"),
                            colors = OutlinedTextFieldDefaults.colors(
                                cursor_color = c(0.30),
                                selection_colors = Some(TextSelectionColors::new(c(0.31), c(0.32))),
                            ),
                        );
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    let cs = scheme();
    let idle = rule.draw();
    assert_eq!(color_of(&idle, "ali"), c(0.02));
    assert_eq!(color_of(&idle, "Ad"), c(0.18));
    assert_eq!(color_of(&idle, "ipucu"), c(0.20));
    // (the suffix's unfocused: Material's)
    assert!(color_of(&idle, "@") == c(0.23) && color_of(&idle, "!") == cs.on_surface_variant);
    assert!(has(&idle.path_colors, c(0.05)) && has(&idle.path_colors, c(0.12)));
    assert!(leading.get() == c(0.15) && trailing.get() == cs.on_surface_variant);
    // The outlined one with nothing asked: Material's placeholder color, a transparent container.
    assert_eq!(color_of(&idle, "yaz"), cs.on_surface_variant);
    assert!(!has(&idle.rect_colors, cs.surface_container_highest));
    assert_eq!(error_trailing.get(), cs.error);

    // Focused: the focused colors, the cursor's; the indicator animates there.
    tap(&rule, bounds(&rule, "ad").center());
    settle(&rule);
    let focused = rule.draw();
    assert!(color_of(&focused, "ali") == c(0.01) && color_of(&focused, "Ad") == c(0.17));
    assert!(color_of(&focused, "@") == c(0.22) && color_of(&focused, "!") == c(0.24));
    assert!(color_of(&focused, "ipucu") == cs.on_surface_variant && leading.get() == c(0.14));
    assert!(has(&focused.path_colors, c(0.04)) && has(&focused.path_colors, c(0.11)));
    assert!(has(&focused.path_colors, c(0.07)), "the cursor");
    // The selection: its highlight.
    key(&rule, Key::A, KeyModifiers::Ctrl);
    assert!(has(&rule.draw().path_colors, c(0.10)));
    key(&rule, Key::MoveEnd, KeyModifiers::None);

    // In error: error over focus; the leading icon's error color not given (Material's on surface variant).
    error.set(true);
    settle(&rule);
    let in_error = rule.draw();
    assert!(color_of(&in_error, "ali") == c(0.03) && color_of(&in_error, "Ad") == c(0.19));
    assert!(color_of(&in_error, "ipucu") == c(0.21) && color_of(&in_error, "@") == cs.on_surface_variant);
    assert!(leading.get() == cs.on_surface_variant && trailing.get() == c(0.16));
    assert!(
        has(&in_error.path_colors, c(0.06))
            && has(&in_error.path_colors, c(0.13))
            && has(&in_error.path_colors, c(0.08))
    );

    // The one line password field: its cursor and selection.
    tap(&rule, bounds(&rule, "parola").center());
    // (the cursor blinks: seen within a blink)
    let mut cursor = false;
    for _ in 0..4 {
        rule.advance_time_by(250);
        cursor |= has(&rule.draw().path_colors, c(0.30));
    }
    assert!(cursor);
    key(&rule, Key::A, KeyModifiers::Ctrl);
    assert!(has(&rule.draw().path_colors, c(0.32)));
}

/// C++ `disabled_text_fields_take_no_focus_and_fade`: disabled, a field takes no focus and its parts fade (the
/// theme's on surface at 38 %, the prefix its on surface variant at 38 %), disabled over error; a focused field
/// disabled lets go of the focus. The filled container keeps its color, the indicator fades (paths here).
fn disabled_text_fields_take_no_focus_and_fade() {
    let enabled = mutable_state_of(true);
    let value = mutable_state_of("ali".to_string());
    let leading = Rc::new(Cell::new(Color::Unspecified));
    let own = Color::new(0.3, 0.6, 0.9, 1.0);
    let (en, v, l) = (enabled.clone(), value.clone(), leading.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (en, v, l) = (en.clone(), v.clone(), l.clone());
        MaterialTheme(
            content = move || {
                let (en, v, l) = (en.clone(), v.clone(), l.clone());
                Column(
                    content = move || {
                        let v2 = v.clone();
                        StringTextField(
                            v.get(),
                            move |it| v2.set(it),
                            modifier = Modifier.test_tag("ad"),
                            label = text_slot("Ad"),
                            prefix = text_slot("@"),
                            leading_icon = color_slot(&l),
                            supporting_text = text_slot("ipucu"),
                            enabled = en.get(),
                        );
                        StringOutlinedTextField(
                            "",
                            |_| {},
                            modifier = Modifier.test_tag("boş"),
                            placeholder = text_slot("yaz"),
                            enabled = false,
                            colors = OutlinedTextFieldDefaults.colors(disabled_placeholder_color = own),
                        );
                        let password = remember_text_field_state();
                        SecureTextField(
                            password,
                            modifier = Modifier.test_tag("parola"),
                            placeholder = text_slot("parola"),
                            supporting_text = text_slot("hatalı"),
                            is_error = true,
                            enabled = false,
                        );
                    },
                );
            },
        );
    }));
    let platform = Rc::new(InputPlatform::default());
    rule.host().set_callbacks(platform.clone());
    rule.wait_for_idle();
    let cs = scheme();
    let faded = named_args!(cs.on_surface.copy(alpha = 0.38));
    tap(&rule, bounds(&rule, "boş").center());
    assert!(rule.host().focused_node().is_none() && !platform.active.get());
    assert_eq!(color_of(&rule.draw(), "yaz"), own);
    tap(&rule, bounds(&rule, "parola").center());
    let c = rule.draw();
    assert!(rule.host().focused_node().is_none() && color_of(&c, "parola") == faded && color_of(&c, "hatalı") == faded);

    // Focused, then disabled: it lets go of the focus and fades.
    tap(&rule, bounds(&rule, "ad").center());
    assert!(rule.host().focused_node().is_some() && platform.active.get());
    assert_eq!(color_of(&rule.draw(), "ali"), cs.on_surface);
    enabled.set(false);
    settle(&rule);
    assert!(rule.host().focused_node().is_none());
    let c = rule.draw();
    assert!(color_of(&c, "ali") == faded && color_of(&c, "Ad") == faded && color_of(&c, "ipucu") == faded);
    assert!(leading.get() == faded && color_of(&c, "@") == named_args!(cs.on_surface_variant.copy(alpha = 0.38)));
    // Both filled ones: the container as it is, the indicator faded (snapped, not animated).
    assert_eq!(c.path_colors.iter().filter(|&&p| p == cs.surface_container_highest).count(), 2);
    assert_eq!(c.path_colors.iter().filter(|&&p| p == faded).count(), 2);
    // (taps do nothing now)
    tap(&rule, bounds(&rule, "ad").center());
    type_text(&rule, "x");
    assert!(rule.host().focused_node().is_none() && value.peek() == "ali");
}

/// C++ `text_field_labels_go_above_or_stay_up`: a label above the field is body small over it (its 16 dp line and
/// 4 dp below, 4 dp in from the side or the end), the field as without a label; one always minimized stays small at
/// the top with the placeholder and prefix shown while empty; the usual one stays expanded, the placeholder hidden.
fn text_field_labels_go_above_or_stay_up() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            MaterialTheme(
                content = || {
                    Column(
                        content = || {
                            StringOutlinedTextField(
                                "ali",
                                |_| {},
                                modifier = Modifier.test_tag("üstte"),
                                label = text_slot("Ad"),
                            );
                            let above = TextFieldState::new("ali2");
                            OutlinedTextField(
                                remember(move || above.clone()),
                                modifier = Modifier.test_tag("üstte2"),
                                label = label_slot("Ad2"),
                                label_position = TextFieldLabelPosition::above(Alignment::Start),
                            );
                            TextField(
                                remember(|| TextFieldState::new("veli")),
                                modifier = Modifier.test_tag("sonda"),
                                line_limits = TextFieldLineLimits::SingleLine,
                                label = label_slot("Soyad"),
                                label_position = TextFieldLabelPosition::above(Alignment::End),
                            );
                            TextField(
                                remember(|| TextFieldState::new("deli")),
                                modifier = Modifier.test_tag("düz"),
                                line_limits = TextFieldLineLimits::SingleLine,
                            );
                            TextField(
                                remember(|| TextFieldState::new("")),
                                modifier = Modifier.test_tag("yukarıda"),
                                line_limits = TextFieldLineLimits::SingleLine,
                                placeholder = text_slot("yaz"),
                                label = label_slot("Tutar"),
                                prefix = text_slot("₺"),
                                label_position =
                                    TextFieldLabelPosition::attached(true, Alignment::Start, Alignment::Start),
                            );
                            TextField(
                                remember(|| TextFieldState::new("")),
                                modifier = Modifier.test_tag("aşağıda"),
                                line_limits = TextFieldLineLimits::SingleLine,
                                placeholder = text_slot("boş"),
                                label = label_slot("Not"),
                            );
                            SecureTextField(
                                remember_text_field_state(),
                                modifier = Modifier.test_tag("parola"),
                                label = label_slot("Parola"),
                                label_position = TextFieldLabelPosition::above(Alignment::Start),
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 1000),
    );
    rule.wait_for_idle();
    let c = rule.draw();
    let cs = scheme();
    // (the value-based outlined one keeps its label attached: minimized in the outline, 8 dp above the box)
    assert!(near(bounds(&rule, "üstte").height(), 64.0, 0.5));
    // Above: body small over the field (its 16 dp line and 4 below), the field itself as without a label.
    let above = bounds(&rule, "üstte2");
    let label = text_bounds(&rule, "Ad2");
    assert!(near(above.height(), 20.0 + 56.0, 0.5), "{above:?}");
    assert!(
        near(drawn(&c, "Ad2")[0].1, 12.0, 0.01)
            && near(label.top, above.top, 0.5)
            && near(label.left, above.left + 4.0, 0.5)
    );
    assert_eq!(drawn(&c, "Ad2")[0].2, cs.on_surface_variant);
    // (the text 16 dp below the field's top, the label's 20 dp lower)
    assert!(drawn(&c, "ali2")[0].0.y - above.top > 36.0);
    let end = bounds(&rule, "sonda");
    let soyad = text_bounds(&rule, "Soyad");
    assert!(near(soyad.right, end.right - 4.0, 0.5), "{soyad:?} {end:?}");
    // The filled one's text sits as in a field without a label, 20 dp lower.
    let plain = bounds(&rule, "düz");
    assert!(near(drawn(&c, "veli")[0].0.y - end.top, drawn(&c, "deli")[0].0.y - plain.top + 20.0, 0.5));
    // Always minimized: the label small at the top, the placeholder and the prefix shown while empty.
    assert!(near(drawn(&c, "Tutar")[0].1, 12.0, 0.01) && !drawn(&c, "yaz").is_empty() && !drawn(&c, "₺").is_empty());
    // The usual one: the label expanded, the placeholder hidden.
    assert!(drawn(&c, "boş").is_empty() && near(drawn(&c, "Not")[0].1, 16.0, 0.01));
    // A password field's too.
    assert!(near(bounds(&rule, "parola").height(), 76.0, 0.5) && near(drawn(&c, "Parola")[0].1, 12.0, 0.01));
}

/// C++ `outlined_fields_keep_room_for_the_floated_label`: an outlined field with a label is half the minimized
/// label's line (8 dp) taller, the label across the outline inside the field; without a label, and a filled one,
/// 56 dp.
fn outlined_fields_keep_room_for_the_floated_label() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(
                    content = || {
                        StringOutlinedTextField(
                            "ali",
                            |_| {},
                            modifier = Modifier.test_tag("etiketli"),
                            label = text_slot("Ad"),
                        );
                        StringOutlinedTextField("veli", |_| {}, modifier = Modifier.test_tag("etiketsiz"));
                        StringTextField(
                            "can",
                            |_| {},
                            modifier = Modifier.test_tag("dolgulu"),
                            label = text_slot("Ad"),
                        );
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    let labelled = bounds(&rule, "etiketli");
    assert!(near(labelled.height(), 64.0, 0.5));
    assert!(near(bounds(&rule, "etiketsiz").height(), 56.0, 0.5) && near(bounds(&rule, "dolgulu").height(), 56.0, 0.5));
    // The floated label (body small: 16 dp line) sits across the outline (8 dp down), inside the field.
    let label = named_args!(rule.on_all_nodes_with_text("Ad", use_unmerged_tree = true)).fetch_semantics_nodes()[0]
        .bounds_in_root();
    assert!(label.top >= labelled.top - 0.5 && label.top < labelled.top + 4.0, "{label:?} {labelled:?}");
    assert!(near(label.center().y, labelled.top + 8.0, 0.5));
}

/// C++ `text_field_shapes_round_the_container_and_the_outline`: the filled container in its shape (the theme's
/// extra small top: 4 dp at the top), a field's own shape (12 dp all round), the outlined one's (24 dp, the border
/// following it), a circle's (half the 56 dp height).
fn text_field_shapes_round_the_container_and_the_outline() {
    let blue = Color::new(0.2, 0.4, 0.8, 1.0);
    let green = Color::new(0.2, 0.7, 0.3, 1.0);
    let half_green = named_args!(green.copy(alpha = 0.5));
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        MaterialTheme(
            content = move || {
                Column(
                    content = move || {
                        StringTextField("a", |_| {}, modifier = Modifier.test_tag("dolgulu"));
                        StringTextField(
                            "b",
                            |_| {},
                            colors = TextFieldDefaults.colors(unfocused_container_color = blue),
                            shape = Arc::new(RoundedCornerShape(12.dp())),
                        );
                        StringOutlinedTextField("c", |_| {}, modifier = Modifier.test_tag("çizgili"));
                        StringOutlinedTextField(
                            "d",
                            |_| {},
                            modifier = Modifier.test_tag("yuvarlak"),
                            colors = OutlinedTextFieldDefaults.colors(unfocused_container_color = green),
                            shape = Arc::new(RoundedCornerShape(24.dp())),
                        );
                        SecureTextField(
                            remember(|| TextFieldState::new("x")),
                            colors = TextFieldDefaults.colors(unfocused_container_color = half_green),
                            shape = Arc::new(CircleShape),
                        );
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    let c = rule.draw();
    let corners = |color: Color| {
        let i =
            c.rect_colors.iter().position(|&r| r == color).unwrap_or_else(|| panic!("{color:?} {:?}", c.rect_colors));
        c.rect_corners[i]
    };
    // The default filled container: a path, rounded 4 dp at the top only.
    let filled = bounds(&rule, "dolgulu");
    let (_, top) = path_shapes(&c, scheme().surface_container_highest)
        .into_iter()
        .find(|(b, _)| near(b.top, filled.top, 0.5))
        .expect("the filled container");
    assert!(!contains(&top, Offset::new(0.5, 0.5)) && contains(&top, Offset::new(2.0, 2.0)));
    assert!(contains(&top, Offset::new(0.5, filled.bottom - 0.5)));
    let own = corners(blue);
    assert!(near(own.top_left, 12.0, 0.01) && near(own.bottom_right, 12.0, 0.01));
    let outlined = corners(green);
    assert!(near(outlined.top_left, 24.0, 0.01) && near(outlined.bottom_left, 24.0, 0.01));
    assert!(near(corners(half_green).top_left, 28.0, 0.01));
    // The outlines follow the corners.
    let border_of = |tag: &str| {
        let b = bounds(&rule, tag);
        let i = c.strokes.iter().position(|s| near(s.top, b.top, 1.0)).expect("its border");
        c.stroke_corners[i]
    };
    assert!(near(border_of("çizgili").top_left, 4.0, 1.0) && near(border_of("yuvarlak").top_left, 24.0, 1.0));
}

/// C++ `text_field_prefix_and_suffix_sit_around_the_text`: the prefix at the start padding, the text 2 dp after it,
/// the suffix at the end padding, on the text's baseline, in the theme's colors; taps find their place past the
/// prefix; an outlined field's suffix waits for its label to minimize; on several lines, on the first one.
fn text_field_prefix_and_suffix_sit_around_the_text() {
    let price = mutable_state_of("12".to_string());
    let weight = mutable_state_of(String::new());
    let (p, w) = (price.clone(), weight.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (p, w) = (p.clone(), w.clone());
        MaterialTheme(
            content = move || {
                let (p, w) = (p.clone(), w.clone());
                Column(
                    content = move || {
                        let (p2, w2) = (p.clone(), w.clone());
                        StringTextField(
                            p.get(),
                            move |it| p2.set(it),
                            modifier = Modifier.test_tag("fiyat"),
                            single_line = true,
                            prefix = text_slot("$"),
                            suffix = text_slot("kg"),
                        );
                        StringOutlinedTextField(
                            w.get(),
                            move |it| w2.set(it),
                            modifier = Modifier.test_tag("kilo"),
                            single_line = true,
                            label = text_slot("Kilo"),
                            suffix = text_slot("kg"),
                        );
                        StringTextField(
                            "a\nb",
                            |_| {},
                            modifier = Modifier.test_tag("satırlar"),
                            prefix = text_slot("#"),
                        );
                        // (no label: shown empty too)
                        StringTextField("", |_| {}, single_line = true, prefix = text_slot("@"));
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    let cs = scheme();
    let f = bounds(&rule, "fiyat");
    let c = rule.draw();
    let (dollar, text, kg) = (drawn(&c, "$"), drawn(&c, "12"), drawn(&c, "kg"));
    // (the outlined one's suffix waits for its label to minimize)
    assert!(dollar.len() == 1 && text.len() == 1 && kg.len() == 1 && drawn(&c, "@").len() == 1);
    // "$" at the start padding, then 2 dp, then the text; "kg" at the end, 16 dp in.
    let (dollar_width, kg_width) = (text_bounds(&rule, "$").width(), text_bounds(&rule, "kg").width());
    assert!(near(dollar[0].0.x, f.left + 16.0, 0.5) && near(text[0].0.x, f.left + 16.0 + dollar_width + 2.0, 0.5));
    assert!(near(kg[0].0.x, f.right - 16.0 - kg_width, 0.5));
    assert!(near(dollar[0].0.y, text[0].0.y, 0.5) && near(kg[0].0.y, text[0].0.y, 0.5));
    assert!(dollar[0].2 == cs.on_surface_variant && kg[0].2 == cs.on_surface_variant && text[0].2 == cs.on_surface);
    // Taps find their place past the prefix: just right of "$" is the start, before "kg" the end.
    tap(&rule, Offset::new(text[0].0.x + 1.0, f.center().y));
    type_text(&rule, "9");
    assert_eq!(price.peek(), "912");
    tap(&rule, Offset::new(f.right - 40.0, f.center().y));
    type_text(&rule, "5");
    assert_eq!(price.peek(), "9125");
    // Focused, the outlined one's label minimizes and its suffix shows.
    let k = bounds(&rule, "kilo");
    tap(&rule, Offset::new(k.left + 100.0, k.center().y));
    settle(&rule);
    let kg = drawn(&rule.draw(), "kg");
    assert!(kg.len() == 2 && near(kg[1].0.x, k.right - 16.0 - kg_width, 0.5), "{kg:?}");
    // Several lines: on the first line's baseline.
    let m = bounds(&rule, "satırlar");
    let c = rule.draw();
    let (hash, a) = (drawn(&c, "#"), c.text_draws.iter().position(|t| t.text.starts_with('a')));
    assert!(hash.len() == 1 && a.is_some());
    let a = c.baselines[a.unwrap()];
    let hash_width = text_bounds(&rule, "#").width();
    assert!(near(hash[0].0.x, m.left + 16.0, 0.5) && near(a.x, m.left + 16.0 + hash_width + 2.0, 0.5));
    assert!(near(hash[0].0.y, a.y, 0.5));
}

/// C++ `text_fields_take_composable_labels_and_text_forms`, its Material part: a label, a placeholder and a
/// supporting text as composables. (Its links, SelectionContainer, text measurer and `MultiLine(maxHeightInLines =
/// …)` parts are foundation's and ui's.)
fn text_fields_take_composable_labels_and_text_forms() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(
                    content = || {
                        StringOutlinedTextField(
                            String::new(),
                            |_| {},
                            label = text_slot("E-posta"),
                            supporting_text = text_slot("Paylaşmayız"),
                            modifier = Modifier.test_tag("alan"),
                        );
                        StringOutlinedTextField(String::new(), |_| {}, placeholder = text_slot("Mesaj"));
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    for text in ["E-posta", "Paylaşmayız", "Mesaj"] {
        named_args!(rule.on_node_with_text(text, use_unmerged_tree = true)).assert_exists();
    }
    assert!(rule.drawn_texts().contains(&"E-posta".to_string()));
}

/// C++ `a_tap_on_a_text_fields_icon_focuses_it`: the leading icon is part of the field — a tap there focuses it,
/// the caret at the nearer end (the start) —; a clickable trailing icon keeps its click and the field its caret,
/// a long press on it selects nothing either.
fn a_tap_on_a_text_fields_icon_focuses_it() {
    let value = mutable_state_of(String::new());
    let eyes = Rc::new(Cell::new(0));
    let away = FocusRequester::new();
    let (v, e, a) = (value.clone(), eyes.clone(), away.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, e, a) = (v.clone(), e.clone(), a.clone());
        MaterialTheme(
            content = move || {
                let (v, e, a) = (v.clone(), e.clone(), a.clone());
                Column(
                    content = move || {
                        let (v2, e) = (v.clone(), e.clone());
                        StringOutlinedTextField(
                            v.get(),
                            move |it| v2.set(it),
                            modifier = Modifier.test_tag("alan"),
                            label = text_slot("Şifre"),
                            leading_icon = slot(kompozable_lambda!(|| {
                                VectorIcon(Icons::Filled::Lock(), Some("kilit".to_string()));
                            })),
                            trailing_icon = slot(kompozable_lambda!(move || {
                                let e = e.clone();
                                IconButton(
                                    move || e.set(e.get() + 1),
                                    content = || VectorIcon(Icons::Filled::Visibility(), Some("göz".to_string())),
                                );
                            })),
                        );
                        Box(modifier = Modifier.size(10.dp()).focus_requester(&a).focusable());
                    },
                );
            },
        );
    }));
    let platform = Rc::new(InputPlatform::default());
    rule.host().set_callbacks(platform.clone());
    rule.wait_for_idle();
    // (the field merges its icons' semantics: their own nodes are in the unmerged tree)
    let icon = |description: &str| {
        named_args!(rule.on_node_with_content_description(description, use_unmerged_tree = true))
            .get_unclipped_bounds_in_root()
            .center()
    };
    let lock = || icon("kilit");
    tap(&rule, lock());
    assert!(platform.active.get());
    type_text(&rule, "a");
    assert_eq!(value.peek(), "a");
    // The caret goes to the nearer end: the start beside the leading icon.
    value.set("xyz".to_string());
    rule.wait_for_idle();
    away.request_focus();
    rule.wait_for_idle();
    tap(&rule, lock());
    type_text(&rule, "a");
    assert_eq!(value.peek(), "axyz");
    // A clickable trailing icon keeps its click, and the field the caret.
    let eye = icon("göz");
    tap(&rule, eye);
    type_text(&rule, "b");
    assert!(eyes.get() == 1 && value.peek() == "abxyz", "{} {}", eyes.get(), value.peek());
    // Held on it, it doesn't select a word of the field either.
    rule.host().pointer_down(eye, -1, 0, PointerType::Touch);
    rule.advance_time_by(600);
    rule.host().pointer_up(eye, -1, 0);
    rule.wait_for_idle();
    type_text(&rule, "c");
    assert_eq!(value.peek(), "abcxyz");
}

/// C++ `provide_text_style_and_text_without_a_direction_follow_the_layout`, its text field part (the rest is in
/// components.rs): digits in a field right to left start at the right.
fn provide_text_style_and_text_without_a_direction_follow_the_layout() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
            StringTextField("678", |_| {}, modifier = Modifier.test_tag("alan"), single_line = true);
        });
    }));
    rule.wait_for_idle();
    let field = bounds(&rule, "alan");
    let c = rule.draw();
    assert!(drawn(&c, "678")[0].0.x > field.left + field.width() / 2.0);
}

/// C++ `draw_reaches_canvas`: a text and a field's placeholder are drawn.
fn draw_reaches_canvas() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(
                    content = || {
                        Text("Merhaba");
                        StringTextField("", |_| {}, placeholder = text_slot("Yaz…"));
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    assert_eq!(rule.drawn_texts(), vec!["Merhaba".to_string(), "Yaz…".to_string()]);
}

/// C++ `overlays_fields_and_indicators_animate_composing_around_them_only_as_they_turn`, its text field part: a
/// field's label minimizing as it is focused doesn't compose what is around it on the way (Compose's label
/// composes itself — its style follows the progress —, nothing above it).
fn overlays_fields_and_indicators_animate_composing_around_them_only_as_they_turn() {
    let composed = Rc::new(Cell::new(0));
    let text = mutable_state_of(String::new());
    let (c, t) = (composed.clone(), text.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        c.set(c.get() + 1);
        let t = t.clone();
        Column(
            content = move || {
                let t2 = t.clone();
                StringTextField(
                    t.get(),
                    move |s| t2.set(s),
                    modifier = Modifier.test_tag("alan"),
                    label = text_slot("Ad"),
                );
            },
        );
    }));
    rule.wait_for_idle();
    let field = bounds(&rule, "alan").center();
    rule.host().pointer_down(field, -1, 0, PointerType::Touch);
    rule.host().pointer_up(field, -1, 0);
    rule.advance_frame();
    rule.advance_frame();
    let before = composed.get();
    let size = || drawn(&rule.draw(), "Ad")[0].1;
    let start = size();
    rule.advance_time_by(48);
    assert_eq!(composed.get(), before);
    // (on the way)
    assert!(size() < start || start < 14.0, "{start} {}", size());
    settle(&rule);
    assert_eq!(composed.get(), before);
    assert!(near(size(), 12.0, 0.01));
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

/// Both engines: the state-based field edits its state (its output transformation shown, what tells empty), the
/// String one tells each text, the TextFieldValue one each value (a moved caret too); the outlined ones the same.
fn fields_of_both_engines_edit_their_text() {
    let state = TextFieldState::new("");
    let outlined_state = TextFieldState::new("x");
    let string = mutable_state_of("a".to_string());
    let value = mutable_state_of(TextFieldValue::from_text("bc", TextRange::new(2, 2), None));
    let (s, os, st, v) = (state.clone(), outlined_state.clone(), string.clone(), value.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (s, os, st, v) = (s.clone(), os.clone(), st.clone(), v.clone());
            MaterialTheme(
                content = move || {
                    let (s, os, st, v) = (s.clone(), os.clone(), st.clone(), v.clone());
                    Column(
                        content = move || {
                            TextField(
                                s.clone(),
                                modifier = Modifier.test_tag("durum"),
                                placeholder = text_slot("boş"),
                                output_transformation = Some(Rc::new(|buffer: &mut TextFieldBuffer| {
                                    buffer.append("%");
                                })),
                            );
                            OutlinedTextField(os.clone(), modifier = Modifier.test_tag("çizgili"));
                            let st2 = st.clone();
                            StringTextField(st.get(), move |it| st2.set(it), modifier = Modifier.test_tag("metin"));
                            let v2 = v.clone();
                            ValueTextField(v.get(), move |it| v2.set(it), modifier = Modifier.test_tag("değer"));
                            let v3 = v.clone();
                            ValueOutlinedTextField(
                                v.get(),
                                move |it| v3.set(it),
                                modifier = Modifier.test_tag("değer2"),
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 1000),
    );
    rule.wait_for_idle();
    // The output transformation's text isn't empty: no placeholder, though the state is.
    let c = rule.draw();
    assert!(drawn(&c, "boş").is_empty() && !drawn(&c, "%").is_empty());
    tap(&rule, bounds(&rule, "durum").center());
    type_text(&rule, "ad");
    assert!(state.text() == "ad" && !drawn(&rule.draw(), "ad%").is_empty());
    tap(&rule, bounds(&rule, "çizgili").center());
    key(&rule, Key::MoveEnd, KeyModifiers::None);
    type_text(&rule, "y");
    assert_eq!(outlined_state.text(), "xy");
    tap(&rule, bounds(&rule, "metin").center());
    key(&rule, Key::MoveEnd, KeyModifiers::None);
    type_text(&rule, "b");
    assert_eq!(string.peek(), "ab");
    tap(&rule, bounds(&rule, "değer").center());
    key(&rule, Key::MoveEnd, KeyModifiers::None);
    type_text(&rule, "d");
    assert!(value.peek().text() == "bcd" && value.peek().selection() == TextRange::new(3, 3));
    key(&rule, Key::DirectionLeft, KeyModifiers::None);
    assert_eq!(value.peek().selection(), TextRange::new(2, 2));
    tap(&rule, bounds(&rule, "değer2").center());
    key(&rule, Key::MoveEnd, KeyModifiers::None);
    type_text(&rule, "e");
    assert_eq!(value.peek().text(), "bcde");
}

/// Secure fields: their text obfuscated (the character just typed shown a moment, then hidden), one line, the
/// character given; filled and outlined.
fn secure_fields_obfuscate_their_text_on_one_line() {
    let password = TextFieldState::new("gizli");
    let other = TextFieldState::new("abc");
    let (p, o) = (password.clone(), other.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (p, o) = (p.clone(), o.clone());
        MaterialTheme(
            content = move || {
                let (p, o) = (p.clone(), o.clone());
                Column(
                    content = move || {
                        SecureTextField(
                            p.clone(),
                            modifier = Modifier.test_tag("parola"),
                            label = label_slot("Parola"),
                        );
                        OutlinedSecureTextField(
                            o.clone(),
                            modifier = Modifier.test_tag("yıldız"),
                            text_obfuscation_character = '*',
                            text_obfuscation_mode = TextObfuscationMode::Hidden,
                        );
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(!drawn(&c, "•••••").is_empty() && !drawn(&c, "***").is_empty() && drawn(&c, "gizli").is_empty());
    assert!(near(bounds(&rule, "parola").height(), 56.0, 0.5) && near(bounds(&rule, "yıldız").height(), 56.0, 0.5));
    tap(&rule, bounds(&rule, "parola").center());
    rule.host().text_input("x");
    rule.advance_frame();
    assert!(password.text().contains('x'));
    // (revealed a moment)
    assert!(rule.draw().texts.iter().any(|t| t.contains('x')), "{:?}", rule.draw().texts);
    rule.advance_time_by(2000);
    rule.wait_for_idle();
    assert!(!drawn(&rule.draw(), "••••••").is_empty());
}

/// An attached label goes from its expanded alignment (here centred) to its minimized one (the end) as the field
/// is focused, its scope telling how far it is (0 expanded, 1 minimized).
fn attached_labels_move_between_their_alignments_and_tell_their_progress() {
    let progress = Rc::new(RefCell::new(Vec::new()));
    let p = progress.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let p = p.clone();
        MaterialTheme(
            content = move || {
                let p = p.clone();
                let label: TextFieldLabel = Rc::new(move |scope: &dyn TextFieldLabelScope| {
                    p.borrow_mut().push(scope.label_minimized_progress());
                    kompozable_block!({
                        Text("Ad");
                    })
                });
                TextField(
                    remember(|| TextFieldState::new("")),
                    modifier = Modifier.test_tag("alan"),
                    label = Some(label),
                    label_position =
                        TextFieldLabelPosition::attached(false, Alignment::End, Alignment::CenterHorizontally),
                );
            },
        );
    }));
    rule.wait_for_idle();
    let field = bounds(&rule, "alan");
    let label = text_bounds(&rule, "Ad");
    // Expanded: body large, centred across the field.
    assert!(near(label.height(), 24.0, 0.5) && near(label.center().x, field.center().x, 1.0), "{label:?}");
    assert_eq!(*progress.borrow().last().unwrap(), 0.0);
    tap(&rule, field.center());
    settle(&rule);
    let label = text_bounds(&rule, "Ad");
    // Minimized: body small, at the end padding, at the top padding.
    assert!(near(label.right, field.right - 16.0, 0.5) && near(label.top, field.top + 8.0, 0.5), "{label:?}");
    assert_eq!(*progress.borrow().last().unwrap(), 1.0);
}

/// The placeholder of a labelled field: hidden while the label is expanded, faded in (a layer's alpha) as the
/// field is focused; gone once there is text.
fn the_placeholder_fades_in_as_the_field_is_focused() {
    let state = TextFieldState::new("");
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            content = move || {
                TextField(
                    s.clone(),
                    modifier = Modifier.test_tag("alan"),
                    label = label_slot("Ad"),
                    placeholder = text_slot("yaz"),
                );
            },
        );
    }));
    rule.wait_for_idle();
    assert!(drawn(&rule.draw(), "yaz").is_empty());
    let field = bounds(&rule, "alan").center();
    rule.host().pointer_down(field, -1, 0, PointerType::Touch);
    rule.host().pointer_up(field, -1, 0);
    let mut partly = false;
    for _ in 0..12 {
        rule.advance_frame();
        let c = rule.draw();
        partly |= !drawn(&c, "yaz").is_empty() && c.layer_alphas.iter().any(|a| *a > 0.0 && *a < 1.0);
    }
    settle(&rule);
    let c = rule.draw();
    assert!(!drawn(&c, "yaz").is_empty() && !c.layer_alphas.iter().any(|a| *a > 0.0 && *a < 1.0));
    assert!(partly, "faded in");
    type_text(&rule, "a");
    settle(&rule);
    assert!(drawn(&rule.draw(), "yaz").is_empty());
}

/// The indicator line along the filled container's bottom: 1 dp in on surface variant, 2 dp in the primary color
/// focused (animated), the error color in error, on surface at 38 % disabled; a modifier of its own on any layout.
fn the_indicator_line_follows_focus_error_and_enabled() {
    let error = mutable_state_of(false);
    let enabled = mutable_state_of(true);
    let source = MutableInteractionSource::new();
    let (e, en, src) = (error.clone(), enabled.clone(), source.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (e, en, src) = (e.clone(), en.clone(), src.clone());
        MaterialTheme(
            content = move || {
                let (e, en, src) = (e.clone(), en.clone(), src.clone());
                Column(
                    content = move || {
                        TextField(
                            remember(|| TextFieldState::new("a")),
                            modifier = Modifier.test_tag("alan"),
                            is_error = e.get(),
                            enabled = en.get(),
                        );
                        Box(modifier = Modifier
                            .size_wh(100.dp(), 40.dp())
                            .test_tag("kutu")
                            .focusable_with(true, Some(src.clone()))
                            .indicator_line(
                                true,
                                false,
                                src.as_interaction_source(),
                                focused_indicator_line_thickness = 3.dp(),
                            ));
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    let cs = scheme();
    let field = bounds(&rule, "alan");
    let line = |color: Color| {
        let c = rule.draw();
        (0..c.paths.len())
            .find(|&i| c.path_colors[i] == color && near(c.paths[i].bottom, field.bottom, 0.5))
            .map(|i| c.paths[i].height())
    };
    assert_eq!(line(cs.on_surface_variant), Some(1.0));
    rule.host().pointer_down(field.center(), -1, 0, PointerType::Touch);
    rule.host().pointer_up(field.center(), -1, 0);
    let mut midway = false;
    for _ in 0..12 {
        rule.advance_frame();
        let c = rule.draw();
        midway |= (0..c.paths.len()).any(|i| {
            near(c.paths[i].bottom, field.bottom, 0.5) && c.paths[i].height() > 1.05 && c.paths[i].height() < 1.95
        });
    }
    settle(&rule);
    assert_eq!(line(cs.primary), Some(2.0));
    assert!(midway, "the thickness animates");
    error.set(true);
    settle(&rule);
    assert_eq!(line(cs.error), Some(2.0));
    enabled.set(false);
    rule.advance_frame();
    rule.wait_for_idle();
    // (snapped: disabled at once; unfocused, 1 dp)
    assert_eq!(line(named_args!(cs.on_surface.copy(alpha = 0.38))), Some(1.0));

    // On a layout of its own: the theme's filled shape and colors, its own focused thickness.
    let b = bounds(&rule, "kutu");
    let own = |color: Color| {
        let c = rule.draw();
        (0..c.paths.len())
            .find(|&i| c.path_colors[i] == color && near(c.paths[i].bottom, b.bottom, 0.5))
            .map(|i| c.paths[i].height())
    };
    assert_eq!(own(cs.on_surface_variant), Some(1.0));
    rule.on_node_with_tag("kutu").request_focus();
    settle(&rule);
    assert_eq!(own(cs.primary), Some(3.0));
}

/// The outlined field's border is cut where its minimized label sits: from 4 dp before the label to 4 dp after it,
/// across the top edge; no cut while the label is expanded; a label above cuts nothing.
fn the_outline_is_cut_where_the_minimized_label_sits() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(
                    content = || {
                        StringOutlinedTextField(
                            "ali",
                            |_| {},
                            modifier = Modifier.test_tag("dolu"),
                            label = text_slot("Ad"),
                        );
                        StringOutlinedTextField(
                            "",
                            |_| {},
                            modifier = Modifier.test_tag("boş"),
                            label = text_slot("Ad"),
                        );
                        OutlinedTextField(
                            remember(|| TextFieldState::new("veli")),
                            modifier = Modifier.test_tag("üstte"),
                            label = label_slot("Üst"),
                            label_position = TextFieldLabelPosition::above(Alignment::Start),
                        );
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    let c = rule.draw();
    let cuts: Vec<Rect> = c.clip_shapes.iter().filter(|s| s.op == ClipOp::Difference).map(|s| s.bounds).collect();
    let full = bounds(&rule, "dolu");
    // (the container 8 dp down; the label 12.8 dp wide, 16 dp tall)
    assert_eq!(cuts.len(), 1, "{cuts:?}");
    let cut = cuts[0];
    assert!(near(cut.left, 12.0, 0.5) && near(cut.right, 16.0 + 12.8 + 4.0, 0.5), "{cut:?}");
    assert!(near(cut.top, full.top + 8.0 - 8.0, 0.5) && near(cut.bottom, full.top + 8.0 + 8.0, 0.5), "{cut:?}");
}

/// The supporting text: below the field, 4 dp down and 16 dp in, body small; in error the error color, and the
/// field tells accessibility the default error message ("Invalid input").
fn supporting_text_sits_below_and_errors_are_told() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                StringTextField(
                    "a",
                    |_| {},
                    modifier = Modifier.test_tag("alan"),
                    supporting_text = text_slot("ipucu"),
                    is_error = true,
                );
            },
        );
    }));
    rule.wait_for_idle();
    let field = bounds(&rule, "alan");
    let supporting = text_bounds(&rule, "ipucu");
    assert!(near(field.height(), 56.0 + 4.0 + 16.0, 0.5), "{field:?}");
    assert!(near(supporting.top, field.top + 60.0, 0.5) && near(supporting.left, field.left + 16.0, 0.5));
    let c = rule.draw();
    assert!(near(drawn(&c, "ipucu")[0].1, 12.0, 0.01) && drawn(&c, "ipucu")[0].2 == scheme().error);
    let node = rule.on_node_with_tag("alan").fetch_semantics_node();
    assert_eq!(node.config().get(SemanticsProperties::Error), Some("Invalid input".to_string()));
}

/// Intrinsic sizes: a field's height asked (its parts' heights: 56 dp, the supporting text's 20 more; the outlined
/// labelled one 8 dp more), its width (at least its least width).
fn fields_answer_intrinsic_sizes() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(
                    content = || {
                        Row(
                            modifier = Modifier.height_intrinsic(IntrinsicSize::Min),
                            content = || {
                                StringTextField(
                                    "a",
                                    |_| {},
                                    label = text_slot("Ad"),
                                    supporting_text = text_slot("ipucu"),
                                );
                                Box(modifier = Modifier.width(2.dp()).fill_max_height().test_tag("bir"));
                            },
                        );
                        Row(
                            modifier = Modifier.height_intrinsic(IntrinsicSize::Max),
                            content = || {
                                StringOutlinedTextField(
                                    "a",
                                    |_| {},
                                    label = text_slot("Ad"),
                                    leading_icon = text_slot("@"),
                                );
                                Box(modifier = Modifier.width(2.dp()).fill_max_height().test_tag("iki"));
                            },
                        );
                        Column(
                            modifier = Modifier.width_intrinsic(IntrinsicSize::Max).test_tag("üç"),
                            content = || {
                                StringTextField("a", |_| {}, modifier = Modifier.fill_max_width());
                            },
                        );
                    },
                );
            },
        );
    }));
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "bir").height(), 76.0, 0.5), "{:?}", bounds(&rule, "bir"));
    assert!(near(bounds(&rule, "iki").height(), 64.0, 0.5), "{:?}", bounds(&rule, "iki"));
    assert!(near(bounds(&rule, "üç").width(), 280.0, 0.5), "{:?}", bounds(&rule, "üç"));
}

/// Right to left: the leading icon at the right, the prefix after it, the label from the right; the trailing icon
/// at the left.
fn right_to_left_fields_mirror_their_parts() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                    StringTextField(
                        "",
                        |_| {},
                        modifier = Modifier.test_tag("alan"),
                        label = text_slot("Ad"),
                        leading_icon = slot(kompozable_lambda!(|| {
                            Box(modifier = Modifier.size(24.dp()).test_tag("baş"));
                        })),
                        trailing_icon = slot(kompozable_lambda!(|| {
                            Box(modifier = Modifier.size(24.dp()).test_tag("son"));
                        })),
                    );
                });
            },
        );
    }));
    rule.wait_for_idle();
    let field = bounds(&rule, "alan");
    // (in 48 dp boxes, centred)
    let unmerged = |tag: &str| rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root();
    assert!(near(unmerged("baş").right, field.right - 12.0, 0.5) && near(unmerged("son").left, field.left + 12.0, 0.5));
    let label = text_bounds(&rule, "Ad");
    // The expanded label from the right of the space the icons leave, padded by 16 less the icons' 12.
    assert!(near(label.right, field.right - 48.0 - 4.0, 0.5), "{label:?} {field:?}");
}

/// TextFieldDefaults and OutlinedTextFieldDefaults: their sizes, shapes and paddings, the colors of the theme's
/// tokens, `colors(…)` over them, TextFieldColors' copy, constructor and how a state's color is chosen (disabled
/// over error over focused); Compose's deprecated members.
#[allow(deprecated)]
fn text_field_defaults_and_colors() {
    assert!(TextFieldDefaults::MinHeight == 56.dp() && TextFieldDefaults::MinWidth == 280.dp());
    assert!(
        TextFieldDefaults::UnfocusedIndicatorThickness == 1.dp()
            && TextFieldDefaults::FocusedIndicatorThickness == 2.dp()
    );
    assert!(OutlinedTextFieldDefaults::MinHeight == 56.dp() && OutlinedTextFieldDefaults::MinWidth == 280.dp());
    assert!(
        OutlinedTextFieldDefaults::UnfocusedBorderThickness == 1.dp()
            && OutlinedTextFieldDefaults::FocusedBorderThickness == 2.dp()
    );
    assert!(
        TextFieldDefaults::UnfocusedBorderThickness == 1.dp() && TextFieldDefaults::FocusedBorderThickness == 2.dp()
    );
    assert!(TextFieldDefaults.content_padding_with_label() == PaddingValues::new(16.dp(), 8.dp(), 16.dp(), 8.dp()));
    assert!(TextFieldDefaults.content_padding_without_label() == PaddingValues::all(16.dp()));
    assert!(OutlinedTextFieldDefaults.content_padding() == PaddingValues::all(16.dp()));
    assert!(
        named_args!(TextFieldDefaults.content_padding_with_label(top = 2.dp()))
            == PaddingValues::new(16.dp(), 2.dp(), 16.dp(), 8.dp())
    );
    assert!(TextFieldDefaults.text_field_with_label_padding() == TextFieldDefaults.content_padding_with_label());
    assert!(TextFieldDefaults.text_field_without_label_padding() == TextFieldDefaults.content_padding_without_label());
    assert!(TextFieldDefaults.outlined_text_field_padding() == OutlinedTextFieldDefaults.content_padding());

    let seen = Rc::new(RefCell::new(None));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        MaterialTheme(
            content = move || {
                *s.borrow_mut() = Some((
                    TextFieldDefaults.colors(),
                    TextFieldDefaults.colors(focused_text_color = Color::Red, selection_colors = None),
                    OutlinedTextFieldDefaults.colors(),
                    OutlinedTextFieldDefaults.colors(focused_border_color = Color::Green),
                    TextFieldDefaults::shape(),
                    OutlinedTextFieldDefaults::shape(),
                    TextFieldDefaults::outlined_shape(),
                    TextFieldDefaults::filled_shape(),
                    LocalTextSelectionColors.current(),
                ));
            },
        );
    }));
    rule.wait_for_idle();
    let (filled, red, outlined, green, shape, outlined_shape, deprecated_outlined, deprecated_filled, selection) =
        seen.borrow().clone().unwrap();
    let cs = scheme();
    assert!(
        filled.focused_text_color == cs.on_surface && filled.unfocused_container_color == cs.surface_container_highest
    );
    assert!(filled.focused_indicator_color == cs.primary && filled.unfocused_indicator_color == cs.on_surface_variant);
    assert!(filled.disabled_text_color == named_args!(cs.on_surface.copy(alpha = 0.38)));
    assert!(
        filled.error_cursor_color == cs.error
            && filled.cursor_color == cs.primary
            && filled.text_selection_colors == selection
    );
    assert!(red.focused_text_color == Color::Red && red == filled.copy_red());
    assert!(
        outlined.unfocused_container_color == Color::Transparent && outlined.unfocused_indicator_color == cs.outline
    );
    assert!(green.focused_indicator_color == Color::Green && green.unfocused_indicator_color == cs.outline);
    let radii = |shape: &Arc<dyn Shape>| match shape.create_outline(
        Size::new(100.0, 56.0),
        LayoutDirection::Ltr,
        &Density { density: 1.0, font_scale: 1.0 },
    ) {
        Outline::Rounded(r) => (r.top_left_corner_radius.x, r.bottom_left_corner_radius.x),
        other => panic!("{other:?}"),
    };
    assert!(radii(&shape) == (4.0, 0.0) && radii(&outlined_shape) == (4.0, 4.0));
    assert!(*deprecated_outlined == *outlined_shape && *deprecated_filled == *shape);

    // Disabled over error over focused, for each part.
    let colors = TextFieldColors(
        Color::new(0.01, 0.0, 0.0, 1.0),
        Color::new(0.02, 0.0, 0.0, 1.0),
        Color::new(0.03, 0.0, 0.0, 1.0),
        Color::new(0.04, 0.0, 0.0, 1.0),
        Color::new(0.05, 0.0, 0.0, 1.0),
        Color::new(0.06, 0.0, 0.0, 1.0),
        Color::new(0.07, 0.0, 0.0, 1.0),
        Color::new(0.08, 0.0, 0.0, 1.0),
        Color::new(0.09, 0.0, 0.0, 1.0),
        Color::new(0.10, 0.0, 0.0, 1.0),
        selection,
        Color::new(0.11, 0.0, 0.0, 1.0),
        Color::new(0.12, 0.0, 0.0, 1.0),
        Color::new(0.13, 0.0, 0.0, 1.0),
        Color::new(0.14, 0.0, 0.0, 1.0),
        Color::new(0.15, 0.0, 0.0, 1.0),
        Color::new(0.16, 0.0, 0.0, 1.0),
        Color::new(0.17, 0.0, 0.0, 1.0),
        Color::new(0.18, 0.0, 0.0, 1.0),
        Color::new(0.19, 0.0, 0.0, 1.0),
        Color::new(0.20, 0.0, 0.0, 1.0),
        Color::new(0.21, 0.0, 0.0, 1.0),
        Color::new(0.22, 0.0, 0.0, 1.0),
        Color::new(0.23, 0.0, 0.0, 1.0),
        Color::new(0.24, 0.0, 0.0, 1.0),
        Color::new(0.25, 0.0, 0.0, 1.0),
        Color::new(0.26, 0.0, 0.0, 1.0),
        Color::new(0.27, 0.0, 0.0, 1.0),
        Color::new(0.28, 0.0, 0.0, 1.0),
        Color::new(0.29, 0.0, 0.0, 1.0),
        Color::new(0.30, 0.0, 0.0, 1.0),
        Color::new(0.31, 0.0, 0.0, 1.0),
        Color::new(0.32, 0.0, 0.0, 1.0),
        Color::new(0.33, 0.0, 0.0, 1.0),
        Color::new(0.34, 0.0, 0.0, 1.0),
        Color::new(0.35, 0.0, 0.0, 1.0),
        Color::new(0.36, 0.0, 0.0, 1.0),
        Color::new(0.37, 0.0, 0.0, 1.0),
        Color::new(0.38, 0.0, 0.0, 1.0),
        Color::new(0.39, 0.0, 0.0, 1.0),
        Color::new(0.40, 0.0, 0.0, 1.0),
        Color::new(0.41, 0.0, 0.0, 1.0),
        Color::new(0.42, 0.0, 0.0, 1.0),
    );
    let r = |c: Color| (c.red() * 100.0).round() as i32;
    for (f, base) in [
        (TextFieldColors::text_color as fn(&TextFieldColors, bool, bool, bool) -> Color, 1),
        (TextFieldColors::container_color, 5),
        (TextFieldColors::indicator_color, 11),
        (TextFieldColors::leading_icon_color, 15),
        (TextFieldColors::trailing_icon_color, 19),
        (TextFieldColors::label_color, 23),
        (TextFieldColors::placeholder_color, 27),
        (TextFieldColors::supporting_text_color, 31),
        (TextFieldColors::prefix_color, 35),
        (TextFieldColors::suffix_color, 39),
    ] {
        // (focused, unfocused, disabled, error)
        assert_eq!(r(f(&colors, true, false, true)), base);
        assert_eq!(r(f(&colors, true, false, false)), base + 1);
        assert_eq!(r(f(&colors, false, true, true)), base + 2);
        assert_eq!(r(f(&colors, true, true, true)), base + 3);
    }
    assert!(r(colors.cursor_color_of(false)) == 9 && r(colors.cursor_color_of(true)) == 10);
    let copied = named_args!(colors.copy(error_suffix_color = Color::Blue, focused_text_color = Color::Unspecified));
    assert!(copied.error_suffix_color == Color::Blue && copied.focused_text_color == colors.focused_text_color);
    assert!(copied != colors && named_args!(copied.copy(error_suffix_color = colors.error_suffix_color)) == colors);
}

trait CopyRed {
    fn copy_red(&self) -> TextFieldColors;
}

impl CopyRed for TextFieldColors {
    fn copy_red(&self) -> TextFieldColors {
        named_args!(self.copy(focused_text_color = Color::Red))
    }
}

/// The defaults composed on foundation's fields as an app writes them: `decorator(…)` on BasicTextField and
/// BasicSecureTextField, `DecorationBox(…)` on the value-based BasicTextField, `Container(…)` (and the deprecated
/// `ContainerBox(…)`) on their own.
#[allow(deprecated)]
fn decorators_decoration_boxes_and_containers_on_basic_fields() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            MaterialTheme(
                content = || {
                    Column(
                        content = || {
                            let source = remember(MutableInteractionSource::new);
                            let state = remember(|| TextFieldState::new(""));
                            let decorator = TextFieldDefaults.decorator(
                                state.clone(),
                                true,
                                TextFieldLineLimits::SingleLine,
                                None,
                                source.as_interaction_source(),
                                label = label_slot("Dolu"),
                            );
                            BasicTextField(
                                state,
                                modifier = Modifier.test_tag("bir"),
                                interaction_source = Some(source.clone()),
                                decorator = Some(decorator),
                            );
                            let outlined_source = remember(MutableInteractionSource::new);
                            let outlined_state = remember(|| TextFieldState::new("x"));
                            let outlined = OutlinedTextFieldDefaults.decorator(
                                outlined_state.clone(),
                                true,
                                TextFieldLineLimits::Default,
                                None,
                                outlined_source.as_interaction_source(),
                                placeholder = text_slot("yok"),
                            );
                            BasicSecureTextField(
                                outlined_state,
                                modifier = Modifier.test_tag("iki"),
                                interaction_source = Some(outlined_source.clone()),
                                decorator = Some(outlined),
                            );
                            let value_source = remember(MutableInteractionSource::new);
                            let s = value_source.as_interaction_source();
                            let decoration_box: DecorationBox = Rc::new(move |inner: &dyn Fn()| {
                                named_args!(TextFieldDefaults.DecorationBox(
                                    "",
                                    inner,
                                    true,
                                    true,
                                    VisualTransformation::None,
                                    s.clone(),
                                    label = text_slot("Kutu"),
                                ));
                            });
                            BasicTextFieldString(
                                "",
                                |_| {},
                                modifier = Modifier.test_tag("üç"),
                                interaction_source = Some(value_source.clone()),
                                decoration_box = Some(decoration_box),
                            );
                            let outlined_value_source = remember(MutableInteractionSource::new);
                            let s = outlined_value_source.as_interaction_source();
                            let outlined_box: DecorationBox = Rc::new(move |inner: &dyn Fn()| {
                                named_args!(OutlinedTextFieldDefaults.DecorationBox(
                                    "dört",
                                    inner,
                                    true,
                                    true,
                                    VisualTransformation::None,
                                    s.clone(),
                                    supporting_text = text_slot("alt"),
                                ));
                            });
                            BasicTextFieldString(
                                "dört",
                                |_| {},
                                modifier = Modifier.test_tag("dört"),
                                interaction_source = Some(outlined_value_source.clone()),
                                decoration_box = Some(outlined_box),
                            );
                            let container_source = remember(MutableInteractionSource::new);
                            TextFieldDefaults.Container(
                                true,
                                false,
                                container_source.as_interaction_source(),
                                modifier = Modifier.size_wh(100.dp(), 30.dp()).test_tag("kap"),
                                unfocused_indicator_line_thickness = 4.dp(),
                            );
                            OutlinedTextFieldDefaults.Container(
                                true,
                                true,
                                container_source.as_interaction_source(),
                                modifier = Modifier.size_wh(100.dp(), 30.dp()).test_tag("kap2"),
                            );
                            Box(
                                modifier = Modifier.size_wh(100.dp(), 30.dp()),
                                content = || {
                                    let colors = TextFieldDefaults.colors();
                                    TextFieldDefaults.ContainerBox(
                                        true,
                                        false,
                                        container_source.as_interaction_source(),
                                        colors,
                                    );
                                },
                            );
                            Box(
                                modifier = Modifier.size_wh(100.dp(), 30.dp()),
                                content = || {
                                    OutlinedTextFieldDefaults.ContainerBox(
                                        true,
                                        false,
                                        container_source.as_interaction_source(),
                                    );
                                },
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 1000),
    );
    rule.wait_for_idle();
    let cs = scheme();
    let c = rule.draw();
    // The decorated ones as the Material fields are.
    assert!(near(bounds(&rule, "bir").height(), 56.0, 0.5) && near(drawn(&c, "Dolu")[0].1, 16.0, 0.01));
    assert!(near(bounds(&rule, "iki").height(), 56.0, 0.5) && drawn(&c, "yok").is_empty());
    assert!(near(bounds(&rule, "üç").height(), 56.0, 0.5) && !drawn(&c, "Kutu").is_empty());
    assert!(near(bounds(&rule, "dört").height(), 76.0, 0.5) && near(drawn(&c, "alt")[0].1, 12.0, 0.01));
    // The containers: the filled one's indicator 4 dp, the outlined one's border in error.
    let kap = bounds(&rule, "kap");
    assert!((0..c.paths.len()).any(|i| c.path_colors[i] == cs.on_surface_variant
        && near(c.paths[i].bottom, kap.bottom, 0.5)
        && near(c.paths[i].height(), 4.0, 0.01)));
    assert!(has(&c.stroke_colors, cs.error));
    // Focused, the filled decorated one's label minimizes.
    tap(&rule, bounds(&rule, "bir").center());
    settle(&rule);
    assert!(near(drawn(&rule.draw(), "Dolu")[0].1, 12.0, 0.01));
}

/// TextFieldLabelPosition: Compose's equality and toString, the default attached at the start.
fn the_label_position_is_compared_and_printed_as_compose_s() {
    let attached = TextFieldLabelPosition::default();
    assert!(attached == TextFieldLabelPosition::attached(false, Alignment::Start, Alignment::Start));
    assert!(attached != TextFieldLabelPosition::attached(true, Alignment::Start, Alignment::Start));
    assert!(!attached.is_above() && TextFieldLabelPosition::above(Alignment::End).is_above());
    assert!(
        TextFieldLabelPosition::above(Alignment::End) == TextFieldLabelPosition::Above { alignment: Alignment::End }
    );
    assert_eq!(
        format!("{attached:?}"),
        "Attached(alwaysMinimize=false, minimizedAlignment=Horizontal(bias=-1), expandedAlignment=Horizontal(bias=-1))"
    );
    assert_eq!(
        format!("{:?}", TextFieldLabelPosition::above(Alignment::CenterHorizontally)),
        "Above(alignment=Horizontal(bias=0))"
    );
}

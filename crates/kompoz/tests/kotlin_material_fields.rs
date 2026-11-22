//! Material's text fields in Kotlin's syntax, from a `.kt` file (kompoz_file!): material3 1.5's TextFieldSamples.kt
//! (the state-based fields — filled, outlined, secure —, a label, placeholder, prefix and suffix, supporting text, an
//! error state, icons, input and output transformations, an initial selection, a dense content padding, a text area,
//! and BasicTextField decorated by `TextFieldDefaults.decorator(…)` / `DecorationBox(…)` on their containers); the C++
//! Kompoz's own Compose code (kompozable_tests.cpp's `text_as_compose_writes_it`, 2346-2372;
//! `text_field_transformations_as_compose_writes_them`, 2899-2919); and what the samples leave out (the value-based
//! overloads, the label above, the theme's shape, colors, the secure fields, `Modifier.indicatorLine`). NamedHeads'
//! TextField (`required_arguments_by_name`) is in kotlin_buttons.rs, beside its TextButton; `the_users_add_text_dialog`
//! (249-380, AddTextDialog's TextField) in kotlin_lists_sheets.rs, beside its Dialog. Density 1: dp are px.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::input::PointerType;
use kompoz::ui::semantics::SemanticsProperties;
use kompoz::ui::text::remember_text_measurer;
use std::rc::Rc;

kompoz_file!("tests/kt/material_fields.kt");

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A mouse's click at `p` (a finger's tap on a field would show its cursor handle).
fn click(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Mouse);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

fn type_text(rule: &KompozTestRule, text: &str) {
    assert!(rule.host().text_input(text), "no input session open");
    rule.wait_for_idle();
}

/// The text a field holds (its EditableText).
fn shown(rule: &KompozTestRule, tag: &str) -> String {
    let node = rule.on_node_with_tag(tag).fetch_semantics_node();
    node.config().get(SemanticsProperties::EditableText).map(|t| t.text().to_string()).unwrap_or_default()
}

fn drawn(rule: &KompozTestRule, text: &str) -> bool {
    rule.drawn_texts().iter().any(|t| t == text)
}

fn in_theme(content: impl Fn() + 'static) -> KompozTestRule {
    let content = Rc::new(content);
    KompozTestRule::new(kompozable_lambda!(move || {
        let content = content.clone();
        MaterialTheme(content = move || content());
    }))
}

/// The samples' plain fields: 56 dp, the outlined one 8 dp more for its floated label, the supporting text below,
/// the dense one 48 dp; an initial selection over all the text; the text area 120 dp tall; typing shows the text.
fn text_field_samples() {
    let rule = in_theme(kompozable_lambda!(|| {
        Column(
            content = || {
                SimpleTextFieldSample();
                SimpleOutlinedTextFieldSample();
                TextFieldWithSupportingText();
                DenseTextFieldContentPadding();
                TextFieldWithInitialValueAndSelection();
                OutlinedTextFieldWithInitialValueAndSelection();
                TextArea();
            },
        );
    }));
    assert_eq!(bounds(&rule, "simple").height(), 56.0);
    assert_eq!(bounds(&rule, "outlined").height(), 64.0);
    assert!(bounds(&rule, "supporting").height() > 56.0);
    // (its own least height given, the default least one doesn't apply: 4 + 16 + 24 + 4)
    assert_eq!(bounds(&rule, "dense").height(), 48.0);
    assert_eq!(bounds(&rule, "area").height(), 120.0);
    for tag in ["initial", "initial-outlined"] {
        let node = rule.on_node_with_tag(tag).fetch_semantics_node();
        assert_eq!(node.config().get(SemanticsProperties::TextSelectionRange), Some(TextRange::new(0, 12)), "{tag}");
        assert_eq!(shown(&rule, tag), "Initial text");
    }
    let texts = rule.drawn_texts();
    assert!(texts.iter().filter(|t| *t == "Label").count() >= 6 && texts.iter().any(|t| t.starts_with("Supporting")));
    click(&rule, bounds(&rule, "simple").center());
    type_text(&rule, "merhaba");
    assert!(drawn(&rule, "merhaba"));
}

/// TextFieldWithTransformations: digits only, at most 10 (a letter, or a change past 10, reverts the change), shown
/// as (XXX) XXX-XXXX by the output transformation; TextFieldWithIcons: the trailing IconButton clears the field.
fn transformation_and_icon_samples() {
    let rule = in_theme(kompozable_lambda!(|| {
        Column(
            content = || {
                TextFieldWithTransformations();
                TextFieldWithIcons();
            },
        );
    }));
    click(&rule, bounds(&rule, "phone").center());
    type_text(&rule, "555");
    type_text(&rule, "x");
    type_text(&rule, "12345678");
    type_text(&rule, "1234567");
    type_text(&rule, "8");
    // (the letter refused; "12345678" would make 11, past maxLength(10): refused whole, as Compose's MaxLengthFilter
    // reverts all the change; the 11th digit refused)
    let node = rule.on_node_with_tag("phone").fetch_semantics_node();
    let input = node.config().get(SemanticsProperties::InputText).map(|t| t.text().to_string());
    assert_eq!(input.as_deref(), Some("5551234567"));
    assert_eq!(shown(&rule, "phone"), "(555) 123-4567");
    assert!(drawn(&rule, "(555) 123-4567"));
    click(&rule, bounds(&rule, "icons").center());
    type_text(&rule, "silinecek");
    assert_eq!(shown(&rule, "icons"), "silinecek");
    rule.on_node_with_content_description("Clear text").perform_click();
    assert_eq!(shown(&rule, "icons"), "");
}

/// TextFieldWithPlaceholder and TextFieldWithPrefixAndSuffix: empty and unfocused, the label expanded and the
/// placeholder, prefix and suffix hidden; the sample's Checkbox ticked, always minimized
/// (`TextFieldLabelPosition.Attached(alwaysMinimize = …)`), all shown. PasswordTextField: what is typed hidden (the
/// last character shown for a moment), shown by the trailing IconButton.
fn placeholder_prefix_suffix_and_password_samples() {
    let rule = in_theme(kompozable_lambda!(|| TextFieldWithPlaceholder()));
    assert!(!drawn(&rule, "example@gmail.com") && drawn(&rule, "Email"));
    rule.on_node_with(kompoz::test::has_role(kompoz::ui::semantics::Role::Checkbox), false)
        .assert_is_off()
        .perform_click();
    rule.advance_time_by(600);
    assert!(drawn(&rule, "example@gmail.com"));
    rule.on_node_with(kompoz::test::has_role(kompoz::ui::semantics::Role::Checkbox), false).assert_is_on();

    let rule = in_theme(kompozable_lambda!(|| TextFieldWithPrefixAndSuffix()));
    assert!(!drawn(&rule, "www."));
    rule.on_node_with(kompoz::test::has_role(kompoz::ui::semantics::Role::Checkbox), false)
        .assert_is_off()
        .perform_click();
    rule.advance_time_by(600);
    let texts = rule.drawn_texts();
    assert!(["www.", ".com", "google"].iter().all(|t| texts.contains(&t.to_string())), "{texts:?}");

    let rule = in_theme(kompozable_lambda!(|| PasswordTextField()));
    click(&rule, bounds(&rule, "password").center());
    type_text(&rule, "gizli");
    // (RevealLastTyped: the last character typed shown for a moment, then hidden)
    rule.advance_time_by(2000);
    assert!(drawn(&rule, "•••••"), "{:?}", rule.drawn_texts());
    named_args!(rule.on_node_with_content_description("Show password", use_unmerged_tree = true)).perform_click();
    rule.wait_for_idle();
    assert!(drawn(&rule, "gizli"));
    named_args!(rule.on_node_with_content_description("Hide password", use_unmerged_tree = true)).assert_exists();
}

/// TextFieldWithErrorState: past 10 characters (validated by a `snapshotFlow` of the text) the field is in error —
/// its label "Username*", the message and the count in its supporting text, told to accessibility as its error —,
/// and the semantics block's max text length.
fn error_state_sample() {
    let rule = in_theme(kompozable_lambda!(|| TextFieldWithErrorState()));
    let node = rule.on_node_with_tag("error").fetch_semantics_node();
    assert_eq!(node.config().get(SemanticsProperties::MaxTextLength), Some(10));
    assert!(drawn(&rule, "Username") && drawn(&rule, "Limit: 0/10"));
    click(&rule, bounds(&rule, "error").center());
    type_text(&rule, "kullanıcıadı");
    assert!(drawn(&rule, "Username*") && drawn(&rule, "Text input too long") && drawn(&rule, "Limit: 12/10"));
    let node = rule.on_node_with_tag("error").fetch_semantics_node();
    assert_eq!(node.config().get(SemanticsProperties::Error).as_deref(), Some("Text input too long"));
    rule.on_node_with_tag("error").perform_text_clearance();
    rule.wait_for_idle();
    assert!(drawn(&rule, "Username") && !drawn(&rule, "Text input too long"));
}

/// TextFieldWithHideKeyboardOnImeAction: Done runs the handler, which hides the keyboard (LocalSoftwareKeyboardController);
/// the field keeps the focus.
fn hide_keyboard_sample() {
    /// A platform counting the keyboard's hiding.
    #[derive(Default)]
    struct Keyboard(std::cell::Cell<i32>);
    impl kompoz::ui::platform::PlatformCallbacks for Keyboard {
        fn hide_software_keyboard(&self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let rule = in_theme(kompozable_lambda!(|| TextFieldWithHideKeyboardOnImeAction()));
    let keyboard = Rc::new(Keyboard::default());
    rule.host().set_callbacks(keyboard.clone());
    click(&rule, bounds(&rule, "hide").center());
    let hidden = keyboard.0.get();
    rule.on_node_with_tag("hide").perform_ime_action();
    rule.wait_for_idle();
    assert!(keyboard.0.get() > hidden);
    rule.on_node_with_tag("hide").assert_is_focused();
}

fn value_text_fields() {
    let rule = in_theme(kompozable_lambda!(|| ValueTextFields()));
    click(&rule, bounds(&rule, "string").center());
    type_text(&rule, "Ada");
    rule.on_node_with_text("text=Ada").assert_exists();
    click(&rule, bounds(&rule, "value").center());
    type_text(&rule, "cd");
    // (more than 3 characters: in error, told to accessibility)
    let node = rule.on_node_with_tag("value").fetch_semantics_node();
    assert!(node.config().get(SemanticsProperties::Error).is_some());
}

/// The four custom fields (state-based with a decorator, value-based with a DecorationBox; filled and outlined):
/// 56 dp; the filled ones' indicator 2 dp unfocused and 4 dp in blue focused, the text dark gray; the outlined ones'
/// border on a rectangle.
fn decorated_basic_text_fields() {
    let rule = in_theme(kompozable_lambda!(|| {
        Column(
            content = || {
                CustomTextFieldUsingDecorator();
                CustomOutlinedTextFieldUsingDecorator();
                CustomTextFieldBasedOnDecorationBox();
                CustomOutlinedTextFieldBasedOnDecorationBox();
            },
        );
    }));
    for tag in ["decorated", "outlined-decorated", "box-decorated", "boxed"] {
        assert_eq!(bounds(&rule, tag).height(), 56.0, "{tag}");
    }
    for tag in ["decorated", "box-decorated"] {
        let field = bounds(&rule, tag);
        let line = |color: Color| {
            let c = rule.draw();
            (0..c.paths.len())
                .find(|&i| c.path_colors[i] == color && (c.paths[i].bottom - field.bottom).abs() < 0.5)
                .map(|i| c.paths[i].height())
        };
        assert_eq!(line(light_color_scheme().on_surface_variant), Some(2.0), "{tag}");
        click(&rule, field.center());
        rule.advance_time_by(600);
        assert_eq!(line(Color::Blue), Some(4.0), "{tag}");
        type_text(&rule, "x");
        let c = rule.draw();
        let x = c.texts.iter().position(|t| t == "x").unwrap();
        assert_eq!(c.colors[x], Color::DarkGray, "{tag}");
        rule.on_node_with_tag(tag).perform_text_clearance();
    }
    // (the outlined ones' border: square corners — RectangleShape —, in blue once focused)
    let field = bounds(&rule, "outlined-decorated");
    let border = |c: &kompoz::test::RecordingCanvas| {
        let i = c.strokes.iter().position(|s| (s.top - field.top).abs() < 1.0).expect("its border");
        (c.stroke_colors[i], c.stroke_corners[i].top_left)
    };
    assert_eq!(border(&rule.draw()).1, 0.0);
    click(&rule, field.center());
    rule.advance_time_by(600);
    assert_eq!(border(&rule.draw()).0, Color::Blue);
}

/// kompozable_tests.cpp `text_as_compose_writes_it`: the annotated text (a clickable link, a bold run), the email
/// field's label and supporting text, the message field's placeholder, the selectable text.
fn text_as_compose_writes_it() {
    let rule = in_theme(kompozable_lambda!(|| TextForms()));
    rule.on_node(kompoz::test::has_text("accept the terms now", true, false)).assert_exists();
    for text in ["Email", "Never shared", "Message", "Order KMP-1"] {
        rule.on_node_with_text(text).assert_exists();
    }
    // (the measured "7%" drawn in the Canvas)
    assert!(drawn(&rule, "7%"));
}

/// kompozable_tests.cpp `text_field_transformations_as_compose_writes_them`: Material's OutlinedTextField taking the
/// state and both transformations: a letter refused, the 11th digit past the limit; shown with its separators; the
/// Button lets the focus go.
fn text_field_transformations_as_compose_writes_them() {
    let rule = in_theme(kompozable_lambda!(|| Phone()));
    rule.on_node_with_text("değer 5551234567 · 12 · ab").assert_exists();
    assert_eq!(shown(&rule, "telefon"), "555 123 45 67");
    rule.on_node_with_tag("telefon").perform_text_input("x9");
    rule.on_node(kompoz::test::has_text("değer 5551234567", true, false)).assert_exists();
    rule.on_node_with_tag("telefon").assert_is_focused();
    rule.on_node_with_text("Odağı bırak").perform_click();
    rule.on_node_with_tag("telefon").assert_is_not_focused();
}

/// The label above the field (TextFieldLabelPosition.Above: the field keeps its least height, the label over it), a
/// prefix and suffix, a supporting text only in error, the theme's field shape; an outlined field's colors and shape;
/// the secure fields hiding their text.
fn labels_shapes_and_secure_fields() {
    let rule = in_theme(kompozable_lambda!(|| LabelsAndShapes()));
    let field = bounds(&rule, "above");
    let label = named_args!(rule.on_node_with_text("Tutar", use_unmerged_tree = true)).get_unclipped_bounds_in_root();
    // (the field's node holds the label above its 56 dp container: the label at its top, over the container)
    assert!(label.top == field.top && label.bottom <= field.bottom - 56.0, "{label:?} {field:?}");
    assert!(!drawn(&rule, "En çok 4 hane"));
    click(&rule, field.center());
    type_text(&rule, "12345");
    assert!(drawn(&rule, "En çok 4 hane") && drawn(&rule, "₺") && drawn(&rule, ",00"));
    let node = rule.on_node_with_tag("above").fetch_semantics_node();
    assert!(node.config().get(SemanticsProperties::Error).is_some());
    // (the outlined field's red border, 12 dp corners; the secure fields' bullets)
    let c = rule.draw();
    let red = c.stroke_colors.iter().position(|&s| s == Color::Red).expect("the red border");
    assert_eq!(c.stroke_corners[red].top_left, 12.0);
    assert!(rule.drawn_texts().iter().filter(|t| *t == "••••••").count() == 2, "{:?}", rule.drawn_texts());
    let secure = rule.on_node_with_tag("outlined-secure").fetch_semantics_node();
    assert!(secure.config().contains(SemanticsProperties::Password));
}

/// `Modifier.indicatorLine` under a BasicTextField of one's own: a 3 dp line in the given unfocused color at the
/// bottom of what it is on (the field and its 8 dp padding: the tag's node is the field's, inside the padding).
fn indicator_line_on_a_field_of_ones_own() {
    let rule = in_theme(kompozable_lambda!(|| IndicatorLineField()));
    let field = bounds(&rule, "line");
    let c = rule.draw();
    let line = (0..c.paths.len()).find(|&i| c.path_colors[i] == Color::Magenta);
    let line = line.map(|i| c.paths[i]).expect("the indicator line");
    assert!(
        (line.bottom - (field.bottom + 8.0)).abs() < 0.5 && (line.height() - 3.0).abs() < 0.5,
        "{line:?} {field:?}"
    );
    assert!((line.left - (field.left - 8.0)).abs() < 0.5 && (line.right - (field.right + 8.0)).abs() < 0.5);
}

fn main() {
    common::run(&[
        ("text_field_samples", text_field_samples),
        ("transformation_and_icon_samples", transformation_and_icon_samples),
        ("placeholder_prefix_suffix_and_password_samples", placeholder_prefix_suffix_and_password_samples),
        ("error_state_sample", error_state_sample),
        ("hide_keyboard_sample", hide_keyboard_sample),
        ("value_text_fields", value_text_fields),
        ("decorated_basic_text_fields", decorated_basic_text_fields),
        ("text_as_compose_writes_it", text_as_compose_writes_it),
        ("text_field_transformations_as_compose_writes_them", text_field_transformations_as_compose_writes_them),
        ("labels_shapes_and_secure_fields", labels_shapes_and_secure_fields),
        ("indicator_line_on_a_field_of_ones_own", indicator_line_on_a_field_of_ones_own),
    ]);
}

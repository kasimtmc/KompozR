//! The state-based text fields in Kotlin's syntax, as a Compose user writes them: a single-line BasicTextField with
//! its keyboard's action key and a handler of it (`onKeyboardAction = { performDefaultAction -> … }`), one decorated
//! by a `TextFieldDecorator { innerTextField -> … }`, a BasicSecureTextField hiding its text, and the C++ Kompoz's
//! fields (kompozable_tests.cpp 2343-2366's) ported onto them — through `kompoz!`; and in a `.kt` file
//! (kompoz_file!) a `KeyboardActionHandler { … }` moving the focus on, a secure field with an obfuscation character
//! of its own and a lambda as its decorator, and the phone field of 2862-2893. The C++ composed Material's
//! TextField/OutlinedTextField: their decoration (container, label, indicator) is stage 12's, so a decorator of the
//! test's own stands for it. The same behaviour crates/kompoz-foundation/tests/text_field.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::input::PointerType;
use kompoz::ui::semantics::SemanticsProperties;
use std::cell::RefCell;
use std::rc::Rc;

/// A node's bounds in the root (px; the rule's density is 1).
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).fetch_semantics_node().bounds_in_root()
}

/// Whether `text` is drawn (a secure field draws its mask; its EditableText is its text, as Compose's output
/// text, accessibility knowing it is a password).
fn drawn(rule: &KompozTestRule, text: &str) -> bool {
    rule.drawn_texts().iter().any(|t| t == text)
}

/// The text a field holds (its EditableText).
fn shown(rule: &KompozTestRule, tag: &str) -> String {
    let node = rule.on_node_with_tag(tag).fetch_semantics_node();
    node.config().get(SemanticsProperties::EditableText).map(|t| t.text().to_string()).unwrap_or_default()
}

/// The text a node says.
fn text_of(rule: &KompozTestRule, tag: &str) -> String {
    let node = rule.on_node_with_tag(tag).fetch_semantics_node();
    node.config().get(SemanticsProperties::Text).map(|t| t[0].text().to_string()).unwrap_or_default()
}

fn is_focused(rule: &KompozTestRule, tag: &str) -> bool {
    rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::Focused) == Some(true)
}

/// A finger's tap at `p`.
fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

/// What the input method commits to the focused field.
fn type_text(rule: &KompozTestRule, text: &str) {
    assert!(rule.host().text_input(text), "no input session open");
    rule.wait_for_idle();
}

kompoz! {
    @Composable
    fun Search(searched: (String) -> Unit) {
        val query = rememberTextFieldState()
        Column {
            BasicTextField(
                state = query,
                modifier = Modifier.width(200.dp).testTag("ara"),
                lineLimits = TextFieldLineLimits.SingleLine,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                onKeyboardAction = { performDefaultAction ->
                    searched("${query.text}")
                    performDefaultAction()
                },
            )
            Text("aranan: ${query.text}", modifier = Modifier.testTag("aranan"))
        }
    }

    @Composable
    fun Labeled() {
        val state = rememberTextFieldState("abc")
        BasicTextField(
            state = state,
            modifier = Modifier.testTag("alan"),
            textStyle = TextStyle(fontSize = 16.sp),
            decorator = TextFieldDecorator { innerTextField ->
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(24.dp).testTag("simge"))
                    Box(Modifier.padding(start = 8.dp).testTag("iç")) { innerTextField() }
                }
            },
        )
    }

    @Composable
    fun Password() {
        val password = rememberTextFieldState("gizli")
        BasicSecureTextField(
            state = password,
            modifier = Modifier.width(200.dp).testTag("şifre"),
            textObfuscationMode = TextObfuscationMode.Hidden,
        )
    }

    // kompozable_tests.cpp 2343-2366's text fields: an email field on one line with its label and supporting text,
    // a message field of at most four lines with its placeholder. The C++'s OutlinedTextFields are Material's
    // (stage 12's): BasicTextFields decorated as they are stand for them; its value-and-callback email field is
    // ported onto a state, the overload WP2b brings.
    @Composable
    fun TextForms() {
        val email = rememberTextFieldState()
        val message = rememberTextFieldState()
        Column {
            BasicTextField(
                state = email,
                modifier = Modifier.width(200.dp).testTag("eposta"),
                lineLimits = TextFieldLineLimits.SingleLine,
                decorator = TextFieldDecorator { innerTextField ->
                    Column {
                        Text("Email")
                        innerTextField()
                        Text("Never shared")
                    }
                },
            )
            BasicTextField(
                state = message,
                modifier = Modifier.width(200.dp).testTag("mesaj"),
                textStyle = TextStyle(fontSize = 16.sp),
                lineLimits = TextFieldLineLimits.MultiLine(maxHeightInLines = 4),
                decorator = TextFieldDecorator { innerTextField ->
                    Box {
                        if (message.text.isEmpty()) Text("Message")
                        innerTextField()
                    }
                },
            )
        }
    }
}

/// A single-line field with the Done key: focused by its RequestFocus action, typed in, its keyboard action's
/// handler run (by the test rule's `performImeAction`) with the text typed.
fn a_single_line_field_and_its_keyboard_action() {
    let searched = Rc::new(RefCell::new(Vec::new()));
    let s = searched.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Search(move |q| s.borrow_mut().push(q));
    }));
    rule.on_node_with_tag("ara").request_focus();
    rule.wait_for_idle();
    assert!(is_focused(&rule, "ara"));
    type_text(&rule, "kedi");
    assert_eq!((shown(&rule, "ara"), text_of(&rule, "aranan")), ("kedi".to_string(), "aranan: kedi".to_string()));
    rule.on_node_with_tag("ara").perform_ime_action();
    assert_eq!(*searched.borrow(), ["kedi"]);
    // (Done's default action hides the keyboard; the field keeps the focus, as Compose's does)
    assert!(is_focused(&rule, "ara"));
}

/// A decorator's icon before the inner field: the text sits after it, and a tap on the icon focuses the field (a
/// decoration is the field's touch target).
fn a_decorated_field() {
    let rule = KompozTestRule::new(Labeled);
    let (field, icon, inner) = (bounds(&rule, "alan"), bounds(&rule, "simge"), bounds(&rule, "iç"));
    assert_eq!((icon.left, icon.width()), (field.left, 24.0));
    assert_eq!(inner.left, icon.right + 8.0);
    // (16 sp: 8 px a character, 20 px a line; the row as tall as the icon)
    assert!(inner.width() >= 24.0 && (inner.height(), field.height()) == (20.0, 24.0), "{inner:?} {field:?}");
    tap(&rule, icon.center());
    assert!(is_focused(&rule, "alan"));
    // The tap kept within the inner field: the caret at its start, nearest the icon (Compose's
    // `coercedInVisibleBoundsOfInputText`).
    type_text(&rule, "d");
    assert_eq!(shown(&rule, "alan"), "dabc");
}

/// A secure field in the Hidden mode draws bullets for its text, typed or not.
fn a_secure_field_hides_its_text() {
    let rule = KompozTestRule::new(Password);
    assert!(drawn(&rule, "•••••"));
    rule.on_node_with_tag("şifre").perform_text_input("12");
    assert!(drawn(&rule, "•••••••"));
    assert!(rule.on_node_with_tag("şifre").fetch_semantics_node().config().contains(SemanticsProperties::Password));
}

/// kompozable_tests.cpp 2343-2366 (`text_as_compose_writes_it`)'s fields: their label, supporting text and
/// placeholder shown; typing hides the placeholder; a single line ignores Enter's newline (shown as a space), the
/// message grows with its lines up to four.
fn text_fields_as_compose_writes_them() {
    let rule = KompozTestRule::new(TextForms);
    for text in ["Email", "Never shared", "Message"] {
        assert!(rule.on_node_with_text(text).exists(), "{text}");
    }
    rule.on_node_with_tag("eposta").perform_text_input("a@b.c");
    assert_eq!(shown(&rule, "eposta"), "a@b.c");
    let one_line = bounds(&rule, "mesaj").height();
    assert_eq!(one_line, 20.0);
    rule.on_node_with_tag("mesaj").perform_text_input("1\n2");
    assert!(!rule.on_node_with_text("Message").exists());
    assert_eq!(bounds(&rule, "mesaj").height(), 40.0);
    rule.on_node_with_tag("mesaj").perform_text_input("\n3\n4\n5\n6");
    assert_eq!((shown(&rule, "mesaj"), bounds(&rule, "mesaj").height()), ("1\n2\n3\n4\n5\n6".to_string(), 80.0));
}

kompoz_file!("tests/kt/text_field.kt");

/// A `KeyboardActionHandler { … }` on a field whose action is Next: its default action moves the focus to the
/// password field, whose characters are its own obfuscation character.
fn a_keyboard_action_handler_moves_the_focus_on_from_a_kt_file() {
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let s = submitted.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Login(move |user, password| s.borrow_mut().push((user, password)));
    }));
    rule.on_node_with_tag("kullanıcı").perform_text_input("ali");
    rule.on_node_with_tag("kullanıcı").perform_ime_action();
    assert!(is_focused(&rule, "parola") && !is_focused(&rule, "kullanıcı"));
    type_text(&rule, "1234");
    assert!(drawn(&rule, "****"));
    rule.on_node_with_tag("parola").perform_ime_action();
    assert_eq!(*submitted.borrow(), [("ali".to_string(), "1234".to_string())]);
    // The decorator (a lambda given as one) pads the inner field by 4 px.
    let (frame, inner) = (bounds(&rule, "parola"), bounds(&rule, "parola-iç"));
    assert_eq!((inner.left - frame.left, inner.top - frame.top), (4.0, 4.0));
}

/// kompozable_tests.cpp 2862-2893 (`text_field_transformations_as_compose_writes_them`) on BasicTextField: the
/// letter typed refused by `InputTransformation.byValue`, the digit by `maxLength(10)`; the output transformation's
/// separators shown; the focus let go by LocalFocusManager.
fn the_phone_field_from_a_kt_file() {
    let rule = KompozTestRule::new(Phone);
    assert!(rule.on_node_with_text("değer 5551234567 · 12 · ab").exists());
    assert_eq!(shown(&rule, "telefon"), "555 123 45 67");
    rule.on_node_with_tag("telefon").perform_text_input("x9");
    assert!(rule.on_node(kompoz::test::has_text("değer 5551234567", true, false)).exists());
    assert!(is_focused(&rule, "telefon"));
    rule.on_node_with_text("Odağı bırak").perform_click();
    assert!(!is_focused(&rule, "telefon"));
    rule.on_node_with_tag("telefon").perform_text_replacement("555");
    rule.on_node_with_tag("telefon").perform_text_input("12a");
    assert_eq!(shown(&rule, "telefon"), "555");
    rule.on_node_with_tag("telefon").perform_text_input("12");
    assert_eq!(shown(&rule, "telefon"), "555 12");
    assert!(rule.on_node_with_text("değer 55512 · 12 · ab").exists());
}

tests!(
    a_single_line_field_and_its_keyboard_action,
    a_decorated_field,
    a_secure_field_hides_its_text,
    text_fields_as_compose_writes_them,
    a_keyboard_action_handler_moves_the_focus_on_from_a_kt_file,
    the_phone_field_from_a_kt_file,
);

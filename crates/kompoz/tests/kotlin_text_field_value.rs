//! The value-based text fields in Kotlin's syntax, as a Compose user writes them: `var text by remember {
//! mutableStateOf("") }` and `BasicTextField(value = text, onValueChange = { text = it })` (the String overload,
//! chosen by the value's type), a `TextFieldValue` one keeping its selection (that overload), a password shown by
//! `PasswordVisualTransformation()` or not (`VisualTransformation.None`), a decoration box around the inner field,
//! and the state overload beside them — through `kompoz!`; and in a `.kt` file (kompoz_file!) a sign-in form of
//! String fields with keyboard options and actions (its password masked by `PasswordVisualTransformation(mask =
//! '*')`), and a TextFieldValue field the app selects all of (`note.copy(selection = TextRange(0, …))`). The same
//! behaviour crates/kompoz-foundation/tests/text_field_value.rs checks in Rust.

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

/// Whether `text` is drawn.
fn drawn(rule: &KompozTestRule, text: &str) -> bool {
    rule.drawn_texts().iter().any(|t| t == text)
}

/// The text a field shows (its EditableText: the transformed text).
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
    fun Name(changed: (String) -> Unit) {
        var text by remember { mutableStateOf("") }
        Column {
            BasicTextField(
                value = text,
                onValueChange = {
                    text = it
                    changed(it)
                },
                modifier = Modifier.width(200.dp).testTag("ad"),
                textStyle = TextStyle(fontSize = 16.sp),
                singleLine = true,
            )
            Text("ad: $text", modifier = Modifier.testTag("ad-metni"))
        }
    }

    @Composable
    fun Selected() {
        var value by remember { mutableStateOf(TextFieldValue(text = "merhaba", selection = TextRange(0, 3))) }
        Column {
            BasicTextField(
                value = value,
                onValueChange = { value = it },
                modifier = Modifier.width(200.dp).testTag("seçili"),
                textStyle = TextStyle(fontSize = 16.sp),
            )
            Text("seçim ${value.selection.start}..${value.selection.end} ${value.text}", modifier = Modifier.testTag("seçim"))
        }
    }

    @Composable
    fun Secret() {
        var password by remember { mutableStateOf("gizli") }
        var visible by remember { mutableStateOf(false) }
        Column {
            BasicTextField(
                value = password,
                onValueChange = { password = it },
                modifier = Modifier.width(200.dp).testTag("parola"),
                singleLine = true,
                visualTransformation = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
            )
            BasicText("göster", modifier = Modifier.testTag("göster").clickable { visible = !visible })
        }
    }

    @Composable
    fun Decorated() {
        var text by remember { mutableStateOf("abc") }
        val state = rememberTextFieldState("durum")
        Column {
            BasicTextField(
                value = text,
                onValueChange = { text = it },
                modifier = Modifier.testTag("süslü"),
                textStyle = TextStyle(fontSize = 16.sp),
                decorationBox = { innerTextField ->
                    Row(modifier = Modifier.padding(4.dp)) {
                        Box(modifier = Modifier.size(24.dp).testTag("simge"))
                        Spacer(modifier = Modifier.width(8.dp))
                        Box(modifier = Modifier.testTag("iç")) { innerTextField() }
                    }
                },
            )
            BasicTextField(state = state, modifier = Modifier.testTag("durum"))
        }
    }
}

/// `var text by remember { mutableStateOf("") }` and `BasicTextField(value = text, onValueChange = { text = it })`:
/// the String overload; what is typed comes back as the text, the app hearing each text once.
fn a_string_field_as_compose_writes_it() {
    let changes = Rc::new(RefCell::new(Vec::<String>::new()));
    let c = changes.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        Name(move |it| c.borrow_mut().push(it));
    }));
    tap(&rule, bounds(&rule, "ad").center());
    assert!(is_focused(&rule, "ad"));
    type_text(&rule, "ali");
    rule.host().send_edit_commands(&[kompoz::ui::text_input::EditCommand::SetSelection { start: 0, end: 0 }]);
    rule.wait_for_idle();
    assert_eq!((shown(&rule, "ad"), text_of(&rule, "ad-metni")), ("ali".to_string(), "ad: ali".to_string()));
    // (the caret moved by the input method: no new text, not heard)
    assert_eq!(*changes.borrow(), ["ali"]);
}

/// A TextFieldValue field: the app's selection shown and kept, what the user selects coming back with the text.
fn a_text_field_value_field_keeps_its_selection() {
    let rule = KompozTestRule::new(Selected);
    assert_eq!(text_of(&rule, "seçim"), "seçim 0..3 merhaba");
    let config = rule.on_node_with_tag("seçili").fetch_semantics_node().config().clone();
    assert_eq!(config.get(SemanticsProperties::TextSelectionRange), Some(TextRange::new(0, 3)));
    rule.on_node_with_tag("seçili").perform_text_input_selection(TextRange::new(2, 5), true);
    assert_eq!(text_of(&rule, "seçim"), "seçim 2..5 merhaba");
    rule.on_node_with_tag("seçili").perform_text_input("X");
    assert_eq!(text_of(&rule, "seçim"), "seçim 3..3 meXba");
}

/// A password's mask (`PasswordVisualTransformation()`), shown as it is with `VisualTransformation.None`: the
/// text kept; accessibility told it is a password while masked.
fn a_password_shown_or_not() {
    let rule = KompozTestRule::new(Secret);
    assert!(drawn(&rule, "•••••") && shown(&rule, "parola") == "•••••");
    assert!(rule.on_node_with_tag("parola").fetch_semantics_node().config().contains(SemanticsProperties::Password));
    rule.on_node_with_tag("parola").perform_text_input("1");
    assert!(drawn(&rule, "••••••"));
    rule.on_node_with_tag("göster").perform_click();
    // (typed at the caret the field started with: the text's start, Compose's `TextFieldValue(text = value)`)
    assert!(drawn(&rule, "1gizli"));
    assert!(!rule.on_node_with_tag("parola").fetch_semantics_node().config().contains(SemanticsProperties::Password));
}

/// A decoration box's icon before the inner field (`decorationBox = { innerTextField -> … }`): the text after it, a
/// tap on the decoration focusing the field; the state overload beside it, reached by `state =`.
fn a_decoration_box_and_the_state_overload() {
    let rule = KompozTestRule::new(Decorated);
    let (icon, inner) = (bounds(&rule, "simge"), bounds(&rule, "iç"));
    assert_eq!(inner.left, icon.right + 8.0);
    tap(&rule, icon.center());
    assert!(is_focused(&rule, "süslü"));
    type_text(&rule, "d");
    assert_eq!(shown(&rule, "süslü"), "dabc");
    rule.on_node_with_tag("durum").perform_text_input("!");
    assert_eq!(shown(&rule, "durum"), "durum!");
}

kompoz_file!("tests/kt/text_field_value.kt");

/// A sign-in form from a `.kt` file: the user's name (Next moves the focus on), its password masked with a mask
/// character of its own (Done signs in); a clear button empties the name — the input method told.
fn a_sign_in_form_from_a_kt_file() {
    let signed_in = Rc::new(RefCell::new(Vec::new()));
    let s = signed_in.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        SignIn(move |user, password| s.borrow_mut().push((user, password)));
    }));
    rule.on_node_with_tag("kullanıcı").perform_text_input("ali");
    rule.on_node_with_tag("kullanıcı").perform_ime_action();
    assert!(is_focused(&rule, "parola") && !is_focused(&rule, "kullanıcı"));
    type_text(&rule, "1234");
    assert!(drawn(&rule, "****"));
    rule.on_node_with_tag("parola").perform_ime_action();
    assert_eq!(*signed_in.borrow(), [("ali".to_string(), "1234".to_string())]);
    rule.on_node_with_text("Temizle").perform_click();
    assert_eq!(shown(&rule, "kullanıcı"), "");
}

/// A TextFieldValue field the app selects all in from a `.kt` file (`value.copy(selection = TextRange(0, …))`),
/// typed over.
fn the_app_selects_in_a_value_field_from_a_kt_file() {
    let rule = KompozTestRule::new(Note);
    rule.on_node_with_tag("not").perform_text_input("!");
    rule.on_node_with_text("Tümünü seç").perform_click();
    assert_eq!(text_of(&rule, "not-seçim"), "0..7");
    rule.on_node_with_tag("not").perform_text_input("yeni");
    assert_eq!((shown(&rule, "not"), text_of(&rule, "not-seçim")), ("yeni".to_string(), "4..4".to_string()));
}

tests!(
    a_string_field_as_compose_writes_it,
    a_text_field_value_field_keeps_its_selection,
    a_password_shown_or_not,
    a_decoration_box_and_the_state_overload,
    a_sign_in_form_from_a_kt_file,
    the_app_selects_in_a_value_field_from_a_kt_file,
);

//! A text field's model in Kotlin's syntax, as a Compose user writes it (kompozable_tests.cpp 2862-2893): a state
//! remembered with its text, `InputTransformation.byValue { current, proposed -> }` chained with
//! `InputTransformation.maxLength`, an `OutputTransformation { … }` inserting separators, `state.edit { append(…);
//! replace(…) }` — through `kompoz!` and in a `.kt` file (kompoz_file!), the transformations given to Material's
//! OutlinedTextField as the C++ composed it and typed into. The same behaviour crates/kompoz-foundation/tests/text_input.rs
//! checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;

kompoz! {
    @Composable
    fun Greeting(name: String) {
        val state = rememberTextFieldState("Merhaba")
        LaunchedEffect(name) {
            state.edit {
                append(", ")
                append(name)
            }
        }
        Text("${state.text}", modifier = Modifier.testTag("selam"))
    }
}

/// The text a node shows.
fn text_of(rule: &KompozTestRule, tag: &str) -> String {
    let node = rule.on_node_with_tag(tag).fetch_semantics_node();
    node.config()
        .get(kompoz::ui::semantics::SemanticsProperties::Text)
        .map(|t| t[0].text().to_string())
        .unwrap_or_default()
}

fn a_state_edited_from_a_launched_effect() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| Greeting("dünya".to_string())));
    assert_eq!(text_of(&rule, "selam"), "Merhaba, dünya");
}

kompoz_file!("tests/kt/text_input.kt");

/// The text the phone field shows (its EditableText: the output transformation's).
fn shown(rule: &KompozTestRule) -> String {
    let node = rule.on_node_with_tag("telefon").fetch_semantics_node();
    node.config()
        .get(kompoz::ui::semantics::SemanticsProperties::EditableText)
        .map(|t| t.text().to_string())
        .unwrap_or_default()
}

fn transformations_as_compose_writes_them() {
    let rule = KompozTestRule::new(Phone);
    assert!(rule.on_node_with_text("değer 5551234567 · 12 · ab").exists());
    assert_eq!(shown(&rule), "555 123 45 67");
    rule.on_node_with_tag("telefon").perform_text_input("x9"); // the letter refused…
    rule.on_node_with_tag("telefon").perform_text_input("9"); // …the digit too: past the length's limit
    assert!(rule.on_node(kompoz::test::has_text("değer 5551234567", true, false)).exists());
    rule.on_node_with_tag("telefon").perform_text_replacement("555");
    rule.on_node_with_tag("telefon").perform_text_input("12");
    assert!(rule.on_node_with_text("değer 55512 · 12 · ab").exists());
    assert_eq!(shown(&rule), "555 12");
}

fn a_state_edited_in_code_from_a_kt_file() {
    let edits = mutable_state_of(1);
    let e = edits.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || Note(e.get())));
    assert_eq!(text_of(&rule, "not"), "not: satır 1");
    edits.set(2);
    rule.wait_for_idle();
    // "satır 1" (8 bytes: lengths are UTF-8 bytes) isn't longer than 9; "satır 1satır 2" is: its first two bytes
    // replaced.
    assert_eq!(text_of(&rule, "not"), "not: Stır 1satır 2");
}

tests!(
    a_state_edited_from_a_launched_effect,
    transformations_as_compose_writes_them,
    a_state_edited_in_code_from_a_kt_file
);

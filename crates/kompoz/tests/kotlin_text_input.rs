//! A text field's model in Kotlin's syntax, as a Compose user writes it (kompozable_tests.cpp 2862-2893): a state
//! remembered with its text, `InputTransformation.byValue { current, proposed -> }` chained with
//! `InputTransformation.maxLength`, an `OutputTransformation { … }` inserting separators, `state.edit { append(…);
//! replace(…) }` — through `kompoz!` and in a `.kt` file (kompoz_file!). The field the C++ composed (Material's
//! OutlinedTextField) is stage 12's and BasicTextField WP2b's: the test applies the transformations to the state
//! as a field does (its transformed state, the user's typing). The same behaviour
//! crates/kompoz-foundation/tests/text_input.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::foundation::text::input::internal::{TextFieldEditUndoBehavior, TransformedTextFieldState};
use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, Text};
use std::cell::RefCell;
use std::rc::Rc;

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

type Fields = (TextFieldState, InputTransformation, Rc<dyn OutputTransformation>);

/// The user typing `text` into a field of `fields`' state and transformations (what BasicTextField does with them).
fn typed(fields: &Fields, text: &str) {
    let field = TransformedTextFieldState::new(fields.0.clone(), Some(fields.1.clone()), None, Some(fields.2.clone()));
    field.replace_selected_text(text, true, TextFieldEditUndoBehavior::MergeIfPossible, true, false);
}

fn transformations_as_compose_writes_them() {
    let held: Rc<RefCell<Option<Fields>>> = Rc::new(RefCell::new(None));
    let h = held.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let h = h.clone();
        Phone(move |state, input, output| *h.borrow_mut() = Some((state, input, output)));
    }));
    assert!(rule.on_node_with_text("değer 5551234567 · 12 · ab").exists());
    let fields = held.borrow().clone().unwrap();
    let shown = TransformedTextFieldState::new(fields.0.clone(), None, None, Some(fields.2.clone()));
    assert_eq!(shown.output_text().text(), "555 123 45 67");
    typed(&fields, "x9"); // the letter refused…
    typed(&fields, "9"); // …the digit too: past the length's limit
    rule.wait_for_idle();
    assert!(rule.on_node(kompoz::test::has_text("değer 5551234567", true, false)).exists());
    fields.0.set_text_and_place_cursor_at_end("555");
    typed(&fields, "12");
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("değer 55512 · 12 · ab").exists());
    assert_eq!(shown.output_text().text(), "555 12");
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

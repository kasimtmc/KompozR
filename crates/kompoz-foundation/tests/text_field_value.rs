//! The value-based text fields — BasicTextField(value: String) and BasicTextField(value: TextFieldValue) through
//! Compose's CoreTextField engine (WP2c) — and the C++ Kompoz's value-based text field tests ported onto them (the
//! C++ had one engine; its value-based API is the reference for the round trip: the app keeps the text, the field
//! the selection and the composition). Compose's behaviour where the two differ (said where it matters).
//!
//! The texts are 16 sp in the fake backend unless said: 8 px a character, 20 px lines. Numbers in px.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_coroutines::MutableStateFlow;
use kompoz_foundation::gestures::Orientation;
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::LocalAutofillHighlightColor;
use kompoz_foundation::text::selection::SelectionHandleInfoKey;
use kompoz_foundation::text::selection::text_prepared_selection::TextPreparedSelectionState;
use kompoz_foundation::text::text_field_scroll::TextFieldScrollerPosition;
use kompoz_foundation::text::undo_manager::UndoManager;
use kompoz_foundation::text::validating_offset_mapping::filter_with_validation;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_text::input::{
    EditCommand, MapOffsets, OffsetMapping, PasswordVisualTransformation, TextFieldValue, TransformedText,
    VisualTransformation,
};
use kompoz_text::{AnnotatedString, TextRange, TextStyle};
use kompoz_ui::input::*;
use kompoz_ui::platform::{
    HapticFeedbackType, ImeAction, ImeOptions, LocalCursorBlinkEnabled, PlatformCallbacks, TextEditState,
    TextToolbarActions,
};
use kompoz_ui::semantics::*;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// The 16 sp style: 8 px a character, 20 px lines.
fn style16() -> TextStyle {
    TextStyle { font_size: 16.sp(), ..TextStyle::Default }
}

/// A platform keeping its clipboard, what it is told of text input, its toolbar and haptics.
#[derive(Default)]
struct Ime {
    clipboard: RefCell<Option<String>>,
    shown: RefCell<Vec<(bool, ImeOptions)>>,
    changed: RefCell<Vec<TextEditState>>,
    hidden: Cell<i32>,
    toolbar: RefCell<Option<(Rect, TextToolbarActions)>>,
    haptics: RefCell<Vec<HapticFeedbackType>>,
}

impl PlatformCallbacks for Ime {
    fn get_clipboard(&self) -> Option<String> {
        self.clipboard.borrow().clone()
    }
    fn set_clipboard(&self, text: &str) {
        *self.clipboard.borrow_mut() = Some(text.to_string());
    }
    fn text_input(&self, active: bool, _area: Rect, _state: &TextEditState, options: &ImeOptions) {
        self.shown.borrow_mut().push((active, options.clone()));
    }
    fn text_input_changed(&self, state: &TextEditState) {
        self.changed.borrow_mut().push(state.clone());
    }
    fn hide_software_keyboard(&self) {
        self.hidden.set(self.hidden.get() + 1);
    }
    fn show_text_toolbar(&self, rect: Rect, actions: &TextToolbarActions) {
        *self.toolbar.borrow_mut() = Some((rect, actions.clone()));
    }
    fn hide_text_toolbar(&self) {
        *self.toolbar.borrow_mut() = None;
    }
    fn haptic_feedback(&self, kind: HapticFeedbackType) {
        self.haptics.borrow_mut().push(kind);
    }
}

impl Ime {
    fn clipboard(&self) -> String {
        self.clipboard.borrow().clone().unwrap_or_default()
    }
    fn set_clip(&self, text: &str) {
        *self.clipboard.borrow_mut() = Some(text.to_string());
    }
    /// The texts the input method was told the editor has (the app's own changes).
    fn resyncs(&self) -> Vec<String> {
        self.changed.borrow().iter().map(|s| s.text.clone()).collect()
    }
    /// How many times the platform was told an input method is active.
    fn shows(&self) -> usize {
        self.shown.borrow().iter().filter(|(a, _)| *a).count()
    }
    /// Whether an input method is active as last told.
    fn active(&self) -> bool {
        self.shown.borrow().last().is_some_and(|(a, _)| *a)
    }
    fn toolbar_shown(&self) -> bool {
        self.toolbar.borrow().is_some()
    }
}

fn ime(rule: &KompozTestRule) -> Rc<Ime> {
    let p = Rc::new(Ime::default());
    rule.host().set_callbacks(p.clone());
    p
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).fetch_semantics_node().bounds_in_root()
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

fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

fn long_press(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_time_by(600);
    rule.host().pointer_up(p, -1, 0);
    rule.wait_for_idle();
}

/// Mouse clicks at `p`, `n` in a row, `modifiers` held, after a pause that ends a previous series.
fn clicks(rule: &KompozTestRule, p: Offset, n: usize, modifiers: KeyModifiers) {
    rule.advance_time_by(500);
    rule.host().set_keyboard_modifiers(modifiers);
    for _ in 0..n {
        rule.host().pointer_down(p, -1, 0, PointerType::Mouse);
        rule.host().pointer_up(p, -1, 0);
    }
    rule.host().set_keyboard_modifiers(NONE);
    rule.wait_for_idle();
}

/// The handles shown (their SelectionHandleInfo semantics).
fn handles(rule: &KompozTestRule) -> Vec<kompoz_foundation::text::selection::SelectionHandleInfo> {
    rule.all_nodes(true).iter().filter_map(|n| n.config().get(SelectionHandleInfoKey)).collect()
}

/// The cursors drawn: paths in the default cursor brush's black (the text is drawn as text).
fn cursors(canvas: &RecordingCanvas) -> Vec<Rect> {
    canvas.paths.iter().zip(&canvas.path_colors).filter(|(_, c)| **c == Color::Black).map(|(r, _)| *r).collect()
}

fn is_focused(rule: &KompozTestRule, tag: &str) -> bool {
    rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::Focused) == Some(true)
}

fn config_of(rule: &KompozTestRule, tag: &str) -> SemanticsConfiguration {
    rule.on_node_with_tag(tag).fetch_semantics_node().config().clone()
}

const NONE: KeyModifiers = KeyModifiers::None;
const CTRL: KeyModifiers = KeyModifiers::Ctrl;
const SHIFT: KeyModifiers = KeyModifiers::Shift;

fn ctrl_shift() -> KeyModifiers {
    KeyModifiers::Ctrl | KeyModifiers::Shift
}

/// A String field over `value`, its changes kept (as the C++ tests' `TextField(value, onValueChange)`).
#[kompozable]
fn StringField(value: MutableState<String>, tag: &'static str) {
    let v = value.clone();
    BasicTextFieldString(
        value.get(),
        move |it: String| v.set(it),
        modifier = Modifier.width(300.dp()).test_tag(tag),
        text_style = style16(),
    );
}

/// A TextFieldValue field over `value`, its changes kept.
#[kompozable]
fn ValueField(value: MutableState<TextFieldValue>, tag: &'static str) {
    let v = value.clone();
    BasicTextFieldValue(
        value.get(),
        move |it: TextFieldValue| v.set(it),
        modifier = Modifier.width(300.dp()).test_tag(tag),
        text_style = style16(),
    );
}

fn string_rule(initial: &str) -> (KompozTestRule, MutableState<String>, Rc<Ime>) {
    let value = mutable_state_of(initial.to_string());
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || StringField(v.clone(), "f")));
    let p = ime(&rule);
    rule.wait_for_idle();
    (rule, value, p)
}

// ---------------------------------------------------------------------------------------------------------------
// Typing and the input method

/// C++ 735 (its value-based half): UTF-8 typed and deleted whole; two inputs before a recomposition both kept; an
/// emoji sequence deleted whole by Backspace.
fn typing_goes_through_the_input_session_in_utf8() {
    let (rule, value, _ime) = string_rule("");
    rule.on_node_with_tag("f").perform_text_input("çağ");
    assert_eq!(value.peek(), "çağ");
    key(&rule, Key::Backspace, NONE);
    assert_eq!(value.peek(), "ça");
    // Two inputs before a recomposition: the first one isn't lost.
    rule.host().text_input("x");
    rule.host().text_input("y");
    rule.wait_for_idle();
    assert_eq!(value.peek(), "çaxy");
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    type_text(&rule, family);
    key(&rule, Key::Backspace, NONE);
    assert_eq!(value.peek(), "çaxy");
}

/// C++ 794 (its value-based half): an input method composing (the composition the field's, the app hearing only
/// the text), then committing its correction; Backspace at the caret.
fn input_method_compositions_and_corrections() {
    let value = mutable_state_of(TextFieldValue::default());
    let text = mutable_state_of(String::new());
    let (v, t) = (value.clone(), text.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, t) = (v.clone(), t.clone());
        Column(move || {
            ValueField(v.clone(), "value");
            StringField(t.clone(), "text");
        });
    }));
    let _ime = ime(&rule);
    rule.on_node_with_tag("value").perform_click();
    rule.host().set_composing_text("Guzel", 1);
    rule.wait_for_idle();
    let v = value.peek();
    assert_eq!((v.text(), v.composition(), v.selection()), ("Guzel", Some(TextRange::new(0, 5)), TextRange::at(5)));
    rule.host().text_input("Güzel ");
    rule.wait_for_idle();
    let v = value.peek();
    assert_eq!((v.text(), v.composition(), v.selection()), ("Güzel ", None, TextRange::at(7)));
    // The same on a String field: the app hears the text as it is composed.
    rule.on_node_with_tag("text").perform_click();
    rule.host().set_composing_text("Guzel", 1);
    rule.wait_for_idle();
    assert_eq!(text.peek(), "Guzel");
    rule.host().text_input("Güzel ");
    rule.wait_for_idle();
    assert_eq!(text.peek(), "Güzel ");
    rule.host().send_edit_commands(&[
        EditCommand::DeleteAll,
        EditCommand::CommitText { text: "abcd".into(), new_cursor_position: 1 },
        EditCommand::SetSelection { start: 2, end: 2 },
    ]);
    rule.wait_for_idle();
    key(&rule, Key::Backspace, NONE); // at the caret, not at the end
    assert_eq!(text.peek(), "acd");
}

/// C++ 813 (its value-based half): a caret moved by the input method changes no text (a String field hears
/// nothing); text typed and taken back before the app composed again is two changes, both heard.
fn only_text_changes_are_text_changes() {
    let value = mutable_state_of("kalem".to_string());
    let changes = Rc::new(RefCell::new(Vec::<String>::new()));
    let (v, c) = (value.clone(), changes.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v2, c) = (v.clone(), c.clone());
        BasicTextFieldString(
            v.get(),
            move |it: String| {
                c.borrow_mut().push(it.clone());
                v2.set(it);
            },
            modifier = Modifier.width(300.dp()).test_tag("f"),
            text_style = style16(),
        );
    }));
    let _ime = ime(&rule);
    rule.on_node_with_tag("f").perform_click(); // the focus and a caret: no change
    rule.host().send_edit_commands(&[EditCommand::SetSelection { start: 2, end: 2 }]);
    rule.wait_for_idle();
    assert!(changes.borrow().is_empty() && is_focused(&rule, "f"));
    rule.host().send_edit_commands(&[EditCommand::SetSelection { start: 5, end: 5 }]);
    rule.host().text_input("i"); // typed and taken back before the app composed again
    rule.host().send_edit_commands(&[EditCommand::Backspace]);
    rule.wait_for_idle();
    assert_eq!(*changes.borrow(), vec!["kalemi".to_string(), "kalem".to_string()]);
    assert_eq!(value.peek(), "kalem");
}

/// C++ 1085: a field whose value comes back a frame later (a flow collected as state) keeps what is typed: the
/// input method isn't told the old text meanwhile; a field refusing letters takes the input method back to what it
/// keeps.
fn a_field_whose_value_comes_back_a_frame_later_keeps_what_is_typed() {
    let query = MutableStateFlow::new(String::new());
    let digits_only = mutable_state_of("12".to_string());
    let ticks = mutable_state_of(0);
    let (q, d, t) = (query.clone(), digits_only.clone(), ticks.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (q, d, t) = (q.clone(), d.clone(), t.clone());
        Column(move || {
            BasicText(format!("{}", t.get()));
            let current = q.collect_as_state();
            let q2 = q.clone();
            BasicTextFieldString(
                current,
                move |v: String| q2.set_value(v),
                modifier = Modifier.width(300.dp()).test_tag("ara"),
                single_line = true,
            );
            let d2 = d.clone();
            // One refusing letters: the input method goes back to what it keeps.
            BasicTextFieldString(
                d.get(),
                move |v: String| {
                    if v.chars().all(|c| c.is_ascii_digit()) {
                        d2.set(v);
                    }
                },
                modifier = Modifier.width(300.dp()).test_tag("sayi"),
                single_line = true,
            );
        });
    }));
    let p = ime(&rule);
    // As a platform delivers them: at the start of a frame, with something else changing in it.
    let type_now = |text: &str| {
        rule.host().text_input(text);
        ticks.set(ticks.peek() + 1);
        rule.advance_frame();
    };
    rule.on_node_with_tag("ara").perform_click();
    type_now("a");
    type_now("n");
    rule.wait_for_idle();
    assert_eq!((query.value(), p.resyncs()), ("an".to_string(), Vec::<String>::new()));
    rule.on_node_with_tag("sayi").perform_click();
    // (the click's caret is the field's own change, told the input method too)
    p.changed.borrow_mut().clear();
    type_now("x");
    rule.wait_for_idle();
    assert_eq!((digits_only.peek(), p.resyncs()), ("12".to_string(), vec!["12".to_string()]));
    type_now("3");
    rule.wait_for_idle();
    assert_eq!(digits_only.peek(), "123");
}

/// C++ 1452 (its value-based half): the input method's own edits aren't echoed back to it, the app's are; the
/// action key of a single-line field (Done) runs the app's action; tapping the focused field shows the keyboard
/// again.
fn app_side_change_resyncs_input_method() {
    let value = mutable_state_of(String::new());
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v2, v3) = (v.clone(), v.clone());
        let submit: kompoz_foundation::text::KeyboardAction = Rc::new(move |_| v3.set(String::new()));
        BasicTextFieldString(
            v.get(),
            move |it: String| v2.set(it),
            modifier = Modifier.width(300.dp()).test_tag("f"),
            text_style = style16(),
            keyboard_options = KeyboardOptions(ime_action = ImeAction::Done),
            keyboard_actions = KeyboardActions(on_done = Some(submit)),
            single_line = true,
        );
    }));
    let p = ime(&rule);
    rule.on_node_with_tag("f").perform_click();
    assert!(p.active() && p.shows() >= 1);
    rule.host().set_composing_text("kalem", 1);
    rule.wait_for_idle();
    assert!(p.resyncs().is_empty()); // the input method's own edit isn't echoed back
    key(&rule, Key::Enter, NONE); // the app's action clears the field
    assert_eq!((value.peek().as_str(), p.resyncs()), ("", vec![String::new()]));
    let before = p.shows();
    rule.on_node_with_tag("f").perform_click(); // tapping the focused field brings the keyboard back
    assert_eq!(p.shows(), before + 1);
    // The input method's own action key runs the app's action too.
    type_text(&rule, "abc");
    assert!(rule.host().ime_action(ImeAction::Done));
    rule.wait_for_idle();
    assert_eq!(value.peek(), "");
}

/// The input session opens with the keyboard options as the input method's options, closes when the field loses
/// the focus (its composition finished), and goes through the field's own text input service into the platform's
/// session (one at a time: another field's session ends it).
fn the_session_follows_the_focus() {
    let a = mutable_state_of(TextFieldValue::default());
    let b = mutable_state_of(String::new());
    let (va, vb) = (a.clone(), b.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (va, vb) = (va.clone(), vb.clone());
        Column(move || {
            let va2 = va.clone();
            BasicTextFieldValue(
                va.get(),
                move |it: TextFieldValue| va2.set(it),
                modifier = Modifier.width(300.dp()).test_tag("a"),
                keyboard_options = KeyboardOptions(keyboard_type = kompoz_ui::platform::KeyboardType::Email),
                single_line = true,
            );
            StringField(vb.clone(), "b");
        });
    }));
    let p = ime(&rule);
    rule.on_node_with_tag("a").perform_click();
    let options = p.shown.borrow().last().unwrap().1.clone();
    assert!(
        options.keyboard_type == kompoz_ui::platform::KeyboardType::Email
            && options.single_line
            && options.show_keyboard
    );
    rule.host().set_composing_text("ab", 1);
    rule.wait_for_idle();
    assert_eq!(a.peek().composition(), Some(TextRange::new(0, 2)));
    rule.on_node_with_tag("b").perform_click();
    // The composition finished as the focus left; the other field's session took over.
    assert_eq!((a.peek().text(), a.peek().composition()), ("ab", None));
    type_text(&rule, "c");
    assert_eq!((a.peek().text(), b.peek().as_str()), ("ab", "c"));
    rule.host().clear_focus(true);
    rule.wait_for_idle();
    assert!(!p.active() && !rule.host().text_input_service().has_session());
}

/// The field's input goes through the platform text input API as Compose 1.12's legacy adapter has it: an
/// interceptor around the field sees its input method request (and may change it), what is typed reaching the
/// field through it.
fn its_input_method_requests_are_intercepted() {
    use kompoz_ui::platform::KeyboardType;
    use kompoz_ui::platform_text_input::{
        InterceptPlatformTextInput, PlatformTextInputInterceptor, PlatformTextInputMethodRequest,
        PlatformTextInputSession, SessionFuture,
    };
    let intercepted = Rc::new(Cell::new(0));
    let i = intercepted.clone();
    let interceptor: Rc<dyn PlatformTextInputInterceptor> =
        Rc::new(move |request: PlatformTextInputMethodRequest, next: PlatformTextInputSession| -> SessionFuture {
            i.set(i.get() + 1);
            let mut request = request;
            request.ime_options.keyboard_type = KeyboardType::Phone;
            next.start_input_method(request)
        });
    let value = mutable_state_of(String::new());
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v = v.clone();
        InterceptPlatformTextInput(interceptor.clone(), move || StringField(v.clone(), "f"));
    }));
    let p = ime(&rule);
    rule.on_node_with_tag("f").perform_click();
    assert_eq!(intercepted.get(), 1);
    assert_eq!(p.shown.borrow().last().map(|(_, o)| o.keyboard_type), Some(KeyboardType::Phone));
    type_text(&rule, "555");
    assert_eq!(value.peek(), "555");
}

// ---------------------------------------------------------------------------------------------------------------
// Keys

/// C++ 11138 (its value-based half): editing keys, the selection and the clipboard.
fn editing_keys_selection_and_clipboard() {
    let (rule, value, p) = string_rule("abcdef");
    rule.on_node_with_tag("f").perform_click();
    key(&rule, Key::MoveHome, NONE);
    key(&rule, Key::DirectionRight, SHIFT);
    key(&rule, Key::DirectionRight, SHIFT);
    key(&rule, Key::X, CTRL);
    assert_eq!((value.peek().as_str(), p.clipboard().as_str()), ("cdef", "ab"));
    key(&rule, Key::MoveEnd, NONE);
    key(&rule, Key::V, CTRL);
    assert_eq!(value.peek(), "cdefab");
    key(&rule, Key::DirectionLeft, NONE);
    key(&rule, Key::DirectionLeft, NONE);
    key(&rule, Key::Delete, NONE);
    assert_eq!(value.peek(), "cdefb");
    key(&rule, Key::DirectionLeft, SHIFT);
    key(&rule, Key::C, CTRL);
    assert_eq!(p.clipboard(), "f");
    key(&rule, Key::A, CTRL);
    key(&rule, Key::Backspace, NONE);
    assert_eq!(value.peek(), "");
}

/// C++ 11170 (its value-based half): Compose's key mapping (Ctrl, not macOS) by words and paragraphs; a read-only
/// field takes no edit. Words are the layout's (Compose's `getWordBoundary`: Android's WordBoundary, punctuation a
/// word of its own): where the C++ (as Compose desktop's ICU boundaries) stepped over ", " with the word before it,
/// Ctrl+arrows stop at the comma and Ctrl+Backspace takes it alone; End goes to the line's visible end.
fn editing_keys_go_by_words_and_paragraphs() {
    let line = mutable_state_of("bir iki, üç".to_string());
    let lines = mutable_state_of("a\nbc\nd".to_string());
    let locked = mutable_state_of("sabit metin".to_string());
    let (l1, l2, l3) = (line.clone(), lines.clone(), locked.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l1, l2, l3) = (l1.clone(), l2.clone(), l3.clone());
        Column(move || {
            let (a, b, c) = (l1.clone(), l2.clone(), l3.clone());
            BasicTextFieldString(
                l1.get(),
                move |v: String| a.set(v),
                modifier = Modifier.width(300.dp()).test_tag("tek"),
                text_style = style16(),
                single_line = true,
            );
            BasicTextFieldString(
                l2.get(),
                move |v: String| b.set(v),
                modifier = Modifier.width(300.dp()).test_tag("çok"),
                text_style = style16(),
            );
            BasicTextFieldString(
                l3.get(),
                move |v: String| c.set(v),
                modifier = Modifier.width(300.dp()).test_tag("kilitli"),
                text_style = style16(),
                read_only = true,
            );
        });
    }));
    let p = ime(&rule);
    rule.on_node_with_tag("tek").perform_click();
    key(&rule, Key::MoveEnd, NONE);
    key(&rule, Key::DirectionLeft, CTRL); // to "üç"
    key(&rule, Key::DirectionLeft, CTRL); // to the comma
    key(&rule, Key::DirectionLeft, CTRL); // to "iki"
    key(&rule, Key::DirectionRight, ctrl_shift());
    key(&rule, Key::C, CTRL);
    assert_eq!(p.clipboard(), "iki");
    key(&rule, Key::DirectionRight, CTRL); // past the comma
    key(&rule, Key::DirectionRight, CTRL); // past the space to the end of "üç"
    key(&rule, Key::Backspace, CTRL);
    assert_eq!(line.peek(), "bir iki, ");
    key(&rule, Key::Backspace, CTRL); // ", " alone
    assert_eq!(line.peek(), "bir iki");
    key(&rule, Key::Backspace, CTRL);
    assert_eq!(line.peek(), "bir ");
    key(&rule, Key::MoveHome, NONE);
    key(&rule, Key::Delete, CTRL);
    assert_eq!(line.peek(), " ");
    key(&rule, Key::MoveEnd, CTRL); // (End: the line's visible end, before the space)
    key(&rule, Key::H, CTRL); // Backspace
    assert_eq!(line.peek(), "");

    rule.on_node_with_tag("çok").perform_click();
    key(&rule, Key::MoveEnd, CTRL);
    key(&rule, Key::DirectionUp, CTRL); // the start of "d"
    key(&rule, Key::DirectionUp, CTRL); // already there: the start of "bc"
    key(&rule, Key::DirectionDown, ctrl_shift());
    key(&rule, Key::C, CTRL);
    assert_eq!(p.clipboard(), "bc");
    key(&rule, Key::DirectionUp, CTRL); // from the selection's start: "a"
    key(&rule, Key::Delete, KeyModifiers::Alt); // to the line's end
    assert_eq!(lines.peek(), "\nbc\nd");
    // Lines: up and down by the layout's lines, at the x they started from.
    lines.set("abc\nd\nefg".to_string());
    rule.wait_for_idle();
    key(&rule, Key::MoveEnd, CTRL);
    key(&rule, Key::DirectionUp, NONE);
    key(&rule, Key::DirectionUp, NONE); // past "d", back at the column it left
    type_text(&rule, "1");
    assert_eq!(lines.peek(), "abc1\nd\nefg");
    key(&rule, Key::DirectionUp, NONE); // the first line: to its start
    type_text(&rule, "0");
    assert_eq!(lines.peek(), "0abc1\nd\nefg");

    rule.on_node_with_tag("kilitli").perform_click();
    key(&rule, Key::MoveEnd, NONE);
    key(&rule, Key::Backspace, CTRL);
    key(&rule, Key::Delete, KeyModifiers::Alt);
    type_text(&rule, "x");
    assert_eq!(locked.peek(), "sabit metin");
}

/// Tab types a tab in a multi-line field and moves the focus out of a single-line one; Enter types a line feed in
/// a multi-line one.
fn tab_and_enter() {
    let one = mutable_state_of("a".to_string());
    let many = mutable_state_of("b".to_string());
    let (o, m) = (one.clone(), many.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, m) = (o.clone(), m.clone());
        Column(move || {
            let (a, b) = (o.clone(), m.clone());
            BasicTextFieldString(
                o.get(),
                move |v: String| a.set(v),
                modifier = Modifier.width(300.dp()).test_tag("one"),
                single_line = true,
            );
            BasicTextFieldString(
                m.get(),
                move |v: String| b.set(v),
                modifier = Modifier.width(300.dp()).test_tag("many"),
            );
        });
    }));
    let _p = ime(&rule);
    rule.on_node_with_tag("many").perform_click();
    key(&rule, Key::MoveEnd, CTRL);
    key(&rule, Key::Tab, NONE);
    key(&rule, Key::Enter, NONE);
    assert_eq!(many.peek(), "b\t\n");
    rule.on_node_with_tag("one").perform_click();
    key(&rule, Key::Tab, NONE);
    assert!(is_focused(&rule, "many") && one.peek() == "a");
}

// ---------------------------------------------------------------------------------------------------------------
// Undo

/// Compose's legacy UndoManager: snapshots every five seconds at most and at key commands; a value of the same text
/// moves the last snapshot's selection; undo and redo; the stored characters bound the history.
fn legacy_undo_manager_snapshots() {
    let v = |text: &str, at: usize| TextFieldValue::from_text(text, TextRange::at(at), None);
    let undo = UndoManager::new(10);
    undo.snapshot_if_needed_at(&v("", 0), 10_000);
    undo.snapshot_if_needed_at(&v("a", 1), 11_000); // within five seconds: none
    undo.snapshot_if_needed_at(&v("ab", 2), 15_001); // more than five seconds since the last
    assert_eq!(undo.undo_values().iter().map(|v| v.text().to_string()).collect::<Vec<_>>(), vec!["ab", ""]);
    undo.force_next_snapshot();
    undo.snapshot_if_needed_at(&v("abc", 3), 15_002); // forced
    undo.make_snapshot(&v("abc", 1)); // the same text: the last one's selection moved
    assert_eq!(undo.undo_values()[0], v("abc", 1));
    assert_eq!(undo.undo(), Some(v("ab", 2)));
    assert_eq!(undo.undo(), Some(v("", 0)));
    assert_eq!(undo.undo(), None); // nothing before the first
    assert_eq!(undo.redo(), Some(v("ab", 2)));
    undo.make_snapshot(&v("abx", 3)); // a new one clears the redo stack
    assert_eq!(undo.redo(), None);
    // Beyond its characters, the oldest go (the newest kept).
    undo.make_snapshot(&v("abxyz", 5));
    undo.make_snapshot(&v("abxyz12", 7));
    assert!(undo.undo_values().len() < 5 && undo.undo_values()[0].text() == "abxyz12");
}

/// C++ 11229 (its value-based half, adapted to Compose's legacy undo): a run typed within five seconds is one
/// step (the C++ split it at words), key commands make steps; redo by Ctrl+Shift+Z and Ctrl+Y; the selection comes
/// back with the text (typed where the undone edit was); moved after an undo, it stays moved though the field
/// composes again.
fn a_field_whose_text_the_app_keeps_undoes_too() {
    let value = mutable_state_of(String::new());
    let label = mutable_state_of("Not".to_string());
    let (v, l) = (value.clone(), label.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, l) = (v.clone(), l.clone());
        Column(move || {
            BasicText(l.get());
            StringField(v.clone(), "f");
        });
    }));
    let _p = ime(&rule);
    rule.on_node_with_tag("f").perform_click();
    type_text(&rule, "bir");
    type_text(&rule, " iki");
    key(&rule, Key::Z, CTRL);
    assert_eq!(value.peek(), ""); // within five seconds: one step
    key(&rule, Key::Z, ctrl_shift());
    assert_eq!(value.peek(), "bir iki");
    key(&rule, Key::Y, CTRL); // nothing more to redo
    assert_eq!(value.peek(), "bir iki");
    // The selection comes back with the text: typed where the undone edit was.
    key(&rule, Key::MoveHome, NONE);
    key(&rule, Key::Delete, NONE);
    assert_eq!(value.peek(), "ir iki");
    key(&rule, Key::Z, CTRL);
    type_text(&rule, "_");
    assert_eq!(value.peek(), "_bir iki");
    // A key command (select all) makes a step; typing over it is undone back to it.
    key(&rule, Key::A, CTRL);
    type_text(&rule, "yeni");
    assert_eq!(value.peek(), "yeni");
    key(&rule, Key::Z, CTRL);
    assert_eq!(value.peek(), "_bir iki");
    // Moved after an undo (the whole text selected), it stays moved though the field composes again.
    key(&rule, Key::DirectionRight, NONE);
    label.set("Not 2".to_string());
    rule.wait_for_idle();
    type_text(&rule, "-");
    assert_eq!(value.peek(), "_bir iki-");
}

/// C++ 11286 (its value-based half): an undo the app refuses leaves the text. A TextFieldValue field the app keeps
/// leaves its caret too; a String field keeps the refused undo's selection on the app's text (Compose's: the
/// field's own selection, the app's text) — typed at the start.
fn an_undo_the_app_refuses_leaves_the_text() {
    let value = mutable_state_of(TextFieldValue::default());
    let text = mutable_state_of(String::new());
    let (v, t) = (value.clone(), text.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, t) = (v.clone(), t.clone());
        Column(move || {
            let (v2, t2) = (v.clone(), t.clone());
            // Never empty.
            BasicTextFieldValue(
                v.get(),
                move |it: TextFieldValue| {
                    if !it.text().is_empty() {
                        v2.set(it);
                    }
                },
                modifier = Modifier.width(300.dp()).test_tag("value"),
            );
            BasicTextFieldString(
                t.get(),
                move |it: String| {
                    if !it.is_empty() {
                        t2.set(it);
                    }
                },
                modifier = Modifier.width(300.dp()).test_tag("text"),
            );
        });
    }));
    let _p = ime(&rule);
    rule.on_node_with_tag("value").perform_click();
    type_text(&rule, "a");
    key(&rule, Key::Z, CTRL); // back to nothing: refused
    assert_eq!(value.peek().text(), "a");
    type_text(&rule, "b");
    type_text(&rule, "c");
    assert_eq!(value.peek().text(), "abc");
    rule.on_node_with_tag("text").perform_click();
    type_text(&rule, "a");
    key(&rule, Key::Z, CTRL);
    assert_eq!(text.peek(), "a");
    type_text(&rule, "b");
    assert_eq!(text.peek(), "ba");
}

// ---------------------------------------------------------------------------------------------------------------
// The TextFieldValue overload

/// The TextFieldValue field: the selection and composition are the app's to keep (a selection set by the app is
/// the field's); only another value is told.
fn the_value_overload_keeps_the_apps_selection() {
    let value = mutable_state_of(TextFieldValue::from_text("merhaba", TextRange::new(1, 3), None));
    let told = Rc::new(Cell::new(0));
    let (v, n) = (value.clone(), told.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v2, n) = (v.clone(), n.clone());
        BasicTextFieldValue(
            v.get(),
            move |it: TextFieldValue| {
                n.set(n.get() + 1);
                v2.set(it);
            },
            modifier = Modifier.width(300.dp()).test_tag("f"),
            text_style = style16(),
        );
    }));
    let p = ime(&rule);
    rule.on_node_with_tag("f").perform_click();
    let told_after_click = told.get();
    // The tap placed the caret: a value of its own.
    assert!(told_after_click >= 1 && value.peek().selection().collapsed());
    value.set(TextFieldValue::from_text("merhaba", TextRange::new(0, 3), None));
    rule.wait_for_idle();
    assert_eq!(config_of(&rule, "f").get(SemanticsProperties::TextSelectionRange), Some(TextRange::new(0, 3)));
    // The app's selection is the input method's too.
    assert!(p.changed.borrow().last().is_some_and(|s| (s.selection_start, s.selection_end) == (0, 3)));
    type_text(&rule, "X");
    assert_eq!(value.peek().text(), "Xhaba");
    // A value told back unchanged isn't told again.
    let before = told.get();
    rule.host().send_edit_commands(&[EditCommand::SetSelection { start: 2, end: 2 }]);
    rule.wait_for_idle();
    assert_eq!(told.get(), before + 1);
    rule.host().send_edit_commands(&[EditCommand::SetSelection { start: 2, end: 2 }]);
    rule.wait_for_idle();
    assert_eq!(told.get(), before + 1);
}

// ---------------------------------------------------------------------------------------------------------------
// Visual transformations

/// A password field (PasswordVisualTransformation): its text shown as bullets, one a character; the caret mapped by
/// character; nothing copied or cut; a password to accessibility (its input text the text, its editable text the
/// bullets).
fn password_visual_transformation() {
    let value = mutable_state_of("şifre".to_string());
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v2 = v.clone();
        BasicTextFieldString(
            v.get(),
            move |it: String| v2.set(it),
            modifier = Modifier.width(300.dp()).test_tag("f"),
            text_style = style16(),
            visual_transformation = PasswordVisualTransformation(),
        );
    }));
    let p = ime(&rule);
    rule.wait_for_idle();
    assert_eq!(rule.drawn_texts(), vec!["•••••".to_string()]);
    let config = config_of(&rule, "f");
    assert!(config.contains(SemanticsProperties::Password));
    assert_eq!(config.get(SemanticsProperties::EditableText).map(|t| t.text().to_string()), Some("•••••".into()));
    assert_eq!(config.get(SemanticsProperties::InputText).map(|t| t.text().to_string()), Some("şifre".into()));
    rule.on_node_with_tag("f").perform_click();
    key(&rule, Key::MoveEnd, NONE);
    key(&rule, Key::DirectionLeft, NONE);
    key(&rule, Key::DirectionLeft, NONE);
    type_text(&rule, "x");
    assert_eq!(value.peek(), "şifxre");
    p.set_clip("pano");
    key(&rule, Key::A, CTRL);
    key(&rule, Key::C, CTRL);
    key(&rule, Key::X, CTRL);
    assert_eq!((p.clipboard().as_str(), value.peek().as_str()), ("pano", "şifxre"));
    assert!(!config_of(&rule, "f").contains(SemanticsActions::CopyText));
    // Its own mask.
    assert_eq!(
        named_args!(PasswordVisualTransformation(mask = '*')).filter(&AnnotatedString::new("ab")).text.text(),
        "**"
    );
    assert!(
        named_args!(PasswordVisualTransformation(mask = '*')) == named_args!(PasswordVisualTransformation(mask = '*'))
    );
    assert!(named_args!(PasswordVisualTransformation(mask = '*')) != PasswordVisualTransformation());
}

/// A phone number's separators: "5551234567" shown as "555-123-4567", the offsets mapped both ways.
struct PhoneMapping {
    len: usize,
}

impl MapOffsets for PhoneMapping {
    fn original_to_transformed(&self, offset: usize) -> usize {
        if offset <= 3 {
            offset
        } else if offset <= 6 {
            offset + 1
        } else {
            offset + 2
        }
    }
    fn transformed_to_original(&self, offset: usize) -> usize {
        let original = if offset <= 3 {
            offset
        } else if offset <= 7 {
            offset - 1
        } else {
            offset - 2
        };
        original.min(self.len)
    }
}

fn phone() -> VisualTransformation {
    VisualTransformation::new(|text: &AnnotatedString| {
        let digits = text.text();
        let mut shown = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i == 3 || i == 6 {
                shown.push('-');
            }
            shown.push(c);
        }
        TransformedText::new(AnnotatedString::new(shown), OffsetMapping::of(PhoneMapping { len: digits.len() }))
    })
}

/// A phone number mask with its offset mapping: the separators shown, not in the value; the caret and a tap
/// mapped; selection set by accessibility in the shown text mapped to the value's.
fn phone_number_mask_maps_offsets() {
    let value = mutable_state_of(TextFieldValue::from_text("5551234567", TextRange::at(10), None));
    let v = value.clone();
    let mask = phone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, mask) = (v.clone(), mask.clone());
        // (not blinking: the cursor drawn whenever looked at)
        KompozitionLocalProvider(&[LocalCursorBlinkEnabled.provides(false)], move || {
            let v2 = v.clone();
            BasicTextFieldValue(
                v.get(),
                move |it: TextFieldValue| v2.set(it),
                modifier = Modifier.width(300.dp()).test_tag("f"),
                text_style = style16(),
                visual_transformation = mask.clone(),
            );
        });
    }));
    let _p = ime(&rule);
    rule.wait_for_idle();
    assert_eq!(rule.drawn_texts(), vec!["555-123-4567".to_string()]);
    let config = config_of(&rule, "f");
    assert_eq!(
        config.get(SemanticsProperties::EditableText).map(|t| t.text().to_string()),
        Some("555-123-4567".into())
    );
    // A tap after the first separator (x = 4.5 characters): the caret after "555" … "1" in the value.
    let b = bounds(&rule, "f");
    tap(&rule, at(b.left + 4.6 * 8.0, b.center().y));
    assert_eq!(value.peek().selection(), TextRange::at(4));
    // Keys move over the value's characters.
    key(&rule, Key::DirectionLeft, NONE);
    assert_eq!(value.peek().selection(), TextRange::at(3));
    type_text(&rule, "9");
    assert_eq!(value.peek().text(), "55591234567");
    // Accessibility's selection in the shown text ("123" at 4..7) is the value's 3..6.
    rule.on_node_with_tag("f").perform_text_input_selection(TextRange::new(4, 7), false);
    rule.wait_for_idle();
    assert_eq!(value.peek().selection(), TextRange::new(3, 6));
    // The caret drawn where the mapping puts it: the value's 7 after "555-123-".
    value.set(TextFieldValue::from_text("5551234567", TextRange::at(7), None));
    rule.wait_for_idle();
    let caret = cursors(&rule.draw());
    assert!(caret.len() == 1 && near(caret[0].left - b.left, 9.0 * 8.0 + 1.0, 0.01), "{caret:?}");
}

/// A transformation whose mapping gives an offset outside the other text panics with Compose's message.
fn a_faulty_offset_mapping_panics() {
    let faulty = VisualTransformation::new(|text: &AnnotatedString| {
        TransformedText::new(text.clone(), OffsetMapping::new(|o| o + 5, |o| o))
    });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        filter_with_validation(&faulty, &AnnotatedString::new("abc"))
    }));
    let message = result.err().and_then(|e| e.downcast_ref::<String>().cloned()).unwrap_or_default();
    assert!(message.contains("OffsetMapping.originalToTransformed returned invalid mapping: 0 -> 5"), "{message}");
}

// ---------------------------------------------------------------------------------------------------------------
// Scrolling

/// Compose's TextFieldScrollerPosition: the cursor's end brought into view, its start where it fits, the offset
/// kept within what can be scrolled; the end that moved is followed.
fn scroller_position_keeps_the_cursor_in_view() {
    let p = TextFieldScrollerPosition::new(Orientation::Horizontal, 0.0);
    p.update(Orientation::Horizontal, Rect::new(150.0, 0.0, 152.0, 20.0), 100, 200);
    assert_eq!((p.offset(), p.maximum(), p.viewport_size()), (52.0, 100.0, 100));
    p.update(Orientation::Horizontal, Rect::new(10.0, 0.0, 12.0, 20.0), 100, 200);
    assert_eq!(p.offset(), 10.0); // back to the cursor's start
    p.update(Orientation::Horizontal, Rect::new(10.0, 0.0, 12.0, 20.0), 100, 50);
    assert_eq!(p.offset(), 0.0); // nothing to scroll: within 0 and the (negative) maximum
    p.set_offset(50.0);
    p.coerce_offset(20.0, 200.0, 100); // a cursor longer than the view: its end first
    assert_eq!(p.offset(), 100.0);
    p.set_previous_selection(TextRange::new(2, 5));
    assert_eq!(p.get_offset_to_follow(TextRange::new(2, 7)), 7);
    assert_eq!(p.get_offset_to_follow(TextRange::new(1, 5)), 1);
    assert_eq!(p.get_offset_to_follow(TextRange::new(5, 2)), 5);
}

/// A single-line field scrolls sideways to keep the cursor in view as text is typed past its width; Home brings
/// the start back; a drag scrolls it.
fn single_line_field_scrolls_to_the_cursor() {
    let value = mutable_state_of(String::new());
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v2 = v.clone();
        BasicTextFieldString(
            v.get(),
            move |it: String| v2.set(it),
            modifier = Modifier.width(80.dp()).test_tag("f"),
            text_style = style16(),
            single_line = true,
        );
    }));
    let _p = ime(&rule);
    rule.on_node_with_tag("f").perform_click();
    let b = bounds(&rule, "f");
    assert!(near(b.width(), 80.0, 0.5) && near(b.height(), 20.0, 0.5), "{b:?}");
    type_text(&rule, "abcdefghijklmnopqrst"); // 160 px
    let canvas = rule.draw();
    // The text's end at the field's right edge (the cursor after it drawn within the text's width: Compose's)...
    let start = canvas.baselines[0].x;
    assert!(near(start - b.left, 80.0 - 160.0, 1.0), "{start} {b:?}");
    key(&rule, Key::MoveHome, NONE);
    let start = rule.draw().baselines[0].x;
    assert!(near(start, b.left, 1.0), "{start}");
    // A multi-line field is as tall as its lines and doesn't scroll sideways.
    assert_eq!(value.peek().len(), 20);
}

/// A multi-line field grows with its lines up to its max lines, then scrolls down to the cursor; at least its min
/// lines tall.
fn multi_line_field_grows_then_scrolls() {
    let value = mutable_state_of(String::new());
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v2 = v.clone();
        BasicTextFieldString(
            v.get(),
            move |it: String| v2.set(it),
            modifier = Modifier.width(200.dp()).test_tag("f"),
            text_style = style16(),
            min_lines = 2,
            max_lines = 3,
        );
    }));
    let _p = ime(&rule);
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "f").height(), 40.0, 0.5));
    rule.on_node_with_tag("f").perform_click();
    type_text(&rule, "1\n2\n3");
    assert!(near(bounds(&rule, "f").height(), 60.0, 0.5));
    type_text(&rule, "\n4\n5");
    let b = bounds(&rule, "f");
    assert!(near(b.height(), 60.0, 0.5));
    // Scrolled down two lines: the first line drawn 40 px above the field's top.
    let canvas = rule.draw();
    let first = canvas.baselines[0].y;
    assert!(first < b.top - 20.0, "{first} {b:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// Selection

/// The mouse: a click places the caret, a double click selects a word, a triple click the paragraph, shift+click
/// extends, a drag selects; the selection drawn behind the text.
fn mouse_selects_words_paragraphs_and_extends() {
    let value = mutable_state_of(TextFieldValue::from_text("bir iki üç", TextRange::Zero, None));
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || ValueField(v.clone(), "f")));
    let _p = ime(&rule);
    rule.wait_for_idle();
    let b = bounds(&rule, "f");
    let y = b.center().y;
    clicks(&rule, at(b.left + 5.0 * 8.0 + 2.0, y), 1, NONE);
    assert!(is_focused(&rule, "f"));
    assert_eq!(value.peek().selection(), TextRange::at(5));
    // A double click: the word.
    clicks(&rule, at(b.left + 5.0 * 8.0 + 2.0, y), 2, NONE);
    assert_eq!(value.peek().selection(), TextRange::new(4, 7));
    assert!(!rule.draw().paths.is_empty()); // the selection's highlight
    // A triple click: the paragraph.
    clicks(&rule, at(b.left + 5.0 * 8.0 + 2.0, y), 3, NONE);
    assert_eq!(value.peek().selection(), TextRange::new(0, 12));
    // Shift+click extends from where the selection began.
    clicks(&rule, at(b.left + 1.0 * 8.0 + 2.0, y), 1, NONE);
    clicks(&rule, at(b.left + 7.0 * 8.0 + 2.0, y), 1, SHIFT);
    assert_eq!(value.peek().selection(), TextRange::new(1, 7));
    // A drag selects from where it went down.
    rule.advance_time_by(1000);
    rule.host().pointer_down(at(b.left + 2.0, y), -1, 0, PointerType::Mouse);
    for x in [20.0, 40.0, 60.0] {
        rule.advance_frame();
        rule.host().pointer_move(at(b.left + x, y), -1, 0);
    }
    rule.host().pointer_up(at(b.left + 60.0, y), -1, 0);
    rule.wait_for_idle();
    assert_eq!(value.peek().selection(), TextRange::new(0, 7));
}

/// Touch: a tap places the caret with its handle; a long press selects a word with its handles and the toolbar
/// (copy, cut, paste, select all), a tap deselects; the toolbar's actions work; a selection handle's drag extends
/// the selection with a haptic tick.
fn touch_selects_with_handles_and_the_toolbar() {
    let value = mutable_state_of(TextFieldValue::from_text("bir iki üç", TextRange::Zero, None));
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || ValueField(v.clone(), "f")));
    let p = ime(&rule);
    rule.wait_for_idle();
    let b = bounds(&rule, "f");
    let y = b.center().y;
    tap(&rule, at(b.left + 2.0 * 8.0 + 2.0, y));
    assert_eq!(value.peek().selection(), TextRange::at(2));
    // The cursor's handle (a popup with its handle info).
    assert_eq!(handles(&rule).len(), 1, "{:?}", handles(&rule));
    rule.advance_time_by(500); // not a double tap
    long_press(&rule, at(b.left + 5.0 * 8.0, y));
    assert_eq!(value.peek().selection(), TextRange::new(4, 7));
    assert!(p.haptics.borrow().contains(&HapticFeedbackType::LongPress));
    assert!(p.toolbar_shown());
    assert_eq!(handles(&rule).len(), 2);
    // Copy from the toolbar.
    let copy = p.toolbar.borrow().as_ref().and_then(|(_, a)| a.on_copy_requested.clone()).expect("copy offered");
    copy();
    rule.wait_for_idle();
    assert_eq!(p.clipboard(), "iki");
    assert!(value.peek().selection().collapsed());
    // Paste over a new selection, from the toolbar.
    long_press(&rule, at(b.left + 1.0 * 8.0, y));
    assert_eq!(value.peek().selection(), TextRange::new(0, 3));
    let paste = p.toolbar.borrow().as_ref().and_then(|(_, a)| a.on_paste_requested.clone()).expect("paste offered");
    paste();
    rule.wait_for_idle();
    assert_eq!((value.peek().text(), value.peek().selection()), ("iki iki üç", TextRange::at(3)));
    // Select all, then cut.
    long_press(&rule, at(b.left + 1.0 * 8.0, y));
    let select_all = p.toolbar.borrow().as_ref().and_then(|(_, a)| a.on_select_all_requested.clone()).unwrap();
    select_all();
    rule.wait_for_idle();
    assert_eq!(value.peek().selection(), TextRange::new(0, 12));
    let cut = p.toolbar.borrow().as_ref().and_then(|(_, a)| a.on_cut_requested.clone()).expect("cut offered");
    cut();
    rule.wait_for_idle();
    assert_eq!((value.peek().text(), p.clipboard().as_str()), ("", "iki iki üç"));
    // A handle's drag extends the selection.
    value.set(TextFieldValue::from_text("bir iki üç", TextRange::Zero, None));
    rule.wait_for_idle();
    long_press(&rule, at(b.left + 1.0 * 8.0, y));
    assert_eq!(value.peek().selection(), TextRange::new(0, 3));
    let end = handles(&rule)
        .into_iter()
        .find(|h| h.handle == kompoz_foundation::text::Handle::SelectionEnd)
        .expect("the end handle");
    assert!(near(end.position.x, 24.0, 0.01) && near(end.position.y, 20.0, 0.01), "{end:?}");
    // (the handle hangs under its point: grabbed there)
    let from = at(b.left + end.position.x + 11.0, b.top + end.position.y + 11.0);
    rule.host().pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=6 {
        rule.advance_frame();
        rule.host().pointer_move(at(from.x + 8.0 * i as f32, from.y), -1, 0);
    }
    // The magnifier while the handle is dragged: over its line's middle (10 px down the text).
    rule.advance_frame();
    let magnifier = rule
        .all_nodes(true)
        .iter()
        .filter_map(|n| n.config().get(kompoz_foundation::magnifier::MagnifierPositionInRoot))
        .map(|m| (m.0)())
        .find(|o| o.is_specified());
    assert!(magnifier.is_some_and(|m| (m.y - (b.top + 10.0)).abs() < 0.5), "{magnifier:?}");
    rule.host().pointer_up(at(from.x + 48.0, from.y), -1, 0);
    rule.wait_for_idle();
    let gone = rule
        .all_nodes(true)
        .iter()
        .filter_map(|n| n.config().get(kompoz_foundation::magnifier::MagnifierPositionInRoot))
        .any(|m| (m.0)().is_specified());
    assert!(!gone);
    assert_eq!(value.peek().selection().start, 0);
    assert!(value.peek().selection().end > 3, "{:?}", value.peek().selection());
    assert!(p.haptics.borrow().contains(&HapticFeedbackType::TextHandleMove));
    // A tap deselects, the toolbar goes.
    tap(&rule, at(b.left + 9.0 * 8.0, y));
    assert!(value.peek().selection().collapsed() && !p.toolbar_shown());
}

/// The field loses the focus: its selection collapses, its handles and toolbar go.
fn losing_the_focus_deselects() {
    let value = mutable_state_of(TextFieldValue::from_text("bir iki", TextRange::Zero, None));
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || ValueField(v.clone(), "f")));
    let p = ime(&rule);
    rule.wait_for_idle();
    let b = bounds(&rule, "f");
    long_press(&rule, at(b.left + 5.0 * 8.0, b.center().y));
    assert_eq!(value.peek().selection(), TextRange::new(4, 7));
    rule.host().clear_focus(true);
    rule.wait_for_idle();
    assert!(value.peek().selection().collapsed() && !p.toolbar_shown());
}

// ---------------------------------------------------------------------------------------------------------------
// Semantics, enabled, read-only

/// Semantics as CoreTextField sets them: the text, the selection, editable or not, disabled; setting the text and
/// inserting at the cursor through the session; the action key.
fn semantics_and_actions() {
    let value = mutable_state_of("abc".to_string());
    let enabled = mutable_state_of(true);
    let read_only = mutable_state_of(false);
    let (v, e, r) = (value.clone(), enabled.clone(), read_only.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v2 = v.clone();
        BasicTextFieldString(
            v.get(),
            move |it: String| v2.set(it),
            modifier = Modifier.width(300.dp()).test_tag("f"),
            enabled = e.get(),
            read_only = r.get(),
            keyboard_options = KeyboardOptions(ime_action = ImeAction::Search),
        );
    }));
    let _p = ime(&rule);
    rule.wait_for_idle();
    let config = config_of(&rule, "f");
    assert_eq!(config.get(SemanticsProperties::IsEditable), Some(true));
    assert!(config.contains(SemanticsActions::SetText) && config.contains(SemanticsActions::OnImeAction));
    assert_eq!(config.get(SemanticsProperties::ImeAction), Some(ImeAction::Search));
    rule.on_node_with_tag("f").perform_text_replacement("yeni");
    assert_eq!(value.peek(), "yeni");
    rule.on_node_with_tag("f").perform_text_input("!"); // focused: through the session
    assert_eq!(value.peek(), "yeni!");
    read_only.set(true);
    rule.wait_for_idle();
    let config = config_of(&rule, "f");
    assert_eq!(config.get(SemanticsProperties::IsEditable), Some(false));
    assert!(!config.contains(SemanticsActions::SetText));
    type_text(&rule, "x");
    assert_eq!(value.peek(), "yeni!");
    enabled.set(false);
    rule.wait_for_idle();
    assert!(config_of(&rule, "f").contains(SemanticsProperties::Disabled));
    assert!(!is_focused(&rule, "f"));
}

/// Autofill as CoreTextField has it: the content type from the keyboard type, the field's text told the service,
/// a fill (through the session's processor while focused) given to the app, the highlight drawn until the text
/// changes again; a read-only field keeps its text.
fn autofill_fills_value_fields() {
    #[derive(Default)]
    struct Autofill {
        values: RefCell<Vec<String>>,
    }
    impl PlatformCallbacks for Autofill {
        fn autofill_value(&self, _id: u64, text: &str) {
            self.values.borrow_mut().push(text.to_string());
        }
    }
    let user = mutable_state_of(String::new());
    let password = mutable_state_of(String::new());
    let locked = mutable_state_of("sabit".to_string());
    let (u, p, l) = (user.clone(), password.clone(), locked.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (u, p, l) = (u.clone(), p.clone(), l.clone());
        // (the desktop has no highlight color of its own: one given)
        let yellow = Color::new(1.0, 1.0, 0.0, 0.3);
        KompozitionLocalProvider(&[LocalAutofillHighlightColor.provides(yellow)], move || {
            let (u, p, l) = (u.clone(), p.clone(), l.clone());
            Column(move || {
                let (u2, p2, l2) = (u.clone(), p.clone(), l.clone());
                BasicTextFieldString(
                    u.get(),
                    move |it: String| u2.set(it),
                    modifier = Modifier.width(200.dp()).test_tag("kullanıcı"),
                    text_style = style16(),
                    keyboard_options = KeyboardOptions(keyboard_type = kompoz_ui::platform::KeyboardType::Email),
                );
                BasicTextFieldString(
                    p.get(),
                    move |it: String| p2.set(it),
                    modifier = Modifier.width(200.dp()).test_tag("parola"),
                    text_style = style16(),
                    keyboard_options = KeyboardOptions(keyboard_type = kompoz_ui::platform::KeyboardType::Password),
                    visual_transformation = PasswordVisualTransformation(),
                );
                BasicTextFieldString(
                    l.get(),
                    move |it: String| l2.set(it),
                    modifier = Modifier.width(200.dp()).test_tag("sabit"),
                    read_only = true,
                );
            });
        });
    }));
    let a = Rc::new(Autofill::default());
    rule.host().set_callbacks(a.clone());
    rule.wait_for_idle();
    let config = |tag: &str| config_of(&rule, tag);
    let hints = |tag: &str| config(tag).get(SemanticsProperties::ContentType).map(|c| c.hints);
    assert_eq!(hints("kullanıcı"), Some(vec!["emailAddress"]));
    assert!(hints("parola") == Some(vec!["password"]) && config("parola").contains(SemanticsProperties::Password));
    let id = |tag: &str| rule.on_node_with_tag(tag).fetch_semantics_node().id();
    let fill = |field: u64, text: &str| {
        let filled = rule.host().perform_autofill(field, FillableData::Text(text.to_string()));
        rule.wait_for_idle();
        filled
    };
    // Focused: filled through its session; not focused: as a value with the caret at its end.
    rule.on_node_with_tag("kullanıcı").perform_click();
    assert!(fill(id("kullanıcı"), "ayse@example.com") && fill(id("parola"), "gizli"));
    assert!(user.peek() == "ayse@example.com" && password.peek() == "gizli");
    assert!(a.values.borrow().contains(&"ayse@example.com".to_string()));
    // The highlight over both fields, until the text changes again.
    let highlights = |canvas: &RecordingCanvas| {
        canvas.rect_colors.iter().filter(|c| c.alpha() > 0.0 && c.alpha() < 1.0 && c.red() > 0.9).count()
    };
    assert_eq!(highlights(&rule.draw()), 2);
    type_text(&rule, "!");
    assert_eq!(user.peek(), "ayse@example.com!");
    assert_eq!(highlights(&rule.draw()), 1);
    fill(id("sabit"), "x");
    assert_eq!(locked.peek(), "sabit");
}

/// The cursor blinks while the field and its window have the focus: drawn at the caret, hidden half a second
/// later, shown again at an edit.
fn the_cursor_blinks_at_the_caret() {
    let value = mutable_state_of("ab".to_string());
    let blink = mutable_state_of(false);
    let (v, k) = (value.clone(), blink.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v = v.clone();
        KompozitionLocalProvider(&[LocalCursorBlinkEnabled.provides(k.get())], move || StringField(v.clone(), "f"));
    }));
    let _p = ime(&rule);
    rule.on_node_with_tag("f").perform_click();
    let b = bounds(&rule, "f");
    // Not blinking: a 2 px line (a butt-capped stroke: drawn as a path, its bounds its middle) at the caret, 2
    // characters in (x 16 to 18: at 17), the line's height; after an edit at the new caret.
    let caret = cursors(&rule.draw());
    assert_eq!(caret.len(), 1, "{caret:?}");
    assert!(near(caret[0].left - b.left, 17.0, 0.01) && near(caret[0].height(), 20.0, 0.01), "{caret:?}");
    type_text(&rule, "c");
    let caret = cursors(&rule.draw());
    assert!(caret.len() == 1 && near(caret[0].left - b.left, 25.0, 0.01), "{caret:?}");
    // Blinking: shown at an edit, hidden half a second later, shown again half a second after.
    blink.set(true);
    rule.wait_for_idle();
    rule.host().text_input("d");
    rule.advance_frame();
    assert_eq!(cursors(&rule.draw()).len(), 1);
    rule.advance_time_by(600);
    assert!(cursors(&rule.draw()).is_empty());
    rule.advance_time_by(500);
    assert_eq!(cursors(&rule.draw()).len(), 1);
    // A selection: no cursor.
    key(&rule, Key::A, CTRL);
    assert!(cursors(&rule.draw()).is_empty());
}

/// The interaction source hears the presses and the focus of the field.
fn the_interaction_source_hears_presses_and_focus() {
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let value = mutable_state_of(String::new());
    let v = value.clone();
    let pressed_seen = Rc::new(Cell::new(false));
    let focused_seen = Rc::new(Cell::new(false));
    let (ps, fs) = (pressed_seen.clone(), focused_seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let v2 = v.clone();
        if s.collect_is_pressed_as_state().get() {
            ps.set(true);
        }
        if s.collect_is_focused_as_state().get() {
            fs.set(true);
        }
        BasicTextFieldString(
            v.get(),
            move |it: String| v2.set(it),
            modifier = Modifier.width(300.dp()).test_tag("f"),
            interaction_source = Some(s.clone()),
        );
    }));
    let _p = ime(&rule);
    let b = bounds(&rule, "f");
    rule.host().pointer_down(b.center(), -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.advance_frame();
    rule.host().pointer_up(b.center(), -1, 0);
    rule.wait_for_idle();
    assert!(pressed_seen.get() && focused_seen.get());
}

/// The legacy prepared selection's kept column survives vertical moves only.
fn prepared_selection_keeps_the_column_for_vertical_moves() {
    let mut state = TextPreparedSelectionState::new();
    state.cached_x = Some(12.0);
    state.reset_cached_x();
    assert_eq!(state.cached_x, None);
}

tests!(
    typing_goes_through_the_input_session_in_utf8,
    input_method_compositions_and_corrections,
    only_text_changes_are_text_changes,
    a_field_whose_value_comes_back_a_frame_later_keeps_what_is_typed,
    app_side_change_resyncs_input_method,
    the_session_follows_the_focus,
    its_input_method_requests_are_intercepted,
    editing_keys_selection_and_clipboard,
    editing_keys_go_by_words_and_paragraphs,
    tab_and_enter,
    legacy_undo_manager_snapshots,
    a_field_whose_text_the_app_keeps_undoes_too,
    an_undo_the_app_refuses_leaves_the_text,
    the_value_overload_keeps_the_apps_selection,
    password_visual_transformation,
    phone_number_mask_maps_offsets,
    a_faulty_offset_mapping_panics,
    scroller_position_keeps_the_cursor_in_view,
    single_line_field_scrolls_to_the_cursor,
    multi_line_field_grows_then_scrolls,
    mouse_selects_words_paragraphs_and_extends,
    touch_selects_with_handles_and_the_toolbar,
    losing_the_focus_deselects,
    semantics_and_actions,
    autofill_fills_value_fields,
    the_cursor_blinks_at_the_caret,
    the_interaction_source_hears_presses_and_focus,
    prepared_selection_keeps_the_column_for_vertical_moves,
);

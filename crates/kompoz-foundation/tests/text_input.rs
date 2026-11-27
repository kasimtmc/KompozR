//! The state-based text field's model and its input plumbing (WP2a): `TextFieldState` and `TextFieldBuffer`, the
//! change list, tracked styles, undo, input and output transformations, the transformed state's offset mapping,
//! key mappings and the key handler, dead keys, keyboard options and actions, and an input method session
//! established from a node — the C++ Kompoz's text field tests' model halves (their composable halves are
//! BasicTextField's, WP2b's) and Compose's behaviour where the two differ.
//!
//! The field here is a test node (`ModelField`): what BasicTextField's decorator node will be, minimal — focusable,
//! its key events to the key handler, its semantics' text actions to the transformed state, an input method session
//! while focused, its text laid out by the test font (FakeTextMeasurer's numbers) into its text layout state.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::input::internal::*;
use kompoz_foundation::text::input::*;
use kompoz_foundation::text::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_text::{AnnotatedString, SpanStyle, TextRange, TextStyle};
use kompoz_ui::clipboard::ClipEntry;
use kompoz_ui::focus::{FocusEventModifierNode, FocusState};
use kompoz_ui::input::*;
use kompoz_ui::layout::{GlobalPositionAwareModifierNode, LayoutCoordinates, LayoutModifierNode, Measurable};
use kompoz_ui::modifier::{ModifierNode, ModifierNodeElement, NodeCx, NodeHandle};
use kompoz_ui::platform::{
    ImeAction, ImeOptions, LocalClipboard, LocalFontFamilyResolver, LocalSoftwareKeyboardController, PlatformCallbacks,
    TextEditState,
};
use kompoz_ui::platform_text_input::{PlatformTextInputModifierNode, establish_text_input_session};
use kompoz_ui::semantics::{
    AccessibilityAction, SemanticsActions, SemanticsConfiguration, SemanticsModifierNode, SemanticsProperties,
};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// The test field

struct FieldConfig {
    state: TransformedTextFieldState,
    layout: Rc<TextLayoutState>,
    single_line: bool,
    editable: bool,
    ime_options: ImeOptions,
    on_ime_action: Rc<dyn Fn(ImeAction)>,
    on_submit: Rc<dyn Fn() -> bool>,
}

#[derive(Clone)]
struct FieldElement(Rc<FieldConfig>);

impl PartialEq for FieldElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl ModifierNodeElement for FieldElement {
    type Node = FieldNode;
    fn create(&self) -> FieldNode {
        FieldNode { config: self.0.clone(), keys: TextFieldKeyEventHandler::new(), handle: None, session: None }
    }
    fn update(&self, node: &mut FieldNode) {
        node.config = self.0.clone();
    }
}

struct FieldNode {
    config: Rc<FieldConfig>,
    keys: TextFieldKeyEventHandler,
    handle: Option<NodeHandle>,
    session: Option<kompoz_coroutines::Job>,
}

impl ModifierNode for FieldNode {
    node_capabilities!(layout, semantics, key_input, focus_event, global_position_aware);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        if let Some(job) = self.session.take() {
            job.cancel();
        }
    }
}

impl PlatformTextInputModifierNode for FieldNode {}

impl FieldNode {
    fn cx(&self) -> NodeCx {
        self.handle.as_ref().and_then(|h| h.cx()).expect("attached")
    }

    /// The input session while focused (BasicTextField's `startInputSession`).
    fn start_input_session(&mut self) {
        let cx = self.cx();
        let c = self.config.clone();
        let session = establish_text_input_session(self, &cx, move |scope| {
            let on_ime_action = c.on_ime_action.clone();
            platform_specific_text_input_session(
                &scope,
                c.state.clone(),
                c.layout.clone(),
                c.ime_options.clone(),
                None,
                Some(on_ime_action),
            )
        });
        self.session = Some(cx.coroutine_scope().launch(async move { match session.await {} }));
    }
}

impl FocusEventModifierNode for FieldNode {
    fn on_focus_event(&mut self, state: FocusState) {
        if state.is_focused() {
            if self.session.is_none() && self.config.editable {
                self.start_input_session();
            }
        } else if let Some(job) = self.session.take() {
            job.cancel();
        }
    }
}

impl kompoz_ui::input::KeyInputModifierNode for FieldNode {
    fn on_key_event(&mut self, event: &KeyEvent) -> bool {
        let cx = self.cx();
        let clipboard = cx.current_value_of(&LocalClipboard);
        let state = self.config.state.clone();
        let handler: ClipboardKeyCommandsHandler = Rc::new(move |command, _| {
            let selected = state.visual_text().get_selected_text().to_string();
            match command {
                KeyCommand::COPY => {
                    if !selected.is_empty() {
                        clipboard.set_clip_entry_now(Some(ClipEntry::with_plain_text(selected)));
                    }
                }
                KeyCommand::CUT => {
                    if !selected.is_empty() {
                        clipboard.set_clip_entry_now(Some(ClipEntry::with_plain_text(selected)));
                        state.delete_selected_text(true);
                    }
                }
                KeyCommand::PASTE => {
                    if let Some(text) = clipboard.get_clip_entry_now().and_then(|e| e.read_text()) {
                        state.replace_selected_text(&text, true, TextFieldEditUndoBehavior::NeverMerge, true, true);
                    }
                }
                _ => {}
            }
        });
        let keyboard = cx.current_value_of(&LocalSoftwareKeyboardController);
        let c = self.config.clone();
        let on_submit = c.on_submit.clone();
        self.keys.on_key_event(event, &c.state, &c.layout, &handler, &keyboard, c.editable, c.single_line, move || {
            on_submit()
        })
    }

    fn on_pre_key_event(&mut self, event: &KeyEvent) -> bool {
        let state = self.config.state.clone();
        self.keys.on_pre_key_event(event, &state, || {})
    }
}

impl SemanticsModifierNode for FieldNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        let c = self.config.clone();
        let output = c.state.output_text();
        config.set_editable_text(AnnotatedString::new(output.text()));
        config.set_text_selection_range(output.selection());
        config.set_is_editable(c.editable);
        let s = c.state.clone();
        let set_text: Rc<dyn Fn(AnnotatedString) -> bool> = Rc::new(move |text: AnnotatedString| {
            s.replace_all(text.text());
            true
        });
        config.set(SemanticsActions::SetText, AccessibilityAction::new(None, Some(set_text)));
        let s = c.state.clone();
        let insert: Rc<dyn Fn(AnnotatedString) -> bool> = Rc::new(move |text: AnnotatedString| {
            s.replace_selected_text(text.text(), true, TextFieldEditUndoBehavior::MergeIfPossible, true, false);
            true
        });
        config.set(SemanticsActions::InsertTextAtCursor, AccessibilityAction::new(None, Some(insert)));
        let s = c.state.clone();
        let select: Rc<dyn Fn(i32, i32, bool) -> bool> = Rc::new(move |start: i32, end: i32, relative: bool| {
            let range = TextRange::new(start.max(0) as usize, end.max(0) as usize);
            if relative {
                s.select_untransformed_chars_in(range);
            } else {
                s.select_chars_in(range);
            }
            true
        });
        config.set(SemanticsActions::SetSelection, AccessibilityAction::new(None, Some(select)));
        let action = c.ime_options.ime_action;
        let on_ime_action = c.on_ime_action.clone();
        let perform: Rc<dyn Fn() -> bool> = Rc::new(move || {
            on_ime_action(action);
            true
        });
        config.set(SemanticsProperties::ImeAction, action);
        config.set(SemanticsActions::OnImeAction, AccessibilityAction::new(None, Some(perform)));
    }
}

impl GlobalPositionAwareModifierNode for FieldNode {
    fn on_globally_positioned(&mut self, coordinates: &LayoutCoordinates) {
        let layout = &self.config.layout;
        layout.set_decorator_node_coordinates(Some(coordinates.clone()));
        layout.set_core_node_coordinates(Some(coordinates.clone()));
        layout.set_text_layout_node_coordinates(Some(coordinates.clone()));
    }
}

impl LayoutModifierNode for FieldNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let resolver = self.cx().current_value_of(&LocalFontFamilyResolver);
        let density = Density::new(scope.density, scope.font_scale);
        let style = TextStyle { font_size: 16.sp(), ..TextStyle::Default };
        let constraints_for_text = Constraints::new(0, constraints.max_width, 0, Constraints::Infinity);
        let layout = &self.config.layout;
        layout.update_non_measure_inputs(
            &self.config.state,
            &style,
            self.config.single_line,
            !self.config.single_line,
            &KeyboardOptions::Default,
        );
        let result =
            layout.layout_with_new_measure_inputs(density, scope.layout_direction(), resolver, constraints_for_text);
        let size = result.size;
        let placeable = measurable.measure(Constraints::fixed(size.width.max(1), size.height.max(1)));
        let (w, h) = (constraints.constrain_width(size.width.max(1)), constraints.constrain_height(size.height.max(1)));
        scope.layout(w, h, move |p| p.place(&placeable, 0, 0))
    }
}

/// What a test field is made with.
struct Field {
    transformed: TransformedTextFieldState,
    layout: Rc<TextLayoutState>,
    single_line: bool,
    editable: bool,
    ime_options: ImeOptions,
    actions: Rc<RefCell<Vec<ImeAction>>>,
    submits: Rc<Cell<i32>>,
}

impl Field {
    fn new(state: &TextFieldState) -> Field {
        Field::with(state, None, None, None)
    }

    fn with(
        state: &TextFieldState,
        input: Option<InputTransformation>,
        output: Option<Rc<dyn OutputTransformation>>,
        codepoints: Option<Rc<dyn CodepointTransformation>>,
    ) -> Field {
        Field {
            transformed: TransformedTextFieldState::new(state.clone(), input, codepoints, output),
            layout: Rc::new(TextLayoutState::new()),
            single_line: false,
            editable: true,
            ime_options: ImeOptions { single_line: false, ..ImeOptions::default() },
            actions: Rc::new(RefCell::new(Vec::new())),
            submits: Rc::new(Cell::new(0)),
        }
    }

    fn single_line(mut self) -> Field {
        self.single_line = true;
        self.ime_options.single_line = true;
        self
    }

    fn read_only(mut self) -> Field {
        self.editable = false;
        self
    }

    fn config(&self) -> Rc<FieldConfig> {
        let actions = self.actions.clone();
        let submits = self.submits.clone();
        Rc::new(FieldConfig {
            state: self.transformed.clone(),
            layout: self.layout.clone(),
            single_line: self.single_line,
            editable: self.editable,
            ime_options: self.ime_options.clone(),
            on_ime_action: Rc::new(move |a| actions.borrow_mut().push(a)),
            on_submit: Rc::new(move || {
                submits.set(submits.get() + 1);
                true
            }),
        })
    }
}

/// The test field in a composition: the config remembered once (the node keeps its session).
#[kompozable]
fn ModelField(config: Rc<FieldConfig>, tag: &'static str) {
    Box(modifier = Modifier.test_tag(tag).then_element(FieldElement(config)).focusable().width(300.dp()));
}

/// A platform recording what the host tells it of text input, and holding a clipboard.
#[derive(Default)]
struct Ime {
    shown: RefCell<Vec<(bool, ImeOptions)>>,
    changed: RefCell<Vec<TextEditState>>,
    clipboard: RefCell<String>,
    hidden: Cell<i32>,
}

impl PlatformCallbacks for Ime {
    fn text_input(&self, active: bool, _area: Rect, _state: &TextEditState, options: &ImeOptions) {
        self.shown.borrow_mut().push((active, options.clone()));
    }
    fn text_input_changed(&self, state: &TextEditState) {
        self.changed.borrow_mut().push(state.clone());
    }
    fn get_clip_entry(&self) -> Option<kompoz_ui::clipboard::ClipEntry> {
        Some(kompoz_ui::clipboard::ClipEntry::with_plain_text(self.clipboard.borrow().clone()))
    }
    fn set_clip_entry(&self, entry: Option<kompoz_ui::clipboard::ClipEntry>) {
        *self.clipboard.borrow_mut() = entry.and_then(|e| e.read_plain_text()).unwrap_or_default();
    }
    fn hide_software_keyboard(&self) {
        self.hidden.set(self.hidden.get() + 1);
    }
}

impl Ime {
    fn told(&self) -> usize {
        self.changed.borrow().len()
    }
    fn last(&self) -> TextEditState {
        self.changed.borrow().last().cloned().unwrap_or_default()
    }
}

/// A rule showing `fields` (tagged "f0", "f1", …) in a column, with a recording platform.
fn rule_with(fields: Vec<Rc<FieldConfig>>) -> (KompozTestRule, Rc<Ime>) {
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let fields = fields.clone();
        Column(move || {
            for (i, config) in fields.iter().enumerate() {
                let tag: &'static str = ["f0", "f1", "f2", "f3"][i];
                ModelField(config.clone(), tag);
            }
        });
    }));
    let ime = Rc::new(Ime::default());
    rule.host().set_callbacks(ime.clone());
    (rule, ime)
}

fn focus(rule: &KompozTestRule, tag: &str) {
    rule.on_node_with_tag(tag).perform_semantics_action(kompoz_ui::semantics::SemanticsActions::RequestFocus);
}

fn key(rule: &KompozTestRule, key: Key, modifiers: KeyModifiers) {
    rule.host().key_down(key, modifiers);
    rule.host().key_up(key, modifiers);
    rule.wait_for_idle();
}

/// Types `text` a character at a time, as a keyboard's typing reaches the input session.
fn type_text(rule: &KompozTestRule, text: &str) {
    for c in text.chars() {
        rule.host().text_input(&c.to_string());
    }
    rule.wait_for_idle();
}

const CTRL: KeyModifiers = KeyModifiers::Ctrl;
const SHIFT: KeyModifiers = KeyModifiers::Shift;
const NONE: KeyModifiers = KeyModifiers::None;

fn ctrl_shift() -> KeyModifiers {
    KeyModifiers::Ctrl | KeyModifiers::Shift
}

// ---------------------------------------------------------------------------------------------------------------
// TextFieldBuffer and its change list

/// C++ 20998: a buffer's edits move its selection as Compose's do.
fn text_field_buffer_edits_and_moves_the_selection() {
    let mut b = TextFieldBuffer::of(TextFieldCharSequence::new("merhaba dünya", TextRange::at(7), None));
    assert!(b.changes().change_count() == 0 && b.length() == 14);
    b.insert(0, ">> "); // before the caret: it moves along
    assert_eq!((b.to_string().as_str(), b.selection()), (">> merhaba dünya", TextRange::at(10)));
    b.append("!"); // after it: it stays
    assert_eq!(b.selection(), TextRange::at(10));
    b.set_selection(TextRange::new(3, 10)); // "merhaba"
    b.replace(3, 10, "selam"); // what covered the replaced text covers the new one
    assert_eq!((b.to_string().as_str(), b.selection()), (">> selam dünya!", TextRange::new(3, 8)));
    b.set_selection(TextRange::at(5));
    b.replace(3, 8, "hey"); // a caret inside the replaced text goes to its end
    assert_eq!((b.to_string().as_str(), b.selection()), (">> hey dünya!", TextRange::at(6)));
    b.delete(0, 3);
    assert_eq!((b.to_string().as_str(), b.selection()), ("hey dünya!", TextRange::at(3)));
    b.select_all();
    assert!(b.selection() == TextRange::new(0, b.length()) && b.has_selection());
    b.place_cursor_at_end();
    assert!(b.selection() == TextRange::at(b.length()) && b.changes().change_count() > 0);
    b.revert_all_changes();
    assert_eq!(
        (b.to_string().as_str(), b.selection(), b.original_text()),
        ("merhaba dünya", TextRange::at(7), "merhaba dünya")
    );
    assert_eq!(b.changes().change_count(), 0);
    // (C++ clamped a selection out of the text; Compose throws: an assertion here.)
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut b = TextFieldBuffer::of(TextFieldCharSequence::new("ab", TextRange::Zero, None));
        b.set_selection(TextRange::new(0, 9));
    }));
    assert!(out.is_err());
    let backwards = TextRange::new(9, 2);
    assert!(backwards.min() == 2 && backwards.max() == 9 && backwards.length() == 7 && !backwards.collapsed());
    // Offsets inside a character move to its boundaries: a caret back, a range out.
    let mut b = TextFieldBuffer::of(TextFieldCharSequence::new("aüb", TextRange::Zero, None));
    b.place_cursor_before_char_at(2);
    assert_eq!(b.selection(), TextRange::at(1));
    b.place_cursor_after_char_at(1);
    assert_eq!(b.selection(), TextRange::at(3)); // after "ü" (two bytes)
    b.set_selection(TextRange::new(2, 3));
    assert_eq!(b.selection(), TextRange::new(1, 3));
    assert_eq!(b.char_at(2), 'ü');
}

/// Compose's ChangeTracker: changes in the original text's terms, merged when they touch.
fn change_lists_merge_touching_changes() {
    let mut b = TextFieldBuffer::of(TextFieldCharSequence::new("abcdef", TextRange::Zero, None));
    b.replace(1, 2, "XY"); // b → XY
    b.insert(0, "_");
    let ranges = |b: &TextFieldBuffer| {
        (0..b.changes().change_count())
            .map(|i| (b.changes().get_original_range(i), b.changes().get_range(i)))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ranges(&b),
        [(TextRange::new(0, 0), TextRange::new(0, 1)), (TextRange::new(1, 2), TextRange::new(2, 4))]
    );
    b.delete(4, 6); // "cd" after "XY": touches it
    assert_eq!(ranges(&b)[1], (TextRange::new(1, 4), TextRange::new(2, 4)));
    assert_eq!(format!("{:?}", b.changes()), "ChangeList(changes=[(0,0)->(0,1), (1,4)->(2,4)])");
    let mut seen = Vec::new();
    b.changes().for_each_change_reversed(|range, original| seen.push((range, original)));
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].1, TextRange::new(1, 4));
    // A no-op isn't a change.
    let mut b = TextFieldBuffer::of(TextFieldCharSequence::new("ab", TextRange::Zero, None));
    b.replace(1, 1, "");
    assert_eq!(b.changes().change_count(), 0);
}

/// Compose's OffsetMappingCalculator examples (its documentation's tables), in bytes.
fn offset_mapping_maps_both_ways() {
    let r = TextRange::new;
    let at = TextRange::at;
    // Inserting "zzz" into "ab" at 1.
    let mut m = OffsetMappingCalculator::new();
    m.record_edit_operation(1, 1, 3);
    assert_eq!([0, 1, 2].map(|o| m.map_from_source(o)), [at(0), r(1, 4), at(5)]);
    assert_eq!([0, 1, 2, 3, 4, 5].map(|o| m.map_from_dest(o)), [at(0), at(1), at(1), at(1), at(1), at(2)]);
    // Replacing "bcd" of "abcde" with "zz".
    let mut m = OffsetMappingCalculator::new();
    m.record_edit_operation(1, 4, 2);
    assert_eq!([0, 1, 2, 3, 4, 5].map(|o| m.map_from_source(o)), [at(0), at(1), r(1, 3), r(1, 3), at(3), at(4)]);
    assert_eq!([0, 1, 2, 3, 4].map(|o| m.map_from_dest(o)), [at(0), at(1), r(1, 4), at(4), at(5)]);
    // Multiple inserts: "abcd" → "a(bc)d".
    let mut m = OffsetMappingCalculator::new();
    m.record_edit_operation(1, 1, 1);
    m.record_edit_operation(4, 4, 1);
    assert_eq!([0, 1, 2, 3, 4].map(|o| m.map_from_source(o)), [at(0), r(1, 2), at(3), r(4, 5), at(6)]);
    // A one-byte replacement by one byte maps one to one (and isn't kept).
    let mut m = OffsetMappingCalculator::new();
    m.record_edit_operation(1, 2, 1);
    assert_eq!(m.map_from_source(2), at(2));
}

// ---------------------------------------------------------------------------------------------------------------
// Tracked styles

fn bold() -> SpanStyle {
    SpanStyle { font_weight: Some(kompoz_text::FontWeight::Bold), ..SpanStyle::default() }
}

/// Compose 1.12's tracked styles: ranges follow the edits by their expand policy, die when deleted away, are
/// queried by overlap (a collapsed query meets what starts at it), and become the state's text styles.
fn tracked_styles_follow_the_edits() {
    let state = TextFieldState::new("hello world");
    let mut tracked = None;
    state.edit(|b| {
        let t = b.add_style_tracked(bold(), TextRange::new(0, 5), ExpandPolicy::AtEnd);
        assert!(b.is_valid(t));
        // Typed at its end: taken in (AtEnd); at its start: not.
        b.insert(5, "!");
        b.insert(0, ">");
        assert_eq!(b.text_range(t), TextRange::new(1, 7));
        b.set_expand_policy(t, ExpandPolicy::InsideOnly);
        b.insert(7, "?");
        assert_eq!(b.text_range(t), TextRange::new(1, 7));
        assert_eq!(b.get_span_styles(TextRange::at(1)), [t]); // a collapsed query at its start meets it
        assert!(b.get_span_styles(TextRange::at(7)).is_empty()); // at its end doesn't
        assert!(b.get_span_styles(TextRange::new(7, 9)).is_empty());
        assert_eq!(b.get_paragraph_styles(TextRange::new(0, 9)), []);
        tracked = Some(t);
    });
    let styles = state.text_styles();
    let spans = styles.get_span_styles(TextRange::new(0, 3));
    assert_eq!((spans.len(), spans[0].start, spans[0].end), (1, 1, 7));
    // A handle is its buffer's: another edit's buffer doesn't know it.
    state.edit(|b| {
        assert!(!b.is_valid(tracked.unwrap()));
        let t = b.get_span_styles(TextRange::new(0, 3))[0];
        // Deleted away: gone.
        b.delete(0, 8);
        assert!(!b.is_valid(t));
        assert_eq!(b.text_range(t), TextRange::Zero);
    });
    assert!(state.text_styles().get_span_styles(TextRange::new(0, 5)).is_empty());
    // Removing, changing a style, and reverting all changes brings the original styles back.
    let state = TextFieldState::new("abc");
    state.edit(|b| b.add_style(bold(), 0, 2));
    state.edit(|b| {
        let t = b.get_span_styles(TextRange::new(0, 3))[0];
        b.set_span_style(t, SpanStyle::default());
        assert_eq!(b.span_style(t), SpanStyle::default());
        b.set_text_range(t, TextRange::new(1, 3));
        assert!(b.remove_style(t) && !b.remove_style(t));
        b.revert_all_changes();
        assert_eq!(b.get_span_styles(TextRange::new(0, 3)).len(), 1);
    });
    // Transitions: where a style starts or ends.
    let mut s = TextStyleBuffer::new();
    s.replace_text(0, 0, 0);
    let mut b = TextFieldBuffer::of(TextFieldCharSequence::new("0123456789", TextRange::Zero, None));
    b.add_style(bold(), 2, 4);
    b.add_style(bold(), 6, 8);
    let styles = b.to_text_field_char_sequence().text_field_text_styles().cloned().unwrap();
    let buffer = styles.text_style_buffer();
    assert_eq!(
        (buffer.next_transition(0, 10), buffer.next_transition(4, 10), buffer.next_transition(8, 10)),
        (2, 6, 10)
    );
    assert_eq!(
        (buffer.previous_transition(10, 0), buffer.previous_transition(6, 0), buffer.previous_transition(2, 0)),
        (8, 4, 0)
    );
}

// ---------------------------------------------------------------------------------------------------------------
// TextFieldState

/// C++ 20976 (its model half): the state holds the text; typing through the input session changes it, the app's
/// change tells the input method.
fn text_field_state_holds_the_text() {
    let state = TextFieldState::new("merhaba");
    let field = Field::new(&state);
    let (rule, ime) = rule_with(vec![field.config()]);
    focus(&rule, "f0");
    rule.host().text_input(" dünya");
    rule.wait_for_idle();
    assert_eq!(state.text(), "merhaba dünya");
    rule.on_node_with_tag("f0").assert(has_text("merhaba dünya", false, false));
    state.clear_text(); // the app changes it: the keyboard is told
    rule.wait_for_idle();
    assert!(ime.told() > 0 && ime.last().text.is_empty());
}

/// C++ 21025 (its model half): the selection is kept, the input method follows the app's changes.
fn text_field_state_keeps_its_selection() {
    let state = TextFieldState::new("merhaba");
    let field = Field::new(&state).single_line();
    let (rule, ime) = rule_with(vec![field.config()]);
    assert_eq!(state.selection(), TextRange::at(7));
    state.edit(|b| b.place_cursor_before_char_at(3)); // before it is focused
    focus(&rule, "f0"); // focused: the input method starts from that caret
    rule.host().text_input("X");
    rule.wait_for_idle();
    assert_eq!((state.text(), state.selection()), ("merXhaba".to_string(), TextRange::at(4)));
    key(&rule, Key::DirectionLeft, NONE); // the caret moving is kept too
    assert_eq!(state.selection(), TextRange::at(3));
    key(&rule, Key::DirectionRight, SHIFT);
    assert_eq!(state.selection(), TextRange::new(3, 4));
    // Changed by the app while focused: the input method takes the text and the selection.
    state.set_text_and_select_all("yeni metin");
    rule.wait_for_idle();
    let last = ime.last();
    assert_eq!((last.text.as_str(), last.selection_start, last.selection_end), ("yeni metin", 0, 10));
    rule.host().text_input("Z"); // typed over the selection
    rule.wait_for_idle();
    assert_eq!((state.text(), state.selection()), ("Z".to_string(), TextRange::at(1)));
    state.edit(|b| {
        b.append(" sonu");
        b.place_cursor_before_char_at(0);
    });
    rule.wait_for_idle();
    let last = ime.last();
    assert_eq!((last.text.as_str(), last.selection_start, last.selection_end), ("Z sonu", 0, 0));
    state.edit(|b| b.set_selection(TextRange::new(2, 6))); // the selection alone
    rule.wait_for_idle();
    assert_eq!((ime.last().selection_start, ime.last().selection_end), (2, 6));
    rule.host().text_input("dip");
    rule.wait_for_idle();
    assert_eq!(state.text(), "Z dip");
    // Typed, then taken back by the app at once: the input method hears so.
    rule.host().text_input("!");
    state.set_text_and_place_cursor_at_end("Z dip");
    rule.advance_frame();
    assert_eq!((ime.last().text.as_str(), ime.last().selection_end), ("Z dip", 5));
}

/// `remember_text_field_state` saves its text, selection and undo history (Compose's `TextFieldState.Saver`).
fn text_field_state_saves_its_text_selection_and_history() {
    let held: Rc<RefCell<Option<TextFieldState>>> = Rc::new(RefCell::new(None));
    let content = |held: Rc<RefCell<Option<TextFieldState>>>, shown: MutableState<bool>| {
        kompozable_lambda!(move || {
            if shown.get() {
                let state = remember_text_field_state(
                    initial_text = "abc".to_string(),
                    initial_selection = TextRange::new(1, 2),
                );
                *held.borrow_mut() = Some(state);
            }
        })
    };
    let bytes = {
        let rule = KompozTestRule::new(content(held.clone(), mutable_state_of(true)));
        let state = held.borrow().clone().unwrap();
        assert_eq!((state.text(), state.selection()), ("abc".to_string(), TextRange::new(1, 2)));
        state.edit(|b| {
            b.append("d");
        });
        rule.host().save_state()
    };
    let shown = mutable_state_of(false);
    let rule = KompozTestRule::new(content(held.clone(), shown.clone()));
    rule.host().restore_state(&bytes);
    shown.set(true);
    rule.wait_for_idle();
    let restored = held.borrow().clone().unwrap();
    assert_eq!((restored.text(), restored.selection()), ("abcd".to_string(), TextRange::new(1, 2)));
    assert!(restored.undo_state().can_undo());
    restored.undo_state().undo();
    assert_eq!(restored.text(), "abc");
    let state = restored;
    // Nested edits aren't allowed.
    let nested = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| state.edit(|_| state.edit(|_| {}))));
    assert!(nested.is_err());
}

// ---------------------------------------------------------------------------------------------------------------
// Transformations

/// C++ 21258 (its model half, the value-based field's maxLength on a state here): input transformations accept,
/// change or refuse what is typed, and the input method hears of what they changed.
fn input_transformations_change_or_refuse_what_is_typed() {
    let code = TextFieldState::new("");
    let name = TextFieldState::new("");
    let digits = TextFieldState::new("12");
    let only_digits = InputTransformation
        .by_value(|current, proposed| if proposed.chars().all(|c| c.is_ascii_digit()) { proposed } else { current });
    let fields = vec![
        Field::with(&code, Some(InputTransformation.max_length(4)), None, None).single_line().config(),
        Field::with(&name, Some(InputTransformation.all_caps(kompoz_text::Locale::new("tr"))), None, None)
            .single_line()
            .config(),
        Field::with(&digits, Some(only_digits.then(InputTransformation.max_length(5))), None, None)
            .single_line()
            .config(),
    ];
    let (rule, ime) = rule_with(fields);
    // At most four characters (code points, not bytes): a fifth, or more pasted, is refused, the input method told.
    focus(&rule, "f0");
    rule.host().text_input("çğö");
    rule.host().text_input("d");
    rule.wait_for_idle();
    assert_eq!((code.text().as_str(), ime.told()), ("çğöd", 0)); // kept as typed: nothing to tell
    rule.host().text_input("e");
    rule.wait_for_idle();
    assert_eq!((code.text().as_str(), ime.told()), ("çğöd", 1));
    assert_eq!((ime.last().text.as_str(), ime.last().selection_start), ("çğöd", 7));
    key(&rule, Key::Backspace, NONE);
    rule.host().text_input("xyz");
    rule.wait_for_idle();
    assert_eq!(code.text(), "çğö");
    let before = ime.told();
    rule.host().set_composing_text("a", 1); // a word being composed, kept as it is
    rule.wait_for_idle();
    assert_eq!((code.text().as_str(), code.composition(), ime.told()), ("çğöa", Some(TextRange::new(6, 7)), before));
    // Capitals, Turkish: i → İ, ı → I; only what was typed (text set in code stays as it is).
    focus(&rule, "f1");
    rule.host().text_input("istanbul ılık");
    rule.wait_for_idle();
    let caps = "İSTANBUL ILIK";
    assert_eq!((name.text().as_str(), name.selection()), (caps, TextRange::at(caps.len())));
    assert_eq!((ime.last().text.as_str(), ime.last().selection_end), (caps, caps.len()));
    key(&rule, Key::MoveHome, NONE);
    rule.host().text_input("ç");
    rule.wait_for_idle();
    assert_eq!((name.text(), name.selection()), (format!("Ç{caps}"), TextRange::at(2))); // the caret after the capital
    name.set_text_and_place_cursor_at_end("küçük");
    rule.wait_for_idle();
    assert_eq!(name.text(), "küçük");
    rule.host().text_input("ş");
    rule.wait_for_idle();
    assert_eq!(name.text(), "küçükŞ");
    name.edit(|b| {
        let len = b.length();
        b.replace(0, len, "üzüm");
        b.place_cursor_before_char_at(0);
    });
    rule.wait_for_idle();
    rule.host().text_input("ç"); // "ç" before "ü": they share a first byte, only "ç" was typed
    rule.wait_for_idle();
    assert_eq!(name.text(), "Çüzüm");
    // Digits only, then at most five.
    focus(&rule, "f2");
    rule.host().text_input("3");
    rule.host().text_input("a");
    rule.wait_for_idle();
    assert_eq!(digits.text(), "123");
    rule.host().text_input("45");
    rule.host().text_input("6");
    rule.wait_for_idle();
    assert_eq!((digits.text().as_str(), ime.last().text.as_str()), ("12345", "12345"));
    key(&rule, Key::MoveHome, NONE);
    rule.host().text_input("x"); // refused: the caret stays where it was
    rule.wait_for_idle();
    assert_eq!((digits.text().as_str(), digits.selection()), ("12345", TextRange::Zero));
    // by_value's own text replaces the proposed one where it differs (" x" pasted, the caret after it).
    let no_spaces = InputTransformation.by_value(|_, proposed| proposed.replace(' ', ""));
    let mut pasted = TextFieldBuffer::new(
        TextFieldCharSequence::new(" x", TextRange::at(2), None),
        None,
        Some(TextFieldCharSequence::new("", TextRange::Zero, None)),
        None,
    );
    no_spaces.transform_input(&mut pasted);
    assert_eq!((pasted.to_string().as_str(), pasted.selection()), ("x", TextRange::at(1)));
    // Compose's equality and descriptions; the keyboard all_caps asks for; max_length's semantics.
    assert_eq!(InputTransformation.max_length(4), InputTransformation.max_length(4));
    assert_ne!(InputTransformation.max_length(4), InputTransformation.max_length(5));
    assert_eq!(
        format!("{:?}", InputTransformation.max_length(4)),
        "InputTransformation.then(InputTransformation.maxLength(4))"
    );
    let caps_options = InputTransformation.all_caps(kompoz_text::Locale::new("tr")).keyboard_options().unwrap();
    assert_eq!(caps_options.capitalization, kompoz_ui::platform::KeyboardCapitalization::Characters);
    let mut config = SemanticsConfiguration::default();
    only_digits.then(InputTransformation.max_length(5)).apply_semantics(&mut config);
    assert_eq!(config.get(SemanticsProperties::MaxTextLength), Some(5));
}

/// C++ 21077 (its model half): an output transformation formats what is shown; the state keeps the text, offsets
/// map between the two (the caret kept out of what was inserted), semantics read the formatted text. (Where the
/// C++ highlighted a selected amount without its "$", Compose's selection keeps to a wedge's start: the "$" in.)
fn output_transformation_formats_what_is_shown() {
    let phone = TextFieldState::new("5551234567");
    let amount = TextFieldState::new("250");
    let groups: Rc<dyn OutputTransformation> = output_transformation(|b: &mut TextFieldBuffer| {
        for at in [8, 6, 3] {
            if b.length() > at {
                b.insert(at, " ");
            }
        }
    });
    let money: Rc<dyn OutputTransformation> = output_transformation(|b: &mut TextFieldBuffer| {
        b.insert(0, "$");
        b.append(" USD");
    });
    let tel = Field::with(&phone, None, Some(groups), None).single_line();
    let sum = Field::with(&amount, None, Some(money), None).single_line();
    let (tel_state, sum_state) = (tel.transformed.clone(), sum.transformed.clone());
    let (rule, _ime) = rule_with(vec![tel.config(), sum.config()]);
    assert_eq!((tel_state.output_text().text(), sum_state.output_text().text()), ("555 123 45 67", "$250 USD"));
    assert_eq!(phone.text(), "5551234567");
    // Typed at the end: the value grows, the groups follow.
    focus(&rule, "f0");
    rule.host().text_input("8");
    rule.wait_for_idle();
    assert_eq!((phone.text().as_str(), tel_state.output_text().text()), ("55512345678", "555 123 45 678"));
    // A caret just past a separator is before the digit after it; on the separator itself, where it was put.
    tel_state.place_cursor_before_char_at(4);
    rule.wait_for_idle();
    assert_eq!(phone.selection(), TextRange::at(3));
    rule.host().text_input("0");
    rule.wait_for_idle();
    assert_eq!((phone.text().as_str(), tel_state.output_text().text()), ("555012345678", "555 012 34 5678"));
    tel_state.place_cursor_before_char_at(3);
    assert_eq!(phone.selection(), TextRange::at(3));
    assert_eq!(tel_state.map_from_transformed(4), TextRange::at(3));
    assert_eq!(tel_state.map_to_transformed(3), TextRange::new(3, 4)); // an insertion: either side
    // Around a prefix and a suffix: the caret stays between them; Home goes before the digits.
    focus(&rule, "f1");
    assert_eq!(amount.selection(), TextRange::at(3));
    rule.host().text_input("0");
    key(&rule, Key::MoveHome, NONE);
    rule.host().text_input("1");
    rule.wait_for_idle();
    assert_eq!((amount.text().as_str(), sum_state.output_text().text()), ("12500", "$12500 USD"));
    amount.edit(|b| b.select_all());
    rule.wait_for_idle();
    assert_eq!(sum_state.output_text().selection(), TextRange::new(0, 6));
    // Semantics read the formatted text (Compose's outputText); the input method gets the text itself.
    rule.on_node_with_tag("f1").assert(has_text("$12500 USD", false, false));
    let session = rule.host().text_input_service().session_state().unwrap();
    assert_eq!(session.text, "12500");
}

/// A password's mask (Compose's `CodepointTransformation.mask`): every code point shown as the mask, offsets mapped
/// at character boundaries (a two-byte "ç" shows as a three-byte dot).
fn codepoint_transformations_mask_and_flatten() {
    let state = TextFieldState::with_selection("abç", TextRange::at(4));
    let masked = TransformedTextFieldState::new(state.clone(), None, Some(mask('•')), None);
    assert_eq!(masked.visual_text().text(), "•••");
    assert_eq!(masked.visual_text().selection(), TextRange::at(9));
    assert_eq!(masked.map_from_transformed(6), TextRange::at(2));
    masked.select_chars_in(TextRange::new(3, 9));
    assert_eq!(state.selection(), TextRange::new(1, 4));
    assert!(
        same_codepoint_transformation(&*mask('•'), &*mask('•'))
            && !same_codepoint_transformation(&*mask('•'), &*mask('*'))
    );
    let lines = TextFieldState::new("a\nb\r");
    let flat = TransformedTextFieldState::new(lines, None, Some(Rc::new(SingleLineCodepointTransformation)), None);
    assert_eq!(flat.visual_text().text(), "a b\u{FEFF}");
}

/// Compose's `calculateNextCursorPositionAndWedgeAffinity`: a caret moving through what an output transformation
/// inserted goes across it, keeping to the side it came from.
fn carets_move_across_what_was_inserted() {
    let state = TextFieldState::with_selection("ab", TextRange::at(1));
    let inserted: Rc<dyn OutputTransformation> = output_transformation(|b: &mut TextFieldBuffer| b.insert(1, "--"));
    let t = TransformedTextFieldState::new(state, None, None, Some(inserted));
    assert_eq!(t.visual_text().text(), "a--b");
    let forward = calculate_next_cursor_position_and_wedge_affinity(Some(2), 1, &t);
    assert_eq!((forward.cursor, forward.wedge_affinity), (3, Some(WedgeAffinity::End)));
    let back = calculate_next_cursor_position_and_wedge_affinity(Some(2), 3, &t);
    assert_eq!((back.cursor, back.wedge_affinity), (1, Some(WedgeAffinity::Start)));
    assert_eq!(t.get_index_transformation_type(2).0, IndexTransformationType::Insertion);
    assert_eq!(t.get_index_transformation_type(0).0, IndexTransformationType::Untransformed);
    let none = calculate_next_cursor_position_and_wedge_affinity(None, 1, &t);
    assert_eq!((none.cursor, none.wedge_affinity), (1, None));
}

// ---------------------------------------------------------------------------------------------------------------
// Undo

/// Compose's TextUndoOperation merge rules: insertions one after another and deletions the same way from the same
/// place merge within five seconds; a line break, an edit not allowed to merge, a pause, a jump don't.
fn undo_operations_merge_as_composes() {
    let at = TextRange::at;
    let insert = |index: usize, text: &str, time: i64| {
        TextUndoOperation::at_time(index, "", text, at(index), at(index + text.len()), time, true)
    };
    let merged = insert(0, "a", 0).merge(&insert(1, "b", 100)).unwrap();
    assert_eq!((merged.post_text.as_str(), merged.post_selection), ("ab", at(2)));
    assert!(insert(0, "a", 0).merge(&insert(5, "b", 100)).is_none()); // not where it ended
    assert!(insert(0, "a", 0).merge(&insert(1, "b", SNAPSHOTS_INTERVAL_MILLIS)).is_none()); // too late
    assert!(insert(0, "a", 0).merge(&insert(1, "\n", 10)).is_none()); // a line break
    let fixed = TextUndoOperation::at_time(0, "", "a", at(0), at(1), 0, false);
    assert!(fixed.merge(&insert(1, "b", 10)).is_none());
    // Backspaces merge backwards, deletes forwards; a selection deleted doesn't.
    let backspace = |index: usize, text: &str| {
        TextUndoOperation::at_time(index, text, "", at(index + text.len()), at(index), 0, true)
    };
    let b = backspace(3, "d").merge(&backspace(2, "c")).unwrap();
    assert_eq!((b.index, b.pre_text.as_str(), b.deletion_type()), (2, "cd", TextDeleteType::Start));
    let delete = |index: usize, text: &str| TextUndoOperation::at_time(index, text, "", at(index), at(index), 0, true);
    assert_eq!(delete(2, "c").merge(&delete(2, "d")).unwrap().pre_text, "cd");
    let selected = TextUndoOperation::at_time(1, "bc", "", TextRange::new(1, 3), at(1), 0, true);
    assert_eq!(selected.deletion_type(), TextDeleteType::Inner);
    assert!(selected.merge(&delete(1, "d")).is_none());
    assert_eq!(TextUndoOperation::at_time(0, "a", "b", at(0), at(1), 0, true).text_edit_type(), TextEditType::Replace);
}

/// A status reading only the undo state, in a scope of its own: undoing must tell it, not just the text.
#[kompozable]
fn UndoStatus(state: TextFieldState) {
    let undo = state.undo_state();
    Text(format!("{} {}", undo.can_undo(), undo.can_redo()), modifier = Modifier.test_tag("durum"));
}

/// C++ 21169, Compose's rules: typing goes into one step while it goes on (Compose doesn't split at words: the
/// C++ did), a line break is a step of its own and so is what comes after it, an edit in code is a step of its
/// own and drops what could be redone; Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z.
fn text_field_state_undoes_typing_a_run_at_a_time() {
    let state = TextFieldState::new("");
    let field = Field::new(&state);
    let config = field.config();
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (config, s) = (config.clone(), s.clone());
        Column(move || {
            ModelField(config.clone(), "f0");
            UndoStatus(s.clone());
        });
    }));
    let ime = Rc::new(Ime::default());
    rule.host().set_callbacks(ime.clone());
    let status = || {
        rule.on_node_with_tag("durum").fetch_semantics_node().config().get(SemanticsProperties::Text).unwrap()[0]
            .text()
            .to_string()
    };
    assert_eq!(status(), "false false");
    focus(&rule, "f0");
    type_text(&rule, "merhaba dünya");
    assert_eq!((state.text().as_str(), status().as_str()), ("merhaba dünya", "true false"));
    key(&rule, Key::Z, NONE); // Z alone isn't undo
    assert_eq!(state.text(), "merhaba dünya");
    key(&rule, Key::Z, CTRL); // the run typed, undone at once
    assert_eq!((state.text().as_str(), state.selection(), status().as_str()), ("", TextRange::Zero, "false true"));
    key(&rule, Key::Y, CTRL);
    key(&rule, Key::Z, ctrl_shift()); // nothing more to redo
    assert_eq!(
        (state.text().as_str(), status().as_str(), ime.last().text.as_str()),
        ("merhaba dünya", "true false", "merhaba dünya")
    );
    // Backspace three times: one step back.
    for _ in 0..3 {
        rule.host().key_down(Key::Backspace, NONE);
    }
    rule.wait_for_idle();
    assert_eq!(state.text(), "merhaba dü");
    key(&rule, Key::Z, CTRL);
    assert_eq!(state.text(), "merhaba dünya");
    // A line break is a step of its own, and so is what comes after it.
    type_text(&rule, "\nso");
    key(&rule, Key::Z, CTRL);
    assert_eq!(state.text(), "merhaba dünya\n");
    key(&rule, Key::Z, CTRL);
    assert_eq!(state.text(), "merhaba dünya");
    // An edit made in code is a step too (not merged with the typing), and a new edit drops what could be redone.
    state.edit(|b| {
        b.append("!");
        b.place_cursor_at_end();
    });
    rule.wait_for_idle();
    type_text(&rule, "?");
    key(&rule, Key::Z, CTRL);
    assert_eq!((state.text().as_str(), status().as_str()), ("merhaba dünya!", "true true"));
    key(&rule, Key::Z, CTRL);
    assert_eq!(state.text(), "merhaba dünya");
    type_text(&rule, "x");
    assert_eq!(status(), "true false");
    key(&rule, Key::MoveHome, NONE); // typed somewhere else: a step of its own
    type_text(&rule, "Y");
    key(&rule, Key::Z, CTRL);
    assert_eq!((state.text().as_str(), status().as_str()), ("merhaba dünyax", "true true"));
    state.edit(|b| {
        b.append(".");
    });
    rule.wait_for_idle();
    assert_eq!(status(), "true false");
    state.undo_state().clear_history();
    rule.wait_for_idle();
    assert_eq!(status(), "false false");
    state.undo_state().undo(); // nothing to undo: nothing happens
    assert_eq!(state.text(), "merhaba dünyax.");
    // Steps are whole characters: "ö" typed before "ü" (they share a first byte), then "x" after it, is one step.
    state.edit(|b| {
        let len = b.length();
        b.replace(0, len, "aü");
        b.place_cursor_before_char_at(1);
    });
    rule.wait_for_idle();
    type_text(&rule, "ö");
    type_text(&rule, "x");
    key(&rule, Key::Z, CTRL);
    assert_eq!(state.text(), "aü");
}

/// C++ 11229 and 11286 (their state halves; the value-based field's are WP2c's): the selection comes back with
/// the text undone, typed where the undone edit was; an undo isn't an edit as the user — the field's input
/// transformation doesn't see it (Compose's `editWithNoSideEffects`), so a field refusing an empty text still
/// undoes to one.
fn undo_brings_the_selection_back_and_skips_the_input_transformation() {
    let state = TextFieldState::new("");
    let never_empty = TextFieldState::new("");
    let refuse_empty =
        InputTransformation.by_value(|current, proposed| if proposed.is_empty() { current } else { proposed });
    let (rule, _ime) = rule_with(vec![
        Field::new(&state).config(),
        Field::with(&never_empty, Some(refuse_empty), None, None).config(),
    ]);
    focus(&rule, "f0");
    rule.host().text_input("bir iki");
    rule.wait_for_idle();
    key(&rule, Key::MoveHome, NONE);
    key(&rule, Key::Delete, NONE);
    assert_eq!(state.text(), "ir iki");
    key(&rule, Key::Z, CTRL);
    rule.host().text_input("_");
    rule.wait_for_idle();
    assert_eq!(state.text(), "_bir iki");
    focus(&rule, "f1");
    rule.host().text_input("a");
    rule.wait_for_idle();
    key(&rule, Key::Backspace, NONE); // typed back to nothing: refused
    assert_eq!(never_empty.text(), "a");
    key(&rule, Key::Z, CTRL); // an undo isn't filtered
    assert_eq!(never_empty.text(), "");
    type_text(&rule, "bc");
    assert_eq!(never_empty.text(), "bc");
}

// ---------------------------------------------------------------------------------------------------------------
// Typing through the input session and keys

/// C++ 735 (its state half): UTF-8 typed and deleted whole; two inputs before a recomposition both kept; an emoji
/// sequence deleted whole by Backspace, a flag stepped over by an arrow.
fn typing_goes_through_the_input_session_in_utf8() {
    let state = TextFieldState::new("");
    let (rule, _ime) = rule_with(vec![Field::new(&state).config()]);
    rule.on_node_with_tag("f0").perform_text_input("çağ");
    assert_eq!(state.text(), "çağ");
    key(&rule, Key::Backspace, NONE);
    assert_eq!(state.text(), "ça");
    rule.host().text_input("x");
    rule.host().text_input("y");
    rule.wait_for_idle();
    assert_eq!(state.text(), "çaxy");
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    state.set_text_and_place_cursor_at_end(&format!("ab{family}"));
    rule.wait_for_idle();
    key(&rule, Key::Backspace, NONE);
    assert_eq!(state.text(), "ab");
    let turkey = "\u{1F1F9}\u{1F1F7}";
    state.set_text_and_place_cursor_at_end(&format!("x{turkey}y"));
    rule.wait_for_idle();
    key(&rule, Key::DirectionLeft, NONE);
    key(&rule, Key::DirectionLeft, NONE);
    assert_eq!(state.selection(), TextRange::at(1));
}

/// C++ 794 (its state half): an input method composing, then committing its correction; Backspace at the caret.
fn input_method_compositions_and_corrections() {
    let state = TextFieldState::new("");
    let (rule, ime) = rule_with(vec![Field::new(&state).config()]);
    focus(&rule, "f0");
    rule.host().set_composing_text("Guzel", 1);
    rule.wait_for_idle();
    assert_eq!((state.text().as_str(), state.composition()), ("Guzel", Some(TextRange::new(0, 5))));
    rule.host().text_input("Güzel ");
    rule.wait_for_idle();
    assert_eq!((state.text().as_str(), state.composition()), ("Güzel ", None));
    // The caret inside a composition, a part of it selected (an input method's highlight).
    rule.host().set_composing_text_with_selection("abc", 1, 2);
    rule.wait_for_idle();
    assert_eq!((state.composition(), state.selection()), (Some(TextRange::new(7, 10)), TextRange::new(8, 9)));
    // The selection leaving the composition finishes it (Compose desktop's input method session).
    state.edit(|b| b.place_cursor_before_char_at(0));
    rule.wait_for_idle();
    assert_eq!(state.composition(), None);
    rule.host().send_edit_commands(&[
        kompoz_ui::text_input::EditCommand::DeleteAll,
        kompoz_ui::text_input::EditCommand::CommitText { text: "abcd".into(), new_cursor_position: 1 },
        kompoz_ui::text_input::EditCommand::SetSelection { start: 2, end: 2 },
    ]);
    rule.wait_for_idle();
    key(&rule, Key::Backspace, NONE); // at the caret, not at the end
    assert_eq!(state.text(), "acd");
    // Surrounding text deleted by the input method (bytes, whole characters), and in code points.
    state.set_text_and_place_cursor_at_end("şeker");
    rule.wait_for_idle();
    rule.host()
        .send_edit_commands(&[kompoz_ui::text_input::EditCommand::DeleteSurroundingText { before: 2, after: 0 }]);
    rule.wait_for_idle();
    assert_eq!(state.text(), "şek");
    rule.host().send_edit_commands(&[
        kompoz_ui::text_input::EditCommand::SetSelection { start: 2, end: 2 },
        kompoz_ui::text_input::EditCommand::DeleteSurroundingTextInCodePoints { before: 1, after: 1 },
    ]);
    rule.wait_for_idle();
    assert_eq!(state.text(), "k");
    let _ = ime;
}

/// C++ 813 (its state half): a caret moved by the input method changes no text (no undo step); text typed and
/// taken back before a frame is two changes, both heard.
fn only_text_changes_are_text_changes() {
    let state = TextFieldState::new("kalem");
    let changes = Rc::new(RefCell::new(Vec::new()));
    let c = changes.clone();
    state.add_notify_ime_listener(Rc::new(move |old: &TextFieldCharSequence, new: &TextFieldCharSequence, _| {
        if old.text() != new.text() {
            c.borrow_mut().push(new.text().to_string());
        }
    }));
    let (rule, _ime) = rule_with(vec![Field::new(&state).config()]);
    focus(&rule, "f0");
    rule.host().send_edit_commands(&[kompoz_ui::text_input::EditCommand::SetSelection { start: 2, end: 2 }]);
    rule.wait_for_idle();
    assert!(changes.borrow().is_empty() && !state.undo_state().can_undo() && state.selection() == TextRange::at(2));
    rule.host().send_edit_commands(&[kompoz_ui::text_input::EditCommand::SetSelection { start: 5, end: 5 }]);
    rule.host().text_input("i");
    rule.host().send_edit_commands(&[kompoz_ui::text_input::EditCommand::Backspace]);
    rule.wait_for_idle();
    assert_eq!(
        (changes.borrow().clone(), state.text()),
        (vec!["kalemi".to_string(), "kalem".to_string()], "kalem".to_string())
    );
}

/// C++ 1452 (its model half): the input method's own edits aren't echoed back to it, the app's are; Enter in a
/// single-line field submits; the software keyboard controller shows the input method again (what tapping a
/// focused field does), and so does the D-pad's center.
fn app_side_changes_resync_the_input_method() {
    let state = TextFieldState::new("");
    let field = Field::new(&state).single_line();
    let submits = field.submits.clone();
    let s = state.clone();
    let config = Rc::new(FieldConfig {
        on_submit: Rc::new(move || {
            submits.set(submits.get() + 1);
            s.clear_text();
            true
        }),
        ..Rc::try_unwrap(field.config()).ok().unwrap()
    });
    let (rule, ime) = rule_with(vec![config]);
    // (What tells the platform the input method is on: a session opening, the field's place told, shown again.)
    let shows = || ime.shown.borrow().iter().filter(|(active, _)| *active).count();
    focus(&rule, "f0");
    let opened = shows();
    assert!(opened >= 1 && ime.shown.borrow().iter().all(|(active, _)| *active));
    rule.host().set_composing_text("kalem", 1);
    rule.wait_for_idle();
    assert_eq!(ime.told(), 0); // the input method's own edit is not echoed back
    key(&rule, Key::Enter, NONE); // submitting clears the field
    assert_eq!(
        (state.text().as_str(), field.submits.get(), ime.last().text.as_str()),
        ("", 1, ""),
        "{:?}",
        ime.changed.borrow()
    );
    assert_eq!(ime.told(), 1, "{:?}", ime.changed.borrow());
    let keyboard = rule.host().text_input_service();
    let before = shows();
    kompoz_ui::platform::SoftwareKeyboardController::new(keyboard).show();
    assert_eq!(shows(), before + 1);
    key(&rule, Key::DirectionCenter, NONE);
    assert_eq!(shows(), before + 2);
}

/// C++ 11138: editing keys, the selection and the clipboard.
fn editing_keys_selection_and_clipboard() {
    let state = TextFieldState::new("abcdef");
    let (rule, ime) = rule_with(vec![Field::new(&state).config()]);
    focus(&rule, "f0");
    key(&rule, Key::MoveHome, NONE);
    key(&rule, Key::DirectionRight, SHIFT);
    key(&rule, Key::DirectionRight, SHIFT);
    key(&rule, Key::X, CTRL);
    assert_eq!((state.text().as_str(), ime.clipboard.borrow().as_str()), ("cdef", "ab"));
    key(&rule, Key::MoveEnd, NONE);
    key(&rule, Key::V, CTRL);
    assert_eq!(state.text(), "cdefab");
    key(&rule, Key::DirectionLeft, NONE);
    key(&rule, Key::DirectionLeft, NONE);
    key(&rule, Key::Delete, NONE);
    assert_eq!(state.text(), "cdefb");
    key(&rule, Key::DirectionLeft, SHIFT);
    key(&rule, Key::C, CTRL);
    assert_eq!(ime.clipboard.borrow().as_str(), "f");
    key(&rule, Key::A, CTRL);
    key(&rule, Key::Backspace, NONE);
    assert_eq!(state.text(), "");
    // A key down taken: its key up too (not to reach the nodes around); a move that moves nothing isn't taken.
    assert!(!rule.host().key_down(Key::DirectionLeft, NONE) && !rule.host().key_up(Key::DirectionLeft, NONE));
    rule.host().text_input("x");
    rule.wait_for_idle();
    assert!(rule.host().key_down(Key::DirectionLeft, NONE) && rule.host().key_up(Key::DirectionLeft, NONE));
    assert!(!rule.host().key_up(Key::DirectionRight, NONE));
}

/// C++ 11170: Compose's key mapping (Ctrl, not macOS) by words and paragraphs; a read-only field takes no edit.
/// Words are the layout's (Compose's `getWordBoundary`: Android's WordBoundary, punctuation a word of its own):
/// where the C++ (as Compose desktop's ICU boundaries) stepped over ", " with the word before it, Ctrl+arrows stop
/// at the comma and Ctrl+Backspace takes it alone.
fn editing_keys_go_by_words_and_paragraphs() {
    let line = TextFieldState::new("bir iki, üç");
    let lines = TextFieldState::new("a\nbc\nd");
    let locked = TextFieldState::new("sabit metin");
    let (rule, ime) = rule_with(vec![
        Field::new(&line).single_line().config(),
        Field::new(&lines).config(),
        Field::new(&locked).read_only().config(),
    ]);
    focus(&rule, "f0");
    key(&rule, Key::MoveEnd, NONE);
    key(&rule, Key::DirectionLeft, CTRL); // to "üç"
    assert_eq!(line.selection(), TextRange::at(9));
    key(&rule, Key::DirectionLeft, CTRL); // to the comma
    assert_eq!(line.selection(), TextRange::at(7));
    key(&rule, Key::DirectionLeft, CTRL); // to "iki"
    key(&rule, Key::DirectionRight, ctrl_shift());
    key(&rule, Key::C, CTRL);
    assert_eq!(ime.clipboard.borrow().as_str(), "iki");
    key(&rule, Key::DirectionRight, CTRL); // past the comma
    assert_eq!(line.selection(), TextRange::at(8));
    key(&rule, Key::DirectionRight, CTRL); // past the space to the end of "üç"
    key(&rule, Key::Backspace, CTRL);
    assert_eq!(line.text(), "bir iki, ");
    key(&rule, Key::Backspace, CTRL); // ", " alone
    assert_eq!(line.text(), "bir iki");
    key(&rule, Key::Backspace, CTRL);
    assert_eq!(line.text(), "bir ");
    key(&rule, Key::MoveHome, NONE);
    key(&rule, Key::Delete, CTRL);
    assert_eq!(line.text(), " ");
    // (End goes to the line's visible end, before trailing spaces, as Compose's `getLineEnd(visibleEnd = true)`:
    // Ctrl+End to the text's end.)
    key(&rule, Key::MoveEnd, NONE);
    assert_eq!(line.selection(), TextRange::at(0));
    key(&rule, Key::MoveEnd, CTRL);
    key(&rule, Key::H, CTRL); // Backspace
    assert_eq!(line.text(), "");

    focus(&rule, "f1");
    key(&rule, Key::MoveEnd, CTRL);
    key(&rule, Key::DirectionUp, CTRL); // the start of "d"
    key(&rule, Key::DirectionUp, CTRL); // already there: the start of "bc"
    key(&rule, Key::DirectionDown, ctrl_shift());
    key(&rule, Key::C, CTRL);
    assert_eq!(ime.clipboard.borrow().as_str(), "bc");
    key(&rule, Key::DirectionUp, CTRL); // from the selection's start: "a"
    key(&rule, Key::Delete, KeyModifiers::Alt); // to the line's end
    assert_eq!(lines.text(), "\nbc\nd");
    // Lines: up and down by the layout's lines, at the x they started from.
    lines.set_text_and_place_cursor_at_end("abc\nd\nefg");
    rule.wait_for_idle();
    key(&rule, Key::DirectionUp, NONE);
    key(&rule, Key::DirectionUp, NONE);
    assert_eq!(lines.selection(), TextRange::at(3)); // past "d", back at the column it left
    key(&rule, Key::DirectionUp, NONE); // the first line: to its start
    assert_eq!(lines.selection(), TextRange::at(0));

    focus(&rule, "f2");
    key(&rule, Key::MoveEnd, NONE);
    key(&rule, Key::Backspace, CTRL);
    key(&rule, Key::Delete, KeyModifiers::Alt);
    assert_eq!(locked.text(), "sabit metin");
}

// ---------------------------------------------------------------------------------------------------------------
// Key mappings, dead keys, keyboard options and actions

fn down(key: Key, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(key, KeyEventType::KeyDown, modifiers)
}

/// Compose desktop's mappings: Linux's (Ctrl+Home to the text's start), macOS's (Cmd shortcuts, Option by words,
/// Emacs' Ctrl keys as Compose has them), Windows's (Alt+Backspace undoes).
fn key_mappings_follow_the_platform() {
    use KeyCommand::*;
    let linux = default_skiko_key_mapping();
    assert_eq!(linux.map(&down(Key::MoveHome, CTRL)), Some(HOME));
    assert_eq!(linux.map(&down(Key::MoveEnd, ctrl_shift())), Some(SELECT_END));
    assert_eq!(linux.map(&down(Key::MoveHome, NONE)), Some(LINE_START));
    assert_eq!(linux.map(&down(Key::Backspace, CTRL)), Some(DELETE_PREV_WORD));
    assert_eq!(linux.map(&down(Key::Z, ctrl_shift())), Some(REDO));
    assert_eq!(linux.map(&down(Key::Insert, SHIFT)), Some(PASTE));
    assert_eq!(linux.map(&down(Key::Backslash, CTRL)), Some(DESELECT));
    assert_eq!(linux.map(&down(Key::Z, NONE)), None);
    let mac = mac_os_key_mapping();
    assert_eq!(mac.map(&down(Key::Z, KeyModifiers::Meta)), Some(UNDO));
    assert_eq!(mac.map(&down(Key::Z, CTRL)), None);
    assert_eq!(mac.map(&down(Key::DirectionLeft, KeyModifiers::Alt)), Some(LEFT_WORD));
    assert_eq!(mac.map(&down(Key::Backspace, KeyModifiers::Meta)), Some(DELETE_FROM_LINE_START));
    assert_eq!(mac.map(&down(Key::F, CTRL)), Some(LEFT_CHAR));
    assert_eq!(mac.map(&down(Key::Spacebar, KeyModifiers::Ctrl | KeyModifiers::Meta)), Some(CHARACTER_PALETTE));
    let windows = windows_key_mapping();
    assert_eq!(windows.map(&down(Key::Backspace, KeyModifiers::Alt)), Some(UNDO));
    assert_eq!(windows.map(&down(Key::MoveHome, CTRL)), Some(HOME));
    assert!(PASTE.edits_text() && !COPY.edits_text() && UNDO.edits_text() && !SELECT_ALL.edits_text());
    // Another platform's mapping for this thread's fields.
    set_key_mapping_override(Some(Rc::new(mac_os_key_mapping())));
    assert_eq!(platform_default_key_mapping().map(&down(Key::Z, KeyModifiers::Meta)), Some(UNDO));
    // None: the running platform's own.
    set_key_mapping_override(None);
    let own = create_platform_default_key_mapping(DesktopPlatform::current());
    for event in
        [down(Key::MoveHome, CTRL), down(Key::Z, KeyModifiers::Meta), down(Key::DirectionLeft, KeyModifiers::Alt)]
    {
        assert_eq!(platform_default_key_mapping().map(&event), own.map(&event), "{event:?}");
    }
    // (back to the mapping these tests press keys in)
    use_the_default_key_mapping();
}

/// Android's dead keys: an accent then a letter types the accented letter; the accent twice or then a space types
/// it alone; a pair that doesn't combine types the letter. Typed events are KEY_TYPED's (Unknown); KeyDown isn't.
fn dead_keys_combine_with_the_next_key() {
    let typed = |code_point: i32| KeyEvent::with_code_point(Key::Unknown, KeyEventType::Unknown, code_point, NONE);
    let dead = |accent: char| typed(accent as i32 | COMBINING_ACCENT);
    let mut c = DeadKeyCombiner::new();
    assert_eq!(c.consume(&dead('\u{00B4}')), None);
    assert_eq!(c.consume(&typed('e' as i32)), Some('é' as i32));
    c.consume(&dead('\u{00A8}'));
    assert_eq!(c.consume(&typed('u' as i32)), Some('ü' as i32));
    c.consume(&dead('`'));
    assert_eq!(c.consume(&typed(' ' as i32)), Some('`' as i32));
    c.consume(&dead('-'));
    assert_eq!(c.consume(&typed('d' as i32)), Some('đ' as i32));
    c.consume(&dead('~'));
    assert_eq!(c.consume(&typed('x' as i32)), Some('x' as i32));
    assert_eq!(c.consume(&typed('q' as i32)), Some('q' as i32));
    assert_eq!(get_dead_char('\u{02C7}' as u32, 'c' as u32), 'č' as u32);
    assert!(is_typed_event(&typed('a' as i32)) && !is_typed_event(&down(Key::A, NONE)));
    assert!(!is_typed_event(&typed(0x08)));
    // Through a field's key handler: a dead key, then a letter.
    let state = TextFieldState::new("");
    let (rule, _ime) = rule_with(vec![Field::new(&state).config()]);
    focus(&rule, "f0");
    rule.host().send_key_event(dead('\u{02C6}')); // (not taken: it types nothing yet, as Compose's)
    assert!(rule.host().send_key_event(typed('o' as i32)));
    rule.wait_for_idle();
    assert_eq!(state.text(), "ô");
}

/// Compose's KeyboardOptions: unspecified values fall back to the defaults in ImeOptions, and to another's in a
/// merge; KeyboardActions run the app's action for a key, else its default (Next moves the focus, Done hides the
/// keyboard).
fn keyboard_options_and_actions() {
    use kompoz_ui::platform::{KeyboardCapitalization, KeyboardType};
    let ime = KeyboardOptions::Default.to_ime_options(true);
    assert_eq!(
        (ime.keyboard_type, ime.ime_action, ime.capitalization, ime.auto_correct, ime.single_line),
        (KeyboardType::Text, ImeAction::Default, KeyboardCapitalization::None, true, true)
    );
    let email = KeyboardOptions { keyboard_type: KeyboardType::Email, ..KeyboardOptions::Default };
    let done =
        KeyboardOptions { ime_action: ImeAction::Done, keyboard_type: KeyboardType::Text, ..KeyboardOptions::Default };
    let merged = email.merge(Some(&done));
    assert_eq!((merged.keyboard_type, merged.ime_action), (KeyboardType::Text, ImeAction::Done));
    let filled = email.fill_unspecified_values_with(Some(&done));
    assert_eq!((filled.keyboard_type, filled.ime_action), (KeyboardType::Email, ImeAction::Done));
    assert!(KeyboardOptions::Default.is_completely_unspecified());
    assert!(!KeyboardOptions::SecureTextField.to_ime_options(true).auto_correct);
    let copied = named_args!(done.copy(ime_action = ImeAction::Go));
    assert_eq!((copied.ime_action, copied.show_keyboard_on_focus), (ImeAction::Go, None));
    // Actions: the app's, else the default.
    let ran = Rc::new(RefCell::new(Vec::new()));
    let r = ran.clone();
    let platform = Rc::new(Ime::default());
    let held: Rc<RefCell<Option<kompoz_ui::focus::FocusManager>>> = Rc::new(RefCell::new(None));
    let h = held.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *h.borrow_mut() = Some(kompoz_ui::platform::LocalFocusManager.current());
        Column(|| {
            Box(modifier = Modifier.test_tag("a").focusable().size(10.dp()));
            Box(modifier = Modifier.test_tag("b").focusable().size(10.dp()));
        });
    }));
    rule.host().set_callbacks(platform.clone());
    focus(&rule, "a");
    let on_search: KeyboardAction = Rc::new(move |scope: &dyn KeyboardActionScope| {
        r.borrow_mut().push("search");
        scope.default_keyboard_action(ImeAction::Done);
    });
    let actions = KeyboardActions { on_search: Some(on_search), ..KeyboardActions::Default };
    let focus_manager = held.borrow().clone().unwrap();
    let runner = KeyboardActionRunner::new(
        Some(kompoz_ui::platform::SoftwareKeyboardController::new(rule.host().text_input_service())),
        actions,
        focus_manager,
    );
    assert!(runner.run_action(ImeAction::Search));
    assert_eq!((ran.borrow().clone(), platform.hidden.get()), (vec!["search"], 1));
    assert!(runner.run_action(ImeAction::Next));
    rule.wait_for_idle();
    rule.on_node_with_tag("b").assert(is_focused());
    assert!(!runner.run_action(ImeAction::Go) && !runner.run_action(ImeAction::Default));
    let any = KeyboardActions::on_any(|_| {});
    assert!(any.on_send.is_some() && any == any.clone() && any != KeyboardActions::Default);
}

/// Line limits and obfuscation modes as Compose has them.
fn line_limits_and_obfuscation_modes() {
    assert_eq!(TextFieldLineLimits::Default, TextFieldLineLimits::multi_line(1, i32::MAX));
    assert!(TextFieldLineLimits::SingleLine.is_single_line() && !TextFieldLineLimits::Default.is_single_line());
    assert_eq!(TextFieldLineLimits::SingleLine.to_string(), "TextFieldLineLimits.SingleLine");
    assert_eq!(TextFieldLineLimits::multi_line(2, 3).to_string(), "MultiLine(minHeightInLines=2, maxHeightInLines=3)");
    assert!(std::panic::catch_unwind(|| TextFieldLineLimits::multi_line(0, 3)).is_err());
    assert!(std::panic::catch_unwind(|| TextFieldLineLimits::multi_line(4, 3)).is_err());
    assert_eq!(TextObfuscationMode::default(), TextObfuscationMode::System);
    assert_eq!(
        [TextObfuscationMode::Visible, TextObfuscationMode::RevealLastTyped, TextObfuscationMode::Hidden]
            .map(|m| m.value()),
        [0, 1, 2]
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Sessions and test actions

/// One input method session at a time: focusing another field cancels the first's (the platform hears it close,
/// then the other open), as Compose's owner session mutex does; an interceptor around a field changes its request.
fn input_sessions_one_at_a_time_and_intercepted() {
    use kompoz_ui::platform::KeyboardType;
    use kompoz_ui::platform_text_input::{
        InterceptPlatformTextInput, PlatformTextInputInterceptor, PlatformTextInputMethodRequest,
        PlatformTextInputSession, SessionFuture,
    };
    let (a, b) = (TextFieldState::new("a"), TextFieldState::new("b"));
    let (fa, fb) = (Field::new(&a).config(), Field::new(&b).config());
    let intercepted = Rc::new(Cell::new(0));
    let i = intercepted.clone();
    let interceptor: Rc<dyn PlatformTextInputInterceptor> =
        Rc::new(move |request: PlatformTextInputMethodRequest, next: PlatformTextInputSession| -> SessionFuture {
            i.set(i.get() + 1);
            let mut request = request;
            request.ime_options.keyboard_type = KeyboardType::Email;
            next.start_input_method(request)
        });
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (fa, fb, interceptor) = (fa.clone(), fb.clone(), interceptor.clone());
        Column(move || {
            ModelField(fa.clone(), "f0");
            let fb = fb.clone();
            InterceptPlatformTextInput(interceptor.clone(), move || ModelField(fb.clone(), "f1"));
        });
    }));
    let ime = Rc::new(Ime::default());
    rule.host().set_callbacks(ime.clone());
    focus(&rule, "f0");
    focus(&rule, "f1");
    // (The changes of what the platform is told: a field's place told again keeps it on.)
    let mut shown: Vec<(bool, KeyboardType)> =
        ime.shown.borrow().iter().map(|(on, o)| (*on, o.keyboard_type)).collect();
    shown.dedup();
    assert_eq!(
        shown,
        [(true, KeyboardType::Unspecified), (false, KeyboardType::Unspecified), (true, KeyboardType::Email)]
    );
    assert_eq!(intercepted.get(), 1);
    rule.host().text_input("!");
    rule.wait_for_idle();
    assert_eq!((a.text().as_str(), b.text().as_str()), ("a", "b!"));
    // The focus leaving: the session ends, the input method is told.
    rule.host().focus.clear_focus(true);
    rule.wait_for_idle();
    assert_eq!(ime.shown.borrow().last().map(|(on, _)| *on), Some(false));
    assert!(!rule.host().text_input("?"));
}

/// Compose ui-test's text actions on a field: typing, replacing, clearing, selecting, its keyboard action.
fn test_rule_text_actions() {
    let state = TextFieldState::new("abc");
    let mut field = Field::new(&state);
    field.ime_options.ime_action = ImeAction::Search;
    let actions = field.actions.clone();
    let (rule, _ime) = rule_with(vec![field.config()]);
    let node = rule.on_node_with_tag("f0");
    node.assert(is_editable()).assert(has_ime_action(ImeAction::Search)).assert(has_set_text_action());
    node.perform_text_input("d");
    assert_eq!(state.text(), "abcd");
    node.assert(is_focused());
    node.perform_text_input_selection(TextRange::new(1, 3), true);
    assert_eq!(state.selection(), TextRange::new(1, 3));
    node.perform_text_input("X");
    assert_eq!(state.text(), "aXd");
    node.perform_text_replacement("yeni");
    assert_eq!(state.text(), "yeni");
    node.perform_text_clearance();
    assert_eq!(state.text(), "");
    node.perform_ime_action();
    assert_eq!(actions.borrow().clone(), [ImeAction::Search]);
    // The platform's action key reaches the same session.
    assert!(rule.host().ime_action(ImeAction::Search));
    assert_eq!(actions.borrow().len(), 2);
}

tests!(
    default_key_mapping;
    text_field_buffer_edits_and_moves_the_selection,
    change_lists_merge_touching_changes,
    offset_mapping_maps_both_ways,
    tracked_styles_follow_the_edits,
    text_field_state_holds_the_text,
    text_field_state_keeps_its_selection,
    text_field_state_saves_its_text_selection_and_history,
    input_transformations_change_or_refuse_what_is_typed,
    output_transformation_formats_what_is_shown,
    codepoint_transformations_mask_and_flatten,
    carets_move_across_what_was_inserted,
    undo_operations_merge_as_composes,
    text_field_state_undoes_typing_a_run_at_a_time,
    undo_brings_the_selection_back_and_skips_the_input_transformation,
    typing_goes_through_the_input_session_in_utf8,
    input_method_compositions_and_corrections,
    only_text_changes_are_text_changes,
    app_side_changes_resync_the_input_method,
    editing_keys_selection_and_clipboard,
    editing_keys_go_by_words_and_paragraphs,
    key_mappings_follow_the_platform,
    dead_keys_combine_with_the_next_key,
    keyboard_options_and_actions,
    line_limits_and_obfuscation_modes,
    input_sessions_one_at_a_time_and_intercepted,
    test_rule_text_actions,
);

//! Selecting in the state-based text fields (WP2b) — a finger's long press, handles and the toolbar, the mouse's
//! clicks, drags and shift, the focus coming back, password fields, multi-line and right-to-left fields — and the
//! text field halves of the C++ Kompoz's selection tests whose SelectionContainer halves are in selection.rs.
//! Their Material TextField's decoration (16 dp padding, 56 dp height, label) is stage 12's, by the same names in
//! kompoz-material3/tests/text_field_halves.rs: the fields here are undecorated BasicTextFields; Compose's
//! behaviour where the C++ differs (said where it matters).
//!
//! The texts are 16 sp in the fake backend: 8 px a character, 20 px lines. Offsets are UTF-8 bytes. Numbers in
//! px. The cursor doesn't blink here (LocalCursorBlinkEnabled off): idling doesn't run its timers on.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::magnifier::MagnifierPositionInRoot;
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::selection::{LocalTextSelectionColors, SelectionHandleInfo, SelectionHandleInfoKey};
use kompoz_foundation::text::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_text::{TextDirection, TextRange, TextStyle};
use kompoz_ui::input::*;
use kompoz_ui::platform::{
    HapticFeedbackType, ImeOptions, LocalCursorBlinkEnabled, LocalTextToolbar, PlatformCallbacks, TextEditState,
    TextToolbar, TextToolbarAction, TextToolbarStatus,
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

/// The 16 sp style the C++ fields' text had (bodyLarge's size): 8 px a character, 20 px lines.
fn style16() -> TextStyle {
    TextStyle { font_size: 16.sp(), ..TextStyle::Default }
}

/// A platform keeping its clipboard, its haptics, and what it is told of text input (shown or not, single line,
/// where).
#[derive(Default)]
struct TestPlatform {
    clipboard: RefCell<Option<String>>,
    haptics: RefCell<Vec<HapticFeedbackType>>,
    text_inputs: RefCell<Vec<(bool, bool, Rect)>>,
}

impl PlatformCallbacks for TestPlatform {
    fn get_clip_entry(&self) -> Option<kompoz_ui::clipboard::ClipEntry> {
        self.clipboard.borrow().clone().map(kompoz_ui::clipboard::ClipEntry::with_plain_text)
    }
    fn set_clip_entry(&self, entry: Option<kompoz_ui::clipboard::ClipEntry>) {
        *self.clipboard.borrow_mut() = entry.and_then(|e| e.read_plain_text());
    }
    fn haptic_feedback(&self, kind: HapticFeedbackType) {
        self.haptics.borrow_mut().push(kind);
    }
    fn text_input(&self, active: bool, area: Rect, _state: &TextEditState, options: &ImeOptions) {
        self.text_inputs.borrow_mut().push((active, options.single_line, area));
    }
}

impl TestPlatform {
    fn clipboard(&self) -> String {
        self.clipboard.borrow().clone().unwrap_or_default()
    }
    fn set_clip(&self, text: &str) {
        *self.clipboard.borrow_mut() = Some(text.to_string());
    }
    /// How many haptics of `kind` were felt.
    fn felt(&self, kind: HapticFeedbackType) -> usize {
        self.haptics.borrow().iter().filter(|&&k| k == kind).count()
    }
    /// How often the input method was asked to show again where it was (not told the cursor moved).
    fn shown_again(&self) -> usize {
        let told = self.text_inputs.borrow();
        (1..told.len()).filter(|&i| told[i].0 && told[i - 1].0 && told[i].2 == told[i - 1].2).count()
    }
    /// Whether the last input method shown was a single line's.
    fn last_single_line(&self) -> Option<bool> {
        self.text_inputs.borrow().iter().rev().find(|(active, _, _)| *active).map(|(_, single, _)| *single)
    }
}

fn platform(rule: &KompozTestRule) -> Rc<TestPlatform> {
    let p = Rc::new(TestPlatform::default());
    rule.host().set_callbacks(p.clone());
    p
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).fetch_semantics_node().bounds_in_root()
}

/// Where a character of a tagged field is: `index` characters in, 2 px into it, on line `line`'s middle.
fn char_at(rule: &KompozTestRule, tag: &str, index: usize, line: usize) -> Offset {
    let r = bounds(rule, tag);
    at(r.left + 8.0 * index as f32 + 2.0, r.top + 10.0 + 20.0 * line as f32)
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

/// Mouse clicks at `p`, `n` in a row, after a pause that ends a previous series.
fn clicks(rule: &KompozTestRule, p: Offset, n: usize) {
    rule.advance_time_by(500);
    for _ in 0..n {
        rule.host().pointer_down(p, -1, 0, PointerType::Mouse);
        rule.host().pointer_up(p, -1, 0);
    }
    rule.wait_for_idle();
}

/// A finger's drag from `from`, `steps` moves of `step`, a frame each, held (`release`: lifted after).
fn finger_drag(rule: &KompozTestRule, from: Offset, step: Offset, steps: usize, release: bool) {
    rule.host().pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=steps {
        rule.advance_frame();
        rule.host().pointer_move(from + step * i as f32, -1, 0);
    }
    if release {
        rule.host().pointer_up(from + step * steps as f32, -1, 0);
    }
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

fn is_focused(rule: &KompozTestRule, tag: &str) -> bool {
    rule.on_node_with_tag(tag).fetch_semantics_node().config().get(SemanticsProperties::Focused) == Some(true)
}

const NONE: KeyModifiers = KeyModifiers::None;
const CTRL: KeyModifiers = KeyModifiers::Ctrl;
const SHIFT: KeyModifiers = KeyModifiers::Shift;

/// The handles shown (their SelectionHandleInfo semantics).
fn handles(rule: &KompozTestRule) -> Vec<SelectionHandleInfo> {
    rule.all_nodes(true).iter().filter_map(|n| n.config().get(SelectionHandleInfoKey)).collect()
}

fn handle(rule: &KompozTestRule, which: Handle) -> Option<SelectionHandleInfo> {
    handles(rule).into_iter().find(|h| h.handle == which)
}

/// Where the magnifier shows the content of, in the root (none while none shows).
fn magnifier_at(rule: &KompozTestRule) -> Option<Offset> {
    rule.all_nodes(true)
        .iter()
        .filter_map(|n| n.config().get(MagnifierPositionInRoot))
        .map(|p| (p.0)())
        .find(|o| o.is_specified())
}

/// The selection highlights drawn within `within` (paths in the selection's background color).
fn highlights(rule: &KompozTestRule, within: Rect) -> Vec<Rect> {
    let canvas = rule.draw();
    let background = LocalTextSelectionColors.default_value().background_color;
    (0..canvas.paths.len())
        .filter(|&i| canvas.path_colors[i] == background && within.contains(canvas.paths[i].center()))
        .map(|i| canvas.paths[i])
        .collect()
}

/// A text toolbar of the test's own: what it was shown with, how often shown and hidden.
#[derive(Default)]
struct RecordingToolbar {
    shows: Cell<usize>,
    hides: Cell<usize>,
    rect: Cell<Rect>,
    status: Cell<Option<TextToolbarStatus>>,
    copy: RefCell<TextToolbarAction>,
    paste: RefCell<TextToolbarAction>,
    cut: RefCell<TextToolbarAction>,
    select_all: RefCell<TextToolbarAction>,
}

impl TextToolbar for RecordingToolbar {
    fn show_menu(
        &self,
        rect: Rect,
        on_copy_requested: TextToolbarAction,
        on_paste_requested: TextToolbarAction,
        on_cut_requested: TextToolbarAction,
        on_select_all_requested: TextToolbarAction,
        _on_autofill_requested: TextToolbarAction,
    ) {
        self.shows.set(self.shows.get() + 1);
        self.rect.set(rect);
        *self.copy.borrow_mut() = on_copy_requested;
        *self.paste.borrow_mut() = on_paste_requested;
        *self.cut.borrow_mut() = on_cut_requested;
        *self.select_all.borrow_mut() = on_select_all_requested;
        self.status.set(Some(TextToolbarStatus::Shown));
    }
    fn hide(&self) {
        self.hides.set(self.hides.get() + 1);
        self.status.set(Some(TextToolbarStatus::Hidden));
    }
    fn status(&self) -> TextToolbarStatus {
        self.status.get().unwrap_or(TextToolbarStatus::Hidden)
    }
}

impl RecordingToolbar {
    fn shown(&self) -> bool {
        self.status() == TextToolbarStatus::Shown
    }
    /// Which items it offers while shown: copy, paste, cut, select all.
    fn offers(&self) -> (bool, bool, bool, bool) {
        let shown = self.shown();
        (
            shown && self.copy.borrow().is_some(),
            shown && self.paste.borrow().is_some(),
            shown && self.cut.borrow().is_some(),
            shown && self.select_all.borrow().is_some(),
        )
    }
    fn choose(&self, rule: &KompozTestRule, item: &RefCell<TextToolbarAction>) {
        let action = item.borrow().clone().expect("the toolbar offers it");
        action();
        rule.wait_for_idle();
    }
}

/// `content` with `toolbar` as its LocalTextToolbar, the cursor not blinking.
#[kompozable(inline)]
fn Steady(toolbar: Rc<RecordingToolbar>, content: impl FnOnce()) {
    let t: Rc<dyn TextToolbar> = toolbar;
    KompozitionLocalProvider(&[LocalTextToolbar.provides_unequal(t), LocalCursorBlinkEnabled.provides(false)], content);
}

/// A single-line field of `state` filling the width under a 200 px box, a 300 px box under it (tags "field" and
/// "below"; C++'s TextField with `singleLine`).
fn field_under_a_box(
    state: TextFieldState,
    toolbar: Rc<RecordingToolbar>,
    read_only: MutableState<bool>,
) -> KompozTestRule {
    KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (s, r) = (state.clone(), read_only.clone());
            Steady(toolbar.clone(), move || {
                Column(move || {
                    Box(modifier = Modifier.height(200.dp()));
                    BasicTextField(
                        s.clone(),
                        modifier = Modifier.fill_max_width().test_tag("field"),
                        read_only = r.get(),
                        text_style = style16(),
                        line_limits = TextFieldLineLimits::SingleLine,
                    );
                    Box(modifier = Modifier.fill_max_width().height(300.dp()).test_tag("below"));
                });
            });
        }),
        IntSize::new(400, 800),
    )
}

// ---------------------------------------------------------------------------------------------------------------
// The C++ Kompoz's tests

/// C++ 904: a long press selects a word, felt, with handles and the toolbar (cut, copy, select all) over it; copy
/// collapses it; the end handle dragged extends it, a tick a step; select all and cut from the toolbar; typing
/// ends it (Compose: a press elsewhere doesn't — the field keeps its focus and selection).
fn long_press_selects_a_word_with_handles_and_a_toolbar() {
    let state = TextFieldState::new("merhaba güzel dünya");
    let toolbar = Rc::new(RecordingToolbar::default());
    let rule = field_under_a_box(state.clone(), toolbar.clone(), mutable_state_of(false));
    let p = platform(&rule);
    let field = bounds(&rule, "field");
    long_press(&rule, char_at(&rule, "field", 10, 0)); // in "güzel" (bytes 8..14: ü takes two)
    assert_eq!(state.selection(), TextRange::new(8, 14));
    assert_eq!(*p.haptics.borrow(), vec![HapticFeedbackType::LongPress]); // felt as it selects
    assert!(handle(&rule, Handle::SelectionStart).is_some() && handle(&rule, Handle::SelectionEnd).is_some());
    // Nothing in the clipboard: no paste.
    assert_eq!(toolbar.offers(), (true, false, true, true));
    // Over the selection: "güzel", characters 8..13, on the field's line (C++ checked its own toolbar was drawn
    // above the field: the toolbar is LocalTextToolbar's here).
    let r = toolbar.rect.get();
    assert!(near(r.left, field.left + 64.0, 0.01) && near(r.width(), 40.0, 0.01), "{r:?}");
    assert!(near(r.top, field.top, 0.01) && near(r.bottom, field.top + 20.0, 0.01), "{r:?}");
    toolbar.choose(&rule, &toolbar.copy);
    assert_eq!(p.clipboard(), "güzel");
    // Compose's copy(cancelSelection = true): collapsed to its end, the handles and toolbar gone.
    assert_eq!(state.selection(), TextRange::at(14));
    assert!(handles(&rule).is_empty() && !toolbar.shown());
    // The end handle hangs under the end of "merhaba": dragged six characters on, " güzel" joins (by the word: the
    // drag snaps to words once it passes into one, Compose's CharacterWithWordAccelerate).
    long_press(&rule, char_at(&rule, "field", 3, 0));
    assert_eq!(state.selection(), TextRange::new(0, 7));
    let end = handle(&rule, Handle::SelectionEnd).expect("the end handle");
    assert!(near(end.position.x, 56.0, 0.01) && near(end.position.y, 20.0, 0.01), "{end:?}");
    let ticks = p.felt(HapticFeedbackType::TextHandleMove);
    let grab = at(field.left + 56.0 + 11.0, field.top + 20.0 + 11.0);
    finger_drag(&rule, grab, at(4.0, 0.0), 12, true); // half a character at a time
    assert_eq!(state.selection(), TextRange::new(0, 14));
    // A tick each time the end moved: once here, by the whole word (the C++ ticked a character at a time, six).
    assert_eq!(p.felt(HapticFeedbackType::TextHandleMove), ticks + 1);
    let end = handle(&rule, Handle::SelectionEnd).expect("the end handle");
    assert!(near(end.position.x, 104.0, 0.01), "{end:?}"); // now under the end of "güzel"
    // C++ checked the moved handle's drawn rect (22 px wide): Compose's handles are 25 px, their point's square
    // checked by the next tests.
    let ticks = p.felt(HapticFeedbackType::TextHandleMove);
    toolbar.choose(&rule, &toolbar.select_all);
    assert_eq!(state.selection(), TextRange::new(0, "merhaba güzel dünya".len()));
    assert_eq!(p.felt(HapticFeedbackType::TextHandleMove), ticks); // no handle moved
    assert!(toolbar.offers().2);
    toolbar.choose(&rule, &toolbar.cut);
    assert!(state.text().is_empty() && p.clipboard() == "merhaba güzel dünya");
    assert!(handles(&rule).is_empty() && !toolbar.shown());
    // Compose: a press elsewhere (on nothing focusable) leaves the field focused and its selection as it was, the
    // toolbar shown; the C++ ended it.
    state.set_text_and_place_cursor_at_end("bir iki");
    rule.wait_for_idle();
    long_press(&rule, char_at(&rule, "field", 5, 0));
    assert_eq!(state.selection(), TextRange::new(4, 7));
    tap(&rule, bounds(&rule, "below").center());
    assert!(is_focused(&rule, "field") && state.selection() == TextRange::new(4, 7) && toolbar.shown());
    // Typing ends it.
    long_press(&rule, char_at(&rule, "field", 1, 0));
    type_text(&rule, "x");
    assert_eq!((state.text().as_str(), state.selection()), ("x iki", TextRange::at(1)));
    assert!(!toolbar.shown() && handle(&rule, Handle::SelectionStart).is_none());
}

/// C++ 1127: a finger's tap puts the cursor with a handle under it, which a tap toggles the toolbar of (paste,
/// select all) and a drag moves; the mouse's long press selects nothing, its click shows no handle, its double
/// click takes the word. (Compose offers paste only with text in the clipboard: some is put there.)
fn caret_handle_for_fingers_double_click_for_the_mouse() {
    let state = TextFieldState::new("merhaba güzel dünya");
    let toolbar = Rc::new(RecordingToolbar::default());
    let rule = field_under_a_box(state.clone(), toolbar.clone(), mutable_state_of(false));
    let p = platform(&rule);
    p.set_clip("pano");
    let field = bounds(&rule, "field");
    tap(&rule, char_at(&rule, "field", 3, 0));
    assert_eq!(state.selection(), TextRange::at(3));
    let cursor = handle(&rule, Handle::Cursor).expect("the cursor handle");
    assert!(near(cursor.position.x, 25.0, 0.01) && near(cursor.position.y, 20.0, 0.01), "{cursor:?}");
    assert!(!toolbar.shown());
    // A tap on the handle: the toolbar, without copy.
    rule.advance_time_by(500); // not a double tap
    let grab = at(field.left + 25.0, field.top + 20.0 + 13.0);
    tap(&rule, grab);
    assert_eq!(toolbar.offers(), (false, true, false, true));
    assert!(handle(&rule, Handle::Cursor).is_some());
    // Dragging it moves the cursor: 40 px, less the 8 px touch slop the drag doesn't take, four characters on. (By
    // 10 px a move: a move the handle followed exactly, the finger still over the same point of it, is no move for
    // it — Compose's HitPathTracker dispatches no move that changed no position.)
    finger_drag(&rule, grab, at(10.0, 0.0), 4, true);
    assert_eq!(state.selection(), TextRange::at(7));
    // Compose keeps the toolbar's state through a handle's drag: shown again once it ends (the C++ hid it).
    assert!(toolbar.shown());
    // The mouse: a long press doesn't select, a click shows no handle, a double click takes the word.
    rule.advance_time_by(500);
    let mouse_at = char_at(&rule, "field", 10, 0);
    rule.host().pointer_down(mouse_at, -1, 0, PointerType::Mouse);
    rule.advance_time_by(600);
    rule.host().pointer_up(mouse_at, -1, 0);
    rule.wait_for_idle();
    assert!(state.selection().collapsed() && handles(&rule).is_empty() && !toolbar.shown());
    clicks(&rule, mouse_at, 1);
    assert_eq!(state.selection(), TextRange::at(11)); // before "e" ("ü" takes two bytes)
    assert!(handles(&rule).is_empty());
    clicks(&rule, mouse_at, 2);
    assert_eq!(state.selection(), TextRange::new(8, 14));
    assert!(handles(&rule).is_empty() && !toolbar.shown());
}

/// C++ 1172: a double click takes letters, digits, underscores and what is beyond ASCII (ICU's word boundaries:
/// the same word as the C++'s).
fn a_double_click_takes_letters_digits_underscores_and_what_is_beyond_ascii() {
    let state = TextFieldState::new("x snake_case2ü, y");
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Steady(Rc::new(RecordingToolbar::default()), move || {
                BasicTextField(s.clone(), modifier = Modifier.fill_max_width().test_tag("f"), text_style = style16());
            });
        }),
        IntSize::new(400, 800),
    );
    clicks(&rule, char_at(&rule, "f", 4, 0), 2); // in "snake"
    assert_eq!(state.selection(), TextRange::new(2, 15)); // "snake_case2ü" (ü takes two bytes), not the comma
}

/// C++ 1191: a mouse press places the cursor and focuses, a drag selects; two clicks take words, three
/// paragraphs, dragging by them; Shift extends; nothing changes the text. (Compose's mouse selection is never
/// reversed: dragged back from a word, it runs forwards.)
fn the_mouse_selects_text_by_dragging_clicking_and_shift() {
    let state = TextFieldState::new("alpha beta gamma\ndelta");
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Steady(Rc::new(RecordingToolbar::default()), move || {
                Column(move || {
                    Box(modifier = Modifier.height(100.dp()));
                    BasicTextField(
                        s.clone(),
                        modifier = Modifier.fill_max_width().test_tag("f"),
                        text_style = style16(),
                    );
                });
            });
        }),
        IntSize::new(400, 800),
    );
    let p = platform(&rule);
    let h = rule.host();
    let x = |column: usize| char_at(&rule, "f", column, 0);
    // Where a click puts the cursor.
    let caret_at = |column: usize| {
        clicks(&rule, x(column), 1);
        state.selection().end
    };
    let c1 = caret_at(1);
    assert!(is_focused(&rule, "f") && state.selection() == TextRange::at(c1)); // focused by the press
    let before = p.shown_again();
    let (c3, c13) = (caret_at(3), caret_at(13));
    assert_eq!(p.shown_again(), before + 2); // a click on the focused field brings back a keyboard dismissed
    assert_eq!((c1, c3, c13), (1, 3, 13));
    // A drag selects from the press.
    rule.advance_time_by(500);
    h.pointer_down(x(1), -1, 0, PointerType::Mouse);
    for i in 2..=13 {
        h.pointer_move(x(i), -1, 0);
    }
    h.pointer_up(x(13), -1, 0);
    rule.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(c1, c13));
    // A double click, then dragging: whole words, from "beta" on to the end of "gamma"...
    rule.advance_time_by(500);
    h.pointer_down(x(7), -1, 0, PointerType::Mouse);
    h.pointer_up(x(7), -1, 0);
    h.pointer_down(x(7), -1, 0, PointerType::Mouse);
    rule.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(6, 10)); // "beta"
    h.pointer_move(x(13), -1, 0);
    rule.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(6, 16));
    // ...or back, from the end of "beta" to the start of "alpha" (Compose: forwards, 0..10; the C++'s 10..0).
    h.pointer_move(x(1), -1, 0);
    h.pointer_up(x(1), -1, 0);
    rule.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(0, 10));
    // Three clicks: the paragraph (the first line, without its line break).
    clicks(&rule, x(7), 3);
    assert_eq!(state.selection(), TextRange::new(0, 16));
    // Shift and a click: the selection goes from where it started to there.
    caret_at(3);
    h.set_keyboard_modifiers(SHIFT);
    clicks(&rule, x(13), 1);
    h.set_keyboard_modifiers(NONE);
    rule.wait_for_idle();
    assert_eq!(state.selection(), TextRange::new(c3, c13));
    assert_eq!(state.text(), "alpha beta gamma\ndelta"); // selecting changes no text
}

/// C++ 12272: a field all selected, left for a password field and come back to shows no stale selection (its
/// selection collapsed to the end as it lost the focus); a password field's Ctrl+C copies nothing. (Single-line
/// fields: Tab in a multi-line one types a tab, as Compose's; the C++'s OutlinedTextFields' outline is stage
/// 12's: kompoz-material3 text_field_halves.rs.)
fn refocused_field_shows_no_stale_selection_and_password_is_not_copied() {
    let (value, secret) = (TextFieldState::new("metin"), TextFieldState::new("gizli"));
    let (v, s) = (value.clone(), secret.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (v, s) = (v.clone(), s.clone());
        Steady(Rc::new(RecordingToolbar::default()), move || {
            Column(move || {
                BasicTextField(
                    v.clone(),
                    modifier = Modifier.width(200.dp()).test_tag("f"),
                    text_style = style16(),
                    line_limits = TextFieldLineLimits::SingleLine,
                );
                BasicSecureTextField(
                    s.clone(),
                    modifier = Modifier.width(200.dp()).test_tag("p"),
                    text_style = style16(),
                );
            });
        });
    }));
    let p = platform(&rule);
    p.set_clip("önceki");
    tap(&rule, bounds(&rule, "f").center());
    key(&rule, Key::A, CTRL);
    assert_eq!(value.selection(), TextRange::new(0, 5));
    assert_eq!(highlights(&rule, bounds(&rule, "f")).len(), 1);
    key(&rule, Key::Tab, NONE); // to the password field
    assert!(is_focused(&rule, "p"));
    key(&rule, Key::A, CTRL);
    assert_eq!(secret.selection(), TextRange::new(0, 5));
    key(&rule, Key::C, CTRL);
    assert_eq!(p.clipboard(), "önceki"); // not the password
    key(&rule, Key::Tab, SHIFT); // back: the cursor at the end, nothing selected
    assert!(is_focused(&rule, "f"));
    assert_eq!(value.selection(), TextRange::at(5));
    assert!(highlights(&rule, bounds(&rule, "f")).is_empty());
}

/// C++ 21136: a password field's selection is highlighted over the dots that stand for it ("bç": one dot in, two
/// dots wide, though "ç" takes two bytes and a dot three). (C++ checked Material's primary color at 40%: the
/// highlight is LocalTextSelectionColors' here.)
fn password_field_maps_its_selection_to_the_dots() {
    let secret = TextFieldState::new("abçd");
    let s = secret.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Steady(Rc::new(RecordingToolbar::default()), move || {
            BasicSecureTextField(
                s.clone(),
                modifier = Modifier.width(200.dp()).test_tag("şifre"),
                text_style = style16(),
            );
        });
    }));
    key(&rule, Key::Tab, NONE);
    assert!(is_focused(&rule, "şifre"));
    secret.edit(|b| b.set_selection(TextRange::new(1, 4))); // "bç"
    rule.wait_for_idle();
    let field = bounds(&rule, "şifre");
    let drawn = highlights(&rule, field);
    assert_eq!(drawn.len(), 1, "{drawn:?}");
    assert!(near(drawn[0].left, field.left + 8.0, 0.01) && near(drawn[0].width(), 16.0, 0.01), "{drawn:?}");
}

/// C++ 20943: a tap on a multi-line field's second line puts the cursor there; the input method is told a
/// multi-line field's Enter makes new lines, a single-line one's doesn't.
fn tapping_a_multi_line_field_puts_the_caret_on_that_line() {
    let state = TextFieldState::new("abc\ndefgh\nij");
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Steady(Rc::new(RecordingToolbar::default()), move || {
                Column(move || {
                    BasicTextField(
                        s.clone(),
                        modifier = Modifier.width(200.dp()).test_tag("f"),
                        text_style = style16(),
                    );
                    BasicTextField(
                        TextFieldState::new(""),
                        modifier = Modifier.width(200.dp()).test_tag("single"),
                        text_style = style16(),
                        line_limits = TextFieldLineLimits::SingleLine,
                    );
                });
            });
        }),
        IntSize::new(400, 800),
    );
    let p = platform(&rule);
    assert!(near(bounds(&rule, "f").height(), 60.0, 0.5)); // three lines
    tap(&rule, char_at(&rule, "f", 0, 1)); // the second line's start
    type_text(&rule, "X");
    assert_eq!(state.text(), "abc\nXdefgh\nij");
    assert_eq!(p.last_single_line(), Some(false));
    tap(&rule, bounds(&rule, "single").center());
    assert!(is_focused(&rule, "single"));
    assert_eq!(p.last_single_line(), Some(true));
}

/// C++ 6894: a right-to-left field's text sits at its right, its start rightmost: a tap at the right edge is the
/// start (Delete takes the first letter), one at the text's left edge its end (Backspace takes the last). (Given
/// the content's direction: Compose's unspecified one is the layout's.)
fn right_to_left_text_field_puts_the_start_at_the_right() {
    let state = TextFieldState::new("שלום");
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Steady(Rc::new(RecordingToolbar::default()), move || {
                BasicTextField(
                    s.clone(),
                    modifier = Modifier.fill_max_width().test_tag("field"),
                    text_style = TextStyle { text_direction: TextDirection::Content, ..style16() },
                    line_limits = TextFieldLineLimits::SingleLine,
                );
            });
        }),
        IntSize::new(400, 800),
    );
    let field = bounds(&rule, "field");
    // "שלום" is 32 px, 368..400.
    tap(&rule, at(field.left + 398.0, field.center().y));
    assert_eq!(state.selection(), TextRange::at(0));
    key(&rule, Key::Delete, NONE);
    assert_eq!(state.text(), "לום");
    // "לום" is 24 px, 376..400: its left edge is its end.
    rule.advance_time_by(500); // not a double tap
    tap(&rule, at(field.left + 377.0, field.center().y));
    assert_eq!(state.selection(), TextRange::at("לום".len()));
    key(&rule, Key::Backspace, NONE);
    assert_eq!(state.text(), "לו");
}

/// C++ 25310's text field half (its SelectionContainer half is in selection.rs): a toolbar provided through
/// LocalTextToolbar is told where the selection is and what the field can do — copy, paste (with text in the
/// clipboard), cut, select all — once while nothing changes, again when it does, hidden when the selection goes.
/// Compose: a press elsewhere (on nothing focusable) leaves it shown, a tap in the field hides it; made read-only
/// it isn't told again (read-only is no state of its), shown anew it offers copy and select all only.
fn text_toolbar_of_ones_own_is_told_instead_of_kompoz_drawing_one() {
    let state = TextFieldState::new("merhaba güzel dünya");
    let toolbar = Rc::new(RecordingToolbar::default());
    let locked = mutable_state_of(false);
    let rule = field_under_a_box(state.clone(), toolbar.clone(), locked.clone());
    let p = platform(&rule);
    p.set_clip("pano");
    let field = bounds(&rule, "field");
    long_press(&rule, char_at(&rule, "field", 10, 0)); // "güzel" selected
    assert_eq!(toolbar.shows.get(), 1);
    assert_eq!(toolbar.offers(), (true, true, true, true));
    // Over the selection: "güzel", characters 8..13, on the field's line.
    let r = toolbar.rect.get();
    assert!(near(r.left, field.left + 64.0, 0.01) && near(r.width(), 40.0, 0.01), "{r:?}");
    assert!(r.top >= field.top - 0.01 && r.bottom <= field.bottom + 0.01, "{r:?}");
    rule.wait_for_idle();
    rule.advance_time_by(100);
    assert_eq!(toolbar.shows.get(), 1); // told once while nothing changes
    toolbar.choose(&rule, &toolbar.copy);
    assert_eq!(p.clipboard(), "güzel");
    assert!(handles(&rule).is_empty() && toolbar.hides.get() == 1 && !toolbar.shown());
    // Select all from it: told again where the whole text is.
    long_press(&rule, char_at(&rule, "field", 3, 0));
    assert_eq!(toolbar.shows.get(), 2);
    toolbar.choose(&rule, &toolbar.select_all);
    assert_eq!(toolbar.shows.get(), 3);
    assert!(near(toolbar.rect.get().left, field.left, 0.01) && !toolbar.offers().3);
    // A press elsewhere leaves it; a tap in the field hides it.
    tap(&rule, bounds(&rule, "below").center());
    assert!(toolbar.shown() && toolbar.hides.get() == 1);
    rule.advance_time_by(500);
    tap(&rule, char_at(&rule, "field", 3, 0));
    assert!(!toolbar.shown() && toolbar.hides.get() == 2);
    // Made read-only while shown: not told again; shown anew, copy and select all only.
    rule.advance_time_by(500);
    long_press(&rule, char_at(&rule, "field", 3, 0));
    assert_eq!((toolbar.shows.get(), toolbar.offers()), (4, (true, true, true, true)));
    locked.set(true);
    rule.wait_for_idle();
    assert_eq!(toolbar.shows.get(), 4);
    long_press(&rule, char_at(&rule, "field", 10, 0));
    assert_eq!((toolbar.shows.get(), toolbar.offers()), (5, (true, false, false, true)));
}

/// C++ 6685's text field half (its SelectionContainer half is in selection.rs): while a finger drags a handle the
/// magnifier shows where the drag is — its x within the line, at the line's middle — zoomed, the toolbar out of
/// the way; both go back with the finger. (A handle's drag starts past the 8 px touch slop, which it doesn't move
/// by: 40 of the 48 px.)
fn dragging_a_handle_shows_the_text_magnifier() {
    let state = TextFieldState::new("merhaba güzel dünya");
    let toolbar = Rc::new(RecordingToolbar::default());
    let rule = field_under_a_box(state.clone(), toolbar.clone(), mutable_state_of(false));
    let field = bounds(&rule, "field");
    long_press(&rule, char_at(&rule, "field", 3, 0)); // "merhaba"
    assert!(magnifier_at(&rule).is_none()); // selecting doesn't show it
    assert!(toolbar.shown());
    let grab = at(field.left + 56.0 + 11.0, field.top + 20.0 + 11.0); // the end handle
    finger_drag(&rule, grab, at(8.0, 0.0), 6, false);
    let shown = magnifier_at(&rule).expect("the magnifier");
    assert!(near(shown.x, field.left + 56.0 + 40.0, 0.01) && near(shown.y, field.top + 10.0, 0.01), "{shown:?}");
    assert!(!toolbar.shown()); // out of the way meanwhile
    let canvas = rule.draw();
    assert!(canvas.layer_transforms.iter().any(|t| near(t.scale_x, 1.25, 0.01) && near(t.translation_y, -42.0, 0.01)));
    rule.host().pointer_up(grab + at(48.0, 0.0), -1, 0);
    rule.wait_for_idle();
    assert!(magnifier_at(&rule).is_none() && toolbar.shown()); // gone with the finger, the toolbar back
}

/// C++ 6749's text field half (its SelectionContainer half is in selection.rs): each handle's point is a square at
/// its top, towards the text — the start's on its right, the end's on its left; the start handle dragged up 30 px
/// (22 past the touch slop) goes up a line. (Compose's handles are 25 px, their squares 12.5; the C++'s 22 and 11.)
fn selection_handles_point_at_their_end_and_hold_its_line_by_the_middle() {
    let state = TextFieldState::new("merhaba güzel\ndünya burada");
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Steady(Rc::new(RecordingToolbar::default()), move || {
                Column(move || {
                    Box(modifier = Modifier.height(200.dp()));
                    BasicTextField(
                        s.clone(),
                        modifier = Modifier.fill_max_width().test_tag("field"),
                        text_style = style16(),
                    );
                });
            });
        }),
        IntSize::new(400, 800),
    );
    let field = bounds(&rule, "field");
    long_press(&rule, char_at(&rule, "field", 8, 1)); // in "burada", on the second line
    assert_eq!(state.selection(), TextRange::new(22, 28)); // (ü takes two bytes)
    let colors = LocalTextSelectionColors.default_value();
    let canvas = rule.draw();
    let mut points: Vec<Rect> = (0..canvas.rects.len())
        .filter(|&i| near(canvas.rects[i].width(), 12.5, 0.01) && near(canvas.rects[i].height(), 12.5, 0.01))
        .filter(|&i| canvas.rect_colors[i] == colors.handle_color)
        .map(|i| canvas.rects[i])
        .collect();
    points.sort_by(|a, b| a.left.total_cmp(&b.left));
    assert_eq!(points.len(), 2, "{points:?}");
    assert!(
        near(points[0].right, field.left + 48.0, 0.01) && near(points[0].top, field.top + 40.0, 0.01),
        "{points:?}"
    );
    assert!(near(points[1].left, field.left + 96.0, 0.01) && near(points[1].top, field.top + 40.0, 0.01), "{points:?}");
    let grab = at(field.left + 48.0 - 12.5, field.top + 40.0 + 12.5); // the start handle, in its middle
    finger_drag(&rule, grab, at(0.0, -5.0), 6, true);
    let selection = state.selection();
    assert!(selection.start < 14 && selection.end == 28, "{selection:?}");
}

tests!(
    long_press_selects_a_word_with_handles_and_a_toolbar,
    caret_handle_for_fingers_double_click_for_the_mouse,
    a_double_click_takes_letters_digits_underscores_and_what_is_beyond_ascii,
    the_mouse_selects_text_by_dragging_clicking_and_shift,
    refocused_field_shows_no_stale_selection_and_password_is_not_copied,
    password_field_maps_its_selection_to_the_dots,
    tapping_a_multi_line_field_puts_the_caret_on_that_line,
    right_to_left_text_field_puts_the_start_at_the_right,
    text_toolbar_of_ones_own_is_told_instead_of_kompoz_drawing_one,
    dragging_a_handle_shows_the_text_magnifier,
    selection_handles_point_at_their_end_and_hold_its_line_by_the_middle,
);

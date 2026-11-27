//! Typing on the web through the page's hidden text area (C++'s, which knew only what was committed): the text area
//! holds the focused field's text and selection (a window of a long field around its caret), so that the browser's
//! input methods — a desktop's composition, a phone's keyboard with its suggestions, its corrections and its
//! deletions — see the text around the caret (their surrounding text), and what they do to it comes back as the
//! field's edits:
//!
//! - outside a composition, what an `input` event changed (the text area's text against what it held: the part
//!   around the selection that differs) is the field's: the text deleted before and after the selection
//!   ([`Event::DeleteSurroundingText`]), then what replaces the selection ([`Event::Text`]);
//! - a composition (`compositionstart` … `compositionupdate` … `compositionend`) is the field's composing text
//!   ([`Event::Preedit`], its caret where the text area's is), committed at its end — the text area's own `input`
//!   events meanwhile only followed;
//! - the field changed by the app (`text_input_changed`) is set on the text area, unless a composition is under way
//!   (setting the text would end it in the browser): then once it has ended.
//!
//! Offsets are bytes of the field's text here (the events'), UTF-16 code units on the page.

use crate::app::Event;
use kompoz_ui::platform::TextEditState;

/// The most of a field the text area holds (bytes): around the caret beyond it.
pub const WINDOW: usize = 64 * 1024;

/// What the text area is set to: its text and its selection in UTF-16 code units.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Contents {
    pub text: String,
    pub start: u32,
    pub end: u32,
}

/// The text area as the platform knows it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TextArea {
    /// What it holds (as it was last set, or as the page last said it was).
    text: String,
    /// Its selection, bytes of `text`, the start not after the end.
    selection: (usize, usize),
    /// An input method composes in it.
    composing: bool,
    /// Where the composition began (the selection's start then), bytes of `text`.
    composition_start: usize,
    /// The field as the app said it while a composition was under way: set once it ends.
    deferred: Option<TextEditState>,
}

/// The byte offset of UTF-16 offset `utf16` in `text` (at a character's start; the end past it).
pub fn byte_offset(text: &str, utf16: u32) -> usize {
    let mut units = 0u32;
    for (i, c) in text.char_indices() {
        if units >= utf16 {
            return i;
        }
        units += c.len_utf16() as u32;
    }
    text.len()
}

/// The UTF-16 offset of byte offset `byte` (a character's start) in `text`.
pub fn utf16_offset(text: &str, byte: usize) -> u32 {
    text[..byte.min(text.len())].chars().map(|c| c.len_utf16() as u32).sum()
}

/// `at`, moved back to a character's start.
fn floor_boundary(text: &str, mut at: usize) -> usize {
    at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

impl TextArea {
    /// The field as the app has it (focused, or changed by the app): what the text area is to be set to, if that
    /// differs from what it holds (none while a composition is under way: then once it ends).
    pub fn field(&mut self, state: &TextEditState) -> Option<Contents> {
        if self.composing {
            self.deferred = Some(state.clone());
            return None;
        }
        let (text, selection) = window(state);
        if text == self.text && selection == self.selection {
            return None;
        }
        let contents = Contents {
            start: utf16_offset(&text, selection.0),
            end: utf16_offset(&text, selection.1),
            text: text.clone(),
        };
        self.text = text;
        self.selection = selection;
        Some(contents)
    }

    /// The text area emptied (no field).
    pub fn clear(&mut self) -> Option<Contents> {
        self.composing = false;
        self.deferred = None;
        self.field(&TextEditState::default())
    }

    /// The page's text and selection (UTF-16) in bytes.
    fn read(value: &str, start: u32, end: u32) -> (usize, usize) {
        let (a, b) = (byte_offset(value, start), byte_offset(value, end));
        (a.min(b), a.max(b))
    }

    /// An `input` event: the text area now holds `value`, its selection `start`..`end` (UTF-16). The field's edits.
    pub fn input(&mut self, value: &str, start: u32, end: u32) -> Vec<Event> {
        let selection = TextArea::read(value, start, end);
        if self.composing {
            self.text = value.to_string();
            self.selection = selection;
            return Vec::new();
        }
        let events = edits(&self.text, self.selection, value);
        self.text = value.to_string();
        self.selection = selection;
        events
    }

    /// `compositionstart`: the composition replaces the selection.
    pub fn composition_start(&mut self) {
        self.composing = true;
        self.composition_start = self.selection.0;
    }

    /// `compositionupdate`: the composing text is `data`, the text area holding `value` with its caret at `start`
    /// (UTF-16): the field's composing text, its caret there.
    pub fn composition_update(&mut self, data: &str, value: &str, start: u32, end: u32) -> Vec<Event> {
        if !self.composing {
            self.composition_start();
        }
        let selection = TextArea::read(value, start, end);
        let in_data = |at: usize| floor_boundary(data, at.saturating_sub(self.composition_start));
        let cursor = (in_data(selection.0), in_data(selection.1));
        self.text = value.to_string();
        self.selection = selection;
        vec![Event::Preedit { text: data.to_string(), cursor: (!data.is_empty()).then_some(cursor) }]
    }

    /// `compositionend`: `data` committed (none: the composition called off), the text area holding `value`. The
    /// field's edits, and what the text area is to be set to if the app changed the field meanwhile.
    pub fn composition_end(&mut self, data: &str, value: &str, start: u32, end: u32) -> (Vec<Event>, Option<Contents>) {
        self.composing = false;
        self.text = value.to_string();
        self.selection = TextArea::read(value, start, end);
        let mut events = vec![Event::Preedit { text: String::new(), cursor: None }];
        if !data.is_empty() {
            events.push(Event::Text(data.to_string()));
        }
        let set = self.deferred.take().and_then(|state| self.field(&state));
        (events, set)
    }
}

/// What of a field the text area holds: all of it, or the window around its selection a long one's (whole
/// characters), and the selection in it.
fn window(state: &TextEditState) -> (String, (usize, usize)) {
    let text = &state.text;
    let (s, e) = (state.selection_start.min(state.selection_end), state.selection_start.max(state.selection_end));
    let (s, e) = (floor_boundary(text, s), floor_boundary(text, e));
    if text.len() <= WINDOW {
        return (text.clone(), (s, e));
    }
    let half = WINDOW / 2;
    let from = floor_boundary(text, s.saturating_sub(half));
    let to = floor_boundary(text, (e + half).min(text.len()));
    (text[from..to].to_string(), (s - from, e - from))
}

/// The field's edits that make `old` (its selection `selection`) into `new`: the part that differs, widened to
/// hold the selection, is the text deleted before and after the selection and what replaces it.
fn edits(old: &str, selection: (usize, usize), new: &str) -> Vec<Event> {
    if old == new {
        return Vec::new();
    }
    let (s, e) = selection;
    let mut prefix = old.bytes().zip(new.bytes()).take_while(|(a, b)| a == b).count().min(s);
    prefix = floor_boundary(old, prefix).min(floor_boundary(new, prefix));
    let most = (old.len() - e).min(new.len() - prefix);
    let mut suffix = old.bytes().rev().zip(new.bytes().rev()).take_while(|(a, b)| a == b).count().min(most);
    while suffix > 0 && !(old.is_char_boundary(old.len() - suffix) && new.is_char_boundary(new.len() - suffix)) {
        suffix -= 1;
    }
    let (before, after) = (s - prefix, old.len() - suffix - e);
    let inserted = &new[prefix..new.len() - suffix];
    let mut events = Vec::new();
    if before > 0 || after > 0 {
        events.push(Event::DeleteSurroundingText { before, after });
    }
    if !inserted.is_empty() || s != e {
        events.push(Event::Text(inserted.to_string()));
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(text: &str, start: usize, end: usize) -> TextEditState {
        TextEditState { text: text.into(), selection_start: start, selection_end: end, composition: None }
    }

    fn area(text: &str, start: usize, end: usize) -> TextArea {
        let mut a = TextArea::default();
        a.field(&field(text, start, end));
        a
    }

    fn text(t: &str) -> Event {
        Event::Text(t.into())
    }

    #[test]
    fn utf16_and_bytes() {
        let s = "aş😀b";
        assert_eq!(byte_offset(s, 0), 0);
        assert_eq!(byte_offset(s, 2), 3);
        assert_eq!(byte_offset(s, 4), 7);
        assert_eq!(byte_offset(s, 5), 8);
        assert_eq!(byte_offset(s, 9), 8);
        assert_eq!(utf16_offset(s, 7), 4);
    }

    #[test]
    fn the_field_is_set_once_until_it_changes() {
        let mut a = TextArea::default();
        let set = a.field(&field("merhaba ş", 9, 11)).unwrap();
        assert_eq!(set, Contents { text: "merhaba ş".into(), start: 8, end: 9 });
        assert_eq!(a.field(&field("merhaba ş", 9, 11)), None);
        assert!(a.field(&field("merhaba ş", 0, 0)).is_some());
    }

    #[test]
    fn typing_deleting_and_corrections_are_the_fields_edits() {
        // Typed at the caret.
        let mut a = area("ab", 2, 2);
        assert_eq!(a.input("abc", 3, 3), [text("c")]);
        // Over a selection.
        let mut a = area("abc", 1, 2);
        assert_eq!(a.input("axc", 2, 2), [text("x")]);
        // A phone keyboard's backspace (a character of two bytes).
        let mut a = area("aş", 3, 3);
        assert_eq!(a.input("a", 1, 1), [Event::DeleteSurroundingText { before: 2, after: 0 }]);
        // The selection deleted.
        let mut a = area("abcd", 1, 3);
        assert_eq!(a.input("ad", 1, 1), [text("")]);
        // A correction of the word before the caret (and its space kept).
        let mut a = area("teh ", 4, 4);
        assert_eq!(a.input("the ", 4, 4), [Event::DeleteSurroundingText { before: 3, after: 0 }, text("he ")]);
        // Nothing changed (the caret moved): nothing.
        let mut a = area("abc", 1, 1);
        assert_eq!(a.input("abc", 2, 2), []);
    }

    #[test]
    fn a_composition_is_the_fields_composing_text_then_committed() {
        let mut a = area("x", 1, 1);
        a.composition_start();
        assert_eq!(a.composition_update("k", "xk", 2, 2), [Event::Preedit { text: "k".into(), cursor: Some((1, 1)) }]);
        // The text area's own input events meanwhile: followed only.
        assert_eq!(a.input("xか", 2, 2), []);
        assert_eq!(
            a.composition_update("かな", "xかな", 2, 2),
            [Event::Preedit { text: "かな".into(), cursor: Some((3, 3)) }]
        );
        // The app's own change meanwhile waits for the end.
        assert_eq!(a.field(&field("xかな", 7, 7)), None);
        let (events, set) = a.composition_end("仮名", "x仮名", 3, 3);
        assert_eq!(events, [Event::Preedit { text: String::new(), cursor: None }, text("仮名")]);
        assert_eq!(set, Some(Contents { text: "xかな".into(), start: 3, end: 3 }));
        // A composition called off: its text gone, nothing committed.
        let mut a = area("", 0, 0);
        a.composition_start();
        a.composition_update("a", "a", 1, 1);
        let (events, _) = a.composition_end("", "", 0, 0);
        assert_eq!(events, [Event::Preedit { text: String::new(), cursor: None }]);
        // The input event after the end (Firefox sends one): nothing more.
        let mut a = area("", 0, 0);
        a.composition_start();
        a.composition_end("é", "é", 1, 1);
        assert_eq!(a.input("é", 1, 1), []);
    }

    #[test]
    fn a_long_field_is_held_around_its_caret() {
        let long = "ş".repeat(WINDOW);
        let at = long.len() / 2;
        let mut a = TextArea::default();
        let set = a.field(&field(&long, at, at)).unwrap();
        assert!(set.text.len() <= WINDOW && set.text.len() >= WINDOW - 4);
        assert_eq!(byte_offset(&set.text, set.start), set.text.len() / 2);
        // Typing there: the field's edits, as in a short one.
        let mut typed = set.text.clone();
        let caret = byte_offset(&set.text, set.start);
        typed.insert(caret, 'a');
        assert_eq!(a.input(&typed, set.start + 1, set.start + 1), [text("a")]);
    }
}

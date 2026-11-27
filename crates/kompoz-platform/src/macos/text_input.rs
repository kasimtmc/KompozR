//! What AppKit's input methods see of the focused field (NSTextInputClient's model): its text, selection and
//! composition as the host last told them (`PlatformCallbacks::text_input`, `text_input_changed`, and after the app
//! core handled what was typed: the session's state), in AppKit's terms — ranges of UTF-16 code units of the whole
//! text, where KompozR's are UTF-8 bytes —, and what an input method does to it (`insertText:replacementRange:`,
//! `setMarkedText:selectedRange:replacementRange:`, `unmarkText`) as the platform's events for the app core, applied
//! here at once too (an input method asks again before the core has handled them: `selectedRange` right after
//! `insertText:` must already count what it inserted).
//!
//! A replacement range (the press-and-hold accent menu replacing the letter before the caret, a reconversion) is
//! the text around the selection deleted first (`Event::DeleteSurroundingText`), when it covers the selection; the
//! inserted or composed text then takes the selection's place. Elsewhere it can't be said with the events: what is
//! there stays, the text goes in at the selection.
//!
//! Plain Rust: tested on every host.

use crate::app::Event;
use kompoz_graphics::Rect;
use kompoz_ui::platform::TextEditState;
use kompoz_ui::text_input::EditCommand;

/// Foundation's NSNotFound (NSIntegerMax): no range.
pub const NOT_FOUND: usize = isize::MAX as usize;

/// The text's length in UTF-16 code units.
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Byte `byte` of `text` (a char boundary at or before it) in UTF-16 code units.
pub fn utf16_of_byte(text: &str, byte: usize) -> usize {
    let mut byte = byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    utf16_len(&text[..byte])
}

/// UTF-16 code unit `unit` of `text` as a byte offset (inside a surrogate pair: its character's start; past the
/// end: the end).
pub fn byte_of_utf16(text: &str, unit: usize) -> usize {
    let mut units = 0;
    for (i, c) in text.char_indices() {
        let next = units + c.len_utf16();
        if next > unit {
            return i;
        }
        units = next;
    }
    text.len()
}

/// The focused field as the input methods see it.
#[derive(Clone, Debug, Default)]
pub struct Ime {
    /// A field is focused (a session is open).
    pub active: bool,
    /// Where the field (its caret) is, px in the window.
    pub area: Rect,
    pub state: TextEditState,
}

impl Ime {
    fn text(&self) -> &str {
        &self.state.text
    }

    /// The selection, UTF-16 (location, length).
    pub fn selected_range(&self) -> (usize, usize) {
        let s = &self.state;
        let (a, b) = (s.selection_start.min(s.selection_end), s.selection_start.max(s.selection_end));
        let a16 = utf16_of_byte(self.text(), a);
        (a16, utf16_of_byte(self.text(), b) - a16)
    }

    /// The composition, UTF-16 (location, length); none without one.
    pub fn marked_range(&self) -> Option<(usize, usize)> {
        let (a, b) = self.state.composition?;
        let (a, b) = (a.min(b), a.max(b));
        if a == b {
            return None;
        }
        let a16 = utf16_of_byte(self.text(), a);
        Some((a16, utf16_of_byte(self.text(), b) - a16))
    }

    pub fn has_marked_text(&self) -> bool {
        self.marked_range().is_some()
    }

    /// The text of UTF-16 range (location, length), and the range it actually is (clamped to the text); none if it
    /// starts past the end.
    pub fn substring(&self, location: usize, length: usize) -> Option<(String, (usize, usize))> {
        let len = utf16_len(self.text());
        if location > len || location == NOT_FOUND {
            return None;
        }
        let end = location.saturating_add(length).min(len);
        let (a, b) = (byte_of_utf16(self.text(), location), byte_of_utf16(self.text(), end));
        let actual = (utf16_of_byte(self.text(), a), utf16_of_byte(self.text(), b) - utf16_of_byte(self.text(), a));
        Some((self.text()[a..b].to_string(), actual))
    }

    /// What deletes the replacement range (UTF-16) around the selection (before its start, after its end), applied
    /// here; none (nothing deleted) without one, while composing (the composition is what is replaced), or where it
    /// doesn't cover the selection.
    fn delete_replaced(&mut self, replacement: Option<(usize, usize)>) -> Option<Event> {
        let (location, length) = replacement.filter(|&(l, _)| l != NOT_FOUND)?;
        if self.has_marked_text() {
            return None;
        }
        let text = self.text();
        let (a, b) = (byte_of_utf16(text, location), byte_of_utf16(text, location.saturating_add(length)));
        let s = &self.state;
        let (start, end) = (s.selection_start.min(s.selection_end), s.selection_start.max(s.selection_end));
        if a > start || b < end || (a == start && b == end) {
            return None;
        }
        let (before, after) = (start - a, b - end);
        self.state.apply(&EditCommand::DeleteSurroundingText { before, after });
        Some(Event::DeleteSurroundingText { before, after })
    }

    /// `insertText:replacementRange:`: `text` in place of the composition, else the selection (and the replacement
    /// range around it); the events for the app core.
    pub fn insert(&mut self, text: &str, replacement: Option<(usize, usize)>) -> Vec<Event> {
        let mut events: Vec<Event> = self.delete_replaced(replacement).into_iter().collect();
        self.state.apply(&EditCommand::CommitText { text: text.to_string(), new_cursor_position: 1 });
        events.push(Event::Text(text.to_string()));
        events
    }

    /// `setMarkedText:selectedRange:replacementRange:`: `text` composing in place of the composition, else the
    /// selection (and the replacement range around it), `selected` (UTF-16 of `text`: its caret, or what it
    /// highlights) as the preedit's caret; empty: the composition gone. The events for the app core.
    pub fn set_marked(
        &mut self,
        text: &str,
        selected: (usize, usize),
        replacement: Option<(usize, usize)>,
    ) -> Vec<Event> {
        let mut events: Vec<Event> = self.delete_replaced(replacement).into_iter().collect();
        let start = byte_of_utf16(text, selected.0.min(utf16_len(text)));
        let end = byte_of_utf16(text, selected.0.saturating_add(selected.1));
        let at = {
            let s = &self.state;
            match s.composition {
                Some((a, b)) => a.min(b),
                None => s.selection_start.min(s.selection_end),
            }
            .min(s.text.len())
        };
        self.state.apply(&EditCommand::SetComposingText { text: text.to_string(), new_cursor_position: 1 });
        if !text.is_empty() && (start, end) != (text.len(), text.len()) {
            self.state.apply(&EditCommand::SetSelection { start: at + start, end: at + end });
        }
        events.push(Event::Preedit { text: text.to_string(), cursor: Some((start, end)) });
        events
    }

    /// `unmarkText`: the composition kept as it is (committed); the events for the app core (none without one).
    pub fn unmark(&mut self) -> Vec<Event> {
        let Some((a, b)) = self.state.composition else { return Vec::new() };
        let (a, b) = (a.min(b).min(self.state.text.len()), a.max(b).min(self.state.text.len()));
        let composed = self.state.text[a..b].to_string();
        self.state.apply(&EditCommand::CommitText { text: composed.clone(), new_cursor_position: 1 });
        vec![Event::Text(composed)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ime(text: &str, selection: (usize, usize), composition: Option<(usize, usize)>) -> Ime {
        Ime {
            active: true,
            area: Rect::new(0.0, 0.0, 10.0, 10.0),
            state: TextEditState {
                text: text.into(),
                selection_start: selection.0,
                selection_end: selection.1,
                composition,
            },
        }
    }

    #[test]
    fn ranges_are_utf16() {
        // "ğ😀a": ğ is 2 bytes and 1 unit, 😀 4 bytes and 2 units.
        let text = "ğ😀a";
        assert_eq!(utf16_len(text), 4);
        assert_eq!(utf16_of_byte(text, 2), 1);
        assert_eq!(utf16_of_byte(text, 6), 3);
        assert_eq!(utf16_of_byte(text, 4), 1); // (inside 😀: its start)
        assert_eq!(byte_of_utf16(text, 1), 2);
        assert_eq!(byte_of_utf16(text, 2), 2); // (between its halves: its start)
        assert_eq!(byte_of_utf16(text, 3), 6);
        assert_eq!(byte_of_utf16(text, 9), 7);
        let i = ime(text, (6, 2), Some((2, 6)));
        assert_eq!(i.selected_range(), (1, 2)); // (in order)
        assert_eq!(i.marked_range(), Some((1, 2)));
        assert_eq!(i.substring(1, 2), Some(("😀".into(), (1, 2))));
        assert_eq!(i.substring(3, 100), Some(("a".into(), (3, 1))));
        assert_eq!(i.substring(5, 1), None);
        assert!(!ime("", (0, 0), None).has_marked_text());
    }

    #[test]
    fn typing_inserts_at_the_caret_and_counts_at_once() {
        let mut i = ime("ab", (1, 1), None);
        assert_eq!(i.insert("ç", None), [Event::Text("ç".into())]);
        assert_eq!(i.state.text, "açb");
        assert_eq!(i.selected_range(), (2, 0));
    }

    #[test]
    fn composing_then_committing() {
        // Japanese: "か" composing, its caret at its end; then "漢字" with its first highlighted; then committed.
        let mut i = ime("x", (1, 1), None);
        assert_eq!(i.set_marked("か", (1, 0), None), [Event::Preedit { text: "か".into(), cursor: Some((3, 3)) }]);
        assert_eq!(i.marked_range(), Some((1, 1)));
        let events = i.set_marked("漢字", (0, 1), None);
        assert_eq!(events, [Event::Preedit { text: "漢字".into(), cursor: Some((0, 3)) }]);
        assert_eq!(i.state.text, "x漢字");
        assert_eq!(i.selected_range(), (1, 1));
        assert_eq!(i.insert("漢字", None), [Event::Text("漢字".into())]);
        assert_eq!((i.state.text.as_str(), i.state.composition), ("x漢字", None));
        assert_eq!(i.selected_range(), (3, 0));
        // Marked again and unmarked: kept.
        i.set_marked("ab", (2, 0), None);
        assert_eq!(i.unmark(), [Event::Text("ab".into())]);
        assert_eq!((i.state.text.as_str(), i.state.composition), ("x漢字ab", None));
        assert!(i.unmark().is_empty());
        // Emptied: the composition gone, nothing left of it.
        i.set_marked("q", (1, 0), None);
        assert_eq!(i.set_marked("", (0, 0), None), [Event::Preedit { text: String::new(), cursor: Some((0, 0)) }]);
        assert_eq!(i.state.text, "x漢字ab");
    }

    #[test]
    fn a_replacement_range_around_the_caret_is_deleted_first() {
        // Press and hold e: é replacing the e before the caret.
        let mut i = ime("se", (2, 2), None);
        let events = i.insert("é", Some((1, 1)));
        assert_eq!(events, [Event::DeleteSurroundingText { before: 1, after: 0 }, Event::Text("é".into())]);
        assert_eq!(i.state.text, "sé");
        // A reconversion of a selected word with a letter after it: composing over all of it.
        let mut i = ime("kanji!", (0, 5), None);
        let events = i.set_marked("漢字", (2, 0), Some((0, 6)));
        assert_eq!(events[0], Event::DeleteSurroundingText { before: 0, after: 1 });
        assert_eq!(i.state.text, "漢字");
        // Elsewhere (not around the selection): at the selection, nothing deleted.
        let mut i = ime("abc", (3, 3), None);
        assert_eq!(i.insert("d", Some((0, 1))), [Event::Text("d".into())]);
        assert_eq!(i.state.text, "abcd");
        // NSNotFound: none.
        assert_eq!(i.insert("e", Some((NOT_FOUND, 0))), [Event::Text("e".into())]);
    }
}

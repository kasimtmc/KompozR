//! What AppKit's input methods see of the focused field (NSTextInputClient's model): its text, selection and
//! composition as the host last told them (`PlatformCallbacks::text_input`, `text_input_changed`, and after the app
//! core handled what was typed: the session's state), in AppKit's terms — ranges of UTF-16 code units of the whole
//! text, where KompozR's are UTF-8 bytes —, and what an input method does to it (`insertText:replacementRange:`,
//! `setMarkedText:selectedRange:replacementRange:`, `unmarkText`) as the platform's events for the app core, applied
//! here at once too (an input method asks again before the core has handled them: `selectedRange` right after
//! `insertText:` must already count what it inserted).
//!
//! A replacement range (the press-and-hold accent menu replacing the letter before the caret, a reconversion of a
//! word anywhere in the field, a dictation's correction) is selected first (`Event::SetSelection`, as Windows' text
//! services do), wherever it is; the inserted or composed text then takes the selection's place, as AppKit's own
//! text views do.
//!
//! Where its characters are (`firstRectForCharacterRange:`, `characterIndexForPoint:`) comes from the field's text
//! layout while its session is open (`PlatformCallbacks::update_text_layout_result`); the caret's box the host
//! tells stands for them until the field is laid out.
//!
//! Plain Rust: tested on every host.

use crate::app::Event;
use kompoz_graphics::{Offset, Rect};
use kompoz_ui::platform::TextEditState;
use kompoz_ui::platform_text_layout::{PlatformTextLayout, caret_area};
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
    /// Its text layout as last told (none before it was laid out, or once its session closed).
    pub layout: Option<PlatformTextLayout>,
}

impl Ime {
    fn text(&self) -> &str {
        &self.state.text
    }

    /// Its text layout, where it is laid out for the text the input methods see.
    fn laid_out(&self) -> Option<&PlatformTextLayout> {
        self.layout.as_ref().filter(|l| l.text == self.state.text)
    }

    /// Where the part of UTF-16 range (`location`, `length`) on its first line is, px in the window, and the range
    /// that part is (`firstRectForCharacterRange:actualRange:`): its characters' boxes on that line (the caret's at
    /// an empty range: at its glyph); the caret's box for the range asked for until the field is laid out.
    pub fn first_rect(&self, location: usize, length: usize) -> (Rect, (usize, usize)) {
        let Some(layout) = self.laid_out() else { return (self.area, (location, length)) };
        let text = self.text();
        let start = byte_of_utf16(text, location.min(utf16_len(text)));
        let end = byte_of_utf16(text, location.saturating_add(length).min(utf16_len(text)));
        if start == end {
            return (
                caret_area(self.area, &TextEditState { selection_end: start, ..self.state.clone() }, Some(layout)),
                (utf16_of_byte(text, start), 0),
            );
        }
        let (rect, until) = layout.first_rect(start, end);
        let (a, b) = (utf16_of_byte(text, start), utf16_of_byte(text, until));
        (rect, (a, b - a))
    }

    /// The character (its UTF-16 index) at `point` (px in the window: `characterIndexForPoint:`): the one whose
    /// box holds it as the field is laid out, none elsewhere; until it is laid out, the caret's where the point is in
    /// the caret's box.
    pub fn index_at(&self, point: Offset) -> usize {
        if !self.active {
            return NOT_FOUND;
        }
        match self.laid_out() {
            Some(layout) => {
                layout.character_at(point).map_or(NOT_FOUND, |i| utf16_of_byte(self.text(), layout.characters[i].start))
            }
            None if self.area.left <= point.x
                && point.x <= self.area.right
                && self.area.top <= point.y
                && point.y <= self.area.bottom =>
            {
                utf16_of_byte(self.text(), self.state.selection_end)
            }
            None => NOT_FOUND,
        }
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

    /// What selects the replacement range (UTF-16, clamped to the text and to whole characters), applied here; none
    /// (nothing to do) without one, while composing (the composition is what is replaced: AppKit's input methods
    /// replace a range only with nothing marked), or where it is the selection already.
    fn select_replaced(&mut self, replacement: Option<(usize, usize)>) -> Option<Event> {
        let (location, length) = replacement.filter(|&(l, _)| l != NOT_FOUND)?;
        if self.has_marked_text() {
            return None;
        }
        let text = self.text();
        let (start, end) = (byte_of_utf16(text, location), byte_of_utf16(text, location.saturating_add(length)));
        let s = &self.state;
        if (start, end) == (s.selection_start.min(s.selection_end), s.selection_start.max(s.selection_end)) {
            return None;
        }
        self.state.apply(&EditCommand::SetSelection { start, end });
        Some(Event::SetSelection { start, end })
    }

    /// `insertText:replacementRange:`: `text` in place of the composition, else the replacement range (selected
    /// first), else the selection; the events for the app core.
    pub fn insert(&mut self, text: &str, replacement: Option<(usize, usize)>) -> Vec<Event> {
        let mut events: Vec<Event> = self.select_replaced(replacement).into_iter().collect();
        self.state.apply(&EditCommand::CommitText { text: text.to_string(), new_cursor_position: 1 });
        events.push(Event::Text(text.to_string()));
        events
    }

    /// `setMarkedText:selectedRange:replacementRange:`: `text` composing in place of the composition, else the
    /// replacement range (selected first), else the selection, `selected` (UTF-16 of `text`: its caret, or what it
    /// highlights) as the preedit's caret; empty: the composition gone. The events for the app core.
    pub fn set_marked(
        &mut self,
        text: &str,
        selected: (usize, usize),
        replacement: Option<(usize, usize)>,
    ) -> Vec<Event> {
        let mut events: Vec<Event> = self.select_replaced(replacement).into_iter().collect();
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
            layout: None,
        }
    }

    #[test]
    fn where_characters_are_comes_from_the_layout() {
        // "ağaç dalı", 8 px a character from 10, 20, wrapped after "ağaç " (20 px lines).
        let mut i = ime("ağaç dalı", (1, 1), None);
        i.area = Rect::new(18.0, 20.0, 26.0, 40.0);
        // Not laid out: the caret's box for any range, the caret where the point is in it.
        assert_eq!(i.first_rect(2, 3), (i.area, (2, 3)));
        assert_eq!(i.index_at(Offset::new(20.0, 30.0)), 1);
        assert_eq!(i.index_at(Offset::new(60.0, 30.0)), NOT_FOUND);
        let layout = crate::accessibility::text::test_layout("ağaç dalı", Offset::new(10.0, 20.0), 8.0, 20.0, 5);
        i.layout = Some(layout);
        // The first line's part of "ğaç da": "ğaç " (four units), its box; an empty range's: the caret's glyph.
        assert_eq!(i.first_rect(1, 6), (Rect::new(18.0, 20.0, 50.0, 40.0), (1, 4)));
        assert_eq!(i.first_rect(6, 2), (Rect::new(18.0, 40.0, 34.0, 60.0), (6, 2)));
        assert_eq!(i.first_rect(2, 0), (Rect::new(26.0, 20.0, 27.0, 40.0), (2, 0)));
        // A point's character, by real hit-testing.
        assert_eq!(i.index_at(Offset::new(31.0, 30.0)), 2);
        assert_eq!(i.index_at(Offset::new(20.0, 50.0)), 6);
        assert_eq!(i.index_at(Offset::new(300.0, 30.0)), NOT_FOUND);
        // Laid out for another text (it changed since): not read.
        i.state.text.push('!');
        assert_eq!(i.first_rect(1, 6).0, i.area);
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
    fn a_replacement_range_is_selected_first_wherever_it_is() {
        // Press and hold e: é replacing the e before the caret.
        let mut i = ime("se", (2, 2), None);
        let events = i.insert("é", Some((1, 1)));
        assert_eq!(events, [Event::SetSelection { start: 1, end: 2 }, Event::Text("é".into())]);
        assert_eq!(i.state.text, "sé");
        assert_eq!(i.selected_range(), (2, 0));
        // A reconversion of a selected word with a letter after it: composing over all of it.
        let mut i = ime("kanji!", (0, 5), None);
        let events = i.set_marked("漢字", (2, 0), Some((0, 6)));
        assert_eq!(events[0], Event::SetSelection { start: 0, end: 6 });
        assert_eq!(i.state.text, "漢字");
        assert_eq!(i.marked_range(), Some((0, 2)));
        // A word away from the caret (a reconversion, a correction): replaced where it is, the caret after it.
        let mut i = ime("bir iki üç", (3, 3), None);
        let events = i.insert("dört", Some((4, 3)));
        assert_eq!(events, [Event::SetSelection { start: 4, end: 7 }, Event::Text("dört".into())]);
        assert_eq!(i.state.text, "bir dört üç");
        assert_eq!(i.selected_range(), (8, 0));
        // In UTF-16 (😀 two units, four bytes), past the end clamped.
        let mut i = ime("😀a", (0, 0), None);
        assert_eq!(i.insert("b", Some((2, 9)))[0], Event::SetSelection { start: 4, end: 5 });
        assert_eq!(i.state.text, "😀b");
        // The selection already, NSNotFound, or while composing: nothing selected.
        let mut i = ime("abc", (1, 2), None);
        assert_eq!(i.insert("x", Some((1, 1))), [Event::Text("x".into())]);
        assert_eq!(i.insert("e", Some((NOT_FOUND, 0))), [Event::Text("e".into())]);
        i.set_marked("k", (1, 0), None);
        assert_eq!(i.set_marked("ka", (2, 0), Some((0, 1))).len(), 1);
    }
}

//! The text an editor's commands work on (Compose's `androidx.compose.ui.text.input.EditingBuffer`): its text, its
//! selection (in order: a caret when collapsed) and the part an input method is composing. What both text field
//! engines apply an input method's [`EditCommand`](crate::input::EditCommand)s to: the value-based field through
//! its [`EditProcessor`](crate::input::EditProcessor), the state-based one (on the desktop) through one too, as
//! Compose's do.
//!
//! Offsets are UTF-8 byte offsets (Compose counts UTF-16 units). Rust's text can't be cut inside a character, so an
//! offset off a character boundary is moved to one: a range's start back and its end forward (it covers whole
//! characters), a selection's ends back.

use crate::annotated_string::AnnotatedString;
use crate::range::TextRange;

/// `at` (clamped to `text`) moved back to a character boundary.
pub(crate) fn floor_char(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// `at` (clamped to `text`) moved forward to a character boundary.
pub(crate) fn ceil_char(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at += 1;
    }
    at
}

/// An editor's text, selection and composition, as edit commands change them (Compose's EditingBuffer).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EditingBuffer {
    text: String,
    selection_start: usize,
    selection_end: usize,
    composition: Option<(usize, usize)>,
}

impl EditingBuffer {
    /// `text` with `selection` (its ends in order). Panics, as Compose's, if the selection is outside the text.
    pub fn new(text: &AnnotatedString, selection: TextRange) -> EditingBuffer {
        EditingBuffer::from_text(text.text(), selection)
    }

    /// Plain `text` with `selection`.
    pub fn from_text(text: &str, selection: TextRange) -> EditingBuffer {
        let (start, end) = (selection.min(), selection.max());
        if start > text.len() {
            panic!("start ({start}) offset is outside of text region {}", text.len());
        }
        if end > text.len() {
            panic!("end ({end}) offset is outside of text region {}", text.len());
        }
        EditingBuffer {
            text: text.to_string(),
            selection_start: floor_char(text, start),
            selection_end: floor_char(text, end),
            composition: None,
        }
    }

    /// The text's length (bytes).
    pub fn length(&self) -> usize {
        self.text.len()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn selection_start(&self) -> usize {
        self.selection_start
    }

    pub fn selection_end(&self) -> usize {
        self.selection_end
    }

    /// The selection, in order.
    pub fn selection(&self) -> TextRange {
        TextRange::new(self.selection_start, self.selection_end)
    }

    /// Whether something is being composed.
    pub fn has_composition(&self) -> bool {
        self.composition.is_some()
    }

    /// What is being composed, if anything.
    pub fn composition(&self) -> Option<TextRange> {
        self.composition.map(|(s, e)| TextRange::new(s, e))
    }

    /// Where the composition starts (Compose's `compositionStart`; none: NOWHERE).
    pub fn composition_start(&self) -> Option<usize> {
        self.composition.map(|c| c.0)
    }

    /// Where the composition ends.
    pub fn composition_end(&self) -> Option<usize> {
        self.composition.map(|c| c.1)
    }

    /// The caret, if the selection is collapsed (Compose's `cursor`, -1 when it isn't).
    pub fn cursor(&self) -> Option<usize> {
        (self.selection_start == self.selection_end).then_some(self.selection_end)
    }

    /// Collapses the selection to a caret at `cursor` (Compose's `cursor = …`).
    pub fn set_cursor(&mut self, cursor: usize) {
        self.set_selection(cursor, cursor);
    }

    fn check_range(&self, start: usize, end: usize) {
        if start > self.text.len() {
            panic!("start ({start}) offset is outside of text region {}", self.text.len());
        }
        if end > self.text.len() {
            panic!("end ({end}) offset is outside of text region {}", self.text.len());
        }
        if start > end {
            panic!("Do not set reversed range: {start} > {end}");
        }
    }

    /// `text` in place of `start..end` (whole characters), the caret after it, the composition finished.
    pub fn replace(&mut self, start: usize, end: usize, text: &str) {
        self.check_range(start, end);
        let (start, end) = (floor_char(&self.text, start), ceil_char(&self.text, end));
        self.text.replace_range(start..end, text);
        self.selection_start = start + text.len();
        self.selection_end = start + text.len();
        self.composition = None;
    }

    /// `start..end` (whole characters, in either order) taken out; the selection and the composition move with
    /// the text after it (a composition left empty is finished).
    pub fn delete(&mut self, start: usize, end: usize) {
        let (lo, hi) = (start.min(end), start.max(end));
        let (lo, hi) = (floor_char(&self.text, lo), ceil_char(&self.text, hi));
        let deleted = TextRange::new(lo, hi);
        self.text.replace_range(lo..hi, "");
        let selection = update_range_after_delete(self.selection(), deleted);
        self.selection_start = selection.min();
        self.selection_end = selection.max();
        if let Some((s, e)) = self.composition {
            let composition = update_range_after_delete(TextRange::new(s, e), deleted);
            if composition.collapsed() {
                self.commit_composition();
            } else {
                self.composition = Some((composition.min(), composition.max()));
            }
        }
    }

    /// The selection from `start` to `end` (moved back to character boundaries). Panics if it is outside the
    /// text or reversed, as Compose's.
    pub fn set_selection(&mut self, start: usize, end: usize) {
        self.check_range(start, end);
        self.selection_start = floor_char(&self.text, start);
        self.selection_end = floor_char(&self.text, end);
    }

    /// `start..end` composing (whole characters). Panics if it is outside the text, reversed or empty.
    pub fn set_composition(&mut self, start: usize, end: usize) {
        if start > self.text.len() {
            panic!("start ({start}) offset is outside of text region {}", self.text.len());
        }
        if end > self.text.len() {
            panic!("end ({end}) offset is outside of text region {}", self.text.len());
        }
        if start >= end {
            panic!("Do not set reversed or empty range: {start} > {end}");
        }
        self.composition = Some((floor_char(&self.text, start), ceil_char(&self.text, end)));
    }

    /// The composed text taken out, nothing composing.
    pub fn cancel_composition(&mut self) {
        if let Some((s, e)) = self.composition {
            self.replace(s, e, "");
        }
        self.composition = None;
    }

    /// The composition finished: its text stays.
    pub fn commit_composition(&mut self) {
        self.composition = None;
    }

    /// The text as an annotated string (no annotations).
    pub fn to_annotated_string(&self) -> AnnotatedString {
        AnnotatedString::new(self.text.as_str())
    }
}

impl std::fmt::Display for EditingBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

/// `target` after `deleted` was taken out of the text it is a range of (Compose's `updateRangeAfterDelete`): a part
/// of it deleted shrinks it, all of it collapses it where the deletion was.
pub fn update_range_after_delete(target: TextRange, deleted: TextRange) -> TextRange {
    let (mut min, mut max) = (target.min(), target.max());
    if deleted.intersects(target) {
        if deleted.contains(target) {
            min = deleted.min();
            max = min;
        } else if target.contains(deleted) {
            max -= deleted.length();
        } else if deleted.contains_offset(min) {
            min = deleted.min();
            max -= deleted.length();
        } else {
            max = deleted.min();
        }
    } else if max > deleted.min() {
        min -= deleted.length();
        max -= deleted.length();
    }
    TextRange::new(min, max)
}

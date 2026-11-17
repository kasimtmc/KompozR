//! What an input method (or a key) does to the text being edited (Compose's `EditCommand` and its implementations
//! in `androidx.compose.ui.text.input`), applied to an [`EditingBuffer`] as Compose's are. Both text field engines
//! take an input method's commands as these.
//!
//! Offsets and lengths are UTF-8 bytes (Compose's are UTF-16 units), whole characters as [`EditingBuffer`] keeps
//! them; [`DeleteSurroundingTextInCodePoints`](EditCommand::DeleteSurroundingTextInCodePoints) counts code points,
//! [`MoveCursor`](EditCommand::MoveCursor) and [`Backspace`](EditCommand::Backspace) grapheme clusters, as
//! Compose's.

use crate::editing_buffer::EditingBuffer;
use crate::segment::{find_following_break, find_preceding_break};

/// An edit of the text, its selection or its composition (Compose's EditCommand classes, one variant each).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EditCommand {
    /// `text` in place of the composition (or, without one, the selection), the composition finished; the caret
    /// then `new_cursor_position` from it: above 0, that many minus one after the text's end; else that many
    /// before its start (Compose's CommitTextCommand).
    CommitText { text: String, new_cursor_position: i32 },
    /// The text from `start` to `end` (either order) composing, none if they are the same; a composition before
    /// finished (Compose's SetComposingRegionCommand).
    SetComposingRegion { start: usize, end: usize },
    /// `text` in place of the composition (or the selection), and composing (none if it is empty); the caret as
    /// for [`CommitText`](EditCommand::CommitText) (Compose's SetComposingTextCommand).
    SetComposingText { text: String, new_cursor_position: i32 },
    /// `after` bytes after the selection deleted, then `before` bytes before it, whole characters (Compose's
    /// DeleteSurroundingTextCommand, its `lengthAfterCursor` and `lengthBeforeCursor`).
    DeleteSurroundingText { before: usize, after: usize },
    /// `after` code points after the selection deleted, then `before` before it (Compose's
    /// DeleteSurroundingTextInCodePointsCommand).
    DeleteSurroundingTextInCodePoints { before: usize, after: usize },
    /// The selection from `start` to `end` (put in order), clamped to the text (Compose's SetSelectionCommand).
    SetSelection { start: usize, end: usize },
    /// The composition finished: its text stays (Compose's FinishComposingTextCommand).
    FinishComposingText,
    /// The composition deleted, else the selection, else the grapheme cluster before the caret (Compose's
    /// BackspaceCommand).
    Backspace,
    /// The caret moved `amount` grapheme clusters (back if negative), from the selection's start when there is a
    /// selection (Compose's MoveCursorCommand).
    MoveCursor { amount: i32 },
    /// All the text deleted (Compose's DeleteAllCommand).
    DeleteAll,
}

/// Where the caret goes after `text` went in ending at `cursor`, `new_cursor_position` from it (Compose's rule).
fn cursor_after(cursor: usize, new_cursor_position: i32, text_len: usize, length: usize) -> usize {
    let cursor = cursor as i64;
    let p = new_cursor_position as i64;
    let at = if p > 0 { cursor + p - 1 } else { cursor + p - text_len as i64 };
    at.clamp(0, length as i64) as usize
}

impl EditCommand {
    /// Applies it to `buffer` (Compose's `applyTo`).
    pub fn apply_to(&self, buffer: &mut EditingBuffer) {
        match self {
            EditCommand::CommitText { text, new_cursor_position } => {
                match buffer.composition() {
                    Some(c) => buffer.replace(c.start, c.end, text),
                    None => buffer.replace(buffer.selection_start(), buffer.selection_end(), text),
                }
                let cursor = buffer.cursor().unwrap_or(buffer.selection_end());
                let at = cursor_after(cursor, *new_cursor_position, text.len(), buffer.length());
                buffer.set_cursor(at);
            }
            EditCommand::SetComposingRegion { start, end } => {
                if buffer.has_composition() {
                    buffer.commit_composition();
                }
                let (start, end) = ((*start).min(buffer.length()), (*end).min(buffer.length()));
                if start < end {
                    buffer.set_composition(start, end);
                } else if end < start {
                    buffer.set_composition(end, start);
                }
            }
            EditCommand::SetComposingText { text, new_cursor_position } => {
                let start = match buffer.composition() {
                    Some(c) => {
                        buffer.replace(c.start, c.end, text);
                        c.start
                    }
                    None => {
                        let start = buffer.selection_start();
                        buffer.replace(start, buffer.selection_end(), text);
                        start
                    }
                };
                if !text.is_empty() {
                    buffer.set_composition(start, start + text.len());
                }
                let cursor = buffer.cursor().unwrap_or(buffer.selection_end());
                let at = cursor_after(cursor, *new_cursor_position, text.len(), buffer.length());
                buffer.set_cursor(at);
            }
            EditCommand::DeleteSurroundingText { before, after } => {
                let end = buffer.selection_end().saturating_add(*after).min(buffer.length());
                buffer.delete(buffer.selection_end(), end);
                let start = buffer.selection_start().saturating_sub(*before);
                buffer.delete(start, buffer.selection_start());
            }
            EditCommand::DeleteSurroundingTextInCodePoints { before, after } => {
                let text = buffer.text();
                let after_end = text[buffer.selection_end()..]
                    .char_indices()
                    .nth(*after)
                    .map_or(text.len(), |(i, _)| buffer.selection_end() + i);
                let before_start = if *before == 0 {
                    buffer.selection_start()
                } else {
                    text[..buffer.selection_start()].char_indices().rev().nth(*before - 1).map_or(0, |(i, _)| i)
                };
                buffer.delete(buffer.selection_end(), after_end);
                buffer.delete(before_start, buffer.selection_start());
            }
            EditCommand::SetSelection { start, end } => {
                let (start, end) = ((*start).min(buffer.length()), (*end).min(buffer.length()));
                buffer.set_selection(start.min(end), start.max(end));
            }
            EditCommand::FinishComposingText => buffer.commit_composition(),
            EditCommand::Backspace => {
                if let Some(c) = buffer.composition() {
                    buffer.delete(c.start, c.end);
                    return;
                }
                let Some(cursor) = buffer.cursor() else {
                    let (start, end) = (buffer.selection_start(), buffer.selection_end());
                    buffer.set_cursor(start);
                    buffer.delete(start, end);
                    return;
                };
                if cursor == 0 {
                    return;
                }
                let previous = find_preceding_break(buffer.text(), cursor).unwrap_or(0);
                buffer.delete(previous, cursor);
            }
            EditCommand::MoveCursor { amount } => {
                if buffer.cursor().is_none() {
                    buffer.set_cursor(buffer.selection_start());
                }
                let mut cursor = buffer.selection_start();
                let text = buffer.text();
                if *amount > 0 {
                    for _ in 0..*amount {
                        match find_following_break(text, cursor) {
                            Some(next) => cursor = next,
                            None => break,
                        }
                    }
                } else {
                    for _ in 0..amount.unsigned_abs() {
                        match find_preceding_break(text, cursor) {
                            Some(previous) => cursor = previous,
                            None => break,
                        }
                    }
                }
                buffer.set_cursor(cursor);
            }
            EditCommand::DeleteAll => {
                let length = buffer.length();
                buffer.replace(0, length, "");
            }
        }
    }
}

//! A state-based field's input method session (Compose's `platformSpecificTextInputSession`, the desktop's): the
//! request the field makes of the platform's input method — its untransformed value and state to read, its IME
//! options, what to do with the input method's commands and action key, where it is — started in the field's text
//! input session ([`establish_text_input_session`](kompoz_ui::platform_text_input::establish_text_input_session)).
//!
//! The input method's commands are applied to the state as Compose's Android session applies them (its
//! ImeEditCommand edits: each a precise replacement of the composition or the selection, a common prefix and suffix
//! left alone), all of a batch in one edit as the user (through the field's input transformation, merged into the
//! undo history as typing is). Compose desktop runs them through an EditProcessor and replaces all the text with the
//! result instead; KompozR's desktop platforms send every typed character through the input session (Compose
//! desktop types most through key events), so its edits must be typing's: ranges as precise as a key's. The input
//! method sees the untransformed text (as Compose desktop's does): its offsets are the state's.

use super::text_layout_state::TextLayoutState;
use super::transformed_text_field_state::TransformedTextFieldState;
use crate::content::internal::ReceiveContentConfiguration;
use crate::content::{PlatformTransferableContent, TransferableContent, TransferableContentSource};
use crate::text::input::text_field_buffer::adjust_text_range;
use crate::text::input::{TextFieldBuffer, TextFieldCharSequence};
use kompoz_graphics::Offset;
use kompoz_text::input::{EditCommand, TextFieldValue};
use kompoz_text::segment::{find_following_break, find_preceding_break};
use kompoz_text::{TextLayoutResult, TextRange};
use kompoz_ui::clipboard::ClipEntry;
use kompoz_ui::platform::{ImeAction, ImeOptions};
use kompoz_ui::platform_text_input::{
    PlatformTextInputMethodRequest, PlatformTextInputSessionScope, SessionFuture, TextEditingScope, TextEditorState,
};
use std::rc::Rc;

/// A field's value as the input method sees it: its untransformed text, selection and composition.
fn to_text_field_value(value: &TextFieldCharSequence) -> TextFieldValue {
    TextFieldValue::from_text(value.text(), value.selection(), value.composition())
}

/// Replaces `start..end` with `text` as an input method's edit (Compose's `imeReplace`): only the part that
/// differs from what is there replaced (the composition finished either way), the caret after the text.
pub fn ime_replace(buffer: &mut TextFieldBuffer, start: usize, end: usize, text: &str) {
    let (min, max) = (start.min(end), start.max(end));
    let current = buffer.as_char_sequence();
    let (mut i, mut c_min) = (0, min);
    while c_min < max && i < text.len() {
        let (Some(a), Some(b)) = (text[i..].chars().next(), current[c_min..].chars().next()) else { break };
        if a != b || c_min + b.len_utf8() > max {
            break;
        }
        i += a.len_utf8();
        c_min += b.len_utf8();
    }
    let (mut j, mut c_max) = (text.len(), max);
    while c_max > c_min && j > i {
        let (Some(a), Some(b)) = (text[..j].chars().next_back(), current[..c_max].chars().next_back()) else { break };
        if a != b || c_max - b.len_utf8() < c_min || j - a.len_utf8() < i {
            break;
        }
        j -= a.len_utf8();
        c_max -= b.len_utf8();
    }
    if c_min != c_max || i != j {
        buffer.replace_from(c_min, c_max, text, i, j, false);
    } else {
        buffer.commit_composition();
        buffer.clear_highlight();
    }
    let at = (min + text.len()).min(buffer.length());
    buffer.set_selection(TextRange::at(at));
}

/// Deletes `start..end` as an input method's edit (Compose's `imeDelete`): the composition kept around what is
/// left of it.
pub fn ime_delete(buffer: &mut TextFieldBuffer, start: usize, end: usize) {
    buffer.suggestion_selected = false;
    let initial = buffer.composition();
    let (min, max) = (start.min(end), start.max(end));
    buffer.delete(min, max);
    if let Some(c) = initial {
        let adjusted = adjust_text_range(c, min, max, 0);
        if adjusted.collapsed() {
            buffer.commit_composition();
        } else {
            buffer.set_composition(adjusted.min(), adjusted.max(), None);
        }
    }
}

/// Where the caret goes after text of `text_len` bytes went in at `cursor` (the selection's start), as an input
/// method's `newCursorPosition` says.
fn new_cursor(cursor: usize, new_cursor_position: i32, text_len: usize, length: usize) -> usize {
    let (c, p) = (cursor as i64, new_cursor_position as i64);
    let at = if p > 0 { c + p - 1 } else { c + p - text_len as i64 };
    at.clamp(0, length as i64) as usize
}

/// Applies an input method's `command` to `buffer` (the field's untransformed text), as Compose's Android session
/// applies them (ImeEditCommand's `commitText`, `setComposingText`, `setComposingRegion`, `deleteSurroundingText`,
/// `deleteSurroundingTextInCodePoints`, `finishComposingText`, `setSelection`), and the EditProcessor's others
/// (Backspace, MoveCursor, DeleteAll) as their EditCommands do.
pub fn apply_ime_edit_command(buffer: &mut TextFieldBuffer, command: &EditCommand) {
    match command {
        EditCommand::CommitText { text, new_cursor_position } => {
            let range = buffer.composition().unwrap_or(buffer.selection());
            ime_replace(buffer, range.min(), range.max(), text);
            let cursor = new_cursor(buffer.selection().min(), *new_cursor_position, text.len(), buffer.length());
            buffer.suggestion_selected = false;
            buffer.set_selection(TextRange::at(cursor));
        }
        EditCommand::SetComposingText { text, new_cursor_position } => {
            let start = match buffer.composition() {
                Some(c) => {
                    ime_replace(buffer, c.min(), c.max(), text);
                    c.min()
                }
                None => {
                    let s = buffer.selection();
                    ime_replace(buffer, s.min(), s.max(), text);
                    s.min()
                }
            };
            if !text.is_empty() {
                buffer.set_composition(start, start + text.len(), None);
            }
            let cursor = new_cursor(buffer.selection().min(), *new_cursor_position, text.len(), buffer.length());
            buffer.suggestion_selected = false;
            buffer.set_selection(TextRange::at(cursor));
        }
        EditCommand::SetComposingRegion { start, end } => {
            if buffer.has_composition() {
                buffer.commit_composition();
            }
            let (start, end) = ((*start).min(buffer.length()), (*end).min(buffer.length()));
            if start != end {
                buffer.set_composition(start.min(end), start.max(end), None);
            }
        }
        EditCommand::DeleteSurroundingText { before, after } => {
            let selection = buffer.selection();
            let length = buffer.length();
            let end = selection.max().saturating_add(*after).min(length);
            ime_delete(buffer, selection.max(), end);
            let start = selection.min().saturating_sub(*before);
            ime_delete(buffer, start, selection.min());
        }
        EditCommand::DeleteSurroundingTextInCodePoints { before, after } => {
            let selection = buffer.selection();
            let text = buffer.as_char_sequence();
            let after_end =
                text[selection.max()..].char_indices().nth(*after).map_or(text.len(), |(i, _)| selection.max() + i);
            let before_start = if *before == 0 {
                selection.min()
            } else {
                text[..selection.min()].char_indices().rev().nth(*before - 1).map_or(0, |(i, _)| i)
            };
            ime_delete(buffer, selection.max(), after_end);
            ime_delete(buffer, before_start, selection.min());
        }
        EditCommand::FinishComposingText => buffer.commit_composition(),
        EditCommand::SetSelection { start, end } => {
            let length = buffer.length();
            buffer.set_selection(TextRange::new((*start).min(length), (*end).min(length)));
        }
        EditCommand::Backspace => {
            if let Some(c) = buffer.composition() {
                ime_delete(buffer, c.min(), c.max());
                return;
            }
            let selection = buffer.selection();
            if !selection.collapsed() {
                ime_delete(buffer, selection.min(), selection.max());
                return;
            }
            if selection.start == 0 {
                return;
            }
            let previous = find_preceding_break(buffer.as_char_sequence(), selection.start).unwrap_or(0);
            ime_delete(buffer, previous, selection.start);
        }
        EditCommand::MoveCursor { amount } => {
            let mut cursor = buffer.selection().min();
            let text = buffer.as_char_sequence();
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
            buffer.set_selection(TextRange::at(cursor));
        }
        EditCommand::DeleteAll => {
            let length = buffer.length();
            ime_replace(buffer, 0, length, "");
        }
    }
}

/// The editor's state an input method reads: the field's untransformed value.
struct FieldEditorState(TransformedTextFieldState);

impl TextEditorState for FieldEditorState {
    fn text(&self) -> String {
        self.0.untransformed_text().text().to_string()
    }
    fn selection(&self) -> TextRange {
        self.0.untransformed_text().selection()
    }
    fn composition(&self) -> Option<TextRange> {
        self.0.untransformed_text().composition()
    }
    fn length(&self) -> usize {
        self.0.untransformed_text().len()
    }
}

/// An input method's edits of a buffer (Compose desktop's `TextEditingScope(buffer)`).
struct BufferEditingScope<'a>(&'a mut TextFieldBuffer);

impl TextEditingScope for BufferEditingScope<'_> {
    fn delete_surrounding_text_in_code_points(&mut self, before: usize, after: usize) {
        apply_ime_edit_command(self.0, &EditCommand::DeleteSurroundingTextInCodePoints { before, after });
    }
    fn commit_text(&mut self, text: &str, new_cursor_position: i32) {
        apply_ime_edit_command(self.0, &EditCommand::CommitText { text: text.to_string(), new_cursor_position });
    }
    fn set_composing_text(&mut self, text: &str, new_cursor_position: i32) {
        apply_ime_edit_command(self.0, &EditCommand::SetComposingText { text: text.to_string(), new_cursor_position });
    }
    fn finish_composing_text(&mut self) {
        self.0.commit_composition();
    }
}

/// Starts the platform's input method for a field until cancelled (Compose's desktop
/// `platformSpecificTextInputSession`): its request reads `state`'s untransformed value, applies the input method's
/// commands to it as the user, runs `on_ime_action` for the action key, and says where the caret, the field, its
/// visible text and its text's top left are from `layout_state`. Under a content receiver
/// (`receive_content_configuration`) it takes any content the input method commits besides text, given to the
/// receivers (Compose's Android session: `contentMimeTypes` all, `commitContent` to `onCommitContent`).
pub fn platform_specific_text_input_session(
    session: &PlatformTextInputSessionScope,
    state: TransformedTextFieldState,
    layout_state: Rc<TextLayoutState>,
    ime_options: ImeOptions,
    receive_content_configuration: Option<Rc<dyn ReceiveContentConfiguration>>,
    on_ime_action: Option<Rc<dyn Fn(ImeAction)>>,
) -> SessionFuture {
    let value_state = state.clone();
    let edit_state = state.clone();
    let text_state = state.clone();
    let (l1, l2, l3, l4, l5) =
        (layout_state.clone(), layout_state.clone(), layout_state.clone(), layout_state.clone(), layout_state);
    let rect_state = state.clone();
    let request = PlatformTextInputMethodRequest {
        value: Rc::new(move || to_text_field_value(&value_state.untransformed_text())),
        state: Rc::new(FieldEditorState(state)),
        ime_options,
        on_edit_command: Rc::new(move |commands: &[EditCommand]| {
            edit_state.edit_untransformed_text_as_user(false, |buffer| {
                for command in commands {
                    apply_ime_edit_command(buffer, command);
                }
            });
        }),
        on_ime_action,
        text_layout_result: Rc::new(move || l1.layout_result()),
        focused_rect_in_root: Rc::new(move || {
            let layout: TextLayoutResult = l2.layout_result()?;
            let coordinates = l2.text_layout_node_coordinates()?;
            let caret = layout.get_cursor_rect(rect_state.visual_text().selection().max());
            Some(caret.translate(coordinates.local_to_root(Offset::Zero)))
        }),
        text_field_rect_in_root: Rc::new(move || l3.decorator_node_coordinates().map(|c| c.bounds_in_root())),
        text_clipping_rect_in_root: Rc::new(move || l4.core_node_coordinates().map(|c| c.bounds_in_root())),
        unclipped_text_offset_in_root: Rc::new(move || l5.text_layout_node_coordinates().map(|c| c.position_in_root())),
        edit_text: Rc::new(move |block: &mut dyn FnMut(&mut dyn TextEditingScope)| {
            text_state.edit_untransformed_text_as_user(false, |buffer| block(&mut BufferEditingScope(buffer)));
        }),
        content_mime_types: match receive_content_configuration {
            Some(_) => vec!["*/*".to_string()],
            None => Vec::new(),
        },
        on_commit_content: receive_content_configuration.map(|configuration| {
            Rc::new(move |entry: ClipEntry, link_uri: Option<String>| {
                let metadata = entry.clip_metadata().clone();
                let platform = PlatformTransferableContent { link_uri };
                let content =
                    TransferableContent::new(entry, metadata, TransferableContentSource::Keyboard, Some(platform));
                configuration.on_commit_content(content)
            }) as Rc<dyn Fn(ClipEntry, Option<String>) -> bool>
        }),
    };
    session.start_input_method(request)
}

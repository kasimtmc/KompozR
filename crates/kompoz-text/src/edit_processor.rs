//! Applies an input method's edit commands to an editor's value (Compose's `androidx.compose.ui.text.input.
//! EditProcessor`): the value-based text field keeps one per field; the state-based one makes one per batch on the
//! desktop (Compose's skiko `platformSpecificTextInputSession`).

use crate::edit_command::EditCommand;
use crate::editing_buffer::EditingBuffer;
use crate::input::TextFieldValue;
use crate::range::TextRange;

/// An editor's value as edit commands change it (Compose's EditProcessor).
#[derive(Clone, Debug)]
pub struct EditProcessor {
    buffer_state: TextFieldValue,
    buffer: EditingBuffer,
}

impl Default for EditProcessor {
    fn default() -> Self {
        EditProcessor::new()
    }
}

impl EditProcessor {
    /// One holding empty text.
    pub fn new() -> EditProcessor {
        let buffer_state = TextFieldValue::from_text("", TextRange::Zero, None);
        let buffer = EditingBuffer::new(buffer_state.annotated_string(), buffer_state.selection());
        EditProcessor { buffer_state, buffer }
    }

    /// Takes `value` as the editor's value now (Compose's `reset`): new text starts the buffer afresh (finishing
    /// any composition), a new selection moves it, the composition follows the value's. Returns the value before
    /// and the one taken (the composition dropped where the buffer finished it): what Compose hands the input
    /// session's `updateState(oldValue, newValue)` — the caller tells its session.
    pub fn reset(&mut self, value: &TextFieldValue) -> (TextFieldValue, TextFieldValue) {
        let mut text_changed = false;
        let mut selection_changed = false;
        let composition_changed = value.composition() != self.buffer.composition();
        if self.buffer_state.text() != value.text() {
            self.buffer = EditingBuffer::new(value.annotated_string(), value.selection());
            text_changed = true;
        } else if self.buffer_state.selection() != value.selection() {
            self.buffer.set_selection(value.selection().min(), value.selection().max());
            selection_changed = true;
        }
        match value.composition() {
            None => self.buffer.commit_composition(),
            Some(c) if !c.collapsed() => self.buffer.set_composition(c.min(), c.max()),
            Some(_) => {}
        }
        let new_value = if text_changed || (!selection_changed && composition_changed) {
            self.buffer.commit_composition();
            TextFieldValue::new(value.annotated_string().clone(), value.selection(), None)
        } else {
            value.clone()
        };
        let old_value = std::mem::replace(&mut self.buffer_state, new_value.clone());
        (old_value, new_value)
    }

    /// Applies `edit_commands` in order and gives the value they leave (Compose's `apply`): a selection that was
    /// reversed stays reversed.
    pub fn apply(&mut self, edit_commands: &[EditCommand]) -> TextFieldValue {
        for command in edit_commands {
            command.apply_to(&mut self.buffer);
        }
        let selection = self.buffer.selection();
        let selection = if self.buffer_state.selection().reversed() {
            TextRange::new(selection.max(), selection.min())
        } else {
            selection
        };
        let new_state = TextFieldValue::new(self.buffer.to_annotated_string(), selection, self.buffer.composition());
        self.buffer_state = new_state.clone();
        new_state
    }

    /// The value as it is now (Compose's `toTextFieldValue`).
    pub fn to_text_field_value(&self) -> TextFieldValue {
        self.buffer_state.clone()
    }

    /// The buffer the commands apply to.
    pub fn buffer(&self) -> &EditingBuffer {
        &self.buffer
    }
}

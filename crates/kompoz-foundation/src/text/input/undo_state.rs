//! A [`TextFieldState`]'s undo history as the app sees it (Compose's UndoState).

use super::TextFieldState;

/// Undo and redo of a field's edits (Compose's UndoState): whether there is something to undo or redo (read in a
/// snapshot: its readers hear of changes), undo, redo, forget.
#[derive(Clone, PartialEq, Debug)]
pub struct UndoState {
    state: TextFieldState,
}

impl UndoState {
    pub(crate) fn new(state: TextFieldState) -> UndoState {
        UndoState { state }
    }

    /// Whether there is an edit to undo.
    pub fn can_undo(&self) -> bool {
        self.state.text_undo_manager().can_undo()
    }

    /// Whether there is an undone edit to redo.
    pub fn can_redo(&self) -> bool {
        self.state.text_undo_manager().can_redo()
    }

    /// Undoes the last edit (nothing if there is none).
    pub fn undo(&self) {
        self.state.text_undo_manager().undo(&self.state);
    }

    /// Redoes the last undone edit (nothing if there is none).
    pub fn redo(&self) {
        self.state.text_undo_manager().redo(&self.state);
    }

    /// Forgets the history.
    pub fn clear_history(&self) {
        self.state.text_undo_manager().clear_history();
    }
}

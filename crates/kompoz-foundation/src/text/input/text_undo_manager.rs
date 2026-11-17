//! A field's undo history (Compose's TextUndoManager): edits recorded as they come, the latest held aside while
//! the next ones may merge into it ([`TextUndoOperation::merge`]), then flushed to the undo stack.

use super::TextFieldState;
use super::change_tracker::ChangeList;
use super::internal::undo::{TextUndoOperation, UndoManager, redo_on, undo_on};
use super::text_field_char_sequence::TextFieldCharSequence;
use kompoz_runtime::{MutableState, SavedValue, mutable_state_of, without_read_observation};
use std::cell::RefCell;

/// How many steps a field's history holds (Compose's `TEXT_UNDO_CAPACITY`).
const TEXT_UNDO_CAPACITY: usize = 100;

/// A field's undo history (Compose's TextUndoManager).
pub struct TextUndoManager {
    /// The latest edit, which the next may still merge into (Compose's `stagingUndo`, a snapshot state).
    staging_undo: MutableState<Option<TextUndoOperation>>,
    undo_manager: RefCell<UndoManager<TextUndoOperation>>,
}

impl Default for TextUndoManager {
    fn default() -> Self {
        TextUndoManager::new(None, UndoManager::new(Vec::new(), Vec::new(), TEXT_UNDO_CAPACITY))
    }
}

impl TextUndoManager {
    pub fn new(initial_staging_undo: Option<TextUndoOperation>, undo_manager: UndoManager<TextUndoOperation>) -> Self {
        TextUndoManager {
            staging_undo: mutable_state_of(initial_staging_undo),
            undo_manager: RefCell::new(undo_manager),
        }
    }

    /// Whether there is something to undo (read in a snapshot: its readers hear of changes).
    pub fn can_undo(&self) -> bool {
        self.undo_manager.borrow().can_undo() || self.staging_undo.with(|s| s.is_some())
    }

    /// Whether there is something to redo.
    pub fn can_redo(&self) -> bool {
        self.undo_manager.borrow().can_redo() && self.staging_undo.with(|s| s.is_none())
    }

    /// Undoes the last step on `state`, if there is one.
    pub fn undo(&self, state: &TextFieldState) {
        if !self.can_undo() {
            return;
        }
        self.flush();
        let op = self.undo_manager.borrow_mut().undo();
        undo_on(state, &op);
    }

    /// Redoes the last step undone on `state`, if there is one.
    pub fn redo(&self, state: &TextFieldState) {
        if !self.can_redo() {
            return;
        }
        let op = self.undo_manager.borrow_mut().redo();
        redo_on(state, &op);
    }

    /// Records `op`: merged into the edit held aside if it can be, else that one flushed and this held aside.
    pub fn record(&self, op: TextUndoOperation) {
        let staging = without_read_observation(|| self.staging_undo.peek());
        let Some(staging) = staging else {
            self.staging_undo.set(Some(op));
            return;
        };
        if let Some(merged) = staging.merge(&op) {
            self.staging_undo.set(Some(merged));
            return;
        }
        self.flush();
        self.staging_undo.set(Some(op));
    }

    /// Forgets the history.
    pub fn clear_history(&self) {
        self.staging_undo.set(None);
        self.undo_manager.borrow_mut().clear_history();
    }

    fn flush(&self) {
        let staging = without_read_observation(|| self.staging_undo.peek());
        if let Some(op) = staging {
            self.undo_manager.borrow_mut().record(op);
        }
        self.staging_undo.set(None);
    }

    /// Records the changes from `pre` to `post` (Compose's `recordChanges`): several changes as one step replacing
    /// all the text (never merged); one as the step of its range; none, nothing.
    pub fn record_changes(
        &self,
        pre: &TextFieldCharSequence,
        post: &TextFieldCharSequence,
        changes: &impl ChangeList,
        allow_merge: bool,
    ) {
        if changes.change_count() > 1 {
            self.record(TextUndoOperation::new(0, pre.text(), post.text(), pre.selection(), post.selection(), false));
        } else if changes.change_count() == 1 {
            let pre_range = changes.get_original_range(0);
            let post_range = changes.get_range(0);
            if !pre_range.collapsed() || !post_range.collapsed() {
                self.record(TextUndoOperation::new(
                    pre_range.min(),
                    pre_range.substring(pre.text()),
                    post_range.substring(post.text()),
                    pre.selection(),
                    post.selection(),
                    allow_merge,
                ));
            }
        }
    }

    /// Its saved form (Compose's `TextUndoManager.Saver`): the edit held aside and the stacks.
    pub fn save(&self) -> SavedValue {
        let staging = self.staging_undo.peek();
        SavedValue::list([
            staging.map_or(SavedValue::Null, |op| op.save()),
            self.undo_manager.borrow().save(&|op: &TextUndoOperation| op.save()),
        ])
    }

    /// Back from [`save`](TextUndoManager::save)'s form.
    pub fn restore(saved: &SavedValue) -> Option<TextUndoManager> {
        let list = saved.as_list()?;
        let staging = list.first()?;
        let staging = if staging.is_null() { None } else { Some(TextUndoOperation::restore(staging)?) };
        let undo_manager = UndoManager::restore(list.get(1)?, &TextUndoOperation::restore)?;
        Some(TextUndoManager::new(staging, undo_manager))
    }
}

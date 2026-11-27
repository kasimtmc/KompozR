//! Undo and redo of a field's edits (Compose's `text.input.internal.undo`): the undo and redo stacks
//! ([`UndoManager`]), an undoable edit ([`TextUndoOperation`]) and how edits are told apart.

use crate::text::input::TextFieldState;
use kompoz_runtime::{SavedValue, Saver, SnapshotStateList, mutable_confined_state_list_of};
use kompoz_text::TextRange;

/// Edits made within this many milliseconds of each other may be one undo step (Compose's
/// `SNAPSHOTS_INTERVAL_MILLIS`, the legacy UndoManager's too).
pub const SNAPSHOTS_INTERVAL_MILLIS: i64 = 5000;

/// The wall clock in milliseconds (Compose's `timeNowMillis`, `System.currentTimeMillis()`).
pub fn time_now_millis() -> i64 {
    kompoz_runtime::clock::current_time_millis()
}

/// The undo and redo stacks (Compose's internal UndoManager): what is recorded clears the redo stack; the oldest
/// steps go beyond `capacity`. Snapshot state lists, so their readers hear of changes.
#[derive(Clone)]
pub struct UndoManager<T: Clone + 'static> {
    undo_stack: SnapshotStateList<T>,
    redo_stack: SnapshotStateList<T>,
    capacity: usize,
}

impl<T: Clone + 'static> UndoManager<T> {
    /// With these stacks and room for `capacity` steps. Panics if they hold more.
    pub fn new(initial_undo_stack: Vec<T>, initial_redo_stack: Vec<T>, capacity: usize) -> UndoManager<T> {
        assert!(
            initial_redo_stack.len() + initial_undo_stack.len() <= capacity,
            "Initial list of undo and redo operations have a size greater than the given capacity."
        );
        UndoManager {
            undo_stack: mutable_confined_state_list_of(initial_undo_stack),
            redo_stack: mutable_confined_state_list_of(initial_redo_stack),
            capacity,
        }
    }

    /// Whether there is a step to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether there is a step to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// How many steps both stacks hold.
    pub fn size(&self) -> usize {
        self.undo_stack.len() + self.redo_stack.len()
    }

    /// Records `undoable_action` as the step to undo next; the redo stack cleared.
    pub fn record(&mut self, undoable_action: T) {
        self.redo_stack.clear();
        while self.size() > self.capacity.saturating_sub(1) && !self.undo_stack.is_empty() {
            self.undo_stack.remove_at(0);
        }
        self.undo_stack.add(undoable_action);
    }

    /// The step to undo, moved to the redo stack. Panics if there is none.
    pub fn undo(&mut self) -> T {
        assert!(
            self.can_undo(),
            "It's an error to call undo while there is nothing to undo. Please first check `canUndo` value before \
             calling the `undo` function."
        );
        let top = self.undo_stack.remove_at(self.undo_stack.len() - 1);
        self.redo_stack.add(top.clone());
        top
    }

    /// The step to redo, moved back to the undo stack. Panics if there is none.
    pub fn redo(&mut self) -> T {
        assert!(
            self.can_redo(),
            "It's an error to call redo while there is nothing to redo. Please first check `canRedo` value before \
             calling the `redo` function."
        );
        let top = self.redo_stack.remove_at(self.redo_stack.len() - 1);
        self.undo_stack.add(top.clone());
        top
    }

    /// Forgets every step.
    pub fn clear_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Its saved form: capacity, stack sizes, then the steps (Compose's `createSaver`).
    pub fn save(&self, item: &dyn Fn(&T) -> SavedValue) -> SavedValue {
        let undo = self.undo_stack.to_vec();
        let redo = self.redo_stack.to_vec();
        let mut list = vec![
            SavedValue::from(self.capacity as i64),
            SavedValue::from(undo.len() as i64),
            SavedValue::from(redo.len() as i64),
        ];
        list.extend(undo.iter().map(item));
        list.extend(redo.iter().map(item));
        SavedValue::list(list)
    }

    /// Back from [`save`](UndoManager::save)'s form.
    pub fn restore(saved: &SavedValue, item: &dyn Fn(&SavedValue) -> Option<T>) -> Option<UndoManager<T>> {
        let list = saved.as_list()?;
        let capacity = list.first()?.as_int()? as usize;
        let undo_size = list.get(1)?.as_int()? as usize;
        let redo_size = list.get(2)?.as_int()? as usize;
        let undo = list.get(3..3 + undo_size)?.iter().map(item).collect::<Option<Vec<T>>>()?;
        let redo = list.get(3 + undo_size..3 + undo_size + redo_size)?.iter().map(item).collect::<Option<Vec<T>>>()?;
        Some(UndoManager::new(undo, redo, capacity))
    }
}

/// What an edit did to the text (Compose's TextEditType).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextEditType {
    Insert,
    Delete,
    Replace,
}

/// How a deletion was made (Compose's TextDeleteType): backwards from the caret (Backspace), forwards (Delete),
/// a selection deleted, or not by the user.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextDeleteType {
    Start,
    End,
    Inner,
    NotByUser,
}

/// How an edit goes into the undo history (Compose's TextFieldEditUndoBehavior).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextFieldEditUndoBehavior {
    /// Merged with the step before if it can be.
    MergeIfPossible,
    /// The history cleared.
    ClearHistory,
    /// A step of its own.
    NeverMerge,
}

/// An undoable edit (Compose's TextUndoOperation): `pre_text` at `index` became `post_text`, the selection going
/// from `pre_selection` to `post_selection`, at `time_in_millis`; `can_merge` whether later edits may join it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextUndoOperation {
    pub index: usize,
    pub pre_text: String,
    pub post_text: String,
    pub pre_selection: TextRange,
    pub post_selection: TextRange,
    pub time_in_millis: i64,
    pub can_merge: bool,
    text_edit_type: TextEditType,
}

impl TextUndoOperation {
    /// An edit made now ([`time_now_millis`]). Panics if both texts are empty, as Compose's.
    pub fn new(
        index: usize,
        pre_text: impl Into<String>,
        post_text: impl Into<String>,
        pre_selection: TextRange,
        post_selection: TextRange,
        can_merge: bool,
    ) -> TextUndoOperation {
        TextUndoOperation::at_time(
            index,
            pre_text,
            post_text,
            pre_selection,
            post_selection,
            time_now_millis(),
            can_merge,
        )
    }

    /// An edit made at `time_in_millis`.
    pub fn at_time(
        index: usize,
        pre_text: impl Into<String>,
        post_text: impl Into<String>,
        pre_selection: TextRange,
        post_selection: TextRange,
        time_in_millis: i64,
        can_merge: bool,
    ) -> TextUndoOperation {
        let (pre_text, post_text) = (pre_text.into(), post_text.into());
        let text_edit_type = match (pre_text.is_empty(), post_text.is_empty()) {
            (true, true) => panic!("Either pre or post text must not be empty"),
            (true, false) => TextEditType::Insert,
            (false, true) => TextEditType::Delete,
            (false, false) => TextEditType::Replace,
        };
        TextUndoOperation {
            index,
            pre_text,
            post_text,
            pre_selection,
            post_selection,
            time_in_millis,
            can_merge,
            text_edit_type,
        }
    }

    /// What it did to the text.
    pub fn text_edit_type(&self) -> TextEditType {
        self.text_edit_type
    }

    /// How its deletion was made (Compose's `deletionType`).
    pub fn deletion_type(&self) -> TextDeleteType {
        if self.text_edit_type != TextEditType::Delete || !self.post_selection.collapsed() {
            return TextDeleteType::NotByUser;
        }
        if self.pre_selection.collapsed() {
            if self.pre_selection.start > self.post_selection.start {
                TextDeleteType::Start
            } else {
                TextDeleteType::End
            }
        } else if self.pre_selection.start == self.post_selection.start && self.pre_selection.start == self.index {
            TextDeleteType::Inner
        } else {
            TextDeleteType::NotByUser
        }
    }

    fn is_new_line_insert(&self) -> bool {
        self.post_text == "\n" || self.post_text == "\r\n"
    }

    /// It and `next` as one step, if they can be (Compose's `merge`): both mergeable, `next` within
    /// [`SNAPSHOTS_INTERVAL_MILLIS`] after it, neither a line break typed, of the same type — insertions one after
    /// the other, or deletions going the same way from the same place.
    pub fn merge(&self, next: &TextUndoOperation) -> Option<TextUndoOperation> {
        if !self.can_merge || !next.can_merge {
            return None;
        }
        if next.time_in_millis < self.time_in_millis
            || next.time_in_millis - self.time_in_millis >= SNAPSHOTS_INTERVAL_MILLIS
        {
            return None;
        }
        if self.is_new_line_insert() || next.is_new_line_insert() {
            return None;
        }
        if self.text_edit_type != next.text_edit_type {
            return None;
        }
        if self.text_edit_type == TextEditType::Insert && self.index + self.post_text.len() == next.index {
            return Some(TextUndoOperation::at_time(
                self.index,
                "",
                format!("{}{}", self.post_text, next.post_text),
                self.pre_selection,
                next.post_selection,
                self.time_in_millis,
                true,
            ));
        } else if self.text_edit_type == TextEditType::Delete {
            let deletion = self.deletion_type();
            if deletion == next.deletion_type() && matches!(deletion, TextDeleteType::Start | TextDeleteType::End) {
                if self.index == next.index + next.pre_text.len() {
                    return Some(TextUndoOperation::at_time(
                        next.index,
                        format!("{}{}", next.pre_text, self.pre_text),
                        "",
                        self.pre_selection,
                        next.post_selection,
                        self.time_in_millis,
                        true,
                    ));
                } else if self.index == next.index {
                    return Some(TextUndoOperation::at_time(
                        self.index,
                        format!("{}{}", self.pre_text, next.pre_text),
                        "",
                        self.pre_selection,
                        next.post_selection,
                        self.time_in_millis,
                        true,
                    ));
                }
            }
        }
        None
    }

    /// Its saver (Compose's `TextUndoOperation.Saver`): what it saves comes back mergeable.
    pub fn saver() -> Saver<TextUndoOperation> {
        Saver::new(|op: &TextUndoOperation| op.save(), |saved: &SavedValue| TextUndoOperation::restore(saved))
    }

    pub(crate) fn save(&self) -> SavedValue {
        SavedValue::list([
            SavedValue::from(self.index as i64),
            SavedValue::from(self.pre_text.clone()),
            SavedValue::from(self.post_text.clone()),
            SavedValue::from(self.pre_selection.start as i64),
            SavedValue::from(self.pre_selection.end as i64),
            SavedValue::from(self.post_selection.start as i64),
            SavedValue::from(self.post_selection.end as i64),
            SavedValue::from(self.time_in_millis),
        ])
    }

    pub(crate) fn restore(saved: &SavedValue) -> Option<TextUndoOperation> {
        let l = saved.as_list()?;
        let int = |i: usize| l.get(i).and_then(SavedValue::as_int);
        let string = |i: usize| l.get(i).and_then(SavedValue::as_str).map(str::to_string);
        Some(TextUndoOperation::at_time(
            int(0)? as usize,
            string(1)?,
            string(2)?,
            TextRange::new(int(3)? as usize, int(4)? as usize),
            TextRange::new(int(5)? as usize, int(6)? as usize),
            int(7)?,
            true,
        ))
    }
}

/// Undoes `op` on `state`: its post text back to its pre text, its pre selection (Compose's `TextFieldState.undo`,
/// an edit with no side effects: no input transformation, no undo record).
pub(crate) fn undo_on(state: &TextFieldState, op: &TextUndoOperation) {
    state.edit_with_no_side_effects(|b| {
        b.replace(op.index, op.index + op.post_text.len(), &op.pre_text);
        b.set_selection_coerced(op.pre_selection.start, op.pre_selection.end);
    });
}

/// Redoes `op` on `state` (Compose's `TextFieldState.redo`).
pub(crate) fn redo_on(state: &TextFieldState, op: &TextUndoOperation) {
    state.edit_with_no_side_effects(|b| {
        b.replace(op.index, op.index + op.pre_text.len(), &op.post_text);
        b.set_selection_coerced(op.post_selection.start, op.post_selection.end);
    });
}

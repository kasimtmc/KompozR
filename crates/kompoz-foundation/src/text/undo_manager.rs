//! The value-based text field's undo history (Compose's legacy `androidx.compose.foundation.text.UndoManager`):
//! snapshots of the field's value, taken when a key command ran (something pasted, a word deleted, …), before an
//! undo, and when the last one is older than [`SNAPSHOTS_INTERVAL_MILLIS`] — so a run of typing within those five
//! seconds is one step. A value of the same text as the last snapshot only moves that snapshot's selection.

use crate::text::input::internal::{SNAPSHOTS_INTERVAL_MILLIS, time_now_millis};
use kompoz_text::input::TextFieldValue;
use std::cell::RefCell;
use std::rc::Rc;

/// What is stored of the history at most, in characters (Compose's default `maxStoredCharacters`).
pub const DefaultMaxStoredCharacters: usize = 100_000;

/// An entry of a stack (Compose's Entry: the stacks are linked lists, newest first).
struct Entry {
    next: Option<Box<Entry>>,
    value: TextFieldValue,
}

struct Stacks {
    undo_stack: Option<Box<Entry>>,
    redo_stack: Option<Box<Entry>>,
    stored_characters: usize,
    last_snapshot: Option<i64>,
    force_next_snapshot: bool,
}

/// A value-based field's undo history (Compose's legacy UndoManager): its snapshots of the field's value hold at
/// most `max_stored_characters` characters together (counted in characters: Compose's UTF-16 units), the oldest
/// dropped beyond. Cheap to clone: clones are the same history.
#[derive(Clone)]
pub struct UndoManager {
    max_stored_characters: usize,
    stacks: Rc<RefCell<Stacks>>,
}

impl Default for UndoManager {
    fn default() -> Self {
        UndoManager::new(DefaultMaxStoredCharacters)
    }
}

impl PartialEq for UndoManager {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.stacks, &other.stacks)
    }
}

fn characters(value: &TextFieldValue) -> usize {
    value.text().chars().count()
}

impl UndoManager {
    /// An empty history keeping at most `max_stored_characters` characters.
    pub fn new(max_stored_characters: usize) -> UndoManager {
        UndoManager {
            max_stored_characters,
            stacks: Rc::new(RefCell::new(Stacks {
                undo_stack: None,
                redo_stack: None,
                stored_characters: 0,
                last_snapshot: None,
                force_next_snapshot: false,
            })),
        }
    }

    /// The characters it may store (Compose's `maxStoredCharacters`).
    pub fn max_stored_characters(&self) -> usize {
        self.max_stored_characters
    }

    /// Takes a snapshot of `value` if one was forced or the last one is older than [`SNAPSHOTS_INTERVAL_MILLIS`]
    /// (Compose's `snapshotIfNeeded`, called at each composition of the field).
    pub fn snapshot_if_needed(&self, value: &TextFieldValue) {
        self.snapshot_if_needed_at(value, time_now_millis());
    }

    /// [`snapshot_if_needed`](Self::snapshot_if_needed) at `now` (ms).
    pub fn snapshot_if_needed_at(&self, value: &TextFieldValue, now: i64) {
        let take = {
            let mut s = self.stacks.borrow_mut();
            let take = s.force_next_snapshot || now > s.last_snapshot.unwrap_or(0) + SNAPSHOTS_INTERVAL_MILLIS;
            if take {
                s.last_snapshot = Some(now);
            }
            take
        };
        if take {
            self.make_snapshot(value);
        }
    }

    /// The next [`snapshot_if_needed`](Self::snapshot_if_needed) takes one (Compose's `forceNextSnapshot`).
    pub fn force_next_snapshot(&self) {
        self.stacks.borrow_mut().force_next_snapshot = true;
    }

    /// Takes a snapshot of `value` unless it is the last one (Compose's `makeSnapshot`): the same text only moves
    /// the last one's selection and composition; a new one clears the redo stack.
    pub fn make_snapshot(&self, value: &TextFieldValue) {
        let mut s = self.stacks.borrow_mut();
        s.force_next_snapshot = false;
        if let Some(top) = s.undo_stack.as_mut() {
            if top.value == *value {
                return;
            }
            if top.value.text() == value.text() {
                top.value = value.clone();
                return;
            }
        }
        let next = s.undo_stack.take();
        s.undo_stack = Some(Box::new(Entry { next, value: value.clone() }));
        s.redo_stack = None;
        s.stored_characters += characters(value);
        if s.stored_characters > self.max_stored_characters {
            Self::remove_last_undo(&mut s);
        }
    }

    /// Drops the oldest snapshot but the newest (Compose's `removeLastUndo`).
    fn remove_last_undo(s: &mut Stacks) {
        let Some(mut entry) = s.undo_stack.as_mut() else { return };
        if entry.next.is_none() {
            return;
        }
        while entry.next.as_ref().is_some_and(|n| n.next.is_some()) {
            entry = entry.next.as_mut().expect("a next entry");
        }
        entry.next = None;
    }

    /// The value before the last snapshot, the last one going to the redo stack; none if there is none before it
    /// (Compose's `undo`).
    pub fn undo(&self) -> Option<TextFieldValue> {
        let mut s = self.stacks.borrow_mut();
        let mut undo_entry = s.undo_stack.take()?;
        let Some(next_entry) = undo_entry.next.take() else {
            s.undo_stack = Some(undo_entry);
            return None;
        };
        let value = next_entry.value.clone();
        s.undo_stack = Some(next_entry);
        s.stored_characters = s.stored_characters.saturating_sub(characters(&undo_entry.value));
        let redo_next = s.redo_stack.take();
        s.redo_stack = Some(Box::new(Entry { next: redo_next, value: undo_entry.value }));
        Some(value)
    }

    /// The value last undone, back on the undo stack; none if nothing was undone (Compose's `redo`).
    pub fn redo(&self) -> Option<TextFieldValue> {
        let mut s = self.stacks.borrow_mut();
        let mut redo_entry = s.redo_stack.take()?;
        s.redo_stack = redo_entry.next.take();
        let value = redo_entry.value.clone();
        s.stored_characters += characters(&value);
        let undo_next = s.undo_stack.take();
        s.undo_stack = Some(Box::new(Entry { next: undo_next, value: redo_entry.value }));
        Some(value)
    }

    /// The snapshots on the undo stack, newest first (for tests and inspection).
    pub fn undo_values(&self) -> Vec<TextFieldValue> {
        let s = self.stacks.borrow();
        let mut values = Vec::new();
        let mut entry = s.undo_stack.as_ref();
        while let Some(e) = entry {
            values.push(e.value.clone());
            entry = e.next.as_ref();
        }
        values
    }

    /// The characters its snapshots hold now (Compose's `storedCharacters`).
    pub fn stored_characters(&self) -> usize {
        self.stacks.borrow().stored_characters
    }
}

//! The state of a state-based text field (Compose's TextFieldState): its text, selection, composition and styles,
//! edited by the app through [`edit`](TextFieldState::edit) and by the user through the field (an input method,
//! keys, gestures: the internal `edit_as_user`, through the field's [`InputTransformation`](super::
//! InputTransformation)), with an undo history ([`undo_state`](TextFieldState::undo_state)). Snapshot state: what
//! reads it hears of its changes.

use super::change_tracker::{ChangeList, ChangeTracker};
use super::input_transformation::InputTransformation;
use super::internal::undo::TextFieldEditUndoBehavior;
use super::text_field_buffer::TextFieldBuffer;
use super::text_field_char_sequence::{PlacedAnnotation, TextFieldCharSequence};
use super::text_field_text_styles::TextFieldTextStyles;
use super::text_undo_manager::TextUndoManager;
use super::undo_state::UndoState;
use kompoz_runtime::{
    MutableState, SavedValue, Saver, kompozable, mutable_state_of, remember_saveable_with, without_read_observation,
};
use kompoz_text::annotated_string::{Annotation, Range};
use kompoz_text::{SpanStyle, TextDecoration, TextRange};
use std::cell::RefCell;
use std::rc::Rc;

/// Told of each change of a field's value, for the input method (Compose's `TextFieldState.NotifyImeListener`):
/// the value before, after, and whether the input method should restart (the text changed under a composition).
pub type NotifyImeListener = Rc<dyn Fn(&TextFieldCharSequence, &TextFieldCharSequence, bool)>;

struct Inner {
    text_undo_manager: TextUndoManager,
    /// What the input method's and the user's edits are applied to (Compose's `mainBuffer`).
    main_buffer: RefCell<TextFieldBuffer>,
    /// An `edit` is running (snapshot state: two snapshots may edit at once).
    is_editing: MutableState<bool>,
    value: MutableState<TextFieldCharSequence>,
    /// The last change was the user's (Compose's `userCommit`).
    user_commit: MutableState<bool>,
    suggestion_selected: MutableState<bool>,
    notify_ime_listeners: RefCell<Vec<NotifyImeListener>>,
}

/// A text field's state (Compose's TextFieldState): cheap to clone, equal when the same. Make one in a composable
/// with [`remember_text_field_state`].
#[derive(Clone)]
pub struct TextFieldState(Rc<Inner>);

impl PartialEq for TextFieldState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for TextFieldState {
    /// `TextFieldState(selection=…, text="…")` (not read observed).
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let value = without_read_observation(|| self.0.value.peek());
        write!(f, "TextFieldState(selection={:?}, text=\"{}\")", value.selection(), value.text())
    }
}

impl Default for TextFieldState {
    fn default() -> Self {
        TextFieldState::new("")
    }
}

impl TextFieldState {
    /// Holding `initial_text`, `initial_selection` selected (clamped to it: `TextRange::at(usize::MAX)` is the caret
    /// at its end).
    ///
    /// Kotlin: `TextFieldState(initialText: String = "", initialSelection: TextRange = TextRange(initialText.length))`
    pub fn with_selection(initial_text: &str, initial_selection: TextRange) -> TextFieldState {
        TextFieldState::with_undo_manager(initial_text, initial_selection, TextUndoManager::default())
    }

    /// Holding `initial_text`, the caret at its end.
    pub fn new(initial_text: &str) -> TextFieldState {
        TextFieldState::with_selection(initial_text, TextRange::at(initial_text.len()))
    }

    pub(crate) fn with_undo_manager(
        initial_text: &str,
        initial_selection: TextRange,
        text_undo_manager: TextUndoManager,
    ) -> TextFieldState {
        let initial =
            TextFieldCharSequence::new(initial_text, initial_selection.coerce_in(0, initial_text.len()), None);
        TextFieldState(Rc::new(Inner {
            text_undo_manager,
            main_buffer: RefCell::new(TextFieldBuffer::of(initial.clone())),
            is_editing: mutable_state_of(false),
            value: mutable_state_of(initial),
            user_commit: mutable_state_of(false),
            suggestion_selected: mutable_state_of(false),
            notify_ime_listeners: RefCell::new(Vec::new()),
        }))
    }

    /// Its contents now (read observed).
    pub fn value(&self) -> TextFieldCharSequence {
        self.0.value.get()
    }

    /// The text (Compose's `text`).
    pub fn text(&self) -> String {
        self.0.value.with(|v| v.text().to_string())
    }

    /// The selection, a caret when collapsed (Compose's `selection`).
    pub fn selection(&self) -> TextRange {
        self.0.value.with(|v| v.selection())
    }

    /// What an input method is composing, if anything (Compose's `composition`).
    pub fn composition(&self) -> Option<TextRange> {
        self.0.value.with(|v| v.composition())
    }

    /// Its text's styles (Compose's `textStyles`).
    pub fn text_styles(&self) -> TextFieldTextStyles {
        TextFieldTextStyles(self.0.value.with(|v| v.text_field_text_styles().cloned()))
    }

    /// The last change was the user's (Compose's `userCommit`).
    pub fn user_commit(&self) -> bool {
        self.0.user_commit.get()
    }

    /// A transliteration's suggestion is being chosen (Compose's `suggestionSelected`).
    pub fn suggestion_selected(&self) -> bool {
        self.0.suggestion_selected.get()
    }

    /// Its undo history, to undo, redo and clear (Compose's `undoState`).
    pub fn undo_state(&self) -> UndoState {
        UndoState::new(self.clone())
    }

    pub(crate) fn text_undo_manager(&self) -> &TextUndoManager {
        &self.0.text_undo_manager
    }

    /// Edits it (Compose's `edit`): `block` changes a buffer of its contents; what it leaves is the new value, a
    /// step of the undo history of its own (never merged), the composition finished if anything changed. Not as
    /// the user: the field's input transformation doesn't see it. Panics if called inside another edit.
    pub fn edit(&self, block: impl FnOnce(&mut TextFieldBuffer)) {
        let mut buffer = self.start_edit();
        // (Ends the edit however the block ends.)
        struct Finish<'a>(&'a TextFieldState);
        impl Drop for Finish<'_> {
            fn drop(&mut self) {
                self.0.finish_editing();
            }
        }
        let _finish = Finish(self);
        block(&mut buffer);
        self.commit_edit(buffer);
    }

    /// [`edit`](Self::edit) whose `block` suspends (Kotlin's inline `edit`: its lambda, run in place, suspends with
    /// the coroutine it is called in; the state is being edited meanwhile, so another edit then panics, as in
    /// Compose).
    #[doc(hidden)]
    pub async fn edit_async(&self, block: impl AsyncFnOnce(&mut TextFieldBuffer)) {
        let mut buffer = self.start_edit();
        // (Ends the edit however the block ends, a dropped coroutine's too.)
        struct Finish<'a>(&'a TextFieldState);
        impl Drop for Finish<'_> {
            fn drop(&mut self) {
                self.0.finish_editing();
            }
        }
        let _finish = Finish(self);
        block(&mut buffer).await;
        self.commit_edit(buffer);
    }

    fn start_edit(&self) -> TextFieldBuffer {
        let editing = without_read_observation(|| self.0.is_editing.peek());
        assert!(!editing, "TextFieldState does not support concurrent or nested editing.");
        self.0.is_editing.set(true);
        TextFieldBuffer::of(self.0.value.peek())
    }

    fn commit_edit(&self, new_value: TextFieldBuffer) {
        let text_changed = new_value.changes().change_count() > 0;
        let (selection_changed, style_changed) = {
            let main = self.0.main_buffer.borrow();
            (new_value.selection() != main.selection(), new_value.text_style_buffer != main.text_style_buffer)
        };
        if text_changed {
            let previous = self.0.value.peek();
            self.record_edit_for_undo(
                &previous,
                &new_value.to_text_field_char_sequence(),
                new_value.changes(),
                TextFieldEditUndoBehavior::NeverMerge,
            );
        }
        self.sync_main_buffer_to_temporary_buffer(&new_value, text_changed, selection_changed, style_changed);
    }

    fn finish_editing(&self) {
        self.0.is_editing.set(false);
        self.0.user_commit.set(false);
    }

    /// Edits it as the user (Compose's `editAsUser`): `block` changes the main buffer; `input_transformation`, if
    /// any, then gets the change to accept, change or refuse; the undo history records it as `undo_behavior` says.
    /// `restart_ime_if_content_changes`: the input method is told to restart if the text changed under a
    /// composition.
    pub fn edit_as_user(
        &self,
        input_transformation: Option<&InputTransformation>,
        restart_ime_if_content_changes: bool,
        undo_behavior: TextFieldEditUndoBehavior,
        block: impl FnOnce(&mut TextFieldBuffer),
    ) {
        {
            let mut main = self.0.main_buffer.borrow_mut();
            main.change_tracker_mut().clear_changes();
            block(&mut main);
        }
        self.commit_edit_as_user(input_transformation, restart_ime_if_content_changes, undo_behavior);
        self.0.user_commit.set(true);
        let suggestion = self.0.main_buffer.borrow().suggestion_selected;
        self.0.suggestion_selected.set(suggestion);
    }

    /// Edits it with no side effect (Compose's `editWithNoSideEffects`): no input transformation, no undo record
    /// (what undo and redo do).
    pub(crate) fn edit_with_no_side_effects(&self, block: impl FnOnce(&mut TextFieldBuffer)) {
        let after = {
            let mut main = self.0.main_buffer.borrow_mut();
            main.change_tracker_mut().clear_changes();
            block(&mut main);
            main.to_text_field_char_sequence()
        };
        let old = self.0.value.peek();
        self.update_value_and_notify_listeners(&old, after, true);
    }

    fn main_value(&self, text_field_text_styles_from_before: bool) -> TextFieldCharSequence {
        let main = self.0.main_buffer.borrow();
        let styles = if text_field_text_styles_from_before {
            self.0.value.peek().text_field_text_styles().cloned()
        } else {
            main.get_text_field_text_styles()
        };
        TextFieldCharSequence::with_all(
            main.as_char_sequence(),
            main.selection(),
            main.composition(),
            main.highlight(),
            Some(finalize_composing_annotations(main.composition(), main.composing_annotations())),
            None,
            styles,
        )
    }

    fn commit_edit_as_user(
        &self,
        input_transformation: Option<&InputTransformation>,
        restart_ime_if_content_changes: bool,
        undo_behavior: TextFieldEditUndoBehavior,
    ) {
        let before = self.0.value.peek();
        let (change_count, selection) = {
            let main = self.0.main_buffer.borrow();
            (main.changes().change_count(), main.selection())
        };
        if change_count == 0 && before.selection() == selection {
            let differs = {
                let main = self.0.main_buffer.borrow();
                before.composition() != main.composition()
                    || before.highlight() != main.highlight()
                    || before.composing_annotations() != main.composing_annotations()
            };
            if differs {
                let after = self.main_value(true);
                self.update_value_and_notify_listeners(&before, after, restart_ime_if_content_changes);
            }
            return;
        }
        let content_may_have_changed = change_count != 0;
        let after = self.main_value(false);
        let Some(input_transformation) = input_transformation else {
            self.update_value_and_notify_listeners(
                &before,
                after.clone(),
                content_may_have_changed && restart_ime_if_content_changes,
            );
            let changes = self.0.main_buffer.borrow().changes().clone();
            self.record_edit_for_undo(&before, &after, &changes, undo_behavior);
            return;
        };
        let changes = self.0.main_buffer.borrow().changes().clone();
        let mut buffer = TextFieldBuffer::new(after.clone(), Some(&changes), Some(before.clone()), None);
        input_transformation.transform_input(&mut buffer);
        let text_changed_by_filter = !after.content_equals(buffer.as_char_sequence());
        let selection_changed_by_filter = buffer.selection() != after.selection();
        let style_changed_by_filter =
            buffer.text_style_buffer.as_ref() != after.text_field_text_styles().map(|s| &*s.text_style_buffer);
        if text_changed_by_filter || selection_changed_by_filter || style_changed_by_filter {
            self.sync_main_buffer_to_temporary_buffer(
                &buffer,
                text_changed_by_filter,
                selection_changed_by_filter,
                style_changed_by_filter,
            );
        } else {
            let value = buffer.to_text_field_char_sequence_with(
                buffer.selection(),
                after.composition(),
                buffer.composing_annotations().map(|a| a.to_vec()).filter(|a| !a.is_empty()),
            );
            self.update_value_and_notify_listeners(&before, value, restart_ime_if_content_changes);
        }
        let value = self.0.value.peek();
        self.record_edit_for_undo(&before, &value, buffer.changes(), undo_behavior);
    }

    fn update_value_and_notify_listeners(
        &self,
        old_value: &TextFieldCharSequence,
        new_value: TextFieldCharSequence,
        restart_ime_if_content_changes: bool,
    ) {
        self.0.value.set(new_value.clone());
        let listeners = self.0.notify_ime_listeners.borrow().clone();
        let restart = restart_ime_if_content_changes
            && !old_value.content_equals(new_value.text())
            && old_value.composition().is_some();
        for listener in listeners {
            listener(old_value, &new_value, restart);
        }
        self.0.user_commit.set(false);
    }

    fn record_edit_for_undo(
        &self,
        previous_value: &TextFieldCharSequence,
        post_value: &TextFieldCharSequence,
        changes: &ChangeTracker,
        undo_behavior: TextFieldEditUndoBehavior,
    ) {
        let manager = &self.0.text_undo_manager;
        match undo_behavior {
            TextFieldEditUndoBehavior::ClearHistory => manager.clear_history(),
            TextFieldEditUndoBehavior::MergeIfPossible => {
                manager.record_changes(previous_value, post_value, changes, true)
            }
            TextFieldEditUndoBehavior::NeverMerge => manager.record_changes(previous_value, post_value, changes, false),
        }
    }

    /// Tells `listener` of each change of its value (Compose's `addNotifyImeListener`).
    pub fn add_notify_ime_listener(&self, listener: NotifyImeListener) {
        self.0.notify_ime_listeners.borrow_mut().push(listener);
    }

    /// Stops telling `listener` (Compose's `removeNotifyImeListener`).
    pub fn remove_notify_ime_listener(&self, listener: &NotifyImeListener) {
        self.0.notify_ime_listeners.borrow_mut().retain(|l| !Rc::ptr_eq(l, listener));
    }

    /// The main buffer made what `temporary_buffer` holds (Compose's `syncMainBufferToTemporaryBuffer`): a new
    /// one if the text or the styles changed, else its selection moved if that changed; the composition finished
    /// if anything changed.
    pub(crate) fn sync_main_buffer_to_temporary_buffer(
        &self,
        temporary_buffer: &TextFieldBuffer,
        text_changed: bool,
        selection_changed: bool,
        style_changed: bool,
    ) {
        let old_value = self.0.main_buffer.borrow().to_text_field_char_sequence();
        {
            let mut main = self.0.main_buffer.borrow_mut();
            if text_changed || style_changed {
                *main = TextFieldBuffer::of(TextFieldCharSequence::with_all(
                    temporary_buffer.as_char_sequence(),
                    temporary_buffer.selection(),
                    None,
                    None,
                    None,
                    None,
                    temporary_buffer.get_text_field_text_styles(),
                ));
            } else if selection_changed {
                main.set_selection(temporary_buffer.selection());
            }
            if text_changed || selection_changed || old_value.composition() != temporary_buffer.composition() {
                main.commit_composition();
            }
        }
        let final_value = self.0.main_buffer.borrow().to_text_field_char_sequence();
        self.update_value_and_notify_listeners(&old_value, final_value, true);
    }

    /// Its saver (Compose's `TextFieldState.Saver`): text, selection, undo history.
    pub fn saver() -> Saver<TextFieldState> {
        Saver::new(
            |state: &TextFieldState| {
                let value = state.0.value.peek();
                SavedValue::list([
                    SavedValue::from(value.text()),
                    SavedValue::from(value.selection().start as i64),
                    SavedValue::from(value.selection().end as i64),
                    state.0.text_undo_manager.save(),
                ])
            },
            |saved: &SavedValue| {
                let l = saved.as_list()?;
                let text = l.first()?.as_str()?;
                let start = l.get(1)?.as_int()? as usize;
                let end = l.get(2)?.as_int()? as usize;
                let undo = TextUndoManager::restore(l.get(3)?)?;
                Some(TextFieldState::with_undo_manager(text, TextRange::new(start, end), undo))
            },
        )
    }

    /// The text set to `text`, the caret at its end (Compose's `setTextAndPlaceCursorAtEnd`).
    pub fn set_text_and_place_cursor_at_end(&self, text: &str) {
        self.edit(|b| {
            let len = b.length();
            b.replace(0, len, text);
            b.place_cursor_at_end();
        });
    }

    /// The text set to `text`, all of it selected (Compose's `setTextAndSelectAll`).
    pub fn set_text_and_select_all(&self, text: &str) {
        self.edit(|b| {
            let len = b.length();
            b.replace(0, len, text);
            b.select_all();
        });
    }

    /// The text deleted (Compose's `clearText`).
    pub fn clear_text(&self) {
        self.edit(|b| {
            let len = b.length();
            b.delete(0, len);
            b.place_cursor_at_end();
        });
    }

    /// A buffer of its contents now, to edit apart from it (Compose's `toTextFieldBuffer`).
    pub fn to_text_field_buffer(&self) -> TextFieldBuffer {
        TextFieldBuffer::of(self.0.value.peek())
    }
}

/// The composition's annotations: the input method's, else an underline over the composition (Compose's
/// `finalizeComposingAnnotations`).
fn finalize_composing_annotations(
    composition: Option<TextRange>,
    annotations: Option<&[PlacedAnnotation]>,
) -> Vec<PlacedAnnotation> {
    match (annotations, composition) {
        (Some(a), _) if !a.is_empty() => a.to_vec(),
        (_, Some(c)) if !c.collapsed() => vec![Range::new(
            Annotation::SpanStyle(SpanStyle {
                text_decoration: Some(TextDecoration::Underline),
                ..SpanStyle::default()
            }),
            c.min(),
            c.max(),
        )],
        _ => Vec::new(),
    }
}

/// A [`TextFieldState`] remembered and saved (Compose's `rememberTextFieldState`): `initial_text` with
/// `initial_selection` (the caret at its end by default: the selection is clamped to the text).
///
/// Kotlin: `rememberTextFieldState(initialText: String = "", initialSelection: TextRange = TextRange(initialText.length))`
#[kompozable]
pub fn remember_text_field_state(
    #[default(String::new())] initial_text: String,
    #[default(TextRange::at(usize::MAX))] initial_selection: TextRange,
) -> TextFieldState {
    remember_saveable_with(&TextFieldState::saver(), move || {
        TextFieldState::with_selection(&initial_text, initial_selection)
    })
}

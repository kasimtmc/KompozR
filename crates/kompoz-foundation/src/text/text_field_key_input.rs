//! A value-based field's keys (Compose's TextFieldKeyInput.kt): a typed character (through the dead key combiner)
//! committed, a key down mapped to a [`KeyCommand`] by the platform's key mapping and run — copy, cut and paste
//! through the selection manager, moves and deletions as the text was laid out, a new line or tab (or a single
//! line's action key), undo and redo through the field's history.

use super::core_text_field::LegacyTextFieldState;
use super::selection::text_field_selection_manager::TextFieldSelectionManager;
use super::selection::text_prepared_selection::{TextFieldPreparedSelection, TextPreparedSelectionState};
use super::text_field_delegate::OnValueChange;
use super::undo_manager::UndoManager;
use crate::text::dead_key_combiner::DeadKeyCombiner;
use crate::text::key_event_helpers::{is_typed_event, show_character_palette};
use crate::text::key_mapping::{KeyCommand, KeyMapping, platform_default_key_mapping};
use kompoz_runtime::remember;
use kompoz_text::input::{EditCommand, OffsetMapping, TextFieldValue};
use kompoz_ui::Modifier;
use kompoz_ui::input::{KeyEvent, KeyEventType};
use kompoz_ui::platform::ImeAction;
use std::cell::RefCell;
use std::rc::Rc;

/// A field's key handling for one composition (Compose's TextFieldKeyInput): the value it was composed with,
/// what it may do, its kept state (the column of vertical moves, the dead key) and where its changes go.
pub struct TextFieldKeyInput {
    pub state: Rc<LegacyTextFieldState>,
    pub selection_manager: Rc<TextFieldSelectionManager>,
    pub value: TextFieldValue,
    pub editable: bool,
    pub single_line: bool,
    pub prepared_selection_state: Rc<RefCell<TextPreparedSelectionState>>,
    pub offset_mapping: OffsetMapping,
    pub undo_manager: Option<UndoManager>,
    key_combiner: Rc<RefCell<DeadKeyCombiner>>,
    key_mapping: Rc<dyn KeyMapping>,
    on_value_change: OnValueChange,
    ime_action: ImeAction,
}

impl TextFieldKeyInput {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: Rc<LegacyTextFieldState>,
        selection_manager: Rc<TextFieldSelectionManager>,
        value: TextFieldValue,
        editable: bool,
        single_line: bool,
        prepared_selection_state: Rc<RefCell<TextPreparedSelectionState>>,
        offset_mapping: OffsetMapping,
        undo_manager: Option<UndoManager>,
        key_combiner: Rc<RefCell<DeadKeyCombiner>>,
        key_mapping: Rc<dyn KeyMapping>,
        on_value_change: OnValueChange,
        ime_action: ImeAction,
    ) -> TextFieldKeyInput {
        TextFieldKeyInput {
            state,
            selection_manager,
            value,
            editable,
            single_line,
            prepared_selection_state,
            offset_mapping,
            undo_manager,
            key_combiner,
            key_mapping,
            on_value_change,
            ime_action,
        }
    }

    /// `commands` applied after finishing the composition, the value they leave told (Compose's `List.apply`).
    fn apply_commands(&self, commands: Vec<EditCommand>) {
        let mut all = Vec::with_capacity(commands.len() + 1);
        all.push(EditCommand::FinishComposingText);
        all.extend(commands);
        let new_value = self.state.processor.borrow_mut().apply(&all);
        (self.on_value_change)(new_value);
    }

    /// A typed character's commit (Compose's `typedCommand`): none for an event that isn't a typed one, or a dead
    /// key waiting for the next.
    fn typed_command(&self, event: &KeyEvent) -> Option<EditCommand> {
        if !is_typed_event(event) {
            return None;
        }
        let code_point = self.key_combiner.borrow_mut().consume(event)?;
        let text = char::from_u32(code_point as u32).map(String::from).unwrap_or_default();
        Some(EditCommand::CommitText { text, new_cursor_position: 1 })
    }

    /// Handles `event` (Compose's `process`): whether it took it.
    pub fn process(&self, event: &KeyEvent) -> bool {
        if let Some(command) = self.typed_command(event) {
            return if self.editable {
                self.apply_commands(vec![command]);
                self.prepared_selection_state.borrow_mut().reset_cached_x();
                true
            } else {
                false
            };
        }
        if event.r#type != KeyEventType::KeyDown {
            return false;
        }
        let Some(command) = self.key_mapping.map(event) else { return false };
        if command.edits_text() && !self.editable {
            return false;
        }
        let mut consumed = true;
        let mut deletion: Option<Vec<EditCommand>> = None;
        let mut commit: Option<EditCommand> = None;
        let mut value_change: Option<TextFieldValue> = None;
        self.command_execution_context(|s| match command {
            KeyCommand::COPY => self.selection_manager.copy(false),
            KeyCommand::PASTE => self.selection_manager.paste(),
            KeyCommand::CUT => self.selection_manager.cut(),
            KeyCommand::LEFT_CHAR => {
                s.collapse_left_or(|s| {
                    s.move_cursor_left();
                });
            }
            KeyCommand::RIGHT_CHAR => {
                s.collapse_right_or(|s| {
                    s.move_cursor_right();
                });
            }
            KeyCommand::LEFT_WORD => {
                s.move_cursor_left_by_word();
            }
            KeyCommand::RIGHT_WORD => {
                s.move_cursor_right_by_word();
            }
            KeyCommand::PREV_PARAGRAPH => {
                s.move_cursor_prev_by_paragraph();
            }
            KeyCommand::NEXT_PARAGRAPH => {
                s.move_cursor_next_by_paragraph();
            }
            KeyCommand::UP => {
                s.move_cursor_up_by_line();
            }
            KeyCommand::DOWN => {
                s.move_cursor_down_by_line();
            }
            KeyCommand::PAGE_UP => {
                s.move_cursor_up_by_page();
            }
            KeyCommand::PAGE_DOWN => {
                s.move_cursor_down_by_page();
            }
            KeyCommand::LINE_START => {
                s.move_cursor_to_line_start();
            }
            KeyCommand::LINE_END => {
                s.move_cursor_to_line_end();
            }
            KeyCommand::LINE_LEFT => {
                s.move_cursor_to_line_left_side();
            }
            KeyCommand::LINE_RIGHT => {
                s.move_cursor_to_line_right_side();
            }
            KeyCommand::HOME => {
                s.move_cursor_to_home();
            }
            KeyCommand::END => {
                s.move_cursor_to_end();
            }
            KeyCommand::DELETE_PREV_CHAR => {
                deletion = s.delete_if_selected_or(|s| {
                    let preceding = s.get_preceding_code_point_or_emoji_start_index()?;
                    Some(EditCommand::DeleteSurroundingText { before: s.selection.end - preceding, after: 0 })
                });
            }
            KeyCommand::DELETE_NEXT_CHAR => {
                // (some software keyboards come this way rather than through the input connection)
                deletion = s.delete_if_selected_or(|s| {
                    // No next character: the cursor is at the end, nothing to do.
                    let next = s.get_next_character_index()?;
                    Some(EditCommand::DeleteSurroundingText { before: 0, after: next - s.selection.end })
                });
            }
            KeyCommand::DELETE_PREV_WORD => {
                deletion = s.delete_if_selected_or(|s| {
                    let it = s.get_previous_word_offset()?;
                    Some(EditCommand::DeleteSurroundingText { before: s.selection.end - it, after: 0 })
                });
            }
            KeyCommand::DELETE_NEXT_WORD => {
                deletion = s.delete_if_selected_or(|s| {
                    let it = s.get_next_word_offset()?;
                    Some(EditCommand::DeleteSurroundingText { before: 0, after: it - s.selection.end })
                });
            }
            KeyCommand::DELETE_FROM_LINE_START => {
                deletion = s.delete_if_selected_or(|s| {
                    let it = s.get_line_start_by_offset()?;
                    Some(EditCommand::DeleteSurroundingText { before: s.selection.end - it, after: 0 })
                });
            }
            KeyCommand::DELETE_TO_LINE_END => {
                deletion = s.delete_if_selected_or(|s| {
                    let it = s.get_line_end_by_offset()?;
                    Some(EditCommand::DeleteSurroundingText { before: 0, after: it - s.selection.end })
                });
            }
            KeyCommand::NEW_LINE => {
                if !self.single_line {
                    commit = Some(EditCommand::CommitText { text: "\n".into(), new_cursor_position: 1 });
                } else {
                    consumed = self.state.on_ime_action_performed_with_result(self.ime_action);
                }
            }
            KeyCommand::TAB => {
                if !self.single_line {
                    commit = Some(EditCommand::CommitText { text: "\t".into(), new_cursor_position: 1 });
                } else {
                    // (on to the focus system)
                    consumed = false;
                }
            }
            KeyCommand::SELECT_ALL => {
                s.select_all();
            }
            KeyCommand::SELECT_LEFT_CHAR => {
                s.move_cursor_left().select_movement();
            }
            KeyCommand::SELECT_RIGHT_CHAR => {
                s.move_cursor_right().select_movement();
            }
            KeyCommand::SELECT_LEFT_WORD => {
                s.move_cursor_left_by_word().select_movement();
            }
            KeyCommand::SELECT_RIGHT_WORD => {
                s.move_cursor_right_by_word().select_movement();
            }
            KeyCommand::SELECT_PREV_PARAGRAPH => {
                s.move_cursor_prev_by_paragraph().select_movement();
            }
            KeyCommand::SELECT_NEXT_PARAGRAPH => {
                s.move_cursor_next_by_paragraph().select_movement();
            }
            KeyCommand::SELECT_LINE_START => {
                s.move_cursor_to_line_start().select_movement();
            }
            KeyCommand::SELECT_LINE_END => {
                s.move_cursor_to_line_end().select_movement();
            }
            KeyCommand::SELECT_LINE_LEFT => {
                s.move_cursor_to_line_left_side().select_movement();
            }
            KeyCommand::SELECT_LINE_RIGHT => {
                s.move_cursor_to_line_right_side().select_movement();
            }
            KeyCommand::SELECT_UP => {
                s.move_cursor_up_by_line().select_movement();
            }
            KeyCommand::SELECT_DOWN => {
                s.move_cursor_down_by_line().select_movement();
            }
            KeyCommand::SELECT_PAGE_UP => {
                s.move_cursor_up_by_page().select_movement();
            }
            KeyCommand::SELECT_PAGE_DOWN => {
                s.move_cursor_down_by_page().select_movement();
            }
            KeyCommand::SELECT_HOME => {
                s.move_cursor_to_home().select_movement();
            }
            KeyCommand::SELECT_END => {
                s.move_cursor_to_end().select_movement();
            }
            KeyCommand::DESELECT => {
                s.deselect();
            }
            KeyCommand::UNDO => {
                if let Some(undo_manager) = &self.undo_manager {
                    undo_manager.make_snapshot(&self.value);
                    value_change = undo_manager.undo();
                }
            }
            KeyCommand::REDO => {
                if let Some(undo_manager) = &self.undo_manager {
                    value_change = undo_manager.redo();
                }
            }
            KeyCommand::CHARACTER_PALETTE => show_character_palette(),
            // (handled by the focus modifier)
            KeyCommand::CENTER => {}
        });
        // (the commands applied after the prepared selection's context: Compose applies them inside it, before its
        // own value is compared; the same order of changes told)
        if let Some(commands) = deletion {
            self.apply_commands(commands);
        }
        if let Some(command) = commit {
            self.apply_commands(vec![command]);
        }
        if let Some(value) = value_change {
            (self.on_value_change)(value);
        }
        if let Some(undo_manager) = &self.undo_manager {
            undo_manager.force_next_snapshot();
        }
        consumed
    }

    /// Runs `block` on the selection prepared from the value, telling the value it leaves if it moved the selection
    /// (Compose's `commandExecutionContext`).
    fn command_execution_context(&self, block: impl FnOnce(&mut TextFieldPreparedSelection)) {
        let new_value = {
            let mut prepared_state = self.prepared_selection_state.borrow_mut();
            let mut prepared = TextFieldPreparedSelection::new(
                self.value.clone(),
                self.offset_mapping.clone(),
                self.state.peek_layout_result(),
                &mut prepared_state,
            );
            block(&mut prepared);
            let changed = prepared.selection != self.value.selection()
                || prepared.annotated_string != *self.value.annotated_string();
            changed.then(|| prepared.value())
        };
        if let Some(value) = new_value {
            (self.on_value_change)(value);
        }
    }
}

/// The field's keys (Compose's `textFieldKeyInput`, a composed modifier): its prepared selection's state and dead
/// key combiner remembered, a key input made of this composition's value.
#[allow(clippy::too_many_arguments)]
#[track_caller]
pub fn text_field_key_input(
    modifier: Modifier,
    state: &Rc<LegacyTextFieldState>,
    manager: &Rc<TextFieldSelectionManager>,
    value: &TextFieldValue,
    on_value_change: OnValueChange,
    editable: bool,
    single_line: bool,
    offset_mapping: &OffsetMapping,
    undo_manager: &UndoManager,
    ime_action: ImeAction,
) -> Modifier {
    kompoz_runtime::group(|| {
        kompoz_runtime::kompozable_block!({
            let prepared_selection_state = remember(|| Rc::new(RefCell::new(TextPreparedSelectionState::new())));
            let key_combiner = remember(|| Rc::new(RefCell::new(DeadKeyCombiner::new())));
            let processor = TextFieldKeyInput::new(
                state.clone(),
                manager.clone(),
                value.clone(),
                editable,
                single_line,
                prepared_selection_state,
                offset_mapping.clone(),
                Some(undo_manager.clone()),
                key_combiner,
                platform_default_key_mapping(),
                on_value_change,
                ime_action,
            );
            modifier.on_key_event(move |event| processor.process(event))
        })
    })
}

//! A state-based field's key handling (Compose's TextFieldKeyEventHandler, desktop's `createTextFieldKeyEventHandler`):
//! typed characters (key events that type: see [`is_typed_event`](crate::text::is_typed_event)) through the dead key
//! combiner, then the platform's [key mapping](crate::text::platform_default_key_mapping)'s commands — moves,
//! selections and deletions by characters, words, paragraphs, lines and pages, the clipboard, undo and redo, new
//! lines and tabs (or submitting a single-line field, the focus moving on with Tab), the keyboard shown again. A key
//! down it takes, its key up is taken too.

use super::text_layout_state::TextLayoutState;
use super::text_prepared_selection::{SelectionMovementDeletionContext, TextFieldPreparedSelectionState};
use super::transformed_text_field_state::{SelectionWedgeAffinity, TransformedTextFieldState};
use super::undo::TextFieldEditUndoBehavior;
use crate::text::dead_key_combiner::DeadKeyCombiner;
use crate::text::key_event_helpers::{
    cancels_text_selection, is_from_hardware_source, is_from_soft_keyboard, is_typed_event, show_character_palette,
};
use crate::text::key_mapping::{KeyCommand, KeyMapping, platform_default_key_mapping};
use kompoz_ui::input::{KeyEvent, KeyEventType};
use kompoz_ui::platform::SoftwareKeyboardController;
use std::collections::HashSet;
use std::rc::Rc;

/// What runs the clipboard's commands (copy, cut, paste) for the field (Compose's ClipboardKeyCommandsHandler):
/// the command and whether it came from a physical keyboard.
pub type ClipboardKeyCommandsHandler = Rc<dyn Fn(KeyCommand, bool)>;

/// A state-based field's key handler (Compose's TextFieldKeyEventHandler): one per field, keeping the x vertical
/// moves keep, the dead key pending, and the keys it took down.
pub struct TextFieldKeyEventHandler {
    prepared_selection_state: TextFieldPreparedSelectionState,
    dead_key_combiner: DeadKeyCombiner,
    key_mapping: Rc<dyn KeyMapping>,
    /// The keys whose downs it took: their ups are taken too (not to reach the nodes around).
    currently_consumed_down_keys: HashSet<u64>,
}

impl Default for TextFieldKeyEventHandler {
    fn default() -> Self {
        TextFieldKeyEventHandler::new()
    }
}

impl TextFieldKeyEventHandler {
    /// One mapping keys with the platform's mapping (Compose's `createTextFieldKeyEventHandler`, the desktop's).
    pub fn new() -> TextFieldKeyEventHandler {
        TextFieldKeyEventHandler::with_key_mapping(platform_default_key_mapping())
    }

    /// One mapping keys with `key_mapping`.
    pub fn with_key_mapping(key_mapping: Rc<dyn KeyMapping>) -> TextFieldKeyEventHandler {
        TextFieldKeyEventHandler {
            prepared_selection_state: TextFieldPreparedSelectionState::default(),
            dead_key_combiner: DeadKeyCombiner::new(),
            key_mapping,
            currently_consumed_down_keys: HashSet::new(),
        }
    }

    /// Forgets the column kept for Up and Down (Compose's `preparedSelectionState.resetCachedX()`): the field's
    /// decorator calls it when its selection moved other than by these keys (typing through the input session:
    /// Compose desktop's typed key events reset it).
    pub fn reset_cached_x(&mut self) {
        self.prepared_selection_state.reset_cached_x();
    }

    /// Before the field's own key handling (Compose's `onPreKeyEvent`): a key that cancels a selection deselects
    /// it (`deselect`, the selection state's) and is taken.
    pub fn on_pre_key_event(
        &mut self,
        event: &KeyEvent,
        text_field_state: &TransformedTextFieldState,
        deselect: impl FnOnce(),
    ) -> bool {
        let selection = text_field_state.visual_text().selection();
        if !selection.collapsed() && cancels_text_selection(event) {
            deselect();
            true
        } else {
            false
        }
    }

    /// The field's key handling (Compose's `onKeyEvent`): whether it took the event.
    #[allow(clippy::too_many_arguments)]
    pub fn on_key_event(
        &mut self,
        event: &KeyEvent,
        text_field_state: &TransformedTextFieldState,
        text_layout_state: &TextLayoutState,
        clipboard_key_commands_handler: &ClipboardKeyCommandsHandler,
        keyboard_controller: &SoftwareKeyboardController,
        editable: bool,
        single_line: bool,
        on_submit: impl FnOnce() -> bool,
    ) -> bool {
        let key_code = event.key.0;
        if event.r#type == KeyEventType::KeyUp {
            return self.currently_consumed_down_keys.remove(&key_code);
        }
        if event.r#type == KeyEventType::Unknown && !is_typed_event(event) {
            return false;
        }
        let consumed = self.process_key_down_event(
            event,
            text_field_state,
            text_layout_state,
            clipboard_key_commands_handler,
            keyboard_controller,
            editable,
            single_line,
            on_submit,
        );
        if consumed {
            self.currently_consumed_down_keys.insert(key_code);
        }
        consumed
    }

    #[allow(clippy::too_many_arguments)]
    fn process_key_down_event(
        &mut self,
        event: &KeyEvent,
        state: &TransformedTextFieldState,
        text_layout_state: &TextLayoutState,
        clipboard: &ClipboardKeyCommandsHandler,
        keyboard_controller: &SoftwareKeyboardController,
        editable: bool,
        single_line: bool,
        on_submit: impl FnOnce() -> bool,
    ) -> bool {
        let soft = is_from_soft_keyboard(event);
        let hardware = is_from_hardware_source(event);
        if is_typed_event(event)
            && let Some(code_point) = self.dead_key_combiner.consume(event)
        {
            if !editable {
                return false;
            }
            let text = char::from_u32(code_point as u32).map(String::from).unwrap_or_default();
            state.replace_selected_text(&text, true, TextFieldEditUndoBehavior::MergeIfPossible, !soft, hardware);
            self.prepared_selection_state.reset_cached_x();
            return true;
        }
        let Some(command) = self.key_mapping.map(event) else { return false };
        if command.edits_text() && !editable {
            return false;
        }
        let layout_result = text_layout_state.layout_result();
        let visible_height = visible_text_layout_height(text_layout_state);
        let mut c = SelectionMovementDeletionContext::new(
            state,
            layout_result,
            soft,
            hardware,
            visible_height,
            &mut self.prepared_selection_state,
        );
        let mut consumed = true;
        use KeyCommand::*;
        match command {
            COPY | PASTE | CUT => clipboard(command, hardware),
            LEFT_CHAR => {
                c.collapse_left_or(|c| {
                    c.move_cursor_left_by_char();
                });
            }
            RIGHT_CHAR => {
                c.collapse_right_or(|c| {
                    c.move_cursor_right_by_char();
                });
            }
            LEFT_WORD => {
                c.move_cursor_left_by_word();
            }
            RIGHT_WORD => {
                c.move_cursor_right_by_word();
            }
            PREV_PARAGRAPH => {
                c.move_cursor_prev_by_paragraph();
            }
            NEXT_PARAGRAPH => {
                c.move_cursor_next_by_paragraph();
            }
            UP => {
                c.move_cursor_up_by_line();
            }
            DOWN => {
                c.move_cursor_down_by_line();
            }
            PAGE_UP => {
                c.move_cursor_up_by_page();
            }
            PAGE_DOWN => {
                c.move_cursor_down_by_page();
            }
            LINE_START => {
                c.move_cursor_to_line_start();
            }
            LINE_END => {
                c.move_cursor_to_line_end();
            }
            LINE_LEFT => {
                c.move_cursor_to_line_left_side();
            }
            LINE_RIGHT => {
                c.move_cursor_to_line_right_side();
            }
            HOME => {
                c.move_cursor_to_home();
            }
            END => {
                c.move_cursor_to_end();
            }
            DELETE_PREV_CHAR => {
                c.move_cursor_prev_by_code_point_or_emoji().delete_movement();
            }
            DELETE_NEXT_CHAR => {
                c.move_cursor_next_by_char().delete_movement();
            }
            DELETE_PREV_WORD => {
                c.move_cursor_prev_by_word().delete_movement();
            }
            DELETE_NEXT_WORD => {
                c.move_cursor_next_by_word().delete_movement();
            }
            DELETE_FROM_LINE_START => {
                c.move_cursor_to_line_start().delete_movement();
            }
            DELETE_TO_LINE_END => {
                c.move_cursor_to_line_end().delete_movement();
            }
            NEW_LINE => {
                if !single_line {
                    state.replace_selected_text(
                        "\n",
                        true,
                        TextFieldEditUndoBehavior::MergeIfPossible,
                        !soft,
                        hardware,
                    );
                } else {
                    consumed = on_submit();
                }
            }
            TAB => {
                if !single_line {
                    state.replace_selected_text(
                        "\t",
                        true,
                        TextFieldEditUndoBehavior::MergeIfPossible,
                        !soft,
                        hardware,
                    );
                } else {
                    // (On to the focus system.)
                    consumed = false;
                }
            }
            SELECT_ALL => {
                c.select_all();
            }
            SELECT_LEFT_CHAR => {
                c.move_cursor_left_by_char().select_movement();
            }
            SELECT_RIGHT_CHAR => {
                c.move_cursor_right_by_char().select_movement();
            }
            SELECT_LEFT_WORD => {
                c.move_cursor_left_by_word().select_movement();
            }
            SELECT_RIGHT_WORD => {
                c.move_cursor_right_by_word().select_movement();
            }
            SELECT_PREV_PARAGRAPH => {
                c.move_cursor_prev_by_paragraph().select_movement();
            }
            SELECT_NEXT_PARAGRAPH => {
                c.move_cursor_next_by_paragraph().select_movement();
            }
            SELECT_LINE_START => {
                c.move_cursor_to_line_start().select_movement();
            }
            SELECT_LINE_END => {
                c.move_cursor_to_line_end().select_movement();
            }
            SELECT_LINE_LEFT => {
                c.move_cursor_to_line_left_side().select_movement();
            }
            SELECT_LINE_RIGHT => {
                c.move_cursor_to_line_right_side().select_movement();
            }
            SELECT_UP => {
                c.move_cursor_up_by_line().select_movement();
            }
            SELECT_DOWN => {
                c.move_cursor_down_by_line().select_movement();
            }
            SELECT_PAGE_UP => {
                c.move_cursor_up_by_page().select_movement();
            }
            SELECT_PAGE_DOWN => {
                c.move_cursor_down_by_page().select_movement();
            }
            SELECT_HOME => {
                c.move_cursor_to_home().select_movement();
            }
            SELECT_END => {
                c.move_cursor_to_end().select_movement();
            }
            DESELECT => {
                c.deselect();
            }
            UNDO => state.undo(),
            REDO => state.redo(),
            CHARACTER_PALETTE => show_character_palette(),
            CENTER => keyboard_controller.show(),
        }
        // A move that changed nothing wasn't taken.
        if matches!(command, UP | DOWN | LEFT_CHAR | RIGHT_CHAR) {
            consumed = c.initial_value.selection() != c.selection;
        }
        // (The selection applied at once, at the end.)
        if c.selection != c.initial_value.selection() {
            state.select_chars_in(c.selection);
        }
        if let Some(wedge) = c.wedge_affinity {
            let collapsed =
                kompoz_runtime::without_read_observation(|| state.untransformed_text().selection().collapsed());
            if collapsed {
                state.set_selection_wedge_affinity(SelectionWedgeAffinity::new(wedge));
            } else {
                state.set_selection_wedge_affinity(SelectionWedgeAffinity {
                    end_affinity: wedge,
                    ..c.initial_wedge_affinity
                });
            }
        }
        consumed
    }
}

/// The height of the field's visible text (Compose's `getVisibleTextLayoutHeight`): the text layout node's bounds
/// in the decoration box; NaN until laid out.
fn visible_text_layout_height(state: &TextLayoutState) -> f32 {
    let (Some(text), Some(decorator)) = (state.text_layout_node_coordinates(), state.decorator_node_coordinates())
    else {
        return f32::NAN;
    };
    if !text.is_attached() || !decorator.is_attached() {
        return f32::NAN;
    }
    decorator.local_bounding_box_of(&text, true).height()
}

//! A value-based field's semantics (Compose's CoreTextFieldSemanticsModifier): its value's text (input text), the
//! shown text (editable text), the selection; autofill's content and fill; disabled, password, editable; the
//! editing actions (set text, insert at the cursor, set the selection, the input method's action, click and long
//! click, copy, cut, paste) — through the input session's processor while one is open.

use crate::text::core_text_field::{LegacyTextFieldState, request_focus_and_show_keyboard_if_needed};
use crate::text::selection::text_field_selection_manager::TextFieldSelectionManager;
use crate::text::text_field_delegate as delegate;
use kompoz_text::input::{EditCommand, OffsetMapping, TextFieldValue, TransformedText};
use kompoz_text::{AnnotatedString, TextRange};
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::platform::{ImeOptions, KeyboardType};
use kompoz_ui::semantics::{ContentDataType, ContentType, FillableData, SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::RefCell;
use std::rc::Rc;

/// The semantics node's element (Compose's CoreTextFieldSemanticsModifier, a data class).
#[derive(Clone)]
pub struct CoreTextFieldSemanticsModifier {
    pub transformed_text: TransformedText,
    pub value: TextFieldValue,
    pub state: Rc<LegacyTextFieldState>,
    pub read_only: bool,
    pub enabled: bool,
    pub is_password: bool,
    pub offset_mapping: OffsetMapping,
    pub manager: Rc<TextFieldSelectionManager>,
    pub ime_options: ImeOptions,
    pub focus_requester: FocusRequester,
}

impl PartialEq for CoreTextFieldSemanticsModifier {
    fn eq(&self, other: &Self) -> bool {
        self.transformed_text == other.transformed_text
            && self.value == other.value
            && Rc::ptr_eq(&self.state, &other.state)
            && self.read_only == other.read_only
            && self.enabled == other.enabled
            && self.is_password == other.is_password
            && self.offset_mapping == other.offset_mapping
            && Rc::ptr_eq(&self.manager, &other.manager)
            && self.ime_options == other.ime_options
            && self.focus_requester == other.focus_requester
    }
}

impl ModifierNodeElement for CoreTextFieldSemanticsModifier {
    type Node = CoreTextFieldSemanticsModifierNode;
    fn create(&self) -> CoreTextFieldSemanticsModifierNode {
        let node = CoreTextFieldSemanticsModifierNode {
            fields: Rc::new(RefCell::new(self.clone())),
            handle: Rc::new(RefCell::new(None)),
        };
        node.set_request_autofill_action(&self.manager);
        node
    }
    fn update(&self, node: &mut CoreTextFieldSemanticsModifierNode) {
        node.update_node_semantics(self);
    }
    fn name(&self) -> &'static str {
        "CoreTextFieldSemanticsModifier"
    }
}

/// The semantics node (Compose's CoreTextFieldSemanticsModifierNode).
pub struct CoreTextFieldSemanticsModifierNode {
    fields: Rc<RefCell<CoreTextFieldSemanticsModifier>>,
    handle: Rc<RefCell<Option<NodeHandle>>>,
}

impl CoreTextFieldSemanticsModifierNode {
    /// The manager's autofill action asks the platform's autofill for this field (Compose's `requestAutofill`).
    fn set_request_autofill_action(&self, manager: &TextFieldSelectionManager) {
        let handle = self.handle.clone();
        manager.set_request_autofill_action(Some(Rc::new(move || {
            if let Some(cx) = handle.borrow().as_ref().and_then(|h| h.cx()) {
                cx.request_autofill();
            }
        })));
    }

    /// Takes a new element's values (Compose's `updateNodeSemantics`): the semantics told again where something
    /// they show changed.
    fn update_node_semantics(&mut self, e: &CoreTextFieldSemanticsModifier) {
        let (previous_editable, previous_enabled, previous_is_password, previous_ime_options, previous_manager) = {
            let f = self.fields.borrow();
            (f.enabled && !f.read_only, f.enabled, f.is_password, f.ime_options.clone(), f.manager.clone())
        };
        let editable = e.enabled && !e.read_only;
        *self.fields.borrow_mut() = e.clone();
        if (e.enabled != previous_enabled
            || editable != previous_editable
            || e.ime_options != previous_ime_options
            || e.is_password != previous_is_password
            || !e.value.selection().collapsed())
            && let Some(h) = self.handle.borrow().as_ref()
        {
            h.invalidate_semantics();
        }
        if !Rc::ptr_eq(&e.manager, &previous_manager) {
            self.set_request_autofill_action(&e.manager);
        }
    }
}

/// A text from accessibility or autofill (Compose's private `handleTextUpdateFromSemantics`): through the session's
/// processor while one is open (as an input method's commands, so the buffer stays in step), else as a value with
/// the caret at its end.
fn handle_text_update_from_semantics(state: &LegacyTextFieldState, text: &str, read_only: bool, enabled: bool) {
    if read_only || !enabled {
        return;
    }
    match state.input_session() {
        Some(session) => delegate::on_edit_command(
            &[EditCommand::DeleteAll, EditCommand::CommitText { text: text.to_string(), new_cursor_position: 1 }],
            &state.processor,
            &state.on_value_change_callback(),
            Some(&session),
        ),
        None => state.on_value_change(TextFieldValue::from_text(text, TextRange::at(text.len()), None)),
    }
}

impl ModifierNode for CoreTextFieldSemanticsModifierNode {
    node_capabilities!(semantics);

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.handle.borrow_mut() = Some(cx.handle());
    }
}

impl SemanticsModifierNode for CoreTextFieldSemanticsModifierNode {
    fn should_merge_descendant_semantics(&self) -> bool {
        true
    }

    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        let f = self.fields.borrow().clone();
        let value = f.value.clone();
        config.set_input_text(value.annotated_string().clone());
        config.set_editable_text(f.transformed_text.text.clone());
        config.set_text_selection_range(value.selection());

        // Autofill: the app sets the content type; a field holds text.
        config.set_content_data_type(ContentDataType::Text);
        config.set_fillable_data(FillableData::Text(value.text().to_string()));
        let (fill_state, read_only, enabled) = (f.state.clone(), f.read_only, f.enabled);
        config.on_fill_data(move |data: FillableData| {
            fill_state.set_just_autofilled(true);
            fill_state.set_autofill_highlight_on(true);
            let text = data.text_value().unwrap_or_default().to_string();
            handle_text_update_from_semantics(&fill_state, &text, read_only, enabled);
            true
        });
        match f.ime_options.keyboard_type {
            KeyboardType::Email => config.set_content_type(ContentType::EmailAddress()),
            KeyboardType::Password | KeyboardType::NumberPassword => config.set_content_type(ContentType::Password()),
            KeyboardType::Phone => config.set_content_type(ContentType::PhoneNumber()),
            _ => {}
        }

        if !f.enabled {
            config.disabled();
        }
        if f.is_password {
            config.password();
        }
        let editable = f.enabled && !f.read_only;
        config.set_is_editable(editable);
        let layout_state = f.state.clone();
        config.get_text_layout_result(move |results| match layout_state.peek_layout_result() {
            Some(result) => {
                results.push(result.value.clone());
                true
            }
            None => false,
        });

        if editable {
            let set_state = f.state.clone();
            config.set_text_action(move |text: AnnotatedString| {
                handle_text_update_from_semantics(&set_state, text.text(), read_only, enabled);
                true
            });
            let (insert_state, insert_value) = (f.state.clone(), value.clone());
            config.insert_text_at_cursor(move |text: AnnotatedString| {
                if read_only || !enabled {
                    return false;
                }
                // In a session: as an input method's commands (its composition finished first), the buffer in
                // step with what follows before the next composition (the tests' path).
                match insert_state.input_session() {
                    Some(session) => delegate::on_edit_command(
                        &[
                            EditCommand::FinishComposingText,
                            EditCommand::CommitText { text: text.text().to_string(), new_cursor_position: 1 },
                        ],
                        &insert_state.processor,
                        &insert_state.on_value_change_callback(),
                        Some(&session),
                    ),
                    None => {
                        let selection = insert_value.selection();
                        let current = insert_value.text();
                        let (start, end) = (selection.start.min(current.len()), selection.end.min(current.len()));
                        let (lo, hi) = (start.min(end), start.max(end));
                        // (Kotlin's replaceRange(start, end): start before end)
                        let new_text = format!("{}{}{}", &current[..lo], text.text(), &current[hi..]);
                        let new_cursor = TextRange::at(start + text.text().len());
                        insert_state.on_value_change(TextFieldValue::from_text(&new_text, new_cursor, None));
                    }
                }
                true
            });
        }

        let (select_state, select_manager, select_value, select_mapping) =
            (f.state.clone(), f.manager.clone(), value.clone(), f.offset_mapping.clone());
        config.set_selection(move |selection_start: i32, selection_end: i32, relative_to_original_text: bool| {
            // In traversal mode the selection is the original text's (`textSelectionRange`); otherwise the shown
            // text's.
            let map = |offset: i32| -> i64 {
                if offset < 0 {
                    return offset as i64;
                }
                if relative_to_original_text {
                    offset as i64
                } else {
                    select_mapping.transformed_to_original(offset as usize) as i64
                }
            };
            let (start, end) = (map(selection_start), map(selection_end));
            let current = select_value.selection();
            if !enabled || (start == current.start as i64 && end == current.end as i64) {
                false
            } else if start.min(end) >= 0 && start.max(end) <= select_value.text().len() as i64 {
                // No toolbar for traversal (the volume keys) nor a caret moved to an end.
                if relative_to_original_text || start == end {
                    select_manager.exit_selection_mode();
                } else {
                    select_manager.enter_selection_mode(true);
                }
                select_state.on_value_change(TextFieldValue::new(
                    select_value.annotated_string().clone(),
                    TextRange::new(start as usize, end as usize),
                    None,
                ));
                true
            } else {
                select_manager.exit_selection_mode();
                false
            }
        });
        let (ime_state, ime_action) = (f.state.clone(), f.ime_options.ime_action);
        config.on_ime_action(ime_action, move || {
            // (the default action where the app has none: the platform's own is never wanted)
            ime_state.on_ime_action_performed(ime_action);
            true
        });
        let (click_state, focus_requester) = (f.state.clone(), f.focus_requester.clone());
        config.on_click(move || {
            // (given even when disabled)
            request_focus_and_show_keyboard_if_needed(&click_state, &focus_requester, !read_only);
            true
        });
        let long_click_manager = f.manager.clone();
        config.on_long_click(move || {
            long_click_manager.enter_selection_mode(true);
            true
        });
        if !value.selection().collapsed() && !f.is_password {
            let copy_manager = f.manager.clone();
            config.copy_text(move || {
                copy_manager.copy(true);
                true
            });
            if f.enabled && !f.read_only {
                let cut_manager = f.manager.clone();
                config.cut_text(move || {
                    cut_manager.cut();
                    true
                });
            }
        }
        if f.enabled && !f.read_only {
            let paste_manager = f.manager.clone();
            config.paste_text(move || {
                paste_manager.paste();
                true
            });
        }
    }
}

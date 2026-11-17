//! Text actions on a node (Compose's TextActions.kt): typing at the caret, replacing or clearing the text, setting
//! the selection, the keyboard's action key — through the node's semantics actions, the node focused first, as
//! Compose's do (a text field takes them as its input method's).

use crate::interaction::{SemanticsNodeInteraction, general_error};
use crate::matcher::{
    has_ime_action, has_insert_text_at_cursor_action, has_perform_ime_action, has_request_focus_action,
    has_set_text_action, is_enabled, is_focused,
};
use kompoz_text::{AnnotatedString, TextRange};
use kompoz_ui::platform::ImeAction;
use kompoz_ui::semantics::{SemanticsActions, SemanticsNode};

impl SemanticsNodeInteraction<'_> {
    /// The node, checked for text input (enabled, focusable, and with SetText and InsertTextAtCursor if
    /// `require_editable`), focused if it isn't (Compose's `getNodeAndFocus`).
    #[track_caller]
    fn get_node_and_focus(&self, error_on_fail: &str, require_editable: bool) -> SemanticsNode {
        let node = self.fetch_with(error_on_fail);
        let fail = |what: &str| -> ! {
            panic!("{}", general_error(&format!("{error_on_fail}\nReason: {what}"), &self.selector, &node))
        };
        if !is_enabled().matches(&node) {
            fail("the node is not enabled");
        }
        if !has_request_focus_action().matches(&node) {
            fail("the node has no RequestFocus action");
        }
        if require_editable {
            if !has_set_text_action().matches(&node) {
                fail("the node has no SetText action");
            }
            if !has_insert_text_at_cursor_action().matches(&node) {
                fail("the node has no InsertTextAtCursor action");
            }
        }
        if !is_focused().matches(&node) {
            self.perform_semantics_action(SemanticsActions::RequestFocus);
        }
        node
    }

    /// Types `text` at the caret, in place of the selection (Compose's `performTextInput`: the node focused, its
    /// InsertTextAtCursor action), the rule run until idle.
    #[track_caller]
    pub fn perform_text_input(&self, text: &str) -> &Self {
        self.get_node_and_focus("Failed to perform text input.", true);
        self.perform_semantics_action_with(SemanticsActions::InsertTextAtCursor, |f| {
            f(AnnotatedString::new(text));
        })
    }

    /// Replaces the node's text with `text` (Compose's `performTextReplacement`: its SetText action).
    #[track_caller]
    pub fn perform_text_replacement(&self, text: &str) -> &Self {
        self.get_node_and_focus("Failed to perform text input.", true);
        self.perform_semantics_action_with(SemanticsActions::SetText, |f| {
            f(AnnotatedString::new(text));
        })
    }

    /// Clears the node's text (Compose's `performTextClearance`).
    #[track_caller]
    pub fn perform_text_clearance(&self) -> &Self {
        self.perform_text_replacement("")
    }

    /// Selects `selection` of the node's text — of its original text if `relative_to_original_text`, else of the
    /// text it shows (Compose's `performTextInputSelection`: its SetSelection action, the node focused).
    #[track_caller]
    pub fn perform_text_input_selection(&self, selection: TextRange, relative_to_original_text: bool) -> &Self {
        self.get_node_and_focus("Failed to perform text input selection.", false);
        self.perform_semantics_action_with(SemanticsActions::SetSelection, |f| {
            f(selection.min() as i32, selection.max() as i32, relative_to_original_text);
        })
    }

    /// Performs the node's keyboard action (Compose's `performImeAction`: its PerformImeAction action, the node
    /// focused). Panics if it has none, its action is the default one, or the action says it didn't run.
    #[track_caller]
    pub fn perform_ime_action(&self) -> &Self {
        let error_on_fail = "Failed to perform IME action.";
        self.assert(has_perform_ime_action());
        let node = self.fetch_with(error_on_fail);
        if has_ime_action(ImeAction::Default).matches(&node) {
            panic!(
                "{}",
                general_error(
                    &format!("{error_on_fail}\nReason: its action is ImeAction.Default"),
                    &self.selector,
                    &node
                )
            );
        }
        let node = self.get_node_and_focus(error_on_fail, false);
        let mut performed = true;
        self.perform_semantics_action_with(SemanticsActions::OnImeAction, |f| performed = f());
        if !performed {
            panic!("{}", general_error("Failed to perform IME action, handler returned false.", &self.selector, &node));
        }
        self
    }
}

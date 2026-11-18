//! A value-based field's selection (Compose's selection/TextFieldSelectionManager.kt and its Android actuals): the
//! touch observers (a long press selecting words then dragging, the handles' and the cursor handle's drags), the
//! mouse's (clicks, double and triple clicks, drags, shift to extend), selection mode and the floating toolbar,
//! copy, cut, paste, select all and autofill, the handles' positions, the magnifier's center.
//!
//! The clipboard is called at once (KompozR's is synchronous; Compose launches undispatched coroutines). The text
//! toolbar is the classic one (Compose's new context menu path is a flag, off on the desktop).

use super::adjustment::SelectionAdjustment;
use super::gestures::MouseSelectionObserver;
use super::handles::{OffsetProvider, SelectionHandle};
use super::helpers::{get_line_height, get_selection_handle_coordinates, is_position_inside_selection};
use super::layout::{SelectionLayout, get_text_field_selection_layout};
use super::manager::visible_bounds;
use super::mode::contains_inclusive;
use crate::magnifier::MagnifierModifiers;
use crate::text::core_text_field::{HandleState, LegacyTextFieldState, ObservedCell};
use crate::text::input::internal::selection::DefaultCursorThickness;
use crate::text::long_press_text_drag_observer::{
    Handle, TextDragObserver, detect_down_and_drag_gestures_with_observer,
};
use crate::text::text_field_delegate::OnValueChange;
use crate::text::undo_manager::UndoManager;
use crate::text::validating_offset_mapping::validating_empty_offset_mapping_identity;
use kompoz_coroutines::CoroutineScope;
use kompoz_graphics::{Density, Dp, DpSize, IntSize, Offset, Rect};
use kompoz_runtime::{MutableState, mutable_state_of, remember, remember_keyed, without_read_observation};
use kompoz_text::input::{OffsetMapping, TextFieldValue, VisualTransformation};
use kompoz_text::{AnnotatedString, ResolvedTextDirection, TextRange};
use kompoz_ui::Modifier;
use kompoz_ui::focus::{FocusDirection, FocusRequester};
use kompoz_ui::input::PointerInputScope;
use kompoz_ui::platform::{
    Clipboard, HapticFeedback, HapticFeedbackType, LocalDensity, TextToolbar, TextToolbarAction, TextToolbarStatus,
};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// A value-based field's selection manager (Compose's TextFieldSelectionManager): its field's value, state and
/// offset mapping as the composition gives them, what selection gestures and handles do with them, and the
/// toolbar's and clipboard's actions.
pub struct TextFieldSelectionManager {
    pub undo_manager: Option<UndoManager>,
    offset_mapping: RefCell<OffsetMapping>,
    on_value_change: RefCell<OnValueChange>,
    state: RefCell<Option<Rc<LegacyTextFieldState>>>,
    /// The field's value (a state: Compose's `valueState`).
    value_state: ObservedCell<TextFieldValue>,
    visual_transformation: RefCell<VisualTransformation>,
    request_autofill_action: RefCell<Option<Rc<dyn Fn()>>>,
    clipboard: RefCell<Option<Clipboard>>,
    coroutine_scope: RefCell<Option<CoroutineScope>>,
    text_toolbar: RefCell<Option<Rc<dyn TextToolbar>>>,
    haptic_feed_back: RefCell<Option<HapticFeedback>>,
    focus_requester: RefCell<Option<FocusRequester>>,
    editable: MutableState<bool>,
    enabled: MutableState<bool>,
    /// Where a drag began, in the decoration box.
    drag_begin_position: Cell<Offset>,
    /// The selection when a long press drag began (its words).
    drag_begin_selection: Cell<Option<TextRange>>,
    drag_total_distance: Cell<Offset>,
    dragging_handle: MutableState<Option<Handle>>,
    current_drag_position: MutableState<Option<Offset>>,
    /// The raw offset of the last drag (none: none since the drag began).
    previous_raw_drag_offset: Cell<Option<usize>>,
    /// The value when selection mode began (Compose's `oldValue`).
    old_value: RefCell<TextFieldValue>,
    previous_selection_layout: RefCell<Option<SelectionLayout>>,
    /// The latest selection given (Compose's `latestSelection`).
    latest_selection: Cell<Option<TextRange>>,
    has_available_text_to_paste: MutableState<bool>,
    touch_selection_observer: RefCell<Option<Rc<TouchSelectionObserver>>>,
    mouse_selection_observer: RefCell<Option<Rc<MouseObserver>>>,
    this: RefCell<Weak<TextFieldSelectionManager>>,
}

impl TextFieldSelectionManager {
    /// One recording key commands' snapshots in `undo_manager`.
    pub fn new(undo_manager: Option<UndoManager>) -> Rc<TextFieldSelectionManager> {
        let manager = Rc::new(TextFieldSelectionManager {
            undo_manager,
            offset_mapping: RefCell::new(validating_empty_offset_mapping_identity()),
            on_value_change: RefCell::new(Rc::new(|_| {})),
            state: RefCell::new(None),
            value_state: ObservedCell::new(TextFieldValue::default()),
            visual_transformation: RefCell::new(VisualTransformation::None),
            request_autofill_action: RefCell::new(None),
            clipboard: RefCell::new(None),
            coroutine_scope: RefCell::new(None),
            text_toolbar: RefCell::new(None),
            haptic_feed_back: RefCell::new(None),
            focus_requester: RefCell::new(None),
            editable: mutable_state_of(true),
            enabled: mutable_state_of(true),
            drag_begin_position: Cell::new(Offset::Zero),
            drag_begin_selection: Cell::new(None),
            drag_total_distance: Cell::new(Offset::Zero),
            dragging_handle: mutable_state_of(None),
            current_drag_position: mutable_state_of(None),
            previous_raw_drag_offset: Cell::new(None),
            old_value: RefCell::new(TextFieldValue::default()),
            previous_selection_layout: RefCell::new(None),
            latest_selection: Cell::new(None),
            has_available_text_to_paste: mutable_state_of(false),
            touch_selection_observer: RefCell::new(None),
            mouse_selection_observer: RefCell::new(None),
            this: RefCell::new(Weak::new()),
        });
        let weak = Rc::downgrade(&manager);
        *manager.this.borrow_mut() = weak.clone();
        *manager.touch_selection_observer.borrow_mut() = Some(Rc::new(TouchSelectionObserver {
            manager: weak.clone(),
            is_long_press_selection_only: Cell::new(true),
            running_selection: Cell::new(None),
            selection_adjustment_mode: Cell::new(SelectionAdjustment::None),
        }));
        *manager.mouse_selection_observer.borrow_mut() = Some(Rc::new(MouseObserver {
            manager: weak,
            is_double_or_triple_click_selection_only: Cell::new(true),
            initial_selection: Cell::new(None),
        }));
        manager
    }

    // ---- What the composition gives

    pub fn offset_mapping(&self) -> OffsetMapping {
        self.offset_mapping.borrow().clone()
    }
    pub fn set_offset_mapping(&self, offset_mapping: OffsetMapping) {
        *self.offset_mapping.borrow_mut() = offset_mapping;
    }
    pub fn set_on_value_change(&self, on_value_change: OnValueChange) {
        *self.on_value_change.borrow_mut() = on_value_change;
    }
    fn on_value_change(&self, value: TextFieldValue) {
        let f = self.on_value_change.borrow().clone();
        f(value);
    }
    pub fn state(&self) -> Option<Rc<LegacyTextFieldState>> {
        self.state.borrow().clone()
    }
    pub fn set_state(&self, state: Option<Rc<LegacyTextFieldState>>) {
        *self.state.borrow_mut() = state;
    }

    /// The field's value (read as a state).
    pub fn value(&self) -> TextFieldValue {
        self.value_state.get()
    }

    /// The field's value, not observed.
    pub fn value_peek(&self) -> TextFieldValue {
        self.value_state.peek()
    }

    /// Takes the field's value (its readers told only of another one), its selection the latest.
    pub fn set_value(&self, value: TextFieldValue) {
        let selection = value.selection();
        if self.value_state.peek() != value {
            self.value_state.set(value);
        }
        self.latest_selection.set(Some(selection));
    }

    /// What the field shows (Compose's `transformedText`): its text delegate's text.
    pub fn transformed_text(&self) -> Option<AnnotatedString> {
        self.state().map(|s| s.text_delegate().text().clone())
    }

    pub fn visual_transformation(&self) -> VisualTransformation {
        self.visual_transformation.borrow().clone()
    }
    pub fn set_visual_transformation(&self, visual_transformation: VisualTransformation) {
        *self.visual_transformation.borrow_mut() = visual_transformation;
    }
    pub fn set_request_autofill_action(&self, action: Option<Rc<dyn Fn()>>) {
        *self.request_autofill_action.borrow_mut() = action;
    }
    pub fn set_clipboard(&self, clipboard: Option<Clipboard>) {
        *self.clipboard.borrow_mut() = clipboard;
    }
    pub fn set_coroutine_scope(&self, scope: Option<CoroutineScope>) {
        *self.coroutine_scope.borrow_mut() = scope;
    }
    pub fn text_toolbar(&self) -> Option<Rc<dyn TextToolbar>> {
        self.text_toolbar.borrow().clone()
    }
    pub fn set_text_toolbar(&self, toolbar: Option<Rc<dyn TextToolbar>>) {
        *self.text_toolbar.borrow_mut() = toolbar;
    }
    pub fn set_haptic_feed_back(&self, haptic: Option<HapticFeedback>) {
        *self.haptic_feed_back.borrow_mut() = haptic;
    }
    pub fn set_focus_requester(&self, focus_requester: Option<FocusRequester>) {
        *self.focus_requester.borrow_mut() = focus_requester;
    }
    pub fn editable(&self) -> bool {
        self.editable.get()
    }
    pub fn set_editable(&self, editable: bool) {
        self.editable.set(editable);
    }
    pub fn enabled(&self) -> bool {
        self.enabled.get()
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }

    /// The handle being dragged, if any (a state: Compose's `draggingHandle`).
    pub fn dragging_handle(&self) -> Option<Handle> {
        self.dragging_handle.get()
    }

    /// Where the drag is, in the decoration box, if one is under way (a state: Compose's `currentDragPosition`).
    pub fn current_drag_position(&self) -> Option<Offset> {
        self.current_drag_position.get()
    }

    /// The latest selection given (Compose's `latestSelection`).
    pub fn latest_selection(&self) -> Option<TextRange> {
        self.latest_selection.get()
    }

    /// The touch's selection observer (Compose's `touchSelectionObserver`).
    pub fn touch_selection_observer(&self) -> Rc<dyn TextDragObserver> {
        self.touch_selection_observer.borrow().clone().expect("the touch observer")
    }

    /// The mouse's selection observer (Compose's `mouseSelectionObserver`).
    pub fn mouse_selection_observer(&self) -> Rc<dyn MouseSelectionObserver> {
        self.mouse_selection_observer.borrow().clone().expect("the mouse observer")
    }

    fn rc(&self) -> Rc<TextFieldSelectionManager> {
        self.this.borrow().upgrade().expect("a live manager")
    }

    fn perform_haptic(&self, kind: HapticFeedbackType) {
        let haptic = self.haptic_feed_back.borrow().clone();
        if let Some(h) = haptic {
            h.perform_haptic_feedback(kind);
        }
    }

    fn request_focus(&self) {
        let requester = self.focus_requester.borrow().clone();
        if let Some(r) = requester {
            r.request_focus_with(FocusDirection::Enter);
        }
    }

    // ---- Handles

    /// What observes a selection handle's drags (Compose's `handleDragObserver`).
    pub fn handle_drag_observer(&self, is_start_handle: bool) -> Rc<dyn TextDragObserver> {
        Rc::new(HandleDragObserver { manager: self.this.borrow().clone(), is_start_handle })
    }

    /// What observes the cursor handle's drags (Compose's `cursorDragObserver`).
    pub fn cursor_drag_observer(&self) -> Rc<dyn TextDragObserver> {
        Rc::new(CursorDragObserver { manager: self.this.borrow().clone() })
    }

    /// Enters selection mode (Compose's `enterSelectionMode`): the field focused, its value then noted, the toolbar
    /// shown if `show_floating_toolbar`, the selection's handles.
    pub fn enter_selection_mode(&self, show_floating_toolbar: bool) {
        if self.state().is_some_and(|s| !s.has_focus()) {
            self.request_focus();
        }
        *self.old_value.borrow_mut() = self.value_peek();
        self.update_floating_toolbar(show_floating_toolbar);
        self.set_handle_state(HandleState::Selection);
    }

    /// Leaves selection mode (Compose's `exitSelectionMode`): no toolbar, no handles.
    pub fn exit_selection_mode(&self) {
        self.update_floating_toolbar(false);
        self.set_handle_state(HandleState::None);
    }

    /// A selection collapses to a caret — at `position` (in the decoration box) if given, else at its end; the
    /// cursor's handle where a position is given on text (Compose's `deselect`).
    pub fn deselect(&self, position: Option<Offset>) {
        let value = self.value_peek();
        if !value.selection().collapsed() {
            let layout_result = self.state().and_then(|s| s.peek_layout_result());
            let new_cursor_offset = match (position, layout_result) {
                (Some(position), Some(layout)) => {
                    self.offset_mapping().transformed_to_original(layout.get_offset_for_position(position, true))
                }
                _ => value.selection().max(),
            };
            let new_value = TextFieldValue::new(
                value.annotated_string().clone(),
                TextRange::at(new_cursor_offset),
                value.composition(),
            );
            let selection = new_value.selection();
            self.on_value_change(new_value);
            self.latest_selection.set(Some(selection));
        }
        let selection_mode =
            if position.is_some() && !value.text().is_empty() { HandleState::Cursor } else { HandleState::None };
        self.set_handle_state(selection_mode);
        self.update_floating_toolbar(false);
    }

    /// An input method's selection preview highlight (Compose's `setSelectionPreviewHighlight`).
    pub fn set_selection_preview_highlight(&self, range: TextRange) {
        if let Some(state) = self.state() {
            state.set_selection_preview_highlight_range(range);
            state.set_deletion_preview_highlight_range(TextRange::Zero);
        }
        if !range.collapsed() {
            self.exit_selection_mode();
        }
    }

    /// An input method's deletion preview highlight (Compose's `setDeletionPreviewHighlight`).
    pub fn set_deletion_preview_highlight(&self, range: TextRange) {
        if let Some(state) = self.state() {
            state.set_deletion_preview_highlight_range(range);
            state.set_selection_preview_highlight_range(TextRange::Zero);
        }
        if !range.collapsed() {
            self.exit_selection_mode();
        }
    }

    /// No preview highlight (Compose's `clearPreviewHighlight`).
    pub fn clear_preview_highlight(&self) {
        if let Some(state) = self.state() {
            state.set_deletion_preview_highlight_range(TextRange::Zero);
            state.set_selection_preview_highlight_range(TextRange::Zero);
        }
    }

    /// Whether the text toolbar is shown (Compose's `textToolbarShown`).
    pub fn text_toolbar_shown(&self) -> bool {
        self.text_toolbar().is_some_and(|t| t.status() == TextToolbarStatus::Shown)
    }

    fn is_password(&self) -> bool {
        self.visual_transformation.borrow().is_password()
    }

    fn has_selection(&self) -> bool {
        !self.value().selection().collapsed()
    }

    // ---- The clipboard's actions

    /// Whether copy can be offered (Compose's `canShowCopyMenuItem`).
    pub fn can_show_copy_menu_item(&self) -> bool {
        self.is_copy_allowed() && self.clipboard.borrow().is_some()
    }

    /// Whether copying is allowed: a selection, not a password's (Compose's `isCopyAllowed`).
    pub fn is_copy_allowed(&self) -> bool {
        self.has_selection() && !self.is_password()
    }

    /// Reads whether the clipboard has text to paste (Compose's `updateClipboardEntry`, its desktop
    /// `hasAvailableTextToPaste`).
    pub fn update_clipboard_entry(&self) {
        let clipboard = self.clipboard.borrow().clone();
        if let Some(clipboard) = clipboard {
            self.has_available_text_to_paste.set(clipboard.get_text().is_some_and(|t| !t.is_empty()));
        }
    }

    /// The shown text and the latest selection in it, for a context menu (Compose's `getContextTextAndSelection`).
    pub fn get_context_text_and_selection(&self) -> Option<(String, TextRange)> {
        let text = self.transformed_text()?.text().to_string();
        let selection = self.latest_selection.get()?;
        let mapping = self.offset_mapping();
        Some((
            text,
            TextRange::new(
                mapping.original_to_transformed(selection.start),
                mapping.original_to_transformed(selection.end),
            ),
        ))
    }

    /// Whether paste can be offered (Compose's `canShowPasteMenuItem`): text in the clipboard as last read.
    pub fn can_show_paste_menu_item(&self) -> bool {
        self.is_paste_allowed() && self.has_available_text_to_paste.get() && self.clipboard.borrow().is_some()
    }

    /// Whether pasting is allowed: editable (Compose's `isPasteAllowed`).
    pub fn is_paste_allowed(&self) -> bool {
        self.editable()
    }

    /// Whether cut can be offered (Compose's `canShowCutMenuItem`).
    pub fn can_show_cut_menu_item(&self) -> bool {
        self.is_cut_allowed() && self.clipboard.borrow().is_some()
    }

    /// Whether cutting is allowed: a selection, editable, not a password's (Compose's `isCutAllowed`).
    pub fn is_cut_allowed(&self) -> bool {
        self.has_selection() && self.editable() && !self.is_password()
    }

    /// Whether select all can be offered: not all selected (Compose's `canShowSelectAllMenuItem`).
    pub fn can_show_select_all_menu_item(&self) -> bool {
        let value = self.value();
        value.selection().length() != value.text().len()
    }

    /// Whether autofill can be offered: editable, a caret (Compose's `canShowAutofillMenuItem`).
    pub fn can_show_autofill_menu_item(&self) -> bool {
        self.editable() && self.value().selection().collapsed()
    }

    /// Copies the selected text to the clipboard, collapsing the selection to its end when `cancel_selection`
    /// (Compose's `copy`).
    pub fn copy(&self, cancel_selection: bool) {
        if let Some(value_to_copy) = self.copy_with_result(cancel_selection) {
            let clipboard = self.clipboard.borrow().clone();
            if let Some(clipboard) = clipboard {
                clipboard.set_text(value_to_copy.text());
            }
        }
    }

    /// The selected text, the selection collapsed to its end when `cancel_selection` (Compose's
    /// `copyWithResult`): none where copying isn't allowed.
    pub fn copy_with_result(&self, cancel_selection: bool) -> Option<AnnotatedString> {
        if !self.is_copy_allowed() {
            return None;
        }
        let value = self.value_peek();
        let selected_text = value.get_selected_text();
        if !cancel_selection {
            return Some(selected_text);
        }
        let new_cursor_offset = value.selection().max();
        self.on_value_change(TextFieldValue::new(
            value.annotated_string().clone(),
            TextRange::at(new_cursor_offset),
            None,
        ));
        self.set_handle_state(HandleState::None);
        Some(selected_text)
    }

    /// Pastes the clipboard's text over the selection (Compose's `paste()`).
    pub fn paste(&self) {
        let clipboard = self.clipboard.borrow().clone();
        let Some(text) = clipboard.and_then(|c| c.get_text()) else { return };
        self.paste_text(&AnnotatedString::new(text));
    }

    /// Pastes `text` over the selection, the caret after it, a step of its own in the history (Compose's
    /// `paste(text)`).
    pub fn paste_text(&self, text: &AnnotatedString) {
        if !self.is_paste_allowed() {
            return;
        }
        let value = self.value_peek();
        let len = value.text().len();
        let mut builder = kompoz_text::AnnotatedStringBuilder::new();
        builder.append(value.get_text_before_selection(len));
        builder.append(text);
        builder.append(value.get_text_after_selection(len));
        let new_text = builder.to_annotated_string();
        let new_cursor_offset = value.selection().min() + text.text().len();
        self.on_value_change(TextFieldValue::new(new_text, TextRange::at(new_cursor_offset), None));
        self.set_handle_state(HandleState::None);
        if let Some(undo_manager) = &self.undo_manager {
            undo_manager.force_next_snapshot();
        }
    }

    /// Cuts the selected text to the clipboard (Compose's `cut`).
    pub fn cut(&self) {
        if let Some(value_to_cut) = self.cut_with_result() {
            let clipboard = self.clipboard.borrow().clone();
            if let Some(clipboard) = clipboard {
                clipboard.set_text(value_to_cut.text());
            }
        }
    }

    /// The selected text removed, given back; a step of its own in the history (Compose's `cutWithResult`): none
    /// where cutting isn't allowed.
    pub fn cut_with_result(&self) -> Option<AnnotatedString> {
        if !self.is_cut_allowed() {
            return None;
        }
        let value = self.value_peek();
        let selected_text = value.get_selected_text();
        let len = value.text().len();
        let mut builder = kompoz_text::AnnotatedStringBuilder::new();
        builder.append(value.get_text_before_selection(len));
        builder.append(value.get_text_after_selection(len));
        let new_text = builder.to_annotated_string();
        let new_cursor_offset = value.selection().min();
        self.on_value_change(TextFieldValue::new(new_text, TextRange::at(new_cursor_offset), None));
        self.set_handle_state(HandleState::None);
        if let Some(undo_manager) = &self.undo_manager {
            undo_manager.force_next_snapshot();
        }
        Some(selected_text)
    }

    /// Selects all the text, in selection mode with the toolbar (Compose's `selectAll`).
    pub fn select_all(&self) {
        let value = self.value_peek();
        let new_value =
            TextFieldValue::new(value.annotated_string().clone(), TextRange::new(0, value.text().len()), None);
        let selection = new_value.selection();
        self.on_value_change(new_value);
        self.latest_selection.set(Some(selection));
        let old = self.old_value.borrow().clone();
        *self.old_value.borrow_mut() =
            TextFieldValue::new(old.annotated_string().clone(), selection, old.composition());
        self.enter_selection_mode(true);
    }

    /// Asks for the platform's autofill (Compose's `autofill`).
    pub fn autofill(&self) {
        let action = self.request_autofill_action.borrow().clone();
        if let Some(action) = action {
            action();
        }
    }

    /// Where a selection handle points, in the inner field (Compose's `getHandlePosition`): unspecified until the
    /// layout is of the shown text.
    pub fn get_handle_position(&self, is_start_handle: bool) -> Offset {
        let Some(state) = self.state() else { return Offset::Unspecified };
        let Some(layout) = state.layout_result() else { return Offset::Unspecified };
        let text_layout_result = &layout.value;
        // (the layout and the value out of step: asked again once they are in step)
        let Some(transformed_text) = self.transformed_text() else { return Offset::Unspecified };
        if transformed_text.text() != text_layout_result.layout_input.text.text() {
            return Offset::Unspecified;
        }
        let value = self.value();
        let offset = if is_start_handle { value.selection().start } else { value.selection().end };
        get_selection_handle_coordinates(
            text_layout_result,
            self.offset_mapping().original_to_transformed(offset),
            is_start_handle,
            value.selection().reversed(),
        )
    }

    /// The height of a handle's line (Compose's `getHandleLineHeight`).
    pub fn get_handle_line_height(&self, is_start_handle: bool) -> f32 {
        let value = self.value();
        let offset = if is_start_handle { value.selection().start } else { value.selection().end };
        self.state().and_then(|s| s.layout_result()).map_or(0.0, |l| get_line_height(&l.value, offset))
    }

    /// Where the cursor handle points: the caret's middle, at its bottom (Compose's `getCursorPosition`).
    pub fn get_cursor_position(&self, density: Density) -> Offset {
        let cursor_rect = self.get_cursor_rect();
        Offset::new(cursor_rect.left + density.to_px(DefaultCursorThickness) / 2.0, cursor_rect.bottom)
    }

    /// The caret's rect in the inner field (Compose's `getCursorRect`).
    pub fn get_cursor_rect(&self) -> Rect {
        let offset = self.offset_mapping().original_to_transformed(self.value().selection().start);
        let Some(layout) = self.state().and_then(|s| s.layout_result()) else { return Rect::Zero };
        let len = layout.value.layout_input.text.text().len();
        layout.value.get_cursor_rect(offset.min(len))
    }

    /// Shows the floating toolbar or hides it (Compose's `updateFloatingToolbar`).
    fn update_floating_toolbar(&self, show: bool) {
        if let Some(state) = self.state() {
            state.set_show_floating_toolbar(show);
        }
        if show {
            self.show_selection_toolbar();
        } else {
            self.hide_selection_toolbar();
        }
    }

    /// Shows the text toolbar over the selection with what can be done now (Compose's `showSelectionToolbar`, its
    /// `showSelectionToolbarViaTextToolbar`): not while disabled or used with a mouse.
    pub fn show_selection_toolbar(&self) {
        let allowed = without_read_observation(|| self.enabled() && self.state().is_none_or(|s| s.is_in_touch_mode()));
        if !allowed {
            return;
        }
        self.update_clipboard_entry();
        without_read_observation(|| {
            let me = self.rc();
            let action = |enabled: bool, f: fn(&TextFieldSelectionManager)| -> TextToolbarAction {
                if !enabled {
                    return None;
                }
                let weak = Rc::downgrade(&me);
                Some(Rc::new(move || {
                    if let Some(m) = weak.upgrade() {
                        f(&m);
                    }
                }))
            };
            let copy = action(self.can_show_copy_menu_item(), |m| {
                m.copy(true);
                m.hide_selection_toolbar();
            });
            let cut = action(self.can_show_cut_menu_item(), |m| {
                m.cut();
                m.hide_selection_toolbar();
            });
            let paste = action(self.can_show_paste_menu_item(), |m| {
                m.paste();
                m.hide_selection_toolbar();
            });
            let select_all = action(self.can_show_select_all_menu_item(), |m| m.select_all());
            let autofill = action(self.can_show_autofill_menu_item(), |m| m.autofill());
            if let Some(toolbar) = self.text_toolbar() {
                toolbar.show_menu(self.get_content_rect(), copy, paste, cut, select_all, autofill);
            }
        });
    }

    /// Hides the text toolbar if it shows (Compose's `hideSelectionToolbar`).
    pub fn hide_selection_toolbar(&self) {
        if let Some(toolbar) = self.text_toolbar()
            && toolbar.status() == TextToolbarStatus::Shown
        {
            toolbar.hide();
        }
    }

    /// Selects the word at `position` (in the decoration box) unless it is in the selection already (Compose's
    /// `selectWordAtPositionIfNotAlreadySelected`: a context menu's right click).
    pub fn select_word_at_position_if_not_already_selected(&self, position: Offset) {
        let Some(layout) = self.state().and_then(|s| s.peek_layout_result()) else { return };
        let inside = is_position_inside_selection(
            &layout.value,
            layout.translate_decoration_to_inner_coordinates(position),
            Some(self.value_peek().selection()),
        );
        if !inside {
            self.update_selection(&self.value_peek(), position, true, false, SelectionAdjustment::Word, false, None);
        }
    }

    /// Whether the text changed since selection mode began (Compose's `isTextChanged`).
    pub fn is_text_changed(&self) -> bool {
        self.old_value.borrow().text() != self.value().text()
    }

    /// The selection's rect in the root, the handles' height below it, for the toolbar (Compose's private
    /// `getContentRect`): empty while the layout is stale.
    fn get_content_rect(&self) -> Rect {
        let Some(state) = self.state().filter(|s| !s.is_layout_result_stale()) else { return Rect::Zero };
        let value = self.value();
        let mapping = self.offset_mapping();
        let transformed_start = mapping.original_to_transformed(value.selection().start);
        let transformed_end = mapping.original_to_transformed(value.selection().end);
        let coordinates = state.layout_coordinates();
        let to_root = |p: Offset| coordinates.as_ref().map_or(Offset::Zero, |c| c.local_to_root(p));
        let start_offset = to_root(self.get_handle_position(true));
        let end_offset = to_root(self.get_handle_position(false));
        let layout = state.layout_result();
        let top_of = |offset: usize| {
            let y = layout.as_ref().map_or(0.0, |l| l.value.get_cursor_rect(offset).top);
            coordinates.as_ref().map_or(0.0, |c| c.local_to_root(Offset::new(0.0, y)).y)
        };
        let (start_top, end_top) = (top_of(transformed_start), top_of(transformed_end));
        let left = start_offset.x.min(end_offset.x);
        let right = start_offset.x.max(end_offset.x);
        let top = start_top.min(end_top);
        let bottom = start_offset.y.max(end_offset.y) + 25.0 * state.text_delegate().density().density;
        Rect::new(left, top, right, bottom)
    }

    /// The selection for a gesture at `current_position` (in the decoration box), adjusted, given to the field
    /// (Compose's private `updateSelection`): a haptic tick for touch where it changed, the handles shown where in
    /// view, the toolbar following a mouse's selection.
    #[allow(clippy::too_many_arguments)]
    fn update_selection(
        &self,
        value: &TextFieldValue,
        current_position: Offset,
        is_start_of_selection: bool,
        is_start_handle: bool,
        adjustment: SelectionAdjustment,
        is_touch_based_selection: bool,
        haptic_feedback_type: Option<HapticFeedbackType>,
    ) -> TextRange {
        let Some(state) = self.state() else { return TextRange::Zero };
        let Some(layout_result) = state.peek_layout_result() else { return TextRange::Zero };
        let mapping = self.offset_mapping();
        let previous_transformed_selection = TextRange::new(
            mapping.original_to_transformed(value.selection().start),
            mapping.original_to_transformed(value.selection().end),
        );
        let current_offset = layout_result.get_offset_for_position(current_position, false);
        let raw_start_handle_offset = if is_start_handle || is_start_of_selection {
            current_offset
        } else {
            previous_transformed_selection.start
        };
        let raw_end_handle_offset =
            if !is_start_handle || is_start_of_selection { current_offset } else { previous_transformed_selection.end };
        let previous_selection_layout = self.previous_selection_layout.borrow().clone();
        let raw_previous_handle_offset = if is_start_of_selection || previous_selection_layout.is_none() {
            None
        } else {
            self.previous_raw_drag_offset.get()
        };
        let selection_layout = get_text_field_selection_layout(
            &layout_result.value,
            raw_start_handle_offset,
            raw_end_handle_offset,
            raw_previous_handle_offset,
            previous_transformed_selection,
            is_start_of_selection,
            is_start_handle,
        );
        if !selection_layout.should_recompute_selection(previous_selection_layout.as_ref()) {
            return value.selection();
        }
        let new_transformed_selection = adjustment.adjust(&selection_layout);
        *self.previous_selection_layout.borrow_mut() = Some(selection_layout);
        self.previous_raw_drag_offset.set(Some(current_offset));
        let new_selection = TextRange::new(
            mapping.transformed_to_original(new_transformed_selection.start.offset),
            mapping.transformed_to_original(new_transformed_selection.end.offset),
        );
        if new_selection == value.selection() {
            return value.selection();
        }
        let only_change_is_reversed = new_selection.reversed() != value.selection().reversed()
            && TextRange::new(new_selection.end, new_selection.start) == value.selection();
        let both_selections_collapsed = new_selection.collapsed() && value.selection().collapsed();
        if is_touch_based_selection
            && !value.text().is_empty()
            && !only_change_is_reversed
            && !both_selections_collapsed
            && let Some(kind) = haptic_feedback_type
        {
            self.perform_haptic(kind);
        }
        self.on_value_change(TextFieldValue::new(value.annotated_string().clone(), new_selection, None));
        self.latest_selection.set(Some(new_selection));
        if !is_touch_based_selection {
            self.update_floating_toolbar(!new_selection.collapsed());
        }
        state.set_is_in_touch_mode(is_touch_based_selection);
        // (the handles may have been hidden scrolled out of view: shown or hidden again for the new selection)
        state.set_show_selection_handle_start(
            !new_selection.collapsed() && self.is_selection_handle_in_visible_bound(true),
        );
        state.set_show_selection_handle_end(
            !new_selection.collapsed() && self.is_selection_handle_in_visible_bound(false),
        );
        state.set_show_cursor_handle(new_selection.collapsed() && self.is_selection_handle_in_visible_bound(true));
        new_selection
    }

    fn set_handle_state(&self, handle_state: HandleState) {
        if let Some(state) = self.state()
            && state.peek_handle_state() != handle_state
        {
            state.set_handle_state(handle_state);
        }
    }

    /// Whether a handle points within the inner field's visible bounds (Compose's
    /// `isSelectionHandleInVisibleBound`, its default).
    pub fn is_selection_handle_in_visible_bound(&self, is_start_handle: bool) -> bool {
        let Some(coordinates) = self.state().and_then(|s| s.layout_coordinates()) else { return false };
        contains_inclusive(visible_bounds(&coordinates), self.get_handle_position(is_start_handle))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The observers

/// A long press then a drag (Compose's `touchSelectionObserver`): the drag moves the selection's end.
struct TouchSelectionObserver {
    manager: Weak<TextFieldSelectionManager>,
    is_long_press_selection_only: Cell<bool>,
    running_selection: Cell<Option<TextRange>>,
    selection_adjustment_mode: Cell<SelectionAdjustment>,
}

impl TouchSelectionObserver {
    fn on_end(&self, m: &TextFieldSelectionManager) {
        m.dragging_handle.set(None);
        m.current_drag_position.set(None);
        self.selection_adjustment_mode.set(SelectionAdjustment::None);
        m.update_floating_toolbar(true);
        let collapsed = self.running_selection.get().map_or(m.value_peek().selection().collapsed(), |s| s.collapsed());
        m.set_handle_state(if collapsed { HandleState::Cursor } else { HandleState::Selection });
        if let Some(state) = m.state() {
            state.set_show_selection_handle_start(!collapsed && m.is_selection_handle_in_visible_bound(true));
            state.set_show_selection_handle_end(!collapsed && m.is_selection_handle_in_visible_bound(false));
            state.set_show_cursor_handle(collapsed && m.is_selection_handle_in_visible_bound(true));
        }
        // (a long press alone would have a platform's selection suggested: none on the desktop)
        m.drag_begin_selection.set(None);
    }
}

impl TextDragObserver for TouchSelectionObserver {
    fn on_down(&self, _point: Offset) {}

    fn on_up(&self) {}

    fn on_start(&self, start_point: Offset, selection_adjustment: SelectionAdjustment) {
        let Some(m) = self.manager.upgrade() else { return };
        if !m.enabled.peek() || m.dragging_handle.peek().is_some() {
            return;
        }
        // The selection's end follows the drag.
        m.dragging_handle.set(Some(Handle::SelectionEnd));
        m.previous_raw_drag_offset.set(None);
        self.is_long_press_selection_only.set(true);
        self.selection_adjustment_mode.set(selection_adjustment);
        m.hide_selection_toolbar();
        let layout_result = m.state().and_then(|s| s.peek_layout_result());
        if layout_result.as_ref().is_none_or(|l| !l.is_position_on_text(start_point)) {
            // A long press beside the text: a caret at its line's end.
            if let Some(layout_result) = layout_result {
                let transformed_offset = layout_result.get_offset_for_position(start_point, true);
                let offset = m.offset_mapping().transformed_to_original(transformed_offset);
                let value = m.value_peek();
                let new_value = TextFieldValue::new(value.annotated_string().clone(), TextRange::at(offset), None);
                m.enter_selection_mode(false);
                m.perform_haptic(HapticFeedbackType::LongPress);
                let selection = new_value.selection();
                m.on_value_change(new_value);
                m.latest_selection.set(Some(selection));
            }
            self.is_long_press_selection_only.set(false);
        } else {
            let value = m.value_peek();
            if value.text().is_empty() {
                return;
            }
            m.enter_selection_mode(false);
            // (from no selection: one before isn't the context of this one)
            let adjusted_start_selection = m.update_selection(
                &TextFieldValue::new(value.annotated_string().clone(), TextRange::Zero, value.composition()),
                start_point,
                true,
                false,
                selection_adjustment,
                true,
                Some(HapticFeedbackType::LongPress),
            );
            // Touch begins at the adjusted selection (a character selection snaps to its first word's start).
            m.drag_begin_selection.set(Some(adjusted_start_selection));
            self.running_selection.set(Some(adjusted_start_selection));
        }
        // (no handles until the drag ends)
        m.set_handle_state(HandleState::None);
        m.drag_begin_position.set(start_point);
        m.current_drag_position.set(Some(start_point));
        m.drag_total_distance.set(Offset::Zero);
    }

    fn on_drag(&self, delta: Offset) {
        let Some(m) = self.manager.upgrade() else { return };
        // (no selection started: no drag taken)
        if !m.enabled.peek() || m.value_peek().text().is_empty() {
            return;
        }
        m.drag_total_distance.set(m.drag_total_distance.get() + delta);
        let Some(layout_result) = m.state().and_then(|s| s.peek_layout_result()) else { return };
        let current = m.drag_begin_position.get() + m.drag_total_distance.get();
        m.current_drag_position.set(Some(current));
        let value = m.value_peek();
        let new_selection = if m.drag_begin_selection.get().is_none() && !layout_result.is_position_on_text(current) {
            // Both the drag's start and end beside the text.
            let mapping = m.offset_mapping();
            let start_offset = mapping
                .transformed_to_original(layout_result.get_offset_for_position(m.drag_begin_position.get(), true));
            let end_offset = mapping.transformed_to_original(layout_result.get_offset_for_position(current, true));
            let adjustment =
                if start_offset == end_offset { SelectionAdjustment::None } else { SelectionAdjustment::Word };
            m.update_selection(
                &value,
                current,
                false,
                false,
                adjustment,
                true,
                Some(HapticFeedbackType::TextHandleMove),
            )
        } else {
            let start_offset = m
                .drag_begin_selection
                .get()
                .map_or_else(|| layout_result.get_offset_for_position(m.drag_begin_position.get(), false), |s| s.start);
            let end_offset = layout_result.get_offset_for_position(current, false);
            if m.drag_begin_selection.get().is_none() && start_offset == end_offset {
                // From beside the text: no selection until one isn't collapsed.
                return;
            }
            m.update_selection(
                &value,
                current,
                false,
                false,
                self.selection_adjustment_mode.get(),
                true,
                Some(HapticFeedbackType::TextHandleMove),
            )
        };
        self.running_selection.set(Some(new_selection));
        if Some(new_selection) != m.drag_begin_selection.get() {
            self.is_long_press_selection_only.set(false);
        }
        m.update_floating_toolbar(false);
    }

    fn on_stop(&self) {
        if let Some(m) = self.manager.upgrade() {
            self.on_end(&m);
        }
    }

    fn on_cancel(&self) {
        if let Some(m) = self.manager.upgrade() {
            self.on_end(&m);
        }
    }
}

/// The mouse's selection (Compose's `mouseSelectionObserver`).
struct MouseObserver {
    manager: Weak<TextFieldSelectionManager>,
    is_double_or_triple_click_selection_only: Cell<bool>,
    initial_selection: Cell<Option<TextRange>>,
}

impl MouseObserver {
    /// Compose's `updateMouseSelection`.
    fn update_mouse_selection(
        &self,
        m: &TextFieldSelectionManager,
        current_position: Offset,
        is_start_of_selection: bool,
        adjustment: SelectionAdjustment,
    ) -> TextRange {
        let value = m.value_peek();
        let new_selection =
            m.update_selection(&value, current_position, is_start_of_selection, false, adjustment, false, None);
        if Some(new_selection) != self.initial_selection.get() {
            self.is_double_or_triple_click_selection_only.set(false);
        }
        m.set_handle_state(if new_selection.collapsed() { HandleState::Cursor } else { HandleState::Selection });
        new_selection
    }

    fn usable(&self) -> Option<Rc<TextFieldSelectionManager>> {
        let m = self.manager.upgrade()?;
        // (no selection without a layout: not taken)
        m.state().and_then(|s| s.peek_layout_result())?;
        Some(m)
    }
}

impl MouseSelectionObserver for MouseObserver {
    fn on_extend(&self, down_position: Offset) -> bool {
        let Some(m) = self.usable() else { return false };
        if !m.enabled.peek() {
            return false;
        }
        m.previous_raw_drag_offset.set(None);
        m.request_focus();
        self.update_mouse_selection(&m, down_position, false, SelectionAdjustment::None);
        true
    }

    fn on_extend_drag(&self, drag_position: Offset) -> bool {
        let Some(m) = self.manager.upgrade() else { return false };
        if !m.enabled.peek() || m.value_peek().text().is_empty() {
            return false;
        }
        if m.state().and_then(|s| s.peek_layout_result()).is_none() {
            return false;
        }
        self.update_mouse_selection(&m, drag_position, false, SelectionAdjustment::None);
        true
    }

    fn on_start(&self, down_position: Offset, adjustment: SelectionAdjustment, click_count: u32) -> bool {
        let Some(m) = self.manager.upgrade() else { return false };
        if !m.enabled.peek() || m.value_peek().text().is_empty() {
            return false;
        }
        if m.state().and_then(|s| s.peek_layout_result()).is_none() {
            return false;
        }
        m.request_focus();
        m.drag_begin_position.set(down_position);
        m.previous_raw_drag_offset.set(None);
        m.enter_selection_mode(true);
        let new_selection = self.update_mouse_selection(&m, down_position, true, adjustment);
        if click_count >= 2 {
            self.is_double_or_triple_click_selection_only.set(true);
            self.initial_selection.set(Some(new_selection));
        }
        true
    }

    fn on_drag(&self, drag_position: Offset, adjustment: SelectionAdjustment) -> bool {
        let Some(m) = self.manager.upgrade() else { return false };
        if !m.enabled.peek() || m.value_peek().text().is_empty() {
            return false;
        }
        if m.state().and_then(|s| s.peek_layout_result()).is_none() {
            return false;
        }
        self.update_mouse_selection(&m, drag_position, false, adjustment);
        true
    }

    fn on_drag_done(&self) {
        // (a double or triple click alone would have a platform's selection suggested: none on the desktop)
    }
}

/// A selection handle's drags (Compose's `handleDragObserver`).
struct HandleDragObserver {
    manager: Weak<TextFieldSelectionManager>,
    is_start_handle: bool,
}

impl TextDragObserver for HandleDragObserver {
    fn on_down(&self, _point: Offset) {
        let Some(m) = self.manager.upgrade() else { return };
        m.dragging_handle.set(Some(if self.is_start_handle { Handle::SelectionStart } else { Handle::SelectionEnd }));
        // Where the handle's character is, in the inner field, then in the decoration box.
        let handle_coordinates = super::handles::get_adjusted_coordinates(m.get_handle_position(self.is_start_handle));
        let Some(layout_result) = m.state().and_then(|s| s.peek_layout_result()) else { return };
        let translated_position = layout_result.translate_inner_to_decoration_coordinates(handle_coordinates);
        m.drag_begin_position.set(translated_position);
        m.current_drag_position.set(Some(translated_position));
        m.drag_total_distance.set(Offset::Zero);
        m.previous_raw_drag_offset.set(None);
        if let Some(state) = m.state() {
            state.set_is_in_touch_mode(true);
        }
        m.update_floating_toolbar(false);
    }

    fn on_up(&self) {
        let Some(m) = self.manager.upgrade() else { return };
        m.dragging_handle.set(None);
        m.current_drag_position.set(None);
        m.update_floating_toolbar(true);
    }

    fn on_start(&self, _start_point: Offset, _selection_adjustment: SelectionAdjustment) {
        // (handled in on_down)
    }

    fn on_drag(&self, delta: Offset) {
        let Some(m) = self.manager.upgrade() else { return };
        m.drag_total_distance.set(m.drag_total_distance.get() + delta);
        let current = m.drag_begin_position.get() + m.drag_total_distance.get();
        m.current_drag_position.set(Some(current));
        // (a handle's drag is touch)
        m.update_selection(
            &m.value_peek(),
            current,
            false,
            self.is_start_handle,
            SelectionAdjustment::CharacterWithWordAccelerate,
            true,
            Some(HapticFeedbackType::TextHandleMove),
        );
        m.update_floating_toolbar(false);
    }

    fn on_stop(&self) {
        let Some(m) = self.manager.upgrade() else { return };
        m.dragging_handle.set(None);
        m.current_drag_position.set(None);
        m.update_floating_toolbar(true);
    }

    fn on_cancel(&self) {}
}

/// The cursor handle's drags (Compose's `cursorDragObserver`).
struct CursorDragObserver {
    manager: Weak<TextFieldSelectionManager>,
}

impl TextDragObserver for CursorDragObserver {
    fn on_down(&self, _point: Offset) {}

    fn on_up(&self) {
        let Some(m) = self.manager.upgrade() else { return };
        m.dragging_handle.set(None);
        m.current_drag_position.set(None);
    }

    fn on_start(&self, _start_point: Offset, _selection_adjustment: SelectionAdjustment) {
        let Some(m) = self.manager.upgrade() else { return };
        let handle_coordinates = super::handles::get_adjusted_coordinates(m.get_handle_position(true));
        let Some(layout_result) = m.state().and_then(|s| s.peek_layout_result()) else { return };
        let translated_position = layout_result.translate_inner_to_decoration_coordinates(handle_coordinates);
        m.drag_begin_position.set(translated_position);
        m.current_drag_position.set(Some(translated_position));
        m.drag_total_distance.set(Offset::Zero);
        m.dragging_handle.set(Some(Handle::Cursor));
        m.update_floating_toolbar(false);
    }

    fn on_drag(&self, delta: Offset) {
        let Some(m) = self.manager.upgrade() else { return };
        m.drag_total_distance.set(m.drag_total_distance.get() + delta);
        let Some(layout_result) = m.state().and_then(|s| s.peek_layout_result()) else { return };
        let current = m.drag_begin_position.get() + m.drag_total_distance.get();
        m.current_drag_position.set(Some(current));
        let offset = m.offset_mapping().transformed_to_original(layout_result.get_offset_for_position(current, true));
        let new_selection = TextRange::at(offset);
        let value = m.value_peek();
        // (nothing changed: no change told, no haptics)
        if new_selection == value.selection() {
            return;
        }
        if m.state().is_none_or(|s| s.is_in_touch_mode()) {
            m.perform_haptic(HapticFeedbackType::TextHandleMove);
        }
        m.on_value_change(TextFieldValue::new(value.annotated_string().clone(), new_selection, None));
        m.latest_selection.set(Some(new_selection));
    }

    fn on_stop(&self) {
        let Some(m) = self.manager.upgrade() else { return };
        m.dragging_handle.set(None);
        m.current_drag_position.set(None);
    }

    fn on_cancel(&self) {}
}

// ---------------------------------------------------------------------------------------------------------------
// Composables and modifiers

/// A selection handle of a field (Compose's TextFieldSelectionHandle): where the manager says, dragged by its
/// observer.
#[track_caller]
#[allow(non_snake_case)]
pub fn TextFieldSelectionHandle(
    is_start_handle: bool,
    direction: ResolvedTextDirection,
    manager: Rc<TextFieldSelectionManager>,
) {
    kompoz_runtime::group(|| {
        kompoz_runtime::kompozable_block!({
            let key = (is_start_handle, Rc::as_ptr(&manager) as usize);
            let m = manager.clone();
            let observer = remember_keyed(key, move || m.handle_drag_observer(is_start_handle));
            let provider = manager.clone();
            SelectionHandle(
                offset_provider = OffsetProvider::new(move || provider.get_handle_position(is_start_handle)),
                is_start_handle = is_start_handle,
                direction = direction,
                handles_crossed = manager.value().selection().reversed(),
                line_height = manager.get_handle_line_height(is_start_handle),
                modifier = Modifier.pointer_input(key, move |scope: PointerInputScope| {
                    let observer = observer.clone();
                    async move { detect_down_and_drag_gestures_with_observer(scope, observer).await }
                }),
            );
        })
    })
}

/// Where a dragged handle's magnifier centers, in the decoration box (Compose's
/// `calculateSelectionMagnifierCenterAndroid`): at the drag's x kept within the handle's line, at the line's
/// middle; unspecified when nothing is dragged, the text is empty or not laid out, or the drag went further from
/// the line than half the magnifier's width.
pub fn calculate_selection_magnifier_center_android(
    manager: &TextFieldSelectionManager,
    magnifier_size: IntSize,
) -> Offset {
    // (the drag's position read as a state: composed again as it moves)
    let Some(local_drag_position) = manager.current_drag_position() else { return Offset::Unspecified };
    // Never in an empty field.
    if manager.transformed_text().is_none_or(|t| t.text().is_empty()) {
        return Offset::Unspecified;
    }
    let value = manager.value();
    let raw_text_offset = match manager.dragging_handle() {
        None => return Offset::Unspecified,
        Some(Handle::Cursor) | Some(Handle::SelectionStart) => value.selection().start,
        Some(Handle::SelectionEnd) => value.selection().end,
    };
    // Not laid out yet: none.
    let Some(state) = manager.state() else { return Offset::Unspecified };
    let Some(text_layout_result_proxy) = state.layout_result() else { return Offset::Unspecified };
    let transformed_text = state.text_delegate().text().clone();
    let text_offset =
        manager.offset_mapping().original_to_transformed(raw_text_offset).min(transformed_text.text().len());
    let drag_x = text_layout_result_proxy.translate_decoration_to_inner_coordinates(local_drag_position).x;
    let layout_result = &text_layout_result_proxy.value;
    let line = layout_result.get_line_for_offset(text_offset);
    let line_start = layout_result.get_line_left(line);
    let line_end = layout_result.get_line_right(line);
    let center_x = drag_x.clamp(line_start.min(line_end), line_start.max(line_end));
    // Hidden when dragged too far (Android's Editor), once the magnifier's size is known.
    if magnifier_size != IntSize::Zero && (drag_x - center_x).abs() > (magnifier_size.width / 2) as f32 {
        return Offset::Unspecified;
    }
    let top = layout_result.get_line_top(line);
    let bottom = layout_result.get_line_bottom(line);
    Offset::new(center_x, (bottom - top) / 2.0 + top)
}

/// A field's magnifier while a handle is dragged (Compose's Android `textFieldMagnifier`): its center animated as
/// [`calculate_selection_magnifier_center_android`] moves — along a line at once, between lines by a spring.
#[track_caller]
pub fn text_field_magnifier(modifier: Modifier, manager: &Rc<TextFieldSelectionManager>) -> Modifier {
    kompoz_runtime::group(|| {
        kompoz_runtime::kompozable_block!({
            let density = LocalDensity.current();
            let magnifier_size: MutableState<IntSize> = remember(|| mutable_state_of(IntSize::Zero));
            let (center_manager, size) = (Rc::downgrade(manager), magnifier_size.clone());
            let animated_center =
                super::container::remember_animated_magnifier_position(move || match center_manager.upgrade() {
                    Some(m) => calculate_selection_magnifier_center_android(&m, size.get()),
                    None => Offset::Unspecified,
                });
            let source_center = move |_: &Density| animated_center.get();
            let on_size_changed: Rc<dyn Fn(DpSize)> = Rc::new(move |s: DpSize| {
                magnifier_size.set(IntSize::new(density.round_to_px(s.width), density.round_to_px(s.height)));
            });
            modifier.magnifier_with(
                source_center,
                None,
                Some(on_size_changed),
                f32::NAN,
                DpSize::Unspecified,
                Dp::Unspecified,
                Dp::Unspecified,
                true,
            )
        })
    })
}

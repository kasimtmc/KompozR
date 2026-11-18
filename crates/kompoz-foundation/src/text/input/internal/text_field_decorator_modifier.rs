//! A state-based field's decorator node (Compose's TextFieldDecoratorModifier): on the decoration box, it is the
//! field's focus (a focusable), its input method session while focused and editable, its keys, its gestures (taps,
//! selection, through the selection state), its semantics (the text, the selection, the editing actions,
//! autofill's), its focus rect, the autofill highlight, and the toolbar's and handles' observers while focused.

use super::selection::{TextFieldSelectionState, TextToolbarState};
use super::text_field_key_event_handler::{ClipboardKeyCommandsHandler, TextFieldKeyEventHandler};
use super::text_input_session::platform_specific_text_input_session;
use super::text_layout_state::TextLayoutState;
use super::transformed_text_field_state::TransformedTextFieldState;
use crate::content::internal::{
    ReceiveContentConfiguration, drag_and_drop_request_permission, get_receive_content_configuration,
};
use crate::content::{MediaType, TransferableContent, TransferableContentSource};
use crate::focusable::{FocusableNode, apply_focusable_semantics};
use crate::interaction::MutableInteractionSource;
use crate::interaction::drag_and_drop_hover::DragAndDropHoverInteraction;
use crate::text::autofill_highlight::{
    LocalAutofillHighlightBrush, LocalAutofillHighlightColor, autofill_highlight_color, resolve_autofill_highlight,
};
use crate::text::input::internal::selection::ReceiveContentConfigurationProvider;
use crate::text::input::internal::undo::TextFieldEditUndoBehavior;
use crate::text::input::{InputTransformation, KeyboardActionHandler};
use crate::text::key_mapping::KeyCommand;
use crate::text::keyboard_options::KeyboardOptions;
use crate::text::long_press_text_drag_observer::Handle;
use kompoz_coroutines::{Job, snapshot_flow};
use kompoz_graphics::{IntSize, Offset};
use kompoz_runtime::{MutableState, mutable_state_of};
use kompoz_text::{AnnotatedString, TextRange};
use kompoz_ui::draganddrop::{DragAndDropEvent, DragAndDropNode, DragAndDropTarget, DragAndDropTargetModifierNode};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::focus::{FocusDirection, FocusProperties, FocusPropertiesModifierNode, Focusability};
use kompoz_ui::input::{KeyEvent, KeyInputModifierNode, PointerEvent, PointerEventPass, PointerInputModifierNode};
use kompoz_ui::input::{PointerInputScope, SuspendingPointerInputModifierNode};
use kompoz_ui::layout::{GlobalPositionAwareModifierNode, LayoutAwareModifierNode, LayoutCoordinates};
use kompoz_ui::modifier::{DelegatedNode, ObserverModifierNode};
use kompoz_ui::platform::{
    ImeAction, InputMode, KeyboardType, LocalFocusManager, LocalInputModeManager, LocalSoftwareKeyboardController,
    LocalWindowInfo, SoftwareKeyboardController, WindowInfo,
};
use kompoz_ui::platform_text_input::{PlatformTextInputModifierNode, establish_text_input_session};
use kompoz_ui::semantics::{
    ContentDataType, ContentType, FillableData, InputTextSuggestionState, SemanticsConfiguration, SemanticsModifierNode,
};
use kompoz_ui::{ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The decorator node's element (Compose's TextFieldDecoratorModifier, a data class: equal by value, states and
/// handlers by identity).
#[derive(Clone)]
pub struct TextFieldDecoratorModifier {
    pub text_field_state: TransformedTextFieldState,
    pub text_layout_state: Rc<TextLayoutState>,
    pub text_field_selection_state: Rc<TextFieldSelectionState>,
    pub filter: Option<InputTransformation>,
    pub enabled: bool,
    pub read_only: bool,
    pub keyboard_options: KeyboardOptions,
    pub keyboard_action_handler: Option<Rc<dyn KeyboardActionHandler>>,
    pub single_line: bool,
    pub interaction_source: MutableInteractionSource,
    pub is_password: bool,
}

impl PartialEq for TextFieldDecoratorModifier {
    fn eq(&self, other: &Self) -> bool {
        let same_handler = match (&self.keyboard_action_handler, &other.keyboard_action_handler) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        self.text_field_state == other.text_field_state
            && Rc::ptr_eq(&self.text_layout_state, &other.text_layout_state)
            && Rc::ptr_eq(&self.text_field_selection_state, &other.text_field_selection_state)
            && self.filter == other.filter
            && self.enabled == other.enabled
            && self.read_only == other.read_only
            && self.keyboard_options == other.keyboard_options
            && same_handler
            && self.single_line == other.single_line
            && self.interaction_source == other.interaction_source
            && self.is_password == other.is_password
    }
}

impl ModifierNodeElement for TextFieldDecoratorModifier {
    type Node = TextFieldDecoratorModifierNode;
    fn create(&self) -> TextFieldDecoratorModifierNode {
        TextFieldDecoratorModifierNode::new(self)
    }
    fn update(&self, node: &mut TextFieldDecoratorModifierNode) {
        node.update_node(self);
    }
    fn name(&self) -> &'static str {
        "textFieldDecorator"
    }
}

/// The node the field's input session is established for (Compose's PlatformTextInputModifierNode, the decorator
/// node itself there): a delegate of the decorator, so the session can be started from its callbacks (a focus
/// change, a tap) as from its own methods.
struct TextInputSessionNode;

impl ModifierNode for TextInputSessionNode {}

impl PlatformTextInputModifierNode for TextInputSessionNode {}

/// What the node, its delegates' callbacks and its coroutines share (the node's fields in Compose).
struct DecoratorShared {
    text_field_state: RefCell<TransformedTextFieldState>,
    text_layout_state: RefCell<Rc<TextLayoutState>>,
    text_field_selection_state: RefCell<Rc<TextFieldSelectionState>>,
    filter: RefCell<Option<InputTransformation>>,
    enabled: Cell<bool>,
    read_only: Cell<bool>,
    keyboard_options: RefCell<KeyboardOptions>,
    keyboard_action_handler: RefCell<Option<Rc<dyn KeyboardActionHandler>>>,
    single_line: Cell<bool>,
    interaction_source: RefCell<MutableInteractionSource>,
    is_password: Cell<bool>,
    handle: RefCell<Option<NodeHandle>>,
    focusable: RefCell<Option<DelegatedNode<FocusableNode>>>,
    focusable_delegated: Cell<bool>,
    session_node: RefCell<Option<DelegatedNode<TextInputSessionNode>>>,
    /// The window (read in the observed reads: its focus is a state).
    window_info: RefCell<Option<WindowInfo>>,
    toolbar_and_handles_visibility_observer_job: RefCell<Option<Job>>,
    input_session_job: RefCell<Option<Job>>,
    autofill_highlight_on: MutableState<bool>,
}

impl DecoratorShared {
    fn handle(&self) -> Option<NodeHandle> {
        self.handle.borrow().clone()
    }

    fn cx(&self) -> Option<NodeCx> {
        self.handle().and_then(|h| h.cx())
    }

    fn selection_state(&self) -> Rc<TextFieldSelectionState> {
        self.text_field_selection_state.borrow().clone()
    }

    fn state(&self) -> TransformedTextFieldState {
        self.text_field_state.borrow().clone()
    }

    fn editable(&self) -> bool {
        self.enabled.get() && !self.read_only.get()
    }

    fn focus_state_is_focused(&self) -> bool {
        self.focusable_delegated.get()
            && self.focusable.borrow().as_ref().is_some_and(|f| f.borrow().focus_state().is_focused())
    }

    /// The field and its window have the focus (Compose's `isFocused`: the window's focus read only while the
    /// field has its own).
    fn is_focused(&self) -> bool {
        self.focus_state_is_focused() && self.window_info.borrow().as_ref().is_some_and(|w| w.is_window_focused())
    }

    fn request_focus(&self) {
        let focusable = self.focusable.borrow().clone();
        if self.focusable_delegated.get()
            && let Some(f) = focusable
        {
            f.borrow().request_focus();
        }
    }

    fn keyboard_controller(&self) -> SoftwareKeyboardController {
        self.handle()
            .map_or_else(SoftwareKeyboardController::default, |h| h.current_value_of(&LocalSoftwareKeyboardController))
    }

    /// The focus came or went (the focusable's `onFocusChange`).
    fn on_focus_change(self: &Rc<Self>, is_focused: bool) {
        let editable = self.editable();
        if is_focused {
            self.apply_current_input_mode();
            if editable {
                self.start_input_session(false);
            }
        } else {
            self.dispose_input_session();
            // Only losing the focus ends a composition (not the window losing its own).
            self.state().edit_untransformed_text_as_user(true, |b| b.commit_composition());
            // Deselected, even read-only.
            self.state().collapse_selection_to_max();
        }
        self.update_window_focus();
    }

    /// Reads the window's focus observed, and reacts to the field's focus (Compose's `updateWindowFocus`).
    fn update_window_focus(self: &Rc<Self>) {
        let Some(cx) = self.cx() else { return };
        cx.observe_reads(|| {
            *self.window_info.borrow_mut() = Some(cx.current_value_of(&LocalWindowInfo));
            self.on_is_focused_updated();
        });
    }

    /// Compose's `onIsFocusedUpdated`: the selection state told; its observers run while focused.
    fn on_is_focused_updated(self: &Rc<Self>) {
        let focused = self.is_focused();
        let selection_state = self.selection_state();
        selection_state.set_is_window_and_text_field_focused(focused);
        if focused && self.toolbar_and_handles_visibility_observer_job.borrow().is_none() {
            if let Some(scope) = self.handle().and_then(|h| h.coroutine_scope()) {
                let job = scope.launch(selection_state.start_toolbar_and_handles_visibility_observer());
                *self.toolbar_and_handles_visibility_observer_job.borrow_mut() = Some(job);
            }
        } else if !focused && let Some(job) = self.toolbar_and_handles_visibility_observer_job.borrow_mut().take() {
            job.cancel();
        }
    }

    /// A focus by keys (not touch) shows no touch handles (Compose's `applyCurrentInputMode`).
    fn apply_current_input_mode(&self) {
        if let Some(h) = self.handle()
            && h.current_value_of(&LocalInputModeManager).input_mode() != InputMode::Touch
        {
            self.selection_state().set_is_in_touch_mode(false);
        }
    }

    /// Starts the input method's session (Compose's `startInputSession`): from a tap always, from the focus only
    /// if the keyboard is to show on focus. The session before is ended by the new one.
    fn start_input_session(self: &Rc<Self>, from_tap: bool) {
        if !from_tap && !self.keyboard_options.borrow().show_keyboard_on_focus_or_default() {
            return;
        }
        let Some(cx) = self.cx() else { return };
        let Some(session_node) = self.session_node.borrow().clone() else { return };
        let (state, layout) = (self.state(), self.text_layout_state.borrow().clone());
        let mut ime_options = self.keyboard_options.borrow().to_ime_options(self.single_line.get());
        // A tap shows the keyboard even where the focus alone wouldn't (KompozR's ImeOptions say whether the
        // session's start shows it: Compose shows it with the input method a tap starts).
        ime_options.show_keyboard |= from_tap;
        let weak = Rc::downgrade(self);
        let on_ime_action: Rc<dyn Fn(ImeAction)> = Rc::new(move |action| {
            if let Some(s) = weak.upgrade() {
                s.on_ime_action_performed(action);
            }
        });
        let receive_content_configuration = get_receive_content_configuration(&cx);
        let session = establish_text_input_session(&*session_node.borrow(), &session_node.cx(), move |scope| {
            platform_specific_text_input_session(
                &scope,
                state,
                layout,
                ime_options,
                receive_content_configuration,
                Some(on_ime_action),
            )
        });
        if let Some(old) = self.input_session_job.borrow_mut().take() {
            old.cancel();
        }
        let job = cx.coroutine_scope().launch(async move { match session.await {} });
        *self.input_session_job.borrow_mut() = Some(job);
    }

    /// Ends the session (Compose's `disposeInputSession`).
    fn dispose_input_session(&self) {
        if let Some(job) = self.input_session_job.borrow_mut().take() {
            job.cancel();
        }
    }

    /// The input method's action key or a single-line field's Enter (Compose's `onImeActionPerformed`): the
    /// field's handler, given the default action, else the default action.
    fn on_ime_action_performed(self: &Rc<Self>, ime_action: ImeAction) -> bool {
        let handler = self.keyboard_action_handler.borrow().clone();
        match handler {
            Some(handler) if ime_action != ImeAction::None && ime_action != ImeAction::Default => {
                let weak = Rc::downgrade(self);
                handler.on_keyboard_action(&move || {
                    if let Some(s) = weak.upgrade() {
                        s.default_keyboard_action_with_result(ime_action);
                    }
                });
                true
            }
            _ => self.default_keyboard_action_with_result(ime_action),
        }
    }

    /// Next and Previous move the focus, Done hides the keyboard (Compose's `defaultKeyboardActionWithResult`).
    fn default_keyboard_action_with_result(&self, ime_action: ImeAction) -> bool {
        let Some(h) = self.handle() else { return false };
        match ime_action {
            ImeAction::Next => {
                h.current_value_of(&LocalFocusManager).move_focus(FocusDirection::Next);
                true
            }
            ImeAction::Previous => {
                h.current_value_of(&LocalFocusManager).move_focus(FocusDirection::Previous);
                true
            }
            ImeAction::Done => {
                self.keyboard_controller().hide();
                true
            }
            _ => false,
        }
    }

    /// The nearest content receiver's configuration around the field (Compose's `getReceiveContentConfiguration`).
    fn receive_content_configuration(&self) -> Option<Rc<dyn ReceiveContentConfiguration>> {
        get_receive_content_configuration(&self.cx()?)
    }

    /// What gives the selection state the receiver's configuration (Compose's
    /// `receiveContentConfigurationProvider`).
    fn receive_content_configuration_provider(self: &Rc<Self>) -> ReceiveContentConfigurationProvider {
        let weak = Rc::downgrade(self);
        Rc::new(move || weak.upgrade()?.receive_content_configuration())
    }

    /// Asks the platform's autofill to fill this field (Compose's `requestAutofill`, the toolbar's autofill item).
    fn request_autofill(&self) {
        if let Some(cx) = self.cx() {
            cx.request_autofill();
        }
    }
}

/// The decorator node (Compose's TextFieldDecoratorModifierNode).
pub struct TextFieldDecoratorModifierNode {
    shared: Rc<DecoratorShared>,
    pointer_input_node: Option<DelegatedNode<SuspendingPointerInputModifierNode>>,
    drag_and_drop_node: Option<DelegatedNode<DragAndDropNode>>,
    text_field_key_event_handler: TextFieldKeyEventHandler,
    /// The shown text's selection after the last key event: another one means it moved otherwise (typing
    /// through the input session), and the column Up and Down keep is forgotten.
    selection_after_key_event: Option<TextRange>,
}

impl TextFieldDecoratorModifierNode {
    fn new(e: &TextFieldDecoratorModifier) -> TextFieldDecoratorModifierNode {
        let shared = Rc::new(DecoratorShared {
            text_field_state: RefCell::new(e.text_field_state.clone()),
            text_layout_state: RefCell::new(e.text_layout_state.clone()),
            text_field_selection_state: RefCell::new(e.text_field_selection_state.clone()),
            filter: RefCell::new(e.filter.clone()),
            enabled: Cell::new(e.enabled),
            read_only: Cell::new(e.read_only),
            keyboard_options: RefCell::new(e.keyboard_options.clone()),
            keyboard_action_handler: RefCell::new(e.keyboard_action_handler.clone()),
            single_line: Cell::new(e.single_line),
            interaction_source: RefCell::new(e.interaction_source.clone()),
            is_password: Cell::new(e.is_password),
            handle: RefCell::new(None),
            focusable: RefCell::new(None),
            focusable_delegated: Cell::new(false),
            session_node: RefCell::new(None),
            window_info: RefCell::new(None),
            toolbar_and_handles_visibility_observer_job: RefCell::new(None),
            input_session_job: RefCell::new(None),
            autofill_highlight_on: mutable_state_of(false),
        });
        let weak = Rc::downgrade(&shared);
        e.text_field_selection_state.set_request_autofill_action(Some(Rc::new(move || {
            if let Some(s) = weak.upgrade() {
                s.request_autofill();
            }
        })));
        TextFieldDecoratorModifierNode {
            shared,
            pointer_input_node: None,
            drag_and_drop_node: None,
            text_field_key_event_handler: TextFieldKeyEventHandler::new(),
            selection_after_key_event: None,
        }
    }

    fn make_focusable(&self) -> FocusableNode {
        let weak = Rc::downgrade(&self.shared);
        let on_focus_change: Rc<dyn Fn(bool)> = Rc::new(move |is_focused| {
            if let Some(s) = weak.upgrade() {
                s.on_focus_change(is_focused);
            }
        });
        FocusableNode::new(
            Some(self.shared.interaction_source.borrow().clone()),
            Focusability::Always,
            Some(on_focus_change),
        )
    }

    fn delegate_focusable(&self, cx: &NodeCx) {
        let node = cx.delegate(self.make_focusable());
        *self.shared.focusable.borrow_mut() = Some(node);
        self.shared.focusable_delegated.set(true);
    }

    fn undelegate_focusable(&self, cx: &NodeCx) {
        let node = self.shared.focusable.borrow_mut().take();
        if let Some(node) = node {
            cx.undelegate(&node);
        }
        self.shared.focusable_delegated.set(false);
    }

    /// The gestures: touch mode, taps, selection (Compose's `pointerInputNode`).
    fn pointer_input_block(
        shared: &Rc<DecoratorShared>,
    ) -> impl Fn(PointerInputScope) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + 'static {
        let weak = Rc::downgrade(shared);
        move |scope: PointerInputScope| {
            let weak = weak.clone();
            Box::pin(async move {
                let Some(shared) = weak.upgrade() else { return };
                let selection_state = shared.selection_state();
                let interaction_source = shared.interaction_source.borrow().clone();
                drop(shared);
                let focus = weak.clone();
                let request_focus: Rc<dyn Fn()> = Rc::new(move || {
                    if let Some(s) = focus.upgrade()
                        && !s.selection_state().is_window_and_text_field_focused()
                    {
                        s.request_focus();
                    }
                });
                let keyboard = weak.clone();
                let show_keyboard: Rc<dyn Fn()> = Rc::new(move || {
                    let Some(s) = keyboard.upgrade() else { return };
                    if s.input_session_job.borrow().is_some() {
                        // The session is there: the keyboard again.
                        s.keyboard_controller().show();
                    } else {
                        s.start_input_session(true);
                    }
                });
                kompoz_coroutines::coroutine_scope(move |cs| async move {
                    let (a, b, c) = (selection_state.clone(), selection_state.clone(), selection_state);
                    let (s1, s2, s3) = (scope.clone(), scope.clone(), scope);
                    let focus_for_selection = request_focus.clone();
                    cs.launch(async move { a.detect_touch_mode(s1).await });
                    cs.launch(async move {
                        b.detect_text_field_tap_gestures(s2, Some(interaction_source), request_focus, show_keyboard)
                            .await
                    });
                    cs.launch(async move { c.text_field_selection_gestures(s3, focus_for_selection).await });
                })
                .await;
            })
        }
    }

    /// Takes a new element's values and reacts to what changed (Compose's `updateNode`).
    fn update_node(&mut self, e: &TextFieldDecoratorModifier) {
        let s = self.shared.clone();
        let previous_editable = s.editable();
        let previous_enabled = s.enabled.get();
        let previous_state = s.state();
        let previous_keyboard_options = s.keyboard_options.borrow().clone();
        let previous_selection_state = s.selection_state();
        let previous_interaction_source = s.interaction_source.borrow().clone();
        let previous_is_password = s.is_password.get();
        let editable = e.enabled && !e.read_only;

        *s.text_field_state.borrow_mut() = e.text_field_state.clone();
        *s.text_layout_state.borrow_mut() = e.text_layout_state.clone();
        *s.text_field_selection_state.borrow_mut() = e.text_field_selection_state.clone();
        *s.filter.borrow_mut() = e.filter.clone();
        s.enabled.set(e.enabled);
        s.read_only.set(e.read_only);
        *s.keyboard_options.borrow_mut() = e.keyboard_options.clone();
        *s.keyboard_action_handler.borrow_mut() = e.keyboard_action_handler.clone();
        s.single_line.set(e.single_line);
        *s.interaction_source.borrow_mut() = e.interaction_source.clone();
        s.is_password.set(e.is_password);

        // Something of the session changed: it starts again.
        if editable != previous_editable
            || e.text_field_state != previous_state
            || e.keyboard_options != previous_keyboard_options
        {
            if editable && (s.is_focused() || s.input_session_job.borrow().is_some()) {
                s.start_input_session(false);
            } else if !editable {
                s.dispose_input_session();
            }
        }

        if (e.enabled != previous_enabled
            || editable != previous_editable
            || e.keyboard_options.ime_action_or_default() != previous_keyboard_options.ime_action_or_default()
            || e.is_password != previous_is_password)
            && let Some(h) = s.handle()
        {
            h.invalidate_semantics();
        }

        if !Rc::ptr_eq(&e.text_field_selection_state, &previous_selection_state) {
            if let Some(p) = &self.pointer_input_node {
                p.borrow().reset_pointer_input_handler();
            }
            if s.handle().is_some_and(|h| h.is_attached()) {
                e.text_field_selection_state
                    .set_receive_content_configuration(Some(s.receive_content_configuration_provider()));
            }
            if s.handle().is_some_and(|h| h.is_attached())
                && s.is_focused()
                && s.toolbar_and_handles_visibility_observer_job.borrow().is_some()
            {
                if let Some(old) = s.toolbar_and_handles_visibility_observer_job.borrow_mut().take() {
                    old.cancel();
                }
                if let Some(scope) = s.handle().and_then(|h| h.coroutine_scope()) {
                    let job = scope
                        .launch(e.text_field_selection_state.clone().start_toolbar_and_handles_visibility_observer());
                    *s.toolbar_and_handles_visibility_observer_job.borrow_mut() = Some(job);
                }
            }
            let weak = Rc::downgrade(&s);
            e.text_field_selection_state.set_request_autofill_action(Some(Rc::new(move || {
                if let Some(s) = weak.upgrade() {
                    s.request_autofill();
                }
            })));
        }

        if e.interaction_source != previous_interaction_source {
            if let Some(p) = &self.pointer_input_node {
                p.borrow().reset_pointer_input_handler();
            }
            let focusable = s.focusable.borrow().clone();
            if s.focusable_delegated.get()
                && let Some(f) = focusable
            {
                f.borrow_mut().update(Some(e.interaction_source.clone()));
            }
        }

        if e.enabled != previous_enabled
            && let Some(cx) = s.cx()
        {
            if e.enabled {
                self.delegate_focusable(&cx);
            } else {
                self.undelegate_focusable(&cx);
            }
        }
    }
}

impl PlatformTextInputModifierNode for TextFieldDecoratorModifierNode {}

impl ModifierNode for TextFieldDecoratorModifierNode {
    node_capabilities!(
        draw,
        semantics,
        global_position_aware,
        pointer_input,
        key_input,
        focus_properties,
        layout_aware,
        observer
    );

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.shared.handle.borrow_mut() = Some(cx.handle());
        if self.shared.session_node.borrow().is_none() {
            let node = cx.delegate(TextInputSessionNode);
            *self.shared.session_node.borrow_mut() = Some(node);
        }
        if self.pointer_input_node.is_none() {
            let block = Self::pointer_input_block(&self.shared);
            self.pointer_input_node = Some(cx.delegate(SuspendingPointerInputModifierNode::new(block)));
        }
        if self.drag_and_drop_node.is_none() {
            let target = Rc::new(TextFieldDropTarget { shared: Rc::downgrade(&self.shared), enter: Cell::new(None) });
            // (Compose's `textFieldDragAndDropNode` with the decorator's callbacks, Android's actual: a drag is
            // taken if it is text, or anything under a content receiver)
            let hints = target.clone();
            let should_start = move |event: &DragAndDropEvent| {
                hints.hint_media_types().iter().any(|m| *m == MediaType::All || event.has_mime_type(m.representation()))
            };
            self.drag_and_drop_node = Some(cx.delegate(DragAndDropTargetModifierNode(should_start, target)));
        }
        self.shared.update_window_focus();
        let provider = self.shared.receive_content_configuration_provider();
        self.shared.selection_state().set_receive_content_configuration(Some(provider));
        if self.shared.enabled.get() {
            self.delegate_focusable(cx);
        }
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.shared.dispose_input_session();
        self.shared.toolbar_and_handles_visibility_observer_job.borrow_mut().take();
        self.shared.selection_state().set_receive_content_configuration(None);
    }
}

/// The field's drop target (Compose's `textFieldDragAndDropNode` with the decorator's callbacks, Android's actual —
/// Compose for desktop leaves it unimplemented, refusing every drag): it takes text, or under a content receiver
/// anything (the receiver's listener says what it takes); over the field the caret follows the drag; a drop's
/// content goes to the receivers first, what they leave of its plain text in place of the selection.
struct TextFieldDropTarget {
    shared: std::rc::Weak<DecoratorShared>,
    /// The enter emitted to the interaction source (Compose's `dragEnterEvent`).
    enter: Cell<Option<DragAndDropHoverInteraction::Enter>>,
}

impl TextFieldDropTarget {
    /// Compose's `emitDragExitEvent`.
    fn emit_drag_exit_event(&self, shared: &DecoratorShared) {
        if let Some(enter) = self.enter.take() {
            shared.interaction_source.borrow().try_emit(DragAndDropHoverInteraction::Exit::new(enter));
        }
    }

    /// The types the field takes (Compose's `hintMediaTypes`): any under a content receiver (its listener decides),
    /// else text.
    fn hint_media_types(&self) -> Vec<MediaType> {
        match self.shared.upgrade().and_then(|s| s.receive_content_configuration()) {
            Some(_) => vec![MediaType::All],
            None => vec![MediaType::Text],
        }
    }
}

impl DragAndDropTarget for TextFieldDropTarget {
    fn on_drop(&self, event: &DragAndDropEvent) -> bool {
        let Some(shared) = self.shared.upgrade() else { return false };
        let receive_content_configuration = shared.receive_content_configuration();
        if receive_content_configuration.is_some() {
            drag_and_drop_request_permission(event);
        }
        self.emit_drag_exit_event(&shared);
        shared.selection_state().clear_handle_dragging();
        let clip_entry = event.clip_entry().cloned().unwrap_or_default();
        let mut plain_text = clip_entry.read_plain_text();
        // (Under a receiver every drop is taken: the receiver's listener evaluates it.)
        if let Some(configuration) = receive_content_configuration {
            let clip_metadata = clip_entry.clip_metadata().clone();
            let transferable_content =
                TransferableContent::new(clip_entry, clip_metadata, TransferableContentSource::DragAndDrop, None);
            let remaining = configuration.receive_content_listener().on_receive(transferable_content);
            plain_text = remaining.and_then(|r| r.clip_entry().read_plain_text());
        }
        if let Some(text) = plain_text {
            shared.state().replace_selected_text(&text, false, TextFieldEditUndoBehavior::MergeIfPossible, true, false);
        }
        true
    }

    fn on_entered(&self, _event: &DragAndDropEvent) {
        let Some(shared) = self.shared.upgrade() else { return };
        let enter = DragAndDropHoverInteraction::Enter::new();
        shared.interaction_source.borrow().try_emit(enter);
        self.enter.set(Some(enter));
        // (The field isn't a receiver but behaves as one: the receivers around it hear the drag came in.)
        if let Some(configuration) = shared.receive_content_configuration() {
            configuration.receive_content_listener().on_drag_enter();
        }
    }

    fn on_moved(&self, event: &DragAndDropEvent) {
        let Some(shared) = self.shared.upgrade() else { return };
        let layout = shared.text_layout_state.borrow().clone();
        let position_on_text_field = layout.from_window_to_decoration(event.position_in_root);
        if let Some(cursor) = layout.get_offset_for_position(position_on_text_field, true) {
            shared.state().select_chars_in(TextRange::at(cursor));
        }
        shared.selection_state().update_handle_dragging(Handle::Cursor, position_on_text_field);
    }

    fn on_exited(&self, _event: &DragAndDropEvent) {
        let Some(shared) = self.shared.upgrade() else { return };
        self.emit_drag_exit_event(&shared);
        shared.selection_state().clear_handle_dragging();
        if let Some(configuration) = shared.receive_content_configuration() {
            configuration.receive_content_listener().on_drag_exit();
        }
    }

    fn on_ended(&self, _event: &DragAndDropEvent) {
        if let Some(shared) = self.shared.upgrade() {
            self.emit_drag_exit_event(&shared);
        }
    }
}

impl DrawModifierNode for TextFieldDecoratorModifierNode {
    /// The autofill highlight over the content (and over a Material background), once a service filled it.
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        scope.draw_content();
        if self.shared.autofill_highlight_on.get()
            && let Some(h) = self.shared.handle()
        {
            let brush = resolve_autofill_highlight(
                h.current_value_of(&LocalAutofillHighlightBrush),
                h.current_value_of(&LocalAutofillHighlightColor),
                autofill_highlight_color(),
            );
            if brush.solid_color().is_none_or(|c| c.is_specified()) {
                let size = scope.size();
                scope.draw_rect_with(
                    brush,
                    Offset::Zero,
                    size,
                    1.0,
                    kompoz_ui::draw::DrawStyle::Fill,
                    None,
                    kompoz_ui::draw::DefaultBlendMode,
                );
            }
        }
    }
}

impl SemanticsModifierNode for TextFieldDecoratorModifierNode {
    fn should_merge_descendant_semantics(&self) -> bool {
        true
    }

    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        let s = self.shared.clone();
        let state = s.state();
        let text = state.output_text();
        let selection = text.selection();
        config.set_input_text(AnnotatedString::new(state.untransformed_text().text()));
        config.set_editable_text(AnnotatedString::new(text.text()));
        config.set_text_selection_range(selection);
        config.set_text_composition_range(state.untransformed_composition());
        config.set_input_text_suggestion_state(InputTextSuggestionState {
            is_committed_by_input_method_editor: state.user_commit(),
            is_transliteration_suggestion_selected: state.suggestion_selected(),
        });
        if !s.enabled.get() {
            config.disabled();
        }
        if s.is_password.get() {
            config.password();
        }
        let editable = s.editable();
        config.set_is_editable(editable);

        // Autofill: the app sets the content type; a field holds text.
        config.set_content_data_type(ContentDataType::Text);
        config.set_fillable_data(FillableData::Text(text.text().to_string()));
        let fill = s.clone();
        config.on_fill_data(move |data: FillableData| {
            if !fill.editable() {
                return false;
            }
            if let Some(t) = data.text_value() {
                fill.state().replace_all(t);
            }
            fill.autofill_highlight_on.set(true);
            if let Some(scope) = fill.handle().and_then(|h| h.coroutine_scope()) {
                let observed = fill.clone();
                scope.launch(async move {
                    // The highlight goes at the text's next change.
                    let state = observed.state();
                    let mut first = true;
                    let mut done = false;
                    let highlight = observed.autofill_highlight_on.clone();
                    snapshot_flow(move || state.untransformed_text().text().to_string())
                        .collect(move |_| {
                            if std::mem::take(&mut first) || done {
                                return;
                            }
                            done = true;
                            highlight.set(false);
                        })
                        .await;
                });
            }
            true
        });
        match s.keyboard_options.borrow().keyboard_type {
            KeyboardType::Email => config.set_content_type(ContentType::EmailAddress()),
            KeyboardType::Password | KeyboardType::NumberPassword => config.set_content_type(ContentType::Password()),
            KeyboardType::Phone => config.set_content_type(ContentType::PhoneNumber()),
            _ => {}
        }

        let layout_state = s.text_layout_state.borrow().clone();
        config.get_text_layout_result(move |results| match layout_state.layout_result() {
            Some(result) => {
                results.push(result);
                true
            }
            None => false,
        });
        if editable {
            let set = s.clone();
            config.set_text_action(move |new_text: AnnotatedString| {
                if !set.editable() {
                    return false;
                }
                set.state().replace_all(new_text.text());
                true
            });
            let insert = s.clone();
            config.insert_text_at_cursor(move |new_text: AnnotatedString| {
                if !insert.editable() {
                    return false;
                }
                // The composition is finished first (a focused field's input method may have one).
                insert.state().replace_selected_text(
                    new_text.text(),
                    true,
                    super::undo::TextFieldEditUndoBehavior::MergeIfPossible,
                    true,
                    false,
                );
                true
            });
        }
        let select = s.clone();
        config.set_selection(move |start: i32, end: i32, relative_to_original: bool| {
            let state = select.state();
            // Relative to the original text (traversal) or to the shown one.
            let text = if relative_to_original { state.untransformed_text() } else { state.visual_text() };
            let current = text.selection();
            if !select.enabled.get() || start.min(end) < 0 || start.max(end) as usize > text.len() {
                return false;
            }
            let (start, end) = (start as usize, end as usize);
            if start == current.start && end == current.end {
                return true;
            }
            let range = TextRange::new(start, end);
            let selection_state = select.selection_state();
            if relative_to_original || start == end {
                selection_state.update_text_toolbar_state(TextToolbarState::None);
            } else {
                selection_state.update_text_toolbar_state(TextToolbarState::Selection);
            }
            if relative_to_original {
                state.select_untransformed_chars_in(range);
            } else {
                state.select_chars_in(range);
            }
            true
        });
        let effective_ime_action = s.keyboard_options.borrow().ime_action_or_default();
        let ime = s.clone();
        config.on_ime_action(effective_ime_action, move || {
            ime.on_ime_action_performed(effective_ime_action);
            true
        });
        let click = s.clone();
        config.on_click(move || {
            // Given even when disabled.
            if !click.is_focused() {
                click.request_focus();
            } else if !click.read_only.get() {
                click.keyboard_controller().show();
            }
            true
        });
        let long_click = s.clone();
        config.on_long_click(move || {
            if !long_click.is_focused() {
                long_click.request_focus();
            }
            long_click.selection_state().update_text_toolbar_state(TextToolbarState::Selection);
            true
        });
        if !selection.collapsed() && !s.is_password.get() {
            let copy = s.selection_state();
            config.copy_text(move || {
                copy.copy(true);
                true
            });
            if s.enabled.get() && !s.read_only.get() {
                let cut = s.selection_state();
                config.cut_text(move || {
                    cut.cut();
                    true
                });
            }
        }
        if editable {
            let paste = s.selection_state();
            config.paste_text(move || {
                paste.paste(false);
                true
            });
        }
        if let Some(filter) = s.filter.borrow().as_ref() {
            filter.apply_semantics(config);
        }
        if s.enabled.get()
            && s.focusable_delegated.get()
            && let Some(f) = s.focusable.borrow().as_ref()
        {
            apply_focusable_semantics(f.borrow().shared(), config);
        }
    }
}

impl FocusPropertiesModifierNode for TextFieldDecoratorModifierNode {
    fn apply_focus_properties(&mut self, properties: &mut FocusProperties) {
        properties.focus_rect = self.shared.selection_state().get_focus_rect();
    }
}

impl GlobalPositionAwareModifierNode for TextFieldDecoratorModifierNode {
    fn on_globally_positioned(&mut self, coordinates: &LayoutCoordinates) {
        self.shared.text_layout_state.borrow().set_decorator_node_coordinates(Some(coordinates.clone()));
        if self.shared.enabled.get()
            && let Some(f) = self.shared.focusable.borrow().as_ref()
        {
            f.borrow_mut().on_globally_positioned(coordinates);
        }
    }
}

impl PointerInputModifierNode for TextFieldDecoratorModifierNode {
    fn on_pointer_event(&mut self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        if let Some(p) = &self.pointer_input_node {
            p.borrow_mut().on_pointer_event(event, pass, bounds);
        }
    }

    fn on_cancel_pointer_input(&mut self) {
        if let Some(p) = &self.pointer_input_node {
            p.borrow_mut().on_cancel_pointer_input();
        }
    }
}

impl KeyInputModifierNode for TextFieldDecoratorModifierNode {
    fn on_pre_key_event(&mut self, event: &KeyEvent) -> bool {
        let state = self.shared.state();
        let selection_state = self.shared.selection_state();
        self.text_field_key_event_handler.on_pre_key_event(event, &state, || selection_state.deselect())
    }

    fn on_key_event(&mut self, event: &KeyEvent) -> bool {
        let s = self.shared.clone();
        // Typed through the input session (KompozR's desktop typing; Compose desktop's typed key events reset it).
        if self.selection_after_key_event != Some(s.state().visual_text().selection()) {
            self.text_field_key_event_handler.reset_cached_x();
        }
        let selection_state = s.selection_state();
        let clipboard_handler: ClipboardKeyCommandsHandler =
            Rc::new(move |command, is_from_hardware_source| match command {
                KeyCommand::COPY => selection_state.copy(false),
                KeyCommand::CUT => selection_state.cut(),
                KeyCommand::PASTE => selection_state.paste(is_from_hardware_source),
                _ => {}
            });
        let state = s.state();
        let layout = s.text_layout_state.borrow().clone();
        let keyboard = s.keyboard_controller();
        let submit = s.clone();
        let consumed = self.text_field_key_event_handler.on_key_event(
            event,
            &state,
            &layout,
            &clipboard_handler,
            &keyboard,
            s.editable(),
            s.single_line.get(),
            move || {
                let action = submit.keyboard_options.borrow().ime_action_or_default();
                submit.on_ime_action_performed(action)
            },
        );
        self.selection_after_key_event = Some(s.state().visual_text().selection());
        consumed
    }
}

impl ObserverModifierNode for TextFieldDecoratorModifierNode {
    fn on_observed_reads_changed(&mut self) {
        let shared = self.shared.clone();
        kompoz_ui::input::when_node_released(move || shared.update_window_focus());
    }
}

impl LayoutAwareModifierNode for TextFieldDecoratorModifierNode {
    fn on_placed(&mut self, coordinates: &LayoutCoordinates) {
        if let Some(d) = &self.drag_and_drop_node
            && let Some(aware) = d.borrow_mut().as_layout_aware()
        {
            aware.on_placed(coordinates);
        }
    }

    fn on_remeasured(&mut self, size: IntSize) {
        if let Some(d) = &self.drag_and_drop_node
            && let Some(aware) = d.borrow_mut().as_layout_aware()
        {
            aware.on_remeasured(size);
        }
    }
}

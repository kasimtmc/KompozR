//! The value-based field's text input service (Compose 1.12's LegacyPlatformTextInputServiceAdapter, its skiko
//! actual, and LegacyAdaptingPlatformTextInputModifierNode): the field's own [`PlatformTextInputService`] — what
//! its `TextInputService` starts its sessions with — going through the field's modifier node into the platform
//! text input API (`establish_text_input_session`, a [`PlatformTextInputMethodRequest`] reading the value the field
//! last told it), so interceptors and the one-session-at-a-time rule apply to it as to the state-based field.

use crate::text::core_text_field::{LegacyTextFieldState, ObservedCell};
use crate::text::selection::text_field_selection_manager::TextFieldSelectionManager;
use kompoz_coroutines::{CoroutineStart, Job};
use kompoz_graphics::{Matrix, Offset, Rect};
use kompoz_runtime::named_args;
use kompoz_text::input::{EditCommand, OffsetMapping, TextFieldValue};
use kompoz_text::{TextLayoutResult, TextRange};
use kompoz_ui::layout::{GlobalPositionAwareModifierNode, LayoutCoordinates};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::platform::{
    ImeAction, ImeOptions, LocalSoftwareKeyboardController, LocalViewConfiguration, SoftwareKeyboardController,
    ViewConfiguration,
};
use kompoz_ui::platform_text_input::{
    PlatformTextInputMethodRequest, PlatformTextInputModifierNode, PlatformTextInputSessionScope, SessionFuture,
    TextEditingScope, TextEditorState, establish_text_input_session,
};
use kompoz_ui::text_input::PlatformTextInputService;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// What the adapter knows of the node it goes through (Compose's LegacyPlatformTextInputNode).
pub trait LegacyPlatformTextInputNode {
    /// The software keyboard's controller where the node is.
    fn software_keyboard_controller(&self) -> Option<SoftwareKeyboardController>;
    /// The node's coordinates (a state).
    fn layout_coordinates(&self) -> Option<LayoutCoordinates>;
    /// The field's state.
    fn legacy_text_field_state(&self) -> Option<Rc<LegacyTextFieldState>>;
    /// The field's selection manager.
    fn text_field_selection_manager(&self) -> Option<Rc<TextFieldSelectionManager>>;
    /// The view configuration where the node is.
    fn view_configuration(&self) -> ViewConfiguration;
    /// Launches `block` as a text input session of the node, in its coroutine scope (none while detached).
    fn launch_text_input_session(
        &self,
        block: Box<dyn FnOnce(PlatformTextInputSessionScope) -> SessionFuture>,
    ) -> Option<Job>;
}

/// A value-based field's text input service (Compose's LegacyPlatformTextInputServiceAdapter, the skiko one): it
/// works while exactly one [`legacy_text_input_adapter`] modifier holding it is attached; a started input launches
/// a session of that node, its request reading the value, layout and rects the field last told.
pub struct LegacyPlatformTextInputServiceAdapter {
    text_input_modifier_node: RefCell<Option<Weak<dyn LegacyPlatformTextInputNode>>>,
    job: RefCell<Option<Job>>,
    text_field_value: Rc<ObservedCell<TextFieldValue>>,
    text_layout_result: Rc<ObservedCell<Option<TextLayoutResult>>>,
    focused_rect_in_root: Rc<ObservedCell<Option<Rect>>>,
    text_field_rect_in_root: Rc<ObservedCell<Option<Rect>>>,
    text_clipping_rect_in_root: Rc<ObservedCell<Option<Rect>>>,
    unclipped_text_offset_in_root: Rc<ObservedCell<Option<Offset>>>,
}

impl LegacyPlatformTextInputServiceAdapter {
    /// Compose's `createLegacyPlatformTextInputServiceAdapter`.
    pub fn new() -> Rc<LegacyPlatformTextInputServiceAdapter> {
        Rc::new(LegacyPlatformTextInputServiceAdapter {
            text_input_modifier_node: RefCell::new(None),
            job: RefCell::new(None),
            text_field_value: Rc::new(ObservedCell::new(TextFieldValue::default())),
            text_layout_result: Rc::new(ObservedCell::new(None)),
            focused_rect_in_root: Rc::new(ObservedCell::new(None)),
            text_field_rect_in_root: Rc::new(ObservedCell::new(None)),
            text_clipping_rect_in_root: Rc::new(ObservedCell::new(None)),
            unclipped_text_offset_in_root: Rc::new(ObservedCell::new(None)),
        })
    }

    fn node(&self) -> Option<Rc<dyn LegacyPlatformTextInputNode>> {
        self.text_input_modifier_node.borrow().as_ref().and_then(|w| w.upgrade())
    }

    /// Its node attached (Compose's `registerModifier`): panics if another one is.
    pub fn register_modifier(&self, node: Weak<dyn LegacyPlatformTextInputNode>) {
        assert!(self.node().is_none(), "Expected textInputModifierNode to be null");
        *self.text_input_modifier_node.borrow_mut() = Some(node);
    }

    /// Its node detached (Compose's `unregisterModifier`): panics if it is another one.
    pub fn unregister_modifier(&self, node: &Weak<dyn LegacyPlatformTextInputNode>) {
        let same = self.text_input_modifier_node.borrow().as_ref().is_some_and(|n| Weak::ptr_eq(n, node));
        assert!(same, "Expected textInputModifierNode to be the node unregistering");
        *self.text_input_modifier_node.borrow_mut() = None;
    }

    /// Stylus handwriting (Compose's `startStylusHandwriting`: nothing on the desktop).
    pub fn start_stylus_handwriting(&self) {}

    /// The request a started input makes (Compose's skiko `makeRequest`).
    fn make_request(
        &self,
        ime_options: &ImeOptions,
        on_edit_command: Rc<dyn Fn(&[EditCommand])>,
        on_ime_action_performed: Rc<dyn Fn(ImeAction)>,
    ) -> PlatformTextInputMethodRequest {
        let value = self.text_field_value.clone();
        let state_value = self.text_field_value.clone();
        let edit_command = on_edit_command.clone();
        let edit_text: Rc<dyn Fn(&mut dyn FnMut(&mut dyn TextEditingScope))> = Rc::new(move |block| {
            let mut scope = CommandsScope(Vec::new());
            block(&mut scope);
            edit_command(&scope.0);
        });
        let (layout, focused, field, clipping, unclipped) = (
            self.text_layout_result.clone(),
            self.focused_rect_in_root.clone(),
            self.text_field_rect_in_root.clone(),
            self.text_clipping_rect_in_root.clone(),
            self.unclipped_text_offset_in_root.clone(),
        );
        PlatformTextInputMethodRequest {
            value: Rc::new(move || value.get()),
            state: Rc::new(AdapterEditorState(state_value)),
            ime_options: ime_options.clone(),
            on_edit_command,
            on_ime_action: Some(on_ime_action_performed),
            text_layout_result: Rc::new(move || layout.get()),
            focused_rect_in_root: Rc::new(move || focused.get()),
            text_field_rect_in_root: Rc::new(move || field.get()),
            text_clipping_rect_in_root: Rc::new(move || clipping.get()),
            unclipped_text_offset_in_root: Rc::new(move || unclipped.get()),
            edit_text,
            // (the legacy field takes text only: Compose's takes no content)
            content_mime_types: Vec::new(),
            on_commit_content: None,
        }
    }
}

impl PlatformTextInputService for LegacyPlatformTextInputServiceAdapter {
    fn start_input(
        &self,
        value: &TextFieldValue,
        ime_options: &ImeOptions,
        on_edit_command: Rc<dyn Fn(&[EditCommand])>,
        on_ime_action_performed: Rc<dyn Fn(ImeAction)>,
    ) {
        self.text_field_value.set(value.clone());
        let Some(node) = self.node() else { return };
        let request = self.make_request(ime_options, on_edit_command, on_ime_action_performed);
        let job = node.launch_text_input_session(Box::new(move |scope: PlatformTextInputSessionScope| {
            scope.start_input_method(request)
        }));
        *self.job.borrow_mut() = job;
    }

    fn stop_input(&self) {
        if let Some(job) = self.job.borrow_mut().take() {
            job.cancel();
        }
        self.text_field_value.set(TextFieldValue::default());
    }

    fn show_software_keyboard(&self) {
        if let Some(keyboard) = self.node().and_then(|n| n.software_keyboard_controller()) {
            keyboard.show();
        }
    }

    fn hide_software_keyboard(&self) {
        if let Some(keyboard) = self.node().and_then(|n| n.software_keyboard_controller()) {
            keyboard.hide();
        }
    }

    fn update_state(&self, _old_value: Option<&TextFieldValue>, new_value: &TextFieldValue) {
        self.text_field_value.set(new_value.clone());
    }

    fn update_text_layout_result(
        &self,
        text_field_value: &TextFieldValue,
        offset_mapping: &OffsetMapping,
        text_layout_result: &TextLayoutResult,
        text_field_to_root_transform: &dyn Fn(&mut Matrix),
        inner_text_field_bounds: Rect,
        decoration_box_bounds: Rect,
    ) {
        self.text_field_value.set(text_field_value.clone());
        self.text_layout_result.set(Some(text_layout_result.clone()));
        let mut matrix = Matrix::new();
        text_field_to_root_transform(&mut matrix);
        self.text_field_rect_in_root.set(Some(matrix.map_rect(decoration_box_bounds)));
        let clipping = matrix.map_rect(inner_text_field_bounds);
        self.text_clipping_rect_in_root.set(Some(clipping));
        let cursor_offset = offset_mapping.original_to_transformed(text_field_value.selection().max());
        self.focused_rect_in_root.set(Some(matrix.map_rect(text_layout_result.get_cursor_rect(cursor_offset))));
        self.unclipped_text_offset_in_root.set(Some(clipping.top_left() - inner_text_field_bounds.top_left()));
    }
}

/// The editor's state as the input method reads it: the value last told (Compose's skiko adapter's
/// TextEditorState).
struct AdapterEditorState(Rc<ObservedCell<TextFieldValue>>);

impl TextEditorState for AdapterEditorState {
    fn text(&self) -> String {
        self.0.get().text().to_string()
    }
    fn selection(&self) -> TextRange {
        self.0.get().selection()
    }
    fn composition(&self) -> Option<TextRange> {
        self.0.get().composition()
    }
}

/// What an input method's edit block does, as commands (Compose's skiko `TextEditingScope(commands)`).
struct CommandsScope(Vec<EditCommand>);

impl TextEditingScope for CommandsScope {
    fn delete_surrounding_text_in_code_points(&mut self, before: usize, after: usize) {
        // (Compose's skiko adapter makes a DeleteSurroundingTextCommand of these lengths, which count UTF-16 units
        // there; they are code points: the code point command)
        self.0.push(EditCommand::DeleteSurroundingTextInCodePoints { before, after });
    }
    fn commit_text(&mut self, text: &str, new_cursor_position: i32) {
        self.0.push(EditCommand::CommitText { text: text.to_string(), new_cursor_position });
    }
    fn set_composing_text(&mut self, text: &str, new_cursor_position: i32) {
        self.0.push(EditCommand::SetComposingText { text: text.to_string(), new_cursor_position });
    }
    fn finish_composing_text(&mut self) {
        self.0.push(EditCommand::FinishComposingText);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The modifier

/// Connects `service_adapter` to the platform text input API through this node (Compose's
/// `legacyTextInputAdapter`): the adapter works while it is attached.
pub fn legacy_text_input_adapter(
    modifier: Modifier,
    service_adapter: &Rc<LegacyPlatformTextInputServiceAdapter>,
    legacy_text_field_state: &Rc<LegacyTextFieldState>,
    text_field_selection_manager: &Rc<TextFieldSelectionManager>,
) -> Modifier {
    modifier.then_element(LegacyAdaptingPlatformTextInputModifier {
        service_adapter: service_adapter.clone(),
        legacy_text_field_state: legacy_text_field_state.clone(),
        text_field_selection_manager: text_field_selection_manager.clone(),
    })
}

/// The adapter modifier's element (Compose's LegacyAdaptingPlatformTextInputModifier, a data class).
#[derive(Clone)]
struct LegacyAdaptingPlatformTextInputModifier {
    service_adapter: Rc<LegacyPlatformTextInputServiceAdapter>,
    legacy_text_field_state: Rc<LegacyTextFieldState>,
    text_field_selection_manager: Rc<TextFieldSelectionManager>,
}

impl PartialEq for LegacyAdaptingPlatformTextInputModifier {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.service_adapter, &other.service_adapter)
            && Rc::ptr_eq(&self.legacy_text_field_state, &other.legacy_text_field_state)
            && Rc::ptr_eq(&self.text_field_selection_manager, &other.text_field_selection_manager)
    }
}

impl ModifierNodeElement for LegacyAdaptingPlatformTextInputModifier {
    type Node = LegacyAdaptingPlatformTextInputModifierNode;
    fn create(&self) -> LegacyAdaptingPlatformTextInputModifierNode {
        LegacyAdaptingPlatformTextInputModifierNode {
            service_adapter: self.service_adapter.clone(),
            shared: Rc::new(NodeShared {
                legacy_text_field_state: RefCell::new(self.legacy_text_field_state.clone()),
                text_field_selection_manager: RefCell::new(self.text_field_selection_manager.clone()),
                layout_coordinates: ObservedCell::new(None),
                handle: RefCell::new(None),
                session_node: RefCell::new(None),
            }),
        }
    }
    fn update(&self, node: &mut LegacyAdaptingPlatformTextInputModifierNode) {
        node.set_service_adapter(self.service_adapter.clone());
        *node.shared.legacy_text_field_state.borrow_mut() = self.legacy_text_field_state.clone();
        *node.shared.text_field_selection_manager.borrow_mut() = self.text_field_selection_manager.clone();
    }
    fn name(&self) -> &'static str {
        "legacyTextInputAdapter"
    }
}

/// The node sessions are established for (Compose's PlatformTextInputModifierNode, the adapting node itself
/// there): a delegate of it, so a session can be launched from the adapter as from the node's own methods.
struct TextInputSessionNode;

impl ModifierNode for TextInputSessionNode {}

impl PlatformTextInputModifierNode for TextInputSessionNode {}

/// What the node and the adapter share (the node's fields in Compose).
struct NodeShared {
    legacy_text_field_state: RefCell<Rc<LegacyTextFieldState>>,
    text_field_selection_manager: RefCell<Rc<TextFieldSelectionManager>>,
    layout_coordinates: ObservedCell<Option<LayoutCoordinates>>,
    handle: RefCell<Option<NodeHandle>>,
    session_node: RefCell<Option<DelegatedNode<TextInputSessionNode>>>,
}

impl LegacyPlatformTextInputNode for NodeShared {
    fn software_keyboard_controller(&self) -> Option<SoftwareKeyboardController> {
        self.handle.borrow().as_ref().map(|h| h.current_value_of(&LocalSoftwareKeyboardController))
    }

    fn layout_coordinates(&self) -> Option<LayoutCoordinates> {
        self.layout_coordinates.get()
    }

    fn legacy_text_field_state(&self) -> Option<Rc<LegacyTextFieldState>> {
        Some(self.legacy_text_field_state.borrow().clone())
    }

    fn text_field_selection_manager(&self) -> Option<Rc<TextFieldSelectionManager>> {
        Some(self.text_field_selection_manager.borrow().clone())
    }

    fn view_configuration(&self) -> ViewConfiguration {
        self.handle
            .borrow()
            .as_ref()
            .map_or_else(ViewConfiguration::default, |h| h.current_value_of(&LocalViewConfiguration))
    }

    fn launch_text_input_session(
        &self,
        block: Box<dyn FnOnce(PlatformTextInputSessionScope) -> SessionFuture>,
    ) -> Option<Job> {
        let cx = self.handle.borrow().as_ref().and_then(|h| h.cx())?;
        if !cx.is_attached() {
            return None;
        }
        let session_node = self.session_node.borrow().clone()?;
        let session = establish_text_input_session(&*session_node.borrow(), &session_node.cx(), block);
        Some(named_args!(
            cx.coroutine_scope().launch(start = CoroutineStart::UNDISPATCHED, async move { match session.await {} })
        ))
    }
}

/// The adapting node (Compose's LegacyAdaptingPlatformTextInputModifierNode): the adapter's node while attached,
/// its coordinates followed.
pub struct LegacyAdaptingPlatformTextInputModifierNode {
    service_adapter: Rc<LegacyPlatformTextInputServiceAdapter>,
    shared: Rc<NodeShared>,
}

impl LegacyAdaptingPlatformTextInputModifierNode {
    fn as_node(&self) -> Weak<dyn LegacyPlatformTextInputNode> {
        let rc: Rc<dyn LegacyPlatformTextInputNode> = self.shared.clone();
        Rc::downgrade(&rc)
    }

    fn is_attached(&self) -> bool {
        self.shared.handle.borrow().as_ref().is_some_and(|h| h.is_attached())
    }

    /// Takes another adapter (Compose's `setServiceAdapter`): the old one's input stopped and the node moved over.
    fn set_service_adapter(&mut self, service_adapter: Rc<LegacyPlatformTextInputServiceAdapter>) {
        if Rc::ptr_eq(&self.service_adapter, &service_adapter) {
            return;
        }
        if self.is_attached() {
            self.service_adapter.stop_input();
            self.service_adapter.unregister_modifier(&self.as_node());
        }
        self.service_adapter = service_adapter;
        if self.is_attached() {
            self.service_adapter.register_modifier(self.as_node());
        }
    }
}

impl ModifierNode for LegacyAdaptingPlatformTextInputModifierNode {
    node_capabilities!(global_position_aware);

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.shared.handle.borrow_mut() = Some(cx.handle());
        if self.shared.session_node.borrow().is_none() {
            let node = cx.delegate(TextInputSessionNode);
            *self.shared.session_node.borrow_mut() = Some(node);
        }
        self.service_adapter.register_modifier(self.as_node());
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.service_adapter.unregister_modifier(&self.as_node());
    }
}

impl PlatformTextInputModifierNode for LegacyAdaptingPlatformTextInputModifierNode {}

impl GlobalPositionAwareModifierNode for LegacyAdaptingPlatformTextInputModifierNode {
    fn on_globally_positioned(&mut self, coordinates: &LayoutCoordinates) {
        self.shared.layout_coordinates.set(Some(coordinates.clone()));
    }
}

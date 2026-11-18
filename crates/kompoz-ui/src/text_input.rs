//! Text input from the platform's input method: the host's text input service (Compose's `TextInputService` and
//! `TextInputSession` of `androidx.compose.ui.text.input`), what both Compose APIs go through here:
//!
//! - the legacy one: a focused editor starts a session with its text, selection and options
//!   ([`TextInputService::start_input`]) and gets what the input method does as [`EditCommand`]s and its action
//!   key. A service may be made over a [`PlatformTextInputService`] of its own
//!   ([`TextInputService::with_platform_service`], Compose's `TextInputService(platformTextInputService)`): the
//!   value-based text field's is, its service going through its modifier node into the API below (Compose 1.12's
//!   LegacyPlatformTextInputServiceAdapter);
//! - the one of Compose 1.7+ ([`platform_text_input`](crate::platform_text_input):
//!   `PlatformTextInputModifierNode`, `establish_text_input_session`, `start_input_method`): a session made of a
//!   [`PlatformTextInputMethodRequest`](crate::platform_text_input::PlatformTextInputMethodRequest), whose editor
//!   is read again after the commands it took and whenever what it holds changes (Compose desktop's
//!   DesktopTextInputService2 reads its request's state).
//!
//! One session is open at a time, a new one closing the one before; the platform is told the session's state
//! through its callbacks ([`PlatformCallbacks::text_input`](crate::platform::PlatformCallbacks::text_input) and
//! [`text_input_changed`](crate::platform::PlatformCallbacks::text_input_changed)); what it receives it hands to the
//! host ([`KompozHost::text_input`](crate::KompozHost::text_input), `set_composing_text`, `send_edit_commands`,
//! `ime_action`), which hands it to the session open now.
//!
//! Offsets are UTF-8 byte offsets in the text, as [`TextEditState`]'s and kompoz-text's (Compose counts UTF-16
//! units): what a command gives is clamped to the text and to whole characters (kompoz-text's
//! [`EditingBuffer`]). A platform speaking UTF-16 or code points converts at its boundary (text-input-v3 and XIM
//! speak UTF-8 bytes already).

use crate::platform::{ImeAction, ImeOptions, PlatformHandle, TextEditState};
use kompoz_graphics::{Matrix, Rect};
use kompoz_runtime::KompozitionLocal;
use kompoz_text::input::OffsetMapping;
use kompoz_text::{TextLayoutResult, TextRange};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub use kompoz_text::input::{EditCommand, EditProcessor, EditingBuffer, TextFieldValue};

/// What a [`TextInputService`] made over it gives its sessions to (Compose's PlatformTextInputService): the
/// host's service is the platform's own; a value-based field makes one of its own going through its modifier node
/// (Compose's LegacyPlatformTextInputServiceAdapter).
pub trait PlatformTextInputService {
    /// An editor holding `value` starts taking input (Compose's `startInput`): what the input method does to
    /// `on_edit_command`, its action key to `on_ime_action_performed`.
    fn start_input(
        &self,
        value: &TextFieldValue,
        ime_options: &ImeOptions,
        on_edit_command: Rc<dyn Fn(&[EditCommand])>,
        on_ime_action_performed: Rc<dyn Fn(ImeAction)>,
    );
    /// Input starts without an editor (Compose's `startInput()`).
    fn start_input_without_editor(&self) {}
    /// Input stops (Compose's `stopInput`).
    fn stop_input(&self);
    /// Shows the software keyboard (Compose's `showSoftwareKeyboard`).
    fn show_software_keyboard(&self);
    /// Hides the software keyboard (Compose's `hideSoftwareKeyboard`).
    fn hide_software_keyboard(&self);
    /// The editor's value changed from `old_value` to `new_value` (Compose's `updateState`).
    fn update_state(&self, old_value: Option<&TextFieldValue>, new_value: &TextFieldValue);
    /// The editor's caret is at `rect` in the root (Compose's `notifyFocusedRect`).
    fn notify_focused_rect(&self, rect: Rect) {
        let _ = rect;
    }
    /// The editor's text was laid out again or moved (Compose's `updateTextLayoutResult`): its value, how its shown
    /// text maps to it, the layout, what transforms the inner field's coordinates to the root's, the inner field's
    /// visible bounds and the decoration box's, in the inner field's coordinates.
    fn update_text_layout_result(
        &self,
        text_field_value: &TextFieldValue,
        offset_mapping: &OffsetMapping,
        text_layout_result: &TextLayoutResult,
        text_field_to_root_transform: &dyn Fn(&mut Matrix),
        inner_text_field_bounds: Rect,
        decoration_box_bounds: Rect,
    ) {
        let _ = (text_field_value, offset_mapping, text_layout_result, text_field_to_root_transform);
        let _ = (inner_text_field_bounds, decoration_box_bounds);
    }
}

impl TextEditState {
    /// The editing buffer holding it (its selection in order).
    pub fn to_editing_buffer(&self) -> EditingBuffer {
        let len = self.text.len();
        let selection = TextRange::new(self.selection_start.min(len), self.selection_end.min(len));
        let mut buffer = EditingBuffer::from_text(&self.text, selection);
        if let Some((s, e)) = self.composition {
            let (s, e) = (s.min(e).min(len), s.max(e).min(len));
            if s < e {
                buffer.set_composition(s, e);
            }
        }
        buffer
    }

    /// What `buffer` holds.
    pub fn from_editing_buffer(buffer: &EditingBuffer) -> TextEditState {
        TextEditState {
            text: buffer.text().to_string(),
            selection_start: buffer.selection_start(),
            selection_end: buffer.selection_end(),
            composition: buffer.composition().map(|c| (c.start, c.end)),
        }
    }

    /// Applies `command` to it, as Compose's command applies to its EditingBuffer (the selection comes out in
    /// order).
    pub fn apply(&mut self, command: &EditCommand) {
        let mut buffer = self.to_editing_buffer();
        command.apply_to(&mut buffer);
        *self = TextEditState::from_editing_buffer(&buffer);
    }

    /// An editor's value as the input method sees it.
    pub fn from_value(value: &TextFieldValue) -> TextEditState {
        TextEditState {
            text: value.text().to_string(),
            selection_start: value.selection().start,
            selection_end: value.selection().end,
            composition: value.composition().map(|c| (c.min(), c.max())),
        }
    }

    /// It as an editor's value.
    pub fn to_value(&self) -> TextFieldValue {
        TextFieldValue::from_text(
            &self.text,
            TextRange::new(self.selection_start, self.selection_end),
            self.composition.map(|(s, e)| TextRange::new(s, e)),
        )
    }
}

/// An open session's state.
struct SessionState {
    on_edit_command: Rc<dyn Fn(&[EditCommand])>,
    on_ime_action: Rc<dyn Fn(ImeAction)>,
    /// The text as the input method sees it.
    state: RefCell<TextEditState>,
    options: ImeOptions,
    /// Where the editor is in the window (px), as last told.
    area: Cell<Rect>,
    open: Cell<bool>,
    /// What the editor holds now, read after it took commands (a [`start_input_method`](
    /// TextInputService::start_input_method) session's request): where it differs from what the input method
    /// sees (it refused or changed what came), the input method is told.
    value: Option<Rc<dyn Fn() -> TextFieldValue>>,
    /// Counted up as the input method's view moves with its commands (what follows the editor's value reads it,
    /// so that the editor going back to a value it had is seen as a change from what the input method sees).
    version: kompoz_runtime::MutableState<u64>,
}

struct ServiceState {
    platform: PlatformHandle,
    session: RefCell<Option<Rc<SessionState>>>,
    /// The sessions of the 1.7+ API (Compose's owner `textInputSession` mutex).
    text_input_sessions: crate::platform_text_input::SessionMutex,
    /// What its sessions go to when it was made over one (Compose's TextInputService(platformTextInputService)):
    /// then the platform's callbacks aren't told, the service is.
    platform_service: Option<Rc<dyn PlatformTextInputService>>,
}

/// What a focused editor starts its input sessions with (Compose's TextInputService and the owner's
/// `textInputSession`): one session is open at a time, a new one closing the one before; the platform is told as
/// they open and close. Provided at the root by the host ([`LocalTextInputService`]). Cheap to clone; equal when
/// the same.
#[derive(Clone)]
pub struct TextInputService(Rc<ServiceState>);

impl TextInputService {
    /// One telling `platform`'s callbacks.
    pub fn new(platform: PlatformHandle) -> TextInputService {
        TextInputService(Rc::new(ServiceState {
            platform,
            session: RefCell::new(None),
            text_input_sessions: crate::platform_text_input::SessionMutex::new(),
            platform_service: None,
        }))
    }

    /// One whose sessions go to `platform_text_input_service` (Compose's `TextInputService(platformTextInputService)`):
    /// a session started is the service's input started, its updates the service's, its end the service's input
    /// stopped; one session is current at a time.
    pub fn with_platform_service(platform_text_input_service: Rc<dyn PlatformTextInputService>) -> TextInputService {
        TextInputService(Rc::new(ServiceState {
            platform: PlatformHandle::default(),
            session: RefCell::new(None),
            text_input_sessions: crate::platform_text_input::SessionMutex::new(),
            platform_service: Some(platform_text_input_service),
        }))
    }

    /// The service its sessions go to, if it was made over one.
    fn platform_service(&self) -> Option<Rc<dyn PlatformTextInputService>> {
        self.0.platform_service.clone()
    }

    /// Opens a session for an editor holding `value` (Compose's `startInput`): what the input method does comes to
    /// `on_edit_command`, its action key to `on_ime_action`; the platform shows its input method for it.
    pub fn start_input(
        &self,
        value: TextEditState,
        options: ImeOptions,
        on_edit_command: impl Fn(&[EditCommand]) + 'static,
        on_ime_action: impl Fn(ImeAction) + 'static,
    ) -> TextInputSession {
        self.open(value, options, Rc::new(on_edit_command), Rc::new(on_ime_action), None)
    }

    /// Opens a session whose editor's value `value` reads (the 1.7+ API's request): the input method is told what
    /// it holds after each batch of commands, where that differs from what the commands would have left.
    pub(crate) fn start_input_reading(
        &self,
        options: ImeOptions,
        value: Rc<dyn Fn() -> TextFieldValue>,
        on_edit_command: Rc<dyn Fn(&[EditCommand])>,
        on_ime_action: Rc<dyn Fn(ImeAction)>,
    ) -> TextInputSession {
        let state = kompoz_runtime::without_read_observation(|| TextEditState::from_value(&value()));
        self.open(state, options, on_edit_command, on_ime_action, Some(value))
    }

    fn open(
        &self,
        value: TextEditState,
        options: ImeOptions,
        on_edit_command: Rc<dyn Fn(&[EditCommand])>,
        on_ime_action: Rc<dyn Fn(ImeAction)>,
        reads: Option<Rc<dyn Fn() -> TextFieldValue>>,
    ) -> TextInputSession {
        if let Some(service) = self.platform_service() {
            service.start_input(&value.to_value(), &options, on_edit_command.clone(), on_ime_action.clone());
        }
        let session = Rc::new(SessionState {
            on_edit_command,
            on_ime_action,
            state: RefCell::new(value),
            options,
            area: Cell::new(Rect::Zero),
            open: Cell::new(true),
            value: reads,
            version: kompoz_runtime::mutable_state_of(0),
        });
        let before = self.0.session.replace(Some(session.clone()));
        if self.0.platform_service.is_some() {
            // (Compose's: the current session replaced, the service's input not stopped)
            if let Some(before) = before {
                before.open.set(false);
            }
            return TextInputSession { service: self.clone(), session };
        }
        if let Some(before) = before {
            // (The platform hears the one before close, then this one open: another field's typing begins.)
            before.open.set(false);
            self.tell(&before, false);
        }
        self.tell(&session, true);
        TextInputSession { service: self.clone(), session }
    }

    /// Closes `session` if it is open (Compose's `stopInput`): the platform hides its input method.
    pub fn stop_input(&self, session: &TextInputSession) {
        session.dispose();
    }

    /// Shows the software keyboard for the open session again, if one is open (Compose's `showSoftwareKeyboard`).
    pub fn show_software_keyboard(&self) {
        if let Some(session) = self.current() {
            match self.platform_service() {
                Some(service) => service.show_software_keyboard(),
                None => self.tell(&session, true),
            }
        }
    }

    /// Hides the software keyboard (Compose's `hideSoftwareKeyboard`).
    pub fn hide_software_keyboard(&self) {
        match self.platform_service() {
            Some(service) => service.hide_software_keyboard(),
            None => self.0.platform.get().hide_software_keyboard(),
        }
    }

    /// The open session's text, selection and composition as the input method sees them, if one is open.
    pub fn session_state(&self) -> Option<TextEditState> {
        self.current().map(|s| s.state.borrow().clone())
    }

    /// The open session's options, if one is open.
    pub fn session_options(&self) -> Option<ImeOptions> {
        self.current().map(|s| s.options.clone())
    }

    /// Whether a session is open.
    pub fn has_session(&self) -> bool {
        self.current().is_some()
    }

    fn current(&self) -> Option<Rc<SessionState>> {
        self.0.session.borrow().clone()
    }

    /// The sessions of the 1.7+ API (the owner's session mutex).
    pub(crate) fn text_input_sessions(&self) -> crate::platform_text_input::SessionMutex {
        self.0.text_input_sessions.clone()
    }

    /// The open session's editor read again (a reading session's): what it holds told to the input method where it
    /// differs from what the input method sees.
    pub(crate) fn resync_open(&self) {
        if let Some(session) = self.current() {
            self.resync(&session);
        }
    }

    /// The open session's editor is shown at `rect` (px) now.
    pub(crate) fn notify_open_focused_rect(&self, rect: Rect) {
        if let Some(session) = self.current()
            && session.area.get() != rect
        {
            session.area.set(rect);
            self.tell(&session, true);
        }
    }

    fn tell(&self, session: &SessionState, active: bool) {
        let state = session.state.borrow().clone();
        self.0.platform.get().text_input(active, session.area.get(), &state, &session.options);
    }

    /// `commands` to the open session's editor; false if none is open. (No borrow is held while the editor runs.)
    pub(crate) fn send(&self, commands: &[EditCommand]) -> bool {
        let Some(session) = self.current() else { return false };
        // The session keeps the state the input method sees, as the editor will have it.
        {
            let mut state = session.state.borrow_mut();
            for c in commands {
                state.apply(c);
            }
        }
        let on_edit_command = session.on_edit_command.clone();
        on_edit_command(commands);
        self.resync(&session);
        if session.value.is_some() {
            session.version.set(session.version.peek().wrapping_add(1));
        }
        true
    }

    /// A reading session's editor read again: what it holds told to the input method where it differs from what
    /// the input method sees.
    fn resync(&self, session: &Rc<SessionState>) {
        let Some(value) = session.value.clone() else { return };
        if !session.open.get() {
            return;
        }
        let now = kompoz_runtime::without_read_observation(|| TextEditState::from_value(&value()));
        let changed = *session.state.borrow() != now;
        if changed {
            *session.state.borrow_mut() = now.clone();
            self.0.platform.get().text_input_changed(&now);
        }
    }

    /// An input method's composing text with its caret at byte `start` of it, or bytes `start` to `end` of it
    /// selected (on whole characters): Compose's SetComposingTextCommand, then a SetSelectionCommand inside the
    /// composition (as an Android input method's `setComposingText` and `setSelection`), to the open session's
    /// editor; false if none is open.
    pub(crate) fn set_composing_text_with_selection(&self, text: &str, start: usize, end: usize) -> bool {
        let Some(session) = self.current() else { return false };
        // The composition will begin where what it replaces begins.
        let at = {
            let state = session.state.borrow();
            match state.composition {
                Some((s, e)) => s.min(e),
                None => state.selection_start.min(state.selection_end),
            }
            .min(state.text.len())
        };
        let floor = |at: usize| {
            let mut at = at.min(text.len());
            while !text.is_char_boundary(at) {
                at -= 1;
            }
            at
        };
        let (start, end) = (floor(start.min(end)), floor(start.max(end)));
        self.send(&[
            EditCommand::SetComposingText { text: text.to_string(), new_cursor_position: 1 },
            EditCommand::SetSelection { start: at + start, end: at + end },
        ])
    }

    /// The input method's action key to the open session's editor; false if none is open.
    pub(crate) fn perform_ime_action(&self, action: ImeAction) -> bool {
        let Some(session) = self.current() else { return false };
        let on_ime_action = session.on_ime_action.clone();
        on_ime_action(action);
        self.resync(&session);
        true
    }
}

impl PartialEq for TextInputService {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for TextInputService {
    /// One telling no platform (what nothing provides: headless).
    fn default() -> Self {
        TextInputService::new(PlatformHandle::default())
    }
}

/// An editor's input session (Compose's TextInputSession): open until disposed of or another one starts.
pub struct TextInputSession {
    service: TextInputService,
    session: Rc<SessionState>,
}

impl TextInputSession {
    /// Its version: counted up as the input method's view moves with its commands (a reading session's).
    pub(crate) fn version(&self) -> kompoz_runtime::MutableState<u64> {
        self.session.version.clone()
    }

    /// Whether it is still the open one.
    pub fn is_open(&self) -> bool {
        self.session.open.get()
    }

    /// The editor changed its text, selection or composition itself (Compose's `updateState`): the input method is
    /// synced again where it differs from what it has.
    pub fn update_state(&self, new_value: &TextEditState) {
        if !self.is_open() {
            return;
        }
        if let Some(service) = self.service.platform_service() {
            *self.session.state.borrow_mut() = new_value.clone();
            service.update_state(None, &new_value.to_value());
            return;
        }
        let changed = *self.session.state.borrow() != *new_value;
        if changed {
            *self.session.state.borrow_mut() = new_value.clone();
            self.service.0.platform.get().text_input_changed(new_value);
        }
    }

    /// The editor's value changed from `old_value` to `new_value` (Compose's `updateState(oldValue, newValue)`):
    /// [`update_state`](Self::update_state) with the editor's values; false if the session isn't open.
    pub fn update_state_from(&self, old_value: Option<&TextFieldValue>, new_value: &TextFieldValue) -> bool {
        if !self.is_open() {
            return false;
        }
        match self.service.platform_service() {
            Some(service) => {
                *self.session.state.borrow_mut() = TextEditState::from_value(new_value);
                service.update_state(old_value, new_value);
            }
            None => self.update_state(&TextEditState::from_value(new_value)),
        }
        true
    }

    /// The editor's text was laid out again or moved (Compose's `updateTextLayoutResult`), told to the service it
    /// goes to (the host's own learns where the editor is from [`notify_focused_rect`](Self::notify_focused_rect));
    /// false if the session isn't open.
    pub fn update_text_layout_result(
        &self,
        text_field_value: &TextFieldValue,
        offset_mapping: &OffsetMapping,
        text_layout_result: &TextLayoutResult,
        text_field_to_root_transform: &dyn Fn(&mut Matrix),
        inner_text_field_bounds: Rect,
        decoration_box_bounds: Rect,
    ) -> bool {
        if !self.is_open() {
            return false;
        }
        if let Some(service) = self.service.platform_service() {
            service.update_text_layout_result(
                text_field_value,
                offset_mapping,
                text_layout_result,
                text_field_to_root_transform,
                inner_text_field_bounds,
                decoration_box_bounds,
            );
        }
        true
    }

    /// Where the editor (its caret) is in the window, px (Compose's `notifyFocusedRect`): where the input method
    /// shows its candidates.
    pub fn notify_focused_rect(&self, rect: Rect) {
        if !self.is_open() {
            return;
        }
        if let Some(service) = self.service.platform_service() {
            service.notify_focused_rect(rect);
            return;
        }
        if self.session.area.get() == rect {
            return;
        }
        self.session.area.set(rect);
        self.service.tell(&self.session, true);
    }

    /// Shows the software keyboard again (Compose's); false if the session isn't open.
    pub fn show_software_keyboard(&self) -> bool {
        if self.is_open() {
            match self.service.platform_service() {
                Some(service) => service.show_software_keyboard(),
                None => self.service.tell(&self.session, true),
            }
        }
        self.is_open()
    }

    /// Hides the software keyboard (Compose's); false if the session isn't open.
    pub fn hide_software_keyboard(&self) -> bool {
        if self.is_open() {
            self.service.hide_software_keyboard();
        }
        self.is_open()
    }

    /// Closes it (Compose's `dispose`): the platform hides its input method.
    pub fn dispose(&self) {
        if !self.session.open.replace(false) {
            return;
        }
        let current = self.service.0.session.borrow().as_ref().is_some_and(|s| Rc::ptr_eq(s, &self.session));
        if current {
            self.service.0.session.borrow_mut().take();
            match self.service.platform_service() {
                Some(service) => service.stop_input(),
                None => self.service.tell(&self.session, false),
            }
        }
    }
}

/// The text input service (Compose's LocalTextInputService): the host's.
pub static LocalTextInputService: KompozitionLocal<TextInputService> =
    KompozitionLocal::new_static(TextInputService::default);

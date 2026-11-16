//! Text input from the platform's input method (Compose's `TextInputService` and `TextInputSession` of ui-text's
//! `androidx.compose.ui.text.input`, and its `EditCommand`s): a focused editor starts a session with its text,
//! selection and options, and gets what the input method does to them as edit commands — text committed (typed on
//! a keyboard, or chosen in an input method), composing text (an input method's preedit), the composing region set
//! or finished, text around the caret deleted, the selection moved — and the input method's action key. The platform
//! is told the session's state through its callbacks ([`PlatformCallbacks::text_input`](crate::platform::PlatformCallbacks::text_input) and
//! [`text_input_changed`](crate::platform::PlatformCallbacks::text_input_changed)); what it receives it hands to the host
//! ([`KompozHost::text_input`](crate::KompozHost::text_input), `set_composing_text`, `send_edit_commands`,
//! `ime_action`), which hands it to the session open now.
//!
//! Offsets are UTF-8 byte offsets in the text, as [`TextEditState`]'s (Compose counts UTF-16 units): what a command
//! gives is clamped to the text and to whole characters.

use crate::platform::{ImeAction, ImeOptions, PlatformHandle, TextEditState};
use kompoz_graphics::Rect;
use kompoz_runtime::KompozitionLocal;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What an input method does to the text being edited (Compose's EditCommand implementations, those the platforms'
/// input methods send). Applied with [`apply`](EditCommand::apply) as Compose's `EditingBuffer` applies them.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EditCommand {
    /// `text` in place of the composition (or, without one, the selection), the composition finished; the caret
    /// then `new_cursor_position` from it as Compose has it: above 0, that many minus one after the text's end;
    /// else that many before its start (Compose's CommitTextCommand).
    CommitText { text: String, new_cursor_position: i32 },
    /// `text` in place of the composition (or the selection), and composing (none if it is empty); the caret as
    /// for [`CommitText`](EditCommand::CommitText) (Compose's SetComposingTextCommand).
    SetComposingText { text: String, new_cursor_position: i32 },
    /// The text from `start` to `end` composing (none if they are the same) (Compose's SetComposingRegionCommand).
    SetComposingRegion { start: usize, end: usize },
    /// The composition finished: its text stays (Compose's FinishComposingTextCommand).
    FinishComposingText,
    /// `before` bytes before the selection and `after` bytes after it deleted, whole characters (Compose's
    /// DeleteSurroundingTextCommand).
    DeleteSurroundingText { before: usize, after: usize },
    /// The selection from `start` to `end` (Compose's SetSelectionCommand).
    SetSelection { start: usize, end: usize },
}

/// The nearest character boundary of `text` at or before `at` (clamped to it).
fn floor_boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// The nearest character boundary of `text` at or after `at` (clamped to it).
fn ceil_boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at += 1;
    }
    at
}

/// `range` after `deleted` was taken out of the text (Compose's `updateRangeAfterDelete`).
fn after_delete(range: (usize, usize), deleted: (usize, usize)) -> (usize, usize) {
    let shift = |at: usize| {
        if at <= deleted.0 {
            at
        } else if at >= deleted.1 {
            at - (deleted.1 - deleted.0)
        } else {
            deleted.0
        }
    };
    (shift(range.0), shift(range.1))
}

impl EditCommand {
    /// Applies it to `state`, as Compose's command applies to its EditingBuffer.
    pub fn apply(&self, state: &mut TextEditState) {
        let len = state.text.len();
        let sel = (state.selection_start.min(state.selection_end), state.selection_start.max(state.selection_end));
        let sel = (floor_boundary(&state.text, sel.0.min(len)), ceil_boundary(&state.text, sel.1.min(len)));
        match self {
            EditCommand::CommitText { text, new_cursor_position }
            | EditCommand::SetComposingText { text, new_cursor_position } => {
                let (start, end) = match state.composition {
                    Some((s, e)) => (floor_boundary(&state.text, s.min(e)), ceil_boundary(&state.text, s.max(e))),
                    None => sel,
                };
                state.text.replace_range(start..end, text);
                let composing = matches!(self, EditCommand::SetComposingText { .. }) && !text.is_empty();
                state.composition = composing.then_some((start, start + text.len()));
                let p = *new_cursor_position as i64;
                let cursor = if p > 0 { (start + text.len()) as i64 + p - 1 } else { start as i64 + p };
                let cursor = cursor.clamp(0, state.text.len() as i64) as usize;
                let cursor = floor_boundary(&state.text, cursor);
                state.selection_start = cursor;
                state.selection_end = cursor;
            }
            EditCommand::SetComposingRegion { start, end } => {
                let (s, e) = ((*start).min(*end).min(len), (*start).max(*end).min(len));
                state.composition = (s != e).then(|| (floor_boundary(&state.text, s), ceil_boundary(&state.text, e)));
            }
            EditCommand::FinishComposingText => state.composition = None,
            EditCommand::DeleteSurroundingText { before, after } => {
                // (After the selection first, then before it: Compose's order.)
                let end = ceil_boundary(&state.text, sel.1.saturating_add(*after));
                let mut deleted = vec![(sel.1, end)];
                let start = floor_boundary(&state.text, sel.0.saturating_sub(*before));
                deleted.push((start, sel.0));
                let mut selection = sel;
                let mut composition = state.composition;
                for &(a, b) in &deleted {
                    if a < b {
                        state.text.replace_range(a..b, "");
                        selection = after_delete(selection, (a, b));
                        composition = composition.map(|c| after_delete(c, (a, b))).filter(|c| c.0 != c.1);
                    }
                }
                state.selection_start = selection.0;
                state.selection_end = selection.1;
                state.composition = composition;
            }
            EditCommand::SetSelection { start, end } => {
                state.selection_start = floor_boundary(&state.text, *start);
                state.selection_end = floor_boundary(&state.text, *end);
            }
        }
    }
}

/// An open session's state.
struct SessionState {
    on_edit_command: Rc<dyn Fn(&[EditCommand])>,
    on_ime_action: Rc<dyn Fn(ImeAction)>,
    state: RefCell<TextEditState>,
    options: ImeOptions,
    /// Where the editor is in the window (px), as last told.
    area: Cell<Rect>,
    open: Cell<bool>,
}

struct ServiceState {
    platform: PlatformHandle,
    session: RefCell<Option<Rc<SessionState>>>,
}

/// What a focused editor starts its input sessions with (Compose's TextInputService): one session is open at a
/// time, a new one closing the one before; the platform is told as they open and close. Provided at the root by
/// the host ([`LocalTextInputService`]). Cheap to clone; equal when the same.
#[derive(Clone)]
pub struct TextInputService(Rc<ServiceState>);

impl TextInputService {
    /// One telling `platform`'s callbacks.
    pub fn new(platform: PlatformHandle) -> TextInputService {
        TextInputService(Rc::new(ServiceState { platform, session: RefCell::new(None) }))
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
        let session = Rc::new(SessionState {
            on_edit_command: Rc::new(on_edit_command),
            on_ime_action: Rc::new(on_ime_action),
            state: RefCell::new(value),
            options,
            area: Cell::new(Rect::Zero),
            open: Cell::new(true),
        });
        let before = self.0.session.replace(Some(session.clone()));
        if let Some(before) = before {
            before.open.set(false);
        }
        self.tell(&session, true);
        TextInputSession { service: self.clone(), session }
    }

    /// Closes `session` if it is open (Compose's `stopInput`): the platform hides its input method.
    pub fn stop_input(&self, session: &TextInputSession) {
        session.dispose();
    }

    /// Shows the software keyboard for the open session again (Compose's `showSoftwareKeyboard`).
    pub fn show_software_keyboard(&self) {
        if let Some(session) = self.current() {
            self.tell(&session, true);
        }
    }

    /// Hides the software keyboard (Compose's `hideSoftwareKeyboard`).
    pub fn hide_software_keyboard(&self) {
        self.0.platform.get().hide_software_keyboard();
    }

    /// The open session's text, selection and composition as last told, if one is open.
    pub fn session_state(&self) -> Option<TextEditState> {
        self.current().map(|s| s.state.borrow().clone())
    }

    /// Whether a session is open.
    pub fn has_session(&self) -> bool {
        self.current().is_some()
    }

    fn current(&self) -> Option<Rc<SessionState>> {
        self.0.session.borrow().clone()
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
                c.apply(&mut state);
            }
        }
        let on_edit_command = session.on_edit_command.clone();
        on_edit_command(commands);
        true
    }

    /// The input method's action key to the open session's editor; false if none is open.
    pub(crate) fn perform_ime_action(&self, action: ImeAction) -> bool {
        let Some(session) = self.current() else { return false };
        let on_ime_action = session.on_ime_action.clone();
        on_ime_action(action);
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
        let changed = *self.session.state.borrow() != *new_value;
        if changed {
            *self.session.state.borrow_mut() = new_value.clone();
            self.service.0.platform.get().text_input_changed(new_value);
        }
    }

    /// Where the editor (its caret) is in the window, px (Compose's `notifyFocusedRect`): where the input method
    /// shows its candidates.
    pub fn notify_focused_rect(&self, rect: Rect) {
        if !self.is_open() || self.session.area.get() == rect {
            return;
        }
        self.session.area.set(rect);
        self.service.tell(&self.session, true);
    }

    /// Shows the software keyboard again (Compose's); false if the session isn't open.
    pub fn show_software_keyboard(&self) -> bool {
        if self.is_open() {
            self.service.tell(&self.session, true);
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
            self.service.tell(&self.session, false);
        }
    }
}

/// The text input service (Compose's LocalTextInputService): the host's.
pub static LocalTextInputService: KompozitionLocal<TextInputService> =
    KompozitionLocal::new_static(TextInputService::default);

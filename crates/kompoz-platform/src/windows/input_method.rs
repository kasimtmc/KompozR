//! A window's input method (U7: IMM32 and TSF): the focused field as the host tells it (`text_input`,
//! `text_input_changed`) and as the core answers once it handled what was typed, given to TSF's text services
//! through a text store where TSF is there (`tsf`), else to IMM32's input methods (`imm`); what they compose and
//! commit as the core's events (`ime`'s translations), drawn in the field by KompozR — no input method's own
//! composition window.
//!
//! - IMM32: the window's input context associated only while a field takes typing (not a password's); its
//!   composition (WM_IME_STARTCOMPOSITION … WM_IME_COMPOSITION … WM_IME_ENDCOMPOSITION) read and sent as the
//!   field's composing text (its string, the clause being converted, its caret) and its result as committed text,
//!   the characters Windows would type for that result left out; its requests answered (where a character is,
//!   the text to reconvert or to read around the composition, the reconversion confirmed as the selection, where
//!   its windows go); its windows placed at the caret (the candidates kept out of the caret's box); a composition
//!   the app ended cancelled, one it changed replaced; the field's input scopes set on the window.
//! - TSF: a document manager and context per focused field, the text store answering the text services under their
//!   locks; their edits sent to the core once a lock is released, the app's changes told to them.
//!
//! `KOMPOZ_WIN_IME=imm32` keeps TSF out (IMM32 only); TSF is used where it can be activated, IMM32 otherwise.

use super::ime::{self, Answer, Field};
use super::imm::{self, Context, Part, Request};
use super::sys::{Hwnd, WindowMessage};
use super::tsf::{self, FieldContext, ThreadManager};
use crate::app::Event;
use ::windows::Win32::UI::WindowsAndMessaging as wm;
use kompoz_graphics::Rect;
use kompoz_ui::platform::{ImeOptions, TextEditState};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

/// Which path the window's typing goes through.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Imm32,
    Tsf,
}

/// The input method path chosen: TSF unless `KOMPOZ_WIN_IME=imm32` (or TSF can't be activated).
pub fn mode() -> Mode {
    let forced = kompoz_runtime::environment::var("KOMPOZ_WIN_IME").is_some_and(|v| v.eq_ignore_ascii_case("imm32"));
    if !forced && ThreadManager::get().is_some() { Mode::Tsf } else { Mode::Imm32 }
}

// (imm.h's composition flags and ISC_ bit; Windows' input method key.)
const GCS_COMPSTR: u32 = 0x0008;
const GCS_COMPATTR: u32 = 0x0010;
const GCS_CURSORPOS: u32 = 0x0080;
const GCS_RESULTSTR: u32 = 0x0800;
const ISC_SHOWUICOMPOSITIONWINDOW: u32 = 0x8000_0000;
const IMN_OPENCANDIDATE: usize = 0x0005;
pub const VK_PROCESSKEY: u16 = 0xE5;

/// A window's input method: the focused field, the path, what each path keeps.
pub struct InputMethod {
    hwnd: Cell<Hwnd>,
    mode: Cell<Mode>,
    /// The window's events (its queue).
    events: Rc<RefCell<VecDeque<Event>>>,
    field: RefCell<Field>,
    /// IMM32: a composition is shown in the field.
    composing: Cell<bool>,
    /// IMM32: the units of a result committed from GCS_RESULTSTR that Windows would also type (WM_IME_CHAR, WM_CHAR):
    /// left out as they come, forgotten at the next key.
    typed_result: RefCell<VecDeque<u16>>,
    /// IMM32: where the last RECONVERTSTRING's string began in the field (units).
    reconversion_start: Cell<usize>,
    /// TSF: the focused field's context.
    context: RefCell<Option<FieldContext>>,
    /// IMM32: its windows being placed.
    placing: Cell<bool>,
    /// Itself, for the text store's owner.
    this: RefCell<Weak<InputMethod>>,
}

impl InputMethod {
    pub fn new(events: Rc<RefCell<VecDeque<Event>>>) -> Rc<InputMethod> {
        let made = Rc::new(InputMethod {
            hwnd: Cell::new(Hwnd::NONE),
            mode: Cell::new(Mode::Imm32),
            events,
            field: RefCell::new(Field::default()),
            composing: Cell::new(false),
            typed_result: RefCell::new(VecDeque::new()),
            reconversion_start: Cell::new(0),
            placing: Cell::new(false),
            context: RefCell::new(None),
            this: RefCell::new(Weak::new()),
        });
        *made.this.borrow_mut() = Rc::downgrade(&made);
        made
    }

    /// The window made: its typing through `mode`'s path; no input method until a field takes typing.
    pub fn attach(&self, hwnd: Hwnd, mode: Mode) {
        self.hwnd.set(hwnd);
        self.mode.set(mode);
        imm::associate(hwnd, false);
    }

    /// The window going: its field's context gone.
    pub fn detach(&self) {
        let context = self.context.borrow_mut().take();
        drop(context);
        self.hwnd.set(Hwnd::NONE);
        *self.field.borrow_mut() = Field::default();
        self.composing.set(false);
    }

    #[cfg(test)]
    pub fn mode(&self) -> Mode {
        self.mode.get()
    }

    /// The caret's box of the field that takes typing (px in the client area), if one does (UI Automation's text
    /// ranges at the caret).
    pub fn caret_box(&self) -> Option<Rect> {
        let f = self.field.borrow();
        f.active.then_some(f.area)
    }

    /// The field as the input method sees it (tests).
    #[cfg(test)]
    pub fn field(&self) -> Field {
        self.field.borrow().clone()
    }

    /// The TSF context of the focused field (tests).
    #[cfg(test)]
    pub fn with_context<R>(&self, f: impl FnOnce(Option<&FieldContext>) -> R) -> R {
        f(self.context.borrow().as_ref())
    }

    fn send(&self, events: Vec<Event>) {
        let events = self.field.borrow_mut().send(events);
        self.events.borrow_mut().extend(events);
    }

    /// The host's `text_input`: a field takes typing (or none does) with its caret at `area` (px), wanting
    /// `options`. A field beginning to take it gets the input method (IMM32's context associated, or a TSF
    /// context made); one going on taking it tells it where it moved, what it now wants, what the app changed.
    pub fn text_input(&self, active: bool, area: Rect, state: &TextEditState, options: &ImeOptions) {
        let hwnd = self.hwnd.get();
        if hwnd.is_none() {
            return;
        }
        let (was_active, moved, new_options) = {
            let f = self.field.borrow();
            (f.active, f.area != area, f.options != *options)
        };
        if !active {
            if was_active {
                self.end_field();
            }
            return;
        }
        if !was_active {
            *self.field.borrow_mut() =
                Field { active, area, options: options.clone(), state: ime::normalized(state), unanswered: false };
            self.begin_field(state, options);
            return;
        }
        {
            let mut f = self.field.borrow_mut();
            f.area = area;
            f.options = options.clone();
        }
        if new_options {
            self.options_changed(options);
        }
        if moved {
            self.moved();
        }
        self.changed(state);
    }

    /// The host's `text_input_changed`: the app changed the field.
    pub fn text_input_changed(&self, state: &TextEditState) {
        if self.field.borrow().active {
            self.changed(state);
        }
    }

    /// The field as the core has it after handling what was typed (the loop's answer), none if no session is open.
    pub fn answer(&self, state: Option<TextEditState>) {
        let Some(state) = state else {
            self.field.borrow_mut().unanswered = false;
            return;
        };
        if !self.field.borrow().active {
            return;
        }
        let answer = self.field.borrow_mut().answer(&state);
        if answer == Answer::Changed {
            self.app_changed(&state);
        }
    }

    /// The app's change to the field, unless events sent wait for the core (the answer then says what it made of
    /// them and of the change).
    fn changed(&self, state: &TextEditState) {
        if self.field.borrow().unanswered {
            return;
        }
        let answer = self.field.borrow_mut().answer(state);
        if answer == Answer::Changed {
            self.app_changed(state);
        }
    }

    fn begin_field(&self, state: &TextEditState, options: &ImeOptions) {
        let hwnd = self.hwnd.get();
        match self.mode.get() {
            Mode::Imm32 => {
                imm::associate(hwnd, !ime::hides_text(options));
                imm::set_input_scopes(hwnd, &ime::input_scopes(options));
                self.composing.set(false);
                self.place();
            }
            Mode::Tsf => {
                let made = ThreadManager::get().and_then(|manager| {
                    let owner: Weak<dyn tsf::Owner> = self.this.borrow().clone();
                    FieldContext::new(manager, hwnd, state, options, owner)
                });
                match made {
                    Some(context) => *self.context.borrow_mut() = Some(context),
                    None => {
                        // (TSF refused the field: IMM32's then, from now on.)
                        self.mode.set(Mode::Imm32);
                        self.begin_field(state, options);
                    }
                }
            }
        }
    }

    fn end_field(&self) {
        let hwnd = self.hwnd.get();
        match self.mode.get() {
            Mode::Imm32 => {
                if self.composing.replace(false)
                    && let Some(c) = Context::of(hwnd)
                {
                    c.end_composition(false);
                }
                imm::associate(hwnd, false);
                imm::set_input_scopes(hwnd, &[]);
            }
            Mode::Tsf => {
                let context = self.context.borrow_mut().take();
                drop(context);
            }
        }
        self.typed_result.borrow_mut().clear();
        *self.field.borrow_mut() = Field::default();
    }

    fn options_changed(&self, options: &ImeOptions) {
        let hwnd = self.hwnd.get();
        match self.mode.get() {
            Mode::Imm32 => {
                imm::associate(hwnd, !ime::hides_text(options));
                imm::set_input_scopes(hwnd, &ime::input_scopes(options));
            }
            Mode::Tsf => {
                if let Some(c) = self.context.borrow().as_ref() {
                    c.set_options(options);
                }
            }
        }
    }

    fn moved(&self) {
        match self.mode.get() {
            Mode::Imm32 => self.place(),
            Mode::Tsf => {
                // (The context taken out first: the text services may ask for a lock while told.)
                let context = self.context.borrow_mut().take();
                if let Some(c) = &context {
                    c.layout_changed();
                }
                self.restore(context);
            }
        }
    }

    /// The field the app changed (`state`, the field's now): IMM32's composition, where the field's isn't the one
    /// it composes, replaced by the field's or cancelled; TSF's text services told (their composition
    /// terminated).
    fn app_changed(&self, state: &TextEditState) {
        match self.mode.get() {
            Mode::Imm32 => {
                if !self.composing.get() {
                    return;
                }
                let Some(c) = Context::of(self.hwnd.get()) else { return };
                let state = ime::normalized(state);
                match ime::composition(&state) {
                    Some((a, b)) => {
                        let text: Vec<u16> = state.text[a..b].encode_utf16().collect();
                        if text != c.string(Part::Composition) && !c.set_composition(&text) {
                            self.composing.set(false);
                            c.end_composition(false);
                        }
                    }
                    None => {
                        // (Its messages that follow aren't the field's any more.)
                        self.composing.set(false);
                        c.end_composition(false);
                    }
                }
            }
            Mode::Tsf => {
                let context = self.context.borrow_mut().take();
                if let Some(c) = &context {
                    c.changed_by_app(state);
                }
                self.restore(context);
            }
        }
    }

    /// A context taken out for a call into TSF put back (unless the field ended meanwhile and another began).
    fn restore(&self, context: Option<FieldContext>) {
        let mut slot = self.context.borrow_mut();
        if slot.is_none() && self.field.borrow().active {
            *slot = context;
        }
    }

    // ---- IMM32 ----

    /// The input method's windows at the caret (client coordinates); not again while they are placed (setting them
    /// notifies the window).
    fn place(&self) {
        if self.placing.replace(true) {
            return;
        }
        self.place_now();
        self.placing.set(false);
    }

    fn place_now(&self) {
        let area = self.field.borrow().area;
        let (point, exclude) = ime::placement(area);
        if let Some(c) = Context::of(self.hwnd.get()) {
            c.place((point.left, point.top), exclude);
        }
    }

    /// The window's IMM32 messages (WM_IME_*), while a field takes typing through IMM32: their answer, none for
    /// Windows' own handling.
    pub fn message(&self, hwnd: Hwnd, m: &WindowMessage) -> Option<isize> {
        let (message, w, l) = (m.id(), m.w(), m.l());
        if message == wm::WM_IME_SETCONTEXT {
            // (Its composition window not shown: KompozR draws the composing text.)
            let l = if w != 0 { (l as u32 & !ISC_SHOWUICOMPOSITIONWINDOW) as i32 as isize } else { l };
            return Some(hwnd.default_procedure(message, w, l));
        }
        if self.mode.get() != Mode::Imm32 || !self.field.borrow().active {
            return None;
        }
        match message {
            wm::WM_IME_STARTCOMPOSITION => {
                self.place();
                Some(0)
            }
            wm::WM_IME_COMPOSITION => {
                self.composition(hwnd, l as u32);
                Some(0)
            }
            wm::WM_IME_ENDCOMPOSITION => {
                if self.composing.replace(false) {
                    self.send(vec![Event::Preedit { text: String::new(), cursor: None }]);
                }
                Some(0)
            }
            wm::WM_IME_NOTIFY => {
                // (Not at IMN_SETCANDIDATEPOS: that is what placing them sends.)
                if w == IMN_OPENCANDIDATE {
                    self.place();
                }
                None
            }
            wm::WM_IME_REQUEST => self.request(hwnd, m),
            _ => None,
        }
    }

    /// WM_IME_COMPOSITION: its result committed (the characters Windows would type for it left out), then its
    /// composing text; nothing of either: the composition given up.
    fn composition(&self, hwnd: Hwnd, flags: u32) {
        let Some(c) = Context::of(hwnd) else { return };
        let mut events = Vec::new();
        if flags & GCS_RESULTSTR != 0 {
            let result = c.string(Part::Result);
            if !result.is_empty() {
                *self.typed_result.borrow_mut() = result.iter().copied().collect();
                events.push(Event::Text(String::from_utf16_lossy(&result)));
                self.composing.set(false);
            }
        }
        if flags & GCS_COMPSTR != 0 {
            let text = c.string(Part::Composition);
            let attributes = if flags & GCS_COMPATTR != 0 { c.attributes() } else { Vec::new() };
            let cursor = if flags & GCS_CURSORPOS != 0 { c.cursor() } else { None };
            if !text.is_empty() {
                events.push(ime::imm_preedit(&text, &attributes, cursor));
                self.composing.set(true);
            } else if self.composing.replace(false) {
                events.push(Event::Preedit { text: String::new(), cursor: None });
            }
        } else if flags & GCS_RESULTSTR == 0 && self.composing.replace(false) {
            events.push(Event::Preedit { text: String::new(), cursor: None });
        }
        self.send(events);
    }

    /// WM_IME_REQUEST answered from the field as the core will have it.
    fn request(&self, hwnd: Hwnd, m: &WindowMessage) -> Option<isize> {
        let request = Request::of(m)?;
        let field = self.field.borrow().clone();
        let (point, exclude) = ime::placement(field.area);
        Some(match request {
            Request::CompositionWindow | Request::CandidateWindow => {
                imm::answer_window(m, (point.left, point.top), exclude)
            }
            Request::Reconvert | Request::DocumentFeed => {
                let feed = request == Request::DocumentFeed;
                let Some(r) = ime::reconversion(&field.state, feed, ime::RECONVERSION_MAX) else { return Some(0) };
                if m.l() != 0 {
                    self.reconversion_start.set(r.start);
                }
                imm::answer_reconversion(m, &r.encode())
            }
            Request::ConfirmReconvert => {
                let bytes = imm::confirmed_reconversion(m)?;
                let Some((a, b)) = ime::Reconversion::confirmed(&bytes, self.reconversion_start.get()) else {
                    return Some(0);
                };
                // (The part it reconverts selected: the composition that follows takes its place.)
                let text = &field.state.text;
                let (start, end) = (ime::byte_of_utf16(text, a), ime::byte_of_utf16(text, b));
                if ime::selection(&field.state) != (start, end) {
                    self.send(vec![Event::SetSelection { start, end }]);
                }
                1
            }
            Request::CharPosition => {
                let at = ime::char_position(field.area, hwnd.client_origin());
                imm::answer_char_position(m, at, at)
            }
        })
    }

    /// A key (not the input method's own): what was left of a result's characters is forgotten.
    pub fn key(&self, vk: u16) {
        if vk != VK_PROCESSKEY {
            self.typed_result.borrow_mut().clear();
        }
    }

    /// Whether a character (WM_CHAR, WM_IME_CHAR's unit) is one of the result already committed: left out.
    pub fn typed_by_result(&self, unit: u16) -> bool {
        let mut typed = self.typed_result.borrow_mut();
        if typed.front() == Some(&unit) {
            typed.pop_front();
            true
        } else {
            typed.clear();
            false
        }
    }
}

impl tsf::Owner for InputMethod {
    /// The text services' edit as the core's events (from the field as the core will have it).
    fn edited(&self, presented: TextEditState) {
        if !self.field.borrow().active {
            return;
        }
        let events = ime::transition(&self.field.borrow().state, &presented);
        self.send(events);
    }

    fn caret(&self) -> Rect {
        self.field.borrow().area
    }
}

#[cfg(test)]
mod tests {
    use super::super::window::{Shared, Win32Window};
    use super::*;
    use crate::app::{Platform, WindowSpec};
    use kompoz_gpu::device::Backend;
    use kompoz_graphics::Dp;
    use kompoz_ui::window::WindowId;

    fn pump() {
        while let Some(m) = super::super::sys::peek_message() {
            super::super::sys::dispatch(&m);
        }
    }

    fn open(shared: &Rc<Shared>, title: &str) -> Win32Window {
        let mut w = Win32Window::new(shared, WindowId::MAIN);
        let spec = WindowSpec::main(title, Dp(200.0), Dp(150.0));
        drop(w.open_window(&spec, Backend::Vulkan).expect("a window"));
        pump();
        w
    }

    fn events(shared: &Shared) -> Vec<Event> {
        pump();
        shared.take_events().into_iter().map(|(_, e)| e).collect()
    }

    fn units(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// The window's typing events since (what an input method sends the core).
    fn typed(shared: &Shared) -> Vec<Event> {
        events(shared)
            .into_iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::Text(_)
                        | Event::Preedit { .. }
                        | Event::DeleteSurroundingText { .. }
                        | Event::SetSelection { .. }
                )
            })
            .collect()
    }

    fn state(text: &str, selection: (usize, usize), composition: Option<(usize, usize)>) -> TextEditState {
        TextEditState { text: text.into(), selection_start: selection.0, selection_end: selection.1, composition }
    }

    /// A RECONVERTSTRING request answered as an input method asks it: its size, then into a buffer that large.
    fn reconvert_string(hwnd: Hwnd, request: u32) -> Option<Vec<u8>> {
        let size = hwnd.send(wm::WM_IME_REQUEST, request as usize, 0);
        if size <= 0 {
            return None;
        }
        let mut buffer = vec![0u8; size as usize];
        buffer[..4].copy_from_slice(&(size as u32).to_le_bytes());
        let filled = hwnd.send(wm::WM_IME_REQUEST, request as usize, buffer.as_mut_ptr() as isize);
        assert_eq!(filled, size);
        Some(buffer)
    }

    fn dword(bytes: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
    }

    /// A composition through the field's context (`c`, of `hwnd`'s field holding "ab"): its composing text, the
    /// requests answered from the field while it composes, its result committed (the characters Windows would type
    /// for it left out), a key's after, the core's answer taken.
    fn composition_and_its_result(shared: &Shared, ime: &InputMethod, c: &Context, hwnd: Hwnd) {
        use ::windows::Win32::UI::Input::Ime as imm32;
        // Composing: the field's composing text, its caret where the input method has it.
        c.set_open(true);
        assert!(c.is_open());
        assert!(c.set_composition(&units("かな")));
        assert_eq!(typed(shared), [Event::Preedit { text: "かな".into(), cursor: Some((6, 6)) }]);
        assert_eq!(ime.field().state, state("abかな", (8, 8), Some((2, 8))));
        // The input method reads the text around its composition; asks where its characters are.
        let feed = reconvert_string(hwnd, imm32::IMR_DOCUMENTFEED).expect("the document fed");
        assert_eq!([dword(&feed, 2), dword(&feed, 4), dword(&feed, 5)], [4, 2, 4]);
        assert_eq!(reconvert_string(hwnd, imm32::IMR_RECONVERTSTRING), None, "no reconversion while composing");
        let mut position = imm32::IMECHARPOSITION {
            dwSize: std::mem::size_of::<imm32::IMECHARPOSITION>() as u32,
            ..Default::default()
        };
        let answered =
            hwnd.send(wm::WM_IME_REQUEST, imm32::IMR_QUERYCHARPOSITION as usize, &mut position as *mut _ as isize);
        let (x, y) = hwnd.client_origin();
        assert_eq!(answered, 1);
        assert_eq!((position.pt.x, position.pt.y, position.cLineHeight), (x + 10, y + 20, 20));
        // Committed: the result the field's text, its characters typed by Windows after it left out; a key's after.
        c.end_composition(true);
        assert_eq!(typed(shared), [Event::Text("かな".into())]);
        hwnd.send(wm::WM_CHAR, 'か' as usize, 0);
        hwnd.send(wm::WM_IME_CHAR, 'な' as usize, 0);
        hwnd.send(wm::WM_CHAR, 'x' as usize, 0);
        assert_eq!(typed(shared), [Event::Text("x".into())]);
        // The core's answer: the field as it took it (the key's character too).
        ime.answer(Some(state("abかなx", (9, 9), None)));
        assert_eq!(ime.field().state, state("abかなx", (9, 9), None));
    }

    /// IMM32 under wine (its default input method: compositions set and completed through the context, as an
    /// input method makes them): the context only while a field takes typing (not a password's), its windows at the
    /// caret, its composition the field's composing text and its result committed (the characters Windows would type
    /// for it left out), its requests answered from the field, a composition the app ended cancelled. On Windows
    /// (CI's runners: no input method that composes) the context, its windows, reconversion and the fields' ends.
    #[test]
    fn imm32_compositions_requests_and_the_apps_changes() {
        use ::windows::Win32::UI::Input::Ime as imm32;
        let shared = Shared::connect(WindowId::MAIN).expect("the platform");
        let w = open(&shared, "Giriş");
        let hwnd = w.handle();
        let ime = w.input_method();
        ime.attach(hwnd, Mode::Imm32);
        assert!(Context::of(hwnd).is_none(), "no context outside a field");
        // (The input context composes for the window with the focus: this one, whatever other tests' windows do.)
        hwnd.focus();
        let area = Rect::new(10.0, 20.0, 11.0, 40.0);
        let options = ImeOptions::default();
        ime.text_input(true, area, &state("ab", (2, 2), None), &options);
        events(&shared);
        let c = Context::of(hwnd).expect("a field's context");
        // Its windows at the caret: the composition window's point, the candidates kept out of the caret's box.
        assert_eq!(c.composition_window(), Some((imm32::CFS_POINT, (10, 20))));
        let (style, exclude) = c.candidate_window().expect("the candidate window's place");
        assert_eq!(
            (style, exclude),
            (imm32::CFS_EXCLUDE, ime::CharPosition { left: 10, top: 20, right: 11, bottom: 40 })
        );
        // Its composition window hidden (KompozR draws the composing text): answered, Windows' own handling with it.
        hwnd.send(wm::WM_IME_SETCONTEXT, 1, ISC_SHOWUICOMPOSITIONWINDOW as i32 as isize | 0xF);
        // Composing (wine's default input method composes what it is given; Windows without an East Asian input
        // method installed — CI's runners — may commit it at once or keep it, by the keyboard's state: there only
        // what needs no composition is checked).
        let composes = crate::windows::sys::under_wine();
        if composes {
            composition_and_its_result(&shared, &ime, &c, hwnd);
        }
        // Reconversion: a word selected, the paragraph given; confirmed over more of it, that part selected.
        ime.text_input_changed(&state("kanji desu", (0, 5), None));
        let mut r = reconvert_string(hwnd, imm32::IMR_RECONVERTSTRING).expect("the selection to reconvert");
        assert_eq!([dword(&r, 2), dword(&r, 4), dword(&r, 5)], [10, 5, 0]);
        r[16..20].copy_from_slice(&10u32.to_le_bytes());
        let confirmed =
            hwnd.send(wm::WM_IME_REQUEST, imm32::IMR_CONFIRMRECONVERTSTRING as usize, r.as_mut_ptr() as isize);
        assert_eq!(confirmed, 1);
        assert_eq!(typed(&shared), [Event::SetSelection { start: 0, end: 10 }]);
        // The app ends a composition: the input method's cancelled, nothing more of it sent.
        if composes {
            ime.answer(Some(state("kanji desu", (10, 10), None)));
            assert!(c.set_composition(&units("と")));
            assert_eq!(typed(&shared), [Event::Preedit { text: "と".into(), cursor: Some((3, 3)) }]);
            ime.answer(Some(state("", (0, 0), None)));
            assert!(c.string(Part::Composition).is_empty(), "cancelled");
            assert!(typed(&shared).is_empty());
        }
        // A password: no input method; the field gone: none.
        drop(c);
        let password = ImeOptions { keyboard_type: kompoz_ui::platform::KeyboardType::Password, ..options.clone() };
        ime.text_input(true, area, &state("", (0, 0), None), &password);
        assert!(Context::of(hwnd).is_none(), "no input method in a password");
        ime.text_input(true, area, &state("", (0, 0), None), &options);
        assert!(Context::of(hwnd).is_some());
        ime.text_input(false, area, &state("", (0, 0), None), &options);
        assert!(Context::of(hwnd).is_none());
        drop(w);
        shared.close();
    }

    /// TSF where it can be activated (wine's msctf): the field's typing through a context of its own (none of
    /// IMM32's then), gone with the field.
    #[test]
    fn tsf_takes_the_field_where_it_can() {
        let shared = Shared::connect(WindowId::MAIN).expect("the platform");
        let w = open(&shared, "TSF");
        let (hwnd, ime) = (w.handle(), w.input_method());
        let mode = mode();
        assert_eq!(mode, Mode::Tsf, "wine and Windows have TSF");
        ime.attach(hwnd, mode);
        let options = ImeOptions::default();
        ime.text_input(true, Rect::new(1.0, 2.0, 3.0, 4.0), &state("ab", (2, 2), None), &options);
        assert_eq!(ime.mode(), Mode::Tsf);
        assert!(ime.with_context(|c| c.is_some()));
        assert!(Context::of(hwnd).is_none(), "no IMM32 context beside TSF's");
        ime.text_input(true, Rect::new(5.0, 2.0, 6.0, 4.0), &state("ab", (2, 2), None), &options);
        ime.text_input_changed(&state("abc", (3, 3), None));
        assert_eq!(ime.with_context(|c| c.unwrap().store().document().state), state("abc", (3, 3), None));
        ime.text_input(false, Rect::default(), &state("", (0, 0), None), &options);
        assert!(ime.with_context(|c| c.is_none()));
        drop(w);
        shared.close();
    }
}

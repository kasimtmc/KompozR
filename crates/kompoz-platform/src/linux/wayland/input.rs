//! The seat's input (C++'s pointer, keyboard, touch, pinch and text-input listeners): the pointer's moves, buttons
//! and wheel (notches and a touchpad's scrolls gathered until its frame), the keyboard through xkbcommon (its
//! keymap handed over, compose sequences, repeated client-side by the event loop's timer), fingers on a touch
//! screen, a touchpad's pinches, and an input method's typing (text-input-v3: what it commits, its composing text
//! with its caret and what it deletes, applied at its `done` as the protocol orders them; told whether a field takes
//! typing, where it is, what it takes, its text around the caret and why that changed, each change committed and
//! the commits counted against its `done`s).

use super::WaylandPlatform;
use super::ffi::Object;
use super::generated::*;
use crate::app::{Event, MouseButton};
use crate::linux::sys::{self, Mmap};
use kompoz_ui::platform::{ImeOptions, KeyboardCapitalization, KeyboardType, TextEditState};
use std::os::fd::AsFd;

/// A pointer button (linux/input-event-codes.h's `BTN_LEFT`…) as KompozR's.
pub(super) fn mouse_button(code: u32) -> Option<MouseButton> {
    Some(match code {
        0x110 => MouseButton::Left,
        0x111 => MouseButton::Right,
        0x112 => MouseButton::Middle,
        0x113 => MouseButton::Back,
        0x114 => MouseButton::Forward,
        _ => return None,
    })
}

/// The wheel as SDL tells it (C++'s): a notch is 1 (`value120` / 120), a touchpad's scroll a tenth of its surface
/// units; up and right positive. Gathered until the pointer's frame.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(super) struct Wheel {
    x: f64,
    y: f64,
    /// Notches came in this frame: the axis' distance (also sent) left out.
    discrete: bool,
}

impl Wheel {
    /// `wl_pointer.axis`: a distance in surface units.
    pub fn axis(&mut self, axis: WlPointerAxis, value: f64) {
        if self.discrete {
            return;
        }
        self.add(axis, value / 10.0);
    }

    /// `wl_pointer.axis_value120`: notches in 120ths (`axis_discrete`'s whole notches: 120 each).
    pub fn value120(&mut self, axis: WlPointerAxis, value120: i32) {
        if !self.discrete {
            (self.x, self.y) = (0.0, 0.0); // (the notches, not the axis' distance)
        }
        self.discrete = true;
        self.add(axis, value120 as f64 / 120.0);
    }

    fn add(&mut self, axis: WlPointerAxis, notches: f64) {
        if axis == WlPointerAxis::VERTICAL_SCROLL {
            self.y -= notches;
        } else {
            self.x += notches;
        }
    }

    /// The pointer's frame: the notches gathered (none if nothing turned), and a new frame begun.
    pub fn frame(&mut self) -> Option<(f32, f32)> {
        let turned = (self.x != 0.0 || self.y != 0.0).then_some((self.x as f32, self.y as f32));
        *self = Wheel::default();
        turned
    }
}

/// A touchpad pinch's scale so far (Wayland tells it since the pinch began; KompozR's events the change since the
/// last).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct Pinch {
    scale: f64,
}

impl Default for Pinch {
    fn default() -> Self {
        Pinch { scale: 1.0 }
    }
}

impl Pinch {
    pub fn begin(&mut self) {
        self.scale = 1.0;
    }

    /// An update's scale (since the pinch began) as the change since the last.
    pub fn update(&mut self, scale: f64) -> f32 {
        let change = scale / self.scale.max(1e-3);
        self.scale = scale;
        change as f32
    }
}

/// text-input-v3's content hint and purpose for a field's options (C++'s tellTextInput, Compose 1.12's keyboard
/// types and capitalization as Android's EditorInfo takes them: a hidden password's text hidden and sensitive, a
/// shown one's sensitive; the capitalization as a case hint; ASCII as Latin).
pub(super) fn content_type(options: &ImeOptions) -> (ZwpTextInputV3ContentHint, ZwpTextInputV3ContentPurpose) {
    type Hint = ZwpTextInputV3ContentHint;
    type Purpose = ZwpTextInputV3ContentPurpose;
    let mut hint = if options.auto_correct { Hint::COMPLETION | Hint::SPELLCHECK } else { Hint::NONE };
    let hidden = Hint::HIDDEN_TEXT | Hint::SENSITIVE_DATA;
    let purpose = match options.keyboard_type {
        KeyboardType::Unspecified
        | KeyboardType::Text
        | KeyboardType::PostalAddress
        | KeyboardType::EmailSubject
        | KeyboardType::ShortMessage
        | KeyboardType::LongMessage
        | KeyboardType::Filter
        | KeyboardType::Phonetic => Purpose::NORMAL,
        KeyboardType::Ascii => {
            hint |= Hint::LATIN;
            Purpose::NORMAL
        }
        KeyboardType::Number => Purpose::DIGITS,
        KeyboardType::Decimal | KeyboardType::NumberSigned | KeyboardType::DecimalSigned => Purpose::NUMBER,
        KeyboardType::Phone => Purpose::PHONE,
        KeyboardType::Uri => Purpose::URL,
        KeyboardType::Email => Purpose::EMAIL,
        KeyboardType::PersonName => Purpose::NAME,
        KeyboardType::DateTime => Purpose::DATETIME,
        KeyboardType::Date => Purpose::DATE,
        KeyboardType::Time => Purpose::TIME,
        KeyboardType::Password => {
            hint = hidden;
            Purpose::PASSWORD
        }
        KeyboardType::PasswordVisible => {
            hint = Hint::SENSITIVE_DATA;
            Purpose::PASSWORD
        }
        KeyboardType::NumberPassword => {
            hint = hidden;
            Purpose::PIN
        }
        KeyboardType::DecimalPassword | KeyboardType::NumberPasswordSigned | KeyboardType::DecimalPasswordSigned => {
            hint = hidden;
            Purpose::NUMBER
        }
    };
    hint |= match options.capitalization {
        KeyboardCapitalization::Characters => Hint::UPPERCASE,
        KeyboardCapitalization::Words => Hint::TITLECASE,
        KeyboardCapitalization::Sentences => Hint::AUTO_CAPITALIZATION,
        KeyboardCapitalization::None | KeyboardCapitalization::Unspecified => Hint::NONE,
    };
    if !options.single_line {
        hint |= Hint::MULTILINE;
    }
    (hint, purpose)
}

/// Escape's evdev code (what a key event of the seat's keyboard carries).
const KEY_ESC: u32 = 1;

/// The most of a field's text an input method is told (`set_surrounding_text`: a Wayland message can't be longer).
pub(super) const SURROUNDING_MAX: usize = 4000;

/// The nearest character boundary of `text` at or before `at` (clamped to it).
fn floor_char(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// The nearest character boundary of `text` at or after `at` (clamped to it).
fn ceil_char(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at += 1;
    }
    at
}

/// What text-input-v3's `set_surrounding_text` tells of a field: its text around the caret and the selection, at
/// most `max` bytes, cut at whole characters, and the caret's and the selection's other end's bytes in it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Surrounding {
    pub text: String,
    pub cursor: i32,
    pub anchor: i32,
}

/// The surrounding text of a field (the protocol's rules): its composing text left out with the caret in its place
/// (the input method has it), the caret and the whole selection in it with as much text before and after them as
/// fits in `max` bytes (shared between the two sides, a side's unused share the other's); a selection longer than
/// that: the part of it at the caret. (A selection's anchor is its start, the caret its end: Compose's.)
pub(super) fn surrounding(state: &TextEditState, max: usize) -> Surrounding {
    let full = &state.text;
    let (mut anchor, mut cursor) = (floor_char(full, state.selection_start), floor_char(full, state.selection_end));
    let without;
    let text = match state.composition {
        Some((a, b)) if a != b => {
            let (a, b) = (floor_char(full, a.min(b)), ceil_char(full, a.max(b)));
            without = format!("{}{}", &full[..a], &full[b..]);
            (anchor, cursor) = (a, a);
            without.as_str()
        }
        _ => full.as_str(),
    };
    let (lo, hi) = (anchor.min(cursor), anchor.max(cursor));
    let (start, end) = if hi - lo <= max {
        let spare = max - (hi - lo);
        let after = (spare - (spare / 2).min(lo)).min(text.len() - hi);
        let before = (spare - after).min(lo);
        (lo - before, hi + after)
    } else if cursor == hi {
        (cursor - max, cursor)
    } else {
        (cursor, cursor + max)
    };
    let (start, end) = (ceil_char(text, start), floor_char(text, end));
    let anchor = anchor.clamp(start, end);
    Surrounding { text: text[start..end].to_string(), cursor: (cursor - start) as i32, anchor: (anchor - start) as i32 }
}

/// A preedit's caret (`preedit_string`'s cursor_begin and cursor_end, bytes) as the core's: none if hidden (-1, or
/// any other negative), else in order, on whole characters of `text` (the end rounded out).
pub(super) fn preedit_cursor(text: &str, begin: i32, end: i32) -> Option<(usize, usize)> {
    let (begin, end) = (usize::try_from(begin).ok()?, usize::try_from(end).ok()?);
    let (a, b) = (floor_char(text, begin.min(end)), ceil_char(text, begin.max(end)));
    Some(if begin == end { (a, a) } else { (a, b) })
}

/// What an input method sent since its last `done`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Pending {
    /// The composing text and its caret (bytes; none: hidden).
    preedit: Option<(String, Option<(usize, usize)>)>,
    commit: Option<String>,
    /// Bytes before and after the caret.
    delete: Option<(usize, usize)>,
}

/// A request to the input method (text-input-v3's).
#[derive(Clone, PartialEq, Debug)]
enum Request {
    Enable,
    Disable,
    SurroundingText(Surrounding),
    TextChangeCause(ZwpTextInputV3ChangeCause),
    ContentType(ZwpTextInputV3ContentHint, ZwpTextInputV3ContentPurpose),
    CursorRectangle([i32; 4]),
    Commit,
}

/// How what came since the core's last events is answered to the input method.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Answer {
    /// No `done` came: the field's changes are the app's (or the keyboard's): told as `other`.
    #[default]
    Other,
    /// A `done` of the state the input method was last told: the field after it told as its own doing.
    InputMethod,
    /// A `done` of an older state (more was committed since): applied, not answered (the protocol's rule).
    Stale,
}

/// The input method on the window (text-input-v3), as the host's callbacks and the platform share it: enabled while
/// a field takes typing (again for each field), told the field's place, kind and surrounding text — why it changed
/// too —, each change committed and counted; what it sends applied at its `done`, as the protocol orders it.
#[derive(Default)]
pub(crate) struct TextInput {
    pub input: Option<ZwpTextInputV3>,
    /// The input method is on the window (its enter came).
    entered: bool,
    /// A field takes typing.
    pub active: bool,
    /// The field: its place (points), what it takes, its text as last known.
    area: [i32; 4],
    options: ImeOptions,
    field: TextEditState,
    /// What the input method was last told (none: it isn't enabled).
    told: Option<(Surrounding, [i32; 4], ImeOptions)>,
    /// The commits sent: what the input method's `done` counts.
    commits: u32,
    pending: Pending,
    answer: Answer,
    /// A composing text shown (one to clear when the next `done` has none).
    composing: bool,
    /// The requests made (the tests read them).
    #[cfg(test)]
    sent: Vec<Request>,
}

impl TextInput {
    /// The host's `text_input`: a field takes typing (or none does), over `area` (points), wanting `options`; a
    /// field beginning to take it (after none did: the host tells a field's end before another's start) enables the
    /// input method afresh, a field going on taking it tells it what changed.
    pub fn set(&mut self, active: bool, area: [i32; 4], state: &TextEditState, options: &ImeOptions) {
        let begins = active && !self.active;
        self.active = active;
        self.area = area;
        self.options = options.clone();
        self.field = state.clone();
        if !active {
            self.composing = false;
            self.pending = Pending::default();
        }
        if begins {
            self.told = None;
        }
        self.tell_now();
    }

    /// The host's `text_input_changed`: the field's text changed by the app.
    pub fn changed(&mut self, state: &TextEditState) {
        self.field = state.clone();
        self.tell_now();
    }

    /// The input method told what changed, as the app's doing, unless a `done` waits for its answer (what changes
    /// while the core applies it is told with the answer).
    fn tell_now(&mut self) {
        if self.answer == Answer::Other {
            self.tell(ZwpTextInputV3ChangeCause::OTHER);
        }
    }

    /// The field as it is after the core handled what came (`state`: the open session's, none if none is open):
    /// the input method told it (as its own doing after its `done`, else as the app's), unless the `done` was stale.
    pub fn answer(&mut self, state: Option<TextEditState>) {
        let answer = std::mem::take(&mut self.answer);
        if let Some(state) = state {
            self.field = state;
        }
        match answer {
            Answer::Other => self.tell(ZwpTextInputV3ChangeCause::OTHER),
            Answer::InputMethod => self.tell(ZwpTextInputV3ChangeCause::INPUT_METHOD),
            Answer::Stale => {}
        }
    }

    /// The input method told what changed (as [`requests`](Self::requests) has it).
    fn tell(&mut self, cause: ZwpTextInputV3ChangeCause) {
        let requests = self.requests(cause);
        #[cfg(test)]
        self.sent.extend(requests.iter().cloned());
        let Some(input) = &self.input else { return };
        for r in requests {
            match r {
                Request::Enable => input.enable(),
                Request::Disable => input.disable(),
                Request::SurroundingText(s) => input.set_surrounding_text(&s.text, s.cursor, s.anchor),
                Request::TextChangeCause(cause) => input.set_text_change_cause(cause),
                Request::ContentType(hint, purpose) => input.set_content_type(hint, purpose),
                Request::CursorRectangle([x, y, w, h]) => input.set_cursor_rectangle(x, y, w, h),
                Request::Commit => input.commit(),
            }
        }
    }

    /// What the input method is to be told, while it is on the window, of what changed since it was last told
    /// (C++'s tellTextInput): enabled with all of it for a field beginning to take typing (or when it came to the
    /// window), else the field's place, kind and surrounding text where they changed (why the text changed:
    /// `cause`); disabled when no field takes typing. Committed (and the commit counted) if anything changed.
    fn requests(&mut self, cause: ZwpTextInputV3ChangeCause) -> Vec<Request> {
        let mut requests = Vec::new();
        if !self.entered {
            return requests;
        }
        if !self.active {
            if self.told.take().is_some() {
                requests.push(Request::Disable);
            }
        } else {
            let surrounding = surrounding(&self.field, SURROUNDING_MAX);
            let content_type = |options: &ImeOptions| {
                let (hint, purpose) = content_type(options);
                Request::ContentType(hint, purpose)
            };
            // (A field's place not told yet (none): not told the input method, which would keep it.)
            let place = (self.area != [0; 4]).then_some(Request::CursorRectangle(self.area));
            match &self.told {
                None => {
                    requests.push(Request::Enable);
                    requests.push(Request::SurroundingText(surrounding.clone()));
                    requests.push(content_type(&self.options));
                    requests.extend(place);
                }
                Some((told, area, options)) => {
                    if *told != surrounding {
                        requests.push(Request::SurroundingText(surrounding.clone()));
                        requests.push(Request::TextChangeCause(cause));
                    }
                    if *options != self.options {
                        requests.push(content_type(&self.options));
                    }
                    if *area != self.area {
                        requests.extend(place);
                    }
                }
            }
            self.told = Some((surrounding, self.area, self.options.clone()));
        }
        if !requests.is_empty() {
            requests.push(Request::Commit);
            self.commits = self.commits.wrapping_add(1);
        }
        requests
    }

    /// The input method came to the window: told the field, if one takes typing.
    fn enter(&mut self) {
        self.entered = true;
        self.told = None;
        self.tell(ZwpTextInputV3ChangeCause::INPUT_METHOD);
    }

    /// The input method left the window: its composing text gone (the protocol's), what it sent since its last
    /// `done` dropped; it isn't told anything until it comes back.
    fn leave(&mut self) -> Vec<Event> {
        self.entered = false;
        self.told = None;
        self.pending = Pending::default();
        self.answer = Answer::Other;
        let composing = std::mem::take(&mut self.composing) && self.active;
        if composing { vec![Event::Preedit { text: String::new(), cursor: None }] } else { Vec::new() }
    }

    /// The input method's `done` (of the state its `serial`th commit gave it): what it sent, in the protocol's order —
    /// the composing text replaced by the caret (done here only before text deleted around it: a commit or a new
    /// composing text replace it themselves), the text around the caret deleted, the commit inserted with the caret
    /// after it, the new composing text with its caret in it — as the core's events.
    fn done(&mut self, serial: u32) -> Vec<Event> {
        let pending = std::mem::take(&mut self.pending);
        let mut events = Vec::new();
        if !self.active {
            return events;
        }
        self.answer = if serial == self.commits { Answer::InputMethod } else { Answer::Stale };
        if let Some((before, after)) = pending.delete.filter(|&(before, after)| before > 0 || after > 0) {
            if std::mem::take(&mut self.composing) {
                events.push(Event::Preedit { text: String::new(), cursor: None });
            }
            events.push(Event::DeleteSurroundingText { before, after });
        }
        if let Some(text) = pending.commit.filter(|t| !t.is_empty()) {
            events.push(Event::Text(text));
            self.composing = false;
        }
        match pending.preedit.filter(|(text, _)| !text.is_empty()) {
            Some((text, cursor)) => {
                events.push(Event::Preedit { text, cursor });
                self.composing = true;
            }
            None if self.composing => {
                events.push(Event::Preedit { text: String::new(), cursor: None });
                self.composing = false;
            }
            None => {}
        }
        if events.is_empty() {
            self.answer = Answer::Other; // (nothing changed: nothing to answer)
        }
        events
    }
}

impl WaylandPlatform {
    /// The pointer's event (C++'s pointer listener): on the window's surface the app's; over one of KompozR's edges
    /// ([`decorations`](super::decorations)) the resize cursor, the left button resizing the window by it.
    pub(super) fn on_pointer(&mut self, e: WlPointerEvent) {
        let window = self.win.window.as_ref().map(|w| w.surface.id());
        let on_window = |id| window.is_some_and(|w| w == id);
        let time = sys::monotonic_now();
        let modifiers = self.keyboard.modifiers();
        match e {
            WlPointerEvent::Enter { serial, surface, surface_x, surface_y } => {
                self.pointer_on = surface;
                if let Some(edge) = self.edge_of(surface) {
                    self.over_edge(edge, surface_x, surface_y, Some(serial));
                    return;
                }
                // (This window's cursor, at its scale.)
                let icon = self.shared.icons.borrow().get(&self.win.id).copied().unwrap_or_default();
                let scale = self.scale().ceil() as i32;
                {
                    let mut cursor = self.shared.cursor.borrow_mut();
                    cursor.set_scale(scale);
                    cursor.set_icon(icon);
                    cursor.entered(serial);
                }
                self.shared.tablet.borrow_mut().set_pointer_icon(icon);
                if !on_window(surface) {
                    return;
                }
                (self.pointer_x, self.pointer_y) = (surface_x, surface_y);
                let (x, y) = (surface_x as f32, surface_y as f32);
                self.win.events.push_back(Event::MouseMove { x, y, modifiers, time });
            }
            WlPointerEvent::Leave { surface, .. } => {
                self.pointer_on = Default::default();
                self.shared.cursor.borrow_mut().left();
                if on_window(surface) {
                    self.win.events.push_back(Event::PointerLeft);
                }
            }
            WlPointerEvent::Motion { surface_x, surface_y, .. } => {
                if let Some(edge) = self.edge_of(self.pointer_on) {
                    self.over_edge(edge, surface_x, surface_y, None);
                    return;
                }
                if !on_window(self.pointer_on) {
                    return;
                }
                (self.pointer_x, self.pointer_y) = (surface_x, surface_y);
                let (x, y) = (surface_x as f32, surface_y as f32);
                self.win.events.push_back(Event::MouseMove { x, y, modifiers, time });
            }
            WlPointerEvent::Button { serial, button, state, .. } => {
                self.last_serial = serial;
                let pressed = state == WlPointerButtonState::PRESSED;
                if !pressed && self.outgoing.is_some() {
                    // (A drag of the app's own the compositor takes the release from us: let go before it took the
                    // drag, which then never started, unless the compositor told its source about it.)
                    self.drag_press_released();
                }
                if pressed {
                    self.press_serial = serial;
                    self.press_touch = None;
                }
                if self.edge_of(self.pointer_on).is_some() {
                    // (An edge: the left button resizes the window by it.)
                    if pressed && mouse_button(button) == Some(MouseButton::Left) {
                        self.resize_from_edge(serial, self.win.edge_x, self.win.edge_y);
                    }
                    return;
                }
                if !on_window(self.pointer_on) {
                    return;
                }
                let Some(button) = mouse_button(button) else { return };
                let (x, y) = (self.pointer_x as f32, self.pointer_y as f32);
                self.win.events.push_back(if pressed {
                    Event::MouseDown { x, y, button, modifiers, time }
                } else {
                    Event::MouseUp { x, y, button, modifiers, time }
                });
            }
            WlPointerEvent::Axis { axis, value, .. } => self.wheel.axis(axis, value),
            WlPointerEvent::AxisDiscrete { axis, discrete } => self.wheel.value120(axis, discrete * 120),
            WlPointerEvent::AxisValue120 { axis, value120 } => self.wheel.value120(axis, value120),
            WlPointerEvent::Frame => {
                if let Some((dx, dy)) = self.wheel.frame()
                    && on_window(self.pointer_on)
                {
                    let (x, y) = (self.pointer_x as f32, self.pointer_y as f32);
                    self.win.events.push_back(Event::Wheel { x, y, dx, dy, modifiers, time });
                }
            }
            _ => {}
        }
    }

    /// A touchpad pinch (pointer-gestures), about the pointer: each step as the change since the last, the
    /// fingers' turning too (C++'s; its end told as well).
    pub(super) fn on_pinch(&mut self, e: ZwpPointerGesturePinchV1Event) {
        let (x, y) = (self.pointer_x as f32, self.pointer_y as f32);
        match e {
            ZwpPointerGesturePinchV1Event::Begin { .. } => {
                self.pinch.begin();
                self.win.events.push_back(Event::PinchBegin { x, y });
            }
            ZwpPointerGesturePinchV1Event::Update { scale, rotation, .. } => {
                let scale = self.pinch.update(scale);
                self.win.events.push_back(Event::Pinch { x, y, scale, rotation: rotation as f32 });
            }
            ZwpPointerGesturePinchV1Event::End { .. } => self.win.events.push_back(Event::PinchEnd { x, y }),
        }
    }

    /// The keyboard's event (C++'s keyboard listener, through [`Keyboard`](crate::linux::xkb::Keyboard)).
    pub(super) fn on_keyboard(&mut self, e: WlKeyboardEvent) {
        match e {
            WlKeyboardEvent::Keymap { format, fd, size } => {
                if format != WlKeyboardKeymapFormat::XKB_V1 {
                    return;
                }
                let Ok(map) = Mmap::private_read(fd.as_fd(), size as usize) else { return };
                let text = String::from_utf8_lossy(map.as_slice());
                if let Some(keymap) = self.xkb.keymap_from_string(&text) {
                    self.keyboard.set_keymap(keymap);
                }
            }
            WlKeyboardEvent::Enter { serial, .. } => {
                self.last_serial = serial;
                self.win.events.push_back(Event::FocusGained);
            }
            WlKeyboardEvent::Leave { .. } => {
                let timer = self.keyboard.leave();
                self.repeat(timer);
                self.win.events.push_back(Event::FocusLost);
            }
            WlKeyboardEvent::Key { serial, key, state, .. } => {
                self.last_serial = serial;
                let down = state == WlKeyboardKeyState::PRESSED;
                if key == KEY_ESC && self.outgoing.is_some() {
                    // (Escape calls a drag of the app's own off, where the compositor passes it on: as within the
                    // app.)
                    if down {
                        self.cancel_outgoing_drag();
                    }
                    return;
                }
                let typing = self.shared.text.borrow().active;
                // (evdev's codes are 8 below X's, which xkbcommon takes.)
                let (events, timer) = self.keyboard.key(key + 8, down, sys::monotonic_now(), typing);
                self.win.events.extend(events);
                self.repeat(timer);
            }
            WlKeyboardEvent::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                self.keyboard.update_mask(mods_depressed, mods_latched, mods_locked, group);
            }
            WlKeyboardEvent::RepeatInfo { rate, delay } => self.keyboard.repeat().set_info(rate, delay),
        }
    }

    /// A finger's event (C++'s touch listener): on the window's surface the app's; one put down on an edge resizes
    /// the window by it (nothing more of it is the app's); a cancel lifts them all.
    pub(super) fn on_touch(&mut self, e: WlTouchEvent) {
        let time = sys::monotonic_now();
        match e {
            WlTouchEvent::Down { serial, surface, id, x, y, .. } => {
                self.last_serial = serial;
                if let Some(edge) = self.edge_of(surface) {
                    let [ex, ey, ..] = self.win.window.as_ref().map_or([0; 4], |w| w.edges.place(edge));
                    self.resize_from_edge(serial, ex as f64 + x, ey as f64 + y);
                    return;
                }
                if !self.win.window.as_ref().is_some_and(|w| w.surface.id() == surface) {
                    return;
                }
                self.press_serial = serial;
                self.press_touch = Some(id);
                self.touches.insert(id, (x, y));
                self.win.events.push_back(Event::TouchDown { pointer: id as i64, x: x as f32, y: y as f32, time });
            }
            WlTouchEvent::Up { id, .. } => {
                if self.outgoing.is_some() && self.press_touch == Some(id) {
                    self.drag_press_released(); // (as a release: some compositors pass the lift on after the drag)
                }
                // (Not one of the window's (an edge's), or one let go for a move.)
                let Some((x, y)) = self.touches.remove(&id) else { return };
                self.win.events.push_back(Event::TouchUp { pointer: id as i64, x: x as f32, y: y as f32, time });
            }
            WlTouchEvent::Motion { id, x, y, .. } => {
                let Some(at) = self.touches.get_mut(&id) else { return };
                *at = (x, y);
                self.win.events.push_back(Event::TouchMove { pointer: id as i64, x: x as f32, y: y as f32, time });
            }
            WlTouchEvent::Cancel => {
                let lifted = self.touches_lifted();
                self.win.events.extend(lifted);
            }
            _ => {}
        }
    }

    /// Every finger on the window lifted (a touch sequence cancelled, the touch screen gone).
    pub(super) fn touches_lifted(&mut self) -> Vec<Event> {
        let time = sys::monotonic_now();
        let touches = std::mem::take(&mut self.touches);
        touches
            .into_iter()
            .map(|(id, (x, y))| Event::TouchUp { pointer: id as i64, x: x as f32, y: y as f32, time })
            .collect()
    }

    /// An input method's typing (text-input-v3): gathered until its `done`; it coming to the window (on the window's
    /// surface: told the field) and leaving it.
    pub(super) fn on_text_input(&mut self, e: ZwpTextInputV3Event) {
        let window = self.win.window.as_ref().map(|w| w.surface.id());
        let mut text = self.shared.text.borrow_mut();
        let events = match e {
            ZwpTextInputV3Event::Enter { surface } => {
                if window == Some(surface) {
                    text.enter();
                }
                Vec::new()
            }
            ZwpTextInputV3Event::Leave { .. } => text.leave(),
            ZwpTextInputV3Event::PreeditString { text: preedit, cursor_begin, cursor_end } => {
                let preedit = preedit.unwrap_or_default();
                let cursor = preedit_cursor(&preedit, cursor_begin, cursor_end);
                text.pending.preedit = Some((preedit, cursor));
                Vec::new()
            }
            ZwpTextInputV3Event::CommitString { text: commit } => {
                text.pending.commit = commit;
                Vec::new()
            }
            ZwpTextInputV3Event::DeleteSurroundingText { before_length, after_length } => {
                text.pending.delete = Some((before_length as usize, after_length as usize));
                Vec::new()
            }
            ZwpTextInputV3Event::Done { serial } => text.done(serial),
        };
        drop(text);
        self.win.events.extend(events);
    }

    /// After the core handled typing (an input method's, or the keyboard's): the input method told the field as it
    /// is now (`state`, the open session's).
    pub(super) fn answer_input_method(&mut self, state: Option<TextEditState>) {
        self.shared.text.borrow_mut().answer(state);
        let _ = self.connection.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_are_evdevs() {
        assert_eq!(mouse_button(0x110), Some(MouseButton::Left));
        assert_eq!(mouse_button(0x111), Some(MouseButton::Right));
        assert_eq!(mouse_button(0x112), Some(MouseButton::Middle));
        assert_eq!(mouse_button(0x113), Some(MouseButton::Back));
        assert_eq!(mouse_button(0x114), Some(MouseButton::Forward));
        assert_eq!(mouse_button(0x115), None);
    }

    #[test]
    fn wheel_notches_and_touchpad_scrolls() {
        let mut w = Wheel::default();
        // A notch down: its distance (15) and its 120ths come; the notch counts, down negative.
        w.axis(WlPointerAxis::VERTICAL_SCROLL, 15.0);
        w.value120(WlPointerAxis::VERTICAL_SCROLL, 120);
        w.axis(WlPointerAxis::VERTICAL_SCROLL, 15.0);
        assert_eq!(w.frame(), Some((0.0, -1.0)));
        // A high-resolution wheel's half notch to the right.
        w.value120(WlPointerAxis::HORIZONTAL_SCROLL, 60);
        assert_eq!(w.frame(), Some((0.5, 0.0)));
        // A touchpad's scroll: a tenth of its distance; up positive.
        w.axis(WlPointerAxis::VERTICAL_SCROLL, -25.0);
        w.axis(WlPointerAxis::HORIZONTAL_SCROLL, 5.0);
        assert_eq!(w.frame(), Some((0.5, 2.5)));
        assert_eq!(w.frame(), None);
    }

    #[test]
    fn pinches_tell_the_change_since_the_last() {
        let mut p = Pinch::default();
        p.begin();
        assert_eq!(p.update(1.5), 1.5);
        assert_eq!(p.update(3.0), 2.0);
        assert_eq!(p.update(1.5), 0.5);
        p.begin();
        assert_eq!(p.update(0.5), 0.5);
    }

    #[test]
    fn fields_tell_their_content_type() {
        type Hint = ZwpTextInputV3ContentHint;
        type Purpose = ZwpTextInputV3ContentPurpose;
        let options = |keyboard_type, single_line, auto_correct| ImeOptions {
            keyboard_type,
            single_line,
            auto_correct,
            ..ImeOptions::default()
        };
        assert_eq!(
            content_type(&options(KeyboardType::Text, true, true)),
            (Hint::COMPLETION | Hint::SPELLCHECK, Purpose::NORMAL)
        );
        assert_eq!(content_type(&options(KeyboardType::Email, false, false)), (Hint::MULTILINE, Purpose::EMAIL));
        assert_eq!(
            content_type(&options(KeyboardType::NumberPassword, true, true)),
            (Hint::HIDDEN_TEXT | Hint::SENSITIVE_DATA, Purpose::PIN)
        );
        assert_eq!(content_type(&options(KeyboardType::Decimal, true, false)).1, Purpose::NUMBER);
        assert_eq!(content_type(&options(KeyboardType::Number, true, false)).1, Purpose::DIGITS);
        // Compose 1.12's kinds and the capitalization.
        assert_eq!(
            content_type(&options(KeyboardType::PasswordVisible, true, false)),
            (Hint::SENSITIVE_DATA, Purpose::PASSWORD)
        );
        assert_eq!(content_type(&options(KeyboardType::PersonName, true, false)).1, Purpose::NAME);
        assert_eq!(content_type(&options(KeyboardType::Date, true, false)).1, Purpose::DATE);
        assert_eq!(content_type(&options(KeyboardType::Ascii, true, false)), (Hint::LATIN, Purpose::NORMAL));
        assert_eq!(
            content_type(&options(KeyboardType::DecimalPasswordSigned, true, true)),
            (Hint::HIDDEN_TEXT | Hint::SENSITIVE_DATA, Purpose::NUMBER)
        );
        let caps = |capitalization| ImeOptions { capitalization, auto_correct: false, ..ImeOptions::default() };
        assert_eq!(content_type(&caps(KeyboardCapitalization::Characters)).0, Hint::UPPERCASE);
        assert_eq!(content_type(&caps(KeyboardCapitalization::Words)).0, Hint::TITLECASE);
        assert_eq!(content_type(&caps(KeyboardCapitalization::Sentences)).0, Hint::AUTO_CAPITALIZATION);
        assert_eq!(content_type(&caps(KeyboardCapitalization::None)).0, Hint::NONE);
    }

    fn field(text: &str, selection: (usize, usize), composition: Option<(usize, usize)>) -> TextEditState {
        TextEditState { text: text.into(), selection_start: selection.0, selection_end: selection.1, composition }
    }

    fn told(text: &str, cursor: i32, anchor: i32) -> Surrounding {
        Surrounding { text: text.into(), cursor, anchor }
    }

    #[test]
    fn surrounding_text_is_cut_around_the_caret_at_whole_characters() {
        // All of a short field: the caret its selection's end, the anchor its start (Compose's).
        assert_eq!(surrounding(&field("merhaba", (2, 5), None), 4000), told("merhaba", 5, 2));
        // The composing text left out, the caret in its place.
        assert_eq!(surrounding(&field("abşeyc", (4, 4), Some((2, 6))), 4000), told("abc", 2, 2));
        // At most `max` bytes, half before the caret and half after; near an end, the other side gets the rest.
        let digits = "0123456789";
        assert_eq!(surrounding(&field(digits, (5, 5), None), 4), told("3456", 2, 2));
        assert_eq!(surrounding(&field(digits, (1, 1), None), 4), told("0123", 1, 1));
        assert_eq!(surrounding(&field(digits, (9, 9), None), 4), told("6789", 3, 3));
        // A selection with room: around it.
        assert_eq!(surrounding(&field(digits, (2, 6), None), 6), told("123456", 5, 1));
        // A selection longer than fits: its part at the caret, the anchor at the far edge, either way round.
        assert_eq!(surrounding(&field(digits, (1, 9), None), 4), told("5678", 4, 0));
        assert_eq!(surrounding(&field(digits, (9, 1), None), 4), told("1234", 0, 4));
        // Cut inward at whole characters (ş and ğ are two bytes each); a caret a byte into one: at its start.
        assert_eq!(surrounding(&field("aşbğc", (3, 3), None), 4), told("şb", 2, 2));
        assert_eq!(surrounding(&field("aşb", (2, 2), None), 4000), told("aşb", 1, 1));
        // The protocol's 4000 bytes.
        let long = "ç".repeat(3000);
        let s = surrounding(&field(&long, (3001, 3001), None), SURROUNDING_MAX);
        assert_eq!((s.text.len(), s.cursor, s.anchor), (4000, 2000, 2000));
        assert_eq!(surrounding(&TextEditState::default(), SURROUNDING_MAX), told("", 0, 0));
    }

    #[test]
    fn preedit_carets_are_whole_characters_in_order() {
        assert_eq!(preedit_cursor("şey", -1, -1), None);
        assert_eq!(preedit_cursor("şey", 0, -1), None);
        assert_eq!(preedit_cursor("şey", 2, 2), Some((2, 2)));
        assert_eq!(preedit_cursor("şey", 1, 1), Some((0, 0))); // (a byte into ş: its start)
        assert_eq!(preedit_cursor("şey", 3, 1), Some((0, 3))); // (in order, out to whole characters)
        assert_eq!(preedit_cursor("şey", 0, 1), Some((0, 2)));
        assert_eq!(preedit_cursor("şey", 9, 9), Some((4, 4)));
    }

    #[test]
    fn input_methods_are_applied_at_done_in_the_protocols_order() {
        let mut t = TextInput { active: true, ..TextInput::default() };
        t.pending.preedit = Some(("ş".into(), Some((2, 2))));
        assert_eq!(t.done(0), [Event::Preedit { text: "ş".into(), cursor: Some((2, 2)) }]);
        // Text before the caret deleted and a commit: the composing text gone first (the caret in its place), the
        // deletion, then the commit.
        t.pending.delete = Some((1, 0));
        t.pending.commit = Some("şu".into());
        assert_eq!(
            t.done(0),
            [
                Event::Preedit { text: String::new(), cursor: None },
                Event::DeleteSurroundingText { before: 1, after: 0 },
                Event::Text("şu".into())
            ]
        );
        // A commit replaces the composing text itself; a new composing text after it.
        t.pending.preedit = Some(("x".into(), None));
        t.done(0);
        t.pending.commit = Some("y".into());
        t.pending.preedit = Some(("z".into(), Some((0, 1))));
        assert_eq!(t.done(0), [Event::Text("y".into()), Event::Preedit { text: "z".into(), cursor: Some((0, 1)) }]);
        // A composing text gone with nothing committed: cleared.
        assert_eq!(t.done(0), [Event::Preedit { text: String::new(), cursor: None }]);
        assert_eq!(t.done(0), []);
        // Nothing to delete: nothing.
        t.pending.delete = Some((0, 0));
        assert_eq!(t.done(0), []);
        // No field takes typing: nothing.
        t.active = false;
        t.pending.commit = Some("y".into());
        assert_eq!(t.done(0), []);
    }

    #[test]
    fn the_input_method_is_told_the_field_and_its_changes_and_answered() {
        type Cause = ZwpTextInputV3ChangeCause;
        let mut t = TextInput::default();
        let text = ImeOptions::default();
        let content = |options: &ImeOptions| {
            let (hint, purpose) = content_type(options);
            Request::ContentType(hint, purpose)
        };
        let surrounding = |text: &str, cursor, anchor| Request::SurroundingText(told(text, cursor, anchor));
        // A field takes typing before the input method is on the window: told when it comes, all of it.
        t.set(true, [1, 2, 3, 4], &field("ab", (2, 2), None), &text);
        assert_eq!(t.sent, []);
        t.enter();
        assert_eq!(
            std::mem::take(&mut t.sent),
            [
                Request::Enable,
                surrounding("ab", 2, 2),
                content(&text),
                Request::CursorRectangle([1, 2, 3, 4]),
                Request::Commit
            ]
        );
        // The caret's place moved: only that; the same again: nothing.
        t.set(true, [5, 2, 3, 4], &field("ab", (2, 2), None), &text);
        t.set(true, [5, 2, 3, 4], &field("ab", (2, 2), None), &text);
        assert_eq!(std::mem::take(&mut t.sent), [Request::CursorRectangle([5, 2, 3, 4]), Request::Commit]);
        // The app changed the text: told as the app's doing.
        t.changed(&field("abc", (3, 3), None));
        assert_eq!(
            std::mem::take(&mut t.sent),
            [surrounding("abc", 3, 3), Request::TextChangeCause(Cause::OTHER), Request::Commit]
        );
        assert_eq!(t.commits, 3);
        // The input method's commit, at its done of the third commit: answered with the field after it, as its own
        // doing. Composing changes nothing it is told (the composing text is left out).
        t.pending.commit = Some("d".into());
        assert_eq!(t.done(3), [Event::Text("d".into())]);
        t.answer(Some(field("abcd", (4, 4), None)));
        assert_eq!(
            std::mem::take(&mut t.sent),
            [surrounding("abcd", 4, 4), Request::TextChangeCause(Cause::INPUT_METHOD), Request::Commit]
        );
        t.pending.preedit = Some(("ş".into(), None));
        assert_eq!(t.done(4).len(), 1);
        t.answer(Some(field("abcdş", (6, 6), Some((4, 6)))));
        assert_eq!(t.sent, []);
        // While a done waits for its answer, what the app changes meanwhile (the caret's place) goes with it.
        t.pending.commit = Some("e".into());
        assert_eq!(t.done(4), [Event::Text("e".into())]);
        t.set(true, [6, 2, 3, 4], &field("abcde", (5, 5), None), &text);
        assert_eq!(t.sent, []);
        t.answer(Some(field("abcde", (5, 5), None)));
        assert_eq!(
            std::mem::take(&mut t.sent),
            [
                surrounding("abcde", 5, 5),
                Request::TextChangeCause(Cause::INPUT_METHOD),
                Request::CursorRectangle([6, 2, 3, 4]),
                Request::Commit
            ]
        );
        // A done of an older commit (more was committed since): applied, not answered; the app's next change tells
        // all that changed.
        t.pending.commit = Some("f".into());
        assert_eq!(t.done(2), [Event::Text("f".into())]);
        t.answer(Some(field("abcdef", (6, 6), None)));
        assert_eq!(t.sent, []);
        t.changed(&field("abcdefg", (7, 7), None));
        assert_eq!(
            std::mem::take(&mut t.sent),
            [surrounding("abcdefg", 7, 7), Request::TextChangeCause(Cause::OTHER), Request::Commit]
        );
        // Keyboard typing (no done): told as the app's.
        t.answer(Some(field("abcdefgh", (8, 8), None)));
        assert_eq!(
            std::mem::take(&mut t.sent),
            [surrounding("abcdefgh", 8, 8), Request::TextChangeCause(Cause::OTHER), Request::Commit]
        );
        // Another field (the host ends one before the next begins): disabled, enabled afresh for it.
        let email = ImeOptions { keyboard_type: KeyboardType::Email, ..ImeOptions::default() };
        t.set(false, [6, 2, 3, 4], &field("abcdefgh", (8, 8), None), &text);
        t.set(true, [0, 0, 1, 1], &field("x", (0, 1), None), &email);
        assert_eq!(
            std::mem::take(&mut t.sent),
            [
                Request::Disable,
                Request::Commit,
                Request::Enable,
                surrounding("x", 1, 0),
                content(&email),
                Request::CursorRectangle([0, 0, 1, 1]),
                Request::Commit
            ]
        );
        assert_eq!(t.commits, 9);
        // It leaves while composing: the composing text gone; nothing told until it comes back, then all again.
        t.pending.preedit = Some(("k".into(), None));
        assert_eq!(t.done(9).len(), 1);
        assert_eq!(t.leave(), [Event::Preedit { text: String::new(), cursor: None }]);
        t.set(true, [0, 0, 2, 2], &field("x", (1, 1), None), &email);
        assert_eq!(t.sent, []);
        t.enter();
        assert_eq!(std::mem::take(&mut t.sent)[0], Request::Enable);
        // A field whose place isn't known yet: none told (the input method would keep it).
        t.set(false, [0, 0, 2, 2], &field("x", (1, 1), None), &email);
        t.set(true, [0, 0, 0, 0], &field("y", (1, 1), None), &text);
        assert_eq!(
            std::mem::take(&mut t.sent),
            [
                Request::Disable,
                Request::Commit,
                Request::Enable,
                surrounding("y", 1, 1),
                content(&text),
                Request::Commit
            ]
        );
        // No field takes typing when it leaves: nothing to clear.
        t.set(false, [0, 0, 0, 0], &field("y", (1, 1), None), &text);
        assert_eq!(std::mem::take(&mut t.sent), [Request::Disable, Request::Commit]);
        assert_eq!(t.leave(), []);
    }
}

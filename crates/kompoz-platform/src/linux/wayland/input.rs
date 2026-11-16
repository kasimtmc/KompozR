//! The seat's input (C++'s pointer, keyboard, touch, pinch and text-input listeners): the pointer's moves, buttons
//! and wheel (notches and a touchpad's scrolls gathered until its frame), the keyboard through xkbcommon (its
//! keymap handed over, compose sequences, repeated client-side by the event loop's timer), fingers on a touch
//! screen, a touchpad's pinches, and an input method's typing (text-input-v3: what it commits, its composing text
//! and what it deletes, applied at its `done` as the protocol orders them).
//!
//! What KompozR tells the input method of the field (its surrounding text, why it changed) is WP4's, at the seam
//! [`TextInput::changed`]; here: whether a field takes typing, where it is, and what it takes.

use super::WaylandPlatform;
use super::ffi::Object;
use super::generated::*;
use crate::app::{Event, MouseButton};
use crate::linux::sys::{self, Mmap};
use kompoz_ui::platform::{ImeOptions, KeyboardType, TextEditState};
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

/// text-input-v3's content hint and purpose for a field's options (C++'s tellTextInput).
pub(super) fn content_type(options: &ImeOptions) -> (ZwpTextInputV3ContentHint, ZwpTextInputV3ContentPurpose) {
    type Hint = ZwpTextInputV3ContentHint;
    type Purpose = ZwpTextInputV3ContentPurpose;
    let mut hint = if options.auto_correct { Hint::COMPLETION | Hint::SPELLCHECK } else { Hint::NONE };
    let purpose = match options.keyboard_type {
        KeyboardType::Unspecified | KeyboardType::Text | KeyboardType::Ascii => Purpose::NORMAL,
        KeyboardType::Number => Purpose::DIGITS,
        KeyboardType::Decimal => Purpose::NUMBER,
        KeyboardType::Phone => Purpose::PHONE,
        KeyboardType::Uri => Purpose::URL,
        KeyboardType::Email => Purpose::EMAIL,
        KeyboardType::Password | KeyboardType::NumberPassword => {
            hint = Hint::HIDDEN_TEXT | Hint::SENSITIVE_DATA;
            if options.keyboard_type == KeyboardType::Password { Purpose::PASSWORD } else { Purpose::PIN }
        }
    };
    if !options.single_line {
        hint |= Hint::MULTILINE;
    }
    (hint, purpose)
}

/// What an input method sent since its last `done`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Pending {
    /// The composing text and its caret (bytes; none: hidden).
    preedit: Option<(String, Option<usize>)>,
    commit: Option<String>,
    /// Bytes before and after the caret.
    delete: Option<(usize, usize)>,
}

/// The input method on the window (text-input-v3), as the host's callbacks and the platform share it.
#[derive(Default)]
pub(crate) struct TextInput {
    pub input: Option<ZwpTextInputV3>,
    /// The input method is on the window (its enter came).
    entered: bool,
    /// A field takes typing.
    pub active: bool,
    /// The field, in points.
    area: [i32; 4],
    options: ImeOptions,
    pending: Pending,
    /// A composing text shown (one to clear when the next `done` has none).
    composing: bool,
}

impl TextInput {
    /// The host's `text_input`: a field takes typing (or none does), over `area` (points), wanting `options`.
    pub fn set(&mut self, active: bool, area: [i32; 4], state: &TextEditState, options: &ImeOptions) {
        self.active = active;
        self.area = area;
        self.options = options.clone();
        self.changed(state);
        self.tell();
    }

    /// The host's `text_input_changed`: the field's text changed by the app. What the input method is told of it
    /// (`set_surrounding_text`, `set_text_change_cause`) is WP4's full text-input-v3; here nothing more than the
    /// field's place and type.
    pub fn changed(&mut self, state: &TextEditState) {
        let _ = state;
    }

    /// The input method told whether a field takes typing, where it is and what it takes, while it is on the window
    /// (C++'s tellTextInput).
    fn tell(&self) {
        let Some(input) = &self.input else { return };
        if !self.entered {
            return;
        }
        if self.active {
            input.enable();
            let [x, y, w, h] = self.area;
            input.set_cursor_rectangle(x, y, w, h);
            let (hint, purpose) = content_type(&self.options);
            input.set_content_type(hint, purpose);
        } else {
            input.disable();
        }
        input.commit();
    }

    /// The input method's `done`: what it sent, in the protocol's order (the composing text replaced; the text
    /// around the caret deleted; the commit inserted; the new composing text), as the core's events.
    fn done(&mut self) -> Vec<Event> {
        let pending = std::mem::take(&mut self.pending);
        let mut events = Vec::new();
        if !self.active {
            return events;
        }
        if let Some((before, after)) = pending.delete {
            events.push(Event::DeleteSurroundingText { before, after });
        }
        if let Some(text) = pending.commit.filter(|t| !t.is_empty()) {
            events.push(Event::Text(text));
            self.composing = false; // (a commit replaces the composing text)
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
        events
    }
}

impl WaylandPlatform {
    /// The pointer's event (C++'s pointer listener): on the window's surface the app's; over one of KompozR's edges
    /// ([`decorations`](super::decorations)) the resize cursor, the left button resizing the window by it.
    pub(super) fn on_pointer(&mut self, e: WlPointerEvent) {
        let window = self.window.as_ref().map(|w| w.surface.id());
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
                self.shared.cursor.borrow_mut().entered(serial);
                if !on_window(surface) {
                    return;
                }
                (self.pointer_x, self.pointer_y) = (surface_x, surface_y);
                let (x, y) = (surface_x as f32, surface_y as f32);
                self.events.push_back(Event::MouseMove { x, y, modifiers, time });
            }
            WlPointerEvent::Leave { surface, .. } => {
                self.pointer_on = Default::default();
                self.shared.cursor.borrow_mut().left();
                if on_window(surface) {
                    self.events.push_back(Event::PointerLeft);
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
                self.events.push_back(Event::MouseMove { x, y, modifiers, time });
            }
            WlPointerEvent::Button { serial, button, state, .. } => {
                self.last_serial = serial;
                let pressed = state == WlPointerButtonState::PRESSED;
                if pressed {
                    self.press_serial = serial;
                    self.press_touch = None;
                }
                if self.edge_of(self.pointer_on).is_some() {
                    // (An edge: the left button resizes the window by it.)
                    if pressed && mouse_button(button) == Some(MouseButton::Left) {
                        self.resize_from_edge(serial, self.edge_x, self.edge_y);
                    }
                    return;
                }
                if !on_window(self.pointer_on) {
                    return;
                }
                let Some(button) = mouse_button(button) else { return };
                let (x, y) = (self.pointer_x as f32, self.pointer_y as f32);
                self.events.push_back(if pressed {
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
                    self.events.push_back(Event::Wheel { x, y, dx, dy, modifiers, time });
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
                self.events.push_back(Event::PinchBegin { x, y });
            }
            ZwpPointerGesturePinchV1Event::Update { scale, rotation, .. } => {
                let scale = self.pinch.update(scale);
                self.events.push_back(Event::Pinch { x, y, scale, rotation: rotation as f32 });
            }
            ZwpPointerGesturePinchV1Event::End { .. } => self.events.push_back(Event::PinchEnd { x, y }),
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
                self.events.push_back(Event::FocusGained);
            }
            WlKeyboardEvent::Leave { .. } => {
                let timer = self.keyboard.leave();
                self.repeat(timer);
                self.events.push_back(Event::FocusLost);
            }
            WlKeyboardEvent::Key { serial, key, state, .. } => {
                self.last_serial = serial;
                let down = state == WlKeyboardKeyState::PRESSED;
                let typing = self.shared.text.borrow().active;
                // (evdev's codes are 8 below X's, which xkbcommon takes.)
                let (events, timer) = self.keyboard.key(key + 8, down, sys::monotonic_now(), typing);
                self.events.extend(events);
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
                    let [ex, ey, ..] = self.window.as_ref().map_or([0; 4], |w| w.edges.place(edge));
                    self.resize_from_edge(serial, ex as f64 + x, ey as f64 + y);
                    return;
                }
                if !self.window.as_ref().is_some_and(|w| w.surface.id() == surface) {
                    return;
                }
                self.press_serial = serial;
                self.press_touch = Some(id);
                self.touches.insert(id, (x, y));
                self.events.push_back(Event::TouchDown { pointer: id as i64, x: x as f32, y: y as f32, time });
            }
            WlTouchEvent::Up { id, .. } => {
                // (Not one of the window's (an edge's), or one let go for a move.)
                let Some((x, y)) = self.touches.remove(&id) else { return };
                self.events.push_back(Event::TouchUp { pointer: id as i64, x: x as f32, y: y as f32, time });
            }
            WlTouchEvent::Motion { id, x, y, .. } => {
                let Some(at) = self.touches.get_mut(&id) else { return };
                *at = (x, y);
                self.events.push_back(Event::TouchMove { pointer: id as i64, x: x as f32, y: y as f32, time });
            }
            WlTouchEvent::Cancel => {
                let lifted = self.touches_lifted();
                self.events.extend(lifted);
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

    /// An input method's typing (text-input-v3): gathered until its `done`.
    pub(super) fn on_text_input(&mut self, e: ZwpTextInputV3Event) {
        let mut text = self.shared.text.borrow_mut();
        match e {
            ZwpTextInputV3Event::Enter { .. } => {
                text.entered = true;
                text.tell();
            }
            ZwpTextInputV3Event::Leave { .. } => {
                text.entered = false;
                if let Some(input) = &text.input {
                    input.disable();
                    input.commit();
                }
            }
            ZwpTextInputV3Event::PreeditString { text: preedit, cursor_begin, .. } => {
                text.pending.preedit = Some((preedit.unwrap_or_default(), usize::try_from(cursor_begin).ok()));
            }
            ZwpTextInputV3Event::CommitString { text: commit } => text.pending.commit = commit,
            ZwpTextInputV3Event::DeleteSurroundingText { before_length, after_length } => {
                text.pending.delete = Some((before_length as usize, after_length as usize));
            }
            ZwpTextInputV3Event::Done { .. } => {
                let events = text.done();
                drop(text);
                self.events.extend(events);
            }
        }
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
    }

    #[test]
    fn input_methods_are_applied_at_done() {
        let mut t = TextInput { active: true, ..TextInput::default() };
        t.pending.preedit = Some(("ş".into(), Some(2)));
        assert_eq!(t.done(), [Event::Preedit { text: "ş".into(), cursor: Some(2) }]);
        // A commit replacing it, after the text before the caret is deleted.
        t.pending.delete = Some((1, 0));
        t.pending.commit = Some("şu".into());
        assert_eq!(t.done(), [Event::DeleteSurroundingText { before: 1, after: 0 }, Event::Text("şu".into())]);
        // A composing text gone with nothing committed: cleared.
        t.pending.preedit = Some(("x".into(), None));
        t.done();
        assert_eq!(t.done(), [Event::Preedit { text: String::new(), cursor: None }]);
        assert_eq!(t.done(), []);
        // No field takes typing: nothing.
        t.active = false;
        t.pending.commit = Some("y".into());
        assert_eq!(t.done(), []);
    }
}

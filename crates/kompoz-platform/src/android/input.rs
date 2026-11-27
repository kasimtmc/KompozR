//! Android's input as KompozR's events (C++'s `motion` and `key`, grown to Compose's `MotionEventAdapter`): a
//! `MotionEvent` as KompozView passes it — every pointer with its tool, position and pressure, the positions each
//! passed through since the last event (its historical samples, batched by Android: they go to the host with the
//! move that ends them, for velocity trackers), the buttons, the wheel and a touchpad's gestures — and a `KeyEvent`
//! as Compose's key (Compose's `Key` is Android's key code: `Key.android.kt`'s `Key(nativeKeyCode)`, every
//! `KEYCODE_*`) with the character it types and the meta state as modifiers.
//!
//! Fingers are touches; a stylus and an eraser are the stylus (pressure, barrel buttons, hovering: one stream from
//! its coming near until it goes, as Linux's tablets); a mouse moves, presses its buttons, turns its wheel, leaves;
//! a touchpad's two-finger swipe scrolls (as a wheel's parts of notches, Linux's touchpad scrolls) and its pinch
//! zooms (Linux's pinch), as Android 14 classifies them. A cancel calls off what the fingers began. A stylus's tilt
//! and orientation aren't carried: the core's events have none (Linux's tablets give none either).
//!
//! Pure functions of what Java passes (the decoder's only state: a touchpad gesture under way): tested on the host.

use crate::app::{Event, MouseButton};
use kompoz_ui::input::{Key, KeyModifiers};

/// `MotionEvent`'s actions (`getActionMasked`).
pub mod action {
    pub const DOWN: i32 = 0;
    pub const UP: i32 = 1;
    pub const MOVE: i32 = 2;
    pub const CANCEL: i32 = 3;
    pub const OUTSIDE: i32 = 4;
    pub const POINTER_DOWN: i32 = 5;
    pub const POINTER_UP: i32 = 6;
    pub const HOVER_MOVE: i32 = 7;
    pub const SCROLL: i32 = 8;
    pub const HOVER_ENTER: i32 = 9;
    pub const HOVER_EXIT: i32 = 10;
    pub const BUTTON_PRESS: i32 = 11;
    pub const BUTTON_RELEASE: i32 = 12;
}

/// `MotionEvent`'s tool types.
pub mod tool {
    pub const UNKNOWN: i32 = 0;
    pub const FINGER: i32 = 1;
    pub const STYLUS: i32 = 2;
    pub const MOUSE: i32 = 3;
    pub const ERASER: i32 = 4;
}

/// `MotionEvent`'s button state bits (`getButtonState`, `getActionButton`).
pub mod button {
    pub const PRIMARY: i32 = 1;
    pub const SECONDARY: i32 = 2;
    pub const TERTIARY: i32 = 4;
    pub const BACK: i32 = 8;
    pub const FORWARD: i32 = 16;
    pub const STYLUS_PRIMARY: i32 = 32;
    pub const STYLUS_SECONDARY: i32 = 64;
}

/// `MotionEvent`'s classifications (`getClassification`; a touchpad's gestures from Android 14).
pub mod classification {
    pub const NONE: i32 = 0;
    pub const TWO_FINGER_SWIPE: i32 = 3;
    pub const PINCH: i32 = 5;
}

/// A `MotionEvent` as KompozView passes it: its masked action, the index of the pointer it is about, each pointer's
/// id, tool, position (px in the view) and pressure, its time (ns, `System.nanoTime`'s clock), the button pressed
/// or released and the buttons held, the wheel's axes, the meta state, its classification and a touchpad gesture's
/// axes (the swipe's distance in px and the pinch's scale since the last event, the historical samples' included),
/// the display's density; then its historical samples, oldest first: each one's time, and each pointer's position
/// in it (`history_xy`: a sample's pointers in order, x then y).
#[derive(Clone, PartialEq, Debug)]
pub struct Motion {
    pub action: i32,
    pub index: usize,
    pub ids: Vec<i32>,
    pub tools: Vec<i32>,
    pub xy: Vec<f32>,
    pub pressure: Vec<f32>,
    pub time: i64,
    pub button: i32,
    pub buttons: i32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub meta: i32,
    pub classification: i32,
    pub gesture_scroll_x: f32,
    pub gesture_scroll_y: f32,
    pub pinch_scale: f32,
    pub density: f32,
    pub history_times: Vec<i64>,
    pub history_xy: Vec<f32>,
}

impl Default for Motion {
    fn default() -> Self {
        Motion {
            action: action::MOVE,
            index: 0,
            ids: Vec::new(),
            tools: Vec::new(),
            xy: Vec::new(),
            pressure: Vec::new(),
            time: 0,
            button: 0,
            buttons: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            meta: 0,
            classification: classification::NONE,
            gesture_scroll_x: 0.0,
            gesture_scroll_y: 0.0,
            pinch_scale: 1.0,
            density: 1.0,
            history_times: Vec::new(),
            history_xy: Vec::new(),
        }
    }
}

/// What the decoder knows of a pointer of the event.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Pointer {
    id: i64,
    tool: i32,
    x: f32,
    y: f32,
    pressure: f32,
}

impl Motion {
    /// How many pointers it has (what every array has room for).
    fn count(&self) -> usize {
        self.ids.len().min(self.xy.len() / 2)
    }

    fn pointer(&self, i: usize) -> Option<Pointer> {
        if i >= self.count() {
            return None;
        }
        Some(Pointer {
            id: self.ids[i] as i64,
            // (A pointer without a tool told: a finger, as Compose's TOOL_TYPE_UNKNOWN is a touch.)
            tool: self.tools.get(i).copied().unwrap_or(tool::FINGER),
            x: self.xy[i * 2],
            y: self.xy[i * 2 + 1],
            pressure: self.pressure.get(i).copied().filter(|p| p.is_finite()).unwrap_or(1.0),
        })
    }

    fn pointers(&self) -> impl Iterator<Item = Pointer> + '_ {
        (0..self.count()).filter_map(|i| self.pointer(i))
    }

    /// Pointer `i`'s positions in the historical samples, oldest first, each with its time.
    fn history_of(&self, i: usize) -> Vec<(f32, f32, i64)> {
        let n = self.count();
        self.history_times
            .iter()
            .enumerate()
            .filter_map(|(h, &t)| {
                let at = (h * n + i) * 2;
                Some((*self.history_xy.get(at)?, *self.history_xy.get(at + 1)?, t))
            })
            .collect()
    }

    /// The `History` event of pointer `i` (as the host knows it: `pointer`), if it passed anywhere before.
    fn history(&self, i: usize, pointer: i64) -> Option<Event> {
        let samples = self.history_of(i);
        (!samples.is_empty()).then_some(Event::History { pointer, samples })
    }
}

/// KeyEvent's meta state as Compose's modifiers (`META_SHIFT_ON`, `META_CTRL_ON`, `META_ALT_ON`, `META_META_ON`).
pub fn modifiers_of(meta: i32) -> KeyModifiers {
    let mut m = KeyModifiers::None;
    if meta & 0x1 != 0 {
        m = m | KeyModifiers::Shift;
    }
    if meta & 0x2 != 0 {
        m = m | KeyModifiers::Alt;
    }
    if meta & 0x1000 != 0 {
        m = m | KeyModifiers::Ctrl;
    }
    if meta & 0x10000 != 0 {
        m = m | KeyModifiers::Meta;
    }
    m
}

/// The mouse button of `MotionEvent.getActionButton`'s bit (`BUTTON_PRIMARY` … `BUTTON_FORWARD`).
fn mouse_button(bit: i32) -> Option<MouseButton> {
    Some(match bit {
        button::PRIMARY => MouseButton::Left,
        button::SECONDARY => MouseButton::Right,
        button::TERTIARY => MouseButton::Middle,
        button::BACK => MouseButton::Back,
        button::FORWARD => MouseButton::Forward,
        _ => return None,
    })
}

/// A stylus's barrel buttons as the core's bits (the first, the second): Android's stylus buttons, or a stylus
/// reporting them as the secondary and tertiary buttons (older devices).
pub fn stylus_buttons(state: i32) -> u32 {
    let mut bits = 0;
    if state & (button::STYLUS_PRIMARY | button::SECONDARY) != 0 {
        bits |= 1;
    }
    if state & (button::STYLUS_SECONDARY | button::TERTIARY) != 0 {
        bits |= 2;
    }
    bits
}

fn is_stylus(tool: i32) -> bool {
    tool == tool::STYLUS || tool == tool::ERASER
}

/// What a touchpad's gesture under way needs between its events: where its swipe's cursor is (the first fake
/// finger's place as it went down: Compose's inferred cursor), whether its pinch began.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct MotionDecoder {
    swipe_at: Option<(f32, f32)>,
    pinch_at: Option<(f32, f32)>,
}

impl MotionDecoder {
    /// What `m` is to KompozR.
    pub fn events(&mut self, m: &Motion) -> Vec<Event> {
        let mut out = Vec::new();
        if m.action == action::CANCEL || m.action == action::OUTSIDE {
            // (The system took the gesture: what it began is called off; a stylus that was on it is gone too.)
            if m.action == action::CANCEL {
                out.push(Event::TouchCancel);
                out.extend(m.pointers().filter(|p| is_stylus(p.tool)).map(|p| Event::StylusLeft { pointer: p.id }));
                if let Some((x, y)) = self.pinch_at.take() {
                    out.push(Event::PinchEnd { x, y });
                }
                self.swipe_at = None;
            }
            return out;
        }
        if m.classification == classification::TWO_FINGER_SWIPE || m.classification == classification::PINCH {
            self.touchpad(m, &mut out);
            return out;
        }
        let Some(at) = m.pointer(m.index) else { return out };
        let modifiers = modifiers_of(m.meta);
        let time = m.time;
        match m.action {
            // The wheel, whatever the tool: a mouse's, or a joystick's axes.
            // (AXIS_VSCROLL is positive away from the user, AXIS_HSCROLL to the right: as the user turns the wheel.)
            action::SCROLL => {
                out.push(Event::Wheel { x: at.x, y: at.y, dx: m.scroll_x, dy: m.scroll_y, modifiers, time })
            }
            action::HOVER_ENTER | action::HOVER_MOVE | action::HOVER_EXIT if is_stylus(at.tool) => {
                if m.action == action::HOVER_EXIT {
                    out.push(Event::StylusLeft { pointer: at.id });
                } else {
                    out.extend(m.history(m.index, at.id));
                    out.push(self.stylus(m, at, false));
                }
            }
            action::HOVER_ENTER | action::HOVER_MOVE => {
                out.extend(m.history(m.index, 0));
                out.push(Event::MouseMove { x: at.x, y: at.y, modifiers, time });
            }
            action::HOVER_EXIT => out.push(Event::PointerLeft),
            action::BUTTON_PRESS | action::BUTTON_RELEASE if is_stylus(at.tool) => {
                // (A barrel button pressed or let go: the stylus's report with its buttons now.)
                let down = m.buttons & button::PRIMARY != 0 || m.action == action::BUTTON_PRESS && m.button == 0;
                out.push(self.stylus(m, at, down && at.pressure > 0.0));
            }
            action::BUTTON_PRESS | action::BUTTON_RELEASE => {
                if let Some(button) = mouse_button(m.button) {
                    let (x, y) = (at.x, at.y);
                    out.push(if m.action == action::BUTTON_PRESS {
                        Event::MouseDown { x, y, button, modifiers, time }
                    } else {
                        Event::MouseUp { x, y, button, modifiers, time }
                    });
                }
            }
            action::DOWN | action::POINTER_DOWN | action::UP | action::POINTER_UP if at.tool == tool::MOUSE => {
                // (A mouse's DOWN and UP come with its BUTTON_PRESS and BUTTON_RELEASE, which say which button.)
            }
            action::DOWN | action::POINTER_DOWN => {
                if is_stylus(at.tool) {
                    out.push(self.stylus(m, at, true));
                } else {
                    out.push(Event::TouchDown { pointer: at.id, x: at.x, y: at.y, time });
                }
            }
            action::UP | action::POINTER_UP => {
                if is_stylus(at.tool) {
                    // Lifted, then gone: a stylus still near comes back with its hover (Android's HOVER_ENTER after
                    // UP), one that isn't leaves nothing hovering.
                    out.push(self.stylus(m, at, false));
                    out.push(Event::StylusLeft { pointer: at.id });
                } else {
                    out.push(Event::TouchUp { pointer: at.id, x: at.x, y: at.y, time });
                }
            }
            action::MOVE => {
                let mut moves = Vec::new();
                for (i, p) in m.pointers().enumerate() {
                    if p.tool == tool::MOUSE {
                        // (A mouse dragged with a button held.)
                        out.extend(m.history(i, 0));
                        out.push(Event::MouseMove { x: p.x, y: p.y, modifiers, time });
                    } else if is_stylus(p.tool) {
                        out.extend(m.history(i, p.id));
                        out.push(self.stylus(m, p, true));
                    } else {
                        out.extend(m.history(i, p.id));
                        moves.push((p.id, p.x, p.y));
                    }
                }
                if !moves.is_empty() {
                    out.push(Event::TouchMoves { moves, time });
                }
            }
            _ => {}
        }
        out
    }

    /// A stylus's report: where it is, touching (`down`, with its pressure) or not, its buttons.
    fn stylus(&self, m: &Motion, p: Pointer, down: bool) -> Event {
        Event::Stylus {
            pointer: p.id,
            eraser: p.tool == tool::ERASER,
            x: p.x,
            y: p.y,
            down,
            pressure: if down { p.pressure.clamp(0.0, 1.0) } else { 0.0 },
            buttons: stylus_buttons(m.buttons),
            time: m.time,
        }
    }

    /// A touchpad's gesture (Android 14's fake fingers, as Compose reinterprets them): a two-finger swipe scrolls
    /// where the cursor was as it began (its distance in tenths of a notch a dp, as Linux's touchpad scrolls: the
    /// content follows the fingers); a pinch zooms around the fingers' middle, from its second finger's coming to
    /// its going.
    fn touchpad(&mut self, m: &Motion, out: &mut Vec<Event>) {
        let modifiers = modifiers_of(m.meta);
        let Some(first) = m.pointer(0) else { return };
        if m.classification == classification::TWO_FINGER_SWIPE {
            match m.action {
                action::DOWN => self.swipe_at = Some((first.x, first.y)),
                action::MOVE => {
                    let (x, y) = self.swipe_at.unwrap_or((first.x, first.y));
                    let notch = 10.0 * m.density.max(0.1);
                    let (dx, dy) = (m.gesture_scroll_x / notch, -m.gesture_scroll_y / notch);
                    if dx != 0.0 || dy != 0.0 {
                        out.push(Event::Wheel { x, y, dx, dy, modifiers, time: m.time });
                    }
                }
                action::UP => self.swipe_at = None,
                _ => {}
            }
            return;
        }
        let middle = || {
            let second = m.pointer(1)?;
            Some(((first.x + second.x) / 2.0, (first.y + second.y) / 2.0))
        };
        match m.action {
            action::POINTER_DOWN if m.count() == 2 => {
                if let Some((x, y)) = middle() {
                    self.pinch_at = Some((x, y));
                    out.push(Event::PinchBegin { x, y });
                }
            }
            action::MOVE if self.pinch_at.is_some() => {
                if let Some((x, y)) = middle() {
                    self.pinch_at = Some((x, y));
                    let scale = if m.pinch_scale > 0.0 && m.pinch_scale.is_finite() { m.pinch_scale } else { 1.0 };
                    out.push(Event::Pinch { x, y, scale, rotation: 0.0 });
                }
            }
            action::POINTER_UP | action::UP => {
                if let Some((x, y)) = self.pinch_at.take() {
                    out.push(Event::PinchEnd { x, y });
                }
            }
            _ => {}
        }
    }
}

/// A key pressed or let go (`KeyEvent`'s code, its meta state, the character it types: `getUnicodeChar`, 0 for
/// none; a dead key's accent composed by KompozView), at `time`: its key event, and on a press what it types as
/// text (none for a control character). Every key code is Compose's key (`Key(nativeKeyCode)`).
pub fn key_events(code: i32, down: bool, meta: i32, typed: &str, time: i64) -> Vec<Event> {
    let key = Key::native(code);
    let modifiers = modifiers_of(meta);
    let code_point = typed.chars().next().map_or(0, |c| c as i32);
    let mut out = vec![if down {
        Event::KeyDown { key, code_point, modifiers, time }
    } else {
        Event::KeyUp { key, code_point, modifiers, time }
    }];
    // (Ctrl or Meta with a key is a shortcut, not typing; Alt types: AltGr's characters on some layouts.)
    let shortcut = modifiers.contains(KeyModifiers::Ctrl) || modifiers.contains(KeyModifiers::Meta);
    if down && !shortcut && !typed.is_empty() && !typed.chars().any(char::is_control) {
        out.push(Event::Text(typed.to_string()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(action: i32, index: usize, ids: &[i32], xy: &[f32]) -> Motion {
        Motion {
            action,
            index,
            ids: ids.to_vec(),
            tools: vec![tool::FINGER; ids.len()],
            xy: xy.to_vec(),
            time: 7,
            ..Motion::default()
        }
    }

    fn events(m: &Motion) -> Vec<Event> {
        MotionDecoder::default().events(m)
    }

    #[test]
    fn fingers_are_touches_moved_together() {
        let down = touch(action::DOWN, 0, &[3], &[10.0, 20.0]);
        assert_eq!(events(&down), vec![Event::TouchDown { pointer: 3, x: 10.0, y: 20.0, time: 7 }]);
        let second = touch(action::POINTER_DOWN, 1, &[3, 5], &[10.0, 20.0, 30.0, 40.0]);
        assert_eq!(events(&second), vec![Event::TouchDown { pointer: 5, x: 30.0, y: 40.0, time: 7 }]);
        let moved = touch(action::MOVE, 0, &[3, 5], &[11.0, 21.0, 31.0, 41.0]);
        assert_eq!(events(&moved), vec![Event::TouchMoves { moves: vec![(3, 11.0, 21.0), (5, 31.0, 41.0)], time: 7 }]);
        let up = touch(action::POINTER_UP, 1, &[3, 5], &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(events(&up), vec![Event::TouchUp { pointer: 5, x: 3.0, y: 4.0, time: 7 }]);
        // A cancel calls the gesture off (nothing lifted: nothing clicks).
        assert_eq!(events(&touch(action::CANCEL, 0, &[3, 5], &[1.0, 2.0, 3.0, 4.0])), vec![Event::TouchCancel]);
        // (Arrays shorter than their count: nothing read past them.)
        assert!(events(&touch(action::DOWN, 2, &[3], &[1.0, 2.0])).is_empty());
        let unknown_tool = Motion { tools: vec![], ..touch(action::DOWN, 0, &[1], &[1.0, 1.0]) };
        assert!(matches!(events(&unknown_tool)[..], [Event::TouchDown { pointer: 1, .. }]));
    }

    #[test]
    fn historical_samples_come_before_their_move() {
        // Two fingers, two samples batched before the event: each finger's positions with their times.
        let m = Motion {
            history_times: vec![3, 5],
            history_xy: vec![1.0, 2.0, 10.0, 20.0, 1.5, 2.5, 10.5, 20.5],
            ..touch(action::MOVE, 0, &[0, 1], &[2.0, 3.0, 11.0, 21.0])
        };
        assert_eq!(
            events(&m),
            vec![
                Event::History { pointer: 0, samples: vec![(1.0, 2.0, 3), (1.5, 2.5, 5)] },
                Event::History { pointer: 1, samples: vec![(10.0, 20.0, 3), (10.5, 20.5, 5)] },
                Event::TouchMoves { moves: vec![(0, 2.0, 3.0), (1, 11.0, 21.0)], time: 7 },
            ]
        );
        // A short history array: what is there, nothing past it.
        let short = Motion { history_times: vec![3, 5], history_xy: vec![1.0, 2.0], ..m.clone() };
        assert_eq!(events(&short)[0], Event::History { pointer: 0, samples: vec![(1.0, 2.0, 3)] });
        // A hovering mouse's samples are the mouse's (its pointer 0 for the host).
        let hover = Motion {
            action: action::HOVER_MOVE,
            tools: vec![tool::MOUSE],
            history_times: vec![4],
            history_xy: vec![8.0, 9.0],
            ..touch(action::HOVER_MOVE, 0, &[6], &[9.0, 9.0])
        };
        assert_eq!(
            events(&hover),
            vec![
                Event::History { pointer: 0, samples: vec![(8.0, 9.0, 4)] },
                Event::MouseMove { x: 9.0, y: 9.0, modifiers: KeyModifiers::None, time: 7 }
            ]
        );
    }

    #[test]
    fn a_stylus_hovers_touches_with_pressure_and_leaves() {
        let pen = |action, tool, pressure, buttons| Motion {
            action,
            ids: vec![2],
            tools: vec![tool],
            xy: vec![5.0, 6.0],
            pressure: vec![pressure],
            buttons,
            time: 9,
            ..Motion::default()
        };
        let stylus = |eraser, down, pressure, buttons| Event::Stylus {
            pointer: 2,
            eraser,
            x: 5.0,
            y: 6.0,
            down,
            pressure,
            buttons,
            time: 9,
        };
        assert_eq!(events(&pen(action::HOVER_ENTER, tool::STYLUS, 0.0, 0)), vec![stylus(false, false, 0.0, 0)]);
        assert_eq!(events(&pen(action::DOWN, tool::STYLUS, 0.4, 0)), vec![stylus(false, true, 0.4, 0)]);
        assert_eq!(
            events(&pen(action::MOVE, tool::STYLUS, 0.7, button::STYLUS_PRIMARY)),
            vec![stylus(false, true, 0.7, 1)]
        );
        assert_eq!(
            events(&pen(action::UP, tool::STYLUS, 0.0, 0)),
            vec![stylus(false, false, 0.0, 0), Event::StylusLeft { pointer: 2 }]
        );
        assert_eq!(events(&pen(action::HOVER_EXIT, tool::STYLUS, 0.0, 0)), vec![Event::StylusLeft { pointer: 2 }]);
        // The other end: the eraser, its pressure clamped.
        assert_eq!(events(&pen(action::DOWN, tool::ERASER, 1.5, 0)), vec![stylus(true, true, 1.0, 0)]);
        // A barrel button pressed while it hovers: a report with the buttons.
        let pressed = Motion { button: button::STYLUS_SECONDARY, ..pen(action::BUTTON_PRESS, tool::STYLUS, 0.0, 64) };
        assert_eq!(events(&pressed), vec![stylus(false, false, 0.0, 2)]);
        // A cancel: called off, the stylus gone.
        assert_eq!(
            events(&pen(action::CANCEL, tool::STYLUS, 0.0, 0)),
            vec![Event::TouchCancel, Event::StylusLeft { pointer: 2 }]
        );
        assert_eq!(stylus_buttons(button::SECONDARY | button::TERTIARY), 3);
    }

    #[test]
    fn a_mouse_moves_presses_and_scrolls() {
        let mouse = |action, button| Motion {
            action,
            ids: vec![0],
            tools: vec![tool::MOUSE],
            xy: vec![5.0, 6.0],
            time: 9,
            button,
            scroll_y: 1.0,
            meta: 0x1000,
            ..Motion::default()
        };
        let ctrl = KeyModifiers::Ctrl;
        assert_eq!(
            events(&mouse(action::HOVER_MOVE, 0)),
            vec![Event::MouseMove { x: 5.0, y: 6.0, modifiers: ctrl, time: 9 }]
        );
        assert_eq!(
            events(&mouse(action::MOVE, 0)),
            vec![Event::MouseMove { x: 5.0, y: 6.0, modifiers: ctrl, time: 9 }]
        );
        assert_eq!(
            events(&mouse(action::BUTTON_PRESS, button::SECONDARY)),
            vec![Event::MouseDown { x: 5.0, y: 6.0, button: MouseButton::Right, modifiers: ctrl, time: 9 }]
        );
        assert_eq!(
            events(&mouse(action::BUTTON_RELEASE, button::PRIMARY)),
            vec![Event::MouseUp { x: 5.0, y: 6.0, button: MouseButton::Left, modifiers: ctrl, time: 9 }]
        );
        assert_eq!(
            events(&mouse(action::BUTTON_PRESS, button::BACK))[0],
            Event::MouseDown { x: 5.0, y: 6.0, button: MouseButton::Back, modifiers: ctrl, time: 9 }
        );
        assert_eq!(
            events(&mouse(action::SCROLL, 0)),
            vec![Event::Wheel { x: 5.0, y: 6.0, dx: 0.0, dy: 1.0, modifiers: ctrl, time: 9 }]
        );
        assert_eq!(events(&mouse(action::HOVER_EXIT, 0)), vec![Event::PointerLeft]);
        // (The mouse's DOWN: its BUTTON_PRESS says which button.)
        assert!(events(&mouse(action::DOWN, 1)).is_empty());
    }

    #[test]
    fn a_touchpad_swipe_scrolls_and_its_pinch_zooms() {
        let mut decoder = MotionDecoder::default();
        let pad = |action, classification, xy: &[f32]| Motion {
            action,
            ids: (0..xy.len() as i32 / 2).collect(),
            tools: vec![tool::FINGER; xy.len() / 2],
            xy: xy.to_vec(),
            classification,
            density: 2.0,
            time: 4,
            ..Motion::default()
        };
        let swipe = classification::TWO_FINGER_SWIPE;
        assert!(decoder.events(&pad(action::DOWN, swipe, &[100.0, 50.0])).is_empty());
        // 40 px down at density 2: two notches, the content following the fingers (the wheel turned up), where the
        // cursor was.
        let moved = Motion { gesture_scroll_y: 40.0, ..pad(action::MOVE, swipe, &[100.0, 90.0]) };
        assert_eq!(
            decoder.events(&moved),
            vec![Event::Wheel { x: 100.0, y: 50.0, dx: 0.0, dy: -2.0, modifiers: KeyModifiers::None, time: 4 }]
        );
        assert!(decoder.events(&pad(action::UP, swipe, &[100.0, 90.0])).is_empty());
        let pinch = classification::PINCH;
        assert!(decoder.events(&pad(action::DOWN, pinch, &[10.0, 10.0])).is_empty());
        assert_eq!(
            decoder.events(&pad(action::POINTER_DOWN, pinch, &[10.0, 10.0, 30.0, 10.0])),
            vec![Event::PinchBegin { x: 20.0, y: 10.0 }]
        );
        let grown = Motion { pinch_scale: 1.25, ..pad(action::MOVE, pinch, &[5.0, 10.0, 35.0, 10.0]) };
        assert_eq!(decoder.events(&grown), vec![Event::Pinch { x: 20.0, y: 10.0, scale: 1.25, rotation: 0.0 }]);
        assert_eq!(
            decoder.events(&pad(action::POINTER_UP, pinch, &[5.0, 10.0, 35.0, 10.0])),
            vec![Event::PinchEnd { x: 20.0, y: 10.0 }]
        );
        assert!(decoder.events(&pad(action::UP, pinch, &[5.0, 10.0])).is_empty());
    }

    #[test]
    fn keys_are_composes_and_type_text() {
        // KEYCODE_A with Shift: Compose's Key.A, typing "A".
        let events = key_events(29, true, 0x1, "A", 3);
        assert_eq!(
            events,
            vec![
                Event::KeyDown { key: Key::A, code_point: 'A' as i32, modifiers: KeyModifiers::Shift, time: 3 },
                Event::Text("A".into())
            ]
        );
        // Enter types nothing as text (the key event is the field's), nor does a key let go, nor a shortcut.
        assert_eq!(key_events(66, true, 0, "\n", 3).len(), 1);
        assert_eq!(key_events(29, false, 0, "a", 3).len(), 1);
        assert_eq!(key_events(29, true, 0x1000, "a", 3).len(), 1);
        assert_eq!(
            key_events(66, false, 0, "", 3)[0],
            Event::KeyUp { key: Key::Enter, code_point: 0, modifiers: KeyModifiers::None, time: 3 }
        );
        // Every key code is Compose's key: those C++ left out too (the media keys, the numpad, the F keys past 12).
        assert_eq!(
            key_events(85, true, 0, "", 3)[0],
            Event::KeyDown { key: Key::MediaPlayPause, code_point: 0, modifiers: KeyModifiers::None, time: 3 }
        );
        assert!(matches!(key_events(144, true, 0, "0", 3)[0], Event::KeyDown { key: Key::NumPad0, .. }));
        assert_eq!(modifiers_of(0x1 | 0x2 | 0x1000 | 0x10000), {
            KeyModifiers::Shift | KeyModifiers::Alt | KeyModifiers::Ctrl | KeyModifiers::Meta
        });
    }
}

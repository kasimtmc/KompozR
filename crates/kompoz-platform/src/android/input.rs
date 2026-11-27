//! Android's input as KompozR's events (C++'s `motion` and `key`): a `MotionEvent` as KompozView passes it — fingers
//! (and, until the stylus's own events come, a stylus) as touches, a mouse's moves, buttons and wheel —, a `KeyEvent`
//! as Compose's key (Compose's `Key` is Android's key code: `Key.android.kt`'s `Key(nativeKeyCode)`) with the
//! character it types and the meta state as modifiers. Pure functions of what Java passes: tested on the host.

use crate::app::{Event, MouseButton};
use kompoz_ui::input::{Key, KeyModifiers};

/// `MotionEvent`'s actions (`getActionMasked`).
pub mod action {
    pub const DOWN: i32 = 0;
    pub const UP: i32 = 1;
    pub const MOVE: i32 = 2;
    pub const CANCEL: i32 = 3;
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
    pub const FINGER: i32 = 1;
    pub const STYLUS: i32 = 2;
    pub const MOUSE: i32 = 3;
    pub const ERASER: i32 = 4;
}

/// A `MotionEvent` as KompozView passes it: its masked action, the index of the pointer it is about, each pointer's
/// id and position (px in the view), its time (ns, `System.nanoTime`'s clock), the action pointer's tool type, the
/// button pressed or released, the wheel's axes, the meta state.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Motion {
    pub action: i32,
    pub index: usize,
    pub ids: Vec<i32>,
    pub xy: Vec<f32>,
    pub time: i64,
    pub tool: i32,
    pub button: i32,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub meta: i32,
}

impl Motion {
    fn at(&self, i: usize) -> Option<(i64, f32, f32)> {
        let (x, y) = (*self.xy.get(i * 2)?, *self.xy.get(i * 2 + 1)?);
        Some((*self.ids.get(i)? as i64, x, y))
    }

    fn count(&self) -> usize {
        self.ids.len().min(self.xy.len() / 2)
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
fn mouse_button(button: i32) -> Option<MouseButton> {
    Some(match button {
        1 => MouseButton::Left,
        2 => MouseButton::Right,
        4 => MouseButton::Middle,
        8 => MouseButton::Back,
        16 => MouseButton::Forward,
        _ => return None,
    })
}

/// What a `MotionEvent` is to KompozR: a mouse's (its tool, or a hover, a wheel, a button's press or release: the
/// pointer's moves, its leaving, its buttons, its wheel), else fingers' (the one going down or up, every one moved,
/// all of them lifted on a cancel).
pub fn motion_events(m: &Motion) -> Vec<Event> {
    let modifiers = modifiers_of(m.meta);
    let time = m.time;
    let mouse = m.tool == tool::MOUSE
        || matches!(
            m.action,
            action::HOVER_ENTER
                | action::HOVER_MOVE
                | action::HOVER_EXIT
                | action::SCROLL
                | action::BUTTON_PRESS
                | action::BUTTON_RELEASE
        );
    let mut out = Vec::new();
    if mouse {
        let Some((_, x, y)) = m.at(m.index) else { return out };
        match m.action {
            action::MOVE | action::HOVER_ENTER | action::HOVER_MOVE => {
                out.push(Event::MouseMove { x, y, modifiers, time });
            }
            action::HOVER_EXIT => out.push(Event::PointerLeft),
            // (AXIS_VSCROLL is positive away from the user, AXIS_HSCROLL to the right: as the user turns the wheel.)
            action::SCROLL => out.push(Event::Wheel { x, y, dx: m.scroll_x, dy: m.scroll_y, modifiers, time }),
            action::BUTTON_PRESS | action::BUTTON_RELEASE => {
                if let Some(button) = mouse_button(m.button) {
                    out.push(if m.action == action::BUTTON_PRESS {
                        Event::MouseDown { x, y, button, modifiers, time }
                    } else {
                        Event::MouseUp { x, y, button, modifiers, time }
                    });
                }
            }
            // (A button's DOWN and UP come with its BUTTON_PRESS and BUTTON_RELEASE, which say which button.)
            _ => {}
        }
        return out;
    }
    match m.action {
        action::DOWN | action::POINTER_DOWN => {
            if let Some((pointer, x, y)) = m.at(m.index) {
                out.push(Event::TouchDown { pointer, x, y, time });
            }
        }
        action::UP | action::POINTER_UP => {
            if let Some((pointer, x, y)) = m.at(m.index) {
                out.push(Event::TouchUp { pointer, x, y, time });
            }
        }
        action::MOVE => {
            out.extend((0..m.count()).filter_map(|i| m.at(i)).map(|(pointer, x, y)| Event::TouchMove {
                pointer,
                x,
                y,
                time,
            }));
        }
        // (The system took the gesture: every finger lifted where it is.)
        action::CANCEL => {
            out.extend((0..m.count()).filter_map(|i| m.at(i)).map(|(pointer, x, y)| Event::TouchUp {
                pointer,
                x,
                y,
                time,
            }));
        }
        _ => {}
    }
    out
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
    if down && !typed.is_empty() && !typed.chars().any(char::is_control) {
        out.push(Event::Text(typed.to_string()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(action: i32, index: usize, ids: &[i32], xy: &[f32]) -> Motion {
        Motion { action, index, ids: ids.to_vec(), xy: xy.to_vec(), time: 7, tool: tool::FINGER, ..Motion::default() }
    }

    #[test]
    fn fingers_are_touches() {
        let down = touch(action::DOWN, 0, &[3], &[10.0, 20.0]);
        assert_eq!(motion_events(&down), vec![Event::TouchDown { pointer: 3, x: 10.0, y: 20.0, time: 7 }]);
        let second = touch(action::POINTER_DOWN, 1, &[3, 5], &[10.0, 20.0, 30.0, 40.0]);
        assert_eq!(motion_events(&second), vec![Event::TouchDown { pointer: 5, x: 30.0, y: 40.0, time: 7 }]);
        let moved = touch(action::MOVE, 0, &[3, 5], &[11.0, 21.0, 31.0, 41.0]);
        assert_eq!(
            motion_events(&moved),
            vec![
                Event::TouchMove { pointer: 3, x: 11.0, y: 21.0, time: 7 },
                Event::TouchMove { pointer: 5, x: 31.0, y: 41.0, time: 7 }
            ]
        );
        let cancel = touch(action::CANCEL, 0, &[3, 5], &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(motion_events(&cancel).len(), 2);
        assert!(motion_events(&cancel).iter().all(|e| matches!(e, Event::TouchUp { .. })));
        let up = touch(action::POINTER_UP, 1, &[3, 5], &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(motion_events(&up), vec![Event::TouchUp { pointer: 5, x: 3.0, y: 4.0, time: 7 }]);
        // (Arrays shorter than their count: nothing read past them.)
        assert!(motion_events(&touch(action::DOWN, 2, &[3], &[1.0, 2.0])).is_empty());
    }

    #[test]
    fn a_mouse_moves_presses_and_scrolls() {
        let mouse = |action, button| Motion {
            action,
            index: 0,
            ids: vec![0],
            xy: vec![5.0, 6.0],
            time: 9,
            tool: tool::MOUSE,
            button,
            scroll_y: 1.0,
            meta: 0x1000,
            ..Motion::default()
        };
        let ctrl = KeyModifiers::Ctrl;
        assert_eq!(
            motion_events(&mouse(action::HOVER_MOVE, 0)),
            vec![Event::MouseMove { x: 5.0, y: 6.0, modifiers: ctrl, time: 9 }]
        );
        assert_eq!(
            motion_events(&mouse(action::BUTTON_PRESS, 2)),
            vec![Event::MouseDown { x: 5.0, y: 6.0, button: MouseButton::Right, modifiers: ctrl, time: 9 }]
        );
        assert_eq!(
            motion_events(&mouse(action::BUTTON_RELEASE, 1)),
            vec![Event::MouseUp { x: 5.0, y: 6.0, button: MouseButton::Left, modifiers: ctrl, time: 9 }]
        );
        assert_eq!(
            motion_events(&mouse(action::SCROLL, 0)),
            vec![Event::Wheel { x: 5.0, y: 6.0, dx: 0.0, dy: 1.0, modifiers: ctrl, time: 9 }]
        );
        assert_eq!(motion_events(&mouse(action::HOVER_EXIT, 0)), vec![Event::PointerLeft]);
        // (The mouse's DOWN: its BUTTON_PRESS says which button.)
        assert!(motion_events(&mouse(action::DOWN, 1)).is_empty());
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
        // Enter types nothing as text (the key event is the field's), nor does a key let go.
        assert_eq!(key_events(66, true, 0, "\n", 3).len(), 1);
        assert_eq!(key_events(29, false, 0, "a", 3).len(), 1);
        assert_eq!(
            key_events(66, false, 0, "", 3)[0],
            Event::KeyUp { key: Key::Enter, code_point: 0, modifiers: KeyModifiers::None, time: 3 }
        );
        assert_eq!(modifiers_of(0x1 | 0x2 | 0x1000 | 0x10000), {
            KeyModifiers::Shift | KeyModifiers::Alt | KeyModifiers::Ctrl | KeyModifiers::Meta
        });
    }
}

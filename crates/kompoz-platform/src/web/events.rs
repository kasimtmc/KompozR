//! The page's events as web.js gathers them — a record a line, its fields between tabs, strings percent-encoded
//! (`encodeURIComponent`) — read into [`Record`]s, and the pointers' and the wheel's turned into KompozR's events
//! (C++'s mouse, touch and wheel callbacks, with pointer events): a mouse's buttons, a finger each by its pointer
//! id, a pen's or an eraser's contact, pressure and barrel buttons, and its hovering; the wheel's lines and pages
//! as notches.

use super::aria::AriaAction;
use super::keys;
use crate::app::{Event, MouseButton};
use kompoz_ui::draganddrop::DragAndDropTransferAction;
use kompoz_ui::input::KeyModifiers;
use std::collections::HashSet;

/// A pointer's kind (`PointerEvent.pointerType`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PointerKind {
    Mouse,
    Pen,
    Touch,
}

/// What happened to a pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Down,
    Move,
    Up,
    /// The browser took it (a touch it scrolls with, a drag it starts): as if let go where it is.
    Cancel,
    /// It left the canvas (a mouse with no button held, a pen out of range).
    Leave,
}

/// A pointer event: which pointer, of what kind; where (CSS pixels from the canvas's top left); the button that
/// changed (`PointerEvent.button`: 0 the main one, 1 the middle, 2 the secondary, 3 back, 4 forward, 5 a pen's
/// eraser; -1 none) and those held (`buttons`' bits); the pen's pressure; the modifiers; when (ms, the page's
/// clock).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pointer {
    pub phase: Phase,
    pub id: i64,
    pub kind: PointerKind,
    pub x: f32,
    pub y: f32,
    pub button: i32,
    pub buttons: u32,
    pub pressure: f32,
    pub modifiers: KeyModifiers,
    pub time: f64,
}

/// A drag of the page's (files from a file manager, text from another page) over the canvas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragPhase {
    Enter,
    Over,
    Leave,
    Drop,
}

/// What the page tells.
#[derive(Clone, PartialEq, Debug)]
pub enum Record {
    Pointer(Pointer),
    /// The wheel (or a touchpad) at x, y: its deltas and their unit (`deltaMode`: 0 pixels, 1 lines, 2 pages).
    Wheel {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        mode: u32,
        modifiers: KeyModifiers,
        time: f64,
    },
    /// A key down (or repeated) or up: its `code` and `key`.
    Key {
        down: bool,
        code: String,
        key: String,
        modifiers: KeyModifiers,
        time: f64,
    },
    /// The text area's `input` event: what it holds now and its selection (UTF-16).
    Input {
        value: String,
        start: u32,
        end: u32,
    },
    CompositionStart,
    CompositionUpdate {
        data: String,
        value: String,
        start: u32,
        end: u32,
    },
    CompositionEnd {
        data: String,
        value: String,
        start: u32,
        end: u32,
    },
    /// A paste event's data (the transfer's handle in the glue's table; the page read its files first).
    Paste {
        transfer: u32,
    },
    Drag {
        phase: DragPhase,
        x: f32,
        y: f32,
        transfer: u32,
    },
    /// The page's drag of the app's own ended (`dragend`): dropped with this effect, or not.
    DragEnd {
        action: Option<DragAndDropTransferAction>,
    },
    /// Something of the page's changed (its size, a media query's answer, the focus, the visibility, the address):
    /// read again ([`sys::state`](super::sys::state)). The name says what.
    Page(String),
    /// The page is going away (pagehide).
    Leaving,
    /// A screen reader's action on a node's element of the ARIA mirror (aria.rs).
    Aria {
        node: u64,
        action: AriaAction,
    },
    /// The text area's selection moved by itself (a screen reader's, not a key's nor an input's): UTF-16.
    Select {
        start: u32,
        end: u32,
    },
}

/// A percent-encoded string field.
pub(super) fn decoded(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(b) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A record of a line (none for one not understood).
pub fn parse(line: &str) -> Option<Record> {
    let f: Vec<&str> = line.split('\t').collect();
    let num = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok());
    let int = |i: usize| f.get(i).and_then(|v| v.parse::<i64>().ok());
    let text = |i: usize| f.get(i).map(|v| decoded(v)).unwrap_or_default();
    let mods = |i: usize| keys::modifiers(int(i).unwrap_or(0) as u32);
    Some(match *f.first()? {
        "pointer" => {
            let phase = match *f.get(1)? {
                "down" => Phase::Down,
                "move" => Phase::Move,
                "up" => Phase::Up,
                "cancel" => Phase::Cancel,
                "leave" => Phase::Leave,
                _ => return None,
            };
            let kind = match *f.get(3)? {
                "pen" => PointerKind::Pen,
                "touch" => PointerKind::Touch,
                _ => PointerKind::Mouse,
            };
            Record::Pointer(Pointer {
                phase,
                id: int(2)?,
                kind,
                x: num(4)? as f32,
                y: num(5)? as f32,
                button: int(6).unwrap_or(-1) as i32,
                buttons: int(7).unwrap_or(0) as u32,
                pressure: num(8).unwrap_or(0.0) as f32,
                modifiers: mods(9),
                time: num(10).unwrap_or(-1.0),
            })
        }
        "wheel" => Record::Wheel {
            x: num(1)? as f32,
            y: num(2)? as f32,
            dx: num(3)? as f32,
            dy: num(4)? as f32,
            mode: int(5).unwrap_or(0) as u32,
            modifiers: mods(6),
            time: num(7).unwrap_or(-1.0),
        },
        "key" => Record::Key {
            down: *f.get(1)? == "1",
            code: text(2),
            key: text(3),
            modifiers: mods(4),
            time: num(5).unwrap_or(-1.0),
        },
        "input" => Record::Input { value: text(1), start: int(2)? as u32, end: int(3)? as u32 },
        "compositionstart" => Record::CompositionStart,
        "compositionupdate" | "compositionend" => {
            let (data, value, start, end) = (text(1), text(2), int(3)? as u32, int(4)? as u32);
            if f[0] == "compositionupdate" {
                Record::CompositionUpdate { data, value, start, end }
            } else {
                Record::CompositionEnd { data, value, start, end }
            }
        }
        "paste" => Record::Paste { transfer: int(1)? as u32 },
        "drag" => Record::Drag {
            phase: match *f.get(1)? {
                "enter" => DragPhase::Enter,
                "over" => DragPhase::Over,
                "leave" => DragPhase::Leave,
                "drop" => DragPhase::Drop,
                _ => return None,
            },
            x: num(2).unwrap_or(0.0) as f32,
            y: num(3).unwrap_or(0.0) as f32,
            transfer: int(4).unwrap_or(0) as u32,
        },
        "dragend" => Record::DragEnd {
            action: match *f.get(1)? {
                "copy" => Some(DragAndDropTransferAction::Copy),
                "move" => Some(DragAndDropTransferAction::Move),
                "link" => Some(DragAndDropTransferAction::Link),
                _ => None,
            },
        },
        "page" => Record::Page(text(1)),
        "leaving" => Record::Leaving,
        "aria" => Record::Aria {
            node: f.get(1)?.parse().ok()?,
            action: match *f.get(2)? {
                "click" => AriaAction::Click,
                "longclick" => AriaAction::LongClick,
                "focus" => AriaAction::Focus,
                "progress" => AriaAction::Progress(num(3).filter(|v| v.is_finite())? as f32),
                "scroll" => AriaAction::Scroll(*f.get(3)? == "forward"),
                "custom" => AriaAction::Custom(int(3)?.try_into().ok()?),
                _ => return None,
            },
        },
        "select" => Record::Select { start: int(1)? as u32, end: int(2)? as u32 },
        _ => return None,
    })
}

/// The page's clock (ms) as the platform's (ns); -1: now.
pub fn nanos(ms: f64) -> i64 {
    if ms < 0.0 { -1 } else { (ms * 1e6) as i64 }
}

/// A mouse's button by `PointerEvent.button`.
fn mouse_button(button: i32) -> Option<MouseButton> {
    Some(match button {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        _ => return None,
    })
}

/// The pointers as they are: the fingers down, the pens near.
#[derive(Default)]
pub struct Pointers {
    touches: HashSet<i64>,
    pens: HashSet<i64>,
}

/// A pen's barrel buttons as the stylus event has them (the first, the second): `buttons`' secondary (2) and
/// middle (4) bits.
fn barrel(buttons: u32) -> u32 {
    let mut b = 0;
    if buttons & 2 != 0 {
        b |= 1;
    }
    if buttons & 4 != 0 {
        b |= 2;
    }
    b
}

impl Pointers {
    /// KompozR's events for a pointer event.
    pub fn events(&mut self, p: &Pointer) -> Vec<Event> {
        let time = nanos(p.time);
        let (x, y, modifiers) = (p.x, p.y, p.modifiers);
        match p.kind {
            PointerKind::Mouse => match p.phase {
                Phase::Down => mouse_button(p.button)
                    .map(|button| vec![Event::MouseDown { x, y, button, modifiers, time }])
                    .unwrap_or_default(),
                Phase::Up | Phase::Cancel => mouse_button(p.button)
                    .map(|button| vec![Event::MouseUp { x, y, button, modifiers, time }])
                    .unwrap_or_default(),
                Phase::Move => vec![Event::MouseMove { x, y, modifiers, time }],
                Phase::Leave => vec![Event::PointerLeft],
            },
            PointerKind::Touch => {
                let pointer = p.id;
                match p.phase {
                    Phase::Down => {
                        self.touches.insert(pointer);
                        vec![Event::TouchDown { pointer, x, y, time }]
                    }
                    Phase::Move if self.touches.contains(&pointer) => vec![Event::TouchMove { pointer, x, y, time }],
                    Phase::Up | Phase::Cancel if self.touches.remove(&pointer) => {
                        vec![Event::TouchUp { pointer, x, y, time }]
                    }
                    _ => Vec::new(),
                }
            }
            PointerKind::Pen => {
                let pointer = p.id;
                if p.phase == Phase::Leave {
                    return if self.pens.remove(&pointer) { vec![Event::StylusLeft { pointer }] } else { Vec::new() };
                }
                self.pens.insert(pointer);
                // The eraser: its own end (button 5 held, bit 32), as Linux's tablets tell it.
                let eraser = p.buttons & 32 != 0 || p.button == 5;
                let down = match p.phase {
                    Phase::Up | Phase::Cancel => false,
                    _ => p.buttons & (1 | 32) != 0,
                };
                let pressure = if down { p.pressure.clamp(0.0, 1.0) } else { 0.0 };
                let mut events =
                    vec![Event::Stylus { pointer, eraser, x, y, down, pressure, buttons: barrel(p.buttons), time }];
                // A pen lifted off a screen (a touch screen's pen has no hovering): gone.
                if p.phase == Phase::Cancel {
                    self.pens.remove(&pointer);
                    events.push(Event::StylusLeft { pointer });
                }
                events
            }
        }
    }
}

/// The wheel's deltas as notches (C++'s: pixels a hundredth, lines a third, pages 80), up and right positive.
pub fn wheel_notches(dx: f32, dy: f32, mode: u32) -> (f32, f32) {
    let k = match mode {
        1 => 1.0 / 3.0,
        2 => 80.0,
        _ => 0.01,
    };
    (dx * k, -dy * k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_read() {
        let p = parse("pointer\tdown\t3\tpen\t10.5\t20\t0\t1\t0.5\t3\t1000").unwrap();
        assert_eq!(
            p,
            Record::Pointer(Pointer {
                phase: Phase::Down,
                id: 3,
                kind: PointerKind::Pen,
                x: 10.5,
                y: 20.0,
                button: 0,
                buttons: 1,
                pressure: 0.5,
                modifiers: KeyModifiers::Shift | KeyModifiers::Ctrl,
                time: 1000.0,
            })
        );
        assert_eq!(
            parse("key\t1\tKeyA\t%C5%9F\t0\t5").unwrap(),
            Record::Key { down: true, code: "KeyA".into(), key: "ş".into(), modifiers: KeyModifiers::None, time: 5.0 }
        );
        assert_eq!(parse("input\ta%09b%0A\t3\t3").unwrap(), Record::Input { value: "a\tb\n".into(), start: 3, end: 3 });
        assert_eq!(parse("dragend\tnone").unwrap(), Record::DragEnd { action: None });
        assert_eq!(parse("page\tresize").unwrap(), Record::Page("resize".into()));
        assert_eq!(parse("nothing\t1"), None);
        assert_eq!(parse("pointer\tdown"), None);
        assert_eq!(parse("aria\t42\tclick").unwrap(), Record::Aria { node: 42, action: AriaAction::Click });
        assert_eq!(
            parse("aria\t7\tprogress\t0.75").unwrap(),
            Record::Aria { node: 7, action: AriaAction::Progress(0.75) }
        );
        assert_eq!(
            parse("aria\t7\tscroll\tbackward").unwrap(),
            Record::Aria { node: 7, action: AriaAction::Scroll(false) }
        );
        assert_eq!(parse("aria\t7\tcustom\t2").unwrap(), Record::Aria { node: 7, action: AriaAction::Custom(2) });
        assert_eq!(parse("aria\t7\tprogress\tNaN"), None);
        assert_eq!(parse("select\t1\t4").unwrap(), Record::Select { start: 1, end: 4 });
    }

    fn pointer(phase: Phase, kind: PointerKind, button: i32, buttons: u32) -> Pointer {
        Pointer {
            phase,
            id: 7,
            kind,
            x: 1.0,
            y: 2.0,
            button,
            buttons,
            pressure: 0.25,
            modifiers: KeyModifiers::None,
            time: 2.0,
        }
    }

    #[test]
    fn mice_fingers_and_pens() {
        let mut p = Pointers::default();
        assert_eq!(
            p.events(&pointer(Phase::Down, PointerKind::Mouse, 2, 2)),
            [Event::MouseDown {
                x: 1.0,
                y: 2.0,
                button: MouseButton::Right,
                modifiers: KeyModifiers::None,
                time: 2_000_000
            }]
        );
        assert_eq!(p.events(&pointer(Phase::Leave, PointerKind::Mouse, -1, 0)), [Event::PointerLeft]);
        // A finger: down, moved, up; a move of a finger not down is nothing.
        assert_eq!(p.events(&pointer(Phase::Move, PointerKind::Touch, -1, 0)), []);
        assert!(matches!(p.events(&pointer(Phase::Down, PointerKind::Touch, 0, 1))[..], [Event::TouchDown { .. }]));
        assert!(matches!(p.events(&pointer(Phase::Move, PointerKind::Touch, -1, 1))[..], [Event::TouchMove { .. }]));
        assert!(matches!(p.events(&pointer(Phase::Cancel, PointerKind::Touch, 0, 0))[..], [Event::TouchUp { .. }]));
        // A pen hovering, touching with its barrel button, its eraser, leaving.
        let hover = p.events(&pointer(Phase::Move, PointerKind::Pen, -1, 0));
        assert!(matches!(hover[..], [Event::Stylus { down: false, pressure: 0.0, eraser: false, .. }]));
        let touch = p.events(&pointer(Phase::Down, PointerKind::Pen, 0, 1 | 2));
        assert!(matches!(touch[..], [Event::Stylus { down: true, pressure: 0.25, buttons: 1, .. }]));
        let erase = p.events(&pointer(Phase::Down, PointerKind::Pen, 5, 32));
        assert!(matches!(erase[..], [Event::Stylus { down: true, eraser: true, .. }]));
        assert_eq!(p.events(&pointer(Phase::Leave, PointerKind::Pen, -1, 0)), [Event::StylusLeft { pointer: 7 }]);
        assert_eq!(p.events(&pointer(Phase::Leave, PointerKind::Pen, -1, 0)), []);
    }

    #[test]
    fn wheels_in_notches() {
        assert_eq!(wheel_notches(0.0, 100.0, 0), (0.0, -1.0));
        assert_eq!(wheel_notches(3.0, -3.0, 1), (1.0, 1.0));
        assert_eq!(wheel_notches(0.0, 1.0, 2), (0.0, -80.0));
    }
}

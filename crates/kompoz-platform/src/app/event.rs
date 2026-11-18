//! What a platform tells a KompozR app (C++'s platform_event.hpp): its window, input and lifecycle events in
//! KompozR's own terms. Each platform turns its own events into these; the app core handles only these.
//!
//! Positions are in the window's points (what the platform's events are in: a Wayland surface's logical
//! coordinates, X11's pixels), turned into px by the app core ([`Metrics::point_to_px`](super::Metrics)); times
//! are on the platform's clock ([`Platform::now`](super::Platform::now), ns; -1: now).

use kompoz_ui::draganddrop::DragAndDropTransferAction;
use kompoz_ui::input::{Key, KeyModifiers};

/// A mouse button (the left one presses; the others are buttons of a pointer event).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

/// A platform's event.
#[derive(Clone, PartialEq, Debug)]
pub enum Event {
    /// The app is to end (the window closed, a signal, the session ending).
    Quit,
    // The window.
    FocusGained,
    FocusLost,
    Shown,
    Hidden,
    Minimized,
    Maximized,
    Restored,
    /// What it shows is gone: shown again.
    Exposed,
    /// Its size (in pixels, or points) changed.
    Resized,
    /// Its scale, display, safe area, the system's theme or language changed.
    MetricsChanged,
    /// A mouse or keyboard came or went (what the media scope tells).
    DevicesChanged,
    /// The pixels kept for it are gone.
    SurfaceLost,
    // The app's lifecycle (phones).
    WillEnterBackground,
    DidEnterBackground,
    WillEnterForeground,
    DidEnterForeground,
    LowMemory,
    // Input.
    /// The mouse left the window.
    PointerLeft,
    MouseMove {
        x: f32,
        y: f32,
        modifiers: KeyModifiers,
        time: i64,
    },
    MouseDown {
        x: f32,
        y: f32,
        button: MouseButton,
        modifiers: KeyModifiers,
        time: i64,
    },
    MouseUp {
        x: f32,
        y: f32,
        button: MouseButton,
        modifiers: KeyModifiers,
        time: i64,
    },
    /// The wheel turned by `dx`, `dy` notches (as the user turns them: up and right positive, flipped back if the
    /// system flips; parts of notches from a touchpad or a free-spinning wheel).
    Wheel {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
        modifiers: KeyModifiers,
        time: i64,
    },
    /// A touchpad's pinch began, went on (by `scale` and `rotation` degrees clockwise since the last), ended,
    /// around (x, y).
    PinchBegin {
        x: f32,
        y: f32,
    },
    Pinch {
        x: f32,
        y: f32,
        scale: f32,
        rotation: f32,
    },
    PinchEnd {
        x: f32,
        y: f32,
    },
    /// A finger on the screen (`pointer` its id).
    TouchDown {
        pointer: i64,
        x: f32,
        y: f32,
        time: i64,
    },
    TouchMove {
        pointer: i64,
        x: f32,
        y: f32,
        time: i64,
    },
    TouchUp {
        pointer: i64,
        x: f32,
        y: f32,
        time: i64,
    },
    /// A stylus near the window (`eraser`: its other end, or an eraser of its own; `pointer` its id): where it is,
    /// whether it touches (`down`) and how hard (`pressure`, 0 to 1), its barrel buttons held (a bit each: the
    /// first, the second, the third), as one report of the tablet's.
    Stylus {
        pointer: i64,
        eraser: bool,
        x: f32,
        y: f32,
        down: bool,
        pressure: f32,
        buttons: u32,
        time: i64,
    },
    /// The stylus `pointer` went away (out of its tablet's range, or off the window).
    StylusLeft {
        pointer: i64,
    },
    /// A key pressed (or repeated) or let go: the key, the character it types (a Unicode scalar, 0 for none: what
    /// Compose's key event carries), the modifiers held.
    KeyDown {
        key: Key,
        code_point: i32,
        modifiers: KeyModifiers,
        time: i64,
    },
    KeyUp {
        key: Key,
        code_point: i32,
        modifiers: KeyModifiers,
        time: i64,
    },
    /// Text typed (a key's, or an input method's commit).
    Text(String),
    /// An input method's composing text (its preedit; empty: none), its caret at byte `cursor.0` of it, or the part
    /// of it from `cursor.0` to `cursor.1` highlighted (none: the caret hidden, after it).
    Preedit {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    /// An input method deleted `before` bytes before the caret and `after` after it.
    DeleteSurroundingText {
        before: usize,
        after: usize,
    },
    /// The system's back (Android's button).
    Back,
    /// A drag from another app: begun, moved over the window, a file's path or some text, dropped.
    DropBegin {
        x: f32,
        y: f32,
    },
    DropMove {
        x: f32,
        y: f32,
    },
    DropFile {
        x: f32,
        y: f32,
        path: String,
    },
    DropText {
        x: f32,
        y: f32,
        text: String,
    },
    DropComplete {
        x: f32,
        y: f32,
    },
    /// The app's own drag (one the platform took as it left the window: `PlatformCallbacks::start_drag_and_drop`)
    /// dropped on the window, before its `DropComplete`: what it carries is the app's own (the host has it), nothing
    /// read from the platform.
    DropOwn {
        x: f32,
        y: f32,
    },
    /// The platform's drag of the app's own ended: dropped and taken with `action` (the one the target chose), or
    /// not (none: called off, dropped on nothing, refused).
    OwnDragEnded {
        action: Option<DragAndDropTransferAction>,
    },
    // Things learned on other threads: the desktop's accent color, the wallpaper's colors, an assistive
    // technology's requests (the platform's own: Platform::handled acts on them).
    AccentChanged,
    WallpaperChanged,
    AccessibilityRequest,
    /// Asked for by the host's waker (work for the next frame).
    Wake,
}

impl Event {
    /// Whether it is about the window itself (not input): what meets the window as it is, before metrics changed
    /// are read again.
    pub fn is_window_event(&self) -> bool {
        matches!(
            self,
            Event::FocusGained
                | Event::FocusLost
                | Event::Shown
                | Event::Hidden
                | Event::Minimized
                | Event::Maximized
                | Event::Restored
                | Event::Exposed
                | Event::Resized
                | Event::MetricsChanged
                | Event::PointerLeft
        )
    }

    /// The keys held, for the events that carry them.
    pub fn modifiers(&self) -> Option<KeyModifiers> {
        match self {
            Event::MouseMove { modifiers, .. }
            | Event::MouseDown { modifiers, .. }
            | Event::MouseUp { modifiers, .. }
            | Event::Wheel { modifiers, .. }
            | Event::KeyDown { modifiers, .. }
            | Event::KeyUp { modifiers, .. } => Some(*modifiers),
            _ => None,
        }
    }
}

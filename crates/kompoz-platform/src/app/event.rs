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
    /// The app is to end (a signal, the session ending), whatever its windows' own closing does.
    Quit,
    // The window.
    /// The user closes the window (its title bar's ×, the window manager's close, Alt+F4): the main window's ends
    /// the app (C++'s), a `Window`'s calls its `on_close_request`.
    CloseRequest,
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
    /// Fingers moved together (one report of the platform's: Android's `ACTION_MOVE`), each `(pointer, x, y)`: one
    /// pointer event for all of them, as Compose's.
    TouchMoves {
        moves: Vec<(i64, f32, f32)>,
        time: i64,
    },
    /// The system took the fingers' gesture (Android's `ACTION_CANCEL`): what they began is called off, nothing
    /// clicked.
    TouchCancel,
    /// The positions pointer `pointer` (a finger's id, a stylus's, the mouse's 0) passed through since its last
    /// report, oldest first, each `(x, y, time)`: what the platform batched (Android's historical samples), given
    /// just before the report it leads to (its move), which carries them.
    History {
        pointer: i64,
        samples: Vec<(f32, f32, i64)>,
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
    /// An input method selected bytes `start` to `end` of the field's text (the same: the caret there), the
    /// composition kept (Compose's SetSelectionCommand): the part it reconverts, where its edit away from the caret
    /// begins (Windows' text services edit the field anywhere).
    SetSelection {
        start: usize,
        end: usize,
    },
    /// The system's back (Android's button).
    Back,
    /// A drag from another app: begun (offering `mime_types`, as the platform describes them: empty, not told),
    /// moved over the window, what it carries (a file's path or some text, as a script gives them; or what the
    /// platform reads: `DropData`), dropped.
    DropBegin {
        x: f32,
        y: f32,
        mime_types: Vec<String>,
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
    /// What it dropped is the platform's to give ([`Platform::take_drop_entry`](super::Platform::take_drop_entry)):
    /// all it offers, read as a target asks for it until the drop is done.
    DropData {
        x: f32,
        y: f32,
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
    // The app's tray icons (to the app's core, whichever window they come through).
    /// Tray icon `tray` clicked (activated): its `on_action`.
    TrayAction {
        tray: u64,
    },
    /// An item of tray icon `tray`'s menu clicked: where it is, an index at each level (a submenu's items below it).
    TrayMenuItem {
        tray: u64,
        path: Vec<usize>,
    },
    // A window's menu bar, where the platform shows it (a global menu, Windows' and macOS's own).
    /// An item of window `window`'s menu bar clicked: where it is (the menu's index, then an index at each level).
    MenuBarItem {
        window: kompoz_ui::window::WindowId,
        path: Vec<usize>,
    },
    /// Where the platform can show a menu bar changed (a global menu came or went): each window shows its own again
    /// ([`Platform::set_menu_bar`](super::Platform::set_menu_bar) asked again).
    MenuBarHostChanged,
}

impl Event {
    /// Whether it is about the window itself (not input): what meets the window as it is, before metrics changed
    /// are read again.
    pub fn is_window_event(&self) -> bool {
        matches!(
            self,
            Event::FocusGained
                | Event::FocusLost
                | Event::CloseRequest
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

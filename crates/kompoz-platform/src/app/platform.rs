//! What a KompozR app asks of the platform it runs on, and what the platform asks of the app core (C++'s
//! platform.hpp): a window, its events turned into [`Event`]s, its frames on the display's beat, and what it adds to
//! the host (text input, the clipboard, cursors, insets, screen readers); the app's tray icons and notifications.
//!
//! A [`Platform`] is one window's (each app core has its own: the main window's, a `Window`'s, the app's
//! composition's that has none): the platform's windows by their [`WindowId`], the display's connection shared
//! between them (Wayland's, X11's: each window's `Platform` a handle on it and its own window), the events it reads
//! routed to the core of the window they are for ([`AppWindows`](super::AppWindows)). Each platform has its own; it
//! lives on the UI thread (not `Send`). Platforms that give an app one window (Android, the web) make none for a
//! `Window` (their factory makes no `Platform`).

use super::Event;
use kompoz_gpu::device::{Backend, Surface};
use kompoz_graphics::{Dp, Rect};
use kompoz_ui::clipboard::ClipEntry;
use kompoz_ui::input::KeyModifiers;
use kompoz_ui::media_query::MediaInfo;
use kompoz_ui::platform::PlatformCallbacks;
use kompoz_ui::window::{MenuItemKind, Notification, WindowId, WindowPlacement};
use kompoz_ui::{KompozHost, MotionDurationScale};
use std::rc::Rc;

/// A window as it is opened (the app's from its configuration, a `Window`'s from its request and state): its
/// title, its size in dp, where it goes (none: where the platform puts it; dp from the screen's top left), how it is
/// placed, whether it starts minimized, and what it is ([`Window`](kompoz_ui::window::Window)'s parameters).
#[derive(Clone, PartialEq, Debug)]
pub struct WindowSpec {
    pub id: WindowId,
    pub title: String,
    pub width: Dp,
    pub height: Dp,
    /// Where on the screen, dp from its top left (none: the platform's place; an aligned one is worked out by the
    /// app core where the platform tells the screen's size: [`Platform::screen_size`]).
    pub position: Option<(Dp, Dp)>,
    pub placement: WindowPlacement,
    pub minimized: bool,
    pub undecorated: bool,
    /// What is behind the window shows where its pixels aren't opaque (an ARGB visual on X11, the surface's alpha on
    /// Wayland; presented premultiplied: `Surface::transparent`). Read as it opens.
    pub transparent: bool,
    pub resizable: bool,
    pub focusable: bool,
    pub always_on_top: bool,
    /// The app's name (its cache directory's, its window class's): the app's title, whichever window this is.
    pub app_title: String,
}

impl WindowSpec {
    /// The main window of the app titled `title`, `width` by `height` dp.
    pub fn main(title: &str, width: Dp, height: Dp) -> WindowSpec {
        WindowSpec {
            id: WindowId::MAIN,
            title: title.to_string(),
            width,
            height,
            position: None,
            placement: WindowPlacement::Floating,
            minimized: false,
            undecorated: false,
            transparent: false,
            resizable: true,
            focusable: true,
            always_on_top: false,
            app_title: title.to_string(),
        }
    }
}

/// A tray icon's picture, RGBA (straight), the top row first.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TrayImage {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
}

/// Who shows a window's menu bar ([`Platform::set_menu_bar`]'s answer).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum MenuBarShown {
    /// KompozR, at the window's top (its keys, mnemonics and the items' shortcuts KompozR's).
    #[default]
    Drawn,
    /// The desktop, somewhere of its own (KDE's global menu through dbusmenu): not drawn in the window; the items'
    /// shortcuts still KompozR's to run (the desktop only shows them).
    Exported,
    /// The platform's own menu (Windows' HMENU, macOS's NSMenu): not drawn; the items' shortcuts the platform's
    /// (its accelerator table, its key equivalents), a click an [`Event::MenuBarItem`].
    Native,
}

/// A menu's item as the platform shows it: a tray icon's, a window's menu bar's (a `MenuItem` without its
/// callbacks: a click comes back as an [`Event::TrayMenuItem`] or [`Event::MenuBarItem`] by where it is).
#[derive(Clone, PartialEq, Debug)]
pub struct TrayMenuItem {
    pub kind: MenuItemKind,
    pub text: String,
    pub enabled: bool,
    pub mnemonic: Option<char>,
    /// Its shortcut's keys: the modifiers held, and the key's name (xkb's: "q", "F4", "Delete").
    pub shortcut: Option<(KeyModifiers, String)>,
    pub icon: Option<TrayImage>,
    pub items: Vec<TrayMenuItem>,
}

/// A tray icon as the platform shows it: its id (what its events name), its pictures (the sizes the tray may
/// ask for), its tooltip, its menu.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TrayIcon {
    pub id: u64,
    pub images: Vec<TrayImage>,
    pub tooltip: String,
    pub menu: Vec<TrayMenuItem>,
}

/// A button of the title bar KompozR draws.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WindowButton {
    Minimize,
    Maximize,
    Close,
}

/// The title bar KompozR draws above the app where the platform leaves the window undecorated (Wayland without the
/// compositor's decorations, as on GNOME): its buttons on each side, in order, those the window manager has (the
/// desktop's layout: GNOME's has only Close), and whether the window is maximized.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TitleBar {
    pub start: Vec<WindowButton>,
    pub end: Vec<WindowButton>,
    pub maximized: bool,
}

/// What the title bar asks of the window.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WindowAction {
    /// Dragged by the pointer or finger last pressed on it.
    Move,
    Minimize,
    ToggleMaximize,
    Close,
    /// The window manager's menu, where the action says.
    Menu,
}

/// The window's measures, as the app lays out and draws by them (C++'s Metrics, in px where C++ had dp).
#[derive(Clone, PartialEq, Debug)]
pub struct Metrics {
    /// The drawable's size, px.
    pub pixel_width: i32,
    pub pixel_height: i32,
    /// The window's size in the points its events' positions are in.
    pub point_width: i32,
    pub point_height: i32,
    /// px per dp (the density).
    pub scale: f32,
    /// px per point (the window's events in px).
    pub point_to_px: f32,
    /// The system's text size.
    pub font_scale: f32,
    /// The display's refresh rate, a second.
    pub refresh_rate: f32,
    /// Where the system leaves the window's content (a desktop's or an iPhone's safe area), in points; none where
    /// the insets come otherwise (Android's, set on the host by the platform).
    pub safe_area: Option<Rect>,
    /// The system's theme.
    pub dark_theme: bool,
    /// The preferred language ("tr", "en"; empty: none).
    pub language: String,
    /// Drawn by KompozR at the top of the window; none: the system's, or none at all.
    pub title_bar: Option<TitleBar>,
}

impl Default for Metrics {
    fn default() -> Self {
        Metrics {
            pixel_width: 1,
            pixel_height: 1,
            point_width: 1,
            point_height: 1,
            scale: 1.0,
            point_to_px: 1.0,
            font_scale: 1.0,
            refresh_rate: 60.0,
            safe_area: None,
            dark_theme: false,
            language: String::new(),
            title_bar: None,
        }
    }
}

/// A platform, as the app core uses it (C++'s Platform). Its event loop is its own: it gives the core the events it
/// gathered ([`AppCore::handle`](super::AppCore::handle)) and asks it for frames ([`AppCore::frame`]).
///
/// Re-entrancy: the core calls these between its own work, never from inside the host; the callbacks the platform
/// gives the host ([`attach`](Platform::attach)) keep their state apart from the platform's, so that the host can
/// call them while the platform is being used.
///
/// [`AppCore::frame`]: super::AppCore::frame
pub trait Platform {
    /// A setting from outside the app (`KOMPOZ_PROFILE`, `KOMPOZ_GPU`): an environment variable, or where an app has
    /// none (Android) what the platform keeps for it. None: not set.
    fn option(&self, name: &str) -> Option<String>;
    /// A line into the platform's log.
    fn log(&self, message: &str);
    /// Now, ns, on the clock the events' times are on.
    fn now(&self) -> i64;
    /// The GPU backends `KOMPOZ_GPU=auto` tries here, the best first.
    fn backends(&self) -> Vec<Backend>;
    /// The window, as `window` says, made to be drawn into with `backend`: what the device needs of it (the surface;
    /// its pipeline cache is the core's); why not if it can't be. Once only, or after `close_window` (a window
    /// hidden and shown again).
    fn open_window(&mut self, window: &WindowSpec, backend: Backend) -> Result<Surface, String>;
    /// The window goes (after the device drawing into it is gone).
    fn close_window(&mut self);
    /// What ended the app, to the user.
    fn show_error(&mut self, message: &str);
    fn metrics(&self) -> Metrics;
    /// The drawable's size, px, as `metrics` has it (cheaper: asked each frame).
    fn pixel_size(&self) -> (i32, i32);
    /// Resizes the window (in points).
    fn set_window_size(&mut self, width: i32, height: i32);
    /// What KompozR's title bar asks ([`Metrics::title_bar`]); (x, y): where, in points.
    fn window_action(&mut self, action: WindowAction, x: f32, y: f32) {
        let _ = (action, x, y);
    }
    /// The pointer and the keyboard; with `probe` (after the first frame: what tells may take a few tens of
    /// milliseconds to start) a microphone and a camera looked for too, kept after.
    fn media(&mut self, probe: bool) -> MediaInfo;
    /// The host made: what the platform gives it (waking, text input, the clipboard, cursors, links), as callbacks
    /// (the core serves images, bitmaps and assets itself and passes the rest on to these).
    fn attach(&mut self, host: &KompozHost) -> Rc<dyn PlatformCallbacks>;
    /// The host given its callbacks (and a state file's state): what the platform starts on it (a phone's saved
    /// state back, a deep link it was given).
    fn started(&mut self, host: &KompozHost) {
        let _ = host;
    }
    /// The metrics read again: what the platform sets on the host besides (a phone's palettes).
    fn metrics_updated(&mut self, host: &KompozHost) {
        let _ = host;
    }
    /// An event the core handled: what the platform does after it (the accent color's palette, an assistive
    /// technology's requests).
    fn handled(&mut self, host: &KompozHost, event: &Event) {
        let _ = (host, event);
    }
    /// After a frame's composition: what the platform takes from the host (the semantics tree for screen readers,
    /// the frame rate, the screen kept on).
    fn composed(&mut self, host: &KompozHost, redraw: bool, frame_time: i64);
    /// What a drop the platform reported (its `Event::DropData`) carries: all it offers, read from its source as a
    /// target asks for it until the drop is done (`drop_done`). Taken once.
    fn take_drop_entry(&mut self) -> Option<ClipEntry> {
        None
    }
    /// A drop the platform reported (its `DropComplete`) handled: whether a target in the app took it — what the
    /// platform tells the drag's source (XDND's `XdndFinished`, a Wayland offer finished or let go).
    fn drop_done(&mut self, taken: bool) {
        let _ = taken;
    }
    /// A frame more wanted for the platform's own sake (a semantics tree still to publish).
    fn wants_frames(&self) -> bool {
        false
    }
    /// A frame's own work (its waits left out), for a system that wants it (Android's ADPF).
    fn frame_work(&mut self, ns: i64) {
        let _ = ns;
    }
    /// Back went unused: the system's own (Android leaves the app).
    fn leave_app(&mut self) {}
    /// How long the app's animations take as the system asks (a window's `MotionDurationScale`, read as each
    /// animation asks: macOS's reduced motion ends them at once); none: the system says nothing (they play in real
    /// time). Asked once, as the host is made.
    fn motion_duration_scale(&self) -> Option<MotionDurationScale> {
        None
    }

    // ---- The window as the app sets it (a `Window`'s state and parameters; the app core calls these between
    // frames where they changed) ----

    /// Its title.
    fn set_title(&mut self, title: &str) {
        let _ = title;
    }
    /// Maximized, full screen or neither.
    fn set_placement(&mut self, placement: WindowPlacement) {
        let _ = placement;
    }
    /// Minimized or restored. (Wayland can't restore a window from the app: only bring it forward, `to_front`.)
    fn set_minimized(&mut self, minimized: bool) {
        let _ = minimized;
    }
    /// Moved to `x`, `y` on the screen, its points (where the platform lets an app place its windows: not Wayland).
    fn set_position(&mut self, x: i32, y: i32) {
        let _ = (x, y);
    }
    /// To the front, given the keyboard's focus (Wayland: through an xdg-activation token).
    fn to_front(&mut self) {}
    /// Whether the user can resize it.
    fn set_resizable(&mut self, resizable: bool) {
        let _ = resizable;
    }
    /// Kept above the others.
    fn set_always_on_top(&mut self, on_top: bool) {
        let _ = on_top;
    }
    /// Its icon (none: the app's).
    fn set_icon(&mut self, icon: Option<&TrayImage>) {
        let _ = icon;
    }
    /// Its menu bar's menus as they are now (each a `Menu` item with its items; none: it has none any more): what
    /// shows it. The platform's own menu (Windows' HMENU, macOS's main NSMenu while the window is key) answers
    /// [`MenuBarShown::Native`], the desktop's global menu (Linux's dbusmenu export) [`MenuBarShown::Exported`]; by
    /// default KompozR draws it at the window's top ([`MenuBarShown::Drawn`], as Compose Desktop's Swing JMenuBar
    /// on Linux). Clicks on a platform's menu come back as [`Event::MenuBarItem`]. Asked again as the menus change
    /// and after [`Event::MenuBarHostChanged`].
    fn set_menu_bar(&mut self, menus: Option<&[TrayMenuItem]>) -> MenuBarShown {
        let _ = menus;
        MenuBarShown::Drawn
    }
    /// Where it is on the screen, its points (none: not told, as on Wayland).
    fn window_position(&self) -> Option<(i32, i32)> {
        None
    }
    /// How the window manager placed it, as it last said (none: not told).
    fn window_placement(&self) -> Option<WindowPlacement> {
        None
    }
    /// The size of the screen it is on, its points (none: not told): what an aligned position is worked out in.
    fn screen_size(&self) -> Option<(i32, i32)> {
        None
    }

    // ---- The app's tray icons and notifications (the app core's platform: the main window's, or the app's
    // composition's) ----

    /// Whether this platform shows tray icons (a StatusNotifierItem host on Linux).
    fn tray_supported(&self) -> bool {
        false
    }
    /// The app's tray icons as they are now (all of them, each time any changed): shown, changed or taken away.
    fn sync_trays(&mut self, trays: &[TrayIcon]) {
        let _ = trays;
    }
    /// A notification of the desktop's, sent through tray icon `tray` (its icon the notification's where its type
    /// is None).
    fn send_notification(&mut self, tray: &TrayIcon, notification: &Notification) {
        let _ = (tray, notification);
    }
}

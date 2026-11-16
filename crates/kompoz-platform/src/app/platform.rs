//! What a KompozR app asks of the platform it runs on, and what the platform asks of the app core (C++'s
//! platform.hpp): its window (one), its events turned into [`Event`]s, its frames on the display's beat, and what it
//! adds to the host (text input, the clipboard, cursors, insets, screen readers). Each platform has its own; it
//! lives on the UI thread (not `Send`).

use super::{AppConfig, Event};
use kompoz_gpu::device::{Backend, Surface};
use kompoz_graphics::Rect;
use kompoz_ui::KompozHost;
use kompoz_ui::media_query::MediaInfo;
use kompoz_ui::platform::PlatformCallbacks;
use std::rc::Rc;

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
    /// The window, made to be drawn into with `backend`: what the device needs of it (the surface; its pipeline
    /// cache is the core's); why not if it can't be. Once only, or after `close_window`.
    fn open_window(&mut self, app: &AppConfig, backend: Backend) -> Result<Surface, String>;
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
}

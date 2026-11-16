//! A KompozR app on a platform (C++'s platform/app and main.cpp): the app's entry ([`App`]), what a platform is to
//! the app core ([`Platform`], [`Metrics`], [`Event`]), the core every platform runs the app with ([`AppCore`]), the
//! title bar it draws where the platform leaves the window undecorated ([`WindowFrame`]), and the display platforms
//! built in ([`DISPLAY_PLATFORMS`]).
//!
//! ```text
//! fn main() -> ExitCode {
//!     App::new("Sayaç").size(360.dp(), 480.dp()).run(Counter)
//! }
//! ```

mod app_core;
mod event;
mod platform;
mod profile;
pub mod regions;
mod script;
mod title_bar;

pub use app_core::{AppCore, DeviceFactory, Next, WHEEL_NOTCH_DP};
pub use event::{Event, MouseButton};
pub use platform::{Metrics, Platform, TitleBar, WindowAction, WindowButton};
pub use title_bar::{
    TITLE_BAR_BUTTON, TITLE_BAR_GAP, TITLE_BAR_HEIGHT, TITLE_BAR_ID, TitleBarColors, WindowActions, WindowFrame,
    button_action, button_icon,
};

use crate::system::log;
use kompoz_graphics::Dp;
use kompoz_runtime::Content;
use std::process::ExitCode;
use std::rc::Rc;

/// What the app is (C++'s Application): its title, its window's first size (dp, desktops only), how much of what
/// it downloads it keeps, the command line it was started with.
#[derive(Clone, PartialEq, Debug)]
pub struct AppConfig {
    pub title: String,
    pub width: Dp,
    pub height: Dp,
    /// Images AsyncImage downloaded are kept in the app's cache directory up to this size (the least recently used
    /// go first), for the next time and the next run; 0: not kept.
    pub image_disk_cache_bytes: u64,
    /// The program's arguments (the first its name): an address in them is a deep link.
    pub args: Vec<String>,
}

impl AppConfig {
    /// "KompozR"'s, 480 by 800 dp, a 64 MB image cache, the process's arguments.
    pub fn new(title: impl Into<String>) -> AppConfig {
        AppConfig {
            title: title.into(),
            width: Dp(480.0),
            height: Dp(800.0),
            image_disk_cache_bytes: 64 << 20,
            args: std::env::args().collect(),
        }
    }
}

/// A display platform's entry point: runs the app on it until it quits (what the program returns), or none if it
/// can't run here (no display: the next is tried).
pub type DisplayPlatformRun = fn(&AppConfig, Rc<dyn Content>) -> Option<ExitCode>;

/// A display platform built in.
#[derive(Clone, Copy, Debug)]
pub struct DisplayPlatform {
    /// What `KOMPOZ_PLATFORM` names it ("wayland", "x11").
    pub name: &'static str,
    pub run: DisplayPlatformRun,
}

/// The display platforms built in, tried in this order when the app isn't headless (C++'s main: a Wayland
/// session's, then X11's).
#[cfg(target_os = "linux")]
pub const DISPLAY_PLATFORMS: &[DisplayPlatform] = &[crate::linux::wayland::PLATFORM, crate::linux::x11::PLATFORM];
#[cfg(not(target_os = "linux"))]
pub const DISPLAY_PLATFORMS: &[DisplayPlatform] = &[];

/// A KompozR app (C++'s Application and main): `App::new(title).size(w, h).run(content)`.
#[derive(Clone, PartialEq, Debug)]
pub struct App {
    config: AppConfig,
}

impl App {
    /// An app titled `title` (its window's, its cache directory's name).
    pub fn new(title: impl Into<String>) -> App {
        App { config: AppConfig::new(title) }
    }

    /// Its window's first size (desktops).
    pub fn size(mut self, width: Dp, height: Dp) -> App {
        self.config.width = width;
        self.config.height = height;
        self
    }

    /// How much of the images it downloads it keeps (0: none).
    pub fn image_disk_cache_bytes(mut self, bytes: u64) -> App {
        self.config.image_disk_cache_bytes = bytes;
        self
    }

    /// The program's arguments instead of the process's (the first its name).
    pub fn args(mut self, args: Vec<String>) -> App {
        self.config.args = args;
        self
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Runs the app until it quits; what the program returns (C++'s main): headless with `KOMPOZ_HEADLESS=1`, else
    /// on the display platforms built in, in order (`KOMPOZ_PLATFORM=x11` and the like: that one only), the first
    /// that can run here.
    pub fn run(self, content: impl Content + 'static) -> ExitCode {
        let content: Rc<dyn Content> = Rc::new(content);
        if crate::headless::headless_requested() {
            return crate::headless::run_headless(&self.config, content);
        }
        let only = std::env::var("KOMPOZ_PLATFORM").ok().filter(|p| !p.is_empty());
        if DISPLAY_PLATFORMS.is_empty() {
            log("kompoz: no display platform built in (KOMPOZ_HEADLESS=1 runs the app without a window)");
            return ExitCode::FAILURE;
        }
        for platform in DISPLAY_PLATFORMS {
            if only.as_deref().is_some_and(|o| o != platform.name) {
                continue;
            }
            if let Some(code) = (platform.run)(&self.config, content.clone()) {
                return code;
            }
        }
        let names: Vec<&str> = DISPLAY_PLATFORMS.iter().map(|p| p.name).collect();
        log(&format!(
            "kompoz: no display to run on ({}{}; or KOMPOZ_HEADLESS=1)",
            names.join(", "),
            only.map(|o| format!(", KOMPOZ_PLATFORM={o}")).unwrap_or_default()
        ));
        ExitCode::FAILURE
    }
}

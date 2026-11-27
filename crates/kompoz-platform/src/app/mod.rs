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
pub mod entry;
mod event;
pub mod menu_bar;
mod platform;
mod profile;
pub mod regions;
mod script;
mod title_bar;
mod windows;

pub use app_core::{AppCore, DeviceFactory, Next, lifecycle_state_of, paint_image};
pub use entry::{application, single_window_application};
pub use event::{Event, MouseButton};
pub use kompoz_gpu::device::GpuPriority;
pub use menu_bar::{MENU_BAR_HEIGHT, MENU_BAR_ID, MenuBarColors, MenuBarFrame, MenuBarView, MenuNav};
pub use platform::{
    MenuBarShown, Metrics, Platform, TitleBar, TrayIcon, TrayImage, TrayMenuItem, WindowAction, WindowButton,
    WindowSpec,
};
pub use title_bar::{
    TITLE_BAR_BUTTON, TITLE_BAR_GAP, TITLE_BAR_HEIGHT, TITLE_BAR_ID, TitleBarColors, WindowActions, WindowFrame,
    button_action, button_icon,
};
pub use windows::{AppWindows, MakeWindow, key_name};

use crate::system::log;
use kompoz_graphics::Dp;
use kompoz_runtime::Content;
use kompoz_runtime::kompozable_block;
use kompoz_ui::draw::Painter;
use kompoz_ui::input::KeyEvent;
use kompoz_ui::window::{ApplicationScope, FrameWindowScope, SingleWindowApplicationScope, Window, WindowState};
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
    /// Which GPU draws where there are two (C++'s Application `gpuPriority`; `KOMPOZ_GPU_PRIORITY` instead).
    pub gpu_priority: GpuPriority,
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
            gpu_priority: GpuPriority::Auto,
        }
    }
}

/// What an app composes: its main window's content (`App::run`: closing that window ends the app), or a
/// composition without a window of its own (`application { }`: its `Window`s and `Tray`s; it ends with
/// `exit_application` or when nothing is left in it).
#[derive(Clone)]
pub enum AppContent {
    Window(Rc<dyn Content>),
    Application(Rc<dyn Fn(&ApplicationScope)>),
}

impl AppContent {
    /// The app's root core for it on `platform` (not started).
    pub fn core<P: Platform>(&self, platform: P, config: &AppConfig) -> AppCore<P> {
        match self {
            AppContent::Window(content) => AppCore::new(platform, config.clone(), content.clone()),
            AppContent::Application(content) => AppCore::application(platform, config.clone(), content.clone()),
        }
    }
}

/// A display platform's entry point: runs the app on it until it quits (what the program returns), or none if it
/// can't run here (no display: the next is tried).
pub type DisplayPlatformRun = fn(&AppConfig, AppContent) -> Option<ExitCode>;

/// A display platform built in.
#[derive(Clone, Copy, Debug)]
pub struct DisplayPlatform {
    /// What `KOMPOZ_PLATFORM` names it ("wayland", "x11").
    pub name: &'static str,
    pub run: DisplayPlatformRun,
}

/// The display platforms built in, tried in this order when the app isn't headless (C++'s main: a Wayland
/// session's, then X11's; Win32 on Windows).
#[cfg(target_os = "linux")]
pub const DISPLAY_PLATFORMS: &[DisplayPlatform] = &[crate::linux::wayland::PLATFORM, crate::linux::x11::PLATFORM];
#[cfg(windows)]
pub const DISPLAY_PLATFORMS: &[DisplayPlatform] = &[crate::windows::PLATFORM];
#[cfg(target_os = "macos")]
pub const DISPLAY_PLATFORMS: &[DisplayPlatform] = &[crate::macos::PLATFORM];
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub const DISPLAY_PLATFORMS: &[DisplayPlatform] = &[crate::web::PLATFORM];
#[cfg(not(any(
    target_os = "linux",
    windows,
    target_os = "macos",
    all(target_family = "wasm", target_os = "unknown")
)))]
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

    /// Which GPU draws where there are two (C++'s Application `gpuPriority`: Auto, the screen's, else the
    /// integrated one; Integrated; Discrete, else the integrated one). `KOMPOZ_GPU_PRIORITY=auto|integrated|discrete`
    /// overrides it, `KOMPOZ_GPU_DEVICE` both.
    pub fn gpu_priority(mut self, priority: GpuPriority) -> App {
        self.config.gpu_priority = priority;
        self
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Runs the app until it quits; what the program returns (C++'s main): headless with `KOMPOZ_HEADLESS=1`, else
    /// on the display platforms built in, in order (`KOMPOZ_PLATFORM=x11` and the like: that one only), the first
    /// that can run here. On the web it returns as the app starts (the page runs it on: [`web`](crate::web)).
    pub fn run(self, content: impl Content + 'static) -> ExitCode {
        self.run_content(AppContent::Window(Rc::new(content)))
    }

    /// Runs the app as Compose Desktop's `application { }` does: `content` composed without a window of its own,
    /// given the app's scope (`exit_application`), its `Window`s and `Tray`s shown; until it exits or nothing is
    /// left in it. What the program returns.
    pub fn application(self, content: impl Fn(&ApplicationScope) + 'static) -> ExitCode {
        self.run_content(AppContent::Application(Rc::new(content)))
    }

    /// Runs `content` as [`run`](App::run) does (headless, or on the first display platform that can).
    pub fn run_content(self, content: AppContent) -> ExitCode {
        if crate::headless::headless_requested() {
            return crate::headless::run_headless(&self.config, content);
        }
        let only = kompoz_runtime::environment::var("KOMPOZ_PLATFORM").filter(|p| !p.is_empty());
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

/// What `single_window_application` composes in the app's scope: its one `Window`, closed by `exit_application`
/// (a test's headless run composes the same; not Kotlin's: the platform's entry module is).
#[allow(clippy::too_many_arguments)]
pub fn single_window_content(
    state: WindowState,
    visible: bool,
    title: String,
    icon: Option<Rc<dyn Painter>>,
    undecorated: bool,
    transparent: bool,
    resizable: bool,
    enabled: bool,
    focusable: bool,
    always_on_top: bool,
    on_preview_key_event: Rc<dyn Fn(&KeyEvent) -> bool>,
    on_key_event: Rc<dyn Fn(&KeyEvent) -> bool>,
    content: Rc<dyn Fn(&SingleWindowApplicationScope)>,
) -> impl Fn(&ApplicationScope) + 'static {
    move |app: &ApplicationScope| {
        let (state, title, icon) = (state.clone(), title.clone(), icon.clone());
        let (preview, after, content) = (on_preview_key_event.clone(), on_key_event.clone(), content.clone());
        let (closer, scope) = (app.clone(), app.clone());
        kompozable_block!({
            Window(
                on_close_request = move || closer.exit_application(),
                state = state,
                visible = visible,
                title = title,
                icon = icon,
                undecorated = undecorated,
                transparent = transparent,
                resizable = resizable,
                enabled = enabled,
                focusable = focusable,
                always_on_top = always_on_top,
                on_preview_key_event = move |e: &KeyEvent| preview(e),
                on_key_event = move |e: &KeyEvent| after(e),
                content = move |frame: &FrameWindowScope| {
                    content(&SingleWindowApplicationScope::new(scope.clone(), frame.clone()));
                },
            );
        });
    }
}

//! KompozR's platform without a screen (C++'s headless platform): tests, scripted runs and screenshots, CI.
//! `KOMPOZ_HEADLESS=1` runs an app on it: each window a size and nothing more (the app's, and a `Window`'s each of
//! their own, sharing one loop), the GPU renderer drawing only into their kept frames (or, with
//! `KOMPOZ_HEADLESS_SWAPCHAIN=1`, presenting to a Vulkan headless surface, as a window would: the swapchain's path
//! without a display), no input but a script's (`KOMPOZ_SCRIPT`, "@Title: command" for a `Window`'s), the loop and
//! its timers its own. What its windows are (their titles, sizes, positions, placements), its tray icons and the
//! notifications sent are kept ([`HeadlessShared`]) for tests to read.
//!
//! - `KOMPOZ_SCALE`: px per dp (a phone's 2.625, say); `KOMPOZ_FONT_SCALE`: the system's text size.
//! - `KOMPOZ_HEADLESS_SCREEN=WxH`: the screen's size (points) windows are aligned in (1920x1080).

use crate::app::{
    AppConfig, AppContent, AppCore, AppWindows, Event, Metrics, Platform, TrayIcon, TrayImage, WindowSpec,
};
use crate::system::{log, preferred_language};
use kompoz_gpu::device::{Backend, NATIVE_BACKEND, Surface};
use kompoz_graphics::Rect;
use kompoz_runtime::clock::Instant;
use kompoz_ui::KompozHost;
use kompoz_ui::clipboard::ClipEntry;
use kompoz_ui::media_query::MediaInfo;
use kompoz_ui::platform::PlatformCallbacks;
use kompoz_ui::window::{Notification, WindowId, WindowPlacement, WindowRequest};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
use std::time::Duration;

/// `KOMPOZ_HEADLESS=1`.
pub fn headless_requested() -> bool {
    kompoz_runtime::environment::flag("KOMPOZ_HEADLESS")
}

/// The headless platform's clock: ns since the process first asked.
fn now() -> i64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_nanos() as i64
}

/// The events posted to the loop, from any thread, each for a window, and what wakes it.
#[derive(Default)]
pub struct EventQueue {
    events: Mutex<VecDeque<(WindowId, Event)>>,
    posted: Condvar,
}

impl EventQueue {
    /// `event` to the main window's core before its next frame, waking the loop.
    pub fn post(&self, event: Event) {
        self.post_to(WindowId::MAIN, event);
    }

    /// `event` to window `window`'s core (the main window's, the app's composition's: the root's).
    pub fn post_to(&self, window: WindowId, event: Event) {
        self.events.lock().unwrap_or_else(|e| e.into_inner()).push_back((window, event));
        self.posted.notify_one();
    }

    /// What was posted, taken.
    pub fn take(&self) -> VecDeque<(WindowId, Event)> {
        std::mem::take(&mut *self.events.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// What was posted for `window`, dropped (it closed).
    fn discard(&self, window: WindowId) {
        self.events.lock().unwrap_or_else(|e| e.into_inner()).retain(|(w, _)| *w != window);
    }

    /// Waits until something is posted, or until `wake_at` (the platform's clock; none: as long as it takes). On the
    /// web it doesn't wait: there is one thread, so nothing could be posted meanwhile (and a page can't block).
    pub fn wait(&self, wake_at: Option<i64>) {
        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        {
            let _ = wake_at;
        }
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        self.wait_for_post(wake_at);
    }

    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    fn wait_for_post(&self, wake_at: Option<i64>) {
        let events = self.events.lock().unwrap_or_else(|e| e.into_inner());
        let ready = |q: &mut VecDeque<(WindowId, Event)>| q.is_empty();
        match wake_at {
            None => drop(self.posted.wait_while(events, ready)),
            Some(at) => {
                let timeout = Duration::from_nanos((at - now()).max(0) as u64);
                drop(self.posted.wait_timeout_while(events, timeout, ready));
            }
        }
    }
}

/// A headless window as the app left it (what a test reads).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct HeadlessWindow {
    pub title: String,
    /// Its size, px (its points).
    pub size: (i32, i32),
    /// Where the app put it (points; none: the platform's place).
    pub position: Option<(i32, i32)>,
    pub placement: WindowPlacement,
    pub minimized: bool,
    pub resizable: bool,
    pub always_on_top: bool,
    pub undecorated: bool,
    /// `to_front`s asked.
    pub fronted: u32,
    pub icon: Option<TrayImage>,
}

/// What the headless platform's windows share: the loop's queue, a clipboard of the app's own, the settings given
/// instead of the environment's, which window has the focus, the windows open, the tray icons shown and the
/// notifications sent.
#[derive(Default)]
pub struct HeadlessShared {
    queue: Arc<EventQueue>,
    clipboard: RefCell<Option<ClipEntry>>,
    options: RefCell<HashMap<String, String>>,
    focused: Cell<Option<WindowId>>,
    windows: RefCell<BTreeMap<WindowId, HeadlessWindow>>,
    trays: RefCell<Vec<TrayIcon>>,
    notifications: RefCell<Vec<(u64, Notification)>>,
}

impl HeadlessShared {
    /// The windows open, by id.
    pub fn windows(&self) -> BTreeMap<WindowId, HeadlessWindow> {
        self.windows.borrow().clone()
    }
    /// The window titled `title`, if one is open.
    pub fn window_titled(&self, title: &str) -> Option<HeadlessWindow> {
        self.windows.borrow().values().find(|w| w.title == title).cloned()
    }
    /// The window with the focus.
    pub fn focused(&self) -> Option<WindowId> {
        self.focused.get()
    }
    /// The tray icons shown now.
    pub fn trays(&self) -> Vec<TrayIcon> {
        self.trays.borrow().clone()
    }
    /// The notifications sent so far, with the tray icon each came through.
    pub fn notifications(&self) -> Vec<(u64, Notification)> {
        self.notifications.borrow().clone()
    }

    /// The focus to `window`, from the window that had it (each told).
    fn focus(&self, window: WindowId) {
        if let Some(had) = self.focused.replace(Some(window))
            && had != window
            && self.windows.borrow().contains_key(&had)
        {
            self.queue.post_to(had, Event::FocusLost);
        }
        self.queue.post_to(window, Event::FocusGained);
    }
}

/// The callbacks the headless platform gives the host: waking its loop, a clipboard of the app's own.
struct HeadlessCallbacks {
    shared: Rc<HeadlessShared>,
    window: WindowId,
}

impl PlatformCallbacks for HeadlessCallbacks {
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        let (queue, window) = (self.shared.queue.clone(), self.window);
        Some(Arc::new(move || queue.post_to(window, Event::Wake)))
    }
    fn get_clip_entry(&self) -> Option<ClipEntry> {
        self.shared.clipboard.borrow().clone()
    }
    fn set_clip_entry(&self, entry: Option<ClipEntry>) {
        *self.shared.clipboard.borrow_mut() = entry.map(|e| e.materialized());
    }
}

/// The headless platform: a window that is its size, shown and focused at once, as one on a screen would be; the
/// app's other windows ([`for_window`](HeadlessPlatform::for_window)) share its loop, clipboard and settings.
pub struct HeadlessPlatform {
    shared: Rc<HeadlessShared>,
    window: WindowId,
    /// The window's size: its pixels are its points.
    size: Rc<Cell<(i32, i32)>>,
}

impl HeadlessPlatform {
    /// The main window's.
    pub fn new() -> HeadlessPlatform {
        HeadlessPlatform {
            shared: Rc::new(HeadlessShared::default()),
            window: WindowId::MAIN,
            size: Rc::new(Cell::new((1, 1))),
        }
    }

    /// Window `window`'s, sharing this one's loop, clipboard and settings.
    pub fn for_window(&self, window: WindowId) -> HeadlessPlatform {
        HeadlessPlatform { shared: self.shared.clone(), window, size: Rc::new(Cell::new((1, 1))) }
    }

    /// `name` set to `value` instead of the environment's (a test's `KOMPOZ_SCRIPT`), for all its windows.
    pub fn set_option(&mut self, name: &str, value: &str) {
        self.shared.options.borrow_mut().insert(name.to_string(), value.to_string());
    }

    /// Its events' queue (a test posts to it as a window system would).
    pub fn queue(&self) -> Arc<EventQueue> {
        self.shared.queue.clone()
    }

    /// What its windows share (what a test reads).
    pub fn shared(&self) -> Rc<HeadlessShared> {
        self.shared.clone()
    }

    /// Its window's id.
    pub fn window_id(&self) -> WindowId {
        self.window
    }

    fn post(&self, event: Event) {
        self.shared.queue.post_to(self.window, event);
    }

    fn with_window(&self, f: impl FnOnce(&mut HeadlessWindow)) {
        if let Some(w) = self.shared.windows.borrow_mut().get_mut(&self.window) {
            f(w);
        }
    }
}

impl Default for HeadlessPlatform {
    fn default() -> Self {
        HeadlessPlatform::new()
    }
}

impl Platform for HeadlessPlatform {
    fn option(&self, name: &str) -> Option<String> {
        self.shared.options.borrow().get(name).cloned().or_else(|| kompoz_runtime::environment::var(name))
    }

    fn log(&self, message: &str) {
        log(message);
    }

    fn now(&self) -> i64 {
        now()
    }

    /// The target's own: Vulkan, Metal on Apple's systems (as C++'s headless Metal runs), WebGPU on the web.
    fn backends(&self) -> Vec<Backend> {
        vec![NATIVE_BACKEND]
    }

    /// The window is its size (a dp a pixel, as C++'s), given the focus (the one that had it told it lost it), at
    /// its place if it says; what the device needs: nothing (it draws only into textures), or on Vulkan a headless
    /// surface to present to (`KOMPOZ_HEADLESS_SWAPCHAIN=1`).
    fn open_window(&mut self, window: &WindowSpec, backend: Backend) -> Result<Surface, String> {
        if backend != NATIVE_BACKEND {
            return Err(format!("the headless platform has no {} backend", backend.name()));
        }
        let size = (window.width.0.round().max(1.0) as i32, window.height.0.round().max(1.0) as i32);
        self.size.set(size);
        self.shared.windows.borrow_mut().insert(
            self.window,
            HeadlessWindow {
                title: window.title.clone(),
                size,
                position: window.position.map(|(x, y)| (x.0.round() as i32, y.0.round() as i32)),
                placement: window.placement,
                minimized: window.minimized,
                resizable: window.resizable,
                always_on_top: window.always_on_top,
                undecorated: window.undecorated,
                fronted: 0,
                icon: None,
            },
        );
        let drawable = self.size.clone();
        #[cfg_attr(any(target_family = "wasm", target_vendor = "apple"), allow(unused_mut))]
        let mut surface = Surface { drawable_size: Some(Box::new(move || drawable.get())), ..Surface::default() };
        #[cfg(not(any(target_family = "wasm", target_vendor = "apple")))]
        if self.option("KOMPOZ_HEADLESS_SWAPCHAIN").is_some_and(|v| v == "1") {
            surface.vulkan = Some(kompoz_gpu::device::vulkan::headless_surface());
        }
        if window.minimized {
            self.post(Event::Minimized);
        } else if window.focusable {
            self.shared.focus(self.window);
        }
        for e in [Event::Shown, Event::Resized] {
            self.post(e);
        }
        Ok(surface)
    }

    fn close_window(&mut self) {
        self.shared.queue.discard(self.window);
        self.shared.windows.borrow_mut().remove(&self.window);
        if self.shared.focused.get() == Some(self.window) {
            self.shared.focused.set(None);
        }
    }

    fn show_error(&mut self, message: &str) {
        log(message);
    }

    fn metrics(&self) -> Metrics {
        let (w, h) = self.size.get();
        let number = |name: &str| self.option(name).and_then(|v| v.trim().parse::<f32>().ok());
        Metrics {
            pixel_width: w,
            pixel_height: h,
            point_width: w,
            point_height: h,
            scale: number("KOMPOZ_SCALE").filter(|s| *s > 0.0).unwrap_or(1.0),
            font_scale: number("KOMPOZ_FONT_SCALE").map_or(1.0, |s| s.max(0.5)),
            safe_area: Some(Rect::new(0.0, 0.0, w as f32, h as f32)),
            language: preferred_language().unwrap_or_default(),
            ..Metrics::default()
        }
    }

    fn pixel_size(&self) -> (i32, i32) {
        self.size.get()
    }

    fn set_window_size(&mut self, width: i32, height: i32) {
        let size = (width.max(1), height.max(1));
        self.size.set(size);
        self.with_window(|w| w.size = size);
        self.post(Event::Resized);
    }

    /// No pointer, keyboard, microphone or camera.
    fn media(&mut self, _: bool) -> MediaInfo {
        MediaInfo::default()
    }

    fn attach(&mut self, _: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        Rc::new(HeadlessCallbacks { shared: self.shared.clone(), window: self.window })
    }

    fn composed(&mut self, _: &KompozHost, _: bool, _: i64) {}

    fn set_title(&mut self, title: &str) {
        self.with_window(|w| w.title = title.to_string());
    }

    /// As a window manager would: told back (minimized, restored).
    fn set_minimized(&mut self, minimized: bool) {
        let mut was = false;
        self.with_window(|w| was = std::mem::replace(&mut w.minimized, minimized));
        if was != minimized {
            self.post(if minimized { Event::Minimized } else { Event::Restored });
        }
    }

    fn set_placement(&mut self, placement: WindowPlacement) {
        self.with_window(|w| w.placement = placement);
        if placement == WindowPlacement::Maximized {
            self.post(Event::Maximized);
        }
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.with_window(|w| w.position = Some((x, y)));
    }

    /// Restored if it is minimized, the focus given to it.
    fn to_front(&mut self) {
        let mut was = false;
        self.with_window(|w| {
            w.fronted += 1;
            was = std::mem::take(&mut w.minimized);
        });
        if was {
            self.post(Event::Restored);
        }
        self.shared.focus(self.window);
    }

    fn set_resizable(&mut self, resizable: bool) {
        self.with_window(|w| w.resizable = resizable);
    }

    fn set_always_on_top(&mut self, on_top: bool) {
        self.with_window(|w| w.always_on_top = on_top);
    }

    fn set_icon(&mut self, icon: Option<&TrayImage>) {
        self.with_window(|w| w.icon = icon.cloned());
    }

    fn window_position(&self) -> Option<(i32, i32)> {
        self.shared.windows.borrow().get(&self.window).and_then(|w| w.position)
    }

    fn window_placement(&self) -> Option<WindowPlacement> {
        self.shared.windows.borrow().get(&self.window).map(|w| w.placement)
    }

    /// `KOMPOZ_HEADLESS_SCREEN=WxH`, else 1920 by 1080.
    fn screen_size(&self) -> Option<(i32, i32)> {
        let asked = self.option("KOMPOZ_HEADLESS_SCREEN").and_then(|v| {
            let (w, h) = v.trim().split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        });
        Some(asked.unwrap_or((1920, 1080)))
    }

    /// Kept, for tests to read.
    fn tray_supported(&self) -> bool {
        true
    }

    fn sync_trays(&mut self, trays: &[TrayIcon]) {
        *self.shared.trays.borrow_mut() = trays.to_vec();
    }

    fn send_notification(&mut self, tray: &TrayIcon, notification: &Notification) {
        self.shared.notifications.borrow_mut().push((tray.id, notification.clone()));
    }
}

/// Runs the core's loop on the headless platform (or one built on it, posting to `queue`) until the app quits
/// (C++'s `runHeadless`): the events posted handled, the pause until the next frame is due the host's for work
/// between frames, a frame, then (with nothing to animate) a wait for an event, a wake or the next coroutine delay.
/// One core alone (its `Window`s aren't opened: [`run_windows`] opens them).
///
/// On the web, where nothing may block, it doesn't sleep nor wait: a headless run there goes through to its end at
/// once, frame after frame (a script's, a test's), as a batch.
pub fn run_core<P: Platform>(core: &mut AppCore<P>, queue: &EventQueue) {
    loop {
        for (_, e) in queue.take() {
            if e != Event::Wake {
                core.handle(&e); // (a wake only ends the wait)
            }
        }
        pace(core.frame_due(), |budget| core.idle(budget));
        let next = core.frame(None);
        if next.quit {
            break;
        }
        if !next.busy {
            queue.wait(next.wake_at);
        }
    }
    core.quit();
}

/// Until `due` (none: now): the time the cores' for work between frames (`idle`), then slept away.
fn pace(due: Option<i64>, mut idle: impl FnMut(i64)) {
    let Some(due) = due else { return };
    let at = now();
    if at < due {
        idle(due - at);
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        {
            let at = now();
            if at < due {
                std::thread::sleep(Duration::from_nanos((due - at) as u64));
            }
        }
    }
}

/// Runs the app's cores ([`AppWindows`]: its root and its `Window`s') on the headless platform until the app quits:
/// the events posted handed to the cores of their windows, the windows synced with their requests, a frame of each
/// (paced to the earliest due), synced again, then (with nothing to animate) a wait.
pub fn run_windows<P: Platform>(windows: &mut AppWindows<P>, queue: &EventQueue) {
    loop {
        for (id, e) in queue.take() {
            if e != Event::Wake {
                windows.handle(id, &e);
            }
        }
        windows.sync();
        let due = windows.cores_mut().iter().map(|c| c.frame_due()).collect::<Option<Vec<_>>>();
        let due = due.and_then(|d| d.into_iter().min());
        pace(due, |budget| {
            for core in windows.cores_mut() {
                core.idle(budget);
            }
        });
        let (mut busy, mut wake_at, mut quit) = (false, None::<i64>, false);
        // (The root first: a script's "@Title: command" reaches its window in the same round.)
        let next = windows.app_mut().frame(None);
        windows.sync();
        for next in std::iter::once(next).chain(windows.window_cores_mut().map(|c| c.frame(None)).collect::<Vec<_>>()) {
            busy |= next.busy;
            quit |= next.quit;
            wake_at = match (wake_at, next.wake_at) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
        }
        windows.sync();
        if quit || windows.quitting() {
            break;
        }
        if !busy {
            queue.wait(wake_at);
        }
    }
    windows.quit();
}

/// The app's cores on the headless platform: its root on `platform` (its window the main one, or the app's
/// composition's without one), each `Window` on a window of its own sharing it.
pub fn headless_windows(
    platform: HeadlessPlatform,
    config: &AppConfig,
    content: &AppContent,
) -> AppWindows<HeadlessPlatform> {
    let base = platform.for_window(WindowId::MAIN);
    let platform = match content {
        AppContent::Application(_) => platform.for_window(WindowId::APPLICATION),
        AppContent::Window(_) => platform,
    };
    let core = content.core(platform, config);
    AppWindows::new(core, Box::new(move |r: &WindowRequest| Some(base.for_window(r.id()))))
}

/// The app on the headless platform, until it quits: what the program returns.
pub fn run_headless(config: &AppConfig, content: AppContent) -> ExitCode {
    let platform = HeadlessPlatform::new();
    let queue = platform.queue();
    let mut windows = headless_windows(platform, config, &content);
    if !windows.start() {
        return ExitCode::FAILURE;
    }
    run_windows(&mut windows, &queue);
    ExitCode::SUCCESS
}

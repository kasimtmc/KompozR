//! KompozR's platform without a screen (C++'s headless platform): tests, scripted runs and screenshots, CI.
//! `KOMPOZ_HEADLESS=1` runs an app on it: its window a size and nothing more, the GPU renderer drawing only into its
//! kept frame (or, with `KOMPOZ_HEADLESS_SWAPCHAIN=1`, presenting to a Vulkan headless surface, as a window would:
//! the swapchain's path without a display), no input but a script's (`KOMPOZ_SCRIPT`), the loop and its timers its
//! own.
//!
//! - `KOMPOZ_SCALE`: px per dp (a phone's 2.625, say); `KOMPOZ_FONT_SCALE`: the system's text size.

use crate::app::{AppConfig, AppCore, Event, Metrics, Platform};
use crate::system::{log, preferred_language};
use kompoz_gpu::device::{Backend, Surface};
use kompoz_graphics::Rect;
use kompoz_runtime::Content;
use kompoz_ui::KompozHost;
use kompoz_ui::media_query::MediaInfo;
use kompoz_ui::platform::PlatformCallbacks;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// `KOMPOZ_HEADLESS=1`.
pub fn headless_requested() -> bool {
    std::env::var_os("KOMPOZ_HEADLESS").is_some_and(|v| v == "1")
}

/// The headless platform's clock: ns since the process first asked.
fn now() -> i64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_nanos() as i64
}

/// The events posted to the loop, from any thread, and what wakes it.
#[derive(Default)]
pub struct EventQueue {
    events: Mutex<VecDeque<Event>>,
    posted: Condvar,
}

impl EventQueue {
    /// `event` to the core before its next frame, waking the loop.
    pub fn post(&self, event: Event) {
        self.events.lock().unwrap_or_else(|e| e.into_inner()).push_back(event);
        self.posted.notify_one();
    }

    /// What was posted, taken.
    pub fn take(&self) -> VecDeque<Event> {
        std::mem::take(&mut *self.events.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Waits until something is posted, or until `wake_at` (the platform's clock; none: as long as it takes).
    pub fn wait(&self, wake_at: Option<i64>) {
        let events = self.events.lock().unwrap_or_else(|e| e.into_inner());
        let ready = |q: &mut VecDeque<Event>| q.is_empty();
        match wake_at {
            None => drop(self.posted.wait_while(events, ready)),
            Some(at) => {
                let timeout = Duration::from_nanos((at - now()).max(0) as u64);
                drop(self.posted.wait_timeout_while(events, timeout, ready));
            }
        }
    }
}

/// The callbacks the headless platform gives the host: waking its loop, a clipboard of the app's own.
struct HeadlessCallbacks {
    queue: Arc<EventQueue>,
    clipboard: RefCell<Option<String>>,
}

impl PlatformCallbacks for HeadlessCallbacks {
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        let queue = self.queue.clone();
        Some(Arc::new(move || queue.post(Event::Wake)))
    }
    fn get_clipboard(&self) -> Option<String> {
        self.clipboard.borrow().clone()
    }
    fn set_clipboard(&self, text: &str) {
        *self.clipboard.borrow_mut() = Some(text.to_string());
    }
}

/// The headless platform: a window that is its size, shown and focused at once, as one on a screen would be.
pub struct HeadlessPlatform {
    queue: Arc<EventQueue>,
    /// The window's size: its pixels are its points.
    size: Rc<Cell<(i32, i32)>>,
    /// Settings given instead of the environment's (a test's).
    options: HashMap<String, String>,
}

impl HeadlessPlatform {
    pub fn new() -> HeadlessPlatform {
        HeadlessPlatform {
            queue: Arc::new(EventQueue::default()),
            size: Rc::new(Cell::new((1, 1))),
            options: HashMap::new(),
        }
    }

    /// `name` set to `value` instead of the environment's (a test's `KOMPOZ_SCRIPT`).
    pub fn set_option(&mut self, name: &str, value: &str) {
        self.options.insert(name.to_string(), value.to_string());
    }

    /// Its events' queue (a test posts to it as a window system would).
    pub fn queue(&self) -> Arc<EventQueue> {
        self.queue.clone()
    }
}

impl Default for HeadlessPlatform {
    fn default() -> Self {
        HeadlessPlatform::new()
    }
}

impl Platform for HeadlessPlatform {
    fn option(&self, name: &str) -> Option<String> {
        self.options.get(name).cloned().or_else(|| std::env::var(name).ok())
    }

    fn log(&self, message: &str) {
        log(message);
    }

    fn now(&self) -> i64 {
        now()
    }

    fn backends(&self) -> Vec<Backend> {
        if cfg!(target_vendor = "apple") { vec![Backend::Metal] } else { vec![Backend::Vulkan] }
    }

    /// The window is its size (the app's, a dp a pixel, as C++'s); what the device needs: nothing (it draws only
    /// into textures), or a headless surface to present to (`KOMPOZ_HEADLESS_SWAPCHAIN=1`).
    fn open_window(&mut self, app: &AppConfig, backend: Backend) -> Result<Surface, String> {
        if backend != Backend::Vulkan {
            return Err(format!("the headless platform has no {} backend", backend.name()));
        }
        self.size.set((app.width.0.round().max(1.0) as i32, app.height.0.round().max(1.0) as i32));
        let size = self.size.clone();
        let mut surface = Surface { drawable_size: Some(Box::new(move || size.get())), ..Surface::default() };
        if self.option("KOMPOZ_HEADLESS_SWAPCHAIN").is_some_and(|v| v == "1") {
            surface.vulkan = Some(kompoz_gpu::device::vulkan::headless_surface());
        }
        for e in [Event::FocusGained, Event::Shown, Event::Resized] {
            self.queue.post(e);
        }
        Ok(surface)
    }

    fn close_window(&mut self) {
        self.queue.take();
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
        self.size.set((width.max(1), height.max(1)));
        self.queue.post(Event::Resized);
    }

    /// No pointer, keyboard, microphone or camera.
    fn media(&mut self, _: bool) -> MediaInfo {
        MediaInfo::default()
    }

    fn attach(&mut self, _: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        Rc::new(HeadlessCallbacks { queue: self.queue.clone(), clipboard: RefCell::new(None) })
    }

    fn composed(&mut self, _: &KompozHost, _: bool, _: i64) {}
}

/// Runs the core's loop on the headless platform (or one built on it, posting to `queue`) until the app quits
/// (C++'s `runHeadless`): the events posted handled, the pause until the next frame is due the host's for work
/// between frames, a frame, then (with nothing to animate) a wait for an event, a wake or the next coroutine delay.
pub fn run_core<P: Platform>(core: &mut AppCore<P>, queue: &EventQueue) {
    loop {
        for e in queue.take() {
            if e != Event::Wake {
                core.handle(&e); // (a wake only ends the wait)
            }
        }
        if let Some(due) = core.frame_due() {
            let at = now();
            if at < due {
                core.idle(due - at);
                let at = now();
                if at < due {
                    std::thread::sleep(Duration::from_nanos((due - at) as u64));
                }
            }
        }
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

/// The app on the headless platform, until it quits: what the program returns.
pub fn run_headless(config: &AppConfig, content: Rc<dyn Content>) -> ExitCode {
    let mut core = AppCore::new(HeadlessPlatform::new(), config.clone(), content);
    if !core.start() {
        return ExitCode::FAILURE;
    }
    let queue = core.platform().queue();
    run_core(&mut core, &queue);
    ExitCode::SUCCESS
}

//! KompozR's platform on macOS (C++'s macos_platform.mm), on AppKit through objc2: its windows NSWindows with a view
//! of our own each (`view`: backed by the CAMetalLayer the Metal device presents to, flipped as the app's points are,
//! the window's delegate too), at each display's backing scale (`KOMPOZ_SCALE` instead), their frames on their
//! display's beat (`display_link`: the view's CADisplayLink on macOS 14, Core Video's before), drawn while AppKit
//! tracks a resize, a menu or a drag in a loop of its own (a timer only in that loop's mode steps the app); the
//! mouse, the trackpad's precise scrolling (its momentum too), pinches and turns, a tablet's pen with its pressure,
//! its eraser and its buttons; the keyboard as Compose's keys (`keyboard`: Carbon's key codes, left and right,
//! Command as Meta) and the input methods through NSTextInputClient in full (`text_input`: their marked text as
//! the field's composition, the field's text, selection and composition as they ask, the replacement ranges of the
//! press-and-hold menu and reconversions; the candidates beside the field); the cursors; the pasteboard both ways,
//! lazily (`clipboard`, `types`: text, HTML, PNG and TIFF, files, the app's own types, sensitive entries concealed);
//! drops from other apps and the app's own drags out to them; the menu bar (`app`: the app menu and the Window menu
//! around the key window's own menus, `Platform::set_menu_bar`'s native menus); the theme, the accent color (the
//! dynamic palette's seed) and reduced motion (the hosts' motion duration scale), followed as they change; the
//! languages and the time zone Foundation says (for the runtime's locale and zones: an app started from Finder has
//! no `LANG`); links opened by the workspace; the display kept awake (IOKit); the media devices looked for; errors
//! shown in an alert; the app bundle's resources as the app's assets; its log lines to the unified log and to its
//! standard error.
//!
//! All the app's windows share the main thread's loop (`Loop`, C++'s `run` and `step`): AppKit's events handed to
//! it as they come (each window's become its events), a step of the app's cores, then a wait for the next event, a
//! wake (the hosts' wakers, a display link's tick, a signal: Grand Central Dispatch's main queue posts an
//! application-defined event), or what is due.
//!
//! What comes in later packages: NSAccessibility (MA), the tray and notifications (NSStatusItem,
//! UNUserNotificationCenter).

mod app;
mod clipboard;
mod display_link;
mod keyboard;
mod services;
mod sys;
mod text_input;
mod types;
mod view;
mod window;

pub use window::MacWindow;

use self::app::Shared;
use self::window::WindowState;
use crate::app::{AppConfig, AppContent, AppWindows, DisplayPlatform, Event};
use crate::system::{log, set_asset_reader, set_log_sink};
use kompoz_ui::window::{WindowId, WindowRequest};
use objc2::MainThreadMarker;
use objc2::rc::autoreleasepool;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

/// macOS among the display platforms ([`DISPLAY_PLATFORMS`](crate::app::DISPLAY_PLATFORMS)).
pub const PLATFORM: DisplayPlatform = DisplayPlatform { name: "macos", run };

/// The platform's clock, ns: the time since the system started, not counting its sleep (`CLOCK_UPTIME_RAW`, what
/// AppKit's events' timestamps are on).
pub fn now() -> i64 {
    sys::uptime_ns()
}

/// A log line to standard error (a terminal's, CI's) and to the unified log (Console).
fn log_line(message: &str) {
    eprintln!("{message}");
    sys::os_log(message);
}

/// The app on macOS until it quits (what the program returns), or none where it can't run (not on the main
/// thread, which AppKit needs). Its windows share the main thread's loop: the root core's (the main window's, or the
/// app's composition's without one) and each `Window`'s.
fn run(config: &AppConfig, content: AppContent) -> Option<ExitCode> {
    let Some(mtm) = MainThreadMarker::new() else {
        log("kompoz: macOS: the app runs on the main thread (AppKit's)");
        return None;
    };
    set_log_sink(Some(log_line));
    services::publish_locale();
    if let Some(resources) = services::bundle_resources() {
        set_asset_reader(Some(Box::new(move |path: &str| std::fs::read(resources.join(path)).ok())));
    }
    let app_id = match content {
        AppContent::Application(_) => WindowId::APPLICATION,
        AppContent::Window(_) => WindowId::MAIN,
    };
    let shared = Shared::connect(mtm, app_id, &config.title);
    let root = content.core(MacWindow::new(&shared, app_id), config);
    let make = {
        let shared = shared.clone();
        Box::new(move |r: &WindowRequest| Some(MacWindow::new(&shared, r.id())))
    };
    let looping = Rc::new(Loop {
        windows: RefCell::new(AppWindows::new(root, make)),
        need_frame: RefCell::new(HashMap::new()),
        busy: RefCell::new(HashMap::new()),
        deadline: Cell::new(None),
        done: Cell::new(false),
        ended: Cell::new(false),
    });
    if !looping.windows.borrow_mut().start() {
        shared.close();
        return Some(ExitCode::FAILURE);
    }
    // (AppKit's own loops — a window resized, a menu open, a drag — step the app from a timer of theirs; a drop
    // asks for the app's answer at once.)
    let stepper: Rc<dyn Fn(StepKind)> = {
        let (looping, shared) = (Rc::downgrade(&looping), Rc::downgrade(&shared));
        Rc::new(move |kind| {
            let (Some(looping), Some(shared)) = (looping.upgrade(), shared.upgrade()) else { return };
            match kind {
                StepKind::Frame => {
                    let _ = looping.step(&shared);
                }
            }
        })
    };
    shared.set_stepper(Some(stepper));
    looping.run(&shared);
    shared.set_stepper(None);
    looping.end();
    drop(looping); // (the windows and their devices before the platform)
    shared.close();
    Some(ExitCode::SUCCESS)
}

/// What the platform's views ask of the loop from inside AppKit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StepKind {
    /// What came handed to the cores and the frames that are ready drawn (a tracking loop's timer, a drop).
    Frame,
}

/// What one step of the loop says.
enum Step {
    /// The app is done.
    Quit,
    /// Nothing until an event, a wake or a tick, or this long (none: the cores' deadline alone).
    Wait(Option<Duration>),
    /// Frames were drawn: on at once.
    Again,
}

/// The loop's state (C++'s `run` and `step`): the app's cores, which windows want a frame, which are busy, when the
/// cores next want to be woken.
struct Loop {
    windows: RefCell<AppWindows<MacWindow>>,
    need_frame: RefCell<HashMap<WindowId, bool>>,
    busy: RefCell<HashMap<WindowId, bool>>,
    /// The cores' deadline (the earliest coroutine delay), on the platform's clock.
    deadline: Cell<Option<i64>>,
    /// The loop is to stop.
    done: Cell<bool>,
    /// The app's end done (its state written).
    ended: Cell<bool>,
}

impl Loop {
    /// Until the app is done: AppKit's events handed to it, then a step, then the wait it asks for (each round in
    /// an autorelease pool of its own).
    fn run(&self, shared: &Shared) {
        loop {
            let step = autoreleasepool(|_| {
                shared.drain();
                if self.done.get() {
                    return Step::Quit;
                }
                self.step(shared)
            });
            match step {
                Step::Quit => break,
                Step::Again => continue,
                Step::Wait(timeout) => autoreleasepool(|_| {
                    let timeout = self.until_deadline(timeout);
                    shared.wait(timeout);
                    if self.deadline.get().is_some_and(|d| d <= now()) {
                        self.deadline.set(None);
                        shared.push(Route::All, Event::Wake);
                    }
                }),
            }
        }
    }

    /// The wait's limit: the step's, and the cores' deadline.
    fn until_deadline(&self, timeout: Option<Duration>) -> Option<Duration> {
        let deadline = self.deadline.get().map(|d| Duration::from_nanos((d - now()).max(0) as u64));
        match (timeout, deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// The events gathered handed to the cores of their windows (the field the input methods see updated after
    /// typing), the windows synced with their requests, then a frame of each window that wants one and whose
    /// display ticked — the pause until the earliest is due the hosts' for work between frames — (C++'s `step`,
    /// Wayland's loop). Re-entered from AppKit while a step runs: nothing (the step going on finishes it).
    fn step(&self, shared: &Shared) -> Step {
        if self.done.get() {
            return Step::Quit;
        }
        let Ok(mut windows) = self.windows.try_borrow_mut() else { return Step::Again };
        let mut need_frame = self.need_frame.borrow_mut();
        let mut busy = self.busy.borrow_mut();
        let mut quit = false;
        let mut typed: Vec<WindowId> = Vec::new();
        for (id, e) in shared.take_events() {
            quit |= e == Event::Quit;
            if matches!(e, Event::Text(_) | Event::Preedit { .. } | Event::DeleteSurroundingText { .. })
                && !typed.contains(&id)
            {
                typed.push(id);
            }
            windows.handle(id, &e);
            need_frame.insert(id, true);
        }
        // (The field as the core left it after the typing: what the input methods see next.)
        for id in typed {
            let state = windows.core(id).and_then(|c| c.host()).and_then(|h| h.text_input_service().session_state());
            if let (Some(state), Some(w)) = (state, shared.window(id)) {
                w.set_ime_state(state);
            }
        }
        if quit {
            self.done.set(true);
            return Step::Quit;
        }
        windows.sync();
        let mut drew = false;
        let mut waiting: Option<i64> = None;
        let mut due_at: Option<i64> = None;
        let mut wake_at: Option<i64> = None;
        for id in windows.ids() {
            let scripted = windows.core(id).is_some_and(|c| c.scripted());
            let wanted = *need_frame.get(&id).unwrap_or(&true) || *busy.get(&id).unwrap_or(&false) || scripted;
            let state = shared.window(id);
            if !wanted {
                if let Some(s) = &state {
                    s.frames_unwanted();
                }
                continue;
            }
            // (On the display's beat: a window shown waits for its display link's tick.)
            if let Some(s) = &state
                && !s.frame_ready()
            {
                waiting = earliest(waiting, s.stall_ends());
                continue;
            }
            let Some(core) = windows.core_mut(id) else { continue };
            if let Some(due) = core.frame_due()
                && now() < due
            {
                due_at = earliest(due_at, Some(due));
                continue;
            }
            need_frame.insert(id, false);
            let next = core.frame(None);
            drew = true;
            quit |= next.quit;
            busy.insert(id, next.busy);
            if !next.busy {
                wake_at = earliest(wake_at, next.wake_at);
            }
        }
        windows.sync();
        if quit || windows.quitting() {
            self.done.set(true);
            return Step::Quit;
        }
        if drew {
            let any_busy = busy.values().any(|b| *b);
            self.deadline.set(if any_busy { None } else { wake_at });
            return Step::Again;
        }
        // Nothing drawn: the hosts' work between frames until the earliest is due, then a wait.
        let until = earliest(due_at, waiting);
        if let Some(due) = due_at {
            let at = now();
            if at < due {
                for core in windows.cores_mut() {
                    core.idle(due - at);
                }
            }
        }
        if due_at.is_none() && waiting.is_none() && windows.quitting() {
            self.done.set(true);
            return Step::Quit;
        }
        Step::Wait(until.map(|t| Duration::from_nanos((t - now()).max(0) as u64)))
    }

    /// The app ended (C++'s `quit`): its windows closed, its state written. Once.
    fn end(&self) {
        self.done.set(true);
        if !self.ended.get()
            && let Ok(mut windows) = self.windows.try_borrow_mut()
        {
            self.ended.set(true);
            windows.quit();
        }
    }
}

/// The earlier of two times, either of them where the other is none.
fn earliest(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// The events the windows' views gathered and those of the platform's own (`routed`), for the loop, by window (the
/// root's: `app`).
fn route(windows: &[Rc<WindowState>], routed: Vec<(Route, Event)>, app: WindowId) -> Vec<(WindowId, Event)> {
    let mut out = Vec::new();
    for (route, e) in routed {
        match route {
            Route::App => out.push((app, e)),
            Route::All => out.extend(windows.iter().map(|w| (w.id, e.clone()))),
        }
    }
    for w in windows {
        out.extend(w.take_events().into_iter().map(|e| (w.id, e)));
    }
    out
}

/// Where an event of the platform's own goes: the app's root core, every window's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Route {
    App,
    All,
}

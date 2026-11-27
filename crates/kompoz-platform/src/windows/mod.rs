//! KompozR's platform on Windows (C++'s win32_platform): its windows Win32 windows with the system's frame (DWM's
//! caption in the app's dark or light theme, Mica behind it where Windows 11 has it, round corners), drawn into
//! with Vulkan (`VK_KHR_win32_surface`; lavapipe beside the program where there is no Vulkan device: kompoz-gpu's
//! software fallback) at each monitor's DPI (per-monitor v2: a window moved to another monitor resized to it, its
//! scale with it; `KOMPOZ_SCALE` instead), its frames on the display's beat (the present waits for it; the refresh
//! rate from the monitor's display settings, read again as the window moves and the displays change), drawn while
//! Windows resizes or moves it in its own loop (a timer of the window's steps the app meanwhile); the mouse with its
//! capture, its leaving and a precision touchpad's fractional wheel; the keyboard as Compose's keys (`keyboard`:
//! scan codes, left and right, AltGr) and what it types (WM_CHAR, its surrogate pairs and dead keys); the cursors;
//! the clipboard both ways, lazily (`clipboard`: text, HTML, PNG and bitmaps, files, the app's own types);
//! links opened by the shell; the display kept on; the media devices looked for; errors shown in a message box.
//!
//! All the app's windows share the UI thread's message loop (the main one and each `Window`'s: [`Win32Window`],
//! each a Win32 window and a Vulkan device of its own): one wait (`event_loop`: the thread's messages, the hosts'
//! wake event, a high-resolution timer for what is due), the events of each window handed to its core. A hidden
//! window of the platform's own owns the clipboard and hears the system's broadcasts (the theme, the devices, the
//! session ending), whatever windows the app has.
//!
//! The process: per-monitor DPI aware (also from the program's manifest: [`build`](crate::build)), its log lines to
//! its standard error (a GUI program's to the console it was started from, `AttachConsole`) and to the debugger
//! (`OutputDebugStringW`), Ctrl+C in that console a quit (the app writes its state on the way out), and the session
//! ending too. Images from http URLs come through WinHTTP (`http`).
//!
//! The input methods (`input_method`): TSF's text services through a text store of the focused field's where TSF
//! can be activated, IMM32's input methods otherwise (`KOMPOZ_WIN_IME=imm32`: always) — their composition drawn in
//! the field, their requests (reconversion, the text around, where characters are) answered, the field's input
//! scopes told.
//!
//! Screen readers and other UI Automation clients (Narrator, NVDA, the touch keyboard) read each window's semantics
//! tree through its providers (`uia_bridge`, `uia`'s model: control types, properties, patterns — a field's text
//! and its ranges too —, the tree's changes as events), at the AT-SPI bridge's parity; `KOMPOZ_UIA=0` turns them
//! off.
//!
//! Drags in and out through OLE (`ole`, its formats the clipboard's: `formats`): drop targets answered by the app,
//! read lazily; the app's drags out by DoDragDrop with their image.
//!
//! Pens and touch screens through WM_POINTER (`pointer`): fingers by their ids, each frame of theirs one move of all
//! with the positions they passed through, cancels; a pen's pressure, eraser, barrel button and hovering, its hover
//! icon; the mouse messages Windows makes of them left out. A precision touchpad's pinches and pans through
//! DirectManipulation (`manipulation`: Compose's trackpad pinch and pan events; the mouse's wheel and Ctrl+wheel as
//! they were).
//!
//! What comes in later packages: the desktop's accent, wallpaper and text size, KompozR's
//! own title bar (`KOMPOZ_DECORATIONS=kompoz`: WM_NCCALCSIZE and WM_NCHITTEST, where `window` leaves the frame to
//! the system), the tray.

mod clipboard;
mod event_loop;
mod formats;
pub mod http;
mod ime;
mod imm;
mod input_method;
mod keyboard;
mod manipulation;
mod ole;
mod pointer;
pub mod sys;
mod tsf;
mod uia;
mod uia_bridge;
mod vulkan;
mod window;

pub use ole::drop_target;
pub use uia_bridge::root_provider as uia_root_provider;
pub use window::Win32Window;

use self::event_loop::EventLoop;
use self::window::{Shared, WindowState};
use crate::app::{AppConfig, AppContent, AppWindows, DisplayPlatform, Event};
use crate::system::{log, set_log_sink};
use kompoz_ui::window::{WindowId, WindowRequest};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Windows among the display platforms ([`DISPLAY_PLATFORMS`](crate::app::DISPLAY_PLATFORMS)).
pub const PLATFORM: DisplayPlatform = DisplayPlatform { name: "windows", run };

/// The platform's clock: ns since the process first asked (Windows' performance counter, as std's `Instant`).
pub fn now() -> i64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_nanos() as i64
}

/// A log line to standard error (where there is one) and to the debugger.
fn log_line(message: &str) {
    eprintln!("{message}");
    sys::debug_output(message);
}

/// The app on Windows until it quits (what the program returns), or none where it can't run (C++'s `runWin32`): no
/// window class, no event, no timer. Its windows share the thread's loop: the root core's (the main window's, or the
/// app's composition's without one) and each `Window`'s.
fn run(config: &AppConfig, content: AppContent) -> Option<ExitCode> {
    sys::attach_parent_console();
    set_log_sink(Some(log_line));
    let app_id = match content {
        AppContent::Application(_) => WindowId::APPLICATION,
        AppContent::Window(_) => WindowId::MAIN,
    };
    let shared = match Shared::connect(app_id) {
        Ok(shared) => shared,
        Err(why) => {
            log(&format!("kompoz: Windows: {why}"));
            return None;
        }
    };
    let root = content.core(Win32Window::new(&shared, app_id), config);
    let make = {
        let shared = shared.clone();
        Box::new(move |r: &WindowRequest| Some(Win32Window::new(&shared, r.id())))
    };
    let looping = Rc::new(Loop {
        windows: RefCell::new(AppWindows::new(root, make)),
        need_frame: RefCell::new(HashMap::new()),
        busy: RefCell::new(HashMap::new()),
        done: Cell::new(false),
        ended: Cell::new(false),
    });
    if !looping.windows.borrow_mut().start() {
        shared.close();
        return Some(ExitCode::FAILURE);
    }
    // (Windows' own loops — a window resized or moved, its system menu — step the app from a timer of theirs; the
    // session ending ends it at once: Windows ends the process after WM_ENDSESSION.)
    let stepper: Rc<dyn Fn(StepKind)> = {
        let (looping, shared) = (Rc::downgrade(&looping), Rc::downgrade(&shared));
        Rc::new(move |kind| {
            let (Some(looping), Some(shared)) = (looping.upgrade(), shared.upgrade()) else { return };
            match kind {
                StepKind::Frame => {
                    let _ = looping.step(&shared);
                }
                StepKind::End => looping.end(),
            }
        })
    };
    shared.set_stepper(Some(stepper));
    looping.run(&shared);
    shared.set_stepper(None);
    looping.end();
    drop(looping); // (the windows and their devices before the platform's own window)
    shared.close();
    Some(ExitCode::SUCCESS)
}

/// What the platform's windows ask of the loop from inside Windows' own loops.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StepKind {
    /// What came handed to the cores and the frames that are due drawn (a modal loop's timer, a resize).
    Frame,
    /// The session ends: the app's state written, its windows closed, now.
    End,
}

/// What one step of the loop says.
#[derive(PartialEq)]
enum Step {
    /// The app is done.
    Quit,
    /// Nothing until a message, a wake, or this long (none: the cores' deadline alone).
    Wait(Option<Duration>),
    /// Frames were drawn: on at once.
    Again,
}

/// The loop's state (C++'s `run` and `step`): the app's cores, which windows want a frame, which are busy.
struct Loop {
    windows: RefCell<AppWindows<Win32Window>>,
    need_frame: RefCell<HashMap<WindowId, bool>>,
    busy: RefCell<HashMap<WindowId, bool>>,
    /// The loop is to stop.
    done: Cell<bool>,
    /// The app's end done (its state written).
    ended: Cell<bool>,
}

impl Loop {
    /// Until the app is done: the thread's messages dispatched (each window's become its events), then a step, then
    /// the wait it asks for.
    fn run(&self, shared: &Shared) {
        loop {
            while let Some(message) = sys::peek_message() {
                if message.id() == ::windows::Win32::UI::WindowsAndMessaging::WM_QUIT {
                    self.done.set(true);
                }
                // (A key a text service eats is its own: not translated, not the window's.)
                if tsf::keystroke(&message) {
                    continue;
                }
                sys::dispatch(&message);
            }
            sys::resume_panic();
            if self.done.get() {
                break;
            }
            let step = self.step(shared);
            // (A drag the step handed to OLE runs now, out of it: its modal loop steps the app.)
            if step != Step::Quit && shared.run_drag() {
                sys::resume_panic();
                continue;
            }
            match step {
                Step::Quit => break,
                Step::Again => continue,
                Step::Wait(timeout) => shared.wait(timeout),
            }
        }
    }

    /// The events gathered handed to the cores of their windows, the windows synced with their requests, then a
    /// frame of each window that wants one — the pause until the earliest is due the hosts' for work between
    /// frames, and what comes meanwhile handled before it — (C++'s `step`, Linux's loops). Re-entered from a modal
    /// loop while a step runs: nothing (the step going on finishes it).
    fn step(&self, shared: &Shared) -> Step {
        if self.done.get() {
            return Step::Quit;
        }
        let Ok(mut windows) = self.windows.try_borrow_mut() else { return Step::Again };
        let mut need_frame = self.need_frame.borrow_mut();
        let mut busy = self.busy.borrow_mut();
        let mut quit = false;
        let mut typed = Vec::new();
        for (id, e) in shared.take_events() {
            quit |= e == Event::Quit;
            if matches!(
                e,
                Event::Text(_)
                    | Event::Preedit { .. }
                    | Event::DeleteSurroundingText { .. }
                    | Event::SetSelection { .. }
            ) && !typed.contains(&id)
            {
                typed.push(id);
            }
            windows.handle(id, &e);
            need_frame.insert(id, true);
        }
        for id in typed {
            // (The input method told the field as the typing left it: what the core took of it.)
            let state = windows.core(id).and_then(|c| c.host()).and_then(|h| h.text_input_service().session_state());
            shared.answer_input_method(id, state);
        }
        if quit {
            self.done.set(true);
            return Step::Quit;
        }
        windows.sync();
        let wanted: Vec<WindowId> = windows
            .ids()
            .into_iter()
            .filter(|id| {
                *need_frame.get(id).unwrap_or(&true)
                    || *busy.get(id).unwrap_or(&false)
                    || windows.core(*id).is_some_and(|c| c.scripted())
            })
            .collect();
        if wanted.is_empty() {
            if windows.quitting() {
                self.done.set(true);
                return Step::Quit;
            }
            return Step::Wait(None);
        }
        let due = wanted.iter().filter_map(|&id| windows.core(id).and_then(|c| c.frame_due())).min();
        if let Some(due) = due {
            let at = now();
            if at < due {
                for core in windows.cores_mut() {
                    core.idle(due - at);
                }
                let at = now();
                if at < due {
                    return Step::Wait(Some(Duration::from_nanos((due - at) as u64)));
                }
            }
        }
        let mut wake_at: Option<i64> = None;
        for id in wanted {
            need_frame.insert(id, false);
            let Some(core) = windows.core_mut(id) else { continue };
            let next = core.frame(None);
            quit |= next.quit;
            busy.insert(id, next.busy);
            if !next.busy {
                wake_at = match (wake_at, next.wake_at) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
            }
        }
        windows.sync();
        if quit || windows.quitting() {
            self.done.set(true);
            return Step::Quit;
        }
        let any_busy = busy.values().any(|b| *b);
        shared.event_loop().set_deadline(if any_busy { None } else { wake_at });
        Step::Again
    }

    /// The app ended (C++'s `quit`): its windows closed, its tray icons gone, its state written. Once.
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

/// The events the windows' procedures gathered and those of the platform's own (`routed`), for the loop, by window
/// (the root's: `app`).
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

impl Shared {
    /// Up to `timeout` for a message, a wake or the cores' deadline (C++'s `MsgWaitForMultipleObjectsEx`): a wake
    /// (the hosts', the deadline) to every window; Ctrl+C a quit.
    fn wait(&self, timeout: Option<Duration>) {
        let ready = self.event_loop().wait(timeout, now);
        if ready.woken || ready.deadline {
            self.push(Route::All, Event::Wake);
        }
        if self.event_loop().take_quit_signal() {
            self.push(Route::App, Event::Quit);
        }
    }

    fn event_loop(&self) -> &EventLoop {
        &self.event_loop
    }
}

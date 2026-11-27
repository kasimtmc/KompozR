//! The web platform's loop (the module's docs say how it goes): the app's cores in a thread local, the page's
//! turns, records and calls handed to them.

use super::events::{self, DragPhase, Pointers, Record};
use super::window::{Shared, WebWindow};
use super::{clipboard, files, images, keys, sys, text};
use crate::app::{AppConfig, AppContent, AppWindows, DisplayPlatform, Event};
use crate::system::{set_asset_reader, set_log_sink};
use kompoz_ui::input::Key;
use kompoz_ui::window::{WindowId, WindowRequest};
use std::cell::RefCell;
use std::process::ExitCode;
use std::rc::Rc;

/// The web among the display platforms ([`DISPLAY_PLATFORMS`](crate::app::DISPLAY_PLATFORMS)).
pub const PLATFORM: DisplayPlatform = DisplayPlatform { name: "web", run };

/// Where the app's state is kept across reloads (the tab's sessionStorage, the page's path added by web.js).
pub(crate) const STATE_KEY: &str = "state";

/// The canvas the app draws into: the page's element of this id, else one made to fill the page.
const CANVAS_ID: &str = "kompoz";

thread_local! {
    static LOOP: RefCell<Option<Loop>> = const { RefCell::new(None) };
}

/// The app's loop on the page: its cores, what the page tells them through.
struct Loop {
    windows: AppWindows<WebWindow>,
    shared: Rc<Shared>,
    pointers: Pointers,
    /// The cores want a frame (events came, or the wake came).
    need_frame: bool,
    /// A core's last frame asked for the next at once (it animates, recomposes, runs a script).
    busy: bool,
    /// The app is done; it ends once what it still reads back (a script's last screenshots) is written.
    quitting: bool,
}

/// A log line to the console.
fn log_line(message: &str) {
    kompoz_web::log(kompoz_web::Level::Info, message);
}

/// The app on the web: started in the page's canvas, its loop the page's from now on; `main` returns at once (what
/// it returns says whether the app started). None where there is no page (a module run without one).
fn run(config: &AppConfig, content: AppContent) -> Option<ExitCode> {
    kompoz_web::start();
    set_log_sink(Some(log_line));
    set_asset_reader(Some(Box::new(files::read)));
    text::start();
    images::start();
    if !sys::start(CANVAS_ID) {
        return None;
    }
    let canvas = kompoz_web::page_object("kompoz.canvas");
    let gpu = kompoz_web::page_object("gpu.device");
    let shared = Shared::new(canvas, gpu);
    let app_id = match content {
        AppContent::Application(_) => WindowId::APPLICATION,
        AppContent::Window(_) => WindowId::MAIN,
    };
    let root = content.core(WebWindow::new(&shared, app_id), config);
    // (The page has one canvas: the app's first `Window` draws into it if the app has no window of its own; any
    // other is composed without one.)
    let make = {
        let shared = shared.clone();
        Box::new(move |r: &WindowRequest| shared.owner.get().is_none().then(|| WebWindow::new(&shared, r.id())))
    };
    let mut windows = AppWindows::new(root, make);
    if !windows.start() {
        return Some(ExitCode::FAILURE);
    }
    LOOP.with(|l| {
        *l.borrow_mut() = Some(Loop {
            windows,
            shared,
            pointers: Pointers::default(),
            need_frame: true,
            busy: false,
            quitting: false,
        })
    });
    sys::request_frame();
    Some(ExitCode::SUCCESS)
}

/// Runs `f` on the loop, if there is one and it isn't already running (the page calls in only between its own
/// work: never while Rust runs, which web.js sees to).
fn with_loop<T>(f: impl FnOnce(&mut Loop) -> T) -> Option<T> {
    LOOP.with(|l| l.try_borrow_mut().ok()?.as_mut().map(f))
}

/// A turn of the loop (web.js's animation frame, at `time` ms on the page's clock, with `frame`; or a timeout, a
/// wake, work settled, without): the page's events handed to the cores, then, at an animation frame, a frame of each
/// core that wants one; the next turn asked for.
pub(crate) fn turn(frame: bool, time: f64) {
    let ended = with_loop(|l| l.turn(frame, time)).unwrap_or(false);
    if ended {
        // (The cores and their devices go after the loop's borrow ended: what they drop may log.)
        let gone = LOOP.with(|l| l.borrow_mut().take());
        drop(gone);
        sys::ended(0);
    }
}

/// A copy (`cut`: a cut) event being handled: the events gathered handed to the cores now (Ctrl+C's key, whose
/// copy the app puts on the clipboard: into the event).
pub(crate) fn clipboard_event(cut: bool) {
    let _ = cut; // (the key that made it is among the events: the app copies or cuts as its key says)
    with_loop(|l| {
        l.shared.in_clip_event.set(true);
        l.handle_records();
        l.shared.in_clip_event.set(false);
    });
}

/// The page goes away (pagehide): the app's state kept for the tab's next load.
pub(crate) fn save() {
    with_loop(|l| l.save());
}

/// The browser begins a drag of the press at `x`, `y` (`dragstart`): whether the app hands its drag within it to
/// the page (one under way: a long press's).
pub(crate) fn drag_start(x: f32, y: f32) -> bool {
    with_loop(|l| l.drag_start(x, y)).unwrap_or(false)
}

impl Loop {
    /// The window drawing into the canvas: the page's events are its.
    fn target(&self) -> WindowId {
        self.shared.owner.get().unwrap_or_else(|| self.windows.app_id())
    }

    fn handle(&mut self, event: Event) {
        let id = self.target();
        self.windows.handle(id, &event);
        self.need_frame = true;
    }

    fn turn(&mut self, frame: bool, time: f64) -> bool {
        #[cfg(target_family = "wasm")]
        kompoz_gpu::device::webgpu::finish_reads();
        images::finish();
        for core in self.windows.cores_mut() {
            core.finish_writes();
        }
        self.handle_records();
        if self.quitting {
            return self.try_end();
        }
        self.windows.sync();
        let mut next_wake: Option<i64> = None;
        let mut busy = false;
        if frame {
            let display_time = events::nanos(time);
            let wanted: Vec<WindowId> = self.windows.ids();
            let any_scripted = wanted.iter().any(|id| self.windows.core(*id).is_some_and(|c| c.scripted()));
            if self.need_frame || self.busy || any_scripted {
                self.need_frame = false;
                for id in wanted {
                    let Some(core) = self.windows.core_mut(id) else { continue };
                    let next = core.frame((display_time >= 0).then_some(display_time));
                    self.quitting |= next.quit;
                    busy |= next.busy;
                    if !next.busy {
                        next_wake = match (next_wake, next.wake_at) {
                            (Some(a), Some(b)) => Some(a.min(b)),
                            (a, b) => a.or(b),
                        };
                    }
                }
                self.windows.sync();
            }
        } else {
            self.need_frame = true;
        }
        if frame {
            self.busy = busy;
        }
        self.quitting |= self.windows.quitting();
        if self.quitting {
            return self.try_end();
        }
        if self.busy || self.need_frame {
            sys::request_frame();
        } else if let Some(at) = next_wake {
            let ms = (at - kompoz_web::now_nanos()) as f64 / 1e6;
            sys::wake_in(ms.max(0.0));
        }
        false
    }

    /// The app done: ended once its cores have written what they read back (its screenshots: each a turn later as
    /// the GPU hands them over); else the next turn asked for.
    fn try_end(&mut self) -> bool {
        if self.windows.cores_mut().iter().any(|c| c.writing()) {
            sys::wake_in(4.0);
            return false;
        }
        self.windows.quit();
        // (An app that ended itself starts afresh at the tab's next load.)
        sys::storage_set(STATE_KEY, &[]);
        true
    }

    /// The app's state, for the tab's next load.
    fn save(&mut self) {
        if self.quitting {
            return;
        }
        if let Some(host) = self.windows.app().host() {
            sys::storage_set(STATE_KEY, &host.save_state());
        }
    }

    /// The page's records since the last turn, handed to the cores.
    fn handle_records(&mut self) {
        let records = sys::events();
        for line in records.lines() {
            match events::parse(line) {
                Some(record) => self.record(record),
                None if !line.is_empty() => log_line(&format!("kompoz: web: a record not understood: {line}")),
                None => {}
            }
        }
    }

    fn record(&mut self, record: Record) {
        match record {
            Record::Pointer(p) => {
                for e in self.pointers.events(&p) {
                    self.handle(e);
                }
            }
            Record::Wheel { x, y, dx, dy, mode, modifiers, time } => {
                let (dx, dy) = events::wheel_notches(dx, dy, mode);
                self.handle(Event::Wheel { x, y, dx, dy, modifiers, time: events::nanos(time) });
            }
            Record::Key { down, code, key, modifiers, time } => {
                let k = keys::key_of(&code, &key);
                if k == Key::Unknown {
                    return;
                }
                let code_point = keys::code_point(&key);
                let time = events::nanos(time);
                self.handle(if down {
                    Event::KeyDown { key: k, code_point, modifiers, time }
                } else {
                    Event::KeyUp { key: k, code_point, modifiers, time }
                });
            }
            Record::Input { value, start, end } => {
                let events = self.shared.text_area.borrow_mut().input(&value, start, end);
                for e in events {
                    self.handle(e);
                }
            }
            Record::CompositionStart => self.shared.text_area.borrow_mut().composition_start(),
            Record::CompositionUpdate { data, value, start, end } => {
                let events = self.shared.text_area.borrow_mut().composition_update(&data, &value, start, end);
                for e in events {
                    self.handle(e);
                }
            }
            Record::CompositionEnd { data, value, start, end } => {
                let (events, set) = self.shared.text_area.borrow_mut().composition_end(&data, &value, start, end);
                if let Some(c) = set {
                    sys::text_set(&c.text, c.start, c.end);
                }
                for e in events {
                    self.handle(e);
                }
            }
            Record::Paste { transfer } => {
                if let Some(entry) = clipboard::read_transfer(transfer, "paste") {
                    *self.shared.clip.borrow_mut() = Some(entry);
                }
                kompoz_web::release(transfer);
            }
            Record::Drag { phase, x, y, transfer } => self.drag(phase, x, y, transfer),
            Record::DragEnd { action } => {
                if self.shared.dragging_out.replace(false) {
                    self.handle(Event::OwnDragEnded { action });
                }
            }
            Record::Page(what) => {
                for e in self.shared.read_page() {
                    // (The devices are every core's; the rest the canvas's window's.)
                    if e == Event::DevicesChanged {
                        for id in self.windows.ids() {
                            self.windows.handle(id, &e);
                        }
                        self.need_frame = true;
                    } else {
                        self.handle(e);
                    }
                }
                if what == "hidden" {
                    self.save();
                }
                let hash = self.shared.page.borrow().hash.clone();
                if what == "hash" && *self.shared.location.borrow() != hash {
                    *self.shared.location.borrow_mut() = hash.clone();
                    let id = self.target();
                    if let Some(host) = self.windows.core(id).and_then(|c| c.host()) {
                        host.open_deep_link(&hash);
                    }
                    self.need_frame = true;
                }
            }
            Record::Leaving => self.save(),
        }
    }

    /// A drag over the canvas: one from outside (its types told as it enters, what it brings at its drop), or the
    /// app's own back from the page.
    fn drag(&mut self, phase: DragPhase, x: f32, y: f32, transfer: u32) {
        let own = self.shared.dragging_out.get();
        match phase {
            DragPhase::Enter => {
                let mime_types = if own {
                    Vec::new()
                } else {
                    clipboard::items_of(&sys::transfer_items(transfer))
                        .into_iter()
                        .map(|i| if i.file { kompoz_ui::clipboard::MIMETYPE_TEXT_URILIST.to_string() } else { i.mime })
                        .fold(Vec::new(), |mut types, t| {
                            if !types.contains(&t) {
                                types.push(t);
                            }
                            types
                        })
                };
                self.handle(Event::DropBegin { x, y, mime_types });
            }
            DragPhase::Over => self.handle(Event::DropMove { x, y }),
            DragPhase::Leave => self.handle(Event::DropComplete { x, y }),
            DragPhase::Drop => {
                if own {
                    self.handle(Event::DropOwn { x, y });
                } else {
                    *self.shared.drop_entry.borrow_mut() = clipboard::read_transfer(transfer, "drop");
                    self.handle(Event::DropData { x, y });
                }
                self.handle(Event::DropComplete { x, y });
            }
        }
        if transfer != 0 && phase != DragPhase::Over {
            kompoz_web::release(transfer);
        }
    }

    /// The browser begins a drag of the press (its `dragstart`, at `x`, `y`): the events before it handed over; the
    /// app's drag within it, if one is under way, handed to the page (moved out of the canvas: the host hands a drag
    /// that leaves the window to the platform), now that the page may begin one.
    fn drag_start(&mut self, x: f32, y: f32) -> bool {
        self.handle_records();
        let id = self.target();
        let dragging = self.windows.core(id).and_then(|c| c.host()).is_some_and(|h| h.is_dragging_and_dropping());
        if !dragging {
            return false;
        }
        let _ = (x, y);
        self.shared.drag_starting.set(true);
        let modifiers = kompoz_ui::input::KeyModifiers::None;
        self.handle(Event::MouseMove { x: -1.0, y: -1.0, modifiers, time: -1 });
        self.shared.drag_starting.set(false);
        self.shared.dragging_out.get()
    }
}

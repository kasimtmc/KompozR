//! KompozR's platform on X11 (C++'s x11_platform): its window an X window the window manager decorates, drawn into
//! with Vulkan (`VK_KHR_xlib_surface`), at the desktop's scale (`Xft.dpi`, followed as it changes; `KOMPOZ_SCALE`
//! instead), its frames on the display's beat (the present waits for it; the refresh rate from XRandR's mode under
//! the window); the pointer, its wheel (smooth, as XInput 2 tells it), touch and touchpad pinches through XInput 2
//! (`input`), the keyboard through xkbcommon with an input method (XIM, `keyboard`), the clipboard and drops as
//! selections (ICCCM with INCR, `selection`; XDND 5, `dnd`), the cursors of the desktop's theme (libXcursor).
//! libX11, libXi, libXcursor, libXrandr and libX11-xcb are loaded at run time (`ffi`); where libX11 or XInput 2.2
//! isn't there, or `$DISPLAY` isn't set, the app runs on another platform.
//!
//! `KOMPOZ_PLATFORM=x11` runs it on X11 (Xwayland on a Wayland desktop) instead of Wayland.
//!
//! The desktop's services ([`DesktopServices`]: the portal's theme, accent and text size, the wallpaper's colors, the
//! screen readers' AT-SPI bridge with the window's place on the screen ([`X11Platform::window_bounds`]), the screen
//! saver held off, a microphone and a camera) run as on Wayland, through the platform's hooks.

#[cfg(test)]
mod display_tests;
mod dnd;
mod ffi;
mod input;
mod keyboard;
mod metrics;
mod selection;

use self::dnd::{DropTypes, XdndTarget};
use self::ffi::{
    Atom, Cursor, Decoded, Display, InputMethod, NONE, Time, Window, XEvent, XPoint, XiEvent, vulkan_surface,
};
use self::input::{Devices, Input};
use self::keyboard::X11Keyboard;
use self::selection::{Clipboard, Encoding, IncrReader, decode, encode, part_size};
use super::desktop::{DesktopServices, open_uri};
use super::event_loop::{EventLoop, Waker};
use super::sys::{self, PollFd};
use crate::app::{AppConfig, AppCore, DisplayPlatform, Event, Metrics, Platform};
use crate::system::{log, preferred_language};
use kompoz_gpu::device::{Backend, Surface};
use kompoz_graphics::Rect;
use kompoz_runtime::Content;
use kompoz_ui::KompozHost;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use kompoz_ui::platform::{ImeOptions, PlatformCallbacks, PointerIcon, TextEditState};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::{CStr, c_long};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// X11 among the display platforms ([`DISPLAY_PLATFORMS`](crate::app::DISPLAY_PLATFORMS)).
pub const PLATFORM: DisplayPlatform = DisplayPlatform { name: "x11", run };

/// The app on X11 until it quits (what the program returns), or none where it can't run there (C++'s `runX11`): no
/// X display, its libraries or XInput 2.2 missing.
fn run(config: &AppConfig, content: Rc<dyn Content>) -> Option<ExitCode> {
    let platform = match X11Platform::connect() {
        Ok(platform) => platform,
        Err(why) => {
            if std::env::var("KOMPOZ_PLATFORM").is_ok_and(|p| p == "x11") {
                log(&format!("kompoz: X11: {why}"));
            }
            return None;
        }
    };
    let mut core = AppCore::new(platform, config.clone(), content);
    if !core.start() {
        return Some(ExitCode::FAILURE);
    }
    run_loop(&mut core);
    Some(ExitCode::SUCCESS)
}

/// The loop (C++'s `run`): the X events read (each through the input method first) and handed to the core, then a
/// frame when one is wanted — the pause until it is due the host's for work between frames, and what comes
/// meanwhile handled before it (the present waits for the display's beat: frames are paced, not sent ahead) — or a
/// wait for an X event, a wake (the host's waker, a signal) or the next coroutine delay.
fn run_loop(core: &mut AppCore<X11Platform>) {
    let mut need_frame = true;
    let mut busy = false;
    loop {
        core.platform_mut().pump();
        let mut quit = false;
        for e in core.platform_mut().take_events() {
            quit |= e == Event::Quit;
            core.handle(&e);
            need_frame = true;
        }
        if quit {
            break;
        }
        if need_frame || busy {
            if let Some(due) = core.frame_due() {
                let at = sys::monotonic_now();
                if at < due {
                    core.idle(due - at);
                    let at = sys::monotonic_now();
                    if at < due {
                        if !core.platform_mut().wait(Some(Duration::from_nanos((due - at) as u64))) {
                            break;
                        }
                        continue;
                    }
                }
            }
            need_frame = false;
            let next = core.frame(None);
            if next.quit {
                break;
            }
            busy = next.busy;
            core.platform().event_loop.set_deadline(if next.busy { None } else { next.wake_at });
            continue;
        }
        if !core.platform_mut().wait(None) {
            break; // (the X server went away)
        }
    }
    core.quit();
}

/// The atoms the window speaks with, by their names (C++'s `Atoms`).
macro_rules! atoms {
    ($($field:ident = $name:literal),* $(,)?) => {
        #[allow(non_snake_case)]
        #[derive(Clone, Copy, Debug)]
        struct Atoms { $($field: Atom),* }

        impl Atoms {
            fn intern(display: &Display) -> Atoms {
                let names: &[&CStr] = &[$($name),*];
                let mut atoms = display.intern_atoms(names).into_iter();
                Atoms { $($field: atoms.next().unwrap_or(NONE)),* }
            }
        }
    };
}

atoms! {
    WM_PROTOCOLS = c"WM_PROTOCOLS",
    WM_DELETE_WINDOW = c"WM_DELETE_WINDOW",
    _NET_WM_PING = c"_NET_WM_PING",
    _NET_WM_NAME = c"_NET_WM_NAME",
    _NET_WM_PID = c"_NET_WM_PID",
    _NET_WM_WINDOW_TYPE = c"_NET_WM_WINDOW_TYPE",
    _NET_WM_WINDOW_TYPE_NORMAL = c"_NET_WM_WINDOW_TYPE_NORMAL",
    UTF8_STRING = c"UTF8_STRING",
    CLIPBOARD = c"CLIPBOARD",
    TARGETS = c"TARGETS",
    INCR = c"INCR",
    TEXT = c"TEXT",
    TEXT_PLAIN_UTF8 = c"text/plain;charset=utf-8",
    TEXT_PLAIN = c"text/plain",
    TEXT_URI_LIST = c"text/uri-list",
    RESOURCE_MANAGER = c"RESOURCE_MANAGER",
    KOMPOZ_SELECTION = c"KOMPOZ_SELECTION",
    XdndAware = c"XdndAware",
    XdndEnter = c"XdndEnter",
    XdndPosition = c"XdndPosition",
    XdndStatus = c"XdndStatus",
    XdndLeave = c"XdndLeave",
    XdndDrop = c"XdndDrop",
    XdndFinished = c"XdndFinished",
    XdndSelection = c"XdndSelection",
    XdndTypeList = c"XdndTypeList",
    XdndActionCopy = c"XdndActionCopy",
}

/// The X connection's token in the event loop.
const DISPLAY_SOURCE: u32 = 0;

/// The longest a selection's owner is waited for (C++'s second).
const SELECTION_WAIT_NS: i64 = 1_000_000_000;

/// The input method and what it is told: whether a field takes typing, whether the window has the focus, where the
/// field is (what is composed shows under its start).
struct Ime {
    method: InputMethod,
    active: bool,
    focused: bool,
    spot: XPoint,
}

impl Ime {
    /// The context focused while a field takes typing in the focused window, told where (C++'s `tellInputMethod`).
    fn tell(&mut self) {
        self.method.focus(self.active && self.focused, self.spot);
    }
}

/// The cursors made, as X's, for each of KompozR's, and the one shown.
struct Cursors {
    made: [Cursor; 9],
    icon: PointerIcon,
}

/// What the platform and the host's callbacks share; each cell borrowed only for a moment, never across a call into
/// the host.
struct Shared {
    display: Rc<Display>,
    atoms: Atoms,
    window: Cell<Window>,
    /// The last input event's time (what selections are owned and asked for at).
    last_time: Cell<Time>,
    clipboard: RefCell<Clipboard>,
    cursors: RefCell<Cursors>,
    ime: RefCell<Ime>,
    waker: Waker,
}

impl Shared {
    /// The desktop theme's cursor for the one shown (libXcursor), else X's own (C++'s `showCursor`).
    fn show_cursor(&self) {
        let window = self.window.get();
        if window == NONE {
            return;
        }
        let mut cursors = self.cursors.borrow_mut();
        let (i, name, shape) = match cursors.icon {
            PointerIcon::Default => (0, c"default", ffi::XC_LEFT_PTR),
            PointerIcon::Hand => (1, c"pointer", ffi::XC_HAND2),
            PointerIcon::Text => (2, c"text", ffi::XC_XTERM),
            PointerIcon::Crosshair => (3, c"crosshair", ffi::XC_CROSSHAIR),
            PointerIcon::Move => (4, c"move", ffi::XC_FLEUR),
            PointerIcon::ResizeHorizontal => (5, c"ew-resize", ffi::XC_SB_H_DOUBLE_ARROW),
            PointerIcon::ResizeVertical => (6, c"ns-resize", ffi::XC_SB_V_DOUBLE_ARROW),
            PointerIcon::NotAllowed => (7, c"not-allowed", ffi::XC_X_CURSOR),
            PointerIcon::Wait => (8, c"wait", ffi::XC_WATCH),
        };
        if cursors.made[i] == NONE {
            cursors.made[i] = self.display.theme_cursor(name).unwrap_or_else(|| self.display.font_cursor(shape));
        }
        self.display.define_cursor(window, cursors.made[i]);
        self.display.flush();
    }

    /// What a text target's bytes are; none: not a text target.
    fn encoding_of(&self, target: Atom) -> Option<Encoding> {
        let a = &self.atoms;
        if target == ffi::XA_STRING {
            Some(Encoding::Latin1)
        } else if [a.UTF8_STRING, a.TEXT, a.TEXT_PLAIN_UTF8, a.TEXT_PLAIN].contains(&target) {
            Some(Encoding::Utf8)
        } else {
            None
        }
    }

    /// An event `matches` takes waited for (what else comes stays queued), a second at most (C++'s `await`).
    fn wait_for(&self, matches: &mut dyn FnMut(&XEvent) -> bool) -> Option<XEvent> {
        let deadline = sys::monotonic_now() + SELECTION_WAIT_NS;
        self.display.flush();
        loop {
            if let Some(e) = self.display.check_if_event(matches) {
                return Some(e);
            }
            let left = deadline - sys::monotonic_now();
            if left <= 0 {
                return None;
            }
            let mut fds = [PollFd::readable(self.display.connection_fd())];
            let _ = sys::poll(&mut fds, Some(Duration::from_nanos(left as u64)));
        }
    }

    /// `selection` asked for as `target` (into our KOMPOZ_SELECTION) and its answer read, in parts if it comes in
    /// parts (INCR): its bytes and type; none if refused, or no answer within a second (C++'s `convert`).
    fn convert(&self, selection: Atom, target: Atom, time: Time) -> Option<(Vec<u8>, Atom)> {
        let (d, a, window) = (&self.display, &self.atoms, self.window.get());
        if window == NONE {
            return None;
        }
        d.delete_property(window, a.KOMPOZ_SELECTION);
        d.convert_selection(selection, target, a.KOMPOZ_SELECTION, window, time);
        let answer = self.wait_for(&mut |e| {
            matches!(e.decode(), Decoded::SelectionNotify { requestor, selection: s, .. }
                if requestor == window && s == selection)
        })?;
        let Decoded::SelectionNotify { property, .. } = answer.decode() else { return None };
        if property == NONE {
            return None;
        }
        let first = d.property(window, a.KOMPOZ_SELECTION, true)?;
        if first.kind != a.INCR {
            return Some((first.bytes, first.kind));
        }
        // INCR: each part a new value of the property, deleted once read; an empty one ends it. (A new value whose
        // property is gone was read already: the INCR's own, its notice still queued, or a part read early.)
        let mut reader = IncrReader::default();
        let mut kind = target;
        loop {
            self.wait_for(&mut |e| {
                matches!(e.decode(), Decoded::Property { window: w, atom, state }
                    if w == window && atom == a.KOMPOZ_SELECTION && state == ffi::PROPERTY_NEW_VALUE)
            })?;
            let Some(part) = d.property(window, a.KOMPOZ_SELECTION, true) else { continue };
            if !part.bytes.is_empty() {
                kind = part.kind;
            }
            if let Some(all) = reader.part(&part.bytes) {
                return Some((all, kind));
            }
        }
    }

    /// The clipboard's text: the app's own if it holds it, else the owner's as UTF-8, else as Latin-1 (C++'s
    /// `readClipboard`).
    fn read_clipboard(&self) -> Option<String> {
        let owner = self.display.selection_owner(self.atoms.CLIPBOARD);
        {
            let c = self.clipboard.borrow();
            if owner == self.window.get() && c.owned {
                return Some(c.text.clone());
            }
        }
        if owner == NONE {
            return None;
        }
        for target in [self.atoms.UTF8_STRING, ffi::XA_STRING] {
            if let Some((bytes, kind)) = self.convert(self.atoms.CLIPBOARD, target, self.last_time.get()) {
                return Some(decode(&bytes, self.encoding_of(kind).unwrap_or(Encoding::Utf8)));
            }
        }
        None
    }

    /// The app's text on the clipboard (C++'s `writeClipboard`): the window owns CLIPBOARD, answering for it.
    fn write_clipboard(&self, text: &str) {
        let window = self.window.get();
        self.display.set_selection_owner(self.atoms.CLIPBOARD, window, self.last_time.get());
        let mut c = self.clipboard.borrow_mut();
        c.text = text.to_string();
        c.owned = window != NONE && self.display.selection_owner(self.atoms.CLIPBOARD) == window;
    }

    /// Another app asking for the clipboard the app holds (C++'s `answerSelection`): what types it can have, or the
    /// text (in parts when it is larger than a request carries).
    fn answer_selection(&self, requestor: Window, selection: Atom, target: Atom, property: Atom, time: Time) {
        let (d, a) = (&self.display, &self.atoms);
        let into = if property != NONE { property } else { target }; // (an obsolete client's)
        let mut answered = NONE;
        let mut c = self.clipboard.borrow_mut();
        if selection == a.CLIPBOARD && c.owned {
            if target == a.TARGETS {
                let targets = [a.TARGETS, a.UTF8_STRING, ffi::XA_STRING, a.TEXT, a.TEXT_PLAIN_UTF8, a.TEXT_PLAIN];
                d.set_property_32(requestor, into, ffi::XA_ATOM, &targets.map(|t| t as c_long));
                answered = into;
            } else if let Some(encoding) = self.encoding_of(target) {
                let kind = if target == a.TEXT { a.UTF8_STRING } else { target };
                let data = encode(&c.text, encoding);
                if data.len() > part_size(d.max_request_bytes()) {
                    d.select_input(requestor, ffi::PROPERTY_CHANGE_MASK);
                    d.set_property_32(requestor, into, a.INCR, &[data.len() as c_long]);
                    c.start_transfer(requestor, into, kind);
                } else {
                    d.set_property_8(requestor, into, kind, &data);
                }
                answered = into;
            }
        }
        drop(c);
        d.send_selection_notify(requestor, selection, target, answered, time);
        d.flush();
    }

    /// A requestor took a part of a transfer (deleted the property): the next one, and an empty one after the last
    /// (C++'s `sendNextPart`).
    fn send_next_part(&self, window: Window, property: Atom) {
        let d = &self.display;
        let mut c = self.clipboard.borrow_mut();
        let Some(t) = c.transfers().iter().find(|t| t.requestor == window && t.property == property).copied() else {
            return;
        };
        let data = encode(&c.text, self.encoding_of(t.kind).unwrap_or(Encoding::Utf8));
        let Some(part) = c.next_part(window, property, data.len(), part_size(d.max_request_bytes())) else { return };
        d.set_property_8(window, property, part.kind, &data[part.range]);
        if part.done {
            d.select_input(window, ffi::NO_EVENT_MASK);
        }
        d.flush();
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        for &c in &self.cursors.borrow().made {
            if c != NONE {
                self.display.free_cursor(c);
            }
        }
    }
}

/// The callbacks the X11 platform gives the host (C++'s `attach`): waking the loop, the input method told about the
/// field taking typing, cursors, the clipboard, links.
struct X11Callbacks(Rc<Shared>);

impl PlatformCallbacks for X11Callbacks {
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        Some(self.0.waker.as_fn())
    }

    /// The input method's context focused while a field takes typing, what is composed shown under the field's
    /// start (`area`, px: the window's pixels).
    fn text_input(&self, active: bool, area: Rect, _: &TextEditState, _: &ImeOptions) {
        let spot = |v: f32| v.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        let mut ime = self.0.ime.borrow_mut();
        ime.active = active;
        ime.spot = XPoint { x: spot(area.left), y: spot(area.bottom) };
        ime.tell();
    }

    fn cursor(&self, icon: PointerIcon) {
        self.0.cursors.borrow_mut().icon = icon;
        self.0.show_cursor();
    }

    fn get_clipboard(&self) -> Option<String> {
        self.0.read_clipboard()
    }

    fn set_clipboard(&self, text: &str) {
        self.0.write_clipboard(text);
    }

    fn open_uri(&self, uri: &str) {
        open_uri(uri);
    }
}

/// The X11 platform (C++'s X11Platform): the display, its window, the loop, the input as KompozR's events.
pub struct X11Platform {
    shared: Rc<Shared>,
    event_loop: EventLoop,
    events: VecDeque<Event>,
    title: String,
    /// The window's size, its pixels (what the swapchain is made at).
    size: Rc<Cell<(i32, i32)>>,
    /// Where it is on the screen.
    position: (i32, i32),
    /// The desktop's scale (Xft.dpi / 96).
    dpi_scale: f64,
    refresh_rate: f32,
    input: Input,
    keyboard: X11Keyboard,
    drag: XdndTarget,
    drop_types: DropTypes,
    desktop: DesktopServices,
}

impl X11Platform {
    /// The display, XInput 2.2 and the atoms, the devices, the keyboard, the desktop's scale, the loop (C++'s
    /// `connect`); why not where the app can't run here.
    pub fn connect() -> Result<X11Platform, String> {
        if std::env::var_os("DISPLAY").is_none_or(|d| d.is_empty()) {
            return Err("DISPLAY isn't set".into());
        }
        let display = Rc::new(Display::open()?);
        let atoms = Atoms::intern(&display);
        let root = display.root();
        display.select_input(root, ffi::PROPERTY_CHANGE_MASK); // (the desktop's resources: Xft.dpi)
        display.xi_select_events(root, ffi::XI_ALL_DEVICES, &[ffi::XI_HIERARCHY_CHANGED]);
        let input = Input::new(Devices::new(&display.xi_devices()));
        display.set_detectable_auto_repeat(); // (a held key's repeats come as presses alone)
        let keyboard = X11Keyboard::new(&display);
        let mut event_loop = EventLoop::new().map_err(|e| format!("no event loop: {e}"))?;
        event_loop.watch(display.connection_fd(), DISPLAY_SOURCE);
        event_loop.quit_on_signals().map_err(|e| format!("no signal handlers: {e}"))?;
        let shared = Rc::new(Shared {
            atoms,
            window: Cell::new(NONE),
            last_time: Cell::new(ffi::CURRENT_TIME),
            clipboard: RefCell::new(Clipboard::default()),
            cursors: RefCell::new(Cursors { made: [NONE; 9], icon: PointerIcon::Default }),
            ime: RefCell::new(Ime {
                method: InputMethod::new(display.clone()),
                active: false,
                focused: false,
                spot: XPoint::default(),
            }),
            waker: event_loop.waker(),
            display,
        });
        let drop_types = DropTypes {
            uri_list: atoms.TEXT_URI_LIST,
            text_plain_utf8: atoms.TEXT_PLAIN_UTF8,
            utf8_string: atoms.UTF8_STRING,
            text_plain: atoms.TEXT_PLAIN,
        };
        let mut platform = X11Platform {
            shared,
            event_loop,
            events: VecDeque::new(),
            title: "KompozR".into(),
            size: Rc::new(Cell::new((1, 1))),
            position: (0, 0),
            dpi_scale: 1.0,
            refresh_rate: 60.0,
            input,
            keyboard,
            drag: XdndTarget::default(),
            drop_types,
            desktop: DesktopServices::new(),
        };
        platform.dpi_scale = platform.desktop_scale();
        Ok(platform)
    }

    fn display(&self) -> &Display {
        &self.shared.display
    }

    fn window(&self) -> Window {
        self.shared.window.get()
    }

    /// The window's place on the screen and its size, in its pixels (what a screen reader's bridge reports as the
    /// app's extents; C++ passed it to the desktop's `composed`).
    pub fn window_bounds(&self) -> Rect {
        let (w, h) = self.size.get();
        let (x, y) = self.position;
        Rect::new(x as f32, y as f32, (x + w) as f32, (y + h) as f32)
    }

    /// The events gathered since, taken.
    fn take_events(&mut self) -> VecDeque<Event> {
        std::mem::take(&mut self.events)
    }

    /// px per dp: `KOMPOZ_SCALE`, else the desktop's.
    fn scale(&self) -> f64 {
        self.option("KOMPOZ_SCALE")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|&s| s > 0.0)
            .unwrap_or(self.dpi_scale)
    }

    /// The desktop's scale: Xft.dpi in the root window's resources, / 96.
    fn desktop_scale(&self) -> f64 {
        let resources = self.display().property(self.display().root(), self.shared.atoms.RESOURCE_MANAGER, false);
        metrics::xft_dpi_scale(&resources.map(|p| String::from_utf8_lossy(&p.bytes).into_owned()).unwrap_or_default())
    }

    // ---- The loop ----

    /// The X events that came, each through the input method first (C++'s `pump`); then what the input method's
    /// callbacks told acted on, and what the desktop's services told.
    fn pump(&mut self) {
        while self.display().pending() > 0 {
            let mut e = self.display().next_event();
            if self.display().filter_event(&mut e) {
                continue;
            }
            self.handle(&e);
        }
        let window = Some(self.window()).filter(|&w| w != NONE);
        let mut ime = self.shared.ime.borrow_mut();
        let spot = ime.spot;
        if ime.method.update(window, spot) {
            ime.tell();
        }
        drop(ime);
        self.events.extend(self.desktop.take_events());
    }

    /// Up to `timeout` (none: until one) for an X event, a wake, the deadline or a signal (C++'s `waitForEvents`).
    /// False if the connection broke.
    fn wait(&mut self, timeout: Option<Duration>) -> bool {
        self.display().flush();
        if self.display().pending() > 0 {
            return true;
        }
        let Ok(ready) = self.event_loop.wait(timeout) else { return false };
        if ready.woken || ready.deadline {
            self.events.push_back(Event::Wake);
        }
        if self.event_loop.take_quit_signal() {
            self.events.push_back(Event::Quit);
        }
        !(ready.source(DISPLAY_SOURCE) && self.display().connection_closed())
    }

    /// An event from the X server, or for it to answer (C++'s `handle`).
    fn handle(&mut self, e: &XEvent) {
        if let Some(xi) = self.display().xi_event(e) {
            return self.xi_input(xi);
        }
        if self.display().is_randr_event(e) {
            return self.displays_changed();
        }
        let a = self.shared.atoms;
        match e.decode() {
            Decoded::Key(key) => {
                self.shared.last_time.set(key.time());
                let ime = self.shared.ime.borrow();
                let events =
                    self.keyboard.key(&self.shared.display, &ime.method, &key, sys::monotonic_now(), ime.active);
                drop(ime);
                self.events.extend(events);
            }
            Decoded::Mapping { request } => {
                if request == ffi::MAPPING_KEYBOARD || request == ffi::MAPPING_MODIFIER {
                    self.display().refresh_keyboard_mapping(e);
                    self.keyboard.refresh(&self.shared.display);
                }
            }
            Decoded::Focus { focus_in, mode, detail } => {
                if mode == ffi::NOTIFY_GRAB
                    || mode == ffi::NOTIFY_UNGRAB
                    || detail == ffi::NOTIFY_INFERIOR
                    || detail == ffi::NOTIFY_POINTER
                {
                    return;
                }
                {
                    let mut ime = self.shared.ime.borrow_mut();
                    ime.focused = focus_in;
                    ime.tell();
                }
                if !focus_in {
                    self.keyboard.leave();
                }
                self.events.push_back(if focus_in { Event::FocusGained } else { Event::FocusLost });
            }
            Decoded::Configure { window, width, height, x, y, send_event } => {
                if window != self.window() {
                    return;
                }
                if (width, height) != self.size.get() {
                    self.size.set((width, height));
                    self.events.push_back(Event::Resized);
                }
                self.moved(x, y, send_event);
            }
            Decoded::Expose { count } => {
                if count == 0 {
                    self.events.push_back(Event::Exposed);
                }
            }
            Decoded::Property { window, atom, state } => {
                if window == self.display().root() && atom == a.RESOURCE_MANAGER {
                    let s = self.desktop_scale();
                    if s != self.dpi_scale {
                        self.dpi_scale = s;
                        self.events.push_back(Event::MetricsChanged);
                    }
                } else if state == ffi::PROPERTY_DELETE {
                    self.shared.send_next_part(window, atom);
                }
            }
            Decoded::ClientMessage { message_type, data, .. } => self.message(message_type, data),
            Decoded::SelectionRequest { requestor, selection, target, property, time } => {
                self.shared.answer_selection(requestor, selection, target, property, time);
            }
            Decoded::SelectionClear { selection } => {
                if selection == a.CLIPBOARD {
                    self.shared.clipboard.borrow_mut().owned = false;
                }
            }
            Decoded::SelectionNotify { .. } | Decoded::Other => {}
        }
    }

    /// XInput 2's events: the pointer, its wheel, touch, pinches; the devices (C++'s `input`).
    fn xi_input(&mut self, e: XiEvent) {
        if let XiEvent::Device(d) = &e {
            self.shared.last_time.set(d.time);
        }
        if e == XiEvent::Hierarchy {
            self.input.devices = Devices::new(&self.display().xi_devices());
            self.events.push_back(Event::DevicesChanged);
            return;
        }
        let events = self.input.events(&e, sys::monotonic_now());
        self.events.extend(events);
    }

    // ---- Sizes and displays ----

    /// Where the window is on the screen (a window manager's own configure tells it; else asked), and the refresh
    /// rate of the display it is on (C++'s `moved`).
    fn moved(&mut self, x: i32, y: i32, send_event: bool) {
        self.position = if send_event {
            (x, y)
        } else {
            self.display().translate_coordinates(self.window(), self.display().root(), 0, 0).unwrap_or(self.position)
        };
        self.displays_changed();
    }

    /// The refresh rate of the display under the window's middle (XRandR's mode on its CRTC), told if it changed.
    fn displays_changed(&mut self) {
        let (w, h) = self.size.get();
        let (x, y) = self.position;
        let rate = metrics::refresh_rate_at(&self.display().crtcs(), x + w / 2, y + h / 2);
        if rate != self.refresh_rate {
            self.refresh_rate = rate;
            self.events.push_back(Event::MetricsChanged);
        }
    }

    // ---- The window manager's messages and drags (XDND) ----

    fn message(&mut self, kind: Atom, data: [c_long; 5]) {
        let a = self.shared.atoms;
        let d = self.shared.display.clone();
        if kind == a.WM_PROTOCOLS {
            let what = data[0] as Atom;
            if what == a.WM_DELETE_WINDOW {
                self.events.push_back(Event::Quit);
            } else if what == a._NET_WM_PING {
                let root = d.root();
                d.send_client_message(
                    root,
                    root,
                    kind,
                    data,
                    ffi::SUBSTRUCTURE_NOTIFY_MASK | ffi::SUBSTRUCTURE_REDIRECT_MASK,
                );
                d.flush();
            }
            return;
        }
        if kind == a.XdndEnter {
            // The types it offers (three in the message, or its XdndTypeList).
            let list = XdndTarget::has_type_list(&data)
                .then(|| d.property(data[0] as Window, a.XdndTypeList, false).map(|p| p.longs()).unwrap_or_default());
            self.drag.enter(&data, list.as_deref(), &self.drop_types);
        } else if kind == a.XdndPosition {
            let Some((root_x, root_y)) = self.drag.position_on_screen(&data) else { return };
            let (x, y) = d
                .translate_coordinates(d.root(), self.window(), root_x, root_y)
                .unwrap_or((root_x - self.position.0, root_y - self.position.1));
            let (events, flags, action) = self.drag.moved(x, y, a.XdndActionCopy);
            self.events.extend(events);
            self.drag_reply(a.XdndStatus, flags, action);
        } else if kind == a.XdndLeave {
            let events = self.drag.leave(&data);
            self.events.extend(events);
        } else if kind == a.XdndDrop {
            let Some(request) = self.drag.drop_request(&data) else { return };
            let text = request.and_then(|r| {
                let (bytes, kind) = self.shared.convert(a.XdndSelection, r.kind, r.time)?;
                Some(decode(&bytes, self.shared.encoding_of(kind).unwrap_or(Encoding::Utf8)))
            });
            let source = self.drag.source();
            let (events, accepted, action) = self.drag.dropped(text, &self.drop_types, a.XdndActionCopy);
            self.events.extend(events);
            self.reply_to(source, a.XdndFinished, accepted, action);
        }
    }

    /// A message to the drag's source: whether the window takes it (XdndStatus).
    fn drag_reply(&self, kind: Atom, flags: c_long, action: Atom) {
        self.reply_to(self.drag.source(), kind, flags, action);
    }

    /// A message to a drag's `source`: whether the window takes it (XdndStatus) or took it (XdndFinished).
    fn reply_to(&self, source: Window, kind: Atom, flags: c_long, action: Atom) {
        if source == NONE {
            return;
        }
        let mut data = [self.window() as c_long, flags, 0, 0, 0];
        data[if kind == self.shared.atoms.XdndStatus { 4 } else { 2 }] = action as c_long;
        self.display().send_client_message(source, source, kind, data, ffi::NO_EVENT_MASK);
        self.display().flush();
    }
}

impl Platform for X11Platform {
    fn option(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn log(&self, message: &str) {
        log(message);
    }

    fn now(&self) -> i64 {
        sys::monotonic_now()
    }

    fn backends(&self) -> Vec<Backend> {
        vec![Backend::Vulkan]
    }

    /// The window (C++'s `openWindow`): its names, its process, what it answers (closing, the window manager's
    /// ping), its kind, drops (XDND 5); the pointer, touch and pinches through XInput 2 (the keyboard's events are
    /// the core ones, which the input method filters); its input context; then shown.
    fn open_window(&mut self, app: &AppConfig, backend: Backend) -> Result<Surface, String> {
        if backend != Backend::Vulkan {
            return Err(format!("no {} on X11", backend.name()));
        }
        self.title = app.title.clone();
        let scale = self.scale();
        let width = ((app.width.0 as f64 * scale).round() as i32).max(1);
        let height = ((app.height.0 as f64 * scale).round() as i32).max(1);
        let events = ffi::STRUCTURE_NOTIFY_MASK
            | ffi::FOCUS_CHANGE_MASK
            | ffi::EXPOSURE_MASK
            | ffi::PROPERTY_CHANGE_MASK
            | ffi::KEY_PRESS_MASK
            | ffi::KEY_RELEASE_MASK;
        let d = self.shared.display.clone();
        let a = self.shared.atoms;
        let window = d.create_window(width as u32, height as u32, events).ok_or("the window couldn't be made")?;
        self.shared.window.set(window);
        self.size.set((width, height));
        d.set_property_8(window, a._NET_WM_NAME, a.UTF8_STRING, self.title.as_bytes());
        // (Older tools read a Latin-1 one.)
        let name_type = if self.title.is_ascii() { ffi::XA_STRING } else { a.UTF8_STRING };
        d.set_property_8(window, ffi::XA_WM_NAME, name_type, self.title.as_bytes());
        d.set_class_hint(window, c"dev.kompoz.app", c"dev.kompoz.app");
        d.set_property_32(window, a._NET_WM_PID, ffi::XA_CARDINAL, &[std::process::id() as c_long]);
        d.set_wm_protocols(window, &[a.WM_DELETE_WINDOW, a._NET_WM_PING]);
        d.set_property_32(window, a._NET_WM_WINDOW_TYPE, ffi::XA_ATOM, &[a._NET_WM_WINDOW_TYPE_NORMAL as c_long]);
        d.set_property_32(window, a.XdndAware, ffi::XA_ATOM, &[5]);
        let mut pointer = vec![
            ffi::XI_MOTION,
            ffi::XI_BUTTON_PRESS,
            ffi::XI_BUTTON_RELEASE,
            ffi::XI_ENTER,
            ffi::XI_LEAVE,
            ffi::XI_TOUCH_BEGIN,
            ffi::XI_TOUCH_UPDATE,
            ffi::XI_TOUCH_END,
        ];
        if d.xi_minor() >= 4 {
            pointer.extend([ffi::XI_GESTURE_PINCH_BEGIN, ffi::XI_GESTURE_PINCH_UPDATE, ffi::XI_GESTURE_PINCH_END]);
        }
        d.xi_select_events(window, ffi::XI_ALL_MASTER_DEVICES, &pointer);
        {
            let mut ime = self.shared.ime.borrow_mut();
            let spot = ime.spot;
            if ime.method.open() {
                ime.method.create_context(window, spot);
            }
            ime.tell();
        }
        self.shared.show_cursor();
        d.map_window(window);
        d.flush();
        self.events.push_back(Event::Shown);
        let size = self.size.clone();
        Ok(Surface {
            vulkan: Some(vulkan_surface(&d, window)),
            drawable_size: Some(Box::new(move || size.get())),
            pipeline_cache: None,
        })
    }

    fn close_window(&mut self) {
        self.shared.ime.borrow_mut().method.destroy_context();
        let window = self.shared.window.replace(NONE);
        if window != NONE {
            self.display().destroy_window(window);
            self.display().flush();
        }
    }

    fn show_error(&mut self, message: &str) {
        log(message);
    }

    /// Points are the window's pixels; the scale is the desktop's (Xft.dpi / 96), the text size the portal's;
    /// `KOMPOZ_SCALE` and `KOMPOZ_FONT_SCALE` stand in for them.
    fn metrics(&self) -> Metrics {
        let (w, h) = self.size.get();
        Metrics {
            pixel_width: w,
            pixel_height: h,
            point_width: w,
            point_height: h,
            scale: self.scale() as f32,
            point_to_px: 1.0,
            font_scale: self
                .option("KOMPOZ_FONT_SCALE")
                .and_then(|v| v.trim().parse::<f32>().ok())
                .map(|s| s.max(0.5))
                .unwrap_or_else(|| self.desktop.font_scale().unwrap_or(1.0)),
            refresh_rate: self.refresh_rate,
            safe_area: Some(Rect::new(0.0, 0.0, w as f32, h as f32)),
            dark_theme: self.desktop.dark_theme(),
            language: preferred_language().unwrap_or_default(),
            title_bar: None, // (the window manager decorates the window)
        }
    }

    fn pixel_size(&self) -> (i32, i32) {
        self.size.get()
    }

    fn set_window_size(&mut self, width: i32, height: i32) {
        if self.window() != NONE {
            self.display().resize_window(self.window(), width.max(1) as u32, height.max(1) as u32);
            self.display().flush();
        }
    }

    /// A mouse is a fine pointer, a touch screen alone a coarse one; a keyboard is there; a microphone and a camera
    /// as probed.
    fn media(&mut self, probe: bool) -> MediaInfo {
        let d = &self.input.devices;
        let pointer = if d.has_mouse {
            PointerPrecision::Fine
        } else if d.has_touch {
            PointerPrecision::Coarse
        } else {
            PointerPrecision::None
        };
        self.desktop.media(probe, pointer, true)
    }

    /// The host's callbacks; the desktop's services started, waking the loop.
    fn attach(&mut self, _: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        self.desktop.start(&self.title, self.event_loop.waker().as_fn());
        Rc::new(X11Callbacks(self.shared.clone()))
    }

    fn metrics_updated(&mut self, _: &KompozHost) {
        self.desktop.metrics_updated();
    }

    fn handled(&mut self, host: &KompozHost, event: &Event) {
        self.desktop.handled(host, event);
    }

    /// The screen saver held off while the app keeps the screen on (D-Bus: X11 has no inhibitor of its own); the
    /// semantics tree published to the screen readers, the window where it is on the screen.
    fn composed(&mut self, host: &KompozHost, redraw: bool, frame_time: i64) {
        self.desktop.keep_screen_on(host.keep_screen_on(), &self.title);
        let bounds = self.window_bounds();
        self.desktop.composed(host, redraw, frame_time, bounds, 1.0, &self.title);
    }

    fn wants_frames(&self) -> bool {
        self.desktop.wants_frames()
    }
}

impl Drop for X11Platform {
    /// The desktop's services' threads ended, then the window closed (C++'s destructor).
    fn drop(&mut self) {
        self.desktop.stop();
        self.close_window();
    }
}

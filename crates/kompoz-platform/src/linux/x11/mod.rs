//! KompozR's platform on X11 (C++'s x11_platform): its window an X window the window manager decorates, drawn into
//! with Vulkan (`VK_KHR_xlib_surface`), at the desktop's scale (`Xft.dpi`, followed as it changes; `KOMPOZ_SCALE`
//! instead), its frames on the display's beat (the present waits for it; the refresh rate from XRandR's mode under
//! the window); the pointer, its wheel (smooth, as XInput 2 tells it), touch and touchpad pinches through XInput 2
//! (`input`), the keyboard through xkbcommon with an input method (XIM, `keyboard`; what it composes shown in the
//! field on the spot where it can, `preedit`), the clipboard and drops as selections (ICCCM with INCR, `selection`;
//! XDND 5, `dnd`), the app's own drags out to other apps (XDND 5's source: `dnd_source`, `drag`), the cursors of the
//! desktop's theme (libXcursor).
//! libX11, libXi, libXcursor, libXrandr and libX11-xcb are loaded at run time (`ffi`); where libX11 or XInput 2.2
//! isn't there, or `$DISPLAY` isn't set, the app runs on another platform.
//!
//! `KOMPOZ_PLATFORM=x11` runs it on X11 (Xwayland on a Wayland desktop) instead of Wayland.
//!
//! The desktop's services ([`DesktopServices`]: the portal's theme, accent and text size, the wallpaper's colors, the
//! screen readers' AT-SPI bridge with the window's place on the screen ([`X11Platform::window_bounds`]), the screen
//! saver held off, a microphone and a camera, the tray icons and notifications) run as on Wayland, through the
//! platform's hooks.
//!
//! All the app's windows are on one connection (the main one and each `Window`'s: [`X11Window`], a handle on it by
//! its id), each an X window of its own, drawn into by a Vulkan device of its own; the events are routed to the
//! window they are for by their X window. The platform works on one window at a time ([`X11Platform::select`]: its
//! state the platform's `win`, the others parked). The clipboard is owned by a window of its own that is never
//! shown (it outlives the app's windows: an `application { }` has none at times).

#[cfg(test)]
mod display_tests;
mod dnd;
mod dnd_source;
mod drag;
pub mod ffi;
mod input;
mod keyboard;
mod metrics;
mod preedit;
mod selection;

use self::dnd::XdndTarget;
use self::drag::{OutgoingDrag, PointerAt};
use self::ffi::{
    Atom, Cursor, Decoded, Display, InputMethod, NONE, Time, Window, XEvent, XPoint, XiEvent, vulkan_surface,
};
use self::input::{Devices, Input};
use self::keyboard::X11Keyboard;
use self::selection::{Clipboard, IncrReader, part_size};
use super::desktop::{DesktopServices, open_uri};
use super::event_loop::{EventLoop, Waker};
use super::sys::{self, PollFd};
use crate::app::{
    AppConfig, AppContent, AppWindows, DisplayPlatform, Event, MenuBarShown, Metrics, Platform, TrayIcon, TrayImage,
    TrayMenuItem, WindowSpec,
};
use crate::linux::transfer::{Payload, X11_TEXT_TYPES};
use crate::system::{log, preferred_language};
use kompoz_gpu::device::{Backend, Surface};
use kompoz_graphics::Rect;
use kompoz_ui::KompozHost;
use kompoz_ui::clipboard::{ClipDataReader, ClipEntry, ClipItem, ClipMetadata, is_plain_text_type};
use kompoz_ui::draganddrop::PlatformDragAndDrop;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use kompoz_ui::platform::{ImeOptions, PlatformCallbacks, PointerIcon, TextEditState};
use kompoz_ui::platform_text_layout::{PlatformTextLayout, caret_area};
use kompoz_ui::window::{Notification, WindowId, WindowPlacement, WindowRequest};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::ffi::{CStr, c_long};
use std::process::ExitCode;
use std::rc::Rc;
use std::rc::Weak;
use std::sync::Arc;
use std::time::Duration;

/// X11 among the display platforms ([`DISPLAY_PLATFORMS`](crate::app::DISPLAY_PLATFORMS)).
pub const PLATFORM: DisplayPlatform = DisplayPlatform { name: "x11", run };

/// The app on X11 until it quits (what the program returns), or none where it can't run there (C++'s `runX11`): no
/// X display, its libraries or XInput 2.2 missing. Its windows share one connection: the root core's (the main
/// window's, or the app's composition's without one) and each `Window`'s, a handle on it by id.
fn run(config: &AppConfig, content: AppContent) -> Option<ExitCode> {
    let platform = match X11Platform::connect() {
        Ok(platform) => platform,
        Err(why) => {
            if std::env::var("KOMPOZ_PLATFORM").is_ok_and(|p| p == "x11") {
                log(&format!("kompoz: X11: {why}"));
            }
            return None;
        }
    };
    let x11 = Rc::new(RefCell::new(platform));
    let app_id = match content {
        AppContent::Application(_) => WindowId::APPLICATION,
        AppContent::Window(_) => WindowId::MAIN,
    };
    x11.borrow_mut().app_id = app_id;
    let root = content.core(X11Window::new(&x11, app_id), config);
    let make = {
        let x11 = x11.clone();
        Box::new(move |r: &WindowRequest| Some(X11Window::new(&x11, r.id())))
    };
    let mut windows = AppWindows::new(root, make);
    if !windows.start() {
        return Some(ExitCode::FAILURE);
    }
    run_loop(&mut windows, &x11);
    drop(windows); // (the windows and their devices before the connection)
    Some(ExitCode::SUCCESS)
}

/// The loop (C++'s `run`): the X events read (each through the input method first) and handed to the cores of the
/// windows they are for, the windows synced with their requests, then a frame of each window that wants one — the
/// pause until the earliest is due the hosts' for work between frames, and what comes meanwhile handled before it
/// (the present waits for the display's beat: frames are paced, not sent ahead) — or a wait for an X event, a wake
/// (the hosts' waker, a signal) or the next coroutine delay.
fn run_loop(windows: &mut AppWindows<X11Window>, x11: &Rc<RefCell<X11Platform>>) {
    let mut need_frame: HashMap<WindowId, bool> = HashMap::new();
    let mut busy: HashMap<WindowId, bool> = HashMap::new();
    loop {
        x11.borrow_mut().pump();
        let mut quit = false;
        let events = x11.borrow_mut().take_events();
        for (id, e) in events {
            quit |= e == Event::Quit;
            windows.handle(id, &e);
            need_frame.insert(id, true);
        }
        if quit {
            break;
        }
        windows.sync();
        let ids = windows.ids();
        let wanted: Vec<WindowId> = ids
            .iter()
            .copied()
            .filter(|id| {
                *need_frame.get(id).unwrap_or(&true)
                    || *busy.get(id).unwrap_or(&false)
                    || windows.core(*id).is_some_and(|c| c.scripted())
            })
            .collect();
        if !wanted.is_empty() {
            let due = wanted.iter().filter_map(|&id| windows.core(id).and_then(|c| c.frame_due())).min();
            if let Some(due) = due {
                let at = sys::monotonic_now();
                if at < due {
                    for core in windows.cores_mut() {
                        core.idle(due - at);
                    }
                    let at = sys::monotonic_now();
                    if at < due {
                        if !x11.borrow_mut().wait(Some(Duration::from_nanos((due - at) as u64))) {
                            break;
                        }
                        continue;
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
                break;
            }
            let any_busy = busy.values().any(|b| *b);
            x11.borrow().event_loop.set_deadline(if any_busy { None } else { wake_at });
            continue;
        }
        if windows.quitting() {
            break;
        }
        if !x11.borrow_mut().wait(None) {
            break; // (the X server went away)
        }
    }
    windows.quit();
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
    WM_CHANGE_STATE = c"WM_CHANGE_STATE",
    _NET_WM_STATE = c"_NET_WM_STATE",
    _NET_WM_STATE_MAXIMIZED_VERT = c"_NET_WM_STATE_MAXIMIZED_VERT",
    _NET_WM_STATE_MAXIMIZED_HORZ = c"_NET_WM_STATE_MAXIMIZED_HORZ",
    _NET_WM_STATE_FULLSCREEN = c"_NET_WM_STATE_FULLSCREEN",
    _NET_WM_STATE_HIDDEN = c"_NET_WM_STATE_HIDDEN",
    _NET_WM_STATE_ABOVE = c"_NET_WM_STATE_ABOVE",
    _NET_ACTIVE_WINDOW = c"_NET_ACTIVE_WINDOW",
    _NET_WM_ICON = c"_NET_WM_ICON",
    _MOTIF_WM_HINTS = c"_MOTIF_WM_HINTS",
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
    XdndActionMove = c"XdndActionMove",
    XdndActionLink = c"XdndActionLink",
    XdndProxy = c"XdndProxy",
    _NET_WM_WINDOW_TYPE_DND = c"_NET_WM_WINDOW_TYPE_DND",
    _KDE_NET_WM_APPMENU_SERVICE_NAME = c"_KDE_NET_WM_APPMENU_SERVICE_NAME",
    _KDE_NET_WM_APPMENU_OBJECT_PATH = c"_KDE_NET_WM_APPMENU_OBJECT_PATH",
}

/// The X connection's token in the event loop.
const DISPLAY_SOURCE: u32 = 0;

/// The longest a selection's owner is waited for (C++'s second).
const SELECTION_WAIT_NS: i64 = 1_000_000_000;

/// The input method and what it is told: whether a field takes typing, whether the window has the focus, where the
/// field's caret is (what is composed shows under it); whether what it composes on the spot is shown in the field.
struct Ime {
    method: InputMethod,
    active: bool,
    focused: bool,
    spot: XPoint,
    composing: bool,
}

impl Ime {
    /// The context focused while a field takes typing in the focused window, told where (C++'s `tellInputMethod`).
    fn tell(&mut self) {
        self.method.focus(self.active && self.focused, self.spot);
    }

    /// What the input method composes on the spot, if it changed: as the core's event while a field takes typing
    /// (none composed: told only if something was shown — an empty composing text takes the field's selection).
    fn preedit(&mut self) -> Option<Event> {
        let (text, (start, end)) = self.method.take_preedit()?;
        if !self.active || (text.is_empty() && !self.composing) {
            return None;
        }
        self.composing = !text.is_empty();
        let cursor = (!text.is_empty()).then_some((start, end));
        Some(Event::Preedit { text, cursor })
    }

    /// What the input method composes given up, not told (the field it was for went, or the app changed it).
    fn reset(&mut self) {
        self.method.reset();
        self.composing = false;
    }
}

/// The cursors made, as X's, for each of KompozR's, and each window's (its host's).
struct Cursors {
    made: [Cursor; 9],
    icons: HashMap<Window, PointerIcon>,
}

/// What the platform and the host's callbacks share; each cell borrowed only for a moment, never across a call into
/// the host.
struct Shared {
    display: Rc<Display>,
    atoms: Atoms,
    /// The window the platform works on (its `win`'s).
    window: Cell<Window>,
    /// A window never shown that owns the clipboard and asks for selections (the app's, whichever windows it has).
    selection: Window,
    /// The last input event's time (what selections are owned and asked for at).
    last_time: Cell<Time>,
    clipboard: RefCell<Clipboard>,
    /// The names of the atoms read (they never change on a server).
    atom_names: RefCell<HashMap<Atom, String>>,
    /// Counted up as each drop is done with: what a drop offers is read only until then.
    drop_generation: Cell<u64>,
    cursors: RefCell<Cursors>,
    ime: RefCell<Ime>,
    waker: Waker,
    /// A drag of the app's own the host asked the platform to take, started once the event that moved it out of
    /// the window is handled ([`Platform::handled`]).
    drag_request: RefCell<Option<PlatformDragAndDrop>>,
    /// A drag can be taken: the window is open and the pointer (or a finger) has been seen.
    can_drag: Cell<bool>,
    /// A drag of the app's own runs.
    dragging: Cell<bool>,
}

impl Shared {
    /// The desktop theme's cursor for the one `window`'s host shows (libXcursor), else X's own (C++'s
    /// `showCursor`): X keeps a cursor for each window.
    fn show_cursor(&self, window: Window) {
        if window == NONE {
            return;
        }
        let mut cursors = self.cursors.borrow_mut();
        let icon = cursors.icons.get(&window).copied().unwrap_or_default();
        let (i, name, shape) = match icon {
            PointerIcon::Default => (0, c"default", ffi::XC_LEFT_PTR),
            PointerIcon::Hand => (1, c"pointer", ffi::XC_HAND2),
            PointerIcon::Text | PointerIcon::Handwriting => (2, c"text", ffi::XC_XTERM),
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

    /// `atom`'s name (asked of the server once).
    fn atom_name(&self, atom: Atom) -> Option<String> {
        if let Some(name) = self.atom_names.borrow().get(&atom) {
            return Some(name.clone());
        }
        let name = self.display.atom_name(atom)?;
        self.atom_names.borrow_mut().insert(atom, name.clone());
        Some(name)
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
        let (d, a, window) = (&self.display, &self.atoms, self.selection);
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

    /// The types `selection`'s owner offers (its `TARGETS`, as names), asked for at `time`; none if it doesn't
    /// say.
    fn targets_of(&self, selection: Atom, time: Time) -> Option<Vec<String>> {
        let (bytes, kind) = self.convert(selection, self.atoms.TARGETS, time)?;
        if kind != ffi::XA_ATOM && kind != self.atoms.TARGETS {
            return None;
        }
        let (atoms, _) = bytes.as_chunks::<{ std::mem::size_of::<c_long>() }>();
        Some(atoms.iter().filter_map(|&a| self.atom_name(c_long::from_ne_bytes(a) as Atom)).collect())
    }

    /// What the clipboard holds (C++'s `readClipboard`, all of it): the app's own entry if it holds it; else the
    /// owner's, of the types it offers (UTF-8 and Latin-1 text if it doesn't say), read as they are asked for while
    /// it owns the clipboard.
    fn clip_entry(self: &Rc<Self>) -> Option<ClipEntry> {
        let owner = self.display.selection_owner(self.atoms.CLIPBOARD);
        {
            let c = self.clipboard.borrow();
            if owner == self.selection && c.owned {
                return c.payload.as_ref().map(|p| p.entry().clone());
            }
        }
        if owner == NONE {
            return None;
        }
        let time = self.last_time.get();
        let types = self
            .targets_of(self.atoms.CLIPBOARD, time)
            .unwrap_or_else(|| vec!["UTF8_STRING".to_string(), "STRING".to_string()]);
        let metadata = ClipMetadata::of_offered(&types);
        let data: Vec<String> = types.into_iter().filter(|t| metadata.mime_types().contains(t)).collect();
        if data.is_empty() {
            return None;
        }
        let reader = SelectionReader {
            shared: Rc::downgrade(self),
            selection: self.atoms.CLIPBOARD,
            owner,
            time,
            drop_generation: None,
        };
        Some(ClipEntry::with_metadata(vec![ClipItem::from_reader(data, Rc::new(reader))], metadata))
    }

    /// `entry` on the clipboard (C++'s `writeClipboard`, all its types): the window owns CLIPBOARD, answering for
    /// it with each type the entry is offered as; none: the clipboard let go.
    fn set_clip_entry(&self, entry: Option<ClipEntry>) {
        let window = self.selection;
        let Some(entry) = entry else {
            let owned = std::mem::take(&mut self.clipboard.borrow_mut().owned);
            if owned {
                self.display.set_selection_owner(self.atoms.CLIPBOARD, NONE, self.last_time.get());
            }
            let mut c = self.clipboard.borrow_mut();
            c.payload = None;
            c.targets.clear();
            return;
        };
        let payload = Payload::of_entry(&entry); // (another app's read now: it goes once the app owns it)
        let names = payload.offered(X11_TEXT_TYPES);
        let targets: Vec<(Atom, String)> = names.into_iter().map(|n| (self.display.intern_atom(&n), n)).collect();
        self.display.set_selection_owner(self.atoms.CLIPBOARD, window, self.last_time.get());
        let mut c = self.clipboard.borrow_mut();
        c.payload = Some(payload);
        c.targets = targets;
        c.owned = window != NONE && self.display.selection_owner(self.atoms.CLIPBOARD) == window;
    }

    /// Another app asking for the clipboard the app holds (C++'s `answerSelection`): what types it can have, or one
    /// of them (in parts when it is larger than a request carries).
    fn answer_selection(&self, requestor: Window, selection: Atom, target: Atom, property: Atom, time: Time) {
        let (d, a) = (&self.display, &self.atoms);
        let into = if property != NONE { property } else { target }; // (an obsolete client's)
        let mut answered = NONE;
        let mut c = self.clipboard.borrow_mut();
        if selection == a.CLIPBOARD && c.owned {
            if target == a.TARGETS {
                let mut targets = vec![a.TARGETS as c_long];
                targets.extend(c.targets.iter().map(|t| t.0 as c_long));
                d.set_property_32(requestor, into, ffi::XA_ATOM, &targets);
                answered = into;
            } else if let Some(name) = c.target_name(target).map(str::to_string)
                && let Some(bytes) = c.payload.as_ref().and_then(|p| p.bytes(&name, X11_TEXT_TYPES))
            {
                let kind = if target == a.TEXT { a.UTF8_STRING } else { target };
                if bytes.len() > part_size(d.max_request_bytes()) {
                    d.select_input(requestor, ffi::PROPERTY_CHANGE_MASK);
                    d.set_property_32(requestor, into, a.INCR, &[bytes.len() as c_long]);
                    c.transfers.start(requestor, into, kind, bytes);
                } else {
                    d.set_property_8(requestor, into, kind, &bytes);
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
        let next = self.clipboard.borrow_mut().transfers.next_part(window, property, part_size(d.max_request_bytes()));
        let Some((part, data)) = next else { return };
        d.set_property_8(window, property, part.kind, &data[part.range]);
        if part.done {
            d.select_input(window, ffi::NO_EVENT_MASK);
        }
        d.flush();
    }

    /// What a drop from another app offers (`offered`: its types, the drag's `XdndSelection` asked for at `time`),
    /// read as it is asked for until the drop is done.
    fn drop_entry(self: &Rc<Self>, offered: &[(Atom, String)], time: Time) -> ClipEntry {
        let names: Vec<String> = offered.iter().map(|t| t.1.clone()).collect();
        let metadata = ClipMetadata::of_offered(&names);
        let reader = SelectionReader {
            shared: Rc::downgrade(self),
            selection: self.atoms.XdndSelection,
            owner: NONE,
            time,
            drop_generation: Some(self.drop_generation.get()),
        };
        ClipEntry::with_metadata(vec![ClipItem::from_reader(names, Rc::new(reader))], metadata)
    }
}

/// Reads a type of another app's selection: the clipboard while the same window owns it, or a drop's
/// `XdndSelection` until that drop is done.
struct SelectionReader {
    shared: Weak<Shared>,
    selection: Atom,
    /// The clipboard's owner when it was read (a drop's: none).
    owner: Window,
    time: Time,
    /// The drop it is of (the shared count then).
    drop_generation: Option<u64>,
}

impl ClipDataReader for SelectionReader {
    fn read(&self, mime_type: &str) -> Option<Vec<u8>> {
        let shared = self.shared.upgrade()?;
        match self.drop_generation {
            Some(g) if shared.drop_generation.get() != g => return None,
            None if shared.display.selection_owner(self.selection) != self.owner => return None,
            _ => {}
        }
        let target = shared.display.intern_atom(mime_type);
        let (bytes, kind) = shared.convert(self.selection, target, self.time)?;
        // (Text asked for by a UTF-8 name and given as Latin-1 `STRING`: made UTF-8, as the name says.)
        if kind == ffi::XA_STRING && mime_type != "STRING" && is_plain_text_type(mime_type) {
            return Some(bytes.iter().map(|&b| char::from(b)).collect::<String>().into_bytes());
        }
        Some(bytes)
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        for &c in &self.cursors.borrow().made {
            if c != NONE {
                self.display.free_cursor(c);
            }
        }
        self.display.destroy_window(self.selection);
    }
}

/// The callbacks the X11 platform gives a window's host (C++'s `attach`): waking the loop, the input method told
/// about the field taking typing (while the window has the focus), its cursor, the clipboard, links.
struct X11Callbacks {
    shared: Rc<Shared>,
    /// The window's X window (another as it is hidden and shown again).
    window: Rc<Cell<Window>>,
    /// The field its host told last: whether it takes typing, where (the window's pixels).
    field: Rc<Cell<(bool, XPoint)>>,
    /// The X window the input method's context is on (the focused one).
    ime_window: Rc<Cell<Window>>,
    /// The field's caret box (px) and text as its host told them, and its text layout while its session is open:
    /// where the spot goes (under the caret's glyph, where the layout is known).
    place: RefCell<(Rect, TextEditState, Option<PlatformTextLayout>)>,
}

impl X11Callbacks {
    /// The input method's spot for the field: under its caret (its glyph's, where its layout is known).
    fn spot(&self) -> XPoint {
        let place = self.place.borrow();
        let area = caret_area(place.0, &place.1, place.2.as_ref());
        let spot = |v: f32| v.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        XPoint { x: spot(area.left), y: spot(area.bottom) }
    }

    /// The field (taking typing or not) at `at` told the input method, if its context is on this window.
    fn tell_field(&self, active: bool, at: XPoint) {
        self.field.set((active, at));
        if self.ime_window.get() != self.window.get() {
            return; // (told it as the window gets the focus)
        }
        let mut ime = self.shared.ime.borrow_mut();
        if !active && ime.active {
            ime.reset();
        }
        ime.active = active;
        ime.spot = at;
        ime.tell();
    }
}

impl std::ops::Deref for X11Callbacks {
    type Target = Rc<Shared>;
    fn deref(&self) -> &Rc<Shared> {
        &self.shared
    }
}

impl PlatformCallbacks for X11Callbacks {
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        Some(self.shared.waker.as_fn())
    }

    /// The input method's context focused while a field takes typing, what is composed shown under the field's
    /// caret (`area`, px: the window's pixels); what it composed for a field that no longer takes typing given up.
    fn text_input(&self, active: bool, area: Rect, state: &TextEditState, _: &ImeOptions) {
        {
            let mut place = self.place.borrow_mut();
            (place.0, place.1) = (area, state.clone());
            if !active {
                place.2 = None; // (the session closed: its layout goes with it)
            }
        }
        self.tell_field(active, self.spot());
    }

    /// The focused field was laid out again or moved: the spot goes under its caret's glyph.
    fn update_text_layout_result(&self, layout: &PlatformTextLayout) {
        self.place.borrow_mut().2 = Some(layout.clone());
        if self.field.get().0 {
            self.tell_field(true, self.spot());
        }
    }

    /// The app changed the field's text: what the input method composes on the spot given up unless the field
    /// still holds it as its composition; the spot follows the caret.
    fn text_input_changed(&self, state: &TextEditState) {
        self.place.borrow_mut().1 = state.clone();
        if self.field.get().0 {
            self.tell_field(true, self.spot());
        }
        if self.ime_window.get() != self.window.get() {
            return;
        }
        let mut ime = self.shared.ime.borrow_mut();
        if !ime.composing {
            return;
        }
        let held = state.composition.and_then(|(a, b)| state.text.get(a.min(b)..a.max(b)));
        if held != Some(ime.method.preedit_text().as_str()) {
            ime.reset();
        }
    }

    fn cursor(&self, icon: PointerIcon) {
        let window = self.window.get();
        self.shared.cursors.borrow_mut().icons.insert(window, icon);
        self.shared.show_cursor(window);
    }

    fn get_clip_entry(&self) -> Option<ClipEntry> {
        self.shared.clip_entry()
    }

    fn set_clip_entry(&self, entry: Option<ClipEntry>) {
        self.shared.set_clip_entry(entry);
    }

    fn open_uri(&self, uri: &str) {
        open_uri(uri);
    }

    /// Taken where the window is open and the pointer known, for a source allowing an action; started (XDND's
    /// source) once the event is handled.
    fn start_drag_and_drop(&self, drag: &PlatformDragAndDrop) -> bool {
        let s = &self.shared;
        if !s.can_drag.get() || s.dragging.get() || drag.transfer_data.supported_actions.is_empty() {
            return false;
        }
        *s.drag_request.borrow_mut() = Some(drag.clone());
        true
    }
}

/// A window's state on the connection (the main window's, a `Window`'s, the app's composition's without one): its X
/// window (none while it has none), title, size, place, refresh rate, how the window manager placed it, its events
/// for its core.
struct XWindowData {
    id: WindowId,
    /// Its X window, as its host's callbacks read it (none: not open).
    x: Rc<Cell<Window>>,
    events: VecDeque<Event>,
    title: String,
    /// The window's size, its pixels (what the swapchain is made at).
    size: Rc<Cell<(i32, i32)>>,
    /// Where it is on the screen.
    position: (i32, i32),
    refresh_rate: f32,
    /// As `_NET_WM_STATE` last said: maximized, full screen, hidden (minimized).
    placement: WindowPlacement,
    minimized: bool,
    /// Its host's field (the input method told it while the window has the focus).
    field: Rc<Cell<(bool, XPoint)>>,
    /// A transparent window's colormap (its 32-bit visual's), freed after it; NONE: the root's visual.
    colormap: ffi::Colormap,
}

impl XWindowData {
    fn new(id: WindowId) -> XWindowData {
        XWindowData {
            id,
            x: Rc::new(Cell::new(NONE)),
            events: VecDeque::new(),
            title: "KompozR".into(),
            size: Rc::new(Cell::new((1, 1))),
            position: (0, 0),
            refresh_rate: 60.0,
            placement: WindowPlacement::Floating,
            minimized: false,
            field: Rc::new(Cell::new((false, XPoint::default()))),
            colormap: NONE,
        }
    }
}

/// Where an event of the platform's own goes: the app's root core, every window's, the focused window's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Route {
    App,
    All,
    Focused,
}

/// The X11 platform (C++'s X11Platform): the display, the app's windows, the loop, the input as KompozR's events.
pub struct X11Platform {
    shared: Rc<Shared>,
    event_loop: EventLoop,
    /// The window worked on, and the others.
    win: XWindowData,
    parked: Vec<XWindowData>,
    /// The root core's id (the main window's, or the app's composition's).
    app_id: WindowId,
    /// The window with the keyboard's focus.
    focused: Option<WindowId>,
    /// The X window the input method's context is on.
    ime_window: Rc<Cell<Window>>,
    /// Events of the platform's own, for the cores they go to.
    routed: VecDeque<(Route, Event)>,
    desktop_started: bool,
    /// The desktop's scale (Xft.dpi / 96).
    dpi_scale: f64,
    input: Input,
    keyboard: X11Keyboard,
    drag: XdndTarget,
    /// What the last drop carries, until the core takes it (`take_drop_entry`).
    dropped: Option<ClipEntry>,
    /// The action the window last took a drag over it with, and a drop's finish to send once the core says
    /// whether it was taken (its source, the action).
    drop_action: Atom,
    pending_finish: Option<(Window, Atom)>,
    /// Where the pointer (or a finger) last was: what a drag of the app's own follows.
    pointer: Option<PointerAt>,
    /// A drag of the app's own out to other apps.
    outgoing: Option<OutgoingDrag>,
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
        let selection = display.create_window(1, 1, ffi::PROPERTY_CHANGE_MASK).ok_or("no window for the clipboard")?;
        let shared = Rc::new(Shared {
            atoms,
            window: Cell::new(NONE),
            selection,
            last_time: Cell::new(ffi::CURRENT_TIME),
            clipboard: RefCell::new(Clipboard::default()),
            atom_names: RefCell::new(HashMap::new()),
            drop_generation: Cell::new(0),
            cursors: RefCell::new(Cursors { made: [NONE; 9], icons: HashMap::new() }),
            ime: RefCell::new(Ime {
                method: InputMethod::new(display.clone()),
                active: false,
                focused: false,
                spot: XPoint::default(),
                composing: false,
            }),
            waker: event_loop.waker(),
            display,
            drag_request: RefCell::new(None),
            can_drag: Cell::new(false),
            dragging: Cell::new(false),
        });
        let mut platform = X11Platform {
            shared,
            event_loop,
            win: XWindowData::new(WindowId::MAIN),
            parked: Vec::new(),
            app_id: WindowId::MAIN,
            focused: None,
            ime_window: Rc::new(Cell::new(NONE)),
            routed: VecDeque::new(),
            desktop_started: false,
            dpi_scale: 1.0,
            input,
            keyboard,
            drag: XdndTarget::default(),
            dropped: None,
            drop_action: NONE,
            pending_finish: None,
            pointer: None,
            outgoing: None,
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
        let (w, h) = self.win.size.get();
        let (x, y) = self.win.position;
        Rect::new(x as f32, y as f32, (x + w) as f32, (y + h) as f32)
    }

    /// The events gathered since, each for its window; the platform's own to the cores they go to.
    fn take_events(&mut self) -> Vec<(WindowId, Event)> {
        let mut out = Vec::new();
        let ids = self.window_ids();
        for (route, e) in std::mem::take(&mut self.routed) {
            match route {
                Route::App => out.push((self.app_id, e)),
                Route::Focused => out.push((self.focused.unwrap_or(self.app_id), e)),
                Route::All => out.extend(ids.iter().map(|&id| (id, e.clone()))),
            }
        }
        for w in std::iter::once(&mut self.win).chain(self.parked.iter_mut()) {
            out.extend(w.events.drain(..).map(|e| (w.id, e)));
        }
        out
    }

    /// The ids of the windows the platform has, the root's among them.
    fn window_ids(&self) -> Vec<WindowId> {
        std::iter::once(self.win.id).chain(self.parked.iter().map(|w| w.id)).collect()
    }

    /// Window `id` worked on from here (its state the platform's `win`; the one before parked): made if it is new.
    pub(crate) fn select(&mut self, id: WindowId) {
        if self.win.id != id {
            let data = match self.parked.iter().position(|w| w.id == id) {
                Some(i) => self.parked.swap_remove(i),
                None => XWindowData::new(id),
            };
            let before = std::mem::replace(&mut self.win, data);
            self.parked.push(before);
        }
        self.shared.window.set(self.win.x.get());
    }

    /// The window whose X window is `x`.
    fn window_of(&self, x: Window) -> Option<WindowId> {
        if x == NONE {
            return None;
        }
        std::iter::once(&self.win).chain(self.parked.iter()).find(|w| w.x.get() == x).map(|w| w.id)
    }

    /// Window `id`'s state gone (its handle dropped: its window closed before).
    fn forget_window(&mut self, id: WindowId) {
        if self.win.id == id {
            self.win = self.parked.pop().unwrap_or_else(|| XWindowData::new(self.app_id));
            self.shared.window.set(self.win.x.get());
        } else {
            self.parked.retain(|w| w.id != id);
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
    }

    /// The input method's context on the focused window's X window (made afresh as the focus moves between the
    /// app's windows), told that window's field.
    fn ime_to(&mut self, x: Window) {
        if self.ime_window.get() == x {
            return;
        }
        self.ime_window.set(x);
        let (active, spot) = self.win.field.get();
        let mut ime = self.shared.ime.borrow_mut();
        ime.method.destroy_context();
        ime.active = active;
        ime.spot = spot;
        if ime.method.open() {
            ime.method.create_context(x, spot);
        }
        ime.tell();
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
            if !self.display().filter_event(&mut e) {
                self.handle(&e);
            }
            // (What the input method composes, told in its place among the events: its callbacks ran in the filter
            // or the key's lookup.)
            let preedit = self.shared.ime.borrow_mut().preedit();
            self.routed.extend(preedit.into_iter().map(|e| (Route::Focused, e)));
        }
        let window = Some(self.ime_window.get()).filter(|&w| w != NONE);
        let mut ime = self.shared.ime.borrow_mut();
        let spot = ime.spot;
        if ime.method.update(window, spot) {
            ime.tell();
        }
        let preedit = ime.preedit(); // (a method gone takes what it composed with it)
        drop(ime);
        self.routed.extend(preedit.into_iter().map(|e| (Route::Focused, e)));
        // What the desktop's services told: to all the windows (an assistive technology's requests: each window takes
        // those for its own nodes), the tray's to the app's core.
        for e in self.desktop.take_events() {
            let route = match e {
                Event::TrayAction { .. } | Event::TrayMenuItem { .. } | Event::MenuBarItem { .. } => Route::App,
                _ => Route::All,
            };
            self.routed.push_back((route, e));
        }
        self.drag_timeouts();
    }

    /// Up to `timeout` (none: until one) for an X event, a wake, the deadline or a signal (C++'s `waitForEvents`).
    /// False if the connection broke.
    fn wait(&mut self, timeout: Option<Duration>) -> bool {
        self.display().flush();
        if self.display().pending() > 0 {
            return true;
        }
        // (A drag's target not answering: woken when it is given up on.)
        let timeout = match self.drag_deadline() {
            Some(at) => {
                let left = Duration::from_nanos((at - sys::monotonic_now()).max(0) as u64);
                Some(timeout.map_or(left, |t| t.min(left)))
            }
            None => timeout,
        };
        let Ok(ready) = self.event_loop.wait(timeout) else { return false };
        if ready.woken || ready.deadline {
            self.routed.push_back((Route::All, Event::Wake));
        }
        if self.event_loop.take_quit_signal() {
            self.routed.push_back((Route::App, Event::Quit));
        }
        !(ready.source(DISPLAY_SOURCE) && self.display().connection_closed())
    }

    /// An event from the X server, or for it to answer (C++'s `handle`).
    fn handle(&mut self, e: &XEvent) {
        if let Some((xi, window)) = self.display().xi_event_in(e) {
            if let Some(id) = self.window_of(window) {
                self.select(id);
            }
            if self.outgoing.as_ref().is_some_and(|o| o.is_dragging()) && xi != XiEvent::Hierarchy {
                return self.drag_input(&xi); // (the drag has the pointer: the app none of it)
            }
            return self.xi_input(xi);
        }
        // (The window it is for: its own, a drag's target, a requestor of the clipboard is another's.)
        if let Some(id) = self.window_of(e.window()) {
            self.select(id);
        }
        if self.display().is_randr_event(e) {
            return self.displays_changed();
        }
        let a = self.shared.atoms;
        match e.decode() {
            Decoded::Key(key) => {
                self.shared.last_time.set(key.time());
                let keysym = self.display().lookup_keysym(&key, 0);
                if let Some(p) = &mut self.pointer {
                    p.mods = drag::modifiers_after(key.state(), keysym, key.is_press()); // (what a drag starts with)
                }
                if self.outgoing.as_ref().is_some_and(|o| o.is_dragging())
                    && !self.drag_key(keysym, key.state(), key.is_press())
                {
                    return;
                }
                let ime = self.shared.ime.borrow();
                let events =
                    self.keyboard.key(&self.shared.display, &ime.method, &key, sys::monotonic_now(), ime.active);
                drop(ime);
                self.win.events.extend(events);
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
                if focus_in {
                    self.focused = Some(self.win.id);
                    let x = self.win.x.get();
                    self.ime_to(x);
                } else if self.focused == Some(self.win.id) {
                    self.focused = None;
                }
                {
                    let mut ime = self.shared.ime.borrow_mut();
                    ime.focused = self.focused.is_some();
                    ime.tell();
                }
                if !focus_in {
                    self.keyboard.leave();
                }
                self.win.events.push_back(if focus_in { Event::FocusGained } else { Event::FocusLost });
            }
            Decoded::Configure { window, width, height, x, y, send_event } => {
                if window != self.window() {
                    return;
                }
                if (width, height) != self.win.size.get() {
                    self.win.size.set((width, height));
                    self.win.events.push_back(Event::Resized);
                }
                self.moved(x, y, send_event);
            }
            Decoded::Expose { count } => {
                if count == 0 {
                    self.win.events.push_back(Event::Exposed);
                }
            }
            Decoded::Property { window, atom, state } => {
                if window == self.display().root() && atom == a.RESOURCE_MANAGER {
                    let s = self.desktop_scale();
                    if s != self.dpi_scale {
                        self.dpi_scale = s;
                        self.routed.push_back((Route::All, Event::MetricsChanged));
                    }
                } else if window == self.window() && atom == a._NET_WM_STATE {
                    self.wm_state_changed();
                } else if state == ffi::PROPERTY_DELETE && !self.send_next_drag_part(window, atom) {
                    self.shared.send_next_part(window, atom);
                }
            }
            Decoded::ClientMessage { message_type, data, .. } => self.message(message_type, data),
            Decoded::SelectionRequest { requestor, selection, target, property, time } => {
                if selection == a.XdndSelection && self.answer_drag_selection(requestor, target, property, time) {
                    return;
                }
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
            if let Some(at) = PointerAt::of(d) {
                self.pointer = Some(at);
                self.shared.can_drag.set(self.window() != NONE);
            }
        }
        if e == XiEvent::Hierarchy {
            self.input.devices = Devices::new(&self.display().xi_devices());
            self.win.events.push_back(Event::DevicesChanged);
            return;
        }
        let events = self.input.events(&e, sys::monotonic_now());
        self.win.events.extend(events);
    }

    // ---- Sizes and displays ----

    /// Where the window is on the screen (a window manager's own configure tells it; else asked), and the refresh
    /// rate of the display it is on (C++'s `moved`).
    fn moved(&mut self, x: i32, y: i32, send_event: bool) {
        self.win.position = if send_event {
            (x, y)
        } else {
            self.display()
                .translate_coordinates(self.window(), self.display().root(), 0, 0)
                .unwrap_or(self.win.position)
        };
        self.displays_changed();
    }

    /// The refresh rate of the display under the window's middle (XRandR's mode on its CRTC), told if it changed.
    fn displays_changed(&mut self) {
        let (w, h) = self.win.size.get();
        let (x, y) = self.win.position;
        let rate = metrics::refresh_rate_at(&self.display().crtcs(), x + w / 2, y + h / 2);
        if rate != self.win.refresh_rate {
            self.win.refresh_rate = rate;
            self.win.events.push_back(Event::MetricsChanged);
        }
    }

    // ---- The window manager's messages and drags (XDND) ----

    fn message(&mut self, kind: Atom, data: [c_long; 5]) {
        let a = self.shared.atoms;
        let d = self.shared.display.clone();
        if kind == a.WM_PROTOCOLS {
            let what = data[0] as Atom;
            if what == a.WM_DELETE_WINDOW {
                self.win.events.push_back(Event::CloseRequest);
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
        if kind == a.XdndStatus || kind == a.XdndFinished {
            // (A target answering the app's own drag.)
            return self.drag_reply(kind, data);
        }
        if kind == a.XdndEnter {
            // The types it offers (three in the message, or its XdndTypeList), with their names.
            let list = XdndTarget::has_type_list(&data)
                .then(|| d.property(data[0] as Window, a.XdndTypeList, false).map(|p| p.longs()).unwrap_or_default());
            let offered = XdndTarget::offered_atoms(&data, list.as_deref())
                .into_iter()
                .filter_map(|t| Some((t, self.shared.atom_name(t)?)))
                .collect();
            self.drag.enter(&data, offered, self.window());
        } else if kind == a.XdndPosition {
            let Some((root_x, root_y)) = self.drag.position_on_screen(&data) else { return };
            let (x, y) = d
                .translate_coordinates(d.root(), self.window(), root_x, root_y)
                .unwrap_or((root_x - self.win.position.0, root_y - self.win.position.1));
            // (Another app's drag taken as a copy; the app's own as asked: Move, Ctrl Copy, Ctrl+Shift Link.)
            let asked = data[4] as Atom;
            let own_action = [a.XdndActionCopy, a.XdndActionMove, a.XdndActionLink].contains(&asked);
            let action = match self.drag.is_own() {
                true if own_action => asked,
                true => NONE,
                false => a.XdndActionCopy,
            };
            let (events, flags, action) = self.drag.moved(x, y, action);
            self.win.events.extend(events);
            self.drop_action = action;
            self.reply_to(self.drag.source(), a.XdndStatus, flags, action);
        } else if kind == a.XdndLeave {
            let events = self.drag.leave(&data);
            self.win.events.extend(events);
        } else if kind == a.XdndDrop {
            // (What it carries read from its source as a target asks for it, until the core says it is done.)
            let Some(time) = self.drag.drop_time(&data) else { return };
            self.dropped = time.map(|t| self.shared.drop_entry(self.drag.offered(), t));
            let source = self.drag.source();
            let events = self.drag.dropped(self.dropped.is_some());
            self.win.events.extend(events);
            // (Finished once the core says whether a target took it: `drop_done`.)
            if let Some((old, _)) = self.pending_finish.replace((source, self.drop_action)) {
                self.reply_to(old, a.XdndFinished, 0, NONE);
            }
        }
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

impl X11Platform {
    /// How the window manager has the window now (`_NET_WM_STATE`): maximized, full screen, hidden (minimized),
    /// each change told (C++'s window state read back).
    fn wm_state_changed(&mut self) {
        let a = self.shared.atoms;
        let atoms: Vec<Atom> = self
            .display()
            .property(self.window(), a._NET_WM_STATE, false)
            .map(|p| p.longs().into_iter().map(|l| l as Atom).collect())
            .unwrap_or_default();
        let has = |atom: Atom| atoms.contains(&atom);
        let placement = if has(a._NET_WM_STATE_FULLSCREEN) {
            WindowPlacement::Fullscreen
        } else if has(a._NET_WM_STATE_MAXIMIZED_VERT) && has(a._NET_WM_STATE_MAXIMIZED_HORZ) {
            WindowPlacement::Maximized
        } else {
            WindowPlacement::Floating
        };
        let minimized = has(a._NET_WM_STATE_HIDDEN);
        if placement != self.win.placement {
            self.win.placement = placement;
            self.win.events.push_back(Event::MetricsChanged);
        }
        if minimized != self.win.minimized {
            self.win.minimized = minimized;
            self.win.events.push_back(if minimized { Event::Minimized } else { Event::Restored });
        }
    }

    /// A `_NET_WM_STATE` change asked of the window manager (EWMH: 1 adds, 0 removes the states).
    fn ask_wm_state(&self, add: bool, first: Atom, second: Atom) {
        let (d, a) = (self.display(), self.shared.atoms);
        let data = [add as c_long, first as c_long, second as c_long, 1, 0];
        let root = d.root();
        d.send_client_message(
            root,
            self.window(),
            a._NET_WM_STATE,
            data,
            ffi::SUBSTRUCTURE_NOTIFY_MASK | ffi::SUBSTRUCTURE_REDIRECT_MASK,
        );
        d.flush();
    }

    /// The window's size bounds (ICCCM's WM_NORMAL_HINTS): its size now, or none (resizable).
    fn size_hints(&self, resizable: bool) {
        let (w, h) = self.win.size.get();
        // (flags, x, y, width, height, min w/h, max w/h, increments, aspects, base w/h, gravity)
        let mut hints = [0 as c_long; 18];
        if !resizable {
            hints[0] = 16 | 32; // (PMinSize | PMaxSize)
            hints[5] = w as c_long;
            hints[6] = h as c_long;
            hints[7] = w as c_long;
            hints[8] = h as c_long;
        }
        self.display().set_property_32(self.window(), ffi::XA_WM_NORMAL_HINTS, ffi::XA_WM_SIZE_HINTS, &hints);
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
    /// the core ones, which the input method filters); where it goes, how it is placed, whether it is decorated,
    /// takes the focus, may be resized, stays on top; then shown.
    fn open_window(&mut self, app: &WindowSpec, backend: Backend) -> Result<Surface, String> {
        if backend != Backend::Vulkan {
            return Err(format!("no {} on X11", backend.name()));
        }
        self.win.title = app.title.clone();
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
        // A transparent window: of the screen's 32-bit visual (what a compositing manager shows with its alpha);
        // without one, opaque.
        let argb = if app.transparent { d.create_argb_window(width as u32, height as u32, events) } else { None };
        if app.transparent && argb.is_none() {
            log("kompoz: X11: no 32-bit visual: the transparent window is opaque");
        }
        let window = match argb {
            Some((window, colormap)) => {
                self.win.colormap = colormap;
                window
            }
            None => d.create_window(width as u32, height as u32, events).ok_or("the window couldn't be made")?,
        };
        self.win.x.set(window);
        self.shared.window.set(window);
        self.win.size.set((width, height));
        d.set_property_8(window, a._NET_WM_NAME, a.UTF8_STRING, self.win.title.as_bytes());
        // (Older tools read a Latin-1 one.)
        let name_type = if self.win.title.is_ascii() { ffi::XA_STRING } else { a.UTF8_STRING };
        d.set_property_8(window, ffi::XA_WM_NAME, name_type, self.win.title.as_bytes());
        d.set_class_hint(window, c"dev.kompoz.app", c"dev.kompoz.app");
        d.set_property_32(window, a._NET_WM_PID, ffi::XA_CARDINAL, &[std::process::id() as c_long]);
        d.set_wm_protocols(window, &[a.WM_DELETE_WINDOW, a._NET_WM_PING]);
        d.set_property_32(window, a._NET_WM_WINDOW_TYPE, ffi::XA_ATOM, &[a._NET_WM_WINDOW_TYPE_NORMAL as c_long]);
        d.set_property_32(window, a.XdndAware, ffi::XA_ATOM, &[5]);
        if app.undecorated {
            // (Motif's hints: the decorations field, none.)
            d.set_property_32(window, a._MOTIF_WM_HINTS, a._MOTIF_WM_HINTS, &[2, 0, 0, 0, 0]);
        }
        if !app.focusable {
            // (ICCCM's WM_HINTS: InputHint, no input; the window manager gives it no focus.)
            d.set_property_32(window, ffi::XA_WM_HINTS, ffi::XA_WM_HINTS, &[1, 0, 0, 0, 0, 0, 0, 0, 0]);
        }
        // States the window maps with (EWMH: set before mapping).
        let mut states = Vec::new();
        match app.placement {
            WindowPlacement::Maximized => {
                states.extend([a._NET_WM_STATE_MAXIMIZED_VERT as c_long, a._NET_WM_STATE_MAXIMIZED_HORZ as c_long])
            }
            WindowPlacement::Fullscreen => states.push(a._NET_WM_STATE_FULLSCREEN as c_long),
            WindowPlacement::Floating => {}
        }
        if app.always_on_top {
            states.push(a._NET_WM_STATE_ABOVE as c_long);
        }
        if app.minimized {
            states.push(a._NET_WM_STATE_HIDDEN as c_long);
        }
        if !states.is_empty() {
            d.set_property_32(window, a._NET_WM_STATE, ffi::XA_ATOM, &states);
        }
        if !app.resizable {
            self.size_hints(false);
        }
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
        if self.ime_window.get() == NONE {
            // (The input method's context on the first window; it moves with the focus.)
            self.ime_to(window);
        }
        self.shared.show_cursor(window);
        if let Some((x, y)) = app.position {
            let (x, y) = ((x.0 as f64 * scale).round() as i32, (y.0 as f64 * scale).round() as i32);
            d.move_window(window, x, y);
            self.win.position = (x, y);
        }
        d.map_window(window);
        if app.minimized {
            self.minimize();
        }
        d.flush();
        self.win.events.push_back(Event::Shown);
        let size = self.win.size.clone();
        Ok(Surface {
            vulkan: Some(vulkan_surface(&d, window)),
            drawable_size: Some(Box::new(move || size.get())),
            pipeline_cache: None,
            transparent: self.win.colormap != NONE,
        })
    }

    fn close_window(&mut self) {
        if self.outgoing.is_some() {
            self.cancel_outgoing_drag();
        }
        self.shared.can_drag.set(false);
        let window = self.shared.window.replace(NONE);
        self.win.x.set(NONE);
        self.desktop.window_closed(self.win.id); // (its tree gone from the screen readers)
        if window != NONE {
            if self.ime_window.get() == window {
                self.ime_window.set(NONE);
                self.shared.ime.borrow_mut().method.destroy_context();
            }
            self.shared.cursors.borrow_mut().icons.remove(&window);
            self.display().destroy_window(window);
            let colormap = std::mem::replace(&mut self.win.colormap, NONE);
            self.display().free_colormap(colormap);
            self.display().flush();
        }
    }

    fn show_error(&mut self, message: &str) {
        log(message);
    }

    /// Points are the window's pixels; the scale is the desktop's (Xft.dpi / 96), the text size the portal's;
    /// `KOMPOZ_SCALE` and `KOMPOZ_FONT_SCALE` stand in for them.
    fn metrics(&self) -> Metrics {
        let (w, h) = self.win.size.get();
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
            refresh_rate: self.win.refresh_rate,
            safe_area: Some(Rect::new(0.0, 0.0, w as f32, h as f32)),
            dark_theme: self.desktop.dark_theme(),
            language: preferred_language().unwrap_or_default(),
            title_bar: None, // (the window manager decorates the window)
        }
    }

    fn pixel_size(&self) -> (i32, i32) {
        self.win.size.get()
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

    /// The window's host's callbacks; the desktop's services started with the first, waking the loop.
    fn attach(&mut self, _: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        if !self.desktop_started {
            self.desktop_started = true;
            let title = self.win.title.clone();
            self.desktop.start(&title, self.event_loop.waker().as_fn());
        }
        Rc::new(X11Callbacks {
            shared: self.shared.clone(),
            window: self.win.x.clone(),
            field: self.win.field.clone(),
            ime_window: self.ime_window.clone(),
            place: RefCell::new((Rect::Zero, TextEditState::default(), None)),
        })
    }

    fn metrics_updated(&mut self, _: &KompozHost) {
        self.desktop.metrics_updated();
    }

    fn handled(&mut self, host: &KompozHost, event: &Event) {
        self.desktop.handled(self.win.id, host, event);
        self.start_outgoing_drag(); // (one the host asked for as it handled the event)
    }

    /// The drop's finish sent to its source: taken (with the action the window took it with) or refused.
    fn take_drop_entry(&mut self) -> Option<ClipEntry> {
        self.dropped.take()
    }

    fn drop_done(&mut self, taken: bool) {
        // (What the drop offered can't be read from here on.)
        self.shared.drop_generation.set(self.shared.drop_generation.get() + 1);
        let Some((source, action)) = self.pending_finish.take() else { return };
        let a = self.shared.atoms;
        if taken && action != NONE {
            self.reply_to(source, a.XdndFinished, 1, action);
        } else {
            self.reply_to(source, a.XdndFinished, 0, NONE);
        }
    }

    /// The screen saver held off while the app keeps the screen on (D-Bus: X11 has no inhibitor of its own; any of
    /// its windows); the window's semantics tree published to the screen readers (each open window's, a frame of
    /// the app's each), where it is on the screen.
    fn composed(&mut self, host: &KompozHost, redraw: bool, frame_time: i64) {
        let title = self.win.title.clone();
        if self.win.x.get() == NONE {
            return;
        }
        self.desktop.keep_screen_on(host.keep_screen_on(), &title);
        let bounds = self.window_bounds();
        self.desktop.composed(self.win.id, host, redraw, frame_time, bounds, 1.0, &title);
    }

    fn wants_frames(&self) -> bool {
        self.win.x.get() != NONE && self.desktop.wants_frames(self.win.id)
    }

    fn set_title(&mut self, title: &str) {
        self.win.title = title.to_string();
        let window = self.window();
        if window == NONE {
            return;
        }
        let (d, a) = (self.display(), self.shared.atoms);
        d.set_property_8(window, a._NET_WM_NAME, a.UTF8_STRING, title.as_bytes());
        let name_type = if title.is_ascii() { ffi::XA_STRING } else { a.UTF8_STRING };
        d.set_property_8(window, ffi::XA_WM_NAME, name_type, title.as_bytes());
        d.flush();
    }

    /// Maximized, full screen or neither, asked of the window manager (EWMH's `_NET_WM_STATE`).
    fn set_placement(&mut self, placement: WindowPlacement) {
        if self.window() == NONE {
            return;
        }
        let a = self.shared.atoms;
        let (vert, horz, full) =
            (a._NET_WM_STATE_MAXIMIZED_VERT, a._NET_WM_STATE_MAXIMIZED_HORZ, a._NET_WM_STATE_FULLSCREEN);
        match placement {
            WindowPlacement::Floating => {
                self.ask_wm_state(false, full, NONE);
                self.ask_wm_state(false, vert, horz);
            }
            WindowPlacement::Maximized => {
                self.ask_wm_state(false, full, NONE);
                self.ask_wm_state(true, vert, horz);
            }
            WindowPlacement::Fullscreen => self.ask_wm_state(true, full, NONE),
        }
    }

    /// Minimized (ICCCM's WM_CHANGE_STATE to IconicState, as XIconifyWindow asks), or mapped again and brought
    /// forward.
    fn set_minimized(&mut self, minimized: bool) {
        if self.window() == NONE {
            return;
        }
        if minimized {
            self.minimize();
        } else {
            self.display().map_window(self.window());
            self.to_front();
        }
    }

    fn set_position(&mut self, x: i32, y: i32) {
        if self.window() != NONE {
            self.display().move_window(self.window(), x, y);
            self.display().flush();
        }
    }

    /// To the front with the focus: EWMH's `_NET_ACTIVE_WINDOW` (from an application: source 1), the last input's
    /// time.
    fn to_front(&mut self) {
        let window = self.window();
        if window == NONE {
            return;
        }
        let (d, a) = (self.display(), self.shared.atoms);
        let data = [1, self.shared.last_time.get() as c_long, 0, 0, 0];
        let root = d.root();
        let mask = ffi::SUBSTRUCTURE_NOTIFY_MASK | ffi::SUBSTRUCTURE_REDIRECT_MASK;
        d.send_client_message(root, window, a._NET_ACTIVE_WINDOW, data, mask);
        d.flush();
    }

    fn set_resizable(&mut self, resizable: bool) {
        if self.window() != NONE {
            self.size_hints(resizable);
        }
    }

    fn set_always_on_top(&mut self, on_top: bool) {
        if self.window() != NONE {
            self.ask_wm_state(on_top, self.shared.atoms._NET_WM_STATE_ABOVE, NONE);
        }
    }

    /// EWMH's `_NET_WM_ICON`: its width, height, then ARGB pixels (a CARDINAL each); none: deleted.
    fn set_icon(&mut self, icon: Option<&TrayImage>) {
        let window = self.window();
        if window == NONE {
            return;
        }
        let (d, a) = (self.display(), self.shared.atoms);
        match icon {
            Some(i) => {
                let mut data = vec![i.width as c_long, i.height as c_long];
                data.extend(
                    i.rgba.as_chunks::<4>().0.iter().map(|p| {
                        ((p[3] as u32) << 24 | (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32) as c_long
                    }),
                );
                d.set_property_32(window, a._NET_WM_ICON, ffi::XA_CARDINAL, &data);
            }
            None => d.delete_property(window, a._NET_WM_ICON),
        }
        d.flush();
    }

    /// Exported to the desktop's global menu where there is one (the registrar given the window's id; KDE's
    /// window properties naming where its menus are, as Qt's apps set them), else drawn by KompozR.
    fn set_menu_bar(&mut self, menus: Option<&[TrayMenuItem]>) -> MenuBarShown {
        let window = self.window();
        let id = self.win.id;
        let shown = self.desktop.set_window_menu(id, menus, (window != NONE).then_some(window as u32), window != NONE);
        if window != NONE {
            let (d, a) = (self.display(), self.shared.atoms);
            match self.desktop.window_menu_address(id).filter(|_| shown == MenuBarShown::Exported) {
                Some((name, path)) => {
                    d.set_property_8(window, a._KDE_NET_WM_APPMENU_SERVICE_NAME, a.UTF8_STRING, name.as_bytes());
                    d.set_property_8(window, a._KDE_NET_WM_APPMENU_OBJECT_PATH, a.UTF8_STRING, path.as_bytes());
                }
                None => {
                    d.delete_property(window, a._KDE_NET_WM_APPMENU_SERVICE_NAME);
                    d.delete_property(window, a._KDE_NET_WM_APPMENU_OBJECT_PATH);
                }
            }
            d.flush();
        }
        shown
    }

    fn window_position(&self) -> Option<(i32, i32)> {
        (self.window() != NONE).then_some(self.win.position)
    }

    fn window_placement(&self) -> Option<WindowPlacement> {
        (self.window() != NONE).then_some(self.win.placement)
    }

    /// The root window's size (the screen's, all of its outputs).
    fn screen_size(&self) -> Option<(i32, i32)> {
        let g = self.display().geometry(self.display().root())?;
        Some((g.width, g.height))
    }

    fn tray_supported(&self) -> bool {
        self.desktop.tray_supported()
    }

    fn sync_trays(&mut self, trays: &[TrayIcon]) {
        self.desktop.sync_trays(trays);
    }

    fn send_notification(&mut self, tray: &TrayIcon, notification: &Notification) {
        self.desktop.send_notification(tray, notification);
    }
}

impl X11Platform {
    /// Minimized as XIconifyWindow does it: ICCCM's WM_CHANGE_STATE (IconicState) to the root.
    fn minimize(&self) {
        let (d, a) = (self.display(), self.shared.atoms);
        let root = d.root();
        let mask = ffi::SUBSTRUCTURE_NOTIFY_MASK | ffi::SUBSTRUCTURE_REDIRECT_MASK;
        d.send_client_message(root, self.window(), a.WM_CHANGE_STATE, [3, 0, 0, 0, 0], mask);
        d.flush();
    }

    /// A window made by a test, not shown, as the platform's own (the clipboard's and drops' tests).
    #[cfg(test)]
    fn adopt(&mut self, window: Window) {
        self.win.x.set(window);
        self.shared.window.set(window);
    }
}

impl Drop for X11Platform {
    /// The desktop's services' threads ended, then the windows closed (C++'s destructor).
    fn drop(&mut self) {
        self.desktop.stop();
        for id in self.window_ids() {
            self.select(id);
            self.close_window();
        }
    }
}

/// A window of the app's on the X connection (the root core's, or a `Window`'s): the platform it shares, by the
/// window's id; each call works on its window.
pub struct X11Window {
    platform: Rc<RefCell<X11Platform>>,
    id: WindowId,
}

impl X11Window {
    fn new(platform: &Rc<RefCell<X11Platform>>, id: WindowId) -> X11Window {
        platform.borrow_mut().select(id);
        X11Window { platform: platform.clone(), id }
    }

    /// The platform, its window selected.
    fn on(&self) -> std::cell::RefMut<'_, X11Platform> {
        let mut p = self.platform.borrow_mut();
        p.select(self.id);
        p
    }
}

impl Drop for X11Window {
    fn drop(&mut self) {
        if let Ok(mut p) = self.platform.try_borrow_mut() {
            p.select(self.id);
            p.close_window();
            p.forget_window(self.id);
        }
    }
}

impl Platform for X11Window {
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
    fn open_window(&mut self, window: &WindowSpec, backend: Backend) -> Result<Surface, String> {
        self.on().open_window(window, backend)
    }
    fn close_window(&mut self) {
        self.on().close_window();
    }
    fn show_error(&mut self, message: &str) {
        log(message);
    }
    fn metrics(&self) -> Metrics {
        self.on().metrics()
    }
    fn pixel_size(&self) -> (i32, i32) {
        self.on().pixel_size()
    }
    fn set_window_size(&mut self, width: i32, height: i32) {
        self.on().set_window_size(width, height);
    }
    fn media(&mut self, probe: bool) -> MediaInfo {
        self.on().media(probe)
    }
    fn attach(&mut self, host: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        self.on().attach(host)
    }
    fn metrics_updated(&mut self, host: &KompozHost) {
        self.on().metrics_updated(host);
    }
    fn handled(&mut self, host: &KompozHost, event: &Event) {
        self.on().handled(host, event);
    }
    fn composed(&mut self, host: &KompozHost, redraw: bool, frame_time: i64) {
        self.on().composed(host, redraw, frame_time);
    }
    fn take_drop_entry(&mut self) -> Option<ClipEntry> {
        self.on().take_drop_entry()
    }
    fn drop_done(&mut self, taken: bool) {
        self.on().drop_done(taken);
    }
    fn wants_frames(&self) -> bool {
        self.on().wants_frames()
    }
    fn set_title(&mut self, title: &str) {
        self.on().set_title(title);
    }
    fn set_placement(&mut self, placement: WindowPlacement) {
        self.on().set_placement(placement);
    }
    fn set_minimized(&mut self, minimized: bool) {
        self.on().set_minimized(minimized);
    }
    fn set_position(&mut self, x: i32, y: i32) {
        self.on().set_position(x, y);
    }
    fn to_front(&mut self) {
        self.on().to_front();
    }
    fn set_resizable(&mut self, resizable: bool) {
        self.on().set_resizable(resizable);
    }
    fn set_always_on_top(&mut self, on_top: bool) {
        self.on().set_always_on_top(on_top);
    }
    fn set_icon(&mut self, icon: Option<&TrayImage>) {
        self.on().set_icon(icon);
    }
    fn set_menu_bar(&mut self, menus: Option<&[TrayMenuItem]>) -> MenuBarShown {
        self.on().set_menu_bar(menus)
    }
    fn window_position(&self) -> Option<(i32, i32)> {
        self.on().window_position()
    }
    fn window_placement(&self) -> Option<WindowPlacement> {
        self.on().window_placement()
    }
    fn screen_size(&self) -> Option<(i32, i32)> {
        self.on().screen_size()
    }
    fn tray_supported(&self) -> bool {
        self.on().tray_supported()
    }
    fn sync_trays(&mut self, trays: &[TrayIcon]) {
        self.on().sync_trays(trays);
    }
    fn send_notification(&mut self, tray: &TrayIcon, notification: &Notification) {
        self.on().send_notification(tray, notification);
    }
}

//! KompozR's Wayland platform (C++'s wayland_platform.cpp), on our own binding: libwayland-client loaded at run time
//! ([`ffi`]) and the protocols generated from their XML ([`generated`], `cargo xtask wayland`). One connection for
//! all the app's windows (the main one and each `Window`'s: [`WaylandWindow`], a handle on it by its id), an
//! xdg-shell toplevel each, drawn into by a Vulkan device of its own (`VK_KHR_wayland_surface`), its frames paced by
//! its own frame callbacks, its events routed to it by the surface they come for (the pointer's, the keyboard's,
//! a finger's, the input method's, a tablet tool's, a drag's), brought to the front through xdg-activation (which
//! restores a minimized one: xdg-shell has no request for it; its restoring is seen as the compositor activates it
//! again), its icon its own (xdg-toplevel-icon), its menu bar the desktop's global menu where KDE's has one (appmenu:
//! the dbusmenu's address on the bus), what is behind it showing where a transparent window draws nothing (the
//! swapchain's premultiplied alpha); where it is on the screen and above the others no protocol lets a client say
//! (a `Window`'s position and `always_on_top` aren't followed here);
//! its scale fractional (fractional-scale + viewporter) or whole (the surface's preferred buffer scale, else its
//! outputs'); the compositor's decorations (xdg-decoration) or, without them, KompozR's own (the title bar in the
//! metrics, the window's actions; edges around the window with shadows, resizing it: [`decorations`]); the seat's
//! pointer (wheel notches, touchpad scrolls and pinches), keyboard (xkbcommon, compose sequences, client-side repeat),
//! touch screen, input method (text-input-v3) and graphics tablets (tablet-v2: styluses, erasers, tablet mice, pads);
//! cursors by shape (cursor-shape-v1) or from the cursor theme (libwayland-cursor); the clipboard and drops from other
//! apps (wl_data_device), the app's own drags out to them ([`drag_source`]); the screen kept on (idle-inhibit); and the desktop's services ([`DesktopServices`]: the
//! theme, the accent, the text size, the title bar's buttons, the wallpaper's colors, screen readers, a microphone and
//! a camera).
//!
//! Re-entrancy: protocol events are only queued while libwayland dispatches ([`ffi`]); the platform reads them
//! between the cores' calls, gathering each window's [`Event`](crate::Event)s. The platform works on one window at a
//! time: the window an event or a core's call is for is selected first ([`WaylandPlatform::select`]: its state,
//! [`WindowData`], is the platform's `win` while it is), the others parked. What the hosts' callbacks need (the
//! cursor, the input method, the tablets' cursors, the clipboard) is kept apart ([`Shared`]), borrowed only briefly,
//! never across a call into a host; each window's field and cursor are kept there by its id, the seat's own shown
//! for the window the pointer or the keyboard is in.
//!
//! The environment:
//! - `KOMPOZ_SCALE`: px per dp instead of the compositor's scale; `KOMPOZ_FONT_SCALE`: the system's text size.
//! - `KOMPOZ_DECORATIONS=kompoz`: KompozR's decorations even where the compositor has its own.
//! - `KOMPOZ_WAYLAND_IGNORE=name,…`: those globals left unbound, as on a compositor without them (what the
//!   fallbacks are tested with: `wp_cursor_shape_manager_v1` for the cursor theme, `wp_fractional_scale_manager_v1`
//!   for whole scales, `zxdg_decoration_manager_v1` for KompozR's decorations).

mod cursor;
mod data_device;
mod decorations;
mod drag_source;
pub mod ffi;
pub mod generated;
mod input;
mod tablet;
mod vulkan;
mod window;

use self::cursor::Cursors;
use self::data_device::Clipboard;
use self::ffi::{Connection, Object, ObjectId};
use self::generated::*;
use self::input::{Pinch, TextInput, Wheel};
use self::tablet::Tablets;
use self::window::{Output, WindowState};
use super::desktop::DesktopServices;
use super::event_loop::{EventLoop, Waker};
use super::sys;
use super::xkb::{self, ComposeState, Keyboard, RepeatTimer};
use crate::app::{
    AppConfig, AppContent, AppWindows, DisplayPlatform, Event, MenuBarShown, Metrics, Platform, TrayIcon, TrayImage,
    TrayMenuItem, WindowAction, WindowSpec,
};
use crate::system::{log, preferred_language};
use kompoz_gpu::device::{Backend, Surface};
use kompoz_graphics::Rect;
use kompoz_ui::KompozHost;
use kompoz_ui::clipboard::ClipEntry;
use kompoz_ui::draganddrop::PlatformDragAndDrop;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use kompoz_ui::platform::{ImeOptions, PlatformCallbacks, PointerIcon, TextEditState};
use kompoz_ui::window::{Notification, WindowId, WindowPlacement, WindowRequest};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// The Wayland platform, as `App::run` tries it (first, unless `KOMPOZ_PLATFORM` names another).
pub const PLATFORM: DisplayPlatform = DisplayPlatform { name: "wayland", run };

/// The event loop's token for the display's socket.
const DISPLAY: u32 = 1;

/// The app on the compositor until it quits (C++'s `runWayland`); none where there is no Wayland display. Its
/// windows share one connection: the root core's (the main window's, or the app's composition's without one) and
/// each `Window`'s, a handle on it by id.
fn run(config: &AppConfig, content: AppContent) -> Option<ExitCode> {
    std::env::var_os("WAYLAND_DISPLAY").filter(|d| !d.is_empty())?;
    let platform = match WaylandPlatform::connect() {
        Ok(platform) => platform,
        Err(why) => {
            log(&format!("kompoz: Wayland: {why}"));
            return None;
        }
    };
    let wayland = Rc::new(RefCell::new(platform));
    let app_id = match content {
        AppContent::Application(_) => WindowId::APPLICATION,
        AppContent::Window(_) => WindowId::MAIN,
    };
    wayland.borrow_mut().app_id = app_id;
    let root = content.core(WaylandWindow::new(&wayland, app_id), config);
    let make = {
        let wayland = wayland.clone();
        Box::new(move |r: &WindowRequest| Some(WaylandWindow::new(&wayland, r.id())))
    };
    let mut windows = AppWindows::new(root, make);
    if !windows.start() {
        return Some(ExitCode::FAILURE);
    }
    run_loop(&mut windows, &wayland);
    drop(windows); // (the windows and their devices before the connection)
    Some(ExitCode::SUCCESS)
}

/// The loop (C++'s `run`): the compositor's events and the loop's wakes handled by the cores of the windows they are
/// for, then a frame of each window that wants one and whose last frame callback came (asked for before each frame:
/// the compositor says when the next is worth drawing; a window without a surface — the app's composition, a hidden
/// window — whenever it wants one), the windows synced with their requests, then a wait for what comes next.
fn run_loop(windows: &mut AppWindows<WaylandWindow>, wayland: &Rc<RefCell<WaylandPlatform>>) {
    let mut need_frame: HashMap<WindowId, bool> = HashMap::new();
    loop {
        let events = wayland.borrow_mut().take_events();
        let mut quit = false;
        let mut typed = None;
        for (id, e) in events {
            quit |= e == Event::Quit;
            if matches!(e, Event::Text(_) | Event::Preedit { .. } | Event::DeleteSurroundingText { .. }) {
                typed = Some(id);
            }
            windows.handle(id, &e);
            need_frame.insert(id, true);
        }
        if let Some(id) = typed {
            // (The input method told the field as the typing left it.)
            let state = windows.core(id).and_then(|c| c.host()).and_then(|h| h.text_input_service().session_state());
            wayland.borrow_mut().answer_input_method(state);
        }
        if quit || wayland.borrow().broken {
            break;
        }
        windows.sync();
        for id in windows.ids() {
            let scripted = windows.core(id).is_some_and(|c| c.scripted());
            let need = need_frame.entry(id).or_insert(true);
            *need |= scripted;
            if !wayland.borrow_mut().frame_ready(id, need) {
                continue;
            }
            *need = false;
            wayland.borrow_mut().before_frame(id);
            let Some(core) = windows.core_mut(id) else { continue };
            let next = core.frame(None);
            wayland.borrow_mut().after_frame(id, next.busy, next.wake_at);
            quit |= next.quit;
        }
        windows.sync();
        if quit || windows.quitting() {
            break;
        }
        let ids = windows.ids();
        let now = ids.iter().any(|id| {
            let need = need_frame.get(id).copied().unwrap_or(true) || windows.core(*id).is_some_and(|c| c.scripted());
            need && wayland.borrow().can_frame(*id)
        });
        if !now {
            // Waiting for the compositor's beat: the hosts' work between frames until the earliest is due.
            let due = wayland.borrow().next_beat();
            let at = sys::monotonic_now();
            if let Some(due) = due
                && at < due
            {
                for core in windows.cores_mut() {
                    core.idle(due - at);
                }
            }
        }
        if !wayland.borrow_mut().wait(now) {
            break; // (the compositor went away)
        }
    }
    windows.quit();
}

/// A display output: its scale and refresh rate, and whether the window is on it (C++'s Output).
struct OutputObject {
    output: WlOutput,
    name: u32,
    info: Output,
}

/// The globals bound (the versions C++ binds).
#[derive(Default)]
struct Globals {
    compositor: Option<Rc<WlCompositor>>,
    wm_base: Option<XdgWmBase>,
    decorations: Option<ZxdgDecorationManagerV1>,
    fractional: Option<WpFractionalScaleManagerV1>,
    viewporter: Option<WpViewporter>,
    cursor_shapes: Option<WpCursorShapeManagerV1>,
    text_inputs: Option<ZwpTextInputManagerV3>,
    tablets: Option<ZwpTabletManagerV2>,
    idle_inhibits: Option<ZwpIdleInhibitManagerV1>,
    gestures: Option<ZwpPointerGesturesV1>,
    activation: Option<XdgActivationV1>,
    /// KDE's appmenu (a window's menu bar's address on the bus, for the global menu).
    appmenu: Option<OrgKdeKwinAppmenuManager>,
    /// xdg-toplevel-icon (a window's own icon).
    toplevel_icons: Option<XdgToplevelIconManagerV1>,
    shm: Option<Rc<WlShm>>,
    subcompositor: Option<WlSubcompositor>,
    seat: Option<WlSeat>,
    // (Last of them: the registry the others came from.)
    registry: Option<WlRegistry>,
}

/// The window's objects (destroyed in this order: KompozR's edges and roles before the surface). The decoration and
/// the fractional scale are only kept: their events come to the platform, dropping them would end them.
struct Window {
    /// Its icon (xdg-toplevel-icon) and the buffer it was made of (the buffer kept until the icon is destroyed:
    /// the tuple's order).
    icon: Option<(XdgToplevelIconV1, WlBuffer)>,
    /// Where its menu bar is on the bus, told the compositor (KDE's appmenu).
    appmenu: Option<OrgKdeKwinAppmenu>,
    edges: decorations::Edges,
    frame_callback: Option<WlCallback>,
    inhibitor: Option<ZwpIdleInhibitorV1>,
    _decoration: Option<ZxdgToplevelDecorationV1>,
    toplevel: XdgToplevel,
    xdg_surface: XdgSurface,
    viewport: Option<WpViewport>,
    _fractional: Option<WpFractionalScaleV1>,
    surface: WlSurface,
}

/// The seat's devices the platform keeps (the pointer is the cursor's: [`Cursors`]).
#[derive(Default)]
struct Devices {
    keyboard: Option<WlKeyboard>,
    touch: Option<WlTouch>,
    pinch: Option<ZwpPointerGesturePinchV1>,
}

/// What the host's callbacks use (C++'s callbacks reached into the platform; here the platform and they share
/// this, each borrowing a part briefly and never across a call into the host).
pub(crate) struct Shared {
    connection: Connection,
    waker: Waker,
    cursor: RefCell<Cursors>,
    /// Each window's cursor as its host last set it, and the window the pointer is in (its cursor shown).
    icons: RefCell<HashMap<WindowId, PointerIcon>>,
    pointer_window: Cell<Option<WindowId>>,
    text: RefCell<TextInput>,
    /// Each window's field as its host last told it, and the window the input method is on (its field told).
    fields: RefCell<HashMap<WindowId, Field>>,
    text_window: Cell<Option<WindowId>>,
    tablet: RefCell<Tablets>,
    clipboard: RefCell<Clipboard>,
    /// A drag of the app's own the host asked the platform to take, started once the event that moved it out of
    /// the window is handled ([`Platform::handled`]).
    drag_request: RefCell<Option<PlatformDragAndDrop>>,
    /// A drag can be taken: the window is open and a press (whose grab it starts from) happened.
    can_drag: Cell<bool>,
    /// A drag of the app's own runs (the compositor's).
    dragging: Cell<bool>,
}

/// A window's field as its host told it: whether it takes typing, where (points), its text, what it wants.
type Field = (bool, [i32; 4], TextEditState, ImeOptions);

/// A window's host's callbacks on Wayland: the platform's shared state, the window's id and scale.
struct WaylandCallbacks {
    shared: Rc<Shared>,
    window: WindowId,
    /// px per point (the window's scale), for the input method's area.
    scale: Rc<Cell<f64>>,
}

impl std::ops::Deref for WaylandCallbacks {
    type Target = Rc<Shared>;
    fn deref(&self) -> &Rc<Shared> {
        &self.shared
    }
}

impl PlatformCallbacks for WaylandCallbacks {
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        Some(self.shared.waker.as_fn())
    }

    /// (C++'s textInput: the area from px to points.) Kept as the window's field; told the input method while it is
    /// on this window.
    fn text_input(&self, active: bool, area: Rect, state: &TextEditState, options: &ImeOptions) {
        let k = 1.0 / self.scale.get().max(1e-3) as f32;
        let area = [area.left * k, area.top * k, area.width() * k, area.height() * k].map(|v| v.round() as i32);
        self.shared.fields.borrow_mut().insert(self.window, (active, area, state.clone(), options.clone()));
        if self.shared.text_window.get().is_none_or(|w| w == self.window) {
            self.shared.text.borrow_mut().set(active, area, state, options);
            let _ = self.shared.connection.flush();
        }
    }

    fn text_input_changed(&self, state: &TextEditState) {
        if let Some(field) = self.shared.fields.borrow_mut().get_mut(&self.window) {
            field.2 = state.clone();
        }
        if self.shared.text_window.get().is_none_or(|w| w == self.window) {
            self.shared.text.borrow_mut().changed(state);
            let _ = self.shared.connection.flush();
        }
    }

    /// Kept as the window's cursor; shown while the pointer is in this window.
    fn cursor(&self, icon: PointerIcon) {
        self.shared.icons.borrow_mut().insert(self.window, icon);
        if self.shared.pointer_window.get().is_none_or(|w| w == self.window) {
            self.shared.cursor.borrow_mut().set_icon(icon);
            self.shared.tablet.borrow_mut().set_pointer_icon(icon);
            let _ = self.shared.connection.flush();
        }
    }

    fn stylus_hover_icon(&self, icon: Option<PointerIcon>) {
        self.shared.tablet.borrow_mut().set_stylus_icon(icon);
        let _ = self.shared.connection.flush();
    }

    fn get_clip_entry(&self) -> Option<ClipEntry> {
        self.shared.clipboard.borrow().entry(&Rc::downgrade(&self.shared))
    }

    /// (Another app's entry read first, as it is: it may go once ours replaces it.)
    fn set_clip_entry(&self, entry: Option<ClipEntry>) {
        let entry = entry.map(|e| e.materialized());
        self.shared.clipboard.borrow_mut().write(entry);
        let _ = self.shared.connection.flush();
    }

    fn open_uri(&self, uri: &str) {
        super::desktop::open_uri(uri);
    }

    /// Taken where the compositor can run it (data device 3: a drag's actions and end are told; a source allowing
    /// copy or move: Wayland has no link) from the press it began with; started once the event is handled.
    fn start_drag_and_drop(&self, drag: &PlatformDragAndDrop) -> bool {
        let s = &self.shared;
        let clipboard = s.clipboard.borrow();
        let runs = clipboard.device.is_some() && clipboard.manager.as_ref().is_some_and(|m| m.version() >= 3);
        let actions = drag_source::actions_mask(&drag.transfer_data.supported_actions);
        if !runs || !s.can_drag.get() || s.dragging.get() || actions == WlDataDeviceManagerDndAction::NONE {
            return false;
        }
        *s.drag_request.borrow_mut() = Some(drag.clone());
        true
    }
}

/// A window's state on the connection (the main window's, a `Window`'s, the app's composition's without a surface):
/// its objects, size, scale, configure and frame callbacks, the outputs it is on, its events for its core. The one
/// the platform works on is its `win`; the others are parked ([`WaylandPlatform::select`]).
pub(crate) struct WindowData {
    id: WindowId,
    window: Option<Window>,
    /// Its core's events, gathered from the protocol's.
    events: VecDeque<Event>,
    title: String,
    /// The window's size in points (the compositor's logical pixels, a dp each at its own scale).
    width: i32,
    height: i32,
    /// A configure's size (0: the app's own).
    pending_width: i32,
    pending_height: i32,
    /// Its size the last time it floated (neither maximized, fullscreen nor tiled): what it goes back to when a
    /// configure leaves the size to it on the way back from one of those (Weston's 0 by 0; KWin tells the size).
    floating_width: i32,
    floating_height: i32,
    configured: bool,
    /// fractional-scale's scale, in 120ths (0: none told).
    scale120: u32,
    /// Else the whole scale the surface is given.
    buffer_scale: i32,
    /// px per point, as its host's callbacks read it (the input method's area).
    scale_cell: Rc<Cell<f64>>,
    /// The window's state as the last configure told it (the next one's until its xdg_surface configure).
    pending_state: WindowState,
    state: WindowState,
    /// What the window manager does (xdg_toplevel's wm_capabilities: all until it says).
    can_minimize: bool,
    can_maximize: bool,
    /// KompozR decorates the window (the compositor has no decorations, or KOMPOZ_DECORATIONS=kompoz, or the
    /// `Window` is undecorated: then without a title bar): the title bar in the metrics, the edges around it
    /// ([`decorations`]).
    own_decorations: bool,
    /// A `Window` without decorations: none of the compositor's, no title bar of KompozR's.
    undecorated: bool,
    /// Its icon, as its `Window` gives it (given again to a window shown again).
    icon: Option<TrayImage>,
    /// Minimized by the app or its title bar (Wayland tells a client nothing of its minimizing): until the
    /// compositor shows it again (activated, or no longer suspended), which is its restoring.
    minimized: bool,
    /// The pointer over an edge: where, in the window's points (what a press resizes from).
    edge_x: f64,
    edge_y: f64,
    /// The drawable's size, px (the GPU device reads it).
    drawable: Rc<Cell<(i32, i32)>>,
    /// The outputs it is on.
    entered: Vec<ObjectId>,
    /// Its frames: the last one asked for another at once; its frame callback awaited, or come since; when it was
    /// asked for (monotonic ns); the next frame's wake (a coroutine's delay), when nothing animates.
    busy: bool,
    awaiting_callback: bool,
    callback_done: bool,
    frame_asked_at: i64,
    wake_at: Option<i64>,
}

impl WindowData {
    fn new(id: WindowId) -> WindowData {
        WindowData {
            id,
            window: None,
            events: VecDeque::new(),
            title: "KompozR".into(),
            width: 1,
            height: 1,
            pending_width: 0,
            pending_height: 0,
            floating_width: 0,
            floating_height: 0,
            configured: false,
            scale120: 0,
            buffer_scale: 1,
            scale_cell: Rc::new(Cell::new(1.0)),
            pending_state: WindowState::default(),
            state: WindowState::default(),
            can_minimize: true,
            can_maximize: true,
            own_decorations: false,
            undecorated: false,
            icon: None,
            minimized: false,
            edge_x: 0.0,
            edge_y: 0.0,
            drawable: Rc::new(Cell::new((1, 1))),
            entered: Vec::new(),
            busy: false,
            awaiting_callback: false,
            callback_done: false,
            frame_asked_at: 0,
            wake_at: None,
        }
    }

    /// Whether the window has this object (its surface, an edge's, its xdg surface, toplevel, decoration, fractional
    /// scale, frame callback).
    fn owns(&self, id: ObjectId) -> bool {
        let Some(w) = &self.window else { return false };
        w.surface.id() == id
            || w.xdg_surface.id() == id
            || w.toplevel.id() == id
            || w._decoration.as_ref().is_some_and(|d| d.id() == id)
            || w._fractional.as_ref().is_some_and(|f| f.id() == id)
            || w.frame_callback.as_ref().is_some_and(|c| c.id() == id)
            || w.edges.edge_of(id).is_some()
    }
}

/// Where an event of the platform's own goes: the app's root core, every window's, the window with the keyboard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Route {
    App,
    All,
    Keyboard,
}

/// The Wayland platform (C++'s WaylandPlatform): the connection, the seat, the globals, and the app's windows. Its
/// fields are dropped in order: the windows and the seat's objects, then the globals, then the connection.
pub struct WaylandPlatform {
    /// (First: its threads end before the rest goes.)
    desktop: DesktopServices,
    desktop_started: bool,
    /// A drag of the app's own the compositor runs (before the window: its icon and source go first), and the window
    /// it started from.
    outgoing: Option<drag_source::OutgoingDrag>,
    drag_source_window: Option<WindowId>,
    /// The window worked on, and the others.
    win: WindowData,
    parked: Vec<WindowData>,
    /// The root core's id (the main window's, or the app's composition's).
    app_id: WindowId,
    /// The windows the seat's devices are in: the pointer's, the keyboard's, the last finger's, the input method's,
    /// the tablet tools', a drag's coming in.
    pointer_window: Option<WindowId>,
    keyboard_window: Option<WindowId>,
    touch_window: Option<WindowId>,
    tablet_window: Option<WindowId>,
    drag_window: Option<WindowId>,
    /// Activation tokens asked for (`to_front`), each for a window.
    activations: Vec<(XdgActivationTokenV1, WindowId)>,
    /// Events of the platform's own, for the cores they go to.
    routed: VecDeque<(Route, Event)>,
    devices: Devices,
    outputs: Vec<OutputObject>,
    globals: Globals,
    shared: Rc<Shared>,
    keyboard: Keyboard,
    xkb: xkb::Context,
    event_loop: EventLoop,
    /// The serial of the last input event (what the clipboard is set with), and of the last press (what a move or
    /// the window menu goes by) and its finger (none: the pointer's).
    last_serial: u32,
    press_serial: u32,
    press_touch: Option<i32>,
    /// The surface the pointer is over, where it is on the window, the wheel gathered until the pointer's frame.
    pointer_on: ObjectId,
    pointer_x: f64,
    pointer_y: f64,
    wheel: Wheel,
    pinch: Pinch,
    touches: BTreeMap<i32, (f64, f64)>,
    /// What is dragged over the window (an offer) and where; whether it is the app's own drag coming back.
    drag: Option<ObjectId>,
    drag_x: f64,
    drag_y: f64,
    drag_own: bool,
    /// An offer dropped on the window, finished (or let go: refused) once the core says whether it was taken.
    dropped: Option<ObjectId>,
    /// What it carries, until the core takes it (`take_drop_entry`).
    dropped_entry: Option<ClipEntry>,
    // The loop.
    connection: Connection,
    /// The connection broke: the app ends.
    broken: bool,
}

impl WaylandPlatform {
    /// The display, its globals and the seat (C++'s `connect`); why not where the app can't run here.
    pub fn connect() -> Result<WaylandPlatform, String> {
        let connection = Connection::connect()?;
        let xkb = xkb::Context::new()?;
        let mut event_loop = EventLoop::new().map_err(|e| format!("no event loop: {e}"))?;
        event_loop.watch(connection.fd(), DISPLAY);
        if let Err(e) = event_loop.quit_on_signals() {
            log(&format!("kompoz: SIGINT and SIGTERM won't quit the app ({e})"));
        }
        let compose = xkb.compose_table(&xkb::compose_locale()).and_then(|t| ComposeState::new(&t));
        let shared = Rc::new(Shared {
            connection: connection.clone(),
            waker: event_loop.waker(),
            cursor: RefCell::new(Cursors::new(connection.clone())),
            icons: RefCell::new(HashMap::new()),
            pointer_window: Cell::new(None),
            text: RefCell::new(TextInput::default()),
            fields: RefCell::new(HashMap::new()),
            text_window: Cell::new(None),
            tablet: RefCell::new(Tablets::new(connection.clone())),
            clipboard: RefCell::new(Clipboard::default()),
            drag_request: RefCell::new(None),
            can_drag: Cell::new(false),
            dragging: Cell::new(false),
        });
        let mut p = WaylandPlatform {
            desktop: DesktopServices::new(),
            desktop_started: false,
            outgoing: None,
            drag_source_window: None,
            win: WindowData::new(WindowId::MAIN),
            parked: Vec::new(),
            app_id: WindowId::MAIN,
            pointer_window: None,
            keyboard_window: None,
            touch_window: None,
            tablet_window: None,
            drag_window: None,
            activations: Vec::new(),
            routed: VecDeque::new(),
            devices: Devices::default(),
            outputs: Vec::new(),
            globals: Globals::default(),
            shared,
            keyboard: Keyboard::new(compose),
            xkb,
            event_loop,
            last_serial: 0,
            press_serial: 0,
            press_touch: None,
            pointer_on: ObjectId::NULL,
            pointer_x: 0.0,
            pointer_y: 0.0,
            wheel: Wheel::default(),
            pinch: Pinch::default(),
            touches: BTreeMap::new(),
            drag: None,
            drag_x: 0.0,
            drag_y: 0.0,
            drag_own: false,
            dropped: None,
            dropped_entry: None,
            connection,
            broken: false,
        };
        p.globals.registry = p.connection.display().get_registry();
        p.roundtrip()?; // the globals
        let g = &p.globals;
        if g.compositor.is_none() || g.wm_base.is_none() || g.shm.is_none() || g.subcompositor.is_none() {
            return Err("the compositor lacks wl_compositor, xdg_wm_base, wl_shm or wl_subcompositor".into());
        }
        p.roundtrip()?; // the seat's capabilities, the outputs
        Ok(p)
    }

    /// A roundtrip, and what came handled.
    fn roundtrip(&mut self) -> Result<(), String> {
        self.connection.roundtrip().map_err(|e| e.to_string())?;
        self.pump();
        Ok(())
    }

    /// The protocol's events queued so far, handled.
    fn pump(&mut self) {
        while let Some(raw) = self.connection.take_event() {
            if let Some((sender, event)) = generated::Event::decode(raw) {
                self.on_event(sender, event);
            }
        }
        self.shared.clipboard.borrow_mut().serial = self.last_serial;
        self.shared.can_drag.set(self.win.window.is_some() && self.press_serial != 0);
    }

    /// The cores' events, each for its window: what was read dispatched and handled first; the platform's own to
    /// the cores they go to.
    fn take_events(&mut self) -> Vec<(WindowId, Event)> {
        if let Err(e) = self.connection.dispatch_pending() {
            self.lost(&e.to_string());
        }
        self.pump();
        let mut out = Vec::new();
        let ids = self.window_ids();
        for (route, e) in std::mem::take(&mut self.routed) {
            match route {
                Route::App => out.push((self.app_id, e)),
                Route::Keyboard => out.push((self.keyboard_window.unwrap_or(self.app_id), e)),
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
        if self.win.id == id {
            return;
        }
        let data = match self.parked.iter().position(|w| w.id == id) {
            Some(i) => self.parked.swap_remove(i),
            None => WindowData::new(id),
        };
        let before = std::mem::replace(&mut self.win, data);
        self.parked.push(before);
    }

    /// Window `id`'s state gone (its handle dropped: its window closed before).
    fn forget_window(&mut self, id: WindowId) {
        if self.win.id == id {
            match self.parked.pop() {
                Some(other) => self.win = other,
                None => self.win = WindowData::new(self.app_id),
            }
        } else {
            self.parked.retain(|w| w.id != id);
        }
        for seat in [&mut self.pointer_window, &mut self.keyboard_window, &mut self.touch_window] {
            if *seat == Some(id) {
                *seat = None;
            }
        }
        for seat in [&mut self.tablet_window, &mut self.drag_window, &mut self.drag_source_window] {
            if *seat == Some(id) {
                *seat = None;
            }
        }
        self.shared.icons.borrow_mut().remove(&id);
        self.shared.fields.borrow_mut().remove(&id);
        if self.shared.pointer_window.get() == Some(id) {
            self.shared.pointer_window.set(None);
        }
        if self.shared.text_window.get() == Some(id) {
            self.shared.text_window.set(None);
        }
        self.activations.retain(|(_, w)| *w != id);
    }

    /// The window that has object `id` (a surface, an edge's, its xdg objects, a frame callback).
    fn window_of(&self, id: ObjectId) -> Option<WindowId> {
        std::iter::once(&self.win).chain(self.parked.iter()).find(|w| w.owns(id)).map(|w| w.id)
    }

    /// Whether window `id` can draw a frame now (configured, its last frame callback come); a window without a
    /// surface (the app's composition, a hidden window) can always. With `need` (an event or a wake came for it)
    /// or as the last frame asked (busy, its callback come): one is due.
    fn frame_ready(&mut self, id: WindowId, need: &mut bool) -> bool {
        self.select(id);
        let w = &mut self.win;
        if std::mem::take(&mut w.callback_done) {
            *need |= w.busy;
        }
        if w.window.is_none() {
            *need |= w.busy || w.wake_at.is_some_and(|t| t <= sys::monotonic_now());
            return *need;
        }
        *need && !w.awaiting_callback && w.configured
    }

    /// Whether window `id` could draw now (nothing awaited).
    fn can_frame(&self, id: WindowId) -> bool {
        std::iter::once(&self.win)
            .chain(self.parked.iter())
            .find(|w| w.id == id)
            .is_some_and(|w| w.window.is_none() || (!w.awaiting_callback && w.configured))
    }

    /// When the earliest window awaiting its frame callback is due on the outputs' beat, if any is and the display
    /// has nothing more for now.
    fn next_beat(&self) -> Option<i64> {
        if self.connection.has_events() {
            return None;
        }
        let period = self.frame_period();
        std::iter::once(&self.win)
            .chain(self.parked.iter())
            .filter(|w| w.awaiting_callback)
            .map(|w| w.frame_asked_at + period)
            .min()
    }

    /// The connection broke: logged, the app ends.
    fn lost(&mut self, why: &str) {
        if !self.broken {
            log(&format!("kompoz: Wayland: the connection broke: {why}"));
        }
        self.broken = true;
    }

    /// Waits for the display's events, a wake, the deadline or a key's repeat (none if `now`: only what is there),
    /// reading and dispatching what the display sent (C++'s `waitForEvents`); false if the connection broke.
    fn wait(&mut self, now: bool) -> bool {
        // (Events read earlier but not dispatched first, so that the read is the display's alone.)
        while !self.connection.prepare_read() {
            if self.connection.dispatch_pending().is_err() {
                return false;
            }
        }
        let now = now || self.connection.has_events();
        if let Err(e) = self.connection.flush() {
            self.connection.cancel_read();
            self.lost(&e.to_string());
            return false;
        }
        self.event_loop.set_deadline(self.deadline());
        let ready = match self.event_loop.wait(now.then_some(Duration::ZERO)) {
            Ok(ready) => ready,
            Err(e) => {
                self.connection.cancel_read();
                self.lost(&e.to_string());
                return false;
            }
        };
        if ready.source(DISPLAY) {
            if let Err(e) = self.connection.read_events() {
                self.lost(&e.to_string());
                return false;
            }
        } else {
            self.connection.cancel_read();
        }
        if ready.woken && self.event_loop.take_quit_signal() {
            self.routed.push_back((Route::App, Event::Quit));
        }
        if ready.woken || ready.deadline {
            self.routed.push_back((Route::All, Event::Wake));
        }
        if ready.deadline {
            self.shared.cursor.borrow_mut().animate(sys::monotonic_now());
            self.shared.tablet.borrow_mut().animate(sys::monotonic_now());
        }
        // What the desktop's services told meanwhile (they wake the loop): to all the windows (an assistive
        // technology's requests: each window takes those for its own nodes), the tray's to the app's core.
        for e in self.desktop.take_events() {
            let route = match e {
                Event::TrayAction { .. } | Event::TrayMenuItem { .. } | Event::MenuBarItem { .. } => Route::App,
                _ => Route::All,
            };
            self.routed.push_back((route, e));
        }
        if ready.repeats > 0 {
            let typing = self.shared.text.borrow().active;
            let repeated = self.keyboard.repeated(ready.repeats, sys::monotonic_now(), typing);
            self.routed.extend(repeated.into_iter().map(|e| (Route::Keyboard, e)));
        }
        match self.connection.dispatch_pending() {
            Ok(_) => true,
            Err(e) => {
                self.lost(&e.to_string());
                false
            }
        }
    }

    /// When the loop is to wake by itself: the next coroutine delay, the cursors' next animation frame (the
    /// pointer's, the tablet tools').
    fn deadline(&self) -> Option<i64> {
        let cursor = self.shared.cursor.borrow().next_frame_at();
        let tools = self.shared.tablet.borrow().next_frame_at();
        let wakes = std::iter::once(&self.win).chain(self.parked.iter()).filter_map(|w| w.wake_at);
        wakes.chain([cursor, tools].into_iter().flatten()).min()
    }

    /// A repeat timer's change, done.
    fn repeat(&mut self, timer: RepeatTimer) {
        match timer {
            RepeatTimer::Start { delay, interval } => self.event_loop.start_repeat(delay, interval),
            RepeatTimer::Stop => self.event_loop.stop_repeat(),
            RepeatTimer::Keep => {}
        }
    }

    /// Before a frame: its frame callback asked for (it goes with the commit the frame presents with, else with the
    /// one after it: a frame that showed nothing still hears when the next is due), the surface's size for the scale
    /// given with it.
    fn before_frame(&mut self, id: WindowId) {
        self.select(id);
        let Some(w) = &mut self.win.window else { return };
        w.frame_callback = w.surface.frame();
        self.win.awaiting_callback = w.frame_callback.is_some();
        self.win.frame_asked_at = sys::monotonic_now();
    }

    /// After a frame of window `id`: the surface committed (with the frame callback, if the frame didn't present),
    /// what the frame asked of the next (at once, or a wake then).
    fn after_frame(&mut self, id: WindowId, busy: bool, wake_at: Option<i64>) {
        self.select(id);
        if let Some(w) = &self.win.window {
            w.surface.commit();
        }
        self.win.busy = busy;
        self.win.wake_at = if busy { None } else { wake_at };
    }

    /// The refresh period of the outputs the window is on, ns.
    fn frame_period(&self) -> i64 {
        (1e9 / self.refresh_rate() as f64) as i64
    }

    /// The window a protocol event is for: the one whose object sent it, or the one the seat's device it comes from is
    /// in (a device's enter names the surface: the window it is in from then on). None: the platform's own (the
    /// registry, the seat, the outputs, offers, the app's own drag).
    fn target(&mut self, sender: ObjectId, event: &generated::Event) -> Option<WindowId> {
        use generated::Event as E;
        let of = |p: &Self, surface: ObjectId| p.window_of(surface);
        Some(match event {
            E::WlSurface(_)
            | E::XdgSurface(_)
            | E::XdgToplevel(_)
            | E::ZxdgToplevelDecorationV1(_)
            | E::WpFractionalScaleV1(_)
            | E::WlCallback(_) => of(self, sender)?,
            E::WlPointer(WlPointerEvent::Enter { surface, .. }) => {
                self.pointer_window = of(self, *surface);
                self.shared.pointer_window.set(self.pointer_window);
                self.pointer_window?
            }
            E::WlPointer(WlPointerEvent::Leave { surface, .. }) => of(self, *surface).or(self.pointer_window)?,
            E::WlPointer(_) | E::ZwpPointerGesturePinchV1(_) => self.pointer_window?,
            E::WlKeyboard(WlKeyboardEvent::Enter { surface, .. }) => {
                self.keyboard_window = of(self, *surface);
                self.keyboard_window?
            }
            E::WlKeyboard(WlKeyboardEvent::Leave { surface, .. }) => of(self, *surface).or(self.keyboard_window)?,
            E::WlKeyboard(WlKeyboardEvent::Key { .. } | WlKeyboardEvent::Modifiers { .. }) => self.keyboard_window?,
            E::WlTouch(WlTouchEvent::Down { surface, .. }) => {
                self.touch_window = of(self, *surface);
                self.touch_window?
            }
            E::WlTouch(_) => self.touch_window?,
            E::ZwpTextInputV3(ZwpTextInputV3Event::Enter { surface }) => {
                let window = of(self, *surface)?;
                self.text_window_entered(window);
                window
            }
            E::ZwpTextInputV3(_) => self.shared.text_window.get()?,
            E::ZwpTabletToolV2(ZwpTabletToolV2Event::ProximityIn { surface, .. }) => {
                self.tablet_window = of(self, *surface);
                self.tablet_window?
            }
            E::ZwpTabletToolV2(_) => self.tablet_window?,
            E::ZwpTabletPadV2(ZwpTabletPadV2Event::Enter { surface, .. }) => of(self, *surface)?,
            E::WlDataDevice(WlDataDeviceEvent::Enter { surface, .. }) => {
                self.drag_window = of(self, *surface);
                self.drag_window?
            }
            E::WlDataDevice(_) => self.drag_window?,
            E::WlDataSource(_) if self.outgoing.as_ref().is_some_and(|o| o.is_source(sender)) => {
                self.drag_source_window?
            }
            _ => return None,
        })
    }

    /// The input method came to window `window`: the field it was told before (another window's) let go, this
    /// window's told.
    fn text_window_entered(&mut self, window: WindowId) {
        let before = self.shared.text_window.replace(Some(window));
        if before == Some(window) {
            return;
        }
        let field = self.shared.fields.borrow().get(&window).cloned();
        let mut text = self.shared.text.borrow_mut();
        match field {
            Some((active, area, state, options)) => text.set(active, area, &state, &options),
            None => text.set(false, [0; 4], &TextEditState::default(), &ImeOptions::default()),
        }
    }

    /// A protocol event (C++'s listeners), on the window it is for.
    fn on_event(&mut self, sender: ObjectId, event: generated::Event) {
        use generated::Event as E;
        if let Some(window) = self.target(sender, &event) {
            self.select(window);
        }
        match event {
            E::XdgActivationTokenV1(XdgActivationTokenV1Event::Done { token }) => self.activate(sender, &token),
            E::WlRegistry(e) => self.on_registry(e),
            E::XdgWmBase(XdgWmBaseEvent::Ping { serial }) => {
                if let Some(base) = &self.globals.wm_base {
                    base.pong(serial);
                }
            }
            E::WlOutput(e) => {
                self.on_output(sender, e);
                // (Every window on it: its scale may have changed.)
                for id in self.window_ids() {
                    self.select(id);
                    self.output_scale_changed();
                }
                self.draw_icon(); // (a drag's icon over it: its scale may have changed)
            }
            E::WlSurface(e) if self.outgoing.as_ref().is_some_and(|o| o.is_icon(sender)) => self.on_icon_surface(e),
            E::WlSurface(e) => self.on_surface(sender, e),
            E::WpFractionalScaleV1(e) if self.outgoing.as_ref().is_some_and(|o| o.is_icon_scale(sender)) => {
                self.on_icon_fractional_scale(e);
            }
            E::WpFractionalScaleV1(e) => self.on_fractional_scale(e),
            E::XdgSurface(e) => self.on_xdg_surface(e),
            E::XdgToplevel(e) => self.on_toplevel(e),
            E::ZxdgToplevelDecorationV1(e) => self.on_decoration(e),
            E::WlCallback(WlCallbackEvent::Done { .. })
                if self.outgoing.as_ref().is_some_and(|o| o.is_check(sender)) =>
            {
                self.drag_check_done();
            }
            E::WlCallback(WlCallbackEvent::Done { .. }) => {
                if let Some(w) = &mut self.win.window
                    && w.frame_callback.as_ref().is_some_and(|c| c.id() == sender)
                {
                    w.frame_callback = None; // (destroyed: the compositor is done with it)
                    self.win.awaiting_callback = false;
                    self.win.callback_done = true;
                }
            }
            E::WlSeat(e) => self.on_seat(e),
            E::WlPointer(e) => self.on_pointer(e),
            E::WlKeyboard(e) => self.on_keyboard(e),
            E::WlTouch(e) => self.on_touch(e),
            E::ZwpPointerGesturePinchV1(e) => self.on_pinch(e),
            E::ZwpTextInputV3(e) => self.on_text_input(e),
            E::ZwpTabletSeatV2(e) => self.on_tablet_seat(e),
            E::ZwpTabletV2(e) => self.on_tablet(sender, e),
            E::ZwpTabletToolV2(e) => self.on_tablet_tool(sender, e),
            E::ZwpTabletPadV2(e) => self.on_tablet_pad(sender, e),
            E::WlDataOffer(e) => self.on_data_offer(sender, e),
            E::WlDataDevice(e) => self.on_data_device(e),
            E::WlDataSource(e) if self.outgoing.as_ref().is_some_and(|o| o.is_source(sender)) => {
                self.on_drag_source(e);
            }
            E::WlDataSource(e) => self.on_data_source(sender, e),
            E::WlBuffer(WlBufferEvent::Release) if !self.icon_buffer_released(sender) => {
                // (An edge's buffer, of whichever window.)
                for id in self.window_ids() {
                    self.select(id);
                    self.buffer_released(sender);
                }
            }
            _ => {}
        }
    }

    /// A global (C++'s registry listener): bound, at most at the version C++ binds, unless `KOMPOZ_WAYLAND_IGNORE`
    /// names it.
    fn on_registry(&mut self, e: WlRegistryEvent) {
        match e {
            WlRegistryEvent::Global { name, interface, version } => {
                let ignored = std::env::var("KOMPOZ_WAYLAND_IGNORE").unwrap_or_default();
                if ignored.split(',').any(|i| i.trim() == interface) {
                    return;
                }
                // (Taken while the others are bound, given back after.)
                let Some(taken) = self.globals.registry.take() else { return };
                let registry = &taken;
                let g = &mut self.globals;
                fn bind<T: Object>(registry: &WlRegistry, name: u32, version: u32, most: u32) -> Option<T> {
                    registry.bind(name, version.min(most))
                }
                match interface.as_str() {
                    "wl_compositor" => g.compositor = bind(registry, name, version, 6).map(Rc::new),
                    "xdg_wm_base" => g.wm_base = bind(registry, name, version, 6),
                    "zxdg_decoration_manager_v1" => g.decorations = bind(registry, name, version, 1),
                    "wp_fractional_scale_manager_v1" => g.fractional = bind(registry, name, version, 1),
                    "wp_viewporter" => g.viewporter = bind(registry, name, version, 1),
                    "wp_cursor_shape_manager_v1" => g.cursor_shapes = bind(registry, name, version, 1),
                    "zwp_text_input_manager_v3" => g.text_inputs = bind(registry, name, version, 1),
                    "zwp_tablet_manager_v2" => g.tablets = bind(registry, name, version, 1),
                    "wl_data_device_manager" => {
                        self.shared.clipboard.borrow_mut().manager = bind(registry, name, version, 3);
                    }
                    "wl_shm" => g.shm = bind(registry, name, version, 1).map(Rc::new),
                    "wl_subcompositor" => g.subcompositor = bind(registry, name, version, 1),
                    "zwp_pointer_gestures_v1" => g.gestures = bind(registry, name, version, 1),
                    "zwp_idle_inhibit_manager_v1" => g.idle_inhibits = bind(registry, name, version, 1),
                    "xdg_activation_v1" => g.activation = bind(registry, name, version, 1),
                    "org_kde_kwin_appmenu_manager" => g.appmenu = bind(registry, name, version, 1),
                    "xdg_toplevel_icon_manager_v1" => g.toplevel_icons = bind(registry, name, version, 1),
                    "wl_seat" if g.seat.is_none() => g.seat = bind(registry, name, version, 8),
                    "wl_output" => {
                        if let Some(output) = bind(registry, name, version, 2) {
                            self.outputs.push(OutputObject { output, name, info: Output::default() });
                        }
                    }
                    _ => {}
                }
                self.globals.registry = Some(taken);
                self.attach_seat();
            }
            WlRegistryEvent::GlobalRemove { name } => {
                self.outputs.retain(|o| o.name != name);
            }
        }
    }

    /// The seat's objects that need a manager as well (whichever came first): the data device, the text input, the
    /// tablets, the cursor's shape device or theme, the touchpad's pinch.
    fn attach_seat(&mut self) {
        let g = &self.globals;
        let Some(seat) = &g.seat else { return };
        self.shared.clipboard.borrow_mut().attach(seat);
        self.shared.tablet.borrow_mut().attach(g.tablets.as_ref(), seat, g.compositor.clone(), g.shm.clone());
        {
            let mut text = self.shared.text.borrow_mut();
            if let Some(manager) = &g.text_inputs
                && text.input.is_none()
            {
                text.input = manager.get_text_input(seat);
            }
        }
        let mut cursor = self.shared.cursor.borrow_mut();
        cursor.attach(g.cursor_shapes.as_ref(), g.compositor.clone(), g.shm.clone());
        if let (Some(pointer), Some(gestures)) = (&cursor.pointer, &g.gestures)
            && self.devices.pinch.is_none()
        {
            self.devices.pinch = gestures.get_pinch_gesture(pointer);
        }
    }

    /// The seat's capabilities (C++'s seat listener): its pointer, keyboard and touch screen made or let go.
    fn on_seat(&mut self, e: WlSeatEvent) {
        let WlSeatEvent::Capabilities { capabilities: caps } = e else { return };
        // (Taken while its devices are made or let go, given back after.)
        let Some(seat) = self.globals.seat.take() else { return };
        let mut changed = false;
        {
            let mut cursor = self.shared.cursor.borrow_mut();
            let has = caps.contains(WlSeatCapability::POINTER);
            if has && cursor.pointer.is_none() {
                cursor.pointer = seat.get_pointer();
                changed = true;
            } else if !has && cursor.pointer.is_some() {
                self.devices.pinch = None;
                cursor.release_pointer();
                changed = true;
            }
        }
        let has = caps.contains(WlSeatCapability::KEYBOARD);
        if has && self.devices.keyboard.is_none() {
            self.devices.keyboard = seat.get_keyboard();
            changed = true;
        } else if !has && self.devices.keyboard.is_some() {
            self.devices.keyboard = None; // (released)
            let timer = self.keyboard.leave();
            self.repeat(timer);
            changed = true;
        }
        let has = caps.contains(WlSeatCapability::TOUCH);
        if has && self.devices.touch.is_none() {
            self.devices.touch = seat.get_touch();
            changed = true;
        } else if !has && self.devices.touch.is_some() {
            self.devices.touch = None;
            let lifted = self.touches_lifted();
            self.win.events.extend(lifted);
            changed = true;
        }
        self.globals.seat = Some(seat);
        self.attach_seat();
        if changed {
            self.win.events.push_back(Event::DevicesChanged);
        }
    }
}

impl Drop for WaylandPlatform {
    fn drop(&mut self) {
        self.desktop.stop();
        self.outgoing = None;
        self.activations.clear();
        for id in self.window_ids() {
            self.select(id);
            self.close_window();
        }
        // (What the callbacks hold goes with the platform's objects, before the globals they came from.)
        self.shared.cursor.borrow_mut().release_pointer();
        *self.shared.text.borrow_mut() = TextInput::default();
        *self.shared.tablet.borrow_mut() = Tablets::new(self.connection.clone());
        *self.shared.clipboard.borrow_mut() = Clipboard::default();
        self.devices = Devices::default();
        let _ = self.connection.flush();
    }
}

impl Platform for WaylandPlatform {
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
        if backend != Backend::Vulkan {
            return Err(format!("no {} on Wayland", backend.name()));
        }
        let made = self.make_window(window);
        if let Err(why) = made {
            self.close_window();
            return Err(why);
        }
        let size = self.win.drawable.clone();
        let w = self.win.window.as_ref().ok_or("no window")?;
        Ok(Surface {
            vulkan: Some(vulkan::surface(&self.connection, &w.surface)),
            drawable_size: Some(Box::new(move || size.get())),
            ..Surface::default()
        })
    }

    fn close_window(&mut self) {
        if self.outgoing.is_some() && self.drag_source_window == Some(self.win.id) {
            self.outgoing = None; // (its source and icon go with the window they started from)
        }
        self.win.window = None;
        self.win.configured = false;
        self.win.awaiting_callback = false;
        self.win.entered.clear();
        self.desktop.window_closed(self.win.id); // (its tree gone from the screen readers)
        let _ = self.connection.flush();
    }

    fn show_error(&mut self, message: &str) {
        log(message);
    }

    /// Points are the compositor's logical pixels; the scale its own (fractional-scale's, else the buffer scale);
    /// KOMPOZ_SCALE and KOMPOZ_FONT_SCALE stand in for it and the desktop's text size (C++'s); the theme and the
    /// title bar's buttons the desktop's.
    fn metrics(&self) -> Metrics {
        let number = |name: &str| self.option(name).and_then(|v| v.trim().parse::<f32>().ok());
        let scale = self.scale() as f32;
        let (pw, ph) = self.win.drawable.get();
        Metrics {
            pixel_width: pw,
            pixel_height: ph,
            point_width: self.win.width,
            point_height: self.win.height,
            scale: number("KOMPOZ_SCALE").filter(|s| *s > 0.0).unwrap_or(scale),
            point_to_px: scale,
            font_scale: match number("KOMPOZ_FONT_SCALE") {
                Some(s) => s.max(0.5),
                None => self.desktop.font_scale().unwrap_or(1.0),
            },
            refresh_rate: self.refresh_rate(),
            safe_area: Some(Rect::new(0.0, 0.0, self.win.width as f32, self.win.height as f32)),
            dark_theme: self.desktop.dark_theme(),
            language: preferred_language().unwrap_or_default(),
            title_bar: (self.win.own_decorations && !self.win.undecorated && !self.win.state.fullscreen)
                .then(|| self.title_bar()),
        }
    }

    fn pixel_size(&self) -> (i32, i32) {
        self.win.drawable.get()
    }

    fn set_window_size(&mut self, width: i32, height: i32) {
        self.win.width = width.max(1);
        self.win.height = height.max(1);
        if self.win.state.floating() {
            (self.win.floating_width, self.win.floating_height) = (self.win.width, self.win.height);
        }
        self.apply_size();
        self.update_edges();
        self.win.events.push_back(Event::Resized);
    }

    fn window_action(&mut self, action: WindowAction, x: f32, y: f32) {
        self.act(action, x, y);
    }

    /// A mouse is a fine pointer, a touch screen a coarse one, a keyboard the seat has a physical one; the
    /// microphone and camera as the desktop's services probe them (C++'s).
    fn media(&mut self, probe: bool) -> MediaInfo {
        let pointer = self.shared.cursor.borrow().pointer.is_some();
        let precision = if pointer {
            PointerPrecision::Fine
        } else if self.devices.touch.is_some() {
            PointerPrecision::Coarse
        } else {
            PointerPrecision::None
        };
        self.desktop.media(probe, precision, self.devices.keyboard.is_some())
    }

    /// The window's host's callbacks; the desktop's services started with the first (their threads wake the loop).
    fn attach(&mut self, _host: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        if !self.desktop_started {
            self.desktop_started = true;
            let title = self.win.title.clone();
            self.desktop.start(&title, self.event_loop.waker().as_fn());
        }
        Rc::new(WaylandCallbacks {
            shared: self.shared.clone(),
            window: self.win.id,
            scale: self.win.scale_cell.clone(),
        })
    }

    fn metrics_updated(&mut self, _host: &KompozHost) {
        self.desktop.metrics_updated();
    }

    fn handled(&mut self, host: &KompozHost, event: &Event) {
        self.desktop.handled(self.win.id, host, event);
        self.start_outgoing_drag(); // (one the host asked for as it handled the event)
    }

    fn take_drop_entry(&mut self) -> Option<ClipEntry> {
        self.dropped_entry.take()
    }

    fn drop_done(&mut self, taken: bool) {
        self.finish_drop(taken);
    }

    /// keep_screen_on: an idle inhibitor on the window while it is laid out (C++'s); the window's semantics tree to
    /// the screen readers (each window with a surface: a frame of the app's each; at the origin: Wayland doesn't
    /// tell a window where it is).
    fn composed(&mut self, host: &KompozHost, redraw: bool, frame_time: i64) {
        if self.win.window.is_some() {
            let window = Rect::new(0.0, 0.0, self.win.width as f32, self.win.height as f32);
            let title = self.win.title.clone();
            let id = self.win.id;
            self.desktop.composed(id, host, redraw, frame_time, window, (1.0 / self.scale()) as f32, &title);
        }
        let on = host.keep_screen_on();
        let Some(w) = &mut self.win.window else { return };
        if on == w.inhibitor.is_some() {
            return;
        }
        w.inhibitor = match (&self.globals.idle_inhibits, on) {
            (Some(manager), true) => manager.create_inhibitor(&w.surface),
            _ => None,
        };
    }

    fn wants_frames(&self) -> bool {
        self.win.window.is_some() && self.desktop.wants_frames(self.win.id)
    }

    fn set_title(&mut self, title: &str) {
        self.win.title = title.to_string();
        if let Some(w) = &self.win.window {
            w.toplevel.set_title(title);
            let _ = self.connection.flush();
        }
    }

    /// Maximized or full screen asked of the compositor (it tells what it did in the next configure).
    fn set_placement(&mut self, placement: WindowPlacement) {
        let Some(w) = &self.win.window else { return };
        let s = self.win.state;
        match placement {
            WindowPlacement::Floating => {
                if s.fullscreen {
                    w.toplevel.unset_fullscreen();
                }
                if s.maximized {
                    w.toplevel.unset_maximized();
                }
            }
            WindowPlacement::Maximized => {
                if s.fullscreen {
                    w.toplevel.unset_fullscreen();
                }
                w.toplevel.set_maximized();
            }
            WindowPlacement::Fullscreen => w.toplevel.set_fullscreen(None),
        }
        let _ = self.connection.flush();
    }

    /// Minimized (xdg_toplevel's set_minimized); a window can't restore itself on Wayland (xdg-shell has no
    /// request for it): restoring brings it to the front (`to_front`, through an activation token), which the
    /// compositor restores it for; its restoring is told as the compositor activates it again or stops suspending
    /// it (the user restoring it from the taskbar too).
    fn set_minimized(&mut self, minimized: bool) {
        if minimized {
            if let Some(w) = &self.win.window {
                w.toplevel.set_minimized();
                let _ = self.connection.flush();
            }
            // (Wayland tells a client nothing of its minimizing: the core's state says it, until the compositor
            // shows the window again.)
            self.win.minimized = true;
            self.win.events.push_back(Event::Minimized);
        } else {
            self.to_front();
        }
    }

    /// To the front through xdg-activation: a token asked for with the last input's serial (and the surface that
    /// has the keyboard), the window activated with it when it comes. Without xdg-activation: nothing (Wayland lets
    /// no window raise itself otherwise).
    fn to_front(&mut self) {
        let (Some(activation), Some(w)) = (&self.globals.activation, &self.win.window) else { return };
        let Some(token) = activation.get_activation_token() else { return };
        if let Some(seat) = &self.globals.seat
            && self.last_serial != 0
        {
            token.set_serial(self.last_serial, seat);
        }
        token.set_app_id("dev.kompoz.app");
        token.set_surface(&w.surface);
        token.commit();
        self.activations.push((token, self.win.id));
        let _ = self.connection.flush();
    }

    /// The compositor keeps the size between the bounds: none (resizable), or the window's own.
    fn set_resizable(&mut self, resizable: bool) {
        self.apply_resizable(resizable);
    }

    fn window_placement(&self) -> Option<WindowPlacement> {
        self.win.window.as_ref()?;
        let s = self.win.state;
        Some(if s.fullscreen {
            WindowPlacement::Fullscreen
        } else if s.maximized {
            WindowPlacement::Maximized
        } else {
            WindowPlacement::Floating
        })
    }

    /// Its icon through xdg-toplevel-icon (kept: a window shown again gets it again); a compositor without it
    /// shows the app's icon, by the app id's desktop entry.
    fn set_icon(&mut self, icon: Option<&TrayImage>) {
        self.win.icon = icon.cloned();
        self.apply_icon();
    }

    /// Exported to the desktop's global menu where there is one and the compositor takes the window's menu's
    /// address (KDE's appmenu protocol: the bus name and path, as Qt's apps tell it), else drawn by KompozR.
    fn set_menu_bar(&mut self, menus: Option<&[TrayMenuItem]>) -> MenuBarShown {
        let id = self.win.id;
        let address = self.desktop.window_menu_address(id);
        let can = self.globals.appmenu.is_some() && self.win.window.is_some() && address.is_some();
        let shown = self.desktop.set_window_menu(id, menus, None, can);
        if let Some(w) = &mut self.win.window {
            match address.filter(|_| shown == MenuBarShown::Exported) {
                Some((name, path)) => {
                    if w.appmenu.is_none() {
                        w.appmenu = self.globals.appmenu.as_ref().and_then(|m| m.create(&w.surface));
                        if w.appmenu.is_some() {
                            // (Once a window, as its bar leaves it: what tells a drawn bar's absence from a bug.)
                            crate::system::log("kompoz: Wayland: the menu bar is the desktop's global menu");
                        }
                    }
                    if let Some(appmenu) = &w.appmenu {
                        appmenu.set_address(&name, &path);
                    }
                }
                None => w.appmenu = None, // (released: the compositor forgets the address)
            }
            let _ = self.connection.flush();
        }
        shown
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

impl WaylandPlatform {
    /// The window's icon (its `Window`'s, kept in its state) given to the compositor (xdg-toplevel-icon): a square
    /// ARGB8888 buffer, premultiplied; none: the app's again.
    fn apply_icon(&mut self) {
        let (Some(manager), Some(w)) = (&self.globals.toplevel_icons, &mut self.win.window) else { return };
        let made = self.win.icon.as_ref().and_then(|image| {
            let shm = self.globals.shm.as_ref()?;
            let side = image.width.min(image.height);
            let pixels: Vec<u32> = (0..side)
                .flat_map(|y| (0..side).map(move |x| (y * image.width + x) as usize * 4))
                .map(|i| {
                    let p = &image.rgba[i..i + 4];
                    let a = p[3] as u32;
                    let pre = |c: u8| (c as u32 * a + 127) / 255;
                    a << 24 | pre(p[0]) << 16 | pre(p[1]) << 8 | pre(p[2])
                })
                .collect();
            let buffer = decorations::shm_buffer(shm, "kompoz-icon", &pixels, side, side)?;
            let icon = manager.create_icon()?;
            icon.add_buffer(&buffer, 1);
            Some((icon, buffer))
        });
        manager.set_icon(&w.toplevel, made.as_ref().map(|(icon, _)| icon));
        w.surface.commit();
        w.icon = made; // (the one before goes: its icon, then its buffer)
        let _ = self.connection.flush();
    }

    /// An activation token came (asked for by `to_front`): its window activated with it.
    fn activate(&mut self, token: ObjectId, name: &str) {
        let Some(i) = self.activations.iter().position(|(t, _)| t.id() == token) else { return };
        let (made, window) = self.activations.remove(i);
        drop(made);
        self.select(window);
        if let (Some(activation), Some(w)) = (&self.globals.activation, &self.win.window) {
            activation.activate(name, &w.surface);
            let _ = self.connection.flush();
        }
    }

    /// The compositor's bounds on the window's size: none, or its size now (not resizable).
    pub(super) fn apply_resizable(&mut self, resizable: bool) {
        let Some(w) = &self.win.window else { return };
        if resizable {
            w.toplevel.set_min_size(0, 0);
            w.toplevel.set_max_size(0, 0);
        } else {
            w.toplevel.set_min_size(self.win.width, self.win.height);
            w.toplevel.set_max_size(self.win.width, self.win.height);
        }
        w.surface.commit();
        let _ = self.connection.flush();
    }
}

/// A window of the app's on the Wayland connection (the root core's, or a `Window`'s): the platform it shares, by
/// the window's id ("the platform keyed by window id"); each call works on its window.
pub struct WaylandWindow {
    platform: Rc<RefCell<WaylandPlatform>>,
    id: WindowId,
}

impl WaylandWindow {
    fn new(platform: &Rc<RefCell<WaylandPlatform>>, id: WindowId) -> WaylandWindow {
        platform.borrow_mut().select(id);
        WaylandWindow { platform: platform.clone(), id }
    }

    /// The platform, its window selected.
    fn on(&self) -> std::cell::RefMut<'_, WaylandPlatform> {
        let mut p = self.platform.borrow_mut();
        p.select(self.id);
        p
    }
}

impl Drop for WaylandWindow {
    fn drop(&mut self) {
        if let Ok(mut p) = self.platform.try_borrow_mut() {
            p.select(self.id);
            p.close_window();
            p.forget_window(self.id);
        }
    }
}

impl Platform for WaylandWindow {
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
    fn window_action(&mut self, action: WindowAction, x: f32, y: f32) {
        self.on().window_action(action, x, y);
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
    fn to_front(&mut self) {
        self.on().to_front();
    }
    fn set_resizable(&mut self, resizable: bool) {
        self.on().set_resizable(resizable);
    }
    fn window_placement(&self) -> Option<WindowPlacement> {
        self.on().window_placement()
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
    fn set_icon(&mut self, icon: Option<&TrayImage>) {
        self.on().set_icon(icon);
    }
    fn set_menu_bar(&mut self, menus: Option<&[TrayMenuItem]>) -> MenuBarShown {
        self.on().set_menu_bar(menus)
    }
}

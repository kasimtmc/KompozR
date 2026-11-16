//! KompozR's Wayland platform (C++'s wayland_platform.cpp), on our own binding: libwayland-client loaded at run time
//! ([`ffi`]) and the protocols generated from their XML ([`generated`], `cargo xtask wayland`). One xdg-shell
//! toplevel drawn into by Vulkan (`VK_KHR_wayland_surface`), its frames paced by the compositor's frame callbacks;
//! its scale fractional (fractional-scale + viewporter) or whole (the surface's preferred buffer scale, else its
//! outputs'); the compositor's decorations (xdg-decoration) or, without them, the hooks KompozR's own need (the
//! title bar in the metrics, the window's actions, its geometry); the seat's pointer (wheel notches, touchpad
//! scrolls and pinches), keyboard (xkbcommon, compose sequences, client-side repeat), touch screen and input method
//! (text-input-v3); cursors by shape (cursor-shape-v1) or from the cursor theme (libwayland-cursor); the clipboard
//! and drops from other apps (wl_data_device); the screen kept on (idle-inhibit); and the desktop's services
//! ([`DesktopServices`]: the theme, the accent, the text size, the title bar's buttons, the wallpaper's colors,
//! screen readers, a microphone and a camera).
//!
//! Re-entrancy: protocol events are only queued while libwayland dispatches ([`ffi`]); the platform reads them
//! between the core's calls, gathering the core's [`Event`](crate::Event)s. What the host's callbacks need (the
//! cursor, the input method, the clipboard) is kept apart ([`Shared`]), borrowed only briefly, never across a call
//! into the host.
//!
//! The environment:
//! - `KOMPOZ_SCALE`: px per dp instead of the compositor's scale; `KOMPOZ_FONT_SCALE`: the system's text size.
//! - `KOMPOZ_DECORATIONS=kompoz`: KompozR's decorations even where the compositor has its own.
//! - `KOMPOZ_WAYLAND_IGNORE=name,…`: those globals left unbound, as on a compositor without them (what the
//!   fallbacks are tested with: `wp_cursor_shape_manager_v1` for the cursor theme, `wp_fractional_scale_manager_v1`
//!   for whole scales, `zxdg_decoration_manager_v1` for KompozR's decorations).

mod cursor;
mod data_device;
pub mod ffi;
pub mod generated;
mod input;
mod vulkan;
mod window;

use self::cursor::Cursors;
use self::data_device::Clipboard;
use self::ffi::{Connection, Object, ObjectId};
use self::generated::*;
use self::input::{Pinch, TextInput, Wheel};
use self::window::{Output, WindowState};
use super::desktop::DesktopServices;
use super::event_loop::{EventLoop, Waker};
use super::sys;
use super::xkb::{self, ComposeState, Keyboard, RepeatTimer};
use crate::app::{AppConfig, AppCore, DisplayPlatform, Event, Metrics, Platform, WindowAction};
use crate::system::{log, preferred_language};
use kompoz_gpu::device::{Backend, Surface};
use kompoz_graphics::Rect;
use kompoz_runtime::Content;
use kompoz_ui::KompozHost;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use kompoz_ui::platform::{ImeOptions, PlatformCallbacks, PointerIcon, TextEditState};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// The Wayland platform, as `App::run` tries it (first, unless `KOMPOZ_PLATFORM` names another).
pub const PLATFORM: DisplayPlatform = DisplayPlatform { name: "wayland", run };

/// The event loop's token for the display's socket.
const DISPLAY: u32 = 1;

/// The app on the compositor until it quits (C++'s `runWayland`); none where there is no Wayland display.
fn run(config: &AppConfig, content: Rc<dyn Content>) -> Option<ExitCode> {
    std::env::var_os("WAYLAND_DISPLAY").filter(|d| !d.is_empty())?;
    let platform = match WaylandPlatform::connect() {
        Ok(platform) => platform,
        Err(why) => {
            log(&format!("kompoz: Wayland: {why}"));
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

/// The loop (C++'s `run`): the compositor's events and the loop's wakes handled, a frame whenever one is wanted and
/// the last one's frame callback came (asked for before each frame: the compositor says when the next is worth
/// drawing), then a wait for what comes next.
fn run_loop(core: &mut AppCore<WaylandPlatform>) {
    let mut need_frame = true;
    loop {
        let events = core.platform_mut().take_events();
        let mut quit = false;
        for e in events {
            quit |= e == Event::Quit;
            core.handle(&e);
            need_frame = true;
        }
        if quit || core.platform().broken {
            break;
        }
        let p = core.platform_mut();
        if std::mem::take(&mut p.callback_done) {
            need_frame |= p.busy;
        }
        if need_frame && !p.awaiting_callback && p.configured {
            need_frame = false;
            p.before_frame();
            let next = core.frame(None);
            let p = core.platform_mut();
            p.after_frame();
            if next.quit {
                break;
            }
            p.busy = next.busy;
            p.wake_at = if next.busy { None } else { next.wake_at };
        }
        let p = core.platform();
        let now = need_frame && !p.awaiting_callback && p.configured;
        if !now && p.awaiting_callback && !p.connection.has_events() {
            // Waiting for the compositor's beat: the host's work between frames until it is due.
            let due = p.frame_asked_at + p.frame_period();
            let at = sys::monotonic_now();
            if at < due {
                core.idle(due - at);
            }
        }
        if !core.platform_mut().wait(now) {
            break; // (the compositor went away)
        }
    }
    core.quit();
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
    idle_inhibits: Option<ZwpIdleInhibitManagerV1>,
    gestures: Option<ZwpPointerGesturesV1>,
    shm: Option<Rc<WlShm>>,
    subcompositor: Option<WlSubcompositor>,
    seat: Option<WlSeat>,
    // (Last of them: the registry the others came from.)
    registry: Option<WlRegistry>,
}

/// The window's objects (destroyed in this order: roles before the surface). The decoration and the fractional
/// scale are only kept: their events come to the platform, dropping them would end them.
struct Window {
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
    /// px per point (the window's scale), for the input method's area.
    scale: Cell<f64>,
    cursor: RefCell<Cursors>,
    text: RefCell<TextInput>,
    clipboard: RefCell<Clipboard>,
}

/// The host's callbacks on Wayland.
struct WaylandCallbacks(Rc<Shared>);

impl PlatformCallbacks for WaylandCallbacks {
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        Some(self.0.waker.as_fn())
    }

    /// (C++'s textInput: the area from px to points.)
    fn text_input(&self, active: bool, area: Rect, state: &TextEditState, options: &ImeOptions) {
        let k = 1.0 / self.0.scale.get().max(1e-3) as f32;
        let area = [area.left * k, area.top * k, area.width() * k, area.height() * k].map(|v| v.round() as i32);
        self.0.text.borrow_mut().set(active, area, state, options);
        let _ = self.0.connection.flush();
    }

    fn text_input_changed(&self, state: &TextEditState) {
        self.0.text.borrow_mut().changed(state);
        let _ = self.0.connection.flush();
    }

    fn cursor(&self, icon: PointerIcon) {
        self.0.cursor.borrow_mut().set_icon(icon);
        let _ = self.0.connection.flush();
    }

    fn get_clipboard(&self) -> Option<String> {
        self.0.clipboard.borrow_mut().read(&self.0.connection)
    }

    fn set_clipboard(&self, text: &str) {
        self.0.clipboard.borrow_mut().write(text);
        let _ = self.0.connection.flush();
    }

    fn open_uri(&self, uri: &str) {
        super::desktop::open_uri(uri);
    }
}

/// The Wayland platform (C++'s WaylandPlatform). Its fields are dropped in order: the window and the seat's objects,
/// then the globals, then the connection.
pub struct WaylandPlatform {
    /// (First: its threads end before the rest goes.)
    desktop: DesktopServices,
    window: Option<Window>,
    devices: Devices,
    outputs: Vec<OutputObject>,
    globals: Globals,
    shared: Rc<Shared>,
    keyboard: Keyboard,
    xkb: xkb::Context,
    event_loop: EventLoop,
    /// The core's events, gathered from the protocol's.
    events: VecDeque<Event>,
    title: String,
    /// The window's size in points (the compositor's logical pixels, a dp each at its own scale).
    width: i32,
    height: i32,
    /// A configure's size (0: the app's own).
    pending_width: i32,
    pending_height: i32,
    configured: bool,
    /// fractional-scale's scale, in 120ths (0: none told).
    scale120: u32,
    /// Else the whole scale the surface is given.
    buffer_scale: i32,
    /// The window's state as the last configure told it (the next one's until its xdg_surface configure).
    pending_state: WindowState,
    state: WindowState,
    /// What the window manager does (xdg_toplevel's wm_capabilities: all until it says).
    can_minimize: bool,
    can_maximize: bool,
    /// KompozR decorates the window (the compositor has no decorations, or KOMPOZ_DECORATIONS=kompoz): the title bar
    /// in the metrics, the window geometry set; the edges around it are KompozR's own decorations' (WP5).
    own_decorations: bool,
    /// The drawable's size, px (the GPU device reads it).
    drawable: Rc<Cell<(i32, i32)>>,
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
    /// What is dragged over the window (an offer) and where.
    drag: Option<ObjectId>,
    drag_x: f64,
    drag_y: f64,
    // The loop.
    connection: Connection,
    busy: bool,
    awaiting_callback: bool,
    callback_done: bool,
    /// When the last frame callback was asked for (monotonic ns).
    frame_asked_at: i64,
    /// The next frame's wake (a coroutine's delay), when nothing animates.
    wake_at: Option<i64>,
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
            scale: Cell::new(1.0),
            cursor: RefCell::new(Cursors::new(connection.clone())),
            text: RefCell::new(TextInput::default()),
            clipboard: RefCell::new(Clipboard::default()),
        });
        let mut p = WaylandPlatform {
            desktop: DesktopServices::new(),
            window: None,
            devices: Devices::default(),
            outputs: Vec::new(),
            globals: Globals::default(),
            shared,
            keyboard: Keyboard::new(compose),
            xkb,
            event_loop,
            events: VecDeque::new(),
            title: "KompozR".into(),
            width: 1,
            height: 1,
            pending_width: 0,
            pending_height: 0,
            configured: false,
            scale120: 0,
            buffer_scale: 1,
            pending_state: WindowState::default(),
            state: WindowState::default(),
            can_minimize: true,
            can_maximize: true,
            own_decorations: false,
            drawable: Rc::new(Cell::new((1, 1))),
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
            connection,
            busy: false,
            awaiting_callback: false,
            callback_done: false,
            frame_asked_at: 0,
            wake_at: None,
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
    }

    /// The core's events: what was read dispatched and handled first.
    fn take_events(&mut self) -> VecDeque<Event> {
        if let Err(e) = self.connection.dispatch_pending() {
            self.lost(&e.to_string());
        }
        self.pump();
        std::mem::take(&mut self.events)
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
            self.events.push_back(Event::Quit);
        }
        if ready.woken || ready.deadline {
            self.events.push_back(Event::Wake);
        }
        if ready.deadline {
            self.shared.cursor.borrow_mut().animate(sys::monotonic_now());
        }
        // What the desktop's services told meanwhile (they wake the loop).
        self.events.extend(self.desktop.take_events());
        if ready.repeats > 0 {
            let typing = self.shared.text.borrow().active;
            let repeated = self.keyboard.repeated(ready.repeats, sys::monotonic_now(), typing);
            self.events.extend(repeated);
        }
        match self.connection.dispatch_pending() {
            Ok(_) => true,
            Err(e) => {
                self.lost(&e.to_string());
                false
            }
        }
    }

    /// When the loop is to wake by itself: the next coroutine delay, the cursor's next animation frame.
    fn deadline(&self) -> Option<i64> {
        let cursor = self.shared.cursor.borrow().next_frame_at();
        match (self.wake_at, cursor) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
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
    fn before_frame(&mut self) {
        let Some(w) = &mut self.window else { return };
        w.frame_callback = w.surface.frame();
        self.awaiting_callback = w.frame_callback.is_some();
        self.frame_asked_at = sys::monotonic_now();
    }

    /// After a frame: the surface committed (with the frame callback, if the frame didn't present).
    fn after_frame(&mut self) {
        if let Some(w) = &self.window {
            w.surface.commit();
        }
    }

    /// The refresh period of the outputs the window is on, ns.
    fn frame_period(&self) -> i64 {
        (1e9 / self.refresh_rate() as f64) as i64
    }

    /// A protocol event (C++'s listeners).
    fn on_event(&mut self, sender: ObjectId, event: generated::Event) {
        use generated::Event as E;
        match event {
            E::WlRegistry(e) => self.on_registry(e),
            E::XdgWmBase(XdgWmBaseEvent::Ping { serial }) => {
                if let Some(base) = &self.globals.wm_base {
                    base.pong(serial);
                }
            }
            E::WlOutput(e) => self.on_output(sender, e),
            E::WlSurface(e) => self.on_surface(sender, e),
            E::WpFractionalScaleV1(e) => self.on_fractional_scale(e),
            E::XdgSurface(e) => self.on_xdg_surface(e),
            E::XdgToplevel(e) => self.on_toplevel(e),
            E::ZxdgToplevelDecorationV1(e) => self.on_decoration(e),
            E::WlCallback(WlCallbackEvent::Done { .. }) => {
                if let Some(w) = &mut self.window
                    && w.frame_callback.as_ref().is_some_and(|c| c.id() == sender)
                {
                    w.frame_callback = None; // (destroyed: the compositor is done with it)
                    self.awaiting_callback = false;
                    self.callback_done = true;
                }
            }
            E::WlSeat(e) => self.on_seat(e),
            E::WlPointer(e) => self.on_pointer(e),
            E::WlKeyboard(e) => self.on_keyboard(e),
            E::WlTouch(e) => self.on_touch(e),
            E::ZwpPointerGesturePinchV1(e) => self.on_pinch(e),
            E::ZwpTextInputV3(e) => self.on_text_input(e),
            E::WlDataOffer(e) => self.on_data_offer(sender, e),
            E::WlDataDevice(e) => self.on_data_device(e),
            E::WlDataSource(e) => self.on_data_source(sender, e),
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
                    "wl_data_device_manager" => {
                        self.shared.clipboard.borrow_mut().manager = bind(registry, name, version, 3);
                    }
                    "wl_shm" => g.shm = bind(registry, name, version, 1).map(Rc::new),
                    "wl_subcompositor" => g.subcompositor = bind(registry, name, version, 1),
                    "zwp_pointer_gestures_v1" => g.gestures = bind(registry, name, version, 1),
                    "zwp_idle_inhibit_manager_v1" => g.idle_inhibits = bind(registry, name, version, 1),
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
    /// cursor's shape device or theme, the touchpad's pinch.
    fn attach_seat(&mut self) {
        let g = &self.globals;
        let Some(seat) = &g.seat else { return };
        self.shared.clipboard.borrow_mut().attach(seat);
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
            self.events.extend(lifted);
            changed = true;
        }
        self.globals.seat = Some(seat);
        self.attach_seat();
        if changed {
            self.events.push_back(Event::DevicesChanged);
        }
    }
}

impl Drop for WaylandPlatform {
    fn drop(&mut self) {
        self.desktop.stop();
        self.close_window();
        // (What the callbacks hold goes with the platform's objects, before the globals they came from.)
        self.shared.cursor.borrow_mut().release_pointer();
        *self.shared.text.borrow_mut() = TextInput::default();
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

    fn open_window(&mut self, app: &AppConfig, backend: Backend) -> Result<Surface, String> {
        if backend != Backend::Vulkan {
            return Err(format!("no {} on Wayland", backend.name()));
        }
        let made = self.make_window(app);
        if let Err(why) = made {
            self.close_window();
            return Err(why);
        }
        let size = self.drawable.clone();
        let w = self.window.as_ref().ok_or("no window")?;
        Ok(Surface {
            vulkan: Some(vulkan::surface(&self.connection, &w.surface)),
            drawable_size: Some(Box::new(move || size.get())),
            ..Surface::default()
        })
    }

    fn close_window(&mut self) {
        self.window = None;
        self.configured = false;
        self.awaiting_callback = false;
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
        let (pw, ph) = self.drawable.get();
        Metrics {
            pixel_width: pw,
            pixel_height: ph,
            point_width: self.width,
            point_height: self.height,
            scale: number("KOMPOZ_SCALE").filter(|s| *s > 0.0).unwrap_or(scale),
            point_to_px: scale,
            font_scale: match number("KOMPOZ_FONT_SCALE") {
                Some(s) => s.max(0.5),
                None => self.desktop.font_scale().unwrap_or(1.0),
            },
            refresh_rate: self.refresh_rate(),
            safe_area: Some(Rect::new(0.0, 0.0, self.width as f32, self.height as f32)),
            dark_theme: self.desktop.dark_theme(),
            language: preferred_language().unwrap_or_default(),
            title_bar: (self.own_decorations && !self.state.fullscreen).then(|| self.title_bar()),
        }
    }

    fn pixel_size(&self) -> (i32, i32) {
        self.drawable.get()
    }

    fn set_window_size(&mut self, width: i32, height: i32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.apply_size();
        self.events.push_back(Event::Resized);
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

    /// The callbacks; the desktop's services started (their threads wake the loop).
    fn attach(&mut self, _host: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        self.desktop.start(&self.title, self.event_loop.waker().as_fn());
        Rc::new(WaylandCallbacks(self.shared.clone()))
    }

    fn metrics_updated(&mut self, _host: &KompozHost) {
        self.desktop.metrics_updated();
    }

    fn handled(&mut self, host: &KompozHost, event: &Event) {
        self.desktop.handled(host, event);
    }

    /// keep_screen_on: an idle inhibitor on the window while it is laid out (C++'s); the semantics tree to the
    /// screen readers (the window at the origin: Wayland doesn't tell a window where it is).
    fn composed(&mut self, host: &KompozHost, redraw: bool, frame_time: i64) {
        let window = Rect::new(0.0, 0.0, self.width as f32, self.height as f32);
        self.desktop.composed(host, redraw, frame_time, window, (1.0 / self.scale()) as f32, &self.title);
        let on = host.keep_screen_on();
        let Some(w) = &mut self.window else { return };
        if on == w.inhibitor.is_some() {
            return;
        }
        w.inhibitor = match (&self.globals.idle_inhibits, on) {
            (Some(manager), true) => manager.create_inhibitor(&w.surface),
            _ => None,
        };
    }

    fn wants_frames(&self) -> bool {
        self.desktop.wants_frames()
    }
}

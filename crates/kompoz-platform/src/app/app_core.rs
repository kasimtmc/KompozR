//! The app core (C++'s app_core.cpp): a KompozR app on any platform. The app composed by the host, drawn by the GPU
//! renderer into a kept frame where only what changed is drawn again, shown in the platform's window; the
//! platform's events turned into the host's input; frames paced on the display's beat; a script driving it
//! (`KOMPOZ_SCRIPT`); the environment's flags. What is the platform's own comes through [`Platform`]; nothing here
//! knows which one it is on.
//!
//! The environment:
//! - `KOMPOZ_GPU=vulkan|metal|webgpu|auto`: the renderer's backend (auto: the platform's order, the first that
//!   starts).
//! - `KOMPOZ_SCRIPT`: see [`script`](super::script). `KOMPOZ_STATE_FILE=path`: the saved state written there at exit
//!   and read at start (`remember_saveable` on the desktop, as Android ends and brings back an app).
//! - `KOMPOZ_TRACE`: what each recomposition ran. `KOMPOZ_PROFILE`: where frame time goes.
//! - `KOMPOZ_DAMAGE_CHECK`: each frame's kept pixels compared with all of it drawn afresh (stale pixels logged).
//!   `KOMPOZ_KEEP_CHECK`: each frame's recording keeping nodes compared with one drawing them all.
//!   `KOMPOZ_DAMAGE_SHOW`: what was drawn again outlined in the window. `KOMPOZ_FULL_REDRAW`: all of every frame
//!   drawn.
//! - `KOMPOZ_LAYOUT_DIRECTION=rtl|ltr`, `KOMPOZ_LANGUAGE=tr|en`: instead of the system language's.

use super::menu_bar::MenuBarFrame;
use super::profile::{DrawCounts, Profile};
use super::regions::{ShownChanges, drawn_area, frame_changes};
use super::script::{Command, Script, WindowEvent, parse_command};
use super::title_bar::{WindowActions, WindowFrame};
use super::windows::menu_item;
use super::{AppConfig, Event, MenuBarShown, MouseButton, Platform, TitleBar, TrayImage, WindowAction, WindowSpec};
use crate::image_loader::{load_image_async, load_image_file, start_image_disk_cache, stop_image_loader};
use crate::system::{app_cache_directory, log, read_file};
use kompoz_coroutines::CoroutineContext;
use kompoz_gpu::Renderer;
use kompoz_gpu::device::{self, Backend, ClearColor, Device, Format, IRect, Surface, Texture, TextureUsage};
use kompoz_gpu::renderer::RendererBitmaps;
use kompoz_graphics::{
    Color, Density, Dp, DpSize, ImageBitmap, IntSize, LayoutDirection, Matrix, Offset, Rect, Size, encode_png,
};
use kompoz_lifecycle::LifecycleState;
use kompoz_runtime::clock::Instant;
use kompoz_runtime::{Content, MutableState, RecompositionStats, kompozable_lambda, mutable_state_of};
use kompoz_ui::clipboard::{ClipEntry, ClipMetadata};
use kompoz_ui::draganddrop::DragAndDropTransferData;
use kompoz_ui::graphics_context::{CornerRadii, GraphicsContext, ShadowLight};
use kompoz_ui::input::{Key, KeyEvent, KeyEventType, PointerButton, PointerButtons, PointerEventType, PointerType};
use kompoz_ui::platform::{
    HapticFeedbackType, ImageRequestSize, ImeOptions, Insets, PlatformCallbacks, PointerIcon, TextEditState,
    TextToolbarActions,
};
use kompoz_ui::strings::{Strings, is_rtl_language};
use kompoz_ui::text_input::EditCommand;
use kompoz_ui::window::{
    ApplicationScope, ComposeWindow, FrameWindowScope, WindowPlacement, WindowPosition, WindowProperties, WindowRequest,
};
use kompoz_ui::{DisplayList, KompozHost};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// What the platform's frame loop does after a frame (C++'s Next).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Next {
    /// The next frame right away (something animates, recomposes, or a script runs): don't wait for events.
    pub busy: bool,
    /// Else a wake then (a coroutine's delay), on the platform's clock; none: wait for events only.
    pub wake_at: Option<i64>,
    /// The app is done.
    pub quit: bool,
}

/// Frames of a script in virtual time: 60 a second.
const SCRIPT_FRAME_NS: i64 = 16_666_667;

/// What makes the GPU device on a backend for a window's surface: kompoz-gpu's [`device::create`] unless the
/// platform (or a test, wrapping it) gives its own.
pub type DeviceFactory = Box<dyn Fn(Backend, Surface) -> Result<Box<dyn Device>, String>>;

/// The renderer and what it draws into.
struct Gpu {
    renderer: Rc<RefCell<Renderer>>,
    bitmaps: Rc<RendererBitmaps>,
    /// The kept frame.
    frame: Option<Texture>,
    timed: bool,
}

/// The flags of the environment.
#[derive(Clone, Copy, Debug, Default)]
struct Flags {
    trace: bool,
    profile: bool,
    damage_check: bool,
    keep_check: bool,
    damage_show: bool,
    full_redraw: bool,
}

/// What a frame drew.
#[derive(Clone, Copy, Debug, Default)]
struct Drawn {
    /// To present.
    shown: bool,
    /// Drawn again, of the screen.
    area: f32,
}

/// Whether the app's lifecycle follows the app going to the background and back (phones) rather than its window.
const PHONE: bool = cfg!(any(target_os = "android", target_os = "ios"));

/// The app's lifecycle state on desktops, of its window (CMP's and C++'s mapping): created
/// while it is minimized or hidden, resumed while it has the focus, started while it is shown without it. (The host
/// dropped: destroyed.)
pub fn lifecycle_state_of(focused: bool, minimized: bool, hidden: bool) -> LifecycleState {
    if minimized || hidden {
        LifecycleState::Created
    } else if focused {
        LifecycleState::Resumed
    } else {
        LifecycleState::Started
    }
}

/// Which of the app's compositions a core runs.
#[derive(Clone)]
enum Kind {
    /// The app's own window (`App::run`): closing it ends the app (C++'s).
    Main,
    /// The app's composition without a window (`application { }`, Compose Desktop's): its GPU device draws only into
    /// textures (the app's bitmaps, its tray icons).
    Application,
    /// A `Window` of the app's: its request's content in a host of its own.
    Window(Rc<WindowRequest>),
}

/// What of a `Window`'s request and state the platform's window was last given (or reported).
#[derive(Clone, Debug, Default)]
struct Applied {
    title: String,
    size: DpSize,
    minimized: bool,
    placement: WindowPlacement,
    position: WindowPosition,
    front: u64,
    /// The menu bar's version the platform was last given (none: not yet, or to give again).
    menu_bar: Option<u64>,
    properties: Option<WindowProperties>,
}

/// The app core on platform `P` (C++'s AppCore): made with the app's configuration and content (the main window's,
/// [`new`](AppCore::new); the app's composition's without one, [`application`](AppCore::application); a `Window`'s,
/// [`window`](AppCore::window)), then [`start`](AppCore::start)ed; the platform's loop hands it events
/// ([`handle`](AppCore::handle)) and asks it for frames ([`frame`](AppCore::frame)), pacing them by
/// [`frame_due`](AppCore::frame_due); [`quit`](AppCore::quit) as it goes. The app's cores together:
/// [`AppWindows`](super::AppWindows).
pub struct AppCore<P: Platform> {
    platform: P,
    config: AppConfig,
    content: Rc<dyn Content>,
    kind: Kind,
    host: Option<KompozHost>,
    gpu: Option<Gpu>,
    /// The renderer, for the host's callbacks (another as a hidden window is shown again).
    renderer_slot: Rc<RefCell<Weak<RefCell<Renderer>>>>,
    /// Between the device's begin_frame and end_frame (what is drawn into bitmaps then is drawn in that frame).
    in_frame: Rc<Cell<bool>>,
    window_open: bool,
    /// A `Window`'s: shown (its `visible`); hidden, its host goes on without a GPU device or a platform window.
    shown: bool,
    applied: Applied,
    /// The window's title (KompozR's title bar shows it).
    title: MutableState<String>,
    /// `exit_application` asked (the app ends after this frame).
    exit: Rc<Cell<bool>>,
    /// The host's handle once started (what `exit_application` wakes the loop with).
    exit_wake: Rc<RefCell<Option<kompoz_ui::HostHandle>>>,
    /// A script's "@Title: command"s, for the windows they name (the app's `AppWindows` hands them on).
    script_out: Vec<(String, String)>,
    /// The app's virtual clock while its script runs: a window's frames are at its times (none: its own clock).
    virtual_clock: Option<i64>,
    /// A `Window`'s icon given to the platform once.
    icon_told: bool,
    /// Frames so far.
    frames: u64,
    /// The frame recorded; only what changed in it is drawn again into the kept frame.
    display: DisplayList,
    /// What changed in each frame shown (for a swapchain image holding an earlier frame).
    shown_changes: ShownChanges,
    /// px per dp, px per window point.
    scale: f32,
    point_to_px: f32,
    /// KompozR's title bar, as the platform says (Metrics::title_bar).
    title_bar: MutableState<Option<TitleBar>>,
    /// Who shows a `Window`'s menu bar, as the platform last said (KompozR draws it where none other does).
    menu_bar_shown: MutableState<MenuBarShown>,
    /// What the title bar asked of the window while the host was busy (its input, its frame), done after.
    window_actions: Rc<RefCell<Vec<(WindowAction, Offset)>>>,
    quit: bool,
    script: Script,
    /// Added to the platform's clock after a script whose virtual time got ahead of it.
    clock_offset: i64,
    flags: Flags,
    profile: Profile,
    /// The window needs the kept pixels again (exposed, first frame).
    show_frame: bool,
    /// Frames still to draw whole and show after the window was resized.
    resize_frames: i32,
    /// The window's size, scale, insets, theme or language changed: read once before the next frame or input.
    metrics_changed: bool,
    focused: bool,
    /// The window minimized, hidden (with focused: the app's lifecycle on desktops).
    minimized: bool,
    hidden: bool,
    /// A microphone and a camera looked for (after the first frame shown).
    media_probed: bool,
    /// The display's refresh interval: frames not shown are paced to it.
    frame_period_ns: i64,
    /// The last frame's time, on the display's beat.
    last_frame_time: Option<i64>,
    /// A drag from another app coming in: what it drops, gathered until it is complete.
    dropping: bool,
    drop_files: Vec<String>,
    drop_text: String,
    drop_at: Offset,
    /// The app's own drag was dropped (`Event::DropOwn`): what it carries is the host's.
    drop_own: bool,
    /// What the drag coming in offers, as the platform described it (none: not told).
    drop_offered: Option<Vec<String>>,
    /// What it dropped, as the platform reads it (`Event::DropData`).
    drop_entry: Option<ClipEntry>,
    /// A drag of the app's own the platform has (it left the window): its MIME types (what the window is told it
    /// carries as it comes back over it).
    own_drag: Rc<RefCell<Option<Vec<String>>>>,
    /// `KOMPOZ_DAMAGE_CHECK`'s frames compared and those that differed (counted as the pixels come back).
    damage_frames: Rc<Cell<i32>>,
    damage_wrong: Rc<Cell<i32>>,
    /// Pixels read back still to come (a screenshot's, a damage check's: WebGPU's come a turn or two later).
    reading: Rc<Cell<usize>>,
    /// Screenshots read back, to write (their paths and PNG bytes).
    written: Rc<RefCell<Vec<(String, Option<Vec<u8>>)>>>,
    keep_frames: i32,
    keep_wrong: i32,
    traced: RecompositionStats,
    device_factory: Option<DeviceFactory>,
}

impl<P: Platform> AppCore<P> {
    /// A core for `config`'s app composing `content`, on `platform` (not started).
    pub fn new(platform: P, config: AppConfig, content: Rc<dyn Content>) -> AppCore<P> {
        let mut display = DisplayList::new();
        display.set_allow_moves(true); // (the kept frame moves its pixels)
        display.set_keep_nodes(true); // (nodes that didn't change aren't drawn again)
        let title = mutable_state_of(config.title.clone());
        AppCore {
            platform,
            config,
            content,
            kind: Kind::Main,
            host: None,
            gpu: None,
            renderer_slot: Rc::new(RefCell::new(Weak::new())),
            in_frame: Rc::new(Cell::new(false)),
            window_open: false,
            shown: true,
            applied: Applied::default(),
            title,
            exit: Rc::new(Cell::new(false)),
            exit_wake: Rc::default(),
            script_out: Vec::new(),
            virtual_clock: None,
            icon_told: false,
            frames: 0,
            display,
            shown_changes: ShownChanges::default(),
            scale: 1.0,
            point_to_px: 1.0,
            title_bar: mutable_state_of(None),
            menu_bar_shown: mutable_state_of(MenuBarShown::Drawn),
            window_actions: Rc::default(),
            quit: false,
            script: Script::default(),
            clock_offset: 0,
            flags: Flags::default(),
            profile: Profile::default(),
            show_frame: true,
            resize_frames: 0,
            metrics_changed: false,
            focused: true,
            minimized: false,
            hidden: false,
            media_probed: false,
            frame_period_ns: 16_666_667,
            last_frame_time: None,
            dropping: false,
            drop_files: Vec::new(),
            drop_text: String::new(),
            drop_at: Offset::Zero,
            drop_own: false,
            drop_offered: None,
            drop_entry: None,
            own_drag: Rc::default(),
            damage_frames: Rc::default(),
            damage_wrong: Rc::default(),
            reading: Rc::default(),
            written: Rc::default(),
            keep_frames: 0,
            keep_wrong: 0,
            traced: RecompositionStats::default(),
            device_factory: None,
        }
    }

    /// A core for the app's composition without a window of its own (`application { }`, Compose Desktop's): `content`
    /// given the app's scope (its `exit_application` ends the app after the frame it is asked in), on `platform`
    /// (the app's: its tray icons, its notifications); its windows are `Window`s ([`AppWindows`](super::AppWindows)
    /// opens them). Not started.
    pub fn application(platform: P, config: AppConfig, content: Rc<dyn Fn(&ApplicationScope)>) -> AppCore<P> {
        let exit = Rc::new(Cell::new(false));
        let exit_wake: Rc<RefCell<Option<kompoz_ui::HostHandle>>> = Rc::default();
        let scope = {
            let (exit, wake) = (exit.clone(), exit_wake.clone());
            ApplicationScope::new(Rc::new(move || {
                exit.set(true);
                if let Some(h) = wake.borrow().as_ref() {
                    h.wake();
                }
            }))
        };
        let content: Rc<dyn Content> = Rc::new(kompozable_lambda!(move || content(&scope)));
        let mut core = AppCore::new(platform, config, content);
        core.kind = Kind::Application;
        core.exit = exit;
        core.exit_wake = exit_wake;
        core
    }

    /// A core for a `Window` of the app's (C++'s per-window AppCore): its request's content in a host of its own
    /// (the caller's locals provided), on `platform` (the window's), its title and state kept as the request has
    /// them ([`sync_window`](AppCore::sync_window)), the user closing it its `on_close_request`. `app`: the app's
    /// configuration (its title names the caches). Not started.
    pub fn window(platform: P, app: &AppConfig, request: Rc<WindowRequest>) -> AppCore<P> {
        let mut config = app.clone();
        let size = request.state().size();
        if size.is_specified() {
            (config.width, config.height) = (size.width, size.height);
        }
        let content: Rc<dyn Content> = {
            let request = request.clone();
            let scope = FrameWindowScope::new(ComposeWindow::new(&request));
            Rc::new(kompozable_lambda!(move || {
                let content = request.content();
                content(&scope);
            }))
        };
        let mut core = AppCore::new(platform, config, content);
        core.title.set(request.title());
        core.applied.title = request.title();
        core.shown = request.visible();
        core.kind = Kind::Window(request);
        core
    }

    /// Whether this is the app's root core (the main window's, or the app's composition's): what is the process's
    /// (the GPU layer's log, the image caches, the bitmaps' renderer, the state file, deep links, the script) is its
    /// own.
    fn is_root(&self) -> bool {
        !matches!(self.kind, Kind::Window(_))
    }

    /// A `Window`'s request (none: the main window's core, the app's composition's).
    pub fn request(&self) -> Option<&Rc<WindowRequest>> {
        match &self.kind {
            Kind::Window(r) => Some(r),
            _ => None,
        }
    }

    /// Whether it is the app's composition without a window (`application { }`).
    pub fn is_application(&self) -> bool {
        matches!(self.kind, Kind::Application)
    }

    /// The window's title now.
    pub fn title(&self) -> String {
        self.title.peek()
    }

    /// Frames so far.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// What `exit_application` sets (the app ends after the frame it is set in).
    pub fn exit_flag(&self) -> Rc<Cell<bool>> {
        self.exit.clone()
    }

    /// Makes the GPU device with `factory` instead of kompoz-gpu's `device::create` (before `start`).
    pub fn set_device_factory(&mut self, factory: DeviceFactory) {
        self.device_factory = Some(factory);
    }

    pub fn platform(&self) -> &P {
        &self.platform
    }

    pub fn platform_mut(&mut self) -> &mut P {
        &mut self.platform
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// The host (once started).
    pub fn host(&self) -> Option<&KompozHost> {
        self.host.as_ref()
    }

    /// The title bar KompozR is to draw, as the platform last said (read: what reads it composes again when it
    /// changes).
    pub fn title_bar(&self) -> MutableState<Option<TitleBar>> {
        self.title_bar.clone()
    }

    /// What the title bar asks of the window, at `at` (px in the window): to the platform, in its points.
    pub fn window_action(&mut self, action: WindowAction, at: Offset) {
        let k = 1.0 / self.point_to_px;
        self.platform.window_action(action, at.x * k, at.y * k);
    }

    /// What the title bar asked while the host was busy, done in order (C++'s WindowFrame called the platform from
    /// inside the host; here the platform is the core's, so the bar's requests wait for the host to be done).
    fn do_window_actions(&mut self) {
        let asked = std::mem::take(&mut *self.window_actions.borrow_mut());
        for (action, at) in asked {
            self.window_action(action, at);
        }
    }

    fn logf(&self, message: &str) {
        self.platform.log(message);
    }

    fn flag(&self, name: &str) -> bool {
        self.platform.option(name).is_some()
    }

    fn now(&self) -> u64 {
        self.platform.now().max(0) as u64
    }

    /// Frame and input times are on one clock: a script's virtual one while it runs, the platform's after.
    fn clock(&self) -> i64 {
        self.platform.now() + self.clock_offset
    }

    fn event_time(&self, timestamp: i64) -> i64 {
        if self.script.running() && !self.script.realtime {
            self.script.clock
        } else {
            (if timestamp < 0 { self.platform.now() } else { timestamp }) + self.clock_offset
        }
    }

    /// A position in the window's points, in px.
    fn to_px(&self, x: f32, y: f32) -> Offset {
        Offset::new(x * self.point_to_px, y * self.point_to_px)
    }

    /// What a drag coming over the window may carry, as the targets are told: the app's own drag's MIME types (one
    /// the platform has, coming back), else what drops from other apps are taken as (which isn't told until the
    /// drop).
    fn drop_types(&self) -> Vec<String> {
        if let Some(own) = self.own_drag.borrow().clone() {
            return own;
        }
        self.drop_offered.clone().unwrap_or_else(|| vec!["text/uri-list".into(), "text/plain".into()])
    }

    // ---- Starting ----

    /// The app's window opened and the GPU renderer started on the first backend that does, the host made and
    /// given its callbacks (C++'s `start`); false if the app can't run (logged, and shown when not scripted).
    pub fn start(&mut self) -> bool {
        if self.is_root() {
            device::set_logger(Some(log));
            start_image_disk_cache(&self.config.title, self.config.image_disk_cache_bytes);
            // Which GPU draws, for all the app's windows (its gpu_priority; KOMPOZ_GPU_PRIORITY instead).
            let mut priority = self.config.gpu_priority;
            if let Some(asked) = self.platform.option("KOMPOZ_GPU_PRIORITY") {
                match asked.trim() {
                    "auto" => priority = device::GpuPriority::Auto,
                    "integrated" => priority = device::GpuPriority::Integrated,
                    "discrete" => priority = device::GpuPriority::Discrete,
                    other => self.logf(&format!(
                        "kompoz: KOMPOZ_GPU_PRIORITY={other}: not auto, integrated or discrete: the app's own"
                    )),
                }
            }
            device::set_gpu_priority(priority);
        }
        self.flags = Flags {
            trace: self.flag("KOMPOZ_TRACE"),
            profile: self.flag("KOMPOZ_PROFILE"),
            damage_check: self.flag("KOMPOZ_DAMAGE_CHECK"),
            keep_check: self.flag("KOMPOZ_KEEP_CHECK"),
            damage_show: self.flag("KOMPOZ_DAMAGE_SHOW"),
            full_redraw: self.flag("KOMPOZ_FULL_REDRAW"),
        };
        // (The platform's scale before the window opens: a `Window`'s place is worked out in dp from it.)
        if matches!(self.kind, Kind::Window(_)) {
            let m = self.platform.metrics();
            (self.scale, self.point_to_px) = (if m.scale > 0.0 { m.scale } else { 1.0 }, m.point_to_px.max(1e-3));
        }
        let scripted = self.flag("KOMPOZ_SCRIPT"); // (an error is logged, not shown in a box to close)
        // KOMPOZ_GPU's first word.
        let choice = self.platform.option("KOMPOZ_GPU").unwrap_or_default();
        let choice = choice.split_whitespace().next().unwrap_or("").to_string();
        // The backends asked for, each tried on a window of its own: the first that starts draws. Asked for by
        // name, one that doesn't start is the app's failure; "auto" goes on to the next.
        let order = self.gpu_order(&choice);
        for &backend in &order {
            if self.start_gpu(backend) || order.len() == 1 {
                break;
            }
        }
        if self.gpu.is_none() {
            self.logf(&format!(
                "kompoz: no GPU renderer started (KOMPOZ_GPU={})",
                if choice.is_empty() { "auto" } else { &choice }
            ));
            if !scripted {
                self.platform.show_error("No graphics device could be started (Vulkan, Metal or WebGPU).");
            }
            return false;
        }
        let gpu = self.gpu.as_ref().expect("a renderer");
        // The pipelines it made in earlier runs, made again ahead of the frames that need them.
        let backend = gpu.renderer.borrow().device().backend();
        if let Some(dir) = app_cache_directory(&self.config.title)
            && self.is_root()
        {
            gpu.renderer.borrow_mut().warm_pipelines(&dir.join(format!("pipelines-{}.list", backend.name())));
        }
        if self.is_root() {
            // (The app's bitmaps drawn by its root's renderer, which lives as long as the app: a window's draws them
            // read back from it.)
            kompoz_ui::draw::set_bitmap_renderer(Some(gpu.bitmaps.clone()));
        }
        // The app's content in the window's frame: below KompozR's title bar while the platform reports one, what
        // the bar asks of the window queued for the core to do once the host is done. (The app's composition
        // without a window: as it is.)
        let content = self.content.clone();
        let (title_bar, title) = (self.title_bar.clone(), self.title.clone());
        let queue = self.window_actions.clone();
        let act: WindowActions = Rc::new(move |action, at| queue.borrow_mut().push((action, at)));
        // (A `Window`'s menu bar between the title bar and the content while KompozR draws it.)
        let menu_bar = match &self.kind {
            Kind::Window(request) => {
                Some(MenuBarFrame { request: Rc::downgrade(request), shown: self.menu_bar_shown.clone() })
            }
            _ => None,
        };
        let framed = kompozable_lambda!(move || {
            WindowFrame(title_bar.get(), title.get(), act.clone(), menu_bar.clone(), content.clone());
        });
        // (The system's word on animations' durations, where it has one: the effects' coroutine context's.)
        let effects = self.platform.motion_duration_scale().map(CoroutineContext::from).unwrap_or_default();
        let host = match &self.kind {
            Kind::Main => KompozHost::with_effect_coroutine_context(framed, effects),
            Kind::Application => {
                let content = self.content.clone();
                KompozHost::with_effect_coroutine_context(kompozable_lambda!(move || content.compose()), effects)
            }
            Kind::Window(request) => {
                let host = KompozHost::new_window_with_effect_coroutine_context(framed, request.locals(), effects);
                // The window's keys: an open menu of the drawn menu bar's first (it has the keyboard), then the
                // window's own handlers around its focused path, then the menu bar's shortcuts and mnemonics
                // (where the platform's own menu doesn't run the shortcuts).
                let (preview, after) = (Rc::downgrade(request), Rc::downgrade(request));
                let shown = self.menu_bar_shown.clone();
                host.set_window_key_handlers(Some((
                    Rc::new(move |e| {
                        preview.upgrade().is_some_and(|r| r.menu_bar_preview_key(e) || r.on_preview_key_event(e))
                    }),
                    Rc::new(move |e| {
                        let accelerators = shown.peek() != MenuBarShown::Native;
                        after.upgrade().is_some_and(|r| r.on_key_event(e) || r.menu_bar_key(e, accelerators))
                    }),
                )));
                host
            }
        };
        let callbacks = AppCallbacks {
            platform: self.platform.attach(&host),
            renderer: self.renderer_slot.clone(),
            in_frame: self.in_frame.clone(),
            own_drag: self.own_drag.clone(),
        };
        host.set_callbacks(Rc::new(callbacks));
        host.set_keep_check(self.flags.keep_check);
        *self.exit_wake.borrow_mut() = Some(host.handle());
        if self.is_root() {
            // Deep links: an address given on the command line.
            for arg in self.config.args.iter().skip(1) {
                if arg.contains("://") {
                    host.open_deep_link(arg);
                }
            }
            if let Some(path) = self.platform.option("KOMPOZ_STATE_FILE")
                && let Some(state) = read_file(&path)
            {
                host.restore_state(&state);
            }
        }
        self.platform.started(&host);
        self.host = Some(host);
        if self.is_root() {
            self.script = Script::parse(&self.platform.option("KOMPOZ_SCRIPT").unwrap_or_default());
        }
        if let Kind::Window(request) = &self.kind {
            let state = request.state();
            let (placement, minimized, position, size) = state.peek();
            self.applied = Applied {
                title: request.title(),
                size,
                minimized,
                placement,
                position,
                front: state.to_front_requests(),
                menu_bar: None,
                properties: Some(request.properties()),
            };
        }
        self.update_metrics();
        true
    }

    /// The window as it opens: the main one's from the app's configuration, a `Window`'s from its request and state
    /// (an aligned position worked out in the screen the platform tells).
    fn window_spec(&self) -> WindowSpec {
        let mut spec = WindowSpec::main(&self.config.title, self.config.width, self.config.height);
        if let Kind::Window(request) = &self.kind {
            let p = request.properties();
            let (placement, minimized, position, size) = request.state().peek();
            spec.id = request.id();
            spec.title = p.title;
            if size.is_specified() {
                (spec.width, spec.height) = (size.width, size.height);
            }
            spec.placement = placement;
            spec.minimized = minimized;
            spec.undecorated = p.undecorated;
            spec.transparent = p.transparent;
            spec.resizable = p.resizable;
            spec.focusable = p.focusable;
            spec.always_on_top = p.always_on_top;
            spec.position = self.position_on_screen(&position, spec.width, spec.height);
        }
        spec
    }

    /// Where `position` puts a window of `width` by `height` dp: dp from the screen's top left (none: the platform's
    /// place, or an aligned one where the screen's size isn't told).
    fn position_on_screen(&self, position: &WindowPosition, width: Dp, height: Dp) -> Option<(Dp, Dp)> {
        match position {
            WindowPosition::PlatformDefault => None,
            &WindowPosition::Absolute { x, y } => Some((x, y)),
            WindowPosition::Aligned(alignment) => {
                let (sw, sh) = self.platform.screen_size()?;
                let k = self.point_to_px / self.scale; // (dp per point)
                let screen = IntSize::new((sw as f32 * k) as i32, (sh as f32 * k) as i32);
                let size = IntSize::new(width.0 as i32, height.0 as i32);
                let at = alignment.align(size, screen, LayoutDirection::Ltr);
                Some((Dp(at.x as f32), Dp(at.y as f32)))
            }
        }
    }

    /// KOMPOZ_GPU's backends, in the order they are tried: a name is that one; "auto" (or nothing) the platform's
    /// own order.
    fn gpu_order(&self, choice: &str) -> Vec<Backend> {
        match choice {
            "vulkan" => vec![Backend::Vulkan],
            "metal" => vec![Backend::Metal],
            "webgpu" => vec![Backend::WebGpu],
            "" | "auto" => self.platform.backends(),
            other => {
                self.logf(&format!("kompoz: KOMPOZ_GPU={other}: not vulkan, metal, webgpu or auto: auto"));
                self.platform.backends()
            }
        }
    }

    /// The renderer's device on `backend`, in a window the platform opens for it; false (and the window closed
    /// again) if it can't be made.
    fn start_gpu(&mut self, backend: Backend) -> bool {
        // (The app's composition without a window: a device that draws only into textures.)
        let spec = self.window_spec();
        let mut surface = if self.is_application() {
            Surface::default()
        } else {
            match self.platform.open_window(&spec, backend) {
                Ok(surface) => surface,
                Err(why) => {
                    self.logf(&format!("kompoz: no window for {}: {why}", backend.name()));
                    return false;
                }
            }
        };
        // A transparent window's: presented with its alpha, its frames drawn over nothing (where the app draws
        // nothing, what is behind the window shows).
        surface.transparent = spec.transparent && !self.is_application();
        self.window_open = !self.is_application();
        // The compiled pipelines beside the downloaded images, a file for each backend and build of the shaders;
        // another's goes (a driver keeps what it was given: an older build's pipelines would stay in it, read at
        // every start, the file growing with each update).
        if let Some(dir) = app_cache_directory(&self.config.title) {
            let prefix = format!("pipelines-{}", backend.name());
            let name = format!("{prefix}-{:016x}", Renderer::shaders_version());
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let other = entry.file_name().to_string_lossy().into_owned();
                    if other == prefix || (other.starts_with(&format!("{prefix}-")) && !other.starts_with(&name)) {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
            surface.pipeline_cache = Some(dir.join(name));
        }
        let device = match &self.device_factory {
            Some(factory) => factory(backend, surface),
            None => device::create(backend, surface),
        };
        let made = device.and_then(|d| Renderer::new(d).ok_or("no renderer".to_string()));
        match made {
            Ok(mut renderer) => {
                if spec.transparent {
                    renderer.set_frame_background(Color::Transparent);
                }
                self.logf(&format!(
                    "kompoz: renderer: KompozR on {} ({})",
                    backend.name(),
                    renderer.device().device_name()
                ));
                let mut timed = false;
                if self.flags.profile {
                    timed = renderer.device_mut().set_gpu_timing(true);
                    if !timed {
                        self.logf("kompoz: profile: this GPU's time can't be measured here");
                    }
                }
                let renderer = Rc::new(RefCell::new(renderer));
                let bitmaps = RendererBitmaps::new(&renderer);
                *self.renderer_slot.borrow_mut() = Rc::downgrade(&renderer);
                self.gpu = Some(Gpu { renderer, bitmaps, frame: None, timed });
                true
            }
            Err(why) => {
                self.logf(&format!("kompoz: no {} device: {why}", backend.name()));
                if self.window_open {
                    self.platform.close_window();
                }
                self.window_open = false;
                false
            }
        }
    }

    /// The GPU device let go (its frames done), then the window (a `Window` hidden; the core going).
    fn release_gpu(&mut self) {
        if let Some(gpu) = self.gpu.take() {
            let Gpu { renderer, bitmaps, frame, .. } = gpu;
            if self.is_root() {
                // What only the renderer has of bitmaps still about: back into them.
                kompoz_ui::draw::set_bitmap_renderer(None);
            }
            drop(bitmaps);
            drop(frame);
            if let Ok(mut r) = renderer.try_borrow_mut() {
                r.device_mut().wait_idle();
                // (KOMPOZ_GPU_VALIDATION=1: what the API's validation found, for scripted runs to fail on.)
                let errors = r.device().validation_errors();
                if errors > 0 {
                    self.platform.log(&format!("kompoz: GPU validation: {errors} errors"));
                }
            }
            drop(renderer);
        }
        if self.window_open {
            self.platform.close_window();
            self.window_open = false;
        }
    }

    /// A `Window` shown or hidden (its `visible`): hidden, its GPU device and platform window let go, its host going
    /// on (its state kept, Compose Desktop's); shown again, a window and a device made afresh, all of it drawn.
    /// False if it can't be shown again (logged).
    pub fn set_visible(&mut self, visible: bool) -> bool {
        if visible == self.shown || self.host.is_none() {
            self.shown = visible;
            return true;
        }
        self.shown = visible;
        if !visible {
            self.applied.menu_bar = None; // (its window goes: told again as it is shown)
            self.release_gpu();
            self.hidden = true;
            self.update_lifecycle();
            return true;
        }
        let choice = self.platform.option("KOMPOZ_GPU").unwrap_or_default();
        let backends = self.gpu_order(choice.split_whitespace().next().unwrap_or(""));
        if !backends.iter().any(|&b| self.start_gpu(b)) {
            self.logf("kompoz: the window couldn't be shown again: no GPU device");
            return false;
        }
        self.hidden = false;
        self.update_lifecycle();
        self.display.invalidate();
        self.shown_changes = ShownChanges::default();
        self.show_frame = true;
        self.metrics_changed = true;
        if let Some(host) = &self.host {
            host.request_redraw();
        }
        true
    }

    /// Whether it is shown (a hidden `Window`'s isn't).
    pub fn is_shown(&self) -> bool {
        self.shown
    }

    // ---- Metrics ----

    /// The platform's metrics read again (C++'s `updateMetrics`): density, viewport, the shadows' light, the
    /// refresh rate, the safe area, the theme, the title bar, the language (its direction and the library's texts),
    /// the media.
    fn update_metrics(&mut self) {
        let Some(host) = &self.host else { return };
        let m = self.platform.metrics();
        let scale = if m.scale > 0.0 { m.scale } else { 1.0 };
        self.scale = scale;
        self.point_to_px = if m.point_to_px > 0.0 { m.point_to_px } else { 1.0 };
        host.set_density(Density::new(scale, m.font_scale));
        host.set_viewport_size(IntSize::new(m.pixel_width.max(1), m.pixel_height.max(1)));
        // Elevation shadows' light: above the window's top middle, as Android's (its theme's values, in px).
        let light = ShadowLight {
            center: Offset::new(m.pixel_width as f32 / 2.0, 0.0),
            z: 500.0 * scale,
            radius: 800.0 * scale,
            ..ShadowLight::default()
        };
        self.display.set_light(light);
        if let Some(gpu) = &self.gpu {
            gpu.renderer.borrow_mut().set_light(light);
        }
        self.frame_period_ns = (1e9 / if m.refresh_rate > 0.0 { m.refresh_rate } else { 60.0 }) as i64;
        // A desktop's safe area: taken as the display cutout.
        if let Some(safe) = m.safe_area {
            let k = self.point_to_px;
            let px = |v: f32| (v * k).round() as i32;
            let mut insets = host.system_insets();
            insets.display_cutout = Insets::new(
                px(safe.left),
                px(safe.top),
                px(m.point_width as f32 - safe.right),
                px(m.point_height as f32 - safe.bottom),
            );
            host.set_system_insets(insets);
        }
        host.set_system_dark_theme(m.dark_theme);
        // A `Window`'s size as the user (or the window manager) left it, back into its state (dp).
        if let Kind::Window(r) = &self.kind
            && self.window_open
        {
            let k = self.point_to_px / scale;
            let size = DpSize(Dp(m.point_width as f32 * k), Dp(m.point_height as f32 * k));
            if (size.width.0 - self.applied.size.width.0).abs() >= 0.5
                || (size.height.0 - self.applied.size.height.0).abs() >= 0.5
            {
                self.applied.size = size;
                r.state().set_size(size);
            }
        }
        let Some(host) = &self.host else { return };
        if self.title_bar.peek() != m.title_bar {
            self.title_bar.set(m.title_bar.clone());
        }
        // The preferred language: right to left when it is written so (KOMPOZ_LAYOUT_DIRECTION=rtl|ltr decides
        // instead), and the library's texts in it (KOMPOZ_LANGUAGE=tr|en decides instead).
        let mut language = m.language.clone();
        let mut rtl = is_rtl_language(&language);
        if let Some(forced) = self.platform.option("KOMPOZ_LAYOUT_DIRECTION") {
            rtl = forced == "rtl";
        }
        host.set_layout_direction(if rtl { LayoutDirection::Rtl } else { LayoutDirection::Ltr });
        if let Some(forced) = self.platform.option("KOMPOZ_LANGUAGE") {
            language = forced;
        }
        host.set_strings(Strings::for_language(&language));
        self.platform.metrics_updated(host);
        self.refresh_media();
        if let Some(host) = &self.host {
            host.request_redraw();
        }
    }

    /// The device for the media scope, as the platform knows it (a microphone and a camera looked for only after
    /// the first frame: starting the systems that tell takes a few tens of milliseconds).
    fn refresh_media(&mut self) {
        let info = self.platform.media(self.media_probed);
        if let Some(host) = &self.host {
            host.set_media_info(info);
        }
    }

    // ---- Events ----

    /// An event (C++'s `handle`), then the platform's own after it.
    pub fn handle(&mut self, event: &Event) {
        let start = if self.flags.profile { self.now() } else { 0 };
        self.handle_event(event);
        self.do_window_actions();
        if let Some(host) = &self.host {
            self.platform.handled(host, event);
        }
        if self.flags.profile {
            self.profile.events_ns += self.now().saturating_sub(start);
        }
    }

    /// The mouse's buttons other than the left one, as pointer events know them.
    fn other_button(button: MouseButton) -> PointerButton {
        match button {
            MouseButton::Right => PointerButton::Secondary,
            MouseButton::Middle => PointerButton::Tertiary,
            MouseButton::Back => PointerButton::Back,
            _ => PointerButton::Forward,
        }
    }

    fn handle_event(&mut self, e: &Event) {
        if self.metrics_changed && !e.is_window_event() {
            // (Input meets the window as it is now.)
            self.metrics_changed = false;
            self.update_metrics();
        }
        let Some(host) = &self.host else {
            if matches!(e, Event::Quit) {
                self.quit = true;
            }
            return;
        };
        // (A `Window` not enabled takes no input.)
        if let Kind::Window(r) = &self.kind
            && is_input(e)
            && !r.properties().enabled
        {
            return;
        }
        if let Some(modifiers) = e.modifiers() {
            host.set_keyboard_modifiers(modifiers); // (pointer events carry the keys held down)
        }
        match e {
            Event::Quit => self.quit = true,
            Event::MenuBarItem { path, .. } => {
                if let Kind::Window(r) = &self.kind {
                    r.clone().click_menu_bar_item(path);
                }
            }
            Event::MenuBarHostChanged => self.applied.menu_bar = None, // (shown again at the next sync)
            Event::CloseRequest => match &self.kind {
                Kind::Main => self.quit = true,
                Kind::Window(r) => r.clone().close_requested(),
                Kind::Application => {}
            },
            Event::FocusGained | Event::FocusLost => {
                self.focused = matches!(e, Event::FocusGained);
                host.set_window_focused(self.focused);
                self.update_lifecycle();
            }
            Event::Minimized | Event::Maximized => {
                self.minimized = matches!(e, Event::Minimized);
                self.minimized_by_user(self.minimized);
                self.update_lifecycle();
            }
            Event::Hidden => {
                self.hidden = true;
                self.update_lifecycle();
            }
            // Phones: the app's lifecycle follows it going to the background and back (paused before the system may
            // end it: what that saves goes in the saved state), not its window.
            Event::WillEnterBackground if PHONE => host.set_lifecycle_state(LifecycleState::Started),
            Event::DidEnterBackground if PHONE => host.set_lifecycle_state(LifecycleState::Created),
            Event::WillEnterForeground if PHONE => host.set_lifecycle_state(LifecycleState::Started),
            Event::DevicesChanged => self.refresh_media(),
            Event::Resized => {
                self.resize_frames = 4;
                self.metrics_changed = true;
            }
            Event::MetricsChanged => self.metrics_changed = true,
            Event::DidEnterForeground | Event::Shown | Event::Restored => {
                if matches!(e, Event::DidEnterForeground) {
                    self.update_metrics(); // (the wallpaper, and so the dynamic colors, may have changed meanwhile)
                    if PHONE && let Some(host) = &self.host {
                        host.set_lifecycle_state(LifecycleState::Resumed);
                    }
                }
                match e {
                    Event::Shown => self.hidden = false,
                    Event::Restored => {
                        self.minimized = false;
                        self.minimized_by_user(false);
                    }
                    _ => {}
                }
                self.update_lifecycle();
                // Back on the screen: all of it drawn again, in case the kept pixels went with the old surface.
                self.display.invalidate();
                self.show_frame = true;
                if let Some(host) = &self.host {
                    host.request_redraw();
                }
            }
            Event::Exposed => {
                self.show_frame = true;
                host.request_redraw();
            }
            Event::LowMemory => {
                if let Some(gpu) = &self.gpu {
                    gpu.renderer.borrow_mut().trim_memory();
                }
            }
            Event::SurfaceLost => {
                // The kept pixels are gone.
                self.display.invalidate();
                self.show_frame = true;
                host.request_redraw();
            }
            Event::PointerLeft => host.pointer_exit(),
            &Event::MouseMove { x, y, time, .. } => {
                host.pointer_move(self.to_px(x, y), self.event_time(time), 0);
            }
            &Event::MouseDown { x, y, button, time, .. } => {
                let at = self.to_px(x, y);
                let t = self.event_time(time);
                if button == MouseButton::Left {
                    host.pointer_down(at, t, 0, PointerType::Mouse);
                } else {
                    // (A context menu opens on the secondary button's press: Compose's, through pointer events.)
                    host.mouse_button(at, t, Self::other_button(button), true);
                }
            }
            &Event::MouseUp { x, y, button, time, .. } => {
                let at = self.to_px(x, y);
                let t = self.event_time(time);
                if button == MouseButton::Left {
                    host.pointer_up(at, t, 0);
                } else {
                    host.mouse_button(at, t, Self::other_button(button), false);
                }
            }
            &Event::TouchDown { pointer, x, y, time } => {
                host.pointer_down(self.to_px(x, y), self.event_time(time), pointer, PointerType::Touch);
            }
            &Event::TouchMove { pointer, x, y, time } => {
                host.pointer_move(self.to_px(x, y), self.event_time(time), pointer);
            }
            Event::TouchMoves { moves, time } => {
                let moves: Vec<(i64, Offset)> = moves.iter().map(|&(p, x, y)| (p, self.to_px(x, y))).collect();
                host.pointer_moves(&moves, self.event_time(*time));
            }
            Event::TouchCancel => host.pointer_cancel(),
            Event::History { pointer, samples } => {
                let samples: Vec<(Offset, i64)> =
                    samples.iter().map(|&(x, y, time)| (self.to_px(x, y), self.event_time(time))).collect();
                host.pointer_historical(*pointer, &samples);
            }
            &Event::TouchUp { pointer, x, y, time } => {
                host.pointer_up(self.to_px(x, y), self.event_time(time), pointer);
                host.pointer_exit(); // (a lifted finger doesn't hover)
            }
            &Event::Stylus { pointer, eraser, x, y, down, pressure, buttons, time } => {
                let r#type = if eraser { PointerType::Eraser } else { PointerType::Stylus };
                let at = self.to_px(x, y);
                let t = self.event_time(time);
                host.stylus(at, t, pointer, r#type, down, pressure, PointerButtons(buttons));
            }
            &Event::StylusLeft { pointer } => host.stylus_exit(pointer),
            &Event::Wheel { x, y, dx, dy, modifiers, .. } => {
                let at = self.to_px(x, y);
                // Ctrl+wheel zooms (a transformable), as touchpad pinches do; Ctrl+Shift turns it, 15° a notch.
                let ctrl = modifiers.contains(kompoz_ui::input::KeyModifiers::Ctrl);
                let shift = modifiers.contains(kompoz_ui::input::KeyModifiers::Shift);
                if ctrl && shift && host.zoom(at, 1.0, -15.0 * dy) {
                    return;
                }
                if ctrl && host.zoom(at, 1.1f32.powf(dy), 0.0) {
                    return;
                }
                host.scroll(at, Offset::new(dx, -dy));
            }
            &Event::PinchBegin { x, y } => {
                host.scale(self.to_px(x, y), PointerEventType::ScaleStart, 1.0, 0.0);
            }
            &Event::Pinch { x, y, scale, rotation } => {
                host.zoom(self.to_px(x, y), scale, rotation);
            }
            &Event::PinchEnd { x, y } => {
                host.scale(self.to_px(x, y), PointerEventType::ScaleEnd, 1.0, 0.0);
            }
            Event::Text(text) => {
                host.text_input(text);
            }
            Event::Preedit { text, cursor } => match *cursor {
                // (The caret after the composing text: Compose's plain SetComposingTextCommand; inside it, or a part
                // of it highlighted: the selection set inside the composition after it.)
                Some((start, end)) if !text.is_empty() && (start, end) != (text.len(), text.len()) => {
                    host.set_composing_text_with_selection(text, start, end);
                }
                _ => {
                    host.set_composing_text(text, 1);
                }
            },
            &Event::DeleteSurroundingText { before, after } => {
                host.send_edit_commands(&[EditCommand::DeleteSurroundingText { before, after }]);
            }
            &Event::SetSelection { start, end } => {
                host.send_edit_commands(&[EditCommand::SetSelection { start, end }]);
            }
            Event::Back => {
                if !host.back_invoked() {
                    self.platform.leave_app(); // (as usual: Android leaves the app)
                }
            }
            &Event::KeyDown { key, code_point, modifiers, .. } | &Event::KeyUp { key, code_point, modifiers, .. } => {
                if key == Key::Unknown {
                    return;
                }
                let r#type =
                    if matches!(e, Event::KeyDown { .. }) { KeyEventType::KeyDown } else { KeyEventType::KeyUp };
                host.send_key_event(KeyEvent::with_code_point(key, r#type, code_point, modifiers));
            }
            // Files from a file manager, text from a browser: the drag and drop targets take them. (The app's own
            // drag, back over the window from the platform: with its own MIME types; the host has what it carries.)
            Event::DropBegin { mime_types, .. } => {
                self.dropping = true;
                self.drop_own = false;
                self.drop_files.clear();
                self.drop_text.clear();
                self.drop_entry = None;
                self.drop_offered = (!mime_types.is_empty()).then(|| mime_types.clone());
                host.external_drag_entered(self.drop_types());
            }
            &Event::DropMove { x, y } => {
                if !self.dropping {
                    self.dropping = true;
                    host.external_drag_entered(self.drop_types());
                }
                let wanted = host.external_drag_moved(self.to_px(x, y));
                self.platform.drop_over(wanted);
            }
            Event::DropFile { x, y, path } => {
                if !path.is_empty() {
                    self.drop_files.push(path.clone());
                }
                self.drop_at = self.to_px(*x, *y);
            }
            Event::DropText { x, y, text } => {
                self.drop_text += text;
                self.drop_at = self.to_px(*x, *y);
            }
            &Event::DropData { x, y } => {
                self.drop_entry = self.platform.take_drop_entry();
                self.drop_at = self.to_px(x, y);
            }
            &Event::DropOwn { x, y } => {
                self.drop_own = true;
                self.drop_at = self.to_px(x, y);
            }
            &Event::DropComplete { .. } => {
                self.dropping = false;
                self.drop_offered = None;
                let data = if std::mem::take(&mut self.drop_own) {
                    // (the host's own taken instead)
                    let mime_types = self.own_drag.borrow().clone().unwrap_or_default();
                    let clip_entry = ClipEntry::with_metadata(Vec::new(), ClipMetadata::new(mime_types));
                    DragAndDropTransferData { clip_entry, ..Default::default() }
                } else if let Some(clip_entry) = self.drop_entry.take() {
                    // (all it offers, read as the target asks for it, until the platform is told it is done)
                    DragAndDropTransferData { clip_entry, ..Default::default() }
                } else if !self.drop_files.is_empty() {
                    let files = std::mem::take(&mut self.drop_files);
                    DragAndDropTransferData { clip_entry: ClipEntry::with_files(files), ..Default::default() }
                } else if !self.drop_text.is_empty() {
                    let text = std::mem::take(&mut self.drop_text);
                    DragAndDropTransferData { clip_entry: ClipEntry::with_plain_text(text), ..Default::default() }
                } else {
                    host.external_drag_left(); // it left the window, or brought nothing
                    self.platform.drop_done(false);
                    return;
                };
                let taken = host.external_drop(data, self.drop_at);
                self.platform.drop_done(taken);
            }
            // The platform's drag of the app's own ended: its source told.
            &Event::OwnDragEnded { action } => {
                self.own_drag.borrow_mut().take();
                host.platform_drag_ended(action);
            }
            // (AccentChanged, WallpaperChanged, AccessibilityRequest and Wake are the platform's own:
            // Platform::handled; the phones' lifecycle events on desktops: the window's says it.)
            _ => {}
        }
    }

    /// The app's lifecycle on desktops (C++'s `updateLifecycle`, CMP's `ComposeContainer.updateLifecycleState`):
    /// [`lifecycle_state_of`] the window; phones follow the app's background events instead.
    fn update_lifecycle(&self) {
        if PHONE {
            return;
        }
        if let Some(host) = &self.host {
            host.set_lifecycle_state(lifecycle_state_of(self.focused, self.minimized, self.hidden));
        }
    }

    // ---- Frames ----

    /// When the next frame is due on the platform's clock, for a platform pacing its frames itself (C++'s
    /// `frameDue`): one a display beat, never a second for the beat of the last (a frame started sooner would get
    /// the last one's time: a scroll or an animation would stand still in it and jump two beats in the next); none:
    /// now. A script in virtual time has its frames at once.
    pub fn frame_due(&self) -> Option<i64> {
        if self.virtual_clock.is_some() {
            return None; // (the app's script's frames, at once)
        }
        let last = self.last_frame_time?;
        if self.script.running() && !self.script.realtime {
            return None;
        }
        Some(last - self.clock_offset + self.frame_period_ns * 3 / 5)
    }

    /// The time until the next frame is due: the host's for work between frames (a lazy list's next item).
    pub fn idle(&mut self, budget_nanos: i64) {
        if let Some(host) = &self.host
            && host.has_idle_work()
        {
            host.idle(budget_nanos);
        }
        self.do_window_actions();
    }

    /// A frame (C++'s `frame`): a script's next step, composition, what changed drawn and shown. `display_time`:
    /// the frame's time where the platform knows it (the vsync it starts on); else the core puts it on the display's
    /// beat.
    pub fn frame(&mut self, display_time: Option<i64>) -> Next {
        let frame_start = self.now();
        self.frames += 1;
        if let Some(t) = self.virtual_clock {
            self.script.clock = self.script.clock.max(t);
        }
        let scripted = self.script.running();
        let shot = if self.script.enabled() { self.step_script() } else { None };
        let now = if let Some(t) = self.virtual_clock {
            // (The app's script's virtual time: this window's frames at its frames' times.)
            self.last_frame_time = Some(t);
            t
        } else if scripted && !self.script.realtime {
            self.script.clock += SCRIPT_FRAME_NS;
            self.script.clock
        } else {
            let t = match display_time {
                Some(t) => t + self.clock_offset,
                None => kompoz_ui::host::frame_time_on_beat(self.last_frame_time, self.clock(), self.frame_period_ns),
            };
            self.last_frame_time = Some(t);
            t
        };
        if scripted {
            self.script.clock = self.script.clock.max(now);
            if !self.script.running() {
                // Its last frame: real time from here, never back.
                self.clock_offset = (now - self.platform.now()).max(0);
            }
        }
        if self.metrics_changed {
            self.metrics_changed = false;
            self.update_metrics();
        }
        let Some(host) = &self.host else { return Next { quit: true, ..Next::default() } };
        let t0 = if self.flags.profile { self.now() } else { 0 };
        let glyphs_before = self.gpu.as_ref().map_or(0, |g| g.renderer.borrow().glyphs_rasterized());
        let redraw = host.frame(now);
        self.do_window_actions();
        let Some(host) = &self.host else { return Next { quit: true, ..Next::default() } };
        let t1 = if self.flags.profile { self.now() } else { 0 };
        let (mut t2, mut t3) = (t1, t1);
        self.platform.composed(host, redraw, now);
        let stats = host.stats();
        if self.flags.trace && stats.recompositions != self.traced.recompositions {
            self.logf(&format!(
                "kompoz: recomposition {}: {} scopes ran, {} skipped",
                stats.recompositions,
                stats.scopes_run - self.traced.scopes_run,
                stats.scopes_skipped - self.traced.scopes_skipped
            ));
            self.traced = stats;
        }
        if self.flags.keep_check {
            let mismatches = host.take_keep_check_mismatches();
            if redraw {
                self.keep_frames += 1;
            }
            if !mismatches.is_empty() {
                self.keep_wrong += 1;
            }
            for m in mismatches {
                self.logf(&m);
            }
        }
        if self.resize_frames > 0 {
            self.resize_frames -= 1;
            self.display.invalidate();
            self.show_frame = true;
        }
        let mut drawn = Drawn::default();
        let mut waited = 0;
        // (Drawn where there is a window to show it in: not the app's composition's, nor a hidden window's.)
        if (redraw || shot.is_some() || self.show_frame)
            && self.window_open
            && let Some(gpu) = self.gpu.as_ref()
        {
            let renderer = gpu.renderer.clone();
            let before = self.now();
            renderer.borrow_mut().device_mut().begin_frame(); // (waits for the GPU to be done with a frame before)
            waited = self.now().saturating_sub(before);
            self.in_frame.set(true);
            gpu.bitmaps.set_in_frame(true);
            self.profile.acquire_ns = 0; // (a frame with nothing to show asks for no image)
            drawn = self.draw_frame(shot.as_deref());
            let work =
                self.now().saturating_sub(frame_start).saturating_sub(waited).saturating_sub(self.profile.acquire_ns);
            self.platform.frame_work(work as i64);
            t2 = if self.flags.profile { self.now() } else { 0 };
            let gpu = self.gpu.as_ref().expect("a renderer");
            renderer.borrow_mut().device_mut().end_frame(drawn.shown);
            self.in_frame.set(false);
            gpu.bitmaps.set_in_frame(false);
            renderer.borrow_mut().end_frame();
            self.finish_writes();
            if drawn.shown && !self.media_probed {
                self.media_probed = true;
                self.refresh_media();
            }
            t3 = if self.flags.profile { self.now() } else { 0 };
        }
        if self.flags.profile && redraw {
            let counts = self.draw_counts(glyphs_before);
            let log = |m: &str| self.platform.log(m);
            self.profile.frame(
                &log,
                self.frame_period_ns as u64,
                t0,
                t1,
                waited,
                t2,
                t3,
                drawn.shown,
                drawn.area,
                counts,
            );
        }
        if let Some(gpu) = &self.gpu
            && gpu.timed
        {
            let times = gpu.renderer.borrow_mut().device_mut().take_gpu_times();
            let log = |m: &str| self.platform.log(m);
            self.profile.gpu(&log, self.frame_period_ns as u64, times);
        }
        // Waiting for the next event when there is nothing to animate or compose, or until the next coroutine
        // delay is due.
        let Some(host) = &self.host else { return Next { quit: true, ..Next::default() } };
        let busy =
            host.has_pending_work() || self.script.running() || self.resize_frames > 0 || self.platform.wants_frames();
        // (Frames meant to follow each other: what the app shows asks for the next. Not a script waiting its frames
        // out, a semantics publish, a timer or work on another thread: a pause in which nothing changes on the
        // screen missed no refresh.)
        let due = host.wants_frame();
        if due && !self.profile.busy {
            self.profile.due_since = self.now(); // (a pause before it missed nothing)
        }
        self.profile.busy = due;
        let wake_at = host.next_deadline().map(|deadline| deadline - self.clock_offset + 1_000_000);
        Next { busy, wake_at, quit: self.quit || self.exit.get() }
    }

    /// The renderer's and the display list's counts of the frame just drawn.
    fn draw_counts(&self, glyphs_before: usize) -> DrawCounts {
        let Some(gpu) = &self.gpu else { return DrawCounts::default() };
        let r = gpu.renderer.borrow();
        DrawCounts {
            drawn_nodes: self.display.drawn_nodes(),
            kept_nodes: self.display.kept_nodes(),
            layers: r.layer_count,
            kept_layers: r.kept_layers,
            paths: r.path_count,
            kept_paths: r.kept_paths,
            path_shadows: r.path_shadows,
            kept_shadows: r.kept_shadows,
            textures_made: r.textures_made,
            glyphs: r.glyphs_rasterized() - glyphs_before,
        }
    }

    /// Draws the frame (C++'s `drawFrame`, in a frame of the device's begun by the caller): recorded, then what
    /// changed drawn again into the kept pixels, which are put in the window if anything changed (or for a
    /// screenshot, or when the window lost them), only what the swapchain image lacks of them copied over.
    fn draw_frame(&mut self, shot: Option<&str>) -> Drawn {
        let (pw, ph) = self.platform.pixel_size();
        let (pw, ph) = (pw.max(1), ph.max(1));
        let Some(host) = &self.host else { return Drawn::default() };
        let Some(gpu) = &mut self.gpu else { return Drawn::default() };
        let renderer = gpu.renderer.clone();
        let mut r = renderer.borrow_mut();
        if gpu.frame.as_ref().is_none_or(|f| f.width() != pw || f.height() != ph) {
            gpu.frame = r.device_mut().create_texture(
                pw,
                ph,
                Format::Rgba8,
                TextureUsage::SAMPLED | TextureUsage::RENDER_TARGET,
            );
            self.display.invalidate();
            self.show_frame = true;
        }
        let Some(frame) = gpu.frame.as_ref().map(Texture::id) else {
            self.platform.log("kompoz: no memory for the kept frame");
            return Drawn::default();
        };
        r.layer_count = 0;
        r.kept_layers = 0;
        r.path_count = 0;
        r.kept_paths = 0;
        r.textures_made = 0;
        r.path_shadows = 0;
        r.kept_shadows = 0;
        let screen = Size::new(pw as f32, ph as f32);
        let recording = Instant::now();
        // (The renderer isn't borrowed while the host draws: what draws into bitmaps reaches it.)
        drop(r);
        self.display.begin(screen);
        host.draw(&mut self.display);
        if self.flags.full_redraw {
            self.display.invalidate();
        }
        let damage = self.display.end();
        self.profile.record_ns = recording.elapsed().as_nanos() as u64;
        let mut r = renderer.borrow_mut();
        // (Drawn only where it changed, into the frame before.)
        let kept_changes = if damage.is_some() { r.draw_frame(frame, &self.display, true) } else { true };
        if let Some(damage) = damage
            && self.flags.damage_check
            && let Some(full) = r.device_mut().create_texture(
                pw,
                ph,
                Format::Rgba8,
                TextureUsage::SAMPLED | TextureUsage::RENDER_TARGET,
            )
        {
            r.draw_frame(full.id(), &self.display, false);
            r.flush();
            // (Both read back, then compared: at once where the device waits for the GPU, a turn later on WebGPU.)
            let all = IRect::new(0, 0, pw, ph);
            let pair: Rc<RefCell<(Option<Vec<u8>>, Option<Vec<u8>>)>> = Rc::default();
            for kept in [true, false] {
                let (pair, frames, wrong, reading) =
                    (pair.clone(), self.damage_frames.clone(), self.damage_wrong.clone(), self.reading.clone());
                reading.set(reading.get() + 1);
                let texture = if kept { frame } else { full.id() };
                r.device_mut().read_pixels_async(
                    texture,
                    all,
                    Box::new(move |pixels| {
                        reading.set(reading.get() - 1);
                        let mut p = pair.borrow_mut();
                        if kept {
                            p.0 = Some(pixels);
                        } else {
                            p.1 = Some(pixels);
                        }
                        if let (Some(kept), Some(fresh)) = (&p.0, &p.1) {
                            frames.set(frames.get() + 1);
                            if let Some(why) = compare_damage(kept, fresh, pw, ph, damage) {
                                wrong.set(wrong.get() + 1);
                                log(&why);
                            }
                        }
                    }),
                );
            }
        }
        let area = drawn_area(self.display.areas(), screen.width, screen.height);
        if damage.is_none() && shot.is_none() && !self.show_frame {
            r.flush();
            return Drawn { shown: false, area: 0.0 };
        }
        let acquiring = Instant::now();
        let swapchain = r.device_mut().swapchain_texture();
        self.profile.acquire_ns = acquiring.elapsed().as_nanos() as u64;
        if let Some(target) = swapchain {
            // What this frame changed in the kept pixels (all of it when the window needs it all, or the outlines
            // of what was drawn again are shown, which stay in the image).
            let all = self.show_frame || self.flags.damage_show || !kept_changes;
            let changed = if all {
                None
            } else if damage.is_some() {
                Some(frame_changes(self.display.areas(), self.display.moves()))
            } else {
                Some(Vec::new())
            };
            self.shown_changes.shown(changed);
            // A swapchain image holding the frame `age` frames ago: what changed since copied over.
            let copied = self.shown_changes.copied(r.device().swapchain_age(), pw, ph);
            self.show_frame = false;
            if copied.is_none() {
                r.device_mut().discard(target); // (all of it copied over: what it held isn't loaded on a tiling GPU)
            }
            let turns = r.device().swapchain_turns();
            r.begin_target(target, None);
            r.blit(frame, turns, false, copied.as_deref());
            if damage.is_some() && self.flags.damage_show {
                // What was drawn again, outlined (only in the window, not the kept pixels), turned as the frame is.
                let mut turned = Matrix::new();
                let (cs, sn) = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)][(turns & 3) as usize];
                turned.set(0, 0, cs);
                turned.set(0, 1, sn);
                turned.set(1, 0, -sn);
                turned.set(1, 1, cs);
                turned.set(
                    3,
                    0,
                    match turns & 3 {
                        1 => screen.height,
                        2 => screen.width,
                        _ => 0.0,
                    },
                );
                turned.set(
                    3,
                    1,
                    match turns & 3 {
                        2 => screen.height,
                        3 => screen.width,
                        _ => 0.0,
                    },
                );
                r.push_transform(&turned);
                for &a in self.display.areas() {
                    r.stroke_round_rect(a, CornerRadii::Zero, 3.0, Color::new(1.0, 0.0, 80.0 / 255.0, 1.0));
                }
                r.pop_transform();
            }
            r.end_target();
        }
        r.flush();
        if let Some(path) = shot {
            // (Written once read back: at once where the device waits for the GPU, a turn later on WebGPU.)
            let (written, reading, path) = (self.written.clone(), self.reading.clone(), path.to_string());
            reading.set(reading.get() + 1);
            r.device_mut().read_pixels_async(
                frame,
                IRect::new(0, 0, pw, ph),
                Box::new(move |pixels| {
                    reading.set(reading.get() - 1);
                    written.borrow_mut().push((path, encode_png(pw, ph, &pixels)));
                }),
            );
        }
        Drawn { shown: swapchain.is_some(), area }
    }

    // ---- The script ----

    /// The script's next step (C++'s `stepScript`): a swipe's next move, a frame waited, or commands until one
    /// waits; a screenshot's path to take in this frame, if one asks.
    fn step_script(&mut self) -> Option<String> {
        let px = self.scale; // (a script's positions are dp)
        let dp = |p: Offset| Offset::new(p.x * px, p.y * px);
        let clock = self.script.clock;
        if self.script.swipe_step < self.script.swipe_frames {
            let s = &mut self.script;
            s.swipe_step += 1;
            let t = s.swipe_step as f32 / s.swipe_frames as f32;
            let p = s.swipe_from + (s.swipe_to - s.swipe_from) * t;
            let last = s.swipe_step == s.swipe_frames;
            let host = self.host.as_ref()?;
            host.pointer_move(dp(p), clock, 0);
            if last {
                host.pointer_up(dp(p), clock, 0);
            }
            return None;
        }
        if self.script.wait_frames > 0 {
            self.script.wait_frames -= 1;
            return None;
        }
        if self.script.sleep_until > 0 {
            if self.platform.now() < self.script.sleep_until {
                // (not a frame after another while the time goes by; on the web, which can't block, the page's next
                // frame comes when it does)
                #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
                std::thread::sleep(std::time::Duration::from_millis(2));
                return None;
            }
            self.script.sleep_until = 0;
        }
        while self.script.next < self.script.commands.len() {
            let line = self.script.commands[self.script.next].clone();
            self.script.next += 1;
            // "@Title: command": the command in the app's window of that title (a `Window`'s), at its next frame.
            if let Some(rest) = line.trim_start().strip_prefix('@') {
                match rest.split_once(':') {
                    Some((title, command)) => self.script_out.push((title.to_string(), command.to_string())),
                    None => self.logf(&format!("kompoz: script: '{line}' names no command (@Title: command)")),
                }
                continue;
            }
            let Some(command) = parse_command(&line) else { continue };
            let host = self.host.as_ref()?;
            match command {
                Command::Wait(frames) => {
                    self.script.wait_frames = frames;
                    return None;
                }
                Command::Realtime => {
                    // The rest in real time, frames paced to the display: as a person would see it.
                    self.script.realtime = true;
                    self.clock_offset = (self.script.clock - self.platform.now()).max(0); // (never back)
                }
                Command::Sleep(ms) => {
                    // Real milliseconds: what other threads do (an image read, the wallpaper's colors).
                    self.script.sleep_until = self.platform.now() + ms.max(0) as i64 * 1_000_000;
                    return None;
                }
                Command::Click(p) => {
                    host.pointer_move(dp(p), -1, 0);
                    host.pointer_down(dp(p), -1, 0, PointerType::Touch);
                    host.pointer_up(dp(p), -1, 0);
                }
                Command::RightClick(p) => {
                    host.mouse_button(dp(p), -1, PointerButton::Secondary, true);
                    host.mouse_button(dp(p), -1, PointerButton::Secondary, false);
                }
                Command::MiddleClick(p) => {
                    host.mouse_button(dp(p), -1, PointerButton::Tertiary, true);
                    host.mouse_button(dp(p), -1, PointerButton::Tertiary, false);
                }
                // A press held across waits (a long press).
                Command::Down(p) => {
                    host.pointer_down(dp(p), clock, 0, PointerType::Touch);
                }
                Command::Up(p) => {
                    host.pointer_up(dp(p), clock, 0);
                }
                Command::Move(p) => {
                    host.pointer_move(dp(p), -1, 0);
                }
                // Several fingers.
                Command::FingerDown(id, p) => {
                    host.pointer_down(dp(p), clock, id, PointerType::Touch);
                }
                Command::FingerMove(id, p) => {
                    host.pointer_move(dp(p), clock, id);
                }
                Command::FingerUp(id, p) => {
                    host.pointer_up(dp(p), clock, id);
                }
                Command::Drag { from, to, mouse } => {
                    let r#type = if mouse { PointerType::Mouse } else { PointerType::Touch };
                    host.pointer_down(dp(from), -1, 0, r#type);
                    for i in 1..=10 {
                        host.pointer_move(dp(from + (to - from) * (i as f32 / 10.0)), -1, 0);
                    }
                    host.pointer_up(dp(to), -1, 0);
                }
                Command::Swipe { from, to, frames } => {
                    let s = &mut self.script;
                    s.swipe_from = from;
                    s.swipe_to = to;
                    s.swipe_frames = frames.max(1);
                    s.swipe_step = 0;
                    host.pointer_down(dp(from), clock, 0, PointerType::Touch);
                    return None;
                }
                Command::Wheel(p, notches) => {
                    host.scroll(dp(p), Offset::new(0.0, -notches));
                }
                Command::Zoom(p, factor) => {
                    host.zoom(dp(p), factor, 0.0);
                }
                // As a file manager's or a browser's drag would come: in the window's points.
                Command::DropFile(p, what) | Command::DropText(p, what) => {
                    let file = matches!(line.split_whitespace().next(), Some("dropfile"));
                    let k = px / self.point_to_px;
                    let (x, y) = (p.x * k, p.y * k);
                    let carried =
                        if file { Event::DropFile { x, y, path: what } } else { Event::DropText { x, y, text: what } };
                    let begin = Event::DropBegin { x, y, mime_types: Vec::new() };
                    for e in [begin, Event::DropMove { x, y }, carried, Event::DropComplete { x, y }] {
                        self.handle_event(&e);
                    }
                }
                // As the window manager would.
                Command::Window(what) => {
                    let e = match what {
                        Some(WindowEvent::FocusLost) => Some(Event::FocusLost),
                        Some(WindowEvent::FocusGained) => Some(Event::FocusGained),
                        Some(WindowEvent::Minimized) => Some(Event::Minimized),
                        Some(WindowEvent::Restored) => Some(Event::Restored),
                        Some(WindowEvent::Close) => Some(Event::CloseRequest),
                        None => None,
                    };
                    if let Some(e) = e {
                        self.handle_event(&e);
                    }
                }
                Command::Type(text) => {
                    host.text_input(&text);
                }
                Command::Key(key) => {
                    if let Some((key, modifiers)) = key {
                        host.key_down(key, modifiers);
                        host.key_up(key, modifiers);
                    }
                }
                Command::Resize(w, h) => {
                    if w > 0 && h > 0 {
                        self.platform.set_window_size(w, h);
                    }
                    self.update_metrics();
                }
                Command::Shot(path) => {
                    if !path.is_empty() {
                        return Some(path);
                    }
                }
                Command::Quit => {
                    self.quit = true;
                    return None;
                }
                Command::Unknown(name) => self.logf(&format!("kompoz: unknown script command '{name}'")),
            }
        }
        None
    }

    // ---- Quitting ----

    /// The screenshots read back since written through the platform ([`Platform::write_file`]: a file; the web's
    /// page). A frame does it after drawing; the web's loop also as the GPU hands pixels back between frames.
    pub fn finish_writes(&mut self) {
        let written = std::mem::take(&mut *self.written.borrow_mut());
        for (path, png) in written {
            let saved = png.is_some_and(|png| self.platform.write_file(&path, &png));
            self.logf(&format!("kompoz: screenshot {path} {}", if saved { "saved" } else { "not saved" }));
        }
    }

    /// Whether pixels read back are still to come or to be written (an app ending waits for its screenshots).
    pub fn writing(&self) -> bool {
        self.reading.get() > 0 || !self.written.borrow().is_empty()
    }

    /// The window's native surface went away or is another (between frames): what the device made for the old one
    /// let go of now.
    pub fn surface_changed(&mut self) {
        if let Some(gpu) = &self.gpu {
            gpu.renderer.borrow_mut().device_mut().surface_changed();
        }
    }

    /// The window's pixels in `x, y, width, height` (px, cut to the window) as the last frame drew them: the kept
    /// frame read back (RGBA, alpha premultiplied, row by row) and the size read; none before a frame was drawn, or
    /// while the renderer is busy. What a platform copies of the window itself (Android's scroll capture: the parts
    /// of a long screenshot).
    pub fn read_frame(&mut self, x: i32, y: i32, width: i32, height: i32) -> Option<(i32, i32, Vec<u8>)> {
        let gpu = self.gpu.as_ref()?;
        let frame = gpu.frame.as_ref()?;
        let (fw, fh) = (frame.width(), frame.height());
        let (left, top) = (x.clamp(0, fw), y.clamp(0, fh));
        let (right, bottom) = (x.saturating_add(width).clamp(left, fw), y.saturating_add(height).clamp(top, fh));
        let (w, h) = (right - left, bottom - top);
        if w <= 0 || h <= 0 {
            return None;
        }
        let mut r = gpu.renderer.try_borrow_mut().ok()?;
        let own = !self.in_frame.get(); // (between frames: a frame of its own)
        if own {
            r.device_mut().begin_frame();
        }
        let pixels = r.device_mut().read_pixels(frame.id(), IRect::new(left, top, w, h));
        if own {
            r.device_mut().end_frame(false);
        }
        (pixels.len() == w as usize * h as usize * 4).then_some((w, h, pixels))
    }

    /// Before the platform goes (C++'s `quit`): `KOMPOZ_STATE_FILE` written, as Android keeps an app's state when
    /// it ends it.
    pub fn quit(&mut self) {
        if let Some(path) = self.platform.option("KOMPOZ_STATE_FILE")
            && let Some(host) = &self.host
            && std::fs::write(&path, host.save_state()).is_err()
        {
            self.logf(&format!("kompoz: can't write the state file {path}"));
        }
    }

    /// Whether the app asked to quit (a script's `quit`, the main window closed, `exit_application`).
    pub fn quitting(&self) -> bool {
        self.quit || self.exit.get()
    }

    /// The app's script's virtual time while it runs in virtual time (none: real time, or no script): what the
    /// app's windows' frames follow ([`set_virtual_clock`](AppCore::set_virtual_clock)).
    pub fn virtual_time(&self) -> Option<i64> {
        (self.script.running() && !self.script.realtime).then_some(self.script.clock)
    }

    /// A window's frames at the app's script's virtual times (none: its own clock again).
    pub fn set_virtual_clock(&mut self, time: Option<i64>) {
        if self.virtual_clock.is_some() && time.is_none() {
            // (Real time from here, never back.)
            if let Some(t) = self.last_frame_time {
                self.clock_offset = self.clock_offset.max(t - self.platform.now());
            }
        }
        self.virtual_clock = time;
    }

    /// Whether its next frame is due whatever the platform says: a script of its own runs, or the app's runs in
    /// virtual time (its windows' frames follow it, one each of its).
    pub fn scripted(&self) -> bool {
        self.script.running() || self.virtual_clock.is_some()
    }

    /// The script's "@Title: command"s since, taken (for the windows they name).
    pub fn take_script_forwards(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.script_out)
    }

    /// A script's command for this window (another's "@Title: command"), run at its next frame.
    pub fn run_script(&mut self, command: &str) {
        self.script.commands.push(command.to_string());
        if let Some(host) = &self.host {
            host.request_redraw(); // (a frame for it: a platform frames a window only when something asks)
            host.wake();
        }
    }

    /// The user minimized or restored a `Window`: its state told (once each way).
    fn minimized_by_user(&mut self, minimized: bool) {
        if let Kind::Window(r) = &self.kind
            && self.applied.minimized != minimized
        {
            self.applied.minimized = minimized;
            r.state().set_is_minimized(minimized);
        }
    }

    /// A `Window`'s title, state and parameters as its request has them now, given to the platform where they
    /// changed (between frames: C++'s `syncWindow`); how the window manager placed it and where read back into its
    /// state.
    pub fn sync_window(&mut self) {
        let Kind::Window(request) = &self.kind else { return };
        let request = request.clone();
        let p = request.properties();
        let state = request.state();
        let (placement, minimized, position, size) = state.peek();
        if p.title != self.applied.title {
            self.applied.title = p.title.clone();
            self.title.set(p.title.clone()); // (KompozR's own title bar shows it)
            self.platform.set_title(&p.title);
        }
        if !self.window_open {
            return; // (hidden: the rest as it is shown again)
        }
        // Its menu bar, to whatever shows it (the platform, or KompozR's own frame).
        let version = request.menu_bar_version();
        if self.applied.menu_bar != Some(version) {
            self.applied.menu_bar = Some(version);
            let menus: Option<Vec<_>> = request.menu_bar_menus().map(|m| m.iter().map(menu_item).collect());
            let shown = self.platform.set_menu_bar(menus.as_deref());
            if self.menu_bar_shown.peek() != shown {
                self.menu_bar_shown.set(shown);
            }
        }
        // What the window manager did, back into the state first (then what the app set since is told).
        if let Some(placed) = self.platform.window_placement()
            && placed != self.applied.placement
            && placement == self.applied.placement
        {
            self.applied.placement = placed;
            state.set_placement(placed);
        }
        let k = self.point_to_px / self.scale; // (dp per point)
        if let Some((x, y)) = self.platform.window_position() {
            let at = WindowPosition::Absolute { x: Dp((x as f32 * k).round()), y: Dp((y as f32 * k).round()) };
            if at != self.applied.position && position == self.applied.position {
                self.applied.position = at.clone();
                state.set_position(at);
            }
        }
        let (placement, minimized, position, size) = if state.peek() != (placement, minimized, position.clone(), size) {
            state.peek()
        } else {
            (placement, minimized, position, size)
        };
        let points = |v: Dp| ((v.0 / k).round() as i32).max(1);
        if size.is_specified()
            && ((size.width.0 - self.applied.size.width.0).abs() >= 0.5
                || (size.height.0 - self.applied.size.height.0).abs() >= 0.5)
        {
            self.applied.size = size;
            if placement == WindowPlacement::Floating {
                self.platform.set_window_size(points(size.width), points(size.height));
            }
        }
        if minimized != self.applied.minimized {
            self.applied.minimized = minimized;
            self.platform.set_minimized(minimized);
        }
        if placement != self.applied.placement {
            self.applied.placement = placement;
            self.platform.set_placement(placement);
        }
        if position != self.applied.position {
            self.applied.position = position.clone();
            if let Some((x, y)) = self.position_on_screen(&position, size.width, size.height) {
                self.platform.set_position(points(x), points(y));
            }
        }
        let front = state.to_front_requests();
        if front != self.applied.front {
            self.applied.front = front;
            self.platform.to_front();
        }
        let before = self.applied.properties.replace(p.clone()).unwrap_or_default();
        if before.resizable != p.resizable {
            self.platform.set_resizable(p.resizable);
        }
        if before.always_on_top != p.always_on_top {
            self.platform.set_always_on_top(p.always_on_top);
        }
        let same_icon = match (&before.icon, &p.icon) {
            (None, None) => true,
            (Some(a), Some(b)) => a.painter_eq(&**b),
            _ => false,
        };
        if !same_icon || !self.icon_told {
            self.icon_told = true;
            let image = p.icon.as_ref().and_then(|icon| self.painted(&**icon, 64));
            self.platform.set_icon(image.as_ref());
        }
    }

    /// `painter` drawn at `size` by `size` px (its intrinsic aspect kept, centered), as a tray or a title bar shows
    /// it: by the app's renderer (the bitmaps' own), read back; none without one.
    pub fn painted(&self, painter: &dyn kompoz_ui::draw::Painter, size: i32) -> Option<TrayImage> {
        paint_image(painter, size)
    }
}

/// Whether `e` is input (what a `Window` that isn't enabled leaves out).
fn is_input(e: &Event) -> bool {
    e.modifiers().is_some()
        || matches!(
            e,
            Event::MouseMove { .. }
                | Event::MouseDown { .. }
                | Event::MouseUp { .. }
                | Event::TouchDown { .. }
                | Event::TouchMove { .. }
                | Event::TouchMoves { .. }
                | Event::TouchCancel
                | Event::History { .. }
                | Event::TouchUp { .. }
                | Event::Stylus { .. }
                | Event::Wheel { .. }
                | Event::PinchBegin { .. }
                | Event::Pinch { .. }
                | Event::PinchEnd { .. }
                | Event::Text(_)
                | Event::Preedit { .. }
                | Event::DeleteSurroundingText { .. }
                | Event::SetSelection { .. }
                | Event::DropBegin { .. }
                | Event::DropMove { .. }
                | Event::DropComplete { .. }
        )
}

/// `painter` drawn into a bitmap of `size` by `size` px (its intrinsic aspect kept, centered) by the renderer set for
/// the app's bitmaps, its pixels read back (none where there is none, or nothing drew).
pub fn paint_image(painter: &dyn kompoz_ui::draw::Painter, size: i32) -> Option<TrayImage> {
    use kompoz_ui::draw::{Canvas, DrawScope};
    let size = size.max(1);
    let bitmap = ImageBitmap::new(
        size,
        size,
        kompoz_graphics::ImageBitmapConfig::Argb8888,
        true,
        &kompoz_graphics::colorspace::ColorSpaces::Srgb,
    );
    {
        let mut canvas = Canvas::from_image(&bitmap);
        let intrinsic = painter.intrinsic_size();
        let s = size as f32;
        let (w, h) = if intrinsic.is_specified() && intrinsic.width > 0.0 && intrinsic.height > 0.0 {
            let k = (s / intrinsic.width).min(s / intrinsic.height);
            (intrinsic.width * k, intrinsic.height * k)
        } else {
            (s, s)
        };
        let mut scope =
            DrawScope::over_canvas(&mut canvas, Size::new(s, s), LayoutDirection::Ltr, Density::new(1.0, 1.0));
        scope.translate_with((s - w) / 2.0, (s - h) / 2.0, |scope| painter.draw(scope, Size::new(w, h), 1.0, None));
    }
    if bitmap.version() > 0 {
        let rgba = bitmap.pixels();
        return Some(TrayImage { width: size, height: size, rgba: rgba.to_vec() });
    }
    None
}

impl<P: Platform> Drop for AppCore<P> {
    fn drop(&mut self) {
        if self.is_root() {
            stop_image_loader(); // (its threads end before the rest goes)
        }
        if self.flags.damage_check {
            self.logf(&format!(
                "kompoz: damage check: {} of {} frames differed",
                self.damage_wrong.get(),
                self.damage_frames.get()
            ));
        }
        if self.flags.keep_check {
            self.logf(&format!(
                "kompoz: keep check: {} of {} frames recorded otherwise",
                self.keep_wrong, self.keep_frames
            ));
        }
        self.host.take(); // (runs effect cleanups while the window still exists)
        self.release_gpu();
    }
}

/// `KOMPOZ_DAMAGE_CHECK`: the kept pixels (`kept`), with only what changed drawn again, against all of the frame
/// drawn afresh (`fresh`), `w` x `h` RGBA; why they differ (stale pixels on the screen), if they do.
fn compare_damage(kept: &[u8], fresh: &[u8], w: i32, h: i32, damage: Rect) -> Option<String> {
    if kept.len() != fresh.len() || kept.is_empty() {
        return None;
    }
    let (mut count, mut worst) = (0, 0);
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, -1, -1);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize * 4;
            let d = (0..4).map(|k| (kept[i + k] as i32 - fresh[i + k] as i32).abs()).max().unwrap_or(0);
            if d == 0 {
                continue;
            }
            count += 1;
            worst = worst.max(d);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    (count > 0).then(|| {
        format!(
            "kompoz: damage check: {count} pixels differ (by up to {worst}) in {x0},{y0} {}x{} px; drawn again: \
             {:.0},{:.0} {:.0}x{:.0} px",
            x1 - x0 + 1,
            y1 - y0 + 1,
            damage.left,
            damage.top,
            damage.width(),
            damage.height()
        )
    })
}

/// Pixels read back from a target (RGBA, premultiplied) as an image: the colors divided back out.
fn unpremultiplied(width: i32, height: i32, mut pixels: Vec<u8>) -> Option<ImageBitmap> {
    if pixels.len() != width as usize * height as usize * 4 {
        return None;
    }
    for p in pixels.as_chunks_mut::<4>().0 {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = (*c as u32 * 255 / a).min(255) as u8;
            }
        }
    }
    ImageBitmap::from_pixels(width, height, pixels)
}

/// The host's callbacks: images, bitmaps drawn to images and assets the core's own; the rest the platform's (C++'s
/// AppCore callbacks with the platform's `attach`ed).
struct AppCallbacks {
    platform: Rc<dyn PlatformCallbacks>,
    /// The core's renderer (another as a hidden window is shown again; none while it is hidden).
    renderer: Rc<RefCell<Weak<RefCell<Renderer>>>>,
    in_frame: Rc<Cell<bool>>,
    /// The MIME types of the app's own drag the platform took, until it ends (`Event::OwnDragEnded`).
    own_drag: Rc<RefCell<Option<Vec<String>>>>,
}

impl PlatformCallbacks for AppCallbacks {
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        self.platform.waker()
    }
    fn text_input(&self, active: bool, area: Rect, state: &TextEditState, options: &ImeOptions) {
        self.platform.text_input(active, area, state, options);
    }
    fn text_input_changed(&self, state: &TextEditState) {
        self.platform.text_input_changed(state);
    }
    fn cursor(&self, icon: PointerIcon) {
        self.platform.cursor(icon);
    }
    fn stylus_hover_icon(&self, icon: Option<PointerIcon>) {
        self.platform.stylus_hover_icon(icon);
    }
    fn load_image(&self, path: &str) -> Option<ImageBitmap> {
        load_image_file(path)
    }
    fn load_image_async(&self, model: &str, size: ImageRequestSize, done: Box<dyn FnOnce(Option<ImageBitmap>) + Send>) {
        load_image_async(model, size, done);
    }
    fn get_clip_entry(&self) -> Option<ClipEntry> {
        self.platform.get_clip_entry()
    }
    fn set_clip_entry(&self, entry: Option<ClipEntry>) {
        self.platform.set_clip_entry(entry);
    }
    fn system_bars_appearance(&self, light_status_bars: bool, light_navigation_bars: bool) {
        self.platform.system_bars_appearance(light_status_bars, light_navigation_bars);
    }
    fn location_changed(&self, path: &str) {
        self.platform.location_changed(path);
    }
    fn open_uri(&self, uri: &str) {
        self.platform.open_uri(uri);
    }
    fn haptic_feedback(&self, kind: HapticFeedbackType) {
        self.platform.haptic_feedback(kind);
    }
    fn hide_software_keyboard(&self) {
        self.platform.hide_software_keyboard();
    }
    fn can_control_ime(&self) -> bool {
        self.platform.can_control_ime()
    }
    fn ime_control_by(&self, px: f32) {
        self.platform.ime_control_by(px);
    }
    fn ime_control_end(&self, velocity: f32) {
        self.platform.ime_control_end(velocity);
    }
    /// `draw` (px, from 0, 0) into a texture of `size` at the window's density, read back (GraphicsLayer's
    /// `toImageBitmap`): in the frame going on, or one of its own between frames; none while the renderer is busy.
    fn render_image(&self, size: Size, draw: &dyn Fn(&mut dyn GraphicsContext)) -> Option<ImageBitmap> {
        let renderer = self.renderer.borrow().upgrade()?;
        let mut r = renderer.try_borrow_mut().ok()?;
        let (w, h) = ((size.width.ceil() as i32).max(1), (size.height.ceil() as i32).max(1));
        if w > kompoz_graphics::MAX_IMAGE_DIMENSION || h > kompoz_graphics::MAX_IMAGE_DIMENSION {
            return None;
        }
        let own = !self.in_frame.get(); // (asked for between frames: a frame of its own)
        if own {
            r.device_mut().begin_frame();
        }
        let usage = TextureUsage::SAMPLED | TextureUsage::RENDER_TARGET;
        let target = r.device_mut().create_texture(w, h, Format::Rgba8, usage);
        let mut pixels = Vec::new();
        if let Some(target) = &target {
            r.begin_target(target.id(), Some(ClearColor::TRANSPARENT));
            draw(&mut *r);
            r.end_target();
            r.flush();
            pixels = r.device_mut().read_pixels(target.id(), IRect::new(0, 0, w, h));
        }
        if own {
            r.device_mut().end_frame(false);
        }
        unpremultiplied(w, h, pixels)
    }
    fn autofill_focus(&self, id: u64, bounds: Rect, entered: bool) {
        self.platform.autofill_focus(id, bounds, entered);
    }
    fn autofill_value(&self, id: u64, text: &str) {
        self.platform.autofill_value(id, text);
    }
    fn autofill_visibility(&self, id: u64, visible: bool) {
        self.platform.autofill_visibility(id, visible);
    }
    fn autofill_done(&self, commit: bool) {
        self.platform.autofill_done(commit);
    }
    fn autofill_request(&self, id: u64, bounds: Rect) {
        self.platform.autofill_request(id, bounds);
    }
    fn system_gesture_exclusion(&self, rects: &[kompoz_graphics::IntRect]) {
        self.platform.system_gesture_exclusion(rects);
    }
    fn read_asset(&self, path: &str) -> Option<Vec<u8>> {
        read_file(path)
    }
    fn show_text_toolbar(&self, rect: Rect, actions: &TextToolbarActions) {
        self.platform.show_text_toolbar(rect, actions);
    }
    fn hide_text_toolbar(&self) {
        self.platform.hide_text_toolbar();
    }
    /// The platform's own drag of a drag within the app that left the window; while it runs, the window is told
    /// the app's own MIME types as it comes back over it.
    fn start_drag_and_drop(&self, drag: &kompoz_ui::draganddrop::PlatformDragAndDrop) -> bool {
        let taken = self.platform.start_drag_and_drop(drag);
        if taken {
            *self.own_drag.borrow_mut() = Some(drag.transfer_data.mime_types());
        }
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::lifecycle_state_of;
    use kompoz_lifecycle::LifecycleState;

    /// The window's state to the app's lifecycle state: minimized or hidden is created whatever the focus; shown,
    /// focused is resumed, unfocused started.
    #[test]
    fn the_windows_state_is_the_apps_lifecycle_state() {
        let cases = [
            (true, false, false, LifecycleState::Resumed),
            (false, false, false, LifecycleState::Started),
            (true, true, false, LifecycleState::Created),
            (false, true, false, LifecycleState::Created),
            (true, false, true, LifecycleState::Created),
            (false, false, true, LifecycleState::Created),
            (true, true, true, LifecycleState::Created),
        ];
        for (focused, minimized, hidden, expected) in cases {
            assert_eq!(lifecycle_state_of(focused, minimized, hidden), expected, "{focused} {minimized} {hidden}");
        }
    }
}

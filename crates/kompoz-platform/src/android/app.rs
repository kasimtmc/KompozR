//! The app as it runs on Android's UI thread (C++'s `AndroidPlatform`'s side that Java calls): made as the activity
//! is created, its core started with the view's first surface (it has a window to open then), the activity's events
//! handed to the core as they come, its frames on the display's beat (`AChoreographer`: one posted at a time), work
//! from other threads and a coroutine's delay waking the UI thread's looper (an eventfd, a timerfd), the window the
//! view's surface has followed (another after the background: the GPU device's surface made again, before the old
//! window is let go), its state saved into the activity's Bundle and given back after a process death; what the
//! system's services tell (`services`) handed over: the insets, back's steps, deep links, drags, the animator scale
//! and the wallpaper's colours.
//!
//! Everything runs on the UI thread: the natives (`natives`) and the looper's and choreographer's callbacks find the
//! app in a thread-local. A call that comes while the app is busy (Java calling back into native code from inside a
//! call of ours) has its events kept and handed over when that call is done.

use super::clip::Description;
use super::configuration::Configuration;
use super::drag::{self, DragEvent};
use super::input::{Motion, MotionDecoder, key_events};
use super::jni::{Env, Global, JObject, MethodId, Vm};
use super::ndk::{Assets, Looper, NativeWindow, api_level, post_frame_callback};
use super::platform::{AndroidPlatform, Shared};
use super::saved_state;
use super::services::Services;
use super::system::BackPhase;
use super::text_input::TextInput;
use super::{AndroidApp, log_error, log_line};
use crate::app::{AppCore, Event};
use crate::system::{set_asset_reader, set_cache_directory};
use crate::unix::sys;
use crate::unix::waker::Waker;
use kompoz_ui::platform::SystemInsets;
use std::cell::{Cell, RefCell};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// KompozActivity's lifecycle events (its `LIFECYCLE_*`), in order.
const LIFECYCLE: [Event; 7] = [
    Event::WillEnterBackground, // onPause
    Event::DidEnterBackground,  // onStop
    Event::WillEnterForeground, // onStart
    Event::DidEnterForeground,  // onResume
    Event::LowMemory,           // onTrimMemory (RUNNING_LOW and worse), onLowMemory
    Event::FocusGained,         // onWindowFocusChanged
    Event::FocusLost,
];

thread_local! {
    static APP: RefCell<Option<Running>> = const { RefCell::new(None) };
    /// Events that came while the app was busy, for when it is done.
    static LATE: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    /// The view's motion events' decoder (a touchpad's gesture under way): apart from the app, so that what comes
    /// while it is busy is decoded in order too.
    static MOTION: RefCell<MotionDecoder> = RefCell::new(MotionDecoder::default());
}

/// Each activity's app its number: what the callbacks posted for an app carry, so that one of an app gone is
/// ignored.
static GENERATION: AtomicUsize = AtomicUsize::new(0);

/// The process's VM (JNI_OnLoad's) and the app's entry (`android_app!`'s).
pub(super) static VM: std::sync::OnceLock<Vm> = std::sync::OnceLock::new();
pub(super) static ENTRY: std::sync::OnceLock<fn() -> AndroidApp> = std::sync::OnceLock::new();

/// What `nativeCreate` is given.
pub(super) struct Created<'a> {
    pub activity: JObject,
    pub view: JObject,
    pub assets: JObject,
    pub files_dir: PathBuf,
    pub external_files_dir: Option<PathBuf>,
    pub cache_dir: PathBuf,
    pub saved: Option<Vec<u8>>,
    pub env: Env<'a>,
}

/// The app of the activity now (C++'s gApp). Its fields drop in order: the core (the GPU device with it) before the
/// window it drew into, the looper's watches before the descriptors they watch.
pub(super) struct Running {
    core: AppCore<AndroidPlatform>,
    window: Option<NativeWindow>,
    _looper: Looper,
    shared: Rc<Shared>,
    timer: OwnedFd,
    activity: Global,
    view: Global,
    /// (The asset manager the asset reader reads, kept alive while it does.)
    _assets: Option<Global>,
    finish: Option<MethodId>,
    generation: usize,
    /// The core started (the view's first surface came).
    started: bool,
    /// The activity is stopped (in the background): no frames.
    stopped: bool,
    /// The app ended (its script's `quit`, `exit_application`): the activity finishing.
    ended: bool,
    frame_posted: Cell<bool>,
    /// Events before the core started, handed to it as it does (the lifecycle the activity went through).
    pending: Vec<Event>,
}

impl Drop for Running {
    fn drop(&mut self) {
        // (Before the asset manager's reference goes.)
        set_asset_reader(None);
    }
}

impl Running {
    /// The app of a new activity (an earlier one's is gone first: the activity was made again), not started.
    pub fn create(c: Created<'_>) -> Option<()> {
        let entry = ENTRY.get()?;
        let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        destroy();
        let env = c.env;
        let activity = env.new_global(c.activity)?;
        let view = env.new_global(c.view)?;
        let finish = env.class_of(c.activity).and_then(|class| env.method(&class, c"finish", c"()V"));
        // The APK's assets, where a path isn't a file (a `content://` URI's bytes from its provider); the app's
        // cache directory; relative paths (a script's shots) in the app's files.
        let assets_ref = env.new_global(c.assets);
        if let Some(assets) = Assets::from_java(&env, c.assets) {
            set_asset_reader(Some(Box::new(move |path| {
                if path.starts_with("content://") { super::network::read_content(path) } else { assets.read(path) }
            })));
        }
        set_cache_directory(Some(c.cache_dir.clone()));
        let _ = std::fs::create_dir_all(&c.files_dir);
        let _ = std::env::set_current_dir(&c.files_dir);
        // Work from other threads and a coroutine's delay wake the UI thread's looper.
        let wake = Arc::new(sys::eventfd().ok()?);
        let timer = sys::timerfd().ok()?;
        let mut looper = Looper::for_thread()?;
        if !looper.watch(wake.as_raw_fd(), super::natives::on_wake, generation)
            || !looper.watch(timer.as_raw_fd(), super::natives::on_timer, generation)
        {
            log_error("kompoz: the UI thread's looper refused KompozR's descriptors");
            return None;
        }
        let restored = saved_state::restore(c.saved.as_deref(), &c.files_dir);
        // The input method's side of the view; stylus handwriting where the system has it (Android 14).
        let text_input = TextInput::new(&env, c.view).map(Rc::new);
        if text_input.is_none() {
            log_error("kompoz: KompozView's input methods aren't there: no text input");
        }
        kompoz_ui::platform::set_stylus_handwriting_supported(api_level() >= 34);
        // Accessibility, content capture, translation, autofill and scroll capture: told through the view.
        super::delegate::attach(&env, c.view);
        MOTION.with(|m| *m.borrow_mut() = MotionDecoder::default());
        let shared = Rc::new(Shared {
            window: Rc::new(Cell::new(0)),
            size: Cell::new((0, 0)),
            configuration: RefCell::new(Configuration::default()),
            files_dir: c.files_dir,
            external_files_dir: c.external_files_dir,
            restored: RefCell::new(restored),
            waker: Waker::new(wake),
            media: Cell::new(None),
            view_configuration: Cell::new(None),
            text_input,
            services: RefCell::new(None),
            insets: RefCell::new(None),
            deep_links: RefCell::new(Vec::new()),
            animator_scale: Rc::new(Cell::new(1.0)),
            drop_entry: RefCell::new(None),
            drop_taken: Cell::new(false),
        });
        let made = entry();
        let mut config = made.config;
        config.args = vec![config.title.clone()]; // (the process's are app_process's)
        let core = made.content.core(AndroidPlatform::new(shared.clone()), &config);
        let running = Running {
            core,
            window: None,
            _looper: looper,
            shared,
            timer,
            activity,
            view,
            _assets: assets_ref,
            finish,
            generation,
            started: false,
            stopped: false,
            ended: false,
            frame_posted: Cell::new(false),
            pending: Vec::new(),
        };
        APP.with(|app| *app.borrow_mut() = Some(running));
        Some(())
    }

    fn is_activity(&self, env: &Env<'_>, object: JObject) -> bool {
        env.same(self.activity.object(), object)
    }

    fn is_view(&self, env: &Env<'_>, object: JObject) -> bool {
        env.same(self.view.object(), object)
    }

    /// An event for the core: handled now (a frame asked for after it), or kept until the core starts.
    fn deliver(&mut self, event: Event) {
        if !self.started {
            self.pending.push(event);
            return;
        }
        self.core.handle(&event);
        self.request_frame();
    }

    /// The events that came while the app was busy.
    fn deliver_late(&mut self) {
        loop {
            let late = LATE.with(|l| std::mem::take(&mut *l.borrow_mut()));
            if late.is_empty() {
                break;
            }
            for event in late {
                self.deliver(event);
            }
        }
    }

    /// The host, once the core started.
    pub(super) fn host(&self) -> Option<&kompoz_ui::KompozHost> {
        if self.started { self.core.host() } else { None }
    }

    /// The window's pixels in that area as the last frame drew them (the core's `read_frame`).
    pub(super) fn read_frame(&mut self, x: i32, y: i32, width: i32, height: i32) -> Option<(i32, i32, Vec<u8>)> {
        self.core.read_frame(x, y, width, height)
    }

    /// A frame on the display's next beat (one posted at a time).
    pub(super) fn request_frame(&self) {
        if self.ended || self.frame_posted.get() {
            return;
        }
        if post_frame_callback(super::natives::on_frame, self.generation) {
            self.frame_posted.set(true);
        }
    }

    /// The activity's configuration (as it starts, and as it changes): the metrics read again.
    pub fn configure(&mut self, configuration: Configuration) {
        kompoz_runtime::environment::set_system_languages(Some(configuration.languages.clone()));
        let zone = configuration.time_zone.clone();
        kompoz_runtime::environment::set_system_time_zone((!zone.is_empty()).then_some(zone));
        *self.shared.configuration.borrow_mut() = configuration;
        self.deliver(Event::MetricsChanged);
    }

    /// The view's surface came, changed size or went (none): the app starts with the first (it has a window to
    /// open then); another window after the background has the device's surface made again for it.
    pub fn surface(&mut self, env: &Env<'_>, surface: JObject, width: i32, height: i32) {
        let now = NativeWindow::from_surface(env, surface);
        let same = now.as_ref().map(NativeWindow::address) == self.window.as_ref().map(NativeWindow::address);
        self.shared.size.set(if now.is_some() { (width, height) } else { (0, 0) });
        if same {
            drop(now); // (our second reference on the same window)
            if self.window.is_some() {
                self.deliver(Event::Resized);
            }
            return;
        }
        self.shared.window.set(now.as_ref().map_or(0, NativeWindow::address));
        if self.started {
            // (The device lets go of the old window's surface — and makes the new one's — before it is released:
            // Android may hand its buffers to another the moment surfaceDestroyed returns.)
            self.core.surface_changed();
        }
        drop(std::mem::replace(&mut self.window, now));
        if !self.started {
            if self.window.is_some() {
                self.start(env);
            }
            return;
        }
        if self.window.is_none() {
            self.deliver(Event::Hidden);
        } else {
            self.deliver(Event::Shown); // (back: all of it drawn again)
            self.deliver(Event::Resized);
        }
    }

    /// The core started on the view's first window, then the events it missed.
    fn start(&mut self, env: &Env<'_>) {
        self.started = true;
        if !self.core.start() {
            log_error("kompoz: the app couldn't start (no GPU device): the activity finishes");
            self.end(env);
            return;
        }
        for event in std::mem::take(&mut self.pending) {
            self.core.handle(&event);
        }
        self.request_frame();
    }

    /// The activity's lifecycle (KompozActivity's `LIFECYCLE_*`).
    pub fn lifecycle(&mut self, code: i32) {
        let Some(event) = usize::try_from(code).ok().and_then(|i| LIFECYCLE.get(i)) else { return };
        match event {
            Event::DidEnterBackground => self.stopped = true,
            Event::WillEnterForeground => self.stopped = false,
            _ => {}
        }
        self.deliver(event.clone());
    }

    // ---- The system's services (`services`' natives) ----

    /// KompozActivity's service objects, for the platform's callbacks; the animator scale as it is now.
    pub fn set_services(&mut self, services: Rc<Services>) {
        self.shared.animator_scale.set(services.animator_scale());
        *self.shared.services.borrow_mut() = Some(services);
    }

    /// A link the app is opened with: its navigation goes there (once the host is there).
    pub fn deep_link(&mut self, link: String) {
        match self.host() {
            Some(host) => {
                host.open_deep_link(&link);
                self.request_frame();
            }
            None => self.shared.deep_links.borrow_mut().push(link),
        }
    }

    /// The insets as they are now (kept for the host as it starts).
    pub fn insets(&mut self, insets: SystemInsets) {
        *self.shared.insets.borrow_mut() = Some(insets.clone());
        if let Some(host) = self.host() {
            host.set_system_insets(insets);
            self.request_frame();
        }
    }

    /// A step of back: the gesture's to the host's back handling (Compose's PredictiveBackHandler); back itself
    /// as an event (the host's, or the system's if nothing in the app takes it).
    pub fn back(&mut self, step: BackPhase) {
        if let BackPhase::Invoked = step {
            self.deliver(Event::Back);
            return;
        }
        let Some(host) = self.host() else { return };
        match step {
            BackPhase::Started(event) => host.back_started(event),
            BackPhase::Progressed(event) => host.back_progressed(event),
            BackPhase::Cancelled => host.back_cancelled(),
            BackPhase::Invoked => {}
        }
        self.request_frame();
    }

    /// The animator duration scale changed (read by animations as they run).
    pub fn animator_scale(&mut self, scale: f32) {
        self.shared.animator_scale.set(scale);
        self.request_frame();
    }

    /// The wallpaper's colours changed: the system's palettes read again.
    pub fn wallpaper_changed(&mut self) {
        self.deliver(Event::WallpaperChanged);
    }

    /// A drag over the view (`drag`'s events), handed to the core; for a drop, whether a target took it.
    pub fn drag(&mut self, event: &DragEvent, description: Option<Description>) -> bool {
        if !self.started {
            return false;
        }
        if event.action == drag::action::DROP && !event.own {
            let entry = self.shared.services().zip(description).map(|(s, d)| s.drop_entry(d));
            *self.shared.drop_entry.borrow_mut() = entry;
        }
        self.shared.drop_taken.set(false);
        // (Android's drags have no action of their own: the app's own one taken is a copy.)
        for e in drag::events(event, &[]) {
            self.deliver(e);
        }
        self.shared.drop_entry.borrow_mut().take();
        self.shared.drop_taken.get()
    }

    /// The input devices (KompozInputDevices' media info) and the system's gesture thresholds, as they start and
    /// as devices come and go or the configuration changes.
    pub fn devices(&mut self, media: &[i32], view_configuration: &[f32]) {
        self.shared.media.set(Some(super::devices::media_info(media)));
        let density = self.shared.configuration.borrow().density;
        self.shared.view_configuration.set(Some(super::devices::view_configuration(view_configuration, density)));
        self.deliver(Event::DevicesChanged);
        self.deliver(Event::MetricsChanged); // (the view configuration set on the host again)
    }

    /// A batch of the input connection's edits (`text_input`'s `edit`).
    pub fn ime_edit(&mut self, session: i32, ops: &[i32], texts: &[u8], text: String, offsets: &[i32]) {
        let (Some(t), Some(host)) = (self.shared.text_input.clone(), self.core.host()) else { return };
        t.edit(host, session, ops, texts, text, offsets);
        self.request_frame();
    }

    /// The input method's action key.
    pub fn ime_action(&mut self, session: i32, editor_action: i32) {
        let (Some(t), Some(host)) = (self.shared.text_input.clone(), self.core.host()) else { return };
        t.action(host, session, editor_action);
        self.request_frame();
    }

    /// Content the keyboard commits; whether the field took it.
    pub fn ime_content(&mut self, session: i32, mime_types: &str, uri: String, link: Option<String>) -> bool {
        let (Some(t), Some(host)) = (self.shared.text_input.clone(), self.core.host()) else { return false };
        let taken = t.content(host, session, mime_types, uri, link);
        self.request_frame();
        taken
    }

    /// A text toolbar item clicked: its action.
    pub fn toolbar_action(&mut self, item: i32) {
        let action = self.shared.text_input.as_ref().and_then(|t| t.toolbar_action(item));
        if let Some(action) = action {
            action();
            self.request_frame();
        }
    }

    /// A key; whether KompozR took it (every key is Compose's, so all of them).
    pub fn key(&mut self, code: i32, down: bool, meta: i32, typed: &str, time: i64) -> bool {
        for event in key_events(code, down, meta, typed, time) {
            self.deliver(event);
        }
        true
    }

    /// What goes into the activity's Bundle: the host's state (or, before the core started, the one restored and
    /// not yet given to it), with the size guard ([`saved_state`]).
    pub fn save_state(&self) -> Option<Vec<u8>> {
        let state = match self.core.host() {
            Some(host) if self.started => host.save_state(),
            _ => self.shared.restored.borrow().clone()?,
        };
        let token = format!("{:x}", sys::monotonic_now());
        let saved = saved_state::save(&state, &self.shared.files_dir, &token);
        if saved.is_none() {
            log_error(&format!("kompoz: the saved state ({} bytes) couldn't be kept", state.len()));
        }
        saved
    }

    /// The display's beat (C++'s `frame`): a frame, unless there is nothing to show it on; then a wake due before
    /// the next beat taken in it (a timer would make it the one after), else the timer armed for it; what is left
    /// until the next beat (less 2 ms for its input) the host's for work between frames (a lazy list's next items).
    pub fn frame(&mut self, frame_time: i64) {
        self.frame_posted.set(false);
        if !self.started || self.ended || self.stopped || self.window.is_none() {
            return;
        }
        let next = self.core.frame(Some(frame_time));
        super::delegate::after_frame(self);
        if next.quit || self.core.quitting() {
            if let Some(env) = VM.get().and_then(Vm::env) {
                self.end(&env);
            }
            return;
        }
        let period = self.shared.configuration.borrow().frame_period_ns();
        let now = sys::monotonic_now();
        let soon = next.wake_at.is_some_and(|at| at - now < period);
        if next.busy || soon {
            self.request_frame();
        }
        let _ = sys::timerfd_set_at(self.timer.as_fd(), if next.busy || soon { None } else { next.wake_at });
        let until = frame_time + period - 2_000_000;
        let now = sys::monotonic_now();
        if until > now {
            self.core.idle(until - now);
        }
    }

    /// The looper's wake or timer: work for a frame.
    pub fn woken(&mut self) {
        self.deliver(Event::Wake);
    }

    /// The app ended: the activity finishes (its onDestroy lets the app go).
    fn end(&mut self, env: &Env<'_>) {
        if std::mem::replace(&mut self.ended, true) {
            return;
        }
        log_line("kompoz: the app ended: the activity finishes");
        if let Some(finish) = self.finish {
            env.call_void(self.activity.object(), finish, &[]);
        }
    }
}

// ---- The app in its thread-local, for the natives and callbacks ----

/// `f` on the app, if there is one and it isn't busy (else none: a busy one's events are kept with [`later`]); the
/// events that came meanwhile handed over after it.
fn with_app<R>(f: impl FnOnce(&mut Running) -> R) -> Option<R> {
    APP.with(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        let app = slot.as_mut()?;
        let r = f(app);
        app.deliver_late();
        Some(r)
    })
}

/// Whether the app is busy (a call of ours under way): what comes is kept for later.
fn busy() -> bool {
    APP.with(|cell| cell.try_borrow_mut().is_err())
}

/// `events` kept for when the app's call under way is done.
fn later(events: Vec<Event>) {
    LATE.with(|l| l.borrow_mut().extend(events));
}

/// `f` on the app of the activity `activity` (none: another's, or no app).
pub(super) fn with_activity<R>(env: &Env<'_>, activity: JObject, f: impl FnOnce(&mut Running) -> R) -> Option<R> {
    with_app(|app| app.is_activity(env, activity).then(|| f(app))).flatten()
}

/// `f` on the app of the view `view`.
pub(super) fn with_view<R>(env: &Env<'_>, view: JObject, f: impl FnOnce(&mut Running) -> R) -> Option<R> {
    with_app(|app| app.is_view(env, view).then(|| f(app))).flatten()
}

/// The view's motion event: decoded, its events handed to the app, or kept while it is busy.
pub(super) fn view_motion(env: &Env<'_>, view: JObject, motion: &Motion) {
    let events = MOTION.with(|m| m.borrow_mut().events(motion));
    if busy() {
        later(events);
        return;
    }
    with_view(env, view, |app| {
        for event in events {
            app.deliver(event);
        }
    });
}

/// `f` on the app of the callback posted for `generation` (none: the app it was for is gone).
pub(super) fn with_generation<R>(generation: usize, f: impl FnOnce(&mut Running) -> R) -> Option<R> {
    with_app(|app| (app.generation == generation).then(|| f(app))).flatten()
}

/// The activity's app gone (its onDestroy; or another activity's app made): the core's end (KOMPOZ_STATE_FILE
/// written), then the core and its device, then the window.
pub(super) fn destroy() {
    let running = APP.with(|cell| cell.try_borrow_mut().ok().and_then(|mut slot| slot.take()));
    if let Some(mut running) = running {
        running.core.quit();
        drop(running);
        super::delegate::detach();
    }
}

/// The activity `activity`'s app gone, if it is the app now.
pub(super) fn destroy_activity(env: &Env<'_>, activity: JObject) {
    if with_activity(env, activity, |_| ()).is_some() {
        destroy();
    }
}

/// After a panic (logged by the hook): the app let go and its activity finished (what is left of it can't be
/// trusted to draw again).
pub(super) fn failed() {
    let running = APP.with(|cell| cell.try_borrow_mut().ok().and_then(|mut slot| slot.take()));
    let Some(mut running) = running else { return };
    if let Some(env) = VM.get().and_then(Vm::env) {
        running.end(&env);
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(running)));
}

//! The app core's platform on Android (C++'s `AndroidPlatform : Platform`): the view's window to draw into with
//! Vulkan, its size, the activity's configuration as the metrics, the options from the app's files, logcat, the
//! monotonic clock (System.nanoTime's, MotionEvent's and the choreographer's), the host's waker (the looper's
//! eventfd), the state saved before a process death given back to the host as it starts; the input devices as the
//! media info and the system's gesture thresholds as the view configuration; the input method, the pointer's icons
//! and the text toolbar (`text_input`) as the host's callbacks; and the system's services (`services`): the
//! clipboard, the system bars' icons, URIs, haptics, the keyboard moved by hand, gesture exclusion and drags as the
//! host's callbacks, the insets and deep links given to the host as it starts, the system's palettes and bold text
//! as the metrics are read, and after each frame what the host asks of the window — back taken, the screen kept
//! on, sensitive content, the refresh rate voted for (busy: the display's fastest) —, each frame's work told to the
//! system's performance hints, the animator duration scale as the motion duration scale.
//!
//! What it keeps is [`Shared`] with the natives (`super::app`), which change it as the activity tells them; the core
//! reads it through the platform. All on the UI thread.

use super::autofill::Notice;
use super::configuration::{Configuration, option_file, option_value};
use super::delegate::{Pending, queue_autofill};
use super::performance::{Hints, set_frame_rate};
use super::services::Services;
use super::system::{
    AdjustedResolver, FrameRateVoter, animator_scale, category_index, dynamic_palette, surface_rate, wallpaper_palette,
};
use super::text_input::TextInput;
use super::vulkan;
use crate::app::{Event, Metrics, Platform, WindowSpec};
use crate::unix::waker::Waker;
use kompoz_gpu::device::{Backend, Surface};
use kompoz_graphics::{IntRect, Rect};
use kompoz_text::font::FontFamilyResolver;
use kompoz_ui::KompozHost;
use kompoz_ui::clipboard::ClipEntry;
use kompoz_ui::draganddrop::PlatformDragAndDrop;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use kompoz_ui::motion_duration_scale::MotionDurationScale;
use kompoz_ui::platform::{
    HapticFeedbackType, ImeOptions, PlatformCallbacks, PointerIcon, SystemInsets, TextEditState, TextToolbarActions,
    ViewConfiguration,
};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// What the platform and the natives share.
pub(super) struct Shared {
    /// The view's native window now (its address; 0: none, the app in the background).
    pub window: Rc<Cell<usize>>,
    /// Its size in pixels, as the view's surface last said.
    pub size: Cell<(i32, i32)>,
    pub configuration: RefCell<Configuration>,
    /// The app's files (`Context.getFilesDir`) and external files (`getExternalFilesDir(null)`; empty: none).
    pub files_dir: PathBuf,
    pub external_files_dir: Option<PathBuf>,
    /// The state saved before the process ended, for the host as it starts.
    pub restored: RefCell<Option<Vec<u8>>>,
    pub waker: Waker,
    /// The input devices as KompozInputDevices told them (none yet: the configuration's touchscreen and keyboard).
    pub media: Cell<Option<MediaInfo>>,
    /// The system's gesture thresholds (none yet: the defaults at the density).
    pub view_configuration: Cell<Option<ViewConfiguration>>,
    /// The input method's side (none: KompozView's methods weren't found; logged).
    pub text_input: Option<Rc<TextInput>>,
    /// The system's services (KompozActivity's objects; none until it gave them, or if they weren't there).
    pub services: RefCell<Option<Rc<Services>>>,
    /// The insets as KompozInsets last told them (for the host as it starts).
    pub insets: RefCell<Option<SystemInsets>>,
    /// Links the app was opened with before its host was there.
    pub deep_links: RefCell<Vec<String>>,
    /// The animator duration scale as the system last said (what the motion duration scale reads).
    pub animator_scale: Rc<Cell<f32>>,
    /// The drop being taken: what it carries (read from its ClipData as a target asks), and whether a target took
    /// it.
    pub drop_entry: RefCell<Option<ClipEntry>>,
    pub drop_taken: Cell<bool>,
}

impl Shared {
    /// The system's services, if KompozActivity gave them.
    pub fn services(&self) -> Option<Rc<Services>> {
        self.services.borrow().clone()
    }
}

/// What the platform last told the window, and its own state between frames (the UI thread's).
#[derive(Default)]
struct Told {
    keep_screen_on: Option<bool>,
    sensitive: Option<bool>,
    wants_back: Option<bool>,
    voter: FrameRateVoter,
    /// The window the refresh rate was voted on (another: voted again).
    voted_window: usize,
    /// Fingers on the screen now (busy while there are).
    fingers: i32,
    /// The weight adjustment the host's fonts have, and the resolver it was put over.
    font: Option<(i32, Rc<dyn FontFamilyResolver>)>,
    hints: Option<Hints>,
}

/// The platform (one: Android gives an app one window, so a `Window` gets none).
pub struct AndroidPlatform {
    pub(super) shared: Rc<Shared>,
    told: Told,
}

impl AndroidPlatform {
    pub(super) fn new(shared: Rc<Shared>) -> AndroidPlatform {
        AndroidPlatform { shared, told: Told::default() }
    }
}

/// What the host asks of the platform: the waker, the input method, the pointer's icons, the text toolbar, and the
/// system's services (the clipboard, the system bars, URIs, haptics, the keyboard moved by hand, gesture exclusion,
/// drags out of the window).
struct AndroidCallbacks {
    waker: Arc<dyn Fn() + Send + Sync>,
    text_input: Option<Rc<TextInput>>,
    shared: Rc<Shared>,
}

impl PlatformCallbacks for AndroidCallbacks {
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        Some(self.waker.clone())
    }
    fn text_input(&self, active: bool, area: Rect, state: &TextEditState, options: &ImeOptions) {
        if let Some(t) = &self.text_input {
            t.text_input(active, area, state, options);
        }
    }
    fn text_input_changed(&self, state: &TextEditState) {
        if let Some(t) = &self.text_input {
            t.update(state);
        }
    }
    fn update_text_layout_result(&self, layout: &kompoz_ui::platform_text_layout::PlatformTextLayout) {
        if let Some(t) = &self.text_input {
            t.update_text_layout_result(layout);
        }
    }
    fn hide_software_keyboard(&self) {
        if let Some(t) = &self.text_input {
            t.hide_keyboard();
        }
    }
    fn start_stylus_handwriting(&self) {
        if let Some(t) = &self.text_input {
            t.start_stylus_handwriting();
        }
    }
    fn cursor(&self, icon: PointerIcon) {
        if let Some(t) = &self.text_input {
            t.cursor(icon);
        }
    }
    fn stylus_hover_icon(&self, icon: Option<PointerIcon>) {
        if let Some(t) = &self.text_input {
            t.stylus_hover_icon(icon);
        }
    }
    fn show_text_toolbar(&self, rect: Rect, actions: &TextToolbarActions) {
        if let Some(t) = &self.text_input {
            t.show_text_toolbar(rect, actions);
        }
    }
    fn hide_text_toolbar(&self) {
        if let Some(t) = &self.text_input {
            t.hide_text_toolbar();
        }
    }
    // Autofill (Compose's AndroidAutofillManager over the AutofillManager): told after the frame (`delegate`).
    fn autofill_focus(&self, id: u64, bounds: Rect, entered: bool) {
        let Some(id) = autofill_id(id) else { return };
        let notice = if entered { Notice::Entered { id, bounds } } else { Notice::Exited { id } };
        queue_autofill(Pending::Notice(notice));
    }
    fn autofill_value(&self, id: u64, text: &str) {
        queue_autofill(Pending::Value(id, text.to_string()));
    }
    fn autofill_visibility(&self, id: u64, visible: bool) {
        if let Some(id) = autofill_id(id) {
            queue_autofill(Pending::Notice(Notice::Visibility { id, visible }));
        }
    }
    fn autofill_done(&self, commit: bool) {
        queue_autofill(Pending::Notice(Notice::Done { commit }));
    }
    fn autofill_request(&self, id: u64, bounds: Rect) {
        if let Some(id) = autofill_id(id) {
            queue_autofill(Pending::Notice(Notice::Request { id, bounds }));
        }
    }
    // The system's services (Compose's AndroidClipboard, WindowInsetsController, UriHandler, HapticFeedback, …).
    fn get_clip_entry(&self) -> Option<ClipEntry> {
        self.shared.services()?.clip_entry()
    }
    fn set_clip_entry(&self, entry: Option<ClipEntry>) {
        if let Some(s) = self.shared.services() {
            s.set_clip_entry(entry.as_ref());
        }
    }
    fn system_bars_appearance(&self, light_status_bars: bool, light_navigation_bars: bool) {
        if let Some(s) = self.shared.services() {
            s.system_bars_appearance(light_status_bars, light_navigation_bars);
        }
    }
    fn open_uri(&self, uri: &str) {
        if let Some(s) = self.shared.services() {
            s.open_uri(uri);
        }
    }
    fn haptic_feedback(&self, kind: HapticFeedbackType) {
        if let Some(s) = self.shared.services() {
            s.haptic(kind);
        }
    }
    fn can_control_ime(&self) -> bool {
        self.shared.services().is_some_and(|s| s.can_control_ime())
    }
    fn ime_control_by(&self, px: f32) {
        if let Some(s) = self.shared.services() {
            s.ime_control_by(px);
        }
    }
    fn ime_control_end(&self, velocity: f32) {
        if let Some(s) = self.shared.services() {
            s.ime_control_end(velocity);
        }
    }
    fn system_gesture_exclusion(&self, rects: &[IntRect]) {
        if let Some(s) = self.shared.services() {
            s.gesture_exclusion(rects);
        }
    }
    fn start_drag_and_drop(&self, drag: &PlatformDragAndDrop) -> bool {
        self.shared.services().is_some_and(|s| s.start_drag(drag))
    }
}

/// A node's autofill virtual id (its semantics id).
fn autofill_id(id: u64) -> Option<i32> {
    i32::try_from(id).ok()
}

impl AndroidPlatform {
    /// An option's own value (none: not set), as its file in the app's files or external files has it, else the
    /// process's environment (`adb shell`'s can't reach an app, but a wrapper script's does: `wrap.sh`).
    fn read_option(&self, name: &str) -> Option<String> {
        let file = option_file(name);
        let dirs = std::iter::once(&self.shared.files_dir).chain(self.shared.external_files_dir.as_ref());
        for dir in dirs {
            if let Ok(text) = std::fs::read_to_string(dir.join(&file)) {
                return Some(option_value(&text));
            }
        }
        kompoz_runtime::environment::var(name)
    }

    fn number(&self, name: &str) -> Option<f32> {
        self.read_option(name).and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite() && *v > 0.0)
    }
}

impl Platform for AndroidPlatform {
    fn option(&self, name: &str) -> Option<String> {
        self.read_option(name)
    }

    fn log(&self, message: &str) {
        super::log_line(message);
    }

    fn now(&self) -> i64 {
        crate::unix::sys::monotonic_now()
    }

    /// Vulkan alone: Android 10 (the app's minSdk) has Vulkan 1.1 on every 64-bit device it launched on.
    fn backends(&self) -> Vec<Backend> {
        vec![Backend::Vulkan]
    }

    fn open_window(&mut self, _: &WindowSpec, backend: Backend) -> Result<Surface, String> {
        if backend != Backend::Vulkan {
            return Err(format!("no {} on Android", backend.name()));
        }
        let shared = self.shared.clone();
        Ok(Surface {
            vulkan: Some(vulkan::surface(self.shared.window.clone())),
            drawable_size: Some(Box::new(move || shared.size.get())),
            ..Surface::default()
        })
    }

    /// (The window is the view's: it stays.)
    fn close_window(&mut self) {}

    fn show_error(&mut self, message: &str) {
        super::log_error(message);
    }

    fn metrics(&self) -> Metrics {
        let (w, h) = self.shared.size.get();
        let scale = self.number("KOMPOZ_SCALE");
        let font_scale = self.number("KOMPOZ_FONT_SCALE");
        self.shared.configuration.borrow().metrics(w, h, scale, font_scale)
    }

    fn pixel_size(&self) -> (i32, i32) {
        let (w, h) = self.shared.size.get();
        (w.max(1), h.max(1))
    }

    /// (The system sizes the activity's window.)
    fn set_window_size(&mut self, _: i32, _: i32) {}

    /// The input devices as KompozInputDevices told them (the most precise pointer, a physical keyboard, a
    /// microphone, a camera, a TV's or a car's distance); before it has, the touchscreen and the keyboard as the
    /// configuration says.
    fn media(&mut self, _probe: bool) -> MediaInfo {
        if let Some(media) = self.shared.media.get() {
            return media;
        }
        let c = self.shared.configuration.borrow();
        MediaInfo {
            pointer_precision: if c.touchscreen { PointerPrecision::Coarse } else { PointerPrecision::None },
            physical_keyboard: c.physical_keyboard,
            ..MediaInfo::default()
        }
    }

    fn attach(&mut self, _: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        Rc::new(AndroidCallbacks {
            waker: self.shared.waker.as_fn(),
            text_input: self.shared.text_input.clone(),
            shared: self.shared.clone(),
        })
    }

    /// The state saved before the process ended, back in the host (Compose's saved instance state restored); the
    /// insets told so far, the links the app was opened with.
    fn started(&mut self, host: &KompozHost) {
        if let Some(state) = self.shared.restored.borrow_mut().take() {
            host.restore_state(&state);
        }
        if let Some(insets) = self.shared.insets.borrow().clone() {
            host.set_system_insets(insets);
        }
        for link in std::mem::take(&mut *self.shared.deep_links.borrow_mut()) {
            host.open_deep_link(&link);
        }
    }

    /// The system's gesture thresholds (touch slop, long press, double tap, flings, handwriting slop) as the
    /// host's, over the defaults the density gave it; the system's palettes and bold text read again.
    fn metrics_updated(&mut self, host: &KompozHost) {
        if let Some(configuration) = self.shared.view_configuration.get() {
            host.set_view_configuration(configuration);
        }
        self.palettes(host);
        self.font_weight(host);
    }

    /// The fingers on the screen counted (the app is busy while there are); the palettes read again as the
    /// wallpaper's colours change; the performance hints' session closed in the background.
    fn handled(&mut self, host: &KompozHost, event: &Event) {
        match event {
            Event::TouchDown { .. } => self.told.fingers += 1,
            Event::TouchUp { .. } => self.told.fingers = (self.told.fingers - 1).max(0),
            Event::TouchCancel | Event::WillEnterBackground => self.told.fingers = 0,
            Event::WallpaperChanged => self.palettes(host),
            Event::DidEnterBackground => self.told.hints = None,
            _ => {}
        }
    }

    /// The input method told where the field and its caret are now, and the field's state where its Editable
    /// differs (a key typed, the app changed it); accessibility, content capture and autofill told what changed;
    /// the window told what the host asks of it as it changes (back taken, the screen kept on, sensitive content,
    /// the refresh rate).
    fn composed(&mut self, host: &KompozHost, redraw: bool, frame_time: i64) {
        if let Some(t) = &self.shared.text_input {
            t.sync(host);
        }
        super::delegate::composed(host, redraw);
        self.tell_window(host, frame_time);
    }

    /// A frame's own work, told to the system's performance hints (a session opened for the UI thread on the
    /// first, its target the display's beat).
    fn frame_work(&mut self, ns: i64) {
        let target = self.shared.configuration.borrow().frame_period_ns();
        match &mut self.told.hints {
            Some(hints) => hints.set_target(target),
            None => self.told.hints = Hints::new(target),
        }
        if let Some(hints) = &self.told.hints {
            hints.report(ns);
        }
    }

    /// Back nothing in the app took: the system's (the activity finished, or its task moved back).
    fn leave_app(&mut self) {
        if let Some(s) = self.shared.services() {
            s.leave();
        }
    }

    /// The animator duration scale as the system has it now, read as each animation asks (Compose's window
    /// MotionDurationScale over `Settings.Global.ANIMATOR_DURATION_SCALE`).
    fn motion_duration_scale(&self) -> Option<MotionDurationScale> {
        let scale = self.shared.animator_scale.clone();
        Some(MotionDurationScale::from_fn(move || animator_scale(scale.get())))
    }

    fn take_drop_entry(&mut self) -> Option<ClipEntry> {
        self.shared.drop_entry.borrow_mut().take()
    }

    fn drop_done(&mut self, taken: bool) {
        self.shared.drop_taken.set(taken);
    }
}

impl AndroidPlatform {
    /// Android 12's system palette (and the wallpaper's colours' palettes) as the host's, read now.
    fn palettes(&self, host: &KompozHost) {
        let Some(s) = self.shared.services() else { return };
        host.set_dynamic_tonal_palette(s.dynamic_colors().and_then(|c| dynamic_palette(&c)));
        host.set_wallpaper_tonal_palette(s.wallpaper_colors().and_then(|c| wallpaper_palette(&c)));
    }

    /// Bold text: the host's fonts resolved heavier by the system's adjustment (Compose's
    /// AndroidFontResolveInterceptor), or as they were when it is off again.
    fn font_weight(&mut self, host: &KompozHost) {
        let adjustment = self.shared.configuration.borrow().font_weight_adjustment;
        if self.told.font.as_ref().map_or(0, |(a, _)| *a) == adjustment {
            return;
        }
        let base = match self.told.font.take() {
            Some((_, base)) => base,
            None => host.font_family_resolver(),
        };
        if adjustment == 0 {
            host.set_font_family_resolver(base);
        } else {
            host.set_font_family_resolver(Rc::new(AdjustedResolver { inner: base.clone(), adjustment }));
            self.told.font = Some((adjustment, base));
        }
    }

    /// What the host asks of the window, told as it changes: back (Android 13's callback while the app takes it),
    /// the screen kept on, sensitive content (Android 15), and the refresh rate — the app's own preference, the
    /// display's fastest while it is busy (a finger down, a frame wanted) and a second after — on the view's window
    /// (`ANativeWindow_setFrameRate`, Android 11) and the view (Android 15's categories).
    fn tell_window(&mut self, host: &KompozHost, frame_time: i64) {
        let Some(s) = self.shared.services() else { return };
        let told = &mut self.told;
        let wants_back = host.wants_back();
        if told.wants_back != Some(wants_back) {
            told.wants_back = Some(wants_back);
            s.set_back_enabled(wants_back);
        }
        let keep = host.keep_screen_on();
        if told.keep_screen_on != Some(keep) {
            told.keep_screen_on = Some(keep);
            s.keep_screen_on(keep);
        }
        let sensitive = host.sensitive_content();
        if told.sensitive != Some(sensitive) {
            told.sensitive = Some(sensitive);
            s.content_sensitive(sensitive);
        }
        let window = self.shared.window.get();
        if window != told.voted_window {
            told.voted_window = window;
            told.voter.reset();
        }
        let busy = told.fingers > 0 || host.has_pending_work();
        let vote = told.voter.vote(frame_time, busy, host.preferred_frame_rate(), host.preferred_frame_rate_category());
        if let Some(vote) = vote {
            let fastest = self.shared.configuration.borrow().fastest_refresh_rate;
            set_frame_rate(window, surface_rate(vote, fastest));
            s.frame_rate(vote.rate, category_index(vote.category));
        }
    }
}

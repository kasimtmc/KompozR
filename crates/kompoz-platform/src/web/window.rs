//! The app's window on the web: the page's canvas (C++'s), drawn into with WebGPU (the `GPUDevice` the loader asked
//! for), its size the page's (or the size the app gives it), its pixels the CSS pixels times the device pixel ratio
//! (`KOMPOZ_SCALE` instead), the theme, reduced motion, the pointer's precision and the screen's safe area from the
//! page's media queries and CSS; what the app asks of it (its title, full screen, the cursor, the text area under
//! the focused field, the clipboard, links, vibration, the screen kept on, the address's fragment) as the page's
//! calls ([`sys`]).

use super::aria::{self, Mirror};
use super::clipboard;
use super::ime::TextArea;
use super::page::{PageState, cursor_name, keyboard_kind};
use super::sys;
use crate::accessibility::{Snapshot, Window as A11yWindow};
use crate::app::{Event, Metrics, Platform, WindowSpec};
use crate::system::preferred_language;
use kompoz_gpu::device::{Backend, Surface, WebGpuSurface};
use kompoz_graphics::Rect;
use kompoz_ui::KompozHost;
use kompoz_ui::clipboard::ClipEntry;
use kompoz_ui::draganddrop::PlatformDragAndDrop;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use kompoz_ui::motion_duration_scale::MotionDurationScale;
use kompoz_ui::platform::{HapticFeedbackType, ImeOptions, PlatformCallbacks, PointerIcon, TextEditState};
use kompoz_ui::window::{WindowId, WindowPlacement};
use kompoz_web::JsObject;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// What the platform keeps of the page, shared by its window and the callbacks it gives the host.
pub(super) struct Shared {
    /// The canvas (the glue's handle), and the `GPUDevice` the loader asked for.
    canvas: Option<JsObject>,
    gpu: Option<JsObject>,
    pub(super) page: RefCell<PageState>,
    /// The canvas's size as the app set it (none: the page's).
    fixed: Cell<Option<(f64, f64)>>,
    /// The drawable's size in pixels (the device reads it).
    drawable: Rc<Cell<(i32, i32)>>,
    /// px per CSS pixel, as `KOMPOZ_SCALE` says (none: the device pixel ratio).
    scale_override: Option<f64>,
    /// The window drawn into the canvas (the app's main one, or its first `Window`).
    pub(super) owner: Cell<Option<WindowId>>,
    pub(super) text_area: RefCell<TextArea>,
    /// A field takes the keyboard's typing.
    field_active: Cell<bool>,
    /// The text area's place over the field (CSS pixels) and its keyboard, as last set.
    text_input: RefCell<((f64, f64, f64, f64), String)>,
    /// What the clipboard holds as far as the app knows: what it copied, or what a paste brought.
    pub(super) clip: RefCell<Option<ClipEntry>>,
    /// A copy or cut event is being handled (what the app copies goes into it).
    pub(super) in_clip_event: Cell<bool>,
    /// What a drop brings (the app core takes it at its `DropData`).
    pub(super) drop_entry: RefCell<Option<ClipEntry>>,
    /// Animations end at once (the page prefers reduced motion).
    reduced_motion: Rc<Cell<bool>>,
    keep_awake: Cell<bool>,
    /// The address's fragment as the app last set it.
    pub(super) location: RefCell<String>,
    /// The app's own drag the page took (its drag and drop's), until it ends.
    pub(super) dragging_out: Cell<bool>,
    /// The page handles a `dragstart` (a drag the browser begins): the app's drag may be handed to it now.
    pub(super) drag_starting: Cell<bool>,
    /// The ARIA mirror of the canvas's window (none: `KOMPOZ_ARIA=0`).
    pub(super) aria: RefCell<Option<Aria>>,
    /// The field the keyboard types into is multi-line (its options' `single_line` false), while one is.
    input_multi_line: Cell<Option<bool>>,
}

/// The ARIA mirror as published: the tree the page shows, whether the app's changed since (and whether that is to
/// be published at once: the field typed into changed), when it was published (the platform's clock, ns).
pub(super) struct Aria {
    pub(super) mirror: Mirror,
    dirty: bool,
    urgent: bool,
    last: Option<i64>,
}

impl Shared {
    pub(super) fn new(canvas: Option<JsObject>, gpu: Option<JsObject>) -> Rc<Shared> {
        let scale_override = kompoz_runtime::environment::var("KOMPOZ_SCALE")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|s| *s > 0.0 && s.is_finite());
        let shared = Rc::new(Shared {
            canvas,
            gpu,
            page: RefCell::new(PageState::default()),
            fixed: Cell::new(None),
            drawable: Rc::new(Cell::new((1, 1))),
            scale_override,
            owner: Cell::new(None),
            text_area: RefCell::new(TextArea::default()),
            field_active: Cell::new(false),
            text_input: RefCell::new(((0.0, 0.0, 0.0, 0.0), "text enter off on".into())),
            clip: RefCell::new(None),
            in_clip_event: Cell::new(false),
            drop_entry: RefCell::new(None),
            reduced_motion: Rc::new(Cell::new(false)),
            keep_awake: Cell::new(false),
            location: RefCell::new(String::new()),
            dragging_out: Cell::new(false),
            drag_starting: Cell::new(false),
            aria: RefCell::new(
                (kompoz_runtime::environment::var("KOMPOZ_ARIA").as_deref().map(str::trim) != Some("0"))
                    .then(|| Aria { mirror: Mirror::default(), dirty: true, urgent: false, last: None }),
            ),
            input_multi_line: Cell::new(None),
        });
        shared.read_page();
        shared
    }

    /// The page read again: what changed, as the app's events.
    pub(super) fn read_page(&self) -> Vec<Event> {
        let page = PageState::parse(&sys::state());
        let before = self.page.replace(page.clone());
        self.reduced_motion.set(page.reduced_motion);
        self.drawable.set(self.pixel_size());
        page.changes(&before)
    }

    /// px per CSS pixel.
    fn scale(&self) -> f64 {
        self.scale_override.unwrap_or(self.page.borrow().ratio)
    }

    /// The canvas's size in pixels: as the browser has it in device pixels, or its CSS size times the scale.
    fn pixel_size(&self) -> (i32, i32) {
        let page = self.page.borrow();
        if self.scale_override.is_none() && page.device_width > 0 && page.device_height > 0 {
            return (page.device_width, page.device_height);
        }
        let k = self.scale_override.unwrap_or(page.ratio);
        (((page.width * k).round() as i32).max(1), ((page.height * k).round() as i32).max(1))
    }

    /// px per point (a CSS pixel).
    fn point_to_px(&self) -> f64 {
        let width = self.page.borrow().width.max(1.0);
        self.pixel_size().0 as f64 / width
    }

    /// The mirror's tree to publish at the next frame (the field typed into changed: the text area is to be linked
    /// to it before what is typed there, a screen reader's typing too).
    fn aria_changed(&self) {
        if let Some(a) = self.aria.borrow_mut().as_mut() {
            a.dirty = true;
            a.urgent = true;
        }
    }

    /// After a frame of the canvas's window (`redraw`: it drew one, at `frame_time`): its semantics tree mirrored
    /// as it changed (as the AT-SPI bridge publishes it: at most every 50 ms, 500 ms while the app is busy; at once
    /// when the field typed into changed).
    fn publish_aria(&self, host: &KompozHost, redraw: bool, frame_time: i64) {
        let mut aria = self.aria.borrow_mut();
        let Some(a) = aria.as_mut() else { return };
        a.dirty |= redraw || host.take_semantics_changed();
        let interval = if host.has_pending_work() { 500_000_000 } else { 50_000_000 };
        if !a.dirty || (!a.urgent && a.last.is_some_and(|t| frame_time - t <= interval)) {
            return;
        }
        a.last = Some(frame_time);
        a.dirty = false;
        a.urgent = false;
        let k = (1.0 / self.point_to_px().max(1e-3)) as f32;
        let window = {
            let page = self.page.borrow();
            A11yWindow {
                x: 0.0,
                y: 0.0,
                width: page.width as f32,
                height: page.height as f32,
                px_to_points: k,
                focused: page.focused,
                title: String::new(),
            }
        };
        let tree = Snapshot::new(host.accessibility_tree(), window);
        let ops = a.mirror.update(tree, k, self.input_multi_line.get());
        if !ops.is_empty() {
            sys::aria(&aria::encode(&ops));
        }
    }

    /// Whether the mirror has a tree still to publish.
    fn aria_dirty(&self) -> bool {
        self.aria.borrow().as_ref().is_some_and(|a| a.dirty)
    }
}

/// The web's window: the page's canvas, for the window that draws into it.
pub struct WebWindow {
    shared: Rc<Shared>,
    id: WindowId,
}

impl WebWindow {
    pub(super) fn new(shared: &Rc<Shared>, id: WindowId) -> WebWindow {
        WebWindow { shared: shared.clone(), id }
    }

    /// Whether this window draws into the canvas.
    fn owns_canvas(&self) -> bool {
        self.shared.owner.get() == Some(self.id)
    }
}

/// The callbacks the window gives its host.
struct WebCallbacks {
    shared: Rc<Shared>,
}

impl PlatformCallbacks for WebCallbacks {
    /// A turn of the loop at the next animation frame (from any thread: the web has one).
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        Some(Arc::new(sys::request_frame))
    }

    /// The text area over the field (its area from px to CSS pixels), its keyboard for the field's options, the
    /// field's text and selection in it; or let go of (the on-screen keyboard hidden).
    fn text_input(&self, active: bool, area: Rect, state: &TextEditState, options: &ImeOptions) {
        let k = 1.0 / self.shared.point_to_px().max(1e-3);
        let css = (area.left as f64 * k, area.top as f64 * k, area.width() as f64 * k, area.height() as f64 * k);
        self.shared.field_active.set(active);
        let contents = if active {
            self.shared.text_area.borrow_mut().field(state)
        } else {
            self.shared.text_area.borrow_mut().clear()
        };
        if let Some(c) = contents {
            sys::text_set(&c.text, c.start, c.end);
        }
        // (A field that doesn't show the on-screen keyboard still takes the typing: its text area focused with
        // `inputmode=none`.)
        let kind = keyboard_kind(options);
        let kind = if options.show_keyboard { kind } else { format!("none{}", &kind[kind.find(' ').unwrap_or(0)..]) };
        sys::text_input(active, css, &kind);
        *self.shared.text_input.borrow_mut() = (css, kind);
        // (A field taking the typing, or none now — another field's typing begins with this one's closed first —:
        // the mirror links the text area to the focused one at once.)
        let multi_line = active.then_some(!options.single_line);
        if self.shared.input_multi_line.replace(multi_line) != multi_line {
            self.shared.aria_changed();
        }
    }

    fn text_input_changed(&self, state: &TextEditState) {
        if !self.shared.field_active.get() {
            return;
        }
        if let Some(c) = self.shared.text_area.borrow_mut().field(state) {
            sys::text_set(&c.text, c.start, c.end);
        }
    }

    fn cursor(&self, icon: PointerIcon) {
        sys::set_cursor(cursor_name(icon));
    }

    fn get_clip_entry(&self) -> Option<ClipEntry> {
        self.shared.clip.borrow().clone()
    }

    /// Kept (what a paste of the app's own gives back), and offered to the page: into the copy or cut event being
    /// handled, else through the Clipboard API.
    fn set_clip_entry(&self, entry: Option<ClipEntry>) {
        let entry = entry.map(|e| e.materialized());
        clipboard::offer(entry.as_ref().unwrap_or(&ClipEntry::default()), self.shared.in_clip_event.get());
        *self.shared.clip.borrow_mut() = entry;
    }

    /// The address's fragment (C++'s), not a new history entry.
    fn location_changed(&self, path: &str) {
        *self.shared.location.borrow_mut() = path.to_string();
        sys::location_set(path);
    }

    fn open_uri(&self, uri: &str) {
        sys::open_uri(uri);
    }

    /// The Vibration API (C++'s 25 ms for a long press, a confirmation or a refusal; 8 for the light ones).
    fn haptic_feedback(&self, kind: HapticFeedbackType) {
        let strong =
            matches!(kind, HapticFeedbackType::LongPress | HapticFeedbackType::Confirm | HapticFeedbackType::Reject);
        sys::vibrate(if strong { 25 } else { 8 });
    }

    /// The on-screen keyboard hidden, the field still taking the typing (`inputmode=none`).
    fn hide_software_keyboard(&self) {
        let (area, kind) = self.shared.text_input.borrow().clone();
        let kind = format!("none{}", &kind[kind.find(' ').unwrap_or(0)..]);
        sys::text_input(self.shared.field_active.get(), area, &kind);
    }

    /// The page's own drag and drop, where the browser lets one begin: a drag the browser begins of a press (its
    /// `dragstart`, after the press moved a few pixels) while the app's drag within it is under way (one begun by a
    /// long press, before the press moved), handed over then (mod.rs's `drag_start`); not one that leaves the canvas
    /// later (a page can't begin a drag but in a `dragstart`). What it carries offered to the page as the
    /// clipboard's types are.
    fn start_drag_and_drop(&self, drag: &PlatformDragAndDrop) -> bool {
        if self.shared.dragging_out.get() || !self.shared.drag_starting.get() {
            return false;
        }
        let actions = drag.transfer_data.supported_actions.iter().fold(0, |bits, a| {
            bits | match *a {
                kompoz_ui::draganddrop::DragAndDropTransferAction::Copy => 1,
                kompoz_ui::draganddrop::DragAndDropTransferAction::Move => 2,
                _ => 4,
            }
        });
        sys::clip_begin();
        for (mime, bytes) in clipboard::offered(&drag.transfer_data.clip_entry) {
            sys::clip_add(&mime, &bytes);
        }
        let taken = sys::drag_out(if actions == 0 { 1 } else { actions });
        self.shared.dragging_out.set(taken);
        taken
    }
}

impl Platform for WebWindow {
    /// The page's query string's variables (`?KOMPOZ_SCRIPT=…`: kompoz-web's environment).
    fn option(&self, name: &str) -> Option<String> {
        kompoz_runtime::environment::var(name)
    }

    fn log(&self, message: &str) {
        kompoz_web::log(kompoz_web::Level::Info, message);
    }

    fn now(&self) -> i64 {
        kompoz_web::now_nanos()
    }

    fn backends(&self) -> Vec<Backend> {
        vec![Backend::WebGpu]
    }

    /// The canvas, for the first window that opens (the page has one): WebGPU's surface on it.
    fn open_window(&mut self, window: &WindowSpec, backend: Backend) -> Result<Surface, String> {
        if backend != Backend::WebGpu {
            return Err(format!("the web has no {} backend", backend.name()));
        }
        if self.shared.owner.get().is_some_and(|o| o != self.id) {
            return Err("the page's canvas is another window's".into());
        }
        let canvas = self.shared.canvas.as_ref().ok_or("the page has no canvas")?;
        let gpu = self.shared.gpu.as_ref().ok_or(
            "the page has no WebGPU device (the browser has no WebGPU, or its adapter refused: see the console)",
        )?;
        self.shared.owner.set(Some(self.id));
        sys::set_title(&window.title);
        let drawable = self.shared.drawable.clone();
        Ok(Surface {
            webgpu: Some(WebGpuSurface { device: gpu.raw(), canvas: Some(canvas.raw()) }),
            drawable_size: Some(Box::new(move || drawable.get())),
            transparent: window.transparent,
            ..Surface::default()
        })
    }

    fn close_window(&mut self) {
        if self.owns_canvas() {
            self.shared.owner.set(None);
        }
    }

    /// The browser's alert (C++'s).
    fn show_error(&mut self, message: &str) {
        kompoz_web::log(kompoz_web::Level::Error, message);
        sys::alert(message);
    }

    fn metrics(&self) -> Metrics {
        let page = self.shared.page.borrow().clone();
        let (pixel_width, pixel_height) = self.shared.pixel_size();
        let [l, t, r, b] = page.safe;
        Metrics {
            pixel_width,
            pixel_height,
            point_width: page.width.round() as i32,
            point_height: page.height.round() as i32,
            scale: self.shared.scale() as f32,
            point_to_px: self.shared.point_to_px() as f32,
            font_scale: self
                .option("KOMPOZ_FONT_SCALE")
                .and_then(|v| v.trim().parse::<f32>().ok())
                .map(|s| s.max(0.5))
                .unwrap_or(1.0),
            refresh_rate: page.refresh as f32,
            safe_area: Some(Rect::new(
                l as f32,
                t as f32,
                (page.width - r).max(l) as f32,
                (page.height - b).max(t) as f32,
            )),
            dark_theme: page.dark,
            language: preferred_language().unwrap_or_default(),
            title_bar: None,
        }
    }

    fn pixel_size(&self) -> (i32, i32) {
        self.shared.pixel_size()
    }

    /// The canvas made that big on the page (CSS pixels), no longer following the page's size.
    fn set_window_size(&mut self, width: i32, height: i32) {
        if !self.owns_canvas() {
            return;
        }
        self.shared.fixed.set(Some((width as f64, height as f64)));
        sys::set_canvas_size(width as f64, height as f64);
    }

    /// The page's pointer as its media queries tell it (a fine one: a mouse or a touchpad, and a keyboard with it,
    /// as C++'s); a microphone and a camera as the page's devices list them.
    fn media(&mut self, probe: bool) -> MediaInfo {
        let _ = probe; // (the page lists its devices as it starts and as they change)
        let page = self.shared.page.borrow();
        let pointer = if page.fine_pointer {
            PointerPrecision::Fine
        } else if page.coarse_pointer {
            PointerPrecision::Coarse
        } else {
            PointerPrecision::None
        };
        MediaInfo {
            pointer_precision: pointer,
            physical_keyboard: page.fine_pointer,
            has_microphone: page.microphone,
            has_camera: page.camera,
            ..MediaInfo::default()
        }
    }

    fn attach(&mut self, _: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        Rc::new(WebCallbacks { shared: self.shared.clone() })
    }

    /// The canvas's window: the state the tab kept across a reload (sessionStorage, as Android keeps an app's
    /// state when it ends it), then the address's fragment as a deep link (C++'s).
    fn started(&mut self, host: &KompozHost) {
        if self.shared.owner.get().is_some_and(|o| o != self.id) {
            return;
        }
        if let Some(state) = sys::storage_get(super::STATE_KEY).filter(|s| !s.is_empty()) {
            host.restore_state(&state);
        }
        let hash = self.shared.page.borrow().hash.clone();
        if !hash.is_empty() {
            *self.shared.location.borrow_mut() = hash.clone();
            host.open_deep_link(&hash);
        }
    }

    /// The screen kept on while a `keep_screen_on` is in the tree (the Screen Wake Lock API); the canvas's window's
    /// semantics tree mirrored for screen readers.
    fn composed(&mut self, host: &KompozHost, redraw: bool, frame_time: i64) {
        let keep = host.keep_screen_on();
        if keep != self.shared.keep_awake.get() {
            self.shared.keep_awake.set(keep);
            sys::keep_awake(keep);
        }
        if self.owns_canvas() {
            self.shared.publish_aria(host, redraw, frame_time);
        }
    }

    /// A frame more while the mirror has a tree to publish.
    fn wants_frames(&self) -> bool {
        self.owns_canvas() && self.shared.aria_dirty()
    }

    fn take_drop_entry(&mut self) -> Option<ClipEntry> {
        self.shared.drop_entry.borrow_mut().take()
    }

    fn set_title(&mut self, title: &str) {
        if self.owns_canvas() {
            sys::set_title(title);
        }
    }

    /// Full screen through the Fullscreen API (the page's whole screen); maximized is what the page already is.
    fn set_placement(&mut self, placement: WindowPlacement) {
        if self.owns_canvas() {
            sys::fullscreen(placement == WindowPlacement::Fullscreen);
        }
    }

    fn window_placement(&self) -> Option<WindowPlacement> {
        let page = self.shared.page.borrow();
        Some(if page.fullscreen {
            WindowPlacement::Fullscreen
        } else if self.shared.fixed.get().is_none() {
            WindowPlacement::Maximized
        } else {
            WindowPlacement::Floating
        })
    }

    fn screen_size(&self) -> Option<(i32, i32)> {
        let (w, h) = self.shared.page.borrow().screen;
        (w > 0 && h > 0).then_some((w, h))
    }

    /// Animations end at once while the page prefers reduced motion (`prefers-reduced-motion: reduce`: Android's
    /// animations turned off).
    fn motion_duration_scale(&self) -> Option<MotionDurationScale> {
        let reduced = self.shared.reduced_motion.clone();
        Some(MotionDurationScale::from_fn(move || if reduced.get() { 0.0 } else { 1.0 }))
    }

    /// Handed to the page (a test runner's, or a download).
    fn write_file(&mut self, path: &str, bytes: &[u8]) -> bool {
        sys::file_save(path, bytes);
        true
    }
}

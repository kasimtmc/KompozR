//! The Windows platform's windows (C++'s Win32Platform's window part): what the platform shares between them
//! ([`Shared`]: the loop's wait, the window class, the platform's own hidden window, the clipboard, the cursors, the
//! events of its own), each window's state as its window procedure keeps it ([`WindowState`]: its size, DPI,
//! placement, events), and the handle each app core has on its window ([`Win32Window`], the `Platform`).
//!
//! Win32 calls a window's procedure from inside the calls that change it (`SetWindowPos`, `ShowWindow`,
//! `DestroyWindow`) and from inside Windows' own loops (resizing, moving, the system menu): the procedure touches
//! only its window's state (cells, borrowed for a moment) and the platform's queue, never a core; a modal loop's
//! frames are stepped through the loop's stepper, which skips a step while one runs.

use super::clipboard::Clipboard;
use super::event_loop::{EventLoop, Waker};
use super::input_method::{self, InputMethod};
use super::keyboard::{self, Characters, VK_CONTROL, VK_F4, VK_MENU, VK_SPACE};
use super::sys::{self, Cursor, Hwnd, Icon, Placed, WindowHandler, WindowMessage};
use super::uia_bridge::{UiaAction, UiaBridge};
use super::{Route, StepKind, now, route};
use crate::app::{Event, Metrics, MouseButton, Platform, TitleBarColors, TrayImage, WindowSpec};
use crate::system::{log, preferred_language};
use ::windows::Win32::UI::WindowsAndMessaging as wm;
use kompoz_gpu::device::{Backend, Surface};
use kompoz_graphics::{Color, Rect};
use kompoz_ui::KompozHost;
use kompoz_ui::clipboard::ClipEntry;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use kompoz_ui::platform::{ImeOptions, PlatformCallbacks, PointerIcon, TextEditState};
use kompoz_ui::semantics::{SemanticsAction, SemanticsProperties};
use kompoz_ui::window::{WindowId, WindowPlacement};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// The class of the app's windows and the platform's own (C++'s "KompozWindow").
const CLASS: &str = "KompozWindow";

/// The timer that steps the app while Windows resizes or moves a window, or shows its system menu, in a loop of its
/// own (Windows' least period: 10 ms).
const MODAL_TIMER: usize = 1;
const MODAL_TIMER_MS: u32 = 10;

/// The largest a window gets (Windows' own limit for a window's coordinates).
const MAX_SIZE: i32 = 32_767;

/// The mouse left the client area (winuser.h; the `windows` crate has it among the common controls').
const WM_MOUSELEAVE: u32 = 0x02A3;

// DWM's window attributes (dwmapi.h) and their values.
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
/// The same before Windows 10 20H1.
const DWMWA_USE_IMMERSIVE_DARK_MODE_OLD: u32 = 19;
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWCP_ROUND: u32 = 2;
const DWMWA_CAPTION_COLOR: u32 = 35;
const DWMWA_TEXT_COLOR: u32 = 36;
const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
const DWMSBT_MAINWINDOW: u32 = 2; // (Mica)

/// What the platform's windows share: the loop's wait, the platform's own window, the clipboard, the cursors, the
/// windows' states (by their cores' ids), the platform's own events, the stepper of Windows' modal loops.
pub struct Shared {
    pub(super) event_loop: EventLoop,
    /// The platform's own hidden window: it owns the clipboard and hears the system's broadcasts.
    helper: Cell<Hwnd>,
    clipboard: Clipboard,
    windows: RefCell<Vec<Rc<WindowState>>>,
    routed: RefCell<VecDeque<(Route, Event)>>,
    stepper: RefCell<Option<Rc<dyn Fn(StepKind)>>>,
    cursors: RefCell<[Option<Cursor>; 9]>,
    /// The root core's id.
    app_id: WindowId,
    /// The system's apps' theme (Settings > Personalization > Colors: "dark").
    dark: Cell<bool>,
    /// A microphone and a camera, once looked for.
    probed: Cell<Option<(bool, bool)>>,
    /// The display kept on (some window's host asks).
    kept_on: Cell<bool>,
}

impl Shared {
    /// The process made per-monitor DPI aware, the window class registered, the loop's wait made, the platform's
    /// own window (C++'s `connect`); why not where the app can't run.
    pub fn connect(app_id: WindowId) -> Result<Rc<Shared>, String> {
        sys::set_per_monitor_dpi_awareness();
        if !sys::register_class(CLASS) {
            return Err(format!("no window class ({})", sys::last_error()));
        }
        let event_loop = EventLoop::new()?;
        let shared = Rc::new(Shared {
            event_loop,
            helper: Cell::new(Hwnd::NONE),
            clipboard: Clipboard::new(),
            windows: RefCell::new(Vec::new()),
            routed: RefCell::new(VecDeque::new()),
            stepper: RefCell::new(None),
            cursors: RefCell::new([None; 9]),
            app_id,
            dark: Cell::new(dark_mode()),
            probed: Cell::new(None),
            kept_on: Cell::new(false),
        });
        let helper = sys::create_hidden_window(CLASS, Rc::new(Helper { shared: Rc::downgrade(&shared) }))
            .ok_or_else(|| format!("no window of the platform's own ({})", sys::last_error()))?;
        shared.helper.set(helper);
        Ok(shared)
    }

    /// The platform's own window gone (what the app put on the clipboard rendered first: WM_RENDERALLFORMATS), the
    /// display let sleep again, TSF deactivated.
    pub fn close(&self) {
        let helper = self.helper.replace(Hwnd::NONE);
        if !helper.is_none() {
            helper.destroy();
        }
        if self.kept_on.replace(false) {
            sys::keep_display_on(false);
        }
        if self.windows.borrow().iter().all(|w| w.hwnd.get().is_none()) {
            super::tsf::ThreadManager::shutdown();
        }
    }

    /// The field of window `id` as its core has it after handling what was typed (none: no session open): its
    /// input method answered.
    pub(super) fn answer_input_method(&self, id: WindowId, state: Option<TextEditState>) {
        let window = self.windows.borrow().iter().find(|w| w.id == id).cloned();
        if let Some(w) = window {
            w.ime.answer(state);
        }
    }

    pub(super) fn set_stepper(&self, stepper: Option<Rc<dyn Fn(StepKind)>>) {
        *self.stepper.borrow_mut() = stepper;
    }

    /// The loop stepped from inside a modal loop (its stepper skips a step while one runs).
    fn step(&self, kind: StepKind) {
        let stepper = self.stepper.borrow().clone();
        if let Some(step) = stepper {
            step(kind);
        }
    }

    pub(super) fn push(&self, route: Route, event: Event) {
        self.routed.borrow_mut().push_back((route, event));
    }

    /// The events gathered since, each for its window.
    pub(super) fn take_events(&self) -> Vec<(WindowId, Event)> {
        let routed: Vec<(Route, Event)> = self.routed.borrow_mut().drain(..).collect();
        let windows = self.windows.borrow().clone();
        route(&windows, routed, self.app_id)
    }

    pub(super) fn waker(&self) -> Waker {
        self.event_loop.waker()
    }

    /// The system's cursor for `icon` (C++'s `showCursor`: loaded once).
    fn cursor(&self, icon: PointerIcon) -> Option<Cursor> {
        let (i, id) = match icon {
            PointerIcon::Default => (0, wm::IDC_ARROW),
            PointerIcon::Hand => (1, wm::IDC_HAND),
            PointerIcon::Text | PointerIcon::Handwriting => (2, wm::IDC_IBEAM),
            PointerIcon::Crosshair => (3, wm::IDC_CROSS),
            PointerIcon::Move => (4, wm::IDC_SIZEALL),
            PointerIcon::ResizeHorizontal => (5, wm::IDC_SIZEWE),
            PointerIcon::ResizeVertical => (6, wm::IDC_SIZENS),
            PointerIcon::NotAllowed => (7, wm::IDC_NO),
            PointerIcon::Wait => (8, wm::IDC_WAIT),
        };
        let mut cursors = self.cursors.borrow_mut();
        if cursors[i].is_none() {
            cursors[i] = Cursor::system(id);
        }
        cursors[i]
    }

    /// The display kept on while any window's host asks.
    fn keep_display_on(&self) {
        let on = self.windows.borrow().iter().any(|w| w.keep_on.get());
        if on != self.kept_on.get() {
            self.kept_on.set(on);
            sys::keep_display_on(on);
        }
    }

    /// The system's theme changed (WM_SETTINGCHANGE "ImmersiveColorSet"): each window's caption with it, the cores
    /// told.
    fn theme_changed(&self) {
        let dark = dark_mode();
        if dark == self.dark.replace(dark) {
            return;
        }
        for w in self.windows.borrow().iter() {
            let hwnd = w.hwnd.get();
            if !hwnd.is_none() {
                caption(hwnd, dark);
            }
        }
        self.push(Route::All, Event::MetricsChanged);
    }
}

/// Whether the user prefers dark apps (C++'s `darkMode`: `AppsUseLightTheme` 0).
fn dark_mode() -> bool {
    sys::user_dword(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "AppsUseLightTheme") == Some(0)
}

/// The system's caption (DWM's frame, KompozR leaving it to the system) in the system's theme: dark or light;
/// Mica behind it where Windows 11 has the backdrop (22H2), else the caption and its text in the colors KompozR's own
/// title bar would have (Material 3's surface container and on-surface: the system's palette's where W6 brings one,
/// else the baseline's); round corners (Windows 11). What DWM doesn't know (Windows 10, wine) it refuses: the frame
/// stays as it is.
fn caption(hwnd: Hwnd, dark: bool) {
    let on = dark as u32;
    if !hwnd.set_dwm_attribute(DWMWA_USE_IMMERSIVE_DARK_MODE, on) {
        hwnd.set_dwm_attribute(DWMWA_USE_IMMERSIVE_DARK_MODE_OLD, on);
    }
    hwnd.set_dwm_attribute(DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND);
    if !hwnd.set_dwm_attribute(DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW) {
        let colors = TitleBarColors::of(dark, None);
        hwnd.set_dwm_attribute(DWMWA_CAPTION_COLOR, colorref(colors.surface_container));
        hwnd.set_dwm_attribute(DWMWA_TEXT_COLOR, colorref(colors.on_surface));
    }
}

/// A color as a COLORREF (0x00BBGGRR).
fn colorref(c: Color) -> u32 {
    let argb = c.to_argb();
    let (r, g, b) = ((argb >> 16) & 0xff, (argb >> 8) & 0xff, argb & 0xff);
    b << 16 | g << 8 | r
}

/// The platform's own window's messages: the clipboard's requests, the system's broadcasts.
struct Helper {
    shared: Weak<Shared>,
}

impl WindowHandler for Helper {
    fn message(&self, hwnd: Hwnd, m: &WindowMessage) -> Option<isize> {
        let shared = self.shared.upgrade()?;
        let w = m.w();
        match m.id() {
            wm::WM_RENDERFORMAT => {
                shared.clipboard.render(w as u32);
                Some(0)
            }
            wm::WM_RENDERALLFORMATS => {
                shared.clipboard.render_all(hwnd);
                Some(0)
            }
            wm::WM_DESTROYCLIPBOARD => {
                shared.clipboard.lost(hwnd);
                Some(0)
            }
            wm::WM_SETTINGCHANGE => {
                if m.setting_name().as_deref() == Some("ImmersiveColorSet") {
                    shared.theme_changed();
                }
                None
            }
            wm::WM_DEVICECHANGE => {
                const DBT_DEVNODES_CHANGED: usize = 0x0007;
                if w == DBT_DEVNODES_CHANGED {
                    shared.push(Route::All, Event::DevicesChanged);
                }
                None
            }
            wm::WM_QUERYENDSESSION => Some(1),
            wm::WM_ENDSESSION => {
                if w != 0 {
                    // (Windows ends the process once every window answered: the app's state written now.)
                    shared.step(StepKind::End);
                }
                Some(0)
            }
            _ => None,
        }
    }
}

/// A window's state as its procedure keeps it (C++'s Win32Platform's fields): what the core reads through its
/// handle, the events for its core.
pub struct WindowState {
    pub(super) id: WindowId,
    shared: Weak<Shared>,
    hwnd: Cell<Hwnd>,
    events: Rc<RefCell<VecDeque<Event>>>,
    /// Its input method: the focused field's composition and requests (IMM32's or TSF's).
    ime: Rc<InputMethod>,
    /// The client area's size, its pixels (what the swapchain is made at; kept as it was while minimized).
    size: Rc<Cell<(i32, i32)>>,
    dpi: Cell<u32>,
    refresh_rate: Cell<f32>,
    /// How it is placed, as Windows last said (full screen: the app's own).
    placement: Cell<WindowPlacement>,
    minimized: Cell<bool>,
    /// In Windows' own loop (resizing or moving it, its system menu): its frames stepped by a timer.
    modal: Cell<bool>,
    /// WM_MOUSELEAVE asked for.
    tracking_leave: Cell<bool>,
    /// The mouse's buttons held (the mouse captured while any is).
    buttons: Cell<u32>,
    characters: RefCell<Characters>,
    cursor: Cell<PointerIcon>,
    title: RefCell<String>,
    decorated: Cell<bool>,
    resizable: Cell<bool>,
    /// What full screen left: its styles and its rectangle, to go back to.
    windowed: Cell<Option<(u32, u32, sys::Rect)>>,
    icon: RefCell<Option<Icon>>,
    /// Its host asks for the display to be kept on.
    keep_on: Cell<bool>,
    /// Its UI Automation bridge while it is open (none: `KOMPOZ_UIA=0`).
    uia: RefCell<Option<UiaBridge>>,
    /// Its semantics tree to publish to UI Automation (it changed), and when it was last (the platform's clock).
    uia_dirty: Cell<bool>,
    uia_last: Cell<Option<i64>>,
}

impl WindowState {
    fn new(shared: &Rc<Shared>, id: WindowId) -> WindowState {
        let events = Rc::new(RefCell::new(VecDeque::new()));
        WindowState {
            id,
            shared: Rc::downgrade(shared),
            hwnd: Cell::new(Hwnd::NONE),
            ime: InputMethod::new(events.clone()),
            events,
            size: Rc::new(Cell::new((1, 1))),
            dpi: Cell::new(sys::system_dpi()),
            refresh_rate: Cell::new(60.0),
            placement: Cell::new(WindowPlacement::Floating),
            minimized: Cell::new(false),
            modal: Cell::new(false),
            tracking_leave: Cell::new(false),
            buttons: Cell::new(0),
            characters: RefCell::new(Characters::default()),
            cursor: Cell::new(PointerIcon::Default),
            title: RefCell::new("KompozR".into()),
            decorated: Cell::new(true),
            resizable: Cell::new(true),
            windowed: Cell::new(None),
            icon: RefCell::new(None),
            keep_on: Cell::new(false),
            uia: RefCell::new(None),
            uia_dirty: Cell::new(true),
            uia_last: Cell::new(None),
        }
    }

    fn push(&self, event: Event) {
        self.events.borrow_mut().push_back(event);
    }

    /// The events gathered since (a UI Automation client's requests among them).
    pub(super) fn take_events(&self) -> Vec<Event> {
        if self.uia.borrow().as_ref().is_some_and(UiaBridge::take_pending) {
            self.push(Event::AccessibilityRequest);
        }
        self.events.borrow_mut().drain(..).collect()
    }

    /// The window's bridge, if it has one (cloned: what it calls may come back to the window's procedure).
    fn uia(&self) -> Option<UiaBridge> {
        self.uia.borrow().clone()
    }

    /// The client area's size as it is now; whether it changed (minimized: kept as it was).
    fn client_size(&self, hwnd: Hwnd) -> bool {
        let (w, h) = hwnd.client_size();
        if w <= 0 || h <= 0 || (w, h) == self.size.get() {
            return false;
        }
        self.size.set((w, h));
        true
    }

    /// The refresh rate of the monitor the window is on, told if it changed.
    fn refresh_rate_changed(&self, hwnd: Hwnd) {
        let rate = hwnd.monitor().and_then(|m| m.refresh_rate()).unwrap_or(60.0);
        if rate != self.refresh_rate.get() {
            self.refresh_rate.set(rate);
            self.push(Event::MetricsChanged);
        }
    }

    /// The window's styles for how it is to be (decorated, resizable).
    fn styles(&self) -> (wm::WINDOW_STYLE, wm::WINDOW_EX_STYLE) {
        let mut style = if self.decorated.get() {
            wm::WS_OVERLAPPEDWINDOW
        } else {
            // (No frame: a popup that still minimizes and restores from the taskbar.)
            wm::WS_POPUP | wm::WS_SYSMENU | wm::WS_MINIMIZEBOX | wm::WS_MAXIMIZEBOX
        };
        if !self.resizable.get() {
            style &= !(wm::WS_THICKFRAME | wm::WS_MAXIMIZEBOX);
        }
        (style, wm::WINDOW_EX_STYLE(0))
    }

    fn modifiers_event_at(&self, x: i32, y: i32) -> (f32, f32, kompoz_ui::input::KeyModifiers, i64) {
        (x as f32, y as f32, keyboard::modifiers(), now())
    }

    /// The mouse (C++'s `mouse`): moves (WM_MOUSELEAVE asked for as it comes in), its buttons (the mouse captured
    /// while any is held: their release comes outside the window too).
    fn mouse(&self, hwnd: Hwnd, message: u32, w: usize, l: isize) -> isize {
        let (x, y) = point_of(l);
        let (x, y, modifiers, time) = self.modifiers_event_at(x, y);
        if message == wm::WM_MOUSEMOVE {
            if !self.tracking_leave.get() {
                self.tracking_leave.set(hwnd.track_mouse_leave());
            }
            self.push(Event::MouseMove { x, y, modifiers, time });
            return 0;
        }
        let down = matches!(message, wm::WM_LBUTTONDOWN | wm::WM_RBUTTONDOWN | wm::WM_MBUTTONDOWN | wm::WM_XBUTTONDOWN);
        let button = match message {
            wm::WM_LBUTTONDOWN | wm::WM_LBUTTONUP => MouseButton::Left,
            wm::WM_RBUTTONDOWN | wm::WM_RBUTTONUP => MouseButton::Right,
            wm::WM_MBUTTONDOWN | wm::WM_MBUTTONUP => MouseButton::Middle,
            // (XBUTTON1 in the high word: back; XBUTTON2: forward)
            _ if (w >> 16) & 0xffff == 1 => MouseButton::Back,
            _ => MouseButton::Forward,
        };
        let held = if down { self.buttons.get() + 1 } else { self.buttons.get().saturating_sub(1) };
        self.buttons.set(held);
        if down && held == 1 {
            hwnd.capture();
        }
        if !down && held == 0 {
            sys::release_capture();
        }
        self.push(if down {
            Event::MouseDown { x, y, button, modifiers, time }
        } else {
            Event::MouseUp { x, y, button, modifiers, time }
        });
        // (X buttons answer TRUE: Windows then sends no WM_APPCOMMAND of them.)
        if matches!(message, wm::WM_XBUTTONDOWN | wm::WM_XBUTTONUP) { 1 } else { 0 }
    }

    /// A key message (C++'s keys, completed): its key (scan code and extended bit: left and right, the keypad's
    /// Enter), the character it types, the modifiers; the left Ctrl Windows makes up for AltGr left out. Alt+F4
    /// and Alt+Space are Windows' (closing, the window's menu); the other Alt keys and F10 aren't (they would open
    /// a menu the window hasn't): answered.
    fn key(&self, hwnd: Hwnd, message: u32, w: usize, l: isize) -> Option<isize> {
        let vk = w as u16;
        let (scan, extended, _) = keyboard::key_parts(l);
        let down = matches!(message, wm::WM_KEYDOWN | wm::WM_SYSKEYDOWN);
        let system = matches!(message, wm::WM_SYSKEYDOWN | wm::WM_SYSKEYUP);
        let answer = if message == wm::WM_SYSKEYDOWN && (vk == VK_F4 || vk == VK_SPACE) { None } else { Some(0) };
        let answer = if system { answer } else { None };
        // (An input method's key: its own, VK_PROCESSKEY.)
        if down {
            self.ime.key(vk);
        }
        if vk == input_method::VK_PROCESSKEY {
            return answer;
        }
        // (AltGr: the left Ctrl Windows sends first, at the right Alt's time.)
        if vk == VK_CONTROL && !extended {
            let time = sys::message_time();
            let next = sys::peek_next(hwnd, wm::WM_KEYFIRST, wm::WM_KEYLAST);
            if let Some((m, nw, nl, t)) = next
                && t == time
                && nw as u16 == VK_MENU
                && keyboard::key_parts(nl).1
                && (matches!(m, wm::WM_KEYDOWN | wm::WM_SYSKEYDOWN) == down)
            {
                return answer;
            }
        }
        let key = keyboard::key_of(vk, scan, extended, || keyboard::unshifted_character(vk, scan));
        let code_point = keyboard::code_point(vk, scan);
        let (modifiers, time) = (keyboard::modifiers(), now());
        self.push(if down {
            Event::KeyDown { key, code_point, modifiers, time }
        } else {
            Event::KeyUp { key, code_point, modifiers, time }
        });
        answer
    }

    /// Full screen (the monitor's whole rectangle, no frame) or back to how it was.
    fn set_fullscreen(&self, hwnd: Hwnd, on: bool) {
        let flags = wm::SWP_NOZORDER | wm::SWP_NOACTIVATE | wm::SWP_FRAMECHANGED;
        if on {
            if self.windowed.get().is_some() {
                return;
            }
            if hwnd.is_maximized() {
                hwnd.show(wm::SW_RESTORE);
            }
            let (style, ex_style) = (hwnd.style(), hwnd.ex_style());
            self.windowed.set(Some((style.0, ex_style.0, hwnd.window_rect())));
            let Some(monitor) = hwnd.monitor() else { return };
            let r = monitor.rect;
            hwnd.set_styles((style & !(wm::WS_OVERLAPPEDWINDOW | wm::WS_POPUP)) | wm::WS_POPUP, ex_style);
            hwnd.set_pos(None, r.left, r.top, r.width(), r.height(), flags);
            self.placement.set(WindowPlacement::Fullscreen);
        } else if let Some((style, ex_style, r)) = self.windowed.take() {
            hwnd.set_styles(wm::WINDOW_STYLE(style), wm::WINDOW_EX_STYLE(ex_style));
            hwnd.set_pos(None, r.left, r.top, r.width(), r.height(), flags);
            self.placement.set(WindowPlacement::Floating);
        }
        self.push(Event::MetricsChanged);
    }
}

/// Code points `start` and `end` of field `id`'s text as typed (what UI Automation's ranges are in) as its bytes
/// (what its SetSelection action takes).
fn typed_bytes(host: &KompozHost, id: u64, start: usize, end: usize) -> (i32, i32) {
    let typed = host
        .semantics_owner()
        .find(id, true)
        .and_then(|n| {
            let config = n.config();
            config.get(SemanticsProperties::InputText).or_else(|| config.get(SemanticsProperties::EditableText))
        })
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    let byte = |at: usize| typed.char_indices().nth(at).map_or(typed.len(), |(i, _)| i) as i32;
    (byte(start), byte(end))
}

/// A mouse message's position (signed: outside the window while captured).
fn point_of(l: isize) -> (i32, i32) {
    ((l & 0xffff) as u16 as i16 as i32, ((l >> 16) & 0xffff) as u16 as i16 as i32)
}

impl WindowHandler for WindowState {
    /// A window message (C++'s `message`): its answer, or none for Windows' own handling.
    fn message(&self, hwnd: Hwnd, m: &WindowMessage) -> Option<isize> {
        let shared = self.shared.upgrade()?;
        let (message, w, l) = (m.id(), m.w(), m.l());
        if (wm::WM_IME_SETCONTEXT..=wm::WM_IME_KEYUP).contains(&message)
            || matches!(message, wm::WM_IME_STARTCOMPOSITION | wm::WM_IME_ENDCOMPOSITION | wm::WM_IME_COMPOSITION)
        {
            if message == wm::WM_IME_CHAR {
                // (An input method's character: left out where its result was committed already.)
                if !self.ime.typed_by_result(w as u16)
                    && let Some(text) = self.characters.borrow_mut().unit(w as u16)
                {
                    self.push(Event::Text(text));
                }
                return Some(0);
            }
            if let Some(answer) = self.ime.message(hwnd, m) {
                return Some(answer);
            }
            return None;
        }
        match message {
            wm::WM_CLOSE => {
                self.push(Event::CloseRequest);
                Some(0)
            }
            // UI Automation asks for the window's root (the first time: its tree is to be published).
            wm::WM_GETOBJECT => {
                let (answer, first) = self.uia()?.answer(hwnd, w, l)?;
                if first {
                    self.push(Event::AccessibilityRequest);
                }
                Some(answer)
            }
            wm::WM_SIZE => {
                const SIZE_RESTORED: usize = 0;
                const SIZE_MINIMIZED: usize = 1;
                const SIZE_MAXIMIZED: usize = 2;
                match w {
                    SIZE_MINIMIZED => {
                        if !self.minimized.replace(true) {
                            self.push(Event::Minimized);
                        }
                        return Some(0);
                    }
                    SIZE_MAXIMIZED | SIZE_RESTORED => {
                        let was_minimized = self.minimized.replace(false);
                        let placement = if w == SIZE_MAXIMIZED {
                            WindowPlacement::Maximized
                        } else if self.windowed.get().is_some() {
                            WindowPlacement::Fullscreen
                        } else {
                            WindowPlacement::Floating
                        };
                        let before = self.placement.replace(placement);
                        if placement == WindowPlacement::Maximized && before != placement {
                            self.push(Event::Maximized);
                        } else if was_minimized || (before == WindowPlacement::Maximized && placement != before) {
                            self.push(Event::Restored);
                        }
                    }
                    _ => {}
                }
                if self.client_size(hwnd) {
                    self.push(Event::Resized);
                }
                if self.modal.get() {
                    shared.step(StepKind::Frame); // (drawn as it is resized)
                }
                Some(0)
            }
            wm::WM_ENTERSIZEMOVE | wm::WM_ENTERMENULOOP => {
                self.modal.set(true);
                hwnd.set_timer(MODAL_TIMER, MODAL_TIMER_MS);
                Some(0)
            }
            wm::WM_EXITSIZEMOVE | wm::WM_EXITMENULOOP => {
                self.modal.set(false);
                hwnd.kill_timer(MODAL_TIMER);
                Some(0)
            }
            wm::WM_TIMER if w == MODAL_TIMER => {
                shared.step(StepKind::Frame);
                Some(0)
            }
            wm::WM_PAINT => {
                hwnd.validate(); // (KompozR draws it)
                self.push(Event::Exposed);
                Some(0)
            }
            wm::WM_ERASEBKGND => Some(1),
            wm::WM_GETMINMAXINFO => {
                // (Its size the app's to choose, larger than the screen too, as Compose Desktop's windows can be: a
                // test's taller window on a small CI screen.)
                m.set_max_track_size(MAX_SIZE, MAX_SIZE);
                Some(0)
            }
            wm::WM_DPICHANGED => {
                self.dpi.set(((w >> 16) & 0xffff) as u32);
                // (The rectangle Windows suggests for the new DPI: the same size in dp.)
                let Some(r) = m.suggested_rect() else { return Some(0) };
                hwnd.set_pos(None, r.left, r.top, r.width(), r.height(), wm::SWP_NOZORDER | wm::SWP_NOACTIVATE);
                self.push(Event::MetricsChanged);
                Some(0)
            }
            wm::WM_WINDOWPOSCHANGED | wm::WM_DISPLAYCHANGE => {
                self.refresh_rate_changed(hwnd);
                None
            }
            wm::WM_SETFOCUS | wm::WM_KILLFOCUS => {
                self.push(if message == wm::WM_SETFOCUS { Event::FocusGained } else { Event::FocusLost });
                None
            }
            wm::WM_SETCURSOR => {
                const HTCLIENT: isize = 1;
                if l & 0xffff != HTCLIENT {
                    return None;
                }
                if let Some(c) = shared.cursor(self.cursor.get()) {
                    c.show();
                }
                Some(1)
            }
            wm::WM_MOUSEMOVE
            | wm::WM_LBUTTONDOWN
            | wm::WM_LBUTTONUP
            | wm::WM_RBUTTONDOWN
            | wm::WM_RBUTTONUP
            | wm::WM_MBUTTONDOWN
            | wm::WM_MBUTTONUP
            | wm::WM_XBUTTONDOWN
            | wm::WM_XBUTTONUP => Some(self.mouse(hwnd, message, w, l)),
            WM_MOUSELEAVE => {
                self.tracking_leave.set(false);
                self.push(Event::PointerLeft);
                Some(0)
            }
            wm::WM_CAPTURECHANGED => {
                // (Another window took the mouse: no button is held for this one any more.)
                self.buttons.set(0);
                None
            }
            wm::WM_MOUSEWHEEL | wm::WM_MOUSEHWHEEL => {
                // (A notch is 120, a precision touchpad's less; up and right positive. The position is the
                // screen's.)
                let (sx, sy) = point_of(l);
                let (x, y) = hwnd.screen_to_client(sx, sy);
                let notches = ((w >> 16) & 0xffff) as u16 as i16 as f32 / 120.0;
                let (dx, dy) = if message == wm::WM_MOUSEWHEEL { (0.0, notches) } else { (notches, 0.0) };
                let (x, y, modifiers, time) = self.modifiers_event_at(x, y);
                self.push(Event::Wheel { x, y, dx, dy, modifiers, time });
                Some(0)
            }
            wm::WM_KEYDOWN | wm::WM_SYSKEYDOWN | wm::WM_KEYUP | wm::WM_SYSKEYUP => self.key(hwnd, message, w, l),
            // (Alt+key's characters: Windows' menu mnemonics, which the window has none of; Alt+Space its menu.)
            wm::WM_SYSCHAR => {
                if w as u16 == VK_SPACE {
                    None
                } else {
                    Some(0)
                }
            }
            // A character typed (an input method's too, a dead key's with the next), in UTF-16 halves.
            wm::WM_CHAR => {
                if self.ime.typed_by_result(w as u16) {
                    return Some(0);
                }
                if let Some(text) = self.characters.borrow_mut().unit(w as u16) {
                    self.push(Event::Text(text));
                }
                Some(0)
            }
            _ => None,
        }
    }
}

/// A window of the app's (the root core's, or a `Window`'s): its state, the platform it shares.
pub struct Win32Window {
    shared: Rc<Shared>,
    state: Rc<WindowState>,
}

impl Win32Window {
    /// A handle on window `id` (opened by the core: `open_window`).
    pub fn new(shared: &Rc<Shared>, id: WindowId) -> Win32Window {
        let state = Rc::new(WindowState::new(shared, id));
        shared.windows.borrow_mut().push(state.clone());
        Win32Window { shared: shared.clone(), state }
    }

    fn hwnd(&self) -> Hwnd {
        self.state.hwnd.get()
    }

    /// Its window's handle (tests).
    #[cfg(test)]
    pub(super) fn handle(&self) -> Hwnd {
        self.state.hwnd.get()
    }

    /// Its input method (tests).
    #[cfg(test)]
    pub(super) fn input_method(&self) -> Rc<InputMethod> {
        self.state.ime.clone()
    }

    /// px per dp: `KOMPOZ_SCALE`, else the window's DPI / 96.
    fn scale(&self) -> f64 {
        self.option("KOMPOZ_SCALE")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|&s| s > 0.0)
            .unwrap_or(self.state.dpi.get() as f64 / 96.0)
    }
}

impl Drop for Win32Window {
    fn drop(&mut self) {
        self.close_window();
        self.shared.windows.borrow_mut().retain(|w| !Rc::ptr_eq(w, &self.state));
        self.shared.keep_display_on();
    }
}

/// The callbacks the Windows platform gives a window's host (C++'s `attach`): waking the loop, the cursor, the
/// clipboard, links.
struct Win32Callbacks {
    shared: Rc<Shared>,
    state: Rc<WindowState>,
    waker: Waker,
}

impl PlatformCallbacks for Win32Callbacks {
    fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        Some(self.waker.as_fn())
    }

    /// The window's input method given the focused field (IMM32's context, TSF's document), or taken from it.
    fn text_input(&self, active: bool, area: Rect, state: &TextEditState, options: &ImeOptions) {
        self.state.ime.text_input(active, area, state, options);
    }

    fn text_input_changed(&self, state: &TextEditState) {
        self.state.ime.text_input_changed(state);
    }

    /// Shown at once where the mouse is over the window, else as it comes in (WM_SETCURSOR).
    fn cursor(&self, icon: PointerIcon) {
        self.state.cursor.set(icon);
        let hwnd = self.state.hwnd.get();
        if !hwnd.is_none()
            && hwnd.under_cursor()
            && let Some(c) = self.shared.cursor(icon)
        {
            c.show();
        }
    }

    fn get_clip_entry(&self) -> Option<ClipEntry> {
        self.shared.clipboard.entry(self.shared.helper.get())
    }

    fn set_clip_entry(&self, entry: Option<ClipEntry>) {
        self.shared.clipboard.set(self.shared.helper.get(), entry);
    }

    /// Opened by the shell (C++'s `ShellExecuteW`).
    fn open_uri(&self, uri: &str) {
        if !sys::shell_open(uri) {
            log(&format!("kompoz: can't open {uri} ({})", sys::last_error()));
        }
    }
}

impl Platform for Win32Window {
    fn option(&self, name: &str) -> Option<String> {
        kompoz_runtime::environment::var(name)
    }

    fn log(&self, message: &str) {
        log(message);
    }

    fn now(&self) -> i64 {
        now()
    }

    fn backends(&self) -> Vec<Backend> {
        vec![Backend::Vulkan]
    }

    /// The window (C++'s `openWindow`): made at its place (Windows' own, or the one asked for: there first, so
    /// that its DPI is that monitor's), sized from dp at that DPI with its frame around, its caption in the system's
    /// theme, its refresh rate read; then shown as it is to be (maximized, full screen, minimized, without taking
    /// the focus).
    fn open_window(&mut self, spec: &WindowSpec, backend: Backend) -> Result<Surface, String> {
        if backend != Backend::Vulkan {
            return Err(format!("no {} on Windows", backend.name()));
        }
        let state = &self.state;
        *state.title.borrow_mut() = spec.title.clone();
        state.decorated.set(!spec.undecorated);
        state.resizable.set(spec.resizable);
        let (style, mut ex_style) = state.styles();
        if !spec.focusable {
            ex_style |= wm::WS_EX_NOACTIVATE;
        }
        if spec.always_on_top {
            ex_style |= wm::WS_EX_TOPMOST;
        }
        let system_scale = self
            .option("KOMPOZ_SCALE")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|&s| s > 0.0)
            .unwrap_or(sys::system_dpi() as f64 / 96.0);
        let at = match spec.position {
            Some((x, y)) => Placed {
                x: (x.0 as f64 * system_scale).round() as i32,
                y: (y.0 as f64 * system_scale).round() as i32,
                width: 100,
                height: 100,
            },
            None => Placed { x: wm::CW_USEDEFAULT, y: wm::CW_USEDEFAULT, width: 100, height: 100 },
        };
        let hwnd = sys::create_window(super::window::CLASS, &spec.title, style, ex_style, at, state.clone())
            .ok_or_else(|| format!("the window couldn't be made ({})", sys::last_error()))?;
        state.hwnd.set(hwnd);
        state.ime.attach(hwnd, input_method::mode());
        if UiaBridge::enabled() {
            *state.uia.borrow_mut() = Some(UiaBridge::new(hwnd, self.shared.waker().as_fn()));
            state.uia_dirty.set(true);
            state.uia_last.set(None);
        }
        state.dpi.set(hwnd.dpi());
        let scale = self.scale();
        let width = ((spec.width.0 as f64 * scale).round() as i32).max(1);
        let height = ((spec.height.0 as f64 * scale).round() as i32).max(1);
        let (fw, fh) = hwnd.frame_size(width, height, state.dpi.get());
        hwnd.set_pos(None, 0, 0, fw, fh, wm::SWP_NOMOVE | wm::SWP_NOZORDER | wm::SWP_NOACTIVATE);
        state.client_size(hwnd);
        caption(hwnd, self.shared.dark.get());
        state.refresh_rate.set(hwnd.monitor().and_then(|m| m.refresh_rate()).unwrap_or(60.0));
        if let Some(icon) = state.icon.borrow().as_ref() {
            hwnd.set_icon(Some(icon));
        }
        let show = match (spec.minimized, spec.placement) {
            (true, _) => wm::SW_SHOWMINNOACTIVE,
            (false, WindowPlacement::Maximized) => wm::SW_SHOWMAXIMIZED,
            _ if !spec.focusable => wm::SW_SHOWNOACTIVATE,
            _ => wm::SW_SHOWNORMAL,
        };
        hwnd.show(show);
        if spec.placement == WindowPlacement::Fullscreen {
            state.set_fullscreen(hwnd, true);
        }
        state.push(Event::Shown);
        let size = state.size.clone();
        Ok(Surface {
            vulkan: Some(super::vulkan::surface(hwnd)),
            drawable_size: Some(Box::new(move || size.get())),
            pipeline_cache: None,
            // (Opaque for now, a transparent window's too: Windows' Vulkan drivers present win32 surfaces opaque
            // only, so what is behind shows through DWM's composition of the window, which W7 builds with the frames.)
            transparent: false,
        })
    }

    fn close_window(&mut self) {
        let hwnd = self.state.hwnd.replace(Hwnd::NONE);
        if hwnd.is_none() {
            return;
        }
        if self.state.buttons.replace(0) > 0 {
            sys::release_capture();
        }
        self.state.modal.set(false);
        self.state.tracking_leave.set(false);
        self.state.windowed.set(None);
        self.state.ime.detach();
        let bridge = self.state.uia.borrow_mut().take();
        if let Some(bridge) = bridge {
            bridge.close();
        }
        hwnd.destroy();
    }

    /// A message box (C++'s `showError`), over the window where it has one.
    fn show_error(&mut self, message: &str) {
        let owner = Some(self.hwnd()).filter(|h| !h.is_none());
        sys::message_box(owner, &self.state.title.borrow(), message);
    }

    /// Points are the window's pixels; the scale its monitor's DPI / 96; `KOMPOZ_SCALE` and `KOMPOZ_FONT_SCALE`
    /// stand in for them (the system's text size is the desktop services' package's, W6).
    fn metrics(&self) -> Metrics {
        let (w, h) = self.state.size.get();
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
                .unwrap_or(1.0),
            refresh_rate: self.state.refresh_rate.get(),
            safe_area: Some(Rect::new(0.0, 0.0, w as f32, h as f32)),
            dark_theme: self.shared.dark.get(),
            language: preferred_language().unwrap_or_default(),
            // (The system's frame: DWM draws the caption. KompozR's own is `KOMPOZ_DECORATIONS=kompoz`'s, W7.)
            title_bar: None,
        }
    }

    fn pixel_size(&self) -> (i32, i32) {
        self.state.size.get()
    }

    /// The client area resized to `width` × `height` pixels (its frame around it at its DPI).
    fn set_window_size(&mut self, width: i32, height: i32) {
        let hwnd = self.hwnd();
        if hwnd.is_none() {
            return;
        }
        let (fw, fh) = hwnd.frame_size(width.max(1), height.max(1), self.state.dpi.get());
        hwnd.set_pos(None, 0, 0, fw, fh, wm::SWP_NOMOVE | wm::SWP_NOZORDER | wm::SWP_NOACTIVATE);
    }

    /// A mouse is a fine pointer, a touch screen alone a coarse one; a physical keyboard as the raw input devices
    /// say; a microphone (waveIn) and a camera (its device interface) as probed (C++'s `media`).
    fn media(&mut self, probe: bool) -> MediaInfo {
        if probe && self.shared.probed.get().is_none() {
            self.shared.probed.set(Some((sys::has_microphone(), sys::has_camera())));
        }
        let (microphone, camera) = self.shared.probed.get().unwrap_or((false, false));
        let pointer = if sys::has_mouse() {
            PointerPrecision::Fine
        } else if sys::has_touch_screen() {
            PointerPrecision::Coarse
        } else {
            PointerPrecision::None
        };
        MediaInfo {
            pointer_precision: pointer,
            physical_keyboard: sys::has_keyboard(),
            has_microphone: microphone,
            has_camera: camera,
            ..MediaInfo::default()
        }
    }

    fn attach(&mut self, _: &KompozHost) -> Rc<dyn PlatformCallbacks> {
        Rc::new(Win32Callbacks { shared: self.shared.clone(), state: self.state.clone(), waker: self.shared.waker() })
    }

    /// What UI Automation clients asked of the window's nodes performed (C++'s `performSemanticsAction`s); its tree
    /// to publish again as the window's focus changes.
    fn handled(&mut self, host: &KompozHost, event: &Event) {
        match event {
            Event::AccessibilityRequest => {
                let Some(bridge) = self.state.uia() else { return };
                for r in bridge.take_requests() {
                    let action = match r.action {
                        UiaAction::Do(action) => action,
                        UiaAction::Select { start, end } => {
                            let (start, end) = typed_bytes(host, r.id, start, end);
                            SemanticsAction::SetSelection { start, end, relative_to_original_text: true }
                        }
                    };
                    host.perform_semantics_action(r.id, action);
                }
            }
            Event::FocusGained | Event::FocusLost => self.state.uia_dirty.set(true),
            _ => {}
        }
    }

    /// The display kept on while the host asks (any window's: `SetThreadExecutionState`); the window's semantics
    /// tree published to UI Automation as its clients ask for it (at once when they do, else at most every 50 ms,
    /// 500 ms while the app is busy: Linux's AT-SPI bridge's pace), with the focused field's caret box.
    fn composed(&mut self, host: &KompozHost, redraw: bool, frame_time: i64) {
        let on = host.keep_screen_on() && !self.hwnd().is_none();
        if self.state.keep_on.replace(on) != on {
            self.shared.keep_display_on();
        }
        let Some(bridge) = self.state.uia().filter(UiaBridge::active) else { return };
        let requested = bridge.take_tree_request();
        let state = &self.state;
        state.uia_dirty.set(state.uia_dirty.get() || redraw || requested || host.take_semantics_changed());
        let interval = if host.has_pending_work() { 500_000_000 } else { 50_000_000 };
        if state.uia_dirty.get() && (requested || state.uia_last.get().is_none_or(|t| frame_time - t > interval)) {
            state.uia_last.set(Some(frame_time));
            state.uia_dirty.set(false);
            bridge.publish(host.accessibility_tree(), state.ime.caret_box());
        }
    }

    /// One more frame publishes the tree to UI Automation (it changed, or a client wants it).
    fn wants_frames(&self) -> bool {
        self.state.uia().is_some_and(|b| b.active()) && self.state.uia_dirty.get()
    }

    fn set_title(&mut self, title: &str) {
        *self.state.title.borrow_mut() = title.to_string();
        let hwnd = self.hwnd();
        if !hwnd.is_none() {
            hwnd.set_title(title);
        }
    }

    fn set_placement(&mut self, placement: WindowPlacement) {
        let hwnd = self.hwnd();
        if hwnd.is_none() {
            return;
        }
        match placement {
            WindowPlacement::Floating => {
                self.state.set_fullscreen(hwnd, false);
                if hwnd.is_maximized() {
                    hwnd.show(wm::SW_RESTORE);
                }
            }
            WindowPlacement::Maximized => {
                self.state.set_fullscreen(hwnd, false);
                hwnd.show(wm::SW_MAXIMIZE);
            }
            WindowPlacement::Fullscreen => self.state.set_fullscreen(hwnd, true),
        }
    }

    fn set_minimized(&mut self, minimized: bool) {
        let hwnd = self.hwnd();
        if !hwnd.is_none() {
            hwnd.show(if minimized { wm::SW_MINIMIZE } else { wm::SW_RESTORE });
        }
    }

    /// Its frame's top left to `x`, `y` on the virtual screen.
    fn set_position(&mut self, x: i32, y: i32) {
        let hwnd = self.hwnd();
        if !hwnd.is_none() {
            hwnd.set_pos(None, x, y, 0, 0, wm::SWP_NOSIZE | wm::SWP_NOZORDER | wm::SWP_NOACTIVATE);
        }
    }

    /// Restored if minimized, and to the foreground (C++'s `raise`: as far as Windows lets the app — its input was
    /// last in one of its windows).
    fn to_front(&mut self) {
        let hwnd = self.hwnd();
        if hwnd.is_none() {
            return;
        }
        if hwnd.is_minimized() {
            hwnd.show(wm::SW_RESTORE);
        }
        hwnd.set_foreground();
    }

    /// Its resizing frame and maximize button there or not.
    fn set_resizable(&mut self, resizable: bool) {
        self.state.resizable.set(resizable);
        let hwnd = self.hwnd();
        if hwnd.is_none() || self.state.windowed.get().is_some() {
            return;
        }
        let (style, _) = self.state.styles();
        let keep = hwnd.style() & (wm::WS_VISIBLE | wm::WS_MINIMIZE | wm::WS_MAXIMIZE);
        hwnd.set_styles(style | keep, hwnd.ex_style());
        let flags = wm::SWP_NOMOVE | wm::SWP_NOSIZE | wm::SWP_NOZORDER | wm::SWP_NOACTIVATE | wm::SWP_FRAMECHANGED;
        hwnd.set_pos(None, 0, 0, 0, 0, flags);
    }

    fn set_always_on_top(&mut self, on_top: bool) {
        let hwnd = self.hwnd();
        if !hwnd.is_none() {
            hwnd.set_topmost(on_top, wm::SWP_NOMOVE | wm::SWP_NOSIZE | wm::SWP_NOACTIVATE);
        }
    }

    /// Its icons (the taskbar's, Alt+Tab's, the caption's) from the picture (none: the program's).
    fn set_icon(&mut self, icon: Option<&TrayImage>) {
        let made = icon.and_then(|i| Icon::from_rgba(i.width, i.height, &i.rgba));
        let hwnd = self.hwnd();
        if !hwnd.is_none() {
            hwnd.set_icon(made.as_ref());
        }
        *self.state.icon.borrow_mut() = made; // (the one before destroyed once the window shows this one)
    }

    /// Its frame's top left on the virtual screen.
    fn window_position(&self) -> Option<(i32, i32)> {
        let hwnd = self.hwnd();
        (!hwnd.is_none() && !hwnd.is_minimized()).then(|| {
            let r = hwnd.window_rect();
            (r.left, r.top)
        })
    }

    fn window_placement(&self) -> Option<WindowPlacement> {
        (!self.hwnd().is_none()).then(|| self.state.placement.get())
    }

    /// The monitor it is on (the primary one before it is open), its size.
    fn screen_size(&self) -> Option<(i32, i32)> {
        let hwnd = self.hwnd();
        let monitor = if hwnd.is_none() { sys::primary_monitor() } else { hwnd.monitor() }?;
        Some((monitor.rect.width(), monitor.rect.height()))
    }
}

/// Windows made and driven by messages sent to them as Windows sends them (on Windows, and under wine): what each
/// becomes, the sizes and the placements as Windows has them, the clipboard both ways (its formats promised and
/// rendered as another window asks).
#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_graphics::{Dp, ImageBitmap};
    use kompoz_ui::clipboard::{ClipItem, ClipValue};
    use kompoz_ui::input::{Key, KeyModifiers};

    /// A window opened as the app core opens it (no device: its surface left unused), its messages so far taken.
    fn open(shared: &Rc<Shared>, id: WindowId, title: &str) -> Win32Window {
        let mut w = Win32Window::new(shared, id);
        let mut spec = WindowSpec::main(title, Dp(200.0), Dp(150.0));
        spec.id = id;
        let surface = w.open_window(&spec, Backend::Vulkan).expect("a window");
        drop(surface);
        pump();
        w
    }

    fn pump() {
        while let Some(m) = sys::peek_message() {
            sys::dispatch(&m);
        }
    }

    fn events(shared: &Shared, id: WindowId) -> Vec<Event> {
        pump();
        shared.take_events().into_iter().filter(|(w, _)| *w == id).map(|(_, e)| e).collect()
    }

    /// A key message's lParam: its scan code, extended or not, going up or not.
    fn key_l(scan: u32, extended: bool, up: bool) -> isize {
        let mut l = 1 | (scan << 16) | ((extended as u32) << 24);
        if up {
            l |= 3 << 30;
        }
        l as i32 as isize
    }

    #[test]
    fn windows_become_events_and_sizes() {
        let shared = Shared::connect(WindowId::MAIN).expect("the platform");
        let mut w = open(&shared, WindowId::MAIN, "Pencere");
        let hwnd = w.hwnd();
        let opened = events(&shared, WindowId::MAIN);
        assert!(opened.contains(&Event::Shown), "{opened:?}");
        // Its client area at its DPI's size; metrics say it.
        let scale = w.scale();
        let (cw, ch) = hwnd.client_size();
        assert_eq!((cw, ch), ((200.0 * scale).round() as i32, (150.0 * scale).round() as i32));
        assert_eq!(w.pixel_size(), (cw, ch));
        assert_eq!(w.metrics().point_to_px, 1.0);
        // Resized by the core: Resized, the new size.
        w.set_window_size(300, 220);
        assert!(events(&shared, WindowId::MAIN).contains(&Event::Resized));
        assert_eq!(w.pixel_size(), (300, 220));
        assert_eq!(hwnd.client_size(), (300, 220));
        // The close button: a close request, the window still there.
        hwnd.send(wm::WM_CLOSE, 0, 0);
        assert_eq!(events(&shared, WindowId::MAIN), [Event::CloseRequest]);
        assert!(hwnd.is_visible());
        // The mouse: moves, a press and its release (captured meanwhile), the wheel's notches (a precision
        // touchpad's part of one), its leaving.
        let at = |x: i32, y: i32| ((y as isize) << 16) | (x as isize & 0xffff);
        hwnd.send(wm::WM_MOUSEMOVE, 0, at(10, 20));
        hwnd.send(wm::WM_LBUTTONDOWN, 1, at(10, 20));
        hwnd.send(wm::WM_LBUTTONUP, 0, at(-5, 30));
        let (sx, sy) = hwnd.client_origin();
        hwnd.send(wm::WM_MOUSEWHEEL, 60usize << 16, at(sx + 7, sy + 8));
        hwnd.send(wm::WM_MOUSEHWHEEL, (-120i16 as u16 as usize) << 16, at(sx + 7, sy + 8));
        hwnd.send(super::WM_MOUSELEAVE, 0, 0);
        let mouse = events(&shared, WindowId::MAIN);
        let kinds: Vec<String> =
            mouse.iter().map(|e| format!("{e:?}").split([' ', '{']).next().unwrap().to_string()).collect();
        // (The real mouse isn't over the window: Windows' own WM_MOUSELEAVE may come too.)
        assert_eq!(kinds[..6], ["MouseMove", "MouseDown", "MouseUp", "Wheel", "Wheel", "PointerLeft"], "{mouse:?}");
        assert!(kinds[6..].iter().all(|k| k == "PointerLeft"), "{mouse:?}");
        assert!(matches!(mouse[1], Event::MouseDown { x: 10.0, y: 20.0, button: MouseButton::Left, .. }));
        assert!(matches!(mouse[2], Event::MouseUp { x: -5.0, y: 30.0, .. })); // (outside: captured)
        assert!(matches!(mouse[3], Event::Wheel { x: 7.0, y: 8.0, dx: 0.0, dy: 0.5, .. }), "{:?}", mouse[3]);
        assert!(matches!(mouse[4], Event::Wheel { dx: -1.0, dy: 0.0, .. }));
        // Minimized and restored, maximized: as Windows says, the placement with it.
        hwnd.show(wm::SW_MINIMIZE);
        assert!(events(&shared, WindowId::MAIN).contains(&Event::Minimized));
        hwnd.show(wm::SW_RESTORE);
        assert!(events(&shared, WindowId::MAIN).contains(&Event::Restored));
        w.set_placement(WindowPlacement::Maximized);
        assert!(events(&shared, WindowId::MAIN).contains(&Event::Maximized));
        assert_eq!(w.window_placement(), Some(WindowPlacement::Maximized));
        w.set_placement(WindowPlacement::Floating);
        pump();
        assert_eq!(w.window_placement(), Some(WindowPlacement::Floating));
        // Its title, its place.
        w.set_title("Başlık");
        w.set_position(120, 90);
        assert_eq!(w.window_position(), Some((120, 90)));
        w.close_window();
        assert!(!hwnd.is_visible());
        drop(w);
        shared.close();
    }

    #[test]
    fn keys_and_characters() {
        let shared = Shared::connect(WindowId::MAIN).expect("the platform");
        let w = open(&shared, WindowId::MAIN, "Tuşlar");
        let hwnd = w.hwnd();
        events(&shared, WindowId::MAIN);
        // A, its character; the keypad's Enter (extended); the right Ctrl; F10 answered (no menu).
        hwnd.send(wm::WM_KEYDOWN, 0x41, key_l(0x1E, false, false));
        hwnd.send(wm::WM_CHAR, 'a' as usize, key_l(0x1E, false, false));
        hwnd.send(wm::WM_KEYUP, 0x41, key_l(0x1E, false, true));
        hwnd.send(wm::WM_KEYDOWN, 0x0D, key_l(0x1C, true, false));
        hwnd.send(wm::WM_KEYDOWN, 0x11, key_l(0x1D, true, false));
        assert_eq!(hwnd.send(wm::WM_SYSKEYDOWN, 0x79, key_l(0x44, false, false)), 0);
        // A character beyond the BMP in its two halves; a control character left out.
        hwnd.send(wm::WM_CHAR, 0xD83D, 0);
        hwnd.send(wm::WM_CHAR, 0xDE00, 0);
        hwnd.send(wm::WM_CHAR, 0x08, 0);
        let got = events(&shared, WindowId::MAIN);
        let keys: Vec<(bool, Key)> = got
            .iter()
            .filter_map(|e| match e {
                Event::KeyDown { key, .. } => Some((true, *key)),
                Event::KeyUp { key, .. } => Some((false, *key)),
                _ => None,
            })
            .collect();
        assert_eq!(
            keys,
            [(true, Key::A), (false, Key::A), (true, Key::NumPadEnter), (true, Key::CtrlRight), (true, Key::F10)]
        );
        let texts: Vec<&str> =
            got.iter().filter_map(|e| if let Event::Text(t) = e { Some(t.as_str()) } else { None }).collect();
        assert_eq!(texts, ["a", "😀"]);
        // (The keys held as the messages were sent: none.)
        assert!(got.iter().all(|e| e.modifiers().is_none_or(|m| m == KeyModifiers::None)));
        drop(w);
        shared.close();
    }

    #[test]
    fn several_windows_each_their_events() {
        let shared = Shared::connect(WindowId::MAIN).expect("the platform");
        let a = open(&shared, WindowId::MAIN, "Bir");
        let b = open(&shared, WindowId(7), "İki");
        shared.take_events();
        b.hwnd().send(wm::WM_CLOSE, 0, 0);
        a.hwnd().send(wm::WM_PAINT, 0, 0);
        shared.push(Route::All, Event::Wake);
        pump();
        let all = shared.take_events();
        assert!(all.contains(&(WindowId(7), Event::CloseRequest)));
        assert!(all.contains(&(WindowId::MAIN, Event::Exposed)));
        assert!(all.contains(&(WindowId::MAIN, Event::Wake)) && all.contains(&(WindowId(7), Event::Wake)));
        assert!(!all.contains(&(WindowId::MAIN, Event::CloseRequest)));
        drop(b);
        assert_eq!(shared.windows.borrow().len(), 1);
        drop(a);
        shared.close();
    }

    #[test]
    fn the_clipboard_both_ways() {
        let shared = Shared::connect(WindowId::MAIN).expect("the platform");
        let owner = shared.helper.get();
        let image = ImageBitmap::from_pixels(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 128]).unwrap();
        let entry = ClipEntry::new(vec![
            ClipItem::with_html_text("kalın yazı", "<b>kalın</b> yazı"),
            ClipItem::with_image(image),
            ClipItem::of(vec![(
                "application/x-kompoz-meyve".into(),
                ClipValue::Bytes(std::rc::Rc::from(&[1u8, 2, 3][..])),
            )]),
        ]);
        shared.clipboard.set(owner, Some(entry.clone()));
        // The app's own, as it put it there.
        let own = shared.clipboard.entry(owner).expect("the app's entry");
        assert!(own.read_plain_text().is_some());
        // As another app reads it: each format rendered by the owner as it is asked for.
        let other = super::super::clipboard::clip_entry(shared.clipboard.formats, Hwnd::NONE).expect("an entry");
        let types = other.clip_metadata().mime_types().to_vec();
        for t in ["text/plain", "text/html", "image/png", "application/x-kompoz-meyve"] {
            assert!(types.iter().any(|x| x == t), "{t} in {types:?}");
        }
        assert_eq!(other.read_plain_text().as_deref(), Some("kalın yazı"));
        assert_eq!(other.items()[0].html_text().as_deref(), Some("<b>kalın</b> yazı"));
        let png = other.items()[0].bytes("image/png").expect("the image");
        let back = kompoz_graphics::decode_to_image_bitmap(&png).unwrap();
        assert_eq!(back.pixels().as_slice(), [255, 0, 0, 255, 0, 0, 255, 128]);
        assert_eq!(other.items()[0].bytes("application/x-kompoz-meyve").as_deref(), Some(&[1u8, 2, 3][..]));
        // Files: a DROPFILES, read back as their paths.
        shared.clipboard.set(owner, Some(ClipEntry::with_files(vec!["C:\\Users\\kasim\\Görüntü 1.png".into()])));
        let files = super::super::clipboard::clip_entry(shared.clipboard.formats, Hwnd::NONE).expect("files");
        assert_eq!(files.files(), ["C:\\Users\\kasim\\Görüntü 1.png"]);
        // A sensitive entry: marked for the clipboard's history and monitors.
        let secret = ClipEntry::with_metadata(
            vec![ClipItem::with_text("parola")],
            kompoz_ui::clipboard::ClipMetadata::new(vec!["text/plain".into()]).with_sensitive(true),
        );
        shared.clipboard.set(owner, Some(secret));
        let read = super::super::clipboard::clip_entry(shared.clipboard.formats, Hwnd::NONE).expect("the secret");
        assert!(read.clip_metadata().is_sensitive());
        // Emptied: nothing.
        shared.clipboard.set(owner, None);
        assert!(super::super::clipboard::clip_entry(shared.clipboard.formats, Hwnd::NONE).is_none());
        shared.close();
    }
}

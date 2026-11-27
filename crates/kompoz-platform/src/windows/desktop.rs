//! The desktop's services on Windows (Linux's `DesktopServices`' counterpart, C++'s win32 accent and wallpaper
//! parts): the apps' light or dark theme, the accent color, the system's text size, high contrast, the client area's
//! animations and the mouse's and the caret's settings, read from the registry and the system's parameters
//! ([`settings`](super::settings) makes them the app's) and followed as they change — a thread of its own waits on
//! the registry's keys (`RegNotifyChangeKeyValue`: the theme's, DWM's, the accent's, the text size's, the desktop's
//! wallpaper) and tells the platform's own window, which also hears the system's broadcasts (WM_SETTINGCHANGE,
//! WM_DWMCOLORIZATIONCOLORCHANGED); the wallpaper's colors ([`wallpaper`](super::wallpaper)). The registry rather
//! than WinRT's UISettings (the stage's decision: no WinRT runtime to activate, the same keys Windows' own frames
//! read, and wine has them).
//!
//! What a change does: the theme the windows' captions and the metrics (dark theme), the text size the metrics'
//! font scale, high contrast the media (`UiMediaScope::high_contrast`), the animations the motion duration scale (0
//! when off: they end at once), the mouse's and the caret's the view configuration, the accent the dynamic tonal
//! palette (as Linux's portal accent), the wallpaper the wallpaper's palette; "intl" the language read again (the
//! metrics; Windows' `GetUserDefaultLocaleName` is asked each time), WM_TIMECHANGE the system's time zone forgotten
//! (read again as it is next asked). `KOMPOZ_NO_DESKTOP_ACCENT`: no accent; `KOMPOZ_NO_WALLPAPER_COLORS`: no
//! wallpaper's colors (shots the same on every machine).

#![allow(unsafe_code)]

use super::settings::{Changes, PointerSettings, RawSettings, Settings};
use super::sys::{self, Hwnd};
use super::wallpaper::Wallpaper;
use ::windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, WAIT_OBJECT_0};
use ::windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_NOTIFY, KEY_READ, REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME,
    RRF_RT_REG_BINARY, RRF_RT_REG_SZ, RegCloseKey, RegGetValueW, RegNotifyChangeKeyValue, RegOpenKeyExW,
};
use ::windows::Win32::System::Threading::{CreateEventW, INFINITE, SetEvent, WaitForMultipleObjects};
use ::windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use ::windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use ::windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
use ::windows::Win32::UI::WindowsAndMessaging as wm;
use ::windows::core::PCWSTR;
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use kompoz_ui::motion_duration_scale::MotionDurationScale;
use kompoz_ui::platform::ViewConfiguration;
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;
use std::thread::JoinHandle;

/// The keys followed, and what each holds.
pub(super) const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
pub(super) const DWM: &str = r"Software\Microsoft\Windows\DWM";
pub(super) const ACCENT: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent";
pub(super) const ACCESSIBILITY: &str = r"Software\Microsoft\Accessibility";
pub(super) const DESKTOP: &str = r"Control Panel\Desktop";
const KEYS: [&str; 5] = [PERSONALIZE, DWM, ACCENT, ACCESSIBILITY, DESKTOP];
/// Which of them says the wallpaper.
const DESKTOP_KEY: usize = 4;
/// WM_DESKTOP_KEY's wParam for any of them (read them all again).
const ALL_KEYS: usize = usize::MAX;

/// What the desktop's threads post to the platform's own window: a key changed (wParam: which), a wallpaper's
/// palette came.
pub(super) const WM_DESKTOP_KEY: u32 = wm::WM_APP + 0x60;
pub(super) const WM_DESKTOP_WALLPAPER: u32 = wm::WM_APP + 0x61;

/// WM_SETTINGCHANGE's wParam for the wallpaper (SPI_SETDESKWALLPAPER).
const SPI_SETDESKWALLPAPER: usize = 0x0014;

/// The settings as the registry and the system's parameters have them now (C++'s `darkMode` and DesktopAccent's
/// read), the accent read where `accent_wanted`.
pub(super) fn read_settings(accent_wanted: bool) -> Settings {
    Settings::from_raw(&read_raw(), accent_wanted)
}

/// Their values.
fn read_raw() -> RawSettings {
    RawSettings {
        apps_use_light_theme: sys::user_dword(PERSONALIZE, "AppsUseLightTheme"),
        accent_color: sys::user_dword(DWM, "AccentColor"),
        accent_palette: user_binary(ACCENT, "AccentPalette"),
        colorization_color: sys::user_dword(DWM, "ColorizationColor"),
        text_scale_factor: sys::user_dword(ACCESSIBILITY, "TextScaleFactor"),
        high_contrast: high_contrast(),
        client_area_animation: spi_bool(wm::SPI_GETCLIENTAREAANIMATION),
        pointer: pointer_settings(),
    }
}

/// A REG_BINARY value of the user's (none: not there).
fn user_binary(key: &str, value: &str) -> Option<Vec<u8>> {
    let (key, value) = (sys::wide(key), sys::wide(value));
    let mut size = 0u32;
    // SAFETY: the strings live through the call; no buffer: only its size asked (written to `size`).
    let asked = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_BINARY,
            None,
            None,
            Some(&mut size),
        )
    };
    if asked != ERROR_SUCCESS || size == 0 || size > 4096 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    // SAFETY: as above; `data` is `size` writable bytes, the size said, and the call writes at most that many.
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_BINARY,
            None,
            Some(data.as_mut_ptr() as *mut c_void),
            Some(&mut size),
        )
    };
    if read != ERROR_SUCCESS {
        return None;
    }
    data.truncate(size as usize);
    Some(data)
}

/// A REG_SZ (or REG_EXPAND_SZ, expanded) value of the user's (none: not there).
pub(super) fn user_string(key: &str, value: &str) -> Option<String> {
    let (key, value) = (sys::wide(key), sys::wide(value));
    let mut text = vec![0u16; 1024];
    let mut size = (text.len() * 2) as u32;
    // SAFETY: the strings live through the call; `text` is `size` writable bytes, the size said; the call writes a
    // 0-terminated string of at most that many.
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ, // (a REG_EXPAND_SZ too, expanded)
            None,
            Some(text.as_mut_ptr() as *mut c_void),
            Some(&mut size),
        )
    };
    if read != ERROR_SUCCESS {
        return None;
    }
    let n = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    Some(String::from_utf16_lossy(&text[..n]))
}

/// A BOOL system parameter (none: not told).
fn spi_bool(action: wm::SYSTEM_PARAMETERS_INFO_ACTION) -> Option<bool> {
    let mut on = ::windows::core::BOOL(0);
    // SAFETY: a BOOL-valued parameter: pvParam points to a BOOL, written.
    let read = unsafe {
        wm::SystemParametersInfoW(
            action,
            0,
            Some(&mut on as *mut _ as *mut c_void),
            wm::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    read.ok().map(|_| on.as_bool())
}

/// A UINT system parameter (none: not told).
fn spi_u32(action: wm::SYSTEM_PARAMETERS_INFO_ACTION) -> Option<u32> {
    let mut value = 0u32;
    // SAFETY: a UINT-valued parameter: pvParam points to a UINT, written.
    let read = unsafe {
        wm::SystemParametersInfoW(
            action,
            0,
            Some(&mut value as *mut u32 as *mut c_void),
            wm::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    read.ok().map(|_| value)
}

/// Whether a high contrast theme is on (SPI_GETHIGHCONTRAST's HCF_HIGHCONTRASTON).
fn high_contrast() -> bool {
    let mut hc = HIGHCONTRASTW { cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32, ..Default::default() };
    // SAFETY: pvParam points to a HIGHCONTRASTW whose size is in cbSize, as uiParam says; written (its scheme's
    // name, a pointer into the system's own memory, isn't read).
    let read = unsafe {
        wm::SystemParametersInfoW(
            wm::SPI_GETHIGHCONTRAST,
            hc.cbSize,
            Some(&mut hc as *mut HIGHCONTRASTW as *mut c_void),
            wm::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    read.is_ok() && hc.dwFlags.contains(HCF_HIGHCONTRASTON)
}

/// The mouse's and the caret's settings, lengths at 96 DPI.
fn pointer_settings() -> PointerSettings {
    let metric = |index: wm::SYSTEM_METRICS_INDEX, default: u32| {
        // SAFETY: no pointers.
        let v = unsafe { GetSystemMetricsForDpi(index, 96) };
        if v > 0 { v as u32 } else { default }
    };
    let defaults = PointerSettings::default();
    PointerSettings {
        // SAFETY: no arguments.
        double_click_ms: Some(unsafe { GetDoubleClickTime() }).filter(|&t| t > 0).unwrap_or(defaults.double_click_ms),
        double_click: (metric(wm::SM_CXDOUBLECLK, 4), metric(wm::SM_CYDOUBLECLK, 4)),
        drag: (metric(wm::SM_CXDRAG, 4), metric(wm::SM_CYDRAG, 4)),
        // SAFETY: no arguments (0 when it fails: Windows' default then).
        caret_blink_ms: Some(unsafe { wm::GetCaretBlinkTime() }).filter(|&t| t > 0).unwrap_or(defaults.caret_blink_ms),
        wheel_lines: spi_u32(wm::SPI_GETWHEELSCROLLLINES).unwrap_or(defaults.wheel_lines),
    }
}

/// An auto-reset event's handle, as a plain value that can go to another thread (closed by its owner, [`Watcher`],
/// after the thread that waits on it ended).
#[derive(Clone, Copy)]
struct RawEvent(isize);

impl RawEvent {
    fn new() -> Option<RawEvent> {
        // SAFETY: no attributes, no name: an anonymous auto-reset event, not set.
        let handle = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }.ok()?;
        Some(RawEvent(handle.0 as isize))
    }
    fn handle(self) -> HANDLE {
        HANDLE(self.0 as *mut c_void)
    }
    fn set(self) {
        // SAFETY: the event is open (its owner closes it only after the threads that use it ended).
        let _ = unsafe { SetEvent(self.handle()) };
    }
    fn close(self) {
        // SAFETY: closed once, by its owner, after its threads ended.
        let _ = unsafe { CloseHandle(self.handle()) };
    }
}

/// A key followed: open (`exact`: itself, else the nearest of its parents there is, watched with its subtree until
/// the key itself is made), and the event its change sets.
struct Watched {
    path: &'static str,
    key: Option<(HKEY, bool)>,
    event: RawEvent,
}

impl Watched {
    /// Opened (the key itself if it is there now) and armed: its next change (a value set or a subkey made) sets
    /// its event.
    fn arm(&mut self) {
        if self.key.is_none_or(|(_, exact)| !exact) {
            let mut path = self.path;
            loop {
                if let Some(key) = open(path) {
                    if let Some((old, _)) = self.key.replace((key, path == self.path)) {
                        close(old);
                    }
                    break;
                }
                match path.rfind('\\') {
                    Some(at) => path = &path[..at],
                    None => break,
                }
            }
        }
        if let Some((key, exact)) = self.key {
            // SAFETY: the key is open; the event too (both outlive the notification: the key is closed only as the
            // thread ends, which closes the notification with it).
            let _ = unsafe {
                RegNotifyChangeKeyValue(
                    key,
                    !exact,
                    REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                    Some(self.event.handle()),
                    true,
                )
            };
        }
    }
}

/// A key of the user's opened to be followed (none: not there).
fn open(path: &str) -> Option<HKEY> {
    let path = sys::wide(path);
    let mut key = HKEY::default();
    // SAFETY: the path lives through the call; `key` is written.
    let opened =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), None, KEY_NOTIFY | KEY_READ, &mut key) };
    (opened == ERROR_SUCCESS).then_some(key)
}

fn close(key: HKEY) {
    // SAFETY: opened by `open`, closed once.
    let _ = unsafe { RegCloseKey(key) };
}

/// The registry's keys followed on a thread of its own: a change posts [`WM_DESKTOP_KEY`] (which key) to the
/// platform's own window.
struct Watcher {
    stop: RawEvent,
    thread: Option<JoinHandle<()>>,
}

impl Watcher {
    /// Started, its keys armed before it returns (a change from then on is told; one between the settings' first
    /// reading and the arming too: the thread asks for a reading once armed).
    fn start(helper: Hwnd) -> Option<Watcher> {
        let stop = RawEvent::new()?;
        let (armed, is_armed) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new().name("kompoz-registry".into()).spawn(move || {
            watch(stop, helper, move || {
                let _ = armed.send(());
            })
        });
        match thread {
            Ok(thread) => {
                let _ = is_armed.recv_timeout(std::time::Duration::from_secs(2));
                Some(Watcher { stop, thread: Some(thread) })
            }
            Err(_) => {
                stop.close();
                None
            }
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.set();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.stop.close();
    }
}

/// The thread: each key armed (then `armed`, and a reading asked for), then each change told (and the key armed
/// again) until stopped.
fn watch(stop: RawEvent, helper: Hwnd, armed: impl FnOnce()) {
    let mut keys: Vec<Watched> = Vec::new();
    for path in KEYS {
        let Some(event) = RawEvent::new() else { break };
        keys.push(Watched { path, key: None, event });
    }
    for k in &mut keys {
        k.arm();
    }
    armed();
    helper.post(WM_DESKTOP_KEY, ALL_KEYS, 0);
    let mut handles = vec![stop.handle()];
    handles.extend(keys.iter().map(|k| k.event.handle()));
    loop {
        // SAFETY: the events are open while this thread runs (its own, closed below; `stop` closed after it ended).
        let woken = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
        let i = woken.0.wrapping_sub(WAIT_OBJECT_0.0) as usize;
        if i == 0 || i >= handles.len() {
            break; // (stopped, or the wait failed)
        }
        let k = &mut keys[i - 1];
        k.arm();
        helper.post(WM_DESKTOP_KEY, i - 1, 0);
    }
    for k in keys {
        if let Some((key, _)) = k.key {
            close(key);
        }
        k.event.close();
    }
}

/// The desktop's services the platform runs (on the UI thread; its threads post to the platform's own window).
pub(super) struct Desktop {
    settings: RefCell<Settings>,
    accent_wanted: bool,
    /// The motion duration scale (0: the client area's animations off), what the windows' scales read.
    motion: Rc<Cell<f32>>,
    watcher: RefCell<Option<Watcher>>,
    wallpaper: RefCell<Option<Wallpaper>>,
    /// Followed (started): the settings read again as they change.
    following: Cell<bool>,
}

impl Desktop {
    /// The settings read now (`KOMPOZ_NO_DESKTOP_ACCENT`: without the accent); nothing followed yet.
    pub(super) fn new() -> Desktop {
        let accent_wanted = kompoz_runtime::environment::var_os("KOMPOZ_NO_DESKTOP_ACCENT").is_none();
        let settings = read_settings(accent_wanted);
        Desktop {
            motion: Rc::new(Cell::new(settings.motion_duration_scale())),
            settings: RefCell::new(settings),
            accent_wanted,
            watcher: RefCell::new(None),
            wallpaper: RefCell::new(None),
            following: Cell::new(false),
        }
    }

    /// Followed from now on, the threads posting to `helper` (the platform's own window); the wallpaper looked for
    /// unless `KOMPOZ_NO_WALLPAPER_COLORS`.
    pub(super) fn start(&self, helper: Hwnd) {
        self.following.set(true);
        *self.watcher.borrow_mut() = Watcher::start(helper);
        if kompoz_runtime::environment::var_os("KOMPOZ_NO_WALLPAPER_COLORS").is_none() {
            *self.wallpaper.borrow_mut() = Wallpaper::start(helper);
        }
    }

    /// The threads ended (the registry's at once; a wallpaper's palette still being worked out told to stop, not
    /// waited for).
    pub(super) fn stop(&self) {
        self.following.set(false);
        self.watcher.borrow_mut().take();
        self.wallpaper.borrow_mut().take();
    }

    /// Read again (where followed): what changed.
    fn read_again(&self) -> Changes {
        if !self.following.get() {
            return Changes::default();
        }
        let now = read_settings(self.accent_wanted);
        let changes = Changes::between(&self.settings.borrow(), &now);
        self.motion.set(now.motion_duration_scale());
        *self.settings.borrow_mut() = now;
        changes
    }

    /// WM_SETTINGCHANGE (`w` its wParam, `name` its area's): all read again (Windows says which parameter or area
    /// changed only now and then; reading is cheap); the wallpaper looked at again for SPI_SETDESKWALLPAPER; "intl"
    /// the metrics (the language).
    pub(super) fn setting_changed(&self, w: usize, name: Option<&str>) -> Changes {
        if w == SPI_SETDESKWALLPAPER {
            self.wallpaper_changed();
        }
        let mut changes = self.read_again();
        changes.metrics |= self.following.get() && name == Some("intl");
        changes
    }

    /// A key changed (WM_DESKTOP_KEY's wParam: which): read again; the desktop's key the wallpaper's too.
    pub(super) fn key_changed(&self, which: usize) -> Changes {
        if which == DESKTOP_KEY || which == ALL_KEYS {
            self.wallpaper_changed();
        }
        self.read_again()
    }

    /// DWM's colorization changed (WM_DWMCOLORIZATIONCOLORCHANGED): read again.
    pub(super) fn colorization_changed(&self) -> Changes {
        self.read_again()
    }

    /// The wallpaper may have changed: its thread looks again.
    fn wallpaper_changed(&self) {
        if let Some(w) = self.wallpaper.borrow().as_ref() {
            w.look_again();
        }
    }

    /// The monitor the app's main window is on (its rectangle on the virtual screen): the wallpaper's (Windows has
    /// one a monitor).
    pub(super) fn set_monitor(&self, rect: sys::Rect) {
        if let Some(w) = self.wallpaper.borrow().as_ref() {
            w.set_monitor(rect);
        }
    }

    pub(super) fn dark(&self) -> bool {
        self.settings.borrow().dark
    }

    pub(super) fn font_scale(&self) -> f32 {
        self.settings.borrow().text_scale
    }

    pub(super) fn high_contrast(&self) -> bool {
        self.settings.borrow().high_contrast
    }

    #[cfg(test)]
    pub(super) fn settings(&self) -> Settings {
        self.settings.borrow().clone()
    }

    /// The accent's palette (none: no accent told).
    pub(super) fn accent_palette(&self) -> Option<DynamicTonalPalette> {
        self.settings.borrow().accent.map(DynamicTonalPalette::from_seed)
    }

    /// The wallpaper's palette, the latest (none: none found, or not looked for).
    pub(super) fn wallpaper_palette(&self) -> Option<DynamicTonalPalette> {
        self.wallpaper.borrow().as_ref().and_then(Wallpaper::palette)
    }

    /// The view configuration at `scale` px per dp.
    pub(super) fn view_configuration(&self, scale: f32) -> ViewConfiguration {
        self.settings.borrow().pointer.view_configuration(scale)
    }

    /// A window's motion duration scale: 0 while the client area's animations are off, read as each animation asks.
    pub(super) fn motion_duration_scale(&self) -> MotionDurationScale {
        let motion = self.motion.clone();
        MotionDurationScale::from_fn(move || motion.get())
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The system's time changed (WM_TIMECHANGE: its clock, or its zone): the system's zone read again as it is next
/// asked (Windows' zone may be another now).
pub(super) fn time_changed() {
    kompoz_runtime::environment::set_system_time_zone(None);
}

#[cfg(test)]
mod tests {
    use super::super::window::{Shared, Win32Window};
    use super::*;
    use crate::app::{Event, Platform};
    use ::windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use ::windows::Win32::System::Registry::{
        KEY_SET_VALUE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_VALUE_TYPE, RRF_NOEXPAND, RRF_RT_ANY, RegCreateKeyExW,
        RegDeleteValueW, RegSetValueExW,
    };
    use ::windows::core::s;
    use kompoz_graphics::Color;
    use kompoz_graphics::material_color::hct::Hct;
    use kompoz_ui::platform::WheelScrollAmount;
    use kompoz_ui::window::WindowId;
    use std::time::{Duration, Instant};

    /// Running under wine (ntdll's `wine_get_version`): what a real Windows' user would see changed system-wide
    /// (a high contrast theme) is changed only there.
    fn under_wine() -> bool {
        let ntdll = sys::wide("ntdll.dll");
        // SAFETY: a module's name and an export's, 0-terminated and alive through the calls.
        unsafe {
            GetModuleHandleW(PCWSTR(ntdll.as_ptr()))
                .ok()
                .is_some_and(|m| GetProcAddress(m, s!("wine_get_version")).is_some())
        }
    }

    /// A value of the user's as it was (its type and bytes; none: not there), to put back.
    fn saved(key: &str, value: &str) -> Option<(REG_VALUE_TYPE, Vec<u8>)> {
        let (k, v) = (sys::wide(key), sys::wide(value));
        let mut kind = REG_VALUE_TYPE(0);
        let mut data = vec![0u8; 4096];
        let mut size = data.len() as u32;
        // SAFETY: the strings live through the call; `data` is `size` writable bytes, as said; `kind` is written.
        let read = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(k.as_ptr()),
                PCWSTR(v.as_ptr()),
                RRF_RT_ANY | RRF_NOEXPAND,
                Some(&mut kind),
                Some(data.as_mut_ptr() as *mut c_void),
                Some(&mut size),
            )
        };
        (read == ERROR_SUCCESS).then(|| {
            data.truncate(size as usize);
            (kind, data)
        })
    }

    /// A value of the user's set (its key made where it isn't there), as RegSetValueEx does; none: deleted.
    fn put(key: &str, value: &str, data: Option<(REG_VALUE_TYPE, &[u8])>) {
        let (k, v) = (sys::wide(key), sys::wide(value));
        let mut opened = HKEY::default();
        // SAFETY: the strings live through the calls; the key is written, then closed.
        unsafe {
            let made = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(k.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut opened,
                None,
            );
            assert_eq!(made, ERROR_SUCCESS, "{key} made");
            match data {
                Some((kind, bytes)) => {
                    let set = RegSetValueExW(opened, PCWSTR(v.as_ptr()), None, kind, Some(bytes));
                    assert_eq!(set, ERROR_SUCCESS, "{key}\\{value} set");
                }
                None => {
                    let _ = RegDeleteValueW(opened, PCWSTR(v.as_ptr()));
                }
            }
            let _ = RegCloseKey(opened);
        }
    }

    fn put_dword(key: &str, value: &str, v: u32) {
        put(key, value, Some((REG_DWORD, &v.to_le_bytes())));
    }

    /// A system parameter set as Settings sets it (written, its change told to the windows: SPIF_SENDCHANGE).
    fn set_parameter(action: wm::SYSTEM_PARAMETERS_INFO_ACTION, ui: u32, pv: *mut c_void) -> bool {
        // SAFETY: each caller passes what its parameter takes (a value in uiParam or pvParam, or a struct's or a
        // string's pointer, alive through the call).
        unsafe { wm::SystemParametersInfoW(action, ui, Some(pv), wm::SPIF_UPDATEINIFILE | wm::SPIF_SENDCHANGE) }.is_ok()
    }

    fn set_high_contrast(on: bool) -> bool {
        let mut hc = HIGHCONTRASTW { cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32, ..Default::default() };
        if on {
            hc.dwFlags = HCF_HIGHCONTRASTON;
        }
        set_parameter(wm::SPI_SETHIGHCONTRAST, hc.cbSize, &mut hc as *mut HIGHCONTRASTW as *mut c_void)
    }

    fn set_wallpaper(path: &str) -> bool {
        let mut w = sys::wide(path);
        set_parameter(wm::SPI_SETDESKWALLPAPER, 0, w.as_mut_ptr() as *mut c_void)
    }

    /// What the test changed, put back however it ends.
    struct Restore {
        values: Vec<(&'static str, &'static str, Option<(REG_VALUE_TYPE, Vec<u8>)>)>,
        animation: Option<bool>,
        high_contrast: Option<bool>,
        pointer: PointerSettings,
        wallpaper: Option<String>,
    }

    impl Drop for Restore {
        fn drop(&mut self) {
            for (key, value, data) in &self.values {
                put(key, value, data.as_ref().map(|(kind, bytes)| (*kind, bytes.as_slice())));
            }
            if let Some(on) = self.animation {
                set_parameter(wm::SPI_SETCLIENTAREAANIMATION, 0, on as usize as *mut c_void);
            }
            if let Some(on) = self.high_contrast {
                set_high_contrast(on);
            }
            set_parameter(wm::SPI_SETWHEELSCROLLLINES, self.pointer.wheel_lines, std::ptr::null_mut());
            set_parameter(wm::SPI_SETDOUBLECLICKTIME, self.pointer.double_click_ms, std::ptr::null_mut());
            if let Some(path) = &self.wallpaper {
                set_wallpaper(path);
            }
        }
    }

    /// Messages handled and the events gathered until `done` says so of them (at most ten seconds: the registry's
    /// thread, a palette's, wine's server between); whether it did.
    fn until(shared: &Shared, events: &mut Vec<Event>, mut done: impl FnMut(&[Event]) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            while let Some(m) = sys::peek_message() {
                sys::dispatch(&m);
            }
            events.extend(shared.take_events().into_iter().map(|(_, e)| e));
            if done(events) {
                return true;
            }
            if Instant::now() > deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The desktop's settings followed as they change while the app runs: the theme, the accent and the text size
    /// written to the registry (RegSetValueEx, as Settings writes them); the animations, the wheel's lines, the
    /// double click time and the wallpaper set as system parameters; high contrast too under wine (on a real
    /// Windows it switches the user's theme); "intl" and the time changing. Everything put back after.
    #[test]
    fn the_desktop_is_followed_as_it_changes() {
        let wine = under_wine();
        let _restore = Restore {
            values: [(PERSONALIZE, "AppsUseLightTheme"), (DWM, "AccentColor"), (ACCESSIBILITY, "TextScaleFactor")]
                .into_iter()
                .map(|(k, v)| (k, v, saved(k, v)))
                .collect(),
            animation: spi_bool(wm::SPI_GETCLIENTAREAANIMATION),
            high_contrast: wine.then(high_contrast),
            pointer: pointer_settings(),
            wallpaper: super::super::wallpaper::desk_wallpaper(),
        };
        put_dword(PERSONALIZE, "AppsUseLightTheme", 1);
        put_dword(DWM, "AccentColor", 0xff_d7_78_00); // (ABGR: #0078D7)
        put(ACCESSIBILITY, "TextScaleFactor", None);
        let shared = Shared::connect(WindowId::MAIN).expect("the platform");
        let mut window = Win32Window::new(&shared, WindowId::MAIN);
        shared.start_desktop();
        let d = &shared.desktop;
        let mut events = Vec::new();
        assert!(!d.dark() && !window.metrics().dark_theme);
        assert_eq!(d.settings().accent, Some(Color::from_argb(0xff0078d7)));
        assert_eq!(window.metrics().font_scale, 1.0);

        // The theme: dark.
        put_dword(PERSONALIZE, "AppsUseLightTheme", 0);
        assert!(until(&shared, &mut events, |e| e.contains(&Event::MetricsChanged)), "the theme followed");
        assert!(d.dark() && window.metrics().dark_theme);
        events.clear();

        // The accent: its palette.
        put_dword(DWM, "AccentColor", 0xff_32_7d_2e); // (#2E7D32)
        assert!(until(&shared, &mut events, |e| e.contains(&Event::AccentChanged)), "the accent followed");
        assert_eq!(d.accent_palette(), Some(DynamicTonalPalette::from_seed(Color::from_argb(0xff2e7d32))));
        events.clear();

        // The text size: the font scale.
        put_dword(ACCESSIBILITY, "TextScaleFactor", 150);
        assert!(until(&shared, &mut events, |e| e.contains(&Event::MetricsChanged)), "the text size followed");
        assert_eq!(window.metrics().font_scale, 1.5);
        events.clear();

        // The animations off: the motion duration scale 0, as each animation reads it.
        let scale = window.motion_duration_scale().expect("a scale");
        assert_eq!(scale.scale_factor(), 1.0);
        assert!(set_parameter(wm::SPI_SETCLIENTAREAANIMATION, 0, std::ptr::null_mut()), "animations set off");
        assert!(until(&shared, &mut events, |_| scale.scale_factor() == 0.0), "the animations followed");
        assert!(set_parameter(wm::SPI_SETCLIENTAREAANIMATION, 0, true as usize as *mut c_void));
        assert!(until(&shared, &mut events, |_| scale.scale_factor() == 1.0));
        events.clear();

        // The wheel's lines and the double click time: the view configuration.
        assert!(set_parameter(wm::SPI_SETWHEELSCROLLLINES, 7, std::ptr::null_mut()));
        assert!(set_parameter(wm::SPI_SETDOUBLECLICKTIME, 700, std::ptr::null_mut()));
        let followed = until(&shared, &mut events, |_| {
            let c = d.view_configuration(1.0);
            c.double_tap_timeout_millis() == 700 && c.wheel_scroll_amount == WheelScrollAmount::Lines(7.0)
        });
        assert!(followed, "the mouse's settings followed: {:?}", d.settings().pointer);
        assert!(d.view_configuration(2.0).double_click_area.is_some_and(|a| a.width > 0.0));
        assert!(events.contains(&Event::MetricsChanged));
        events.clear();

        // High contrast (under wine only: a real Windows' theme isn't switched by a test), where it can be set (wine
        // 11's SPI_SETHIGHCONTRAST is a FIXME, its SPI_GETHIGHCONTRAST always "off": its reading is `settings`'
        // test): the media say it.
        if wine && !set_high_contrast(true) {
            println!("high contrast: this wine can't set it (SPI_SETHIGHCONTRAST)");
        } else if wine {
            assert!(until(&shared, &mut events, |e| e.contains(&Event::MetricsChanged)), "high contrast followed");
            assert!(window.media(false).high_contrast);
            assert!(set_high_contrast(false));
            assert!(until(&shared, &mut events, |_| !window.media(false).high_contrast));
            events.clear();
        }

        // The wallpaper: its palette (unless KOMPOZ_NO_WALLPAPER_COLORS).
        if kompoz_runtime::environment::var_os("KOMPOZ_NO_WALLPAPER_COLORS").is_none() {
            let path = std::env::temp_dir().join(format!("kompoz-wallpaper-{}.bmp", std::process::id()));
            std::fs::write(&path, crate::wallpaper_palette::test_bmp(0xD32F2F, 64, 48)).unwrap();
            assert!(set_wallpaper(&path.to_string_lossy()), "the wallpaper set");
            let hue = Hct::from_color(DynamicTonalPalette::from_seed(Color::hex(0xD32F2F)).accent1[8]).hue();
            let red = |d: &Desktop| {
                d.wallpaper_palette().is_some_and(|p| (Hct::from_color(p.accent1[8]).hue() - hue).abs() < 8.0)
            };
            let followed = until(&shared, &mut events, |e| e.contains(&Event::WallpaperChanged) && red(d));
            assert!(followed, "the wallpaper followed");
            events.clear();
            let _ = std::fs::remove_file(path);
        }

        // The language ("intl"): the metrics read again; the time: a wake (the zone read again as next asked).
        let intl = sys::wide("intl");
        shared.helper_window().send(wm::WM_SETTINGCHANGE, 0, intl.as_ptr() as isize);
        assert!(until(&shared, &mut events, |e| e.contains(&Event::MetricsChanged)), "intl");
        events.clear();
        shared.helper_window().send(wm::WM_TIMECHANGE, 0, 0);
        assert!(until(&shared, &mut events, |e| e.contains(&Event::Wake)), "the time");

        drop(window);
        shared.close();
    }
}

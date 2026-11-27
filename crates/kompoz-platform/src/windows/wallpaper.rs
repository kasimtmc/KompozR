//! The wallpaper's own colors on Windows (C++'s common/wallpaper_colors.cpp's Windows part, Linux's `wallpaper`'s
//! counterpart): the wallpaper of the monitor the app's main window is on (IDesktopWallpaper: each monitor's own,
//! a slideshow's picture now), else the desktop's (the `Control Panel\Desktop` key's `Wallpaper`, what
//! SPI_GETDESKWALLPAPER says), else the copy Windows keeps of it (`%APPDATA%\Microsoft\Windows\Themes\TranscodedWallpaper`, the
//! picture a slideshow or a deleted file leaves); its palette worked out by the core Linux shares
//! ([`wallpaper_palette`](crate::wallpaper_palette)) on a thread of its own, and again whenever it may have changed
//! — the platform's own window tells (WM_SETTINGCHANGE's SPI_SETDESKWALLPAPER, the desktop's key changed), and the
//! picture is looked at again every 30 seconds (a slideshow's next one, a picture replaced in place, its file's time).
//! A palette that came is posted to the platform's own window ([`WM_DESKTOP_WALLPAPER`]).

#![allow(unsafe_code)]

use super::desktop::{DESKTOP, WM_DESKTOP_WALLPAPER};
use super::sys::{self, Hwnd, Rect};
use crate::wallpaper_palette::PaletteWork;
use ::windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
use ::windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use ::windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForSingleObject};
use ::windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};
use ::windows::Win32::UI::WindowsAndMessaging as wm;
use ::windows::core::{PCWSTR, PWSTR};
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

/// How often the picture is looked at again without being told.
const LOOK_AGAIN: Duration = Duration::from_secs(30);

/// What the thread and the platform share.
struct Shared {
    running: AtomicBool,
    palette: Mutex<Option<DynamicTonalPalette>>,
    /// The monitor whose wallpaper it is (none yet: the primary one's).
    monitor: Mutex<Option<Rect>>,
    /// The thread's wake (an auto-reset event's handle: look again, a palette came, stop).
    wake: isize,
    helper: Hwnd,
}

impl Shared {
    fn wake(&self) {
        // SAFETY: the event is open while this is (closed as the last of the threads holding it lets it go).
        let _ = unsafe { SetEvent(HANDLE(self.wake as *mut c_void)) };
    }

    /// The palette now: told the platform's window if it changed.
    fn tell(&self, value: Option<DynamicTonalPalette>) {
        {
            let mut palette = self.palette.lock().unwrap_or_else(|e| e.into_inner());
            if *palette == value {
                return;
            }
            *palette = value;
        }
        self.helper.post(WM_DESKTOP_WALLPAPER, 0, 0);
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        // SAFETY: made by `Wallpaper::start`, closed once, here: no thread holds it any more.
        let _ = unsafe { CloseHandle(HANDLE(self.wake as *mut c_void)) };
    }
}

/// The wallpaper's colors, found and followed on a thread of its own.
pub(super) struct Wallpaper {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Wallpaper {
    /// Looking for it from now on; each palette that comes posted to `helper`.
    pub(super) fn start(helper: Hwnd) -> Option<Wallpaper> {
        // SAFETY: no attributes, no name: an anonymous auto-reset event, not set.
        let wake = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }.ok()?;
        let shared = Arc::new(Shared {
            running: AtomicBool::new(true),
            palette: Mutex::new(None),
            monitor: Mutex::new(None),
            wake: wake.0 as isize,
            helper,
        });
        let theirs = shared.clone();
        let thread = std::thread::Builder::new().name("kompoz-wallpaper".into()).spawn(move || run(&theirs)).ok()?;
        Some(Wallpaper { shared, thread: Some(thread) })
    }

    /// The latest palette; none: no wallpaper found (or it couldn't be read).
    pub(super) fn palette(&self) -> Option<DynamicTonalPalette> {
        *self.shared.palette.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// It may have changed: looked at again now.
    pub(super) fn look_again(&self) {
        self.shared.wake();
    }

    /// The monitor whose wallpaper it is.
    pub(super) fn set_monitor(&self, rect: Rect) {
        let changed = self.shared.monitor.lock().unwrap_or_else(|e| e.into_inner()).replace(rect) != Some(rect);
        if changed {
            self.shared.wake();
        }
    }
}

impl Drop for Wallpaper {
    fn drop(&mut self) {
        self.shared.running.store(false, Ordering::SeqCst);
        self.shared.wake();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Which wallpaper: its path and its file's time.
type Key = (Option<String>, Option<SystemTime>);

/// The thread: COM for IDesktopWallpaper, then the wallpaper read whenever it may have changed (its path or its
/// file's time differ), its palette worked out on a thread of its own and told when it comes, unless the wallpaper
/// changed again by then (Linux's loop).
fn run(shared: &Arc<Shared>) {
    // SAFETY: this thread's COM, in the multithreaded apartment (no messages to pump); balanced below.
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    let mut told: Option<Key> = None;
    let mut working: Option<(Key, PaletteWork)> = None;
    while shared.running.load(Ordering::SeqCst) {
        let monitor = *shared.monitor.lock().unwrap_or_else(|e| e.into_inner());
        let path = find_wallpaper(monitor, com);
        let stamp = path.as_ref().and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        let key: Key = (path.clone(), stamp);
        let ready = working.as_ref().filter(|(k, _)| *k == key).and_then(|(_, work)| work.take());
        if let Some(palette) = ready {
            working = None;
            told = Some(key);
            shared.tell(palette);
        } else if told.as_ref() == Some(&key) {
            working = None; // (back to the one told before another's palette came: that one stops)
        } else if working.as_ref().is_none_or(|(k, _)| *k != key) {
            working = None; // (one for a wallpaper no longer shown stops)
            match path {
                Some(path) => {
                    let wake = shared.clone();
                    working = Some((key, PaletteWork::start(path, move || wake.wake())));
                }
                None => {
                    told = Some(key);
                    shared.tell(None);
                }
            }
        }
        // SAFETY: the event is open while `shared` is.
        let _ = unsafe { WaitForSingleObject(HANDLE(shared.wake as *mut c_void), LOOK_AGAIN.as_millis() as u32) };
    }
    drop(working); // (a palette still being worked out told to stop, not waited for)
    if com {
        // SAFETY: balanced with the CoInitializeEx above, on this thread.
        unsafe { CoUninitialize() };
    }
}

/// Where the wallpaper is: the one of the monitor at `monitor` (IDesktopWallpaper, where COM is there and has it),
/// else the desktop's, else the copy Windows keeps; none if no picture is (a solid color).
fn find_wallpaper(monitor: Option<Rect>, com: bool) -> Option<String> {
    let usable = |p: &str| !p.is_empty() && std::path::Path::new(p).is_file();
    if com && let Some(p) = monitor_wallpaper(monitor).filter(|p| usable(p)) {
        return Some(p);
    }
    // (The key's value first: what SPI_GETDESKWALLPAPER says on Windows, kept by it; wine's SPI_GETDESKWALLPAPER is
    // a process's copy, not told when another process sets the wallpaper.)
    if let Some(p) = super::desktop::user_string(DESKTOP, "Wallpaper").filter(|p| usable(p)) {
        return Some(p);
    }
    if let Some(p) = desk_wallpaper().filter(|p| usable(p)) {
        return Some(p);
    }
    let transcoded = std::env::var_os("APPDATA")
        .map(|d| PathBuf::from(d).join(r"Microsoft\Windows\Themes\TranscodedWallpaper"))
        .filter(|p| p.is_file())?;
    // (empty when the desktop is a solid color)
    std::fs::metadata(&transcoded).ok().filter(|m| m.len() > 0)?;
    Some(transcoded.to_string_lossy().into_owned())
}

/// SPI_GETDESKWALLPAPER: the desktop's picture's path (empty: none).
pub(super) fn desk_wallpaper() -> Option<String> {
    let mut path = [0u16; MAX_PATH as usize + 1];
    // SAFETY: pvParam is `path`, uiParam its length in characters; the call writes a 0-terminated path into it.
    let read = unsafe {
        wm::SystemParametersInfoW(
            wm::SPI_GETDESKWALLPAPER,
            path.len() as u32,
            Some(path.as_mut_ptr() as *mut c_void),
            wm::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    read.ok()?;
    let n = path.iter().position(|&c| c == 0).unwrap_or(path.len());
    Some(String::from_utf16_lossy(&path[..n]))
}

/// A COM string the callee allocated, as a String (freed).
fn take_string(s: PWSTR) -> Option<String> {
    if s.is_null() {
        return None;
    }
    // SAFETY: a 0-terminated string from the call that returned it; read before it is freed, freed once.
    let text = unsafe { s.to_string() }.ok();
    // SAFETY: allocated with CoTaskMemAlloc by the callee (IDesktopWallpaper's strings are).
    unsafe { CoTaskMemFree(Some(s.0 as *const c_void)) };
    text
}

/// IDesktopWallpaper's picture for the monitor at `monitor` (the one containing its middle; the first where none
/// does, or none is given): a slideshow's current picture too.
fn monitor_wallpaper(monitor: Option<Rect>) -> Option<String> {
    // SAFETY: COM is initialized on this thread (the caller's `com`); the object is released as it goes.
    let wallpaper: IDesktopWallpaper = unsafe { CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL) }.ok()?;
    // SAFETY: a call on the object made above.
    let count = unsafe { wallpaper.GetMonitorDevicePathCount() }.ok()?;
    let mut first = None;
    for i in 0..count {
        // SAFETY: as above; the path is the callee's, taken (freed) at once.
        let Some(id) = take_string(unsafe { wallpaper.GetMonitorDevicePathAt(i) }.unwrap_or(PWSTR::null())) else {
            continue;
        };
        let wide = sys::wide(&id);
        // SAFETY: as above; `wide` is 0-terminated and lives through the calls.
        let rect = unsafe { wallpaper.GetMonitorRECT(PCWSTR(wide.as_ptr())) };
        // SAFETY: as above.
        let path = take_string(unsafe { wallpaper.GetWallpaper(PCWSTR(wide.as_ptr())) }.unwrap_or(PWSTR::null()));
        let Some(path) = path.filter(|p| !p.is_empty()) else { continue };
        let contains = match (monitor, rect) {
            (Some(m), Ok(r)) => {
                let (x, y) = ((m.left + m.right) / 2, (m.top + m.bottom) / 2);
                r.left <= x && x < r.right && r.top <= y && y < r.bottom
            }
            _ => false,
        };
        if contains {
            return Some(path);
        }
        first.get_or_insert(path);
    }
    first
}

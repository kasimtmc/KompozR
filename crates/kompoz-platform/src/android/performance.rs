//! Android's performance hints and the surface's refresh rate vote (C++'s android_performance and its
//! `setFrameRate`): an ADPF session (`APerformanceHint`, Android 13) for the UI thread, its target the display's
//! beat, told each frame's own work (`reportActualWorkDuration`) so the system sets the CPU's clocks for the
//! frames to come rather than after they are late; and the refresh rate the app votes for on the view's window
//! (`ANativeWindow_setFrameRate`, Android 11). Both looked up at run time: the app runs from Android 10.
//!
//! An FFI module: its `unsafe` blocks say why they hold.

#![allow(unsafe_code)]

use std::ffi::{CStr, c_void};
use std::sync::OnceLock;

type GetManager = unsafe extern "C" fn() -> *mut c_void;
type CreateSession = unsafe extern "C" fn(*mut c_void, *const i32, usize, i64) -> *mut c_void;
type UpdateTarget = unsafe extern "C" fn(*mut c_void, i64) -> i32;
type ReportActual = unsafe extern "C" fn(*mut c_void, i64) -> i32;
type CloseSession = unsafe extern "C" fn(*mut c_void);
type SetFrameRate = unsafe extern "C" fn(*mut c_void, f32, i8) -> i32;

/// The functions libandroid has (none: this Android hasn't them).
struct Api {
    hints: Option<(GetManager, CreateSession, UpdateTarget, ReportActual, CloseSession)>,
    set_frame_rate: Option<SetFrameRate>,
}

/// `name` in libandroid, as a function of type `F` (none: not there).
///
/// # Safety
/// `F` must be the function's type as the NDK declares it.
unsafe fn symbol<F: Copy>(library: *mut c_void, name: &CStr) -> Option<F> {
    // SAFETY: a lookup in a library dlopen kept open; the caller says the type.
    unsafe {
        let f = libc::dlsym(library, name.as_ptr());
        (!f.is_null()).then(|| std::mem::transmute_copy::<*mut c_void, F>(&f))
    }
}

fn api() -> &'static Api {
    static API: OnceLock<Api> = OnceLock::new();
    API.get_or_init(|| {
        // SAFETY: libandroid is the system's (always loaded in an app); the handle is never closed. Each symbol's
        // type is the NDK's declaration of it.
        unsafe {
            let library = libc::dlopen(c"libandroid.so".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if library.is_null() {
                return Api { hints: None, set_frame_rate: None };
            }
            let hints = (|| {
                Some((
                    symbol(library, c"APerformanceHint_getManager")?,
                    symbol(library, c"APerformanceHint_createSession")?,
                    symbol(library, c"APerformanceHint_updateTargetWorkDuration")?,
                    symbol(library, c"APerformanceHint_reportActualWorkDuration")?,
                    symbol(library, c"APerformanceHint_closeSession")?,
                ))
            })();
            Api { hints, set_frame_rate: symbol(library, c"ANativeWindow_setFrameRate") }
        }
    })
}

/// An ADPF session for the UI thread (none where the system has none: before Android 13, or a device without
/// performance hints), closed as it drops.
pub struct Hints {
    session: *mut c_void,
    target: i64,
}

impl Hints {
    /// A session for this thread, its frames' target `target_ns`.
    pub fn new(target_ns: i64) -> Option<Hints> {
        let (get_manager, create, ..) = api().hints?;
        if target_ns <= 0 {
            return None;
        }
        // SAFETY: the NDK's functions; the thread id lives through the call (the session copies it).
        let session = unsafe {
            let manager = get_manager();
            if manager.is_null() {
                return None;
            }
            let tid = libc::gettid();
            create(manager, &tid, 1, target_ns)
        };
        (!session.is_null()).then_some(Hints { session, target: target_ns })
    }

    /// The frames' target from now on (the display's beat changed).
    pub fn set_target(&mut self, target_ns: i64) {
        let Some((_, _, update, ..)) = api().hints else { return };
        if target_ns > 0 && target_ns != self.target {
            // SAFETY: our open session.
            unsafe { update(self.session, target_ns) };
            self.target = target_ns;
        }
    }

    /// A frame's own work took `ns`.
    pub fn report(&self, ns: i64) {
        let Some((_, _, _, report, _)) = api().hints else { return };
        if ns > 0 {
            // SAFETY: our open session.
            unsafe { report(self.session, ns) };
        }
    }
}

impl Drop for Hints {
    fn drop(&mut self) {
        if let Some((.., close)) = api().hints {
            // SAFETY: our session, closed once.
            unsafe { close(self.session) };
        }
    }
}

/// `ANATIVEWINDOW_FRAME_RATE_COMPATIBILITY_DEFAULT`: the app's content as it is (not a video's fixed rate).
const COMPATIBILITY_DEFAULT: i8 = 0;

/// The refresh rate the app's content wants on the native window `window` (its address; Hz, 0: no vote), where
/// Android has the call (11); whether it was told.
pub fn set_frame_rate(window: usize, hz: f32) -> bool {
    let Some(set) = api().set_frame_rate else { return false };
    if window == 0 || !hz.is_finite() || hz < 0.0 {
        return false;
    }
    // SAFETY: the view's window, kept alive by the app while it is the view's (the platform's `window`).
    unsafe { set(window as *mut c_void, hz, COMPATIBILITY_DEFAULT) == 0 }
}

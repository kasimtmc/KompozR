//! The NDK's C functions KompozR calls (our own declarations, as the survey's D4 chose: no `ndk-sys`), all in the
//! system's libandroid.so and liblog.so at API 29 (the app's minSdk: none needs a `dlsym`), and safe wrappers of
//! them: the view's native window ([`NativeWindow`]), the UI thread's looper and the descriptors it watches
//! ([`Looper`]), the display's frame callbacks ([`post_frame_callback`]), the APK's assets ([`Assets`]) and logcat
//! ([`log_write`]).
//!
//! An FFI module: its `unsafe` is here, each block saying why it holds.

#![allow(unsafe_code)]

use super::jni::{Env, JNIEnv, JObject, jobject};
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::os::fd::RawFd;

/// The NDK's opaque types.
#[repr(C)]
pub struct ANativeWindow {
    _private: [u8; 0],
}
#[repr(C)]
pub struct ALooper {
    _private: [u8; 0],
}
#[repr(C)]
pub struct AAssetManager {
    _private: [u8; 0],
}
#[repr(C)]
pub struct AAsset {
    _private: [u8; 0],
}
#[repr(C)]
pub struct AChoreographer {
    _private: [u8; 0],
}

/// What the looper calls when a watched descriptor is ready (1: keep watching it).
pub type ALooperCallback = unsafe extern "C" fn(fd: c_int, events: c_int, data: *mut c_void) -> c_int;
/// What the choreographer calls on the display's next beat, with the frame's time (CLOCK_MONOTONIC, ns).
pub type AChoreographerFrameCallback64 = unsafe extern "C" fn(frame_time_nanos: i64, data: *mut c_void);

const ALOOPER_POLL_CALLBACK: c_int = -2;
const ALOOPER_EVENT_INPUT: c_int = 1;
const AASSET_MODE_BUFFER: c_int = 3;
pub const ANDROID_LOG_INFO: c_int = 4;
pub const ANDROID_LOG_ERROR: c_int = 6;

#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_fromSurface(env: *mut JNIEnv, surface: jobject) -> *mut ANativeWindow;
    fn ANativeWindow_release(window: *mut ANativeWindow);
    fn ALooper_forThread() -> *mut ALooper;
    fn ALooper_acquire(looper: *mut ALooper);
    fn ALooper_release(looper: *mut ALooper);
    fn ALooper_addFd(
        looper: *mut ALooper,
        fd: c_int,
        ident: c_int,
        events: c_int,
        callback: Option<ALooperCallback>,
        data: *mut c_void,
    ) -> c_int;
    fn ALooper_removeFd(looper: *mut ALooper, fd: c_int) -> c_int;
    fn AAssetManager_fromJava(env: *mut JNIEnv, asset_manager: jobject) -> *mut AAssetManager;
    fn AAssetManager_open(manager: *mut AAssetManager, filename: *const c_char, mode: c_int) -> *mut AAsset;
    fn AAsset_getBuffer(asset: *mut AAsset) -> *const c_void;
    fn AAsset_getLength64(asset: *mut AAsset) -> i64;
    fn AAsset_close(asset: *mut AAsset);
    fn AChoreographer_getInstance() -> *mut AChoreographer;
    fn AChoreographer_postFrameCallback64(
        choreographer: *mut AChoreographer,
        callback: AChoreographerFrameCallback64,
        data: *mut c_void,
    );
}

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(priority: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

/// A line into logcat under `tag` (a NUL in it ends it there).
pub fn log_write(priority: c_int, tag: &CStr, text: &str) {
    let text = CString::new(text.split('\0').next().unwrap_or("")).unwrap_or_default();
    // SAFETY: both strings are NUL-terminated and live through the call; liblog copies them.
    unsafe {
        __android_log_write(priority, tag.as_ptr(), text.as_ptr());
    }
}

/// A view's native window (a reference of ours on it, let go as it drops): what Vulkan presents to.
pub struct NativeWindow(*mut ANativeWindow);

impl NativeWindow {
    /// The native window of `surface` (an `android.view.Surface`), a reference of ours; none for null or a surface
    /// already released.
    pub fn from_surface(env: &Env<'_>, surface: JObject) -> Option<NativeWindow> {
        if surface.is_null() {
            return None;
        }
        // SAFETY: a live Surface reference of the current native call's env; the window comes with a reference
        // of ours (released in drop).
        let window = unsafe { ANativeWindow_fromSurface(env.raw(), surface.raw()) };
        (!window.is_null()).then_some(NativeWindow(window))
    }

    /// Its address: what the GPU device makes its surface for, and tells windows apart by.
    pub fn address(&self) -> usize {
        self.0 as usize
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        // SAFETY: our reference, released once (after the GPU device let go of its surface: the platform's order).
        unsafe { ANativeWindow_release(self.0) }
    }
}

/// The UI thread's looper and the descriptors it watches for KompozR (a wake's eventfd, a timer's timerfd), each
/// calling `callback` as it becomes readable; removed and let go as it drops.
pub struct Looper {
    looper: *mut ALooper,
    fds: Vec<RawFd>,
}

impl Looper {
    /// The calling thread's looper (the UI thread has one: Android's main Looper); none on a thread without one.
    pub fn for_thread() -> Option<Looper> {
        // SAFETY: ALooper_forThread only reads the thread's looper; acquired so it outlives us.
        unsafe {
            let looper = ALooper_forThread();
            if looper.is_null() {
                return None;
            }
            ALooper_acquire(looper);
            Some(Looper { looper, fds: Vec::new() })
        }
    }

    /// `fd` watched: `callback` called on this thread when it can be read (it must read it, or be called again at
    /// once), with `data`. False if the looper refused it.
    pub fn watch(&mut self, fd: RawFd, callback: ALooperCallback, data: usize) -> bool {
        // SAFETY: the looper is alive (acquired); fd stays open while watched (its owner outlives this Looper:
        // the platform drops the Looper first); the callback is an extern "C" function of the looper's type.
        let added = unsafe {
            ALooper_addFd(self.looper, fd, ALOOPER_POLL_CALLBACK, ALOOPER_EVENT_INPUT, Some(callback), data as *mut _)
        };
        if added == 1 {
            self.fds.push(fd);
        }
        added == 1
    }
}

impl Drop for Looper {
    fn drop(&mut self) {
        // SAFETY: the descriptors this Looper added, on the looper still acquired, then the looper let go.
        unsafe {
            for &fd in &self.fds {
                ALooper_removeFd(self.looper, fd);
            }
            ALooper_release(self.looper);
        }
    }
}

/// `callback` called with `data` on the display's next beat (once), on this thread (its looper's: the UI thread).
/// False on a thread without a choreographer.
pub fn post_frame_callback(callback: AChoreographerFrameCallback64, data: usize) -> bool {
    // SAFETY: the thread's choreographer (none: nothing posted); the callback an extern "C" function of its type,
    // data a plain number (never dereferenced).
    unsafe {
        let choreographer = AChoreographer_getInstance();
        if choreographer.is_null() {
            return false;
        }
        AChoreographer_postFrameCallback64(choreographer, callback, data as *mut _);
    }
    true
}

/// The APK's assets (an `AAssetManager` of the app's `AssetManager`, which the platform keeps a global reference to
/// for as long as this lives).
pub struct Assets(*mut AAssetManager);

// SAFETY: the NDK's AAssetManager is thread-safe ("AAssetManager ... can be used from multiple threads"); each read
// opens and closes an AAsset of its own on the calling thread.
unsafe impl Send for Assets {}
// SAFETY: as above.
unsafe impl Sync for Assets {}

impl Assets {
    /// The asset manager of `manager` (an `android.content.res.AssetManager`); none for null.
    pub fn from_java(env: &Env<'_>, manager: JObject) -> Option<Assets> {
        if manager.is_null() {
            return None;
        }
        // SAFETY: a live AssetManager reference; the native manager lives as long as the Java one (kept by the
        // platform's global reference).
        let assets = unsafe { AAssetManager_fromJava(env.raw(), manager.raw()) };
        (!assets.is_null()).then_some(Assets(assets))
    }

    /// The asset at `path` (relative to the APK's `assets/`), its bytes; none if there is none.
    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        let name = CString::new(path.trim_start_matches('/')).ok()?;
        // SAFETY: the manager is alive (see Assets); the asset opened here is read and closed on this thread.
        unsafe {
            let asset = AAssetManager_open(self.0, name.as_ptr(), AASSET_MODE_BUFFER);
            if asset.is_null() {
                return None;
            }
            let data = AAsset_getBuffer(asset).cast::<u8>();
            let length = AAsset_getLength64(asset);
            let bytes =
                (!data.is_null() && length >= 0).then(|| std::slice::from_raw_parts(data, length as usize).to_vec());
            AAsset_close(asset);
            bytes
        }
    }
}

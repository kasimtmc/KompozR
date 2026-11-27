//! Where Java and the system call KompozR (C++'s `Java_dev_kompoz_*` exports and its looper and choreographer
//! callbacks): `JNI_OnLoad`'s work ([`on_load`]: the natives registered with `RegisterNatives`, the log and the panic
//! hook set), KompozActivity's and KompozView's native methods, the looper's wake and timer, the choreographer's
//! frames. Each catches a panic before it would unwind into Java or the system (which aborts the process without a
//! word): logged by the hook, the app let go and its activity finished.
//!
//! An FFI module: what Java passes is raw (`JNIEnv *`, references); each `unsafe` block says why it holds.

#![allow(unsafe_code)]

use super::app::{self, Created, ENTRY, Running, VM, view_motion, with_activity, with_generation, with_view};
use super::configuration::Configuration;
use super::input::Motion;
use super::jni::{Env, JNI_VERSION_1_6, JNIEnv, JObject, Local, Vm, jboolean, jint, jlong, native};
use super::{AndroidApp, log_error, log_line};
use crate::unix::sys;
use std::ffi::{CStr, c_int, c_void};
use std::os::fd::BorrowedFd;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

/// `f`, a panic in it caught (`what` names the entry in the log): the app let go and its activity finished, and
/// `fallback` returned to the caller.
fn guard<R>(what: &str, fallback: R, f: impl FnOnce() -> R) -> R {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(_) => {
            log_error(&format!("kompoz: a panic in {what}: the app ends"));
            let _ = catch_unwind(app::failed);
            fallback
        }
    }
}

/// The env of a native call.
///
/// # Safety
/// `env` must be the `JNIEnv *` the VM passed to the native method being run.
unsafe fn env<'a>(env: *mut JNIEnv) -> Env<'a> {
    // SAFETY: (the caller's) the current call's env.
    unsafe { Env::from_raw(env) }
}

/// A path Java passed as UTF-8 bytes (none: null or empty).
fn path(env: &Env<'_>, bytes: JObject) -> Option<PathBuf> {
    env.utf8(bytes).filter(|p| !p.is_empty()).map(PathBuf::from)
}

/// `JNI_OnLoad` (`android_app!`'s): the log to logcat, the panic hook, the VM and the app's entry kept, the natives
/// of KompozActivity and KompozView registered (from this thread, the UI thread loading the library, whose class
/// loader is the app's). JNI's version, or -1 (the library refused: `System.loadLibrary` throws) if a class or a
/// method isn't there.
pub fn on_load(vm: Vm, entry: fn() -> AndroidApp) -> jint {
    guard("JNI_OnLoad", -1, || {
        crate::system::set_log_sink(Some(log_line));
        std::panic::set_hook(Box::new(|info| {
            log_error(&format!("kompoz: {info}\n{}", std::backtrace::Backtrace::force_capture()));
        }));
        let _ = VM.set(vm);
        let _ = ENTRY.set(entry);
        let Some(env) = vm.env() else {
            log_error("kompoz: JNI_OnLoad without an env");
            return -1;
        };
        for (class, methods) in natives() {
            let registered = env.find_class(class).is_some_and(|c: Local<'_>| env.register_natives(&c, &methods));
            if !registered {
                log_error(&format!("kompoz: the natives of {} couldn't be registered", class.to_string_lossy()));
                return -1;
            }
        }
        JNI_VERSION_1_6
    })
}

/// Each class's native methods, by name and JNI signature (KompozActivity.kt's and KompozView.kt's `external fun`s).
fn natives() -> Vec<(&'static CStr, Vec<super::jni::JNINativeMethod>)> {
    vec![
        (
            c"dev/kompozr/KompozActivity",
            vec![
                native(
                    c"nativeCreate",
                    c"(Ldev/kompozr/KompozView;Landroid/content/res/AssetManager;[B[B[B[B)V",
                    native_create as *mut c_void,
                ),
                native(c"nativeConfiguration", c"([F[B[B)V", native_configuration as *mut c_void),
                native(c"nativeLifecycle", c"(I)V", native_lifecycle as *mut c_void),
                native(c"nativeSaveState", c"()[B", native_save_state as *mut c_void),
                native(c"nativeDestroy", c"()V", native_destroy as *mut c_void),
            ],
        ),
        (
            c"dev/kompozr/KompozView",
            vec![
                native(c"nativeSurface", c"(Landroid/view/Surface;II)V", native_surface as *mut c_void),
                native(c"nativeMotion", c"([I[I[I[F[F[F[J[FJ)V", native_motion as *mut c_void),
                native(c"nativeKey", c"(IZI[BJ)Z", native_key as *mut c_void),
                native(c"nativeDevices", c"([I[F)V", native_devices as *mut c_void),
                native(c"nativeImeEdit", c"(I[I[B[B[I)V", native_ime_edit as *mut c_void),
                native(c"nativeImeAction", c"(II)V", native_ime_action as *mut c_void),
                native(c"nativeImeContent", c"(I[B[B[B)Z", native_ime_content as *mut c_void),
                native(c"nativeToolbarAction", c"(I)V", native_toolbar_action as *mut c_void),
            ],
        ),
    ]
}

// ---- KompozActivity ----

/// `nativeCreate(view, assets, filesDir, externalFilesDir, cacheDir, savedState)`: the activity's app made.
#[allow(clippy::too_many_arguments)]
extern "system" fn native_create(
    raw: *mut JNIEnv,
    activity: JObject,
    view: JObject,
    assets: JObject,
    files: JObject,
    external_files: JObject,
    cache: JObject,
    saved: JObject,
) {
    guard("nativeCreate", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let Some(files_dir) = path(&env, files) else { return };
        let cache_dir = path(&env, cache).unwrap_or_else(|| files_dir.join("cache"));
        let created = Created {
            activity,
            view,
            assets,
            external_files_dir: path(&env, external_files),
            cache_dir,
            saved: env.bytes(saved),
            files_dir,
            env,
        };
        if Running::create(created).is_none() {
            log_error("kompoz: the app couldn't be made");
        }
    })
}

/// `nativeConfiguration(values, languages, timeZone)`: the display's and the system's configuration, as it starts
/// and as it changes.
extern "system" fn native_configuration(
    raw: *mut JNIEnv,
    activity: JObject,
    values: JObject,
    languages: JObject,
    time_zone: JObject,
) {
    guard("nativeConfiguration", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let configuration = Configuration::from_java(
            &env.floats(values),
            &env.utf8(languages).unwrap_or_default(),
            &env.utf8(time_zone).unwrap_or_default(),
        );
        with_activity(&env, activity, |app| app.configure(configuration));
    })
}

/// `nativeLifecycle(event)`: KompozActivity's `LIFECYCLE_*`.
extern "system" fn native_lifecycle(raw: *mut JNIEnv, activity: JObject, event: jint) {
    guard("nativeLifecycle", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        with_activity(&env, activity, |app| app.lifecycle(event));
    })
}

/// `nativeSaveState(): ByteArray?`: what goes into the Bundle (null: nothing).
extern "system" fn native_save_state(raw: *mut JNIEnv, activity: JObject) -> JObject {
    guard("nativeSaveState", JObject::null(), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let Some(Some(saved)) = with_activity(&env, activity, |app| app.save_state()) else {
            return JObject::null();
        };
        // (The local reference goes back to Java, which deletes it.)
        env.new_bytes(&saved).map_or(JObject::null(), |array| array.into_object())
    })
}

/// `nativeDestroy()`: the activity's app gone.
extern "system" fn native_destroy(raw: *mut JNIEnv, activity: JObject) {
    guard("nativeDestroy", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        app::destroy_activity(&env, activity);
    })
}

// ---- KompozView ----

/// `nativeSurface(surface, width, height)`: the view's surface came, changed size or went (null). The old window is
/// let go before this returns (surfaceDestroyed's contract).
extern "system" fn native_surface(raw: *mut JNIEnv, view: JObject, surface: JObject, width: jint, height: jint) {
    guard("nativeSurface", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        with_view(&env, view, |app| app.surface(&env, surface, width, height));
    })
}

/// `nativeMotion(state, ids, tools, xy, pressure, axes, historyTimes, historyXY, timeNanos)`: KompozView's
/// `motion` (its `state`: the masked action, the action's pointer index, the action button, the button state, the
/// meta state, the classification; its `axes`: the wheel's two, a touchpad swipe's distances, its pinch's scale,
/// the density).
#[allow(clippy::too_many_arguments)]
extern "system" fn native_motion(
    raw: *mut JNIEnv,
    view: JObject,
    state: JObject,
    ids: JObject,
    tools: JObject,
    xy: JObject,
    pressure: JObject,
    axes: JObject,
    history_times: JObject,
    history_xy: JObject,
    time: jlong,
) {
    guard("nativeMotion", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let state = env.ints(state);
        let axes = env.floats(axes);
        let int = |i: usize| state.get(i).copied().unwrap_or(0);
        let axis = |i: usize, default: f32| axes.get(i).copied().filter(|v| v.is_finite()).unwrap_or(default);
        let motion = Motion {
            action: int(0),
            index: int(1).max(0) as usize,
            button: int(2),
            buttons: int(3),
            meta: int(4),
            classification: int(5),
            ids: env.ints(ids),
            tools: env.ints(tools),
            xy: env.floats(xy),
            pressure: env.floats(pressure),
            scroll_x: axis(0, 0.0),
            scroll_y: axis(1, 0.0),
            gesture_scroll_x: axis(2, 0.0),
            gesture_scroll_y: axis(3, 0.0),
            pinch_scale: axis(4, 1.0),
            density: axis(5, 1.0),
            history_times: env.longs(history_times),
            history_xy: env.floats(history_xy),
            time,
        };
        view_motion(&env, view, &motion);
    })
}

/// `nativeKey(keyCode, down, meta, typed, timeNanos): Boolean`: whether KompozR took the key.
extern "system" fn native_key(
    raw: *mut JNIEnv,
    view: JObject,
    code: jint,
    down: jboolean,
    meta: jint,
    typed: JObject,
    time: jlong,
) -> jboolean {
    guard("nativeKey", 0, || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let typed = env.utf8(typed).unwrap_or_default();
        let taken = with_view(&env, view, |app| app.key(code, down != 0, meta, &typed, time));
        taken.unwrap_or(false) as jboolean
    })
}

/// `nativeDevices(mediaInfo, viewConfiguration)`: KompozInputDevices' (as the view is attached, as devices come
/// and go, as the configuration changes).
extern "system" fn native_devices(raw: *mut JNIEnv, view: JObject, media: JObject, configuration: JObject) {
    guard("nativeDevices", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let (media, configuration) = (env.ints(media), env.floats(configuration));
        with_view(&env, view, |app| app.devices(&media, &configuration));
    })
}

/// `nativeImeEdit(session, ops, texts, text, offsets)`: a batch of KompozInputConnection's edits and what they left
/// in its Editable.
extern "system" fn native_ime_edit(
    raw: *mut JNIEnv,
    view: JObject,
    session: jint,
    ops: JObject,
    texts: JObject,
    text: JObject,
    offsets: JObject,
) {
    guard("nativeImeEdit", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let (ops, texts) = (env.ints(ops), env.bytes(texts).unwrap_or_default());
        let (text, offsets) = (env.utf8(text).unwrap_or_default(), env.ints(offsets));
        with_view(&env, view, |app| app.ime_edit(session, &ops, &texts, text, &offsets));
    })
}

/// `nativeImeAction(session, editorAction)`: the input method's action key.
extern "system" fn native_ime_action(raw: *mut JNIEnv, view: JObject, session: jint, action: jint) {
    guard("nativeImeAction", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        with_view(&env, view, |app| app.ime_action(session, action));
    })
}

/// `nativeImeContent(session, mimeTypes, uri, linkUri): Boolean`: content the keyboard commits (answered at once:
/// the keyboard waits; false while the app is busy).
extern "system" fn native_ime_content(
    raw: *mut JNIEnv,
    view: JObject,
    session: jint,
    mime_types: JObject,
    uri: JObject,
    link: JObject,
) -> jboolean {
    guard("nativeImeContent", 0, || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let mime_types = env.utf8(mime_types).unwrap_or_default();
        let Some(uri) = env.utf8(uri) else { return 0 };
        let link = env.utf8(link);
        let taken = with_view(&env, view, |app| app.ime_content(session, &mime_types, uri, link));
        taken.unwrap_or(false) as jboolean
    })
}

/// `nativeToolbarAction(item)`: a text toolbar item clicked (KompozTextToolbar's `ITEM_*`).
extern "system" fn native_toolbar_action(raw: *mut JNIEnv, view: JObject, item: jint) {
    guard("nativeToolbarAction", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        with_view(&env, view, |app| app.toolbar_action(item));
    })
}

// ---- The looper's and the choreographer's callbacks ----

/// Reads (empties) the counter of the eventfd or timerfd `fd`.
fn drain(fd: c_int) {
    // SAFETY: fd is one of the app's descriptors, open while the looper watches it (Running drops its watches
    // first); borrowed for this read only.
    let fd = unsafe { BorrowedFd::borrow_raw(fd) };
    let _ = sys::eventfd_read(fd);
}

/// The host's waker (work from another thread): a frame.
pub(super) unsafe extern "C" fn on_wake(fd: c_int, _events: c_int, data: *mut c_void) -> c_int {
    guard("the looper's wake", 1, || {
        drain(fd);
        with_generation(data as usize, |app| app.woken());
        1
    })
}

/// A coroutine's delay due: a frame.
pub(super) unsafe extern "C" fn on_timer(fd: c_int, _events: c_int, data: *mut c_void) -> c_int {
    guard("the looper's timer", 1, || {
        drain(fd);
        with_generation(data as usize, |app| app.woken());
        1
    })
}

/// The display's beat: the frame posted for the app of `data`'s generation, at the vsync's time.
pub(super) unsafe extern "C" fn on_frame(frame_time_nanos: i64, data: *mut c_void) {
    guard("a frame", (), || {
        with_generation(data as usize, |app| app.frame(frame_time_nanos));
    })
}

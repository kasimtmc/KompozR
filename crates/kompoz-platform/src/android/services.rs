//! The system's services as the app's side calls them and hears from them (C++'s KompozInput statics and their
//! natives): the Kotlin objects KompozActivity makes beside its view — KompozInsets (insets, the bars' icons, the
//! keyboard moved by hand), KompozBack (back's callback registered while the app takes back, the system's back when
//! it doesn't), KompozServices (haptics, URIs, the screen kept on, sensitive content, refresh rate votes, gesture
//! exclusion, the system's palettes, the animator scale), KompozClipboard and KompozDragAndDrop — given to the app
//! by `nativeServices` and called on the UI thread; and their natives, which hand what they tell to the app
//! (`super::app`'s Running): the insets as they change and move, back's steps, deep links, drags over the view, the
//! wallpaper's colours and the animator scale changing.
//!
//! An FFI module: what Java passes is raw; each `unsafe` block says why it holds.

#![allow(unsafe_code)]

use super::app::{Running, with_activity, with_view};
use super::clip::{Description, decode_description, encode_entry};
use super::drag::DragEvent;
use super::jni::{
    Env, Global, JNIEnv, JNINativeMethod, JObject, Local, MethodId, Vm, jboolean, jfloat, jint, jlong, jvalue, native,
};
use super::natives::{env, guard};
use super::system::{back_phase, haptic_index};
use super::{insets, log_error};
use kompoz_graphics::IntRect;
use kompoz_ui::clipboard::{ClipDataReader, ClipEntry, ClipItem};
use kompoz_ui::draganddrop::PlatformDragAndDrop;
use kompoz_ui::platform::HapticFeedbackType;
use std::ffi::{CStr, c_void};
use std::rc::Rc;

/// The Kotlin objects and the methods called on them.
pub(super) struct Services {
    vm: Vm,
    insets: Global,
    back: Global,
    services: Global,
    clipboard: Global,
    drag: Global,
    system_bars_appearance: MethodId,
    can_control_ime: MethodId,
    ime_control_by: MethodId,
    ime_control_end: MethodId,
    back_enabled: MethodId,
    back_leave: MethodId,
    haptic: MethodId,
    open_uri: MethodId,
    keep_screen_on: MethodId,
    content_sensitive: MethodId,
    frame_rate: MethodId,
    gesture_exclusion: MethodId,
    dynamic_colors: MethodId,
    wallpaper_colors: MethodId,
    animator_scale: MethodId,
    clip_description: MethodId,
    clip_read: MethodId,
    clip_set: MethodId,
    drag_start: MethodId,
    drag_read: MethodId,
}

impl Services {
    /// The objects `nativeServices` gave, their methods found (none: one isn't there; logged).
    fn new(env: &Env<'_>, objects: [JObject; 5]) -> Option<Services> {
        let [insets, back, services, clipboard, drag] = objects;
        let class = |object: JObject| env.class_of(object);
        let (ci, cb, cs, cc, cd) = (class(insets)?, class(back)?, class(services)?, class(clipboard)?, class(drag)?);
        let m = |class: &Local<'_>, name: &CStr, signature: &CStr| {
            let found = env.method(class, name, signature);
            if found.is_none() {
                log_error(&format!("kompoz: the method {} isn't there", name.to_string_lossy()));
            }
            found
        };
        Some(Services {
            vm: env.vm()?,
            system_bars_appearance: m(&ci, c"setSystemBarsAppearance", c"(ZZ)V")?,
            can_control_ime: m(&ci, c"canControlIme", c"()Z")?,
            ime_control_by: m(&ci, c"imeControlBy", c"(F)V")?,
            ime_control_end: m(&ci, c"imeControlEnd", c"(F)V")?,
            back_enabled: m(&cb, c"setEnabled", c"(Z)V")?,
            back_leave: m(&cb, c"leave", c"()V")?,
            haptic: m(&cs, c"haptic", c"(I)V")?,
            open_uri: m(&cs, c"openUri", c"([B)Z")?,
            keep_screen_on: m(&cs, c"setKeepScreenOn", c"(Z)V")?,
            content_sensitive: m(&cs, c"setContentSensitive", c"(Z)V")?,
            frame_rate: m(&cs, c"setFrameRate", c"(FI)V")?,
            gesture_exclusion: m(&cs, c"setGestureExclusion", c"([I)V")?,
            dynamic_colors: m(&cs, c"dynamicColors", c"()[I")?,
            wallpaper_colors: m(&cs, c"wallpaperColors", c"()[I")?,
            animator_scale: m(&cs, c"animatorScale", c"()F")?,
            clip_description: m(&cc, c"description", c"()[B")?,
            clip_read: m(&cc, c"read", c"([BJ)[B")?,
            clip_set: m(&cc, c"set", c"([B)V")?,
            drag_start: m(&cd, c"start", c"([B[BIIII)Z")?,
            drag_read: m(&cd, c"read", c"([B)[B")?,
            insets: env.new_global(insets)?,
            back: env.new_global(back)?,
            services: env.new_global(services)?,
            clipboard: env.new_global(clipboard)?,
            drag: env.new_global(drag)?,
        })
    }

    /// `f` with the UI thread's env (the app calls these there).
    fn with<R>(&self, f: impl FnOnce(&Env<'static>) -> Option<R>) -> Option<R> {
        self.vm.env().and_then(|env| f(&env))
    }

    fn call(&self, object: &Global, method: MethodId, args: &[jvalue]) {
        self.with(|env| env.call_void(object.object(), method, args).then_some(()));
    }

    /// `object.method(args)`'s byte array (none: null).
    fn bytes(&self, object: &Global, method: MethodId, args: &[jvalue]) -> Option<Vec<u8>> {
        self.with(|env| env.bytes(env.call_object(object.object(), method, args)?.object()))
    }

    /// `object.method(args)`'s int array (none: null).
    fn ints(&self, object: &Global, method: MethodId) -> Option<Vec<i32>> {
        self.with(|env| Some(env.ints(env.call_object(object.object(), method, &[])?.object())))
    }

    // ---- KompozInsets ----

    pub fn system_bars_appearance(&self, light_status_bars: bool, light_navigation_bars: bool) {
        let args = [jvalue::from(light_status_bars), jvalue::from(light_navigation_bars)];
        self.call(&self.insets, self.system_bars_appearance, &args);
    }

    pub fn can_control_ime(&self) -> bool {
        self.with(|env| Some(env.call_boolean(self.insets.object(), self.can_control_ime, &[]))).unwrap_or(false)
    }

    pub fn ime_control_by(&self, px: f32) {
        self.call(&self.insets, self.ime_control_by, &[jvalue::from(px)]);
    }

    pub fn ime_control_end(&self, velocity: f32) {
        self.call(&self.insets, self.ime_control_end, &[jvalue::from(velocity)]);
    }

    // ---- KompozBack ----

    /// The app takes back now (its callback registered: Android 13+), or no longer.
    pub fn set_back_enabled(&self, enabled: bool) {
        self.call(&self.back, self.back_enabled, &[jvalue::from(enabled)]);
    }

    /// Back nothing in the app took: the system's own.
    pub fn leave(&self) {
        self.call(&self.back, self.back_leave, &[]);
    }

    // ---- KompozServices ----

    pub fn haptic(&self, kind: HapticFeedbackType) {
        self.call(&self.services, self.haptic, &[jvalue::from(haptic_index(kind))]);
    }

    pub fn open_uri(&self, uri: &str) {
        let opened = self.with(|env| {
            let uri = env.new_bytes(uri.as_bytes())?;
            Some(env.call_boolean(self.services.object(), self.open_uri, &[jvalue::from(uri.object())]))
        });
        if opened != Some(true) {
            super::log_line(&format!("kompoz: nothing opens {uri}"));
        }
    }

    pub fn keep_screen_on(&self, on: bool) {
        self.call(&self.services, self.keep_screen_on, &[jvalue::from(on)]);
    }

    pub fn content_sensitive(&self, sensitive: bool) {
        self.call(&self.services, self.content_sensitive, &[jvalue::from(sensitive)]);
    }

    /// The view's vote (Android 15): a rate, else a category (`system::category_index`'s).
    pub fn frame_rate(&self, rate: f32, category: i32) {
        self.call(&self.services, self.frame_rate, &[jvalue::from(rate), jvalue::from(category)]);
    }

    pub fn gesture_exclusion(&self, rects: &[IntRect]) {
        let ints: Vec<i32> = rects.iter().flat_map(|r| [r.left, r.top, r.right, r.bottom]).collect();
        self.with(|env| {
            let array = env.new_ints(&ints)?;
            env.call_void(self.services.object(), self.gesture_exclusion, &[jvalue::from(array.object())]).then_some(())
        });
    }

    /// Android 12's 65 system colours (none before 12).
    pub fn dynamic_colors(&self) -> Option<Vec<i32>> {
        self.ints(&self.services, self.dynamic_colors)
    }

    /// The wallpaper's three colours (none: none).
    pub fn wallpaper_colors(&self) -> Option<Vec<i32>> {
        self.ints(&self.services, self.wallpaper_colors)
    }

    /// The animator duration scale now.
    pub fn animator_scale(&self) -> f32 {
        self.with(|env| env.call_float(self.services.object(), self.animator_scale, &[])).unwrap_or(1.0)
    }

    // ---- KompozClipboard ----

    /// What the clipboard holds: described now (not read: Android tells the user of a read), its items read as the
    /// app asks for a type, if it still holds the same clip then.
    pub fn clip_entry(self: &Rc<Self>) -> Option<ClipEntry> {
        let description = decode_description(&self.bytes(&self.clipboard, self.clip_description, &[])?)?;
        let reader = Rc::new(ClipboardReader { services: self.clone(), timestamp: description.timestamp });
        Some(entry(description, reader))
    }

    /// The app's entry put on the clipboard (none: emptied).
    pub fn set_clip_entry(&self, entry: Option<&ClipEntry>) {
        let bytes = entry.map(encode_entry);
        self.with(|env| {
            let array = match &bytes {
                Some(b) => Some(env.new_bytes(b)?),
                None => None,
            };
            let argument = array.as_ref().map_or(JObject::null(), Local::object);
            env.call_void(self.clipboard.object(), self.clip_set, &[jvalue::from(argument)]).then_some(())
        });
    }

    // ---- KompozDragAndDrop ----

    /// A drop's entry: described by its ClipDescription, its items read from its ClipData as the app asks (until
    /// the drop is done).
    pub fn drop_entry(self: &Rc<Self>, description: Description) -> ClipEntry {
        entry(description, Rc::new(DropReader { services: self.clone() }))
    }

    /// The app's drag that left its window started as the system's, shown by its decoration (rendered by the host,
    /// where it draws offscreen) with the pointer where it is in it; whether the system took it.
    pub fn start_drag(&self, drag: &PlatformDragAndDrop) -> bool {
        let entry = encode_entry(&drag.transfer_data.clip_entry);
        let icon = drag.render_icon(1.0);
        let (width, height) = icon.as_ref().map_or((0, 0), |i| (i.width(), i.height()));
        let pixels = icon.map(|i| premultiplied(&i.pixels()));
        let (x, y) = (drag.decoration_offset.x.round() as i32, drag.decoration_offset.y.round() as i32);
        self.with(|env| {
            let entry = env.new_bytes(&entry)?;
            let pixels = match &pixels {
                Some(p) => Some(env.new_bytes(p)?),
                None => None,
            };
            let args = [
                jvalue::from(entry.object()),
                jvalue::from(pixels.as_ref().map_or(JObject::null(), Local::object)),
                jvalue::from(width),
                jvalue::from(height),
                jvalue::from(x),
                jvalue::from(y),
            ];
            Some(env.call_boolean(self.drag.object(), self.drag_start, &args))
        })
        .unwrap_or(false)
    }
}

/// An entry of another app's clip: one item of the types it is described by, read through `reader`.
fn entry(description: Description, reader: Rc<dyn ClipDataReader>) -> ClipEntry {
    let types = description.metadata.mime_types().to_vec();
    ClipEntry::with_metadata(vec![ClipItem::from_reader(types, reader)], description.metadata)
}

/// Straight RGBA premultiplied (what an Android bitmap holds).
fn premultiplied(rgba: &[u8]) -> Vec<u8> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = p[3] as u32;
            let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
            [m(p[0]), m(p[1]), m(p[2]), p[3]]
        })
        .collect()
}

/// Reads the clipboard's clip as the app asks for a type, if it is still the one described.
struct ClipboardReader {
    services: Rc<Services>,
    timestamp: i64,
}

impl ClipDataReader for ClipboardReader {
    fn read(&self, mime_type: &str) -> Option<Vec<u8>> {
        let s = &self.services;
        s.with(|env| {
            let mime = env.new_bytes(mime_type.as_bytes())?;
            let args = [jvalue::from(mime.object()), jvalue::from(self.timestamp)];
            env.bytes(env.call_object(s.clipboard.object(), s.clip_read, &args)?.object())
        })
    }
}

/// Reads the drop being taken as the app asks for a type.
struct DropReader {
    services: Rc<Services>,
}

impl ClipDataReader for DropReader {
    fn read(&self, mime_type: &str) -> Option<Vec<u8>> {
        let s = &self.services;
        s.with(|env| {
            let mime = env.new_bytes(mime_type.as_bytes())?;
            env.bytes(env.call_object(s.drag.object(), s.drag_read, &[jvalue::from(mime.object())])?.object())
        })
    }
}

// ---- The natives ----

/// KompozActivity's natives of these services.
pub(super) fn activity_natives() -> Vec<JNINativeMethod> {
    vec![
        native(
            c"nativeServices",
            c"(Ldev/kompozr/KompozInsets;Ldev/kompozr/KompozBack;Ldev/kompozr/KompozServices;Ldev/kompozr/KompozClipboard;Ldev/kompozr/KompozDragAndDrop;)V",
            native_services as *mut c_void,
        ),
        native(c"nativeDeepLink", c"([B)V", native_deep_link as *mut c_void),
    ]
}

/// The services' own classes and their natives.
pub(super) fn class_natives() -> Vec<(&'static CStr, Vec<JNINativeMethod>)> {
    vec![
        (
            c"dev/kompozr/KompozInsets",
            vec![native(c"nativeInsets", c"(Ldev/kompozr/KompozView;[I[F)V", native_insets as *mut c_void)],
        ),
        (
            c"dev/kompozr/KompozBack",
            vec![native(c"nativeBack", c"(Ldev/kompozr/KompozView;IFFFIJ)V", native_back as *mut c_void)],
        ),
        (
            c"dev/kompozr/KompozServices",
            vec![
                native(c"nativeAnimatorScale", c"(Ldev/kompozr/KompozView;F)V", native_animator_scale as *mut c_void),
                native(c"nativeWallpaperChanged", c"(Ldev/kompozr/KompozView;)V", native_wallpaper as *mut c_void),
            ],
        ),
        (
            c"dev/kompozr/KompozDragAndDrop",
            vec![native(c"nativeDrag", c"(Ldev/kompozr/KompozView;IFF[BZZ)Z", native_drag as *mut c_void)],
        ),
    ]
}

/// `nativeServices(insets, back, services, clipboard, dragAndDrop)`: the activity's service objects, for its app.
extern "system" fn native_services(
    raw: *mut JNIEnv,
    activity: JObject,
    insets: JObject,
    back: JObject,
    services: JObject,
    clipboard: JObject,
    drag: JObject,
) {
    guard("nativeServices", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let Some(made) = Services::new(&env, [insets, back, services, clipboard, drag]) else {
            log_error("kompoz: the system's services aren't there: no insets, back, clipboard or drags");
            return;
        };
        with_activity(&env, activity, |app| app.set_services(Rc::new(made)));
    })
}

/// `nativeDeepLink(link)`: a link the app is opened with (its launch intent's, a new intent's).
extern "system" fn native_deep_link(raw: *mut JNIEnv, activity: JObject, link: JObject) {
    guard("nativeDeepLink", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let Some(link) = env.utf8(link).filter(|l| !l.is_empty()) else { return };
        with_activity(&env, activity, |app| app.deep_link(link));
    })
}

/// `nativeInsets(view, ints, floats)`: the insets as they are now (`super::insets`' layout).
extern "system" fn native_insets(raw: *mut JNIEnv, _this: JObject, view: JObject, ints: JObject, floats: JObject) {
    guard("nativeInsets", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let told = insets::system_insets(&env.ints(ints), &env.floats(floats));
        with_view(&env, view, |app| app.insets(told));
    })
}

/// `nativeBack(view, phase, x, y, progress, edge, frameTimeMillis)`: a step of back.
#[allow(clippy::too_many_arguments)]
extern "system" fn native_back(
    raw: *mut JNIEnv,
    _this: JObject,
    view: JObject,
    phase: jint,
    x: jfloat,
    y: jfloat,
    progress: jfloat,
    edge: jint,
    frame_time_millis: jlong,
) {
    guard("nativeBack", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let Some(step) = back_phase(phase, x, y, progress, edge, frame_time_millis) else { return };
        with_view(&env, view, |app| app.back(step));
    })
}

/// `nativeAnimatorScale(view, scale)`: the animator duration scale changed.
extern "system" fn native_animator_scale(raw: *mut JNIEnv, _this: JObject, view: JObject, scale: jfloat) {
    guard("nativeAnimatorScale", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        with_view(&env, view, |app| app.animator_scale(scale));
    })
}

/// `nativeWallpaperChanged(view)`: the wallpaper's colours (and so the system's) changed.
extern "system" fn native_wallpaper(raw: *mut JNIEnv, _this: JObject, view: JObject) {
    guard("nativeWallpaperChanged", (), || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        with_view(&env, view, Running::wallpaper_changed);
    })
}

/// `nativeDrag(view, action, x, y, description, own, result): Boolean`: a drag event over the view; for a drop,
/// whether a target took it.
#[allow(clippy::too_many_arguments)]
extern "system" fn native_drag(
    raw: *mut JNIEnv,
    _this: JObject,
    view: JObject,
    action: jint,
    x: jfloat,
    y: jfloat,
    description: JObject,
    own: jboolean,
    result: jboolean,
) -> jboolean {
    guard("nativeDrag", 0, || {
        // SAFETY: a native method's env.
        let env = unsafe { env(raw) };
        let description = env.bytes(description).and_then(|b| decode_description(&b));
        let mime_types = description.as_ref().map(|d| d.metadata.mime_types().to_vec()).unwrap_or_default();
        let event = DragEvent { action, x, y, mime_types, own: own != 0, result: result != 0 };
        with_view(&env, view, |app| app.drag(&event, description)).unwrap_or(false) as jboolean
    })
}

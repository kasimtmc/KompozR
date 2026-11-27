//! What the app reads through Java on its worker threads (C++'s `httpGet`, `readContent` and BitmapFactory's
//! `decodeImage`, its threads attached to the VM): an http(s) URL's body through HttpURLConnection (the image
//! loader's HTTP client, as libcurl is on Linux and WinHTTP on Windows), a `content://` URI's bytes through the
//! ContentResolver (what the asset reader gives for one: a keyboard's picture, a drop's, the clipboard's), and the
//! pictures our decoders lack (HEIF, AVIF, GIF's first frame) decoded by BitmapFactory (the image loader's fallback
//! decoder). KompozContent's static methods, found as the library loads (on the UI thread: a worker thread's class
//! loader can't see the app's classes); a worker thread attached as it first calls, detached as it ends. Its one
//! `unsafe` says why it holds.

#![allow(unsafe_code)]

use super::app::VM;
use super::jni::{Env, Global, MethodId, jvalue};
use super::log_error;
use kompoz_graphics::ImageBitmap;
use kompoz_ui::platform::ImageRequestSize;
use std::sync::OnceLock;

/// KompozContent's class and its static methods.
struct Content {
    class: Global,
    http_get: MethodId,
    read_content: MethodId,
    decode_image: MethodId,
}

// SAFETY: a global reference is valid on every thread (JNI's "global references ... can be used across threads");
// this one is the process's for good (never dropped: a static), so no thread deletes it while another uses it.
unsafe impl Send for Content {}
// SAFETY: as above; its method ids are plain identifiers.
unsafe impl Sync for Content {}

static CONTENT: OnceLock<Content> = OnceLock::new();

/// KompozContent's methods found (JNI_OnLoad, on the UI thread), the image loader's fallback decoder set.
pub(super) fn load(env: &Env<'_>) -> bool {
    let found = (|| {
        let class = env.find_class(c"dev/kompozr/KompozContent")?;
        let m = |name: &std::ffi::CStr, signature: &std::ffi::CStr| env.static_method(&class, name, signature);
        Some(Content {
            http_get: m(c"httpGet", c"([B)[B")?,
            read_content: m(c"readContent", c"([B)[B")?,
            decode_image: m(c"decodeImage", c"([BIIZ)[I")?,
            class: env.new_global(class.object())?,
        })
    })();
    let Some(content) = found else {
        log_error("kompoz: KompozContent's methods aren't there: no HTTP, content:// or BitmapFactory");
        return false;
    };
    let _ = CONTENT.set(content);
    crate::image_loader::set_fallback_decoder(Some(decode));
    true
}

/// Detaches the thread it belongs to as that ends (one this module attached).
struct Attached;

impl Drop for Attached {
    fn drop(&mut self) {
        if let Some(vm) = VM.get() {
            vm.detach();
        }
    }
}

thread_local! {
    /// This thread was attached here (none: it was already, as the UI thread is).
    static ATTACHED: std::cell::RefCell<Option<Attached>> = const { std::cell::RefCell::new(None) };
}

/// `f` with this thread's env (attached first if it isn't: a worker's) and KompozContent.
fn with_java<R>(f: impl FnOnce(&Env<'static>, &Content) -> Option<R>) -> Option<R> {
    let content = CONTENT.get()?;
    let vm = VM.get()?;
    let env = match vm.env() {
        Some(env) => env,
        None => {
            let env = vm.attach()?;
            ATTACHED.with(|a| *a.borrow_mut() = Some(Attached));
            env
        }
    };
    f(&env, content)
}

/// `KompozContent.<method>(bytes)`'s byte array.
fn call_bytes(method: fn(&Content) -> MethodId, argument: &[u8]) -> Option<Vec<u8>> {
    with_java(|env, content| {
        let argument = env.new_bytes(argument)?;
        let result =
            env.call_static_object(content.class.object(), method(content), &[jvalue::from(argument.object())])?;
        env.bytes(result.object())
    })
}

/// An http(s) URL's body (none: it failed, or the server said no; logged by KompozContent): the image loader's
/// HTTP client on Android.
pub fn http_get(url: &str) -> Option<Vec<u8>> {
    call_bytes(|c| c.http_get, url.as_bytes()).filter(|b| !b.is_empty())
}

/// A `content://` URI's bytes from its provider (none if it can't be read).
pub fn read_content(uri: &str) -> Option<Vec<u8>> {
    call_bytes(|c| c.read_content, uri.as_bytes())
}

/// A picture BitmapFactory decodes (the formats ours lack), for `size` as the loader asks (scaled down by a power of
/// two as far as it allows).
fn decode(bytes: &[u8], size: ImageRequestSize) -> Option<ImageBitmap> {
    let ints = with_java(|env, content| {
        let data = env.new_bytes(bytes)?;
        let args = [
            jvalue::from(data.object()),
            jvalue::from(size.width.max(0)),
            jvalue::from(size.height.max(0)),
            jvalue::from(size.fill),
        ];
        let result = env.call_static_object(content.class.object(), content.decode_image, &args)?;
        Some(env.ints(result.object()))
    })?;
    image_of_argb(&ints)
}

/// BitmapFactory's answer (`[width, height]` then ARGB pixels, straight) as a bitmap.
fn image_of_argb(ints: &[i32]) -> Option<ImageBitmap> {
    let (&width, &height) = (ints.first()?, ints.get(1)?);
    let pixels = ints.get(2..)?;
    if width <= 0 || height <= 0 || pixels.len() < width as usize * height as usize {
        return None;
    }
    let rgba = pixels.iter().flat_map(|&p| {
        let [a, r, g, b] = (p as u32).to_be_bytes();
        [r, g, b, a]
    });
    ImageBitmap::from_pixels(width, height, rgba.collect())
}

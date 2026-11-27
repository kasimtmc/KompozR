//! The glue's Rust side (js/kompoz.js is its JavaScript side): the core imports every part shares.

#![allow(unsafe_code)]

use std::num::NonZeroU32;

#[link(wasm_import_module = "kompoz")]
unsafe extern "C" {
    fn handle_release(handle: u32);
    fn handle_clone(handle: u32) -> u32;
    fn result_take(into: *mut u8);
    fn result_drop();
    fn log_message(level: u32, message: *const u8, length: usize);
    fn clock_monotonic() -> f64;
    fn clock_unix() -> f64;
    fn page_query() -> i32;
    fn page_languages() -> i32;
    fn page_time_zone() -> i32;
    fn page_find(name: *const u8, length: usize) -> u32;
}

/// A JavaScript object of the glue's table that this value owns: dropped, the table lets go of it (the page's
/// garbage collector may then take it).
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct JsObject(NonZeroU32);

impl JsObject {
    /// Owns the object of `handle` (0: none), as an import returned it.
    pub fn from_raw(handle: u32) -> Option<JsObject> {
        NonZeroU32::new(handle).map(JsObject)
    }

    /// Its handle, for an import's argument (still owned by this value).
    pub fn raw(&self) -> u32 {
        self.0.get()
    }

    /// Its handle, no longer owned by a value: whoever has it lets go of it ([`release`]).
    pub fn into_raw(self) -> u32 {
        let raw = self.0.get();
        std::mem::forget(self);
        raw
    }

    /// Another handle of the same object (each let go of on its own).
    pub fn share(&self) -> JsObject {
        JsObject::share_raw(self.raw()).expect("an owned handle's object")
    }

    /// A handle of our own of the object of a handle someone else owns (the platform's); none if there is none.
    pub fn share_raw(handle: u32) -> Option<JsObject> {
        if handle == 0 {
            return None;
        }
        // SAFETY: handle_clone only reads the table at the handle; it returns a new handle (0 if there is nothing
        // at it).
        JsObject::from_raw(unsafe { handle_clone(handle) })
    }
}

impl Clone for JsObject {
    fn clone(&self) -> JsObject {
        self.share()
    }
}

impl Drop for JsObject {
    fn drop(&mut self) {
        release(self.0.get());
    }
}

/// Lets go of a handle that no [`JsObject`] owns (0: nothing).
pub fn release(handle: u32) {
    if handle != 0 {
        // SAFETY: handle_release only removes the table's entry at the handle (none there: nothing).
        unsafe { handle_release(handle) }
    }
}

/// What an import held as the glue's result, `length` bytes as it returned (negative: it held none), copied out
/// (the glue then holds nothing).
pub fn take_result(length: i32) -> Option<Vec<u8>> {
    let length = usize::try_from(length).ok()?;
    let mut bytes = Vec::<u8>::with_capacity(length);
    // SAFETY: the glue holds exactly `length` bytes (the import that held them said so, and nothing has run in
    // between), which result_take copies into the `length` bytes of capacity at the pointer, then drops.
    unsafe {
        result_take(bytes.as_mut_ptr());
        bytes.set_len(length);
    }
    Some(bytes)
}

/// The same as a string (UTF-8; a malformed one: none).
pub fn take_string(length: i32) -> Option<String> {
    String::from_utf8(take_result(length)?).ok()
}

/// Lets go of what an import held as the result without reading it.
pub fn drop_result() {
    // SAFETY: result_drop only clears the glue's result.
    unsafe { result_drop() }
}

/// How a message is written to the console.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Info,
    Warning,
    Error,
}

/// A message to the page's console (`console.log`, `.warn`, `.error`; a page that listens to the glue's log, as the
/// test runner's, gets it too).
pub fn log(level: Level, message: &str) {
    let level = match level {
        Level::Info => 0,
        Level::Warning => 1,
        Level::Error => 2,
    };
    // SAFETY: the import reads `message.len()` bytes at its pointer, which are the string's.
    unsafe { log_message(level, message.as_ptr(), message.len()) }
}

/// An object the page made before the module started, by its name (`"gpu.device"`: the `GPUDevice` the loader asked
/// for), as a handle of its own; none if the page has none by that name.
pub fn page_object(name: &str) -> Option<JsObject> {
    // SAFETY: the import reads `name.len()` bytes at its pointer, which are the string's.
    JsObject::from_raw(unsafe { page_find(name.as_ptr(), name.len()) })
}

fn monotonic_millis() -> f64 {
    // SAFETY: no arguments; performance.now().
    unsafe { clock_monotonic() }
}

fn unix_millis() -> f64 {
    // SAFETY: no arguments; Date.now().
    unsafe { clock_unix() }
}

/// The page's query string's variables (`?A=1&B=x%20y`: decoded as `URLSearchParams` does).
pub fn query_variables() -> Vec<(String, String)> {
    // SAFETY: the import holds the variables (`name=value` lines, decoded) as the result and returns their length.
    let text = take_string(unsafe { page_query() }).unwrap_or_default();
    text.lines().filter_map(|line| line.split_once('=')).map(|(n, v)| (n.to_string(), v.to_string())).collect()
}

/// Starts the glue's part of the runtime (once; later calls do nothing): the page's clocks, its query string as
/// the environment, the browser's languages and time zone, and a panic hook writing panics to the console (the
/// web's std writes nowhere). The web platform calls it first thing; so do the tests run in a browser.
pub fn start() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        kompoz_runtime::clock::set_source(kompoz_runtime::clock::ClockSource { monotonic_millis, unix_millis });
        kompoz_runtime::environment::set_vars(query_variables());
        // SAFETY: each import holds its answer (lines; a zone's id) as the result and returns its length.
        let languages = take_string(unsafe { page_languages() }).unwrap_or_default();
        let languages: Vec<String> = languages.lines().map(str::to_string).collect();
        if !languages.is_empty() {
            kompoz_runtime::environment::set_system_languages(Some(languages));
        }
        // SAFETY: as above.
        if let Some(zone) = take_string(unsafe { page_time_zone() }).filter(|z| !z.is_empty()) {
            kompoz_runtime::environment::set_system_time_zone(Some(zone));
        }
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log(Level::Error, &info.to_string());
            previous(info);
        }));
    });
}

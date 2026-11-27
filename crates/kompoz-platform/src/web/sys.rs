//! The page's calls the web platform makes, as imports of the web glue's module `kompoz` (kompoz-web; their
//! JavaScript is web.js beside this file), each wrapped safely: strings and bytes go in as a pointer and a length,
//! come back as the glue's result ([`kompoz_web::take_result`]); the page's objects (a paste's or a drop's data
//! transfer, an image being loaded) are the glue's handles.

#![allow(unsafe_code)]

use kompoz_web::{take_result, take_string};

/// The imports (the wrappers below have their names).
mod imports {
    #[link(wasm_import_module = "kompoz")]
    unsafe extern "C" {
        pub(super) fn web_start(canvas_id: *const u8, length: usize) -> u32;
        pub(super) fn web_state() -> i32;
        pub(super) fn web_events() -> i32;
        pub(super) fn web_request_frame();
        pub(super) fn web_wake_in(ms: f64);
        pub(super) fn web_ended(code: u32);
        pub(super) fn web_set_canvas_size(width: f64, height: f64);
        pub(super) fn web_set_title(title: *const u8, length: usize);
        pub(super) fn web_set_cursor(name: *const u8, length: usize);
        pub(super) fn web_fullscreen(on: u32);
        pub(super) fn web_text_input(
            active: u32,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
            kind: *const u8,
            length: usize,
        );
        pub(super) fn web_text_set(text: *const u8, length: usize, start: u32, end: u32);
        pub(super) fn web_clip_begin();
        pub(super) fn web_clip_add(mime: *const u8, mime_length: usize, data: *const u8, length: usize);
        pub(super) fn web_clip_commit(in_event: u32);
        pub(super) fn web_transfer_items(transfer: u32) -> i32;
        pub(super) fn web_transfer_data(transfer: u32, item: u32) -> i32;
        pub(super) fn web_drag_out(actions: u32) -> u32;
        pub(super) fn web_open_uri(uri: *const u8, length: usize);
        pub(super) fn web_vibrate(ms: u32);
        pub(super) fn web_keep_awake(on: u32);
        pub(super) fn web_location_set(path: *const u8, length: usize);
        pub(super) fn web_storage_get(key: *const u8, length: usize) -> i32;
        pub(super) fn web_storage_set(key: *const u8, key_length: usize, data: *const u8, length: usize);
        pub(super) fn web_alert(message: *const u8, length: usize);
        pub(super) fn web_file_save(path: *const u8, path_length: usize, data: *const u8, length: usize);
        pub(super) fn web_text_measure(text: *const u8, length: usize, font: *const u8, font_length: usize) -> i32;
        pub(super) fn web_text_draw(
            text: *const u8,
            length: usize,
            font: *const u8,
            font_length: usize,
            width: u32,
            height: u32,
            x: f64,
            y: f64,
        ) -> i32;
        pub(super) fn web_image_load(url: *const u8, length: usize, width: u32, height: u32, fill: u32) -> u32;
        pub(super) fn web_image_result(image: u32) -> i32;
    }
}

/// The page made ready for the app (once): its canvas (the element of id `canvas_id`, or one made to fill the page),
/// the hidden text area the keyboard types into, the page's listeners. False if the page has no room for it (no
/// document).
pub fn start(canvas_id: &str) -> bool {
    // SAFETY: the import reads `canvas_id.len()` bytes at its pointer, which are the string's.
    unsafe { imports::web_start(canvas_id.as_ptr(), canvas_id.len()) != 0 }
}

/// The page as it is now: `name=value` lines (the canvas's CSS size and the device pixel ratio, its pixels, the
/// media queries' answers, the focus, the visibility, the address's fragment).
pub fn state() -> String {
    // SAFETY: the import holds the lines as the result and returns their length.
    take_string(unsafe { imports::web_state() }).unwrap_or_default()
}

/// What the page's listeners gathered since last asked, a record a line (events.rs reads them), taken.
pub fn events() -> String {
    // SAFETY: the import holds the records as the result and returns their length.
    take_string(unsafe { imports::web_events() }).unwrap_or_default()
}

/// A turn of the loop at the display's next frame (requestAnimationFrame; one at a time).
pub fn request_frame() {
    // SAFETY: no arguments.
    unsafe { imports::web_request_frame() }
}

/// A turn of the loop in `ms` milliseconds (a timeout replacing the one before; negative: none).
pub fn wake_in(ms: f64) {
    // SAFETY: a number.
    unsafe { imports::web_wake_in(ms) }
}

/// The app ended with `code` (0: success): the page stops its loop and says so.
pub fn ended(code: u32) {
    // SAFETY: a number.
    unsafe { imports::web_ended(code) }
}

/// The canvas's size on the page, CSS pixels (0 by 0: the whole page, following it).
pub fn set_canvas_size(width: f64, height: f64) {
    // SAFETY: numbers.
    unsafe { imports::web_set_canvas_size(width, height) }
}

/// The document's title.
pub fn set_title(title: &str) {
    // SAFETY: the import reads `title.len()` bytes at its pointer, which are the string's.
    unsafe { imports::web_set_title(title.as_ptr(), title.len()) }
}

/// The canvas's cursor, by its CSS name.
pub fn set_cursor(name: &str) {
    // SAFETY: as above.
    unsafe { imports::web_set_cursor(name.as_ptr(), name.len()) }
}

/// The page full screen (the Fullscreen API), or not.
pub fn fullscreen(on: bool) {
    // SAFETY: a number.
    unsafe { imports::web_fullscreen(on as u32) }
}

/// The hidden text area active (focused, over the field at `x, y, width, height` CSS pixels, its keyboard of kind
/// `kind`: `inputmode enterkeyhint autocapitalize autocorrect` separated by spaces) or not.
pub fn text_input(active: bool, area: (f64, f64, f64, f64), kind: &str) {
    let (x, y, w, h) = area;
    // SAFETY: numbers, and `kind.len()` bytes at its pointer, which are the string's.
    unsafe { imports::web_text_input(active as u32, x, y, w, h, kind.as_ptr(), kind.len()) }
}

/// The text area's text and selection (UTF-16 offsets, as the page counts them).
pub fn text_set(text: &str, start: u32, end: u32) {
    // SAFETY: the import reads `text.len()` bytes at its pointer, which are the string's.
    unsafe { imports::web_text_set(text.as_ptr(), text.len(), start, end) }
}

/// What goes on the clipboard: begun, each type added, then committed — into the copy or cut event being handled
/// (`in_event`), else through the asynchronous Clipboard API.
pub fn clip_begin() {
    // SAFETY: no arguments.
    unsafe { imports::web_clip_begin() }
}

pub fn clip_add(mime: &str, data: &[u8]) {
    // SAFETY: the import reads the two ranges, which are the string's and the slice's.
    unsafe { imports::web_clip_add(mime.as_ptr(), mime.len(), data.as_ptr(), data.len()) }
}

pub fn clip_commit(in_event: bool) {
    // SAFETY: a number.
    unsafe { imports::web_clip_commit(in_event as u32) }
}

/// A transfer's items (a paste's, a drop's: the page read them as they came): a line each, `kind\tmime\tname`
/// (`kind`: `string` or `file`).
pub fn transfer_items(transfer: u32) -> String {
    // SAFETY: the import holds the lines as the result and returns their length.
    take_string(unsafe { imports::web_transfer_items(transfer) }).unwrap_or_default()
}

/// Item `item`'s bytes (a string's as UTF-8, a file's as they are).
pub fn transfer_data(transfer: u32, item: u32) -> Option<Vec<u8>> {
    // SAFETY: the import holds the bytes as the result and returns their length.
    take_result(unsafe { imports::web_transfer_data(transfer, item) })
}

/// The app's drag handed to the page's own drag and drop (what it carries: what `clip_add` gathered since
/// `clip_begin`; `actions`: a bit each for copy, move, link): whether the page took it — only while it handles the
/// `dragstart` of a press the browser begins a drag with.
pub fn drag_out(actions: u32) -> bool {
    // SAFETY: a number (the data is what clip_add gathered).
    unsafe { imports::web_drag_out(actions) != 0 }
}

pub fn open_uri(uri: &str) {
    // SAFETY: the import reads `uri.len()` bytes at its pointer, which are the string's.
    unsafe { imports::web_open_uri(uri.as_ptr(), uri.len()) }
}

pub fn vibrate(ms: u32) {
    // SAFETY: a number.
    unsafe { imports::web_vibrate(ms) }
}

/// The screen kept on (the Screen Wake Lock API, asked again as the page is shown), or not.
pub fn keep_awake(on: bool) {
    // SAFETY: a number.
    unsafe { imports::web_keep_awake(on as u32) }
}

/// The address's fragment set to `path` (history.replaceState: no new entry).
pub fn location_set(path: &str) {
    // SAFETY: the import reads `path.len()` bytes at its pointer, which are the string's.
    unsafe { imports::web_location_set(path.as_ptr(), path.len()) }
}

/// The session's storage's bytes under `key` (sessionStorage: kept across reloads of the tab).
pub fn storage_get(key: &str) -> Option<Vec<u8>> {
    // SAFETY: the import reads the key, holds the bytes as the result and returns their length (-1: none).
    take_result(unsafe { imports::web_storage_get(key.as_ptr(), key.len()) })
}

pub fn storage_set(key: &str, data: &[u8]) {
    // SAFETY: the import reads the two ranges, which are the string's and the slice's.
    unsafe { imports::web_storage_set(key.as_ptr(), key.len(), data.as_ptr(), data.len()) }
}

pub fn alert(message: &str) {
    // SAFETY: the import reads `message.len()` bytes at its pointer, which are the string's.
    unsafe { imports::web_alert(message.as_ptr(), message.len()) }
}

/// A file the app writes (a script's screenshot): handed to the page (a test runner's, or a download).
pub fn file_save(path: &str, data: &[u8]) {
    // SAFETY: the import reads the two ranges, which are the string's and the slice's.
    unsafe { imports::web_file_save(path.as_ptr(), path.len(), data.as_ptr(), data.len()) }
}

/// What the browser measures of `text` in CSS font `font`: its advance; its ink's left, right, ascent and descent
/// from the origin; the font's ascent and descent (seven numbers, a space between).
pub fn text_measure(text: &str, font: &str) -> String {
    // SAFETY: the import reads the two strings and holds its answer as the result.
    take_string(unsafe { imports::web_text_measure(text.as_ptr(), text.len(), font.as_ptr(), font.len()) })
        .unwrap_or_default()
}

/// `text` drawn by the browser in CSS font `font`, its origin at `x, y` of a `width` by `height` image: its pixels,
/// RGBA (straight alpha), the top row first.
pub fn text_draw(text: &str, font: &str, width: u32, height: u32, x: f64, y: f64) -> Option<Vec<u8>> {
    // SAFETY: the import reads the two strings and holds the pixels as the result.
    take_result(unsafe {
        imports::web_text_draw(text.as_ptr(), text.len(), font.as_ptr(), font.len(), width, height, x, y)
    })
}

/// An image fetched and decoded by the browser (createImageBitmap), for `width` by `height` (0: as it is; `fill`:
/// covering the box rather than fitting in it): its load's number.
pub fn image_load(url: &str, width: u32, height: u32, fill: bool) -> u32 {
    // SAFETY: the import reads `url.len()` bytes at its pointer, which are the string's.
    unsafe { imports::web_image_load(url.as_ptr(), url.len(), width, height, fill as u32) }
}

/// What load `image` gave: none while it is under way; `Some(None)` if it failed; else its width, height and RGBA
/// pixels (the load forgotten once taken).
pub fn image_result(image: u32) -> Option<Option<(u32, u32, Vec<u8>)>> {
    // SAFETY: the import holds a finished load's answer as the result: the width and height (u32, little-endian)
    // then the pixels; -2 while it is under way, -1 for a failure.
    let length = unsafe { imports::web_image_result(image) };
    match length {
        -2 => None,
        n if n < 0 => Some(None),
        n => {
            let bytes = take_result(n)?;
            if bytes.len() < 8 {
                return Some(None);
            }
            let w = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
            let h = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
            Some(Some((w, h, bytes[8..].to_vec())))
        }
    }
}

// ---- What the page calls (web.js, only between the module's own calls) ----

/// A turn of the loop: at an animation frame (`frame`, at `time` ms on the page's clock), or not.
#[unsafe(no_mangle)]
pub extern "C" fn kompoz_web_turn(frame: u32, time: f64) {
    super::run::turn(frame != 0, time);
}

/// A copy (`cut` 0) or cut (1) event: the app's copy goes into it.
#[unsafe(no_mangle)]
pub extern "C" fn kompoz_web_clipboard(cut: u32) {
    super::run::clipboard_event(cut != 0);
}

/// The page goes away: the app's state kept for the tab's next load.
#[unsafe(no_mangle)]
pub extern "C" fn kompoz_web_save() {
    super::run::save();
}

/// The browser begins a drag of a press at `x`, `y` (CSS pixels in the canvas): 1 if the app hands it its drag.
#[unsafe(no_mangle)]
pub extern "C" fn kompoz_web_drag_start(x: f64, y: f64) -> u32 {
    super::run::drag_start(x as f32, y as f32) as u32
}

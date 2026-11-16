//! http(s) downloads on Linux through libcurl, loaded at run time as C++ Kompoz's Linux builds link it
//! (common/image_loader.cpp's httpGet): what the image loader fetches URLs with ([`http_get`], its client until
//! [`set_http_client`](crate::image_loader::set_http_client) says otherwise). Redirects followed, 30 seconds at most,
//! a body only from a 2xx response, a download given up as the app quits. Its FFI is the `ffi` module's.

mod ffi;

pub use ffi::Response;

/// Whether libcurl can be had (why not if not): loaded (and initialized) once, when first asked or used.
pub fn load() -> Result<(), String> {
    ffi::load()
}

/// A GET of `url` as C++ Kompoz makes it (redirects followed, 30 seconds at most in all, no signals), given up when
/// `cancelled` says so (it is asked as the transfer goes, about once a second while nothing comes): the last
/// response's status and body, whatever the status; why not if the transfer failed (no libcurl, no connection, timed
/// out, given up).
pub fn get(url: &str, cancelled: fn() -> bool) -> Result<Response, String> {
    ffi::get(url, cancelled)
}

/// The image loader's HTTP client (C++'s `httpGet`): the body of a 2xx response to a GET of `url`, given up as the
/// app quits ([`stopping`](crate::image_loader::stopping)); none if it failed (logged when there is no libcurl, as
/// C++ logs a build without it).
pub fn http_get(url: &str) -> Option<Vec<u8>> {
    if let Err(why) = load() {
        crate::system::log(&format!("kompoz: no HTTP client (libcurl was not found: {why}): {url}"));
        return None;
    }
    let response = get(url, crate::image_loader::stopping).ok()?;
    (response.status / 100 == 2).then_some(response.body)
}

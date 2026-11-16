//! libcurl, loaded at run time (`dlopen` of libcurl.so.4: a binary that runs where it isn't installed, its images
//! from URLs then not loading), and the one transfer C++ Kompoz makes with it: a GET. The only `unsafe` code of the
//! HTTP client, each block saying why it holds.

#![allow(unsafe_code)]

use crate::linux::sys::Library;
use std::ffi::{CStr, CString, c_char, c_int, c_long, c_void};
use std::sync::OnceLock;

type Ptr = *mut c_void;
/// `curl_off_t`: 64 bits on every Linux (`long` on LP64, `long long` on 32-bit targets).
type OffT = i64;

/// `CURLE_OK`.
const OK: c_int = 0;
/// `CURL_GLOBAL_DEFAULT` (`CURL_GLOBAL_ALL`: SSL and Win32's sockets).
const GLOBAL_DEFAULT: c_long = 3;
/// `CURLOPTTYPE_LONG`, `CURLOPTTYPE_OBJECTPOINT` (strings, data pointers), `CURLOPTTYPE_FUNCTIONPOINT`.
const OPT_LONG: c_int = 0;
const OPT_POINTER: c_int = 10000;
const OPT_FUNCTION: c_int = 20000;
/// The options set (curl.h's numbers).
const OPT_WRITEDATA: c_int = OPT_POINTER + 1;
const OPT_URL: c_int = OPT_POINTER + 2;
const OPT_WRITEFUNCTION: c_int = OPT_FUNCTION + 11;
const OPT_TIMEOUT: c_int = OPT_LONG + 13;
const OPT_NOPROGRESS: c_int = OPT_LONG + 43;
const OPT_FOLLOWLOCATION: c_int = OPT_LONG + 52;
const OPT_XFERINFODATA: c_int = OPT_POINTER + 57;
const OPT_NOSIGNAL: c_int = OPT_LONG + 99;
const OPT_XFERINFOFUNCTION: c_int = OPT_FUNCTION + 219;
/// `CURLINFO_RESPONSE_CODE` (`CURLINFO_LONG + 2`).
const INFO_RESPONSE_CODE: c_int = 0x0020_0000 + 2;

/// `curl_write_callback`.
type WriteFn = extern "C" fn(*const c_char, usize, usize, Ptr) -> usize;
/// `curl_xferinfo_callback`.
type XferInfoFn = extern "C" fn(Ptr, OffT, OffT, OffT, OffT) -> c_int;

/// The functions used, looked up once.
struct Functions {
    global_init: unsafe extern "C" fn(c_long) -> c_int,
    easy_init: unsafe extern "C" fn() -> Ptr,
    easy_setopt: unsafe extern "C" fn(Ptr, c_int, ...) -> c_int,
    easy_perform: unsafe extern "C" fn(Ptr) -> c_int,
    easy_getinfo: unsafe extern "C" fn(Ptr, c_int, ...) -> c_int,
    easy_cleanup: unsafe extern "C" fn(Ptr),
    easy_strerror: unsafe extern "C" fn(c_int) -> *const c_char,
}

/// The library, loaded and initialized, and its functions. Kept for the process's life (as linking it would be):
/// libcurl is never cleaned up globally, as C++ Kompoz never does.
struct Lib {
    f: Functions,
    _library: Library,
}

/// Looks `name` up in `library` as a function pointer of type `F`.
///
/// # Safety
/// `F` must be the symbol's C signature as an `unsafe extern "C" fn`.
unsafe fn function<F: Copy>(library: &Library, name: &CStr) -> Result<F, String> {
    let p = library.symbol(name).ok_or_else(|| format!("libcurl has no {}", name.to_string_lossy()))?;
    debug_assert_eq!(std::mem::size_of::<F>(), std::mem::size_of::<*mut c_void>());
    // SAFETY: the caller says `F` is the symbol's signature, a function pointer as big as `p`.
    Ok(unsafe { std::mem::transmute_copy::<*mut c_void, F>(&p) })
}

impl Lib {
    fn load() -> Result<Lib, String> {
        let library = Library::open(&["libcurl.so.4", "libcurl-gnutls.so.4", "libcurl.so"])?;
        let l = &library;
        // SAFETY: each `function` call's type is curl.h's (easy.h's) signature of its symbol, as `Functions`
        // declares it: CURLcode, CURLoption and CURLINFO are C enums (ints), the handle a pointer.
        let f = unsafe {
            Functions {
                global_init: function(l, c"curl_global_init")?,
                easy_init: function(l, c"curl_easy_init")?,
                easy_setopt: function(l, c"curl_easy_setopt")?,
                easy_perform: function(l, c"curl_easy_perform")?,
                easy_getinfo: function(l, c"curl_easy_getinfo")?,
                easy_cleanup: function(l, c"curl_easy_cleanup")?,
                easy_strerror: function(l, c"curl_easy_strerror")?,
            }
        };
        // Once for the process, before any transfer (C++'s std::call_once): the OnceLock this runs in makes it once.
        // SAFETY: a plain call with curl.h's flags; nothing else uses libcurl yet (the lock is held).
        let code = unsafe { (f.global_init)(GLOBAL_DEFAULT) };
        if code != OK {
            return Err(format!("curl_global_init failed ({code})"));
        }
        Ok(Lib { f, _library: library })
    }

    fn error(&self, code: c_int) -> String {
        // SAFETY: curl_easy_strerror returns a static C string for any code.
        let text = unsafe { (self.f.easy_strerror)(code) };
        if text.is_null() {
            return format!("libcurl error {code}");
        }
        // SAFETY: not null: a static, NUL-terminated string.
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
    }
}

/// libcurl, loaded once; why not if it can't be had.
fn lib() -> Result<&'static Lib, String> {
    static LIB: OnceLock<Result<Lib, String>> = OnceLock::new();
    LIB.get_or_init(Lib::load).as_ref().map_err(Clone::clone)
}

/// Whether libcurl could be loaded (why not if not).
pub(super) fn load() -> Result<(), String> {
    lib().map(|_| ())
}

/// What a transfer's callbacks are handed: the body as it comes, and what says to give up.
struct Sink {
    body: Vec<u8>,
    cancelled: fn() -> bool,
}

/// `CURLOPT_WRITEFUNCTION`: the body's bytes as they come, kept.
extern "C" fn write(data: *const c_char, size: usize, count: usize, sink: Ptr) -> usize {
    let n = size.saturating_mul(count);
    if n == 0 || data.is_null() {
        return n;
    }
    // SAFETY: `sink` is the `Sink` given as CURLOPT_WRITEDATA, alive and not otherwise used while
    // curl_easy_perform runs (which is when this is called, on the thread that called it).
    let sink = unsafe { &mut *sink.cast::<Sink>() };
    // SAFETY: libcurl hands `size * count` readable bytes at `data` for the call.
    sink.body.extend_from_slice(unsafe { std::slice::from_raw_parts(data.cast::<u8>(), n) });
    n
}

/// `CURLOPT_XFERINFOFUNCTION`: called as the transfer goes (about once a second at least, while nothing comes too);
/// not 0 aborts it (the app is quitting).
extern "C" fn progress(sink: Ptr, _: OffT, _: OffT, _: OffT, _: OffT) -> c_int {
    // SAFETY: `sink` is the `Sink` given as CURLOPT_XFERINFODATA, alive while curl_easy_perform runs; only its
    // `cancelled` is read (the write callback is never running at the same time: one thread).
    let cancelled = unsafe { (*sink.cast::<Sink>()).cancelled };
    c_int::from(cancelled())
}

/// What a GET came to: the response's status (the last one, redirects followed) and its body.
pub struct Response {
    pub status: i64,
    pub body: Vec<u8>,
}

/// An easy handle, cleaned up when dropped.
struct Easy<'a> {
    lib: &'a Lib,
    raw: Ptr,
}

impl Drop for Easy<'_> {
    fn drop(&mut self) {
        // SAFETY: the handle from curl_easy_init, cleaned up once, no transfer running on it.
        unsafe { (self.lib.f.easy_cleanup)(self.raw) };
    }
}

impl Easy<'_> {
    /// Sets a `long` option.
    fn long(&self, option: c_int, value: c_long) -> Result<(), String> {
        // SAFETY: the handle is valid; `option` is a CURLOPTTYPE_LONG one, which takes a `long`.
        self.check(unsafe { (self.lib.f.easy_setopt)(self.raw, option, value) })
    }

    fn check(&self, code: c_int) -> Result<(), String> {
        if code == OK { Ok(()) } else { Err(self.lib.error(code)) }
    }
}

/// A GET of `url` as C++ Kompoz's httpGet makes it: redirects followed, 30 seconds at most in all, no signals
/// (it runs on a worker thread), the body gathered, given up when `cancelled` says so. Its status and body; why not
/// if the transfer failed (no libcurl, no connection, timed out, given up).
pub(super) fn get(url: &str, cancelled: fn() -> bool) -> Result<Response, String> {
    let lib = lib()?;
    let url = CString::new(url).map_err(|_| "a NUL in the URL".to_string())?;
    // SAFETY: a plain call; libcurl is initialized (lib()).
    let raw = unsafe { (lib.f.easy_init)() };
    if raw.is_null() {
        return Err("curl_easy_init failed".into());
    }
    let easy = Easy { lib, raw };
    let mut sink = Sink { body: Vec::new(), cancelled };
    let sink_ptr: Ptr = (&raw mut sink).cast();
    // SAFETY: the handle is valid; CURLOPT_URL takes a C string, which libcurl copies (and `url` outlives the
    // transfer anyway).
    easy.check(unsafe { (lib.f.easy_setopt)(raw, OPT_URL, url.as_ptr()) })?;
    easy.long(OPT_FOLLOWLOCATION, 1)?;
    easy.long(OPT_TIMEOUT, 30)?;
    easy.long(OPT_NOSIGNAL, 1)?;
    // SAFETY: the handle is valid; CURLOPT_WRITEFUNCTION takes a curl_write_callback, which `write` is, and
    // CURLOPT_WRITEDATA the pointer handed to it: `sink`, which lives until the transfer is over (below).
    unsafe {
        easy.check((lib.f.easy_setopt)(raw, OPT_WRITEFUNCTION, write as WriteFn))?;
        easy.check((lib.f.easy_setopt)(raw, OPT_WRITEDATA, sink_ptr))?;
    }
    easy.long(OPT_NOPROGRESS, 0)?;
    // SAFETY: as above: `progress` is a curl_xferinfo_callback, handed `sink` (C++'s reads a global flag instead).
    unsafe {
        easy.check((lib.f.easy_setopt)(raw, OPT_XFERINFOFUNCTION, progress as XferInfoFn))?;
        easy.check((lib.f.easy_setopt)(raw, OPT_XFERINFODATA, sink_ptr))?;
    }
    // SAFETY: the handle is set up; the callbacks' `sink` is alive and untouched here until it returns.
    easy.check(unsafe { (lib.f.easy_perform)(raw) })?;
    let mut status: c_long = 0;
    // SAFETY: CURLINFO_RESPONSE_CODE writes a `long` through the pointer, which points at `status`.
    easy.check(unsafe { (lib.f.easy_getinfo)(raw, INFO_RESPONSE_CODE, &raw mut status) })?;
    drop(easy);
    // (`long` is 32 bits on 32-bit targets: the conversion is a no-op only on 64-bit ones.)
    #[allow(clippy::useless_conversion)]
    let status = i64::from(status);
    Ok(Response { status, body: sink.body })
}

//! What the macOS platform calls below AppKit, as C functions of the system's own (no binding crate has them):
//! the unified log (`os_log`, as clang's `os_log` macro calls it), the power manager's assertion that keeps the
//! display awake (IOKit), and the quit signals turned into the main queue's work (Grand Central Dispatch's signal
//! sources, through dispatch2).

#![allow(unsafe_code)]

use dispatch2::{DispatchObject, DispatchQueue, DispatchRetained, DispatchSource};
use objc2_foundation::NSString;
use std::ffi::{CStr, c_char, c_void};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// The time since the system started, not counting its sleep, ns (`CLOCK_UPTIME_RAW`: AppKit's events' clock).
pub fn uptime_ns() -> i64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `t` is a valid timespec to write to; CLOCK_UPTIME_RAW is one of the system's clocks.
    unsafe { libc::clock_gettime(libc::CLOCK_UPTIME_RAW, &mut t) };
    t.tv_sec * 1_000_000_000 + t.tv_nsec
}

/// An `os_log_t`.
#[repr(C)]
struct OsLog {
    _opaque: [u8; 0],
}

unsafe extern "C" {
    /// The image this code is in (its Mach-O header): what `os_log` finds its format strings by. ld64 defines it for
    /// every image it links.
    static __dso_handle: u8;
    fn os_log_create(subsystem: *const c_char, category: *const c_char) -> *mut OsLog;
    /// What clang's `os_log` macro expands to: the message's format (in the image's `__oslogstring` section) and its
    /// arguments encoded as the macro encodes them.
    fn _os_log_impl(dso: *const c_void, log: *mut OsLog, kind: u8, format: *const c_char, buffer: *mut u8, size: u32);
}

/// The format of every line: one public string (`%{public}s`, else the log hides it as `<private>`), in the section
/// `os_log` reads formats from, as clang puts its literals there.
#[unsafe(link_section = "__TEXT,__oslogstring,cstring_literals")]
static FORMAT: [u8; 11] = *b"%{public}s\0";

/// The app's log (`dev.kompozr`, category "app"), made once; none if the system gave none.
fn app_log() -> Option<usize> {
    static LOG: OnceLock<usize> = OnceLock::new();
    let log = *LOG.get_or_init(|| {
        // SAFETY: both are NUL-terminated strings of 'static storage; os_log_create returns a log object that lives
        // as long as the process (or the default log, never freed either).
        unsafe { os_log_create(c"dev.kompozr".as_ptr(), c"app".as_ptr()) as usize }
    });
    (log != 0).then_some(log)
}

/// A line into the unified log (the default level: Console and `log stream` show it).
pub fn os_log(message: &str) {
    let Some(log) = app_log() else { return };
    let mut line: Vec<u8> = message.bytes().filter(|&b| b != 0).collect();
    line.push(0);
    let text = CStr::from_bytes_with_nul(&line).expect("no NUL inside: filtered");
    // The arguments as clang encodes them (LLVM's OSLog.h): a summary byte (2: an argument that isn't a scalar), their
    // count, then each: a descriptor ((kind 2: a string) << 4 | flags 2: public), its size, its value (a pointer).
    let pointer = (text.as_ptr() as usize as u64).to_ne_bytes();
    let mut buffer = [0u8; 12];
    buffer[..4].copy_from_slice(&[0x02, 0x01, 0x22, 0x08]);
    buffer[4..].copy_from_slice(&pointer);
    // SAFETY: `__dso_handle` is this image's header (ld64's), FORMAT a NUL-terminated format in this image's
    // `__oslogstring` section, `log` the log os_log_create made; `buffer` holds 12 bytes encoded as the format's one
    // public string argument, its pointer to `text`, NUL-terminated and alive until the call returns. Type 0 is
    // OS_LOG_TYPE_DEFAULT.
    unsafe {
        _os_log_impl(
            &raw const __dso_handle as *const c_void,
            log as *mut OsLog,
            0,
            FORMAT.as_ptr() as *const c_char,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        );
    }
}

// ---- The display kept awake (IOKit's power management) ----------------------------------------------------------

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(kind: *const c_void, level: u32, name: *const c_void, id: *mut u32) -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
}

/// `kIOPMAssertionLevelOn`.
const ASSERTION_LEVEL_ON: u32 = 255;

/// An assertion that the display doesn't sleep while the user is idle (`kIOPMAssertionTypePreventUserIdleDisplaySleep`,
/// C++'s keepScreenOn), named `reason`; its id, none if the system refused.
pub fn keep_display_awake(reason: &str) -> Option<u32> {
    let kind = NSString::from_str("PreventUserIdleDisplaySleep");
    let name = NSString::from_str(reason);
    let mut id = 0u32;
    // SAFETY: NSString is toll-free bridged with CFString: both are valid CFStringRefs for the call (kept alive by
    // the Retained values until it returns); `id` is a valid u32 to write the assertion's id to.
    let result = unsafe {
        IOPMAssertionCreateWithName(
            &*kind as *const NSString as *const c_void,
            ASSERTION_LEVEL_ON,
            &*name as *const NSString as *const c_void,
            &mut id,
        )
    };
    (result == 0).then_some(id)
}

/// The assertion let go.
pub fn let_display_sleep(id: u32) {
    // SAFETY: `id` is an assertion keep_display_awake made and nothing released yet (the caller's).
    unsafe {
        IOPMAssertionRelease(id);
    }
}

// ---- Quit signals -----------------------------------------------------------------------------------------------

/// A quit signal (SIGINT, SIGTERM) came since last asked.
static QUIT_SIGNAL: AtomicBool = AtomicBool::new(false);

/// The function the signals' sources call on the main queue, and what then wakes the loop.
static ON_SIGNAL: OnceLock<fn()> = OnceLock::new();

extern "C" fn signalled(_context: *mut c_void) {
    QUIT_SIGNAL.store(true, Ordering::SeqCst);
    if let Some(wake) = ON_SIGNAL.get() {
        wake();
    }
}

/// SIGINT and SIGTERM made a quit (C++'s dispatch sources): their default action (ending the process) turned off,
/// each a dispatch source on the main queue that marks it and calls `wake`; for the process's life. Once.
pub fn catch_quit_signals(wake: fn()) {
    static SOURCES: OnceLock<Vec<SendSource>> = OnceLock::new();
    let _ = ON_SIGNAL.set(wake);
    SOURCES.get_or_init(|| {
        let mut sources = Vec::new();
        for signal in [libc::SIGINT, libc::SIGTERM] {
            // SAFETY: ignoring a signal is always sound; the dispatch source still sees it (kqueue's EVFILT_SIGNAL).
            unsafe {
                libc::signal(signal, libc::SIG_IGN);
            }
            // SAFETY: `_dispatch_source_type_signal` is libdispatch's signal source type, with the signal's number as
            // its handle and no mask, on the main queue (alive for the process).
            let source = unsafe {
                DispatchSource::new(
                    &raw const dispatch2::_dispatch_source_type_signal as *mut _,
                    signal as usize,
                    0,
                    Some(DispatchQueue::main()),
                )
            };
            source.set_event_handler_f(signalled);
            source.resume();
            sources.push(SendSource(source));
        }
        sources
    });
}

/// A dispatch source kept for the process's life (dispatch objects are thread-safe).
struct SendSource(#[allow(dead_code)] DispatchRetained<DispatchSource>);

// SAFETY: libdispatch's objects are reference counted atomically and safe to keep or release on any thread; the
// sources are only kept here, never used after being resumed.
unsafe impl Send for SendSource {}
// SAFETY: as above; nothing is reached through a shared reference.
unsafe impl Sync for SendSource {}

/// Whether a quit signal came since last asked.
pub fn take_quit_signal() -> bool {
    QUIT_SIGNAL.swap(false, Ordering::SeqCst)
}

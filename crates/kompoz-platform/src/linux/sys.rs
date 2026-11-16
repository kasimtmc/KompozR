//! What the Linux platforms need of the kernel and the C library, as safe functions over libc (the only module of
//! the crate with `unsafe` code besides the FFI modules): eventfds (waking a loop from any thread), timerfds (a
//! deadline, a key's repeats), `poll`, the monotonic clock the events' times are on, memfds and mappings (shared
//! memory for the compositor, a keymap handed over), pipes (clipboard and drag data), SIGINT and SIGTERM turned into
//! a quit, and loading shared libraries (`dlopen`), whose symbols the FFI modules turn into functions.
//!
//! Every `unsafe` block says why it holds.

#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_void};
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Duration;

/// The result of a call returning -1 on failure (errno then says why).
fn check(result: libc::c_int) -> io::Result<libc::c_int> {
    if result < 0 { Err(io::Error::last_os_error()) } else { Ok(result) }
}

/// A new file descriptor from a call returning one (or -1).
fn owned(fd: libc::c_int) -> io::Result<OwnedFd> {
    let fd = check(fd)?;
    // SAFETY: the call just returned it, open and owned by no one else.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// The monotonic clock (`CLOCK_MONOTONIC`), ns: the clock Wayland's and the timers' times are on.
// (`time_t` and `long` are 32 bits on 32-bit targets: the conversions are no-ops only on 64-bit ones.)
#[allow(clippy::useless_conversion)]
pub fn monotonic_now() -> i64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `t` is a valid timespec to write; CLOCK_MONOTONIC always exists on Linux.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) };
    i64::from(t.tv_sec) * 1_000_000_000 + i64::from(t.tv_nsec)
}

// ---- eventfd ----------------------------------------------------------------------------------------------------

/// An eventfd (non-blocking, closed on exec): a counter written from anywhere, readable while it isn't 0.
pub fn eventfd() -> io::Result<OwnedFd> {
    // SAFETY: plain call; the flags are eventfd's.
    owned(unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) })
}

/// Adds `value` to an eventfd's counter (async-signal-safe: a signal handler may call it through its raw fd).
pub fn eventfd_write(fd: BorrowedFd<'_>, value: u64) -> io::Result<()> {
    write_eventfd(fd.as_raw_fd(), value)
}

fn write_eventfd(fd: RawFd, value: u64) -> io::Result<()> {
    let bytes = value.to_ne_bytes();
    // SAFETY: `bytes` is 8 readable bytes, what an eventfd takes; a bad fd fails with EBADF.
    let n = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
    if n == bytes.len() as isize {
        return Ok(());
    }
    let e = io::Error::last_os_error();
    // (Full: the counter is near its end, the reader is woken anyway.)
    if e.kind() == io::ErrorKind::WouldBlock { Ok(()) } else { Err(e) }
}

/// An eventfd's counter, taken (0 if nothing was written since).
pub fn eventfd_read(fd: BorrowedFd<'_>) -> io::Result<u64> {
    read_u64(fd)
}

/// Eight bytes read from a non-blocking fd as a number (an eventfd's counter, a timerfd's expirations); 0 if there
/// is nothing to read.
fn read_u64(fd: BorrowedFd<'_>) -> io::Result<u64> {
    let mut bytes = [0u8; 8];
    // SAFETY: `bytes` is 8 writable bytes; the fd is open (borrowed).
    let n = unsafe { libc::read(fd.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len()) };
    if n == bytes.len() as isize {
        return Ok(u64::from_ne_bytes(bytes));
    }
    let e = io::Error::last_os_error();
    if n < 0 && e.kind() == io::ErrorKind::WouldBlock { Ok(0) } else { Err(e) }
}

// ---- timerfd ----------------------------------------------------------------------------------------------------

/// A timerfd on the monotonic clock (non-blocking, closed on exec), disarmed.
pub fn timerfd() -> io::Result<OwnedFd> {
    // SAFETY: plain call; the clock and flags are timerfd's.
    owned(unsafe { libc::timerfd_create(libc::CLOCK_MONOTONIC, libc::TFD_CLOEXEC | libc::TFD_NONBLOCK) })
}

fn timespec(ns: i64) -> libc::timespec {
    let ns = ns.max(0);
    libc::timespec { tv_sec: (ns / 1_000_000_000) as libc::time_t, tv_nsec: (ns % 1_000_000_000) as libc::c_long }
}

fn set_timer(fd: BorrowedFd<'_>, flags: libc::c_int, value: i64, interval: i64) -> io::Result<()> {
    let spec = libc::itimerspec { it_interval: timespec(interval), it_value: timespec(value) };
    // SAFETY: `spec` is a valid itimerspec to read; the old value isn't asked for (null).
    check(unsafe { libc::timerfd_settime(fd.as_raw_fd(), flags, &spec, std::ptr::null_mut()) })?;
    Ok(())
}

/// Arms a timerfd to expire once at `at` (monotonic ns; one already past expires at once), or disarms it (none).
pub fn timerfd_set_at(fd: BorrowedFd<'_>, at: Option<i64>) -> io::Result<()> {
    match at {
        // (0 would disarm it: 1 ns is long past.)
        Some(at) => set_timer(fd, libc::TFD_TIMER_ABSTIME, at.max(1), 0),
        None => set_timer(fd, 0, 0, 0),
    }
}

/// Arms a timerfd to expire after `first`, then every `interval` (zero: once), from now; a zero `first` disarms
/// it.
pub fn timerfd_set_repeating(fd: BorrowedFd<'_>, first: Duration, interval: Duration) -> io::Result<()> {
    set_timer(fd, 0, first.as_nanos().min(i64::MAX as u128) as i64, interval.as_nanos().min(i64::MAX as u128) as i64)
}

/// How many times a timerfd expired since it was last read (0: not yet).
pub fn timerfd_read(fd: BorrowedFd<'_>) -> io::Result<u64> {
    read_u64(fd)
}

// ---- poll -------------------------------------------------------------------------------------------------------

/// A file descriptor to `poll`, what to wait for on it, and what came.
#[derive(Clone, Copy, Debug)]
pub struct PollFd {
    pub fd: RawFd,
    /// Wait until it can be read (else until it can be written).
    pub readable: bool,
    /// After `poll`: it is ready (or hung up, or in error: reading or writing it says which).
    pub ready: bool,
}

impl PollFd {
    pub fn readable(fd: RawFd) -> PollFd {
        PollFd { fd, readable: true, ready: false }
    }
    pub fn writable(fd: RawFd) -> PollFd {
        PollFd { fd, readable: false, ready: false }
    }
}

/// Waits until one of `fds` is ready, at most `timeout` (none: as long as it takes); how many are. A signal arriving
/// ends the wait early (0 ready).
pub fn poll(fds: &mut [PollFd], timeout: Option<Duration>) -> io::Result<usize> {
    let mut raw: Vec<libc::pollfd> = fds
        .iter()
        .map(|f| libc::pollfd { fd: f.fd, events: if f.readable { libc::POLLIN } else { libc::POLLOUT }, revents: 0 })
        .collect();
    let ms = match timeout {
        None => -1,
        // (Rounded up: a wait for a deadline doesn't wake just before it.)
        Some(t) => t.as_nanos().div_ceil(1_000_000).min(i32::MAX as u128) as libc::c_int,
    };
    // SAFETY: `raw` is `raw.len()` valid pollfd structs; poll writes only their revents.
    let n = unsafe { libc::poll(raw.as_mut_ptr(), raw.len() as libc::nfds_t, ms) };
    if n < 0 {
        let e = io::Error::last_os_error();
        if e.kind() == io::ErrorKind::Interrupted {
            fds.iter_mut().for_each(|f| f.ready = false);
            return Ok(0);
        }
        return Err(e);
    }
    for (f, r) in fds.iter_mut().zip(&raw) {
        f.ready = r.revents != 0;
    }
    Ok(n as usize)
}

// ---- Shared memory, mappings, pipes --------------------------------------------------------------------------------

/// An anonymous file in memory (`memfd_create`, closed on exec, sealable), of no size yet (`File::set_len` sizes it):
/// shared memory to hand a compositor.
pub fn memfd(name: &str) -> io::Result<OwnedFd> {
    let name = CString::new(name).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: `name` is a valid C string; the flags are memfd_create's.
    owned(unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING) })
}

/// A file mapped into memory, unmapped when dropped.
pub struct Mmap {
    ptr: *mut c_void,
    len: usize,
}

impl Mmap {
    /// `len` bytes of `fd` from its start, shared with whoever else maps it (writable or not).
    pub fn shared(fd: BorrowedFd<'_>, len: usize, writable: bool) -> io::Result<Mmap> {
        let protection = if writable { libc::PROT_READ | libc::PROT_WRITE } else { libc::PROT_READ };
        Mmap::map(fd, len, protection, libc::MAP_SHARED)
    }

    /// `len` bytes of `fd` from its start, read only, private (a keymap handed over: Wayland's needs MAP_PRIVATE).
    pub fn private_read(fd: BorrowedFd<'_>, len: usize) -> io::Result<Mmap> {
        Mmap::map(fd, len, libc::PROT_READ, libc::MAP_PRIVATE)
    }

    fn map(fd: BorrowedFd<'_>, len: usize, protection: libc::c_int, flags: libc::c_int) -> io::Result<Mmap> {
        if len == 0 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        // SAFETY: a new mapping of an open fd at an address the kernel picks: nothing existing is touched.
        let ptr = unsafe { libc::mmap(std::ptr::null_mut(), len, protection, flags, fd.as_raw_fd(), 0) };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(Mmap { ptr, len })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Its bytes. (Another process may write a shared mapping meanwhile: what is read is what was there.)
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: the mapping is `len` readable bytes for as long as `self` lives.
        unsafe { std::slice::from_raw_parts(self.ptr.cast(), self.len) }
    }

    /// Its bytes to write (a mapping made writable; else writing them faults).
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: the mapping is `len` bytes for as long as `self` lives, borrowed mutably once.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.cast(), self.len) }
    }
}

impl Drop for Mmap {
    fn drop(&mut self) {
        // SAFETY: the mapping made by `map`, not unmapped before; no slice of it outlives `self`.
        unsafe { libc::munmap(self.ptr, self.len) };
    }
}

/// A pipe (its read end, its write end), closed on exec; `nonblocking` for both ends.
pub fn pipe(nonblocking: bool) -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as libc::c_int; 2];
    let flags = libc::O_CLOEXEC | if nonblocking { libc::O_NONBLOCK } else { 0 };
    // SAFETY: `fds` is two writable ints, what pipe2 fills.
    check(unsafe { libc::pipe2(fds.as_mut_ptr(), flags) })?;
    // SAFETY: pipe2 just opened both, owned by no one else.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

/// Makes a file descriptor non-blocking (or blocking).
pub fn set_nonblocking(fd: BorrowedFd<'_>, on: bool) -> io::Result<()> {
    // SAFETY: fcntl on an open fd, reading its flags.
    let flags = check(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) })?;
    let flags = if on { flags | libc::O_NONBLOCK } else { flags & !libc::O_NONBLOCK };
    // SAFETY: the same, setting them.
    check(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags) })?;
    Ok(())
}

// ---- Signals ----------------------------------------------------------------------------------------------------

static QUIT_SIGNALLED: AtomicBool = AtomicBool::new(false);
static SIGNAL_WAKE_FD: AtomicI32 = AtomicI32::new(-1);

/// SIGINT and SIGTERM's handler: async-signal-safe (an atomic store and a write).
extern "C" fn on_quit_signal(_: libc::c_int) {
    QUIT_SIGNALLED.store(true, Ordering::SeqCst);
    let fd = SIGNAL_WAKE_FD.load(Ordering::SeqCst);
    if fd >= 0 {
        let _ = write_eventfd(fd, 1);
    }
}

/// SIGINT and SIGTERM (Ctrl+C in a terminal, a service stopping it) as a quit, as C++'s (and SDL's): the app
/// writes its state on its way out. The handler sets a flag ([`take_quit_signal`]) and writes `wake_fd` (the loop's
/// eventfd, which must stay open while the handler is installed).
pub fn quit_on_signals(wake_fd: RawFd) -> io::Result<()> {
    SIGNAL_WAKE_FD.store(wake_fd, Ordering::SeqCst);
    // SAFETY: a zeroed sigaction is a valid one to fill (no handler, no flags, an empty mask).
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = on_quit_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
    action.sa_flags = libc::SA_RESTART;
    // SAFETY: `sa_mask` is the action's own signal set.
    unsafe { libc::sigemptyset(&mut action.sa_mask) };
    for signal in [libc::SIGINT, libc::SIGTERM] {
        // SAFETY: a valid action whose handler only stores an atomic and writes an fd (async-signal-safe); the old
        // action isn't asked for.
        check(unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) })?;
    }
    Ok(())
}

/// The wake fd the signal handler writes no longer (the loop's going): its signals still set the flag.
pub fn forget_signal_wake_fd(wake_fd: RawFd) {
    let _ = SIGNAL_WAKE_FD.compare_exchange(wake_fd, -1, Ordering::SeqCst, Ordering::SeqCst);
}

/// Whether SIGINT or SIGTERM came since this was last asked.
pub fn take_quit_signal() -> bool {
    QUIT_SIGNALLED.swap(false, Ordering::SeqCst)
}

/// Sends `signal` to this process (tests: a SIGTERM as a service manager would send it).
pub fn raise(signal: libc::c_int) -> io::Result<()> {
    // SAFETY: raise only delivers the signal to this thread, its handler (ours, or the default) runs.
    check(unsafe { libc::raise(signal) })?;
    Ok(())
}

// ---- Shared libraries -----------------------------------------------------------------------------------------------

/// A shared library loaded with `dlopen` (now, its symbols kept to itself), closed when dropped. What a symbol is
/// the FFI module looking it up says, and it keeps the library while it uses the symbol.
pub struct Library {
    handle: *mut c_void,
}

impl Library {
    /// The first of `names` that loads ("libxkbcommon.so.0", then "libxkbcommon.so"); why none did.
    pub fn open(names: &[&str]) -> Result<Library, String> {
        let mut why = String::new();
        for name in names {
            let Ok(c) = CString::new(*name) else { continue };
            // SAFETY: `c` is a valid C string; the library's constructors run, as linking it would run them.
            let handle = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
            if !handle.is_null() {
                return Ok(Library { handle });
            }
            why = dl_error();
        }
        Err(if why.is_empty() { format!("{} not found", names.join(", ")) } else { why })
    }

    /// The address of the symbol `name`, if the library has it.
    pub fn symbol(&self, name: &CStr) -> Option<*mut c_void> {
        // SAFETY: the handle is open while `self` lives; `name` is a valid C string.
        let p = unsafe { libc::dlsym(self.handle, name.as_ptr()) };
        (!p.is_null()).then_some(p)
    }
}

/// dlerror's message (none: an empty string).
fn dl_error() -> String {
    // SAFETY: dlerror returns null or a C string valid until the next dl call on this thread, copied at once.
    let e = unsafe { libc::dlerror() };
    if e.is_null() {
        return String::new();
    }
    // SAFETY: not null: a valid C string, as above.
    unsafe { CStr::from_ptr(e) }.to_string_lossy().into_owned()
}

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: the handle from dlopen, closed once; no symbol of it is used after (its users hold the library).
        unsafe { libc::dlclose(self.handle) };
    }
}

// SAFETY: a dlopen handle may be used and closed from any thread (dlsym and dlclose are thread-safe).
unsafe impl Send for Library {}
// SAFETY: as above: looking symbols up from several threads at once is safe.
unsafe impl Sync for Library {}

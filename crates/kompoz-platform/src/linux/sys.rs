//! What the Linux platforms need of the kernel and the C library besides what Android shares with them
//! ([`crate::unix::sys`], re-exported here: eventfds, timerfds, `poll`, the monotonic clock, pipes, `dlopen`), as safe
//! functions over libc (with that one, the only modules of the crate with `unsafe` code besides the FFI modules):
//! memfds and mappings (shared memory for the compositor, a keymap handed over), inotify (the desktop's settings files
//! followed), V4L2's capability query (a camera looked for), SIGINT and SIGTERM turned into a quit.
//!
//! Every `unsafe` block says why it holds.

#![allow(unsafe_code)]

pub use crate::unix::sys::*;

use crate::unix::sys::{check, owned, write_eventfd};
use std::ffi::{CString, c_void};
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

// ---- Shared memory, mappings ------------------------------------------------------------------------------------

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

// ---- inotify ------------------------------------------------------------------------------------------------------

/// An inotify instance (non-blocking, closed on exec): reading it gives the events of what it watches
/// (`inotify_event`s: wd, mask, cookie, the name's length, the name).
pub fn inotify() -> io::Result<OwnedFd> {
    // SAFETY: plain call; the flags are inotify_init1's.
    owned(unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) })
}

/// Watches `path` for the events in `mask` (`libc::IN_CLOSE_WRITE` …); the watch's descriptor.
pub fn inotify_add_watch(fd: BorrowedFd<'_>, path: &std::path::Path, mask: u32) -> io::Result<i32> {
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: an open inotify fd (borrowed) and a valid C string.
    check(unsafe { libc::inotify_add_watch(fd.as_raw_fd(), path.as_ptr(), mask) })
}

// ---- V4L2 ---------------------------------------------------------------------------------------------------------

/// `struct v4l2_capability` (linux/videodev2.h), what VIDIOC_QUERYCAP fills.
#[repr(C)]
struct V4l2Capability {
    driver: [u8; 16],
    card: [u8; 32],
    bus_info: [u8; 32],
    version: u32,
    capabilities: u32,
    device_caps: u32,
    reserved: [u32; 3],
}

const V4L2_CAP_VIDEO_CAPTURE: u32 = 0x0000_0001;
const V4L2_CAP_DEVICE_CAPS: u32 = 0x8000_0000;

/// Whether the V4L2 device open as `fd` captures video (its own capabilities where it tells them apart from its
/// driver's, as C++'s `hasCamera` asks); an error if it is no V4L2 device.
pub fn v4l2_captures_video(fd: BorrowedFd<'_>) -> io::Result<bool> {
    let mut caps = V4l2Capability {
        driver: [0; 16],
        card: [0; 32],
        bus_info: [0; 32],
        version: 0,
        capabilities: 0,
        device_caps: 0,
        reserved: [0; 3],
    };
    let request = libc::_IOR::<V4l2Capability>(b'V' as u32, 0);
    // SAFETY: VIDIOC_QUERYCAP writes one v4l2_capability, `caps` (repr(C), the kernel's layout); the fd is open.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request, &mut caps as *mut V4l2Capability) })?;
    let caps = if caps.capabilities & V4L2_CAP_DEVICE_CAPS != 0 { caps.device_caps } else { caps.capabilities };
    Ok(caps & V4L2_CAP_VIDEO_CAPTURE != 0)
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

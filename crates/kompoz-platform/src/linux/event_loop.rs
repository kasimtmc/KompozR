//! The Linux platforms' event loop (C++'s Wayland and X11 loops' common part, on [`sys::poll`]): it waits
//! with `poll` on the file descriptors it watches (the display's connection, a D-Bus or clipboard fd), its wake
//! eventfd (written by [`Waker`] from any thread: the host's waker, a D-Bus thread, a signal), its deadline timerfd
//! (a coroutine's delay: [`Next::wake_at`](crate::app::Next::wake_at)) and its key-repeat timerfd (a key held,
//! repeated client-side as Wayland asks), and says which of them are ready.
//!
//! ```text
//! let mut lp = EventLoop::new()?;
//! lp.watch(display_fd, DISPLAY);
//! host_waker = lp.waker();             // PlatformCallbacks::waker
//! loop {
//!     lp.set_deadline(next.wake_at);
//!     let ready = lp.wait(None)?;      // (Some(Duration::ZERO): only what is ready now)
//!     if ready.source(DISPLAY) { … }   // read its events
//!     for _ in 0..ready.repeats { … }  // the held key again
//! }
//! ```

use super::sys::{self, PollFd};
use std::io;
use std::os::fd::{AsFd, AsRawFd, OwnedFd, RawFd};
use std::sync::Arc;
use std::time::Duration;

/// Wakes an [`EventLoop`] from any thread (its eventfd written). Cheap to clone; `Send` and `Sync`.
#[derive(Clone)]
pub struct Waker(Arc<OwnedFd>);

impl Waker {
    /// Wakes the loop: its `wait` returns with [`Ready::woken`] (now, or at once if it isn't waiting).
    pub fn wake(&self) {
        let _ = sys::eventfd_write(self.0.as_fd(), 1);
    }

    /// As the host's waker ([`PlatformCallbacks::waker`](kompoz_ui::platform::PlatformCallbacks::waker)).
    pub fn as_fn(&self) -> Arc<dyn Fn() + Send + Sync> {
        let waker = self.clone();
        Arc::new(move || waker.wake())
    }
}

/// What woke a [`wait`](EventLoop::wait).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Ready {
    /// A [`Waker`] (or a signal) woke it.
    pub woken: bool,
    /// The deadline came.
    pub deadline: bool,
    /// The held key's repeats due since the last wait (more than one if the loop was late).
    pub repeats: u64,
    /// The watched sources that are ready (their tokens).
    pub sources: Vec<u32>,
}

impl Ready {
    /// Whether the source with `token` is ready.
    pub fn source(&self, token: u32) -> bool {
        self.sources.contains(&token)
    }

    /// Whether anything is.
    pub fn any(&self) -> bool {
        self.woken || self.deadline || self.repeats > 0 || !self.sources.is_empty()
    }
}

/// The loop: its eventfd and timerfds, and the file descriptors it watches (each by a token).
pub struct EventLoop {
    wake: Arc<OwnedFd>,
    deadline: OwnedFd,
    repeat: OwnedFd,
    sources: Vec<(RawFd, u32)>,
    signals: bool,
}

impl EventLoop {
    pub fn new() -> io::Result<EventLoop> {
        Ok(EventLoop {
            wake: Arc::new(sys::eventfd()?),
            deadline: sys::timerfd()?,
            repeat: sys::timerfd()?,
            sources: Vec::new(),
            signals: false,
        })
    }

    /// What wakes it from other threads.
    pub fn waker(&self) -> Waker {
        Waker(self.wake.clone())
    }

    /// Watches `fd` (to read) as `token`: ready ones are in [`Ready::sources`]. The fd must stay open while it is
    /// watched; another watch of the token replaces it.
    pub fn watch(&mut self, fd: RawFd, token: u32) {
        self.unwatch(token);
        self.sources.push((fd, token));
    }

    pub fn unwatch(&mut self, token: u32) {
        self.sources.retain(|&(_, t)| t != token);
    }

    /// Wakes `wait` at `at` (the monotonic clock, ns: [`sys::monotonic_now`]'s), or not (none).
    pub fn set_deadline(&self, at: Option<i64>) {
        let _ = sys::timerfd_set_at(self.deadline.as_fd(), at);
    }

    /// Repeats the held key: the first repeat after `delay`, then every `interval`.
    pub fn start_repeat(&self, delay: Duration, interval: Duration) {
        let _ = sys::timerfd_set_repeating(self.repeat.as_fd(), delay.max(Duration::from_nanos(1)), interval);
    }

    /// No key repeats any more.
    pub fn stop_repeat(&self) {
        let _ = sys::timerfd_set_repeating(self.repeat.as_fd(), Duration::ZERO, Duration::ZERO);
        let _ = sys::timerfd_read(self.repeat.as_fd()); // (repeats that came meanwhile, dropped)
    }

    /// SIGINT and SIGTERM wake it as a quit ([`EventLoop::take_quit_signal`]): `sys::quit_on_signals` on its
    /// eventfd, for as long as it lives.
    pub fn quit_on_signals(&mut self) -> io::Result<()> {
        sys::quit_on_signals(self.wake.as_raw_fd())?;
        self.signals = true;
        Ok(())
    }

    /// Whether SIGINT or SIGTERM came since this was last asked.
    pub fn take_quit_signal(&self) -> bool {
        sys::take_quit_signal()
    }

    /// Waits until something is ready, at most `timeout` (none: as long as it takes; zero: only what is ready
    /// now), and says what: the wake's counter, the deadline and the repeats are taken (the deadline disarmed).
    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<Ready> {
        let mut fds = vec![
            PollFd::readable(self.wake.as_raw_fd()),
            PollFd::readable(self.deadline.as_raw_fd()),
            PollFd::readable(self.repeat.as_raw_fd()),
        ];
        fds.extend(self.sources.iter().map(|&(fd, _)| PollFd::readable(fd)));
        sys::poll(&mut fds, timeout)?;
        let mut ready = Ready::default();
        if fds[0].ready {
            ready.woken = sys::eventfd_read(self.wake.as_fd())? > 0;
        }
        if fds[1].ready && sys::timerfd_read(self.deadline.as_fd())? > 0 {
            ready.deadline = true;
            self.set_deadline(None);
        }
        if fds[2].ready {
            ready.repeats = sys::timerfd_read(self.repeat.as_fd())?;
        }
        ready.sources = fds[3..].iter().zip(&self.sources).filter(|(f, _)| f.ready).map(|(_, &(_, t))| t).collect();
        Ok(ready)
    }
}

impl Drop for EventLoop {
    fn drop(&mut self) {
        if self.signals {
            sys::forget_signal_wake_fd(self.wake.as_raw_fd());
        }
    }
}

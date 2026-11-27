//! What wakes a platform's loop from any thread (the host's waker, a D-Bus thread, an image loaded): an eventfd
//! written, which Linux's event loop polls (`linux::event_loop`) and Android's looper
//! watches.

use super::sys;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;

/// Wakes a loop from any thread (its eventfd written). Cheap to clone; `Send` and `Sync`.
#[derive(Clone)]
pub struct Waker(Arc<OwnedFd>);

impl Waker {
    /// The waker of the loop reading `eventfd` (one [`sys::eventfd`] made).
    pub fn new(eventfd: Arc<OwnedFd>) -> Waker {
        Waker(eventfd)
    }

    /// Wakes the loop: its wait ends (now, or at once if it isn't waiting).
    pub fn wake(&self) {
        let _ = sys::eventfd_write(self.0.as_fd(), 1);
    }

    /// As the host's waker ([`PlatformCallbacks::waker`](kompoz_ui::platform::PlatformCallbacks::waker)).
    pub fn as_fn(&self) -> Arc<dyn Fn() + Send + Sync> {
        let waker = self.clone();
        Arc::new(move || waker.wake())
    }

    /// The eventfd it writes (for a loop to watch).
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

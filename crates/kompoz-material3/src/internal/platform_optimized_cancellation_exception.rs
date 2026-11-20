//! A cancellation without the cost of a stack trace, as Compose's internal PlatformOptimizedCancellationException.kt
//! (its JVM actual: a `CancellationException` whose `fillInStackTrace` fills in none — cancellations are thrown
//! often, as control flow, and Android 6 and older crashed cloning a missing stack).
//!
//! KompozR's coroutines cancel without exceptions: a coroutine "throws" one by cancelling itself
//! ([`kompoz_coroutines::cancel_current`], its future dropped where it waits, its destructors its `finally`), and
//! a caller catching one gets a [`kompoz_coroutines::Cancelled`] ([`kompoz_coroutines::catch_cancellation`]); no
//! stack trace is taken. What Compose's subclasses carry, their message, is kept here; throwing one is cancelling
//! the coroutine it is thrown in.

use kompoz_coroutines::cancel_current;
use std::fmt;

/// A cancellation of a message (Compose's abstract `PlatformOptimizedCancellationException(message)`: Rust has no
/// subclasses; a Kotlin subclass is this with its message).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct PlatformOptimizedCancellationException {
    message: Option<String>,
}

impl PlatformOptimizedCancellationException {
    /// One of `message` (Kotlin's default: none).
    pub(crate) fn new(message: Option<String>) -> PlatformOptimizedCancellationException {
        PlatformOptimizedCancellationException { message }
    }

    /// Its message (Kotlin's `message`).
    pub(crate) fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Throws it (Kotlin's `throw`): the coroutine awaiting this is cancelled, and nothing after runs.
    pub(crate) async fn throw<T>(self) -> T {
        cancel_current().await
    }
}

/// Kotlin's toString of a `CancellationException`: its class, then its message.
impl fmt::Display for PlatformOptimizedCancellationException {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.message {
            Some(message) => write!(f, "kotlinx.coroutines.CancellationException: {message}"),
            None => f.write_str("kotlinx.coroutines.CancellationException"),
        }
    }
}

impl std::error::Error for PlatformOptimizedCancellationException {}

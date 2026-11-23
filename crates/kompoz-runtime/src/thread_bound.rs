//! A value bound to the thread that made it, shareable as `Send + Sync` ([`ThreadBound`]): what lets a snapshot
//! state, a transition's state or an animation value hold an `Rc` (a composable lambda, a custom alignment), as
//! Compose's states hold any object — Compose touches them on the main thread only, and so does this, checked.

use std::fmt;
use std::mem::ManuallyDrop;
use std::thread::ThreadId;

/// `value` bound to the thread that made it (the UI thread, in practice): `Send + Sync` whatever `T` is, the value
/// reachable ([`get`](Self::get), `Clone`, `==`, `Debug`) only on that thread — elsewhere it panics. Dropped on
/// another thread, the value is leaked rather than dropped there (an `Rc`'s count is never touched off its thread).
pub struct ThreadBound<T> {
    value: ManuallyDrop<T>,
    thread: ThreadId,
}

// SAFETY: the value is reached only on the thread that made it (`get` checks, and every impl below goes through
// it); off that thread it is never touched, not even dropped (`Drop` leaks it there).
unsafe impl<T> Send for ThreadBound<T> {}
// SAFETY: as above: a shared reference on another thread can't reach the value.
unsafe impl<T> Sync for ThreadBound<T> {}

impl<T> ThreadBound<T> {
    /// `value`, bound to this thread.
    pub fn new(value: T) -> ThreadBound<T> {
        ThreadBound { value: ManuallyDrop::new(value), thread: std::thread::current().id() }
    }

    /// Whether this is the thread it is bound to.
    pub fn is_valid(&self) -> bool {
        std::thread::current().id() == self.thread
    }

    /// The value; panics off the thread it is bound to.
    pub fn get(&self) -> &T {
        assert!(self.is_valid(), "kompoz: a ThreadBound value reached off the thread that made it");
        &self.value
    }
}

impl<T> Drop for ThreadBound<T> {
    fn drop(&mut self) {
        if self.is_valid() {
            // SAFETY: dropped once, here, on its own thread; never reached again.
            unsafe { ManuallyDrop::drop(&mut self.value) }
        }
        // (else leaked: its thread alone may drop it)
    }
}

impl<T: Clone> Clone for ThreadBound<T> {
    fn clone(&self) -> Self {
        ThreadBound::new(self.get().clone())
    }
}

impl<T: PartialEq> PartialEq for ThreadBound<T> {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl<T: fmt::Debug> fmt::Debug for ThreadBound<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.get(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::ThreadBound;
    use std::rc::Rc;

    #[test]
    fn a_bound_value_is_reached_on_its_thread_only_and_never_dropped_elsewhere() {
        let rc = Rc::new(5);
        let bound = ThreadBound::new(rc.clone());
        assert_eq!(**bound.get(), 5);
        assert_eq!(Rc::strong_count(&rc), 2);
        let copy = bound.clone();
        assert_eq!(copy, bound);
        assert_eq!(Rc::strong_count(&rc), 3);
        drop(copy);
        assert_eq!(Rc::strong_count(&rc), 2);
        // Elsewhere: not reachable, and dropping it there leaks it rather than touching the count.
        let elsewhere = std::thread::spawn(move || {
            assert!(!bound.is_valid());
            let reached = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| **bound.get()));
            assert!(reached.is_err());
            drop(bound);
        });
        elsewhere.join().unwrap();
        assert_eq!(Rc::strong_count(&rc), 2);
    }
}

//! One mutation at a time, a newer one cancelling an older of no higher priority (Compose's MutatorMutex: public in
//! foundation, an internal copy in animation-core for Animatable). KompozR has one, here, public: foundation
//! re-exports it (Compose's `androidx.compose.foundation.MutatorMutex`), Animatable uses it.

use kompoz_coroutines::{CoroutineScope, cancel_current, coroutine_scope};
use kompoz_runtime::named;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::Future;
use std::rc::Rc;
use std::task::{Poll, Waker};

/// How a mutation stands against another (Compose's MutatePriority): a newer one cancels a running one of its
/// priority or lower, and is itself cancelled (never runs) when the running one's is higher.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum MutatePriority {
    /// What a program does on its own (an animation).
    #[default]
    Default,
    /// What the user does (a drag): it cancels default mutations, and they don't cancel it.
    UserInput,
    /// What the user can't interrupt.
    PreventUserInput,
}

/// A running mutation: its priority and the scope its block runs in (cancelled to interrupt it).
struct Mutator {
    id: u64,
    priority: MutatePriority,
    scope: CoroutineScope,
}

struct Waiter {
    waker: RefCell<Option<Waker>>,
    granted: Cell<bool>,
}

struct Inner {
    current: RefCell<Option<Mutator>>,
    /// The lock (Kotlin's Mutex, fair: handed to the first waiter).
    locked: Cell<bool>,
    waiters: RefCell<VecDeque<Rc<Waiter>>>,
    next_id: Cell<u64>,
}

/// Mutual exclusion of mutations of a state (Compose's MutatorMutex): [`mutate`](MutatorMutex::mutate) runs one
/// at a time; a new one cancels the running one if its priority is at least as high (that one's coroutine is
/// cancelled, as Compose's MutationInterruptedException — a CancellationException — cancels it), else it is itself
/// cancelled. The new one starts once the old one has let go. Clones are the same mutex.
#[derive(Clone)]
pub struct MutatorMutex {
    inner: Rc<Inner>,
}

/// Compose's `MutatorMutex()`.
pub fn MutatorMutex() -> MutatorMutex {
    MutatorMutex::new()
}

impl Default for MutatorMutex {
    fn default() -> Self {
        MutatorMutex::new()
    }
}

impl PartialEq for MutatorMutex {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl std::fmt::Debug for MutatorMutex {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("MutatorMutex")
    }
}

/// Lets go of the lock when dropped: to the first waiter, or unlocked.
struct LockGuard(Rc<Inner>);

impl Drop for LockGuard {
    fn drop(&mut self) {
        unlock(&self.0);
    }
}

fn unlock(inner: &Inner) {
    let next = inner.waiters.borrow_mut().pop_front();
    match next {
        Some(waiter) => {
            waiter.granted.set(true);
            let waker = waiter.waker.borrow_mut().take();
            if let Some(w) = waker {
                w.wake();
            }
        }
        None => inner.locked.set(false),
    }
}

/// Clears the mutex's current mutator if it is still this one (Compose's `finally { compareAndSet(mutator, null) }`).
struct ClearCurrent(Rc<Inner>, u64);

impl Drop for ClearCurrent {
    fn drop(&mut self) {
        let mut current = self.0.current.borrow_mut();
        if current.as_ref().is_some_and(|m| m.id == self.1) {
            *current = None;
        }
    }
}

impl MutatorMutex {
    pub fn new() -> MutatorMutex {
        MutatorMutex {
            inner: Rc::new(Inner {
                current: RefCell::new(None),
                locked: Cell::new(false),
                waiters: RefCell::new(VecDeque::new()),
                next_id: Cell::new(0),
            }),
        }
    }

    /// Makes `mutator` the current one, cancelling the old one; false (nothing changed) if the old one's priority
    /// is higher.
    fn try_mutate_or_cancel(&self, mutator: Mutator) -> bool {
        let old = {
            let mut current = self.inner.current.borrow_mut();
            if current.as_ref().is_some_and(|old| mutator.priority < old.priority) {
                return false;
            }
            current.replace(mutator)
        };
        // (its block is dropped where it waits, letting go of the lock)
        if let Some(old) = old {
            old.scope.cancel();
        }
        true
    }

    /// Waits for the lock (Kotlin's `Mutex.lock`, fair).
    async fn lock(&self) -> LockGuard {
        let inner = self.inner.clone();
        if !inner.locked.get() && inner.waiters.borrow().is_empty() {
            inner.locked.set(true);
            return LockGuard(inner);
        }
        let waiter = Rc::new(Waiter { waker: RefCell::new(None), granted: Cell::new(false) });
        inner.waiters.borrow_mut().push_back(waiter.clone());

        /// Gives the lock on (or leaves the queue) if dropped while waiting for it.
        struct Waiting(Rc<Inner>, Option<Rc<Waiter>>);
        impl Drop for Waiting {
            fn drop(&mut self) {
                if let Some(w) = self.1.take() {
                    if w.granted.get() {
                        unlock(&self.0);
                    } else {
                        self.0.waiters.borrow_mut().retain(|o| !Rc::ptr_eq(o, &w));
                    }
                }
            }
        }
        let mut waiting = Waiting(inner.clone(), Some(waiter.clone()));
        std::future::poll_fn(|cx| {
            if waiter.granted.get() {
                Poll::Ready(())
            } else {
                *waiter.waker.borrow_mut() = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await;
        waiting.1 = None;
        LockGuard(inner)
    }
}

#[named]
impl MutatorMutex {
    /// Runs `block` as the mutation of `priority` (Compose's mutate): the running one is cancelled if its priority
    /// is no higher, else this one is (its coroutine cancelled: `block` never runs); `block` runs once the old one
    /// has let go. Its value when it completes.
    pub fn mutate<Fut: Future + 'static>(
        &self,
        #[default(MutatePriority::Default)] priority: MutatePriority,
        block: Fut,
    ) -> impl Future<Output = Fut::Output> + 'static
    where
        Fut::Output: 'static,
    {
        let this = self.clone();
        async move {
            coroutine_scope(move |scope| async move {
                let id = this.inner.next_id.get();
                this.inner.next_id.set(id + 1);
                if !this.try_mutate_or_cancel(Mutator { id, priority, scope }) {
                    // (Compose's CancellationException("Current mutation had a higher priority"))
                    return cancel_current().await;
                }
                let _lock = this.lock().await;
                let _clear = ClearCurrent(this.inner.clone(), id);
                block.await
            })
            .await
        }
    }

    /// [`mutate`](MutatorMutex::mutate) with a receiver for `block` (Compose's mutateWith).
    pub fn mutate_with<T: 'static, F: FnOnce(T) -> Fut + 'static, Fut: Future + 'static>(
        &self,
        receiver: T,
        #[default(MutatePriority::Default)] priority: MutatePriority,
        block: F,
    ) -> impl Future<Output = Fut::Output> + 'static
    where
        Fut::Output: 'static,
    {
        let this = self.clone();
        async move {
            coroutine_scope(move |scope| async move {
                let id = this.inner.next_id.get();
                this.inner.next_id.set(id + 1);
                if !this.try_mutate_or_cancel(Mutator { id, priority, scope }) {
                    return cancel_current().await;
                }
                let _lock = this.lock().await;
                let _clear = ClearCurrent(this.inner.clone(), id);
                block(receiver).await
            })
            .await
        }
    }
}

impl MutatorMutex {
    /// Runs `block` now if no mutation holds the lock (Compose's tryMutate): whether it ran. It cancels nothing,
    /// and a mutation started meanwhile is not cancelled by it.
    pub fn try_mutate(&self, block: impl FnOnce()) -> bool {
        let did_lock = self.try_lock();
        if did_lock {
            let _unlock = LockGuard(self.inner.clone());
            block();
        }
        did_lock
    }

    /// Takes the lock if it is free (Compose's tryLock).
    #[doc(hidden)]
    pub fn try_lock(&self) -> bool {
        if self.inner.locked.get() {
            false
        } else {
            self.inner.locked.set(true);
            true
        }
    }

    /// Lets go of the lock (Compose's unlock).
    #[doc(hidden)]
    pub fn unlock(&self) {
        unlock(&self.inner);
    }
}

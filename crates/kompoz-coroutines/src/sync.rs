//! Synchronization primitives for coroutines, as kotlinx.coroutines' `kotlinx.coroutines.sync` package: [`Mutex`],
//! a suspending, fair mutual exclusion lock (what Compose's `SnackbarHostState` queues its snackbars with).

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::Future;
use std::rc::Rc;
use std::task::{Poll, Waker};

/// Who holds a [`Mutex`] (Kotlin's `owner: Any?`): any value, the same owner being the same `Rc` (Kotlin compares
/// owners by identity).
pub type MutexOwner = Rc<dyn Any>;

struct Waiter {
    owner: Option<MutexOwner>,
    waker: RefCell<Option<Waker>>,
    granted: Cell<bool>,
}

struct Inner {
    locked: Cell<bool>,
    /// The owner holding the lock (none for an anonymous holder).
    owner: RefCell<Option<MutexOwner>>,
    waiters: RefCell<VecDeque<Rc<Waiter>>>,
}

/// Mutual exclusion for coroutines (kotlinx.coroutines' `interface Mutex` and its `MutexImpl`): locked or unlocked;
/// a coroutine locking a locked mutex suspends until it is unlocked. Fair: it is handed to the coroutines waiting
/// in the order they came (first come, first served). Not reentrant: a coroutine locking a mutex it holds waits
/// forever (with an owner, it is told instead). A coroutine cancelled while waiting for it leaves the queue (one
/// it was handed to as it was cancelled hands it on). Clones are the same mutex.
///
/// (kotlinx's deprecated `onLock` select clause isn't ported: KompozR's coroutines have no `select`.)
#[derive(Clone)]
pub struct Mutex {
    inner: Rc<Inner>,
}

/// A new mutex, unlocked unless `locked` (kotlinx's `Mutex(locked: Boolean = false)`; a locked one has no owner).
///
/// Kotlin: `Mutex(locked: Boolean = false): Mutex`
#[allow(non_snake_case)]
pub fn Mutex(locked: bool) -> Mutex {
    Mutex::new(locked)
}

impl Default for Mutex {
    fn default() -> Self {
        Mutex::new(false)
    }
}

/// The same mutex (kotlinx's mutexes are compared by identity).
impl PartialEq for Mutex {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for Mutex {}

/// kotlinx's `MutexImpl.toString`: `Mutex@<hash>[isLocked=…,owner=…]` without the hash.
impl std::fmt::Debug for Mutex {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Mutex[isLocked={},owner={}]",
            self.is_locked(),
            if self.owner().is_some() { "<owner>" } else { "null" }
        )
    }
}

fn same_owner(a: &Option<MutexOwner>, b: &Option<MutexOwner>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

impl Mutex {
    /// A new mutex, unlocked unless `locked` (kotlinx's `Mutex(locked)`).
    pub fn new(locked: bool) -> Mutex {
        Mutex {
            inner: Rc::new(Inner {
                locked: Cell::new(locked),
                owner: RefCell::new(None),
                waiters: RefCell::new(VecDeque::new()),
            }),
        }
    }

    fn owner(&self) -> Option<MutexOwner> {
        self.inner.owner.borrow().clone()
    }

    /// Whether the mutex is locked (kotlinx's `isLocked`).
    pub fn is_locked(&self) -> bool {
        self.inner.locked.get()
    }

    /// Whether `owner` holds the mutex (kotlinx's `holdsLock(owner: Any)`).
    pub fn holds_lock(&self, owner: &MutexOwner) -> bool {
        self.is_locked() && same_owner(&self.owner(), &Some(owner.clone()))
    }

    /// Locks the mutex if it is unlocked and none waits for it: whether it did (kotlinx's `tryLock(owner: Any? =
    /// null)`). Panics (kotlinx's IllegalStateException) if `owner` already holds it.
    ///
    /// Kotlin: `tryLock(owner: Any? = null): Boolean`
    pub fn try_lock(&self, owner: Option<MutexOwner>) -> bool {
        let inner = &self.inner;
        if inner.locked.get() {
            if same_owner(&inner.owner.borrow(), &owner) {
                panic!("This mutex is already locked by the specified owner");
            }
            return false;
        }
        // (fair: a mutex being handed to a waiter is locked, so none waits on an unlocked one)
        inner.locked.set(true);
        *inner.owner.borrow_mut() = owner;
        true
    }

    /// Locks the mutex, suspending until it is unlocked if it is locked (kotlinx's suspending `lock(owner: Any? =
    /// null)`): fair, the waiting coroutines get it in the order they came. Cancelled while waiting (its future
    /// dropped), the coroutine leaves the queue; handed the mutex as it is cancelled, it unlocks it. Panics if
    /// `owner` already holds it (kotlinx's IllegalStateException).
    ///
    /// Kotlin: `lock(owner: Any? = null)`
    pub fn lock(&self, owner: Option<MutexOwner>) -> impl Future<Output = ()> + 'static + use<> {
        let this = self.clone();
        async move {
            if this.try_lock(owner.clone()) {
                return;
            }
            let inner = this.inner.clone();
            let waiter = Rc::new(Waiter { owner, waker: RefCell::new(None), granted: Cell::new(false) });
            inner.waiters.borrow_mut().push_back(waiter.clone());

            /// Hands the mutex on (or leaves the queue) if dropped while waiting for it.
            struct Waiting(Rc<Inner>, Option<Rc<Waiter>>);
            impl Drop for Waiting {
                fn drop(&mut self) {
                    if let Some(w) = self.1.take() {
                        if w.granted.get() {
                            unlock_inner(&self.0);
                        } else {
                            self.0.waiters.borrow_mut().retain(|o| !Rc::ptr_eq(o, &w));
                        }
                    }
                }
            }
            let mut waiting = Waiting(inner, Some(waiter.clone()));
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
        }
    }

    /// Unlocks the mutex, handing it to the first coroutine waiting (kotlinx's `unlock(owner: Any? = null)`).
    /// Panics (kotlinx's IllegalStateException) if it is unlocked, or held by an owner other than `owner`.
    ///
    /// Kotlin: `unlock(owner: Any? = null)`
    pub fn unlock(&self, owner: Option<MutexOwner>) {
        let inner = &self.inner;
        if !inner.locked.get() {
            panic!("This mutex is not locked");
        }
        let held_by = inner.owner.borrow().clone();
        if let Some(held_by) = &held_by {
            match &owner {
                Some(owner) if Rc::ptr_eq(owner, held_by) => {}
                None => {}
                Some(_) => panic!("This mutex is locked by another owner, not the specified one"),
            }
        }
        unlock_inner(inner);
    }

    /// Runs `action` holding the mutex: locks it (suspending while it is locked), runs it, and unlocks it however
    /// it ends — done, failed or cancelled (kotlinx's `Mutex.withLock(owner: Any? = null, action)`).
    ///
    /// Kotlin: `withLock(owner: Any? = null, action: suspend () -> T): T`
    pub fn with_lock<T: 'static, Fut: Future<Output = T> + 'static, F: FnOnce() -> Fut + 'static>(
        &self,
        owner: Option<MutexOwner>,
        action: F,
    ) -> impl Future<Output = T> + 'static + use<T, Fut, F> {
        let this = self.clone();
        async move {
            this.lock(owner.clone()).await;
            /// kotlinx's `finally { unlock(owner) }`.
            struct Unlock(Mutex, Option<MutexOwner>);
            impl Drop for Unlock {
                fn drop(&mut self) {
                    self.0.unlock(self.1.take());
                }
            }
            let _unlock = Unlock(this, owner);
            action().await
        }
    }
}

/// Hands the mutex to the first waiter (it stays locked, owned by that waiter's owner), or unlocks it.
fn unlock_inner(inner: &Inner) {
    let next = inner.waiters.borrow_mut().pop_front();
    match next {
        Some(waiter) => {
            *inner.owner.borrow_mut() = waiter.owner.clone();
            waiter.granted.set(true);
            let waker = waiter.waker.borrow_mut().take();
            if let Some(w) = waker {
                w.wake();
            }
        }
        None => {
            *inner.owner.borrow_mut() = None;
            inner.locked.set(false);
        }
    }
}

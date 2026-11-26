//! A local `var` Kotlin's lambdas and local functions share (stage 14): [`Ref`].
//!
//! Kotlin's lambda captures a local `var` by reference: what it assigns is the local's new value outside, what
//! the code outside assigns it sees. A Rust closure the callee keeps (`onClick`, a coroutine, a local function)
//! can't borrow the local, so the Kotlin syntax holds such a `var` — one a kept lambda or a local function assigns —
//! in a `Ref`, a shared cell: the closure captures a clone of it (the same cell), reads and writes go through it
//! (`KtDelegate::get_value`, `KtMutableDelegate::set_value`, as a delegated local's do).

use super::delegate::{KtDelegate, KtMutableDelegate};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// A local `var` shared by the lambdas that capture it (see the module's doc); clones are the same variable.
pub struct Ref<T>(Rc<RefCell<T>>);

impl<T> Ref<T> {
    /// The variable, holding `value`.
    pub fn new(value: T) -> Ref<T> {
        Ref(Rc::new(RefCell::new(value)))
    }

    /// Assigns it.
    pub fn set(&self, value: T) {
        *self.0.borrow_mut() = value;
    }

    /// What `f` gives of its value, lent.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.0.borrow())
    }

    /// Changes its value in place (`list.add(x)` on a captured list).
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.0.borrow_mut())
    }
}

impl<T: Clone> Ref<T> {
    /// Its value (a clone: Kotlin's reference to the object it holds).
    pub fn get(&self) -> T {
        self.0.borrow().clone()
    }
}

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        Ref(self.0.clone())
    }
}

impl<T: fmt::Debug> fmt::Debug for Ref<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.borrow().fmt(f)
    }
}

impl<T: Clone> KtDelegate for Ref<T> {
    type Value = T;
    fn get_value(&self) -> T {
        self.get()
    }
}

impl<T: Clone> KtMutableDelegate for Ref<T> {
    fn set_value(&self, value: T) {
        self.set(value);
    }
}

/// A property's backing field (Kotlin's `field` in its accessors) read and set as a delegate is.
impl<T: Clone> KtDelegate for super::KtVar<T> {
    type Value = T;
    fn get_value(&self) -> T {
        self.get()
    }
}

impl<T: Clone> KtMutableDelegate for super::KtVar<T> {
    fn set_value(&self, value: T) {
        self.set(value);
    }
}

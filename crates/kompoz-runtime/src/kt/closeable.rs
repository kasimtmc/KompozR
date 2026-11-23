//! Kotlin's `AutoCloseable` (kotlin.AutoCloseable): a resource closed when it is done with (what a ViewModel closes
//! as it is cleared).

use std::any::Any;
use std::rc::Rc;

/// A resource closed when done with (Kotlin's `AutoCloseable`, a fun interface): a closure `|| …` is one, run as it
/// is closed.
pub trait AutoCloseable: Any {
    /// Closes it (Kotlin's `close()`).
    fn close(&self);
}

impl<F: Fn() + 'static> AutoCloseable for F {
    fn close(&self) {
        self()
    }
}

/// A resource closed by `close_action` (Kotlin's `AutoCloseable { … }`).
#[allow(non_snake_case)]
pub fn AutoCloseable(close_action: impl Fn() + 'static) -> Rc<dyn AutoCloseable> {
    Rc::new(close_action)
}

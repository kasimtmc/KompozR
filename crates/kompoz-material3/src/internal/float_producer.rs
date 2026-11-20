//! A float given when asked for, as Compose's internal FloatProducer.kt (a `fun interface`): what a component reads
//! as it lays out or draws (a progress, an animated value) without composing again. As kompoz-graphics'
//! `ColorProducer` is Compose's fun interface `ColorProducer`: a shared closure, equal only to itself.

use std::fmt;
use std::rc::Rc;

/// A float given when it is asked for (Compose's internal `fun interface FloatProducer`). Equal only to itself (a
/// clone of it).
#[derive(Clone)]
pub(crate) struct FloatProducer(Rc<dyn Fn() -> f32>);

impl FloatProducer {
    /// The producer `produce` is (Kotlin's SAM conversion of a lambda).
    pub(crate) fn new(produce: impl Fn() -> f32 + 'static) -> FloatProducer {
        FloatProducer(Rc::new(produce))
    }

    /// The float now (Compose's `invoke`).
    pub(crate) fn invoke(&self) -> f32 {
        (self.0)()
    }
}

impl<F: Fn() -> f32 + 'static> From<F> for FloatProducer {
    fn from(produce: F) -> FloatProducer {
        FloatProducer::new(produce)
    }
}

impl PartialEq for FloatProducer {
    fn eq(&self, other: &FloatProducer) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for FloatProducer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FloatProducer({:p})", Rc::as_ptr(&self.0) as *const ())
    }
}

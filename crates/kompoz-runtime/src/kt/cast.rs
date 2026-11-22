//! Kotlin's casts: `x as T` ([`KtAs::kt_as`]: a ClassCastException when `x` isn't a `T`) and `x as? T`
//! ([`KtAs::kt_as_safe`]: null then). A value is a `T` when it is one already (a clone of it), or when the crate of a
//! shared trait object's trait says which concrete types its objects can be (kompoz-graphics: an `Arc<dyn Shape>` as a
//! `RoundedCornerShape`, …).

use super::panic_kt;

/// A value cast to `T`, as Kotlin's `as` / `as?` (implemented for `T` itself, and by the crates of shared trait
/// objects for the concrete types they hold).
pub trait KtAs<T> {
    /// It as a `T`, if it is one (Kotlin's `x as? T`).
    fn kt_as_safe(&self) -> Option<T>;

    /// It as a `T` (Kotlin's `x as T`): a ClassCastException if it isn't one.
    #[track_caller]
    fn kt_as(&self) -> T {
        match self.kt_as_safe() {
            Some(value) => value,
            None => {
                panic_kt("ClassCastException", &format!("the value cannot be cast to {}", std::any::type_name::<T>()))
            }
        }
    }
}

impl<T: Clone> KtAs<T> for T {
    fn kt_as_safe(&self) -> Option<T> {
        Some(self.clone())
    }
}

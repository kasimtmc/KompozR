//! `kotlin.reflect` as navigation needs it: a type as a value (`typeOf<T>()`, a typed route's `typeMap` keys).

use std::any::TypeId;
use std::fmt;

/// A type (Kotlin's `KType`): `type_of::<T>()`. `T` and `Option<T>` are different types, as Kotlin's `T` and
/// `T?` are.
#[derive(Clone, Copy)]
pub struct KType {
    id: TypeId,
    name: &'static str,
}

impl KType {
    /// Its Rust name (`alloc::string::String`).
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Whether it is an `Option<…>` (Kotlin's `isMarkedNullable`).
    pub fn is_marked_nullable(&self) -> bool {
        self.name.starts_with("core::option::Option<")
    }

    /// Its `TypeId`.
    pub fn type_id(&self) -> TypeId {
        self.id
    }
}

impl PartialEq for KType {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for KType {}

impl std::hash::Hash for KType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl fmt::Debug for KType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

impl fmt::Display for KType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

/// The type `T` (Kotlin's `typeOf<T>()`).
pub fn type_of<T: ?Sized + 'static>() -> KType {
    KType { id: TypeId::of::<T>(), name: std::any::type_name::<T>() }
}

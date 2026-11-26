//! What Kotlin's classes and objects are made of in the Rust the Kotlin syntax writes (stage 14): [`KtObject`]
//! (Kotlin's `Any`: a value whose class can be asked, `is`/`as` on it), the cells a class's properties live in
//! ([`KtVal`], [`KtVar`]: set as the object is made, in Kotlin's order of initializers and `init` blocks), a data
//! class's `var` property ([`KtShared`]: shared by the copies Rust makes of the value, as Kotlin's one object is),
//! an object declaration's single instance ([`KtSingleton`]), and Kotlin's default `toString()` of a class
//! ([`default_to_string`]).

use super::text::KtToString;
use super::{KtAs, panic_kt};
use std::any::Any;
use std::cell::{OnceCell, RefCell};
use std::fmt;
use std::rc::Rc;

/// Kotlin's `Any`: a value whose class can be asked at run time (`x is T`, `x as T`). Every `'static` type is one;
/// a value held as `Rc<dyn KtObject>` (what Kotlin types `Any`) is cast with [`<dyn KtObject>::kt_as`].
pub trait KtObject: Any {
    /// The value as Rust's `Any` (what a cast downcasts).
    fn as_any(&self) -> &dyn Any;

    /// Its class's name (Rust's type name, the last segment).
    fn kt_class_name(&self) -> &'static str {
        let full = std::any::type_name_of_val(self.as_any());
        let path = full.split('<').next().unwrap_or(full);
        path.rsplit("::").next().unwrap_or(path)
    }
}

impl<T: Any> KtObject for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn kt_class_name(&self) -> &'static str {
        let full = std::any::type_name::<T>();
        let path = full.split('<').next().unwrap_or(full);
        path.rsplit("::").next().unwrap_or(path)
    }
}

impl dyn KtObject {
    /// Whether it is a `T` (Kotlin's `x is T`).
    pub fn kt_is<T: Any>(&self) -> bool {
        self.as_any().is::<T>()
    }

    /// It as a `T`, if it is one (Kotlin's `x as? T`: a clone of it, Kotlin's reference being the object's).
    pub fn kt_as<T: Any + Clone>(&self) -> Option<T> {
        self.as_any().downcast_ref::<T>().cloned()
    }
}

/// A value of Kotlin's `Any` cast (`Rc<dyn KtObject>` as a `T`: Kotlin's `x as? T`).
pub fn kt_as_object<T: Any + Clone>(value: &Rc<dyn KtObject>) -> Option<T> {
    (**value).kt_as::<T>()
}

/// Kotlin's `x is T` of a value: [`KtAs`]'s cast succeeding.
pub fn kt_is<T, V: KtAs<T> + ?Sized>(value: &V) -> bool {
    value.kt_as_safe().is_some()
}

/// A `val` property of a class: set once while the object is made (its initializer, or the constructor's
/// parameter), read after. Reading it before (a member function an `init` block calls reading a property declared
/// after it) is an error, where Kotlin reads the type's default.
pub struct KtVal<T>(OnceCell<T>);

impl<T> KtVal<T> {
    pub const fn new() -> KtVal<T> {
        KtVal(OnceCell::new())
    }

    /// Sets it (as the object is made).
    pub fn init(&self, value: T) {
        if self.0.set(value).is_err() {
            panic_kt("IllegalStateException", "a val property set twice");
        }
    }

    /// Its value, lent.
    #[track_caller]
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        match self.0.get() {
            Some(v) => f(v),
            None => panic_kt("UninitializedPropertyAccessException", "a property read before it was initialized"),
        }
    }

    /// Its value (Kotlin's value shared: a clone).
    #[track_caller]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }
}

impl<T> Default for KtVal<T> {
    fn default() -> Self {
        KtVal::new()
    }
}

/// A `var` property of a class: set while the object is made, read and written after.
pub struct KtVar<T>(RefCell<Option<T>>);

impl<T> KtVar<T> {
    pub const fn new() -> KtVar<T> {
        KtVar(RefCell::new(None))
    }

    /// Sets it (as the object is made, or later: Kotlin's assignment).
    pub fn set(&self, value: T) {
        *self.0.borrow_mut() = Some(value);
    }

    /// Its value, lent (not across a write of it).
    #[track_caller]
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        match &*self.0.borrow() {
            Some(v) => f(v),
            None => panic_kt("UninitializedPropertyAccessException", "a property read before it was initialized"),
        }
    }

    /// Its value (a clone).
    #[track_caller]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }
}

impl<T> Default for KtVar<T> {
    fn default() -> Self {
        KtVar::new()
    }
}

/// A data class's `var` property: the copies Rust makes of the value (Kotlin's references to one object) share it,
/// so a write through one is seen through the others; `==`, `toString()` and `copy()` see its value.
pub struct KtShared<T>(Rc<RefCell<T>>);

impl<T> KtShared<T> {
    pub fn new(value: T) -> KtShared<T> {
        KtShared(Rc::new(RefCell::new(value)))
    }

    pub fn set(&self, value: T) {
        *self.0.borrow_mut() = value;
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.0.borrow().clone()
    }
}

impl<T> Clone for KtShared<T> {
    fn clone(&self) -> Self {
        KtShared(self.0.clone())
    }
}

impl<T: PartialEq> PartialEq for KtShared<T> {
    fn eq(&self, other: &Self) -> bool {
        *self.0.borrow() == *other.0.borrow()
    }
}

impl<T: KtToString> KtToString for KtShared<T> {
    fn kt_write(&self, out: &mut String) {
        self.0.borrow().kt_write(out);
    }
}

impl<T: fmt::Debug> fmt::Debug for KtShared<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.borrow().fmt(f)
    }
}

/// An object declaration's instance, per thread (Kotlin's object is made at its first use; the Kotlin syntax's
/// state is the thread's, as its top-level properties are): `with` gives it, made by `make` (which stores it before
/// running its initializers, so they may use the object, as Kotlin's do) the first time.
pub struct KtSingleton<F: 'static>(OnceCell<Rc<F>>);

impl<F: 'static> KtSingleton<F> {
    pub const fn new() -> KtSingleton<F> {
        KtSingleton(OnceCell::new())
    }

    /// The instance: made with `empty` and initialized with `init` the first time.
    pub fn get(&self, empty: impl FnOnce() -> F, init: impl FnOnce(&Rc<F>)) -> Rc<F> {
        if let Some(f) = self.0.get() {
            return f.clone();
        }
        let f = Rc::new(empty());
        let _ = self.0.set(f.clone());
        init(&f);
        f
    }
}

impl<F: 'static> Default for KtSingleton<F> {
    fn default() -> Self {
        KtSingleton::new()
    }
}

/// Kotlin's default `toString()` of an object of class `name` (`Ledger@1b6d3586`): its class's name and an
/// identity hash (the object's address here).
pub fn default_to_string(name: &str, address: *const ()) -> String {
    format!("{name}@{:x}", (address as usize) & 0xffff_ffff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_values_cast() {
        let v: Rc<dyn KtObject> = Rc::new(5_i32);
        assert!(v.kt_is::<i32>() && !v.kt_is::<String>());
        assert_eq!(kt_as_object::<i32>(&v), Some(5));
        assert_eq!(kt_as_object::<String>(&v), None);
        // (through the `Rc`: an `Rc` is a `KtObject` of its own)
        assert_eq!((*v).kt_class_name(), "i32");
        assert!(kt_is::<i32, _>(&3_i32));
    }

    #[test]
    fn cells() {
        let v = KtVal::new();
        v.init(String::from("a"));
        assert_eq!(v.get(), "a");
        let w = KtVar::new();
        w.set(1);
        w.set(w.get() + 1);
        assert_eq!(w.get(), 2);
        let s = KtShared::new(1);
        let t = s.clone();
        t.set(5);
        assert_eq!(s.get(), 5);
        assert!(s == KtShared::new(5));
        assert_eq!(s.kt_to_string(), "5");
    }

    #[test]
    fn a_singleton_is_made_once_and_seen_while_made() {
        thread_local! { static S: KtSingleton<KtVar<i32>> = const { KtSingleton::new() }; }
        let first = S.with(|s| s.get(KtVar::new, |f| f.set(10)));
        first.set(11);
        let again = S.with(|s| s.get(KtVar::new, |f| f.set(99)));
        assert_eq!(again.get(), 11);
    }
}

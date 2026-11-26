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

/// Kotlin's `x is T`/`x as? T` of a value of `Any` (`&*x` of an `Rc<dyn KtObject>`): the object's class.
impl<T: Any + Clone> KtAs<T> for dyn KtObject {
    fn kt_as_safe(&self) -> Option<T> {
        self.kt_as::<T>()
    }
}

/// Kotlin's `toString()` of a value of `Any`: a string's, a number's, a boolean's or a char's text, else its
/// class's name and an identity hash (a value of `Any` whose text its class gives is held as its interface's).
impl KtToString for dyn KtObject {
    fn kt_write(&self, out: &mut String) {
        let any = self.as_any();
        macro_rules! shown {
            ($($t:ty),*) => {$(
                if let Some(v) = any.downcast_ref::<$t>() {
                    return v.kt_write(out);
                }
            )*};
        }
        shown!(String, &'static str, i32, i64, f32, f64, bool, char, u8, u16, u32, u64, i8, i16, usize);
        out.push_str(&default_to_string(self.kt_class_name(), any as *const dyn Any as *const ()));
    }
}

/// Kotlin's `==` of two values of `Any` (a data class's property of `Any`, `TopLevelRoute(route: Any, …)`): the same
/// object, or equal strings, numbers, booleans or chars (their classes' `equals`); values of other classes are equal
/// when they are the one object (their `equals` isn't known through `Any` here).
/// (Of any lifetimes: a derived `==` of a field `Rc<dyn KtObject>` compares through them.)
impl<'a, 'b> PartialEq<dyn KtObject + 'b> for dyn KtObject + 'a {
    fn eq(&self, other: &(dyn KtObject + 'b)) -> bool {
        let (a, b) = (self.as_any(), other.as_any());
        if std::ptr::addr_eq(a as *const dyn Any, b as *const dyn Any) {
            return true;
        }
        macro_rules! same {
            ($($t:ty),*) => {$(
                if let (Some(x), Some(y)) = (a.downcast_ref::<$t>(), b.downcast_ref::<$t>()) {
                    return x == y;
                }
            )*};
        }
        same!(String, &'static str, i32, i64, f32, f64, bool, char, u8, u16, u32, u64, i8, i16, usize);
        false
    }
}

impl fmt::Display for dyn KtObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.kt_to_string())
    }
}

/// Kotlin's `==` of a value (`None`: null) with one of `Any`/`Any?` (`Money(5) == m`, `m == Money(5)`): the
/// value's class's `equals` — the other a value of its class, equal to it —; null equal to null only.
#[doc(hidden)]
pub fn eq_object<T: Any + PartialEq>(value: Option<&T>, other: Option<&dyn KtObject>) -> bool {
    match (value, other) {
        (None, None) => true,
        (Some(v), Some(o)) => o.as_any().downcast_ref::<T>() == Some(v),
        _ => false,
    }
}

/// Kotlin's `hashCode()` of a value of the API's or the standard library's types (stage 14 FINAL-T): Kotlin's own
/// for numbers, booleans, chars, strings (Java's `s[0]*31^(n-1) + …`) and lists (`31 * h + e.hashCode()`); a type
/// of the API its own, consistent with its `==`. A user's class's is its `Hash` ([`hash_code_of`]) or its own
/// `hashCode()`.
#[doc(hidden)]
pub trait KtHashCode {
    fn kt_hash_code(&self) -> i32;
}

impl KtHashCode for i32 {
    fn kt_hash_code(&self) -> i32 {
        *self
    }
}

impl KtHashCode for i64 {
    fn kt_hash_code(&self) -> i32 {
        (*self ^ (*self >> 32)) as i32
    }
}

impl KtHashCode for bool {
    fn kt_hash_code(&self) -> i32 {
        if *self { 1231 } else { 1237 }
    }
}

impl KtHashCode for char {
    fn kt_hash_code(&self) -> i32 {
        *self as i32
    }
}

impl KtHashCode for f32 {
    fn kt_hash_code(&self) -> i32 {
        super::KtBits::kt_bits(self) as i32
    }
}

impl KtHashCode for f64 {
    fn kt_hash_code(&self) -> i32 {
        let b = super::KtBits::kt_bits(self);
        (b ^ (b >> 32)) as i32
    }
}

impl KtHashCode for String {
    fn kt_hash_code(&self) -> i32 {
        self.as_str().kt_hash_code()
    }
}

impl KtHashCode for str {
    fn kt_hash_code(&self) -> i32 {
        self.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
    }
}

impl<T: KtHashCode> KtHashCode for Vec<T> {
    fn kt_hash_code(&self) -> i32 {
        self.iter().fold(1i32, |h, e| h.wrapping_mul(31).wrapping_add(e.kt_hash_code()))
    }
}

impl<T: KtHashCode> KtHashCode for Option<T> {
    fn kt_hash_code(&self) -> i32 {
        self.as_ref().map_or(0, KtHashCode::kt_hash_code)
    }
}

impl<T: KtHashCode + ?Sized> KtHashCode for Rc<T> {
    fn kt_hash_code(&self) -> i32 {
        (**self).kt_hash_code()
    }
}

/// Kotlin's `hashCode()` of a value whose `==` Rust's `Hash` agrees with (a user's data class's, enum's): its hash.
#[doc(hidden)]
pub fn hash_code_of<T: std::hash::Hash + ?Sized>(value: &T) -> i32 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut h);
    let v = h.finish();
    (v ^ (v >> 32)) as i32
}

/// A value of Kotlin's `Any` cast (`Rc<dyn KtObject>` as a `T`: Kotlin's `x as? T`).
pub fn kt_as_object<T: Any + Clone>(value: &Rc<dyn KtObject>) -> Option<T> {
    (**value).kt_as::<T>()
}

/// Kotlin's `Nothing` as a type argument (`Outcome<Nothing>`: a type no value has, Rust's `!` there being
/// unstable): what a value of every type is, so each of the traits Kotlin's values have (`==`, `toString()`,
/// ordering, hashing) is its.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum KtNothing {}

impl fmt::Display for KtNothing {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {}
    }
}

impl KtToString for KtNothing {
    fn kt_write(&self, _: &mut String) {
        match *self {}
    }
}

/// Kotlin's `this as? T` in a member of an interface or of an open, abstract or sealed class (a trait's provided
/// method: `Self` the object's own class, unsized there): the object, if it is a `T` (`this is Success` in a sealed
/// class's member, the case it is).
#[doc(hidden)]
pub fn kt_as_this<T: Any + Clone, V: KtObject + ?Sized>(value: &V) -> Option<T> {
    value.as_any().downcast_ref::<T>().cloned()
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

    /// Its value, lent for as long as the cell lives (a base class's part a class's object holds: a ViewModel's
    /// `ViewModelImpl`).
    #[track_caller]
    pub fn get_ref(&self) -> &T {
        match self.0.get() {
            Some(v) => v,
            None => panic_kt("UninitializedPropertyAccessException", "a property read before it was initialized"),
        }
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

/// Kotlin's `==` of two values of `Any` (`Any?`) as a data class holds them (its property's field: the shared object,
/// in an `Option`, in a `var`'s [`KtShared`]): what they hold compared (`dyn KtObject`'s `==`). A derived `==` of an
/// `Rc<dyn KtObject>` field isn't one Rust writes.
#[doc(hidden)]
pub trait KtEqAny {
    fn kt_eq_any(&self, other: &Self) -> bool;
}
impl KtEqAny for Rc<dyn KtObject> {
    fn kt_eq_any(&self, other: &Self) -> bool {
        **self == **other
    }
}
impl KtEqAny for Option<Rc<dyn KtObject>> {
    fn kt_eq_any(&self, other: &Self) -> bool {
        match (self, other) {
            (Some(a), Some(b)) => a.kt_eq_any(b),
            (a, b) => a.is_none() && b.is_none(),
        }
    }
}
impl<T: KtEqAny> KtEqAny for KtShared<T> {
    fn kt_eq_any(&self, other: &Self) -> bool {
        self.0.borrow().kt_eq_any(&other.0.borrow())
    }
}

/// [`KtEqAny`]'s `==`.
#[doc(hidden)]
pub fn eq_any<T: KtEqAny>(a: &T, b: &T) -> bool {
    a.kt_eq_any(b)
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

/// A data class's `var` property hashed as its value is (Kotlin's `hashCode()` of the data class reads it).
impl<T: std::hash::Hash> std::hash::Hash for KtShared<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.borrow().hash(state);
    }
}

impl<T: Eq> Eq for KtShared<T> {}

/// A data class's `Float`/`Double` property as Kotlin's generated `equals`/`hashCode` see it (stage 14): its bits
/// (`floatToIntBits`: every NaN one value, `0.0` and `-0.0` two), so `P(Float.NaN) == P(Float.NaN)` and
/// `P(0f) != P(-0f)`, as Kotlin's `Float.compare` makes them, and equal values hash alike.
#[doc(hidden)]
pub trait KtBits {
    type Bits: Eq + std::hash::Hash;
    fn kt_bits(&self) -> Self::Bits;
}

impl KtBits for f32 {
    type Bits = u32;
    fn kt_bits(&self) -> u32 {
        if self.is_nan() { f32::NAN.to_bits() } else { self.to_bits() }
    }
}

impl KtBits for f64 {
    type Bits = u64;
    fn kt_bits(&self) -> u64 {
        if self.is_nan() { f64::NAN.to_bits() } else { self.to_bits() }
    }
}

impl<T: KtBits> KtBits for Option<T> {
    type Bits = Option<T::Bits>;
    fn kt_bits(&self) -> Option<T::Bits> {
        self.as_ref().map(KtBits::kt_bits)
    }
}

impl<T: KtBits> KtBits for KtShared<T> {
    type Bits = T::Bits;
    fn kt_bits(&self) -> T::Bits {
        self.0.borrow().kt_bits()
    }
}

/// A `@Serializable` data class's `var` property saved as its value is.
impl<T: crate::Saveable> crate::Saveable for KtShared<T> {
    fn save(&self) -> crate::SavedValue {
        self.0.borrow().save()
    }
    fn restore(saved: &crate::SavedValue) -> Option<Self> {
        T::restore(saved).map(KtShared::new)
    }
    fn descriptor() -> crate::SerialDescriptor {
        T::descriptor()
    }
}

impl<T: fmt::Debug> fmt::Debug for KtShared<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.borrow().fmt(f)
    }
}

/// An object declaration's instance, per thread (Kotlin's object is made at its first use; the Kotlin syntax's
/// state is the thread's, as its top-level properties are): `get` gives it, made by `empty` (and stored before its
/// initializers run, so they may use the object, as Kotlin's do) the first time. It lives as long as the program
/// (Kotlin's object is never collected): a reference to it is `'static`, what a base class's part an object holds
/// (`fn view_model_impl(&self) -> &ViewModelImpl`) is lent from.
pub struct KtSingleton<F: 'static>(OnceCell<&'static F>);

impl<F: 'static> KtSingleton<F> {
    pub const fn new() -> KtSingleton<F> {
        KtSingleton(OnceCell::new())
    }

    /// The instance: made with `empty` and initialized with `init` the first time.
    pub fn get(&self, empty: impl FnOnce() -> F, init: impl FnOnce(&'static F)) -> &'static F {
        if let Some(f) = self.0.get() {
            return f;
        }
        let f: &'static F = Box::leak(Box::new(empty()));
        // (under Miri: kept reachable from a static, so its leak check sees the object Kotlin keeps for the
        // program's life as kept, not lost when the thread holding the cell ends)
        #[cfg(miri)]
        miri_keep(f as *const F as *const u8);
        let _ = self.0.set(f);
        init(f);
        f
    }
}

/// The objects [`KtSingleton`] made, reachable from a static (Miri's leak check).
#[cfg(miri)]
fn miri_keep(p: *const u8) {
    struct Kept(Vec<*const u8>);
    // SAFETY: the pointers are only kept, never read.
    unsafe impl Send for Kept {}
    static KEPT: std::sync::Mutex<Kept> = std::sync::Mutex::new(Kept(Vec::new()));
    KEPT.lock().unwrap_or_else(|e| e.into_inner()).0.push(p);
}

/// What a suspending member of a Kotlin interface gives in Rust (a trait's `async` method can't be called through a
/// trait object): its future, boxed.
pub type KtFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a>>;

/// A value of a Kotlin interface (a trait object: `value`, what its object is, and `cast`, the object's own answer
/// for the interfaces and sealed classes it is) as a `T`, if it is one: Kotlin's `x as? T` of an interface's value.
pub fn kt_dyn_as<T: Any + Clone>(value: &dyn Any, cast: impl FnOnce() -> Option<Box<dyn Any>>) -> Option<T> {
    if let Some(v) = value.downcast_ref::<T>() {
        return Some(v.clone());
    }
    cast().and_then(|b| b.downcast::<T>().ok()).map(|b| *b)
}

/// Whether two values are one object (Kotlin's `===`, and `==` of a class that doesn't override `equals`).
pub fn kt_same_object(a: &dyn Any, b: &dyn Any) -> bool {
    std::ptr::addr_eq(a as *const dyn Any, b as *const dyn Any)
}

/// Kotlin's `===` of an object of a class whose copies share one object (`identity`: its shared state's address)
/// and a value of `Any`/`Any?` (`this === other` in `equals`): the same object, `equals` never asked.
#[doc(hidden)]
pub fn kt_same_as<T: Any>(value: &T, other: Option<&dyn KtObject>, identity: fn(&T) -> *const ()) -> bool {
    other.and_then(|o| o.as_any().downcast_ref::<T>()).is_some_and(|o| identity(o) == identity(value))
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

    #[test]
    fn interface_values_cast() {
        #[derive(Clone, PartialEq, Debug)]
        struct A(i32);
        let a = A(1);
        assert_eq!(kt_dyn_as::<A>(&a, || None), Some(A(1)));
        assert_eq!(kt_dyn_as::<String>(&a, || Some(Box::new(String::from("as")))), Some("as".to_string()));
        assert_eq!(kt_dyn_as::<String>(&a, || None), None);
        let b = A(1);
        assert!(kt_same_object(&a, &a) && !kt_same_object(&a, &b));
    }
}

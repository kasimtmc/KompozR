//! Kotlin's `by` delegation (`var count by remember { mutableStateOf(0) }`): what a delegated property reads and
//! writes ([`KtDelegate`], [`KtMutableDelegate`]), `by lazy { … }` ([`lazy`]), `lateinit var` ([`KtLateinit`]),
//! and `x++`/`x--` on one ([`post_inc`]…, through [`KtInc`]).

use super::panic_kt;
use crate::{DerivedState, MutableState, State};
use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

/// What `val x by d` reads: `d.get_value()` (Kotlin's `getValue` operator), each time `x` is read.
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be a `by` delegate",
    label = "a `by` delegate is read with `getValue`",
    note = "a state (`mutableStateOf`, `derivedStateOf`, `animate…AsState`), `lazy {{ … }}`, or a type implementing `kt::KtDelegate`"
)]
pub trait KtDelegate {
    type Value;
    fn get_value(&self) -> Self::Value;
}

/// `d` as a delegate of `T`'s: a delegated property's declared type (`var checkedIndex: Int? by remember {
/// mutableStateOf(null) }`) telling what its delegate holds, which Rust can't take from `null` alone.
pub fn delegate_of<T, D: KtDelegate<Value = T>>(d: D) -> D {
    d
}

/// What `var x by d` also writes: `x = v` is `d.set_value(v)` (Kotlin's `setValue` operator).
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be a `var` delegate: it can't be set",
    label = "a `var` delegated to it is set with `setValue`",
    note = "a `MutableState` (`mutableStateOf`), or a type implementing `kt::KtMutableDelegate`; a read-only one is for `val`"
)]
pub trait KtMutableDelegate: KtDelegate {
    fn set_value(&self, value: Self::Value);
}

impl<T: Clone + 'static> KtDelegate for MutableState<T> {
    type Value = T;
    fn get_value(&self) -> T {
        self.get()
    }
}
impl<T: Clone + 'static> KtMutableDelegate for MutableState<T> {
    fn set_value(&self, value: T) {
        self.set(value);
    }
}
impl<T: Clone + 'static> KtDelegate for State<T> {
    type Value = T;
    fn get_value(&self) -> T {
        self.get()
    }
}
impl<T: Clone + 'static> KtDelegate for DerivedState<T> {
    type Value = T;
    fn get_value(&self) -> T {
        self.get()
    }
}
impl<D: KtDelegate + ?Sized> KtDelegate for &D {
    type Value = D::Value;
    fn get_value(&self) -> D::Value {
        (**self).get_value()
    }
}
impl<D: KtMutableDelegate + ?Sized> KtMutableDelegate for &D {
    fn set_value(&self, value: D::Value) {
        (**self).set_value(value);
    }
}
impl<D: KtDelegate + ?Sized> KtDelegate for Rc<D> {
    type Value = D::Value;
    fn get_value(&self) -> D::Value {
        (**self).get_value()
    }
}
impl<D: KtMutableDelegate + ?Sized> KtMutableDelegate for Rc<D> {
    fn set_value(&self, value: D::Value) {
        (**self).set_value(value);
    }
}

/// Kotlin's `ReadOnlyProperty { _, _ -> value }`: a delegate giving one value, made before (what `val items by
/// handle.saveable(saver) { mutableStateListOf() }` holds: viewmodel-compose's `PropertyDelegateProvider`'s).
pub struct KtValue<T>(pub T);

impl<T: Clone> KtDelegate for KtValue<T> {
    type Value = T;
    fn get_value(&self) -> T {
        self.0.clone()
    }
}

/// Kotlin's `by lazy { … }`: the value made the first time it is read, kept after. Copies share it (one value).
pub struct KtLazy<T> {
    inner: Rc<LazyInner<T>>,
}

struct LazyInner<T> {
    value: OnceCell<T>,
    make: RefCell<Option<Box<dyn FnOnce() -> T>>>,
}

impl<T> Clone for KtLazy<T> {
    fn clone(&self) -> Self {
        KtLazy { inner: self.inner.clone() }
    }
}

impl<T> KtLazy<T> {
    /// Kotlin: `isInitialized(): Boolean`
    pub fn is_initialized(&self) -> bool {
        self.inner.value.get().is_some()
    }
    /// The value, made now if it isn't yet.
    pub fn value_ref(&self) -> &T {
        self.inner.value.get_or_init(|| {
            let make = self.inner.make.borrow_mut().take().expect("a lazy value is made once");
            make()
        })
    }
}

impl<T: Clone> KtLazy<T> {
    /// Kotlin: `value: T`
    pub fn value(&self) -> T {
        self.value_ref().clone()
    }
}

impl<T: Clone> KtDelegate for KtLazy<T> {
    type Value = T;
    fn get_value(&self) -> T {
        self.value()
    }
}

/// Kotlin's `lazy { … }`.
/// Kotlin: `lazy(initializer: () -> T): Lazy<T>`
pub fn lazy<T: 'static>(initializer: impl FnOnce() -> T + 'static) -> KtLazy<T> {
    KtLazy { inner: Rc::new(LazyInner { value: OnceCell::new(), make: RefCell::new(Some(Box::new(initializer))) }) }
}

/// Kotlin's `lateinit var name: T`: set later; read before, a panic as Kotlin's
/// UninitializedPropertyAccessException. Copies share it.
pub struct KtLateinit<T> {
    value: Rc<RefCell<Option<T>>>,
    name: &'static str,
}

impl<T> Clone for KtLateinit<T> {
    fn clone(&self) -> Self {
        KtLateinit { value: self.value.clone(), name: self.name }
    }
}

impl<T> KtLateinit<T> {
    /// A property of this name, not set yet.
    pub fn new(name: &'static str) -> KtLateinit<T> {
        KtLateinit { value: Rc::new(RefCell::new(None)), name }
    }
    /// Kotlin's `::name.isInitialized`.
    pub fn is_initialized(&self) -> bool {
        self.value.borrow().is_some()
    }
}

impl<T: Clone> KtDelegate for KtLateinit<T> {
    type Value = T;
    #[track_caller]
    fn get_value(&self) -> T {
        match &*self.value.borrow() {
            Some(v) => v.clone(),
            None => panic_kt(
                "UninitializedPropertyAccessException",
                &format!("lateinit property {} has not been initialized", self.name),
            ),
        }
    }
}
impl<T: Clone> KtMutableDelegate for KtLateinit<T> {
    fn set_value(&self, value: T) {
        *self.value.borrow_mut() = Some(value);
    }
}

/// Kotlin's `inc()`/`dec()` operators: what `x++` and `x--` add and take (an `Int` wraps, as Kotlin's).
pub trait KtInc: Sized {
    fn inc(self) -> Self;
    fn dec(self) -> Self;
}

macro_rules! inc_ints {
    ($($t:ty),*) => {$(
        impl KtInc for $t {
            fn inc(self) -> $t {
                self.wrapping_add(1)
            }
            fn dec(self) -> $t {
                self.wrapping_sub(1)
            }
        }
    )*};
}
inc_ints!(i8, i16, i32, i64, u8, u16, u32, u64, isize, usize);

impl KtInc for f32 {
    fn inc(self) -> f32 {
        self + 1.0
    }
    fn dec(self) -> f32 {
        self - 1.0
    }
}
impl KtInc for f64 {
    fn inc(self) -> f64 {
        self + 1.0
    }
    fn dec(self) -> f64 {
        self - 1.0
    }
}
/// The next and previous code point (past the surrogates, which no `char` is).
impl KtInc for char {
    fn inc(self) -> char {
        char::from_u32(self as u32 + 1).or_else(|| char::from_u32(0xE000)).unwrap_or(self)
    }
    fn dec(self) -> char {
        char::from_u32((self as u32).wrapping_sub(1)).or_else(|| char::from_u32(0xD7FF)).unwrap_or(self)
    }
}

/// `x++` on a delegated `var`, as a value: the old value (the delegate set to the next).
pub fn post_inc<D: KtMutableDelegate>(delegate: &D) -> D::Value
where
    D::Value: KtInc + Clone,
{
    let v = delegate.get_value();
    delegate.set_value(v.clone().inc());
    v
}

/// `++x` on a delegated `var`, as a value: the new value.
pub fn pre_inc<D: KtMutableDelegate>(delegate: &D) -> D::Value
where
    D::Value: KtInc + Clone,
{
    let v = delegate.get_value().inc();
    delegate.set_value(v.clone());
    v
}

/// `x--` on a delegated `var`, as a value: the old value.
pub fn post_dec<D: KtMutableDelegate>(delegate: &D) -> D::Value
where
    D::Value: KtInc + Clone,
{
    let v = delegate.get_value();
    delegate.set_value(v.clone().dec());
    v
}

/// `--x` on a delegated `var`, as a value: the new value.
pub fn pre_dec<D: KtMutableDelegate>(delegate: &D) -> D::Value
where
    D::Value: KtInc + Clone,
{
    let v = delegate.get_value().dec();
    delegate.set_value(v.clone());
    v
}

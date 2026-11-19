//! Kotlin's null and scope functions: `a?.f()` ([`safe`], [`safe!`](crate::kt::safe)), `a ?: b` ([`elvis`]),
//! `a!!` ([`not_null`]), `let`/`also`/`apply`/`run`/`takeIf`/`takeUnless` ([`KtScope`]), `run { }`, `with(x) { }`.
//! A nullable is an `Option`.

use super::panic_kt;
use std::cell::Cell;

/// Kotlin's `a?.f()`: `f` of the value when there is one. What `f` gives is made nullable; when it is nullable
/// already, the result is that (`Option<Option<U>>`), which [`safe!`](crate::kt::safe) flattens.
pub fn safe<T, R>(value: Option<T>, f: impl FnOnce(T) -> R) -> Option<R> {
    value.map(f)
}

/// What `a ?: b` gives for the `b` it is given: a value of `a`'s type (`a ?: 0`: not nullable) or a nullable one
/// (`a ?: b` with `b` nullable too: nullable).
pub trait KtElvis<T> {
    type Out;
    fn kt_from_value(value: T) -> Self::Out;
    fn kt_from_other(self) -> Self::Out;
}
impl<T> KtElvis<T> for T {
    type Out = T;
    fn kt_from_value(value: T) -> T {
        value
    }
    fn kt_from_other(self) -> T {
        self
    }
}
impl<T> KtElvis<T> for Option<T> {
    type Out = Option<T>;
    fn kt_from_value(value: T) -> Option<T> {
        Some(value)
    }
    fn kt_from_other(self) -> Option<T> {
        self
    }
}

/// Kotlin's `a ?: b`: `a`'s value, else `b` (made only then). `a ?: return`, `?: throw`, `?: error(…)` are a
/// `match` the generator writes, as their `b` never gives a value.
pub fn elvis<T, R: KtElvis<T>>(value: Option<T>, otherwise: impl FnOnce() -> R) -> R::Out {
    match value {
        Some(v) => R::kt_from_value(v),
        None => otherwise().kt_from_other(),
    }
}

/// Kotlin's `a!!`: the value, else a panic as Kotlin's NullPointerException.
#[track_caller]
pub fn not_null<T>(value: Option<T>) -> T {
    match value {
        Some(v) => v,
        None => panic_kt("NullPointerException", "`!!` on a null value"),
    }
}

/// What [`safe!`](crate::kt::safe) flattens through: `(&KtNullProbe::new(x)).kt_flatten()` is `x` as one
/// nullable, `Option<Option<U>>` made `Option<U>`.
#[doc(hidden)]
pub struct KtNullProbe<T>(Cell<Option<T>>);

impl<T> KtNullProbe<T> {
    pub fn new(value: T) -> KtNullProbe<T> {
        KtNullProbe(Cell::new(Some(value)))
    }
}

#[doc(hidden)]
pub trait KtFlattenNested {
    type Out;
    fn kt_flatten(&self) -> Self::Out;
}
impl<U> KtFlattenNested for KtNullProbe<Option<Option<U>>> {
    type Out = Option<U>;
    fn kt_flatten(&self) -> Option<U> {
        self.0.take().flatten().flatten()
    }
}

#[doc(hidden)]
pub trait KtFlattenPlain {
    type Out;
    fn kt_flatten(&self) -> Self::Out;
}
impl<R> KtFlattenPlain for &KtNullProbe<Option<R>> {
    type Out = Option<R>;
    fn kt_flatten(&self) -> Option<R> {
        self.0.take().flatten()
    }
}

/// Kotlin's scope functions, on every value: `x.r#let { … }`, `x.also { … }`, `x.apply { … }` (the block gets the
/// value as its receiver, `&mut`), `x.run { … }`, `x.take_if { … }`, `x.take_unless { … }`. A lambda that gets the
/// value as `it` gets it itself (a clone where the value is kept too).
#[allow(async_fn_in_trait)]
pub trait KtScope: Sized {
    /// Kotlin: `let(block: (T) -> R): R`
    fn r#let<R>(self, block: impl FnOnce(Self) -> R) -> R {
        block(self)
    }
    /// Kotlin: `also(block: (T) -> Unit): T`
    fn also(self, block: impl FnOnce(Self)) -> Self
    where
        Self: Clone,
    {
        block(self.clone());
        self
    }
    /// Kotlin: `apply(block: T.() -> Unit): T`
    fn apply(mut self, block: impl FnOnce(&mut Self)) -> Self {
        block(&mut self);
        self
    }
    /// Kotlin: `run(block: T.() -> R): R`
    fn run<R>(mut self, block: impl FnOnce(&mut Self) -> R) -> R {
        block(&mut self)
    }
    /// Kotlin: `takeIf(predicate: (T) -> Boolean): T?`
    fn take_if(self, predicate: impl FnOnce(Self) -> bool) -> Option<Self>
    where
        Self: Clone,
    {
        if predicate(self.clone()) { Some(self) } else { None }
    }
    /// Kotlin: `takeUnless(predicate: (T) -> Boolean): T?`
    fn take_unless(self, predicate: impl FnOnce(Self) -> bool) -> Option<Self>
    where
        Self: Clone,
    {
        if predicate(self.clone()) { None } else { Some(self) }
    }

    // ---- The suspending twins (see the kt module's doc): Kotlin's inline functions, their lambdas async closures ----

    #[doc(hidden)]
    async fn let_async<R>(self, block: impl AsyncFnOnce(Self) -> R) -> R {
        block(self).await
    }
    #[doc(hidden)]
    async fn also_async(self, block: impl AsyncFnOnce(Self)) -> Self
    where
        Self: Clone,
    {
        block(self.clone()).await;
        self
    }
    #[doc(hidden)]
    async fn apply_async(mut self, block: impl AsyncFnOnce(&mut Self)) -> Self {
        block(&mut self).await;
        self
    }
    #[doc(hidden)]
    async fn run_async<R>(mut self, block: impl AsyncFnOnce(&mut Self) -> R) -> R {
        block(&mut self).await
    }
    #[doc(hidden)]
    async fn take_if_async(self, predicate: impl AsyncFnOnce(Self) -> bool) -> Option<Self>
    where
        Self: Clone,
    {
        if predicate(self.clone()).await { Some(self) } else { None }
    }
    #[doc(hidden)]
    async fn take_unless_async(self, predicate: impl AsyncFnOnce(Self) -> bool) -> Option<Self>
    where
        Self: Clone,
    {
        if predicate(self.clone()).await { None } else { Some(self) }
    }
}

impl<T> KtScope for T {}

/// Kotlin's `run { … }` (no receiver): the block's value.
/// Kotlin: `run(block: () -> R): R`
pub fn run<R>(block: impl FnOnce() -> R) -> R {
    block()
}

/// Kotlin's `with(x) { … }`: the block with `x` as its receiver.
/// Kotlin: `with(receiver: T, block: T.() -> R): R`
pub fn with<T, R>(mut receiver: T, block: impl FnOnce(&mut T) -> R) -> R {
    block(&mut receiver)
}

/// [`run`]'s suspending twin (see the kt module's doc).
#[doc(hidden)]
pub async fn run_async<R>(block: impl AsyncFnOnce() -> R) -> R {
    block().await
}

/// [`with`]'s suspending twin (see the kt module's doc).
#[doc(hidden)]
pub async fn with_async<T, R>(mut receiver: T, block: impl AsyncFnOnce(&mut T) -> R) -> R {
    block(&mut receiver).await
}

/// [`safe`] whose `f` suspends (`a?.f()` in a coroutine, `f` a suspending call: Kotlin's safe call is no function, its
/// call suspends with its caller).
#[doc(hidden)]
pub async fn safe_async<T, R>(value: Option<T>, f: impl AsyncFnOnce(T) -> R) -> Option<R> {
    match value {
        Some(v) => Some(f(v).await),
        None => None,
    }
}

/// [`elvis`] whose `otherwise` suspends (`a ?: load()` in a coroutine).
#[doc(hidden)]
pub async fn elvis_async<T, R: KtElvis<T>>(value: Option<T>, otherwise: impl AsyncFnOnce() -> R) -> R::Out {
    match value {
        Some(v) => R::kt_from_value(v),
        None => otherwise().await.kt_from_other(),
    }
}

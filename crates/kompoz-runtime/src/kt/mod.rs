//! Kotlin's standard library as the Kotlin syntax (`kompoz! { … }`) needs it — what C++ Kompoz's `kotlin.hpp` is to
//! kompozc — with Kotlin's meanings where Rust's differ: values written as Kotlin writes them ([`KtToString`],
//! [`template!`]: `"3.0"`, `[a, b]`, `null`), `by` delegates ([`KtDelegate`]), ranges ([`range_to`], [`until`],
//! [`down_to`], [`step`], [`is_in`]), strings in UTF-16 units ([`KtStr`], [`KtChar`], [`format()`]),
//! collections ([`list_of`], [`KtList`], [`KtMutableList`], [`KtMap`], [`KtMapExt`]), nullables ([`safe`],
//! [`elvis`], [`not_null`]), scope functions ([`KtScope`]), numbers and `kotlin.math` ([`KtNumber`], [`KtFloat`]),
//! a Rust `Result` as Kotlin's `Result` ([`KtResult`]: `getOrThrow()`, …), casts ([`KtAs`]: `as`, `as?`), [`require`],
//! [`check`], [`error`].
//!
//! Kotlin's exceptions are panics whose message is what Kotlin's `toString()` of the exception says
//! (`IllegalStateException: Check failed.`), at the caller's line (`#[track_caller]`); Kotlin's `try` catches them
//! as the [`Throwable`]s they are ([`throwable`]: this needs `panic = "unwind"`, Rust's default).
//!
//! The extension traits are not in the crate's prelude (a `Vec`'s `first()` would become Kotlin's): code that wants
//! them imports [`ext`]'s glob (`use kompoz_runtime::kt::ext::*;`), as the Kotlin syntax's generated code does.
//!
//! Each function Kotlin calls by another name, or whose Kotlin parameters have defaults, says so in a doc line
//! ``Kotlin: `name(param: Type = default, …)` ``: the API index (`cargo xtask kotlin-index`) reads it, and the
//! generator fills a call's missing arguments from those defaults (Kotlin expressions). A Rust `Option<T>` parameter
//! takes `Some(v)` for a Kotlin value given and `None` for null.
//!
//! Kotlin's inline functions (`with`, `let`, `run`, `apply`, `also`, `takeIf`, `repeat`, `forEach`, `map`, `filter`,
//! `fold`, `getOrPut`, `require`, …) run their lambdas in place: in a coroutine such a lambda suspends with its caller
//! (`with(anim) { animateTo(1f) }`, `repeat(3) { delay(10) }`). Each shim of one has a suspending twin, `#[doc(hidden)]`
//! and named `<name>_async` next to it (in its trait, its impl, or its module), taking the lambdas as async closures
//! (`AsyncFnOnce`/`AsyncFnMut`) and otherwise the same: same parameters in the same order, same evaluation order, same
//! results. The generator calls it, awaited, where a lambda given to the shim suspends; the API index marks the shims
//! that have one (`ASYNC_TWIN`). Kotlin's `crossinline` lambdas (`sortedBy`), and the functions Kotlin doesn't inline
//! (`removeAll(predicate)`, `joinToString(transform)`, `lazy`), have none: their lambdas can't suspend, as in Kotlin;
//! where only some of Kotlin's overloads take theirs `crossinline` (`withStyle(ParagraphStyle)`), the twin bounds the
//! parameter telling them apart by a narrower trait (`impl SuspendingBuilderStyle`), which the index reads.
//! A twin's exception is at the shim's line (an `async fn` can't be `#[track_caller]`), as a Kotlin stack trace has
//! an inline function's own line on top. Compose's own inline functions follow the convention in their crates
//! (`build_annotated_string_async`, `AnnotatedStringBuilder::with_style_async`, `take_or_else_async`,
//! `TextFieldState::edit_async`, the draw transforms'…); a `#[named]` one's twin is a `fn` giving an `impl Future`
//! (what `#[named]` takes), with the same defaults, so a call naming its arguments goes through its builder too.

pub mod cast;
// (stage 13 WP1: what a ViewModel closes)
pub mod closeable;
pub mod collections;
pub mod delegate;
pub mod format;
// (WP6: pickers) java.time's dates, what Compose's JVM date picker APIs take.
pub mod java_time;
pub mod math;
pub mod null;
// (stage 14 WP0: Kotlin's classes and objects)
pub mod object;
pub mod random;
pub mod range;
// (stage 14 WPB: a local `var` shared by the lambdas capturing it)
pub mod reference;
// (stage 13: navigation's typeMap keys) kotlin.reflect's typeOf
pub mod reflect;
// (WP4: Navigation 3's deep links) Kotlin's Regex, Java's syntax.
pub mod regex;
pub mod result;
pub mod string;
pub mod text;
// (stage 14 WPB: Kotlin's exceptions on unwinding)
pub mod throwable;

#[cfg(test)]
mod tests;

pub use cast::KtAs;
pub use closeable::AutoCloseable;
pub use collections::{
    KtComponent3, KtComponent4, KtComponent5, KtComponents, KtIterable, KtList, KtListOfLists, KtListOfNullable, KtMap,
    KtMapExt, KtMapLike, KtMutableList, KtPair, KtSet, KtTriple, List, MutableList, Pair, Triple, array_list_of,
    empty_list, empty_map, empty_set, float_array_of, int_array_of, list_of, list_of_not_null, map_of, mutable_list_of,
    mutable_map_of, mutable_set_of, set_of, to,
};
#[doc(hidden)]
pub use collections::{List_async, MutableList_async};
pub use delegate::{
    KtDelegate, KtInc, KtLateinit, KtLazy, KtMutableDelegate, delegate_of, lazy, post_dec, post_inc, pre_dec, pre_inc,
};
pub use format::format;
pub use java_time::{DayOfWeek, LocalDate, YearMonth};
pub use math::{
    E, KtAbs, KtFloat, KtNumber, KtSum, PI, abs, acos, asin, atan, atan2, cbrt, ceil, cos, cosh, exp, floor, hypot, ln,
    log2, log10, max, max_of, min, min_of, pow, round, sign, sin, sinh, sqrt, tan, tanh, to_char, truncate,
};
pub use null::{KtElvis, KtScope, elvis, not_null, run, safe, with};
#[doc(hidden)]
pub use null::{elvis_async, run_async, safe_async, with_async};
pub use object::{
    KtFuture, KtNothing, KtObject, KtShared, KtSingleton, KtVal, KtVar, default_to_string, kt_as_object, kt_as_this,
    kt_dyn_as, kt_is, kt_same_object,
};
pub use random::{KtShuffle, Random};
pub use range::{
    IndexedValue, KtClosedRange, KtContains, KtProgression, KtProgressionIter, KtRangeBound, KtStep, down_to,
    eq_nullable, is_in, range_to, step, to_closed_range, until,
};
pub use reference::Ref;
pub use reflect::{KType, type_of};
pub use regex::{KtToRegex, MatchGroup, MatchGroupCollection, MatchResult, Regex, RegexOption};
pub use result::KtResult;
pub use string::{KtChar, KtDelimiters, KtNeedle, KtNullableStr, KtStr};
pub use text::{KtDisplay, KtToString, to_string};
pub use throwable::{
    Jump, KtExceptionArg, KtThrowable, OnCancel, Throwable, catching, catching_async, exception, is_standard_class,
    parent_class, rethrow, throw,
};

#[doc(hidden)]
pub use null::{KtFlattenNested, KtFlattenPlain, KtNullProbe};
#[doc(hidden)]
pub use text::{FormatArg, KtProbe, KtViaDisplay, KtViaToString};

/// The extension traits, imported for their methods only (`use kompoz_runtime::kt::ext::*;`).
pub mod ext {
    pub use super::collections::{
        KtComponent3 as _, KtComponent4 as _, KtComponent5 as _, KtComponents as _, KtList as _, KtListOfLists as _,
        KtListOfNullable as _, KtMapExt as _, KtMutableList as _, KtPair as _, KtTriple as _,
    };
    pub use super::delegate::{KtDelegate as _, KtInc as _, KtMutableDelegate as _};
    pub use super::math::{KtFloat as _, KtNumber as _};
    pub use super::null::KtScope as _;
    pub use super::random::KtShuffle as _;
    pub use super::regex::KtToRegex as _;
    pub use super::result::KtResult as _;
    pub use super::string::{KtChar as _, KtNullableStr as _, KtStr as _};
    pub use super::text::KtToString as _;
}

/// Panics as Kotlin throws `kind` with `message` (none when empty): `IllegalStateException: message`, the panic's
/// text; a Kotlin `catch` takes it as that class's exception ([`throwable`]).
#[track_caller]
pub fn panic_kt(kind: &str, message: &str) -> ! {
    let message = if message.is_empty() { None } else { Some(message.to_string()) };
    throw(Throwable::new(kind, message, None))
}

/// Kotlin's `require(value)`: an IllegalArgumentException (`Failed requirement.`, or the message's text) when false.
/// Kotlin: `require(value: Boolean, lazyMessage: (() -> Any)? = null)`
#[track_caller]
pub fn require(value: bool, lazy_message: Option<&dyn Fn() -> String>) {
    if !value {
        panic_kt("IllegalArgumentException", &lazy_message.map_or_else(|| "Failed requirement.".to_string(), |m| m()));
    }
}

/// Kotlin's `check(value)`: an IllegalStateException (`Check failed.`, or the message's text) when false.
/// Kotlin: `check(value: Boolean, lazyMessage: (() -> Any)? = null)`
#[track_caller]
pub fn check(value: bool, lazy_message: Option<&dyn Fn() -> String>) {
    if !value {
        panic_kt("IllegalStateException", &lazy_message.map_or_else(|| "Check failed.".to_string(), |m| m()));
    }
}

/// [`require`]'s suspending twin (see the module's doc): its message's lambda an async closure.
#[doc(hidden)]
pub async fn require_async(value: bool, lazy_message: Option<&impl AsyncFn() -> String>) {
    if !value {
        let message = match lazy_message {
            Some(m) => m().await,
            None => "Failed requirement.".to_string(),
        };
        panic_kt("IllegalArgumentException", &message);
    }
}

/// [`check`]'s suspending twin (see the module's doc).
#[doc(hidden)]
pub async fn check_async(value: bool, lazy_message: Option<&impl AsyncFn() -> String>) {
    if !value {
        let message = match lazy_message {
            Some(m) => m().await,
            None => "Check failed.".to_string(),
        };
        panic_kt("IllegalStateException", &message);
    }
}

/// Kotlin's `requireNotNull(value)`: the value, else an IllegalArgumentException.
/// Kotlin: `requireNotNull(value: T?, lazyMessage: (() -> Any)? = null): T`
#[track_caller]
pub fn require_not_null<T>(value: Option<T>, lazy_message: Option<&dyn Fn() -> String>) -> T {
    match value {
        Some(v) => v,
        None => panic_kt(
            "IllegalArgumentException",
            &lazy_message.map_or_else(|| "Required value was null.".to_string(), |m| m()),
        ),
    }
}

/// Kotlin's `checkNotNull(value)`: the value, else an IllegalStateException.
/// Kotlin: `checkNotNull(value: T?, lazyMessage: (() -> Any)? = null): T`
#[track_caller]
pub fn check_not_null<T>(value: Option<T>, lazy_message: Option<&dyn Fn() -> String>) -> T {
    match value {
        Some(v) => v,
        None => panic_kt(
            "IllegalStateException",
            &lazy_message.map_or_else(|| "Required value was null.".to_string(), |m| m()),
        ),
    }
}

/// [`require_not_null`]'s suspending twin (see the module's doc).
#[doc(hidden)]
pub async fn require_not_null_async<T>(value: Option<T>, lazy_message: Option<&impl AsyncFn() -> String>) -> T {
    match value {
        Some(v) => v,
        None => {
            let message = match lazy_message {
                Some(m) => m().await,
                None => "Required value was null.".to_string(),
            };
            panic_kt("IllegalArgumentException", &message)
        }
    }
}

/// [`check_not_null`]'s suspending twin (see the module's doc).
#[doc(hidden)]
pub async fn check_not_null_async<T>(value: Option<T>, lazy_message: Option<&impl AsyncFn() -> String>) -> T {
    match value {
        Some(v) => v,
        None => {
            let message = match lazy_message {
                Some(m) => m().await,
                None => "Required value was null.".to_string(),
            };
            panic_kt("IllegalStateException", &message)
        }
    }
}

/// Kotlin's `error(message)`: an IllegalStateException, never returning (a value of any type: `x ?: error("…")`
/// is a `match` whose arm is this).
/// Kotlin: `error(message: Any): Nothing`
#[track_caller]
pub fn error(message: impl KtToString) -> ! {
    panic_kt("IllegalStateException", &message.kt_to_string())
}

/// Kotlin's `TODO()`: a NotImplementedError.
/// Kotlin: `TODO(reason: String? = null): Nothing`
#[track_caller]
#[allow(non_snake_case)]
pub fn TODO(reason: Option<&str>) -> ! {
    match reason {
        Some(r) => panic_kt("NotImplementedError", &format!("An operation is not implemented: {r}")),
        None => panic_kt("NotImplementedError", "An operation is not implemented."),
    }
}

/// Kotlin's `repeat(times) { i -> … }`.
/// Kotlin: `repeat(times: Int, action: (Int) -> Unit)`
pub fn repeat(times: i32, action: impl FnMut(i32)) {
    (0..times.max(0)).for_each(action);
}

/// [`repeat`]'s suspending twin (see the module's doc).
#[doc(hidden)]
pub async fn repeat_async(times: i32, mut action: impl AsyncFnMut(i32)) {
    for i in 0..times.max(0) {
        action(i).await;
    }
}

/// Kotlin's `println(message)`: its text (Kotlin's `toString()`) and a line end, on the standard output.
/// Kotlin: `println(message: Any?)`
pub fn println(message: impl KtToString) {
    println!("{}", message.kt_to_string());
}

/// Kotlin's `println()`: a line end.
/// Kotlin: `println()`
pub fn println_empty() {
    println!();
}

/// Kotlin's `print(message)`.
/// Kotlin: `print(message: Any?)`
pub fn print(message: impl KtToString) {
    print!("{}", message.kt_to_string());
}

/// Kotlin's string template `"Count: $count, ${a + b}"`: the pieces' texts, each as Kotlin writes it (its
/// [`KtToString`], else its `Display`): `template!("Count: ", count)`.
#[macro_export]
#[doc(hidden)]
macro_rules! __kt_template {
    ($($piece:expr),* $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::kt::{KtViaDisplay as _, KtViaToString as _};
        let mut __out = ::std::string::String::new();
        $( (&$crate::kt::KtProbe(&$piece)).kt_write_probe(&mut __out); )*
        __out
    }};
}

/// Kotlin's `a?.f()` flattened: `safe!(a, |it| it.f())` is `f`'s value or null, one nullable however nullable `f`'s
/// value is (`Option<Option<U>>` is `Option<U>`, as Kotlin's `T?` is).
#[macro_export]
#[doc(hidden)]
macro_rules! __kt_safe {
    ($value:expr, $f:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::kt::{KtFlattenNested as _, KtFlattenPlain as _};
        (&$crate::kt::KtNullProbe::new($crate::kt::safe($value, $f))).kt_flatten()
    }};
}

/// `x` as one nullable: `Option<Option<U>>` made `Option<U>`, any other `Option` as it is.
#[macro_export]
#[doc(hidden)]
macro_rules! __kt_flatten {
    ($value:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::kt::{KtFlattenNested as _, KtFlattenPlain as _};
        (&$crate::kt::KtNullProbe::new($value)).kt_flatten()
    }};
}

/// A name the Kotlin syntax reads without seeing its declaration (a top-level `val` of another `kompoz!` or `.kt`
/// file, a Rust `thread_local!`, constant or static): its value as Kotlin reads it — a thread-local's (a translated
/// top-level `val` is one) cloned out of it, any other value cloned (Kotlin's values are shared).
#[macro_export]
#[doc(hidden)]
macro_rules! __kt_top_level {
    ($name:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::kt::{KtTopLevelKey as _, KtTopLevelValue as _};
        (&$crate::kt::KtTopLevelProbe(&$name)).kt_top_level()
    }};
}

/// `f` of such a name's value where it is (`LocalSpacing.current` of another file's local: a static's in place, a
/// thread-local's inside it): `top_level_with!(LocalSpacing, |v| v.current())`.
#[macro_export]
#[doc(hidden)]
macro_rules! __kt_top_level_with {
    ($name:expr, $f:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::kt::{KtTopLevelKeyWith as _, KtTopLevelValueWith as _};
        (&$crate::kt::KtTopLevelProbe(&$name)).kt_top_level_with($f)
    }};
}

/// What [`top_level!`] reads a value through (a thread-local's is found first, a plain value's one autoref later).
#[doc(hidden)]
pub struct KtTopLevelProbe<'a, T: ?Sized>(pub &'a T);

/// A thread-local's value, cloned out of it.
#[doc(hidden)]
pub trait KtTopLevelKey {
    type Value;
    fn kt_top_level(&self) -> Self::Value;
}

impl<T: Clone + 'static> KtTopLevelKey for KtTopLevelProbe<'static, std::thread::LocalKey<T>> {
    type Value = T;
    fn kt_top_level(&self) -> T {
        self.0.with(T::clone)
    }
}

/// Any other value, cloned.
#[doc(hidden)]
pub trait KtTopLevelValue {
    type Value;
    fn kt_top_level(&self) -> Self::Value;
}

impl<T: Clone> KtTopLevelValue for &KtTopLevelProbe<'_, T> {
    type Value = T;
    fn kt_top_level(&self) -> T {
        self.0.clone()
    }
}

/// A thread-local's value, lent.
#[doc(hidden)]
pub trait KtTopLevelKeyWith {
    type Value;
    fn kt_top_level_with<R>(&self, f: impl FnOnce(&Self::Value) -> R) -> R;
}

impl<T: 'static> KtTopLevelKeyWith for KtTopLevelProbe<'static, std::thread::LocalKey<T>> {
    type Value = T;
    fn kt_top_level_with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.0.with(f)
    }
}

/// Any other value, lent where it is.
#[doc(hidden)]
pub trait KtTopLevelValueWith {
    type Value;
    fn kt_top_level_with<R>(&self, f: impl FnOnce(&Self::Value) -> R) -> R;
}

impl<T> KtTopLevelValueWith for &KtTopLevelProbe<'_, T> {
    type Value = T;
    fn kt_top_level_with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(self.0)
    }
}

pub use crate::__kt_flatten as flatten;
pub use crate::__kt_safe as safe;
pub use crate::__kt_template as template;
pub use crate::__kt_top_level as top_level;
pub use crate::__kt_top_level_with as top_level_with;

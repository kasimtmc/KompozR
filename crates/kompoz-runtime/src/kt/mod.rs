//! Kotlin's standard library as the Kotlin syntax (`kompoz! { … }`) needs it — what C++ Kompoz's `kotlin.hpp` is to
//! kompozc — with Kotlin's meanings where Rust's differ: values written as Kotlin writes them ([`KtToString`],
//! [`template!`]: `"3.0"`, `[a, b]`, `null`), `by` delegates ([`KtDelegate`]), ranges ([`range_to`], [`until`],
//! [`down_to`], [`step`], [`is_in`]), strings in UTF-16 units ([`KtStr`], [`KtChar`], [`format()`]),
//! collections ([`list_of`], [`KtList`], [`KtMutableList`], [`KtMap`], [`KtMapExt`]), nullables ([`safe`],
//! [`elvis`], [`not_null`]), scope functions ([`KtScope`]), numbers and `kotlin.math` ([`KtNumber`], [`KtFloat`]),
//! [`require`], [`check`], [`error`].
//!
//! Kotlin's exceptions are panics whose message is what Kotlin's `toString()` of the exception says
//! (`IllegalStateException: Check failed.`), at the caller's line (`#[track_caller]`).
//!
//! The extension traits are not in the crate's prelude (a `Vec`'s `first()` would become Kotlin's): code that wants
//! them imports [`ext`]'s glob (`use kompoz_runtime::kt::ext::*;`), as the Kotlin syntax's generated code does.
//!
//! Each function Kotlin calls by another name, or whose Kotlin parameters have defaults, says so in a doc line
//! ``Kotlin: `name(param: Type = default, …)` ``: the API index (`cargo xtask kotlin-index`) reads it, and the
//! generator fills a call's missing arguments from those defaults (Kotlin expressions). A Rust `Option<T>` parameter
//! takes `Some(v)` for a Kotlin value given and `None` for null.

pub mod collections;
pub mod delegate;
pub mod format;
pub mod math;
pub mod null;
pub mod range;
pub mod string;
pub mod text;

#[cfg(test)]
mod tests;

pub use collections::{
    KtComponent3, KtComponents, KtIterable, KtList, KtListOfLists, KtListOfNullable, KtMap, KtMapExt, KtMapLike,
    KtMutableList, KtPair, KtSet, KtTriple, List, MutableList, Pair, Triple, array_list_of, empty_list, empty_map,
    empty_set, float_array_of, int_array_of, list_of, list_of_not_null, map_of, mutable_list_of, mutable_map_of,
    mutable_set_of, set_of, to,
};
pub use delegate::{
    KtDelegate, KtInc, KtLateinit, KtLazy, KtMutableDelegate, lazy, post_dec, post_inc, pre_dec, pre_inc,
};
pub use format::format;
pub use math::{
    E, KtAbs, KtFloat, KtNumber, KtSum, PI, abs, acos, asin, atan, atan2, cbrt, ceil, cos, cosh, exp, floor, hypot, ln,
    log2, log10, max, max_of, min, min_of, pow, round, sign, sin, sinh, sqrt, tan, tanh, to_char, truncate,
};
pub use null::{KtElvis, KtScope, elvis, not_null, run, safe, with};
pub use range::{
    IndexedValue, KtContains, KtProgression, KtProgressionIter, KtRangeBound, KtStep, down_to, is_in, range_to, step,
    until,
};
pub use string::{KtChar, KtDelimiters, KtNeedle, KtNullableStr, KtStr};
pub use text::{KtDisplay, KtToString, to_string};

#[doc(hidden)]
pub use null::{KtFlattenNested, KtFlattenPlain, KtNullProbe};
#[doc(hidden)]
pub use text::{FormatArg, KtProbe, KtViaDisplay, KtViaToString};

/// The extension traits, imported for their methods only (`use kompoz_runtime::kt::ext::*;`).
pub mod ext {
    pub use super::collections::{
        KtComponent3 as _, KtComponents as _, KtList as _, KtListOfLists as _, KtListOfNullable as _, KtMapExt as _,
        KtMutableList as _, KtPair as _, KtTriple as _,
    };
    pub use super::delegate::{KtDelegate as _, KtInc as _, KtMutableDelegate as _};
    pub use super::math::{KtFloat as _, KtNumber as _};
    pub use super::null::KtScope as _;
    pub use super::string::{KtChar as _, KtNullableStr as _, KtStr as _};
    pub use super::text::KtToString as _;
}

/// Panics as Kotlin throws `kind` with `message` (none when empty): `IllegalStateException: message`.
#[track_caller]
pub fn panic_kt(kind: &str, message: &str) -> ! {
    if message.is_empty() { panic!("{kind}") } else { panic!("{kind}: {message}") }
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

pub use crate::__kt_flatten as flatten;
pub use crate::__kt_safe as safe;
pub use crate::__kt_template as template;

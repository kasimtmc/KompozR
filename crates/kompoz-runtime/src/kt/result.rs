//! Kotlin's `kotlin.Result<T>` on Rust's `Result<T, E>`: what a Rust function that can fail (an XML resource read,
//! a parse) gives Kotlin code is Kotlin's Result of its value, read as Kotlin reads one (`getOrThrow()`,
//! `getOrNull()`, `isSuccess`, `isFailure`, `exceptionOrNull()`); its failure thrown is a panic whose message is
//! the error's (as Kotlin's exception's `toString()`).

use std::fmt::Display;

/// Kotlin's `Result<T>` members on a Rust `Result<T, E>` (its error `Display`: what Kotlin's exception says).
pub trait KtResult<T> {
    /// Kotlin's `getOrThrow()`: the value, else the failure thrown (a panic of the error's message).
    #[track_caller]
    fn get_or_throw(self) -> T;
    /// Kotlin's `getOrNull()`.
    fn get_or_null(self) -> Option<T>;
    /// Kotlin's `isSuccess`.
    fn is_success(&self) -> bool;
    /// Kotlin's `isFailure`.
    fn is_failure(&self) -> bool;
    /// Kotlin's `exceptionOrNull()`: the failure's message (Kotlin's exception's `toString()`), none on success.
    fn exception_or_null(&self) -> Option<String>;
}

impl<T, E: Display> KtResult<T> for Result<T, E> {
    #[track_caller]
    fn get_or_throw(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }
    fn get_or_null(self) -> Option<T> {
        self.ok()
    }
    fn is_success(&self) -> bool {
        self.is_ok()
    }
    fn is_failure(&self) -> bool {
        self.is_err()
    }
    fn exception_or_null(&self) -> Option<String> {
        self.as_ref().err().map(|e| e.to_string())
    }
}

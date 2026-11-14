//! Stage 0 prototype: the same samples in the Rust API's candidate call syntaxes, to choose one.
//! `cargo run -p kompoz-syntax-proto --example a` / `--example b`.

#![allow(non_snake_case, non_upper_case_globals)]

extern crate self as kompoz_syntax_proto;

pub mod a;
pub mod b;
pub mod common;
#[doc(hidden)]
pub mod kz;

/// Composes a parameterless `#[kompozable]` function from plain Rust (`main`); in KompozR, what the application
/// entry point does.
#[macro_export]
macro_rules! compose {
    ($f:path) => {
        $crate::common::compose(|| {
            #[allow(unused_imports)]
            use $crate::kz::{ViaBuilder as _, ViaFn as _};
            (&&$crate::kz::Probe($f)).__kz_builder().__call()
        })
    };
}

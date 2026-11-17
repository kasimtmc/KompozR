// A template whose brackets Rust's lexer splits unbalanced (`"${ "(" }"` is `"${ "`, `(`, `" }"` to it): rustc
// stops before any macro runs, so its error is the only one (a `.kt` file read by `kompoz_file!` takes it).

use kompoz::prelude::*;

kompoz! {
    fun paren(): String = "${ "(" }"
}

fn main() {}

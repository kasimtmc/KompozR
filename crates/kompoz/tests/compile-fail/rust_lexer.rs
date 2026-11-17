// Kotlin that Rust's lexer, which reads `kompoz! { … }` first, rejects: a backticked name, the escapes `\$`, `\b`
// and `\uXXXX`, a raw string's backslash, a name right before a quote. rustc's errors, and ours saying what to
// write instead or to put the Kotlin in a `.kt` file.

use kompoz::prelude::*;

kompoz! {
    fun `in`(x: Int): Int = x + 1

    fun texts(price: Int): String {
        val sign = "\$$price\b"
        val a = "\u0041"
        val digits = """\d+"""
        return if (price > 0) sign else"$a$digits"
    }
}

fn main() {}

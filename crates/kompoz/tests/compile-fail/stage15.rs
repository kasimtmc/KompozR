// Kotlin the syntax doesn't translate yet: an error saying it comes with stage 15, on the Kotlin construct.

use kompoz::prelude::*;

kompoz! {
    sealed class Problem(val code: Int) : IllegalStateException("kod $code")

    fun firstLong(words: List<String>): String = buildString {
        for (w in words) {
            if (w.length > 3) return w
            append(w)
        }
    }
}

fn main() {}

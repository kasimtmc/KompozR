// Kotlin the syntax doesn't translate yet: an error saying it comes with stage 15, on the Kotlin construct.

use kompoz::prelude::*;

kompoz! {
    sealed class Problem(val code: Int) : IllegalStateException("kod $code")
}

fn main() {}

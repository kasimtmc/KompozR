// Kotlin the syntax doesn't translate yet: an error saying it comes with stage 15, on the Kotlin construct.

use kompoz::prelude::*;

kompoz! {
    fun reset(count: MutableState<Int>?) {
        count?.value = 0
    }
}

fn main() {}

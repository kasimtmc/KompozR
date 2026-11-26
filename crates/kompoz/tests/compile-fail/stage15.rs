// Kotlin the syntax doesn't translate yet: an error saying it comes with stage 15, on the Kotlin construct.

use kompoz::prelude::*;

kompoz! {
    @Composable
    fun Rows(xs: List<String>) {
        Column {
            if (xs.isEmpty()) return
            Text(xs.first())
        }
    }
}

fn main() {}

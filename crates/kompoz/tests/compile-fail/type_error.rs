// rustc's errors in the Rust the Kotlin of `kompoz! { … }` becomes, on the Kotlin tokens: a `String` where an
// `Int` goes, a misspelled member.

use kompoz::prelude::*;

kompoz! {
    @Composable
    fun Count(n: Int) {
        BasicText("sayı: $n")
    }

    @Composable
    fun Screen(label: String) {
        Count("üç")
        BasicText(label.lenght.toString())
    }
}

fn main() {}

// A Kotlin parse error in `kompoz! { … }`: our error, on the Kotlin token (a parameter's type left out); the
// declarations around it are still translated.

use kompoz::prelude::*;

kompoz! {
    @Composable
    fun Greeting(name: ) {
        BasicText("Merhaba, $name")
    }

    @Composable
    fun Screen() {
        BasicText("ekran")
    }
}

fn main() {
    let _ = Screen;
}

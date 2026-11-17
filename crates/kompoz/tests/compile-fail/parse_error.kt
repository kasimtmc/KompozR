// A parse error and a stage-14 construct in a .kt file (tests/compile-fail/file_errors.rs).

package com.example.errors

@Composable
fun Greeting(name: ) {
    BasicText("Merhaba, $name")
}

object Settings {
    val dark = false
}

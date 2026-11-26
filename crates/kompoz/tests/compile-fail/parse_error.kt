// A parse error and a construct of stage 15 in a .kt file (tests/compile-fail/file_errors.rs).

package com.example.errors

@Composable
fun Greeting(name: ) {
    BasicText("Merhaba, $name")
}

sealed class Failure(val code: Int) : IllegalStateException("kod $code")

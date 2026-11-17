// A String where an Int goes, in a .kt file (tests/compile-fail/file_errors.rs).

package com.example.errors

@Composable
fun Count(n: Int) {
    BasicText("sayı: $n")
}

@Composable
fun Screen() {
    Count("üç")
}

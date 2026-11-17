// Kotlin in a file of its own (kompoz_file!): composables, top-level vals, an extension function, effects and
// locals, Kotlin's evaluation order, backticked names (which kompoz! can't take).
package com.example.ui

import androidx.compose.runtime.*

val Accent: Color = Color.Red
const val Greeting: String = "Merhaba"
val Steps: Int = 3

fun Modifier.tagged(name: String): Modifier = this.testTag("tag-$name")

fun `match`(x: Int): Int = x * Steps

@Composable
fun FileScreen(name: String, onDone: () -> Unit = {}) {
    var shown by remember { mutableStateOf(true) }
    val upper by remember(name) { derivedStateOf { name.uppercase() } }
    Column(Modifier.tagged("screen")) {
        Text("$Greeting, $upper")
        if (shown) {
            Effectful(onDone)
        }
        Box(Modifier.size(10.dp).tagged("hide").clickable { shown = false })
        CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Rtl) {
            Text("yön: ${LocalLayoutDirection.current}", modifier = Modifier.tagged("dir"))
        }
        Text("adım: ${`match`(2)}")
        Text("görünür: $shown")
    }
}

@Composable
fun Effectful(onDone: () -> Unit) {
    DisposableEffect(Unit) {
        record("başla")
        onDispose {
            record("bitti")
            onDone()
        }
    }
}

// Arguments by name are evaluated in the order written (Kotlin's), whatever their parameters' order.
fun ordered(): String = joined(second = record("b"), first = record("a"))

fun joined(first: String, second: String = "-"): String = "$first$second"

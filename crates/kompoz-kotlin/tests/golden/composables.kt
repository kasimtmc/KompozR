// Composables: restartable and inline ones, parameters with types and defaults, visibility, a value-returning
// one, `@Composable` as `@Kompozable`, imports and the package ignored.
package com.example.ui

import androidx.compose.runtime.*
import androidx.compose.foundation.layout.Column

@Kompozable
fun Counter(start: Int = 0) {
    var count by remember { mutableStateOf(start) }
    Column {
        Text("Count: $count")
        Button({ count++ }) { Text("+") }
    }
}

@Composable
fun Button(onClick: () -> Unit, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Box(modifier.clickable(onClick = onClick).testTag("button")) {
        content()
    }
}

@Kompozable
internal fun Labeled(label: String, value: Int = 7, suffix: String = "", content: @Kompozable () -> Unit) {
    Row {
        Text("$label=$value$suffix")
        content()
    }
}

@Kompozable
private fun NamedArguments() {
    Column {
        Labeled(suffix = "!", label = "a") { Text("içerik") }
        Labeled("b", 3) { }
        Labeled("c", content = {})
    }
}

@Kompozable
fun textSize(): TextUnit = 18.sp

@Composable
inline fun Twice(content: @Composable () -> Unit) {
    content()
    content()
}

@Kompozable
fun Maybe(title: String?, count: Long = 10L) {
    if (title != null) {
        Text(title)
    }
    Text("$count")
}

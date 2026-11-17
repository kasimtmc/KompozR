// KompozR's Counter in Kotlin's syntax, as a Compose user writes it: the same count and button as the Rust Counter
// (examples/counter), drawn the same, pixel for pixel.

package com.example.counter

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The button's color, and the text's. */
val Accent = Color(0xff3f51b5)
val Ink = Color(0xff1c1b1f)

/** A count and a button that adds to it; the count kept across runs with `KOMPOZ_STATE_FILE`. */
@Composable
fun Counter() {
    var count by rememberSaveable { mutableStateOf(0) }
    Column(
        modifier = Modifier.fillMaxSize().background(Color.White).padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        // (The button first, at the top left: the scripts' `click 70 70` presses it.)
        Box(
            modifier = Modifier
                .size(120.dp, 80.dp)
                .background(Accent, RoundedCornerShape(16.dp))
                .clickable(role = Role.Button) { count++ },
            contentAlignment = Alignment.Center,
        ) {
            BasicText("+1", style = TextStyle(fontSize = 28.sp, color = Color.White))
        }
        BasicText("Sayaç: $count", style = TextStyle(fontSize = 32.sp, color = Ink))
    }
}

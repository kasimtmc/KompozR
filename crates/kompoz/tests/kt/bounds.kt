// Lookahead animations of bounds in a Kotlin file of its own (kompoz_file!): `animateContentSize()`, and a box in a
// `LookaheadScope { }` gliding with `Modifier.animateBounds(this@LookaheadScope, …)`, as Compose writes them.
package com.example.bounds

import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.*

@Composable
fun Growing(big: MutableState<Boolean>) {
    Box(modifier = Modifier.testTag("büyüyen").animateContentSize()) {
        Box(modifier = Modifier.size(if (big.value) 200.dp else 100.dp))
    }
}

@Composable
fun Gliding(right: MutableState<Boolean>) {
    LookaheadScope {
        Box(
            modifier = Modifier.size(300.dp, 200.dp),
            contentAlignment = if (right.value) Alignment.BottomEnd else Alignment.TopStart,
        ) {
            Box(
                modifier = Modifier
                    .animateBounds(this@LookaheadScope, boundsTransform = BoundsTransform { _, _ -> tween(400, easing = LinearEasing) })
                    .size(40.dp)
                    .testTag("kayan"),
            )
        }
    }
}

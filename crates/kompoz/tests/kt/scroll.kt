// Scrolling in a Kotlin file of its own (kompoz_file!): a scrollable of a state of one's own with its overscroll
// effect remembered and drawn around it, a draggable with its stop callback, and a horizontal scroll animated to
// a value from a LaunchedEffect, as Compose writes them.
package com.example.scroll

import androidx.compose.foundation.*
import androidx.compose.foundation.gestures.*
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.*

@Composable
fun Pull(seen: (Float) -> Unit) {
    var offset by remember { mutableStateOf(0f) }
    val state = rememberScrollableState { delta ->
        // Takes what keeps it within 0..300; the rest is left (for the overscroll at the ends).
        val next = (offset + delta).coerceIn(0f, 300f)
        val consumed = next - offset
        offset = next
        seen(offset)
        consumed
    }
    val overscroll = rememberOverscrollEffect()
    Box(
        Modifier.size(200.dp)
            .testTag("pull")
            .overscroll(overscroll)
            .scrollable(
                state,
                Orientation.Vertical,
                overscrollEffect = overscroll,
                reverseDirection = true,
                flingBehavior = ScrollableDefaults.flingBehavior(),
            )
    )
}

@Composable
fun Slider(moved: (Float) -> Unit, stopped: (Float) -> Unit) {
    var x by remember { mutableStateOf(0f) }
    Box(
        Modifier.size(300.dp, 60.dp)
            .testTag("slider")
            .draggable(
                rememberDraggableState { delta ->
                    x += delta
                    moved(x)
                },
                Orientation.Horizontal,
                onDragStopped = { velocity -> stopped(velocity) },
            )
    )
}

@Composable
fun Strip(target: Int) {
    val state = rememberScrollState()
    LaunchedEffect(target) {
        state.animateScrollTo(target)
    }
    Row(Modifier.fillMaxWidth().horizontalScroll(state).testTag("strip")) {
        repeat(20) { i -> Box(Modifier.size(50.dp).testTag("cell $i")) }
    }
}

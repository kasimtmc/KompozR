// Transitions in a Kotlin file of its own (kompoz_file!): a SeekableTransitionState seeked, animated and snapped from
// a coroutine, a child transition of a parent's state, a DeferredTransitionState's pending state, as Compose writes them.
package com.example.transitions

import androidx.compose.animation.core.*
import androidx.compose.runtime.*

@Composable
fun Seeking(state: SeekableTransitionState<Boolean>, go: Int, seen: (Float) -> Unit) {
    val transition = rememberTransition(state, label = "arama")
    val x by transition.animateFloat(transitionSpec = { tween(320, easing = LinearEasing) }) { if (it) 100f else 0f }
    LaunchedEffect(go) {
        when (go) {
            1 -> state.seekTo(0.5f, targetState = true)
            2 -> state.animateTo(true)
            3 -> state.snapTo(false)
        }
    }
    seen(x)
}

@Composable
fun Child(target: Int, seen: (Float, Boolean) -> Unit) {
    val parent = updateTransition(target, label = "ebeveyn")
    val child = parent.createChildTransition(label = "çocuk") { it > 1 }
    val x by child.animateFloat(transitionSpec = { tween(200) }) { if (it) 1f else 0f }
    seen(x, parent.isRunning)
}

@Composable
fun Deferring(state: DeferredTransitionState<Boolean>, seen: (Boolean?, Float) -> Unit) {
    val transition = rememberTransition(state)
    val x by transition.animateFloat { if (it) 1f else 0f }
    seen(transition.pendingTargetState, x)
}

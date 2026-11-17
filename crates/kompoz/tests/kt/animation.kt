// Animations in a Kotlin file of its own (kompoz_file!): a color Animatable, an AnimationState flung along a decay,
// the suspend `animate` of any type, a MutatorMutex, as Compose writes them.
package com.example.animation

import androidx.compose.animation.core.*
import androidx.compose.runtime.*

val Fade: AnimationSpec<Color> = tween(durationMillis = 100, easing = LinearEasing)

@Composable
fun Tint(seen: (Color) -> Unit) {
    val color = remember { Animatable(Color.Black) }
    LaunchedEffect(Unit) {
        color.animateTo(Color.White, Fade)
    }
    seen(color.value)
    Box(Modifier.testTag("tint").size(10.dp).background(color.value))
}

@Composable
fun Flung(result: (Float, Float) -> Unit) {
    LaunchedEffect(Unit) {
        val state = AnimationState(initialValue = 0f, initialVelocity = 420f)
        state.animateDecay(exponentialDecay())
        result(state.value, state.velocity)
    }
}

@Composable
fun Path2D(points: (Offset) -> Unit) {
    LaunchedEffect(Unit) {
        animate(
            Offset.VectorConverter,
            initialValue = Offset.Zero,
            targetValue = Offset(30f, 60f),
            animationSpec = tween(48, easing = LinearEasing),
        ) { value, _ ->
            points(value)
        }
    }
}

@Composable
fun Mutations(log: (String) -> Unit) {
    val mutex = remember { MutatorMutex() }
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) {
        scope.launch {
            mutex.mutate(MutatePriority.UserInput) {
                log("drag")
                delay(100)
                log("drag done")
            }
        }
        scope.launch {
            mutex.mutate {
                log("animation")
            }
        }
    }
}

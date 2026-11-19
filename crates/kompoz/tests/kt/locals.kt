// A local computed where it is read, in a Kotlin file of its own (kompoz_file!): Compose's way to have an animated
// color followed by the composables reading it alone — `LocalTint providesComputed { color.value }`, the State read in
// the computation, where the local is read — and a computed default reading the density there.
package com.example.locals

import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.runtime.*
import androidx.compose.ui.graphics.*
import androidx.compose.ui.platform.*
import androidx.compose.ui.unit.*

val LocalTint = compositionLocalOf { Color.Black }

val LocalHairline = compositionLocalWithComputedDefaultOf { with(LocalDensity.currentValue) { 10.dp.toPx() } }

@Composable
fun TintReader(read: (Color) -> Unit) {
    read(LocalTint.current)
}

@Composable
fun Tinted(on: Boolean, tick: Int, composed: (Int) -> Unit, read: (Color) -> Unit) {
    composed(tick)
    val color = animateColorAsState(if (on) Color.White else Color.Black, tween(160, easing = LinearEasing))
    CompositionLocalProvider(LocalTint providesComputed { color.value }) {
        TintReader(read)
    }
}

@Composable
fun Hairlines(seen: (Float) -> Unit) {
    seen(LocalHairline.current)
    CompositionLocalProvider(LocalDensity provides Density(2f)) {
        seen(LocalHairline.current)
    }
}

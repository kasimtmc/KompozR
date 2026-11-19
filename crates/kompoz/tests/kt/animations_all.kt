// Animation as Compose writes it (the C++ Kompoz's kompozable_tests.cpp `Animations`, whole): animated values by
// delegate, AnimatedVisibility and AnimatedContent with their named arguments and infix transforms, a transition's
// values, an infinite transition, an Animatable in a coroutine, Crossfade, a shared element with
// this@AnimatedContent in a SharedTransitionLayout.
package com.example.animations

import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt

// enum class Panel { Closed, Open }: in the test's Rust (enum classes come with stage 14's Kotlin syntax)
val InkBlue = Color(0xFF1E88E5)   // Compose's Color(Long): AARRGGBB

@Composable
fun Animations(open: Boolean, count: Int) {
    val scale by animateFloatAsState(if (open) 1.2f else 1f, spring(Spring.DampingRatioMediumBouncy), label = "scale")
    val tint by animateColorAsState(if (open) Color.White else Color.Black, tween(300))
    val transition = updateTransition(targetState = if (open) Panel.Open else Panel.Closed, label = "panel")
    val size by transition.animateDp(label = "size") { if (it == Panel.Open) 200.dp else 80.dp }
    val pulse by rememberInfiniteTransition(label = "pulse").animateFloat(
        initialValue = 0.85f, targetValue = 1f, animationSpec = infiniteRepeatable(tween(800), RepeatMode.Reverse), label = "scale")
    val offset = remember { Animatable(0f) }
    LaunchedEffect(open) {
        if (open) {
            for (x in listOf(-12f, 12f, 0f)) offset.animateTo(x, tween(50))
        }
    }
    Column {
        Text("ölçek ${(scale * 10).roundToInt()}", modifier = Modifier.graphicsLayer { scaleX = pulse; scaleY = pulse }, color = tint)
        Box(modifier = Modifier.size(size).offset { IntOffset(offset.value.roundToInt(), 0) }.testTag("kutu"))
        AnimatedVisibility(visible = open, enter = fadeIn() + expandVertically(), exit = fadeOut() + shrinkVertically(), label = "details") {
            Text("ayrıntılar")
        }
        AnimatedContent(
            targetState = count,
            transitionSpec = {
                if (targetState > initialState) slideInVertically { it } togetherWith slideOutVertically { -it }
                else (slideInVertically { -it } togetherWith slideOutVertically { it }) using SizeTransform(clip = false)
            },
            label = "count",
        ) { n ->
            Text("sayı $n")
        }
        Crossfade(targetState = open, label = "page") { p -> Text(if (p) "açık" else "kapalı") }
        SharedTransitionLayout(modifier = Modifier.fillMaxWidth()) {
            AnimatedContent(targetState = open, label = "photo") { o ->
                Box(modifier = Modifier.size(if (o) 80.dp else 40.dp).testTag(if (o) "büyük" else "küçük")
                        .sharedElement(rememberSharedContentState(key = "photo"), animatedVisibilityScope = this@AnimatedContent))
            }
        }
    }
}

// Content coming in and going in a Kotlin file of its own (kompoz_file!), as Compose writes it: every enter and exit
// transition (with their lambdas, `+`, `None`), AnimatedVisibility of a transition's state, AnimatedContent of a
// SeekableTransitionState's transition with slideIntoContainer/slideOutOfContainer and KeepUntilTransitionsFinished, a
// ContentTransform made by hand, Crossfade of a transition with a content key, the deferred forms with a
// MutableTransform and a MutableContentTransform, the content's transition of EnterExitStates.
package com.example.animated

import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.*
import androidx.compose.ui.graphics.*
import androidx.compose.ui.unit.*

@Composable
fun EveryTransition(shown: Boolean) {
    Column {
        AnimatedVisibility(
            visible = shown,
            enter = slideIn(tween(320, easing = LinearEasing)) { IntOffset(it.width * 2, 0) } + fadeIn(initialAlpha = 0.3f),
            exit = slideOut(tween(320, easing = LinearEasing)) { IntOffset(-it.width, 0) } + fadeOut(targetAlpha = 0.1f),
        ) {
            Box(modifier = Modifier.size(100.dp, 50.dp).testTag("kayan"))
        }
        AnimatedVisibility(
            visible = shown,
            modifier = Modifier.testTag("büyüyen"),
            enter = expandIn(tween(320, easing = LinearEasing), expandFrom = Alignment.TopStart, clip = false) { IntSize(it.width / 2, it.height / 2) },
            exit = shrinkHorizontally(tween(320, easing = LinearEasing), shrinkTowards = Alignment.Start) { it / 4 },
        ) {
            Box(modifier = Modifier.size(100.dp, 50.dp))
        }
        AnimatedVisibility(
            visible = shown,
            enter = scaleIn(initialScale = 0.5f, transformOrigin = TransformOrigin(0f, 0f)) + unveilIn(initialColor = Color.Red) +
                expandVertically(expandFrom = Alignment.Top) { it / 2 } + slideInHorizontally { -it } + EnterTransition.None,
            exit = scaleOut(targetScale = 0.5f) + veilOut(targetColor = Color.Blue, matchParentSize = true) +
                shrinkOut(shrinkTowards = Alignment.Center) + slideOutVertically { it } + ExitTransition.None,
        ) {
            // (the content's own transition: of EnterExitStates)
            val alpha by transition.animateFloat(label = "kendi") { if (it == EnterExitState.Visible) 1f else 0f }
            Box(modifier = Modifier.size(20.dp).graphicsLayer { this.alpha = alpha }.testTag("ölçekli"))
        }
    }
}

@Composable
fun OfATransition(step: Int) {
    val transition = updateTransition(step, label = "adım")
    Column {
        transition.AnimatedVisibility(visible = { it > 0 }, modifier = Modifier.testTag("görünür"), enter = fadeIn(tween(200)), exit = fadeOut(tween(200))) {
            Box(modifier = Modifier.size(20.dp))
        }
        transition.AnimatedContent(transitionSpec = { fadeIn(tween(160)) togetherWith fadeOut(tween(160)) }) { s ->
            Text("adım $s")
        }
        transition.Crossfade(animationSpec = tween(240), contentKey = { it / 2 }) { s -> Text("geçiş $s") }
    }
}

@Composable
fun Sliding(state: SeekableTransitionState<Int>) {
    val transition = rememberTransition(state, label = "sayfa")
    transition.AnimatedContent(
        modifier = Modifier.testTag("kap"),
        transitionSpec = {
            if (targetState > initialState) {
                slideIntoContainer(AnimatedContentTransitionScope.SlideDirection.Left, tween(320, easing = LinearEasing)) togetherWith
                    slideOutOfContainer(AnimatedContentTransitionScope.SlideDirection.Left, tween(320, easing = LinearEasing))
            } else {
                ContentTransform(fadeIn(tween(320)), ExitTransition.KeepUntilTransitionsFinished, targetContentZIndex = -1f, sizeTransform = null)
            }
        },
        contentAlignment = Alignment.Center,
    ) { page ->
        Box(modifier = Modifier.size(100.dp, 40.dp).testTag("parça $page"))
    }
}

@Composable
fun Deferred(visible: DeferredTransitionState<Boolean>, page: DeferredTransitionState<Int>, drag: Int) {
    val shown = rememberTransition(visible)
    val transform = remember { MutableTransform() }
    transform.update { fullSize -> offset = IntOffset(drag, 0); alpha = 1f - drag / 200f }
    Column {
        shown.DeferredAnimatedVisibility(visible = { it }, exit = slideOut(tween(320)) { IntOffset(it.width * 2, 0) }, mutableTransform = transform) {
            Box(modifier = Modifier.size(100.dp, 40.dp).testTag("sürüklenen"))
        }
        val pages = rememberTransition(page)
        pages.DeferredAnimatedContent(
            transitionSpec = { fadeIn(tween(200)) togetherWith fadeOut(tween(200)) },
            mutableTransformSpec = {
                MutableContentTransform {
                    initialContentTransform { offset = IntOffset(-drag, 0) }
                    targetContentTransform { fullSize -> offset = IntOffset(fullSize.width - drag, 0) }
                }
            },
        ) { n ->
            Box(modifier = Modifier.size(100.dp, 40.dp).testTag("sayfa $n"))
        }
    }
}

@Composable
fun Pane(scope: AnimatedVisibilityScope, page: Int) {
    Box(modifier = with(scope) { Modifier.animateEnterExit(enter = slideInHorizontally { it }) }.size(100.dp, 40.dp).testTag("bölme $page"))
}

@Composable
fun Panes(page: Int) {
    // (this@AnimatedContent: the content's scope, an AnimatedVisibilityScope, given on; one key for each pair of pages)
    AnimatedContent(targetState = page, contentKey = { it / 2 }) { p ->
        Pane(this@AnimatedContent, p)
    }
}

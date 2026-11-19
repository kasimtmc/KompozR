// AnimatedVisibility, AnimatedContent and Crossfade as Compose writes them: transitions combined with `+`, the infix
// `togetherWith` and `using` in an AnimatedContentTransitionScope, `AnimatedVisibility` in a Row (RowScope's), the
// visibleState form, `animateEnterExit` of the content's scope, `this@AnimatedContent`.

@Composable
fun Visibilities(open: Boolean, count: Int) {
    Column {
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
            Text("sayı $n", modifier = Modifier.animateEnterExit(enter = fadeIn()))
        }
        Crossfade(targetState = open, label = "page") { p -> Text(if (p) "açık" else "kapalı") }
        Row {
            AnimatedVisibility(open) { Box(modifier = Modifier.size(10.dp)) }
        }
    }
}

@Composable
fun Entering() {
    val shown = remember { MutableTransitionState(false).apply { targetState = true } }
    Column {
        AnimatedVisibility(visibleState = shown, enter = fadeIn(animationSpec = tween(100)) + expandVertically(animationSpec = tween(100)),
                           exit = fadeOut(animationSpec = tween(100))) {
            Box(modifier = Modifier.animateEnterExit(enter = slideInVertically(animationSpec = tween(100)) { it }, exit = scaleOut(targetScale = 0.5f))
                    .size(10.dp).testTag("giren"))
        }
        Text("durum ${shown.currentState}")
    }
}

@Composable
fun Pane(scope: AnimatedVisibilityScope, page: Int) {
    Box(modifier = with(scope) { Modifier.animateEnterExit() }.size(10.dp).testTag("sayfa $page"))
}

@Composable
fun Contents(page: Int) {
    AnimatedContent(targetState = page, contentKey = { it / 2 }) { p ->
        Pane(this@AnimatedContent, p)
    }
}

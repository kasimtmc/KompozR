@Composable
fun Values(open: Boolean) {
    val scale by animateFloatAsState(if (open) 1.2f else 1f, spring(Spring.DampingRatioMediumBouncy), label = "scale")
    val transition = updateTransition(targetState = open, label = "panel")
    val size by transition.animateDp(transitionSpec = { if (false isTransitioningTo true) tween(160) else snap() }) {
        if (it) 200.dp else 80.dp
    }
    val count by transition.animateInt { if (it) 2 else 1 }
    Text("ölçek ${(scale * 10).roundToInt()} $count", modifier = Modifier.size(size))
}

@Composable
fun Steps(step: MutableTransitionState<Int>, seekable: SeekableTransitionState<Int>) {
    val steps = rememberTransition(step, label = "adım")
    val x by steps.animateFloat(transitionSpec = { tween(160) }) { it * 10f }
    val seeking = rememberTransition(seekable)
    val child = seeking.createChildTransition { it > 1 }
    val y by child.animateFloat { if (it) 1f else 0f }
    Text("$x $y")
}

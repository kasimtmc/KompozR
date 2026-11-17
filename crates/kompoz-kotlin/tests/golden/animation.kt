@Composable
fun Grow(done: () -> Unit) {
    val size = remember { Animatable(0f) }
    LaunchedEffect(Unit) {
        size.animateTo(100f, tween(300, easing = LinearEasing))
        size.animateTo(40f, spring(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = Spring.StiffnessMedium))
        done()
    }
    Box(Modifier.testTag("grow").size(size.value.dp))
}

@Composable
fun Slide(offset: Animatable<Offset, AnimationVector2D>) {
    LaunchedEffect(offset) {
        offset.animateTo(Offset(100f, 50f), animationSpec = tween(durationMillis = 160, delayMillis = 0))
        offset.snapTo(Offset.Zero)
    }
    Box(Modifier.testTag("slide").offset(offset.value.x.dp, offset.value.y.dp).size(10.dp))
}

@Composable
fun Pulse(log: (Float) -> Unit) {
    val scale = remember { Animatable(1f) }
    val scope = rememberCoroutineScope()
    Box(Modifier.testTag("pulse").size(20.dp).clickable {
        scope.launch {
            scale.animateTo(
                1f,
                keyframes {
                    durationMillis = 400
                    1.3f at 100 using FastOutSlowInEasing
                    0.9f at 250
                },
            ) {
                log(value)
            }
        }
    })
}

@Composable
fun Fling(target: (Float) -> Unit) {
    val position = remember { Animatable(0f, Offset.VisibilityThreshold.x) }
    LaunchedEffect(Unit) {
        position.updateBounds(upperBound = 150f)
        val result = position.animateDecay(1000f, exponentialDecay())
        target(position.value)
        if (result.endReason == AnimationEndReason.BoundReached) {
            position.stop()
        }
    }
}

@Composable
fun Ticker(seen: (Float, Float) -> Unit) {
    LaunchedEffect(Unit) {
        animate(0f, 10f, animationSpec = tween(100, easing = LinearEasing)) { value, velocity ->
            seen(value, velocity)
        }
    }
}

@Composable
fun Forever(ticks: (Float) -> Unit) {
    val angle = remember { Animatable(0f) }
    LaunchedEffect(Unit) {
        angle.animateTo(360f, infiniteRepeatable(tween(1000, easing = LinearEasing), RepeatMode.Restart)) {
            ticks(value)
        }
    }
}

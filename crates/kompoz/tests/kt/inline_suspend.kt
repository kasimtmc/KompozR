// Kotlin's inline functions suspending with their coroutine, in a .kt file (kompoz_file!: calls keyed by their
// places): a scope function and a forEach whose lambdas animate, a safe call and `?:` that suspend.

suspend fun fetched(n: Int): Int {
    delay(50)
    return n + 1
}

@Composable
fun Stepper(go: Boolean, target: Float?, log: (String) -> Unit) {
    val anim = remember { Animatable(0f) }
    LaunchedEffect(go) {
        if (!go) return@LaunchedEffect
        listOf(10f, 20f, 30f).forEach { step ->
            with(anim) { animateTo(step, tween(100, easing = LinearEasing)) }
            log("at ${anim.value}")
        }
        val missing: Animatable<Float, AnimationVector1D>? = null
        missing?.animateTo(99f)
        val there: Animatable<Float, AnimationVector1D>? = anim
        there?.animateTo(40f, tween(100, easing = LinearEasing))
        val n = target?.toInt() ?: fetched(1)
        log("n $n ${anim.value}")
    }
    Box(Modifier.testTag("stepper").size(anim.value.dp))
}

// Kotlin's implicit `it` is the nearest lambda's that has one: a lambda inside that takes no parameter (a coroutine's,
// a scope's receiver lambda, `run { }`'s) leaves `it` the outer lambda's, which takes it then; one with an `it` of its
// own hides the outer one; a lambda whose parameter isn't used takes `_`.
@Composable
fun Launched(xs: List<Int>, log: (Int) -> Unit) {
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) {
        xs.forEach { scope.launch { delay(it.toLong()) } }
        xs.forEach { x -> scope.launch { log(x) } }
        xs.forEach { run { log(it + 1) } }
        xs.forEach { with(scope) { launch { log(it * 2) } } }
        xs.map { it + 1 }.forEach { listOf(it).forEach { log(it) } }
        xs.forEach { scope.launch { listOf(1).forEach { log(it) } } }
        xs.forEach { scope.launch { log(0) } }
        repeat(2) { scope.launch { delay(10); log(it) } }
    }
    Box(Modifier.pointerInput(Unit) { xs.forEach { awaitPointerEventScope { log(it) } } })
}

// (a lambda given to what the index doesn't know: its `it` its own when it uses one)
@Composable
fun Unknown(log: (Int) -> Unit) {
    madeElsewhere { madeElsewhere { log(it) } }
    madeElsewhere { madeElsewhere2 { log(0) }; log(it) }
}

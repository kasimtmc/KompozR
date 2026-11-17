// Lambdas: `it`, parameters, `_`, kept and inline ones, receiver lambdas (draw, graphics layer, semantics,
// pointer input), Layout's measure policy, suspend lambdas, effects, locals provided, annotated expressions.

@Kompozable
fun Drawing(color: Color) {
    Canvas(Modifier.size(100.dp)) {
        drawCircle(color, radius = size.minDimension / 4)
        drawRect(Color.Blue, topLeft = Offset(10f, 10f), size = Size(20f, 20f))
        drawLine(color, start = Offset.Zero, end = center, strokeWidth = 2f)
    }
    Box(Modifier.graphicsLayer { scaleX = 2f; alpha = 0.5f }.semantics { contentDescription = "kutu" })
}

@Kompozable
fun Effects(key: Int) {
    var ticks by remember { mutableStateOf(0) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(key) {
        while (ticks < 3) {
            delay(100)
            ticks += 1
        }
    }
    LaunchedEffect(key, ticks) { println(ticks) }
    DisposableEffect(Unit) {
        println("başladı")
        onDispose { println("bitti") }
    }
    SideEffect { println(ticks) }
    Box(Modifier.clickable { scope.launch { delay(10); ticks = 0 } }.pointerInput(Unit) {
        detectTapGestures(onTap = { ticks++ })
    })
    CompositionLocalProvider(LocalDensity provides Density(2f)) {
        Text("${LocalDensity.current.density}")
    }
}

@Kompozable
fun Stacked(content: @Kompozable () -> Unit) {
    Layout(content = content, modifier = Modifier.testTag("stack")) { measurables, constraints ->
        val placeables = measurables.map { it.measure(constraints) }
        val height = placeables.sumOf { it.height }
        layout(constraints.maxWidth, height) {
            var y = 0
            placeables.forEach {
                it.place(0, y)
                y += it.height
            }
        }
    }
}

fun lambdaValues(): Int {
    val add = { a: Int, b: Int -> a + b }
    val pairs = listOf(1 to "a", 2 to "b")
    val names = pairs.map { (n, s) -> "$s$n" }
    val ignored = pairs.map { (_, s) -> s }
    val firsts = pairs.map { p -> p.first }
    val annotated = @Suppress("x") add(1, 2)
    return add(names.size, firsts.size) + ignored.size + annotated
}

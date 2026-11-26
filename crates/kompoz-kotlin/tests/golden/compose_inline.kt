// Compose's own inline functions in a coroutine: their lambdas suspend with it, as the kt shims' do. A lambda that
// suspends is an async closure given to the API's suspending twin (`build_annotated_string_async`, `with_style_async`,
// `PathData_async`, `group_async` through its builder, `take_or_else_async`, `edit_async`, `try_mutate_async`, the draw
// transforms'), the call awaited; withTransform takes both lambdas inline: where one suspends, both are async
// closures. In the composition, withStyle's and withLink's lambdas are the builder's receiver lambdas. Kotlin's
// `crossinline` lambdas (withAnnotation's, withStyle(ParagraphStyle)'s: the generator knows the overload by its
// argument's type, a plain closure where it doesn't suspend) and Compose's snapshot blocks (which must not suspend)
// can't suspend: errors. (`with(scope)` lends a clone of the value, as it does any; a draw scope has none: in a
// coroutine it is one of KompozR's own making, so `paint` shows what the transforms' calls are written as.)
suspend fun later(n: Int): Int {
    delay(100)
    return n
}

@Composable
fun Built(log: (AnnotatedString) -> Unit) {
    val plain = buildAnnotatedString {
        append("a")
        withStyle(SpanStyle(color = Color.Red)) { append("b") }
        withLink(LinkAnnotation.Url("https://kompoz.dev")) { append("c") }
    }
    log(plain)
    LaunchedEffect(Unit) {
        val built = buildAnnotatedString {
            append("x")
            withStyle(SpanStyle(color = Color.Blue)) {
                append("${later(1)}")
                withLink(LinkAnnotation.Url("https://kompoz.dev")) {
                    delay(50)
                    append("y")
                }
            }
            withStyle(SpanStyle(fontSize = 12.sp)) { append("z") }
            withStyle(ParagraphStyle(textAlign = TextAlign.Center)) { append("p") }
        }
        log(built)
    }
}

@Composable
fun Values(state: TextFieldState, mutex: MutatorMutex, log: (String) -> Unit) {
    LaunchedEffect(Unit) {
        val color = Color.Unspecified.takeOrElse {
            delay(10)
            Color.Green
        }
        val width = Dp.Unspecified.takeOrElse { later(3).dp }
        val size = TextUnit.Unspecified.takeOrElse { 7.sp }
        state.edit {
            append("b")
            delay(100)
            append("c")
        }
        val ran = mutex.tryMutate { delay(100) }
        log("$color $width $size $ran")
    }
}

@Composable
fun Vectors(nodes: (List<PathNode>) -> Unit, icon: (ImageVector) -> Unit) {
    LaunchedEffect(Unit) {
        nodes(PathData {
            moveTo(0f, 0f)
            delay(10)
            lineTo(4f, 4f)
        })
        icon(ImageVector.Builder(name = "v", defaultWidth = 8.dp, defaultHeight = 8.dp, viewportWidth = 8f,
            viewportHeight = 8f)
            .group(name = "g", rotate = 45f) {
                delay(10)
                path(fill = SolidColor(Color.Red)) {
                    moveTo(0f, 0f)
                    delay(10)
                    lineTo(8f, 8f)
                }
            }
            .build())
    }
}

// (a draw scope in a coroutine: the transforms' twins — named ones through their builders —, both of
// withTransform's lambdas async where one suspends, nested ones)
suspend fun paint(scope: DrawScope, log: (String) -> Unit) {
    with(scope) {
        translate(left = 2f) {
            delay(10)
            drawRect(Color.Red, size = Size(1f, 1f))
        }
        withTransform({ translate(1f, 1f) }) {
            delay(10)
            drawRect(Color.Blue)
        }
        withTransform({
            delay(10)
            rotate(90f)
        }) { drawRect(Color.Green) }
        clipRect(right = 5f) {
            rotate(45f) {
                drawIntoCanvas {
                    delay(1)
                    it.drawRect(Rect(0f, 0f, 1f, 1f), Paint())
                }
            }
        }
        inset(1f, 2f, 3f, 4f) {
            scale(2f, 3f) {
                delay(1)
                drawRect(Color.Red)
            }
        }
        withTransform({ translate(1f, 1f) }) { drawRect(Color.Blue) }
    }
    log("painted")
}

@Composable
fun NotSuspending(log: (Int) -> Unit) {
    LaunchedEffect(Unit) {
        val tagged = buildAnnotatedString {
            withAnnotation("tag", "x") {
                delay(1)
                append("a")
            }
            val centered = ParagraphStyle(textAlign = TextAlign.Center)
            withStyle(centered) {
                append("b")
                delay(1)
            }
        }
        Snapshot.withMutableSnapshot { delay(1) }
        val read = Snapshot.withoutReadObservation { later(2) }
        log(read)
    }
}

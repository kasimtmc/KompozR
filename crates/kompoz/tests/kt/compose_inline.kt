// Compose's inline functions suspending with their coroutine, in a .kt file (kompoz_file!: calls keyed by their
// places): a withStyle whose lambda doesn't suspend left plain, a withLink that suspends, a withStyle(ParagraphStyle)
// (Kotlin's takes its lambda `crossinline`) in the suspending builder, takeOrElse, a text field state's edit.

suspend fun typed(): String {
    delay(10)
    return "42"
}

@Composable
fun Linked(go: Boolean, state: TextFieldState, log: (String) -> Unit) {
    LaunchedEffect(go) {
        if (!go) return@LaunchedEffect
        val text = buildAnnotatedString {
            withStyle(SpanStyle(fontSize = 12.sp)) { append("a") }
            withLink(LinkAnnotation.Url("https://kompoz.dev")) {
                delay(100)
                append("b")
            }
            withStyle(ParagraphStyle(textAlign = TextAlign.Center)) { append("c") }
        }
        log("linked ${text.text}")
        val size = TextUnit.Unspecified.takeOrElse {
            delay(10)
            7.sp
        }
        log("size ${size.value}")
        state.edit { append(typed()) }
        log("typed ${state.text}")
    }
}

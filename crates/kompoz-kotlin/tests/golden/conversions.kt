// What a call converts as Kotlin's values go where Rust's types differ: a lambda to a shared closure (`Callback`, an
// `Option` of one, its parameters typed), a value to a type it converts to (`Stroke` → `DrawStyle`), a list to a
// slice, another integer type to an `Int`, a string literal to a function value's `String`, nothing to a vararg, a
// lambda to a function pointer; an interface parameter (`Shape`) shared; a nested object (`Arrangement.Absolute`); a
// vararg `Any?`'s arithmetic lent whole (`"%.1f".format(0.3 + 0.27 * i, …)`).

@Composable
fun Conversions(log: (String) -> Unit, onLines: (Int) -> Unit, shape: Shape = RoundedCornerShape(4.dp)) {
    val points = remember { mutableStateListOf<Offset>() }
    Row(horizontalArrangement = Arrangement.Absolute.Left) {
        Box(Modifier.combinedClickable(onLongClick = { log("long") }) { log("click") }.background(Color.Red, shape))
        BasicText("metin", onTextLayout = { onLines(it.lineCount) })
        Canvas(Modifier.size(10.dp)) {
            drawCircle(Color.Red, radius = 4f, style = Stroke(width = 2f))
            drawLine(Color.Red, Offset(0f, 0f), Offset(1f, 1f), pathEffect = PathEffect.dashPathEffect(floatArrayOf(2f, 1f)))
        }
        Text("${points.size}")
        Text("%.1f km · %d dk".format(0.3 + 0.27 * points.size, 4 + 3 * points.size))
    }
}

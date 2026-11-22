// Sliders as Compose writes them: a `Float` `a..b` is Kotlin's `ClosedFloatingPointRange` (Rust's `RangeInclusive`,
// no progression), read by `.start`/`.endInclusive`/`isEmpty()`/`in`, held in a state and set from a callback; an
// API type's callback property set (`state.onValueChange = { … }`: its setter's nullable function, the lambda made
// it); a `var x: T? by …` typed by its declaration; a nullable `DrawScope.(Offset)` lambda or null into a parameter
// whose default depends on others (`Option<Option<…>>`); an overload chosen by its first argument's type.

@Composable
fun Sliders(bold: Boolean) {
    var price by remember { mutableStateOf(200f..800f) }
    val whole: ClosedFloatingPointRange<Float> = 0f..1000f
    var picked: Int? by remember { mutableStateOf(null) }
    val stars = rememberSliderState(value = 3f, steps = 3, valueRange = 1f..5f)
    val hours = rememberRangeSliderState(activeRangeStart = 9f, activeRangeEnd = 17f, valueRange = 0f..24f)
    stars.shouldAutoSnap = false
    stars.onValueChange = { v -> picked = v.roundToInt() }
    Column {
        RangeSlider(value = price, onValueChange = { price = it }, valueRange = whole, steps = 19)
        Slider(
            state = stars,
            track = { s ->
                SliderDefaults.Track(
                    s,
                    drawStopIndicator = if (bold) { { drawCircle(Color.Red, radius = 3.dp.toPx(), center = it) } } else null,
                )
            },
        )
        RangeSlider(state = hours, track = { SliderDefaults.Track(it) })
        val inside = 500f in price && !whole.isEmpty()
        Text("${price.start.roundToInt()}–${price.endInclusive.roundToInt()} ${picked ?: 0} $inside")
    }
}

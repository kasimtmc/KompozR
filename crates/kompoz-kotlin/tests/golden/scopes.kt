// Receivers' members through what they deref to (a MeasureScope's Density: `roundToPx`), the drawing scope's own
// transforms with a block (`rotate(45f) { … }`), a generic owner's lambda given its type argument by `&mut`
// (`state.update { it.add(…) }`), a state list's own methods, a detector's builder after its own arguments.

fun Modifier.tallerBy(extra: Dp) = layout { measurable, constraints ->
    val placeable = measurable.measure(constraints)
    val more = extra.roundToPx()
    layout(placeable.width, placeable.height + more) {
        placeable.placeRelative(0, more)
    }
}

@Composable
fun Scopes() {
    val todos = remember { mutableStateOf(mutableListOf("bir")) }
    val tags = remember { mutableStateListOf("a") }
    Canvas(Modifier.size(40.dp).clickable {
        todos.update { it.add("iki") }
        tags.add("b")
    }.pointerInput(Unit) {
        detectDragGestures(onDragStart = { tags.clear() }, onDrag = { change, _ -> tags.add("${change.position}") })
    }) {
        rotate(45f) { drawCircle(Color.Blue, center.x / 4) }
    }
}

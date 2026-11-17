// SubcomposeLayout: the C++ Kompoz's spelling (SubkompozLayout, subkompoz) found as Compose's, a slot's content
// keeping the measure scope it uses (a clone of it, not the reference), Int.toDp() as Density's to_dp of a float;
// BoxWithConstraints' scope members read as its getters.

@Kompozable
fun Underlined(content: @Kompozable () -> Unit) {
    SubkompozLayout(modifier = Modifier.testTag("underlined")) { constraints ->
        val main = subkompoz("main", content).map { it.measure(constraints.copy(minWidth = 0, minHeight = 0)) }
        val width = main.maxOf { it.width }
        val height = main.sumOf { it.height }
        val line = subkompoz("line") { Box(modifier = Modifier.size(width.toDp(), 2.dp)) }.map { it.measure(constraints) }
        layout(width, height + 2.dp.roundToPx()) {
            main.forEach { it.placeRelative(0, 0) }
            line.forEach { it.placeRelative(0, height) }
        }
    }
}

@Composable
fun Adaptive() {
    BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
        Text(if (maxWidth < 600.dp) "compact ${constraints.maxWidth}" else "expanded ${minHeight}")
    }
}

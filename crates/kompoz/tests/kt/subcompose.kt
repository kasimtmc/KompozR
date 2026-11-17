// SubcomposeLayout and BoxWithConstraints as Compose writes them (kotlin_subcompose.rs reads this file through
// kompoz_file!): a layout whose second slot is composed from what the first measured, a slot reuse policy given
// through a remembered state, and BoxWithConstraints' scope (its constraints, its bounds in dp).
package com.example.subcompose

@Composable
fun TitleAndSide(title: String) {
    val state = remember { SubcomposeLayoutState(SubcomposeSlotReusePolicy(2)) }
    SubcomposeLayout(state = state, modifier = Modifier.testTag("title and side")) { constraints ->
        val loose = constraints.copy(minWidth = 0, minHeight = 0)
        val titles = subcompose("title") {
            Box(modifier = Modifier.size(100.dp, 10.dp).testTag(title))
        }.map { it.measure(loose) }
        val titleWidth = titles.sumOf { it.width }
        val sides = subcompose("side") {
            Box(modifier = Modifier.size(20.dp, 10.dp).testTag("side"))
        }.map { it.measure(loose) }
        layout(titleWidth + sides.sumOf { it.width }, 10) {
            titles.forEach { it.place(0, 0) }
            sides.forEach { it.place(titleWidth, 0) }
        }
    }
}

@Composable
fun Adaptive() {
    Column {
        TitleAndSide("başlık")
        BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
            val size = if (maxWidth < 600.dp) "compact" else "expanded"
            Text("$size ${constraints.maxWidth}x${maxHeight.value.toInt()} min ${minWidth.value.toInt()}x${minHeight.value.toInt()}")
        }
    }
}

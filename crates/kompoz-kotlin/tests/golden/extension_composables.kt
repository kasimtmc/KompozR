// Composable extension functions (a hidden composable of a clone of the receiver, reached by a method of its
// type), a state's type argument typing what it holds, and a `color` argument for a `brush: impl Into<Brush>`.

@Composable
fun TabIndicatorScope.UnderlineIndicator(index: Int, thick: Boolean = false) {
    Box(Modifier.tabIndicatorOffset(index).height(if (thick) 6.dp else 2.dp))
}

@Composable
fun Tabs() {
    var tab by remember { mutableStateOf(0) }
    SecondaryTabRow(selectedTabIndex = tab, indicator = { UnderlineIndicator(tab, thick = true) }) {
        Tab(selected = tab == 0, onClick = { tab = 0 }, text = { Text("bir") })
    }
}

@Composable
fun Held() {
    var held by remember { mutableStateOf<String?>(null) }
    val shown = held ?: "yok"
    Text(shown)
}

@Composable
fun Framed() {
    Box(Modifier.size(10.dp).drawWithContent { drawRoundRect(color = Color.Red, cornerRadius = CornerRadius(2f)) })
}

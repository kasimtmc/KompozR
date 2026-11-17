// A `.kt` file (kompoz_file!): its tokens share one span, so each call keeping something in the composition gets a
// key of its Kotlin line and column.

@Composable
fun Screen(flow: StateFlow<Int>) {
    var shown by remember { mutableStateOf(true) }
    val value by flow.collectAsState()
    Column {
        if (shown) Text("görünür $value")
        Text("hep")
        LaunchedEffect(shown) { println(shown) }
    }
}

// A shared closure (`Rc<dyn Fn(f32) -> f32>`) is a Kotlin function value: kept in a `val`, given on to a lambda
// parameter of its type, called; still one kept under a declared function type.
@Composable
fun SharedClosures() {
    val threshold = SwipeToDismissBoxDefaults.positionalThreshold
    val state = remember { SwipeToDismissBoxState(SwipeToDismissBoxValue.EndToStart, positionalThreshold = threshold) }
    val own = rememberSwipeToDismissBoxState(positionalThreshold = threshold)
    val typed: (Float) -> Float = SwipeToDismissBoxDefaults.positionalThreshold
    val third = rememberSwipeToDismissBoxState(positionalThreshold = typed)
    val fourth = rememberSwipeToDismissBoxState(positionalThreshold = typed)
    Text("${threshold(400f)} ${state.currentValue == own.currentValue} ${third.currentValue == fourth.currentValue}")
}

// A lambda kept in a `val` (a closure moving its captures in) is a Kotlin function value too: given to two
// parameters, each a clone of it; called in place.
@Composable
fun LambdaValues() {
    var count by remember { mutableStateOf(0) }
    val onRefresh: () -> Unit = { count += 1 }
    val described = { "sayı $count" }
    Column {
        Button(onClick = onRefresh) { Text(described()) }
        PullToRefreshBox(isRefreshing = false, onRefresh = onRefresh) { Text("$count") }
    }
}

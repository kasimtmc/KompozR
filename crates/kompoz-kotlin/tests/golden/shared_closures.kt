// A shared closure (`Rc<dyn Fn(f32) -> f32>`) is a Kotlin function value: kept in a `val`, given on to a lambda
// parameter of its type, called.
@Composable
fun SharedClosures() {
    val threshold = SwipeToDismissBoxDefaults.positionalThreshold
    val state = remember { SwipeToDismissBoxState(SwipeToDismissBoxValue.EndToStart, positionalThreshold = threshold) }
    val own = rememberSwipeToDismissBoxState(positionalThreshold = threshold)
    Text("${threshold(400f)} ${state.currentValue == own.currentValue}")
}

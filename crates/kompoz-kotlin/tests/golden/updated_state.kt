// `rememberUpdatedState(newValue)`: the runtime's `remember_updated_state!` (Kotlin's `==` of the value read where
// its type is known), a `State<T>` read through `by` or `.value`.

@Composable
fun Landing(onTimeout: () -> Unit, red: Boolean) {
    val currentOnTimeout by rememberUpdatedState(onTimeout)
    val color = rememberUpdatedState(newValue = if (red) Color.Red else Color.Blue)
    LaunchedEffect(true) {
        delay(300)
        currentOnTimeout()
    }
    Text(if (color.value == Color.Red) "red" else "blue")
}

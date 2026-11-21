// Snackbars and tooltips as Compose writes them (WP5): `showSnackbar(message)` chosen over
// `showSnackbar(visuals: SnackbarVisuals)` (a string isn't a shared trait object: Kotlin's applicable overload);
// `PlainTooltip { … }` in a tooltip lambda, a method of the tooltip scope's composables trait (implemented for its
// trait object); a flow's `collect` taking a suspending collector; `tooltipState.show()` with its default priority.
@Composable
fun Snackbars() {
    val snackbarHostState = remember { SnackbarHostState() }
    val tooltipState = rememberTooltipState()
    val scope = rememberCoroutineScope()
    var count by remember { mutableStateOf(0) }
    LaunchedEffect(Unit) {
        snapshotFlow { count }.collect { n ->
            if (snackbarHostState.showSnackbar("Snackbar # $n", actionLabel = "Undo") == SnackbarResult.ActionPerformed) count = 0
        }
    }
    Column {
        TooltipBox(
            positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
            tooltip = { PlainTooltip(caretShape = TooltipDefaults.caretShape()) { Text("Add") } },
            state = tooltipState,
        ) {
            IconButton(onClick = { count++ }) { Icon(Icons.Filled.Add, contentDescription = "Add") }
        }
        Button(onClick = { scope.launch { tooltipState.show() } }) { Text("Show") }
        Button(onClick = { scope.launch { snackbarHostState.showSnackbar("Shown") } }) { Text("Snackbar") }
        SnackbarHost(snackbarHostState)
    }
}

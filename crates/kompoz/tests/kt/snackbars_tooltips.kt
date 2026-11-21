// Material's snackbars and plain tooltips as Compose's samples write them: material3 1.5's ScaffoldSamples' snackbar
// samples (their Scaffold is WP4a's: its slots are laid out by ScaffoldStandIn here, the body filling the screen, the
// host at the bottom, the floating action button at the bottom end) and TooltipSamples' plain tooltips (the rich ones
// are WP7's). ScaffoldWithCustomSnackbar declares a class implementing SnackbarVisuals: classes are stage 14's (its
// Rust port is in kompoz-material3's tests/snackbars_tooltips.rs).

@Composable
fun ScaffoldStandIn(
    snackbarHost: @Composable () -> Unit,
    floatingActionButton: @Composable () -> Unit,
    content: @Composable () -> Unit,
) {
    Box(modifier = Modifier.fillMaxSize()) {
        content()
        Box(modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 88.dp)) { snackbarHost() }
        Box(modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp)) { floatingActionButton() }
    }
}

@Composable
fun ScaffoldWithSimpleSnackbar() {
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    ScaffoldStandIn(
        snackbarHost = { SnackbarHost(snackbarHostState) },
        floatingActionButton = {
            var clickCount by remember { mutableStateOf(0) }
            ExtendedFloatingActionButton(
                onClick = {
                    // show snackbar as a suspend function
                    scope.launch { snackbarHostState.showSnackbar("Snackbar # ${++clickCount}") }
                }
            ) {
                Text("Show snackbar")
            }
        },
        content = {
            Text(
                text = "Body content",
                modifier = Modifier.fillMaxSize().wrapContentSize(),
            )
        },
    )
}

@Composable
fun ScaffoldWithIndefiniteSnackbar() {
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    ScaffoldStandIn(
        snackbarHost = { SnackbarHost(snackbarHostState) },
        floatingActionButton = {
            var clickCount by remember { mutableStateOf(0) }
            ExtendedFloatingActionButton(
                onClick = {
                    // show snackbar as a suspend function
                    scope.launch {
                        snackbarHostState.showSnackbar(
                            message = "Snackbar # ${++clickCount}",
                            actionLabel = "Action",
                            withDismissAction = true,
                            duration = SnackbarDuration.Indefinite,
                        )
                    }
                }
            ) {
                Text("Show snackbar")
            }
        },
        content = {
            Text(
                text = "Body content",
                modifier = Modifier.fillMaxSize().wrapContentSize(),
            )
        },
    )
}

// (ScaffoldWithCoroutinesSnackbar makes its queue with kotlinx's `Channel<Int>(Channel.CONFLATED)`: KompozR's
// coroutines have the channel (`Channel::conflated()`) but not kotlinx's `Channel(capacity)` factory and its
// companion's constants, so that sample is in Rust only (kompoz-material3's tests). Its flow collected with a
// suspending collector is written here of a snapshot flow instead.)
@Composable
fun SnackbarForEachCount() {
    val snackbarHostState = remember { SnackbarHostState() }
    var clickCount by remember { mutableStateOf(0) }
    LaunchedEffect(Unit) {
        snapshotFlow { clickCount }.collect { index ->
            if (index > 0) {
                val result = snackbarHostState.showSnackbar(message = "Snackbar # $index", actionLabel = "Action on $index")
                when (result) {
                    SnackbarResult.ActionPerformed -> clickCount = 0
                    SnackbarResult.Dismissed -> {}
                }
            }
        }
    }
    ScaffoldStandIn(
        snackbarHost = { SnackbarHost(snackbarHostState) },
        floatingActionButton = {
            ExtendedFloatingActionButton(onClick = { clickCount++ }) { Text("Show snackbar") }
        },
        content = { Text("Snackbar demo: $clickCount", modifier = Modifier.fillMaxSize().wrapContentSize()) },
    )
}

@Composable
fun ScaffoldWithMultilineSnackbar() {
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    ScaffoldStandIn(
        snackbarHost = {
            SnackbarHost(snackbarHostState) { data ->
                Snackbar(modifier = Modifier.testTag("multiline")) {
                    // The Material spec recommends a maximum of 2 lines of text.
                    Text(data.visuals.message, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
            }
        },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = {
                    scope.launch {
                        val longMessage =
                            "Very very very very very very very very very very very very very " +
                                "very very very very very very very very very very very very " +
                                "very very very very very very very very very very long message"
                        snackbarHostState.showSnackbar(longMessage)
                    }
                }
            ) {
                Text("Show snackbar")
            }
        },
        content = {
            Text(
                text = "Multiline Snackbar Demo",
                modifier = Modifier.fillMaxSize().wrapContentSize(),
            )
        },
    )
}

@Composable
fun PlainTooltipSample() {
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = { PlainTooltip { Text("Add to favorites") } },
        state = rememberTooltipState(),
    ) {
        IconButton(onClick = { /* Icon button's click event */ }) {
            Icon(imageVector = Icons.Filled.Favorite, contentDescription = "Localized Description")
        }
    }
}

@Composable
fun PlainTooltipWithManualInvocationSample() {
    val tooltipState = rememberTooltipState()
    val scope = rememberCoroutineScope()
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        TooltipBox(
            positionProvider =
                TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
            tooltip = { PlainTooltip { Text("Add to list") } },
            state = tooltipState,
        ) {
            IconButton(onClick = { /* Icon button's click event */ }) {
                Icon(
                    imageVector = Icons.Filled.AddCircle,
                    contentDescription = "Localized Description",
                )
            }
        }
        Spacer(Modifier.requiredHeight(30.dp))
        OutlinedButton(onClick = { scope.launch { tooltipState.show() } }) {
            Text("Display tooltip")
        }
    }
}

@Composable
fun PlainTooltipWithCaret() {
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = {
            PlainTooltip(caretShape = TooltipDefaults.caretShape()) { Text("Add to favorites") }
        },
        state = rememberTooltipState(),
    ) {
        IconButton(onClick = { /* Icon button's click event */ }) {
            Icon(imageVector = Icons.Filled.Favorite, contentDescription = "Localized Description")
        }
    }
}

@Composable
fun PlainTooltipWithCaretBelowAnchor() {
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Below),
        tooltip = {
            PlainTooltip(caretShape = TooltipDefaults.caretShape()) { Text("Add to favorites") }
        },
        state = rememberTooltipState(),
    ) {
        IconButton(onClick = { /* Icon button's click event */ }) {
            Icon(imageVector = Icons.Filled.Favorite, contentDescription = "Localized Description")
        }
    }
}

@Composable
fun PlainTooltipWithCaretLeftOfAnchor() {
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Left),
        tooltip = {
            PlainTooltip(caretShape = TooltipDefaults.caretShape()) { Text("Add to favorites") }
        },
        state = rememberTooltipState(),
    ) {
        IconButton(onClick = { /* Icon button's click event */ }) {
            Icon(imageVector = Icons.Filled.Favorite, contentDescription = "Localized Description")
        }
    }
}

@Composable
fun PlainTooltipWithCaretRightOfAnchor() {
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Right),
        tooltip = {
            PlainTooltip(caretShape = TooltipDefaults.caretShape()) { Text("Add to favorites") }
        },
        state = rememberTooltipState(),
    ) {
        IconButton(onClick = { /* Icon button's click event */ }) {
            Icon(imageVector = Icons.Filled.Favorite, contentDescription = "Localized Description")
        }
    }
}

@Composable
fun PlainTooltipWithCaretStartOfAnchor() {
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Start),
        tooltip = {
            PlainTooltip(caretShape = TooltipDefaults.caretShape()) { Text("Add to favorites") }
        },
        state = rememberTooltipState(),
    ) {
        IconButton(onClick = { /* Icon button's click event */ }) {
            Icon(imageVector = Icons.Filled.Favorite, contentDescription = "Localized Description")
        }
    }
}

@Composable
fun PlainTooltipWithCaretEndOfAnchor() {
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.End),
        tooltip = {
            PlainTooltip(caretShape = TooltipDefaults.caretShape()) { Text("Add to favorites") }
        },
        state = rememberTooltipState(),
    ) {
        IconButton(onClick = { /* Icon button's click event */ }) {
            Icon(imageVector = Icons.Filled.Favorite, contentDescription = "Localized Description")
        }
    }
}

@Composable
fun PlainTooltipWithCustomCaret() {
    TooltipBox(
        positionProvider =
            TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
        tooltip = {
            PlainTooltip(caretShape = TooltipDefaults.caretShape(DpSize(24.dp, 12.dp))) {
                Text("Add to favorites")
            }
        },
        state = rememberTooltipState(),
    ) {
        IconButton(onClick = { /* Icon button's click event */ }) {
            Icon(imageVector = Icons.Filled.Favorite, contentDescription = "Localized Description")
        }
    }
}

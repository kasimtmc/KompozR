// Material's swipe to dismiss and pull to refresh as Compose's samples write them (material3 1.5's
// SwipeToDismissSamples.kt), and the rest of their API as a Compose app writes it (SwipeForms, PullForms).
// PullToRefreshSamples.kt's samples all compose a Scaffold with a TopAppBar (WP4a): they come with that package.

@Preview
@Sampled
@Composable
fun SwipeToDismissListItems() {
    val dismissState = rememberSwipeToDismissBoxState()
    var isVisible by remember { mutableStateOf(true) }
    val scope = rememberCoroutineScope()
    if (isVisible) {
        SwipeToDismissBox(
            state = dismissState,
            backgroundContent = {
                val color by
                    animateColorAsState(
                        when (dismissState.targetValue) {
                            SwipeToDismissBoxValue.Settled -> Color.LightGray
                            SwipeToDismissBoxValue.StartToEnd -> Color.Green
                            SwipeToDismissBoxValue.EndToStart -> Color.Red
                        }
                    )
                Box(Modifier.fillMaxSize().background(color))
            },
            onDismiss = { direction ->
                if (direction == SwipeToDismissBoxValue.EndToStart) {
                    isVisible = false
                } else {
                    scope.launch { dismissState.reset() }
                }
            },
        ) {
            OutlinedCard(shape = RectangleShape) {
                ListItem(
                    headlineContent = { Text("Cupcake") },
                    supportingContent = { Text("Swipe me left or right!") },
                )
            }
        }
    }
}

// The swipe-to-dismiss API by name: a state of its own threshold, one made by the constructor with the defaults'
// threshold, a direction turned off, the dismiss callback, the state moved from code.
@Composable
fun SwipeForms() {
    var log by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()
    val threshold = SwipeToDismissBoxDefaults.positionalThreshold
    val first = rememberSwipeToDismissBoxState(initialValue = SwipeToDismissBoxValue.Settled, positionalThreshold = { it / 2 })
    val second = remember { SwipeToDismissBoxState(SwipeToDismissBoxValue.EndToStart, positionalThreshold = threshold) }
    Column {
        SwipeToDismissBox(
            state = first,
            backgroundContent = { Text(if (first.dismissDirection == SwipeToDismissBoxValue.EndToStart) "sola" else "yerinde") },
            modifier = Modifier.testTag("ilk"),
            enableDismissFromStartToEnd = false,
            gesturesEnabled = true,
            onDismiss = { log = log + if (it == SwipeToDismissBoxValue.EndToStart) " sola" else " sağa" },
        ) {
            Text("ilk satır", modifier = Modifier.fillMaxWidth().height(60.dp))
        }
        SwipeToDismissBox(state = second, backgroundContent = {}) {
            Text("ikinci", modifier = Modifier.fillMaxWidth().height(60.dp).testTag("ikinci"))
        }
        Button(onClick = { scope.launch { second.reset() } }) { Text("geri getir") }
        Button(onClick = { scope.launch { first.snapTo(SwipeToDismissBoxValue.Settled) } }) { Text("yerine koy") }
        val settled = second.currentValue == SwipeToDismissBoxValue.Settled
        Text("günlük:$log · ${if (settled) "ikinci yerinde" else "ikinci gitti"} · ${(first.progress * 100).roundToInt()}")
        Text("eşik ${threshold(400f).roundToInt()}")
    }
}

// The pull-to-refresh API by name: Modifier.pullToRefresh with every parameter, the default Indicator and an
// IndicatorBox of content of its own with the defaults' colors and elevation, the remembered state and one made by its
// function, moved from code; a PullToRefreshBox with every parameter, its indicator the LoadingIndicator.
@Composable
fun PullForms() {
    var refreshing by remember { mutableStateOf(false) }
    var refreshes by remember { mutableStateOf(0) }
    val state = rememberPullToRefreshState()
    val other = remember { PullToRefreshState() }
    var boxRefreshing by remember { mutableStateOf(false) }
    val boxState = rememberPullToRefreshState()
    val scope = rememberCoroutineScope()
    Column {
        Box(
            modifier = Modifier.fillMaxWidth().height(400.dp).pullToRefresh(
                isRefreshing = refreshing,
                state = state,
                enabled = true,
                threshold = PullToRefreshDefaults.PositionalThreshold,
                onRefresh = {
                    refreshes++
                    refreshing = true
                },
            ).testTag("çekilir")
        ) {
            LazyColumn(modifier = Modifier.fillMaxSize()) { items(50) { Text("Satır $it", modifier = Modifier.height(50.dp)) } }
            PullToRefreshDefaults.Indicator(
                state = state,
                isRefreshing = refreshing,
                modifier = Modifier.align(Alignment.TopStart).testTag("ok"),
                containerColor = PullToRefreshDefaults.indicatorContainerColor,
                color = PullToRefreshDefaults.indicatorColor,
                maxDistance = PullToRefreshDefaults.IndicatorMaxDistance,
            )
            PullToRefreshDefaults.IndicatorBox(
                state = state,
                isRefreshing = refreshing,
                modifier = Modifier.align(Alignment.TopCenter),
                containerColor = PullToRefreshDefaults.indicatorContainerColor,
                elevation = PullToRefreshDefaults.Elevation,
            ) {
                Box(modifier = Modifier.size(16.dp).background(PullToRefreshDefaults.indicatorColor).testTag("gösterge"))
            }
        }
        Button(onClick = { refreshing = false }) { Text("bitti") }
        Button(onClick = {
            scope.launch {
                other.snapTo(0.5f)
                other.animateToThreshold()
            }
        }) { Text("öbürü") }
        Text("yenileme $refreshes · ${(state.distanceFraction * 100).roundToInt()} · ${(other.distanceFraction * 100).roundToInt()} · ${state.isAnimating}")
        PullToRefreshBox(
            isRefreshing = boxRefreshing,
            onRefresh = { boxRefreshing = true },
            modifier = Modifier.fillMaxWidth().height(200.dp).testTag("kutu"),
            state = boxState,
            contentAlignment = Alignment.TopStart,
            indicator = {
                PullToRefreshDefaults.LoadingIndicator(
                    state = boxState,
                    isRefreshing = boxRefreshing,
                    modifier = Modifier.align(Alignment.TopCenter).testTag("yükleme"),
                    containerColor = PullToRefreshDefaults.loadingIndicatorContainerColor,
                    color = PullToRefreshDefaults.loadingIndicatorColor,
                    elevation = PullToRefreshDefaults.LoadingIndicatorElevation,
                    maxDistance = PullToRefreshDefaults.IndicatorMaxDistance,
                )
            },
            enabled = true,
            threshold = 60.dp,
        ) {
            LazyColumn(modifier = Modifier.fillMaxSize()) { items(20) { Text("Öğe $it", modifier = Modifier.height(50.dp)) } }
        }
        Text("kutu: $boxRefreshing")
    }
}

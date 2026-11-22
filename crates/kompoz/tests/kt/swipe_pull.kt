// Material's swipe to dismiss and pull to refresh as Compose's samples write them (material3 1.5's
// SwipeToDismissSamples.kt, PullToRefreshSamples.kt — each a Scaffold with a TopAppBar — and LoadingIndicatorSamples.kt's
// LoadingIndicatorPullToRefreshSample), and the rest of their API as a Compose app writes it (SwipeForms, PullForms).

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

// ---- material3 1.5's PullToRefreshSamples.kt (+ LoadingIndicatorSamples.kt's LoadingIndicatorPullToRefreshSample), as
// written. PullToRefreshViewModelSample (`object : ViewModel()`) and PullToRefreshSampleCustomState (`object :
// PullToRefreshState`) declare object expressions: Kotlin's classes are stage 14's (their Rust ports are in
// kompoz-material3's tests/swipe_pull.rs).

@OptIn(ExperimentalMaterial3Api::class)
@Sampled
@Composable
@Preview
fun PullToRefreshSample() {
    var itemCount by remember { mutableIntStateOf(15) }
    var isRefreshing by remember { mutableStateOf(false) }
    val state = rememberPullToRefreshState()
    val coroutineScope = rememberCoroutineScope()
    val onRefresh: () -> Unit = {
        isRefreshing = true
        coroutineScope.launch {
            delay(5000)
            itemCount += 5
            isRefreshing = false
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Title") },
                // Provide an accessible alternative to trigger refresh.
                actions = {
                    IconButton(onClick = onRefresh) {
                        Icon(Icons.Filled.Refresh, "Trigger Refresh")
                    }
                },
            )
        }
    ) {
        PullToRefreshBox(
            modifier = Modifier.padding(it),
            state = state,
            isRefreshing = isRefreshing,
            onRefresh = onRefresh,
        ) {
            LazyColumn(Modifier.fillMaxSize()) {
                items(itemCount) { ListItem({ Text(text = "Item ${itemCount - it}") }) }
            }
        }
    }
}

@Sampled
@Composable
@Preview
@OptIn(ExperimentalMaterial3ExpressiveApi::class, ExperimentalMaterial3Api::class)
fun PullToRefreshWithLoadingIndicatorSample() {
    var itemCount by remember { mutableIntStateOf(15) }
    var isRefreshing by remember { mutableStateOf(false) }
    val state = rememberPullToRefreshState()
    val coroutineScope = rememberCoroutineScope()
    val onRefresh: () -> Unit = {
        isRefreshing = true
        coroutineScope.launch {
            delay(5000)
            itemCount += 5
            isRefreshing = false
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Title") },
                // Provide an accessible alternative to trigger refresh.
                actions = {
                    IconButton(onClick = onRefresh) {
                        Icon(Icons.Filled.Refresh, "Trigger Refresh")
                    }
                },
            )
        }
    ) {
        PullToRefreshBox(
            modifier = Modifier.padding(it),
            state = state,
            isRefreshing = isRefreshing,
            onRefresh = onRefresh,
            indicator = {
                PullToRefreshDefaults.LoadingIndicator(
                    state = state,
                    isRefreshing = isRefreshing,
                    modifier = Modifier.align(Alignment.TopCenter),
                )
            },
        ) {
            LazyColumn(Modifier.fillMaxSize()) {
                items(itemCount) { ListItem({ Text(text = "Item ${itemCount - it}") }) }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Sampled
@Composable
@Preview
fun PullToRefreshScalingSample() {
    var itemCount by remember { mutableIntStateOf(15) }
    var isRefreshing by remember { mutableStateOf(false) }
    val state = rememberPullToRefreshState()
    val coroutineScope = rememberCoroutineScope()
    val onRefresh: () -> Unit = {
        isRefreshing = true
        coroutineScope.launch {
            // fetch something
            delay(5000)
            itemCount += 5
            isRefreshing = false
        }
    }

    val scaleFraction = {
        if (isRefreshing) 1f
        else LinearOutSlowInEasing.transform(state.distanceFraction).coerceIn(0f, 1f)
    }

    Scaffold(
        modifier =
            Modifier.pullToRefresh(
                state = state,
                isRefreshing = isRefreshing,
                onRefresh = onRefresh,
            ),
        topBar = {
            TopAppBar(
                title = { Text("TopAppBar") },
                // Provide an accessible alternative to trigger refresh.
                actions = {
                    IconButton(onClick = onRefresh) {
                        Icon(Icons.Filled.Refresh, "Trigger Refresh")
                    }
                },
            )
        },
    ) {
        Box(Modifier.padding(it)) {
            LazyColumn(Modifier.fillMaxSize()) {
                if (!isRefreshing) {
                    items(itemCount) { ListItem({ Text(text = "Item ${itemCount - it}") }) }
                }
            }
            Box(
                Modifier.align(Alignment.TopCenter).graphicsLayer {
                    scaleX = scaleFraction()
                    scaleY = scaleFraction()
                }
            ) {
                PullToRefreshDefaults.Indicator(state = state, isRefreshing = isRefreshing)
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Sampled
@Composable
@Preview
fun PullToRefreshLinearProgressIndicatorSample() {
    var itemCount by remember { mutableIntStateOf(15) }
    var isRefreshing by remember { mutableStateOf(false) }
    val state = rememberPullToRefreshState()
    val coroutineScope = rememberCoroutineScope()
    val onRefresh: () -> Unit = {
        isRefreshing = true
        coroutineScope.launch {
            // fetch something
            delay(5000)
            itemCount += 5
            isRefreshing = false
        }
    }

    Scaffold(
        modifier =
            Modifier.pullToRefresh(
                state = state,
                isRefreshing = isRefreshing,
                onRefresh = onRefresh,
            ),
        topBar = {
            TopAppBar(
                title = { Text("TopAppBar") },
                // Provide an accessible alternative to trigger refresh.
                actions = {
                    IconButton(onClick = onRefresh) {
                        Icon(Icons.Filled.Refresh, "Trigger Refresh")
                    }
                },
            )
        },
    ) {
        Box(Modifier.padding(it)) {
            LazyColumn(Modifier.fillMaxSize()) {
                if (!isRefreshing) {
                    items(itemCount) { ListItem({ Text(text = "Item ${itemCount - it}") }) }
                }
            }
            if (isRefreshing) {
                LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            } else {
                LinearProgressIndicator(
                    modifier = Modifier.fillMaxWidth(),
                    progress = { state.distanceFraction },
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Sampled
@Composable
@Preview
fun PullToRefreshCustomIndicatorWithDefaultTransform() {
    var itemCount by remember { mutableIntStateOf(15) }
    var isRefreshing by remember { mutableStateOf(false) }
    val state = rememberPullToRefreshState()
    val coroutineScope = rememberCoroutineScope()
    val onRefresh: () -> Unit = {
        isRefreshing = true
        coroutineScope.launch {
            delay(1500)
            itemCount += 5
            isRefreshing = false
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Title") },
                // Provide an accessible alternative to trigger refresh.
                actions = {
                    IconButton(onClick = onRefresh) {
                        Icon(Icons.Filled.Refresh, "Trigger Refresh")
                    }
                },
            )
        }
    ) {
        PullToRefreshBox(
            modifier = Modifier.padding(it),
            state = state,
            isRefreshing = isRefreshing,
            onRefresh = onRefresh,
            indicator = {
                PullToRefreshDefaults.IndicatorBox(
                    state = state,
                    isRefreshing = isRefreshing,
                    modifier = Modifier.align(Alignment.TopCenter),
                    elevation = 0.dp,
                ) {
                    if (isRefreshing) {
                        CircularProgressIndicator()
                    } else {
                        CircularProgressIndicator(
                            progress = { state.distanceFraction },
                            trackColor = ProgressIndicatorDefaults.circularIndeterminateTrackColor,
                        )
                    }
                }
            },
        ) {
            LazyColumn(Modifier.fillMaxSize()) {
                items(itemCount) { ListItem({ Text(text = "Item ${itemCount - it}") }) }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class, ExperimentalMaterial3Api::class)
@Sampled
@Composable
fun LoadingIndicatorPullToRefreshSample() {
    var itemCount by remember { mutableStateOf(15) }
    var isRefreshing by remember { mutableStateOf(false) }
    val state = rememberPullToRefreshState()
    val coroutineScope = rememberCoroutineScope()
    val onRefresh: () -> Unit = {
        isRefreshing = true
        coroutineScope.launch {
            // fetch something
            delay(5000)
            itemCount += 5
            isRefreshing = false
        }
    }

    val scaleFraction = {
        if (isRefreshing) 1f
        else LinearOutSlowInEasing.transform(state.distanceFraction).coerceIn(0f, 1f)
    }

    Scaffold(
        modifier =
            Modifier.pullToRefresh(
                state = state,
                isRefreshing = isRefreshing,
                onRefresh = onRefresh,
            ),
        topBar = {
            TopAppBar(
                title = { Text("TopAppBar") },
                // Provide an accessible alternative to trigger refresh.
                actions = {
                    IconButton(onClick = onRefresh) {
                        Icon(Icons.Filled.Refresh, "Trigger Refresh")
                    }
                },
            )
        },
    ) {
        Box(Modifier.padding(it)) {
            LazyColumn(Modifier.fillMaxSize()) {
                if (!isRefreshing) {
                    items(itemCount) { ListItem({ Text(text = "Item ${itemCount - it}") }) }
                }
            }
            Box(
                Modifier.align(Alignment.TopCenter).graphicsLayer {
                    scaleX = scaleFraction()
                    scaleY = scaleFraction()
                }
            ) {
                PullToRefreshDefaults.LoadingIndicator(state = state, isRefreshing = isRefreshing)
            }
        }
    }
}

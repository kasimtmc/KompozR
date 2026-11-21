// Material's bottom sheets as Compose's samples write them (material3 1.5's BottomSheetSamples.kt and
// DragHandleSamples.kt). ModalBottomSheetSample composes a Checkbox (WP2) and an OutlinedTextField (WP3),
// BottomSheetScaffoldNestedScrollSample a TopAppBar with its scroll behavior (WP4a): they come with those packages.

@OptIn(ExperimentalMaterial3Api::class)
@Sampled
@Preview
@Composable
fun ManualModalBottomSheetSample() {
    var showSheet by remember { mutableStateOf(false) }
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Button(onClick = { showSheet = true }) { Text("Show Manual Sheet") }
    }
    val sheetState = rememberModalBottomSheetState()

    if (showSheet) {
        Scrim("scrim", onClick = { showSheet = false })
        BottomSheet(
            modifier = Modifier,
            state = sheetState,
            onDismissRequest = { showSheet = false },
        ) {
            LazyColumn {
                items(25) {
                    ListItem(
                        headlineContent = { Text("Item $it") },
                        leadingContent = {
                            Icon(
                                Icons.Default.Favorite,
                                contentDescription = "Localized description",
                            )
                        },
                        colors =
                            ListItemDefaults.colors(
                                containerColor = MaterialTheme.colorScheme.surfaceContainerLow
                            ),
                    )
                }
            }
        }
    }
    // Handle sheet animations
    LaunchedEffect(showSheet) {
        if (showSheet) {
            sheetState.show()
        } else {
            sheetState.hide()
        }
    }
}

@Preview
@Sampled
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SimpleBottomSheetScaffoldSample() {
    val scope = rememberCoroutineScope()
    val scaffoldState = rememberBottomSheetScaffoldState()

    BottomSheetScaffold(
        scaffoldState = scaffoldState,
        sheetPeekHeight = 128.dp,
        sheetContent = {
            Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                Box(Modifier.fillMaxWidth().height(128.dp), contentAlignment = Alignment.Center) {
                    Text("Swipe up to expand sheet")
                }
                Text("Sheet content")
                Button(
                    modifier =
                        Modifier.padding(bottom = 64.dp).focusProperties {
                            // Make sure the button is not keyboard focusable when it's offscreen.
                            canFocus =
                                scaffoldState.bottomSheetState.currentValue == SheetValue.Expanded
                        },
                    onClick = { scope.launch { scaffoldState.bottomSheetState.partialExpand() } },
                ) {
                    Text("Click to collapse sheet")
                }
            }
        },
    ) { innerPadding ->
        Box(
            modifier = Modifier.fillMaxSize().padding(innerPadding),
            contentAlignment = Alignment.Center,
        ) {
            Text("Scaffold Content")
        }
    }
}

@Preview
@Sampled
@Composable
fun VerticalDragHandleSample() {
    var offsetX by remember { mutableStateOf(0f) }
    var screenSize by remember { mutableStateOf(IntSize.Zero) }
    val density = LocalDensity.current

    Box(
        modifier =
            Modifier.fillMaxSize().onGloballyPositioned { layoutCoordinates ->
                screenSize = layoutCoordinates.size
                if (offsetX == 0f) {
                    offsetX = screenSize.width / 2f
                }
            }
    ) {
        Surface(
            modifier = Modifier.width(with(density) { offsetX.toDp() }).fillMaxHeight(),
            color = MaterialTheme.colorScheme.surfaceContainerHighest,
            shape = RoundedCornerShape(0.dp, 24.dp, 24.dp, 0.dp),
        ) {
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.CenterEnd) {
                VerticalDragHandle(
                    modifier =
                        Modifier.draggable(
                                orientation = Orientation.Horizontal,
                                state =
                                    rememberDraggableState { delta ->
                                        offsetX =
                                            (offsetX + delta).coerceIn(
                                                with(density) { 48.dp.toPx() },
                                                screenSize.width.toFloat(),
                                            )
                                    },
                            )
                            .systemGestureExclusion() // To avoid colliding with the back gesture
                )
            }
        }
    }
}

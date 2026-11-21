// Material's tabs and dialogs as Compose's samples write them (material3 1.5's TabSamples and AlertDialogSamples,
// as written; their @Sampled/@Preview annotations left out).

// ---- TabSamples.kt ----

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun PrimaryTextTabs() {
    var state by remember { mutableStateOf(0) }
    val titles = listOf("Tab 1", "Tab 2", "Tab 3 with lots of text")
    Column {
        PrimaryTabRow(selectedTabIndex = state) {
            titles.forEachIndexed { index, title ->
                Tab(
                    selected = state == index,
                    onClick = { state = index },
                    text = { Text(text = title, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                )
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Primary tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun PrimaryIconTabs() {
    var state by remember { mutableStateOf(0) }
    val icons = listOf(Icons.Filled.Favorite, Icons.Filled.Favorite, Icons.Filled.Favorite)
    Column {
        PrimaryTabRow(selectedTabIndex = state) {
            icons.forEachIndexed { index, icon ->
                // Icon-only tab should have a tooltip associated with it.
                TooltipBox(
                    positionProvider =
                        TooltipDefaults.rememberTooltipPositionProvider(
                            TooltipAnchorPosition.Above
                        ),
                    tooltip = { PlainTooltip { Text("Favorite") } },
                    state = rememberTooltipState(),
                ) {
                    Tab(
                        selected = state == index,
                        onClick = { state = index },
                        icon = { Icon(icon, contentDescription = "Favorite") },
                    )
                }
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Icon tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun SecondaryTextTabs() {
    var state by remember { mutableStateOf(0) }
    val titles = listOf("Tab 1", "Tab 2", "Tab 3 with lots of text")
    Column {
        SecondaryTabRow(selectedTabIndex = state) {
            titles.forEachIndexed { index, title ->
                Tab(
                    selected = state == index,
                    onClick = { state = index },
                    text = { Text(text = title, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                )
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Secondary tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun TextTabs() {
    var state by remember { mutableStateOf(0) }
    val titles = listOf("Tab 1", "Tab 2", "Tab 3 with lots of text")
    Column {
        PrimaryTabRow(selectedTabIndex = state) {
            titles.forEachIndexed { index, title ->
                Tab(
                    selected = state == index,
                    onClick = { state = index },
                    text = { Text(text = title, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                )
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Text tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun SecondaryIconTabs() {
    var state by remember { mutableStateOf(0) }
    val icons = listOf(Icons.Filled.Favorite, Icons.Filled.Favorite, Icons.Filled.Favorite)
    Column {
        SecondaryTabRow(selectedTabIndex = state) {
            icons.forEachIndexed { index, icon ->
                // Icon-only tab should have a tooltip associated with it.
                TooltipBox(
                    positionProvider =
                        TooltipDefaults.rememberTooltipPositionProvider(
                            TooltipAnchorPosition.Above
                        ),
                    tooltip = { PlainTooltip { Text("Favorite") } },
                    state = rememberTooltipState(),
                ) {
                    Tab(
                        selected = state == index,
                        onClick = { state = index },
                        icon = { Icon(icon, contentDescription = "Favorite") },
                    )
                }
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Icon tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun TextAndIconTabs() {
    var state by remember { mutableStateOf(0) }
    val titlesAndIcons =
        listOf(
            "Tab 1" to Icons.Filled.Favorite,
            "Tab 2" to Icons.Filled.Favorite,
            "Tab 3 with lots of text" to Icons.Filled.Favorite,
        )
    Column {
        PrimaryTabRow(selectedTabIndex = state) {
            titlesAndIcons.forEachIndexed { index, (title, icon) ->
                Tab(
                    selected = state == index,
                    onClick = { state = index },
                    text = { Text(text = title, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                    icon = { Icon(icon, contentDescription = null) },
                )
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Text and icon tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun LeadingIconTabs() {
    var state by remember { mutableStateOf(0) }
    val titlesAndIcons =
        listOf(
            "Tab" to Icons.Filled.Favorite,
            "Tab & icon" to Icons.Filled.Favorite,
            "Tab 3 with lots of text" to Icons.Filled.Favorite,
        )
    Column {
        PrimaryTabRow(selectedTabIndex = state) {
            titlesAndIcons.forEachIndexed { index, (title, icon) ->
                LeadingIconTab(
                    selected = state == index,
                    onClick = { state = index },
                    text = {
                        BadgedBox(badge = { Badge(modifier = Modifier) { Text("999+") } }) {
                            Text(title)
                        }
                    },
                    icon = { Icon(icon, contentDescription = null) },
                )
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Leading icon tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun ScrollingPrimaryTextTabs() {
    var state by remember { mutableStateOf(0) }
    val titles =
        listOf(
            "Tab 1",
            "Tab 2",
            "Tab 3 with lots of text",
            "Tab 4",
            "Tab 5",
            "Tab 6 with lots of text",
            "Tab 7",
            "Tab 8",
            "Tab 9 with lots of text",
            "Tab 10",
        )
    Column {
        PrimaryScrollableTabRow(selectedTabIndex = state) {
            titles.forEachIndexed { index, title ->
                Tab(selected = state == index, onClick = { state = index }, text = { Text(title) })
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Scrolling primary tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun ScrollingSecondaryTextTabs() {
    var state by remember { mutableStateOf(0) }
    val titles =
        listOf(
            "Tab 1",
            "Tab 2",
            "Tab 3 with lots of text",
            "Tab 4",
            "Tab 5",
            "Tab 6 with lots of text",
            "Tab 7",
            "Tab 8",
            "Tab 9 with lots of text",
            "Tab 10",
        )
    Column {
        SecondaryScrollableTabRow(selectedTabIndex = state) {
            titles.forEachIndexed { index, title ->
                Tab(selected = state == index, onClick = { state = index }, text = { Text(title) })
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Scrolling secondary tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun ScrollingTextTabs() {
    var state by remember { mutableStateOf(0) }
    val titles =
        listOf(
            "Tab 1",
            "Tab 2",
            "Tab 3 with lots of text",
            "Tab 4",
            "Tab 5",
            "Tab 6 with lots of text",
            "Tab 7",
            "Tab 8",
            "Tab 9 with lots of text",
            "Tab 10",
        )
    Column {
        PrimaryScrollableTabRow(selectedTabIndex = state) {
            titles.forEachIndexed { index, title ->
                Tab(selected = state == index, onClick = { state = index }, text = { Text(title) })
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Scrolling text tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun FancyTabs() {
    var state by remember { mutableStateOf(0) }
    val titles = listOf("Tab 1", "Tab 2", "Tab 3")
    Column {
        SecondaryTabRow(selectedTabIndex = state) {
            titles.forEachIndexed { index, title ->
                FancyTab(title = title, onClick = { state = index }, selected = (index == state))
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Fancy tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun FancyIndicatorTabs() {
    var state by remember { mutableStateOf(0) }
    val titles = listOf("Tab 1", "Tab 2", "Tab 3")

    Column {
        SecondaryTabRow(
            selectedTabIndex = state,
            indicator = {
                FancyIndicator(
                    MaterialTheme.colorScheme.primary,
                    Modifier.tabIndicatorOffset(state),
                )
            },
        ) {
            titles.forEachIndexed { index, title ->
                Tab(selected = state == index, onClick = { state = index }, text = { Text(title) })
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Fancy indicator tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

// (FancyIndicatorContainerTabs, FancyAnimatedIndicatorWithModifier and ScrollingFancyIndicatorContainerTabs are here
// as M15 writes them, in comments: the indicator keeps its Animatables in `mutableStateOf<Animatable<Dp, …>?>(null)`
// states, and KompozR's snapshot states hold Send values only (an Animatable isn't) — a state of a non-Send value
// for the Kotlin syntax is kompoz-runtime's to add. Their Rust twins are in kompoz-material3's tests/tabs.rs; a
// composable extension of TabIndicatorScope is tested in kotlin_lists_sheets.rs (IndicatorExtension).)
/*
@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun FancyIndicatorContainerTabs() {
    var state by remember { mutableStateOf(0) }
    val titles = listOf("Tab 1", "Tab 2", "Tab 3")

    Column {
        SecondaryTabRow(
            selectedTabIndex = state,
            indicator = { FancyAnimatedIndicatorWithModifier(state) },
        ) {
            titles.forEachIndexed { index, title ->
                Tab(selected = state == index, onClick = { state = index }, text = { Text(title) })
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Fancy transition tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}
*/

@Composable
fun FancyIndicator(color: Color, modifier: Modifier = Modifier) {
    // Draws a rounded rectangular with border around the Tab, with a 5.dp padding from the edges
    // Color is passed in as a parameter [color]
    Box(
        modifier
            .padding(5.dp)
            .fillMaxSize()
            .border(BorderStroke(2.dp, color), RoundedCornerShape(5.dp))
    )
}

/*
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TabIndicatorScope.FancyAnimatedIndicatorWithModifier(
    index: Int,
    isScrollable: Boolean = false,
) {
    val colors =
        listOf(
            MaterialTheme.colorScheme.primary,
            MaterialTheme.colorScheme.secondary,
            MaterialTheme.colorScheme.tertiary,
        )
    var startAnimatable by remember { mutableStateOf<Animatable<Dp, AnimationVector1D>?>(null) }
    var endAnimatable by remember { mutableStateOf<Animatable<Dp, AnimationVector1D>?>(null) }
    val coroutineScope = rememberCoroutineScope()
    val indicatorColor: Color by animateColorAsState(colors[index % colors.size], label = "")

    Box(
        Modifier.tabIndicatorLayout {
                measurable: Measurable,
                constraints: Constraints,
                tabPositions: List<TabPosition> ->
                val newStart = tabPositions[index].left
                val newEnd = tabPositions[index].right
                val startAnim =
                    startAnimatable
                        ?: Animatable(newStart, Dp.VectorConverter).also { startAnimatable = it }

                val endAnim =
                    endAnimatable
                        ?: Animatable(newEnd, Dp.VectorConverter).also { endAnimatable = it }

                if (endAnim.targetValue != newEnd) {
                    coroutineScope.launch {
                        endAnim.animateTo(
                            newEnd,
                            animationSpec =
                                if (endAnim.targetValue < newEnd) {
                                    spring(dampingRatio = 1f, stiffness = 1000f)
                                } else {
                                    spring(dampingRatio = 1f, stiffness = 50f)
                                },
                        )
                    }
                }

                if (startAnim.targetValue != newStart) {
                    coroutineScope.launch {
                        startAnim.animateTo(
                            newStart,
                            animationSpec =
                                // Handle directionality here, if we are moving to the right, we
                                // want the right side of the indicator to move faster, if we are
                                // moving to the left, we want the left side to move faster.
                                if (startAnim.targetValue < newStart) {
                                    spring(dampingRatio = 1f, stiffness = 50f)
                                } else {
                                    spring(dampingRatio = 1f, stiffness = 1000f)
                                },
                        )
                    }
                }

                val indicatorEnd = endAnim.value.roundToPx()
                val indicatorStart = startAnim.value.roundToPx()

                // Apply an offset from the start to correctly position the indicator around the tab
                val placeable =
                    measurable.measure(
                        constraints.copy(
                            maxWidth = indicatorEnd - indicatorStart,
                            minWidth = indicatorEnd - indicatorStart,
                        )
                    )
                layout(constraints.maxWidth, constraints.maxHeight) {
                    val contentWidth = tabPositions[index].contentWidth.roundToPx()
                    val tabWidth = tabPositions[index].width.roundToPx()
                    val relativeOffset = if (isScrollable) (tabWidth - contentWidth) / 2 else 0
                    placeable.place(indicatorStart - relativeOffset, 0)
                }
            }
            .padding(5.dp)
            .fillMaxSize()
            .drawWithContent {
                drawRoundRect(
                    color = indicatorColor,
                    cornerRadius = CornerRadius(5.dp.toPx()),
                    style = Stroke(width = 2.dp.toPx()),
                )
            }
    )
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun ScrollingFancyIndicatorContainerTabs() {
    var state by remember { mutableStateOf(0) }
    val titles =
        listOf(
            "Tab 1",
            "Tab 2",
            "Tab 3 with lots of text",
            "Tab 4",
            "Tab 5",
            "Tab 6 with lots of text",
            "Tab 7",
            "Tab 8",
            "Tab 9 with lots of text",
            "Tab 10",
        )

    Column {
        SecondaryScrollableTabRow(
            selectedTabIndex = state,
            indicator = { FancyAnimatedIndicatorWithModifier(state, isScrollable = true) },
        ) {
            titles.forEachIndexed { index, title ->
                Tab(selected = state == index, onClick = { state = index }, text = { Text(title) })
            }
        }
        Text(
            modifier = Modifier.align(Alignment.CenterHorizontally),
            text = "Scrolling fancy transition tab ${state + 1} selected",
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}
*/

@Composable
fun FancyTab(title: String, onClick: () -> Unit, selected: Boolean) {
    Tab(selected, onClick) {
        Column(
            Modifier.padding(10.dp).height(50.dp).fillMaxWidth(),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            Box(
                Modifier.size(10.dp)
                    .align(Alignment.CenterHorizontally)
                    .background(
                        color =
                            if (selected) MaterialTheme.colorScheme.primary
                            else MaterialTheme.colorScheme.background
                    )
            )
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.align(Alignment.CenterHorizontally),
            )
        }
    }
}

// ---- AlertDialogSamples.kt ----

@Composable
fun AlertDialogSample() {
    val openDialog = remember { mutableStateOf(true) }

    Button(onClick = { openDialog.value = true }) { Text("Open dialog") }

    if (openDialog.value) {
        AlertDialog(
            onDismissRequest = {
                // Dismiss the dialog when the user clicks outside the dialog or on the back
                // button. If you want to disable that functionality, simply use an empty
                // onDismissRequest.
                openDialog.value = false
            },
            title = { Text(text = "Title") },
            text = { Text(text = "Turned on by default") },
            confirmButton = {
                TextButton(onClick = { openDialog.value = false }) { Text("Confirm") }
            },
            dismissButton = {
                TextButton(onClick = { openDialog.value = false }) { Text("Dismiss") }
            },
        )
    }
}

@Composable
fun AlertDialogWithIconSample() {
    val openDialog = remember { mutableStateOf(true) }

    Button(onClick = { openDialog.value = true }) { Text("Open dialog") }

    if (openDialog.value) {
        AlertDialog(
            onDismissRequest = {
                // Dismiss the dialog when the user clicks outside the dialog or on the back
                // button. If you want to disable that functionality, simply use an empty
                // onDismissRequest.
                openDialog.value = false
            },
            icon = {
                Icon(
                    Icons.Filled.Favorite,
                    contentDescription = null,
                    modifier = Modifier.size(AlertDialogDefaults.IconSize),
                )
            },
            title = { Text(text = "Title") },
            text = {
                Text(
                    "This area typically contains the supportive text " +
                        "which presents the details regarding the Dialog's purpose."
                )
            },
            confirmButton = {
                TextButton(onClick = { openDialog.value = false }) { Text("Confirm") }
            },
            dismissButton = {
                TextButton(onClick = { openDialog.value = false }) { Text("Dismiss") }
            },
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BasicAlertDialogSample() {
    val openDialog = remember { mutableStateOf(true) }

    Button(onClick = { openDialog.value = true }) { Text("Open dialog") }

    if (openDialog.value) {
        BasicAlertDialog(
            onDismissRequest = {
                // Dismiss the dialog when the user clicks outside the dialog or on the back
                // button. If you want to disable that functionality, simply use an empty
                // onDismissRequest.
                openDialog.value = false
            }
        ) {
            Surface(
                modifier = Modifier.wrapContentWidth().wrapContentHeight(),
                shape = MaterialTheme.shapes.large,
                tonalElevation = AlertDialogDefaults.TonalElevation,
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text(
                        text =
                            "This area typically contains the supportive text " +
                                "which presents the details regarding the Dialog's purpose."
                    )
                    Spacer(modifier = Modifier.height(24.dp))
                    TextButton(
                        onClick = { openDialog.value = false },
                        modifier = Modifier.align(Alignment.End),
                    ) {
                        Text("Confirm")
                    }
                }
            }
        }
    }
}

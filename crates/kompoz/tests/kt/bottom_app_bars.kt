// Material's bottom app bars as Compose's samples write them: material3 1.5's AppBarSamples.kt bottom-bar samples
// (SimpleBottomAppBar to BottomAppBarWithOverflow), as written (their @Sampled/@Preview annotations and KDoc left out).

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SimpleBottomAppBar() {
    BottomAppBar(
        actions = {
            TooltipBox(
                positionProvider =
                    TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
                tooltip = { PlainTooltip { Text("Menu") } },
                state = rememberTooltipState(),
            ) {
                IconButton(onClick = { /* doSomething() */ }) {
                    Icon(Icons.Filled.Menu, contentDescription = "Menu button")
                }
            }
        }
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BottomAppBarWithFAB() {
    BottomAppBar(
        actions = {
            TooltipBox(
                positionProvider =
                    TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
                tooltip = { PlainTooltip { Text("Check") } },
                state = rememberTooltipState(),
            ) {
                IconButton(onClick = { /* doSomething() */ }) {
                    Icon(Icons.Filled.Check, contentDescription = "Check")
                }
            }
            TooltipBox(
                positionProvider =
                    TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
                tooltip = { PlainTooltip { Text("Edit") } },
                state = rememberTooltipState(),
            ) {
                IconButton(onClick = { /* doSomething() */ }) {
                    Icon(Icons.Filled.Edit, contentDescription = "Edit")
                }
            }
        },
        floatingActionButton = {
            TooltipBox(
                positionProvider =
                    TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
                tooltip = { PlainTooltip { Text("Add") } },
                state = rememberTooltipState(),
            ) {
                FloatingActionButton(
                    onClick = { /* do something */ },
                    containerColor = BottomAppBarDefaults.bottomAppBarFabColor,
                    elevation = FloatingActionButtonDefaults.bottomAppBarFabElevation(),
                ) {
                    Icon(Icons.Filled.Add, "Add")
                }
            }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ExitAlwaysBottomAppBar() {
    val scrollBehavior = BottomAppBarDefaults.exitAlwaysScrollBehavior()
    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        bottomBar = {
            BottomAppBar(
                actions = {
                    TooltipBox(
                        positionProvider =
                            TooltipDefaults.rememberTooltipPositionProvider(
                                TooltipAnchorPosition.Above
                            ),
                        tooltip = { PlainTooltip { Text("Check") } },
                        state = rememberTooltipState(),
                    ) {
                        IconButton(onClick = { /* doSomething() */ }) {
                            Icon(Icons.Filled.Check, contentDescription = "Check")
                        }
                    }
                    TooltipBox(
                        positionProvider =
                            TooltipDefaults.rememberTooltipPositionProvider(
                                TooltipAnchorPosition.Above
                            ),
                        tooltip = { PlainTooltip { Text("Edit") } },
                        state = rememberTooltipState(),
                    ) {
                        IconButton(onClick = { /* doSomething() */ }) {
                            Icon(Icons.Filled.Edit, contentDescription = "Edit")
                        }
                    }
                },
                scrollBehavior = scrollBehavior,
            )
        },
        floatingActionButton = {
            TooltipBox(
                positionProvider =
                    TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
                tooltip = { PlainTooltip { Text("Add") } },
                state = rememberTooltipState(),
            ) {
                FloatingActionButton(
                    modifier = Modifier.offset(y = 4.dp),
                    onClick = { /* do something */ },
                    containerColor = BottomAppBarDefaults.bottomAppBarFabColor,
                    elevation = FloatingActionButtonDefaults.bottomAppBarFabElevation(),
                ) {
                    Icon(Icons.Filled.Add, "Add")
                }
            }
        },
        floatingActionButtonPosition = FabPosition.EndOverlay,
        content = { innerPadding ->
            LazyColumn(
                contentPadding = innerPadding,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val list = (0..75).map { it.toString() }
                items(count = list.size) {
                    Text(
                        text = list[it],
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                    )
                }
            }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ExitAlwaysBottomAppBarSpacedAround() {
    val scrollBehavior = BottomAppBarDefaults.exitAlwaysScrollBehavior()
    val icons =
        listOf(
            Icons.AutoMirrored.Filled.ArrowBack,
            Icons.AutoMirrored.Filled.ArrowForward,
            Icons.Filled.Add,
            Icons.Filled.Check,
            Icons.Filled.Edit,
        )
    val buttons = listOf("Back", "Forward", "Add", "Check", "Edit")

    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        bottomBar = {
            FlexibleBottomAppBar(
                horizontalArrangement = Arrangement.SpaceAround,
                contentPadding = PaddingValues(horizontal = 0.dp),
                scrollBehavior = scrollBehavior,
                content = {
                    buttons.forEachIndexed { index, button ->
                        TooltipBox(
                            positionProvider =
                                TooltipDefaults.rememberTooltipPositionProvider(
                                    TooltipAnchorPosition.Above
                                ),
                            tooltip = { PlainTooltip { Text(button) } },
                            state = rememberTooltipState(),
                        ) {
                            if (index == 2) {
                                FilledIconButton(
                                    modifier = Modifier.width(56.dp),
                                    onClick = { /* doSomething() */ },
                                ) {
                                    Icon(icons[index], contentDescription = button)
                                }
                            } else {
                                IconButton(onClick = { /* doSomething() */ }) {
                                    Icon(icons[index], contentDescription = button)
                                }
                            }
                        }
                    }
                },
            )
        },
        content = { innerPadding ->
            LazyColumn(
                contentPadding = innerPadding,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val list = (0..75).map { it.toString() }
                items(count = list.size) {
                    Text(
                        text = list[it],
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                    )
                }
            }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ExitAlwaysBottomAppBarSpacedBetween() {
    val scrollBehavior = BottomAppBarDefaults.exitAlwaysScrollBehavior()
    val icons =
        listOf(
            Icons.AutoMirrored.Filled.ArrowBack,
            Icons.AutoMirrored.Filled.ArrowForward,
            Icons.Filled.Add,
            Icons.Filled.Check,
            Icons.Filled.Edit,
        )
    val buttons = listOf("Back", "Forward", "Add", "Check", "Edit")
    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        bottomBar = {
            FlexibleBottomAppBar(
                horizontalArrangement = Arrangement.SpaceBetween,
                scrollBehavior = scrollBehavior,
                content = {
                    buttons.forEachIndexed { index, button ->
                        TooltipBox(
                            positionProvider =
                                TooltipDefaults.rememberTooltipPositionProvider(
                                    TooltipAnchorPosition.Above
                                ),
                            tooltip = { PlainTooltip { Text(button) } },
                            state = rememberTooltipState(),
                        ) {
                            if (index == 2) {
                                FilledIconButton(
                                    modifier = Modifier.width(56.dp),
                                    onClick = { /* doSomething() */ },
                                ) {
                                    Icon(icons[index], contentDescription = button)
                                }
                            } else {
                                IconButton(onClick = { /* doSomething() */ }) {
                                    Icon(icons[index], contentDescription = button)
                                }
                            }
                        }
                    }
                },
            )
        },
        content = { innerPadding ->
            LazyColumn(
                contentPadding = innerPadding,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val list = (0..75).map { it.toString() }
                items(count = list.size) {
                    Text(
                        text = list[it],
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                    )
                }
            }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ExitAlwaysBottomAppBarSpacedEvenly() {
    val scrollBehavior = BottomAppBarDefaults.exitAlwaysScrollBehavior()
    val icons =
        listOf(
            Icons.AutoMirrored.Filled.ArrowBack,
            Icons.AutoMirrored.Filled.ArrowForward,
            Icons.Filled.Add,
            Icons.Filled.Check,
            Icons.Filled.Edit,
        )
    val buttons = listOf("Back", "Forward", "Add", "Check", "Edit")
    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        bottomBar = {
            FlexibleBottomAppBar(
                horizontalArrangement = Arrangement.SpaceEvenly,
                contentPadding = PaddingValues(horizontal = 0.dp),
                scrollBehavior = scrollBehavior,
                content = {
                    buttons.forEachIndexed { index, button ->
                        TooltipBox(
                            positionProvider =
                                TooltipDefaults.rememberTooltipPositionProvider(
                                    TooltipAnchorPosition.Above
                                ),
                            tooltip = { PlainTooltip { Text(button) } },
                            state = rememberTooltipState(),
                        ) {
                            if (index == 2) {
                                FilledIconButton(
                                    modifier = Modifier.width(56.dp),
                                    onClick = { /* doSomething() */ },
                                ) {
                                    Icon(icons[index], contentDescription = button)
                                }
                            } else {
                                IconButton(onClick = { /* doSomething() */ }) {
                                    Icon(icons[index], contentDescription = button)
                                }
                            }
                        }
                    }
                },
            )
        },
        content = { innerPadding ->
            LazyColumn(
                contentPadding = innerPadding,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val list = (0..75).map { it.toString() }
                items(count = list.size) {
                    Text(
                        text = list[it],
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                    )
                }
            }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ExitAlwaysBottomAppBarFixed() {
    val scrollBehavior = BottomAppBarDefaults.exitAlwaysScrollBehavior()
    val icons =
        listOf(
            Icons.AutoMirrored.Filled.ArrowBack,
            Icons.AutoMirrored.Filled.ArrowForward,
            Icons.Filled.Add,
            Icons.Filled.Check,
            Icons.Filled.Edit,
        )
    val buttons = listOf("Back", "Forward", "Add", "Check", "Edit")
    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        bottomBar = {
            FlexibleBottomAppBar(
                horizontalArrangement = BottomAppBarDefaults.FlexibleFixedHorizontalArrangement,
                scrollBehavior = scrollBehavior,
                content = {
                    buttons.forEachIndexed { index, button ->
                        TooltipBox(
                            positionProvider =
                                TooltipDefaults.rememberTooltipPositionProvider(
                                    TooltipAnchorPosition.Above
                                ),
                            tooltip = { PlainTooltip { Text(button) } },
                            state = rememberTooltipState(),
                        ) {
                            if (index == 2) {
                                FilledIconButton(
                                    modifier = Modifier.width(56.dp),
                                    onClick = { /* doSomething() */ },
                                ) {
                                    Icon(icons[index], contentDescription = button)
                                }
                            } else {
                                IconButton(onClick = { /* doSomething() */ }) {
                                    Icon(icons[index], contentDescription = button)
                                }
                            }
                        }
                    }
                },
            )
        },
        content = { innerPadding ->
            LazyColumn(
                contentPadding = innerPadding,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val list = (0..75).map { it.toString() }
                items(count = list.size) {
                    Text(
                        text = list[it],
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                    )
                }
            }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ExitAlwaysBottomAppBarFixedVibrant() {
    val scrollBehavior = BottomAppBarDefaults.exitAlwaysScrollBehavior()
    val icons =
        listOf(
            Icons.AutoMirrored.Filled.ArrowBack,
            Icons.AutoMirrored.Filled.ArrowForward,
            Icons.Filled.Add,
            Icons.Filled.Check,
            Icons.Filled.Edit,
        )
    val buttons = listOf("Back", "Forward", "Add", "Check", "Edit")
    Scaffold(
        modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
        bottomBar = {
            FlexibleBottomAppBar(
                horizontalArrangement = BottomAppBarDefaults.FlexibleFixedHorizontalArrangement,
                scrollBehavior = scrollBehavior,
                containerColor =
                    MaterialTheme.colorScheme.primaryContainer, // TODO(b/356885344): tokens
                content = {
                    buttons.forEachIndexed { index, button ->
                        TooltipBox(
                            positionProvider =
                                TooltipDefaults.rememberTooltipPositionProvider(
                                    TooltipAnchorPosition.Above
                                ),
                            tooltip = { PlainTooltip { Text(button) } },
                            state = rememberTooltipState(),
                        ) {
                            if (index == 2) {
                                FilledIconButton(
                                    modifier = Modifier.width(56.dp),
                                    onClick = { /* doSomething() */ },
                                ) {
                                    Icon(icons[index], contentDescription = button)
                                }
                            } else {
                                IconButton(onClick = { /* doSomething() */ }) {
                                    Icon(icons[index], contentDescription = button)
                                }
                            }
                        }
                    }
                },
            )
        },
        content = { innerPadding ->
            LazyColumn(
                contentPadding = innerPadding,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val list = (0..75).map { it.toString() }
                items(count = list.size) {
                    Text(
                        text = list[it],
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                    )
                }
            }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun BottomAppBarWithOverflow() {
    val icons =
        listOf(
            Icons.AutoMirrored.Filled.ArrowBack,
            Icons.AutoMirrored.Filled.ArrowForward,
            Icons.Filled.Add,
            Icons.Filled.Check,
            Icons.Filled.Edit,
            Icons.Filled.Favorite,
        )
    val items = listOf("Back", "Forward", "Add", "Check", "Edit", "Favorite")
    FlexibleBottomAppBar(
        contentPadding = PaddingValues(horizontal = 96.dp),
        horizontalArrangement = BottomAppBarDefaults.FlexibleFixedHorizontalArrangement,
    ) {
        AppBarRow(
            overflowIndicator = { menuState ->
                TooltipBox(
                    positionProvider =
                        TooltipDefaults.rememberTooltipPositionProvider(
                            TooltipAnchorPosition.Above
                        ),
                    tooltip = { PlainTooltip { Text("Overflow") } },
                    state = rememberTooltipState(),
                ) {
                    IconButton(
                        onClick = {
                            if (menuState.isShowing) {
                                menuState.dismiss()
                            } else {
                                menuState.show()
                            }
                        }
                    ) {
                        Icon(imageVector = Icons.Filled.MoreVert, contentDescription = "Overflow")
                    }
                }
            }
        ) {
            items.forEachIndexed { index, item ->
                clickableItem(
                    onClick = { /* doSomething() */ },
                    icon = { Icon(icons[index], contentDescription = item) },
                    label = item,
                )
            }
        }
    }
}

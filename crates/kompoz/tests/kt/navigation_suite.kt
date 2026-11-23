// Material's navigation suite as Compose's samples write it (material3-adaptive-navigation-suite 1.5's
// NavigationSuiteScaffoldSamples.kt): the scaffold of the window's navigation suite type, hidden and shown by its
// state, and the scaffold of a custom configuration of types.

@file:Suppress("DEPRECATION") // Suppress for WindowWidthSizeClass

@Preview
@Sampled
@Composable
fun NavigationSuiteScaffoldSample() {
    var selectedItem by remember { mutableIntStateOf(0) }
    val navItems = listOf("Songs", "Artists", "Playlists")
    val navSuiteType =
        NavigationSuiteScaffoldDefaults.navigationSuiteType(currentWindowAdaptiveInfoV2())
    val state = rememberNavigationSuiteScaffoldState()
    val scope = rememberCoroutineScope()

    NavigationSuiteScaffold(
        state = state,
        navigationItems = {
            navItems.forEachIndexed { index, navItem ->
                NavigationSuiteItem(
                    icon = {
                        Icon(
                            if (selectedItem == index) Icons.Filled.Favorite
                            else Icons.Outlined.FavoriteBorder,
                            contentDescription = null,
                        )
                    },
                    label = { Text(navItem) },
                    selected = selectedItem == index,
                    onClick = { selectedItem = index },
                )
            }
        },
    ) {
        // Screen content.
        Column(
            modifier = Modifier.fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                modifier = Modifier.padding(16.dp),
                text =
                    "Current NavigationSuiteType: $navSuiteType\n" +
                        "Visibility: ${state.currentValue}",
                textAlign = TextAlign.Center,
            )
            Button(onClick = { scope.launch { state.toggle() } }) {
                Text("Hide/show navigation component")
            }
        }
    }
}

@Preview
@Sampled
@Composable
@Suppress("DEPRECATION") // WindowWidthSizeClass is deprecated
fun NavigationSuiteScaffoldCustomConfigSample() {
    var selectedItem by remember { mutableIntStateOf(0) }
    val navItems = listOf("Songs", "Artists", "Playlists")
    // Custom configuration that shows a wide navigation rail in small/medium width screens, an
    // expanded wide navigation rail in expanded width screens, and a short navigation bar in small
    // height screens.
    val navSuiteType =
        with(currentWindowAdaptiveInfoV2()) {
            if (
                windowSizeClass.windowWidthSizeClass == WindowWidthSizeClass.COMPACT ||
                    windowSizeClass.windowWidthSizeClass == WindowWidthSizeClass.MEDIUM
            ) {
                NavigationSuiteType.WideNavigationRailCollapsed
            } else if (windowSizeClass.windowHeightSizeClass == WindowHeightSizeClass.COMPACT) {
                NavigationSuiteType.ShortNavigationBarMedium
            } else if (windowSizeClass.windowWidthSizeClass == WindowWidthSizeClass.EXPANDED) {
                NavigationSuiteType.WideNavigationRailExpanded
            } else {
                NavigationSuiteScaffoldDefaults.navigationSuiteType(currentWindowAdaptiveInfoV2())
            }
        }
    val state = rememberNavigationSuiteScaffoldState()
    val scope = rememberCoroutineScope()

    NavigationSuiteScaffold(
        navigationSuiteType = navSuiteType,
        state = state,
        navigationItemVerticalArrangement = Arrangement.Center,
        navigationItems = {
            navItems.forEachIndexed { index, navItem ->
                NavigationSuiteItem(
                    navigationSuiteType = navSuiteType,
                    icon = {
                        Icon(
                            if (selectedItem == index) Icons.Filled.Favorite
                            else Icons.Outlined.FavoriteBorder,
                            contentDescription = null,
                        )
                    },
                    label = { Text(navItem) },
                    selected = selectedItem == index,
                    onClick = { selectedItem = index },
                )
            }
        },
    ) {
        // Screen content.
        Column(
            modifier = Modifier.fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                modifier = Modifier.padding(16.dp),
                text =
                    "Current NavigationSuiteType: $navSuiteType\n" +
                        "Visibility: ${state.currentValue}",
                textAlign = TextAlign.Center,
            )
            Button(onClick = { scope.launch { state.toggle() } }) {
                Text("Hide/show navigation component")
            }
        }
    }
}

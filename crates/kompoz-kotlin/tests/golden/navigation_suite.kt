// The navigation suite (WP8): a NavigationSuiteScope receiver's `item(…)` inside `navigationSuiteItems = { }`, the
// scaffold and layout overloads chosen by their parameters' names, a parameter named `type` (Rust's `r#type`), and a
// WindowAdaptiveInfo's properties read through `with(info) { }`.

@Composable
fun Suite(tab: Int, onTab: (Int) -> Unit) {
    NavigationSuiteScaffold(
        navigationSuiteItems = {
            item(selected = tab == 0, onClick = { onTab(0) }, icon = { Icon(Icons.Filled.Home, contentDescription = null) }, label = { Text("Home") })
            item(selected = tab == 1, onClick = { onTab(1) }, icon = { Icon(Icons.Filled.Settings, contentDescription = null) })
        },
        layoutType = NavigationSuiteType.NavigationBar,
    ) {
        Text("Tab $tab")
    }
}

@Composable
fun Items(type: NavigationSuiteType, onClick: () -> Unit) {
    NavigationSuite(type, Modifier.testTag("suite"), verticalArrangement = Arrangement.Bottom) {
        NavigationSuiteItem(true, onClick, { Icon(Icons.Filled.Home, "") }, { Text("Ev") }, navigationSuiteType = type)
    }
}

@Composable
fun Bodies(which: Int) {
    val body = @Composable { Box(Modifier.size(60.dp, 40.dp)) }
    val bar = @Composable { Box(Modifier.size(80.dp, 80.dp)) }
    when (which) {
        2 -> NavigationSuiteScaffoldLayout(navigationSuite = bar, navigationSuiteType = NavigationSuiteType.NavigationBar, content = body)
        else -> NavigationSuiteScaffoldLayout(navigationSuite = bar, layoutType = NavigationSuiteType.None, content = body)
    }
}

@Composable
fun Suited(): NavigationSuiteType =
    with(currentWindowAdaptiveInfoV2()) {
        if (windowPosture.isTabletop || windowSizeClass.minWidthDp < 600) NavigationSuiteType.ShortNavigationBarMedium
        else NavigationSuiteScaffoldDefaults.navigationSuiteType(currentWindowAdaptiveInfoV2())
    }

@Composable
fun OwnItems(tab: Int, onTab: (Int) -> Unit, state: NavigationSuiteScaffoldState) {
    NavigationSuiteScaffold(
        navigationItems = {
            NavigationSuiteItem(tab == 0, { onTab(0) }, { Icon(Icons.Filled.Home, "") }, { Text("Ev") })
            NavigationSuiteItem(tab == 1, { onTab(1) }, { Icon(Icons.Filled.Settings, "") }, { Text("Ayarlar") },
                                badge = { Badge { Text("3") } })
        },
        state = state,
        primaryActionContent = { Box(Modifier.size(56.dp)) },
    ) {
        Box(Modifier.fillMaxSize())
    }
}

// KompozR's showcase: the C++ Kompoz showcase's shell (examples/showcase/main.cpp) — the drawer, the navigation
// suite, the app bar that scrolls away, the snackbars, the about dialog — around its screens. Turn on the eye in the
// app bar to see which parts actually run again.
//
// The screens are routes of a NavHost (Compose's typed routes, the C++ showcase's `routes::` namespace: Kotlin's
// top-level @Serializable objects, lib.rs's `routes`); of them the Material and places screens are here, the home,
// list (with its item screen, its LikesViewModel and its scroll-to-top button) and components screens come with
// stage 16 (their routes show LaterScreen). The C++ showcase's desktop menu bar (MenuBar: Dosya, Görünüm, Git) comes
// with the window APIs (stage 15/16); its SystemBarsAppearance (the status bar icons on Android) with stage 15.

@Composable
fun ShowcaseApp() {
    val system = isSystemInDarkTheme()
    var dark by remember { mutableStateOf(system) }
    // The system's, as it changes, until the switch is used: then the user's.
    var darkChosen by remember { mutableStateOf(false) }
    LaunchedEffect(system) {
        if (!darkChosen) dark = system
    }
    val colorSource = rememberSaveable { mutableStateOf(ColorSource.System) }  // Material You where the system has it
    var showRekompozitions by remember { mutableStateOf(false) }
    // The screens are routes of a NavHost; the tabs keep their own back stacks (and scroll
    // positions), saved while another tab is shown, as Compose's bottom navigation does.
    val nav = rememberNavController()
    val entry by nav.currentBackStackEntryAsState()
    val destination = entry?.destination
    val route = destination?.route ?: "home"
    val tab = when {
        destination?.hierarchy?.any { it.hasRoute<ListGraph>() } == true -> 1
        destination?.hierarchy?.any { it.hasRoute<Components>() } == true -> 2
        destination?.hierarchy?.any { it.hasRoute<Material>() } == true -> 3
        else -> 0
    }
    val tabs = navOptions {
        popUpTo<Home> { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
    val goTab: (Int) -> Unit = { i ->
        when (i) {
            0 -> nav.navigate(Home, tabs)
            1 -> nav.navigate(ListGraph, tabs)
            2 -> nav.navigate(Components, tabs)
            else -> nav.navigate(Material, tabs)
        }
    }
    val snackbar = remember { SnackbarHostState() }
    var overflow by remember { mutableStateOf(false) }
    // The whole app laid out right to left: the system language's direction at first, then the menu's.
    val systemDirection = LocalLayoutDirection.current
    var rightToLeft by remember { mutableStateOf(systemDirection == LayoutDirection.Rtl) }
    var about by remember { mutableStateOf(false) }
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    // The app bar scrolls away with the content and comes back when it scrolls the other way.
    val barScroll = TopAppBarDefaults.enterAlwaysScrollBehavior()
    // A new screen starts with the bar shown; the screen it was on keeps it as it was (after the app was ended
    // too: the screen seen last is saved).
    var seen by rememberSaveable { mutableStateOf(route) }
    LaunchedEffect(route) {
        if (seen != route) {
            barScroll.state.heightOffset = 0f
            barScroll.state.contentOffset = 0f
            seen = route
        }
    }
    val destinations = listOf("Ana sayfa" to Icons.Filled.Home, "Liste" to Icons.AutoMirrored.Filled.List,
                              "Bileşenler" to Icons.Filled.Star, "Material" to Icons.Filled.CheckCircle)
    val closeDrawer: () -> Unit = { scope.launch { drawer.close() } }
    var notified by remember { mutableStateOf(0) }

    val drawerContent: @Composable () -> Unit = {
        ModalDrawerSheet {
            Text("Kompoz", modifier = Modifier.padding(16.dp, 8.dp, 16.dp, 16.dp), style = MaterialTheme.typography.titleLarge)
            for (i in 0 until 4) {
                val (label, icon) = destinations[i]
                NavigationDrawerItem(
                    label = { Text(label) },
                    selected = tab == i,
                    onClick = {
                        goTab(i)
                        closeDrawer()
                    },
                    icon = { Icon(icon, contentDescription = label) },
                    badge = if (i == 3) { { Badge { Text("Yeni") } } } else null)
            }
            HorizontalDivider(modifier = Modifier.padding(16.dp, 8.dp))
            NavigationDrawerItem(
                label = { Text("Hakkında") },
                selected = false,
                onClick = {
                    about = true
                    closeDrawer()
                },
                icon = { Icon(Icons.Filled.Info, contentDescription = "Hakkında") })
        }
    }

    val topBar: @Composable () -> Unit = {
        TopAppBar(
            title = { Text("Kompoz") },
            navigationIcon = {
                IconButton(onClick = { scope.launch { drawer.open() } }) { Icon(Icons.Filled.Menu, contentDescription = "Menü") }
            },
            actions = {
                IconButton(onClick = {
                    dark = !dark
                    darkChosen = true
                }) { Icon(Icons.Filled.DarkMode, contentDescription = "Koyu tema") }
                IconButton(onClick = {
                    showRekompozitions = !showRekompozitions
                    scope.launch {
                        snackbar.showSnackbar(if (showRekompozitions) "Recomposition vurgusu açık" else "Recomposition vurgusu kapalı")
                    }
                }) {
                    Icon(Icons.Filled.Visibility, contentDescription = "Recomposition'ları göster",
                         tint = if (showRekompozitions) MaterialTheme.colorScheme.primary else LocalContentColor.current)
                }
                Box {  // the menu opens below its anchor, this Box
                    IconButton(onClick = { overflow = true }) { Icon(Icons.Filled.MoreVert, contentDescription = "Diğer seçenekler") }
                    DropdownMenu(expanded = overflow, onDismissRequest = { overflow = false }) {
                        DropdownMenuItem(
                            text = { Text("Hakkında") },
                            onClick = {
                                overflow = false
                                about = true
                            },
                            leadingIcon = { Icon(Icons.Filled.Info, contentDescription = "Hakkında") })
                        DropdownMenuItem(
                            text = { Text("Material bileşenleri") },
                            onClick = {
                                overflow = false
                                goTab(3)
                            },
                            leadingIcon = { Icon(Icons.Filled.CheckCircle, contentDescription = "Material") })
                        // LocalLayoutDirection: rows from the right, Start is the right, horizontal lists start there.
                        DropdownMenuItem(
                            text = { Text(if (rightToLeft) "Soldan sağa (LTR)" else "Sağdan sola (RTL)") },
                            onClick = {
                                overflow = false
                                rightToLeft = !rightToLeft
                            },
                            leadingIcon = { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Yön") })
                    }
                }
            },
            scrollBehavior = barScroll)
    }

    // (The list screen's scroll-to-top button, the FAB of ListAll, comes with that screen: stage 16.)
    val fab: @Composable () -> Unit = {
        if (destination?.hasRoute<Home>() == true) {
            ExtendedFloatingActionButton(
                onClick = {
                    // As Compose apps do: wait for how it went (with an action it stays until answered or closed).
                    // Every other one has a long action, which goes on a line of its own (see snackbarHost).
                    val longAction = notified++ % 2 == 1
                    scope.launch {
                        val result = snackbar.showSnackbar(
                            if (longAction) "Bildirimler kapalı; önemli haberleri kaçırabilirsiniz." else "Merhaba! Bu bir Snackbar.",
                            actionLabel = if (longAction) "Bildirimleri aç" else "Geri al",
                            withDismissAction = true)
                        if (result == SnackbarResult.ActionPerformed) snackbar.showSnackbar(if (longAction) "Bildirimler açıldı" else "Geri alındı")
                    }
                },
                icon = { Icon(Icons.Filled.Notifications, contentDescription = null) },
                text = { Text("Bildir") })
        }
    }

    val screen: @Composable (PaddingValues) -> Unit = { padding ->
        // Clear of the bars; the keyboard only takes what the bottom bar doesn't already cover.
        NavHost(
            navController = nav,
            startDestination = Home,
            modifier = Modifier.fillMaxSize().padding(padding).consumeWindowInsets(padding).imePadding(),
            enterTransition = { fadeIn(animationSpec = tween(220, delayMillis = 60)) },
            exitTransition = { fadeOut(animationSpec = tween(120)) },
        ) {
            composable<Home> { LaterScreen(destinations[0].first) }
            // The list and its items: a nested graph (its LikesViewModel comes with its screens).
            navigation<ListGraph>(startDestination = ListAll) {
                composable<ListAll> { LaterScreen(destinations[1].first) }
                // Also opened by a link: `showcase kompoz://item/42`, or an Android intent.
                composable<Item>(deepLinks = listOf(navDeepLink<Item>("kompoz://item"))) { e ->
                    LaterScreen("Öğe ${e.toRoute<Item>().n}")
                }
            }
            composable<Components>(deepLinks = listOf(navDeepLink { uriPattern = "kompoz://components" })) {
                LaterScreen(destinations[2].first)
            }
            composable<Material>(deepLinks = listOf(navDeepLink { uriPattern = "kompoz://material" })) {
                MaterialScreen(Modifier.fillMaxSize(), colorSource, nav)
            }
            composable<Places>(deepLinks = listOf(navDeepLink { uriPattern = "kompoz://places" })) {
                PlacesScreen(Modifier.fillMaxSize())
            }
        }
        if (about) {
            AlertDialog(
                onDismissRequest = { about = false },
                confirmButton = { TextButton(onClick = { about = false }) { Text("Tamam") } },
                icon = { Icon(Icons.Filled.Info, contentDescription = "Bilgi") },
                title = { Text("Kompoz") },
                text = {
                    Text("C++20 ile Jetpack Compose tarzı, çok platformlu bir arayüz kütüphanesi: Linux, Windows, Android " +
                             "ve web. Apache 2.0 lisanslı.\n\n" +
                             "İçindekiler: Noto Sans fontları (SIL OFL 1.1), FreeType (portions of this software are " +
                             "copyright © 2026 The FreeType Project, www.freetype.org), HarfBuzz (MIT), libpng ve zlib. " +
                             "Lisans metinleri uygulamanın licenses klasöründe.")
                })
        }
    }

    // The colors from where the user chose, the app's own where that has none.
    val scheme = when {
        colorSource.value == ColorSource.System && isDynamicColorAvailable() -> if (dark) dynamicDarkColorScheme() else dynamicLightColorScheme()
        colorSource.value == ColorSource.Wallpaper && isWallpaperColorAvailable() -> if (dark) wallpaperDarkColorScheme() else wallpaperLightColorScheme()
        else -> if (dark) darkColorScheme() else lightColorScheme()
    }
    MaterialTheme(colorScheme = animateColorScheme(scheme)) {
        // The showcase speaks Turkish, and so do the library's own texts in it (LocalStrings).
        CompositionLocalProvider(
            LocalStrings provides Strings.turkish(),
            LocalShowRekompozitions provides showRekompozitions,
            LocalLayoutDirection provides (if (rightToLeft) LayoutDirection.Rtl else LayoutDirection.Ltr),
        ) {
            ModalNavigationDrawer(drawerContent = drawerContent, drawerState = drawer) {
                // The tabs where they suit the window (currentWindowAdaptiveInfo), as M3 Expressive's: a short bar
                // on a phone (its labels beside the icons when it lies on its side), a wide rail on a wider window.
                NavigationSuiteScaffold(
                    navigationItems = {
                        for (i in 0 until 4) {
                            val (label, icon) = destinations[i]
                            NavigationSuiteItem(selected = tab == i, onClick = { goTab(i) }, icon = { Icon(icon, contentDescription = null) },
                                                label = { Text(label) })
                        }
                    },
                    modifier = Modifier.fillMaxSize(),
                ) {
                    Scaffold(
                        modifier = Modifier.fillMaxSize().nestedScroll(barScroll.nestedScrollConnection),
                        topBar = topBar,
                        floatingActionButton = fab,
                        // A snackbar of the app's own (here the standard one): a long action under the text.
                        snackbarHost = {
                            SnackbarHost(snackbar) { data ->
                                Snackbar(data, actionOnNewLine = (data.visuals.actionLabel?.length ?: 0) > 12)
                            }
                        },
                    ) { padding -> screen(padding) }
                }
            }
        }
    }
}

// The screens the C++ showcase has besides Material and places (home, list and its items, components) come with
// stage 16: until then their routes say so.
@Composable
fun LaterScreen(name: String) {
    Box(modifier = Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) {
        Text("$name ekranı KompozR'da 16. aşamada gelir; Material sekmesi hazır.", color = MaterialTheme.colorScheme.onSurfaceVariant,
             textAlign = TextAlign.Center, style = MaterialTheme.typography.bodyLarge)
    }
}

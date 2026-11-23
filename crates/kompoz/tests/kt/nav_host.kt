// The NavHost as Compose writes it (navigation-compose 2.10): the C++ Kompoz's Navigator
// (kompozable_tests.cpp `navigation_as_compose_writes_it`: NavHost by name with a graph builder, typed routes,
// navigate with its options block, the back stack entry's destination, a nested graph, a deep link, the host's
// transitions, BackHandler and combinedClickable by name) and AppShellForms (`app_shell_as_compose_writes_it`:
// destination.hierarchy.any { it.hasRoute<T>() }, navOptions { popUpTo<T> { saveState = true } }, kompozable<T>(deepLinks
// =), navigation<T>(startDestination =), NavigationSuiteScaffold, a snackbar's visuals), and navigation-compose's
// NavigationSamples.kt, SizeTransformSample.kt and SharedElementSample.kt (2.10.2; the last two's edits at their
// sections below).
//
// Kotlin's classes are stage 14's: the @Serializable routes (Detail, Start, Shell, AccountGraph, Member, the samples'
// Profile, Scrollable, Dialog, Dashboard, Nested, NestedWithArg, User, WrappedUUID) and the samples' NavType object
// (WrappedUUIDNavType) and serializer are Rust twins beside this file (kotlin_nav_host.rs, D6). AppShellForms' desktop
// MenuBar is stage 15/16's (D7). The samples are androidx.compose.material's (Material 2): KompozR has Material 3,
// whose Button (`containerColor`, M2's `backgroundColor`), HorizontalDivider (M2's Divider) and TextField they use
// here (and `Color.LightGray` the source's static import `LightGray`); their strings are tests/navResources'
// (`R.string.*`), read as `stringResource(R.string.x)` where the source reads `X.resourceId` (the routes' property,
// one a companion object's: Kotlin's classes are stage 14's). The samples' screens Profile, Dashboard and Scrollable
// are ProfileScreen, DashboardScreen and ScrollableScreen here: Kotlin keeps a class and a function of one name apart,
// Rust's twin of the route object and the function would share its value namespace.

package androidx.navigation.compose.samples

// ---- the C++ Kompoz's Navigator -----------------------------------------------------------------------------------

@Kompozable
fun Navigator(nav: NavController) {
    val entry by nav.currentBackStackEntryAsState()
    val route = entry?.destination?.route
    var selecting by remember { mutableStateOf(false) }
    BackHandler(enabled = selecting) { selecting = false }
    Column {
        Text("rota ${route ?: "yok"}", modifier = Modifier.combinedClickable(onClick = {}, onLongClick = { selecting = true }))
        NavHost(navController = nav, startDestination = Start, modifier = Modifier.size(200.dp),
                enterTransition = { slideInHorizontally { it } }, exitTransition = { slideOutHorizontally { -it } }) {
            kompozable<Start> {
                Button(onClick = { nav.navigate(Detail(id = 7, tab = "b")) { launchSingleTop = true } }) { Text("Aç") }
            }
            kompozable<Detail> { backStackEntry ->
                val detail = backStackEntry.toRoute<Detail>()
                Text("ayrıntı ${detail.id} ${detail.tab ?: "-"}")
                Button(onClick = { nav.navigate(Start) { popUpTo<Start>(); launchSingleTop = true } }) { Text("Geri") }
            }
            navigation(startDestination = "a/{n}", route = "grup") {
                kompozable("a/{n}", arguments = listOf(navArgument("n") { type = NavType.IntType }),
                           deepLinks = listOf(navDeepLink { uriPattern = "kompoz://a/{n}" })) { e ->
                    Text("a ${e.arguments?.getInt("n") ?: 0}")
                }
            }
        }
    }
}

// The test's app: Navigator with the controller it remembers.
@Kompozable
fun NavigatorApp(onController: (NavHostController) -> Unit) {
    val nav = rememberNavController()
    onController(nav)
    Navigator(nav)
}

// ---- the C++ Kompoz's AppShellForms --------------------------------------------------------------------------------

@Kompozable
fun AppShellForms(nav: NavController) {
    val entry by nav.currentBackStackEntryAsState()
    val destination = entry?.destination
    val inAccount = destination?.hierarchy?.any { it.hasRoute<AccountGraph>() } == true
    val tabs = navOptions {
        popUpTo<Shell> { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
    val snackbars = remember { SnackbarHostState() }
    LaunchedEffect(Unit) { snackbars.showSnackbar("m", actionLabel = "Geri al", duration = SnackbarDuration.Indefinite) }
    val data = snackbars.currentSnackbarData
    Column {
        Text("eylem ${data?.visuals?.actionLabel?.length ?: 0}")
        NavigationSuiteScaffold(
            navigationItems = {
                NavigationSuiteItem(selected = !inAccount, onClick = { nav.navigate(Shell, tabs) },
                                    icon = { Icon(Icons.Filled.Home, contentDescription = null) }, label = { Text("Kabuk") })
                NavigationSuiteItem(selected = inAccount, onClick = { nav.navigate(AccountGraph, tabs) },
                                    icon = { Icon(Icons.Filled.Settings, contentDescription = null) }, label = { Text("Hesap") })
            },
            navigationSuiteType = NavigationSuiteType.NavigationRail,
            modifier = Modifier.weight(1f),
        ) {
            NavHost(navController = nav, startDestination = Shell) {
                kompozable<Shell> { Text(if (destination?.hasRoute<Shell>() == true) "kabuk ekranı" else "yolda") }
                navigation<AccountGraph>(startDestination = Member(0)) {
                    kompozable<Member>(deepLinks = listOf(navDeepLink<Member>("kompoz://member"))) { e -> Text("hesap ${e.toRoute<Member>().n}") }
                }
            }
        }
        Text(if (inAccount) "sekme hesap" else "sekme kabuk")
    }
}

@Kompozable
fun AppShellApp(onController: (NavHostController) -> Unit) {
    val nav = rememberNavController()
    onController(nav)
    AppShellForms(nav)
}

// ---- navigation-compose's NavigationSamples.kt ------------------------------------------------------------------

@Composable
fun BasicNav() {
    val navController = rememberNavController()
    NavHost(navController, startDestination = Profile) {
        composable<Profile> { ProfileScreen(navController) }
        composable<Dashboard>(
            enterTransition = {
                if (initialState.destination.hasRoute<Scrollable>()) {
                    // Slide in when entering from Scrollable
                    slideIntoContainer(AnimatedContentTransitionScope.SlideDirection.Start)
                } else {
                    null
                }
            },
            popExitTransition = {
                if (targetState.destination.hasRoute<Scrollable>()) {
                    // Slide out when popping back to Scrollable
                    slideOutOfContainer(AnimatedContentTransitionScope.SlideDirection.End)
                } else {
                    null
                }
            },
        ) {
            DashboardScreen(navController)
        }
        composable<Scrollable>(
            exitTransition = {
                if (targetState.destination.hasRoute<Dashboard>()) {
                    // Slide out when navigating to Dashboard
                    slideOutOfContainer(AnimatedContentTransitionScope.SlideDirection.Start)
                } else {
                    null
                }
            },
            popEnterTransition = {
                if (initialState.destination.hasRoute<Dashboard>()) {
                    // Slide back in when returning from Dashboard
                    slideIntoContainer(AnimatedContentTransitionScope.SlideDirection.End)
                } else {
                    null
                }
            },
        ) {
            ScrollableScreen(navController)
        }
        dialog<Dialog> { DialogContent(navController) }
    }
}

@Composable
fun NestedNavStartDestination() {
    val navController = rememberNavController()
    NavHost(navController, startDestination = Nested) {
        navigation<Nested>(startDestination = Profile) {
            composable<Profile> { ProfileScreen(navController) }
        }
        composable<Dashboard> { DashboardScreen(navController) }
        composable<Scrollable> { ScrollableScreen(navController) }
        dialog<Dialog> { DialogContent(navController) }
    }
}

@Composable
fun NestedNavInGraph() {
    val navController = rememberNavController()
    NavHost(navController, startDestination = Profile) {
        composable<Profile> { ProfileScreen(navController) }
        navigation<Dashboard>(startDestination = Nested) {
            composable<Nested> { DashboardScreen(navController) }
        }
        composable<Scrollable> { ScrollableScreen(navController) }
        dialog<Dialog> { DialogContent(navController) }
    }
}

@Composable
fun NavScaffold() {
    val navController = rememberNavController()
    Scaffold { innerPadding ->
        NavHost(navController, Profile, Modifier.padding(innerPadding)) {
            composable<Profile> { ProfileScreen(navController) }
            composable<Dashboard> { DashboardScreen(navController) }
            composable<Scrollable> { ScrollableScreen(navController) }
            dialog<Dialog> { DialogContent(navController) }
        }
    }
}

@Composable
fun NavWithArgsInNestedGraph() {
    val navController = rememberNavController()
    NavHost(navController, startDestination = Profile) {
        composable<Profile> { ProfileWithArgs(navController) }
        navigation<Dashboard>(startDestination = NestedWithArg::class) {
            composable<NestedWithArg> {
                // argument from parent graph Destination.Dashboard will automatically be
                // bundled into the start destination's arguments
                val userId: String? = it.toRoute<NestedWithArg>().userId
                DashboardScreen(navController, userId)
            }
        }
    }
}

@Composable
fun ProfileScreen(navController: NavHostController) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = stringResource(R.string.profile))
        NavigateButton(stringResource(R.string.dashboard)) { navController.navigate(Dashboard()) }
        HorizontalDivider(color = Color.Black)
        NavigateButton(stringResource(R.string.scrollable)) { navController.navigate(Scrollable) }
        HorizontalDivider(color = Color.Black)
        NavigateButton(stringResource(R.string.dialog)) { navController.navigate(Dialog) }
        Spacer(Modifier.weight(1f))
        NavigateBackButton(navController)
    }
}

@Composable
fun ProfileWithArgs(navController: NavController) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = stringResource(R.string.profile))
        HorizontalDivider(color = Color.Black)
        val state = rememberSaveable { mutableStateOf("") }
        Box {
            TextField(
                value = state.value,
                onValueChange = { state.value = it },
                placeholder = { Text("Enter userId here") },
            )
        }
        HorizontalDivider(color = Color.Black)
        NavigateButton("Dashboard with userId") { navController.navigate(Dashboard(state.value)) }
    }
}

@Composable
fun DashboardScreen(navController: NavController, title: String? = null) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = title ?: stringResource(R.string.dashboard))
        Spacer(Modifier.weight(1f))
        NavigateBackButton(navController)
    }
}

@Composable
fun ScrollableScreen(navController: NavController) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        NavigateButton(stringResource(R.string.dashboard)) { navController.navigate(Dashboard()) }
        LazyColumn(modifier = Modifier.weight(1f)) {
            items(phrases) { phrase -> Text(phrase, fontSize = 30.sp) }
        }
        NavigateBackButton(navController)
    }
}

@Composable
fun DialogContent(navController: NavController) {
    val dialogWidth = 300.dp
    val dialogHeight = 300.dp
    Column(Modifier.size(dialogWidth, dialogHeight).background(Color.White).padding(8.dp)) {
        NavigateBackButton(navController)
        LazyColumn(modifier = Modifier.weight(1f)) {
            items(phrases) { phrase -> Text(phrase, fontSize = 16.sp) }
        }
    }
}

@Composable
fun NavigateButton(text: String, listener: () -> Unit = {}) {
    Button(
        onClick = listener,
        colors = ButtonDefaults.buttonColors(containerColor = Color.LightGray),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(text = "Navigate to $text")
    }
}

@Composable
fun NavigateBackButton(navController: NavController) {
    // Use LocalLifecycleOwner.current as a proxy for the NavBackStackEntry
    // associated with this Composable
    if (
        navController.currentBackStackEntry == LocalLifecycleOwner.current &&
            navController.previousBackStackEntry != null
    ) {
        Button(
            onClick = { navController.popBackStack() },
            colors = ButtonDefaults.buttonColors(containerColor = Color.LightGray),
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(text = "Go to Previous screen")
        }
    }
}

@Composable
fun NavHostPreview() {
    CompositionLocalProvider(LocalInspectionMode provides true) {
        Box(Modifier.fillMaxSize().background(Color.Red)) {
            NavHost(navController = rememberNavController(), startDestination = "home") {
                composable("home") {
                    Box(Modifier.fillMaxSize().background(Color.Blue)) {
                        Text(text = "test", modifier = Modifier.testTag("text"))
                    }
                }
            }
        }
    }
}

@Composable
fun ThirdPartySerializableType() {
    val navController = rememberNavController()
    NavHost(navController, startDestination = User(WrappedUUID(UUID.randomUUID()))) {
        composable<User>(typeMap = mapOf(typeOf<WrappedUUID>() to WrappedUUIDNavType)) { entry ->
            val uuid = entry.toRoute<User>().id.uuid
            Box(Modifier.fillMaxSize().background(Color.Blue)) {
                Text(text = "uuid: $uuid", modifier = Modifier.testTag("text"))
            }
        }
    }
}

val phrases =
    listOf(
        "Easy As Pie",
        "Wouldn't Harm a Fly",
        "No-Brainer",
        "Keep On Truckin'",
        "An Arm and a Leg",
        "Down To Earth",
        "Under the Weather",
        "Up In Arms",
        "Cup Of Joe",
        "Not the Sharpest Tool in the Shed",
        "Ring Any Bells?",
        "Son of a Gun",
        "Hard Pill to Swallow",
        "Close But No Cigar",
        "Beating a Dead Horse",
        "If You Can't Stand the Heat, Get Out of the Kitchen",
        "Cut To The Chase",
        "Heads Up",
        "Goody Two-Shoes",
        "Fish Out Of Water",
        "Cry Over Spilt Milk",
        "Elephant in the Room",
        "There's No I in Team",
        "Poke Fun At",
        "Talk the Talk",
        "Know the Ropes",
        "Fool's Gold",
        "It's Not Brain Surgery",
        "Fight Fire With Fire",
        "Go For Broke",
    )

// ---- navigation-compose's SizeTransformSample.kt ------------------------------------------------------------------
//
// SizeTransformComposable and SizeTransformNav as written. The @Serializable objects Collapsed, Expanded, NestedGraph
// and InnerCollapsed are Rust twins beside this file (kotlin_nav_host.rs: Kotlin's classes are stage 14's).

@Composable
fun SizeTransformComposable() {
    val navController = rememberNavController()
    Box {
        NavHost(navController, startDestination = Collapsed) {
            composable<Collapsed>(
                enterTransition = { EnterTransition.None },
                exitTransition = { ExitTransition.None },
                sizeTransform = {
                    SizeTransform { initialSize, targetSize ->
                        keyframes {
                            durationMillis = 500
                            IntSize(
                                initialSize.width,
                                (initialSize.height + targetSize.height) / 2,
                            ) at 150
                        }
                    }
                },
            ) {
                CollapsedScreen { navController.navigate(Expanded) }
            }
            composable<Expanded>(
                enterTransition = { EnterTransition.None },
                exitTransition = { ExitTransition.None },
                sizeTransform = {
                    SizeTransform { initialSize, targetSize ->
                        keyframes {
                            durationMillis = 500
                            IntSize(targetSize.width, initialSize.height + 400) at 150
                        }
                    }
                },
            ) {
                ExpandedScreen { navController.popBackStack() }
            }
        }
    }
}

@Composable
fun SizeTransformNav() {
    val navController = rememberNavController()
    Box {
        NavHost(navController, startDestination = Collapsed) {
            navigation<NestedGraph>(
                enterTransition = { EnterTransition.None },
                exitTransition = { ExitTransition.None },
                startDestination = InnerCollapsed::class,
                sizeTransform = {
                    SizeTransform { initialSize, targetSize ->
                        keyframes {
                            durationMillis = 500
                            IntSize(
                                initialSize.width,
                                (initialSize.height + targetSize.height) / 2,
                            ) at 150
                        }
                    }
                },
            ) {
                composable<InnerCollapsed>(
                    enterTransition = { EnterTransition.None },
                    exitTransition = { ExitTransition.None },
                ) {
                    ExpandedScreen {}
                }
            }
        }
    }
}

@Composable
fun CollapsedScreen(onNavigate: () -> Unit) {
    Box(Modifier.clickable { onNavigate() }.size(40.dp).background(Green))
}

@Composable
fun ExpandedScreen(onNavigate: () -> Unit) {
    Box(Modifier.clickable { onNavigate() }.size(500.dp).background(Blue))
}

private val Blue = Color(0xFF2196F3)
private val Green = Color(0xFF4CAF50)

// ---- navigation-compose's SharedElementSample.kt ------------------------------------------------------------------
//
// AnimatedNav, SharedElementAnimationNav and TopAppBarElement as written. The @Serializable objects Select,
// SharedElement, TopAppBarShared, RedBox, BlueBox, First, Second and Third are Rust twins beside this file
// (kotlin_nav_host.rs: Kotlin's classes are stage 14's). The sample's composables RedBox and BlueBox are RedBoxScreen
// and BlueBoxScreen here: Kotlin keeps a class and a function of one name apart, Rust's twin of the route object and
// the function would share its value namespace. The sample is androidx.compose.material's (Material 2): its Button,
// Text and TopAppBar are Material 3's here (M3's TopAppBar, experimental in Compose, needs no @OptIn in KompozR).

@Composable
fun AnimatedNav() {
    val navController = rememberNavController()
    NavHost(navController, startDestination = Select) {
        composable<Select> {
            Column {
                Box(
                    Modifier.heightIn(min = 48.dp)
                        .fillMaxWidth()
                        .clickable(onClick = { navController.navigate(SharedElement) })
                        .padding(horizontal = 16.dp)
                        .wrapContentHeight(Alignment.CenterVertically),
                    contentAlignment = Alignment.CenterStart,
                ) {
                    Text("AnimationNav")
                }
                Box(
                    Modifier.heightIn(min = 48.dp)
                        .fillMaxWidth()
                        .clickable(onClick = { navController.navigate(TopAppBarShared) })
                        .padding(horizontal = 16.dp)
                        .wrapContentHeight(Alignment.CenterVertically),
                    contentAlignment = Alignment.CenterStart,
                ) {
                    Text("Top Bar Shared Element")
                }
            }
        }
        composable<SharedElement> { SharedElementAnimationNav() }
        composable<TopAppBarShared> { TopAppBarElement() }
    }
}

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun SharedElementAnimationNav() {
    val navController = rememberNavController()
    SharedTransitionLayout {
        val selectFirst = mutableStateOf(true)
        NavHost(navController, startDestination = RedBox) {
            composable<RedBox> {
                RedBoxScreen(this@SharedTransitionLayout, this, selectFirst) {
                    navController.navigate(BlueBox)
                }
            }
            composable<BlueBox> {
                BlueBoxScreen(this@SharedTransitionLayout, this, selectFirst) {
                    navController.popBackStack()
                }
            }
        }
    }
}

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun RedBoxScreen(
    sharedScope: SharedTransitionScope,
    scope: AnimatedContentScope,
    selectFirst: MutableState<Boolean>,
    onNavigate: () -> Unit,
) {
    with(sharedScope) {
        Box(
            Modifier.sharedBounds(
                    rememberSharedContentState("name"),
                    scope,
                    renderInOverlayDuringTransition = selectFirst.value,
                )
                .clickable(
                    onClick = {
                        selectFirst.value = !selectFirst.value
                        onNavigate()
                    }
                )
                .background(Color.Red)
                .size(100.dp)
        ) {
            Text("start", color = Color.White)
        }
    }
}

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun BlueBoxScreen(
    sharedScope: SharedTransitionScope,
    scope: AnimatedContentScope,
    selectFirst: MutableState<Boolean>,
    onPopBack: () -> Unit,
) {
    with(sharedScope) {
        Box(
            Modifier.offset(180.dp, 180.dp)
                .sharedBounds(
                    rememberSharedContentState("name"),
                    scope,
                    renderInOverlayDuringTransition = !selectFirst.value,
                )
                .clickable(
                    onClick = {
                        selectFirst.value = !selectFirst.value
                        onPopBack()
                    }
                )
                .alpha(0.5f)
                .background(Color.Blue)
                .size(180.dp)
        ) {
            Text("finish", color = Color.White)
        }
    }
}

@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun TopAppBarElement() {
    val navController = rememberNavController()
    SharedTransitionLayout {
        NavHost(navController, startDestination = First) {
            composable<First>(
                enterTransition = { slideIntoContainer(SlideDirection.Right) },
                exitTransition = { slideOutOfContainer(SlideDirection.Left) },
                popExitTransition = { slideOutOfContainer(SlideDirection.Right) },
            ) {
                Column {
                    TopAppBar(
                        title = { Text("first") },
                        modifier =
                            Modifier.sharedElement(
                                rememberSharedContentState("appBar"),
                                this@composable,
                            ),
                    )
                    Text("first", color = Color.White)
                    Button(onClick = { navController.navigate(Second) }) {
                        Text("Navigate to Second")
                    }
                }
            }
            composable<Second>(
                enterTransition = { slideIntoContainer(SlideDirection.Right) },
                exitTransition = { slideOutOfContainer(SlideDirection.Left) },
                popExitTransition = { slideOutOfContainer(SlideDirection.Right) },
            ) {
                Column {
                    TopAppBar(
                        title = { Text("second") },
                        modifier =
                            Modifier.sharedElement(
                                rememberSharedContentState("appBar"),
                                this@composable,
                            ),
                    )
                    Text("second", color = Color.White)
                    Button(onClick = { navController.navigate(Third) }) {
                        Text("Navigate to Third")
                    }
                }
            }
            composable<Third>(
                enterTransition = { slideIntoContainer(SlideDirection.Right) },
                exitTransition = { slideOutOfContainer(SlideDirection.Left) },
                popExitTransition = { slideOutOfContainer(SlideDirection.Right) },
            ) {
                Column {
                    Text("third", color = Color.White)
                    Button(onClick = { navController.popBackStack<First>(false) }) {
                        Text("Pop back to First")
                    }
                }
            }
        }
    }
}

// The NavHost as Compose writes it (navigation-compose 2.10): the C++ Kompoz's Navigator
// (kompozable_tests.cpp `navigation_as_compose_writes_it`: NavHost by name with a graph builder, typed routes,
// navigate with its options block, the back stack entry's destination, a nested graph, a deep link, the host's
// transitions, BackHandler and combinedClickable by name) and AppShellForms (`app_shell_as_compose_writes_it`:
// destination.hierarchy.any { it.hasRoute<T>() }, navOptions { popUpTo<T> { saveState = true } }, kompozable<T>(deepLinks
// =), navigation<T>(startDestination =), NavigationSuiteScaffold, a snackbar's visuals), and navigation-compose's
// NavigationSamples.kt (2.10.2): the @Serializable routes as classes and objects (a companion's `resourceId`), the
// screens Profile, Dashboard and Scrollable beside the routes of their names, the samples' NavType object.
//
// AppShellForms' desktop MenuBar is stage 15/16's (D7). The samples are androidx.compose.material's (Material 2):
// KompozR has Material 3, whose Button (`containerColor`, M2's `backgroundColor`), HorizontalDivider (M2's Divider)
// and TextField they use here (and `Color.LightGray` the source's static import `LightGray`); their strings are
// tests/navResources' (`R.string.*`). NavigationSamples' `@Serializable class User` and `@Serializable class
// WrappedUUID` (plain classes, not data classes, as route and route value) are Rust beside the test's copy of this
// file (crates/kompoz/tests/kotlin_nav_host.rs): the Kotlin syntax derives a route for a `@Serializable` data class or
// object only.

package androidx.navigation.compose.samples

// ---- the C++ Kompoz's Navigator -----------------------------------------------------------------------------------

@Serializable
data class Detail(val id: Int, val tab: String? = null)

@Serializable
object Start

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

@Serializable
object Shell

@Serializable
object AccountGraph

@Serializable
data class Member(val n: Int)

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

// (`resourceId: Int`, an Android R id, is a StringResource here: KompozR's resources are Compose Multiplatform's)
@Serializable
object Profile {
    val resourceId: StringResource = R.string.profile
}

@Serializable
object Scrollable {
    val resourceId: StringResource = R.string.scrollable
}

@Serializable
object Dialog {
    val resourceId: StringResource = R.string.dialog
}

@Serializable
data class Dashboard(val userId: String? = "no value given") {
    companion object {
        val resourceId: StringResource = R.string.dashboard
    }
}

@Serializable object Nested

@Serializable data class NestedWithArg(val userId: String? = "default nested arg")

@Composable
fun BasicNav() {
    val navController = rememberNavController()
    NavHost(navController, startDestination = Profile) {
        composable<Profile> { Profile(navController) }
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
            Dashboard(navController)
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
            Scrollable(navController)
        }
        dialog<Dialog> { DialogContent(navController) }
    }
}

@Composable
fun NestedNavStartDestination() {
    val navController = rememberNavController()
    NavHost(navController, startDestination = Nested) {
        navigation<Nested>(startDestination = Profile) {
            composable<Profile> { Profile(navController) }
        }
        composable<Dashboard> { Dashboard(navController) }
        composable<Scrollable> { Scrollable(navController) }
        dialog<Dialog> { DialogContent(navController) }
    }
}

@Composable
fun NestedNavInGraph() {
    val navController = rememberNavController()
    NavHost(navController, startDestination = Profile) {
        composable<Profile> { Profile(navController) }
        navigation<Dashboard>(startDestination = Nested) {
            composable<Nested> { Dashboard(navController) }
        }
        composable<Scrollable> { Scrollable(navController) }
        dialog<Dialog> { DialogContent(navController) }
    }
}

@Composable
fun NavScaffold() {
    val navController = rememberNavController()
    Scaffold { innerPadding ->
        NavHost(navController, Profile, Modifier.padding(innerPadding)) {
            composable<Profile> { Profile(navController) }
            composable<Dashboard> { Dashboard(navController) }
            composable<Scrollable> { Scrollable(navController) }
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
                val userId = it.toRoute<NestedWithArg>().userId
                Dashboard(navController, userId)
            }
        }
    }
}

@Composable
fun Profile(navController: NavHostController) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = stringResource(Profile.resourceId))
        NavigateButton(stringResource(Dashboard.resourceId)) { navController.navigate(Dashboard()) }
        HorizontalDivider(color = Color.Black)
        NavigateButton(stringResource(Scrollable.resourceId)) { navController.navigate(Scrollable) }
        HorizontalDivider(color = Color.Black)
        NavigateButton(stringResource(Dialog.resourceId)) { navController.navigate(Dialog) }
        Spacer(Modifier.weight(1f))
        NavigateBackButton(navController)
    }
}

@Composable
fun ProfileWithArgs(navController: NavController) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = stringResource(Profile.resourceId))
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
fun Dashboard(navController: NavController, title: String? = null) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = title ?: stringResource(Dashboard.resourceId))
        Spacer(Modifier.weight(1f))
        NavigateBackButton(navController)
    }
}

@Composable
fun Scrollable(navController: NavController) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        NavigateButton(stringResource(Dashboard.resourceId)) { navController.navigate(Dashboard()) }
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

object WrappedUUIDNavType : NavType<WrappedUUID>(isNullableAllowed = false) {
    override fun put(bundle: Bundle, key: String, value: WrappedUUID) {
        bundle.putString(key, value.uuid.toString())
    }

    // (Bundle.getString's `String?` given to Java's UUID.fromString, whose parameter is a platform type Kotlin
    // doesn't check: `!!` here, KompozR's java.util being Kotlin's types)
    override fun get(bundle: Bundle, key: String): WrappedUUID? =
        WrappedUUID(UUID.fromString(bundle.getString(key)!!))

    override fun parseValue(value: String): WrappedUUID = WrappedUUID(UUID.fromString(value))

    override fun serializeAsValue(value: WrappedUUID): String = Uri.encode(value.uuid.toString())
}

// (the sample's `object UUIDSerializer : KSerializer<UUID>`, WrappedUUID's `@Serializable(with = …)`: KompozR's
// routes keep a UUID by its own NavType, the typeMap's above)

private val phrases =
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

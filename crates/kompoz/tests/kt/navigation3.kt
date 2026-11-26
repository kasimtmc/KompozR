// Navigation 3 as a Compose app writes it: navigation3-ui 1.2's samples (OverlaySamples.kt, SceneStateSamples.kt,
// CustomSceneSamples.kt, BasicSamples.kt, DeepLinkNonPrimitiveArgSamples.kt, SceneStrategySamples.kt,
// ComplexBackStackSamples.kt, AnimatedOverlaySamples.kt) and navigation3-runtime 1.2's
// NavigationSamples.kt (the screens BasicSamples.kt imports), as written (their @Sampled/@OptIn annotations left out);
// checked by kotlin_navigation3.rs. The runtime samples of their own package are kt/navigation3_runtime.kt.
//
// What KompozR has otherwise: NavigationSamples.kt's `R.string.*` are tests/navResources' (`compose_resources!`), so
// its keys' `resourceId`s are Compose Multiplatform's StringResource where Android's are `Int`s (as kt/carousel.kt's),
// its Material 2 `Divider` Material 3's `HorizontalDivider` (as kt/nav_host.kt's).
//
// (CustomSceneSamples.kt's scenes and strategies are generic classes of the crate implementing the API's generic
// interfaces, their `T` bounded by what the API's types need of it; SceneStrategySamples.kt's are local ones;
// ComplexBackStackSamples.kt casts keys to the crate's interfaces of `NavKey`; AnimatedOverlaySamples.kt's object
// expression has a `lateinit var` its members share — see its section for what KompozR writes otherwise there.)
//
// Stage 15's (Android): BasicSamples.kt's SceneNavSharedEntrySample and SceneNavSharedElementSample — their `Cat`
// key's `@DrawableRes val imageId: Int`, Android's resource ids (`R.drawable.cat_1`), a serializable property of a
// navigation key and the shared elements' key: Compose Multiplatform's resources are no integers (Rust's form of the
// samples: crates/kompoz-navigation3/tests/navigation3.rs).

package androidx.navigation3.ui.samples

import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.BasicText
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.HouseSiding
import androidx.compose.material.icons.filled.Person
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Color.Companion.LightGray
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.navigation3.rememberViewModelStoreNavEntryDecorator
import androidx.navigation3.runtime.*
import androidx.navigation3.runtime.deeplink.DeepLinkRequest
import androidx.navigation3.runtime.deeplink.DeepLinkSerializer
import androidx.navigation3.runtime.deeplink.DeepLinkUri
import androidx.navigation3.runtime.deeplink.UriDeepLinkMatcher
import androidx.navigation3.scene.*
import androidx.navigation3.ui.NavDisplay
import androidx.navigationevent.NavigationEvent
import androidx.navigationevent.compose.NavigationBackHandler
import androidx.navigationevent.compose.rememberNavigationEventState
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.serializer

// ---- OverlaySamples.kt ----

@Serializable private object OverlaidNavKey : NavKey

@Serializable private object DialogNavKey : NavKey

@Composable
fun DialogSample() {
    val backStack = rememberNavBackStack(OverlaidNavKey)
    NavDisplay(
        backStack,
        onBack = { backStack.removeLastOrNull() },
        sceneStrategies = listOf(DialogSceneStrategy()),
        entryProvider =
            entryProvider {
                entry<OverlaidNavKey> {
                    Box(
                        Modifier.fillMaxSize()
                            .background(Color(0.2f, 0.2f, 1.0f, 1.0f))
                            .border(10.dp, Color.Blue),
                        contentAlignment = Alignment.Center,
                    ) {
                        Column(horizontalAlignment = Alignment.CenterHorizontally) {
                            BasicText(
                                "Overlaid Entry",
                                Modifier.size(50.dp),
                                style = TextStyle(textAlign = TextAlign.Center),
                            )
                            Button(onClick = { backStack.add(DialogNavKey) }) {
                                Text("Open Dialog")
                            }
                        }
                    }
                }
                entry<DialogNavKey>(metadata = metadata { put(DialogKey, DialogProperties()) }) {
                    RedBox("Dialog")
                }
            },
    )
}

// ---- SceneStateSamples.kt ----

@Composable
fun SceneStateSample() {
    val backStack = rememberSaveable { mutableStateListOf("a", "b") }
    val entries =
        rememberDecoratedNavEntries(backStack) { key -> NavEntry(key) { Text("Key = $key") } }
    val sceneState =
        rememberSceneState(
            entries,
            listOf(SinglePaneSceneStrategy()),
            onBack = { backStack.removeLastOrNull() },
        )
    val currentScene = sceneState.currentScene
    val navigationEventState =
        rememberNavigationEventState(
            currentInfo = SceneInfo(currentScene),
            backInfo = sceneState.previousScenes.map { SceneInfo(it) },
        )
    NavigationBackHandler(
        navigationEventState,
        isBackEnabled = currentScene.previousEntries.isNotEmpty(),
        onBackCompleted = {
            // Remove entries from the back stack until we've removed all popped entries
            repeat(entries.size - currentScene.previousEntries.size) {
                backStack.removeLastOrNull()
            }
        },
    )
    NavDisplay(sceneState, navigationEventState)
}

// ---- CustomSceneSamples.kt (its generic strategy and scene: Rust beside the file, see above) ----

@Serializable private object A : NavKey

@Serializable private object B : NavKey

@Serializable private object C : NavKey

@Composable
fun SceneDefaultTransitionsSample() {
    val backStack = rememberNavBackStack(A)
    NavDisplay(
        backStack,
        onBack = { backStack.removeLastOrNull() },
        entryDecorators = listOf(rememberSaveableStateHolderNavEntryDecorator()),
        // NavDisplay default transitions slide vertically
        transitionSpec = { slideVertical },
        popTransitionSpec = { slideVertical },
        predictivePopTransitionSpec = { slideVertical },
        // but the Scene provides default transitions that slide horizontally
        sceneStrategies = listOf(DefaultSceneTransitionsSceneStrategy()),
        entryProvider =
            entryProvider {
                entry<A> { BlueBox("A") { backStack.add(B) } }
                entry<B> { RedBox("B") }
            },
    )
}

private class DefaultSceneTransitionsSceneStrategy<T : Any>() : SceneStrategy<T> {
    override fun SceneStrategyScope<T>.calculateScene(entries: List<NavEntry<T>>): Scene<T>? {
        if (entries.isEmpty()) return null
        return DefaultSceneTransitionsScene(entries.last(), entries.dropLast(1))
    }
}

private data class DefaultSceneTransitionsScene<T : Any>(
    val currentEntry: NavEntry<T>,
    override val previousEntries: List<NavEntry<T>>,
) : Scene<T> {
    override val key = currentEntry.contentKey
    override val entries: List<NavEntry<T>> = listOf(currentEntry)
    override val content: @Composable (() -> Unit) = { currentEntry.Content() }
    // overrides the default animations passed to NavDisplay
    override val metadata: Map<String, Any> = sceneTransitions
}

@Composable
fun SceneOverrideEntryTransitionsSample() {
    val backStack = rememberNavBackStack(A)
    NavDisplay(
        backStack,
        onBack = { backStack.removeLastOrNull() },
        entryDecorators = listOf(rememberSaveableStateHolderNavEntryDecorator()),
        // the Scene overrides the NavEntry's slide vertical with slide horizontal transitions
        sceneStrategies = listOf(SceneOverrideEntryTransitionsSceneStrategy()),
        entryProvider =
            entryProvider {
                entry<A> { BlueBox("A") { backStack.add(B) } }
                // the entry defines slide vertical transitions
                entry<B>(
                    metadata =
                        metadata {
                            put(NavDisplay.TransitionKey) { slideVertical }
                            put(NavDisplay.PopTransitionKey) { slideVertical }
                            put(NavDisplay.PredictivePopTransitionKey, { _: Int -> slideVertical })
                        }
                ) {
                    RedBox("B")
                }
            },
    )
}

private class SceneOverrideEntryTransitionsSceneStrategy<T : Any>() : SceneStrategy<T> {
    override fun SceneStrategyScope<T>.calculateScene(entries: List<NavEntry<T>>): Scene<T>? {
        if (entries.isEmpty()) return null
        return SceneOverrideEntryTransitionsScene(entries.last(), entries.dropLast(1))
    }
}

private data class SceneOverrideEntryTransitionsScene<T : Any>(
    val currentEntry: NavEntry<T>,
    override val previousEntries: List<NavEntry<T>>,
) : Scene<T> {
    override val key = currentEntry.contentKey
    override val entries: List<NavEntry<T>> = listOf(currentEntry)
    override val content: @Composable (() -> Unit) = { currentEntry.Content() }
    // super.metadata defaults to [entries.last()]'s metadata
    // so this overrides the last entry's transitions
    override val metadata: Map<String, Any> = super.metadata + sceneTransitions
}

private val sceneTransitions = metadata {
    put(NavDisplay.TransitionKey) { slideHorizontal }
    put(NavDisplay.PopTransitionKey) { slideHorizontal }
    put(NavDisplay.PredictivePopTransitionKey) { slideHorizontal }
}

// ---- SceneStrategySamples.kt (navigation3-ui's scene samples) ----

@Suppress("FunctionName", "unused")
fun SceneStrategyOnBackSample() {
    /** Class that shows a [NavEntry] as a [Dialog]. */
    class SimpleDialogScene<T : Any>(
        private val entry: NavEntry<T>,
        override val previousEntries: List<NavEntry<T>>,
        onBack: () -> Unit,
    ) : OverlayScene<T> {
        override val key = entry.contentKey
        override val entries = listOf(entry)
        override val overlaidEntries = previousEntries

        override val content: @Composable () -> Unit = {
            Dialog(onDismissRequest = onBack) { entry.Content() }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is SimpleDialogScene<*>) return false
            return entry == other.entry && previousEntries == other.previousEntries
        }

        override fun hashCode(): Int {
            return entry.hashCode() * 31 + previousEntries.hashCode()
        }
    }

    /**
     * A [SceneStrategy] that shows every screen above the first one as a dialog.
     *
     * In a real example, you'd have developers opt into displaying their entry as a dialog by
     * providing a companion method that provides [NavEntry.metadata].
     *
     * @see androidx.navigation3.scene.DialogSceneStrategy
     */
    class SimpleDialogSceneStrategy<T : Any> : SceneStrategy<T> {
        override fun SceneStrategyScope<T>.calculateScene(entries: List<NavEntry<T>>): Scene<T>? {
            val entry = entries.lastOrNull() ?: return null
            val previousEntries = entries.dropLast(1)
            // Only show this as a dialog if there is something 'underneath' the dialog
            return if (previousEntries.isNotEmpty()) {
                SimpleDialogScene(entry, previousEntries, onBack)
            } else {
                null
            }
        }
    }
}

// ---- CustomSceneSamples.kt (continued) ----

private val duration = 5000
private val animSpec: FiniteAnimationSpec<IntOffset> =
    tween(duration, easing = LinearOutSlowInEasing)
private val slideHorizontal =
    slideInHorizontally(animSpec) { it / 2 } togetherWith slideOutHorizontally(animSpec) { -it / 2 }
private val slideVertical =
    slideInVertically(animSpec) { it / 2 } togetherWith slideOutVertically(animSpec) { -it / 2 }

@Composable
fun BlueBox(text: String, onClick: (() -> Unit)?) {
    Box(
        Modifier.fillMaxSize().background(Color(0.2f, 0.2f, 1.0f, 1.0f)).border(10.dp, Color.Blue),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            BasicText(text, Modifier.size(50.dp), style = TextStyle(textAlign = TextAlign.Center))
            if (onClick != null) {
                Button(onClick = onClick) { Text("Click to navigate") }
            }
        }
    }
}

@Composable
fun RedBox(text: String) {
    Box(
        Modifier.fillMaxSize().background(Color(1.0f, 0.3f, 0.3f, 1.0f)).border(10.dp, Color.Red),
        contentAlignment = Alignment.Center,
    ) {
        BasicText(text, Modifier.size(50.dp), style = TextStyle(textAlign = TextAlign.Center))
    }
}

@Composable
fun GreenBox(text: String) {
    Box(Modifier.fillMaxSize().background(Color.Green), contentAlignment = Alignment.Center) {
        BasicText(text, Modifier.size(300.dp), style = TextStyle(textAlign = TextAlign.Center))
    }
}

// ---- BasicSamples.kt ----

@Composable
fun SceneNav() {
    val backStack = rememberNavBackStack(Profile)
    val showDialog = remember { mutableStateOf(false) }
    NavDisplay(
        backStack = backStack,
        entryDecorators =
            listOf(
                rememberSaveableStateHolderNavEntryDecorator(),
                rememberViewModelStoreNavEntryDecorator(),
            ),
        onBack = { backStack.removeAt(backStack.lastIndex) },
        entryProvider =
            // (the fallback's `NavKey` written: Rust's closure is made before the generic call telling its type)
            entryProvider({ key: NavKey -> NavEntry(key) { Text(text = "Invalid Key") } }) {
                entry<Profile> {
                    val viewModel = viewModel<ProfileViewModel>()
                    Profile(viewModel, { backStack.add(it) }) {
                        backStack.removeAt(backStack.lastIndex)
                    }
                }
                entry<Scrollable> {
                    Scrollable({ backStack.add(it) }) { backStack.removeAt(backStack.lastIndex) }
                }
                entry<DialogBase> {
                    DialogBase(onClick = { showDialog.value = true }) {
                        backStack.removeAt(backStack.lastIndex)
                    }
                }
                entry<Dashboard>(
                    metadata =
                        metadata {
                            put(NavDisplay.PredictivePopTransitionKey) { swipeEdge: Int ->
                                if (swipeEdge == NavigationEvent.EDGE_RIGHT) {
                                    slideInHorizontally(tween(700)) { it / 2 } togetherWith
                                        slideOutHorizontally(tween(700)) { -it / 2 }
                                } else {
                                    slideInHorizontally(tween(700)) { -it / 2 } togetherWith
                                        slideOutHorizontally(tween(700)) { it / 2 }
                                }
                            }
                        }
                ) { dashboardArgs ->
                    val userId = dashboardArgs.userId
                    Dashboard(userId, onBack = { backStack.removeAt(backStack.lastIndex) })
                }
            },
    )
    if (showDialog.value) {
        DialogContent(onDismissRequest = { showDialog.value = false })
    }
}

// ---- navigation3-runtime's NavigationSamples.kt ----

@Serializable
object Profile : NavKey {
    val resourceId: StringResource = R.string.profile
}

class ProfileViewModel : ViewModel() {
    val name = "no user"
}

@Serializable
object Scrollable : NavKey {
    val resourceId: StringResource = R.string.scrollable
}

@Serializable
object DialogBase : NavKey {
    val resourceId: StringResource = R.string.dialog
}

@Serializable
data class Dashboard(val userId: String? = "no value given") : NavKey {
    companion object {
        val resourceId: StringResource = R.string.dashboard
    }
}

@Composable
fun Profile(viewModel: ProfileViewModel, navigateTo: (NavKey) -> Unit, onBack: () -> Unit) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = "${viewModel.name} ${stringResource(Profile.resourceId)}")
        NavigateButton(stringResource(Dashboard.resourceId)) { navigateTo(Dashboard()) }
        HorizontalDivider(color = Color.Black)
        NavigateButton(stringResource(Scrollable.resourceId)) { navigateTo(Scrollable) }
        HorizontalDivider(color = Color.Black)
        NavigateButton(stringResource(DialogBase.resourceId)) { navigateTo(DialogBase) }
        Spacer(Modifier.weight(1f))
        NavigateBackButton(onBack)
    }
}

@Composable
fun Dashboard(title: String? = null, onBack: () -> Unit) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = title ?: stringResource(Dashboard.resourceId))
        Spacer(Modifier.weight(1f))
        NavigateBackButton(onBack)
    }
}

@Composable
fun Scrollable(navigateTo: (NavKey) -> Unit, onBack: () -> Unit) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        NavigateButton(stringResource(Dashboard.resourceId)) { navigateTo(Dashboard()) }
        LazyColumn(modifier = Modifier.weight(1f)) {
            items(phrases) { phrase -> Text(phrase, fontSize = 30.sp) }
        }
        NavigateBackButton(onBack)
    }
}

@Composable
fun DialogBase(onClick: () -> Unit, onBack: () -> Unit) {
    Column(Modifier.fillMaxSize()) {
        Text(stringResource(R.string.dialog))
        Button(onClick = onClick) { Text("Show Dialog") }
        Spacer(Modifier.weight(1f))
        NavigateBackButton(onBack)
    }
}

@Composable
fun DialogContent(onDismissRequest: () -> Unit) {
    Dialog(onDismissRequest = onDismissRequest) {
        val dialogWidth = 300.dp
        val dialogHeight = 300.dp
        Column(Modifier.size(dialogWidth, dialogHeight).background(Color.White).padding(8.dp)) {
            LazyColumn(modifier = Modifier.weight(1f)) {
                items(phrases) { phrase -> Text(phrase, fontSize = 16.sp) }
            }
        }
    }
}

@Composable
fun NavigateButton(text: String, listener: () -> Unit = {}) {
    Button(
        onClick = listener,
        colors = ButtonDefaults.buttonColors(containerColor = LightGray),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(text = "Navigate to $text")
    }
}

@Composable
fun NavigateBackButton(onBack: () -> Unit) {
    Button(
        onClick = onBack,
        colors = ButtonDefaults.buttonColors(containerColor = LightGray),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(text = "Go to Previous screen")
    }
}

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

// ---- DeepLinkNonPrimitiveArgSamples.kt ----

@Serializable private object HomeKey : NavKey

/** Resources for [NonPrimitiveArgFlattenedSample] */
@Serializable private data class Name(val firstName: String, val lastName: String)

@Serializable private data class ProfileKey(val name: Name, val age: Int) : NavKey

/** Resources for [DeepLinkSerializerSample] */
private data class Product(val id: Int, val name: String)

@Serializable
private data class ProductInventoryKey(
    @Serializable(with = ProductSerializer::class) val product: Product,
    val count: Int,
) : NavKey

// serializer to deserialize string value into a Product object
private object ProductSerializer : DeepLinkSerializer<Product>() {
    override val serialName: String = "androidx.navigation3.ui.samples.Product"

    override fun deserialize(value: String): Product {
        val splitArgs = value.split("-", limit = 2)
        val id =
            splitArgs[0].toIntOrNull()
                ?: throw SerializationException("Invalid product id: ${splitArgs[0]}")
        val name = splitArgs[1]
        return Product(id, name)
    }

    override fun serialize(value: Product): String = "${value.id}-${value.name}"
}

/** Resources for [ListTypeDeepLinkSerializerSample] */
private data class Hat(val id: Int, val weight: Double)

@Serializable
private data class HatListKey(val hats: List<@Serializable(with = HatSerializer::class) Hat>) :
    NavKey

// serializer to deserialize string value into a Hat object
private object HatSerializer : DeepLinkSerializer<Hat>() {
    override val serialName: String = "androidx.navigation3.ui.samples.Hat"

    override fun deserialize(value: String): Hat {
        val splitArgs = value.split(":", limit = 2)
        val id =
            splitArgs[0].toIntOrNull()
                ?: throw SerializationException("Invalid product id: ${splitArgs[0]}")
        val weight =
            splitArgs[1].toDoubleOrNull()
                ?: throw SerializationException("Invalid weight: ${splitArgs[1]}")
        return Hat(id, weight)
    }

    override fun serialize(value: Hat) = "${value.id}:${value.weight}"
}

@Composable
fun NonPrimitiveArgFlattenedSample() {
    val matcher =
        UriDeepLinkMatcher(
            uriPattern =
                DeepLinkUri(
                    "http://www.nav3example.com/profile?firstName={firstName}&lastName={lastName}&age={age}"
                ),
            serializer = serializer<ProfileKey>(),
        )

    val request =
        DeepLinkRequest(
            uri =
                DeepLinkUri("http://www.nav3example.com/profile?firstName=john&lastName=doe&age=25")
        )
    val startKey = matcher.match(request)?.key ?: HomeKey
    val backStack = rememberNavBackStack(startKey)

    NavDisplay(
        backStack,
        onBack = { backStack.removeLastOrNull() },
        entryProvider =
            entryProvider {
                entry<HomeKey> { RedBox("Deeplink failed. Displaying default Home screen") }
                entry<ProfileKey> { GreenBox(it.toString()) }
            },
    )
}

@Composable
fun DeepLinkSerializerSample() {
    val matcher =
        UriDeepLinkMatcher(
            uriPattern =
                DeepLinkUri("http://www.nav3example.com/products?product={product}&count={count}"),
            serializer = serializer<ProductInventoryKey>(),
        )

    val request =
        DeepLinkRequest(
            uri = DeepLinkUri("http://www.nav3example.com/products?product=123-productA&count=12")
        )
    val startKey = matcher.match(request)?.key ?: HomeKey
    val backStack = rememberNavBackStack(startKey)

    NavDisplay(
        backStack,
        onBack = { backStack.removeLastOrNull() },
        entryProvider =
            entryProvider {
                entry<HomeKey> { RedBox("Deeplink failed. Displaying default Home screen") }
                entry<ProductInventoryKey> { GreenBox(it.toString()) }
            },
    )
}

@Composable
fun ListTypeDeepLinkSerializerSample() {
    val matcher =
        UriDeepLinkMatcher(
            uriPattern = DeepLinkUri("http://www.nav3example.com/hatList?hats={hats}"),
            serializer = serializer<HatListKey>(),
        )

    val request =
        DeepLinkRequest(
            uri = DeepLinkUri("http://www.nav3example.com/hatList?hats=2342:4.5&hats=2643:5.6")
        )
    val startKey = matcher.match(request)?.key ?: HomeKey
    val backStack = rememberNavBackStack(startKey)

    NavDisplay(
        backStack,
        onBack = { backStack.removeLastOrNull() },
        entryProvider =
            entryProvider {
                entry<HomeKey> { RedBox("Deeplink failed. Displaying default Home screen") }
                entry<HatListKey> { GreenBox(it.toString()) }
            },
    )
}

// ---- ComplexBackStackSamples.kt ----

@Composable
fun MultipleBackStackSample() {
    val tabKeys = listOf(Home, User)

    /** set up User entries */
    val homeBackStack = rememberNavBackStack(Home)
    val homeTabEntries =
        getHomeTabEntries(
            backStackString = homeBackStack.map { (it as SampleKey).name }.toString(),
            backStack = homeBackStack,
        )

    /** set up User entries */
    val userBackStack = rememberNavBackStack(User)
    val userTabEntries =
        getUserTabEntries(
            backStackString = userBackStack.map { (it as SampleKey).name }.toString(),
            backStack = userBackStack,
        )

    /** condition for which tab to switch to */
    var currentTab: SampleTabKey by remember { mutableStateOf(Home) }

    Scaffold(
        bottomBar = {
            NavigationBar {
                tabKeys.forEach { tab ->
                    val isSelected = tab == currentTab
                    NavigationBarItem(
                        selected = isSelected,
                        onClick = { currentTab = tab },
                        icon = { Icon(imageVector = tab.imageVector, contentDescription = null) },
                    )
                }
            }
        }
    ) {
        /** Swap backStacks based on the current selected tab */
        NavDisplay(
            entries = if (currentTab == Home) homeTabEntries else userTabEntries,
            onBack = {
                val currBackStack = if (currentTab == Home) homeBackStack else userBackStack
                currBackStack.removeLastOrNull()
            },
        )
    }
}

@Composable
fun ConcatenatedBackStackSample() {
    val tabKeys = listOf(Home, User)

    /** set up User entries */
    val homeBackStack = rememberNavBackStack(Home)
    val homeBackStackNames = homeBackStack.map { (it as SampleKey).name }
    val homeTabEntries =
        getHomeTabEntries(
            backStackString = homeBackStackNames.toString(),
            backStack = homeBackStack,
        )

    /** set up User entries */
    val userBackStack = rememberNavBackStack(User)
    val userTabEntries =
        getUserTabEntries(
            backStackString =
                (homeBackStackNames + userBackStack.map { (it as SampleKey).name }).toString(),
            backStack = userBackStack,
        )

    /** condition for which tab to switch to */
    var currentTab: SampleTabKey by remember { mutableStateOf(Home) }

    Scaffold(
        bottomBar = {
            NavigationBar {
                tabKeys.forEach { tab ->
                    val isSelected = tab == currentTab
                    NavigationBarItem(
                        selected = isSelected,
                        onClick = { currentTab = tab },
                        icon = { Icon(imageVector = tab.imageVector, contentDescription = null) },
                    )
                }
            }
        }
    ) {
        /** Swap backStacks based on the current selected tab */
        NavDisplay(
            entries = if (currentTab == Home) homeTabEntries else homeTabEntries + userTabEntries,
            onBack = {
                val currBackStack = if (currentTab == Home) homeBackStack else userBackStack
                currBackStack.removeLastOrNull()
                if (currentTab == User && userBackStack.isEmpty()) {
                    currentTab = Home
                }
            },
        )
    }
}

@Composable
fun getHomeTabEntries(
    backStackString: String,
    backStack: NavBackStack<NavKey>,
): List<NavEntry<NavKey>> {
    if (backStack.isEmpty()) backStack.add(Home)
    val homeDecorators =
        listOf<NavEntryDecorator<NavKey>>(
            rememberSaveableStateHolderNavEntryDecorator(),
            rememberViewModelStoreNavEntryDecorator(),
        )
    val homeEntryProvider =
        entryProvider<NavKey> {
            entry<Home> {
                HomeScreen(backStackString = backStackString) {
                    backStack.add(Detail("in Home Tab"))
                }
            }
            entry<Detail> { key ->
                DetailScreen(
                    backStackString = "${backStack.map { (it as SampleKey).name }}",
                    sourceTab = key.sourceTab,
                )
            }
        }
    return rememberDecoratedNavEntries(backStack, homeDecorators, homeEntryProvider)
}

@Composable
fun getUserTabEntries(
    backStackString: String,
    backStack: NavBackStack<NavKey>,
): List<NavEntry<NavKey>> {
    if (backStack.isEmpty()) backStack.add(User)
    val userDecorators =
        listOf<NavEntryDecorator<NavKey>>(
            rememberSaveableStateHolderNavEntryDecorator(),
            rememberViewModelStoreNavEntryDecorator(),
        )
    val userEntryProvider =
        entryProvider<NavKey> {
            entry<User> {
                UserScreen(backStackString = backStackString) {
                    backStack.add(Detail("in User Tab"))
                }
            }
            entry<Detail> { key ->
                DetailScreen(backStackString = backStackString, sourceTab = key.sourceTab)
            }
        }
    return rememberDecoratedNavEntries(backStack, userDecorators, userEntryProvider)
}

private interface SampleKey : NavKey {
    val name: String
}

private interface SampleTabKey : SampleKey {
    val imageVector: ImageVector
}

@Serializable
private object Home : SampleTabKey {
    override val name: String = "Home"
    override val imageVector: ImageVector = Icons.Filled.HouseSiding
}

@Serializable
private object User : SampleTabKey {
    override val name: String = "User"
    override val imageVector: ImageVector = Icons.Filled.Person
}

@Serializable
data class Detail(val sourceTab: String) : SampleKey {
    override val name: String = "Detail"
}

// (BasicSamples.kt's screens of the samples above)

@Composable
fun HomeScreen(backStackString: String, onClick: () -> Unit) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = "Home Page")
        Text(text = "current backStack:$backStackString")
        NavigateButton("Detail", onClick)
    }
}

@Composable
fun UserScreen(backStackString: String, onClick: () -> Unit) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = "User Page")
        Text(text = "current backStack:$backStackString")
        NavigateButton("Detail", onClick)
    }
}

@Composable
fun DetailScreen(backStackString: String, sourceTab: String) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = "Detail Page $sourceTab")
        Text(text = "current backStack:$backStackString")
    }
}

// ---- AnimatedOverlaySamples.kt ----
// (KompozR's: its keys `NavKey`s where the sample's strategy takes `Any` — a `SceneStrategy<Any>` given a back stack
// of `NavKey`s, Kotlin's variance of the display's `T`, which Rust's types don't have —, and material3 1.5.0-alpha17's
// `rememberModalBottomSheetState()` where the sample's newer material3 has `rememberBottomSheetState(initialValue =
// SheetValue.Hidden)`, the same hidden modal sheet state)

@Serializable private object Landing : NavKey

@Serializable private object AnimatedBottomSheet : NavKey

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AnimatedBottomSheetSample() {
    val backStack = rememberNavBackStack(Landing)
    NavDisplay(
        backStack = backStack,
        onBack = { backStack.removeLastOrNull() },
        sceneStrategies = listOf(AnimatedBottomSheetSceneStrategy()),
        entryProvider =
            entryProvider {
                entry<Landing> {
                    Box(Modifier.fillMaxSize().background(Color.Green)) {
                        Button(onClick = { backStack.add(AnimatedBottomSheet) }) {
                            Text("Click to open bottom sheet")
                        }
                    }
                }
                entry<AnimatedBottomSheet>(
                    metadata = AnimatedBottomSheetSceneStrategy.bottomSheet()
                ) {
                    Column {
                        Text("BottomSheet")
                        Button(onClick = { backStack.removeLastOrNull() }) {
                            Text("Close BottomSheet")
                        }
                    }
                }
            },
    )
}

/** An [SceneStrategy] that renders a [NavEntry] within a [ModalBottomSheet]. */
@OptIn(ExperimentalMaterial3Api::class)
private class AnimatedBottomSheetSceneStrategy() : SceneStrategy<NavKey> {

    override fun SceneStrategyScope<NavKey>.calculateScene(entries: List<NavEntry<NavKey>>): Scene<NavKey>? {
        val entry = entries.lastOrNull() ?: return null
        entry.metadata[MetadataKey] ?: return null

        return object : OverlayScene<NavKey> {
            override val key = entry.contentKey
            override val entries = listOf(entry)
            override val previousEntries = entries.dropLast(1)
            override val overlaidEntries = entries.dropLast(1)

            lateinit var sheetState: SheetState

            override val content: @Composable (() -> Unit) = {
                sheetState = rememberModalBottomSheetState()
                val minHeight = LocalWindowInfo.current.containerSize.height * 0.2 // 50% height
                ModalBottomSheet(
                    onDismissRequest = onBack,
                    containerColor = Color.Blue,
                    sheetState = sheetState,
                    modifier = Modifier.heightIn(min = minHeight.dp),
                ) {
                    entry.Content()
                }
            }

            override suspend fun onRemove() {
                // run hide animations when this bottom sheet is popped from the backStack
                sheetState.hide()
            }
        }
    }

    companion object {
        object MetadataKey : NavMetadataKey<ModalBottomSheetProperties>

        fun bottomSheet(
            sheetProperties: ModalBottomSheetProperties = ModalBottomSheetProperties()
        ) = metadata { put(MetadataKey, sheetProperties) }
    }
}

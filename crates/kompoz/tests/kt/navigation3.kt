// Navigation 3 as a Compose app writes it: navigation3-ui 1.2's samples (OverlaySamples.kt's DialogSample,
// SceneStateSamples.kt's SceneStateSample, CustomSceneSamples.kt's transitions, ComplexBackStackSamples.kt's back
// stacks, BasicSamples.kt's SceneNav) and navigation3-runtime 1.2's (NavigationSamples.kt's screens).
//
// Kotlin's classes are stage 14's: what is declared as a class here is Rust beside this file (kotlin_navigation3.rs,
// D6) — the samples' `@Serializable object`/`data class` keys (NavKey), their Scene classes and strategies, the
// ViewModel. Android's `R.string`s are the texts they name.

package androidx.navigation3.ui.samples

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.BasicText
import androidx.compose.animation.*
import androidx.compose.animation.core.*
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.navigation3.rememberViewModelStoreNavEntryDecorator
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.window.DialogProperties
import androidx.navigation3.runtime.*
import androidx.navigation3.scene.*
import androidx.navigation3.ui.NavDisplay
import androidx.navigationevent.compose.NavigationBackHandler
import androidx.navigationevent.compose.rememberNavigationEventState

// ---- OverlaySamples.kt ----

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
                    Column {
                        BasicText("Overlaid Entry")
                        Button(onClick = { backStack.add(DialogNavKey) }, modifier = Modifier.testTag("open")) {
                            Text("Open Dialog")
                        }
                    }
                }
                entry<DialogNavKey>(metadata = metadata { put(DialogKey, DialogProperties()) }) {
                    BasicText("Dialog")
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

// ---- CustomSceneSamples.kt (its scenes and strategies are Rust twins; its slides of 5 s a function here: Kotlin's
// top-level `private val` is stage 14's) ----

// (the sample's keys `A` and `B` are `KeyA` and `KeyB`: a one-letter type's name is a type parameter to the
// generator's types)
@Composable
fun SceneDefaultTransitionsSample() {
    val backStack = rememberNavBackStack(KeyA)
    NavDisplay(
        backStack,
        onBack = { backStack.removeLastOrNull() },
        entryDecorators = listOf(rememberSaveableStateHolderNavEntryDecorator()),
        // NavDisplay default transitions slide vertically
        transitionSpec = { slideVertical() },
        popTransitionSpec = { slideVertical() },
        predictivePopTransitionSpec = { slideVertical() },
        // but the Scene provides default transitions that slide horizontally
        sceneStrategies = listOf(DefaultSceneTransitionsSceneStrategy()),
        entryProvider =
            entryProvider {
                entry<KeyA> { BlueBox("A") { backStack.add(KeyB) } }
                entry<KeyB> { RedBox("B") }
            },
    )
}

fun slideVertical(): ContentTransform =
    slideInVertically(tween(5000, easing = LinearOutSlowInEasing)) { it / 2 } togetherWith
        slideOutVertically(tween(5000, easing = LinearOutSlowInEasing)) { -it / 2 }

// (the sample's `onClick: (() -> Unit)?`: a nullable function type is stage 14's — here one always given)
@Composable
fun BlueBox(text: String, onClick: () -> Unit) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            BasicText(text)
            Button(onClick = onClick, modifier = Modifier.testTag("navigate")) { Text("Click to navigate") }
        }
    }
}

@Composable
fun RedBox(text: String) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { BasicText(text) }
}

// ---- BasicSamples.kt's SceneNav (NavigationSamples.kt's screens, their R.strings as texts) ----

@Composable
fun SceneNav() {
    val backStack = rememberNavBackStack(Profile)
    NavDisplay(
        backStack = backStack,
        entryDecorators =
            listOf(
                rememberSaveableStateHolderNavEntryDecorator(),
                rememberViewModelStoreNavEntryDecorator(),
            ),
        onBack = { backStack.removeAt(backStack.lastIndex) },
        entryProvider =
            // (the lambdas' `NavKey` parameters written: Kotlin infers them from NavDisplay's `T`, the Rust twin's
            // generic call can't — stage 14)
            entryProvider({ key: NavKey -> NavEntry(key) { Text(text = "Invalid Key") } }) {
                entry<Profile> {
                    val viewModel = viewModel<ProfileViewModel>()
                    ProfileScreen(viewModel, { key: NavKey -> backStack.add(key) }) {
                        backStack.removeAt(backStack.lastIndex)
                    }
                }
                entry<Dashboard> { dashboardArgs ->
                    val userId: String? = dashboardArgs.userId
                    DashboardScreen(userId, onBack = { backStack.removeAt(backStack.lastIndex) })
                }
            },
    )
}

// (the screens are `Profile` and `Dashboard` in the sample, beside its keys of those names: one name for a class and a
// function is Kotlin's, stage 14's here)
@Composable
fun ProfileScreen(viewModel: ProfileViewModel, navigateTo: (NavKey) -> Unit, onBack: () -> Unit) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = "${viewModel.name} Profile")
        NavigateButton("Dashboard") { navigateTo(Dashboard()) }
        NavigateBackButton(onBack)
    }
}

@Composable
fun DashboardScreen(title: String? = null, onBack: () -> Unit) {
    Column(Modifier.fillMaxSize().then(Modifier.padding(8.dp))) {
        Text(text = title ?: "Dashboard")
        NavigateBackButton(onBack)
    }
}

@Composable
fun NavigateButton(text: String, listener: () -> Unit = {}) {
    Button(onClick = listener, modifier = Modifier.fillMaxWidth().testTag(text)) {
        Text(text = "Navigate to $text")
    }
}

@Composable
fun NavigateBackButton(onBack: () -> Unit) {
    Button(onClick = onBack, modifier = Modifier.fillMaxWidth().testTag("back")) {
        Text(text = "Go to Previous screen")
    }
}

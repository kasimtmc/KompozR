// Back handling as a Compose app writes it: activity-compose 1.13's samples (BackHandlerSample.kt: BackHandler,
// PredictiveBack), navigationevent-compose 1.1's sample (RememberNavigationEventDispatcherOwnerSamples.kt), and
// navigationevent's composables as an app uses them (NavigationBackHandler with a hoisted state whose transition
// state an animation reads, NavigationEventHandler with forward, a scope of its own).
//
// Kotlin's classes are stage 14's: what is declared as a class here is Rust beside this file
// (kotlin_navigation_event.rs) — the sample's `class SampleCallbackHelper` and the app's `data class Screen :
// NavigationEventInfo()`. The samples' TextField is androidx.compose.material's (Material 2): Material 3's here.

package androidx.activity.compose.samples

import androidx.activity.BackEventCompat
import androidx.activity.compose.BackHandler
import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.navigationevent.NavigationEventInfo
import androidx.navigationevent.NavigationEventTransitionState
import androidx.navigationevent.compose.*
import kotlinx.coroutines.flow.Flow

// ---- BackHandlerSample.kt (its `BackHandler()` sample as `BackHandlerSample()`: in Rust one name is one function) ----

@Sampled
@Composable
fun BackHandlerSample() {
    var text by remember { mutableStateOf("") }

    TextField(value = text, onValueChange = { text = it }, modifier = Modifier.testTag("metin"))

    BackHandler(text.isNotEmpty()) {
        // handle back event
        text = ""
    }
}

// The sample's PredictiveBack catches the gesture's cancellation (`try { … } catch (e: CancellationException) {
// callback.cancelPop() }`): Kotlin's try is stage 14's (tools/stage14-pending.md), so here it is as written without
// it — a cancelled gesture's coroutine just ends, cancelPop isn't called.
/*
    PredictiveBackHandler(text.isNotEmpty()) { progress: Flow<BackEventCompat> ->
        callback.preparePop()
        try {
            progress.collect { backEvent -> callback.updateProgress(backEvent.progress) }
            callback.popBackStack()
        } catch (e: CancellationException) {
            callback.cancelPop()
        }
    }
*/
@Sampled
@Composable
fun PredictiveBack(callback: SampleCallbackHelper) {
    var text by remember { mutableStateOf("") }

    TextField(value = text, onValueChange = { text = it }, modifier = Modifier.testTag("metin"))

    PredictiveBackHandler(text.isNotEmpty()) { progress: Flow<BackEventCompat> ->
        callback.preparePop()
        progress.collect { backEvent -> callback.updateProgress(backEvent.progress) }
        callback.popBackStack()
    }
}

// ---- RememberNavigationEventDispatcherOwnerSamples.kt ----

@Sampled
@Composable
fun RememberNavigationEventDispatcherOwner() {
    val newOwner = rememberNavigationEventDispatcherOwner(enabled = true)
    CompositionLocalProvider(LocalNavigationEventDispatcherOwner provides newOwner) {
        // This content will now be part of the new navigation scope
        Text("kapsam")
    }
}

// ---- navigationevent's composables as an app uses them ----

// A detail screen closed by back: its state hoisted, the gesture's state read for the animation.
@Composable
fun Detail(onClose: () -> Unit) {
    val state = rememberNavigationEventState(currentInfo = Screen("detay"), backInfo = listOf(Screen("liste")))
    NavigationBackHandler(state = state, onBackCancelled = { }, onBackCompleted = onClose)
    // (the gesture's progress is `(transition as InProgress).latestEvent.progress` after an `is` check: Kotlin's `is`
    // is stage 14's — here whether one is under way)
    val idle = state.transitionState == NavigationEventTransitionState.Idle
    Text(if (idle) "detay" else "detay kapanıyor")
}

// Pages with back and forward, and a part of the screen with its own scope (disabled: its handler is skipped).
@Composable
fun Pages(page: Int, onPage: (Int) -> Unit, scoped: Boolean) {
    val state = rememberNavigationEventState(NavigationEventInfo.None)
    NavigationEventHandler(
        state = state,
        isForwardEnabled = page < 2,
        onForwardCompleted = { onPage(page + 1) },
        isBackEnabled = page > 0,
        onBackCompleted = { onPage(page - 1) },
    )
    Column {
        Text("sayfa $page")
        val owner = rememberNavigationEventDispatcherOwner(enabled = scoped)
        CompositionLocalProvider(LocalNavigationEventDispatcherOwner provides owner) {
            val inner = rememberNavigationEventState(NavigationEventInfo.None)
            NavigationBackHandler(state = inner) { onPage(-1) }
        }
    }
}

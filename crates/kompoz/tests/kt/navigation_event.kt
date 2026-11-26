// Back handling as a Compose app writes it: activity-compose 1.13's samples (BackHandlerSample.kt: BackHandler,
// PredictiveBack, SampleCallbackHelper), navigationevent-compose 1.1's sample
// (RememberNavigationEventDispatcherOwnerSamples.kt), navigationevent's composables as an app uses them
// (NavigationBackHandler with a hoisted state whose transition state an animation reads, NavigationEventHandler with
// forward, a scope of its own) and a handler of its own as activity-compose's BackHandlerCompat writes one (`object :
// NavigationEventHandler<NavigationEventInfo>(info, isBackEnabled = false) { … }`). The samples' TextField is
// androidx.compose.material's (Material 2): Material 3's here.

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
import androidx.navigationevent.NavigationEvent
import androidx.navigationevent.NavigationEventHandler
import kotlin.coroutines.cancellation.CancellationException
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

@Sampled
@Composable
fun PredictiveBack(callback: SampleCallbackHelper) {
    var text by remember { mutableStateOf("") }

    TextField(value = text, onValueChange = { text = it }, modifier = Modifier.testTag("metin"))

    PredictiveBackHandler(text.isNotEmpty()) { progress: Flow<BackEventCompat> ->
        callback.preparePop()
        try {
            progress.collect { backEvent -> callback.updateProgress(backEvent.progress) }
            callback.popBackStack()
        } catch (e: CancellationException) {
            callback.cancelPop()
        }
    }
}

public class SampleCallbackHelper(
    val preparePop: () -> Unit,
    val updateProgress: (progress: Float) -> Unit,
    val popBackStack: () -> Unit,
    val cancelPop: () -> Unit,
)

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

data class Screen(val name: String) : NavigationEventInfo()

// A detail screen closed by back: its state hoisted, the gesture's state read for the animation.
@Composable
fun Detail(onClose: () -> Unit) {
    val state = rememberNavigationEventState(currentInfo = Screen("detay"), backInfo = listOf(Screen("liste")))
    NavigationBackHandler(state = state, onBackCancelled = { }, onBackCompleted = onClose)
    val transition = state.transitionState
    Text(if (transition is NavigationEventTransitionState.InProgress) "detay kapanıyor ${transition.latestEvent.progress}" else "detay")
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

// ---- a handler of the app's own, as activity-compose's BackHandlerCompat writes its one ----

// Its info of any type (`NavigationEventHandler<NavigationEventInfo>`), back disabled until enabled; its overrides
// telling the log.
fun loggingHandler(info: NavigationEventInfo, log: MutableState<String>): NavigationEventHandler<NavigationEventInfo> =
    object : NavigationEventHandler<NavigationEventInfo>(info, isBackEnabled = false) {
        override fun onBackStarted(event: NavigationEvent) {
            log.value += "başla ${event.progress} "
        }

        override fun onBackProgressed(event: NavigationEvent) {
            log.value += "ilerle ${event.progress} "
        }

        override fun onBackCompleted() {
            log.value += "bitti "
        }

        override fun onBackCancelled() {
            log.value += "vazgeçti "
        }
    }

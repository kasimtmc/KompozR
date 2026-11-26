// Lifecycles as a Compose app writes them: the C++ Kompoz's KitchenForms (kompozable_tests.cpp, whole: a ViewModel's
// flow's collectAsStateWithLifecycle() as a member, Lifecycle.State names from LocalLifecycleOwner.current, a channel's
// and a producer's Kotlin forms) and
// lifecycle-runtime-compose 2.11's samples (LifecycleComposeSamples.kt, DropUnlessLifecycleSamples.kt,
// RememberLifecycleOwnerSamples.kt).
//
// The samples' local classes, private interfaces and local composables as written.

package androidx.lifecycle.compose.samples

import androidx.compose.foundation.layout.*
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.*
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.compose.*
import kotlinx.coroutines.channels.*
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.launch

// ---- KitchenForms (kompozable_tests.cpp 3186-3226) ----

// Whole, as the C++ test writes it: a Channel's trySend(…).isSuccess, consumeEach { order -> } suspending per element,
// scope.produce<Int> { send(i) }, the ViewModel's flow's collectAsStateWithLifecycle() as a member, Lifecycle.State
// names from LocalLifecycleOwner.current, a ViewConfiguration's timeouts in millis, semantics { contentType = … },
// SharingStarted.WhileSubscribed(5000).
class TickViewModel : ViewModel() {
    val ticks: StateFlow<Int> = flow<Int> {
        var i = 0
        while (true) {
            emit(i++)
            delay(100)
        }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), 0)
}

@Composable
fun KitchenForms() {
    val orders = remember { Channel<Int>(2) }
    var served by remember { mutableStateOf(listOf<Int>()) }
    var refused by remember { mutableStateOf(0) }
    var countdown by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()
    val vm = viewModel<TickViewModel>()
    val ticks by vm.ticks.collectAsStateWithLifecycle()
    val state by LocalLifecycleOwner.current.lifecycle.currentStateAsState()
    val gestures = LocalViewConfiguration.current
    LaunchedEffect(Unit) {
        orders.consumeEach { order ->
            delay(10)
            served = served + order
        }
    }
    Column(modifier = Modifier.padding(PaddingValues(12.dp, 8.dp))) {
        Box(modifier = Modifier.size(10.dp).semantics { contentType = ContentType.Username }.testTag("alan"))
        Button(onClick = { if (!orders.trySend(served.size + 1).isSuccess) refused++ }, modifier = Modifier.testTag("ver")) { Text("sipariş") }
        Button(onClick = {
            scope.launch {
                val numbers = scope.produce<Int> { for (i in 3 downTo 1) send(i) }
                numbers.consumeEach { countdown = countdown + it.toString() }
                countdown = countdown + "!"
            }
        }) { Text("say") }
        Text("servis ${served.size} · ret $refused · $countdown", modifier = Modifier.testTag("durum"))
        Text("durum ${if (state == Lifecycle.State.RESUMED || state == Lifecycle.State.STARTED) "açık" else "kapalı"} · ${gestures.longPressTimeoutMillis} ms · tik ${if (ticks >= 0) "var" else "yok"}")
    }
}

// ---- LifecycleComposeSamples.kt ----

@Sampled
@Composable
fun StateFlowCollectAsStateWithLifecycle() {
    class ExampleState {
        private val _uiState = MutableStateFlow("")
        val uiState: StateFlow<String> = _uiState.asStateFlow()
    }

    val state = remember { ExampleState() }

    val uiState by state.uiState.collectAsStateWithLifecycle()
    Text(text = uiState)
}

@Sampled
@Composable
fun FlowCollectAsStateWithLifecycle() {
    class ExampleState {
        val counter = flow {
            var count = 0
            while (true) {
                emit(count++)
                delay(1000)
            }
        }
    }

    val state = remember { ExampleState() }
    val count by state.counter.collectAsStateWithLifecycle(initialValue = 0)
    Text(text = "$count")
}

private interface TimeAnalytics {
    fun stopTimeTracking(): TimeAnalytics
}

private interface DataAnalytics {
    fun trackScreenView(screenName: String)

    fun startTimeTracking(): TimeAnalytics

    fun sendDisposalAnalytics(timeAnalytics: TimeAnalytics)
}

@Sampled
@Composable
fun lifecycleEventEffectSample() {
    @Composable
    fun Analytics(dataAnalytics: DataAnalytics) {
        LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { dataAnalytics.trackScreenView("screen1") }

        // ...
    }
}

@Sampled
@Composable
fun lifecycleStartEffectSample() {
    @Composable
    fun Analytics(dataAnalytics: DataAnalytics) {
        LifecycleStartEffect(dataAnalytics) {
            val timeTracker = dataAnalytics.startTimeTracking()

            onStopOrDispose { timeTracker.stopTimeTracking() }
        }

        // ...
    }
}

@Sampled
@Composable
fun lifecycleResumeEffectSample() {
    @Composable
    fun Analytics(dataAnalytics: DataAnalytics) {
        LifecycleResumeEffect(dataAnalytics) {
            val timeTracker = dataAnalytics.startTimeTracking()

            onPauseOrDispose { timeTracker.stopTimeTracking() }
        }

        // ...
    }
}

// ---- DropUnlessLifecycleSamples.kt ----

@Sampled
@Composable
fun DropUnlessStarted() {
    Button(
        onClick =
            dropUnlessStarted {
                // Normally all actions in a composable are permitted, but the dropUnlessStarted
                // will only run the action if the lifecycle is at least STARTED.
            }
    ) {
        Text(text = "Navigate to next screen")
    }
}

@Sampled
@Composable
fun DropUnlessResumed() {
    Button(
        onClick =
            dropUnlessResumed {
                // Normally all actions in a composable are permitted, but the dropUnlessResumed
                // will only run the action if the lifecycle is at least RESUMED.
            }
    ) {
        Text(text = "Navigate to next screen")
    }
}

// ---- RememberLifecycleOwnerSamples.kt ----

@Composable
fun LifecycleAwareText() {
    val lifecycleState = LocalLifecycleOwner.current.lifecycle.currentStateAsState()
    Text(text = "Current lifecycle state is ${lifecycleState.value}")
}

@Sampled
@Composable
fun ComposableWithMaxLifecycle() {
    val cappedLifecycleOwner = rememberLifecycleOwner(maxLifecycle = Lifecycle.State.STARTED)
    CompositionLocalProvider(LocalLifecycleOwner provides cappedLifecycleOwner) {
        LifecycleAwareText()
    }
}

@Sampled
@Composable
fun ComposableWithParentNull() {
    val detachedLifecycleOwner =
        rememberLifecycleOwner(parent = null, maxLifecycle = Lifecycle.State.STARTED)
    CompositionLocalProvider(LocalLifecycleOwner provides detachedLifecycleOwner) {
        LifecycleAwareText()
    }
}

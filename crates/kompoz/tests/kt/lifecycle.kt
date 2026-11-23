// Lifecycles as a Compose app writes them: the C++ Kompoz's KitchenForms (kompozable_tests.cpp, its lifecycle lines:
// a flow's collectAsStateWithLifecycle() as a member, Lifecycle.State names from LocalLifecycleOwner.current) and
// lifecycle-runtime-compose 2.11's samples (LifecycleComposeSamples.kt, DropUnlessLifecycleSamples.kt,
// RememberLifecycleOwnerSamples.kt).
//
// Kotlin's classes are stage 14's: what the sources declare as classes is Rust beside this file
// (kotlin_lifecycle.rs) — the samples' local `class ExampleState` (two of them: the second is `CounterState` there)
// and their private `interface DataAnalytics` / `TimeAnalytics`, and KitchenForms' `class TickViewModel :
// ViewModel()` (`viewModel<TickViewModel>()` is stage 13 WP1's: its ticks are given to KitchenForms here).

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
import androidx.lifecycle.compose.*
import kotlinx.coroutines.channels.*
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.launch

// ---- KitchenForms (kompozable_tests.cpp 3186-3226) ----

// Its lifecycle lines, in their Column (the others — a Channel's trySend(…).isSuccess, scope.produce<Int> { }, a
// suspending consumeEach { } — are the coroutines' Kotlin forms: tools/stage13-followups.md, WP0).
@Composable
fun KitchenForms(ticksOfViewModel: StateFlow<Int>) {
    val ticks by ticksOfViewModel.collectAsStateWithLifecycle()
    val state by LocalLifecycleOwner.current.lifecycle.currentStateAsState()
    val gestures = LocalViewConfiguration.current
    Column(modifier = Modifier.padding(PaddingValues(12.dp, 8.dp))) {
        Box(modifier = Modifier.size(10.dp).semantics { contentType = ContentType.Username }.testTag("alan"))
        Text("durum ${if (state == Lifecycle.State.RESUMED || state == Lifecycle.State.STARTED) "açık" else "kapalı"} · ${gestures.longPressTimeoutMillis} ms · tik ${if (ticks >= 0) "var" else "yok"}")
    }
}

// ---- LifecycleComposeSamples.kt ----

@Sampled
@Composable
fun StateFlowCollectAsStateWithLifecycle() {
    val state = remember { ExampleState() }

    val uiState by state.uiState.collectAsStateWithLifecycle()
    Text(text = uiState)
}

@Sampled
@Composable
fun FlowCollectAsStateWithLifecycle() {
    val state = remember { CounterState() }

    val count by state.counter.collectAsStateWithLifecycle(initialValue = 0)
    Text(text = "$count")
}

// lifecycleEventEffectSample, lifecycleStartEffectSample and lifecycleResumeEffectSample declare their Analytics as
// local functions (stage 14's); verbatim here, their Analytics as one top-level composable (ScreenAnalytics) in
// kotlin_lifecycle.rs.
/*
@Sampled
@Composable
fun lifecycleEventEffectSample() {
    @Composable
    fun Analytics(dataAnalytics: DataAnalytics) {
        LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { dataAnalytics.trackScreenView("screen1") }
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
    }
}
*/

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

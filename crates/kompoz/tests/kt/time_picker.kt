// Material's time pickers as Compose's samples write them (material3 1.5's TimePickerSamples.kt): the dial picker in
// a dialog and the time input in a dialog, the time they show written by java.text's `SimpleDateFormat` in the
// default locale from java.util's `Calendar` (the kt shims': CLDR's AM/PM markers through ICU4X; `Locale` is
// java.util's by the import, as in Kotlin).
//
// Not here, with the reason (its twin, TimePickerSwitchableSampleTwin in kotlin_pickers.rs, is tested: the sample but
// its `LocalConfiguration` lines, the window's height in dp read instead):
// - TimePickerSwitchableSample, the dialog switching between the two: Android's `LocalConfiguration.current
//   .screenHeightDp` decides whether the dial fits (`androidx.compose.ui.platform.LocalConfiguration`, androidMain
//   only: Compose for desktop and the C++ Kompoz have none) — the Android platform, stage 15. It waits in the
//   comment below, as written.

import androidx.compose.material3.TimePickerDialogDefaults.MinHeightForTimePicker
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Sampled
@Composable
@Preview
fun TimePickerSample() {
    var showTimePicker by remember { mutableStateOf(false) }
    val state = rememberTimePickerState()
    val formatter = remember { SimpleDateFormat("hh:mm a", Locale.getDefault()) }
    val snackState = remember { SnackbarHostState() }
    val snackScope = rememberCoroutineScope()

    Box(propagateMinConstraints = false) {
        Button(modifier = Modifier.align(Alignment.Center), onClick = { showTimePicker = true }) {
            Text("Set Time")
        }
        SnackbarHost(hostState = snackState)
    }

    if (showTimePicker) {
        TimePickerDialog(
            title = { TimePickerDialogDefaults.Title(displayMode = TimePickerDisplayMode.Picker) },
            onDismissRequest = { showTimePicker = false },
            confirmButton = {
                TextButton(
                    enabled = state.isInputValid,
                    onClick = {
                        state.isHourInputValid
                        val cal = Calendar.getInstance()
                        cal.set(Calendar.HOUR_OF_DAY, state.hour)
                        cal.set(Calendar.MINUTE, state.minute)
                        cal.isLenient = false
                        snackScope.launch {
                            snackState.showSnackbar("Entered time: ${formatter.format(cal.time)}")
                        }
                        showTimePicker = false
                    },
                ) {
                    Text("Ok")
                }
            },
            dismissButton = { TextButton(onClick = { showTimePicker = false }) { Text("Cancel") } },
            modeToggleButton = {},
        ) {
            TimePicker(state = state)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Sampled
@Composable
@Preview
fun TimeInputSample() {
    var showTimePicker by remember { mutableStateOf(false) }
    val state = rememberTimePickerState()
    val formatter = remember { SimpleDateFormat("hh:mm a", Locale.getDefault()) }
    val snackState = remember { SnackbarHostState() }
    val snackScope = rememberCoroutineScope()

    Box(propagateMinConstraints = false) {
        Button(modifier = Modifier.align(Alignment.Center), onClick = { showTimePicker = true }) {
            Text("Set Time")
        }
        SnackbarHost(hostState = snackState)
    }

    if (showTimePicker) {
        TimePickerDialog(
            title = { TimePickerDialogDefaults.Title(displayMode = TimePickerDisplayMode.Input) },
            onDismissRequest = { showTimePicker = false },
            confirmButton = {
                TextButton(
                    onClick = {
                        val cal = Calendar.getInstance()
                        cal.set(Calendar.HOUR_OF_DAY, state.hour)
                        cal.set(Calendar.MINUTE, state.minute)
                        cal.isLenient = false
                        snackScope.launch {
                            snackState.showSnackbar("Entered time: ${formatter.format(cal.time)}")
                        }
                        showTimePicker = false
                    }
                ) {
                    Text("Ok")
                }
            },
            dismissButton = { TextButton(onClick = { showTimePicker = false }) { Text("Cancel") } },
            modeToggleButton = {},
        ) {
            TimeInput(state = state)
        }
    }
}

/*
@OptIn(ExperimentalMaterial3Api::class)
@Sampled
@Composable
@Preview
fun TimePickerSwitchableSample() {
    var showTimePicker by remember { mutableStateOf(false) }
    val state = rememberTimePickerState()
    val formatter = remember { SimpleDateFormat("hh:mm a", Locale.getDefault()) }
    val snackState = remember { SnackbarHostState() }
    var displayMode by remember { mutableStateOf(TimePickerDisplayMode.Picker) }
    val snackScope = rememberCoroutineScope()
    val configuration = LocalConfiguration.current

    Box(propagateMinConstraints = false) {
        Button(modifier = Modifier.align(Alignment.Center), onClick = { showTimePicker = true }) {
            Text("Set Time")
        }
        SnackbarHost(hostState = snackState)
    }

    if (showTimePicker) {
        TimePickerDialog(
            title = { TimePickerDialogDefaults.Title(displayMode = displayMode) },
            onDismissRequest = { showTimePicker = false },
            confirmButton = {
                TextButton(
                    onClick = {
                        val cal = Calendar.getInstance()
                        cal.set(Calendar.HOUR_OF_DAY, state.hour)
                        cal.set(Calendar.MINUTE, state.minute)
                        cal.isLenient = false
                        snackScope.launch {
                            snackState.showSnackbar("Entered time: ${formatter.format(cal.time)}")
                        }
                        showTimePicker = false
                    }
                ) {
                    Text("Ok")
                }
            },
            dismissButton = { TextButton(onClick = { showTimePicker = false }) { Text("Cancel") } },
            modeToggleButton = {
                if (configuration.screenHeightDp.dp > MinHeightForTimePicker) {
                    TimePickerDialogDefaults.DisplayModeToggle(
                        onDisplayModeChange = {
                            displayMode =
                                if (displayMode == TimePickerDisplayMode.Picker) {
                                    TimePickerDisplayMode.Input
                                } else {
                                    TimePickerDisplayMode.Picker
                                }
                        },
                        displayMode = displayMode,
                    )
                }
            },
        ) {
            if (
                displayMode == TimePickerDisplayMode.Picker &&
                    configuration.screenHeightDp.dp > MinHeightForTimePicker
            ) {
                TimePicker(state = state)
            } else {
                TimeInput(state = state)
            }
        }
    }
}
*/

// Time pickers as Compose writes them (WP6): an object's member imported by name and read by it
// (`import …TimePickerDialogDefaults.MinHeightForTimePicker`, as TimePickerSamples does; a companion's member under
// another name), a state's properties and extension properties (`state.hour`, `state.isPm`), its properties set.
import androidx.compose.material3.TimePickerDialogDefaults.MinHeightForTimePicker
import androidx.compose.material3.TimePickerSelectionMode.Companion.Minute as MinuteMode

@Composable
fun TimePickers(height: Dp) {
    val state = rememberTimePickerState(initialHour = 9, is24Hour = true)
    Column {
        if (height > MinHeightForTimePicker) {
            TimePicker(state = state, layoutType = TimePickerLayoutType.Vertical)
        } else {
            TimeInput(state)
        }
        Text("${state.hour} ${state.isPm} ${state.isInputValid} ${state.selection}")
        Button(onClick = { state.selection = MinuteMode; state.hour = 12 }) { Text("Minutes") }
    }
}

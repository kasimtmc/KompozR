// The date pickers' forms (WP6): a Kotlin `IntRange` given to a parameter Rust holds as a `RangeInclusive`
// (`yearRange = 2000..2030`), `a ?: "text"` of a value that isn't text, `LocalDate`s' `a..b` (a `ClosedRange`)
// printed, java.time's `LocalDate.of`, the state's properties and its JVM extension functions.

@Composable
fun YearsAndElvis(month: Long) {
    val state = rememberDatePickerState(initialDisplayedMonthMillis = month, yearRange = 2000..2030)
    DatePicker(state = state)
    Text("picked ${state.selectedDateMillis ?: "none"} in ${state.yearRange}")
    TextButton(onClick = { state.displayMode = DisplayMode.Input; state.selectedDateMillis = null }) { Text("type") }
}

@Composable
fun DateRange() {
    val state = rememberDateRangePickerState(
        initialSelectedStartDate = LocalDate.of(2023, 4, 15),
        initialSelectedEndDate = LocalDate.of(2023, 4, 20),
    )
    val start = state.getSelectedStartDate()
    if (start != null) {
        val range = start..state.getSelectedEndDate()!!
        Text("range $range from ${start.dayOfWeek}")
    }
    Text("${state.getSelectedEndDate() ?: "open"}")
}

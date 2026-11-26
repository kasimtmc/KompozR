// Material's date pickers as Compose's samples write them (material3 1.5's DatePickerSamples.kt): the date picker,
// its java.time form, its dialog, its input mode, the date range picker and its java.time form.
//
// Not here, with the reason — the Android platform, stage 15 (their Rust twins are tested in
// crates/kompoz-material3/tests/date_picker.rs, their Kotlin twins in kotlin_pickers.rs):
// - DatePickerWithDateSelectableDatesSample: its `object : SelectableDates { … }` and its JVM classes (java.time's
//   `Instant`/`ZoneId`, java.util's `Calendar`/`TimeZone`) are translated, but Android's `Build.VERSION.SDK_INT >=
//   Build.VERSION_CODES.O` (android.os.Build) chooses between its two ways to the day of the week. Its Kotlin twin
//   is the sample with that check a parameter, both ways tested.
// - DatePickerCustomLocaleSample, DatePickerCustomLocaleApi26Sample: Android's `LocaleList.forLanguageTags`,
//   `Configuration`, `LocalContext.current.createConfigurationContext` and `LocalConfiguration` (android.os,
//   android.content.res, androidMain's composition locals) give the state its Hebrew locale. Their Kotlin twins are
//   the samples with `Locale.forLanguageTag("HE")` (java.util's, Compose for desktop's `CalendarLocale`) instead,
//   right to left.

@Preview
@Sampled
@Composable
fun DatePickerSample() {
    Column(
        modifier = Modifier.verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        // Pre-select a date for January 4, 2020
        val datePickerState = rememberDatePickerState(initialSelectedDateMillis = 1578096000000)
        DatePicker(state = datePickerState, modifier = Modifier.padding(16.dp))

        Text(
            "Selected date timestamp: ${datePickerState.selectedDateMillis ?: "no selection"}",
            modifier = Modifier.align(Alignment.CenterHorizontally),
        )
    }
}

@RequiresApi(Build.VERSION_CODES.O)
@Preview
@Sampled
@Composable
fun DatePickerApi26Sample() {
    Column(
        modifier = Modifier.verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        // Pre-select a date for April 15, 2023
        val datePickerState =
            rememberDatePickerState(initialSelectedDate = LocalDate.of(2023, 4, 15))
        DatePicker(state = datePickerState, modifier = Modifier.padding(16.dp))

        Text(
            "Selected date: ${datePickerState.getSelectedDate() ?: "no selection"}",
            modifier = Modifier.align(Alignment.CenterHorizontally),
        )
    }
}

@Preview
@Sampled
@Composable
fun DatePickerDialogSample() {
    // Decoupled snackbar host state from scaffold state for demo purposes.
    val snackState = remember { SnackbarHostState() }
    val snackScope = rememberCoroutineScope()
    SnackbarHost(hostState = snackState, Modifier)
    val openDialog = remember { mutableStateOf(true) }
    // TODO demo how to read the selected date from the state.
    if (openDialog.value) {
        val datePickerState = rememberDatePickerState()
        val confirmEnabled = remember {
            derivedStateOf { datePickerState.selectedDateMillis != null }
        }
        DatePickerDialog(
            onDismissRequest = {
                // Dismiss the dialog when the user clicks outside the dialog or on the back
                // button. If you want to disable that functionality, simply use an empty
                // onDismissRequest.
                openDialog.value = false
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        openDialog.value = false
                        snackScope.launch {
                            snackState.showSnackbar(
                                "Selected date timestamp: ${datePickerState.selectedDateMillis}"
                            )
                        }
                    },
                    enabled = confirmEnabled.value,
                ) {
                    Text("OK")
                }
            },
            dismissButton = {
                TextButton(onClick = { openDialog.value = false }) { Text("Cancel") }
            },
        ) {
            // The verticalScroll will allow scrolling to show the entire month in case there is not
            // enough horizontal space (for example, when in landscape mode).
            // Note that it's still currently recommended to use a DisplayMode.Input at the state in
            // those cases.
            DatePicker(
                state = datePickerState,
                modifier = Modifier.verticalScroll(rememberScrollState()),
            )
        }
    }
}

@Preview
@Sampled
@Composable
fun DateInputSample() {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        val state = rememberDatePickerState(initialDisplayMode = DisplayMode.Input)
        DatePicker(state = state, modifier = Modifier.padding(16.dp))

        Text(
            "Entered date timestamp: ${state.selectedDateMillis ?: "no input"}",
            modifier = Modifier.align(Alignment.CenterHorizontally),
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Preview
@Sampled
@Composable
fun DateRangePickerSample() {
    // Decoupled snackbar host state from scaffold state for demo purposes.
    val snackState = remember { SnackbarHostState() }
    val snackScope = rememberCoroutineScope()
    SnackbarHost(hostState = snackState, Modifier.zIndex(1f))

    val state = rememberDateRangePickerState()
    Column(modifier = Modifier.fillMaxSize(), verticalArrangement = Arrangement.Top) {
        // Add a row with "Save" and dismiss actions.
        Row(
            modifier =
                Modifier.fillMaxWidth()
                    .background(DatePickerDefaults.colors().containerColor)
                    .padding(start = 12.dp, end = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            TooltipBox(
                positionProvider =
                    TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
                tooltip = { PlainTooltip { Text("Close") } },
                state = rememberTooltipState(),
            ) {
                IconButton(onClick = { /* dismiss the UI */ }) {
                    Icon(Icons.Filled.Close, contentDescription = "Close")
                }
            }
            TextButton(
                onClick = {
                    snackScope.launch {
                        val range = state.selectedStartDateMillis!!..state.selectedEndDateMillis!!
                        snackState.showSnackbar("Saved range (timestamps): $range")
                    }
                },
                enabled = state.selectedEndDateMillis != null,
            ) {
                Text(text = "Save")
            }
        }
        DateRangePicker(state = state, modifier = Modifier.weight(1f))
    }
}

@RequiresApi(Build.VERSION_CODES.O)
@OptIn(ExperimentalMaterial3Api::class)
@Preview
@Sampled
@Composable
fun DateRangePickerApi26Sample() {
    // Decoupled snackbar host state from scaffold state for demo purposes.
    val snackState = remember { SnackbarHostState() }
    val snackScope = rememberCoroutineScope()
    SnackbarHost(hostState = snackState, Modifier.zIndex(1f))

    // Creates a state with pre-selected date range.
    val state =
        rememberDateRangePickerState(
            initialSelectedStartDate = LocalDate.of(2023, 4, 15),
            initialSelectedEndDate = LocalDate.of(2023, 4, 20),
        )
    Column(modifier = Modifier.fillMaxSize(), verticalArrangement = Arrangement.Top) {
        // Add a row with "Save" and dismiss actions.
        Row(
            modifier =
                Modifier.fillMaxWidth()
                    .background(DatePickerDefaults.colors().containerColor)
                    .padding(start = 12.dp, end = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            TooltipBox(
                positionProvider =
                    TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above),
                tooltip = { PlainTooltip { Text("Close") } },
                state = rememberTooltipState(),
            ) {
                IconButton(onClick = { /* dismiss the UI */ }) {
                    Icon(Icons.Filled.Close, contentDescription = "Close")
                }
            }
            TextButton(
                onClick = {
                    snackScope.launch {
                        val range = state.getSelectedStartDate()!!..state.getSelectedEndDate()!!
                        snackState.showSnackbar("Saved range: $range")
                    }
                },
                enabled = state.getSelectedEndDate() != null,
            ) {
                Text(text = "Save")
            }
        }
        DateRangePicker(state = state, modifier = Modifier.weight(1f))
    }
}

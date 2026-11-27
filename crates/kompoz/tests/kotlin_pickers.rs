//! Material's date and time pickers in Kotlin's syntax, as a Compose user writes them: the C++ Kompoz's picker tests
//! written in Kotlin through `kompoz!` (its kompozable_tests.cpp has no picker source but SectionsForms' dialog), and
//! material3's DatePickerSamples and TimePickerSamples from `.kt` files (kompoz_file!).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, shortcut_modifier};
use kompoz::ui::window::PopupPositionProvider;

// ==== date pickers ===================================================================================================
// The C++ Kompoz's date picker tests (kompoz_tests.cpp: date_picker_picks_days_months_and_years,
// date_picker_takes_a_typed_date_in_input_mode, date_picker_dialog_confirms_the_date,
// date_range_picker_takes_typed_dates_in_input_mode, date_range_picker_picks_a_range) written in Kotlin, checked as
// crates/kompoz-material3/tests/date_picker.rs checks them (in English here; Compose's behaviour, the differences
// from the C++'s said there), and material3's DatePickerSamples from kt/date_picker.kt. "Today" is a test clock's.

use kompoz::material3::testing::set_today_for_tests;

const DAY: i64 = 86_400_000;
const AUG_1_2026: i64 = 20_666 * DAY;

kompoz! {
    @Composable
    fun DatePicking(month: Long) {
        val state = rememberDatePickerState(initialDisplayedMonthMillis = month)
        Column {
            DatePicker(state = state)
            Text("picked ${state.selectedDateMillis ?: "none"} shown ${state.displayedMonthMillis}")
        }
    }

    @Composable
    fun DateTyping(month: Long) {
        val state = rememberDatePickerState(initialDisplayedMonthMillis = month, yearRange = 2000..2030, initialDisplayMode = DisplayMode.Input)
        Column {
            DatePicker(state = state)
            Text("picked ${state.selectedDateMillis ?: "none"} mode ${state.displayMode}")
        }
    }

    @Composable
    fun DateDialog(start: Long) {
        var open by remember { mutableStateOf(true) }
        var confirmed by remember { mutableStateOf<Long?>(null) }
        Text("confirmed ${confirmed ?: "none"}")
        if (open) {
            val state = rememberDatePickerState(initialSelectedDateMillis = start)
            DatePickerDialog(
                onDismissRequest = { open = false },
                confirmButton = { TextButton(onClick = { confirmed = state.selectedDateMillis; open = false }) { Text("OK") } },
                dismissButton = { TextButton(onClick = { open = false }) { Text("Cancel") } },
            ) {
                DatePicker(state = state, modifier = Modifier.testTag("picker"))
            }
        }
    }

    @Composable
    fun RangeTyping(month: Long) {
        val state = rememberDateRangePickerState(initialDisplayedMonthMillis = month, initialDisplayMode = DisplayMode.Input)
        Column {
            DateRangePicker(state = state, modifier = Modifier.height(400.dp))
            Text("range ${state.selectedStartDateMillis ?: "none"} ${state.selectedEndDateMillis ?: "none"}")
        }
    }

    @Composable
    fun RangePicking(month: Long) {
        val state = rememberDateRangePickerState(initialDisplayedMonthMillis = month)
        Column {
            Text("range ${state.selectedStartDateMillis ?: "none"} ${state.selectedEndDateMillis ?: "none"}")
            DateRangePicker(state = state, modifier = Modifier.weight(1f))
        }
    }
}

fn date_rule(content: impl kompoz::runtime::Content + 'static) -> KompozTestRule {
    set_today_for_tests(Some(|| (2026, 8, 4)));
    KompozTestRule::with_viewport(content, IntSize::new(400, 800))
}

fn tap_at(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, kompoz::ui::input::PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(at, -1, 0);
    rule.advance_frame();
}

fn type_digits(rule: &KompozTestRule, field: &str, digits: &str) {
    use kompoz::ui::input::Key;
    let at = rule.on_node_with_content_description(field).get_unclipped_bounds_in_root().center();
    tap_at(rule, at);
    rule.host().key_down(Key::A, shortcut_modifier());
    rule.host().key_up(Key::A, shortcut_modifier());
    rule.wait_for_idle();
    rule.host().text_input(digits);
    rule.wait_for_idle();
}

fn date_picker_picks_days_months_and_years() {
    let rule = date_rule(kompozable_lambda!(|| DatePicking(AUG_1_2026)));
    rule.on_node_with_text("Today, Tuesday, August 4, 2026").assert_exists();
    rule.on_node_with_text("Monday, August 17, 2026").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(&format!("picked {} shown {AUG_1_2026}", AUG_1_2026 + 16 * DAY)).assert_exists();
    rule.on_node_with_content_description("Change to next month").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("September 2026").assert_exists();
    rule.on_node_with_text(&format!("picked {} shown {}", AUG_1_2026 + 16 * DAY, AUG_1_2026 + 31 * DAY))
        .assert_exists();
    rule.on_node_with_text("September 2026").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Navigate to year 2030").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("September 2030").assert_exists();
    set_today_for_tests(None);
}

fn date_picker_takes_a_typed_date_in_input_mode() {
    let rule = date_rule(kompozable_lambda!(|| DateTyping(AUG_1_2026)));
    rule.on_node_with_text("picked none mode Input").assert_exists();
    type_digits(&rule, "Date, MM/DD/YYYY", "08152026");
    rule.on_node_with_text(&format!("picked {} mode Input", AUG_1_2026 + 14 * DAY)).assert_exists();
    assert!(rule.drawn_texts().iter().any(|t| t == "08/15/2026"));
    type_digits(&rule, "Date, MM/DD/YYYY", "01012040");
    rule.on_node_with_text("picked none mode Input").assert_exists();
    assert!(rule.drawn_texts().join(" ").contains("Date out of expected year range 2000 - 2030"));
    rule.on_node_with_content_description("Switch to calendar input mode").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("picked none mode Picker").assert_exists();
    set_today_for_tests(None);
}

fn date_picker_dialog_confirms_the_date() {
    let march_5 = AUG_1_2026 - 149 * DAY;
    let rule = date_rule(kompozable_lambda!(move || DateDialog(march_5)));
    rule.on_node_with_text("Mar 5, 2026").assert_exists();
    let picker = rule.on_node_with_tag("picker").get_unclipped_bounds_in_root();
    assert!((picker.width() - 360.0).abs() < 0.5 && (picker.left - 20.0).abs() < 0.5, "{picker:?}");
    rule.on_node_with_text("Friday, March 20, 2026").perform_click();
    rule.on_node_with_text("OK").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(&format!("confirmed {}", march_5 + 15 * DAY)).assert_exists();
    assert_eq!(rule.host().overlay_count(), 0);
    set_today_for_tests(None);
}

fn date_range_picker_takes_typed_dates_in_input_mode() {
    let rule = date_rule(kompozable_lambda!(|| RangeTyping(AUG_1_2026)));
    type_digits(&rule, "Start date, MM/DD/YYYY", "08102026");
    rule.on_node_with_text(&format!("range {} none", AUG_1_2026 + 9 * DAY)).assert_exists();
    type_digits(&rule, "End date, MM/DD/YYYY", "08052026");
    assert!(rule.drawn_texts().join(" ").contains("Invalid date range input"));
    type_digits(&rule, "End date, MM/DD/YYYY", "08202026");
    rule.on_node_with_text(&format!("range {} {}", AUG_1_2026 + 9 * DAY, AUG_1_2026 + 19 * DAY)).assert_exists();
    set_today_for_tests(None);
}

fn date_range_picker_picks_a_range() {
    let rule = date_rule(kompozable_lambda!(|| RangePicking(AUG_1_2026)));
    rule.on_node_with_text("Wednesday, August 12, 2026").perform_click();
    rule.on_node_with_text("Thursday, August 20, 2026").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(&format!("range {} {}", AUG_1_2026 + 11 * DAY, AUG_1_2026 + 19 * DAY)).assert_exists();
    rule.on_node_with_text("In range, Saturday, August 15, 2026").assert_exists();
    rule.on_node_with_text("Saturday, August 1, 2026").perform_click(); // a whole range: a new start
    rule.wait_for_idle();
    rule.on_node_with_text("range 1785542400000 none").assert_exists();
    set_today_for_tests(None);
}

kompoz_file!("tests/kt/date_picker.kt");

// The Kotlin twins of DatePickerSamples' Android samples (in a `.kt` file: their nested string literals aren't Rust
// tokens).
kompoz_file!("tests/kt/date_picker_twins.kt");

/// The samples of kt/date_picker.kt, checked as their Rust twins are.
fn date_picker_samples() {
    set_today_for_tests(Some(|| (2023, 4, 3)));
    let rule = KompozTestRule::with_viewport(DatePickerSample, IntSize::new(400, 900));
    rule.on_node_with_text("January 2020").assert_exists();
    rule.on_node_with_text("Selected date timestamp: 1578096000000").assert_exists();
    let rule = KompozTestRule::with_viewport(DatePickerApi26Sample, IntSize::new(400, 900));
    rule.on_node_with_text("Selected date: 2023-04-15").assert_exists();
    rule.on_node_with_text("Friday, April 21, 2023").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Selected date: 2023-04-21").assert_exists();
    let rule = KompozTestRule::with_viewport(DatePickerDialogSample, IntSize::new(400, 900));
    rule.on_node_with_text("OK").assert_is_not_enabled();
    rule.on_node_with_text("Wednesday, April 12, 2023").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("OK").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Selected date timestamp: 1681257600000").assert_exists();
    let rule = KompozTestRule::with_viewport(DateInputSample, IntSize::new(400, 900));
    rule.on_node_with_text("Entered date timestamp: no input").assert_exists();
    type_digits(&rule, "Date, MM/DD/YYYY", "04152023");
    rule.on_node_with_text("Entered date timestamp: 1681516800000").assert_exists();
    let rule = KompozTestRule::with_viewport(DateRangePickerSample, IntSize::new(400, 900));
    rule.on_node_with_text("Save").assert_is_not_enabled();
    rule.on_node_with_text("Today, Monday, April 3, 2023").perform_click();
    rule.on_node_with_text("Friday, April 7, 2023").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Save").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Saved range (timestamps): 1680480000000..1680825600000").assert_exists();
    let rule = KompozTestRule::with_viewport(DateRangePickerApi26Sample, IntSize::new(400, 900));
    rule.on_node_with_text("Save").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Saved range: 2023-04-15..2023-04-20").assert_exists();
    // The twins: weekends and 2022 disabled by java.time's day of the week, then by java.util's calendar's.
    for java_time in [true, false] {
        let rule = KompozTestRule::with_viewport(
            kompozable_lambda!(move || DatePickerWithDateSelectableDatesSampleTwin(java_time)),
            IntSize::new(400, 900),
        );
        rule.on_node_with_text("Saturday, April 1, 2023").assert_is_not_enabled();
        rule.on_node_with_text("Sunday, April 2, 2023").assert_is_not_enabled();
        rule.on_node_with_text("Today, Monday, April 3, 2023").assert_is_enabled().perform_click();
        rule.wait_for_idle();
        rule.on_node_with_text("Selected date timestamp: 1680480000000").assert_exists();
    }
    // The Hebrew state's month and day, as CLDR writes them in Hebrew (not in English).
    let he = CalendarLocale::for_language_tag("HE");
    let month = DatePickerDefaults.date_formatter().format_month_year(Some(1578096000000), &he).unwrap();
    let day = DatePickerDefaults.date_formatter().format_date(Some(1578096000000), &he, true).unwrap();
    assert_ne!(month, "January 2020");
    let rule = KompozTestRule::with_viewport(DatePickerCustomLocaleSampleTwin, IntSize::new(400, 900));
    rule.on_node_with_text(&month).assert_exists();
    rule.on_node_with_text(&day).assert_is_selected();
    rule.on_node_with_text("Selected date timestamp: 1578096000000").assert_exists();
    let rule = KompozTestRule::with_viewport(DatePickerCustomLocaleApi26SampleTwin, IntSize::new(400, 900));
    rule.on_node_with_text(&month).assert_exists();
    rule.on_node_with_text("Selected date: 2020-01-04").assert_exists();
    set_today_for_tests(None);
}

// ==== time pickers ===================================================================================================
// The C++ Kompoz's time picker tests (kompoz_tests.cpp: time_picker_dial_picks_hours_then_minutes,
// time_picker_lays_out_side_by_side_in_landscape, time_picker_twelve_hour_clock, time_input_types_hours_and_minutes,
// time_picker_dialog_switches_modes_and_confirms) written in Kotlin, checked as crates/kompoz-material3/tests/
// time_picker.rs checks them (in English here; M15's behaviour, the differences from the C++'s said there), and
// material3's TimePickerSamples from kt/time_picker.kt.

kompoz! {
    @Composable
    fun DialPicking() {
        val state = rememberTimePickerState(9, 30, is24Hour = true)
        Column {
            TimePicker(state = state, layoutType = TimePickerLayoutType.Vertical)
            Text("time ${state.hour}:${state.minute} ${state.selection}")
        }
    }

    @Composable
    fun LandscapePicker() {
        TimePicker(rememberTimePickerState(9, 30, false))
    }

    @Composable
    fun TwelveHourPicker() {
        val state = rememberTimePickerState(initialHour = 21, initialMinute = 5, is24Hour = false)
        Column {
            TimePicker(state, layoutType = TimePickerLayoutType.Vertical)
            Text("hour ${state.hour} pm ${state.isPm}")
        }
    }

    @Composable
    fun TimeTyping() {
        val state = rememberTimePickerState(9, 30, true)
        Column {
            TimeInput(state)
            Text("time ${state.hour}:${state.minute} typed ${state.hourInput} valid ${state.isInputValid}")
        }
    }

    @Composable
    fun TimeDialog() {
        var open by remember { mutableStateOf(true) }
        var confirmed by remember { mutableStateOf("none") }
        Text("confirmed $confirmed")
        if (open) {
            val state = rememberTimePickerState(8, 15, is24Hour = true)
            var typing by remember { mutableStateOf(false) }
            TimePickerDialog(
                onDismissRequest = { open = false },
                confirmButton = {
                    TextButton(onClick = { confirmed = "${state.hour}:${state.minute}"; open = false }) { Text("OK") }
                },
                title = { Text("Pick a time") },
                dismissButton = { TextButton(onClick = { open = false }) { Text("Cancel") } },
                modeToggleButton = {
                    IconButton(onClick = { typing = !typing }) {
                        Icon(if (typing) Icons.Filled.Schedule else Icons.Filled.Keyboard, "Mode")
                    }
                },
            ) {
                if (typing) {
                    TimeInput(state)
                } else {
                    TimePicker(state, layoutType = TimePickerLayoutType.Vertical)
                }
            }
        }
    }
}

/// A dial number, by its description alone (an hour or minute box is described by what it picks and its value).
fn kt_dial<'a>(rule: &'a KompozTestRule, description: &str) -> kompoz::test::SemanticsNodeInteraction<'a> {
    rule.on_node(kompoz::test::SemanticsMatcher::expect_value(
        kompoz::ui::semantics::SemanticsProperties::ContentDescription,
        vec![description.to_string()],
    ))
}

fn kt_settle(rule: &KompozTestRule) {
    rule.advance_time_by(1000);
    rule.wait_for_idle();
}

fn kt_key(rule: &KompozTestRule, key: kompoz::ui::input::Key) {
    rule.host().key_down(key, kompoz::ui::input::KeyModifiers::None);
    rule.host().key_up(key, kompoz::ui::input::KeyModifiers::None);
    rule.wait_for_idle();
}

fn kt_type(rule: &KompozTestRule, text: &str) {
    rule.host().text_input(text);
    rule.wait_for_idle();
}

fn time_picker_dial_picks_hours_then_minutes() {
    let rule = KompozTestRule::with_viewport(DialPicking, IntSize::new(400, 800));
    let (outer, inner) = (kt_dial(&rule, "0 hours"), kt_dial(&rule, "12 hours"));
    let (outer, inner) = (outer.get_unclipped_bounds_in_root(), inner.get_unclipped_bounds_in_root());
    assert!((inner.top - outer.top - 32.0).abs() < 0.5);
    kt_dial(&rule, "15 hours").perform_click(); // the inner ring
    kt_settle(&rule);
    rule.on_node_with_text("time 15:30 Minute").assert_exists();
    kt_dial(&rule, "40 minutes").perform_click();
    kt_settle(&rule);
    rule.on_node_with_text("time 15:40 Minute").assert_exists();
    rule.on_node_with_content_description("Select hour").perform_click();
    kt_settle(&rule);
    rule.on_node_with_text("time 15:40 Hour").assert_exists();
    kt_dial(&rule, "3 hours").perform_click(); // the outer ring: the morning's
    kt_settle(&rule);
    rule.on_node_with_text("time 3:40 Minute").assert_exists();
}

fn time_picker_lays_out_side_by_side_in_landscape() {
    let rule = KompozTestRule::with_viewport(LandscapePicker, IntSize::new(800, 400));
    let bounds = |text: &str| rule.on_node_with_text(text).get_unclipped_bounds_in_root();
    let (hour, minute, am) = (bounds("09"), bounds("30"), bounds("AM"));
    let twelve = kt_dial(&rule, "12 o'clock").get_unclipped_bounds_in_root();
    assert!((hour.top - minute.top).abs() < 0.5 && minute.left > hour.right);
    assert!(am.top > hour.bottom && twelve.left > minute.right + 52.0);
}

fn time_picker_twelve_hour_clock() {
    let rule = KompozTestRule::with_viewport(TwelveHourPicker, IntSize::new(400, 800));
    rule.on_node_with_text("09").assert_exists();
    rule.on_node_with_text("AM").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("hour 9 pm false").assert_exists();
    kt_dial(&rule, "12 o'clock").perform_click(); // 12 in the morning
    kt_settle(&rule);
    rule.on_node_with_text("hour 0 pm false").assert_exists();
    rule.on_node_with_text("PM").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("hour 12 pm true").assert_exists();
}

fn time_input_types_hours_and_minutes() {
    use kompoz::ui::input::Key;
    let rule = KompozTestRule::with_viewport(TimeTyping, IntSize::new(400, 800));
    rule.on_node_with_content_description("for hour").perform_click();
    rule.wait_for_idle();
    kt_key(&rule, Key::MoveEnd);
    kt_key(&rule, Key::Backspace);
    kt_key(&rule, Key::Backspace);
    kt_type(&rule, "14");
    rule.on_node_with_text("time 14:30 typed 14 valid true").assert_exists();
    kt_key(&rule, Key::MoveHome);
    kt_type(&rule, "7"); // typed at the start of a full field: it starts over
    rule.on_node_with_text("time 7:30 typed 7 valid true").assert_exists();
    kt_type(&rule, "5"); // 75: taken as typed, not an hour
    rule.on_node_with_text("time 7:30 typed 75 valid false").assert_exists();
    kt_key(&rule, Key::Backspace);
    rule.on_node_with_text("time 7:30 typed 7 valid true").assert_exists();
}

fn time_picker_dialog_switches_modes_and_confirms() {
    let rule = KompozTestRule::with_viewport(TimeDialog, IntSize::new(400, 800));
    rule.on_node_with_text("Pick a time").assert_exists();
    kt_dial(&rule, "20 hours").perform_click();
    kt_settle(&rule);
    rule.on_node_with_content_description("Mode").perform_click();
    kt_settle(&rule);
    assert!(!kt_dial(&rule, "12 hours").exists() && !rule.on_all_nodes_with_text("20").is_empty());
    rule.on_node_with_text("OK").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("confirmed 20:15").assert_exists();
    assert_eq!(rule.host().overlay_count(), 0);
}

kompoz_file!("tests/kt/time_picker.kt");

// TimePickerSwitchableSample's twin: the sample but its Android lines (`val configuration =
// LocalConfiguration.current`, `configuration.screenHeightDp.dp`: Android's, stage 15), the window's height in dp
// read instead; its time written by java.text's `SimpleDateFormat` from java.util's `Calendar` as the sample's.
kompoz! {
    import androidx.compose.material3.TimePickerDialogDefaults.MinHeightForTimePicker
    import java.text.SimpleDateFormat
    import java.util.Calendar
    import java.util.Locale

    @OptIn(ExperimentalMaterial3Api::class)
    @Composable
    fun TimePickerSwitchableSampleTwin() {
        var showTimePicker by remember { mutableStateOf(false) }
        val state = rememberTimePickerState()
        val formatter = remember { SimpleDateFormat("hh:mm a", Locale.getDefault()) }
        val snackState = remember { SnackbarHostState() }
        var displayMode by remember { mutableStateOf(TimePickerDisplayMode.Picker) }
        val snackScope = rememberCoroutineScope()
        val screenHeight = with(LocalDensity.current) { LocalWindowInfo.current.containerSize.height.toDp() }

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
                    if (screenHeight > MinHeightForTimePicker) {
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
                        screenHeight > MinHeightForTimePicker
                ) {
                    TimePicker(state = state)
                } else {
                    TimeInput(state = state)
                }
            }
        }
    }
}

/// TimePickerSample's behaviour: the dial in the dialog, Ok shows the time picked; Cancel closes it.
fn kt_check_time_picker_sample(rule: KompozTestRule) {
    rule.on_node_with_text("Set Time").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Select Time").assert_exists();
    kt_dial(&rule, "3 o'clock").perform_click();
    kt_settle(&rule);
    kt_dial(&rule, "30 minutes").perform_click();
    kt_settle(&rule);
    rule.on_node_with_text("PM").perform_click();
    rule.on_node_with_text("Ok").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Entered time: 03:30 PM").assert_exists();
    rule.advance_time_by(5000); // (the snackbar, over the button, gone)
    rule.wait_for_idle();
    rule.on_node_with_text("Set Time").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Cancel").perform_click();
    rule.wait_for_idle();
    assert_eq!(rule.host().overlay_count(), 0);
}

/// TimeInputSample's behaviour: the input in the dialog; typed 7 (on to the minute) and 45, Ok shows it (`shown`:
/// the default locale's text of 07:45).
fn kt_check_time_input_sample(rule: KompozTestRule, shown: &str) {
    use kompoz::ui::input::Key;
    rule.on_node_with_text("Set Time").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Enter Time").assert_exists();
    rule.on_node_with_content_description("for hour").perform_click();
    rule.wait_for_idle();
    kt_key(&rule, Key::MoveEnd);
    kt_key(&rule, Key::Backspace);
    kt_key(&rule, Key::Backspace);
    kt_type(&rule, "7"); // on to the minute
    kt_settle(&rule);
    kt_key(&rule, Key::MoveEnd);
    kt_key(&rule, Key::Backspace);
    kt_key(&rule, Key::Backspace);
    kt_type(&rule, "45");
    rule.on_node_with_text("Ok").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text(&format!("Entered time: {shown}")).assert_exists();
}

/// TimePickerSwitchableSample's behaviour: the toggle switches the dial and the input while the window is taller
/// than 300 dp; a shorter one has the input, no toggle.
fn kt_check_time_picker_switchable_sample(rule: KompozTestRule) {
    rule.on_node_with_text("Set Time").perform_click();
    rule.wait_for_idle();
    assert!(kt_dial(&rule, "12 o'clock").exists());
    rule.on_node_with_content_description("Switch to text input mode").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Enter Time").assert_exists();
    assert!(!kt_dial(&rule, "12 o'clock").exists());
    rule.on_node_with_content_description("Switch to clock mode").perform_click();
    rule.wait_for_idle();
    assert!(kt_dial(&rule, "12 o'clock").exists());
    rule.host().set_viewport_size(IntSize::new(800, 280));
    rule.wait_for_idle();
    rule.on_node_with_content_description("for hour").assert_exists();
    assert!(!rule.on_node_with_content_description("Switch to text input mode").exists());
    rule.on_node_with_text("Ok").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Entered time: 12:00 AM").assert_exists();
}

/// The samples of kt/time_picker.kt and TimePickerSwitchableSample's twin, their times in the default locale's
/// (Java's `Locale.getDefault()`, the system's: English here, then Turkish's AM/PM markers).
fn time_picker_samples() {
    let system = JavaLocale::get_default();
    JavaLocale::set_default(JavaLocale::US);
    kt_check_time_picker_sample(KompozTestRule::new(TimePickerSample));
    kt_check_time_input_sample(KompozTestRule::new(TimeInputSample), "07:45 AM");
    kt_check_time_picker_switchable_sample(KompozTestRule::new(TimePickerSwitchableSampleTwin));
    JavaLocale::set_default(JavaLocale::new("tr"));
    kt_check_time_input_sample(KompozTestRule::new(TimeInputSample), "07:45 ÖÖ");
    JavaLocale::set_default(system);
}

fn main() {
    common::run(&[
        // (date pickers)
        ("date_picker_picks_days_months_and_years", date_picker_picks_days_months_and_years),
        ("date_picker_takes_a_typed_date_in_input_mode", date_picker_takes_a_typed_date_in_input_mode),
        ("date_picker_dialog_confirms_the_date", date_picker_dialog_confirms_the_date),
        ("date_range_picker_takes_typed_dates_in_input_mode", date_range_picker_takes_typed_dates_in_input_mode),
        ("date_range_picker_picks_a_range", date_range_picker_picks_a_range),
        ("date_picker_samples", date_picker_samples),
        // (time pickers)
        ("time_picker_dial_picks_hours_then_minutes", time_picker_dial_picks_hours_then_minutes),
        ("time_picker_lays_out_side_by_side_in_landscape", time_picker_lays_out_side_by_side_in_landscape),
        ("time_picker_twelve_hour_clock", time_picker_twelve_hour_clock),
        ("time_input_types_hours_and_minutes", time_input_types_hours_and_minutes),
        ("time_picker_dialog_switches_modes_and_confirms", time_picker_dialog_switches_modes_and_confirms),
        ("time_picker_samples", time_picker_samples),
    ]);
}

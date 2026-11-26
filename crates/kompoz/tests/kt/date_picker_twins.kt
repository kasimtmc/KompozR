// The Kotlin twins of DatePickerSamples' Android samples (kt/date_picker.kt says why they aren't there): the samples
// but their Android lines — `Build.VERSION.SDK_INT >= Build.VERSION_CODES.O` a parameter; Android's `LocaleList` and
// `Configuration` a java.util `Locale` given to the state (Compose for desktop's `CalendarLocale`), the layout right
// to left.

import java.time.DayOfWeek
import java.time.Instant
import java.time.ZoneId
import java.util.Calendar
import java.util.Locale
import java.util.TimeZone

@Composable
fun DatePickerWithDateSelectableDatesSampleTwin(javaTime: Boolean) {
    val datePickerState =
        rememberDatePickerState(
            selectableDates =
                object : SelectableDates {
                    // Blocks Sunday and Saturday from being selected.
                    override fun isSelectableDate(utcTimeMillis: Long): Boolean {
                        return if (javaTime) {
                            val dayOfWeek =
                                Instant.ofEpochMilli(utcTimeMillis)
                                    .atZone(ZoneId.of("UTC"))
                                    .toLocalDate()
                                    .dayOfWeek
                            dayOfWeek != DayOfWeek.SUNDAY && dayOfWeek != DayOfWeek.SATURDAY
                        } else {
                            val calendar = Calendar.getInstance(TimeZone.getTimeZone("UTC"))
                            calendar.timeInMillis = utcTimeMillis
                            calendar[Calendar.DAY_OF_WEEK] != Calendar.SUNDAY &&
                                calendar[Calendar.DAY_OF_WEEK] != Calendar.SATURDAY
                        }
                    }

                    // Allow selecting dates from year 2023 forward.
                    override fun isSelectableYear(year: Int): Boolean {
                        return year > 2022
                    }
                }
        )

    Column(
        modifier = Modifier.verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        DatePicker(state = datePickerState)
        Text(
            "Selected date timestamp: ${datePickerState.selectedDateMillis ?: "no selection"}",
            modifier = Modifier.align(Alignment.CenterHorizontally),
        )
    }
}

@Composable
fun DatePickerCustomLocaleSampleTwin() {
    val preferredLocale = Locale.forLanguageTag("HE")
    CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Rtl) {
        Column(
            modifier = Modifier.verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            // Pre-select a date for January 4, 2020
            val datePickerState = remember {
                DatePickerState(
                    initialSelectedDateMillis = 1578096000000,
                    // Set to "HE" locale.
                    locale = preferredLocale,
                )
            }
            DatePicker(state = datePickerState, modifier = Modifier.padding(16.dp))

            Text(
                "Selected date timestamp: ${datePickerState.selectedDateMillis ?: "no selection"}",
                modifier = Modifier.align(Alignment.CenterHorizontally),
            )
        }
    }
}

@Composable
fun DatePickerCustomLocaleApi26SampleTwin() {
    val preferredLocale = Locale.forLanguageTag("HE")
    CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Rtl) {
        Column(
            modifier = Modifier.verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            // Pre-select a date for January 4, 2020
            val datePickerState = remember {
                DatePickerState(
                    initialSelectedDate = LocalDate.of(2020, 1, 4),
                    // Set to "HE" locale.
                    locale = preferredLocale,
                )
            }
            DatePicker(state = datePickerState, modifier = Modifier.padding(16.dp))

            Text(
                "Selected date: ${datePickerState.getSelectedDate() ?: "no selection"}",
                modifier = Modifier.align(Alignment.CenterHorizontally),
            )
        }
    }
}

//! The date pickers' `java.time` forms (Compose's jvmAndAndroidMain DatePicker.jvmAndAndroid.kt, as Compose for
//! desktop has it): states made of, and read as, [`LocalDate`]s and [`YearMonth`]s — the dates' UTC midnights, the
//! months' first days' — beside the UTC milliseconds the states hold.

use std::ops::RangeInclusive;
use std::rc::Rc;

use kompoz_runtime::kt::{LocalDate, YearMonth};
use kompoz_runtime::{kompozable, named, named_args};

use crate::calendar_locale::CalendarLocale;
use crate::date_picker::{
    DatePickerDefaults, DatePickerState, DisplayMode, SelectableDates, date_picker_state, remember_date_picker_state,
};
use crate::date_range_picker::{DateRangePickerState, date_range_picker_state, remember_date_range_picker_state};
use crate::internal::calendar_model::MillisecondsIn24Hours;

/// A [`DatePickerState`] of a [`LocalDate`] remembered and saved (Compose's JVM `rememberDatePickerState(
/// initialSelectedDate, …)`): `initial_selected_date` picked, the month of `initial_displayed_month` shown (None is
/// Kotlin's default, the picked date's month; `Some(None)` the current month), as
/// [`remember_date_picker_state`] says.
///
/// Kotlin: `rememberDatePickerState(initialSelectedDate: LocalDate?, initialDisplayedMonth: YearMonth? = initialSelectedDate?.let { YearMonth.from(it) }, yearRange: IntRange = DatePickerDefaults.YearRange, initialDisplayMode: DisplayMode = DisplayMode.Picker, selectableDates: SelectableDates = DatePickerDefaults.AllDates)`
#[kompozable]
pub fn remember_date_picker_state_of_date(
    initial_selected_date: Option<LocalDate>,
    #[default(None)] initial_displayed_month: Option<Option<YearMonth>>,
    #[default(DatePickerDefaults::YearRange())] year_range: RangeInclusive<i32>,
    #[default(DisplayMode::Picker)] initial_display_mode: DisplayMode,
    #[default(DatePickerDefaults::AllDates())] selectable_dates: Rc<dyn SelectableDates>,
) -> Rc<dyn DatePickerState> {
    let initial_displayed_month =
        initial_displayed_month.unwrap_or_else(|| initial_selected_date.map(|date| date.year_month()));
    remember_date_picker_state(
        initial_selected_date.map(get_local_date_millis_utc),
        Some(initial_displayed_month.map(get_year_month_millis_utc)),
        year_range,
        initial_display_mode,
        selectable_dates,
    )
}

/// A [`DatePickerState`] of a [`LocalDate`] in `locale` (Compose's JVM `DatePickerState(locale,
/// initialSelectedDate, …)`), made outside composition.
///
/// Kotlin: `DatePickerState(locale: CalendarLocale, initialSelectedDate: LocalDate?, initialDisplayedMonth: YearMonth? = initialSelectedDate?.let { YearMonth.from(it) }, yearRange: IntRange = DatePickerDefaults.YearRange, initialDisplayMode: DisplayMode = DisplayMode.Picker, selectableDates: SelectableDates = DatePickerDefaults.AllDates)`
#[named]
pub fn date_picker_state_of_date(
    locale: CalendarLocale,
    initial_selected_date: Option<LocalDate>,
    #[default(None)] initial_displayed_month: Option<Option<YearMonth>>,
    #[default(DatePickerDefaults::YearRange())] year_range: RangeInclusive<i32>,
    #[default(DisplayMode::Picker)] initial_display_mode: DisplayMode,
    #[default(DatePickerDefaults::AllDates())] selectable_dates: Rc<dyn SelectableDates>,
) -> Rc<dyn DatePickerState> {
    let initial_displayed_month =
        initial_displayed_month.unwrap_or_else(|| initial_selected_date.map(|date| date.year_month()));
    named_args!(date_picker_state(
        locale,
        initial_selected_date_millis = initial_selected_date.map(get_local_date_millis_utc),
        initial_displayed_month_millis = Some(initial_displayed_month.map(get_year_month_millis_utc)),
        year_range = year_range,
        initial_display_mode = initial_display_mode,
        selectable_dates = selectable_dates,
    ))
}

/// A [`DateRangePickerState`] of [`LocalDate`]s remembered and saved (Compose's JVM `rememberDateRangePickerState(
/// initialSelectedStartDate, initialSelectedEndDate, …)`): the month of `initial_displayed_month` shown (None is
/// Kotlin's default, the start's month; `Some(None)` the current month), as [`remember_date_range_picker_state`]
/// says.
///
/// Kotlin: `rememberDateRangePickerState(initialSelectedStartDate: LocalDate?, initialSelectedEndDate: LocalDate?, initialDisplayedMonth: YearMonth? = initialSelectedStartDate?.let { YearMonth.from(it) }, yearRange: IntRange = DatePickerDefaults.YearRange, initialDisplayMode: DisplayMode = DisplayMode.Picker, selectableDates: SelectableDates = DatePickerDefaults.AllDates)`
#[kompozable]
pub fn remember_date_range_picker_state_of_dates(
    initial_selected_start_date: Option<LocalDate>,
    initial_selected_end_date: Option<LocalDate>,
    #[default(None)] initial_displayed_month: Option<Option<YearMonth>>,
    #[default(DatePickerDefaults::YearRange())] year_range: RangeInclusive<i32>,
    #[default(DisplayMode::Picker)] initial_display_mode: DisplayMode,
    #[default(DatePickerDefaults::AllDates())] selectable_dates: Rc<dyn SelectableDates>,
) -> Rc<dyn DateRangePickerState> {
    let initial_displayed_month =
        initial_displayed_month.unwrap_or_else(|| initial_selected_start_date.map(|date| date.year_month()));
    remember_date_range_picker_state(
        initial_selected_start_date.map(get_local_date_millis_utc),
        initial_selected_end_date.map(get_local_date_millis_utc),
        Some(initial_displayed_month.map(get_year_month_millis_utc)),
        year_range,
        initial_display_mode,
        selectable_dates,
    )
}

/// A [`DateRangePickerState`] of [`LocalDate`]s in `locale` (Compose's JVM `DateRangePickerState(locale,
/// initialSelectedStartDate, initialSelectedEndDate, …)`), made outside composition.
///
/// Kotlin: `DateRangePickerState(locale: CalendarLocale, initialSelectedStartDate: LocalDate?, initialSelectedEndDate: LocalDate?, initialDisplayedMonth: YearMonth? = initialSelectedStartDate?.let { YearMonth.from(it) }, yearRange: IntRange = DatePickerDefaults.YearRange, initialDisplayMode: DisplayMode = DisplayMode.Picker, selectableDates: SelectableDates = DatePickerDefaults.AllDates)`
#[named]
pub fn date_range_picker_state_of_dates(
    locale: CalendarLocale,
    initial_selected_start_date: Option<LocalDate>,
    initial_selected_end_date: Option<LocalDate>,
    #[default(None)] initial_displayed_month: Option<Option<YearMonth>>,
    #[default(DatePickerDefaults::YearRange())] year_range: RangeInclusive<i32>,
    #[default(DisplayMode::Picker)] initial_display_mode: DisplayMode,
    #[default(DatePickerDefaults::AllDates())] selectable_dates: Rc<dyn SelectableDates>,
) -> Rc<dyn DateRangePickerState> {
    let initial_displayed_month =
        initial_displayed_month.unwrap_or_else(|| initial_selected_start_date.map(|date| date.year_month()));
    named_args!(date_range_picker_state(
        locale,
        initial_selected_start_date_millis = initial_selected_start_date.map(get_local_date_millis_utc),
        initial_selected_end_date_millis = initial_selected_end_date.map(get_local_date_millis_utc),
        initial_displayed_month_millis = Some(initial_displayed_month.map(get_year_month_millis_utc)),
        year_range = year_range,
        initial_display_mode = initial_display_mode,
        selectable_dates = selectable_dates,
    ))
}

/// A [`DatePickerState`] read and written as `java.time` dates (Compose's JVM extension functions of
/// `DatePickerState`).
pub trait DatePickerStateJavaTime {
    /// Picks `date` (its UTC midnight; none if `None` or outside the year range) (Compose's
    /// `DatePickerState.setSelectedDate`).
    ///
    /// Kotlin: `setSelectedDate(date: LocalDate?)`
    fn set_selected_date(&self, date: Option<LocalDate>);

    /// The picked date, if any (Compose's `DatePickerState.getSelectedDate`).
    fn get_selected_date(&self) -> Option<LocalDate>;

    /// Shows `year_month` if in the year range (Compose's `DatePickerState.setDisplayedMonth`).
    ///
    /// Kotlin: `setDisplayedMonth(yearMonth: YearMonth)`
    fn set_displayed_month(&self, year_month: YearMonth);

    /// The month shown (Compose's `DatePickerState.getDisplayedMonth`).
    fn get_displayed_month(&self) -> YearMonth;
}

impl DatePickerStateJavaTime for dyn DatePickerState {
    fn set_selected_date(&self, date: Option<LocalDate>) {
        // The selectedDateMillis setter handles the yearRange validation.
        self.set_selected_date_millis(date.map(get_local_date_millis_utc));
    }

    fn get_selected_date(&self) -> Option<LocalDate> {
        self.selected_date_millis().map(get_local_date)
    }

    fn set_displayed_month(&self, year_month: YearMonth) {
        // The displayedMonthMillis setter handles the yearRange validation.
        self.set_displayed_month_millis(get_year_month_millis_utc(year_month));
    }

    fn get_displayed_month(&self) -> YearMonth {
        get_year_month(self.displayed_month_millis())
    }
}

/// A [`DateRangePickerState`] read and written as `java.time` dates (Compose's JVM extension functions of
/// `DateRangePickerState`).
pub trait DateRangePickerStateJavaTime {
    /// Picks the range of the dates given, as [`set_selection`](DateRangePickerState::set_selection) takes it
    /// (Compose's `DateRangePickerState.setSelection(startDate, endDate)`).
    ///
    /// Kotlin: `setSelection(startDate: LocalDate?, endDate: LocalDate?)`
    fn set_selection_of_dates(&self, start_date: Option<LocalDate>, end_date: Option<LocalDate>);

    /// The start date, if any (Compose's `DateRangePickerState.getSelectedStartDate`).
    fn get_selected_start_date(&self) -> Option<LocalDate>;

    /// The end date, if any (Compose's `DateRangePickerState.getSelectedEndDate`).
    fn get_selected_end_date(&self) -> Option<LocalDate>;

    /// Shows `year_month` if in the year range (Compose's `DateRangePickerState.setDisplayedMonth`).
    ///
    /// Kotlin: `setDisplayedMonth(yearMonth: YearMonth)`
    fn set_displayed_month(&self, year_month: YearMonth);

    /// The month shown (Compose's `DateRangePickerState.getDisplayedMonth`).
    fn get_displayed_month(&self) -> YearMonth;
}

impl DateRangePickerStateJavaTime for dyn DateRangePickerState {
    fn set_selection_of_dates(&self, start_date: Option<LocalDate>, end_date: Option<LocalDate>) {
        self.set_selection(start_date.map(get_local_date_millis_utc), end_date.map(get_local_date_millis_utc));
    }

    fn get_selected_start_date(&self) -> Option<LocalDate> {
        self.selected_start_date_millis().map(get_local_date)
    }

    fn get_selected_end_date(&self) -> Option<LocalDate> {
        self.selected_end_date_millis().map(get_local_date)
    }

    fn set_displayed_month(&self, year_month: YearMonth) {
        // The displayedMonthMillis setter handles the yearRange validation.
        self.set_displayed_month_millis(get_year_month_millis_utc(year_month));
    }

    fn get_displayed_month(&self) -> YearMonth {
        get_year_month(self.displayed_month_millis())
    }
}

/// The month of a UTC instant (Compose's private `getYearMonth`).
fn get_year_month(millis_utc_first_of_month: i64) -> YearMonth {
    get_local_date(millis_utc_first_of_month).year_month()
}

/// The UTC milliseconds of a month's first day's midnight (Compose's private `getYearMonthMillisUtc`).
fn get_year_month_millis_utc(year_month: YearMonth) -> i64 {
    get_local_date_millis_utc(year_month.at_day(1))
}

/// The date of a UTC instant (Compose's private `getLocalDate`).
fn get_local_date(millis_utc: i64) -> LocalDate {
    LocalDate::of_epoch_day(millis_utc.div_euclid(MillisecondsIn24Hours))
}

/// The UTC milliseconds of a date's midnight (Compose's private `getLocalDateMillisUtc`).
fn get_local_date_millis_utc(date: LocalDate) -> i64 {
    date.to_epoch_day() * MillisecondsIn24Hours
}

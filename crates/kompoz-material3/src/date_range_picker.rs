//! Material's date range picker, as Compose's DateRangePicker.kt (material3 1.5.0-alpha17): [`DateRangePicker`] —
//! the header (a title, the start and end dates as the headline, the mode button) over either the months one under
//! another (a day tapped starts the range, a later one ends it, the days between banded) or the start's and end's
//! text fields. Its [`DateRangePickerState`] holds the start and end dates and the month shown as UTC milliseconds
//! ([`remember_date_range_picker_state`]).

use std::ops::RangeInclusive;
use std::rc::Rc;

use kompoz_animation::Crossfade;
use kompoz_coroutines::{CoroutineScope, LaunchedEffect, remember_coroutine_scope};
use kompoz_foundation::lazy::LazyListState;
use kompoz_foundation::prelude::*;
use kompoz_graphics::LayoutDirection;
use kompoz_graphics::{Color, Dp, IntOffset, Offset, Size};
use kompoz_runtime::{
    MutableState, SavedValue, Saver, kompozable, kompozable_block, list_saver, mutable_state_of, named, named_args,
    remember, remember_keyed, remember_saveable_with,
};
use kompoz_ui::draw::ContentDrawScope;
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::semantics::{CustomAccessibilityAction, LiveRegionMode, ScrollAxisRange};
use kompoz_ui::{Alignment, Modifier};

use crate::calendar_locale::{CalendarLocale, default_locale};
use crate::date_picker::{
    BaseDatePickerStateImpl, CalendarModelHandle, DateEntryContainer, DatePickerColors, DatePickerDefaults,
    DatePickerFormatter, DatePickerHorizontalPadding, DatePickerModeTogglePadding, DisplayMode,
    DisplayModeToggleButton, Month, RecommendedSizeForAccessibility, SelectableDates, WeekDays,
    number_of_months_in_range, update_displayed_month,
};
use crate::date_range_input::DateRangeInputContent;
use crate::internal::calendar_model::{CalendarDate, CalendarModel, CalendarMonth, DaysInWeek, create_calendar_model};
use crate::internal::strings::{Strings, get_string};
use crate::text::{ProvideTextStyle, Text};
use crate::tokens::{DatePickerModalTokens, MotionSchemeKeyTokens};

/// A date range picker (Compose's `DateRangePicker`): as [`DatePicker`](crate::DatePicker), the range's start and
/// end picked — `title` (None is Kotlin's default, [`DateRangePickerDefaults::DateRangePickerTitle`]; `Some(None)`
/// none), `headline` (None is the default, [`DateRangePickerDefaults::DateRangePickerHeadline`]; `Some(None)`
/// none) — over the months, one under another, or the two text fields. A selection the state's
/// [`set_selection`](DateRangePickerState::set_selection) refuses is ignored.
///
/// Kotlin: `DateRangePicker(state: DateRangePickerState, modifier: Modifier = Modifier, dateFormatter: DatePickerFormatter = remember { DatePickerDefaults.dateFormatter() }, colors: DatePickerColors = DatePickerDefaults.colors(), title: (@Composable () -> Unit)? = { … }, headline: (@Composable () -> Unit)? = { … }, showModeToggle: Boolean = true, focusRequester: FocusRequester? = remember { FocusRequester() })`
#[kompozable]
pub fn DateRangePicker(
    state: Rc<dyn DateRangePickerState>,
    #[default(Modifier)] modifier: Modifier,
    #[default(remember(|| DatePickerDefaults.date_formatter()))] date_formatter: Rc<dyn DatePickerFormatter>,
    #[default(DatePickerDefaults.colors())] colors: DatePickerColors,
    #[default(None)] title: Option<Option<Rc<dyn Fn()>>>,
    #[default(None)] headline: Option<Option<Rc<dyn Fn()>>>,
    #[default(true)] show_mode_toggle: bool,
    #[default(Some(remember(FocusRequester::new)))] focus_requester: Option<FocusRequester>,
) {
    let title = title.unwrap_or_else(|| {
        let state = state.clone();
        Some(Rc::new(move || {
            DateRangePickerDefaults.DateRangePickerTitle(
                display_mode = state.display_mode(),
                modifier = Modifier.padding_values(DateRangePickerTitlePadding()),
                content_color = colors.title_content_color,
            )
        }) as Rc<dyn Fn()>)
    });
    let headline = headline.unwrap_or_else(|| {
        let (state, date_formatter) = (state.clone(), date_formatter.clone());
        Some(Rc::new(move || {
            DateRangePickerDefaults.DateRangePickerHeadline(
                selected_start_date_millis = state.selected_start_date_millis(),
                selected_end_date_millis = state.selected_end_date_millis(),
                display_mode = state.display_mode(),
                date_formatter = date_formatter.clone(),
                modifier = Modifier.padding_values(DateRangePickerHeadlinePadding()),
                content_color = colors.headline_content_color,
            )
        }) as Rc<dyn Fn()>)
    });
    let calendar_model = {
        let state = state.clone();
        remember_keyed(state.locale(), move || match state.calendar_model_handle() {
            Some(handle) => handle.0,
            None => create_calendar_model(state.locale()),
        })
    };
    let mode_toggle_button: Option<Rc<dyn Fn()>> = if show_mode_toggle {
        let state = state.clone();
        Some(Rc::new(move || {
            let state = state.clone();
            DisplayModeToggleButton(
                Modifier.padding_values(DatePickerModeTogglePadding()),
                state.display_mode(),
                Rc::new(move |display_mode| state.set_display_mode(display_mode)),
                colors,
            )
        }))
    } else {
        None
    };
    let content_state = state.clone();
    DateEntryContainer(
        modifier,
        title,
        headline,
        mode_toggle_button,
        colors,
        DatePickerModalTokens::RangeSelectionHeaderHeadlineFont.value(),
        DatePickerModalTokens::RangeSelectionHeaderContainerHeight - HeaderHeightOffset,
        move || {
            let state = content_state.clone();
            let on_select: Rc<dyn DateRangePickerState> = state.clone();
            let on_month = state.clone();
            SwitchableDateEntryContent(
                state.selected_start_date_millis(),
                state.selected_end_date_millis(),
                state.displayed_month_millis(),
                state.display_mode(),
                // Kotlin catches the IllegalArgumentException a state's setSelection may throw and ignores it: a
                // Rust state refuses a selection by not making it (a panic isn't an exception to catch).
                Rc::new(move |start_date_millis, end_date_millis| {
                    on_select.set_selection(start_date_millis, end_date_millis)
                }),
                Rc::new(move |month_in_millis| on_month.set_displayed_month_millis(month_in_millis)),
                calendar_model.clone(),
                state.year_range(),
                date_formatter.clone(),
                state.selectable_dates(),
                colors,
                focus_requester.clone(),
            );
        },
    );
}

/// The state of a [`DateRangePicker`] (Compose's interface `DateRangePickerState`): the range's start and end and
/// the month shown as UTC milliseconds since the epoch, the display mode, the years, which dates may be picked and
/// the locale. [`remember_date_range_picker_state`] or [`date_range_picker_state`] make one.
pub trait DateRangePickerState {
    /// The start date's UTC milliseconds, if one is picked (Compose's `selectedStartDateMillis`).
    fn selected_start_date_millis(&self) -> Option<i64>;

    /// The end date's UTC milliseconds, if one is picked (Compose's `selectedEndDateMillis`).
    fn selected_end_date_millis(&self) -> Option<i64>;

    /// The UTC milliseconds of the first day of the month shown (Compose's `displayedMonthMillis`).
    fn displayed_month_millis(&self) -> i64;

    /// Shows the month the UTC milliseconds given are in, if it is in the year range (Compose's
    /// `displayedMonthMillis` setter).
    fn set_displayed_month_millis(&self, month_millis: i64);

    /// Whether the calendar or the text fields show (Compose's `displayMode`).
    fn display_mode(&self) -> DisplayMode;

    /// Shows the calendar or the text fields, the start date's month first (Compose's `displayMode` setter).
    fn set_display_mode(&self, display_mode: DisplayMode);

    /// The years dates may be picked in (Compose's `yearRange`).
    fn year_range(&self) -> RangeInclusive<i32>;

    /// Which dates may be picked (Compose's `selectableDates`).
    fn selectable_dates(&self) -> Rc<dyn SelectableDates>;

    /// The locale dates are formatted in (Compose's `locale`).
    fn locale(&self) -> CalendarLocale;

    /// Picks the range of the UTC milliseconds given (Compose's `setSelection`): the default state picks none if
    /// the start is missing, a date is outside the year range, or the end is before the start (Kotlin documents
    /// an `IllegalArgumentException` a state may throw for those; the default one doesn't).
    fn set_selection(&self, start_date_millis: Option<i64>, end_date_millis: Option<i64>);

    /// The calendar a state of the crate counts in ([`DatePickerState`](crate::DatePickerState)'s).
    #[doc(hidden)]
    fn calendar_model_handle(&self) -> Option<CalendarModelHandle> {
        None
    }
}

impl PartialEq for dyn DateRangePickerState {
    /// The same state (Kotlin's `==` of a class without `equals`: its identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// A [`DateRangePickerState`] remembered across recompositions and saved with the UI's state (Compose's
/// `rememberDateRangePickerState`), in the platform's locale: the start and end picked (as
/// [`set_selection`](DateRangePickerState::set_selection) takes them), the month of
/// `initial_displayed_month_millis` shown (None is Kotlin's default, the start's; `Some(None)` the current month),
/// `initial_display_mode`, `selectable_dates` (updated each recomposition).
///
/// Kotlin: `rememberDateRangePickerState(initialSelectedStartDateMillis: Long? = null, initialSelectedEndDateMillis: Long? = null, initialDisplayedMonthMillis: Long? = initialSelectedStartDateMillis, yearRange: IntRange = DatePickerDefaults.YearRange, initialDisplayMode: DisplayMode = DisplayMode.Picker, selectableDates: SelectableDates = DatePickerDefaults.AllDates)`
#[kompozable]
pub fn remember_date_range_picker_state(
    #[default(None)] initial_selected_start_date_millis: Option<i64>,
    #[default(None)] initial_selected_end_date_millis: Option<i64>,
    #[default(None)] initial_displayed_month_millis: Option<Option<i64>>,
    #[default(DatePickerDefaults::YearRange())] year_range: RangeInclusive<i32>,
    #[default(DisplayMode::Picker)] initial_display_mode: DisplayMode,
    #[default(DatePickerDefaults::AllDates())] selectable_dates: Rc<dyn SelectableDates>,
) -> Rc<dyn DateRangePickerState> {
    let initial_displayed_month_millis = initial_displayed_month_millis.unwrap_or(initial_selected_start_date_millis);
    let locale = default_locale();
    let saver = DateRangePickerStateImpl::Saver(selectable_dates.clone(), locale.clone());
    let state = {
        let (selectable_dates, locale) = (selectable_dates.clone(), locale);
        remember_saveable_with(&saver, move || {
            Rc::new(DateRangePickerStateImpl::new(
                initial_selected_start_date_millis,
                initial_selected_end_date_millis,
                initial_displayed_month_millis,
                year_range,
                initial_display_mode,
                selectable_dates,
                locale,
            ))
        })
    };
    // Update the state's selectable dates if they were changed.
    state.base.set_selectable_dates(selectable_dates);
    state
}

/// A [`DateRangePickerState`] in `locale` (Compose's `DateRangePickerState(locale, …)`), made outside composition:
/// as [`remember_date_range_picker_state`] says.
///
/// Kotlin: `DateRangePickerState(locale: CalendarLocale, initialSelectedStartDateMillis: Long? = null, initialSelectedEndDateMillis: Long? = null, initialDisplayedMonthMillis: Long? = initialSelectedStartDateMillis, yearRange: IntRange = DatePickerDefaults.YearRange, initialDisplayMode: DisplayMode = DisplayMode.Picker, selectableDates: SelectableDates = DatePickerDefaults.AllDates)`
#[named]
pub fn date_range_picker_state(
    locale: CalendarLocale,
    #[default(None)] initial_selected_start_date_millis: Option<i64>,
    #[default(None)] initial_selected_end_date_millis: Option<i64>,
    #[default(None)] initial_displayed_month_millis: Option<Option<i64>>,
    #[default(DatePickerDefaults::YearRange())] year_range: RangeInclusive<i32>,
    #[default(DisplayMode::Picker)] initial_display_mode: DisplayMode,
    #[default(DatePickerDefaults::AllDates())] selectable_dates: Rc<dyn SelectableDates>,
) -> Rc<dyn DateRangePickerState> {
    Rc::new(DateRangePickerStateImpl::new(
        initial_selected_start_date_millis,
        initial_selected_end_date_millis,
        initial_displayed_month_millis.unwrap_or(initial_selected_start_date_millis),
        year_range,
        initial_display_mode,
        selectable_dates,
        locale,
    ))
}

/// What date range pickers are made of by default (Compose's `DateRangePickerDefaults`).
#[derive(Clone, Copy, Debug)]
pub struct DateRangePickerDefaults;

#[named]
impl DateRangePickerDefaults {
    /// A date range picker's title (Compose's `DateRangePickerTitle`): "Select dates" for the calendar, "Enter
    /// dates" for the text fields.
    #[track_caller]
    pub fn DateRangePickerTitle(
        &self,
        display_mode: DisplayMode,
        #[default(Modifier)] modifier: Modifier,
        #[default(DatePickerDefaults.colors().title_content_color)] content_color: Color,
    ) {
        kompozable_block!({
            match display_mode {
                DisplayMode::Picker => {
                    Text(get_string(Strings::DateRangePickerTitle), modifier = modifier, color = content_color)
                }
                DisplayMode::Input => {
                    Text(get_string(Strings::DateRangeInputTitle), modifier = modifier, color = content_color)
                }
                _ => {}
            }
        })
    }

    /// A date range picker's headline (Compose's `DateRangePickerHeadline`): the start date (or "Start date") -
    /// the end date (or "End date"), 4 dp apart, read out politely as one: "Start date: …, End date: …".
    #[track_caller]
    pub fn DateRangePickerHeadline(
        &self,
        selected_start_date_millis: Option<i64>,
        selected_end_date_millis: Option<i64>,
        display_mode: DisplayMode,
        date_formatter: Rc<dyn DatePickerFormatter>,
        #[default(Modifier)] modifier: Modifier,
        #[default(DatePickerDefaults.colors().headline_content_color)] content_color: Color,
    ) {
        let start_date_text = get_string(Strings::DateRangePickerStartHeadline);
        let end_date_text = get_string(Strings::DateRangePickerEndHeadline);
        let (start_placeholder, end_placeholder) = (start_date_text.clone(), end_date_text.clone());
        kompozable_block!({
            DateRangePickerHeadlineImpl(
                selected_start_date_millis,
                selected_end_date_millis,
                display_mode,
                date_formatter,
                modifier,
                content_color,
                start_date_text,
                end_date_text,
                Rc::new(move || Text(start_placeholder.clone(), color = content_color)),
                Rc::new(move || Text(end_placeholder.clone(), color = content_color)),
                Rc::new(move || Text("-", color = content_color)),
                default_locale(),
            );
        })
    }
}

/// The headline's row (Compose's private `DateRangePickerDefaults.DateRangePickerHeadline` of placeholders).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn DateRangePickerHeadlineImpl(
    selected_start_date_millis: Option<i64>,
    selected_end_date_millis: Option<i64>,
    display_mode: DisplayMode,
    date_formatter: Rc<dyn DatePickerFormatter>,
    modifier: Modifier,
    content_color: Color,
    start_date_text: String,
    end_date_text: String,
    start_date_placeholder: Rc<dyn Fn()>,
    end_date_placeholder: Rc<dyn Fn()>,
    dates_delimiter: Rc<dyn Fn()>,
    locale: CalendarLocale,
) {
    let formatter_start_date = date_formatter.format_date(selected_start_date_millis, &locale, false);
    let formatter_end_date = date_formatter.format_date(selected_end_date_millis, &locale, false);
    let no_selection = || match display_mode {
        DisplayMode::Picker => get_string(Strings::DatePickerNoSelectionDescription),
        DisplayMode::Input => get_string(Strings::DateInputNoInputDescription),
        _ => String::new(),
    };
    let verbose_start_date_description =
        date_formatter.format_date(selected_start_date_millis, &locale, true).unwrap_or_else(no_selection);
    let verbose_end_date_description =
        date_formatter.format_date(selected_end_date_millis, &locale, true).unwrap_or_else(no_selection);
    let start_headline_description = format!("{start_date_text}: {verbose_start_date_description}");
    let end_headline_description = format!("{end_date_text}: {verbose_end_date_description}");
    let description = format!("{start_headline_description}, {end_headline_description}");
    Row(
        modifier = modifier.clear_and_set_semantics(move |c| {
            c.set_live_region(LiveRegionMode::Polite);
            c.set_content_description(description.clone());
        }),
        vertical_alignment = Alignment::CenterVertically,
        horizontal_arrangement = Arrangement::spaced_by(Dp(4.0)),
        content = move || {
            match &formatter_start_date {
                Some(start) => Text(start.clone(), color = content_color),
                None => start_date_placeholder(),
            }
            dates_delimiter();
            match &formatter_end_date {
                Some(end) => Text(end.clone(), color = content_color),
                None => end_date_placeholder(),
            }
        },
    );
}

/// The default [`DateRangePickerState`] (Compose's private `DateRangePickerStateImpl`).
pub(crate) struct DateRangePickerStateImpl {
    pub(crate) base: BaseDatePickerStateImpl,
    selected_start_date: MutableState<Option<CalendarDate>>,
    selected_end_date: MutableState<Option<CalendarDate>>,
    display_mode: MutableState<DisplayMode>,
}

impl DateRangePickerStateImpl {
    pub(crate) fn new(
        initial_selected_start_date_millis: Option<i64>,
        initial_selected_end_date_millis: Option<i64>,
        initial_displayed_month_millis: Option<i64>,
        year_range: RangeInclusive<i32>,
        initial_display_mode: DisplayMode,
        selectable_dates: Rc<dyn SelectableDates>,
        locale: CalendarLocale,
    ) -> DateRangePickerStateImpl {
        let state = DateRangePickerStateImpl {
            base: BaseDatePickerStateImpl::new(initial_displayed_month_millis, year_range, selectable_dates, locale),
            selected_start_date: mutable_state_of(None),
            selected_end_date: mutable_state_of(None),
            display_mode: mutable_state_of(initial_display_mode),
        };
        state.set_selection(initial_selected_start_date_millis, initial_selected_end_date_millis);
        state
    }

    /// The date of the UTC milliseconds given, if within the year range (Compose's private `getDate`).
    fn get_date(&self, date_millis: Option<i64>) -> Option<CalendarDate> {
        let date = self.base.calendar_model.get_canonical_date(date_millis?);
        // Validate that the date is within the valid years range.
        self.base.year_range.contains(&date.year).then_some(date)
    }

    /// Saves the start and end dates, the month shown, the year range and the display mode (Compose's companion's
    /// `Saver(selectableDates, locale)`).
    pub(crate) fn Saver(
        selectable_dates: Rc<dyn SelectableDates>,
        locale: CalendarLocale,
    ) -> Saver<Rc<DateRangePickerStateImpl>> {
        let millis = |m: Option<i64>| m.map(SavedValue::Int).unwrap_or(SavedValue::Null);
        list_saver(
            move |it: &Rc<DateRangePickerStateImpl>| {
                vec![
                    millis(it.selected_start_date_millis()),
                    millis(it.selected_end_date_millis()),
                    SavedValue::Int(it.displayed_month_millis()),
                    SavedValue::Int(*it.base.year_range.start() as i64),
                    SavedValue::Int(*it.base.year_range.end() as i64),
                    SavedValue::Int(it.display_mode().0 as i64),
                ]
            },
            move |value: &[SavedValue]| {
                Some(Rc::new(DateRangePickerStateImpl::new(
                    value.first()?.as_int(),
                    value.get(1)?.as_int(),
                    value.get(2)?.as_int(),
                    value.get(3)?.as_int()? as i32..=value.get(4)?.as_int()? as i32,
                    DisplayMode(value.get(5)?.as_int()? as i32),
                    selectable_dates.clone(),
                    locale.clone(),
                )))
            },
        )
    }
}

impl DateRangePickerState for DateRangePickerStateImpl {
    fn selected_start_date_millis(&self) -> Option<i64> {
        self.selected_start_date.get().map(|date| date.utc_time_millis)
    }

    fn selected_end_date_millis(&self) -> Option<i64> {
        self.selected_end_date.get().map(|date| date.utc_time_millis)
    }

    fn displayed_month_millis(&self) -> i64 {
        self.base.displayed_month_millis()
    }

    fn set_displayed_month_millis(&self, month_millis: i64) {
        self.base.set_displayed_month_millis(month_millis);
    }

    fn display_mode(&self) -> DisplayMode {
        self.display_mode.get()
    }

    fn set_display_mode(&self, display_mode: DisplayMode) {
        if let Some(millis) = self.selected_start_date_millis() {
            self.set_displayed_month_millis(self.base.calendar_model.get_month(millis).start_utc_time_millis);
        }
        self.display_mode.set(display_mode);
    }

    fn year_range(&self) -> RangeInclusive<i32> {
        self.base.year_range.clone()
    }

    fn selectable_dates(&self) -> Rc<dyn SelectableDates> {
        self.base.selectable_dates()
    }

    fn locale(&self) -> CalendarLocale {
        self.base.locale.clone()
    }

    fn set_selection(&self, start_date_millis: Option<i64>, end_date_millis: Option<i64>) {
        let start_date = self.get_date(start_date_millis);
        let end_date = self.get_date(end_date_millis);
        // Validate that an end date cannot be set without a start date and that the end date appears on or after
        // the start date.
        match start_date {
            Some(start) if end_date.is_none_or(|end| start.utc_time_millis <= end.utc_time_millis) => {
                self.selected_start_date.set(Some(start));
                self.selected_end_date.set(end_date);
            }
            _ => {
                self.selected_start_date.set(None);
                self.selected_end_date.set(None);
            }
        }
    }

    fn calendar_model_handle(&self) -> Option<CalendarModelHandle> {
        Some(CalendarModelHandle(self.base.calendar_model.clone()))
    }
}

/// The months or the text fields, cross-faded as the mode switches (Compose's private
/// `SwitchableDateEntryContent` of the range picker).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SwitchableDateEntryContent(
    selected_start_date_millis: Option<i64>,
    selected_end_date_millis: Option<i64>,
    displayed_month_millis: i64,
    display_mode: DisplayMode,
    on_dates_selection_change: Rc<dyn Fn(Option<i64>, Option<i64>)>,
    on_displayed_month_change: Rc<dyn Fn(i64)>,
    calendar_model: Rc<dyn CalendarModel>,
    year_range: RangeInclusive<i32>,
    date_formatter: Rc<dyn DatePickerFormatter>,
    selectable_dates: Rc<dyn SelectableDates>,
    colors: DatePickerColors,
    focus_requester: Option<FocusRequester>,
) {
    // TODO(b/266480386): Apply the motion spec for this once we have it. Consider replacing this with
    //  AnimatedContent when it's out of experimental.
    // TODO Load the motionScheme tokens from the component tokens file
    Crossfade(
        target_state = display_mode,
        animation_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>(),
        modifier = Modifier.semantics(|c| c.set_is_container(true)),
        content = move |mode: DisplayMode| match mode {
            DisplayMode::Picker => DateRangePickerContent(
                selected_start_date_millis,
                selected_end_date_millis,
                displayed_month_millis,
                on_dates_selection_change.clone(),
                on_displayed_month_change.clone(),
                calendar_model.clone(),
                year_range.clone(),
                date_formatter.clone(),
                selectable_dates.clone(),
                colors,
            ),
            DisplayMode::Input => DateRangeInputContent(
                selected_start_date_millis,
                selected_end_date_millis,
                on_dates_selection_change.clone(),
                calendar_model.clone(),
                year_range.clone(),
                date_formatter.clone(),
                selectable_dates.clone(),
                colors,
                focus_requester.clone(),
            ),
            _ => {}
        },
    );
}

/// The range's calendar (Compose's private `DateRangePickerContent`): the weekdays over the months' list.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn DateRangePickerContent(
    selected_start_date_millis: Option<i64>,
    selected_end_date_millis: Option<i64>,
    displayed_month_millis: i64,
    on_dates_selection_change: Rc<dyn Fn(Option<i64>, Option<i64>)>,
    on_displayed_month_change: Rc<dyn Fn(i64)>,
    calendar_model: Rc<dyn CalendarModel>,
    year_range: RangeInclusive<i32>,
    date_formatter: Rc<dyn DatePickerFormatter>,
    selectable_dates: Rc<dyn SelectableDates>,
    colors: DatePickerColors,
) {
    let displayed_month = calendar_model.get_month(displayed_month_millis);
    let month_index = displayed_month.index_in(&year_range).max(0);
    let months_list_state = remember_lazy_list_state(initial_first_visible_item_index = month_index);

    // Scroll to the resolved displayedMonth, if needed.
    {
        let state = months_list_state.clone();
        LaunchedEffect(month_index, async move {
            // Unlike the DatePicker, we don't have to check here for isScrollInProgress and scroll to the monthIndex
            // even when there is a current scroll operation.
            if state.first_visible_item_index() != month_index {
                state.scroll_to_item(month_index, 0).await;
            }
        });
    }

    Column(
        modifier = named_args!(Modifier.padding_hv(horizontal = DatePickerHorizontalPadding)),
        content = move || {
            WeekDays(colors, calendar_model.clone());
            VerticalMonthsList(
                months_list_state.clone(),
                selected_start_date_millis,
                selected_end_date_millis,
                on_dates_selection_change.clone(),
                on_displayed_month_change.clone(),
                calendar_model.clone(),
                year_range.clone(),
                date_formatter.clone(),
                selectable_dates.clone(),
                colors,
            );
        },
    );
}

/// The months one under another (Compose's private `VerticalMonthsList`): each month's name over its grid, the
/// range banded; taps go through [`update_date_selection`]; the names carry actions scrolling a month up or down.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn VerticalMonthsList(
    lazy_list_state: LazyListState,
    selected_start_date_millis: Option<i64>,
    selected_end_date_millis: Option<i64>,
    on_dates_selection_change: Rc<dyn Fn(Option<i64>, Option<i64>)>,
    on_displayed_month_change: Rc<dyn Fn(i64)>,
    calendar_model: Rc<dyn CalendarModel>,
    year_range: RangeInclusive<i32>,
    date_formatter: Rc<dyn DatePickerFormatter>,
    selectable_dates: Rc<dyn SelectableDates>,
    colors: DatePickerColors,
) {
    let today = calendar_model.today();
    let first_month = {
        let model = calendar_model.clone();
        let first = *year_range.start();
        // January.
        remember_keyed(year_range.clone(), move || model.get_month_of_year(first, 1))
    };
    {
        let (state, model, years) = (lazy_list_state.clone(), calendar_model.clone(), year_range.clone());
        ProvideTextStyle(DatePickerModalTokens::DateLabelTextFont.value(), move || {
            let coroutine_scope = remember_coroutine_scope();
            let scroll_to_previous_month_label = get_string(Strings::DateRangePickerScrollToShowPreviousMonth);
            let scroll_to_next_month_label = get_string(Strings::DateRangePickerScrollToShowNextMonth);

            // The updateDateSelection will invoke the onDatesSelectionChange with the proper selection according to
            // the current state.
            let on_change = on_dates_selection_change.clone();
            let on_date_selection_change: Rc<dyn Fn(i64)> = Rc::new(move |date_in_millis| {
                update_date_selection(date_in_millis, selected_start_date_millis, selected_end_date_millis, &*on_change)
            });

            let custom_accessibility_action = custom_scroll_actions(
                &state,
                &coroutine_scope,
                scroll_to_previous_month_label,
                scroll_to_next_month_label,
            );

            let (list_state, model, years) = (state.clone(), model.clone(), years.clone());
            let (formatter, dates) = (date_formatter.clone(), selectable_dates.clone());
            LazyColumn(
                // Apply this to have the screen reader traverse outside the visible list of months and not scroll
                // them by default.
                modifier = Modifier
                    .semantics(|c| c.set_vertical_scroll_axis_range(ScrollAxisRange::new(|| 0.0, || 0.0, false))),
                state = Some(state.clone()),
                content = move |scope: &mut LazyListScope| {
                    let (list_state, model) = (list_state.clone(), model.clone());
                    let (formatter, dates) = (formatter.clone(), dates.clone());
                    let (on_select, actions) = (on_date_selection_change.clone(), custom_accessibility_action.clone());
                    scope.items(number_of_months_in_range(&years), None, None, move |it| {
                        let month = model.plus_months(&first_month, it);
                        let (list_state, model) = (list_state.clone(), model.clone());
                        let (formatter, dates) = (formatter.clone(), dates.clone());
                        let (on_select, actions) = (on_select.clone(), actions.clone());
                        Column(
                            modifier = Modifier.fill_parent_max_width(1.0),
                            content = move || {
                                let (formatter_name, model_name, actions) =
                                    (formatter.clone(), model.clone(), actions.clone());
                                ProvideTextStyle(
                                    DatePickerModalTokens::RangeSelectionMonthSubheadFont.value(),
                                    move || {
                                        let actions = actions.clone();
                                        Text(
                                            formatter_name
                                                .format_month_year(
                                                    Some(month.start_utc_time_millis),
                                                    model_name.locale(),
                                                )
                                                .unwrap_or_else(|| "-".to_string()),
                                            modifier = Modifier
                                                .padding_values(CalendarMonthSubheadPadding())
                                                .semantics(move |c| c.set_custom_actions(actions.clone())),
                                            color = colors.subhead_content_color,
                                        );
                                    },
                                );
                                let range_selection_info: Option<SelectedRangeInfo> =
                                    match (selected_start_date_millis, selected_end_date_millis) {
                                        (Some(start), Some(end)) => {
                                            let model = model.clone();
                                            remember_keyed((start, end), move || {
                                                SelectedRangeInfo::calculate_range_info(
                                                    &month,
                                                    &model.get_canonical_date(start),
                                                    &model.get_canonical_date(end),
                                                )
                                            })
                                        }
                                        _ => None,
                                    };
                                Month(
                                    month,
                                    on_select.clone(),
                                    today.utc_time_millis,
                                    selected_start_date_millis,
                                    selected_end_date_millis,
                                    range_selection_info,
                                    formatter.clone(),
                                    dates.clone(),
                                    colors,
                                    model.locale().clone(),
                                    list_state.clone(),
                                    None,
                                    Rc::new(|| {}),
                                );
                            },
                        );
                    });
                },
            );
        });
    }
    let state = lazy_list_state.clone();
    LaunchedEffect(
        lazy_list_state,
        update_displayed_month(state, on_displayed_month_change, calendar_model, year_range),
    );
}

/// The selection after a tap on a day (Compose's private `updateDateSelection`): a start when there is none or a
/// whole range already, the end when on or after the start, a new start when before it.
fn update_date_selection(
    date_in_millis: i64,
    current_start_date_millis: Option<i64>,
    current_end_date_millis: Option<i64>,
    on_dates_selection_change: &dyn Fn(Option<i64>, Option<i64>),
) {
    match (current_start_date_millis, current_end_date_millis) {
        (None, None) | (Some(_), Some(_)) => {
            // Set the selection to "start" only.
            on_dates_selection_change(Some(date_in_millis), None)
        }
        (Some(start), None) if date_in_millis >= start => {
            // Set the end date.
            on_dates_selection_change(Some(start), Some(date_in_millis))
        }
        _ => {
            // The user selected an earlier date than the start date, so reset the start.
            on_dates_selection_change(Some(date_in_millis), None)
        }
    }
}

/// Compose's internal `CalendarMonthSubheadPadding`: 24 dp at the start, 20 dp above, 8 dp below.
pub(crate) fn CalendarMonthSubheadPadding() -> PaddingValues {
    PaddingValues::new(Dp(24.0), Dp(20.0), Dp(0.0), Dp(8.0))
}

/// Where a month's part of a range is in its grid (Compose's internal class `SelectedRangeInfo`): the first and
/// last cells, and whether they are the range's start and end. Equal only to itself (a class without `equals`).
#[derive(Clone)]
pub(crate) struct SelectedRangeInfo(Rc<SelectedRangeInfoData>);

pub(crate) struct SelectedRangeInfoData {
    pub(crate) grid_start_coordinates: IntOffset,
    pub(crate) grid_end_coordinates: IntOffset,
    pub(crate) first_is_selection_start: bool,
    pub(crate) last_is_selection_end: bool,
}

impl PartialEq for SelectedRangeInfo {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::ops::Deref for SelectedRangeInfo {
    type Target = SelectedRangeInfoData;
    fn deref(&self) -> &SelectedRangeInfoData {
        &self.0
    }
}

impl SelectedRangeInfo {
    /// The range's part in `month`, if any (Compose's companion's `calculateRangeInfo`).
    pub(crate) fn calculate_range_info(
        month: &CalendarMonth,
        start_date: &CalendarDate,
        end_date: &CalendarDate,
    ) -> Option<SelectedRangeInfo> {
        if start_date.utc_time_millis > month.end_utc_time_millis()
            || end_date.utc_time_millis < month.start_utc_time_millis
        {
            return None;
        }
        let first_is_selection_start = start_date.utc_time_millis >= month.start_utc_time_millis;
        let last_is_selection_end = end_date.utc_time_millis <= month.end_utc_time_millis();
        let start_grid_item_offset = if first_is_selection_start {
            month.days_from_start_of_week_to_first_of_month + start_date.day_of_month - 1
        } else {
            month.days_from_start_of_week_to_first_of_month
        };
        let end_grid_item_offset = if last_is_selection_end {
            month.days_from_start_of_week_to_first_of_month + end_date.day_of_month - 1
        } else {
            month.days_from_start_of_week_to_first_of_month + month.number_of_days - 1
        };

        // Calculate the selected coordinates within the cells grid.
        let grid_start_coordinates =
            IntOffset::new(start_grid_item_offset % DaysInWeek, start_grid_item_offset / DaysInWeek);
        let grid_end_coordinates = IntOffset::new(end_grid_item_offset % DaysInWeek, end_grid_item_offset / DaysInWeek);
        Some(SelectedRangeInfo(Rc::new(SelectedRangeInfoData {
            grid_start_coordinates,
            grid_end_coordinates,
            first_is_selection_start,
            last_is_selection_end,
        })))
    }
}

/// Draws a month's band of the range behind its days (Compose's internal `ContentDrawScope.drawRangeBackground`):
/// from the start's center to the end's, a state layer's height, row by row, mirrored right to left.
pub(crate) fn draw_range_background(
    scope: &mut ContentDrawScope<'_>,
    selected_range_info: &SelectedRangeInfo,
    color: Color,
) {
    // The LazyVerticalGrid is defined to space the items horizontally by DaysHorizontalPadding (e.g. 4.dp).
    // However, as the grid is not limited in width, the spacing can go beyond that value, so this drawing takes this
    // into account.
    // TODO: Use the date's container width and height from the tokens once b/247694457 is resolved.
    let item_container_width = scope.to_px(RecommendedSizeForAccessibility);
    let item_container_height = scope.to_px(RecommendedSizeForAccessibility);
    let item_state_layer_height = scope.to_px(DatePickerModalTokens::DateStateLayerHeight);
    let state_layer_vertical_padding = (item_container_height - item_state_layer_height) / 2.0;
    let width = scope.size().width;
    let horizontal_space_between_items = (width - DaysInWeek as f32 * item_container_width) / DaysInWeek as f32;

    let (x1, y1) = (selected_range_info.grid_start_coordinates.x, selected_range_info.grid_start_coordinates.y);
    let (x2, y2) = (selected_range_info.grid_end_coordinates.x, selected_range_info.grid_end_coordinates.y);
    // The endX and startX are offset to include only half the item's width when dealing with first and last items in
    // the selection in order to keep the selection edges rounded.
    let mut start_x = x1 as f32 * (item_container_width + horizontal_space_between_items)
        + (if selected_range_info.first_is_selection_start { item_container_width / 2.0 } else { 0.0 })
        + horizontal_space_between_items / 2.0;
    let start_y = y1 as f32 * item_container_height + state_layer_vertical_padding;
    let mut end_x = x2 as f32 * (item_container_width + horizontal_space_between_items)
        + (if selected_range_info.last_is_selection_end { item_container_width / 2.0 } else { item_container_width })
        + horizontal_space_between_items / 2.0;
    let end_y = y2 as f32 * item_container_height + state_layer_vertical_padding;

    let is_rtl = scope.layout_direction() == LayoutDirection::Rtl;
    // Adjust the start and end in case the layout is RTL.
    if is_rtl {
        start_x = width - start_x;
        end_x = width - end_x;
    }

    // Draw the first row background
    let first_width = if y1 == y2 {
        end_x - start_x
    } else if is_rtl {
        -start_x
    } else {
        width - start_x
    };
    named_args!(scope.draw_rect(
        color,
        top_left = Offset::new(start_x, start_y),
        size = Size::new(first_width, item_state_layer_height),
    ));

    if y1 != y2 {
        for y in (1..y2 - y1).rev() {
            // Draw background behind the rows in between.
            named_args!(scope.draw_rect(
                color,
                top_left = Offset::new(0.0, start_y + (y as f32 * item_container_height)),
                size = Size::new(width, item_state_layer_height),
            ));
        }
        // Draw the last row selection background
        let top_left_x = if scope.layout_direction() == LayoutDirection::Ltr { 0.0 } else { width };
        named_args!(scope.draw_rect(
            color,
            top_left = Offset::new(top_left_x, end_y),
            size = Size::new(if is_rtl { end_x - width } else { end_x }, item_state_layer_height),
        ));
    }
}

/// The month names' actions scrolling a month up or down (Compose's private `customScrollActions`): refused at
/// either end.
fn custom_scroll_actions(
    state: &LazyListState,
    coroutine_scope: &CoroutineScope,
    scroll_up_label: String,
    scroll_down_label: String,
) -> Vec<CustomAccessibilityAction> {
    let (up_state, up_scope) = (state.clone(), coroutine_scope.clone());
    let scroll_up_action = move || {
        if !up_state.can_scroll_backward() {
            false
        } else {
            let state = up_state.clone();
            named_args!(
                up_scope.launch(async move { state.scroll_to_item(state.first_visible_item_index() - 1, 0).await })
            );
            true
        }
    };
    let (down_state, down_scope) = (state.clone(), coroutine_scope.clone());
    let scroll_down_action = move || {
        if !down_state.can_scroll_forward() {
            false
        } else {
            let state = down_state.clone();
            named_args!(
                down_scope.launch(async move { state.scroll_to_item(state.first_visible_item_index() + 1, 0).await })
            );
            true
        }
    };
    vec![
        CustomAccessibilityAction::new(scroll_up_label, scroll_up_action),
        CustomAccessibilityAction::new(scroll_down_label, scroll_down_action),
    ]
}

fn DateRangePickerTitlePadding() -> PaddingValues {
    PaddingValues::new(Dp(64.0), Dp(0.0), Dp(12.0), Dp(0.0))
}

fn DateRangePickerHeadlinePadding() -> PaddingValues {
    PaddingValues::new(Dp(64.0), Dp(0.0), Dp(12.0), Dp(12.0))
}

// An offset that is applied to the token value for the RangeSelectionHeaderContainerHeight. The implementation does
// not render a "Save" and "X" buttons by default, so we don't take those into account when setting the header's max
// height.
const HeaderHeightOffset: Dp = Dp(60.0);

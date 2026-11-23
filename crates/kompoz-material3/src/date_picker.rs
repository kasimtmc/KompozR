//! Material's date picker, as Compose's DatePicker.kt (material3 1.5.0-alpha17; Compose for desktop's actuals):
//! [`DatePicker`] — a header (a title, the picked date as the headline, a button switching to typing the date) over
//! either a calendar (a month at a time, swiped or stepped through, a grid of years behind the month's name) or a
//! text field the date is typed in ([`DisplayMode`]). Its [`DatePickerState`] holds the picked date and the month
//! shown as UTC milliseconds ([`rememberDatePickerState`](remember_date_picker_state)), limited to a range of years
//! and to the [`SelectableDates`]; dates are written as the state's [`CalendarLocale`] writes them
//! ([`DatePickerFormatter`]), in the colors of [`DatePickerColors`].

use std::any::Any;
use std::fmt;
use std::ops::RangeInclusive;
use std::rc::Rc;
use std::sync::Arc;

use kompoz_animation::{
    AnimatedContent, AnimatedContentScope, AnimatedContentTransitionScope, AnimatedVisibility, AnimatedVisibilityScope,
    FiniteAnimationSpec, SizeTransform, animate_color_as_state, expand_vertically, fade_in, fade_out,
    shrink_vertically, slide_in_vertically, slide_out_vertically,
};
use kompoz_animation::{DecayAnimationSpec, exponential_decay};
use kompoz_coroutines::{CoroutineScope, LaunchedEffect, remember_coroutine_scope, snapshot_flow};
use kompoz_foundation::gestures::FlingBehavior;
use kompoz_foundation::gestures::snapping::{
    SnapLayoutInfoProvider, lazy_list_snap_layout_info_provider, snap_fling_behavior,
};
use kompoz_foundation::lazy::LazyListState;
use kompoz_foundation::lazy::grid::{GridCells, LazyVerticalGrid, remember_lazy_grid_state};
use kompoz_foundation::prelude::*;
use kompoz_graphics::LayoutDirection;
use kompoz_graphics::vector::ImageVector;
use kompoz_graphics::{BorderStroke, CircleShape, Color, Dp, IntOffset, IntSize, Shape};
use kompoz_runtime::{
    KompozitionLocalProvider, KotlinProperties, MutableState, SavedValue, Saver, SnapshotMutationPolicy, State,
    kompozable, kompozable_block, list_saver, mutable_confined_state_of_with_policy, mutable_state_of, named,
    named_args, remember, remember_keyed, remember_saveable, remember_saveable_with, remember_updated_state,
};
use kompoz_text::AnnotatedString;
use kompoz_text::TextStyle;
use kompoz_text::style::TextAlign;
use kompoz_ui::focus::{FocusDirection, FocusManager, FocusRequester};
use kompoz_ui::input::{Key, KeyEvent, KeyEventType};
use kompoz_ui::platform::{LocalDensity, LocalFocusManager, LocalLayoutDirection};
use kompoz_ui::semantics::{LiveRegionMode, Role, ScrollAxisRange};
use kompoz_ui::{Alignment, Modifier};

use crate::button::{ButtonDefaults, TextButton};
use crate::calendar_locale::{CalendarLocale, default_locale, to_local_string_min};
use crate::color_scheme::{ColorScheme, DisabledAlpha};
use crate::content_color::LocalContentColor;
use crate::date_input::DateInputContent;
use crate::date_range_picker::{SelectedRangeInfo, draw_range_background};
use crate::divider::{DividerDefaults, HorizontalDivider};
use crate::icon::VectorIcon;
use crate::icon_button::IconButton;
use crate::interactive_component_size::LocalMinimumInteractiveComponentSize;
use crate::internal::calendar_model::{
    CalendarModel, CalendarMonth, DaysInWeek, FormatterCache, MillisecondsIn24Hours, create_calendar_model,
    format_with_skeleton,
};
use crate::internal::icons::Icons;
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::internal::strings::{Strings, format_string, get_string};
use crate::material_theme::MaterialTheme;
use crate::surface::SelectableSurface;
use crate::text::{ProvideTextStyle, Text};
use crate::text_field_defaults::{TextFieldColors, default_outlined_text_field_colors};
use crate::tokens::{DatePickerModalTokens, DividerTokens, ElevationTokens, MotionSchemeKeyTokens};
use crate::tooltip::{
    TooltipAnchorPosition, TooltipBox, TooltipDefaults, TooltipScope, TooltipScopeComposables, remember_tooltip_state,
};

// ---- DatePicker ------------------------------------------------------------------------------------------------

/// A date picker (Compose's `DatePicker`): the header — `title` (None is Kotlin's default,
/// [`DatePickerDefaults::DatePickerTitle`]; `Some(None)` none), `headline` (None is the default,
/// [`DatePickerDefaults::DatePickerHeadline`] of the picked date; `Some(None)` none) and, if `show_mode_toggle`, a
/// button switching between the calendar and the text field — over the calendar or the text field of the state's
/// [`DisplayMode`]. Dates are formatted by `date_formatter` in the state's locale; the text field takes the focus of
/// `focus_requester` when it shows (`None`: it doesn't).
///
/// Kotlin: `DatePicker(state: DatePickerState, modifier: Modifier = Modifier, dateFormatter: DatePickerFormatter = remember { DatePickerDefaults.dateFormatter() }, colors: DatePickerColors = DatePickerDefaults.colors(), title: (@Composable () -> Unit)? = { … }, headline: (@Composable () -> Unit)? = { … }, showModeToggle: Boolean = true, focusRequester: FocusRequester? = remember { FocusRequester() })`
#[kompozable]
pub fn DatePicker(
    state: Rc<dyn DatePickerState>,
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
            DatePickerDefaults.DatePickerTitle(
                display_mode = state.display_mode(),
                modifier = Modifier.padding_values(DatePickerTitlePadding()),
                content_color = colors.title_content_color,
            )
        }) as Rc<dyn Fn()>)
    });
    let headline = headline.unwrap_or_else(|| {
        let (state, date_formatter) = (state.clone(), date_formatter.clone());
        Some(Rc::new(move || {
            DatePickerDefaults.DatePickerHeadline(
                selected_date_millis = state.selected_date_millis(),
                display_mode = state.display_mode(),
                date_formatter = date_formatter.clone(),
                modifier = Modifier.padding_values(DatePickerHeadlinePadding()),
                content_color = colors.headline_content_color,
            )
        }) as Rc<dyn Fn()>)
    });
    let locale = state.locale();
    let calendar_model = {
        let state = state.clone();
        remember_keyed(locale.clone(), move || match state.calendar_model_handle() {
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
        DatePickerModalTokens::HeaderHeadlineFont.value(),
        DatePickerModalTokens::HeaderContainerHeight,
        move || {
            let state = content_state.clone();
            let (on_select, on_month) = (state.clone(), state.clone());
            SwitchableDateEntryContent(
                state.selected_date_millis(),
                state.displayed_month_millis(),
                state.display_mode(),
                Rc::new(move |date_in_millis| on_select.set_selected_date_millis(date_in_millis)),
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

// ---- DatePickerState -------------------------------------------------------------------------------------------

/// The state of a [`DatePicker`] (Compose's interface `DatePickerState`): the picked date and the month shown, as
/// UTC milliseconds since the epoch at their days' (months' first days') midnight, the display mode, the years it
/// is limited to, which dates may be picked and the locale dates are formatted in.
/// [`remember_date_picker_state`] or [`date_picker_state`] make one.
pub trait DatePickerState {
    /// The picked date's UTC milliseconds, if one is picked (Compose's `selectedDateMillis`).
    fn selected_date_millis(&self) -> Option<i64>;

    /// Picks the date of the UTC milliseconds given (their day's midnight), none if `None` or outside the year
    /// range (Compose's `selectedDateMillis` setter).
    fn set_selected_date_millis(&self, date_millis: Option<i64>);

    /// The UTC milliseconds of the first day of the month shown (Compose's `displayedMonthMillis`).
    fn displayed_month_millis(&self) -> i64;

    /// Shows the month the UTC milliseconds given are in, if it is in the year range (Compose's
    /// `displayedMonthMillis` setter).
    fn set_displayed_month_millis(&self, month_millis: i64);

    /// Whether the calendar or the text field shows (Compose's `displayMode`).
    fn display_mode(&self) -> DisplayMode;

    /// Shows the calendar or the text field, the picked date's month first (Compose's `displayMode` setter).
    fn set_display_mode(&self, display_mode: DisplayMode);

    /// The years dates may be picked in (Compose's `yearRange`).
    fn year_range(&self) -> RangeInclusive<i32>;

    /// Which dates may be picked (Compose's `selectableDates`).
    fn selectable_dates(&self) -> Rc<dyn SelectableDates>;

    /// The locale dates are formatted in (Compose's `locale`).
    fn locale(&self) -> CalendarLocale;

    /// The calendar a state of the crate counts in (Compose's `state is BaseDatePickerStateImpl` check in
    /// [`DatePicker`]); a state of the app's own has none (the picker makes one of its locale).
    #[doc(hidden)]
    fn calendar_model_handle(&self) -> Option<CalendarModelHandle> {
        None
    }
}

impl PartialEq for dyn DatePickerState {
    /// The same state (Kotlin's `==` of a class without `equals`: its identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// A state's calendar ([`DatePickerState::calendar_model_handle`]): opaque outside the crate.
#[doc(hidden)]
#[derive(Clone)]
pub struct CalendarModelHandle(pub(crate) Rc<dyn CalendarModel>);

/// Which dates a date picker lets the user pick (Compose's interface `SelectableDates`): all by default. Others
/// show disabled.
pub trait SelectableDates {
    /// Whether the date of the UTC milliseconds given (its midnight) may be picked (Compose's `isSelectableDate`).
    fn is_selectable_date(&self, utc_time_millis: i64) -> bool {
        let _ = utc_time_millis;
        true
    }

    /// Whether dates in `year` may be picked (Compose's `isSelectableYear`).
    fn is_selectable_year(&self, year: i32) -> bool {
        let _ = year;
        true
    }
}

/// How a date picker writes months and dates (Compose's interface `DatePickerFormatter`);
/// [`DatePickerDefaults::date_formatter`] makes the default one.
pub trait DatePickerFormatter {
    /// The month of the UTC milliseconds given and its year, as the year picker's button shows them (Compose's
    /// `formatMonthYear`).
    fn format_month_year(&self, month_millis: Option<i64>, locale: &CalendarLocale) -> Option<String>;

    /// The date of the UTC milliseconds given, as the headline shows it, or for a screen reader if
    /// `for_content_description` (Compose's `formatDate`; Kotlin's `forContentDescription = false` default is
    /// [`format_date_default`](DatePickerFormatter::format_date_default)).
    fn format_date(
        &self,
        date_millis: Option<i64>,
        locale: &CalendarLocale,
        for_content_description: bool,
    ) -> Option<String>;

    /// [`format_date`](DatePickerFormatter::format_date) not for a screen reader (Kotlin's
    /// `formatDate(dateMillis, locale)`).
    fn format_date_default(&self, date_millis: Option<i64>, locale: &CalendarLocale) -> Option<String> {
        self.format_date(date_millis, locale, false)
    }

    /// What tells the default formatter's equal ones (Kotlin's `equals` of its class): `None` for others.
    #[doc(hidden)]
    fn as_any(&self) -> Option<&dyn Any> {
        None
    }
}

impl PartialEq for dyn DatePickerFormatter {
    /// Default formatters of the same skeletons are equal (Kotlin's `equals` of Compose's
    /// `DatePickerFormatterImpl`); others are the same formatter.
    fn eq(&self, other: &Self) -> bool {
        match (
            self.as_any().and_then(|a| a.downcast_ref::<DatePickerFormatterImpl>()),
            other.as_any().and_then(|a| a.downcast_ref::<DatePickerFormatterImpl>()),
        ) {
            (Some(a), Some(b)) => a == b,
            _ => std::ptr::addr_eq(self, other),
        }
    }
}

/// Whether a date picker shows its calendar or its text field (Compose's value class `DisplayMode`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DisplayMode(pub(crate) i32);

impl DisplayMode {
    /// The calendar, picked from.
    pub const Picker: DisplayMode = DisplayMode(0);
    /// The text field, typed in.
    pub const Input: DisplayMode = DisplayMode(1);
}

/// Compose's toString: the mode's name.
impl fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            DisplayMode::Picker => "Picker",
            DisplayMode::Input => "Input",
            _ => "Unknown",
        })
    }
}

impl fmt::Debug for DisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// A [`DatePickerState`] remembered across recompositions and saved with the UI's state (Compose's
/// `rememberDatePickerState`), in the platform's locale: `initial_selected_date_millis` picked (none if `None` or
/// outside `year_range`), the month of `initial_displayed_month_millis` shown (None is Kotlin's default, the
/// picked date's; `Some(None)` the current month; the current month too if outside `year_range`),
/// `initial_display_mode`, `selectable_dates` (updated each recomposition).
///
/// Kotlin: `rememberDatePickerState(initialSelectedDateMillis: Long? = null, initialDisplayedMonthMillis: Long? = initialSelectedDateMillis, yearRange: IntRange = DatePickerDefaults.YearRange, initialDisplayMode: DisplayMode = DisplayMode.Picker, selectableDates: SelectableDates = DatePickerDefaults.AllDates)`
#[kompozable]
pub fn remember_date_picker_state(
    #[default(None)] initial_selected_date_millis: Option<i64>,
    #[default(None)] initial_displayed_month_millis: Option<Option<i64>>,
    #[default(DatePickerDefaults::YearRange())] year_range: RangeInclusive<i32>,
    #[default(DisplayMode::Picker)] initial_display_mode: DisplayMode,
    #[default(DatePickerDefaults::AllDates())] selectable_dates: Rc<dyn SelectableDates>,
) -> Rc<dyn DatePickerState> {
    let initial_displayed_month_millis = initial_displayed_month_millis.unwrap_or(initial_selected_date_millis);
    let locale = default_locale();
    let saver = DatePickerStateImpl::Saver(selectable_dates.clone(), locale.clone());
    let state = {
        let (selectable_dates, locale) = (selectable_dates.clone(), locale);
        remember_saveable_with(&saver, move || {
            Rc::new(DatePickerStateImpl::new(
                initial_selected_date_millis,
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

/// A [`DatePickerState`] in `locale` (Compose's factory function `DatePickerState(locale, …)`), made outside composition: as
/// [`remember_date_picker_state`] says.
///
/// Kotlin: `DatePickerState(locale: CalendarLocale, initialSelectedDateMillis: Long? = null, initialDisplayedMonthMillis: Long? = initialSelectedDateMillis, yearRange: IntRange = DatePickerDefaults.YearRange, initialDisplayMode: DisplayMode = DisplayMode.Picker, selectableDates: SelectableDates = DatePickerDefaults.AllDates)`
#[named]
pub fn date_picker_state(
    locale: CalendarLocale,
    #[default(None)] initial_selected_date_millis: Option<i64>,
    #[default(None)] initial_displayed_month_millis: Option<Option<i64>>,
    #[default(DatePickerDefaults::YearRange())] year_range: RangeInclusive<i32>,
    #[default(DisplayMode::Picker)] initial_display_mode: DisplayMode,
    #[default(DatePickerDefaults::AllDates())] selectable_dates: Rc<dyn SelectableDates>,
) -> Rc<dyn DatePickerState> {
    Rc::new(DatePickerStateImpl::new(
        initial_selected_date_millis,
        initial_displayed_month_millis.unwrap_or(initial_selected_date_millis),
        year_range,
        initial_display_mode,
        selectable_dates,
        locale,
    ))
}

/// What the date pickers' states share (Compose's internal abstract class `BaseDatePickerStateImpl`): the year
/// range, the calendar of the locale, the selectable dates, the month shown.
pub(crate) struct BaseDatePickerStateImpl {
    pub(crate) year_range: RangeInclusive<i32>,
    pub(crate) locale: CalendarLocale,
    pub(crate) calendar_model: Rc<dyn CalendarModel>,
    /// Compose's `var selectableDates by mutableStateOf(selectableDates)` (confined: an app's `SelectableDates`
    /// needn't be `Send`; another object is a new value, as Kotlin's `==` of objects).
    selectable_dates: MutableState<Rc<dyn SelectableDates>>,
    displayed_month: MutableState<CalendarMonth>,
}

impl BaseDatePickerStateImpl {
    pub(crate) fn new(
        initial_displayed_month_millis: Option<i64>,
        year_range: RangeInclusive<i32>,
        selectable_dates: Rc<dyn SelectableDates>,
        locale: CalendarLocale,
    ) -> BaseDatePickerStateImpl {
        let calendar_model = create_calendar_model(locale.clone());
        let displayed_month = match initial_displayed_month_millis {
            Some(millis) => {
                let month = calendar_model.get_month(millis);
                if year_range.contains(&month.year) {
                    month
                } else {
                    // The initial display month's year is out of the years range, so just set the displayed month
                    // to the current one.
                    calendar_model.get_month_of_date(&calendar_model.today())
                }
            }
            // Set the displayed month to the current one.
            None => calendar_model.get_month_of_date(&calendar_model.today()),
        };
        BaseDatePickerStateImpl {
            year_range,
            locale,
            calendar_model,
            selectable_dates: mutable_confined_state_of_with_policy(
                selectable_dates,
                SnapshotMutationPolicy::new(|a, b| std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b))),
            ),
            displayed_month: mutable_state_of(displayed_month),
        }
    }

    pub(crate) fn selectable_dates(&self) -> Rc<dyn SelectableDates> {
        self.selectable_dates.get()
    }

    pub(crate) fn set_selectable_dates(&self, selectable_dates: Rc<dyn SelectableDates>) {
        self.selectable_dates.set(selectable_dates);
    }

    pub(crate) fn displayed_month_millis(&self) -> i64 {
        self.displayed_month.get().start_utc_time_millis
    }

    pub(crate) fn set_displayed_month_millis(&self, month_millis: i64) {
        let month = self.calendar_model.get_month(month_millis);
        // Set the displayed month only if the month's year is within the years range.
        if self.year_range.contains(&month.year) {
            self.displayed_month.set(month);
        }
    }
}

/// The default [`DatePickerState`] (Compose's private `DatePickerStateImpl`).
pub(crate) struct DatePickerStateImpl {
    pub(crate) base: BaseDatePickerStateImpl,
    selected_date: MutableState<Option<crate::internal::calendar_model::CalendarDate>>,
    display_mode: MutableState<DisplayMode>,
}

impl DatePickerStateImpl {
    pub(crate) fn new(
        initial_selected_date_millis: Option<i64>,
        initial_displayed_month_millis: Option<i64>,
        year_range: RangeInclusive<i32>,
        initial_display_mode: DisplayMode,
        selectable_dates: Rc<dyn SelectableDates>,
        locale: CalendarLocale,
    ) -> DatePickerStateImpl {
        let base = BaseDatePickerStateImpl::new(initial_displayed_month_millis, year_range, selectable_dates, locale);
        let selected_date = initial_selected_date_millis.and_then(|millis| {
            let date = base.calendar_model.get_canonical_date(millis);
            // If the provided initial date's year is out of the years range, return null. Otherwise, return the
            // date.
            base.year_range.contains(&date.year).then_some(date)
        });
        DatePickerStateImpl {
            base,
            selected_date: mutable_state_of(selected_date),
            display_mode: mutable_state_of(initial_display_mode),
        }
    }

    /// Saves the picked date, the month shown, the year range and the display mode (Compose's companion's
    /// `Saver(selectableDates, locale)`).
    pub(crate) fn Saver(
        selectable_dates: Rc<dyn SelectableDates>,
        locale: CalendarLocale,
    ) -> Saver<Rc<DatePickerStateImpl>> {
        list_saver(
            |it: &Rc<DatePickerStateImpl>| {
                vec![
                    it.selected_date_millis().map(SavedValue::Int).unwrap_or(SavedValue::Null),
                    SavedValue::Int(it.displayed_month_millis()),
                    SavedValue::Int(*it.base.year_range.start() as i64),
                    SavedValue::Int(*it.base.year_range.end() as i64),
                    SavedValue::Int(it.display_mode().0 as i64),
                ]
            },
            move |value: &[SavedValue]| {
                Some(Rc::new(DatePickerStateImpl::new(
                    value.first()?.as_int(),
                    value.get(1)?.as_int(),
                    value.get(2)?.as_int()? as i32..=value.get(3)?.as_int()? as i32,
                    DisplayMode(value.get(4)?.as_int()? as i32),
                    selectable_dates.clone(),
                    locale.clone(),
                )))
            },
        )
    }
}

impl DatePickerState for DatePickerStateImpl {
    fn selected_date_millis(&self) -> Option<i64> {
        self.selected_date.get().map(|date| date.utc_time_millis)
    }

    fn set_selected_date_millis(&self, date_millis: Option<i64>) {
        match date_millis {
            Some(millis) => {
                let date = self.base.calendar_model.get_canonical_date(millis);
                // Validate that the give date is within the valid years range. In not, set the selected date to
                // null.
                self.selected_date.set(self.base.year_range.contains(&date.year).then_some(date));
            }
            None => self.selected_date.set(None),
        }
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
        if let Some(millis) = self.selected_date_millis() {
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

    fn calendar_model_handle(&self) -> Option<CalendarModelHandle> {
        Some(CalendarModelHandle(self.base.calendar_model.clone()))
    }
}

// ---- DatePickerFormatterImpl ----------------------------------------------------------------------------------

/// The default [`DatePickerFormatter`] (Compose's private `DatePickerFormatterImpl`): months and dates by skeletons.
pub(crate) struct DatePickerFormatterImpl {
    year_selection_skeleton: String,
    selected_date_skeleton: String,
    selected_date_description_skeleton: String,
    // A map for caching formatter related results for better performance.
    formatter_cache: FormatterCache,
}

impl PartialEq for DatePickerFormatterImpl {
    fn eq(&self, other: &Self) -> bool {
        self.year_selection_skeleton == other.year_selection_skeleton
            && self.selected_date_skeleton == other.selected_date_skeleton
            && self.selected_date_description_skeleton == other.selected_date_description_skeleton
    }
}

impl DatePickerFormatter for DatePickerFormatterImpl {
    fn format_month_year(&self, month_millis: Option<i64>, locale: &CalendarLocale) -> Option<String> {
        let month_millis = month_millis?;
        Some(format_with_skeleton(month_millis, &self.year_selection_skeleton, locale, &self.formatter_cache))
    }

    fn format_date(
        &self,
        date_millis: Option<i64>,
        locale: &CalendarLocale,
        for_content_description: bool,
    ) -> Option<String> {
        let date_millis = date_millis?;
        let skeleton = if for_content_description {
            &self.selected_date_description_skeleton
        } else {
            &self.selected_date_skeleton
        };
        Some(format_with_skeleton(date_millis, skeleton, locale, &self.formatter_cache))
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }
}

// ---- DatePickerDefaults ----------------------------------------------------------------------------------------

/// What date pickers are made of by default (Compose's `DatePickerDefaults`).
#[derive(Clone, Copy, Debug)]
pub struct DatePickerDefaults;

/// All dates selectable (Compose's `DatePickerDefaults.AllDates` object).
struct AllDates;

impl SelectableDates for AllDates {}

impl DatePickerDefaults {
    /// The years a date picker covers by default, 1900 to 2100 (Compose's `YearRange`).
    pub fn YearRange() -> RangeInclusive<i32> {
        1900..=2100
    }

    /// A date picker's tonal elevation, none (Compose's `TonalElevation`).
    pub const TonalElevation: Dp = ElevationTokens::Level0;

    /// Every date selectable (Compose's `AllDates`): one object, the same each time.
    pub fn AllDates() -> Rc<dyn SelectableDates> {
        thread_local! {
            static ALL_DATES: Rc<dyn SelectableDates> = Rc::new(AllDates);
        }
        ALL_DATES.with(Rc::clone)
    }

    /// The skeleton of the year picker's button: the month's full name and the year (Compose's
    /// `YearMonthSkeleton`).
    pub const YearMonthSkeleton: &'static str = "yMMMM";

    /// The skeleton of the headline's date: the month abbreviated (Compose's `YearAbbrMonthDaySkeleton`).
    pub const YearAbbrMonthDaySkeleton: &'static str = "yMMMd";

    /// The skeleton of dates read out: the weekday and the month's full name (Compose's
    /// `YearMonthWeekdayDaySkeleton`).
    pub const YearMonthWeekdayDaySkeleton: &'static str = "yMMMMEEEEd";

    /// A date picker's shape (Compose's `shape`).
    pub fn shape() -> Arc<dyn Shape> {
        DatePickerModalTokens::ContainerShape.value()
    }

    /// A date picker's default colors in `scheme` (Compose's internal composable `ColorScheme.defaultDatePickerColors`:
    /// the text field's read the selection colors provided).
    #[track_caller]
    pub(crate) fn default_date_picker_colors(scheme: &ColorScheme) -> DatePickerColors {
        let t = |token| scheme.from_token(token);
        let disabled = |c: Color| named_args!(c.copy(alpha = DisabledAlpha));
        DatePickerColors {
            container_color: t(DatePickerModalTokens::ContainerColor),
            title_content_color: t(DatePickerModalTokens::HeaderSupportingTextColor),
            headline_content_color: t(DatePickerModalTokens::HeaderHeadlineColor),
            weekday_content_color: t(DatePickerModalTokens::WeekdaysLabelTextColor),
            subhead_content_color: t(DatePickerModalTokens::RangeSelectionMonthSubheadColor),
            // TODO(b/234060211): Apply this from the MenuButton tokens or defaults.
            navigation_content_color: scheme.on_surface_variant,
            year_content_color: t(DatePickerModalTokens::SelectionYearUnselectedLabelTextColor),
            // TODO: Using DisabledAlpha as there are no token values for the disabled states.
            disabled_year_content_color: disabled(t(DatePickerModalTokens::SelectionYearUnselectedLabelTextColor)),
            current_year_content_color: t(DatePickerModalTokens::DateTodayLabelTextColor),
            selected_year_content_color: t(DatePickerModalTokens::SelectionYearSelectedLabelTextColor),
            disabled_selected_year_content_color: disabled(t(
                DatePickerModalTokens::SelectionYearSelectedLabelTextColor,
            )),
            selected_year_container_color: t(DatePickerModalTokens::SelectionYearSelectedContainerColor),
            disabled_selected_year_container_color: disabled(t(
                DatePickerModalTokens::SelectionYearSelectedContainerColor,
            )),
            day_content_color: t(DatePickerModalTokens::DateUnselectedLabelTextColor),
            disabled_day_content_color: disabled(t(DatePickerModalTokens::DateUnselectedLabelTextColor)),
            selected_day_content_color: t(DatePickerModalTokens::DateSelectedLabelTextColor),
            disabled_selected_day_content_color: disabled(t(DatePickerModalTokens::DateSelectedLabelTextColor)),
            selected_day_container_color: t(DatePickerModalTokens::DateSelectedContainerColor),
            disabled_selected_day_container_color: disabled(t(DatePickerModalTokens::DateSelectedContainerColor)),
            today_content_color: t(DatePickerModalTokens::DateTodayLabelTextColor),
            today_date_border_color: t(DatePickerModalTokens::DateTodayContainerOutlineColor),
            day_in_selection_range_content_color: t(DatePickerModalTokens::SelectionDateInRangeLabelTextColor),
            day_in_selection_range_container_color: t(
                DatePickerModalTokens::RangeSelectionActiveIndicatorContainerColor,
            ),
            divider_color: t(DividerTokens::Color),
            date_text_field_colors: default_outlined_text_field_colors(scheme, LocalTextSelectionColors.current()),
        }
    }
}

#[named]
#[allow(clippy::too_many_arguments)]
impl DatePickerDefaults {
    /// A date picker's colors, each given one other than unspecified in place of the theme's (Compose's
    /// `colors()` and `colors(containerColor, …, dateTextFieldColors)`; `date_text_field_colors` None is the
    /// theme's outlined text field colors).
    #[track_caller]
    pub fn colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] title_content_color: Color,
        #[default(Color::Unspecified)] headline_content_color: Color,
        #[default(Color::Unspecified)] weekday_content_color: Color,
        #[default(Color::Unspecified)] subhead_content_color: Color,
        #[default(Color::Unspecified)] navigation_content_color: Color,
        #[default(Color::Unspecified)] year_content_color: Color,
        #[default(Color::Unspecified)] disabled_year_content_color: Color,
        #[default(Color::Unspecified)] current_year_content_color: Color,
        #[default(Color::Unspecified)] selected_year_content_color: Color,
        #[default(Color::Unspecified)] disabled_selected_year_content_color: Color,
        #[default(Color::Unspecified)] selected_year_container_color: Color,
        #[default(Color::Unspecified)] disabled_selected_year_container_color: Color,
        #[default(Color::Unspecified)] day_content_color: Color,
        #[default(Color::Unspecified)] disabled_day_content_color: Color,
        #[default(Color::Unspecified)] selected_day_content_color: Color,
        #[default(Color::Unspecified)] disabled_selected_day_content_color: Color,
        #[default(Color::Unspecified)] selected_day_container_color: Color,
        #[default(Color::Unspecified)] disabled_selected_day_container_color: Color,
        #[default(Color::Unspecified)] today_content_color: Color,
        #[default(Color::Unspecified)] today_date_border_color: Color,
        #[default(Color::Unspecified)] day_in_selection_range_content_color: Color,
        #[default(Color::Unspecified)] day_in_selection_range_container_color: Color,
        #[default(Color::Unspecified)] divider_color: Color,
        #[default(None)] date_text_field_colors: Option<TextFieldColors>,
    ) -> DatePickerColors {
        named_args!(Self::default_date_picker_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            title_content_color = title_content_color,
            headline_content_color = headline_content_color,
            weekday_content_color = weekday_content_color,
            subhead_content_color = subhead_content_color,
            navigation_content_color = navigation_content_color,
            year_content_color = year_content_color,
            disabled_year_content_color = disabled_year_content_color,
            current_year_content_color = current_year_content_color,
            selected_year_content_color = selected_year_content_color,
            disabled_selected_year_content_color = disabled_selected_year_content_color,
            selected_year_container_color = selected_year_container_color,
            disabled_selected_year_container_color = disabled_selected_year_container_color,
            day_content_color = day_content_color,
            disabled_day_content_color = disabled_day_content_color,
            selected_day_content_color = selected_day_content_color,
            disabled_selected_day_content_color = disabled_selected_day_content_color,
            selected_day_container_color = selected_day_container_color,
            disabled_selected_day_container_color = disabled_selected_day_container_color,
            today_content_color = today_content_color,
            today_date_border_color = today_date_border_color,
            day_in_selection_range_content_color = day_in_selection_range_content_color,
            day_in_selection_range_container_color = day_in_selection_range_container_color,
            divider_color = divider_color,
            date_text_field_colors = date_text_field_colors,
        ))
    }

    /// The default [`DatePickerFormatter`], of the skeletons given (Compose's `dateFormatter`).
    pub fn date_formatter(
        &self,
        #[default(DatePickerDefaults::YearMonthSkeleton.to_string())] year_selection_skeleton: String,
        #[default(DatePickerDefaults::YearAbbrMonthDaySkeleton.to_string())] selected_date_skeleton: String,
        #[default(DatePickerDefaults::YearMonthWeekdayDaySkeleton.to_string())]
        selected_date_description_skeleton: String,
    ) -> Rc<dyn DatePickerFormatter> {
        Rc::new(DatePickerFormatterImpl {
            year_selection_skeleton,
            selected_date_skeleton,
            selected_date_description_skeleton,
            formatter_cache: FormatterCache::default(),
        })
    }

    /// A date picker's title (Compose's `DatePickerTitle`): "Select date" for the calendar, "Enter date" for the
    /// text field, in `content_color` (the theme's title color by default).
    #[track_caller]
    pub fn DatePickerTitle(
        &self,
        display_mode: DisplayMode,
        #[default(Modifier)] modifier: Modifier,
        #[default(DatePickerDefaults.colors().title_content_color)] content_color: Color,
    ) {
        kompozable_block!({
            match display_mode {
                DisplayMode::Picker => {
                    Text(get_string(Strings::DatePickerTitle), modifier = modifier, color = content_color)
                }
                DisplayMode::Input => {
                    Text(get_string(Strings::DateInputTitle), modifier = modifier, color = content_color)
                }
                _ => {}
            }
        })
    }

    /// A date picker's headline (Compose's `DatePickerHeadline`): the picked date (or "Selected date" / "Entered
    /// date" if none) on one line, read out politely as it changes with the full date (or "No selection" / "No
    /// input").
    #[track_caller]
    pub fn DatePickerHeadline(
        &self,
        selected_date_millis: Option<i64>,
        display_mode: DisplayMode,
        date_formatter: Rc<dyn DatePickerFormatter>,
        #[default(Modifier)] modifier: Modifier,
        #[default(DatePickerDefaults.colors().headline_content_color)] content_color: Color,
    ) {
        let locale = default_locale();
        let formatted_date = date_formatter.format_date(selected_date_millis, &locale, false);
        let verbose_date_description =
            date_formatter.format_date(selected_date_millis, &locale, true).unwrap_or_else(|| match display_mode {
                DisplayMode::Picker => get_string(Strings::DatePickerNoSelectionDescription),
                DisplayMode::Input => get_string(Strings::DateInputNoInputDescription),
                _ => String::new(),
            });
        let headline_text = formatted_date.unwrap_or_else(|| match display_mode {
            DisplayMode::Picker => get_string(Strings::DatePickerHeadline),
            DisplayMode::Input => get_string(Strings::DateInputHeadline),
            _ => String::new(),
        });
        let headline_description = format_headline_description(
            &match display_mode {
                DisplayMode::Picker => get_string(Strings::DatePickerHeadlineDescription),
                DisplayMode::Input => get_string(Strings::DateInputHeadlineDescription),
                _ => String::new(),
            },
            &verbose_date_description,
        );
        kompozable_block!({
            Text(
                headline_text,
                modifier = modifier.semantics(move |c| {
                    c.set_live_region(LiveRegionMode::Polite);
                    c.set_content_description(headline_description.clone());
                }),
                color = content_color,
                max_lines = 1,
            );
        })
    }

    /// The months list's fling: snapping to a month, with no approach (Compose's internal
    /// `rememberSnapFlingBehavior`).
    #[track_caller]
    pub(crate) fn remember_snap_fling_behavior(
        &self,
        lazy_list_state: LazyListState,
        #[default(Arc::new(exponential_decay::<f32>()))] decay_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
    ) -> Rc<dyn FlingBehavior> {
        // TODO Load the motionScheme tokens from the component tokens file
        let animation_spec: Arc<dyn FiniteAnimationSpec<f32>> = MotionSchemeKeyTokens::DefaultEffects.value::<f32>();
        remember_keyed((DecayKey(decay_animation_spec.clone()), lazy_list_state.clone()), move || {
            let original = lazy_list_snap_layout_info_provider(&lazy_list_state);
            let snap_layout_info_provider: Rc<dyn SnapLayoutInfoProvider> = Rc::new(NoApproach(original));
            snap_fling_behavior(snap_layout_info_provider, decay_animation_spec, animation_spec)
                as Rc<dyn FlingBehavior>
        })
    }
}

/// A decay spec as a remember key: the same spec object.
struct DecayKey(Arc<dyn DecayAnimationSpec<f32>>);

impl PartialEq for DecayKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Compose's `object : SnapLayoutInfoProvider by original { override fun calculateApproachOffset(…) = 0f }`.
struct NoApproach(Rc<dyn SnapLayoutInfoProvider>);

impl SnapLayoutInfoProvider for NoApproach {
    fn calculate_approach_offset(&self, _velocity: f32, _decay_offset: f32) -> f32 {
        0.0
    }

    fn calculate_snap_offset(&self, velocity: f32) -> f32 {
        self.0.calculate_snap_offset(velocity)
    }
}

/// The headline's description: its template with the date in (Compose's internal `formatHeadlineDescription`;
/// the JVM's actual `template.format(verboseDateDescription)`).
pub(crate) fn format_headline_description(template: &str, verbose_date_description: &str) -> String {
    format_string(template, &[&verbose_date_description])
}

/// The year's description: its template with the year in (Compose's internal
/// `formatDatePickerNavigateToYearString`; the JVM's actual `template.format(localizedYear)`).
pub(crate) fn format_date_picker_navigate_to_year_string(template: &str, localized_year: &str) -> String {
    format_string(template, &[&localized_year])
}

// ---- DatePickerColors ------------------------------------------------------------------------------------------

/// A date picker's colors (Compose's `DatePickerColors`), made by [`DatePickerDefaults::colors`]. Equal by its
/// colors but the navigation's, the divider's and the text field's (Kotlin's `equals` of the class).
#[derive(Clone, Copy, Debug, KotlinProperties)]
pub struct DatePickerColors {
    pub container_color: Color,
    pub title_content_color: Color,
    pub headline_content_color: Color,
    pub weekday_content_color: Color,
    pub subhead_content_color: Color,
    pub navigation_content_color: Color,
    pub year_content_color: Color,
    pub disabled_year_content_color: Color,
    pub current_year_content_color: Color,
    pub selected_year_content_color: Color,
    pub disabled_selected_year_content_color: Color,
    pub selected_year_container_color: Color,
    pub disabled_selected_year_container_color: Color,
    pub day_content_color: Color,
    pub disabled_day_content_color: Color,
    pub selected_day_content_color: Color,
    pub disabled_selected_day_content_color: Color,
    pub selected_day_container_color: Color,
    pub disabled_selected_day_container_color: Color,
    pub today_content_color: Color,
    pub today_date_border_color: Color,
    pub day_in_selection_range_container_color: Color,
    pub day_in_selection_range_content_color: Color,
    pub divider_color: Color,
    pub date_text_field_colors: TextFieldColors,
}

impl DatePickerColors {
    /// Kotlin: `DatePickerColors(containerColor: Color, titleContentColor: Color, headlineContentColor: Color, weekdayContentColor: Color, subheadContentColor: Color, navigationContentColor: Color, yearContentColor: Color, disabledYearContentColor: Color, currentYearContentColor: Color, selectedYearContentColor: Color, disabledSelectedYearContentColor: Color, selectedYearContainerColor: Color, disabledSelectedYearContainerColor: Color, dayContentColor: Color, disabledDayContentColor: Color, selectedDayContentColor: Color, disabledSelectedDayContentColor: Color, selectedDayContainerColor: Color, disabledSelectedDayContainerColor: Color, todayContentColor: Color, todayDateBorderColor: Color, dayInSelectionRangeContainerColor: Color, dayInSelectionRangeContentColor: Color, dividerColor: Color, dateTextFieldColors: TextFieldColors)`
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        container_color: Color,
        title_content_color: Color,
        headline_content_color: Color,
        weekday_content_color: Color,
        subhead_content_color: Color,
        navigation_content_color: Color,
        year_content_color: Color,
        disabled_year_content_color: Color,
        current_year_content_color: Color,
        selected_year_content_color: Color,
        disabled_selected_year_content_color: Color,
        selected_year_container_color: Color,
        disabled_selected_year_container_color: Color,
        day_content_color: Color,
        disabled_day_content_color: Color,
        selected_day_content_color: Color,
        disabled_selected_day_content_color: Color,
        selected_day_container_color: Color,
        disabled_selected_day_container_color: Color,
        today_content_color: Color,
        today_date_border_color: Color,
        day_in_selection_range_container_color: Color,
        day_in_selection_range_content_color: Color,
        divider_color: Color,
        date_text_field_colors: TextFieldColors,
    ) -> DatePickerColors {
        DatePickerColors {
            container_color,
            title_content_color,
            headline_content_color,
            weekday_content_color,
            subhead_content_color,
            navigation_content_color,
            year_content_color,
            disabled_year_content_color,
            current_year_content_color,
            selected_year_content_color,
            disabled_selected_year_content_color,
            selected_year_container_color,
            disabled_selected_year_container_color,
            day_content_color,
            disabled_day_content_color,
            selected_day_content_color,
            disabled_selected_day_content_color,
            selected_day_container_color,
            disabled_selected_day_container_color,
            today_content_color,
            today_date_border_color,
            day_in_selection_range_container_color,
            day_in_selection_range_content_color,
            divider_color,
            date_text_field_colors,
        }
    }
}

impl PartialEq for DatePickerColors {
    fn eq(&self, other: &Self) -> bool {
        self.container_color == other.container_color
            && self.title_content_color == other.title_content_color
            && self.headline_content_color == other.headline_content_color
            && self.weekday_content_color == other.weekday_content_color
            && self.subhead_content_color == other.subhead_content_color
            && self.year_content_color == other.year_content_color
            && self.disabled_year_content_color == other.disabled_year_content_color
            && self.current_year_content_color == other.current_year_content_color
            && self.selected_year_content_color == other.selected_year_content_color
            && self.disabled_selected_year_content_color == other.disabled_selected_year_content_color
            && self.selected_year_container_color == other.selected_year_container_color
            && self.disabled_selected_year_container_color == other.disabled_selected_year_container_color
            && self.day_content_color == other.day_content_color
            && self.disabled_day_content_color == other.disabled_day_content_color
            && self.selected_day_content_color == other.selected_day_content_color
            && self.disabled_selected_day_content_color == other.disabled_selected_day_content_color
            && self.selected_day_container_color == other.selected_day_container_color
            && self.disabled_selected_day_container_color == other.disabled_selected_day_container_color
            && self.today_content_color == other.today_content_color
            && self.today_date_border_color == other.today_date_border_color
            && self.day_in_selection_range_container_color == other.day_in_selection_range_container_color
            && self.day_in_selection_range_content_color == other.day_in_selection_range_content_color
    }
}

#[named]
#[allow(clippy::too_many_arguments)]
impl DatePickerColors {
    /// The same with each color given one other than unspecified (Compose's `copy`; `date_text_field_colors`
    /// None keeps its own).
    pub fn copy(
        &self,
        #[default(self.container_color)] container_color: Color,
        #[default(self.title_content_color)] title_content_color: Color,
        #[default(self.headline_content_color)] headline_content_color: Color,
        #[default(self.weekday_content_color)] weekday_content_color: Color,
        #[default(self.subhead_content_color)] subhead_content_color: Color,
        #[default(self.navigation_content_color)] navigation_content_color: Color,
        #[default(self.year_content_color)] year_content_color: Color,
        #[default(self.disabled_year_content_color)] disabled_year_content_color: Color,
        #[default(self.current_year_content_color)] current_year_content_color: Color,
        #[default(self.selected_year_content_color)] selected_year_content_color: Color,
        #[default(self.disabled_selected_year_content_color)] disabled_selected_year_content_color: Color,
        #[default(self.selected_year_container_color)] selected_year_container_color: Color,
        #[default(self.disabled_selected_year_container_color)] disabled_selected_year_container_color: Color,
        #[default(self.day_content_color)] day_content_color: Color,
        #[default(self.disabled_day_content_color)] disabled_day_content_color: Color,
        #[default(self.selected_day_content_color)] selected_day_content_color: Color,
        #[default(self.disabled_selected_day_content_color)] disabled_selected_day_content_color: Color,
        #[default(self.selected_day_container_color)] selected_day_container_color: Color,
        #[default(self.disabled_selected_day_container_color)] disabled_selected_day_container_color: Color,
        #[default(self.today_content_color)] today_content_color: Color,
        #[default(self.today_date_border_color)] today_date_border_color: Color,
        #[default(self.day_in_selection_range_container_color)] day_in_selection_range_container_color: Color,
        #[default(self.day_in_selection_range_content_color)] day_in_selection_range_content_color: Color,
        #[default(self.divider_color)] divider_color: Color,
        #[default(Some(self.date_text_field_colors))] date_text_field_colors: Option<TextFieldColors>,
    ) -> DatePickerColors {
        DatePickerColors {
            container_color: container_color.take_or_else(|| self.container_color),
            title_content_color: title_content_color.take_or_else(|| self.title_content_color),
            headline_content_color: headline_content_color.take_or_else(|| self.headline_content_color),
            weekday_content_color: weekday_content_color.take_or_else(|| self.weekday_content_color),
            subhead_content_color: subhead_content_color.take_or_else(|| self.subhead_content_color),
            navigation_content_color: navigation_content_color.take_or_else(|| self.navigation_content_color),
            year_content_color: year_content_color.take_or_else(|| self.year_content_color),
            disabled_year_content_color: disabled_year_content_color.take_or_else(|| self.disabled_year_content_color),
            current_year_content_color: current_year_content_color.take_or_else(|| self.current_year_content_color),
            selected_year_content_color: selected_year_content_color.take_or_else(|| self.selected_year_content_color),
            disabled_selected_year_content_color: disabled_selected_year_content_color
                .take_or_else(|| self.disabled_selected_year_content_color),
            selected_year_container_color: selected_year_container_color
                .take_or_else(|| self.selected_year_container_color),
            disabled_selected_year_container_color: disabled_selected_year_container_color
                .take_or_else(|| self.disabled_selected_year_container_color),
            day_content_color: day_content_color.take_or_else(|| self.day_content_color),
            disabled_day_content_color: disabled_day_content_color.take_or_else(|| self.disabled_day_content_color),
            selected_day_content_color: selected_day_content_color.take_or_else(|| self.selected_day_content_color),
            disabled_selected_day_content_color: disabled_selected_day_content_color
                .take_or_else(|| self.disabled_selected_day_content_color),
            selected_day_container_color: selected_day_container_color
                .take_or_else(|| self.selected_day_container_color),
            disabled_selected_day_container_color: disabled_selected_day_container_color
                .take_or_else(|| self.disabled_selected_day_container_color),
            today_content_color: today_content_color.take_or_else(|| self.today_content_color),
            today_date_border_color: today_date_border_color.take_or_else(|| self.today_date_border_color),
            day_in_selection_range_container_color: day_in_selection_range_container_color
                .take_or_else(|| self.day_in_selection_range_container_color),
            day_in_selection_range_content_color: day_in_selection_range_content_color
                .take_or_else(|| self.day_in_selection_range_content_color),
            divider_color: divider_color.take_or_else(|| self.divider_color),
            date_text_field_colors: date_text_field_colors.unwrap_or(self.date_text_field_colors),
        }
    }
}

impl DatePickerColors {
    /// A day's text color (Compose's internal `dayContentColor`): animated unless the day is in a range.
    #[track_caller]
    pub(crate) fn day_content_color_for(
        &self,
        is_today: bool,
        selected: bool,
        in_range: bool,
        enabled: bool,
    ) -> State<Color> {
        let target = if selected && enabled {
            self.selected_day_content_color
        } else if selected && !enabled {
            self.disabled_selected_day_content_color
        } else if in_range && enabled {
            self.day_in_selection_range_content_color
        } else if in_range && !enabled {
            self.disabled_day_content_color
        } else if is_today && enabled {
            self.today_content_color
        } else if enabled {
            self.day_content_color
        } else {
            self.disabled_day_content_color
        };
        if in_range {
            remember_updated_state!(target)
        } else {
            // Animate the content color only when the day is not in a range.
            // TODO Load the motionScheme tokens from the component tokens file
            named_args!(animate_color_as_state(
                target,
                animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<Color>()
            ))
        }
    }

    /// A day's container color (Compose's internal `dayContainerColor`): animated if `animate`.
    #[track_caller]
    pub(crate) fn day_container_color_for(&self, selected: bool, enabled: bool, animate: bool) -> State<Color> {
        let target = if selected {
            if enabled { self.selected_day_container_color } else { self.disabled_selected_day_container_color }
        } else {
            Color::Transparent
        };
        if animate {
            // TODO Load the motionScheme tokens from the component tokens file
            named_args!(animate_color_as_state(
                target,
                animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<Color>()
            ))
        } else {
            remember_updated_state!(target)
        }
    }

    /// A year's text color (Compose's internal `yearContentColor`), animated.
    #[track_caller]
    pub(crate) fn year_content_color_for(&self, current_year: bool, selected: bool, enabled: bool) -> State<Color> {
        let target = if selected && enabled {
            self.selected_year_content_color
        } else if selected && !enabled {
            self.disabled_selected_year_content_color
        } else if current_year && enabled {
            self.current_year_content_color
        } else if enabled {
            self.year_content_color
        } else {
            self.disabled_year_content_color
        };
        // TODO Load the motionScheme tokens from the component tokens file
        named_args!(animate_color_as_state(
            target,
            animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<Color>()
        ))
    }

    /// A year's container color (Compose's internal `yearContainerColor`), animated.
    #[track_caller]
    pub(crate) fn year_container_color_for(&self, selected: bool, enabled: bool) -> State<Color> {
        let target = if selected {
            if enabled { self.selected_year_container_color } else { self.disabled_selected_year_container_color }
        } else {
            Color::Transparent
        };
        // TODO Load the motionScheme tokens from the component tokens file
        named_args!(animate_color_as_state(
            target,
            animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<Color>()
        ))
    }
}

// ---- The pieces --------------------------------------------------------------------------------------------------

/// The date pickers' frame (Compose's internal `DateEntryContainer`): at least 360 dp wide in the container color,
/// the header — the title, the headline beside the mode button, a divider under them if any — over `content`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn DateEntryContainer(
    modifier: Modifier,
    title: Option<Rc<dyn Fn()>>,
    headline: Option<Rc<dyn Fn()>>,
    mode_toggle_button: Option<Rc<dyn Fn()>>,
    colors: DatePickerColors,
    headline_text_style: TextStyle,
    header_min_height: Dp,
    content: impl Fn() + 'static,
) {
    let has_title = title.is_some();
    Column(
        modifier = named_args!(modifier.size_in(min_width = DatePickerModalTokens::ContainerWidth))
            // TODO(b/347038246): replace `isContainer` with `isTraversalGroup` with new pruning API.
            .semantics(|c| c.set_is_container(true))
            .background(colors.container_color),
        content = move || {
            let (headline, mode_toggle_button) = (headline.clone(), mode_toggle_button.clone());
            let headline_text_style = headline_text_style.clone();
            DatePickerHeader(
                Modifier,
                title.clone(),
                colors.title_content_color,
                colors.headline_content_color,
                header_min_height,
                move || {
                    let (headline, mode_toggle_button) = (headline.clone(), mode_toggle_button.clone());
                    let headline_text_style = headline_text_style.clone();
                    Column(
                        modifier = Modifier.fill_max_width(),
                        content = move || {
                            let horizontal_arrangement = match (&headline, &mode_toggle_button) {
                                (Some(_), Some(_)) => Arrangement::SpaceBetween,
                                (Some(_), None) => Arrangement::Start,
                                _ => Arrangement::End,
                            };
                            let (row_headline, row_toggle) = (headline.clone(), mode_toggle_button.clone());
                            let headline_text_style = headline_text_style.clone();
                            Row(
                                modifier = Modifier.fill_max_width(),
                                horizontal_arrangement = horizontal_arrangement,
                                vertical_alignment = Alignment::CenterVertically,
                                content = move || {
                                    if let Some(headline) = row_headline.clone() {
                                        let weight = Modifier.weight_with(1.0, true);
                                        ProvideTextStyle(headline_text_style.clone(), move || {
                                            let headline = headline.clone();
                                            Box(modifier = weight.clone(), content = move || headline());
                                        });
                                    }
                                    if let Some(toggle) = &row_toggle {
                                        toggle();
                                    }
                                },
                            );
                            // Display a divider only when there is a title, headline, or a mode toggle.
                            if has_title || headline.is_some() || mode_toggle_button.is_some() {
                                HorizontalDivider(color = colors.divider_color);
                            }
                        },
                    );
                },
            );
            content();
        },
    );
}

/// The button switching between the calendar and the text field (Compose's internal `DisplayModeToggleButton`): a
/// pencil to type, a calendar to pick, with their tooltips.
#[kompozable]
pub(crate) fn DisplayModeToggleButton(
    modifier: Modifier,
    display_mode: DisplayMode,
    on_display_mode_change: Rc<dyn Fn(DisplayMode)>,
    colors: DatePickerColors,
) {
    KompozitionLocalProvider(&[LocalContentColor.provides(colors.headline_content_color)], move || {
        let on_change = on_display_mode_change.clone();
        if display_mode == DisplayMode::Picker {
            IconButtonWithTooltip(
                Rc::new(move || on_change(DisplayMode::Input)),
                Icons::Filled::Edit(),
                get_string(Strings::DatePickerSwitchToInputMode),
                modifier.clone(),
                true,
            );
        } else {
            IconButtonWithTooltip(
                Rc::new(move || on_change(DisplayMode::Picker)),
                Icons::Filled::DateRange(),
                get_string(Strings::DatePickerSwitchToCalendarMode),
                modifier.clone(),
                true,
            );
        }
    });
}

/// The calendar or the text field, animated as the mode switches (Compose's private
/// `SwitchableDateEntryContent`).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SwitchableDateEntryContent(
    selected_date_millis: Option<i64>,
    displayed_month_millis: i64,
    display_mode: DisplayMode,
    on_date_selection_change: Rc<dyn Fn(Option<i64>)>,
    on_displayed_month_change: Rc<dyn Fn(i64)>,
    calendar_model: Rc<dyn CalendarModel>,
    year_range: RangeInclusive<i32>,
    date_formatter: Rc<dyn DatePickerFormatter>,
    selectable_dates: Rc<dyn SelectableDates>,
    colors: DatePickerColors,
    focus_requester: Option<FocusRequester>,
) {
    // Parallax effect offset that will slightly scroll in and out the navigation part of the picker when the display
    // mode changes.
    let parallax_target = -LocalDensity.current().round_to_px(Dp(48.0));
    // TODO Load the motionScheme tokens from the component tokens file
    let effects_in_animation_spec: Arc<dyn FiniteAnimationSpec<f32>> =
        MotionSchemeKeyTokens::DefaultEffects.value::<f32>();
    let effects_out_animation_spec: Arc<dyn FiniteAnimationSpec<f32>> =
        MotionSchemeKeyTokens::FastEffects.value::<f32>();
    let spatial_in_out_animation_spec: Arc<dyn FiniteAnimationSpec<IntOffset>> =
        MotionSchemeKeyTokens::DefaultSpatial.value::<IntOffset>();
    let spatial_size_animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>> =
        MotionSchemeKeyTokens::DefaultSpatial.value::<IntSize>();
    AnimatedContent(
        target_state = display_mode,
        // TODO(b/347038246): replace `isContainer` with `isTraversalGroup` with new pruning API.
        modifier = Modifier.semantics(|c| c.set_is_container(true)),
        transition_spec = move |scope: &AnimatedContentTransitionScope<DisplayMode>| {
            let (spatial, effects_in, effects_out) = (
                spatial_in_out_animation_spec.clone(),
                effects_in_animation_spec.clone(),
                effects_out_animation_spec.clone(),
            );
            // When animating the input mode, fade out the calendar picker and slide in the text field from the
            // bottom with a delay to show up after the picker is hidden.
            let transform = if scope.target_state() == DisplayMode::Input {
                (slide_in_vertically(spatial.clone(), |height: i32| height) + fade_in(effects_in, 0.0)).together_with(
                    fade_out(effects_out, 0.0) + slide_out_vertically(spatial, move |_: i32| parallax_target),
                )
            } else {
                // When animating the picker mode, slide out text field and fade in calendar picker with a delay to
                // show up after the text field is hidden.
                (slide_in_vertically(spatial.clone(), move |_: i32| parallax_target) + fade_in(effects_in, 0.0))
                    .together_with(
                        slide_out_vertically(spatial, |full_height: i32| full_height) + fade_out(effects_out, 0.0),
                    )
            };
            let size_spec = spatial_size_animation_spec.clone();
            scope.using(transform, Some(SizeTransform(true, move |_, _| size_spec.clone())))
        },
        label = "DatePickerDisplayModeAnimation".to_string(),
        content = move |_: &AnimatedContentScope, mode: DisplayMode| match mode {
            DisplayMode::Picker => {
                let on_select = on_date_selection_change.clone();
                DatePickerContent(
                    selected_date_millis,
                    displayed_month_millis,
                    Rc::new(move |date_in_millis| on_select(Some(date_in_millis))),
                    on_displayed_month_change.clone(),
                    calendar_model.clone(),
                    year_range.clone(),
                    date_formatter.clone(),
                    selectable_dates.clone(),
                    colors,
                )
            }
            DisplayMode::Input => DateInputContent(
                selected_date_millis,
                on_date_selection_change.clone(),
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

/// The calendar (Compose's private `DatePickerContent`): the months' navigation over the weekdays and the months
/// list, the year grid growing over them when the month's button is pressed.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn DatePickerContent(
    selected_date_millis: Option<i64>,
    displayed_month_millis: i64,
    on_date_selection_change: Rc<dyn Fn(i64)>,
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
            // The DatePicker has other actions that can trigger a scroll and update the displayedMonthMillis as they
            // do so, hence we check here for isScrollInProgress and only scroll to the monthIndex when there is none
            // in progress.
            if !state.is_scroll_in_progress() && state.first_visible_item_index() != month_index {
                state.scroll_to_item(month_index, 0).await;
            }
        });
    }

    let coroutine_scope = remember_coroutine_scope();
    let year_picker_visible = remember_saveable(|| mutable_state_of(false));
    let focus_manager = LocalFocusManager.current();
    let (
        next_button_focus_requester,
        year_selection_button_focus_requester,
        current_year_focus_requester,
        divider_focus_requester,
    ) = remember(|| (FocusRequester::new(), FocusRequester::new(), FocusRequester::new(), FocusRequester::new()));
    Column(
        content = move || {
            let year_picker_text = date_formatter
                .format_month_year(Some(displayed_month_millis), calendar_model.locale())
                .unwrap_or_else(|| "-".to_string());
            let (next_state, previous_state, next_scope, previous_scope) = (
                months_list_state.clone(),
                months_list_state.clone(),
                coroutine_scope.clone(),
                coroutine_scope.clone(),
            );
            let (visible_toggle, current_year_fr, focus_manager_tab) =
                (year_picker_visible.clone(), current_year_focus_requester.clone(), focus_manager.clone());
            MonthsNavigation(
                named_args!(Modifier.padding_hv(horizontal = DatePickerHorizontalPadding)),
                months_list_state.can_scroll_forward(),
                months_list_state.can_scroll_backward(),
                year_picker_visible.get(),
                year_picker_text,
                Modifier.focus_requester(&next_button_focus_requester),
                Rc::new(move || {
                    let state = next_state.clone();
                    next_scope.launch(async move {
                        // Kotlin ignores an IllegalArgumentException here (a click while the list still animates to the
                        // next item): KompozR's animateScrollToItem has none to throw.
                        state.animate_scroll_to_item(state.first_visible_item_index() + 1, 0).await;
                    });
                }),
                Rc::new(move || {
                    let state = previous_state.clone();
                    previous_scope.launch(async move {
                        state.animate_scroll_to_item(state.first_visible_item_index() - 1, 0).await;
                    });
                }),
                Rc::new(move || visible_toggle.set(!visible_toggle.peek())),
                Rc::new(move || {
                    // Keyboard focus on the selected year when tabbed from the open year picker button.
                    let moved = current_year_fr.request_focus();
                    if !moved {
                        // If grid is scrolled and selected year is not in view just move focus to closest year option
                        // from button.
                        focus_manager_tab.move_focus(FocusDirection::Down);
                    }
                }),
                year_selection_button_focus_requester.clone(),
                colors,
            );

            let (months_list_state, coroutine_scope) = (months_list_state.clone(), coroutine_scope.clone());
            let (calendar_model, year_range, date_formatter, selectable_dates) =
                (calendar_model.clone(), year_range.clone(), date_formatter.clone(), selectable_dates.clone());
            let (on_date_selection_change, on_displayed_month_change) =
                (on_date_selection_change.clone(), on_displayed_month_change.clone());
            let (year_picker_visible, focus_manager) = (year_picker_visible.clone(), focus_manager.clone());
            let (next_button_focus_requester, year_selection_button_focus_requester) =
                (next_button_focus_requester.clone(), year_selection_button_focus_requester.clone());
            let (current_year_focus_requester, divider_focus_requester) =
                (current_year_focus_requester.clone(), divider_focus_requester.clone());
            Box(content = move || {
                {
                    let (state, model, years, formatter, dates) = (
                        months_list_state.clone(),
                        calendar_model.clone(),
                        year_range.clone(),
                        date_formatter.clone(),
                        selectable_dates.clone(),
                    );
                    let (on_select, on_month, focus_manager, next) = (
                        on_date_selection_change.clone(),
                        on_displayed_month_change.clone(),
                        focus_manager.clone(),
                        next_button_focus_requester.clone(),
                    );
                    Column(
                        modifier = named_args!(Modifier.padding_hv(horizontal = DatePickerHorizontalPadding)),
                        content = move || {
                            WeekDays(colors, model.clone());
                            let next = next.clone();
                            HorizontalMonthsList(
                                state.clone(),
                                selected_date_millis,
                                on_select.clone(),
                                on_month.clone(),
                                model.clone(),
                                years.clone(),
                                formatter.clone(),
                                dates.clone(),
                                colors,
                                Rc::new(move || {
                                    next.request_focus();
                                }),
                                focus_manager.clone(),
                            );
                        },
                    );
                }
                // TODO Load the motionScheme tokens from the component tokens file
                let fade_in_animation_spec: Arc<dyn FiniteAnimationSpec<f32>> =
                    MotionSchemeKeyTokens::DefaultEffects.value::<f32>();
                let fade_out_animation_spec: Arc<dyn FiniteAnimationSpec<f32>> =
                    MotionSchemeKeyTokens::FastEffects.value::<f32>();
                let shrink_expand_animation_spec: Arc<dyn FiniteAnimationSpec<IntSize>> =
                    MotionSchemeKeyTokens::DefaultEffects.value::<IntSize>();
                let (state, model, years, dates, visible) = (
                    months_list_state.clone(),
                    calendar_model.clone(),
                    year_range.clone(),
                    selectable_dates.clone(),
                    year_picker_visible.clone(),
                );
                let (scope, focus_manager, year_button, current_year, divider) = (
                    coroutine_scope.clone(),
                    focus_manager.clone(),
                    year_selection_button_focus_requester.clone(),
                    current_year_focus_requester.clone(),
                    divider_focus_requester.clone(),
                );
                AnimatedVisibility(
                    visible = year_picker_visible.get(),
                    modifier = Modifier.clip_to_bounds(),
                    enter = named_args!(expand_vertically(animation_spec = shrink_expand_animation_spec.clone()))
                        + fade_in(fade_in_animation_spec, 0.6),
                    exit = named_args!(shrink_vertically(animation_spec = shrink_expand_animation_spec))
                        + fade_out(fade_out_animation_spec, 0.0),
                    content = move |_: &AnimatedVisibilityScope| {
                        // Apply a paneTitle to make the screen reader focus on a relevant node after this column is
                        // hidden and disposed.
                        // TODO(b/186443263): Have the screen reader focus on a year in the list when the list is
                        //  revealed.
                        let years_pane_title = get_string(Strings::DatePickerYearPickerPaneTitle);
                        let (state, model, years, dates, visible) =
                            (state.clone(), model.clone(), years.clone(), dates.clone(), visible.clone());
                        let (scope, focus_manager, year_button, current_year, divider) = (
                            scope.clone(),
                            focus_manager.clone(),
                            year_button.clone(),
                            current_year.clone(),
                            divider.clone(),
                        );
                        Column(
                            modifier = Modifier.semantics(move |c| c.set_pane_title(years_pane_title.clone())),
                            content = move || {
                                let (state, years_for_select, visible) =
                                    (state.clone(), years.clone(), visible.clone());
                                let scope = scope.clone();
                                let (fm_tab, divider_tab, year_button) =
                                    (focus_manager.clone(), divider.clone(), year_button.clone());
                                YearPicker(
                                    // Keep the height the same as the monthly calendar + weekdays height, and take into
                                    // account the thickness of the divider that will be composed below it.
                                    Modifier
                                        .required_height(
                                            RecommendedSizeForAccessibility * (MaxCalendarRows + 1) as f32
                                                - DividerDefaults::Thickness,
                                        )
                                        .padding_hv_with(DatePickerHorizontalPadding, Dp(0.0)),
                                    displayed_month_millis,
                                    Rc::new(move |year| {
                                        // Switch back to the monthly calendar and scroll to the selected year.
                                        visible.set(!visible.peek());
                                        let state = state.clone();
                                        let first = *years_for_select.start();
                                        scope.launch(async move {
                                            // Scroll to the selected year (maintaining the month of year). A
                                            // LaunchEffect at the MonthsList will take care of rest and will update the
                                            // state's displayedMonth to the month we scrolled to.
                                            state
                                                .scroll_to_item((year - first) * 12 + displayed_month.month - 1, 0)
                                                .await;
                                        });
                                    }),
                                    dates.clone(),
                                    model.clone(),
                                    years.clone(),
                                    colors,
                                    current_year.clone(),
                                    Rc::new(move || {
                                        // Shift + Tab should exit year selection grid and move focus backwards.
                                        year_button.request_focus();
                                    }),
                                    Rc::new(move || {
                                        // Tab should exit year selection grid and move focus forward.
                                        divider_tab.request_focus();
                                        fm_tab.move_focus(FocusDirection::Next);
                                    }),
                                );
                                // Make the divider a focus target so that we can properly move keyboard focus to
                                // dismiss/confirm buttons, which we don't have access to from DatePicker. However, the
                                // divider won't ever actually have the focus stay on it, so it'll be as if it's not
                                // focusable when interacting with the picker.
                                let fm = focus_manager.clone();
                                HorizontalDivider(
                                    color = colors.divider_color,
                                    modifier = Modifier
                                        .focus_requester(&divider)
                                        .on_key_event(move |it: &KeyEvent| {
                                            if it.key == Key::DirectionUp
                                                || (it.is_shift_pressed() && it.key == Key::Tab)
                                            {
                                                // If focus is coming from below, move back up.
                                                fm.move_focus(FocusDirection::Previous);
                                                true
                                            } else if it.key == Key::DirectionDown || it.key == Key::Tab {
                                                // If focus is coming from above, move forward down.
                                                fm.move_focus(FocusDirection::Next);
                                                true
                                            } else {
                                                false
                                            }
                                        })
                                        .focus_target(),
                                );
                            },
                        );
                    },
                );
            });
        },
    );
}

/// The header's column (Compose's internal `DatePickerHeader`): the title in its style and color (the header at
/// least `min_height` tall when there is one) over the content in the headline's color.
#[kompozable]
pub(crate) fn DatePickerHeader(
    modifier: Modifier,
    title: Option<Rc<dyn Fn()>>,
    title_content_color: Color,
    headline_content_color: Color,
    min_height: Dp,
    content: impl Fn() + 'static,
) {
    // Apply a defaultMinSize only when the title is not null.
    let height_modifier =
        if title.is_some() { named_args!(Modifier.default_min_size(min_height = min_height)) } else { Modifier };
    let content = Rc::new(content);
    Column(
        modifier = modifier.fill_max_width().then(height_modifier),
        vertical_arrangement = Arrangement::SpaceBetween,
        content = move || {
            if let Some(title) = title.clone() {
                let text_style = DatePickerModalTokens::HeaderSupportingTextFont.value();
                ProvideContentColorTextStyle(title_content_color, text_style, move || {
                    let title = title.clone();
                    Box(content_alignment = Alignment::BottomStart, content = move || title());
                });
            }
            let content = content.clone();
            KompozitionLocalProvider(&[LocalContentColor.provides(headline_content_color)], move || content());
        },
    );
}

/// The months, a page each, snapping (Compose's private `HorizontalMonthsList`), telling the month shown as it
/// scrolls.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn HorizontalMonthsList(
    lazy_list_state: LazyListState,
    selected_date_millis: Option<i64>,
    on_date_selection_change: Rc<dyn Fn(i64)>,
    on_displayed_month_change: Rc<dyn Fn(i64)>,
    calendar_model: Rc<dyn CalendarModel>,
    year_range: RangeInclusive<i32>,
    date_formatter: Rc<dyn DatePickerFormatter>,
    selectable_dates: Rc<dyn SelectableDates>,
    colors: DatePickerColors,
    on_return_focus: Rc<dyn Fn()>,
    focus_manager: FocusManager,
) {
    let today = calendar_model.today();
    let first_month = {
        let model = calendar_model.clone();
        let first = *year_range.start();
        // January.
        remember_keyed(year_range.clone(), move || model.get_month_of_year(first, 1))
    };
    let fling_behavior = DatePickerDefaults.remember_snap_fling_behavior(lazy_list_state.clone());
    {
        let (state, model, years) = (lazy_list_state.clone(), calendar_model.clone(), year_range.clone());
        ProvideTextStyle(DatePickerModalTokens::DateLabelTextFont.value(), move || {
            let (list_state, model, years) = (state.clone(), model.clone(), years.clone());
            let (on_select, formatter, dates) =
                (on_date_selection_change.clone(), date_formatter.clone(), selectable_dates.clone());
            let (on_return_focus, focus_manager) = (on_return_focus.clone(), focus_manager.clone());
            LazyRow(
                // Apply this to prevent the screen reader from scrolling to the next or previous month, and instead,
                // traverse outside the Month composable when swiping from a focused first or last day of the month.
                modifier = Modifier
                    .semantics(|c| c.set_horizontal_scroll_axis_range(ScrollAxisRange::new(|| 0.0, || 0.0, false))),
                state = Some(state.clone()),
                fling_behavior = Some(fling_behavior.clone()),
                content = move |scope: &mut LazyListScope| {
                    let (list_state, model) = (list_state.clone(), model.clone());
                    let (on_select, formatter, dates) = (on_select.clone(), formatter.clone(), dates.clone());
                    let (on_return_focus, focus_manager) = (on_return_focus.clone(), focus_manager.clone());
                    scope.items(number_of_months_in_range(&years), None, None, move |it| {
                        let month = model.plus_months(&first_month, it);
                        let (list_state, locale) = (list_state.clone(), model.locale().clone());
                        let (on_select, formatter, dates) = (on_select.clone(), formatter.clone(), dates.clone());
                        let (on_return_focus, focus_manager) = (on_return_focus.clone(), focus_manager.clone());
                        Box(
                            modifier = Modifier.fill_parent_max_width(1.0),
                            content = move || {
                                Month(
                                    month,
                                    on_select.clone(),
                                    today.utc_time_millis,
                                    selected_date_millis,
                                    None,
                                    None,
                                    formatter.clone(),
                                    dates.clone(),
                                    colors,
                                    locale.clone(),
                                    list_state.clone(),
                                    Some(focus_manager.clone()),
                                    on_return_focus.clone(),
                                )
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

/// Tells the month the list's first visible item is as it changes (Compose's internal suspending
/// `updateDisplayedMonth`).
pub(crate) async fn update_displayed_month(
    lazy_list_state: LazyListState,
    on_displayed_month_change: Rc<dyn Fn(i64)>,
    calendar_model: Rc<dyn CalendarModel>,
    year_range: RangeInclusive<i32>,
) {
    let state = lazy_list_state.clone();
    snapshot_flow(move || state.first_visible_item_index())
        .collect(move |_| {
            let year_offset = lazy_list_state.first_visible_item_index() / 12;
            let month = lazy_list_state.first_visible_item_index() % 12 + 1;
            on_displayed_month_change(
                calendar_model.get_month_of_year(year_range.start() + year_offset, month).start_utc_time_millis,
            );
        })
        .await;
}

/// The weekdays' row (Compose's internal `WeekDays`): their narrow names from the locale's first day, each read
/// out by its full name, as wide as a day.
#[kompozable]
pub(crate) fn WeekDays(colors: DatePickerColors, calendar_model: Rc<dyn CalendarModel>) {
    let first_day_of_week = calendar_model.first_day_of_week();
    let weekdays = calendar_model.weekday_names();
    // Start with firstDayOfWeek - 1 as the days are 1-based.
    let split = ((first_day_of_week - 1).max(0) as usize).min(weekdays.len());
    let day_names: Vec<(String, String)> = weekdays[split..].iter().chain(&weekdays[..split]).cloned().collect();
    let text_style = DatePickerModalTokens::WeekdaysLabelTextFont.value();
    let min_size = LocalMinimumInteractiveComponentSize.current();
    Row(
        modifier =
            named_args!(Modifier.default_min_size(min_height = RecommendedSizeForAccessibility)).fill_max_width(),
        horizontal_arrangement = Arrangement::SpaceEvenly,
        vertical_alignment = Alignment::CenterVertically,
        content = move || {
            for (full, short) in &day_names {
                let (full, short, text_style) = (full.clone(), short.clone(), text_style.clone());
                Box(
                    modifier = named_args!(
                        Modifier
                            .clear_and_set_semantics(move |c| c.set_content_description(full.clone()))
                            // Match the minimum size to the Day's required size. This will ensure an aligned layout
                            // for the weekday letters to the days below, even when the
                            // LocalMinimumInteractiveComponentSize is set to a lower value than the recommended
                            // 48.dp.
                            .size_in(
                                min_width = DatePickerModalTokens::DateContainerWidth,
                                min_height = DatePickerModalTokens::DateContainerHeight,
                            )
                    )
                    .size_wh(min_size, min_size),
                    content_alignment = Alignment::Center,
                    content = move || {
                        Text(
                            short.clone(),
                            modifier = Modifier.wrap_content_size(),
                            color = colors.weekday_content_color,
                            style = text_style.clone(),
                            text_align = Some(TextAlign::Center),
                        )
                    },
                );
            }
        },
    );
}

/// A month's grid of days (Compose's internal `Month`): six weeks of seven cells from the locale's first day, the
/// days in their places, `start_date_millis` and `end_date_millis` picked, today outlined, the range's band drawn
/// behind (`range_selection_info`), the days not selectable disabled.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn Month(
    month: CalendarMonth,
    on_date_selection_change: Rc<dyn Fn(i64)>,
    today_millis: i64,
    start_date_millis: Option<i64>,
    end_date_millis: Option<i64>,
    range_selection_info: Option<SelectedRangeInfo>,
    date_formatter: Rc<dyn DatePickerFormatter>,
    selectable_dates: Rc<dyn SelectableDates>,
    colors: DatePickerColors,
    locale: CalendarLocale,
    lazy_list_state: LazyListState,
    focus_manager: Option<FocusManager>,
    on_return_focus: Rc<dyn Fn()>,
) {
    let range_selection_draw_modifier = match range_selection_info.clone() {
        Some(info) => Modifier.draw_with_content(move |scope| {
            draw_range_background(scope, &info, colors.day_in_selection_range_container_color);
            scope.draw_content();
        }),
        None => Modifier,
    };
    let coroutine_scope = remember_coroutine_scope();
    let is_rtl = LocalLayoutDirection.current() == LayoutDirection::Rtl;
    let first_enabled_date_cell = get_first_enabled_day(&month, &*selectable_dates);
    let last_enabled_date_cell = get_last_enabled_day(&month, &*selectable_dates);
    let min_size = LocalMinimumInteractiveComponentSize.current();

    Column(
        modifier = Modifier
            .required_height(RecommendedSizeForAccessibility * MaxCalendarRows as f32)
            .then(range_selection_draw_modifier),
        vertical_arrangement = Arrangement::SpaceEvenly,
        content = move || {
            let mut cell_index = 0;
            for _week_index in 0..MaxCalendarRows {
                let first_cell = cell_index;
                cell_index += DaysInWeek;
                let on_select = on_date_selection_change.clone();
                let (formatter, dates, locale) = (date_formatter.clone(), selectable_dates.clone(), locale.clone());
                let (list_state, scope, focus_manager, on_return_focus) =
                    (lazy_list_state.clone(), coroutine_scope.clone(), focus_manager.clone(), on_return_focus.clone());
                let range_selection_info = range_selection_info.clone();
                Row(
                    modifier = Modifier.fill_max_width(),
                    horizontal_arrangement = Arrangement::SpaceEvenly,
                    vertical_alignment = Alignment::CenterVertically,
                    content = move || {
                        for cell_index in first_cell..first_cell + DaysInWeek {
                            if cell_index < month.days_from_start_of_week_to_first_of_month
                                || cell_index >= month.days_from_start_of_week_to_first_of_month + month.number_of_days
                            {
                                // Empty cell.
                                // Match the spacer's minimum size to the Day's required size. This will ensure an
                                // aligned layout, even when a LocalMinimumInteractiveComponentSize is set to a lower
                                // value than the recommended 48.dp.
                                Spacer(
                                    named_args!(Modifier.size_in(
                                        min_width = DatePickerModalTokens::DateContainerWidth,
                                        min_height = DatePickerModalTokens::DateContainerHeight,
                                    ))
                                    .size_wh(min_size, min_size),
                                );
                            } else {
                                let day_number = cell_index - month.days_from_start_of_week_to_first_of_month;
                                let date_in_millis =
                                    month.start_utc_time_millis + (day_number as i64 * MillisecondsIn24Hours);
                                let is_today = date_in_millis == today_millis;
                                let start_date_selected = Some(date_in_millis) == start_date_millis;
                                let end_date_selected = Some(date_in_millis) == end_date_millis;
                                let in_range = if let Some(info) = &range_selection_info {
                                    remember_keyed((info.clone(), date_in_millis), move || {
                                        mutable_state_of(
                                            date_in_millis >= start_date_millis.unwrap_or(i64::MAX)
                                                && date_in_millis <= end_date_millis.unwrap_or(i64::MIN),
                                        )
                                    })
                                    .get()
                                } else {
                                    false
                                };
                                let day_content_description = day_content_description(
                                    range_selection_info.is_some(),
                                    is_today,
                                    start_date_selected,
                                    end_date_selected,
                                    in_range,
                                );
                                let formatted_date_description =
                                    formatter.format_date(Some(date_in_millis), &locale, true).unwrap_or_default();
                                let enabled = {
                                    let dates = dates.clone();
                                    let year = month.year;
                                    remember_keyed((date_in_millis, SelectableDatesKey(dates.clone())), move || {
                                        // Disabled day in case its year is not selectable, or the date itself is
                                        // specifically not allowed by the state's SelectableDates.
                                        dates.is_selectable_year(year) && dates.is_selectable_date(date_in_millis)
                                    })
                                };
                                let on_select = on_select.clone();
                                Day(
                                    to_local_string_min(day_number + 1, 1, &locale),
                                    day_on_key_event(
                                        Modifier,
                                        is_rtl,
                                        cell_index == first_enabled_date_cell,
                                        cell_index == last_enabled_date_cell,
                                        list_state.clone(),
                                        scope.clone(),
                                        focus_manager.clone(),
                                        on_return_focus.clone(),
                                    ),
                                    start_date_selected || end_date_selected,
                                    Rc::new(move || on_select(date_in_millis)),
                                    // Only animate on the first selected day. This is important to disable when
                                    // drawing a range marker behind the days on an end-date selection.
                                    start_date_selected,
                                    enabled,
                                    is_today,
                                    in_range,
                                    match day_content_description {
                                        Some(description) => format!("{description}, {formatted_date_description}"),
                                        None => formatted_date_description,
                                    },
                                    colors,
                                );
                            }
                        }
                    },
                );
            }
        },
    );
}

/// A [`SelectableDates`] as a remember key: the same object (Kotlin's `==` of objects).
#[derive(Clone)]
pub(crate) struct SelectableDatesKey(pub(crate) Rc<dyn SelectableDates>);

impl PartialEq for SelectableDatesKey {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(Rc::as_ptr(&self.0), Rc::as_ptr(&other.0))
    }
}

/// The number of months of a year range (Compose's internal `numberOfMonthsInRange`).
pub(crate) fn number_of_months_in_range(year_range: &RangeInclusive<i32>) -> i32 {
    (year_range.end() - year_range.start() + 1) * 12
}

/// A day's keyboard handling (Compose's private `Modifier.dayOnKeyEvent`): the arrows move between days, past the
/// month's first or last enabled day to the month before or after; Tab from the last and Shift+Tab from the first
/// leave the days. None without a focus manager (the range picker).
#[allow(clippy::too_many_arguments)]
fn day_on_key_event(
    modifier: Modifier,
    is_rtl: bool,
    is_first_day: bool,
    is_last_day: bool,
    state: LazyListState,
    coroutine_scope: CoroutineScope,
    focus_manager: Option<FocusManager>,
    on_return_focus: Rc<dyn Fn()>,
) -> Modifier {
    let Some(focus_manager) = focus_manager else {
        // This happens for range date picker which doesn't need this keyboard navigation logic.
        return modifier;
    };
    if is_first_day {
        modifier.on_key_event(move |it: &KeyEvent| {
            // Shift + tab should exit days selection back to next month button.
            if is_shift_tab(it) {
                on_return_focus();
                return true;
            }
            if state.is_scroll_in_progress() {
                // Do nothing if scroll is currently happening. Like if left/right key was quickly pressed after
                // right/left key, which could cause weird focus navigation behavior.
                return true;
            }
            if is_direction_backwards(it, is_rtl) {
                // Make sure it scrolls only if going to previous month.
                go_to_month(-1, &state, &focus_manager, FocusDirection::Previous, &coroutine_scope);
                true
            } else if is_direction_forward(it, is_rtl) {
                focus_manager.move_focus(FocusDirection::Next);
                true
            } else {
                false
            }
        })
    } else if is_last_day {
        modifier.on_key_event(move |it: &KeyEvent| {
            // Tab should exit days selection and move focus down.
            if is_tab(it) {
                // Move to dismiss button.
                let moved = focus_manager.move_focus(FocusDirection::Down);
                if moved {
                    let direction = if is_rtl { FocusDirection::Left } else { FocusDirection::Right };
                    // Then move to ok button as its focus comes before the dismiss button's.
                    focus_manager.move_focus(direction);
                } else if !state.is_scroll_in_progress() {
                    // If focus didn't move, it's because there's no focus target down like to an ok or cancel
                    // button. So scroll and move focus forward instead to next month.
                    go_to_month(1, &state, &focus_manager, FocusDirection::Next, &coroutine_scope);
                }
                return true;
            }
            if state.is_scroll_in_progress() {
                // Do nothing if scroll is currently happening.
                return true;
            }
            if is_direction_forward(it, is_rtl) {
                // Make sure it scrolls only if going to next month.
                go_to_month(1, &state, &focus_manager, FocusDirection::Next, &coroutine_scope);
                true
            } else if is_direction_backwards(it, is_rtl) {
                focus_manager.move_focus(FocusDirection::Previous);
                true
            } else {
                false
            }
        })
    } else {
        modifier.on_key_event(move |it: &KeyEvent| {
            // Right and left keys should only go to next and previous dates in the month.
            if is_direction_forward(it, is_rtl) {
                focus_manager.move_focus(FocusDirection::Next);
                true
            } else if is_direction_backwards(it, is_rtl) {
                focus_manager.move_focus(FocusDirection::Previous);
                true
            } else {
                false
            }
        })
    }
}

/// Scrolls `month` months on, then moves the focus (Compose's private `goToMonth`).
fn go_to_month(
    month: i32,
    state: &LazyListState,
    focus_manager: &FocusManager,
    focus_direction: FocusDirection,
    coroutine_scope: &CoroutineScope,
) {
    let (state, focus_manager) = (state.clone(), focus_manager.clone());
    named_args!(coroutine_scope.launch(async move {
        state.animate_scroll_to_item(state.first_visible_item_index() + month, 0).await;
        focus_manager.move_focus(focus_direction);
    }));
}

/// The cell of a month's first selectable day (Compose's private `getFirstEnabledDay`).
fn get_first_enabled_day(month: &CalendarMonth, selectable_dates: &dyn SelectableDates) -> i32 {
    let mut first_cell = month.days_from_start_of_week_to_first_of_month;
    let last_cell = (month.days_from_start_of_week_to_first_of_month + month.number_of_days) - 1;
    if selectable_dates.is_selectable_year(month.year) {
        let mut day = 0i64;
        while !selectable_dates.is_selectable_date(month.start_utc_time_millis + (day * MillisecondsIn24Hours))
            && first_cell <= last_cell
        {
            day += 1;
            first_cell += 1;
        }
    }
    first_cell
}

/// The cell of a month's last selectable day (Compose's private `getLastEnabledDay`).
fn get_last_enabled_day(month: &CalendarMonth, selectable_dates: &dyn SelectableDates) -> i32 {
    let first_cell = month.days_from_start_of_week_to_first_of_month;
    let mut last_cell = (month.days_from_start_of_week_to_first_of_month + month.number_of_days) - 1;
    if selectable_dates.is_selectable_year(month.year) {
        let mut day = 0i64;
        while !selectable_dates.is_selectable_date(month.end_utc_time_millis() - (day * MillisecondsIn24Hours))
            && last_cell >= first_cell
        {
            day += 1;
            last_cell -= 1;
        }
    }
    last_cell
}

/// What a day is besides its date, read out before it (Compose's private `dayContentDescription`): "Start date",
/// "End date" or "In range" in a range picker, "Today".
#[kompozable]
fn day_content_description(
    range_selection_enabled: bool,
    is_today: bool,
    is_start_date: bool,
    is_end_date: bool,
    is_in_range: bool,
) -> Option<String> {
    let mut description = String::new();
    if range_selection_enabled {
        if is_start_date {
            description.push_str(&get_string(Strings::DateRangePickerStartHeadline));
        } else if is_end_date {
            description.push_str(&get_string(Strings::DateRangePickerEndHeadline));
        } else if is_in_range {
            description.push_str(&get_string(Strings::DateRangePickerDayInRange));
        }
    }
    if is_today {
        if !description.is_empty() {
            description.push_str(", ");
        }
        description.push_str(&get_string(Strings::DatePickerTodayDescription));
    }
    if description.is_empty() { None } else { Some(description) }
}

/// A day (Compose's private `Day`): a selectable circle 40 dp across, the day's number in it, its semantics the
/// date read out as one button; the selected day filled, today outlined.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn Day(
    text: String,
    modifier: Modifier,
    selected: bool,
    on_click: Rc<dyn Fn()>,
    animate_checked: bool,
    enabled: bool,
    today: bool,
    in_range: bool,
    description: String,
    colors: DatePickerColors,
) {
    SelectableSurface(
        selected = selected,
        on_click = move || on_click(),
        // Apply and merge semantics here. This will ensure that when scrolling the list the entire Day surface is
        // treated as one unit and holds the date semantics even when it's not completely visible atm.
        modifier = modifier.semantics_with(true, move |c| {
            c.set_text(AnnotatedString::new(description.clone()));
            c.set_role(Role::Button);
        }),
        enabled = enabled,
        shape = DatePickerModalTokens::DateContainerShape.value(),
        color = colors.day_container_color_for(selected, enabled, animate_checked).value(),
        border = if today && !selected {
            Some(BorderStroke(DatePickerModalTokens::DateTodayContainerOutlineWidth, colors.today_date_border_color))
        } else {
            None
        },
        content = move || {
            let text = text.clone();
            Box(
                modifier = Modifier.required_size_wh(
                    DatePickerModalTokens::DateContainerWidth,
                    DatePickerModalTokens::DateContainerHeight,
                ),
                content_alignment = Alignment::Center,
                content = move || {
                    Text(
                        text.clone(),
                        // The semantics are set at the Surface level.
                        modifier = Modifier.clear_and_set_semantics(|_| {}),
                        color = colors.day_content_color_for(today, selected, in_range, enabled).value(),
                        text_align = Some(TextAlign::Center),
                    )
                },
            );
        },
    );
}

/// The years' grid (Compose's private `YearPicker`): three a row, the displayed year selected, the current one
/// outlined, starting a row before the displayed year; it takes the keyboard focus on the displayed year as it
/// opens.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn YearPicker(
    modifier: Modifier,
    displayed_month_millis: i64,
    on_year_selected: Rc<dyn Fn(i32)>,
    selectable_dates: Rc<dyn SelectableDates>,
    calendar_model: Rc<dyn CalendarModel>,
    year_range: RangeInclusive<i32>,
    colors: DatePickerColors,
    current_year_focus_requester: FocusRequester,
    on_year_shift_tab_pressed: Rc<dyn Fn()>,
    on_year_tab_pressed: Rc<dyn Fn()>,
) {
    {
        let current_year_focus_requester = current_year_focus_requester.clone();
        ProvideTextStyle(DatePickerModalTokens::SelectionYearLabelTextFont.value(), move || {
            let current_year = calendar_model.get_month_of_date(&calendar_model.today()).year;
            let displayed_year = calendar_model.get_month(displayed_month_millis).year;
            let lazy_grid_state = remember_lazy_grid_state(
                // Set the initial index to a few years before the current year to allow quicker selection of
                // previous years.
                initial_first_visible_item_index = (displayed_year - year_range.start() - YearsInRow).max(0),
            );
            let (years, model, dates, on_year_selected) =
                (year_range.clone(), calendar_model.clone(), selectable_dates.clone(), on_year_selected.clone());
            let (shift_tab, tab, focus) =
                (on_year_shift_tab_pressed.clone(), on_year_tab_pressed.clone(), current_year_focus_requester.clone());
            LazyVerticalGrid(
                columns = GridCells::Fixed(YearsInRow),
                // Match the years container color to any elevated surface color that is composed under it.
                modifier = modifier.clone().background(colors.container_color),
                state = Some(lazy_grid_state),
                horizontal_arrangement = Arrangement::SpaceEvenly,
                vertical_arrangement = Some(Arrangement::spaced_by(YearsVerticalPadding)),
                content = move |scope: &mut LazyGridScope| {
                    let first = *years.start();
                    let (model, dates, on_year_selected) = (model.clone(), dates.clone(), on_year_selected.clone());
                    let (shift_tab, tab, focus) = (shift_tab.clone(), tab.clone(), focus.clone());
                    scope.items(years.clone().count() as i32, None, None, None, move |it| {
                        let selected_year = it + first;
                        let localized_year = to_local_string_min(selected_year, 1, model.locale());
                        let (shift_tab, tab) = (shift_tab.clone(), tab.clone());
                        let on_year_selected = on_year_selected.clone();
                        Year(
                            localized_year.clone(),
                            Modifier
                                .required_size_wh(
                                    DatePickerModalTokens::SelectionYearContainerWidth,
                                    DatePickerModalTokens::SelectionYearContainerHeight,
                                )
                                .on_key_event(move |it: &KeyEvent| {
                                    if is_shift_tab(it) {
                                        shift_tab();
                                        return true;
                                    }
                                    if is_tab(it) {
                                        tab();
                                        return true;
                                    }
                                    false
                                })
                                .then(if selected_year == displayed_year {
                                    Modifier.focus_requester(&focus)
                                } else {
                                    Modifier
                                }),
                            selected_year == displayed_year,
                            selected_year == current_year,
                            Rc::new(move || on_year_selected(selected_year)),
                            dates.is_selectable_year(selected_year),
                            format_date_picker_navigate_to_year_string(
                                &get_string(Strings::DatePickerNavigateToYearDescription),
                                &localized_year,
                            ),
                            colors,
                        );
                    });
                },
            );
        });
    }
    // Keyboard focus on the selected year when the year picker opens.
    let focus = current_year_focus_requester.clone();
    LaunchedEffect(current_year_focus_requester, async move {
        focus.request_focus();
    });
}

/// A year (Compose's private `Year`): a selectable pill, the year in it, read out as a button.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn Year(
    text: String,
    modifier: Modifier,
    selected: bool,
    current_year: bool,
    on_click: Rc<dyn Fn()>,
    enabled: bool,
    description: String,
    colors: DatePickerColors,
) {
    let border = remember_keyed((current_year, selected), move || {
        if current_year && !selected {
            // Use the day's spec to draw a border around the current year.
            Some(BorderStroke(DatePickerModalTokens::DateTodayContainerOutlineWidth, colors.today_date_border_color))
        } else {
            None
        }
    });
    SelectableSurface(
        selected = selected,
        on_click = move || on_click(),
        // Apply and merge semantics here. This will ensure that when scrolling the list the entire Year surface is
        // treated as one unit and holds the date semantics even when it's not completely visible atm.
        modifier = modifier.semantics_with(true, move |c| {
            c.set_text(AnnotatedString::new(description.clone()));
            c.set_role(Role::Button);
        }),
        enabled = enabled,
        shape = DatePickerModalTokens::SelectionYearStateLayerShape.value(),
        color = colors.year_container_color_for(selected, enabled).value(),
        border = border,
        content = move || {
            let text = text.clone();
            Box(
                modifier = Modifier.fill_max_width(),
                content_alignment = Alignment::Center,
                content = move || {
                    Text(
                        text.clone(),
                        // The semantics are set at the Surface level.
                        modifier = Modifier.clear_and_set_semantics(|_| {}),
                        color = colors.year_content_color_for(current_year, selected, enabled).value(),
                        text_align = Some(TextAlign::Center),
                    )
                },
            );
        },
    );
}

/// The months' navigation (Compose's private `MonthsNavigation`): the month's button opening the years, the
/// arrows to the month before and after (hidden while the years show).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn MonthsNavigation(
    modifier: Modifier,
    next_available: bool,
    previous_available: bool,
    year_picker_visible: bool,
    year_picker_text: String,
    next_button_modifier: Modifier,
    on_next_clicked: Rc<dyn Fn()>,
    on_previous_clicked: Rc<dyn Fn()>,
    on_year_picker_button_clicked: Rc<dyn Fn()>,
    on_year_picker_button_tab_pressed: Rc<dyn Fn()>,
    year_selection_button_focus_requester: FocusRequester,
    colors: DatePickerColors,
) {
    Row(
        modifier = modifier.fill_max_width().required_height(MonthYearHeight),
        horizontal_arrangement = if year_picker_visible { Arrangement::Start } else { Arrangement::SpaceBetween },
        vertical_alignment = Alignment::CenterVertically,
        content = move || {
            // A menu button for selecting a year.
            let tab = on_year_picker_button_tab_pressed.clone();
            let text = year_picker_text.clone();
            YearPickerMenuButton(
                on_year_picker_button_clicked.clone(),
                year_picker_visible,
                Modifier.focus_requester(&year_selection_button_focus_requester).on_key_event(move |it: &KeyEvent| {
                    if year_picker_visible && is_tab(it) {
                        tab();
                        return true;
                    }
                    false
                }),
                move || {
                    let description = text.clone();
                    Text(
                        text.clone(),
                        modifier = Modifier.semantics(move |c| {
                            // Make the screen reader read out updates to the menu button text as the user navigates
                            // the arrows or scrolls to change the displayed month.
                            c.set_live_region(LiveRegionMode::Polite);
                            c.set_content_description(description.clone());
                        }),
                        color = colors.navigation_content_color,
                    )
                },
            );
            // Show arrows for traversing months (only visible when the year selection is off)
            if !year_picker_visible {
                let (previous, next, next_modifier) =
                    (on_previous_clicked.clone(), on_next_clicked.clone(), next_button_modifier.clone());
                KompozitionLocalProvider(&[LocalContentColor.provides(colors.navigation_content_color)], move || {
                    let (previous, next, next_modifier) = (previous.clone(), next.clone(), next_modifier.clone());
                    Row(content = move || {
                        IconButtonWithTooltip(
                            previous.clone(),
                            Icons::AutoMirrored::Filled::KeyboardArrowLeft(),
                            get_string(Strings::DatePickerSwitchToPreviousMonth),
                            Modifier,
                            previous_available,
                        );
                        IconButtonWithTooltip(
                            next.clone(),
                            Icons::AutoMirrored::Filled::KeyboardArrowRight(),
                            get_string(Strings::DatePickerSwitchToNextMonth),
                            next_modifier.clone(),
                            next_available,
                        );
                    });
                });
            }
        },
    );
}

/// The month's button (Compose's private `YearPickerMenuButton`, a stand-in for the MenuButton Material doesn't
/// have yet): a round text button, the content and a drop-down arrow turned over while the years show.
#[kompozable]
fn YearPickerMenuButton(on_click: Rc<dyn Fn()>, expanded: bool, modifier: Modifier, content: impl Fn() + 'static) {
    TextButton(
        on_click = move || on_click(),
        modifier = modifier,
        shape = Arc::new(CircleShape) as Arc<dyn Shape>,
        colors = named_args!(ButtonDefaults.text_button_colors(content_color = LocalContentColor.current())),
        elevation = None,
        border = None,
        content = move || {
            content();
            Spacer(Modifier.size(ButtonDefaults::IconSpacing));
            VectorIcon(
                Icons::Filled::ArrowDropDown(),
                Some(if expanded {
                    get_string(Strings::DatePickerSwitchToDaySelection)
                } else {
                    get_string(Strings::DatePickerSwitchToYearSelection)
                }),
                Modifier.rotate(if expanded { 180.0 } else { 0.0 }),
            );
        },
    );
}

/// An icon button with a plain tooltip of its description above (Compose's private `IconButtonWithTooltip`).
#[kompozable]
fn IconButtonWithTooltip(
    on_click: Rc<dyn Fn()>,
    icon: ImageVector,
    content_description: String,
    modifier: Modifier,
    enabled: bool,
) {
    let tooltip_text = content_description.clone();
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
        tooltip = move |s: &dyn TooltipScope| {
            let text = tooltip_text.clone();
            s.PlainTooltip(content = move || Text(text.clone()));
        },
        state = remember_tooltip_state(),
        content = move || {
            let (on_click, icon, description) = (on_click.clone(), icon.clone(), content_description.clone());
            IconButton(
                on_click = move || on_click(),
                modifier = modifier.clone(),
                enabled = enabled,
                content = move || VectorIcon(icon.clone(), Some(description.clone())),
            );
        },
    );
}

fn is_direction_backwards(event: &KeyEvent, is_rtl: bool) -> bool {
    if is_rtl { is_direction_right(event) } else { is_direction_left(event) }
}

fn is_direction_forward(event: &KeyEvent, is_rtl: bool) -> bool {
    if is_rtl { is_direction_left(event) } else { is_direction_right(event) }
}

pub(crate) const RecommendedSizeForAccessibility: Dp = Dp(48.0);
pub(crate) const MonthYearHeight: Dp = Dp(56.0);
pub(crate) const DatePickerHorizontalPadding: Dp = Dp(12.0);

/// Compose's internal `DatePickerModeTogglePadding`: 12 dp at the end and the bottom.
pub(crate) fn DatePickerModeTogglePadding() -> PaddingValues {
    PaddingValues::new(Dp(0.0), Dp(0.0), Dp(12.0), Dp(12.0))
}

fn DatePickerTitlePadding() -> PaddingValues {
    PaddingValues::new(Dp(24.0), Dp(16.0), Dp(12.0), Dp(0.0))
}

fn DatePickerHeadlinePadding() -> PaddingValues {
    PaddingValues::new(Dp(24.0), Dp(0.0), Dp(12.0), Dp(12.0))
}

const YearsVerticalPadding: Dp = Dp(16.0);

const MaxCalendarRows: i32 = 6;
const YearsInRow: i32 = 3;

fn is_shift_tab(event: &KeyEvent) -> bool {
    event.is_shift_pressed() && event.r#type == KeyEventType::KeyDown && event.key == Key::Tab
}

fn is_tab(event: &KeyEvent) -> bool {
    !event.is_shift_pressed() && event.r#type == KeyEventType::KeyDown && event.key == Key::Tab
}

fn is_direction_left(event: &KeyEvent) -> bool {
    event.r#type == KeyEventType::KeyDown && event.key == Key::DirectionLeft
}

fn is_direction_right(event: &KeyEvent) -> bool {
    event.r#type == KeyEventType::KeyDown && event.key == Key::DirectionRight
}

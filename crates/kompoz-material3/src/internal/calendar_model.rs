//! The calendar the date pickers count in (Compose's internal/CalendarModel.kt, with Compose for desktop's actuals
//! of CalendarModel.skiko.kt: the model is [`KotlinxDatetimeCalendarModel`], skeletons are formatted by
//! [`PlatformDateFormat`]).

use std::any::Any;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

use crate::calendar_locale::CalendarLocale;
use crate::internal::kotlinx_datetime_calendar_model::KotlinxDatetimeCalendarModel;
use crate::internal::platform_date_format::PlatformDateFormat;

/// Compose's `formatterCache`s (`MutableMap<String, Any>`): results kept by key.
pub(crate) type FormatterCache = RefCell<HashMap<String, Rc<dyn Any>>>;

/// The calendar model of `locale` (Compose's `createCalendarModel`; desktop's actual makes a
/// [`KotlinxDatetimeCalendarModel`]).
pub(crate) fn create_calendar_model(locale: CalendarLocale) -> Rc<dyn CalendarModel> {
    Rc::new(KotlinxDatetimeCalendarModel::new(locale))
}

/// `utc_time_millis` formatted by a date skeleton in `locale` (Compose's internal `formatWithSkeleton`; desktop's
/// actual asks a [`PlatformDateFormat`] of the locale).
pub(crate) fn format_with_skeleton(
    utc_time_millis: i64,
    skeleton: &str,
    locale: &CalendarLocale,
    cache: &FormatterCache,
) -> String {
    PlatformDateFormat::new(locale.clone()).format_with_skeleton(utc_time_millis, skeleton, cache)
}

/// A calendar (Compose's internal abstract class `CalendarModel`): today, the locale's week, months and dates by
/// their UTC milliseconds, formatting and parsing.
pub(crate) trait CalendarModel {
    /// The locale it formats in (Compose's `locale`).
    fn locale(&self) -> &CalendarLocale;

    /// What it caches formatters in (Compose's `formatterCache`).
    fn formatter_cache(&self) -> &FormatterCache;

    /// Today's date, at its UTC midnight (Compose's `today`).
    fn today(&self) -> CalendarDate;

    /// The first day of the week in ISO 8601 numbering (Monday 1 … Sunday 7) (Compose's `firstDayOfWeek`).
    fn first_day_of_week(&self) -> i32;

    /// The weekdays' full and short names, Monday first (Compose's `weekdayNames`).
    fn weekday_names(&self) -> Vec<(String, String)>;

    /// The format dates are typed in (Compose's `getDateInputFormat`; desktop's ignores `locale` for the model's).
    fn get_date_input_format(&self, locale: &CalendarLocale) -> DateInputFormat;

    /// The date at the UTC midnight of the day `time_in_millis` is in (Compose's `getCanonicalDate`).
    fn get_canonical_date(&self, time_in_millis: i64) -> CalendarDate;

    /// The month `time_in_millis` is in (Compose's `getMonth(timeInMillis)`).
    fn get_month(&self, time_in_millis: i64) -> CalendarMonth;

    /// The month of `date` (Compose's `getMonth(date)`).
    fn get_month_of_date(&self, date: &CalendarDate) -> CalendarMonth {
        self.get_month(date.utc_time_millis)
    }

    /// The month `month` (1 to 12) of `year` (Compose's `getMonth(year, month)`).
    fn get_month_of_year(&self, year: i32, month: i32) -> CalendarMonth;

    /// `date`'s day of the week in ISO 8601 numbering (Compose's `getDayOfWeek`).
    fn get_day_of_week(&self, date: &CalendarDate) -> i32;

    /// The month `added_months_count` after `from` (Compose's `plusMonths`).
    fn plus_months(&self, from: &CalendarMonth, added_months_count: i32) -> CalendarMonth;

    /// The month `subtracted_months_count` before `from` (Compose's `minusMonths`).
    fn minus_months(&self, from: &CalendarMonth, subtracted_months_count: i32) -> CalendarMonth;

    /// `month` formatted by a skeleton (Compose's `formatWithSkeleton(month, skeleton, locale)`).
    fn format_month_with_skeleton(&self, month: &CalendarMonth, skeleton: &str, locale: &CalendarLocale) -> String {
        format_with_skeleton(month.start_utc_time_millis, skeleton, locale, self.formatter_cache())
    }

    /// `date` formatted by a skeleton (Compose's `formatWithSkeleton(date, skeleton, locale)`).
    fn format_date_with_skeleton(&self, date: &CalendarDate, skeleton: &str, locale: &CalendarLocale) -> String {
        format_with_skeleton(date.utc_time_millis, skeleton, locale, self.formatter_cache())
    }

    /// `utc_time_millis` formatted by a pattern (Compose's `formatWithPattern`).
    fn format_with_pattern(&self, utc_time_millis: i64, pattern: &str, locale: &CalendarLocale) -> String;

    /// The date `date` is by `pattern`, if it is one (Compose's `parse`).
    fn parse(&self, date: &str, pattern: &str, locale: &CalendarLocale) -> Option<CalendarDate>;
}

/// A date (Compose's internal data class `CalendarDate`), compared by its milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CalendarDate {
    pub(crate) year: i32,
    /// 1 to 12.
    pub(crate) month: i32,
    pub(crate) day_of_month: i32,
    pub(crate) utc_time_millis: i64,
}

impl PartialOrd for CalendarDate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CalendarDate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.utc_time_millis.cmp(&other.utc_time_millis)
    }
}

impl CalendarDate {
    /// Formatted by a skeleton in the model's locale (Compose's `format`).
    pub(crate) fn format(&self, calendar_model: &dyn CalendarModel, skeleton: &str) -> String {
        calendar_model.format_date_with_skeleton(self, skeleton, calendar_model.locale())
    }
}

/// A month (Compose's internal data class `CalendarMonth`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CalendarMonth {
    pub(crate) year: i32,
    /// 1 to 12.
    pub(crate) month: i32,
    pub(crate) number_of_days: i32,
    /// The days before the first of the month in its first week (by the locale's first day of the week).
    pub(crate) days_from_start_of_week_to_first_of_month: i32,
    pub(crate) start_utc_time_millis: i64,
}

impl CalendarMonth {
    /// The last millisecond of the month (Compose's `endUtcTimeMillis`).
    pub(crate) fn end_utc_time_millis(&self) -> i64 {
        self.start_utc_time_millis + (self.number_of_days as i64 * MillisecondsIn24Hours) - 1
    }

    /// The month's index in a range of years (Compose's `indexIn`).
    pub(crate) fn index_in(&self, years: &std::ops::RangeInclusive<i32>) -> i32 {
        (self.year - years.start()) * 12 + self.month - 1
    }

    /// Formatted by a skeleton in the model's locale (Compose's `format`).
    pub(crate) fn format(&self, calendar_model: &dyn CalendarModel, skeleton: &str) -> String {
        calendar_model.format_month_with_skeleton(self, skeleton, calendar_model.locale())
    }
}

/// The pattern dates are typed by, with its delimiter (Compose's internal data class `DateInputFormat`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DateInputFormat {
    pub(crate) pattern_with_delimiters: String,
    pub(crate) delimiter: char,
    /// The pattern without its delimiters (Compose's `patternWithoutDelimiters`).
    pub(crate) pattern_without_delimiters: String,
}

impl DateInputFormat {
    pub(crate) fn new(pattern_with_delimiters: String, delimiter: char) -> DateInputFormat {
        let pattern_without_delimiters = pattern_with_delimiters.replace(delimiter, "");
        DateInputFormat { pattern_with_delimiters, delimiter, pattern_without_delimiters }
    }
}

/// The input format of a locale's short date pattern (Compose's `datePatternAsInputFormat`): only the day, month,
/// year and delimiters stay, as two digits of day and month and four of the year.
pub(crate) fn date_pattern_as_input_format(locale_format: &str) -> DateInputFormat {
    // Regex("[^dMy/\\-.]") → "".
    let kept: String = locale_format.chars().filter(|c| matches!(c, 'd' | 'M' | 'y' | '/' | '-' | '.')).collect();
    // Regex("d{1,2}") → "dd", Regex("M{1,2}") → "MM", Regex("y{1,4}") → "yyyy": each run of a letter is replaced
    // a group (of at most 2 or 4) at a time.
    let replace_runs = |s: &str, letter: char, max: usize, with: &str| -> String {
        let mut out = String::new();
        let mut run = 0usize;
        let flush = |out: &mut String, run: usize| {
            for _ in 0..run.div_ceil(max) {
                out.push_str(with);
            }
        };
        for c in s.chars() {
            if c == letter {
                run += 1;
            } else {
                flush(&mut out, run);
                run = 0;
                out.push(c);
            }
        }
        flush(&mut out, run);
        out
    };
    let pattern = replace_runs(&kept, 'd', 2, "dd");
    let pattern = replace_runs(&pattern, 'M', 2, "MM");
    let pattern = replace_runs(&pattern, 'y', 4, "yyyy");
    // Edge case for the Kako locale.
    let pattern = pattern.replace("My", "M/y");
    // Removes a dot suffix that appears in some formats.
    let pattern = pattern.strip_suffix('.').map(str::to_string).unwrap_or(pattern);
    // Compose's `!!`: a short date pattern always has a delimiter.
    let delimiter = pattern.chars().find(|c| matches!(c, '/' | '-' | '.')).expect("a date pattern with no delimiter");
    DateInputFormat::new(pattern, delimiter)
}

pub(crate) const DaysInWeek: i32 = 7;
pub(crate) const MillisecondsIn24Hours: i64 = 86_400_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_patterns_become_input_formats() {
        let f = date_pattern_as_input_format("M/d/yy");
        assert_eq!((f.pattern_with_delimiters.as_str(), f.delimiter), ("MM/dd/yyyy", '/'));
        assert_eq!(f.pattern_without_delimiters, "MMddyyyy");
        assert_eq!(date_pattern_as_input_format("d.MM.y").pattern_with_delimiters, "dd.MM.yyyy");
        assert_eq!(date_pattern_as_input_format("y-MM-dd").pattern_with_delimiters, "yyyy-MM-dd");
        assert_eq!(date_pattern_as_input_format("dd.MM.yy.").pattern_with_delimiters, "dd.MM.yyyy");
        // Kako's "My" gets a slash.
        assert_eq!(date_pattern_as_input_format("d MMM y").pattern_with_delimiters, "ddMMMM/yyyy");
    }
}

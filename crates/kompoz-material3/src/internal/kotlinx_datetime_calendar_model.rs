//! Compose for desktop's calendar model (skikoMain's internal/KotlinxDatetimeCalendarModel.kt): proleptic
//! Gregorian dates by their UTC milliseconds (kotlinx-datetime's `LocalDate` arithmetic, written out here), today
//! from the system's clock in its time zone (the C library's `localtime_r`, as kotlinx-datetime's
//! `TimeZone.currentSystemDefault()` reads the system's zone), the locale's week, names and formats from a
//! [`PlatformDateFormat`].

use std::cell::Cell;
use std::collections::HashMap;

use crate::calendar_locale::CalendarLocale;
use crate::internal::calendar_model::{
    CalendarDate, CalendarModel, CalendarMonth, DateInputFormat, FormatterCache, MillisecondsIn24Hours,
};
use crate::internal::platform_date_format::PlatformDateFormat;

pub(crate) struct KotlinxDatetimeCalendarModel {
    locale: CalendarLocale,
    formatter_cache: FormatterCache,
    platform_date_format: PlatformDateFormat,
}

impl KotlinxDatetimeCalendarModel {
    pub(crate) fn new(locale: CalendarLocale) -> KotlinxDatetimeCalendarModel {
        KotlinxDatetimeCalendarModel {
            platform_date_format: PlatformDateFormat::new(locale.clone()),
            formatter_cache: FormatterCache::new(HashMap::new()),
            locale,
        }
    }

    /// The month `instant`'s UTC day is in (Compose's private `Instant.toCalendarMonth(TimeZone.UTC)`).
    fn to_calendar_month(&self, utc_time_millis: i64) -> CalendarMonth {
        let (year, month, _) = civil_from_days(utc_time_millis.div_euclid(MillisecondsIn24Hours));
        let start_days = days_from_civil(year, month, 1);
        CalendarMonth {
            year,
            month,
            number_of_days: number_of_days(year, month),
            days_from_start_of_week_to_first_of_month: self.days_from_start_of_week_to_first_of_month(start_days),
            start_utc_time_millis: start_days * MillisecondsIn24Hours,
        }
    }

    /// Compose's private `LocalDate.daysFromStartOfWeekToFirstOfMonth()`.
    fn days_from_start_of_week_to_first_of_month(&self, epoch_days: i64) -> i32 {
        let difference = iso_day_number(epoch_days) - self.first_day_of_week();
        if difference >= 0 { difference } else { 7 + difference }
    }
}

impl CalendarModel for KotlinxDatetimeCalendarModel {
    fn locale(&self) -> &CalendarLocale {
        &self.locale
    }

    fn formatter_cache(&self) -> &FormatterCache {
        &self.formatter_cache
    }

    fn today(&self) -> CalendarDate {
        let (year, month, day_of_month) = local_today();
        CalendarDate {
            year,
            month,
            day_of_month,
            utc_time_millis: days_from_civil(year, month, day_of_month) * MillisecondsIn24Hours,
        }
    }

    fn first_day_of_week(&self) -> i32 {
        self.platform_date_format.first_day_of_week()
    }

    fn weekday_names(&self) -> Vec<(String, String)> {
        self.platform_date_format.weekday_names()
    }

    fn get_date_input_format(&self, _locale: &CalendarLocale) -> DateInputFormat {
        self.platform_date_format.get_date_input_format()
    }

    fn get_canonical_date(&self, time_in_millis: i64) -> CalendarDate {
        let days = time_in_millis.div_euclid(MillisecondsIn24Hours);
        let (year, month, day_of_month) = civil_from_days(days);
        CalendarDate { year, month, day_of_month, utc_time_millis: days * MillisecondsIn24Hours }
    }

    fn get_month(&self, time_in_millis: i64) -> CalendarMonth {
        self.to_calendar_month(time_in_millis)
    }

    fn get_month_of_year(&self, year: i32, month: i32) -> CalendarMonth {
        // kotlinx-datetime's LocalDate(year, month, 1) throws for a month out of 1..12, as Kotlin's would.
        assert!((1..=12).contains(&month), "month {month} is out of 1..12");
        self.to_calendar_month(days_from_civil(year, month, 1) * MillisecondsIn24Hours)
    }

    fn get_day_of_week(&self, date: &CalendarDate) -> i32 {
        iso_day_number(days_from_civil(date.year, date.month, date.day_of_month))
    }

    fn plus_months(&self, from: &CalendarMonth, added_months_count: i32) -> CalendarMonth {
        // The first of `from` plus the months (a first of a month has no day to clamp).
        let (year, month, _) = civil_from_days(from.start_utc_time_millis.div_euclid(MillisecondsIn24Hours));
        let months = year as i64 * 12 + (month - 1) as i64 + added_months_count as i64;
        let (year, month) = (months.div_euclid(12) as i32, months.rem_euclid(12) as i32 + 1);
        self.to_calendar_month(days_from_civil(year, month, 1) * MillisecondsIn24Hours)
    }

    fn minus_months(&self, from: &CalendarMonth, subtracted_months_count: i32) -> CalendarMonth {
        self.plus_months(from, -subtracted_months_count)
    }

    fn format_with_pattern(&self, utc_time_millis: i64, pattern: &str, _locale: &CalendarLocale) -> String {
        self.platform_date_format.format_with_pattern(utc_time_millis, pattern, &self.formatter_cache)
    }

    fn parse(&self, date: &str, pattern: &str, locale: &CalendarLocale) -> Option<CalendarDate> {
        self.platform_date_format.parse(date, pattern, locale, &self.formatter_cache)
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's `days_from_civil`, what kotlinx-datetime's
/// `LocalDate.toEpochDays` computes).
pub(crate) fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let y = year as i64 - if month <= 2 { 1 } else { 0 };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = month as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The proleptic Gregorian date of days since 1970-01-01 (Howard Hinnant's `civil_from_days`).
pub(crate) fn civil_from_days(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year as i32, month as i32, day as i32)
}

/// The ISO 8601 day of the week (Monday 1 … Sunday 7) of days since 1970-01-01, a Thursday.
pub(crate) fn iso_day_number(epoch_days: i64) -> i32 {
    ((epoch_days + 3).rem_euclid(7) + 1) as i32
}

/// Compose's `Int.isLeapYear()`.
pub(crate) fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// Compose's `Month.numberOfDays(isLeap)`.
pub(crate) fn number_of_days(year: i32, month: i32) -> i32 {
    match month {
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 30,
    }
}

thread_local! {
    /// What [`set_today_for_tests`] gave (kotlinx-datetime's `Clock` a test passes in Compose's tests).
    static TODAY_OVERRIDE: Cell<Option<fn() -> (i32, i32, i32)>> = const { Cell::new(None) };
}

/// Makes this thread's calendars read today's local date from `today` (a test's clock: year, month 1 to 12, day),
/// or from the system's clock again with `None`.
pub fn set_today_for_tests(today: Option<fn() -> (i32, i32, i32)>) {
    TODAY_OVERRIDE.with(|t| t.set(today));
}

/// Today's date in the system's time zone (kotlinx-datetime's `TimeZone.currentSystemDefault()`, the JVM's default
/// zone: kompoz-runtime's, on every target): year, month 1 to 12, day of the month.
fn local_today() -> (i32, i32, i32) {
    if let Some(today) = TODAY_OVERRIDE.with(Cell::get) {
        return today();
    }
    let now = kompoz_runtime::kt::Instant::of_epoch_milli(kompoz_runtime::clock::current_time_millis());
    let today = now.at_zone(kompoz_runtime::kt::ZoneId::system_default());
    (today.year(), today.month_value(), today.day_of_month())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_days_round_trip_and_weekdays() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(days_from_civil(2026, 8, 17)), (2026, 8, 17));
        assert_eq!(civil_from_days(days_from_civil(1600, 2, 29)), (1600, 2, 29));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        // 1 August 2026 is a Saturday; 4 January 2020 (1578096000000 ms) too.
        assert_eq!(iso_day_number(days_from_civil(2026, 8, 1)), 6);
        assert_eq!(days_from_civil(2020, 1, 4) * MillisecondsIn24Hours, 1_578_096_000_000);
    }

    #[test]
    fn months_count_by_the_locales_week() {
        let tr = KotlinxDatetimeCalendarModel::new(CalendarLocale::for_language_tag("tr"));
        let august = tr.get_month_of_year(2026, 8);
        assert_eq!((august.number_of_days, august.days_from_start_of_week_to_first_of_month), (31, 5));
        let us = KotlinxDatetimeCalendarModel::new(CalendarLocale::for_language_tag("en-US"));
        assert_eq!(us.get_month_of_year(2026, 8).days_from_start_of_week_to_first_of_month, 6);
        let february = tr.plus_months(&august, 18);
        assert_eq!((february.year, february.month, february.number_of_days), (2028, 2, 29));
        let back = tr.minus_months(&february, 30);
        assert_eq!((back.year, back.month), (2025, 8));
        assert_eq!(tr.get_month(august.end_utc_time_millis()), august);
        let date = tr.get_canonical_date(days_from_civil(2026, 8, 17) * MillisecondsIn24Hours + 5_000_000);
        assert_eq!((date.year, date.month, date.day_of_month), (2026, 8, 17));
        assert_eq!(tr.get_day_of_week(&date), 1);
    }
}

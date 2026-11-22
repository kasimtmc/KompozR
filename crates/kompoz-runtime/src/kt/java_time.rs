//! The `java.time` dates Kotlin/JVM code writes ([`LocalDate`], [`YearMonth`], [`DayOfWeek`]): what Compose's JVM
//! APIs take (material3's date pickers' `LocalDate` and `YearMonth` overloads, from Compose for desktop's
//! jvmAndAndroidMain). Proleptic ISO dates, compared and printed as Java's (`2023-04-15`, `2023-04`); Java's
//! `DateTimeException` for a date that doesn't exist is a panic with its message.

use std::fmt;

/// A date without a time or a zone (Java's `java.time.LocalDate`): ISO calendar, years −999 999 999 to 999 999 999
/// in Java (any `i32` here), months 1 to 12.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalDate {
    year: i32,
    month: u8,
    day: u8,
}

impl LocalDate {
    /// The date of a year, a month (1 to 12) and a day of the month (Java's `LocalDate.of(year, month,
    /// dayOfMonth)`); panics with Java's `DateTimeException` message if there is no such date.
    ///
    /// Kotlin: `LocalDate.of(year: Int, month: Int, dayOfMonth: Int)`
    #[track_caller]
    pub fn of(year: i32, month: i32, day_of_month: i32) -> LocalDate {
        if !(1..=12).contains(&month) {
            panic!("DateTimeException: Invalid value for MonthOfYear (valid values 1 - 12): {month}");
        }
        if day_of_month < 1 || day_of_month > days_in_month(year, month) {
            if (1..=31).contains(&day_of_month) {
                panic!("DateTimeException: Invalid date '{} {day_of_month}'", MONTH_NAMES[month as usize - 1]);
            }
            panic!("DateTimeException: Invalid value for DayOfMonth (valid values 1 - 28/31): {day_of_month}");
        }
        LocalDate { year, month: month as u8, day: day_of_month as u8 }
    }

    /// The date `epoch_day` days after 1970-01-01 (Java's `LocalDate.ofEpochDay`).
    ///
    /// Kotlin: `LocalDate.ofEpochDay(epochDay: Long)`
    pub fn of_epoch_day(epoch_day: i64) -> LocalDate {
        let z = epoch_day + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
        LocalDate { year: year as i32, month: month as u8, day: day as u8 }
    }

    /// Days since 1970-01-01 (Java's `toEpochDay`).
    pub fn to_epoch_day(&self) -> i64 {
        let (year, month, day) = (self.year as i64, self.month as i64, self.day as i64);
        let y = year - if month <= 2 { 1 } else { 0 };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The year (Java's `getYear`, Kotlin's `year`).
    pub fn year(&self) -> i32 {
        self.year
    }

    /// The month, 1 to 12 (Java's `getMonthValue`, Kotlin's `monthValue`).
    pub fn month_value(&self) -> i32 {
        self.month as i32
    }

    /// The day of the month (Java's `getDayOfMonth`, Kotlin's `dayOfMonth`).
    pub fn day_of_month(&self) -> i32 {
        self.day as i32
    }

    /// The day of the week (Java's `getDayOfWeek`, Kotlin's `dayOfWeek`).
    pub fn day_of_week(&self) -> DayOfWeek {
        DayOfWeek::of(((self.to_epoch_day() + 3).rem_euclid(7) + 1) as i32)
    }

    /// The month of the date (Java's `YearMonth.from(date)`).
    ///
    /// Kotlin: `YearMonth.from(temporal: LocalDate)`
    pub fn year_month(&self) -> YearMonth {
        YearMonth { year: self.year, month: self.month }
    }

    /// `days` days later (Java's `plusDays`).
    pub fn plus_days(&self, days: i64) -> LocalDate {
        LocalDate::of_epoch_day(self.to_epoch_day() + days)
    }

    /// Whether this date is before `other` (Java's `isBefore`).
    pub fn is_before(&self, other: &LocalDate) -> bool {
        self < other
    }

    /// Whether this date is after `other` (Java's `isAfter`).
    pub fn is_after(&self, other: &LocalDate) -> bool {
        self > other
    }
}

/// ISO 8601 as Java prints it: `2023-04-15`, a year beyond four digits with its sign (`+10000-01-01`).
impl fmt::Display for LocalDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_year(f, self.year)?;
        write!(f, "-{:02}-{:02}", self.month, self.day)
    }
}

impl fmt::Debug for LocalDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// A month of a year (Java's `java.time.YearMonth`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct YearMonth {
    year: i32,
    month: u8,
}

impl YearMonth {
    /// The month (1 to 12) of a year (Java's `YearMonth.of(year, month)`); panics with Java's message for another
    /// month.
    ///
    /// Kotlin: `YearMonth.of(year: Int, month: Int)`
    #[track_caller]
    pub fn of(year: i32, month: i32) -> YearMonth {
        if !(1..=12).contains(&month) {
            panic!("DateTimeException: Invalid value for MonthOfYear (valid values 1 - 12): {month}");
        }
        YearMonth { year, month: month as u8 }
    }

    /// The year (Java's `getYear`, Kotlin's `year`).
    pub fn year(&self) -> i32 {
        self.year
    }

    /// The month, 1 to 12 (Java's `getMonthValue`, Kotlin's `monthValue`).
    pub fn month_value(&self) -> i32 {
        self.month as i32
    }

    /// The month's number of days (Java's `lengthOfMonth`).
    pub fn length_of_month(&self) -> i32 {
        days_in_month(self.year, self.month as i32)
    }

    /// The date of a day of this month (Java's `atDay`).
    ///
    /// Kotlin: `atDay(dayOfMonth: Int)`
    #[track_caller]
    pub fn at_day(&self, day_of_month: i32) -> LocalDate {
        LocalDate::of(self.year, self.month as i32, day_of_month)
    }
}

/// ISO 8601 as Java prints it: `2023-04`.
impl fmt::Display for YearMonth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_year(f, self.year)?;
        write!(f, "-{:02}", self.month)
    }
}

impl fmt::Debug for YearMonth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// A day of the week (Java's enum `java.time.DayOfWeek`), Monday first, as ISO 8601 numbers them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum DayOfWeek {
    MONDAY = 1,
    TUESDAY,
    WEDNESDAY,
    THURSDAY,
    FRIDAY,
    SATURDAY,
    SUNDAY,
}

impl DayOfWeek {
    /// The day of an ISO number, 1 (Monday) to 7 (Sunday) (Java's `DayOfWeek.of`).
    ///
    /// Kotlin: `DayOfWeek.of(dayOfWeek: Int)`
    #[track_caller]
    pub fn of(day_of_week: i32) -> DayOfWeek {
        match day_of_week {
            1 => DayOfWeek::MONDAY,
            2 => DayOfWeek::TUESDAY,
            3 => DayOfWeek::WEDNESDAY,
            4 => DayOfWeek::THURSDAY,
            5 => DayOfWeek::FRIDAY,
            6 => DayOfWeek::SATURDAY,
            7 => DayOfWeek::SUNDAY,
            _ => panic!("DateTimeException: Invalid value for DayOfWeek: {day_of_week}"),
        }
    }

    /// The ISO number, 1 (Monday) to 7 (Sunday) (Java's `getValue`, Kotlin's `value`).
    pub fn value(&self) -> i32 {
        *self as i32
    }
}

/// Java's `toString`: the constant's name.
impl fmt::Display for DayOfWeek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

const MONTH_NAMES: [&str; 12] = [
    "JANUARY",
    "FEBRUARY",
    "MARCH",
    "APRIL",
    "MAY",
    "JUNE",
    "JULY",
    "AUGUST",
    "SEPTEMBER",
    "OCTOBER",
    "NOVEMBER",
    "DECEMBER",
];

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        2 => 28 + is_leap(year) as i32,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Java's ISO year: four digits at least, a sign beyond four or below zero.
fn write_year(f: &mut fmt::Formatter<'_>, year: i32) -> fmt::Result {
    if year.unsigned_abs() < 10_000 {
        if year < 0 { write!(f, "-{:04}", year.unsigned_abs()) } else { write!(f, "{year:04}") }
    } else if year > 0 {
        write!(f, "+{year}")
    } else {
        write!(f, "{year}")
    }
}

/// Kotlin's `toString` of the dates, as string templates write them: Java's ISO text.
macro_rules! kt_to_string_by_display {
    ($($t:ty),*) => {$(
        impl crate::kt::KtToString for $t {
            fn kt_write(&self, out: &mut String) {
                use fmt::Write;
                let _ = write!(out, "{self}");
            }
        }
    )*};
}
kt_to_string_by_display!(LocalDate, YearMonth, DayOfWeek);

/// `start..end` of dates or months: a range of what isn't counted (they're `Comparable` in Java), printed
/// `2023-04-15..2023-04-20`.
macro_rules! comparable_range_bound {
    ($($t:ty),*) => {$(
        impl crate::kt::KtRangeBound for $t {
            fn kt_until(start: $t, end: $t) -> crate::kt::KtProgression<$t> {
                crate::kt::KtProgression::open_end(start, end)
            }
            fn kt_contains(range: &crate::kt::KtProgression<$t>, value: &$t) -> bool {
                crate::kt::range::ordered_contains(range, value)
            }
            fn kt_is_empty(range: &crate::kt::KtProgression<$t>) -> bool {
                crate::kt::range::ordered_is_empty(range)
            }
        }
    )*};
}
comparable_range_bound!(LocalDate, YearMonth);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_count_and_print_as_java_does() {
        let date = LocalDate::of(2023, 4, 15);
        assert_eq!(date.to_string(), "2023-04-15");
        assert_eq!((date.year(), date.month_value(), date.day_of_month()), (2023, 4, 15));
        assert_eq!(date.day_of_week(), DayOfWeek::SATURDAY);
        assert_eq!(LocalDate::of_epoch_day(date.to_epoch_day()), date);
        assert_eq!(LocalDate::of(1970, 1, 1).to_epoch_day(), 0);
        assert_eq!(date.plus_days(20), LocalDate::of(2023, 5, 5));
        assert!(date.is_before(&LocalDate::of(2023, 4, 20)) && !date.is_after(&LocalDate::of(2024, 1, 1)));
        assert_eq!(YearMonth::of(2023, 4).to_string(), "2023-04");
        assert_eq!(YearMonth::of(2024, 2).length_of_month(), 29);
        assert_eq!(date.year_month().at_day(1), LocalDate::of(2023, 4, 1));
        assert_eq!(LocalDate::of(12345, 1, 2).to_string(), "+12345-01-02");
        assert_eq!(LocalDate::of(-5, 1, 2).to_string(), "-0005-01-02");
        assert_eq!(DayOfWeek::of(7).to_string(), "SUNDAY");
        let refused = std::panic::catch_unwind(|| LocalDate::of(2023, 2, 30)).unwrap_err();
        assert_eq!(refused.downcast_ref::<String>().unwrap(), "DateTimeException: Invalid date 'FEBRUARY 30'");
    }
}

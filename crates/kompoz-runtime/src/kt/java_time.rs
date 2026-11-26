//! The `java.time` dates Kotlin/JVM code writes ([`LocalDate`], [`YearMonth`], [`DayOfWeek`]): what Compose's JVM
//! APIs take (material3's date pickers' `LocalDate` and `YearMonth` overloads, from Compose for desktop's
//! jvmAndAndroidMain), and the instants and zones a `SelectableDates` reads a UTC day's weekday through
//! ([`Instant`], [`ZoneId`], [`ZonedDateTime`]: DatePickerSamples' `Instant.ofEpochMilli(t).atZone(ZoneId.of("UTC"))
//! .toLocalDate().dayOfWeek`; zones from the system's database, as `java.util`'s). Proleptic ISO dates, compared and
//! printed as Java's (`2023-04-15`, `2023-04`, `2023-04-15T10:15:30Z`); Java's `DateTimeException` for a date that
//! doesn't exist is a panic with its message.

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

/// An instant on the time line (Java's `java.time.Instant`): seconds and nanoseconds since 1970-01-01T00:00Z.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Instant {
    seconds: i64,
    nanos: i32,
}

impl Instant {
    /// The instant `epoch_milli` milliseconds after the epoch (Java's `Instant.ofEpochMilli`).
    ///
    /// Kotlin: `Instant.ofEpochMilli(epochMilli: Long)`
    pub fn of_epoch_milli(epoch_milli: i64) -> Instant {
        Instant { seconds: epoch_milli.div_euclid(1000), nanos: epoch_milli.rem_euclid(1000) as i32 * 1_000_000 }
    }

    /// The instant `epoch_second` seconds after the epoch (Java's `Instant.ofEpochSecond`).
    ///
    /// Kotlin: `Instant.ofEpochSecond(epochSecond: Long)`
    pub fn of_epoch_second(epoch_second: i64) -> Instant {
        Instant { seconds: epoch_second, nanos: 0 }
    }

    /// Now, by the system's clock (Java's `Instant.now()`).
    ///
    /// Kotlin: `Instant.now()`
    pub fn now() -> Instant {
        Instant::of_epoch_milli(super::java_util::current_time_millis())
    }

    /// Milliseconds since the epoch (Java's `toEpochMilli`).
    pub fn to_epoch_milli(&self) -> i64 {
        self.seconds * 1000 + (self.nanos / 1_000_000) as i64
    }

    /// Whole seconds since the epoch (Java's `getEpochSecond`, Kotlin's `epochSecond`).
    pub fn epoch_second(&self) -> i64 {
        self.seconds
    }

    /// The nanoseconds within the second (Java's `getNano`, Kotlin's `nano`).
    pub fn nano(&self) -> i32 {
        self.nanos
    }

    /// The date and time of this instant in `zone` (Java's `atZone`).
    pub fn at_zone(&self, zone: ZoneId) -> ZonedDateTime {
        ZonedDateTime::of_instant(*self, zone)
    }

    /// Whether this instant is before `other` (Java's `isBefore`).
    pub fn is_before(&self, other: Instant) -> bool {
        *self < other
    }

    /// Whether this instant is after `other` (Java's `isAfter`).
    pub fn is_after(&self, other: Instant) -> bool {
        *self > other
    }
}

/// ISO 8601 in UTC as Java prints it: `2023-04-15T10:15:30Z`, a fraction of a second by threes (`.120Z`).
impl fmt::Display for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let date = LocalDate::of_epoch_day(self.seconds.div_euclid(86_400));
        let second_of_day = self.seconds.rem_euclid(86_400);
        write!(f, "{date}T{:02}:{:02}:{:02}", second_of_day / 3600, second_of_day / 60 % 60, second_of_day % 60)?;
        write_fraction(f, self.nanos)?;
        f.write_str("Z")
    }
}

impl fmt::Debug for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Java's fraction of a second: none for 0, else 3, 6 or 9 digits.
fn write_fraction(f: &mut fmt::Formatter<'_>, nanos: i32) -> fmt::Result {
    if nanos == 0 {
        Ok(())
    } else if nanos % 1_000_000 == 0 {
        write!(f, ".{:03}", nanos / 1_000_000)
    } else if nanos % 1000 == 0 {
        write!(f, ".{:06}", nanos / 1000)
    } else {
        write!(f, ".{nanos:09}")
    }
}

/// A time zone's ID (Java's `java.time.ZoneId`): a region of the system's zone database (`Europe/Istanbul`, `UTC`),
/// or a fixed offset (`ZoneOffset`: `Z`, `+03:00`; `UTC+03:00` a region of one).
#[derive(Clone, PartialEq)]
pub struct ZoneId {
    id: std::sync::Arc<str>,
    zone: super::java_util::TimeZone,
    /// Whether it is an offset itself (Java's `ZoneOffset`), not a region.
    offset_only: bool,
}

impl ZoneId {
    /// The zone of an ID (Java's `ZoneId.of`): `Z` or an offset (`+3`, `+03:00`, `-0530`), a prefix and an offset
    /// (`UTC+3`, `GMT+03:00`), or a region; panics with Java's `ZoneRulesException` for a region the system's
    /// database hasn't, `DateTimeException` for an ID that isn't one.
    ///
    /// Kotlin: `ZoneId.of(zoneId: String)`
    #[track_caller]
    pub fn of(zone_id: &str) -> ZoneId {
        if zone_id == "Z" || zone_id.starts_with(['+', '-']) {
            let Some(offset) = parse_zone_offset(zone_id) else {
                panic!("DateTimeException: Invalid ID for ZoneOffset, invalid format: {zone_id}");
            };
            return ZoneId::of_offset(offset);
        }
        for prefix in ["UTC", "GMT", "UT"] {
            if let Some(rest) = zone_id.strip_prefix(prefix)
                && (rest.is_empty() || rest.starts_with(['+', '-']))
            {
                let offset = if rest.is_empty() {
                    0
                } else {
                    match parse_zone_offset(rest) {
                        Some(o) => o,
                        None => panic!("DateTimeException: Invalid ID for offset-based ZoneId: {zone_id}"),
                    }
                };
                let id = if offset == 0 { prefix.to_string() } else { format!("{prefix}{}", offset_id(offset)) };
                return ZoneId::fixed(&id, offset, false);
            }
        }
        let valid = zone_id.len() >= 2
            && zone_id.starts_with(|c: char| c.is_ascii_alphabetic())
            && zone_id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '~' | '+' | '-'));
        if !valid {
            panic!("DateTimeException: Invalid ID for region-based ZoneId, invalid format: {zone_id}");
        }
        match super::zoneinfo::load(zone_id) {
            Some(rules) => ZoneId {
                id: zone_id.into(),
                zone: super::java_util::TimeZone::of_rules(zone_id, rules),
                offset_only: false,
            },
            None => panic!("ZoneRulesException: Unknown time-zone ID: {zone_id}"),
        }
    }

    /// The system's zone (Java's `ZoneId.systemDefault()`: the default `TimeZone`'s).
    ///
    /// Kotlin: `ZoneId.systemDefault()`
    pub fn system_default() -> ZoneId {
        let zone = super::java_util::TimeZone::get_default();
        ZoneId { id: zone.id().into(), zone, offset_only: false }
    }

    fn of_offset(offset: i32) -> ZoneId {
        let id = if offset == 0 { "Z".to_string() } else { offset_id(offset) };
        ZoneId::fixed(&id, offset, true)
    }

    fn fixed(id: &str, offset: i32, offset_only: bool) -> ZoneId {
        let rules = std::sync::Arc::new(super::zoneinfo::ZoneRules::fixed(offset, id));
        ZoneId { id: id.into(), zone: super::java_util::TimeZone::of_rules(id, rules), offset_only }
    }

    /// The ID (Java's `getId`, Kotlin's `id`).
    pub fn id(&self) -> String {
        self.id.to_string()
    }
}

/// The ID.
impl fmt::Display for ZoneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)
    }
}

impl fmt::Debug for ZoneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// An offset's seconds of `±h`, `±hh`, `±hh:mm`, `±hhmm`, `±hh:mm:ss` (Java's `ZoneOffset.of`, to ±18:00).
fn parse_zone_offset(text: &str) -> Option<i32> {
    if text == "Z" {
        return Some(0);
    }
    let (sign, rest) = match text.as_bytes().first()? {
        b'+' => (1, &text[1..]),
        b'-' => (-1, &text[1..]),
        _ => return None,
    };
    if !rest.bytes().all(|b| b.is_ascii_digit() || b == b':') {
        return None;
    }
    let n = |s: &str| s.parse::<i32>().ok();
    let (h, m, s) = match (rest.len(), rest.contains(':')) {
        (1 | 2, false) => (n(rest)?, 0, 0),
        (4, false) => (n(&rest[..2])?, n(&rest[2..])?, 0),
        (6, false) => (n(&rest[..2])?, n(&rest[2..4])?, n(&rest[4..])?),
        (5, true) if &rest[2..3] == ":" => (n(&rest[..2])?, n(&rest[3..])?, 0),
        (8, true) if &rest[2..3] == ":" && &rest[5..6] == ":" => (n(&rest[..2])?, n(&rest[3..5])?, n(&rest[6..])?),
        _ => return None,
    };
    (h <= 18 && m <= 59 && s <= 59 && (h < 18 || m + s == 0)).then_some(sign * (h * 3600 + m * 60 + s))
}

/// Java's `ZoneOffset` ID: `+03:00`, `-05:30:15`.
fn offset_id(offset: i32) -> String {
    let a = offset.abs();
    let sign = if offset < 0 { '-' } else { '+' };
    if a % 60 == 0 {
        format!("{sign}{:02}:{:02}", a / 3600, a / 60 % 60)
    } else {
        format!("{sign}{:02}:{:02}:{:02}", a / 3600, a / 60 % 60, a % 60)
    }
}

/// A date and time in a zone (Java's `java.time.ZonedDateTime`): the instant's local date and time there and the
/// zone's offset at it.
#[derive(Clone, PartialEq)]
pub struct ZonedDateTime {
    date: LocalDate,
    second_of_day: i32,
    nanos: i32,
    offset: i32,
    zone: ZoneId,
}

impl ZonedDateTime {
    fn of_instant(instant: Instant, zone: ZoneId) -> ZonedDateTime {
        let offset = zone.zone.get_offset(instant.seconds * 1000) / 1000;
        let local = instant.seconds + offset as i64;
        ZonedDateTime {
            date: LocalDate::of_epoch_day(local.div_euclid(86_400)),
            second_of_day: local.rem_euclid(86_400) as i32,
            nanos: instant.nanos,
            offset,
            zone,
        }
    }

    /// The date (Java's `toLocalDate`).
    pub fn to_local_date(&self) -> LocalDate {
        self.date
    }

    /// The instant (Java's `toInstant`).
    pub fn to_instant(&self) -> Instant {
        Instant { seconds: self.to_epoch_second(), nanos: self.nanos }
    }

    /// Seconds since the epoch (Java's `toEpochSecond`).
    pub fn to_epoch_second(&self) -> i64 {
        self.date.to_epoch_day() * 86_400 + self.second_of_day as i64 - self.offset as i64
    }

    /// The year (Java's `getYear`, Kotlin's `year`).
    pub fn year(&self) -> i32 {
        self.date.year()
    }

    /// The month, 1 to 12 (Java's `getMonthValue`, Kotlin's `monthValue`).
    pub fn month_value(&self) -> i32 {
        self.date.month_value()
    }

    /// The day of the month (Java's `getDayOfMonth`, Kotlin's `dayOfMonth`).
    pub fn day_of_month(&self) -> i32 {
        self.date.day_of_month()
    }

    /// The day of the week (Java's `getDayOfWeek`, Kotlin's `dayOfWeek`).
    pub fn day_of_week(&self) -> DayOfWeek {
        self.date.day_of_week()
    }

    /// The hour, 0 to 23 (Java's `getHour`, Kotlin's `hour`).
    pub fn hour(&self) -> i32 {
        self.second_of_day / 3600
    }

    /// The minute (Java's `getMinute`, Kotlin's `minute`).
    pub fn minute(&self) -> i32 {
        self.second_of_day / 60 % 60
    }

    /// The second (Java's `getSecond`, Kotlin's `second`).
    pub fn second(&self) -> i32 {
        self.second_of_day % 60
    }

    /// The nanoseconds within the second (Java's `getNano`, Kotlin's `nano`).
    pub fn nano(&self) -> i32 {
        self.nanos
    }

    /// The zone (Java's `getZone`, Kotlin's `zone`).
    pub fn zone(&self) -> ZoneId {
        self.zone.clone()
    }
}

/// ISO 8601 as Java prints it: `2023-04-15T10:15Z[UTC]`, `2023-04-15T13:15:30+03:00[Europe/Istanbul]` (the
/// seconds only when not zero; the region's ID after an offset's).
impl fmt::Display for ZonedDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}T{:02}:{:02}", self.date, self.hour(), self.minute())?;
        if self.second() != 0 || self.nanos != 0 {
            write!(f, ":{:02}", self.second())?;
            write_fraction(f, self.nanos)?;
        }
        f.write_str(&if self.offset == 0 { "Z".to_string() } else { offset_id(self.offset) })?;
        if !self.zone.offset_only {
            write!(f, "[{}]", self.zone.id)?;
        }
        Ok(())
    }
}

impl fmt::Debug for ZonedDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
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
kt_to_string_by_display!(LocalDate, YearMonth, DayOfWeek, Instant, ZoneId, ZonedDateTime);

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

    #[test]
    fn instants_in_zones_as_java_has_them() {
        // 2023-04-15T10:15:30.120Z, a Saturday.
        let instant = Instant::of_epoch_milli(1_681_553_730_120);
        assert_eq!(instant.to_string(), "2023-04-15T10:15:30.120Z");
        assert_eq!(
            (instant.epoch_second(), instant.nano(), instant.to_epoch_milli()),
            (1_681_553_730, 120_000_000, 1_681_553_730_120)
        );
        assert_eq!(Instant::of_epoch_second(0).to_string(), "1970-01-01T00:00:00Z");
        let utc = instant.at_zone(ZoneId::of("UTC"));
        assert_eq!(utc.to_local_date().day_of_week(), DayOfWeek::SATURDAY);
        assert_eq!(utc.to_string(), "2023-04-15T10:15:30.120Z[UTC]");
        let late = Instant::of_epoch_second(1_681_595_000).at_zone(ZoneId::of("+03:00"));
        assert_eq!((late.to_local_date(), late.hour()), (LocalDate::of(2023, 4, 16), 0));
        assert_eq!(late.to_string(), "2023-04-16T00:43:20+03:00");
        assert_eq!(late.to_instant(), Instant::of_epoch_second(1_681_595_000));
        assert_eq!(ZoneId::of("UTC+3").id(), "UTC+03:00");
        assert_eq!(ZoneId::of("Z").id(), "Z");
        let refused = std::panic::catch_unwind(|| ZoneId::of("Nowhere/Never")).unwrap_err();
        assert_eq!(
            refused.downcast_ref::<String>().unwrap(),
            "ZoneRulesException: Unknown time-zone ID: Nowhere/Never"
        );
        assert!(std::panic::catch_unwind(|| ZoneId::of("+25:00")).is_err());
    }
}

//! A locale's date formats (Compose for desktop's internal `PlatformDateFormat`, skikoMain's expect class with
//! desktopMain's actual: `java.util.Calendar`, `java.text.SimpleDateFormat` and `java.time.format` there, CLDR's data
//! through ICU4X here — the same CLDR the JDK's locale data comes from).
//!
//! Dates are Gregorian whatever the locale's own calendar (Compose's date pickers count in kotlinx-datetime's ISO
//! calendar; Java's `SimpleDateFormat` would switch to Thai Buddhist years in `th-TH`, a JDK quirk not kept).

use std::rc::Rc;

use icu_calendar::Date;
use icu_calendar::cal::Gregorian;
use icu_calendar::types::Weekday;
use icu_calendar::week::WeekInformation;
use icu_datetime::fieldsets::enums::DateFieldSet;
use icu_datetime::fieldsets::{T, YMD, YMDE};
use icu_datetime::input::Time;
use icu_datetime::pattern::{DateTimePattern, FixedCalendarDateTimeNames};
use icu_datetime::{FixedCalendarDateTimeFormatter, NoCalendarFormatter};
use writeable::{TryWriteable, Writeable};

use crate::DatePickerDefaults;
use crate::calendar_locale::CalendarLocale;
use crate::internal::calendar_model::{
    CalendarDate, DateInputFormat, FormatterCache, MillisecondsIn24Hours, date_pattern_as_input_format,
};
use crate::internal::kotlinx_datetime_calendar_model::{civil_from_days, days_from_civil, number_of_days};

/// The date formats of a locale (Compose for desktop's `PlatformDateFormat(locale)`).
pub(crate) struct PlatformDateFormat {
    locale: CalendarLocale,
}

impl PlatformDateFormat {
    pub(crate) fn new(locale: CalendarLocale) -> PlatformDateFormat {
        PlatformDateFormat { locale }
    }

    /// The locale's first day of the week, ISO 8601 numbering (desktop: `Calendar.getInstance(locale)
    /// .firstDayOfWeek` through `dayInISO8601`; CLDR's week data of the locale's region, or of its likely region: `en` is
    /// the US's, Sunday first).
    pub(crate) fn first_day_of_week(&self) -> i32 {
        let weekday = WeekInformation::try_new(self.locale.icu_locale().into())
            .map(|info| info.first_weekday)
            .unwrap_or(Weekday::Monday);
        weekday as i32
    }

    /// The weekdays' full and narrow names, Monday first (desktop: `DayOfWeek.getDisplayName(TextStyle.FULL)` and
    /// `TextStyle.NARROW`).
    pub(crate) fn weekday_names(&self) -> Vec<(String, String)> {
        // 3 January 2000 was a Monday.
        (0..7)
            .map(|i| {
                let millis = (days_from_civil(2000, 1, 3) + i) * MillisecondsIn24Hours;
                (self.format_pattern(millis, "EEEE"), self.format_pattern(millis, "EEEEE"))
            })
            .collect()
    }

    /// `utc_time_millis` formatted by a date pattern (desktop: `SimpleDateFormat(pattern, locale)` in UTC; the
    /// pattern letters are CLDR's, the same as Java's for dates).
    pub(crate) fn format_with_pattern(&self, utc_time_millis: i64, pattern: &str, cache: &FormatterCache) -> String {
        let key = format!("{pattern}{}", self.locale.to_language_tag());
        let formatter = {
            let found = cache.borrow().get(&key).cloned();
            match found.and_then(|f| f.downcast::<PatternFormatter>().ok()) {
                Some(formatter) => formatter,
                None => {
                    let formatter = Rc::new(PatternFormatter::new(&self.locale, pattern));
                    cache.borrow_mut().insert(key, formatter.clone());
                    formatter
                }
            }
        };
        formatter.format(utc_time_millis)
    }

    fn format_pattern(&self, utc_time_millis: i64, pattern: &str) -> String {
        PatternFormatter::new(&self.locale, pattern).format(utc_time_millis)
    }

    /// `utc_time_millis` formatted by a skeleton (desktop's actual: the skeletons Compose's date picker uses are
    /// the locale's medium and full dates and "LLLL yyyy"; any other skeleton is taken as a pattern).
    pub(crate) fn format_with_skeleton(&self, utc_time_millis: i64, skeleton: &str, cache: &FormatterCache) -> String {
        let date = gregorian_date(utc_time_millis);
        let prefs = self.locale.icu_locale().into();
        let length_formatted = if skeleton == DatePickerDefaults::YearAbbrMonthDaySkeleton {
            // `DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM)`.
            FixedCalendarDateTimeFormatter::<Gregorian, YMD>::try_new(prefs, YMD::medium())
                .ok()
                .map(|f| f.format(&date).write_to_string().into_owned())
        } else if skeleton == DatePickerDefaults::YearMonthWeekdayDaySkeleton {
            // `DateTimeFormatter.ofLocalizedDate(FormatStyle.FULL)`.
            FixedCalendarDateTimeFormatter::<Gregorian, YMDE>::try_new(prefs, YMDE::long())
                .ok()
                .map(|f| f.format(&date).write_to_string().into_owned())
        } else {
            None
        };
        if let Some(formatted) = length_formatted {
            return formatted;
        }
        let pattern = if skeleton == DatePickerDefaults::YearMonthSkeleton {
            // L is a pattern for standalone month (without day).
            "LLLL yyyy"
        } else {
            skeleton
        };
        self.format_with_pattern(utc_time_millis, pattern, cache)
    }

    /// The date `date` is by a numeric `pattern` (desktop: `SimpleDateFormat(pattern)` in UTC, not lenient: its
    /// day, month and year fields, adjacent ones by their widths, the last taking the digits left; other
    /// characters must be as they are; what follows the pattern is ignored, as Java's `parse` does).
    pub(crate) fn parse(
        &self,
        date: &str,
        pattern: &str,
        _locale: &CalendarLocale,
        _cache: &FormatterCache,
    ) -> Option<CalendarDate> {
        parse_numeric_date(date, pattern)
    }

    /// The format dates are typed in: the locale's short date pattern as an input format (desktop:
    /// `DateFormat.getDateInstance(DateFormat.SHORT, locale)`'s pattern through `datePatternAsInputFormat`).
    pub(crate) fn get_date_input_format(&self) -> DateInputFormat {
        let date = gregorian_date(0);
        let pattern =
            FixedCalendarDateTimeFormatter::<Gregorian, YMD>::try_new(self.locale.icu_locale().into(), YMD::short())
                .map(|f| f.format(&date).pattern().write_to_string().into_owned())
                .unwrap_or_else(|_| "y-MM-dd".to_string());
        date_pattern_as_input_format(&pattern)
    }

    /// Whether the locale writes times by a 24-hour clock (desktop: its long time pattern has `H`).
    pub(crate) fn is_24_hour_format(&self) -> bool {
        let Ok(formatter) = NoCalendarFormatter::<T>::try_new(self.locale.icu_locale().into(), T::hm()) else {
            return false;
        };
        let Ok(time) = Time::try_new(13, 0, 0, 0) else { return false };
        let pattern = formatter.format(&time).pattern().write_to_string().into_owned();
        // Outside quotes, an hour of 0–23 (`H`) or 1–24 (`k`).
        let mut quoted = false;
        pattern.chars().any(|c| {
            if c == '\'' {
                quoted = !quoted;
            }
            !quoted && (c == 'H' || c == 'k')
        })
    }
}

/// A pattern with the names it needs loaded (what desktop's cached `SimpleDateFormat` is).
struct PatternFormatter {
    names: Option<FixedCalendarDateTimeNames<Gregorian, DateFieldSet>>,
    pattern: Option<DateTimePattern>,
}

impl PatternFormatter {
    fn new(locale: &CalendarLocale, pattern: &str) -> PatternFormatter {
        let pattern = DateTimePattern::try_from_pattern_str(pattern).ok();
        let names = pattern.as_ref().and_then(|pattern| {
            let mut names =
                FixedCalendarDateTimeNames::<Gregorian, DateFieldSet>::try_new(locale.icu_locale().into()).ok()?;
            names.include_for_pattern(pattern).ok()?;
            Some(names)
        });
        PatternFormatter { names, pattern }
    }

    fn format(&self, utc_time_millis: i64) -> String {
        let (Some(names), Some(pattern)) = (&self.names, &self.pattern) else {
            return String::new();
        };
        let date = gregorian_date(utc_time_millis);
        let formatted = names.with_pattern_unchecked(pattern).format(&date);
        match formatted.try_write_to_string() {
            Ok(s) => s.into_owned(),
            // Written with what fallbacks it could (a field it has no names for).
            Err((_, s)) => s.into_owned(),
        }
    }
}

/// The Gregorian date of a UTC instant's day.
fn gregorian_date(utc_time_millis: i64) -> Date<Gregorian> {
    let (year, month, day) = civil_from_days(utc_time_millis.div_euclid(MillisecondsIn24Hours));
    Date::try_new_gregorian(year, month as u8, day as u8).unwrap_or_else(|_| {
        // Out of ICU4X's range of years: the nearest date it has.
        Date::try_new_gregorian(year.clamp(-9999, 9999), month as u8, day as u8).expect("a valid Gregorian date")
    })
}

/// [`PlatformDateFormat::parse`]'s numeric parsing of `SimpleDateFormat` patterns.
fn parse_numeric_date(date: &str, pattern: &str) -> Option<CalendarDate> {
    // The pattern's fields: (letter, count) runs and literal characters.
    enum Item {
        Field(char, usize),
        Literal(char),
    }
    let mut items: Vec<Item> = Vec::new();
    let mut chars = pattern.chars().peekable();
    let mut quoted = false;
    while let Some(c) = chars.next() {
        if c == '\'' {
            if chars.peek() == Some(&'\'') {
                chars.next();
                items.push(Item::Literal('\''));
            } else {
                quoted = !quoted;
            }
        } else if !quoted && c.is_ascii_alphabetic() {
            let mut count = 1;
            while chars.peek() == Some(&c) {
                chars.next();
                count += 1;
            }
            items.push(Item::Field(c, count));
        } else {
            items.push(Item::Literal(c));
        }
    }
    let text: Vec<char> = date.chars().collect();
    let mut position = 0usize;
    let (mut year, mut month, mut day) = (None, None, None);
    for (index, item) in items.iter().enumerate() {
        match item {
            Item::Literal(c) => {
                // Java skips white space where the pattern has some.
                if c.is_whitespace() {
                    while text.get(position).is_some_and(|t| t.is_whitespace()) {
                        position += 1;
                    }
                    continue;
                }
                if text.get(position) != Some(c) {
                    return None;
                }
                position += 1;
            }
            Item::Field(letter, count) => {
                // A field followed by another numeric field takes its width; otherwise all the digits there.
                let abutting = matches!(items.get(index + 1), Some(Item::Field(..)));
                let start = position;
                while text.get(position).is_some_and(|t| t.is_ascii_digit()) && (!abutting || position - start < *count)
                {
                    position += 1;
                }
                if position == start {
                    return None;
                }
                let value: i64 = text[start..position].iter().collect::<String>().parse().ok()?;
                let value = i32::try_from(value).ok()?;
                match letter {
                    'y' | 'u' => {
                        // "yy" with two digits is a year within 80 years before and 20 after now (Java's default
                        // century); any other width is the year as written.
                        year = Some(if *count == 2 && position - start == 2 { two_digit_year(value) } else { value })
                    }
                    'M' | 'L' => month = Some(value),
                    'd' => day = Some(value),
                    // Another numeric field isn't a date's: Java would read it and ignore it for the date.
                    _ => {}
                }
            }
        }
    }
    let (year, month, day) = (year.unwrap_or(1970), month.unwrap_or(1), day.unwrap_or(1));
    // Not lenient: the fields must make a date.
    if !(1..=12).contains(&month) || day < 1 || day > number_of_days(year, month) {
        return None;
    }
    Some(CalendarDate {
        year,
        month,
        day_of_month: day,
        utc_time_millis: days_from_civil(year, month, day) * MillisecondsIn24Hours,
    })
}

/// A two-digit year in Java's default century: the 100 years from 80 years before today.
fn two_digit_year(value: i32) -> i32 {
    let this_year = civil_from_days(kompoz_runtime::clock::unix_time().as_secs().div_euclid(86_400)).0;
    let start = this_year - 80;
    let candidate = start - start.rem_euclid(100) + value;
    if candidate < start { candidate + 100 } else { candidate }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn millis(year: i32, month: i32, day: i32) -> i64 {
        days_from_civil(year, month, day) * MillisecondsIn24Hours
    }

    #[test]
    fn dates_are_written_as_the_locale_writes_them() {
        let cache = FormatterCache::new(HashMap::new());
        let en = PlatformDateFormat::new(CalendarLocale::for_language_tag("en"));
        let tr = PlatformDateFormat::new(CalendarLocale::for_language_tag("tr"));
        let day = millis(2026, 8, 17);
        assert_eq!(en.format_with_skeleton(day, "yMMMM", &cache), "August 2026");
        assert_eq!(tr.format_with_skeleton(day, "yMMMM", &cache), "Ağustos 2026");
        assert_eq!(en.format_with_skeleton(day, "yMMMd", &cache), "Aug 17, 2026");
        assert_eq!(tr.format_with_skeleton(day, "yMMMd", &cache), "17 Ağu 2026");
        assert_eq!(en.format_with_skeleton(day, "yMMMMEEEEd", &cache), "Monday, August 17, 2026");
        assert_eq!(tr.format_with_skeleton(day, "yMMMMEEEEd", &cache), "17 Ağustos 2026 Pazartesi");
        assert_eq!(en.format_with_pattern(day, "dd/MM/yyyy", &cache), "17/08/2026");
        assert_eq!(en.first_day_of_week(), 7);
        assert_eq!(tr.first_day_of_week(), 1);
        assert_eq!(en.weekday_names()[0], ("Monday".to_string(), "M".to_string()));
        assert_eq!(tr.weekday_names()[6], ("Pazar".to_string(), "P".to_string()));
        assert_eq!(en.get_date_input_format().pattern_with_delimiters, "MM/dd/yyyy");
        assert_eq!(tr.get_date_input_format().pattern_with_delimiters, "dd.MM.yyyy");
        assert!(!en.is_24_hour_format());
        assert!(tr.is_24_hour_format());
    }

    #[test]
    fn typed_dates_parse_strictly() {
        let d = parse_numeric_date("15082026", "ddMMyyyy").unwrap();
        assert_eq!((d.year, d.month, d.day_of_month, d.utc_time_millis), (2026, 8, 15, millis(2026, 8, 15)));
        assert!(parse_numeric_date("31022026", "ddMMyyyy").is_none());
        assert!(parse_numeric_date("15/08", "dd/MM/yyyy").is_none());
        assert_eq!(parse_numeric_date("08-15-2026", "MM-dd-yyyy").unwrap().day_of_month, 15);
        assert!(parse_numeric_date("1x082026", "ddMMyyyy").is_none());
    }
}

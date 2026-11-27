//! `java.text.SimpleDateFormat`: dates written and read by Java's pattern letters (`hh:mm a`, `dd.MM.yyyy`,
//! `EEE, d MMM yyyy HH:mm:ss Z`) in a locale — its month, weekday, era and AM/PM names, its zones' names and its
//! digits from CLDR through ICU4X (the JDK's locale data is CLDR's too), the fields Java's ([`super::java_util`]'s
//! calendar: the zone's, Julian before 1582-10-15).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use icu_calendar::Date as IcuDate;
use icu_calendar::cal::Gregorian;
use icu_datetime::fieldsets::enums::CompositeFieldSet;
use icu_datetime::input::{DateTime, Time, TimeZone as IcuZone, UtcOffset, ZonedDateTime};
use icu_datetime::pattern::{DateTimePattern, FixedCalendarDateTimeNames};
use icu_decimal::DecimalFormatter;
use icu_decimal::input::Decimal;
use icu_decimal::options::{DecimalFormatterOptions, GroupingStrategy};
use writeable::{TryWriteable, Writeable};

use super::java_util::{
    Calendar, Date, DateTimeFields, JavaLocale, TimeZone, cleared_calendar, current_time_millis, date_time_fields,
    zone_short_name,
};
use super::panic_kt;

/// Java's pattern letters.
const PATTERN_LETTERS: &str = "GyYMLwWDdFEuaHkKhmsSzZX";

/// A pattern's part: a field (its letter, how many times) or literal text.
#[derive(Clone, Debug, PartialEq)]
enum Item {
    Field(char, usize),
    Literal(String),
}

/// A date format of Java's pattern letters (Java's `java.text.SimpleDateFormat`): it writes a [`Date`] in its
/// zone (the default one until told) and locale, and reads one back, leniently unless told. A reference, as Java's
/// object: clones are the same format.
#[derive(Clone)]
pub struct SimpleDateFormat(Rc<RefCell<FormatState>>);

struct FormatState {
    pattern: String,
    items: Vec<Item>,
    locale: JavaLocale,
    data: Rc<LocaleData>,
    zone: TimeZone,
    lenient: bool,
    /// The year two-digit years are read from (Java's default century: 80 years before the format was made).
    century_start: i32,
}

impl SimpleDateFormat {
    /// A format of `pattern` in the default locale (Java's `SimpleDateFormat(pattern)`); panics with Java's
    /// `IllegalArgumentException` for a letter that isn't a pattern letter or an open quote.
    ///
    /// Kotlin: `SimpleDateFormat(pattern: String)`
    #[track_caller]
    pub fn new(pattern: &str) -> SimpleDateFormat {
        SimpleDateFormat::with_locale(pattern, JavaLocale::get_default())
    }

    /// A format of `pattern` in `locale` (Java's `SimpleDateFormat(pattern, locale)`).
    ///
    /// Kotlin: `SimpleDateFormat(pattern: String, locale: Locale)`
    #[track_caller]
    pub fn with_locale(pattern: &str, locale: JavaLocale) -> SimpleDateFormat {
        let items = compile(pattern);
        let zone = TimeZone::get_default();
        let this_year = date_time_fields(current_time_millis(), &zone, &locale);
        SimpleDateFormat(Rc::new(RefCell::new(FormatState {
            pattern: pattern.to_string(),
            items,
            data: Rc::new(LocaleData::new(&locale)),
            locale,
            zone,
            lenient: true,
            century_start: this_year.year * if this_year.era == 0 { -1 } else { 1 } - 80,
        })))
    }

    /// `date` written by the pattern (Java's `format`).
    pub fn format(&self, date: Date) -> String {
        let s = self.0.borrow();
        let fields = date_time_fields(date.time(), &s.zone, &s.locale);
        let mut out = String::new();
        for item in &s.items {
            match item {
                Item::Literal(text) => out.push_str(text),
                Item::Field(letter, count) => s.write_field(&mut out, *letter, *count, &fields),
            }
        }
        out
    }

    /// The date `source` writes by the pattern (Java's `parse(String)`): from its start, what follows ignored;
    /// panics with Java's `ParseException` (`Unparseable date: "…"`) if it doesn't, or if its fields don't make a
    /// date and the format isn't lenient.
    #[track_caller]
    pub fn parse(&self, source: &str) -> Date {
        match self.0.borrow().parse(source) {
            Some(date) => date,
            None => panic_kt("ParseException", &format!("Unparseable date: \"{source}\"")),
        }
    }

    /// The pattern (Java's `toPattern`).
    pub fn to_pattern(&self) -> String {
        self.0.borrow().pattern.clone()
    }

    /// Makes `pattern` the format's (Java's `applyPattern`).
    #[track_caller]
    pub fn apply_pattern(&self, pattern: &str) {
        let items = compile(pattern);
        let mut s = self.0.borrow_mut();
        s.items = items;
        s.pattern = pattern.to_string();
    }

    /// The zone dates are written and read in (Java's `getTimeZone`, Kotlin's `timeZone`).
    pub fn time_zone(&self) -> TimeZone {
        self.0.borrow().zone.clone()
    }

    /// Kotlin's `format.timeZone = zone`.
    pub fn set_time_zone(&self, zone: TimeZone) {
        self.0.borrow_mut().zone = zone;
    }

    /// Whether reading takes fields out of range, carried over (Java's `isLenient`, Kotlin's `isLenient`).
    pub fn is_lenient(&self) -> bool {
        self.0.borrow().lenient
    }

    /// Kotlin's `format.isLenient = lenient`.
    pub fn set_is_lenient(&self, lenient: bool) {
        self.0.borrow_mut().lenient = lenient;
    }
}

impl PartialEq for SimpleDateFormat {
    /// Java's `equals`: the same pattern, locale and leniency.
    fn eq(&self, other: &SimpleDateFormat) -> bool {
        let (a, b) = (self.0.borrow(), other.0.borrow());
        a.pattern == b.pattern && a.locale == b.locale && a.lenient == b.lenient
    }
}

impl std::fmt::Debug for SimpleDateFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.0.borrow();
        write!(f, "SimpleDateFormat({:?}, {})", s.pattern, s.locale)
    }
}

/// A pattern's items (Java's `SimpleDateFormat.compile`): letters in runs, `'…'` quoted text, `''` a quote.
#[track_caller]
fn compile(pattern: &str) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    let literal = |items: &mut Vec<Item>, c: char| match items.last_mut() {
        Some(Item::Literal(text)) => text.push(c),
        _ => items.push(Item::Literal(c.to_string())),
    };
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    let mut quoted = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            if chars.get(i + 1) == Some(&'\'') {
                literal(&mut items, '\'');
                i += 2;
                continue;
            }
            quoted = !quoted;
            i += 1;
            continue;
        }
        if quoted || !c.is_ascii_alphabetic() {
            literal(&mut items, c);
            i += 1;
            continue;
        }
        if !PATTERN_LETTERS.contains(c) {
            panic_kt("IllegalArgumentException", &format!("Illegal pattern character '{c}'"));
        }
        let mut count = 1;
        while chars.get(i + count) == Some(&c) {
            count += 1;
        }
        if c == 'X' && count > 3 {
            panic_kt("IllegalArgumentException", "invalid ISO 8601 format: length=4");
        }
        items.push(Item::Field(c, count));
        i += count;
    }
    if quoted {
        panic_kt("IllegalArgumentException", "Unterminated quote");
    }
    items
}

/// Whether a field is written as a number when it is `count` letters long (what abuts another number when read).
fn is_numeric(letter: char, count: usize) -> bool {
    match letter {
        'M' | 'L' => count <= 2,
        'G' | 'E' | 'a' | 'z' | 'Z' | 'X' => false,
        _ => true,
    }
}

impl FormatState {
    fn write_field(&self, out: &mut String, letter: char, count: usize, f: &DateTimeFields) {
        let number = |out: &mut String, value: i32, min: usize| out.push_str(&self.data.digits(value as i64, min));
        let year_text = |out: &mut String, year: i32| {
            if count == 2 { number(out, year.rem_euclid(100), 2) } else { number(out, year, count) }
        };
        match letter {
            'G' => out.push_str(&self.data.era(f.era, count >= 4)),
            'y' => year_text(out, f.year),
            'Y' => year_text(out, f.week_year),
            'M' | 'L' if count >= 3 => out.push_str(&self.data.month(f.month, letter, count >= 4)),
            'M' | 'L' => number(out, f.month + 1, count),
            'd' => number(out, f.day_of_month, count),
            'D' => number(out, f.day_of_year, count),
            'F' => number(out, f.day_of_week_in_month, count),
            'w' => number(out, f.week_of_year, count),
            'W' => number(out, f.week_of_month, count),
            'E' => out.push_str(&self.data.weekday(f.day_of_week, count >= 4)),
            'u' => number(out, if f.day_of_week == 1 { 7 } else { f.day_of_week - 1 }, count),
            'a' => out.push_str(&self.data.am_pm(f.hour_of_day >= 12)),
            'H' => number(out, f.hour_of_day, count),
            'k' => number(out, if f.hour_of_day == 0 { 24 } else { f.hour_of_day }, count),
            'K' => number(out, f.hour_of_day % 12, count),
            'h' => number(out, if f.hour_of_day % 12 == 0 { 12 } else { f.hour_of_day % 12 }, count),
            'm' => number(out, f.minute, count),
            's' => number(out, f.second, count),
            'S' => number(out, f.millisecond, count),
            'z' => out.push_str(&self.zone_name(f, count >= 4)),
            'Z' => out.push_str(&offset_text(f.offset, 2, false)),
            'X' => out.push_str(&offset_text(f.offset, count, true)),
            _ => {}
        }
    }

    /// The zone's specific name at the instant of `f` (Java's `z`, `zzzz`): CLDR's through ICU4X (`Central European
    /// Standard Time`), else its abbreviation (`CET`) or Java's `GMT+03:00`; a custom zone's ID.
    fn zone_name(&self, f: &DateTimeFields, long: bool) -> String {
        if self.zone.is_custom() {
            return self.zone.id();
        }
        self.data
            .zone(&self.zone.id(), f, long)
            .unwrap_or_else(|| zone_short_name(&self.zone, &f.abbreviation, f.offset))
    }

    /// The date `source` writes, none if it doesn't (Java's `SimpleDateFormat.parse` and its `CalendarBuilder`).
    fn parse(&self, source: &str) -> Option<Date> {
        let text: Vec<char> = source.chars().collect();
        let mut at = 0usize;
        // (field, value) in the order read
        let mut set: Vec<(i32, i32)> = Vec::new();
        for (index, item) in self.items.iter().enumerate() {
            match item {
                Item::Literal(literal) => {
                    for c in literal.chars() {
                        if c.is_whitespace() && self.lenient {
                            while text.get(at).is_some_and(|t| t.is_whitespace()) {
                                at += 1;
                            }
                        } else if text.get(at) == Some(&c) {
                            at += 1;
                        } else {
                            return None;
                        }
                    }
                }
                &Item::Field(letter, count) => {
                    // (Java: spaces before a field are skipped)
                    while text.get(at).is_some_and(|&t| t == ' ' || t == '\t') {
                        at += 1;
                    }
                    let abutting = matches!(self.items.get(index + 1), Some(&Item::Field(l, c)) if is_numeric(l, c));
                    at = self.parse_field(&text, at, letter, count, abutting, &mut set)?;
                }
            }
        }
        let calendar: Calendar = cleared_calendar(self.zone.clone(), &self.locale, self.lenient);
        for (field, value) in set {
            calendar.set(field, value);
        }
        calendar.try_time_in_millis().ok().map(Date::of_millis)
    }

    /// Reads a field at `at`, its value(s) into `set`: where it ends, none if it isn't there.
    fn parse_field(
        &self,
        text: &[char],
        at: usize,
        letter: char,
        count: usize,
        abutting: bool,
        set: &mut Vec<(i32, i32)>,
    ) -> Option<usize> {
        use super::java_util::Calendar as C;
        let field = |f: super::java_util::CalendarField| f.0;
        let names = |candidates: Vec<(String, i32)>| -> Option<(usize, i32)> {
            // The longest name there, as Java's `matchString` (case ignored).
            candidates
                .into_iter()
                .filter(|(name, _)| !name.is_empty() && starts_with_ignoring_case(&text[at..], name))
                .max_by_key(|(name, _)| name.chars().count())
                .map(|(name, value)| (at + name.chars().count(), value))
        };
        if !is_numeric(letter, count) {
            let (end, value) = match letter {
                'G' => {
                    names((0..2).flat_map(|e| [(self.data.era(e, true), e), (self.data.era(e, false), e)]).collect())?
                }
                'M' | 'L' => names(
                    (0..12)
                        .flat_map(|m| [true, false].into_iter().flat_map(move |long| ['M', 'L'].map(|l| (l, long, m))))
                        .map(|(l, long, m)| (self.data.month(m, l, long), m))
                        .collect(),
                )?,
                'E' => names(
                    (1..=7).flat_map(|d| [(self.data.weekday(d, true), d), (self.data.weekday(d, false), d)]).collect(),
                )?,
                'a' => names(vec![(self.data.am_pm(false), 0), (self.data.am_pm(true), 1)])?,
                _ => {
                    let (end, offset) = self.parse_zone(text, at)?;
                    set.push((field(C::ZONE_OFFSET), offset));
                    set.push((field(C::DST_OFFSET), 0));
                    return Some(end);
                }
            };
            let target = match letter {
                'G' => C::ERA,
                'M' | 'L' => C::MONTH,
                'E' => C::DAY_OF_WEEK,
                _ => C::AM_PM,
            };
            set.push((field(target), value));
            return Some(end);
        }
        let limit = if abutting { count } else { usize::MAX };
        let (end, value) = self.data.parse_number(text, at, limit)?;
        let value = i32::try_from(value).ok()?;
        let target = match letter {
            'y' | 'Y' => {
                // Two digits of a short pattern's year: within the century from 80 years ago.
                let two_digits = count <= 2 && end - at == 2 && text[at..end].iter().all(|c| c.is_ascii_digit());
                let year = if two_digits {
                    let start = self.century_start;
                    let ambiguous = start.rem_euclid(100);
                    value + start.div_euclid(100) * 100 + if value < ambiguous { 100 } else { 0 }
                } else {
                    value
                };
                set.push((field(C::YEAR), year));
                return Some(end);
            }
            'M' | 'L' => {
                set.push((field(C::MONTH), value - 1));
                return Some(end);
            }
            'h' | 'k' => {
                let wraps = if letter == 'h' { 12 } else { 24 };
                let value = if value == wraps { 0 } else { value };
                set.push((field(if letter == 'h' { C::HOUR } else { C::HOUR_OF_DAY }), value));
                return Some(end);
            }
            'u' => {
                set.push((field(C::DAY_OF_WEEK), value % 7 + 1));
                return Some(end);
            }
            'd' => C::DAY_OF_MONTH,
            'D' => C::DAY_OF_YEAR,
            'F' => C::DAY_OF_WEEK_IN_MONTH,
            'w' => C::WEEK_OF_YEAR,
            'W' => C::WEEK_OF_MONTH,
            'H' => C::HOUR_OF_DAY,
            'K' => C::HOUR,
            'm' => C::MINUTE,
            's' => C::SECOND,
            _ => C::MILLISECOND,
        };
        set.push((field(target), value));
        Some(end)
    }

    /// A zone written at `at`: `GMT`/`UTC` and an offset or none, `Z`, `+hh`, `+hhmm`, `+hh:mm`, or the format's
    /// zone's name; its offset (milliseconds) and where it ends.
    fn parse_zone(&self, text: &[char], at: usize) -> Option<(usize, i32)> {
        let rest = &text[at..];
        for prefix in ["GMT", "UTC"] {
            if starts_with_ignoring_case(rest, prefix) {
                let after = at + 3;
                return Some(parse_offset(text, after).unwrap_or((after, 0)));
            }
        }
        if rest.first() == Some(&'Z') {
            return Some((at + 1, 0));
        }
        if let Some(found) = parse_offset(text, at) {
            return Some(found);
        }
        let now = current_time_millis();
        let fields = date_time_fields(now, &self.zone, &self.locale);
        [true, false]
            .into_iter()
            .map(|long| self.zone_name(&fields, long))
            .filter(|name| starts_with_ignoring_case(rest, name))
            .max_by_key(|name| name.chars().count())
            .map(|name| (at + name.chars().count(), self.zone.get_offset(now)))
    }
}

/// `+hh`, `+hhmm` or `+hh:mm` at `at`: (end, offset in milliseconds).
fn parse_offset(text: &[char], at: usize) -> Option<(usize, i32)> {
    let sign = match text.get(at)? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let digit = |i: usize| text.get(i).and_then(|c| c.to_digit(10)).map(|d| d as i32);
    let mut i = at + 1;
    let mut hours = digit(i)?;
    i += 1;
    if let Some(d) = digit(i) {
        hours = hours * 10 + d;
        i += 1;
    }
    let colon = text.get(i) == Some(&':');
    let m = if colon { i + 1 } else { i };
    let minutes = match (digit(m), digit(m + 1)) {
        (Some(a), Some(b)) => {
            i = m + 2;
            a * 10 + b
        }
        _ if colon => return None,
        _ => 0,
    };
    (hours <= 23 && minutes <= 59).then_some((i, sign * (hours * 60 + minutes) * 60_000))
}

/// Java's RFC 822 (`Z`: `+0300`) and ISO 8601 (`X`: `+03`, `+0300`, `+03:00`; `Z` for UTC) offsets, ASCII digits.
fn offset_text(offset: i32, count: usize, iso: bool) -> String {
    if iso && offset == 0 {
        return "Z".to_string();
    }
    let minutes = offset.abs() / 60_000;
    let sign = if offset < 0 { '-' } else { '+' };
    match (iso, count) {
        (true, 1) => format!("{sign}{:02}", minutes / 60),
        (true, 3) => format!("{sign}{:02}:{:02}", minutes / 60, minutes % 60),
        _ => format!("{sign}{:02}{:02}", minutes / 60, minutes % 60),
    }
}

fn starts_with_ignoring_case(text: &[char], prefix: &str) -> bool {
    let mut t = text.iter();
    prefix.chars().all(|p| t.next().is_some_and(|c| c.to_lowercase().eq(p.to_lowercase())))
}

/// A locale's names and digits, CLDR's through ICU4X, loaded as they are asked for.
struct LocaleData {
    locale: icu_locale_core::Locale,
    /// One-field patterns and the names they need (ICU4X holds one length of each kind of name per object).
    patterns: RefCell<
        HashMap<String, Option<Rc<(FixedCalendarDateTimeNames<Gregorian, CompositeFieldSet>, DateTimePattern)>>>,
    >,
    decimal: Option<DecimalFormatter>,
    /// The locale's zero digit (`0`, `٠`): its digits are read too.
    zero: char,
}

impl LocaleData {
    fn new(locale: &JavaLocale) -> LocaleData {
        let icu = locale.icu_locale();
        let mut options = DecimalFormatterOptions::default();
        options.grouping_strategy = Some(GroupingStrategy::Never);
        let decimal = DecimalFormatter::try_new((&icu).into(), options).ok();
        let zero =
            decimal.as_ref().and_then(|d| d.format(&Decimal::from(0)).write_to_string().chars().next()).unwrap_or('0');
        LocaleData { locale: icu, patterns: RefCell::new(HashMap::new()), decimal, zero }
    }

    /// `value` in the locale's digits, at least `min` of them (Java's `zeroPaddingNumber`, no grouping).
    fn digits(&self, value: i64, min: usize) -> String {
        let mut decimal = Decimal::from(value);
        decimal.absolute.pad_start(min.min(i16::MAX as usize) as i16);
        match &self.decimal {
            Some(f) => f.format(&decimal).write_to_string().into_owned(),
            None => decimal.to_string(),
        }
    }

    /// A number at `at` (a `-` sign, then ASCII or the locale's digits, at most `limit` of them): (end, value).
    fn parse_number(&self, text: &[char], at: usize, limit: usize) -> Option<(usize, i64)> {
        let mut i = at;
        let negative = text.get(i) == Some(&'-');
        i += negative as usize;
        let start = i;
        let mut value: i64 = 0;
        while i - start < limit {
            let Some(&c) = text.get(i) else { break };
            let digit = c.to_digit(10).or_else(|| {
                let d = (c as u32).wrapping_sub(self.zero as u32);
                (d < 10).then_some(d)
            });
            let Some(digit) = digit else { break };
            value = value.checked_mul(10)?.checked_add(digit as i64)?;
            i += 1;
        }
        (i > start).then_some((i, if negative { -value } else { value }))
    }

    /// The text of a one-field CLDR `pattern` for a date, an hour and, for a zone's name, a zone.
    fn text(&self, pattern: &str, date: (i32, u8, u8), hour: u8, zone: Option<(&str, i32)>) -> Option<String> {
        let entry = self
            .patterns
            .borrow_mut()
            .entry(pattern.to_string())
            .or_insert_with(|| {
                let parsed = DateTimePattern::try_from_pattern_str(pattern).ok()?;
                let mut names =
                    FixedCalendarDateTimeNames::<Gregorian, CompositeFieldSet>::try_new((&self.locale).into()).ok()?;
                names.include_for_pattern(&parsed).ok()?;
                Some(Rc::new((names, parsed)))
            })
            .clone()?;
        let (names, parsed) = &*entry;
        let date = IcuDate::try_new_gregorian(date.0, date.1, date.2).ok()?;
        let time = Time::try_new(hour, 0, 0, 0).ok()?;
        let info = match zone {
            Some((id, offset)) => IcuZone::from_iana_id(id).with_offset(UtcOffset::try_from_seconds(offset).ok()),
            None => IcuZone::UNKNOWN.with_offset(Some(UtcOffset::zero())),
        }
        .at_date_time(DateTime { date, time });
        let input = ZonedDateTime { date, time, zone: info };
        let formatted = names.with_pattern_unchecked(parsed).format(&input);
        Some(match formatted.try_write_to_string() {
            Ok(s) => s.into_owned(),
            Err((_, s)) => s.into_owned(),
        })
    }

    /// A month's name, 0 January (Java's `MMM`/`MMMM` format names, `LLL`/`LLLL` standalone ones).
    fn month(&self, month: i32, letter: char, long: bool) -> String {
        let pattern = format!("{letter}{letter}{letter}{}", if long { letter.to_string() } else { String::new() });
        self.text(&pattern, (2000, (month.rem_euclid(12) + 1) as u8, 1), 0, None).unwrap_or_default()
    }

    /// A weekday's name, 1 Sunday (Java's `E`: short, `EEEE`: full).
    fn weekday(&self, day_of_week: i32, long: bool) -> String {
        // (2000-01-02 was a Sunday)
        let day = (2 + (day_of_week - 1).rem_euclid(7)) as u8;
        self.text(if long { "EEEE" } else { "EEE" }, (2000, 1, day), 0, None).unwrap_or_default()
    }

    /// AM's or PM's marker (Java's `a`: the abbreviated one).
    fn am_pm(&self, pm: bool) -> String {
        self.text("a", (2000, 1, 1), if pm { 13 } else { 1 }, None).unwrap_or_default()
    }

    /// An era's name, 0 BC (Java's `G`: short, `GGGG`: long).
    fn era(&self, era: i32, long: bool) -> String {
        let year = if era == 0 { 0 } else { 2000 };
        self.text(if long { "GGGG" } else { "G" }, (year, 1, 1), 0, None).unwrap_or_default()
    }

    /// A zone's specific name at an instant whose fields `f` are (Java's `z`, `zzzz`); none where CLDR has no name
    /// for it but the localized GMT format (`GMT+1`: the JDK then names it otherwise).
    fn zone(&self, id: &str, f: &DateTimeFields, long: bool) -> Option<String> {
        let year = if f.era == 0 { 1 - f.year } else { f.year };
        let date = (year, (f.month + 1) as u8, f.day_of_month as u8);
        let pattern = if long { "zzzz" } else { "z" };
        let name = self.text(pattern, date, f.hour_of_day as u8, Some((id, f.offset / 1000)))?;
        let unnamed = self.text(pattern, date, f.hour_of_day as u8, Some(("Etc/Unknown", f.offset / 1000)))?;
        (name != unnamed).then_some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kt::zoneinfo::days_from_civil;

    fn at(y: i32, m: i32, d: i32, h: i64, min: i64, s: i64) -> Date {
        Date::of_millis(days_from_civil(y, m, d) * 86_400_000 + ((h * 60 + min) * 60 + s) * 1000)
    }

    fn utc_format(pattern: &str, locale: JavaLocale) -> SimpleDateFormat {
        let f = SimpleDateFormat::with_locale(pattern, locale);
        f.set_time_zone(TimeZone::get_time_zone("UTC"));
        f
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn times_are_written_in_the_locale() {
        let pm = at(2023, 4, 15, 15, 30, 0);
        assert_eq!(utc_format("hh:mm a", JavaLocale::US).format(pm), "03:30 PM");
        assert_eq!(utc_format("hh:mm a", JavaLocale::US).format(at(2023, 4, 15, 0, 5, 0)), "12:05 AM");
        assert_eq!(utc_format("hh:mm a", JavaLocale::new("tr")).format(pm), "03:30 ÖS");
        let he = utc_format("hh:mm a", JavaLocale::new("he")).format(pm);
        assert!(he.starts_with("03:30 "), "{he}");
        assert_eq!(utc_format("HH:mm:ss.SSS", JavaLocale::US).format(at(2023, 4, 15, 9, 5, 7)), "09:05:07.000");
        assert_eq!(utc_format("k K h H", JavaLocale::US).format(at(2023, 4, 15, 0, 0, 0)), "24 0 12 0");
        assert_eq!(utc_format("hh:mm a", JavaLocale::new("ar-EG")).format(pm).chars().next(), Some('٠'));
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn dates_are_written_in_the_locale() {
        let day = at(2023, 4, 15, 10, 15, 30);
        let us = utc_format("EEE, d MMM yyyy HH:mm:ss Z", JavaLocale::US);
        assert_eq!(us.format(day), "Sat, 15 Apr 2023 10:15:30 +0000");
        assert_eq!(utc_format("EEEE d MMMM yy G", JavaLocale::US).format(day), "Saturday 15 April 23 AD");
        assert_eq!(utc_format("d MMMM EEEE", JavaLocale::new("tr")).format(day), "15 Nisan Cumartesi");
        assert_eq!(utc_format("D w W F u", JavaLocale::US).format(day), "105 15 3 3 6");
        assert_eq!(utc_format("''yyyy'-'MM'' 'at' X XX XXX", JavaLocale::US).format(day), "'2023-04' at Z Z Z");
        let plus3 = SimpleDateFormat::with_locale("HH:mm z X XXX", JavaLocale::US);
        plus3.set_time_zone(TimeZone::get_time_zone("GMT+03:00"));
        assert_eq!(plus3.format(day), "13:15 GMT+03:00 +03 +03:00");
        // (Julian before the cutover, as Java's calendar)
        assert_eq!(utc_format("yyyy-MM-dd", JavaLocale::US).format(at(1582, 10, 14, 0, 0, 0)), "1582-10-04");
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn zones_are_named_as_cldr_names_them() {
        let berlin = TimeZone::get_time_zone("Europe/Berlin");
        if berlin.id() != "Europe/Berlin" {
            return; // (no zoneinfo database here)
        }
        let f = SimpleDateFormat::with_locale("z|zzzz", JavaLocale::US);
        f.set_time_zone(berlin);
        // (Windows' zones have no abbreviations: the short names are their offsets, as Java's for such zones.)
        let (cet, cest) = if cfg!(windows) { ("GMT+01:00", "GMT+02:00") } else { ("CET", "CEST") };
        assert_eq!(f.format(at(2023, 1, 15, 12, 0, 0)), format!("{cet}|Central European Standard Time"));
        assert_eq!(f.format(at(2023, 7, 15, 12, 0, 0)), format!("{cest}|Central European Summer Time"));
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn dates_are_read_back() {
        let us = utc_format("hh:mm a", JavaLocale::US);
        assert_eq!(us.parse("03:30 PM"), at(1970, 1, 1, 15, 30, 0));
        assert_eq!(us.parse("12:05 am and more"), at(1970, 1, 1, 0, 5, 0));
        let full = utc_format("EEE, d MMM yyyy HH:mm:ss Z", JavaLocale::US);
        assert_eq!(full.parse("Sat, 15 Apr 2023 13:15:30 +0300"), at(2023, 4, 15, 10, 15, 30));
        assert_eq!(utc_format("ddMMyyyy", JavaLocale::US).parse("15042023"), at(2023, 4, 15, 0, 0, 0));
        assert_eq!(utc_format("d MMMM yyyy", JavaLocale::new("tr")).parse("15 nisan 2023"), at(2023, 4, 15, 0, 0, 0));
        let dots = utc_format("dd.MM.yyyy", JavaLocale::US);
        assert_eq!(dots.parse("31.02.2023"), at(2023, 3, 3, 0, 0, 0));
        dots.set_is_lenient(false);
        let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dots.parse("31.02.2023"))).unwrap_err();
        assert_eq!(refused.downcast_ref::<String>().unwrap(), "ParseException: Unparseable date: \"31.02.2023\"");
        let short = utc_format("dd/MM/yy", JavaLocale::US);
        let year = date_time_fields(short.parse("01/01/15").time(), &short.time_zone(), &JavaLocale::US).year;
        assert_eq!(year, 2015);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| us.parse("3:30"))).is_err());
    }

    /// TimePickerSamples' confirm button in Rust's syntax: today at the time picked, written by
    /// `SimpleDateFormat("hh:mm a", locale)` (in one zone given: other tests change the default one meanwhile).
    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn the_samples_time_written_in_rust() {
        let formatter = utc_format("hh:mm a", JavaLocale::US);
        let cal = Calendar::get_instance_in(TimeZone::get_time_zone("UTC"));
        cal.set(Calendar::HOUR_OF_DAY, 15);
        cal.set(Calendar::MINUTE, 30);
        cal.set_is_lenient(false);
        assert_eq!(formatter.format(cal.time()), "03:30 PM");
        assert_eq!(cal.get(Calendar::AM_PM), Calendar::PM);
        // (the same calendar through its clones, as Kotlin's references)
        let same = cal.clone();
        same.set(Calendar::HOUR_OF_DAY, 7);
        assert_eq!(formatter.format(cal.time()), "07:30 AM");
        let other = cal.copy();
        other.add(Calendar::MINUTE, 15);
        assert_eq!(
            (formatter.format(cal.time()), formatter.format(other.time())),
            ("07:30 AM".into(), "07:45 AM".into())
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn patterns_are_checked() {
        let refused =
            std::panic::catch_unwind(|| SimpleDateFormat::with_locale("hh:mm b", JavaLocale::US)).unwrap_err();
        assert_eq!(
            refused.downcast_ref::<String>().unwrap(),
            "IllegalArgumentException: Illegal pattern character 'b'"
        );
        assert!(std::panic::catch_unwind(|| SimpleDateFormat::with_locale("'open", JavaLocale::US)).is_err());
        let f = SimpleDateFormat::with_locale("yyyy", JavaLocale::US);
        f.apply_pattern("MM");
        assert_eq!(f.to_pattern(), "MM");
    }
}

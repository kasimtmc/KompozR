//! The `java.util` classes Kotlin/JVM code writes dates and times with ([`Calendar`], [`Date`], [`TimeZone`],
//! [`JavaLocale`]): what Compose's samples use (TimePickerSamples' `Calendar.getInstance()` and
//! `SimpleDateFormat("hh:mm a", Locale.getDefault())`). Java's behaviour: a calendar's fields set one by one and
//! resolved when read (`GregorianCalendar`'s rules: the last set wins, out-of-range values carried over unless
//! the calendar isn't lenient), Julian dates before the Gregorian cutover of 1582-10-15, the system's clock, time
//! zone (the system's zoneinfo database, as the JDK's own tzdb) and locale (CLDR's week data through ICU4X, as the
//! JDK's CLDR locale data). Java's exceptions are panics with their messages (`IllegalArgumentException: MINUTE`).
//!
//! `java.util.Locale` is [`JavaLocale`] here: Kotlin's `Locale` is Compose's text locale unless the file imports
//! `java.util.Locale`, as in Kotlin (the Kotlin syntax reads the imports).

use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use icu_locale_core::Locale as IcuLocale;

use super::panic_kt;
use super::zoneinfo::{self, ZoneRules, civil_from_days, days_from_civil};

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 3_600_000;
const DAY_MS: i64 = 86_400_000;

/// The milliseconds since 1970-01-01T00:00Z now (Java's `System.currentTimeMillis`).
pub(crate) fn current_time_millis() -> i64 {
    crate::clock::current_time_millis()
}

// ==== Locale =========================================================================================================

/// A locale (Java's `java.util.Locale`): a language, a script, a country and a variant, as Java keeps them
/// (`Locale("he")`'s language `he`, `Locale("en", "us")`'s country `US`). Kotlin's `Locale` in a file importing
/// `java.util.Locale`.
///
/// Kotlin: `java.util.Locale`
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct JavaLocale {
    language: Cow<'static, str>,
    script: Cow<'static, str>,
    country: Cow<'static, str>,
    variant: Cow<'static, str>,
}

static DEFAULT_LOCALE: RwLock<Option<JavaLocale>> = RwLock::new(None);

impl JavaLocale {
    const fn constant(language: &'static str, country: &'static str) -> JavaLocale {
        JavaLocale {
            language: Cow::Borrowed(language),
            script: Cow::Borrowed(""),
            country: Cow::Borrowed(country),
            variant: Cow::Borrowed(""),
        }
    }

    /// Java's `Locale.ROOT`: no language, no country.
    pub const ROOT: JavaLocale = JavaLocale::constant("", "");
    pub const ENGLISH: JavaLocale = JavaLocale::constant("en", "");
    pub const FRENCH: JavaLocale = JavaLocale::constant("fr", "");
    pub const GERMAN: JavaLocale = JavaLocale::constant("de", "");
    pub const ITALIAN: JavaLocale = JavaLocale::constant("it", "");
    pub const JAPANESE: JavaLocale = JavaLocale::constant("ja", "");
    pub const KOREAN: JavaLocale = JavaLocale::constant("ko", "");
    pub const CHINESE: JavaLocale = JavaLocale::constant("zh", "");
    pub const SIMPLIFIED_CHINESE: JavaLocale = JavaLocale::constant("zh", "CN");
    pub const TRADITIONAL_CHINESE: JavaLocale = JavaLocale::constant("zh", "TW");
    pub const FRANCE: JavaLocale = JavaLocale::constant("fr", "FR");
    pub const GERMANY: JavaLocale = JavaLocale::constant("de", "DE");
    pub const ITALY: JavaLocale = JavaLocale::constant("it", "IT");
    pub const JAPAN: JavaLocale = JavaLocale::constant("ja", "JP");
    pub const KOREA: JavaLocale = JavaLocale::constant("ko", "KR");
    pub const UK: JavaLocale = JavaLocale::constant("en", "GB");
    pub const US: JavaLocale = JavaLocale::constant("en", "US");
    pub const CANADA: JavaLocale = JavaLocale::constant("en", "CA");
    pub const CANADA_FRENCH: JavaLocale = JavaLocale::constant("fr", "CA");
    pub const CHINA: JavaLocale = JavaLocale::constant("zh", "CN");
    pub const PRC: JavaLocale = JavaLocale::constant("zh", "CN");
    pub const TAIWAN: JavaLocale = JavaLocale::constant("zh", "TW");

    /// The locale of a language (Java's `Locale(language)`: lower-cased, kept as given otherwise).
    ///
    /// Kotlin: `JavaLocale(language: String)`
    pub fn new(language: &str) -> JavaLocale {
        JavaLocale::with_variant(language, "", "")
    }

    /// The locale of a language in a country (Java's `Locale(language, country)`: the country upper-cased).
    ///
    /// Kotlin: `JavaLocale(language: String, country: String)`
    pub fn with_country(language: &str, country: &str) -> JavaLocale {
        JavaLocale::with_variant(language, country, "")
    }

    /// Java's `Locale(language, country, variant)`.
    ///
    /// Kotlin: `JavaLocale(language: String, country: String, variant: String)`
    pub fn with_variant(language: &str, country: &str, variant: &str) -> JavaLocale {
        JavaLocale {
            language: Cow::Owned(language.to_ascii_lowercase()),
            script: Cow::Borrowed(""),
            country: Cow::Owned(country.to_ascii_uppercase()),
            variant: Cow::Owned(variant.to_string()),
        }
    }

    /// The locale of a BCP 47 language tag (Java's `Locale.forLanguageTag`): `und` and one that isn't well formed
    /// are the root locale.
    ///
    /// Kotlin: `JavaLocale.forLanguageTag(languageTag: String)`
    pub fn for_language_tag(language_tag: &str) -> JavaLocale {
        let Ok(locale) = IcuLocale::try_from_str(language_tag) else { return JavaLocale::ROOT };
        let id = &locale.id;
        let language = if id.language.is_unknown() { String::new() } else { id.language.as_str().to_string() };
        let variant = id.variants.iter().map(|v| v.as_str()).collect::<Vec<_>>().join("_");
        JavaLocale {
            language: Cow::Owned(language),
            script: Cow::Owned(id.script.map(|s| s.as_str().to_string()).unwrap_or_default()),
            country: Cow::Owned(id.region.map(|r| r.as_str().to_string()).unwrap_or_default()),
            variant: Cow::Owned(variant),
        }
    }

    /// The default locale (Java's `Locale.getDefault()`): the one [`set_default`](JavaLocale::set_default) set,
    /// else the system's — `KOMPOZ_LANGUAGE` (KompozR's language over the system's), else POSIX's `LC_ALL`,
    /// `LC_MESSAGES`, `LANG` (`tr_TR.UTF-8` is `tr_TR`), else the first of the languages the system says the user
    /// prefers ([`system_languages`](crate::environment::system_languages): Windows', Android's, the browser's,
    /// macOS's); English's for none (`C`).
    ///
    /// Kotlin: `JavaLocale.getDefault()`
    pub fn get_default() -> JavaLocale {
        if let Some(set) = DEFAULT_LOCALE.read().unwrap_or_else(|e| e.into_inner()).clone() {
            return set;
        }
        let posix = ["KOMPOZ_LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|v| crate::environment::var(v).filter(|s| !s.is_empty()));
        let locale = match posix {
            Some(name) => JavaLocale::of_posix_name(&name),
            None => match crate::environment::system_languages().first() {
                Some(tag) => JavaLocale::for_language_tag(tag),
                None => JavaLocale::of_posix_name(""),
            },
        };
        *DEFAULT_LOCALE.write().unwrap_or_else(|e| e.into_inner()) = Some(locale.clone());
        locale
    }

    /// Makes `new_locale` the default (Java's `Locale.setDefault`): the process's, as Java's.
    ///
    /// Kotlin: `JavaLocale.setDefault(newLocale: Locale)`
    pub fn set_default(new_locale: JavaLocale) {
        *DEFAULT_LOCALE.write().unwrap_or_else(|e| e.into_inner()) = Some(new_locale);
    }

    /// A POSIX locale name (`pt_BR.UTF-8`, `sr_RS@latin`, `C`), as the JDK reads the system's.
    fn of_posix_name(name: &str) -> JavaLocale {
        let base = name.split(['.', '@']).next().unwrap_or("");
        if base.is_empty() || base == "C" || base.eq_ignore_ascii_case("posix") {
            return JavaLocale::constant("en", "");
        }
        let mut parts = base.split(['_', '-']);
        let language = parts.next().unwrap_or("");
        let country = parts.next().unwrap_or("");
        JavaLocale::with_country(language, country)
    }

    /// The language's code, lower case, empty for none (Java's `getLanguage`).
    pub fn language(&self) -> String {
        self.language.to_string()
    }

    /// The script's code (`Latn`), empty for none (Java's `getScript`).
    pub fn script(&self) -> String {
        self.script.to_string()
    }

    /// The country's code, upper case, empty for none (Java's `getCountry`).
    pub fn country(&self) -> String {
        self.country.to_string()
    }

    /// The variant, empty for none (Java's `getVariant`).
    pub fn variant(&self) -> String {
        self.variant.to_string()
    }

    /// The BCP 47 language tag (Java's `toLanguageTag`: `en-US`, `und` for the root locale).
    pub fn to_language_tag(&self) -> String {
        self.icu_locale().to_string()
    }

    /// ICU4X's locale this is: its CLDR data's (one that isn't well formed is the root locale's).
    pub fn icu_locale(&self) -> IcuLocale {
        let mut tag = if self.language.is_empty() { "und".to_string() } else { self.language.to_string() };
        for part in [&self.script, &self.country] {
            if !part.is_empty() {
                tag.push('-');
                tag.push_str(part);
            }
        }
        let with_variant = (!self.variant.is_empty()).then(|| format!("{tag}-{}", self.variant.replace('_', "-")));
        with_variant
            .and_then(|t| IcuLocale::try_from_str(&t).ok())
            .or_else(|| IcuLocale::try_from_str(&tag).ok())
            .unwrap_or(IcuLocale::UNKNOWN)
    }
}

/// Java's `toString`: `en_US`, `he`, `_US`, `zh_TW_#Hant` (a script after `_#`).
impl fmt::Display for JavaLocale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (l, c, v, s) = (&self.language, &self.country, &self.variant, &self.script);
        if l.is_empty() && c.is_empty() && v.is_empty() {
            return Ok(());
        }
        f.write_str(l)?;
        if !c.is_empty() || !v.is_empty() || !s.is_empty() {
            write!(f, "_{c}")?;
        }
        if !v.is_empty() {
            write!(f, "_{v}")?;
        }
        if !s.is_empty() {
            write!(f, "_#{s}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for JavaLocale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// ==== TimeZone =======================================================================================================

/// A time zone (Java's `java.util.TimeZone`): an IANA zone of the system's database (`Europe/Istanbul`), UTC, or a
/// fixed offset (`GMT+03:00`). Clones are the same zone.
#[derive(Clone)]
pub struct TimeZone {
    id: Arc<str>,
    rules: Arc<ZoneRules>,
    /// A custom ID's zone (`GMT+03:00`): its name is its ID, as Java's.
    custom: bool,
}

static DEFAULT_ZONE: RwLock<Option<TimeZone>> = RwLock::new(None);
/// The system's zone, read once (until a `set_default(None)` reads it again). Kept apart from the set one, so a
/// first read racing a `set_default` on another thread can't overwrite it.
static SYSTEM_ZONE: RwLock<Option<TimeZone>> = RwLock::new(None);

impl TimeZone {
    /// The zone of an ID (Java's `TimeZone.getTimeZone`): an IANA zone (`America/New_York`, `UTC`), or a custom
    /// one, `GMT` and an offset (`GMT+3`, `GMT-08:00`, `GMT+0530`, its ID then normalized: `GMT+03:00`); GMT for
    /// one the system doesn't know, as Java's.
    ///
    /// Kotlin: `TimeZone.getTimeZone(ID: String)`
    pub fn get_time_zone(id: &str) -> TimeZone {
        if let Some(offset) = custom_offset(id) {
            return TimeZone::of_offset_id(offset);
        }
        if let Some(rules) = zoneinfo::load(id) {
            return TimeZone { id: id.into(), rules, custom: false };
        }
        if id == "UTC" || id == "GMT" {
            return TimeZone { id: id.into(), rules: Arc::new(ZoneRules::fixed(0, id)), custom: false };
        }
        TimeZone::gmt()
    }

    fn gmt() -> TimeZone {
        TimeZone { id: "GMT".into(), rules: Arc::new(ZoneRules::fixed(0, "GMT")), custom: false }
    }

    /// A fixed zone of `offset` milliseconds, its ID Java's custom one (`GMT+03:00`, `GMT` for none).
    fn of_offset_id(offset: i32) -> TimeZone {
        let id = if offset == 0 { "GMT".to_string() } else { gmt_text(offset) };
        TimeZone { rules: Arc::new(ZoneRules::fixed(offset / 1000, &id)), id: id.into(), custom: offset != 0 }
    }

    /// The default zone (Java's `TimeZone.getDefault()`): the one [`set_default`](TimeZone::set_default) set, else
    /// the system's (`TZ`, the platform's word, else the system's own: `/etc/localtime`, Android's property, Windows'
    /// zone); GMT if the system has none.
    ///
    /// Kotlin: `TimeZone.getDefault()`
    pub fn get_default() -> TimeZone {
        if let Some(set) = DEFAULT_ZONE.read().unwrap_or_else(|e| e.into_inner()).clone() {
            return set;
        }
        if let Some(system) = SYSTEM_ZONE.read().unwrap_or_else(|e| e.into_inner()).clone() {
            return system;
        }
        let zone = match zoneinfo::system_zone() {
            Some((id, rules)) => TimeZone { id: id.into(), rules, custom: false },
            None => TimeZone::gmt(),
        };
        *SYSTEM_ZONE.write().unwrap_or_else(|e| e.into_inner()) = Some(zone.clone());
        zone
    }

    /// Makes `zone` the default (Java's `TimeZone.setDefault`), the system's again for none.
    ///
    /// Kotlin: `TimeZone.setDefault(zone: TimeZone?)`
    pub fn set_default(zone: Option<TimeZone>) {
        if zone.is_none() {
            *SYSTEM_ZONE.write().unwrap_or_else(|e| e.into_inner()) = None;
        }
        *DEFAULT_ZONE.write().unwrap_or_else(|e| e.into_inner()) = zone;
    }

    /// The system's zone is to be read again (the platform said another: [`crate::environment`]).
    pub(crate) fn forget_system_zone() {
        *SYSTEM_ZONE.write().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// The zone's ID (Java's `getID`, Kotlin's `id`).
    pub fn id(&self) -> String {
        self.id.to_string()
    }

    /// The offset from UTC at `date` (milliseconds since the epoch), daylight saving time included (Java's
    /// `getOffset(long)`).
    pub fn get_offset(&self, date: i64) -> i32 {
        self.rules.at(date.div_euclid(1000)).total * 1000
    }

    /// The offset from UTC without daylight saving time (Java's `getRawOffset`, Kotlin's `rawOffset`).
    pub fn raw_offset(&self) -> i32 {
        self.rules.raw_offset() * 1000
    }

    /// How much daylight saving time moves the clock, milliseconds (Java's `getDSTSavings`).
    pub fn dst_savings(&self) -> i32 {
        self.rules.dst_savings() * 1000
    }

    /// Whether the zone has daylight saving time now or later (Java's `useDaylightTime`).
    pub fn use_daylight_time(&self) -> bool {
        self.rules.uses_daylight_time(current_time_millis() / 1000)
    }

    /// Whether `date` is in daylight saving time here (Java's `inDaylightTime`).
    pub fn in_daylight_time(&self, date: Date) -> bool {
        self.rules.at(date.millis.div_euclid(1000)).dst
    }

    /// (total, standard) offsets in milliseconds, and the abbreviation, at `millis`.
    pub(crate) fn offsets_at(&self, millis: i64) -> (i32, i32, String) {
        let o = self.rules.at(millis.div_euclid(1000));
        (o.total * 1000, o.standard * 1000, o.abbreviation)
    }

    /// The offset (milliseconds) of a local time `wall` (milliseconds since the local epoch), as Java resolves one
    /// a transition skips or repeats.
    pub(crate) fn offset_by_wall(&self, wall: i64) -> i32 {
        self.rules.offset_by_wall(wall.div_euclid(1000)) * 1000
    }

    /// Whether this is a custom ID's fixed zone (`GMT+03:00`).
    pub(crate) fn is_custom(&self) -> bool {
        self.custom
    }

    /// The zone of a `ZoneId` of java.time (an IANA region, or a fixed offset in seconds).
    pub(crate) fn of_rules(id: &str, rules: Arc<ZoneRules>) -> TimeZone {
        TimeZone { id: id.into(), rules, custom: false }
    }
}

impl PartialEq for TimeZone {
    fn eq(&self, other: &TimeZone) -> bool {
        self.id == other.id && (Arc::ptr_eq(&self.rules, &other.rules) || self.raw_offset() == other.raw_offset())
    }
}

impl fmt::Debug for TimeZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TimeZone({})", self.id)
    }
}

/// The offset (milliseconds) of a custom zone ID: `GMT` then `+`/`-` and hours, `hh:mm` or `hhmm` (Java's
/// `TimeZone.parseCustomTimeZone`: hours to 23, minutes to 59).
fn custom_offset(id: &str) -> Option<i32> {
    let rest = id.strip_prefix("GMT")?;
    let (sign, rest) = match rest.as_bytes().first()? {
        b'+' => (1, &rest[1..]),
        b'-' => (-1, &rest[1..]),
        _ => return None,
    };
    let digits = |s: &str| !s.is_empty() && s.len() <= 2 && s.bytes().all(|b| b.is_ascii_digit());
    let (hours, minutes) = if let Some((h, m)) = rest.split_once(':') {
        if !digits(h) || m.len() != 2 || !digits(m) {
            return None;
        }
        (h.parse::<i32>().ok()?, m.parse::<i32>().ok()?)
    } else if rest.len() <= 2 && digits(rest) {
        (rest.parse().ok()?, 0)
    } else if (3..=4).contains(&rest.len()) && rest.bytes().all(|b| b.is_ascii_digit()) {
        let split = rest.len() - 2;
        (rest[..split].parse().ok()?, rest[split..].parse().ok()?)
    } else {
        return None;
    };
    (hours <= 23 && minutes <= 59).then(|| sign * (hours * 60 + minutes) * 60_000)
}

/// `GMT+03:00` for an offset in milliseconds (Java's custom zone ID and name).
pub(crate) fn gmt_text(offset: i32) -> String {
    let minutes = offset.abs() / 60_000;
    format!("GMT{}{:02}:{:02}", if offset < 0 { '-' } else { '+' }, minutes / 60, minutes % 60)
}

// ==== Date ===========================================================================================================

/// An instant, milliseconds since 1970-01-01T00:00Z (Java's `java.util.Date`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Date {
    millis: i64,
}

impl Date {
    /// Now (Java's `Date()`).
    ///
    /// Kotlin: `Date()`
    pub fn new() -> Date {
        Date { millis: current_time_millis() }
    }

    /// The instant `date` milliseconds after the epoch (Java's `Date(long)`).
    ///
    /// Kotlin: `Date(date: Long)`
    pub fn of_millis(date: i64) -> Date {
        Date { millis: date }
    }

    /// Its milliseconds since the epoch (Java's `getTime`, Kotlin's `time`).
    pub fn time(&self) -> i64 {
        self.millis
    }

    /// Kotlin's `date.time = millis`.
    pub fn set_time(&mut self, time: i64) {
        self.millis = time;
    }

    /// Whether it is before `when` (Java's `before`).
    pub fn before(&self, when: Date) -> bool {
        self.millis < when.millis
    }

    /// Whether it is after `when` (Java's `after`).
    pub fn after(&self, when: Date) -> bool {
        self.millis > when.millis
    }

    /// −1, 0 or 1 as it is before, at or after `another_date` (Java's `compareTo`).
    pub fn compare_to(&self, another_date: Date) -> i32 {
        self.millis.cmp(&another_date.millis) as i32
    }
}

impl Default for Date {
    fn default() -> Date {
        Date::new()
    }
}

/// Java's `toString`: `Sat Apr 15 10:15:30 CEST 2023`, in the default zone, in English.
impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        let zone = TimeZone::get_default();
        let fields = Fields::of(self.millis, &zone, 1, 1);
        let (_, _, abbreviation) = zone.offsets_at(self.millis);
        let name = zone_short_name(&zone, &abbreviation, fields.get(ZONE_OFFSET) + fields.get(DST_OFFSET));
        write!(
            f,
            "{} {} {:02} {:02}:{:02}:{:02} {name} {}",
            DAYS[(fields.get(DAY_OF_WEEK) - 1) as usize],
            MONTHS[fields.get(MONTH) as usize],
            fields.get(DAY_OF_MONTH),
            fields.get(HOUR_OF_DAY),
            fields.get(MINUTE),
            fields.get(SECOND),
            fields.year_ce(),
        )
    }
}

impl fmt::Debug for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// A zone's short name where CLDR's isn't asked for: a custom zone's ID, an alphabetic abbreviation (`CEST`), else
/// Java's `GMT+03:00`.
pub(crate) fn zone_short_name(zone: &TimeZone, abbreviation: &str, offset: i32) -> String {
    if zone.is_custom() {
        return zone.id();
    }
    if !abbreviation.is_empty() && abbreviation.chars().all(|c| c.is_ascii_alphabetic()) {
        return abbreviation.to_string();
    }
    if offset == 0 { "GMT".to_string() } else { gmt_text(offset) }
}

// ==== Calendar =======================================================================================================

/// A calendar field (Java's `Calendar.YEAR`, `Calendar.HOUR_OF_DAY`, … — `int`s there): what [`Calendar::get`]
/// and [`Calendar::set`] (Kotlin's `cal[field]`, `cal[field] = value`) take. An `Int` converts to one.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct CalendarField(pub i32);

impl From<i32> for CalendarField {
    fn from(field: i32) -> CalendarField {
        CalendarField(field)
    }
}

const FIELD_COUNT: usize = 17;
const ERA: usize = 0;
const YEAR: usize = 1;
const MONTH: usize = 2;
const WEEK_OF_YEAR: usize = 3;
const WEEK_OF_MONTH: usize = 4;
const DAY_OF_MONTH: usize = 5;
const DAY_OF_YEAR: usize = 6;
const DAY_OF_WEEK: usize = 7;
const DAY_OF_WEEK_IN_MONTH: usize = 8;
const AM_PM: usize = 9;
const HOUR: usize = 10;
const HOUR_OF_DAY: usize = 11;
const MINUTE: usize = 12;
const SECOND: usize = 13;
const MILLISECOND: usize = 14;
const ZONE_OFFSET: usize = 15;
const DST_OFFSET: usize = 16;

const FIELD_NAMES: [&str; FIELD_COUNT] = [
    "ERA",
    "YEAR",
    "MONTH",
    "WEEK_OF_YEAR",
    "WEEK_OF_MONTH",
    "DAY_OF_MONTH",
    "DAY_OF_YEAR",
    "DAY_OF_WEEK",
    "DAY_OF_WEEK_IN_MONTH",
    "AM_PM",
    "HOUR",
    "HOUR_OF_DAY",
    "MINUTE",
    "SECOND",
    "MILLISECOND",
    "ZONE_OFFSET",
    "DST_OFFSET",
];
/// `GregorianCalendar`'s `getMinimum` and `getMaximum` of each field.
const MIN_VALUES: [i32; FIELD_COUNT] = [0, 1, 0, 1, 0, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, -13 * HOUR_MS as i32, 0];
const MAX_VALUES: [i32; FIELD_COUNT] =
    [1, 292_278_994, 11, 53, 6, 31, 366, 7, 6, 1, 11, 23, 59, 59, 999, 14 * HOUR_MS as i32, 2 * HOUR_MS as i32];

/// A field's stamp: never set, computed from the time, else the order it was set in.
const UNSET: u32 = 0;
const COMPUTED: u32 = 1;
const MINIMUM_USER_STAMP: u32 = 2;

/// 1582-10-15, the first day of the Gregorian calendar (Java's default cutover), in days since the epoch.
const GREGORIAN_CUTOVER_DAY: i64 = -141_427;

/// The date and time of an instant in a zone, as a calendar's fields.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fields([i32; FIELD_COUNT]);

impl Fields {
    fn get(&self, field: usize) -> i32 {
        self.0[field]
    }

    /// The year with its era's sign: 1 BC is 0.
    fn year_ce(&self) -> i32 {
        if self.0[ERA] == 0 { 1 - self.0[YEAR] } else { self.0[YEAR] }
    }

    /// The fields of `millis` in `zone`, weeks counted from `first_day` (1 Sunday … 7 Saturday) with `min_days`
    /// in the first one (Java's `GregorianCalendar.computeFields`).
    fn of(millis: i64, zone: &TimeZone, first_day: i32, min_days: i32) -> Fields {
        let (total, standard, _) = zone.offsets_at(millis);
        let local = millis + total as i64;
        let day = local.div_euclid(DAY_MS);
        let time_of_day = local.rem_euclid(DAY_MS) as i32;
        let (year, month, dom) = date_of_day(day);
        let jan1 = day_of_date(year, 1, 1);
        let mut f = [0; FIELD_COUNT];
        f[ERA] = (year > 0) as i32;
        f[YEAR] = if year > 0 { year } else { 1 - year };
        f[MONTH] = month - 1;
        f[DAY_OF_MONTH] = dom;
        f[DAY_OF_YEAR] = (day - jan1 + 1) as i32;
        f[DAY_OF_WEEK] = day_of_week(day);
        f[DAY_OF_WEEK_IN_MONTH] = (dom - 1) / 7 + 1;
        let mut week = week_number(jan1, day, first_day, min_days);
        if week == 0 {
            // (the last week of the year before)
            week = week_number(day_of_date(year - 1, 1, 1), day, first_day, min_days);
        } else if week >= 52 {
            let next_jan1 = day_of_date(year + 1, 1, 1);
            let mut next_week1 = day_of_week_on_or_before(next_jan1 + 6, first_day);
            if next_week1 - next_jan1 >= min_days as i64 {
                next_week1 -= 7;
            }
            if day >= next_week1 {
                week = 1;
            }
        }
        f[WEEK_OF_YEAR] = week;
        f[WEEK_OF_MONTH] = week_number(day_of_date(year, month, 1), day, first_day, min_days);
        f[HOUR_OF_DAY] = time_of_day / HOUR_MS as i32;
        f[AM_PM] = f[HOUR_OF_DAY] / 12;
        f[HOUR] = f[HOUR_OF_DAY] % 12;
        f[MINUTE] = time_of_day / MINUTE_MS as i32 % 60;
        f[SECOND] = time_of_day / 1000 % 60;
        f[MILLISECOND] = time_of_day % 1000;
        f[ZONE_OFFSET] = standard;
        f[DST_OFFSET] = total - standard;
        Fields(f)
    }

    /// Java's ISO-style week-based year of these fields (its `getWeekYear`).
    fn week_year(&self) -> i32 {
        let year = self.year_ce();
        if self.0[WEEK_OF_YEAR] == 1 && self.0[MONTH] == 11 {
            year + 1
        } else if self.0[WEEK_OF_YEAR] >= 52 && self.0[MONTH] == 0 {
            year - 1
        } else {
            year
        }
    }
}

/// The days since the epoch of a date: Gregorian from the cutover on, Julian before (Java's `GregorianCalendar`).
fn day_of_date(year: i32, month: i32, day: i32) -> i64 {
    let gregorian = days_from_civil(year, month, day);
    if gregorian >= GREGORIAN_CUTOVER_DAY { gregorian } else { days_from_julian(year, month, day) }
}

/// The date of days since the epoch, Julian before the cutover.
fn date_of_day(day: i64) -> (i32, i32, i32) {
    if day >= GREGORIAN_CUTOVER_DAY { civil_from_days(day) } else { julian_from_days(day) }
}

/// Days since the epoch of a proleptic Julian date (through its Julian day number).
fn days_from_julian(year: i32, month: i32, day: i32) -> i64 {
    let a = (14 - month as i64) / 12;
    let y = year as i64 + 4800 - a;
    let m = month as i64 + 12 * a - 3;
    let jdn = day as i64 + (153 * m + 2) / 5 + 365 * y + y.div_euclid(4) - 32_083;
    jdn - 2_440_588
}

fn julian_from_days(day: i64) -> (i32, i32, i32) {
    let jdn = day + 2_440_588;
    let c = jdn + 32_082;
    let d = (4 * c + 3).div_euclid(1461);
    let e = c - (1461 * d).div_euclid(4);
    let m = (5 * e + 2) / 153;
    let dom = e - (153 * m + 2) / 5 + 1;
    let month = m + 3 - 12 * (m / 10);
    let year = d - 4800 + m / 10;
    (year as i32, month as i32, dom as i32)
}

/// Java's day of the week of days since the epoch: 1 Sunday … 7 Saturday (1970-01-01 was a Thursday).
fn day_of_week(day: i64) -> i32 {
    (day + 4).rem_euclid(7) as i32 + 1
}

/// The last day on or before `day` that is a `dow` (1 Sunday … 7 Saturday).
fn day_of_week_on_or_before(day: i64, dow: i32) -> i64 {
    day - (day_of_week(day) - dow).rem_euclid(7) as i64
}

/// The week of `day` in the period starting on `day1` (Java's `getWeekNumber`): 0 for the period before's last.
fn week_number(day1: i64, day: i64, first_day: i32, min_days: i32) -> i32 {
    let mut week1 = day_of_week_on_or_before(day1 + 6, first_day);
    if week1 - day1 >= min_days as i64 {
        week1 -= 7;
    }
    (day - week1).div_euclid(7) as i32 + 1
}

fn month_length(year: i32, month: i32) -> i32 {
    (day_of_date(if month == 12 { year + 1 } else { year }, if month == 12 { 1 } else { month + 1 }, 1)
        - day_of_date(year, month, 1)) as i32
}

/// The regions whose weeks' first has at least four days (CLDR's `minDays` 4; 1 elsewhere): Europe's ISO weeks.
const ISO_WEEK_REGIONS: &[&str] = &[
    "AD", "AN", "AT", "AX", "BE", "BG", "CH", "CZ", "DE", "DK", "EE", "ES", "FI", "FJ", "FO", "FR", "GB", "GF", "GG",
    "GI", "GP", "GR", "HU", "IE", "IM", "IS", "IT", "JE", "LI", "LT", "LU", "MC", "MQ", "NL", "NO", "PL", "PT", "RE",
    "RU", "SE", "SJ", "SK", "SM", "VA",
];

/// A locale's first day of the week (1 Sunday … 7 Saturday) and the days its first week of a year needs (CLDR's
/// week data, as the JDK's `Calendar.getInstance(locale)` has them).
fn week_data(locale: &JavaLocale) -> (i32, i32) {
    let icu = locale.icu_locale();
    let first = icu_calendar::week::WeekInformation::try_new((&icu).into())
        .map(|w| w.first_weekday as i32 % 7 + 1)
        .unwrap_or(2);
    let region = locale.country.as_ref();
    (first, if ISO_WEEK_REGIONS.contains(&region) { 4 } else { 1 })
}

/// A calendar (Java's `java.util.Calendar`, a `GregorianCalendar`): an instant and its date and time fields in a
/// zone, set one by one and resolved when read — the last set of each kind wins (`DAY_OF_MONTH` against
/// `DAY_OF_WEEK` and the weeks, `HOUR_OF_DAY` against `HOUR` and `AM_PM`), values out of range carried into the next
/// field, or refused if it isn't lenient (`IllegalArgumentException` with the field's name). A reference, as Java's
/// object: clones are the same calendar.
#[derive(Clone)]
pub struct Calendar(Rc<RefCell<CalendarState>>);

#[derive(Clone)]
struct CalendarState {
    time: i64,
    time_set: bool,
    fields: [i32; FIELD_COUNT],
    stamps: [u32; FIELD_COUNT],
    next_stamp: u32,
    /// Whether the fields are those of `time` (all of them computed).
    fields_set: bool,
    lenient: bool,
    zone: TimeZone,
    first_day_of_week: i32,
    minimal_days_in_first_week: i32,
}

impl Calendar {
    pub const ERA: CalendarField = CalendarField(0);
    pub const YEAR: CalendarField = CalendarField(1);
    pub const MONTH: CalendarField = CalendarField(2);
    pub const WEEK_OF_YEAR: CalendarField = CalendarField(3);
    pub const WEEK_OF_MONTH: CalendarField = CalendarField(4);
    pub const DATE: CalendarField = CalendarField(5);
    pub const DAY_OF_MONTH: CalendarField = CalendarField(5);
    pub const DAY_OF_YEAR: CalendarField = CalendarField(6);
    pub const DAY_OF_WEEK: CalendarField = CalendarField(7);
    pub const DAY_OF_WEEK_IN_MONTH: CalendarField = CalendarField(8);
    pub const AM_PM: CalendarField = CalendarField(9);
    pub const HOUR: CalendarField = CalendarField(10);
    pub const HOUR_OF_DAY: CalendarField = CalendarField(11);
    pub const MINUTE: CalendarField = CalendarField(12);
    pub const SECOND: CalendarField = CalendarField(13);
    pub const MILLISECOND: CalendarField = CalendarField(14);
    pub const ZONE_OFFSET: CalendarField = CalendarField(15);
    pub const DST_OFFSET: CalendarField = CalendarField(16);
    pub const FIELD_COUNT: i32 = 17;

    pub const SUNDAY: i32 = 1;
    pub const MONDAY: i32 = 2;
    pub const TUESDAY: i32 = 3;
    pub const WEDNESDAY: i32 = 4;
    pub const THURSDAY: i32 = 5;
    pub const FRIDAY: i32 = 6;
    pub const SATURDAY: i32 = 7;

    pub const JANUARY: i32 = 0;
    pub const FEBRUARY: i32 = 1;
    pub const MARCH: i32 = 2;
    pub const APRIL: i32 = 3;
    pub const MAY: i32 = 4;
    pub const JUNE: i32 = 5;
    pub const JULY: i32 = 6;
    pub const AUGUST: i32 = 7;
    pub const SEPTEMBER: i32 = 8;
    pub const OCTOBER: i32 = 9;
    pub const NOVEMBER: i32 = 10;
    pub const DECEMBER: i32 = 11;
    pub const UNDECIMBER: i32 = 12;

    pub const AM: i32 = 0;
    pub const PM: i32 = 1;

    /// A calendar of now in the default zone and locale (Java's `Calendar.getInstance()`).
    ///
    /// Kotlin: `Calendar.getInstance()`
    pub fn get_instance() -> Calendar {
        Calendar::of(TimeZone::get_default(), &JavaLocale::get_default())
    }

    /// Java's `Calendar.getInstance(zone)`.
    ///
    /// Kotlin: `Calendar.getInstance(zone: TimeZone)`
    pub fn get_instance_in(zone: TimeZone) -> Calendar {
        Calendar::of(zone, &JavaLocale::get_default())
    }

    /// Java's `Calendar.getInstance(aLocale)`: its weeks the locale's.
    ///
    /// Kotlin: `Calendar.getInstance(aLocale: Locale)`
    pub fn get_instance_for(a_locale: JavaLocale) -> Calendar {
        Calendar::of(TimeZone::get_default(), &a_locale)
    }

    /// Java's `Calendar.getInstance(zone, aLocale)`.
    ///
    /// Kotlin: `Calendar.getInstance(zone: TimeZone, aLocale: Locale)`
    pub fn get_instance_in_for(zone: TimeZone, a_locale: JavaLocale) -> Calendar {
        Calendar::of(zone, &a_locale)
    }

    fn of(zone: TimeZone, locale: &JavaLocale) -> Calendar {
        let (first_day_of_week, minimal_days_in_first_week) = week_data(locale);
        let mut state = CalendarState {
            time: 0,
            time_set: false,
            fields: [0; FIELD_COUNT],
            stamps: [UNSET; FIELD_COUNT],
            next_stamp: MINIMUM_USER_STAMP,
            fields_set: false,
            lenient: true,
            zone,
            first_day_of_week,
            minimal_days_in_first_week,
        };
        state.set_time_in_millis(current_time_millis());
        Calendar(Rc::new(RefCell::new(state)))
    }

    /// A field's value, the fields set since resolved first (Java's `get`, Kotlin's `cal[field]`); panics with
    /// Java's `IllegalArgumentException` if they don't make a time and the calendar isn't lenient.
    #[track_caller]
    pub fn get(&self, field: impl Into<CalendarField>) -> i32 {
        let field = checked_field(field.into());
        let mut s = self.0.borrow_mut();
        s.complete();
        s.fields[field]
    }

    /// Sets a field (Java's `set(field, value)`, Kotlin's `cal[field] = value`): resolved with the others when the
    /// calendar is read.
    #[track_caller]
    pub fn set(&self, field: impl Into<CalendarField>, value: i32) {
        let field = checked_field(field.into());
        self.0.borrow_mut().set_field(field, value);
    }

    /// Java's `set(year, month, date)`.
    ///
    /// Kotlin: `set(year: Int, month: Int, date: Int)`
    pub fn set_date(&self, year: i32, month: i32, date: i32) {
        let mut s = self.0.borrow_mut();
        for (f, v) in [(YEAR, year), (MONTH, month), (DAY_OF_MONTH, date)] {
            s.set_field(f, v);
        }
    }

    /// Java's `set(year, month, date, hourOfDay, minute)`.
    ///
    /// Kotlin: `set(year: Int, month: Int, date: Int, hourOfDay: Int, minute: Int)`
    pub fn set_date_time(&self, year: i32, month: i32, date: i32, hour_of_day: i32, minute: i32) {
        self.set_date(year, month, date);
        let mut s = self.0.borrow_mut();
        s.set_field(HOUR_OF_DAY, hour_of_day);
        s.set_field(MINUTE, minute);
    }

    /// Java's `set(year, month, date, hourOfDay, minute, second)`.
    ///
    /// Kotlin: `set(year: Int, month: Int, date: Int, hourOfDay: Int, minute: Int, second: Int)`
    pub fn set_date_time_seconds(&self, year: i32, month: i32, date: i32, hour_of_day: i32, minute: i32, second: i32) {
        self.set_date_time(year, month, date, hour_of_day, minute);
        self.0.borrow_mut().set_field(SECOND, second);
    }

    /// Adds `amount` to a field, the larger fields carried and the smaller kept in range (Java's `add`: a month
    /// later of January 31 is the last of February).
    #[track_caller]
    pub fn add(&self, field: impl Into<CalendarField>, amount: i32) {
        let field = checked_field(field.into());
        if amount == 0 {
            return;
        }
        let mut s = self.0.borrow_mut();
        s.complete();
        match field {
            ERA => {
                let era = (s.fields[ERA] + amount).clamp(0, 1);
                s.set_field(ERA, era);
            }
            YEAR | MONTH => {
                let months = if field == YEAR { amount as i64 * 12 } else { amount as i64 };
                let total = s.fields_year_ce() as i64 * 12 + s.fields[MONTH] as i64 + months;
                let (year, month) = (total.div_euclid(12) as i32, total.rem_euclid(12) as i32);
                let dom = s.fields[DAY_OF_MONTH].min(month_length(year, month + 1));
                let era = (year > 0) as i32;
                s.set_field(ERA, era);
                s.set_field(YEAR, if year > 0 { year } else { 1 - year });
                s.set_field(MONTH, month);
                s.set_field(DAY_OF_MONTH, dom);
            }
            ZONE_OFFSET | DST_OFFSET => panic_kt("IllegalArgumentException", ""),
            _ => {
                // A date field moves the local date by days (the time of day kept); a time field the instant.
                let days = match field {
                    WEEK_OF_YEAR | WEEK_OF_MONTH | DAY_OF_WEEK_IN_MONTH => Some(amount as i64 * 7),
                    DAY_OF_MONTH | DAY_OF_YEAR | DAY_OF_WEEK => Some(amount as i64),
                    _ => None,
                };
                match days {
                    Some(days) => {
                        let wall = s.time + s.fields[ZONE_OFFSET] as i64 + s.fields[DST_OFFSET] as i64 + days * DAY_MS;
                        let offset = s.zone.offset_by_wall(wall);
                        s.set_time_in_millis(wall - offset as i64);
                    }
                    None => {
                        let unit = match field {
                            AM_PM => 12 * HOUR_MS,
                            HOUR | HOUR_OF_DAY => HOUR_MS,
                            MINUTE => MINUTE_MS,
                            SECOND => 1000,
                            _ => 1,
                        };
                        let time = s.time + amount as i64 * unit;
                        s.set_time_in_millis(time);
                    }
                }
                return;
            }
        }
        s.complete();
    }

    /// Unsets every field: read again, they are those of 1970-01-01 00:00 in the zone (Java's `clear()`).
    ///
    /// Kotlin: `clear()`
    pub fn clear(&self) {
        let mut s = self.0.borrow_mut();
        s.fields = [0; FIELD_COUNT];
        s.stamps = [UNSET; FIELD_COUNT];
        s.time_set = false;
        s.fields_set = false;
    }

    /// Unsets a field (Java's `clear(field)`).
    ///
    /// Kotlin: `clear(field: Int)`
    #[track_caller]
    pub fn clear_field(&self, field: impl Into<CalendarField>) {
        let field = checked_field(field.into());
        let mut s = self.0.borrow_mut();
        s.fields[field] = 0;
        s.stamps[field] = UNSET;
        s.time_set = false;
        s.fields_set = false;
    }

    /// Whether a field has a value, set or computed (Java's `isSet`).
    #[track_caller]
    pub fn is_set(&self, field: impl Into<CalendarField>) -> bool {
        self.0.borrow().stamps[checked_field(field.into())] != UNSET
    }

    /// The instant (Java's `getTime`, Kotlin's `time`).
    #[track_caller]
    pub fn time(&self) -> Date {
        Date { millis: self.time_in_millis() }
    }

    /// Kotlin's `cal.time = date` (Java's `setTime`).
    pub fn set_time(&self, date: Date) {
        self.set_time_in_millis(date.millis);
    }

    /// The instant's milliseconds since the epoch (Java's `getTimeInMillis`, Kotlin's `timeInMillis`).
    #[track_caller]
    pub fn time_in_millis(&self) -> i64 {
        match self.try_time_in_millis() {
            Ok(time) => time,
            Err(message) => panic_kt("IllegalArgumentException", &message),
        }
    }

    /// The time, or what Java's `IllegalArgumentException` would say.
    pub(crate) fn try_time_in_millis(&self) -> Result<i64, String> {
        let mut s = self.0.borrow_mut();
        if !s.time_set {
            s.compute_time()?;
        }
        Ok(s.time)
    }

    /// Kotlin's `cal.timeInMillis = millis`: the fields become the instant's.
    pub fn set_time_in_millis(&self, millis: i64) {
        self.0.borrow_mut().set_time_in_millis(millis);
    }

    /// Whether values out of range are carried over (Java's `isLenient`, Kotlin's `isLenient`): yes until told.
    pub fn is_lenient(&self) -> bool {
        self.0.borrow().lenient
    }

    /// Kotlin's `cal.isLenient = lenient` (Java's `setLenient`).
    pub fn set_is_lenient(&self, lenient: bool) {
        self.0.borrow_mut().lenient = lenient;
    }

    /// The zone (Java's `getTimeZone`, Kotlin's `timeZone`).
    pub fn time_zone(&self) -> TimeZone {
        self.0.borrow().zone.clone()
    }

    /// Kotlin's `cal.timeZone = zone`: the same instant, its fields the new zone's.
    pub fn set_time_zone(&self, value: TimeZone) {
        let mut s = self.0.borrow_mut();
        s.zone = value;
        // (Java: the fields are recomputed from the time when next read)
        if s.time_set {
            let time = s.time;
            s.set_time_in_millis(time);
        }
    }

    /// The week's first day, 1 Sunday … 7 Saturday (Java's `getFirstDayOfWeek`, Kotlin's `firstDayOfWeek`): the
    /// locale's.
    pub fn first_day_of_week(&self) -> i32 {
        self.0.borrow().first_day_of_week
    }

    /// Kotlin's `cal.firstDayOfWeek = value`.
    pub fn set_first_day_of_week(&self, value: i32) {
        let mut s = self.0.borrow_mut();
        s.first_day_of_week = value;
        s.fields_set = false;
    }

    /// The days the year's first week needs (Java's `getMinimalDaysInFirstWeek`): 1, or 4 for ISO's weeks.
    pub fn minimal_days_in_first_week(&self) -> i32 {
        self.0.borrow().minimal_days_in_first_week
    }

    /// Kotlin's `cal.minimalDaysInFirstWeek = value`.
    pub fn set_minimal_days_in_first_week(&self, value: i32) {
        let mut s = self.0.borrow_mut();
        s.minimal_days_in_first_week = value;
        s.fields_set = false;
    }

    /// The largest value a field can have at the calendar's date (Java's `getActualMaximum`: the month's days for
    /// `DAY_OF_MONTH`).
    #[track_caller]
    pub fn get_actual_maximum(&self, field: impl Into<CalendarField>) -> i32 {
        let field = checked_field(field.into());
        let mut s = self.0.borrow_mut();
        s.complete();
        let year = s.fields_year_ce();
        match field {
            DAY_OF_MONTH => month_length(year, s.fields[MONTH] + 1),
            DAY_OF_YEAR => (day_of_date(year + 1, 1, 1) - day_of_date(year, 1, 1)) as i32,
            DAY_OF_WEEK_IN_MONTH => (month_length(year, s.fields[MONTH] + 1) - 1) / 7 + 1,
            WEEK_OF_MONTH => {
                let first = day_of_date(year, s.fields[MONTH] + 1, 1);
                let last = first + month_length(year, s.fields[MONTH] + 1) as i64 - 1;
                week_number(first, last, s.first_day_of_week, s.minimal_days_in_first_week)
            }
            WEEK_OF_YEAR => {
                let dec31 = day_of_date(year, 12, 31);
                let week = Fields::of(
                    (dec31 - 3) * DAY_MS - s.fields[ZONE_OFFSET] as i64,
                    &s.zone,
                    s.first_day_of_week,
                    s.minimal_days_in_first_week,
                )
                .get(WEEK_OF_YEAR);
                week.max(52)
            }
            _ => MAX_VALUES[field],
        }
    }

    /// The smallest value a field can have at the calendar's date (Java's `getActualMinimum`).
    #[track_caller]
    pub fn get_actual_minimum(&self, field: impl Into<CalendarField>) -> i32 {
        MIN_VALUES[checked_field(field.into())]
    }

    /// The field's smallest value of any date (Java's `getMinimum`).
    #[track_caller]
    pub fn get_minimum(&self, field: impl Into<CalendarField>) -> i32 {
        MIN_VALUES[checked_field(field.into())]
    }

    /// The field's largest value of any date (Java's `getMaximum`).
    #[track_caller]
    pub fn get_maximum(&self, field: impl Into<CalendarField>) -> i32 {
        MAX_VALUES[checked_field(field.into())]
    }

    /// Whether this calendar's instant is before `when`'s (Java's `before`).
    #[track_caller]
    pub fn before(&self, when: &Calendar) -> bool {
        self.time_in_millis() < when.time_in_millis()
    }

    /// Whether this calendar's instant is after `when`'s (Java's `after`).
    #[track_caller]
    pub fn after(&self, when: &Calendar) -> bool {
        self.time_in_millis() > when.time_in_millis()
    }

    /// −1, 0 or 1 as this calendar's instant is before, at or after `another_calendar`'s (Java's `compareTo`).
    #[track_caller]
    pub fn compare_to(&self, another_calendar: &Calendar) -> i32 {
        self.time_in_millis().cmp(&another_calendar.time_in_millis()) as i32
    }

    /// Another calendar of the same state (Java's `clone`; Rust's `clone` is the same calendar, as Kotlin's
    /// reference).
    ///
    /// Kotlin: `clone()`
    pub fn copy(&self) -> Calendar {
        Calendar(Rc::new(RefCell::new(self.0.borrow().clone())))
    }
}

impl PartialEq for Calendar {
    /// Java's `equals`: the same instant, leniency, weeks and zone.
    fn eq(&self, other: &Calendar) -> bool {
        if Rc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        let (a, b) = (self.time_in_millis(), other.time_in_millis());
        let (s, o) = (self.0.borrow(), other.0.borrow());
        a == b
            && s.lenient == o.lenient
            && s.first_day_of_week == o.first_day_of_week
            && s.minimal_days_in_first_week == o.minimal_days_in_first_week
            && s.zone == o.zone
    }
}

impl fmt::Debug for Calendar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.0.borrow();
        write!(f, "Calendar(time={}, zone={}, fields={:?})", s.time, s.zone.id, s.fields)
    }
}

#[track_caller]
fn checked_field(field: CalendarField) -> usize {
    if !(0..FIELD_COUNT as i32).contains(&field.0) {
        panic_kt("ArrayIndexOutOfBoundsException", &format!("Index {} out of bounds for length 17", field.0));
    }
    field.0 as usize
}

impl CalendarState {
    fn set_field(&mut self, field: usize, value: i32) {
        // (Java: a calendar's fields made whole before one is set, so the others keep the time's)
        if self.time_set && !self.fields_set {
            self.compute_fields();
        }
        self.fields[field] = value;
        self.stamps[field] = self.next_stamp;
        self.next_stamp += 1;
        self.time_set = false;
        self.fields_set = false;
    }

    fn set_time_in_millis(&mut self, millis: i64) {
        self.time = millis;
        self.time_set = true;
        self.compute_fields();
    }

    fn compute_fields(&mut self) {
        let fields = Fields::of(self.time, &self.zone, self.first_day_of_week, self.minimal_days_in_first_week);
        self.fields = fields.0;
        self.stamps = [COMPUTED; FIELD_COUNT];
        self.fields_set = true;
    }

    /// The time and every field (Java's `complete`): Java's `IllegalArgumentException` if the fields don't make a
    /// time and the calendar isn't lenient.
    #[track_caller]
    fn complete(&mut self) {
        if !self.time_set
            && let Err(message) = self.compute_time()
        {
            panic_kt("IllegalArgumentException", &message);
        }
        if !self.fields_set {
            self.compute_fields();
        }
    }

    fn fields_year_ce(&self) -> i32 {
        Fields(self.fields).year_ce()
    }

    fn is_set(&self, field: usize) -> bool {
        self.stamps[field] != UNSET
    }

    /// The time of the fields (Java's `GregorianCalendar.computeTime`): its field selection, its lenient
    /// arithmetic, its zone by wall time; not lenient, a field out of range or changed by the resolution refused.
    fn compute_time(&mut self) -> Result<(), String> {
        let original = self.fields;
        if !self.lenient {
            for field in 0..FIELD_COUNT {
                let value = self.fields[field];
                if self.stamps[field] >= MINIMUM_USER_STAMP && !(MIN_VALUES[field]..=MAX_VALUES[field]).contains(&value)
                {
                    return Err(FIELD_NAMES[field].to_string());
                }
            }
        }
        let mask = self.select_fields();
        let in_mask = |f: usize| mask & (1 << f) != 0;
        let mut year = if self.is_set(YEAR) { self.fields[YEAR] } else { 1970 };
        let era = if self.is_set(ERA) { self.fields[ERA] } else { 1 };
        if era == 0 {
            year = 1 - year;
        }
        let mut time_of_day: i64 = 0;
        if in_mask(HOUR_OF_DAY) {
            time_of_day += self.fields[HOUR_OF_DAY] as i64;
        } else {
            time_of_day += self.fields[HOUR] as i64;
            if in_mask(AM_PM) {
                time_of_day += 12 * self.fields[AM_PM] as i64;
            }
        }
        time_of_day = time_of_day * 60 + self.fields[MINUTE] as i64;
        time_of_day = time_of_day * 60 + self.fields[SECOND] as i64;
        time_of_day = time_of_day * 1000 + self.fields[MILLISECOND] as i64;
        let day = time_of_day.div_euclid(DAY_MS) + self.fixed_day(year, mask);
        let wall = day * DAY_MS + time_of_day.rem_euclid(DAY_MS);
        let mut offset = if in_mask(ZONE_OFFSET) && in_mask(DST_OFFSET) { 0 } else { self.zone.offset_by_wall(wall) };
        if in_mask(ZONE_OFFSET) || in_mask(DST_OFFSET) {
            let (total, standard) = (offset, self.zone.raw_offset());
            let zone_offset = if in_mask(ZONE_OFFSET) { self.fields[ZONE_OFFSET] } else { standard };
            let dst_offset = if in_mask(DST_OFFSET) { self.fields[DST_OFFSET] } else { total - standard };
            offset = zone_offset + dst_offset;
        }
        let time = wall - offset as i64;
        let fields = Fields::of(time, &self.zone, self.first_day_of_week, self.minimal_days_in_first_week);
        if !self.lenient {
            for field in 0..FIELD_COUNT {
                if self.stamps[field] >= MINIMUM_USER_STAMP && original[field] != fields.0[field] {
                    return Err(format!("{}: {} -> {}", FIELD_NAMES[field], original[field], fields.0[field]));
                }
            }
        }
        self.time = time;
        self.time_set = true;
        self.fields = fields.0;
        // (the fields set stay set, as Java's `setFieldsNormalized` keeps their stamps)
        for stamp in &mut self.stamps {
            if *stamp == UNSET {
                *stamp = COMPUTED;
            }
        }
        self.fields_set = true;
        Ok(())
    }

    /// Which fields make the date and the time (Java's `Calendar.selectFields`), as a mask.
    fn select_fields(&self) -> u32 {
        let st = &self.stamps;
        let aggregate = |a: u32, b: u32| if a == UNSET || b == UNSET { UNSET } else { a.max(b) };
        let mut mask: u32 = 1 << YEAR;
        if st[ERA] != UNSET {
            mask |= 1 << ERA;
        }
        let dow_stamp = st[DAY_OF_WEEK];
        let month_stamp = st[MONTH];
        let mut dom_stamp = st[DAY_OF_MONTH];
        let mut wom_stamp = aggregate(st[WEEK_OF_MONTH], dow_stamp);
        let mut dowim_stamp = aggregate(st[DAY_OF_WEEK_IN_MONTH], dow_stamp);
        let doy_stamp = st[DAY_OF_YEAR];
        let mut woy_stamp = aggregate(st[WEEK_OF_YEAR], dow_stamp);
        let mut best = dom_stamp.max(wom_stamp).max(dowim_stamp).max(doy_stamp).max(woy_stamp);
        if best == UNSET {
            wom_stamp = st[WEEK_OF_MONTH];
            dowim_stamp = st[DAY_OF_WEEK_IN_MONTH].max(dow_stamp);
            woy_stamp = st[WEEK_OF_YEAR];
            best = wom_stamp.max(dowim_stamp).max(woy_stamp);
            if best == UNSET {
                dom_stamp = month_stamp;
                best = month_stamp;
            }
        }
        if best == dom_stamp
            || (best == wom_stamp && st[WEEK_OF_MONTH] >= st[WEEK_OF_YEAR])
            || (best == dowim_stamp && st[DAY_OF_WEEK_IN_MONTH] >= st[WEEK_OF_YEAR])
        {
            mask |= 1 << MONTH;
            if best == dom_stamp {
                mask |= 1 << DAY_OF_MONTH;
            } else {
                if dow_stamp != UNSET {
                    mask |= 1 << DAY_OF_WEEK;
                }
                if wom_stamp == dowim_stamp {
                    if st[WEEK_OF_MONTH] >= st[DAY_OF_WEEK_IN_MONTH] {
                        mask |= 1 << WEEK_OF_MONTH;
                    } else {
                        mask |= 1 << DAY_OF_WEEK_IN_MONTH;
                    }
                } else if best == wom_stamp {
                    mask |= 1 << WEEK_OF_MONTH;
                } else if st[DAY_OF_WEEK_IN_MONTH] != UNSET {
                    mask |= 1 << DAY_OF_WEEK_IN_MONTH;
                }
            }
        } else if best == doy_stamp {
            mask |= 1 << DAY_OF_YEAR;
        } else {
            if dow_stamp != UNSET {
                mask |= 1 << DAY_OF_WEEK;
            }
            mask |= 1 << WEEK_OF_YEAR;
        }
        let hod_stamp = st[HOUR_OF_DAY];
        let hour_stamp = aggregate(st[HOUR], st[AM_PM]);
        let mut best = hour_stamp.max(hod_stamp);
        if best == UNSET {
            best = st[HOUR].max(st[AM_PM]);
        }
        if best != UNSET {
            if best == hod_stamp {
                mask |= 1 << HOUR_OF_DAY;
            } else {
                mask |= 1 << HOUR;
                if st[AM_PM] != UNSET {
                    mask |= 1 << AM_PM;
                }
            }
        }
        if st[MINUTE] != UNSET {
            mask |= 1 << MINUTE;
        }
        if st[SECOND] != UNSET {
            mask |= 1 << SECOND;
        }
        if st[MILLISECOND] != UNSET {
            mask |= 1 << MILLISECOND;
        }
        if st[ZONE_OFFSET] >= MINIMUM_USER_STAMP {
            mask |= 1 << ZONE_OFFSET;
        }
        if st[DST_OFFSET] >= MINIMUM_USER_STAMP {
            mask |= 1 << DST_OFFSET;
        }
        mask
    }

    /// The day (since the epoch) the date fields of `mask` make in `year` (Java's `getFixedDate`).
    fn fixed_day(&self, year: i32, mask: u32) -> i64 {
        let in_mask = |f: usize| mask & (1 << f) != 0;
        let (first_day, min_days) = (self.first_day_of_week, self.minimal_days_in_first_week);
        if in_mask(MONTH) {
            let month = self.fields[MONTH];
            let (year, month) = (year + month.div_euclid(12), month.rem_euclid(12) + 1);
            let first = day_of_date(year, month, 1);
            if in_mask(DAY_OF_MONTH) {
                let dom = if self.is_set(DAY_OF_MONTH) { self.fields[DAY_OF_MONTH] } else { 1 };
                return first + dom as i64 - 1;
            }
            if in_mask(WEEK_OF_MONTH) {
                let mut week1 = day_of_week_on_or_before(first + 6, first_day);
                if week1 - first >= min_days as i64 {
                    week1 -= 7;
                }
                if in_mask(DAY_OF_WEEK) {
                    week1 = day_of_week_on_or_before(week1 + 6, self.fields[DAY_OF_WEEK]);
                }
                return week1 + 7 * (self.fields[WEEK_OF_MONTH] as i64 - 1);
            }
            let dow = if in_mask(DAY_OF_WEEK) { self.fields[DAY_OF_WEEK] } else { first_day };
            let dowim = if in_mask(DAY_OF_WEEK_IN_MONTH) { self.fields[DAY_OF_WEEK_IN_MONTH] } else { 1 };
            if dowim >= 0 {
                return day_of_week_on_or_before(first + 7 * dowim as i64 - 1, dow);
            }
            let last = month_length(year, month) + 7 * (dowim + 1);
            return day_of_week_on_or_before(first + last as i64 - 1, dow);
        }
        let jan1 = day_of_date(year, 1, 1);
        if in_mask(DAY_OF_YEAR) {
            return jan1 + self.fields[DAY_OF_YEAR] as i64 - 1;
        }
        let mut week1 = day_of_week_on_or_before(jan1 + 6, first_day);
        if week1 - jan1 >= min_days as i64 {
            week1 -= 7;
        }
        if in_mask(DAY_OF_WEEK) && self.fields[DAY_OF_WEEK] != first_day {
            week1 = day_of_week_on_or_before(week1 + 6, self.fields[DAY_OF_WEEK]);
        }
        week1 + 7 * (self.fields[WEEK_OF_YEAR] as i64 - 1)
    }
}

/// The fields of an instant in a zone, for the formats (`SimpleDateFormat`): year, month (0–11), day of the month,
/// day of the year, day of the week (1 Sunday), day of week in month, week of year and month, hour of day, minute,
/// second, millisecond, era (0 BC), week year, offset and its abbreviation.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DateTimeFields {
    pub(crate) era: i32,
    /// The year of its era (1 BC is 1).
    pub(crate) year: i32,
    pub(crate) week_year: i32,
    pub(crate) month: i32,
    pub(crate) day_of_month: i32,
    pub(crate) day_of_year: i32,
    pub(crate) day_of_week: i32,
    pub(crate) day_of_week_in_month: i32,
    pub(crate) week_of_year: i32,
    pub(crate) week_of_month: i32,
    pub(crate) hour_of_day: i32,
    pub(crate) minute: i32,
    pub(crate) second: i32,
    pub(crate) millisecond: i32,
    /// The total offset, milliseconds.
    pub(crate) offset: i32,
    pub(crate) abbreviation: String,
}

/// The fields of `millis` in `zone`, the weeks `locale`'s.
pub(crate) fn date_time_fields(millis: i64, zone: &TimeZone, locale: &JavaLocale) -> DateTimeFields {
    let (first_day, min_days) = week_data(locale);
    let f = Fields::of(millis, zone, first_day, min_days);
    let (_, _, abbreviation) = zone.offsets_at(millis);
    DateTimeFields {
        era: f.get(ERA),
        year: f.get(YEAR),
        week_year: f.week_year(),
        month: f.get(MONTH),
        day_of_month: f.get(DAY_OF_MONTH),
        day_of_year: f.get(DAY_OF_YEAR),
        day_of_week: f.get(DAY_OF_WEEK),
        day_of_week_in_month: f.get(DAY_OF_WEEK_IN_MONTH),
        week_of_year: f.get(WEEK_OF_YEAR),
        week_of_month: f.get(WEEK_OF_MONTH),
        hour_of_day: f.get(HOUR_OF_DAY),
        minute: f.get(MINUTE),
        second: f.get(SECOND),
        millisecond: f.get(MILLISECOND),
        offset: f.get(ZONE_OFFSET) + f.get(DST_OFFSET),
        abbreviation,
    }
}

/// A calendar of `zone`, `locale`'s weeks, all fields cleared (the start of a parse), lenient or not.
pub(crate) fn cleared_calendar(zone: TimeZone, locale: &JavaLocale, lenient: bool) -> Calendar {
    let calendar = Calendar::of(zone, locale);
    calendar.clear();
    calendar.set_is_lenient(lenient);
    calendar
}

/// Kotlin's `toString` of these, as string templates write them: Java's.
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
kt_to_string_by_display!(JavaLocale, Date);

// ==== UUID ===========================================================================================================

/// A universally unique identifier, 128 bits (Java's `java.util.UUID`): a random one, or one read from its text.
/// Its text is Java's: five groups of hex digits, `8-4-4-4-12`, lowercase.
///
/// Kotlin: `java.util.UUID`
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UUID {
    most_sig_bits: u64,
    least_sig_bits: u64,
}

impl UUID {
    /// The UUID of the two halves (Java's `UUID(long, long)`).
    ///
    /// Kotlin: `UUID(mostSigBits: Long, leastSigBits: Long)`
    pub fn new(most_sig_bits: i64, least_sig_bits: i64) -> UUID {
        UUID { most_sig_bits: most_sig_bits as u64, least_sig_bits: least_sig_bits as u64 }
    }

    /// A random one, version 4 (Java's `UUID.randomUUID()`): from the process's random hasher keys, a counter and
    /// the clock.
    ///
    /// Kotlin: `UUID.randomUUID()`
    pub fn random_uuid() -> UUID {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = crate::clock::unix_time().as_nanos() as u128;
        let half = |i: u64| {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(n);
            h.write_u64(i);
            h.write_u128(nanos);
            h.finish()
        };
        let (msb, lsb) = (half(0), half(1));
        // (version 4 in the 13th digit, the IETF variant in the 17th)
        UUID {
            most_sig_bits: (msb & !0xf000) | 0x4000,
            least_sig_bits: (lsb & 0x3fff_ffff_ffff_ffff) | 0x8000_0000_0000_0000,
        }
    }

    /// The UUID its text names (Java's `UUID.fromString`): five groups of hex digits; an
    /// IllegalArgumentException otherwise ("Invalid UUID string: …").
    ///
    /// Kotlin: `UUID.fromString(name: String)`
    #[track_caller]
    pub fn from_string(name: &str) -> UUID {
        UUID::parse(name)
            .unwrap_or_else(|| panic_kt("IllegalArgumentException", &format!("Invalid UUID string: {name}")))
    }

    /// The UUID its text names, if it names one.
    fn parse(name: &str) -> Option<UUID> {
        let parts: Vec<&str> = name.split('-').collect();
        let valid = parts.len() == 5
            && name.len() <= 36
            && parts.iter().all(|p| !p.is_empty() && p.len() <= 16 && p.chars().all(|c| c.is_ascii_hexdigit()));
        if !valid {
            return None;
        }
        let n = |k: usize| u64::from_str_radix(parts[k], 16).unwrap_or(0);
        Some(UUID {
            most_sig_bits: (n(0) << 32) | ((n(1) & 0xffff) << 16) | (n(2) & 0xffff),
            least_sig_bits: ((n(3) & 0xffff) << 48) | (n(4) & 0xffff_ffff_ffff),
        })
    }

    /// Its upper 64 bits (Java's `getMostSignificantBits`).
    pub fn most_significant_bits(&self) -> i64 {
        self.most_sig_bits as i64
    }

    /// Its lower 64 bits (Java's `getLeastSignificantBits`).
    pub fn least_significant_bits(&self) -> i64 {
        self.least_sig_bits as i64
    }

    /// Its version (Java's `version()`): 4 for a random one.
    pub fn version(&self) -> i32 {
        ((self.most_sig_bits >> 12) & 0xf) as i32
    }
}

/// Java's `toString`: `0f0e0d0c-0b0a-0908-0706-050403020100`.
impl fmt::Display for UUID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (m, l) = (self.most_sig_bits, self.least_sig_bits);
        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            m >> 32,
            (m >> 16) & 0xffff,
            m & 0xffff,
            l >> 48,
            l & 0xffff_ffff_ffff
        )
    }
}

impl fmt::Debug for UUID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Saved as its text (what an app's `KSerializer<UUID>` writes: NavigationSamples' and the codec samples'
/// `UUIDSerializer`).
impl crate::Saveable for UUID {
    fn save(&self) -> crate::SavedValue {
        crate::SavedValue::from(self.to_string().as_str())
    }
    fn restore(saved: &crate::SavedValue) -> Option<UUID> {
        UUID::parse(saved.as_str()?)
    }
}

impl super::KtToString for UUID {
    fn kt_write(&self, out: &mut String) {
        use fmt::Write;
        let _ = write!(out, "{self}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc() -> TimeZone {
        TimeZone::get_time_zone("GMT")
    }

    fn calendar_at(millis: i64, zone: TimeZone) -> Calendar {
        let cal = Calendar::get_instance_in_for(zone, JavaLocale::US);
        cal.set_time_in_millis(millis);
        cal
    }

    fn millis(y: i32, m: i32, d: i32, h: i64, min: i64) -> i64 {
        days_from_civil(y, m, d) * DAY_MS + h * HOUR_MS + min * MINUTE_MS
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn julian_dates_before_the_cutover() {
        assert_eq!(julian_from_days(GREGORIAN_CUTOVER_DAY - 1), (1582, 10, 4));
        assert_eq!(days_from_julian(1582, 10, 4), GREGORIAN_CUTOVER_DAY - 1);
        for day in [-500_000, -200_000, GREGORIAN_CUTOVER_DAY - 1] {
            let (y, m, d) = julian_from_days(day);
            assert_eq!(days_from_julian(y, m, d), day);
        }
        assert_eq!(day_of_date(1582, 10, 15), GREGORIAN_CUTOVER_DAY);
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn fields_are_resolved_as_javas() {
        // 2023-04-15 (a Saturday) 10:15 UTC.
        let cal = calendar_at(millis(2023, 4, 15, 10, 15), utc());
        assert_eq!(
            [Calendar::YEAR, Calendar::MONTH, Calendar::DAY_OF_MONTH, Calendar::DAY_OF_WEEK, Calendar::HOUR]
                .map(|f| cal.get(f)),
            [2023, Calendar::APRIL, 15, Calendar::SATURDAY, 10]
        );
        assert_eq!((cal.get(Calendar::DAY_OF_YEAR), cal.get(Calendar::WEEK_OF_YEAR)), (105, 15));
        // The last set wins: a month then a day is that day of that month (January 31 → February 15).
        let cal = calendar_at(millis(2023, 1, 31, 0, 0), utc());
        cal.set(Calendar::MONTH, Calendar::FEBRUARY);
        cal.set(Calendar::DAY_OF_MONTH, 15);
        assert_eq!(cal.time_in_millis(), millis(2023, 2, 15, 0, 0));
        // A day of the week set last: that day of the same week (Sunday first in the US).
        cal.set(Calendar::DAY_OF_WEEK, Calendar::MONDAY);
        assert_eq!(cal.get(Calendar::DAY_OF_MONTH), 13);
        // Lenient: 75 minutes carry into the hour.
        cal.set(Calendar::HOUR_OF_DAY, 9);
        cal.set(Calendar::MINUTE, 75);
        assert_eq!((cal.get(Calendar::HOUR_OF_DAY), cal.get(Calendar::MINUTE)), (10, 15));
        // HOUR and AM_PM set after HOUR_OF_DAY win.
        cal.set(Calendar::HOUR_OF_DAY, 3);
        cal.set(Calendar::HOUR, 4);
        cal.set(Calendar::AM_PM, Calendar::PM);
        assert_eq!(cal.get(Calendar::HOUR_OF_DAY), 16);
        // The day in the month's week and the year's day.
        cal.set(Calendar::DAY_OF_WEEK_IN_MONTH, -1);
        cal.set(Calendar::DAY_OF_WEEK, Calendar::FRIDAY);
        assert_eq!(cal.get(Calendar::DAY_OF_MONTH), 24);
        cal.set(Calendar::DAY_OF_YEAR, 1);
        assert_eq!((cal.get(Calendar::MONTH), cal.get(Calendar::DAY_OF_MONTH)), (0, 1));
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn a_calendar_not_lenient_refuses_what_isnt_a_time() {
        let cal = calendar_at(millis(2023, 2, 10, 0, 0), utc());
        cal.set_is_lenient(false);
        cal.set(Calendar::HOUR_OF_DAY, 25);
        let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cal.time())).unwrap_err();
        assert_eq!(refused.downcast_ref::<String>().unwrap(), "IllegalArgumentException: HOUR_OF_DAY");
        let cal = calendar_at(millis(2023, 2, 10, 0, 0), utc());
        cal.set_is_lenient(false);
        cal.set(Calendar::DAY_OF_MONTH, 30);
        let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cal.get(Calendar::MONTH))).unwrap_err();
        assert_eq!(refused.downcast_ref::<String>().unwrap(), "IllegalArgumentException: DAY_OF_MONTH: 30 -> 2");
        let cal = calendar_at(millis(2023, 2, 10, 0, 0), utc());
        cal.set_is_lenient(false);
        cal.set(Calendar::HOUR_OF_DAY, 15);
        cal.set(Calendar::MINUTE, 30);
        assert_eq!(cal.time(), Date::of_millis(millis(2023, 2, 10, 15, 30)));
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn adding_carries_and_clamps() {
        let cal = calendar_at(millis(2023, 1, 31, 12, 0), utc());
        cal.add(Calendar::MONTH, 1);
        assert_eq!(cal.time_in_millis(), millis(2023, 2, 28, 12, 0));
        cal.add(Calendar::DAY_OF_MONTH, 2);
        assert_eq!(cal.time_in_millis(), millis(2023, 3, 2, 12, 0));
        cal.add(Calendar::HOUR_OF_DAY, -13);
        assert_eq!(cal.time_in_millis(), millis(2023, 3, 1, 23, 0));
        cal.add(Calendar::YEAR, -2024);
        assert_eq!((cal.get(Calendar::ERA), cal.get(Calendar::YEAR)), (0, 2));
        assert_eq!(cal.get_actual_maximum(Calendar::DAY_OF_MONTH), 31);
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn zones_by_id_and_offsets() {
        assert_eq!(TimeZone::get_time_zone("GMT+3").id(), "GMT+03:00");
        assert_eq!(TimeZone::get_time_zone("GMT-0830").raw_offset(), -(8 * 60 + 30) * 60_000);
        assert_eq!(TimeZone::get_time_zone("Nowhere/Never").id(), "GMT");
        assert_eq!(TimeZone::get_time_zone("UTC").get_offset(0), 0);
        let plus3 = TimeZone::get_time_zone("GMT+03:00");
        let cal = calendar_at(millis(2023, 4, 15, 22, 0), plus3.clone());
        assert_eq!((cal.get(Calendar::DAY_OF_MONTH), cal.get(Calendar::HOUR_OF_DAY)), (16, 1));
        assert_eq!(cal.get(Calendar::ZONE_OFFSET), 3 * HOUR_MS as i32);
        // (where the system has its zoneinfo database)
        let ny = TimeZone::get_time_zone("America/New_York");
        if ny.id() == "America/New_York" {
            assert_eq!(ny.get_offset(millis(2023, 7, 1, 0, 0)), -4 * HOUR_MS as i32);
            assert_eq!(ny.raw_offset(), -5 * HOUR_MS as i32);
            assert!(ny.use_daylight_time());
            // 02:30 on 2023-03-12 doesn't exist there: Java's 03:30 EDT.
            let cal = calendar_at(millis(2023, 3, 12, 12, 0), ny);
            cal.set(Calendar::HOUR_OF_DAY, 2);
            cal.set(Calendar::MINUTE, 30);
            assert_eq!((cal.get(Calendar::HOUR_OF_DAY), cal.get(Calendar::DST_OFFSET)), (3, HOUR_MS as i32));
        }
    }

    #[test]
    fn locales_as_java_keeps_them() {
        let he = JavaLocale::new("HE");
        assert_eq!((he.language(), he.country(), he.to_string()), ("he".into(), "".into(), "he".into()));
        assert_eq!(JavaLocale::with_country("en", "us"), JavaLocale::US);
        assert_eq!(JavaLocale::US.to_string(), "en_US");
        assert_eq!(JavaLocale::US.to_language_tag(), "en-US");
        let tw = JavaLocale::for_language_tag("zh-Hant-TW");
        assert_eq!((tw.script(), tw.to_string()), ("Hant".into(), "zh_TW_#Hant".into()));
        assert_eq!(JavaLocale::for_language_tag("und").to_string(), "");
        assert_eq!(JavaLocale::of_posix_name("tr_TR.UTF-8"), JavaLocale::with_country("tr", "TR"));
        assert_eq!(JavaLocale::of_posix_name("C"), JavaLocale::ENGLISH);
        assert_eq!(week_data(&JavaLocale::US), (Calendar::SUNDAY, 1));
        assert_eq!(week_data(&JavaLocale::GERMANY), (Calendar::MONDAY, 4));
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads the system's zoneinfo, which Miri's isolation forbids")]
    fn dates_print_as_javas() {
        let date = Date::of_millis(millis(2023, 4, 15, 10, 15) + 30_000);
        TimeZone::set_default(Some(TimeZone::get_time_zone("GMT+03:00")));
        assert_eq!(date.to_string(), "Sat Apr 15 13:15:30 GMT+03:00 2023");
        TimeZone::set_default(Some(TimeZone::get_time_zone("UTC")));
        assert_eq!(date.to_string(), "Sat Apr 15 10:15:30 UTC 2023");
        TimeZone::set_default(None);
        assert!(date.before(Date::of_millis(date.time() + 1)) && date.compare_to(date) == 0);
    }

    #[test]
    #[cfg_attr(miri, ignore = "a random UUID reads the real-time clock, which Miri's isolation forbids")]
    fn uuids_as_javas() {
        let text = "0f0e0d0c-0b0a-0908-0706-050403020100";
        let uuid = UUID::from_string(text);
        assert_eq!(uuid.to_string(), text);
        assert_eq!(UUID::from_string("1-2-3-4-5").to_string(), "00000001-0002-0003-0004-000000000005");
        let random = UUID::random_uuid();
        assert!(random.version() == 4 && random != UUID::random_uuid());
        assert_eq!(UUID::from_string(&random.to_string()), random);
        assert!(std::panic::catch_unwind(|| UUID::from_string("nope")).is_err());
    }
}

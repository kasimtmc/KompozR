//! The locale dates and numbers are written in (Compose's CalendarLocale.kt with Compose for desktop's actuals:
//! `CalendarLocale` is `java.util.Locale` there, `defaultLocale()` the JVM's default locale, `toLocalString` a
//! `java.text.NumberFormat`). KompozR's locale is the platform's language, [`LocalStrings`]' `locale` (its
//! `KOMPOZ_LANGUAGE`, the one Material's texts follow), and CLDR's data through ICU4X takes Java's place.

use std::fmt;

use icu_decimal::DecimalFormatter;
use icu_decimal::input::Decimal;
use icu_decimal::options::{DecimalFormatterOptions, GroupingStrategy};
use icu_locale_core::Locale;
use kompoz_runtime::kompozable;
use kompoz_ui::strings::LocalStrings;
use writeable::Writeable;

/// A locale for formatting dates and numbers (Compose's `CalendarLocale`, Compose for desktop's
/// `java.util.Locale`): a BCP 47 language tag.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CalendarLocale {
    locale: Locale,
}

impl CalendarLocale {
    /// The locale of a BCP 47 language tag (Java's `Locale.forLanguageTag`): one that isn't well formed gives the
    /// root locale (`und`), as Java's does. POSIX names (`tr_TR.UTF-8`) are taken too.
    pub fn for_language_tag(tag: &str) -> CalendarLocale {
        let locale = Locale::try_from_str(tag).ok().or_else(|| {
            // A POSIX name, as the platform's language comes (`tr_TR.UTF-8`).
            tag.contains(['_', '.', '@'])
                .then(|| Locale::try_from_str(&kompoz_ui::strings::language_tag(tag)).ok())
                .flatten()
        });
        CalendarLocale { locale: locale.unwrap_or(Locale::UNKNOWN) }
    }

    /// The BCP 47 language tag (Java's `Locale.toLanguageTag`).
    pub fn to_language_tag(&self) -> String {
        self.locale.to_string()
    }

    /// ICU4X's locale this is.
    pub fn icu_locale(&self) -> &Locale {
        &self.locale
    }
}

impl From<Locale> for CalendarLocale {
    fn from(locale: Locale) -> CalendarLocale {
        CalendarLocale { locale }
    }
}

/// (stage 14 WPD) Compose for desktop's `CalendarLocale` is `java.util.Locale`: the kt shims' one (Kotlin's `Locale`
/// in a file importing `java.util.Locale`, `Locale("he")`) is one.
impl From<kompoz_runtime::kt::JavaLocale> for CalendarLocale {
    fn from(locale: kompoz_runtime::kt::JavaLocale) -> CalendarLocale {
        CalendarLocale { locale: locale.icu_locale() }
    }
}

impl fmt::Debug for CalendarLocale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Java's `Locale.toString` writes `tr_TR`: the tag is what tells one apart here.
        write!(f, "{}", self.locale)
    }
}

/// The locale dates are written in (Compose's internal `defaultLocale()`; Compose for desktop's actual is the JVM's
/// default locale, the system's): KompozR's platform language, [`LocalStrings`]' `locale`.
#[kompozable]
pub(crate) fn default_locale() -> CalendarLocale {
    CalendarLocale::for_language_tag(&LocalStrings.current().locale)
}

/// `value` written in `locale`'s digits with at least `min_digits` and at most `max_digits` of them (the highest
/// dropped, as Java's `NumberFormat.maximumIntegerDigits`), grouped by the locale's separator if `is_grouping_used`
/// (Compose's internal `Int.toLocalString`). Kotlin's `locale = null` (Java's default locale) is the caller's
/// [`default_locale`], a composable read here: the callers pass it.
pub(crate) fn to_local_string(
    value: i32,
    min_digits: i32,
    max_digits: i32,
    is_grouping_used: bool,
    locale: &CalendarLocale,
) -> String {
    let mut options = DecimalFormatterOptions::default();
    options.grouping_strategy = Some(if is_grouping_used { GroupingStrategy::Auto } else { GroupingStrategy::Never });
    // Java drops the digits above the maximum: the value modulo 10^max (the sign kept).
    let value = if max_digits < 10 { value % 10_i32.pow(max_digits.max(0) as u32).max(1) } else { value };
    let mut decimal = Decimal::from(value);
    decimal.absolute.pad_start(min_digits.min(max_digits).clamp(0, i16::MAX as i32) as i16);
    match DecimalFormatter::try_new(locale.icu_locale().into(), options) {
        Ok(formatter) => formatter.format(&decimal).write_to_string().into_owned(),
        // ICU4X's compiled data falls back to the root locale: it has every locale's formatter.
        Err(_) => decimal.to_string(),
    }
}

/// [`to_local_string`] with Compose's defaults (`minDigits = 1, maxDigits = 40, isGroupingUsed = false`) but
/// `min_digits`.
pub(crate) fn to_local_string_min(value: i32, min_digits: i32, locale: &CalendarLocale) -> String {
    to_local_string(value, min_digits, 40, false, locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_take_the_locales_digits_and_widths() {
        let en = CalendarLocale::for_language_tag("en");
        assert_eq!(to_local_string_min(5, 2, &en), "05");
        assert_eq!(to_local_string_min(2026, 1, &en), "2026");
        assert_eq!(to_local_string(12345, 1, 40, true, &en), "12,345");
        assert_eq!(to_local_string(12345, 1, 3, false, &en), "345");
        let tr = CalendarLocale::for_language_tag("tr_TR.UTF-8");
        assert_eq!(tr.to_language_tag(), "tr-TR");
        assert_eq!(to_local_string(12345, 1, 40, true, &tr), "12.345");
        let ar = CalendarLocale::for_language_tag("ar-EG");
        assert_eq!(to_local_string_min(7, 2, &ar), "٠٧");
        assert_eq!(CalendarLocale::for_language_tag("??").to_language_tag(), "und");
    }
}

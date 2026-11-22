//! Whether the platform writes times on a 24-hour clock, as Compose's TimeFormat.kt (material3 1.5.0-alpha17) with
//! Compose for desktop's TimeFormat.skiko.kt: what [`remember_time_picker_state`](crate::remember_time_picker_state)
//! takes by default.

use crate::calendar_locale::default_locale;
use crate::internal::platform_date_format::PlatformDateFormat;
use kompoz_runtime::kompozable;

/// Whether times are written on a 24-hour clock in the platform's locale (Compose's internal composable
/// `is24HourFormat`; Compose for desktop's actual: `PlatformDateFormat(defaultLocale()).is24HourFormat()`, the
/// locale's CLDR time format — English's 12-hour clock, Turkish's 24-hour one). Read as the platform's language,
/// [`LocalStrings`](kompoz_ui::strings::LocalStrings)' locale, changes.
#[kompozable]
pub(crate) fn is_24_hour_format() -> bool {
    PlatformDateFormat::new(default_locale()).is_24_hour_format()
}

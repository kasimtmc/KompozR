//! What the crates ask of the process's environment and the system, on every target: environment variables
//! ([`var`]: the options `KOMPOZ_*`, `TZ`, `LANG` …), the languages the user prefers ([`system_languages`]) and
//! the system's time zone and zone database ([`set_system_time_zone`], [`set_time_zone_database`]; the zones
//! themselves are [`kt::TimeZone`](crate::kt::TimeZone)'s and [`kt::ZoneId`](crate::kt::ZoneId)'s).
//!
//! Natively the variables are the process's. On the web (wasm32-unknown-unknown, which has no environment) they
//! are what the web platform hands over when it starts (`set_vars`: the page's query string, as C++'s
//! `?KOMPOZ_GPU=…`); until it does there are none.
//!
//! What a platform knows better than the environment it says here: the languages (Android's `Locale` list, the
//! browser's `navigator.languages`, macOS's preferred languages) and the time zone (Java's `TimeZone.getDefault`, the
//! browser's `Intl` zone), with the zones' data where the system has no TZif files (the web's embedded database).
//! Without a platform's word: Windows' own calls (`GetUserDefaultLocaleName`, `GetDynamicTimeZoneInformation`),
//! Android's system properties and its tzdata bundle, the Unix files elsewhere.

use std::borrow::Cow;
use std::ffi::OsString;
use std::sync::RwLock;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
static VARS: RwLock<Vec<(String, String)>> = RwLock::new(Vec::new());

/// The environment variable `name`, if it is set (to Unicode).
pub fn var(name: &str) -> Option<String> {
    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    {
        VARS.read().unwrap_or_else(|e| e.into_inner()).iter().find(|(n, _)| n == name).map(|(_, v)| v.clone())
    }
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    {
        std::env::var(name).ok()
    }
}

/// The environment variable `name` as the system has it (a path that needn't be Unicode), if it is set.
pub fn var_os(name: &str) -> Option<OsString> {
    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    {
        var(name).map(OsString::from)
    }
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    {
        std::env::var_os(name)
    }
}

/// Whether the environment variable `name` is set to `1` (a flag: `KOMPOZ_GPU_VALIDATION=1`).
pub fn flag(name: &str) -> bool {
    var_os(name).is_some_and(|v| v == "1")
}

/// The web's environment from now on: the web platform's, from the page's query string (`?KOMPOZ_GPU_VALIDATION=1
/// &LANG=tr_TR`), set when it starts; a name given twice is the last one's.
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub fn set_vars(vars: impl IntoIterator<Item = (String, String)>) {
    let mut all: Vec<(String, String)> = Vec::new();
    for (name, value) in vars {
        all.retain(|(n, _)| *n != name);
        all.push((name, value));
    }
    *VARS.write().unwrap_or_else(|e| e.into_inner()) = all;
}

// ---- Languages ------------------------------------------------------------------------------------------------

static LANGUAGES: RwLock<Option<Vec<String>>> = RwLock::new(None);

/// The languages the user prefers, as the system says them (BCP 47 tags: `tr-TR`, `en`), the first the most: the
/// platform's (set with [`set_system_languages`]), else the system's own where it can be asked without one
/// (Windows' `GetUserDefaultLocaleName`); none if neither says. The environment's POSIX variables (`LANG` …) are
/// read by those who ask before this, each its own way (they are the user's word over the system's, as on Linux).
pub fn system_languages() -> Vec<String> {
    if let Some(set) = LANGUAGES.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return set.clone();
    }
    own::languages()
}

/// The languages the user prefers, as the platform knows them (Android's `LocaleList`, the browser's
/// `navigator.languages`, macOS's `preferredLanguages`), from now on (when it starts and when the user changes
/// them); none to ask the system again.
pub fn set_system_languages(languages: Option<Vec<String>>) {
    *LANGUAGES.write().unwrap_or_else(|e| e.into_inner()) = languages.map(given_languages);
}

/// The tags a platform gave, the empty ones left out.
fn given_languages(tags: Vec<String>) -> Vec<String> {
    tags.into_iter().filter(|t| !t.is_empty()).collect()
}

// ---- Time zones -----------------------------------------------------------------------------------------------

/// A zone database: the TZif bytes (RFC 8536) of a zone by its IANA ID (`Europe/Istanbul`), none for one it hasn't.
pub type TimeZoneDatabase = Box<dyn Fn(&str) -> Option<Cow<'static, [u8]>> + Send + Sync>;

static ZONE_ID: RwLock<Option<String>> = RwLock::new(None);
static ZONE_DATABASE: RwLock<Option<TimeZoneDatabase>> = RwLock::new(None);

/// The system's time zone by its IANA ID, as the platform knows it (Java's `TimeZone.getDefault().getID()` on
/// Android, `Intl.DateTimeFormat().resolvedOptions().timeZone` on the web), from now on; none to find it as the
/// system has it again. `TZ` in the environment is still the user's word over it (as the JDK reads it).
pub fn set_system_time_zone(id: Option<String>) {
    *ZONE_ID.write().unwrap_or_else(|e| e.into_inner()) = id.filter(|id| !id.is_empty());
    crate::kt::forget_system_time_zone();
}

/// The zone database zones are looked up in first from now on (the web's, embedded: it has no zoneinfo files);
/// none for the system's own alone.
pub fn set_time_zone_database(database: Option<TimeZoneDatabase>) {
    *ZONE_DATABASE.write().unwrap_or_else(|e| e.into_inner()) = database;
    crate::kt::forget_system_time_zone();
}

/// What [`set_system_time_zone`] said.
pub(crate) fn platform_time_zone() -> Option<String> {
    ZONE_ID.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// A zone's TZif bytes from what [`set_time_zone_database`] gave, if it has the zone.
pub(crate) fn platform_zone_data(id: &str) -> Option<Cow<'static, [u8]>> {
    ZONE_DATABASE.read().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|database| database(id))
}

/// The system's own word, where it is asked without a platform.
#[cfg(windows)]
mod own {
    #![allow(unsafe_code)]

    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;

    /// `LOCALE_NAME_MAX_LENGTH`, with its terminating 0.
    const LOCALE_NAME_MAX_LENGTH: usize = 85;

    /// The user's default locale (`GetUserDefaultLocaleName`: `tr-TR`), as C++'s `preferredLanguage` asks.
    pub(super) fn languages() -> Vec<String> {
        let mut name = [0u16; LOCALE_NAME_MAX_LENGTH];
        // SAFETY: `name` is LOCALE_NAME_MAX_LENGTH writable UTF-16 units, the length passed; the call writes at most
        // that many (its terminating 0 counted) and returns how many, 0 on failure.
        let n = unsafe { GetUserDefaultLocaleName(name.as_mut_ptr(), name.len() as i32) };
        if n <= 1 {
            return Vec::new();
        }
        let tag = String::from_utf16_lossy(&name[..n as usize - 1]);
        // (`x-IV-mathan`, the invariant locale's, names no language)
        if tag.is_empty() || tag.starts_with("x-") { Vec::new() } else { vec![tag] }
    }
}

#[cfg(not(windows))]
mod own {
    /// None here: Linux says it in the environment; Android, the web and macOS through their platforms.
    pub(super) fn languages() -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_platform_says_the_languages() {
        // (not set here: the process's other tests read them, and Java's default locale is kept once read)
        assert_eq!(given_languages(vec!["tr-TR".into(), String::new(), "en".into()]), ["tr-TR", "en"]);
        if cfg!(not(windows)) {
            assert!(system_languages().is_empty());
        }
    }

    #[test]
    #[cfg(windows)]
    fn windows_says_the_users_locale() {
        let tags = own::languages();
        assert_eq!(tags.len(), 1);
        assert!(tags[0].parse::<icu_locale_core::Locale>().is_ok(), "{tags:?}");
    }

    #[test]
    fn variables_are_the_processes() {
        assert_eq!(var("KOMPOZ_SURELY_NOT_SET_ANYWHERE"), None);
        assert!(!flag("KOMPOZ_SURELY_NOT_SET_ANYWHERE"));
        assert_eq!(var("PATH"), std::env::var("PATH").ok());
    }
}

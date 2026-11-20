//! Material's texts, as Compose for desktop's internal Strings.skiko.kt: a text by its id ([`Strings`]) in the UI's
//! language — KompozR's [`LocalStrings`]' locale (the platform's language, `KOMPOZ_LANGUAGE`) — through Compose's
//! locale chain (`pt_BR`, then `pt`, then English), from the translations generated of Compose for desktop's l10n
//! (`l10n/`, `cargo xtask material3-strings`).

use super::l10n::{Translation, translation_for};
use kompoz_ui::strings::LocalStrings;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

/// A text of Material's (Compose's internal `Strings` value class): its id; the ids are its constants
/// (`Strings::NavigationMenu`, …, generated with the translations).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Strings(pub(crate) i32);

impl fmt::Debug for Strings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Strings(value={})", self.0)
    }
}

thread_local! {
    /// The translations found so far by locale tag (Compose's `translationByLocaleTag`).
    static TRANSLATION_BY_LOCALE_TAG: RefCell<HashMap<String, &'static Translation>> = RefCell::new(HashMap::new());
}

/// Compose's internal locale tag: `""`, `"tr"`, `"pt_BR"`.
fn locale_tag(language: &str, region: &str) -> String {
    if language.is_empty() {
        String::new()
    } else if region.is_empty() {
        language.to_string()
    } else {
        format!("{language}_{region}")
    }
}

/// The tags to look a translation up by, the most specific first (Compose's `localeTagChain`).
fn locale_tag_chain(language: &str, region: &str) -> Vec<String> {
    let mut chain = Vec::new();
    if !region.is_empty() {
        chain.push(locale_tag(language, region));
    }
    if !language.is_empty() {
        chain.push(locale_tag(language, ""));
    }
    chain.push(locale_tag("", ""));
    chain
}

/// The translation of a locale (Compose's `findTranslation`): the first of its chain there is.
fn find_translation(language: &str, region: &str) -> &'static Translation {
    locale_tag_chain(language, region)
        .iter()
        .find_map(|tag| translation_for(tag))
        .expect("the default translation (English's) is always there")
}

/// `string` in a language and region (Compose's `getTranslation`).
pub(crate) fn translation(string: Strings, language: &str, region: &str) -> String {
    let tag = locale_tag(language, region);
    let translation = TRANSLATION_BY_LOCALE_TAG
        .with(|t| *t.borrow_mut().entry(tag).or_insert_with(|| find_translation(language, region)));
    match translation.get(string.0 as usize).copied().flatten() {
        Some(text) => text.to_string(),
        None => panic!("Missing translation for {string:?}"),
    }
}

/// `string` in the UI's language (Compose's internal composable `getString`): [`LocalStrings`]' locale.
pub(crate) fn get_string(string: Strings) -> String {
    let strings = LocalStrings.current();
    translation(string, strings.locale_language(), strings.locale_region())
}

/// `string` in the UI's language, its `%1$s`/`%1$d` places filled with `format_args` in order (Compose's
/// `getString(string, vararg formatArgs)`).
pub(crate) fn get_string_formatted(string: Strings, format_args: &[&dyn fmt::Display]) -> String {
    format_string(&get_string(string), format_args)
}

/// `%n$d` and `%n$s` replaced by the n-th argument (Compose for desktop's internal `formatString`).
pub(crate) fn format_string(string: &str, format_args: &[&dyn fmt::Display]) -> String {
    let mut result = string.to_string();
    for (index, arg) in format_args.iter().enumerate() {
        let arg = arg.to_string();
        result = result.replace(&format!("%{}$d", index + 1), &arg).replace(&format!("%{}$s", index + 1), &arg);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_follow_compose_for_desktops_locale_chain() {
        // The region's translation, else the language's, else English (Compose's `localeTagChain`).
        assert_eq!(translation(Strings::CloseSheet, "pt", "BR"), "Fechar planilha");
        assert_eq!(translation(Strings::CloseSheet, "pt", "PT"), "Fechar folha");
        assert_eq!(translation(Strings::CloseSheet, "pt", "AO"), "Fechar planilha");
        assert_eq!(translation(Strings::CloseSheet, "zh", "TW"), "關閉功能表");
        assert_eq!(translation(Strings::CloseSheet, "de", ""), "Tabelle schließen");
        assert_eq!(translation(Strings::CloseSheet, "tr", "TR"), "Sayfayı kapat");
        assert_eq!(translation(Strings::CloseSheet, "xx", ""), "Close sheet");
        // (Hebrew by both its codes)
        assert_eq!(translation(Strings::Dialog, "he", ""), translation(Strings::Dialog, "iw", ""));
    }

    #[test]
    fn texts_follow_kompozrs_language() {
        // (outside a composition: LocalStrings' default, English)
        assert_eq!(get_string(Strings::NavigationMenu), "Navigation menu");
        let languages = kompoz_ui::strings::Strings::for_language("tr_TR.UTF-8");
        assert_eq!(
            translation(Strings::NavigationMenu, languages.locale_language(), languages.locale_region()),
            "Gezinme menüsü"
        );
    }

    #[test]
    fn formats_fill_their_places() {
        assert_eq!(format_string("%1$s - %2$s", &[&1990, &"2030"]), "1990 - 2030");
        assert_eq!(format_string("%1$d saat", &[&7]), "7 saat");
        assert_eq!(get_string_formatted(Strings::TimePickerHourSuffix, &[&3]), "3 o'clock");
    }
}

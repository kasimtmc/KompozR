//! The adaptive layout's texts, as Compose for desktop's internal Strings.skiko.kt (adaptive-layout 1.2.0): a text by
//! its id ([`Strings`]) in the UI's language — KompozR's [`LocalStrings`]' locale (the platform's language,
//! `KOMPOZ_LANGUAGE`) — through Compose's locale chain (`pt_BR`, then `pt`, then English), from the translations
//! generated of Compose for desktop's l10n (`l10n/`, `cargo xtask adaptive-strings`); read in a composition
//! ([`get_string`]) or by a modifier node, through its composition locals ([`get_string_of_node`]).

use super::l10n::{Translation, translation_for};
use kompoz_ui::modifier::NodeHandle;
use kompoz_ui::strings::{LocalStrings, Strings as UiStrings};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

/// A text of the adaptive layout's (Compose's internal `Strings` value class): its id; the ids are its constants
/// (`Strings::defaultPaneTitlePrimary`, …, generated with the translations).
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

/// Compose's private locale tag: `""`, `"tr"`, `"pt_BR"`.
fn locale_tag(language: &str, region: &str) -> String {
    if language.is_empty() {
        String::new()
    } else if region.is_empty() {
        language.to_string()
    } else {
        format!("{language}_{region}")
    }
}

/// The tags to look a translation up by, the most specific first (Compose's private `localeTagChain`).
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

/// The translation of a locale (Compose's private `findTranslation`): the first of its chain there is (each holds
/// all the texts).
fn find_translation(language: &str, region: &str) -> &'static Translation {
    locale_tag_chain(language, region)
        .iter()
        .find_map(|tag| translation_for(tag))
        .expect("the default translation (English's) is always there")
}

/// `string` in a language and region (Compose's private `getTranslation`).
fn translation(string: Strings, language: &str, region: &str) -> String {
    let tag = locale_tag(language, region);
    let translation = TRANSLATION_BY_LOCALE_TAG
        .with(|t| *t.borrow_mut().entry(tag).or_insert_with(|| find_translation(language, region)));
    match translation.get(string.0 as usize).copied().flatten() {
        Some(text) => text.to_string(),
        None => panic!("Missing translation for {string:?}"),
    }
}

/// `string` in the language of `strings` (KompozR's `LocalStrings` value: its locale).
fn translation_in(string: Strings, strings: &UiStrings) -> String {
    translation(string, strings.locale_language(), strings.locale_region())
}

/// `string` in the UI's language (Compose's internal composable `getString(string)`): [`LocalStrings`]' locale.
pub(crate) fn get_string(string: Strings) -> String {
    translation_in(string, &LocalStrings.current())
}

/// `string` in the UI's language, its places filled with `format_args` (Compose's composable `getString(string,
/// vararg formatArgs)`; see [`format_string`]).
pub(crate) fn get_string_formatted(string: Strings, format_args: &[&dyn fmt::Display]) -> String {
    format_string(&get_string(string), format_args)
}

/// `string` in the language of where `node`'s layout was composed (Compose's internal
/// `CompositionLocalConsumerModifierNode.getString(string)`; Compose for desktop's reads `Locale.current`, KompozR's
/// texts follow `LocalStrings` read through the node).
pub(crate) fn get_string_of_node(node: &NodeHandle, string: Strings) -> String {
    translation_in(string, &node.current_value_of(&LocalStrings))
}

/// The same with its places filled (Compose's `CompositionLocalConsumerModifierNode.getString(string, vararg
/// formatArgs)`).
pub(crate) fn get_string_of_node_formatted(
    node: &NodeHandle,
    string: Strings,
    format_args: &[&dyn fmt::Display],
) -> String {
    format_string(&get_string_of_node(node, string), format_args)
}

/// `string`'s places filled with `format_args` (Compose for desktop's internal `String.format`, which fills the
/// numbered `%n$d` and `%n$s` only). Deviation: the adaptive layout's translations write their places unnumbered
/// (`"Current pane split, %s"`, `"%d percent"`), which Compose for desktop's format leaves as they are; they are
/// filled here in order too, as Compose's `String.format` (Android's, Java's) fills them.
pub(crate) fn format_string(string: &str, format_args: &[&dyn fmt::Display]) -> String {
    let mut result = string.to_string();
    for (index, arg) in format_args.iter().enumerate() {
        let arg = arg.to_string();
        result = result.replace(&format!("%{}$d", index + 1), &arg).replace(&format!("%{}$s", index + 1), &arg);
    }
    // The unnumbered places, in order (`%%` is a percent sign).
    let mut out = String::with_capacity(result.len());
    let mut next = 0;
    let mut chars = result.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('%') => {
                chars.next();
                out.push('%');
            }
            Some('s' | 'd') if next < format_args.len() => {
                chars.next();
                out.push_str(&format_args[next].to_string());
                next += 1;
            }
            _ => out.push('%'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_follow_compose_for_desktops_locale_chain() {
        // The region's translation, else the language's, else English (Compose's `localeTagChain`).
        assert_eq!(translation(Strings::defaultPaneTitlePrimary, "tr", "TR"), "Birincil bölme");
        assert_eq!(translation(Strings::defaultPaneTitlePrimary, "xx", ""), "Primary pane");
        assert_eq!(
            translation(Strings::dragToResizeExpandedStateDescription, "pt", "BR"),
            translation(Strings::dragToResizeExpandedStateDescription, "pt", "")
        );
        // (Hebrew and Indonesian by both their codes)
        assert_eq!(
            translation(Strings::defaultPaneTitleSecondary, "he", ""),
            translation(Strings::defaultPaneTitleSecondary, "iw", "")
        );
        assert_eq!(
            translation(Strings::defaultPaneTitleSecondary, "id", ""),
            translation(Strings::defaultPaneTitleSecondary, "in", "")
        );
        // (outside a composition: LocalStrings' default, English)
        assert_eq!(get_string(Strings::defaultPaneTitleTertiary), "Tertiary pane");
    }

    #[test]
    fn formats_fill_numbered_and_unnumbered_places() {
        assert_eq!(format_string("%1$s - %2$s", &[&1990, &"2030"]), "1990 - 2030");
        assert_eq!(format_string("%d percent", &[&50]), "50 percent");
        assert_eq!(format_string("Current pane split, %s", &[&"50 percent"]), "Current pane split, 50 percent");
        assert_eq!(format_string("%d%% of %s", &[&3, &"x"]), "3% of x");
        assert_eq!(
            get_string_formatted(Strings::defaultPaneExpansionStartOffsetAnchorDescription, &[&120]),
            "120 DPs from start"
        );
    }
}

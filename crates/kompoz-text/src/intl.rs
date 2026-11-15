//! Languages and regions (Compose's `androidx.compose.ui.text.intl`: Locale, LocaleList), the current ones the
//! platform says, and locale-aware case mapping (Compose's String.toUpperCase(locale) and its kin).

use icu_locale_core::LanguageIdentifier;
use std::fmt;
use std::sync::{Arc, RwLock};

/// A language, script and region (Compose's Locale), as a BCP 47 language tag says them: `tr`, `en-US`,
/// `zh-Hant-TW`. Equal by its tag.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Locale {
    id: LanguageIdentifier,
}

impl Locale {
    /// The locale of a language tag (Compose's Locale(languageTag)); `_` separates as `-` does, and a tag that
    /// isn't well formed is undetermined (`und`), as Java's forLanguageTag makes it.
    pub fn new(language_tag: &str) -> Locale {
        let tag = language_tag.trim().replace('_', "-");
        let id = LanguageIdentifier::try_from_str(&tag).unwrap_or(LanguageIdentifier::UNKNOWN);
        Locale { id }
    }
    /// The first of the current ones (Compose's Locale.current).
    pub fn current() -> Locale {
        LocaleList::current().get(0).cloned().unwrap_or_else(|| Locale::new("und"))
    }
    /// Its language (`tr`; `und` if undetermined).
    pub fn language(&self) -> &str {
        self.id.language.as_str()
    }
    /// Its script (`Hant`), or empty.
    pub fn script(&self) -> &str {
        self.id.script.as_ref().map_or("", |s| s.as_str())
    }
    /// Its region (`TR`), or empty.
    pub fn region(&self) -> &str {
        self.id.region.as_ref().map_or("", |r| r.as_str())
    }
    /// Its BCP 47 language tag, canonical (Compose's toLanguageTag).
    pub fn to_language_tag(&self) -> String {
        self.id.to_string()
    }
    /// What ICU (segmenters, line breakers) takes.
    pub fn language_identifier(&self) -> &LanguageIdentifier {
        &self.id
    }
}

/// Its language tag.
impl fmt::Debug for Locale {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// Locales in order of preference (Compose's LocaleList).
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct LocaleList {
    locales: Arc<[Locale]>,
}

static CURRENT: RwLock<Option<LocaleList>> = RwLock::new(None);

impl LocaleList {
    /// None (Compose's LocaleList.Empty).
    pub fn empty() -> LocaleList {
        LocaleList::default()
    }
    /// These locales.
    pub fn new(locales: impl IntoIterator<Item = Locale>) -> LocaleList {
        LocaleList { locales: locales.into_iter().collect() }
    }
    /// The locales of comma separated language tags (Compose's LocaleList(languageTags)).
    pub fn from_tags(language_tags: &str) -> LocaleList {
        LocaleList::new(language_tags.split(',').map(|t| Locale::new(t.trim())))
    }
    /// The ones the user prefers, as the platform said them ([`set_current`](LocaleList::set_current)), else the
    /// process's environment's (`LC_ALL`, `LC_MESSAGES`, `LANG`; English if none): Compose's LocaleList.current.
    pub fn current() -> LocaleList {
        if let Some(l) = CURRENT.read().expect("current locales").as_ref() {
            return l.clone();
        }
        LocaleList::new([environment_locale()])
    }
    /// What the user prefers now (a platform tells it when it starts and when the user changes it).
    pub fn set_current(locales: LocaleList) {
        *CURRENT.write().expect("current locales") = Some(locales);
    }
    pub fn get(&self, i: usize) -> Option<&Locale> {
        self.locales.get(i)
    }
    pub fn len(&self) -> usize {
        self.locales.len()
    }
    pub fn is_empty(&self) -> bool {
        self.locales.is_empty()
    }
    pub fn contains(&self, locale: &Locale) -> bool {
        self.locales.contains(locale)
    }
    pub fn iter(&self) -> std::slice::Iter<'_, Locale> {
        self.locales.iter()
    }
    pub fn as_slice(&self) -> &[Locale] {
        &self.locales
    }
    /// Its first, or the current one if it has none (what case mapping goes by, as Compose's).
    pub fn first_or_current(&self) -> Locale {
        self.locales.first().cloned().unwrap_or_else(Locale::current)
    }
}

/// `LocaleList(localeList=[tr-TR, en])`, as Compose writes it.
impl fmt::Debug for LocaleList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LocaleList(localeList={:?})", &*self.locales)
    }
}

impl<'a> IntoIterator for &'a LocaleList {
    type Item = &'a Locale;
    type IntoIter = std::slice::Iter<'a, Locale>;
    fn into_iter(self) -> Self::IntoIter {
        self.locales.iter()
    }
}

/// The environment's locale (a POSIX one: `tr_TR.UTF-8@euro` is `tr-TR`; `C` and `POSIX` are English).
fn environment_locale() -> Locale {
    let value = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty())
        .unwrap_or_default();
    let tag = value.split(['.', '@']).next().unwrap_or("");
    if tag.is_empty() || tag == "C" || tag == "POSIX" {
        return Locale::new("en-US");
    }
    Locale::new(&tag.replace('_', "-"))
}

// ---- Case mapping ---------------------------------------------------------------------------------------------

/// Whether the locale maps case its own way: Turkish and Azeri dotted and dotless i, Lithuanian's dot above.
fn special_casing(locale: &Locale) -> Option<&'static str> {
    match locale.language() {
        "tr" | "az" => Some("tr"),
        "lt" => Some("lt"),
        _ => None,
    }
}

/// Characters whose dot a following accent above replaces (Unicode's Soft_Dotted, the ones Lithuanian's rules
/// meet).
fn soft_dotted(c: char) -> bool {
    matches!(
        c,
        'i' | 'j'
            | '\u{012F}'
            | '\u{0249}'
            | '\u{0268}'
            | '\u{029D}'
            | '\u{02B2}'
            | '\u{03F3}'
            | '\u{0456}'
            | '\u{0458}'
            | '\u{1D62}'
            | '\u{1D96}'
            | '\u{1DA4}'
            | '\u{1DA8}'
            | '\u{1E2D}'
            | '\u{1ECB}'
            | '\u{2071}'
            | '\u{2148}'
            | '\u{2149}'
            | '\u{2C7C}'
    )
}

/// An accent drawn above (combining class 230), as Lithuanian's More_Above looks for.
fn accent_above(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{0314}' | '\u{033D}'..='\u{0344}' | '\u{0346}' | '\u{034A}'..='\u{034C}'
        | '\u{0350}'..='\u{0352}' | '\u{0357}' | '\u{035B}' | '\u{0363}'..='\u{036F}')
}

/// `text` in upper case as `locale`'s language writes it (Compose's String.toUpperCase(locale)).
pub fn to_upper_case(text: &str, locale: &Locale) -> String {
    match special_casing(locale) {
        Some("tr") => text.replace('i', "\u{0130}").to_uppercase(),
        Some("lt") => {
            // A dot above a soft-dotted letter goes (the capital has none).
            let mut out = String::with_capacity(text.len());
            let mut previous = None;
            for c in text.chars() {
                if c == '\u{0307}' && previous.is_some_and(soft_dotted) {
                    continue;
                }
                out.push(c);
                if !accent_above(c) {
                    previous = Some(c);
                }
            }
            out.to_uppercase()
        }
        _ => text.to_uppercase(),
    }
}

/// `text` in lower case as `locale`'s language writes it (Compose's String.toLowerCase(locale)).
pub fn to_lower_case(text: &str, locale: &Locale) -> String {
    match special_casing(locale) {
        Some("tr") => {
            let mut out = String::with_capacity(text.len());
            let mut chars = text.chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    // I with a dot above is i.
                    'I' if chars.peek() == Some(&'\u{0307}') => {
                        chars.next();
                        out.push('i');
                    }
                    'I' => out.push('\u{0131}'),
                    '\u{0130}' => out.push('i'),
                    c => out.push(c),
                }
            }
            out.to_lowercase()
        }
        Some("lt") => {
            // i keeps its dot under a further accent above; Ì Í Ĩ spell theirs out.
            let mut out = String::with_capacity(text.len());
            let mut chars = text.chars().peekable();
            while let Some(c) = chars.next() {
                let more_above = chars.peek().is_some_and(|&n| accent_above(n));
                match c {
                    'I' if more_above => out.push_str("i\u{0307}"),
                    'J' if more_above => out.push_str("j\u{0307}"),
                    '\u{012E}' if more_above => out.push_str("\u{012F}\u{0307}"),
                    '\u{00CC}' => out.push_str("i\u{0307}\u{0300}"),
                    '\u{00CD}' => out.push_str("i\u{0307}\u{0301}"),
                    '\u{0128}' => out.push_str("i\u{0307}\u{0303}"),
                    c => out.push(c),
                }
            }
            out.to_lowercase()
        }
        _ => text.to_lowercase(),
    }
}

/// A character in title case as `locale`'s language writes it (Kotlin's Char.titlecase(locale)): the digraphs'
/// and ligatures' own title forms, else its upper case.
fn title_case(c: char, locale: &Locale) -> String {
    let own = match c {
        '\u{01C4}'..='\u{01C6}' => "\u{01C5}",
        '\u{01C7}'..='\u{01C9}' => "\u{01C8}",
        '\u{01CA}'..='\u{01CC}' => "\u{01CB}",
        '\u{01F1}'..='\u{01F3}' => "\u{01F2}",
        '\u{00DF}' => "Ss",
        '\u{FB00}' => "Ff",
        '\u{FB01}' => "Fi",
        '\u{FB02}' => "Fl",
        '\u{FB03}' => "Ffi",
        '\u{FB04}' => "Ffl",
        '\u{FB05}' | '\u{FB06}' => "St",
        '\u{0587}' => "\u{0535}\u{0582}",
        '\u{FB13}' => "\u{0544}\u{0576}",
        '\u{FB14}' => "\u{0544}\u{0565}",
        '\u{FB15}' => "\u{0544}\u{056B}",
        '\u{FB16}' => "\u{054E}\u{0576}",
        '\u{FB17}' => "\u{0544}\u{056D}",
        _ => "",
    };
    if !own.is_empty() {
        return own.to_string();
    }
    // Greek with a subscript iota: its capital with the iota still under it (U+1F88…), not "ΑΙ".
    let greek = match c {
        '\u{1F80}'..='\u{1F87}' | '\u{1F90}'..='\u{1F97}' | '\u{1FA0}'..='\u{1FA7}' => char::from_u32(c as u32 + 8),
        '\u{1F88}'..='\u{1F8F}' | '\u{1F98}'..='\u{1F9F}' | '\u{1FA8}'..='\u{1FAF}' => Some(c),
        '\u{1FB3}' => Some('\u{1FBC}'),
        '\u{1FC3}' => Some('\u{1FCC}'),
        '\u{1FF3}' => Some('\u{1FFC}'),
        _ => None,
    };
    if let Some(g) = greek {
        return g.to_string();
    }
    to_upper_case(c.encode_utf8(&mut [0; 4]), locale)
}

/// `text` with its first character in title case (Compose's String.capitalize(locale)).
pub fn capitalize(text: &str, locale: &Locale) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => title_case(first, locale) + chars.as_str(),
        None => String::new(),
    }
}

/// `text` with its first character in lower case (Compose's String.decapitalize(locale)).
pub fn decapitalize(text: &str, locale: &Locale) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => to_lower_case(first.encode_utf8(&mut [0; 4]), locale) + chars.as_str(),
        None => String::new(),
    }
}

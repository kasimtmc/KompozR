//! The library's own texts (C++'s `Strings`, Compose's string resources): what its components show (a text field's
//! menu, the title bar's buttons) or tell accessibility services ("Selected"), in the system's language where
//! KompozR has it (English and Turkish; the platform tells the host, `KOMPOZ_LANGUAGE=tr` or `en` decides instead)
//! and in English otherwise; and which languages are written right to left. An app with texts of its own provides
//! them, whole or changed:
//!
//! ```text
//! let mine = Strings { copy: "Duplicate".into(), ..Strings::english() };
//! KompozitionLocalProvider(&[LocalStrings.provides(mine)], content);
//! ```
//!
//! (The pickers' texts come with the Material components that show them.)

use kompoz_runtime::KompozitionLocal;

/// The library's texts in one language.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Strings {
    /// The language they are in (ISO 639).
    pub language: String,
    // A text field's or a selection's menu and toolbar.
    pub cut: String,
    pub copy: String,
    pub paste: String,
    pub select_all: String,
    // For accessibility: what icons stand for and states mean.
    pub selected: String,
    pub checked: String,
    pub partially_checked: String,
    pub more_options: String,
    pub submenu: String,
    /// An exposed dropdown menu's arrow; a window's close button.
    pub open: String,
    pub close: String,
    /// A split button's trailing arrow.
    pub open_menu: String,
    pub close_menu: String,
    /// A snackbar's close button.
    pub dismiss: String,
    /// A bottom sheet's.
    pub drag_handle: String,
    pub pull_to_refresh: String,
    pub refreshing: String,
    /// The title bar KompozR draws where the desktop leaves a window undecorated (its close button: `close`).
    pub minimize: String,
    pub maximize: String,
    pub restore: String,
    /// A time picker's.
    pub am: String,
    pub pm: String,
    pub hour: String,
    pub minute: String,
    /// A tooltip anchor's long-click action, and a tooltip's pane title (Compose's BasicTooltipStrings).
    pub tooltip_label: String,
    pub tooltip_description: String,
}

impl Strings {
    /// English's.
    pub fn english() -> Strings {
        let s = |t: &str| t.to_string();
        Strings {
            language: s("en"),
            cut: s("Cut"),
            copy: s("Copy"),
            paste: s("Paste"),
            select_all: s("Select all"),
            selected: s("Selected"),
            checked: s("Checked"),
            partially_checked: s("Partially checked"),
            more_options: s("More options"),
            submenu: s("Submenu"),
            open: s("Open"),
            close: s("Close"),
            open_menu: s("Open menu"),
            close_menu: s("Close menu"),
            dismiss: s("Dismiss"),
            drag_handle: s("Drag handle"),
            pull_to_refresh: s("Pull to refresh"),
            refreshing: s("Refreshing"),
            minimize: s("Minimize"),
            maximize: s("Maximize"),
            restore: s("Restore"),
            am: s("AM"),
            pm: s("PM"),
            hour: s("Hour"),
            minute: s("Minute"),
            tooltip_label: s("show tooltip"),
            tooltip_description: s("tooltip"),
        }
    }

    /// Turkish's.
    pub fn turkish() -> Strings {
        let s = |t: &str| t.to_string();
        Strings {
            language: s("tr"),
            cut: s("Kes"),
            copy: s("Kopyala"),
            paste: s("Yapıştır"),
            select_all: s("Tümünü seç"),
            selected: s("Seçili"),
            checked: s("İşaretli"),
            partially_checked: s("Kısmen işaretli"),
            more_options: s("Diğer seçenekler"),
            submenu: s("Alt menü"),
            open: s("Aç"),
            close: s("Kapat"),
            open_menu: s("Menüyü aç"),
            close_menu: s("Menüyü kapat"),
            dismiss: s("Kapat"),
            drag_handle: s("Sürükleme tutamacı"),
            pull_to_refresh: s("Yenilemek için çekin"),
            refreshing: s("Yenileniyor"),
            minimize: s("Simge durumuna küçült"),
            maximize: s("Ekranı kapla"),
            restore: s("Önceki boyutuna getir"),
            am: s("ÖÖ"),
            pm: s("ÖS"),
            hour: s("Saat"),
            minute: s("Dakika"),
            tooltip_label: s("ipucunu göster"),
            tooltip_description: s("ipucu"),
        }
    }

    /// A language as the platform names it ("tr", "tr-TR", "tr_TR.UTF-8"): Turkish's, else English's.
    pub fn for_language(language: &str) -> Strings {
        let b = language.as_bytes();
        let turkish = b.len() >= 2
            && b[0].eq_ignore_ascii_case(&b't')
            && b[1].eq_ignore_ascii_case(&b'r')
            && (b.len() == 2 || !b[2].is_ascii_alphabetic());
        if turkish { Strings::turkish() } else { Strings::english() }
    }
}

impl Default for Strings {
    /// English's.
    fn default() -> Self {
        Strings::english()
    }
}

/// Whether a language ("ar", "fa-IR", "he_IL.UTF-8") is written right to left (C++'s `isRtlLanguage`): Arabic,
/// Central Kurdish, Dhivehi, Persian, Hebrew (and its old code), Kashmiri, Pashto, Sindhi, Uyghur, Urdu, Yiddish.
pub fn is_rtl_language(tag: &str) -> bool {
    let language: String = tag.chars().take_while(|&c| c != '-' && c != '_').map(|c| c.to_ascii_lowercase()).collect();
    const RTL: [&str; 12] = ["ar", "ckb", "dv", "fa", "he", "iw", "ks", "ps", "sd", "ug", "ur", "yi"];
    RTL.contains(&language.as_str())
}

/// The library's texts (C++'s LocalStrings): the host's, in the system's language.
pub static LocalStrings: KompozitionLocal<Strings> = KompozitionLocal::new(Strings::english);

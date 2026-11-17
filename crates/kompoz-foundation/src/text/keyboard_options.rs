//! The software keyboard a text field asks for (Compose's KeyboardOptions): its capitalization, autocorrect, kind,
//! action key, platform options, whether it shows when the field gets the focus, the languages hinted; made into
//! the input method's [`ImeOptions`] with the field's single-lineness.

use kompoz_runtime::named;
use kompoz_text::LocaleList;
use kompoz_ui::platform::{ImeAction, ImeOptions, KeyboardCapitalization, KeyboardType, PlatformImeOptions};

/// A field's keyboard options (Compose's KeyboardOptions): what is unspecified (`Unspecified`, `None`) falls back
/// to a default when made into [`ImeOptions`] (no capitalization, autocorrect on, a text keyboard, the default
/// action, shown on focus, no hinted language), or to another's in [`merge`](KeyboardOptions::merge).
#[derive(Clone, PartialEq, Debug)]
pub struct KeyboardOptions {
    pub capitalization: KeyboardCapitalization,
    pub auto_correct_enabled: Option<bool>,
    pub keyboard_type: KeyboardType,
    pub ime_action: ImeAction,
    pub platform_ime_options: Option<PlatformImeOptions>,
    pub show_keyboard_on_focus: Option<bool>,
    pub hint_locales: Option<LocaleList>,
}

impl Default for KeyboardOptions {
    fn default() -> Self {
        KeyboardOptions::Default
    }
}

impl KeyboardOptions {
    /// Everything unspecified (Compose's `KeyboardOptions.Default`).
    pub const Default: KeyboardOptions = KeyboardOptions {
        capitalization: KeyboardCapitalization::Unspecified,
        auto_correct_enabled: None,
        keyboard_type: KeyboardType::Unspecified,
        ime_action: ImeAction::Unspecified,
        platform_ime_options: None,
        show_keyboard_on_focus: None,
        hint_locales: None,
    };

    /// A secure field's: no autocorrect, a password keyboard (Compose's `KeyboardOptions.SecureTextField`).
    pub const SecureTextField: KeyboardOptions = KeyboardOptions {
        auto_correct_enabled: Some(false),
        keyboard_type: KeyboardType::Password,
        ..KeyboardOptions::Default
    };

    /// Autocorrect, on unless said otherwise (Compose's `autoCorrect`).
    pub fn auto_correct(&self) -> bool {
        self.auto_correct_enabled.unwrap_or(true)
    }

    fn capitalization_or_default(&self) -> KeyboardCapitalization {
        match self.capitalization {
            KeyboardCapitalization::Unspecified => KeyboardCapitalization::None,
            c => c,
        }
    }

    fn keyboard_type_or_default(&self) -> KeyboardType {
        match self.keyboard_type {
            KeyboardType::Unspecified => KeyboardType::Text,
            t => t,
        }
    }

    /// The action key, the default one unless said (Compose's `imeActionOrDefault`).
    pub fn ime_action_or_default(&self) -> ImeAction {
        match self.ime_action {
            ImeAction::Unspecified => ImeAction::Default,
            a => a,
        }
    }

    /// Whether the keyboard shows when the field gets the focus, yes unless said (Compose's
    /// `showKeyboardOnFocusOrDefault`).
    pub fn show_keyboard_on_focus_or_default(&self) -> bool {
        self.show_keyboard_on_focus.unwrap_or(true)
    }

    /// Whether nothing is said.
    pub fn is_completely_unspecified(&self) -> bool {
        *self == KeyboardOptions::Default
    }

    /// The input method's options for a field that is `single_line` or not (Compose's `toImeOptions`): the
    /// defaults where nothing is said.
    pub fn to_ime_options(&self, single_line: bool) -> ImeOptions {
        ImeOptions {
            single_line,
            capitalization: self.capitalization_or_default(),
            auto_correct: self.auto_correct(),
            keyboard_type: self.keyboard_type_or_default(),
            ime_action: self.ime_action_or_default(),
            platform_ime_options: self.platform_ime_options.clone(),
            hint_locales: self.hint_locales.clone().unwrap_or_default(),
            show_keyboard: self.show_keyboard_on_focus_or_default(),
        }
    }

    /// `other`'s options, these where `other` says nothing (Compose's `merge`:
    /// `other.fillUnspecifiedValuesWith(this)`).
    pub fn merge(&self, other: Option<&KeyboardOptions>) -> KeyboardOptions {
        match other {
            Some(other) => other.fill_unspecified_values_with(Some(self)),
            None => self.clone(),
        }
    }

    /// These options with `other`'s where these say nothing (Compose's `fillUnspecifiedValuesWith`).
    pub fn fill_unspecified_values_with(&self, other: Option<&KeyboardOptions>) -> KeyboardOptions {
        let Some(other) = other else { return self.clone() };
        if other.is_completely_unspecified() || other == self {
            return self.clone();
        }
        if self.is_completely_unspecified() {
            return other.clone();
        }
        KeyboardOptions {
            capitalization: match self.capitalization {
                KeyboardCapitalization::Unspecified => other.capitalization,
                c => c,
            },
            auto_correct_enabled: self.auto_correct_enabled.or(other.auto_correct_enabled),
            keyboard_type: match self.keyboard_type {
                KeyboardType::Unspecified => other.keyboard_type,
                t => t,
            },
            ime_action: match self.ime_action {
                ImeAction::Unspecified => other.ime_action,
                a => a,
            },
            platform_ime_options: self.platform_ime_options.clone().or_else(|| other.platform_ime_options.clone()),
            show_keyboard_on_focus: self.show_keyboard_on_focus.or(other.show_keyboard_on_focus),
            hint_locales: self.hint_locales.clone().or_else(|| other.hint_locales.clone()),
        }
    }
}

#[named]
impl KeyboardOptions {
    /// The same with what is given changed (Compose's `copy`; as Compose's, `show_keyboard_on_focus` and
    /// `hint_locales` are unspecified unless given).
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.capitalization)] capitalization: KeyboardCapitalization,
        #[default(self.auto_correct_enabled)] auto_correct_enabled: Option<bool>,
        #[default(self.keyboard_type)] keyboard_type: KeyboardType,
        #[default(self.ime_action)] ime_action: ImeAction,
        #[default(self.platform_ime_options.clone())] platform_ime_options: Option<PlatformImeOptions>,
        #[default(None)] show_keyboard_on_focus: Option<bool>,
        #[default(None)] hint_locales: Option<LocaleList>,
    ) -> KeyboardOptions {
        KeyboardOptions {
            capitalization,
            auto_correct_enabled,
            keyboard_type,
            ime_action,
            platform_ime_options,
            show_keyboard_on_focus,
            hint_locales,
        }
    }
}

/// Compose's `KeyboardOptions(capitalization = …, autoCorrectEnabled = …, keyboardType = …, imeAction = …,
/// platformImeOptions = …, showKeyboardOnFocus = …, hintLocales = …)`, everything unspecified by default.
#[named(type)]
pub fn KeyboardOptions(
    #[default(KeyboardCapitalization::Unspecified)] capitalization: KeyboardCapitalization,
    #[default(None)] auto_correct_enabled: Option<bool>,
    #[default(KeyboardType::Unspecified)] keyboard_type: KeyboardType,
    #[default(ImeAction::Unspecified)] ime_action: ImeAction,
    #[default(None)] platform_ime_options: Option<PlatformImeOptions>,
    #[default(None)] show_keyboard_on_focus: Option<bool>,
    #[default(None)] hint_locales: Option<LocaleList>,
) -> KeyboardOptions {
    KeyboardOptions {
        capitalization,
        auto_correct_enabled,
        keyboard_type,
        ime_action,
        platform_ime_options,
        show_keyboard_on_focus,
        hint_locales,
    }
}

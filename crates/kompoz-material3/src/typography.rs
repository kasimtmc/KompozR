//! The type scale, as Compose's Typography.kt: a theme's text styles ([`Typography`]) — display, headline, title,
//! body and label in three sizes, and Material Expressive's emphasized ones — of Material's tokens unless given,
//! in a font family of one's own if asked ([`Typography::with_font_family`]).

use crate::material_theme::MaterialTheme;
use crate::tokens::{TypographyKeyTokens, TypographyTokens};
use kompoz_runtime::{KotlinProperties, named};
use kompoz_text::{FontFamily, TextStyle};
use std::fmt;
use std::sync::OnceLock;

/// A theme's text styles (Compose's Typography): what Material's components write with (a button's label in
/// `label_large`, a dialog's title in `headline_small`, `body_large` what `MaterialTheme` gives text by default).
/// Equal by its styles.
#[derive(Clone, PartialEq, KotlinProperties)]
pub struct Typography {
    pub display_large: TextStyle,
    pub display_medium: TextStyle,
    pub display_small: TextStyle,
    pub headline_large: TextStyle,
    pub headline_medium: TextStyle,
    pub headline_small: TextStyle,
    pub title_large: TextStyle,
    pub title_medium: TextStyle,
    pub title_small: TextStyle,
    pub body_large: TextStyle,
    pub body_medium: TextStyle,
    pub body_small: TextStyle,
    pub label_large: TextStyle,
    pub label_medium: TextStyle,
    pub label_small: TextStyle,
    pub display_large_emphasized: TextStyle,
    pub display_medium_emphasized: TextStyle,
    pub display_small_emphasized: TextStyle,
    pub headline_large_emphasized: TextStyle,
    pub headline_medium_emphasized: TextStyle,
    pub headline_small_emphasized: TextStyle,
    pub title_large_emphasized: TextStyle,
    pub title_medium_emphasized: TextStyle,
    pub title_small_emphasized: TextStyle,
    pub body_large_emphasized: TextStyle,
    pub body_medium_emphasized: TextStyle,
    pub body_small_emphasized: TextStyle,
    pub label_large_emphasized: TextStyle,
    pub label_medium_emphasized: TextStyle,
    pub label_small_emphasized: TextStyle,
}

/// The baseline type scale's tokens (Compose's `typographyTokens`), made once.
fn typography_tokens() -> &'static TypographyTokens {
    static TOKENS: OnceLock<TypographyTokens> = OnceLock::new();
    TOKENS.get_or_init(|| TypographyTokens::new(None))
}

/// The type scale, each style Material's unless given (Compose's `Typography(…)`).
#[named(type)]
#[allow(clippy::too_many_arguments)]
pub fn Typography(
    #[default(typography_tokens().DisplayLarge())] display_large: TextStyle,
    #[default(typography_tokens().DisplayMedium())] display_medium: TextStyle,
    #[default(typography_tokens().DisplaySmall())] display_small: TextStyle,
    #[default(typography_tokens().HeadlineLarge())] headline_large: TextStyle,
    #[default(typography_tokens().HeadlineMedium())] headline_medium: TextStyle,
    #[default(typography_tokens().HeadlineSmall())] headline_small: TextStyle,
    #[default(typography_tokens().TitleLarge())] title_large: TextStyle,
    #[default(typography_tokens().TitleMedium())] title_medium: TextStyle,
    #[default(typography_tokens().TitleSmall())] title_small: TextStyle,
    #[default(typography_tokens().BodyLarge())] body_large: TextStyle,
    #[default(typography_tokens().BodyMedium())] body_medium: TextStyle,
    #[default(typography_tokens().BodySmall())] body_small: TextStyle,
    #[default(typography_tokens().LabelLarge())] label_large: TextStyle,
    #[default(typography_tokens().LabelMedium())] label_medium: TextStyle,
    #[default(typography_tokens().LabelSmall())] label_small: TextStyle,
    #[default(typography_tokens().DisplayLargeEmphasized())] display_large_emphasized: TextStyle,
    #[default(typography_tokens().DisplayMediumEmphasized())] display_medium_emphasized: TextStyle,
    #[default(typography_tokens().DisplaySmallEmphasized())] display_small_emphasized: TextStyle,
    #[default(typography_tokens().HeadlineLargeEmphasized())] headline_large_emphasized: TextStyle,
    #[default(typography_tokens().HeadlineMediumEmphasized())] headline_medium_emphasized: TextStyle,
    #[default(typography_tokens().HeadlineSmallEmphasized())] headline_small_emphasized: TextStyle,
    #[default(typography_tokens().TitleLargeEmphasized())] title_large_emphasized: TextStyle,
    #[default(typography_tokens().TitleMediumEmphasized())] title_medium_emphasized: TextStyle,
    #[default(typography_tokens().TitleSmallEmphasized())] title_small_emphasized: TextStyle,
    #[default(typography_tokens().BodyLargeEmphasized())] body_large_emphasized: TextStyle,
    #[default(typography_tokens().BodyMediumEmphasized())] body_medium_emphasized: TextStyle,
    #[default(typography_tokens().BodySmallEmphasized())] body_small_emphasized: TextStyle,
    #[default(typography_tokens().LabelLargeEmphasized())] label_large_emphasized: TextStyle,
    #[default(typography_tokens().LabelMediumEmphasized())] label_medium_emphasized: TextStyle,
    #[default(typography_tokens().LabelSmallEmphasized())] label_small_emphasized: TextStyle,
) -> Typography {
    Typography {
        display_large,
        display_medium,
        display_small,
        headline_large,
        headline_medium,
        headline_small,
        title_large,
        title_medium,
        title_small,
        body_large,
        body_medium,
        body_small,
        label_large,
        label_medium,
        label_small,
        display_large_emphasized,
        display_medium_emphasized,
        display_small_emphasized,
        headline_large_emphasized,
        headline_medium_emphasized,
        headline_small_emphasized,
        title_large_emphasized,
        title_medium_emphasized,
        title_small_emphasized,
        body_large_emphasized,
        body_medium_emphasized,
        body_small_emphasized,
        label_large_emphasized,
        label_medium_emphasized,
        label_small_emphasized,
    }
}

impl Default for Typography {
    /// Material's ([`Typography()`](fn@Typography) of nothing given).
    fn default() -> Typography {
        named_default()
    }
}

fn named_default() -> Typography {
    let t = typography_tokens();
    Typography {
        display_large: t.DisplayLarge(),
        display_medium: t.DisplayMedium(),
        display_small: t.DisplaySmall(),
        headline_large: t.HeadlineLarge(),
        headline_medium: t.HeadlineMedium(),
        headline_small: t.HeadlineSmall(),
        title_large: t.TitleLarge(),
        title_medium: t.TitleMedium(),
        title_small: t.TitleSmall(),
        body_large: t.BodyLarge(),
        body_medium: t.BodyMedium(),
        body_small: t.BodySmall(),
        label_large: t.LabelLarge(),
        label_medium: t.LabelMedium(),
        label_small: t.LabelSmall(),
        display_large_emphasized: t.DisplayLargeEmphasized(),
        display_medium_emphasized: t.DisplayMediumEmphasized(),
        display_small_emphasized: t.DisplaySmallEmphasized(),
        headline_large_emphasized: t.HeadlineLargeEmphasized(),
        headline_medium_emphasized: t.HeadlineMediumEmphasized(),
        headline_small_emphasized: t.HeadlineSmallEmphasized(),
        title_large_emphasized: t.TitleLargeEmphasized(),
        title_medium_emphasized: t.TitleMediumEmphasized(),
        title_small_emphasized: t.TitleSmallEmphasized(),
        body_large_emphasized: t.BodyLargeEmphasized(),
        body_medium_emphasized: t.BodyMediumEmphasized(),
        body_small_emphasized: t.BodySmallEmphasized(),
        label_large_emphasized: t.LabelLargeEmphasized(),
        label_medium_emphasized: t.LabelMediumEmphasized(),
        label_small_emphasized: t.LabelSmallEmphasized(),
    }
}

/// The type scale in `font_family`, each style Material's in it unless given (Compose's
/// `Typography(fontFamily, displayLarge = null, …)`).
///
/// Kotlin: `Typography(fontFamily: FontFamily)`
#[named]
#[allow(clippy::too_many_arguments)]
pub fn typography_with_font_family(
    font_family: FontFamily,
    #[default(None)] display_large: Option<TextStyle>,
    #[default(None)] display_medium: Option<TextStyle>,
    #[default(None)] display_small: Option<TextStyle>,
    #[default(None)] headline_large: Option<TextStyle>,
    #[default(None)] headline_medium: Option<TextStyle>,
    #[default(None)] headline_small: Option<TextStyle>,
    #[default(None)] title_large: Option<TextStyle>,
    #[default(None)] title_medium: Option<TextStyle>,
    #[default(None)] title_small: Option<TextStyle>,
    #[default(None)] body_large: Option<TextStyle>,
    #[default(None)] body_medium: Option<TextStyle>,
    #[default(None)] body_small: Option<TextStyle>,
    #[default(None)] label_large: Option<TextStyle>,
    #[default(None)] label_medium: Option<TextStyle>,
    #[default(None)] label_small: Option<TextStyle>,
    #[default(None)] display_large_emphasized: Option<TextStyle>,
    #[default(None)] display_medium_emphasized: Option<TextStyle>,
    #[default(None)] display_small_emphasized: Option<TextStyle>,
    #[default(None)] headline_large_emphasized: Option<TextStyle>,
    #[default(None)] headline_medium_emphasized: Option<TextStyle>,
    #[default(None)] headline_small_emphasized: Option<TextStyle>,
    #[default(None)] title_large_emphasized: Option<TextStyle>,
    #[default(None)] title_medium_emphasized: Option<TextStyle>,
    #[default(None)] title_small_emphasized: Option<TextStyle>,
    #[default(None)] body_large_emphasized: Option<TextStyle>,
    #[default(None)] body_medium_emphasized: Option<TextStyle>,
    #[default(None)] body_small_emphasized: Option<TextStyle>,
    #[default(None)] label_large_emphasized: Option<TextStyle>,
    #[default(None)] label_medium_emphasized: Option<TextStyle>,
    #[default(None)] label_small_emphasized: Option<TextStyle>,
) -> Typography {
    let tokens = TypographyTokens::new(Some(font_family));
    Typography {
        display_large: display_large.unwrap_or_else(|| tokens.DisplayLarge()),
        display_medium: display_medium.unwrap_or_else(|| tokens.DisplayMedium()),
        display_small: display_small.unwrap_or_else(|| tokens.DisplaySmall()),
        headline_large: headline_large.unwrap_or_else(|| tokens.HeadlineLarge()),
        headline_medium: headline_medium.unwrap_or_else(|| tokens.HeadlineMedium()),
        headline_small: headline_small.unwrap_or_else(|| tokens.HeadlineSmall()),
        title_large: title_large.unwrap_or_else(|| tokens.TitleLarge()),
        title_medium: title_medium.unwrap_or_else(|| tokens.TitleMedium()),
        title_small: title_small.unwrap_or_else(|| tokens.TitleSmall()),
        body_large: body_large.unwrap_or_else(|| tokens.BodyLarge()),
        body_medium: body_medium.unwrap_or_else(|| tokens.BodyMedium()),
        body_small: body_small.unwrap_or_else(|| tokens.BodySmall()),
        label_large: label_large.unwrap_or_else(|| tokens.LabelLarge()),
        label_medium: label_medium.unwrap_or_else(|| tokens.LabelMedium()),
        label_small: label_small.unwrap_or_else(|| tokens.LabelSmall()),
        display_large_emphasized: display_large_emphasized.unwrap_or_else(|| tokens.DisplayLargeEmphasized()),
        display_medium_emphasized: display_medium_emphasized.unwrap_or_else(|| tokens.DisplayMediumEmphasized()),
        display_small_emphasized: display_small_emphasized.unwrap_or_else(|| tokens.DisplaySmallEmphasized()),
        headline_large_emphasized: headline_large_emphasized.unwrap_or_else(|| tokens.HeadlineLargeEmphasized()),
        headline_medium_emphasized: headline_medium_emphasized.unwrap_or_else(|| tokens.HeadlineMediumEmphasized()),
        headline_small_emphasized: headline_small_emphasized.unwrap_or_else(|| tokens.HeadlineSmallEmphasized()),
        title_large_emphasized: title_large_emphasized.unwrap_or_else(|| tokens.TitleLargeEmphasized()),
        title_medium_emphasized: title_medium_emphasized.unwrap_or_else(|| tokens.TitleMediumEmphasized()),
        title_small_emphasized: title_small_emphasized.unwrap_or_else(|| tokens.TitleSmallEmphasized()),
        body_large_emphasized: body_large_emphasized.unwrap_or_else(|| tokens.BodyLargeEmphasized()),
        body_medium_emphasized: body_medium_emphasized.unwrap_or_else(|| tokens.BodyMediumEmphasized()),
        body_small_emphasized: body_small_emphasized.unwrap_or_else(|| tokens.BodySmallEmphasized()),
        label_large_emphasized: label_large_emphasized.unwrap_or_else(|| tokens.LabelLargeEmphasized()),
        label_medium_emphasized: label_medium_emphasized.unwrap_or_else(|| tokens.LabelMediumEmphasized()),
        label_small_emphasized: label_small_emphasized.unwrap_or_else(|| tokens.LabelSmallEmphasized()),
    }
}

#[named]
impl Typography {
    /// The same with what is given replaced (Compose's copy).
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.display_large.clone())] display_large: TextStyle,
        #[default(self.display_medium.clone())] display_medium: TextStyle,
        #[default(self.display_small.clone())] display_small: TextStyle,
        #[default(self.headline_large.clone())] headline_large: TextStyle,
        #[default(self.headline_medium.clone())] headline_medium: TextStyle,
        #[default(self.headline_small.clone())] headline_small: TextStyle,
        #[default(self.title_large.clone())] title_large: TextStyle,
        #[default(self.title_medium.clone())] title_medium: TextStyle,
        #[default(self.title_small.clone())] title_small: TextStyle,
        #[default(self.body_large.clone())] body_large: TextStyle,
        #[default(self.body_medium.clone())] body_medium: TextStyle,
        #[default(self.body_small.clone())] body_small: TextStyle,
        #[default(self.label_large.clone())] label_large: TextStyle,
        #[default(self.label_medium.clone())] label_medium: TextStyle,
        #[default(self.label_small.clone())] label_small: TextStyle,
        #[default(self.display_large_emphasized.clone())] display_large_emphasized: TextStyle,
        #[default(self.display_medium_emphasized.clone())] display_medium_emphasized: TextStyle,
        #[default(self.display_small_emphasized.clone())] display_small_emphasized: TextStyle,
        #[default(self.headline_large_emphasized.clone())] headline_large_emphasized: TextStyle,
        #[default(self.headline_medium_emphasized.clone())] headline_medium_emphasized: TextStyle,
        #[default(self.headline_small_emphasized.clone())] headline_small_emphasized: TextStyle,
        #[default(self.title_large_emphasized.clone())] title_large_emphasized: TextStyle,
        #[default(self.title_medium_emphasized.clone())] title_medium_emphasized: TextStyle,
        #[default(self.title_small_emphasized.clone())] title_small_emphasized: TextStyle,
        #[default(self.body_large_emphasized.clone())] body_large_emphasized: TextStyle,
        #[default(self.body_medium_emphasized.clone())] body_medium_emphasized: TextStyle,
        #[default(self.body_small_emphasized.clone())] body_small_emphasized: TextStyle,
        #[default(self.label_large_emphasized.clone())] label_large_emphasized: TextStyle,
        #[default(self.label_medium_emphasized.clone())] label_medium_emphasized: TextStyle,
        #[default(self.label_small_emphasized.clone())] label_small_emphasized: TextStyle,
    ) -> Typography {
        Typography {
            display_large,
            display_medium,
            display_small,
            headline_large,
            headline_medium,
            headline_small,
            title_large,
            title_medium,
            title_small,
            body_large,
            body_medium,
            body_small,
            label_large,
            label_medium,
            label_small,
            display_large_emphasized,
            display_medium_emphasized,
            display_small_emphasized,
            headline_large_emphasized,
            headline_medium_emphasized,
            headline_small_emphasized,
            title_large_emphasized,
            title_medium_emphasized,
            title_small_emphasized,
            body_large_emphasized,
            body_medium_emphasized,
            body_small_emphasized,
            label_large_emphasized,
            label_medium_emphasized,
            label_small_emphasized,
        }
    }
}

impl Typography {
    /// A token's style in this type scale (Compose's internal `Typography.fromToken`).
    #[allow(dead_code, clippy::wrong_self_convention)] // (Compose's internal `fromToken`; the components', as they come)
    pub(crate) fn from_token(&self, value: TypographyKeyTokens) -> TextStyle {
        match value {
            TypographyKeyTokens::DisplayLarge => self.display_large.clone(),
            TypographyKeyTokens::DisplayMedium => self.display_medium.clone(),
            TypographyKeyTokens::DisplaySmall => self.display_small.clone(),
            TypographyKeyTokens::HeadlineLarge => self.headline_large.clone(),
            TypographyKeyTokens::HeadlineMedium => self.headline_medium.clone(),
            TypographyKeyTokens::HeadlineSmall => self.headline_small.clone(),
            TypographyKeyTokens::TitleLarge => self.title_large.clone(),
            TypographyKeyTokens::TitleMedium => self.title_medium.clone(),
            TypographyKeyTokens::TitleSmall => self.title_small.clone(),
            TypographyKeyTokens::BodyLarge => self.body_large.clone(),
            TypographyKeyTokens::BodyMedium => self.body_medium.clone(),
            TypographyKeyTokens::BodySmall => self.body_small.clone(),
            TypographyKeyTokens::LabelLarge => self.label_large.clone(),
            TypographyKeyTokens::LabelMedium => self.label_medium.clone(),
            TypographyKeyTokens::LabelSmall => self.label_small.clone(),
            TypographyKeyTokens::DisplayLargeEmphasized => self.display_large_emphasized.clone(),
            TypographyKeyTokens::DisplayMediumEmphasized => self.display_medium_emphasized.clone(),
            TypographyKeyTokens::DisplaySmallEmphasized => self.display_small_emphasized.clone(),
            TypographyKeyTokens::HeadlineLargeEmphasized => self.headline_large_emphasized.clone(),
            TypographyKeyTokens::HeadlineMediumEmphasized => self.headline_medium_emphasized.clone(),
            TypographyKeyTokens::HeadlineSmallEmphasized => self.headline_small_emphasized.clone(),
            TypographyKeyTokens::TitleLargeEmphasized => self.title_large_emphasized.clone(),
            TypographyKeyTokens::TitleMediumEmphasized => self.title_medium_emphasized.clone(),
            TypographyKeyTokens::TitleSmallEmphasized => self.title_small_emphasized.clone(),
            TypographyKeyTokens::BodyLargeEmphasized => self.body_large_emphasized.clone(),
            TypographyKeyTokens::BodyMediumEmphasized => self.body_medium_emphasized.clone(),
            TypographyKeyTokens::BodySmallEmphasized => self.body_small_emphasized.clone(),
            TypographyKeyTokens::LabelLargeEmphasized => self.label_large_emphasized.clone(),
            TypographyKeyTokens::LabelMediumEmphasized => self.label_medium_emphasized.clone(),
            TypographyKeyTokens::LabelSmallEmphasized => self.label_small_emphasized.clone(),
        }
    }
}

impl TypographyKeyTokens {
    /// The token's style in the theme's type scale (Compose's internal `TypographyKeyTokens.value`).
    #[allow(dead_code)] // (the components', used as they come)
    pub(crate) fn value(self) -> TextStyle {
        MaterialTheme::typography().from_token(self)
    }
}

/// Compose's toString.
impl fmt::Display for Typography {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Typography(displayLarge={:?}, displayMedium={:?},displaySmall={:?}, headlineLarge={:?}, headlineMedium={:?}, headlineSmall={:?}, titleLarge={:?}, titleMedium={:?}, titleSmall={:?}, bodyLarge={:?}, bodyMedium={:?}, bodySmall={:?}, labelLarge={:?}, labelMedium={:?}, labelSmall={:?}, displayLargeEmphasized={:?}, displayMediumEmphasized={:?}, displaySmallEmphasized={:?}, headlineLargeEmphasized={:?}, headlineMediumEmphasized={:?}, headlineSmallEmphasized={:?}, titleLargeEmphasized={:?}, titleMediumEmphasized={:?}, titleSmallEmphasized={:?}, bodyLargeEmphasized={:?}, bodyMediumEmphasized={:?}, bodySmallEmphasized={:?}, labelLargeEmphasized={:?}, labelMediumEmphasized={:?}, labelSmallEmphasized={:?})",
            self.display_large,
            self.display_medium,
            self.display_small,
            self.headline_large,
            self.headline_medium,
            self.headline_small,
            self.title_large,
            self.title_medium,
            self.title_small,
            self.body_large,
            self.body_medium,
            self.body_small,
            self.label_large,
            self.label_medium,
            self.label_small,
            self.display_large_emphasized,
            self.display_medium_emphasized,
            self.display_small_emphasized,
            self.headline_large_emphasized,
            self.headline_medium_emphasized,
            self.headline_small_emphasized,
            self.title_large_emphasized,
            self.title_medium_emphasized,
            self.title_small_emphasized,
            self.body_large_emphasized,
            self.body_medium_emphasized,
            self.body_small_emphasized,
            self.label_large_emphasized,
            self.label_medium_emphasized,
            self.label_small_emphasized
        )
    }
}

impl fmt::Debug for Typography {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

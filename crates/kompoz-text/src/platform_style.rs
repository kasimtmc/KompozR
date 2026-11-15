//! What a text style carries for the platform's text (Compose's PlatformTextStyle, PlatformSpanStyle and
//! PlatformParagraphStyle, as Android's).

use crate::lerp_discrete;

/// Whether a paragraph's emoji are drawn in the color emoji font even where a font of the text has them (Compose's
/// EmojiSupportMatch, Android's EmojiCompat): the default (emoji presentation prefers the color font), never
/// (each character in the first font that has it), or always.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum EmojiSupportMatch {
    #[default]
    Default,
    None,
    All,
}

/// A paragraph's platform settings (Compose's PlatformParagraphStyle): whether its first line's top and last
/// line's bottom are padded to the fonts' tallest glyphs (`include_font_padding`, Android's; off by default as
/// Compose's), and how emoji take the color font.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct PlatformParagraphStyle {
    pub include_font_padding: bool,
    pub emoji_support_match: EmojiSupportMatch,
}

impl PlatformParagraphStyle {
    /// Compose's PlatformParagraphStyle.Default.
    pub const Default: PlatformParagraphStyle =
        PlatformParagraphStyle { include_font_padding: false, emoji_support_match: EmojiSupportMatch::Default };

    /// `other` if given (Compose's merge: the later one wins).
    pub fn merge(&self, other: Option<&PlatformParagraphStyle>) -> PlatformParagraphStyle {
        other.copied().unwrap_or(*self)
    }
}

/// Between two (Compose's lerp: the nearer one's settings).
pub fn lerp_platform_paragraph_style(
    start: &PlatformParagraphStyle,
    stop: &PlatformParagraphStyle,
    fraction: f32,
) -> PlatformParagraphStyle {
    if start.include_font_padding == stop.include_font_padding {
        return *start;
    }
    PlatformParagraphStyle {
        include_font_padding: lerp_discrete(start.include_font_padding, stop.include_font_padding, fraction),
        emoji_support_match: lerp_discrete(start.emoji_support_match, stop.emoji_support_match, fraction),
    }
}

/// A span's platform settings (Compose's PlatformSpanStyle): none (Android's has none either).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct PlatformSpanStyle;

impl PlatformSpanStyle {
    /// Compose's PlatformSpanStyle.Default.
    pub const Default: PlatformSpanStyle = PlatformSpanStyle;

    /// This (Compose's merge keeps the receiver).
    pub fn merge(&self, _other: Option<&PlatformSpanStyle>) -> PlatformSpanStyle {
        *self
    }
}

/// A text style's platform settings (Compose's PlatformTextStyle): its span's and its paragraph's.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct PlatformTextStyle {
    pub span_style: Option<PlatformSpanStyle>,
    pub paragraph_style: Option<PlatformParagraphStyle>,
}

impl PlatformTextStyle {
    pub const fn new(
        span_style: Option<PlatformSpanStyle>,
        paragraph_style: Option<PlatformParagraphStyle>,
    ) -> PlatformTextStyle {
        PlatformTextStyle { span_style, paragraph_style }
    }
    /// Compose's PlatformTextStyle(includeFontPadding).
    pub fn with_include_font_padding(include_font_padding: bool) -> PlatformTextStyle {
        let p = PlatformParagraphStyle { include_font_padding, ..PlatformParagraphStyle::Default };
        PlatformTextStyle::new(None, Some(p))
    }
    /// Compose's PlatformTextStyle(emojiSupportMatch).
    pub fn with_emoji_support_match(emoji_support_match: EmojiSupportMatch) -> PlatformTextStyle {
        let p = PlatformParagraphStyle { emoji_support_match, ..PlatformParagraphStyle::Default };
        PlatformTextStyle::new(None, Some(p))
    }

    /// Of a span's and a paragraph's (none of either: none, as Compose's createPlatformTextStyleInternal).
    pub fn of(
        span_style: Option<PlatformSpanStyle>,
        paragraph_style: Option<PlatformParagraphStyle>,
    ) -> Option<PlatformTextStyle> {
        (span_style.is_some() || paragraph_style.is_some())
            .then_some(PlatformTextStyle::new(span_style, paragraph_style))
    }
}

//! KompozR's text, as Compose's `ui-text`: text styles (TextStyle, SpanStyle, ParagraphStyle and what they hold),
//! AnnotatedString with its styles, annotations and links and its HTML reader, fonts and families and what
//! resolves them, locales, a text field's value, and the contracts the text pipeline is built on — what laid out
//! text is drawn as ([`TextDraw`], glyph runs naming fonts in the [`FontRegistry`]) and the font backend at its
//! bottom ([`FontBackend`]: [`HarfrustBackend`] shaping with harfrust in the fonts of a [`FontCollection`] — the
//! system's or registered ones — that [`FontFamilyResolverImpl`] resolves families to, and [`FakeBackend`] measuring
//! as the C++ Kompoz's tests did), and text segmentation ([`segment`]: grapheme clusters, words, sentences, emoji).
//!
//! Offsets into text are UTF-8 bytes, always on character boundaries (a [`TextRange`] slices its text directly);
//! platform bridges convert to UTF-16 or characters at their edge. Lengths are px; what an app writes is in sp and
//! em ([`TextUnit`](kompoz_graphics::TextUnit)), turned into px by a [`Density`](kompoz_graphics::Density).
//!
//! [`FontRegistry`]: font::FontRegistry

#![allow(non_upper_case_globals, non_snake_case)]

pub mod annotated_string;
pub mod draw;
pub mod font;
mod html;
pub mod input;
pub mod intl;
pub mod link;
pub mod paragraph_style;
pub mod placeholder;
pub mod platform_style;
pub mod range;
mod savers;
pub mod segment;
pub mod shape;
pub mod span_style;
pub mod style;
pub mod text_style;

pub use annotated_string::{
    AnnotatedString, AnnotatedStringBuilder, Annotation, BuilderAppend, BuilderStyle, INLINE_CONTENT_TAG, Range,
    build_annotated_string,
};
pub use draw::{Glyph, GlyphColorKind, GlyphRun, GlyphSynthesis, TextDraw, TextRunStyle};
pub use font::{
    DeviceFontFamilyName, Font, FontCollection, FontFamily, FontFamilyResolver, FontFamilyResolverImpl, FontFeature,
    FontId, FontListFontFamily, FontLoadingStrategy, FontRegistry, FontSource, FontStyle, FontSynthesis, FontVariation,
    FontVariationSetting, FontVariationSettings, FontWeight, GenericFontFamily, Typeface, lerp_font_weight,
};
pub use input::TextFieldValue;
pub use intl::{Locale, LocaleList};
pub use link::{LinkAnnotation, LinkInteractionListener, TextLinkStyles, TtsAnnotation, UrlAnnotation};
pub use paragraph_style::{ParagraphStyle, lerp_paragraph_style};
pub use placeholder::{Placeholder, PlaceholderVerticalAlign};
pub use platform_style::{EmojiSupportMatch, PlatformParagraphStyle, PlatformSpanStyle, PlatformTextStyle};
pub use range::TextRange;
pub use shape::{FakeBackend, FakeFontFamilyResolver, FontBackend, FontMetrics, HarfrustBackend};
pub use span_style::{SpanStyle, TextForegroundStyle, lerp_span_style};
pub use style::{
    BaselineShift, Hyphens, LineBreak, LineHeightStyle, ResolvedTextDirection, TextAlign, TextDecoration,
    TextDirection, TextGeometricTransform, TextIndent, TextMotion, TextOverflow, resolve_text_direction,
};
pub use text_style::{TextStyle, lerp_text_style};

/// The nearer of two values: `a` below a half, `b` from it (Compose's lerpDiscrete).
pub(crate) fn lerp_discrete<T>(a: T, b: T, fraction: f32) -> T {
    if fraction < 0.5 { a } else { b }
}

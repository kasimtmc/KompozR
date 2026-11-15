//! Fonts (Compose's `androidx.compose.ui.text.font`): families, fonts and where they come from, weights, styles,
//! synthesis, variable fonts' settings, OpenType feature settings, CSS's font matching, the resolver that turns a
//! family into a face, and the registry glyph runs name their fonts in.

mod family;
pub mod feature;
#[allow(clippy::module_inception)]
mod font;
mod matcher;
mod registry;
mod resolver;
mod synthesis;
mod variation;
mod weight;

pub use family::{FontFamily, FontListFontFamily, GenericFontFamily};
pub use feature::{FontFeature, parse_feature, parse_font_feature_settings};
pub use font::{DeviceFontFamilyName, Font, FontLoadingStrategy, FontSource};
pub use matcher::FontMatcher;
pub use registry::{FontData, FontId, FontRegistry};
pub use resolver::{FixedState, FontFamilyResolver, FontLoadError, Typeface, fixed_state};
pub use synthesis::{FontStyle, FontSynthesis};
pub use variation::{FontVariation, FontVariationSetting, FontVariationSettings};
pub use weight::{FontWeight, lerp_font_weight};

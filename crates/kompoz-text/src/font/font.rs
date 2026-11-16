//! A font of a family (Compose's Font): where its file is, its weight and style, its instance's axes, how it
//! loads.

use super::synthesis::FontStyle;
use super::variation::{FontVariation, FontVariationSettings};
use super::weight::FontWeight;
use kompoz_runtime::named;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

/// How a font is loaded (Compose's FontLoadingStrategy): before the text is laid out (`Blocking`); if it is on
/// the device already, else not at all (`OptionalLocal`); or in the background, the text laid out in the fallback
/// first and again once it has loaded (`Async`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum FontLoadingStrategy {
    #[default]
    Blocking,
    OptionalLocal,
    Async,
}

/// A family installed on the device, by its name (Android's DeviceFontFamilyName: "sans-serif-condensed",
/// fontconfig's "Noto Serif").
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct DeviceFontFamilyName(Arc<str>);

impl DeviceFontFamilyName {
    /// A name (an empty one panics, as Compose's).
    pub fn new(name: impl Into<Arc<str>>) -> DeviceFontFamilyName {
        let name = name.into();
        assert!(!name.is_empty(), "kompoz: name may not be empty");
        DeviceFontFamilyName(name)
    }
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// Where a font's data comes from.
#[derive(Clone)]
pub enum FontSource {
    /// A file on the device.
    File(PathBuf),
    /// The app's own asset, by its path (read through the asset loader the font resolver is given).
    Asset(Arc<str>),
    /// Bytes in memory, the face at `index` of a collection (Compose desktop's Font(identity, data)): equal by
    /// `identity` and index, which say which font it is.
    Bytes { identity: Arc<str>, data: Arc<[u8]>, index: u32 },
    /// A family installed on the device, its face for the font's weight and style.
    Device(DeviceFontFamilyName),
}

impl PartialEq for FontSource {
    fn eq(&self, o: &FontSource) -> bool {
        match (self, o) {
            (FontSource::File(a), FontSource::File(b)) => a == b,
            (FontSource::Asset(a), FontSource::Asset(b)) => a == b,
            (FontSource::Bytes { identity: a, index: i, .. }, FontSource::Bytes { identity: b, index: j, .. }) => {
                a == b && i == j
            }
            (FontSource::Device(a), FontSource::Device(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for FontSource {}

impl Hash for FontSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            FontSource::File(p) => p.hash(state),
            FontSource::Asset(p) => p.hash(state),
            FontSource::Bytes { identity, index, .. } => (identity, index).hash(state),
            FontSource::Device(n) => n.hash(state),
        }
    }
}

/// Where it is (the bytes' identity, not the bytes).
impl fmt::Debug for FontSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FontSource::File(p) => write!(f, "File({p:?})"),
            FontSource::Asset(p) => write!(f, "Asset({p:?})"),
            FontSource::Bytes { identity, data, index } => {
                write!(f, "Bytes({identity:?}, {} bytes, index {index})", data.len())
            }
            FontSource::Device(n) => write!(f, "Device({:?})", n.name()),
        }
    }
}

/// A font of a family (Compose's Font, its file, asset, bytes and device-name kinds): where it comes from, the
/// weight and style it is for, the settings of its axes (a variable font's instance: the same file can be several
/// of a family's fonts), how it loads. Structurally equal and hashed, so families of the same fonts are equal.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Font {
    pub source: FontSource,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub variation_settings: FontVariationSettings,
    pub loading_strategy: FontLoadingStrategy,
}

impl Font {
    /// Compose's maximum wait for an async font, after which the fallback stays.
    pub const MaximumAsyncTimeoutMillis: i64 = 15_000;

    /// A font from `source`, its axes set for its weight and style unless told (`variation_settings` None), as
    /// Compose's Font functions do; a device family's are left as they are.
    pub fn new_with(
        source: FontSource,
        weight: FontWeight,
        style: FontStyle,
        variation_settings: Option<FontVariationSettings>,
        loading_strategy: FontLoadingStrategy,
    ) -> Font {
        let variation_settings = variation_settings.unwrap_or_else(|| match source {
            FontSource::Device(_) => FontVariationSettings::Empty,
            _ => FontVariation::settings(weight, style, []),
        });
        Font { source, weight, style, variation_settings, loading_strategy }
    }

    /// A file's regular upright font, loaded before the text is laid out.
    pub fn file(path: impl Into<PathBuf>) -> Font {
        Font::new_with(FontSource::File(path.into()), FontWeight::Normal, FontStyle::Normal, None, Default::default())
    }
    /// An asset's.
    pub fn asset(path: impl Into<Arc<str>>) -> Font {
        Font::new_with(FontSource::Asset(path.into()), FontWeight::Normal, FontStyle::Normal, None, Default::default())
    }
    /// Bytes' (the first face).
    pub fn bytes(identity: impl Into<Arc<str>>, data: impl Into<Arc<[u8]>>) -> Font {
        let source = FontSource::Bytes { identity: identity.into(), data: data.into(), index: 0 };
        Font::new_with(source, FontWeight::Normal, FontStyle::Normal, None, Default::default())
    }
    /// A device family's.
    pub fn device(name: DeviceFontFamilyName) -> Font {
        Font::new_with(FontSource::Device(name), FontWeight::Normal, FontStyle::Normal, None, Default::default())
    }

    /// The same for another weight and style (its axes as they were).
    pub fn with_weight_and_style(&self, weight: FontWeight, style: FontStyle) -> Font {
        Font { weight, style, ..self.clone() }
    }
}

/// Compose's `Font(file, weight = Normal, style = Normal, variationSettings = Settings(weight, style))` and its
/// asset, bytes and device kinds, from a [`FontSource`] (`loading_strategy` Compose's ResourceFont's).
#[named(type)]
pub fn Font(
    source: FontSource,
    #[default(FontWeight::Normal)] weight: FontWeight,
    #[default(FontStyle::Normal)] style: FontStyle,
    #[default(None)] variation_settings: Option<FontVariationSettings>,
    #[default(FontLoadingStrategy::Blocking)] loading_strategy: FontLoadingStrategy,
) -> Font {
    Font::new_with(source, weight, style, variation_settings, loading_strategy)
}

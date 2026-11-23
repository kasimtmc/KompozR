//! Font resources (CMP's FontResources.kt, its skiko form): a font file of `font*/` as a font of a family, its
//! weight, style and axes as asked, read once for each of them.

use crate::caches::AsyncCache;
use crate::environment::{ResourceEnvironment, ResourceSelectionError};
use crate::reader::{DefaultResourceReader, LocalResourceReader, MissingResourceException, ResourceReader};
use crate::resource::FontResource;
use crate::state::remember_resource_state;
use kompoz_runtime::kompozable;
use kompoz_text::{
    DeviceFontFamilyName, Font, FontLoadingStrategy, FontSource, FontStyle, FontVariation, FontVariationSettings,
    FontWeight,
};
use std::fmt;
use std::sync::Arc;

/// Why a font resource can't be read: its file, its item.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FontResourceError {
    Missing(MissingResourceException),
    Selection(ResourceSelectionError),
}

impl fmt::Display for FontResourceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FontResourceError::Missing(e) => e.fmt(f),
            FontResourceError::Selection(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for FontResourceError {}

static FONT_CACHE: AsyncCache<String, Font> = AsyncCache::new();

pub(crate) fn clear_cache() {
    FONT_CACHE.clear();
}

/// The font an asynchronous platform shows while one loads (CMP's `defaultFont`: a system font of its own
/// identity); desktop loads at once, so composables never give it.
pub fn default_font() -> Font {
    Font::device(DeviceFontFamilyName::new("org.jetbrains.compose.resources.defaultFont"))
}

/// The settings' cache key (CMP's `FontVariation.Settings.getCacheKey()`): each setting, sorted.
fn cache_key(settings: &FontVariationSettings) -> String {
    let mut parts: Vec<String> = settings.settings().iter().map(|s| format!("{s:?}")).collect();
    parts.sort();
    parts.join(",")
}

/// `[size:last byte]` (CMP's `footprint`), which tells two files of a path apart in a font's identity.
fn footprint(bytes: &[u8]) -> String {
    match bytes.last() {
        Some(b) => format!("[{}:{}]", bytes.len(), *b as i8),
        None => format!("[{}:null]", bytes.len()),
    }
}

async fn load_font(
    resource: &FontResource,
    weight: FontWeight,
    style: FontStyle,
    settings: &FontVariationSettings,
    reader: &dyn ResourceReader,
    environment: &ResourceEnvironment,
) -> Result<Font, FontResourceError> {
    let path =
        resource.get_resource_item_by_environment(environment).map_err(FontResourceError::Selection)?.path.clone();
    let key = format!("{path}:{weight:?}:{style:?}:{}", cache_key(settings));
    FONT_CACHE
        .get_or_load(key.clone(), || async {
            let bytes = reader.read(&path).await.map_err(FontResourceError::Missing)?;
            let identity: Arc<str> = format!("{key}{}", footprint(&bytes)).into();
            let source = FontSource::Bytes { identity, data: bytes.into(), index: 0 };
            Ok(Font::new_with(source, weight, style, Some(settings.clone()), FontLoadingStrategy::Blocking))
        })
        .await
}

/// A font of a family from a font resource (CMP's `Font(resource, weight = Normal, style = Normal,
/// variationSettings = FontVariation.Settings(weight, style))`): its file for the environment here, read once for
/// these weight, style and axes. Panics if it can't be read.
///
/// Kotlin: `Font(resource: FontResource, weight: FontWeight = FontWeight.Normal, style: FontStyle = FontStyle.Normal, variationSettings: FontVariation.Settings = FontVariation.Settings(weight, style))`
#[kompozable]
pub fn font_resource(
    resource: FontResource,
    #[default(FontWeight::Normal)] weight: FontWeight,
    #[default(FontStyle::Normal)] style: FontStyle,
    #[default(None)] variation_settings: Option<FontVariationSettings>,
) -> Font {
    let settings = variation_settings.unwrap_or_else(|| FontVariation::settings(weight, style, []));
    let reader = LocalResourceReader.current();
    let keys = (resource.clone(), weight, style, settings.clone());
    let state = remember_resource_state(keys, default_font, move |env| async move {
        load_font(&resource, weight, style, &settings, &*reader, &env).await.unwrap_or_else(|e| panic!("{e}"))
    });
    state.get()
}

/// A font's bytes for `environment`, read by the default reader (CMP's `getFontResourceBytes`).
pub async fn get_font_resource_bytes(
    environment: &ResourceEnvironment,
    resource: &FontResource,
) -> Result<Vec<u8>, FontResourceError> {
    let item = resource.get_resource_item_by_environment(environment).map_err(FontResourceError::Selection)?;
    DefaultResourceReader.read(&item.path).await.map_err(FontResourceError::Missing)
}

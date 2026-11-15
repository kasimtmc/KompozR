//! Text helpers of the UI (Compose's `androidx.compose.ui.text` in ui): a remembered [`TextMeasurer`] with the
//! composition's fonts, density and layout direction.

use crate::platform::{LocalDensity, LocalFontFamilyResolver, LocalLayoutDirection};
use kompoz_runtime::{kompozable, remember_keyed};
use kompoz_text::{DefaultCacheSize, TextMeasurer};
use std::rc::Rc;

/// A text measurer remembered where it is called (Compose's rememberTextMeasurer): it lays text out with the
/// composition's font family resolver, density and layout direction where a measure gives none, keeping its last
/// `cache_size` layouts; a new one when any of them changes.
#[kompozable]
pub fn remember_text_measurer(#[default(DefaultCacheSize)] cache_size: usize) -> Rc<TextMeasurer> {
    let resolver = LocalFontFamilyResolver.current();
    let density = LocalDensity.current();
    let direction = LocalLayoutDirection.current();
    let identity = Rc::as_ptr(&resolver) as *const () as usize;
    remember_keyed((identity, density, direction, cache_size), move || {
        Rc::new(TextMeasurer::new(resolver, density, direction, cache_size))
    })
}

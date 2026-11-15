//! What turns a family, weight, style and synthesis into the face text is shaped in (Compose's
//! FontFamily.Resolver), and what it turns them into (Compose's Typeface).

use super::family::{FontFamily, GenericFontFamily};
use super::font::Font;
use super::registry::FontId;
use super::synthesis::{FontStyle, FontSynthesis};
use super::variation::FontVariationSettings;
use super::weight::FontWeight;
use crate::draw::GlyphSynthesis;
use crate::shape::FontBackend;
use kompoz_runtime::State;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

/// A face resolved for a weight and style (Compose's Typeface): the font text in that style is shaped in first
/// (by its [`FontId`]), its axes as set (the matched font's variation settings), what is faked (a lighter face
/// emboldened, an upright one slanted), the weight and style asked for, and the system family characters it
/// lacks fall back through (the backend's fallback, by script and locale, starts there).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Typeface {
    pub font: FontId,
    pub variation_settings: FontVariationSettings,
    pub synthesis: GlyphSynthesis,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub fallback: GenericFontFamily,
}

/// Why a font didn't load (Compose's "Could not load font").
#[derive(Clone, PartialEq, Debug)]
pub struct FontLoadError {
    pub font: Option<Font>,
    pub message: String,
}

impl fmt::Display for FontLoadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.font {
            Some(font) => write!(f, "could not load font {:?}: {}", font.source, self.message),
            None => write!(f, "could not load font: {}", self.message),
        }
    }
}

impl std::error::Error for FontLoadError {}

/// Resolves families to faces (Compose's FontFamily.Resolver; `LocalFontFamilyResolver` provides the host's):
/// `resolve` gives the face a family, weight, style and synthesis take now, as a state that changes when an async
/// font finishes loading (text laid out in the fallback meanwhile lays out again); `preload` loads a family's
/// fonts ahead; `backend` is what shapes and measures with the faces it resolves (they come from one collection,
/// one registry).
pub trait FontFamilyResolver {
    /// Loads `family`'s fonts (their async ones too) so text in them lays out at once (Compose's preload); fails
    /// if a font can't be loaded.
    fn preload(&self, family: &FontFamily) -> Pin<Box<dyn Future<Output = Result<(), FontLoadError>>>>;

    /// The face `family` (None: the default family) takes for `weight`, `style` and `synthesis`.
    fn resolve(
        &self,
        family: Option<&FontFamily>,
        weight: FontWeight,
        style: FontStyle,
        synthesis: FontSynthesis,
    ) -> State<Typeface>;

    /// What shapes and measures text in the faces it resolves.
    fn backend(&self) -> Rc<dyn FontBackend>;
}

/// A state that never changes: a face resolved at once.
pub struct FixedState<T>(pub T);

impl<T: 'static> kompoz_runtime::StateSource<T> for FixedState<T> {
    fn with_value(&self, _subscribe: bool, f: &mut dyn FnMut(&T)) {
        f(&self.0)
    }
    fn identity(&self) -> *const () {
        self as *const FixedState<T> as *const ()
    }
}

/// A state holding `value` for good.
pub fn fixed_state<T: Clone + 'static>(value: T) -> State<T> {
    State::from_source(FixedState(value))
}

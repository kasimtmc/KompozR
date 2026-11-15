//! The fonts text can be set in: the system's (fontconfig, CoreText, DirectWrite through fontique) and fonts
//! registered by bytes, grouped in families, with the generic families (sans-serif, serif, monospace, cursive,
//! emoji) and the fallback families by script and language that fill in what a font lacks. Only bytes and indices
//! cross fontique's boundary (it reads fonts with another read-fonts than ours): the faces it hands out are
//! registered in the [`FontRegistry`] and read back by id.

use super::family::GenericFontFamily;
use super::registry::{FontBytes, FontId, FontRegistry};
use super::synthesis::FontStyle;
use super::weight::FontWeight;
use crate::intl::Locale;
use fontique::{
    Blob, Collection, CollectionOptions, FallbackKey, FamilyId, FontInfo, FontWidth, GenericFamily, Language, Script,
    SourceCache, SourceId, SourceInfo, SourceKind,
};
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

/// A face a family gave for a weight and style: the font (registered), the weight and style it is designed in,
/// the settings of its axes that make it the weight and style asked for (a variable font's `wght`, `ital`,
/// `slnt`), and the family it is of.
#[derive(Clone, PartialEq, Debug)]
pub struct CollectionFace {
    pub font: FontId,
    pub weight: f32,
    pub italic: bool,
    pub variations: Vec<([u8; 4], f32)>,
    pub family: FontFamilyId,
}

impl CollectionFace {
    /// The weight it is drawn at: its `wght` axis' setting, else its design's.
    pub fn effective_weight(&self) -> f32 {
        self.variations.iter().find(|(t, _)| t == b"wght").map_or(self.weight, |(_, v)| *v)
    }
    /// Whether it is drawn italic: designed so, or its `ital` or `slnt` axis set so.
    pub fn effective_italic(&self) -> bool {
        self.italic || self.variations.iter().any(|(t, v)| (t == b"ital" && *v > 0.0) || (t == b"slnt" && *v != 0.0))
    }
}

/// A family of a [`FontCollection`], by number (fontique's family id).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FontFamilyId(FamilyId);

impl FontFamilyId {
    fn of(id: FamilyId) -> FontFamilyId {
        FontFamilyId(id)
    }
    /// Its number.
    pub fn to_u64(self) -> u64 {
        self.0.to_u64()
    }
}

/// What a [`FontCollection`] holds besides the fonts registered with it: the system's too, or not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FontCollectionOptions {
    pub system_fonts: bool,
}

impl Default for FontCollectionOptions {
    fn default() -> FontCollectionOptions {
        FontCollectionOptions { system_fonts: true }
    }
}

struct Inner {
    collection: Collection,
    sources: SourceCache,
    /// Families registered by bytes, in the order they came: the last fallback for what nothing else has.
    registered: Vec<FamilyId>,
    /// The faces handed out, by font file and index: their ids.
    ids: HashMap<(u64, u32), FontId>,
}

/// A collection of font families (single-threaded, as the UI is): the system's, if asked for, and those
/// registered from bytes; the generic families and the fallback families by script and language (the system's
/// unless set here: a collection of registered fonts only sets its own). The system's fonts are looked at when
/// it is first asked something, not when it is made (a host that lays out no text never scans them).
pub struct FontCollection {
    options: FontCollectionOptions,
    inner: RefCell<Option<Inner>>,
}

impl FontCollection {
    /// A collection of the system's fonts (and any registered later).
    pub fn system() -> FontCollection {
        FontCollection::new(FontCollectionOptions { system_fonts: true })
    }

    /// A collection of only the fonts registered with it (what tests use, so nothing depends on the machine).
    pub fn registered_only() -> FontCollection {
        FontCollection::new(FontCollectionOptions { system_fonts: false })
    }

    pub fn new(options: FontCollectionOptions) -> FontCollection {
        FontCollection { options, inner: RefCell::new(None) }
    }

    /// Whether it holds the system's fonts.
    pub fn has_system_fonts(&self) -> bool {
        self.options.system_fonts
    }

    /// Its state, made (the system's fonts scanned) the first time.
    fn inner(&self) -> RefMut<'_, Inner> {
        RefMut::map(self.inner.borrow_mut(), |inner| {
            inner.get_or_insert_with(|| Inner {
                collection: Collection::new(CollectionOptions {
                    shared: false,
                    system_fonts: self.options.system_fonts,
                }),
                sources: SourceCache::default(),
                registered: Vec::new(),
                ids: HashMap::new(),
            })
        })
    }

    /// Registers the fonts in a font file's bytes (every face of a collection), each in its family (by the name
    /// the font gives); the families' names, in the order the file has them.
    pub fn register(&self, bytes: FontBytes) -> Vec<String> {
        let mut inner = self.inner();
        let added = inner.collection.register_fonts(Blob::new(bytes), None);
        let mut names = Vec::new();
        for (family, _) in added {
            if !inner.registered.contains(&family) {
                inner.registered.push(family);
            }
            if let Some(name) = inner.collection.family_name(family) {
                let name = name.to_string();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        names
    }

    /// Registers the fonts of a file (read now); its families' names, None if it can't be read.
    pub fn register_file(&self, path: impl AsRef<Path>) -> Option<Vec<String>> {
        let bytes = std::fs::read(path).ok()?;
        Some(self.register(Arc::new(bytes)))
    }

    /// Every family's name (registered ones before the system's, each kind in no particular order).
    pub fn family_names(&self) -> Vec<String> {
        self.inner().collection.family_names().map(str::to_string).collect()
    }

    /// The family of a name (any of its names, ignoring case).
    pub fn family_id(&self, name: &str) -> Option<FontFamilyId> {
        self.inner().collection.family_id(name).map(FontFamilyId::of)
    }

    /// A family's name.
    pub fn family_name(&self, family: FontFamilyId) -> Option<String> {
        self.inner().collection.family_name(family.0).map(str::to_string)
    }

    /// The families a generic family is, by name, in order (replacing the system's: what a collection of
    /// registered fonts says its sans-serif is). The default family is sans-serif's.
    pub fn set_generic_families(&self, generic: GenericFontFamily, families: &[&str]) {
        self.set_generic(generic_of(generic), families);
    }

    /// The families emoji are shown in, in order (fontique's emoji generic family).
    pub fn set_emoji_families(&self, families: &[&str]) {
        self.set_generic(GenericFamily::Emoji, families);
    }

    fn set_generic(&self, generic: GenericFamily, families: &[&str]) {
        let mut inner = self.inner();
        let ids: Vec<FamilyId> = families.iter().filter_map(|n| inner.collection.family_id(n)).collect();
        inner.collection.set_generic_families(generic, ids.into_iter());
    }

    /// The families that fill in characters of `script` (an ISO 15924 code) a font lacks, for text in `locale`
    /// (None: any language), in order (replacing the system's).
    pub fn set_fallback_families(&self, script: [u8; 4], locale: Option<&Locale>, families: &[&str]) {
        let mut inner = self.inner();
        let ids: Vec<FamilyId> = families.iter().filter_map(|n| inner.collection.family_id(n)).collect();
        let key = fallback_key(script, locale);
        inner.collection.set_fallbacks(key, ids.into_iter());
    }

    /// The families of a generic family, in order (the default family's are sans-serif's).
    pub fn generic_families(&self, generic: GenericFontFamily) -> Vec<FontFamilyId> {
        self.generic(generic_of(generic))
    }

    /// The families emoji are shown in.
    pub fn emoji_families(&self) -> Vec<FontFamilyId> {
        self.generic(GenericFamily::Emoji)
    }

    fn generic(&self, generic: GenericFamily) -> Vec<FontFamilyId> {
        self.inner().collection.generic_families(generic).map(FontFamilyId::of).collect()
    }

    /// The families that fill in `script` for `locale` (the system's choice unless set); none for a locale that
    /// writes the script no way of its own (English and Arabic: the locale-free ones are what apply).
    pub fn fallback_families(&self, script: [u8; 4], locale: Option<&Locale>) -> Vec<FontFamilyId> {
        let key = fallback_key(script, locale);
        if locale.is_some() && !key.is_tracked() {
            return Vec::new();
        }
        self.inner().collection.fallback_families(key).map(FontFamilyId::of).collect()
    }

    /// The families registered by bytes, in the order they came.
    pub fn registered_families(&self) -> Vec<FontFamilyId> {
        self.inner().registered.iter().map(|f| FontFamilyId::of(*f)).collect()
    }

    /// The face of `family` that a weight and style take (CSS's font matching, as fontique does it), registered;
    /// None if the family has none or it can't be read.
    pub fn face(&self, family: FontFamilyId, weight: FontWeight, style: FontStyle) -> Option<CollectionFace> {
        let mut inner = self.inner();
        let info = inner.collection.family(family.0)?;
        let fstyle = match style {
            FontStyle::Normal => fontique::FontStyle::Normal,
            FontStyle::Italic => fontique::FontStyle::Italic,
        };
        let fweight = fontique::FontWeight::new(weight.weight as f32);
        let font = info.match_font(FontWidth::NORMAL, fstyle, fweight, true)?.clone();
        let synthesis = font.synthesis(FontWidth::NORMAL, fstyle, fweight);
        let variations = synthesis.variation_settings().iter().map(|(t, v)| (t.to_be_bytes(), *v)).collect();
        let id = inner.register_face(&font)?;
        Some(CollectionFace {
            font: id,
            weight: font.weight().value(),
            italic: !matches!(font.style(), fontique::FontStyle::Normal),
            variations,
            family,
        })
    }

    /// The face a family of this name has for a weight and style.
    pub fn face_by_name(&self, name: &str, weight: FontWeight, style: FontStyle) -> Option<CollectionFace> {
        let family = self.family_id(name)?;
        self.face(family, weight, style)
    }
}

impl Inner {
    /// The id of a face fontique describes, its file loaded (mapped) once.
    fn register_face(&mut self, font: &FontInfo) -> Option<FontId> {
        let key = (font.source().id().to_u64(), font.index());
        if let Some(id) = self.ids.get(&key) {
            return Some(*id);
        }
        let blob = font.load(Some(&mut self.sources))?;
        let id = FontRegistry::register_bytes(blob.into_raw_parts().0, font.index());
        self.ids.insert(key, id);
        Some(id)
    }
}

/// The bytes of a font file, mapped into memory (as fontique maps the system's): None if it can't be read. Any
/// thread may call it.
pub fn load_font_file(path: &Path) -> Option<FontBytes> {
    let source = SourceInfo::new(SourceId::new(), SourceKind::Path(Arc::from(path)));
    SourceCache::default().get(&source).map(|blob| blob.into_raw_parts().0)
}

/// Its families' names.
impl fmt::Debug for FontCollection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let registered = self.inner.try_borrow().map_or(0, |i| i.as_ref().map_or(0, |i| i.registered.len()));
        write!(f, "FontCollection(system_fonts={}, registered={registered})", self.options.system_fonts)
    }
}

fn generic_of(generic: GenericFontFamily) -> GenericFamily {
    match generic {
        GenericFontFamily::Default | GenericFontFamily::SansSerif => GenericFamily::SansSerif,
        GenericFontFamily::Serif => GenericFamily::Serif,
        GenericFontFamily::Monospace => GenericFamily::Monospace,
        GenericFontFamily::Cursive => GenericFamily::Cursive,
    }
}

fn fallback_key(script: [u8; 4], locale: Option<&Locale>) -> FallbackKey {
    let language = locale.and_then(|l| Language::parse(&l.to_language_tag()).ok());
    FallbackKey::new(Script::from_bytes(script), language.as_ref())
}

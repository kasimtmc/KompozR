//! Qualifiers (CMP's Qualifier.kt): what a resource directory's name says it is for — a language (`values-tr`), a
//! region after it (`values-pt-rBR`), a theme (`drawable-dark`), a screen density (`drawable-xxhdpi`) — and what an
//! environment is (its language, region, theme and density).

use std::fmt;

/// One qualifier of a resource item (CMP's `Qualifier` interface, its four kinds).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Qualifier {
    Language(LanguageQualifier),
    Region(RegionQualifier),
    Theme(ThemeQualifier),
    Density(DensityQualifier),
}

impl From<LanguageQualifier> for Qualifier {
    fn from(q: LanguageQualifier) -> Qualifier {
        Qualifier::Language(q)
    }
}

impl From<RegionQualifier> for Qualifier {
    fn from(q: RegionQualifier) -> Qualifier {
        Qualifier::Region(q)
    }
}

impl From<ThemeQualifier> for Qualifier {
    fn from(q: ThemeQualifier) -> Qualifier {
        Qualifier::Theme(q)
    }
}

impl From<DensityQualifier> for Qualifier {
    fn from(q: DensityQualifier) -> Qualifier {
        Qualifier::Density(q)
    }
}

/// A language (ISO 639: `en`, `tr`, `fil`); equal by it (CMP's LanguageQualifier).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct LanguageQualifier {
    pub language: String,
}

impl LanguageQualifier {
    /// Kotlin: `LanguageQualifier(language: String)`
    pub fn new(language: impl Into<String>) -> LanguageQualifier {
        LanguageQualifier { language: language.into() }
    }
}

impl fmt::Debug for LanguageQualifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LanguageQualifier(language='{}')", self.language)
    }
}

/// A region (ISO 3166: `US`, `TR`; written `rUS` in a directory's name); equal by it (CMP's RegionQualifier).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RegionQualifier {
    pub region: String,
}

impl RegionQualifier {
    /// Kotlin: `RegionQualifier(region: String)`
    pub fn new(region: impl Into<String>) -> RegionQualifier {
        RegionQualifier { region: region.into() }
    }
}

impl fmt::Debug for RegionQualifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RegionQualifier(region='{}')", self.region)
    }
}

/// A theme (CMP's ThemeQualifier: `LIGHT`, `DARK`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ThemeQualifier {
    /// Kotlin: `LIGHT`
    Light,
    /// Kotlin: `DARK`
    Dark,
}

impl ThemeQualifier {
    /// Dark if `is_dark`, else light.
    pub fn select_by_value(is_dark: bool) -> ThemeQualifier {
        if is_dark { ThemeQualifier::Dark } else { ThemeQualifier::Light }
    }
    pub const entries: [ThemeQualifier; 2] = [ThemeQualifier::Light, ThemeQualifier::Dark];
}

/// A screen density (CMP's DensityQualifier, Android's buckets) and its dots per inch.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DensityQualifier {
    /// Kotlin: `LDPI` (120 dpi)
    Ldpi,
    /// Kotlin: `MDPI` (160 dpi: 1 px a dp)
    Mdpi,
    /// Kotlin: `HDPI` (240 dpi)
    Hdpi,
    /// Kotlin: `XHDPI` (320 dpi)
    Xhdpi,
    /// Kotlin: `XXHDPI` (480 dpi)
    Xxhdpi,
    /// Kotlin: `XXXHDPI` (640 dpi)
    Xxxhdpi,
}

impl DensityQualifier {
    /// All of them, from the lowest (Kotlin's `entries`).
    pub const entries: [DensityQualifier; 6] = [
        DensityQualifier::Ldpi,
        DensityQualifier::Mdpi,
        DensityQualifier::Hdpi,
        DensityQualifier::Xhdpi,
        DensityQualifier::Xxhdpi,
        DensityQualifier::Xxxhdpi,
    ];

    /// Its dots per inch.
    pub fn dpi(self) -> i32 {
        match self {
            DensityQualifier::Ldpi => 120,
            DensityQualifier::Mdpi => 160,
            DensityQualifier::Hdpi => 240,
            DensityQualifier::Xhdpi => 320,
            DensityQualifier::Xxhdpi => 480,
            DensityQualifier::Xxxhdpi => 640,
        }
    }

    /// The bucket of `dpi` dots per inch: the lowest that is at least as dense.
    pub fn select_by_value(dpi: i32) -> DensityQualifier {
        DensityQualifier::entries.into_iter().take(5).find(|q| dpi <= q.dpi()).unwrap_or(DensityQualifier::Xxxhdpi)
    }

    /// The bucket of a density (px a dp: Compose's `Density.density`).
    pub fn select_by_density(density: f32) -> DensityQualifier {
        match density {
            d if d <= 0.75 => DensityQualifier::Ldpi,
            d if d <= 1.0 => DensityQualifier::Mdpi,
            d if d <= 1.5 => DensityQualifier::Hdpi,
            d if d <= 2.0 => DensityQualifier::Xhdpi,
            d if d <= 3.0 => DensityQualifier::Xxhdpi,
            _ => DensityQualifier::Xxxhdpi,
        }
    }

    /// Its name as Kotlin writes it (`XXHDPI`).
    pub fn name(self) -> &'static str {
        match self {
            DensityQualifier::Ldpi => "LDPI",
            DensityQualifier::Mdpi => "MDPI",
            DensityQualifier::Hdpi => "HDPI",
            DensityQualifier::Xhdpi => "XHDPI",
            DensityQualifier::Xxhdpi => "XXHDPI",
            DensityQualifier::Xxxhdpi => "XXXHDPI",
        }
    }
}

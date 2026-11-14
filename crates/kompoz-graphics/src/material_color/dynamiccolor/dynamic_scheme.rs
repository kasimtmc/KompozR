//! Material Color Utilities' DynamicScheme, Variant, SpecVersion and Platform, and the palettes each spec makes
//! of a source color.

use super::{DynamicColor, MaterialDynamicColors};
use crate::color::Color;
use crate::material_color::dislike::DislikeAnalyzer;
use crate::material_color::hct::Hct;
use crate::material_color::palettes::TonalPalette;
use crate::material_color::temperature::TemperatureCache;
use crate::material_color::utils::math_utils::sanitize_degrees_double;

/// How a scheme turns its source color into palettes (Material Color Utilities' Variant).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variant {
    /// All grey.
    Monochrome,
    /// Close to grey, a hint of the source's hue.
    Neutral,
    /// Calm, the source's hue at a low chroma: Android's default.
    TonalSpot,
    /// The source's hue at the most chroma.
    Vibrant,
    /// The source's hue turned, a playful mix.
    Expressive,
    /// The source as it is, its tertiary its complement.
    Fidelity,
    /// The source as it is (a picture's), its tertiary an analogous color.
    Content,
    /// Colorful accents, grey neutrals.
    Rainbow,
    /// Accents of hues around the source's.
    FruitSalad,
}

/// Which version of the Material spec a scheme's colors follow (Material Color Utilities' SpecVersion): 2025's
/// (Material 3 Expressive) changes tonal spot, neutral, vibrant and expressive; the others stay 2021's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpecVersion {
    Spec2021,
    Spec2025,
}

/// What the scheme is for (Material Color Utilities' Platform): 2025's colors differ on a watch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Platform {
    Phone,
    Watch,
}

/// A color scheme of a source color (Material Color Utilities' DynamicScheme): its variant, light or dark, its
/// contrast level (-1 reduced, 0 standard, 0.5 medium, 1 high), the platform and spec it is for, and the six
/// palettes it is made of. Ask it the color of a [`MaterialDynamicColors`] role ([`DynamicScheme::get_color`], or
/// the role's method: `scheme.primary()`).
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicScheme {
    pub source_color_argb: u32,
    pub source_color_hct: Hct,
    pub variant: Variant,
    pub is_dark: bool,
    pub contrast_level: f64,
    pub platform: Platform,
    pub spec_version: SpecVersion,
    pub primary_palette: TonalPalette,
    pub secondary_palette: TonalPalette,
    pub tertiary_palette: TonalPalette,
    pub neutral_palette: TonalPalette,
    pub neutral_variant_palette: TonalPalette,
    pub error_palette: TonalPalette,
}

impl DynamicScheme {
    /// What schemes follow unless told: 2021's.
    pub const DEFAULT_SPEC_VERSION: SpecVersion = SpecVersion::Spec2021;
    pub const DEFAULT_PLATFORM: Platform = Platform::Phone;

    /// The scheme of a source color in a variant, its palettes the spec's (a spec other than 2021 only for tonal
    /// spot, neutral, vibrant and expressive: the others fall back to 2021's).
    pub fn new(
        source_color_hct: Hct,
        variant: Variant,
        is_dark: bool,
        contrast_level: f64,
        platform: Platform,
        spec_version: SpecVersion,
    ) -> DynamicScheme {
        let spec = maybe_fallback_spec_version(spec_version, variant);
        let palette = |role: PaletteRole| scheme_palette(spec, role, variant, &source_color_hct, is_dark, platform);
        DynamicScheme::with_palettes(
            source_color_hct,
            variant,
            is_dark,
            contrast_level,
            platform,
            spec_version,
            palette(PaletteRole::Primary),
            palette(PaletteRole::Secondary),
            palette(PaletteRole::Tertiary),
            palette(PaletteRole::Neutral),
            palette(PaletteRole::NeutralVariant),
            error_palette(spec, variant, &source_color_hct, platform),
        )
    }

    /// The scheme of these palettes (the error palette, unless given, hue 25 at chroma 84).
    #[allow(clippy::too_many_arguments)]
    pub fn with_palettes(
        source_color_hct: Hct,
        variant: Variant,
        is_dark: bool,
        contrast_level: f64,
        platform: Platform,
        spec_version: SpecVersion,
        primary_palette: TonalPalette,
        secondary_palette: TonalPalette,
        tertiary_palette: TonalPalette,
        neutral_palette: TonalPalette,
        neutral_variant_palette: TonalPalette,
        error_palette: Option<TonalPalette>,
    ) -> DynamicScheme {
        DynamicScheme {
            source_color_argb: source_color_hct.to_int(),
            source_color_hct,
            variant,
            is_dark,
            contrast_level,
            platform,
            spec_version: maybe_fallback_spec_version(spec_version, variant),
            primary_palette,
            secondary_palette,
            tertiary_palette,
            neutral_palette,
            neutral_variant_palette,
            error_palette: error_palette.unwrap_or_else(|| TonalPalette::from_hue_and_chroma(25.0, 84.0)),
        }
    }

    /// `other`, light or dark and at another contrast level, its palettes kept.
    pub fn from_scheme(other: &DynamicScheme, is_dark: bool, contrast_level: f64) -> DynamicScheme {
        DynamicScheme { is_dark, contrast_level, ..other.clone() }
    }

    /// The hue of the range of `hue_breakpoints` the source's hue is in (`hues[i]` for breakpoints i..i+1), or
    /// the source's hue where it is in none.
    pub fn get_piecewise_hue(source_color_hct: &Hct, hue_breakpoints: &[f64], hues: &[f64]) -> f64 {
        let size = (hue_breakpoints.len().saturating_sub(1)).min(hues.len());
        let source_hue = source_color_hct.hue();
        for i in 0..size {
            if source_hue >= hue_breakpoints[i] && source_hue < hue_breakpoints[i + 1] {
                return sanitize_degrees_double(hues[i]);
            }
        }
        // No condition matched, return the source hue.
        source_hue
    }

    /// The source's hue turned by the rotation of the range it is in.
    pub fn get_rotated_hue(source_color_hct: &Hct, hue_breakpoints: &[f64], rotations: &[f64]) -> f64 {
        let mut rotation = DynamicScheme::get_piecewise_hue(source_color_hct, hue_breakpoints, rotations);
        if (hue_breakpoints.len().saturating_sub(1)).min(rotations.len()) == 0 {
            // No condition matched, return the source hue.
            rotation = 0.0;
        }
        sanitize_degrees_double(source_color_hct.hue() + rotation)
    }

    /// The ARGB color of a role.
    pub fn get_argb(&self, color: &DynamicColor) -> u32 {
        color.get_argb(self)
    }

    /// The color of a role, in HCT.
    pub fn get_hct(&self, color: &DynamicColor) -> Hct {
        color.get_hct(self)
    }

    /// The color of a role.
    pub fn get_color(&self, color: &DynamicColor) -> Color {
        Color::from_argb(color.get_argb(self))
    }
}

macro_rules! roles {
    ($($role:ident)*) => {
        impl DynamicScheme {
            $(
                #[doc = concat!("The color of the `", stringify!($role), "` role ([`MaterialDynamicColors::", stringify!($role), "`]).")]
                pub fn $role(&self) -> Color {
                    self.get_color(&MaterialDynamicColors.$role())
                }
            )*
        }
    };
}

roles!(
    primary_palette_key_color secondary_palette_key_color tertiary_palette_key_color neutral_palette_key_color
    neutral_variant_palette_key_color error_palette_key_color
    background on_background surface surface_dim surface_bright surface_container_lowest surface_container_low
    surface_container surface_container_high surface_container_highest on_surface surface_variant on_surface_variant
    outline outline_variant inverse_surface inverse_on_surface shadow scrim surface_tint
    primary primary_dim on_primary primary_container on_primary_container inverse_primary
    primary_fixed primary_fixed_dim on_primary_fixed on_primary_fixed_variant
    secondary secondary_dim on_secondary secondary_container on_secondary_container
    secondary_fixed secondary_fixed_dim on_secondary_fixed on_secondary_fixed_variant
    tertiary tertiary_dim on_tertiary tertiary_container on_tertiary_container
    tertiary_fixed tertiary_fixed_dim on_tertiary_fixed on_tertiary_fixed_variant
    error error_dim on_error error_container on_error_container
);

/// Spec versions other than 2021 are only for tonal spot, neutral, vibrant and expressive.
fn maybe_fallback_spec_version(spec_version: SpecVersion, variant: Variant) -> SpecVersion {
    match variant {
        Variant::Expressive | Variant::Vibrant | Variant::TonalSpot | Variant::Neutral => spec_version,
        _ => SpecVersion::Spec2021,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PaletteRole {
    Primary,
    Secondary,
    Tertiary,
    Neutral,
    NeutralVariant,
}

/// A role's palette of a spec (Material Color Utilities' ColorSpec getPrimaryPalette, …): 2025's where it has
/// one, 2021's otherwise.
fn scheme_palette(
    spec: SpecVersion,
    role: PaletteRole,
    variant: Variant,
    source: &Hct,
    is_dark: bool,
    platform: Platform,
) -> TonalPalette {
    if spec == SpecVersion::Spec2025
        && let Some(p) = palette_2025(role, variant, source, is_dark, platform)
    {
        return p;
    }
    palette_2021(role, variant, source)
}

fn error_palette(spec: SpecVersion, variant: Variant, source: &Hct, platform: Platform) -> Option<TonalPalette> {
    if spec != SpecVersion::Spec2025 {
        return None;
    }
    let error_hue = DynamicScheme::get_piecewise_hue(
        source,
        &[0.0, 3.0, 13.0, 23.0, 33.0, 43.0, 153.0, 273.0, 360.0],
        &[12.0, 22.0, 32.0, 12.0, 22.0, 32.0, 22.0, 12.0],
    );
    let phone = platform == Platform::Phone;
    let chroma = match variant {
        Variant::Neutral => {
            if phone {
                50.0
            } else {
                40.0
            }
        }
        Variant::TonalSpot => {
            if phone {
                60.0
            } else {
                48.0
            }
        }
        Variant::Expressive => {
            if phone {
                64.0
            } else {
                48.0
            }
        }
        Variant::Vibrant => {
            if phone {
                80.0
            } else {
                60.0
            }
        }
        _ => return None,
    };
    Some(TonalPalette::from_hue_and_chroma(error_hue, chroma))
}

fn palette_2021(role: PaletteRole, variant: Variant, source: &Hct) -> TonalPalette {
    let p = TonalPalette::from_hue_and_chroma;
    let (hue, chroma) = (source.hue(), source.chroma());
    match role {
        PaletteRole::Primary => match variant {
            Variant::Content | Variant::Fidelity => p(hue, chroma),
            Variant::FruitSalad => p(sanitize_degrees_double(hue - 50.0), 48.0),
            Variant::Monochrome => p(hue, 0.0),
            Variant::Neutral => p(hue, 12.0),
            Variant::Rainbow => p(hue, 48.0),
            Variant::TonalSpot => p(hue, 36.0),
            Variant::Expressive => p(sanitize_degrees_double(hue + 240.0), 40.0),
            Variant::Vibrant => p(hue, 200.0),
        },
        PaletteRole::Secondary => match variant {
            Variant::Content | Variant::Fidelity => p(hue, (chroma - 32.0).max(chroma * 0.5)),
            Variant::FruitSalad => p(sanitize_degrees_double(hue - 50.0), 36.0),
            Variant::Monochrome => p(hue, 0.0),
            Variant::Neutral => p(hue, 8.0),
            Variant::Rainbow | Variant::TonalSpot => p(hue, 16.0),
            Variant::Expressive => p(
                DynamicScheme::get_rotated_hue(
                    source,
                    &[0.0, 21.0, 51.0, 121.0, 151.0, 191.0, 271.0, 321.0, 360.0],
                    &[45.0, 95.0, 45.0, 20.0, 45.0, 90.0, 45.0, 45.0, 45.0],
                ),
                24.0,
            ),
            Variant::Vibrant => p(
                DynamicScheme::get_rotated_hue(
                    source,
                    &[0.0, 41.0, 61.0, 101.0, 131.0, 181.0, 251.0, 301.0, 360.0],
                    &[18.0, 15.0, 10.0, 12.0, 15.0, 18.0, 15.0, 12.0, 12.0],
                ),
                24.0,
            ),
        },
        PaletteRole::Tertiary => match variant {
            Variant::Content => TonalPalette::from_hct(DislikeAnalyzer::fix_if_disliked(
                TemperatureCache::new(*source).analogous_colors_with(3, 6)[2],
            )),
            Variant::Fidelity => {
                TonalPalette::from_hct(DislikeAnalyzer::fix_if_disliked(TemperatureCache::new(*source).complement()))
            }
            Variant::FruitSalad => p(hue, 36.0),
            Variant::Monochrome => p(hue, 0.0),
            Variant::Neutral => p(hue, 16.0),
            Variant::Rainbow | Variant::TonalSpot => p(sanitize_degrees_double(hue + 60.0), 24.0),
            Variant::Expressive => p(
                DynamicScheme::get_rotated_hue(
                    source,
                    &[0.0, 21.0, 51.0, 121.0, 151.0, 191.0, 271.0, 321.0, 360.0],
                    &[120.0, 120.0, 20.0, 45.0, 20.0, 15.0, 20.0, 120.0, 120.0],
                ),
                32.0,
            ),
            Variant::Vibrant => p(
                DynamicScheme::get_rotated_hue(
                    source,
                    &[0.0, 41.0, 61.0, 101.0, 131.0, 181.0, 251.0, 301.0, 360.0],
                    &[35.0, 30.0, 20.0, 25.0, 30.0, 35.0, 30.0, 25.0, 25.0],
                ),
                32.0,
            ),
        },
        PaletteRole::Neutral => match variant {
            Variant::Content | Variant::Fidelity => p(hue, chroma / 8.0),
            Variant::FruitSalad => p(hue, 10.0),
            Variant::Monochrome => p(hue, 0.0),
            Variant::Neutral => p(hue, 2.0),
            Variant::Rainbow => p(hue, 0.0),
            Variant::TonalSpot => p(hue, 6.0),
            Variant::Expressive => p(sanitize_degrees_double(hue + 15.0), 8.0),
            Variant::Vibrant => p(hue, 10.0),
        },
        PaletteRole::NeutralVariant => match variant {
            Variant::Content | Variant::Fidelity => p(hue, (chroma / 8.0) + 4.0),
            Variant::FruitSalad => p(hue, 16.0),
            Variant::Monochrome => p(hue, 0.0),
            Variant::Neutral => p(hue, 2.0),
            Variant::Rainbow => p(hue, 0.0),
            Variant::TonalSpot => p(hue, 8.0),
            Variant::Expressive => p(sanitize_degrees_double(hue + 15.0), 12.0),
            Variant::Vibrant => p(hue, 12.0),
        },
    }
}

fn expressive_neutral_hue_2025(source: &Hct) -> f64 {
    DynamicScheme::get_rotated_hue(
        source,
        &[0.0, 71.0, 124.0, 253.0, 278.0, 300.0, 360.0],
        &[10.0, 0.0, 10.0, 0.0, 10.0, 0.0],
    )
}

fn expressive_neutral_chroma_2025(source: &Hct, is_dark: bool, platform: Platform) -> f64 {
    let neutral_hue = expressive_neutral_hue_2025(source);
    let phone = platform == Platform::Phone;
    if phone && is_dark && Hct::is_yellow(neutral_hue) {
        6.0
    } else if phone && is_dark {
        14.0
    } else if phone {
        18.0
    } else {
        12.0
    }
}

fn vibrant_neutral_hue_2025(source: &Hct) -> f64 {
    DynamicScheme::get_rotated_hue(source, &[0.0, 38.0, 105.0, 140.0, 333.0, 360.0], &[-14.0, 10.0, -14.0, 10.0, -14.0])
}

fn vibrant_neutral_chroma_2025(source: &Hct, platform: Platform) -> f64 {
    let neutral_hue = vibrant_neutral_hue_2025(source);
    if platform == Platform::Phone || Hct::is_blue(neutral_hue) { 28.0 } else { 20.0 }
}

/// 2025's palette of a role, where it has its own (tonal spot, neutral, vibrant and expressive).
fn palette_2025(
    role: PaletteRole,
    variant: Variant,
    source: &Hct,
    is_dark: bool,
    platform: Platform,
) -> Option<TonalPalette> {
    let p = TonalPalette::from_hue_and_chroma;
    let hue = source.hue();
    let phone = platform == Platform::Phone;
    let palette = match role {
        PaletteRole::Primary => match variant {
            Variant::Neutral => p(
                hue,
                if phone && Hct::is_blue(hue) {
                    12.0
                } else if phone {
                    8.0
                } else if Hct::is_blue(hue) {
                    16.0
                } else {
                    12.0
                },
            ),
            Variant::TonalSpot => p(hue, if phone && is_dark { 26.0 } else { 32.0 }),
            Variant::Expressive => p(
                hue,
                if phone && is_dark {
                    36.0
                } else if phone {
                    48.0
                } else {
                    40.0
                },
            ),
            Variant::Vibrant => p(hue, if phone { 74.0 } else { 56.0 }),
            _ => return None,
        },
        PaletteRole::Secondary => match variant {
            Variant::Neutral => p(
                hue,
                if phone && Hct::is_blue(hue) {
                    6.0
                } else if phone {
                    4.0
                } else if Hct::is_blue(hue) {
                    10.0
                } else {
                    6.0
                },
            ),
            Variant::TonalSpot => p(hue, 16.0),
            Variant::Expressive => p(
                DynamicScheme::get_rotated_hue(
                    source,
                    &[0.0, 105.0, 140.0, 204.0, 253.0, 278.0, 300.0, 333.0, 360.0],
                    &[-160.0, 155.0, -100.0, 96.0, -96.0, -156.0, -165.0, -160.0],
                ),
                if phone && is_dark { 16.0 } else { 24.0 },
            ),
            Variant::Vibrant => p(
                DynamicScheme::get_rotated_hue(
                    source,
                    &[0.0, 38.0, 105.0, 140.0, 333.0, 360.0],
                    &[-14.0, 10.0, -14.0, 10.0, -14.0],
                ),
                if phone { 56.0 } else { 36.0 },
            ),
            _ => return None,
        },
        PaletteRole::Tertiary => match variant {
            Variant::Neutral => p(
                DynamicScheme::get_rotated_hue(
                    source,
                    &[0.0, 38.0, 105.0, 161.0, 204.0, 278.0, 333.0, 360.0],
                    &[-32.0, 26.0, 10.0, -39.0, 24.0, -15.0, -32.0],
                ),
                if phone { 20.0 } else { 36.0 },
            ),
            Variant::TonalSpot => p(
                DynamicScheme::get_rotated_hue(
                    source,
                    &[0.0, 20.0, 71.0, 161.0, 333.0, 360.0],
                    &[-40.0, 48.0, -32.0, 40.0, -32.0],
                ),
                if phone { 28.0 } else { 32.0 },
            ),
            Variant::Expressive => p(
                DynamicScheme::get_rotated_hue(
                    source,
                    &[0.0, 105.0, 140.0, 204.0, 253.0, 278.0, 300.0, 333.0, 360.0],
                    &[-165.0, 160.0, -105.0, 101.0, -101.0, -160.0, -170.0, -165.0],
                ),
                48.0,
            ),
            Variant::Vibrant => p(
                DynamicScheme::get_rotated_hue(
                    source,
                    &[0.0, 38.0, 71.0, 105.0, 140.0, 161.0, 253.0, 333.0, 360.0],
                    &[-72.0, 35.0, 24.0, -24.0, 62.0, 50.0, 62.0, -72.0],
                ),
                56.0,
            ),
            _ => return None,
        },
        PaletteRole::Neutral => match variant {
            Variant::Neutral => p(hue, if phone { 1.4 } else { 6.0 }),
            Variant::TonalSpot => p(hue, if phone { 5.0 } else { 10.0 }),
            Variant::Expressive => {
                p(expressive_neutral_hue_2025(source), expressive_neutral_chroma_2025(source, is_dark, platform))
            }
            Variant::Vibrant => p(vibrant_neutral_hue_2025(source), vibrant_neutral_chroma_2025(source, platform)),
            _ => return None,
        },
        PaletteRole::NeutralVariant => match variant {
            Variant::Neutral => p(hue, (if phone { 1.4 } else { 6.0 }) * 2.2),
            Variant::TonalSpot => p(hue, (if phone { 5.0 } else { 10.0 }) * 1.7),
            Variant::Expressive => {
                let neutral_hue = expressive_neutral_hue_2025(source);
                let neutral_chroma = expressive_neutral_chroma_2025(source, is_dark, platform);
                p(neutral_hue, neutral_chroma * if (105.0..125.0).contains(&neutral_hue) { 1.6 } else { 2.3 })
            }
            Variant::Vibrant => {
                p(vibrant_neutral_hue_2025(source), vibrant_neutral_chroma_2025(source, platform) * 1.29)
            }
            _ => return None,
        },
    };
    Some(palette)
}

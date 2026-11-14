//! Material Color Utilities' MaterialDynamicColors: the roles of a Material scheme, as the 2021 spec makes them
//! (ColorSpecDelegateImpl2021) and as the 2025 one changes them (ColorSpecDelegateImpl2025) in schemes of that
//! spec.

use super::{
    ContrastCurve, DeltaConstraint, DynamicColor, DynamicColorBuilder, DynamicScheme, Platform, SpecVersion,
    ToneDeltaPair, TonePolarity, Variant,
};
use crate::material_color::dislike::DislikeAnalyzer;
use crate::material_color::hct::Hct;
use crate::material_color::palettes::TonalPalette;
use crate::material_color::utils::math_utils::clamp_double;

/// The roles of a Material color scheme (Material Color Utilities' MaterialDynamicColors), each a
/// [`DynamicColor`] to ask a [`DynamicScheme`] the color of. Its roles follow the scheme's spec version: 2021's,
/// or 2025's where it changes them.
#[derive(Clone, Copy, Debug, Default)]
pub struct MaterialDynamicColors;

const M: MaterialDynamicColors = MaterialDynamicColors;

fn b(name: &'static str) -> DynamicColorBuilder {
    DynamicColor::builder(name)
}

fn curve(low: f64, normal: f64, medium: f64, high: f64) -> Option<ContrastCurve> {
    Some(ContrastCurve::new(low, normal, medium, high))
}

fn pair(
    role_a: DynamicColor,
    role_b: DynamicColor,
    delta: f64,
    polarity: TonePolarity,
    stay_together: bool,
) -> Option<ToneDeltaPair> {
    Some(ToneDeltaPair::new(role_a, role_b, delta, polarity, stay_together, DeltaConstraint::Exact))
}

fn pair_with(
    role_a: DynamicColor,
    role_b: DynamicColor,
    delta: f64,
    polarity: TonePolarity,
    stay_together: bool,
    constraint: DeltaConstraint,
) -> Option<ToneDeltaPair> {
    Some(ToneDeltaPair::new(role_a, role_b, delta, polarity, stay_together, constraint))
}

fn is_fidelity(s: &DynamicScheme) -> bool {
    s.variant == Variant::Fidelity || s.variant == Variant::Content
}

fn is_monochrome(s: &DynamicScheme) -> bool {
    s.variant == Variant::Monochrome
}

fn phone(s: &DynamicScheme) -> bool {
    s.platform == Platform::Phone
}

/// 2021: the tone nearest `tone` (going one way) whose color has the chroma, or as near it as there is.
fn find_desired_chroma_by_tone(hue: f64, chroma: f64, tone: f64, by_decreasing_tone: bool) -> f64 {
    let mut answer = tone;
    let mut closest_to_chroma = Hct::from(hue, chroma, tone);
    if closest_to_chroma.chroma() < chroma {
        let mut chroma_peak = closest_to_chroma.chroma();
        while closest_to_chroma.chroma() < chroma {
            answer += if by_decreasing_tone { -1.0 } else { 1.0 };
            let potential_solution = Hct::from(hue, chroma, answer);
            if chroma_peak > potential_solution.chroma() {
                break;
            }
            if (potential_solution.chroma() - chroma).abs() < 0.4 {
                break;
            }
            let potential_delta = (potential_solution.chroma() - chroma).abs();
            let current_delta = (closest_to_chroma.chroma() - chroma).abs();
            if potential_delta < current_delta {
                closest_to_chroma = potential_solution;
            }
            chroma_peak = chroma_peak.max(potential_solution.chroma());
        }
    }
    answer
}

/// 2025: from `tone`, one way, the first tone with the most chroma (up to the chroma asked for).
fn find_best_tone_for_chroma(hue: f64, chroma: f64, tone: f64, by_decreasing_tone: bool) -> f64 {
    let mut tone = tone;
    let mut answer = tone;
    let mut best_candidate = Hct::from(hue, chroma, answer);
    while best_candidate.chroma() < chroma {
        if !(0.0..=100.0).contains(&tone) {
            break;
        }
        tone += if by_decreasing_tone { -1.0 } else { 1.0 };
        let new_candidate = Hct::from(hue, chroma, tone);
        if best_candidate.chroma() < new_candidate.chroma() {
            best_candidate = new_candidate;
            answer = tone;
        }
    }
    answer
}

/// The lightest tone of the palette's most chroma, within the bounds.
fn t_max_c(palette: &TonalPalette, lower_bound: f64, upper_bound: f64, chroma_multiplier: f64) -> f64 {
    let answer = find_best_tone_for_chroma(palette.hue(), palette.chroma() * chroma_multiplier, 100.0, true);
    clamp_double(lower_bound, upper_bound, answer)
}

fn t_max(palette: &TonalPalette) -> f64 {
    t_max_c(palette, 0.0, 100.0, 1.0)
}

fn t_max_in(palette: &TonalPalette, lower_bound: f64, upper_bound: f64) -> f64 {
    t_max_c(palette, lower_bound, upper_bound, 1.0)
}

/// The darkest tone of the palette's most chroma, within the bounds.
fn t_min_c(palette: &TonalPalette, lower_bound: f64, upper_bound: f64) -> f64 {
    let answer = find_best_tone_for_chroma(palette.hue(), palette.chroma(), 0.0, false);
    clamp_double(lower_bound, upper_bound, answer)
}

/// 2025's contrast curve of a standard contrast.
fn get_curve(default_contrast: f64) -> Option<ContrastCurve> {
    let c = default_contrast;
    if c == 1.5 {
        curve(1.5, 1.5, 3.0, 5.5)
    } else if c == 3.0 {
        curve(3.0, 3.0, 4.5, 7.0)
    } else if c == 4.5 {
        curve(4.5, 4.5, 7.0, 11.0)
    } else if c == 6.0 {
        curve(6.0, 6.0, 7.0, 11.0)
    } else if c == 7.0 {
        curve(7.0, 7.0, 11.0, 21.0)
    } else if c == 9.0 {
        curve(9.0, 9.0, 11.0, 21.0)
    } else if c == 11.0 {
        curve(11.0, 11.0, 21.0, 21.0)
    } else if c == 21.0 {
        curve(21.0, 21.0, 21.0, 21.0)
    } else {
        curve(c, c, 7.0, 21.0)
    }
}

/// 2025's background of a foreground: the highest surface on a phone, the high container on a watch.
fn surface_for(s: &DynamicScheme) -> Option<DynamicColor> {
    Some(if phone(s) { M.highest_surface(s) } else { M.surface_container_high() })
}

/// 2025's chroma multipliers of the neutral foregrounds on a phone.
fn neutral_foreground_chroma_multiplier(s: &DynamicScheme) -> f64 {
    if !phone(s) {
        return 1.0;
    }
    let yellow = Hct::is_yellow(s.neutral_palette.hue());
    match s.variant {
        Variant::Neutral => 2.2,
        Variant::TonalSpot => 1.7,
        Variant::Expressive => {
            if yellow && s.is_dark {
                3.0
            } else if yellow {
                2.3
            } else {
                1.6
            }
        }
        _ => 1.0,
    }
}

/// 2025: the same scheme, light and at standard contrast (what the fixed colors are of).
fn light_standard(s: &DynamicScheme) -> DynamicScheme {
    DynamicScheme::from_scheme(s, false, 0.0)
}

/// 2025: a container's contrast with the surface, at more than standard contrast on a phone.
fn container_curve(s: &DynamicScheme) -> Option<ContrastCurve> {
    if phone(s) && s.contrast_level > 0.0 { get_curve(1.5) } else { None }
}

/// 2025: a container's background, on a phone.
fn container_background(s: &DynamicScheme) -> Option<DynamicColor> {
    if phone(s) { Some(M.highest_surface(s)) } else { None }
}

/// 2025's neutral surface tones: (dark, yellow, vibrant, other).
fn surface_tone(s: &DynamicScheme, dark: f64, yellow: f64, vibrant: f64, other: f64) -> f64 {
    if s.is_dark {
        dark
    } else if Hct::is_yellow(s.neutral_palette.hue()) {
        yellow
    } else if s.variant == Variant::Vibrant {
        vibrant
    } else {
        other
    }
}

/// 2025's neutral surface chroma multipliers: (neutral, tonal spot, expressive yellow, expressive, vibrant).
fn surface_chroma(
    s: &DynamicScheme,
    neutral: f64,
    tonal_spot: f64,
    expressive_yellow: f64,
    expressive: f64,
    vibrant: f64,
) -> f64 {
    match s.variant {
        Variant::Neutral => neutral,
        Variant::TonalSpot => tonal_spot,
        Variant::Expressive => {
            if Hct::is_yellow(s.neutral_palette.hue()) {
                expressive_yellow
            } else {
                expressive
            }
        }
        Variant::Vibrant => vibrant,
        _ => 1.0,
    }
}

impl MaterialDynamicColors {
    /// The surface the most foregrounds are drawn on: the bright one in dark schemes, the dim one in light ones.
    pub fn highest_surface(&self, s: &DynamicScheme) -> DynamicColor {
        if s.is_dark { self.surface_bright() } else { self.surface_dim() }
    }

    /// Every role, in Material Color Utilities' order.
    pub fn all_colors(&self) -> Vec<DynamicColor> {
        vec![
            self.background(),
            self.on_background(),
            self.surface(),
            self.surface_dim(),
            self.surface_bright(),
            self.surface_container_lowest(),
            self.surface_container_low(),
            self.surface_container(),
            self.surface_container_high(),
            self.surface_container_highest(),
            self.on_surface(),
            self.surface_variant(),
            self.on_surface_variant(),
            self.outline(),
            self.outline_variant(),
            self.inverse_surface(),
            self.inverse_on_surface(),
            self.shadow(),
            self.scrim(),
            self.surface_tint(),
            self.primary(),
            self.primary_dim(),
            self.on_primary(),
            self.primary_container(),
            self.on_primary_container(),
            self.inverse_primary(),
            self.primary_fixed(),
            self.primary_fixed_dim(),
            self.on_primary_fixed(),
            self.on_primary_fixed_variant(),
            self.secondary(),
            self.secondary_dim(),
            self.on_secondary(),
            self.secondary_container(),
            self.on_secondary_container(),
            self.secondary_fixed(),
            self.secondary_fixed_dim(),
            self.on_secondary_fixed(),
            self.on_secondary_fixed_variant(),
            self.tertiary(),
            self.tertiary_dim(),
            self.on_tertiary(),
            self.tertiary_container(),
            self.on_tertiary_container(),
            self.tertiary_fixed(),
            self.tertiary_fixed_dim(),
            self.on_tertiary_fixed(),
            self.on_tertiary_fixed_variant(),
            self.error(),
            self.error_dim(),
            self.on_error(),
            self.error_container(),
            self.on_error_container(),
            self.primary_palette_key_color(),
            self.secondary_palette_key_color(),
            self.tertiary_palette_key_color(),
            self.neutral_palette_key_color(),
            self.neutral_variant_palette_key_color(),
            self.error_palette_key_color(),
        ]
    }

    // ---- Palette key colors ----------------------------------------------------------------------------------

    pub fn primary_palette_key_color(&self) -> DynamicColor {
        DynamicColor::from_palette(
            "primary_palette_key_color",
            |s| s.primary_palette,
            |s| s.primary_palette.key_color().tone(),
            false,
        )
    }

    pub fn secondary_palette_key_color(&self) -> DynamicColor {
        DynamicColor::from_palette(
            "secondary_palette_key_color",
            |s| s.secondary_palette,
            |s| s.secondary_palette.key_color().tone(),
            false,
        )
    }

    pub fn tertiary_palette_key_color(&self) -> DynamicColor {
        DynamicColor::from_palette(
            "tertiary_palette_key_color",
            |s| s.tertiary_palette,
            |s| s.tertiary_palette.key_color().tone(),
            false,
        )
    }

    pub fn neutral_palette_key_color(&self) -> DynamicColor {
        DynamicColor::from_palette(
            "neutral_palette_key_color",
            |s| s.neutral_palette,
            |s| s.neutral_palette.key_color().tone(),
            false,
        )
    }

    pub fn neutral_variant_palette_key_color(&self) -> DynamicColor {
        DynamicColor::from_palette(
            "neutral_variant_palette_key_color",
            |s| s.neutral_variant_palette,
            |s| s.neutral_variant_palette.key_color().tone(),
            false,
        )
    }

    pub fn error_palette_key_color(&self) -> DynamicColor {
        DynamicColor::from_palette(
            "error_palette_key_color",
            |s| s.error_palette,
            |s| s.error_palette.key_color().tone(),
            false,
        )
    }

    // ---- Surfaces ----------------------------------------------------------------------------------------------

    pub fn background(&self) -> DynamicColor {
        let color2021 = b("background")
            .palette(|s| s.neutral_palette)
            .tone(|s| if s.is_dark { 6.0 } else { 98.0 })
            .is_background(true)
            .build();
        let color2025 = self.surface().to_builder().name("background").build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn on_background(&self) -> DynamicColor {
        let color2021 = b("on_background")
            .palette(|s| s.neutral_palette)
            .tone(|s| if s.is_dark { 90.0 } else { 10.0 })
            .background(|_| Some(M.background()))
            .contrast_curve(|_| curve(3.0, 3.0, 4.5, 7.0))
            .build();
        let color2025 = self
            .on_surface()
            .to_builder()
            .name("on_background")
            .tone(|s| if s.platform == Platform::Watch { 100.0 } else { M.on_surface().get_tone(s) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn surface(&self) -> DynamicColor {
        let color2021 = b("surface")
            .palette(|s| s.neutral_palette)
            .tone(|s| if s.is_dark { 6.0 } else { 98.0 })
            .is_background(true)
            .build();
        let color2025 = b("surface")
            .palette(|s| s.neutral_palette)
            .tone(|s| if phone(s) { surface_tone(s, 4.0, 99.0, 97.0, 98.0) } else { 0.0 })
            .is_background(true)
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn surface_dim(&self) -> DynamicColor {
        let color2021 = b("surface_dim")
            .palette(|s| s.neutral_palette)
            .tone(|s| if s.is_dark { 6.0 } else { ContrastCurve::new(87.0, 87.0, 80.0, 75.0).get(s.contrast_level) })
            .is_background(true)
            .build();
        let color2025 = b("surface_dim")
            .palette(|s| s.neutral_palette)
            .tone(|s| surface_tone(s, 4.0, 90.0, 85.0, 87.0))
            .is_background(true)
            .chroma_multiplier(|s| if !s.is_dark { surface_chroma(s, 2.5, 1.7, 2.7, 1.75, 1.36) } else { 1.0 })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn surface_bright(&self) -> DynamicColor {
        let color2021 = b("surface_bright")
            .palette(|s| s.neutral_palette)
            .tone(|s| if s.is_dark { ContrastCurve::new(24.0, 24.0, 29.0, 34.0).get(s.contrast_level) } else { 98.0 })
            .is_background(true)
            .build();
        let color2025 = b("surface_bright")
            .palette(|s| s.neutral_palette)
            .tone(|s| surface_tone(s, 18.0, 99.0, 97.0, 98.0))
            .is_background(true)
            .chroma_multiplier(|s| if s.is_dark { surface_chroma(s, 2.5, 1.7, 2.7, 1.75, 1.36) } else { 1.0 })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn surface_container_lowest(&self) -> DynamicColor {
        let color2021 = b("surface_container_lowest")
            .palette(|s| s.neutral_palette)
            .tone(|s| if s.is_dark { ContrastCurve::new(4.0, 4.0, 2.0, 0.0).get(s.contrast_level) } else { 100.0 })
            .is_background(true)
            .build();
        let color2025 = b("surface_container_lowest")
            .palette(|s| s.neutral_palette)
            .tone(|s| if s.is_dark { 0.0 } else { 100.0 })
            .is_background(true)
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn surface_container_low(&self) -> DynamicColor {
        let color2021 = b("surface_container_low")
            .palette(|s| s.neutral_palette)
            .tone(|s| {
                if s.is_dark {
                    ContrastCurve::new(10.0, 10.0, 11.0, 12.0).get(s.contrast_level)
                } else {
                    ContrastCurve::new(96.0, 96.0, 96.0, 95.0).get(s.contrast_level)
                }
            })
            .is_background(true)
            .build();
        let color2025 = b("surface_container_low")
            .palette(|s| s.neutral_palette)
            .tone(|s| if phone(s) { surface_tone(s, 6.0, 98.0, 95.0, 96.0) } else { 15.0 })
            .is_background(true)
            .chroma_multiplier(|s| if phone(s) { surface_chroma(s, 1.3, 1.25, 1.3, 1.15, 1.08) } else { 1.0 })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn surface_container(&self) -> DynamicColor {
        let color2021 = b("surface_container")
            .palette(|s| s.neutral_palette)
            .tone(|s| {
                if s.is_dark {
                    ContrastCurve::new(12.0, 12.0, 16.0, 20.0).get(s.contrast_level)
                } else {
                    ContrastCurve::new(94.0, 94.0, 92.0, 90.0).get(s.contrast_level)
                }
            })
            .is_background(true)
            .build();
        let color2025 = b("surface_container")
            .palette(|s| s.neutral_palette)
            .tone(|s| if phone(s) { surface_tone(s, 9.0, 96.0, 92.0, 94.0) } else { 20.0 })
            .is_background(true)
            .chroma_multiplier(|s| if phone(s) { surface_chroma(s, 1.6, 1.4, 1.6, 1.3, 1.15) } else { 1.0 })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn surface_container_high(&self) -> DynamicColor {
        let color2021 = b("surface_container_high")
            .palette(|s| s.neutral_palette)
            .tone(|s| {
                if s.is_dark {
                    ContrastCurve::new(17.0, 17.0, 21.0, 25.0).get(s.contrast_level)
                } else {
                    ContrastCurve::new(92.0, 92.0, 88.0, 85.0).get(s.contrast_level)
                }
            })
            .is_background(true)
            .build();
        let color2025 = b("surface_container_high")
            .palette(|s| s.neutral_palette)
            .tone(|s| if phone(s) { surface_tone(s, 12.0, 94.0, 90.0, 92.0) } else { 25.0 })
            .is_background(true)
            .chroma_multiplier(|s| if phone(s) { surface_chroma(s, 1.9, 1.5, 1.95, 1.45, 1.22) } else { 1.0 })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn surface_container_highest(&self) -> DynamicColor {
        let color2021 = b("surface_container_highest")
            .palette(|s| s.neutral_palette)
            .tone(|s| {
                if s.is_dark {
                    ContrastCurve::new(22.0, 22.0, 26.0, 30.0).get(s.contrast_level)
                } else {
                    ContrastCurve::new(90.0, 90.0, 84.0, 80.0).get(s.contrast_level)
                }
            })
            .is_background(true)
            .build();
        let color2025 = b("surface_container_highest")
            .palette(|s| s.neutral_palette)
            .tone(|s| surface_tone(s, 15.0, 92.0, 88.0, 90.0))
            .is_background(true)
            .chroma_multiplier(|s| surface_chroma(s, 2.2, 1.7, 2.3, 1.6, 1.29))
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn on_surface(&self) -> DynamicColor {
        let color2021 = b("on_surface")
            .palette(|s| s.neutral_palette)
            .tone(|s| if s.is_dark { 90.0 } else { 10.0 })
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(4.5, 7.0, 11.0, 21.0))
            .build();
        let color2025 = b("on_surface")
            .palette(|s| s.neutral_palette)
            .tone(|s| {
                if s.variant == Variant::Vibrant {
                    t_max_c(&s.neutral_palette, 0.0, 100.0, 1.1)
                } else {
                    // (getInitialToneFromBackground of the 2025 background.)
                    surface_for(s).map_or(50.0, |bg| bg.get_tone(s))
                }
            })
            .chroma_multiplier(neutral_foreground_chroma_multiplier)
            .background(surface_for)
            .contrast_curve(|s| if s.is_dark && phone(s) { get_curve(11.0) } else { get_curve(9.0) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn surface_variant(&self) -> DynamicColor {
        let color2021 = b("surface_variant")
            .palette(|s| s.neutral_variant_palette)
            .tone(|s| if s.is_dark { 30.0 } else { 90.0 })
            .is_background(true)
            .build();
        let color2025 = self.surface_container_highest().to_builder().name("surface_variant").build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn on_surface_variant(&self) -> DynamicColor {
        let color2021 = b("on_surface_variant")
            .palette(|s| s.neutral_variant_palette)
            .tone(|s| if s.is_dark { 80.0 } else { 30.0 })
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(3.0, 4.5, 7.0, 11.0))
            .build();
        let color2025 = b("on_surface_variant")
            .palette(|s| s.neutral_palette)
            .chroma_multiplier(neutral_foreground_chroma_multiplier)
            .background(surface_for)
            .contrast_curve(|s| {
                if phone(s) && s.is_dark {
                    get_curve(6.0)
                } else if phone(s) {
                    get_curve(4.5)
                } else {
                    get_curve(7.0)
                }
            })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn inverse_surface(&self) -> DynamicColor {
        let color2021 = b("inverse_surface")
            .palette(|s| s.neutral_palette)
            .tone(|s| if s.is_dark { 90.0 } else { 20.0 })
            .is_background(true)
            .build();
        let color2025 = b("inverse_surface")
            .palette(|s| s.neutral_palette)
            .tone(|s| if s.is_dark { 98.0 } else { 4.0 })
            .is_background(true)
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn inverse_on_surface(&self) -> DynamicColor {
        let color2021 = b("inverse_on_surface")
            .palette(|s| s.neutral_palette)
            .tone(|s| if s.is_dark { 20.0 } else { 95.0 })
            .background(|_| Some(M.inverse_surface()))
            .contrast_curve(|_| curve(4.5, 7.0, 11.0, 21.0))
            .build();
        let color2025 = b("inverse_on_surface")
            .palette(|s| s.neutral_palette)
            .background(|_| Some(M.inverse_surface()))
            .contrast_curve(|_| get_curve(7.0))
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn outline(&self) -> DynamicColor {
        let color2021 = b("outline")
            .palette(|s| s.neutral_variant_palette)
            .tone(|s| if s.is_dark { 60.0 } else { 50.0 })
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(1.5, 3.0, 4.5, 7.0))
            .build();
        let color2025 = b("outline")
            .palette(|s| s.neutral_palette)
            .chroma_multiplier(neutral_foreground_chroma_multiplier)
            .background(surface_for)
            .contrast_curve(|s| if phone(s) { get_curve(3.0) } else { get_curve(4.5) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn outline_variant(&self) -> DynamicColor {
        let color2021 = b("outline_variant")
            .palette(|s| s.neutral_variant_palette)
            .tone(|s| if s.is_dark { 30.0 } else { 80.0 })
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(1.0, 1.0, 3.0, 4.5))
            .build();
        let color2025 = b("outline_variant")
            .palette(|s| s.neutral_palette)
            .chroma_multiplier(neutral_foreground_chroma_multiplier)
            .background(surface_for)
            .contrast_curve(|s| if phone(s) { get_curve(1.5) } else { get_curve(3.0) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn shadow(&self) -> DynamicColor {
        DynamicColor::from_palette("shadow", |s| s.neutral_palette, |_| 0.0, false)
    }

    pub fn scrim(&self) -> DynamicColor {
        DynamicColor::from_palette("scrim", |s| s.neutral_palette, |_| 0.0, false)
    }

    pub fn surface_tint(&self) -> DynamicColor {
        let color2021 = b("surface_tint")
            .palette(|s| s.primary_palette)
            .tone(|s| if s.is_dark { 80.0 } else { 40.0 })
            .is_background(true)
            .build();
        let color2025 = self.primary().to_builder().name("surface_tint").build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    // ---- Primary -----------------------------------------------------------------------------------------------

    pub fn primary(&self) -> DynamicColor {
        let color2021 = b("primary")
            .palette(|s| s.primary_palette)
            .tone(|s| {
                if is_monochrome(s) {
                    if s.is_dark { 100.0 } else { 0.0 }
                } else if s.is_dark {
                    80.0
                } else {
                    40.0
                }
            })
            .is_background(true)
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(3.0, 4.5, 7.0, 7.0))
            .tone_delta_pair(|_| pair(M.primary_container(), M.primary(), 10.0, TonePolarity::Nearer, false))
            .build();
        let color2025 = b("primary")
            .palette(|s| s.primary_palette)
            .tone(|s| match s.variant {
                Variant::Neutral => {
                    if phone(s) && s.is_dark {
                        80.0
                    } else if phone(s) {
                        40.0
                    } else {
                        90.0
                    }
                }
                Variant::TonalSpot => {
                    if phone(s) && s.is_dark {
                        80.0
                    } else if phone(s) {
                        t_max(&s.primary_palette)
                    } else {
                        t_max_in(&s.primary_palette, 0.0, 90.0)
                    }
                }
                Variant::Expressive => {
                    if phone(s) {
                        let hue = s.primary_palette.hue();
                        let upper = if Hct::is_yellow(hue) {
                            25.0
                        } else if Hct::is_cyan(hue) {
                            88.0
                        } else {
                            98.0
                        };
                        t_max_in(&s.primary_palette, 0.0, upper)
                    } else {
                        t_max(&s.primary_palette)
                    }
                }
                Variant::Vibrant => {
                    if phone(s) {
                        let upper = if Hct::is_cyan(s.primary_palette.hue()) { 88.0 } else { 98.0 };
                        t_max_in(&s.primary_palette, 0.0, upper)
                    } else {
                        t_max(&s.primary_palette)
                    }
                }
                _ => 0.0,
            })
            .is_background(true)
            .background(surface_for)
            .contrast_curve(|s| if phone(s) { get_curve(4.5) } else { get_curve(7.0) })
            .tone_delta_pair(|s| {
                if phone(s) {
                    pair_with(
                        M.primary_container(),
                        M.primary(),
                        5.0,
                        TonePolarity::RelativeLighter,
                        true,
                        DeltaConstraint::Farther,
                    )
                } else {
                    None
                }
            })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    /// 2025 (a watch's): primary, dimmer.
    pub fn primary_dim(&self) -> DynamicColor {
        b("primary_dim")
            .palette(|s| s.primary_palette)
            .tone(|s| match s.variant {
                Variant::Neutral => 85.0,
                Variant::TonalSpot => t_max_in(&s.primary_palette, 0.0, 90.0),
                _ => t_max(&s.primary_palette),
            })
            .is_background(true)
            .background(|_| Some(M.surface_container_high()))
            .contrast_curve(|_| get_curve(4.5))
            .tone_delta_pair(|_| {
                pair_with(M.primary_dim(), M.primary(), 5.0, TonePolarity::Darker, true, DeltaConstraint::Farther)
            })
            .build()
    }

    pub fn on_primary(&self) -> DynamicColor {
        let color2021 = b("on_primary")
            .palette(|s| s.primary_palette)
            .tone(|s| {
                if is_monochrome(s) {
                    if s.is_dark { 10.0 } else { 90.0 }
                } else if s.is_dark {
                    20.0
                } else {
                    100.0
                }
            })
            .background(|_| Some(M.primary()))
            .contrast_curve(|_| curve(4.5, 7.0, 11.0, 21.0))
            .build();
        let color2025 = b("on_primary")
            .palette(|s| s.primary_palette)
            .background(|s| Some(if phone(s) { M.primary() } else { M.primary_dim() }))
            .contrast_curve(|s| if phone(s) { get_curve(6.0) } else { get_curve(7.0) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn primary_container(&self) -> DynamicColor {
        let color2021 = b("primary_container")
            .palette(|s| s.primary_palette)
            .tone(|s| {
                if is_fidelity(s) {
                    s.source_color_hct.tone()
                } else if is_monochrome(s) {
                    if s.is_dark { 85.0 } else { 25.0 }
                } else if s.is_dark {
                    30.0
                } else {
                    90.0
                }
            })
            .is_background(true)
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(1.0, 1.0, 3.0, 4.5))
            .tone_delta_pair(|_| pair(M.primary_container(), M.primary(), 10.0, TonePolarity::Nearer, false))
            .build();
        let color2025 = b("primary_container")
            .palette(|s| s.primary_palette)
            .tone(|s| {
                let p = &s.primary_palette;
                if s.platform == Platform::Watch {
                    return 30.0;
                }
                match s.variant {
                    Variant::Neutral => {
                        if s.is_dark {
                            30.0
                        } else {
                            90.0
                        }
                    }
                    Variant::TonalSpot => {
                        if s.is_dark {
                            t_min_c(p, 35.0, 93.0)
                        } else {
                            t_max_in(p, 0.0, 90.0)
                        }
                    }
                    Variant::Expressive => {
                        if s.is_dark {
                            t_max_in(p, 30.0, 93.0)
                        } else if Hct::is_cyan(p.hue()) {
                            t_max_in(p, 78.0, 88.0)
                        } else {
                            t_max_in(p, 78.0, 90.0)
                        }
                    }
                    Variant::Vibrant => {
                        if s.is_dark {
                            t_min_c(p, 66.0, 93.0)
                        } else if Hct::is_cyan(p.hue()) {
                            t_max_in(p, 66.0, 88.0)
                        } else {
                            t_max_in(p, 66.0, 93.0)
                        }
                    }
                    _ => 0.0,
                }
            })
            .is_background(true)
            .background(container_background)
            .tone_delta_pair(|s| {
                if phone(s) {
                    None
                } else {
                    pair_with(
                        M.primary_container(),
                        M.primary_dim(),
                        10.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Farther,
                    )
                }
            })
            .contrast_curve(container_curve)
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn on_primary_container(&self) -> DynamicColor {
        let color2021 = b("on_primary_container")
            .palette(|s| s.primary_palette)
            .tone(|s| {
                if is_fidelity(s) {
                    DynamicColor::foreground_tone((M.primary_container().tone)(s), 4.5)
                } else if is_monochrome(s) {
                    if s.is_dark { 0.0 } else { 100.0 }
                } else if s.is_dark {
                    90.0
                } else {
                    30.0
                }
            })
            .background(|_| Some(M.primary_container()))
            .contrast_curve(|_| curve(3.0, 4.5, 7.0, 11.0))
            .build();
        let color2025 = b("on_primary_container")
            .palette(|s| s.primary_palette)
            .background(|_| Some(M.primary_container()))
            .contrast_curve(|s| if phone(s) { get_curve(6.0) } else { get_curve(7.0) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn inverse_primary(&self) -> DynamicColor {
        let color2021 = b("inverse_primary")
            .palette(|s| s.primary_palette)
            .tone(|s| if s.is_dark { 40.0 } else { 80.0 })
            .background(|_| Some(M.inverse_surface()))
            .contrast_curve(|_| curve(3.0, 4.5, 7.0, 7.0))
            .build();
        let color2025 = b("inverse_primary")
            .palette(|s| s.primary_palette)
            .tone(|s| t_max(&s.primary_palette))
            .background(|_| Some(M.inverse_surface()))
            .contrast_curve(|s| if phone(s) { get_curve(6.0) } else { get_curve(7.0) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    // ---- Secondary ---------------------------------------------------------------------------------------------

    pub fn secondary(&self) -> DynamicColor {
        let color2021 = b("secondary")
            .palette(|s| s.secondary_palette)
            .tone(|s| if s.is_dark { 80.0 } else { 40.0 })
            .is_background(true)
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(3.0, 4.5, 7.0, 7.0))
            .tone_delta_pair(|_| pair(M.secondary_container(), M.secondary(), 10.0, TonePolarity::Nearer, false))
            .build();
        let color2025 = b("secondary")
            .palette(|s| s.secondary_palette)
            .tone(|s| {
                let p = &s.secondary_palette;
                if s.platform == Platform::Watch {
                    return if s.variant == Variant::Neutral { 90.0 } else { t_max_in(p, 0.0, 90.0) };
                }
                match s.variant {
                    Variant::Neutral => {
                        if s.is_dark {
                            t_min_c(p, 0.0, 98.0)
                        } else {
                            t_max(p)
                        }
                    }
                    Variant::Vibrant => {
                        if s.is_dark {
                            t_max_in(p, 0.0, 90.0)
                        } else {
                            t_max_in(p, 0.0, 98.0)
                        }
                    }
                    // Expressive and tonal spot.
                    _ => {
                        if s.is_dark {
                            80.0
                        } else {
                            t_max(p)
                        }
                    }
                }
            })
            .is_background(true)
            .background(surface_for)
            .contrast_curve(|s| if phone(s) { get_curve(4.5) } else { get_curve(7.0) })
            .tone_delta_pair(|s| {
                if phone(s) {
                    pair_with(
                        M.secondary_container(),
                        M.secondary(),
                        5.0,
                        TonePolarity::RelativeLighter,
                        true,
                        DeltaConstraint::Farther,
                    )
                } else {
                    None
                }
            })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    /// 2025 (a watch's): secondary, dimmer.
    pub fn secondary_dim(&self) -> DynamicColor {
        b("secondary_dim")
            .palette(|s| s.secondary_palette)
            .tone(|s| if s.variant == Variant::Neutral { 85.0 } else { t_max_in(&s.secondary_palette, 0.0, 90.0) })
            .is_background(true)
            .background(|_| Some(M.surface_container_high()))
            .contrast_curve(|_| get_curve(4.5))
            .tone_delta_pair(|_| {
                pair_with(M.secondary_dim(), M.secondary(), 5.0, TonePolarity::Darker, true, DeltaConstraint::Farther)
            })
            .build()
    }

    pub fn on_secondary(&self) -> DynamicColor {
        let color2021 = b("on_secondary")
            .palette(|s| s.secondary_palette)
            .tone(|s| {
                if is_monochrome(s) {
                    if s.is_dark { 10.0 } else { 100.0 }
                } else if s.is_dark {
                    20.0
                } else {
                    100.0
                }
            })
            .background(|_| Some(M.secondary()))
            .contrast_curve(|_| curve(4.5, 7.0, 11.0, 21.0))
            .build();
        let color2025 = b("on_secondary")
            .palette(|s| s.secondary_palette)
            .background(|s| Some(if phone(s) { M.secondary() } else { M.secondary_dim() }))
            .contrast_curve(|s| if phone(s) { get_curve(6.0) } else { get_curve(7.0) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn secondary_container(&self) -> DynamicColor {
        let color2021 = b("secondary_container")
            .palette(|s| s.secondary_palette)
            .tone(|s| {
                let initial_tone = if s.is_dark { 30.0 } else { 90.0 };
                if is_monochrome(s) {
                    if s.is_dark { 30.0 } else { 85.0 }
                } else if is_fidelity(s) {
                    find_desired_chroma_by_tone(
                        s.secondary_palette.hue(),
                        s.secondary_palette.chroma(),
                        initial_tone,
                        !s.is_dark,
                    )
                } else {
                    initial_tone
                }
            })
            .is_background(true)
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(1.0, 1.0, 3.0, 4.5))
            .tone_delta_pair(|_| pair(M.secondary_container(), M.secondary(), 10.0, TonePolarity::Nearer, false))
            .build();
        let color2025 = b("secondary_container")
            .palette(|s| s.secondary_palette)
            .tone(|s| {
                let p = &s.secondary_palette;
                if s.platform == Platform::Watch {
                    30.0
                } else if s.variant == Variant::Vibrant {
                    if s.is_dark { t_min_c(p, 30.0, 40.0) } else { t_max_in(p, 84.0, 90.0) }
                } else if s.variant == Variant::Expressive {
                    if s.is_dark { 15.0 } else { t_max_in(p, 90.0, 95.0) }
                } else if s.is_dark {
                    25.0
                } else {
                    90.0
                }
            })
            .is_background(true)
            .background(container_background)
            .tone_delta_pair(|s| {
                if s.platform == Platform::Watch {
                    pair_with(
                        M.secondary_container(),
                        M.secondary_dim(),
                        10.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Farther,
                    )
                } else {
                    None
                }
            })
            .contrast_curve(container_curve)
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn on_secondary_container(&self) -> DynamicColor {
        let color2021 = b("on_secondary_container")
            .palette(|s| s.secondary_palette)
            .tone(|s| {
                if is_monochrome(s) {
                    if s.is_dark { 90.0 } else { 10.0 }
                } else if is_fidelity(s) {
                    DynamicColor::foreground_tone((M.secondary_container().tone)(s), 4.5)
                } else if s.is_dark {
                    90.0
                } else {
                    30.0
                }
            })
            .background(|_| Some(M.secondary_container()))
            .contrast_curve(|_| curve(3.0, 4.5, 7.0, 11.0))
            .build();
        let color2025 = b("on_secondary_container")
            .palette(|s| s.secondary_palette)
            .background(|_| Some(M.secondary_container()))
            .contrast_curve(|s| if phone(s) { get_curve(6.0) } else { get_curve(7.0) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    // ---- Tertiary ----------------------------------------------------------------------------------------------

    pub fn tertiary(&self) -> DynamicColor {
        let color2021 = b("tertiary")
            .palette(|s| s.tertiary_palette)
            .tone(|s| {
                if is_monochrome(s) {
                    if s.is_dark { 90.0 } else { 25.0 }
                } else if s.is_dark {
                    80.0
                } else {
                    40.0
                }
            })
            .is_background(true)
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(3.0, 4.5, 7.0, 7.0))
            .tone_delta_pair(|_| pair(M.tertiary_container(), M.tertiary(), 10.0, TonePolarity::Nearer, false))
            .build();
        let color2025 = b("tertiary")
            .palette(|s| s.tertiary_palette)
            .tone(|s| {
                let p = &s.tertiary_palette;
                if s.platform == Platform::Watch {
                    return if s.variant == Variant::TonalSpot { t_max_in(p, 0.0, 90.0) } else { t_max(p) };
                }
                match s.variant {
                    Variant::Expressive | Variant::Vibrant => {
                        if Hct::is_cyan(p.hue()) {
                            t_max_in(p, 0.0, 88.0)
                        } else if s.is_dark {
                            t_max_in(p, 0.0, 98.0)
                        } else {
                            t_max_in(p, 0.0, 100.0)
                        }
                    }
                    // Neutral and tonal spot.
                    _ => {
                        if s.is_dark {
                            t_max_in(p, 0.0, 98.0)
                        } else {
                            t_max(p)
                        }
                    }
                }
            })
            .is_background(true)
            .background(surface_for)
            .contrast_curve(|s| if phone(s) { get_curve(4.5) } else { get_curve(7.0) })
            .tone_delta_pair(|s| {
                if phone(s) {
                    pair_with(
                        M.tertiary_container(),
                        M.tertiary(),
                        5.0,
                        TonePolarity::RelativeLighter,
                        true,
                        DeltaConstraint::Farther,
                    )
                } else {
                    None
                }
            })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    /// 2025 (a watch's): tertiary, dimmer.
    pub fn tertiary_dim(&self) -> DynamicColor {
        b("tertiary_dim")
            .palette(|s| s.tertiary_palette)
            .tone(|s| {
                if s.variant == Variant::TonalSpot {
                    t_max_in(&s.tertiary_palette, 0.0, 90.0)
                } else {
                    t_max(&s.tertiary_palette)
                }
            })
            .is_background(true)
            .background(|_| Some(M.surface_container_high()))
            .contrast_curve(|_| get_curve(4.5))
            .tone_delta_pair(|_| {
                pair_with(M.tertiary_dim(), M.tertiary(), 5.0, TonePolarity::Darker, true, DeltaConstraint::Farther)
            })
            .build()
    }

    pub fn on_tertiary(&self) -> DynamicColor {
        let color2021 = b("on_tertiary")
            .palette(|s| s.tertiary_palette)
            .tone(|s| {
                if is_monochrome(s) {
                    if s.is_dark { 10.0 } else { 90.0 }
                } else if s.is_dark {
                    20.0
                } else {
                    100.0
                }
            })
            .background(|_| Some(M.tertiary()))
            .contrast_curve(|_| curve(4.5, 7.0, 11.0, 21.0))
            .build();
        let color2025 = b("on_tertiary")
            .palette(|s| s.tertiary_palette)
            .background(|s| Some(if phone(s) { M.tertiary() } else { M.tertiary_dim() }))
            .contrast_curve(|s| if phone(s) { get_curve(6.0) } else { get_curve(7.0) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn tertiary_container(&self) -> DynamicColor {
        let color2021 = b("tertiary_container")
            .palette(|s| s.tertiary_palette)
            .tone(|s| {
                if is_monochrome(s) {
                    if s.is_dark { 60.0 } else { 49.0 }
                } else if is_fidelity(s) {
                    DislikeAnalyzer::fix_if_disliked(s.tertiary_palette.get_hct(s.source_color_hct.tone())).tone()
                } else if s.is_dark {
                    30.0
                } else {
                    90.0
                }
            })
            .is_background(true)
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(1.0, 1.0, 3.0, 4.5))
            .tone_delta_pair(|_| pair(M.tertiary_container(), M.tertiary(), 10.0, TonePolarity::Nearer, false))
            .build();
        let color2025 = b("tertiary_container")
            .palette(|s| s.tertiary_palette)
            .tone(|s| {
                let p = &s.tertiary_palette;
                if s.platform == Platform::Watch {
                    return if s.variant == Variant::TonalSpot { t_max_in(p, 0.0, 90.0) } else { t_max(p) };
                }
                match s.variant {
                    Variant::Neutral => {
                        if s.is_dark {
                            t_max_in(p, 0.0, 93.0)
                        } else {
                            t_max_in(p, 0.0, 96.0)
                        }
                    }
                    Variant::TonalSpot => {
                        if s.is_dark {
                            t_max_in(p, 0.0, 93.0)
                        } else {
                            t_max_in(p, 0.0, 100.0)
                        }
                    }
                    Variant::Expressive => {
                        if Hct::is_cyan(p.hue()) {
                            t_max_in(p, 75.0, 88.0)
                        } else if s.is_dark {
                            t_max_in(p, 75.0, 93.0)
                        } else {
                            t_max_in(p, 75.0, 100.0)
                        }
                    }
                    Variant::Vibrant => {
                        if s.is_dark {
                            t_max_in(p, 0.0, 93.0)
                        } else {
                            t_max_in(p, 72.0, 100.0)
                        }
                    }
                    _ => 0.0,
                }
            })
            .is_background(true)
            .background(container_background)
            .tone_delta_pair(|s| {
                if s.platform == Platform::Watch {
                    pair_with(
                        M.tertiary_container(),
                        M.tertiary_dim(),
                        10.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Farther,
                    )
                } else {
                    None
                }
            })
            .contrast_curve(container_curve)
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn on_tertiary_container(&self) -> DynamicColor {
        let color2021 = b("on_tertiary_container")
            .palette(|s| s.tertiary_palette)
            .tone(|s| {
                if is_monochrome(s) {
                    if s.is_dark { 0.0 } else { 100.0 }
                } else if is_fidelity(s) {
                    DynamicColor::foreground_tone((M.tertiary_container().tone)(s), 4.5)
                } else if s.is_dark {
                    90.0
                } else {
                    30.0
                }
            })
            .background(|_| Some(M.tertiary_container()))
            .contrast_curve(|_| curve(3.0, 4.5, 7.0, 11.0))
            .build();
        let color2025 = b("on_tertiary_container")
            .palette(|s| s.tertiary_palette)
            .background(|_| Some(M.tertiary_container()))
            .contrast_curve(|s| if phone(s) { get_curve(6.0) } else { get_curve(7.0) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    // ---- Error -------------------------------------------------------------------------------------------------

    pub fn error(&self) -> DynamicColor {
        let color2021 = b("error")
            .palette(|s| s.error_palette)
            .tone(|s| if s.is_dark { 80.0 } else { 40.0 })
            .is_background(true)
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(3.0, 4.5, 7.0, 7.0))
            .tone_delta_pair(|_| pair(M.error_container(), M.error(), 10.0, TonePolarity::Nearer, false))
            .build();
        let color2025 = b("error")
            .palette(|s| s.error_palette)
            .tone(|s| {
                if phone(s) {
                    if s.is_dark { t_min_c(&s.error_palette, 0.0, 98.0) } else { t_max(&s.error_palette) }
                } else {
                    t_min_c(&s.error_palette, 0.0, 100.0)
                }
            })
            .is_background(true)
            .background(surface_for)
            .contrast_curve(|s| if phone(s) { get_curve(4.5) } else { get_curve(7.0) })
            .tone_delta_pair(|s| {
                if phone(s) {
                    pair_with(
                        M.error_container(),
                        M.error(),
                        5.0,
                        TonePolarity::RelativeLighter,
                        true,
                        DeltaConstraint::Farther,
                    )
                } else {
                    None
                }
            })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    /// 2025 (a watch's): error, dimmer.
    pub fn error_dim(&self) -> DynamicColor {
        b("error_dim")
            .palette(|s| s.error_palette)
            .tone(|s| t_min_c(&s.error_palette, 0.0, 100.0))
            .is_background(true)
            .background(|_| Some(M.surface_container_high()))
            .contrast_curve(|_| get_curve(4.5))
            .tone_delta_pair(|_| {
                pair_with(M.error_dim(), M.error(), 5.0, TonePolarity::Darker, true, DeltaConstraint::Farther)
            })
            .build()
    }

    pub fn on_error(&self) -> DynamicColor {
        let color2021 = b("on_error")
            .palette(|s| s.error_palette)
            .tone(|s| if s.is_dark { 20.0 } else { 100.0 })
            .background(|_| Some(M.error()))
            .contrast_curve(|_| curve(4.5, 7.0, 11.0, 21.0))
            .build();
        let color2025 = b("on_error")
            .palette(|s| s.error_palette)
            .background(|s| Some(if phone(s) { M.error() } else { M.error_dim() }))
            .contrast_curve(|s| if phone(s) { get_curve(6.0) } else { get_curve(7.0) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn error_container(&self) -> DynamicColor {
        let color2021 = b("error_container")
            .palette(|s| s.error_palette)
            .tone(|s| if s.is_dark { 30.0 } else { 90.0 })
            .is_background(true)
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(1.0, 1.0, 3.0, 4.5))
            .tone_delta_pair(|_| pair(M.error_container(), M.error(), 10.0, TonePolarity::Nearer, false))
            .build();
        let color2025 = b("error_container")
            .palette(|s| s.error_palette)
            .tone(|s| {
                if s.platform == Platform::Watch {
                    30.0
                } else if s.is_dark {
                    t_min_c(&s.error_palette, 30.0, 93.0)
                } else {
                    t_max_in(&s.error_palette, 0.0, 90.0)
                }
            })
            .is_background(true)
            .background(container_background)
            .tone_delta_pair(|s| {
                if s.platform == Platform::Watch {
                    pair_with(
                        M.error_container(),
                        M.error_dim(),
                        10.0,
                        TonePolarity::Darker,
                        true,
                        DeltaConstraint::Farther,
                    )
                } else {
                    None
                }
            })
            .contrast_curve(container_curve)
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    pub fn on_error_container(&self) -> DynamicColor {
        let color2021 = b("on_error_container")
            .palette(|s| s.error_palette)
            .tone(|s| {
                if is_monochrome(s) {
                    if s.is_dark { 90.0 } else { 10.0 }
                } else if s.is_dark {
                    90.0
                } else {
                    30.0
                }
            })
            .background(|_| Some(M.error_container()))
            .contrast_curve(|_| curve(3.0, 4.5, 7.0, 11.0))
            .build();
        let color2025 = b("on_error_container")
            .palette(|s| s.error_palette)
            .background(|_| Some(M.error_container()))
            .contrast_curve(|s| if phone(s) { get_curve(4.5) } else { get_curve(7.0) })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    // ---- Fixed colors ------------------------------------------------------------------------------------------

    pub fn primary_fixed(&self) -> DynamicColor {
        self.fixed(
            "primary_fixed",
            |s| s.primary_palette,
            |s| if is_monochrome(s) { 40.0 } else { 90.0 },
            || M.primary_fixed(),
            || M.primary_fixed_dim(),
            || M.primary_container(),
        )
    }

    pub fn primary_fixed_dim(&self) -> DynamicColor {
        self.fixed_dim(
            "primary_fixed_dim",
            |s| s.primary_palette,
            |s| if is_monochrome(s) { 30.0 } else { 80.0 },
            || M.primary_fixed(),
            || M.primary_fixed_dim(),
        )
    }

    pub fn on_primary_fixed(&self) -> DynamicColor {
        self.on_fixed(
            "on_primary_fixed",
            |s| s.primary_palette,
            |s| if is_monochrome(s) { 100.0 } else { 10.0 },
            || M.primary_fixed(),
            || M.primary_fixed_dim(),
        )
    }

    pub fn on_primary_fixed_variant(&self) -> DynamicColor {
        self.on_fixed_variant(
            "on_primary_fixed_variant",
            |s| s.primary_palette,
            |s| if is_monochrome(s) { 90.0 } else { 30.0 },
            || M.primary_fixed(),
            || M.primary_fixed_dim(),
        )
    }

    pub fn secondary_fixed(&self) -> DynamicColor {
        self.fixed(
            "secondary_fixed",
            |s| s.secondary_palette,
            |s| if is_monochrome(s) { 80.0 } else { 90.0 },
            || M.secondary_fixed(),
            || M.secondary_fixed_dim(),
            || M.secondary_container(),
        )
    }

    pub fn secondary_fixed_dim(&self) -> DynamicColor {
        self.fixed_dim(
            "secondary_fixed_dim",
            |s| s.secondary_palette,
            |s| if is_monochrome(s) { 70.0 } else { 80.0 },
            || M.secondary_fixed(),
            || M.secondary_fixed_dim(),
        )
    }

    pub fn on_secondary_fixed(&self) -> DynamicColor {
        self.on_fixed(
            "on_secondary_fixed",
            |s| s.secondary_palette,
            |_| 10.0,
            || M.secondary_fixed(),
            || M.secondary_fixed_dim(),
        )
    }

    pub fn on_secondary_fixed_variant(&self) -> DynamicColor {
        self.on_fixed_variant(
            "on_secondary_fixed_variant",
            |s| s.secondary_palette,
            |s| if is_monochrome(s) { 25.0 } else { 30.0 },
            || M.secondary_fixed(),
            || M.secondary_fixed_dim(),
        )
    }

    pub fn tertiary_fixed(&self) -> DynamicColor {
        self.fixed(
            "tertiary_fixed",
            |s| s.tertiary_palette,
            |s| if is_monochrome(s) { 40.0 } else { 90.0 },
            || M.tertiary_fixed(),
            || M.tertiary_fixed_dim(),
            || M.tertiary_container(),
        )
    }

    pub fn tertiary_fixed_dim(&self) -> DynamicColor {
        self.fixed_dim(
            "tertiary_fixed_dim",
            |s| s.tertiary_palette,
            |s| if is_monochrome(s) { 30.0 } else { 80.0 },
            || M.tertiary_fixed(),
            || M.tertiary_fixed_dim(),
        )
    }

    pub fn on_tertiary_fixed(&self) -> DynamicColor {
        self.on_fixed(
            "on_tertiary_fixed",
            |s| s.tertiary_palette,
            |s| if is_monochrome(s) { 100.0 } else { 10.0 },
            || M.tertiary_fixed(),
            || M.tertiary_fixed_dim(),
        )
    }

    pub fn on_tertiary_fixed_variant(&self) -> DynamicColor {
        self.on_fixed_variant(
            "on_tertiary_fixed_variant",
            |s| s.tertiary_palette,
            |s| if is_monochrome(s) { 90.0 } else { 30.0 },
            || M.tertiary_fixed(),
            || M.tertiary_fixed_dim(),
        )
    }

    /// A fixed color (the same light or dark): 2021, the tone given, a delta of 10 lighter than its dim one;
    /// 2025, its container's in the light scheme at standard contrast.
    fn fixed(
        &self,
        name: &'static str,
        palette: fn(&DynamicScheme) -> TonalPalette,
        tone: fn(&DynamicScheme) -> f64,
        fixed: fn() -> DynamicColor,
        fixed_dim: fn() -> DynamicColor,
        container: fn() -> DynamicColor,
    ) -> DynamicColor {
        let color2021 = b(name)
            .palette(palette)
            .tone(tone)
            .is_background(true)
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(1.0, 1.0, 3.0, 4.5))
            .tone_delta_pair(move |_| pair(fixed(), fixed_dim(), 10.0, TonePolarity::Lighter, true))
            .build();
        let color2025 = b(name)
            .palette(palette)
            .tone(move |s| container().get_tone(&light_standard(s)))
            .is_background(true)
            .background(container_background)
            .contrast_curve(container_curve)
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    /// A fixed color's dim one: 2021, the tone given, a delta of 10 darker; 2025, exactly 5 darker.
    fn fixed_dim(
        &self,
        name: &'static str,
        palette: fn(&DynamicScheme) -> TonalPalette,
        tone: fn(&DynamicScheme) -> f64,
        fixed: fn() -> DynamicColor,
        fixed_dim: fn() -> DynamicColor,
    ) -> DynamicColor {
        let color2021 = b(name)
            .palette(palette)
            .tone(tone)
            .is_background(true)
            .background(|s| Some(M.highest_surface(s)))
            .contrast_curve(|_| curve(1.0, 1.0, 3.0, 4.5))
            .tone_delta_pair(move |_| pair(fixed(), fixed_dim(), 10.0, TonePolarity::Lighter, true))
            .build();
        let color2025 = b(name)
            .palette(palette)
            .tone(move |s| fixed().get_tone(s))
            .is_background(true)
            .tone_delta_pair(move |_| {
                pair_with(fixed_dim(), fixed(), 5.0, TonePolarity::Darker, true, DeltaConstraint::Exact)
            })
            .build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    /// What is drawn on a fixed color and its dim one: 2021, in contrast with both; 2025, with the dim one, 7:1.
    fn on_fixed(
        &self,
        name: &'static str,
        palette: fn(&DynamicScheme) -> TonalPalette,
        tone: fn(&DynamicScheme) -> f64,
        fixed: fn() -> DynamicColor,
        fixed_dim: fn() -> DynamicColor,
    ) -> DynamicColor {
        let color2021 = b(name)
            .palette(palette)
            .tone(tone)
            .background(move |_| Some(fixed_dim()))
            .second_background(move |_| Some(fixed()))
            .contrast_curve(|_| curve(4.5, 7.0, 11.0, 21.0))
            .build();
        let color2025 =
            b(name).palette(palette).background(move |_| Some(fixed_dim())).contrast_curve(|_| get_curve(7.0)).build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }

    /// The same, a variant of less contrast: 2021, 3:1 to 11:1 with both; 2025, 4.5:1 with the dim one.
    fn on_fixed_variant(
        &self,
        name: &'static str,
        palette: fn(&DynamicScheme) -> TonalPalette,
        tone: fn(&DynamicScheme) -> f64,
        fixed: fn() -> DynamicColor,
        fixed_dim: fn() -> DynamicColor,
    ) -> DynamicColor {
        let color2021 = b(name)
            .palette(palette)
            .tone(tone)
            .background(move |_| Some(fixed_dim()))
            .second_background(move |_| Some(fixed()))
            .contrast_curve(|_| curve(3.0, 4.5, 7.0, 11.0))
            .build();
        let color2025 =
            b(name).palette(palette).background(move |_| Some(fixed_dim())).contrast_curve(|_| get_curve(4.5)).build();
        color2021.extend_spec_version(SpecVersion::Spec2025, color2025)
    }
}

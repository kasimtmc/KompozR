//! Dynamic color (Material Color Utilities' `dynamiccolor`): a scheme ([`DynamicScheme`]) made of a source color,
//! a variant, light or dark and a contrast level, and the colors of its roles ([`MaterialDynamicColors`]), each
//! a palette and a tone kept in contrast with what it is drawn on.

mod dynamic_scheme;
mod material_dynamic_colors;

pub use dynamic_scheme::{DynamicScheme, Platform, SpecVersion, Variant};
pub use material_dynamic_colors::MaterialDynamicColors;

use crate::material_color::contrast::Contrast;
use crate::material_color::hct::Hct;
use crate::material_color::palettes::TonalPalette;
use crate::material_color::utils::math_utils::{clamp_double, lerp, round};
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

/// A function of a scheme, as a dynamic color's parts are.
pub type SchemeFn<T> = Arc<dyn Fn(&DynamicScheme) -> T + Send + Sync>;

/// A contrast ratio for each contrast level (Material Color Utilities' ContrastCurve): at -1 (reduced), 0
/// (standard), 0.5 (medium) and 1 (high), linear between.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContrastCurve {
    pub low: f64,
    pub normal: f64,
    pub medium: f64,
    pub high: f64,
}

impl ContrastCurve {
    pub const fn new(low: f64, normal: f64, medium: f64, high: f64) -> ContrastCurve {
        ContrastCurve { low, normal, medium, high }
    }

    /// The ratio at a contrast level, -1..1.
    pub fn get(&self, contrast_level: f64) -> f64 {
        if contrast_level <= -1.0 {
            self.low
        } else if contrast_level < 0.0 {
            lerp(self.low, self.normal, (contrast_level - -1.0) / 1.0)
        } else if contrast_level < 0.5 {
            lerp(self.normal, self.medium, (contrast_level - 0.0) / 0.5)
        } else if contrast_level < 1.0 {
            lerp(self.medium, self.high, (contrast_level - 0.5) / 0.5)
        } else {
            self.high
        }
    }
}

/// Which way a [`ToneDeltaPair`]'s delta goes (Material Color Utilities' TonePolarity).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TonePolarity {
    /// Role A darker than role B.
    Darker,
    /// Role A lighter than role B.
    Lighter,
    /// Role A nearer the background's tone (2021).
    Nearer,
    /// Role A farther from it (2021).
    Farther,
    /// Darker in light schemes, lighter in dark ones (2025).
    RelativeDarker,
    /// Lighter in light schemes, darker in dark ones (2025).
    RelativeLighter,
}

/// How a [`ToneDeltaPair`]'s delta is kept (Material Color Utilities' DeltaConstraint, 2025).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeltaConstraint {
    /// Exactly the delta apart.
    Exact,
    /// At most the delta apart.
    Nearer,
    /// At least the delta apart.
    Farther,
}

/// Two roles whose tones are kept a delta apart (Material Color Utilities' ToneDeltaPair): a container and the
/// color on it, a fixed color and its dim one.
#[derive(Clone)]
pub struct ToneDeltaPair {
    pub role_a: DynamicColor,
    pub role_b: DynamicColor,
    pub delta: f64,
    pub polarity: TonePolarity,
    /// Whether the two move together out of the awkward 50..59 tones (2021).
    pub stay_together: bool,
    pub constraint: DeltaConstraint,
}

impl ToneDeltaPair {
    pub fn new(
        role_a: DynamicColor,
        role_b: DynamicColor,
        delta: f64,
        polarity: TonePolarity,
        stay_together: bool,
        constraint: DeltaConstraint,
    ) -> ToneDeltaPair {
        ToneDeltaPair { role_a, role_b, delta, polarity, stay_together, constraint }
    }
}

/// A color role of a scheme (Material Color Utilities' DynamicColor): which palette, which tone, and what keeps
/// that tone readable: the background it is drawn on (and a second one) with the contrast it needs there, and the
/// role it is kept a delta apart from. Background roles stay out of the awkward 50..59 tones.
#[derive(Clone)]
pub struct DynamicColor {
    pub name: Cow<'static, str>,
    pub palette: SchemeFn<TonalPalette>,
    pub tone: SchemeFn<f64>,
    pub is_background: bool,
    pub chroma_multiplier: Option<SchemeFn<f64>>,
    pub background: Option<SchemeFn<Option<DynamicColor>>>,
    pub second_background: Option<SchemeFn<Option<DynamicColor>>>,
    pub contrast_curve: Option<SchemeFn<Option<ContrastCurve>>>,
    pub tone_delta_pair: Option<SchemeFn<Option<ToneDeltaPair>>>,
}

impl fmt::Debug for DynamicColor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DynamicColor({})", self.name)
    }
}

/// A dynamic color's parts, set one by one (Material Color Utilities' DynamicColor.Builder).
#[derive(Clone, Default)]
pub struct DynamicColorBuilder {
    name: Cow<'static, str>,
    palette: Option<SchemeFn<TonalPalette>>,
    tone: Option<SchemeFn<f64>>,
    is_background: bool,
    chroma_multiplier: Option<SchemeFn<f64>>,
    background: Option<SchemeFn<Option<DynamicColor>>>,
    second_background: Option<SchemeFn<Option<DynamicColor>>>,
    contrast_curve: Option<SchemeFn<Option<ContrastCurve>>>,
    tone_delta_pair: Option<SchemeFn<Option<ToneDeltaPair>>>,
}

impl DynamicColorBuilder {
    pub fn name(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.name = name.into();
        self
    }
    pub fn palette(mut self, f: impl Fn(&DynamicScheme) -> TonalPalette + Send + Sync + 'static) -> Self {
        self.palette = Some(Arc::new(f));
        self
    }
    pub fn tone(mut self, f: impl Fn(&DynamicScheme) -> f64 + Send + Sync + 'static) -> Self {
        self.tone = Some(Arc::new(f));
        self
    }
    pub fn is_background(mut self, is_background: bool) -> Self {
        self.is_background = is_background;
        self
    }
    pub fn chroma_multiplier(mut self, f: impl Fn(&DynamicScheme) -> f64 + Send + Sync + 'static) -> Self {
        self.chroma_multiplier = Some(Arc::new(f));
        self
    }
    pub fn background(mut self, f: impl Fn(&DynamicScheme) -> Option<DynamicColor> + Send + Sync + 'static) -> Self {
        self.background = Some(Arc::new(f));
        self
    }
    pub fn second_background(
        mut self,
        f: impl Fn(&DynamicScheme) -> Option<DynamicColor> + Send + Sync + 'static,
    ) -> Self {
        self.second_background = Some(Arc::new(f));
        self
    }
    pub fn contrast_curve(
        mut self,
        f: impl Fn(&DynamicScheme) -> Option<ContrastCurve> + Send + Sync + 'static,
    ) -> Self {
        self.contrast_curve = Some(Arc::new(f));
        self
    }
    pub fn tone_delta_pair(
        mut self,
        f: impl Fn(&DynamicScheme) -> Option<ToneDeltaPair> + Send + Sync + 'static,
    ) -> Self {
        self.tone_delta_pair = Some(Arc::new(f));
        self
    }

    /// The color; its tone, unless set, its background's (50 without one). Panics, as Material Color Utilities
    /// throws, for a second background or a contrast curve without a background, or a background without a
    /// contrast curve.
    pub fn build(self) -> DynamicColor {
        if self.background.is_none() && self.second_background.is_some() {
            panic!("Color {} has secondBackground defined, but background is not defined.", self.name);
        }
        if self.background.is_none() && self.contrast_curve.is_some() {
            panic!("Color {} has contrastCurve defined, but background is not defined.", self.name);
        }
        if self.background.is_some() && self.contrast_curve.is_none() {
            panic!("Color {} has background defined, but contrastCurve is not defined.", self.name);
        }
        let tone = self.tone.unwrap_or_else(|| DynamicColor::get_initial_tone_from_background(self.background.clone()));
        DynamicColor {
            palette: self.palette.unwrap_or_else(|| panic!("Color {} has no palette.", self.name)),
            name: self.name,
            tone,
            is_background: self.is_background,
            chroma_multiplier: self.chroma_multiplier,
            background: self.background,
            second_background: self.second_background,
            contrast_curve: self.contrast_curve,
            tone_delta_pair: self.tone_delta_pair,
        }
    }
}

impl DynamicColor {
    /// A color's parts, set one by one.
    pub fn builder(name: impl Into<Cow<'static, str>>) -> DynamicColorBuilder {
        DynamicColorBuilder::default().name(name)
    }

    /// A color of a palette and a tone, nothing kept in contrast (Material Color Utilities' fromPalette).
    pub fn from_palette(
        name: impl Into<Cow<'static, str>>,
        palette: impl Fn(&DynamicScheme) -> TonalPalette + Send + Sync + 'static,
        tone: impl Fn(&DynamicScheme) -> f64 + Send + Sync + 'static,
        is_background: bool,
    ) -> DynamicColor {
        DynamicColor::builder(name).palette(palette).tone(tone).is_background(is_background).build()
    }

    /// Its parts, to change some.
    pub fn to_builder(&self) -> DynamicColorBuilder {
        DynamicColorBuilder {
            name: self.name.clone(),
            palette: Some(self.palette.clone()),
            tone: Some(self.tone.clone()),
            is_background: self.is_background,
            chroma_multiplier: self.chroma_multiplier.clone(),
            background: self.background.clone(),
            second_background: self.second_background.clone(),
            contrast_curve: self.contrast_curve.clone(),
            tone_delta_pair: self.tone_delta_pair.clone(),
        }
    }

    /// This color, but `extended`'s in schemes of `spec_version` (Material Color Utilities' extendSpecVersion):
    /// the same name and background-ness. Panics where they differ.
    pub fn extend_spec_version(&self, spec_version: SpecVersion, extended: DynamicColor) -> DynamicColor {
        if self.name != extended.name {
            panic!(
                "Attempting to extend color {} with color {} of different name for spec version {spec_version:?}.",
                self.name, extended.name
            );
        }
        if self.is_background != extended.is_background {
            panic!(
                "Attempting to extend color {} as a {} with color {} as a {} for spec version {spec_version:?}.",
                self.name,
                if self.is_background { "background" } else { "foreground" },
                extended.name,
                if extended.is_background { "background" } else { "foreground" }
            );
        }
        let (original, extended) = (Arc::new(self.clone()), Arc::new(extended));
        let pick =
            move |s: &DynamicScheme| if s.spec_version == spec_version { extended.clone() } else { original.clone() };
        let (p1, p2, p3, p4, p5, p6, p7) =
            (pick.clone(), pick.clone(), pick.clone(), pick.clone(), pick.clone(), pick.clone(), pick);
        DynamicColor::builder(self.name.clone())
            .is_background(self.is_background)
            .palette(move |s| (p1(s).palette)(s))
            .tone(move |s| (p2(s).tone)(s))
            .chroma_multiplier(move |s| p3(s).chroma_multiplier.as_ref().map_or(1.0, |f| f(s)))
            .background(move |s| p4(s).background.as_ref().and_then(|f| f(s)))
            .second_background(move |s| p5(s).second_background.as_ref().and_then(|f| f(s)))
            .contrast_curve(move |s| p6(s).contrast_curve.as_ref().and_then(|f| f(s)))
            .tone_delta_pair(move |s| p7(s).tone_delta_pair.as_ref().and_then(|f| f(s)))
            .build()
    }

    /// The ARGB color of this role in `scheme`.
    pub fn get_argb(&self, scheme: &DynamicScheme) -> u32 {
        self.get_hct(scheme).to_int()
    }

    /// The color of this role in `scheme`.
    pub fn get_hct(&self, scheme: &DynamicScheme) -> Hct {
        match scheme.spec_version {
            SpecVersion::Spec2021 => (self.palette)(scheme).get_hct(self.get_tone(scheme)),
            SpecVersion::Spec2025 => {
                let palette = (self.palette)(scheme);
                let tone = self.get_tone(scheme);
                let chroma = palette.chroma() * self.chroma_multiplier.as_ref().map_or(1.0, |f| f(scheme));
                Hct::from(palette.hue(), chroma, tone)
            }
        }
    }

    /// The tone of this role in `scheme`, after what keeps it in contrast.
    pub fn get_tone(&self, scheme: &DynamicScheme) -> f64 {
        match scheme.spec_version {
            SpecVersion::Spec2021 => self.get_tone_2021(scheme),
            SpecVersion::Spec2025 => self.get_tone_2025(scheme),
        }
    }

    fn background_of(&self, scheme: &DynamicScheme) -> Option<DynamicColor> {
        self.background.as_ref().and_then(|f| f(scheme))
    }
    fn second_background_of(&self, scheme: &DynamicScheme) -> Option<DynamicColor> {
        self.second_background.as_ref().and_then(|f| f(scheme))
    }
    fn contrast_curve_of(&self, scheme: &DynamicScheme) -> Option<ContrastCurve> {
        self.contrast_curve.as_ref().and_then(|f| f(scheme))
    }

    /// Material Color Utilities' ColorCalculationDelegateImpl2021.getTone.
    fn get_tone_2021(&self, scheme: &DynamicScheme) -> f64 {
        let decreasing_contrast = scheme.contrast_level < 0.0;
        // Case 1: dual foreground, pair of colors with delta constraint.
        if let Some(pair) = self.tone_delta_pair.as_ref().and_then(|f| f(scheme)) {
            let (role_a, role_b) = (&pair.role_a, &pair.role_b);
            let (delta, polarity, stay_together) = (pair.delta, pair.polarity, pair.stay_together);
            let a_is_nearer = polarity == TonePolarity::Nearer
                || (polarity == TonePolarity::Lighter && !scheme.is_dark)
                || (polarity == TonePolarity::Darker && scheme.is_dark);
            let (nearer, farther) = if a_is_nearer { (role_a, role_b) } else { (role_b, role_a) };
            let am_nearer = self.name == nearer.name;
            let expansion_dir = if scheme.is_dark { 1.0 } else { -1.0 };
            let mut n_tone = (nearer.tone)(scheme);
            let mut f_tone = (farther.tone)(scheme);

            // 1st round: solve to min for each, if background and contrast curve are defined.
            if self.background.is_some() && nearer.contrast_curve.is_some() && farther.contrast_curve.is_some() {
                let bg = self.background_of(scheme);
                let n_contrast_curve = nearer.contrast_curve_of(scheme);
                let f_contrast_curve = farther.contrast_curve_of(scheme);
                if let (Some(bg), Some(n_curve), Some(f_curve)) = (bg, n_contrast_curve, f_contrast_curve) {
                    let bg_tone = bg.get_tone(scheme);
                    let n_contrast = n_curve.get(scheme.contrast_level);
                    let f_contrast = f_curve.get(scheme.contrast_level);
                    // If a color is good enough, it is not adjusted.
                    if Contrast::ratio_of_tones(bg_tone, n_tone) < n_contrast {
                        n_tone = DynamicColor::foreground_tone(bg_tone, n_contrast);
                    }
                    if Contrast::ratio_of_tones(bg_tone, f_tone) < f_contrast {
                        f_tone = DynamicColor::foreground_tone(bg_tone, f_contrast);
                    }
                    if decreasing_contrast {
                        // If decreasing contrast, adjust color to the "bare minimum" that satisfies contrast.
                        n_tone = DynamicColor::foreground_tone(bg_tone, n_contrast);
                        f_tone = DynamicColor::foreground_tone(bg_tone, f_contrast);
                    }
                }
            }

            // If the constraint is not satisfied, try another round.
            if (f_tone - n_tone) * expansion_dir < delta {
                // 2nd round: expand farther to match delta.
                f_tone = clamp_double(0.0, 100.0, n_tone + delta * expansion_dir);
                // If the constraint is not satisfied, try another round.
                if (f_tone - n_tone) * expansion_dir < delta {
                    // 3rd round: contract nearer to match delta.
                    n_tone = clamp_double(0.0, 100.0, f_tone - delta * expansion_dir);
                }
            }

            // Avoids the 50-59 awkward zone.
            if (50.0..60.0).contains(&n_tone) {
                // If `nearer` is in the awkward zone, move it away, together with `farther`.
                if expansion_dir > 0.0 {
                    n_tone = 60.0;
                    f_tone = f_tone.max(n_tone + delta * expansion_dir);
                } else {
                    n_tone = 49.0;
                    f_tone = f_tone.min(n_tone + delta * expansion_dir);
                }
            } else if (50.0..60.0).contains(&f_tone) {
                if stay_together {
                    // Fixes both, to avoid two colors on opposite sides of the "awkward zone".
                    if expansion_dir > 0.0 {
                        n_tone = 60.0;
                        f_tone = f_tone.max(n_tone + delta * expansion_dir);
                    } else {
                        n_tone = 49.0;
                        f_tone = f_tone.min(n_tone + delta * expansion_dir);
                    }
                } else {
                    // Not required to stay together; fixes just one.
                    f_tone = if expansion_dir > 0.0 { 60.0 } else { 49.0 };
                }
            }
            return if am_nearer { n_tone } else { f_tone };
        }

        // Case 2: no contrast pair; just solve for itself.
        let mut answer = (self.tone)(scheme);
        let (Some(bg), Some(curve)) = (self.background_of(scheme), self.contrast_curve_of(scheme)) else {
            return answer; // No adjustment for colors with no background.
        };
        let bg_tone = bg.get_tone(scheme);
        let desired_ratio = curve.get(scheme.contrast_level);
        if Contrast::ratio_of_tones(bg_tone, answer) < desired_ratio {
            // (What is good enough is not "improved".)
            answer = DynamicColor::foreground_tone(bg_tone, desired_ratio);
        }
        if decreasing_contrast {
            answer = DynamicColor::foreground_tone(bg_tone, desired_ratio);
        }
        if self.is_background && (50.0..60.0).contains(&answer) {
            answer = if Contrast::ratio_of_tones(49.0, bg_tone) >= desired_ratio { 49.0 } else { 60.0 };
        }
        self.solve_second_background(scheme, &bg, answer, desired_ratio)
    }

    /// Material Color Utilities' ColorCalculationDelegateImpl2025.getTone.
    fn get_tone_2025(&self, scheme: &DynamicScheme) -> f64 {
        // Case 0: tone delta constraint.
        if let Some(pair) = self.tone_delta_pair.as_ref().and_then(|f| f(scheme)) {
            let (role_a, role_b, polarity, constraint) = (&pair.role_a, &pair.role_b, pair.polarity, pair.constraint);
            let absolute_delta = if polarity == TonePolarity::Darker
                || (polarity == TonePolarity::RelativeLighter && scheme.is_dark)
                || (polarity == TonePolarity::RelativeDarker && !scheme.is_dark)
            {
                -pair.delta
            } else {
                pair.delta
            };
            let am_role_a = self.name == role_a.name;
            let (self_role, ref_role) = if am_role_a { (role_a, role_b) } else { (role_b, role_a) };
            let mut self_tone = (self_role.tone)(scheme);
            let ref_tone = ref_role.get_tone(scheme);
            let relative_delta = absolute_delta * if am_role_a { 1.0 } else { -1.0 };
            self_tone = match constraint {
                DeltaConstraint::Exact => clamp_double(0.0, 100.0, ref_tone + relative_delta),
                DeltaConstraint::Nearer => {
                    if relative_delta > 0.0 {
                        clamp_double(0.0, 100.0, clamp_double(ref_tone, ref_tone + relative_delta, self_tone))
                    } else {
                        clamp_double(0.0, 100.0, clamp_double(ref_tone + relative_delta, ref_tone, self_tone))
                    }
                }
                DeltaConstraint::Farther => {
                    if relative_delta > 0.0 {
                        clamp_double(ref_tone + relative_delta, 100.0, self_tone)
                    } else {
                        clamp_double(0.0, ref_tone + relative_delta, self_tone)
                    }
                }
            };
            if self.background.is_some()
                && self.contrast_curve.is_some()
                && let (Some(bg), Some(curve)) = (self.background_of(scheme), self.contrast_curve_of(scheme))
            {
                let bg_tone = bg.get_tone(scheme);
                let self_contrast = curve.get(scheme.contrast_level);
                if !(Contrast::ratio_of_tones(bg_tone, self_tone) >= self_contrast && scheme.contrast_level >= 0.0) {
                    self_tone = DynamicColor::foreground_tone(bg_tone, self_contrast);
                }
            }
            // This can avoid the awkward tones for background colors including the access fixed colors. Accent
            // fixed dim colors should not be adjusted.
            if self.is_background && !self.name.ends_with("_fixed_dim") {
                self_tone = if self_tone >= 57.0 {
                    clamp_double(65.0, 100.0, self_tone)
                } else {
                    clamp_double(0.0, 49.0, self_tone)
                };
            }
            return self_tone;
        }

        // Case 1: no tone delta pair; just solve for itself.
        let mut answer = (self.tone)(scheme);
        let (Some(bg), Some(curve)) = (self.background_of(scheme), self.contrast_curve_of(scheme)) else {
            return answer; // No adjustment for colors with no background.
        };
        let bg_tone = bg.get_tone(scheme);
        let desired_ratio = curve.get(scheme.contrast_level);
        // Recalculate the tone from desired contrast ratio if the current contrast ratio is not enough or desired
        // contrast level is decreasing (<0).
        if !(Contrast::ratio_of_tones(bg_tone, answer) >= desired_ratio && scheme.contrast_level >= 0.0) {
            answer = DynamicColor::foreground_tone(bg_tone, desired_ratio);
        }
        // This can avoid the awkward tones for background colors including the access fixed colors. Accent fixed
        // dim colors should not be adjusted.
        if self.is_background && !self.name.ends_with("_fixed_dim") {
            answer = if answer >= 57.0 { clamp_double(65.0, 100.0, answer) } else { clamp_double(0.0, 49.0, answer) };
        }
        self.solve_second_background(scheme, &bg, answer, desired_ratio)
    }

    /// Case 3: a tone in contrast with two backgrounds (a fixed color and its dim one).
    fn solve_second_background(
        &self,
        scheme: &DynamicScheme,
        bg: &DynamicColor,
        answer: f64,
        desired_ratio: f64,
    ) -> f64 {
        let Some(bg2) = self.second_background_of(scheme) else {
            return answer;
        };
        let bg_tone1 = bg.get_tone(scheme);
        let bg_tone2 = bg2.get_tone(scheme);
        let upper = bg_tone1.max(bg_tone2);
        let lower = bg_tone1.min(bg_tone2);
        if Contrast::ratio_of_tones(upper, answer) >= desired_ratio
            && Contrast::ratio_of_tones(lower, answer) >= desired_ratio
        {
            return answer;
        }
        // The darkest light tone that satisfies the desired ratio, or -1 if such ratio cannot be reached.
        let light_option = Contrast::lighter(upper, desired_ratio);
        // The lightest dark tone that satisfies the desired ratio, or -1 if such ratio cannot be reached.
        let dark_option = Contrast::darker(lower, desired_ratio);
        // Tones suitable for the foreground.
        let mut availables = Vec::new();
        if light_option != -1.0 {
            availables.push(light_option);
        }
        if dark_option != -1.0 {
            availables.push(dark_option);
        }
        let prefers_light = DynamicColor::tone_prefers_light_foreground(bg_tone1)
            || DynamicColor::tone_prefers_light_foreground(bg_tone2);
        if prefers_light {
            return if light_option < 0.0 { 100.0 } else { light_option };
        }
        if availables.len() == 1 {
            return availables[0];
        }
        if dark_option < 0.0 { 0.0 } else { dark_option }
    }

    /// The tone (lighter or darker than `bg_tone`) that has `ratio` against it, the lighter preferred on a dark
    /// background; or the one nearest it where neither has.
    pub fn foreground_tone(bg_tone: f64, ratio: f64) -> f64 {
        let lighter_tone = Contrast::lighter_unsafe(bg_tone, ratio);
        let darker_tone = Contrast::darker_unsafe(bg_tone, ratio);
        let lighter_ratio = Contrast::ratio_of_tones(lighter_tone, bg_tone);
        let darker_ratio = Contrast::ratio_of_tones(darker_tone, bg_tone);
        if DynamicColor::tone_prefers_light_foreground(bg_tone) {
            // This handles an edge case where the initial contrast ratio is high (ex. 13.0), and the ratio passed
            // to the function is that high ratio, and both the lighter and darker ratio fail to pass that ratio.
            //
            // This was observed with Tonal Spot's On Primary Container turning black momentarily between high and
            // max contrast in light mode. PC's standard tone was T90, OPC's was T10, it was light mode, and the
            // contrast value was 0.6568521221032331.
            let negligible_difference =
                (lighter_ratio - darker_ratio).abs() < 0.1 && lighter_ratio < ratio && darker_ratio < ratio;
            if lighter_ratio >= ratio || lighter_ratio >= darker_ratio || negligible_difference {
                lighter_tone
            } else {
                darker_tone
            }
        } else if darker_ratio >= ratio || darker_ratio >= lighter_ratio {
            darker_tone
        } else {
            lighter_tone
        }
    }

    /// Whether a light foreground is preferred on this tone: under 60, rounded (people prefer white on mid-tones,
    /// though black has more contrast up to 49).
    pub fn tone_prefers_light_foreground(tone: f64) -> bool {
        round(tone) < 60.0
    }

    /// Whether a light foreground has contrast enough on this tone: 49 or under, rounded.
    pub fn tone_allows_light_foreground(tone: f64) -> bool {
        round(tone) <= 49.0
    }

    /// The tone, moved to 49 where it prefers a light foreground but has not contrast enough for one.
    pub fn enable_light_foreground(tone: f64) -> f64 {
        if DynamicColor::tone_prefers_light_foreground(tone) && !DynamicColor::tone_allows_light_foreground(tone) {
            49.0
        } else {
            tone
        }
    }

    /// A tone function that is the background's tone, or 50 without one.
    pub fn get_initial_tone_from_background(background: Option<SchemeFn<Option<DynamicColor>>>) -> SchemeFn<f64> {
        match background {
            None => Arc::new(|_| 50.0),
            Some(background) => Arc::new(move |s| background(s).map_or(50.0, |bg| bg.get_tone(s))),
        }
    }
}

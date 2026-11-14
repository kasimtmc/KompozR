//! The Material schemes (Material Color Utilities' `scheme`): a [`DynamicScheme`] of a source color in each
//! [`Variant`].

use crate::material_color::dynamiccolor::{DynamicScheme, Platform, SpecVersion, Variant};
use crate::material_color::hct::Hct;

macro_rules! schemes {
    ($($(#[$doc:meta])* $name:ident: $variant:ident;)*) => {$(
        $(#[$doc])*
        pub struct $name;

        impl $name {
            /// Of a source color, light or dark, at a contrast level (-1 reduced, 0 standard, 0.5 medium, 1 high),
            /// for a phone, in the default spec (2021's).
            #[allow(clippy::new_ret_no_self)]
            pub fn new(source_color_hct: Hct, is_dark: bool, contrast_level: f64) -> DynamicScheme {
                $name::with_spec(source_color_hct, is_dark, contrast_level, DynamicScheme::DEFAULT_SPEC_VERSION, DynamicScheme::DEFAULT_PLATFORM)
            }

            /// The same in a spec version and for a platform.
            pub fn with_spec(
                source_color_hct: Hct,
                is_dark: bool,
                contrast_level: f64,
                spec_version: SpecVersion,
                platform: Platform,
            ) -> DynamicScheme {
                DynamicScheme::new(source_color_hct, Variant::$variant, is_dark, contrast_level, platform, spec_version)
            }
        }
    )*};
}

schemes! {
    /// A calm scheme: the source's hue at a low chroma, tertiary 60° away (Material Color Utilities'
    /// SchemeTonalSpot; Android's default).
    SchemeTonalSpot: TonalSpot;
    /// The source's hue at the most chroma, secondary and tertiary turned by its hue's range (SchemeVibrant).
    SchemeVibrant: Vibrant;
    /// A playful scheme, the primary's hue far from the source's (SchemeExpressive).
    SchemeExpressive: Expressive;
    /// The source color as it is (a picture's), tertiary an analogous color (SchemeContent).
    SchemeContent: Content;
    /// The source color as it is, tertiary its complement (SchemeFidelity).
    SchemeFidelity: Fidelity;
    /// All grey (SchemeMonochrome).
    SchemeMonochrome: Monochrome;
    /// Close to grey, a hint of the source's hue (SchemeNeutral).
    SchemeNeutral: Neutral;
    /// Colorful accents, grey neutrals (SchemeRainbow).
    SchemeRainbow: Rainbow;
    /// Accents of hues around the source's (SchemeFruitSalad).
    SchemeFruitSalad: FruitSalad;
}

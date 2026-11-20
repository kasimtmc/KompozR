//! The color scheme, as Compose's ColorScheme.kt: a theme's 48 color roles ([`ColorScheme`]), the baseline light and
//! dark schemes of Material's tokens ([`light_color_scheme`], [`dark_color_scheme`], the Expressive light one),
//! the content color a background takes ([`ColorScheme::content_color_for`], [`content_color_for`]), the surface
//! color tinted by an elevation ([`ColorScheme::surface_color_at_elevation`]), and whether a surface is tinted so
//! ([`LocalTonalElevationEnabled`]).
//!
//! A scheme is a value here: two with the same colors are the same scheme (Compose's class compares instances,
//! and keeps on each the default colors of the components it has worked out, an allocation saving; KompozR's
//! component colors are plain values, made where they are asked for).

use crate::content_color::LocalContentColor;
use crate::material_theme::MaterialTheme;
use crate::tokens::{ColorDarkTokens, ColorLightTokens, ColorSchemeKeyTokens, PaletteTokens};
use kompoz_graphics::{Color, Dp};
use kompoz_runtime::{KompozitionLocal, KotlinProperties, named, named_args};
use std::fmt;

/// A Material theme's colors (Compose's ColorScheme): each role's color, the ones drawn on them (`on_…`), the
/// containers and their content, the surfaces and their containers by emphasis, the fixed roles (the same light or
/// dark). Make one with [`light_color_scheme`] or [`dark_color_scheme`] (Material's baseline, any role given
/// otherwise), from the system's palette ([`dynamic_light_color_scheme`](crate::dynamic_light_color_scheme)), or
/// all its colors given ([`ColorScheme()`](fn@ColorScheme)); change some with [`copy`](ColorScheme::copy).
#[derive(Clone, Copy, PartialEq, Eq, Hash, KotlinProperties)]
pub struct ColorScheme {
    pub primary: Color,
    pub on_primary: Color,
    pub primary_container: Color,
    pub on_primary_container: Color,
    pub inverse_primary: Color,
    pub secondary: Color,
    pub on_secondary: Color,
    pub secondary_container: Color,
    pub on_secondary_container: Color,
    pub tertiary: Color,
    pub on_tertiary: Color,
    pub tertiary_container: Color,
    pub on_tertiary_container: Color,
    pub background: Color,
    pub on_background: Color,
    pub surface: Color,
    pub on_surface: Color,
    pub surface_variant: Color,
    pub on_surface_variant: Color,
    pub surface_tint: Color,
    pub inverse_surface: Color,
    pub inverse_on_surface: Color,
    pub error: Color,
    pub on_error: Color,
    pub error_container: Color,
    pub on_error_container: Color,
    pub outline: Color,
    pub outline_variant: Color,
    pub scrim: Color,
    pub surface_bright: Color,
    pub surface_dim: Color,
    pub surface_container: Color,
    pub surface_container_high: Color,
    pub surface_container_highest: Color,
    pub surface_container_low: Color,
    pub surface_container_lowest: Color,
    pub primary_fixed: Color,
    pub primary_fixed_dim: Color,
    pub on_primary_fixed: Color,
    pub on_primary_fixed_variant: Color,
    pub secondary_fixed: Color,
    pub secondary_fixed_dim: Color,
    pub on_secondary_fixed: Color,
    pub on_secondary_fixed_variant: Color,
    pub tertiary_fixed: Color,
    pub tertiary_fixed_dim: Color,
    pub on_tertiary_fixed: Color,
    pub on_tertiary_fixed_variant: Color,
}

/// A scheme of every color given (Compose's `ColorScheme(…)` constructor).
#[named(type)]
#[allow(clippy::too_many_arguments)]
pub fn ColorScheme(
    primary: Color,
    on_primary: Color,
    primary_container: Color,
    on_primary_container: Color,
    inverse_primary: Color,
    secondary: Color,
    on_secondary: Color,
    secondary_container: Color,
    on_secondary_container: Color,
    tertiary: Color,
    on_tertiary: Color,
    tertiary_container: Color,
    on_tertiary_container: Color,
    background: Color,
    on_background: Color,
    surface: Color,
    on_surface: Color,
    surface_variant: Color,
    on_surface_variant: Color,
    surface_tint: Color,
    inverse_surface: Color,
    inverse_on_surface: Color,
    error: Color,
    on_error: Color,
    error_container: Color,
    on_error_container: Color,
    outline: Color,
    outline_variant: Color,
    scrim: Color,
    surface_bright: Color,
    surface_dim: Color,
    surface_container: Color,
    surface_container_high: Color,
    surface_container_highest: Color,
    surface_container_low: Color,
    surface_container_lowest: Color,
    primary_fixed: Color,
    primary_fixed_dim: Color,
    on_primary_fixed: Color,
    on_primary_fixed_variant: Color,
    secondary_fixed: Color,
    secondary_fixed_dim: Color,
    on_secondary_fixed: Color,
    on_secondary_fixed_variant: Color,
    tertiary_fixed: Color,
    tertiary_fixed_dim: Color,
    on_tertiary_fixed: Color,
    on_tertiary_fixed_variant: Color,
) -> ColorScheme {
    ColorScheme {
        primary,
        on_primary,
        primary_container,
        on_primary_container,
        inverse_primary,
        secondary,
        on_secondary,
        secondary_container,
        on_secondary_container,
        tertiary,
        on_tertiary,
        tertiary_container,
        on_tertiary_container,
        background,
        on_background,
        surface,
        on_surface,
        surface_variant,
        on_surface_variant,
        surface_tint,
        inverse_surface,
        inverse_on_surface,
        error,
        on_error,
        error_container,
        on_error_container,
        outline,
        outline_variant,
        scrim,
        surface_bright,
        surface_dim,
        surface_container,
        surface_container_high,
        surface_container_highest,
        surface_container_low,
        surface_container_lowest,
        primary_fixed,
        primary_fixed_dim,
        on_primary_fixed,
        on_primary_fixed_variant,
        secondary_fixed,
        secondary_fixed_dim,
        on_secondary_fixed,
        on_secondary_fixed_variant,
        tertiary_fixed,
        tertiary_fixed_dim,
        on_tertiary_fixed,
        on_tertiary_fixed_variant,
    }
}

/// A scheme without the fixed roles (unspecified; Compose's deprecated constructor of before them).
///
/// Kotlin: `ColorScheme(primary: Color, onPrimary: Color, primaryContainer: Color, onPrimaryContainer: Color, inversePrimary: Color, secondary: Color, onSecondary: Color, secondaryContainer: Color, onSecondaryContainer: Color, tertiary: Color, onTertiary: Color, tertiaryContainer: Color, onTertiaryContainer: Color, background: Color, onBackground: Color, surface: Color, onSurface: Color, surfaceVariant: Color, onSurfaceVariant: Color, surfaceTint: Color, inverseSurface: Color, inverseOnSurface: Color, error: Color, onError: Color, errorContainer: Color, onErrorContainer: Color, outline: Color, outlineVariant: Color, scrim: Color, surfaceBright: Color, surfaceDim: Color, surfaceContainer: Color, surfaceContainerHigh: Color, surfaceContainerHighest: Color, surfaceContainerLow: Color, surfaceContainerLowest: Color)`
#[deprecated(note = "Use constructor with additional 'fixed' container roles.")]
#[allow(clippy::too_many_arguments)]
pub fn color_scheme_without_fixed_roles(
    primary: Color,
    on_primary: Color,
    primary_container: Color,
    on_primary_container: Color,
    inverse_primary: Color,
    secondary: Color,
    on_secondary: Color,
    secondary_container: Color,
    on_secondary_container: Color,
    tertiary: Color,
    on_tertiary: Color,
    tertiary_container: Color,
    on_tertiary_container: Color,
    background: Color,
    on_background: Color,
    surface: Color,
    on_surface: Color,
    surface_variant: Color,
    on_surface_variant: Color,
    surface_tint: Color,
    inverse_surface: Color,
    inverse_on_surface: Color,
    error: Color,
    on_error: Color,
    error_container: Color,
    on_error_container: Color,
    outline: Color,
    outline_variant: Color,
    scrim: Color,
    surface_bright: Color,
    surface_dim: Color,
    surface_container: Color,
    surface_container_high: Color,
    surface_container_highest: Color,
    surface_container_low: Color,
    surface_container_lowest: Color,
) -> ColorScheme {
    ColorScheme {
        primary,
        on_primary,
        primary_container,
        on_primary_container,
        inverse_primary,
        secondary,
        on_secondary,
        secondary_container,
        on_secondary_container,
        tertiary,
        on_tertiary,
        tertiary_container,
        on_tertiary_container,
        background,
        on_background,
        surface,
        on_surface,
        surface_variant,
        on_surface_variant,
        surface_tint,
        inverse_surface,
        inverse_on_surface,
        error,
        on_error,
        error_container,
        on_error_container,
        outline,
        outline_variant,
        scrim,
        surface_bright,
        surface_dim,
        surface_container,
        surface_container_high,
        surface_container_highest,
        surface_container_low,
        surface_container_lowest,
        primary_fixed: Color::Unspecified,
        primary_fixed_dim: Color::Unspecified,
        on_primary_fixed: Color::Unspecified,
        on_primary_fixed_variant: Color::Unspecified,
        secondary_fixed: Color::Unspecified,
        secondary_fixed_dim: Color::Unspecified,
        on_secondary_fixed: Color::Unspecified,
        on_secondary_fixed_variant: Color::Unspecified,
        tertiary_fixed: Color::Unspecified,
        tertiary_fixed_dim: Color::Unspecified,
        on_tertiary_fixed: Color::Unspecified,
        on_tertiary_fixed_variant: Color::Unspecified,
    }
}

/// A scheme without the surface containers and the fixed roles (unspecified; Compose's deprecated constructor of
/// before them).
///
/// Kotlin: `ColorScheme(primary: Color, onPrimary: Color, primaryContainer: Color, onPrimaryContainer: Color, inversePrimary: Color, secondary: Color, onSecondary: Color, secondaryContainer: Color, onSecondaryContainer: Color, tertiary: Color, onTertiary: Color, tertiaryContainer: Color, onTertiaryContainer: Color, background: Color, onBackground: Color, surface: Color, onSurface: Color, surfaceVariant: Color, onSurfaceVariant: Color, surfaceTint: Color, inverseSurface: Color, inverseOnSurface: Color, error: Color, onError: Color, errorContainer: Color, onErrorContainer: Color, outline: Color, outlineVariant: Color, scrim: Color)`
#[deprecated(note = "Use constructor with additional 'surfaceContainer' roles.")]
#[allow(clippy::too_many_arguments)]
pub fn color_scheme_without_surface_containers(
    primary: Color,
    on_primary: Color,
    primary_container: Color,
    on_primary_container: Color,
    inverse_primary: Color,
    secondary: Color,
    on_secondary: Color,
    secondary_container: Color,
    on_secondary_container: Color,
    tertiary: Color,
    on_tertiary: Color,
    tertiary_container: Color,
    on_tertiary_container: Color,
    background: Color,
    on_background: Color,
    surface: Color,
    on_surface: Color,
    surface_variant: Color,
    on_surface_variant: Color,
    surface_tint: Color,
    inverse_surface: Color,
    inverse_on_surface: Color,
    error: Color,
    on_error: Color,
    error_container: Color,
    on_error_container: Color,
    outline: Color,
    outline_variant: Color,
    scrim: Color,
) -> ColorScheme {
    ColorScheme {
        primary,
        on_primary,
        primary_container,
        on_primary_container,
        inverse_primary,
        secondary,
        on_secondary,
        secondary_container,
        on_secondary_container,
        tertiary,
        on_tertiary,
        tertiary_container,
        on_tertiary_container,
        background,
        on_background,
        surface,
        on_surface,
        surface_variant,
        on_surface_variant,
        surface_tint,
        inverse_surface,
        inverse_on_surface,
        error,
        on_error,
        error_container,
        on_error_container,
        outline,
        outline_variant,
        scrim,
        surface_bright: Color::Unspecified,
        surface_dim: Color::Unspecified,
        surface_container: Color::Unspecified,
        surface_container_high: Color::Unspecified,
        surface_container_highest: Color::Unspecified,
        surface_container_low: Color::Unspecified,
        surface_container_lowest: Color::Unspecified,
        primary_fixed: Color::Unspecified,
        primary_fixed_dim: Color::Unspecified,
        on_primary_fixed: Color::Unspecified,
        on_primary_fixed_variant: Color::Unspecified,
        secondary_fixed: Color::Unspecified,
        secondary_fixed_dim: Color::Unspecified,
        on_secondary_fixed: Color::Unspecified,
        on_secondary_fixed_variant: Color::Unspecified,
        tertiary_fixed: Color::Unspecified,
        tertiary_fixed_dim: Color::Unspecified,
        on_tertiary_fixed: Color::Unspecified,
        on_tertiary_fixed_variant: Color::Unspecified,
    }
}

#[named]
impl ColorScheme {
    /// The same with what is given replaced (Compose's copy).
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.primary)] primary: Color,
        #[default(self.on_primary)] on_primary: Color,
        #[default(self.primary_container)] primary_container: Color,
        #[default(self.on_primary_container)] on_primary_container: Color,
        #[default(self.inverse_primary)] inverse_primary: Color,
        #[default(self.secondary)] secondary: Color,
        #[default(self.on_secondary)] on_secondary: Color,
        #[default(self.secondary_container)] secondary_container: Color,
        #[default(self.on_secondary_container)] on_secondary_container: Color,
        #[default(self.tertiary)] tertiary: Color,
        #[default(self.on_tertiary)] on_tertiary: Color,
        #[default(self.tertiary_container)] tertiary_container: Color,
        #[default(self.on_tertiary_container)] on_tertiary_container: Color,
        #[default(self.background)] background: Color,
        #[default(self.on_background)] on_background: Color,
        #[default(self.surface)] surface: Color,
        #[default(self.on_surface)] on_surface: Color,
        #[default(self.surface_variant)] surface_variant: Color,
        #[default(self.on_surface_variant)] on_surface_variant: Color,
        #[default(self.surface_tint)] surface_tint: Color,
        #[default(self.inverse_surface)] inverse_surface: Color,
        #[default(self.inverse_on_surface)] inverse_on_surface: Color,
        #[default(self.error)] error: Color,
        #[default(self.on_error)] on_error: Color,
        #[default(self.error_container)] error_container: Color,
        #[default(self.on_error_container)] on_error_container: Color,
        #[default(self.outline)] outline: Color,
        #[default(self.outline_variant)] outline_variant: Color,
        #[default(self.scrim)] scrim: Color,
        #[default(self.surface_bright)] surface_bright: Color,
        #[default(self.surface_dim)] surface_dim: Color,
        #[default(self.surface_container)] surface_container: Color,
        #[default(self.surface_container_high)] surface_container_high: Color,
        #[default(self.surface_container_highest)] surface_container_highest: Color,
        #[default(self.surface_container_low)] surface_container_low: Color,
        #[default(self.surface_container_lowest)] surface_container_lowest: Color,
        #[default(self.primary_fixed)] primary_fixed: Color,
        #[default(self.primary_fixed_dim)] primary_fixed_dim: Color,
        #[default(self.on_primary_fixed)] on_primary_fixed: Color,
        #[default(self.on_primary_fixed_variant)] on_primary_fixed_variant: Color,
        #[default(self.secondary_fixed)] secondary_fixed: Color,
        #[default(self.secondary_fixed_dim)] secondary_fixed_dim: Color,
        #[default(self.on_secondary_fixed)] on_secondary_fixed: Color,
        #[default(self.on_secondary_fixed_variant)] on_secondary_fixed_variant: Color,
        #[default(self.tertiary_fixed)] tertiary_fixed: Color,
        #[default(self.tertiary_fixed_dim)] tertiary_fixed_dim: Color,
        #[default(self.on_tertiary_fixed)] on_tertiary_fixed: Color,
        #[default(self.on_tertiary_fixed_variant)] on_tertiary_fixed_variant: Color,
    ) -> ColorScheme {
        ColorScheme {
            primary,
            on_primary,
            primary_container,
            on_primary_container,
            inverse_primary,
            secondary,
            on_secondary,
            secondary_container,
            on_secondary_container,
            tertiary,
            on_tertiary,
            tertiary_container,
            on_tertiary_container,
            background,
            on_background,
            surface,
            on_surface,
            surface_variant,
            on_surface_variant,
            surface_tint,
            inverse_surface,
            inverse_on_surface,
            error,
            on_error,
            error_container,
            on_error_container,
            outline,
            outline_variant,
            scrim,
            surface_bright,
            surface_dim,
            surface_container,
            surface_container_high,
            surface_container_highest,
            surface_container_low,
            surface_container_lowest,
            primary_fixed,
            primary_fixed_dim,
            on_primary_fixed,
            on_primary_fixed_variant,
            secondary_fixed,
            secondary_fixed_dim,
            on_secondary_fixed,
            on_secondary_fixed_variant,
            tertiary_fixed,
            tertiary_fixed_dim,
            on_tertiary_fixed,
            on_tertiary_fixed_variant,
        }
    }
}

impl ColorScheme {
    /// The content color of a `background_color` of this scheme (Compose's `ColorScheme.contentColorFor`): the `on`
    /// color of the first role of that color (in Compose's order: the accents, the background, the error, the
    /// containers, the surfaces, the fixed roles), unspecified for a color of no role.
    #[allow(clippy::if_same_then_else)] // (Compose's `when`: a branch a background color, several on one)
    pub fn content_color_for(&self, background_color: Color) -> Color {
        if background_color == self.primary {
            self.on_primary
        } else if background_color == self.secondary {
            self.on_secondary
        } else if background_color == self.tertiary {
            self.on_tertiary
        } else if background_color == self.background {
            self.on_background
        } else if background_color == self.error {
            self.on_error
        } else if background_color == self.primary_container {
            self.on_primary_container
        } else if background_color == self.secondary_container {
            self.on_secondary_container
        } else if background_color == self.tertiary_container {
            self.on_tertiary_container
        } else if background_color == self.error_container {
            self.on_error_container
        } else if background_color == self.inverse_surface {
            self.inverse_on_surface
        } else if background_color == self.surface {
            self.on_surface
        } else if background_color == self.surface_variant {
            self.on_surface_variant
        } else if background_color == self.surface_bright {
            self.on_surface
        } else if background_color == self.surface_container {
            self.on_surface
        } else if background_color == self.surface_container_high {
            self.on_surface
        } else if background_color == self.surface_container_highest {
            self.on_surface
        } else if background_color == self.surface_container_low {
            self.on_surface
        } else if background_color == self.surface_container_lowest {
            self.on_surface
        } else if background_color == self.surface_dim {
            self.on_surface
        } else if background_color == self.primary_fixed {
            self.on_primary_fixed
        } else if background_color == self.primary_fixed_dim {
            self.on_primary_fixed
        } else if background_color == self.secondary_fixed {
            self.on_secondary_fixed
        } else if background_color == self.secondary_fixed_dim {
            self.on_secondary_fixed
        } else if background_color == self.tertiary_fixed {
            self.on_tertiary_fixed
        } else if background_color == self.tertiary_fixed_dim {
            self.on_tertiary_fixed
        } else {
            Color::Unspecified
        }
    }

    /// The surface tinted by `surface_tint` as much as an `elevation` lifts it (Compose's
    /// `ColorScheme.surfaceColorAtElevation`): none at 0, `(4.5 ln(elevation + 1) + 2)` percent of the tint over
    /// it above.
    pub fn surface_color_at_elevation(&self, elevation: Dp) -> Color {
        if elevation == Dp(0.0) {
            return self.surface;
        }
        let alpha = ((4.5 * (elevation.0 + 1.0).ln()) + 2.0) / 100.0;
        named_args!(self.surface_tint.copy(alpha = alpha)).composite_over(self.surface)
    }

    /// A token's color in this scheme (Compose's internal `ColorScheme.fromToken`).
    #[allow(clippy::wrong_self_convention)] // (Compose's internal `fromToken`, a member of the theme's value)
    pub(crate) fn from_token(&self, value: ColorSchemeKeyTokens) -> Color {
        match value {
            ColorSchemeKeyTokens::Background => self.background,
            ColorSchemeKeyTokens::Error => self.error,
            ColorSchemeKeyTokens::ErrorContainer => self.error_container,
            ColorSchemeKeyTokens::InverseOnSurface => self.inverse_on_surface,
            ColorSchemeKeyTokens::InversePrimary => self.inverse_primary,
            ColorSchemeKeyTokens::InverseSurface => self.inverse_surface,
            ColorSchemeKeyTokens::OnBackground => self.on_background,
            ColorSchemeKeyTokens::OnError => self.on_error,
            ColorSchemeKeyTokens::OnErrorContainer => self.on_error_container,
            ColorSchemeKeyTokens::OnPrimary => self.on_primary,
            ColorSchemeKeyTokens::OnPrimaryContainer => self.on_primary_container,
            ColorSchemeKeyTokens::OnSecondary => self.on_secondary,
            ColorSchemeKeyTokens::OnSecondaryContainer => self.on_secondary_container,
            ColorSchemeKeyTokens::OnSurface => self.on_surface,
            ColorSchemeKeyTokens::OnSurfaceVariant => self.on_surface_variant,
            ColorSchemeKeyTokens::SurfaceTint => self.surface_tint,
            ColorSchemeKeyTokens::OnTertiary => self.on_tertiary,
            ColorSchemeKeyTokens::OnTertiaryContainer => self.on_tertiary_container,
            ColorSchemeKeyTokens::Outline => self.outline,
            ColorSchemeKeyTokens::OutlineVariant => self.outline_variant,
            ColorSchemeKeyTokens::Primary => self.primary,
            ColorSchemeKeyTokens::PrimaryContainer => self.primary_container,
            ColorSchemeKeyTokens::Scrim => self.scrim,
            ColorSchemeKeyTokens::Secondary => self.secondary,
            ColorSchemeKeyTokens::SecondaryContainer => self.secondary_container,
            ColorSchemeKeyTokens::Surface => self.surface,
            ColorSchemeKeyTokens::SurfaceVariant => self.surface_variant,
            ColorSchemeKeyTokens::SurfaceBright => self.surface_bright,
            ColorSchemeKeyTokens::SurfaceContainer => self.surface_container,
            ColorSchemeKeyTokens::SurfaceContainerHigh => self.surface_container_high,
            ColorSchemeKeyTokens::SurfaceContainerHighest => self.surface_container_highest,
            ColorSchemeKeyTokens::SurfaceContainerLow => self.surface_container_low,
            ColorSchemeKeyTokens::SurfaceContainerLowest => self.surface_container_lowest,
            ColorSchemeKeyTokens::SurfaceDim => self.surface_dim,
            ColorSchemeKeyTokens::Tertiary => self.tertiary,
            ColorSchemeKeyTokens::TertiaryContainer => self.tertiary_container,
            ColorSchemeKeyTokens::PrimaryFixed => self.primary_fixed,
            ColorSchemeKeyTokens::PrimaryFixedDim => self.primary_fixed_dim,
            ColorSchemeKeyTokens::OnPrimaryFixed => self.on_primary_fixed,
            ColorSchemeKeyTokens::OnPrimaryFixedVariant => self.on_primary_fixed_variant,
            ColorSchemeKeyTokens::SecondaryFixed => self.secondary_fixed,
            ColorSchemeKeyTokens::SecondaryFixedDim => self.secondary_fixed_dim,
            ColorSchemeKeyTokens::OnSecondaryFixed => self.on_secondary_fixed,
            ColorSchemeKeyTokens::OnSecondaryFixedVariant => self.on_secondary_fixed_variant,
            ColorSchemeKeyTokens::TertiaryFixed => self.tertiary_fixed,
            ColorSchemeKeyTokens::TertiaryFixedDim => self.tertiary_fixed_dim,
            ColorSchemeKeyTokens::OnTertiaryFixed => self.on_tertiary_fixed,
            ColorSchemeKeyTokens::OnTertiaryFixedVariant => self.on_tertiary_fixed_variant,
        }
    }

    /// `background_color` tinted by `elevation` when it is this scheme's surface and tonal elevation is on
    /// ([`LocalTonalElevationEnabled`]), else as it is (Compose's internal `ColorScheme.applyTonalElevation`).
    pub(crate) fn apply_tonal_elevation(&self, background_color: Color, elevation: Dp) -> Color {
        let tonal_elevation_enabled = LocalTonalElevationEnabled.current();
        if background_color == self.surface && tonal_elevation_enabled {
            self.surface_color_at_elevation(elevation)
        } else {
            background_color
        }
    }
}

/// Compose's toString (`ColorScheme(primary=Color(…)onPrimary=…)`, its fields run together, `onPrimaryFixed`
/// showing `onPrimaryContainer`'s color as Compose's does).
impl fmt::Display for ColorScheme {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("ColorScheme(")?;
        write!(f, "primary={:?}", self.primary)?;
        write!(f, "onPrimary={:?}", self.on_primary)?;
        write!(f, "primaryContainer={:?}", self.primary_container)?;
        write!(f, "onPrimaryContainer={:?}", self.on_primary_container)?;
        write!(f, "inversePrimary={:?}", self.inverse_primary)?;
        write!(f, "secondary={:?}", self.secondary)?;
        write!(f, "onSecondary={:?}", self.on_secondary)?;
        write!(f, "secondaryContainer={:?}", self.secondary_container)?;
        write!(f, "onSecondaryContainer={:?}", self.on_secondary_container)?;
        write!(f, "tertiary={:?}", self.tertiary)?;
        write!(f, "onTertiary={:?}", self.on_tertiary)?;
        write!(f, "tertiaryContainer={:?}", self.tertiary_container)?;
        write!(f, "onTertiaryContainer={:?}", self.on_tertiary_container)?;
        write!(f, "background={:?}", self.background)?;
        write!(f, "onBackground={:?}", self.on_background)?;
        write!(f, "surface={:?}", self.surface)?;
        write!(f, "onSurface={:?}", self.on_surface)?;
        write!(f, "surfaceVariant={:?}", self.surface_variant)?;
        write!(f, "onSurfaceVariant={:?}", self.on_surface_variant)?;
        write!(f, "surfaceTint={:?}", self.surface_tint)?;
        write!(f, "inverseSurface={:?}", self.inverse_surface)?;
        write!(f, "inverseOnSurface={:?}", self.inverse_on_surface)?;
        write!(f, "error={:?}", self.error)?;
        write!(f, "onError={:?}", self.on_error)?;
        write!(f, "errorContainer={:?}", self.error_container)?;
        write!(f, "onErrorContainer={:?}", self.on_error_container)?;
        write!(f, "outline={:?}", self.outline)?;
        write!(f, "outlineVariant={:?}", self.outline_variant)?;
        write!(f, "scrim={:?}", self.scrim)?;
        write!(f, "surfaceBright={:?}", self.surface_bright)?;
        write!(f, "surfaceDim={:?}", self.surface_dim)?;
        write!(f, "surfaceContainer={:?}", self.surface_container)?;
        write!(f, "surfaceContainerHigh={:?}", self.surface_container_high)?;
        write!(f, "surfaceContainerHighest={:?}", self.surface_container_highest)?;
        write!(f, "surfaceContainerLow={:?}", self.surface_container_low)?;
        write!(f, "surfaceContainerLowest={:?}", self.surface_container_lowest)?;
        write!(f, "primaryFixed={:?}", self.primary_fixed)?;
        write!(f, "primaryFixedDim={:?}", self.primary_fixed_dim)?;
        write!(f, "onPrimaryFixed={:?}", self.on_primary_container)?;
        write!(f, "onPrimaryFixedVariant={:?}", self.on_primary_fixed_variant)?;
        write!(f, "secondaryFixed={:?}", self.secondary_fixed)?;
        write!(f, "secondaryFixedDim={:?}", self.secondary_fixed_dim)?;
        write!(f, "onSecondaryFixed={:?}", self.on_secondary_fixed)?;
        write!(f, "onSecondaryFixedVariant={:?}", self.on_secondary_fixed_variant)?;
        write!(f, "tertiaryFixed={:?}", self.tertiary_fixed)?;
        write!(f, "tertiaryFixedDim={:?}", self.tertiary_fixed_dim)?;
        write!(f, "onTertiaryFixed={:?}", self.on_tertiary_fixed)?;
        write!(f, "onTertiaryFixedVariant={:?}", self.on_tertiary_fixed_variant)?;
        f.write_str(")")
    }
}

impl fmt::Debug for ColorScheme {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Default for ColorScheme {
    /// The baseline light scheme ([`light_color_scheme`]).
    fn default() -> ColorScheme {
        light_color_scheme()
    }
}

/// Material's baseline light scheme (its light tokens), any role given otherwise (Compose's
/// `lightColorScheme`): the surface tint is the primary color unless given.
#[named]
#[allow(clippy::too_many_arguments)]
pub fn light_color_scheme(
    #[default(ColorLightTokens::Primary)] primary: Color,
    #[default(ColorLightTokens::OnPrimary)] on_primary: Color,
    #[default(ColorLightTokens::PrimaryContainer)] primary_container: Color,
    #[default(ColorLightTokens::OnPrimaryContainer)] on_primary_container: Color,
    #[default(ColorLightTokens::InversePrimary)] inverse_primary: Color,
    #[default(ColorLightTokens::Secondary)] secondary: Color,
    #[default(ColorLightTokens::OnSecondary)] on_secondary: Color,
    #[default(ColorLightTokens::SecondaryContainer)] secondary_container: Color,
    #[default(ColorLightTokens::OnSecondaryContainer)] on_secondary_container: Color,
    #[default(ColorLightTokens::Tertiary)] tertiary: Color,
    #[default(ColorLightTokens::OnTertiary)] on_tertiary: Color,
    #[default(ColorLightTokens::TertiaryContainer)] tertiary_container: Color,
    #[default(ColorLightTokens::OnTertiaryContainer)] on_tertiary_container: Color,
    #[default(ColorLightTokens::Background)] background: Color,
    #[default(ColorLightTokens::OnBackground)] on_background: Color,
    #[default(ColorLightTokens::Surface)] surface: Color,
    #[default(ColorLightTokens::OnSurface)] on_surface: Color,
    #[default(ColorLightTokens::SurfaceVariant)] surface_variant: Color,
    #[default(ColorLightTokens::OnSurfaceVariant)] on_surface_variant: Color,
    #[default(None)] surface_tint: Option<Color>,
    #[default(ColorLightTokens::InverseSurface)] inverse_surface: Color,
    #[default(ColorLightTokens::InverseOnSurface)] inverse_on_surface: Color,
    #[default(ColorLightTokens::Error)] error: Color,
    #[default(ColorLightTokens::OnError)] on_error: Color,
    #[default(ColorLightTokens::ErrorContainer)] error_container: Color,
    #[default(ColorLightTokens::OnErrorContainer)] on_error_container: Color,
    #[default(ColorLightTokens::Outline)] outline: Color,
    #[default(ColorLightTokens::OutlineVariant)] outline_variant: Color,
    #[default(ColorLightTokens::Scrim)] scrim: Color,
    #[default(ColorLightTokens::SurfaceBright)] surface_bright: Color,
    #[default(ColorLightTokens::SurfaceContainer)] surface_container: Color,
    #[default(ColorLightTokens::SurfaceContainerHigh)] surface_container_high: Color,
    #[default(ColorLightTokens::SurfaceContainerHighest)] surface_container_highest: Color,
    #[default(ColorLightTokens::SurfaceContainerLow)] surface_container_low: Color,
    #[default(ColorLightTokens::SurfaceContainerLowest)] surface_container_lowest: Color,
    #[default(ColorLightTokens::SurfaceDim)] surface_dim: Color,
    #[default(ColorLightTokens::PrimaryFixed)] primary_fixed: Color,
    #[default(ColorLightTokens::PrimaryFixedDim)] primary_fixed_dim: Color,
    #[default(ColorLightTokens::OnPrimaryFixed)] on_primary_fixed: Color,
    #[default(ColorLightTokens::OnPrimaryFixedVariant)] on_primary_fixed_variant: Color,
    #[default(ColorLightTokens::SecondaryFixed)] secondary_fixed: Color,
    #[default(ColorLightTokens::SecondaryFixedDim)] secondary_fixed_dim: Color,
    #[default(ColorLightTokens::OnSecondaryFixed)] on_secondary_fixed: Color,
    #[default(ColorLightTokens::OnSecondaryFixedVariant)] on_secondary_fixed_variant: Color,
    #[default(ColorLightTokens::TertiaryFixed)] tertiary_fixed: Color,
    #[default(ColorLightTokens::TertiaryFixedDim)] tertiary_fixed_dim: Color,
    #[default(ColorLightTokens::OnTertiaryFixed)] on_tertiary_fixed: Color,
    #[default(ColorLightTokens::OnTertiaryFixedVariant)] on_tertiary_fixed_variant: Color,
) -> ColorScheme {
    ColorScheme {
        primary,
        on_primary,
        primary_container,
        on_primary_container,
        inverse_primary,
        secondary,
        on_secondary,
        secondary_container,
        on_secondary_container,
        tertiary,
        on_tertiary,
        tertiary_container,
        on_tertiary_container,
        background,
        on_background,
        surface,
        on_surface,
        surface_variant,
        on_surface_variant,
        surface_tint: surface_tint.unwrap_or(primary),
        inverse_surface,
        inverse_on_surface,
        error,
        on_error,
        error_container,
        on_error_container,
        outline,
        outline_variant,
        scrim,
        surface_bright,
        surface_dim,
        surface_container,
        surface_container_high,
        surface_container_highest,
        surface_container_low,
        surface_container_lowest,
        primary_fixed,
        primary_fixed_dim,
        on_primary_fixed,
        on_primary_fixed_variant,
        secondary_fixed,
        secondary_fixed_dim,
        on_secondary_fixed,
        on_secondary_fixed_variant,
        tertiary_fixed,
        tertiary_fixed_dim,
        on_tertiary_fixed,
        on_tertiary_fixed_variant,
    }
}

/// Material's baseline dark scheme (its dark tokens), any role given otherwise (Compose's
/// `darkColorScheme`): the surface tint is the primary color unless given.
#[named]
#[allow(clippy::too_many_arguments)]
pub fn dark_color_scheme(
    #[default(ColorDarkTokens::Primary)] primary: Color,
    #[default(ColorDarkTokens::OnPrimary)] on_primary: Color,
    #[default(ColorDarkTokens::PrimaryContainer)] primary_container: Color,
    #[default(ColorDarkTokens::OnPrimaryContainer)] on_primary_container: Color,
    #[default(ColorDarkTokens::InversePrimary)] inverse_primary: Color,
    #[default(ColorDarkTokens::Secondary)] secondary: Color,
    #[default(ColorDarkTokens::OnSecondary)] on_secondary: Color,
    #[default(ColorDarkTokens::SecondaryContainer)] secondary_container: Color,
    #[default(ColorDarkTokens::OnSecondaryContainer)] on_secondary_container: Color,
    #[default(ColorDarkTokens::Tertiary)] tertiary: Color,
    #[default(ColorDarkTokens::OnTertiary)] on_tertiary: Color,
    #[default(ColorDarkTokens::TertiaryContainer)] tertiary_container: Color,
    #[default(ColorDarkTokens::OnTertiaryContainer)] on_tertiary_container: Color,
    #[default(ColorDarkTokens::Background)] background: Color,
    #[default(ColorDarkTokens::OnBackground)] on_background: Color,
    #[default(ColorDarkTokens::Surface)] surface: Color,
    #[default(ColorDarkTokens::OnSurface)] on_surface: Color,
    #[default(ColorDarkTokens::SurfaceVariant)] surface_variant: Color,
    #[default(ColorDarkTokens::OnSurfaceVariant)] on_surface_variant: Color,
    #[default(None)] surface_tint: Option<Color>,
    #[default(ColorDarkTokens::InverseSurface)] inverse_surface: Color,
    #[default(ColorDarkTokens::InverseOnSurface)] inverse_on_surface: Color,
    #[default(ColorDarkTokens::Error)] error: Color,
    #[default(ColorDarkTokens::OnError)] on_error: Color,
    #[default(ColorDarkTokens::ErrorContainer)] error_container: Color,
    #[default(ColorDarkTokens::OnErrorContainer)] on_error_container: Color,
    #[default(ColorDarkTokens::Outline)] outline: Color,
    #[default(ColorDarkTokens::OutlineVariant)] outline_variant: Color,
    #[default(ColorDarkTokens::Scrim)] scrim: Color,
    #[default(ColorDarkTokens::SurfaceBright)] surface_bright: Color,
    #[default(ColorDarkTokens::SurfaceContainer)] surface_container: Color,
    #[default(ColorDarkTokens::SurfaceContainerHigh)] surface_container_high: Color,
    #[default(ColorDarkTokens::SurfaceContainerHighest)] surface_container_highest: Color,
    #[default(ColorDarkTokens::SurfaceContainerLow)] surface_container_low: Color,
    #[default(ColorDarkTokens::SurfaceContainerLowest)] surface_container_lowest: Color,
    #[default(ColorDarkTokens::SurfaceDim)] surface_dim: Color,
    #[default(ColorDarkTokens::PrimaryFixed)] primary_fixed: Color,
    #[default(ColorDarkTokens::PrimaryFixedDim)] primary_fixed_dim: Color,
    #[default(ColorDarkTokens::OnPrimaryFixed)] on_primary_fixed: Color,
    #[default(ColorDarkTokens::OnPrimaryFixedVariant)] on_primary_fixed_variant: Color,
    #[default(ColorDarkTokens::SecondaryFixed)] secondary_fixed: Color,
    #[default(ColorDarkTokens::SecondaryFixedDim)] secondary_fixed_dim: Color,
    #[default(ColorDarkTokens::OnSecondaryFixed)] on_secondary_fixed: Color,
    #[default(ColorDarkTokens::OnSecondaryFixedVariant)] on_secondary_fixed_variant: Color,
    #[default(ColorDarkTokens::TertiaryFixed)] tertiary_fixed: Color,
    #[default(ColorDarkTokens::TertiaryFixedDim)] tertiary_fixed_dim: Color,
    #[default(ColorDarkTokens::OnTertiaryFixed)] on_tertiary_fixed: Color,
    #[default(ColorDarkTokens::OnTertiaryFixedVariant)] on_tertiary_fixed_variant: Color,
) -> ColorScheme {
    ColorScheme {
        primary,
        on_primary,
        primary_container,
        on_primary_container,
        inverse_primary,
        secondary,
        on_secondary,
        secondary_container,
        on_secondary_container,
        tertiary,
        on_tertiary,
        tertiary_container,
        on_tertiary_container,
        background,
        on_background,
        surface,
        on_surface,
        surface_variant,
        on_surface_variant,
        surface_tint: surface_tint.unwrap_or(primary),
        inverse_surface,
        inverse_on_surface,
        error,
        on_error,
        error_container,
        on_error_container,
        outline,
        outline_variant,
        scrim,
        surface_bright,
        surface_dim,
        surface_container,
        surface_container_high,
        surface_container_highest,
        surface_container_low,
        surface_container_lowest,
        primary_fixed,
        primary_fixed_dim,
        on_primary_fixed,
        on_primary_fixed_variant,
        secondary_fixed,
        secondary_fixed_dim,
        on_secondary_fixed,
        on_secondary_fixed_variant,
        tertiary_fixed,
        tertiary_fixed_dim,
        on_tertiary_fixed,
        on_tertiary_fixed_variant,
    }
}

/// Material Expressive's light scheme (Compose's `expressiveLightColorScheme`): the baseline's, its containers'
/// content colors at tone 30.
pub fn expressive_light_color_scheme() -> ColorScheme {
    ColorScheme {
        on_primary_container: PaletteTokens::Primary30,
        on_secondary_container: PaletteTokens::Secondary30,
        on_tertiary_container: PaletteTokens::Tertiary30,
        on_error_container: PaletteTokens::Error30,
        ..light_color_scheme()
    }
}

/// The content color of `background_color` in the theme's scheme (Compose's composable `contentColorFor`): its role's
/// `on` color, else the content color around it ([`LocalContentColor`]).
pub fn content_color_for(background_color: Color) -> Color {
    MaterialTheme::color_scheme().content_color_for(background_color).take_or_else(|| LocalContentColor.current())
}

/// How opaque what is disabled is drawn (Compose's internal `DisabledAlpha`; the components' as they come).
#[allow(dead_code)]
pub(crate) const DisabledAlpha: f32 = 0.38;

impl ColorSchemeKeyTokens {
    /// The token's color in the theme's scheme (Compose's internal `ColorSchemeKeyTokens.value`).
    pub(crate) fn value(self) -> Color {
        MaterialTheme::color_scheme().from_token(self)
    }
}

/// Whether a surface of the scheme's surface color is tinted by its tonal elevation (Compose's
/// LocalTonalElevationEnabled): yes unless provided otherwise.
pub static LocalTonalElevationEnabled: KompozitionLocal<bool> = KompozitionLocal::new_static(|| true);

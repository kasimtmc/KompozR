//! The icon buttons' defaults, as Compose's IconButtonDefaults.kt ([`IconButtonDefaults`]): the colors of each kind
//! of icon button and icon toggle button (Material's standard ones and Expressive's vibrant ones), the outlined ones'
//! borders, the shapes of each of Expressive's five sizes (round, square, pressed, selected), the shapes a button
//! morphs between ([`IconButtonDefaults::shapes`], [`IconButtonDefaults::toggleable_shapes`]), and each size's icon
//! and container ([`IconButtonWidthOption`]: narrow, uniform or wide).
//!
//! Compose caches each scheme's default colors and each `Shapes`' default shapes in the scheme (its
//! `defaultIconButtonColorsCached` and the others); KompozR's [`ColorScheme`] and [`Shapes`](crate::Shapes) are
//! plain values without such caches (see `color_scheme.rs`), so the defaults are made where they are asked for —
//! the same values, but for the ones Compose makes of the content color around (`iconButtonColors()` and the
//! outlined and toggle ones): Compose's cache keeps the first content color it was made with and corrects the
//! content colors after, which gives the same, except where a caller passes `Color::Unspecified` for a content
//! color on purpose (Compose's then the first cached one, KompozR's the one around), and Compose's
//! `defaultOutlinedIconToggleButtonColors` reads the standard toggle button's cache (a slip of Compose's: where
//! `iconToggleButtonColors()` was asked for first, the outlined toggle button's default colors are the standard
//! one's); here they are always the outlined ones.
//!
//! A Kotlin function of no parameters beside one with defaulted ones (`filledIconButtonColors()` and
//! `filledIconButtonColors(containerColor, …)`) is one method here whose parameters are `Option`s: none given is the
//! overload without parameters, any given the other, the rest at their Kotlin defaults — the two differ in Compose,
//! the parameters' defaults not being the defaults' values (`contentColorFor(Color.Unspecified)` is the content
//! color around, `Color.Unspecified.copy(alpha = 0.38f)` a color). `shapes()` and `toggleable_shapes()`, whose
//! overloads are the same, are one each too.

use crate::color_scheme::{ColorScheme, content_color_for};
use crate::content_color::LocalContentColor;
use crate::icon_button::{IconButtonColors, IconButtonShapes, IconToggleButtonColors, IconToggleButtonShapes};
use crate::material_theme::MaterialTheme;
use crate::shapes::Shapes;
use crate::tokens::StandardIconButtonTokens;
use crate::tokens::{
    FilledIconButtonTokens, FilledTonalIconButtonTokens, LargeIconButtonTokens, MediumIconButtonTokens,
    OutlinedIconButtonTokens, SmallIconButtonTokens, XLargeIconButtonTokens, XSmallIconButtonTokens,
};
use kompoz_graphics::{BorderStroke, Color, Dp, DpSize, Shape};
use kompoz_runtime::{kompozable_block, named, named_args, remember_keyed};
use std::fmt;
use std::sync::Arc;

/// The icon buttons' and icon toggle buttons' defaults (Compose's `object IconButtonDefaults`).
pub struct IconButtonDefaults;

/// `color` at `alpha` (Kotlin's `color.copy(alpha = …)`: the disabled opacities).
fn faded(color: Color, alpha: f32) -> Color {
    named_args!(color.copy(alpha = alpha))
}

// ---- Colors ----------------------------------------------------------------------------------------------------

/// Whether none of the colors is given: Kotlin's overload without parameters (`iconButtonColors()`) rather than the
/// one with (`iconButtonColors(containerColor, …)`).
fn none_given(colors: &[Option<Color>]) -> bool {
    colors.iter().all(Option::is_none)
}

impl IconButtonDefaults {
    /// The colors made of the content color around, `defaults`' content colors corrected to it where they differ
    /// (Compose's `iconButtonColors()` and the outlined one's body: its cached defaults' content color may be
    /// another).
    fn local_content_colors(defaults: IconButtonColors, content_color: Color, disabled_alpha: f32) -> IconButtonColors {
        if defaults.content_color == content_color {
            defaults
        } else {
            named_args!(defaults.copy(
                content_color = content_color,
                disabled_content_color = faded(content_color, disabled_alpha),
            ))
        }
    }

    /// The same of toggle buttons' colors.
    fn local_content_toggle_colors(
        defaults: IconToggleButtonColors,
        content_color: Color,
        disabled_alpha: f32,
    ) -> IconToggleButtonColors {
        if defaults.content_color == content_color {
            defaults
        } else {
            named_args!(defaults.copy(
                content_color = content_color,
                disabled_content_color = faded(content_color, disabled_alpha),
            ))
        }
    }

    /// The standard icon button's default colors in `local_content_color` (Compose's internal
    /// `ColorScheme.defaultIconButtonColors(localContentColor)`).
    pub(crate) fn default_icon_button_colors(_scheme: &ColorScheme, local_content_color: Color) -> IconButtonColors {
        IconButtonColors::new(
            Color::Transparent,
            local_content_color,
            Color::Transparent,
            faded(local_content_color, StandardIconButtonTokens::DisabledOpacity),
        )
    }

    /// Compose's internal `ColorScheme.defaultIconButtonVibrantColors()`.
    pub(crate) fn default_icon_button_vibrant_colors(scheme: &ColorScheme) -> IconButtonColors {
        IconButtonColors::new(
            Color::Transparent,
            scheme.from_token(StandardIconButtonTokens::Color),
            Color::Transparent,
            faded(
                scheme.from_token(StandardIconButtonTokens::DisabledColor),
                StandardIconButtonTokens::DisabledOpacity,
            ),
        )
    }

    /// Compose's internal `ColorScheme.defaultIconToggleButtonColors(localContentColor)`.
    pub(crate) fn default_icon_toggle_button_colors(
        scheme: &ColorScheme,
        local_content_color: Color,
    ) -> IconToggleButtonColors {
        IconToggleButtonColors::new(
            Color::Transparent,
            local_content_color,
            Color::Transparent,
            faded(local_content_color, StandardIconButtonTokens::DisabledOpacity),
            Color::Transparent,
            scheme.from_token(StandardIconButtonTokens::SelectedColor),
        )
    }

    /// Compose's internal `ColorScheme.defaultIconToggleButtonVibrantColors()`.
    pub(crate) fn default_icon_toggle_button_vibrant_colors(scheme: &ColorScheme) -> IconToggleButtonColors {
        IconToggleButtonColors::new(
            Color::Transparent,
            scheme.from_token(StandardIconButtonTokens::UnselectedColor),
            Color::Transparent,
            faded(
                scheme.from_token(StandardIconButtonTokens::DisabledColor),
                StandardIconButtonTokens::DisabledOpacity,
            ),
            Color::Transparent,
            scheme.from_token(StandardIconButtonTokens::SelectedColor),
        )
    }

    /// Compose's internal `ColorScheme.defaultFilledIconButtonColors`.
    pub(crate) fn default_filled_icon_button_colors(scheme: &ColorScheme) -> IconButtonColors {
        IconButtonColors::new(
            scheme.from_token(FilledIconButtonTokens::ContainerColor),
            scheme.from_token(FilledIconButtonTokens::Color),
            faded(
                scheme.from_token(FilledIconButtonTokens::DisabledContainerColor),
                FilledIconButtonTokens::DisabledContainerOpacity,
            ),
            faded(scheme.from_token(FilledIconButtonTokens::DisabledColor), FilledIconButtonTokens::DisabledOpacity),
        )
    }

    /// Compose's internal `ColorScheme.defaultFilledIconToggleButtonColors`.
    pub(crate) fn default_filled_icon_toggle_button_colors(scheme: &ColorScheme) -> IconToggleButtonColors {
        IconToggleButtonColors::new(
            scheme.from_token(FilledIconButtonTokens::UnselectedContainerColor),
            // (Compose's TODO(b/228455081): contentColorFor would give on surface variant while the token is
            // primary; the token is used)
            scheme.from_token(FilledIconButtonTokens::UnselectedColor),
            faded(
                scheme.from_token(FilledIconButtonTokens::DisabledContainerColor),
                FilledIconButtonTokens::DisabledContainerOpacity,
            ),
            faded(scheme.from_token(FilledIconButtonTokens::DisabledColor), FilledIconButtonTokens::DisabledOpacity),
            scheme.from_token(FilledIconButtonTokens::SelectedContainerColor),
            scheme.from_token(FilledIconButtonTokens::SelectedColor),
        )
    }

    /// Compose's internal `ColorScheme.defaultFilledTonalIconButtonColors`.
    pub(crate) fn default_filled_tonal_icon_button_colors(scheme: &ColorScheme) -> IconButtonColors {
        IconButtonColors::new(
            scheme.from_token(FilledTonalIconButtonTokens::ContainerColor),
            scheme.from_token(FilledTonalIconButtonTokens::Color),
            faded(
                scheme.from_token(FilledTonalIconButtonTokens::DisabledContainerColor),
                FilledTonalIconButtonTokens::DisabledContainerOpacity,
            ),
            faded(
                scheme.from_token(FilledTonalIconButtonTokens::DisabledColor),
                FilledTonalIconButtonTokens::DisabledOpacity,
            ),
        )
    }

    /// Compose's internal `ColorScheme.defaultFilledTonalIconToggleButtonColors`.
    pub(crate) fn default_filled_tonal_icon_toggle_button_colors(scheme: &ColorScheme) -> IconToggleButtonColors {
        IconToggleButtonColors::new(
            scheme.from_token(FilledTonalIconButtonTokens::UnselectedContainerColor),
            scheme.from_token(FilledTonalIconButtonTokens::UnselectedColor),
            faded(
                scheme.from_token(FilledTonalIconButtonTokens::DisabledContainerColor),
                FilledTonalIconButtonTokens::DisabledContainerOpacity,
            ),
            faded(
                scheme.from_token(FilledTonalIconButtonTokens::DisabledColor),
                FilledTonalIconButtonTokens::DisabledOpacity,
            ),
            scheme.from_token(FilledTonalIconButtonTokens::SelectedContainerColor),
            scheme.from_token(FilledTonalIconButtonTokens::SelectedColor),
        )
    }

    /// Compose's internal `ColorScheme.defaultOutlinedIconButtonColors(localContentColor)`.
    pub(crate) fn default_outlined_icon_button_colors(
        _scheme: &ColorScheme,
        local_content_color: Color,
    ) -> IconButtonColors {
        IconButtonColors::new(
            Color::Transparent,
            local_content_color,
            Color::Transparent,
            faded(local_content_color, OutlinedIconButtonTokens::DisabledOpacity),
        )
    }

    /// Compose's internal `ColorScheme.defaultOutlinedIconButtonVibrantColors()`.
    pub(crate) fn default_outlined_icon_button_vibrant_colors(scheme: &ColorScheme) -> IconButtonColors {
        IconButtonColors::new(
            Color::Transparent,
            scheme.from_token(OutlinedIconButtonTokens::Color),
            Color::Transparent,
            faded(
                scheme.from_token(OutlinedIconButtonTokens::DisabledColor),
                OutlinedIconButtonTokens::DisabledOpacity,
            ),
        )
    }

    /// Compose's internal `ColorScheme.defaultOutlinedIconToggleButtonColors(localContentColor)`: its checked
    /// content the scheme's content color of its checked container (inverse on surface).
    pub(crate) fn default_outlined_icon_toggle_button_colors(
        scheme: &ColorScheme,
        local_content_color: Color,
    ) -> IconToggleButtonColors {
        IconToggleButtonColors::new(
            Color::Transparent,
            local_content_color,
            Color::Transparent,
            faded(local_content_color, OutlinedIconButtonTokens::DisabledOpacity),
            scheme.from_token(OutlinedIconButtonTokens::SelectedContainerColor),
            scheme.content_color_for(scheme.from_token(OutlinedIconButtonTokens::SelectedContainerColor)),
        )
    }

    /// Compose's internal `ColorScheme.defaultOutlinedIconToggleButtonVibrantColors()`.
    pub(crate) fn default_outlined_icon_toggle_button_vibrant_colors(scheme: &ColorScheme) -> IconToggleButtonColors {
        IconToggleButtonColors::new(
            Color::Transparent,
            scheme.from_token(OutlinedIconButtonTokens::UnselectedColor),
            Color::Transparent,
            faded(
                scheme.from_token(OutlinedIconButtonTokens::DisabledColor),
                OutlinedIconButtonTokens::DisabledOpacity,
            ),
            scheme.from_token(OutlinedIconButtonTokens::SelectedContainerColor),
            scheme.from_token(OutlinedIconButtonTokens::SelectedColor),
        )
    }
}

#[named]
impl IconButtonDefaults {
    /// The standard icon button's colors (Compose's `iconButtonColors()` and `iconButtonColors(containerColor,
    /// …)`): with nothing given, no container, the content in the content color around (`LocalContentColor`),
    /// disabled at 38% of it; else those given over them (an unspecified one keeps the default's) — `content_color`
    /// where not given the content color around, `disabled_content_color` `content_color` at 38%.
    ///
    /// Kotlin: `iconButtonColors(containerColor: Color = Color.Unspecified, contentColor: Color = LocalContentColor.current, disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = contentColor.copy(alpha = 0.38f))`
    pub fn icon_button_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
    ) -> IconButtonColors {
        let local_content_color = LocalContentColor.current();
        let defaults = Self::default_icon_button_colors(&MaterialTheme::color_scheme(), local_content_color);
        if none_given(&[container_color, content_color, disabled_container_color, disabled_content_color]) {
            return Self::local_content_colors(
                defaults,
                local_content_color,
                StandardIconButtonTokens::DisabledOpacity,
            );
        }
        let content_color = content_color.unwrap_or(local_content_color);
        named_args!(
            defaults.copy(
                container_color = container_color.unwrap_or(Color::Unspecified),
                content_color = content_color,
                disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
                disabled_content_color = disabled_content_color
                    .unwrap_or_else(|| faded(content_color, StandardIconButtonTokens::DisabledOpacity)),
            )
        )
    }

    /// The vibrant icon button's colors (Compose's Expressive `iconButtonVibrantColors()` and
    /// `iconButtonVibrantColors(containerColor, …)`): with nothing given, no container, the content in the scheme's
    /// on surface variant, disabled on surface at 38%; else those given over them — `disabled_content_color` where
    /// not given `content_color` at 38% (a color even of an unspecified `content_color`, as Compose's
    /// `Color.Unspecified.copy(alpha = 0.38f)` is).
    ///
    /// Kotlin: `iconButtonVibrantColors(containerColor: Color = Color.Unspecified, contentColor: Color = Color.Unspecified, disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = contentColor.copy(alpha = 0.38f))`
    pub fn icon_button_vibrant_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
    ) -> IconButtonColors {
        let defaults = Self::default_icon_button_vibrant_colors(&MaterialTheme::color_scheme());
        if none_given(&[container_color, content_color, disabled_container_color, disabled_content_color]) {
            return defaults;
        }
        let content_color = content_color.unwrap_or(Color::Unspecified);
        named_args!(
            defaults.copy(
                container_color = container_color.unwrap_or(Color::Unspecified),
                content_color = content_color,
                disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
                disabled_content_color = disabled_content_color
                    .unwrap_or_else(|| faded(content_color, StandardIconButtonTokens::DisabledOpacity)),
            )
        )
    }

    /// The standard icon toggle button's colors (Compose's `iconToggleButtonColors()` and
    /// `iconToggleButtonColors(containerColor, …)`): with nothing given, no container, the content in the content
    /// color around (disabled at 38% of it), checked in the scheme's primary; else those given over them —
    /// `content_color` where not given the content color around, `disabled_content_color` `content_color` at 38%.
    ///
    /// Kotlin: `iconToggleButtonColors(containerColor: Color = Color.Unspecified, contentColor: Color = LocalContentColor.current, disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = contentColor.copy(alpha = 0.38f), checkedContainerColor: Color = Color.Unspecified, checkedContentColor: Color = Color.Unspecified)`
    #[allow(clippy::too_many_arguments)]
    pub fn icon_toggle_button_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
        #[default(None)] checked_container_color: Option<Color>,
        #[default(None)] checked_content_color: Option<Color>,
    ) -> IconToggleButtonColors {
        let local_content_color = LocalContentColor.current();
        let defaults = Self::default_icon_toggle_button_colors(&MaterialTheme::color_scheme(), local_content_color);
        let given = [
            container_color,
            content_color,
            disabled_container_color,
            disabled_content_color,
            checked_container_color,
            checked_content_color,
        ];
        if none_given(&given) {
            return Self::local_content_toggle_colors(
                defaults,
                local_content_color,
                StandardIconButtonTokens::DisabledOpacity,
            );
        }
        let content_color = content_color.unwrap_or(local_content_color);
        named_args!(
            defaults.copy(
                container_color = container_color.unwrap_or(Color::Unspecified),
                content_color = content_color,
                disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
                disabled_content_color = disabled_content_color
                    .unwrap_or_else(|| faded(content_color, StandardIconButtonTokens::DisabledOpacity)),
                checked_container_color = checked_container_color.unwrap_or(Color::Unspecified),
                checked_content_color = checked_content_color.unwrap_or(Color::Unspecified),
            )
        )
    }

    /// The vibrant icon toggle button's colors (Compose's Expressive `iconToggleButtonVibrantColors()` and
    /// `iconToggleButtonVibrantColors(containerColor, …)`): with nothing given, no container, the content in on
    /// surface variant, checked in primary, disabled on surface at 38%; else those given over them —
    /// `disabled_content_color` where not given `content_color` at 38%.
    ///
    /// Kotlin: `iconToggleButtonVibrantColors(containerColor: Color = Color.Unspecified, contentColor: Color = Color.Unspecified, disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = contentColor.copy(alpha = 0.38f), checkedContainerColor: Color = Color.Unspecified, checkedContentColor: Color = Color.Unspecified)`
    #[allow(clippy::too_many_arguments)]
    pub fn icon_toggle_button_vibrant_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
        #[default(None)] checked_container_color: Option<Color>,
        #[default(None)] checked_content_color: Option<Color>,
    ) -> IconToggleButtonColors {
        let defaults = Self::default_icon_toggle_button_vibrant_colors(&MaterialTheme::color_scheme());
        let given = [
            container_color,
            content_color,
            disabled_container_color,
            disabled_content_color,
            checked_container_color,
            checked_content_color,
        ];
        if none_given(&given) {
            return defaults;
        }
        let content_color = content_color.unwrap_or(Color::Unspecified);
        named_args!(
            defaults.copy(
                container_color = container_color.unwrap_or(Color::Unspecified),
                content_color = content_color,
                disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
                disabled_content_color = disabled_content_color
                    .unwrap_or_else(|| faded(content_color, StandardIconButtonTokens::DisabledOpacity)),
                checked_container_color = checked_container_color.unwrap_or(Color::Unspecified),
                checked_content_color = checked_content_color.unwrap_or(Color::Unspecified),
            )
        )
    }

    /// The filled icon button's colors (Compose's `filledIconButtonColors()` and `filledIconButtonColors(containerColor,
    /// …)`): with nothing given, on the scheme's primary, the content on primary, disabled on surface at 10% under
    /// on surface at 38%; else those given over them — `content_color` where not given
    /// `content_color_for(container_color)` (the content color around for an unspecified container or one not of
    /// the scheme).
    ///
    /// Kotlin: `filledIconButtonColors(containerColor: Color = Color.Unspecified, contentColor: Color = contentColorFor(containerColor), disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = Color.Unspecified)`
    pub fn filled_icon_button_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
    ) -> IconButtonColors {
        let defaults = Self::default_filled_icon_button_colors(&MaterialTheme::color_scheme());
        if none_given(&[container_color, content_color, disabled_container_color, disabled_content_color]) {
            return defaults;
        }
        let container_color = container_color.unwrap_or(Color::Unspecified);
        named_args!(defaults.copy(
            container_color = container_color,
            content_color = content_color.unwrap_or_else(|| content_color_for(container_color)),
            disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
            disabled_content_color = disabled_content_color.unwrap_or(Color::Unspecified),
        ))
    }

    /// The filled icon toggle button's colors (Compose's `filledIconToggleButtonColors()` and
    /// `filledIconToggleButtonColors(containerColor, …)`): with nothing given, unchecked on the scheme's surface
    /// container in on surface variant, checked on primary in on primary, disabled on surface at 10% under on
    /// surface at 38%; else those given over them — `checked_content_color` where not given
    /// `content_color_for(checked_container_color)`.
    ///
    /// Kotlin: `filledIconToggleButtonColors(containerColor: Color = Color.Unspecified, contentColor: Color = Color.Unspecified, disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = Color.Unspecified, checkedContainerColor: Color = Color.Unspecified, checkedContentColor: Color = contentColorFor(checkedContainerColor))`
    #[allow(clippy::too_many_arguments)]
    pub fn filled_icon_toggle_button_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
        #[default(None)] checked_container_color: Option<Color>,
        #[default(None)] checked_content_color: Option<Color>,
    ) -> IconToggleButtonColors {
        let defaults = Self::default_filled_icon_toggle_button_colors(&MaterialTheme::color_scheme());
        let given = [
            container_color,
            content_color,
            disabled_container_color,
            disabled_content_color,
            checked_container_color,
            checked_content_color,
        ];
        if none_given(&given) {
            return defaults;
        }
        let checked_container_color = checked_container_color.unwrap_or(Color::Unspecified);
        named_args!(defaults.copy(
            container_color = container_color.unwrap_or(Color::Unspecified),
            content_color = content_color.unwrap_or(Color::Unspecified),
            disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
            disabled_content_color = disabled_content_color.unwrap_or(Color::Unspecified),
            checked_container_color = checked_container_color,
            checked_content_color = checked_content_color.unwrap_or_else(|| content_color_for(checked_container_color)),
        ))
    }

    /// The filled tonal icon button's colors (Compose's `filledTonalIconButtonColors()` and
    /// `filledTonalIconButtonColors(containerColor, …)`): with nothing given, on the scheme's secondary container,
    /// the content on secondary container, disabled on surface at 10% under on surface at 38%; else those given over
    /// them — `content_color` where not given `content_color_for(container_color)`.
    ///
    /// Kotlin: `filledTonalIconButtonColors(containerColor: Color = Color.Unspecified, contentColor: Color = contentColorFor(containerColor), disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = Color.Unspecified)`
    pub fn filled_tonal_icon_button_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
    ) -> IconButtonColors {
        let defaults = Self::default_filled_tonal_icon_button_colors(&MaterialTheme::color_scheme());
        if none_given(&[container_color, content_color, disabled_container_color, disabled_content_color]) {
            return defaults;
        }
        let container_color = container_color.unwrap_or(Color::Unspecified);
        named_args!(defaults.copy(
            container_color = container_color,
            content_color = content_color.unwrap_or_else(|| content_color_for(container_color)),
            disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
            disabled_content_color = disabled_content_color.unwrap_or(Color::Unspecified),
        ))
    }

    /// The filled tonal icon toggle button's colors (Compose's `filledTonalIconToggleButtonColors()` and
    /// `filledTonalIconToggleButtonColors(containerColor, …)`): with nothing given, unchecked on secondary container
    /// in on secondary container, checked on secondary in on secondary, disabled on surface at 10% under on surface
    /// at 38%; else those given over them — `content_color` and `checked_content_color` where not given the
    /// `content_color_for` of their containers.
    ///
    /// Kotlin: `filledTonalIconToggleButtonColors(containerColor: Color = Color.Unspecified, contentColor: Color = contentColorFor(containerColor), disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = Color.Unspecified, checkedContainerColor: Color = Color.Unspecified, checkedContentColor: Color = contentColorFor(checkedContainerColor))`
    #[allow(clippy::too_many_arguments)]
    pub fn filled_tonal_icon_toggle_button_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
        #[default(None)] checked_container_color: Option<Color>,
        #[default(None)] checked_content_color: Option<Color>,
    ) -> IconToggleButtonColors {
        let defaults = Self::default_filled_tonal_icon_toggle_button_colors(&MaterialTheme::color_scheme());
        let given = [
            container_color,
            content_color,
            disabled_container_color,
            disabled_content_color,
            checked_container_color,
            checked_content_color,
        ];
        if none_given(&given) {
            return defaults;
        }
        let container_color = container_color.unwrap_or(Color::Unspecified);
        let checked_container_color = checked_container_color.unwrap_or(Color::Unspecified);
        named_args!(defaults.copy(
            container_color = container_color,
            content_color = content_color.unwrap_or_else(|| content_color_for(container_color)),
            disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
            disabled_content_color = disabled_content_color.unwrap_or(Color::Unspecified),
            checked_container_color = checked_container_color,
            checked_content_color = checked_content_color.unwrap_or_else(|| content_color_for(checked_container_color)),
        ))
    }

    /// The outlined icon button's colors (Compose's `outlinedIconButtonColors()` and
    /// `outlinedIconButtonColors(containerColor, …)`): with nothing given, no container, the content in the content
    /// color around, disabled at 38% of it; else those given over them — `content_color` where not given the content
    /// color around, `disabled_content_color` `content_color` at 38%.
    ///
    /// Kotlin: `outlinedIconButtonColors(containerColor: Color = Color.Unspecified, contentColor: Color = LocalContentColor.current, disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = contentColor.copy(alpha = 0.38f))`
    pub fn outlined_icon_button_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
    ) -> IconButtonColors {
        let local_content_color = LocalContentColor.current();
        let defaults = Self::default_outlined_icon_button_colors(&MaterialTheme::color_scheme(), local_content_color);
        if none_given(&[container_color, content_color, disabled_container_color, disabled_content_color]) {
            return Self::local_content_colors(
                defaults,
                local_content_color,
                OutlinedIconButtonTokens::DisabledOpacity,
            );
        }
        let content_color = content_color.unwrap_or(local_content_color);
        named_args!(
            defaults.copy(
                container_color = container_color.unwrap_or(Color::Unspecified),
                content_color = content_color,
                disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
                disabled_content_color = disabled_content_color
                    .unwrap_or_else(|| faded(content_color, OutlinedIconButtonTokens::DisabledOpacity)),
            )
        )
    }

    /// The vibrant outlined icon button's colors (Compose's Expressive `outlinedIconButtonVibrantColors()` and
    /// `outlinedIconButtonVibrantColors(containerColor, …)`): with nothing given, no container, the content in on
    /// surface variant, disabled on surface at 38%; else those given over them — `disabled_content_color` where not
    /// given `content_color` at 38%.
    ///
    /// Kotlin: `outlinedIconButtonVibrantColors(containerColor: Color = Color.Unspecified, contentColor: Color = Color.Unspecified, disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = contentColor.copy(alpha = 0.38f))`
    pub fn outlined_icon_button_vibrant_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
    ) -> IconButtonColors {
        let defaults = Self::default_outlined_icon_button_vibrant_colors(&MaterialTheme::color_scheme());
        if none_given(&[container_color, content_color, disabled_container_color, disabled_content_color]) {
            return defaults;
        }
        let content_color = content_color.unwrap_or(Color::Unspecified);
        named_args!(
            defaults.copy(
                container_color = container_color.unwrap_or(Color::Unspecified),
                content_color = content_color,
                disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
                disabled_content_color = disabled_content_color
                    .unwrap_or_else(|| faded(content_color, OutlinedIconButtonTokens::DisabledOpacity)),
            )
        )
    }

    /// The outlined icon toggle button's colors (Compose's `outlinedIconToggleButtonColors()` and
    /// `outlinedIconToggleButtonColors(containerColor, …)`): with nothing given, unchecked no container in the
    /// content color around (disabled at 38% of it), checked on the scheme's inverse surface in inverse on surface;
    /// else those given over them — `content_color` where not given the content color around,
    /// `disabled_content_color` `content_color` at 38%, `checked_content_color`
    /// `content_color_for(checked_container_color)`.
    ///
    /// Kotlin: `outlinedIconToggleButtonColors(containerColor: Color = Color.Unspecified, contentColor: Color = LocalContentColor.current, disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = contentColor.copy(alpha = 0.38f), checkedContainerColor: Color = Color.Unspecified, checkedContentColor: Color = contentColorFor(checkedContainerColor))`
    #[allow(clippy::too_many_arguments)]
    pub fn outlined_icon_toggle_button_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
        #[default(None)] checked_container_color: Option<Color>,
        #[default(None)] checked_content_color: Option<Color>,
    ) -> IconToggleButtonColors {
        let local_content_color = LocalContentColor.current();
        let defaults =
            Self::default_outlined_icon_toggle_button_colors(&MaterialTheme::color_scheme(), local_content_color);
        let given = [
            container_color,
            content_color,
            disabled_container_color,
            disabled_content_color,
            checked_container_color,
            checked_content_color,
        ];
        if none_given(&given) {
            return Self::local_content_toggle_colors(
                defaults,
                local_content_color,
                OutlinedIconButtonTokens::DisabledOpacity,
            );
        }
        let content_color = content_color.unwrap_or(local_content_color);
        let checked_container_color = checked_container_color.unwrap_or(Color::Unspecified);
        named_args!(
            defaults.copy(
                container_color = container_color.unwrap_or(Color::Unspecified),
                content_color = content_color,
                disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
                disabled_content_color = disabled_content_color
                    .unwrap_or_else(|| faded(content_color, OutlinedIconButtonTokens::DisabledOpacity)),
                checked_container_color = checked_container_color,
                checked_content_color =
                    checked_content_color.unwrap_or_else(|| content_color_for(checked_container_color)),
            )
        )
    }

    /// The vibrant outlined icon toggle button's colors (Compose's Expressive
    /// `outlinedIconToggleButtonVibrantColors()` and `outlinedIconToggleButtonVibrantColors(containerColor, …)`):
    /// with nothing given, unchecked no container in on surface variant, checked on inverse surface in inverse on
    /// surface, disabled on surface at 38%; else those given over them — `disabled_content_color` where not given
    /// `content_color` at 38%, `checked_content_color` `content_color_for(checked_container_color)`.
    ///
    /// Kotlin: `outlinedIconToggleButtonVibrantColors(containerColor: Color = Color.Unspecified, contentColor: Color = Color.Unspecified, disabledContainerColor: Color = Color.Unspecified, disabledContentColor: Color = contentColor.copy(alpha = 0.38f), checkedContainerColor: Color = Color.Unspecified, checkedContentColor: Color = contentColorFor(checkedContainerColor))`
    #[allow(clippy::too_many_arguments)]
    pub fn outlined_icon_toggle_button_vibrant_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_content_color: Option<Color>,
        #[default(None)] checked_container_color: Option<Color>,
        #[default(None)] checked_content_color: Option<Color>,
    ) -> IconToggleButtonColors {
        let defaults = Self::default_outlined_icon_toggle_button_vibrant_colors(&MaterialTheme::color_scheme());
        let given = [
            container_color,
            content_color,
            disabled_container_color,
            disabled_content_color,
            checked_container_color,
            checked_content_color,
        ];
        if none_given(&given) {
            return defaults;
        }
        let content_color = content_color.unwrap_or(Color::Unspecified);
        let checked_container_color = checked_container_color.unwrap_or(Color::Unspecified);
        named_args!(
            defaults.copy(
                container_color = container_color.unwrap_or(Color::Unspecified),
                content_color = content_color,
                disabled_container_color = disabled_container_color.unwrap_or(Color::Unspecified),
                disabled_content_color = disabled_content_color
                    .unwrap_or_else(|| faded(content_color, OutlinedIconButtonTokens::DisabledOpacity)),
                checked_container_color = checked_container_color,
                checked_content_color =
                    checked_content_color.unwrap_or_else(|| content_color_for(checked_container_color)),
            )
        )
    }
}

// ---- Borders ---------------------------------------------------------------------------------------------------

impl IconButtonDefaults {
    /// The outlined icon toggle button's border (Compose's `outlinedIconToggleButtonBorder(enabled, checked)`): none
    /// when checked, else [`outlined_icon_button_border`](Self::outlined_icon_button_border).
    #[track_caller]
    pub fn outlined_icon_toggle_button_border(enabled: bool, checked: bool) -> Option<BorderStroke> {
        if checked {
            return None;
        }
        Some(Self::outlined_icon_button_border(enabled))
    }

    /// The vibrant outlined icon toggle button's border (Compose's Expressive
    /// `outlinedIconToggleButtonVibrantBorder(enabled, checked)`): none when checked, else
    /// [`outlined_icon_button_vibrant_border`](Self::outlined_icon_button_vibrant_border).
    #[track_caller]
    pub fn outlined_icon_toggle_button_vibrant_border(enabled: bool, checked: bool) -> Option<BorderStroke> {
        if checked {
            return None;
        }
        Some(Self::outlined_icon_button_vibrant_border(enabled))
    }

    /// The outlined icon button's border (Compose's `outlinedIconButtonBorder(enabled)`): 1 dp in the content color
    /// around, at 38% when disabled; remembered while its color stays.
    #[track_caller]
    pub fn outlined_icon_button_border(enabled: bool) -> BorderStroke {
        kompozable_block!({
            let outline_color = LocalContentColor.current();
            let color =
                if enabled { outline_color } else { faded(outline_color, OutlinedIconButtonTokens::DisabledOpacity) };
            remember_keyed(color, move || BorderStroke(SmallIconButtonTokens::OutlinedOutlineWidth, color))
        })
    }

    /// The vibrant outlined icon button's border (Compose's Expressive `outlinedIconButtonVibrantBorder(enabled)`):
    /// 1 dp in the scheme's outline variant, at 38% when disabled; remembered while its color stays.
    #[track_caller]
    pub fn outlined_icon_button_vibrant_border(enabled: bool) -> BorderStroke {
        kompozable_block!({
            let outline_color = OutlinedIconButtonTokens::OutlineColor.value();
            let color =
                if enabled { outline_color } else { faded(outline_color, OutlinedIconButtonTokens::DisabledOpacity) };
            remember_keyed(color, move || BorderStroke(SmallIconButtonTokens::OutlinedOutlineWidth, color))
        })
    }
}

// ---- Shapes ----------------------------------------------------------------------------------------------------

impl IconButtonDefaults {
    /// The standard icon button's shape (Compose's `standardShape`): the small size's round one, a circle.
    pub fn standard_shape() -> Arc<dyn Shape> {
        SmallIconButtonTokens::ContainerShapeRound.value()
    }

    /// The filled icon buttons' shape (Compose's `filledShape`): a circle.
    pub fn filled_shape() -> Arc<dyn Shape> {
        SmallIconButtonTokens::ContainerShapeRound.value()
    }

    /// The outlined icon buttons' shape (Compose's `outlinedShape`): a circle.
    pub fn outlined_shape() -> Arc<dyn Shape> {
        SmallIconButtonTokens::ContainerShapeRound.value()
    }

    /// The extra small round shape (Compose's Expressive `extraSmallRoundShape`): a circle.
    pub fn extra_small_round_shape() -> Arc<dyn Shape> {
        XSmallIconButtonTokens::ContainerShapeRound.value()
    }

    /// The extra small square shape (Compose's Expressive `extraSmallSquareShape`): the theme's medium shape.
    pub fn extra_small_square_shape() -> Arc<dyn Shape> {
        XSmallIconButtonTokens::ContainerShapeSquare.value()
    }

    /// The extra small pressed shape (Compose's Expressive `extraSmallPressedShape`): the theme's small shape.
    pub fn extra_small_pressed_shape() -> Arc<dyn Shape> {
        XSmallIconButtonTokens::PressedContainerShape.value()
    }

    /// The extra small selected round shape (Compose's Expressive `extraSmallSelectedRoundShape`): the theme's
    /// medium shape.
    pub fn extra_small_selected_round_shape() -> Arc<dyn Shape> {
        XSmallIconButtonTokens::SelectedContainerShapeRound.value()
    }

    /// The extra small selected square shape (Compose's Expressive `extraSmallSelectedSquareShape`): a circle.
    pub fn extra_small_selected_square_shape() -> Arc<dyn Shape> {
        XSmallIconButtonTokens::SelectedContainerShapeSquare.value()
    }

    /// The small round shape (Compose's Expressive `smallRoundShape`): a circle.
    pub fn small_round_shape() -> Arc<dyn Shape> {
        SmallIconButtonTokens::ContainerShapeRound.value()
    }

    /// The small square shape (Compose's Expressive `smallSquareShape`): the theme's medium shape.
    pub fn small_square_shape() -> Arc<dyn Shape> {
        SmallIconButtonTokens::ContainerShapeSquare.value()
    }

    /// The small pressed shape (Compose's Expressive `smallPressedShape`): the theme's small shape.
    pub fn small_pressed_shape() -> Arc<dyn Shape> {
        SmallIconButtonTokens::PressedContainerShape.value()
    }

    /// The small selected round shape (Compose's Expressive `smallSelectedRoundShape`): the theme's medium shape.
    pub fn small_selected_round_shape() -> Arc<dyn Shape> {
        SmallIconButtonTokens::SelectedContainerShapeRound.value()
    }

    /// The small selected square shape (Compose's Expressive `smallSelectedSquareShape`): a circle.
    pub fn small_selected_square_shape() -> Arc<dyn Shape> {
        SmallIconButtonTokens::SelectedContainerShapeSquare.value()
    }

    /// The small selected square shape under its earlier name (Compose's `SmallSelectedSquareShape`, deprecated and
    /// hidden from Kotlin source: kept for binary compatibility).
    #[deprecated(note = "Use the camel case version")]
    #[doc(hidden)]
    pub fn SmallSelectedSquareShape() -> Arc<dyn Shape> {
        SmallIconButtonTokens::SelectedContainerShapeSquare.value()
    }

    /// The medium round shape (Compose's Expressive `mediumRoundShape`): a circle.
    pub fn medium_round_shape() -> Arc<dyn Shape> {
        MediumIconButtonTokens::ContainerShapeRound.value()
    }

    /// The medium square shape (Compose's Expressive `mediumSquareShape`): the theme's large shape.
    pub fn medium_square_shape() -> Arc<dyn Shape> {
        MediumIconButtonTokens::ContainerShapeSquare.value()
    }

    /// The medium pressed shape (Compose's Expressive `mediumPressedShape`): the theme's medium shape.
    pub fn medium_pressed_shape() -> Arc<dyn Shape> {
        MediumIconButtonTokens::PressedContainerShape.value()
    }

    /// The medium selected round shape (Compose's Expressive `mediumSelectedRoundShape`): the theme's large shape.
    pub fn medium_selected_round_shape() -> Arc<dyn Shape> {
        MediumIconButtonTokens::SelectedContainerShapeRound.value()
    }

    /// The medium selected square shape (Compose's Expressive `mediumSelectedSquareShape`): a circle.
    pub fn medium_selected_square_shape() -> Arc<dyn Shape> {
        MediumIconButtonTokens::SelectedContainerShapeSquare.value()
    }

    /// The large round shape (Compose's Expressive `largeRoundShape`): a circle.
    pub fn large_round_shape() -> Arc<dyn Shape> {
        LargeIconButtonTokens::ContainerShapeRound.value()
    }

    /// The large square shape (Compose's Expressive `largeSquareShape`): the theme's extra large shape.
    pub fn large_square_shape() -> Arc<dyn Shape> {
        LargeIconButtonTokens::ContainerShapeSquare.value()
    }

    /// The large pressed shape (Compose's Expressive `largePressedShape`): the theme's large shape.
    pub fn large_pressed_shape() -> Arc<dyn Shape> {
        LargeIconButtonTokens::PressedContainerShape.value()
    }

    /// The large selected round shape (Compose's Expressive `largeSelectedRoundShape`): the theme's extra large
    /// shape.
    pub fn large_selected_round_shape() -> Arc<dyn Shape> {
        LargeIconButtonTokens::SelectedContainerShapeRound.value()
    }

    /// The large selected square shape (Compose's Expressive `largeSelectedSquareShape`): a circle.
    pub fn large_selected_square_shape() -> Arc<dyn Shape> {
        LargeIconButtonTokens::SelectedContainerShapeSquare.value()
    }

    /// The extra large round shape (Compose's Expressive `extraLargeRoundShape`): a circle.
    pub fn extra_large_round_shape() -> Arc<dyn Shape> {
        XLargeIconButtonTokens::ContainerShapeRound.value()
    }

    /// The extra large square shape (Compose's Expressive `extraLargeSquareShape`): the theme's extra large shape.
    pub fn extra_large_square_shape() -> Arc<dyn Shape> {
        XLargeIconButtonTokens::ContainerShapeSquare.value()
    }

    /// The extra large pressed shape (Compose's Expressive `extraLargePressedShape`): the theme's large shape.
    pub fn extra_large_pressed_shape() -> Arc<dyn Shape> {
        XLargeIconButtonTokens::PressedContainerShape.value()
    }

    /// The extra large selected round shape (Compose's Expressive `extraLargeSelectedRoundShape`): the theme's
    /// extra large shape.
    pub fn extra_large_selected_round_shape() -> Arc<dyn Shape> {
        XLargeIconButtonTokens::SelectedContainerShapeRound.value()
    }

    /// The extra large selected square shape (Compose's Expressive `extraLargeSelectedSquareShape`): a circle.
    pub fn extra_large_selected_square_shape() -> Arc<dyn Shape> {
        XLargeIconButtonTokens::SelectedContainerShapeSquare.value()
    }

    /// The default shapes an icon button morphs between (Compose's internal `Shapes.defaultIconButtonShapes`): the
    /// small size's round one (a circle), its pressed one.
    pub(crate) fn default_icon_button_shapes(shapes: &Shapes) -> IconButtonShapes {
        IconButtonShapes::new(
            shapes.from_token(SmallIconButtonTokens::ContainerShapeRound),
            Some(shapes.from_token(SmallIconButtonTokens::PressedContainerShape)),
        )
    }

    /// The default shapes an icon toggle button morphs between (Compose's internal
    /// `Shapes.defaultIconToggleButtonShapes`): the small size's round one, its pressed one, its selected round one.
    pub(crate) fn default_icon_toggle_button_shapes(shapes: &Shapes) -> IconToggleButtonShapes {
        IconToggleButtonShapes::new(
            shapes.from_token(SmallIconButtonTokens::ContainerShapeRound),
            Some(shapes.from_token(SmallIconButtonTokens::PressedContainerShape)),
            Some(shapes.from_token(SmallIconButtonTokens::SelectedContainerShapeRound)),
        )
    }
}

#[named]
impl IconButtonDefaults {
    /// The shapes an icon button morphs between as it is pressed (Compose's Expressive `shapes(shape,
    /// pressedShape)`, and `shapes()`, the same): the defaults (a circle, pressed the theme's small shape), the ones
    /// given instead.
    ///
    /// Kotlin: `shapes(shape: Shape? = null, pressedShape: Shape? = null)`
    pub fn shapes(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
    ) -> IconButtonShapes {
        named_args!(
            Self::default_icon_button_shapes(&MaterialTheme::shapes())
                .copy(shape = shape, pressed_shape = pressed_shape,)
        )
    }

    /// The shapes an icon toggle button morphs between as it is pressed and checked (Compose's Expressive
    /// `toggleableShapes(shape, pressedShape, checkedShape)`, and `toggleableShapes()`, the same): the defaults (a
    /// circle, pressed the theme's small shape, checked its medium one), the ones given instead.
    ///
    /// Kotlin: `toggleableShapes(shape: Shape? = null, pressedShape: Shape? = null, checkedShape: Shape? = null)`
    pub fn toggleable_shapes(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
        #[default(None)] checked_shape: Option<Arc<dyn Shape>>,
    ) -> IconToggleButtonShapes {
        named_args!(Self::default_icon_toggle_button_shapes(&MaterialTheme::shapes()).copy(
            shape = shape,
            pressed_shape = pressed_shape,
            checked_shape = checked_shape,
        ))
    }
}

// ---- Sizes -----------------------------------------------------------------------------------------------------

impl IconButtonDefaults {
    /// The extra small icon button's icon, 20 dp.
    ///
    /// Kotlin: `extraSmallIconSize`
    pub const ExtraSmallIconSize: Dp = XSmallIconButtonTokens::IconSize;

    /// The small icon button's icon, 24 dp.
    ///
    /// Kotlin: `smallIconSize`
    pub const SmallIconSize: Dp = SmallIconButtonTokens::IconSize;

    /// The medium icon button's icon, 24 dp.
    ///
    /// Kotlin: `mediumIconSize`
    pub const MediumIconSize: Dp = MediumIconButtonTokens::IconSize;

    /// The large icon button's icon, 32 dp.
    ///
    /// Kotlin: `largeIconSize`
    pub const LargeIconSize: Dp = LargeIconButtonTokens::IconSize;

    /// The extra large icon button's icon, 40 dp.
    ///
    /// Kotlin: `extraLargeIconSize`
    pub const ExtraLargeIconSize: Dp = XLargeIconButtonTokens::IconSize;
}

/// A container's size: the icon and the space at its sides, the height (Compose's `when (widthOption)` of the
/// container sizes, `else -> 0.dp` for a width option none of the three).
fn container_size(
    width_option: IconButtonWidthOption,
    icon_size: Dp,
    height: Dp,
    narrow: Dp,
    uniform: Dp,
    wide: Dp,
) -> DpSize {
    let horizontal_space = match width_option {
        IconButtonWidthOption::Narrow => narrow,
        IconButtonWidthOption::Uniform => uniform,
        IconButtonWidthOption::Wide => wide,
        _ => Dp(0.0),
    };
    DpSize::new(icon_size + horizontal_space, height)
}

#[named]
impl IconButtonDefaults {
    /// The extra small icon button's container (Compose's Expressive `extraSmallContainerSize(widthOption)`): 32 dp
    /// tall, its 20 dp icon with 4 dp (narrow), 6 dp (uniform) or 10 dp (wide) at each side.
    ///
    /// Kotlin: `extraSmallContainerSize(widthOption: IconButtonWidthOption = IconButtonWidthOption.Uniform)`
    pub fn extra_small_container_size(
        &self,
        #[default(IconButtonWidthOption::Uniform)] width_option: IconButtonWidthOption,
    ) -> DpSize {
        type T = XSmallIconButtonTokens;
        container_size(
            width_option,
            T::IconSize,
            T::ContainerHeight,
            T::NarrowLeadingSpace + T::NarrowTrailingSpace,
            T::DefaultLeadingSpace + T::DefaultLeadingSpace,
            T::WideLeadingSpace + T::WideTrailingSpace,
        )
    }

    /// The small icon button's container (Compose's Expressive `smallContainerSize(widthOption)`): 40 dp tall, its
    /// 24 dp icon with 4 dp (narrow), 8 dp (uniform: 40 dp square, the standard icon button's) or 14 dp (wide) at
    /// each side.
    ///
    /// Kotlin: `smallContainerSize(widthOption: IconButtonWidthOption = IconButtonWidthOption.Uniform)`
    pub fn small_container_size(
        &self,
        #[default(IconButtonWidthOption::Uniform)] width_option: IconButtonWidthOption,
    ) -> DpSize {
        type T = SmallIconButtonTokens;
        container_size(
            width_option,
            T::IconSize,
            T::ContainerHeight,
            T::NarrowLeadingSpace + T::NarrowTrailingSpace,
            T::DefaultLeadingSpace + T::DefaultLeadingSpace,
            T::WideLeadingSpace + T::WideTrailingSpace,
        )
    }

    /// The medium icon button's container (Compose's Expressive `mediumContainerSize(widthOption)`): 56 dp tall, its
    /// 24 dp icon with 12 dp (narrow), 16 dp (uniform) or 24 dp (wide) at each side.
    ///
    /// Kotlin: `mediumContainerSize(widthOption: IconButtonWidthOption = IconButtonWidthOption.Uniform)`
    pub fn medium_container_size(
        &self,
        #[default(IconButtonWidthOption::Uniform)] width_option: IconButtonWidthOption,
    ) -> DpSize {
        type T = MediumIconButtonTokens;
        container_size(
            width_option,
            T::IconSize,
            T::ContainerHeight,
            T::NarrowLeadingSpace + T::NarrowTrailingSpace,
            T::DefaultLeadingSpace + T::DefaultLeadingSpace,
            T::WideLeadingSpace + T::WideTrailingSpace,
        )
    }

    /// The large icon button's container (Compose's Expressive `largeContainerSize(widthOption)`): 96 dp tall, its
    /// 32 dp icon with 16 dp (narrow), 32 dp (uniform) or 48 dp (wide) at each side.
    ///
    /// Kotlin: `largeContainerSize(widthOption: IconButtonWidthOption = IconButtonWidthOption.Uniform)`
    pub fn large_container_size(
        &self,
        #[default(IconButtonWidthOption::Uniform)] width_option: IconButtonWidthOption,
    ) -> DpSize {
        type T = LargeIconButtonTokens;
        container_size(
            width_option,
            T::IconSize,
            T::ContainerHeight,
            T::NarrowLeadingSpace + T::NarrowTrailingSpace,
            T::UniformLeadingSpace + T::UniformLeadingSpace,
            T::WideLeadingSpace + T::WideTrailingSpace,
        )
    }

    /// The extra large icon button's container (Compose's Expressive `extraLargeContainerSize(widthOption)`): 136 dp
    /// tall, its 40 dp icon with 32 dp (narrow), 48 dp (uniform) or 72 dp (wide) at each side.
    ///
    /// Kotlin: `extraLargeContainerSize(widthOption: IconButtonWidthOption = IconButtonWidthOption.Uniform)`
    pub fn extra_large_container_size(
        &self,
        #[default(IconButtonWidthOption::Uniform)] width_option: IconButtonWidthOption,
    ) -> DpSize {
        type T = XLargeIconButtonTokens;
        container_size(
            width_option,
            T::IconSize,
            T::ContainerHeight,
            T::NarrowLeadingSpace + T::NarrowTrailingSpace,
            T::DefaultLeadingSpace + T::DefaultLeadingSpace,
            T::WideLeadingSpace + T::WideTrailingSpace,
        )
    }
}

/// How wide an icon button's container is for its height (Compose's Expressive value class
/// `IconButtonDefaults.IconButtonWidthOption`): the space at the icon's sides narrower than, as wide as, or wider
/// than its uniform (square) one.
///
/// Kotlin: `IconButtonDefaults.IconButtonWidthOption`
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct IconButtonWidthOption(i32);

impl IconButtonWidthOption {
    /// Narrower than it is tall.
    pub const Narrow: IconButtonWidthOption = IconButtonWidthOption(0);
    /// As wide as it is tall (the default).
    pub const Uniform: IconButtonWidthOption = IconButtonWidthOption(1);
    /// Wider than it is tall.
    pub const Wide: IconButtonWidthOption = IconButtonWidthOption(2);
}

/// Compose's toString: the option's name.
impl fmt::Display for IconButtonWidthOption {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            IconButtonWidthOption::Narrow => "Narrow",
            IconButtonWidthOption::Uniform => "Uniform",
            IconButtonWidthOption::Wide => "Wide",
            _ => "Unknown",
        })
    }
}

impl fmt::Debug for IconButtonWidthOption {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

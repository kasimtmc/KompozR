//! Material Expressive's toggle buttons, as Compose's ToggleButton.kt (material3 1.5.0-alpha17): the
//! [`ToggleButton`] (filled: the surface container unchecked, primary checked), the [`ElevatedToggleButton`], the
//! [`TonalToggleButton`] and the [`OutlinedToggleButton`] — a toggleable surface, a checkbox to accessibility, its
//! label in the theme's `label_large`, whose colors follow `checked` ([`ToggleButtonColors`]) and whose shape morphs
//! between [`ToggleButtonShapes`]' as it is pressed and checked (corner by corner where all three are corner based,
//! along the theme's fast spatial spec). [`ToggleButtonDefaults`] has their colors, shapes (each Expressive size's
//! too: [`ToggleButtonDefaults::shapes_for`]), sizes and padding.

use crate::button::{ButtonDefaults, ButtonElevation, corner_based_shape_of, rounded_corner_shape_of};
use crate::color_scheme::ColorScheme;
use crate::internal::animated_shape::remember_animated_shape;
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::material_theme::MaterialTheme;
use crate::shapes::Shapes;
use crate::surface::ToggleableSurface;
use crate::tokens::{
    ButtonLargeTokens, ButtonMediumTokens, ButtonSmallTokens, ButtonXLargeTokens, ButtonXSmallTokens,
    ElevatedButtonTokens, FilledButtonTokens, MotionSchemeKeyTokens, OutlinedButtonTokens, TonalButtonTokens,
};
use kompoz_animation::FiniteAnimationSpec;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{BorderStroke, Color, Dp, RoundedCornerShape, Shape};
use kompoz_runtime::{KotlinProperties, key, kompozable, named, named_args, remember};
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::cell::RefCell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

// ---- The toggle buttons ----------------------------------------------------------------------------------------

/// A toggle button (Compose's Expressive `ToggleButton`): a button switching between its unchecked and checked
/// colors as `checked` says, `on_checked_change` called with the other state when clicked. Its shape morphs
/// between `shapes`' — the pressed one while pressed, the checked one while checked, else its shape — corner by
/// corner along the theme's fast spatial spec where all three are corner based (rounded ones as rounded), else
/// switching between them. `content` sits in a row, centered both ways, padded by `content_padding`, at least
/// [`ToggleButtonDefaults::MinHeight`] tall, its text in the theme's `label_large` in `colors`' content color; the
/// surface casts `elevation`'s shadow (none without one) under `border`, and is a checkbox to accessibility.
/// Where not `enabled` it takes no input and shows `colors`' disabled ones.
///
/// Kotlin: `ToggleButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ToggleButton(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ToggleButtonDefaults.shapes_for(ButtonDefaults::MinHeight()))] shapes: ToggleButtonShapes,
    #[default(ToggleButtonDefaults.toggle_button_colors())] colors: ToggleButtonColors,
    #[default(Some(ButtonDefaults.button_elevation()))] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(ButtonDefaults.content_padding_for(ButtonDefaults::MinHeight()))] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    // TODO Load the motionScheme tokens from the component tokens file
    let default_animation_spec = MotionSchemeKeyTokens::FastSpatial.value::<f32>();
    let pressed = interaction_source.collect_is_pressed_as_state().value();
    let container_color = colors.container_color_for(enabled, checked);
    let content_color = colors.content_color_for(enabled, checked);
    let shadow_elevation = match elevation {
        Some(elevation) => elevation.shadow_elevation(enabled, &interaction_source).value(),
        None => Dp(0.0),
    };
    let button_shape = shape_by_interaction(shapes, pressed, checked, default_animation_spec);

    let content = Rc::new(content);
    ToggleableSurface(
        checked,
        on_checked_change,
        modifier = modifier.semantics(|s| s.set_role(Role::Checkbox)),
        enabled = enabled,
        shape = button_shape,
        color = container_color,
        content_color = Some(content_color),
        shadow_elevation = shadow_elevation,
        border = border,
        interaction_source = Some(interaction_source),
        content = move || {
            let (content, content_padding) = (content.clone(), content_padding.clone());
            ProvideContentColorTextStyle(content_color, MaterialTheme::typography().label_large, move || {
                let content = content.clone();
                Row(
                    modifier = Modifier
                        .default_min_size_with(Dp::Unspecified, ToggleButtonDefaults::MinHeight)
                        .padding_values(content_padding.clone()),
                    horizontal_arrangement = Arrangement::Center,
                    vertical_alignment = Alignment::CenterVertically,
                    content = move || content(),
                );
            });
        },
    );
}

/// An elevated toggle button (Compose's Expressive `ElevatedToggleButton`, high emphasis: only where it must stand
/// apart from a patterned container): a [`ToggleButton`] of [`ToggleButtonDefaults::elevated_toggle_button_colors`]
/// (the surface container low unchecked) casting [`ButtonDefaults::elevated_button_elevation`]'s shadow.
///
/// Kotlin: `ElevatedToggleButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ElevatedToggleButton(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ToggleButtonDefaults.shapes_for(ButtonDefaults::MinHeight()))] shapes: ToggleButtonShapes,
    #[default(ToggleButtonDefaults.elevated_toggle_button_colors())] colors: ToggleButtonColors,
    #[default(Some(ButtonDefaults.elevated_button_elevation()))] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(ButtonDefaults.content_padding_for(ButtonDefaults::MinHeight()))] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    ToggleButton(
        checked,
        on_checked_change,
        modifier = modifier,
        enabled = enabled,
        shapes = shapes,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = content_padding,
        interaction_source = interaction_source,
        content = content,
    );
}

/// A tonal toggle button (Compose's Expressive `TonalToggleButton`, between the filled and the outlined in
/// emphasis): a [`ToggleButton`] of [`ToggleButtonDefaults::tonal_toggle_button_colors`] (the secondary container
/// unchecked, secondary checked) with [`ButtonDefaults::filled_tonal_button_elevation`].
///
/// Kotlin: `TonalToggleButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TonalToggleButton(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ToggleButtonDefaults.shapes_for(ButtonDefaults::MinHeight()))] shapes: ToggleButtonShapes,
    #[default(ToggleButtonDefaults.tonal_toggle_button_colors())] colors: ToggleButtonColors,
    #[default(Some(ButtonDefaults.filled_tonal_button_elevation()))] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(ButtonDefaults.content_padding_for(ButtonDefaults::MinHeight()))] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    ToggleButton(
        checked,
        on_checked_change,
        modifier = modifier,
        enabled = enabled,
        shapes = shapes,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = content_padding,
        interaction_source = interaction_source,
        content = content,
    );
}

/// An outlined toggle button (Compose's Expressive `OutlinedToggleButton`, medium emphasis): a [`ToggleButton`] of
/// [`ToggleButtonDefaults::outlined_toggle_button_colors`] (transparent unchecked, the inverse surface checked),
/// without a shadow, outlined while unchecked. `border`: `None` (the default) is Kotlin's
/// `if (!checked) ButtonDefaults.outlinedButtonBorder(enabled) else null`, `Some(None)` Kotlin's `null` (none).
///
/// Kotlin: `OutlinedToggleButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn OutlinedToggleButton(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ToggleButtonDefaults.shapes_for(ButtonDefaults::MinHeight()))] shapes: ToggleButtonShapes,
    #[default(ToggleButtonDefaults.outlined_toggle_button_colors())] colors: ToggleButtonColors,
    #[default(None)] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(ButtonDefaults.content_padding_for(ButtonDefaults::MinHeight()))] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let border = border.unwrap_or_else(|| {
        if !checked { Some(named_args!(ButtonDefaults.outlined_button_border(enabled = enabled))) } else { None }
    });
    ToggleButton(
        checked,
        on_checked_change,
        modifier = modifier,
        enabled = enabled,
        shapes = shapes,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = content_padding,
        interaction_source = interaction_source,
        content = content,
    );
}

// ---- ToggleButtonDefaults --------------------------------------------------------------------------------------

/// The toggle buttons' defaults (Compose's Expressive `ToggleButtonDefaults`): sizes, the padding, shapes (each
/// Expressive size's: [`ToggleButtonDefaults::shapes_for`]), the four kinds' colors. Plain values are associated
/// consts (functions where Compose's are read in composition or make a padding); what Compose computes from
/// arguments or the theme are methods of the value (`ToggleButtonDefaults.toggle_button_colors()`).
#[derive(Clone, Copy, Debug)]
pub struct ToggleButtonDefaults;

impl ToggleButtonDefaults {
    const ToggleButtonStartPadding: Dp = ButtonSmallTokens::LeadingSpace;
    const ToggleButtonEndPadding: Dp = ButtonSmallTokens::TrailingSpace;
    const ButtonVerticalPadding: Dp = Dp(8.0);

    /// A toggle button's least height, the small button's 40 dp (Compose's `MinHeight`).
    pub const MinHeight: Dp = ButtonSmallTokens::ContainerHeight;

    /// The space between a toggle button's icon and its text, 8 dp (Compose's `IconSpacing`).
    pub const IconSpacing: Dp = ButtonSmallTokens::IconLabelSpace;

    /// The size of a toggle button's icon, 20 dp (Compose's `IconSize`).
    pub const IconSize: Dp = ButtonSmallTokens::IconSize;

    /// The padding of a toggle button's content: 16 dp at the sides, 8 above and below (Compose's
    /// `ContentPadding`).
    pub fn ContentPadding() -> PaddingValues {
        PaddingValues::new(
            Self::ToggleButtonStartPadding,
            Self::ButtonVerticalPadding,
            Self::ToggleButtonEndPadding,
            Self::ButtonVerticalPadding,
        )
    }

    /// A round toggle button's shape, round ends (Compose's `roundShape`).
    pub fn round_shape() -> Arc<dyn Shape> {
        ButtonSmallTokens::ContainerShapeRound.value()
    }

    /// A square toggle button's shape, the theme's medium corners (Compose's `squareShape`).
    pub fn square_shape() -> Arc<dyn Shape> {
        ButtonSmallTokens::ContainerShapeSquare.value()
    }

    /// A toggle button's shape at rest, round ends (Compose's `shape`).
    pub fn shape() -> Arc<dyn Shape> {
        ButtonSmallTokens::ContainerShapeRound.value()
    }

    /// A pressed toggle button's shape, 6 dp corners (Compose's `pressedShape`).
    pub fn pressed_shape() -> Arc<dyn Shape> {
        Arc::new(RoundedCornerShape(Dp(6.0)))
    }

    /// A checked toggle button's shape, the theme's medium corners (Compose's `checkedShape`).
    pub fn checked_shape() -> Arc<dyn Shape> {
        ButtonSmallTokens::SelectedContainerShapeSquare.value()
    }

    /// A square extra small toggle button's shape (Compose's `extraSmallSquareShape`).
    pub fn extra_small_square_shape() -> Arc<dyn Shape> {
        ButtonXSmallTokens::ContainerShapeSquare.value()
    }

    /// A square medium toggle button's shape (Compose's `mediumSquareShape`).
    pub fn medium_square_shape() -> Arc<dyn Shape> {
        ButtonMediumTokens::ContainerShapeSquare.value()
    }

    /// A square large toggle button's shape (Compose's `largeSquareShape`).
    pub fn large_square_shape() -> Arc<dyn Shape> {
        ButtonLargeTokens::ContainerShapeSquare.value()
    }

    /// A square extra large toggle button's shape (Compose's `extraLargeSquareShape`).
    pub fn extra_large_square_shape() -> Arc<dyn Shape> {
        ButtonXLargeTokens::ContainerShapeSquare.value()
    }

    /// A pressed extra small toggle button's shape (Compose's `extraSmallPressedShape`).
    pub fn extra_small_pressed_shape() -> Arc<dyn Shape> {
        ButtonXSmallTokens::PressedContainerShape.value()
    }

    /// A pressed medium toggle button's shape (Compose's `mediumPressedShape`).
    pub fn medium_pressed_shape() -> Arc<dyn Shape> {
        ButtonMediumTokens::PressedContainerShape.value()
    }

    /// A pressed large toggle button's shape (Compose's `largePressedShape`).
    pub fn large_pressed_shape() -> Arc<dyn Shape> {
        ButtonLargeTokens::PressedContainerShape.value()
    }

    /// A pressed extra large toggle button's shape (Compose's `extraLargePressedShape`).
    pub fn extra_large_pressed_shape() -> Arc<dyn Shape> {
        ButtonXLargeTokens::PressedContainerShape.value()
    }

    /// A checked extra small toggle button's shape, square (Compose's `extraSmallCheckedSquareShape`).
    pub fn extra_small_checked_square_shape() -> Arc<dyn Shape> {
        ButtonXSmallTokens::ContainerShapeSquare.value()
    }

    /// A checked medium toggle button's shape, square (Compose's `mediumCheckedSquareShape`).
    pub fn medium_checked_square_shape() -> Arc<dyn Shape> {
        ButtonMediumTokens::ContainerShapeSquare.value()
    }

    /// A checked large toggle button's shape, square (Compose's `largeCheckedSquareShape`).
    pub fn large_checked_square_shape() -> Arc<dyn Shape> {
        ButtonLargeTokens::ContainerShapeSquare.value()
    }

    /// A checked extra large toggle button's shape, square (Compose's `extraLargeCheckedSquareShape`).
    pub fn extra_large_checked_square_shape() -> Arc<dyn Shape> {
        ButtonXLargeTokens::ContainerShapeSquare.value()
    }

    /// A theme's default toggle button shapes: round, 6 dp corners pressed, the medium corners checked (Compose's
    /// internal `Shapes.defaultToggleButtonShapes`; its per-instance cache isn't kept, as the color schemes' aren't).
    pub(crate) fn default_toggle_button_shapes(shapes: &Shapes) -> ToggleButtonShapes {
        ToggleButtonShapes::new(
            shapes.from_token(ButtonSmallTokens::ContainerShapeRound),
            Arc::new(RoundedCornerShape(Dp(6.0))),
            shapes.from_token(ButtonSmallTokens::SelectedContainerShapeSquare),
        )
    }

    /// A toggle button's colors in `scheme` (Compose's internal `ColorScheme.defaultToggleButtonColors`): the
    /// surface container under the surface variant's text unchecked, primary under its text checked; disabled, the
    /// surface's text at 10% under it at 38%.
    pub(crate) fn default_toggle_button_colors(scheme: &ColorScheme) -> ToggleButtonColors {
        ToggleButtonColors::new(
            scheme.from_token(FilledButtonTokens::UnselectedContainerColor),
            scheme.from_token(FilledButtonTokens::UnselectedPressedLabelTextColor),
            named_args!(
                scheme
                    .from_token(FilledButtonTokens::DisabledContainerColor)
                    .copy(alpha = FilledButtonTokens::DisabledContainerOpacity)
            ),
            named_args!(
                scheme
                    .from_token(FilledButtonTokens::DisabledLabelTextColor)
                    .copy(alpha = FilledButtonTokens::DisabledLabelTextOpacity)
            ),
            scheme.from_token(FilledButtonTokens::SelectedContainerColor),
            scheme.from_token(FilledButtonTokens::SelectedPressedLabelTextColor),
        )
    }

    /// An elevated toggle button's colors in `scheme` (Compose's internal
    /// `ColorScheme.defaultElevatedToggleButtonColors`): the surface container low under primary text unchecked,
    /// primary checked.
    pub(crate) fn default_elevated_toggle_button_colors(scheme: &ColorScheme) -> ToggleButtonColors {
        ToggleButtonColors::new(
            scheme.from_token(ElevatedButtonTokens::UnselectedContainerColor),
            scheme.from_token(ElevatedButtonTokens::UnselectedPressedLabelTextColor),
            named_args!(
                scheme
                    .from_token(ElevatedButtonTokens::DisabledContainerColor)
                    .copy(alpha = ElevatedButtonTokens::DisabledContainerOpacity)
            ),
            named_args!(
                scheme
                    .from_token(ElevatedButtonTokens::DisabledLabelTextColor)
                    .copy(alpha = ElevatedButtonTokens::DisabledLabelTextOpacity)
            ),
            scheme.from_token(ElevatedButtonTokens::SelectedContainerColor),
            scheme.from_token(ElevatedButtonTokens::SelectedPressedLabelTextColor),
        )
    }

    /// A tonal toggle button's colors in `scheme` (Compose's internal `ColorScheme.defaultTonalToggleButtonColors`):
    /// the secondary container unchecked, secondary checked.
    pub(crate) fn default_tonal_toggle_button_colors(scheme: &ColorScheme) -> ToggleButtonColors {
        ToggleButtonColors::new(
            scheme.from_token(TonalButtonTokens::UnselectedContainerColor),
            scheme.from_token(TonalButtonTokens::UnselectedLabelTextColor),
            named_args!(
                scheme
                    .from_token(TonalButtonTokens::DisabledContainerColor)
                    .copy(alpha = TonalButtonTokens::DisabledContainerOpacity)
            ),
            named_args!(
                scheme
                    .from_token(TonalButtonTokens::DisabledLabelTextColor)
                    .copy(alpha = TonalButtonTokens::DisabledLabelTextOpacity)
            ),
            scheme.from_token(TonalButtonTokens::SelectedContainerColor),
            scheme.from_token(TonalButtonTokens::SelectedLabelTextColor),
        )
    }

    /// An outlined toggle button's colors in `scheme` (Compose's internal
    /// `ColorScheme.defaultOutlinedToggleButtonColors`): transparent unchecked, the inverse surface checked.
    pub(crate) fn default_outlined_toggle_button_colors(scheme: &ColorScheme) -> ToggleButtonColors {
        ToggleButtonColors::new(
            Color::Transparent,
            scheme.from_token(OutlinedButtonTokens::UnselectedLabelTextColor),
            named_args!(
                scheme
                    .from_token(OutlinedButtonTokens::DisabledOutlineColor)
                    .copy(alpha = OutlinedButtonTokens::DisabledContainerOpacity)
            ),
            named_args!(
                scheme
                    .from_token(OutlinedButtonTokens::DisabledLabelTextColor)
                    .copy(alpha = OutlinedButtonTokens::DisabledLabelTextOpacity)
            ),
            scheme.from_token(OutlinedButtonTokens::SelectedContainerColor),
            scheme.from_token(OutlinedButtonTokens::SelectedLabelTextColor),
        )
    }
}

#[named]
impl ToggleButtonDefaults {
    /// The theme's toggle button shapes with `shape`, `pressed_shape` and `checked_shape` where given (Compose's
    /// `shapes()` and `shapes(shape, pressedShape, checkedShape)`): round, 6 dp corners pressed, the theme's medium
    /// corners checked.
    pub fn shapes(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
        #[default(None)] checked_shape: Option<Arc<dyn Shape>>,
    ) -> ToggleButtonShapes {
        named_args!(Self::default_toggle_button_shapes(&MaterialTheme::shapes()).copy(
            shape = shape,
            pressed_shape = pressed_shape,
            checked_shape = checked_shape,
        ))
    }

    /// A toggle button's colors, each given one other than unspecified in place of the theme's (Compose's
    /// `toggleButtonColors()` and `toggleButtonColors(containerColor, …)`).
    pub fn toggle_button_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
        #[default(Color::Unspecified)] checked_container_color: Color,
        #[default(Color::Unspecified)] checked_content_color: Color,
    ) -> ToggleButtonColors {
        named_args!(Self::default_toggle_button_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            content_color = content_color,
            disabled_container_color = disabled_container_color,
            disabled_content_color = disabled_content_color,
            checked_container_color = checked_container_color,
            checked_content_color = checked_content_color,
        ))
    }

    /// An elevated toggle button's colors, each given one in place of the theme's (Compose's
    /// `elevatedToggleButtonColors()` and `elevatedToggleButtonColors(…)`).
    pub fn elevated_toggle_button_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
        #[default(Color::Unspecified)] checked_container_color: Color,
        #[default(Color::Unspecified)] checked_content_color: Color,
    ) -> ToggleButtonColors {
        named_args!(Self::default_elevated_toggle_button_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            content_color = content_color,
            disabled_container_color = disabled_container_color,
            disabled_content_color = disabled_content_color,
            checked_container_color = checked_container_color,
            checked_content_color = checked_content_color,
        ))
    }

    /// A tonal toggle button's colors, each given one in place of the theme's (Compose's
    /// `tonalToggleButtonColors()` and `tonalToggleButtonColors(…)`).
    pub fn tonal_toggle_button_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
        #[default(Color::Unspecified)] checked_container_color: Color,
        #[default(Color::Unspecified)] checked_content_color: Color,
    ) -> ToggleButtonColors {
        named_args!(Self::default_tonal_toggle_button_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            content_color = content_color,
            disabled_container_color = disabled_container_color,
            disabled_content_color = disabled_content_color,
            checked_container_color = checked_container_color,
            checked_content_color = checked_content_color,
        ))
    }

    /// An outlined toggle button's colors, each given one in place of the theme's (Compose's
    /// `outlinedToggleButtonColors()` and `outlinedToggleButtonColors(…)`).
    pub fn outlined_toggle_button_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
        #[default(Color::Unspecified)] checked_container_color: Color,
        #[default(Color::Unspecified)] checked_content_color: Color,
    ) -> ToggleButtonColors {
        named_args!(Self::default_outlined_toggle_button_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            content_color = content_color,
            disabled_container_color = disabled_container_color,
            disabled_content_color = disabled_content_color,
            checked_container_color = checked_container_color,
            checked_content_color = checked_content_color,
        ))
    }

    /// The shapes of a toggle button `button_height` tall (Compose's `shapesFor`): round, pressed and checked in
    /// the corners of the nearest of the extra small to extra large sizes (the small size's [`Self::shapes`]).
    pub fn shapes_for(&self, button_height: Dp) -> ToggleButtonShapes {
        let x_small_height = ButtonDefaults::ExtraSmallContainerHeight;
        let small_height = ButtonDefaults::MinHeight();
        let medium_height = ButtonDefaults::MediumContainerHeight();
        let large_height = ButtonDefaults::LargeContainerHeight();
        let x_large_height = ButtonDefaults::ExtraLargeContainerHeight;
        if button_height <= (x_small_height + small_height) / 2.0 {
            named_args!(self.shapes(
                shape = Some(Self::shape()),
                pressed_shape = Some(Self::extra_small_pressed_shape()),
                checked_shape = Some(Self::extra_small_checked_square_shape()),
            ))
        } else if button_height <= (small_height + medium_height) / 2.0 {
            self.shapes()
        } else if button_height <= (medium_height + large_height) / 2.0 {
            named_args!(self.shapes(
                shape = Some(Self::shape()),
                pressed_shape = Some(Self::medium_pressed_shape()),
                checked_shape = Some(Self::medium_checked_square_shape()),
            ))
        } else if button_height <= (large_height + x_large_height) / 2.0 {
            named_args!(self.shapes(
                shape = Some(Self::shape()),
                pressed_shape = Some(Self::large_pressed_shape()),
                checked_shape = Some(Self::large_checked_square_shape()),
            ))
        } else {
            named_args!(self.shapes(
                shape = Some(Self::shape()),
                pressed_shape = Some(Self::extra_large_pressed_shape()),
                checked_shape = Some(Self::extra_large_checked_square_shape()),
            ))
        }
    }
}

// ---- ToggleButtonColors ----------------------------------------------------------------------------------------

/// A toggle button's colors (Compose's `ToggleButtonColors`): its container's and its content's, unchecked,
/// disabled and checked. Equal by its colors.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct ToggleButtonColors {
    /// The container's color, unchecked.
    pub container_color: Color,
    /// The content's color, unchecked.
    pub content_color: Color,
    /// The container's color, disabled.
    pub disabled_container_color: Color,
    /// The content's color, disabled.
    pub disabled_content_color: Color,
    /// The container's color, checked.
    pub checked_container_color: Color,
    /// The content's color, checked.
    pub checked_content_color: Color,
}

impl ToggleButtonColors {
    /// Kotlin: `ToggleButtonColors(containerColor: Color, contentColor: Color, disabledContainerColor: Color, disabledContentColor: Color, checkedContainerColor: Color, checkedContentColor: Color)`
    pub fn new(
        container_color: Color,
        content_color: Color,
        disabled_container_color: Color,
        disabled_content_color: Color,
        checked_container_color: Color,
        checked_content_color: Color,
    ) -> ToggleButtonColors {
        ToggleButtonColors {
            container_color,
            content_color,
            disabled_container_color,
            disabled_content_color,
            checked_container_color,
            checked_content_color,
        }
    }

    /// The container's color (Compose's internal `containerColor(enabled, checked)`): the checked one where
    /// enabled and checked, the unchecked one where enabled, else the disabled one.
    pub(crate) fn container_color_for(&self, enabled: bool, checked: bool) -> Color {
        if enabled && checked {
            self.checked_container_color
        } else if enabled {
            self.container_color
        } else {
            self.disabled_container_color
        }
    }

    /// The content's color (Compose's internal `contentColor(enabled, checked)`), as
    /// [`Self::container_color_for`] picks.
    pub(crate) fn content_color_for(&self, enabled: bool, checked: bool) -> Color {
        if enabled && checked {
            self.checked_content_color
        } else if enabled {
            self.content_color
        } else {
            self.disabled_content_color
        }
    }
}

#[named]
impl ToggleButtonColors {
    /// The same with the colors given (Compose's `copy`): an unspecified one keeps its own.
    pub fn copy(
        &self,
        #[default(self.container_color)] container_color: Color,
        #[default(self.content_color)] content_color: Color,
        #[default(self.disabled_container_color)] disabled_container_color: Color,
        #[default(self.disabled_content_color)] disabled_content_color: Color,
        #[default(self.checked_container_color)] checked_container_color: Color,
        #[default(self.checked_content_color)] checked_content_color: Color,
    ) -> ToggleButtonColors {
        ToggleButtonColors::new(
            container_color.take_or_else(|| self.container_color),
            content_color.take_or_else(|| self.content_color),
            disabled_container_color.take_or_else(|| self.disabled_container_color),
            disabled_content_color.take_or_else(|| self.disabled_content_color),
            checked_container_color.take_or_else(|| self.checked_container_color),
            checked_content_color.take_or_else(|| self.checked_content_color),
        )
    }
}

// ---- ToggleButtonShapes ----------------------------------------------------------------------------------------

/// The shapes a toggle button morphs between (Compose's Expressive `ToggleButtonShapes`): its `shape` at rest,
/// its `pressed_shape` while pressed and its `checked_shape` while checked. Equal where all three are.
#[derive(Clone, Debug, KotlinProperties)]
pub struct ToggleButtonShapes {
    /// The shape at rest, unchecked.
    pub shape: Arc<dyn Shape>,
    /// The shape while pressed.
    pub pressed_shape: Arc<dyn Shape>,
    /// The shape while checked.
    pub checked_shape: Arc<dyn Shape>,
}

impl ToggleButtonShapes {
    /// Kotlin: `ToggleButtonShapes(shape: Shape, pressedShape: Shape, checkedShape: Shape)`
    pub fn new(
        shape: Arc<dyn Shape>,
        pressed_shape: Arc<dyn Shape>,
        checked_shape: Arc<dyn Shape>,
    ) -> ToggleButtonShapes {
        ToggleButtonShapes { shape, pressed_shape, checked_shape }
    }

    /// Whether all three are rounded corner shapes (Compose's internal `ToggleButtonShapes.hasRoundedCornerShapes`;
    /// ButtonGroup and SplitButton ask it too).
    pub(crate) fn has_rounded_corner_shapes(&self) -> bool {
        rounded_corner_shape_of(&*self.shape).is_some()
            && rounded_corner_shape_of(&*self.pressed_shape).is_some()
            && rounded_corner_shape_of(&*self.checked_shape).is_some()
    }

    /// Whether all three are corner based shapes (Compose's internal `ToggleButtonShapes.hasCornerBasedShapes`).
    pub(crate) fn has_corner_based_shapes(&self) -> bool {
        corner_based_shape_of(&*self.shape).is_some()
            && corner_based_shape_of(&*self.pressed_shape).is_some()
            && corner_based_shape_of(&*self.checked_shape).is_some()
    }
}

#[named]
impl ToggleButtonShapes {
    /// The same with the shapes given (Compose's `copy(shape, pressedShape, checkedShape)`: `None` keeps its own).
    pub fn copy(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
        #[default(None)] checked_shape: Option<Arc<dyn Shape>>,
    ) -> ToggleButtonShapes {
        ToggleButtonShapes::new(
            shape.unwrap_or_else(|| self.shape.clone()),
            pressed_shape.unwrap_or_else(|| self.pressed_shape.clone()),
            checked_shape.unwrap_or_else(|| self.checked_shape.clone()),
        )
    }
}

impl PartialEq for ToggleButtonShapes {
    fn eq(&self, other: &Self) -> bool {
        *self.shape == *other.shape
            && *self.pressed_shape == *other.pressed_shape
            && *self.checked_shape == *other.checked_shape
    }
}

/// Compose's `hashCode`: the shapes', each as its `Debug` says (shapes of the same corners alike).
impl Hash for ToggleButtonShapes {
    fn hash<H: Hasher>(&self, state: &mut H) {
        struct Writer<'h, H: Hasher>(&'h mut H);
        impl<H: Hasher> fmt::Write for Writer<'_, H> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                self.0.write(s.as_bytes());
                Ok(())
            }
        }
        for shape in [&self.shape, &self.pressed_shape, &self.checked_shape] {
            let _ = fmt::write(&mut Writer(state), format_args!("{shape:?}"));
        }
    }
}

// ---- Shape morphing --------------------------------------------------------------------------------------------

/// The shape of a toggle button now (Compose's private `shapeByInteraction(shapes, pressed, checked,
/// animationSpec)`): the pressed one while `pressed`, else the checked one while `checked`, else its shape —
/// morphing corner by corner along `animation_spec` where all three are corner based (rounded ones as rounded), a
/// state of its own for each `shapes` (Compose's `key(shapes)`); else switched.
#[kompozable]
fn shape_by_interaction(
    shapes: ToggleButtonShapes,
    pressed: bool,
    checked: bool,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
) -> Arc<dyn Shape> {
    let shape = if pressed {
        shapes.pressed_shape.clone()
    } else if checked {
        shapes.checked_shape.clone()
    } else {
        shapes.shape.clone()
    };
    // (Compose's `key(shapes)` compares the shapes by `equals`: an identity of each run of equal shapes here)
    let seen = remember(|| Rc::new(RefCell::new((shapes.clone(), 0u64))));
    let generation = {
        let mut seen = seen.borrow_mut();
        if seen.0 != shapes {
            *seen = (shapes.clone(), seen.1 + 1);
        }
        seen.1
    };
    if shapes.has_rounded_corner_shapes() {
        let rounded = rounded_corner_shape_of(&*shape).expect("a rounded corner shape");
        key(&generation, || remember_animated_shape(rounded, animation_spec))
    } else if shapes.has_corner_based_shapes() {
        let corner_based = corner_based_shape_of(&*shape).expect("a corner based shape");
        key(&generation, || remember_animated_shape(corner_based, animation_spec))
    } else {
        shape
    }
}

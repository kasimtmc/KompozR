//! Material's buttons, as Compose's Button.kt: the filled [`Button`] (high emphasis), the [`ElevatedButton`] (a
//! tonal one with a shadow), the [`FilledTonalButton`], the [`OutlinedButton`] (medium emphasis) and the
//! [`TextButton`] (low emphasis) — a clickable surface of the button's colors and shape, its label in the theme's
//! `label_large`, at least 58 by 40 dp, a shadow that rises and falls with its interactions ([`ButtonElevation`]) —,
//! and their Material Expressive forms whose shape morphs to a pressed one as they are pressed ([`ButtonShapes`]:
//! [`ButtonWithShapes`] and the others). [`ButtonDefaults`] has their colors, elevations, shapes, paddings and the
//! Expressive sizes (extra small to extra large).
//!
//! Compose's overloads taking `shapes: ButtonShapes` are composables of their own here: [`ButtonWithShapes`]
//! (Kotlin's `Button(onClick, shapes, …)`), [`ElevatedButtonWithShapes`], [`FilledTonalButtonWithShapes`],
//! [`OutlinedButtonWithShapes`] and [`TextButtonWithShapes`].

use crate::color_scheme::ColorScheme;
use crate::internal::animated_shape::remember_animated_shape;
use crate::internal::elevation::AnimateElevation;
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::material_theme::MaterialTheme;
use crate::precision_pointer::should_use_precision_pointer_component_sizing_state;
use crate::shapes::Shapes;
use crate::surface::ClickableSurface;
use crate::tokens::{
    BaselineButtonTokens, ButtonLargeTokens, ButtonMediumTokens, ButtonSmallTokens, ButtonXLargeTokens,
    ButtonXSmallTokens, ColorSchemeKeyTokens, ElevatedButtonTokens, FilledButtonTokens, FilledTonalButtonTokens,
    MotionSchemeKeyTokens, OutlinedButtonTokens, TextButtonTokens,
};
use kompoz_animation::{Animatable, AnimationVector1D, FiniteAnimationSpec, VectorConverter};
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{
    AbsoluteCutCornerShape, AbsoluteRoundedCornerShape, BorderStroke, Color, CornerBasedShape, CutCornerShape, Dp,
    Offset, RoundedCornerShape, Shape, UnitExt,
};
use kompoz_runtime::{
    KotlinProperties, State, key, kompozable, kompozable_block, mutable_state_list_of, named, named_args, remember,
};
use kompoz_text::TextStyle;
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- The buttons -----------------------------------------------------------------------------------------------

/// A filled button (Compose's `Button`, Material's high-emphasis button: "Save", "Join now", "Confirm"):
/// `content` in a row, centered both ways, padded by `content_padding`, at least [`ButtonDefaults::MinWidth`] by
/// [`ButtonDefaults::MinHeight`], on a clickable surface of `shape` in `colors`' container color (the disabled one
/// where not `enabled`) under `border`, its text in the theme's `label_large` in `colors`' content color, with a
/// shadow of `elevation` that follows its interactions (none: no shadow); `on_click` called when it is clicked
/// (not when disabled); a button to accessibility. Its interactions are emitted to `interaction_source` (one of its
/// own where none).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn Button(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ButtonDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(ButtonDefaults.button_colors())] colors: ButtonColors,
    #[default(Some(ButtonDefaults.button_elevation()))] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(ButtonDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let container_color = colors.container_color_for(enabled);
    let content_color = colors.content_color_for(enabled);
    let shadow_elevation = match elevation {
        Some(elevation) => elevation.shadow_elevation(enabled, &interaction_source).value(),
        None => Dp(0.0),
    };
    ButtonSurface(
        on_click,
        modifier,
        enabled,
        shape,
        container_color,
        content_color,
        shadow_elevation,
        border,
        content_padding,
        interaction_source,
        content,
    );
}

/// A filled button whose shape morphs between `shapes`' as it is pressed (Compose's Expressive `Button(onClick,
/// shapes, …)`): corner by corner along the theme's default effects spec where both shapes are corner based (no
/// bounce), else switching between them. Its content padding is [`ButtonDefaults::content_padding_for`] its min
/// height by default; the rest as [`Button`]'s.
///
/// Kotlin: `Button(onClick: () -> Unit, shapes: ButtonShapes)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ButtonWithShapes(
    on_click: impl Fn() + 'static,
    shapes: ButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ButtonDefaults.button_colors())] colors: ButtonColors,
    #[default(Some(ButtonDefaults.button_elevation()))] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(ButtonDefaults.content_padding_for(ButtonDefaults::MinHeight()))] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    // TODO Load the motionScheme tokens from the component tokens file
    // MotionSchemeKeyTokens.DefaultEffects is intentional here to prevent
    // any bounce in this component.
    let default_animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>();
    let pressed = interaction_source.collect_is_pressed_as_state().value();
    let container_color = colors.container_color_for(enabled);
    let content_color = colors.content_color_for(enabled);
    let shadow_elevation = match elevation {
        Some(elevation) => elevation.shadow_elevation(enabled, &interaction_source).value(),
        None => Dp(0.0),
    };
    let button_shape = shape_by_interaction(shapes, pressed, default_animation_spec);
    ButtonSurface(
        on_click,
        modifier,
        enabled,
        button_shape,
        container_color,
        content_color,
        shadow_elevation,
        border,
        content_padding,
        interaction_source,
        content,
    );
}

/// What both forms of Compose's `Button` compose: the clickable surface, a button to accessibility, its label's
/// color and style, the padded row.
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
fn ButtonSurface(
    on_click: impl Fn() + 'static,
    modifier: Modifier,
    enabled: bool,
    shape: Arc<dyn Shape>,
    container_color: Color,
    content_color: Color,
    shadow_elevation: Dp,
    border: Option<BorderStroke>,
    content_padding: PaddingValues,
    interaction_source: MutableInteractionSource,
    content: impl Fn() + 'static,
) {
    let content = Rc::new(content);
    ClickableSurface(
        on_click,
        modifier = modifier.semantics(|s| s.set_role(Role::Button)),
        enabled = enabled,
        shape = shape,
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
                        .default_min_size_with(ButtonDefaults::MinWidth, ButtonDefaults::MinHeight())
                        .padding_values(content_padding.clone()),
                    horizontal_arrangement = Arrangement::Center,
                    vertical_alignment = Alignment::CenterVertically,
                    content = move || content(),
                );
            });
        },
    );
}

/// An elevated button (Compose's `ElevatedButton`): a [`FilledTonalButton`] with a shadow, for a button that must
/// stand apart from a patterned container — the surface container low color, 1 dp up (more as it is hovered or
/// focused). See [`Button`] for the rest.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ElevatedButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ButtonDefaults::elevated_shape())] shape: Arc<dyn Shape>,
    #[default(ButtonDefaults.elevated_button_colors())] colors: ButtonColors,
    #[default(Some(ButtonDefaults.elevated_button_elevation()))] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(ButtonDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    Button(
        on_click,
        modifier = modifier,
        enabled = enabled,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = content_padding,
        interaction_source = interaction_source,
        content = content,
    );
}

/// An elevated button whose shape morphs between `shapes`' as it is pressed (Compose's Expressive
/// `ElevatedButton(onClick, shapes, …)`): see [`ElevatedButton`] and [`ButtonWithShapes`].
///
/// Kotlin: `ElevatedButton(onClick: () -> Unit, shapes: ButtonShapes)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ElevatedButtonWithShapes(
    on_click: impl Fn() + 'static,
    shapes: ButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ButtonDefaults.elevated_button_colors())] colors: ButtonColors,
    #[default(Some(ButtonDefaults.elevated_button_elevation()))] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(ButtonDefaults.content_padding_for(ButtonDefaults::MinHeight()))] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    ButtonWithShapes(
        on_click,
        shapes,
        modifier = modifier,
        enabled = enabled,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = content_padding,
        interaction_source = interaction_source,
        content = content,
    );
}

/// A filled tonal button (Compose's `FilledTonalButton`): between a [`Button`] and an [`OutlinedButton`] in
/// emphasis — the secondary container color, flat (a shadow only as it is hovered). See [`Button`] for the rest.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn FilledTonalButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ButtonDefaults::filled_tonal_shape())] shape: Arc<dyn Shape>,
    #[default(ButtonDefaults.filled_tonal_button_colors())] colors: ButtonColors,
    #[default(Some(ButtonDefaults.filled_tonal_button_elevation()))] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(ButtonDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    Button(
        on_click,
        modifier = modifier,
        enabled = enabled,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = content_padding,
        interaction_source = interaction_source,
        content = content,
    );
}

/// A filled tonal button whose shape morphs between `shapes`' as it is pressed (Compose's Expressive
/// `FilledTonalButton(onClick, shapes, …)`): see [`FilledTonalButton`] and [`ButtonWithShapes`].
///
/// Kotlin: `FilledTonalButton(onClick: () -> Unit, shapes: ButtonShapes)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn FilledTonalButtonWithShapes(
    on_click: impl Fn() + 'static,
    shapes: ButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ButtonDefaults.filled_tonal_button_colors())] colors: ButtonColors,
    #[default(Some(ButtonDefaults.filled_tonal_button_elevation()))] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(ButtonDefaults.content_padding_for(ButtonDefaults::MinHeight()))] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    ButtonWithShapes(
        on_click,
        shapes,
        modifier = modifier,
        enabled = enabled,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = content_padding,
        interaction_source = interaction_source,
        content = content,
    );
}

/// An outlined button (Compose's `OutlinedButton`, medium emphasis): transparent, no shadow, in a line of the
/// scheme's outline variant ([`ButtonDefaults::outlined_button_border`] of `enabled`: a tenth of it disabled),
/// its label primary. `border`: the outer `None` is that default (Kotlin's `outlinedButtonBorder(enabled)`),
/// `Some(None)` no line. See [`Button`] for the rest.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn OutlinedButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ButtonDefaults::outlined_shape())] shape: Arc<dyn Shape>,
    #[default(ButtonDefaults.outlined_button_colors())] colors: ButtonColors,
    #[default(None)] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(ButtonDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let border = border.unwrap_or_else(|| Some(named_args!(ButtonDefaults.outlined_button_border(enabled = enabled))));
    Button(
        on_click,
        modifier = modifier,
        enabled = enabled,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = content_padding,
        interaction_source = interaction_source,
        content = content,
    );
}

/// An outlined button whose shape morphs between `shapes`' as it is pressed (Compose's Expressive
/// `OutlinedButton(onClick, shapes, …)`): see [`OutlinedButton`] (its `border`) and [`ButtonWithShapes`].
///
/// Kotlin: `OutlinedButton(onClick: () -> Unit, shapes: ButtonShapes)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn OutlinedButtonWithShapes(
    on_click: impl Fn() + 'static,
    shapes: ButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ButtonDefaults.outlined_button_colors())] colors: ButtonColors,
    #[default(None)] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(ButtonDefaults.content_padding_for(ButtonDefaults::MinHeight()))] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let border = border.unwrap_or_else(|| Some(named_args!(ButtonDefaults.outlined_button_border(enabled = enabled))));
    ButtonWithShapes(
        on_click,
        shapes,
        modifier = modifier,
        enabled = enabled,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = content_padding,
        interaction_source = interaction_source,
        content = content,
    );
}

/// A text button (Compose's `TextButton`, low emphasis: a dialog's or a card's actions): no container, no
/// shadow, its label primary, padded 12 dp at the sides ([`ButtonDefaults::TextButtonContentPadding`]). See
/// [`Button`] for the rest.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TextButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ButtonDefaults::text_shape())] shape: Arc<dyn Shape>,
    #[default(ButtonDefaults.text_button_colors())] colors: ButtonColors,
    #[default(None)] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(ButtonDefaults::TextButtonContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    Button(
        on_click,
        modifier = modifier,
        enabled = enabled,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = content_padding,
        interaction_source = interaction_source,
        content = content,
    );
}

/// A text button whose shape morphs between `shapes`' as it is pressed (Compose's Expressive `TextButton(onClick,
/// shapes, …)`): see [`TextButton`] and [`ButtonWithShapes`] (its padding [`ButtonDefaults::content_padding_for`]
/// its min height, not the text button's).
///
/// Kotlin: `TextButton(onClick: () -> Unit, shapes: ButtonShapes)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TextButtonWithShapes(
    on_click: impl Fn() + 'static,
    shapes: ButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(ButtonDefaults.text_button_colors())] colors: ButtonColors,
    #[default(None)] elevation: Option<ButtonElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(ButtonDefaults.content_padding_for(ButtonDefaults::MinHeight()))] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    ButtonWithShapes(
        on_click,
        shapes,
        modifier = modifier,
        enabled = enabled,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = content_padding,
        interaction_source = interaction_source,
        content = content,
    );
}

// ---- ButtonDefaults --------------------------------------------------------------------------------------------

/// The buttons' defaults (Compose's `ButtonDefaults`): paddings, sizes (Material Expressive's extra small to extra
/// large too: [`ButtonDefaults::content_padding_for`], [`ButtonDefaults::icon_size_for`] and the others pick by a
/// button's height), shapes, colors, elevations, the outlined button's border. Plain values are associated consts
/// (or functions where they depend on the precision pointer sizing, or make a padding); what Compose reads in
/// composition or computes from arguments are methods of the value (`ButtonDefaults.button_colors()`).
#[derive(Clone, Copy, Debug)]
pub struct ButtonDefaults;

/// Whether components take their precision pointer sizes (Compose's `shouldUsePrecisionPointerComponentSizing.value`).
fn precision_pointer() -> bool {
    should_use_precision_pointer_component_sizing_state().value()
}

impl ButtonDefaults {
    const ButtonLeadingSpace: Dp = BaselineButtonTokens::LeadingSpace;
    const ButtonTrailingSpace: Dp = BaselineButtonTokens::TrailingSpace;
    const ButtonWithIconStartpadding: Dp = Dp(16.0);
    const SmallStartPadding: Dp = ButtonSmallTokens::LeadingSpace;
    const SmallEndPadding: Dp = ButtonSmallTokens::TrailingSpace;
    const ButtonVerticalPadding: Dp = Dp(8.0);
    const TextButtonHorizontalPadding: Dp = Dp(12.0);
    const TextButtonWithIconHorizontalEndPadding: Dp = Dp(16.0);

    /// The padding of a button's content: 24 dp at the sides, 8 above and below (Compose's
    /// `ButtonDefaults.ContentPadding`).
    pub fn ContentPadding() -> PaddingValues {
        PaddingValues::new(
            Self::ButtonLeadingSpace,
            Self::ButtonVerticalPadding,
            Self::ButtonTrailingSpace,
            Self::ButtonVerticalPadding,
        )
    }

    /// The padding of a button's content with an icon at its start: 16 dp before it (Compose's
    /// `ButtonWithIconContentPadding`).
    pub fn ButtonWithIconContentPadding() -> PaddingValues {
        PaddingValues::new(
            Self::ButtonWithIconStartpadding,
            Self::ButtonVerticalPadding,
            Self::ButtonTrailingSpace,
            Self::ButtonVerticalPadding,
        )
    }

    /// A small button's padding, 8 dp above and below (Compose's hidden `SmallButtonContentPadding`, kept for
    /// binary compatibility).
    #[deprecated(note = "For binary compatibility")]
    pub fn SmallButtonContentPadding() -> PaddingValues {
        PaddingValues::new(
            Self::SmallStartPadding,
            Self::ButtonVerticalPadding,
            Self::SmallEndPadding,
            Self::ButtonVerticalPadding,
        )
    }

    /// A small button's padding: 16 dp at the sides, 10 above and below (8 with the precision pointer sizing)
    /// (Compose's Expressive `SmallContentPadding`).
    pub fn SmallContentPadding() -> PaddingValues {
        PaddingValues::new(
            Self::SmallStartPadding,
            Self::SmallVerticalPadding(),
            Self::SmallEndPadding,
            Self::SmallVerticalPadding(),
        )
    }

    /// A small button's padding, smaller at an end with an icon (Compose's private `getSmallContentPadding`).
    fn get_small_content_padding(has_start_icon: bool, has_end_icon: bool) -> PaddingValues {
        PaddingValues::new(
            if has_start_icon { Self::IconSmallHorizontalPadding() } else { Self::SmallStartPadding },
            Self::SmallVerticalPadding(),
            if has_end_icon { Self::IconSmallHorizontalPadding() } else { Self::SmallEndPadding },
            Self::SmallVerticalPadding(),
        )
    }

    /// An extra small button's padding: 12 dp at the sides, 6 above and below (Compose's Expressive
    /// `ExtraSmallContentPadding`).
    pub fn ExtraSmallContentPadding() -> PaddingValues {
        // TODO update with the value from ButtonXSmallTokens.kt once it's been corrected
        PaddingValues::new(Dp(12.0), Dp(6.0), Dp(12.0), Dp(6.0))
    }

    /// A medium button's padding: 24 dp at the sides, 16 above and below (12 with the precision pointer sizing)
    /// (Compose's Expressive `MediumContentPadding`).
    pub fn MediumContentPadding() -> PaddingValues {
        PaddingValues::new(
            Self::MediumLeadingPadding,
            Self::MediumVerticalPadding(),
            Self::MediumTrailingPadding,
            Self::MediumVerticalPadding(),
        )
    }

    /// A medium button's padding, other at an end with an icon (Compose's private `getMediumContentPadding`).
    fn get_medium_content_padding(has_leading_icon: bool, has_trailing_icon: bool) -> PaddingValues {
        PaddingValues::new(
            if has_leading_icon { Self::IconMediumLeadingPadding() } else { Self::MediumLeadingPadding },
            Self::MediumVerticalPadding(),
            if has_trailing_icon { Self::IconMediumTrailingPadding() } else { Self::MediumTrailingPadding },
            Self::MediumVerticalPadding(),
        )
    }

    /// A large button's padding: 48 dp at the sides, 32 above and below (32 and 14 with the precision pointer
    /// sizing) (Compose's Expressive `LargeContentPadding`).
    pub fn LargeContentPadding() -> PaddingValues {
        PaddingValues::new(
            Self::LargeLeadingPadding(),
            Self::LargeVerticalPadding(),
            Self::LargeTrailingPadding(),
            Self::LargeVerticalPadding(),
        )
    }

    /// A large button's padding, other at an end with an icon (Compose's private `getLargeContentPadding`).
    fn get_large_content_padding(has_leading_icon: bool, has_trailing_icon: bool) -> PaddingValues {
        PaddingValues::new(
            if has_leading_icon { Self::IconLargeLeadingPadding() } else { Self::LargeLeadingPadding() },
            Self::LargeVerticalPadding(),
            if has_trailing_icon { Self::IconLargeTrailingPadding() } else { Self::LargeTrailingPadding() },
            Self::LargeVerticalPadding(),
        )
    }

    /// An extra large button's padding: 64 dp at the sides, 48 above and below (Compose's Expressive
    /// `ExtraLargeContentPadding`).
    pub fn ExtraLargeContentPadding() -> PaddingValues {
        PaddingValues::new(ButtonXLargeTokens::LeadingSpace, Dp(48.0), ButtonXLargeTokens::TrailingSpace, Dp(48.0))
    }

    /// A text button's padding: 12 dp at the sides, the rest [`ContentPadding`](Self::ContentPadding)'s (Compose's
    /// `TextButtonContentPadding`).
    pub fn TextButtonContentPadding() -> PaddingValues {
        let content_padding = Self::ContentPadding();
        PaddingValues::new(
            Self::TextButtonHorizontalPadding,
            content_padding.calculate_top_padding(),
            Self::TextButtonHorizontalPadding,
            content_padding.calculate_bottom_padding(),
        )
    }

    /// A text button's padding with an icon: 16 dp after the label (Compose's `TextButtonWithIconContentPadding`).
    pub fn TextButtonWithIconContentPadding() -> PaddingValues {
        let content_padding = Self::ContentPadding();
        PaddingValues::new(
            Self::TextButtonHorizontalPadding,
            content_padding.calculate_top_padding(),
            Self::TextButtonWithIconHorizontalEndPadding,
            content_padding.calculate_bottom_padding(),
        )
    }

    /// A button's least width, 58 dp (Compose's `MinWidth`): a button stays at least this wide in a layout that
    /// allows it.
    pub const MinWidth: Dp = Dp(58.0);

    /// A button's least height, the small button's 40 dp (36 with the precision pointer sizing) (Compose's
    /// `MinHeight`). (Compose reads the sizing once, as its object is made; it is read as asked here: on the
    /// desktop it never changes.)
    pub fn MinHeight() -> Dp {
        if precision_pointer() { Dp(36.0) } else { ButtonSmallTokens::ContainerHeight }
    }

    /// An extra small button's height, 32 dp (Compose's Expressive `ExtraSmallContainerHeight`).
    pub const ExtraSmallContainerHeight: Dp = ButtonXSmallTokens::ContainerHeight;

    /// A medium button's height, 56 dp (46 with the precision pointer sizing) (Compose's Expressive
    /// `MediumContainerHeight`).
    pub fn MediumContainerHeight() -> Dp {
        if precision_pointer() { Dp(46.0) } else { ButtonMediumTokens::ContainerHeight }
    }

    /// A large button's height, 96 dp (54 with the precision pointer sizing) (Compose's Expressive
    /// `LargeContainerHeight`).
    pub fn LargeContainerHeight() -> Dp {
        if precision_pointer() { Dp(54.0) } else { ButtonLargeTokens::ContainerHeight }
    }

    /// An extra large button's height, 136 dp (Compose's Expressive `ExtraLargeContainerHeight`).
    pub const ExtraLargeContainerHeight: Dp = ButtonXLargeTokens::ContainerHeight;

    /// The size of a button's icon, 18 dp (Compose's `IconSize`).
    // TODO update with the correct value in BaselineButtonTokens when available
    pub const IconSize: Dp = Dp(18.0);

    /// An extra small button's icon size, 20 dp (Compose's Expressive `ExtraSmallIconSize`).
    pub const ExtraSmallIconSize: Dp = ButtonXSmallTokens::IconSize;

    /// A small button's icon size, 20 dp (Compose's Expressive `SmallIconSize`).
    pub const SmallIconSize: Dp = ButtonSmallTokens::IconSize;

    /// A medium button's icon size, 24 dp (Compose's Expressive `MediumIconSize`).
    pub const MediumIconSize: Dp = ButtonMediumTokens::IconSize;

    /// A large button's icon size, 32 dp (24 with the precision pointer sizing) (Compose's Expressive
    /// `LargeIconSize`).
    pub fn LargeIconSize() -> Dp {
        if precision_pointer() { Dp(24.0) } else { ButtonLargeTokens::IconSize }
    }

    /// An extra large button's icon size, 40 dp (Compose's Expressive `ExtraLargeIconSize`).
    pub const ExtraLargeIconSize: Dp = ButtonXLargeTokens::IconSize;

    /// The space between a button's icon and its label, 8 dp (Compose's `IconSpacing`): a `Spacer` of it.
    pub const IconSpacing: Dp = ButtonSmallTokens::IconLabelSpace;

    /// An extra small button's space between icon and label, 4 dp (Compose's Expressive `ExtraSmallIconSpacing`).
    // TODO use the value from ButtonXSmallTokens.kt once it's been corrected
    pub const ExtraSmallIconSpacing: Dp = Dp(4.0);

    /// A medium button's space between icon and label, 8 dp (Compose's Expressive `MediumIconSpacing`).
    pub const MediumIconSpacing: Dp = ButtonMediumTokens::IconLabelSpace;

    /// A large button's space between icon and label, 12 dp (8 with the precision pointer sizing) (Compose's
    /// Expressive `LargeIconSpacing`).
    pub fn LargeIconSpacing() -> Dp {
        if precision_pointer() { Dp(8.0) } else { ButtonLargeTokens::IconLabelSpace }
    }

    /// An extra large button's space between icon and label, 16 dp (Compose's Expressive `ExtraLargeIconSpacing`).
    pub const ExtraLargeIconSpacing: Dp = ButtonXLargeTokens::IconLabelSpace;

    /// A square button's shape, the theme's medium corners (Compose's Expressive `squareShape`).
    pub fn square_shape() -> Arc<dyn Shape> {
        ButtonSmallTokens::ContainerShapeSquare.value()
    }

    /// A pressed button's shape, the theme's small corners (Compose's Expressive `pressedShape`).
    pub fn pressed_shape() -> Arc<dyn Shape> {
        ButtonSmallTokens::PressedContainerShape.value()
    }

    /// A pressed extra small button's shape (Compose's Expressive `extraSmallPressedShape`).
    pub fn extra_small_pressed_shape() -> Arc<dyn Shape> {
        ButtonXSmallTokens::PressedContainerShape.value()
    }

    /// A pressed medium button's shape (Compose's Expressive `mediumPressedShape`).
    pub fn medium_pressed_shape() -> Arc<dyn Shape> {
        ButtonMediumTokens::PressedContainerShape.value()
    }

    /// A pressed large button's shape (Compose's Expressive `largePressedShape`).
    pub fn large_pressed_shape() -> Arc<dyn Shape> {
        ButtonLargeTokens::PressedContainerShape.value()
    }

    /// A pressed extra large button's shape (Compose's Expressive `extraLargePressedShape`).
    pub fn extra_large_pressed_shape() -> Arc<dyn Shape> {
        ButtonXLargeTokens::PressedContainerShape.value()
    }

    /// A filled button's shape, round ends (Compose's `shape`).
    pub fn shape() -> Arc<dyn Shape> {
        ButtonSmallTokens::ContainerShapeRound.value()
    }

    /// An elevated button's shape (Compose's `elevatedShape`).
    pub fn elevated_shape() -> Arc<dyn Shape> {
        ButtonSmallTokens::ContainerShapeRound.value()
    }

    /// A filled tonal button's shape (Compose's `filledTonalShape`).
    pub fn filled_tonal_shape() -> Arc<dyn Shape> {
        ButtonSmallTokens::ContainerShapeRound.value()
    }

    /// An outlined button's shape (Compose's `outlinedShape`).
    pub fn outlined_shape() -> Arc<dyn Shape> {
        ButtonSmallTokens::ContainerShapeRound.value()
    }

    /// A text button's shape (Compose's `textShape`).
    pub fn text_shape() -> Arc<dyn Shape> {
        ButtonSmallTokens::ContainerShapeRound.value()
    }

    /// A theme's default button shapes: round, the small corners pressed (Compose's internal
    /// `Shapes.defaultButtonShapes`; its per-instance cache isn't kept, as the color schemes' aren't).
    pub(crate) fn default_button_shapes(shapes: &Shapes) -> ButtonShapes {
        ButtonShapes::new(
            shapes.from_token(ButtonSmallTokens::ContainerShapeRound),
            shapes.from_token(ButtonSmallTokens::PressedContainerShape),
        )
    }

    /// A filled button's colors in `scheme` (Compose's internal `ColorScheme.defaultButtonColors`): primary, its
    /// label on primary; disabled, the surface's text at 12% under it at 38%.
    pub(crate) fn default_button_colors(scheme: &ColorScheme) -> ButtonColors {
        ButtonColors::new(
            scheme.from_token(FilledButtonTokens::ContainerColor),
            scheme.from_token(FilledButtonTokens::LabelTextColor),
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
        )
    }

    /// An elevated button's colors in `scheme` (Compose's internal `ColorScheme.defaultElevatedButtonColors`): the
    /// surface container low, its label primary.
    pub(crate) fn default_elevated_button_colors(scheme: &ColorScheme) -> ButtonColors {
        ButtonColors::new(
            scheme.from_token(ElevatedButtonTokens::ContainerColor),
            scheme.from_token(ElevatedButtonTokens::LabelTextColor),
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
        )
    }

    /// A filled tonal button's colors in `scheme` (Compose's internal `ColorScheme.defaultFilledTonalButtonColors`):
    /// the secondary container, its label on it.
    pub(crate) fn default_filled_tonal_button_colors(scheme: &ColorScheme) -> ButtonColors {
        ButtonColors::new(
            scheme.from_token(FilledTonalButtonTokens::ContainerColor),
            scheme.from_token(FilledTonalButtonTokens::LabelTextColor),
            named_args!(
                scheme
                    .from_token(FilledTonalButtonTokens::DisabledContainerColor)
                    .copy(alpha = FilledTonalButtonTokens::DisabledContainerOpacity)
            ),
            named_args!(
                scheme
                    .from_token(FilledTonalButtonTokens::DisabledLabelTextColor)
                    .copy(alpha = FilledTonalButtonTokens::DisabledLabelTextOpacity)
            ),
        )
    }

    /// An outlined button's colors in `scheme` (Compose's internal `ColorScheme.defaultOutlinedButtonColors`):
    /// transparent, its label primary.
    pub(crate) fn default_outlined_button_colors(scheme: &ColorScheme) -> ButtonColors {
        ButtonColors::new(
            Color::Transparent,
            scheme.from_token(OutlinedButtonTokens::LabelTextColor),
            Color::Transparent,
            named_args!(
                scheme
                    .from_token(OutlinedButtonTokens::DisabledLabelTextColor)
                    .copy(alpha = OutlinedButtonTokens::DisabledLabelTextOpacity)
            ),
        )
    }

    /// A text button's colors in `scheme` (Compose's internal `ColorScheme.defaultTextButtonColors`): transparent,
    /// its label primary.
    pub(crate) fn default_text_button_colors(scheme: &ColorScheme) -> ButtonColors {
        ButtonColors::new(
            Color::Transparent,
            // TODO replace with the token value once it's corrected
            scheme.from_token(ColorSchemeKeyTokens::Primary),
            Color::Transparent,
            named_args!(
                scheme
                    .from_token(TextButtonTokens::DisabledLabelColor)
                    .copy(alpha = TextButtonTokens::DisabledLabelOpacity)
            ),
        )
    }

    /// The outlined button's line at full strength whether enabled or not (Compose's deprecated property
    /// `outlinedButtonBorder`, its Kotlin name; [`outlined_button_border`](Self::outlined_button_border) is the
    /// function).
    #[deprecated(
        note = "Please use the version that takes an `enabled` param to get the `BorderStroke` with the correct opacity"
    )]
    pub fn outlined_button_border_property() -> BorderStroke {
        BorderStroke(ButtonSmallTokens::OutlinedOutlineWidth, OutlinedButtonTokens::OutlineColor.value())
    }

    // (the precision pointer sizes: Compose's private vals)
    fn SmallVerticalPadding() -> Dp {
        if precision_pointer() { Dp(8.0) } else { Dp(10.0) }
    }
    fn IconSmallHorizontalPadding() -> Dp {
        if precision_pointer() { Dp(12.0) } else { Self::SmallStartPadding }
    }
    const MediumLeadingPadding: Dp = ButtonMediumTokens::LeadingSpace;
    const MediumTrailingPadding: Dp = ButtonMediumTokens::TrailingSpace;
    fn MediumVerticalPadding() -> Dp {
        if precision_pointer() { Dp(12.0) } else { Dp(16.0) }
    }
    fn IconMediumLeadingPadding() -> Dp {
        if precision_pointer() { Dp(20.0) } else { ButtonMediumTokens::LeadingSpace }
    }
    fn IconMediumTrailingPadding() -> Dp {
        if precision_pointer() { Dp(20.0) } else { ButtonMediumTokens::TrailingSpace }
    }
    fn LargeVerticalPadding() -> Dp {
        if precision_pointer() { Dp(14.0) } else { Dp(32.0) }
    }
    fn LargeLeadingPadding() -> Dp {
        if precision_pointer() { Dp(32.0) } else { ButtonLargeTokens::LeadingSpace }
    }
    fn LargeTrailingPadding() -> Dp {
        if precision_pointer() { Dp(32.0) } else { ButtonLargeTokens::TrailingSpace }
    }
    fn IconLargeLeadingPadding() -> Dp {
        if precision_pointer() { Dp(28.0) } else { ButtonLargeTokens::LeadingSpace }
    }
    fn IconLargeTrailingPadding() -> Dp {
        if precision_pointer() { Dp(28.0) } else { ButtonLargeTokens::TrailingSpace }
    }
}

#[named]
impl ButtonDefaults {
    /// The theme's button shapes with `shape` and `pressed_shape` where given (Compose's Expressive `shapes()` and
    /// `shapes(shape, pressedShape)`): round, the theme's small corners pressed.
    pub fn shapes(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
    ) -> ButtonShapes {
        named_args!(
            Self::default_button_shapes(&MaterialTheme::shapes()).copy(shape = shape, pressed_shape = pressed_shape)
        )
    }

    /// A filled button's colors, each given one other than unspecified in place of the theme's (Compose's
    /// `buttonColors()` and `buttonColors(containerColor, contentColor, disabledContainerColor,
    /// disabledContentColor)`).
    pub fn button_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
    ) -> ButtonColors {
        named_args!(Self::default_button_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            content_color = content_color,
            disabled_container_color = disabled_container_color,
            disabled_content_color = disabled_content_color,
        ))
    }

    /// An elevated button's colors, each given one in place of the theme's (Compose's `elevatedButtonColors()` and
    /// `elevatedButtonColors(…)`).
    pub fn elevated_button_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
    ) -> ButtonColors {
        named_args!(Self::default_elevated_button_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            content_color = content_color,
            disabled_container_color = disabled_container_color,
            disabled_content_color = disabled_content_color,
        ))
    }

    /// A filled tonal button's colors, each given one in place of the theme's (Compose's
    /// `filledTonalButtonColors()` and `filledTonalButtonColors(…)`).
    pub fn filled_tonal_button_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
    ) -> ButtonColors {
        named_args!(Self::default_filled_tonal_button_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            content_color = content_color,
            disabled_container_color = disabled_container_color,
            disabled_content_color = disabled_content_color,
        ))
    }

    /// An outlined button's colors, each given one in place of the theme's (Compose's `outlinedButtonColors()` and
    /// `outlinedButtonColors(…)`).
    pub fn outlined_button_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
    ) -> ButtonColors {
        named_args!(Self::default_outlined_button_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            content_color = content_color,
            disabled_container_color = disabled_container_color,
            disabled_content_color = disabled_content_color,
        ))
    }

    /// A text button's colors, each given one in place of the theme's (Compose's `textButtonColors()` and
    /// `textButtonColors(…)`).
    pub fn text_button_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_content_color: Color,
    ) -> ButtonColors {
        named_args!(Self::default_text_button_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            content_color = content_color,
            disabled_container_color = disabled_container_color,
            disabled_content_color = disabled_content_color,
        ))
    }

    /// A filled button's elevation (Compose's `buttonElevation`): flat, 1 dp hovered.
    pub fn button_elevation(
        &self,
        #[default(FilledButtonTokens::ContainerElevation)] default_elevation: Dp,
        #[default(FilledButtonTokens::PressedContainerElevation)] pressed_elevation: Dp,
        #[default(FilledButtonTokens::FocusedContainerElevation)] focused_elevation: Dp,
        #[default(FilledButtonTokens::HoveredContainerElevation)] hovered_elevation: Dp,
        #[default(FilledButtonTokens::DisabledContainerElevation)] disabled_elevation: Dp,
    ) -> ButtonElevation {
        ButtonElevation::new(
            default_elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            disabled_elevation,
        )
    }

    /// An elevated button's elevation (Compose's `elevatedButtonElevation`): 1 dp, 3 hovered, none disabled.
    pub fn elevated_button_elevation(
        &self,
        #[default(ElevatedButtonTokens::ContainerElevation)] default_elevation: Dp,
        #[default(ElevatedButtonTokens::PressedContainerElevation)] pressed_elevation: Dp,
        #[default(ElevatedButtonTokens::FocusedContainerElevation)] focused_elevation: Dp,
        #[default(ElevatedButtonTokens::HoveredContainerElevation)] hovered_elevation: Dp,
        #[default(ElevatedButtonTokens::DisabledContainerElevation)] disabled_elevation: Dp,
    ) -> ButtonElevation {
        ButtonElevation::new(
            default_elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            disabled_elevation,
        )
    }

    /// A filled tonal button's elevation (Compose's `filledTonalButtonElevation`): flat, 1 dp hovered.
    pub fn filled_tonal_button_elevation(
        &self,
        #[default(FilledTonalButtonTokens::ContainerElevation)] default_elevation: Dp,
        #[default(FilledTonalButtonTokens::PressedContainerElevation)] pressed_elevation: Dp,
        #[default(FilledTonalButtonTokens::FocusContainerElevation)] focused_elevation: Dp,
        #[default(FilledTonalButtonTokens::HoverContainerElevation)] hovered_elevation: Dp,
        #[default(Dp(0.0))] disabled_elevation: Dp,
    ) -> ButtonElevation {
        ButtonElevation::new(
            default_elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            disabled_elevation,
        )
    }

    /// The outlined button's line: 1 dp of the scheme's outline variant, a tenth of it where not `enabled`
    /// (Compose's `outlinedButtonBorder(enabled)`).
    pub fn outlined_button_border(&self, #[default(true)] enabled: bool) -> BorderStroke {
        BorderStroke(
            ButtonSmallTokens::OutlinedOutlineWidth,
            if enabled {
                OutlinedButtonTokens::OutlineColor.value()
            } else {
                named_args!(
                    OutlinedButtonTokens::OutlineColor
                        .value()
                        .copy(alpha = OutlinedButtonTokens::DisabledContainerOpacity)
                )
            },
        )
    }

    /// The shapes of a button `button_height` tall (Compose's Expressive `shapesFor`): round, pressed in the
    /// corners of the nearest of the extra small to extra large sizes.
    pub fn shapes_for(&self, button_height: Dp) -> ButtonShapes {
        let x_small_height = Self::ExtraSmallContainerHeight;
        let small_height = Self::MinHeight();
        let medium_height = Self::MediumContainerHeight();
        let large_height = Self::LargeContainerHeight();
        let x_large_height = Self::ExtraLargeContainerHeight;
        if button_height <= (x_small_height + small_height) / 2.0 {
            named_args!(
                self.shapes(shape = Some(Self::shape()), pressed_shape = Some(Self::extra_small_pressed_shape()))
            )
        } else if button_height <= (small_height + medium_height) / 2.0 {
            self.shapes()
        } else if button_height <= (medium_height + large_height) / 2.0 {
            named_args!(self.shapes(shape = Some(Self::shape()), pressed_shape = Some(Self::medium_pressed_shape())))
        } else if button_height <= (large_height + x_large_height) / 2.0 {
            named_args!(self.shapes(shape = Some(Self::shape()), pressed_shape = Some(Self::large_pressed_shape())))
        } else {
            named_args!(
                self.shapes(shape = Some(Self::shape()), pressed_shape = Some(Self::extra_large_pressed_shape()))
            )
        }
    }

    /// The padding of a button `button_height` tall (Compose's `contentPaddingFor(buttonHeight, hasStartIcon,
    /// hasEndIcon)`): of the size it is at least, an end with an icon padded less (small to large sizes).
    pub fn content_padding_for(
        &self,
        button_height: Dp,
        #[default(false)] has_start_icon: bool,
        #[default(false)] has_end_icon: bool,
    ) -> PaddingValues {
        let small_height = Self::MinHeight();
        let medium_height = Self::MediumContainerHeight();
        let large_height = Self::LargeContainerHeight();
        let x_large_height = Self::ExtraLargeContainerHeight;
        if button_height < small_height {
            Self::ExtraSmallContentPadding()
        } else if button_height < medium_height {
            Self::get_small_content_padding(has_start_icon, has_end_icon)
        } else if button_height < large_height {
            Self::get_medium_content_padding(has_start_icon, has_end_icon)
        } else if button_height < x_large_height {
            Self::get_large_content_padding(has_start_icon, has_end_icon)
        } else {
            Self::ExtraLargeContentPadding()
        }
    }

    /// The padding of a button `button_height` tall without its icons' (Compose's hidden
    /// `contentPaddingFor(buttonHeight)`, kept for binary compatibility).
    #[deprecated(note = "Deprecated in favor of function with hasLeadingIcon and hasTrailingIcon params")]
    pub fn content_padding_for_height(&self, button_height: Dp) -> PaddingValues {
        let small_height = Self::MinHeight();
        let medium_height = Self::MediumContainerHeight();
        let large_height = Self::LargeContainerHeight();
        let x_large_height = Self::ExtraLargeContainerHeight;
        if button_height < small_height {
            Self::ExtraSmallContentPadding()
        } else if button_height < medium_height {
            Self::SmallContentPadding()
        } else if button_height < large_height {
            Self::MediumContentPadding()
        } else if button_height < x_large_height {
            Self::LargeContentPadding()
        } else {
            Self::ExtraLargeContentPadding()
        }
    }

    /// The icon size of a button `button_height` tall (Compose's Expressive `iconSizeFor`).
    pub fn icon_size_for(&self, button_height: Dp) -> Dp {
        let small_height = Self::MinHeight();
        let medium_height = Self::MediumContainerHeight();
        let large_height = Self::LargeContainerHeight();
        let x_large_height = Self::ExtraLargeContainerHeight;
        if button_height < small_height {
            Self::ExtraSmallIconSize
        } else if button_height < medium_height {
            Self::SmallIconSize
        } else if button_height < large_height {
            Self::MediumIconSize
        } else if button_height < x_large_height {
            Self::LargeIconSize()
        } else {
            Self::ExtraLargeIconSize
        }
    }

    /// The space between icon and label of a button `button_height` tall (Compose's Expressive `iconSpacingFor`).
    pub fn icon_spacing_for(&self, button_height: Dp) -> Dp {
        let small_height = Self::MinHeight();
        let medium_height = Self::MediumContainerHeight();
        let large_height = Self::LargeContainerHeight();
        let x_large_height = Self::ExtraLargeContainerHeight;
        if button_height < small_height {
            Self::ExtraSmallIconSpacing
        } else if button_height < medium_height {
            Self::IconSpacing
        } else if button_height < large_height {
            Self::MediumIconSpacing
        } else if button_height < x_large_height {
            Self::LargeIconSpacing()
        } else {
            Self::ExtraLargeIconSpacing
        }
    }

    /// The label style of a button `button_height` tall (Compose's Expressive `textStyleFor`): the theme's
    /// `label_large`, `title_medium`, `headline_small` or `headline_large` (smaller with the precision pointer
    /// sizing).
    pub fn text_style_for(&self, button_height: Dp) -> TextStyle {
        let medium_height = Self::MediumContainerHeight();
        let large_height = Self::LargeContainerHeight();
        let x_large_height = Self::ExtraLargeContainerHeight;
        let typography = MaterialTheme::typography();
        if button_height < medium_height {
            typography.label_large
        } else if button_height < large_height {
            if precision_pointer() {
                named_args!(typography.title_medium.copy(font_size = 15.sp(), line_height = 22.sp()))
            } else {
                typography.title_medium
            }
        } else if button_height < x_large_height {
            if precision_pointer() {
                named_args!(typography.headline_small.copy(font_size = 18.sp(), line_height = 26.sp()))
            } else {
                typography.headline_small
            }
        } else {
            typography.headline_large
        }
    }
}

// ---- ButtonElevation -------------------------------------------------------------------------------------------

/// A button's elevation in its states (Compose's `ButtonElevation`): its shadow animates between them as it is
/// pressed, hovered or focused, and snaps to the disabled one. Made by [`ButtonDefaults`]' elevation functions;
/// equal by its elevations.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ButtonElevation {
    default_elevation: Dp,
    pressed_elevation: Dp,
    focused_elevation: Dp,
    hovered_elevation: Dp,
    disabled_elevation: Dp,
}

impl ButtonElevation {
    /// Compose's internal constructor.
    pub(crate) fn new(
        default_elevation: Dp,
        pressed_elevation: Dp,
        focused_elevation: Dp,
        hovered_elevation: Dp,
        disabled_elevation: Dp,
    ) -> ButtonElevation {
        ButtonElevation {
            default_elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            disabled_elevation,
        }
    }

    /// The shadow's elevation as state (Compose's internal composable `shadowElevation`): the disabled one where
    /// not `enabled`, else that of the last of `interaction_source`'s interactions going on (a press, a hover, a
    /// focus), or the default one, animated to as [`AnimateElevation`] says.
    #[track_caller]
    pub(crate) fn shadow_elevation(&self, enabled: bool, interaction_source: &InteractionSource) -> State<Dp> {
        self.animate_elevation(enabled, interaction_source)
    }

    /// Compose's private `animateElevation`.
    #[track_caller]
    fn animate_elevation(&self, enabled: bool, interaction_source: &InteractionSource) -> State<Dp> {
        let this = *self;
        let interaction_source = interaction_source.clone();
        kompozable_block!({
            let interactions = remember(|| mutable_state_list_of(Vec::<Arc<dyn Interaction>>::new()));
            let collected = interactions.clone();
            let collecting = interaction_source.interactions().collect(move |interaction| {
                track_interaction(&collected, interaction);
            });
            LaunchedEffect(interaction_source.clone(), collecting);

            let interaction = interactions.with(|list| list.last().cloned());

            let target = if !enabled {
                this.disabled_elevation
            } else {
                match &interaction {
                    Some(i) if i.is::<PressInteraction::Press>() => this.pressed_elevation,
                    Some(i) if i.is::<HoverInteraction::Enter>() => this.hovered_elevation,
                    Some(i) if i.is::<FocusInteraction::Focus>() => this.focused_elevation,
                    _ => this.default_elevation,
                }
            };

            let animatable: Animatable<Dp, AnimationVector1D> = remember(|| Animatable((target, Dp::VectorConverter)));

            let a = animatable.clone();
            LaunchedEffect(target, async move {
                if a.target_value() != target {
                    if !enabled {
                        // No transition when moving to a disabled state
                        a.snap_to(target).await;
                    } else {
                        let last_target = a.target_value();
                        let last_interaction: Option<Arc<dyn Interaction>> = if last_target == this.pressed_elevation {
                            Some(Arc::new(PressInteraction::Press::new(Offset::Zero)) as Arc<dyn Interaction>)
                        } else if last_target == this.hovered_elevation {
                            Some(Arc::new(HoverInteraction::Enter::new()) as Arc<dyn Interaction>)
                        } else if last_target == this.focused_elevation {
                            Some(Arc::new(FocusInteraction::Focus::new()) as Arc<dyn Interaction>)
                        } else {
                            None
                        };
                        a.animate_elevation(target, last_interaction.as_deref(), interaction.as_deref()).await;
                    }
                }
            });

            animatable.as_state()
        })
    }
}

/// An interaction's part in what goes on (Compose's `when (interaction)` of a collected one): a hover, a focus or
/// a press added, its end taking it away (the first of it).
pub(crate) fn track_interaction(
    interactions: &kompoz_runtime::SnapshotStateList<Arc<dyn Interaction>>,
    interaction: Arc<dyn Interaction>,
) {
    if interaction.is::<HoverInteraction::Enter>()
        || interaction.is::<FocusInteraction::Focus>()
        || interaction.is::<PressInteraction::Press>()
    {
        interactions.add(interaction);
    } else {
        interactions.update(|list| remove_ended(list, &*interaction));
    }
}

/// Takes the first interaction `end` ends out of `list` (Kotlin's `remove(interaction.enter)` and the others).
pub(crate) fn remove_ended(list: &mut Vec<Arc<dyn Interaction>>, end: &dyn Interaction) {
    let started = |i: &Arc<dyn Interaction>| -> bool {
        if let Some(exit) = end.downcast_ref::<HoverInteraction::Exit>() {
            i.downcast_ref::<HoverInteraction::Enter>() == Some(&exit.enter)
        } else if let Some(unfocus) = end.downcast_ref::<FocusInteraction::Unfocus>() {
            i.downcast_ref::<FocusInteraction::Focus>() == Some(&unfocus.focus)
        } else if let Some(release) = end.downcast_ref::<PressInteraction::Release>() {
            i.downcast_ref::<PressInteraction::Press>() == Some(&release.press)
        } else if let Some(cancel) = end.downcast_ref::<PressInteraction::Cancel>() {
            i.downcast_ref::<PressInteraction::Press>() == Some(&cancel.press)
        } else {
            false
        }
    };
    if let Some(index) = list.iter().position(started) {
        list.remove(index);
    }
}

// ---- ButtonColors ----------------------------------------------------------------------------------------------

/// A button's colors (Compose's `ButtonColors`): its container's and its content's, enabled and disabled. Equal by
/// its colors.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct ButtonColors {
    pub container_color: Color,
    pub content_color: Color,
    pub disabled_container_color: Color,
    pub disabled_content_color: Color,
}

impl ButtonColors {
    /// Kotlin: `ButtonColors(containerColor: Color, contentColor: Color, disabledContainerColor: Color, disabledContentColor: Color)`
    pub fn new(
        container_color: Color,
        content_color: Color,
        disabled_container_color: Color,
        disabled_content_color: Color,
    ) -> ButtonColors {
        ButtonColors { container_color, content_color, disabled_container_color, disabled_content_color }
    }

    /// The container's color, `enabled` or not (Compose's internal `containerColor(enabled)`; named apart from the
    /// property's getter here).
    pub(crate) fn container_color_for(&self, enabled: bool) -> Color {
        if enabled { self.container_color } else { self.disabled_container_color }
    }

    /// The content's color, `enabled` or not (Compose's internal `contentColor(enabled)`).
    pub(crate) fn content_color_for(&self, enabled: bool) -> Color {
        if enabled { self.content_color } else { self.disabled_content_color }
    }
}

#[named]
impl ButtonColors {
    /// The same with each color given one other than unspecified (Compose's `copy`).
    pub fn copy(
        &self,
        #[default(self.container_color)] container_color: Color,
        #[default(self.content_color)] content_color: Color,
        #[default(self.disabled_container_color)] disabled_container_color: Color,
        #[default(self.disabled_content_color)] disabled_content_color: Color,
    ) -> ButtonColors {
        ButtonColors::new(
            container_color.take_or_else(|| self.container_color),
            content_color.take_or_else(|| self.content_color),
            disabled_container_color.take_or_else(|| self.disabled_container_color),
            disabled_content_color.take_or_else(|| self.disabled_content_color),
        )
    }
}

// ---- ButtonShapes ----------------------------------------------------------------------------------------------

/// The shapes an Expressive button morphs between (Compose's Expressive `ButtonShapes`): its shape at rest and
/// pressed. Equal by its shapes.
#[derive(Clone, KotlinProperties)]
pub struct ButtonShapes {
    pub shape: Arc<dyn Shape>,
    pub pressed_shape: Arc<dyn Shape>,
}

impl ButtonShapes {
    /// Kotlin: `ButtonShapes(shape: Shape, pressedShape: Shape)`
    pub fn new(shape: Arc<dyn Shape>, pressed_shape: Arc<dyn Shape>) -> ButtonShapes {
        ButtonShapes { shape, pressed_shape }
    }

    /// Whether both shapes are rounded corner shapes (Compose's internal `hasRoundedCornerShapes`).
    pub(crate) fn has_rounded_corner_shapes(&self) -> bool {
        rounded_corner_shape_of(&*self.shape).is_some() && rounded_corner_shape_of(&*self.pressed_shape).is_some()
    }

    /// Whether both shapes are corner based (Compose's internal `hasCornerBasedShapes`).
    pub(crate) fn has_corner_based_shapes(&self) -> bool {
        corner_based_shape_of(&*self.shape).is_some() && corner_based_shape_of(&*self.pressed_shape).is_some()
    }
}

#[named]
impl ButtonShapes {
    /// The same with the shapes given (Compose's `copy(shape, pressedShape)`: `None` keeps its own).
    pub fn copy(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
    ) -> ButtonShapes {
        ButtonShapes::new(
            shape.unwrap_or_else(|| self.shape.clone()),
            pressed_shape.unwrap_or_else(|| self.pressed_shape.clone()),
        )
    }
}

impl PartialEq for ButtonShapes {
    fn eq(&self, other: &Self) -> bool {
        *self.shape == *other.shape && *self.pressed_shape == *other.pressed_shape
    }
}

impl fmt::Debug for ButtonShapes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ButtonShapes").field("shape", &self.shape).field("pressed_shape", &self.pressed_shape).finish()
    }
}

/// `shape` as a [`RoundedCornerShape`] where it is one (Kotlin's `is RoundedCornerShape`: a theme's rounded
/// [`CornerBasedShape`] too).
pub(crate) fn rounded_corner_shape_of(shape: &dyn Shape) -> Option<RoundedCornerShape> {
    let any = shape.shape_as_any();
    if let Some(rounded) = any.downcast_ref::<RoundedCornerShape>() {
        return Some(rounded.clone());
    }
    match any.downcast_ref::<CornerBasedShape>() {
        Some(CornerBasedShape::Rounded(rounded)) => Some(rounded.clone()),
        _ => None,
    }
}

/// `shape` as a [`CornerBasedShape`] where it is one (Kotlin's `is CornerBasedShape`: rounded or cut corners,
/// relative or absolute).
pub(crate) fn corner_based_shape_of(shape: &dyn Shape) -> Option<CornerBasedShape> {
    let any = shape.shape_as_any();
    if let Some(shape) = any.downcast_ref::<CornerBasedShape>() {
        Some(shape.clone())
    } else if let Some(shape) = any.downcast_ref::<RoundedCornerShape>() {
        Some(shape.clone().into())
    } else if let Some(shape) = any.downcast_ref::<CutCornerShape>() {
        Some(shape.clone().into())
    } else if let Some(shape) = any.downcast_ref::<AbsoluteRoundedCornerShape>() {
        Some(shape.clone().into())
    } else {
        any.downcast_ref::<AbsoluteCutCornerShape>().map(|shape| shape.clone().into())
    }
}

/// The shape of a button `pressed` or not (Compose's private `shapeByInteraction`): morphing corner by corner along
/// `animation_spec` where both of `shapes` are corner based (rounded ones as rounded), a state of its own for each
/// `shapes` (Compose's `key(shapes)`); else the one of the state.
#[kompozable]
fn shape_by_interaction(
    shapes: ButtonShapes,
    pressed: bool,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
) -> Arc<dyn Shape> {
    let shape = if pressed { shapes.pressed_shape.clone() } else { shapes.shape.clone() };
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

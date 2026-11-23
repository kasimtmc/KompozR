//! Material Expressive's split buttons, as Compose's SplitButton.kt (material3 1.5.0-alpha17): a leading button
//! (the action) and a trailing one (typically opening a menu) side by side, [`SplitButtonDefaults::Spacing`] apart,
//! laid out by [`SplitButtonLayout`]. Their inner corners morph as they are pressed, the trailing one turns round
//! while checked ([`SplitButtonShapes`]); [`SplitButtonDefaults`] has the buttons (filled, tonal, outlined,
//! elevated), each Expressive size's shapes, paddings and icon sizes.
//!
//! Compose's `SplitButtonDefaults.TrailingButton(checked, onCheckedChange, …)` overload is
//! [`SplitButtonDefaults::ToggleableTrailingButton`] here (doc-lined: Kotlin calls it `TrailingButton`).

use crate::button::{ButtonColors, ButtonDefaults, ButtonElevation, corner_based_shape_of, rounded_corner_shape_of};
use crate::horizontal_center_optically::{
    HorizontalCenterOpticallyModifiers, as_shape_with_horizontal_center_optically,
};
use crate::interactive_component_size::{InteractiveComponentSizeModifiers, LocalMinimumInteractiveComponentSize};
use crate::internal::animated_shape::remember_animated_shape;
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::material_theme::MaterialTheme;
use crate::shapes::ShapeDefaults;
use crate::surface::{ClickableSurface, ToggleableSurface};
use crate::tokens::{
    ButtonSmallTokens, MotionSchemeKeyTokens, SplitButtonLargeTokens, SplitButtonMediumTokens, SplitButtonSmallTokens,
    SplitButtonXLargeTokens, SplitButtonXSmallTokens, StateTokens,
};
use kompoz_animation::FiniteAnimationSpec;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{BorderStroke, CircleShape, CornerSize, Dp, RoundedCornerShape, Shape};
use kompoz_runtime::{KompozitionLocalProvider, kompozable, kompozable_block, named, named_args, remember};
use kompoz_ui::Constraints;
use kompoz_ui::draw::{DefaultBlendMode, Fill};
use kompoz_ui::layout::{Layout, measure_policy};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

const LeadingButtonLayoutId: &str = "LeadingButton";
const TrailingButtonLayoutId: &str = "TrailingButton";

// ---- SplitButtonLayout -----------------------------------------------------------------------------------------

/// A split button's layout (Compose's Expressive `SplitButtonLayout`): `leading_button` (the action) then
/// `trailing_button` (typically opening a menu of related actions), `spacing` apart. The leading button is
/// measured first, unconstrained below; the trailing one gets what is left of the width and exactly the leading
/// one's height. The two take no 48 dp room of their own (the layout's `LocalMinimumInteractiveComponentSize` is 0
/// inside): the layout keeps it instead (`minimum_interactive_component_size`).
#[kompozable]
pub fn SplitButtonLayout(
    leading_button: impl Fn() + 'static,
    trailing_button: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(SplitButtonDefaults::Spacing)] spacing: Dp,
) {
    let (leading_button, trailing_button) = (Rc::new(leading_button), Rc::new(trailing_button));
    Layout(
        modifier = modifier.minimum_interactive_component_size(),
        content = move || {
            let (leading_button, trailing_button) = (leading_button.clone(), trailing_button.clone());
            // Override min component size enforcement to avoid create extra padding internally
            // Enforce it on the parent instead
            KompozitionLocalProvider(&[LocalMinimumInteractiveComponentSize.provides(Dp(0.0))], move || {
                let (leading_button, trailing_button) = (leading_button.clone(), trailing_button.clone());
                Box(
                    modifier = Modifier.layout_id(LeadingButtonLayoutId),
                    content_alignment = Alignment::Center,
                    content = move || leading_button(),
                );
                Box(
                    modifier = Modifier.layout_id(TrailingButtonLayoutId),
                    content_alignment = Alignment::Center,
                    content = move || trailing_button(),
                );
            });
        },
        measure_policy = measure_policy(move |scope, measurables, constraints| {
            let loose_constraints: Constraints = named_args!(constraints.copy(min_width = 0, min_height = 0));
            let spacing_px = scope.round_to_px(spacing);

            let leading_button_placeable = measurables
                .iter()
                .find(|m| m.layout_id().is_some_and(|id| id.is(&LeadingButtonLayoutId)))
                .expect("Collection contains no element matching the predicate.")
                .measure(loose_constraints);

            let trailing_button_placeable = measurables
                .iter()
                .find(|m| m.layout_id().is_some_and(|id| id.is(&TrailingButtonLayoutId)))
                .expect("Collection contains no element matching the predicate.")
                .measure({
                    let offset = loose_constraints.offset(-(leading_button_placeable.width() + spacing_px), 0);
                    named_args!(offset.copy(
                        min_height = leading_button_placeable.height(),
                        max_height = leading_button_placeable.height(),
                    ))
                });

            let content_width = leading_button_placeable.width() + trailing_button_placeable.width() + spacing_px;
            let content_height = leading_button_placeable.height().max(trailing_button_placeable.height());

            let width = constraints.constrain_width(content_width);
            let height = constraints.constrain_height(content_height);

            scope.layout(width, height, move |p| {
                p.place_relative(&leading_button_placeable, 0, 0);
                p.place_relative(&trailing_button_placeable, leading_button_placeable.width() + spacing_px, 0);
            })
        }),
    );
}

// ---- SplitButtonDefaults ---------------------------------------------------------------------------------------

/// The split buttons' defaults (Compose's Expressive `SplitButtonDefaults`): the spacing, corner sizes, paddings,
/// heights and icon sizes of each Expressive size (extra small to extra large; [`Self::leading_button_shapes_for`]
/// and the others pick by a button's height), and the leading and trailing buttons themselves (filled, tonal,
/// outlined, elevated) — composables of the value (`SplitButtonDefaults.LeadingButton(…)`), as Compose's
/// object's. Compose's hidden deprecated members (`InnerCornerSize`, `LeadingButtonContentPadding`,
/// `TrailingButtonContentPadding`, `leadingButtonShapes`, `trailingButtonShapes`) aren't ported (Kotlin can't
/// call them).
#[derive(Clone, Copy, Debug)]
pub struct SplitButtonDefaults;

impl SplitButtonDefaults {
    /// The leading button's icon size, 20 dp (Compose's `LeadingIconSize`).
    pub const LeadingIconSize: Dp = ButtonSmallTokens::IconSize;

    /// The trailing button's icon size, 22 dp (Compose's `TrailingIconSize`).
    pub const TrailingIconSize: Dp = SplitButtonSmallTokens::TrailingIconSize;

    /// The space between the two buttons, 2 dp (Compose's `Spacing`).
    pub const Spacing: Dp = SplitButtonSmallTokens::BetweenSpace;

    /// The inner corners of an extra small split button, 4 dp (Compose's `ExtraSmallInnerCornerSize`).
    pub const ExtraSmallInnerCornerSize: CornerSize = SplitButtonXSmallTokens::InnerCornerCornerSize;

    /// The inner corners of a small split button, 4 dp (Compose's `SmallInnerCornerSize`).
    pub const SmallInnerCornerSize: CornerSize = SplitButtonSmallTokens::InnerCornerCornerSize;

    /// The inner corners of a medium split button, 4 dp (Compose's `MediumInnerCornerSize`).
    pub const MediumInnerCornerSize: CornerSize = SplitButtonMediumTokens::InnerCornerCornerSize;

    /// The inner corners of a large split button, 8 dp (Compose's `LargeInnerCornerSize`).
    pub const LargeInnerCornerSize: CornerSize = SplitButtonLargeTokens::InnerCornerCornerSize;

    /// The inner corners of an extra large split button, 12 dp (Compose's `ExtraLargeInnerCornerSize`).
    pub const ExtraLargeInnerCornerSize: CornerSize = SplitButtonXLargeTokens::InnerCornerCornerSize;

    /// The inner corners of a pressed extra small split button (Compose's `ExtraSmallInnerCornerSizePressed`).
    pub const ExtraSmallInnerCornerSizePressed: CornerSize = SplitButtonXSmallTokens::InnerPressedCornerCornerSize;

    /// The inner corners of a pressed small split button (Compose's `SmallInnerCornerSizePressed`).
    pub const SmallInnerCornerSizePressed: CornerSize = SplitButtonSmallTokens::InnerPressedCornerCornerSize;

    /// The inner corners of a pressed medium split button (Compose's `MediumInnerCornerSizePressed`).
    pub const MediumInnerCornerSizePressed: CornerSize = SplitButtonMediumTokens::InnerPressedCornerCornerSize;

    /// The inner corners of a pressed large split button (Compose's `LargeInnerCornerSizePressed`).
    pub const LargeInnerCornerSizePressed: CornerSize = SplitButtonLargeTokens::InnerPressedCornerCornerSize;

    /// The inner corners of a pressed extra large split button (Compose's `ExtraLargeInnerCornerSizePressed`).
    pub const ExtraLargeInnerCornerSizePressed: CornerSize = SplitButtonXLargeTokens::InnerPressedCornerCornerSize;

    /// The outer corners, full (Compose's `OuterCornerSize`).
    pub const OuterCornerSize: CornerSize = ShapeDefaults::CornerFull;

    /// The padding of an extra small leading button (Compose's `ExtraSmallLeadingButtonContentPadding`).
    pub fn ExtraSmallLeadingButtonContentPadding() -> PaddingValues {
        Self::horizontal(
            SplitButtonXSmallTokens::LeadingButtonLeadingSpace,
            SplitButtonXSmallTokens::LeadingButtonTrailingSpace,
        )
    }

    /// The padding of a small leading button: 16 dp before, 12 after (Compose's
    /// `SmallLeadingButtonContentPadding`).
    pub fn SmallLeadingButtonContentPadding() -> PaddingValues {
        Self::horizontal(
            SplitButtonSmallTokens::LeadingButtonLeadingSpace,
            SplitButtonSmallTokens::LeadingButtonTrailingSpace,
        )
    }

    /// The padding of a medium leading button (Compose's `MediumLeadingButtonContentPadding`).
    pub fn MediumLeadingButtonContentPadding() -> PaddingValues {
        Self::horizontal(
            SplitButtonMediumTokens::LeadingButtonLeadingSpace,
            SplitButtonMediumTokens::LeadingButtonTrailingSpace,
        )
    }

    /// The padding of a large leading button (Compose's `LargeLeadingButtonContentPadding`).
    pub fn LargeLeadingButtonContentPadding() -> PaddingValues {
        Self::horizontal(
            SplitButtonLargeTokens::LeadingButtonLeadingSpace,
            SplitButtonLargeTokens::LeadingButtonTrailingSpace,
        )
    }

    /// The padding of an extra large leading button (Compose's `ExtraLargeLeadingButtonContentPadding`).
    pub fn ExtraLargeLeadingButtonContentPadding() -> PaddingValues {
        Self::horizontal(
            SplitButtonXLargeTokens::LeadingButtonLeadingSpace,
            SplitButtonXLargeTokens::LeadingButtonTrailingSpace,
        )
    }

    /// The padding of an extra small trailing button (Compose's `ExtraSmallTrailingButtonContentPadding`).
    pub fn ExtraSmallTrailingButtonContentPadding() -> PaddingValues {
        Self::horizontal(
            SplitButtonXSmallTokens::TrailingButtonLeadingSpace,
            SplitButtonXSmallTokens::TrailingButtonTrailingSpace,
        )
    }

    /// The padding of a small trailing button: 13 dp each side (Compose's `SmallTrailingButtonContentPadding`).
    pub fn SmallTrailingButtonContentPadding() -> PaddingValues {
        Self::horizontal(
            SplitButtonSmallTokens::TrailingButtonLeadingSpace,
            SplitButtonSmallTokens::TrailingButtonTrailingSpace,
        )
    }

    /// The padding of a medium trailing button (Compose's `MediumTrailingButtonContentPadding`).
    pub fn MediumTrailingButtonContentPadding() -> PaddingValues {
        Self::horizontal(
            SplitButtonMediumTokens::TrailingButtonLeadingSpace,
            SplitButtonMediumTokens::TrailingButtonTrailingSpace,
        )
    }

    /// The padding of a large trailing button (Compose's `LargeTrailingButtonContentPadding`).
    pub fn LargeTrailingButtonContentPadding() -> PaddingValues {
        Self::horizontal(
            SplitButtonLargeTokens::TrailingButtonLeadingSpace,
            SplitButtonLargeTokens::TrailingButtonTrailingSpace,
        )
    }

    /// The padding of an extra large trailing button (Compose's `ExtraLargeTrailingButtonContentPadding`).
    pub fn ExtraLargeTrailingButtonContentPadding() -> PaddingValues {
        Self::horizontal(
            SplitButtonXLargeTokens::TrailingButtonLeadingSpace,
            SplitButtonXLargeTokens::TrailingButtonTrailingSpace,
        )
    }

    /// The trailing icon of an extra small split button (Compose's `ExtraSmallTrailingButtonIconSize`).
    pub const ExtraSmallTrailingButtonIconSize: Dp = SplitButtonXSmallTokens::TrailingIconSize;

    /// The trailing icon of a small split button, 22 dp (Compose's `SmallTrailingButtonIconSize`).
    pub const SmallTrailingButtonIconSize: Dp = SplitButtonSmallTokens::TrailingIconSize;

    /// The trailing icon of a medium split button (Compose's `MediumTrailingButtonIconSize`).
    pub const MediumTrailingButtonIconSize: Dp = SplitButtonMediumTokens::TrailingIconSize;

    /// The trailing icon of a large split button (Compose's `LargeTrailingButtonIconSize`).
    pub const LargeTrailingButtonIconSize: Dp = SplitButtonLargeTokens::TrailingIconSize;

    /// The trailing icon of an extra large split button (Compose's `ExtraLargeTrailingButtonIconSize`).
    pub const ExtraLargeTrailingButtonIconSize: Dp = SplitButtonXLargeTokens::TrailingIconSize;

    const LeadingButtonMinWidth: Dp = Dp(48.0);

    /// A small split button's height, 40 dp (Compose's `SmallContainerHeight`).
    pub const SmallContainerHeight: Dp = SplitButtonSmallTokens::ContainerHeight;

    /// An extra small split button's height, 32 dp (Compose's `ExtraSmallContainerHeight`).
    pub const ExtraSmallContainerHeight: Dp = SplitButtonXSmallTokens::ContainerHeight;

    /// A medium split button's height, 56 dp (Compose's `MediumContainerHeight`).
    pub const MediumContainerHeight: Dp = SplitButtonMediumTokens::ContainerHeight;

    /// A large split button's height, 96 dp (Compose's `LargeContainerHeight`).
    pub const LargeContainerHeight: Dp = SplitButtonLargeTokens::ContainerHeight;

    /// An extra large split button's height, 136 dp (Compose's `ExtraLargeContainerHeight`).
    pub const ExtraLargeContainerHeight: Dp = SplitButtonXLargeTokens::ContainerHeight;

    const TrailingButtonMinWidth: Dp = Self::LeadingButtonMinWidth;

    const TrailingButtonStateLayerAlpha: f32 = StateTokens::PressedStateLayerOpacity;

    /// Kotlin's `PaddingValues(start, end)`.
    fn horizontal(start: Dp, end: Dp) -> PaddingValues {
        PaddingValues::new(start, Dp(0.0), end, Dp(0.0))
    }

    /// Compose's private `leadingButtonShape(endCornerSize)`: the outer corners full, the inner ones `end_corner_size`.
    fn leading_button_shape(end_corner_size: CornerSize) -> Arc<dyn Shape> {
        Arc::new(RoundedCornerShape::new(
            Self::OuterCornerSize,
            end_corner_size.clone(),
            end_corner_size,
            Self::OuterCornerSize,
        ))
    }

    /// Compose's private `TrailingCheckedShape`: a circle.
    fn trailing_checked_shape() -> Arc<dyn Shape> {
        Arc::new(CircleShape)
    }

    /// Compose's private `leadingButtonShapesInternal(endCornerSize, endCornerSizePressed)`.
    fn leading_button_shapes_internal(
        end_corner_size: CornerSize,
        end_corner_size_pressed: CornerSize,
    ) -> SplitButtonShapes {
        SplitButtonShapes::new(
            Self::leading_button_shape(end_corner_size),
            Some(Self::leading_button_shape(end_corner_size_pressed)),
            None,
        )
    }

    /// Compose's private `trailingButtonShape(startCornerSize)`: the inner corners `start_corner_size`, the outer
    /// ones full.
    fn trailing_button_shape(start_corner_size: CornerSize) -> Arc<dyn Shape> {
        Arc::new(RoundedCornerShape::new(
            start_corner_size.clone(),
            Self::OuterCornerSize,
            Self::OuterCornerSize,
            start_corner_size,
        ))
    }

    /// Compose's private `trailingButtonShapesInternal(startCornerSize, startCornerSizePressed)`: round when
    /// checked.
    fn trailing_button_shapes_internal(
        start_corner_size: CornerSize,
        start_corner_size_pressed: CornerSize,
    ) -> SplitButtonShapes {
        SplitButtonShapes::new(
            Self::trailing_button_shape(start_corner_size),
            Some(Self::trailing_button_shape(start_corner_size_pressed)),
            Some(Self::trailing_checked_shape()),
        )
    }
}

#[named]
impl SplitButtonDefaults {
    /// The leading button's shapes for a split button `button_height` tall (Compose's `leadingButtonShapesFor`):
    /// the outer corners full, the inner ones (and pressed ones) of the nearest Expressive size.
    pub fn leading_button_shapes_for(&self, button_height: Dp) -> SplitButtonShapes {
        let x_small_height = ButtonDefaults::ExtraSmallContainerHeight;
        let small_height = ButtonDefaults::MinHeight();
        let medium_height = ButtonDefaults::MediumContainerHeight();
        let large_height = ButtonDefaults::LargeContainerHeight();
        let x_large_height = ButtonDefaults::ExtraLargeContainerHeight;
        if button_height <= (x_small_height + small_height) / 2.0 {
            Self::leading_button_shapes_internal(
                Self::ExtraSmallInnerCornerSize,
                Self::ExtraSmallInnerCornerSizePressed,
            )
        } else if button_height <= (small_height + medium_height) / 2.0 {
            Self::leading_button_shapes_internal(Self::SmallInnerCornerSize, Self::SmallInnerCornerSizePressed)
        } else if button_height <= (medium_height + large_height) / 2.0 {
            Self::leading_button_shapes_internal(Self::MediumInnerCornerSize, Self::MediumInnerCornerSizePressed)
        } else if button_height <= (large_height + x_large_height) / 2.0 {
            Self::leading_button_shapes_internal(Self::LargeInnerCornerSize, Self::LargeInnerCornerSizePressed)
        } else {
            Self::leading_button_shapes_internal(
                Self::ExtraLargeInnerCornerSize,
                Self::ExtraLargeInnerCornerSizePressed,
            )
        }
    }

    /// The trailing button's shapes for a split button `button_height` tall (Compose's `trailingButtonShapesFor`):
    /// the outer corners full, the inner ones (and pressed ones) of the nearest Expressive size, a circle checked.
    pub fn trailing_button_shapes_for(&self, button_height: Dp) -> SplitButtonShapes {
        let x_small_height = ButtonDefaults::ExtraSmallContainerHeight;
        let small_height = ButtonDefaults::MinHeight();
        let medium_height = ButtonDefaults::MediumContainerHeight();
        let large_height = ButtonDefaults::LargeContainerHeight();
        let x_large_height = ButtonDefaults::ExtraLargeContainerHeight;
        if button_height <= (x_small_height + small_height) / 2.0 {
            Self::trailing_button_shapes_internal(
                Self::ExtraSmallInnerCornerSize,
                Self::ExtraSmallInnerCornerSizePressed,
            )
        } else if button_height <= (small_height + medium_height) / 2.0 {
            Self::trailing_button_shapes_internal(Self::SmallInnerCornerSize, Self::SmallInnerCornerSizePressed)
        } else if button_height <= (medium_height + large_height) / 2.0 {
            Self::trailing_button_shapes_internal(Self::MediumInnerCornerSize, Self::MediumInnerCornerSizePressed)
        } else if button_height <= (large_height + x_large_height) / 2.0 {
            Self::trailing_button_shapes_internal(Self::LargeInnerCornerSize, Self::LargeInnerCornerSizePressed)
        } else {
            Self::trailing_button_shapes_internal(
                Self::ExtraLargeInnerCornerSize,
                Self::ExtraLargeInnerCornerSizePressed,
            )
        }
    }

    /// The leading button's padding for a split button `button_height` tall (Compose's
    /// `leadingButtonContentPaddingFor`): of the size it is at least.
    pub fn leading_button_content_padding_for(&self, button_height: Dp) -> PaddingValues {
        if button_height < Self::SmallContainerHeight {
            Self::ExtraSmallLeadingButtonContentPadding()
        } else if button_height < Self::MediumContainerHeight {
            Self::SmallLeadingButtonContentPadding()
        } else if button_height < Self::LargeContainerHeight {
            Self::MediumLeadingButtonContentPadding()
        } else if button_height < Self::ExtraLargeContainerHeight {
            Self::LargeLeadingButtonContentPadding()
        } else {
            Self::ExtraLargeLeadingButtonContentPadding()
        }
    }

    /// The trailing button's padding for a split button `button_height` tall (Compose's
    /// `trailingButtonContentPaddingFor`): of the size it is at least.
    pub fn trailing_button_content_padding_for(&self, button_height: Dp) -> PaddingValues {
        if button_height < Self::SmallContainerHeight {
            Self::ExtraSmallTrailingButtonContentPadding()
        } else if button_height < Self::MediumContainerHeight {
            Self::SmallTrailingButtonContentPadding()
        } else if button_height < Self::LargeContainerHeight {
            Self::MediumTrailingButtonContentPadding()
        } else if button_height < Self::ExtraLargeContainerHeight {
            Self::LargeTrailingButtonContentPadding()
        } else {
            Self::ExtraLargeTrailingButtonContentPadding()
        }
    }

    /// The leading button's icon size for a split button `button_height` tall (Compose's
    /// `leadingButtonIconSizeFor`): a button's ([`ButtonDefaults::icon_size_for`]).
    pub fn leading_button_icon_size_for(&self, button_height: Dp) -> Dp {
        ButtonDefaults.icon_size_for(button_height)
    }

    /// The trailing button's icon size for a split button `button_height` tall (Compose's
    /// `trailingButtonIconSizeFor`): of the size it is at least.
    pub fn trailing_button_icon_size_for(&self, button_height: Dp) -> Dp {
        let small_height = ButtonDefaults::MinHeight();
        let medium_height = ButtonDefaults::MediumContainerHeight();
        let large_height = ButtonDefaults::LargeContainerHeight();
        let x_large_height = ButtonDefaults::ExtraLargeContainerHeight;
        if button_height < small_height {
            Self::ExtraSmallTrailingButtonIconSize
        } else if button_height < medium_height {
            Self::SmallTrailingButtonIconSize
        } else if button_height < large_height {
            Self::MediumTrailingButtonIconSize
        } else if button_height < x_large_height {
            Self::LargeTrailingButtonIconSize
        } else {
            Self::ExtraLargeTrailingButtonIconSize
        }
    }
}

/// The trailing buttons' row's optical centering (Compose's `when (shape)` of `TrailingButton`): toward the
/// rounder side of an animated shape or a corner based one, within the content padding; else nothing.
fn center_optically(
    shape: &Arc<dyn Shape>,
    content_padding: &PaddingValues,
    layout_direction: kompoz_graphics::LayoutDirection,
) -> Modifier {
    let max_start_offset = content_padding.calculate_start_padding(layout_direction);
    let max_end_offset = content_padding.calculate_end_padding(layout_direction);
    if let Some(shape) = as_shape_with_horizontal_center_optically(shape) {
        Modifier.horizontal_center_optically_of_with(shape, max_start_offset, max_end_offset)
    } else if let Some(shape) = corner_based_shape_of(&**shape) {
        Modifier.horizontal_center_optically_with(shape, max_start_offset, max_end_offset)
    } else {
        Modifier
    }
}

#[named]
impl SplitButtonDefaults {
    /// A split button's leading button (Compose's composable `SplitButtonDefaults.LeadingButton`): a clickable
    /// surface in `colors` (a filled button's by default), a button to accessibility, whose inner corners morph as
    /// it is pressed (`shapes`, along the theme's default effects spec); `content` in a row, centered, padded by
    /// `content_padding`, at least 48 by [`Self::SmallContainerHeight`], its text in the theme's `label_large`.
    ///
    /// Kotlin: `LeadingButton(onClick: () -> Unit, modifier: Modifier = Modifier)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn LeadingButton(
        &self,
        on_click: impl Fn() + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SplitButtonDefaults.leading_button_shapes_for(SplitButtonDefaults::SmallContainerHeight))]
        shapes: SplitButtonShapes,
        #[default(ButtonDefaults.button_colors())] colors: ButtonColors,
        #[default(Some(ButtonDefaults.button_elevation()))] elevation: Option<ButtonElevation>,
        #[default(None)] border: Option<BorderStroke>,
        #[default(SplitButtonDefaults.leading_button_content_padding_for(SplitButtonDefaults::SmallContainerHeight))]
        content_padding: PaddingValues,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));

            // TODO Load the motionScheme tokens from the component tokens file
            let default_animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>();
            let pressed = interaction_source.collect_is_pressed_as_state().value();
            let content_color = colors.content_color_for(enabled);
            let container_color = colors.container_color_for(enabled);
            let shape = shape_by_interaction(shapes, pressed, false, default_animation_spec);
            let shadow_elevation = match elevation {
                Some(elevation) => elevation.shadow_elevation(enabled, &interaction_source).value(),
                None => Dp(0.0),
            };

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
                                .default_min_size_with(
                                    SplitButtonDefaults::LeadingButtonMinWidth,
                                    SplitButtonDefaults::SmallContainerHeight,
                                )
                                .padding_values(content_padding.clone()),
                            horizontal_arrangement = Arrangement::Center,
                            vertical_alignment = Alignment::CenterVertically,
                            content = move || content(),
                        );
                    });
                },
            );
        })
    }

    /// A split button's trailing button that is clicked (Compose's composable `SplitButtonDefaults.TrailingButton(
    /// onClick, …)`): as [`Self::LeadingButton`], its inner corners on the other side; its content moved toward the
    /// rounder side to look centered (within its padding).
    ///
    /// Kotlin: `TrailingButton(onClick: () -> Unit, modifier: Modifier = Modifier)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn TrailingButton(
        &self,
        on_click: impl Fn() + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SplitButtonDefaults.trailing_button_shapes_for(SplitButtonDefaults::SmallContainerHeight))]
        shapes: SplitButtonShapes,
        #[default(ButtonDefaults.button_colors())] colors: ButtonColors,
        #[default(Some(ButtonDefaults.button_elevation()))] elevation: Option<ButtonElevation>,
        #[default(None)] border: Option<BorderStroke>,
        #[default(SplitButtonDefaults.trailing_button_content_padding_for(SplitButtonDefaults::SmallContainerHeight))]
        content_padding: PaddingValues,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));

            // TODO Load the motionScheme tokens from the component tokens file
            let default_animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>();
            let pressed = interaction_source.collect_is_pressed_as_state().value();
            let layout_direction = LocalLayoutDirection.current();
            let shape = shape_by_interaction(shapes, pressed, false, default_animation_spec);
            let content_color = colors.content_color_for(enabled);
            let container_color = colors.container_color_for(enabled);
            let shadow_elevation = match elevation {
                Some(elevation) => elevation.shadow_elevation(enabled, &interaction_source).value(),
                None => Dp(0.0),
            };

            let content = Rc::new(content);
            let row_shape = shape.clone();
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
                    let (content, content_padding, shape) =
                        (content.clone(), content_padding.clone(), row_shape.clone());
                    ProvideContentColorTextStyle(content_color, MaterialTheme::typography().label_large, move || {
                        let content = content.clone();
                        Row(
                            modifier = Modifier
                                .default_min_size_with(
                                    SplitButtonDefaults::TrailingButtonMinWidth,
                                    SplitButtonDefaults::SmallContainerHeight,
                                )
                                .then(center_optically(&shape, &content_padding, layout_direction))
                                .padding_values(content_padding.clone()),
                            horizontal_arrangement = Arrangement::Center,
                            vertical_alignment = Alignment::CenterVertically,
                            content = move || content(),
                        );
                    });
                },
            );
        })
    }

    /// A split button's trailing button that is toggled (Compose's composable `SplitButtonDefaults.TrailingButton(
    /// checked, onCheckedChange, …)`, typically opening a menu while checked): a toggleable surface in `colors`, a
    /// button to accessibility, whose inner corners morph as it is pressed and which turns round while `checked`
    /// (`shapes`), drawing its content color at the pressed state layer's 10% over itself while checked; its
    /// content moved toward the rounder side to look centered (within its padding).
    ///
    /// Kotlin: `TrailingButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn ToggleableTrailingButton(
        &self,
        checked: bool,
        on_checked_change: impl Fn(bool) + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SplitButtonDefaults.trailing_button_shapes_for(SplitButtonDefaults::SmallContainerHeight))]
        shapes: SplitButtonShapes,
        #[default(ButtonDefaults.button_colors())] colors: ButtonColors,
        #[default(Some(ButtonDefaults.button_elevation()))] elevation: Option<ButtonElevation>,
        #[default(None)] border: Option<BorderStroke>,
        #[default(SplitButtonDefaults.trailing_button_content_padding_for(SplitButtonDefaults::SmallContainerHeight))]
        content_padding: PaddingValues,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));

            // TODO Load the motionScheme tokens from the component tokens file
            let default_animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>();
            let pressed = interaction_source.collect_is_pressed_as_state().value();
            let layout_direction = LocalLayoutDirection.current();
            let shape = shape_by_interaction(shapes, pressed, checked, default_animation_spec);
            let content_color = colors.content_color_for(enabled);
            let container_color = colors.container_color_for(enabled);
            let shadow_elevation = match elevation {
                Some(elevation) => elevation.shadow_elevation(enabled, &interaction_source).value(),
                None => Dp(0.0),
            };

            let content = Rc::new(content);
            let (drawn_shape, row_shape) = (shape.clone(), shape.clone());
            ToggleableSurface(
                checked,
                on_checked_change,
                modifier = modifier
                    .draw_with_content(move |c| {
                        c.draw_content();
                        if checked {
                            let outline = drawn_shape.create_outline(c.size(), layout_direction, &c.density());
                            c.draw_outline_with(
                                &outline,
                                content_color,
                                SplitButtonDefaults::TrailingButtonStateLayerAlpha,
                                Fill,
                                None,
                                DefaultBlendMode,
                            );
                        }
                    })
                    .semantics(|s| s.set_role(Role::Button)),
                enabled = enabled,
                shape = shape,
                color = container_color,
                content_color = Some(content_color),
                shadow_elevation = shadow_elevation,
                border = border,
                interaction_source = Some(interaction_source),
                content = move || {
                    let (content, content_padding, shape) =
                        (content.clone(), content_padding.clone(), row_shape.clone());
                    ProvideContentColorTextStyle(content_color, MaterialTheme::typography().label_large, move || {
                        let content = content.clone();
                        Row(
                            modifier = Modifier
                                .default_min_size_with(
                                    SplitButtonDefaults::TrailingButtonMinWidth,
                                    SplitButtonDefaults::SmallContainerHeight,
                                )
                                .then(center_optically(&shape, &content_padding, layout_direction))
                                .padding_values(content_padding.clone()),
                            horizontal_arrangement = Arrangement::Center,
                            vertical_alignment = Alignment::CenterVertically,
                            content = move || content(),
                        );
                    });
                },
            );
        })
    }

    /// A tonal split button's leading button (Compose's composable `SplitButtonDefaults.TonalLeadingButton`): a
    /// [`Self::LeadingButton`] of the filled tonal button's colors and elevation.
    ///
    /// Kotlin: `TonalLeadingButton(onClick: () -> Unit, modifier: Modifier = Modifier)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn TonalLeadingButton(
        &self,
        on_click: impl Fn() + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SplitButtonDefaults.leading_button_shapes_for(SplitButtonDefaults::SmallContainerHeight))]
        shapes: SplitButtonShapes,
        #[default(ButtonDefaults.filled_tonal_button_colors())] colors: ButtonColors,
        #[default(Some(ButtonDefaults.filled_tonal_button_elevation()))] elevation: Option<ButtonElevation>,
        #[default(None)] border: Option<BorderStroke>,
        #[default(SplitButtonDefaults.leading_button_content_padding_for(SplitButtonDefaults::SmallContainerHeight))]
        content_padding: PaddingValues,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            SplitButtonDefaults.LeadingButton(
                on_click,
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
        })
    }

    /// A tonal split button's trailing button (Compose's composable `SplitButtonDefaults.TonalTrailingButton`): a
    /// [`Self::ToggleableTrailingButton`] of the filled tonal button's colors and elevation.
    ///
    /// Kotlin: `TonalTrailingButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn TonalTrailingButton(
        &self,
        checked: bool,
        on_checked_change: impl Fn(bool) + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SplitButtonDefaults.trailing_button_shapes_for(SplitButtonDefaults::SmallContainerHeight))]
        shapes: SplitButtonShapes,
        #[default(ButtonDefaults.filled_tonal_button_colors())] colors: ButtonColors,
        #[default(Some(ButtonDefaults.filled_tonal_button_elevation()))] elevation: Option<ButtonElevation>,
        #[default(None)] border: Option<BorderStroke>,
        #[default(SplitButtonDefaults.trailing_button_content_padding_for(SplitButtonDefaults::SmallContainerHeight))]
        content_padding: PaddingValues,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            SplitButtonDefaults.ToggleableTrailingButton(
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
        })
    }

    /// An outlined split button's leading button (Compose's composable `SplitButtonDefaults.OutlinedLeadingButton`):
    /// a [`Self::LeadingButton`] of the outlined button's colors, no shadow, its border. `border`: `None` (the
    /// default) is Kotlin's `ButtonDefaults.outlinedButtonBorder(enabled)`, `Some(None)` Kotlin's `null` (none).
    ///
    /// Kotlin: `OutlinedLeadingButton(onClick: () -> Unit, modifier: Modifier = Modifier)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn OutlinedLeadingButton(
        &self,
        on_click: impl Fn() + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SplitButtonDefaults.leading_button_shapes_for(SplitButtonDefaults::SmallContainerHeight))]
        shapes: SplitButtonShapes,
        #[default(ButtonDefaults.outlined_button_colors())] colors: ButtonColors,
        #[default(None)] elevation: Option<ButtonElevation>,
        #[default(None)] border: Option<Option<BorderStroke>>,
        #[default(SplitButtonDefaults.leading_button_content_padding_for(SplitButtonDefaults::SmallContainerHeight))]
        content_padding: PaddingValues,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            let border =
                border.unwrap_or_else(|| Some(named_args!(ButtonDefaults.outlined_button_border(enabled = enabled))));
            SplitButtonDefaults.LeadingButton(
                on_click,
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
        })
    }

    /// An outlined split button's trailing button (Compose's composable
    /// `SplitButtonDefaults.OutlinedTrailingButton`): a [`Self::ToggleableTrailingButton`] of the outlined button's
    /// colors, no shadow, its border (`border` as [`Self::OutlinedLeadingButton`]'s).
    ///
    /// Kotlin: `OutlinedTrailingButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn OutlinedTrailingButton(
        &self,
        checked: bool,
        on_checked_change: impl Fn(bool) + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SplitButtonDefaults.trailing_button_shapes_for(SplitButtonDefaults::SmallContainerHeight))]
        shapes: SplitButtonShapes,
        #[default(ButtonDefaults.outlined_button_colors())] colors: ButtonColors,
        #[default(None)] elevation: Option<ButtonElevation>,
        #[default(None)] border: Option<Option<BorderStroke>>,
        #[default(SplitButtonDefaults.trailing_button_content_padding_for(SplitButtonDefaults::SmallContainerHeight))]
        content_padding: PaddingValues,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            let border =
                border.unwrap_or_else(|| Some(named_args!(ButtonDefaults.outlined_button_border(enabled = enabled))));
            SplitButtonDefaults.ToggleableTrailingButton(
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
        })
    }

    /// An elevated split button's leading button (Compose's composable `SplitButtonDefaults.ElevatedLeadingButton`):
    /// a [`Self::LeadingButton`] of the elevated button's colors and elevation.
    ///
    /// Kotlin: `ElevatedLeadingButton(onClick: () -> Unit, modifier: Modifier = Modifier)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn ElevatedLeadingButton(
        &self,
        on_click: impl Fn() + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SplitButtonDefaults.leading_button_shapes_for(SplitButtonDefaults::SmallContainerHeight))]
        shapes: SplitButtonShapes,
        #[default(ButtonDefaults.elevated_button_colors())] colors: ButtonColors,
        #[default(Some(ButtonDefaults.elevated_button_elevation()))] elevation: Option<ButtonElevation>,
        #[default(None)] border: Option<BorderStroke>,
        #[default(SplitButtonDefaults.leading_button_content_padding_for(SplitButtonDefaults::SmallContainerHeight))]
        content_padding: PaddingValues,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            SplitButtonDefaults.LeadingButton(
                on_click,
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
        })
    }

    /// An elevated split button's trailing button (Compose's composable
    /// `SplitButtonDefaults.ElevatedTrailingButton`): a [`Self::ToggleableTrailingButton`] of the elevated button's
    /// colors and elevation.
    ///
    /// Kotlin: `ElevatedTrailingButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, modifier: Modifier = Modifier)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn ElevatedTrailingButton(
        &self,
        checked: bool,
        on_checked_change: impl Fn(bool) + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SplitButtonDefaults.trailing_button_shapes_for(SplitButtonDefaults::SmallContainerHeight))]
        shapes: SplitButtonShapes,
        #[default(ButtonDefaults.elevated_button_colors())] colors: ButtonColors,
        #[default(Some(ButtonDefaults.elevated_button_elevation()))] elevation: Option<ButtonElevation>,
        #[default(None)] border: Option<BorderStroke>,
        #[default(SplitButtonDefaults.trailing_button_content_padding_for(SplitButtonDefaults::SmallContainerHeight))]
        content_padding: PaddingValues,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            SplitButtonDefaults.ToggleableTrailingButton(
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
        })
    }
}

// ---- SplitButtonShapes -----------------------------------------------------------------------------------------

/// The shapes a split button's half morphs between (Compose's Expressive `SplitButtonShapes`): its `shape` at
/// rest, its `pressed_shape` while pressed and its `checked_shape` while checked (`None`: its shape then). Equal
/// where all three are.
#[derive(Clone)]
pub struct SplitButtonShapes {
    /// The shape at rest.
    pub shape: Arc<dyn Shape>,
    /// The shape while pressed; `None` keeps `shape`.
    pub pressed_shape: Option<Arc<dyn Shape>>,
    /// The shape while checked; `None` keeps `shape`.
    pub checked_shape: Option<Arc<dyn Shape>>,
}

impl SplitButtonShapes {
    /// Kotlin: `SplitButtonShapes(shape: Shape, pressedShape: Shape?, checkedShape: Shape?)`
    pub fn new(
        shape: Arc<dyn Shape>,
        pressed_shape: Option<Arc<dyn Shape>>,
        checked_shape: Option<Arc<dyn Shape>>,
    ) -> SplitButtonShapes {
        SplitButtonShapes { shape, pressed_shape, checked_shape }
    }

    /// Whether they are rounded corner shapes (Compose's private `hasRoundedCornerShapes`): the shapes given
    /// (a missing one doesn't count), and the shape at rest.
    fn has_rounded_corner_shapes(&self) -> bool {
        // Ignore null shapes and only check default shape for RoundedCorner
        if self.pressed_shape.as_ref().is_some_and(|s| rounded_corner_shape_of(&**s).is_none()) {
            return false;
        }
        if self.checked_shape.as_ref().is_some_and(|s| rounded_corner_shape_of(&**s).is_none()) {
            return false;
        }
        rounded_corner_shape_of(&*self.shape).is_some()
    }

    /// Whether they are corner based shapes (Compose's private `hasCornerBasedShapes`), as
    /// [`Self::has_rounded_corner_shapes`] asks.
    fn has_corner_based_shapes(&self) -> bool {
        // Ignore null shapes and only check default shape for RoundedCorner
        if self.pressed_shape.as_ref().is_some_and(|s| corner_based_shape_of(&**s).is_none()) {
            return false;
        }
        if self.checked_shape.as_ref().is_some_and(|s| corner_based_shape_of(&**s).is_none()) {
            return false;
        }
        corner_based_shape_of(&*self.shape).is_some()
    }
}

impl PartialEq for SplitButtonShapes {
    fn eq(&self, other: &Self) -> bool {
        fn same(a: &Option<Arc<dyn Shape>>, b: &Option<Arc<dyn Shape>>) -> bool {
            match (a, b) {
                (Some(a), Some(b)) => **a == **b,
                (None, None) => true,
                _ => false,
            }
        }
        *self.shape == *other.shape
            && same(&self.pressed_shape, &other.pressed_shape)
            && same(&self.checked_shape, &other.checked_shape)
    }
}

/// Compose's `hashCode`: the shapes given, each as its `Debug` says (shapes of the same corners alike).
impl Hash for SplitButtonShapes {
    fn hash<H: Hasher>(&self, state: &mut H) {
        struct Writer<'h, H: Hasher>(&'h mut H);
        impl<H: Hasher> fmt::Write for Writer<'_, H> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                self.0.write(s.as_bytes());
                Ok(())
            }
        }
        for shape in [Some(&self.shape), self.pressed_shape.as_ref(), self.checked_shape.as_ref()].into_iter().flatten()
        {
            let _ = fmt::write(&mut Writer(state), format_args!("{shape:?}"));
        }
    }
}

impl fmt::Debug for SplitButtonShapes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SplitButtonShapes")
            .field("shape", &self.shape)
            .field("pressed_shape", &self.pressed_shape)
            .field("checked_shape", &self.checked_shape)
            .finish()
    }
}

// ---- Shape morphing --------------------------------------------------------------------------------------------

/// The shape of a split button's half now (Compose's private `shapeByInteraction`): the pressed one while
/// `pressed`, else the checked one while `checked` (its shape where it has none), else its shape — morphing corner
/// by corner along `animation_spec` where they are corner based (rounded ones as rounded), else switched. (No
/// `key(shapes)` here, as Compose's: new shapes animate from the shape it is at.)
#[kompozable]
fn shape_by_interaction(
    shapes: SplitButtonShapes,
    pressed: bool,
    checked: bool,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
) -> Arc<dyn Shape> {
    let shape = if pressed {
        shapes.pressed_shape.clone().unwrap_or_else(|| shapes.shape.clone())
    } else if checked {
        shapes.checked_shape.clone().unwrap_or_else(|| shapes.shape.clone())
    } else {
        shapes.shape.clone()
    };

    if shapes.has_rounded_corner_shapes() {
        let rounded = rounded_corner_shape_of(&*shape).expect("a rounded corner shape");
        remember_animated_shape(rounded, animation_spec)
    } else if shapes.has_corner_based_shapes() {
        let corner_based = corner_based_shape_of(&*shape).expect("a corner based shape");
        remember_animated_shape(corner_based, animation_spec)
    } else {
        shape
    }
}

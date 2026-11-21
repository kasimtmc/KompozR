//! The menus' defaults, as Compose's MenuDefaults.kt ([`MenuDefaults`]): their elevations, icon sizes, paddings,
//! the shapes and colors of menus, groups and items (the Expressive menu's by where a group or item is), the
//! labels of groups and items; and the schemes' and shapes' default item colors and shapes the components take.
//!
//! Compose's composable properties (`MenuDefaults.shape`, …) are associated functions; its non-composable
//! properties made of values the precision pointer flag picks between (`TrailingIconSize`, the paddings) are
//! too, read as asked (Kotlin reads them once, as the object is made: on the desktop the flag has no effect).
//! Its overloads with and without parameters (`itemColors()`, `itemShapes()`, `groupShapes()`) are one method
//! each, what isn't given unspecified (`None`) — the theme's.

// (`LabelWithSupportingText`'s own generated items name it)
#![allow(deprecated)]

use crate::color_scheme::ColorScheme;
use crate::material_theme::MaterialTheme;
use crate::menu::{
    DropdownMenuGroupDefaultMinHeight, DropdownMenuGroupVerticalPadding, DropdownMenuItemDefaultMaxWidth,
    DropdownMenuItemDefaultMinWidth, DropdownMenuItemHorizontalPadding, MenuGroupShapes, MenuItemColors,
    MenuItemShapes,
};
use crate::precision_pointer::should_use_precision_pointer_component_sizing_state;
use crate::shapes::Shapes;
use crate::text::ProvideTextStyle;
use crate::tokens::{
    ElevationTokens, ListTokens, MenuTokens, SegmentedMenuTokens, ShapeTokens, StandardMenuTokens, VibrantMenuTokens,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, RoundedCornerShape, Shape};
use kompoz_runtime::{kompozable_block, named, named_args};
use kompoz_ui::{Alignment, Modifier};
use std::rc::Rc;
use std::sync::Arc;

/// The default values of [`DropdownMenu`](crate::DropdownMenu) and its items (Compose's `object MenuDefaults`).
pub struct MenuDefaults;

#[named]
impl MenuDefaults {
    /// The default tonal elevation for a menu (Compose's `TonalElevation`): none.
    pub const TonalElevation: Dp = ElevationTokens::Level0;

    /// The default shadow elevation for a menu (Compose's `ShadowElevation`): 3 dp.
    pub const ShadowElevation: Dp = MenuTokens::ContainerElevation;

    /// The default leading icon size for a menu item (Compose's `LeadingIconSize`): 20 dp.
    pub const LeadingIconSize: Dp = SegmentedMenuTokens::ItemLeadingIconSize;

    /// The default trailing icon size for a menu item (Compose's `TrailingIconSize`): 20 dp, 24 for a precision
    /// pointer.
    pub fn TrailingIconSize() -> Dp {
        if should_use_precision_pointer_component_sizing_state().value() {
            Dp(24.0)
        } else {
            SegmentedMenuTokens::ItemTrailingIconSize
        }
    }

    /// The default shape for a menu (Compose's composable `shape`): the theme's extra small one.
    pub fn shape() -> Arc<dyn Shape> {
        MenuTokens::ContainerShape.value()
    }

    /// The default container color for a menu (Compose's composable `containerColor`): the scheme's surface
    /// container.
    pub fn container_color() -> Color {
        MenuTokens::ContainerColor.value()
    }

    /// The standard default container color for a group within a menu (Compose's composable
    /// `groupStandardContainerColor`): surface based, for lower emphasis (the scheme's surface container low).
    pub fn group_standard_container_color() -> Color {
        StandardMenuTokens::ContainerColor.value()
    }

    /// The vibrant default container color for a group within a menu (Compose's composable
    /// `groupVibrantContainerColor`): tertiary based, for higher emphasis — to be used sparingly.
    pub fn group_vibrant_container_color() -> Color {
        VibrantMenuTokens::ContainerColor.value()
    }

    /// The default shape for the leading group of a menu (Compose's composable `leadingGroupShape`): 16 dp corners
    /// at the top, 8 at the bottom.
    pub fn leading_group_shape() -> Arc<dyn Shape> {
        Arc::new(leading_group_shape_value())
    }

    /// The default shape for the middle group of a menu (Compose's composable `middleGroupShape`): the theme's
    /// small one.
    pub fn middle_group_shape() -> Arc<dyn Shape> {
        SegmentedMenuTokens::GroupShape.value()
    }

    /// The default shape for the trailing group of a menu (Compose's composable `trailingGroupShape`): 8 dp corners
    /// at the top, 16 at the bottom.
    pub fn trailing_group_shape() -> Arc<dyn Shape> {
        Arc::new(trailing_group_shape_value())
    }

    /// The default shape for the leading item of a menu or group (Compose's composable `leadingItemShape`): 12 dp
    /// corners at the top, 4 at the bottom.
    pub fn leading_item_shape() -> Arc<dyn Shape> {
        Arc::new(leading_item_shape_value())
    }

    /// The default shape for the middle item of a menu or group (Compose's composable `middleItemShape`): the
    /// theme's extra small one.
    pub fn middle_item_shape() -> Arc<dyn Shape> {
        SegmentedMenuTokens::ItemShape.value()
    }

    /// The default shape for the trailing item of a menu or group (Compose's composable `trailingItemShape`): 4 dp
    /// corners at the top, 12 at the bottom.
    pub fn trailing_item_shape() -> Arc<dyn Shape> {
        Arc::new(trailing_item_shape_value())
    }

    /// The default shape for a standalone item of a menu or group (Compose's composable `standaloneItemShape`):
    /// the middle item's.
    pub fn standalone_item_shape() -> Arc<dyn Shape> {
        MenuDefaults::middle_item_shape()
    }

    /// The selected shape for items of a group (Compose's composable `selectedItemShape`): the theme's medium one.
    pub fn selected_item_shape() -> Arc<dyn Shape> {
        SegmentedMenuTokens::ItemSelectedShape.value()
    }

    /// The default shape for a standalone group of a menu (Compose's composable `standaloneGroupShape`): the
    /// theme's large one.
    pub fn standalone_group_shape() -> Arc<dyn Shape> {
        SegmentedMenuTokens::ContainerShape.value()
    }

    /// The shape for a group of a menu that is no longer being hovered (Compose's composable
    /// `inactiveGroupShape`): the theme's small one.
    pub fn inactive_group_shape() -> Arc<dyn Shape> {
        SegmentedMenuTokens::InactiveContainerShape.value()
    }

    /// The default spacing between each menu group, usually a [`Spacer`]'s height (Compose's `GroupSpacing`):
    /// 2 dp.
    pub const GroupSpacing: Dp = SegmentedMenuTokens::SegmentedGap;

    /// The default padding of a [`HorizontalDivider`](crate::HorizontalDivider) in a menu group (Compose's
    /// `HorizontalDividerPadding`): 12 dp across, 2 down.
    pub fn HorizontalDividerPadding() -> PaddingValues {
        PaddingValues::hv(Dp(12.0), Dp(2.0))
    }

    /// The default padding of a menu group's [`Label`](Self::Label) (Compose's
    /// `DropdownMenuGroupLabelHorizontalPadding`): 12 dp at the start, 4 at the end.
    pub fn DropdownMenuGroupLabelHorizontalPadding() -> PaddingValues {
        named_args!(PaddingValues(start = Dp(12.0), end = Dp(4.0)))
    }

    /// The shapes the group at `index` of `count` groups in a menu takes (Compose's composable `groupShape`): a
    /// lone group the standalone ones, the first the leading ones, the last the trailing ones, the rest the
    /// middle ones.
    ///
    /// Kotlin: `groupShape(index: Int, count: Int): MenuGroupShapes`
    pub fn group_shape(index: i32, count: i32) -> MenuGroupShapes {
        let shapes = MaterialTheme::shapes();
        if count == 1 {
            return default_menu_standalone_group_shapes(&shapes);
        }

        if index == 0 {
            default_menu_leading_group_shapes(&shapes)
        } else if index == count - 1 {
            default_menu_trailing_group_shapes(&shapes)
        } else {
            default_menu_middle_group_shapes(&shapes)
        }
    }

    /// The shapes the item at `index` of `count` items in a menu or group takes (Compose's composable
    /// `itemShape`): of a group, count the group's items; without groups, the menu's.
    ///
    /// Kotlin: `itemShape(index: Int, count: Int): MenuItemShapes`
    pub fn item_shape(index: i32, count: i32) -> MenuItemShapes {
        let shapes = MaterialTheme::shapes();
        if count == 1 {
            return default_menu_standalone_item_shapes(&shapes);
        }

        if index == 0 {
            default_menu_leading_item_shapes(&shapes)
        } else if index == count - 1 {
            default_menu_trailing_item_shapes(&shapes)
        } else {
            default_menu_middle_item_shapes(&shapes)
        }
    }

    /// The text and icon colors of a standard item (Compose's composable `itemColors()` and `itemColors(…)`): what
    /// is given replacing the scheme's.
    ///
    /// Kotlin: `itemColors(textColor: Color = Color.Unspecified, leadingIconColor: Color = Color.Unspecified, trailingIconColor: Color = Color.Unspecified, disabledTextColor: Color = Color.Unspecified, disabledLeadingIconColor: Color = Color.Unspecified, disabledTrailingIconColor: Color = Color.Unspecified): MenuItemColors`
    pub fn item_colors(
        &self,
        #[default(Color::Unspecified)] text_color: Color,
        #[default(Color::Unspecified)] leading_icon_color: Color,
        #[default(Color::Unspecified)] trailing_icon_color: Color,
        #[default(Color::Unspecified)] disabled_text_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
    ) -> MenuItemColors {
        named_args!(default_menu_item_colors(&MaterialTheme::color_scheme()).copy(
            text_color = text_color,
            leading_icon_color = leading_icon_color,
            trailing_icon_color = trailing_icon_color,
            disabled_text_color = disabled_text_color,
            disabled_leading_icon_color = disabled_leading_icon_color,
            disabled_trailing_icon_color = disabled_trailing_icon_color,
        ))
    }

    /// The shapes of a checkable or selectable item (Compose's composable `itemShapes()` and `itemShapes(shape,
    /// selectedShape)`): `shape` (the standalone item shape if `None`) unselected, `selected_shape` (the selected
    /// item shape if `None`) selected.
    ///
    /// Kotlin: `itemShapes(shape: Shape? = null, selectedShape: Shape? = null): MenuItemShapes`
    pub fn item_shapes(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] selected_shape: Option<Arc<dyn Shape>>,
    ) -> MenuItemShapes {
        named_args!(
            default_menu_standalone_item_shapes(&MaterialTheme::shapes())
                .copy(shape = shape, selected_shape = selected_shape)
        )
    }

    /// The shapes of a [`DropdownMenuGroup`](crate::DropdownMenuGroup) (Compose's composable `groupShapes()` and
    /// `groupShapes(shape, inactiveShape)`): `shape` (the standalone group shape if `None`), `inactive_shape` (the
    /// inactive group shape if `None`) once no longer hovered.
    ///
    /// Kotlin: `groupShapes(shape: Shape? = null, inactiveShape: Shape? = null): MenuGroupShapes`
    pub fn group_shapes(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] inactive_shape: Option<Arc<dyn Shape>>,
    ) -> MenuGroupShapes {
        named_args!(
            default_menu_standalone_group_shapes(&MaterialTheme::shapes())
                .copy(shape = shape, inactive_shape = inactive_shape)
        )
    }

    /// The label of a [`DropdownMenuGroup`](crate::DropdownMenuGroup) or a part of it (Compose's composable
    /// `Label`): `content` in the label large style, aligned by `content_alignment` in `padding`, at least 112 dp
    /// wide (at most 280) and 32 dp tall.
    ///
    /// Kotlin: `Label(contentAlignment: Alignment = Alignment.CenterStart, padding: PaddingValues = DropdownMenuGroupLabelHorizontalPadding, content: @Composable () -> Unit)`
    #[track_caller]
    pub fn Label(
        &self,
        #[default(Alignment::CenterStart)] content_alignment: Alignment,
        #[default(MenuDefaults::DropdownMenuGroupLabelHorizontalPadding())] padding: PaddingValues,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            let content = Rc::new(content);
            ProvideTextStyle(MaterialTheme::typography().label_large, move || {
                let content = content.clone();
                Box(
                    modifier = Modifier
                        .size_in(
                            min_width = DropdownMenuItemDefaultMinWidth,
                            max_width = DropdownMenuItemDefaultMaxWidth,
                            min_height = DropdownMenuGroupDefaultMinHeight,
                        )
                        .padding_values(padding.clone()),
                    content_alignment = content_alignment.clone(),
                    content = move || content(),
                );
            });
        });
    }

    /// The default padding of an item's [`DropdownMenuItemTrailingLabel`](Self::DropdownMenuItemTrailingLabel)
    /// (Compose's `DropdownMenuItemTrailingLabelHorizontalPadding`): none, 6 dp at the end for a precision pointer.
    pub fn DropdownMenuItemTrailingLabelHorizontalPadding() -> PaddingValues {
        if should_use_precision_pointer_component_sizing_state().value() {
            named_args!(PaddingValues(start = Dp(0.0), end = Dp(6.0)))
        } else {
            PaddingValues::all(Dp(0.0))
        }
    }

    /// A trailing label for an item's trailing icon slot (Compose's composable `DropdownMenuItemTrailingLabel`):
    /// `content` in the label large style, in `padding`.
    ///
    /// Kotlin: `DropdownMenuItemTrailingLabel(padding: PaddingValues = DropdownMenuItemTrailingLabelHorizontalPadding, content: @Composable () -> Unit)`
    #[track_caller]
    pub fn DropdownMenuItemTrailingLabel(
        &self,
        #[default(MenuDefaults::DropdownMenuItemTrailingLabelHorizontalPadding())] padding: PaddingValues,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            let content = Rc::new(content);
            ProvideTextStyle(MaterialTheme::typography().label_large, move || {
                let content = content.clone();
                Box(modifier = Modifier.padding_values(padding.clone()), content = move || content());
            });
        });
    }

    /// A label over its supporting text, for an item's text (Compose's deprecated composable
    /// `LabelWithSupportingText`, replaced by the items' `supporting_text`): `content` in the label large style,
    /// `supporting_text` under it in the body medium one.
    ///
    /// Kotlin: `LabelWithSupportingText(supportingText: @Composable () -> Unit, content: @Composable () -> Unit)`
    #[deprecated(note = "Removed in favor of the DropdownMenuItem APIs that have supportingText as a parameter.")]
    #[track_caller]
    pub fn LabelWithSupportingText(&self, supporting_text: impl Fn() + 'static, content: impl Fn() + 'static) {
        kompozable_block!({
            let typography = MaterialTheme::typography();
            Column(
                content = move || {
                    ProvideTextStyle(typography.label_large.clone(), content);
                    ProvideTextStyle(typography.body_medium.clone(), supporting_text);
                },
            );
        });
    }

    /// The text, icon and container colors of a checkable or selectable item of the standard variant (Compose's
    /// composable `selectableItemColors(…)`): surface based, what is given replacing the scheme's.
    ///
    /// Kotlin: `selectableItemColors(textColor: Color = Color.Unspecified, containerColor: Color = Color.Unspecified, leadingIconColor: Color = Color.Unspecified, trailingIconColor: Color = Color.Unspecified, disabledTextColor: Color = Color.Unspecified, disabledLeadingIconColor: Color = Color.Unspecified, disabledTrailingIconColor: Color = Color.Unspecified, selectedContainerColor: Color = Color.Unspecified, selectedTextColor: Color = Color.Unspecified, selectedLeadingIconColor: Color = Color.Unspecified, selectedTrailingIconColor: Color = Color.Unspecified): MenuItemColors`
    #[allow(clippy::too_many_arguments)]
    pub fn selectable_item_colors(
        &self,
        #[default(Color::Unspecified)] text_color: Color,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] leading_icon_color: Color,
        #[default(Color::Unspecified)] trailing_icon_color: Color,
        #[default(Color::Unspecified)] disabled_text_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
        #[default(Color::Unspecified)] selected_container_color: Color,
        #[default(Color::Unspecified)] selected_text_color: Color,
        #[default(Color::Unspecified)] selected_leading_icon_color: Color,
        #[default(Color::Unspecified)] selected_trailing_icon_color: Color,
    ) -> MenuItemColors {
        named_args!(default_menu_selectable_item_colors(&MaterialTheme::color_scheme()).copy(
            text_color = text_color,
            container_color = container_color,
            leading_icon_color = leading_icon_color,
            trailing_icon_color = trailing_icon_color,
            disabled_text_color = disabled_text_color,
            disabled_leading_icon_color = disabled_leading_icon_color,
            disabled_trailing_icon_color = disabled_trailing_icon_color,
            selected_container_color = selected_container_color,
            selected_text_color = selected_text_color,
            selected_leading_icon_color = selected_leading_icon_color,
            selected_trailing_icon_color = selected_trailing_icon_color,
        ))
    }

    /// The same of the vibrant variant (Compose's composable `selectableItemVibrantColors(…)`): tertiary based.
    ///
    /// Kotlin: `selectableItemVibrantColors(textColor: Color = Color.Unspecified, containerColor: Color = Color.Unspecified, leadingIconColor: Color = Color.Unspecified, trailingIconColor: Color = Color.Unspecified, disabledTextColor: Color = Color.Unspecified, disabledLeadingIconColor: Color = Color.Unspecified, disabledTrailingIconColor: Color = Color.Unspecified, selectedContainerColor: Color = Color.Unspecified, selectedTextColor: Color = Color.Unspecified, selectedLeadingIconColor: Color = Color.Unspecified, selectedTrailingIconColor: Color = Color.Unspecified): MenuItemColors`
    #[allow(clippy::too_many_arguments)]
    pub fn selectable_item_vibrant_colors(
        &self,
        #[default(Color::Unspecified)] text_color: Color,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] leading_icon_color: Color,
        #[default(Color::Unspecified)] trailing_icon_color: Color,
        #[default(Color::Unspecified)] disabled_text_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
        #[default(Color::Unspecified)] selected_container_color: Color,
        #[default(Color::Unspecified)] selected_text_color: Color,
        #[default(Color::Unspecified)] selected_leading_icon_color: Color,
        #[default(Color::Unspecified)] selected_trailing_icon_color: Color,
    ) -> MenuItemColors {
        named_args!(default_menu_selectable_item_vibrant_colors(&MaterialTheme::color_scheme()).copy(
            text_color = text_color,
            container_color = container_color,
            leading_icon_color = leading_icon_color,
            trailing_icon_color = trailing_icon_color,
            disabled_text_color = disabled_text_color,
            disabled_leading_icon_color = disabled_leading_icon_color,
            disabled_trailing_icon_color = disabled_trailing_icon_color,
            selected_container_color = selected_container_color,
            selected_text_color = selected_text_color,
            selected_leading_icon_color = selected_leading_icon_color,
            selected_trailing_icon_color = selected_trailing_icon_color,
        ))
    }

    /// The default padding of a standard item (Compose's `DropdownMenuItemContentPadding`): 12 dp across.
    pub fn DropdownMenuItemContentPadding() -> PaddingValues {
        PaddingValues::hv(DropdownMenuItemHorizontalPadding, Dp(0.0))
    }

    /// The default padding of a checkable or selectable item (Compose's
    /// `DropdownMenuSelectableItemContentPadding`): 12 dp all round; 16 at the start, 10 at the end for a
    /// precision pointer.
    pub fn DropdownMenuSelectableItemContentPadding() -> PaddingValues {
        if should_use_precision_pointer_component_sizing_state().value() {
            PaddingValues::new(Dp(16.0), SelectableItemVerticalPadding, Dp(10.0), SelectableItemVerticalPadding)
        } else {
            PaddingValues::hv(DropdownMenuItemHorizontalPadding, SelectableItemVerticalPadding)
        }
    }

    /// The default padding of a [`DropdownMenuGroup`](crate::DropdownMenuGroup)'s content (Compose's
    /// `DropdownMenuGroupContentPadding`): 2 dp above and below.
    pub fn DropdownMenuGroupContentPadding() -> PaddingValues {
        PaddingValues::hv(Dp(0.0), DropdownMenuGroupVerticalPadding)
    }
}

const SelectableItemVerticalPadding: Dp = Dp(12.0);

/// The color with `alpha` (Kotlin's `copy(alpha = …)`).
fn with_alpha(color: Color, alpha: f32) -> Color {
    named_args!(color.copy(alpha = alpha))
}

/// A standard item's colors in `scheme` (Compose's internal `ColorScheme.defaultMenuItemColors`; not cached on
/// the scheme here: made as asked).
pub(crate) fn default_menu_item_colors(scheme: &ColorScheme) -> MenuItemColors {
    MenuItemColors {
        text_color: scheme.from_token(ListTokens::ItemLabelTextColor),
        leading_icon_color: scheme.from_token(ListTokens::ItemLeadingIconColor),
        trailing_icon_color: scheme.from_token(ListTokens::ItemTrailingIconColor),
        disabled_text_color: with_alpha(
            scheme.from_token(ListTokens::ItemDisabledLabelTextColor),
            ListTokens::ItemDisabledLabelTextOpacity,
        ),
        disabled_leading_icon_color: with_alpha(
            scheme.from_token(ListTokens::ItemDisabledLeadingIconColor),
            ListTokens::ItemDisabledLeadingIconOpacity,
        ),
        disabled_trailing_icon_color: with_alpha(
            scheme.from_token(ListTokens::ItemDisabledTrailingIconColor),
            ListTokens::ItemDisabledTrailingIconOpacity,
        ),
        container_color: Color::Unspecified,
        disabled_container_color: Color::Unspecified,
        selected_text_color: Color::Unspecified,
        selected_leading_icon_color: Color::Unspecified,
        selected_trailing_icon_color: Color::Unspecified,
        selected_container_color: Color::Unspecified,
    }
}

/// A selectable item's colors of the standard variant in `scheme` (Compose's internal
/// `ColorScheme.defaultMenuSelectableItemColors`).
pub(crate) fn default_menu_selectable_item_colors(scheme: &ColorScheme) -> MenuItemColors {
    MenuItemColors {
        text_color: scheme.from_token(StandardMenuTokens::ItemLabelTextColor),
        container_color: scheme.from_token(StandardMenuTokens::ContainerColor),
        leading_icon_color: scheme.from_token(StandardMenuTokens::ItemLeadingIconColor),
        trailing_icon_color: scheme.from_token(StandardMenuTokens::ItemTrailingIconColor),
        disabled_text_color: with_alpha(
            scheme.from_token(StandardMenuTokens::ItemDisabledLabelTextColor),
            StandardMenuTokens::ItemDisabledLabelTextOpacity,
        ),
        disabled_leading_icon_color: with_alpha(
            scheme.from_token(StandardMenuTokens::ItemDisabledLeadingIconColor),
            StandardMenuTokens::ItemDisabledLeadingIconOpacity,
        ),
        disabled_trailing_icon_color: with_alpha(
            scheme.from_token(StandardMenuTokens::ItemDisabledTrailingIconColor),
            StandardMenuTokens::ItemDisabledTrailingIconOpacity,
        ),
        disabled_container_color: scheme.from_token(StandardMenuTokens::ContainerColor),
        selected_text_color: scheme.from_token(StandardMenuTokens::ItemSelectedLabelTextColor),
        selected_container_color: scheme.from_token(StandardMenuTokens::ItemSelectedContainerColor),
        selected_leading_icon_color: scheme.from_token(StandardMenuTokens::ItemSelectedLeadingIconColor),
        selected_trailing_icon_color: scheme.from_token(StandardMenuTokens::ItemSelectedTrailingIconColor),
    }
}

/// A selectable item's colors of the vibrant variant in `scheme` (Compose's internal
/// `ColorScheme.defaultMenuSelectableItemVibrantColors`).
pub(crate) fn default_menu_selectable_item_vibrant_colors(scheme: &ColorScheme) -> MenuItemColors {
    MenuItemColors {
        text_color: scheme.from_token(VibrantMenuTokens::ItemLabelTextColor),
        container_color: scheme.from_token(VibrantMenuTokens::ContainerColor),
        leading_icon_color: scheme.from_token(VibrantMenuTokens::ItemLeadingIconColor),
        trailing_icon_color: scheme.from_token(VibrantMenuTokens::ItemTrailingIconColor),
        disabled_text_color: with_alpha(
            scheme.from_token(VibrantMenuTokens::ItemDisabledLabelTextColor),
            VibrantMenuTokens::ItemDisabledLabelTextOpacity,
        ),
        disabled_leading_icon_color: with_alpha(
            scheme.from_token(VibrantMenuTokens::ItemDisabledLeadingIconColor),
            VibrantMenuTokens::ItemDisabledLeadingIconOpacity,
        ),
        disabled_trailing_icon_color: with_alpha(
            scheme.from_token(VibrantMenuTokens::ItemDisabledTrailingIconColor),
            VibrantMenuTokens::ItemDisabledTrailingIconOpacity,
        ),
        disabled_container_color: scheme.from_token(VibrantMenuTokens::ContainerColor),
        selected_text_color: scheme.from_token(VibrantMenuTokens::ItemSelectedLabelTextColor),
        selected_container_color: scheme.from_token(VibrantMenuTokens::ItemSelectedContainerColor),
        selected_leading_icon_color: scheme.from_token(VibrantMenuTokens::ItemSelectedLeadingIconColor),
        selected_trailing_icon_color: scheme.from_token(VibrantMenuTokens::ItemSelectedTrailingIconColor),
    }
}

/// Compose's `RoundedCornerShape(topStart = Large, topEnd = Large, bottomStart = Small, bottomEnd = Small)`.
fn leading_group_shape_value() -> RoundedCornerShape {
    RoundedCornerShape::new(
        ShapeTokens::CornerValueLarge,
        ShapeTokens::CornerValueLarge,
        ShapeTokens::CornerValueSmall,
        ShapeTokens::CornerValueSmall,
    )
}

/// Compose's `RoundedCornerShape(topStart = Small, topEnd = Small, bottomStart = Large, bottomEnd = Large)`.
fn trailing_group_shape_value() -> RoundedCornerShape {
    RoundedCornerShape::new(
        ShapeTokens::CornerValueSmall,
        ShapeTokens::CornerValueSmall,
        ShapeTokens::CornerValueLarge,
        ShapeTokens::CornerValueLarge,
    )
}

/// Compose's `RoundedCornerShape(topStart = Medium, topEnd = Medium, bottomStart = ExtraSmall, bottomEnd =
/// ExtraSmall)`.
fn leading_item_shape_value() -> RoundedCornerShape {
    RoundedCornerShape::new(
        ShapeTokens::CornerValueMedium,
        ShapeTokens::CornerValueMedium,
        ShapeTokens::CornerValueExtraSmall,
        ShapeTokens::CornerValueExtraSmall,
    )
}

/// Compose's `RoundedCornerShape(topStart = ExtraSmall, topEnd = ExtraSmall, bottomStart = Medium, bottomEnd =
/// Medium)`.
fn trailing_item_shape_value() -> RoundedCornerShape {
    RoundedCornerShape::new(
        ShapeTokens::CornerValueExtraSmall,
        ShapeTokens::CornerValueExtraSmall,
        ShapeTokens::CornerValueMedium,
        ShapeTokens::CornerValueMedium,
    )
}

/// A standalone item's shapes in `shapes` (Compose's internal `Shapes.defaultMenuStandaloneItemShapes`; not cached
/// on the shapes here).
pub(crate) fn default_menu_standalone_item_shapes(shapes: &Shapes) -> MenuItemShapes {
    MenuItemShapes {
        shape: shapes.from_token(SegmentedMenuTokens::ItemShape),
        selected_shape: shapes.from_token(SegmentedMenuTokens::ItemSelectedShape),
    }
}

/// A leading item's shapes (Compose's internal `Shapes.defaultMenuLeadingItemShapes`).
pub(crate) fn default_menu_leading_item_shapes(shapes: &Shapes) -> MenuItemShapes {
    MenuItemShapes {
        shape: Arc::new(leading_item_shape_value()),
        selected_shape: shapes.from_token(SegmentedMenuTokens::ItemSelectedShape),
    }
}

/// A middle item's shapes (Compose's internal `Shapes.defaultMenuMiddleItemShapes`).
pub(crate) fn default_menu_middle_item_shapes(shapes: &Shapes) -> MenuItemShapes {
    MenuItemShapes {
        shape: shapes.from_token(SegmentedMenuTokens::ItemShape),
        selected_shape: shapes.from_token(SegmentedMenuTokens::ItemSelectedShape),
    }
}

/// A trailing item's shapes (Compose's internal `Shapes.defaultMenuTrailingItemShapes`).
pub(crate) fn default_menu_trailing_item_shapes(shapes: &Shapes) -> MenuItemShapes {
    MenuItemShapes {
        shape: Arc::new(trailing_item_shape_value()),
        selected_shape: shapes.from_token(SegmentedMenuTokens::ItemSelectedShape),
    }
}

/// A standalone group's shapes (Compose's internal `Shapes.defaultMenuStandaloneGroupShapes`).
pub(crate) fn default_menu_standalone_group_shapes(shapes: &Shapes) -> MenuGroupShapes {
    MenuGroupShapes {
        shape: shapes.from_token(SegmentedMenuTokens::ContainerShape),
        inactive_shape: shapes.from_token(SegmentedMenuTokens::InactiveContainerShape),
    }
}

/// A leading group's shapes (Compose's internal `Shapes.defaultMenuLeadingGroupShapes`).
pub(crate) fn default_menu_leading_group_shapes(shapes: &Shapes) -> MenuGroupShapes {
    MenuGroupShapes {
        shape: Arc::new(leading_group_shape_value()),
        inactive_shape: shapes.from_token(SegmentedMenuTokens::InactiveContainerShape),
    }
}

/// A middle group's shapes (Compose's internal `Shapes.defaultMenuMiddleGroupShapes`).
pub(crate) fn default_menu_middle_group_shapes(shapes: &Shapes) -> MenuGroupShapes {
    MenuGroupShapes {
        shape: shapes.from_token(SegmentedMenuTokens::GroupShape),
        inactive_shape: shapes.from_token(SegmentedMenuTokens::InactiveContainerShape),
    }
}

/// A trailing group's shapes (Compose's internal `Shapes.defaultMenuTrailingGroupShapes`).
pub(crate) fn default_menu_trailing_group_shapes(shapes: &Shapes) -> MenuGroupShapes {
    MenuGroupShapes {
        shape: Arc::new(trailing_group_shape_value()),
        inactive_shape: shapes.from_token(SegmentedMenuTokens::InactiveContainerShape),
    }
}

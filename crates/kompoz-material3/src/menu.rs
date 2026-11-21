//! Material's menus, as Compose's Menu.kt (its common code; Compose for desktop's actuals are in
//! `menu_desktop.rs`): the Expressive menu's groups ([`DropdownMenuGroup`]) and items — one taking a shape
//! ([`ShapedDropdownMenuItem`]), one checked like a checkbox ([`CheckableDropdownMenuItem`]), one selected like a
//! radio button ([`SelectableDropdownMenuItem`]), each with a supporting text if given —, the colors and shapes of
//! items and groups ([`MenuItemColors`], [`MenuItemShapes`], [`MenuGroupShapes`]), and what the menus share: the
//! menu's animated surface, the items' content and its layout, where the menu scales from.
//!
//! Compose's `DropdownMenuItem` overloads by their first parameters are composables of their own here (as
//! [`ClickableSurface`](crate::ClickableSurface) and its siblings are): `DropdownMenuItem(text, onClick, …)` is
//! [`DropdownMenuItem`](crate::DropdownMenuItem), `DropdownMenuItem(onClick, text, shape, …)`
//! [`ShapedDropdownMenuItem`], `DropdownMenuItem(checked, onCheckedChange, …)` [`CheckableDropdownMenuItem`] and
//! `DropdownMenuItem(selected, onClick, …)` [`SelectableDropdownMenuItem`]. The overloads Compose keeps hidden
//! for binary compatibility (no supporting text) can't be called from Kotlin and aren't ported.

use crate::content_color::LocalContentColor;
use crate::internal::animated_shape::remember_animated_shape;
use crate::material_theme::MaterialTheme;
use crate::menu_defaults::MenuDefaults;
use crate::precision_pointer::should_use_precision_pointer_component_sizing_state;
use crate::ripples::ripple;
use crate::surface::{SelectableSurface, Surface};
use crate::text::ProvideTextStyle;
use crate::text_field::find_first;
use crate::tokens::{ListTokens, MotionSchemeKeyTokens, SegmentedMenuTokens};
use kompoz_animation::{
    AnimatedVisibility, AnimatedVisibilityScope, FiniteAnimationSpec, MutableTransitionState, animate_color_as_state,
    expand_horizontally, fade_in, fade_out, shrink_horizontally, update_transition_of_state,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{
    AbsoluteCutCornerShape, AbsoluteRoundedCornerShape, BorderStroke, Color, CornerBasedShape, CutCornerShape, Dp,
    IntRect, IntSize, RoundedCornerShape, Shape,
};
use kompoz_runtime::{
    KompozitionLocalProvider, KotlinProperties, MutableState, key, kompozable, mutable_state_of, named, named_args,
    remember, remember_keyed,
};
use kompoz_ui::layout::{
    GraphicsLayerScope, Measurable, MeasurePolicy, MeasureResult, MeasureScope, TransformOrigin, layout_into,
};
use kompoz_ui::platform::LocalInspectionMode;
use kompoz_ui::semantics::Role;
use kompoz_ui::window::PopupProperties;
use kompoz_ui::{Alignment, Constraints, Modifier};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

/// A group of a [`DropdownMenuPopup`](crate::DropdownMenuPopup)'s items set apart (Compose's Expressive
/// `DropdownMenuGroup`): `content` in a column padded by `content_padding` on a surface of `container_color` in
/// its `shapes`' shape — morphing to their inactive shape once the group has been hovered and isn't any more,
/// along the theme's fast spatial spec —, with its tonal and shadow elevation and border. `modifier` is the
/// column's; its hovers go to `interaction_source` (one of its own where none).
///
/// Kotlin: `DropdownMenuGroup(shapes: MenuGroupShapes, modifier: Modifier = Modifier, containerColor: Color = MenuDefaults.groupStandardContainerColor, tonalElevation: Dp = MenuDefaults.TonalElevation, shadowElevation: Dp = MenuDefaults.ShadowElevation, border: BorderStroke? = null, contentPadding: PaddingValues = MenuDefaults.DropdownMenuGroupContentPadding, interactionSource: MutableInteractionSource? = null, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn DropdownMenuGroup(
    shapes: MenuGroupShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(MenuDefaults::group_standard_container_color())] container_color: Color,
    #[default(MenuDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(MenuDefaults::ShadowElevation)] shadow_elevation: Dp,
    #[default(None)] border: Option<BorderStroke>,
    #[default(MenuDefaults::DropdownMenuGroupContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let hovered = interaction_source.as_interaction_source().collect_is_hovered_as_state().get();
    let has_been_hovered = remember(|| mutable_state_of(false));
    if hovered {
        has_been_hovered.set(true);
    }

    let morph_spec = MotionSchemeKeyTokens::FastSpatial.value::<f32>();
    let group_shape = group_shape_by_interaction(shapes, has_been_hovered.get(), hovered, morph_spec);

    Surface(
        modifier = Modifier.hoverable(interaction_source),
        shape = group_shape,
        color = container_color,
        tonal_elevation = tonal_elevation,
        shadow_elevation = shadow_elevation,
        border = border,
        content = move || {
            Column(modifier = modifier.padding_values(content_padding), content = content);
        },
    );
}

/// A menu item of a shape of its own (Compose's Expressive `DropdownMenuItem(onClick, text, shape, …)`): `text` (and
/// `supporting_text` under it) between the icons, in `colors` for its enabled state, a button to accessibility;
/// `shape` is [`MenuDefaults::leading_item_shape`]'s, [`MenuDefaults::middle_item_shape`]'s or
/// [`MenuDefaults::trailing_item_shape`]'s by where it is. `on_click` runs on a click unless not `enabled`; its
/// interactions go to `interaction_source` (one of its own where none).
///
/// Kotlin: `DropdownMenuItem(onClick: () -> Unit, text: @Composable () -> Unit, shape: Shape, modifier: Modifier = Modifier, leadingIcon: @Composable (() -> Unit)? = null, trailingIcon: @Composable (() -> Unit)? = null, enabled: Boolean = true, colors: MenuItemColors = MenuDefaults.itemColors(), contentPadding: PaddingValues = MenuDefaults.DropdownMenuSelectableItemContentPadding, interactionSource: MutableInteractionSource? = null, supportingText: @Composable (() -> Unit)? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ShapedDropdownMenuItem(
    on_click: impl Fn() + 'static,
    text: impl Fn() + 'static,
    shape: Arc<dyn Shape>,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(true)] enabled: bool,
    #[default(MenuDefaults.item_colors())] colors: MenuItemColors,
    #[default(MenuDefaults::DropdownMenuSelectableItemContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(None)] supporting_text: Option<Rc<dyn Fn()>>,
) {
    let shapes = named_args!(MenuDefaults.item_shapes(shape = Some(shape)));
    SelectableDropdownMenuItemContent(
        false,
        Rc::new(on_click),
        Rc::new(text),
        modifier.semantics(|c| c.set_role(Role::Button)),
        supporting_text,
        leading_icon,
        None,
        trailing_icon,
        enabled,
        colors,
        shapes,
        content_padding,
        interaction_source,
    );
}

/// A menu item that is checked or not, as a checkbox or switch in a menu (Compose's Expressive
/// `DropdownMenuItem(checked, onCheckedChange, …)`): a click calls `on_checked_change` with the other state; checked,
/// it takes its `shapes`' selected shape (morphing there along the theme's fast spatial spec), `colors`' selected
/// colors (its container's fading there) and `checked_leading_icon` (coming in across where it has no
/// `leading_icon`); a checkbox to accessibility. Otherwise as [`ShapedDropdownMenuItem`].
///
/// Kotlin: `DropdownMenuItem(checked: Boolean, onCheckedChange: (Boolean) -> Unit, text: @Composable () -> Unit, shapes: MenuItemShapes, modifier: Modifier = Modifier, leadingIcon: @Composable (() -> Unit)? = null, checkedLeadingIcon: @Composable (() -> Unit)? = null, trailingIcon: @Composable (() -> Unit)? = null, enabled: Boolean = true, colors: MenuItemColors = MenuDefaults.selectableItemColors(), contentPadding: PaddingValues = MenuDefaults.DropdownMenuSelectableItemContentPadding, interactionSource: MutableInteractionSource? = null, supportingText: @Composable (() -> Unit)? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn CheckableDropdownMenuItem(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    text: impl Fn() + 'static,
    shapes: MenuItemShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] checked_leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(true)] enabled: bool,
    #[default(MenuDefaults.selectable_item_colors())] colors: MenuItemColors,
    #[default(MenuDefaults::DropdownMenuSelectableItemContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(None)] supporting_text: Option<Rc<dyn Fn()>>,
) {
    SelectableDropdownMenuItemContent(
        checked,
        Rc::new(move || on_checked_change(!checked)),
        Rc::new(text),
        modifier.semantics(|c| c.set_role(Role::Checkbox)),
        supporting_text,
        leading_icon,
        checked_leading_icon,
        trailing_icon,
        enabled,
        colors,
        shapes,
        content_padding,
        interaction_source,
    );
}

/// A menu item that is selected or not, as a radio button in a menu (Compose's Expressive
/// `DropdownMenuItem(selected, onClick, …)`): `on_click` runs on a click; selected, it takes its `shapes`' selected
/// shape, `colors`' selected colors and `selected_leading_icon`; a radio button to accessibility. Otherwise as
/// [`CheckableDropdownMenuItem`].
///
/// Kotlin: `DropdownMenuItem(selected: Boolean, onClick: () -> Unit, text: @Composable () -> Unit, shapes: MenuItemShapes, modifier: Modifier = Modifier, leadingIcon: @Composable (() -> Unit)? = null, selectedLeadingIcon: @Composable (() -> Unit)? = null, trailingIcon: @Composable (() -> Unit)? = null, enabled: Boolean = true, colors: MenuItemColors = MenuDefaults.selectableItemColors(), contentPadding: PaddingValues = MenuDefaults.DropdownMenuSelectableItemContentPadding, interactionSource: MutableInteractionSource? = null, supportingText: @Composable (() -> Unit)? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SelectableDropdownMenuItem(
    selected: bool,
    on_click: impl Fn() + 'static,
    text: impl Fn() + 'static,
    shapes: MenuItemShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] selected_leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(true)] enabled: bool,
    #[default(MenuDefaults.selectable_item_colors())] colors: MenuItemColors,
    #[default(MenuDefaults::DropdownMenuSelectableItemContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(None)] supporting_text: Option<Rc<dyn Fn()>>,
) {
    SelectableDropdownMenuItemContent(
        selected,
        Rc::new(on_click),
        Rc::new(text),
        modifier.semantics(|c| c.set_role(Role::RadioButton)),
        supporting_text,
        leading_icon,
        selected_leading_icon,
        trailing_icon,
        enabled,
        colors,
        shapes,
        content_padding,
        interaction_source,
    );
}

/// The properties of the menus' popups (Compose's internal `DefaultMenuProperties`, Compose for desktop's actual):
/// focusable, dismissed by back and by a press outside, kept in the window.
pub(crate) const DefaultMenuProperties: PopupProperties = PopupProperties {
    focusable: true,
    dismiss_on_back_press: true,
    dismiss_on_click_outside: true,
    clipping_enabled: true,
    use_platform_default_width: false,
};

// ---- Colors and shapes ----------------------------------------------------------------------------------------

/// A menu item's text, icon and container colors in its states (Compose's `MenuItemColors`): enabled, disabled,
/// and enabled and selected (a checkable or selectable item's). Make them with [`MenuDefaults`]' `item_colors`,
/// `selectable_item_colors` and `selectable_item_vibrant_colors`, [`MenuItemColors()`](fn@MenuItemColors) or
/// [`copy`](MenuItemColors::copy).
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct MenuItemColors {
    /// The text color when enabled.
    pub text_color: Color,
    /// The leading icon color when enabled.
    pub leading_icon_color: Color,
    /// The trailing icon color when enabled.
    pub trailing_icon_color: Color,
    /// The text color when not enabled.
    pub disabled_text_color: Color,
    /// The leading icon color when not enabled.
    pub disabled_leading_icon_color: Color,
    /// The trailing icon color when not enabled.
    pub disabled_trailing_icon_color: Color,
    /// The container color when enabled and unselected.
    pub container_color: Color,
    /// The container color when not enabled.
    pub disabled_container_color: Color,
    /// The text color when enabled and selected.
    pub selected_text_color: Color,
    /// The leading icon color when enabled and selected.
    pub selected_leading_icon_color: Color,
    /// The trailing icon color when enabled and selected.
    pub selected_trailing_icon_color: Color,
    /// The container color when enabled and selected.
    pub selected_container_color: Color,
}

/// Colors of an item (Compose's `MenuItemColors(…)` constructors: the six text and icon colors of a standard item,
/// the container and selected colors too for a selectable one; what the six-color one leaves out is unspecified).
///
/// Kotlin: `MenuItemColors(textColor: Color, leadingIconColor: Color, trailingIconColor: Color, disabledTextColor: Color, disabledLeadingIconColor: Color, disabledTrailingIconColor: Color, containerColor: Color = Color.Unspecified, disabledContainerColor: Color = Color.Unspecified, selectedTextColor: Color = Color.Unspecified, selectedLeadingIconColor: Color = Color.Unspecified, selectedTrailingIconColor: Color = Color.Unspecified, selectedContainerColor: Color = Color.Unspecified)`
#[named(type)]
#[allow(clippy::too_many_arguments)]
pub fn MenuItemColors(
    text_color: Color,
    leading_icon_color: Color,
    trailing_icon_color: Color,
    disabled_text_color: Color,
    disabled_leading_icon_color: Color,
    disabled_trailing_icon_color: Color,
    #[default(Color::Unspecified)] container_color: Color,
    #[default(Color::Unspecified)] disabled_container_color: Color,
    #[default(Color::Unspecified)] selected_text_color: Color,
    #[default(Color::Unspecified)] selected_leading_icon_color: Color,
    #[default(Color::Unspecified)] selected_trailing_icon_color: Color,
    #[default(Color::Unspecified)] selected_container_color: Color,
) -> MenuItemColors {
    MenuItemColors {
        text_color,
        leading_icon_color,
        trailing_icon_color,
        disabled_text_color,
        disabled_leading_icon_color,
        disabled_trailing_icon_color,
        container_color,
        disabled_container_color,
        selected_text_color,
        selected_leading_icon_color,
        selected_trailing_icon_color,
        selected_container_color,
    }
}

/// Kotlin's `Color.takeOrElse`.
fn take_or_else(color: Color, other: Color) -> Color {
    if color.is_specified() { color } else { other }
}

#[named]
impl MenuItemColors {
    /// The same with what is given replaced, an unspecified color keeping its own (Compose's two `copy`s: the six
    /// text and icon colors first, as the standard one's, then the container and selected ones of the Expressive
    /// one; named, the same).
    ///
    /// Kotlin: `copy(textColor: Color = this.textColor, leadingIconColor: Color = this.leadingIconColor, trailingIconColor: Color = this.trailingIconColor, disabledTextColor: Color = this.disabledTextColor, disabledLeadingIconColor: Color = this.disabledLeadingIconColor, disabledTrailingIconColor: Color = this.disabledTrailingIconColor, containerColor: Color = this.containerColor, disabledContainerColor: Color = this.disabledContainerColor, selectedTextColor: Color = this.selectedTextColor, selectedContainerColor: Color = this.selectedContainerColor, selectedLeadingIconColor: Color = this.selectedLeadingIconColor, selectedTrailingIconColor: Color = this.selectedTrailingIconColor)`
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(Color::Unspecified)] text_color: Color,
        #[default(Color::Unspecified)] leading_icon_color: Color,
        #[default(Color::Unspecified)] trailing_icon_color: Color,
        #[default(Color::Unspecified)] disabled_text_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] selected_text_color: Color,
        #[default(Color::Unspecified)] selected_container_color: Color,
        #[default(Color::Unspecified)] selected_leading_icon_color: Color,
        #[default(Color::Unspecified)] selected_trailing_icon_color: Color,
    ) -> MenuItemColors {
        MenuItemColors {
            text_color: take_or_else(text_color, self.text_color),
            leading_icon_color: take_or_else(leading_icon_color, self.leading_icon_color),
            trailing_icon_color: take_or_else(trailing_icon_color, self.trailing_icon_color),
            disabled_text_color: take_or_else(disabled_text_color, self.disabled_text_color),
            disabled_leading_icon_color: take_or_else(disabled_leading_icon_color, self.disabled_leading_icon_color),
            disabled_trailing_icon_color: take_or_else(disabled_trailing_icon_color, self.disabled_trailing_icon_color),
            container_color: take_or_else(container_color, self.container_color),
            disabled_container_color: take_or_else(disabled_container_color, self.disabled_container_color),
            selected_text_color: take_or_else(selected_text_color, self.selected_text_color),
            selected_leading_icon_color: take_or_else(selected_leading_icon_color, self.selected_leading_icon_color),
            selected_trailing_icon_color: take_or_else(selected_trailing_icon_color, self.selected_trailing_icon_color),
            selected_container_color: take_or_else(selected_container_color, self.selected_container_color),
        }
    }
}

impl MenuItemColors {
    /// The text color for an item `enabled` and `selected` or not (Compose's internal `textColor`).
    pub(crate) fn text_color_of(&self, enabled: bool, selected: bool) -> Color {
        if enabled {
            if selected { self.selected_text_color } else { self.text_color }
        } else {
            self.disabled_text_color
        }
    }

    /// The leading icon color (Compose's internal `leadingIconColor`).
    pub(crate) fn leading_icon_color_of(&self, enabled: bool, selected: bool) -> Color {
        if enabled {
            if selected { self.selected_leading_icon_color } else { self.leading_icon_color }
        } else {
            self.disabled_leading_icon_color
        }
    }

    /// The trailing icon color (Compose's internal `trailingIconColor`).
    pub(crate) fn trailing_icon_color_of(&self, enabled: bool, selected: bool) -> Color {
        if enabled {
            if selected { self.selected_trailing_icon_color } else { self.trailing_icon_color }
        } else {
            self.disabled_trailing_icon_color
        }
    }

    /// The container color (Compose's internal `containerColor(enabled, selected)`).
    pub(crate) fn container_color_of(&self, enabled: bool, selected: bool) -> Color {
        if enabled {
            if selected { self.selected_container_color } else { self.container_color }
        } else {
            self.disabled_container_color
        }
    }
}

/// The shapes of a checkable or selectable item (Compose's `MenuItemShapes`): `shape` unselected,
/// `selected_shape` selected. Equal as their shapes are.
#[derive(Clone, Debug, KotlinProperties)]
pub struct MenuItemShapes {
    pub shape: Arc<dyn Shape>,
    pub selected_shape: Arc<dyn Shape>,
}

/// Shapes of an item (Compose's `MenuItemShapes(shape, selectedShape)`).
///
/// Kotlin: `MenuItemShapes(shape: Shape, selectedShape: Shape)`
pub fn MenuItemShapes(shape: Arc<dyn Shape>, selected_shape: Arc<dyn Shape>) -> MenuItemShapes {
    MenuItemShapes { shape, selected_shape }
}

impl PartialEq for MenuItemShapes {
    fn eq(&self, other: &Self) -> bool {
        *self.shape == *other.shape && *self.selected_shape == *other.selected_shape
    }
}

#[named]
impl MenuItemShapes {
    /// The same with what is given replaced, `None` keeping its own (Compose's `copy`).
    ///
    /// Kotlin: `copy(shape: Shape? = this.shape, selectedShape: Shape? = this.selectedShape)`
    pub fn copy(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] selected_shape: Option<Arc<dyn Shape>>,
    ) -> MenuItemShapes {
        MenuItemShapes {
            shape: shape.unwrap_or_else(|| self.shape.clone()),
            selected_shape: selected_shape.unwrap_or_else(|| self.selected_shape.clone()),
        }
    }
}

/// The shapes of a [`DropdownMenuGroup`] (Compose's `MenuGroupShapes`): `shape` by default, `inactive_shape` once
/// it has been hovered and isn't any more. Equal as their shapes are.
#[derive(Clone, Debug, KotlinProperties)]
pub struct MenuGroupShapes {
    pub shape: Arc<dyn Shape>,
    pub inactive_shape: Arc<dyn Shape>,
}

/// Shapes of a group (Compose's `MenuGroupShapes(shape, inactiveShape)`).
///
/// Kotlin: `MenuGroupShapes(shape: Shape, inactiveShape: Shape)`
pub fn MenuGroupShapes(shape: Arc<dyn Shape>, inactive_shape: Arc<dyn Shape>) -> MenuGroupShapes {
    MenuGroupShapes { shape, inactive_shape }
}

impl PartialEq for MenuGroupShapes {
    fn eq(&self, other: &Self) -> bool {
        *self.shape == *other.shape && *self.inactive_shape == *other.inactive_shape
    }
}

#[named]
impl MenuGroupShapes {
    /// The same with what is given replaced, `None` keeping its own (Compose's `copy`).
    ///
    /// Kotlin: `copy(shape: Shape? = this.shape, inactiveShape: Shape? = this.inactiveShape)`
    pub fn copy(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] inactive_shape: Option<Arc<dyn Shape>>,
    ) -> MenuGroupShapes {
        MenuGroupShapes {
            shape: shape.unwrap_or_else(|| self.shape.clone()),
            inactive_shape: inactive_shape.unwrap_or_else(|| self.inactive_shape.clone()),
        }
    }
}

/// `shape` as a rounded corner shape where it is one (Kotlin's `shape is RoundedCornerShape`: a theme's shape is
/// KompozR's [`CornerBasedShape`] enum, its rounded variant one too).
pub(crate) fn as_rounded_corner_shape(shape: &dyn Shape) -> Option<RoundedCornerShape> {
    let any = shape.shape_as_any();
    if let Some(rounded) = any.downcast_ref::<RoundedCornerShape>() {
        return Some(rounded.clone());
    }
    match any.downcast_ref::<CornerBasedShape>() {
        Some(CornerBasedShape::Rounded(rounded)) => Some(rounded.clone()),
        _ => None,
    }
}

/// `shape` as a corner based shape where it is one (Kotlin's `shape is CornerBasedShape`: any of the four corner
/// shapes, or KompozR's enum of them).
pub(crate) fn as_corner_based_shape(shape: &dyn Shape) -> Option<CornerBasedShape> {
    let any = shape.shape_as_any();
    if let Some(shape) = any.downcast_ref::<CornerBasedShape>() {
        return Some(shape.clone());
    }
    if let Some(shape) = any.downcast_ref::<RoundedCornerShape>() {
        return Some(shape.clone().into());
    }
    if let Some(shape) = any.downcast_ref::<CutCornerShape>() {
        return Some(shape.clone().into());
    }
    if let Some(shape) = any.downcast_ref::<AbsoluteRoundedCornerShape>() {
        return Some(shape.clone().into());
    }
    any.downcast_ref::<AbsoluteCutCornerShape>().map(|shape| shape.clone().into())
}

impl MenuItemShapes {
    /// Whether both are rounded corner shapes (Compose's internal `hasRoundedCornerShapes`).
    pub(crate) fn has_rounded_corner_shapes(&self) -> bool {
        as_rounded_corner_shape(&*self.shape).is_some() && as_rounded_corner_shape(&*self.selected_shape).is_some()
    }

    /// Whether both are corner based shapes (Compose's internal `hasCornerBasedShapes`).
    pub(crate) fn has_corner_based_shapes(&self) -> bool {
        as_corner_based_shape(&*self.shape).is_some() && as_corner_based_shape(&*self.selected_shape).is_some()
    }
}

impl MenuGroupShapes {
    /// Whether both are rounded corner shapes (Compose's internal `hasRoundedCornerShapes`).
    pub(crate) fn has_rounded_corner_shapes(&self) -> bool {
        as_rounded_corner_shape(&*self.shape).is_some() && as_rounded_corner_shape(&*self.inactive_shape).is_some()
    }

    /// Whether both are corner based shapes (Compose's internal `hasCornerBasedShapes`).
    pub(crate) fn has_corner_based_shapes(&self) -> bool {
        as_corner_based_shape(&*self.shape).is_some() && as_corner_based_shape(&*self.inactive_shape).is_some()
    }
}

// ---- The menu's content ---------------------------------------------------------------------------------------

/// A menu's surface (Compose's internal `DropdownMenuContent`): scaled from 0.8 and faded in as `expanded_state`
/// goes to expanded (and back as it goes the other way) along the theme's fast spatial and fast effects specs,
/// about `transform_origin_state`'s origin (at their targets in inspection mode); `content` in a column padded
/// 8 dp above and below, as wide as its widest item, scrolled by `scroll_state`, with `modifier`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn DropdownMenuContent(
    modifier: Modifier,
    expanded_state: MutableTransitionState<bool>,
    transform_origin_state: MutableState<TransformOrigin>,
    scroll_state: ScrollState,
    shape: Arc<dyn Shape>,
    container_color: Color,
    tonal_elevation: Dp,
    shadow_elevation: Dp,
    border: Option<BorderStroke>,
    content: impl Fn() + 'static,
) {
    // Menu open/close animation.
    let transition = update_transition_of_state(expanded_state.clone(), Some("DropDownMenu".to_string()));
    let scale_animation_spec = MotionSchemeKeyTokens::FastSpatial.value::<f32>();
    let alpha_animation_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>();
    let scale = transition.animate_float(
        move |_| scale_animation_spec.clone(),
        "FloatAnimation".to_string(),
        |expanded: &bool| if *expanded { ExpandedScaleTarget } else { ClosedScaleTarget },
    );
    let alpha = transition.animate_float(
        move |_| alpha_animation_spec.clone(),
        "FloatAnimation".to_string(),
        |expanded: &bool| if *expanded { ExpandedAlphaTarget } else { ClosedAlphaTarget },
    );

    let is_inspecting = LocalInspectionMode.current();
    let layer = menu_graphics_layer(is_inspecting, expanded_state, scale, alpha, transform_origin_state);
    Surface(
        modifier = Modifier.graphics_layer_block(layer),
        shape = shape,
        color = container_color,
        tonal_elevation = tonal_elevation,
        shadow_elevation = shadow_elevation,
        border = border,
        content = move || {
            Column(
                modifier = modifier
                    .padding_hv(vertical = DropdownMenuVerticalPadding)
                    .width_intrinsic(IntrinsicSize::Max)
                    .vertical_scroll(&scroll_state),
                content = content,
            );
        },
    );
}

/// The graphics layer block of a menu's open and close animation (Compose's `graphicsLayer { … }` of
/// `DropdownMenuContent` and `DropdownMenuPopup`): the scale and alpha (their targets while inspecting), the
/// transform origin.
pub(crate) fn menu_graphics_layer(
    is_inspecting: bool,
    expanded_state: MutableTransitionState<bool>,
    scale: kompoz_runtime::State<f32>,
    alpha: kompoz_runtime::State<f32>,
    transform_origin_state: MutableState<TransformOrigin>,
) -> impl Fn(&mut GraphicsLayerScope) + 'static {
    move |l: &mut GraphicsLayerScope| {
        let target = || expanded_state.target_state();
        l.scale_x = if !is_inspecting {
            scale.get()
        } else if target() {
            ExpandedScaleTarget
        } else {
            ClosedScaleTarget
        };
        l.scale_y = if !is_inspecting {
            scale.get()
        } else if target() {
            ExpandedScaleTarget
        } else {
            ClosedScaleTarget
        };
        l.alpha = if !is_inspecting {
            alpha.get()
        } else if target() {
            ExpandedAlphaTarget
        } else {
            ClosedAlphaTarget
        };
        l.transform_origin = transform_origin_state.get();
    }
}

/// The content of a checkable, selectable or shaped item (Compose's internal `DropdownMenuItemContent(selected, …)`,
/// the Expressive one): a selectable surface as wide as the menu, inset 4 dp across (2 dp down too with a
/// supporting text), in its shape for `selected` (morphing between `shapes`' two) and its container color of
/// `colors` (fading between them); in it, in the label large style, the leading icon (the selected one while
/// selected; coming in across and fading in where there is no unselected one), the text (and supporting text) and
/// the trailing icon, each in its color of `colors`, laid out by [`DropdownMenuItemMeasurePolicy`] in
/// `content_padding`, at least 112 dp wide (at most 280) and 44 dp tall.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn SelectableDropdownMenuItemContent(
    selected: bool,
    on_click: Rc<dyn Fn()>,
    text: Rc<dyn Fn()>,
    modifier: Modifier,
    supporting_text: Option<Rc<dyn Fn()>>,
    leading_icon: Option<Rc<dyn Fn()>>,
    selected_leading_icon: Option<Rc<dyn Fn()>>,
    trailing_icon: Option<Rc<dyn Fn()>>,
    enabled: bool,
    colors: MenuItemColors,
    shapes: MenuItemShapes,
    content_padding: PaddingValues,
    interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));

    let expand_and_shrink_spec = MotionSchemeKeyTokens::FastSpatial.value::<IntSize>();
    let fade_in_and_out_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>();
    let morph_spec = MotionSchemeKeyTokens::FastSpatial.value::<f32>();
    let color_animation_spec = MotionSchemeKeyTokens::FastEffects.value::<Color>();

    let container_color = colors.container_color_of(enabled, selected);
    let animated_container_color = animate_color_as_state(container_color, animation_spec = color_animation_spec);
    let item_shape = item_shape_by_interaction(shapes, selected, morph_spec);

    let has_leading_icon = leading_icon.is_some() || selected_leading_icon.is_some();
    let has_trailing_icon = trailing_icon.is_some();

    let padding = if supporting_text.is_some() {
        DropdownMenuSelectableItemWithSupportTexPadding()
    } else {
        DropdownMenuSelectableItemPadding()
    };
    let label_large = MaterialTheme::typography().label_large;
    SelectableSurface(
        selected,
        move || on_click(),
        modifier = modifier.fill_max_width().padding_values(padding),
        enabled = enabled,
        shape = item_shape,
        color = animated_container_color.get(),
        interaction_source = Some(interaction_source),
        content = move || {
            let (text, supporting_text) = (text.clone(), supporting_text.clone());
            let (leading_icon, selected_leading_icon, trailing_icon) =
                (leading_icon.clone(), selected_leading_icon.clone(), trailing_icon.clone());
            let (expand_and_shrink_spec, fade_in_and_out_spec) =
                (expand_and_shrink_spec.clone(), fade_in_and_out_spec.clone());
            ProvideTextStyle(label_large.clone(), move || {
                // (Compose makes a new policy each time, a new instance measuring the layout again; one kept
                // while its two flags stay here: the same layout)
                let measure_policy = remember_keyed((has_leading_icon, has_trailing_icon), || {
                    Rc::new(DropdownMenuItemMeasurePolicy { has_leading_icon, has_trailing_icon })
                        as Rc<dyn MeasurePolicy>
                });
                let modifier = named_args!(Modifier.size_in(
                    min_width = DropdownMenuItemDefaultMinWidth,
                    max_width = DropdownMenuItemDefaultMaxWidth,
                    min_height = SegmentedMenuTokens::Item,
                ))
                .padding_values(content_padding.clone());
                let (text, supporting_text) = (text.clone(), supporting_text.clone());
                let (leading_icon, selected_leading_icon, trailing_icon) =
                    (leading_icon.clone(), selected_leading_icon.clone(), trailing_icon.clone());
                let (expand_and_shrink_spec, fade_in_and_out_spec) =
                    (expand_and_shrink_spec.clone(), fade_in_and_out_spec.clone());
                layout_into(modifier, measure_policy, false, move || {
                    if has_leading_icon {
                        let (leading_icon, selected_leading_icon) =
                            (leading_icon.clone(), selected_leading_icon.clone());
                        let (expand_and_shrink_spec, fade_in_and_out_spec) =
                            (expand_and_shrink_spec.clone(), fade_in_and_out_spec.clone());
                        KompozitionLocalProvider(
                            &[LocalContentColor.provides(colors.leading_icon_color_of(enabled, selected))],
                            move || {
                                Box(
                                    modifier = Modifier.layout_id(LeadingIconLayoutId),
                                    content_alignment = Alignment::Center,
                                    content = move || {
                                        LeadingIcon(
                                            selected,
                                            leading_icon.clone(),
                                            selected_leading_icon.clone(),
                                            expand_and_shrink_spec.clone(),
                                            fade_in_and_out_spec.clone(),
                                        );
                                    },
                                );
                            },
                        );
                    }

                    let (text, supporting_text) = (text.clone(), supporting_text.clone());
                    KompozitionLocalProvider(
                        &[LocalContentColor.provides(colors.text_color_of(enabled, selected))],
                        move || {
                            let text_padding = if has_trailing_icon { DropdownMenuIconTextPadding() } else { Dp(0.0) };
                            Box(
                                modifier = Modifier.layout_id(TextLayoutId).padding_sides(end = text_padding),
                                content_alignment = Alignment::CenterStart,
                                content = move || {
                                    if let Some(supporting_text) = &supporting_text {
                                        LabelWithSupportingText(
                                            supporting_text.clone(),
                                            Modifier.layout_id(TextLayoutId),
                                            text.clone(),
                                        );
                                    } else {
                                        text();
                                    }
                                },
                            );
                        },
                    );

                    if let Some(trailing_icon) = trailing_icon.clone() {
                        KompozitionLocalProvider(
                            &[LocalContentColor.provides(colors.trailing_icon_color_of(enabled, selected))],
                            move || {
                                let trailing_icon = trailing_icon.clone();
                                Box(
                                    modifier = Modifier
                                        .layout_id(TrailingIconLayoutId)
                                        .default_min_size(min_width = SegmentedMenuTokens::ItemTrailingIconSize),
                                    content_alignment = Alignment::Center,
                                    content = move || trailing_icon(),
                                );
                            },
                        );
                    }

                    // for measurement for trailing icon if provided
                    if has_leading_icon {
                        let icon = leading_icon.clone().or_else(|| selected_leading_icon.clone());
                        Box(
                            modifier = Modifier.layout_id(GhostLeadingIconLayoutId),
                            content = move || {
                                if let Some(icon) = icon.clone() {
                                    WrappedLeadingIcon(move || icon());
                                }
                            },
                        );
                    }
                });
            });
        },
    );
}

/// The leading icon of a selectable item's content (the body of Compose's leading icon Box in
/// `DropdownMenuItemContent`): with a selected icon only, it comes in across and fades in as the item is selected
/// (and goes as it isn't); with both, the one for `selected`; else the unselected one.
#[kompozable]
fn LeadingIcon(
    selected: bool,
    leading_icon: Option<Rc<dyn Fn()>>,
    selected_leading_icon: Option<Rc<dyn Fn()>>,
    expand_and_shrink_spec: Arc<dyn FiniteAnimationSpec<IntSize>>,
    fade_in_and_out_spec: Arc<dyn FiniteAnimationSpec<f32>>,
) {
    match (selected_leading_icon, leading_icon) {
        (Some(selected_leading_icon), None) => {
            AnimatedVisibility(
                selected,
                // Defines the animation when the icon enters the composition.
                // It expands horizontally and fades in.
                enter = expand_horizontally(animation_spec = expand_and_shrink_spec.clone())
                    + fade_in(animation_spec = fade_in_and_out_spec.clone()),
                // Defines the animation when the icon exits the composition.
                // It shrinks horizontally and fades out.
                exit = shrink_horizontally(animation_spec = expand_and_shrink_spec)
                    + fade_out(animation_spec = fade_in_and_out_spec),
                content = move |_: &AnimatedVisibilityScope| {
                    let icon = selected_leading_icon.clone();
                    WrappedLeadingIcon(move || icon());
                },
            );
        }
        (Some(selected_leading_icon), Some(_)) if selected => {
            WrappedLeadingIcon(move || selected_leading_icon());
        }
        (Some(_), Some(leading_icon)) => {
            WrappedLeadingIcon(move || leading_icon());
        }
        (None, leading_icon) => {
            let leading_icon = leading_icon.expect("a leading icon (hasLeadingIcon)");
            WrappedLeadingIcon(move || leading_icon());
        }
    }
}

/// The content of a standard item (Compose's internal `DropdownMenuItemContent(text, onClick, …)`, the Row one): a
/// row as wide as the menu, at least 112 dp wide (at most 280: what its intrinsic width takes) and 48 dp tall,
/// clicked (unless not `enabled`) with a bounded ripple, its interactions going to `interaction_source`; in it, in
/// the label large style and `content_padding`, the leading icon (at least 24 dp wide), the text taking the rest
/// (12 dp from the icons) and the trailing icon (at least 24 dp wide), each in its color of `colors`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn DropdownMenuItemContent(
    text: Rc<dyn Fn()>,
    on_click: Rc<dyn Fn()>,
    modifier: Modifier,
    leading_icon: Option<Rc<dyn Fn()>>,
    trailing_icon: Option<Rc<dyn Fn()>>,
    enabled: bool,
    colors: MenuItemColors,
    content_padding: PaddingValues,
    interaction_source: Option<MutableInteractionSource>,
) {
    let label_large = MaterialTheme::typography().label_large;
    Row(
        modifier = modifier
            .clickable_indication_with(
                interaction_source,
                Some(ripple(true, Dp::Unspecified, Color::Unspecified)),
                enabled,
                None,
                None,
                move || on_click(),
            )
            .fill_max_width()
            // Preferred min and max width used during the intrinsic measurement.
            .size_in(
                min_width = DropdownMenuItemDefaultMinWidth,
                max_width = DropdownMenuItemDefaultMaxWidth,
                min_height = MenuListItemContainerHeight,
            )
            .padding_values(content_padding),
        vertical_alignment = Alignment::CenterVertically,
        content = move || {
            let (text, leading_icon, trailing_icon) = (text.clone(), leading_icon.clone(), trailing_icon.clone());
            ProvideTextStyle(label_large.clone(), move || {
                let has_leading_icon = leading_icon.is_some();
                let has_trailing_icon = trailing_icon.is_some();
                if let Some(leading_icon) = leading_icon.clone() {
                    KompozitionLocalProvider(
                        &[LocalContentColor.provides(colors.leading_icon_color_of(enabled, false))],
                        move || {
                            let leading_icon = leading_icon.clone();
                            Box(
                                modifier = Modifier.default_min_size(min_width = ListTokens::ItemLeadingIconSize),
                                content = move || leading_icon(),
                            );
                        },
                    );
                }
                let text = text.clone();
                KompozitionLocalProvider(
                    &[LocalContentColor.provides(colors.text_color_of(enabled, false))],
                    move || {
                        let text = text.clone();
                        Box(
                            modifier = Modifier.weight(1.0).padding_sides(
                                start = if has_leading_icon { DropdownMenuItemHorizontalPadding } else { Dp(0.0) },
                                end = if has_trailing_icon { DropdownMenuItemHorizontalPadding } else { Dp(0.0) },
                            ),
                            content = move || text(),
                        );
                    },
                );
                if let Some(trailing_icon) = trailing_icon.clone() {
                    KompozitionLocalProvider(
                        &[LocalContentColor.provides(colors.trailing_icon_color_of(enabled, false))],
                        move || {
                            let trailing_icon = trailing_icon.clone();
                            Box(
                                modifier = Modifier.default_min_size(min_width = ListTokens::ItemTrailingIconSize),
                                content = move || trailing_icon(),
                            );
                        },
                    );
                }
            });
        },
    );
}

/// Where a menu scales from, as fractions of its size (Compose's internal `calculateTransformOrigin`): across, its
/// left edge where it is to the right of its anchor, its right edge where it is to the left, else the middle of
/// where the two overlap; down the same.
pub(crate) fn calculate_transform_origin(anchor_bounds: IntRect, menu_bounds: IntRect) -> TransformOrigin {
    let pivot_x = if menu_bounds.left >= anchor_bounds.right {
        0.0
    } else if menu_bounds.right <= anchor_bounds.left {
        1.0
    } else if menu_bounds.width() == 0 {
        0.0
    } else {
        let intersection_center =
            (anchor_bounds.left.max(menu_bounds.left) + anchor_bounds.right.min(menu_bounds.right)) / 2;
        (intersection_center - menu_bounds.left) as f32 / menu_bounds.width() as f32
    };
    let pivot_y = if menu_bounds.top >= anchor_bounds.bottom {
        0.0
    } else if menu_bounds.bottom <= anchor_bounds.top {
        1.0
    } else if menu_bounds.height() == 0 {
        0.0
    } else {
        let intersection_center =
            (anchor_bounds.top.max(menu_bounds.top) + anchor_bounds.bottom.min(menu_bounds.bottom)) / 2;
        (intersection_center - menu_bounds.top) as f32 / menu_bounds.height() as f32
    };
    TransformOrigin::new(pivot_x, pivot_y)
}

/// A label over its supporting text (Compose's private `LabelWithSupportingText`): `content` in the label large
/// style, `supporting_text` under it in the body medium one.
#[kompozable]
fn LabelWithSupportingText(supporting_text: Rc<dyn Fn()>, modifier: Modifier, content: Rc<dyn Fn()>) {
    let typography = MaterialTheme::typography();
    Column(
        modifier = modifier,
        content = move || {
            let (content, supporting_text) = (content.clone(), supporting_text.clone());
            ProvideTextStyle(typography.label_large.clone(), move || content());
            ProvideTextStyle(typography.body_medium.clone(), move || supporting_text());
        },
    );
}

/// What keys a morphing shape's group: the shapes it morphs between, by their printed form (Compose keys it by the
/// shapes, `key(shape, selectedShape)`; KompozR's shapes aren't `Hash`, equal ones print the same).
fn shapes_key(kind: &str, shapes: &[&Arc<dyn Shape>]) -> u64 {
    let mut hasher = DefaultHasher::new();
    kind.hash(&mut hasher);
    for shape in shapes {
        // (a theme's shape, KompozR's enum, as the shape it holds: equal to it, as in Kotlin)
        match as_corner_based_shape(&***shape) {
            Some(CornerBasedShape::Rounded(s)) => format!("{s:?}"),
            Some(CornerBasedShape::Cut(s)) => format!("{s:?}"),
            Some(CornerBasedShape::AbsoluteRounded(s)) => format!("{s:?}"),
            Some(CornerBasedShape::AbsoluteCut(s)) => format!("{s:?}"),
            None => format!("{shape:?}"),
        }
        .hash(&mut hasher);
    }
    hasher.finish()
}

/// An item's shape for `selected` (Compose's private `shapeByInteraction(shapes: MenuItemShapes, …)`): morphing to it
/// along `animation_spec` where both shapes are rounded (or corner based) ones, else as it is.
#[kompozable]
fn item_shape_by_interaction(
    shapes: MenuItemShapes,
    selected: bool,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
) -> Arc<dyn Shape> {
    let shape = if selected { shapes.selected_shape.clone() } else { shapes.shape.clone() };

    if shapes.has_rounded_corner_shapes() {
        let current = as_rounded_corner_shape(&*shape).expect("a rounded corner shape");
        key(&shapes_key("rounded", &[&shapes.shape, &shapes.selected_shape]), move || {
            remember_animated_shape(current, animation_spec)
        })
    } else if shapes.has_corner_based_shapes() {
        let current = as_corner_based_shape(&*shape).expect("a corner based shape");
        key(&shapes_key("corner based", &[&shapes.shape, &shapes.selected_shape]), move || {
            remember_animated_shape(current, animation_spec)
        })
    } else {
        shape
    }
}

/// A group's shape (Compose's private `shapeByInteraction(shapes: MenuGroupShapes, …)`): its inactive shape once it
/// has been hovered and isn't, else its shape; morphing as [`item_shape_by_interaction`].
#[kompozable]
fn group_shape_by_interaction(
    shapes: MenuGroupShapes,
    has_been_hovered: bool,
    hovered: bool,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
) -> Arc<dyn Shape> {
    let shape = if has_been_hovered && !hovered { shapes.inactive_shape.clone() } else { shapes.shape.clone() };

    if shapes.has_rounded_corner_shapes() {
        let current = as_rounded_corner_shape(&*shape).expect("a rounded corner shape");
        key(&shapes_key("rounded", &[&shapes.shape, &shapes.inactive_shape]), move || {
            remember_animated_shape(current, animation_spec)
        })
    } else if shapes.has_corner_based_shapes() {
        let current = as_corner_based_shape(&*shape).expect("a corner based shape");
        key(&shapes_key("corner based", &[&shapes.shape, &shapes.inactive_shape]), move || {
            remember_animated_shape(current, animation_spec)
        })
    } else {
        shape
    }
}

/// A leading icon's box (Compose's private `WrappedLeadingIcon`): at least the icon's 20 dp wide, its gap to the
/// text after it.
#[kompozable]
fn WrappedLeadingIcon(content: impl Fn() + 'static) {
    Box(
        modifier = Modifier
            .default_min_size(min_width = SegmentedMenuTokens::ItemLeadingIconSize)
            .padding_sides(end = DropdownMenuIconTextPadding()),
        content = content,
    );
}

/// The layout of a selectable item's content (Compose's private `DropdownMenuItemMeasurePolicy`): the leading icon at
/// the start, the text after it (in what the ghost leading icon — always the widest of the two — and the trailing
/// icon leave), the trailing icon at the end, each centered down; as wide as the constraints allow (unbounded:
/// what its parts take).
pub(crate) struct DropdownMenuItemMeasurePolicy {
    pub(crate) has_leading_icon: bool,
    pub(crate) has_trailing_icon: bool,
}

impl MeasurePolicy for DropdownMenuItemMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        if !self.has_leading_icon && !self.has_trailing_icon {
            just_text_measure_result(scope, measurables, constraints)
        } else if !self.has_trailing_icon {
            no_trailing_icon_measure_result(scope, measurables, constraints)
        } else if !self.has_leading_icon {
            no_leading_icon_measure_result(scope, measurables, constraints)
        } else {
            default_measure_result(scope, measurables, constraints)
        }
    }
}

/// Kotlin's `Alignment.CenterVertically.align(size, space)`.
fn center_vertically(size: i32, space: i32) -> i32 {
    Alignment::CenterVertically.align(size, space)
}

/// Kotlin's `Constraints.copy(minWidth = 0)`.
fn loose_width(constraints: Constraints) -> Constraints {
    named_args!(constraints.copy(min_width = 0))
}

fn just_text_measure_result(
    scope: &MeasureScope,
    measurables: &[&dyn Measurable],
    constraints: Constraints,
) -> MeasureResult {
    let main_content_placeable = find_first(measurables, TextLayoutId).measure(loose_width(constraints));

    let width = if constraints.has_bounded_width() {
        constraints.max_width
    } else {
        // If unbounded, the total width is the sum of the measured static parts.
        main_content_placeable.width()
    };
    let height = constraints.min_height.max(main_content_placeable.height());

    scope.layout(width, height, move |p| {
        p.place_relative(&main_content_placeable, 0, center_vertically(main_content_placeable.height(), height));
    })
}

fn no_leading_icon_measure_result(
    scope: &MeasureScope,
    measurables: &[&dyn Measurable],
    constraints: Constraints,
) -> MeasureResult {
    let trailing_placeable = find_first(measurables, TrailingIconLayoutId).measure(loose_width(constraints));

    let main_content_constraints = if constraints.has_bounded_width() {
        let main_content_max_width = (constraints.max_width - trailing_placeable.width()).max(0);
        Constraints::fixed_width(main_content_max_width)
    } else {
        // If width is unbounded, let the main content measure itself freely.
        loose_width(constraints)
    };

    let main_placeable = find_first(measurables, TextLayoutId).measure(main_content_constraints);

    let width = if constraints.has_bounded_width() {
        constraints.max_width
    } else {
        // If unbounded, the total width is the sum of the measured static parts.
        trailing_placeable.width() + main_placeable.width()
    };

    let height = constraints.min_height.max(trailing_placeable.height().max(main_placeable.height()));

    scope.layout(width, height, move |p| {
        p.place_relative(&main_placeable, 0, center_vertically(main_placeable.height(), height));

        p.place_relative(
            &trailing_placeable,
            width - trailing_placeable.width(),
            center_vertically(trailing_placeable.height(), height),
        );
    })
}

fn no_trailing_icon_measure_result(
    scope: &MeasureScope,
    measurables: &[&dyn Measurable],
    constraints: Constraints,
) -> MeasureResult {
    let leading_placeable = find_first(measurables, LeadingIconLayoutId).measure(loose_width(constraints));
    let ghost_placeable = find_first(measurables, GhostLeadingIconLayoutId).measure(loose_width(constraints));

    let main_content_constraints = if constraints.has_bounded_width() {
        let main_content_max_width = (constraints.max_width - ghost_placeable.width()).max(0);
        Constraints::fixed_width(main_content_max_width)
    } else {
        // If width is unbounded, let the main content measure itself freely.
        loose_width(constraints)
    };
    let main_placeable = find_first(measurables, TextLayoutId).measure(main_content_constraints);

    let width = if constraints.has_bounded_width() {
        constraints.max_width
    } else {
        // If unbounded, the total width is the sum of the measured static parts.
        ghost_placeable.width() + main_placeable.width()
    };
    let height = constraints.min_height.max(leading_placeable.height().max(main_placeable.height()));
    scope.layout(width, height, move |p| {
        p.place_relative(&leading_placeable, 0, center_vertically(leading_placeable.height(), height));

        p.place_relative(
            &main_placeable,
            leading_placeable.width(),
            center_vertically(main_placeable.height(), height),
        );
    })
}

fn default_measure_result(
    scope: &MeasureScope,
    measurables: &[&dyn Measurable],
    constraints: Constraints,
) -> MeasureResult {
    let leading_placeable = find_first(measurables, LeadingIconLayoutId).measure(loose_width(constraints));
    let trailing_placeable = find_first(measurables, TrailingIconLayoutId).measure(loose_width(constraints));
    let ghost_placeable = find_first(measurables, GhostLeadingIconLayoutId).measure(loose_width(constraints));

    let main_content_constraints = if constraints.has_bounded_width() {
        let main_content_max_width =
            (constraints.max_width - ghost_placeable.width() - trailing_placeable.width()).max(0);
        Constraints::fixed_width(main_content_max_width)
    } else {
        // If width is unbounded, let the main content measure itself freely.
        loose_width(constraints)
    };
    let main_placeable = find_first(measurables, TextLayoutId).measure(main_content_constraints);

    let width = if constraints.has_bounded_width() {
        constraints.max_width
    } else {
        // If unbounded, the total width is the sum of the measured static parts.
        ghost_placeable.width() + main_placeable.width() + trailing_placeable.width()
    };
    let height = constraints
        .min_height
        .max(leading_placeable.height().max(main_placeable.height()).max(trailing_placeable.height()));
    scope.layout(width, height, move |p| {
        p.place_relative(&leading_placeable, 0, center_vertically(leading_placeable.height(), height));

        p.place_relative(
            &main_placeable,
            leading_placeable.width(),
            center_vertically(main_placeable.height(), height),
        );

        p.place_relative(
            &trailing_placeable,
            width - trailing_placeable.width(),
            center_vertically(trailing_placeable.height(), height),
        );
    })
}

// Size defaults.
/// The space a menu keeps from the top and bottom of the window (Compose's internal `MenuVerticalMargin`).
pub(crate) const MenuVerticalMargin: Dp = Dp(48.0);
/// The space a menu keeps from the sides of the window (Compose's internal `MenuHorizontalMargin`).
pub(crate) const MenuHorizontalMargin: Dp = Dp(8.0);
const MenuListItemContainerHeight: Dp = Dp(48.0);
pub(crate) const DropdownMenuItemHorizontalPadding: Dp = Dp(12.0);
pub(crate) const DropdownMenuGroupVerticalPadding: Dp = Dp(2.0);

fn DropdownMenuSelectableItemPadding() -> PaddingValues {
    PaddingValues::hv(Dp(4.0), Dp(0.0))
}
fn DropdownMenuSelectableItemWithSupportTexPadding() -> PaddingValues {
    PaddingValues::hv(Dp(4.0), Dp(2.0))
}

/// The gap between a leading icon (or the text) and what follows (Compose's private `DropdownMenuIconTextPadding`:
/// 12 dp for a precision pointer, else 8; read as asked, where Kotlin reads the flag once as the file loads).
fn DropdownMenuIconTextPadding() -> Dp {
    if should_use_precision_pointer_component_sizing_state().value() { Dp(12.0) } else { Dp(8.0) }
}

pub(crate) const DropdownMenuVerticalPadding: Dp = Dp(8.0);
pub(crate) const DropdownMenuItemDefaultMinWidth: Dp = Dp(112.0);
pub(crate) const DropdownMenuItemDefaultMaxWidth: Dp = Dp(280.0);

pub(crate) const DropdownMenuGroupDefaultMinHeight: Dp = Dp(32.0);

const LeadingIconLayoutId: &str = "leadingIcon";
const TextLayoutId: &str = "text";
const TrailingIconLayoutId: &str = "trailingIcon";
const GhostLeadingIconLayoutId: &str = "ghostLeadingIcon";

// Menu open/close animation.
pub(crate) const ExpandedScaleTarget: f32 = 1.0;
pub(crate) const ClosedScaleTarget: f32 = 0.8;
pub(crate) const ExpandedAlphaTarget: f32 = 1.0;
pub(crate) const ClosedAlphaTarget: f32 = 0.0;

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_graphics::RectangleShape;

    #[test]
    fn menus_scale_from_where_they_meet_their_anchor() {
        let anchor = IntRect::new(100, 100, 200, 140);
        // Below and to the right: its top left corner.
        let origin = calculate_transform_origin(anchor, IntRect::new(200, 140, 300, 240));
        assert_eq!(origin, TransformOrigin::new(0.0, 0.0));
        // Above and to the left: its bottom right corner.
        assert_eq!(calculate_transform_origin(anchor, IntRect::new(0, 0, 100, 100)), TransformOrigin::new(1.0, 1.0));
        // Overlapping across: the middle of the overlap (100 to 200 with 100 to 212: 150, 50 of 112 in).
        let origin = calculate_transform_origin(anchor, IntRect::new(100, 140, 212, 300));
        assert!((origin.pivot_fraction_x - 50.0 / 112.0).abs() < 1e-6 && origin.pivot_fraction_y == 0.0);
        // Overlapping down too (a menu over its anchor): the middle of that overlap.
        let origin = calculate_transform_origin(anchor, IntRect::new(150, 120, 250, 220));
        assert_eq!(origin, TransformOrigin::new(25.0 / 100.0, 10.0 / 100.0));
        // Empty bounds overlapping: the start.
        let origin = calculate_transform_origin(anchor, IntRect::new(150, 120, 150, 120));
        assert_eq!(origin, TransformOrigin::new(0.0, 0.0));
    }

    #[test]
    fn shapes_are_rounded_or_corner_based_as_kotlins_subclasses() {
        let rounded = RoundedCornerShape::new(Dp(8.0), Dp(8.0), Dp(8.0), Dp(8.0));
        assert_eq!(as_rounded_corner_shape(&rounded), Some(rounded.clone()));
        let themed: CornerBasedShape = rounded.clone().into();
        assert_eq!(as_rounded_corner_shape(&themed), Some(rounded.clone()));
        let cut = CutCornerShape::new(Dp(4.0), Dp(4.0), Dp(4.0), Dp(4.0));
        assert_eq!(as_rounded_corner_shape(&cut), None);
        assert_eq!(as_corner_based_shape(&cut), Some(cut.clone().into()));
        assert_eq!(as_corner_based_shape(&themed), Some(themed.clone()));
        assert_eq!(as_corner_based_shape(&RectangleShape), None);
        let shapes = MenuItemShapes(Arc::new(rounded.clone()), Arc::new(themed));
        assert!(shapes.has_rounded_corner_shapes() && shapes.has_corner_based_shapes());
        let shapes = MenuItemShapes(Arc::new(rounded), Arc::new(cut.clone()));
        assert!(!shapes.has_rounded_corner_shapes() && shapes.has_corner_based_shapes());
        let groups = MenuGroupShapes(Arc::new(RectangleShape), Arc::new(cut));
        assert!(!groups.has_rounded_corner_shapes() && !groups.has_corner_based_shapes());
    }

    #[test]
    fn item_colors_by_enabled_and_selected() {
        let c = |v: u32| Color::from_argb(0xFF000000 | v);
        let colors = MenuItemColors {
            text_color: c(1),
            leading_icon_color: c(2),
            trailing_icon_color: c(3),
            disabled_text_color: c(4),
            disabled_leading_icon_color: c(5),
            disabled_trailing_icon_color: c(6),
            container_color: c(7),
            disabled_container_color: c(8),
            selected_text_color: c(9),
            selected_leading_icon_color: c(10),
            selected_trailing_icon_color: c(11),
            selected_container_color: c(12),
        };
        let all = |enabled, selected| {
            [
                colors.text_color_of(enabled, selected),
                colors.leading_icon_color_of(enabled, selected),
                colors.trailing_icon_color_of(enabled, selected),
                colors.container_color_of(enabled, selected),
            ]
        };
        assert_eq!(all(true, false), [c(1), c(2), c(3), c(7)]);
        assert_eq!(all(true, true), [c(9), c(10), c(11), c(12)]);
        assert_eq!(all(false, true), [c(4), c(5), c(6), c(8)]);
        assert_eq!(all(false, false), [c(4), c(5), c(6), c(8)]);
    }
}

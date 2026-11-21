//! Material's icon buttons, as Compose's IconButton.kt: an icon (or any content) that is clicked ([`IconButton`],
//! no container, the content in the color around by default; [`FilledIconButton`], [`FilledTonalIconButton`],
//! [`OutlinedIconButton`]) or toggled ([`IconToggleButton`] and its filled, tonal and outlined kinds), 40 dp across
//! in at least the minimum interactive size, the ripple showing its interactions; their colors
//! ([`IconButtonColors`], [`IconToggleButtonColors`]) and, Material Expressive's, the shapes they morph between as
//! they are pressed and checked ([`IconButtonShapes`], [`IconToggleButtonShapes`]).
//!
//! Compose's overloads are composables of their own here: each kind's `shapes` overload is `<Kind>WithShapes`
//! ([`IconButtonWithShapes`], …), its corners animating from shape to shape; the overloads of `IconButton` and
//! `IconToggleButton` without a `shape` (deprecated and hidden from Kotlin source, kept for binary compatibility)
//! are [`IconButtonWithoutShape`] and [`IconToggleButtonWithoutShape`].

// (the deprecated overloads' own generated items name them)
#![allow(deprecated)]

use crate::content_color::LocalContentColor;
use crate::icon_button_defaults::{IconButtonDefaults, IconButtonWidthOption};
use crate::interactive_component_size::InteractiveComponentSizeModifiers;
use crate::internal::animated_shape::{as_corner_based_shape, remember_animated_shape};
use crate::internal::child_parent_semantics::ChildParentSemanticsModifiers;
use crate::ripples::ripple;
use crate::surface::{ClickableSurface, ToggleableSurface};
use crate::tokens::MotionSchemeKeyTokens;
use kompoz_animation::FiniteAnimationSpec;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{BorderStroke, Color, Shape};
use kompoz_runtime::{
    KompozitionLocalProvider, KotlinProperties, UpdatedState, key, kompozable, kompozable_block, named, remember,
    remember_updated_state,
};
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

// ---- IconButton ------------------------------------------------------------------------------------------------

/// An icon button of the standard shape (Compose's deprecated `IconButton(onClick, modifier, enabled, colors,
/// interactionSource, content)` of before its `shape`, hidden from Kotlin source: kept for binary compatibility):
/// [`IconButton`] in [`IconButtonDefaults::standard_shape`].
#[deprecated(note = "Use overload with `shape`")]
#[doc(hidden)]
#[kompozable]
pub fn IconButtonWithoutShape(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.icon_button_colors())] colors: IconButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    IconButton(
        on_click,
        modifier = modifier,
        enabled = enabled,
        colors = colors,
        interaction_source = interaction_source,
        shape = IconButtonDefaults::standard_shape(),
        content = content,
    );
}

/// An icon button (Compose's `IconButton`): `content` (an icon, 24 dp) in the middle of a 40 dp container of
/// `shape` (a circle) in `colors` (no container by default, the content in the content color around — disabled at
/// 38% of it — as `LocalContentColor` to the content), at least the minimum interactive size each way; `on_click`
/// called on a click unless not `enabled`, its interactions shown by the ripple (cut to the shape) and emitted to
/// `interaction_source` (one of its own where none), a button to accessibility, the content's semantics its own.
#[kompozable]
pub fn IconButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.icon_button_colors())] colors: IconButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(IconButtonDefaults::standard_shape())] shape: Arc<dyn Shape>,
    content: impl Fn() + 'static,
) {
    IconButtonImpl(modifier, on_click, enabled, shape, colors, interaction_source, content);
}

/// An icon button whose shape morphs as it is pressed (Compose's Expressive `IconButton(onClick, shapes, …)`): as
/// [`IconButton`], its shape `shapes.shape`, pressed `shapes.pressed_shape`, the corners animating between them
/// (rounded and cut corner shapes; others switch) along the theme's default effects spec.
///
/// Kotlin: `IconButton(onClick: () -> Unit, shapes: IconButtonShapes, modifier: Modifier = Modifier)`
#[kompozable]
pub fn IconButtonWithShapes(
    on_click: impl Fn() + 'static,
    shapes: IconButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.icon_button_colors())] colors: IconButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let shape = shape_for_interaction(shapes, interaction_source.clone());
    IconButtonImpl(modifier, on_click, enabled, shape, colors, Some(interaction_source), content);
}

/// The icon button (Compose's private `IconButtonImpl`): a Box of the small container size (40 dp) in the minimum
/// interactive size, cut to `shape`, on the container color, clickable as a button with the ripple, `content` in
/// its middle in the content color.
#[kompozable]
fn IconButtonImpl(
    modifier: Modifier,
    on_click: impl Fn() + 'static,
    enabled: bool,
    shape: Arc<dyn Shape>,
    colors: IconButtonColors,
    interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let m = modifier
        .minimum_interactive_component_size()
        .size_dp(IconButtonDefaults.small_container_size(width_option = IconButtonWidthOption::Uniform))
        .clip(ClipShape(shape.clone()))
        .background(colors.container_color_for(enabled), shape = shape)
        .clickable_indication_with(
            Some(interaction_source),
            Some(ripple()),
            enabled,
            None,
            Some(Role::Button),
            on_click,
        )
        .child_semantics();
    Box(
        modifier = m,
        content_alignment = Alignment::Center,
        content = move || {
            let content_color = colors.content_color_for(enabled);
            KompozitionLocalProvider(&[LocalContentColor.provides(content_color)], content);
        },
    );
}

// ---- IconToggleButton ------------------------------------------------------------------------------------------

/// An icon toggle button of the standard shape (Compose's deprecated `IconToggleButton(checked, onCheckedChange,
/// modifier, enabled, colors, interactionSource, content)` of before its `shape`, hidden from Kotlin source: kept
/// for binary compatibility): [`IconToggleButton`] in [`IconButtonDefaults::standard_shape`].
#[deprecated(note = "Use overload with `shape`")]
#[doc(hidden)]
#[kompozable]
pub fn IconToggleButtonWithoutShape(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.icon_toggle_button_colors())] colors: IconToggleButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    IconToggleButton(
        checked,
        on_checked_change,
        modifier = modifier,
        enabled = enabled,
        colors = colors,
        interaction_source = interaction_source,
        shape = IconButtonDefaults::standard_shape(),
        content = content,
    );
}

/// An icon toggle button (Compose's `IconToggleButton`): as [`IconButton`], `checked` or not — a click calls
/// `on_checked_change` with the other state — in `colors`' checked or unchecked colors (no container by default,
/// the content in the content color around, checked in the scheme's primary), a checkbox to accessibility.
#[kompozable]
pub fn IconToggleButton(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.icon_toggle_button_colors())] colors: IconToggleButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(IconButtonDefaults::standard_shape())] shape: Arc<dyn Shape>,
    content: impl Fn() + 'static,
) {
    IconToggleButtonImpl(checked, on_checked_change, modifier, enabled, colors, interaction_source, shape, content);
}

/// An icon toggle button whose shape morphs as it is pressed and checked (Compose's Expressive
/// `IconToggleButton(checked, onCheckedChange, shapes, …)`): as [`IconToggleButton`] (its colors the vibrant ones by
/// default: unchecked in on surface variant), its shape `shapes.shape`, pressed `shapes.pressed_shape`, checked
/// `shapes.checked_shape`, the corners animating between them.
///
/// Kotlin: `IconToggleButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, shapes: IconToggleButtonShapes, modifier: Modifier = Modifier)`
#[kompozable]
pub fn IconToggleButtonWithShapes(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    shapes: IconToggleButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.icon_toggle_button_vibrant_colors())] colors: IconToggleButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let shape = checked_shape_for_interaction(checked, shapes, interaction_source.clone());
    IconToggleButtonImpl(
        checked,
        on_checked_change,
        modifier,
        enabled,
        colors,
        Some(interaction_source),
        shape,
        content,
    );
}

/// The icon toggle button (Compose's private `IconToggleButtonImpl`): a Box of the small container size in the
/// minimum interactive size, cut to `shape`, on the container color of its state, toggleable as a checkbox with
/// the ripple, `content` in its middle in its state's content color.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn IconToggleButtonImpl(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    modifier: Modifier,
    enabled: bool,
    colors: IconToggleButtonColors,
    interaction_source: Option<MutableInteractionSource>,
    shape: Arc<dyn Shape>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let m = modifier
        .minimum_interactive_component_size()
        .size_dp(IconButtonDefaults.small_container_size(width_option = IconButtonWidthOption::Uniform))
        .clip(ClipShape(shape))
        .background(colors.container_color_for(enabled, checked).value())
        .toggleable_indication_with(
            checked,
            Some(interaction_source),
            Some(ripple()),
            enabled,
            Some(Role::Checkbox),
            on_checked_change,
        );
    Box(
        modifier = m,
        content_alignment = Alignment::Center,
        content = move || {
            let content_color = colors.content_color_for(enabled, checked).value();
            KompozitionLocalProvider(&[LocalContentColor.provides(content_color)], content);
        },
    );
}

// ---- Filled, tonal, outlined -----------------------------------------------------------------------------------

/// A filled icon button (Compose's `FilledIconButton`): as [`IconButton`], on a container of `shape` (a circle) in
/// `colors` (the scheme's primary, the content on primary; disabled on surface at 10% under on surface at 38%), a
/// surface clicked (its content the surface's content color).
#[kompozable]
pub fn FilledIconButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults::filled_shape())] shape: Arc<dyn Shape>,
    #[default(IconButtonDefaults.filled_icon_button_colors())] colors: IconButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    SurfaceIconButton(on_click, modifier, enabled, shape, colors, None, interaction_source, content);
}

/// A filled icon button whose shape morphs as it is pressed (Compose's Expressive `FilledIconButton(onClick, shapes,
/// …)`): as [`FilledIconButton`], its shape `shapes.shape`, pressed `shapes.pressed_shape`.
///
/// Kotlin: `FilledIconButton(onClick: () -> Unit, shapes: IconButtonShapes, modifier: Modifier = Modifier)`
#[kompozable]
pub fn FilledIconButtonWithShapes(
    on_click: impl Fn() + 'static,
    shapes: IconButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.filled_icon_button_colors())] colors: IconButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    SurfaceIconButtonWithShapes(on_click, modifier, enabled, shapes, colors, None, interaction_source, content);
}

/// A filled icon toggle button (Compose's `FilledIconToggleButton`): as [`IconToggleButton`], on a container of
/// `shape` (a circle) in `colors` (unchecked the scheme's surface container under on surface variant, checked
/// primary under on primary; disabled on surface at 10% under on surface at 38%), a surface toggled.
#[kompozable]
pub fn FilledIconToggleButton(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults::filled_shape())] shape: Arc<dyn Shape>,
    #[default(IconButtonDefaults.filled_icon_toggle_button_colors())] colors: IconToggleButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    SurfaceIconToggleButton(
        checked,
        on_checked_change,
        modifier.semantics(|c| c.set_role(Role::Checkbox)),
        enabled,
        shape,
        colors,
        None,
        interaction_source,
        content,
    );
}

/// A filled icon toggle button whose shape morphs as it is pressed and checked (Compose's Expressive
/// `FilledIconToggleButton(checked, onCheckedChange, shapes, …)`): as [`FilledIconToggleButton`], in `shapes`.
///
/// Kotlin: `FilledIconToggleButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, shapes: IconToggleButtonShapes, modifier: Modifier = Modifier)`
#[kompozable]
pub fn FilledIconToggleButtonWithShapes(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    shapes: IconToggleButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.filled_icon_toggle_button_colors())] colors: IconToggleButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    SurfaceIconToggleButtonWithShapes(
        checked,
        on_checked_change,
        modifier.semantics(|c| c.set_role(Role::Checkbox)),
        enabled,
        shapes,
        colors,
        None,
        interaction_source,
        content,
    );
}

/// A filled tonal icon button (Compose's `FilledTonalIconButton`): as [`FilledIconButton`], in `colors` (the
/// scheme's secondary container, the content on secondary container; disabled on surface at 10% under on surface
/// at 38%).
#[kompozable]
pub fn FilledTonalIconButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults::filled_shape())] shape: Arc<dyn Shape>,
    #[default(IconButtonDefaults.filled_tonal_icon_button_colors())] colors: IconButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    SurfaceIconButton(on_click, modifier, enabled, shape, colors, None, interaction_source, content);
}

/// A filled tonal icon button whose shape morphs as it is pressed (Compose's Expressive
/// `FilledTonalIconButton(onClick, shapes, …)`): as [`FilledTonalIconButton`], in `shapes`.
///
/// Kotlin: `FilledTonalIconButton(onClick: () -> Unit, shapes: IconButtonShapes, modifier: Modifier = Modifier)`
#[kompozable]
pub fn FilledTonalIconButtonWithShapes(
    on_click: impl Fn() + 'static,
    shapes: IconButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.filled_tonal_icon_button_colors())] colors: IconButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    SurfaceIconButtonWithShapes(on_click, modifier, enabled, shapes, colors, None, interaction_source, content);
}

/// A filled tonal icon toggle button (Compose's `FilledTonalIconToggleButton`): as [`FilledIconToggleButton`], in
/// `colors` (unchecked secondary container under on secondary container, checked secondary under on secondary).
#[kompozable]
pub fn FilledTonalIconToggleButton(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults::filled_shape())] shape: Arc<dyn Shape>,
    #[default(IconButtonDefaults.filled_tonal_icon_toggle_button_colors())] colors: IconToggleButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    SurfaceIconToggleButton(
        checked,
        on_checked_change,
        modifier.semantics(|c| c.set_role(Role::Checkbox)),
        enabled,
        shape,
        colors,
        None,
        interaction_source,
        content,
    );
}

/// A filled tonal icon toggle button whose shape morphs as it is pressed and checked (Compose's Expressive
/// `FilledTonalIconToggleButton(checked, onCheckedChange, shapes, …)`): as [`FilledTonalIconToggleButton`], in
/// `shapes`.
///
/// Kotlin: `FilledTonalIconToggleButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, shapes: IconToggleButtonShapes, modifier: Modifier = Modifier)`
#[kompozable]
pub fn FilledTonalIconToggleButtonWithShapes(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    shapes: IconToggleButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.filled_tonal_icon_toggle_button_colors())] colors: IconToggleButtonColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    SurfaceIconToggleButtonWithShapes(
        checked,
        on_checked_change,
        modifier.semantics(|c| c.set_role(Role::Checkbox)),
        enabled,
        shapes,
        colors,
        None,
        interaction_source,
        content,
    );
}

/// An outlined icon button (Compose's `OutlinedIconButton`): as [`FilledIconButton`], in `colors` (no container,
/// the content in the content color around, disabled at 38% of it) inside `border` (`None`: Kotlin's default,
/// [`IconButtonDefaults::outlined_icon_button_border`] of `enabled` — 1 dp in the content color around; `Some(None)`:
/// none).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn OutlinedIconButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults::outlined_shape())] shape: Arc<dyn Shape>,
    #[default(IconButtonDefaults.outlined_icon_button_colors())] colors: IconButtonColors,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let border = match border {
        Some(border) => border,
        None => Some(IconButtonDefaults::outlined_icon_button_border(enabled)),
    };
    SurfaceIconButton(on_click, modifier, enabled, shape, colors, border, interaction_source, content);
}

/// An outlined icon button whose shape morphs as it is pressed (Compose's Expressive `OutlinedIconButton(onClick,
/// shapes, …)`): as [`OutlinedIconButton`], in `shapes`.
///
/// Kotlin: `OutlinedIconButton(onClick: () -> Unit, shapes: IconButtonShapes, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn OutlinedIconButtonWithShapes(
    on_click: impl Fn() + 'static,
    shapes: IconButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.outlined_icon_button_colors())] colors: IconButtonColors,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let border = match border {
        Some(border) => border,
        None => Some(IconButtonDefaults::outlined_icon_button_border(enabled)),
    };
    SurfaceIconButtonWithShapes(on_click, modifier, enabled, shapes, colors, border, interaction_source, content);
}

/// An outlined icon toggle button (Compose's `OutlinedIconToggleButton`): as [`FilledIconToggleButton`], in
/// `colors` (unchecked no container, the content in the content color around; checked the scheme's inverse surface
/// under inverse on surface) inside `border` (`None`: Kotlin's default,
/// [`IconButtonDefaults::outlined_icon_toggle_button_border`] of `enabled` and `checked` — the outline unchecked,
/// none checked; `Some(None)`: none).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn OutlinedIconToggleButton(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults::outlined_shape())] shape: Arc<dyn Shape>,
    #[default(IconButtonDefaults.outlined_icon_toggle_button_colors())] colors: IconToggleButtonColors,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let border = match border {
        Some(border) => border,
        None => IconButtonDefaults::outlined_icon_toggle_button_border(enabled, checked),
    };
    SurfaceIconToggleButton(
        checked,
        on_checked_change,
        modifier.semantics(|c| c.set_role(Role::Checkbox)),
        enabled,
        shape,
        colors,
        border,
        interaction_source,
        content,
    );
}

/// An outlined icon toggle button whose shape morphs as it is pressed and checked (Compose's Expressive
/// `OutlinedIconToggleButton(checked, onCheckedChange, shapes, …)`): as [`OutlinedIconToggleButton`], in `shapes`,
/// its colors the vibrant ones by default (unchecked in on surface variant) inside the vibrant border (`None`:
/// [`IconButtonDefaults::outlined_icon_toggle_button_vibrant_border`] — the scheme's outline variant unchecked,
/// none checked; `Some(None)`: none).
///
/// Kotlin: `OutlinedIconToggleButton(checked: Boolean, onCheckedChange: (Boolean) -> Unit, shapes: IconToggleButtonShapes, modifier: Modifier = Modifier)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn OutlinedIconToggleButtonWithShapes(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    shapes: IconToggleButtonShapes,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(IconButtonDefaults.outlined_icon_toggle_button_vibrant_colors())] colors: IconToggleButtonColors,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let border = match border {
        Some(border) => border,
        None => IconButtonDefaults::outlined_icon_toggle_button_vibrant_border(enabled, checked),
    };
    SurfaceIconToggleButtonWithShapes(
        checked,
        on_checked_change,
        modifier.semantics(|c| c.set_role(Role::Checkbox)),
        enabled,
        shapes,
        colors,
        border,
        interaction_source,
        content,
    );
}

/// An icon button on a surface (Compose's private `SurfaceIconButton(…, shape, …)`): a clicked [`ClickableSurface`]
/// (a button to accessibility) of the colors and `border`, `content` in the middle of a Box of the small container
/// size.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SurfaceIconButton(
    on_click: impl Fn() + 'static,
    modifier: Modifier,
    enabled: bool,
    shape: Arc<dyn Shape>,
    colors: IconButtonColors,
    border: Option<BorderStroke>,
    interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    ClickableSurface(
        on_click,
        modifier = modifier.semantics(|c| c.set_role(Role::Button)),
        enabled = enabled,
        shape = shape,
        color = colors.container_color_for(enabled),
        content_color = Some(colors.content_color_for(enabled)),
        border = border,
        interaction_source = interaction_source,
        content = move || {
            Box(
                modifier = Modifier
                    .size_dp(IconButtonDefaults.small_container_size(width_option = IconButtonWidthOption::Uniform)),
                content_alignment = Alignment::Center,
                content = content,
            );
        },
    );
}

/// The same whose shape morphs (Compose's private `SurfaceIconButton(…, shapes, …)`).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SurfaceIconButtonWithShapes(
    on_click: impl Fn() + 'static,
    modifier: Modifier,
    enabled: bool,
    shapes: IconButtonShapes,
    colors: IconButtonColors,
    border: Option<BorderStroke>,
    interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let shape = shape_for_interaction(shapes, interaction_source.clone());
    SurfaceIconButton(on_click, modifier, enabled, shape, colors, border, Some(interaction_source), content);
}

/// An icon toggle button on a surface (Compose's private `SurfaceIconToggleButton(…, shape, …)`): a toggled
/// [`ToggleableSurface`] (a checkbox to accessibility) of its state's colors and `border`, `content` in the middle
/// of a Box of the small container size.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SurfaceIconToggleButton(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    modifier: Modifier,
    enabled: bool,
    shape: Arc<dyn Shape>,
    colors: IconToggleButtonColors,
    border: Option<BorderStroke>,
    interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let color = colors.container_color_for(enabled, checked).value();
    let content_color = colors.content_color_for(enabled, checked).value();
    ToggleableSurface(
        checked,
        on_checked_change,
        modifier = modifier.semantics(|c| c.set_role(Role::Checkbox)),
        enabled = enabled,
        shape = shape,
        color = color,
        content_color = Some(content_color),
        border = border,
        interaction_source = interaction_source,
        content = move || {
            Box(
                modifier = Modifier
                    .size_dp(IconButtonDefaults.small_container_size(width_option = IconButtonWidthOption::Uniform)),
                content_alignment = Alignment::Center,
                content = content,
            );
        },
    );
}

/// The same whose shape morphs (Compose's private `SurfaceIconToggleButton(…, shapes, …)`).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SurfaceIconToggleButtonWithShapes(
    checked: bool,
    on_checked_change: impl Fn(bool) + 'static,
    modifier: Modifier,
    enabled: bool,
    shapes: IconToggleButtonShapes,
    colors: IconToggleButtonColors,
    border: Option<BorderStroke>,
    interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let shape = checked_shape_for_interaction(checked, shapes, interaction_source.clone());
    SurfaceIconToggleButton(
        checked,
        on_checked_change,
        modifier,
        enabled,
        shape,
        colors,
        border,
        Some(interaction_source),
        content,
    );
}

/// A shared shape as a shape (what `Modifier.clip` takes by value), equal as the shape is.
#[derive(Debug)]
struct ClipShape(Arc<dyn Shape>);

impl PartialEq for ClipShape {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

impl Shape for ClipShape {
    fn create_outline(
        &self,
        size: kompoz_graphics::Size,
        layout_direction: kompoz_graphics::LayoutDirection,
        density: &kompoz_graphics::Density,
    ) -> kompoz_graphics::Outline {
        self.0.create_outline(size, layout_direction, density)
    }
}

// ---- Colors ----------------------------------------------------------------------------------------------------

/// An icon button's colors, enabled and disabled (Compose's `IconButtonColors`): its container's and its
/// content's. Equal where all four are.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct IconButtonColors {
    /// The container's color when enabled.
    pub container_color: Color,
    /// The content's color when enabled.
    pub content_color: Color,
    /// The container's color when not enabled.
    pub disabled_container_color: Color,
    /// The content's color when not enabled.
    pub disabled_content_color: Color,
}

impl IconButtonColors {
    /// Kotlin: `IconButtonColors(containerColor: Color, contentColor: Color, disabledContainerColor: Color, disabledContentColor: Color)`
    pub const fn new(
        container_color: Color,
        content_color: Color,
        disabled_container_color: Color,
        disabled_content_color: Color,
    ) -> IconButtonColors {
        IconButtonColors { container_color, content_color, disabled_container_color, disabled_content_color }
    }

    /// The container's color, enabled or not (Compose's internal `containerColor(enabled)`; the field is its
    /// property).
    pub(crate) fn container_color_for(&self, enabled: bool) -> Color {
        if enabled { self.container_color } else { self.disabled_container_color }
    }

    /// The content's color, enabled or not (Compose's internal `contentColor(enabled)`).
    pub(crate) fn content_color_for(&self, enabled: bool) -> Color {
        if enabled { self.content_color } else { self.disabled_content_color }
    }
}

#[named]
impl IconButtonColors {
    /// These colors, the ones given instead (Compose's `copy`: an unspecified one keeps this one's).
    pub fn copy(
        &self,
        #[default(self.container_color)] container_color: Color,
        #[default(self.content_color)] content_color: Color,
        #[default(self.disabled_container_color)] disabled_container_color: Color,
        #[default(self.disabled_content_color)] disabled_content_color: Color,
    ) -> IconButtonColors {
        IconButtonColors::new(
            container_color.take_or_else(|| self.container_color),
            content_color.take_or_else(|| self.content_color),
            disabled_container_color.take_or_else(|| self.disabled_container_color),
            disabled_content_color.take_or_else(|| self.disabled_content_color),
        )
    }
}

/// An icon toggle button's colors, enabled, disabled and checked (Compose's `IconToggleButtonColors`): its
/// container's and its content's. Equal where all six are.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct IconToggleButtonColors {
    /// The container's color when enabled and unchecked.
    pub container_color: Color,
    /// The content's color when enabled and unchecked.
    pub content_color: Color,
    /// The container's color when not enabled (checked or not).
    pub disabled_container_color: Color,
    /// The content's color when not enabled.
    pub disabled_content_color: Color,
    /// The container's color when enabled and checked.
    pub checked_container_color: Color,
    /// The content's color when enabled and checked.
    pub checked_content_color: Color,
}

impl IconToggleButtonColors {
    /// Kotlin: `IconToggleButtonColors(containerColor: Color, contentColor: Color, disabledContainerColor: Color, disabledContentColor: Color, checkedContainerColor: Color, checkedContentColor: Color)`
    pub const fn new(
        container_color: Color,
        content_color: Color,
        disabled_container_color: Color,
        disabled_content_color: Color,
        checked_container_color: Color,
        checked_content_color: Color,
    ) -> IconToggleButtonColors {
        IconToggleButtonColors {
            container_color,
            content_color,
            disabled_container_color,
            disabled_content_color,
            checked_container_color,
            checked_content_color,
        }
    }

    /// The container's color of the state, as state (Compose's internal composable `containerColor(enabled,
    /// checked)`): disabled, else unchecked or checked.
    #[track_caller]
    pub(crate) fn container_color_for(&self, enabled: bool, checked: bool) -> UpdatedState<Color> {
        let target = if !enabled {
            self.disabled_container_color
        } else if !checked {
            self.container_color
        } else {
            self.checked_container_color
        };
        kompozable_block!(remember_updated_state(target))
    }

    /// The content's color of the state, as state (Compose's internal composable `contentColor(enabled,
    /// checked)`).
    #[track_caller]
    pub(crate) fn content_color_for(&self, enabled: bool, checked: bool) -> UpdatedState<Color> {
        let target = if !enabled {
            self.disabled_content_color
        } else if !checked {
            self.content_color
        } else {
            self.checked_content_color
        };
        kompozable_block!(remember_updated_state(target))
    }
}

#[named]
impl IconToggleButtonColors {
    /// These colors, the ones given instead (Compose's `copy`: an unspecified one keeps this one's).
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.container_color)] container_color: Color,
        #[default(self.content_color)] content_color: Color,
        #[default(self.disabled_container_color)] disabled_container_color: Color,
        #[default(self.disabled_content_color)] disabled_content_color: Color,
        #[default(self.checked_container_color)] checked_container_color: Color,
        #[default(self.checked_content_color)] checked_content_color: Color,
    ) -> IconToggleButtonColors {
        IconToggleButtonColors::new(
            container_color.take_or_else(|| self.container_color),
            content_color.take_or_else(|| self.content_color),
            disabled_container_color.take_or_else(|| self.disabled_container_color),
            disabled_content_color.take_or_else(|| self.disabled_content_color),
            checked_container_color.take_or_else(|| self.checked_container_color),
            checked_content_color.take_or_else(|| self.checked_content_color),
        )
    }
}

// ---- Shapes ----------------------------------------------------------------------------------------------------

/// The shapes an icon button morphs between (Compose's Expressive `IconButtonShapes`): its `shape`, and its
/// `pressed_shape` while pressed. Equal where both are; hashed as Compose's `hashCode` (by what the shapes are).
#[derive(Clone, Debug, KotlinProperties)]
pub struct IconButtonShapes {
    /// The shape at rest.
    pub shape: Arc<dyn Shape>,
    /// The shape while pressed.
    pub pressed_shape: Arc<dyn Shape>,
}

impl IconButtonShapes {
    /// Its shape at rest and pressed (`pressed_shape`: `None` the same `shape` — a button that doesn't morph).
    pub fn new(shape: Arc<dyn Shape>, pressed_shape: Option<Arc<dyn Shape>>) -> IconButtonShapes {
        let pressed_shape = pressed_shape.unwrap_or_else(|| shape.clone());
        IconButtonShapes { shape, pressed_shape }
    }
}

/// Compose's `IconButtonShapes(shape, pressedShape = shape)`: `pressed_shape` `None` (the default) the same `shape`.
#[named(type)]
pub fn IconButtonShapes(
    shape: Arc<dyn Shape>,
    #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
) -> IconButtonShapes {
    IconButtonShapes::new(shape, pressed_shape)
}

#[named]
impl IconButtonShapes {
    /// These shapes, the ones given instead (Compose's `copy`: `None` keeps this one's).
    pub fn copy(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
    ) -> IconButtonShapes {
        IconButtonShapes {
            shape: shape.unwrap_or_else(|| self.shape.clone()),
            pressed_shape: pressed_shape.unwrap_or_else(|| self.pressed_shape.clone()),
        }
    }
}

impl IconButtonShapes {
    /// Whether both shapes are corner based ones, whose corners animate (Compose's internal
    /// `IconButtonShapes.isCornerBasedShape`).
    pub(crate) fn is_corner_based_shape(&self) -> bool {
        as_corner_based_shape(&self.shape).is_some() && as_corner_based_shape(&self.pressed_shape).is_some()
    }

    /// Whether it doesn't morph: its shapes the same one (Compose's internal `IconButtonShapes.isStatic`, by
    /// identity).
    pub(crate) fn is_static(&self) -> bool {
        Arc::ptr_eq(&self.shape, &self.pressed_shape)
    }
}

impl PartialEq for IconButtonShapes {
    fn eq(&self, other: &Self) -> bool {
        *self.shape == *other.shape && *self.pressed_shape == *other.pressed_shape
    }
}

/// Compose's `hashCode`: the shapes' (by what they are, as their `Debug` says).
impl Hash for IconButtonShapes {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_shape(&self.shape, state);
        hash_shape(&self.pressed_shape, state);
    }
}

/// The shapes an icon toggle button morphs between (Compose's Expressive `IconToggleButtonShapes`): its `shape`,
/// its `pressed_shape` while pressed and its `checked_shape` while checked. Equal where all three are.
#[derive(Clone, Debug, KotlinProperties)]
pub struct IconToggleButtonShapes {
    /// The shape at rest, unchecked.
    pub shape: Arc<dyn Shape>,
    /// The shape while pressed.
    pub pressed_shape: Arc<dyn Shape>,
    /// The shape while checked.
    pub checked_shape: Arc<dyn Shape>,
}

impl IconToggleButtonShapes {
    /// Its shapes at rest, pressed and checked (`pressed_shape`, `checked_shape`: `None` the same `shape`).
    pub fn new(
        shape: Arc<dyn Shape>,
        pressed_shape: Option<Arc<dyn Shape>>,
        checked_shape: Option<Arc<dyn Shape>>,
    ) -> IconToggleButtonShapes {
        let pressed_shape = pressed_shape.unwrap_or_else(|| shape.clone());
        let checked_shape = checked_shape.unwrap_or_else(|| shape.clone());
        IconToggleButtonShapes { shape, pressed_shape, checked_shape }
    }
}

/// Compose's `IconToggleButtonShapes(shape, pressedShape = shape, checkedShape = shape)`: `pressed_shape` and
/// `checked_shape` `None` (the defaults) the same `shape`.
#[named(type)]
pub fn IconToggleButtonShapes(
    shape: Arc<dyn Shape>,
    #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
    #[default(None)] checked_shape: Option<Arc<dyn Shape>>,
) -> IconToggleButtonShapes {
    IconToggleButtonShapes::new(shape, pressed_shape, checked_shape)
}

#[named]
impl IconToggleButtonShapes {
    /// These shapes, the ones given instead (Compose's `copy`: `None` keeps this one's).
    pub fn copy(
        &self,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(None)] pressed_shape: Option<Arc<dyn Shape>>,
        #[default(None)] checked_shape: Option<Arc<dyn Shape>>,
    ) -> IconToggleButtonShapes {
        IconToggleButtonShapes {
            shape: shape.unwrap_or_else(|| self.shape.clone()),
            pressed_shape: pressed_shape.unwrap_or_else(|| self.pressed_shape.clone()),
            checked_shape: checked_shape.unwrap_or_else(|| self.checked_shape.clone()),
        }
    }
}

impl IconToggleButtonShapes {
    /// Whether all three are corner based shapes (Compose's internal `IconToggleButtonShapes.isCornerBasedShape`).
    pub(crate) fn is_corner_based_shape(&self) -> bool {
        as_corner_based_shape(&self.shape).is_some()
            && as_corner_based_shape(&self.pressed_shape).is_some()
            && as_corner_based_shape(&self.checked_shape).is_some()
    }

    /// Whether it doesn't morph: its shapes the same one (Compose's internal `IconToggleButtonShapes.isStatic`).
    pub(crate) fn is_static(&self) -> bool {
        Arc::ptr_eq(&self.shape, &self.pressed_shape) && Arc::ptr_eq(&self.shape, &self.checked_shape)
    }
}

impl PartialEq for IconToggleButtonShapes {
    fn eq(&self, other: &Self) -> bool {
        *self.shape == *other.shape
            && *self.pressed_shape == *other.pressed_shape
            && *self.checked_shape == *other.checked_shape
    }
}

/// Compose's `hashCode`: the shapes'.
impl Hash for IconToggleButtonShapes {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_shape(&self.shape, state);
        hash_shape(&self.pressed_shape, state);
        hash_shape(&self.checked_shape, state);
    }
}

/// A shape's hash: what it is, as its `Debug` says (shapes of the same corners alike, an animated shape its own).
fn hash_shape<H: Hasher>(shape: &Arc<dyn Shape>, state: &mut H) {
    struct Writer<'h, H: Hasher>(&'h mut H);
    impl<H: Hasher> fmt::Write for Writer<'_, H> {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            self.0.write(s.as_bytes());
            Ok(())
        }
    }
    let _ = fmt::write(&mut Writer(state), format_args!("{shape:?}"));
}

// ---- Shape morphing --------------------------------------------------------------------------------------------

/// The shape of an icon button now (Compose's private `shapeForInteraction(shapes, interactionSource)`): a static
/// one's shape, else its shape or pressed shape as it is pressed, animated along the theme's default effects spec
/// (no bounce in this component).
#[kompozable]
fn shape_for_interaction(shapes: IconButtonShapes, interaction_source: MutableInteractionSource) -> Arc<dyn Shape> {
    if shapes.is_static() {
        return shapes.shape;
    }
    // (Compose's TODO: load the motion scheme tokens from the component tokens file; DefaultEffects on purpose,
    // no bounce)
    let default_animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>();
    let pressed = interaction_source.collect_is_pressed_as_state();

    shape_by_interaction(shapes, pressed.get(), default_animation_spec)
}

/// The shape of an icon toggle button now (Compose's private `shapeForInteraction(checked, shapes,
/// interactionSource)`).
#[kompozable]
fn checked_shape_for_interaction(
    checked: bool,
    shapes: IconToggleButtonShapes,
    interaction_source: MutableInteractionSource,
) -> Arc<dyn Shape> {
    if shapes.is_static() {
        return shapes.shape;
    }
    let default_animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>();
    let pressed = interaction_source.collect_is_pressed_as_state();

    checked_shape_by_interaction(shapes, pressed.get(), checked, default_animation_spec)
}

/// The pressed or rest shape (Compose's private `shapeByInteraction(shapes, pressed, animationSpec)`): animated to
/// where both are corner based (remembered for these shapes), else switched.
#[kompozable]
fn shape_by_interaction(
    shapes: IconButtonShapes,
    pressed: bool,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
) -> Arc<dyn Shape> {
    let shape = if pressed { shapes.pressed_shape.clone() } else { shapes.shape.clone() };

    if shapes.is_corner_based_shape()
        && let Some(corner_based) = as_corner_based_shape(&shape)
    {
        return key(&shapes, || remember_animated_shape(corner_based, animation_spec));
    }
    shape
}

/// The pressed, checked or rest shape (Compose's private `shapeByInteraction(shapes, pressed, checked,
/// animationSpec)`): pressed first, then checked.
#[kompozable]
fn checked_shape_by_interaction(
    shapes: IconToggleButtonShapes,
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

    if shapes.is_corner_based_shape()
        && let Some(corner_based) = as_corner_based_shape(&shape)
    {
        return key(&shapes, || remember_animated_shape(corner_based, animation_spec));
    }
    shape
}

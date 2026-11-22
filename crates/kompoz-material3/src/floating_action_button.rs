//! Material's floating action buttons, as Compose's FloatingActionButton.kt: a screen's primary action on a
//! clickable surface of the primary container color, raised 6 dp (more as it is hovered) — the
//! [`FloatingActionButton`] (56 dp), [`SmallFloatingActionButton`] (40), Material Expressive's
//! [`MediumFloatingActionButton`] (80) and [`LargeFloatingActionButton`] (96) —; the extended ones with a label
//! ([`ExtendedFloatingActionButton`] and the Expressive small, medium and large sizes), whose label and icon forms
//! narrow to their icon when not `expanded`, animated; [`FloatingActionButtonDefaults`] (shapes, sizes, colors,
//! elevations), [`FloatingActionButtonElevation`], and the modifier showing and hiding a FAB by scale and alpha
//! ([`FloatingActionButtonModifiers::animate_floating_action_button`]).
//!
//! Compose's extended FABs taking `text` and `icon` (and `expanded`) are composables of their own here, named by
//! those parameters: [`TextIconExtendedFloatingActionButton`] (Kotlin's `ExtendedFloatingActionButton(text, icon,
//! onClick, …)`), [`TextIconSmallExtendedFloatingActionButton`], [`TextIconMediumExtendedFloatingActionButton`] and
//! [`TextIconLargeExtendedFloatingActionButton`].

use crate::color_scheme::content_color_for;
use crate::internal::elevation::AnimateElevation;
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::material_theme::MaterialTheme;
use crate::shapes::ShapeDefaults;
use crate::surface::ClickableSurface;
use crate::tokens::{
    ElevationTokens, ExtendedFabLargeTokens, ExtendedFabMediumTokens, ExtendedFabPrimaryTokens, ExtendedFabSmallTokens,
    FabBaselineTokens, FabLargeTokens, FabMediumTokens, FabPrimaryContainerTokens, FabSmallTokens,
    MotionSchemeKeyTokens, TypographyKeyTokens,
};
use kompoz_animation::{
    Animatable, AnimatedVisibility, AnimationSpec, AnimationVector1D, EnterTransition, ExitTransition, Segment,
    VectorConverter, expand_horizontally, fade_in, fade_out, shrink_horizontally, update_transition,
};
use kompoz_coroutines::{LaunchedEffect, coroutine_scope};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, IntOffset, IntSize, Offset, Shape, lerp};
use kompoz_runtime::{
    State, derived_state_of, kompozable, kompozable_block, named, named_args, remember, remember_keyed,
};
use kompoz_text::TextStyle;
use kompoz_ui::draw::{CacheDrawModifierNode, CacheDrawScope, ContentDrawScope};
use kompoz_ui::layout::LayoutModifierNode;
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::semantics::Role;
use kompoz_ui::{
    Alignment, Constraints, Measurable, MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement,
    NodeCx, NodeHandle, node_capabilities,
};
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

// ---- The FABs --------------------------------------------------------------------------------------------------

/// A floating action button (Compose's `FloatingActionButton`): a screen's primary action, `content` (an icon)
/// centered in at least 56 by 56 dp on a clickable surface of `shape` in `container_color`, raised by `elevation`
/// (its default as tonal elevation, its shadow following its interactions), its content in `content_color` (the
/// `on` color of `container_color`, else the one around; Kotlin's default `contentColorFor(containerColor)`, here
/// `None`) and the theme's `label_large`; `on_click` called when it is clicked; a button to accessibility. Its
/// interactions are emitted to `interaction_source` (one of its own where none).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn FloatingActionButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingActionButtonDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
    StyledFloatingActionButton(
        on_click,
        ExtendedFabPrimaryTokens::LabelTextFont.value(),
        FabBaselineTokens::ContainerWidth,
        FabBaselineTokens::ContainerHeight,
        modifier,
        shape,
        container_color,
        content_color,
        elevation,
        interaction_source,
        content,
    );
}

/// A FAB of a text style and least size of its own (Compose's private `FloatingActionButton(onClick, textStyle,
/// minWidth, minHeight, …)`): `content` in a Box of at least `min_width` by `min_height`, centered, its text in
/// `text_style`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn StyledFloatingActionButton(
    on_click: impl Fn() + 'static,
    text_style: TextStyle,
    min_width: Dp,
    min_height: Dp,
    modifier: Modifier,
    shape: Arc<dyn Shape>,
    container_color: Color,
    content_color: Color,
    elevation: FloatingActionButtonElevation,
    interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let tonal_elevation = elevation.tonal_elevation();
    let shadow_elevation = elevation.shadow_elevation(&interaction_source).value();
    let content = Rc::new(content);
    ClickableSurface(
        on_click,
        modifier = modifier.semantics(|s| s.set_role(Role::Button)),
        shape = shape,
        color = container_color,
        content_color = Some(content_color),
        tonal_elevation = tonal_elevation,
        shadow_elevation = shadow_elevation,
        interaction_source = Some(interaction_source),
        content = move || {
            let (content, text_style) = (content.clone(), text_style.clone());
            ProvideContentColorTextStyle(content_color, text_style, move || {
                let content = content.clone();
                Box(
                    modifier = Modifier.default_min_size_with(min_width, min_height),
                    content_alignment = Alignment::Center,
                    content = move || content(),
                );
            });
        },
    );
}

/// A small floating action button (Compose's `SmallFloatingActionButton`): at least 40 by 40 dp, the theme's
/// medium corners. See [`FloatingActionButton`] for the rest.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SmallFloatingActionButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingActionButtonDefaults::small_shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    FloatingActionButton(
        on_click,
        modifier = modifier.size_in_with(
            FabSmallTokens::ContainerWidth,
            FabSmallTokens::ContainerHeight,
            Dp::Unspecified,
            Dp::Unspecified,
        ),
        shape = shape,
        container_color = container_color,
        content_color = content_color,
        elevation = elevation,
        interaction_source = interaction_source,
        content = content,
    );
}

/// A medium floating action button (Compose's Expressive `MediumFloatingActionButton`): at least 80 by 80 dp, the
/// large increased corners (20 dp); its icon best [`FloatingActionButtonDefaults::MediumIconSize`]. See
/// [`FloatingActionButton`] for the rest.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn MediumFloatingActionButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingActionButtonDefaults::medium_shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    FloatingActionButton(
        on_click,
        modifier = modifier.size_in_with(
            FabMediumTokens::ContainerWidth,
            FabMediumTokens::ContainerHeight,
            Dp::Unspecified,
            Dp::Unspecified,
        ),
        shape = shape,
        container_color = container_color,
        content_color = content_color,
        elevation = elevation,
        interaction_source = interaction_source,
        content = content,
    );
}

/// A large floating action button (Compose's `LargeFloatingActionButton`): at least 96 by 96 dp, the theme's
/// extra large corners; its icon best [`FloatingActionButtonDefaults::LargeIconSize`]. See
/// [`FloatingActionButton`] for the rest.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn LargeFloatingActionButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingActionButtonDefaults::large_shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    FloatingActionButton(
        on_click,
        modifier = modifier.size_in_with(
            FabLargeTokens::ContainerWidth,
            FabLargeTokens::ContainerHeight,
            Dp::Unspecified,
            Dp::Unspecified,
        ),
        shape = shape,
        container_color = container_color,
        content_color = content_color,
        elevation = elevation,
        interaction_source = interaction_source,
        content = content,
    );
}

/// An extended FAB of a size (Compose's small, medium and large extended FABs' bodies): `content` in a row,
/// centered, padded `padding_start` and `padding_end`, in `text_style`, at least `min_width` by `min_height`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SizedExtendedFloatingActionButtonRow(
    on_click: impl Fn() + 'static,
    text_style: TextStyle,
    min_width: Dp,
    min_height: Dp,
    padding_start: Dp,
    padding_end: Dp,
    modifier: Modifier,
    shape: Arc<dyn Shape>,
    container_color: Color,
    content_color: Option<Color>,
    elevation: FloatingActionButtonElevation,
    interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
    let content = Rc::new(content);
    StyledFloatingActionButton(
        on_click,
        text_style,
        min_width,
        min_height,
        modifier,
        shape,
        container_color,
        content_color,
        elevation,
        interaction_source,
        move || {
            let content = content.clone();
            Row(
                modifier = Modifier.padding_sides_with(padding_start, Dp(0.0), padding_end, Dp(0.0)),
                horizontal_arrangement = Arrangement::Center,
                vertical_alignment = Alignment::CenterVertically,
                content = move || content(),
            );
        },
    );
}

/// A small extended FAB (Compose's Expressive `SmallExtendedFloatingActionButton(onClick, …, content)`): `content`
/// (a label, an icon before it) in a row 16 dp in from each side, at least 56 by 56 dp, the theme's
/// `title_medium`, the large corners. See [`FloatingActionButton`] for the rest.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SmallExtendedFloatingActionButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingActionButtonDefaults::small_extended_fab_shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    SizedExtendedFloatingActionButtonRow(
        on_click,
        SmallExtendedFabTextStyle.value(),
        SmallExtendedFabMinimumWidth,
        SmallExtendedFabMinimumHeight,
        SmallExtendedFabPaddingStart,
        SmallExtendedFabPaddingEnd,
        modifier,
        shape,
        container_color,
        content_color,
        elevation,
        interaction_source,
        content,
    );
}

/// A medium extended FAB (Compose's Expressive `MediumExtendedFloatingActionButton(onClick, …, content)`): 26 dp
/// in from each side, at least 80 by 80 dp, the theme's `title_large`, the large increased corners. See
/// [`SmallExtendedFloatingActionButton`].
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn MediumExtendedFloatingActionButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingActionButtonDefaults::medium_extended_fab_shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    SizedExtendedFloatingActionButtonRow(
        on_click,
        MediumExtendedFabTextStyle.value(),
        MediumExtendedFabMinimumWidth,
        MediumExtendedFabMinimumHeight,
        MediumExtendedFabPaddingStart,
        MediumExtendedFabPaddingEnd,
        modifier,
        shape,
        container_color,
        content_color,
        elevation,
        interaction_source,
        content,
    );
}

/// A large extended FAB (Compose's Expressive `LargeExtendedFloatingActionButton(onClick, …, content)`): 28 dp in
/// from each side, at least 96 by 96 dp, the theme's `headline_small`, the extra large corners. See
/// [`SmallExtendedFloatingActionButton`].
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn LargeExtendedFloatingActionButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingActionButtonDefaults::large_extended_fab_shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    SizedExtendedFloatingActionButtonRow(
        on_click,
        LargeExtendedFabTextStyle.value(),
        LargeExtendedFabMinimumWidth,
        LargeExtendedFabMinimumHeight,
        LargeExtendedFabPaddingStart,
        LargeExtendedFabPaddingEnd,
        modifier,
        shape,
        container_color,
        content_color,
        elevation,
        interaction_source,
        content,
    );
}

/// An extended FAB (Compose's `ExtendedFloatingActionButton(onClick, …, content)`): `content` (a label, an icon
/// before it) in a row 20 dp in from each side, at least 80 dp wide. See [`FloatingActionButton`] for the rest.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ExtendedFloatingActionButton(
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingActionButtonDefaults::extended_fab_shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let content = Rc::new(content);
    FloatingActionButton(
        on_click,
        modifier = modifier,
        shape = shape,
        container_color = container_color,
        content_color = content_color,
        elevation = elevation,
        interaction_source = interaction_source,
        content = move || {
            let content = content.clone();
            Row(
                modifier = Modifier
                    .size_in_with(ExtendedFabMinimumWidth, Dp::Unspecified, Dp::Unspecified, Dp::Unspecified)
                    .padding_hv_with(ExtendedFabTextPadding, Dp(0.0)),
                horizontal_arrangement = Arrangement::Center,
                vertical_alignment = Alignment::CenterVertically,
                content = move || content(),
            );
        },
    );
}

/// An extended FAB of a label and an icon (Compose's `ExtendedFloatingActionButton(text, icon, onClick, …)`):
/// `icon` 16 dp in, `text` 12 dp after it, 20 dp before the end, at least 80 dp wide; not `expanded`, the label
/// goes (fading out and shrinking towards the start, the theme's motion) and the FAB narrows to its icon, a
/// square of at least 56 dp, the icon centered — it comes back the same way. The label is no part of its
/// semantics (describe the icon). See [`FloatingActionButton`] for the rest.
///
/// Kotlin: `ExtendedFloatingActionButton(text: @Composable () -> Unit, icon: @Composable () -> Unit, onClick: () -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TextIconExtendedFloatingActionButton(
    text: impl Fn() + 'static,
    icon: impl Fn() + 'static,
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] expanded: bool,
    #[default(FloatingActionButtonDefaults::extended_fab_shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let (text, icon) = (Rc::new(text), Rc::new(icon));
    FloatingActionButton(
        on_click,
        modifier = modifier,
        shape = shape,
        container_color = container_color,
        content_color = content_color,
        elevation = elevation,
        interaction_source = interaction_source,
        content = move || {
            let start_padding = if expanded { ExtendedFabStartIconPadding } else { Dp(0.0) };
            let end_padding = if expanded { ExtendedFabTextPadding } else { Dp(0.0) };
            let min_width = if expanded { ExtendedFabMinimumWidth } else { FabBaselineTokens::ContainerWidth };
            let (text, icon) = (text.clone(), icon.clone());
            Row(
                modifier = Modifier
                    .size_in_with(min_width, Dp::Unspecified, Dp::Unspecified, Dp::Unspecified)
                    .padding_sides_with(start_padding, Dp(0.0), end_padding, Dp(0.0)),
                vertical_alignment = Alignment::CenterVertically,
                horizontal_arrangement = if expanded { Arrangement::Start } else { Arrangement::Center },
                content = move || {
                    icon();
                    let text = text.clone();
                    AnimatedVisibility(
                        expanded,
                        enter = extended_fab_expand_animation(),
                        exit = extended_fab_collapse_animation(),
                        content = move |_| {
                            let text = text.clone();
                            Row(
                                modifier = Modifier.clear_and_set_semantics(|_| {}),
                                content = move || {
                                    Spacer(Modifier.width(ExtendedFabEndIconPadding));
                                    text();
                                },
                            );
                        },
                    );
                },
            );
        },
    );
}

/// A small extended FAB of a label and an icon (Compose's Expressive `SmallExtendedFloatingActionButton(text, icon,
/// onClick, …)`): `icon` 16 dp in, `text` 8 dp after it, 16 dp before the end, 56 dp tall, the theme's
/// `title_medium`; not `expanded`, it narrows to a 56 dp square around its icon, its width and the label's alpha
/// animated (the theme's fast spatial and fast effects specs), the label gone once it has. The label is no part of
/// its semantics. See [`FloatingActionButton`] for the rest.
///
/// Kotlin: `SmallExtendedFloatingActionButton(text: @Composable () -> Unit, icon: @Composable () -> Unit, onClick: () -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TextIconSmallExtendedFloatingActionButton(
    text: impl Fn() + 'static,
    icon: impl Fn() + 'static,
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] expanded: bool,
    #[default(FloatingActionButtonDefaults::small_extended_fab_shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    SizedExtendedFloatingActionButton(
        text,
        icon,
        on_click,
        SmallExtendedFabTextStyle.value(),
        SmallExtendedFabMinimumWidth,
        SmallExtendedFabMinimumHeight,
        SmallExtendedFabPaddingStart,
        SmallExtendedFabPaddingEnd,
        SmallExtendedFabIconPadding,
        modifier,
        expanded,
        shape,
        container_color,
        content_color,
        elevation,
        interaction_source,
    );
}

/// A medium extended FAB of a label and an icon (Compose's Expressive `MediumExtendedFloatingActionButton(text,
/// icon, onClick, …)`): 26 dp in from each side, `text` 12 dp after the icon (its best size
/// [`FloatingActionButtonDefaults::MediumIconSize`]), 80 dp tall, the theme's `title_large`; an 80 dp square not
/// `expanded`. See [`TextIconSmallExtendedFloatingActionButton`].
///
/// Kotlin: `MediumExtendedFloatingActionButton(text: @Composable () -> Unit, icon: @Composable () -> Unit, onClick: () -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TextIconMediumExtendedFloatingActionButton(
    text: impl Fn() + 'static,
    icon: impl Fn() + 'static,
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] expanded: bool,
    #[default(FloatingActionButtonDefaults::medium_extended_fab_shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    SizedExtendedFloatingActionButton(
        text,
        icon,
        on_click,
        MediumExtendedFabTextStyle.value(),
        MediumExtendedFabMinimumWidth,
        MediumExtendedFabMinimumHeight,
        MediumExtendedFabPaddingStart,
        MediumExtendedFabPaddingEnd,
        MediumExtendedFabIconPadding,
        modifier,
        expanded,
        shape,
        container_color,
        content_color,
        elevation,
        interaction_source,
    );
}

/// A large extended FAB of a label and an icon (Compose's Expressive `LargeExtendedFloatingActionButton(text, icon,
/// onClick, …)`): 28 dp in from each side, `text` 16 dp after the icon (its best size
/// [`FloatingActionButtonDefaults::LargeIconSize`]), 96 dp tall, the theme's `headline_small`; a 96 dp square not
/// `expanded`. See [`TextIconSmallExtendedFloatingActionButton`].
///
/// Kotlin: `LargeExtendedFloatingActionButton(text: @Composable () -> Unit, icon: @Composable () -> Unit, onClick: () -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TextIconLargeExtendedFloatingActionButton(
    text: impl Fn() + 'static,
    icon: impl Fn() + 'static,
    on_click: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] expanded: bool,
    #[default(FloatingActionButtonDefaults::large_extended_fab_shape())] shape: Arc<dyn Shape>,
    #[default(FloatingActionButtonDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(FloatingActionButtonDefaults.elevation())] elevation: FloatingActionButtonElevation,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    SizedExtendedFloatingActionButton(
        text,
        icon,
        on_click,
        LargeExtendedFabTextStyle.value(),
        LargeExtendedFabMinimumWidth,
        LargeExtendedFabMinimumHeight,
        LargeExtendedFabPaddingStart,
        LargeExtendedFabPaddingEnd,
        LargeExtendedFabIconPadding,
        modifier,
        expanded,
        shape,
        container_color,
        content_color,
        elevation,
        interaction_source,
    );
}

/// The Expressive extended FABs of a label and an icon (Compose's private `ExtendedFloatingActionButton(text, icon,
/// onClick, textStyle, minWidth, minHeight, startPadding, endPadding, iconPadding, …)`): the row's width animated
/// between `min_width` (collapsed) and its widest (expanded, its max intrinsic width), the label (after
/// `icon_padding`) faded with it and left out once fully collapsed.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SizedExtendedFloatingActionButton(
    text: impl Fn() + 'static,
    icon: impl Fn() + 'static,
    on_click: impl Fn() + 'static,
    text_style: TextStyle,
    min_width: Dp,
    min_height: Dp,
    start_padding: Dp,
    end_padding: Dp,
    icon_padding: Dp,
    modifier: Modifier,
    expanded: bool,
    shape: Arc<dyn Shape>,
    container_color: Color,
    content_color: Option<Color>,
    elevation: FloatingActionButtonElevation,
    interaction_source: Option<MutableInteractionSource>,
) {
    let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
    let (text, icon) = (Rc::new(text), Rc::new(icon));
    StyledFloatingActionButton(
        on_click,
        text_style,
        Dp::Unspecified,
        Dp::Unspecified,
        modifier,
        shape,
        container_color,
        content_color,
        elevation,
        interaction_source,
        move || {
            let expand_transition =
                update_transition(if expanded { 1.0f32 } else { 0.0 }, Some("expanded state".to_string()));
            // TODO Load the motionScheme tokens from the component tokens file
            let size_animation_spec = MotionSchemeKeyTokens::FastSpatial.value::<f32>();
            let opacity_animation_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>();
            let expanded_width_progress =
                named_args!(expand_transition.animate_float(
                    transition_spec = move |_: &Segment<f32>| size_animation_spec.clone(),
                    |it: &f32| *it
                ));
            let expanded_alpha_progress = named_args!(expand_transition.animate_float(
                transition_spec = move |_: &Segment<f32>| opacity_animation_spec.clone(),
                |it: &f32| *it
            ));
            let (text, icon) = (text.clone(), icon.clone());
            let width_progress = expanded_width_progress.clone();
            Row(
                modifier = Modifier
                    .layout(move |scope, measurable, constraints| {
                        let expanded_width = measurable.max_intrinsic_width(constraints.max_height);
                        let width =
                            lerp_int(scope.density().round_to_px(min_width), expanded_width, width_progress.value());
                        let placeable = measurable.measure(constraints);
                        scope.layout(width, placeable.height(), move |p| p.place(&placeable, 0, 0))
                    })
                    .size_in_with(min_width, min_height, Dp::Unspecified, Dp::Unspecified)
                    .padding_sides_with(start_padding, Dp(0.0), end_padding, Dp(0.0)),
                vertical_alignment = Alignment::CenterVertically,
                content = move || {
                    icon();
                    let transition = expand_transition.clone();
                    let fully_collapsed = remember_keyed(expand_transition.clone(), move || {
                        let transition = transition.clone();
                        derived_state_of(move || transition.current_state() == 0.0 && !transition.is_running())
                    });
                    if !fully_collapsed.value() {
                        let (text, alpha) = (text.clone(), expanded_alpha_progress.clone());
                        Row(
                            modifier = Modifier
                                .clear_and_set_semantics(|_| {})
                                .graphics_layer_block(move |l| l.alpha = alpha.value()),
                            content = move || {
                                Spacer(Modifier.width(icon_padding));
                                text();
                            },
                        );
                    }
                },
            );
        },
    );
}

// ---- FloatingActionButtonDefaults ------------------------------------------------------------------------------

/// The floating action buttons' defaults (Compose's `FloatingActionButtonDefaults`): their icon sizes, shapes,
/// container color and elevations. Plain values are associated consts; what Compose reads in composition is an
/// associated function, what takes arguments a method of the value (`FloatingActionButtonDefaults.elevation()`).
#[derive(Clone, Copy, Debug)]
pub struct FloatingActionButtonDefaults;

impl FloatingActionButtonDefaults {
    /// The scale a FAB hidden by [`animate_floating_action_button`](FloatingActionButtonModifiers::animate_floating_action_button)
    /// shrinks to (Compose's internal `ShowHideTargetScale`).
    pub(crate) const ShowHideTargetScale: f32 = 0.2;

    /// A medium FAB's icon size, 28 dp (Compose's Expressive `MediumIconSize`).
    pub const MediumIconSize: Dp = FabMediumTokens::IconSize;

    /// A large FAB's icon size, 36 dp (Compose's `LargeIconSize`).
    pub const LargeIconSize: Dp = Dp(36.0); // TODO: FabLargeTokens.IconSize is incorrect

    /// A FAB's shape, the theme's large corners (Compose's `shape`).
    pub fn shape() -> Arc<dyn Shape> {
        FabBaselineTokens::ContainerShape.value()
    }

    /// A small FAB's shape, the theme's medium corners (Compose's `smallShape`).
    pub fn small_shape() -> Arc<dyn Shape> {
        FabSmallTokens::ContainerShape.value()
    }

    /// A medium FAB's shape, 20 dp corners (Compose's Expressive `mediumShape`: `ShapeDefaults.LargeIncreased`).
    pub fn medium_shape() -> Arc<dyn Shape> {
        Arc::new(ShapeDefaults::LargeIncreased()) // TODO: update to use token
    }

    /// A large FAB's shape, the theme's extra large corners (Compose's `largeShape`).
    pub fn large_shape() -> Arc<dyn Shape> {
        FabLargeTokens::ContainerShape.value()
    }

    /// An extended FAB's shape, the theme's large corners (Compose's `extendedFabShape`).
    pub fn extended_fab_shape() -> Arc<dyn Shape> {
        ExtendedFabPrimaryTokens::ContainerShape.value()
    }

    /// A small extended FAB's shape (Compose's Expressive `smallExtendedFabShape`).
    pub fn small_extended_fab_shape() -> Arc<dyn Shape> {
        ExtendedFabSmallTokens::ContainerShape.value()
    }

    /// A medium extended FAB's shape, 20 dp corners (Compose's Expressive `mediumExtendedFabShape`).
    pub fn medium_extended_fab_shape() -> Arc<dyn Shape> {
        Arc::new(ShapeDefaults::LargeIncreased()) // TODO: update to use token
    }

    /// A large extended FAB's shape (Compose's Expressive `largeExtendedFabShape`).
    pub fn large_extended_fab_shape() -> Arc<dyn Shape> {
        ExtendedFabLargeTokens::ContainerShape.value()
    }

    /// A FAB's container color, the scheme's primary container (Compose's `containerColor`).
    pub fn container_color() -> Color {
        FabPrimaryContainerTokens::ContainerColor.value()
    }
}

#[named]
impl FloatingActionButtonDefaults {
    /// A FAB's elevation (Compose's `elevation`): 6 dp, 8 hovered.
    pub fn elevation(
        &self,
        #[default(FabPrimaryContainerTokens::ContainerElevation)] default_elevation: Dp,
        #[default(FabPrimaryContainerTokens::PressedContainerElevation)] pressed_elevation: Dp,
        #[default(FabPrimaryContainerTokens::FocusedContainerElevation)] focused_elevation: Dp,
        #[default(FabPrimaryContainerTokens::HoveredContainerElevation)] hovered_elevation: Dp,
    ) -> FloatingActionButtonElevation {
        FloatingActionButtonElevation::new(default_elevation, pressed_elevation, focused_elevation, hovered_elevation)
    }

    /// A lowered FAB's elevation (Compose's `loweredElevation`): 1 dp, 3 hovered — for a FAB on a surface already
    /// raised (a bottom app bar).
    pub fn lowered_elevation(
        &self,
        #[default(ElevationTokens::Level1)] default_elevation: Dp,
        #[default(ElevationTokens::Level1)] pressed_elevation: Dp,
        #[default(ElevationTokens::Level1)] focused_elevation: Dp,
        #[default(ElevationTokens::Level2)] hovered_elevation: Dp,
    ) -> FloatingActionButtonElevation {
        FloatingActionButtonElevation::new(default_elevation, pressed_elevation, focused_elevation, hovered_elevation)
    }

    /// A FAB's elevation in a bottom app bar (Compose's `bottomAppBarFabElevation`): flat in every state.
    pub fn bottom_app_bar_fab_elevation(
        &self,
        #[default(Dp(0.0))] default_elevation: Dp,
        #[default(Dp(0.0))] pressed_elevation: Dp,
        #[default(Dp(0.0))] focused_elevation: Dp,
        #[default(Dp(0.0))] hovered_elevation: Dp,
    ) -> FloatingActionButtonElevation {
        FloatingActionButtonElevation::new(default_elevation, pressed_elevation, focused_elevation, hovered_elevation)
    }
}

// ---- animateFloatingActionButton -------------------------------------------------------------------------------

/// Showing and hiding a FAB (Compose's Expressive `Modifier.animateFloatingActionButton`).
#[named(trait FloatingActionButtonModifiers)]
impl Modifier {
    /// The FAB shown or hidden as `visible` says, animated: scaled from (and to) `target_scale` towards
    /// `alignment` (a FAB at the bottom end shrinks into that corner) along `scale_animation_spec` (none: the
    /// theme's fast spatial spec), faded along `alpha_animation_spec` (none: the fast effects spec); hidden, it
    /// takes no room (Compose's `Modifier.animateFloatingActionButton`). Drawn in a layer 16 dp larger each way,
    /// so its shadow isn't cut.
    fn animate_floating_action_button(
        self,
        visible: bool,
        alignment: Alignment,
        #[default(FloatingActionButtonDefaults::ShowHideTargetScale)] target_scale: f32,
        #[default(None)] scale_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        #[default(None)] alpha_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    ) -> Modifier {
        self.then_element(FabVisibleModifier {
            visible,
            alignment,
            target_scale,
            scale_animation_spec,
            alpha_animation_spec,
        })
    }

    /// [`animate_floating_action_button`](FloatingActionButtonModifiers::animate_floating_action_button) with all
    /// its parameters, for plain code.
    fn animate_floating_action_button_with(
        self,
        visible: bool,
        alignment: Alignment,
        target_scale: f32,
        scale_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        alpha_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    ) -> Modifier {
        self.then_element(FabVisibleModifier {
            visible,
            alignment,
            target_scale,
            scale_animation_spec,
            alpha_animation_spec,
        })
    }
}

/// The element of `animate_floating_action_button` (Compose's internal data class `FabVisibleModifier`).
struct FabVisibleModifier {
    visible: bool,
    alignment: Alignment,
    target_scale: f32,
    scale_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    alpha_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
}

/// Two optional specs equal (Kotlin's `==` of nullable specs).
fn same_spec(a: &Option<Arc<dyn AnimationSpec<f32>>>, b: &Option<Arc<dyn AnimationSpec<f32>>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => **a == **b,
        (None, None) => true,
        _ => false,
    }
}

impl PartialEq for FabVisibleModifier {
    fn eq(&self, other: &Self) -> bool {
        self.visible == other.visible
            && self.alignment == other.alignment
            && self.target_scale == other.target_scale
            && same_spec(&self.scale_animation_spec, &other.scale_animation_spec)
            && same_spec(&self.alpha_animation_spec, &other.alpha_animation_spec)
    }
}

impl ModifierNodeElement for FabVisibleModifier {
    type Node = FabVisibleNode;
    fn create(&self) -> FabVisibleNode {
        FabVisibleNode::new(
            self.visible,
            self.alignment.clone(),
            self.target_scale,
            self.scale_animation_spec.clone(),
            self.alpha_animation_spec.clone(),
        )
    }
    fn update(&self, node: &mut FabVisibleNode) {
        node.update_node(
            self.visible,
            self.alignment.clone(),
            self.target_scale,
            self.scale_animation_spec.clone(),
            self.alpha_animation_spec.clone(),
        );
    }
    fn name(&self) -> &'static str {
        // (Compose shows nothing in the inspector)
        "animateFloatingActionButton"
    }
}

/// What the node's draw cache reads of it as it changes: the alignment and the target scale.
struct FabVisibleConfig {
    alignment: RefCell<Alignment>,
    target_scale: Cell<f32>,
}

/// The node of `animate_floating_action_button` (Compose's internal `FabVisibleNode`): a scale and an alpha
/// animated to 1 (visible) or 0, the content drawn through a layer of them (its draw cache's, delegated), measured
/// to nothing while fully transparent.
pub(crate) struct FabVisibleNode {
    config: Rc<FabVisibleConfig>,
    scale_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    alpha_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    scale_animatable: Animatable<f32, AnimationVector1D>,
    alpha_animatable: Animatable<f32, AnimationVector1D>,
    handle: Option<NodeHandle>,
    cache: Option<DelegatedNode<CacheDrawModifierNode>>,
}

impl FabVisibleNode {
    fn new(
        visible: bool,
        alignment: Alignment,
        target_scale: f32,
        scale_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        alpha_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    ) -> FabVisibleNode {
        let initial = if visible { 1.0 } else { 0.0 };
        FabVisibleNode {
            config: Rc::new(FabVisibleConfig {
                alignment: RefCell::new(alignment),
                target_scale: Cell::new(target_scale),
            }),
            scale_animation_spec,
            alpha_animation_spec,
            scale_animatable: Animatable(initial),
            alpha_animatable: Animatable(initial),
            handle: None,
            cache: None,
        }
    }

    /// Compose's `updateNode`: the new values kept, the scale and the alpha animated to the visibility given.
    fn update_node(
        &mut self,
        visible: bool,
        alignment: Alignment,
        target_scale: f32,
        scale_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        alpha_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    ) {
        *self.config.alignment.borrow_mut() = alignment;
        self.config.target_scale.set(target_scale);
        self.scale_animation_spec = scale_animation_spec;
        self.alpha_animation_spec = alpha_animation_spec;

        let Some(handle) = self.handle.as_ref().filter(|h| h.is_attached()) else { return };
        let Some(scope) = handle.coroutine_scope() else { return };
        let target = if visible { 1.0 } else { 0.0 };
        let motion_scheme = handle.current_value_of(MaterialTheme::LocalMaterialTheme()).motion_scheme;

        // TODO Load the motionScheme tokens from the component tokens file
        let scale_spec: Arc<dyn AnimationSpec<f32>> = match &self.scale_animation_spec {
            Some(spec) => spec.clone(),
            None => motion_scheme.fast_spatial_spec::<f32>(),
        };
        let scale = self.scale_animatable.clone();
        scope.launch(async move {
            named_args!(scale.animate_to(target, animation_spec = scale_spec)).await;
        });

        // TODO Load the motionScheme tokens from the component tokens file
        let alpha_spec: Arc<dyn AnimationSpec<f32>> = match &self.alpha_animation_spec {
            Some(spec) => spec.clone(),
            None => motion_scheme.fast_effects_spec::<f32>(),
        };
        let alpha = self.alpha_animatable.clone();
        scope.launch(async move {
            named_args!(alpha.animate_to(target, animation_spec = alpha_spec)).await;
        });
    }

    /// The draw cache (Compose's delegated `CacheDrawModifierNode` of `init`): the content recorded into a layer
    /// 16 dp larger each way (so the shadow isn't cut), moved back by that, its alpha and scale (about the
    /// alignment's point) the animated ones.
    fn draw_cache(&self) -> CacheDrawModifierNode {
        let config = self.config.clone();
        let (scale, alpha) = (self.scale_animatable.clone(), self.alpha_animatable.clone());
        CacheDrawModifierNode(move |scope: &mut CacheDrawScope| {
            let layer = scope.obtain_graphics_layer();
            // Use a larger layer size to make sure the elevation shadow doesn't get clipped
            // and offset via layer.topLeft and DrawScope.inset to preserve the visual
            // position of the FAB.
            let layer_inset_size = scope.to_px(Dp(16.0));
            let size = scope.size();
            let layer_size = IntSize::new(
                (size.width + layer_inset_size * 2.0) as i32,
                (size.height + layer_inset_size * 2.0) as i32,
            );
            let node_size = IntSize::new(size.width as i32, size.height as i32);

            let inset = round_to_int(layer_inset_size);
            layer.set_top_left(IntOffset::new(-inset, -inset));
            layer.set_alpha(alpha.value());

            // Scale towards the direction of the provided alignment
            let align_offset = config.alignment.borrow().align(IntSize::new(1, 1), node_size, scope.layout_direction());
            layer.set_pivot_offset(Offset::new(
                align_offset.x as f32 + layer_inset_size,
                align_offset.y as f32 + layer_inset_size,
            ));
            let s = lerp(config.target_scale.get(), 1.0, scale.value());
            layer.set_scale_x(s);
            layer.set_scale_y(s);

            scope.record_sized(&layer, layer_size, |c: &mut ContentDrawScope<'_>| {
                c.inset(layer_inset_size, layer_inset_size, layer_inset_size, layer_inset_size, |c| c.draw_content());
            });

            scope.on_draw_with_content(move |c| c.draw_layer(&layer))
        })
    }
}

impl ModifierNode for FabVisibleNode {
    node_capabilities!(layout);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        // (Compose delegates as it is made; a node has its context once attached)
        if self.cache.is_none() {
            self.cache = Some(cx.delegate(self.draw_cache()));
        }
    }
}

impl LayoutModifierNode for FabVisibleNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        if self.alpha_animatable.value() == 0.0 {
            return scope.layout(0, 0, |_| {});
        }
        let placeable = measurable.measure(constraints);
        scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0))
    }
}

// ---- FloatingActionButtonElevation -----------------------------------------------------------------------------

/// A FAB's elevation in its states (Compose's `FloatingActionButtonElevation`): its default one is its tonal
/// elevation too; its shadow animates to the pressed, hovered or focused one as those go on. Made by
/// [`FloatingActionButtonDefaults`]' elevation functions; equal by its elevations. (Compose's class is open: it
/// can't be subclassed here.)
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FloatingActionButtonElevation {
    default_elevation: Dp,
    pressed_elevation: Dp,
    focused_elevation: Dp,
    hovered_elevation: Dp,
}

impl FloatingActionButtonElevation {
    /// Compose's internal constructor.
    pub(crate) fn new(
        default_elevation: Dp,
        pressed_elevation: Dp,
        focused_elevation: Dp,
        hovered_elevation: Dp,
    ) -> FloatingActionButtonElevation {
        FloatingActionButtonElevation { default_elevation, pressed_elevation, focused_elevation, hovered_elevation }
    }

    /// The shadow's elevation as state (Compose's internal composable `shadowElevation`): that of the last of
    /// `interaction_source`'s interactions going on, animated to as each comes and goes.
    #[track_caller]
    pub(crate) fn shadow_elevation(&self, interaction_source: &InteractionSource) -> State<Dp> {
        self.animate_elevation(interaction_source)
    }

    /// The tonal elevation: the default one (Compose's internal `tonalElevation`).
    pub(crate) fn tonal_elevation(&self) -> Dp {
        self.default_elevation
    }

    /// Compose's private `animateElevation`.
    #[track_caller]
    fn animate_elevation(&self, interaction_source: &InteractionSource) -> State<Dp> {
        let this = *self;
        let interaction_source = interaction_source.clone();
        kompozable_block!({
            let animatable = remember_keyed(interaction_source.clone(), || {
                Rc::new(FloatingActionButtonElevationAnimatable::new(
                    this.default_elevation,
                    this.pressed_elevation,
                    this.hovered_elevation,
                    this.focused_elevation,
                ))
            });

            let updated = animatable.clone();
            LaunchedEffect(this, async move {
                updated
                    .update_elevation(
                        this.default_elevation,
                        this.pressed_elevation,
                        this.hovered_elevation,
                        this.focused_elevation,
                    )
                    .await;
            });

            let collecting = collect_interactions(interaction_source.clone(), animatable.clone());
            LaunchedEffect(interaction_source.clone(), collecting);

            animatable.as_state()
        })
    }
}

/// Collects `interaction_source`'s interactions, animating `animatable` to the last one going on as each comes and
/// goes, each animation launched on its own (Compose's `LaunchedEffect(interactionSource)` body of
/// `animateElevation`).
fn collect_interactions(
    interaction_source: InteractionSource,
    animatable: Rc<FloatingActionButtonElevationAnimatable>,
) -> impl Future<Output = ()> + 'static {
    coroutine_scope(move |scope| async move {
        let mut interactions: Vec<Arc<dyn Interaction>> = Vec::new();
        interaction_source
            .interactions()
            .collect(move |interaction| {
                if interaction.is::<HoverInteraction::Enter>()
                    || interaction.is::<FocusInteraction::Focus>()
                    || interaction.is::<PressInteraction::Press>()
                {
                    interactions.push(interaction);
                } else {
                    crate::internal::interactions::remove_ended(&mut interactions, &*interaction);
                }
                let target_interaction = interactions.last().cloned();
                scope.launch(animatable.animate_elevation(target_interaction));
            })
            .await;
    })
}

/// A FAB's shadow elevation and what it animates by (Compose's private `FloatingActionButtonElevationAnimatable`):
/// the elevations of the states, the interaction it goes to and the one it last went to.
struct FloatingActionButtonElevationAnimatable {
    default_elevation: Cell<Dp>,
    pressed_elevation: Cell<Dp>,
    hovered_elevation: Cell<Dp>,
    focused_elevation: Cell<Dp>,
    animatable: Animatable<Dp, AnimationVector1D>,
    last_target_interaction: RefCell<Option<Arc<dyn Interaction>>>,
    target_interaction: RefCell<Option<Arc<dyn Interaction>>>,
}

/// Runs its closure when dropped: Kotlin's `finally` (a coroutine cancelled runs it too).
struct Finally<F: FnMut()>(F);

impl<F: FnMut()> Drop for Finally<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

impl FloatingActionButtonElevationAnimatable {
    fn new(
        default_elevation: Dp,
        pressed_elevation: Dp,
        hovered_elevation: Dp,
        focused_elevation: Dp,
    ) -> FloatingActionButtonElevationAnimatable {
        FloatingActionButtonElevationAnimatable {
            default_elevation: Cell::new(default_elevation),
            pressed_elevation: Cell::new(pressed_elevation),
            hovered_elevation: Cell::new(hovered_elevation),
            focused_elevation: Cell::new(focused_elevation),
            animatable: Animatable((default_elevation, Dp::VectorConverter)),
            last_target_interaction: RefCell::new(None),
            target_interaction: RefCell::new(None),
        }
    }

    /// The elevation of an interaction (Compose's `Interaction?.calculateTarget`).
    fn calculate_target(&self, interaction: Option<&dyn Interaction>) -> Dp {
        match interaction {
            Some(i) if i.is::<PressInteraction::Press>() => self.pressed_elevation.get(),
            Some(i) if i.is::<HoverInteraction::Enter>() => self.hovered_elevation.get(),
            Some(i) if i.is::<FocusInteraction::Focus>() => self.focused_elevation.get(),
            _ => self.default_elevation.get(),
        }
    }

    /// New elevations, snapped to (Compose's suspending `updateElevation`).
    fn update_elevation(
        self: &Rc<Self>,
        default_elevation: Dp,
        pressed_elevation: Dp,
        hovered_elevation: Dp,
        focused_elevation: Dp,
    ) -> impl Future<Output = ()> + 'static {
        let this = self.clone();
        async move {
            this.default_elevation.set(default_elevation);
            this.pressed_elevation.set(pressed_elevation);
            this.hovered_elevation.set(hovered_elevation);
            this.focused_elevation.set(focused_elevation);
            this.snap_elevation().await;
        }
    }

    /// Snaps to the target interaction's elevation (Compose's private suspending `snapElevation`).
    async fn snap_elevation(self: Rc<Self>) {
        let target = self.calculate_target(self.target_interaction.borrow().as_deref());
        if self.animatable.target_value() != target {
            let this = self.clone();
            let _finally = Finally(move || {
                let target_interaction = this.target_interaction.borrow().clone();
                *this.last_target_interaction.borrow_mut() = target_interaction;
            });
            self.animatable.snap_to(target).await;
        }
    }

    /// Animates to `to`'s elevation from the last interaction's, as [`AnimateElevation`] says (Compose's suspending
    /// `animateElevation`).
    fn animate_elevation(self: &Rc<Self>, to: Option<Arc<dyn Interaction>>) -> impl Future<Output = ()> + 'static {
        let this = self.clone();
        async move {
            let target = this.calculate_target(to.as_deref());
            // Update the interaction even if the values are the same, for when we change to another
            // interaction later
            *this.target_interaction.borrow_mut() = to.clone();
            let (finished, last) = (this.clone(), to.clone());
            let _finally = Finally(move || *finished.last_target_interaction.borrow_mut() = last.clone());
            if this.animatable.target_value() != target {
                let from = this.last_target_interaction.borrow().clone();
                this.animatable.animate_elevation(target, from.as_deref(), to.as_deref()).await;
            }
        }
    }

    /// Its elevation as state (Compose's `asState`).
    fn as_state(&self) -> State<Dp> {
        self.animatable.as_state()
    }
}

// ---- The private values ----------------------------------------------------------------------------------------

const SmallExtendedFabMinimumWidth: Dp = ExtendedFabSmallTokens::ContainerHeight;
const SmallExtendedFabMinimumHeight: Dp = ExtendedFabSmallTokens::ContainerHeight;
const SmallExtendedFabPaddingStart: Dp = ExtendedFabSmallTokens::LeadingSpace;
const SmallExtendedFabPaddingEnd: Dp = ExtendedFabSmallTokens::TrailingSpace;
const SmallExtendedFabIconPadding: Dp = ExtendedFabSmallTokens::IconLabelSpace;
const SmallExtendedFabTextStyle: TypographyKeyTokens = TypographyKeyTokens::TitleMedium;

const MediumExtendedFabMinimumWidth: Dp = ExtendedFabMediumTokens::ContainerHeight;
const MediumExtendedFabMinimumHeight: Dp = ExtendedFabMediumTokens::ContainerHeight;
const MediumExtendedFabPaddingStart: Dp = ExtendedFabMediumTokens::LeadingSpace;
const MediumExtendedFabPaddingEnd: Dp = ExtendedFabMediumTokens::TrailingSpace;
// TODO: ExtendedFabMediumTokens.IconLabelSpace is incorrect
const MediumExtendedFabIconPadding: Dp = Dp(12.0);
const MediumExtendedFabTextStyle: TypographyKeyTokens = TypographyKeyTokens::TitleLarge;

const LargeExtendedFabMinimumWidth: Dp = ExtendedFabLargeTokens::ContainerHeight;
const LargeExtendedFabMinimumHeight: Dp = ExtendedFabLargeTokens::ContainerHeight;
const LargeExtendedFabPaddingStart: Dp = ExtendedFabLargeTokens::LeadingSpace;
const LargeExtendedFabPaddingEnd: Dp = ExtendedFabLargeTokens::TrailingSpace;
// TODO: ExtendedFabLargeTokens.IconLabelSpace is incorrect
const LargeExtendedFabIconPadding: Dp = Dp(16.0);
const LargeExtendedFabTextStyle: TypographyKeyTokens = TypographyKeyTokens::HeadlineSmall;

const ExtendedFabStartIconPadding: Dp = Dp(16.0);

const ExtendedFabEndIconPadding: Dp = Dp(12.0);

const ExtendedFabTextPadding: Dp = Dp(20.0);

const ExtendedFabMinimumWidth: Dp = Dp(80.0);

/// The label leaving a collapsing extended FAB (Compose's private `extendedFabCollapseAnimation`): fading out along
/// the theme's fast effects spec, shrinking towards the start along its default spatial one.
fn extended_fab_collapse_animation() -> ExitTransition {
    // TODO Load the motionScheme tokens from the component tokens file
    named_args!(fade_out(animation_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>()))
        + named_args!(shrink_horizontally(
            animation_spec = MotionSchemeKeyTokens::DefaultSpatial.value::<IntSize>(),
            shrink_towards = Alignment::Start,
        ))
}

/// The label coming into an expanding extended FAB (Compose's private `extendedFabExpandAnimation`): fading in
/// along the theme's default effects spec, growing from the start along its fast spatial one.
fn extended_fab_expand_animation() -> EnterTransition {
    // TODO Load the motionScheme tokens from the component tokens file
    named_args!(fade_in(animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>()))
        + named_args!(expand_horizontally(
            animation_spec = MotionSchemeKeyTokens::FastSpatial.value::<IntSize>(),
            expand_from = Alignment::Start,
        ))
}

/// Kotlin's `lerp(start: Int, stop: Int, fraction: Float)`: `start + ((stop - start) * fraction).roundToInt()`.
fn lerp_int(start: i32, stop: i32, fraction: f32) -> i32 {
    start + round_to_int((stop - start) as f32 * fraction)
}

/// Kotlin's `roundToInt` (half up; NaN fails there, 0 here).
fn round_to_int(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

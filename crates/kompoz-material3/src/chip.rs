//! Material's chips, as Compose's Chip.kt: compact elements of a label between an optional leading icon and an
//! optional trailing one, 32 dp tall, 8 dp round corners — assist chips ([`AssistChip`], [`ElevatedAssistChip`]:
//! smart or automated actions), filter chips ([`FilterChip`], [`ElevatedFilterChip`]: selected to narrow content,
//! their leading icon coming and going animated), input chips ([`InputChip`]: a piece of what the user put in,
//! with an avatar or a leading icon) and suggestion chips ([`SuggestionChip`], [`ElevatedSuggestionChip`]: a
//! suggestion to pick). Flat ones are outlined, elevated ones on a low surface container with a shadow; selected
//! ones on the secondary container. Their colors, elevations and borders per state are [`ChipColors`] /
//! [`SelectableChipColors`], [`ChipElevation`] / [`SelectableChipElevation`] and `BorderStroke`s from their
//! defaults objects.
//!
//! Compose's overloads kept for binary compatibility (hidden from Kotlin source) are composables of their own here,
//! deprecated: `…WithoutArrangement` (Kotlin's overloads without `horizontalArrangement` and `contentPadding`) and
//! `…WithChipBorder` (those taking the deprecated [`ChipBorder`]).

// (the deprecated ChipBorder's own items name it)
#![allow(deprecated)]

use crate::card::track_interaction;
use crate::color_scheme::ColorScheme;
use crate::content_color::LocalContentColor;
use crate::internal::elevation::AnimateElevation;
use crate::material_theme::MaterialTheme;
use crate::surface::{ClickableSurface, SelectableSurface};
use crate::text::LocalTextStyle;
use crate::tokens::{AssistChipTokens, FilterChipTokens, InputChipTokens, MotionSchemeKeyTokens, SuggestionChipTokens};
use kompoz_animation::{
    Animatable, AnimationVector1D, RowAnimatedVisibility, VectorConverter, expand_horizontally, fade_in, fade_out,
    shrink_horizontally,
};
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::interaction::{
    DragInteraction, FocusInteraction, HoverInteraction, Interaction, InteractionSource, MutableInteractionSource,
    PressInteraction,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{BorderStroke, Color, Dp, IntSize, LayoutDirection, Shape};
use kompoz_runtime::{
    KompozitionLocalProvider, KotlinProperties, State, UpdatedState, kompozable, kompozable_block,
    mutable_state_list_of, named, named_args, remember, remember_updated_state,
};
use kompoz_text::TextStyle;
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

// ---- Assist chips ----------------------------------------------------------------------------------------------

/// An assist chip (Compose's `AssistChip`): a smart or automated action — `label` (in the type scale's label
/// large) between `leading_icon` and `trailing_icon` (18 dp, primary), on a clickable surface of `shape` in
/// `colors`, at least 32 dp tall, padded by `content_padding` (8 dp at its sides), the parts placed by
/// `horizontal_arrangement` (8 dp apart). `border` is Kotlin's default `AssistChipDefaults.assistChipBorder(enabled)`
/// where `None` (`Some(None)`: none); `elevation` none (`None`) or the shadow's states. It is a button to
/// accessibility; not `enabled`, it isn't clicked and takes its disabled colors.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn AssistChip(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(AssistChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(AssistChipDefaults.assist_chip_colors())] colors: ChipColors,
    #[default(Some(AssistChipDefaults.assist_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(AssistChipDefaults::horizontal_arrangement())] horizontal_arrangement: HorizontalArrangement,
    #[default(AssistChipDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let border = border.unwrap_or_else(|| Some(AssistChipDefaults.assist_chip_border(enabled)));
    Chip(
        modifier = modifier,
        on_click = on_click,
        enabled = enabled,
        label = label,
        label_text_style = AssistChipTokens::LabelTextFont.value(),
        label_color = colors.label_color_for(enabled),
        leading_icon = leading_icon,
        trailing_icon = trailing_icon,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        min_height = AssistChipDefaults::Height,
        horizontal_arrangement = horizontal_arrangement,
        padding_values = content_padding,
        interaction_source = interaction_source,
    );
}

/// [`AssistChip`] without its arrangement and padding (Compose's hidden overload kept for binary compatibility):
/// the defaults'.
#[deprecated(note = "Maintained for binary compatibility")]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn AssistChipWithoutArrangement(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(AssistChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(AssistChipDefaults.assist_chip_colors())] colors: ChipColors,
    #[default(Some(AssistChipDefaults.assist_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    AssistChip(
        on_click,
        label,
        modifier = modifier,
        enabled = enabled,
        leading_icon = leading_icon,
        trailing_icon = trailing_icon,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        horizontal_arrangement = AssistChipDefaults::horizontal_arrangement(),
        content_padding = AssistChipDefaults::ContentPadding(),
        interaction_source = interaction_source,
    );
}

/// [`AssistChip`] with a [`ChipBorder`] (Compose's hidden overload, "Use version with AssistChip that take a
/// BorderStroke instead"): the parts 8 dp apart, centered.
#[deprecated(
    note = "Maintained for binary compatibility. Use version with AssistChip that take a BorderStroke instead"
)]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn AssistChipWithChipBorder(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(AssistChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(AssistChipDefaults.assist_chip_colors())] colors: ChipColors,
    #[default(Some(AssistChipDefaults.assist_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(Some(AssistChipDefaults.assist_chip_chip_border()))] border: Option<ChipBorder>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let border = border.and_then(|b| b.border_stroke(enabled).value());
    Chip(
        modifier = modifier,
        on_click = on_click,
        enabled = enabled,
        label = label,
        label_text_style = AssistChipTokens::LabelTextFont.value(),
        label_color = colors.label_color_for(enabled),
        leading_icon = leading_icon,
        trailing_icon = trailing_icon,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        min_height = AssistChipDefaults::Height,
        padding_values = AssistChipDefaults::ContentPadding(),
        interaction_source = interaction_source,
    );
}

/// An elevated assist chip (Compose's `ElevatedAssistChip`): an [`AssistChip`] on the low surface container with a
/// shadow of 1 dp (3 hovered), no border by default.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ElevatedAssistChip(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(AssistChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(AssistChipDefaults.elevated_assist_chip_colors())] colors: ChipColors,
    #[default(Some(AssistChipDefaults.elevated_assist_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(AssistChipDefaults::horizontal_arrangement())] horizontal_arrangement: HorizontalArrangement,
    #[default(AssistChipDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    Chip(
        modifier = modifier,
        on_click = on_click,
        enabled = enabled,
        label = label,
        label_text_style = AssistChipTokens::LabelTextFont.value(),
        label_color = colors.label_color_for(enabled),
        leading_icon = leading_icon,
        trailing_icon = trailing_icon,
        elevation = elevation,
        colors = colors,
        min_height = AssistChipDefaults::Height,
        horizontal_arrangement = horizontal_arrangement,
        padding_values = content_padding,
        shape = shape,
        border = border,
        interaction_source = interaction_source,
    );
}

/// [`ElevatedAssistChip`] without its arrangement and padding (Compose's hidden overload kept for binary
/// compatibility): the defaults'.
#[deprecated(note = "Maintained for binary compatibility")]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ElevatedAssistChipWithoutArrangement(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(AssistChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(AssistChipDefaults.elevated_assist_chip_colors())] colors: ChipColors,
    #[default(Some(AssistChipDefaults.elevated_assist_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    ElevatedAssistChip(
        on_click,
        label,
        modifier = modifier,
        enabled = enabled,
        leading_icon = leading_icon,
        trailing_icon = trailing_icon,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        horizontal_arrangement = AssistChipDefaults::horizontal_arrangement(),
        content_padding = AssistChipDefaults::ContentPadding(),
        interaction_source = interaction_source,
    );
}

/// [`ElevatedAssistChip`] with a [`ChipBorder`] (Compose's hidden overload, "Use version with ElevatedAssistChip
/// that take a BorderStroke instead"): the parts 8 dp apart, centered.
#[deprecated(
    note = "Maintained for binary compatibility. Use version with ElevatedAssistChip that take a BorderStroke instead"
)]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ElevatedAssistChipWithChipBorder(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(AssistChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(AssistChipDefaults.elevated_assist_chip_colors())] colors: ChipColors,
    #[default(Some(AssistChipDefaults.elevated_assist_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(None)] border: Option<ChipBorder>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let border = border.and_then(|b| b.border_stroke(enabled).value());
    Chip(
        modifier = modifier,
        on_click = on_click,
        enabled = enabled,
        label = label,
        label_text_style = AssistChipTokens::LabelTextFont.value(),
        label_color = colors.label_color_for(enabled),
        leading_icon = leading_icon,
        trailing_icon = trailing_icon,
        elevation = elevation,
        colors = colors,
        min_height = AssistChipDefaults::Height,
        padding_values = AssistChipDefaults::ContentPadding(),
        shape = shape,
        border = border,
        interaction_source = interaction_source,
    );
}

// ---- Filter chips ----------------------------------------------------------------------------------------------

/// A filter chip (Compose's `FilterChip`): selected or not to narrow content — as an [`AssistChip`], selectable
/// (`selected` told to accessibility, a checkbox to it), on the secondary container when `selected`, outlined when
/// not (`border`: Kotlin's default `FilterChipDefaults.filterChipBorder(enabled, selected)` where `None`,
/// `Some(None)` none); its leading and trailing icons come in and go out animated (expanding from the start and
/// fading in, shrinking and fading out), the chip at most 1000 dp wide.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn FilterChip(
    selected: bool,
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(FilterChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(FilterChipDefaults.filter_chip_colors())] colors: SelectableChipColors,
    #[default(Some(FilterChipDefaults.filter_chip_elevation()))] elevation: Option<SelectableChipElevation>,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(FilterChipDefaults::horizontal_arrangement())] horizontal_arrangement: HorizontalArrangement,
    #[default(FilterChipDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let border = border.unwrap_or_else(|| Some(FilterChipDefaults.filter_chip_border(enabled, selected)));
    SelectableChip(
        selected = selected,
        modifier = modifier,
        on_click = on_click,
        enabled = enabled,
        label = label,
        label_text_style = FilterChipTokens::LabelTextFont.value(),
        leading_icon = leading_icon,
        avatar = None,
        trailing_icon = trailing_icon,
        elevation = elevation,
        colors = colors,
        min_height = FilterChipDefaults::Height,
        horizontal_arrangement = horizontal_arrangement,
        padding_values = content_padding,
        shape = shape,
        border = border,
        interaction_source = interaction_source,
    );
}

/// [`FilterChip`] without its arrangement and padding (Compose's hidden overload kept for binary compatibility):
/// the defaults'.
#[deprecated(note = "Maintained for binary compatibility")]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn FilterChipWithoutArrangement(
    selected: bool,
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(FilterChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(FilterChipDefaults.filter_chip_colors())] colors: SelectableChipColors,
    #[default(Some(FilterChipDefaults.filter_chip_elevation()))] elevation: Option<SelectableChipElevation>,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    FilterChip(
        selected,
        on_click,
        label,
        modifier = modifier,
        enabled = enabled,
        leading_icon = leading_icon,
        trailing_icon = trailing_icon,
        elevation = elevation,
        colors = colors,
        content_padding = FilterChipDefaults::ContentPadding(),
        shape = shape,
        border = border,
        interaction_source = interaction_source,
    );
}

/// An elevated filter chip (Compose's `ElevatedFilterChip`): a [`FilterChip`] on the low surface container (the
/// secondary container selected) with a shadow of 1 dp (3 hovered), no border by default.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ElevatedFilterChip(
    selected: bool,
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(FilterChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(FilterChipDefaults.elevated_filter_chip_colors())] colors: SelectableChipColors,
    #[default(Some(FilterChipDefaults.elevated_filter_chip_elevation()))] elevation: Option<SelectableChipElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(FilterChipDefaults::horizontal_arrangement())] horizontal_arrangement: HorizontalArrangement,
    #[default(FilterChipDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    SelectableChip(
        selected = selected,
        modifier = modifier,
        on_click = on_click,
        enabled = enabled,
        label = label,
        label_text_style = FilterChipTokens::LabelTextFont.value(),
        leading_icon = leading_icon,
        avatar = None,
        trailing_icon = trailing_icon,
        elevation = elevation,
        colors = colors,
        min_height = FilterChipDefaults::Height,
        horizontal_arrangement = horizontal_arrangement,
        padding_values = content_padding,
        shape = shape,
        border = border,
        interaction_source = interaction_source,
    );
}

/// [`ElevatedFilterChip`] without its arrangement and padding (Compose's hidden overload kept for binary
/// compatibility): the defaults'.
#[deprecated(note = "Maintained for binary compatibility")]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ElevatedFilterChipWithoutArrangement(
    selected: bool,
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(FilterChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(FilterChipDefaults.elevated_filter_chip_colors())] colors: SelectableChipColors,
    #[default(Some(FilterChipDefaults.elevated_filter_chip_elevation()))] elevation: Option<SelectableChipElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    ElevatedFilterChip(
        selected,
        on_click,
        label,
        modifier = modifier,
        enabled = enabled,
        leading_icon = leading_icon,
        trailing_icon = trailing_icon,
        elevation = elevation,
        colors = colors,
        content_padding = FilterChipDefaults::ContentPadding(),
        shape = shape,
        border = border,
        interaction_source = interaction_source,
    );
}

// ---- Input chips -----------------------------------------------------------------------------------------------

/// An input chip (Compose's `InputChip`): a piece of what the user put in — as a [`FilterChip`], with `avatar` (in
/// a circle, at 38% disabled; it takes the leading icon's place) before the label; padded by `content_padding`
/// (Kotlin's default `InputChipDefaults.contentPadding(avatar != null, leadingIcon != null, trailingIcon != null)`,
/// here `None`: 4 dp at the start with an avatar or without a leading icon, else 8; 8 dp at the end with a
/// trailing icon, else 4); `border` Kotlin's default `InputChipDefaults.inputChipBorder(enabled, selected)` where
/// `None` (`Some(None)`: none).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn InputChip(
    selected: bool,
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] avatar: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(InputChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(InputChipDefaults.input_chip_colors())] colors: SelectableChipColors,
    #[default(Some(InputChipDefaults.input_chip_elevation()))] elevation: Option<SelectableChipElevation>,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(InputChipDefaults::horizontal_arrangement())] horizontal_arrangement: HorizontalArrangement,
    #[default(None)] content_padding: Option<PaddingValues>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let border = border.unwrap_or_else(|| Some(InputChipDefaults.input_chip_border(enabled, selected)));
    let content_padding = content_padding.unwrap_or_else(|| {
        InputChipDefaults::content_padding(avatar.is_some(), leading_icon.is_some(), trailing_icon.is_some())
    });
    // If given, place the avatar in an InputChipTokens.AvatarShape shape before passing it into the Chip function.
    let shaped_avatar: Option<Rc<dyn Fn()>> = match avatar {
        Some(avatar) => {
            let avatar_opacity = if enabled { 1.0 } else { InputChipTokens::DisabledAvatarOpacity };
            let avatar_shape = InputChipTokens::AvatarShape.value();
            Some(Rc::new(move || {
                let avatar = avatar.clone();
                let avatar_shape = avatar_shape.clone();
                named_args!(Box(
                    modifier = Modifier.graphics_layer_block(move |l| {
                        l.alpha = avatar_opacity;
                        l.shape = avatar_shape.clone();
                        l.clip = true;
                    }),
                    content_alignment = Alignment::Center,
                    content = move || avatar(),
                ));
            }) as Rc<dyn Fn()>)
        }
        None => None,
    };
    SelectableChip(
        selected = selected,
        modifier = modifier,
        on_click = on_click,
        enabled = enabled,
        label = label,
        label_text_style = InputChipTokens::LabelTextFont.value(),
        leading_icon = leading_icon,
        avatar = shaped_avatar,
        trailing_icon = trailing_icon,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        min_height = InputChipDefaults::Height,
        horizontal_arrangement = horizontal_arrangement,
        padding_values = content_padding,
        interaction_source = interaction_source,
    );
}

/// [`InputChip`] without its arrangement and padding (Compose's hidden overload kept for binary compatibility):
/// the defaults'. (Compose's body also shapes the avatar, then passes the one given: [`InputChip`] shapes it.)
#[deprecated(note = "Maintained for binary compatibility")]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn InputChipWithoutArrangement(
    selected: bool,
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] avatar: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(InputChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(InputChipDefaults.input_chip_colors())] colors: SelectableChipColors,
    #[default(Some(InputChipDefaults.input_chip_elevation()))] elevation: Option<SelectableChipElevation>,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let content_padding =
        InputChipDefaults::content_padding(avatar.is_some(), leading_icon.is_some(), trailing_icon.is_some());
    InputChip(
        selected,
        on_click,
        label,
        modifier = modifier,
        enabled = enabled,
        leading_icon = leading_icon,
        avatar = avatar,
        trailing_icon = trailing_icon,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        content_padding = Some(content_padding),
        interaction_source = interaction_source,
    );
}

// ---- Suggestion chips ------------------------------------------------------------------------------------------

/// A suggestion chip (Compose's `SuggestionChip`): a suggestion to pick — as an [`AssistChip`] with `icon` at its
/// start and no trailing icon, its label in the surface variant's text; `border` Kotlin's default
/// `SuggestionChipDefaults.suggestionChipBorder(enabled)` where `None` (`Some(None)`: none).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SuggestionChip(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] icon: Option<Rc<dyn Fn()>>,
    #[default(SuggestionChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(SuggestionChipDefaults.suggestion_chip_colors())] colors: ChipColors,
    #[default(Some(SuggestionChipDefaults.suggestion_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(SuggestionChipDefaults::horizontal_arrangement())] horizontal_arrangement: HorizontalArrangement,
    #[default(SuggestionChipDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let border = border.unwrap_or_else(|| Some(SuggestionChipDefaults.suggestion_chip_border(enabled)));
    Chip(
        modifier = modifier,
        on_click = on_click,
        enabled = enabled,
        label = label,
        label_text_style = SuggestionChipTokens::LabelTextFont.value(),
        label_color = colors.label_color_for(enabled),
        leading_icon = icon,
        trailing_icon = None,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        min_height = SuggestionChipDefaults::Height,
        horizontal_arrangement = horizontal_arrangement,
        padding_values = content_padding,
        interaction_source = interaction_source,
    );
}

/// [`SuggestionChip`] without its arrangement and padding (Compose's hidden overload kept for binary
/// compatibility): the defaults'.
#[deprecated(note = "Maintained for binary compatibility")]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SuggestionChipWithoutArrangement(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] icon: Option<Rc<dyn Fn()>>,
    #[default(SuggestionChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(SuggestionChipDefaults.suggestion_chip_colors())] colors: ChipColors,
    #[default(Some(SuggestionChipDefaults.suggestion_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(None)] border: Option<Option<BorderStroke>>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    SuggestionChip(
        on_click,
        label,
        modifier = modifier,
        enabled = enabled,
        icon = icon,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        horizontal_arrangement = SuggestionChipDefaults::horizontal_arrangement(),
        content_padding = SuggestionChipDefaults::ContentPadding(),
        interaction_source = interaction_source,
    );
}

/// [`SuggestionChip`] with a [`ChipBorder`] (Compose's hidden overload, "Use version with SuggestionChip that take
/// a BorderStroke instead"): the parts 8 dp apart, centered.
#[deprecated(
    note = "Maintained for binary compatibility. Use version with SuggestionChip that take a BorderStroke instead"
)]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SuggestionChipWithChipBorder(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] icon: Option<Rc<dyn Fn()>>,
    #[default(SuggestionChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(SuggestionChipDefaults.suggestion_chip_colors())] colors: ChipColors,
    #[default(Some(SuggestionChipDefaults.suggestion_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(Some(SuggestionChipDefaults.suggestion_chip_chip_border()))] border: Option<ChipBorder>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let border = border.and_then(|b| b.border_stroke(enabled).value());
    Chip(
        modifier = modifier,
        on_click = on_click,
        enabled = enabled,
        label = label,
        label_text_style = SuggestionChipTokens::LabelTextFont.value(),
        label_color = colors.label_color_for(enabled),
        leading_icon = icon,
        trailing_icon = None,
        shape = shape,
        colors = colors,
        elevation = elevation,
        border = border,
        min_height = SuggestionChipDefaults::Height,
        padding_values = SuggestionChipDefaults::ContentPadding(),
        interaction_source = interaction_source,
    );
}

/// An elevated suggestion chip (Compose's `ElevatedSuggestionChip`): a [`SuggestionChip`] on the low surface
/// container with a shadow of 1 dp (3 hovered), no border by default.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ElevatedSuggestionChip(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] icon: Option<Rc<dyn Fn()>>,
    #[default(SuggestionChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(SuggestionChipDefaults.elevated_suggestion_chip_colors())] colors: ChipColors,
    #[default(Some(SuggestionChipDefaults.elevated_suggestion_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(SuggestionChipDefaults::horizontal_arrangement())] horizontal_arrangement: HorizontalArrangement,
    #[default(SuggestionChipDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    Chip(
        modifier = modifier,
        on_click = on_click,
        enabled = enabled,
        label = label,
        label_text_style = SuggestionChipTokens::LabelTextFont.value(),
        label_color = colors.label_color_for(enabled),
        leading_icon = icon,
        trailing_icon = None,
        elevation = elevation,
        colors = colors,
        min_height = SuggestionChipDefaults::Height,
        horizontal_arrangement = horizontal_arrangement,
        padding_values = content_padding,
        shape = shape,
        border = border,
        interaction_source = interaction_source,
    );
}

/// [`ElevatedSuggestionChip`] without its arrangement and padding (Compose's hidden overload kept for binary
/// compatibility): the defaults'.
#[deprecated(note = "Maintained for binary compatibility")]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ElevatedSuggestionChipWithoutArrangement(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] icon: Option<Rc<dyn Fn()>>,
    #[default(SuggestionChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(SuggestionChipDefaults.elevated_suggestion_chip_colors())] colors: ChipColors,
    #[default(Some(SuggestionChipDefaults.elevated_suggestion_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(None)] border: Option<BorderStroke>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    ElevatedSuggestionChip(
        on_click,
        label,
        modifier = modifier,
        enabled = enabled,
        icon = icon,
        elevation = elevation,
        colors = colors,
        horizontal_arrangement = SuggestionChipDefaults::horizontal_arrangement(),
        content_padding = SuggestionChipDefaults::ContentPadding(),
        shape = shape,
        border = border,
        interaction_source = interaction_source,
    );
}

/// [`ElevatedSuggestionChip`] with a [`ChipBorder`] (Compose's hidden overload, "Use version with
/// ElevatedSuggestionChip that take a BorderStroke instead"): the parts 8 dp apart, centered.
#[deprecated(
    note = "Maintained for binary compatibility. Use version with ElevatedSuggestionChip that take a BorderStroke \
            instead"
)]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ElevatedSuggestionChipWithChipBorder(
    on_click: impl Fn() + 'static,
    label: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] icon: Option<Rc<dyn Fn()>>,
    #[default(SuggestionChipDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(SuggestionChipDefaults.elevated_suggestion_chip_colors())] colors: ChipColors,
    #[default(Some(SuggestionChipDefaults.elevated_suggestion_chip_elevation()))] elevation: Option<ChipElevation>,
    #[default(None)] border: Option<ChipBorder>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let border = border.and_then(|b| b.border_stroke(enabled).value());
    Chip(
        modifier = modifier,
        on_click = on_click,
        enabled = enabled,
        label = label,
        label_text_style = SuggestionChipTokens::LabelTextFont.value(),
        label_color = colors.label_color_for(enabled),
        leading_icon = icon,
        trailing_icon = None,
        elevation = elevation,
        colors = colors,
        min_height = SuggestionChipDefaults::Height,
        padding_values = SuggestionChipDefaults::ContentPadding(),
        shape = shape,
        border = border,
        interaction_source = interaction_source,
    );
}

// ---- AssistChipDefaults ----------------------------------------------------------------------------------------

/// The assist chips' defaults (Compose's `AssistChipDefaults`): sizes, padding, arrangement, colors, elevations,
/// borders, shape. Its composables are methods of the value, as Kotlin calls them
/// (`AssistChipDefaults.assist_chip_colors()`).
#[derive(Clone, Copy, Debug)]
pub struct AssistChipDefaults;

impl AssistChipDefaults {
    /// An assist chip's height, 32 dp (Compose's `AssistChipDefaults.Height`).
    pub const Height: Dp = AssistChipTokens::ContainerHeight;

    /// Its icons' size, 18 dp (Compose's `AssistChipDefaults.IconSize`).
    pub const IconSize: Dp = AssistChipTokens::IconSize;

    /// The space between its parts, 8 dp (Compose's `AssistChipDefaults.HorizontalSpacing`).
    pub const HorizontalSpacing: Dp = Dp(8.0);

    /// Its padding, 8 dp at its sides (Compose's `AssistChipDefaults.ContentPadding`).
    pub fn ContentPadding() -> PaddingValues {
        PaddingValues::hv(Dp(8.0), Dp(0.0))
    }

    /// Its parts' arrangement (Compose's `AssistChipDefaults.horizontalArrangement()`): the leading part at the
    /// start, the label [`HorizontalSpacing`](Self::HorizontalSpacing) after it, the trailing part at the end
    /// (mirrored right to left).
    pub fn horizontal_arrangement() -> HorizontalArrangement {
        DefaultHorizontalArrangement()
    }

    /// The same, the label `spacing` after the leading part (Compose's `horizontalArrangement(spacing)`).
    ///
    /// Kotlin: `horizontalArrangement(spacing: Dp): Arrangement.Horizontal`
    pub fn horizontal_arrangement_with_spacing(spacing: Dp) -> HorizontalArrangement {
        if spacing == Self::HorizontalSpacing {
            return DefaultHorizontalArrangement();
        }
        ChipArrangement(spacing)
    }

    /// Its shape: the theme's small corners, 8 dp (Compose's `AssistChipDefaults.shape`).
    pub fn shape() -> Arc<dyn Shape> {
        AssistChipTokens::ContainerShape.value()
    }

    /// An assist chip's colors in `scheme` (Compose's internal `ColorScheme.defaultAssistChipColors`): no
    /// container, the label the surface's text, the icons primary; disabled, the surface's text at 38%.
    pub(crate) fn default_assist_chip_colors(scheme: &ColorScheme) -> ChipColors {
        ChipColors::new(
            Color::Transparent,
            scheme.from_token(AssistChipTokens::LabelTextColor),
            scheme.from_token(AssistChipTokens::IconColor),
            scheme.from_token(AssistChipTokens::IconColor),
            Color::Transparent,
            with_alpha(
                scheme.from_token(AssistChipTokens::DisabledLabelTextColor),
                AssistChipTokens::DisabledLabelTextOpacity,
            ),
            with_alpha(scheme.from_token(AssistChipTokens::DisabledIconColor), AssistChipTokens::DisabledIconOpacity),
            with_alpha(scheme.from_token(AssistChipTokens::DisabledIconColor), AssistChipTokens::DisabledIconOpacity),
        )
    }

    /// An elevated assist chip's colors in `scheme` (Compose's internal
    /// `ColorScheme.defaultElevatedAssistChipColors`): as the flat one's, on the low surface container (the
    /// surface's text at 12% disabled).
    pub(crate) fn default_elevated_assist_chip_colors(scheme: &ColorScheme) -> ChipColors {
        ChipColors::new(
            scheme.from_token(AssistChipTokens::ElevatedContainerColor),
            scheme.from_token(AssistChipTokens::LabelTextColor),
            scheme.from_token(AssistChipTokens::IconColor),
            scheme.from_token(AssistChipTokens::IconColor),
            with_alpha(
                scheme.from_token(AssistChipTokens::ElevatedDisabledContainerColor),
                AssistChipTokens::ElevatedDisabledContainerOpacity,
            ),
            with_alpha(
                scheme.from_token(AssistChipTokens::DisabledLabelTextColor),
                AssistChipTokens::DisabledLabelTextOpacity,
            ),
            with_alpha(scheme.from_token(AssistChipTokens::DisabledIconColor), AssistChipTokens::DisabledIconOpacity),
            with_alpha(scheme.from_token(AssistChipTokens::DisabledIconColor), AssistChipTokens::DisabledIconOpacity),
        )
    }
}

#[named]
impl AssistChipDefaults {
    /// An assist chip's colors (Compose's `AssistChipDefaults.assistChipColors()` and `assistChipColors(…)`): the
    /// theme's ([`default`](Self::default_assist_chip_colors)), those given in place of theirs (an unspecified one
    /// keeps the theme's).
    #[allow(clippy::too_many_arguments)]
    pub fn assist_chip_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] label_color: Color,
        #[default(Color::Unspecified)] leading_icon_content_color: Color,
        #[default(Color::Unspecified)] trailing_icon_content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_label_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_content_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_content_color: Color,
    ) -> ChipColors {
        named_args!(AssistChipDefaults::default_assist_chip_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            label_color = label_color,
            leading_icon_content_color = leading_icon_content_color,
            trailing_icon_content_color = trailing_icon_content_color,
            disabled_container_color = disabled_container_color,
            disabled_label_color = disabled_label_color,
            disabled_leading_icon_content_color = disabled_leading_icon_content_color,
            disabled_trailing_icon_content_color = disabled_trailing_icon_content_color,
        ))
    }

    /// An assist chip's elevation (Compose's `AssistChipDefaults.assistChipElevation`): none, 8 dp dragged; pressed,
    /// focused, hovered and disabled `elevation` where not given (Kotlin's defaults `elevation`, here `None`).
    pub fn assist_chip_elevation(
        &self,
        #[default(AssistChipTokens::FlatContainerElevation)] elevation: Dp,
        #[default(None)] pressed_elevation: Option<Dp>,
        #[default(None)] focused_elevation: Option<Dp>,
        #[default(None)] hovered_elevation: Option<Dp>,
        #[default(AssistChipTokens::DraggedContainerElevation)] dragged_elevation: Dp,
        #[default(None)] disabled_elevation: Option<Dp>,
    ) -> ChipElevation {
        ChipElevation::new(
            elevation,
            pressed_elevation.unwrap_or(elevation),
            focused_elevation.unwrap_or(elevation),
            hovered_elevation.unwrap_or(elevation),
            dragged_elevation,
            disabled_elevation.unwrap_or(elevation),
        )
    }

    /// An assist chip's border (Compose's `AssistChipDefaults.assistChipBorder(enabled, …)`): `border_width` (1 dp)
    /// of `border_color` (the outline variant), `disabled_border_color` (the surface's text at 12%) not `enabled`.
    pub fn assist_chip_border(
        &self,
        enabled: bool,
        #[default(AssistChipTokens::FlatOutlineColor.value())] border_color: Color,
        #[default(with_alpha(AssistChipTokens::FlatDisabledOutlineColor.value(), AssistChipTokens::FlatDisabledOutlineOpacity))]
        disabled_border_color: Color,
        #[default(AssistChipTokens::FlatOutlineWidth)] border_width: Dp,
    ) -> BorderStroke {
        BorderStroke(border_width, if enabled { border_color } else { disabled_border_color })
    }

    /// An assist chip's border as a [`ChipBorder`] (Compose's deprecated `assistChipBorder(borderColor, …)`).
    ///
    /// Kotlin: `assistChipBorder(borderColor: Color = AssistChipTokens.FlatOutlineColor.value, disabledBorderColor: Color = AssistChipTokens.FlatDisabledOutlineColor.value.copy(alpha = AssistChipTokens.FlatDisabledOutlineOpacity), borderWidth: Dp = AssistChipTokens.FlatOutlineWidth): ChipBorder`
    #[deprecated(
        note = "Maintained for binary compatibility. Use the assistChipBorder function that returns BorderStroke instead"
    )]
    pub fn assist_chip_chip_border(
        &self,
        #[default(AssistChipTokens::FlatOutlineColor.value())] border_color: Color,
        #[default(with_alpha(AssistChipTokens::FlatDisabledOutlineColor.value(), AssistChipTokens::FlatDisabledOutlineOpacity))]
        disabled_border_color: Color,
        #[default(AssistChipTokens::FlatOutlineWidth)] border_width: Dp,
    ) -> ChipBorder {
        ChipBorder::new(border_color, disabled_border_color, border_width)
    }

    /// An elevated assist chip's colors (Compose's `AssistChipDefaults.elevatedAssistChipColors()` and
    /// `elevatedAssistChipColors(…)`): with nothing given, the theme's elevated assist chip colors; else those
    /// given over the elevated suggestion chip's (as Compose's), `None` or an unspecified one keeping those
    /// (Kotlin's defaults `Color.Unspecified`; `None` tells the overload without parameters).
    ///
    /// Kotlin: `elevatedAssistChipColors(containerColor: Color = Color.Unspecified, labelColor: Color = Color.Unspecified, leadingIconContentColor: Color = Color.Unspecified, trailingIconContentColor: Color = Color.Unspecified, disabledContainerColor: Color = Color.Unspecified, disabledLabelColor: Color = Color.Unspecified, disabledLeadingIconContentColor: Color = Color.Unspecified, disabledTrailingIconContentColor: Color = Color.Unspecified): ChipColors`
    #[allow(clippy::too_many_arguments)]
    pub fn elevated_assist_chip_colors(
        &self,
        #[default(None)] container_color: Option<Color>,
        #[default(None)] label_color: Option<Color>,
        #[default(None)] leading_icon_content_color: Option<Color>,
        #[default(None)] trailing_icon_content_color: Option<Color>,
        #[default(None)] disabled_container_color: Option<Color>,
        #[default(None)] disabled_label_color: Option<Color>,
        #[default(None)] disabled_leading_icon_content_color: Option<Color>,
        #[default(None)] disabled_trailing_icon_content_color: Option<Color>,
    ) -> ChipColors {
        let scheme = MaterialTheme::color_scheme();
        let given = [
            container_color,
            label_color,
            leading_icon_content_color,
            trailing_icon_content_color,
            disabled_container_color,
            disabled_label_color,
            disabled_leading_icon_content_color,
            disabled_trailing_icon_content_color,
        ];
        if given.iter().all(Option::is_none) {
            return AssistChipDefaults::default_elevated_assist_chip_colors(&scheme);
        }
        let u = |c: Option<Color>| c.unwrap_or(Color::Unspecified);
        named_args!(SuggestionChipDefaults::default_elevated_suggestion_chip_colors(&scheme).copy(
            container_color = u(container_color),
            label_color = u(label_color),
            leading_icon_content_color = u(leading_icon_content_color),
            trailing_icon_content_color = u(trailing_icon_content_color),
            disabled_container_color = u(disabled_container_color),
            disabled_label_color = u(disabled_label_color),
            disabled_leading_icon_content_color = u(disabled_leading_icon_content_color),
            disabled_trailing_icon_content_color = u(disabled_trailing_icon_content_color),
        ))
    }

    /// An elevated assist chip's elevation (Compose's `AssistChipDefaults.elevatedAssistChipElevation`): 1 dp,
    /// 3 dp hovered, 8 dp dragged, none disabled.
    pub fn elevated_assist_chip_elevation(
        &self,
        #[default(AssistChipTokens::ElevatedContainerElevation)] elevation: Dp,
        #[default(AssistChipTokens::ElevatedPressedContainerElevation)] pressed_elevation: Dp,
        #[default(AssistChipTokens::ElevatedFocusContainerElevation)] focused_elevation: Dp,
        #[default(AssistChipTokens::ElevatedHoverContainerElevation)] hovered_elevation: Dp,
        #[default(AssistChipTokens::DraggedContainerElevation)] dragged_elevation: Dp,
        #[default(AssistChipTokens::ElevatedDisabledContainerElevation)] disabled_elevation: Dp,
    ) -> ChipElevation {
        ChipElevation::new(
            elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            dragged_elevation,
            disabled_elevation,
        )
    }
}

// ---- FilterChipDefaults ----------------------------------------------------------------------------------------

/// The filter chips' defaults (Compose's `FilterChipDefaults`).
#[derive(Clone, Copy, Debug)]
pub struct FilterChipDefaults;

impl FilterChipDefaults {
    /// A filter chip's height, 32 dp (Compose's `FilterChipDefaults.Height`).
    pub const Height: Dp = FilterChipTokens::ContainerHeight;

    /// Its icons' size, 18 dp (Compose's `FilterChipDefaults.IconSize`).
    pub const IconSize: Dp = FilterChipTokens::IconSize;

    /// The space between its parts, 8 dp (Compose's `FilterChipDefaults.HorizontalSpacing`).
    pub const HorizontalSpacing: Dp = Dp(8.0);

    /// Its parts' arrangement (Compose's `FilterChipDefaults.horizontalArrangement()`).
    pub fn horizontal_arrangement() -> HorizontalArrangement {
        DefaultHorizontalArrangement()
    }

    /// The same, the label `spacing` after the leading part (Compose's `horizontalArrangement(spacing)`).
    ///
    /// Kotlin: `horizontalArrangement(spacing: Dp): Arrangement.Horizontal`
    pub fn horizontal_arrangement_with_spacing(spacing: Dp) -> HorizontalArrangement {
        if spacing == Self::HorizontalSpacing {
            return DefaultHorizontalArrangement();
        }
        ChipArrangement(spacing)
    }

    /// Its padding, 8 dp at its sides (Compose's `FilterChipDefaults.ContentPadding`).
    pub fn ContentPadding() -> PaddingValues {
        PaddingValues::hv(Dp(8.0), Dp(0.0))
    }

    /// Its shape, 8 dp corners (Compose's `FilterChipDefaults.shape`).
    pub fn shape() -> Arc<dyn Shape> {
        FilterChipTokens::ContainerShape.value()
    }

    /// A filter chip's colors in `scheme` (Compose's internal `ColorScheme.defaultFilterChipColors`): no container
    /// (the secondary container selected; the surface's text at 12% selected and disabled), the label the surface
    /// variant's text (on the secondary container selected), the leading icon primary.
    pub(crate) fn default_filter_chip_colors(scheme: &ColorScheme) -> SelectableChipColors {
        SelectableChipColors::new(
            Color::Transparent,
            scheme.from_token(FilterChipTokens::UnselectedLabelTextColor),
            scheme.from_token(FilterChipTokens::UnselectedLeadingIconColor),
            scheme.from_token(FilterChipTokens::UnselectedTrailingIconColor),
            Color::Transparent,
            with_alpha(
                scheme.from_token(FilterChipTokens::DisabledLabelTextColor),
                FilterChipTokens::DisabledLabelTextOpacity,
            ),
            with_alpha(
                scheme.from_token(FilterChipTokens::DisabledLeadingIconColor),
                FilterChipTokens::DisabledLeadingIconOpacity,
            ),
            with_alpha(
                scheme.from_token(FilterChipTokens::DisabledTrailingIconColor),
                FilterChipTokens::DisabledTrailingIconOpacity,
            ),
            scheme.from_token(FilterChipTokens::FlatSelectedContainerColor),
            with_alpha(
                scheme.from_token(FilterChipTokens::FlatDisabledSelectedContainerColor),
                FilterChipTokens::FlatDisabledSelectedContainerOpacity,
            ),
            scheme.from_token(FilterChipTokens::SelectedLabelTextColor),
            scheme.from_token(FilterChipTokens::SelectedLeadingIconColor),
            scheme.from_token(FilterChipTokens::SelectedTrailingIconColor),
        )
    }

    /// An elevated filter chip's colors in `scheme` (Compose's internal
    /// `ColorScheme.defaultElevatedFilterChipColors`): as the flat one's, on the low surface container (the
    /// surface's text at 12% disabled, selected or not).
    pub(crate) fn default_elevated_filter_chip_colors(scheme: &ColorScheme) -> SelectableChipColors {
        SelectableChipColors::new(
            scheme.from_token(FilterChipTokens::ElevatedUnselectedContainerColor),
            scheme.from_token(FilterChipTokens::UnselectedLabelTextColor),
            scheme.from_token(FilterChipTokens::UnselectedLeadingIconColor),
            scheme.from_token(FilterChipTokens::UnselectedTrailingIconColor),
            with_alpha(
                scheme.from_token(FilterChipTokens::ElevatedDisabledContainerColor),
                FilterChipTokens::ElevatedDisabledContainerOpacity,
            ),
            with_alpha(
                scheme.from_token(FilterChipTokens::DisabledLabelTextColor),
                FilterChipTokens::DisabledLabelTextOpacity,
            ),
            with_alpha(
                scheme.from_token(FilterChipTokens::DisabledLeadingIconColor),
                FilterChipTokens::DisabledLeadingIconOpacity,
            ),
            // (the leading icon's opacity, as Compose's)
            with_alpha(
                scheme.from_token(FilterChipTokens::DisabledTrailingIconColor),
                FilterChipTokens::DisabledLeadingIconOpacity,
            ),
            scheme.from_token(FilterChipTokens::ElevatedSelectedContainerColor),
            with_alpha(
                scheme.from_token(FilterChipTokens::ElevatedDisabledContainerColor),
                FilterChipTokens::ElevatedDisabledContainerOpacity,
            ),
            scheme.from_token(FilterChipTokens::SelectedLabelTextColor),
            scheme.from_token(FilterChipTokens::SelectedLeadingIconColor),
            scheme.from_token(FilterChipTokens::SelectedTrailingIconColor),
        )
    }
}

#[named]
impl FilterChipDefaults {
    /// A filter chip's colors (Compose's `FilterChipDefaults.filterChipColors()` and `filterChipColors(…)`): the
    /// theme's, those given in place of theirs (an unspecified one keeps the theme's; `icon_color` both icons').
    #[allow(clippy::too_many_arguments)]
    pub fn filter_chip_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] label_color: Color,
        #[default(Color::Unspecified)] icon_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_label_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
        #[default(Color::Unspecified)] selected_container_color: Color,
        #[default(Color::Unspecified)] disabled_selected_container_color: Color,
        #[default(Color::Unspecified)] selected_label_color: Color,
        #[default(Color::Unspecified)] selected_leading_icon_color: Color,
        #[default(Color::Unspecified)] selected_trailing_icon_color: Color,
    ) -> SelectableChipColors {
        named_args!(FilterChipDefaults::default_filter_chip_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            label_color = label_color,
            leading_icon_color = icon_color,
            trailing_icon_color = icon_color,
            disabled_container_color = disabled_container_color,
            disabled_label_color = disabled_label_color,
            disabled_leading_icon_color = disabled_leading_icon_color,
            disabled_trailing_icon_color = disabled_trailing_icon_color,
            selected_container_color = selected_container_color,
            disabled_selected_container_color = disabled_selected_container_color,
            selected_label_color = selected_label_color,
            selected_leading_icon_color = selected_leading_icon_color,
            selected_trailing_icon_color = selected_trailing_icon_color,
        ))
    }

    /// A filter chip's elevation (Compose's `FilterChipDefaults.filterChipElevation`): none, 1 dp hovered, 8 dp
    /// dragged; disabled `elevation` where not given (Kotlin's default `elevation`, here `None`).
    pub fn filter_chip_elevation(
        &self,
        #[default(FilterChipTokens::FlatContainerElevation)] elevation: Dp,
        #[default(FilterChipTokens::FlatSelectedPressedContainerElevation)] pressed_elevation: Dp,
        #[default(FilterChipTokens::FlatSelectedFocusContainerElevation)] focused_elevation: Dp,
        #[default(FilterChipTokens::FlatSelectedHoverContainerElevation)] hovered_elevation: Dp,
        #[default(FilterChipTokens::DraggedContainerElevation)] dragged_elevation: Dp,
        #[default(None)] disabled_elevation: Option<Dp>,
    ) -> SelectableChipElevation {
        SelectableChipElevation::new(
            elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            dragged_elevation,
            disabled_elevation.unwrap_or(elevation),
        )
    }

    /// A filter chip's border (Compose's `FilterChipDefaults.filterChipBorder`): `border_width` (1 dp) of
    /// `border_color` (the outline variant) unselected, `selected_border_width` (none) of `selected_border_color`
    /// selected; their disabled colors (the surface's text at 12%, transparent) not `enabled`.
    #[allow(clippy::too_many_arguments)]
    pub fn filter_chip_border(
        &self,
        enabled: bool,
        selected: bool,
        #[default(FilterChipTokens::FlatUnselectedOutlineColor.value())] border_color: Color,
        #[default(Color::Transparent)] selected_border_color: Color,
        #[default(with_alpha(
            FilterChipTokens::FlatDisabledUnselectedOutlineColor.value(),
            FilterChipTokens::FlatDisabledUnselectedOutlineOpacity
        ))]
        disabled_border_color: Color,
        #[default(Color::Transparent)] disabled_selected_border_color: Color,
        #[default(FilterChipTokens::FlatUnselectedOutlineWidth)] border_width: Dp,
        #[default(FilterChipTokens::FlatSelectedOutlineWidth)] selected_border_width: Dp,
    ) -> BorderStroke {
        let color = if enabled {
            if selected { selected_border_color } else { border_color }
        } else if selected {
            disabled_selected_border_color
        } else {
            disabled_border_color
        };
        BorderStroke(if selected { selected_border_width } else { border_width }, color)
    }

    /// An elevated filter chip's colors (Compose's `FilterChipDefaults.elevatedFilterChipColors()` and
    /// `elevatedFilterChipColors(…)`): the theme's, those given in place of theirs.
    #[allow(clippy::too_many_arguments)]
    pub fn elevated_filter_chip_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] label_color: Color,
        #[default(Color::Unspecified)] icon_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_label_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
        #[default(Color::Unspecified)] selected_container_color: Color,
        #[default(Color::Unspecified)] disabled_selected_container_color: Color,
        #[default(Color::Unspecified)] selected_label_color: Color,
        #[default(Color::Unspecified)] selected_leading_icon_color: Color,
        #[default(Color::Unspecified)] selected_trailing_icon_color: Color,
    ) -> SelectableChipColors {
        named_args!(FilterChipDefaults::default_elevated_filter_chip_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            label_color = label_color,
            leading_icon_color = icon_color,
            trailing_icon_color = icon_color,
            disabled_container_color = disabled_container_color,
            disabled_label_color = disabled_label_color,
            disabled_leading_icon_color = disabled_leading_icon_color,
            disabled_trailing_icon_color = disabled_trailing_icon_color,
            selected_container_color = selected_container_color,
            disabled_selected_container_color = disabled_selected_container_color,
            selected_label_color = selected_label_color,
            selected_leading_icon_color = selected_leading_icon_color,
            selected_trailing_icon_color = selected_trailing_icon_color,
        ))
    }

    /// An elevated filter chip's elevation (Compose's `FilterChipDefaults.elevatedFilterChipElevation`): 1 dp,
    /// 3 dp hovered, 8 dp dragged, none disabled.
    pub fn elevated_filter_chip_elevation(
        &self,
        #[default(FilterChipTokens::ElevatedContainerElevation)] elevation: Dp,
        #[default(FilterChipTokens::ElevatedPressedContainerElevation)] pressed_elevation: Dp,
        #[default(FilterChipTokens::ElevatedFocusContainerElevation)] focused_elevation: Dp,
        #[default(FilterChipTokens::ElevatedHoverContainerElevation)] hovered_elevation: Dp,
        #[default(FilterChipTokens::DraggedContainerElevation)] dragged_elevation: Dp,
        #[default(FilterChipTokens::ElevatedDisabledContainerElevation)] disabled_elevation: Dp,
    ) -> SelectableChipElevation {
        SelectableChipElevation::new(
            elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            dragged_elevation,
            disabled_elevation,
        )
    }
}

// ---- InputChipDefaults -----------------------------------------------------------------------------------------

/// The input chips' defaults (Compose's `InputChipDefaults`).
#[derive(Clone, Copy, Debug)]
pub struct InputChipDefaults;

impl InputChipDefaults {
    /// An input chip's height, 32 dp (Compose's `InputChipDefaults.Height`).
    pub const Height: Dp = InputChipTokens::ContainerHeight;

    /// Its icons' size, 18 dp (Compose's `InputChipDefaults.IconSize`).
    pub const IconSize: Dp = InputChipTokens::LeadingIconSize;

    /// Its avatar's size, 24 dp (Compose's `InputChipDefaults.AvatarSize`).
    pub const AvatarSize: Dp = InputChipTokens::AvatarSize;

    /// The space between its parts, 8 dp (Compose's `InputChipDefaults.HorizontalSpacing`).
    pub const HorizontalSpacing: Dp = Dp(8.0);

    /// Its parts' arrangement (Compose's `InputChipDefaults.horizontalArrangement()`).
    pub fn horizontal_arrangement() -> HorizontalArrangement {
        DefaultHorizontalArrangement()
    }

    /// The same, the label `spacing` after the leading part (Compose's `horizontalArrangement(spacing)`).
    ///
    /// Kotlin: `horizontalArrangement(spacing: Dp): Arrangement.Horizontal`
    pub fn horizontal_arrangement_with_spacing(spacing: Dp) -> HorizontalArrangement {
        if spacing == Self::HorizontalSpacing {
            return DefaultHorizontalArrangement();
        }
        ChipArrangement(spacing)
    }

    /// Its padding (Compose's `InputChipDefaults.contentPadding`): 4 dp at the start with an avatar or without a
    /// leading icon, else 8 dp; 8 dp at the end with a trailing icon, else 4 dp.
    pub fn content_padding(has_avatar: bool, has_leading_icon: bool, has_trailing_icon: bool) -> PaddingValues {
        let start = if has_avatar || !has_leading_icon { Dp(4.0) } else { Dp(8.0) };
        let end = if has_trailing_icon { Dp(8.0) } else { Dp(4.0) };
        PaddingValues::new(start, Dp(0.0), end, Dp(0.0))
    }

    /// Its shape, 8 dp corners (Compose's `InputChipDefaults.shape`).
    pub fn shape() -> Arc<dyn Shape> {
        InputChipTokens::ContainerShape.value()
    }

    /// An input chip's colors in `scheme` (Compose's internal `ColorScheme.defaultInputChipColors`): no container
    /// (the secondary container selected), the label and the trailing icon the surface variant's text, the
    /// leading icon primary (on the secondary container selected, but the leading icon: primary).
    pub(crate) fn default_input_chip_colors(scheme: &ColorScheme) -> SelectableChipColors {
        SelectableChipColors::new(
            Color::Transparent,
            scheme.from_token(InputChipTokens::UnselectedLabelTextColor),
            scheme.from_token(InputChipTokens::UnselectedLeadingIconColor),
            scheme.from_token(InputChipTokens::UnselectedTrailingIconColor),
            Color::Transparent,
            with_alpha(
                scheme.from_token(InputChipTokens::DisabledLabelTextColor),
                InputChipTokens::DisabledLabelTextOpacity,
            ),
            with_alpha(
                scheme.from_token(InputChipTokens::DisabledLeadingIconColor),
                InputChipTokens::DisabledLeadingIconOpacity,
            ),
            with_alpha(
                scheme.from_token(InputChipTokens::DisabledTrailingIconColor),
                InputChipTokens::DisabledTrailingIconOpacity,
            ),
            scheme.from_token(InputChipTokens::SelectedContainerColor),
            with_alpha(
                scheme.from_token(InputChipTokens::DisabledSelectedContainerColor),
                InputChipTokens::DisabledSelectedContainerOpacity,
            ),
            scheme.from_token(InputChipTokens::SelectedLabelTextColor),
            scheme.from_token(InputChipTokens::SelectedLeadingIconColor),
            scheme.from_token(InputChipTokens::SelectedTrailingIconColor),
        )
    }
}

#[named]
impl InputChipDefaults {
    /// An input chip's colors (Compose's `InputChipDefaults.inputChipColors()` and `inputChipColors(…)`): the
    /// theme's, those given in place of theirs (an unspecified one keeps the theme's).
    #[allow(clippy::too_many_arguments)]
    pub fn input_chip_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] label_color: Color,
        #[default(Color::Unspecified)] leading_icon_color: Color,
        #[default(Color::Unspecified)] trailing_icon_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_label_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
        #[default(Color::Unspecified)] selected_container_color: Color,
        #[default(Color::Unspecified)] disabled_selected_container_color: Color,
        #[default(Color::Unspecified)] selected_label_color: Color,
        #[default(Color::Unspecified)] selected_leading_icon_color: Color,
        #[default(Color::Unspecified)] selected_trailing_icon_color: Color,
    ) -> SelectableChipColors {
        named_args!(InputChipDefaults::default_input_chip_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            label_color = label_color,
            leading_icon_color = leading_icon_color,
            trailing_icon_color = trailing_icon_color,
            disabled_container_color = disabled_container_color,
            disabled_label_color = disabled_label_color,
            disabled_leading_icon_color = disabled_leading_icon_color,
            disabled_trailing_icon_color = disabled_trailing_icon_color,
            selected_container_color = selected_container_color,
            disabled_selected_container_color = disabled_selected_container_color,
            selected_label_color = selected_label_color,
            selected_leading_icon_color = selected_leading_icon_color,
            selected_trailing_icon_color = selected_trailing_icon_color,
        ))
    }

    /// An input chip's elevation (Compose's `InputChipDefaults.inputChipElevation`): none, 8 dp dragged; pressed,
    /// focused, hovered and disabled `elevation` where not given (Kotlin's defaults `elevation`, here `None`).
    pub fn input_chip_elevation(
        &self,
        #[default(InputChipTokens::ContainerElevation)] elevation: Dp,
        #[default(None)] pressed_elevation: Option<Dp>,
        #[default(None)] focused_elevation: Option<Dp>,
        #[default(None)] hovered_elevation: Option<Dp>,
        #[default(InputChipTokens::DraggedContainerElevation)] dragged_elevation: Dp,
        #[default(None)] disabled_elevation: Option<Dp>,
    ) -> SelectableChipElevation {
        SelectableChipElevation::new(
            elevation,
            pressed_elevation.unwrap_or(elevation),
            focused_elevation.unwrap_or(elevation),
            hovered_elevation.unwrap_or(elevation),
            dragged_elevation,
            disabled_elevation.unwrap_or(elevation),
        )
    }

    /// An input chip's border (Compose's `InputChipDefaults.inputChipBorder`): as
    /// [`FilterChipDefaults::filter_chip_border`]'s, of the input chip's tokens.
    #[allow(clippy::too_many_arguments)]
    pub fn input_chip_border(
        &self,
        enabled: bool,
        selected: bool,
        #[default(InputChipTokens::UnselectedOutlineColor.value())] border_color: Color,
        #[default(Color::Transparent)] selected_border_color: Color,
        #[default(with_alpha(
            InputChipTokens::DisabledUnselectedOutlineColor.value(),
            InputChipTokens::DisabledUnselectedOutlineOpacity
        ))]
        disabled_border_color: Color,
        #[default(Color::Transparent)] disabled_selected_border_color: Color,
        #[default(InputChipTokens::UnselectedOutlineWidth)] border_width: Dp,
        #[default(InputChipTokens::SelectedOutlineWidth)] selected_border_width: Dp,
    ) -> BorderStroke {
        let color = if enabled {
            if selected { selected_border_color } else { border_color }
        } else if selected {
            disabled_selected_border_color
        } else {
            disabled_border_color
        };
        BorderStroke(if selected { selected_border_width } else { border_width }, color)
    }
}

// ---- SuggestionChipDefaults ------------------------------------------------------------------------------------

/// The suggestion chips' defaults (Compose's `SuggestionChipDefaults`).
#[derive(Clone, Copy, Debug)]
pub struct SuggestionChipDefaults;

impl SuggestionChipDefaults {
    /// A suggestion chip's height, 32 dp (Compose's `SuggestionChipDefaults.Height`).
    pub const Height: Dp = SuggestionChipTokens::ContainerHeight;

    /// Its icon's size, 18 dp (Compose's `SuggestionChipDefaults.IconSize`).
    pub const IconSize: Dp = SuggestionChipTokens::LeadingIconSize;

    /// The space between its parts, 8 dp (Compose's `SuggestionChipDefaults.HorizontalSpacing`).
    pub const HorizontalSpacing: Dp = Dp(8.0);

    /// Its padding, 8 dp at its sides (Compose's `SuggestionChipDefaults.ContentPadding`).
    pub fn ContentPadding() -> PaddingValues {
        PaddingValues::hv(Dp(8.0), Dp(0.0))
    }

    /// Its parts' arrangement (Compose's `SuggestionChipDefaults.horizontalArrangement()`).
    pub fn horizontal_arrangement() -> HorizontalArrangement {
        DefaultHorizontalArrangement()
    }

    /// The same, the label `spacing` after the leading part (Compose's `horizontalArrangement(spacing)`).
    ///
    /// Kotlin: `horizontalArrangement(spacing: Dp): Arrangement.Horizontal`
    pub fn horizontal_arrangement_with_spacing(spacing: Dp) -> HorizontalArrangement {
        if spacing == Self::HorizontalSpacing {
            return DefaultHorizontalArrangement();
        }
        ChipArrangement(spacing)
    }

    /// Its shape, 8 dp corners (Compose's `SuggestionChipDefaults.shape`).
    pub fn shape() -> Arc<dyn Shape> {
        SuggestionChipTokens::ContainerShape.value()
    }

    /// A suggestion chip's colors in `scheme` (Compose's internal `ColorScheme.defaultSuggestionChipColors`): no
    /// container, the label the surface variant's text, the icon primary, no trailing icon color (unspecified).
    pub(crate) fn default_suggestion_chip_colors(scheme: &ColorScheme) -> ChipColors {
        ChipColors::new(
            Color::Transparent,
            scheme.from_token(SuggestionChipTokens::LabelTextColor),
            scheme.from_token(SuggestionChipTokens::LeadingIconColor),
            Color::Unspecified,
            Color::Transparent,
            with_alpha(
                scheme.from_token(SuggestionChipTokens::DisabledLabelTextColor),
                SuggestionChipTokens::DisabledLabelTextOpacity,
            ),
            with_alpha(
                scheme.from_token(SuggestionChipTokens::DisabledLeadingIconColor),
                SuggestionChipTokens::DisabledLeadingIconOpacity,
            ),
            Color::Unspecified,
        )
    }

    /// An elevated suggestion chip's colors in `scheme` (Compose's internal
    /// `ColorScheme.defaultElevatedSuggestionChipColors`): as the flat one's, on the low surface container (some
    /// of the assist chip's tokens, as Compose's).
    pub(crate) fn default_elevated_suggestion_chip_colors(scheme: &ColorScheme) -> ChipColors {
        ChipColors::new(
            scheme.from_token(SuggestionChipTokens::ElevatedContainerColor),
            scheme.from_token(SuggestionChipTokens::LabelTextColor),
            scheme.from_token(SuggestionChipTokens::LeadingIconColor),
            Color::Unspecified,
            with_alpha(
                scheme.from_token(SuggestionChipTokens::ElevatedDisabledContainerColor),
                AssistChipTokens::ElevatedDisabledContainerOpacity,
            ),
            with_alpha(
                scheme.from_token(SuggestionChipTokens::DisabledLabelTextColor),
                SuggestionChipTokens::DisabledLabelTextOpacity,
            ),
            with_alpha(scheme.from_token(AssistChipTokens::DisabledIconColor), AssistChipTokens::DisabledIconOpacity),
            Color::Unspecified,
        )
    }
}

#[named]
impl SuggestionChipDefaults {
    /// A suggestion chip's colors (Compose's `SuggestionChipDefaults.suggestionChipColors()` and
    /// `suggestionChipColors(…)`): the theme's, those given in place of theirs (`icon_content_color` the icon's).
    pub fn suggestion_chip_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] label_color: Color,
        #[default(Color::Unspecified)] icon_content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_label_color: Color,
        #[default(Color::Unspecified)] disabled_icon_content_color: Color,
    ) -> ChipColors {
        named_args!(SuggestionChipDefaults::default_suggestion_chip_colors(&MaterialTheme::color_scheme()).copy(
            container_color = container_color,
            label_color = label_color,
            leading_icon_content_color = icon_content_color,
            trailing_icon_content_color = Color::Unspecified,
            disabled_container_color = disabled_container_color,
            disabled_label_color = disabled_label_color,
            disabled_leading_icon_content_color = disabled_icon_content_color,
            disabled_trailing_icon_content_color = Color::Unspecified,
        ))
    }

    /// A suggestion chip's elevation (Compose's `SuggestionChipDefaults.suggestionChipElevation`): none, 4 dp
    /// dragged; pressed, focused, hovered and disabled `elevation` where not given (here `None`).
    pub fn suggestion_chip_elevation(
        &self,
        #[default(SuggestionChipTokens::FlatContainerElevation)] elevation: Dp,
        #[default(None)] pressed_elevation: Option<Dp>,
        #[default(None)] focused_elevation: Option<Dp>,
        #[default(None)] hovered_elevation: Option<Dp>,
        #[default(SuggestionChipTokens::DraggedContainerElevation)] dragged_elevation: Dp,
        #[default(None)] disabled_elevation: Option<Dp>,
    ) -> ChipElevation {
        ChipElevation::new(
            elevation,
            pressed_elevation.unwrap_or(elevation),
            focused_elevation.unwrap_or(elevation),
            hovered_elevation.unwrap_or(elevation),
            dragged_elevation,
            disabled_elevation.unwrap_or(elevation),
        )
    }

    /// A suggestion chip's border (Compose's `SuggestionChipDefaults.suggestionChipBorder(enabled, …)`): 1 dp of
    /// the outline variant, the surface's text at 12% not `enabled`.
    pub fn suggestion_chip_border(
        &self,
        enabled: bool,
        #[default(SuggestionChipTokens::FlatOutlineColor.value())] border_color: Color,
        #[default(with_alpha(
            SuggestionChipTokens::FlatDisabledOutlineColor.value(),
            SuggestionChipTokens::FlatDisabledOutlineOpacity
        ))]
        disabled_border_color: Color,
        #[default(SuggestionChipTokens::FlatOutlineWidth)] border_width: Dp,
    ) -> BorderStroke {
        BorderStroke(border_width, if enabled { border_color } else { disabled_border_color })
    }

    /// A suggestion chip's border as a [`ChipBorder`] (Compose's deprecated `suggestionChipBorder(borderColor,
    /// …)`).
    ///
    /// Kotlin: `suggestionChipBorder(borderColor: Color = SuggestionChipTokens.FlatOutlineColor.value, disabledBorderColor: Color = SuggestionChipTokens.FlatDisabledOutlineColor.value.copy(alpha = SuggestionChipTokens.FlatDisabledOutlineOpacity), borderWidth: Dp = SuggestionChipTokens.FlatOutlineWidth): ChipBorder`
    #[deprecated(note = "Maintained for binary compatibility. Use the suggestChipBorder functions instead")]
    pub fn suggestion_chip_chip_border(
        &self,
        #[default(SuggestionChipTokens::FlatOutlineColor.value())] border_color: Color,
        #[default(with_alpha(
            SuggestionChipTokens::FlatDisabledOutlineColor.value(),
            SuggestionChipTokens::FlatDisabledOutlineOpacity
        ))]
        disabled_border_color: Color,
        #[default(SuggestionChipTokens::FlatOutlineWidth)] border_width: Dp,
    ) -> ChipBorder {
        ChipBorder::new(border_color, disabled_border_color, border_width)
    }

    /// An elevated suggestion chip's colors (Compose's `SuggestionChipDefaults.elevatedSuggestionChipColors()` and
    /// `elevatedSuggestionChipColors(…)`): the theme's, those given in place of theirs.
    pub fn elevated_suggestion_chip_colors(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] label_color: Color,
        #[default(Color::Unspecified)] icon_content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_label_color: Color,
        #[default(Color::Unspecified)] disabled_icon_content_color: Color,
    ) -> ChipColors {
        named_args!(
            SuggestionChipDefaults::default_elevated_suggestion_chip_colors(&MaterialTheme::color_scheme()).copy(
                container_color = container_color,
                label_color = label_color,
                leading_icon_content_color = icon_content_color,
                trailing_icon_content_color = Color::Unspecified,
                disabled_container_color = disabled_container_color,
                disabled_label_color = disabled_label_color,
                disabled_leading_icon_content_color = disabled_icon_content_color,
                disabled_trailing_icon_content_color = Color::Unspecified,
            )
        )
    }

    /// An elevated suggestion chip's elevation (Compose's
    /// `SuggestionChipDefaults.elevatedSuggestionChipElevation`): 1 dp, 3 dp hovered, 8 dp dragged, none disabled.
    pub fn elevated_suggestion_chip_elevation(
        &self,
        #[default(SuggestionChipTokens::ElevatedContainerElevation)] elevation: Dp,
        #[default(SuggestionChipTokens::ElevatedPressedContainerElevation)] pressed_elevation: Dp,
        #[default(SuggestionChipTokens::ElevatedFocusContainerElevation)] focused_elevation: Dp,
        #[default(SuggestionChipTokens::ElevatedHoverContainerElevation)] hovered_elevation: Dp,
        #[default(SuggestionChipTokens::DraggedContainerElevation)] dragged_elevation: Dp,
        #[default(SuggestionChipTokens::ElevatedDisabledContainerElevation)] disabled_elevation: Dp,
    ) -> ChipElevation {
        ChipElevation::new(
            elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            dragged_elevation,
            disabled_elevation,
        )
    }
}

/// `color` at `alpha` (Kotlin's `color.copy(alpha = …)`).
fn with_alpha(color: Color, alpha: f32) -> Color {
    named_args!(color.copy(alpha = alpha))
}

// ---- The arrangement -------------------------------------------------------------------------------------------

/// The chips' arrangement of their three parts (Compose's private `ChipArrangement` class): the first at the
/// start, the second `spacing` after it, the third at the end, mirrored right to left. Equal only to itself (a
/// clone of it), as Compose's class.
fn ChipArrangement(spacing: Dp) -> HorizontalArrangement {
    Arrangement::horizontal(spacing, move |density, total_size, sizes, layout_direction, out_positions| {
        if sizes.is_empty() {
            return;
        }
        let spacing_px = density.round_to_px(spacing);

        for (index, (&size, out)) in sizes.iter().zip(out_positions.iter_mut()).enumerate() {
            let current_x = match index {
                0 => 0,
                1 => sizes[0] + spacing_px,
                2 => total_size - size,
                _ => 0,
            };

            *out = if layout_direction == LayoutDirection::Ltr { current_x } else { total_size - current_x - size };
        }
    })
}

thread_local! {
    static DEFAULT_HORIZONTAL_ARRANGEMENT: HorizontalArrangement = ChipArrangement(SuggestionChipDefaults::HorizontalSpacing);
}

/// The one default arrangement every chip shares (Compose's private `DefaultHorizontalArrangement`).
fn DefaultHorizontalArrangement() -> HorizontalArrangement {
    DEFAULT_HORIZONTAL_ARRANGEMENT.with(HorizontalArrangement::clone)
}

/// The space between a chip's parts by default (Compose's private `HorizontalElementsPadding`).
const HorizontalElementsPadding: Dp = Dp(8.0);

/// How wide a selectable chip gets at most (Compose's private `maxChipWidth`).
const maxChipWidth: Dp = Dp(1000.0);

/// An input chip's padding (Compose's private `inputChipPadding`, unused there too:
/// [`InputChipDefaults::content_padding`] is its public twin).
#[allow(dead_code)]
fn input_chip_padding(has_avatar: bool, has_leading_icon: bool, has_trailing_icon: bool) -> PaddingValues {
    let start = if has_avatar || !has_leading_icon { Dp(4.0) } else { Dp(8.0) };
    let end = if has_trailing_icon { Dp(8.0) } else { Dp(4.0) };
    PaddingValues::new(start, Dp(0.0), end, Dp(0.0))
}

// ---- The layouts -----------------------------------------------------------------------------------------------

/// A chip that is clicked (Compose's private `Chip`): a [`ClickableSurface`] (a button to accessibility) of
/// `shape` in `colors`' container color, with the shadow `elevation` animates (none without one) and `border`, its
/// content a [`ChipContent`].
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn Chip(
    modifier: Modifier,
    on_click: impl Fn() + 'static,
    enabled: bool,
    label: impl Fn() + 'static,
    label_text_style: TextStyle,
    label_color: Color,
    leading_icon: Option<Rc<dyn Fn()>>,
    trailing_icon: Option<Rc<dyn Fn()>>,
    shape: Arc<dyn Shape>,
    colors: ChipColors,
    elevation: Option<ChipElevation>,
    border: Option<BorderStroke>,
    min_height: Dp,
    #[default(Arrangement::spaced_by_aligned(HorizontalElementsPadding, Alignment::CenterHorizontally))]
    horizontal_arrangement: HorizontalArrangement,
    padding_values: PaddingValues,
    interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let source = interaction_source.as_interaction_source();
    let shadow_elevation = match elevation {
        Some(elevation) => elevation.shadow_elevation(enabled, &source).value(),
        None => Dp(0.0),
    };
    ClickableSurface(
        on_click,
        modifier = modifier.semantics(|c| c.set_role(Role::Button)),
        enabled = enabled,
        shape = shape,
        color = colors.container_color_for(enabled),
        shadow_elevation = shadow_elevation,
        border = border,
        interaction_source = Some(interaction_source),
        content = || {
            ChipContent(
                label = label,
                label_text_style = label_text_style,
                label_color = label_color,
                leading_icon = leading_icon,
                avatar = None,
                trailing_icon = trailing_icon,
                leading_icon_color = colors.leading_icon_content_color_for(enabled),
                trailing_icon_color = colors.trailing_icon_content_color_for(enabled),
                min_height = min_height,
                horizontal_arrangement = horizontal_arrangement,
                padding_values = padding_values,
            );
        },
    );
}

/// A chip that is selected (Compose's private `SelectableChip`): a [`SelectableSurface`] (a checkbox to
/// accessibility) in `colors` for `enabled` and `selected`, its content an [`AnimatingChipContent`].
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SelectableChip(
    selected: bool,
    modifier: Modifier,
    on_click: impl Fn() + 'static,
    enabled: bool,
    label: impl Fn() + 'static,
    label_text_style: TextStyle,
    leading_icon: Option<Rc<dyn Fn()>>,
    avatar: Option<Rc<dyn Fn()>>,
    trailing_icon: Option<Rc<dyn Fn()>>,
    shape: Arc<dyn Shape>,
    colors: SelectableChipColors,
    elevation: Option<SelectableChipElevation>,
    border: Option<BorderStroke>,
    min_height: Dp,
    #[default(ChipArrangement(HorizontalElementsPadding))] horizontal_arrangement: HorizontalArrangement,
    padding_values: PaddingValues,
    interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let source = interaction_source.as_interaction_source();
    let shadow_elevation = match elevation {
        Some(elevation) => elevation.shadow_elevation(enabled, &source).value(),
        None => Dp(0.0),
    };
    SelectableSurface(
        selected,
        on_click,
        modifier = modifier.semantics(|c| c.set_role(Role::Checkbox)),
        enabled = enabled,
        shape = shape,
        color = colors.container_color_for(enabled, selected),
        shadow_elevation = shadow_elevation,
        border = border,
        interaction_source = Some(interaction_source),
        content = || {
            // Selectable chips are animating the leading and trailing icons when they change from `null` to
            // non-`null` values.
            AnimatingChipContent(
                label = label,
                label_text_style = label_text_style,
                leading_icon = leading_icon,
                avatar = avatar,
                label_color = colors.label_color_for(enabled, selected),
                trailing_icon = trailing_icon,
                leading_icon_color = colors.leading_icon_content_color_for(enabled, selected),
                trailing_icon_color = colors.trailing_icon_content_color_for(enabled, selected),
                min_height = min_height,
                horizontal_arrangement = horizontal_arrangement,
                padding_values = padding_values,
            );
        },
    );
}

/// A chip's content (Compose's private `ChipContent`): in `label_color` and `label_text_style`, a Row as wide as
/// its widest intrinsic width, at least `min_height` tall, padded by `padding_values`, centered across, placed by
/// `horizontal_arrangement` — the leading part (the avatar, else the leading icon in `leading_icon_color`; an
/// empty Spacer without), the label (taking the rest), the trailing part (the trailing icon in
/// `trailing_icon_color`; an empty Spacer without).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn ChipContent(
    label: impl Fn() + 'static,
    label_text_style: TextStyle,
    label_color: Color,
    leading_icon: Option<Rc<dyn Fn()>>,
    avatar: Option<Rc<dyn Fn()>>,
    trailing_icon: Option<Rc<dyn Fn()>>,
    leading_icon_color: Color,
    trailing_icon_color: Color,
    min_height: Dp,
    horizontal_arrangement: HorizontalArrangement,
    padding_values: PaddingValues,
) {
    KompozitionLocalProvider(
        &[LocalContentColor.provides(label_color), LocalTextStyle.provides(label_text_style)],
        move || {
            Row(
                modifier = Modifier
                    .width_intrinsic(IntrinsicSize::Max)
                    .default_min_size_with(Dp::Unspecified, min_height)
                    .padding_values(padding_values),
                vertical_alignment = Alignment::CenterVertically,
                horizontal_arrangement = horizontal_arrangement,
                content = || {
                    if avatar.is_some() || leading_icon.is_some() {
                        Box(
                            content_alignment = Alignment::Center,
                            content = || {
                                if let Some(leading_content) =
                                    leading_content(avatar.clone(), leading_icon.clone(), leading_icon_color)
                                {
                                    leading_content();
                                }
                            },
                        );
                    } else {
                        Spacer(Modifier.width(Dp(0.0)));
                    }
                    Row(
                        modifier = Modifier.weight(1.0),
                        horizontal_arrangement = Arrangement::Start,
                        vertical_alignment = Alignment::CenterVertically,
                        content = || label(),
                    );
                    if trailing_icon.is_some() {
                        Box(
                            content_alignment = Alignment::Center,
                            content = || {
                                if let Some(trailing_content) =
                                    trailing_content(trailing_icon.clone(), trailing_icon_color)
                                {
                                    trailing_content();
                                }
                            },
                        );
                    } else {
                        Spacer(Modifier.width(Dp(0.0)));
                    }
                },
            );
        },
    );
}

/// A selectable chip's content (Compose's private `AnimatingChipContent`): as [`ChipContent`] (the Row at most
/// 1000 dp wide, the label not filling the rest), its leading and trailing parts coming in (expanding from their
/// side along the scheme's fast spatial spec, fading in along its slow effects one) and going out (shrinking
/// towards their side along its default effects spec, fading out along its fast effects one) as they come and go
/// — the part going out still shown as it was.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn AnimatingChipContent(
    label: impl Fn() + 'static,
    label_text_style: TextStyle,
    label_color: Color,
    leading_icon: Option<Rc<dyn Fn()>>,
    avatar: Option<Rc<dyn Fn()>>,
    trailing_icon: Option<Rc<dyn Fn()>>,
    leading_icon_color: Color,
    trailing_icon_color: Color,
    min_height: Dp,
    horizontal_arrangement: HorizontalArrangement,
    padding_values: PaddingValues,
) {
    KompozitionLocalProvider(
        &[LocalContentColor.provides(label_color), LocalTextStyle.provides(label_text_style)],
        move || {
            let fade_in_spec = MotionSchemeKeyTokens::SlowEffects.value::<f32>();
            let fade_out_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>();
            let expand_spec = MotionSchemeKeyTokens::FastSpatial.value::<IntSize>();
            let shrink_spec = MotionSchemeKeyTokens::DefaultEffects.value::<IntSize>();

            Row(
                modifier = named_args!(Modifier.width_in(max = maxChipWidth))
                    .default_min_size_with(Dp::Unspecified, min_height)
                    .padding_values(padding_values),
                vertical_alignment = Alignment::CenterVertically,
                horizontal_arrangement = horizontal_arrangement,
                content = || {
                    Box(content = || {
                        RowAnimatedVisibility(
                            visible = avatar.is_some() || leading_icon.is_some(),
                            enter = expand_horizontally(animation_spec = expand_spec, expand_from = Alignment::Start)
                                + fade_in(animation_spec = fade_in_spec),
                            exit = shrink_horizontally(animation_spec = shrink_spec, shrink_towards = Alignment::Start)
                                + fade_out(animation_spec = fade_out_spec),
                            content = |_| {
                                let leading_content_retained_state = remember_retained_state(leading_content(
                                    avatar.clone(),
                                    leading_icon.clone(),
                                    leading_icon_color,
                                ));
                                Box(
                                    content_alignment = Alignment::Center,
                                    content = || {
                                        if let Some(content) = leading_content_retained_state.clone() {
                                            content();
                                        }
                                    },
                                );
                            },
                        );
                        if avatar.is_none() && leading_icon.is_none() {
                            Spacer(Modifier.width(Dp(0.0)));
                        }
                    });

                    Row(
                        modifier = Modifier.weight(1.0, false),
                        horizontal_arrangement = Arrangement::Start,
                        vertical_alignment = Alignment::CenterVertically,
                        content = || label(),
                    );

                    Box(content = || {
                        RowAnimatedVisibility(
                            visible = trailing_icon.is_some(),
                            enter = expand_horizontally(animation_spec = expand_spec, expand_from = Alignment::End)
                                + fade_in(animation_spec = fade_in_spec),
                            exit = shrink_horizontally(animation_spec = shrink_spec, shrink_towards = Alignment::End)
                                + fade_out(animation_spec = fade_out_spec),
                            content = |_| {
                                let trailing_content_retained_state = remember_retained_state(trailing_content(
                                    trailing_icon.clone(),
                                    trailing_icon_color,
                                ));
                                Box(
                                    content_alignment = Alignment::Center,
                                    content = || {
                                        if let Some(content) = trailing_content_retained_state.clone() {
                                            content();
                                        }
                                    },
                                );
                            },
                        );
                        if trailing_icon.is_none() {
                            Spacer(Modifier.width(Dp(0.0)));
                        }
                    });
                },
            );
        },
    );
}

/// A chip's leading part (Compose's private `leadingContent`): the avatar (it takes precedence), else the leading
/// icon in `leading_icon_color`, else none.
fn leading_content(
    avatar: Option<Rc<dyn Fn()>>,
    leading_icon: Option<Rc<dyn Fn()>>,
    leading_icon_color: Color,
) -> Option<Rc<dyn Fn()>> {
    match (avatar, leading_icon) {
        // An avatar takes precedence
        (Some(avatar), _) => Some(avatar),
        (None, Some(leading_icon)) => Some(Rc::new(move || {
            let leading_icon = leading_icon.clone();
            kompozable_block!({
                KompozitionLocalProvider(&[LocalContentColor.provides(leading_icon_color)], move || leading_icon());
            });
        })),
        // Neither exists
        (None, None) => None,
    }
}

/// A chip's trailing part (Compose's private `trailingContent`): the trailing icon in `trailing_icon_color`, or
/// none.
fn trailing_content(trailing_icon: Option<Rc<dyn Fn()>>, trailing_icon_color: Color) -> Option<Rc<dyn Fn()>> {
    trailing_icon.map(|trailing_icon| {
        Rc::new(move || {
            let trailing_icon = trailing_icon.clone();
            kompozable_block!({
                KompozitionLocalProvider(&[LocalContentColor.provides(trailing_icon_color)], move || trailing_icon());
            });
        }) as Rc<dyn Fn()>
    })
}

/// The last value given that wasn't none (Compose's private `rememberRetainedState`): what a part going out keeps
/// showing. (Compose's holds it in a snapshot state written and read in the same composition; a plain remembered
/// cell does the same here, a composable lambda not being a state's `Send` value.)
#[track_caller]
fn remember_retained_state<T: Clone + 'static>(target_value: Option<T>) -> Option<T> {
    let retained_state = remember(|| Rc::new(RefCell::new(target_value.clone())));
    if target_value.is_some() {
        *retained_state.borrow_mut() = target_value;
    }
    retained_state.borrow().clone()
}

// ---- ChipElevation ---------------------------------------------------------------------------------------------

/// A chip's elevations in its states (Compose's `ChipElevation`): the one by default, pressed, focused, hovered,
/// dragged and disabled — the shadow it animates between as its interactions come and go. Equal by its elevations
/// but the dragged one (as Compose's `equals`).
#[derive(Clone, Copy, Debug, KotlinProperties)]
pub struct ChipElevation {
    /// The elevation by default.
    pub elevation: Dp,
    /// Pressed.
    pub pressed_elevation: Dp,
    /// Focused.
    pub focused_elevation: Dp,
    /// Hovered.
    pub hovered_elevation: Dp,
    /// Dragged.
    pub dragged_elevation: Dp,
    /// Disabled.
    pub disabled_elevation: Dp,
}

impl PartialEq for ChipElevation {
    fn eq(&self, other: &Self) -> bool {
        self.elevation == other.elevation
            && self.pressed_elevation == other.pressed_elevation
            && self.focused_elevation == other.focused_elevation
            && self.hovered_elevation == other.hovered_elevation
            && self.disabled_elevation == other.disabled_elevation
    }
}

impl ChipElevation {
    /// Elevations of one's own (Compose's constructor).
    ///
    /// Kotlin: `ChipElevation(elevation: Dp, pressedElevation: Dp, focusedElevation: Dp, hoveredElevation: Dp, draggedElevation: Dp, disabledElevation: Dp)`
    pub fn new(
        elevation: Dp,
        pressed_elevation: Dp,
        focused_elevation: Dp,
        hovered_elevation: Dp,
        dragged_elevation: Dp,
        disabled_elevation: Dp,
    ) -> ChipElevation {
        ChipElevation {
            elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            dragged_elevation,
            disabled_elevation,
        }
    }

    /// The shadow's elevation as state (Compose's internal composable `shadowElevation`): the disabled one where
    /// not `enabled`, else that of the last of `interaction_source`'s interactions going on (a press, a hover, a
    /// focus, a drag), or the default one, animated to as [`AnimateElevation`] says from the interaction it last
    /// went to.
    #[track_caller]
    pub(crate) fn shadow_elevation(&self, enabled: bool, interaction_source: &InteractionSource) -> State<Dp> {
        animate_chip_elevation(*self, enabled, interaction_source.clone())
    }
}

/// A selectable chip's elevations in its states (Compose's `SelectableChipElevation`): as [`ChipElevation`].
#[derive(Clone, Copy, Debug, KotlinProperties)]
pub struct SelectableChipElevation {
    /// The elevation by default.
    pub elevation: Dp,
    /// Pressed.
    pub pressed_elevation: Dp,
    /// Focused.
    pub focused_elevation: Dp,
    /// Hovered.
    pub hovered_elevation: Dp,
    /// Dragged.
    pub dragged_elevation: Dp,
    /// Disabled.
    pub disabled_elevation: Dp,
}

impl PartialEq for SelectableChipElevation {
    fn eq(&self, other: &Self) -> bool {
        self.elevation == other.elevation
            && self.pressed_elevation == other.pressed_elevation
            && self.focused_elevation == other.focused_elevation
            && self.hovered_elevation == other.hovered_elevation
            && self.disabled_elevation == other.disabled_elevation
    }
}

impl SelectableChipElevation {
    /// Elevations of one's own (Compose's constructor).
    ///
    /// Kotlin: `SelectableChipElevation(elevation: Dp, pressedElevation: Dp, focusedElevation: Dp, hoveredElevation: Dp, draggedElevation: Dp, disabledElevation: Dp)`
    pub fn new(
        elevation: Dp,
        pressed_elevation: Dp,
        focused_elevation: Dp,
        hovered_elevation: Dp,
        dragged_elevation: Dp,
        disabled_elevation: Dp,
    ) -> SelectableChipElevation {
        SelectableChipElevation {
            elevation,
            pressed_elevation,
            focused_elevation,
            hovered_elevation,
            dragged_elevation,
            disabled_elevation,
        }
    }

    /// The shadow's elevation as state (Compose's internal composable `shadowElevation`): as
    /// [`ChipElevation`]'s.
    #[track_caller]
    pub(crate) fn shadow_elevation(&self, enabled: bool, interaction_source: &InteractionSource) -> State<Dp> {
        let e = ChipElevation::new(
            self.elevation,
            self.pressed_elevation,
            self.focused_elevation,
            self.hovered_elevation,
            self.dragged_elevation,
            self.disabled_elevation,
        );
        animate_chip_elevation(e, enabled, interaction_source.clone())
    }
}

/// The private `animateElevation` of Compose's `ChipElevation` and `SelectableChipElevation` (the same code): the
/// interactions going on collected, the target elevation of the last, animated to from the interaction last gone
/// to (kept once the animation is done), snapped to when disabled.
#[track_caller]
fn animate_chip_elevation(e: ChipElevation, enabled: bool, interaction_source: InteractionSource) -> State<Dp> {
    kompozable_block!({
        let interactions = remember(|| mutable_state_list_of(Vec::<Arc<dyn Interaction>>::new()));
        // (only the effect reads and writes it, as Compose's `var lastInteraction by remember { mutableStateOf }`)
        let last_interaction = remember(|| Rc::new(RefCell::new(None::<Arc<dyn Interaction>>)));
        let collected = interactions.clone();
        let collecting = interaction_source.interactions().collect(move |interaction| {
            track_interaction(&collected, interaction);
        });
        LaunchedEffect(interaction_source.clone(), collecting);

        let interaction = interactions.with(|list| list.last().cloned());

        let target = if !enabled {
            e.disabled_elevation
        } else {
            match &interaction {
                Some(i) if i.is::<PressInteraction::Press>() => e.pressed_elevation,
                Some(i) if i.is::<HoverInteraction::Enter>() => e.hovered_elevation,
                Some(i) if i.is::<FocusInteraction::Focus>() => e.focused_elevation,
                Some(i) if i.is::<DragInteraction::Start>() => e.dragged_elevation,
                _ => e.elevation,
            }
        };

        let animatable: Animatable<Dp, AnimationVector1D> =
            remember(|| Animatable::new(target, Dp::VectorConverter, None, "Animatable"));

        let (a, last) = (animatable.clone(), last_interaction.clone());
        LaunchedEffect(target, async move {
            if a.target_value() != target {
                if !enabled {
                    // No transition when moving to a disabled state
                    a.snap_to(target).await;
                } else {
                    let from = last.borrow().clone();
                    a.animate_elevation(target, from.as_deref(), interaction.as_deref()).await;
                }
                *last.borrow_mut() = interaction;
            }
        });

        animatable.as_state()
    })
}

// ---- ChipColors ------------------------------------------------------------------------------------------------

/// A chip's colors (Compose's `ChipColors`): its container's, its label's, its leading and trailing icons',
/// enabled and disabled. Equal by its colors.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct ChipColors {
    /// The container's color, enabled.
    pub container_color: Color,
    /// The label's color, enabled.
    pub label_color: Color,
    /// The leading icon's color, enabled.
    pub leading_icon_content_color: Color,
    /// The trailing icon's color, enabled.
    pub trailing_icon_content_color: Color,
    /// The container's color, disabled.
    pub disabled_container_color: Color,
    /// The label's color, disabled.
    pub disabled_label_color: Color,
    /// The leading icon's color, disabled.
    pub disabled_leading_icon_content_color: Color,
    /// The trailing icon's color, disabled.
    pub disabled_trailing_icon_content_color: Color,
}

impl ChipColors {
    /// Colors of one's own (Compose's constructor).
    ///
    /// Kotlin: `ChipColors(containerColor: Color, labelColor: Color, leadingIconContentColor: Color, trailingIconContentColor: Color, disabledContainerColor: Color, disabledLabelColor: Color, disabledLeadingIconContentColor: Color, disabledTrailingIconContentColor: Color)`
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        container_color: Color,
        label_color: Color,
        leading_icon_content_color: Color,
        trailing_icon_content_color: Color,
        disabled_container_color: Color,
        disabled_label_color: Color,
        disabled_leading_icon_content_color: Color,
        disabled_trailing_icon_content_color: Color,
    ) -> ChipColors {
        ChipColors {
            container_color,
            label_color,
            leading_icon_content_color,
            trailing_icon_content_color,
            disabled_container_color,
            disabled_label_color,
            disabled_leading_icon_content_color,
            disabled_trailing_icon_content_color,
        }
    }

    /// The container's color, `enabled` or not (Compose's internal `containerColor(enabled)`).
    pub(crate) fn container_color_for(&self, enabled: bool) -> Color {
        if enabled { self.container_color } else { self.disabled_container_color }
    }

    /// The label's color (Compose's internal `labelColor(enabled)`).
    pub(crate) fn label_color_for(&self, enabled: bool) -> Color {
        if enabled { self.label_color } else { self.disabled_label_color }
    }

    /// The leading icon's color (Compose's internal `leadingIconContentColor(enabled)`).
    pub(crate) fn leading_icon_content_color_for(&self, enabled: bool) -> Color {
        if enabled { self.leading_icon_content_color } else { self.disabled_leading_icon_content_color }
    }

    /// The trailing icon's color (Compose's internal `trailingIconContentColor(enabled)`).
    pub(crate) fn trailing_icon_content_color_for(&self, enabled: bool) -> Color {
        if enabled { self.trailing_icon_content_color } else { self.disabled_trailing_icon_content_color }
    }
}

#[named]
impl ChipColors {
    /// These colors with those given in place of their own (Compose's `copy`): an unspecified one keeps its own.
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] label_color: Color,
        #[default(Color::Unspecified)] leading_icon_content_color: Color,
        #[default(Color::Unspecified)] trailing_icon_content_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_label_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_content_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_content_color: Color,
    ) -> ChipColors {
        ChipColors::new(
            container_color.take_or_else(|| self.container_color),
            label_color.take_or_else(|| self.label_color),
            leading_icon_content_color.take_or_else(|| self.leading_icon_content_color),
            trailing_icon_content_color.take_or_else(|| self.trailing_icon_content_color),
            disabled_container_color.take_or_else(|| self.disabled_container_color),
            disabled_label_color.take_or_else(|| self.disabled_label_color),
            disabled_leading_icon_content_color.take_or_else(|| self.disabled_leading_icon_content_color),
            disabled_trailing_icon_content_color.take_or_else(|| self.disabled_trailing_icon_content_color),
        )
    }
}

// ---- SelectableChipColors --------------------------------------------------------------------------------------

/// A selectable chip's colors (Compose's `SelectableChipColors`): its container's, label's and icons', enabled,
/// disabled, selected (and the container disabled and selected). Its colors are private, as Compose's; equal by
/// them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SelectableChipColors {
    container_color: Color,
    label_color: Color,
    leading_icon_color: Color,
    trailing_icon_color: Color,
    disabled_container_color: Color,
    disabled_label_color: Color,
    disabled_leading_icon_color: Color,
    disabled_trailing_icon_color: Color,
    selected_container_color: Color,
    disabled_selected_container_color: Color,
    selected_label_color: Color,
    selected_leading_icon_color: Color,
    selected_trailing_icon_color: Color,
}

impl SelectableChipColors {
    /// Colors of one's own (Compose's constructor).
    ///
    /// Kotlin: `SelectableChipColors(containerColor: Color, labelColor: Color, leadingIconColor: Color, trailingIconColor: Color, disabledContainerColor: Color, disabledLabelColor: Color, disabledLeadingIconColor: Color, disabledTrailingIconColor: Color, selectedContainerColor: Color, disabledSelectedContainerColor: Color, selectedLabelColor: Color, selectedLeadingIconColor: Color, selectedTrailingIconColor: Color)`
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        container_color: Color,
        label_color: Color,
        leading_icon_color: Color,
        trailing_icon_color: Color,
        disabled_container_color: Color,
        disabled_label_color: Color,
        disabled_leading_icon_color: Color,
        disabled_trailing_icon_color: Color,
        selected_container_color: Color,
        disabled_selected_container_color: Color,
        selected_label_color: Color,
        selected_leading_icon_color: Color,
        selected_trailing_icon_color: Color,
    ) -> SelectableChipColors {
        SelectableChipColors {
            container_color,
            label_color,
            leading_icon_color,
            trailing_icon_color,
            disabled_container_color,
            disabled_label_color,
            disabled_leading_icon_color,
            disabled_trailing_icon_color,
            selected_container_color,
            disabled_selected_container_color,
            selected_label_color,
            selected_leading_icon_color,
            selected_trailing_icon_color,
        }
    }

    /// The container's color (Compose's internal `containerColor(enabled, selected)`): disabled, the disabled
    /// selected or the disabled one; else the selected or the plain one.
    pub(crate) fn container_color_for(&self, enabled: bool, selected: bool) -> Color {
        if !enabled {
            if selected { self.disabled_selected_container_color } else { self.disabled_container_color }
        } else if !selected {
            self.container_color
        } else {
            self.selected_container_color
        }
    }

    /// The label's color (Compose's internal `labelColor(enabled, selected)`).
    pub(crate) fn label_color_for(&self, enabled: bool, selected: bool) -> Color {
        if !enabled {
            self.disabled_label_color
        } else if !selected {
            self.label_color
        } else {
            self.selected_label_color
        }
    }

    /// The leading icon's color (Compose's internal `leadingIconContentColor(enabled, selected)`).
    pub(crate) fn leading_icon_content_color_for(&self, enabled: bool, selected: bool) -> Color {
        if !enabled {
            self.disabled_leading_icon_color
        } else if !selected {
            self.leading_icon_color
        } else {
            self.selected_leading_icon_color
        }
    }

    /// The trailing icon's color (Compose's internal `trailingIconContentColor(enabled, selected)`).
    pub(crate) fn trailing_icon_content_color_for(&self, enabled: bool, selected: bool) -> Color {
        if !enabled {
            self.disabled_trailing_icon_color
        } else if !selected {
            self.trailing_icon_color
        } else {
            self.selected_trailing_icon_color
        }
    }
}

#[named]
impl SelectableChipColors {
    /// These colors with those given in place of their own (Compose's `copy`): an unspecified one keeps its own.
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(Color::Unspecified)] label_color: Color,
        #[default(Color::Unspecified)] leading_icon_color: Color,
        #[default(Color::Unspecified)] trailing_icon_color: Color,
        #[default(Color::Unspecified)] disabled_container_color: Color,
        #[default(Color::Unspecified)] disabled_label_color: Color,
        #[default(Color::Unspecified)] disabled_leading_icon_color: Color,
        #[default(Color::Unspecified)] disabled_trailing_icon_color: Color,
        #[default(Color::Unspecified)] selected_container_color: Color,
        #[default(Color::Unspecified)] disabled_selected_container_color: Color,
        #[default(Color::Unspecified)] selected_label_color: Color,
        #[default(Color::Unspecified)] selected_leading_icon_color: Color,
        #[default(Color::Unspecified)] selected_trailing_icon_color: Color,
    ) -> SelectableChipColors {
        SelectableChipColors::new(
            container_color.take_or_else(|| self.container_color),
            label_color.take_or_else(|| self.label_color),
            leading_icon_color.take_or_else(|| self.leading_icon_color),
            trailing_icon_color.take_or_else(|| self.trailing_icon_color),
            disabled_container_color.take_or_else(|| self.disabled_container_color),
            disabled_label_color.take_or_else(|| self.disabled_label_color),
            disabled_leading_icon_color.take_or_else(|| self.disabled_leading_icon_color),
            disabled_trailing_icon_color.take_or_else(|| self.disabled_trailing_icon_color),
            selected_container_color.take_or_else(|| self.selected_container_color),
            disabled_selected_container_color.take_or_else(|| self.disabled_selected_container_color),
            selected_label_color.take_or_else(|| self.selected_label_color),
            selected_leading_icon_color.take_or_else(|| self.selected_leading_icon_color),
            selected_trailing_icon_color.take_or_else(|| self.selected_trailing_icon_color),
        )
    }
}

// ---- ChipBorder ------------------------------------------------------------------------------------------------

/// A chip's border, enabled and disabled (Compose's deprecated `ChipBorder`; its constructor internal): made by
/// the defaults' deprecated `…_chip_border` functions. Equal by its colors and width.
#[deprecated(note = "Maintained for binary compatibility. Use the chipBorder functions instead")]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChipBorder {
    border_color: Color,
    disabled_border_color: Color,
    border_width: Dp,
}

impl ChipBorder {
    /// Compose's internal constructor `ChipBorder(borderColor, disabledBorderColor, borderWidth)`.
    pub(crate) fn new(border_color: Color, disabled_border_color: Color, border_width: Dp) -> ChipBorder {
        ChipBorder { border_color, disabled_border_color, border_width }
    }

    /// The border, `enabled` or not, as state (Compose's internal composable `borderStroke(enabled)`:
    /// `rememberUpdatedState`).
    #[track_caller]
    pub(crate) fn border_stroke(&self, enabled: bool) -> UpdatedState<Option<BorderStroke>> {
        let border =
            Some(BorderStroke(self.border_width, if enabled { self.border_color } else { self.disabled_border_color }));
        kompozable_block!(remember_updated_state(border))
    }
}

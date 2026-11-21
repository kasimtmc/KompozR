//! Snackbars, as Compose's Snackbar.kt: a brief message at the bottom of the screen with an optional action and
//! dismiss button ([`Snackbar`]), the one a [`SnackbarHost`](crate::SnackbarHost) shows of its data
//! ([`DataSnackbar`]), and their defaults ([`SnackbarDefaults`]). The message and the action share a row (the
//! action on the message's baseline), or the action goes on a line of its own; two lines are 68 dp tall at least,
//! one 48 dp; 600 dp wide at most. (Both of Compose's layouts are here: the styling fix's, the default, and the
//! legacy one, by [`ComposeMaterial3Flags::is_snackbar_styling_fix_enabled`].)

use crate::button::{ButtonDefaults, TextButton};
use crate::content_color::LocalContentColor;
use crate::flags::ComposeMaterial3Flags;
use crate::icon::VectorIcon;
use crate::icon_button::IconButton;
use crate::internal::icons::Icons;
use crate::internal::strings::{Strings, get_string};
use crate::snackbar_host::SnackbarData;
use crate::surface::Surface;
use crate::text::{LocalTextStyle, Text};
use crate::tokens::SnackbarTokens;
use crate::tooltip::{
    TooltipAnchorPosition, TooltipBox, TooltipDefaults, TooltipScope, TooltipScopeComposables, remember_tooltip_state,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, Shape};
use kompoz_runtime::{KompozitionLocalProvider, kompozable, named_args};
use kompoz_text::TextStyle;
use kompoz_ui::layout::{AlignmentLine, FirstBaseline, LastBaseline, Layout, measure_policy};
use kompoz_ui::{Alignment, Modifier};
use std::rc::Rc;
use std::sync::Arc;

/// A snackbar (Compose's `Snackbar(modifier, action, …, content)`): `content` (the message, in the body medium
/// style) beside `action` (in `action_content_color` and the label large style; typically a [`TextButton`]) and
/// `dismiss_action` (in `dismiss_action_content_color`; typically an [`IconButton`]) — the action under the message
/// at the end with `action_on_new_line` (for a long action) — on a [`Surface`] of `shape` in `container_color`,
/// its content in `content_color`, raised 6 dp. See [`DataSnackbar`] for the one a host shows.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn Snackbar(
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] action: Option<Rc<dyn Fn()>>,
    #[default(None)] dismiss_action: Option<Rc<dyn Fn()>>,
    #[default(false)] action_on_new_line: bool,
    #[default(SnackbarDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(SnackbarDefaults::color())] container_color: Color,
    #[default(SnackbarDefaults::content_color())] content_color: Color,
    #[default(SnackbarDefaults::action_content_color())] action_content_color: Color,
    #[default(SnackbarDefaults::dismiss_action_content_color())] dismiss_action_content_color: Color,
    content: impl Fn() + 'static,
) {
    let content: Rc<dyn Fn()> = Rc::new(content);
    Surface(
        modifier = modifier,
        shape = shape,
        color = container_color,
        content_color = Some(content_color),
        shadow_elevation = SnackbarTokens::ContainerElevation,
        content = move || {
            let text_style = SnackbarTokens::SupportingTextFont.value();
            let action_text_style = SnackbarTokens::ActionLabelTextFont.value();
            KompozitionLocalProvider(&[LocalTextStyle.provides(text_style)], move || match &action {
                Some(action) if action_on_new_line => {
                    if ComposeMaterial3Flags::is_snackbar_styling_fix_enabled() {
                        NewLineButtonSnackbar(
                            content.clone(),
                            action.clone(),
                            dismiss_action.clone(),
                            action_text_style.clone(),
                            action_content_color,
                            dismiss_action_content_color,
                        );
                    } else {
                        LegacyNewLineButtonSnackbar(
                            content.clone(),
                            action.clone(),
                            dismiss_action.clone(),
                            action_text_style.clone(),
                            action_content_color,
                            dismiss_action_content_color,
                        );
                    }
                }
                _ => {
                    if ComposeMaterial3Flags::is_snackbar_styling_fix_enabled() {
                        OneRowSnackbar(
                            content.clone(),
                            action.clone(),
                            dismiss_action.clone(),
                            action_text_style.clone(),
                            action_content_color,
                            dismiss_action_content_color,
                        );
                    } else {
                        LegacyOneRowSnackbar(
                            content.clone(),
                            action.clone(),
                            dismiss_action.clone(),
                            action_text_style.clone(),
                            action_content_color,
                            dismiss_action_content_color,
                        );
                    }
                }
            });
        },
    );
}

/// The snackbar of a host's data (Compose's `Snackbar(snackbarData, …)`): `snackbar_data`'s message, its action
/// label as a [`TextButton`] in `action_color` performing its action, a close [`IconButton`] (with a tooltip
/// telling it) dismissing it where its visuals ask for one, as [`Snackbar`] padded 12 dp — the action under the
/// message with `action_on_new_line`.
///
/// Kotlin: `Snackbar(snackbarData: SnackbarData, modifier: Modifier = Modifier, actionOnNewLine: Boolean = false, shape: Shape = SnackbarDefaults.shape, containerColor: Color = SnackbarDefaults.color, contentColor: Color = SnackbarDefaults.contentColor, actionColor: Color = SnackbarDefaults.actionColor, actionContentColor: Color = SnackbarDefaults.actionContentColor, dismissActionContentColor: Color = SnackbarDefaults.dismissActionContentColor)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn DataSnackbar(
    snackbar_data: Rc<dyn SnackbarData>,
    #[default(Modifier)] modifier: Modifier,
    #[default(false)] action_on_new_line: bool,
    #[default(SnackbarDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(SnackbarDefaults::color())] container_color: Color,
    #[default(SnackbarDefaults::content_color())] content_color: Color,
    #[default(SnackbarDefaults::action_color())] action_color: Color,
    #[default(SnackbarDefaults::action_content_color())] action_content_color: Color,
    #[default(SnackbarDefaults::dismiss_action_content_color())] dismiss_action_content_color: Color,
) {
    let visuals = snackbar_data.visuals();
    let action_label = visuals.action_label();
    let action_composable: Option<Rc<dyn Fn()>> = match action_label {
        Some(action_label) => {
            let data = snackbar_data.clone();
            Some(Rc::new(move || {
                let (data, action_label): (Rc<dyn SnackbarData>, String) = (data.clone(), action_label.clone());
                TextButton(
                    colors = named_args!(ButtonDefaults.text_button_colors(content_color = action_color)),
                    on_click = move || data.perform_action(),
                    content = move || Text(action_label.clone()),
                );
            }) as Rc<dyn Fn()>)
        }
        None => None,
    };
    let dismiss_action_composable: Option<Rc<dyn Fn()>> = if visuals.with_dismiss_action() {
        let data = snackbar_data.clone();
        Some(Rc::new(move || {
            let content_description = get_string(Strings::SnackbarDismiss);
            let (data, tooltip_text) = (data.clone(), content_description.clone());
            TooltipBox(
                position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                tooltip = move |scope: &dyn TooltipScope| {
                    let text = tooltip_text.clone();
                    scope.PlainTooltip(content = move || Text(text.clone()));
                },
                state = remember_tooltip_state(),
                content = move || {
                    let (data, content_description) = (data.clone(), content_description.clone());
                    IconButton(
                        on_click = move || data.dismiss(),
                        content = move || {
                            VectorIcon(Icons::Filled::Close(), content_description = Some(content_description.clone()))
                        },
                    );
                },
            );
        }) as Rc<dyn Fn()>)
    } else {
        None
    };
    let message = visuals.message();
    Snackbar(
        modifier = modifier.padding(Dp(12.0)),
        action = action_composable,
        dismiss_action = dismiss_action_composable,
        action_on_new_line = action_on_new_line,
        shape = shape,
        container_color = container_color,
        content_color = content_color,
        action_content_color = action_content_color,
        dismiss_action_content_color = dismiss_action_content_color,
        content = move || Text(message.clone()),
    );
}

/// The action under the message, at the end (Compose's private `NewLineButtonSnackbar`).
#[kompozable]
fn NewLineButtonSnackbar(
    text: Rc<dyn Fn()>,
    action: Rc<dyn Fn()>,
    dismiss_action: Option<Rc<dyn Fn()>>,
    action_text_style: TextStyle,
    action_content_color: Color,
    dismiss_action_content_color: Color,
) {
    Column(
        modifier = Modifier.width_in(max = ContainerMaxWidth).fill_max_width().padding_sides(start = HorizontalSpacing),
        content = || {
            Box(
                modifier = Modifier
                    .fill_max_width()
                    .padding_hv(vertical = SnackbarVerticalPadding)
                    .padding_sides(end = HorizontalSpacing),
                content = || text(),
            );

            let end = if dismiss_action.is_none() { HorizontalSpacingButtonSide } else { Dp(0.0) };
            Row(
                modifier = Modifier.align(Alignment::End).padding_sides(bottom = ActionButtonBottomPadding, end = end),
                vertical_alignment = Alignment::CenterVertically,
                content = || {
                    KompozitionLocalProvider(
                        &[
                            LocalContentColor.provides(action_content_color),
                            LocalTextStyle.provides(action_text_style.clone()),
                        ],
                        &*action,
                    );
                    if let Some(dismiss_action) = &dismiss_action {
                        KompozitionLocalProvider(
                            &[LocalContentColor.provides(dismiss_action_content_color)],
                            &**dismiss_action,
                        );
                    }
                },
            );
        },
    );
}

/// The legacy layout of the action under the message (Compose's private `LegacyNewLineButtonSnackbar`).
#[kompozable]
fn LegacyNewLineButtonSnackbar(
    text: Rc<dyn Fn()>,
    action: Rc<dyn Fn()>,
    dismiss_action: Option<Rc<dyn Fn()>>,
    action_text_style: TextStyle,
    action_content_color: Color,
    dismiss_action_content_color: Color,
) {
    Column(
        modifier = Modifier
            // Fill max width, up to ContainerMaxWidth.
            .width_in(max = ContainerMaxWidth)
            .fill_max_width()
            .padding_sides(start = HorizontalSpacing, bottom = SeparateButtonExtraY),
        content = || {
            Box(
                modifier = Modifier
                    .padding_from_baseline(HeightToFirstLine, LongButtonVerticalOffset)
                    .padding_sides(end = HorizontalSpacingButtonSide),
                content = || text(),
            );

            let end = if dismiss_action.is_none() { HorizontalSpacingButtonSide } else { Dp(0.0) };
            Box(
                modifier = Modifier.align(Alignment::End).padding_sides(end = end),
                content = || {
                    Row(content = || {
                        KompozitionLocalProvider(
                            &[
                                LocalContentColor.provides(action_content_color),
                                LocalTextStyle.provides(action_text_style.clone()),
                            ],
                            &*action,
                        );
                        if let Some(dismiss_action) = &dismiss_action {
                            KompozitionLocalProvider(
                                &[LocalContentColor.provides(dismiss_action_content_color)],
                                &**dismiss_action,
                            );
                        }
                    });
                },
            );
        },
    );
}

/// The message, the action and the dismiss button's Boxes of a one-row layout (both layouts' content lambda).
#[kompozable]
fn OneRowChildren(
    text: Rc<dyn Fn()>,
    text_vertical_padding: Dp,
    action: Option<Rc<dyn Fn()>>,
    dismiss_action: Option<Rc<dyn Fn()>>,
    action_text_style: TextStyle,
    action_text_color: Color,
    dismiss_action_color: Color,
) {
    Box(modifier = Modifier.layout_id(TextTag).padding_hv(vertical = text_vertical_padding), content = || text());
    if let Some(action) = &action {
        Box(
            modifier = Modifier.layout_id(ActionTag),
            content = || {
                KompozitionLocalProvider(
                    &[
                        LocalContentColor.provides(action_text_color),
                        LocalTextStyle.provides(action_text_style.clone()),
                    ],
                    &**action,
                );
            },
        );
    }
    if let Some(dismiss_action) = &dismiss_action {
        Box(
            modifier = Modifier.layout_id(DismissActionTag),
            content = || {
                KompozitionLocalProvider(&[LocalContentColor.provides(dismiss_action_color)], &**dismiss_action);
            },
        );
    }
}

const TextTag: &str = "text";
const ActionTag: &str = "action";
const DismissActionTag: &str = "dismissAction";

/// The legacy one-row layout (Compose's private `LegacyOneRowSnackbar`): one line of message centered in 48 dp, the
/// action on its baseline; more lines from 30 dp to the first baseline in 68 dp at least, the action centered.
#[kompozable]
fn LegacyOneRowSnackbar(
    text: Rc<dyn Fn()>,
    action: Option<Rc<dyn Fn()>>,
    dismiss_action: Option<Rc<dyn Fn()>>,
    action_text_style: TextStyle,
    action_text_color: Color,
    dismiss_action_color: Color,
) {
    let end = if dismiss_action.is_none() { HorizontalSpacingButtonSide } else { Dp(0.0) };
    Layout(
        content = move || {
            OneRowChildren(
                text.clone(),
                LegacySnackbarVerticalPadding,
                action.clone(),
                dismiss_action.clone(),
                action_text_style.clone(),
                action_text_color,
                dismiss_action_color,
            )
        },
        modifier = Modifier.padding_sides(start = HorizontalSpacing, end = end),
        measure_policy = measure_policy(|scope, measurables, constraints| {
            let container_width = constraints.max_width.min(scope.round_to_px(ContainerMaxWidth));
            let find = |tag: &'static str| measurables.iter().find(|m| m.layout_id().is_some_and(|id| id.is(&tag)));
            let action_button_placeable = find(ActionTag).map(|m| m.measure(constraints));
            let dismiss_button_placeable = find(DismissActionTag).map(|m| m.measure(constraints));
            let action_button_width = action_button_placeable.as_ref().map_or(0, |p| p.width());
            let action_button_height = action_button_placeable.as_ref().map_or(0, |p| p.height());
            let dismiss_button_width = dismiss_button_placeable.as_ref().map_or(0, |p| p.width());
            let dismiss_button_height = dismiss_button_placeable.as_ref().map_or(0, |p| p.height());
            let extra_spacing_width =
                if dismiss_button_width == 0 { scope.round_to_px(TextEndExtraSpacing) } else { 0 };
            let text_max_width = (container_width - action_button_width - dismiss_button_width - extra_spacing_width)
                .max(constraints.min_width);
            let text_placeable = find(TextTag)
                .expect("Snackbar: its text")
                .measure(named_args!(constraints.copy(min_height = 0, max_width = text_max_width)));

            let first_text_baseline = text_placeable.get(FirstBaseline);
            let last_text_baseline = text_placeable.get(LastBaseline);
            let has_text =
                first_text_baseline != AlignmentLine::Unspecified && last_text_baseline != AlignmentLine::Unspecified;
            let is_one_line = first_text_baseline == last_text_baseline || !has_text;
            let dismiss_button_place_x = container_width - dismiss_button_width;
            let action_button_place_x = dismiss_button_place_x - action_button_width;

            let (text_place_y, container_height, action_button_place_y) = if is_one_line {
                let min_container_height = scope.round_to_px(SnackbarTokens::SingleLineContainerHeight);
                let content_height = action_button_height.max(dismiss_button_height);
                let container_height = min_container_height.max(content_height);
                let text_place_y = (container_height - text_placeable.height()) / 2;
                let action_button_place_y = match &action_button_placeable {
                    Some(action_button_placeable) => {
                        let it = action_button_placeable.get(FirstBaseline);
                        if it != AlignmentLine::Unspecified { text_place_y + first_text_baseline - it } else { 0 }
                    }
                    None => 0,
                };
                (text_place_y, container_height, action_button_place_y)
            } else {
                let baseline_offset = scope.round_to_px(HeightToFirstLine);
                let text_place_y = baseline_offset - first_text_baseline;
                let min_container_height = scope.round_to_px(SnackbarTokens::TwoLinesContainerHeight);
                let content_height = text_place_y + text_placeable.height();
                let container_height = min_container_height.max(content_height);
                let action_button_place_y = match &action_button_placeable {
                    Some(action_button_placeable) => (container_height - action_button_placeable.height()) / 2,
                    None => 0,
                };
                (text_place_y, container_height, action_button_place_y)
            };
            let dismiss_button_place_y = match &dismiss_button_placeable {
                Some(dismiss_button_placeable) => (container_height - dismiss_button_placeable.height()) / 2,
                None => 0,
            };

            scope.layout(container_width, container_height, move |p| {
                named_args!(p.place_relative(&text_placeable, 0, text_place_y));
                if let Some(it) = &action_button_placeable {
                    named_args!(p.place_relative(it, action_button_place_x, action_button_place_y));
                }
                if let Some(it) = &dismiss_button_placeable {
                    named_args!(p.place_relative(it, dismiss_button_place_x, dismiss_button_place_y));
                }
            })
        }),
    );
}

/// The one-row layout (Compose's private `OneRowSnackbar`): the message, the action and the dismiss button
/// centered in a row 48 dp tall at least, the message padded 14 dp above and below.
#[kompozable]
fn OneRowSnackbar(
    text: Rc<dyn Fn()>,
    action: Option<Rc<dyn Fn()>>,
    dismiss_action: Option<Rc<dyn Fn()>>,
    action_text_style: TextStyle,
    action_text_color: Color,
    dismiss_action_color: Color,
) {
    let end = if dismiss_action.is_none() { TextEndExtraSpacing } else { Dp(0.0) };
    Layout(
        content = move || {
            OneRowChildren(
                text.clone(),
                SnackbarVerticalPadding,
                action.clone(),
                dismiss_action.clone(),
                action_text_style.clone(),
                action_text_color,
                dismiss_action_color,
            )
        },
        modifier = Modifier.padding_sides(start = HorizontalSpacing, end = end),
        measure_policy = measure_policy(|scope, measurables, constraints| {
            let min_container_height = scope.round_to_px(SnackbarTokens::SingleLineContainerHeight);
            let container_width = constraints.max_width.min(scope.round_to_px(ContainerMaxWidth));
            let find = |tag: &'static str| measurables.iter().find(|m| m.layout_id().is_some_and(|id| id.is(&tag)));
            let action_button_placeable = find(ActionTag).map(|m| m.measure(constraints));
            let dismiss_button_placeable = find(DismissActionTag).map(|m| m.measure(constraints));
            let action_button_width = action_button_placeable.as_ref().map_or(0, |p| p.width());
            let dismiss_button_width = dismiss_button_placeable.as_ref().map_or(0, |p| p.width());

            let extra_spacing_width =
                if dismiss_button_width == 0 { scope.round_to_px(TextEndExtraSpacing) } else { 0 };
            let text_max_width = (container_width - action_button_width - dismiss_button_width - extra_spacing_width)
                .max(constraints.min_width);
            let text_placeable = find(TextTag)
                .expect("Snackbar: its text")
                .measure(named_args!(constraints.copy(min_height = 0, max_width = text_max_width)));

            let container_height = min_container_height
                .max(text_placeable.height())
                .max(action_button_placeable.as_ref().map_or(0, |p| p.height()))
                .max(dismiss_button_placeable.as_ref().map_or(0, |p| p.height()));

            let dismiss_button_place_x = container_width - dismiss_button_width;
            let action_button_place_x = dismiss_button_place_x - action_button_width;

            scope.layout(container_width, container_height, move |p| {
                named_args!(p.place_relative(&text_placeable, 0, (container_height - text_placeable.height()) / 2));
                if let Some(it) = &action_button_placeable {
                    named_args!(p.place_relative(it, action_button_place_x, (container_height - it.height()) / 2));
                }
                if let Some(it) = &dismiss_button_placeable {
                    named_args!(p.place_relative(it, dismiss_button_place_x, (container_height - it.height()) / 2));
                }
            })
        }),
    );
}

/// The defaults of snackbars (Compose's `object SnackbarDefaults`).
#[derive(Clone, Copy, Debug)]
pub struct SnackbarDefaults;

impl SnackbarDefaults {
    /// A snackbar's shape: the theme's extra small corners (Compose's `shape`).
    pub fn shape() -> Arc<dyn Shape> {
        SnackbarTokens::ContainerShape.value()
    }

    /// A snackbar's container color: the inverse surface (Compose's `color`).
    pub fn color() -> Color {
        SnackbarTokens::ContainerColor.value()
    }

    /// A snackbar's content color: the inverse on-surface (Compose's `contentColor`).
    pub fn content_color() -> Color {
        SnackbarTokens::SupportingTextColor.value()
    }

    /// A snackbar's action color: the inverse primary (Compose's `actionColor`).
    pub fn action_color() -> Color {
        SnackbarTokens::ActionLabelTextColor.value()
    }

    /// A snackbar's action content color: the inverse primary (Compose's `actionContentColor`).
    pub fn action_content_color() -> Color {
        SnackbarTokens::ActionLabelTextColor.value()
    }

    /// A snackbar's dismiss action content color: the inverse on-surface (Compose's `dismissActionContentColor`).
    pub fn dismiss_action_content_color() -> Color {
        SnackbarTokens::IconColor.value()
    }
}

const ContainerMaxWidth: Dp = Dp(600.0);
const HeightToFirstLine: Dp = Dp(30.0);
const HorizontalSpacing: Dp = Dp(16.0);
const HorizontalSpacingButtonSide: Dp = Dp(8.0);
const SeparateButtonExtraY: Dp = Dp(2.0);
const LegacySnackbarVerticalPadding: Dp = Dp(6.0);
const TextEndExtraSpacing: Dp = Dp(8.0);
const LongButtonVerticalOffset: Dp = Dp(12.0);
const SnackbarVerticalPadding: Dp = Dp(14.0);
const ActionButtonBottomPadding: Dp = Dp(4.0);

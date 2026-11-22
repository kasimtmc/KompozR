//! The time picker's dialog, as Compose's TimePickerDialog.kt (material3 1.5.0-alpha17): a dialog for a
//! [`TimePicker`](crate::TimePicker) or [`TimeInput`](crate::TimeInput) ([`TimePickerDialog`]: its title above it,
//! its buttons under it, or beside it in landscape), its defaults ([`TimePickerDialogDefaults`]: the shape, the
//! container color, the title and the button toggling between the dial and the text input) and which of the two it
//! shows ([`TimePickerDisplayMode`]).

use crate::icon::VectorIcon;
use crate::icon_button::IconButton;
use crate::internal::icons::Icons;
use crate::internal::strings::{Strings, get_string};
use crate::material_theme::MaterialTheme;
use crate::surface::Surface;
use crate::text::Text;
use crate::time_picker::ClockDialMinContainerSize;
use crate::tokens::DialogTokens;
use crate::tooltip::{
    TooltipAnchorPosition, TooltipBox, TooltipDefaults, TooltipScope, TooltipScopeComposables, remember_tooltip_state,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, Shape};
use kompoz_runtime::{kompozable, kompozable_block, named, named_args};
use kompoz_ui::Modifier;
use kompoz_ui::layout::{Layout, Measurable, measure_policy};
use kompoz_ui::window::{Dialog, DialogProperties};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// A dialog for a time picker (Compose's `TimePickerDialog`): in a [`Dialog`] (dismissed by `on_dismiss_request` on
/// back or a press outside, as `properties` say — by default not held to the platform's width), a surface of `shape`
/// in `container_color` holding `title` above `content` (a [`TimePicker`](crate::TimePicker) or
/// [`TimeInput`](crate::TimeInput)) and the buttons under it — `mode_toggle_button` at the start, `dismiss_button`
/// and `confirm_button` at the end, 8 dp apart — or, for a dial picker wider than tall, the title and the buttons
/// beside the picker.
///
/// Kotlin: `TimePickerDialog(onDismissRequest: () -> Unit, confirmButton: @Composable () -> Unit, title: @Composable () -> Unit, modifier: Modifier = Modifier, properties: DialogProperties = DialogProperties(usePlatformDefaultWidth = false), modeToggleButton: @Composable (() -> Unit)? = null, dismissButton: @Composable (() -> Unit)? = null, shape: Shape = TimePickerDialogDefaults.shape, containerColor: Color = TimePickerDialogDefaults.containerColor, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TimePickerDialog(
    on_dismiss_request: impl Fn() + 'static,
    confirm_button: impl Fn() + 'static,
    title: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(DialogProperties { use_platform_default_width: false, ..DialogProperties::default() })]
    properties: DialogProperties,
    #[default(None)] mode_toggle_button: Option<Rc<dyn Fn()>>,
    #[default(None)] dismiss_button: Option<Rc<dyn Fn()>>,
    #[default(TimePickerDialogDefaults.shape())] shape: Arc<dyn Shape>,
    #[default(TimePickerDialogDefaults.container_color())] container_color: Color,
    content: impl Fn() + 'static,
) {
    let confirm_button: Rc<dyn Fn()> = Rc::new(confirm_button);
    let title: Rc<dyn Fn()> = Rc::new(title);
    let content: Rc<dyn Fn()> = Rc::new(content);
    Dialog(
        on_dismiss_request,
        properties = properties,
        content = move || {
            TimePickerDialogLayout(
                confirm_button.clone(),
                title.clone(),
                modifier.clone(),
                mode_toggle_button.clone(),
                dismiss_button.clone(),
                shape.clone(),
                container_color,
                content.clone(),
            );
        },
    );
}

/// The dialog's surface and layout (Compose's internal `TimePickerDialogLayout`): a surface of `shape` in
/// `container_color`, the dialog's tonal elevation, holding the title, the content and a row of the buttons (the
/// mode toggle at the start, the dismiss and confirm buttons at the end, 8 dp apart).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn TimePickerDialogLayout(
    confirm_button: Rc<dyn Fn()>,
    title: Rc<dyn Fn()>,
    modifier: Modifier,
    mode_toggle_button: Option<Rc<dyn Fn()>>,
    dismiss_button: Option<Rc<dyn Fn()>>,
    shape: Arc<dyn Shape>,
    container_color: Color,
    content: Rc<dyn Fn()>,
) {
    Surface(
        shape = shape,
        tonal_elevation = DialogTokens::ContainerElevation,
        modifier = modifier,
        color = container_color,
        content = move || {
            let (mode_toggle_button, dismiss_button, confirm_button) =
                (mode_toggle_button.clone(), dismiss_button.clone(), confirm_button.clone());
            TimePickerCustomLayout(
                title.clone(),
                Rc::new(move || {
                    let (mode_toggle_button, dismiss_button, confirm_button) =
                        (mode_toggle_button.clone(), dismiss_button.clone(), confirm_button.clone());
                    Row(
                        modifier = Modifier.fill_max_width(),
                        horizontal_arrangement = Arrangement::spaced_by(Dp(8.0)),
                        content = move || {
                            if let Some(mode_toggle_button) = &mode_toggle_button {
                                mode_toggle_button();
                            }
                            Spacer(Modifier.weight(1.0));
                            if let Some(dismiss_button) = &dismiss_button {
                                dismiss_button();
                            }
                            confirm_button();
                        },
                    );
                }),
                content.clone(),
            );
        },
    );
}

/// Whether a measurable is the dialog's part of `id` (Compose's `layoutId == id`).
fn is_part(measurable: &dyn Measurable, id: &'static str) -> bool {
    measurable.layout_id().is_some_and(|it| it.is(&id))
}

/// The dialog's layout (Compose's internal `TimePickerCustomLayout`): the content 24 dp in at its sides; in portrait
/// the title 24 dp in above it and the actions centered under it, 24 dp to spare below; in landscape (content wider
/// than tall and at least as tall as the smallest dial) the title at the top start, the content and the actions
/// under it centered in the height (all the height there is, if bounded).
#[kompozable]
pub(crate) fn TimePickerCustomLayout(title: Rc<dyn Fn()>, actions: Rc<dyn Fn()>, content: Rc<dyn Fn()>) {
    let content = move || {
        let (title, actions, content) = (title.clone(), actions.clone(), content.clone());
        Box(modifier = Modifier.layout_id("title"), content = move || title());
        Box(modifier = Modifier.layout_id("actions"), content = move || actions());
        Column(modifier = Modifier.layout_id("timePickerContent"), content = move || content());
    };

    let measure_policy = measure_policy(|scope, measurables, constraints| {
        let find = |id: &'static str| {
            measurables.iter().copied().find(|m| is_part(*m, id)).expect("the time picker dialog's part")
        };
        let title_measurable = find("title");
        let content_measurable = find("timePickerContent");
        let actions_measurable = find("actions");

        let content_padding = scope.round_to_px(Dp(24.0));
        let land_max_dialog_height = scope.round_to_px(Dp(384.0));
        let land_title_top_padding = scope.round_to_px(Dp(24.0));
        let land_content_top_padding = scope.round_to_px(Dp(16.0));
        let land_content_actions_padding = scope.round_to_px(Dp(4.0));
        let land_actions_bottom_padding = scope.round_to_px(Dp(8.0));

        let port_title_top_padding = scope.round_to_px(Dp(24.0));
        let port_actions_bottom_padding = scope.round_to_px(Dp(24.0));

        let content_placeable = content_measurable.measure(named_args!(constraints.copy(min_height = 0)));

        // Input mode will be smaller than the smallest TimePickerContent (currently 200.dp)
        // But will always use portrait layout for correctness.
        let is_landscape = content_placeable.width() > content_placeable.height()
            && content_placeable.height() as f32 >= scope.to_px(ClockDialMinContainerSize).trunc();

        // (Compose's landscape and portrait widths are the same)
        let dialog_width = content_placeable.width() + content_padding * 2;

        let part_constraints =
            named_args!(constraints.copy(min_width = 0, min_height = 0, max_width = content_placeable.width()));
        let actions_placeable = actions_measurable.measure(part_constraints);

        let title_placeable = title_measurable.measure(part_constraints);

        let layout_height = if is_landscape {
            let content_total_height = content_placeable.height()
                + actions_placeable.height()
                + land_actions_bottom_padding
                + land_content_top_padding
                + land_content_actions_padding;
            if constraints.has_bounded_height() { constraints.max_height } else { content_total_height }
        } else {
            port_title_top_padding
                + title_placeable.height()
                + content_placeable.height()
                + actions_placeable.height()
                + port_actions_bottom_padding
        };

        let adjusted_actions_bottom_padding =
            if layout_height >= land_max_dialog_height { scope.round_to_px(Dp(16.0)) } else { 0 };
        scope.layout(dialog_width, layout_height, move |p| {
            if is_landscape {
                let content_height = land_content_top_padding
                    + content_placeable.height()
                    + land_content_actions_padding
                    + actions_placeable.height()
                    + land_actions_bottom_padding;
                let remaining_space = layout_height - content_height;

                p.place(&title_placeable, land_title_top_padding, land_title_top_padding);
                let time_picker_content_x = content_padding;
                let time_picker_content_y = land_content_top_padding + remaining_space / 2;
                p.place(&content_placeable, time_picker_content_x, time_picker_content_y);
                let actions_y = time_picker_content_y + content_placeable.height() + land_content_actions_padding
                    - adjusted_actions_bottom_padding
                    + remaining_space / 2;
                p.place(&actions_placeable, time_picker_content_x, actions_y);
            } else {
                let title_x = land_title_top_padding;
                p.place(&title_placeable, title_x, port_title_top_padding);

                let content_x = (dialog_width - content_placeable.width()) / 2;
                let content_y = port_title_top_padding + title_placeable.height();
                p.place(&content_placeable, content_x, content_y);

                let actions_x = (dialog_width - actions_placeable.width()) / 2;
                let actions_y = content_y + content_placeable.height();
                p.place(&actions_placeable, actions_x, actions_y);
            }
        })
    });

    Layout(content = content, measure_policy = measure_policy);
}

/// The time picker dialog's defaults (Compose's `TimePickerDialogDefaults`): its container color and shape, the
/// least window height for the dial ([`MinHeightForTimePicker`](Self::MinHeightForTimePicker)), its title and its
/// display mode toggle. A unit value: `TimePickerDialogDefaults.shape()`.
#[derive(Clone, Copy, Debug)]
pub struct TimePickerDialogDefaults;

impl TimePickerDialogDefaults {
    /// The least window height for a dial picker in the dialog (Compose's `MinHeightForTimePicker`): 300 dp.
    pub const MinHeightForTimePicker: Dp = Dp(300.0);
}

#[named]
impl TimePickerDialogDefaults {
    /// The dialog's container color: the high surface container (Compose's composable `containerColor`).
    #[track_caller]
    pub fn container_color(&self) -> Color {
        DialogTokens::ContainerColor.value()
    }

    /// The dialog's shape: the theme's extra large corners, 28 dp (Compose's composable `shape`).
    #[track_caller]
    pub fn shape(&self) -> Arc<dyn Shape> {
        DialogTokens::ContainerShape.value()
    }

    /// The button toggling between the dial and the text input (Compose's composable `DisplayModeToggle`): an icon
    /// button showing a keyboard while `display_mode` is the dial's ("Switch to text input mode") and a clock while it
    /// is the input's ("Switch to clock mode"), in a tooltip box whose plain tooltip, above it, says the same;
    /// `on_display_mode_change` runs on a click.
    #[track_caller]
    pub fn DisplayModeToggle(
        &self,
        on_display_mode_change: impl Fn() + 'static,
        display_mode: TimePickerDisplayMode,
        #[default(Modifier)] modifier: Modifier,
    ) {
        kompozable_block!({
            TimePickerDisplayModeToggle(Rc::new(on_display_mode_change), display_mode, modifier);
        })
    }

    /// The dialog's title (Compose's composable `Title`): "Select time" for the dial, "Enter time" for the input, in
    /// the label medium style, 20 dp above what follows.
    #[track_caller]
    pub fn Title(&self, display_mode: TimePickerDisplayMode, #[default(Modifier)] modifier: Modifier) {
        kompozable_block!({
            TimePickerDialogTitle(display_mode, modifier);
        })
    }
}

/// The restartable body of [`TimePickerDialogDefaults::DisplayModeToggle`].
#[kompozable]
fn TimePickerDisplayModeToggle(
    on_display_mode_change: Rc<dyn Fn()>,
    display_mode: TimePickerDisplayMode,
    modifier: Modifier,
) {
    let content_description = get_string(if display_mode == TimePickerDisplayMode::Picker {
        Strings::TimePickerToggleKeyboard
    } else {
        Strings::TimePickerToggleTouch
    });
    let tooltip_text = content_description.clone();
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
        tooltip = move |s: &dyn TooltipScope| {
            let text = tooltip_text.clone();
            s.PlainTooltip(content = move || Text(text.clone()));
        },
        state = remember_tooltip_state(),
        content = move || {
            let (on_display_mode_change, content_description) =
                (on_display_mode_change.clone(), content_description.clone());
            IconButton(
                modifier = modifier.clone(),
                on_click = move || on_display_mode_change(),
                content = move || {
                    let icon = if display_mode == TimePickerDisplayMode::Picker {
                        Icons::Outlined::Keyboard()
                    } else {
                        Icons::Outlined::Schedule()
                    };
                    VectorIcon(icon, Some(content_description.clone()));
                },
            );
        },
    );
}

/// The restartable body of [`TimePickerDialogDefaults::Title`].
#[kompozable]
fn TimePickerDialogTitle(display_mode: TimePickerDisplayMode, modifier: Modifier) {
    Text(
        get_string(if display_mode == TimePickerDisplayMode::Picker {
            Strings::TimePickerDialogTitle
        } else {
            Strings::TimeInputDialogTitle
        }),
        modifier = modifier.padding_sides(bottom = Dp(20.0)),
        style = MaterialTheme::typography().label_medium,
    );
}

/// What a time picker dialog shows (Compose's value class `TimePickerDisplayMode`): the dial
/// ([`Picker`](Self::Picker)) or the text input ([`Input`](Self::Input)).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimePickerDisplayMode(i32);

impl TimePickerDisplayMode {
    /// The dial picker.
    pub const Picker: TimePickerDisplayMode = TimePickerDisplayMode(0);
    /// The text input.
    pub const Input: TimePickerDisplayMode = TimePickerDisplayMode(1);
}

/// Compose's toString: the mode's name.
impl fmt::Display for TimePickerDisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            TimePickerDisplayMode::Picker => "Picker",
            TimePickerDisplayMode::Input => "Input",
            _ => "Unknown",
        })
    }
}

impl fmt::Debug for TimePickerDisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

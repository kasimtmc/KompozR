//! A dialog for a date picker, as Compose's DatePickerDialog.kt (material3 1.5.0-alpha17) with Compose for
//! desktop's actual (skikoMain's DatePickerDialog.skiko.kt).

use std::rc::Rc;
use std::sync::Arc;

use kompoz_foundation::prelude::*;
use kompoz_graphics::{Dp, Shape};
use kompoz_runtime::{kompozable, named_args};
use kompoz_ui::window::DialogProperties;
use kompoz_ui::{Alignment, Modifier};

use crate::alert_dialog::{AlertDialogFlowRow, BasicAlertDialog};
use crate::date_picker::{DatePickerColors, DatePickerDefaults};
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::surface::Surface;
use crate::tokens::{DatePickerModalTokens, DialogTokens};

/// A dialog showing a date picker (Compose's `DatePickerDialog`; desktop's actual): a [`BasicAlertDialog`] of a
/// surface exactly 360 dp wide and at most 568 dp tall in the picker's container color, `content` over the buttons
/// — `dismiss_button` (if any) then `confirm_button`, at its end, in the dialog's action style. The content gives way
/// to the buttons on a small screen. `on_dismiss_request` is called on back or a press outside (not by the dismiss
/// button).
///
/// Kotlin: `DatePickerDialog(onDismissRequest: () -> Unit, confirmButton: @Composable () -> Unit, modifier: Modifier = Modifier, dismissButton: @Composable (() -> Unit)? = null, shape: Shape = DatePickerDefaults.shape, tonalElevation: Dp = DatePickerDefaults.TonalElevation, colors: DatePickerColors = DatePickerDefaults.colors(), properties: DialogProperties = DialogProperties(usePlatformDefaultWidth = false), content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
pub fn DatePickerDialog(
    on_dismiss_request: impl Fn() + 'static,
    confirm_button: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] dismiss_button: Option<Rc<dyn Fn()>>,
    #[default(DatePickerDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(DatePickerDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(DatePickerDefaults.colors())] colors: DatePickerColors,
    #[default(DialogProperties { use_platform_default_width: false, ..DialogProperties::default() })]
    properties: DialogProperties,
    content: impl Fn() + 'static,
) {
    let (confirm_button, content) = (Rc::new(confirm_button), Rc::new(content));
    BasicAlertDialog(
        on_dismiss_request = on_dismiss_request,
        modifier = named_args!(modifier.wrap_content_height()),
        properties = properties,
        content = move || {
            let (confirm_button, dismiss_button, content) =
                (confirm_button.clone(), dismiss_button.clone(), content.clone());
            Surface(
                modifier = named_args!(
                    Modifier
                        .required_width(DatePickerModalTokens::ContainerWidth)
                        .height_in(max = DatePickerModalTokens::ContainerHeight)
                ),
                shape = shape.clone(),
                color = colors.container_color,
                tonal_elevation = tonal_elevation,
                content = move || {
                    let (confirm_button, dismiss_button, content) =
                        (confirm_button.clone(), dismiss_button.clone(), content.clone());
                    Column(
                        vertical_arrangement = Arrangement::SpaceBetween,
                        content = move || {
                            // Wrap the content with a Box and Modifier.weight(1f) to ensure that any "confirm" and
                            // "dismiss" buttons are not pushed out of view when running on small screens, or when
                            // nesting a DateRangePicker. Fill is false to support collapsing the dialog's height
                            // when switching to input mode.
                            let content = content.clone();
                            Box(modifier = Modifier.weight_with(1.0, false), content = move || content());
                            // Buttons
                            let (confirm_button, dismiss_button) = (confirm_button.clone(), dismiss_button.clone());
                            Box(
                                modifier = Modifier.align(Alignment::End).padding_values(DialogButtonsPadding()),
                                content = move || {
                                    let (confirm_button, dismiss_button) =
                                        (confirm_button.clone(), dismiss_button.clone());
                                    ProvideContentColorTextStyle(
                                        DialogTokens::ActionLabelTextColor.value(),
                                        DialogTokens::ActionLabelTextFont.value(),
                                        move || {
                                            let (confirm_button, dismiss_button) =
                                                (confirm_button.clone(), dismiss_button.clone());
                                            AlertDialogFlowRow(
                                                DialogButtonsMainAxisSpacing,
                                                DialogButtonsCrossAxisSpacing,
                                                move || {
                                                    if let Some(dismiss_button) = &dismiss_button {
                                                        dismiss_button();
                                                    }
                                                    confirm_button();
                                                },
                                            );
                                        },
                                    );
                                },
                            );
                        },
                    );
                },
            );
        },
    );
}

/// Compose for desktop's `DialogButtonsPadding`: 8 dp below, 6 dp at the end.
fn DialogButtonsPadding() -> PaddingValues {
    PaddingValues::new(Dp(0.0), Dp(0.0), Dp(6.0), Dp(8.0))
}

const DialogButtonsMainAxisSpacing: Dp = Dp(8.0);
const DialogButtonsCrossAxisSpacing: Dp = Dp(12.0);

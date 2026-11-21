//! Material's dialogs, as Compose's AlertDialog.kt with Compose for desktop's AlertDialog.skiko.kt: an alert dialog
//! ([`AlertDialog`]: an icon, a title, a text and the buttons, laid out as Material says on a rounded surface) and
//! the dialog any content makes ([`BasicAlertDialog`]: 280 to 560 dp wide, a pane titled "Dialog" to
//! accessibility). How a basic alert dialog is made can be replaced below a [`LocalBasicAlertDialogOverride`]
//! (Compose's experimental component override API).

// (the deprecated AlertDialog(content) overload is defined here)
#![allow(deprecated)]

use crate::button::ButtonDefaults;
use crate::content_color::LocalContentColor;
use crate::interactive_component_size::LocalMinimumInteractiveComponentSize;
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::internal::strings::{Strings, get_string};
use crate::material_theme::MaterialTheme;
use crate::precision_pointer::should_use_precision_pointer_component_sizing_state;
use crate::surface::Surface;
use crate::tokens::DialogTokens;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, LayoutDirection, Shape, UnitExt};
use kompoz_runtime::{KompozitionLocal, KompozitionLocalProvider, kompozable, kompozable_block, named, named_args};
use kompoz_text::TextStyle;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::window::{Dialog, DialogProperties};
use kompoz_ui::{Alignment, Modifier};
use std::rc::Rc;
use std::sync::Arc;

/// An alert dialog (Compose's `AlertDialog(onDismissRequest, confirmButton, …)`, Compose for desktop's actual): on
/// a [`BasicAlertDialog`], a surface of `shape` in `container_color` (`tonal_elevation` over it) holding, 24 dp in,
/// `icon` (in `icon_content_color`, centered), `title` (`title_content_color`, the headline style; centered under an
/// icon), `text` (`text_content_color`, the supporting style; it gives way to the buttons when the window is short),
/// and at the end `confirm_button` and `dismiss_button` side by side 8 dp apart — the confirming one last — or,
/// when they don't fit, one above the other, the confirming one on top. `on_dismiss_request` runs on back or a press
/// outside it, as `properties` say.
///
/// Kotlin: `AlertDialog(onDismissRequest: () -> Unit, confirmButton: @Composable () -> Unit, modifier: Modifier = Modifier, dismissButton: @Composable (() -> Unit)? = null, icon: @Composable (() -> Unit)? = null, title: @Composable (() -> Unit)? = null, text: @Composable (() -> Unit)? = null, shape: Shape = AlertDialogDefaults.shape, containerColor: Color = AlertDialogDefaults.containerColor, iconContentColor: Color = AlertDialogDefaults.iconContentColor, titleContentColor: Color = AlertDialogDefaults.titleContentColor, textContentColor: Color = AlertDialogDefaults.textContentColor, tonalElevation: Dp = AlertDialogDefaults.TonalElevation, properties: DialogProperties = DialogProperties())`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn AlertDialog(
    on_dismiss_request: impl Fn() + 'static,
    confirm_button: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] dismiss_button: Option<Rc<dyn Fn()>>,
    #[default(None)] icon: Option<Rc<dyn Fn()>>,
    #[default(None)] title: Option<Rc<dyn Fn()>>,
    #[default(None)] text: Option<Rc<dyn Fn()>>,
    #[default(AlertDialogDefaults.shape())] shape: Arc<dyn Shape>,
    #[default(AlertDialogDefaults.container_color())] container_color: Color,
    #[default(AlertDialogDefaults.icon_content_color())] icon_content_color: Color,
    #[default(AlertDialogDefaults.title_content_color())] title_content_color: Color,
    #[default(AlertDialogDefaults.text_content_color())] text_content_color: Color,
    #[default(AlertDialogDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(DialogProperties::default())] properties: DialogProperties,
) {
    AlertDialogImpl(
        Rc::new(on_dismiss_request),
        Rc::new(confirm_button),
        modifier,
        dismiss_button,
        icon,
        title,
        text,
        shape,
        container_color,
        icon_content_color,
        title_content_color,
        text_content_color,
        tonal_elevation,
        properties,
    );
}

/// A dialog of any content (Compose's experimental `BasicAlertDialog`): `content` in a [`Dialog`] (dismissed by
/// `on_dismiss_request` on back or a press outside, as `properties` say), at least 280 dp and at most 560 dp wide,
/// its least size passed on to `content`, a pane titled "Dialog" (in the platform's language) to accessibility. Made
/// by [`LocalBasicAlertDialogOverride`]'s override ([`DefaultBasicAlertDialogOverride`] unless provided).
///
/// Kotlin: `BasicAlertDialog(onDismissRequest: () -> Unit, modifier: Modifier = Modifier, properties: DialogProperties = DialogProperties(), content: @Composable () -> Unit)`
#[kompozable]
pub fn BasicAlertDialog(
    on_dismiss_request: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(DialogProperties::default())] properties: DialogProperties,
    content: impl Fn() + 'static,
) {
    let override_ = LocalBasicAlertDialogOverride.current();
    override_.BasicAlertDialog(&BasicAlertDialogOverrideScope {
        on_dismiss_request: Rc::new(on_dismiss_request),
        modifier,
        properties,
        content: Rc::new(content),
    });
}

/// The basic alert dialog of an override (Compose's experimental `BasicAlertDialogOverride`): what makes every
/// [`BasicAlertDialog`] below a [`LocalBasicAlertDialogOverride`] providing it, from the scope's arguments.
pub trait BasicAlertDialogOverride {
    /// Composes the dialog of `scope`'s arguments (Compose's `BasicAlertDialogOverrideScope.BasicAlertDialog()`).
    fn BasicAlertDialog(&self, scope: &BasicAlertDialogOverrideScope);
}

impl PartialEq for dyn BasicAlertDialogOverride {
    /// The same override (Kotlin's `==` of objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The default way a basic alert dialog is made (Compose's experimental `DefaultBasicAlertDialogOverride`): the
/// content in a [`Dialog`], 280 to 560 dp wide, a pane titled "Dialog".
#[derive(Clone, Copy, Debug)]
pub struct DefaultBasicAlertDialogOverride;

impl BasicAlertDialogOverride for DefaultBasicAlertDialogOverride {
    #[track_caller]
    fn BasicAlertDialog(&self, scope: &BasicAlertDialogOverrideScope) {
        let BasicAlertDialogOverrideScope { on_dismiss_request, modifier, properties, content } = scope.clone();
        kompozable_block!({
            Dialog(
                on_dismiss_request = move || on_dismiss_request(),
                properties = properties,
                content = move || {
                    let dialog_pane_description = get_string(Strings::Dialog);
                    let content = content.clone();
                    Box(
                        modifier = modifier
                            .clone()
                            .size_in_with(DialogMinWidth, Dp::Unspecified, DialogMaxWidth, Dp::Unspecified)
                            .then(Modifier.semantics(move |c| c.set_pane_title(dialog_pane_description.clone()))),
                        propagate_min_constraints = true,
                        content = move || content(),
                    );
                },
            );
        })
    }
}

/// The arguments of a [`BasicAlertDialog`], given to its override (Compose's experimental
/// `BasicAlertDialogOverrideScope`). Made by [`BasicAlertDialog`].
#[derive(Clone)]
pub struct BasicAlertDialogOverrideScope {
    on_dismiss_request: Rc<dyn Fn()>,
    modifier: Modifier,
    properties: DialogProperties,
    content: Rc<dyn Fn()>,
}

impl BasicAlertDialogOverrideScope {
    /// What dismisses the dialog (Compose's `onDismissRequest`).
    pub fn on_dismiss_request(&self) -> Rc<dyn Fn()> {
        self.on_dismiss_request.clone()
    }

    /// The dialog's modifier (Compose's `modifier`).
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }

    /// The dialog's properties (Compose's `properties`).
    pub fn properties(&self) -> DialogProperties {
        self.properties
    }

    /// The dialog's content (Compose's `content`).
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the basic alert dialogs below (Compose's experimental `LocalBasicAlertDialogOverride`):
/// [`DefaultBasicAlertDialogOverride`] unless provided.
pub static LocalBasicAlertDialogOverride: KompozitionLocal<Rc<dyn BasicAlertDialogOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultBasicAlertDialogOverride));

/// A dialog of any content (Compose's deprecated experimental `AlertDialog(onDismissRequest, modifier, properties,
/// content)`, replaced by [`BasicAlertDialog`]): a [`BasicAlertDialog`].
///
/// Kotlin: `AlertDialog(onDismissRequest: () -> Unit, modifier: Modifier = Modifier, properties: DialogProperties = DialogProperties(), content: @Composable () -> Unit)`
#[deprecated(note = "Use BasicAlertDialog instead")]
#[kompozable]
pub fn AlertDialogWithContent(
    on_dismiss_request: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(DialogProperties::default())] properties: DialogProperties,
    content: impl Fn() + 'static,
) {
    BasicAlertDialog(on_dismiss_request, modifier = modifier, properties = properties, content = content);
}

/// The alert dialogs' defaults (Compose's `AlertDialogDefaults`): their shape, colors, icon size and tonal
/// elevation. What Compose reads in composition is a method of the value (`AlertDialogDefaults.shape()`).
#[derive(Clone, Copy, Debug)]
pub struct AlertDialogDefaults;

impl AlertDialogDefaults {
    /// An alert dialog's tonal elevation: none (Compose's `TonalElevation`).
    pub const TonalElevation: Dp = Dp(0.0);

    /// An alert dialog's icon size: 24 dp (28 with the precision pointer sizing) (Compose's `IconSize`). (Compose
    /// reads the sizing once, as its object is made; it is read as asked here: on the desktop it never changes.)
    pub fn IconSize() -> Dp {
        if precision_pointer() { Dp(28.0) } else { DialogTokens::IconSize }
    }
}

#[named]
impl AlertDialogDefaults {
    /// An alert dialog's shape: the theme's extra large corners, 28 dp (Compose's `shape`).
    #[track_caller]
    pub fn shape(&self) -> Arc<dyn Shape> {
        DialogTokens::ContainerShape.value()
    }

    /// An alert dialog's container color: the high surface container (Compose's `containerColor`).
    #[track_caller]
    pub fn container_color(&self) -> Color {
        DialogTokens::ContainerColor.value()
    }

    /// An alert dialog's icon color: the secondary color (Compose's `iconContentColor`).
    #[track_caller]
    pub fn icon_content_color(&self) -> Color {
        DialogTokens::IconColor.value()
    }

    /// An alert dialog's title color: the surface's content color (Compose's `titleContentColor`).
    #[track_caller]
    pub fn title_content_color(&self) -> Color {
        DialogTokens::HeadlineColor.value()
    }

    /// An alert dialog's text color: the surface variant's content color (Compose's `textContentColor`).
    #[track_caller]
    pub fn text_content_color(&self) -> Color {
        DialogTokens::SupportingTextColor.value()
    }
}

/// Whether components take their precision pointer sizes (Compose's `shouldUsePrecisionPointerComponentSizing.value`).
fn precision_pointer() -> bool {
    should_use_precision_pointer_component_sizing_state().value()
}

/// The alert dialog (Compose's internal `AlertDialogImpl`): its content on a [`BasicAlertDialog`], the buttons in a
/// flow row 8 dp apart each way (less across by what the minimum interactive size adds to a button's 40 dp).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn AlertDialogImpl(
    on_dismiss_request: Rc<dyn Fn()>,
    confirm_button: Rc<dyn Fn()>,
    modifier: Modifier,
    dismiss_button: Option<Rc<dyn Fn()>>,
    icon: Option<Rc<dyn Fn()>>,
    title: Option<Rc<dyn Fn()>>,
    text: Option<Rc<dyn Fn()>>,
    shape: Arc<dyn Shape>,
    container_color: Color,
    icon_content_color: Color,
    title_content_color: Color,
    text_content_color: Color,
    tonal_elevation: Dp,
    properties: DialogProperties,
) {
    BasicAlertDialog(
        move || on_dismiss_request(),
        modifier = modifier,
        properties = properties,
        content = move || {
            let (confirm_button, dismiss_button) = (confirm_button.clone(), dismiss_button.clone());
            let buttons: Rc<dyn Fn()> = Rc::new(move || {
                let button_padding_from_mics = LocalMinimumInteractiveComponentSize.current().take_or_else(|| Dp(0.0))
                    - ButtonDefaults::MinHeight();
                let (confirm_button, dismiss_button) = (confirm_button.clone(), dismiss_button.clone());
                AlertDialogFlowRow(
                    ButtonsMainAxisSpacing,
                    (ButtonsCrossAxisSpacing - button_padding_from_mics).coerce_in(Dp(0.0), ButtonsCrossAxisSpacing),
                    move || {
                        confirm_button();
                        if let Some(dismiss_button) = &dismiss_button {
                            dismiss_button();
                        }
                    },
                );
            });
            AlertDialogContent(
                buttons,
                icon = icon.clone(),
                title = title.clone(),
                text = text.clone(),
                shape = shape.clone(),
                container_color = container_color,
                tonal_elevation = tonal_elevation,
                // Note that a button content color is provided here from the dialog's token, but in
                // most cases, TextButtons should be used for dismiss and confirm buttons. TextButtons
                // will not consume this provided content color value, and will used their own defined
                // or default colors.
                button_content_color = DialogTokens::ActionLabelTextColor.value(),
                icon_content_color = icon_content_color,
                title_content_color = title_content_color,
                text_content_color = text_content_color,
            );
        },
    );
}

/// An alert dialog's content (Compose's internal `AlertDialogContent`): on a surface, a column 24 dp in (20 with the
/// precision pointer sizing) of the icon (16 dp under it), the title (16 dp under it), the text (24 dp under it, or
/// 16; weighted, not filling: it gives way to the buttons) and the buttons at the end.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn AlertDialogContent(
    buttons: Rc<dyn Fn()>,
    #[default(Modifier)] modifier: Modifier,
    icon: Option<Rc<dyn Fn()>>,
    title: Option<Rc<dyn Fn()>>,
    text: Option<Rc<dyn Fn()>>,
    shape: Arc<dyn Shape>,
    container_color: Color,
    tonal_elevation: Dp,
    button_content_color: Color,
    icon_content_color: Color,
    title_content_color: Color,
    text_content_color: Color,
) {
    Surface(
        modifier = modifier,
        shape = shape,
        color = container_color,
        tonal_elevation = tonal_elevation,
        content = move || {
            Column(
                modifier = Modifier.padding_values(DialogPadding()),
                content = move || {
                    let has_icon = icon.is_some();
                    if let Some(icon) = icon.clone() {
                        KompozitionLocalProvider(&[LocalContentColor.provides(icon_content_color)], move || {
                            let icon = icon.clone();
                            Box(
                                modifier = Modifier.padding_values(IconPadding()).align(Alignment::CenterHorizontally),
                                content = move || icon(),
                            );
                        });
                    }
                    if let Some(title) = title.clone() {
                        let text_style: TextStyle = if precision_pointer() {
                            named_args!(
                                MaterialTheme::typography()
                                    .headline_small
                                    .copy(font_size = 20.sp(), line_height = 26.sp())
                            )
                        } else {
                            DialogTokens::HeadlineFont.value()
                        };
                        ProvideContentColorTextStyle(title_content_color, text_style, move || {
                            let title = title.clone();
                            Box(
                                // Align the title to the center when an icon is present.
                                modifier = Modifier.padding_values(TitlePadding()).align(if !has_icon {
                                    Alignment::Start
                                } else {
                                    Alignment::CenterHorizontally
                                }),
                                content = move || title(),
                            );
                        });
                    }
                    if let Some(text) = text.clone() {
                        let text_style = DialogTokens::SupportingTextFont.value();
                        ProvideContentColorTextStyle(text_content_color, text_style, move || {
                            let text = text.clone();
                            Box(
                                modifier = Modifier
                                    .weight(1.0, fill = false)
                                    .padding_values(TextPadding())
                                    .align(Alignment::Start),
                                content = move || text(),
                            );
                        });
                    }
                    let buttons = buttons.clone();
                    Box(
                        modifier = Modifier.align(Alignment::End),
                        content = move || {
                            let text_style = DialogTokens::ActionLabelTextFont.value();
                            let buttons = buttons.clone();
                            ProvideContentColorTextStyle(button_content_color, text_style, move || buttons());
                        },
                    );
                },
            );
        },
    );
}

/// The buttons of an alert dialog (Compose's internal `AlertDialogFlowRow`): a flow row laid out right to left (left
/// to right in a right-to-left layout), so the confirming button, given first, ends the row and tops the column when
/// they wrap; the buttons themselves in the layout direction around.
#[kompozable]
pub(crate) fn AlertDialogFlowRow(main_axis_spacing: Dp, cross_axis_spacing: Dp, content: impl Fn() + 'static) {
    let original_layout_direction = LocalLayoutDirection.current();
    let content: Rc<dyn Fn()> = Rc::new(content);
    // The confirm button comes BEFORE the dismiss button when stacked vertically,
    // but AFTER the dismiss button when stacked horizontally.
    KompozitionLocalProvider(&[LocalLayoutDirection.provides(flip(original_layout_direction))], move || {
        let content = content.clone();
        FlowRow(
            horizontal_arrangement = Arrangement::spaced_by(main_axis_spacing),
            vertical_arrangement = Arrangement::spaced_by(cross_axis_spacing),
            content = move || {
                let content = content.clone();
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(original_layout_direction)], move || {
                    content()
                });
            },
        );
    });
}

/// The other layout direction (Compose's private `LayoutDirection.flip()`).
fn flip(direction: LayoutDirection) -> LayoutDirection {
    match direction {
        LayoutDirection::Ltr => LayoutDirection::Rtl,
        LayoutDirection::Rtl => LayoutDirection::Ltr,
    }
}

/// A basic alert dialog's least width, 280 dp (Compose's internal `DialogMinWidth`).
pub(crate) const DialogMinWidth: Dp = Dp(280.0);
/// A basic alert dialog's greatest width, 560 dp (Compose's internal `DialogMaxWidth`).
pub(crate) const DialogMaxWidth: Dp = Dp(560.0);

const ButtonsMainAxisSpacing: Dp = Dp(8.0);
const ButtonsCrossAxisSpacing: Dp = Dp(8.0);

// (Compose reads the precision pointer sizing once, as its file's values are made; read as asked here)
fn DialogPaddingValue() -> Dp {
    if precision_pointer() { Dp(20.0) } else { Dp(24.0) }
}
fn TextPaddingValue() -> Dp {
    if precision_pointer() { Dp(16.0) } else { Dp(24.0) }
}

// Paddings for each of the dialog's parts.
fn DialogPadding() -> PaddingValues {
    PaddingValues::all(DialogPaddingValue())
}
fn IconPadding() -> PaddingValues {
    PaddingValues::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(16.0))
}
fn TitlePadding() -> PaddingValues {
    PaddingValues::new(Dp(0.0), Dp(0.0), Dp(0.0), Dp(16.0))
}
fn TextPadding() -> PaddingValues {
    PaddingValues::new(Dp(0.0), Dp(0.0), Dp(0.0), TextPaddingValue())
}

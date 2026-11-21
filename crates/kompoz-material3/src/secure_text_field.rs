//! Material's password fields, as Compose's SecureTextField.kt: a filled ([`SecureTextField`]) or outlined
//! ([`OutlinedSecureTextField`]) field on foundation's `BasicSecureTextField`, its text obfuscated, one line, a
//! password's keyboard, copying and cutting refused.

use crate::internal::strings::{Strings, get_string};
use crate::internal::text_field_impl::{TextFieldImplModifiers, minimized_label_half_height};
use crate::text::LocalTextStyle;
use crate::text_field::merged_text_style;
use crate::text_field_defaults::{
    OutlinedTextFieldDefaults, TextFieldColors, TextFieldDefaults, TextFieldLabel, TextFieldLabelPosition,
    TextFieldSlot,
};
use kompoz_foundation::prelude::*;
use kompoz_foundation::text::input::internal::OnTextFieldLayout;
use kompoz_graphics::{Dp, Shape, SolidColor};
use kompoz_runtime::{KompozitionLocalProvider, kompozable, named_args, remember};
use kompoz_text::TextStyle;
use kompoz_ui::Modifier;
use std::rc::Rc;
use std::sync::Arc;

/// A secure field's keyboard unless given (Compose's private `SecureTextFieldKeyboardOptions`): a password's, no
/// autocorrection (foundation's `KeyboardOptions.SecureTextField`, the same options).
const SecureTextFieldKeyboardOptions: KeyboardOptions = KeyboardOptions::SecureTextField;

/// What a secure field shows for each character unless given (Compose's private `DefaultObfuscationCharacter`: a
/// bullet).
const DefaultObfuscationCharacter: char = '\u{2022}';

/// A filled password field editing `state` (Compose's `SecureTextField`, on foundation's `BasicSecureTextField`):
/// `state`'s text shown as `text_obfuscation_character`s (`text_obfuscation_mode`: the character just typed shown a
/// moment by default), one line; decorated as [`TextField`](crate::TextField) is. The other parameters as
/// [`TextField`](crate::TextField)'s and `BasicSecureTextField`'s.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SecureTextField(
    state: TextFieldState,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(LocalTextStyle.current())] text_style: TextStyle,
    #[default(TextFieldLabelPosition::default())] label_position: TextFieldLabelPosition,
    #[default(None)] label: Option<TextFieldLabel>,
    #[default(None)] placeholder: Option<TextFieldSlot>,
    #[default(None)] leading_icon: Option<TextFieldSlot>,
    #[default(None)] trailing_icon: Option<TextFieldSlot>,
    #[default(None)] prefix: Option<TextFieldSlot>,
    #[default(None)] suffix: Option<TextFieldSlot>,
    #[default(None)] supporting_text: Option<TextFieldSlot>,
    #[default(false)] is_error: bool,
    #[default(None)] input_transformation: Option<InputTransformation>,
    #[default(TextObfuscationMode::RevealLastTyped)] text_obfuscation_mode: TextObfuscationMode,
    #[default(DefaultObfuscationCharacter)] text_obfuscation_character: char,
    #[default(SecureTextFieldKeyboardOptions)] keyboard_options: KeyboardOptions,
    #[default(None)] on_keyboard_action: Option<Rc<dyn KeyboardActionHandler>>,
    #[default(None)] on_text_layout: Option<OnTextFieldLayout>,
    #[default(TextFieldDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(TextFieldDefaults.colors())] colors: TextFieldColors,
    #[default(None)] content_padding: Option<PaddingValues>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let merged_text_style = merged_text_style(&text_style, &interaction_source, &colors, enabled, is_error);
    let error_message = get_string(Strings::DefaultErrorMessage);
    KompozitionLocalProvider(&[LocalTextSelectionColors.provides(colors.text_selection_colors)], move || {
        let source = interaction_source.as_interaction_source();
        let container_source = source.clone();
        let container: TextFieldSlot = Rc::new(move || {
            TextFieldDefaults.Container(
                enabled,
                is_error,
                container_source.clone(),
                colors = colors,
                shape = shape.clone(),
            );
        });
        let decorator = TextFieldDefaults.decorator(
            state.clone(),
            enabled,
            TextFieldLineLimits::SingleLine,
            None,
            source,
            label_position = label_position.clone(),
            label = label.clone(),
            placeholder = placeholder.clone(),
            leading_icon = leading_icon.clone(),
            trailing_icon = trailing_icon.clone(),
            prefix = prefix.clone(),
            suffix = suffix.clone(),
            supporting_text = supporting_text.clone(),
            is_error = is_error,
            colors = colors,
            content_padding = content_padding.clone(),
            container = Some(container),
        );
        BasicSecureTextField(
            state.clone(),
            modifier = modifier
                .clone()
                .default_error_semantics(is_error, error_message.clone())
                .default_min_size(min_width = TextFieldDefaults::MinWidth, min_height = TextFieldDefaults::MinHeight),
            enabled = enabled,
            text_style = merged_text_style.clone(),
            cursor_brush = SolidColor(colors.cursor_color_of(is_error)),
            keyboard_options = keyboard_options.clone(),
            on_keyboard_action = on_keyboard_action.clone(),
            on_text_layout = on_text_layout.clone(),
            interaction_source = Some(interaction_source.clone()),
            input_transformation = input_transformation.clone(),
            text_obfuscation_mode = text_obfuscation_mode,
            text_obfuscation_character = text_obfuscation_character,
            decorator = Some(decorator),
        );
    });
}

/// An outlined password field editing `state` (Compose's `OutlinedSecureTextField`): as [`SecureTextField`],
/// decorated as [`OutlinedTextField`](crate::OutlinedTextField) is (half the minimized label's height above it
/// with a label attached, its semantics merged).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn OutlinedSecureTextField(
    state: TextFieldState,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(LocalTextStyle.current())] text_style: TextStyle,
    #[default(TextFieldLabelPosition::default())] label_position: TextFieldLabelPosition,
    #[default(None)] label: Option<TextFieldLabel>,
    #[default(None)] placeholder: Option<TextFieldSlot>,
    #[default(None)] leading_icon: Option<TextFieldSlot>,
    #[default(None)] trailing_icon: Option<TextFieldSlot>,
    #[default(None)] prefix: Option<TextFieldSlot>,
    #[default(None)] suffix: Option<TextFieldSlot>,
    #[default(None)] supporting_text: Option<TextFieldSlot>,
    #[default(false)] is_error: bool,
    #[default(None)] input_transformation: Option<InputTransformation>,
    #[default(TextObfuscationMode::RevealLastTyped)] text_obfuscation_mode: TextObfuscationMode,
    #[default(DefaultObfuscationCharacter)] text_obfuscation_character: char,
    #[default(SecureTextFieldKeyboardOptions)] keyboard_options: KeyboardOptions,
    #[default(None)] on_keyboard_action: Option<Rc<dyn KeyboardActionHandler>>,
    #[default(None)] on_text_layout: Option<OnTextFieldLayout>,
    #[default(OutlinedTextFieldDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(OutlinedTextFieldDefaults.colors())] colors: TextFieldColors,
    #[default(OutlinedTextFieldDefaults.content_padding())] content_padding: PaddingValues,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let merged_text_style = merged_text_style(&text_style, &interaction_source, &colors, enabled, is_error);
    let error_message = get_string(Strings::DefaultErrorMessage);
    let label_room = if label.is_some() && !label_position.is_above() {
        named_args!(Modifier.semantics(merge_descendants = true, properties = |_| {})).padding_sides_with(
            Dp(0.0),
            minimized_label_half_height(),
            Dp(0.0),
            Dp(0.0),
        )
    } else {
        Modifier
    };
    KompozitionLocalProvider(&[LocalTextSelectionColors.provides(colors.text_selection_colors)], move || {
        let source = interaction_source.as_interaction_source();
        let container_source = source.clone();
        let container: TextFieldSlot = Rc::new(move || {
            OutlinedTextFieldDefaults.Container(
                enabled,
                is_error,
                container_source.clone(),
                colors = colors,
                shape = shape.clone(),
            );
        });
        let decorator = OutlinedTextFieldDefaults.decorator(
            state.clone(),
            enabled,
            TextFieldLineLimits::SingleLine,
            None,
            source,
            label_position = label_position.clone(),
            label = label.clone(),
            placeholder = placeholder.clone(),
            leading_icon = leading_icon.clone(),
            trailing_icon = trailing_icon.clone(),
            prefix = prefix.clone(),
            suffix = suffix.clone(),
            supporting_text = supporting_text.clone(),
            is_error = is_error,
            colors = colors,
            content_padding = content_padding.clone(),
            container = Some(container),
        );
        BasicSecureTextField(
            state.clone(),
            modifier = modifier
                .clone()
                .then(label_room.clone())
                .default_error_semantics(is_error, error_message.clone())
                .default_min_size(
                    min_width = OutlinedTextFieldDefaults::MinWidth,
                    min_height = OutlinedTextFieldDefaults::MinHeight,
                ),
            enabled = enabled,
            text_style = merged_text_style.clone(),
            cursor_brush = SolidColor(colors.cursor_color_of(is_error)),
            keyboard_options = keyboard_options.clone(),
            on_keyboard_action = on_keyboard_action.clone(),
            on_text_layout = on_text_layout.clone(),
            interaction_source = Some(interaction_source.clone()),
            input_transformation = input_transformation.clone(),
            text_obfuscation_mode = text_obfuscation_mode,
            text_obfuscation_character = text_obfuscation_character,
            decorator = Some(decorator),
        );
    });
}

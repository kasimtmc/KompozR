//! Text, as Compose's `androidx.compose.foundation.text`: [`BasicText`] (a plain or annotated string laid out,
//! drawn and said to accessibility by its text node, kept laid out while only its color changes), its links (a
//! node over each, opening it or telling its listener, restyled as it is focused, hovered or pressed), its inline
//! content ([`InlineTextContent`] in the rooms `append_inline_content` marks), sizing to fit ([`TextAutoSize`]),
//! text selection ([`selection`]: SelectionContainer, DisableSelection and what they are made of) and the
//! controller a selectable text works with ([`modifiers::SelectionController`]), text drag gestures and handles
//! ([`long_press_text_drag_observer`]), the data of text context menus ([`context_menu`]) and text's pointer icon;
//! text input: the state-based field's model ([`input`]), what both field engines share — the keyboard a field asks
//! for ([`KeyboardOptions`]) and its actions ([`KeyboardActions`]), what keys do ([`KeyCommand`], [`KeyMapping`],
//! [`platform_default_key_mapping`]), dead keys ([`DeadKeyCombiner`]) and typed key events ([`is_typed_event`]).

mod auto_size;
mod autofill_highlight;
mod basic_secure_text_field;
mod basic_text;
mod basic_text_field;
pub mod context_menu;
pub mod context_menu_area;
pub mod core_text_field;
mod dead_key_combiner;
mod inline_content;
pub mod input;
mod key_event_helpers;
mod key_mapping;
mod keyboard_actions;
mod keyboard_options;
mod link_scope;
pub mod long_press_text_drag_observer;
pub mod modifiers;
mod pointer_icon;
pub mod selection;
pub mod text_delegate;
pub mod text_field_cursor;
pub mod text_field_delegate;
pub mod text_field_key_input;
pub mod text_field_pointer;
pub mod text_field_scroll;
pub mod text_field_scroll_state;
pub(crate) mod text_field_size;
pub mod text_layout_result_proxy;
pub mod undo_manager;
pub mod validating_offset_mapping;

pub use auto_size::{
    AutoSizeStepBased, StepBased, TextAutoSize, TextAutoSizeDefaults, TextAutoSizeEq, TextAutoSizeLayoutScope,
};
pub use autofill_highlight::{LocalAutofillHighlightBrush, LocalAutofillHighlightColor};
pub use basic_secure_text_field::{BasicSecureTextField, DefaultObfuscationCharacter};
pub use basic_text::{BasicText, BasicTextValue};
pub use basic_text_field::{
    BasicTextField, BasicTextFieldDefaults, BasicTextFieldString, BasicTextFieldValue, CursorHandle,
    CursorHandleHeight, CursorHandleWidth, TextFieldDecorator,
};
pub use context_menu_area::{
    BasicTextContextMenu, LocalTextContextMenu, TextContextMenu, TextContextMenuAction, TextContextMenuArea,
    TextManager,
};
pub use core_text_field::DecorationBox;
pub use dead_key_combiner::{COMBINING_ACCENT, COMBINING_ACCENT_MASK, DeadKeyCombiner, get_dead_char};
pub use inline_content::{InlineContentMap, InlineTextContent};
pub use key_event_helpers::{
    cancels_text_selection, is_from_hardware_source, is_from_soft_keyboard, is_typed_event, show_character_palette,
};
pub use key_mapping::{
    DesktopPlatform, KeyCommand, KeyMapping, common_key_mapping, create_platform_default_key_mapping,
    default_key_mapping, default_skiko_key_mapping, mac_os_key_mapping, platform_default_key_mapping,
    set_key_mapping_override, windows_key_mapping,
};
pub use keyboard_actions::{KeyboardAction, KeyboardActionRunner, KeyboardActionScope, KeyboardActions};
pub use keyboard_options::KeyboardOptions;
pub use long_press_text_drag_observer::{Handle, TextDragObserver, detect_down_and_drag_gestures_with_observer};
pub use modifiers::SelectionController;
pub use modifiers::text_annotated_string::OnTextLayout;
pub use pointer_icon::TextPointerIcon;
pub use selection::{
    LocalSelectionRegistrar, LocalTextSelectionColors, SelectionRegistrar, SelectionRegistrarRef, TextSelectionColors,
};
pub use text_field_scroll_state::{
    TextFieldScrollState, remember_text_field_horizontal_scroll_state, remember_text_field_scroll_state,
    remember_text_field_vertical_scroll_state,
};

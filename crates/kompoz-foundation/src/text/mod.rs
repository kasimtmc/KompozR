//! Text, as Compose's `androidx.compose.foundation.text`: [`BasicText`] (a plain or annotated string laid out,
//! drawn and said to accessibility by its text node, kept laid out while only its color changes), its links (a
//! node over each, opening it or telling its listener, restyled as it is focused, hovered or pressed), its inline
//! content ([`InlineTextContent`] in the rooms `append_inline_content` marks), sizing to fit ([`TextAutoSize`]),
//! text selection ([`selection`]: SelectionContainer, DisableSelection and what they are made of) and the
//! controller a selectable text works with ([`modifiers::SelectionController`]), text drag gestures and handles
//! ([`long_press_text_drag_observer`]), the data of text context menus ([`context_menu`]) and text's pointer icon.

mod auto_size;
mod basic_text;
pub mod context_menu;
mod inline_content;
mod link_scope;
pub mod long_press_text_drag_observer;
pub mod modifiers;
mod pointer_icon;
pub mod selection;

pub use auto_size::{
    AutoSizeStepBased, StepBased, TextAutoSize, TextAutoSizeDefaults, TextAutoSizeEq, TextAutoSizeLayoutScope,
};
pub use basic_text::{BasicText, BasicTextValue};
pub use inline_content::{InlineContentMap, InlineTextContent};
pub use long_press_text_drag_observer::{Handle, TextDragObserver, detect_down_and_drag_gestures_with_observer};
pub use modifiers::SelectionController;
pub use modifiers::text_annotated_string::OnTextLayout;
pub use pointer_icon::TextPointerIcon;
pub use selection::{
    LocalSelectionRegistrar, LocalTextSelectionColors, SelectionRegistrar, SelectionRegistrarRef, TextSelectionColors,
};

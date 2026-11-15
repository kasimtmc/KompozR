//! Text, as Compose's `androidx.compose.foundation.text`: [`BasicText`] (a plain or annotated string laid out,
//! drawn and said to accessibility by its text node, kept laid out while only its color changes), its links (a
//! node over each, opening it or telling its listener, restyled as it is focused, hovered or pressed), its inline
//! content ([`InlineTextContent`] in the rooms `append_inline_content` marks), sizing to fit ([`TextAutoSize`]),
//! and the bridge a selection container's selection goes through ([`modifiers::selection_hook`]).

mod auto_size;
mod basic_text;
mod inline_content;
mod link_scope;
pub mod modifiers;

pub use auto_size::{
    AutoSizeStepBased, StepBased, TextAutoSize, TextAutoSizeDefaults, TextAutoSizeEq, TextAutoSizeLayoutScope,
};
pub use basic_text::{BasicText, BasicTextValue};
pub use inline_content::{InlineContentMap, InlineTextContent};
pub use modifiers::selection_hook::{
    LocalSelectionRegistrar, LocalTextSelectionColors, SelectionController, SelectionRegistrar, SelectionRegistrarRef,
    TextSelectionColors,
};
pub use modifiers::text_annotated_string::OnTextLayout;

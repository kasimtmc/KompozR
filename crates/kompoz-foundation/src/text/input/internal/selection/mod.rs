//! A state-based field's selection (Compose's `text.input.internal.selection`): the selection state and its
//! gestures, the handles' states, the magnifier.

mod text_field_handle_state;
mod text_field_magnifier;
mod text_field_selection_state;

pub use text_field_handle_state::TextFieldHandleState;
pub(crate) use text_field_magnifier::TextFieldMagnifierElement;
pub use text_field_magnifier::{TextFieldMagnifierNode, calculate_selection_magnifier_center_android};
pub use text_field_selection_state::{
    DefaultCursorThickness, InputType, ReceiveContentConfigurationProvider, TextFieldSelectionState,
    TextToolbarHandler, TextToolbarState,
};

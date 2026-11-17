//! The state-based text field's model (Compose's `androidx.compose.foundation.text.input`): the field's state
//! ([`TextFieldState`], [`remember_text_field_state`]), the buffer edits go through ([`TextFieldBuffer`] and its
//! [`ChangeList`]), the undo history ([`UndoState`]), what the field does to the user's edits
//! ([`InputTransformation`]) and to what it shows ([`OutputTransformation`]), its line limits, a secure field's
//! obfuscation, its keyboard action handler, its tracked styles ([`TrackedRange`], [`TextFieldTextStyles`]); and
//! the internals the BasicTextField composable is made of ([`internal`]).

mod change_tracker;
mod input_transformation;
pub mod internal;
mod keyboard_action_handler;
mod output_transformation;
mod text_field_buffer;
mod text_field_char_sequence;
mod text_field_line_limits;
mod text_field_state;
mod text_field_text_styles;
mod text_obfuscation_mode;
mod text_undo_manager;
mod tracked_range;
mod undo_state;

pub use change_tracker::{ChangeList, ChangeTracker};
pub use input_transformation::{InputTransformation, TransformInput};
pub use keyboard_action_handler::KeyboardActionHandler;
pub use output_transformation::{OutputTransformation, output_transformation};
pub use text_field_buffer::{TextFieldBuffer, TextFieldStyle, adjust_text_range, find_common_prefix_and_suffix};
pub use text_field_char_sequence::{PlacedAnnotation, TextFieldCharSequence, TextHighlightType};
pub use text_field_line_limits::TextFieldLineLimits;
pub use text_field_state::{NotifyImeListener, TextFieldState, remember_text_field_state};
pub use text_field_text_styles::{TextFieldTextStyles, TextFieldTextStylesImpl};
pub use text_obfuscation_mode::TextObfuscationMode;
pub use text_undo_manager::TextUndoManager;
pub use tracked_range::{ExpandPolicy, TrackedRange};
pub use undo_state::UndoState;

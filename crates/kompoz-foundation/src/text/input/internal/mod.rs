//! The state-based field's internals (Compose's `androidx.compose.foundation.text.input.internal`): what the
//! BasicTextField composable is made of beyond the public model — the state as it is shown and edited
//! ([`TransformedTextFieldState`]), offset mapping, code point transformations, the text layout state, the key
//! handler and its selection moves, the input method session, undo; the field's nodes (decorator, core, text
//! layout), its layout cache, its cursor's blink and its selection ([`selection`]).

pub mod codepoint_transformation;
pub mod core_text_field_semantics_modifier;
pub mod cursor_animation_state;
pub mod legacy_platform_text_input_service_adapter;
pub mod offset_mapping_calculator;
pub mod selection;
pub mod text_field_core_modifier;
pub mod text_field_decorator_modifier;
pub mod text_field_key_event_handler;
pub mod text_field_layout_state_cache;
pub mod text_field_text_layout_modifier;
pub mod text_input_session;
pub mod text_layout_state;
pub mod text_prepared_selection;
pub mod text_style_buffer;
pub mod transformed_text_field_state;
pub mod undo;

pub use codepoint_transformation::{
    CodepointTransformation, MaskCodepointTransformation, SingleLineCodepointTransformation, mask,
    same_codepoint_transformation, to_visual_text,
};
pub use cursor_animation_state::CursorAnimationState;
pub use offset_mapping_calculator::OffsetMappingCalculator;
pub use text_field_core_modifier::{TextFieldCoreModifier, TextFieldCoreModifierNode};
pub use text_field_decorator_modifier::{TextFieldDecoratorModifier, TextFieldDecoratorModifierNode};
pub use text_field_key_event_handler::{ClipboardKeyCommandsHandler, TextFieldKeyEventHandler};
pub use text_field_layout_state_cache::TextFieldLayoutStateCache;
pub use text_field_text_layout_modifier::{TextFieldTextLayoutModifier, TextFieldTextLayoutModifierNode};
pub use text_input_session::{apply_ime_edit_command, ime_delete, ime_replace, platform_specific_text_input_session};
pub use text_layout_state::{OnTextFieldLayout, TextLayoutState};
pub use text_prepared_selection::{
    CursorAndWedgeAffinity, SelectionMovementDeletionContext, TextFieldPreparedSelectionState,
    calculate_next_cursor_position_and_wedge_affinity,
};
pub use text_style_buffer::TextStyleBuffer;
pub use transformed_text_field_state::{
    IndexTransformationType, SelectionWedgeAffinity, TransformedTextFieldState, WedgeAffinity,
};
pub use undo::{
    SNAPSHOTS_INTERVAL_MILLIS, TextDeleteType, TextEditType, TextFieldEditUndoBehavior, TextUndoOperation, UndoManager,
    time_now_millis,
};

//! What a field's handle shows (Compose's TextFieldHandleState).

use kompoz_graphics::Offset;
use kompoz_text::ResolvedTextDirection;

/// Whether a field's cursor or selection handle shows, where, how tall its line is, which way its text goes and
/// whether the selection's ends crossed (Compose's TextFieldHandleState).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextFieldHandleState {
    pub visible: bool,
    pub position: Offset,
    pub line_height: f32,
    pub direction: ResolvedTextDirection,
    pub handles_crossed: bool,
}

impl TextFieldHandleState {
    /// Not shown.
    pub const Hidden: TextFieldHandleState = TextFieldHandleState {
        visible: false,
        position: Offset::Unspecified,
        line_height: 0.0,
        direction: ResolvedTextDirection::Ltr,
        handles_crossed: false,
    };
}

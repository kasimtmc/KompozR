//! Selection controls, as Compose's `androidx.compose.foundation.selection`: `selectable` (one of a group: a radio
//! button), `toggleable` and `tri_state_toggleable` (on, off, indeterminate: a checkbox, a switch) — clickables
//! that tell accessibility their state — and `selectable_group`.

mod selectable;
mod toggleable;

pub use selectable::SelectableModifiers;
pub use toggleable::ToggleableModifiers;

use kompoz_runtime::named;
use kompoz_ui::Modifier;

/// The selection group modifier of foundation.
#[named(trait SelectionModifiers)]
impl Modifier {
    /// Tells accessibility the selectables in the layout are one group (radio buttons: Compose's
    /// `Modifier.selectableGroup`).
    fn selectable_group(self) -> Modifier {
        self.semantics(|c| c.selectable_group())
    }
}

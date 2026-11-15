//! Selectable groups (Compose's `Modifier.selectableGroup` of selection/SelectableGroup.kt).

use kompoz_runtime::named;
use kompoz_ui::Modifier;

/// The selection modifiers of foundation.
#[named(trait SelectionModifiers)]
impl Modifier {
    /// Tells accessibility the selectables in the layout are one group (radio buttons: Compose's
    /// `Modifier.selectableGroup`).
    fn selectable_group(self) -> Modifier {
        self.semantics(|c| c.selectable_group())
    }
}

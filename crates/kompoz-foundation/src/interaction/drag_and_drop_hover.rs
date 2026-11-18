//! A text field's drag and drop hover interactions (Compose's internal DragAndDropHoverInteraction.kt): a drag
//! with something the field takes came over it, then left.

use super::{Interaction, InteractionSource, collect_tracking_as_state};
use kompoz_runtime::State;

/// The drag and drop hover interactions (Compose's DragAndDropHoverInteraction).
#[allow(non_snake_case)]
pub mod DragAndDropHoverInteraction {
    use super::super::{Interaction, InteractionId};

    /// A drag came over the field.
    #[derive(Clone, Copy, Debug)]
    pub struct Enter {
        id: InteractionId,
    }

    impl Enter {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Enter {
            Enter { id: InteractionId::next() }
        }
    }

    impl Interaction for Enter {}
    identity_eq!(Enter);

    /// The drag that entered left it (or ended).
    #[derive(Clone, Copy, Debug)]
    pub struct Exit {
        id: InteractionId,
        pub enter: Enter,
    }

    impl Exit {
        pub fn new(enter: Enter) -> Exit {
            Exit { id: InteractionId::next(), enter }
        }
    }

    impl Interaction for Exit {}
    identity_eq!(Exit);
}

/// A drag and drop hover interaction's part in what is hovered.
fn track_drag_and_drop_hovers(hovers: &mut Vec<DragAndDropHoverInteraction::Enter>, interaction: &dyn Interaction) {
    if let Some(enter) = interaction.downcast_ref::<DragAndDropHoverInteraction::Enter>() {
        hovers.push(*enter);
    } else if let Some(exit) = interaction.downcast_ref::<DragAndDropHoverInteraction::Exit>() {
        hovers.retain(|e| *e != exit.enter);
    }
}

impl InteractionSource {
    /// Whether a drag hovers it now, as state, while the caller is composed (Compose's internal
    /// `collectIsDragAndDropHoveredAsState`).
    #[track_caller]
    pub(crate) fn collect_is_drag_and_drop_hovered_as_state(&self) -> State<bool> {
        collect_tracking_as_state(self, track_drag_and_drop_hovers)
    }
}

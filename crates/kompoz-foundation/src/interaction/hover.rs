//! Hover interactions (Compose's HoverInteraction.kt).

use super::{Interaction, InteractionSource, collect_tracking_as_state};
use kompoz_runtime::State;

/// The hover interactions (Compose's HoverInteraction): a mouse came over a component
/// ([`Enter`](HoverInteraction::Enter)), then left it ([`Exit`](HoverInteraction::Exit)).
#[allow(non_snake_case)]
pub mod HoverInteraction {
    use super::super::{Interaction, InteractionId};

    /// A pointer came over the component.
    #[derive(Clone, Copy, Debug)]
    pub struct Enter {
        id: InteractionId,
    }

    impl Enter {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Enter {
            Enter { id: InteractionId::next() }
        }
        pub fn id(&self) -> InteractionId {
            self.id
        }
    }

    impl Interaction for Enter {}
    identity_eq!(Enter);

    /// The pointer that entered left it.
    #[derive(Clone, Copy, Debug)]
    pub struct Exit {
        id: InteractionId,
        pub enter: Enter,
    }

    impl Exit {
        pub fn new(enter: Enter) -> Exit {
            Exit { id: InteractionId::next(), enter }
        }
        pub fn id(&self) -> InteractionId {
            self.id
        }
    }

    impl Interaction for Exit {}
    identity_eq!(Exit);
}

impl InteractionSource {
    /// Whether it is hovered now, as state, while the caller is composed (Compose's `collectIsHoveredAsState`).
    #[track_caller]
    pub fn collect_is_hovered_as_state(&self) -> State<bool> {
        collect_tracking_as_state(self, track_hovers)
    }
}

/// A hover interaction's part in what is hovered.
pub(crate) fn track_hovers(hovers: &mut Vec<HoverInteraction::Enter>, interaction: &dyn Interaction) {
    if let Some(enter) = interaction.downcast_ref::<HoverInteraction::Enter>() {
        hovers.push(*enter);
    } else if let Some(exit) = interaction.downcast_ref::<HoverInteraction::Exit>() {
        hovers.retain(|e| *e != exit.enter);
    }
}

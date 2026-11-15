//! Focus interactions (Compose's FocusInteraction.kt).

use super::{Interaction, InteractionSource, collect_tracking_as_state};
use kompoz_runtime::State;

/// The focus interactions (Compose's FocusInteraction): a component took the focus
/// ([`Focus`](FocusInteraction::Focus)), then lost it ([`Unfocus`](FocusInteraction::Unfocus)).
#[allow(non_snake_case)]
pub mod FocusInteraction {
    use super::super::{Interaction, InteractionId};

    /// The component took the focus.
    #[derive(Clone, Copy, Debug)]
    pub struct Focus {
        id: InteractionId,
    }

    impl Focus {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Focus {
            Focus { id: InteractionId::next() }
        }
        pub fn id(&self) -> InteractionId {
            self.id
        }
    }

    impl Interaction for Focus {}
    identity_eq!(Focus);

    /// It lost the focus it took.
    #[derive(Clone, Copy, Debug)]
    pub struct Unfocus {
        id: InteractionId,
        pub focus: Focus,
    }

    impl Unfocus {
        pub fn new(focus: Focus) -> Unfocus {
            Unfocus { id: InteractionId::next(), focus }
        }
        pub fn id(&self) -> InteractionId {
            self.id
        }
    }

    impl Interaction for Unfocus {}
    identity_eq!(Unfocus);
}

impl InteractionSource {
    /// Whether it is focused now, as state, while the caller is composed (Compose's `collectIsFocusedAsState`).
    #[track_caller]
    pub fn collect_is_focused_as_state(&self) -> State<bool> {
        collect_tracking_as_state(self, track_focuses)
    }
}

/// A focus interaction's part in what is focused.
pub(crate) fn track_focuses(focuses: &mut Vec<FocusInteraction::Focus>, interaction: &dyn Interaction) {
    if let Some(focus) = interaction.downcast_ref::<FocusInteraction::Focus>() {
        focuses.push(*focus);
    } else if let Some(unfocus) = interaction.downcast_ref::<FocusInteraction::Unfocus>() {
        focuses.retain(|f| *f != unfocus.focus);
    }
}

//! Press interactions (Compose's PressInteraction.kt).

use super::{Interaction, InteractionSource, collect_tracking_as_state};
use kompoz_runtime::State;

/// The press interactions (Compose's PressInteraction): a [`Press`](PressInteraction::Press), ended by its
/// [`Release`](PressInteraction::Release) or [`Cancel`](PressInteraction::Cancel).
#[allow(non_snake_case)]
pub mod PressInteraction {
    use super::super::{Interaction, InteractionId};
    use kompoz_graphics::Offset;

    /// A component was pressed at `press_position` (in it).
    #[derive(Clone, Copy, Debug)]
    pub struct Press {
        id: InteractionId,
        pub press_position: Offset,
    }

    impl Press {
        pub fn new(press_position: Offset) -> Press {
            Press { id: InteractionId::next(), press_position }
        }
        pub fn id(&self) -> InteractionId {
            self.id
        }
    }

    impl Interaction for Press {}
    identity_eq!(Press);

    /// The press was let go: what it pressed was clicked.
    #[derive(Clone, Copy, Debug)]
    pub struct Release {
        id: InteractionId,
        pub press: Press,
    }

    impl Release {
        pub fn new(press: Press) -> Release {
            Release { id: InteractionId::next(), press }
        }
        pub fn id(&self) -> InteractionId {
            self.id
        }
    }

    impl Interaction for Release {}
    identity_eq!(Release);

    /// The press ended without a click (the pointer moved away, the gesture was taken).
    #[derive(Clone, Copy, Debug)]
    pub struct Cancel {
        id: InteractionId,
        pub press: Press,
    }

    impl Cancel {
        pub fn new(press: Press) -> Cancel {
            Cancel { id: InteractionId::next(), press }
        }
        pub fn id(&self) -> InteractionId {
            self.id
        }
    }

    impl Interaction for Cancel {}
    identity_eq!(Cancel);
}

impl InteractionSource {
    /// Whether it is pressed now, as state, while the caller is composed (Compose's `collectIsPressedAsState`):
    /// true from a [`Press`](PressInteraction::Press) until each press emitted has ended.
    #[track_caller]
    pub fn collect_is_pressed_as_state(&self) -> State<bool> {
        collect_tracking_as_state(self, track_presses)
    }
}

/// A press interaction's part in what is pressed.
pub(crate) fn track_presses(presses: &mut Vec<PressInteraction::Press>, interaction: &dyn Interaction) {
    if let Some(press) = interaction.downcast_ref::<PressInteraction::Press>() {
        presses.push(*press);
    } else if let Some(release) = interaction.downcast_ref::<PressInteraction::Release>() {
        presses.retain(|p| *p != release.press);
    } else if let Some(cancel) = interaction.downcast_ref::<PressInteraction::Cancel>() {
        presses.retain(|p| *p != cancel.press);
    }
}

//! Drag interactions (Compose's DragInteraction.kt).

use super::{Interaction, InteractionSource, collect_tracking_as_state};
use kompoz_runtime::State;

/// The drag interactions (Compose's DragInteraction): a drag [`Start`](DragInteraction::Start)ed, ended by its
/// [`Stop`](DragInteraction::Stop) or [`Cancel`](DragInteraction::Cancel).
#[allow(non_snake_case)]
pub mod DragInteraction {
    use super::super::{Interaction, InteractionId};

    /// A drag started.
    #[derive(Clone, Copy, Debug)]
    pub struct Start {
        id: InteractionId,
    }

    impl Start {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Start {
            Start { id: InteractionId::next() }
        }
        pub fn id(&self) -> InteractionId {
            self.id
        }
    }

    impl Interaction for Start {}
    identity_eq!(Start);

    /// It stopped: the pointer was let go.
    #[derive(Clone, Copy, Debug)]
    pub struct Stop {
        id: InteractionId,
        pub start: Start,
    }

    impl Stop {
        pub fn new(start: Start) -> Stop {
            Stop { id: InteractionId::next(), start }
        }
        pub fn id(&self) -> InteractionId {
            self.id
        }
    }

    impl Interaction for Stop {}
    identity_eq!(Stop);

    /// It was cancelled (taken by another gesture, the component went).
    #[derive(Clone, Copy, Debug)]
    pub struct Cancel {
        id: InteractionId,
        pub start: Start,
    }

    impl Cancel {
        pub fn new(start: Start) -> Cancel {
            Cancel { id: InteractionId::next(), start }
        }
        pub fn id(&self) -> InteractionId {
            self.id
        }
    }

    impl Interaction for Cancel {}
    identity_eq!(Cancel);
}

impl InteractionSource {
    /// Whether it is dragged now, as state, while the caller is composed (Compose's `collectIsDraggedAsState`).
    #[track_caller]
    pub fn collect_is_dragged_as_state(&self) -> State<bool> {
        collect_tracking_as_state(self, track_drags)
    }
}

/// A drag interaction's part in what is dragged.
pub(crate) fn track_drags(drags: &mut Vec<DragInteraction::Start>, interaction: &dyn Interaction) {
    if let Some(start) = interaction.downcast_ref::<DragInteraction::Start>() {
        drags.push(*start);
    } else if let Some(stop) = interaction.downcast_ref::<DragInteraction::Stop>() {
        drags.retain(|s| *s != stop.start);
    } else if let Some(cancel) = interaction.downcast_ref::<DragInteraction::Cancel>() {
        drags.retain(|s| *s != cancel.start);
    }
}

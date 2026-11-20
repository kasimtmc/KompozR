//! An interaction source whose presses are moved, as Compose's internal MappedInteractionSource.kt: the
//! interactions of another, each press (and the release or cancel that ends it) at its position less a delta
//! worked out as it goes by — a navigation item's indicator ripple, its presses in the indicator's own place.
//!
//! Compose's is an `InteractionSource` (an interface: its `interactions` the mapped flow). KompozR's
//! [`InteractionSource`] is foundation's concrete source of a shared flow, which a flow of one's own can't stand in
//! for: this gives the mapped flow ([`interactions`](MappedInteractionSource::interactions)), which an indication
//! taking it reads.

use kompoz_coroutines::Flow;
use kompoz_foundation::interaction::{Interaction, InteractionSource, PressInteraction};
use kompoz_graphics::Offset;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// The interactions of `underlying_interaction_source`, its presses moved by less `calculate_delta()` (Compose's
/// internal `MappedInteractionSource`).
#[derive(Clone)]
pub(crate) struct MappedInteractionSource {
    interactions: Flow<Arc<dyn Interaction>>,
}

impl MappedInteractionSource {
    pub(crate) fn new(
        underlying_interaction_source: &InteractionSource,
        calculate_delta: impl Fn() -> Offset + 'static,
    ) -> MappedInteractionSource {
        // The presses mapped, by the press they were mapped of (Compose's `mappedPresses`).
        let mapped_presses: Rc<RefCell<HashMap<PressInteraction::Press, PressInteraction::Press>>> =
            Rc::new(RefCell::new(HashMap::new()));
        let map_press = move |press: &PressInteraction::Press| {
            PressInteraction::Press::new(press.press_position - calculate_delta())
        };
        let interactions = underlying_interaction_source.interactions().map(move |interaction| {
            if let Some(press) = interaction.downcast_ref::<PressInteraction::Press>() {
                let mapped_press = map_press(press);
                mapped_presses.borrow_mut().insert(*press, mapped_press);
                Arc::new(mapped_press) as Arc<dyn Interaction>
            } else if let Some(cancel) = interaction.downcast_ref::<PressInteraction::Cancel>() {
                let mapped_press = mapped_presses.borrow_mut().remove(&cancel.press);
                match mapped_press {
                    None => interaction,
                    Some(mapped_press) => Arc::new(PressInteraction::Cancel::new(mapped_press)),
                }
            } else if let Some(release) = interaction.downcast_ref::<PressInteraction::Release>() {
                let mapped_press = mapped_presses.borrow_mut().remove(&release.press);
                match mapped_press {
                    None => interaction,
                    Some(mapped_press) => Arc::new(PressInteraction::Release::new(mapped_press)),
                }
            } else {
                interaction
            }
        });
        MappedInteractionSource { interactions }
    }

    /// The interactions, the presses mapped (Compose's `interactions`).
    pub(crate) fn interactions(&self) -> Flow<Arc<dyn Interaction>> {
        self.interactions.clone()
    }
}

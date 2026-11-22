//! The interactions going on, as Compose's components collect them (the `when (interaction)` of a collected
//! interaction in ButtonElevation, FloatingActionButtonElevation, CardElevation, ChipElevation, …): an interaction
//! starting is added to a list, its end takes the first of it away, and the last one left is what the component
//! shows.

use kompoz_foundation::interaction::{
    DragInteraction, FocusInteraction, HoverInteraction, Interaction, PressInteraction,
};
use kompoz_runtime::SnapshotStateList;
use std::sync::Arc;

/// An interaction's part in what goes on, for a component following hovers, focus and presses (Compose's buttons'
/// and FABs' collectors): a hover, a focus or a press added, its end taking it away (the first of it). Anything else
/// changes nothing.
pub(crate) fn track_interaction(
    interactions: &SnapshotStateList<Arc<dyn Interaction>>,
    interaction: Arc<dyn Interaction>,
) {
    if interaction.is::<HoverInteraction::Enter>()
        || interaction.is::<FocusInteraction::Focus>()
        || interaction.is::<PressInteraction::Press>()
    {
        interactions.add(interaction);
    } else {
        interactions.update(|list| remove_ended(list, &*interaction));
    }
}

/// [`track_interaction`] for a component following drags too (Compose's cards' and chips' collectors): a hover, a
/// focus, a press or a drag added, its end taking it away.
pub(crate) fn track_interaction_with_drags(
    interactions: &SnapshotStateList<Arc<dyn Interaction>>,
    interaction: Arc<dyn Interaction>,
) {
    if interaction.is::<DragInteraction::Start>() {
        interactions.add(interaction);
    } else {
        track_interaction(interactions, interaction);
    }
}

/// Takes the first interaction `end` ends out of `list` (Kotlin's `interactions.remove(interaction.enter)` and the
/// others); an interaction that ends nothing, or whose start isn't there, takes nothing away.
pub(crate) fn remove_ended(list: &mut Vec<Arc<dyn Interaction>>, end: &dyn Interaction) {
    let started = |i: &Arc<dyn Interaction>| -> bool {
        if let Some(exit) = end.downcast_ref::<HoverInteraction::Exit>() {
            i.downcast_ref::<HoverInteraction::Enter>() == Some(&exit.enter)
        } else if let Some(unfocus) = end.downcast_ref::<FocusInteraction::Unfocus>() {
            i.downcast_ref::<FocusInteraction::Focus>() == Some(&unfocus.focus)
        } else if let Some(release) = end.downcast_ref::<PressInteraction::Release>() {
            i.downcast_ref::<PressInteraction::Press>() == Some(&release.press)
        } else if let Some(cancel) = end.downcast_ref::<PressInteraction::Cancel>() {
            i.downcast_ref::<PressInteraction::Press>() == Some(&cancel.press)
        } else if let Some(stop) = end.downcast_ref::<DragInteraction::Stop>() {
            i.downcast_ref::<DragInteraction::Start>() == Some(&stop.start)
        } else if let Some(cancel) = end.downcast_ref::<DragInteraction::Cancel>() {
            i.downcast_ref::<DragInteraction::Start>() == Some(&cancel.start)
        } else {
            false
        }
    };
    if let Some(index) = list.iter().position(started) {
        list.remove(index);
    }
}

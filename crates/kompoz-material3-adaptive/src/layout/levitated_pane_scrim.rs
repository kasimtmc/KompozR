//! The default scrim of a levitated pane, as Compose's adaptive-layout LevitatedPaneScrim.kt (1.2.0):
//! [`LevitatedPaneScrim`] and [`LevitatedPaneScrimDefaults`].

use kompoz_foundation::prelude::*;
use kompoz_graphics::Color;
use kompoz_runtime::kompozable;
use kompoz_ui::Modifier;
use std::rc::Rc;

/// The scrim of a levitated pane (Compose's `LevitatedPaneScrim`): the whole room in `color`, taking the clicks
/// (`on_click`; none of its own by default) so that the panes under it don't; out of accessibility once it has a
/// click of its own (Compose's: the default no-op click keeps its semantics).
///
/// Kotlin: `LevitatedPaneScrim(modifier: Modifier = Modifier, onClick: (() -> Unit) = LevitatedPaneScrimDefaults.onClickAction, color: Color = LevitatedPaneScrimDefaults.Color)`
#[kompozable]
pub fn LevitatedPaneScrim(
    #[default(Modifier)] modifier: Modifier,
    #[default(LevitatedPaneScrimDefaults::on_click_action())] on_click: Rc<dyn Fn()>,
    #[default(LevitatedPaneScrimDefaults::Color)] color: Color,
) {
    let is_default = Rc::ptr_eq(&on_click, &LevitatedPaneScrimDefaults::on_click_action());
    let semantics = if is_default {
        Modifier
    } else {
        // Disable a11y for the scrim to respect the no-op default onClick action.
        Modifier.clear_and_set_semantics(|_| {})
    };
    Box(modifier = modifier.fill_max_size().background(color).then(semantics).clickable_indication_with(
        None,
        None,
        true,
        None,
        None,
        move || on_click(),
    ));
}

/// The levitated pane scrim's defaults (Compose's `LevitatedPaneScrimDefaults`).
pub struct LevitatedPaneScrimDefaults;

impl LevitatedPaneScrimDefaults {
    /// Black at 32% (Compose's `Color`).
    pub const Color: Color = Color::new(0.0, 0.0, 0.0, 0.32);

    /// The default click: nothing, one shared function compared by identity (Compose's internal `onClickAction`).
    pub(crate) fn on_click_action() -> Rc<dyn Fn()> {
        thread_local! {
            static ACTION: Rc<dyn Fn()> = Rc::new(|| {});
        }
        ACTION.with(|a| a.clone())
    }
}

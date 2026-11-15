//! `Modifier.toggleable` and `Modifier.tri_state_toggleable`, as Compose's selection/Toggleable.kt.

use crate::clickable::{Common, ToggleableElement, TriStateToggleableElement, clickable_with_indication_if_needed};
use crate::indication::Indication;
use crate::interaction::MutableInteractionSource;
use kompoz_runtime::named;
use kompoz_ui::Modifier;
use kompoz_ui::semantics::{Role, ToggleableState};
use std::rc::Rc;

thread_local! {
    /// The click a toggleable's element carries: its node replaces it with its own, which reads the value and the
    /// callback when it is called (one shared, so it doesn't make the elements unequal).
    static NO_CLICK: Rc<dyn Fn()> = Rc::new(|| {});
}

fn no_click() -> Rc<dyn Fn()> {
    NO_CLICK.with(Rc::clone)
}

/// The toggleable modifiers (Compose's `Modifier.toggleable` and `Modifier.triStateToggleable` overloads).
#[named(trait ToggleableModifiers)]
impl Modifier {
    /// Makes the layout toggle between on and off (Compose's `Modifier.toggleable`): a click calls
    /// `on_value_change` with the other value; accessibility is told its state. The rest is
    /// [`clickable`](crate::clickable::ClickableModifiers::clickable)'s.
    fn toggleable(
        self,
        value: bool,
        #[default(true)] enabled: bool,
        #[default(None)] role: Option<Role>,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        on_value_change: impl Fn(bool) + 'static,
    ) -> Modifier {
        self.toggleable_with(value, enabled, role, interaction_source, on_value_change)
    }

    /// [`toggleable`](ToggleableModifiers::toggleable) with every parameter, for plain code.
    fn toggleable_with(
        self,
        value: bool,
        enabled: bool,
        role: Option<Role>,
        interaction_source: Option<MutableInteractionSource>,
        on_value_change: impl Fn(bool) + 'static,
    ) -> Modifier {
        let common = Common::new(interaction_source, None, true, enabled, None, role, no_click());
        self.then_element(ToggleableElement { value, on_value_change: Rc::new(on_value_change), common })
    }

    /// The same showing `indication` (Compose's `toggleable(value, interactionSource, indication, …)`).
    fn toggleable_indication(
        self,
        value: bool,
        interaction_source: Option<MutableInteractionSource>,
        indication: Option<Rc<dyn Indication>>,
        #[default(true)] enabled: bool,
        #[default(None)] role: Option<Role>,
        on_value_change: impl Fn(bool) + 'static,
    ) -> Modifier {
        self.toggleable_indication_with(value, interaction_source, indication, enabled, role, on_value_change)
    }

    /// [`toggleable_indication`](ToggleableModifiers::toggleable_indication) with every parameter, for plain code.
    fn toggleable_indication_with(
        self,
        value: bool,
        interaction_source: Option<MutableInteractionSource>,
        indication: Option<Rc<dyn Indication>>,
        enabled: bool,
        role: Option<Role>,
        on_value_change: impl Fn(bool) + 'static,
    ) -> Modifier {
        let on_value_change: Rc<dyn Fn(bool)> = Rc::new(on_value_change);
        clickable_with_indication_if_needed(self, interaction_source, indication, move |source, factory| {
            let common = Common::new(source, factory, false, enabled, None, role, no_click());
            Modifier::element(ToggleableElement { value, on_value_change: on_value_change.clone(), common })
        })
    }

    /// Makes the layout a three-state toggle (Compose's `Modifier.triStateToggleable`): a click calls `on_click`
    /// (which decides the next state); accessibility is told its `state`.
    fn tri_state_toggleable(
        self,
        state: ToggleableState,
        #[default(true)] enabled: bool,
        #[default(None)] role: Option<Role>,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        self.tri_state_toggleable_with(state, enabled, role, interaction_source, on_click)
    }

    /// [`tri_state_toggleable`](ToggleableModifiers::tri_state_toggleable) with every parameter, for plain code.
    fn tri_state_toggleable_with(
        self,
        state: ToggleableState,
        enabled: bool,
        role: Option<Role>,
        interaction_source: Option<MutableInteractionSource>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        let common = Common::new(interaction_source, None, true, enabled, None, role, Rc::new(on_click));
        self.then_element(TriStateToggleableElement { state, common })
    }

    /// The same showing `indication` (Compose's `triStateToggleable(state, interactionSource, indication, …)`).
    fn tri_state_toggleable_indication(
        self,
        state: ToggleableState,
        interaction_source: Option<MutableInteractionSource>,
        indication: Option<Rc<dyn Indication>>,
        #[default(true)] enabled: bool,
        #[default(None)] role: Option<Role>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        self.tri_state_toggleable_indication_with(state, interaction_source, indication, enabled, role, on_click)
    }

    /// [`tri_state_toggleable_indication`](ToggleableModifiers::tri_state_toggleable_indication) with every
    /// parameter, for plain code.
    fn tri_state_toggleable_indication_with(
        self,
        state: ToggleableState,
        interaction_source: Option<MutableInteractionSource>,
        indication: Option<Rc<dyn Indication>>,
        enabled: bool,
        role: Option<Role>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        let on_click: Rc<dyn Fn()> = Rc::new(on_click);
        clickable_with_indication_if_needed(self, interaction_source, indication, move |source, factory| {
            let common = Common::new(source, factory, false, enabled, None, role, on_click.clone());
            Modifier::element(TriStateToggleableElement { state, common })
        })
    }
}

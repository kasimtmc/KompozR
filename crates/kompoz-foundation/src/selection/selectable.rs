//! `Modifier.selectable`, as Compose's selection/Selectable.kt.

use crate::clickable::{Common, SelectableElement, clickable_with_indication_if_needed};
use crate::indication::Indication;
use crate::interaction::MutableInteractionSource;
use kompoz_runtime::named;
use kompoz_ui::Modifier;
use kompoz_ui::semantics::Role;
use std::rc::Rc;

/// The selectable modifiers (Compose's `Modifier.selectable` overloads).
#[named(trait SelectableModifiers)]
impl Modifier {
    /// Makes the layout one of a group of choices (Compose's `Modifier.selectable`): a clickable calling `on_click`
    /// that tells accessibility whether it is `selected` (put the group in a `selectable_group`). The rest is
    /// [`clickable`](crate::clickable::ClickableModifiers::clickable)'s.
    fn selectable(
        self,
        selected: bool,
        #[default(true)] enabled: bool,
        #[default(None)] role: Option<Role>,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        self.selectable_with(selected, enabled, role, interaction_source, on_click)
    }

    /// [`selectable`](SelectableModifiers::selectable) with every parameter, for plain code.
    fn selectable_with(
        self,
        selected: bool,
        enabled: bool,
        role: Option<Role>,
        interaction_source: Option<MutableInteractionSource>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        let common = Common::new(interaction_source, None, true, enabled, None, role, Rc::new(on_click));
        self.then_element(SelectableElement { selected, common })
    }

    /// The same showing `indication` (Compose's `selectable(selected, interactionSource, indication, …)`).
    fn selectable_indication(
        self,
        selected: bool,
        interaction_source: Option<MutableInteractionSource>,
        indication: Option<Rc<dyn Indication>>,
        #[default(true)] enabled: bool,
        #[default(None)] role: Option<Role>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        self.selectable_indication_with(selected, interaction_source, indication, enabled, role, on_click)
    }

    /// [`selectable_indication`](SelectableModifiers::selectable_indication) with every parameter, for plain code.
    fn selectable_indication_with(
        self,
        selected: bool,
        interaction_source: Option<MutableInteractionSource>,
        indication: Option<Rc<dyn Indication>>,
        enabled: bool,
        role: Option<Role>,
        on_click: impl Fn() + 'static,
    ) -> Modifier {
        let on_click: Rc<dyn Fn()> = Rc::new(on_click);
        clickable_with_indication_if_needed(self, interaction_source, indication, move |source, factory| {
            let common = Common::new(source, factory, false, enabled, None, role, on_click.clone());
            Modifier::element(SelectableElement { selected, common })
        })
    }
}

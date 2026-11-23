//! The pane expansion drag handle's modifier, as Compose's adaptive-layout PaneExpansionDraggableModifier.kt (1.2.0)
//! with Compose for desktop's actual (PaneExpansionDragHandle.skiko.kt): what a scaffold scope's
//! `pane_expansion_draggable` makes of a drag handle ([`pane_expansion_draggable_internal`]: dragged and settling the
//! split, clicked by its semantics' action, fading as the scaffold moves, at least its touch size to the scaffold),
//! and the default semantics of a drag handle ([`PaneExpansionState::default_drag_handle_semantics`]).

use super::animate_with_fading_modifier::animate_with_fading;
use super::internal::delegable_modifier::{DelegableSemanticsPropertyReceiver, delegable_semantics};
use super::internal::strings::{
    Strings, get_string, get_string_formatted, get_string_of_node, get_string_of_node_formatted,
};
use super::pane_expansion_state::PaneExpansionState;
use super::pane_scaffold::{SemanticsProperties, modify_pane_scaffold_parent_data};
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::gestures::{DraggableModifiers, OnDragStopped, Orientation};
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_foundation::prelude::ClickableModifiers;
use kompoz_graphics::{Density, Dp};
use kompoz_runtime::named_args;
use kompoz_ui::layout::{LookaheadScope, ParentDataModifierNode};
use kompoz_ui::modifier::{ModifierNode, ModifierNodeElement};
use kompoz_ui::semantics::{SemanticsActions, SemanticsConfiguration};
use kompoz_ui::{Modifier, node_capabilities};
use std::any::Any;
use std::rc::Rc;

/// `modifier` making its layout the drag handle of `state` (Compose's internal `Modifier.paneExpansionDraggable`):
/// dragged horizontally through the state (`interaction_source` hearing it), settling on an anchor when let go;
/// clicked (no indication) by the click action `semantics_properties` set, if any; kept from the system's gestures
/// (nothing on the desktop); fading as the scaffold moves by `animate_fraction` in `lookahead_scope`; its semantics
/// `semantics_properties`' merged (else the default ones: what it is, the split's current anchor, a click to the next
/// anchor); at least `min_touch_target_size` to touch, as the scaffold measures it.
pub(crate) fn pane_expansion_draggable_internal(
    modifier: Modifier,
    state: PaneExpansionState,
    min_touch_target_size: Dp,
    interaction_source: MutableInteractionSource,
    semantics_properties: Option<SemanticsProperties>,
    animate_fraction: Rc<dyn Fn() -> f32>,
    lookahead_scope: Rc<dyn LookaheadScope>,
) -> Modifier {
    let settling = state.clone();
    let on_drag_stopped: OnDragStopped = Rc::new(move |_scope: CoroutineScope, velocity: f32| {
        let state = settling.clone();
        Box::pin(async move { state.settle_to_anchor_if_needed(velocity).await })
    });
    let modifier = named_args!(modifier.draggable(
        state.draggable_state(),
        Orientation::Horizontal,
        interaction_source = Some(interaction_source.clone()),
        on_drag_stopped = Some(on_drag_stopped),
    ));
    let modifier = semantics_action(modifier, semantics_properties.as_ref(), interaction_source);
    let modifier = system_gesture_exclusion(modifier);
    let modifier = animate_with_fading(modifier, true, animate_fraction, lookahead_scope, None);
    semantics_with_defaults(modifier, state, semantics_properties)
        .then_element(MinTouchTargetSizeElement { size: min_touch_target_size })
}

impl PaneExpansionState {
    /// A drag handle's default semantics (Compose's composable, deprecated `PaneExpansionState.defaultDragHandleSemantics()`:
    /// leaving the semantics out of `pane_expansion_draggable` gives them): the handle's description, the current
    /// anchor's as its state, and a click (labelled with it) animating to the next anchor, if there is one.
    #[deprecated(
        note = "Just omit the semanticsProperties parameter when using Modifier.paneExpansionDraggable to set up the default semantics"
    )]
    #[track_caller]
    pub fn default_drag_handle_semantics(&self) -> SemanticsProperties {
        let coroutine_scope = kompoz_runtime::kompozable_block!(remember_coroutine_scope());
        let content_desc = get_string(Strings::defaultPaneExpansionDragHandleContentDescription);
        let state_desc = self.current_anchor().map(|anchor| {
            get_string_formatted(Strings::defaultPaneExpansionDragHandleStateDescription, &[&anchor.description()])
        });
        let next_anchor = self.next_anchor();
        let action_label = next_anchor.map(|anchor| {
            get_string_formatted(Strings::defaultPaneExpansionDragHandleActionDescription, &[&anchor.description()])
        });
        let state = self.clone();
        Rc::new(move |config: &mut SemanticsConfiguration| {
            config.set_content_description(content_desc.clone());
            if let Some(state_desc) = &state_desc {
                config.set_state_description(state_desc.clone());
            }
            let Some(next_anchor) = next_anchor else {
                // TODO(conrachen): handle this case
                return;
            };
            let (state, coroutine_scope) = (state.clone(), coroutine_scope.clone());
            named_args!(config.on_click(
                label = action_label.clone(),
                action = move || {
                    named_args!(coroutine_scope.launch(state.animate_to(next_anchor, 0.0)));
                    true
                }
            ));
        })
    }
}

/// Keeps the layout's area from the system's gestures (Compose's internal expect `Modifier.systemGestureExclusion`;
/// Compose for desktop's actual: the modifier itself, the desktop has none).
fn system_gesture_exclusion(modifier: Modifier) -> Modifier {
    modifier
}

/// `modifier` clickable (no indication, `interaction_source` hearing it) by the click action `semantics_properties`
/// set, if it sets one (Compose's private `Modifier.semanticsAction`).
fn semantics_action(
    modifier: Modifier,
    semantics_properties: Option<&SemanticsProperties>,
    interaction_source: MutableInteractionSource,
) -> Modifier {
    match semantics_properties.and_then(on_click_action) {
        Some(action) => modifier.then(named_args!(Modifier.clickable_indication(
            Some(interaction_source),
            None,
            on_click = move || {
                action();
            }
        ))),
        None => modifier,
    }
}

/// The click action the semantics block sets, if any (Compose's private `onClickAction`: the block run on a
/// configuration of its own).
fn on_click_action(semantics_properties: &SemanticsProperties) -> Option<Rc<dyn Fn() -> bool>> {
    let mut config = SemanticsConfiguration::new();
    semantics_properties(&mut config);
    config.get(SemanticsActions::OnClick).and_then(|a| a.action)
}

/// `modifier` with the given semantics merged, else the default ones read through the node (Compose's private
/// `Modifier.semanticsWithDefaults`).
fn semantics_with_defaults(
    modifier: Modifier,
    state: PaneExpansionState,
    semantics_properties: Option<SemanticsProperties>,
) -> Modifier {
    match semantics_properties {
        Some(properties) => modifier.semantics_with(true, move |c| properties(c)),
        None => delegable_semantics(
            modifier,
            true,
            Rc::new(move |r: &mut DelegableSemanticsPropertyReceiver| {
                let node = r.node().clone();
                let content_desc = get_string_of_node(&node, Strings::defaultPaneExpansionDragHandleContentDescription);
                let state_desc = state.current_anchor().map(|anchor| {
                    get_string_of_node_formatted(
                        &node,
                        Strings::defaultPaneExpansionDragHandleStateDescription,
                        &[&anchor.description_of_node(&node)],
                    )
                });
                let next_anchor = state.next_anchor();
                let action_label = next_anchor.map(|anchor| {
                    get_string_of_node_formatted(
                        &node,
                        Strings::defaultPaneExpansionDragHandleActionDescription,
                        &[&anchor.description_of_node(&node)],
                    )
                });
                r.set_content_description(content_desc);
                if let Some(state_desc) = state_desc {
                    r.set_state_description(state_desc);
                }
                let Some(next_anchor) = next_anchor else {
                    // TODO(conrachen): handle this case
                    return;
                };
                let (state, coroutine_scope) = (state.clone(), r.coroutine_scope());
                named_args!(r.on_click(
                    label = action_label,
                    action = move || {
                        named_args!(coroutine_scope.launch(state.animate_to(next_anchor, 0.0)));
                        true
                    }
                ));
            }),
        ),
    }
}

/// Compose's private data class `MinTouchTargetSizeElement`.
#[derive(PartialEq)]
struct MinTouchTargetSizeElement {
    size: Dp,
}

impl ModifierNodeElement for MinTouchTargetSizeElement {
    type Node = MinTouchTargetSizeNode;

    fn create(&self) -> MinTouchTargetSizeNode {
        MinTouchTargetSizeNode { size: self.size }
    }

    fn update(&self, node: &mut MinTouchTargetSizeNode) {
        node.size = self.size;
    }

    fn name(&self) -> &'static str {
        "minTouchTargetSize"
    }
}

/// The least size to touch the handle by, as the scaffold's parent data (Compose's private `MinTouchTargetSizeNode`).
struct MinTouchTargetSizeNode {
    size: Dp,
}

impl ModifierNode for MinTouchTargetSizeNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for MinTouchTargetSizeNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let size = self.size;
        modify_pane_scaffold_parent_data(parent_data, |d| d.min_touch_target_size = size)
    }
}

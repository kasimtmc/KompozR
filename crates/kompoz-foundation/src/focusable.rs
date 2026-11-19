//! Focusables and focus groups, as Compose's Focusable.kt: `Modifier.focusable` (a focus target that tells its
//! focus to an interaction source, to accessibility, to the scroll containers it is in and to a lazy layout's
//! pinning), and `Modifier.focusGroup` (a focus target that never takes the focus itself, so its children are
//! searched as one group: arrow keys stay in it first; from outside it is one target).

use crate::focused_bounds::notify_focused_bounds_observers;
use crate::interaction::{FocusInteraction, IntoInteraction, MutableInteractionSource};
use kompoz_coroutines::CoroutineStart;
use kompoz_runtime::{named, named_args};
use kompoz_ui::focus::{FocusChangeCallback, FocusState, FocusTargetNode, Focusability, bring_into_view};
use kompoz_ui::layout::{GlobalPositionAwareModifierNode, LayoutCoordinates, LocalPinnableContainer, PinnedHandle};
use kompoz_ui::modifier::{DelegatedNode, ObserverModifierNode, TraversableNode};
use kompoz_ui::semantics::{
    AccessibilityAction, ActionFn, SemanticsActions, SemanticsConfiguration, SemanticsModifierNode,
};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The focus modifiers of foundation.
#[named(trait FocusableModifiers)]
impl Modifier {
    /// Makes the layout focusable (Compose's `Modifier.focusable`): reached by the focus system and accessibility's
    /// focus, its focus told to `interaction_source` ([`FocusInteraction`]s) and to accessibility, brought into view
    /// when it takes it. Not `enabled`: no part of the focus.
    fn focusable(
        self,
        #[default(true)] enabled: bool,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
    ) -> Modifier {
        if enabled { self.then_element(FocusableElement { interaction_source }) } else { self }
    }

    /// [`focusable`](FocusableModifiers::focusable) with every parameter, for plain code.
    fn focusable_with(self, enabled: bool, interaction_source: Option<MutableInteractionSource>) -> Modifier {
        if enabled { self.then_element(FocusableElement { interaction_source }) } else { self }
    }

    /// Makes the layout a focus group (Compose's `Modifier.focusGroup`): a target its children's focus goes
    /// through, never focused itself.
    fn focus_group(self) -> Modifier {
        self.then_element(FocusGroupElement)
    }
}

#[derive(PartialEq)]
struct FocusGroupElement;

impl ModifierNodeElement for FocusGroupElement {
    type Node = FocusTargetNode;
    fn create(&self) -> FocusTargetNode {
        FocusTargetNode::new(Focusability::Never, None)
    }
    fn update(&self, _node: &mut FocusTargetNode) {}
    fn name(&self) -> &'static str {
        "focusGroup"
    }
}

/// `Modifier.focusable`'s element (Compose's FocusableElement): equal for the same source.
#[derive(PartialEq)]
struct FocusableElement {
    interaction_source: Option<MutableInteractionSource>,
}

impl ModifierNodeElement for FocusableElement {
    type Node = FocusableNode;
    fn create(&self) -> FocusableNode {
        FocusableNode::new(self.interaction_source.clone(), Focusability::Always, None)
    }
    fn update(&self, node: &mut FocusableNode) {
        node.update(self.interaction_source.clone());
    }
    fn name(&self) -> &'static str {
        "focusable"
    }
}

/// What focusables are found by (Compose's FocusableNode.TraverseKey).
pub(crate) const FocusableNodeTraverseKey: &str = "androidx.compose.foundation.FocusableNode";

/// What a [`FocusableNode`] and its focus target's callback share.
pub(crate) struct FocusableShared {
    interaction_source: RefCell<Option<MutableInteractionSource>>,
    /// Told whether it is focused when that changes (a clickable's).
    on_focus_change: Option<Rc<dyn Fn(bool)>>,
    focused_interaction: Cell<Option<FocusInteraction::Focus>>,
    pinned_handle: RefCell<Option<PinnedHandle>>,
    global_layout_coordinates: RefCell<Option<LayoutCoordinates>>,
    /// The focusable node's.
    handle: RefCell<Option<NodeHandle>>,
    /// Its focus target's.
    target: RefCell<Option<NodeHandle>>,
}

impl FocusableShared {
    fn handle(&self) -> Option<NodeHandle> {
        self.handle.borrow().clone()
    }

    fn target(&self) -> Option<NodeCx> {
        self.target.borrow().as_ref().and_then(NodeHandle::cx)
    }

    fn is_attached(&self) -> bool {
        self.handle().is_some_and(|h| h.is_attached())
    }

    /// Its focus target's state.
    fn focus_state(&self) -> FocusState {
        self.target().map_or(FocusState::Inactive, |t| t.focus_state())
    }

    /// Moves the focus here (Compose's `requestFocus`).
    fn request_focus(&self) -> bool {
        self.target().is_some_and(|t| t.request_focus())
    }

    /// Compose's `onFocusStateChange`: what it does as it takes the focus or loses it.
    fn on_focus_state_change(&self, previous: FocusState, current: FocusState) {
        if !self.is_attached() {
            return;
        }
        let (is_focused, was_focused) = (current.is_focused(), previous.is_focused());
        // (initialized unfocused, or between unfocused states: Inactive to ActiveParent)
        if is_focused == was_focused {
            return;
        }
        if let Some(f) = &self.on_focus_change {
            f(is_focused);
        }
        let handle = self.handle();
        if is_focused {
            if let Some(cx) = handle.as_ref().and_then(NodeHandle::cx)
                && let Some(scope) = handle.as_ref().and_then(NodeHandle::coroutine_scope)
            {
                named_args!(scope.launch(start = CoroutineStart::UNDISPATCHED, coroutine = bring_into_view(&cx, None)));
            }
            let container = self.retrieve_pinnable_container();
            *self.pinned_handle.borrow_mut() = container.map(|c| c.pin());
            self.notify_observer_when_attached();
        } else {
            let pinned = self.pinned_handle.borrow_mut().take();
            if let Some(p) = pinned {
                p.release();
            }
            if let Some(cx) = handle.as_ref().and_then(NodeHandle::cx) {
                notify_focused_bounds_observers(&cx, None);
            }
        }
        if let Some(h) = &handle {
            h.invalidate_semantics();
        }
        self.emit_interaction(is_focused);
    }

    /// The pinnable container it is in, read observed (Compose's `retrievePinnableContainer`).
    fn retrieve_pinnable_container(&self) -> Option<kompoz_ui::layout::PinnableContainerRef> {
        let handle = self.handle()?;
        handle.observe_reads(|| handle.current_value_of(&LocalPinnableContainer))
    }

    fn notify_observer_when_attached(&self) {
        let coordinates = self.global_layout_coordinates.borrow().clone();
        if let Some(c) = coordinates
            && c.is_attached()
            && let Some(cx) = self.handle().and_then(|h| h.cx())
        {
            notify_focused_bounds_observers(&cx, Some(c));
        }
    }

    /// Tells its source the focus came or went (Compose's `emitInteraction`).
    fn emit_interaction(&self, is_focused: bool) {
        let Some(source) = self.interaction_source.borrow().clone() else { return };
        if let Some(old) = self.focused_interaction.take() {
            self.emit_with_fallback(&source, FocusInteraction::Unfocus::new(old));
        }
        if is_focused {
            let focus = FocusInteraction::Focus::new();
            self.emit_with_fallback(&source, focus);
            self.focused_interaction.set(Some(focus));
        }
    }

    /// Emits in its coroutine scope while it is attached, else at once (Compose's `emitWithFallback`): called from
    /// its detach, the scope is cancelled before the launched emit runs, so the scope's end tries the emit unless
    /// the coroutine got to it.
    fn emit_with_fallback(&self, source: &MutableInteractionSource, interaction: impl crate::interaction::Interaction) {
        match self.handle().and_then(|h| h.coroutine_scope()) {
            Some(scope) => {
                let interaction = interaction.into_interaction();
                let (fallback, late) = (source.clone(), interaction.clone());
                let handler = scope.invoke_on_completion(move || {
                    fallback.try_emit(late);
                });
                let source = source.clone();
                scope.launch(async move {
                    source.emit(interaction).await;
                    handler.dispose();
                });
            }
            None => {
                source.try_emit(interaction);
            }
        }
    }

    /// Ends the focus interaction told to the source (Compose's `disposeInteractionSource`).
    fn dispose_interaction_source(&self) {
        let source = self.interaction_source.borrow().clone();
        if let (Some(source), Some(old)) = (source, self.focused_interaction.take()) {
            source.try_emit(FocusInteraction::Unfocus::new(old));
        }
    }
}

/// The node of `Modifier.focusable` (Compose's FocusableNode), and what a clickable delegates to for its focus: it
/// delegates to a focus target of its own (`focusability`), and as the focus comes and goes tells the source, the
/// optional `on_focus_change`, accessibility (`Focused`, the `RequestFocus` action), the scroll containers it is in
/// (brought into view), the focused-bounds observers above it, and the pinnable container it is in (pinned while
/// focused).
pub struct FocusableNode {
    shared: Rc<FocusableShared>,
    focusability: Focusability,
    target: Option<DelegatedNode<FocusTargetNode>>,
}

impl FocusableNode {
    /// A focusable telling `interaction_source` its focus (Compose's `FocusableNode(interactionSource,
    /// focusability, onFocusChange)`).
    pub fn new(
        interaction_source: Option<MutableInteractionSource>,
        focusability: Focusability,
        on_focus_change: Option<Rc<dyn Fn(bool)>>,
    ) -> FocusableNode {
        FocusableNode {
            shared: Rc::new(FocusableShared {
                interaction_source: RefCell::new(interaction_source),
                on_focus_change,
                focused_interaction: Cell::new(None),
                pinned_handle: RefCell::new(None),
                global_layout_coordinates: RefCell::new(None),
                handle: RefCell::new(None),
                target: RefCell::new(None),
            }),
            focusability,
            target: None,
        }
    }

    /// A focusable of `shared`'s state (a clickable's, delegated to again after it was taken back: its focus
    /// target made anew).
    pub(crate) fn from_shared(shared: Rc<FocusableShared>, focusability: Focusability) -> FocusableNode {
        FocusableNode { shared, focusability, target: None }
    }

    /// Another source: the focus told to the old one ended (Compose's `update`).
    pub fn update(&mut self, interaction_source: Option<MutableInteractionSource>) {
        Self::update_shared(&self.shared, interaction_source);
    }

    pub(crate) fn update_shared(shared: &FocusableShared, interaction_source: Option<MutableInteractionSource>) {
        if *shared.interaction_source.borrow() != interaction_source {
            shared.dispose_interaction_source();
            *shared.interaction_source.borrow_mut() = interaction_source;
        }
    }

    /// Its focus target's state (Compose's `focusState`).
    pub fn focus_state(&self) -> FocusState {
        self.shared.focus_state()
    }

    /// Moves the focus to it (Compose's `requestFocus`); whether it moved.
    pub fn request_focus(&self) -> bool {
        self.shared.request_focus()
    }

    pub(crate) fn shared(&self) -> &Rc<FocusableShared> {
        &self.shared
    }
}

impl ModifierNode for FocusableNode {
    node_capabilities!(semantics, global_position_aware, observer, traversable);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.shared.handle.borrow_mut() = Some(cx.handle());
        if self.target.is_none() {
            let shared = self.shared.clone();
            let on_change: FocusChangeCallback = Rc::new(move |previous, current| {
                shared.on_focus_state_change(previous, current);
            });
            let target = cx.delegate(FocusTargetNode::new(self.focusability, Some(on_change)));
            *self.shared.target.borrow_mut() = Some(target.cx().handle());
            self.target = Some(target);
        }
    }

    fn on_reset(&mut self) {
        let pinned = self.shared.pinned_handle.borrow_mut().take();
        if let Some(p) = pinned {
            p.release();
        }
    }
}

impl ObserverModifierNode for FocusableNode {
    /// The pinnable container changed: pinned in the new one while focused (Compose's `onObservedReadsChanged`).
    fn on_observed_reads_changed(&mut self) {
        let container = self.shared.retrieve_pinnable_container();
        if self.shared.focus_state().is_focused() {
            let old = self.shared.pinned_handle.borrow_mut().take();
            if let Some(p) = old {
                p.release();
            }
            *self.shared.pinned_handle.borrow_mut() = container.map(|c| c.pin());
        }
    }
}

impl GlobalPositionAwareModifierNode for FocusableNode {
    /// Where it is, told to the focused-bounds observers above it while it is focused (Compose's
    /// `onGloballyPositioned`).
    fn on_globally_positioned(&mut self, coordinates: &LayoutCoordinates) {
        *self.shared.global_layout_coordinates.borrow_mut() = Some(coordinates.clone());
        if !self.shared.focus_state().is_focused() {
            return;
        }
        if coordinates.is_attached() {
            self.shared.notify_observer_when_attached();
        } else if let Some(cx) = self.shared.handle().and_then(|h| h.cx()) {
            notify_focused_bounds_observers(&cx, None);
        }
    }
}

impl SemanticsModifierNode for FocusableNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        apply_focusable_semantics(&self.shared, config);
    }
}

/// A focusable's semantics (Compose's FocusableNode.applySemantics): whether it is focused, and the action moving
/// the focus to it.
pub(crate) fn apply_focusable_semantics(shared: &Rc<FocusableShared>, config: &mut SemanticsConfiguration) {
    config.set_focused(shared.focus_state().is_focused());
    let shared = shared.clone();
    let action: Rc<ActionFn> = Rc::new(move || shared.request_focus());
    config.set(SemanticsActions::RequestFocus, AccessibilityAction::new(None, Some(action)));
}

impl TraversableNode for FocusableNode {
    fn traverse_key(&self) -> &dyn Any {
        &FocusableNodeTraverseKey
    }
}

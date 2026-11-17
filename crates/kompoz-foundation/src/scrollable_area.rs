//! A scroll container of a layout placing its content itself, as Compose's ScrollableArea.kt:
//! `Modifier.scrollable_area` is a scrollable reversed so its content moves with the finger (right to left too),
//! clipped along its axis, its overscroll effect (the factory's or its own) drawn over it.

// (Compose's parameter lists: the methods of a `#[named]` trait carry no attributes of their own)
#![allow(clippy::too_many_arguments)]

use crate::clip_scrollable_container::ClipScrollableContainerModifiers;
use crate::gestures::scrollable::{ScrollableElement, ScrollableNode};
use crate::gestures::{BringIntoViewSpec, FlingBehavior, Orientation, ScrollableDefaults, ScrollableState};
use crate::interaction::MutableInteractionSource;
use crate::overscroll::{
    LocalOverscrollFactory, OverscrollEffect, OverscrollFactory, same_effect, undelegate_if_delegated,
};
use kompoz_graphics::LayoutDirection;
use kompoz_runtime::named;
use kompoz_ui::modifier::{DelegatableNode, DelegatedNode, ObserverModifierNode};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::rc::Rc;

/// The scrollable areas.
#[named(trait ScrollableAreaModifiers)]
impl Modifier {
    /// Scrolls `state` along `orientation` as a container scrolls (Compose's `Modifier.scrollableArea`): a
    /// scrollable whose deltas move the content with the finger (`reverse_scrolling`: from its end; right to left
    /// horizontally mirrored), clipped along its axis (with room across), [`LocalOverscrollFactory`]'s overscroll
    /// drawn over its content. For a layout that places its content by the state itself (`vertical_scroll` is one
    /// with its own layout). Not `enabled`: not by the user; the clip stays.
    fn scrollable_area(
        self,
        state: impl ScrollableState,
        orientation: Orientation,
        #[default(true)] enabled: bool,
        #[default(false)] reverse_scrolling: bool,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(None)] bring_into_view_spec: Option<Rc<dyn BringIntoViewSpec>>,
    ) -> Modifier {
        scrollable_area_of(
            self,
            Rc::new(state),
            orientation,
            enabled,
            reverse_scrolling,
            fling_behavior,
            interaction_source,
            bring_into_view_spec,
            true,
            None,
        )
    }

    /// The same with `overscroll_effect` (none: no overscroll) instead of the factory's (Compose's
    /// `scrollableArea(state, orientation, overscrollEffect, …)`).
    /// Kotlin: `scrollableArea(state: ScrollableState, orientation: Orientation, overscrollEffect: OverscrollEffect?, enabled: Boolean = true, reverseScrolling: Boolean = false, flingBehavior: FlingBehavior? = null, interactionSource: MutableInteractionSource? = null, bringIntoViewSpec: BringIntoViewSpec? = null)`
    fn scrollable_area_with_overscroll(
        self,
        state: impl ScrollableState,
        orientation: Orientation,
        overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        #[default(true)] enabled: bool,
        #[default(false)] reverse_scrolling: bool,
        #[default(None)] fling_behavior: Option<Rc<dyn FlingBehavior>>,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        #[default(None)] bring_into_view_spec: Option<Rc<dyn BringIntoViewSpec>>,
    ) -> Modifier {
        scrollable_area_of(
            self,
            Rc::new(state),
            orientation,
            enabled,
            reverse_scrolling,
            fling_behavior,
            interaction_source,
            bring_into_view_spec,
            false,
            overscroll_effect,
        )
    }
}

/// Compose's two `scrollableArea`s: the clip, then the area's node.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scrollable_area_of(
    m: Modifier,
    state: Rc<dyn ScrollableState>,
    orientation: Orientation,
    enabled: bool,
    reverse_scrolling: bool,
    fling_behavior: Option<Rc<dyn FlingBehavior>>,
    interaction_source: Option<MutableInteractionSource>,
    bring_into_view_spec: Option<Rc<dyn BringIntoViewSpec>>,
    use_local_overscroll_factory: bool,
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
) -> Modifier {
    m.clip_scrollable_container(orientation).then_element(ScrollableAreaElement {
        state,
        orientation,
        enabled,
        reverse_scrolling,
        fling_behavior,
        interaction_source,
        bring_into_view_spec,
        use_local_overscroll_factory,
        overscroll_effect,
    })
}

struct ScrollableAreaElement {
    state: Rc<dyn ScrollableState>,
    orientation: Orientation,
    enabled: bool,
    reverse_scrolling: bool,
    fling_behavior: Option<Rc<dyn FlingBehavior>>,
    interaction_source: Option<MutableInteractionSource>,
    bring_into_view_spec: Option<Rc<dyn BringIntoViewSpec>>,
    use_local_overscroll_factory: bool,
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
}

impl PartialEq for ScrollableAreaElement {
    fn eq(&self, o: &Self) -> bool {
        *self.state == *o.state
            && self.orientation == o.orientation
            && self.enabled == o.enabled
            && self.reverse_scrolling == o.reverse_scrolling
            && crate::gestures::fling::same_fling_behavior(&self.fling_behavior, &o.fling_behavior)
            && self.interaction_source == o.interaction_source
            && crate::gestures::bring_into_view_spec::same_spec(&self.bring_into_view_spec, &o.bring_into_view_spec)
            && self.use_local_overscroll_factory == o.use_local_overscroll_factory
            && same_effect(&self.overscroll_effect, &o.overscroll_effect)
    }
}

impl ModifierNodeElement for ScrollableAreaElement {
    type Node = ScrollableAreaNode;
    fn create(&self) -> ScrollableAreaNode {
        ScrollableAreaNode {
            state: self.state.clone(),
            orientation: self.orientation,
            enabled: self.enabled,
            reverse_scrolling: self.reverse_scrolling,
            fling_behavior: self.fling_behavior.clone(),
            interaction_source: self.interaction_source.clone(),
            bring_into_view_spec: self.bring_into_view_spec.clone(),
            use_local_overscroll_factory: self.use_local_overscroll_factory,
            user_provided_overscroll_effect: self.overscroll_effect.clone(),
            scrollable_node: None,
            overscroll_node: None,
            local_overscroll_factory: None,
            local_overscroll_factory_created_overscroll_effect: None,
            should_reverse_direction: false,
            handle: None,
        }
    }
    fn update(&self, node: &mut ScrollableAreaNode) {
        node.update(
            self.state.clone(),
            self.orientation,
            self.use_local_overscroll_factory,
            self.overscroll_effect.clone(),
            self.enabled,
            self.reverse_scrolling,
            self.fling_behavior.clone(),
            self.interaction_source.clone(),
            self.bring_into_view_spec.clone(),
        );
    }
    fn name(&self) -> &'static str {
        "scrollableArea"
    }
}

/// `scrollable_area`'s node (Compose's ScrollableAreaNode): it delegates to a scrollable reversed as the layout
/// direction says, and to the overscroll effect's node.
pub struct ScrollableAreaNode {
    state: Rc<dyn ScrollableState>,
    orientation: Orientation,
    enabled: bool,
    reverse_scrolling: bool,
    fling_behavior: Option<Rc<dyn FlingBehavior>>,
    interaction_source: Option<MutableInteractionSource>,
    bring_into_view_spec: Option<Rc<dyn BringIntoViewSpec>>,
    use_local_overscroll_factory: bool,
    user_provided_overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
    scrollable_node: Option<DelegatedNode<ScrollableNode>>,
    overscroll_node: Option<DelegatableNode>,
    local_overscroll_factory: Option<Rc<dyn OverscrollFactory>>,
    local_overscroll_factory_created_overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
    should_reverse_direction: bool,
    handle: Option<NodeHandle>,
}

impl ScrollableAreaNode {
    fn cx(&self) -> Option<NodeCx> {
        self.handle.as_ref().and_then(|h| h.cx())
    }

    /// The effect it uses (Compose's `getOverscrollEffect`).
    fn get_overscroll_effect(&self) -> Option<Rc<dyn OverscrollEffect>> {
        if self.use_local_overscroll_factory {
            self.local_overscroll_factory_created_overscroll_effect.clone()
        } else {
            self.user_provided_overscroll_effect.clone()
        }
    }

    /// Its scrollable's element (what makes and updates it).
    fn scrollable_element(&self) -> ScrollableElement {
        ScrollableElement {
            state: self.state.clone(),
            orientation: self.orientation,
            overscroll_effect: self.get_overscroll_effect(),
            enabled: self.enabled,
            reverse_direction: self.should_reverse_direction,
            fling_behavior: self.fling_behavior.clone(),
            interaction_source: self.interaction_source.clone(),
            bring_into_view_spec: self.bring_into_view_spec.clone(),
        }
    }

    /// Compose's `shouldReverseDirection`.
    fn should_reverse_direction(&self) -> bool {
        let layout_direction =
            self.cx().filter(|c| c.is_attached()).map_or(LayoutDirection::Ltr, |c| c.layout_direction());
        ScrollableDefaults::reverse_direction(layout_direction, self.orientation, self.reverse_scrolling)
    }

    /// Compose's `update`.
    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        state: Rc<dyn ScrollableState>,
        orientation: Orientation,
        use_local_overscroll_factory: bool,
        overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
        enabled: bool,
        reverse_scrolling: bool,
        fling_behavior: Option<Rc<dyn FlingBehavior>>,
        interaction_source: Option<MutableInteractionSource>,
        bring_into_view_spec: Option<Rc<dyn BringIntoViewSpec>>,
    ) {
        self.state = state;
        self.orientation = orientation;
        let mut use_local_overscroll_factory_changed = false;
        if self.use_local_overscroll_factory != use_local_overscroll_factory {
            use_local_overscroll_factory_changed = true;
            self.use_local_overscroll_factory = use_local_overscroll_factory;
        }
        let mut overscroll_effect_changed = false;
        if !same_effect(&self.user_provided_overscroll_effect, &overscroll_effect) {
            overscroll_effect_changed = true;
            self.user_provided_overscroll_effect = overscroll_effect;
        }
        // (the effect changed while the factory's is used: nothing to do)
        if use_local_overscroll_factory_changed || overscroll_effect_changed && !use_local_overscroll_factory {
            if let (Some(node), Some(cx)) = (self.overscroll_node.take(), self.cx()) {
                undelegate_if_delegated(&cx, &node);
            }
            if let Some(cx) = self.cx().filter(|c| c.is_attached()) {
                self.attach_overscroll_node_if_needed(&cx);
            }
        }
        self.enabled = enabled;
        self.reverse_scrolling = reverse_scrolling;
        self.fling_behavior = fling_behavior;
        self.interaction_source = interaction_source;
        self.bring_into_view_spec = bring_into_view_spec;
        self.should_reverse_direction = self.should_reverse_direction();
        let element = self.scrollable_element();
        if let Some(node) = &self.scrollable_node {
            node.borrow().core.update(&element);
        }
    }

    /// Compose's `attachOverscrollNodeIfNeeded`: the factory's effect made (its local read observed), and its
    /// node delegated to unless it is attached elsewhere.
    fn attach_overscroll_node_if_needed(&mut self, cx: &NodeCx) {
        if self.overscroll_node.is_none() {
            // (the factory's effect over the one given)
            if self.use_local_overscroll_factory {
                let factory = cx.observe_reads(|| cx.current_value_of(&LocalOverscrollFactory));
                self.local_overscroll_factory_created_overscroll_effect =
                    factory.as_ref().map(|f| f.create_overscroll_effect());
                self.local_overscroll_factory = factory;
            }
            if let Some(effect) = self.get_overscroll_effect() {
                let node = effect.node();
                if !node.is_attached() {
                    cx.delegate_node(&node);
                    self.overscroll_node = Some(node);
                }
            }
        } else if let Some(node) = &self.overscroll_node {
            // (delegated again if it isn't: nothing if it is this node's already)
            if !node.is_attached() {
                cx.delegate_node(node);
            }
        }
    }
}

impl ModifierNode for ScrollableAreaNode {
    node_capabilities!(observer);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    /// Compose's `onAttach`.
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.should_reverse_direction = self.should_reverse_direction();
        self.attach_overscroll_node_if_needed(cx);
        if self.scrollable_node.is_none() {
            let element = self.scrollable_element();
            self.scrollable_node = Some(cx.delegate(ScrollableNode::new(&element)));
        }
    }

    /// Compose's `onDetach`.
    fn on_detach(&mut self, cx: &NodeCx) {
        if let Some(node) = &self.overscroll_node {
            undelegate_if_delegated(cx, node);
        }
    }

    /// Compose's `onLayoutDirectionChange`.
    fn on_layout_direction_change(&mut self, _cx: &NodeCx) {
        let reverse_direction = self.should_reverse_direction();
        if self.should_reverse_direction != reverse_direction {
            self.should_reverse_direction = reverse_direction;
            let effect = self.get_overscroll_effect();
            self.update(
                self.state.clone(),
                self.orientation,
                self.use_local_overscroll_factory,
                effect,
                self.enabled,
                self.reverse_scrolling,
                self.fling_behavior.clone(),
                self.interaction_source.clone(),
                self.bring_into_view_spec.clone(),
            );
        }
    }
}

impl ObserverModifierNode for ScrollableAreaNode {
    /// Compose's `onObservedReadsChanged`: another factory makes another effect.
    fn on_observed_reads_changed(&mut self) {
        let Some(cx) = self.cx() else { return };
        let factory = cx.current_value_of(&LocalOverscrollFactory);
        let same = match (&factory, &self.local_overscroll_factory) {
            (Some(a), Some(b)) => **a == **b,
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.local_overscroll_factory = factory;
            self.local_overscroll_factory_created_overscroll_effect = None;
            if let Some(node) = self.overscroll_node.take() {
                undelegate_if_delegated(&cx, &node);
            }
            self.attach_overscroll_node_if_needed(&cx);
            let element = self.scrollable_element();
            if let Some(node) = &self.scrollable_node {
                node.borrow().core.update(&element);
            }
        }
    }
}

//! Indication, as Compose's Indication.kt: what a component shows of its interactions (a ripple, a highlight while
//! pressed, hovered or focused) — an [`IndicationNodeFactory`] makes a node drawing a source's interactions for each
//! component; [`LocalIndication`] is what clickables use by default ([`DefaultDebugIndication`] unless a theme
//! provides one); [`Modifier.indication`](IndicationModifiers::indication) draws a source's on any layout.

use crate::interaction::{FocusInteraction, HoverInteraction, Interaction, InteractionSource, PressInteraction};
use kompoz_graphics::Color;
use kompoz_runtime::KompozitionLocal;
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::modifier::{DelegatedNode, TraversableNode};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::any::Any;
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// What a component shows of its interactions (Compose's Indication). Every indication of KompozR's is an
/// [`IndicationNodeFactory`] (Compose's `rememberUpdatedInstance` is deprecated as an error there); one that isn't
/// draws nothing more than the content, as Compose's `NoIndicationInstance`. Indications compare with `==` (Compose
/// requires `equals`): an equal one keeps the node made, another makes a new one.
pub trait Indication: Any + IndicationEq {
    /// It as a node factory, if it is one (Compose's `is IndicationNodeFactory`).
    fn as_node_factory(&self) -> Option<&dyn IndicationNodeFactory> {
        None
    }
}

/// An [`Indication`] that makes a node per component (Compose's IndicationNodeFactory): the node draws (it is a
/// draw node, or delegates to one: its `draw` calls `draw_content` and draws its effects around it) what
/// `interaction_source` tells it. It says it is a factory in [`Indication::as_node_factory`].
pub trait IndicationNodeFactory: Indication {
    /// A node drawing what `interaction_source` emits (Compose's `create`): made again for another source, or when
    /// the factory is no longer equal to the one that made it.
    fn create(&self, interaction_source: InteractionSource) -> Box<dyn ModifierNode>;
}

/// The equality an [`Indication`] has, from its `PartialEq` (Compose's required `equals`).
pub trait IndicationEq {
    fn indication_eq(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: PartialEq + Any> IndicationEq for T {
    fn indication_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PartialEq for dyn Indication {
    fn eq(&self, other: &dyn Indication) -> bool {
        self.indication_eq(other.as_any())
    }
}

impl PartialEq for dyn IndicationNodeFactory {
    fn eq(&self, other: &dyn IndicationNodeFactory) -> bool {
        self.indication_eq(other.as_any())
    }
}

impl fmt::Debug for dyn Indication {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Indication")
    }
}

/// The indication clickables show unless given one (Compose's LocalIndication): [`DefaultDebugIndication`] by
/// default; a theme provides its own (Material's ripple).
pub static LocalIndication: KompozitionLocal<Rc<dyn Indication>> =
    KompozitionLocal::new(|| Rc::new(DefaultDebugIndication));

/// Compose's DefaultDebugIndication, what [`LocalIndication`] gives where nothing provides another: a translucent
/// black rect over the content, darker while pressed than while hovered or focused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DefaultDebugIndication;

impl Indication for DefaultDebugIndication {
    fn as_node_factory(&self) -> Option<&dyn IndicationNodeFactory> {
        Some(self)
    }
}

impl IndicationNodeFactory for DefaultDebugIndication {
    fn create(&self, interaction_source: InteractionSource) -> Box<dyn ModifierNode> {
        Box::new(DefaultDebugIndicationNode {
            interaction_source,
            shown: Rc::new(DebugShown {
                pressed: Cell::new(false),
                hovered: Cell::new(false),
                focused: Cell::new(false),
            }),
        })
    }
}

/// What the debug indication shows.
struct DebugShown {
    pressed: Cell<bool>,
    hovered: Cell<bool>,
    focused: Cell<bool>,
}

/// The node of [`DefaultDebugIndication`] (Compose's DefaultDebugIndicationInstance): it counts its source's
/// presses, hovers and focuses as they come, drawn again when what it shows changes.
struct DefaultDebugIndicationNode {
    interaction_source: InteractionSource,
    shown: Rc<DebugShown>,
}

impl ModifierNode for DefaultDebugIndicationNode {
    node_capabilities!(draw);

    fn on_attach(&mut self, cx: &NodeCx) {
        let (flow, shown, handle) = (self.interaction_source.interactions(), self.shown.clone(), cx.handle());
        let (mut presses, mut hovers, mut focuses) = (0, 0, 0);
        cx.coroutine_scope().launch(flow.collect(move |interaction: Arc<dyn Interaction>| {
            let i = &*interaction;
            if i.is::<PressInteraction::Press>() {
                presses += 1;
            } else if i.is::<PressInteraction::Release>() || i.is::<PressInteraction::Cancel>() {
                presses -= 1;
            } else if i.is::<HoverInteraction::Enter>() {
                hovers += 1;
            } else if i.is::<HoverInteraction::Exit>() {
                hovers -= 1;
            } else if i.is::<FocusInteraction::Focus>() {
                focuses += 1;
            } else if i.is::<FocusInteraction::Unfocus>() {
                focuses -= 1;
            }
            let pressed = shown.pressed.replace(presses > 0) != (presses > 0);
            let hovered = shown.hovered.replace(hovers > 0) != (hovers > 0);
            let focused = shown.focused.replace(focuses > 0) != (focuses > 0);
            if pressed || hovered || focused {
                handle.invalidate_draw();
            }
        }));
    }
}

impl DrawModifierNode for DefaultDebugIndicationNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        scope.draw_content();
        if self.shown.pressed.get() {
            scope.draw_rect(Color::new(0.0, 0.0, 0.0, 0.3));
        } else if self.shown.hovered.get() || self.shown.focused.get() {
            scope.draw_rect(Color::new(0.0, 0.0, 0.0, 0.1));
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The node a factory made, delegated to

/// A node an [`IndicationNodeFactory`] made, as a node of its own type: what a clickable (or `Modifier.indication`)
/// delegates to. Its capabilities and lifecycle are the node's.
pub struct IndicationNode(Box<dyn ModifierNode>);

impl IndicationNode {
    /// The node `factory` makes for `interaction_source`.
    pub fn create(factory: &dyn IndicationNodeFactory, interaction_source: InteractionSource) -> IndicationNode {
        IndicationNode(factory.create(interaction_source))
    }

    /// A node made already (a design system's node a factory's node delegates to: material3's theme-aware
    /// ripple delegating to the ripple node).
    pub fn new(node: Box<dyn ModifierNode>) -> IndicationNode {
        IndicationNode(node)
    }
}

impl ModifierNode for IndicationNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        self.0.on_attach(cx)
    }
    fn on_detach(&mut self, cx: &NodeCx) {
        self.0.on_detach(cx)
    }
    fn on_reset(&mut self) {
        self.0.on_reset()
    }
    fn should_auto_invalidate(&self) -> bool {
        self.0.should_auto_invalidate()
    }
    fn as_layout(&mut self) -> Option<&mut dyn kompoz_ui::layout::LayoutModifierNode> {
        self.0.as_layout()
    }
    fn as_draw(&mut self) -> Option<&mut dyn DrawModifierNode> {
        self.0.as_draw()
    }
    fn as_semantics(&mut self) -> Option<&mut dyn kompoz_ui::semantics::SemanticsModifierNode> {
        self.0.as_semantics()
    }
    fn as_parent_data(&mut self) -> Option<&mut dyn kompoz_ui::layout::ParentDataModifierNode> {
        self.0.as_parent_data()
    }
    fn as_layout_aware(&mut self) -> Option<&mut dyn kompoz_ui::layout::LayoutAwareModifierNode> {
        self.0.as_layout_aware()
    }
    fn as_global_position_aware(&mut self) -> Option<&mut dyn kompoz_ui::layout::GlobalPositionAwareModifierNode> {
        self.0.as_global_position_aware()
    }
    fn as_unplaced_aware(&mut self) -> Option<&mut dyn kompoz_ui::layout::UnplacedAwareModifierNode> {
        self.0.as_unplaced_aware()
    }
    fn as_pointer_input(&mut self) -> Option<&mut dyn kompoz_ui::input::PointerInputModifierNode> {
        self.0.as_pointer_input()
    }
    fn as_indirect_pointer_input(&mut self) -> Option<&mut dyn kompoz_ui::input::IndirectPointerInputModifierNode> {
        self.0.as_indirect_pointer_input()
    }
    fn as_key_input(&mut self) -> Option<&mut dyn kompoz_ui::input::KeyInputModifierNode> {
        self.0.as_key_input()
    }
    fn as_soft_keyboard_interception(
        &mut self,
    ) -> Option<&mut dyn kompoz_ui::input::SoftKeyboardInterceptionModifierNode> {
        self.0.as_soft_keyboard_interception()
    }
    fn as_rotary_input(&mut self) -> Option<&mut dyn kompoz_ui::input::RotaryInputModifierNode> {
        self.0.as_rotary_input()
    }
    fn as_focus_target(&mut self) -> Option<&mut dyn kompoz_ui::focus::FocusTargetModifierNode> {
        self.0.as_focus_target()
    }
    fn as_focus_properties(&mut self) -> Option<&mut dyn kompoz_ui::focus::FocusPropertiesModifierNode> {
        self.0.as_focus_properties()
    }
    fn as_focus_event(&mut self) -> Option<&mut dyn kompoz_ui::focus::FocusEventModifierNode> {
        self.0.as_focus_event()
    }
    fn as_bring_into_view(&mut self) -> Option<&mut dyn kompoz_ui::focus::BringIntoViewModifierNode> {
        self.0.as_bring_into_view()
    }
    fn as_beyond_bounds_layout(&mut self) -> Option<&mut dyn kompoz_ui::focus::BeyondBoundsLayout> {
        self.0.as_beyond_bounds_layout()
    }
    fn as_traversable(&mut self) -> Option<&mut dyn TraversableNode> {
        self.0.as_traversable()
    }
    fn as_observer(&mut self) -> Option<&mut dyn kompoz_ui::modifier::ObserverModifierNode> {
        self.0.as_observer()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Modifier.indication

/// The indication modifier (Compose's `Modifier.indication`).
#[kompoz_runtime::named(trait IndicationModifiers)]
impl Modifier {
    /// Draws `indication` for `interaction_source`'s interactions over the layout (Compose's `Modifier.indication`):
    /// a component of one's own that emits them. None draws nothing; an indication that isn't a node factory
    /// draws nothing more than the content (Compose's `NoIndicationInstance`).
    fn indication(
        self,
        interaction_source: impl Into<InteractionSource>,
        indication: Option<Rc<dyn Indication>>,
    ) -> Modifier {
        let Some(indication) = indication else { return self };
        if indication.as_node_factory().is_none() {
            return self;
        }
        self.then_element(IndicationModifierElement { interaction_source: interaction_source.into(), indication })
    }
}

/// `Modifier.indication`'s element (Compose's IndicationModifierElement): equal for an equal source and factory.
struct IndicationModifierElement {
    interaction_source: InteractionSource,
    indication: Rc<dyn Indication>,
}

impl PartialEq for IndicationModifierElement {
    fn eq(&self, other: &Self) -> bool {
        self.interaction_source == other.interaction_source && *self.indication == *other.indication
    }
}

impl IndicationModifierElement {
    fn factory(&self) -> &dyn IndicationNodeFactory {
        self.indication.as_node_factory().expect("kompoz: an indication modifier of a factory")
    }
}

impl ModifierNodeElement for IndicationModifierElement {
    type Node = IndicationModifierNode;
    fn create(&self) -> IndicationModifierNode {
        IndicationModifierNode {
            made: Some(IndicationNode::create(self.factory(), self.interaction_source.clone())),
            delegated: None,
            handle: None,
        }
    }
    fn update(&self, node: &mut IndicationModifierNode) {
        node.update(IndicationNode::create(self.factory(), self.interaction_source.clone()));
    }
    fn name(&self) -> &'static str {
        "indication"
    }
}

/// The node of `Modifier.indication` (Compose's IndicationModifierNode): it delegates to the node its factory made,
/// replaced by a new one when the source or the factory changes.
pub struct IndicationModifierNode {
    /// The node made, until it is delegated to (at the first attach).
    made: Option<IndicationNode>,
    delegated: Option<DelegatedNode<IndicationNode>>,
    handle: Option<NodeHandle>,
}

impl IndicationModifierNode {
    fn update(&mut self, node: IndicationNode) {
        match self.handle.as_ref().and_then(NodeHandle::cx) {
            Some(cx) => {
                if let Some(old) = self.delegated.take() {
                    cx.undelegate(&old);
                }
                self.delegated = Some(cx.delegate(node));
            }
            None => self.made = Some(node),
        }
    }
}

impl ModifierNode for IndicationModifierNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        if let Some(node) = self.made.take() {
            if let Some(old) = self.delegated.take() {
                cx.undelegate(&old);
            }
            self.delegated = Some(cx.delegate(node));
        }
    }
}

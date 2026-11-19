//! Modifiers, as Compose's: a [`Modifier`] is an ordered list of elements; a layout node turns each into a
//! [`ModifierNode`] (made once, updated when its element changes) in its [`NodeChain`](chain), and the nodes
//! measure, draw, take input, focus and describe themselves as their capabilities say.

pub(crate) mod chain;
pub(crate) mod diff;
mod local;
mod node;

pub use local::{ModifierLocal, ModifierLocalMap, ModifierLocalModifierNode, modifier_local_of};
pub use node::{
    DelegatableNode, DelegatedNode, NodeCx, NodeHandle, NodeKind, ObserverModifierNode, TraversableNode,
    TraversalDecision,
};
pub(crate) use node::{NodeCell, NodeRc, dispatch_for_kind, traverse_descendants_of};

use std::any::{Any, TypeId};
use std::fmt;
use std::rc::Rc;

/// An ordered list of elements that decorate or add behaviour to a layout (Compose's Modifier). Start from the
/// empty one, the constant `Modifier`, and chain: `Modifier.padding(8.dp()).fill_max_width()`. Equal element by
/// element.
#[derive(Clone, Default)]
pub struct Modifier {
    elements: Vec<Rc<dyn AnyElement>>,
}

/// The empty modifier, to start a chain from.
pub const Modifier: Modifier = Modifier { elements: Vec::new() };

impl Modifier {
    /// This, then `other` (Compose's `then`): `other`'s elements come after (inside) these.
    pub fn then(mut self, other: Modifier) -> Modifier {
        if self.elements.is_empty() {
            return other;
        }
        self.elements.extend(other.elements);
        self
    }

    /// A modifier of one element (Compose's `this then element`: `self.then(Modifier::element(e))`).
    pub fn element(element: impl ModifierNodeElement) -> Modifier {
        Modifier { elements: vec![Rc::new(element)] }
    }

    /// This, then one element.
    pub fn then_element(mut self, element: impl ModifierNodeElement) -> Modifier {
        self.elements.push(Rc::new(element));
        self
    }

    /// Its elements, outermost first.
    pub fn elements(&self) -> &[Rc<dyn AnyElement>] {
        &self.elements
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Whether any element satisfies `predicate` (Compose's `any`).
    pub fn any(&self, predicate: impl Fn(&dyn AnyElement) -> bool) -> bool {
        self.elements.iter().any(|e| predicate(&**e))
    }

    /// Whether all do (Compose's `all`).
    pub fn all(&self, predicate: impl Fn(&dyn AnyElement) -> bool) -> bool {
        self.elements.iter().all(|e| predicate(&**e))
    }

    /// The elements folded outermost first (Compose's `foldIn`).
    pub fn fold_in<R>(&self, initial: R, operation: impl Fn(R, &dyn AnyElement) -> R) -> R {
        self.elements.iter().fold(initial, |acc, e| operation(acc, &**e))
    }

    /// The elements folded innermost first (Compose's `foldOut`).
    pub fn fold_out<R>(&self, initial: R, operation: impl Fn(&dyn AnyElement, R) -> R) -> R {
        self.elements.iter().rev().fold(initial, |acc, e| operation(&**e, acc))
    }
}

impl PartialEq for Modifier {
    fn eq(&self, other: &Self) -> bool {
        self.elements.len() == other.elements.len()
            && self.elements.iter().zip(&other.elements).all(|(a, b)| a.eq_dyn(&**b))
    }
}

impl fmt::Debug for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Modifier")?;
        for e in &self.elements {
            write!(f, ".{}", e.name())?;
        }
        Ok(())
    }
}

/// An element of a modifier that makes a node (Compose's ModifierNodeElement): `create` makes its node when it first
/// shows up in a layout's chain; afterwards, as long as an element of the same type is at its place, the node is
/// kept and `update` is called when the element is no longer equal to the last one (`==`). An element holding a
/// closure compares unequal unless it can tell it is the same one (`Rc::ptr_eq`), so its `update` hands the node
/// the new closure — what Compose's lambda-capturing elements amount to.
pub trait ModifierNodeElement: PartialEq + 'static {
    type Node: ModifierNode;
    fn create(&self) -> Self::Node;
    fn update(&self, node: &mut Self::Node);
    /// Its name in a modifier's Debug (Compose's inspector name): the type's by default.
    fn name(&self) -> &'static str {
        let full = std::any::type_name::<Self>();
        full.rsplit("::").next().unwrap_or(full)
    }
}

/// A [`ModifierNodeElement`] of any type, as a modifier holds it.
pub trait AnyElement: Any {
    /// Equal to `other` (of the same type and `==`).
    fn eq_dyn(&self, other: &dyn AnyElement) -> bool;
    /// Its node type's place in a chain is the same as `other`'s (Compose's `areObjectsOfSameType`).
    fn same_type(&self, other: &dyn AnyElement) -> bool {
        self.type_id() == other.type_id()
    }
    fn name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    #[doc(hidden)]
    fn create_node(&self) -> NodeRc;
    #[doc(hidden)]
    fn update_node(&self, node: &NodeRc);
}

impl<E: ModifierNodeElement> AnyElement for E {
    fn eq_dyn(&self, other: &dyn AnyElement) -> bool {
        other.as_any().downcast_ref::<E>().is_some_and(|o| self == o)
    }
    fn name(&self) -> &'static str {
        ModifierNodeElement::name(self)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn create_node(&self) -> NodeRc {
        NodeCell::new_rc(self.create())
    }
    fn update_node(&self, node: &NodeRc) {
        {
            let mut n = node.node.borrow_mut();
            let n: &mut dyn Any = &mut *n;
            let n = n.downcast_mut::<E::Node>().expect("kompoz: a node of another type at an element's place");
            self.update(n);
        }
        node.refresh_provided_locals();
    }
}

impl dyn AnyElement {
    /// The element as a `T`, if it is one.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
    /// Whether it is a `T`.
    pub fn is<T: Any>(&self) -> bool {
        self.as_any().type_id() == TypeId::of::<T>()
    }
}

/// A node of a modifier, made by its element and kept in a layout node's chain (Compose's Modifier.Node). What it
/// does it says through its capabilities: each `as_*` a node has returns `Some(self)` (the [`node_capabilities!`]
/// macro writes them). Its kinds ([`NodeKind`]) are worked out from them once, when it is made; traversals look
/// for a kind (and skip whole chains without it).
///
/// `on_attach` runs when it comes into an attached layout (or its layout is attached), `on_detach` when it leaves,
/// `on_reset` when its layout is reused for other content. A node keeps what `cx.handle()` gives it to act later
/// (from a coroutine, an animation): invalidate itself, read locals, reach its coordinates.
pub trait ModifierNode: Any {
    fn on_attach(&mut self, cx: &NodeCx) {
        let _ = cx;
    }
    fn on_detach(&mut self, cx: &NodeCx) {
        let _ = cx;
    }
    fn on_reset(&mut self) {}

    /// Its layout's layout direction changed (Compose's `onLayoutDirectionChange`): what it measures, places and
    /// draws is invalidated already; this is for what else depends on it (a scrollable's direction). Called on
    /// the nodes of a chain, not on their delegates, as Compose's `headToTail`.
    fn on_layout_direction_change(&mut self, cx: &NodeCx) {
        let _ = cx;
    }

    /// Whether an update of its element invalidates what its capabilities depend on (measurement for a layout
    /// node, drawing for a draw node, …), as Compose's `shouldAutoInvalidate`; a node that invalidates itself as
    /// needed says false.
    fn should_auto_invalidate(&self) -> bool {
        true
    }

    fn as_layout(&mut self) -> Option<&mut dyn crate::layout::LayoutModifierNode> {
        None
    }
    /// An approach layout node's (Compose's `Nodes.ApproachMeasure`): its node is a layout node too.
    fn as_approach_layout(&mut self) -> Option<&mut dyn crate::layout::ApproachLayoutModifierNode> {
        None
    }
    fn as_draw(&mut self) -> Option<&mut dyn crate::draw::DrawModifierNode> {
        None
    }
    fn as_semantics(&mut self) -> Option<&mut dyn crate::semantics::SemanticsModifierNode> {
        None
    }
    fn as_parent_data(&mut self) -> Option<&mut dyn crate::layout::ParentDataModifierNode> {
        None
    }
    fn as_layout_aware(&mut self) -> Option<&mut dyn crate::layout::LayoutAwareModifierNode> {
        None
    }
    fn as_global_position_aware(&mut self) -> Option<&mut dyn crate::layout::GlobalPositionAwareModifierNode> {
        None
    }
    fn as_unplaced_aware(&mut self) -> Option<&mut dyn crate::layout::UnplacedAwareModifierNode> {
        None
    }
    fn as_pointer_input(&mut self) -> Option<&mut dyn crate::input::PointerInputModifierNode> {
        None
    }
    fn as_indirect_pointer_input(&mut self) -> Option<&mut dyn crate::input::IndirectPointerInputModifierNode> {
        None
    }
    fn as_key_input(&mut self) -> Option<&mut dyn crate::input::KeyInputModifierNode> {
        None
    }
    fn as_soft_keyboard_interception(&mut self) -> Option<&mut dyn crate::input::SoftKeyboardInterceptionModifierNode> {
        None
    }
    fn as_rotary_input(&mut self) -> Option<&mut dyn crate::input::RotaryInputModifierNode> {
        None
    }
    fn as_focus_target(&mut self) -> Option<&mut dyn crate::focus::FocusTargetModifierNode> {
        None
    }
    fn as_focus_properties(&mut self) -> Option<&mut dyn crate::focus::FocusPropertiesModifierNode> {
        None
    }
    fn as_focus_event(&mut self) -> Option<&mut dyn crate::focus::FocusEventModifierNode> {
        None
    }
    fn as_bring_into_view(&mut self) -> Option<&mut dyn crate::focus::BringIntoViewModifierNode> {
        None
    }
    fn as_beyond_bounds_layout(&mut self) -> Option<&mut dyn crate::focus::BeyondBoundsLayout> {
        None
    }
    fn as_traversable(&mut self) -> Option<&mut dyn TraversableNode> {
        None
    }
    fn as_modifier_local(&mut self) -> Option<&mut dyn ModifierLocalModifierNode> {
        None
    }
    fn as_observer(&mut self) -> Option<&mut dyn ObserverModifierNode> {
        None
    }
}

/// Writes a [`ModifierNode`]'s capability accessors: `node_capabilities!(layout, draw)` in its impl gives
/// `as_layout` and `as_draw` returning `Some(self)` (the node implements those traits).
///
/// Names: `layout`, `approach_layout` (with `layout`), `draw`, `semantics`, `parent_data`, `layout_aware`, `global_position_aware`,
/// `unplaced_aware`, `pointer_input`, `indirect_pointer_input`, `key_input`, `soft_keyboard_interception`,
/// `rotary_input`, `focus_target`, `focus_properties`, `focus_event`, `bring_into_view`, `beyond_bounds_layout`,
/// `traversable`, `modifier_local`, `observer`.
#[macro_export]
macro_rules! node_capabilities {
    ($($cap:ident),* $(,)?) => {
        $( $crate::node_capabilities!(@one $cap); )*
    };
    (@one layout) => { fn as_layout(&mut self) -> Option<&mut dyn $crate::layout::LayoutModifierNode> { Some(self) } };
    (@one approach_layout) => {
        fn as_approach_layout(&mut self) -> Option<&mut dyn $crate::layout::ApproachLayoutModifierNode> { Some(self) }
    };
    (@one draw) => { fn as_draw(&mut self) -> Option<&mut dyn $crate::draw::DrawModifierNode> { Some(self) } };
    (@one semantics) => {
        fn as_semantics(&mut self) -> Option<&mut dyn $crate::semantics::SemanticsModifierNode> { Some(self) }
    };
    (@one parent_data) => {
        fn as_parent_data(&mut self) -> Option<&mut dyn $crate::layout::ParentDataModifierNode> { Some(self) }
    };
    (@one layout_aware) => {
        fn as_layout_aware(&mut self) -> Option<&mut dyn $crate::layout::LayoutAwareModifierNode> { Some(self) }
    };
    (@one global_position_aware) => {
        fn as_global_position_aware(&mut self) -> Option<&mut dyn $crate::layout::GlobalPositionAwareModifierNode> {
            Some(self)
        }
    };
    (@one unplaced_aware) => {
        fn as_unplaced_aware(&mut self) -> Option<&mut dyn $crate::layout::UnplacedAwareModifierNode> { Some(self) }
    };
    (@one pointer_input) => {
        fn as_pointer_input(&mut self) -> Option<&mut dyn $crate::input::PointerInputModifierNode> { Some(self) }
    };
    (@one indirect_pointer_input) => {
        fn as_indirect_pointer_input(&mut self) -> Option<&mut dyn $crate::input::IndirectPointerInputModifierNode> {
            Some(self)
        }
    };
    (@one key_input) => {
        fn as_key_input(&mut self) -> Option<&mut dyn $crate::input::KeyInputModifierNode> { Some(self) }
    };
    (@one soft_keyboard_interception) => {
        fn as_soft_keyboard_interception(
            &mut self,
        ) -> Option<&mut dyn $crate::input::SoftKeyboardInterceptionModifierNode> {
            Some(self)
        }
    };
    (@one rotary_input) => {
        fn as_rotary_input(&mut self) -> Option<&mut dyn $crate::input::RotaryInputModifierNode> { Some(self) }
    };
    (@one focus_target) => {
        fn as_focus_target(&mut self) -> Option<&mut dyn $crate::focus::FocusTargetModifierNode> { Some(self) }
    };
    (@one focus_properties) => {
        fn as_focus_properties(&mut self) -> Option<&mut dyn $crate::focus::FocusPropertiesModifierNode> { Some(self) }
    };
    (@one focus_event) => {
        fn as_focus_event(&mut self) -> Option<&mut dyn $crate::focus::FocusEventModifierNode> { Some(self) }
    };
    (@one bring_into_view) => {
        fn as_bring_into_view(&mut self) -> Option<&mut dyn $crate::focus::BringIntoViewModifierNode> { Some(self) }
    };
    (@one beyond_bounds_layout) => {
        fn as_beyond_bounds_layout(&mut self) -> Option<&mut dyn $crate::focus::BeyondBoundsLayout> { Some(self) }
    };
    (@one modifier_local) => {
        fn as_modifier_local(&mut self) -> Option<&mut dyn $crate::modifier::ModifierLocalModifierNode> { Some(self) }
    };
    (@one traversable) => {
        fn as_traversable(&mut self) -> Option<&mut dyn $crate::modifier::TraversableNode> { Some(self) }
    };
    (@one observer) => {
        fn as_observer(&mut self) -> Option<&mut dyn $crate::modifier::ObserverModifierNode> { Some(self) }
    };
}

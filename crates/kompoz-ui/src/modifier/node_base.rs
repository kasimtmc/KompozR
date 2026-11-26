//! Kotlin's `Modifier.Node` as a base class (stage 14): what a Kotlin class `class N : Modifier.Node(),
//! DrawModifierNode { … }` is in the Kotlin syntax — a type of one's own holding a [`ModifierNodeImpl`] (the base
//! class's part: the node's place, kept from its attaching), implementing [`ModifierNodeBase`] (its overridable
//! `onAttach`/`onDetach`/`onReset`, its members `coroutineScope`, `invalidateDraw()`, `currentValueOf(…)`, …) and the
//! capability interfaces it lists ([`DrawModifierNode`](crate::draw::DrawModifierNode), …); its [`ModifierNode`]
//! (what a chain holds) calls them ([`attach_node_base`], [`detach_node_base`]). Rust's own nodes implement
//! [`ModifierNode`] directly, given a [`NodeCx`] in each callback.

use super::{ModifierNode, NodeCx, NodeHandle};
use kompoz_coroutines::CoroutineScope;
use kompoz_graphics::{Density, LayoutDirection};
use kompoz_runtime::KompozitionLocal;
use kompoz_runtime::kt::panic_kt;
use std::any::Any;
use std::cell::RefCell;

/// A node of a modifier as Kotlin's class extending `Modifier.Node` is (Compose's abstract class `Modifier.Node`):
/// it holds a [`ModifierNodeImpl`] (what its base class holds), overrides what it does as it is attached, detached
/// and reset, and reaches its place through its members — its coroutine scope, its invalidations, the composition
/// locals where its layout is. The [`ModifierNode`] a chain holds of it attaches it ([`attach_node_base`]: its
/// place kept, then [`on_attach`](Self::on_attach)) and detaches it ([`detach_node_base`]).
///
/// Its members Compose has as extensions of `DelegatableNode` (`requireDensity()`, …) are its own here; those of the
/// node's interfaces are theirs, for a node of this class: `DrawModifierNode.invalidateDraw()`
/// ([`DrawModifierNode::invalidate_draw`](crate::draw::DrawModifierNode::invalidate_draw)),
/// `CompositionLocalConsumerModifierNode.currentValueOf(…)` ([`CompositionLocalConsumerModifierNode::current_value_of`]).
///
/// Kotlin: `abstract class Modifier.Node`
pub trait ModifierNodeBase: Any {
    /// Its base class's part (Kotlin's `Modifier.Node`'s own state): where it is while attached.
    fn modifier_node_impl(&self) -> &ModifierNodeImpl;

    /// Called once it is attached: its place known, its coroutine scope usable (Compose's `onAttach()`).
    fn on_attach(&self) {}

    /// Called as it is detached, before its coroutine scope is cancelled (Compose's `onDetach()`).
    fn on_detach(&self) {}

    /// Called when its layout is reused for other content (Compose's `onReset()`).
    fn on_reset(&self) {}

    /// Whether an update of its element invalidates what it draws, measures, … (Compose's open
    /// `shouldAutoInvalidate`).
    fn should_auto_invalidate(&self) -> bool {
        true
    }

    /// Its coroutine scope while attached, cancelled as it is detached (Compose's `coroutineScope`). Panics when it
    /// isn't attached (Kotlin's IllegalStateException).
    fn coroutine_scope(&self) -> CoroutineScope {
        self.modifier_node_impl().coroutine_scope()
    }

    /// Whether it is attached (Compose's `isAttached`).
    fn is_attached(&self) -> bool {
        self.modifier_node_impl().is_attached()
    }

    /// Its place, while it is attached (Compose's `node`, the delegatable node's: here its handle).
    fn node_handle(&self) -> Option<NodeHandle> {
        self.modifier_node_impl().handle()
    }

    /// Its layout's density (Compose's `DelegatableNode.requireDensity()`). Panics when it isn't attached.
    fn require_density(&self) -> Density {
        self.modifier_node_impl().require_handle().require_density()
    }

    /// Its layout's layout direction (Compose's `DelegatableNode.requireLayoutDirection()`). Panics when it isn't
    /// attached.
    fn require_layout_direction(&self) -> LayoutDirection {
        self.modifier_node_impl().require_handle().require_layout_direction()
    }
}

/// What Kotlin's `Modifier.Node` holds of a node of its class (Compose's base class's own fields): the node's place,
/// kept from its attaching until its detaching.
#[derive(Default)]
pub struct ModifierNodeImpl {
    handle: RefCell<Option<NodeHandle>>,
}

impl ModifierNodeImpl {
    /// A node's base part, not attached yet (Kotlin's `Modifier.Node()` constructor).
    pub fn new() -> ModifierNodeImpl {
        ModifierNodeImpl::default()
    }

    /// Its node's place, while attached.
    pub fn handle(&self) -> Option<NodeHandle> {
        self.handle.borrow().clone().filter(NodeHandle::is_attached)
    }

    /// Whether its node is attached.
    pub fn is_attached(&self) -> bool {
        self.handle().is_some()
    }

    /// Its node's coroutine scope (see [`ModifierNodeBase::coroutine_scope`]).
    #[track_caller]
    pub fn coroutine_scope(&self) -> CoroutineScope {
        self.require_handle().coroutine_scope().unwrap_or_else(|| {
            panic_kt("IllegalStateException", "Cannot obtain node coroutineScope. Is the node attached?")
        })
    }

    #[track_caller]
    fn require_handle(&self) -> NodeHandle {
        self.handle().unwrap_or_else(|| {
            panic_kt("IllegalStateException", "Cannot obtain node coordinator. Is the Modifier.Node attached?")
        })
    }
}

/// Attaches a node of Kotlin's `Modifier.Node` class: its place kept, then its [`on_attach`]
/// (ModifierNodeBase::on_attach). What its [`ModifierNode::on_attach`] does.
pub fn attach_node_base<N: ModifierNodeBase + ModifierNode>(node: &N, cx: &NodeCx) {
    *node.modifier_node_impl().handle.borrow_mut() = Some(cx.handle());
    ModifierNodeBase::on_attach(node);
}

/// Detaches a node of Kotlin's `Modifier.Node` class: its [`on_detach`](ModifierNodeBase::on_detach) (its place
/// still known), then its place forgotten. What its [`ModifierNode::on_detach`] does.
pub fn detach_node_base<N: ModifierNodeBase + ModifierNode>(node: &N, cx: &NodeCx) {
    let _ = cx;
    ModifierNodeBase::on_detach(node);
    *node.modifier_node_impl().handle.borrow_mut() = None;
}

/// A node reading the composition locals where its layout is (Compose's interface
/// `CompositionLocalConsumerModifierNode`): a class of Kotlin's `Modifier.Node` lists it to read them; a node of
/// Rust's reads them through its [`NodeCx`].
pub trait CompositionLocalConsumerModifierNode {
    /// `local`'s value where its layout is (Compose's `currentValueOf`): its default when it isn't attached.
    fn current_value_of<T: Clone + 'static>(&self, local: &'static KompozitionLocal<T>) -> T
    where
        Self: Sized + ModifierNodeBase,
    {
        match self.modifier_node_impl().handle() {
            Some(h) => h.current_value_of(local),
            None => local.default_value(),
        }
    }
}

//! Overscroll, as Compose's Overscroll.kt: an [`OverscrollEffect`] is given what a scrollable's scrolls and
//! flings leave at its ends and shows it (Android's stretch: [`crate::overscroll_android`]); `Modifier.overscroll`
//! draws an effect (a scrollable with an effect of its own only sends it events); [`LocalOverscrollFactory`]
//! makes the effects scroll containers use by default.

use crate::gestures::LocalFuture;
use kompoz_graphics::Offset;
use kompoz_runtime::{KompozitionLocal, KompozitionLocalAccessorScope, kompozable, named, remember_keyed};
use kompoz_ui::input::NestedScrollSource;
use kompoz_ui::modifier::DelegatableNode;
use kompoz_ui::unit::Velocity;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle};
use std::any::Any;
use std::fmt;
use std::rc::Rc;

/// What an overscroll effect calls to fling with what it leaves (Compose's `performFling: suspend (Velocity) ->
/// Velocity`): the velocity the fling used.
pub type PerformFling = Rc<dyn Fn(Velocity) -> LocalFuture<'static, Velocity>>;

/// What a scrollable's scrolls and flings leave at its ends goes to its overscroll effect, which shows it (Compose's
/// OverscrollEffect): `apply_to_scroll` takes a delta and decides what of it goes on to scrolling
/// (`perform_scroll`, which gives back what scrolled) and what it takes; `apply_to_fling` the same for a fling's
/// velocity, in a coroutine; `node` is what draws it (where `Modifier.overscroll` or a scroll container puts it).
/// An effect is shared (`Rc<dyn OverscrollEffect>`); two are equal when their types say so (by default: the same
/// one, as Compose's `equals`).
pub trait OverscrollEffect: Any + OverscrollEffectEq {
    /// Takes `delta` px (from `source`), scrolling what it doesn't keep with `perform_scroll`: what was used of it
    /// (Compose's `applyToScroll`).
    fn apply_to_scroll(
        &self,
        delta: Offset,
        source: NestedScrollSource,
        perform_scroll: &dyn Fn(Offset) -> Offset,
    ) -> Offset;

    /// Takes a fling of `velocity` px/s, flinging what it doesn't keep with `perform_fling` (Compose's
    /// `applyToFling`).
    fn apply_to_fling(&self, velocity: Velocity, perform_fling: PerformFling) -> LocalFuture<'_, ()>;

    /// Whether it shows something now (Compose's `isInProgress`): a scrollable lets it take its scrolls even at
    /// its ends while it does.
    fn is_in_progress(&self) -> bool;

    /// The node that shows it (Compose's `node`): delegated to where it is drawn, sized there. By default a node
    /// that does nothing (a new one each time, as Compose's).
    fn node(&self) -> DelegatableNode {
        DelegatableNode::new(NothingNode)
    }
}

/// An overscroll effect's default node: nothing.
struct NothingNode;
impl ModifierNode for NothingNode {}

/// How overscroll effects compare (a blanket impl for every `PartialEq` one).
#[doc(hidden)]
pub trait OverscrollEffectEq {
    fn overscroll_effect_eq(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: PartialEq + Any> OverscrollEffectEq for T {
    fn overscroll_effect_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PartialEq for dyn OverscrollEffect {
    fn eq(&self, other: &dyn OverscrollEffect) -> bool {
        self.overscroll_effect_eq(other.as_any())
    }
}

impl fmt::Debug for dyn OverscrollEffect {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "OverscrollEffect")
    }
}

/// Whether two optional effects are the same (Compose's `==` on `OverscrollEffect?`).
pub(crate) fn same_effect(a: &Option<Rc<dyn OverscrollEffect>>, b: &Option<Rc<dyn OverscrollEffect>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => **a == **b,
        (None, None) => true,
        _ => false,
    }
}

/// The effect without what it shows: its events only, its node one of nothing (Compose's
/// `withoutVisualEffect`), to draw it elsewhere with [`without_event_handling`] of it.
pub fn without_visual_effect(effect: Rc<dyn OverscrollEffect>) -> Rc<dyn OverscrollEffect> {
    Rc::new(WrappedOverscrollEffect::new(false, true, effect))
}

/// The effect without its events: it only shows what another wrapper of it was given (Compose's
/// `withoutEventHandling`).
pub fn without_event_handling(effect: Rc<dyn OverscrollEffect>) -> Rc<dyn OverscrollEffect> {
    Rc::new(WrappedOverscrollEffect::new(true, false, effect))
}

/// An effect with its events or its node taken off (Compose's WrappedOverscrollEffect): equal to another of the
/// same effect taken off the same way.
struct WrappedOverscrollEffect {
    attach_node: bool,
    event_handling_enabled: bool,
    inner_overscroll_effect: Rc<dyn OverscrollEffect>,
    node: DelegatableNode,
}

impl WrappedOverscrollEffect {
    fn new(attach_node: bool, event_handling_enabled: bool, inner: Rc<dyn OverscrollEffect>) -> Self {
        let node = if attach_node { inner.node() } else { DelegatableNode::new(NothingNode) };
        WrappedOverscrollEffect { attach_node, event_handling_enabled, inner_overscroll_effect: inner, node }
    }
}

impl PartialEq for WrappedOverscrollEffect {
    fn eq(&self, other: &Self) -> bool {
        self.attach_node == other.attach_node
            && self.event_handling_enabled == other.event_handling_enabled
            && *self.inner_overscroll_effect == *other.inner_overscroll_effect
    }
}

impl OverscrollEffect for WrappedOverscrollEffect {
    fn apply_to_scroll(
        &self,
        delta: Offset,
        source: NestedScrollSource,
        perform_scroll: &dyn Fn(Offset) -> Offset,
    ) -> Offset {
        if self.event_handling_enabled {
            self.inner_overscroll_effect.apply_to_scroll(delta, source, perform_scroll)
        } else {
            perform_scroll(delta)
        }
    }

    fn apply_to_fling(&self, velocity: Velocity, perform_fling: PerformFling) -> LocalFuture<'_, ()> {
        if self.event_handling_enabled {
            self.inner_overscroll_effect.apply_to_fling(velocity, perform_fling)
        } else {
            Box::pin(async move {
                perform_fling(velocity).await;
            })
        }
    }

    fn is_in_progress(&self) -> bool {
        self.inner_overscroll_effect.is_in_progress()
    }

    fn node(&self) -> DelegatableNode {
        self.node.clone()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Modifier.overscroll

/// The overscroll modifier.
#[named(trait OverscrollModifiers)]
impl Modifier {
    /// Draws `overscroll_effect` here (its node delegated to: Compose's `Modifier.overscroll`): where a
    /// scrollable given an effect of its own ([`scrollable`](crate::gestures::ScrollableModifiers::scrollable)
    /// with `overscroll_effect`) shows it. Nothing for none.
    fn overscroll(self, overscroll_effect: Option<Rc<dyn OverscrollEffect>>) -> Modifier {
        self.then_element(OverscrollModifierElement { overscroll_effect })
    }
}

struct OverscrollModifierElement {
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
}

impl PartialEq for OverscrollModifierElement {
    fn eq(&self, other: &Self) -> bool {
        same_effect(&self.overscroll_effect, &other.overscroll_effect)
    }
}

impl ModifierNodeElement for OverscrollModifierElement {
    type Node = OverscrollModifierNode;
    fn create(&self) -> OverscrollModifierNode {
        OverscrollModifierNode { overscroll_node: self.overscroll_effect.as_ref().map(|e| e.node()), handle: None }
    }
    fn update(&self, node: &mut OverscrollModifierNode) {
        node.update(self.overscroll_effect.as_ref().map(|e| e.node()));
    }
    fn name(&self) -> &'static str {
        "overscroll"
    }
}

/// `Modifier.overscroll`'s node (Compose's OverscrollModifierNode): it delegates to its effect's node while that
/// isn't attached elsewhere.
pub struct OverscrollModifierNode {
    overscroll_node: Option<DelegatableNode>,
    handle: Option<NodeHandle>,
}

impl OverscrollModifierNode {
    fn update(&mut self, overscroll_node: Option<DelegatableNode>) {
        let cx = self.handle.as_ref().and_then(|h| h.cx()).filter(|c| c.is_attached());
        if let (Some(old), Some(cx)) = (self.overscroll_node.take(), cx.as_ref()) {
            undelegate_if_delegated(cx, &old);
        }
        self.overscroll_node = overscroll_node;
        if let Some(cx) = cx {
            self.attach_if_needed(&cx);
        }
    }

    /// Compose's `attachIfNeeded`: its effect's node delegated to if it isn't attached; else none kept.
    fn attach_if_needed(&mut self, cx: &NodeCx) {
        self.overscroll_node = match self.overscroll_node.take() {
            Some(node) if !node.is_attached() => {
                cx.delegate_node(&node);
                Some(node)
            }
            _ => None,
        };
    }
}

/// Takes back `node` if `cx`'s node delegates to it (Compose's `undelegate` of the node it kept).
pub(crate) fn undelegate_if_delegated(cx: &NodeCx, node: &DelegatableNode) {
    if node.is_delegate_of(cx) {
        cx.undelegate_node(node);
    }
}

impl ModifierNode for OverscrollModifierNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.attach_if_needed(cx);
    }

    fn on_detach(&mut self, cx: &NodeCx) {
        if let Some(node) = &self.overscroll_node {
            undelegate_if_delegated(cx, node);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Factories

/// What makes the overscroll effects of scroll containers (Compose's OverscrollFactory): provided as
/// [`LocalOverscrollFactory`]. Two factories are equal when they make the same effects (a scroll container makes
/// its effect again when its factory changes).
pub trait OverscrollFactory: Any + OverscrollFactoryEq {
    /// A new effect (Compose's `createOverscrollEffect`).
    fn create_overscroll_effect(&self) -> Rc<dyn OverscrollEffect>;
}

/// How overscroll factories compare (a blanket impl for every `PartialEq` one).
#[doc(hidden)]
pub trait OverscrollFactoryEq {
    fn overscroll_factory_eq(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: PartialEq + Any> OverscrollFactoryEq for T {
    fn overscroll_factory_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PartialEq for dyn OverscrollFactory {
    fn eq(&self, other: &dyn OverscrollFactory) -> bool {
        self.overscroll_factory_eq(OverscrollFactoryEq::as_any(other))
    }
}

impl fmt::Debug for dyn OverscrollFactory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "OverscrollFactory")
    }
}

/// The factory the scroll containers below make their overscroll effects with (Compose's
/// LocalOverscrollFactory); none: no overscroll. By default Android's stretch, for the density where it is read
/// (Compose's Android default, worked out from LocalDensity and the deprecated LocalOverscrollConfiguration; a
/// configuration of none turns it off).
pub static LocalOverscrollFactory: KompozitionLocal<Option<Rc<dyn OverscrollFactory>>> =
    KompozitionLocal::new_computed(default_overscroll_factory);

fn default_overscroll_factory(scope: &KompozitionLocalAccessorScope) -> Option<Rc<dyn OverscrollFactory>> {
    crate::overscroll_android::default_overscroll_factory(scope)
}

/// An effect of [`LocalOverscrollFactory`]'s factory, remembered while the factory stays equal (Compose's
/// `rememberOverscrollEffect`); none where no factory is provided.
#[kompozable]
pub fn remember_overscroll_effect() -> Option<Rc<dyn OverscrollEffect>> {
    let overscroll_factory = LocalOverscrollFactory.current()?;
    let key = FactoryKey(overscroll_factory.clone());
    Some(remember_keyed(key, move || overscroll_factory.create_overscroll_effect()))
}

/// A factory compared as the factories are.
#[derive(Clone)]
struct FactoryKey(Rc<dyn OverscrollFactory>);

impl PartialEq for FactoryKey {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

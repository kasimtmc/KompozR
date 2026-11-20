//! A parent's semantics a child takes, as Compose's internal ChildParentSemantics.kt: a component whose semantics
//! (`parent_semantics`, merging what is below it) go onto a clickable child within it (`child_semantics`) while
//! there is one, and stay its own when there isn't (a tooltip's anchor, a surface).

use kompoz_ui::modifier::TraversableNode;
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode};
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::any::Any;
use std::rc::Rc;

type Properties = Rc<dyn Fn(&mut SemanticsConfiguration)>;

/// The modifiers of a parent's semantics a child takes.
pub(crate) trait ChildParentSemanticsModifiers {
    /// The layout takes the semantics of the nearest [`parent_semantics`](Self::parent_semantics) above it, and sets
    /// `properties` (Compose's internal `Modifier.childSemantics`).
    fn child_semantics_with(self, properties: impl Fn(&mut SemanticsConfiguration) + 'static) -> Modifier;
    /// [`child_semantics_with`](Self::child_semantics_with) of no properties of its own.
    fn child_semantics(self) -> Modifier;
    /// `properties`, merging what is below, unless a [`child_semantics`](Self::child_semantics) below takes them
    /// (Compose's internal `Modifier.parentSemantics`).
    fn parent_semantics(self, properties: impl Fn(&mut SemanticsConfiguration) + 'static) -> Modifier;
}

impl ChildParentSemanticsModifiers for Modifier {
    fn child_semantics_with(self, properties: impl Fn(&mut SemanticsConfiguration) + 'static) -> Modifier {
        self.then_element(ChildSemanticsNodeElement { properties: Rc::new(properties) })
    }

    fn child_semantics(self) -> Modifier {
        thread_local! {
            // (Kotlin's default `{}`, the same lambda each time: the element is equal to the last)
            static NONE: Properties = Rc::new(|_| {});
        }
        let properties = NONE.with(Rc::clone);
        self.then_element(ChildSemanticsNodeElement { properties })
    }

    fn parent_semantics(self, properties: impl Fn(&mut SemanticsConfiguration) + 'static) -> Modifier {
        self.then_element(ParentSemanticsNodeElement { properties: Rc::new(properties) })
    }
}

/// Compose's ChildSemanticsNodeElement: equal for the same properties (by instance).
struct ChildSemanticsNodeElement {
    properties: Properties,
}

impl PartialEq for ChildSemanticsNodeElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.properties, &other.properties)
    }
}

impl ModifierNodeElement for ChildSemanticsNodeElement {
    type Node = ChildSemanticsNode;
    fn create(&self) -> ChildSemanticsNode {
        ChildSemanticsNode { properties: self.properties.clone(), handle: None }
    }
    fn update(&self, node: &mut ChildSemanticsNode) {
        node.properties = self.properties.clone();
        if let Some(h) = &node.handle {
            h.invalidate_semantics();
        }
    }
    fn name(&self) -> &'static str {
        "childSemantics"
    }
}

/// Compose's ParentSemanticsNodeElement.
struct ParentSemanticsNodeElement {
    properties: Properties,
}

impl PartialEq for ParentSemanticsNodeElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.properties, &other.properties)
    }
}

impl ModifierNodeElement for ParentSemanticsNodeElement {
    type Node = ParentSemanticsNode;
    fn create(&self) -> ParentSemanticsNode {
        ParentSemanticsNode { properties: self.properties.clone(), semantics_consumed: false, handle: None }
    }
    fn update(&self, node: &mut ParentSemanticsNode) {
        node.properties = self.properties.clone();
        if let Some(h) = &node.handle {
            h.invalidate_semantics();
        }
    }
    fn name(&self) -> &'static str {
        "parentSemantics"
    }
}

/// Compose's internal ParentSemanticsNodeKey.
struct ParentSemanticsNodeKey;

/// Compose's ChildSemanticsNode: it takes the nearest parent's semantics as its own.
struct ChildSemanticsNode {
    properties: Properties,
    handle: Option<NodeHandle>,
}

impl ModifierNode for ChildSemanticsNode {
    node_capabilities!(semantics);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }

    fn on_detach(&mut self, cx: &NodeCx) {
        cx.traverse_ancestors(&ParentSemanticsNodeKey, &mut |node| {
            node.with::<ParentSemanticsNode, _>(|parent| parent.release_semantics());
            false
        });
        self.handle = None;
    }
}

impl SemanticsModifierNode for ChildSemanticsNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        if let Some(cx) = self.handle.as_ref().and_then(NodeHandle::cx) {
            cx.traverse_ancestors(&ParentSemanticsNodeKey, &mut |node| {
                node.with::<ParentSemanticsNode, _>(|parent| parent.obtain_semantics(config));
                false
            });
        }
        (self.properties)(config);
    }
}

/// Compose's ParentSemanticsNode: its semantics its own until a child takes them.
struct ParentSemanticsNode {
    properties: Properties,
    semantics_consumed: bool,
    handle: Option<NodeHandle>,
}

impl ParentSemanticsNode {
    /// Its properties onto a child's semantics (Compose's `obtainSemantics`): no longer its own.
    fn obtain_semantics(&mut self, config: &mut SemanticsConfiguration) {
        self.semantics_consumed = true;
        (self.properties)(config);
        if let Some(h) = &self.handle {
            h.invalidate_semantics();
        }
    }

    /// Its own again (Compose's `releaseSemantics`).
    fn release_semantics(&mut self) {
        self.semantics_consumed = false;
        if let Some(h) = &self.handle {
            h.invalidate_semantics();
        }
    }
}

impl ModifierNode for ParentSemanticsNode {
    node_capabilities!(semantics, traversable);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.handle = None;
    }
}

impl TraversableNode for ParentSemanticsNode {
    fn traverse_key(&self) -> &dyn Any {
        &ParentSemanticsNodeKey
    }
}

impl SemanticsModifierNode for ParentSemanticsNode {
    fn should_merge_descendant_semantics(&self) -> bool {
        true
    }

    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        if !self.semantics_consumed {
            (self.properties)(config);
        }
    }
}

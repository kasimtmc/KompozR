//! A semantics modifier whose properties read the node's composition locals and launch coroutines, as Compose's
//! internal DelegableModifier.kt (adaptive-layout 1.2.0): [`delegable_semantics`] and its receiver
//! ([`DelegableSemanticsPropertyReceiver`]).

use kompoz_coroutines::CoroutineScope;
use kompoz_runtime::KompozitionLocal;
use kompoz_ui::Modifier;
use kompoz_ui::modifier::{ModifierNode, ModifierNodeElement, NodeCx, NodeHandle};
use kompoz_ui::node_capabilities;
use kompoz_ui::semantics::{SemanticsConfiguration, SemanticsModifierNode};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

/// What a [`delegable_semantics`]' properties are set on (Compose's internal interface
/// `DelegableSemanticsPropertyReceiver`: a `SemanticsPropertyReceiver` — the configuration, through `Deref` — and a
/// `CompositionLocalConsumerModifierNode` with the node's coroutine scope).
pub(crate) struct DelegableSemanticsPropertyReceiver<'a> {
    config: &'a mut SemanticsConfiguration,
    node: &'a NodeHandle,
}

impl DelegableSemanticsPropertyReceiver<'_> {
    /// The node's coroutine scope (Compose's `coroutineScope`: the node's, cancelled when it is detached).
    pub(crate) fn coroutine_scope(&self) -> CoroutineScope {
        self.node.coroutine_scope().expect("the node's coroutine scope: it is attached while its semantics are read")
    }

    /// `local`'s value where the node's layout was composed (Compose's `currentValueOf`).
    #[allow(dead_code)]
    pub(crate) fn current_value_of<T: Clone + 'static>(&self, local: &'static KompozitionLocal<T>) -> T {
        self.node.current_value_of(local)
    }

    /// The node (what reads the texts through it: `get_string_of_node`).
    pub(crate) fn node(&self) -> &NodeHandle {
        self.node
    }
}

impl Deref for DelegableSemanticsPropertyReceiver<'_> {
    type Target = SemanticsConfiguration;

    fn deref(&self) -> &SemanticsConfiguration {
        self.config
    }
}

impl DerefMut for DelegableSemanticsPropertyReceiver<'_> {
    fn deref_mut(&mut self) -> &mut SemanticsConfiguration {
        self.config
    }
}

/// The properties a [`delegable_semantics`] sets (Kotlin's `DelegableSemanticsPropertyReceiver.() -> Unit`).
pub(crate) type DelegableSemanticsProperties = Rc<dyn Fn(&mut DelegableSemanticsPropertyReceiver)>;

/// Semantics properties set by `properties` on a receiver that also reads the node's composition locals and has its
/// coroutine scope, what is below merged in with `merge_descendants` (Compose's internal
/// `Modifier.delegableSemantics`). What `properties` reads is observed: the semantics are read again when it changes.
pub(crate) fn delegable_semantics(
    modifier: Modifier,
    merge_descendants: bool,
    properties: DelegableSemanticsProperties,
) -> Modifier {
    modifier
        .then_element(DelegableSemanticsElement { should_merge_descendant_semantics: merge_descendants, properties })
}

struct DelegableSemanticsElement {
    should_merge_descendant_semantics: bool,
    properties: DelegableSemanticsProperties,
}

/// Compose's `equals`: the same merging and the same properties lambda (by identity).
impl PartialEq for DelegableSemanticsElement {
    fn eq(&self, other: &Self) -> bool {
        self.should_merge_descendant_semantics == other.should_merge_descendant_semantics
            && Rc::ptr_eq(&self.properties, &other.properties)
    }
}

impl ModifierNodeElement for DelegableSemanticsElement {
    type Node = DelegableSemanticsNode;

    fn create(&self) -> DelegableSemanticsNode {
        DelegableSemanticsNode {
            should_merge_descendant_semantics: self.should_merge_descendant_semantics,
            properties: self.properties.clone(),
            handle: None,
        }
    }

    fn update(&self, node: &mut DelegableSemanticsNode) {
        node.should_merge_descendant_semantics = self.should_merge_descendant_semantics;
        node.properties = self.properties.clone();
    }

    fn name(&self) -> &'static str {
        // (Compose's inspector name)
        "defaultPaneExpansionDragHandleSemantics"
    }
}

/// The node (Compose's private `DelegableSemanticsNode`).
struct DelegableSemanticsNode {
    should_merge_descendant_semantics: bool,
    properties: DelegableSemanticsProperties,
    handle: Option<NodeHandle>,
}

impl ModifierNode for DelegableSemanticsNode {
    node_capabilities!(semantics);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.handle = None;
    }
}

impl SemanticsModifierNode for DelegableSemanticsNode {
    fn should_merge_descendant_semantics(&self) -> bool {
        self.should_merge_descendant_semantics
    }

    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        let Some(node) = self.handle.clone() else { return };
        let properties = self.properties.clone();
        properties(&mut DelegableSemanticsPropertyReceiver { config, node: &node });
    }
}

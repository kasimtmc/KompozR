//! The semantics modifiers (Compose's SemanticsModifier.kt and Modifier.testTag): `semantics {}`,
//! `clear_and_set_semantics {}`, `test_tag`, and the node interface they implement.

use super::configuration::SemanticsConfiguration;
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement};
use crate::node_capabilities;
use kompoz_runtime::named;
use std::rc::Rc;

/// A node that sets semantics properties (Compose's SemanticsModifierNode). A change of what it sets is told
/// with [`NodeCx::invalidate_semantics`](crate::modifier::NodeCx::invalidate_semantics).
pub trait SemanticsModifierNode {
    /// Whether the semantics of what is below it are dropped (Compose's `shouldClearDescendantSemantics`).
    fn should_clear_descendant_semantics(&self) -> bool {
        false
    }
    /// Whether what is below it merges into it (Compose's `shouldMergeDescendantSemantics`).
    fn should_merge_descendant_semantics(&self) -> bool {
        false
    }
    /// Whether its bounds are its semantics node's (Compose's `isImportantForBounds`).
    fn is_important_for_bounds(&self) -> bool {
        true
    }
    /// Sets its properties (Compose's `SemanticsPropertyReceiver.applySemantics`).
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration);
}

impl Modifier {
    /// A test tag, for tests to find the layout by (Compose's `Modifier.testTag`).
    pub fn test_tag(self, tag: impl Into<String>) -> Modifier {
        self.then_element(TestTagElement { tag: tag.into() })
    }

    /// [`semantics`](Modifier::semantics) with all its parameters, for plain code.
    pub fn semantics_with(
        self,
        merge_descendants: bool,
        properties: impl Fn(&mut SemanticsConfiguration) + 'static,
    ) -> Modifier {
        self.then_element(SemanticsElement { merge_descendants, clear: false, properties: Rc::new(properties) })
    }
}

#[named]
impl Modifier {
    /// Semantics properties set by `properties`, and with `merge_descendants` those of what is below merged in
    /// (Compose's `Modifier.semantics`).
    pub fn semantics(
        self,
        #[default(false)] merge_descendants: bool,
        properties: impl Fn(&mut SemanticsConfiguration) + 'static,
    ) -> Modifier {
        self.then_element(SemanticsElement { merge_descendants, clear: false, properties: Rc::new(properties) })
    }

    /// Semantics properties set by `properties` instead of those of what is below (Compose's
    /// `Modifier.clearAndSetSemantics`).
    pub fn clear_and_set_semantics(self, properties: impl Fn(&mut SemanticsConfiguration) + 'static) -> Modifier {
        self.then_element(SemanticsElement { merge_descendants: false, clear: true, properties: Rc::new(properties) })
    }
}

struct SemanticsElement {
    merge_descendants: bool,
    clear: bool,
    properties: Rc<dyn Fn(&mut SemanticsConfiguration)>,
}

impl PartialEq for SemanticsElement {
    fn eq(&self, other: &Self) -> bool {
        self.merge_descendants == other.merge_descendants
            && self.clear == other.clear
            && Rc::ptr_eq(&self.properties, &other.properties)
    }
}

impl ModifierNodeElement for SemanticsElement {
    type Node = CoreSemanticsModifierNode;
    fn create(&self) -> CoreSemanticsModifierNode {
        CoreSemanticsModifierNode {
            merge_descendants: self.merge_descendants,
            clear: self.clear,
            properties: self.properties.clone(),
        }
    }
    fn update(&self, node: &mut CoreSemanticsModifierNode) {
        node.merge_descendants = self.merge_descendants;
        node.clear = self.clear;
        node.properties = self.properties.clone();
    }
    fn name(&self) -> &'static str {
        if self.clear { "clearAndSetSemantics" } else { "semantics" }
    }
}

/// The node of `Modifier.semantics` and `Modifier.clear_and_set_semantics` (Compose's CoreSemanticsModifierNode).
pub struct CoreSemanticsModifierNode {
    merge_descendants: bool,
    clear: bool,
    properties: Rc<dyn Fn(&mut SemanticsConfiguration)>,
}

impl ModifierNode for CoreSemanticsModifierNode {
    node_capabilities!(semantics);
}

impl SemanticsModifierNode for CoreSemanticsModifierNode {
    fn should_clear_descendant_semantics(&self) -> bool {
        self.clear
    }
    fn should_merge_descendant_semantics(&self) -> bool {
        self.merge_descendants
    }
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        (self.properties)(config)
    }
}

#[derive(PartialEq)]
struct TestTagElement {
    tag: String,
}

impl ModifierNodeElement for TestTagElement {
    type Node = TestTagNode;
    fn create(&self) -> TestTagNode {
        TestTagNode { tag: self.tag.clone() }
    }
    fn update(&self, node: &mut TestTagNode) {
        node.tag = self.tag.clone();
    }
    fn name(&self) -> &'static str {
        "testTag"
    }
}

/// The node of `Modifier.test_tag` (Compose's TestTagNode).
pub struct TestTagNode {
    tag: String,
}

impl ModifierNode for TestTagNode {
    node_capabilities!(semantics);
}

impl SemanticsModifierNode for TestTagNode {
    fn apply_semantics(&mut self, config: &mut SemanticsConfiguration) {
        config.set_test_tag(self.tag.clone());
    }
}

//! Semantics, as Compose's `androidx.compose.ui.semantics`: what a layout means (its text, its description, its
//! role, its state, its actions…), set by semantics modifier nodes into a [`SemanticsConfiguration`], gathered
//! into a tree of [`SemanticsNode`]s — unmerged, or merged where a node merges its descendants — that
//! accessibility and the test rule read through the host's [`SemanticsOwner`].
//!
//! - [`SemanticsPropertyKey`] (a property, how it merges and combines) and the configuration;
//! - the properties, actions and their values ([`SemanticsProperties`], [`SemanticsActions`], [`Role`],
//!   [`AccessibilityAction`], …) with their setters on the configuration (Compose's SemanticsPropertyReceiver);
//! - the modifiers: `Modifier.semantics {}`, `clear_and_set_semantics {}`, `test_tag`;
//! - the tree: [`SemanticsNode`] (merging, clearing, fake nodes, bounds), [`SemanticsOwner`] (the roots of the
//!   host's window and of its popups, every node, [`SemanticsListener`]s told of changes);
//! - the reading order ([`SemanticsNode::sort_by_geometry_groupings`]) and what accessibility bridges read
//!   ([`AccessibilityNode`], [`SemanticsAction`]).

mod accessibility;
mod configuration;
mod modifier;
mod node;
mod properties;
mod sort;

pub use crate::state::ToggleableState;
pub use accessibility::{AccessibilityNode, SemanticsAction};
pub use configuration::{SemanticsConfiguration, SemanticsPropertyKey};
pub use modifier::{CoreSemanticsModifierNode, SemanticsModifierNode, TestTagNode};
pub use node::{SemanticsListener, SemanticsNode, SemanticsOwner};
pub use properties::{
    AccessibilityAction, ActionFn, CollectionInfo, CollectionItemInfo, ContentDataType, ContentType,
    CustomAccessibilityAction, FillableData, IndexForKey, InputTextSuggestionState, LiveRegionMode,
    ProgressBarRangeInfo, Role, ScrollAxisRange, ScrollByOffsetAction, SemanticsActions, SemanticsProperties,
    TextLayoutResultHandle, TextRange,
};

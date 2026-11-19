//! The layout tree, as Compose's `androidx.compose.ui.node`: layout nodes, their coordinators, the owner and its
//! measure-and-layout delegate, the lookahead pass, and the observation of what measure, placement and drawing read.

mod alignment_lines;
mod coordinator;
mod layout_node;
mod lookahead;
mod measure_pass;
mod owner;
mod rulers;
mod snapshot_observer;

pub use coordinator::NodeCoordinator;
pub use layout_node::LayoutNode;
pub(crate) use layout_node::{LayoutState, UsageByParent};
pub(crate) use lookahead::{LookaheadDelegate, LookaheadPassDelegate, LookaheadRoot};
pub(crate) use owner::Owner;
pub(crate) use snapshot_observer::{ReaderScope, SnapshotObserver};

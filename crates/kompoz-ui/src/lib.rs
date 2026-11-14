//! KompozR's UI core, as Jetpack Compose's `androidx.compose.ui`: layout nodes measured and placed through the
//! coordinators of their modifiers' nodes (`Modifier.Node` chains, diffed as Compose's), incremental measure and
//! layout with real intrinsics and alignment lines, drawing through a [`GraphicsContext`](graphics_context), the
//! semantics tree, and the host that drives a composition frame by frame.
//!
//! Lengths are pixels (whole ones for layout); what an app writes in dp becomes pixels through `LocalDensity`.
//! Single-threaded: the tree is `Rc` and lives on the UI thread; other threads write states (snapshots) or post to
//! the host.

#![allow(non_upper_case_globals, non_snake_case)]

pub mod alignment;
pub mod back;
pub mod draw;
pub mod focus;
pub mod graphics_context;
pub mod host;
pub mod input;
pub mod layout;
pub mod modifier;
pub mod node;
pub mod platform;
pub mod semantics;
pub mod unit;
pub mod window;

pub use alignment::{
    AbsoluteAlignment, Alignment, BiasAbsoluteAlignment, BiasAlignment, HorizontalAlignment, VerticalAlignment,
};
pub use host::{HostHandle, KompozHost};
pub use layout::{
    AlignmentLine, FirstBaseline, IntrinsicMeasurable, IntrinsicMeasureScope, LastBaseline, Layout, LayoutCoordinates,
    Measurable, MeasurePolicy, MeasureResult, MeasureScope, MultiMeasureLayout, Placeable, PlacementScope,
    measure_policy,
};
pub use modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, NodeKind};
pub use node::LayoutNode;
pub use unit::Constraints;

//! Layout, as Compose's `androidx.compose.ui.layout`: measuring and placing, measure policies and intrinsics,
//! alignment lines, layout coordinates, graphics layers' properties, the `Layout` composables and ui's layout
//! modifiers (`layout {}`, `layout_id`, `on_size_changed`, `on_globally_positioned`, `on_placed`, `z_index`,
//! `on_layout_rect_changed`, `on_visibility_changed`, `on_first_visible`, `layout_bounds`).

mod alignment_line;
mod composables;
mod coordinates;
mod layer;
mod measure;
mod modifiers;
mod visibility;

pub use alignment_line::{AlignmentLine, FirstBaseline, HorizontalAlignmentLine, LastBaseline, VerticalAlignmentLine};
pub use composables::{Layout, MultiMeasureLayout, layout_into, measure_policy};
pub use coordinates::LayoutCoordinates;
pub use layer::{
    CompositingStrategy, DefaultCameraDistance, GraphicsLayerScope, LayerBlock, LayerOutsets, TransformOrigin,
};
pub(crate) use layer::{LayerGeometry, layer_matrix, mul, rotation};
pub(crate) use measure::{DefaultIntrinsicMeasurable, MinMax, Side};
pub use measure::{
    IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasurePolicy, MeasureResult, MeasureScope, PlaceTarget,
    Placeable, PlacementScope,
};
pub use modifiers::{
    LayoutIdKey, LayoutIdParentData, MeasureFn, OnGloballyPositionedNode, OnPlacedNode, OnSizeChangedNode, ZIndexNode,
};
pub use visibility::{LayoutBoundsHolder, OnVisibilityChangedNode, on_visibility_changed_node};

use crate::unit::Constraints;
use kompoz_graphics::{Density, IntSize};
use std::any::Any;

/// A node that measures and places what it wraps (Compose's LayoutModifierNode): `measurable` is the rest of the
/// chain and the layout itself. Its intrinsic sizes come, by default, from measuring with the wrapped content
/// answering with its own (Compose's MeasuringIntrinsics).
pub trait LayoutModifierNode {
    fn measure(&mut self, scope: &MeasureScope, measurable: &dyn Measurable, constraints: Constraints)
    -> MeasureResult;

    fn min_intrinsic_width(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        measuring_intrinsic(self, scope, measurable, MinMax::Min, Side::Width, height)
    }
    fn min_intrinsic_height(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        measuring_intrinsic(self, scope, measurable, MinMax::Min, Side::Height, width)
    }
    fn max_intrinsic_width(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        measuring_intrinsic(self, scope, measurable, MinMax::Max, Side::Width, height)
    }
    fn max_intrinsic_height(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        measuring_intrinsic(self, scope, measurable, MinMax::Max, Side::Height, width)
    }
}

/// Compose's MeasuringIntrinsics: a layout modifier's intrinsic size, measured with what it wraps answering with
/// its intrinsic size.
fn measuring_intrinsic<N: LayoutModifierNode + ?Sized>(
    node: &mut N,
    scope: &IntrinsicMeasureScope,
    measurable: &dyn IntrinsicMeasurable,
    min_max: MinMax,
    side: Side,
    size: i32,
) -> i32 {
    let wrapped = DefaultIntrinsicMeasurable { measurable, min_max, side };
    let constraints = match side {
        Side::Width => Constraints::new(0, Constraints::Infinity, 0, size),
        Side::Height => Constraints::new(0, size, 0, Constraints::Infinity),
    };
    let result = node.measure(&scope.for_intrinsics(), &wrapped, constraints);
    match side {
        Side::Width => result.width,
        Side::Height => result.height,
    }
}

/// A node that tells the parent layout something about its child (Compose's ParentDataModifierNode): given what
/// the nodes after it said (`parent_data`), it returns what the parent gets.
pub trait ParentDataModifierNode {
    fn modify_parent_data(&mut self, density: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>>;
}

/// A node told about its layout's measurement and placement (Compose's LayoutAwareModifierNode).
pub trait LayoutAwareModifierNode {
    /// The layout it belongs to was measured, to `size`.
    fn on_remeasured(&mut self, size: IntSize) {
        let _ = size;
    }
    /// It was placed; `coordinates` are its coordinator's.
    fn on_placed(&mut self, coordinates: &LayoutCoordinates) {
        let _ = coordinates;
    }
    /// Whether it wants `on_remeasured` (a node only placing says false, and its layout isn't measured again
    /// when it changes: Compose's OnPlacedNode).
    fn observes_remeasure(&self) -> bool {
        true
    }
}

/// A node told where its layout is after each layout pass that moved or resized it or an ancestor (Compose's
/// GlobalPositionAwareModifierNode).
pub trait GlobalPositionAwareModifierNode {
    fn on_globally_positioned(&mut self, coordinates: &LayoutCoordinates);
}

/// A node told when its layout stops being placed (Compose's UnplacedAwareModifierNode).
pub trait UnplacedAwareModifierNode {
    fn on_unplaced(&mut self);
}

//! Intrinsic sizes as a size, as Compose's `Modifier.width(IntrinsicSize)` family: a layout as wide (or high) as
//! what it wraps says it needs at least, or at most.

use kompoz_runtime::named;
use kompoz_ui::layout::LayoutModifierNode;
use kompoz_ui::{
    Constraints, IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasureResult, MeasureScope, Modifier,
    ModifierNode, ModifierNodeElement, node_capabilities,
};

/// Which of a layout's intrinsic sizes a size is (Compose's IntrinsicSize): the least it can be and still show
/// itself well, or the most it uses.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IntrinsicSize {
    Min,
    Max,
}

/// The intrinsic size modifiers (Compose's `width(IntrinsicSize)`, `height(IntrinsicSize)`,
/// `requiredWidth(IntrinsicSize)` and `requiredHeight(IntrinsicSize)`; Compose's overloads of `width` and the
/// others, named for what they take as the size modifiers' overloads are). The size is what the content gives
/// at the incoming max across (Compose measures intrinsics so: one more pass over it, as cheap as a measurement).
#[named(trait IntrinsicSizeModifiers)]
impl Modifier {
    /// As wide as its content's min or max intrinsic width at the incoming max height, within the incoming
    /// constraints (Compose's `width(intrinsicSize)`).
    fn width_intrinsic(self, intrinsic_size: IntrinsicSize) -> Modifier {
        self.then_element(IntrinsicElement { side: Side::Width, size: intrinsic_size, enforce_incoming: true })
    }

    /// As high as its content's min or max intrinsic height at the incoming max width, within the incoming
    /// constraints (Compose's `height(intrinsicSize)`).
    fn height_intrinsic(self, intrinsic_size: IntrinsicSize) -> Modifier {
        self.then_element(IntrinsicElement { side: Side::Height, size: intrinsic_size, enforce_incoming: true })
    }

    /// Exactly as wide as its content's intrinsic width, whatever the incoming constraints (Compose's
    /// `requiredWidth(intrinsicSize)`).
    fn required_width_intrinsic(self, intrinsic_size: IntrinsicSize) -> Modifier {
        self.then_element(IntrinsicElement { side: Side::Width, size: intrinsic_size, enforce_incoming: false })
    }

    /// Exactly as high as its content's intrinsic height (Compose's `requiredHeight(intrinsicSize)`).
    fn required_height_intrinsic(self, intrinsic_size: IntrinsicSize) -> Modifier {
        self.then_element(IntrinsicElement { side: Side::Height, size: intrinsic_size, enforce_incoming: false })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    Width,
    Height,
}

#[derive(PartialEq)]
struct IntrinsicElement {
    side: Side,
    size: IntrinsicSize,
    enforce_incoming: bool,
}

impl ModifierNodeElement for IntrinsicElement {
    type Node = IntrinsicNode;
    fn create(&self) -> IntrinsicNode {
        IntrinsicNode { side: self.side, size: self.size, enforce_incoming: self.enforce_incoming }
    }
    fn update(&self, node: &mut IntrinsicNode) {
        node.side = self.side;
        node.size = self.size;
        node.enforce_incoming = self.enforce_incoming;
    }
    fn name(&self) -> &'static str {
        match (self.side, self.enforce_incoming) {
            (Side::Width, true) => "width",
            (Side::Height, true) => "height",
            (Side::Width, false) => "requiredWidth",
            (Side::Height, false) => "requiredHeight",
        }
    }
}

/// The node of the intrinsic size modifiers (Compose's IntrinsicWidthNode and IntrinsicHeightNode, on
/// IntrinsicSizeModifier): its content measured at the fixed size its intrinsic size gives that way (brought
/// within the incoming constraints, `enforce_incoming`). Its own intrinsic size that way is the one it takes,
/// the other way its content's.
struct IntrinsicNode {
    side: Side,
    size: IntrinsicSize,
    enforce_incoming: bool,
}

impl IntrinsicNode {
    /// The width it takes at `height` (its content's min or max intrinsic width).
    fn width_at(&self, measurable: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        match self.size {
            IntrinsicSize::Min => measurable.min_intrinsic_width(height),
            IntrinsicSize::Max => measurable.max_intrinsic_width(height),
        }
    }

    fn height_at(&self, measurable: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        match self.size {
            IntrinsicSize::Min => measurable.min_intrinsic_height(width),
            IntrinsicSize::Max => measurable.max_intrinsic_height(width),
        }
    }
}

impl ModifierNode for IntrinsicNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for IntrinsicNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        // Compose's calculateContentConstraints.
        let content = match self.side {
            Side::Width => Constraints::fixed_width(self.width_at(measurable, constraints.max_height).max(0)),
            Side::Height => Constraints::fixed_height(self.height_at(measurable, constraints.max_width).max(0)),
        };
        let placeable =
            measurable.measure(if self.enforce_incoming { constraints.constrain(content) } else { content });
        scope.layout(placeable.width(), placeable.height(), move |p| p.place_relative(&placeable, 0, 0))
    }

    fn min_intrinsic_width(
        &mut self,
        _: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        match self.side {
            Side::Width => self.width_at(measurable, height),
            Side::Height => measurable.min_intrinsic_width(height),
        }
    }

    fn max_intrinsic_width(
        &mut self,
        _: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        match self.side {
            Side::Width => self.width_at(measurable, height),
            Side::Height => measurable.max_intrinsic_width(height),
        }
    }

    fn min_intrinsic_height(
        &mut self,
        _: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        match self.side {
            Side::Width => measurable.min_intrinsic_height(width),
            Side::Height => self.height_at(measurable, width),
        }
    }

    fn max_intrinsic_height(
        &mut self,
        _: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        match self.side {
            Side::Width => measurable.max_intrinsic_height(width),
            Side::Height => self.height_at(measurable, width),
        }
    }
}

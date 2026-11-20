//! The minimum interactive size, as Compose's InteractiveComponentSize.kt: an interactive component smaller than
//! [`LocalMinimumInteractiveComponentSize`] (48 dp) is measured that big, itself in the middle, so it is easy to
//! touch; where it was put in that room is told to its parent as two alignment lines.

use kompoz_graphics::Dp;
use kompoz_runtime::{KompozitionLocal, named, named_args};
use kompoz_ui::layout::{
    AlignmentLine, HorizontalAlignmentLine, LayoutModifierNode, Measurable, MeasureResult, MeasureScope,
    VerticalAlignmentLine,
};
use kompoz_ui::{Constraints, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};

/// The modifier of the minimum interactive size (Compose's `Modifier.minimumInteractiveComponentSize`).
#[named(trait InteractiveComponentSizeModifiers)]
impl Modifier {
    /// Reserves at least [`LocalMinimumInteractiveComponentSize`] (48 dp; none where 0 or unspecified) each way:
    /// a smaller layout is measured that big, in the middle of it (Compose's
    /// `Modifier.minimumInteractiveComponentSize()`), its offset in that room told as
    /// [`MinimumInteractiveLeftAlignmentLine`] and [`MinimumInteractiveTopAlignmentLine`].
    fn minimum_interactive_component_size(self) -> Modifier {
        self.then_element(MinimumInteractiveModifier)
    }
}

/// Compose's internal MinimumInteractiveModifier: one element, equal only to itself.
#[derive(PartialEq)]
struct MinimumInteractiveModifier;

impl ModifierNodeElement for MinimumInteractiveModifier {
    type Node = MinimumInteractiveModifierNode;
    fn create(&self) -> MinimumInteractiveModifierNode {
        MinimumInteractiveModifierNode { handle: None }
    }
    fn update(&self, _: &mut MinimumInteractiveModifierNode) {}
    fn name(&self) -> &'static str {
        "minimumInteractiveComponentSize"
    }
}

/// Compose's internal MinimumInteractiveModifierNode.
struct MinimumInteractiveModifierNode {
    handle: Option<NodeHandle>,
}

impl ModifierNode for MinimumInteractiveModifierNode {
    node_capabilities!(layout);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.handle = None;
    }
}

impl LayoutModifierNode for MinimumInteractiveModifierNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let cx = self.handle.as_ref().and_then(NodeHandle::cx);
        let size = cx.as_ref().map_or(Dp(48.0), |cx| cx.current_value_of(&LocalMinimumInteractiveComponentSize));
        let size = if size.is_specified() { Dp(size.0.max(0.0)) } else { size };
        let placeable = measurable.measure(constraints);
        let is_attached = cx.as_ref().is_some_and(NodeCx::is_attached);
        let enforcement = is_attached && size.is_specified() && size.0 > 0.0;
        let size_px = if size.is_specified() { scope.round_to_px(size) } else { 0 };
        let width = if enforcement { placeable.width().max(size_px) } else { placeable.width() };
        let height = if enforcement { placeable.height().max(size_px) } else { placeable.height() };
        let alignment_lines: Vec<(AlignmentLine, i32)> = if enforcement {
            vec![
                (MinimumInteractiveLeftAlignmentLine.into(), half_room(size_px, placeable.width())),
                (MinimumInteractiveTopAlignmentLine.into(), half_room(size_px, placeable.height())),
            ]
        } else {
            Vec::new()
        };
        let center_x = ((width - placeable.width()) as f32 / 2.0).round() as i32;
        let center_y = ((height - placeable.height()) as f32 / 2.0).round() as i32;
        named_args!(scope.layout(
            width,
            height,
            alignment_lines = alignment_lines,
            placement_block = move |p| p.place(&placeable, center_x, center_y)
        ))
    }
}

/// Half the room `size_px` leaves around `measured` (none where it leaves none).
fn half_room(size_px: i32, measured: i32) -> i32 {
    ((size_px - measured) as f32 / 2.0).round().max(0.0) as i32
}

fn min(a: i32, b: i32) -> i32 {
    a.min(b)
}

/// How far down the layout is in the room the minimum interactive size gave it (Compose's
/// MinimumInteractiveTopAlignmentLine; the smallest of several).
pub const MinimumInteractiveTopAlignmentLine: HorizontalAlignmentLine =
    HorizontalAlignmentLine::new("MinimumInteractiveTopAlignmentLine", min);

/// How far across the layout is in the room the minimum interactive size gave it (Compose's
/// MinimumInteractiveLeftAlignmentLine; the smallest of several).
pub const MinimumInteractiveLeftAlignmentLine: VerticalAlignmentLine =
    VerticalAlignmentLine::new("MinimumInteractiveLeftAlignmentLine", min);

/// Whether the minimum interactive size is kept (Compose's deprecated LocalMinimumInteractiveComponentEnforcement:
/// [`LocalMinimumInteractiveComponentSize`] of 0 turns it off instead).
#[deprecated(note = "Use LocalMinimumInteractiveComponentSize with 0.dp to turn off enforcement instead.")]
pub static LocalMinimumInteractiveComponentEnforcement: KompozitionLocal<bool> = KompozitionLocal::new_static(|| true);

/// The size interactive components are kept at least (Compose's LocalMinimumInteractiveComponentSize): 48 dp; 0
/// or unspecified turns it off.
pub static LocalMinimumInteractiveComponentSize: KompozitionLocal<Dp> = KompozitionLocal::new_static(|| Dp(48.0));

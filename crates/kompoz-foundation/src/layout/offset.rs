//! Offsets, as Compose's `Modifier.offset` and `Modifier.absoluteOffset`: what a layout wraps moved, its size and
//! place in its parent as they were.

use super::same_dp;
use kompoz_graphics::{Density, Dp, IntOffset, UnitExt};
use kompoz_runtime::named;
use kompoz_ui::layout::{LayerBlock, LayoutModifierNode};
use kompoz_ui::{
    Constraints, Measurable, MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement, NodeCx,
    NodeHandle, node_capabilities,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// An offset worked out when the layout is placed (what [`offset_px`](OffsetModifiers::offset_px) keeps).
pub type OffsetFn = Rc<dyn Fn(&Density) -> IntOffset>;

/// The offset modifiers (Compose's `Modifier.offset` and `Modifier.absoluteOffset`). The offset doesn't change
/// the layout's size or where its parent puts it, only where what it wraps is drawn, hit and reported; a positive
/// `x` moves towards the end (mirrored right to left), or to the right for the absolute ones.
#[named(trait OffsetModifiers)]
impl Modifier {
    /// Moved by `x` and `y` (Compose's `offset(x, y)`): `x` towards the end.
    fn offset(self, #[default(0.dp())] x: Dp, #[default(0.dp())] y: Dp) -> Modifier {
        self.then_element(OffsetElement { x, y, rtl_aware: true })
    }

    /// [`offset`](OffsetModifiers::offset) with all its parameters, for plain code.
    fn offset_with(self, x: Dp, y: Dp) -> Modifier {
        self.then_element(OffsetElement { x, y, rtl_aware: true })
    }

    /// Moved by `x` and `y`, `x` to the right whatever the layout direction (Compose's `absoluteOffset(x, y)`).
    fn absolute_offset(self, #[default(0.dp())] x: Dp, #[default(0.dp())] y: Dp) -> Modifier {
        self.then_element(OffsetElement { x, y, rtl_aware: false })
    }

    /// [`absolute_offset`](OffsetModifiers::absolute_offset) with all its parameters, for plain code.
    fn absolute_offset_with(self, x: Dp, y: Dp) -> Modifier {
        self.then_element(OffsetElement { x, y, rtl_aware: false })
    }

    /// Moved by the pixels `offset` gives, worked out as the layout is placed (Compose's `offset { }`): the
    /// states it reads place the layout again when they change, without measuring or composing it (what an
    /// animated or scrolled offset wants). The x goes towards the end. What it wraps is placed through a
    /// graphics layer, as Compose's.
    ///
    /// Kotlin: `offset(offset: Density.() -> IntOffset)`
    fn offset_px(self, offset: impl Fn(&Density) -> IntOffset + 'static) -> Modifier {
        self.then_element(OffsetPxElement { offset: Rc::new(offset), rtl_aware: true })
    }

    /// The same, x to the right whatever the layout direction (Compose's `absoluteOffset { }`).
    ///
    /// Kotlin: `absoluteOffset(offset: Density.() -> IntOffset)`
    fn absolute_offset_px(self, offset: impl Fn(&Density) -> IntOffset + 'static) -> Modifier {
        self.then_element(OffsetPxElement { offset: Rc::new(offset), rtl_aware: false })
    }
}

// ---- offset(x, y) ----

struct OffsetElement {
    x: Dp,
    y: Dp,
    rtl_aware: bool,
}

impl PartialEq for OffsetElement {
    fn eq(&self, other: &Self) -> bool {
        same_dp(self.x, other.x) && same_dp(self.y, other.y) && self.rtl_aware == other.rtl_aware
    }
}

impl ModifierNodeElement for OffsetElement {
    type Node = OffsetNode;
    fn create(&self) -> OffsetNode {
        OffsetNode { offset: Rc::new(Cell::new((self.x, self.y, self.rtl_aware))), handle: None }
    }
    fn update(&self, node: &mut OffsetNode) {
        let (x, y, rtl_aware) = node.offset.get();
        if !same_dp(x, self.x) || !same_dp(y, self.y) || rtl_aware != self.rtl_aware {
            node.offset.set((self.x, self.y, self.rtl_aware));
            if let Some(h) = &node.handle {
                h.invalidate_placement();
            }
        }
    }
    fn name(&self) -> &'static str {
        if self.rtl_aware { "offset" } else { "absoluteOffset" }
    }
}

/// The node of `offset` and `absolute_offset` (Compose's OffsetNode): what it wraps measured as it is, placed
/// moved. A new offset only places the layout again (the placement reads the node's latest).
struct OffsetNode {
    /// x, y, and whether x goes towards the end.
    offset: Rc<Cell<(Dp, Dp, bool)>>,
    handle: Option<NodeHandle>,
}

impl ModifierNode for OffsetNode {
    node_capabilities!(layout);
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
    fn on_detach(&mut self, _: &NodeCx) {
        self.handle = None;
    }
    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl LayoutModifierNode for OffsetNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let offset = self.offset.clone();
        let density = scope.density();
        scope.layout(placeable.width(), placeable.height(), move |p| {
            let (x, y, rtl_aware) = offset.get();
            let (x, y) = (density.round_to_px(x), density.round_to_px(y));
            if rtl_aware {
                p.place_relative(&placeable, x, y);
            } else {
                p.place(&placeable, x, y);
            }
        })
    }
}

// ---- offset { } ----

struct OffsetPxElement {
    offset: OffsetFn,
    rtl_aware: bool,
}

impl PartialEq for OffsetPxElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.offset, &other.offset) && self.rtl_aware == other.rtl_aware
    }
}

impl ModifierNodeElement for OffsetPxElement {
    type Node = OffsetPxNode;
    fn create(&self) -> OffsetPxNode {
        OffsetPxNode {
            offset: Rc::new(RefCell::new((self.offset.clone(), self.rtl_aware))),
            layer: Rc::new(|_| {}),
            handle: None,
        }
    }
    fn update(&self, node: &mut OffsetPxNode) {
        let changed = {
            let (offset, rtl_aware) = &*node.offset.borrow();
            !Rc::ptr_eq(offset, &self.offset) || *rtl_aware != self.rtl_aware
        };
        if changed {
            *node.offset.borrow_mut() = (self.offset.clone(), self.rtl_aware);
            if let Some(h) = &node.handle {
                h.invalidate_placement();
            }
        }
    }
    fn name(&self) -> &'static str {
        if self.rtl_aware { "offset" } else { "absoluteOffset" }
    }
}

/// The node of `offset_px` and `absolute_offset_px` (Compose's OffsetPxNode): what it wraps measured as it is,
/// placed through a layer where the block says, the block run as the layout is placed (its reads observed
/// there). A new block only places the layout again.
struct OffsetPxNode {
    /// The block, and whether its x goes towards the end.
    offset: Rc<RefCell<(OffsetFn, bool)>>,
    /// The layer's block (none of its properties set: Compose's DefaultLayerBlock), kept so placing again isn't
    /// taken for a new layer.
    layer: LayerBlock,
    handle: Option<NodeHandle>,
}

impl ModifierNode for OffsetPxNode {
    node_capabilities!(layout);
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
    fn on_detach(&mut self, _: &NodeCx) {
        self.handle = None;
    }
    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

impl LayoutModifierNode for OffsetPxNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let offset = self.offset.clone();
        let layer = self.layer.clone();
        let density = scope.density();
        scope.layout(placeable.width(), placeable.height(), move |p| {
            // (copied out first: the block is the app's code)
            let (block, rtl_aware) = offset.borrow().clone();
            let at = block(&density);
            if rtl_aware {
                p.place_relative_with_layer_block(&placeable, at, layer.clone(), 0.0);
            } else {
                p.place_with_layer_block(&placeable, at, layer.clone(), 0.0);
            }
        })
    }
}

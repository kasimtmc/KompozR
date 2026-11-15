//! Fitting inside or outside rulers, as Compose's `Modifier.fitInside` and `Modifier.fitOutside`: what a layout
//! wraps sized and placed by a rectangle of rulers (the window insets rulers, say), read as it is placed.

use super::{coerce_in, round};
use kompoz_ui::layout::{LayoutModifierNode, RectRulers};
use kompoz_ui::{
    Constraints, Measurable, MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement,
    node_capabilities,
};

/// The ruler alignment modifiers (Compose's `fitInside` and `fitOutside`, from RulerAlignment.kt).
pub trait RulerAlignmentModifiers {
    /// What it wraps fills the part of the layout inside `rulers` (Compose's `fitInside`): with bounded
    /// constraints the layout takes the most it may and measures what it wraps, once placing knows the rulers, to
    /// that part; otherwise it is as big as what it wraps, which is centered in that part. A ruler not provided
    /// is the layout's edge.
    fn fit_inside(self, rulers: RectRulers) -> Modifier;

    /// What it wraps fills the part of the layout outside `rulers` on the first side where there is one: the left,
    /// the top, the right, then the bottom (Compose's `fitOutside`): with bounded constraints; otherwise (or with
    /// nothing outside) it is 0 by 0.
    fn fit_outside(self, rulers: RectRulers) -> Modifier;
}

impl RulerAlignmentModifiers for Modifier {
    fn fit_inside(self, rulers: RectRulers) -> Modifier {
        self.then_element(RulerAlignmentElement { rulers, inside: true })
    }

    fn fit_outside(self, rulers: RectRulers) -> Modifier {
        self.then_element(RulerAlignmentElement { rulers, inside: false })
    }
}

#[derive(PartialEq)]
struct RulerAlignmentElement {
    rulers: RectRulers,
    inside: bool,
}

impl ModifierNodeElement for RulerAlignmentElement {
    type Node = RulerAlignmentNode;
    fn create(&self) -> RulerAlignmentNode {
        RulerAlignmentNode { rulers: self.rulers.clone(), inside: self.inside }
    }
    fn update(&self, node: &mut RulerAlignmentNode) {
        node.rulers = self.rulers.clone();
        node.inside = self.inside;
    }
    fn name(&self) -> &'static str {
        if self.inside { "fitInside" } else { "fitOutside" }
    }
}

struct RulerAlignmentNode {
    rulers: RectRulers,
    inside: bool,
}

impl ModifierNode for RulerAlignmentNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for RulerAlignmentNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let rulers = self.rulers.clone();
        let bounded = constraints.has_bounded_width() && constraints.has_bounded_height();
        // (measured as placing knows the rulers, as Compose's: through a handle kept for the placement block)
        let retained = measurable.retain();
        if self.inside {
            if bounded && let Some(measurable) = retained {
                let (width, height) = (constraints.max_width, constraints.max_height);
                return scope.layout(width, height, move |p| {
                    let left = coerce_in(round(p.current(&rulers.left, 0.0)), 0, width);
                    let top = coerce_in(round(p.current(&rulers.top, 0.0)), 0, height);
                    let right = coerce_in(round(p.current(&rulers.right, width as f32)), 0, width);
                    let bottom = coerce_in(round(p.current(&rulers.bottom, height as f32)), 0, height);
                    let placeable = measurable.measure(Constraints::fixed(right - left, bottom - top));
                    p.place(&placeable, left, top);
                });
            }
            // Can't use the rulers without knowing the size: centered in the space they leave.
            let placeable = measurable.measure(constraints);
            let (width, height) = (placeable.width(), placeable.height());
            return scope.layout(width, height, move |p| {
                let left = coerce_in(round(p.current(&rulers.left, 0.0)), 0, width);
                let top = coerce_in(round(p.current(&rulers.top, 0.0)), 0, height);
                let right = coerce_in(round(p.current(&rulers.right, width as f32)), 0, width);
                let bottom = coerce_in(round(p.current(&rulers.bottom, height as f32)), 0, height);
                p.place(&placeable, (left + right - width) / 2, (top + bottom - height) / 2);
            });
        }
        let Some(measurable) = retained.filter(|_| bounded) else {
            // Can't use the rulers without knowing the size.
            let placeable = measurable.measure(Constraints::fixed(0, 0));
            return scope.layout(0, 0, move |p| p.place(&placeable, 0, 0));
        };
        let (width, height) = (constraints.max_width, constraints.max_height);
        scope.layout(width, height, move |p| {
            let left = round(p.current(&rulers.left, 0.0));
            let top = round(p.current(&rulers.top, 0.0));
            let right = round(p.current(&rulers.right, width as f32));
            let bottom = round(p.current(&rulers.bottom, height as f32));
            let (mut child_width, mut child_height, mut place_left, mut place_top) = (width, height, 0, 0);
            if left > 0 {
                child_width = left; // to the left edge
            } else if top > 0 {
                child_height = top;
            } else if right < width {
                place_left = right;
                child_width = width - right;
            } else if bottom < height {
                place_top = bottom;
                child_height = height - bottom;
            } else {
                child_width = 0;
                child_height = 0;
            }
            let placeable = measurable.measure(Constraints::fixed(child_width, child_height));
            p.place(&placeable, place_left, place_top);
        })
    }
}

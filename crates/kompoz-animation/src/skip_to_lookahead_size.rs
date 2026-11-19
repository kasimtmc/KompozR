//! The SkipToLookaheadSize node, as Compose's `SkipToLookaheadSizeNode.kt`: a layout that, in a lookahead scope,
//! measures its content at the lookahead constraints all along the approach and takes its lookahead size (within
//! its constraints) — the content skips to where it is going while its parent approaches; optionally scaled into the
//! size along the way (`ScaleToBounds`). What `SharedTransitionScope`'s `skipToLookaheadSize()` and its
//! `ResizeMode.ScaleToBounds` are made of.

use kompoz_graphics::{ContentScale, IntSize, ScaleFactor};
use kompoz_ui::layout::{IntrinsicMeasurable, IntrinsicMeasureScope, LayoutModifierNode, Measurable, MeasureResult};
use kompoz_ui::layout::{MeasureScope, PlacementScope, TransformOrigin};
use kompoz_ui::{Alignment, Constraints, Modifier, ModifierNode, ModifierNodeElement, node_capabilities};
use std::rc::Rc;

/// How the content is scaled into the size along the way (Compose's internal ScaleToBoundsImpl, a
/// `SharedTransitionScope.ResizeMode`): by `content_scale` from its lookahead size, placed by `alignment`.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct ScaleToBounds {
    pub(crate) content_scale: ContentScale,
    pub(crate) alignment: Alignment,
}

/// Whether the node skips to the lookahead size (Compose's `isEnabled`), read as it measures.
pub(crate) type SkipEnabled = Rc<dyn Fn() -> bool>;

thread_local! {
    /// Always enabled (Compose's `DefaultEnabled`), one instance so the elements given it stay equal.
    static DEFAULT_ENABLED: SkipEnabled = Rc::new(|| true);
}

/// Always enabled (Compose's private `DefaultEnabled`).
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Compose's default of the element's isEnabled, which no caller in 1.12.1 leaves out")
)]
pub(crate) fn default_skip_enabled() -> SkipEnabled {
    DEFAULT_ENABLED.with(|e| e.clone())
}

/// The node's element (Compose's SkipToLookaheadSizeElement): equal with an equal scaling and the same `is_enabled`.
pub(crate) struct SkipToLookaheadSizeElement {
    pub(crate) scale_to_bounds: Option<ScaleToBounds>,
    pub(crate) is_enabled: SkipEnabled,
}

impl PartialEq for SkipToLookaheadSizeElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.is_enabled, &other.is_enabled) && self.scale_to_bounds == other.scale_to_bounds
    }
}

impl ModifierNodeElement for SkipToLookaheadSizeElement {
    type Node = SkipToLookaheadSizeNode;
    fn create(&self) -> SkipToLookaheadSizeNode {
        SkipToLookaheadSizeNode {
            scale_to_bounds: self.scale_to_bounds.clone(),
            is_enabled: self.is_enabled.clone(),
            lookahead_constraints: None,
            lookahead_size: None,
        }
    }
    fn update(&self, node: &mut SkipToLookaheadSizeNode) {
        // (Compose keeps these in states its measure reads: an update measures it again here)
        node.scale_to_bounds = self.scale_to_bounds.clone();
        node.is_enabled = self.is_enabled.clone();
    }
    fn name(&self) -> &'static str {
        "skipToLookahead"
    }
}

/// The node (Compose's SkipToLookaheadSizeNode).
pub(crate) struct SkipToLookaheadSizeNode {
    scale_to_bounds: Option<ScaleToBounds>,
    is_enabled: SkipEnabled,
    lookahead_constraints: Option<Constraints>,
    /// Its content's lookahead size (Compose's `lookaheadSize`, InvalidSize as none).
    lookahead_size: Option<IntSize>,
}

impl ModifierNode for SkipToLookaheadSizeNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for SkipToLookaheadSizeNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let looking_ahead = scope.is_looking_ahead();
        if looking_ahead {
            self.lookahead_constraints = Some(constraints);
        }
        if !(self.is_enabled)() {
            let placeable = measurable.measure(constraints);
            let (w, h) = (placeable.width(), placeable.height());
            return scope.layout(w, h, move |p: &PlacementScope| p.place(&placeable, 0, 0));
        }
        let placeable = if looking_ahead {
            let p = measurable.measure(constraints);
            self.lookahead_size = Some(IntSize::new(p.width(), p.height()));
            p
        } else {
            let lookahead_constraints =
                self.lookahead_constraints.expect("kompoz: skipping to a lookahead size before the lookahead pass");
            measurable.measure(lookahead_constraints)
        };
        // (Compose constrains its InvalidSize here before a lookahead: the smallest size the constraints allow)
        let lookahead_size = self.lookahead_size.unwrap_or(IntSize::new(i32::MIN, i32::MIN));
        let constrained_size = constraints.constrain_size(lookahead_size);
        let scale_to_bounds = self.scale_to_bounds.clone();
        let layout_direction = scope.layout_direction();
        scope.layout(constrained_size.width, constrained_size.height, move |p: &PlacementScope| {
            let Some(scale_to_bounds) = &scale_to_bounds else {
                p.place(&placeable, 0, 0);
                return;
            };
            let resolved_scale = if lookahead_size.width == 0 || lookahead_size.height == 0 {
                ScaleFactor::new(1.0, 1.0)
            } else {
                scale_to_bounds.content_scale.compute_scale_factor(lookahead_size.to_size(), constrained_size.to_size())
            };
            let scaled = IntSize::new(
                (lookahead_size.width as f32 * resolved_scale.scale_x).round() as i32,
                (lookahead_size.height as f32 * resolved_scale.scale_y).round() as i32,
            );
            let at = scale_to_bounds.alignment.align(scaled, constrained_size, layout_direction);
            p.place_with_layer(&placeable, at.x, at.y, move |layer| {
                layer.scale_x = resolved_scale.scale_x;
                layer.scale_y = resolved_scale.scale_y;
                layer.transform_origin = TransformOrigin::new(0.0, 0.0);
            });
        })
    }

    // If lookahead has occurred, the lookahead size answers (the call goes no further, and agrees with the
    // lookahead); before, or in it, what it wraps does.
    fn max_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        match self.lookahead_size {
            Some(size) if !scope.is_looking_ahead() => size.width,
            _ => m.max_intrinsic_width(height),
        }
    }
    fn min_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        match self.lookahead_size {
            Some(size) if !scope.is_looking_ahead() => size.width,
            _ => m.min_intrinsic_width(height),
        }
    }
    fn max_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        match self.lookahead_size {
            Some(size) if !scope.is_looking_ahead() => size.height,
            _ => m.max_intrinsic_height(width),
        }
    }
    fn min_intrinsic_height(&mut self, scope: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, width: i32) -> i32 {
        match self.lookahead_size {
            Some(size) if !scope.is_looking_ahead() => size.height,
            _ => m.min_intrinsic_height(width),
        }
    }
}

/// The content scaled into the size along the way (Compose's `createContentScaleModifier`): clipped by a graphics
/// layer while enabled when it crops, then the node scaling it.
pub(crate) fn content_scale_modifier(
    modifier: Modifier,
    scale_to_bounds: ScaleToBounds,
    is_enabled: SkipEnabled,
) -> Modifier {
    let clipped = if scale_to_bounds.content_scale == ContentScale::Crop {
        let enabled = is_enabled.clone();
        modifier.graphics_layer_block(move |layer| layer.clip = enabled())
    } else {
        modifier
    };
    clipped.then_element(SkipToLookaheadSizeElement { scale_to_bounds: Some(scale_to_bounds), is_enabled })
}

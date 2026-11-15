//! Aspect ratios, as Compose's `Modifier.aspectRatio`: a size of a given width to height within the constraints.

use super::round;
use kompoz_graphics::IntSize;
use kompoz_runtime::named;
use kompoz_ui::layout::LayoutModifierNode;
use kompoz_ui::{
    Constraints, IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasureResult, MeasureScope, Modifier,
    ModifierNode, ModifierNodeElement, node_capabilities,
};

/// The aspect ratio modifier (Compose's `Modifier.aspectRatio`).
#[named(trait AspectRatioModifiers)]
impl Modifier {
    /// Sized `ratio` wide to 1 high (Compose's `aspectRatio`): the first of these that the incoming constraints
    /// allow — the max width, the max height, the min width, the min height, each with the other side worked out
    /// from it — or, when none fits, the first that has a size at all; with none, as what it wraps measures.
    /// With `match_height_constraints_first` the heights are tried before the widths. Panics on a ratio that
    /// isn't more than 0.
    #[track_caller]
    fn aspect_ratio(self, ratio: f32, #[default(false)] match_height_constraints_first: bool) -> Modifier {
        self.then_element(AspectRatioElement::new(ratio, match_height_constraints_first))
    }

    /// [`aspect_ratio`](AspectRatioModifiers::aspect_ratio) with all its parameters, for plain code.
    #[track_caller]
    fn aspect_ratio_with(self, ratio: f32, match_height_constraints_first: bool) -> Modifier {
        self.then_element(AspectRatioElement::new(ratio, match_height_constraints_first))
    }
}

#[derive(PartialEq)]
struct AspectRatioElement {
    aspect_ratio: f32,
    match_height_constraints_first: bool,
}

impl AspectRatioElement {
    #[track_caller]
    fn new(aspect_ratio: f32, match_height_constraints_first: bool) -> AspectRatioElement {
        assert!(aspect_ratio > 0.0, "aspectRatio {aspect_ratio} must be > 0");
        AspectRatioElement { aspect_ratio, match_height_constraints_first }
    }
}

impl ModifierNodeElement for AspectRatioElement {
    type Node = AspectRatioNode;
    fn create(&self) -> AspectRatioNode {
        AspectRatioNode {
            aspect_ratio: self.aspect_ratio,
            match_height_constraints_first: self.match_height_constraints_first,
        }
    }
    fn update(&self, node: &mut AspectRatioNode) {
        node.aspect_ratio = self.aspect_ratio;
        node.match_height_constraints_first = self.match_height_constraints_first;
    }
    fn name(&self) -> &'static str {
        "aspectRatio"
    }
}

/// The node of `aspect_ratio` (Compose's AspectRatioNode): what it wraps measured at the size it finds (or as
/// the constraints allow, when it finds none). Its intrinsic size across a given side is that side through the
/// ratio.
struct AspectRatioNode {
    aspect_ratio: f32,
    match_height_constraints_first: bool,
}

/// Whether `width` by `height` is within `c` (Compose's `isSatisfiedBy`).
fn satisfied_by(c: Constraints, width: i32, height: i32) -> bool {
    (c.min_width..=c.max_width).contains(&width) && (c.min_height..=c.max_height).contains(&height)
}

impl AspectRatioNode {
    /// The size the constraints give it (Compose's `findSize`), zero if none.
    fn find_size(&self, c: Constraints) -> IntSize {
        type Try = fn(&AspectRatioNode, Constraints, bool) -> IntSize;
        let order: [Try; 4] = if self.match_height_constraints_first {
            [Self::try_max_height, Self::try_max_width, Self::try_min_height, Self::try_min_width]
        } else {
            [Self::try_max_width, Self::try_max_height, Self::try_min_width, Self::try_min_height]
        };
        for enforce in [true, false] {
            for attempt in order {
                let size = attempt(self, c, enforce);
                if size != IntSize::Zero {
                    return size;
                }
            }
        }
        IntSize::Zero
    }

    fn try_max_width(&self, c: Constraints, enforce: bool) -> IntSize {
        if c.max_width != Constraints::Infinity {
            let height = round(c.max_width as f32 / self.aspect_ratio);
            if height > 0 && (!enforce || satisfied_by(c, c.max_width, height)) {
                return IntSize::new(c.max_width, height);
            }
        }
        IntSize::Zero
    }

    fn try_max_height(&self, c: Constraints, enforce: bool) -> IntSize {
        if c.max_height != Constraints::Infinity {
            let width = round(c.max_height as f32 * self.aspect_ratio);
            if width > 0 && (!enforce || satisfied_by(c, width, c.max_height)) {
                return IntSize::new(width, c.max_height);
            }
        }
        IntSize::Zero
    }

    fn try_min_width(&self, c: Constraints, enforce: bool) -> IntSize {
        let height = round(c.min_width as f32 / self.aspect_ratio);
        if height > 0 && (!enforce || satisfied_by(c, c.min_width, height)) {
            return IntSize::new(c.min_width, height);
        }
        IntSize::Zero
    }

    fn try_min_height(&self, c: Constraints, enforce: bool) -> IntSize {
        let width = round(c.min_height as f32 * self.aspect_ratio);
        if width > 0 && (!enforce || satisfied_by(c, width, c.min_height)) {
            return IntSize::new(width, c.min_height);
        }
        IntSize::Zero
    }
}

impl ModifierNode for AspectRatioNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for AspectRatioNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let size = self.find_size(constraints);
        let wrapped = if size != IntSize::Zero { Constraints::fixed(size.width, size.height) } else { constraints };
        let placeable = measurable.measure(wrapped);
        scope.layout(placeable.width(), placeable.height(), move |p| p.place_relative(&placeable, 0, 0))
    }

    fn min_intrinsic_width(
        &mut self,
        _: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        if height != Constraints::Infinity {
            round(height as f32 * self.aspect_ratio)
        } else {
            measurable.min_intrinsic_width(height)
        }
    }

    fn max_intrinsic_width(
        &mut self,
        _: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        if height != Constraints::Infinity {
            round(height as f32 * self.aspect_ratio)
        } else {
            measurable.max_intrinsic_width(height)
        }
    }

    fn min_intrinsic_height(
        &mut self,
        _: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        if width != Constraints::Infinity {
            round(width as f32 / self.aspect_ratio)
        } else {
            measurable.min_intrinsic_height(width)
        }
    }

    fn max_intrinsic_height(
        &mut self,
        _: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        if width != Constraints::Infinity {
            round(width as f32 / self.aspect_ratio)
        } else {
            measurable.max_intrinsic_height(width)
        }
    }
}

//! Sizes, as Compose's `Modifier.size` family: preferred and required sizes and ranges, filling the space there
//! is, wrapping the content, and default minimums.

use super::{coerce_in, round, same_dp};
use kompoz_graphics::{Dp, DpSize, IntOffset, IntSize, LayoutDirection};
use kompoz_runtime::named;
use kompoz_ui::layout::LayoutModifierNode;
use kompoz_ui::{
    Alignment, Constraints, HorizontalAlignment, IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasureResult,
    MeasureScope, Modifier, ModifierNode, ModifierNodeElement, VerticalAlignment, node_capabilities,
};

/// The size modifiers (Compose's `Modifier.size` family). The preferred ones (`size`, `width`, `size_in`, …) stay
/// within the constraints coming in; the required ones (`required_size`, …) don't, and what they wrap is centered
/// over the space the parent gave when they come out bigger or smaller.
#[named(trait SizeModifiers)]
impl Modifier {
    /// A preferred width of `width`, within the incoming constraints (Compose's `width`).
    fn width(self, width: Dp) -> Modifier {
        self.then_element(SizeElement::new(width, Dp::Unspecified, width, Dp::Unspecified, true, "width"))
    }

    /// A preferred height (Compose's `height`).
    fn height(self, height: Dp) -> Modifier {
        self.then_element(SizeElement::new(Dp::Unspecified, height, Dp::Unspecified, height, true, "height"))
    }

    /// A preferred `size` by `size` (Compose's `size(size)`).
    fn size(self, size: Dp) -> Modifier {
        self.then_element(SizeElement::new(size, size, size, size, true, "size"))
    }

    /// A preferred `width` by `height` (Compose's `size(width, height)`).
    fn size_wh(self, width: Dp, height: Dp) -> Modifier {
        self.then_element(SizeElement::new(width, height, width, height, true, "size"))
    }

    /// A preferred size (Compose's `size(DpSize)`).
    fn size_dp(self, size: DpSize) -> Modifier {
        self.then_element(SizeElement::new(size.width, size.height, size.width, size.height, true, "size"))
    }

    /// A preferred width from `min` to `max`, an unspecified bound leaving the incoming one (Compose's `widthIn`).
    fn width_in(self, #[default(Dp::Unspecified)] min: Dp, #[default(Dp::Unspecified)] max: Dp) -> Modifier {
        self.then_element(SizeElement::new(min, Dp::Unspecified, max, Dp::Unspecified, true, "widthIn"))
    }

    /// [`width_in`](SizeModifiers::width_in) with all its parameters, for plain code.
    fn width_in_with(self, min: Dp, max: Dp) -> Modifier {
        self.then_element(SizeElement::new(min, Dp::Unspecified, max, Dp::Unspecified, true, "widthIn"))
    }

    /// A preferred height from `min` to `max` (Compose's `heightIn`).
    fn height_in(self, #[default(Dp::Unspecified)] min: Dp, #[default(Dp::Unspecified)] max: Dp) -> Modifier {
        self.then_element(SizeElement::new(Dp::Unspecified, min, Dp::Unspecified, max, true, "heightIn"))
    }

    /// [`height_in`](SizeModifiers::height_in) with all its parameters, for plain code.
    fn height_in_with(self, min: Dp, max: Dp) -> Modifier {
        self.then_element(SizeElement::new(Dp::Unspecified, min, Dp::Unspecified, max, true, "heightIn"))
    }

    /// A preferred size in a range (Compose's `sizeIn`).
    fn size_in(
        self,
        #[default(Dp::Unspecified)] min_width: Dp,
        #[default(Dp::Unspecified)] min_height: Dp,
        #[default(Dp::Unspecified)] max_width: Dp,
        #[default(Dp::Unspecified)] max_height: Dp,
    ) -> Modifier {
        self.then_element(SizeElement::new(min_width, min_height, max_width, max_height, true, "sizeIn"))
    }

    /// [`size_in`](SizeModifiers::size_in) with all its parameters, for plain code.
    fn size_in_with(self, min_width: Dp, min_height: Dp, max_width: Dp, max_height: Dp) -> Modifier {
        self.then_element(SizeElement::new(min_width, min_height, max_width, max_height, true, "sizeIn"))
    }

    /// Exactly `width` wide, whatever the incoming constraints (Compose's `requiredWidth`).
    fn required_width(self, width: Dp) -> Modifier {
        self.then_element(SizeElement::new(width, Dp::Unspecified, width, Dp::Unspecified, false, "requiredWidth"))
    }

    /// Exactly `height` high (Compose's `requiredHeight`).
    fn required_height(self, height: Dp) -> Modifier {
        self.then_element(SizeElement::new(Dp::Unspecified, height, Dp::Unspecified, height, false, "requiredHeight"))
    }

    /// Exactly `size` by `size` (Compose's `requiredSize(size)`).
    fn required_size(self, size: Dp) -> Modifier {
        self.then_element(SizeElement::new(size, size, size, size, false, "requiredSize"))
    }

    /// Exactly `width` by `height` (Compose's `requiredSize(width, height)`).
    fn required_size_wh(self, width: Dp, height: Dp) -> Modifier {
        self.then_element(SizeElement::new(width, height, width, height, false, "requiredSize"))
    }

    /// Exactly `size` (Compose's `requiredSize(DpSize)`).
    fn required_size_dp(self, size: DpSize) -> Modifier {
        self.then_element(SizeElement::new(size.width, size.height, size.width, size.height, false, "requiredSize"))
    }

    /// A width from `min` to `max` whatever the incoming constraints, an unspecified bound taking the incoming
    /// one (Compose's `requiredWidthIn`).
    fn required_width_in(self, #[default(Dp::Unspecified)] min: Dp, #[default(Dp::Unspecified)] max: Dp) -> Modifier {
        self.then_element(SizeElement::new(min, Dp::Unspecified, max, Dp::Unspecified, false, "requiredWidthIn"))
    }

    /// [`required_width_in`](SizeModifiers::required_width_in) with all its parameters, for plain code.
    fn required_width_in_with(self, min: Dp, max: Dp) -> Modifier {
        self.then_element(SizeElement::new(min, Dp::Unspecified, max, Dp::Unspecified, false, "requiredWidthIn"))
    }

    /// A height from `min` to `max` whatever the incoming constraints (Compose's `requiredHeightIn`).
    fn required_height_in(self, #[default(Dp::Unspecified)] min: Dp, #[default(Dp::Unspecified)] max: Dp) -> Modifier {
        self.then_element(SizeElement::new(Dp::Unspecified, min, Dp::Unspecified, max, false, "requiredHeightIn"))
    }

    /// [`required_height_in`](SizeModifiers::required_height_in) with all its parameters, for plain code.
    fn required_height_in_with(self, min: Dp, max: Dp) -> Modifier {
        self.then_element(SizeElement::new(Dp::Unspecified, min, Dp::Unspecified, max, false, "requiredHeightIn"))
    }

    /// A size in a range whatever the incoming constraints (Compose's `requiredSizeIn`).
    fn required_size_in(
        self,
        #[default(Dp::Unspecified)] min_width: Dp,
        #[default(Dp::Unspecified)] min_height: Dp,
        #[default(Dp::Unspecified)] max_width: Dp,
        #[default(Dp::Unspecified)] max_height: Dp,
    ) -> Modifier {
        self.then_element(SizeElement::new(min_width, min_height, max_width, max_height, false, "requiredSizeIn"))
    }

    /// [`required_size_in`](SizeModifiers::required_size_in) with all its parameters, for plain code.
    fn required_size_in_with(self, min_width: Dp, min_height: Dp, max_width: Dp, max_height: Dp) -> Modifier {
        self.then_element(SizeElement::new(min_width, min_height, max_width, max_height, false, "requiredSizeIn"))
    }

    /// As wide as `fraction` of the incoming max width, when there is one (Compose's `fillMaxWidth`).
    fn fill_max_width(self, #[default(1.0)] fraction: f32) -> Modifier {
        self.then_element(FillElement { direction: Direction::Horizontal, fraction })
    }

    /// [`fill_max_width`](SizeModifiers::fill_max_width) with all its parameters, for plain code.
    fn fill_max_width_with(self, fraction: f32) -> Modifier {
        self.then_element(FillElement { direction: Direction::Horizontal, fraction })
    }

    /// As high as `fraction` of the incoming max height, when there is one (Compose's `fillMaxHeight`).
    fn fill_max_height(self, #[default(1.0)] fraction: f32) -> Modifier {
        self.then_element(FillElement { direction: Direction::Vertical, fraction })
    }

    /// [`fill_max_height`](SizeModifiers::fill_max_height) with all its parameters, for plain code.
    fn fill_max_height_with(self, fraction: f32) -> Modifier {
        self.then_element(FillElement { direction: Direction::Vertical, fraction })
    }

    /// `fraction` of the incoming max size both ways (Compose's `fillMaxSize`).
    fn fill_max_size(self, #[default(1.0)] fraction: f32) -> Modifier {
        self.then_element(FillElement { direction: Direction::Both, fraction })
    }

    /// [`fill_max_size`](SizeModifiers::fill_max_size) with all its parameters, for plain code.
    fn fill_max_size_with(self, fraction: f32) -> Modifier {
        self.then_element(FillElement { direction: Direction::Both, fraction })
    }

    /// Measures what it wraps at its own width, the incoming min width ignored (and the max too, `unbounded`),
    /// placed by `align` in the width it takes (Compose's `wrapContentWidth`).
    fn wrap_content_width(
        self,
        #[default(Alignment::CenterHorizontally)] align: HorizontalAlignment,
        #[default(false)] unbounded: bool,
    ) -> Modifier {
        self.then_element(WrapContentElement { align: WrapAlign::Horizontal(align), unbounded })
    }

    /// [`wrap_content_width`](SizeModifiers::wrap_content_width) with all its parameters, for plain code.
    fn wrap_content_width_with(self, align: HorizontalAlignment, unbounded: bool) -> Modifier {
        self.then_element(WrapContentElement { align: WrapAlign::Horizontal(align), unbounded })
    }

    /// The same for the height (Compose's `wrapContentHeight`).
    fn wrap_content_height(
        self,
        #[default(Alignment::CenterVertically)] align: VerticalAlignment,
        #[default(false)] unbounded: bool,
    ) -> Modifier {
        self.then_element(WrapContentElement { align: WrapAlign::Vertical(align), unbounded })
    }

    /// [`wrap_content_height`](SizeModifiers::wrap_content_height) with all its parameters, for plain code.
    fn wrap_content_height_with(self, align: VerticalAlignment, unbounded: bool) -> Modifier {
        self.then_element(WrapContentElement { align: WrapAlign::Vertical(align), unbounded })
    }

    /// The same both ways (Compose's `wrapContentSize`).
    fn wrap_content_size(
        self,
        #[default(Alignment::Center)] align: Alignment,
        #[default(false)] unbounded: bool,
    ) -> Modifier {
        self.then_element(WrapContentElement { align: WrapAlign::Both(align), unbounded })
    }

    /// [`wrap_content_size`](SizeModifiers::wrap_content_size) with all its parameters, for plain code.
    fn wrap_content_size_with(self, align: Alignment, unbounded: bool) -> Modifier {
        self.then_element(WrapContentElement { align: WrapAlign::Both(align), unbounded })
    }

    /// At least `min_width` by `min_height` where nothing else sets a min (the incoming min is 0), as a component
    /// sets its default size and a caller's own size wins (Compose's `defaultMinSize`).
    fn default_min_size(
        self,
        #[default(Dp::Unspecified)] min_width: Dp,
        #[default(Dp::Unspecified)] min_height: Dp,
    ) -> Modifier {
        self.then_element(UnspecifiedConstraintsElement { min_width, min_height })
    }

    /// [`default_min_size`](SizeModifiers::default_min_size) with all its parameters, for plain code.
    fn default_min_size_with(self, min_width: Dp, min_height: Dp) -> Modifier {
        self.then_element(UnspecifiedConstraintsElement { min_width, min_height })
    }
}

/// Which way a fill or a wrap goes (Compose's Direction).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Direction {
    Vertical,
    Horizontal,
    Both,
}

// ---- size, width, height, …In, required… ----

struct SizeElement {
    min_width: Dp,
    min_height: Dp,
    max_width: Dp,
    max_height: Dp,
    enforce_incoming: bool,
    name: &'static str,
}

impl SizeElement {
    fn new(
        min_width: Dp,
        min_height: Dp,
        max_width: Dp,
        max_height: Dp,
        enforce_incoming: bool,
        name: &'static str,
    ) -> SizeElement {
        SizeElement { min_width, min_height, max_width, max_height, enforce_incoming, name }
    }
}

impl PartialEq for SizeElement {
    fn eq(&self, other: &Self) -> bool {
        same_dp(self.min_width, other.min_width)
            && same_dp(self.min_height, other.min_height)
            && same_dp(self.max_width, other.max_width)
            && same_dp(self.max_height, other.max_height)
            && self.enforce_incoming == other.enforce_incoming
    }
}

impl ModifierNodeElement for SizeElement {
    type Node = SizeNode;
    fn create(&self) -> SizeNode {
        SizeNode {
            min_width: self.min_width,
            min_height: self.min_height,
            max_width: self.max_width,
            max_height: self.max_height,
            enforce_incoming: self.enforce_incoming,
        }
    }
    fn update(&self, node: &mut SizeNode) {
        node.min_width = self.min_width;
        node.min_height = self.min_height;
        node.max_width = self.max_width;
        node.max_height = self.max_height;
        node.enforce_incoming = self.enforce_incoming;
    }
    fn name(&self) -> &'static str {
        self.name
    }
}

/// The node of the size modifiers (Compose's SizeNode): what it wraps measured within its target constraints —
/// brought within the incoming ones (`enforce_incoming`, the preferred sizes) or, for the required ones, the
/// incoming ones taken only for the bounds it doesn't set.
struct SizeNode {
    min_width: Dp,
    min_height: Dp,
    max_width: Dp,
    max_height: Dp,
    enforce_incoming: bool,
}

impl SizeNode {
    /// The constraints its sizes make (Compose's `targetConstraints`): an unspecified max unbounded, an
    /// unspecified min 0, a min at most its max.
    fn target_constraints(&self, scope: &MeasureScope) -> Constraints {
        let max_width = if self.max_width.is_specified() {
            scope.round_to_px(self.max_width).max(0)
        } else {
            Constraints::Infinity
        };
        let max_height = if self.max_height.is_specified() {
            scope.round_to_px(self.max_height).max(0)
        } else {
            Constraints::Infinity
        };
        let min_width = if self.min_width.is_specified() {
            let w = coerce_in(scope.round_to_px(self.min_width), 0, max_width);
            if w != Constraints::Infinity { w } else { 0 }
        } else {
            0
        };
        let min_height = if self.min_height.is_specified() {
            let h = coerce_in(scope.round_to_px(self.min_height), 0, max_height);
            if h != Constraints::Infinity { h } else { 0 }
        } else {
            0
        };
        Constraints::new(min_width, max_width, min_height, max_height)
    }
}

impl ModifierNode for SizeNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for SizeNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let target = self.target_constraints(scope);
        let wrapped = if self.enforce_incoming {
            constraints.constrain(target)
        } else {
            let min_width = if self.min_width.is_specified() {
                target.min_width
            } else {
                constraints.min_width.min(target.max_width)
            };
            let max_width = if self.max_width.is_specified() {
                target.max_width
            } else {
                constraints.max_width.max(target.min_width)
            };
            let min_height = if self.min_height.is_specified() {
                target.min_height
            } else {
                constraints.min_height.min(target.max_height)
            };
            let max_height = if self.max_height.is_specified() {
                target.max_height
            } else {
                constraints.max_height.max(target.min_height)
            };
            Constraints::new(min_width, max_width, min_height, max_height)
        };
        let placeable = measurable.measure(wrapped);
        scope.layout(placeable.width(), placeable.height(), move |p| p.place_relative(&placeable, 0, 0))
    }

    fn min_intrinsic_width(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        let c = self.target_constraints(scope);
        if c.has_fixed_width() {
            c.max_width
        } else {
            let child_height = if self.enforce_incoming { height } else { c.constrain_height(height) };
            c.constrain_width(measurable.min_intrinsic_width(child_height))
        }
    }

    fn min_intrinsic_height(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        let c = self.target_constraints(scope);
        if c.has_fixed_height() {
            c.max_height
        } else {
            let child_width = if self.enforce_incoming { width } else { c.constrain_width(width) };
            c.constrain_height(measurable.min_intrinsic_height(child_width))
        }
    }

    fn max_intrinsic_width(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        let c = self.target_constraints(scope);
        if c.has_fixed_width() {
            c.max_width
        } else {
            let child_height = if self.enforce_incoming { height } else { c.constrain_height(height) };
            c.constrain_width(measurable.max_intrinsic_width(child_height))
        }
    }

    fn max_intrinsic_height(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        let c = self.target_constraints(scope);
        if c.has_fixed_height() {
            c.max_height
        } else {
            let child_width = if self.enforce_incoming { width } else { c.constrain_width(width) };
            c.constrain_height(measurable.max_intrinsic_height(child_width))
        }
    }
}

// ---- fill_max_… ----

#[derive(PartialEq)]
struct FillElement {
    direction: Direction,
    fraction: f32,
}

impl ModifierNodeElement for FillElement {
    type Node = FillNode;
    fn create(&self) -> FillNode {
        FillNode { direction: self.direction, fraction: self.fraction }
    }
    fn update(&self, node: &mut FillNode) {
        node.direction = self.direction;
        node.fraction = self.fraction;
    }
    fn name(&self) -> &'static str {
        match self.direction {
            Direction::Horizontal => "fillMaxWidth",
            Direction::Vertical => "fillMaxHeight",
            Direction::Both => "fillMaxSize",
        }
    }
}

/// The node of `fill_max_width`, `fill_max_height` and `fill_max_size` (Compose's FillNode): a bounded max
/// times its fraction made the exact size that way. Its intrinsic sizes are measured (Compose's default ones).
struct FillNode {
    direction: Direction,
    fraction: f32,
}

impl ModifierNode for FillNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for FillNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let (min_width, max_width) = if constraints.has_bounded_width() && self.direction != Direction::Vertical {
            let width = coerce_in(
                round(constraints.max_width as f32 * self.fraction),
                constraints.min_width,
                constraints.max_width,
            );
            (width, width)
        } else {
            (constraints.min_width, constraints.max_width)
        };
        let (min_height, max_height) = if constraints.has_bounded_height() && self.direction != Direction::Horizontal {
            let height = coerce_in(
                round(constraints.max_height as f32 * self.fraction),
                constraints.min_height,
                constraints.max_height,
            );
            (height, height)
        } else {
            (constraints.min_height, constraints.max_height)
        };
        let placeable = measurable.measure(Constraints::new(min_width, max_width, min_height, max_height));
        scope.layout(placeable.width(), placeable.height(), move |p| p.place_relative(&placeable, 0, 0))
    }
}

// ---- wrap_content_… ----

/// A wrap's alignment: across, down or both (what Compose's WrapContentElement keeps to compare and calls back).
#[derive(Clone, PartialEq, Debug)]
enum WrapAlign {
    Horizontal(HorizontalAlignment),
    Vertical(VerticalAlignment),
    Both(Alignment),
}

impl WrapAlign {
    fn direction(&self) -> Direction {
        match self {
            WrapAlign::Horizontal(_) => Direction::Horizontal,
            WrapAlign::Vertical(_) => Direction::Vertical,
            WrapAlign::Both(_) => Direction::Both,
        }
    }

    /// Where the content goes in the free space `size` (Compose's `alignmentCallback`).
    fn position(&self, size: IntSize, layout_direction: LayoutDirection) -> IntOffset {
        match self {
            WrapAlign::Horizontal(a) => IntOffset::new(a.align(0, size.width, layout_direction), 0),
            WrapAlign::Vertical(a) => IntOffset::new(0, a.align(0, size.height)),
            WrapAlign::Both(a) => a.align(IntSize::new(0, 0), size, layout_direction),
        }
    }
}

#[derive(PartialEq)]
struct WrapContentElement {
    align: WrapAlign,
    unbounded: bool,
}

impl ModifierNodeElement for WrapContentElement {
    type Node = WrapContentNode;
    fn create(&self) -> WrapContentNode {
        WrapContentNode { align: self.align.clone(), unbounded: self.unbounded }
    }
    fn update(&self, node: &mut WrapContentNode) {
        node.align = self.align.clone();
        node.unbounded = self.unbounded;
    }
    fn name(&self) -> &'static str {
        match self.align {
            WrapAlign::Horizontal(_) => "wrapContentWidth",
            WrapAlign::Vertical(_) => "wrapContentHeight",
            WrapAlign::Both(_) => "wrapContentSize",
        }
    }
}

/// The node of the wrap modifiers (Compose's WrapContentNode): what it wraps measured without the incoming min
/// (and max, `unbounded`) its way, the wrapper within the incoming constraints, the content aligned in it. Its
/// intrinsic sizes are measured (Compose's default ones).
struct WrapContentNode {
    align: WrapAlign,
    unbounded: bool,
}

impl ModifierNode for WrapContentNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for WrapContentNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let direction = self.align.direction();
        let wrapped = Constraints::new(
            if direction != Direction::Vertical { 0 } else { constraints.min_width },
            if direction != Direction::Vertical && self.unbounded {
                Constraints::Infinity
            } else {
                constraints.max_width
            },
            if direction != Direction::Horizontal { 0 } else { constraints.min_height },
            if direction != Direction::Horizontal && self.unbounded {
                Constraints::Infinity
            } else {
                constraints.max_height
            },
        );
        let placeable = measurable.measure(wrapped);
        let wrapper_width = coerce_in(placeable.width(), constraints.min_width, constraints.max_width);
        let wrapper_height = coerce_in(placeable.height(), constraints.min_height, constraints.max_height);
        let align = self.align.clone();
        let layout_direction = scope.layout_direction();
        scope.layout(wrapper_width, wrapper_height, move |p| {
            let free = IntSize::new(wrapper_width - placeable.width(), wrapper_height - placeable.height());
            p.place_at(&placeable, align.position(free, layout_direction), 0.0);
        })
    }
}

// ---- default_min_size ----

struct UnspecifiedConstraintsElement {
    min_width: Dp,
    min_height: Dp,
}

impl PartialEq for UnspecifiedConstraintsElement {
    fn eq(&self, other: &Self) -> bool {
        same_dp(self.min_width, other.min_width) && same_dp(self.min_height, other.min_height)
    }
}

impl ModifierNodeElement for UnspecifiedConstraintsElement {
    type Node = UnspecifiedConstraintsNode;
    fn create(&self) -> UnspecifiedConstraintsNode {
        UnspecifiedConstraintsNode { min_width: self.min_width, min_height: self.min_height }
    }
    fn update(&self, node: &mut UnspecifiedConstraintsNode) {
        node.min_width = self.min_width;
        node.min_height = self.min_height;
    }
    fn name(&self) -> &'static str {
        "defaultMinSize"
    }
}

/// The node of `default_min_size` (Compose's UnspecifiedConstraintsNode): its min where the incoming one is 0.
struct UnspecifiedConstraintsNode {
    min_width: Dp,
    min_height: Dp,
}

impl UnspecifiedConstraintsNode {
    fn min_width_px(&self, scope: &MeasureScope) -> i32 {
        if self.min_width.is_specified() { scope.round_to_px(self.min_width) } else { 0 }
    }
    fn min_height_px(&self, scope: &MeasureScope) -> i32 {
        if self.min_height.is_specified() { scope.round_to_px(self.min_height) } else { 0 }
    }
}

impl ModifierNode for UnspecifiedConstraintsNode {
    node_capabilities!(layout);
}

impl LayoutModifierNode for UnspecifiedConstraintsNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let wrapped = Constraints::new(
            if self.min_width.is_specified() && constraints.min_width == 0 {
                coerce_in(scope.round_to_px(self.min_width), 0, constraints.max_width)
            } else {
                constraints.min_width
            },
            constraints.max_width,
            if self.min_height.is_specified() && constraints.min_height == 0 {
                coerce_in(scope.round_to_px(self.min_height), 0, constraints.max_height)
            } else {
                constraints.min_height
            },
            constraints.max_height,
        );
        let placeable = measurable.measure(wrapped);
        scope.layout(placeable.width(), placeable.height(), move |p| p.place_relative(&placeable, 0, 0))
    }

    fn min_intrinsic_width(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        measurable.min_intrinsic_width(height).max(self.min_width_px(scope))
    }
    fn max_intrinsic_width(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        measurable.max_intrinsic_width(height).max(self.min_width_px(scope))
    }
    fn min_intrinsic_height(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        measurable.min_intrinsic_height(width).max(self.min_height_px(scope))
    }
    fn max_intrinsic_height(
        &mut self,
        scope: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        width: i32,
    ) -> i32 {
        measurable.max_intrinsic_height(width).max(self.min_height_px(scope))
    }
}

//! Measuring and placing, as Compose's: a [`Measurable`] measured within [`Constraints`] gives a [`Placeable`]; a
//! [`MeasurePolicy`] measures a layout's children and gives a [`MeasureResult`] — its size, its alignment lines and
//! the block that places the children (kept, and run again when only the placement changes).

use super::{
    AlignmentLine, GraphicsLayerScope, IsRulerProvided, LayerBlock, LayoutCoordinates, Ruler, RulerProvider,
    RulerScope, RulersBlock,
};
use crate::node::NodeCoordinator;
use crate::unit::Constraints;
use kompoz_graphics::{Density, IntOffset, IntSize, LayoutDirection};
use kompoz_runtime::named;
use std::any::Any;
use std::fmt;
use std::rc::Rc;

/// Something whose intrinsic sizes can be asked (Compose's IntrinsicMeasurable): a child of a layout, or what a
/// layout modifier wraps.
pub trait IntrinsicMeasurable {
    /// What its parent data modifiers say about it to its parent (Compose's `parentData`).
    fn parent_data(&self) -> Option<Rc<dyn Any>>;
    /// The least width it can be laid out in at `height` and still show itself well.
    fn min_intrinsic_width(&self, height: i32) -> i32;
    /// The width past which giving it more doesn't make it shorter, at `height`.
    fn max_intrinsic_width(&self, height: i32) -> i32;
    fn min_intrinsic_height(&self, width: i32) -> i32;
    fn max_intrinsic_height(&self, width: i32) -> i32;

    /// Its `Modifier.layout_id`, if it has one (Compose's `Measurable.layoutId`).
    fn layout_id(&self) -> Option<Rc<dyn super::LayoutIdKey>> {
        let data = self.parent_data()?;
        data.downcast_ref::<super::LayoutIdParentData>().map(|d| d.id.clone())
    }
}

/// Something a layout measures (Compose's Measurable). Measure each child once per measure pass.
pub trait Measurable: IntrinsicMeasurable {
    fn measure(&self, constraints: Constraints) -> Placeable;

    /// A handle to measure it later in the same pass, from the placement block (Compose lets a placement block
    /// measure what its measure block was given: a layout that needs what only placing knows, rulers' values, to
    /// pick the constraints); none for what only stands in for it (an intrinsic measurement's).
    fn retain(&self) -> Option<Rc<dyn Measurable>> {
        None
    }
}

/// What a [`Placeable`] places and reads: a layout node, a coordinator, or a fixed size.
pub trait PlaceTarget {
    /// The size it measured itself to (maybe outside the constraints).
    fn measured_size(&self) -> IntSize;
    /// The constraints it was measured with.
    fn measurement_constraints(&self) -> Constraints;
    /// Where `line` is in it, or [`AlignmentLine::Unspecified`].
    fn alignment_line(&self, line: AlignmentLine) -> i32 {
        let _ = line;
        AlignmentLine::Unspecified
    }
    /// Places it at `position` in its parent, above siblings of a lower `z_index`, through a layer if given.
    fn place_at(&self, position: IntOffset, z_index: f32, layer: Option<LayerBlock>);
    fn parent_data(&self) -> Option<Rc<dyn Any>> {
        None
    }
}

/// A measured child, to be placed by its parent's placement block (Compose's Placeable). Its `width` and `height`
/// are its measured size brought within the constraints it was measured with; one that measured itself bigger is
/// centered over that box when placed.
#[derive(Clone)]
pub struct Placeable {
    target: Rc<dyn PlaceTarget>,
}

impl Placeable {
    pub fn new(target: Rc<dyn PlaceTarget>) -> Placeable {
        Placeable { target }
    }

    /// A placeable of a fixed size that places nothing (Compose's EmptyPlaceable), as intrinsic measurements give.
    pub fn empty(width: i32, height: i32) -> Placeable {
        Placeable { target: Rc::new(FixedSize(IntSize::new(width, height))) }
    }

    pub fn width(&self) -> i32 {
        let c = self.target.measurement_constraints();
        self.target.measured_size().width.clamp(c.min_width, c.max_width)
    }
    pub fn height(&self) -> i32 {
        let c = self.target.measurement_constraints();
        self.target.measured_size().height.clamp(c.min_height, c.max_height)
    }
    /// Its size as its parent sees it (within its constraints).
    pub fn size(&self) -> IntSize {
        IntSize::new(self.width(), self.height())
    }
    pub fn measured_width(&self) -> i32 {
        self.target.measured_size().width
    }
    pub fn measured_height(&self) -> i32 {
        self.target.measured_size().height
    }
    pub fn measured_size(&self) -> IntSize {
        self.target.measured_size()
    }
    pub fn measurement_constraints(&self) -> Constraints {
        self.target.measurement_constraints()
    }

    /// Where `line` is in it (from its top for a horizontal line, its left for a vertical one), or
    /// [`AlignmentLine::Unspecified`] (Compose's `placeable[line]`).
    pub fn get(&self, line: impl Into<AlignmentLine>) -> i32 {
        self.target.alignment_line(line.into())
    }

    pub fn parent_data(&self) -> Option<Rc<dyn Any>> {
        self.target.parent_data()
    }

    /// How far its real top left is from where its parent thinks it is (Compose's `apparentToRealOffset`).
    fn apparent_to_real_offset(&self) -> IntOffset {
        let m = self.measured_size();
        IntOffset::new((self.width() - m.width) / 2, (self.height() - m.height) / 2)
    }

    pub(crate) fn place_apparent_to_real_offset(&self, position: IntOffset, z_index: f32, layer: Option<LayerBlock>) {
        self.target.place_at(position + self.apparent_to_real_offset(), z_index, layer);
    }
}

impl fmt::Debug for Placeable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Placeable({}x{})", self.width(), self.height())
    }
}

/// A size and nothing to place (Compose's EmptyPlaceable / FixedSizeIntrinsicsPlaceable).
struct FixedSize(IntSize);

impl PlaceTarget for FixedSize {
    fn measured_size(&self) -> IntSize {
        self.0
    }
    fn measurement_constraints(&self) -> Constraints {
        Constraints::default()
    }
    fn place_at(&self, _: IntOffset, _: f32, _: Option<LayerBlock>) {}
}

// ---------------------------------------------------------------------------------------------------------------
// Scopes and results

/// What a measure policy or a layout modifier measures in (Compose's MeasureScope, and IntrinsicMeasureScope, the
/// same type here): the density (it derefs to it: `scope.round_to_px(16.dp())`), the layout direction, and
/// [`layout`](MeasureScope::layout) to make the result.
#[derive(Clone)]
pub struct MeasureScope {
    pub(crate) density: Density,
    pub(crate) layout_direction: LayoutDirection,
    /// Measuring for an intrinsic size rather than for real (Compose's IntrinsicsMeasureScope).
    pub(crate) intrinsics: bool,
}

/// Compose's IntrinsicMeasureScope: the same as a measure scope here.
pub type IntrinsicMeasureScope = MeasureScope;

impl MeasureScope {
    pub fn new(density: Density, layout_direction: LayoutDirection) -> MeasureScope {
        MeasureScope { density, layout_direction, intrinsics: false }
    }
    pub fn density(&self) -> Density {
        self.density
    }
    pub fn layout_direction(&self) -> LayoutDirection {
        self.layout_direction
    }
    /// Whether this measures for an intrinsic size (a parent asking), not for real.
    pub fn is_intrinsic(&self) -> bool {
        self.intrinsics
    }
    /// The scope an intrinsic measurement measures in (Compose's IntrinsicsMeasureScope).
    pub(crate) fn for_intrinsics(&self) -> MeasureScope {
        MeasureScope { intrinsics: true, ..self.clone() }
    }
}

impl std::ops::Deref for MeasureScope {
    type Target = Density;
    fn deref(&self) -> &Density {
        &self.density
    }
}

#[named]
impl MeasureScope {
    /// The result of a measurement: `width` by `height`, the alignment lines it provides, the rulers it provides
    /// what it places (`rulers`, run as it is placed: Compose's `rulers` parameter), and the block that places what
    /// it measured (run when the layout is placed, and again whenever only its placement changes).
    #[track_caller]
    pub fn layout(
        &self,
        width: i32,
        height: i32,
        #[default(Vec::new())] alignment_lines: Vec<(AlignmentLine, i32)>,
        #[default(None)] rulers: Option<RulersBlock>,
        placement_block: impl Fn(&PlacementScope) + 'static,
    ) -> MeasureResult {
        let mut result = MeasureResult::new(width, height, alignment_lines, placement_block);
        result.rulers = rulers;
        result
    }

    /// The same providing rulers one at a time as they are read (Compose's `layout(width, height,
    /// isRulerProvided, rulerProvider)`): `is_ruler_provided` says which `ruler_provider` may provide, and it is run
    /// for one when it is first read (it may provide others too).
    #[track_caller]
    pub fn layout_with_ruler_provider(
        &self,
        width: i32,
        height: i32,
        is_ruler_provided: impl Fn(&Ruler) -> bool + 'static,
        ruler_provider: impl Fn(&RulerScope, &Ruler) + 'static,
        #[default(Vec::new())] alignment_lines: Vec<(AlignmentLine, i32)>,
        placement_block: impl Fn(&PlacementScope) + 'static,
    ) -> MeasureResult {
        let mut result = MeasureResult::new(width, height, alignment_lines, placement_block);
        result.ruler_provider = Some((Rc::new(is_ruler_provided), Rc::new(ruler_provider)));
        result
    }
}

impl MeasureScope {
    /// `layout` with rulers, from plain code: `rulers` provides them.
    #[track_caller]
    pub fn layout_with_rulers(
        &self,
        width: i32,
        height: i32,
        rulers: impl Fn(&RulerScope) + 'static,
        placement_block: impl Fn(&PlacementScope) + 'static,
    ) -> MeasureResult {
        let mut result = MeasureResult::new(width, height, Vec::new(), placement_block);
        result.rulers = Some(Rc::new(rulers));
        result
    }
}

/// What a measurement gives (Compose's MeasureResult): a size, alignment lines, the rulers it provides (all at
/// once, or one at a time as they are read), and the placement block.
#[derive(Clone)]
pub struct MeasureResult {
    pub width: i32,
    pub height: i32,
    pub(crate) alignment_lines: Rc<Vec<(AlignmentLine, i32)>>,
    pub(crate) rulers: Option<RulersBlock>,
    pub(crate) ruler_provider: Option<(IsRulerProvided, RulerProvider)>,
    place: Rc<dyn Fn(&PlacementScope)>,
}

impl MeasureResult {
    /// Panics on a size Compose can't represent (each side within 0..=16777215).
    #[track_caller]
    pub fn new(
        width: i32,
        height: i32,
        alignment_lines: Vec<(AlignmentLine, i32)>,
        placement_block: impl Fn(&PlacementScope) + 'static,
    ) -> MeasureResult {
        assert!(
            width & !0xFF_FFFF == 0 && height & !0xFF_FFFF == 0,
            "kompoz: Size({width} x {height}) is out of range. Each dimension must be between 0 and 16777215."
        );
        MeasureResult {
            width,
            height,
            alignment_lines: Rc::new(alignment_lines),
            rulers: None,
            ruler_provider: None,
            place: Rc::new(placement_block),
        }
    }

    pub fn alignment_lines(&self) -> &[(AlignmentLine, i32)] {
        &self.alignment_lines
    }

    /// The rulers it provides, if it does all at once.
    pub fn rulers(&self) -> Option<&RulersBlock> {
        self.rulers.as_ref()
    }

    /// Where `line` is, if it provides it.
    pub fn alignment_line(&self, line: AlignmentLine) -> Option<i32> {
        self.alignment_lines.iter().find(|(l, _)| *l == line).map(|(_, v)| *v)
    }

    /// Runs the placement block.
    pub fn place_children(&self, scope: &PlacementScope) {
        (self.place)(scope)
    }

    /// The same result placing by `placement_block` (Compose's `MeasureResult by result` with its own
    /// `placeChildren`).
    pub(crate) fn with_placement(self, placement_block: Rc<dyn Fn(&PlacementScope)>) -> MeasureResult {
        MeasureResult { place: placement_block, ..self }
    }

    pub(crate) fn same_lines(&self, other: &MeasureResult) -> bool {
        Rc::ptr_eq(&self.alignment_lines, &other.alignment_lines) || self.alignment_lines == other.alignment_lines
    }
}

impl fmt::Debug for MeasureResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MeasureResult({}x{}, {:?})", self.width, self.height, self.alignment_lines)
    }
}

/// What a placement block places in (Compose's Placeable.PlacementScope): its parent's width and layout direction,
/// which `place_relative` mirrors by, and the parent's coordinates.
pub struct PlacementScope {
    pub(crate) parent_width: i32,
    pub(crate) parent_layout_direction: LayoutDirection,
    pub(crate) density: Density,
    pub(crate) coordinator: Option<Rc<NodeCoordinator>>,
}

impl PlacementScope {
    /// A scope of a parent `parent_width` wide laid out `parent_layout_direction` (for placing outside a layout:
    /// tests, a host's root).
    pub fn new(parent_width: i32, parent_layout_direction: LayoutDirection) -> PlacementScope {
        PlacementScope { parent_width, parent_layout_direction, density: Density::default(), coordinator: None }
    }

    pub fn parent_width(&self) -> i32 {
        self.parent_width
    }
    pub fn parent_layout_direction(&self) -> LayoutDirection {
        self.parent_layout_direction
    }
    pub fn density(&self) -> Density {
        self.density
    }

    /// The coordinates of the layout placing (Compose's `PlacementScope.coordinates`): asking for them makes the
    /// placement run again whenever the layout moves.
    pub fn coordinates(&self) -> Option<LayoutCoordinates> {
        let c = self.coordinator.as_ref()?;
        if let Some(n) = c.layout_node() {
            n.on_coordinates_accessed_in_placement(c);
        }
        Some(LayoutCoordinates::of(c.clone()))
    }

    /// `ruler`'s value where this places (in the placing layout's coordinates), else `default` (Compose's
    /// `Ruler.current`): a derived ruler works it out; another is the value the nearest layout around providing it
    /// gave, and reading it places this again when that changes. None while placing to work out alignment lines.
    pub fn current(&self, ruler: impl Into<Ruler>, default: f32) -> f32 {
        let ruler = ruler.into();
        if let Some(calculate) = ruler.calculation() {
            return calculate(self, default);
        }
        match &self.coordinator {
            Some(c) => c.find_ruler_value(&ruler, default),
            None => default,
        }
    }

    fn mirrored(&self, placeable: &Placeable, position: IntOffset) -> IntOffset {
        if self.parent_layout_direction == LayoutDirection::Ltr || self.parent_width == 0 {
            position
        } else {
            IntOffset::new(self.parent_width - placeable.width() - position.x, position.y)
        }
    }
}

#[named]
impl PlacementScope {
    /// Places `placeable` with its top left at (`x`, `y`), whatever the layout direction (Compose's `place`).
    pub fn place(&self, placeable: &Placeable, x: i32, y: i32, #[default(0.0)] z_index: f32) {
        placeable.place_apparent_to_real_offset(IntOffset::new(x, y), z_index, None);
    }

    /// The same at `position`, with a z-index.
    pub fn place_at(&self, placeable: &Placeable, position: IntOffset, z_index: f32) {
        placeable.place_apparent_to_real_offset(position, z_index, None);
    }

    /// Places it at (`x`, `y`) from the start: mirrored right to left in a right-to-left parent (Compose's
    /// `placeRelative`).
    pub fn place_relative(&self, placeable: &Placeable, x: i32, y: i32, #[default(0.0)] z_index: f32) {
        let at = self.mirrored(placeable, IntOffset::new(x, y));
        placeable.place_apparent_to_real_offset(at, z_index, None);
    }

    pub fn place_relative_at(&self, placeable: &Placeable, position: IntOffset, z_index: f32) {
        let at = self.mirrored(placeable, position);
        placeable.place_apparent_to_real_offset(at, z_index, None);
    }

    /// Places it through a graphics layer that `layer_block` sets up (run again when what it reads changes,
    /// without placing again: Compose's `placeWithLayer`).
    pub fn place_with_layer(
        &self,
        placeable: &Placeable,
        x: i32,
        y: i32,
        #[default(0.0)] z_index: f32,
        layer_block: impl Fn(&mut GraphicsLayerScope) + 'static,
    ) {
        placeable.place_apparent_to_real_offset(IntOffset::new(x, y), z_index, Some(Rc::new(layer_block)));
    }

    /// The same with a layer block kept elsewhere (so placing again with it isn't taken as a new layer).
    pub fn place_with_layer_block(
        &self,
        placeable: &Placeable,
        position: IntOffset,
        layer_block: LayerBlock,
        z_index: f32,
    ) {
        placeable.place_apparent_to_real_offset(position, z_index, Some(layer_block));
    }

    /// `place_relative` through a layer (Compose's `placeRelativeWithLayer`).
    pub fn place_relative_with_layer(
        &self,
        placeable: &Placeable,
        x: i32,
        y: i32,
        #[default(0.0)] z_index: f32,
        layer_block: impl Fn(&mut GraphicsLayerScope) + 'static,
    ) {
        let at = self.mirrored(placeable, IntOffset::new(x, y));
        placeable.place_apparent_to_real_offset(at, z_index, Some(Rc::new(layer_block)));
    }

    /// `place_relative` with a layer block kept elsewhere.
    pub fn place_relative_with_layer_block(
        &self,
        placeable: &Placeable,
        position: IntOffset,
        layer_block: LayerBlock,
        z_index: f32,
    ) {
        let at = self.mirrored(placeable, position);
        placeable.place_apparent_to_real_offset(at, z_index, Some(layer_block));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Measure policies

/// How a layout measures and places its children (Compose's MeasurePolicy). A closure
/// `|scope, measurables, constraints| scope.layout(…)` is one; its intrinsic sizes then come from measuring as
/// Compose's defaults do (each child answering with its intrinsic size).
pub trait MeasurePolicy: 'static {
    fn measure(&self, scope: &MeasureScope, measurables: &[&dyn Measurable], constraints: Constraints)
    -> MeasureResult;

    fn min_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        default_intrinsic(self, scope, measurables, MinMax::Min, Side::Width, height)
    }
    fn min_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        default_intrinsic(self, scope, measurables, MinMax::Min, Side::Height, width)
    }
    fn max_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        default_intrinsic(self, scope, measurables, MinMax::Max, Side::Width, height)
    }
    fn max_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        default_intrinsic(self, scope, measurables, MinMax::Max, Side::Height, width)
    }
}

impl<F> MeasurePolicy for F
where
    F: Fn(&MeasureScope, &[&dyn Measurable], Constraints) -> MeasureResult + 'static,
{
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        self(scope, measurables, constraints)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MinMax {
    Min,
    Max,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Width,
    Height,
}

/// What stands in for infinity across the side an intrinsic measurement doesn't ask about (Compose's
/// LargeDimension, 2^15 - 1).
pub(crate) const LargeDimension: i32 = (1 << 15) - 1;

/// A child answering a measurement with its intrinsic size (Compose's DefaultIntrinsicMeasurable).
pub(crate) struct DefaultIntrinsicMeasurable<'a> {
    pub(crate) measurable: &'a dyn IntrinsicMeasurable,
    pub(crate) min_max: MinMax,
    pub(crate) side: Side,
}

impl IntrinsicMeasurable for DefaultIntrinsicMeasurable<'_> {
    fn parent_data(&self) -> Option<Rc<dyn Any>> {
        self.measurable.parent_data()
    }
    fn min_intrinsic_width(&self, height: i32) -> i32 {
        self.measurable.min_intrinsic_width(height)
    }
    fn max_intrinsic_width(&self, height: i32) -> i32 {
        self.measurable.max_intrinsic_width(height)
    }
    fn min_intrinsic_height(&self, width: i32) -> i32 {
        self.measurable.min_intrinsic_height(width)
    }
    fn max_intrinsic_height(&self, width: i32) -> i32 {
        self.measurable.max_intrinsic_height(width)
    }
}

impl Measurable for DefaultIntrinsicMeasurable<'_> {
    fn measure(&self, constraints: Constraints) -> Placeable {
        if self.side == Side::Width {
            let width = match self.min_max {
                MinMax::Max => self.measurable.max_intrinsic_width(constraints.max_height),
                MinMax::Min => self.measurable.min_intrinsic_width(constraints.max_height),
            };
            let height = if constraints.has_bounded_height() { constraints.max_height } else { LargeDimension };
            return Placeable::empty(width, height);
        }
        let height = match self.min_max {
            MinMax::Max => self.measurable.max_intrinsic_height(constraints.max_width),
            MinMax::Min => self.measurable.min_intrinsic_height(constraints.max_width),
        };
        let width = if constraints.has_bounded_width() { constraints.max_width } else { LargeDimension };
        Placeable::empty(width, height)
    }
}

/// Compose's default intrinsics of a measure policy: measured with each child answering with its intrinsic size,
/// within `Constraints(maxHeight = height)` (or `maxWidth = width`).
fn default_intrinsic<P: MeasurePolicy + ?Sized>(
    policy: &P,
    scope: &IntrinsicMeasureScope,
    measurables: &[&dyn IntrinsicMeasurable],
    min_max: MinMax,
    side: Side,
    size: i32,
) -> i32 {
    let mapped: Vec<DefaultIntrinsicMeasurable> =
        measurables.iter().map(|m| DefaultIntrinsicMeasurable { measurable: *m, min_max, side }).collect();
    let refs: Vec<&dyn Measurable> = mapped.iter().map(|m| m as &dyn Measurable).collect();
    let constraints = match side {
        Side::Width => Constraints::new(0, Constraints::Infinity, 0, size),
        Side::Height => Constraints::new(0, size, 0, Constraints::Infinity),
    };
    let result = policy.measure(&scope.for_intrinsics(), &refs, constraints);
    match side {
        Side::Width => result.width,
        Side::Height => result.height,
    }
}

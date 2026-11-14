//! `Row` and `Column`, as Compose's: children one after the other along the main axis, weighted children sharing
//! what the others leave, each placed across by its alignment or by an alignment line shared with its siblings
//! (Compose's RowColumnMeasurePolicy and RowColumnImpl, intrinsics included).

use super::arrangement::{HorizontalArrangement, VerticalArrangement};
use super::{coerce_in, round};
use kompoz_graphics::LayoutDirection;
use kompoz_runtime::{kompozable, remember_keyed};
use kompoz_ui::layout::{AlignmentLine, layout_into};
use kompoz_ui::{
    Alignment, Constraints, HorizontalAlignment, IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasurePolicy,
    MeasureResult, MeasureScope, Modifier, Placeable, VerticalAlignment,
};
use std::any::Any;
use std::rc::Rc;

/// A layout of its children side by side (Compose's `Row`): `horizontal_arrangement` puts them along (mirrored
/// right to left), `vertical_alignment` across, unless a child says otherwise (`Modifier.align`, `align_by`,
/// `align_by_baseline`); children with a `Modifier.weight` share what the others leave, by their weights.
#[kompozable(inline)]
pub fn Row(
    #[default(Modifier)] modifier: Modifier,
    #[default(super::Arrangement::Start)] horizontal_arrangement: HorizontalArrangement,
    #[default(Alignment::Top)] vertical_alignment: VerticalAlignment,
    content: impl FnOnce(),
) {
    let policy = row_measure_policy(horizontal_arrangement, vertical_alignment);
    layout_into(modifier, policy, false, content);
}

/// A layout of its children one under the other (Compose's `Column`): `vertical_arrangement` puts them along,
/// `horizontal_alignment` across (mirrored right to left), unless a child says otherwise (`Modifier.align`,
/// `align_by`); children with a `Modifier.weight` share what the others leave, by their weights.
#[kompozable(inline)]
pub fn Column(
    #[default(Modifier)] modifier: Modifier,
    #[default(super::Arrangement::Top)] vertical_arrangement: VerticalArrangement,
    #[default(Alignment::Start)] horizontal_alignment: HorizontalAlignment,
    content: impl FnOnce(),
) {
    let policy = column_measure_policy(vertical_arrangement, horizontal_alignment);
    layout_into(modifier, policy, false, content);
}

/// A Row's policy, kept while its arguments are equal (Compose's `rowMeasurePolicy`).
#[track_caller]
fn row_measure_policy(arrangement: HorizontalArrangement, alignment: VerticalAlignment) -> Rc<dyn MeasurePolicy> {
    remember_keyed((arrangement.clone(), alignment.clone()), move || {
        Rc::new(RowMeasurePolicy { horizontal_arrangement: arrangement, vertical_alignment: alignment })
            as Rc<dyn MeasurePolicy>
    })
}

/// A Column's policy, kept while its arguments are equal (Compose's `columnMeasurePolicy`).
#[track_caller]
fn column_measure_policy(arrangement: VerticalArrangement, alignment: HorizontalAlignment) -> Rc<dyn MeasurePolicy> {
    remember_keyed((arrangement.clone(), alignment.clone()), move || {
        Rc::new(ColumnMeasurePolicy { vertical_arrangement: arrangement, horizontal_alignment: alignment })
            as Rc<dyn MeasurePolicy>
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Parent data

/// What a child of a Row or a Column tells it (Compose's RowColumnParentData): its weight, whether it fills the
/// space its weight gives it, and how it is placed across.
#[derive(Clone, Debug)]
pub(crate) struct RowColumnParentData {
    pub(crate) weight: f32,
    pub(crate) fill: bool,
    pub(crate) cross_axis_alignment: Option<CrossAxisAlignment>,
}

impl Default for RowColumnParentData {
    fn default() -> Self {
        RowColumnParentData { weight: 0.0, fill: true, cross_axis_alignment: None }
    }
}

impl RowColumnParentData {
    /// What the nodes after a parent data node said, if it is a Row's or a Column's, else a new one (Compose's
    /// `(parentData as? RowColumnParentData) ?: RowColumnParentData()`).
    pub(crate) fn from(parent_data: Option<std::boxed::Box<dyn Any>>) -> RowColumnParentData {
        parent_data.and_then(|d| d.downcast::<RowColumnParentData>().ok()).map(|d| *d).unwrap_or_default()
    }
}

fn parent_data_of(measurable: &dyn IntrinsicMeasurable) -> Option<RowColumnParentData> {
    measurable.parent_data()?.downcast_ref::<RowColumnParentData>().cloned()
}

fn weight_of(measurable: &dyn IntrinsicMeasurable) -> f32 {
    parent_data_of(measurable).map_or(0.0, |d| d.weight)
}

/// How a child is placed across a Row or a Column (Compose's CrossAxisAlignment).
#[derive(Clone)]
pub(crate) enum CrossAxisAlignment {
    /// A Row's child's `align`.
    Vertical(VerticalAlignment),
    /// A Column's child's `align`.
    Horizontal(HorizontalAlignment),
    /// `align_by`: its line lined up with its siblings'.
    AlignmentLine(AlignmentLineProvider),
}

impl std::fmt::Debug for CrossAxisAlignment {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CrossAxisAlignment::Vertical(a) => write!(f, "{a:?}"),
            CrossAxisAlignment::Horizontal(a) => write!(f, "{a:?}"),
            CrossAxisAlignment::AlignmentLine(AlignmentLineProvider::Value(l)) => write!(f, "AlignmentLine({l:?})"),
            CrossAxisAlignment::AlignmentLine(AlignmentLineProvider::Block(_)) => write!(f, "AlignmentLine(block)"),
        }
    }
}

/// Where a child's line is (Compose's AlignmentLineProvider): an alignment line it provides, or what a block
/// works out from it.
#[derive(Clone)]
pub(crate) enum AlignmentLineProvider {
    Value(AlignmentLine),
    Block(Rc<dyn Fn(&Placeable) -> i32>),
}

impl AlignmentLineProvider {
    fn calculate_alignment_line_position(&self, placeable: &Placeable) -> i32 {
        match self {
            AlignmentLineProvider::Value(line) => placeable.get(*line),
            AlignmentLineProvider::Block(block) => block(placeable),
        }
    }
}

impl CrossAxisAlignment {
    fn is_relative(&self) -> bool {
        matches!(self, CrossAxisAlignment::AlignmentLine(_))
    }

    fn calculate_alignment_line_position(&self, placeable: &Placeable) -> Option<i32> {
        match self {
            CrossAxisAlignment::AlignmentLine(provider) => Some(provider.calculate_alignment_line_position(placeable)),
            _ => None,
        }
    }

    /// Where the child goes across, in `size` (Compose's `CrossAxisAlignment.align`).
    fn align(
        &self,
        size: i32,
        item_cross_axis_size: i32,
        layout_direction: LayoutDirection,
        placeable: &Placeable,
        before_cross_axis_alignment_line: i32,
    ) -> i32 {
        match self {
            CrossAxisAlignment::Vertical(v) => v.align(item_cross_axis_size, size),
            CrossAxisAlignment::Horizontal(h) => h.align(item_cross_axis_size, size, layout_direction),
            CrossAxisAlignment::AlignmentLine(provider) => {
                let position = provider.calculate_alignment_line_position(placeable);
                if position != AlignmentLine::Unspecified {
                    let line = before_cross_axis_alignment_line - position;
                    if layout_direction == LayoutDirection::Rtl { size - item_cross_axis_size - line } else { line }
                } else {
                    0
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Measuring (Compose's RowColumnMeasurePolicy.measure)

/// What a Row and a Column do their own way (Compose's RowColumnMeasurePolicy).
trait RowColumnMeasurePolicy {
    fn main_axis_size(&self, placeable: &Placeable) -> i32;
    fn cross_axis_size(&self, placeable: &Placeable) -> i32;
    fn populate_main_axis_positions(
        &self,
        main_axis_layout_size: i32,
        children_main_axis_size: &[i32],
        main_axis_positions: &mut [i32],
        scope: &MeasureScope,
    );
    #[allow(clippy::too_many_arguments)]
    fn place_helper(
        &self,
        placeables: Vec<Placeable>,
        datas: Vec<Option<RowColumnParentData>>,
        scope: &MeasureScope,
        before_cross_axis_alignment_line: i32,
        main_axis_positions: Vec<i32>,
        main_axis_layout_size: i32,
        cross_axis_layout_size: i32,
    ) -> MeasureResult;
    fn create_constraints(
        &self,
        main_axis_min: i32,
        cross_axis_min: i32,
        main_axis_max: i32,
        cross_axis_max: i32,
        is_prioritizing: bool,
    ) -> Constraints;
}

#[allow(clippy::too_many_arguments)]
fn measure_row_column<P: RowColumnMeasurePolicy>(
    policy: &P,
    main_axis_min: i32,
    cross_axis_min: i32,
    main_axis_max: i32,
    cross_axis_max: i32,
    arrangement_spacing_int: i32,
    scope: &MeasureScope,
    measurables: &[&dyn Measurable],
) -> MeasureResult {
    let arrangement_spacing_px = arrangement_spacing_int as i64;
    let n = measurables.len();
    let datas: Vec<Option<RowColumnParentData>> = measurables.iter().map(|m| parent_data_of(*m)).collect();
    let weight = |i: usize| datas[i].as_ref().map_or(0.0, |d| d.weight);

    let mut total_weight = 0f32;
    let mut fixed_space = 0i32;
    let mut cross_axis_space = 0i32;
    let mut weight_children_count = 0i64;
    let mut any_align_by = false;
    let mut children_main_axis_size = vec![0i32; n];
    let mut placeables: Vec<Option<Placeable>> = vec![None; n];
    let mut before_cross_axis_alignment_line = 0;
    let mut after_cross_axis_alignment_line = 0;
    // The children without a weight first.
    let mut space_after_last_no_weight = 0;
    for i in 0..n {
        let w = weight(i);
        any_align_by = any_align_by
            || datas[i].as_ref().and_then(|d| d.cross_axis_alignment.as_ref()).is_some_and(|a| a.is_relative());
        if w > 0.0 {
            total_weight += w;
            weight_children_count += 1;
        } else {
            let remaining = main_axis_max - fixed_space;
            // Their preferred size along.
            let placeable = measurables[i].measure(policy.create_constraints(
                0,
                0,
                if main_axis_max == Constraints::Infinity { Constraints::Infinity } else { remaining.max(0) },
                cross_axis_max,
                false,
            ));
            let main = policy.main_axis_size(&placeable);
            let cross = policy.cross_axis_size(&placeable);
            children_main_axis_size[i] = main;
            space_after_last_no_weight = arrangement_spacing_int.min((remaining - main).max(0));
            fixed_space += main + space_after_last_no_weight;
            cross_axis_space = cross_axis_space.max(cross);
            placeables[i] = Some(placeable);
        }
    }

    let mut weighted_space = 0i32;
    if weight_children_count == 0 {
        // (fixed_space has a spacing after the last child)
        fixed_space -= space_after_last_no_weight;
    } else {
        // The weighted ones share what is left, by their weights.
        let target_space = if main_axis_max != Constraints::Infinity { main_axis_max } else { main_axis_min };
        let arrangement_spacing_total = arrangement_spacing_px * (weight_children_count - 1);
        let remaining_to_target = (target_space as i64 - fixed_space as i64 - arrangement_spacing_total).max(0);
        let weight_unit_space = remaining_to_target as f32 / total_weight;
        let mut remainder = remaining_to_target;
        for i in 0..n {
            remainder -= round(weight_unit_space * weight(i)) as i64;
        }
        let mut weighted = 0i64;
        for i in 0..n {
            if placeables[i].is_some() {
                continue;
            }
            let w = weight(i);
            let fill = datas[i].as_ref().is_none_or(|d| d.fill);
            // What rounding took or gave, spread one by one over the first children.
            let remainder_unit = remainder.signum();
            remainder -= remainder_unit;
            let child_main_axis_size = 0.max(round(weight_unit_space * w) + remainder_unit as i32);
            let child_constraints = policy.create_constraints(
                if fill && child_main_axis_size != Constraints::Infinity { child_main_axis_size } else { 0 },
                0,
                child_main_axis_size,
                cross_axis_max,
                true,
            );
            let placeable = measurables[i].measure(child_constraints);
            let main = policy.main_axis_size(&placeable);
            let cross = policy.cross_axis_size(&placeable);
            children_main_axis_size[i] = main;
            weighted += main as i64;
            cross_axis_space = cross_axis_space.max(cross);
            placeables[i] = Some(placeable);
        }
        weighted_space = coerce_in((weighted + arrangement_spacing_total) as i32, 0, main_axis_max - fixed_space);
    }
    let placeables: Vec<Placeable> = placeables.into_iter().map(|p| p.expect("measured")).collect();

    if any_align_by {
        for (placeable, data) in placeables.iter().zip(&datas) {
            let Some(alignment) = data.as_ref().and_then(|d| d.cross_axis_alignment.as_ref()) else { continue };
            let Some(position) = alignment.calculate_alignment_line_position(placeable) else { continue };
            let cross = policy.cross_axis_size(placeable);
            let specified = position != AlignmentLine::Unspecified;
            before_cross_axis_alignment_line =
                before_cross_axis_alignment_line.max(if specified { position } else { 0 });
            after_cross_axis_alignment_line =
                after_cross_axis_alignment_line.max(cross - if specified { position } else { cross });
        }
    }

    // The layout's size, and where the children go.
    let main_axis_layout_size = (fixed_space + weighted_space).max(0).max(main_axis_min);
    let cross_axis_layout_size =
        cross_axis_space.max(cross_axis_min).max(before_cross_axis_alignment_line + after_cross_axis_alignment_line);
    let mut main_axis_positions = vec![0i32; n];
    policy.populate_main_axis_positions(
        main_axis_layout_size,
        &children_main_axis_size,
        &mut main_axis_positions,
        scope,
    );
    policy.place_helper(
        placeables,
        datas,
        scope,
        before_cross_axis_alignment_line,
        main_axis_positions,
        main_axis_layout_size,
        cross_axis_layout_size,
    )
}

// ---------------------------------------------------------------------------------------------------------------
// Row

/// Compose's RowMeasurePolicy.
struct RowMeasurePolicy {
    horizontal_arrangement: HorizontalArrangement,
    vertical_alignment: VerticalAlignment,
}

impl RowColumnMeasurePolicy for RowMeasurePolicy {
    fn main_axis_size(&self, placeable: &Placeable) -> i32 {
        placeable.width()
    }
    fn cross_axis_size(&self, placeable: &Placeable) -> i32 {
        placeable.height()
    }

    fn populate_main_axis_positions(
        &self,
        main_axis_layout_size: i32,
        children_main_axis_size: &[i32],
        main_axis_positions: &mut [i32],
        scope: &MeasureScope,
    ) {
        self.horizontal_arrangement.arrange(
            scope,
            main_axis_layout_size,
            children_main_axis_size,
            scope.layout_direction(),
            main_axis_positions,
        );
    }

    fn place_helper(
        &self,
        placeables: Vec<Placeable>,
        datas: Vec<Option<RowColumnParentData>>,
        scope: &MeasureScope,
        before_cross_axis_alignment_line: i32,
        main_axis_positions: Vec<i32>,
        main_axis_layout_size: i32,
        cross_axis_layout_size: i32,
    ) -> MeasureResult {
        let vertical_alignment = self.vertical_alignment.clone();
        scope.layout(main_axis_layout_size, cross_axis_layout_size, move |p| {
            for (i, placeable) in placeables.iter().enumerate() {
                let cross_axis_position = match datas[i].as_ref().and_then(|d| d.cross_axis_alignment.as_ref()) {
                    Some(alignment) => alignment.align(
                        cross_axis_layout_size,
                        placeable.height(),
                        LayoutDirection::Ltr,
                        placeable,
                        before_cross_axis_alignment_line,
                    ),
                    None => vertical_alignment.align(placeable.height(), cross_axis_layout_size),
                };
                p.place(placeable, main_axis_positions[i], cross_axis_position);
            }
        })
    }

    fn create_constraints(
        &self,
        main_axis_min: i32,
        cross_axis_min: i32,
        main_axis_max: i32,
        cross_axis_max: i32,
        is_prioritizing: bool,
    ) -> Constraints {
        if !is_prioritizing {
            Constraints::new(main_axis_min, main_axis_max, cross_axis_min, cross_axis_max)
        } else {
            Constraints::fit_prioritizing_width(main_axis_min, main_axis_max, cross_axis_min, cross_axis_max)
        }
    }
}

impl MeasurePolicy for RowMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        measure_row_column(
            self,
            constraints.min_width,
            constraints.min_height,
            constraints.max_width,
            constraints.max_height,
            scope.round_to_px(self.horizontal_arrangement.spacing()),
            scope,
            measurables,
        )
    }

    fn min_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        let spacing = scope.round_to_px(self.horizontal_arrangement.spacing());
        intrinsic_main_axis_size(measurables, |m, h| m.min_intrinsic_width(h), height, spacing)
    }
    fn min_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        let spacing = scope.round_to_px(self.horizontal_arrangement.spacing());
        intrinsic_cross_axis_size(
            measurables,
            |m, h| m.max_intrinsic_width(h),
            |m, w| m.min_intrinsic_height(w),
            width,
            spacing,
        )
    }
    fn max_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        let spacing = scope.round_to_px(self.horizontal_arrangement.spacing());
        intrinsic_main_axis_size(measurables, |m, h| m.max_intrinsic_width(h), height, spacing)
    }
    fn max_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        let spacing = scope.round_to_px(self.horizontal_arrangement.spacing());
        intrinsic_cross_axis_size(
            measurables,
            |m, h| m.max_intrinsic_width(h),
            |m, w| m.max_intrinsic_height(w),
            width,
            spacing,
        )
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Column

/// Compose's ColumnMeasurePolicy.
struct ColumnMeasurePolicy {
    vertical_arrangement: VerticalArrangement,
    horizontal_alignment: HorizontalAlignment,
}

impl RowColumnMeasurePolicy for ColumnMeasurePolicy {
    fn main_axis_size(&self, placeable: &Placeable) -> i32 {
        placeable.height()
    }
    fn cross_axis_size(&self, placeable: &Placeable) -> i32 {
        placeable.width()
    }

    fn populate_main_axis_positions(
        &self,
        main_axis_layout_size: i32,
        children_main_axis_size: &[i32],
        main_axis_positions: &mut [i32],
        scope: &MeasureScope,
    ) {
        self.vertical_arrangement.arrange(scope, main_axis_layout_size, children_main_axis_size, main_axis_positions);
    }

    fn place_helper(
        &self,
        placeables: Vec<Placeable>,
        datas: Vec<Option<RowColumnParentData>>,
        scope: &MeasureScope,
        before_cross_axis_alignment_line: i32,
        main_axis_positions: Vec<i32>,
        main_axis_layout_size: i32,
        cross_axis_layout_size: i32,
    ) -> MeasureResult {
        let horizontal_alignment = self.horizontal_alignment.clone();
        let layout_direction = scope.layout_direction();
        scope.layout(cross_axis_layout_size, main_axis_layout_size, move |p| {
            for (i, placeable) in placeables.iter().enumerate() {
                let cross_axis_position = match datas[i].as_ref().and_then(|d| d.cross_axis_alignment.as_ref()) {
                    Some(alignment) => alignment.align(
                        cross_axis_layout_size,
                        placeable.width(),
                        layout_direction,
                        placeable,
                        before_cross_axis_alignment_line,
                    ),
                    None => horizontal_alignment.align(placeable.width(), cross_axis_layout_size, layout_direction),
                };
                p.place(placeable, cross_axis_position, main_axis_positions[i]);
            }
        })
    }

    fn create_constraints(
        &self,
        main_axis_min: i32,
        cross_axis_min: i32,
        main_axis_max: i32,
        cross_axis_max: i32,
        is_prioritizing: bool,
    ) -> Constraints {
        if !is_prioritizing {
            Constraints::new(cross_axis_min, cross_axis_max, main_axis_min, main_axis_max)
        } else {
            Constraints::fit_prioritizing_height(cross_axis_min, cross_axis_max, main_axis_min, main_axis_max)
        }
    }
}

impl MeasurePolicy for ColumnMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        measure_row_column(
            self,
            constraints.min_height,
            constraints.min_width,
            constraints.max_height,
            constraints.max_width,
            scope.round_to_px(self.vertical_arrangement.spacing()),
            scope,
            measurables,
        )
    }

    fn min_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        let spacing = scope.round_to_px(self.vertical_arrangement.spacing());
        intrinsic_cross_axis_size(
            measurables,
            |m, w| m.max_intrinsic_height(w),
            |m, h| m.min_intrinsic_width(h),
            height,
            spacing,
        )
    }
    fn min_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        let spacing = scope.round_to_px(self.vertical_arrangement.spacing());
        intrinsic_main_axis_size(measurables, |m, w| m.min_intrinsic_height(w), width, spacing)
    }
    fn max_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        let spacing = scope.round_to_px(self.vertical_arrangement.spacing());
        intrinsic_cross_axis_size(
            measurables,
            |m, w| m.max_intrinsic_height(w),
            |m, h| m.max_intrinsic_width(h),
            height,
            spacing,
        )
    }
    fn max_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        let spacing = scope.round_to_px(self.vertical_arrangement.spacing());
        intrinsic_main_axis_size(measurables, |m, w| m.max_intrinsic_height(w), width, spacing)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Intrinsics (Compose's IntrinsicMeasureBlocks)

/// The intrinsic size along the main axis: the children's added up, the weighted ones as big as needed for the
/// one wanting the most per unit of weight to get what it wants, and the spacing between them.
fn intrinsic_main_axis_size(
    children: &[&dyn IntrinsicMeasurable],
    main_axis_size: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
    cross_axis_available: i32,
    main_axis_spacing: i32,
) -> i32 {
    if children.is_empty() {
        return 0;
    }
    let mut weight_unit_space = 0;
    let mut fixed_space = 0;
    let mut total_weight = 0f32;
    for child in children {
        let weight = weight_of(*child);
        let size = main_axis_size(*child, cross_axis_available);
        if weight == 0.0 {
            fixed_space += size;
        } else if weight > 0.0 {
            total_weight += weight;
            weight_unit_space = weight_unit_space.max(round(size as f32 / weight));
        }
    }
    round(weight_unit_space as f32 * total_weight) + fixed_space + (children.len() as i32 - 1) * main_axis_spacing
}

/// The intrinsic size across: the biggest child's across, each given the main axis space it would get (the
/// fixed ones what they want of what is available, the weighted ones their share of the rest).
fn intrinsic_cross_axis_size(
    children: &[&dyn IntrinsicMeasurable],
    main_axis_size: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
    cross_axis_size: impl Fn(&dyn IntrinsicMeasurable, i32) -> i32,
    main_axis_available: i32,
    main_axis_spacing: i32,
) -> i32 {
    if children.is_empty() {
        return 0;
    }
    let mut fixed_space = ((children.len() as i32 - 1) * main_axis_spacing).min(main_axis_available);
    let mut cross_axis_max = 0;
    let mut total_weight = 0f32;
    for child in children {
        let weight = weight_of(*child);
        if weight == 0.0 {
            // What it wants along, within what is left; then what it wants across at that.
            let remaining = if main_axis_available == Constraints::Infinity {
                Constraints::Infinity
            } else {
                main_axis_available - fixed_space
            };
            let main_axis_space = main_axis_size(*child, Constraints::Infinity).min(remaining);
            fixed_space += main_axis_space;
            cross_axis_max = cross_axis_max.max(cross_axis_size(*child, main_axis_space));
        } else if weight > 0.0 {
            total_weight += weight;
        }
    }
    // What a weight of 1 gets along.
    let weight_unit_space = if total_weight == 0.0 {
        0
    } else if main_axis_available == Constraints::Infinity {
        Constraints::Infinity
    } else {
        round((main_axis_available - fixed_space).max(0) as f32 / total_weight)
    };
    for child in children {
        let weight = weight_of(*child);
        if weight > 0.0 {
            let main = if weight_unit_space != Constraints::Infinity {
                round(weight_unit_space as f32 * weight)
            } else {
                Constraints::Infinity
            };
            cross_axis_max = cross_axis_max.max(cross_axis_size(*child, main));
        }
    }
    cross_axis_max
}

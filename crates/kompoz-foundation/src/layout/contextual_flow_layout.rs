//! `ContextualFlowRow` and `ContextualFlowColumn`, as Compose's ContextualFlowLayout.kt: flow layouts of
//! `item_count` items composed as they are measured (a `SubcomposeLayout`), each knowing the line it goes in and
//! the room left there, only as many as show; their overflow indicators composed knowing how many show.

use super::Arrangement;
use super::arrangement::{HorizontalArrangement, VerticalArrangement};
use super::flow_layout::{
    ContextualFlowColumnOverflow, ContextualFlowRowOverflow, FlowItems, FlowLayoutOverflowState, FlowLine,
    FlowLineInfo, break_down_items, remember_overflow_state,
};
use super::orientation::{LayoutOrientation, OrientationIndependentConstraints};
use super::row_column::CrossAxisAlignment;
use kompoz_graphics::Dp;
use kompoz_runtime::{kompozable, remember_keyed};
use kompoz_ui::layout::{SubcomposeLayout, SubcomposeMeasureScope};
use kompoz_ui::{Alignment, Constraints, HorizontalAlignment, Measurable, MeasureResult, Modifier, VerticalAlignment};
use std::rc::Rc;

/// A flow layout of `item_count` items in rows, each composed as it is measured (Compose's deprecated
/// `ContextualFlowRow`): `content(scope, index)` is composed for an item knowing its row and where in it it goes
/// and the room left ([`ContextualFlowRowScope`]); only the items that show are composed. Otherwise a `FlowRow`.
/// Asking it for an intrinsic size panics (it is a `SubcomposeLayout`).
#[kompozable]
pub fn ContextualFlowRow(
    item_count: i32,
    #[default(Modifier)] modifier: Modifier,
    #[default(Arrangement::Start)] horizontal_arrangement: HorizontalArrangement,
    #[default(Arrangement::Top)] vertical_arrangement: VerticalArrangement,
    #[default(Alignment::Top)] item_vertical_alignment: VerticalAlignment,
    #[default(i32::MAX)] max_items_in_each_row: i32,
    #[default(i32::MAX)] max_lines: i32,
    #[default(ContextualFlowRowOverflow::Clip)] overflow: ContextualFlowRowOverflow,
    content: impl Fn(&ContextualFlowRowScope, i32) + 'static,
) {
    let overflow_state = remember_overflow_state(&overflow.0);
    let list = overflow_composables(&overflow.0, &overflow_state);
    let content = Rc::new(content);
    let get_composable: Rc<dyn Fn(i32, FlowLineInfo)> = Rc::new(move |index, info: FlowLineInfo| {
        let scope = ContextualFlowRowScope {
            line_index: info.line_index,
            index_in_line: info.position_in_line,
            max_width_in_line: info.max_main_axis_size,
            max_height: info.max_cross_axis_size,
        };
        content(&scope, index);
    });
    // (Compose remembers the policy by its arguments and the content's lambda: this body runs again when one of
    // them changes, or the modifier does, which measures it again here)
    let policy = Rc::new(FlowMeasureLazyPolicy {
        line: FlowLine {
            is_horizontal: true,
            horizontal_arrangement: horizontal_arrangement.clone(),
            vertical_arrangement: vertical_arrangement.clone(),
            cross_axis_alignment: CrossAxisAlignment::Vertical(item_vertical_alignment),
        },
        main_axis_spacing: horizontal_arrangement.spacing(),
        cross_axis_arrangement_spacing: vertical_arrangement.spacing(),
        item_count,
        max_lines,
        max_items_in_main_axis: max_items_in_each_row,
        overflow: overflow_state,
        overflow_composables: list,
        get_composable,
    });
    SubcomposeLayout(
        modifier = modifier,
        measure_policy =
            move |scope: &SubcomposeMeasureScope, constraints: Constraints| policy.measure_lazy(scope, constraints),
    );
}

/// A flow layout of `item_count` items in columns, each composed as it is measured (Compose's deprecated
/// `ContextualFlowColumn`): `content(scope, index)` is composed for an item knowing its column and where in it it
/// goes and the room left ([`ContextualFlowColumnScope`]). Otherwise a `FlowColumn`. Asking it for an intrinsic
/// size panics (it is a `SubcomposeLayout`).
#[kompozable]
pub fn ContextualFlowColumn(
    item_count: i32,
    #[default(Modifier)] modifier: Modifier,
    #[default(Arrangement::Top)] vertical_arrangement: VerticalArrangement,
    #[default(Arrangement::Start)] horizontal_arrangement: HorizontalArrangement,
    #[default(Alignment::Start)] item_horizontal_alignment: HorizontalAlignment,
    #[default(i32::MAX)] max_items_in_each_column: i32,
    #[default(i32::MAX)] max_lines: i32,
    #[default(ContextualFlowColumnOverflow::Clip)] overflow: ContextualFlowColumnOverflow,
    content: impl Fn(&ContextualFlowColumnScope, i32) + 'static,
) {
    let overflow_state = remember_overflow_state(&overflow.0);
    let list = overflow_composables(&overflow.0, &overflow_state);
    let content = Rc::new(content);
    let get_composable: Rc<dyn Fn(i32, FlowLineInfo)> = Rc::new(move |index, info: FlowLineInfo| {
        let scope = ContextualFlowColumnScope {
            line_index: info.line_index,
            index_in_line: info.position_in_line,
            max_width: info.max_cross_axis_size,
            max_height_in_line: info.max_main_axis_size,
        };
        content(&scope, index);
    });
    // (see ContextualFlowRow)
    let policy = Rc::new(FlowMeasureLazyPolicy {
        line: FlowLine {
            is_horizontal: false,
            horizontal_arrangement: horizontal_arrangement.clone(),
            vertical_arrangement: vertical_arrangement.clone(),
            cross_axis_alignment: CrossAxisAlignment::Horizontal(item_horizontal_alignment),
        },
        main_axis_spacing: vertical_arrangement.spacing(),
        cross_axis_arrangement_spacing: horizontal_arrangement.spacing(),
        item_count,
        max_lines,
        max_items_in_main_axis: max_items_in_each_column,
        overflow: overflow_state,
        overflow_composables: list,
        get_composable,
    });
    SubcomposeLayout(
        modifier = modifier,
        measure_policy =
            move |scope: &SubcomposeMeasureScope, constraints: Constraints| policy.measure_lazy(scope, constraints),
    );
}

/// The overflow's indicators' contents, made again when the overflow changes (Compose's `remember(overflow)`).
#[track_caller]
fn overflow_composables(
    overflow: &super::flow_layout::FlowLayoutOverflow,
    state: &Rc<FlowLayoutOverflowState>,
) -> Vec<Rc<dyn Fn()>> {
    let (o, s) = (overflow.clone(), state.clone());
    remember_keyed(overflow.clone(), move || {
        let mut list = Vec::new();
        o.add_overflow_composables(&s, &mut list);
        list
    })
}

/// What a `ContextualFlowRow`'s item is composed in (Compose's `ContextualFlowRowScope`, a `RowScope` with
/// `fill_max_row_height`): the row it goes in, where in it, and the room left there.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ContextualFlowRowScope {
    line_index: i32,
    index_in_line: i32,
    max_width_in_line: Dp,
    max_height: Dp,
}

impl ContextualFlowRowScope {
    /// The row it goes in, from 0 (Compose's `lineIndex`).
    pub fn line_index(&self) -> i32 {
        self.line_index
    }
    /// Where in its row it goes, from 0 (Compose's `indexInLine`).
    pub fn index_in_line(&self) -> i32 {
        self.index_in_line
    }
    /// The width left in its row, spacing taken off (Compose's `maxWidthInLine`).
    pub fn max_width_in_line(&self) -> Dp {
        self.max_width_in_line
    }
    /// The height left in the layout (Compose's `maxHeight`).
    pub fn max_height(&self) -> Dp {
        self.max_height
    }
}

/// What a `ContextualFlowColumn`'s item is composed in (Compose's `ContextualFlowColumnScope`, a `ColumnScope`
/// with `fill_max_column_width`): the column it goes in, where in it, and the room left there.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ContextualFlowColumnScope {
    line_index: i32,
    index_in_line: i32,
    max_width: Dp,
    max_height_in_line: Dp,
}

impl ContextualFlowColumnScope {
    /// The column it goes in, from 0 (Compose's `lineIndex`).
    pub fn line_index(&self) -> i32 {
        self.line_index
    }
    /// Where in its column it goes, from 0 (Compose's `indexInLine`).
    pub fn index_in_line(&self) -> i32 {
        self.index_in_line
    }
    /// The width left in the layout (Compose's `maxWidth`).
    pub fn max_width(&self) -> Dp {
        self.max_width
    }
    /// The height left in its column, spacing taken off (Compose's `maxHeightInLine`).
    pub fn max_height_in_line(&self) -> Dp {
        self.max_height_in_line
    }
}

/// A contextual flow layout's policy (Compose's FlowMeasureLazyPolicy): its items subcomposed as they are broken
/// into lines, its indicators subcomposed for the count that shows.
struct FlowMeasureLazyPolicy {
    line: FlowLine,
    main_axis_spacing: Dp,
    cross_axis_arrangement_spacing: Dp,
    item_count: i32,
    max_lines: i32,
    max_items_in_main_axis: i32,
    overflow: Rc<FlowLayoutOverflowState>,
    overflow_composables: Vec<Rc<dyn Fn()>>,
    get_composable: Rc<dyn Fn(i32, FlowLineInfo)>,
}

impl FlowMeasureLazyPolicy {
    fn measure_lazy(&self, scope: &SubcomposeMeasureScope, constraints: Constraints) -> MeasureResult {
        if self.item_count <= 0
            || (self.max_lines == 0
                || self.max_items_in_main_axis == 0
                || constraints.max_height == 0 && self.overflow.kind != super::flow_layout::OverflowType::Visible)
        {
            return scope.layout(0, 0, |_| {});
        }
        let get_composable = self.get_composable.clone();
        let mut items = ContextualFlowItemIterator {
            item_count: self.item_count,
            get_measurables: Box::new(move |index, info: FlowLineInfo| {
                let get_composable = get_composable.clone();
                scope.subcompose(index, move || get_composable(index, info))
            }),
            list: Vec::new(),
            item_index: 0,
            list_index: 0,
        };
        self.overflow.item_count.set(self.item_count);
        let item_count = self.item_count;
        let get_overflow_measurable = |can_expand: bool, shown_item_count: i32| -> Option<Rc<dyn Measurable>> {
            let composable_index = if can_expand { 0 } else { 1 };
            let content = self.overflow_composables.get(composable_index)?;
            scope
                .subcompose_shared(format!("{can_expand}{item_count}{shown_item_count}"), content.clone())
                .into_iter()
                .next()
        };
        let overflow = self.overflow.set_overflow_measurables_lazy(&self.line, constraints, &get_overflow_measurable);
        let orientation =
            if self.line.is_horizontal { LayoutOrientation::Horizontal } else { LayoutOrientation::Vertical };
        break_down_items(
            scope.measure_scope(),
            &self.line,
            &mut items,
            self.main_axis_spacing,
            self.cross_axis_arrangement_spacing,
            OrientationIndependentConstraints::from(constraints, orientation),
            self.max_items_in_main_axis,
            self.max_lines,
            &overflow,
        )
    }
}

/// A contextual flow layout's items, each composed as it is first asked for, knowing its line (Compose's
/// ContextualFlowItemIterator): an item emitting several layouts gives them one after the other.
struct ContextualFlowItemIterator<'s> {
    item_count: i32,
    get_measurables: Box<dyn Fn(i32, FlowLineInfo) -> Vec<Rc<dyn Measurable>> + 's>,
    list: Vec<Rc<dyn Measurable>>,
    item_index: i32,
    list_index: usize,
}

impl ContextualFlowItemIterator<'_> {
    /// The next layout, composing the next item when the ones composed are used up; none past the last (Compose's
    /// `getNext`, its IndexOutOfBoundsException caught by `safeNext`).
    fn get_next(&mut self, info: FlowLineInfo) -> Option<Rc<dyn Measurable>> {
        if self.list_index < self.list.len() {
            let measurable = self.list[self.list_index].clone();
            self.list_index += 1;
            Some(measurable)
        } else if self.item_index < self.item_count {
            let measurables = (self.get_measurables)(self.item_index, info);
            self.item_index += 1;
            if measurables.is_empty() {
                // (Compose's `next()`: the next item composed with a line info of its own, all zero)
                self.get_next(FlowLineInfo::default())
            } else {
                let measurable = measurables[0].clone();
                self.list.extend(measurables);
                self.list_index += 1;
                Some(measurable)
            }
        } else {
            None
        }
    }
}

impl FlowItems<Rc<dyn Measurable>> for ContextualFlowItemIterator<'_> {
    fn has_next(&self) -> bool {
        self.list_index < self.list.len() || self.item_index < self.item_count
    }
    fn next_item(&mut self, info: Option<&FlowLineInfo>) -> Option<Rc<dyn Measurable>> {
        self.get_next(*info.expect("a contextual layout's line"))
    }
    fn is_contextual(&self) -> bool {
        true
    }
}

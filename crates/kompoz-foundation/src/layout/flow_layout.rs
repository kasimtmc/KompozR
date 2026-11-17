//! `FlowRow` and `FlowColumn`, as Compose's FlowLayout.kt, FlowLayoutBuildingBlocks.kt and FlowLayoutOverflow.kt:
//! children placed along the main axis, wrapping to a new line (a row, a column) when the next doesn't fit or a
//! line has as many as it may, each line measured as a Row or a Column is (weights, alignments,
//! `fill_max_row_height`); at most `max_lines` lines, what doesn't fit clipped or replaced by an indicator
//! (expand, collapse). The flow layouts' measuring, shared with the contextual ones (contextual_flow_layout.rs).

use super::arrangement::{HorizontalArrangement, VerticalArrangement};
use super::orientation::{LayoutOrientation, OrientationIndependentConstraints};
use super::row_column::{
    CrossAxisAlignment, CrossAxisOffsets, FlowLayoutData, MeasurableRef, RowColumnMeasurePolicy, RowColumnParentData,
    measure_row_column, parent_data_of, placeable_parent_data,
};
use super::{Arrangement, coerce_in};
use kompoz_graphics::{Density, Dp, LayoutDirection};
use kompoz_runtime::{kompozable, kompozable_block, named, remember_keyed};
use kompoz_ui::layout::{MultiContentLayout, MultiContentMeasurePolicy, ParentDataModifierNode};
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::{
    Alignment, Constraints, HorizontalAlignment, IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasureResult,
    MeasureScope, Modifier, ModifierNode, ModifierNodeElement, Placeable, VerticalAlignment, node_capabilities,
};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// The composables

/// A layout of its children in rows (Compose's `FlowRow`): side by side as `horizontal_arrangement` says, a new
/// row when the next doesn't fit or a row has `max_items_in_each_row`; the rows one under the other as
/// `vertical_arrangement` says, each child placed in its row by `item_vertical_alignment` unless it says otherwise
/// (`Modifier.align`, `align_by`); a child's `weight` shares what its row leaves, `fill_max_row_height` makes it as
/// tall as its row. At most `max_lines` rows; `overflow` (Compose's deprecated parameter) says what happens to the
/// children that don't fit: clipped (the default), shown anyway, or replaced by an indicator.
#[kompozable]
pub fn FlowRow(
    #[default(Modifier)] modifier: Modifier,
    #[default(Arrangement::Start)] horizontal_arrangement: HorizontalArrangement,
    #[default(Arrangement::Top)] vertical_arrangement: VerticalArrangement,
    #[default(Alignment::Top)] item_vertical_alignment: VerticalAlignment,
    #[default(i32::MAX)] max_items_in_each_row: i32,
    #[default(i32::MAX)] max_lines: i32,
    #[default(FlowRowOverflow::Clip)] overflow: FlowRowOverflow,
    content: impl Fn() + 'static,
) {
    let overflow_state = remember_overflow_state(&overflow.0);
    let measure_policy = flow_measure_policy(
        true,
        horizontal_arrangement,
        vertical_arrangement,
        CrossAxisAlignment::Vertical(item_vertical_alignment),
        max_items_in_each_row,
        max_lines,
        overflow_state.clone(),
    );
    let mut list: Vec<Rc<dyn Fn()>> = vec![Rc::new(content)];
    overflow.0.add_overflow_composables(&overflow_state, &mut list);
    MultiContentLayout(contents = list, measure_policy = measure_policy, modifier = modifier);
}

/// A layout of its children in columns (Compose's `FlowColumn`): one under the other as `vertical_arrangement`
/// says, a new column when the next doesn't fit or a column has `max_items_in_each_column`; the columns side by
/// side as `horizontal_arrangement` says, each child placed in its column by `item_horizontal_alignment` unless it
/// says otherwise; a child's `weight` shares what its column leaves, `fill_max_column_width` makes it as wide as
/// its column. At most `max_lines` columns; `overflow` (Compose's deprecated parameter) says what happens to the
/// children that don't fit.
#[kompozable]
pub fn FlowColumn(
    #[default(Modifier)] modifier: Modifier,
    #[default(Arrangement::Top)] vertical_arrangement: VerticalArrangement,
    #[default(Arrangement::Start)] horizontal_arrangement: HorizontalArrangement,
    #[default(Alignment::Start)] item_horizontal_alignment: HorizontalAlignment,
    #[default(i32::MAX)] max_items_in_each_column: i32,
    #[default(i32::MAX)] max_lines: i32,
    #[default(FlowColumnOverflow::Clip)] overflow: FlowColumnOverflow,
    content: impl Fn() + 'static,
) {
    let overflow_state = remember_overflow_state(&overflow.0);
    let measure_policy = flow_measure_policy(
        false,
        horizontal_arrangement,
        vertical_arrangement,
        CrossAxisAlignment::Horizontal(item_horizontal_alignment),
        max_items_in_each_column,
        max_lines,
        overflow_state.clone(),
    );
    let mut list: Vec<Rc<dyn Fn()>> = vec![Rc::new(content)];
    overflow.0.add_overflow_composables(&overflow_state, &mut list);
    MultiContentLayout(contents = list, measure_policy = measure_policy, modifier = modifier);
}

/// The overflow's state, made again when the overflow changes (Compose's `remember(overflow) {
/// overflow.createOverflowState() }`).
#[track_caller]
pub(crate) fn remember_overflow_state(overflow: &FlowLayoutOverflow) -> Rc<FlowLayoutOverflowState> {
    let o = overflow.clone();
    remember_keyed(overflow.clone(), move || Rc::new(o.create_overflow_state()))
}

/// A flow layout's policy, kept while its arguments are equal (Compose's `rowMeasurementMultiContentHelper` and
/// `columnMeasurementMultiContentHelper`; the state compared as Compose's data class is, by its configuration).
#[track_caller]
fn flow_measure_policy(
    is_horizontal: bool,
    horizontal_arrangement: HorizontalArrangement,
    vertical_arrangement: VerticalArrangement,
    cross_axis_alignment: CrossAxisAlignment,
    max_items_in_main_axis: i32,
    max_lines: i32,
    overflow: Rc<FlowLayoutOverflowState>,
) -> Rc<dyn MultiContentMeasurePolicy> {
    let keys = (
        horizontal_arrangement.clone(),
        vertical_arrangement.clone(),
        AlignmentKey(cross_axis_alignment.clone()),
        max_items_in_main_axis,
        max_lines,
        overflow.config(),
    );
    remember_keyed(keys, move || {
        let main_axis_spacing =
            if is_horizontal { horizontal_arrangement.spacing() } else { vertical_arrangement.spacing() };
        let cross_axis_arrangement_spacing =
            if is_horizontal { vertical_arrangement.spacing() } else { horizontal_arrangement.spacing() };
        Rc::new(FlowMeasurePolicy {
            line: FlowLine { is_horizontal, horizontal_arrangement, vertical_arrangement, cross_axis_alignment },
            main_axis_spacing,
            cross_axis_arrangement_spacing,
            max_items_in_main_axis,
            max_lines,
            overflow,
        }) as Rc<dyn MultiContentMeasurePolicy>
    })
}

/// A flow layout's item alignment as a key (an alignment line provider's block compares by its instance).
#[derive(Clone)]
pub(crate) struct AlignmentKey(pub(crate) CrossAxisAlignment);

impl PartialEq for AlignmentKey {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (CrossAxisAlignment::Vertical(a), CrossAxisAlignment::Vertical(b)) => a == b,
            (CrossAxisAlignment::Horizontal(a), CrossAxisAlignment::Horizontal(b)) => a == b,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Scopes

/// The modifiers of Compose's `FlowRowScope` and `FlowColumnScope` (and the contextual ones'): what a flow
/// layout's child tells it, besides a Row's or a Column's (`weight`, `align`, `align_by`).
#[named(trait FlowLayoutScopeModifiers)]
impl Modifier {
    /// Makes it `fraction` as tall as the tallest of its row (Compose's `FlowRowScope.fillMaxRowHeight`): measured
    /// after the others of its row, at that height. Panics on a fraction outside 0..=1.
    fn fill_max_row_height(self, #[default(1.0)] fraction: f32) -> Modifier {
        assert!((0.0..=1.0).contains(&fraction), "invalid fraction {fraction}; must be >= 0 and <= 1.0");
        self.then_element(FillCrossAxisSizeElement { fraction })
    }

    /// Makes it `fraction` as wide as the widest of its column (Compose's `FlowColumnScope.fillMaxColumnWidth`).
    /// Panics on a fraction outside 0..=1.
    fn fill_max_column_width(self, #[default(1.0)] fraction: f32) -> Modifier {
        assert!((0.0..=1.0).contains(&fraction), "invalid fraction {fraction}; must be >= 0 and <= 1.0");
        self.then_element(FillCrossAxisSizeElement { fraction })
    }
}

#[derive(PartialEq)]
struct FillCrossAxisSizeElement {
    fraction: f32,
}

impl ModifierNodeElement for FillCrossAxisSizeElement {
    type Node = FillCrossAxisSizeNode;
    fn create(&self) -> FillCrossAxisSizeNode {
        FillCrossAxisSizeNode { fraction: self.fraction }
    }
    fn update(&self, node: &mut FillCrossAxisSizeNode) {
        node.fraction = self.fraction;
    }
    fn name(&self) -> &'static str {
        "fraction"
    }
}

/// The node of `fill_max_row_height`/`fill_max_column_width` (Compose's FillCrossAxisSizeNode).
struct FillCrossAxisSizeNode {
    fraction: f32,
}

impl ModifierNode for FillCrossAxisSizeNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for FillCrossAxisSizeNode {
    fn modify_parent_data(&mut self, _: &Density, parent_data: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        let mut data = RowColumnParentData::from(parent_data);
        data.flow_layout_data = Some(FlowLayoutData { fill_cross_axis_fraction: self.fraction });
        Some(Box::new(data))
    }
}

/// What a `FlowRow`'s overflow indicator is composed in (Compose's `FlowRowOverflowScope`, a `FlowRowScope`):
/// how many children there are, and how many show. A clone reads the same layout (a draw block can keep one).
#[derive(Clone)]
pub struct FlowRowOverflowScope {
    state: Rc<FlowLayoutOverflowState>,
    shown: Cell<Option<i32>>,
}

impl FlowRowOverflowScope {
    pub(crate) fn new(state: Rc<FlowLayoutOverflowState>) -> FlowRowOverflowScope {
        FlowRowOverflowScope { state, shown: Cell::new(None) }
    }

    /// How many children the layout has (Compose's `totalItemCount`).
    pub fn total_item_count(&self) -> i32 {
        self.state.item_count.get()
    }

    /// How many of them show (Compose's `shownItemCount`): known once the layout measured (in its draw, say).
    /// Panics when read before, as Compose's throws (a contextual flow layout's indicator is composed as it
    /// measures: it can read it as it composes).
    pub fn shown_item_count(&self) -> i32 {
        if let Some(shown) = self.shown.get() {
            return shown;
        }
        let shown = self.state.shown_item_count();
        self.shown.set(Some(shown));
        shown
    }
}

/// What a `FlowColumn`'s overflow indicator is composed in (Compose's `FlowColumnOverflowScope`).
pub type FlowColumnOverflowScope = FlowRowOverflowScope;

/// What a `ContextualFlowRow`'s overflow indicator is composed in (Compose's `ContextualFlowRowOverflowScope`, a
/// `FlowRowOverflowScope` adding nothing).
pub type ContextualFlowRowOverflowScope = FlowRowOverflowScope;

/// What a `ContextualFlowColumn`'s overflow indicator is composed in (Compose's
/// `ContextualFlowColumnOverflowScope`).
pub type ContextualFlowColumnOverflowScope = FlowRowOverflowScope;

// ---------------------------------------------------------------------------------------------------------------
// Overflow (Compose's FlowLayoutOverflow.kt)

/// What a flow layout does with the children past its lines (Compose's `FlowLayoutOverflow.OverflowType`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum OverflowType {
    Visible,
    Clip,
    ExpandIndicator,
    ExpandOrCollapseIndicator,
}

/// An indicator's content made for a state (Compose's `seeMoreGetter`/`collapseGetter`).
type IndicatorGetter = Rc<dyn Fn(&Rc<FlowLayoutOverflowState>) -> Rc<dyn Fn()>>;

/// What all the flow layouts' overflows are (Compose's sealed `FlowLayoutOverflow`): its type, when a collapse
/// indicator shows, and the indicators' contents. Equal to another of the same type and minimums with the same
/// indicators (by their instances, as Compose compares the class by identity).
#[derive(Clone)]
pub struct FlowLayoutOverflow {
    pub(crate) kind: OverflowType,
    min_lines_to_show_collapse: i32,
    min_cross_axis_size_to_show_collapse: i32,
    see_more_getter: Option<IndicatorGetter>,
    collapse_getter: Option<IndicatorGetter>,
}

impl PartialEq for FlowLayoutOverflow {
    fn eq(&self, other: &Self) -> bool {
        fn same(a: &Option<IndicatorGetter>, b: &Option<IndicatorGetter>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                _ => false,
            }
        }
        self.kind == other.kind
            && self.min_lines_to_show_collapse == other.min_lines_to_show_collapse
            && self.min_cross_axis_size_to_show_collapse == other.min_cross_axis_size_to_show_collapse
            && same(&self.see_more_getter, &other.see_more_getter)
            && same(&self.collapse_getter, &other.collapse_getter)
    }
}

impl std::fmt::Debug for FlowLayoutOverflow {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "FlowLayoutOverflow({:?})", self.kind)
    }
}

impl FlowLayoutOverflow {
    const fn of(kind: OverflowType) -> FlowLayoutOverflow {
        FlowLayoutOverflow {
            kind,
            min_lines_to_show_collapse: 0,
            min_cross_axis_size_to_show_collapse: 0,
            see_more_getter: None,
            collapse_getter: None,
        }
    }

    /// An expand indicator's overflow, `content` composed in a scope over the layout's state.
    fn expand(content: Rc<dyn Fn(&FlowRowOverflowScope)>) -> FlowLayoutOverflow {
        FlowLayoutOverflow {
            see_more_getter: Some(indicator_getter(content)),
            ..FlowLayoutOverflow::of(OverflowType::ExpandIndicator)
        }
    }

    /// An expand-or-collapse overflow (Compose's `expandOrCollapseIndicator`'s body, remembered by its arguments).
    #[track_caller]
    fn expand_or_collapse(
        expand_indicator: Rc<dyn Fn(&FlowRowOverflowScope)>,
        collapse_indicator: Rc<dyn Fn(&FlowRowOverflowScope)>,
        min_lines_to_show_collapse: i32,
        min_cross_axis_size_to_show_collapse: Dp,
    ) -> FlowLayoutOverflow {
        let min_cross_px = LocalDensity.current().round_to_px(min_cross_axis_size_to_show_collapse);
        let keys = (
            min_lines_to_show_collapse,
            min_cross_px,
            ClosureKey(expand_indicator.clone()),
            ClosureKey(collapse_indicator.clone()),
        );
        remember_keyed(keys, move || FlowLayoutOverflow {
            kind: OverflowType::ExpandOrCollapseIndicator,
            min_lines_to_show_collapse,
            min_cross_axis_size_to_show_collapse: min_cross_px,
            see_more_getter: Some(indicator_getter(expand_indicator)),
            collapse_getter: Some(indicator_getter(collapse_indicator)),
        })
    }

    pub(crate) fn create_overflow_state(&self) -> FlowLayoutOverflowState {
        FlowLayoutOverflowState::new(
            self.kind,
            self.min_lines_to_show_collapse,
            self.min_cross_axis_size_to_show_collapse,
        )
    }

    /// Adds the indicators' contents to `list` (after the layout's content: its second and third contents).
    pub(crate) fn add_overflow_composables(&self, state: &Rc<FlowLayoutOverflowState>, list: &mut Vec<Rc<dyn Fn()>>) {
        let expand_indicator = self.see_more_getter.as_ref().map(|g| g(state));
        let collapse_indicator = self.collapse_getter.as_ref().map(|g| g(state));
        match self.kind {
            OverflowType::ExpandIndicator => list.extend(expand_indicator),
            OverflowType::ExpandOrCollapseIndicator => {
                list.extend(expand_indicator);
                list.extend(collapse_indicator);
            }
            _ => {}
        }
    }
}

fn indicator_getter(content: Rc<dyn Fn(&FlowRowOverflowScope)>) -> IndicatorGetter {
    Rc::new(move |state: &Rc<FlowLayoutOverflowState>| {
        let (state, content) = (state.clone(), content.clone());
        Rc::new(move || {
            let scope = FlowRowOverflowScope::new(state.clone());
            content(&scope);
        }) as Rc<dyn Fn()>
    })
}

/// A closure compared by its instance (a Kotlin lambda in `remember`'s keys).
#[derive(Clone)]
struct ClosureKey(Rc<dyn Fn(&FlowRowOverflowScope)>);

impl PartialEq for ClosureKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// What a `FlowRow` does with the children past its rows (Compose's deprecated `FlowRowOverflow`).
#[derive(Clone, PartialEq, Debug)]
pub struct FlowRowOverflow(pub(crate) FlowLayoutOverflow);

impl FlowRowOverflow {
    /// The children past the lines shown anyway, outside the layout (Compose's `Visible`).
    pub const Visible: FlowRowOverflow = FlowRowOverflow(FlowLayoutOverflow::of(OverflowType::Visible));
    /// The children past the lines left out (Compose's `Clip`, the default).
    pub const Clip: FlowRowOverflow = FlowRowOverflow(FlowLayoutOverflow::of(OverflowType::Clip));

    /// An indicator shown after the last child that fits when some don't (Compose's `expandIndicator`): `content`
    /// is composed with the layout's counts.
    pub fn expand_indicator(content: impl Fn(&FlowRowOverflowScope) + 'static) -> FlowRowOverflow {
        FlowRowOverflow(FlowLayoutOverflow::expand(Rc::new(content)))
    }

    /// An expand indicator when some children don't fit, a collapse indicator when all fit and there are at least
    /// `min_rows_to_show_collapse` rows `min_height_to_show_collapse` tall (Compose's composable
    /// `expandOrCollapseIndicator`; remembered).
    ///
    /// Kotlin: `expandOrCollapseIndicator(expandIndicator: @Composable FlowRowOverflowScope.() -> Unit, collapseIndicator: @Composable FlowRowOverflowScope.() -> Unit, minRowsToShowCollapse: Int = 1, minHeightToShowCollapse: Dp = 0.dp)`
    #[track_caller]
    pub fn expand_or_collapse_indicator(
        expand_indicator: impl Fn(&FlowRowOverflowScope) + 'static,
        collapse_indicator: impl Fn(&FlowRowOverflowScope) + 'static,
        min_rows_to_show_collapse: i32,
        min_height_to_show_collapse: Dp,
    ) -> FlowRowOverflow {
        let (expand, collapse) = (Rc::new(expand_indicator), Rc::new(collapse_indicator));
        FlowRowOverflow(kompozable_block!(FlowLayoutOverflow::expand_or_collapse(
            expand,
            collapse,
            min_rows_to_show_collapse,
            min_height_to_show_collapse
        )))
    }
}

/// What a `FlowColumn` does with the children past its columns (Compose's deprecated `FlowColumnOverflow`).
#[derive(Clone, PartialEq, Debug)]
pub struct FlowColumnOverflow(pub(crate) FlowLayoutOverflow);

impl FlowColumnOverflow {
    /// The children past the lines shown anyway, outside the layout (Compose's `Visible`).
    pub const Visible: FlowColumnOverflow = FlowColumnOverflow(FlowLayoutOverflow::of(OverflowType::Visible));
    /// The children past the lines left out (Compose's `Clip`, the default).
    pub const Clip: FlowColumnOverflow = FlowColumnOverflow(FlowLayoutOverflow::of(OverflowType::Clip));

    /// An indicator shown after the last child that fits when some don't (Compose's `expandIndicator`).
    pub fn expand_indicator(content: impl Fn(&FlowColumnOverflowScope) + 'static) -> FlowColumnOverflow {
        FlowColumnOverflow(FlowLayoutOverflow::expand(Rc::new(content)))
    }

    /// An expand indicator when some children don't fit, a collapse indicator when all fit and there are at least
    /// `min_columns_to_show_collapse` columns `min_width_to_show_collapse` wide (Compose's composable
    /// `expandOrCollapseIndicator`; remembered).
    ///
    /// Kotlin: `expandOrCollapseIndicator(expandIndicator: @Composable FlowColumnOverflowScope.() -> Unit, collapseIndicator: @Composable FlowColumnOverflowScope.() -> Unit, minColumnsToShowCollapse: Int = 1, minWidthToShowCollapse: Dp = 0.dp)`
    #[track_caller]
    pub fn expand_or_collapse_indicator(
        expand_indicator: impl Fn(&FlowColumnOverflowScope) + 'static,
        collapse_indicator: impl Fn(&FlowColumnOverflowScope) + 'static,
        min_columns_to_show_collapse: i32,
        min_width_to_show_collapse: Dp,
    ) -> FlowColumnOverflow {
        let (expand, collapse) = (Rc::new(expand_indicator), Rc::new(collapse_indicator));
        FlowColumnOverflow(kompozable_block!(FlowLayoutOverflow::expand_or_collapse(
            expand,
            collapse,
            min_columns_to_show_collapse,
            min_width_to_show_collapse
        )))
    }
}

/// What a `ContextualFlowRow` does with the items past its rows (Compose's deprecated `ContextualFlowRowOverflow`).
#[derive(Clone, PartialEq, Debug)]
pub struct ContextualFlowRowOverflow(pub(crate) FlowLayoutOverflow);

impl ContextualFlowRowOverflow {
    /// The items past the lines shown anyway, outside the layout (Compose's `Visible`).
    pub const Visible: ContextualFlowRowOverflow =
        ContextualFlowRowOverflow(FlowLayoutOverflow::of(OverflowType::Visible));
    /// The items past the lines left out (Compose's `Clip`, the default).
    pub const Clip: ContextualFlowRowOverflow = ContextualFlowRowOverflow(FlowLayoutOverflow::of(OverflowType::Clip));

    /// An indicator shown after the last item that fits when some don't (Compose's `expandIndicator`), composed
    /// knowing how many show.
    pub fn expand_indicator(content: impl Fn(&ContextualFlowRowOverflowScope) + 'static) -> ContextualFlowRowOverflow {
        ContextualFlowRowOverflow(FlowLayoutOverflow::expand(Rc::new(content)))
    }

    /// An expand indicator when some items don't fit, a collapse indicator when all fit and there are at least
    /// `min_rows_to_show_collapse` rows `min_height_to_show_collapse` tall (Compose's composable
    /// `expandOrCollapseIndicator`; remembered).
    ///
    /// Kotlin: `expandOrCollapseIndicator(expandIndicator: @Composable ContextualFlowRowOverflowScope.() -> Unit, collapseIndicator: @Composable ContextualFlowRowOverflowScope.() -> Unit, minRowsToShowCollapse: Int = 1, minHeightToShowCollapse: Dp = 0.dp)`
    #[track_caller]
    pub fn expand_or_collapse_indicator(
        expand_indicator: impl Fn(&ContextualFlowRowOverflowScope) + 'static,
        collapse_indicator: impl Fn(&ContextualFlowRowOverflowScope) + 'static,
        min_rows_to_show_collapse: i32,
        min_height_to_show_collapse: Dp,
    ) -> ContextualFlowRowOverflow {
        let (expand, collapse) = (Rc::new(expand_indicator), Rc::new(collapse_indicator));
        ContextualFlowRowOverflow(kompozable_block!(FlowLayoutOverflow::expand_or_collapse(
            expand,
            collapse,
            min_rows_to_show_collapse,
            min_height_to_show_collapse
        )))
    }
}

/// What a `ContextualFlowColumn` does with the items past its columns (Compose's deprecated
/// `ContextualFlowColumnOverflow`).
#[derive(Clone, PartialEq, Debug)]
pub struct ContextualFlowColumnOverflow(pub(crate) FlowLayoutOverflow);

impl ContextualFlowColumnOverflow {
    /// The items past the lines shown anyway, outside the layout (Compose's `Visible`).
    pub const Visible: ContextualFlowColumnOverflow =
        ContextualFlowColumnOverflow(FlowLayoutOverflow::of(OverflowType::Visible));
    /// The items past the lines left out (Compose's `Clip`, the default).
    pub const Clip: ContextualFlowColumnOverflow =
        ContextualFlowColumnOverflow(FlowLayoutOverflow::of(OverflowType::Clip));

    /// An indicator shown after the last item that fits when some don't (Compose's `expandIndicator`), composed
    /// knowing how many show.
    pub fn expand_indicator(
        content: impl Fn(&ContextualFlowColumnOverflowScope) + 'static,
    ) -> ContextualFlowColumnOverflow {
        ContextualFlowColumnOverflow(FlowLayoutOverflow::expand(Rc::new(content)))
    }

    /// An expand indicator when some items don't fit, a collapse indicator when all fit and there are at least
    /// `min_columns_to_show_collapse` columns `min_width_to_show_collapse` wide (Compose's composable
    /// `expandOrCollapseIndicator`; remembered).
    ///
    /// Kotlin: `expandOrCollapseIndicator(expandIndicator: @Composable ContextualFlowColumnOverflowScope.() -> Unit, collapseIndicator: @Composable ContextualFlowColumnOverflowScope.() -> Unit, minColumnsToShowCollapse: Int = 1, minWidthToShowCollapse: Dp = 0.dp)`
    #[track_caller]
    pub fn expand_or_collapse_indicator(
        expand_indicator: impl Fn(&ContextualFlowColumnOverflowScope) + 'static,
        collapse_indicator: impl Fn(&ContextualFlowColumnOverflowScope) + 'static,
        min_columns_to_show_collapse: i32,
        min_width_to_show_collapse: Dp,
    ) -> ContextualFlowColumnOverflow {
        let (expand, collapse) = (Rc::new(expand_indicator), Rc::new(collapse_indicator));
        ContextualFlowColumnOverflow(kompozable_block!(FlowLayoutOverflow::expand_or_collapse(
            expand,
            collapse,
            min_columns_to_show_collapse,
            min_width_to_show_collapse
        )))
    }
}

/// What the shown-item count panics with when read before the layout measured (Compose's
/// `shownItemLazyErrorMessage`).
const SHOWN_ITEM_LAZY_ERROR_MESSAGE: &str = "Accessing shownItemCount before it is set. Are you calling this in \
    the Composition phase, rather than in the draw phase? Consider our samples on how to use it during the draw \
    phase or consider using ContextualFlowRow/ContextualFlowColumn which initializes this method in the \
    composition phase.";

/// A flow layout's overflow as it measures (Compose's FlowLayoutOverflowState): its configuration, how many
/// children there are and show, and the indicators' sizes. Its indicators' measurables and placeables are the
/// measure's own ([`OverflowMeasurables`]): Rust can't keep a measure's borrows.
pub(crate) struct FlowLayoutOverflowState {
    pub(crate) kind: OverflowType,
    pub(crate) min_lines_to_show_collapse: i32,
    pub(crate) min_cross_axis_size_to_show_collapse: i32,
    pub(crate) item_shown: Cell<i32>,
    pub(crate) item_count: Cell<i32>,
    see_more_size: Cell<Option<(i32, i32)>>,
    collapse_size: Cell<Option<(i32, i32)>>,
}

impl FlowLayoutOverflowState {
    fn new(kind: OverflowType, min_lines: i32, min_cross: i32) -> FlowLayoutOverflowState {
        FlowLayoutOverflowState {
            kind,
            min_lines_to_show_collapse: min_lines,
            min_cross_axis_size_to_show_collapse: min_cross,
            item_shown: Cell::new(-1),
            item_count: Cell::new(0),
            see_more_size: Cell::new(None),
            collapse_size: Cell::new(None),
        }
    }

    /// What Compose's data class compares (its constructor's values).
    pub(crate) fn config(&self) -> (OverflowType, i32, i32) {
        (self.kind, self.min_lines_to_show_collapse, self.min_cross_axis_size_to_show_collapse)
    }

    pub(crate) fn shown_item_count(&self) -> i32 {
        let shown = self.item_shown.get();
        assert!(shown != -1, "{SHOWN_ITEM_LAZY_ERROR_MESSAGE}");
        shown
    }

    /// The indicator's size if one would show after `line_index` (Compose's `ellipsisSize`).
    pub(crate) fn ellipsis_size(
        &self,
        has_next: bool,
        line_index: i32,
        total_cross_axis_size: i32,
    ) -> Option<(i32, i32)> {
        match self.kind {
            OverflowType::Visible | OverflowType::Clip => None,
            OverflowType::ExpandIndicator => {
                if has_next {
                    self.see_more_size.get()
                } else {
                    None
                }
            }
            OverflowType::ExpandOrCollapseIndicator => {
                if has_next {
                    self.see_more_size.get()
                } else if line_index + 1 >= self.min_lines_to_show_collapse
                    && total_cross_axis_size >= self.min_cross_axis_size_to_show_collapse
                {
                    self.collapse_size.get()
                } else {
                    None
                }
            }
        }
    }

    /// Notes the indicators' sizes from their intrinsics (Compose's `setOverflowMeasurables` of intrinsic
    /// measurables).
    pub(crate) fn set_overflow_intrinsic_measurables<'a>(
        &'a self,
        see_more: Option<&'a dyn IntrinsicMeasurable>,
        collapse: Option<&'a dyn IntrinsicMeasurable>,
        is_horizontal: bool,
        constraints: Constraints,
    ) -> OverflowMeasurables<'a, &'a dyn IntrinsicMeasurable> {
        let orientation = orientation(is_horizontal);
        let c = OrientationIndependentConstraints::from(constraints, orientation);
        let mut out = OverflowMeasurables::new(self);
        if let Some(item) = see_more {
            let main = main_axis_min(item, is_horizontal, c.cross_axis_max());
            let cross = cross_axis_min(item, is_horizontal, main);
            self.see_more_size.set(Some((main, cross)));
            out.see_more = Some(item);
        }
        if let Some(item) = collapse {
            let main = main_axis_min(item, is_horizontal, c.cross_axis_max());
            let cross = cross_axis_min(item, is_horizontal, main);
            self.collapse_size.set(Some((main, cross)));
            out.collapse = Some(item);
        }
        out
    }

    /// Measures the indicators (or their intrinsics, for weighted ones) and notes their sizes (Compose's
    /// `setOverflowMeasurables(measurePolicy, …)`).
    pub(crate) fn set_overflow_measurables<'a, M: MeasurableRef>(
        &'a self,
        line: &FlowLine,
        see_more: Option<M>,
        collapse: Option<M>,
        constraints: Constraints,
    ) -> OverflowMeasurables<'a, M> {
        let orientation = orientation(line.is_horizontal);
        let final_constraints = OrientationIndependentConstraints::from(constraints, orientation)
            .copy(Some(0), None, Some(0), None)
            .to_box_constraints(orientation);
        let mut out = OverflowMeasurables::new(self);
        if let Some(item) = see_more {
            let mut stored = None;
            measure_and_cache(item.get(), line, final_constraints, |placeable| {
                let size = placeable.as_ref().map_or((0, 0), |p| (line.main_axis_size(p), line.cross_axis_size(p)));
                self.see_more_size.set(Some(size));
                stored = Some(placeable);
            });
            if let Some(placeable) = stored {
                out.see_more_placeable = placeable;
            }
            out.see_more = Some(item);
        }
        if let Some(item) = collapse {
            let mut stored = None;
            measure_and_cache(item.get(), line, final_constraints, |placeable| {
                let size = placeable.as_ref().map_or((0, 0), |p| (line.main_axis_size(p), line.cross_axis_size(p)));
                self.collapse_size.set(Some(size));
                stored = Some(placeable);
            });
            if let Some(placeable) = stored {
                out.collapse_placeable = placeable;
            }
            out.collapse = Some(item);
        }
        out
    }
}

/// A measure's view of the overflow: the state and the indicators' measurables and placeables (the parts of
/// Compose's FlowLayoutOverflowState a measure sets), or a contextual layout's way to subcompose them
/// (`getOverflowMeasurable`).
pub(crate) struct OverflowMeasurables<'a, M> {
    pub(crate) state: &'a FlowLayoutOverflowState,
    see_more: Option<M>,
    see_more_placeable: Option<Placeable>,
    collapse: Option<M>,
    collapse_placeable: Option<Placeable>,
    get_overflow_measurable: Option<&'a dyn Fn(bool, i32) -> Option<M>>,
}

impl<'a, M: Clone> OverflowMeasurables<'a, M> {
    pub(crate) fn new(state: &'a FlowLayoutOverflowState) -> Self {
        OverflowMeasurables {
            state,
            see_more: None,
            see_more_placeable: None,
            collapse: None,
            collapse_placeable: None,
            get_overflow_measurable: None,
        }
    }

    /// The indicator to show after `line_index`, if one does (Compose's `ellipsisInfo`).
    fn ellipsis_info(
        &self,
        has_next: bool,
        line_index: i32,
        total_cross_axis_size: i32,
    ) -> Option<WrapEllipsisInfo<M>> {
        let state = self.state;
        match state.kind {
            OverflowType::Visible | OverflowType::Clip => None,
            OverflowType::ExpandIndicator | OverflowType::ExpandOrCollapseIndicator => {
                let mut measurable = None;
                let mut placeable = None;
                let ellipsis_size;
                if has_next {
                    measurable = self
                        .get_overflow_measurable
                        .and_then(|get| get(true, state.shown_item_count()))
                        .or_else(|| self.see_more.clone());
                    ellipsis_size = state.see_more_size.get();
                    if self.get_overflow_measurable.is_none() {
                        placeable = self.see_more_placeable.clone();
                    }
                } else {
                    if line_index >= state.min_lines_to_show_collapse - 1
                        && total_cross_axis_size >= state.min_cross_axis_size_to_show_collapse
                    {
                        measurable = self
                            .get_overflow_measurable
                            .and_then(|get| get(false, state.shown_item_count()))
                            .or_else(|| self.collapse.clone());
                    }
                    ellipsis_size = state.collapse_size.get();
                    if self.get_overflow_measurable.is_none() {
                        placeable = self.collapse_placeable.clone();
                    }
                }
                let ellipsis = measurable?;
                Some(WrapEllipsisInfo {
                    ellipsis,
                    placeable,
                    ellipsis_size: ellipsis_size.expect("an indicator measured"),
                    place_ellipsis_on_last_content_line: true,
                })
            }
        }
    }
}

impl<'a> FlowLayoutOverflowState {
    /// A contextual layout's indicators, subcomposed by `get_overflow_measurable` (Compose's
    /// `setOverflowMeasurables(measurePolicy, constraints, getOverflowMeasurable)`).
    pub(crate) fn set_overflow_measurables_lazy<M: MeasurableRef>(
        &'a self,
        line: &FlowLine,
        constraints: Constraints,
        get_overflow_measurable: &'a dyn Fn(bool, i32) -> Option<M>,
    ) -> OverflowMeasurables<'a, M> {
        self.item_shown.set(0);
        let see_more = get_overflow_measurable(true, 0);
        let collapse = get_overflow_measurable(false, 0);
        let mut out = self.set_overflow_measurables(line, see_more, collapse, constraints);
        out.get_overflow_measurable = Some(get_overflow_measurable);
        out
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Building blocks (Compose's FlowLayoutBuildingBlocks.kt)

/// Whether an item ends its line, and the layout (Compose's `WrapInfo`).
#[derive(Clone, Copy, Default, Debug)]
struct WrapInfo {
    is_last_item_in_line: bool,
    is_last_item_in_container: bool,
}

/// The indicator shown at the end, and whether it fits on the last line (Compose's `WrapEllipsisInfo`).
struct WrapEllipsisInfo<M> {
    ellipsis: M,
    placeable: Option<Placeable>,
    ellipsis_size: (i32, i32),
    place_ellipsis_on_last_content_line: bool,
}

/// How lines break, the indicator included (Compose's FlowLayoutBuildingBlocks).
struct FlowLayoutBuildingBlocks<'o, 'a, M> {
    max_items_in_main_axis: i32,
    overflow: &'o OverflowMeasurables<'a, M>,
    max_lines: i32,
    constraints: OrientationIndependentConstraints,
    main_axis_spacing: i32,
    cross_axis_spacing: i32,
}

impl<M: Clone> FlowLayoutBuildingBlocks<'_, '_, M> {
    fn get_wrap_ellipsis_info(
        &self,
        wrap_info: WrapInfo,
        has_next: bool,
        last_content_line_index: i32,
        total_cross_axis_size: i32,
        left_over_main_axis: i32,
        next_index_in_line: i32,
    ) -> Option<WrapEllipsisInfo<M>> {
        if !wrap_info.is_last_item_in_container {
            return None;
        }
        let mut info = self.overflow.ellipsis_info(has_next, last_content_line_index, total_cross_axis_size)?;
        let can_fit_line = last_content_line_index >= 0
            && (next_index_in_line == 0
                || !(left_over_main_axis - info.ellipsis_size.0 < 0
                    || next_index_in_line >= self.max_items_in_main_axis));
        info.place_ellipsis_on_last_content_line = can_fit_line;
        Some(info)
    }

    #[allow(clippy::too_many_arguments)]
    fn get_wrap_info(
        &self,
        next_item_has_next: bool,
        next_index_in_line: i32,
        left_over: (i32, i32),
        next_size: Option<(i32, i32)>,
        line_index: i32,
        total_cross_axis_size: i32,
        current_line_cross_axis_size: i32,
        is_wrapping_round: bool,
        is_ellipsis_wrap: bool,
    ) -> WrapInfo {
        let mut total_container_cross_axis_size = total_cross_axis_size + current_line_cross_axis_size;
        let Some(next_size) = next_size else {
            return WrapInfo { is_last_item_in_line: true, is_last_item_in_container: true };
        };
        let will_overflow_cross_axis = if self.overflow.state.kind == OverflowType::Visible {
            false
        } else {
            line_index >= self.max_lines || left_over.1 - next_size.1 < 0
        };
        if will_overflow_cross_axis {
            return WrapInfo { is_last_item_in_line: true, is_last_item_in_container: true };
        }

        let should_wrap_item = if next_index_in_line == 0 {
            false
        } else {
            next_index_in_line >= self.max_items_in_main_axis || left_over.0 - next_size.0 < 0
        };
        if should_wrap_item {
            if is_wrapping_round {
                return WrapInfo { is_last_item_in_line: true, is_last_item_in_container: true };
            }
            let wrap_info = self.get_wrap_info(
                next_item_has_next,
                0,
                (
                    self.constraints.main_axis_max(),
                    left_over.1 - self.cross_axis_spacing - current_line_cross_axis_size,
                ),
                // (without the main axis spacing added to the second or later items of a line)
                Some((next_size.0 - self.main_axis_spacing, next_size.1)),
                line_index + 1,
                total_container_cross_axis_size,
                0,
                true,
                false,
            );
            return WrapInfo {
                is_last_item_in_line: true,
                is_last_item_in_container: wrap_info.is_last_item_in_container,
            };
        }

        total_container_cross_axis_size = total_cross_axis_size + current_line_cross_axis_size.max(next_size.1);
        let ellipsis = if is_ellipsis_wrap {
            None
        } else {
            self.overflow.state.ellipsis_size(next_item_has_next, line_index, total_container_cross_axis_size)
        };
        let should_wrap_ellipsis = ellipsis.is_some_and(|ellipsis| {
            next_index_in_line + 1 >= self.max_items_in_main_axis
                || left_over.0 - next_size.0 - self.main_axis_spacing - ellipsis.0 < 0
        });
        if should_wrap_ellipsis {
            if is_ellipsis_wrap {
                return WrapInfo { is_last_item_in_line: true, is_last_item_in_container: true };
            }
            let wrap_info = self.get_wrap_info(
                false,
                0,
                (
                    self.constraints.main_axis_max(),
                    left_over.1 - self.cross_axis_spacing - current_line_cross_axis_size.max(next_size.1),
                ),
                ellipsis,
                line_index + 1,
                total_container_cross_axis_size,
                0,
                true,
                true,
            );
            return WrapInfo {
                is_last_item_in_line: wrap_info.is_last_item_in_container,
                is_last_item_in_container: wrap_info.is_last_item_in_container,
            };
        }
        WrapInfo { is_last_item_in_line: false, is_last_item_in_container: false }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A line (Compose's FlowLineMeasurePolicy)

/// How a flow layout measures and places a line, as a Row or a Column (Compose's FlowLineMeasurePolicy).
#[derive(Clone)]
pub(crate) struct FlowLine {
    pub(crate) is_horizontal: bool,
    pub(crate) horizontal_arrangement: HorizontalArrangement,
    pub(crate) vertical_arrangement: VerticalArrangement,
    pub(crate) cross_axis_alignment: CrossAxisAlignment,
}

impl RowColumnMeasurePolicy for FlowLine {
    fn main_axis_size(&self, placeable: &Placeable) -> i32 {
        if self.is_horizontal { placeable.measured_width() } else { placeable.measured_height() }
    }
    fn cross_axis_size(&self, placeable: &Placeable) -> i32 {
        if self.is_horizontal { placeable.measured_height() } else { placeable.measured_width() }
    }

    fn create_constraints(
        &self,
        main_axis_min: i32,
        cross_axis_min: i32,
        main_axis_max: i32,
        cross_axis_max: i32,
        is_prioritizing: bool,
    ) -> Constraints {
        match (self.is_horizontal, is_prioritizing) {
            (true, false) => Constraints::new(main_axis_min, main_axis_max, cross_axis_min, cross_axis_max),
            (true, true) => {
                Constraints::fit_prioritizing_width(main_axis_min, main_axis_max, cross_axis_min, cross_axis_max)
            }
            (false, false) => Constraints::new(cross_axis_min, cross_axis_max, main_axis_min, main_axis_max),
            (false, true) => {
                Constraints::fit_prioritizing_height(cross_axis_min, cross_axis_max, main_axis_min, main_axis_max)
            }
        }
    }

    fn place_helper(
        &self,
        placeables: &[Option<Placeable>],
        scope: &MeasureScope,
        before_cross_axis_alignment_line: i32,
        main_axis_positions: Vec<i32>,
        main_axis_layout_size: i32,
        cross_axis_layout_size: i32,
        cross_axis_offset: Option<CrossAxisOffsets>,
        current_line_index: usize,
        start_index: usize,
        end_index: usize,
    ) -> MeasureResult {
        let (width, height) = if self.is_horizontal {
            (main_axis_layout_size, cross_axis_layout_size)
        } else {
            (cross_axis_layout_size, main_axis_layout_size)
        };
        let layout_direction = if self.is_horizontal { LayoutDirection::Ltr } else { scope.layout_direction() };
        let placeables: Vec<Placeable> =
            placeables[start_index..end_index].iter().map(|p| p.clone().expect("measured")).collect();
        let line = self.clone();
        scope.layout(width, height, move |p| {
            // (where the line is across, known once all the lines are measured)
            let cross_axis_line_offset = cross_axis_offset.as_ref().map_or(0, |o| o.borrow()[current_line_index]);
            for (i, placeable) in placeables.iter().enumerate() {
                let cross_axis_position = line.get_cross_axis_position(
                    placeable,
                    cross_axis_layout_size,
                    layout_direction,
                    before_cross_axis_alignment_line,
                ) + cross_axis_line_offset;
                if line.is_horizontal {
                    p.place(placeable, main_axis_positions[i], cross_axis_position);
                } else {
                    p.place(placeable, cross_axis_position, main_axis_positions[i]);
                }
            }
        })
    }

    fn populate_main_axis_positions(
        &self,
        main_axis_layout_size: i32,
        children_main_axis_size: &[i32],
        main_axis_positions: &mut [i32],
        scope: &MeasureScope,
    ) {
        if self.is_horizontal {
            self.horizontal_arrangement.arrange(
                scope,
                main_axis_layout_size,
                children_main_axis_size,
                scope.layout_direction(),
                main_axis_positions,
            );
        } else {
            self.vertical_arrangement.arrange(
                scope,
                main_axis_layout_size,
                children_main_axis_size,
                main_axis_positions,
            );
        }
    }
}

impl FlowLine {
    fn get_cross_axis_position(
        &self,
        placeable: &Placeable,
        cross_axis_layout_size: i32,
        layout_direction: LayoutDirection,
        before_cross_axis_alignment_line: i32,
    ) -> i32 {
        let child = placeable_parent_data(placeable).and_then(|d| d.cross_axis_alignment);
        let alignment = child.as_ref().unwrap_or(&self.cross_axis_alignment);
        alignment.align(
            cross_axis_layout_size,
            self.cross_axis_size(placeable),
            layout_direction,
            placeable,
            before_cross_axis_alignment_line,
        )
    }

    fn orientation(&self) -> LayoutOrientation {
        orientation(self.is_horizontal)
    }
}

fn orientation(is_horizontal: bool) -> LayoutOrientation {
    if is_horizontal { LayoutOrientation::Horizontal } else { LayoutOrientation::Vertical }
}

// ---------------------------------------------------------------------------------------------------------------
// The policy (Compose's FlowMeasurePolicy)

/// A flow layout's policy (Compose's FlowMeasurePolicy): its first content's children in lines, the second and
/// third contents its expand and collapse indicators.
struct FlowMeasurePolicy {
    line: FlowLine,
    main_axis_spacing: Dp,
    cross_axis_arrangement_spacing: Dp,
    max_items_in_main_axis: i32,
    max_lines: i32,
    overflow: Rc<FlowLayoutOverflowState>,
}

impl MultiContentMeasurePolicy for FlowMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[Vec<&dyn Measurable>],
        constraints: Constraints,
    ) -> MeasureResult {
        if self.max_lines == 0
            || self.max_items_in_main_axis == 0
            || measurables.is_empty()
            || constraints.max_height == 0 && self.overflow.kind != OverflowType::Visible
        {
            return scope.layout(0, 0, |_| {});
        }
        let list = &measurables[0];
        if list.is_empty() {
            return scope.layout(0, 0, |_| {});
        }
        let see_more = measurables.get(1).and_then(|l| l.first().copied());
        let collapse = measurables.get(2).and_then(|l| l.first().copied());
        self.overflow.item_count.set(list.len() as i32);
        let overflow = self.overflow.set_overflow_measurables(&self.line, see_more, collapse, constraints);
        let mut items = PlainItems { list, next: 0 };
        break_down_items(
            scope,
            &self.line,
            &mut items,
            self.main_axis_spacing,
            self.cross_axis_arrangement_spacing,
            OrientationIndependentConstraints::from(constraints, self.line.orientation()),
            self.max_items_in_main_axis,
            self.max_lines,
            &overflow,
        )
    }

    fn min_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        height: i32,
    ) -> i32 {
        let overflow = self.intrinsic_overflow(measurables, Constraints::new(0, Constraints::Infinity, 0, height));
        let children = measurables.first().map(Vec::as_slice).unwrap_or_default();
        if self.line.is_horizontal {
            self.min_intrinsic_main_axis_size(scope, children, height, &overflow)
        } else {
            self.intrinsic_cross_axis_size(scope, children, height, &overflow)
        }
    }

    fn min_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        width: i32,
    ) -> i32 {
        let overflow = self.intrinsic_overflow(measurables, Constraints::new(0, width, 0, Constraints::Infinity));
        let children = measurables.first().map(Vec::as_slice).unwrap_or_default();
        if self.line.is_horizontal {
            self.intrinsic_cross_axis_size(scope, children, width, &overflow)
        } else {
            self.min_intrinsic_main_axis_size(scope, children, width, &overflow)
        }
    }

    fn max_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        width: i32,
    ) -> i32 {
        let overflow = self.intrinsic_overflow(measurables, Constraints::new(0, width, 0, Constraints::Infinity));
        let children = measurables.first().map(Vec::as_slice).unwrap_or_default();
        if self.line.is_horizontal {
            self.intrinsic_cross_axis_size(scope, children, width, &overflow)
        } else {
            self.max_intrinsic_main_axis_size(scope, children, width)
        }
    }

    fn max_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[Vec<&dyn IntrinsicMeasurable>],
        height: i32,
    ) -> i32 {
        let overflow = self.intrinsic_overflow(measurables, Constraints::new(0, Constraints::Infinity, 0, height));
        let children = measurables.first().map(Vec::as_slice).unwrap_or_default();
        if self.line.is_horizontal {
            self.max_intrinsic_main_axis_size(scope, children, height)
        } else {
            self.intrinsic_cross_axis_size(scope, children, height, &overflow)
        }
    }
}

impl FlowMeasurePolicy {
    fn intrinsic_overflow<'a>(
        &'a self,
        measurables: &'a [Vec<&'a dyn IntrinsicMeasurable>],
        constraints: Constraints,
    ) -> OverflowMeasurables<'a, &'a dyn IntrinsicMeasurable> {
        self.overflow.set_overflow_intrinsic_measurables(
            measurables.get(1).and_then(|l| l.first().copied()),
            measurables.get(2).and_then(|l| l.first().copied()),
            self.line.is_horizontal,
            constraints,
        )
    }

    fn min_intrinsic_main_axis_size(
        &self,
        scope: &IntrinsicMeasureScope,
        children: &[&dyn IntrinsicMeasurable],
        cross_axis_available: i32,
        overflow: &OverflowMeasurables<&dyn IntrinsicMeasurable>,
    ) -> i32 {
        let h = self.line.is_horizontal;
        min_intrinsic_main_axis_size(
            children,
            |m, _, size| if h { m.min_intrinsic_width(size) } else { m.min_intrinsic_height(size) },
            |m, _, size| if h { m.min_intrinsic_height(size) } else { m.min_intrinsic_width(size) },
            cross_axis_available,
            scope.round_to_px(self.main_axis_spacing),
            scope.round_to_px(self.cross_axis_arrangement_spacing),
            self.max_items_in_main_axis,
            self.max_lines,
            overflow,
        )
    }

    fn max_intrinsic_main_axis_size(
        &self,
        scope: &IntrinsicMeasureScope,
        children: &[&dyn IntrinsicMeasurable],
        cross_axis_available: i32,
    ) -> i32 {
        let h = self.line.is_horizontal;
        max_intrinsic_main_axis_size(
            children,
            |m, _, size| if h { m.max_intrinsic_width(size) } else { m.max_intrinsic_height(size) },
            cross_axis_available,
            scope.round_to_px(self.main_axis_spacing),
            self.max_items_in_main_axis,
        )
    }

    fn intrinsic_cross_axis_size(
        &self,
        scope: &IntrinsicMeasureScope,
        children: &[&dyn IntrinsicMeasurable],
        main_axis_available: i32,
        overflow: &OverflowMeasurables<&dyn IntrinsicMeasurable>,
    ) -> i32 {
        let h = self.line.is_horizontal;
        intrinsic_cross_axis_size(
            children,
            |m, _, size| if h { m.min_intrinsic_width(size) } else { m.min_intrinsic_height(size) },
            |m, _, size| if h { m.min_intrinsic_height(size) } else { m.min_intrinsic_width(size) },
            main_axis_available,
            scope.round_to_px(self.main_axis_spacing),
            scope.round_to_px(self.cross_axis_arrangement_spacing),
            self.max_items_in_main_axis,
            self.max_lines,
            overflow,
        )
        .0
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Intrinsics

/// The widest line's size along, each line as many children as it may have (Compose's
/// `maxIntrinsicMainAxisSize`).
fn max_intrinsic_main_axis_size(
    children: &[&dyn IntrinsicMeasurable],
    main_axis_size: impl Fn(&dyn IntrinsicMeasurable, usize, i32) -> i32,
    cross_axis_available: i32,
    main_axis_spacing: i32,
    max_items_in_main_axis: i32,
) -> i32 {
    let mut fixed_space = 0;
    let mut current_fixed_space = 0;
    let mut last_break = 0i32;
    for (index, child) in children.iter().enumerate() {
        let size = main_axis_size(*child, index, cross_axis_available) + main_axis_spacing;
        if index as i32 + 1 - last_break == max_items_in_main_axis || index + 1 == children.len() {
            last_break = index as i32;
            current_fixed_space += size;
            current_fixed_space -= main_axis_spacing; // (no spacing after a line's last child)
            fixed_space = fixed_space.max(current_fixed_space);
            current_fixed_space = 0;
        } else {
            current_fixed_space += size;
        }
    }
    fixed_space
}

/// The least size along that keeps the layout within `cross_axis_available` across with all the children that
/// can show shown, by a binary search (Compose's `minIntrinsicMainAxisSize`).
#[allow(clippy::too_many_arguments)]
fn min_intrinsic_main_axis_size<M: Clone>(
    children: &[&dyn IntrinsicMeasurable],
    main_axis_size: impl Fn(&dyn IntrinsicMeasurable, usize, i32) -> i32,
    cross_axis_size: impl Fn(&dyn IntrinsicMeasurable, usize, i32) -> i32,
    cross_axis_available: i32,
    main_axis_spacing: i32,
    cross_axis_spacing: i32,
    max_items_in_main_axis: i32,
    max_lines: i32,
    overflow: &OverflowMeasurables<M>,
) -> i32 {
    if children.is_empty() {
        return 0;
    }
    let mut main_axis_sizes = vec![0; children.len()];
    let mut cross_axis_sizes = vec![0; children.len()];
    for (index, child) in children.iter().enumerate() {
        let main = main_axis_size(*child, index, cross_axis_available);
        main_axis_sizes[index] = main;
        cross_axis_sizes[index] = cross_axis_size(*child, index, main);
    }

    let mut max_items_that_can_be_shown = if max_lines != i32::MAX && max_items_in_main_axis != i32::MAX {
        max_items_in_main_axis.wrapping_mul(max_lines)
    } else {
        i32::MAX
    };
    let count = children.len() as i32;
    let kind = overflow.state.kind;
    let must_have_ellipsis = (max_items_that_can_be_shown < count
        && (kind == OverflowType::ExpandIndicator || kind == OverflowType::ExpandOrCollapseIndicator))
        || (max_items_that_can_be_shown >= count
            && max_lines >= overflow.state.min_lines_to_show_collapse
            && kind == OverflowType::ExpandOrCollapseIndicator);
    max_items_that_can_be_shown -= if must_have_ellipsis { 1 } else { 0 };
    max_items_that_can_be_shown = max_items_that_can_be_shown.min(count);
    let max_main_axis_size = main_axis_sizes.iter().sum::<i32>() + (count - 1) * main_axis_spacing;
    let mut main_axis_used = max_main_axis_size;
    let mut cross_axis_used = *cross_axis_sizes.iter().max().unwrap();

    let minimum_item_size = *main_axis_sizes.iter().max().unwrap();
    let mut low = minimum_item_size;
    let mut high = max_main_axis_size;
    while low <= high {
        if cross_axis_used == cross_axis_available {
            return main_axis_used;
        }
        let mid = (low + high) / 2;
        main_axis_used = mid;
        let (cross, item_shown) = intrinsic_cross_axis_size(
            children,
            |_, index, _| main_axis_sizes[index],
            |_, index, _| cross_axis_sizes[index],
            main_axis_used,
            main_axis_spacing,
            cross_axis_spacing,
            max_items_in_main_axis,
            max_lines,
            overflow,
        );
        cross_axis_used = cross;
        if cross_axis_used > cross_axis_available || item_shown < max_items_that_can_be_shown {
            low = mid + 1;
            if low > high {
                return low;
            }
        } else if cross_axis_used < cross_axis_available {
            high = mid - 1;
        } else {
            return main_axis_used;
        }
    }
    main_axis_used
}

/// The size across of the lines the children make in `main_axis_available`, and how many show (Compose's
/// `intrinsicCrossAxisSize`).
#[allow(clippy::too_many_arguments)]
fn intrinsic_cross_axis_size<M: Clone>(
    children: &[&dyn IntrinsicMeasurable],
    main_axis_size: impl Fn(&dyn IntrinsicMeasurable, usize, i32) -> i32,
    cross_axis_size: impl Fn(&dyn IntrinsicMeasurable, usize, i32) -> i32,
    main_axis_available: i32,
    main_axis_spacing: i32,
    cross_axis_spacing: i32,
    max_items_in_main_axis: i32,
    max_lines: i32,
    overflow: &OverflowMeasurables<M>,
) -> (i32, i32) {
    if children.is_empty() {
        return (0, 0);
    }
    let building_blocks = FlowLayoutBuildingBlocks {
        max_items_in_main_axis,
        overflow,
        max_lines,
        constraints: OrientationIndependentConstraints::new(0, main_axis_available, 0, Constraints::Infinity),
        main_axis_spacing,
        cross_axis_spacing,
    };
    let mut next_child = children.first().copied();
    let mut next_cross_axis_size = next_child.map_or(0, |c| cross_axis_size(c, 0, main_axis_available));
    let mut next_main_axis_size = next_child.map_or(0, |c| main_axis_size(c, 0, next_cross_axis_size));

    let mut remaining = main_axis_available;
    let mut current_cross_axis_size = 0;
    let mut total_cross_axis_size = 0;
    let mut last_break = 0i32;
    let mut line_index = 0;

    let wrap_info = building_blocks.get_wrap_info(
        children.len() > 1,
        0,
        (remaining, Constraints::Infinity),
        next_child.map(|_| (next_main_axis_size, next_cross_axis_size)),
        line_index,
        total_cross_axis_size,
        current_cross_axis_size,
        false,
        false,
    );
    if wrap_info.is_last_item_in_container {
        let size = overflow.state.ellipsis_size(next_child.is_some(), 0, 0).map_or(0, |s| s.1);
        return (size, 0);
    }

    let mut no_of_items_shown = 0;
    for index in 0..children.len() {
        let child_cross_axis_size = next_cross_axis_size;
        let child_main_axis_size = next_main_axis_size;
        remaining -= child_main_axis_size;
        no_of_items_shown = index as i32 + 1;
        current_cross_axis_size = current_cross_axis_size.max(child_cross_axis_size);

        // (looking ahead)
        next_child = children.get(index + 1).copied();
        next_cross_axis_size = next_child.map_or(0, |c| cross_axis_size(c, index + 1, main_axis_available));
        next_main_axis_size =
            next_child.map_or(0, |c| main_axis_size(c, index + 1, next_cross_axis_size) + main_axis_spacing);

        let wrap_info = building_blocks.get_wrap_info(
            index + 2 < children.len(),
            (index as i32 + 1) - last_break,
            (remaining, Constraints::Infinity),
            next_child.map(|_| (next_main_axis_size, next_cross_axis_size)),
            line_index,
            total_cross_axis_size,
            current_cross_axis_size,
            false,
            false,
        );
        if wrap_info.is_last_item_in_line {
            total_cross_axis_size += current_cross_axis_size + cross_axis_spacing;
            let ellipsis_wrap_info = building_blocks.get_wrap_ellipsis_info(
                wrap_info,
                next_child.is_some(),
                line_index,
                total_cross_axis_size,
                remaining,
                (index as i32 + 1) - last_break,
            );
            current_cross_axis_size = 0;
            remaining = main_axis_available;
            last_break = index as i32 + 1;
            next_main_axis_size -= main_axis_spacing;
            line_index += 1;
            if wrap_info.is_last_item_in_container {
                if let Some(info) = ellipsis_wrap_info
                    && !info.place_ellipsis_on_last_content_line
                {
                    total_cross_axis_size += info.ellipsis_size.1 + cross_axis_spacing;
                }
                break;
            }
        }
    }
    // (no spacing after the last line)
    total_cross_axis_size -= cross_axis_spacing;
    (total_cross_axis_size, no_of_items_shown)
}

// ---------------------------------------------------------------------------------------------------------------
// Breaking the children into lines (Compose's `breakDownItems`)

/// Where a line's next child would be and how much room it has (Compose's FlowLineInfo): what a contextual flow
/// layout's item is composed knowing.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct FlowLineInfo {
    pub(crate) line_index: i32,
    pub(crate) position_in_line: i32,
    pub(crate) max_main_axis_size: Dp,
    pub(crate) max_cross_axis_size: Dp,
}

/// The children a flow layout breaks into lines, one at a time (Compose's `Iterator<Measurable>`, or a contextual
/// layout's ContextualFlowItemIterator composing each as it is asked for, knowing its line).
pub(crate) trait FlowItems<M> {
    fn has_next(&self) -> bool;
    /// The next child, none past the last (Compose's `safeNext`).
    fn next_item(&mut self, info: Option<&FlowLineInfo>) -> Option<M>;
    /// Whether its items are composed knowing their line (Compose's `is ContextualFlowItemIterator`).
    fn is_contextual(&self) -> bool {
        false
    }
}

/// A flow layout's children, measured as they are.
struct PlainItems<'l, 'a> {
    list: &'l [&'a dyn Measurable],
    next: usize,
}

impl<'a> FlowItems<&'a dyn Measurable> for PlainItems<'_, 'a> {
    fn has_next(&self) -> bool {
        self.next < self.list.len()
    }
    fn next_item(&mut self, _: Option<&FlowLineInfo>) -> Option<&'a dyn Measurable> {
        let item = self.list.get(self.next).copied();
        self.next += 1;
        item
    }
}

/// Breaks the children into lines as they fit, measures each line as a Row or a Column would, then places the
/// lines across by the cross axis arrangement (Compose's `MeasureScope.breakDownItems`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn break_down_items<M: MeasurableRef>(
    scope: &MeasureScope,
    line_policy: &FlowLine,
    items_iter: &mut dyn FlowItems<M>,
    main_axis_spacing_dp: Dp,
    cross_axis_spacing_dp: Dp,
    constraints: OrientationIndependentConstraints,
    max_items_in_main_axis: i32,
    max_lines: i32,
    overflow: &OverflowMeasurables<M>,
) -> MeasureResult {
    let mut items: Vec<MeasureResult> = Vec::new();
    let main_axis_max = constraints.main_axis_max();
    let main_axis_min = constraints.main_axis_min();
    let cross_axis_max = constraints.cross_axis_max();
    let mut placeables: Vec<Option<Placeable>> = Vec::new();
    let mut measurables: Vec<M> = Vec::new();

    let spacing = scope.to_px(main_axis_spacing_dp).ceil() as i32;
    let cross_axis_spacing = scope.to_px(cross_axis_spacing_dp).ceil() as i32;
    let subset_constraints = OrientationIndependentConstraints::new(0, main_axis_max, 0, cross_axis_max);
    let measure_constraints =
        subset_constraints.copy(Some(0), None, None, None).to_box_constraints(line_policy.orientation());

    let mut index = 0usize;
    let mut placeable_item: Option<Placeable> = None;
    let mut line_index = 0i32;
    let mut left_over = main_axis_max;
    let mut left_over_cross_axis = cross_axis_max;
    let mut line_info = items_iter.is_contextual().then(|| FlowLineInfo {
        line_index,
        position_in_line: 0,
        max_main_axis_size: scope.to_dp(left_over as f32),
        max_cross_axis_size: scope.to_dp(left_over_cross_axis as f32),
    });

    let mut measurable: Option<M> = if items_iter.has_next() { items_iter.next_item(line_info.as_ref()) } else { None };
    let mut next_size = measurable
        .as_ref()
        .map(|m| measure_and_cache(m.get(), line_policy, measure_constraints, |p| placeable_item = p));
    let mut next_main_axis_size = next_size.map(|s| s.0);
    let mut next_cross_axis_size = next_size.map(|s| s.1);

    let mut start_break_line_index = 0usize;
    let mut end_break_line_list: Vec<usize> = Vec::new();
    let mut cross_axis_sizes: Vec<i32> = Vec::new();
    let mut cross_axis_fill_enabled_lines: Vec<i32> = Vec::new();

    let building_blocks = FlowLayoutBuildingBlocks {
        max_items_in_main_axis,
        overflow,
        max_lines,
        constraints,
        main_axis_spacing: spacing,
        cross_axis_spacing,
    };
    let mut ellipsis_wrap_info: Option<WrapEllipsisInfo<M>> = None;
    let mut wrap_info = building_blocks.get_wrap_info(
        items_iter.has_next(),
        0,
        (left_over, left_over_cross_axis),
        next_size,
        0,
        0,
        0,
        false,
        false,
    );
    if wrap_info.is_last_item_in_container {
        ellipsis_wrap_info =
            building_blocks.get_wrap_ellipsis_info(wrap_info, next_size.is_some(), -1, 0, left_over, 0);
    }

    // The size along each line is measured with at least (the widest line's).
    let mut main_axis_total_size = main_axis_min;
    let mut cross_axis_total_size = 0;
    let mut current_line_main_axis_size = 0;
    let mut current_line_cross_axis_size = 0;
    let mut current_line_cross_axis_fill_enabled = false;
    while !wrap_info.is_last_item_in_container {
        let Some(item) = measurable.take() else { break };
        let item_main_axis_size = next_main_axis_size.expect("measured");
        let item_cross_axis_size = next_cross_axis_size.expect("measured");
        current_line_main_axis_size += item_main_axis_size;
        current_line_cross_axis_size = current_line_cross_axis_size.max(item_cross_axis_size);
        left_over -= item_main_axis_size;
        overflow.state.item_shown.set(index as i32 + 1);
        if parent_data_of(item.get()).and_then(|d| d.flow_layout_data).is_some() {
            current_line_cross_axis_fill_enabled = true;
        }
        measurables.push(item);
        if placeables.len() <= index {
            placeables.resize(index + 1, None);
        }
        placeables[index] = placeable_item.take();

        let next_index_in_line = (index + 1 - start_break_line_index) as i32;
        let will_fit_line = next_index_in_line < max_items_in_main_axis;
        if let Some(info) = line_info.as_mut() {
            *info = FlowLineInfo {
                line_index: if will_fit_line { line_index } else { line_index + 1 },
                position_in_line: if will_fit_line { next_index_in_line } else { 0 },
                max_main_axis_size: scope
                    .to_dp(if will_fit_line { (left_over - spacing).max(0) } else { main_axis_max } as f32),
                max_cross_axis_size: scope.to_dp(if will_fit_line {
                    left_over_cross_axis
                } else {
                    (left_over_cross_axis - current_line_cross_axis_size - cross_axis_spacing).max(0)
                } as f32),
            };
        }

        measurable = if items_iter.has_next() { items_iter.next_item(line_info.as_ref()) } else { None };
        placeable_item = None;
        next_size = measurable
            .as_ref()
            .map(|m| measure_and_cache(m.get(), line_policy, measure_constraints, |p| placeable_item = p));
        next_main_axis_size = next_size.map(|s| s.0 + spacing);
        next_cross_axis_size = next_size.map(|s| s.1);

        wrap_info = building_blocks.get_wrap_info(
            items_iter.has_next(),
            next_index_in_line,
            (left_over, left_over_cross_axis),
            next_size.map(|_| (next_main_axis_size.unwrap(), next_cross_axis_size.unwrap())),
            line_index,
            cross_axis_total_size,
            current_line_cross_axis_size,
            false,
            false,
        );
        if wrap_info.is_last_item_in_line {
            main_axis_total_size = main_axis_total_size.max(current_line_main_axis_size);
            main_axis_total_size = main_axis_total_size.min(main_axis_max);
            cross_axis_total_size += current_line_cross_axis_size;
            ellipsis_wrap_info = building_blocks.get_wrap_ellipsis_info(
                wrap_info,
                next_size.is_some(),
                line_index,
                cross_axis_total_size,
                left_over,
                (index + 1 - start_break_line_index) as i32,
            );
            cross_axis_sizes.push(current_line_cross_axis_size);
            if current_line_cross_axis_fill_enabled {
                cross_axis_fill_enabled_lines.push(line_index);
            }
            current_line_cross_axis_fill_enabled = false;
            left_over = main_axis_max;
            left_over_cross_axis = cross_axis_max - cross_axis_total_size - cross_axis_spacing;
            start_break_line_index = index + 1;
            end_break_line_list.push(index + 1);
            current_line_main_axis_size = 0;
            current_line_cross_axis_size = 0;
            // (spacing only between the items of a line)
            next_main_axis_size = next_main_axis_size.map(|s| s - spacing);
            line_index += 1;
            cross_axis_total_size += cross_axis_spacing;
        }
        index += 1;
    }

    if let Some(info) = ellipsis_wrap_info {
        measurables.push(info.ellipsis);
        placeables.resize(measurables.len(), None);
        placeables[measurables.len() - 1] = info.placeable;
        let last_line = end_break_line_list.len() as i32 - 1;
        if info.place_ellipsis_on_last_content_line {
            let line = last_line as usize;
            let last_line_cross_axis = cross_axis_sizes[line];
            cross_axis_sizes[line] = last_line_cross_axis.max(info.ellipsis_size.1);
            let last = end_break_line_list.len() - 1;
            end_break_line_list[last] += 1;
        } else {
            cross_axis_sizes.push(info.ellipsis_size.1);
            // (Compose's `endBreakLineList.last()` throws when no line came before: the indicator alone, here)
            let last = end_break_line_list.last().copied().unwrap_or(0);
            end_break_line_list.push(last + 1);
        }
    }

    placeables.resize(measurables.len(), None);
    let cross_axis_offsets: CrossAxisOffsets = Rc::new(RefCell::new(vec![0; end_break_line_list.len()]));
    let mut cross_axis_sizes_array = vec![0; end_break_line_list.len()];
    cross_axis_total_size = 0;

    let mut start_index = 0;
    for (current_line_index, &end_index) in end_break_line_list.iter().enumerate() {
        let cross_axis_size = cross_axis_sizes[current_line_index];
        let cross_axis_max_size = if cross_axis_fill_enabled_lines.contains(&(current_line_index as i32)) {
            cross_axis_size
        } else if subset_constraints.cross_axis_max() == Constraints::Infinity {
            Constraints::Infinity
        } else {
            subset_constraints.cross_axis_max() - cross_axis_total_size
        };
        let result = measure_row_column(
            line_policy,
            main_axis_total_size,
            subset_constraints.cross_axis_min(),
            subset_constraints.main_axis_max(),
            cross_axis_max_size,
            spacing,
            scope,
            &measurables,
            &mut placeables,
            start_index,
            end_index,
            Some(cross_axis_offsets.clone()),
            current_line_index,
        );
        let (main_axis_size, cross_axis_size) =
            if line_policy.is_horizontal { (result.width, result.height) } else { (result.height, result.width) };
        cross_axis_sizes_array[current_line_index] = cross_axis_size;
        cross_axis_total_size += cross_axis_size;
        main_axis_total_size = main_axis_total_size.max(main_axis_size);
        items.push(result);
        start_index = end_index;
    }

    if items.is_empty() {
        main_axis_total_size = 0;
        cross_axis_total_size = 0;
    }

    place_helper(
        scope,
        constraints,
        main_axis_total_size,
        cross_axis_total_size,
        &cross_axis_sizes_array,
        items,
        line_policy,
        cross_axis_offsets,
    )
}

/// Measures a child once, at its preferred size, if it has no weight and doesn't fill its line across (its
/// placeable kept for its line); else its minimum intrinsic sizes, as its size depends on its line (Compose's
/// `measureAndCache`).
fn measure_and_cache(
    measurable: &dyn Measurable,
    line: &FlowLine,
    constraints: Constraints,
    store_placeable: impl FnOnce(Option<Placeable>),
) -> (i32, i32) {
    let data = parent_data_of(measurable);
    let weight = data.as_ref().map_or(0.0, |d| d.weight);
    let fill = data.as_ref().and_then(|d| d.flow_layout_data);
    if weight == 0.0 && fill.is_none() {
        let placeable = measurable.measure(constraints);
        let sizes = (line.main_axis_size(&placeable), line.cross_axis_size(&placeable));
        store_placeable(Some(placeable));
        sizes
    } else {
        let main = main_axis_min(measurable, line.is_horizontal, Constraints::Infinity);
        let cross = cross_axis_min(measurable, line.is_horizontal, main);
        (main, cross)
    }
}

/// Compose's `IntrinsicMeasurable.mainAxisMin`.
pub(crate) fn main_axis_min(m: &dyn IntrinsicMeasurable, is_horizontal: bool, cross_axis_size: i32) -> i32 {
    if is_horizontal { m.min_intrinsic_width(cross_axis_size) } else { m.min_intrinsic_height(cross_axis_size) }
}

/// Compose's `IntrinsicMeasurable.crossAxisMin`.
pub(crate) fn cross_axis_min(m: &dyn IntrinsicMeasurable, is_horizontal: bool, main_axis_size: i32) -> i32 {
    if is_horizontal { m.min_intrinsic_height(main_axis_size) } else { m.min_intrinsic_width(main_axis_size) }
}

/// Arranges the lines across and makes the layout, the lines placed by their own results (Compose's
/// `MeasureScope.placeHelper`).
#[allow(clippy::too_many_arguments)]
fn place_helper(
    scope: &MeasureScope,
    constraints: OrientationIndependentConstraints,
    main_axis_total_size: i32,
    cross_axis_total_size: i32,
    cross_axis_sizes: &[i32],
    items: Vec<MeasureResult>,
    line: &FlowLine,
    out_position: CrossAxisOffsets,
) -> MeasureResult {
    let mut total_cross_axis_size = cross_axis_total_size;
    let lines = items.len() as i32;
    {
        let mut out = out_position.borrow_mut();
        if line.is_horizontal {
            let arrangement = &line.vertical_arrangement;
            total_cross_axis_size += scope.round_to_px(arrangement.spacing()) * (lines - 1);
            total_cross_axis_size =
                coerce_in(total_cross_axis_size, constraints.cross_axis_min(), constraints.cross_axis_max());
            arrangement.arrange(scope, total_cross_axis_size, cross_axis_sizes, &mut out);
        } else {
            let arrangement = &line.horizontal_arrangement;
            total_cross_axis_size += scope.round_to_px(arrangement.spacing()) * (lines - 1);
            total_cross_axis_size =
                coerce_in(total_cross_axis_size, constraints.cross_axis_min(), constraints.cross_axis_max());
            arrangement.arrange(scope, total_cross_axis_size, cross_axis_sizes, scope.layout_direction(), &mut out);
        }
    }
    let final_main_axis_total_size =
        coerce_in(main_axis_total_size, constraints.main_axis_min(), constraints.main_axis_max());
    let (layout_width, layout_height) = if line.is_horizontal {
        (final_main_axis_total_size, total_cross_axis_size)
    } else {
        (total_cross_axis_size, final_main_axis_total_size)
    };
    scope.layout(layout_width, layout_height, move |p| {
        for result in &items {
            result.place_children(p);
        }
    })
}

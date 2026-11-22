//! `Grid`, as Compose's (foundation-layout's experimental `Grid`, 1.12.1): children in rows and columns of
//! explicit tracks (fixed, percentage, flexible, min-max, content-sized) and the implicit ones they need, placed
//! where they ask (by index, range or named area) or automatically by the flow, spanning tracks, aligned in their
//! cells (Compose's GridMeasurePolicy, its intrinsics the default ones).

use super::{round, same_dp};
use kompoz_graphics::{Density, Dp, IntSize, LayoutDirection};
use kompoz_runtime::kt::KtToString;
use kompoz_runtime::{Memoized, kompozable, named};
use kompoz_ui::layout::{ParentDataModifierNode, SlotKey, layout_into};
use kompoz_ui::{
    Alignment, Constraints, Measurable, MeasurePolicy, MeasureResult, MeasureScope, Modifier, ModifierNode,
    ModifierNodeElement, Placeable, node_capabilities,
};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::ops::{Deref, RangeInclusive};
use std::rc::Rc;

/// A layout of its children in a grid (Compose's `Grid`): `config` defines its columns, rows, gaps, flow and named
/// areas, run as it measures (with the constraints it is measured in, and observed: a state it reads measures the
/// Grid again when it changes). Children say where they go with [`GridScopeModifiers`] (`grid_item`, …); the
/// others are placed in the next free cells along the flow, the grid growing implicit (`Auto`) tracks for what
/// falls outside the explicit ones.
#[kompozable(inline)]
pub fn Grid(
    #[memoized] config: impl Fn(&mut GridConfigurationScope) + 'static,
    #[default(Modifier)] modifier: Modifier,
    content: impl FnOnce(),
) {
    layout_into(modifier, grid_measure_policy(config), false, content);
}

/// A Grid's policy. Compose keeps one policy reading `rememberUpdatedState(config)` as it measures, measured again
/// when a config unequal to the last one comes; a closure has no `==` here (an updated state of it would change at
/// every composition), so the policy is kept by the memoized config's identity, Compose's equality of a lambda: the
/// same closure (the same captures) keeps it, and the Grid isn't measured again; a new one makes a new policy, which
/// measures it again (what Compose's state read does).
#[track_caller]
fn grid_measure_policy<F: Fn(&mut GridConfigurationScope) + 'static>(config: Memoized<F>) -> Rc<dyn MeasurePolicy> {
    config.remember(|config| Rc::new(GridMeasurePolicy { config }) as Rc<dyn MeasurePolicy>)
}

// ---------------------------------------------------------------------------------------------------------------
// The children's modifiers (Compose's GridScope)

/// Compose's `GridScope`, the receiver of a Grid's content there: its constants. Its modifiers are
/// [`GridScopeModifiers`]', usable anywhere (a closure has no receiver; see the module's doc).
pub enum GridScope {}

impl GridScope {
    /// The greatest row or column index (Compose's `GridScope.MaxGridIndex`): named areas pack theirs in 16 bits.
    pub const MaxGridIndex: i32 = 1000;

    /// A row or column left to the flow to find (Compose's `GridScope.GridIndexUnspecified`).
    pub const GridIndexUnspecified: i32 = 0;
}

/// The modifiers a child of a [`Grid`] tells it where it goes with (Compose's `GridScope.gridItem` overloads).
/// Indices are 1-based: a positive one counts from the first track (one past the explicit ones makes implicit
/// tracks), a negative one from the last explicit track (-1 is the last; one before the first is left to the flow).
#[named(trait GridScopeModifiers)]
impl Modifier {
    /// In row `row` and column `column`, spanning `row_span` rows and `column_span` columns, aligned by `alignment`
    /// in its cells (Compose's `gridItem(row, column, rowSpan, columnSpan, alignment)`). A row or column left
    /// [`GridScope::GridIndexUnspecified`] is found: with a row (or a column) given, its first free cells in it;
    /// with neither, the flow's next free ones. Without this modifier, a child is placed by the flow. Panics on an index outside ±[`GridScope::MaxGridIndex`] or a span under 1.
    /// Kotlin: `gridItem(row: Int = GridIndexUnspecified, column: Int = GridIndexUnspecified, rowSpan: Int = 1, columnSpan: Int = 1, alignment: Alignment = Alignment.TopStart)`
    fn grid_item(
        self,
        #[default(GridScope::GridIndexUnspecified)] row: i32,
        #[default(GridScope::GridIndexUnspecified)] column: i32,
        #[default(1)] row_span: i32,
        #[default(1)] column_span: i32,
        #[default(Alignment::TopStart)] alignment: Alignment,
    ) -> Modifier {
        grid_item(self, row, column, row_span, column_span, alignment)
    }

    /// [`grid_item`](GridScopeModifiers::grid_item) with all its parameters, for plain code.
    fn grid_item_with(self, row: i32, column: i32, row_span: i32, column_span: i32, alignment: Alignment) -> Modifier {
        grid_item(self, row, column, row_span, column_span, alignment)
    }

    /// In the rows `rows` and the columns `columns` (their first the index, their length the span), aligned by
    /// `alignment` (Compose's `gridItem(rows, columns, alignment)`): `rows = 2..=3` is `row = 2, row_span = 2`.
    /// Panics on an empty range, and as [`grid_item`](GridScopeModifiers::grid_item) does.
    /// Kotlin: `gridItem(rows: IntRange, columns: IntRange, alignment: Alignment = Alignment.TopStart)`
    fn grid_item_ranges(
        self,
        rows: RangeInclusive<i32>,
        columns: RangeInclusive<i32>,
        #[default(Alignment::TopStart)] alignment: Alignment,
    ) -> Modifier {
        grid_item_ranges(self, rows, columns, alignment)
    }

    /// [`grid_item_ranges`](GridScopeModifiers::grid_item_ranges) with all its parameters, for plain code.
    fn grid_item_ranges_with(
        self,
        rows: RangeInclusive<i32>,
        columns: RangeInclusive<i32>,
        alignment: Alignment,
    ) -> Modifier {
        grid_item_ranges(self, rows, columns, alignment)
    }

    /// In the area named `area_id` by the Grid's config ([`GridConfigurationScope::area`]), aligned by `alignment`
    /// (Compose's `gridItem(areaId, alignment)`). Items of one 2D area are one over the other (the later on top);
    /// in a 1D area (a row or a column only) they flow into its next free cells. An id no area has places it by the
    /// flow. Ids are equal when their values are (of the same type: `"a"` isn't `String::from("a")`).
    /// Kotlin: `gridItem(areaId: Any, alignment: Alignment = Alignment.TopStart)`
    fn grid_item_area(
        self,
        area_id: impl Hash + Eq + 'static,
        #[default(Alignment::TopStart)] alignment: Alignment,
    ) -> Modifier {
        grid_item_area(self, SlotKey::new(area_id), alignment)
    }

    /// [`grid_item_area`](GridScopeModifiers::grid_item_area) with all its parameters, for plain code.
    fn grid_item_area_with(self, area_id: impl Hash + Eq + 'static, alignment: Alignment) -> Modifier {
        grid_item_area(self, SlotKey::new(area_id), alignment)
    }
}

/// Compose's `GridScopeInstance.gridItem(row, column, rowSpan, columnSpan, alignment)`.
fn grid_item(
    modifier: Modifier,
    row: i32,
    column: i32,
    row_span: i32,
    column_span: i32,
    alignment: Alignment,
) -> Modifier {
    let max = GridScope::MaxGridIndex;
    if row != GridScope::GridIndexUnspecified {
        assert!((-max..=max).contains(&row), "row must be between -{max} and {max}");
    }
    if column != GridScope::GridIndexUnspecified {
        assert!((-max..=max).contains(&column), "column must be between -{max} and {max}");
    }
    assert!(row_span > 0, "rowSpan must be > 0");
    assert!(column_span > 0, "columnSpan must be > 0");
    modifier.then_element(GridItemElement { area_id: None, row, column, row_span, column_span, alignment })
}

/// Compose's `GridScopeInstance.gridItem(rows, columns, alignment)`.
fn grid_item_ranges(
    modifier: Modifier,
    rows: RangeInclusive<i32>,
    columns: RangeInclusive<i32>,
    alignment: Alignment,
) -> Modifier {
    let (row, row_span, column, column_span) = from_ranges(&rows, &columns);
    grid_item(modifier, row, column, row_span, column_span, alignment)
}

/// Compose's `GridScopeInstance.gridItem(areaId, alignment)`.
fn grid_item_area(modifier: Modifier, area_id: SlotKey, alignment: Alignment) -> Modifier {
    modifier.then_element(GridItemElement {
        area_id: Some(area_id),
        row: GridScope::GridIndexUnspecified,
        column: GridScope::GridIndexUnspecified,
        row_span: 1,
        column_span: 1,
        alignment,
    })
}

/// A row range's and a column range's first index and length (row, row span, column, column span); panics on an
/// empty one (Kotlin's `IntRange` written as it writes it).
fn from_ranges(rows: &RangeInclusive<i32>, columns: &RangeInclusive<i32>) -> (i32, i32, i32, i32) {
    assert!(!rows.is_empty(), "Row range ({}..{}) cannot be empty", rows.start(), rows.end());
    assert!(!columns.is_empty(), "Column range ({}..{}) cannot be empty", columns.start(), columns.end());
    (
        *rows.start(),
        rows.end().wrapping_sub(*rows.start()).wrapping_add(1),
        *columns.start(),
        columns.end().wrapping_sub(*columns.start()).wrapping_add(1),
    )
}

/// The element of `grid_item` (Compose's GridItemElement).
#[derive(PartialEq)]
struct GridItemElement {
    area_id: Option<SlotKey>,
    row: i32,
    column: i32,
    row_span: i32,
    column_span: i32,
    alignment: Alignment,
}

impl ModifierNodeElement for GridItemElement {
    type Node = GridItemNode;
    fn create(&self) -> GridItemNode {
        GridItemNode(self.data())
    }
    fn update(&self, node: &mut GridItemNode) {
        node.0 = self.data();
    }
    fn name(&self) -> &'static str {
        "gridItem"
    }
}

impl GridItemElement {
    fn data(&self) -> GridItemData {
        GridItemData {
            area_id: self.area_id.clone(),
            row: self.row,
            column: self.column,
            row_span: self.row_span,
            column_span: self.column_span,
            alignment: self.alignment.clone(),
        }
    }
}

/// What a Grid's child tells it (Compose's GridItemNode, which is its own parent data): a named area, or a row and
/// a column (1-based, or unspecified) and spans; its alignment.
#[derive(Clone, Debug)]
struct GridItemData {
    area_id: Option<SlotKey>,
    row: i32,
    column: i32,
    row_span: i32,
    column_span: i32,
    alignment: Alignment,
}

/// The node of `grid_item` (Compose's GridItemNode): it says itself, whatever the nodes after it said.
struct GridItemNode(GridItemData);

impl ModifierNode for GridItemNode {
    node_capabilities!(parent_data);
}

impl ParentDataModifierNode for GridItemNode {
    fn modify_parent_data(&mut self, _: &Density, _: Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        Some(Box::new(self.0.clone()))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The configuration (Compose's GridConfigurationScope and GridConfigurationScopeImpl)

/// What a [`Grid`]'s config is run with as it measures (Compose's `GridConfigurationScope`, a `Density`: it derefs
/// to it): the constraints it is measured in, and what defines the grid. Tracks are added in the order `column` and
/// `row` are called; the gaps' last call wins, each way.
pub struct GridConfigurationScope {
    scope_density: Density,
    constraints: Constraints,
    column_specs: Vec<GridTrackSize>,
    row_specs: Vec<GridTrackSize>,
    /// The named areas, packed as Compose's: row, column, row span and column span in 16 bits each.
    named_areas: HashMap<SlotKey, u64>,
    column_gap: Dp,
    row_gap: Dp,
    flow: GridFlow,
}

impl Deref for GridConfigurationScope {
    type Target = Density;
    fn deref(&self) -> &Density {
        &self.scope_density
    }
}

#[named]
impl GridConfigurationScope {
    /// The constraints its parent measures the Grid in (Compose's `constraints`), for a grid that adapts to them.
    pub fn constraints(&self) -> Constraints {
        self.constraints
    }

    /// How the children that don't say where they go are placed (Compose's `flow`), [`GridFlow::Row`] by default.
    pub fn flow(&self) -> GridFlow {
        self.flow
    }

    /// Sets [`flow`](GridConfigurationScope::flow) (Compose's `flow = …`).
    pub fn set_flow(&mut self, flow: GridFlow) {
        self.flow = flow;
    }

    /// A column of `size` (Compose's `column` overloads, told apart by their argument's type, as Kotlin does): a
    /// [`Dp`] wide ([`GridTrackSize::Fixed`]), a flexible one of an [`Fr`] weight ([`GridTrackSize::Flex`]), an
    /// `f32` percentage (0 to 1) of the room ([`GridTrackSize::Percentage`]), or a [`GridTrackSize`].
    pub fn column(&mut self, size: impl GridTrackSizeArgs) {
        self.column_specs.push(size.grid_track_size());
    }

    /// A row of `size` (Compose's `row` overloads, as [`column`](Self::column)'s).
    pub fn row(&mut self, size: impl GridTrackSizeArgs) {
        self.row_specs.push(size.grid_track_size());
    }

    /// Adds a column of each of `specs` (Compose's `columns(vararg specs)`).
    /// Kotlin: `columns(vararg specs: GridTrackSpec)`
    pub fn columns<S: GridTrackSpec>(&mut self, specs: impl IntoIterator<Item = S>) {
        for spec in specs {
            if let Some(size) = spec.track_size() {
                self.column(size);
            }
        }
    }

    /// Adds a row of each of `specs` (Compose's `rows(vararg specs)`).
    /// Kotlin: `rows(vararg specs: GridTrackSpec)`
    pub fn rows<S: GridTrackSpec>(&mut self, specs: impl IntoIterator<Item = S>) {
        for spec in specs {
            if let Some(size) = spec.track_size() {
                self.row(size);
            }
        }
    }

    /// Names an area `area_id`: from row `row` and column `column` (1-based, negative from the last explicit one),
    /// `row_span` rows and `column_span` columns (Compose's `area(areaId, row, column, rowSpan, columnSpan)`). One
    /// of `row` and `column` left [`GridScope::GridIndexUnspecified`] makes a 1D area (a column or a row) its items
    /// flow along. Ids are equal when their values are, of the same type; naming one again moves it. Panics when
    /// both are unspecified, on an index outside ±[`GridScope::MaxGridIndex`], or a span outside 1 to it.
    /// Kotlin: `area(areaId: Any, row: Int = GridIndexUnspecified, column: Int = GridIndexUnspecified, rowSpan: Int = 1, columnSpan: Int = 1)`
    pub fn area(
        &mut self,
        area_id: impl Hash + Eq + 'static,
        #[default(GridScope::GridIndexUnspecified)] row: i32,
        #[default(GridScope::GridIndexUnspecified)] column: i32,
        #[default(1)] row_span: i32,
        #[default(1)] column_span: i32,
    ) {
        self.add_area(SlotKey::new(area_id), row, column, row_span, column_span);
    }

    /// [`area`](GridConfigurationScope::area) with all its parameters, for plain code.
    pub fn area_with(
        &mut self,
        area_id: impl Hash + Eq + 'static,
        row: i32,
        column: i32,
        row_span: i32,
        column_span: i32,
    ) {
        self.add_area(SlotKey::new(area_id), row, column, row_span, column_span);
    }

    /// Names an area `area_id` over the rows `rows` and the columns `columns` (their first the index, their length
    /// the span; Compose's `area(areaId, rows, columns)`). Panics on an empty range, and as
    /// [`area`](GridConfigurationScope::area) does.
    /// Kotlin: `area(areaId: Any, rows: IntRange, columns: IntRange)`
    pub fn area_ranges(
        &mut self,
        area_id: impl Hash + Eq + 'static,
        rows: RangeInclusive<i32>,
        columns: RangeInclusive<i32>,
    ) {
        let (row, row_span, column, column_span) = from_ranges(&rows, &columns);
        self.add_area(SlotKey::new(area_id), row, column, row_span, column_span);
    }

    /// `all` between rows and between columns (Compose's `gap(all)`). Panics on a negative gap.
    /// Kotlin: `gap(all: Dp)`
    pub fn gap(&mut self, all: Dp) {
        assert!(all.0 >= 0.0, "Gap must be non-negative");
        self.column_gap = all;
        self.row_gap = all;
    }

    /// `row` between rows, `column` between columns (Compose's `gap(row, column)`). Panics on a negative gap.
    /// Kotlin: `gap(row: Dp, column: Dp)`
    pub fn gap_rc(&mut self, row: Dp, column: Dp) {
        assert!(row.0 >= 0.0, "Row gap must be non-negative");
        assert!(column.0 >= 0.0, "Column gap must be non-negative");
        self.row_gap = row;
        self.column_gap = column;
    }

    /// `gap` between columns (Compose's `columnGap`). Panics on a negative gap.
    pub fn column_gap(&mut self, gap: Dp) {
        assert!(gap.0 >= 0.0, "Column gap must be non-negative");
        self.column_gap = gap;
    }

    /// `gap` between rows (Compose's `rowGap`). Panics on a negative gap.
    pub fn row_gap(&mut self, gap: Dp) {
        assert!(gap.0 >= 0.0, "Row gap must be non-negative");
        self.row_gap = gap;
    }

    /// A flexible track of at least `min` (Compose's `minmax`: [`GridTrackSize::MinMax`]).
    pub fn minmax(&self, min: Dp, max: Fr) -> GridTrackSize {
        GridTrackSize::MinMax(min, max)
    }
}

impl GridConfigurationScope {
    /// A scope measured in `constraints`, nothing configured yet.
    fn new(density: Density, constraints: Constraints) -> GridConfigurationScope {
        GridConfigurationScope {
            scope_density: density,
            constraints,
            column_specs: Vec::new(),
            row_specs: Vec::new(),
            named_areas: HashMap::new(),
            column_gap: Dp(0.0),
            row_gap: Dp(0.0),
            flow: GridFlow::Row,
        }
    }

    /// Compose's GridConfigurationScopeImpl.area.
    fn add_area(&mut self, area_id: SlotKey, row: i32, column: i32, row_span: i32, column_span: i32) {
        let max = GridScope::MaxGridIndex;
        assert!(
            row != GridScope::GridIndexUnspecified || column != GridScope::GridIndexUnspecified,
            "An area must specify at least a row or a column."
        );
        assert!((-max..=max).contains(&row), "row must be between -{max} and {max}");
        assert!((-max..=max).contains(&column), "column must be between -{max} and {max}");
        assert!((1..=max).contains(&row_span), "rowSpan must be between 1 and {max}");
        assert!((1..=max).contains(&column_span), "columnSpan must be between 1 and {max}");
        // (the 16-bit packing holds MaxGridIndex)
        const { assert!(GridScope::MaxGridIndex <= i16::MAX as i32) };
        let packed_row = (row as u16 as u64) << 48;
        let packed_column = (column as u16 as u64) << 32;
        let packed_row_span = (row_span as u16 as u64) << 16;
        let packed_column_span = column_span as u16 as u64;
        self.named_areas.insert(area_id, packed_row | packed_column | packed_row_span | packed_column_span);
    }
}

/// What [`column`](GridConfigurationScope::column) and [`row`](GridConfigurationScope::row) take: Kotlin's overloads
/// by their argument's type, one Rust method taking any of them.
pub trait GridTrackSizeArgs {
    /// The track it stands for.
    fn grid_track_size(self) -> GridTrackSize;
}

/// Kotlin: `column(size: Dp)`
impl GridTrackSizeArgs for Dp {
    fn grid_track_size(self) -> GridTrackSize {
        GridTrackSize::Fixed(self)
    }
}

/// Kotlin: `column(weight: Fr)`
impl GridTrackSizeArgs for Fr {
    fn grid_track_size(self) -> GridTrackSize {
        GridTrackSize::Flex(self)
    }
}

/// Kotlin: `column(percentage: Float)`
impl GridTrackSizeArgs for f32 {
    fn grid_track_size(self) -> GridTrackSize {
        GridTrackSize::Percentage(self)
    }
}

/// Kotlin: `column(size: GridTrackSize)`
impl GridTrackSizeArgs for GridTrackSize {
    fn grid_track_size(self) -> GridTrackSize {
        self
    }
}

/// `1.fr()`, `0.5.fr()`: Compose's `1.fr`, `0.5f.fr`, `0.5.fr` (`GridConfigurationScope`'s `Int.fr`, `Float.fr` and
/// `Double.fr`, usable anywhere here).
pub trait FrExt {
    fn fr(self) -> Fr;
}

impl FrExt for i32 {
    fn fr(self) -> Fr {
        Fr(self as f32)
    }
}

impl FrExt for f32 {
    fn fr(self) -> Fr {
        Fr(self)
    }
}

impl FrExt for f64 {
    fn fr(self) -> Fr {
        Fr(self as f32)
    }
}

/// How the children that don't say where they go fill a [`Grid`] (Compose's `GridFlow`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridFlow(i32);

impl GridFlow {
    /// Along the first row, then the next (Compose's `GridFlow.Row`).
    pub const Row: GridFlow = GridFlow(0);

    /// Down the first column, then the next (Compose's `GridFlow.Column`).
    pub const Column: GridFlow = GridFlow(1);
}

impl fmt::Debug for GridFlow {
    /// Compose's toString.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            GridFlow::Row => write!(f, "Row"),
            GridFlow::Column => write!(f, "Column"),
            GridFlow(bits) => write!(f, "GridFlow({bits})"),
        }
    }
}

/// A flexible track's weight (Compose's `Fr`): its share of the room the other tracks leave, by its weight against
/// the others' (`1.fr()`, `2.fr()`: a third and two thirds).
#[derive(Clone, Copy)]
pub struct Fr(pub f32);

impl Fr {
    /// The weight (Compose's `value`).
    pub fn value(&self) -> f32 {
        self.0
    }
}

impl PartialEq for Fr {
    /// As Kotlin compares a value class of a `Float`: by its bits, any NaN the same.
    fn eq(&self, other: &Fr) -> bool {
        self.0.to_bits() == other.0.to_bits() || (self.0.is_nan() && other.0.is_nan())
    }
}

impl fmt::Debug for Fr {
    /// Compose's toString: `1.0.fr`.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.fr", self.0.kt_to_string())
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::GridTrackSize {}
}

/// What [`columns`](GridConfigurationScope::columns) and [`rows`](GridConfigurationScope::rows) take (Compose's
/// sealed `GridTrackSpec`): a [`GridTrackSize`].
pub trait GridTrackSpec: sealed::Sealed {
    /// The track size it is (Compose's `spec is GridTrackSize`).
    fn track_size(&self) -> Option<GridTrackSize>;
}

impl GridTrackSpec for GridTrackSize {
    fn track_size(&self) -> Option<GridTrackSize> {
        Some(*self)
    }
}

/// How big a track (a row or a column) of a [`Grid`] is (Compose's `GridTrackSize`), packed as Compose's: its kind
/// in the top 4 bits, then its value's float bits (MinMax: its two values' top 30 bits each).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridTrackSize {
    encoded_value: u64,
}

impl GridTrackSize {
    const TypeFixed: i32 = 1;
    const TypePercentage: i32 = 2;
    const TypeFlex: i32 = 3;
    const TypeMinContent: i32 = 4;
    const TypeMaxContent: i32 = 5;
    const TypeAuto: i32 = 6;
    const TypeMinMax: i32 = 7;

    /// A track `size` big (Compose's `GridTrackSize.Fixed`). Panics on a negative or unspecified size.
    #[track_caller]
    pub fn Fixed(size: Dp) -> GridTrackSize {
        assert!(
            !same_dp(size, Dp::Unspecified) && size.0 >= 0.0,
            "Fixed size must be non-negative and specified (was {})",
            size.kt_to_string()
        );
        GridTrackSize::pack(Self::TypeFixed, size.0)
    }

    /// A track of `value` (1 is all) of the room the Grid has for its tracks, after its gaps (Compose's
    /// `GridTrackSize.Percentage`); with unbounded room, as big as its content's max intrinsic size. Panics on a
    /// negative value.
    #[track_caller]
    pub fn Percentage(value: f32) -> GridTrackSize {
        assert!(value >= 0.0, "Percentage cannot be negative");
        GridTrackSize::pack(Self::TypePercentage, value)
    }

    /// A flexible track (Compose's `GridTrackSize.Flex`, CSS's `minmax(min-content, <weight>fr)`): at least its
    /// content's min intrinsic size, then a share of the room left by `weight`. Asking its content's intrinsic size,
    /// it can't hold a subcomposing layout (a lazy list): [`MinMax`](GridTrackSize::MinMax) can. Panics on a
    /// negative weight.
    #[track_caller]
    pub fn Flex(weight: Fr) -> GridTrackSize {
        assert!(weight.0 >= 0.0, "Flex weight must be non-negative");
        GridTrackSize::pack(Self::TypeFlex, weight.0)
    }

    /// A flexible track of at least `min`, then a share of the room left by `max` (Compose's
    /// `GridTrackSize.MinMax`, CSS's `minmax(min, max)`), its content's intrinsic size never asked. Panics on a
    /// negative one.
    #[track_caller]
    pub fn MinMax(min: Dp, max: Fr) -> GridTrackSize {
        assert!(min.0 >= 0.0, "MinMax minimum size cannot be negative");
        assert!(max.0 >= 0.0, "MinMax max weight cannot be negative");
        GridTrackSize::pack_min_max(min.0, max.0)
    }

    /// A track as big as its content's min intrinsic size (Compose's `GridTrackSize.MinContent`).
    pub const MinContent: GridTrackSize = GridTrackSize::pack(Self::TypeMinContent, 0.0);

    /// A track as big as its content's max intrinsic size (Compose's `GridTrackSize.MaxContent`).
    pub const MaxContent: GridTrackSize = GridTrackSize::pack(Self::TypeMaxContent, 0.0);

    /// A track of at least its content's min intrinsic size, growing towards its max one with the room the fixed
    /// and content tracks leave, before the flexible ones get any (Compose's `GridTrackSize.Auto`, CSS's
    /// `minmax(min-content, max-content)`). Implicit tracks are these.
    pub const Auto: GridTrackSize = GridTrackSize::pack(Self::TypeAuto, 0.0);

    #[track_caller]
    fn pack_min_max(min: f32, max: f32) -> GridTrackSize {
        assert!(min >= 0.0 && max >= 0.0, "minmax values must be non-negative");
        let type_bits = (Self::TypeMinMax as u64) & 0xF;
        let min_bits = ((min.to_bits() >> 2) as u64) & 0x3FFF_FFFF;
        let max_bits = ((max.to_bits() >> 2) as u64) & 0x3FFF_FFFF;
        GridTrackSize { encoded_value: (type_bits << 60) | (min_bits << 30) | max_bits }
    }

    const fn pack(kind: i32, value: f32) -> GridTrackSize {
        // [4 bits of kind] [28 bits empty] [32 bits of value]
        let type_bits = (kind as u64) & 0xF;
        let value_bits = value.to_bits() as u64;
        GridTrackSize { encoded_value: (type_bits << 60) | value_bits }
    }

    /// Its kind (Compose's `type`).
    fn kind(&self) -> i32 {
        (self.encoded_value >> 60) as i32
    }

    /// Its value: a Fixed's dp, a Percentage's fraction, a Flex's weight.
    fn value(&self) -> f32 {
        f32::from_bits(self.encoded_value as u32)
    }

    /// A MinMax's minimum, in dp.
    fn min_value(&self) -> f32 {
        f32::from_bits((((self.encoded_value >> 30) & 0x3FFF_FFFF) as u32) << 2)
    }

    /// A MinMax's weight.
    fn max_value(&self) -> f32 {
        f32::from_bits(((self.encoded_value & 0x3FFF_FFFF) as u32) << 2)
    }
}

impl fmt::Debug for GridTrackSize {
    /// Compose's toString: `Fixed(10.0dp)`, `Flex(1.0fr)`, `MinMax(0.0dp, 1.0fr)`, `Auto`, ….
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind() {
            Self::TypeFixed => write!(f, "Fixed({}dp)", self.value().kt_to_string()),
            Self::TypePercentage => write!(f, "Percentage({})", self.value().kt_to_string()),
            Self::TypeFlex => write!(f, "Flex({}fr)", self.value().kt_to_string()),
            Self::TypeMinContent => write!(f, "MinContent"),
            Self::TypeMaxContent => write!(f, "MaxContent"),
            Self::TypeAuto => write!(f, "Auto"),
            Self::TypeMinMax => {
                write!(f, "MinMax({}dp, {}fr)", self.min_value().kt_to_string(), self.max_value().kt_to_string())
            }
            _ => write!(f, "Unknown"),
        }
    }
}

/// The track size at `index`: an explicit one's, or `Auto` for an implicit one.
fn spec_at(specs: &[GridTrackSize], index: usize) -> GridTrackSize {
    specs.get(index).copied().unwrap_or(GridTrackSize::Auto)
}

// ---------------------------------------------------------------------------------------------------------------
// Measuring (Compose's GridMeasurePolicy)

/// Compose's GridMeasurePolicy: the config run, the items placed in cells, the tracks sized, the items measured in
/// their cells and placed (not mirrored right to left: their alignments are).
struct GridMeasurePolicy<F> {
    config: F,
}

impl<F: Fn(&mut GridConfigurationScope) + 'static> MeasurePolicy for GridMeasurePolicy<F> {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        // 1. The config.
        let mut config = GridConfigurationScope::new(scope.density(), constraints);
        (self.config)(&mut config);

        // 2. Each item's cell (explicit, by area, or by the flow), and how many tracks the grid has.
        let resolved = resolve_grid_item_indices(
            measurables,
            &config.column_specs,
            &config.row_specs,
            config.flow,
            &config.named_areas,
        );
        let mut grid_items = resolved.grid_items;

        // 3. The tracks' sizes.
        let track_sizes = calculate_grid_track_sizes(
            scope,
            &grid_items,
            &config.column_specs,
            &config.row_specs,
            resolved.grid_size.width,
            resolved.grid_size.height,
            constraints,
            config.column_gap,
            config.row_gap,
        );

        // 4. The items measured in their cells.
        measure_items(&mut grid_items, &track_sizes, scope.layout_direction());

        // 5. Within the constraints (overflowing them if bigger, at least their minimum).
        let layout_width = constraints.constrain_width(track_sizes.total_width);
        let layout_height = constraints.constrain_height(track_sizes.total_height);
        let placed: Vec<PlacedItem> = grid_items
            .into_iter()
            .filter_map(|item| {
                // (only what was measured)
                item.placeable.map(|placeable| PlacedItem {
                    placeable,
                    row: item.row as usize,
                    column: item.column as usize,
                    offset_x: item.offset_x,
                    offset_y: item.offset_y,
                })
            })
            .collect();
        scope.layout(layout_width, layout_height, move |p| {
            let column_offsets = calculate_track_offsets(&track_sizes.column_widths, track_sizes.column_gap_px);
            let row_offsets = calculate_track_offsets(&track_sizes.row_heights, track_sizes.row_gap_px);
            for item in &placed {
                let x = column_offsets[item.column].wrapping_add(item.offset_x);
                let y = row_offsets[item.row].wrapping_add(item.offset_y);
                p.place(&item.placeable, x, y);
            }
        })
    }
}

/// A child of the Grid as it is laid out (Compose's GridItem): its cell (0-based) and spans, its alignment, then
/// its placeable and where it goes in its cells.
struct GridItem<'m> {
    measurable: &'m dyn Measurable,
    row: i32,
    column: i32,
    row_span: i32,
    column_span: i32,
    alignment: Alignment,
    placeable: Option<Placeable>,
    offset_x: i32,
    offset_y: i32,
}

/// A measured item as the placement block places it.
struct PlacedItem {
    placeable: Placeable,
    row: usize,
    column: usize,
    offset_x: i32,
    offset_y: i32,
}

/// Compose's ResolvedGridItemIndicesResult: the items in their cells, in the order they are laid out (by row, then
/// column), and the grid's size in tracks (width: columns, height: rows; explicit and implicit).
struct ResolvedGridItemIndices<'m> {
    grid_items: Vec<GridItem<'m>>,
    grid_size: IntSize,
}

/// Compose's GridTrackSizes: each column's width and row's height, the grid's size (gaps included), the gaps.
struct GridTrackSizes {
    column_widths: Vec<i32>,
    row_heights: Vec<i32>,
    total_width: i32,
    total_height: i32,
    column_gap_px: i32,
    row_gap_px: i32,
}

/// A row or column not given (Compose's UnspecifiedResolvedIndex).
const UnspecifiedResolvedIndex: i32 = -1;

/// Places every item (Compose's `resolveGridItemIndices`): by area, explicit row and column, a row (or column)
/// with the first free cells in it, or the flow's next free cells from its cursor (the end of the last item it
/// placed), wrapping at the explicit tracks; the grid grows implicit tracks for what falls outside them.
fn resolve_grid_item_indices<'m>(
    measurables: &[&'m dyn Measurable],
    column_specs: &[GridTrackSize],
    row_specs: &[GridTrackSize],
    flow: GridFlow,
    named_areas: &HashMap<SlotKey, u64>,
) -> ResolvedGridItemIndices<'m> {
    let max_grid_index = GridScope::MaxGridIndex;
    let mut grid_items: Vec<GridItem<'m>> = Vec::with_capacity(measurables.len());
    // (row << 16 | column)
    let mut occupied_cells: HashSet<i32> = HashSet::new();

    let explicit_col_count = column_specs.len() as i32;
    let explicit_row_count = row_specs.len() as i32;

    // The grid's size: the explicit one, grown by what is placed outside.
    let mut max_row = explicit_row_count;
    let mut max_col = explicit_col_count;

    fn pack_coordinate(row: i32, column: i32) -> i32 {
        (row << 16) | (column & 0xFFFF)
    }

    // Whether any cell of the area is taken (or it reaches past the greatest index).
    let is_area_occupied = |occupied: &HashSet<i32>, start_row: i32, start_col: i32, row_span: i32, col_span: i32| {
        if start_row.wrapping_add(row_span) > max_grid_index || start_col.wrapping_add(col_span) > max_grid_index {
            return true;
        }
        for r in start_row..start_row.wrapping_add(row_span) {
            for c in start_col..start_col.wrapping_add(col_span) {
                if occupied.contains(&pack_coordinate(r, c)) {
                    return true;
                }
            }
        }
        false
    };

    // Where the flow placed its last item ends: the next one is looked for from there.
    let mut auto_placement_cursor_row = 0;
    let mut auto_placement_cursor_col = 0;

    for &measurable in measurables {
        let parent_data = measurable.parent_data();
        let data = parent_data.as_ref().and_then(|d| d.downcast_ref::<GridItemData>());
        let mut row_span = 1;
        let mut col_span = 1;
        let mut requested_row = UnspecifiedResolvedIndex;
        let mut requested_col = UnspecifiedResolvedIndex;
        let mut alignment = Alignment::TopStart;

        if let Some(data) = data {
            alignment = data.alignment.clone();
            // Where it asks to go: its area's bounds, or its own.
            let specified_row;
            let specified_col;
            if let Some(area_id) = &data.area_id {
                if let Some(&packed_bounds) = named_areas.get(area_id) {
                    specified_row = (packed_bounds >> 48) as u16 as i16 as i32;
                    specified_col = ((packed_bounds >> 32) & 0xFFFF) as u16 as i16 as i32;
                    row_span = ((packed_bounds >> 16) & 0xFFFF) as u16 as i16 as i32;
                    col_span = (packed_bounds & 0xFFFF) as u16 as i16 as i32;
                } else {
                    // An area the config doesn't name: placed by the flow.
                    specified_row = GridScope::GridIndexUnspecified;
                    specified_col = GridScope::GridIndexUnspecified;
                    row_span = 1;
                    col_span = 1;
                }
            } else {
                specified_row = data.row;
                specified_col = data.column;
                row_span = data.row_span;
                col_span = data.column_span;
            }
            // 1-based to 0-based (unspecified if it was, or before the first track).
            requested_row = resolve_to_zero_based_index(specified_row, explicit_row_count);
            requested_col = resolve_to_zero_based_index(specified_col, explicit_col_count);
        }

        let mut final_row = UnspecifiedResolvedIndex;
        let mut final_col = UnspecifiedResolvedIndex;

        if requested_row != UnspecifiedResolvedIndex && requested_col != UnspecifiedResolvedIndex {
            // 1. Both given: there, over whatever is there.
            final_row = requested_row;
            final_col = requested_col;
        } else if requested_row != UnspecifiedResolvedIndex {
            // 2. A row: its first free columns (from the cursor, flowing by rows in the cursor's row).
            final_row = requested_row;
            let mut candidate_col = 0;
            if flow == GridFlow::Row && requested_row == auto_placement_cursor_row {
                candidate_col = auto_placement_cursor_col;
            }
            while candidate_col < max_grid_index {
                if !is_area_occupied(&occupied_cells, requested_row, candidate_col, row_span, col_span) {
                    final_col = candidate_col;
                    break;
                }
                candidate_col += 1;
            }
        } else if requested_col != UnspecifiedResolvedIndex {
            // 3. A column: its first free rows (from the cursor, flowing by columns in the cursor's column).
            final_col = requested_col;
            let mut candidate_row = 0;
            if flow == GridFlow::Column && requested_col == auto_placement_cursor_col {
                candidate_row = auto_placement_cursor_row;
            }
            while candidate_row < max_grid_index {
                if !is_area_occupied(&occupied_cells, candidate_row, requested_col, row_span, col_span) {
                    final_row = candidate_row;
                    break;
                }
                candidate_row += 1;
            }
        } else {
            // 4. Neither: the next free cells from the cursor.
            let mut candidate_row = auto_placement_cursor_row;
            let mut candidate_col = auto_placement_cursor_col;
            while candidate_row < max_grid_index && candidate_col < max_grid_index {
                // Past the explicit tracks it wraps to the next line, unless it is at the line's start (where it
                // makes implicit tracks).
                if flow == GridFlow::Row {
                    if explicit_col_count > 0
                        && candidate_col.wrapping_add(col_span) > explicit_col_count
                        && candidate_col > 0
                    {
                        candidate_col = 0;
                        candidate_row += 1;
                        continue;
                    }
                } else if explicit_row_count > 0
                    && candidate_row.wrapping_add(row_span) > explicit_row_count
                    && candidate_row > 0
                {
                    candidate_row = 0;
                    candidate_col += 1;
                    continue;
                }

                if !is_area_occupied(&occupied_cells, candidate_row, candidate_col, row_span, col_span) {
                    final_row = candidate_row;
                    final_col = candidate_col;
                    break;
                }

                if flow == GridFlow::Row {
                    candidate_col += 1;
                    // (no explicit columns to wrap at: wrapped past the greatest index)
                    if candidate_col > max_grid_index {
                        candidate_col = 0;
                        candidate_row += 1;
                    }
                } else {
                    candidate_row += 1;
                    if candidate_row > max_grid_index {
                        candidate_row = 0;
                        candidate_col += 1;
                    }
                }
            }
        }

        // Nowhere found (the greatest index reached): the first cell, over what is there.
        let placement_row = final_row.max(0);
        let placement_col = final_col.max(0);

        for r in placement_row..placement_row.wrapping_add(row_span) {
            for c in placement_col..placement_col.wrapping_add(col_span) {
                occupied_cells.insert(pack_coordinate(r, c));
            }
        }

        grid_items.push(GridItem {
            measurable,
            row: placement_row,
            column: placement_col,
            row_span,
            column_span: col_span,
            alignment,
            placeable: None,
            offset_x: 0,
            offset_y: 0,
        });

        max_row = max_row.max(placement_row.wrapping_add(row_span));
        max_col = max_col.max(placement_col.wrapping_add(col_span));

        // Only what the flow placed moves its cursor (not an explicit item, nor one in a given row or column).
        if requested_row == UnspecifiedResolvedIndex && requested_col == UnspecifiedResolvedIndex {
            if flow == GridFlow::Row {
                auto_placement_cursor_row = placement_row;
                auto_placement_cursor_col = placement_col.wrapping_add(col_span);
            } else {
                auto_placement_cursor_row = placement_row.wrapping_add(row_span);
                auto_placement_cursor_col = placement_col;
            }
        }
    }

    // Laid out from the top start to the bottom end (a stable sort: items of one cell keep their order, the later
    // drawn on top).
    grid_items.sort_by(|a, b| a.row.cmp(&b.row).then(a.column.cmp(&b.column)));

    ResolvedGridItemIndices { grid_items, grid_size: IntSize::new(max_col, max_row) }
}

/// A 1-based index (negative: from the end of the `max_count` explicit tracks) as a 0-based one, or
/// [`UnspecifiedResolvedIndex`] if it is unspecified or before the first track (Compose's
/// `resolveToZeroBasedIndex`).
fn resolve_to_zero_based_index(index: i32, max_count: i32) -> i32 {
    if index == GridScope::GridIndexUnspecified {
        return UnspecifiedResolvedIndex;
    }
    // (past the explicit tracks: implicit ones)
    if index > 0 {
        return index - 1;
    }
    let resolved = max_count + index;
    if resolved >= 0 { resolved } else { UnspecifiedResolvedIndex }
}

/// The tracks' sizes in px (Compose's `calculateGridTrackSizes`): the columns first, then the rows, at their
/// columns' widths (what an item's height depends on).
#[allow(clippy::too_many_arguments)]
fn calculate_grid_track_sizes(
    density: &Density,
    grid_items: &[GridItem],
    column_specs: &[GridTrackSize],
    row_specs: &[GridTrackSize],
    total_col_count: i32,
    total_row_count: i32,
    constraints: Constraints,
    column_gap: Dp,
    row_gap: Dp,
) -> GridTrackSizes {
    let col_gap_px = density.round_to_px(column_gap);
    let row_gap_px = density.round_to_px(row_gap);
    let total_cols = total_col_count.max(0) as usize;
    let total_rows = total_row_count.max(0) as usize;

    // The items by the column and the row they start in.
    let mut items_by_column: Vec<Vec<usize>> = vec![Vec::new(); total_cols];
    let mut items_by_row: Vec<Vec<usize>> = vec![Vec::new(); total_rows];
    for (i, item) in grid_items.iter().enumerate() {
        if (item.column as usize) < total_cols {
            items_by_column[item.column as usize].push(i);
        }
        if (item.row as usize) < total_rows {
            items_by_row[item.row as usize].push(i);
        }
    }

    let mut column_widths = vec![0; total_cols];
    let total_track_width = calculate_column_widths(
        density,
        column_specs,
        constraints.max_width,
        &mut column_widths,
        &items_by_column,
        constraints,
        grid_items,
        col_gap_px,
    );

    let mut row_heights = vec![0; total_rows];
    let total_track_height = calculate_row_heights(
        density,
        row_specs,
        constraints.max_height,
        &mut row_heights,
        &items_by_row,
        constraints,
        &column_widths,
        grid_items,
        row_gap_px,
        col_gap_px,
    );

    // The gaps between all the tracks, implicit ones included.
    let total_column_gap = (total_col_count - 1).max(0).wrapping_mul(col_gap_px);
    let total_row_gap = (total_row_count - 1).max(0).wrapping_mul(row_gap_px);

    GridTrackSizes {
        column_widths,
        row_heights,
        total_width: total_track_width.wrapping_add(total_column_gap),
        total_height: total_track_height.wrapping_add(total_row_gap),
        column_gap_px: col_gap_px,
        row_gap_px,
    }
}

/// The room the tracks have: what is available after the gaps between `count` tracks (unbounded stays so).
fn available_track_space(available_space: i32, gap: i32, count: usize) -> i32 {
    let total_gap_space = gap.wrapping_mul(count as i32 - 1).max(0);
    if available_space == Constraints::Infinity {
        Constraints::Infinity
    } else {
        available_space.wrapping_sub(total_gap_space).max(0)
    }
}

/// Each column's width (Compose's `calculateColumnWidths`): its base size by its spec (from its single-column
/// items' intrinsic widths for the content ones, the min one for a flexible one), grown for the items spanning
/// columns, the `Auto` ones grown towards their max content, then the flexible ones sharing what is left. Gives
/// the widths' sum.
#[allow(clippy::too_many_arguments)]
fn calculate_column_widths(
    density: &Density,
    explicit_specs: &[GridTrackSize],
    available_space: i32,
    out_sizes: &mut [i32],
    items_by_column: &[Vec<usize>],
    constraints: Constraints,
    grid_items: &[GridItem],
    column_gap: i32,
) -> i32 {
    let total_count = out_sizes.len();
    if total_count == 0 {
        return 0;
    }
    let mut total_flex = 0f32;
    let available_track_space = available_track_space(available_space, column_gap, total_count);
    // The Auto columns, to grow them, and their max content (asked once).
    let mut auto_indices: Vec<usize> = Vec::new();
    let mut auto_column_max_sizes = vec![0; total_count];

    // Pass 1: the base sizes, from the single-column items.
    for index in 0..total_count {
        let spec = spec_at(explicit_specs, index);
        let items = &items_by_column[index];
        let size = match spec.kind() {
            GridTrackSize::TypeFixed => density.round_to_px(Dp(spec.value())),
            GridTrackSize::TypePercentage => {
                if available_track_space != Constraints::Infinity {
                    round(spec.value() * available_track_space as f32)
                } else {
                    // (unbounded, as in a horizontally scrolling parent: as big as its content)
                    calculate_max_intrinsic_width(items, grid_items)
                }
            }
            GridTrackSize::TypeFlex => {
                total_flex += spec.value();
                // (at least its min content: `minmax(min-content, <weight>fr)`)
                calculate_min_intrinsic_width(items, grid_items)
            }
            GridTrackSize::TypeMinContent => calculate_min_intrinsic_width(items, grid_items),
            GridTrackSize::TypeMaxContent => calculate_max_intrinsic_width(items, grid_items),
            GridTrackSize::TypeAuto => {
                if available_track_space == Constraints::Infinity {
                    // (unbounded: its max content)
                    calculate_max_intrinsic_width(items, grid_items)
                } else {
                    // Its min content, to grow towards its max content.
                    let (min, max) = calculate_min_max_intrinsic_width(items, grid_items);
                    auto_indices.push(index);
                    auto_column_max_sizes[index] = max;
                    min
                }
            }
            GridTrackSize::TypeMinMax => {
                total_flex += spec.max_value();
                // (its minimum, its content never asked)
                density.round_to_px(Dp(spec.min_value()))
            }
            _ => calculate_max_intrinsic_width(items, grid_items),
        };
        out_sizes[index] = size;
    }

    // Pass 1.5: the items spanning columns.
    distribute_spanning_space(explicit_specs, out_sizes, grid_items, false, constraints, None, column_gap, 0);

    // Pass 1.8: the Auto columns grown, before the flexible ones get anything.
    if available_track_space != Constraints::Infinity && !auto_indices.is_empty() {
        expand_auto_tracks(&auto_indices, out_sizes, &auto_column_max_sizes, available_track_space);
    }

    // Pass 2: the flexible ones share what is left.
    distribute_flex_space_and_get_total(out_sizes, available_track_space, total_flex, explicit_specs)
}

/// Each row's height (Compose's `calculateRowHeights`), as the columns' widths are worked out, its items' intrinsic
/// heights asked at the width of the columns they span.
#[allow(clippy::too_many_arguments)]
fn calculate_row_heights(
    density: &Density,
    explicit_specs: &[GridTrackSize],
    available_space: i32,
    out_sizes: &mut [i32],
    items_by_row: &[Vec<usize>],
    constraints: Constraints,
    column_widths: &[i32],
    grid_items: &[GridItem],
    row_gap: i32,
    column_gap: i32,
) -> i32 {
    let total_count = out_sizes.len();
    if total_count == 0 {
        return 0;
    }
    let mut total_flex = 0f32;
    let available_track_space = available_track_space(available_space, row_gap, total_count);
    let mut auto_indices: Vec<usize> = Vec::new();
    let mut auto_row_max_sizes = vec![0; total_count];
    let fallback_width = constraints.max_width;
    let max_height =
        |items: &[usize]| calculate_max_intrinsic_height(items, grid_items, column_widths, fallback_width, column_gap);
    let min_height =
        |items: &[usize]| calculate_min_intrinsic_height(items, grid_items, column_widths, fallback_width, column_gap);

    // Pass 1: the base sizes, from the single-row items.
    for index in 0..total_count {
        let spec = spec_at(explicit_specs, index);
        let items = &items_by_row[index];
        let size = match spec.kind() {
            GridTrackSize::TypeFixed => density.round_to_px(Dp(spec.value())),
            GridTrackSize::TypePercentage => {
                if available_track_space != Constraints::Infinity {
                    round(spec.value() * available_track_space as f32)
                } else {
                    // (unbounded, as in a vertically scrolling parent: as tall as its content)
                    max_height(items)
                }
            }
            GridTrackSize::TypeFlex => {
                total_flex += spec.value();
                min_height(items)
            }
            GridTrackSize::TypeMinContent => min_height(items),
            GridTrackSize::TypeMaxContent => max_height(items),
            GridTrackSize::TypeAuto => {
                if available_track_space == Constraints::Infinity {
                    max_height(items)
                } else {
                    let (min, max) = calculate_min_max_intrinsic_height(
                        items,
                        grid_items,
                        column_widths,
                        fallback_width,
                        column_gap,
                    );
                    auto_indices.push(index);
                    auto_row_max_sizes[index] = max;
                    min
                }
            }
            GridTrackSize::TypeMinMax => {
                total_flex += spec.max_value();
                density.round_to_px(Dp(spec.min_value()))
            }
            _ => max_height(items),
        };
        out_sizes[index] = size;
    }

    // Pass 1.5: the items spanning rows.
    distribute_spanning_space(
        explicit_specs,
        out_sizes,
        grid_items,
        true,
        constraints,
        Some(column_widths),
        row_gap,
        column_gap,
    );

    // Pass 1.8: the Auto rows grown.
    if available_track_space != Constraints::Infinity && !auto_indices.is_empty() {
        expand_auto_tracks(&auto_indices, out_sizes, &auto_row_max_sizes, available_track_space);
    }

    // Pass 2: the flexible ones share what is left.
    distribute_flex_space_and_get_total(out_sizes, available_track_space, total_flex, explicit_specs)
}

/// The tracks' sum, after the room left (if bounded) is shared by the flexible tracks by their weights (Compose's
/// `distributeFlexSpaceAndGetTotal`), rounded as they accumulate so the shares add up to it.
fn distribute_flex_space_and_get_total(
    out_sizes: &mut [i32],
    available_track_space: i32,
    total_flex: f32,
    explicit_specs: &[GridTrackSize],
) -> i32 {
    let used_space = out_sizes.iter().fold(0i32, |sum, size| sum.wrapping_add(*size));
    let remaining_space = if available_track_space == Constraints::Infinity {
        0
    } else {
        available_track_space.wrapping_sub(used_space).max(0)
    };

    let mut total_added_from_flex = 0;
    if total_flex > 0.0 && remaining_space > 0 {
        let mut distributed = 0;
        let mut accumulated_flex = 0f32;
        for (index, size) in out_sizes.iter_mut().enumerate() {
            let spec = spec_at(explicit_specs, index);
            let weight = match spec.kind() {
                GridTrackSize::TypeFlex => spec.value(),
                GridTrackSize::TypeMinMax => spec.max_value(),
                _ => 0.0,
            };
            if weight > 0.0 {
                accumulated_flex += weight;
                let target_space = round(accumulated_flex / total_flex * remaining_space as f32);
                let share = (target_space - distributed).max(0);
                *size += share;
                distributed += share;
                total_added_from_flex = distributed;
            }
        }
    }
    used_space.wrapping_add(total_added_from_flex)
}

/// The greatest max intrinsic width of the single-column items of `items` (Compose's `calculateMaxIntrinsicWidth`).
fn calculate_max_intrinsic_width(items: &[usize], grid_items: &[GridItem]) -> i32 {
    let mut max_size = 0;
    for &i in items {
        let item = &grid_items[i];
        if item.column_span == 1 {
            max_size = max_size.max(item.measurable.max_intrinsic_width(Constraints::Infinity));
        }
    }
    max_size
}

/// The greatest min intrinsic width of the single-column items (Compose's `calculateMinIntrinsicWidth`).
fn calculate_min_intrinsic_width(items: &[usize], grid_items: &[GridItem]) -> i32 {
    let mut max_size = 0;
    for &i in items {
        let item = &grid_items[i];
        if item.column_span == 1 {
            max_size = max_size.max(item.measurable.min_intrinsic_width(Constraints::Infinity));
        }
    }
    max_size
}

/// The greatest max intrinsic height of the single-row items, each at its columns' width (Compose's
/// `calculateMaxIntrinsicHeight`).
fn calculate_max_intrinsic_height(
    items: &[usize],
    grid_items: &[GridItem],
    column_widths: &[i32],
    fallback_width: i32,
    column_gap: i32,
) -> i32 {
    let mut max_size = 0;
    for &i in items {
        let item = &grid_items[i];
        if item.row_span == 1 {
            let width = get_spanned_width(item, column_widths, fallback_width, column_gap);
            max_size = max_size.max(item.measurable.max_intrinsic_height(width));
        }
    }
    max_size
}

/// The greatest min intrinsic height of the single-row items (Compose's `calculateMinIntrinsicHeight`).
fn calculate_min_intrinsic_height(
    items: &[usize],
    grid_items: &[GridItem],
    column_widths: &[i32],
    fallback_width: i32,
    column_gap: i32,
) -> i32 {
    let mut max_size = 0;
    for &i in items {
        let item = &grid_items[i];
        if item.row_span == 1 {
            let width = get_spanned_width(item, column_widths, fallback_width, column_gap);
            max_size = max_size.max(item.measurable.min_intrinsic_height(width));
        }
    }
    max_size
}

/// The greatest min and max intrinsic widths of the single-column items, in one pass (Compose's
/// `calculateMinMaxIntrinsicWidth`): (min, max).
fn calculate_min_max_intrinsic_width(items: &[usize], grid_items: &[GridItem]) -> (i32, i32) {
    let (mut max_min, mut max_max) = (0, 0);
    for &i in items {
        let item = &grid_items[i];
        if item.column_span == 1 {
            let min = item.measurable.min_intrinsic_width(Constraints::Infinity);
            let max = item.measurable.max_intrinsic_width(Constraints::Infinity);
            max_min = max_min.max(min);
            max_max = max_max.max(max);
        }
    }
    (max_min, max_max)
}

/// The greatest min and max intrinsic heights of the single-row items at their columns' widths (Compose's
/// `calculateMinMaxIntrinsicHeight`): (min, max).
fn calculate_min_max_intrinsic_height(
    items: &[usize],
    grid_items: &[GridItem],
    column_widths: &[i32],
    fallback_width: i32,
    column_gap: i32,
) -> (i32, i32) {
    let (mut max_min, mut max_max) = (0, 0);
    for &i in items {
        let item = &grid_items[i];
        if item.row_span == 1 {
            let width = get_spanned_width(item, column_widths, fallback_width, column_gap);
            let min = item.measurable.min_intrinsic_height(width);
            let max = item.measurable.max_intrinsic_height(width);
            max_min = max_min.max(min);
            max_max = max_max.max(max);
        }
    }
    (max_min, max_max)
}

/// The width of the columns an item spans and the gaps between them (Compose's `getSpannedWidth`); `fallback_width`
/// for one starting past them.
fn get_spanned_width(item: &GridItem, column_widths: &[i32], fallback_width: i32, column_gap: i32) -> i32 {
    let col_start = item.column as usize;
    if col_start >= column_widths.len() {
        return fallback_width;
    }
    let col_end = (col_start + item.column_span as usize).min(column_widths.len());
    let mut width = column_widths[col_start..col_end].iter().fold(0i32, |sum, w| sum.wrapping_add(*w));
    // (the gaps inside the span)
    width = width.wrapping_add((col_end as i32 - col_start as i32 - 1).max(0).wrapping_mul(column_gap));
    width
}

/// Grows the tracks an item spans when it needs more than they and the gaps between them make (Compose's
/// `distributeSpanningSpace`): what it lacks, shared evenly by those that aren't Fixed or Percentage (the first ones
/// a pixel more for the remainder). Its need: its max intrinsic width (at an unbounded height), or its max intrinsic
/// height at the width of its columns.
#[allow(clippy::too_many_arguments)]
fn distribute_spanning_space(
    explicit_specs: &[GridTrackSize],
    sizes: &mut [i32],
    grid_items: &[GridItem],
    is_row_axis: bool,
    constraints: Constraints,
    cross_axis_sizes: Option<&[i32]>,
    main_axis_gap: i32,
    cross_axis_gap: i32,
) {
    let grows = |index: usize| {
        let kind = spec_at(explicit_specs, index).kind();
        kind != GridTrackSize::TypeFixed && kind != GridTrackSize::TypePercentage
    };
    for item in grid_items {
        let track_index = if is_row_axis { item.row } else { item.column };
        let span = if is_row_axis { item.row_span } else { item.column_span };
        // (single-track items made the base sizes)
        if span <= 1 {
            continue;
        }
        let track_index = track_index as usize;
        let end_index = (track_index + span as usize).min(sizes.len());

        // Step 1: what the tracks make now, and which may grow.
        let mut current_spanned_size = 0i32;
        let mut tracks_to_grow_count = 0;
        for (i, size) in sizes.iter().enumerate().take(end_index).skip(track_index) {
            current_spanned_size = current_spanned_size.wrapping_add(*size);
            if grows(i) {
                tracks_to_grow_count += 1;
            }
        }
        // (the gaps inside the span)
        current_spanned_size = current_spanned_size.wrapping_add((span - 1).max(0).wrapping_mul(main_axis_gap));

        // Step 2: what the item needs.
        let required_size = if is_row_axis {
            let item_width = match cross_axis_sizes {
                Some(cross) => {
                    let col_start = item.column as usize;
                    let col_end = (col_start + item.column_span as usize).min(cross.len());
                    let mut width = 0i32;
                    for w in cross.iter().take(col_end).skip(col_start) {
                        width = width.wrapping_add(*w);
                    }
                    width.wrapping_add((item.column_span - 1).max(0).wrapping_mul(cross_axis_gap))
                }
                None => constraints.max_width,
            };
            item.measurable.max_intrinsic_height(item_width)
        } else {
            // (at an unbounded height, not to have an aspect ratio ask for a width by the grid's height)
            item.measurable.max_intrinsic_width(Constraints::Infinity)
        };

        // Step 3: what it lacks, shared.
        let deficit = required_size.wrapping_sub(current_spanned_size);
        if deficit > 0 && tracks_to_grow_count > 0 {
            let share = deficit / tracks_to_grow_count;
            let mut remainder = deficit % tracks_to_grow_count;
            for (i, size) in sizes.iter_mut().enumerate().take(end_index).skip(track_index) {
                if grows(i) {
                    *size += share + if remainder > 0 { 1 } else { 0 };
                    if remainder > 0 {
                        remainder -= 1;
                    }
                }
            }
        }
    }
}

/// Grows the `Auto` tracks from their min content towards their max content with the room left (Compose's
/// `expandAutoTracks`): all the way if it is enough, else by their shares of what they could grow.
fn expand_auto_tracks(auto_track_indices: &[usize], out_sizes: &mut [i32], max_sizes: &[i32], available_space: i32) {
    if auto_track_indices.is_empty() {
        return;
    }
    let used_space = out_sizes.iter().fold(0i32, |sum, size| sum.wrapping_add(*size));
    let remaining_space = available_space.wrapping_sub(used_space);
    if remaining_space <= 0 {
        return;
    }

    // How much each could grow.
    let growth_potentials: Vec<i32> =
        auto_track_indices.iter().map(|&track| (max_sizes[track] - out_sizes[track]).max(0)).collect();
    let total_growth_potential = growth_potentials.iter().fold(0i32, |sum, p| sum.wrapping_add(*p));
    if total_growth_potential == 0 {
        return;
    }

    if remaining_space >= total_growth_potential {
        for (&track, potential) in auto_track_indices.iter().zip(&growth_potentials) {
            out_sizes[track] += potential;
        }
    } else {
        for (&track, potential) in auto_track_indices.iter().zip(&growth_potentials) {
            let share = round(*potential as f32 / total_growth_potential as f32 * remaining_space as f32);
            out_sizes[track] += share;
        }
    }
}

/// Measures each item within its cells (loose constraints, for its alignment to place it; Compose's
/// `measureItems`), and where its alignment puts it in them (right to left, `Start` is the right).
fn measure_items(grid_items: &mut [GridItem], track_sizes: &GridTrackSizes, layout_direction: LayoutDirection) {
    let row_count = track_sizes.row_heights.len();
    let col_count = track_sizes.column_widths.len();
    for item in grid_items {
        let row = item.row as usize;
        let col = item.column as usize;
        if row < row_count && col < col_count {
            let col_limit = (col + item.column_span as usize).min(col_count);
            let mut width = track_sizes.column_widths[col..col_limit].iter().fold(0i32, |s, w| s.wrapping_add(*w));
            if col_limit - col > 1 {
                width = width.wrapping_add(((col_limit - col - 1) as i32).wrapping_mul(track_sizes.column_gap_px));
            }

            let row_limit = (row + item.row_span as usize).min(row_count);
            let mut height = track_sizes.row_heights[row..row_limit].iter().fold(0i32, |s, h| s.wrapping_add(*h));
            if row_limit - row > 1 {
                height = height.wrapping_add(((row_limit - row - 1) as i32).wrapping_mul(track_sizes.row_gap_px));
            }

            let placeable = item.measurable.measure(Constraints::new(0, width, 0, height));
            let alignment_offset = item.alignment.align(
                IntSize::new(placeable.width(), placeable.height()),
                IntSize::new(width, height),
                layout_direction,
            );
            item.placeable = Some(placeable);
            item.offset_x = alignment_offset.x;
            item.offset_y = alignment_offset.y;
        }
    }
}

/// Where each track starts: the sizes and gaps before it (Compose's `calculateTrackOffsets`).
fn calculate_track_offsets(sizes: &[i32], gap_px: i32) -> Vec<i32> {
    let mut current = 0i32;
    sizes
        .iter()
        .map(|size| {
            let offset = current;
            current = current.wrapping_add(*size).wrapping_add(gap_px);
            offset
        })
        .collect()
}

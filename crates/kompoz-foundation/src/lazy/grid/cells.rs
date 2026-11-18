//! A lazy grid's cells and spans (Compose's GridCells in LazyGridDsl.kt, LazyGridSpan.kt): how many columns (or
//! rows) and how wide each is — a fixed count, as many as fit a minimum size, as many of an exact size — and how
//! many of them an item spans; the slots' sizes and positions a measure works out from them.

use crate::layout::{HorizontalArrangement, VerticalArrangement};
use kompoz_graphics::{Constraints, Density, Dp, LayoutDirection};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// Works out a grid's cells of one's own (Compose's `GridCells.calculateCrossAxisCellSizes` of a custom one).
pub trait GridCellsCalculator {
    /// The cells' sizes across the grid, given the space across (`available_size`) and the spacing between two
    /// (from the grid's arrangement across), px; right to left the first is the rightmost.
    fn calculate_cross_axis_cell_sizes(&self, density: &Density, available_size: i32, spacing: i32) -> Vec<i32>;
}

/// The columns of a `LazyVerticalGrid` or the rows of a `LazyHorizontalGrid` (Compose's GridCells): a fixed count
/// sharing the space ([`Fixed`](Self::Fixed)), as many as fit at least `min_size` each sharing the rest
/// ([`Adaptive`](Self::Adaptive)), as many as fit of exactly `size` (the rest arranged by the grid's arrangement;
/// one as big as the grid when even one doesn't fit: [`FixedSize`](Self::FixedSize)), or one's own
/// ([`Custom`](Self::Custom)). Compose checks the count and sizes when made; here when the grid measures.
#[derive(Clone)]
pub enum GridCells {
    /// `count` cells, the space shared (Compose's `GridCells.Fixed(count)`); more than 0.
    Fixed(i32),
    /// As many cells as fit `min_size` each (and the spacing), the extra space shared (Compose's
    /// `GridCells.Adaptive(minSize)`); more than 0 dp.
    Adaptive(Dp),
    /// As many cells as fit of exactly `size` (Compose's `GridCells.FixedSize(size)`); more than 0 dp.
    FixedSize(Dp),
    /// Cells of one's own (Compose's own `GridCells`).
    Custom(Rc<dyn GridCellsCalculator>),
}

impl PartialEq for GridCells {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (GridCells::Fixed(a), GridCells::Fixed(b)) => a == b,
            (GridCells::Adaptive(a), GridCells::Adaptive(b)) => a == b,
            (GridCells::FixedSize(a), GridCells::FixedSize(b)) => a == b,
            (GridCells::Custom(a), GridCells::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for GridCells {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GridCells::Fixed(count) => write!(f, "Fixed({count})"),
            GridCells::Adaptive(size) => write!(f, "Adaptive({})", size.0),
            GridCells::FixedSize(size) => write!(f, "FixedSize({})", size.0),
            GridCells::Custom(_) => f.write_str("Custom"),
        }
    }
}

impl GridCells {
    /// The cells' sizes across, given the space across and the spacing between two, px (Compose's
    /// `calculateCrossAxisCellSizes`).
    pub fn calculate_cross_axis_cell_sizes(&self, density: &Density, available_size: i32, spacing: i32) -> Vec<i32> {
        match self {
            GridCells::Fixed(count) => {
                assert!(*count > 0, "Provided count should be larger than zero");
                calculate_cells_cross_axis_size_impl(available_size, *count, spacing)
            }
            GridCells::Adaptive(min_size) => {
                assert!(min_size.0 > 0.0, "Provided min size should be larger than zero.");
                let count = ((available_size + spacing) / (density.round_to_px(*min_size) + spacing)).max(1);
                calculate_cells_cross_axis_size_impl(available_size, count, spacing)
            }
            GridCells::FixedSize(size) => {
                assert!(size.0 > 0.0, "Provided size should be larger than zero.");
                let cell_size = density.round_to_px(*size);
                if cell_size + spacing < available_size + spacing {
                    let cell_count = (available_size + spacing) / (cell_size + spacing);
                    vec![cell_size; cell_count as usize]
                } else {
                    vec![available_size]
                }
            }
            GridCells::Custom(c) => c.calculate_cross_axis_cell_sizes(density, available_size, spacing),
        }
    }
}

/// `slot_count` cells sharing `grid_size` less the spacing, the pixels left over given to the first ones.
fn calculate_cells_cross_axis_size_impl(grid_size: i32, slot_count: i32, spacing: i32) -> Vec<i32> {
    let grid_size_without_spacing = grid_size - spacing * (slot_count - 1);
    let slot_size = grid_size_without_spacing / slot_count;
    let remaining_pixels = grid_size_without_spacing % slot_count;
    (0..slot_count).map(|i| slot_size + if i < remaining_pixels { 1 } else { 0 }).collect()
}

// ---------------------------------------------------------------------------------------------------------------
// Spans

/// How many cells of its line an item takes (Compose's GridItemSpan): made by [`GridItemSpan()`](fn@GridItemSpan).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GridItemSpan {
    current_line_span: i32,
}

/// An item taking `current_line_span` cells of its line (Compose's `GridItemSpan(currentLineSpan)`): at least 1.
pub fn GridItemSpan(current_line_span: i32) -> GridItemSpan {
    assert!(current_line_span > 0, "The span value should be higher than 0");
    GridItemSpan { current_line_span }
}

impl GridItemSpan {
    /// The cells it takes of its line (Compose's `currentLineSpan`).
    pub fn current_line_span(&self) -> i32 {
        self.current_line_span
    }
}

/// What an item's span is worked out with (Compose's LazyGridItemSpanScope): the cells left on the line it would
/// go on (`max_current_line_span`; more and it goes on the next line) and the cells of a line (`max_line_span`: a
/// span of it takes a whole line).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LazyGridItemSpanScope {
    pub(crate) max_current_line_span: i32,
    pub(crate) max_line_span: i32,
}

impl LazyGridItemSpanScope {
    /// The cells left on the line the item would go on (Compose's `maxCurrentLineSpan`).
    pub fn max_current_line_span(&self) -> i32 {
        self.max_current_line_span
    }

    /// The cells of a line: the columns of a vertical grid (Compose's `maxLineSpan`).
    pub fn max_line_span(&self) -> i32 {
        self.max_line_span
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Slots

/// The cells of a grid's lines as a measure lays them out (Compose's LazyGridSlots): their sizes and their
/// positions across, left to right (mirrored when placed).
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct LazyGridSlots {
    pub(crate) sizes: Vec<i32>,
    pub(crate) positions: Vec<i32>,
}

/// What works out a grid's slots for a measure's constraints (Compose's LazyGridSlotsProvider), cached for the same
/// constraints and density (Compose's GridSlotCache).
pub(crate) struct LazyGridSlotsProvider {
    calculation: Box<dyn Fn(&Density, Constraints) -> LazyGridSlots>,
    cache: RefCell<Option<(Constraints, f32, Rc<LazyGridSlots>)>>,
}

impl LazyGridSlotsProvider {
    fn new(calculation: impl Fn(&Density, Constraints) -> LazyGridSlots + 'static) -> LazyGridSlotsProvider {
        LazyGridSlotsProvider { calculation: Box::new(calculation), cache: RefCell::new(None) }
    }

    pub(crate) fn invoke(&self, density: &Density, constraints: Constraints) -> Rc<LazyGridSlots> {
        if let Some((cached_constraints, cached_density, sizes)) = &*self.cache.borrow()
            && *cached_constraints == constraints
            && *cached_density == density.density
        {
            return sizes.clone();
        }
        let slots = Rc::new((self.calculation)(density, constraints));
        *self.cache.borrow_mut() = Some((constraints, density.density, slots.clone()));
        slots
    }

    /// A vertical grid's columns (Compose's `rememberColumnWidthSums`'s): sized by `columns` in the width, placed
    /// by `horizontal_arrangement` left to right.
    pub(crate) fn column_width_sums(columns: GridCells, horizontal_arrangement: HorizontalArrangement) -> Self {
        LazyGridSlotsProvider::new(move |density, constraints| {
            assert!(
                constraints.max_width != Constraints::Infinity,
                "LazyVerticalGrid's width should be bound by parent."
            );
            let grid_width = constraints.max_width;
            let sizes = columns.calculate_cross_axis_cell_sizes(
                density,
                grid_width,
                density.round_to_px(horizontal_arrangement.spacing()),
            );
            let mut positions = vec![0; sizes.len()];
            horizontal_arrangement.arrange(density, grid_width, &sizes, LayoutDirection::Ltr, &mut positions);
            LazyGridSlots { sizes, positions }
        })
    }

    /// A horizontal grid's rows (Compose's `rememberRowHeightSums`'s).
    pub(crate) fn row_height_sums(rows: GridCells, vertical_arrangement: VerticalArrangement) -> Self {
        LazyGridSlotsProvider::new(move |density, constraints| {
            assert!(
                constraints.max_height != Constraints::Infinity,
                "LazyHorizontalGrid's height should be bound by parent."
            );
            let grid_height = constraints.max_height;
            let sizes = rows.calculate_cross_axis_cell_sizes(
                density,
                grid_height,
                density.round_to_px(vertical_arrangement.spacing()),
            );
            let mut positions = vec![0; sizes.len()];
            vertical_arrangement.arrange(density, grid_height, &sizes, &mut positions);
            LazyGridSlots { sizes, positions }
        })
    }
}

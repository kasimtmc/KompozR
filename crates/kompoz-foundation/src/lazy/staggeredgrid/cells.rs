//! A lazy staggered grid's lanes and spans (Compose's LazyStaggeredGridCells.kt, LazyStaggeredGridSpan.kt): how
//! many columns (or rows) and how wide each is, whether an item takes a lane or the whole line; the lanes' sizes and
//! positions a measure works out from them.

use crate::lazy::grid::GridCellsCalculator;
use kompoz_graphics::{Constraints, Density, Dp};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// The columns of a `LazyVerticalStaggeredGrid` or the rows of a `LazyHorizontalStaggeredGrid` (Compose's
/// StaggeredGridCells): a fixed count sharing the space ([`Fixed`](Self::Fixed)), as many as fit at least
/// `min_size` each sharing the rest ([`Adaptive`](Self::Adaptive)), as many as fit of exactly `size` (one as big as
/// the grid when even one doesn't fit: [`FixedSize`](Self::FixedSize)), or one's own ([`Custom`](Self::Custom)).
/// Compose checks the count and minimum size when made; here when the grid measures.
#[derive(Clone)]
pub enum StaggeredGridCells {
    /// `count` lanes, the space shared (Compose's `StaggeredGridCells.Fixed(count)`); more than 0.
    Fixed(i32),
    /// As many lanes as fit `min_size` each (and the spacing), the extra space shared (Compose's
    /// `StaggeredGridCells.Adaptive(minSize)`); more than 0 dp.
    Adaptive(Dp),
    /// As many lanes as fit of exactly `size` (Compose's `StaggeredGridCells.FixedSize(size)`).
    FixedSize(Dp),
    /// Lanes of one's own (Compose's own `StaggeredGridCells`).
    Custom(Rc<dyn GridCellsCalculator>),
}

impl PartialEq for StaggeredGridCells {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (StaggeredGridCells::Fixed(a), StaggeredGridCells::Fixed(b)) => a == b,
            (StaggeredGridCells::Adaptive(a), StaggeredGridCells::Adaptive(b)) => a == b,
            (StaggeredGridCells::FixedSize(a), StaggeredGridCells::FixedSize(b)) => a == b,
            (StaggeredGridCells::Custom(a), StaggeredGridCells::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for StaggeredGridCells {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StaggeredGridCells::Fixed(count) => write!(f, "Fixed({count})"),
            StaggeredGridCells::Adaptive(size) => write!(f, "Adaptive({})", size.0),
            StaggeredGridCells::FixedSize(size) => write!(f, "FixedSize({})", size.0),
            StaggeredGridCells::Custom(_) => f.write_str("Custom"),
        }
    }
}

impl StaggeredGridCells {
    /// The lanes' sizes across, given the space across and the spacing between two, px (Compose's
    /// `calculateCrossAxisCellSizes`).
    pub fn calculate_cross_axis_cell_sizes(&self, density: &Density, available_size: i32, spacing: i32) -> Vec<i32> {
        match self {
            StaggeredGridCells::Fixed(count) => {
                assert!(*count > 0, "grid with no rows/columns");
                calculate_cells_cross_axis_size_impl(available_size, *count, spacing)
            }
            StaggeredGridCells::Adaptive(min_size) => {
                assert!(min_size.0 > 0.0, "invalid minSize");
                let count = ((available_size + spacing) / (density.round_to_px(*min_size) + spacing)).max(1);
                calculate_cells_cross_axis_size_impl(available_size, count, spacing)
            }
            StaggeredGridCells::FixedSize(size) => {
                let cell_size = density.round_to_px(*size);
                if cell_size + spacing < available_size + spacing {
                    let cell_count = (available_size + spacing) / (cell_size + spacing);
                    vec![cell_size; cell_count as usize]
                } else {
                    vec![available_size]
                }
            }
            StaggeredGridCells::Custom(c) => c.calculate_cross_axis_cell_sizes(density, available_size, spacing),
        }
    }
}

/// `slot_count` lanes sharing `grid_size` less the spacing (none below 0), the pixels left over given to the first.
fn calculate_cells_cross_axis_size_impl(grid_size: i32, slot_count: i32, spacing: i32) -> Vec<i32> {
    let grid_size_without_spacing = grid_size - spacing * (slot_count - 1);
    let slot_size = grid_size_without_spacing / slot_count;
    let remaining_pixels = grid_size_without_spacing % slot_count;
    (0..slot_count)
        .map(|i| if slot_size < 0 { 0 } else { slot_size + if i < remaining_pixels { 1 } else { 0 } })
        .collect()
}

/// What of its line an item of a staggered grid takes (Compose's StaggeredGridItemSpan): the whole line (after the
/// longest lane, the lanes going on under it) or one lane.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StaggeredGridItemSpan {
    /// The whole line (Compose's `StaggeredGridItemSpan.FullLine`).
    FullLine,
    /// One lane (Compose's `StaggeredGridItemSpan.SingleLane`).
    SingleLane,
}

/// The lanes of a staggered grid as a measure lays them out (Compose's LazyStaggeredGridSlots): their positions
/// across, left to right (mirrored when placed), and sizes.
#[derive(Clone, PartialEq, Debug, Default)]
pub(crate) struct LazyStaggeredGridSlots {
    pub(crate) positions: Vec<i32>,
    pub(crate) sizes: Vec<i32>,
}

/// What works out a staggered grid's lanes for a measure's constraints (Compose's
/// LazyGridStaggeredGridSlotsProvider), cached for the same constraints and density (Compose's
/// LazyStaggeredGridSlotCache).
pub(crate) struct LazyStaggeredGridSlotsProvider {
    calculation: Box<dyn Fn(&Density, Constraints) -> LazyStaggeredGridSlots>,
    cache: RefCell<Option<(Constraints, f32, Rc<LazyStaggeredGridSlots>)>>,
}

impl LazyStaggeredGridSlotsProvider {
    pub(crate) fn new(
        calculation: impl Fn(&Density, Constraints) -> LazyStaggeredGridSlots + 'static,
    ) -> LazyStaggeredGridSlotsProvider {
        LazyStaggeredGridSlotsProvider { calculation: Box::new(calculation), cache: RefCell::new(None) }
    }

    pub(crate) fn invoke(&self, density: &Density, constraints: Constraints) -> Rc<LazyStaggeredGridSlots> {
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
}

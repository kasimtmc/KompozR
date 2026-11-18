//! Lazy staggered grids, as Compose's `androidx.compose.foundation.lazy.staggeredgrid`:
//! `LazyVerticalStaggeredGrid` and `LazyHorizontalStaggeredGrid` put each item under the shortest lane (a
//! full-line one, [`StaggeredGridItemSpan::FullLine`], across all lanes under the longest), composing and laying out
//! only those in view (and prefetching each lane's next item between frames); their lanes given by
//! [`StaggeredGridCells`], their block adding the items ([`LazyStaggeredGridScope`]); [`LazyStaggeredGridState`]
//! scrolls them and tells what shows ([`LazyStaggeredGridLayoutInfo`]). Built on the lazy layout core
//! ([`crate::lazy::layout`]); an item's `Modifier.animate_item` is the list's.

mod cells;
mod dsl;
mod grid;
mod lane_info;
mod layout_info;
mod measure;
mod state;

pub use cells::{StaggeredGridCells, StaggeredGridItemSpan};
pub use dsl::{LazyHorizontalStaggeredGrid, LazyStaggeredGridScope, LazyVerticalStaggeredGrid};
pub use layout_info::{LazyStaggeredGridItemInfo, LazyStaggeredGridLayoutInfo};
pub use state::{LazyStaggeredGridState, remember_lazy_staggered_grid_state};

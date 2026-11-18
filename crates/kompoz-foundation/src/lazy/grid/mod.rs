//! Lazy grids, as Compose's `androidx.compose.foundation.lazy.grid`: `LazyVerticalGrid` and `LazyHorizontalGrid`
//! compose and lay out only the lines in view (and prefetch the next one between frames), their cells given by
//! [`GridCells`] (a fixed count, as many as fit a minimum size, as many of an exact size), their block adding the
//! items ([`LazyGridScope`]: `item`, `items`, `items_list`, `items_indexed`, `sticky_header`), each spanning cells of
//! its line ([`GridItemSpan`]); [`LazyGridState`] scrolls them and tells what shows ([`LazyGridLayoutInfo`]). Built
//! on the lazy layout core ([`crate::lazy::layout`]); an item's `Modifier.animate_item` is the list's.

mod cells;
mod dsl;
#[allow(clippy::module_inception)]
mod grid;
mod layout_info;
mod measure;
mod prefetch_strategy;
mod span_layout_provider;
pub(crate) mod state;

pub use cells::{GridCells, GridCellsCalculator, GridItemSpan, LazyGridItemSpanScope};
pub use dsl::{LazyGridScope, LazyHorizontalGrid, LazyVerticalGrid};
pub use layout_info::{LazyGridItemInfo, LazyGridLayoutInfo};
pub use prefetch_strategy::{
    LazyGridPrefetchResultScope, LazyGridPrefetchScope, LazyGridPrefetchStrategy, lazy_grid_prefetch_strategy,
};
pub use state::{
    LazyGridState, remember_lazy_grid_state, remember_lazy_grid_state_with_cache_window,
    remember_lazy_grid_state_with_prefetch_strategy,
};

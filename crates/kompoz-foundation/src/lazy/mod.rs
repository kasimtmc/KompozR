//! Lazy lists, as Compose's `androidx.compose.foundation.lazy`: `LazyColumn` and `LazyRow` compose and lay out
//! only the items in view (and prefetch the next one between frames), their block adding the items
//! ([`LazyListScope`]: `item`, `items`, `items_list`, `items_indexed`, `sticky_header`), keys keeping an item's
//! state and place as items change; [`LazyListState`] scrolls them and tells what shows ([`LazyListLayoutInfo`]);
//! the items' modifiers ([`LazyItemScopeModifiers`]: `fill_parent_max_*`, `animate_item`). Lazy grids are in
//! [`grid`], staggered grids in [`staggeredgrid`]. The core they are built on, shared with the pager, is [`layout`].

pub mod grid;
pub mod layout;
mod list;
pub mod staggeredgrid;

pub use layout::LazyLayoutScrollScope;
pub use list::dsl::{LazyColumn, LazyListItems, LazyListScope, LazyRow, index_key, indexed_key, item_key};
pub use list::item_scope::LazyItemScopeModifiers;
pub use list::layout_info::{LazyListItemInfo, LazyListLayoutInfo};
pub use list::prefetch_strategy::{
    LazyListPrefetchResultScope, LazyListPrefetchScope, LazyListPrefetchStrategy, lazy_list_prefetch_strategy,
};
pub use list::state::{
    LazyListState, remember_lazy_list_state, remember_lazy_list_state_with_cache_window,
    remember_lazy_list_state_with_prefetch_strategy,
};

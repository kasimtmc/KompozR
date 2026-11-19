//! The lazy layout core, as Compose's `androidx.compose.foundation.lazy.layout`: [`LazyLayout`] (a subcomposing
//! layout composing only the items its measure asks for, their content and saveable state kept by key, the
//! compositions of items that left reused by content type), its item providers and interval content (what a DSL
//! scope builds), key index maps, prefetching in the host's pauses (pausable precomposition, nested prefetch, cache
//! windows), pinned items, beyond-bounds layout for focus search, semantics, item animations
//! (`Modifier.animate_item`), sticky items and scrolling to an item. The lazy list is built on it, and so are the
//! grids, staggered grids and pager.

mod beyond_bounds;
mod cache_window;
mod interval;
mod item_animation;
mod item_provider;
mod key;
mod lazy_layout;
mod measured_item;
mod pinnable;
mod prefetch;
mod scroll_delta;
mod scroll_scope;
mod semantics;
mod support;

pub use cache_window::{LazyLayoutCacheWindow, lazy_layout_cache_window, lazy_layout_cache_window_of_fractions};
pub use interval::{Interval, IntervalList, LazyLayoutInterval, LazyLayoutIntervalContent, MutableIntervalList};
pub use item_provider::{LazyLayoutItemProvider, LazyLayoutKeyIndexMap, NearestRangeKeyIndexMap};
pub use key::{DefaultLazyKey, LazyKey, LazyKeyValue, get_default_lazy_layout_key};
pub use lazy_layout::{ItemProviderLambda, LazyLayout, LazyLayoutMeasurePolicy, LazyLayoutMeasureScope};
pub use pinnable::{LazyLayoutPinnableItem, LazyLayoutPinnedItemList, PinnedItem, PinnedItemRef};
pub use prefetch::{
    LazyLayoutPrefetchState, NestedPrefetchScope, PrefetchHandle, PrefetchRequestScope, PrefetchResultScope,
};
pub use scroll_scope::{LazyLayoutScrollScope, LazyLayoutScrollScopeState};

// What the lazy layouts built on the core share (the list here; grids, staggered grids and the pager).
pub(crate) use beyond_bounds::{
    LazyLayoutBeyondBoundsInfo, LazyLayoutBeyondBoundsState, calculate_lazy_layout_pinned_indices,
    lazy_layout_beyond_bounds_modifier,
};
pub(crate) use cache_window::{CacheWindowLogic, CacheWindowScope, InvalidIndex};
pub(crate) use item_animation::{
    LazyLayoutAnimateItemElement, LazyLayoutAnimationSpecs, LazyLayoutItemAnimation, LazyLayoutItemAnimator,
    lazy_layout_item_animator,
};
pub(crate) use item_provider::{LazyLayoutNearestRangeState, find_index_by_key};
pub(crate) use measured_item::{
    LazyLayoutMeasuredItem, LazyLayoutMeasuredItemProvider, PlaceablesCache, StickToTopPlacement, StickyItemsPlacement,
    apply_sticky_items, updated_visible_items,
};
pub(crate) use prefetch::UnspecifiedNestedPrefetchCount;
pub(crate) use scroll_delta::LazyLayoutScrollDeltaBetweenPasses;
pub(crate) use scroll_scope::animate_scroll_to_item;
pub(crate) use semantics::{
    LazyLayoutSemanticState, estimated_lazy_max_scroll_offset, estimated_lazy_scroll_offset, lazy_layout_semantics,
};
pub(crate) use support::{
    AwaitFirstLayoutModifier, ObservableScopeInvalidator, Remeasurement, remember_lazy_layout_bring_into_view_spec,
};

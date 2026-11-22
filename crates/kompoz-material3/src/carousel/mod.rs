//! Material's carousels, as Compose's `androidx.compose.material3.carousel` package: items of a collection changing
//! size as they scroll — [`HorizontalMultiBrowseCarousel`] (large, medium and small items for quick browsing),
//! [`HorizontalUncontainedCarousel`] (items of one size, the last cut off), [`HorizontalCenteredHeroCarousel`] (a
//! large item centered between small ones) —, their state ([`CarouselState`], [`remember_carousel_state`]), the
//! scope of an item's content ([`CarouselItemScope`]: clipping and bordering to the item's mask, its
//! [`CarouselItemDrawInfo`]), the defaults ([`CarouselDefaults`]: the flings, the small items' sizes), and
//! multi-aspect carousels of lazy lists and grids ([`MultiAspectCarouselScope`],
//! [`MultiAspectCarouselItemDrawInfo`]).
//!
//! Inside, as Compose's: the keylines (`keyline_list`: fixed places along the axis giving an item centered there
//! its size), the configurations' keylines (`keylines`, fitted by an `arrangement`), the strategy shifting them as
//! the carousel scrolls to its ends (`strategy`), where items snap (`keyline_snap_position`).

pub(crate) mod arrangement;
#[allow(clippy::module_inception)]
mod carousel;
mod carousel_item_scope;
mod carousel_state;
pub(crate) mod keyline_list;
pub(crate) mod keyline_snap_position;
pub(crate) mod keylines;
mod multi_aspect_carousel;
pub(crate) mod strategy;
#[cfg(test)]
mod tests;

pub use carousel::{
    CarouselDefaults, CarouselItemContent, HorizontalCenteredHeroCarousel, HorizontalMultiBrowseCarousel,
    HorizontalUncontainedCarousel,
};
pub use carousel_item_scope::CarouselItemScope;
pub use carousel_state::{CarouselItemDrawInfo, CarouselState, remember_carousel_state};
pub use multi_aspect_carousel::{
    GridMultiAspectCarouselItemDrawInfo, MultiAspectCarouselItemDrawInfo, MultiAspectCarouselScope,
};

//! Pagers, as Compose's `androidx.compose.foundation.pager`: `HorizontalPager` and `VerticalPager` lay out pages of
//! a page size ([`PageSize`]) lazily and snap to a page when let go ([`PagerDefaults::fling_behavior`], as far as a
//! [`PagerSnapDistance`] allows); [`PagerState`] tells the current, settled and target pages and scrolls to a page
//! ([`remember_pager_state`]); what showed is its [`PagerLayoutInfo`]. Built on the lazy layout core
//! ([`crate::lazy::layout`]) and the snapping fling ([`crate::gestures::snapping`]).
//!
//! One behaviour is the C++ Kompoz's, not Compose's (user-requested): the pages' nested scroll connection settles
//! the pager with what an inner fling leaves (a list on a page flicked past its end turns the page); Compose's
//! connection only takes that velocity.

mod layout_info;
mod lazy_layout_pager;
mod measure;
#[allow(clippy::module_inception)]
mod pager;
mod state;

pub use layout_info::{
    PageInfo, PageSize, PageSizeCalculator, PagerLayoutInfo, PagerSnapDistance, PagerSnapDistanceCalculator,
};
pub use pager::{HorizontalPager, PagerDefaults, VerticalPager};
pub use state::{PagerState, remember_pager_state};

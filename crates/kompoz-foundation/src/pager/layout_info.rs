//! What a pager's pages are and what its last layout showed (Compose's PageSize.kt, PagerSnapDistance.kt,
//! PageInfo.kt, PagerLayoutInfo.kt). Compose's PagerScope, the page content's receiver, has no
//! members: a page's content is a closure of its index.

use crate::gestures::Orientation;
use crate::gestures::snapping::SnapPosition;
use crate::lazy::layout::LazyKey;
use kompoz_graphics::{Density, Dp, IntSize};
use std::fmt;
use std::rc::Rc;

/// Works out a page's size of one's own (Compose's `PageSize.calculateMainAxisPageSize` of a custom one).
pub trait PageSizeCalculator {
    /// The page's length along the pager, given the space there (past the content padding) and the spacing
    /// between pages, px.
    fn calculate_main_axis_page_size(&self, density: &Density, available_space: i32, page_spacing: i32) -> i32;
}

/// How long a pager's pages are along it (Compose's PageSize): the whole space ([`Fill`](Self::Fill)), a fixed
/// length ([`Fixed`](Self::Fixed)), or one's own ([`Custom`](Self::Custom)).
#[derive(Clone)]
pub enum PageSize {
    /// As long as the pager past its content padding (Compose's `PageSize.Fill`).
    Fill,
    /// `page_size` long (Compose's `PageSize.Fixed(pageSize)`).
    Fixed(Dp),
    /// Of one's own (Compose's own PageSize).
    Custom(Rc<dyn PageSizeCalculator>),
}

impl PartialEq for PageSize {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PageSize::Fill, PageSize::Fill) => true,
            (PageSize::Fixed(a), PageSize::Fixed(b)) => a == b,
            (PageSize::Custom(a), PageSize::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for PageSize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PageSize::Fill => f.write_str("Fill"),
            PageSize::Fixed(size) => write!(f, "Fixed({})", size.0),
            PageSize::Custom(_) => f.write_str("Custom"),
        }
    }
}

impl PageSize {
    /// The page's length along the pager (Compose's `calculateMainAxisPageSize`).
    pub fn calculate_main_axis_page_size(&self, density: &Density, available_space: i32, page_spacing: i32) -> i32 {
        match self {
            PageSize::Fill => available_space,
            PageSize::Fixed(size) => density.round_to_px(*size),
            PageSize::Custom(c) => c.calculate_main_axis_page_size(density, available_space, page_spacing),
        }
    }
}

/// Works out the page a fling goes to of one's own (Compose's `PagerSnapDistance.calculateTargetPage` of a custom
/// one).
pub trait PagerSnapDistanceCalculator {
    /// The page to go to, from the page the fling started on, the page its velocity would reach, the velocity, the
    /// page's size and the spacing.
    fn calculate_target_page(
        &self,
        start_page: i32,
        suggested_target_page: i32,
        velocity: f32,
        page_size: i32,
        page_spacing: i32,
    ) -> i32;
}

#[derive(Clone)]
enum SnapDistanceKind {
    MaxPages(i32),
    Custom(Rc<dyn PagerSnapDistanceCalculator>),
}

/// How far a fling can turn a pager's pages (Compose's PagerSnapDistance): at most so many pages
/// ([`at_most`](Self::at_most)), or one's own ([`custom`](Self::custom)).
#[derive(Clone)]
pub struct PagerSnapDistance(SnapDistanceKind);

impl PartialEq for PagerSnapDistance {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (SnapDistanceKind::MaxPages(a), SnapDistanceKind::MaxPages(b)) => a == b,
            (SnapDistanceKind::Custom(a), SnapDistanceKind::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for PagerSnapDistance {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.0 {
            SnapDistanceKind::MaxPages(pages) => write!(f, "PagerSnapDistance.atMost({pages})"),
            SnapDistanceKind::Custom(_) => f.write_str("PagerSnapDistance(custom)"),
        }
    }
}

impl PagerSnapDistance {
    /// At most `pages` pages from the one the fling started on (Compose's `PagerSnapDistance.atMost(pages)`); 0 or
    /// more.
    pub fn at_most(pages: i32) -> PagerSnapDistance {
        assert!(pages >= 0, "pages should be greater than or equal to 0. You have used {pages}.");
        PagerSnapDistance(SnapDistanceKind::MaxPages(pages))
    }

    /// One's own (Compose's own PagerSnapDistance).
    pub fn custom(calculator: Rc<dyn PagerSnapDistanceCalculator>) -> PagerSnapDistance {
        PagerSnapDistance(SnapDistanceKind::Custom(calculator))
    }

    /// The page a fling goes to (Compose's `calculateTargetPage`).
    pub fn calculate_target_page(
        &self,
        start_page: i32,
        suggested_target_page: i32,
        velocity: f32,
        page_size: i32,
        page_spacing: i32,
    ) -> i32 {
        match &self.0 {
            SnapDistanceKind::MaxPages(pages_limit) => {
                // (Compose's PagerSnapDistanceMaxPages, in longs against overflow)
                let start = start_page as i64;
                let min_range = (start - *pages_limit as i64).max(0) as i32;
                let max_range = (start + *pages_limit as i64).min(i32::MAX as i64) as i32;
                suggested_target_page.clamp(min_range, max_range)
            }
            SnapDistanceKind::Custom(c) => {
                c.calculate_target_page(start_page, suggested_target_page, velocity, page_size, page_spacing)
            }
        }
    }
}

/// A page in view (Compose's PageInfo): its index, key and where it is along the pager (px from where the content
/// starts, past the content padding).
#[derive(Clone, PartialEq, Debug)]
pub struct PageInfo {
    pub(crate) index: i32,
    pub(crate) key: LazyKey,
    pub(crate) offset: i32,
}

impl PageInfo {
    pub fn index(&self) -> i32 {
        self.index
    }
    pub fn key(&self) -> LazyKey {
        self.key.clone()
    }
    pub fn offset(&self) -> i32 {
        self.offset
    }
}

/// What a pager's last layout showed (Compose's PagerLayoutInfo): the pages in view, the pages' size and the
/// spacing between them, the part of the content in view, the content padding, the viewport's size, the
/// orientation, whether it is reversed, the pages composed beyond the viewport, where a page snaps to.
#[derive(Clone, PartialEq, Debug)]
pub struct PagerLayoutInfo {
    pub(crate) visible_pages_info: Rc<Vec<PageInfo>>,
    pub(crate) page_size: i32,
    pub(crate) page_spacing: i32,
    pub(crate) viewport_start_offset: i32,
    pub(crate) viewport_end_offset: i32,
    pub(crate) before_content_padding: i32,
    pub(crate) after_content_padding: i32,
    pub(crate) viewport_size: IntSize,
    pub(crate) orientation: Orientation,
    pub(crate) reverse_layout: bool,
    pub(crate) beyond_viewport_page_count: i32,
    pub(crate) snap_position: SnapPosition,
}

impl PagerLayoutInfo {
    pub fn visible_pages_info(&self) -> Vec<PageInfo> {
        (*self.visible_pages_info).clone()
    }
    pub fn page_size(&self) -> i32 {
        self.page_size
    }
    pub fn page_spacing(&self) -> i32 {
        self.page_spacing
    }
    pub fn viewport_start_offset(&self) -> i32 {
        self.viewport_start_offset
    }
    pub fn viewport_end_offset(&self) -> i32 {
        self.viewport_end_offset
    }
    pub fn before_content_padding(&self) -> i32 {
        self.before_content_padding
    }
    pub fn after_content_padding(&self) -> i32 {
        self.after_content_padding
    }
    pub fn viewport_size(&self) -> IntSize {
        self.viewport_size
    }
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    pub fn reverse_layout(&self) -> bool {
        self.reverse_layout
    }
    pub fn beyond_viewport_page_count(&self) -> i32 {
        self.beyond_viewport_page_count
    }
    pub fn snap_position(&self) -> SnapPosition {
        self.snap_position.clone()
    }

    /// The pages in view, without copying them.
    pub(crate) fn pages(&self) -> &[PageInfo] {
        &self.visible_pages_info
    }

    /// The viewport's length along the pager (Compose's `mainAxisViewportSize`).
    pub(crate) fn main_axis_viewport_size(&self) -> i32 {
        if self.orientation == Orientation::Vertical { self.viewport_size.height } else { self.viewport_size.width }
    }

    /// The content's length: all pages, their spacing and the padding (Compose's `calculateContentSize`).
    pub(crate) fn calculate_content_size(&self, page_count: i32) -> i32 {
        let content_padding = self.before_content_padding + self.after_content_padding;
        if page_count == 0 {
            return content_padding;
        }
        let content_size_without_spacing = self.page_size as i64 * page_count as i64;
        let total_spacing = (page_count as i64 - 1) * self.page_spacing as i64;
        let total_size = content_size_without_spacing + total_spacing + content_padding as i64;
        total_size.min(i32::MAX as i64) as i32
    }

    /// The most a pager can scroll (Compose's `calculateNewMaxScrollOffset`): the content less what of the
    /// viewport the last page's snap position leaves.
    pub(crate) fn calculate_new_max_scroll_offset(&self, page_count: i32) -> i64 {
        let page_size_with_spacing = (self.page_spacing + self.page_size) as i64;
        let max_scroll_possible = page_count as i64 * page_size_with_spacing
            + self.before_content_padding as i64
            + self.after_content_padding as i64
            - self.page_spacing as i64;
        let layout_size = if self.orientation == Orientation::Horizontal {
            self.viewport_size.width
        } else {
            self.viewport_size.height
        };
        let snap_position_discount = layout_size
            - self
                .snap_position
                .position(
                    layout_size,
                    self.page_size,
                    self.before_content_padding,
                    self.after_content_padding,
                    page_count - 1,
                    page_count,
                )
                .clamp(0, layout_size.max(0));
        (max_scroll_possible - snap_position_discount as i64).max(0)
    }
}

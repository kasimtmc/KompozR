//! Clipping a scroll container (Compose's ClipScrollableContainer.kt and CheckScrollableContainerConstraints.kt):
//! cut along its axis, with room across for what its content's shadows and stretch reach out.

use crate::gestures::Orientation;
use kompoz_graphics::{Density, Dp, LayoutDirection, Outline, Rect, Shape, Size};
use kompoz_runtime::named;
use kompoz_ui::{Constraints, Modifier};

/// How far across a scroll container shows what its content draws past its sides: elevation shadows, a stretch
/// (Compose's MaxSupportedElevation, 30 dp).
pub const MaxSupportedElevation: Dp = Dp(30.0);

/// A scroll container's clip across a horizontal one: its width, and [`MaxSupportedElevation`] more above and
/// below (Compose's HorizontalScrollableClipShape).
#[derive(Clone, Copy, PartialEq, Debug)]
struct HorizontalScrollableClipShape;

impl Shape for HorizontalScrollableClipShape {
    fn create_outline(&self, size: Size, _layout_direction: LayoutDirection, density: &Density) -> Outline {
        let inflate_size = density.round_to_px(MaxSupportedElevation) as f32;
        Outline::Rectangle(Rect::new(0.0, -inflate_size, size.width, size.height + inflate_size))
    }
}

/// The same for a vertical one: its height, more left and right (Compose's VerticalScrollableClipShape).
#[derive(Clone, Copy, PartialEq, Debug)]
struct VerticalScrollableClipShape;

impl Shape for VerticalScrollableClipShape {
    fn create_outline(&self, size: Size, _layout_direction: LayoutDirection, density: &Density) -> Outline {
        let inflate_size = density.round_to_px(MaxSupportedElevation) as f32;
        Outline::Rectangle(Rect::new(-inflate_size, 0.0, size.width + inflate_size, size.height))
    }
}

/// The scroll containers' clip.
#[named(trait ClipScrollableContainerModifiers)]
impl Modifier {
    /// Cuts what it draws to its bounds along `orientation`, leaving [`MaxSupportedElevation`] each side across
    /// for what reaches out of it (shadows, a stretch): what a scroll container clips with (Compose's
    /// `Modifier.clipScrollableContainer`).
    fn clip_scrollable_container(self, orientation: Orientation) -> Modifier {
        if orientation == Orientation::Vertical {
            self.clip(VerticalScrollableClipShape)
        } else {
            self.clip(HorizontalScrollableClipShape)
        }
    }
}

/// Panics as Compose's `checkScrollableContainerConstraints` throws when a scroll container is measured with no
/// bound along its axis (nested in another of its axis, say): it would have no end to scroll to.
#[track_caller]
pub fn check_scrollable_container_constraints(constraints: Constraints, orientation: Orientation) {
    if orientation == Orientation::Vertical {
        assert!(
            constraints.max_height != Constraints::Infinity,
            "Vertically scrollable component was measured with an infinity maximum height constraints, which is \
             disallowed. One of the common reasons is nesting layouts like LazyColumn and \
             Column(Modifier.verticalScroll()). If you want to add a header before the list of items please add a \
             header as a separate item() before the main items() inside the LazyColumn scope. There could be other \
             reasons for this to happen: your ComposeView was added into a LinearLayout with some weight, you \
             applied Modifier.wrapContentSize(unbounded = true) or wrote a custom layout. Please try to remove the \
             source of infinite constraints in the hierarchy above the scrolling container."
        );
    } else {
        assert!(
            constraints.max_width != Constraints::Infinity,
            "Horizontally scrollable component was measured with an infinity maximum width constraints, which is \
             disallowed. One of the common reasons is nesting layouts like LazyRow and \
             Row(Modifier.horizontalScroll()). If you want to add a header before the list of items please add a \
             header as a separate item() before the main items() inside the LazyRow scope. There could be other \
             reasons for this to happen: your ComposeView was added into a LinearLayout with some weight, you \
             applied Modifier.wrapContentSize(unbounded = true) or wrote a custom layout. Please try to remove the \
             source of infinite constraints in the hierarchy above the scrolling container."
        );
    }
}

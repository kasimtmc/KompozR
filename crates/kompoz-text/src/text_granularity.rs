//! What a range of text is picked by from an area (Compose's TextGranularity and TextInclusionStrategy): by
//! characters or by words, and which of them an area takes.

use kompoz_graphics::Rect;
use std::fmt;
use std::rc::Rc;

/// The units text is picked in from an area (Compose's TextGranularity): grapheme clusters, or words.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TextGranularity {
    #[default]
    Character,
    Word,
}

/// Which pieces of text an area takes, given a piece's bounds and the area (Compose's TextInclusionStrategy):
/// any that overlaps it, those it holds whole, those whose center it holds, or a rule of one's own.
#[derive(Clone)]
pub enum TextInclusionStrategy {
    AnyOverlap,
    ContainsAll,
    ContainsCenter,
    Custom(Rc<dyn Fn(Rect, Rect) -> bool>),
}

impl TextInclusionStrategy {
    /// A rule of one's own (Compose's `TextInclusionStrategy { textBounds, rect -> … }`).
    pub fn new(is_included: impl Fn(Rect, Rect) -> bool + 'static) -> TextInclusionStrategy {
        TextInclusionStrategy::Custom(Rc::new(is_included))
    }

    /// Whether the piece of text within `text_bounds` is taken by `rect`.
    pub fn is_included(&self, text_bounds: Rect, rect: Rect) -> bool {
        match self {
            TextInclusionStrategy::AnyOverlap => text_bounds.overlaps(rect),
            TextInclusionStrategy::ContainsAll => {
                !rect.is_empty()
                    && text_bounds.left >= rect.left
                    && text_bounds.right <= rect.right
                    && text_bounds.top >= rect.top
                    && text_bounds.bottom <= rect.bottom
            }
            TextInclusionStrategy::ContainsCenter => rect.contains(text_bounds.center()),
            TextInclusionStrategy::Custom(f) => f(text_bounds, rect),
        }
    }
}

/// Its name (a rule of one's own: Custom).
impl fmt::Debug for TextInclusionStrategy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            TextInclusionStrategy::AnyOverlap => "AnyOverlap",
            TextInclusionStrategy::ContainsAll => "ContainsAll",
            TextInclusionStrategy::ContainsCenter => "ContainsCenter",
            TextInclusionStrategy::Custom(_) => "Custom",
        })
    }
}

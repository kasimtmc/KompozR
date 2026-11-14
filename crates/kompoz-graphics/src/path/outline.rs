//! What a shape is in a size (Compose's Outline).

use super::path::Path;
use crate::geometry::{Rect, RoundRect};

/// What a Shape is in a size (Compose's Outline): a rect, a rounded rect, or any path, from the top left of what it
/// is for. Backgrounds, borders, clips and shadows draw the first two as themselves and the third as a path.
#[derive(Clone, PartialEq, Debug)]
pub enum Outline {
    Rectangle(Rect),
    Rounded(RoundRect),
    Generic(Path),
}

impl Outline {
    /// The rect it fills: a rect's, a rounded rect's, or its path's bounds.
    pub fn bounds(&self) -> Rect {
        match self {
            Outline::Rectangle(rect) => *rect,
            Outline::Rounded(round_rect) => round_rect.bounding_rect(),
            Outline::Generic(path) => path.get_bounds(),
        }
    }
}

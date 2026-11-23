//! What the adaptive layout's animation modifiers share, as Compose's internal AnimateModifierUtils.kt
//! (adaptive-layout 1.2.0): the invalid size, offset and rect, a layout's offset in its lookahead scope, an offset
//! taken from lookahead to current coordinates, and [`Bounds`].

use kompoz_graphics::{IntOffset, IntRect, IntSize, Offset};
use kompoz_ui::layout::{LookaheadScope, PlacementScope};

/// A size not known (Compose's internal `InvalidIntSize`).
pub(crate) const InvalidIntSize: IntSize = IntSize { width: -1, height: -1 };

/// An offset not known (Compose's internal `InvalidOffset`).
pub(crate) const InvalidOffset: IntOffset = IntOffset { x: i32::MIN, y: i32::MIN };

/// A rect not known (Compose's internal `InvalidIntRect`).
pub(crate) const InvalidIntRect: IntRect = IntRect { left: 0, top: 0, right: -1, bottom: -1 };

/// Where the layout placing is in `lookahead_scope`, as laid out in the lookahead pass (Compose's internal
/// `PlacementScope.lookaheadOffset`): the scope's coordinates' lookahead position of the layout's, rounded.
pub(crate) fn lookahead_offset(p: &PlacementScope, lookahead_scope: &dyn LookaheadScope) -> IntOffset {
    let coordinates = p.coordinates().expect("the coordinates of the layout placing");
    let scope_coordinates = p.lookahead_scope_coordinates(lookahead_scope);
    lookahead_scope.local_lookahead_position_of(&scope_coordinates, &coordinates, Offset::Zero, true).round()
}

/// `offset`, in the lookahead scope's lookahead coordinates, in the placing layout's current ones (Compose's
/// internal `PlacementScope.convertOffsetToCurrentCoordinates`).
pub(crate) fn convert_offset_to_current_coordinates(
    p: &PlacementScope,
    offset: IntOffset,
    lookahead_scope: &dyn LookaheadScope,
) -> IntOffset {
    // The offset is based on the lookahead coordinates, so we need to convert it to the local placement scope's
    // coordinates to place it in the correct position. We did that by acquiring the current origin offset under
    // the lookahead coordinates and then subtracting it from the lookahead offset.
    let coordinates = p.coordinates().expect("the coordinates of the layout placing");
    let scope_coordinates = p.lookahead_scope_coordinates(lookahead_scope);
    offset - scope_coordinates.local_position_of(&coordinates, Offset::Zero).round()
}

/// Whether the rect is known (Compose's internal `IntRect.isValid`).
#[allow(dead_code)]
pub(crate) fn int_rect_is_valid(r: IntRect) -> bool {
    r != InvalidIntRect
}

/// Whether the size is known (Compose's internal `IntSize.isValid`).
pub(crate) fn int_size_is_valid(s: IntSize) -> bool {
    s != InvalidIntSize
}

/// Whether the offset is known (Compose's internal `IntOffset.isValid`).
pub(crate) fn int_offset_is_valid(o: IntOffset) -> bool {
    o != InvalidOffset
}

/// A layout's bounds as an animation keeps them (Compose's internal class `Bounds`): not known to start with.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Bounds {
    pub(crate) top_left: IntOffset,
    pub(crate) size: IntSize,
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds { top_left: InvalidOffset, size: InvalidIntSize }
    }
}

impl Bounds {
    /// Whether both are known (Compose's `isValid`).
    pub(crate) fn is_valid(&self) -> bool {
        int_offset_is_valid(self.top_left) && int_size_is_valid(self.size)
    }

    /// The rect, if known ([`InvalidIntRect`] if not; Compose's `rect`).
    pub(crate) fn rect(&self) -> IntRect {
        if self.is_valid() { IntRect::from_offset_size(self.top_left, self.size) } else { InvalidIntRect }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_invalid_until_both_parts_are_known() {
        let mut b = Bounds::default();
        assert!(!b.is_valid() && b.rect() == InvalidIntRect);
        b.top_left = IntOffset::new(10, 20);
        assert!(!b.is_valid());
        b.size = IntSize::new(30, 40);
        assert!(b.is_valid() && int_rect_is_valid(b.rect()));
        assert_eq!(b.rect(), IntRect { left: 10, top: 20, right: 40, bottom: 60 });
        assert!(!int_size_is_valid(InvalidIntSize) && !int_offset_is_valid(InvalidOffset));
    }
}

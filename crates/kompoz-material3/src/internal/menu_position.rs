//! Where a menu goes, as Compose's internal MenuPosition.kt: its horizontal and vertical positions worked out on
//! their own ([`MenuPosition`]), against its anchor ([`AnchorAlignmentOffsetPosition`]) or the window
//! ([`WindowAlignmentMarginPosition`]), and the dropdown menu's position provider
//! ([`DropdownMenuPositionProvider`]), which tries them in turn until one fits.

use crate::menu::{MenuHorizontalMargin, MenuVerticalMargin};
use kompoz_graphics::{Density, DpOffset, IntOffset, IntRect, IntSize, LayoutDirection};
use kompoz_ui::alignment::{AbsoluteAlignment, Alignment};
use kompoz_ui::window::PopupPositionProvider;
use std::rc::Rc;

/// A menu's positions within a window (Compose's internal `object MenuPosition`): what a [`PopupPositionProvider`]
/// works out, each way on its own.
pub(crate) struct MenuPosition;

/// Works out a menu's y in the window from its anchor's bounds, the window's size and its height (Compose's
/// internal `fun interface MenuPosition.Vertical`).
pub(crate) trait MenuPositionVertical {
    fn position(&self, anchor_bounds: IntRect, window_size: IntSize, menu_height: i32) -> i32;
}

/// Works out a menu's x in the window from its anchor's bounds, the window's size, its width and the layout
/// direction (Compose's internal `fun interface MenuPosition.Horizontal`).
pub(crate) trait MenuPositionHorizontal {
    fn position(
        &self,
        anchor_bounds: IntRect,
        window_size: IntSize,
        menu_width: i32,
        layout_direction: LayoutDirection,
    ) -> i32;
}

/// A closure is a vertical position (Kotlin's `fun interface`'s lambdas).
impl<F: Fn(IntRect, IntSize, i32) -> i32> MenuPositionVertical for F {
    fn position(&self, anchor_bounds: IntRect, window_size: IntSize, menu_height: i32) -> i32 {
        self(anchor_bounds, window_size, menu_height)
    }
}

/// A closure is a horizontal position (Kotlin's `fun interface`'s lambdas).
impl<F: Fn(IntRect, IntSize, i32, LayoutDirection) -> i32> MenuPositionHorizontal for F {
    fn position(
        &self,
        anchor_bounds: IntRect,
        window_size: IntSize,
        menu_width: i32,
        layout_direction: LayoutDirection,
    ) -> i32 {
        self(anchor_bounds, window_size, menu_width, layout_direction)
    }
}

impl MenuPosition {
    /// The menu's start at its anchor's start, `offset` towards the end (Compose's `startToAnchorStart`).
    pub(crate) fn start_to_anchor_start(offset: i32) -> Rc<dyn MenuPositionHorizontal> {
        Rc::new(AnchorAlignmentOffsetPosition::Horizontal::new(Alignment::Start, Alignment::Start, offset))
    }

    /// The menu's end at its anchor's end, `offset` towards the end (Compose's `endToAnchorEnd`).
    pub(crate) fn end_to_anchor_end(offset: i32) -> Rc<dyn MenuPositionHorizontal> {
        Rc::new(AnchorAlignmentOffsetPosition::Horizontal::new(Alignment::End, Alignment::End, offset))
    }

    /// The menu's left at the window's left, kept `margin` inside the window's sides (Compose's
    /// `leftToWindowLeft`).
    pub(crate) fn left_to_window_left(margin: i32) -> Rc<dyn MenuPositionHorizontal> {
        Rc::new(WindowAlignmentMarginPosition::Horizontal::new(AbsoluteAlignment::Left, margin))
    }

    /// The menu's right at the window's right, kept `margin` inside the window's sides (Compose's
    /// `rightToWindowRight`).
    pub(crate) fn right_to_window_right(margin: i32) -> Rc<dyn MenuPositionHorizontal> {
        Rc::new(WindowAlignmentMarginPosition::Horizontal::new(AbsoluteAlignment::Right, margin))
    }

    /// The menu's top at its anchor's bottom, `offset` down (Compose's `topToAnchorBottom`).
    pub(crate) fn top_to_anchor_bottom(offset: i32) -> Rc<dyn MenuPositionVertical> {
        Rc::new(AnchorAlignmentOffsetPosition::Vertical::new(Alignment::Top, Alignment::Bottom, offset))
    }

    /// The menu's bottom at its anchor's top, `offset` down (Compose's `bottomToAnchorTop`).
    pub(crate) fn bottom_to_anchor_top(offset: i32) -> Rc<dyn MenuPositionVertical> {
        Rc::new(AnchorAlignmentOffsetPosition::Vertical::new(Alignment::Bottom, Alignment::Top, offset))
    }

    /// The menu's middle at its anchor's top, `offset` down (Compose's `centerToAnchorTop`).
    pub(crate) fn center_to_anchor_top(offset: i32) -> Rc<dyn MenuPositionVertical> {
        Rc::new(AnchorAlignmentOffsetPosition::Vertical::new(Alignment::CenterVertically, Alignment::Top, offset))
    }

    /// The menu's top at the window's top, kept `margin` inside the window's top and bottom (Compose's
    /// `topToWindowTop`).
    pub(crate) fn top_to_window_top(margin: i32) -> Rc<dyn MenuPositionVertical> {
        Rc::new(WindowAlignmentMarginPosition::Vertical::new(Alignment::Top, margin))
    }

    /// The menu's bottom at the window's bottom, kept `margin` inside the window's top and bottom (Compose's
    /// `bottomToWindowBottom`).
    pub(crate) fn bottom_to_window_bottom(margin: i32) -> Rc<dyn MenuPositionVertical> {
        Rc::new(WindowAlignmentMarginPosition::Vertical::new(Alignment::Bottom, margin))
    }
}

/// Positions of a menu against its anchor (Compose's internal `object AnchorAlignmentOffsetPosition`).
pub(crate) mod AnchorAlignmentOffsetPosition {
    use super::{MenuPositionHorizontal, MenuPositionVertical};
    use kompoz_graphics::{IntRect, IntSize, LayoutDirection};
    use kompoz_ui::alignment::{HorizontalAlignment, VerticalAlignment};

    /// The menu's `menu_alignment` across lined up with its anchor's `anchor_alignment`, `offset` towards the end
    /// (subtracted right to left) (Compose's data class `Horizontal`).
    #[derive(Clone, PartialEq, Debug)]
    pub(crate) struct Horizontal {
        menu_alignment: HorizontalAlignment,
        anchor_alignment: HorizontalAlignment,
        offset: i32,
    }

    impl Horizontal {
        pub(crate) fn new(
            menu_alignment: HorizontalAlignment,
            anchor_alignment: HorizontalAlignment,
            offset: i32,
        ) -> Horizontal {
            Horizontal { menu_alignment, anchor_alignment, offset }
        }
    }

    impl MenuPositionHorizontal for Horizontal {
        fn position(
            &self,
            anchor_bounds: IntRect,
            _window_size: IntSize,
            menu_width: i32,
            layout_direction: LayoutDirection,
        ) -> i32 {
            let anchor_alignment_offset = self.anchor_alignment.align(0, anchor_bounds.width(), layout_direction);
            let menu_alignment_offset = -self.menu_alignment.align(0, menu_width, layout_direction);
            let resolved_offset = if layout_direction == LayoutDirection::Ltr { self.offset } else { -self.offset };
            anchor_bounds.left + anchor_alignment_offset + menu_alignment_offset + resolved_offset
        }
    }

    /// The menu's `menu_alignment` down lined up with its anchor's `anchor_alignment`, `offset` down (Compose's
    /// data class `Vertical`).
    #[derive(Clone, PartialEq, Debug)]
    pub(crate) struct Vertical {
        menu_alignment: VerticalAlignment,
        anchor_alignment: VerticalAlignment,
        offset: i32,
    }

    impl Vertical {
        pub(crate) fn new(
            menu_alignment: VerticalAlignment,
            anchor_alignment: VerticalAlignment,
            offset: i32,
        ) -> Vertical {
            Vertical { menu_alignment, anchor_alignment, offset }
        }
    }

    impl MenuPositionVertical for Vertical {
        fn position(&self, anchor_bounds: IntRect, _window_size: IntSize, menu_height: i32) -> i32 {
            let anchor_alignment_offset = self.anchor_alignment.align(0, anchor_bounds.height());
            let menu_alignment_offset = -self.menu_alignment.align(0, menu_height);
            anchor_bounds.top + anchor_alignment_offset + menu_alignment_offset + self.offset
        }
    }
}

/// Positions of a menu in the window (Compose's internal `object WindowAlignmentMarginPosition`).
pub(crate) mod WindowAlignmentMarginPosition {
    use super::{MenuPositionHorizontal, MenuPositionVertical};
    use kompoz_graphics::{IntRect, IntSize, LayoutDirection};
    use kompoz_ui::alignment::{Alignment, HorizontalAlignment, VerticalAlignment};

    /// The menu aligned across the window by `alignment`, kept `margin` inside its sides; a menu too wide for that
    /// in the middle (Compose's data class `Horizontal`).
    #[derive(Clone, PartialEq, Debug)]
    pub(crate) struct Horizontal {
        alignment: HorizontalAlignment,
        margin: i32,
    }

    impl Horizontal {
        pub(crate) fn new(alignment: HorizontalAlignment, margin: i32) -> Horizontal {
            Horizontal { alignment, margin }
        }
    }

    impl MenuPositionHorizontal for Horizontal {
        fn position(
            &self,
            _anchor_bounds: IntRect,
            window_size: IntSize,
            menu_width: i32,
            layout_direction: LayoutDirection,
        ) -> i32 {
            if menu_width >= window_size.width - 2 * self.margin {
                return Alignment::CenterHorizontally.align(menu_width, window_size.width, layout_direction);
            }
            let x = self.alignment.align(menu_width, window_size.width, layout_direction);
            coerce_in(x, self.margin, window_size.width - self.margin - menu_width)
        }
    }

    /// The menu aligned down the window by `alignment`, kept `margin` inside its top and bottom; a menu too tall
    /// for that in the middle (Compose's data class `Vertical`).
    #[derive(Clone, PartialEq, Debug)]
    pub(crate) struct Vertical {
        alignment: VerticalAlignment,
        margin: i32,
    }

    impl Vertical {
        pub(crate) fn new(alignment: VerticalAlignment, margin: i32) -> Vertical {
            Vertical { alignment, margin }
        }
    }

    impl MenuPositionVertical for Vertical {
        fn position(&self, _anchor_bounds: IntRect, window_size: IntSize, menu_height: i32) -> i32 {
            if menu_height >= window_size.height - 2 * self.margin {
                return Alignment::CenterVertically.align(menu_height, window_size.height);
            }
            let y = self.alignment.align(menu_height, window_size.height);
            coerce_in(y, self.margin, window_size.height - self.margin - menu_height)
        }
    }

    /// Kotlin's `Int.coerceIn`: an empty range throws (here the menu fits, so it isn't).
    fn coerce_in(value: i32, minimum_value: i32, maximum_value: i32) -> i32 {
        if minimum_value > maximum_value {
            kompoz_runtime::kt::panic_kt(
                "IllegalArgumentException",
                &format!(
                    "Cannot coerce value to an empty range: maximum {maximum_value} is less than minimum \
                     {minimum_value}."
                ),
            );
        }
        value.clamp(minimum_value, maximum_value)
    }
}

/// What a dropdown menu's position provider tells of the position it worked out: the anchor's bounds and the
/// menu's (Compose's `onPositionCalculated`).
pub(crate) type OnPositionCalculated = Rc<dyn Fn(IntRect, IntRect)>;

/// Where a Material dropdown menu goes (Compose's internal data class `DropdownMenuPositionProvider`): across, its
/// start at its anchor's start, else its end at the anchor's end, else against the window's side nearer the
/// anchor; down, below its anchor, else above it, else its middle at the anchor's top, else against the window's
/// top or bottom nearer the anchor — the first that keeps it within the margins (`content_offset` moving it from
/// the anchor). `on_position_calculated` is told the bounds it worked out.
#[derive(Clone)]
pub(crate) struct DropdownMenuPositionProvider {
    pub(crate) content_offset: DpOffset,
    pub(crate) density: Density,
    pub(crate) vertical_margin: i32,
    pub(crate) horizontal_margin: i32,
    pub(crate) on_position_calculated: OnPositionCalculated,
    // Horizontal position
    start_to_anchor_start: Rc<dyn MenuPositionHorizontal>,
    end_to_anchor_end: Rc<dyn MenuPositionHorizontal>,
    left_to_window_left: Rc<dyn MenuPositionHorizontal>,
    right_to_window_right: Rc<dyn MenuPositionHorizontal>,
    // Vertical position
    top_to_anchor_bottom: Rc<dyn MenuPositionVertical>,
    bottom_to_anchor_top: Rc<dyn MenuPositionVertical>,
    center_to_anchor_top: Rc<dyn MenuPositionVertical>,
    top_to_window_top: Rc<dyn MenuPositionVertical>,
    bottom_to_window_bottom: Rc<dyn MenuPositionVertical>,
}

impl DropdownMenuPositionProvider {
    /// Compose's constructor, all its parameters given.
    pub(crate) fn new(
        content_offset: DpOffset,
        density: Density,
        vertical_margin: i32,
        horizontal_margin: i32,
        on_position_calculated: OnPositionCalculated,
    ) -> DropdownMenuPositionProvider {
        // Horizontal position
        let content_offset_x = density.round_to_px(content_offset.x);
        // Vertical position
        let content_offset_y = density.round_to_px(content_offset.y);
        DropdownMenuPositionProvider {
            content_offset,
            density,
            vertical_margin,
            horizontal_margin,
            on_position_calculated,
            start_to_anchor_start: MenuPosition::start_to_anchor_start(content_offset_x),
            end_to_anchor_end: MenuPosition::end_to_anchor_end(content_offset_x),
            left_to_window_left: MenuPosition::left_to_window_left(horizontal_margin),
            right_to_window_right: MenuPosition::right_to_window_right(horizontal_margin),
            top_to_anchor_bottom: MenuPosition::top_to_anchor_bottom(content_offset_y),
            bottom_to_anchor_top: MenuPosition::bottom_to_anchor_top(content_offset_y),
            center_to_anchor_top: MenuPosition::center_to_anchor_top(content_offset_y),
            top_to_window_top: MenuPosition::top_to_window_top(vertical_margin),
            bottom_to_window_bottom: MenuPosition::bottom_to_window_bottom(vertical_margin),
        }
    }

    /// Compose's defaults: the margins [`MenuVerticalMargin`] and [`MenuHorizontalMargin`] in `density`, nothing told
    /// of the position.
    pub(crate) fn of(content_offset: DpOffset, density: Density) -> DropdownMenuPositionProvider {
        let vertical_margin = density.round_to_px(MenuVerticalMargin);
        let horizontal_margin = density.round_to_px(MenuHorizontalMargin);
        DropdownMenuPositionProvider::new(
            content_offset,
            density,
            vertical_margin,
            horizontal_margin,
            Rc::new(|_, _| {}),
        )
    }
}

/// Compose's data class equality: of its constructor's properties (the callback by its instance).
impl PartialEq for DropdownMenuPositionProvider {
    fn eq(&self, other: &Self) -> bool {
        self.content_offset == other.content_offset
            && self.density == other.density
            && self.vertical_margin == other.vertical_margin
            && self.horizontal_margin == other.horizontal_margin
            && Rc::ptr_eq(&self.on_position_calculated, &other.on_position_calculated)
    }
}

impl PopupPositionProvider for DropdownMenuPositionProvider {
    fn calculate_position(
        &self,
        anchor_bounds: IntRect,
        window_size: IntSize,
        layout_direction: LayoutDirection,
        popup_content_size: IntSize,
    ) -> IntOffset {
        let x_candidates = [
            &self.start_to_anchor_start,
            &self.end_to_anchor_end,
            if anchor_bounds.center().x < window_size.width / 2 {
                &self.left_to_window_left
            } else {
                &self.right_to_window_right
            },
        ];
        let mut x = 0;
        for (index, candidate) in x_candidates.iter().enumerate() {
            let x_candidate =
                candidate.position(anchor_bounds, window_size, popup_content_size.width, layout_direction);
            if index == x_candidates.len() - 1
                || (x_candidate >= self.horizontal_margin
                    && x_candidate + popup_content_size.width <= window_size.width - self.horizontal_margin)
            {
                x = x_candidate;
                break;
            }
        }

        let y_candidates = [
            &self.top_to_anchor_bottom,
            &self.bottom_to_anchor_top,
            &self.center_to_anchor_top,
            if anchor_bounds.center().y < window_size.height / 2 {
                &self.top_to_window_top
            } else {
                &self.bottom_to_window_bottom
            },
        ];
        let mut y = 0;
        for (index, candidate) in y_candidates.iter().enumerate() {
            let y_candidate = candidate.position(anchor_bounds, window_size, popup_content_size.height);
            if index == y_candidates.len() - 1
                || (y_candidate >= self.vertical_margin
                    && y_candidate + popup_content_size.height <= window_size.height - self.vertical_margin)
            {
                y = y_candidate;
                break;
            }
        }

        let menu_offset = IntOffset::new(x, y);
        (self.on_position_calculated)(anchor_bounds, IntRect::from_offset_size(menu_offset, popup_content_size));
        menu_offset
    }
}

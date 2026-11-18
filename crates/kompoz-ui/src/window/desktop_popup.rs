//! Compose for desktop's popup position providers (DesktopPopup.desktop.kt): at a point of the anchor (a tooltip by
//! the mouse, a context menu where it was opened), at the mouse cursor, or by the anchor's bounds — kept in the window
//! by a margin, turned the other way where they would leave it.

use super::popup::PopupPositionProvider;
use crate::alignment::Alignment;
use crate::platform::{LocalDensity, LocalWindowInfo};
use kompoz_graphics::{Dp, DpOffset, IntOffset, IntRect, IntSize, LayoutDirection, Offset};
use kompoz_runtime::{kompozable, remember_keyed};
use std::rc::Rc;

/// Places a popup at `position_px` (relative to the anchor's top left, or the window's when not
/// `is_relative_to_anchor`), moved by `offset_px`, aligned to that point as `alignment` says (`BottomEnd`: below
/// and after it); turned to the other side of the point where it would come within `window_margin_px` of the
/// window's right or bottom edge, and kept that far from its left and top (Compose for desktop's
/// PopupPositionProviderAtPosition).
#[derive(Clone, PartialEq, Debug)]
pub struct PopupPositionProviderAtPosition {
    pub position_px: Offset,
    pub is_relative_to_anchor: bool,
    pub offset_px: Offset,
    pub alignment: Alignment,
    pub window_margin_px: i32,
}

impl PopupPositionProvider for PopupPositionProviderAtPosition {
    fn calculate_position(
        &self,
        anchor_bounds: IntRect,
        window_size: IntSize,
        layout_direction: LayoutDirection,
        popup_content_size: IntSize,
    ) -> IntOffset {
        let at = self.position_px.round()
            + if self.is_relative_to_anchor { anchor_bounds.top_left() } else { IntOffset::Zero };
        let anchor = IntRect::from_offset_size(at, IntSize::Zero);
        let tooltip_area = IntRect::from_offset_size(
            IntOffset::new(anchor.left - popup_content_size.width, anchor.top - popup_content_size.height),
            IntSize::new(popup_content_size.width * 2, popup_content_size.height * 2),
        );
        let position = self.alignment.align(popup_content_size, tooltip_area.size(), layout_direction);
        let mut x = (tooltip_area.left + position.x) as f32 + self.offset_px.x;
        let mut y = (tooltip_area.top + position.y) as f32 + self.offset_px.y;
        if x + popup_content_size.width as f32 > (window_size.width - self.window_margin_px) as f32 {
            x -= popup_content_size.width as f32;
        }
        if y + popup_content_size.height as f32 > (window_size.height - self.window_margin_px) as f32 {
            y -= (popup_content_size.height + anchor.height()) as f32;
        }
        x = x.max(self.window_margin_px as f32);
        y = y.max(self.window_margin_px as f32);
        IntOffset::new(x.round() as i32, y.round() as i32)
    }
}

/// A [`PopupPositionProviderAtPosition`] at `position_px` of the anchor (Compose for desktop's
/// `rememberPopupPositionProviderAtPosition`), `offset` and `window_margin` in dp.
#[kompozable]
pub fn remember_popup_position_provider_at_position(
    position_px: Offset,
    #[default(DpOffset::Zero)] offset: DpOffset,
    #[default(Alignment::BottomEnd)] alignment: Alignment,
    #[default(Dp(4.0))] window_margin: Dp,
) -> Rc<dyn PopupPositionProvider> {
    let density = LocalDensity.current();
    let offset_px = Offset::new(density.to_px(offset.x), density.to_px(offset.y));
    let window_margin_px = density.round_to_px(window_margin);
    let key = (
        position_px.x.to_bits(),
        position_px.y.to_bits(),
        offset_px.x.to_bits(),
        offset_px.y.to_bits(),
        alignment.clone(),
        window_margin_px,
    );
    remember_keyed(key, move || {
        Rc::new(PopupPositionProviderAtPosition {
            position_px,
            is_relative_to_anchor: true,
            offset_px,
            alignment,
            window_margin_px,
        }) as Rc<dyn PopupPositionProvider>
    })
}

/// Places a popup at the point of the anchor's bounds `anchor` gives, aligned to it as `alignment` says, moved by
/// `offset` (Compose for desktop's `rememberComponentRectPositionProvider`).
#[kompozable]
pub fn remember_component_rect_position_provider(
    #[default(Alignment::BottomCenter)] anchor: Alignment,
    #[default(Alignment::BottomCenter)] alignment: Alignment,
    #[default(DpOffset::Zero)] offset: DpOffset,
) -> Rc<dyn PopupPositionProvider> {
    let density = LocalDensity.current();
    let offset_px = IntOffset::new(density.round_to_px(offset.x), density.round_to_px(offset.y));
    remember_keyed((anchor.clone(), alignment.clone(), offset_px), move || {
        Rc::new(ComponentRectPositionProvider { anchor, alignment, offset_px }) as Rc<dyn PopupPositionProvider>
    })
}

/// `rememberComponentRectPositionProvider`'s provider.
#[derive(Clone, PartialEq, Debug)]
struct ComponentRectPositionProvider {
    anchor: Alignment,
    alignment: Alignment,
    offset_px: IntOffset,
}

impl PopupPositionProvider for ComponentRectPositionProvider {
    fn calculate_position(
        &self,
        anchor_bounds: IntRect,
        _window_size: IntSize,
        layout_direction: LayoutDirection,
        popup_content_size: IntSize,
    ) -> IntOffset {
        let anchor_point = self.anchor.align(IntSize::Zero, anchor_bounds.size(), layout_direction);
        let tooltip_area = IntRect::from_offset_size(
            IntOffset::new(
                anchor_bounds.left + anchor_point.x - popup_content_size.width,
                anchor_bounds.top + anchor_point.y - popup_content_size.height,
            ),
            IntSize::new(popup_content_size.width * 2, popup_content_size.height * 2),
        );
        let position = self.alignment.align(popup_content_size, tooltip_area.size(), layout_direction);
        tooltip_area.top_left() + position + self.offset_px
    }
}

/// Places a popup at the mouse cursor, where it was when this was first composed (Compose for desktop's
/// `rememberCursorPositionProvider`): moved by `offset`, aligned to it as `alignment` says, kept `window_margin`
/// within the window. With no pointer over the window, below its anchor ([`remember_component_rect_position_provider`]
/// of `alignment` and `offset`).
#[kompozable]
pub fn remember_cursor_position_provider(
    #[default(DpOffset::Zero)] offset: DpOffset,
    #[default(Alignment::BottomEnd)] alignment: Alignment,
    #[default(Dp(4.0))] window_margin: Dp,
) -> Rc<dyn PopupPositionProvider> {
    let density = LocalDensity.current();
    let offset_px = Offset::new(density.to_px(offset.x), density.to_px(offset.y));
    let window_margin_px = density.round_to_px(window_margin);
    let window_info = LocalWindowInfo.current();
    // (Compose's `rememberCursorPosition`: read once)
    let cursor_position = remember_keyed((), move || window_info.last_known_pointer_position());
    match cursor_position {
        // (the pointer outside the window: under the anchor)
        None => remember_component_rect_position_provider(alignment = alignment, offset = offset),
        Some(cursor_position) => {
            let key = (
                cursor_position.x.to_bits(),
                cursor_position.y.to_bits(),
                offset_px.x.to_bits(),
                offset_px.y.to_bits(),
                alignment.clone(),
                window_margin_px,
            );
            remember_keyed(key, move || {
                Rc::new(PopupPositionProviderAtPosition {
                    position_px: cursor_position,
                    is_relative_to_anchor: false,
                    offset_px,
                    alignment,
                    window_margin_px,
                }) as Rc<dyn PopupPositionProvider>
            })
        }
    }
}

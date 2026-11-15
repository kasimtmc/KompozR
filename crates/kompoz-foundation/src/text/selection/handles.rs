//! Selection handles (Compose's SelectionHandles.kt and AndroidSelectionHandles.android.kt): a drop under each
//! end of a selection, in a popup of its own, pointing up at its end.

use super::colors::LocalTextSelectionColors;
use crate::layout::{AbsoluteArrangement, Row, SizeModifiers, Spacer};
use crate::text::long_press_text_drag_observer::Handle;
use kompoz_graphics::{BlendMode, Color, Dp, DpSize, IntOffset, IntRect, IntSize, LayoutDirection, Offset, Size};
use kompoz_runtime::{KompozitionLocalProvider, kompozable, remember_keyed};
use kompoz_text::ResolvedTextDirection;
use kompoz_ui::draw::Fill;
use kompoz_ui::platform::LocalViewConfiguration;
use kompoz_ui::semantics::SemanticsPropertyKey;
use kompoz_ui::window::{Popup, PopupPositionProvider, PopupProperties};
use kompoz_ui::{AbsoluteAlignment, Alignment, Modifier};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

/// A selection handle's width (Compose's HandleWidth).
pub const HandleWidth: Dp = Dp(25.0);
/// A selection handle's height (Compose's HandleHeight).
pub const HandleHeight: Dp = Dp(25.0);

/// Where a selection handle points, in its anchor layout (Compose's OffsetProvider): read when its popup is
/// placed; unspecified while it is hidden. Equal when the same one.
#[derive(Clone)]
pub struct OffsetProvider(pub Rc<dyn Fn() -> Offset>);

impl OffsetProvider {
    pub fn new(provide: impl Fn() -> Offset + 'static) -> OffsetProvider {
        OffsetProvider(Rc::new(provide))
    }
    pub fn provide(&self) -> Offset {
        (self.0)()
    }
}

impl PartialEq for OffsetProvider {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for OffsetProvider {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "OffsetProvider({:?})", self.provide())
    }
}

/// How a selection handle is anchored to its position (Compose's SelectionHandleAnchor): a selection's start at
/// its left, its end at its right, a cursor's in its middle.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SelectionHandleAnchor {
    Left,
    Middle,
    Right,
}

/// What a selection handle's popup tells (Compose's SelectionHandleInfo): which handle, the position it points at
/// (in the selectable content), how it is anchored there, and whether its icon shows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SelectionHandleInfo {
    pub handle: Handle,
    pub position: Offset,
    pub anchor: SelectionHandleAnchor,
    pub visible: bool,
}

/// The semantics property of a selection handle's popup (Compose's SelectionHandleInfoKey): tests find handles
/// by it.
pub const SelectionHandleInfoKey: SemanticsPropertyKey<SelectionHandleInfo> =
    SemanticsPropertyKey::new("SelectionHandleInfo");

/// A position a pixel up (Compose's `getAdjustedCoordinates`): a line's bottom is the next line's top, outside
/// its own hit area.
pub fn get_adjusted_coordinates(position: Offset) -> Offset {
    Offset::new(position.x, position.y - 1.0)
}

/// Places a handle's popup so that its reference point is where the handle points (Compose's
/// HandlePositionProvider); a handle being hidden keeps its last place for the frame its popup still lives.
pub struct HandlePositionProvider {
    handle_reference_point: Alignment,
    position_provider: OffsetProvider,
    prev_position: Cell<Offset>,
}

impl HandlePositionProvider {
    pub fn new(handle_reference_point: Alignment, position_provider: OffsetProvider) -> HandlePositionProvider {
        HandlePositionProvider { handle_reference_point, position_provider, prev_position: Cell::new(Offset::Zero) }
    }
}

impl PopupPositionProvider for HandlePositionProvider {
    fn calculate_position(
        &self,
        anchor_bounds: IntRect,
        _window_size: IntSize,
        layout_direction: LayoutDirection,
        popup_content_size: IntSize,
    ) -> IntOffset {
        let provided = self.position_provider.provide();
        let position = if provided.is_specified() { provided } else { self.prev_position.get() };
        self.prev_position.set(position);
        let adjustment = self.handle_reference_point.align(popup_content_size, IntSize::Zero, layout_direction);
        anchor_bounds.top_left() + position.round() + adjustment
    }
}

/// Whether a handle looks left (its point at its right, towards text on its right) (Compose's
/// `isLeftSelectionHandle`).
pub fn is_left_selection_handle(
    is_start_handle: bool,
    direction: ResolvedTextDirection,
    handles_crossed: bool,
) -> bool {
    if is_start_handle {
        is_handle_ltr_direction(direction, handles_crossed)
    } else {
        !is_handle_ltr_direction(direction, handles_crossed)
    }
}

/// Whether the handles point the natural left-to-right way (Compose's `isHandleLtrDirection`): left to right and
/// not crossed, or right to left and crossed.
pub fn is_handle_ltr_direction(direction: ResolvedTextDirection, are_handles_crossed: bool) -> bool {
    direction == ResolvedTextDirection::Ltr && !are_handles_crossed
        || direction == ResolvedTextDirection::Rtl && are_handles_crossed
}

/// A selection handle (Compose's Android SelectionHandle): in a popup placed by `offset_provider` (its top right
/// there for a left handle, its top left for a right one), drawn in the selection colors' handle color, telling its
/// [`SelectionHandleInfo`]; at least `min_touch_target_size` (if specified) to touch. `modifier` takes its drags.
#[kompozable]
pub fn SelectionHandle(
    offset_provider: OffsetProvider,
    is_start_handle: bool,
    direction: ResolvedTextDirection,
    handles_crossed: bool,
    #[default(DpSize::Unspecified)] min_touch_target_size: DpSize,
    line_height: f32,
    modifier: Modifier,
) {
    // (the line's height places Android's handles no differently)
    let _ = line_height;
    let is_left = is_left_selection_handle(is_start_handle, direction, handles_crossed);
    // A left handle's top right is put at the position, a right one's top left.
    let handle_reference_point = if is_left { AbsoluteAlignment::TopRight } else { AbsoluteAlignment::TopLeft };
    let info_provider = offset_provider.clone();
    let semantics_modifier = modifier.semantics(move |config| {
        let position = info_provider.provide();
        config.set(
            SelectionHandleInfoKey,
            SelectionHandleInfo {
                handle: if is_start_handle { Handle::SelectionStart } else { Handle::SelectionEnd },
                position,
                anchor: if is_left { SelectionHandleAnchor::Left } else { SelectionHandleAnchor::Right },
                visible: position.is_specified(),
            },
        );
    });
    // The view configuration goes on into the popup.
    let view_configuration = LocalViewConfiguration.current();
    let handle_color = LocalTextSelectionColors.current().handle_color;
    let visible_provider = offset_provider.clone();
    HandlePopup(
        position_provider = offset_provider,
        handle_reference_point = handle_reference_point,
        content = move || {
            let semantics_modifier = semantics_modifier.clone();
            let visible_provider = visible_provider.clone();
            KompozitionLocalProvider(&[LocalViewConfiguration.provides(view_configuration)], move || {
                let icon_visible = Rc::new(move || visible_provider.provide().is_specified());
                if min_touch_target_size.is_specified() {
                    // In a row as big as the touch target, at the side the handle points from.
                    let arrangement = if is_left { AbsoluteArrangement::Right } else { AbsoluteArrangement::Left };
                    let row_modifier = semantics_modifier.required_size_in_with(
                        min_touch_target_size.width,
                        min_touch_target_size.height,
                        Dp::Unspecified,
                        Dp::Unspecified,
                    );
                    Row(
                        horizontal_arrangement = arrangement,
                        modifier = row_modifier,
                        content = move || {
                            SelectionHandleIcon(Modifier, icon_visible.clone(), is_left, handle_color);
                        },
                    );
                } else {
                    SelectionHandleIcon(semantics_modifier.clone(), icon_visible, is_left, handle_color);
                }
            });
        },
    );
}

/// A handle's icon (Compose's SelectionHandleIcon): a handle-sized spacer drawing the handle while `icon_visible`.
#[kompozable]
pub fn SelectionHandleIcon(modifier: Modifier, icon_visible: Rc<dyn Fn() -> bool>, is_left: bool, color: Color) {
    Spacer(modifier.size_wh(HandleWidth, HandleHeight).then(draw_selection_handle(icon_visible, is_left, color)));
}

/// Draws a selection handle over the content (Compose's `drawSelectionHandle`): a circle filling the size with a
/// square over its top quarter on the side it points from — the top left for a right handle, the top right for a
/// left one (Compose draws a cached mask of it, mirrored for the left one; the same shape).
pub fn draw_selection_handle(icon_visible: Rc<dyn Fn() -> bool>, is_left: bool, color: Color) -> Modifier {
    Modifier.draw_with_content(move |s| {
        s.draw_content();
        if !icon_visible() {
            return;
        }
        let size = s.size();
        let radius = size.width / 2.0;
        let square_left = if is_left { size.width - radius } else { 0.0 };
        s.draw_rect_with(
            color,
            Offset::new(square_left, 0.0),
            Size::new(radius, radius),
            1.0,
            Fill,
            None,
            BlendMode::SrcOver,
        );
        s.draw_circle_with(color, radius, Offset::new(radius, radius), 1.0, Fill, None, BlendMode::SrcOver);
    })
}

/// The popup of a handle (Compose's HandlePopup): placed by a [`HandlePositionProvider`], not kept in the window.
#[kompozable]
pub fn HandlePopup(position_provider: OffsetProvider, handle_reference_point: Alignment, content: impl Fn() + 'static) {
    let provider = remember_keyed((handle_reference_point.clone(), position_provider.clone()), move || {
        Rc::new(HandlePositionProvider::new(handle_reference_point, position_provider)) as Rc<dyn PopupPositionProvider>
    });
    Popup(
        popup_position_provider = Some(provider),
        properties = PopupProperties { clipping_enabled: false, ..PopupProperties::default() },
        content = content,
    );
}

//! Tooltips, as Compose's Tooltip.kt — the base its components stand on: a tooltip's state ([`TooltipState`]), the
//! scope its content is in ([`TooltipScope`]), where it goes around its anchor ([`TooltipAnchorPosition`]) and the
//! position provider that puts it there, flipping it to the other side at the window's edges
//! ([`TooltipDefaults::remember_tooltip_position_provider`]).

use kompoz_animation::{MutableTransitionState, MutatePriority};
use kompoz_foundation::gestures::LocalFuture;
use kompoz_graphics::{Dp, IntOffset, IntRect, IntSize, LayoutDirection};
use kompoz_runtime::{MutableState, mutable_state_of, remember_keyed};
use kompoz_ui::layout::{LayoutCoordinates, MeasureScope};
use kompoz_ui::platform::{LocalDensity, LocalWindowInfo};
use kompoz_ui::window::PopupPositionProvider;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// The space between a tooltip and its anchor (Compose's internal `SpacingBetweenTooltipAndAnchor`).
pub(crate) const SpacingBetweenTooltipAndAnchor: Dp = Dp(4.0);

/// The state that is associated with a tooltip box; each should have its own (Compose's interface
/// `TooltipState`).
pub trait TooltipState {
    /// The current transition state of the tooltip, which starts its fading in and out (Compose's `transition`).
    fn transition(&self) -> MutableTransitionState<bool>;

    /// Whether the tooltip is shown (Compose's `isVisible`, a state read).
    fn is_visible(&self) -> bool;

    /// Whether it stays until the user clicks outside it or [`dismiss`](TooltipState::dismiss) is called; else it
    /// goes after a short while — ideally true when the tooltip shows something to act on (Compose's
    /// `isPersistent`).
    fn is_persistent(&self) -> bool;

    /// Shows the tooltip, the others shown dismissed (Compose's suspending `show(mutatePriority =
    /// MutatePriority.Default)`): `mutate_priority` against their mutex's mutations.
    fn show(&self, mutate_priority: MutatePriority) -> LocalFuture<'static, ()>;

    /// Dismisses the tooltip if it is shown (Compose's `dismiss`).
    fn dismiss(&self);

    /// Cleans up when the state leaves the composition (Compose's `onDispose`).
    fn on_dispose(&self);
}

mod sealed {
    pub trait Sealed {}
}

/// The scope a tooltip's content is in (Compose's sealed interface `TooltipScope`): what its caret is drawn by, the
/// anchor's coordinates and the position provider. (Compose's `Modifier.drawCaret` is hidden, kept for binary
/// compatibility only.)
pub trait TooltipScope: sealed::Sealed {
    /// The anchor's coordinates, to point a caret at it (Compose's `MeasureScope.obtainAnchorBounds`).
    fn obtain_anchor_bounds(&self, scope: &MeasureScope) -> Option<LayoutCoordinates>;

    /// The position provider the tooltip is placed by, to point a caret at the anchor (Compose's
    /// `obtainPositionProvider`).
    fn obtain_position_provider(&self) -> Rc<dyn PopupPositionProvider>;
}

/// Compose's internal `TooltipScopeImpl`.
pub(crate) struct TooltipScopeImpl {
    get_anchor_bounds: Rc<dyn Fn() -> Option<LayoutCoordinates>>,
    position_provider: Rc<dyn PopupPositionProvider>,
}

impl TooltipScopeImpl {
    pub(crate) fn new(
        get_anchor_bounds: impl Fn() -> Option<LayoutCoordinates> + 'static,
        position_provider: Rc<dyn PopupPositionProvider>,
    ) -> TooltipScopeImpl {
        TooltipScopeImpl { get_anchor_bounds: Rc::new(get_anchor_bounds), position_provider }
    }
}

impl sealed::Sealed for TooltipScopeImpl {}

impl TooltipScope for TooltipScopeImpl {
    fn obtain_anchor_bounds(&self, _scope: &MeasureScope) -> Option<LayoutCoordinates> {
        (self.get_anchor_bounds)()
    }

    fn obtain_position_provider(&self) -> Rc<dyn PopupPositionProvider> {
        self.position_provider.clone()
    }
}

/// The anchor's coordinates as a state (Compose's `mutableStateOf<LayoutCoordinates?>(null)`: its readers told when
/// other coordinates are set; coordinates aren't `Send`, so they are kept beside a state of their version).
#[derive(Clone)]
pub(crate) struct AnchorBounds {
    value: Rc<RefCell<Option<LayoutCoordinates>>>,
    version: MutableState<u64>,
}

impl AnchorBounds {
    pub(crate) fn new() -> AnchorBounds {
        AnchorBounds { value: Rc::new(RefCell::new(None)), version: mutable_state_of(0) }
    }

    /// The coordinates (a state read).
    pub(crate) fn get(&self) -> Option<LayoutCoordinates> {
        self.version.get();
        self.value.borrow().clone()
    }

    /// Sets them: their readers are told if they are other coordinates (structural equality, as Compose's).
    pub(crate) fn set(&self, coordinates: Option<LayoutCoordinates>) {
        if *self.value.borrow() == coordinates {
            return;
        }
        *self.value.borrow_mut() = coordinates;
        self.version.set(self.version.peek().wrapping_add(1));
    }
}

/// Where a tooltip goes around its anchor (Compose's value class `TooltipAnchorPosition`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TooltipAnchorPosition(i32);

impl TooltipAnchorPosition {
    /// Places the tooltip above the anchor.
    pub const Above: TooltipAnchorPosition = TooltipAnchorPosition(1);
    /// Places the tooltip below the anchor.
    pub const Below: TooltipAnchorPosition = TooltipAnchorPosition(2);
    /// Places the tooltip on the left of the anchor.
    pub const Left: TooltipAnchorPosition = TooltipAnchorPosition(3);
    /// Places the tooltip on the right of the anchor.
    pub const Right: TooltipAnchorPosition = TooltipAnchorPosition(4);
    /// Places the tooltip at the start of the anchor.
    pub const Start: TooltipAnchorPosition = TooltipAnchorPosition(5);
    /// Places the tooltip at the end of the anchor.
    pub const End: TooltipAnchorPosition = TooltipAnchorPosition(6);
}

/// Compose's toString: the position's name.
impl fmt::Display for TooltipAnchorPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            TooltipAnchorPosition::Above => "Above",
            TooltipAnchorPosition::Below => "Below",
            TooltipAnchorPosition::Left => "Left",
            TooltipAnchorPosition::Right => "Right",
            TooltipAnchorPosition::Start => "Start",
            TooltipAnchorPosition::End => "End",
            _ => "Invalid",
        })
    }
}

impl fmt::Debug for TooltipAnchorPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Tooltips' defaults (Compose's `object TooltipDefaults`): their position providers.
pub struct TooltipDefaults;

impl TooltipDefaults {
    /// The position provider of a plain or rich tooltip at `positioning` around its anchor, `spacing` from it
    /// (Compose's `rememberTooltipPositionProvider(positioning, spacingBetweenTooltipAndAnchor)`, its default 4 dp): left or
    /// right of it (start and end by the layout direction), vertically centered, flipped to the other side at the
    /// window's edge; above or below it, centered, else lined up with its start or end at the window's sides,
    /// flipped below or above at the window's top or bottom. The window is [`LocalWindowInfo`]'s container.
    ///
    /// Kotlin: `rememberTooltipPositionProvider(positioning: TooltipAnchorPosition, spacingBetweenTooltipAndAnchor: Dp = SpacingBetweenTooltipAndAnchor): PopupPositionProvider`
    #[track_caller]
    pub fn remember_tooltip_position_provider(
        positioning: TooltipAnchorPosition,
        spacing_between_tooltip_and_anchor: Dp,
    ) -> Rc<dyn PopupPositionProvider> {
        kompoz_runtime::kompozable_block!({
            let tooltip_anchor_spacing = LocalDensity.current().round_to_px(spacing_between_tooltip_and_anchor);
            let window_container_size = LocalWindowInfo.current().container_size();
            remember_keyed((tooltip_anchor_spacing, positioning, window_container_size), move || {
                Rc::new(TooltipPositionProviderImpl::new(positioning, tooltip_anchor_spacing, window_container_size))
                    as Rc<dyn PopupPositionProvider>
            })
        })
    }

    /// The position provider of a plain or rich tooltip above its anchor, `spacing` from it, centered on it, else
    /// lined up with its start or end at the window's sides, below it where above would leave the window (Compose's
    /// deprecated `rememberTooltipPositionProvider(spacingBetweenTooltipAndAnchor = 4.dp)`).
    ///
    /// Kotlin: `rememberTooltipPositionProvider(spacingBetweenTooltipAndAnchor: Dp = SpacingBetweenTooltipAndAnchor): PopupPositionProvider`
    #[deprecated(
        note = "Deprecated in favor of rememberTooltipPositionProvider API that takes a preferred positioning. Please \
                use rememberTooltipPositionProvider with TooltipAnchorPosition.Above if this same behavior is desired."
    )]
    #[track_caller]
    pub fn remember_tooltip_position_provider_with_spacing(
        spacing_between_tooltip_and_anchor: Dp,
    ) -> Rc<dyn PopupPositionProvider> {
        kompoz_runtime::kompozable_block!({
            let tooltip_anchor_spacing = LocalDensity.current().round_to_px(spacing_between_tooltip_and_anchor);
            remember_keyed(tooltip_anchor_spacing, move || {
                Rc::new(AboveTooltipPositionProvider { tooltip_anchor_spacing }) as Rc<dyn PopupPositionProvider>
            })
        })
    }
}

/// The deprecated provider's `object : PopupPositionProvider` (above, centered; start or end aligned at the
/// window's sides; below if above leaves the window).
struct AboveTooltipPositionProvider {
    tooltip_anchor_spacing: i32,
}

impl PopupPositionProvider for AboveTooltipPositionProvider {
    fn calculate_position(
        &self,
        anchor_bounds: IntRect,
        window_size: IntSize,
        _layout_direction: LayoutDirection,
        popup_content_size: IntSize,
    ) -> IntOffset {
        // Horizontal alignment preference: middle -> start -> end
        // Vertical preference: above -> below

        // Tooltip prefers to be center aligned horizontally.
        let mut x = anchor_bounds.left + (anchor_bounds.width() - popup_content_size.width) / 2;

        if x < 0 {
            // Make tooltip start aligned if colliding with the left side of the screen
            x = anchor_bounds.left;
        } else if x + popup_content_size.width > window_size.width {
            // Make tooltip end aligned if colliding with the right side of the screen
            x = anchor_bounds.right - popup_content_size.width;
        }

        // Tooltip prefers to be above the anchor, but if this causes the tooltip to overlap with the anchor then we
        // place it below the anchor
        let mut y = anchor_bounds.top - popup_content_size.height - self.tooltip_anchor_spacing;
        if y < 0 {
            y = anchor_bounds.bottom + self.tooltip_anchor_spacing;
        }
        IntOffset::new(x, y)
    }
}

/// The tooltip position provider of a positioning (Compose's private `TooltipPositionProviderImpl`): the window it
/// keeps within is the container's size it was made with, not the size it is given.
pub(crate) struct TooltipPositionProviderImpl {
    r#type: TooltipAnchorPosition,
    tooltip_anchor_spacing: i32,
    window_container_size: IntSize,
}

impl TooltipPositionProviderImpl {
    pub(crate) fn new(
        r#type: TooltipAnchorPosition,
        tooltip_anchor_spacing: i32,
        window_container_size: IntSize,
    ) -> TooltipPositionProviderImpl {
        TooltipPositionProviderImpl { r#type, tooltip_anchor_spacing, window_container_size }
    }

    fn left_positioning(&self, anchor_bounds: IntRect, popup_content_size: IntSize, window_size: IntSize) -> IntOffset {
        // Horizontal alignment preference: left -> right
        // Vertical preference: center

        // Tooltip prefers to be to the left of the anchor
        let mut x = anchor_bounds.left - (popup_content_size.width + self.tooltip_anchor_spacing);

        if x < 0 {
            // Flip the tooltip to be on the right if it collides with the left side of the screen
            let x_correction = (anchor_bounds.right + self.tooltip_anchor_spacing + popup_content_size.width
                - window_size.width)
                .max(0);
            x = anchor_bounds.right + self.tooltip_anchor_spacing - x_correction;
        }

        // We vertically center the tooltip with the anchor
        let y = (anchor_bounds.top + anchor_bounds.bottom - popup_content_size.height) / 2;
        IntOffset::new(x, y)
    }

    fn right_positioning(
        &self,
        anchor_bounds: IntRect,
        popup_content_size: IntSize,
        window_size: IntSize,
    ) -> IntOffset {
        // Horizontal alignment preference: right -> left
        // Vertical preference: center

        // Tooltip prefers to be to the right of the anchor
        let mut x = anchor_bounds.right + self.tooltip_anchor_spacing;

        if x + popup_content_size.width > window_size.width {
            // Flip the tooltip to be on the left if it collides with the right side of the screen
            x = (anchor_bounds.left - (popup_content_size.width + self.tooltip_anchor_spacing)).max(0);
        }

        // We vertically center the tooltip with the anchor
        let y = (anchor_bounds.top + anchor_bounds.bottom - popup_content_size.height) / 2;
        IntOffset::new(x, y)
    }

    /// Centered across, else start or end aligned at the window's sides.
    fn horizontally_centered_x(
        &self,
        anchor_bounds: IntRect,
        popup_content_size: IntSize,
        window_size: IntSize,
    ) -> i32 {
        // Horizontal alignment preference: middle -> start -> end

        // Tooltip prefers to be center aligned horizontally.
        let mut x = anchor_bounds.left + (anchor_bounds.width() - popup_content_size.width) / 2;

        if x < 0 {
            // Make tooltip start aligned if colliding with the left side of the screen
            let x_correction = (anchor_bounds.left + popup_content_size.width - window_size.width).max(0);
            x = anchor_bounds.left - x_correction;
        } else if x + popup_content_size.width > window_size.width {
            // Make tooltip end aligned if colliding with the right side of the screen
            x = (anchor_bounds.right - popup_content_size.width).max(0);
        }
        x
    }

    fn above_positioning(
        &self,
        anchor_bounds: IntRect,
        popup_content_size: IntSize,
        window_size: IntSize,
    ) -> IntOffset {
        let x = self.horizontally_centered_x(anchor_bounds, popup_content_size, window_size);

        // Vertical preference: above -> below. Tooltip prefers to be above the anchor, but if this causes the
        // tooltip to overlap with the anchor then we place it below the anchor
        let mut y = anchor_bounds.top - popup_content_size.height - self.tooltip_anchor_spacing;
        if y < 0 {
            y = anchor_bounds.bottom + self.tooltip_anchor_spacing;
        }
        IntOffset::new(x, y)
    }

    fn below_positioning(
        &self,
        anchor_bounds: IntRect,
        popup_content_size: IntSize,
        window_size: IntSize,
    ) -> IntOffset {
        let x = self.horizontally_centered_x(anchor_bounds, popup_content_size, window_size);

        // Vertical preference: below -> above. Tooltip prefers to be below the anchor, but if this causes the
        // tooltip to overlap with the anchor then we place it above the anchor
        let mut y = anchor_bounds.bottom + self.tooltip_anchor_spacing;
        if y + popup_content_size.height > window_size.height {
            y = anchor_bounds.top - popup_content_size.height - self.tooltip_anchor_spacing;
        }
        IntOffset::new(x, y)
    }

    fn start_positioning(
        &self,
        layout_direction: LayoutDirection,
        anchor_bounds: IntRect,
        popup_content_size: IntSize,
        window_size: IntSize,
    ) -> IntOffset {
        if layout_direction == LayoutDirection::Ltr {
            self.left_positioning(anchor_bounds, popup_content_size, window_size)
        } else {
            self.right_positioning(anchor_bounds, popup_content_size, window_size)
        }
    }

    fn end_positioning(
        &self,
        layout_direction: LayoutDirection,
        anchor_bounds: IntRect,
        popup_content_size: IntSize,
        window_size: IntSize,
    ) -> IntOffset {
        if layout_direction == LayoutDirection::Ltr {
            self.right_positioning(anchor_bounds, popup_content_size, window_size)
        } else {
            self.left_positioning(anchor_bounds, popup_content_size, window_size)
        }
    }
}

impl PopupPositionProvider for TooltipPositionProviderImpl {
    fn calculate_position(
        &self,
        anchor_bounds: IntRect,
        _window_size: IntSize,
        layout_direction: LayoutDirection,
        popup_content_size: IntSize,
    ) -> IntOffset {
        let window = self.window_container_size;
        match self.r#type {
            TooltipAnchorPosition::Left => self.left_positioning(anchor_bounds, popup_content_size, window),
            TooltipAnchorPosition::Right => self.right_positioning(anchor_bounds, popup_content_size, window),
            TooltipAnchorPosition::Above => self.above_positioning(anchor_bounds, popup_content_size, window),
            TooltipAnchorPosition::Below => self.below_positioning(anchor_bounds, popup_content_size, window),
            TooltipAnchorPosition::Start => {
                self.start_positioning(layout_direction, anchor_bounds, popup_content_size, window)
            }
            TooltipAnchorPosition::End => {
                self.end_positioning(layout_direction, anchor_bounds, popup_content_size, window)
            }
            _ => self.above_positioning(anchor_bounds, popup_content_size, window),
        }
    }
}

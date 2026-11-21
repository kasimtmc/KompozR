//! Tooltips, as Compose's Tooltip.kt — the base its components stand on: a tooltip's state ([`TooltipState`]), the
//! scope its content is in ([`TooltipScope`]), where it goes around its anchor ([`TooltipAnchorPosition`]) and the
//! position provider that puts it there, flipping it to the other side at the window's edges
//! ([`TooltipDefaults::remember_tooltip_position_provider`]).

// (WP5: the deprecated #[named] position providers' builders call them, as chip.rs's)
#![allow(deprecated)]

use crate::content_color::LocalContentColor;
use crate::internal::basic_tooltip::{BasicTooltipBox, BasicTooltipDefaults, Shown, WaitCancelled};
use crate::surface::Surface;
use crate::text::LocalTextStyle;
use crate::tokens::{MotionSchemeKeyTokens, PlainTooltipTokens};
use kompoz_animation::{MutableTransitionState, MutatePriority, MutatorMutex, remember_transition};
use kompoz_coroutines::with_timeout;
use kompoz_foundation::gestures::LocalFuture;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{
    Color, Density, Dp, DpSize, IntOffset, IntRect, IntSize, LayoutDirection, Matrix, Offset, Outline, Path,
    PathDirection, PathOperation, Rect, Shape, Size,
};
use kompoz_runtime::{
    KompozitionLocalProvider, MutableState, derived_state_of, key, kompozable, mutable_state_of, named, named_args,
    remember, remember_keyed,
};
use kompoz_ui::Modifier;
use kompoz_ui::layout::{GraphicsLayerScope, LayoutCoordinates, MeasureScope};
use kompoz_ui::platform::{LocalDensity, LocalWindowInfo};
use kompoz_ui::window::PopupPositionProvider;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

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
    ///
    /// Kotlin: `show(mutatePriority: MutatePriority = MutatePriority.Default)`
    fn show(&self, mutate_priority: MutatePriority) -> LocalFuture<'static, ()>;

    /// Dismisses the tooltip if it is shown (Compose's `dismiss`).
    fn dismiss(&self);

    /// Cleans up when the state leaves the composition (Compose's `onDispose`).
    fn on_dispose(&self);
}

mod sealed {
    use kompoz_ui::layout::LayoutCoordinates;
    use std::rc::Rc;

    pub trait Sealed {
        /// The anchor's coordinates as a function to keep (WP5: what `PlainTooltip`'s caret layout reads as it
        /// measures, Kotlin's `{ obtainAnchorBounds() }` lambda capturing the scope).
        fn anchor_bounds_getter(&self) -> Rc<dyn Fn() -> Option<LayoutCoordinates>>;
    }
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

impl sealed::Sealed for TooltipScopeImpl {
    fn anchor_bounds_getter(&self) -> Rc<dyn Fn() -> Option<LayoutCoordinates>> {
        self.get_anchor_bounds.clone()
    }
}

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

/// Tooltips' defaults (Compose's `object TooltipDefaults`): their position providers, the plain tooltip's colors,
/// shape and width, the caret's size and shape. (The rich tooltip's members come with `RichTooltip`.)
#[derive(Clone, Copy, Debug)]
pub struct TooltipDefaults;

// (WP5: tooltips — the position providers take their spacing's default; the plain tooltip's members)
#[named]
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
        &self,
        positioning: TooltipAnchorPosition,
        #[default(SpacingBetweenTooltipAndAnchor)] spacing_between_tooltip_and_anchor: Dp,
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
        &self,
        #[default(SpacingBetweenTooltipAndAnchor)] spacing_between_tooltip_and_anchor: Dp,
    ) -> Rc<dyn PopupPositionProvider> {
        kompoz_runtime::kompozable_block!({
            let tooltip_anchor_spacing = LocalDensity.current().round_to_px(spacing_between_tooltip_and_anchor);
            remember_keyed(tooltip_anchor_spacing, move || {
                Rc::new(AboveTooltipPositionProvider { tooltip_anchor_spacing }) as Rc<dyn PopupPositionProvider>
            })
        })
    }

    /// The position provider of a plain tooltip (Compose's deprecated `rememberPlainTooltipPositionProvider`):
    /// above its anchor, `spacing` from it, centered on it (even past the window's sides), below it where above
    /// would leave the window.
    ///
    /// Kotlin: `rememberPlainTooltipPositionProvider(spacingBetweenTooltipAndAnchor: Dp = SpacingBetweenTooltipAndAnchor): PopupPositionProvider`
    #[deprecated(note = "Deprecated in favor of rememberTooltipPositionProvider API.")]
    #[track_caller]
    pub fn remember_plain_tooltip_position_provider(
        &self,
        #[default(SpacingBetweenTooltipAndAnchor)] spacing_between_tooltip_and_anchor: Dp,
    ) -> Rc<dyn PopupPositionProvider> {
        kompoz_runtime::kompozable_block!({
            let tooltip_anchor_spacing = LocalDensity.current().round_to_px(spacing_between_tooltip_and_anchor);
            remember_keyed(tooltip_anchor_spacing, move || {
                Rc::new(PlainTooltipPositionProvider { tooltip_anchor_spacing }) as Rc<dyn PopupPositionProvider>
            })
        })
    }
}

impl TooltipDefaults {
    /// The default caret size of tooltips, 16 × 8 dp.
    ///
    /// Kotlin: `caretSize`
    pub const CaretSize: DpSize = DpSize { width: Dp(16.0), height: Dp(8.0) };

    /// The default maximum width of plain tooltips, 200 dp.
    ///
    /// Kotlin: `plainTooltipMaxWidth`
    pub const PlainTooltipMaxWidth: Dp = Dp(200.0);

    /// A plain tooltip's container shape: the theme's extra small corners (Compose's `plainTooltipContainerShape`).
    pub fn plain_tooltip_container_shape() -> Arc<dyn Shape> {
        PlainTooltipTokens::ContainerShape.value()
    }

    /// A plain tooltip's container color: the inverse surface (Compose's `plainTooltipContainerColor`).
    pub fn plain_tooltip_container_color() -> Color {
        PlainTooltipTokens::ContainerColor.value()
    }

    /// A plain tooltip's content color: the inverse on-surface (Compose's `plainTooltipContentColor`).
    pub fn plain_tooltip_content_color() -> Color {
        PlainTooltipTokens::SupportingTextColor.value()
    }

    /// The default caret shape of tooltips, of [`CaretSize`](TooltipDefaults::CaretSize) (Compose's
    /// `caretShape()`: one shape, shared).
    ///
    /// Kotlin: `caretShape()`
    pub fn caret_shape() -> Arc<dyn Shape> {
        thread_local! {
            static DEFAULT_CARET_SHAPE: Arc<dyn Shape> =
                Arc::new(DefaultTooltipCaretShape::new(TooltipDefaults::CaretSize));
        }
        DEFAULT_CARET_SHAPE.with(Arc::clone)
    }

    /// The caret shape of tooltips of `caret_size` (Compose's `caretShape(caretSize)`; Kotlin's default, the caret
    /// size, is [`caret_shape`](TooltipDefaults::caret_shape)'s, what a call without arguments picks).
    ///
    /// Kotlin: `caretShape(caretSize: DpSize): Shape`
    pub fn caret_shape_of_size(caret_size: DpSize) -> Arc<dyn Shape> {
        Arc::new(DefaultTooltipCaretShape::new(caret_size))
    }
}

/// The deprecated plain tooltip provider's `object : PopupPositionProvider` (above, centered; below if above
/// leaves the window).
struct PlainTooltipPositionProvider {
    tooltip_anchor_spacing: i32,
}

impl PopupPositionProvider for PlainTooltipPositionProvider {
    fn calculate_position(
        &self,
        anchor_bounds: IntRect,
        _window_size: IntSize,
        _layout_direction: LayoutDirection,
        popup_content_size: IntSize,
    ) -> IntOffset {
        let x = anchor_bounds.left + (anchor_bounds.width() - popup_content_size.width) / 2;

        // Tooltip prefers to be above the anchor,
        // but if this causes the tooltip to overlap with the anchor
        // then we place it below the anchor
        let mut y = anchor_bounds.top - popup_content_size.height - self.tooltip_anchor_spacing;
        if y < 0 {
            y = anchor_bounds.bottom + self.tooltip_anchor_spacing;
        }
        IntOffset::new(x, y)
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

// ==== (WP5: tooltips — TooltipBox, PlainTooltip, rememberTooltipState, the caret) ================================

/// A tooltip box (Compose's `TooltipBox`): `tooltip` (in a [`TooltipScope`]) in a popup placed by
/// `position_provider` while `state` is visible, around the anchor `content` (in a Box of `modifier`), growing from
/// 80% and fading in as it shows (the motion scheme's fast spatial and fast effects specs), shrinking and fading out
/// as it goes. With `enable_user_input` a long press, the mouse over the anchor or the anchor's keyboard focus
/// shows it (see the internal `BasicTooltipBox`); a press outside calls `on_dismiss_request` (none: the state is
/// dismissed). `has_action`: the tooltip holds something to act on (accessibility and the keyboard reach it).
/// `focusable` is Compose's, but Compose for desktop's tooltip popup is never focusable (CMP-963).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TooltipBox(
    position_provider: Rc<dyn PopupPositionProvider>,
    tooltip: impl Fn(&dyn TooltipScope) + 'static,
    state: Rc<dyn TooltipState>,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] on_dismiss_request: Option<Rc<dyn Fn()>>,
    #[default(false)] focusable: bool,
    #[default(true)] enable_user_input: bool,
    #[default(false)] has_action: bool,
    content: impl Fn() + 'static,
) {
    // (Compose's deprecated `updateTransition(state.transition, …)`: `rememberTransition`'s twin)
    let transition = remember_transition(state.transition(), Some("tooltip transition".to_string()));
    let anchor_bounds = remember(AnchorBounds::new);
    let (bounds, provider) = (anchor_bounds.clone(), position_provider.clone());
    // (remembered once, as Compose's `remember { TooltipScopeImpl(…) }`: it keeps the first position provider)
    let scope = remember(move || Rc::new(TooltipScopeImpl::new(move || bounds.get(), provider)));

    let content = Rc::new(content);
    let anchor = anchor_bounds.clone();
    let wrapped_content = move || {
        let (anchor, content) = (anchor.clone(), content.clone());
        Box(
            modifier = Modifier.on_globally_positioned(move |it| anchor.set(Some(it.clone()))),
            content = move || content(),
        );
    };

    let tooltip_position = remember(|| mutable_state_of(None::<Offset>));
    // (Compose keys it by the two states, which never change: remembered once)
    let (position, anchor) = (tooltip_position.clone(), anchor_bounds.clone());
    let tooltip_side = remember(move || {
        derived_state_of(move || match (anchor.get(), position.get()) {
            (Some(anchor_bounds), Some(popup_position)) => {
                let anchor_position = anchor_bounds.position_on_screen();
                if popup_position.x <= anchor_position.x {
                    if popup_position.y < anchor_position.y {
                        1 // Top Left
                    } else {
                        3 // Bottom Left
                    }
                } else if popup_position.y < anchor_position.y {
                    2 // Top Right
                } else {
                    4 // Bottom Right
                }
            }
            _ => 0, // Default
        })
    });

    // Define the animation specifications from the motion tokens.
    let scale_spec = MotionSchemeKeyTokens::FastSpatial.value::<f32>();
    let alpha_spec = MotionSchemeKeyTokens::FastEffects.value::<f32>();

    // Animate scale based on the target visibility state.
    let scale = named_args!(transition.animate_float(
        transition_spec = move |_| scale_spec.clone(),
        label = "tooltip transition: scaling".to_string(),
        |it: &bool| if *it { 1.0 } else { 0.8 },
    ));

    // Animate alpha (transparency) based on the target visibility state.
    let alpha = named_args!(transition.animate_float(
        transition_spec = move |_| alpha_spec.clone(),
        label = "tooltip transition: alpha".to_string(),
        |it: &bool| if *it { 1.0 } else { 0.0 },
    ));

    let tooltip = Rc::new(tooltip);
    BasicTooltipBox(
        position_provider = position_provider,
        tooltip = move || {
            let (position, side, scale, alpha) =
                (tooltip_position.clone(), tooltip_side.clone(), scale.clone(), alpha.clone());
            let (tooltip, scope) = (tooltip.clone(), scope.clone());
            Box(
                modifier = Modifier.on_globally_positioned(move |it| position.set(Some(it.position_on_screen()))),
                content = move || {
                    let (scale, alpha, tooltip, scope) = (scale.clone(), alpha.clone(), tooltip.clone(), scope.clone());
                    key(&side.get(), move || {
                        Box(
                            modifier = Modifier.graphics_layer_block(move |l: &mut GraphicsLayerScope| {
                                l.scale_x = scale.get();
                                l.scale_y = scale.get();
                                l.alpha = alpha.get();
                            }),
                            content = move || tooltip(&*scope),
                        );
                    });
                },
            );
        },
        focusable = focusable,
        enable_user_input = enable_user_input,
        on_dismiss_request = on_dismiss_request,
        state = state,
        modifier = modifier,
        has_action = has_action,
        content = wrapped_content,
    );
}

/// A tooltip's scope as the trait object its content is given (what [`TooltipScopeComposables`] is implemented
/// for).
pub type DynTooltipScope<'a> = dyn TooltipScope + 'a;

/// The composables of a [`TooltipScope`] (Compose's extension functions `TooltipScope.PlainTooltip`, …), for
/// every scope (`dyn TooltipScope` too): `scope.PlainTooltip(…)`; Kotlin's `PlainTooltip { … }` in a tooltip
/// lambda.
#[named(trait TooltipScopeComposables)]
impl<'a> DynTooltipScope<'a> {
    /// A plain tooltip, a descriptive message (Compose's `TooltipScope.PlainTooltip`): `content` in `content_color`
    /// and the body small style, padded 8 × 4 dp, 40 × 24 dp at least and `max_width` at most, on a [`Surface`] of
    /// `shape` in `container_color` with `tonal_elevation` and `shadow_elevation`. With a `caret_shape` (see
    /// [`TooltipDefaults::caret_shape`]) a caret drawn of it points at the anchor (the tooltip's shape and the caret's
    /// united), placed by where the position provider put the tooltip; none: no caret.
    ///
    /// Kotlin: `PlainTooltip(modifier: Modifier = Modifier, caretShape: Shape? = null, maxWidth: Dp = TooltipDefaults.plainTooltipMaxWidth, shape: Shape = TooltipDefaults.plainTooltipContainerShape, contentColor: Color = TooltipDefaults.plainTooltipContentColor, containerColor: Color = TooltipDefaults.plainTooltipContainerColor, tonalElevation: Dp = 0.dp, shadowElevation: Dp = 0.dp, content: @Composable () -> Unit)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn PlainTooltip(
        &self,
        #[default(Modifier)] modifier: Modifier,
        #[default(None)] caret_shape: Option<Arc<dyn Shape>>,
        #[default(TooltipDefaults::PlainTooltipMaxWidth)] max_width: Dp,
        #[default(TooltipDefaults::plain_tooltip_container_shape())] shape: Arc<dyn Shape>,
        #[default(TooltipDefaults::plain_tooltip_content_color())] content_color: Color,
        #[default(TooltipDefaults::plain_tooltip_container_color())] container_color: Color,
        #[default(Dp(0.0))] tonal_elevation: Dp,
        #[default(Dp(0.0))] shadow_elevation: Dp,
        content: impl Fn() + 'static,
    ) {
        let content = Rc::new(content);
        kompoz_runtime::kompozable_block!({
            let (tooltip_shape, tooltip_modifier): (Arc<dyn Shape>, Modifier) = match caret_shape {
                Some(caret_shape) => {
                    let transformation_matrix = remember(|| mutable_state_of(Matrix::new()));
                    let density = LocalDensity.current();
                    let window_container_size = LocalWindowInfo.current().container_size();
                    let tooltip_modifier = layout_caret(
                        Modifier,
                        transformation_matrix.clone(),
                        density,
                        window_container_size,
                        self.anchor_bounds_getter(),
                        self.obtain_position_provider(),
                    )
                    .then(modifier);
                    let tooltip_shape =
                        remember_keyed((ShapeKey(shape.clone()), ShapeKey(caret_shape.clone())), move || {
                            Arc::new(TooltipCaretShape { transformation_matrix, tooltip_shape: shape, caret_shape })
                                as Arc<dyn Shape>
                        });
                    (tooltip_shape, tooltip_modifier)
                }
                None => (shape, modifier),
            };

            Surface(
                modifier = tooltip_modifier,
                shape = tooltip_shape,
                color = container_color,
                tonal_elevation = tonal_elevation,
                shadow_elevation = shadow_elevation,
                content = move || {
                    Box(
                        modifier = Modifier
                            .size_in(min_width = TooltipMinWidth, max_width = max_width, min_height = TooltipMinHeight)
                            .padding_values(PlainTooltipContentPadding()),
                        content = move || {
                            let text_style = PlainTooltipTokens::SupportingTextFont.value();
                            KompozitionLocalProvider(
                                &[LocalContentColor.provides(content_color), LocalTextStyle.provides(text_style)],
                                &*content,
                            );
                        },
                    );
                },
            );
        });
    }
}

/// A tooltip's state remembered while `is_persistent` and `mutator_mutex` stay the same (Compose's
/// `rememberTooltipState`): hidden unless `initial_is_visible`; persistent (staying until a click outside or
/// [`dismiss`](TooltipState::dismiss)) only if `is_persistent` — else it goes after a short while; one tooltip of
/// `mutator_mutex` shown at a time (by default, of all the tooltips').
#[kompozable]
pub fn remember_tooltip_state(
    #[default(false)] initial_is_visible: bool,
    #[default(false)] is_persistent: bool,
    #[default(BasicTooltipDefaults::GlobalMutatorMutex())] mutator_mutex: MutatorMutex,
) -> Rc<dyn TooltipState> {
    remember_keyed((is_persistent, mutator_mutex.clone()), move || {
        Rc::new(TooltipStateImpl::new(initial_is_visible, is_persistent, mutator_mutex)) as Rc<dyn TooltipState>
    })
}

/// A tooltip's state (Compose's constructor function `TooltipState(…)`), as [`remember_tooltip_state`]'s but not
/// remembered, and persistent by default.
///
/// Kotlin: `TooltipState(initialIsVisible: Boolean = false, isPersistent: Boolean = true, mutatorMutex: MutatorMutex = BasicTooltipDefaults.GlobalMutatorMutex): TooltipState`
pub fn TooltipState(
    initial_is_visible: bool,
    is_persistent: bool,
    mutator_mutex: MutatorMutex,
) -> Rc<dyn TooltipState> {
    Rc::new(TooltipStateImpl::new(initial_is_visible, is_persistent, mutator_mutex))
}

/// What [`TooltipStateImpl`] shares with its `show`'s cleanup.
struct TooltipStateCore {
    is_persistent: bool,
    transition: MutableTransitionState<bool>,
    /// The continuation used to clean up (Compose's `job`).
    job: RefCell<Option<Rc<Shown>>>,
}

impl TooltipStateCore {
    /// Compose's `dismiss`: the transition's target hidden, and a persistent tooltip's `show` let go of.
    fn dismiss(&self) {
        self.transition.set_target_state(false);
        if self.is_persistent
            && let Some(job) = self.job.borrow().as_ref()
        {
            job.cancel();
        }
    }
}

/// Compose's private `TooltipStateImpl`: visible while its transition is or goes there.
struct TooltipStateImpl {
    core: Rc<TooltipStateCore>,
    mutator_mutex: MutatorMutex,
}

impl TooltipStateImpl {
    fn new(initial_is_visible: bool, is_persistent: bool, mutator_mutex: MutatorMutex) -> TooltipStateImpl {
        TooltipStateImpl {
            core: Rc::new(TooltipStateCore {
                is_persistent,
                transition: MutableTransitionState::new(initial_is_visible),
                job: RefCell::new(None),
            }),
            mutator_mutex,
        }
    }
}

/// Dismisses the tooltip as `show`'s mutation ends, however it ends — timed out, cancelled, interrupted —, unless
/// it was shown with `PreventUserInput` (Kotlin's `finally { if (mutatePriority != PreventUserInput) dismiss() }`).
struct DismissOnDrop(Rc<TooltipStateCore>, MutatePriority);

impl Drop for DismissOnDrop {
    fn drop(&mut self) {
        if self.1 != MutatePriority::PreventUserInput {
            // timeout or cancellation has occurred and we close out the current tooltip.
            self.0.dismiss();
        }
    }
}

impl TooltipState for TooltipStateImpl {
    fn transition(&self) -> MutableTransitionState<bool> {
        self.core.transition.clone()
    }

    fn is_visible(&self) -> bool {
        let transition = &self.core.transition;
        transition.current_state() || transition.target_state()
    }

    fn is_persistent(&self) -> bool {
        self.core.is_persistent
    }

    /// Shows the tooltip of this state, the other tooltips of its mutex dismissed (Compose's `show`): until
    /// dismissed if it is persistent or shown for user input, else for 1.5 s. It ends only cancelled (Kotlin's
    /// CancellationException out of it): the coroutine awaiting it is cancelled.
    fn show(&self, mutate_priority: MutatePriority) -> LocalFuture<'static, ()> {
        let core = self.core.clone();
        let cancellable_show = {
            let core = core.clone();
            move || {
                let core = core.clone();
                async move {
                    let continuation = Rc::new(Shown::default());
                    core.transition.set_target_state(true);
                    *core.job.borrow_mut() = Some(continuation.clone());
                    WaitCancelled(continuation).await;
                }
            }
        };
        let mutex = self.mutator_mutex.clone();
        // Show associated tooltip for [TooltipDuration] amount of time
        // or until tooltip is explicitly dismissed depending on [isPersistent].
        let mutation = async move {
            let _finally = DismissOnDrop(core.clone(), mutate_priority);
            if core.is_persistent || mutate_priority == MutatePriority::UserInput {
                cancellable_show().await;
            } else {
                let duration = Duration::from_millis(BasicTooltipDefaults::TooltipDuration as u64);
                let _ = with_timeout(duration, move |_| cancellable_show()).await;
            }
        };
        std::boxed::Box::pin(async move {
            named_args!(mutex.mutate(mutate_priority, mutation)).await;
            // (cancelled or timed out: Kotlin's exception goes on out of `show`)
            kompoz_coroutines::cancel_current::<()>().await
        })
    }

    /// Dismisses the tooltip of this state if it is shown (Compose's `dismiss`).
    fn dismiss(&self) {
        self.core.dismiss();
    }

    /// Cleans up the mutex when the tooltip of this state leaves the composition (Compose's `onDispose`).
    fn on_dispose(&self) {
        if let Some(job) = self.core.job.borrow().as_ref() {
            job.cancel();
        }
    }
}

/// Where across the tooltip a caret above or below it points (Compose's internal `caretX`): at the anchor's
/// middle, the tooltip's middle where it is centered on it.
pub(crate) fn caret_x(tooltip_width: f32, screen_width_px: i32, anchor_bounds: Rect) -> f32 {
    let anchor_left = anchor_bounds.left;
    let anchor_right = anchor_bounds.right;
    let anchor_mid = (anchor_left + anchor_right) / 2.0;
    let screen_width_px = screen_width_px as f32;
    if tooltip_width >= screen_width_px {
        // Tooltip is greater than or equal to the width of the screen
        // The horizontal placement just needs to be in the center of the anchor
        anchor_mid
    } else if anchor_mid - tooltip_width / 2.0 < 0.0 {
        // The tooltip needs to be start aligned if it would
        // collide with the left side of screen when attempting to center.
        // We have a horizontal correction for the caret if the tooltip will
        // also collide with the right edge of the screen when start aligned
        let horizontal_correction = (tooltip_width - screen_width_px).max(-anchor_left);
        anchor_mid + horizontal_correction
    } else if anchor_mid + tooltip_width / 2.0 > screen_width_px {
        // The tooltip needs to be end aligned if it would
        // collide with the right side of the screen when attempting to center.
        // We have a horizontal correction for the caret if the tooltip will
        // also collide with the left edge of the screen when end aligned
        let horizontal_correction = (tooltip_width - anchor_right).min(0.0);
        anchor_mid + horizontal_correction
    } else {
        // Tooltip can centered neatly without colliding with screen edge
        tooltip_width / 2.0
    }
}

/// The caret's place and turn, as the tooltip is measured (Compose's private `Modifier.layoutCaret`): from the
/// anchor's bounds in the window and where the position provider (its type when it is the tooltip's own) puts the
/// tooltip, the matrix taking the caret's shape (pointing down from its top middle) to the tooltip's side facing
/// the anchor.
fn layout_caret(
    modifier: Modifier,
    transformation_matrix: MutableState<Matrix>,
    density: Density,
    window_container_size: IntSize,
    get_anchor_layout_coordinates: Rc<dyn Fn() -> Option<LayoutCoordinates>>,
    position_provider: Rc<dyn PopupPositionProvider>,
) -> Modifier {
    modifier.layout(move |scope, measurable, constraints| {
        let placeable = measurable.measure(constraints);
        let width = placeable.width();
        let height = placeable.height();
        let window_container_width_in_px = window_container_size.width;
        let window_container_height_in_px = window_container_size.height;
        let tooltip_width = width as f32;
        let tooltip_height = height as f32;
        let anchor_layout_coordinates = get_anchor_layout_coordinates();

        if let Some(anchor_layout_coordinates) = anchor_layout_coordinates {
            let screen_width_px = window_container_width_in_px;
            let tooltip_anchor_spacing = density.round_to_px(SpacingBetweenTooltipAndAnchor) as f32;
            let anchor_bounds = anchor_layout_coordinates.bounds_in_window();
            let anchor_top = anchor_bounds.top;
            let anchor_bottom = anchor_bounds.bottom;
            let anchor_right = anchor_bounds.right;
            let anchor_left = anchor_bounds.left;
            let window_width = window_container_width_in_px as f32;
            let window_height = window_container_height_in_px as f32;
            // (Kotlin's `positionProvider is TooltipPositionProviderImpl`: its type, else None)
            let provider: &dyn std::any::Any = &*position_provider;
            let r#type = provider.downcast_ref::<TooltipPositionProviderImpl>().map(|p| p.r#type);
            let above_or_below_y = || {
                if anchor_top - tooltip_height - tooltip_anchor_spacing < 0.0 { 0.0 } else { tooltip_height }
            };
            let caret_y = match r#type {
                Some(
                    TooltipAnchorPosition::Left
                    | TooltipAnchorPosition::Right
                    | TooltipAnchorPosition::Start
                    | TooltipAnchorPosition::End,
                ) => tooltip_height / 2.0,
                Some(TooltipAnchorPosition::Above) => above_or_below_y(),
                Some(TooltipAnchorPosition::Below) => {
                    if anchor_bottom + tooltip_height + tooltip_anchor_spacing > window_height {
                        tooltip_height
                    } else {
                        0.0
                    }
                }
                // (another positioning, or a custom position provider: treated as the above one)
                _ => above_or_below_y(),
            };

            // Whether a tooltip to the left of the anchor flips to its right, one to its right to its left.
            let left_flips = anchor_left - tooltip_anchor_spacing - tooltip_width < 0.0;
            let right_flips = anchor_right + tooltip_anchor_spacing + tooltip_width > window_width;
            let ltr = scope.layout_direction() == LayoutDirection::Ltr;
            // The caret's x for a tooltip at the left (true) or right of the anchor.
            let side_caret_x = |left: bool| {
                if left {
                    // We are placing the tooltip to the right of the anchor
                    if left_flips { 0.0 } else { tooltip_width }
                } else if right_flips {
                    // We are placing the tooltip to the left of the anchor
                    tooltip_width
                } else {
                    0.0
                }
            };
            let position = match r#type {
                Some(TooltipAnchorPosition::Left) => Offset::new(side_caret_x(true), caret_y),
                Some(TooltipAnchorPosition::Right) => Offset::new(side_caret_x(false), caret_y),
                Some(TooltipAnchorPosition::Start) => Offset::new(side_caret_x(ltr), caret_y),
                Some(TooltipAnchorPosition::End) => Offset::new(side_caret_x(!ltr), caret_y),
                _ => Offset::new(caret_x(tooltip_width, screen_width_px, anchor_bounds), caret_y),
            };

            // Translate matrix to position
            let mut matrix = Matrix::new();
            matrix.translate(position.x, position.y, 0.0);

            // We rotate matrix depending on positioning of the tooltip. Need to rotate it about the z axis by 90
            // degrees: one at the left of the anchor (or flipped to its right) points right (left).
            let side_rotation = |left: bool| {
                if left {
                    // Tooltip is being placed to the right of the anchor
                    if left_flips { 90.0 } else { -90.0 }
                } else if right_flips {
                    // Tooltip is being placed to the left of the anchor
                    -90.0
                } else {
                    90.0
                }
            };
            match r#type {
                Some(TooltipAnchorPosition::Left) => matrix.rotate_z(side_rotation(true)),
                Some(TooltipAnchorPosition::Right) => matrix.rotate_z(side_rotation(false)),
                Some(TooltipAnchorPosition::Start) => matrix.rotate_z(side_rotation(ltr)),
                Some(TooltipAnchorPosition::End) => matrix.rotate_z(side_rotation(!ltr)),
                _ => {
                    if caret_y == 0.0 {
                        // caret needs to be placed above tooltip
                        // Need to rotate it about the x axis by 180 degrees
                        matrix.rotate_x(180.0);
                    }
                }
            }
            transformation_matrix.set(matrix);
        }
        scope.layout(width, height, move |p| p.place(&placeable, 0, 0))
    })
}

/// The default caret shape of tooltips (Compose's `DefaultTooltipCaretShape`): a triangle `caret_size` wide and
/// tall, its top edge's middle at the origin, pointing down. (Equal to another of the same size: Kotlin's class
/// compares by identity.)
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DefaultTooltipCaretShape {
    /// The caret's size.
    pub caret_size: DpSize,
}

impl DefaultTooltipCaretShape {
    /// A caret of `caret_size` (Compose's `DefaultTooltipCaretShape(caretSize)`).
    ///
    /// Kotlin: `DefaultTooltipCaretShape(caretSize: DpSize = TooltipDefaults.caretSize)`
    pub const fn new(caret_size: DpSize) -> DefaultTooltipCaretShape {
        DefaultTooltipCaretShape { caret_size }
    }
}

impl Default for DefaultTooltipCaretShape {
    fn default() -> Self {
        DefaultTooltipCaretShape::new(TooltipDefaults::CaretSize)
    }
}

impl Shape for DefaultTooltipCaretShape {
    fn create_outline(&self, _size: Size, _layout_direction: LayoutDirection, density: &Density) -> Outline {
        let mut caret_path = Path::new();
        let caret_width_px = density.to_px(self.caret_size.width);
        let caret_height_px = density.to_px(self.caret_size.height);

        caret_path.move_to(0.0, 0.0);
        caret_path.line_to(caret_width_px / 2.0, 0.0);
        caret_path.line_to(0.0, caret_height_px);
        caret_path.line_to(-caret_width_px / 2.0, 0.0);
        caret_path.close();

        Outline::Generic(caret_path)
    }
}

/// A tooltip's shape with its caret (Compose's private `TooltipCaretShape`): the tooltip's outline united with the
/// caret's, moved and turned by the matrix the caret layout set (a state read as the outline is made). (Equal to
/// another of the same matrix state and shapes: Kotlin's compares by identity, made again only when those change.)
#[derive(Debug)]
struct TooltipCaretShape {
    transformation_matrix: MutableState<Matrix>,
    tooltip_shape: Arc<dyn Shape>,
    caret_shape: Arc<dyn Shape>,
}

impl PartialEq for TooltipCaretShape {
    fn eq(&self, other: &Self) -> bool {
        self.transformation_matrix == other.transformation_matrix
            && *self.tooltip_shape == *other.tooltip_shape
            && *self.caret_shape == *other.caret_shape
    }
}

/// A shape compared as shapes are (Kotlin's `remember(shape, caretShape)` keys).
#[derive(Clone)]
struct ShapeKey(Arc<dyn Shape>);

impl PartialEq for ShapeKey {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

/// A path of an outline (Compose's `when (outline) { is Outline.Generic -> path.addPath(…) … }`).
fn add_outline_to(path: &mut Path, outline: Outline) {
    match outline {
        Outline::Generic(p) => path.add_path(&p, Offset::Zero),
        Outline::Rounded(round_rect) => path.add_round_rect(&round_rect, PathDirection::CounterClockwise),
        Outline::Rectangle(rect) => path.add_rect(rect, PathDirection::CounterClockwise),
    }
}

impl Shape for TooltipCaretShape {
    fn create_outline(&self, size: Size, layout_direction: LayoutDirection, density: &Density) -> Outline {
        let mut tooltip_path = Path::new();
        let mut combined_path = Path::new();
        let mut caret_path = Path::new();

        let tooltip_outline = self.tooltip_shape.create_outline(size, layout_direction, density);
        let caret_outline = self.caret_shape.create_outline(size, layout_direction, density);

        add_outline_to(&mut tooltip_path, tooltip_outline);

        // Applies the given caret shape to the caret path that will be manipulated
        add_outline_to(&mut caret_path, caret_outline);

        caret_path.transform(&self.transformation_matrix.get());

        combined_path.op(&tooltip_path, &caret_path, PathOperation::Union);

        Outline::Generic(combined_path)
    }
}

/// The smallest a tooltip is (Compose's internal `TooltipMinHeight`, `TooltipMinWidth`).
pub(crate) const TooltipMinHeight: Dp = Dp(24.0);
pub(crate) const TooltipMinWidth: Dp = Dp(40.0);
const PlainTooltipVerticalPadding: Dp = Dp(4.0);
const PlainTooltipHorizontalPadding: Dp = Dp(8.0);
/// A plain tooltip's padding around its content (Compose's internal `PlainTooltipContentPadding`).
pub(crate) fn PlainTooltipContentPadding() -> PaddingValues {
    PaddingValues::hv(PlainTooltipHorizontalPadding, PlainTooltipVerticalPadding)
}

//! A state-based field's core node (Compose's TextFieldCoreModifier): on the inner field, inside the decoration;
//! it scrolls the text layout (a single-line field sideways, a multi-line one down), keeps the cursor in view as
//! the text, the selection or the sizes change, and draws the text, the selection's highlight, a handwriting
//! gesture's highlight and the blinking cursor.

use super::cursor_animation_state::CursorAnimationState;
use super::selection::{DefaultCursorThickness, TextFieldSelectionState};
use super::text_layout_state::TextLayoutState;
use super::transformed_text_field_state::TransformedTextFieldState;
use crate::gestures::{Orientation, ScrollableStateExtensions};
use crate::scroll::ScrollState;
use crate::text::input::TextHighlightType;
use crate::text::selection::LocalTextSelectionColors;
use kompoz_coroutines::{CoroutineStart, Job, snapshot_flow};
use kompoz_graphics::{Brush, Color, Constraints, Density, LayoutDirection, Rect};
use kompoz_runtime::named_args;
use kompoz_text::{TextLayoutResult, TextRange};
use kompoz_ui::draw::{ContentDrawScope, DefaultBlendMode, DrawModifierNode, DrawStyle};
use kompoz_ui::layout::{GlobalPositionAwareModifierNode, LayoutCoordinates, LayoutModifierNode, Measurable};
use kompoz_ui::platform::{LocalCursorBlinkEnabled, LocalWindowInfo};
use kompoz_ui::{
    MeasureResult, MeasureScope, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The core node's element (Compose's TextFieldCoreModifier, a data class: equal by value; states by identity).
#[derive(Clone)]
pub struct TextFieldCoreModifier {
    /// The field and its window have the focus.
    pub is_focused: bool,
    pub is_drag_hovered: bool,
    pub text_layout_state: Rc<TextLayoutState>,
    pub text_field_state: TransformedTextFieldState,
    pub text_field_selection_state: Rc<TextFieldSelectionState>,
    pub cursor_brush: Brush,
    pub writeable: bool,
    pub scroll_state: ScrollState,
    pub orientation: Orientation,
}

impl PartialEq for TextFieldCoreModifier {
    fn eq(&self, other: &Self) -> bool {
        self.is_focused == other.is_focused
            && self.is_drag_hovered == other.is_drag_hovered
            && Rc::ptr_eq(&self.text_layout_state, &other.text_layout_state)
            && self.text_field_state == other.text_field_state
            && Rc::ptr_eq(&self.text_field_selection_state, &other.text_field_selection_state)
            && self.cursor_brush == other.cursor_brush
            && self.writeable == other.writeable
            && self.scroll_state == other.scroll_state
            && self.orientation == other.orientation
    }
}

impl ModifierNodeElement for TextFieldCoreModifier {
    type Node = TextFieldCoreModifierNode;
    fn create(&self) -> TextFieldCoreModifierNode {
        TextFieldCoreModifierNode {
            shared: Rc::new(CoreShared {
                is_focused: Cell::new(self.is_focused),
                is_drag_hovered: Cell::new(self.is_drag_hovered),
                text_layout_state: RefCell::new(self.text_layout_state.clone()),
                text_field_state: RefCell::new(self.text_field_state.clone()),
                text_field_selection_state: RefCell::new(self.text_field_selection_state.clone()),
                cursor_brush: RefCell::new(self.cursor_brush.clone()),
                writeable: Cell::new(self.writeable),
                scroll_state: RefCell::new(self.scroll_state.clone()),
                orientation: Cell::new(self.orientation),
                cursor_animation: RefCell::new(None),
                change_observer_job: RefCell::new(None),
                handle: RefCell::new(None),
                followed: RefCell::new(Followed {
                    selection: None,
                    cursor_rect: Rect::new(-1.0, -1.0, -1.0, -1.0),
                    text_layout_size: 0,
                    container_size: 0,
                }),
            }),
        }
    }
    fn update(&self, node: &mut TextFieldCoreModifierNode) {
        node.update_node(self);
    }
    fn name(&self) -> &'static str {
        "textFieldCore"
    }
}

/// What the node and its cursor coroutine share.
struct CoreShared {
    is_focused: Cell<bool>,
    is_drag_hovered: Cell<bool>,
    text_layout_state: RefCell<Rc<TextLayoutState>>,
    text_field_state: RefCell<TransformedTextFieldState>,
    text_field_selection_state: RefCell<Rc<TextFieldSelectionState>>,
    cursor_brush: RefCell<Brush>,
    writeable: Cell<bool>,
    scroll_state: RefCell<ScrollState>,
    orientation: Cell<Orientation>,
    cursor_animation: RefCell<Option<Rc<CursorAnimationState>>>,
    change_observer_job: RefCell<Option<Job>>,
    handle: RefCell<Option<NodeHandle>>,
    followed: RefCell<Followed>,
}

/// The selection, cursor rect and sizes the scroll last followed (Compose's `previous…`).
struct Followed {
    selection: Option<TextRange>,
    cursor_rect: Rect,
    text_layout_size: i32,
    container_size: i32,
}

/// Whether a brush paints anything (Compose's `Brush.isSpecified`: an unspecified color doesn't).
fn is_specified(brush: &Brush) -> bool {
    brush.solid_color().is_none_or(|c| c.is_specified())
}

impl CoreShared {
    /// Whether the cursor shows while the field has the focus (Compose's `showCursor`): writeable, focused or
    /// hovered by a drag, a brush that paints.
    fn show_cursor(&self) -> bool {
        self.writeable.get()
            && (self.is_focused.get() || self.is_drag_hovered.get())
            && is_specified(&self.cursor_brush.borrow())
    }

    fn handle(&self) -> Option<NodeHandle> {
        self.handle.borrow().clone()
    }

    /// Blinks the cursor, restarted visible whenever the shown text changes, while the window has the focus
    /// (Compose's `startCursorJob`).
    fn start_cursor_job(self: &Rc<Self>) {
        let Some(handle) = self.handle() else { return };
        if self.cursor_animation.borrow().is_none() {
            let animate = handle.current_value_of(&LocalCursorBlinkEnabled);
            *self.cursor_animation.borrow_mut() = Some(CursorAnimationState::new(animate));
            // Draw didn't read the alpha before.
            handle.invalidate_draw();
        }
        let Some(scope) = handle.coroutine_scope() else { return };
        if let Some(old) = self.change_observer_job.borrow_mut().take() {
            old.cancel();
        }
        let shared = self.clone();
        let job = scope.launch(async move {
            // A sign flipping each time the block runs, so that every run is a new value (Compose's trick against
            // the flow's distinctness): 1 or 2 times it.
            let sign = Rc::new(Cell::new(1i32));
            let reader = shared.clone();
            let latest: Rc<RefCell<Option<Job>>> = Rc::new(RefCell::new(None));
            let flow = snapshot_flow(move || {
                // The shown text, so the blink restarts as it or the cursor moves.
                let _ = reader.text_field_state.borrow().visual_text();
                let handle = reader.handle();
                let is_window_focused = handle
                    .as_ref()
                    .is_some_and(|h| h.is_attached() && h.current_value_of(&LocalWindowInfo).is_window_focused());
                let value = if is_window_focused { 1 } else { 2 } * sign.get();
                sign.set(-sign.get());
                value
            });
            let blinker = shared.clone();
            kompoz_coroutines::coroutine_scope(move |scope| async move {
                flow.collect(move |is_window_focused: i32| {
                    // collectLatest: a new value cancels what the last one started.
                    if let Some(job) = latest.borrow_mut().take() {
                        job.cancel();
                    }
                    if is_window_focused.abs() == 1
                        && let Some(animation) = blinker.cursor_animation.borrow().clone()
                    {
                        *latest.borrow_mut() = Some(named_args!(scope.launch(
                            start = CoroutineStart::UNDISPATCHED,
                            coroutine = animation.snap_to_visible_and_animate()
                        )));
                    }
                })
                .await;
            })
            .await;
        });
        *self.change_observer_job.borrow_mut() = Some(job);
    }

    /// Which offset to bring into view (Compose's `calculateOffsetToFollow`): the end or the start that moved, or
    /// the start when a size changed.
    fn calculate_offset_to_follow(
        &self,
        current: TextRange,
        container_size: i32,
        text_layout_size: i32,
    ) -> Option<usize> {
        let f = self.followed.borrow();
        if Some(current.end) != f.selection.map(|p| p.end) {
            Some(current.end)
        } else if Some(current.start) != f.selection.map(|p| p.start)
            || text_layout_size != f.text_layout_size
            || container_size != f.container_size
        {
            Some(current.start)
        } else {
            None
        }
    }

    /// Keeps the cursor in view after the text, the selection or a size changed (Compose's `updateScrollState`):
    /// the scroll's viewport and maximum, then a scroll by what the cursor is out of view, then a request to bring
    /// it into view in the containers above.
    fn update_scroll_state(
        self: &Rc<Self>,
        density: Density,
        container_size: i32,
        text_layout_size: i32,
        current: TextRange,
        layout_direction: LayoutDirection,
    ) {
        let s = self.clone();
        let scroll_state = s.scroll_state.borrow().clone();
        scroll_state.set_viewport_size(container_size);
        scroll_state.set_max_value(text_layout_size - container_size);
        let Some(offset_to_follow) = self.calculate_offset_to_follow(current, container_size, text_layout_size) else {
            return;
        };
        if !s.show_cursor() {
            return;
        }
        let layout_state = s.text_layout_state.borrow().clone();
        let Some(layout) = layout_state.layout_result() else { return };
        let text_length = layout.layout_input.text.text().len();
        let raw_cursor_rect = layout.get_cursor_rect(offset_to_follow.min(text_length));
        let cursor_rect = get_cursor_rect_in_scroller(
            density,
            raw_cursor_rect,
            layout_direction == LayoutDirection::Rtl,
            text_layout_size,
        );
        let should_bring_into_view = {
            let f = self.followed.borrow();
            let should = cursor_rect.left != f.cursor_rect.left
                || cursor_rect.top != f.cursor_rect.top
                || text_layout_size != f.text_layout_size;
            if !should && container_size == f.container_size {
                return;
            }
            should
        };
        let vertical = s.orientation.get() == Orientation::Vertical;
        let (cursor_start, cursor_end) =
            if vertical { (cursor_rect.top, cursor_rect.bottom) } else { (cursor_rect.left, cursor_rect.right) };
        let start_visible_bound = scroll_state.value() as f32;
        let end_visible_bound = start_visible_bound + container_size as f32;
        let offset_difference = if cursor_end > end_visible_bound {
            // The cursor's end into view.
            cursor_end - end_visible_bound
        } else if cursor_start < start_visible_bound && cursor_end - cursor_start > container_size as f32 {
            // Not room for all of it: its end.
            cursor_end - end_visible_bound
        } else if cursor_start < start_visible_bound && cursor_end - cursor_start <= container_size as f32 {
            cursor_start - start_visible_bound
        } else {
            0.0
        };
        *self.followed.borrow_mut() =
            Followed { selection: Some(current), cursor_rect, text_layout_size, container_size };
        let Some(scope) = s.handle().and_then(|h| h.coroutine_scope()) else { return };
        let requester = layout_state.bring_into_view_requester().clone();
        named_args!(scope.launch(start = CoroutineStart::UNDISPATCHED, async move {
            let target_scroll = round_to_next(offset_difference);
            let scrolled = scroll_state.scroll_by(target_scroll).await;
            if should_bring_into_view {
                // A caret too thin to be anything to bring into view is a pixel wide when the scroll fell short.
                let rect = if !current.collapsed()
                    || raw_cursor_rect.width() > 0.0
                    || (target_scroll - scrolled).abs() < 1.0
                {
                    raw_cursor_rect
                } else {
                    Rect::new(
                        raw_cursor_rect.left,
                        raw_cursor_rect.top,
                        raw_cursor_rect.right + 1.0,
                        raw_cursor_rect.bottom,
                    )
                };
                requester.bring_into_view_rect(Some(rect)).await;
            }
        }));
    }

    fn stop_cursor_job(&self) {
        if let Some(job) = self.change_observer_job.borrow_mut().take() {
            job.cancel();
        }
        if let Some(animation) = self.cursor_animation.borrow().as_ref() {
            animation.cancel_and_hide();
        }
    }
}

/// The core node (Compose's TextFieldCoreModifierNode).
pub struct TextFieldCoreModifierNode {
    shared: Rc<CoreShared>,
}

impl TextFieldCoreModifierNode {
    /// Takes a new element's values (Compose's `updateNode`).
    fn update_node(&mut self, e: &TextFieldCoreModifier) {
        let s = &self.shared;
        let previous_show_cursor = s.show_cursor();
        let was_focused = s.is_focused.get();
        let previous_state = s.text_field_state.borrow().clone();
        let previous_layout_state = s.text_layout_state.borrow().clone();
        let previous_selection_state = s.text_field_selection_state.borrow().clone();
        let previous_scroll_state = s.scroll_state.borrow().clone();

        s.is_focused.set(e.is_focused);
        e.text_field_selection_state.set_is_window_and_text_field_focused(e.is_focused);
        s.is_drag_hovered.set(e.is_drag_hovered);
        *s.text_layout_state.borrow_mut() = e.text_layout_state.clone();
        *s.text_field_state.borrow_mut() = e.text_field_state.clone();
        *s.text_field_selection_state.borrow_mut() = e.text_field_selection_state.clone();
        *s.cursor_brush.borrow_mut() = e.cursor_brush.clone();
        s.writeable.set(e.writeable);
        *s.scroll_state.borrow_mut() = e.scroll_state.clone();
        s.orientation.set(e.orientation);

        if !s.show_cursor() {
            s.stop_cursor_job();
        } else if !was_focused || previous_state != e.text_field_state || !previous_show_cursor {
            s.start_cursor_job();
        }

        if (previous_state != e.text_field_state
            || !Rc::ptr_eq(&previous_layout_state, &e.text_layout_state)
            || !Rc::ptr_eq(&previous_selection_state, &e.text_field_selection_state)
            || previous_scroll_state != e.scroll_state)
            && let Some(h) = s.handle()
        {
            h.invalidate_measurement();
        }
    }

    /// Draws a handwriting gesture's preview highlight (Compose's `drawHighlight`).
    fn draw_highlight(
        &self,
        scope: &mut ContentDrawScope,
        highlight: (TextHighlightType, TextRange),
        layout: &TextLayoutResult,
        selection_background: Color,
    ) {
        let (kind, range) = highlight;
        if range.collapsed() {
            return;
        }
        let path = layout.get_path_for_range(range.min(), range.max());
        if kind == TextHighlightType::HandwritingDeletePreview {
            // The text's own color at 20%.
            let style = &layout.layout_input.style;
            if let Some(brush) = &style.brush {
                scope.draw_path_with(&path, brush.clone(), 0.2, DrawStyle::Fill, None, DefaultBlendMode);
            } else {
                let color = if style.color.is_specified() { style.color } else { Color::Black };
                let faded = named_args!(color.copy(alpha = color.alpha() * 0.2));
                scope.draw_path_with(&path, faded, 1.0, DrawStyle::Fill, None, DefaultBlendMode);
            }
        } else {
            scope.draw_path_with(&path, selection_background, 1.0, DrawStyle::Fill, None, DefaultBlendMode);
        }
    }

    /// Draws the cursor (Compose's `drawDefaultCursor`): only while it shows and its alpha (read here, so its
    /// changes redraw) isn't 0.
    fn draw_cursor(&self, scope: &mut ContentDrawScope) {
        let alpha = self.shared.cursor_animation.borrow().as_ref().map_or(0.0, |a| a.cursor_alpha());
        if alpha == 0.0 || !self.shared.show_cursor() {
            return;
        }
        let rect = self.shared.text_field_selection_state.borrow().get_cursor_rect();
        let brush = self.shared.cursor_brush.borrow().clone();
        scope.draw_line_with(
            brush,
            rect.top_center(),
            rect.bottom_center(),
            rect.width(),
            kompoz_graphics::StrokeCap::Butt,
            None,
            alpha,
            None,
            DefaultBlendMode,
        );
    }
}

impl ModifierNode for TextFieldCoreModifierNode {
    node_capabilities!(layout, draw, global_position_aware);

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.shared.handle.borrow_mut() = Some(cx.handle());
        // Attached with the focus (an inner field toggled in and out by its decorator): the cursor at once.
        let focused = self.shared.is_focused.get();
        self.shared.text_field_selection_state.borrow().set_is_window_and_text_field_focused(focused);
        if focused && self.shared.show_cursor() {
            self.shared.start_cursor_job();
        }
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        self.shared.change_observer_job.borrow_mut().take();
    }
}

impl LayoutModifierNode for TextFieldCoreModifierNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let density = Density::new(scope.density, scope.font_scale);
        let direction = scope.layout_direction();
        let selection = self.shared.text_field_state.borrow().visual_text().selection();
        let scroll_state = self.shared.scroll_state.borrow().clone();
        if self.shared.orientation.get() == Orientation::Vertical {
            // It scrolls down: no height limit for the text.
            let placeable = measurable.measure(Constraints::new(
                constraints.min_width,
                constraints.max_width,
                constraints.min_height,
                Constraints::Infinity,
            ));
            let height = placeable.height().min(constraints.max_height);
            let width = placeable.width();
            let shared = self.shared.clone();
            MeasureResult::new(width, height, Vec::new(), move |p| {
                // The cursor kept in view once laid out.
                shared.update_scroll_state(density, height, placeable.height(), selection, direction);
                p.place_relative(&placeable, 0, -scroll_state.value())
            })
        } else {
            // It scrolls sideways: no width limit for the text.
            let placeable = measurable.measure(Constraints::new(
                constraints.min_width,
                Constraints::Infinity,
                constraints.min_height,
                constraints.max_height,
            ));
            let width = placeable.width().min(constraints.max_width);
            let height = placeable.height();
            let shared = self.shared.clone();
            MeasureResult::new(width, height, Vec::new(), move |p| {
                shared.update_scroll_state(density, width, placeable.width(), selection, direction);
                p.place_relative(&placeable, -scroll_state.value(), 0)
            })
        }
    }
}

impl DrawModifierNode for TextFieldCoreModifierNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        scope.draw_content();
        let value = self.shared.text_field_state.borrow().visual_text();
        let layout_state = self.shared.text_layout_state.borrow().clone();
        let Some(layout) = layout_state.layout_result() else { return };
        let selection_background = self
            .shared
            .handle()
            .map_or_else(|| LocalTextSelectionColors.default_value(), |h| h.current_value_of(&LocalTextSelectionColors))
            .background_color;
        if let Some(highlight) = value.highlight() {
            self.draw_highlight(scope, highlight, &layout, selection_background);
        }
        let selection = value.selection();
        if selection.collapsed() {
            draw_text(scope, &layout);
            if value.should_show_selection() {
                self.draw_cursor(scope);
            }
        } else {
            if value.should_show_selection() && selection.min() != selection.max() {
                // Compose's `drawDefaultSelectionHighlight`.
                let path = layout.get_path_for_range(selection.min(), selection.max());
                scope.draw_path_with(&path, selection_background, 1.0, DrawStyle::Fill, None, DefaultBlendMode);
            }
            draw_text(scope, &layout);
        }
    }
}

impl GlobalPositionAwareModifierNode for TextFieldCoreModifierNode {
    fn on_globally_positioned(&mut self, coordinates: &LayoutCoordinates) {
        self.shared.text_layout_state.borrow().set_core_node_coordinates(Some(coordinates.clone()));
    }
}

/// Paints the laid out text in its own style (Compose's `TextPainter.paint`).
fn draw_text(scope: &mut ContentDrawScope, layout: &TextLayoutResult) {
    scope.draw_text_with(
        layout,
        Color::Unspecified,
        kompoz_graphics::Offset::Zero,
        f32::NAN,
        None,
        None,
        None,
        DefaultBlendMode,
    );
}

/// The cursor rect in the scroller's coordinates (Compose's `getCursorRectInScroller`): the cursor's thickness
/// wide, mirrored for right to left, within the text layout.
fn get_cursor_rect_in_scroller(density: Density, cursor_rect: Rect, rtl: bool, text_layout_size: i32) -> Rect {
    let thickness = density.round_to_px(DefaultCursorThickness) as f32;
    let left = if rtl { text_layout_size as f32 - cursor_rect.right } else { cursor_rect.left };
    let right =
        if rtl { text_layout_size as f32 - cursor_rect.right + thickness } else { cursor_rect.left + thickness }
            .min(text_layout_size as f32);
    Rect::new(left, cursor_rect.top, right, cursor_rect.bottom)
}

/// Rounds away from zero: a negative number down, a positive one up (Compose's `roundToNext`).
fn round_to_next(v: f32) -> f32 {
    if v.is_nan() || v.is_infinite() {
        v
    } else if v > 0.0 {
        v.ceil()
    } else {
        v.floor()
    }
}

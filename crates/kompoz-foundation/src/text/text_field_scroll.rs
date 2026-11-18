//! A value-based field's scrolling (Compose's TextFieldScroll.kt): its [`TextFieldScrollerPosition`] (how far it
//! scrolled, how far it can), the decoration box's scrollable over it (a drag, a fling, the wheel), and the inner
//! field's layout letting the text be as tall (or, single-line, as wide) as it is and scrolling it so the cursor
//! stays in view.

use super::modifiers::layout_utils::round_to_int;
use super::text_layout_result_proxy::TextLayoutResultProxy;
use super::validating_offset_mapping::filter_with_validation;
use crate::gestures::{
    DefaultScrollableState, LocalFuture, Orientation, ScrollBlock, ScrollIndicatorState, ScrollableModifiers,
    ScrollableState, remember_scrollable_state,
};
use crate::interaction::MutableInteractionSource;
use crate::overscroll::{OverscrollEffect, OverscrollModifiers};
use crate::text::input::internal::selection::DefaultCursorThickness;
use kompoz_animation::MutatePriority;
use kompoz_graphics::{Constraints, Density, LayoutDirection, Rect};
use kompoz_runtime::{
    DerivedState, MutableState, SavedValue, Saver, derived_state_of, list_saver, mutable_state_of, remember_keyed,
};
use kompoz_text::input::{TextFieldValue, TransformedText, VisualTransformation};
use kompoz_text::{TextLayoutResult, TextRange};
use kompoz_ui::layout::{LayoutModifierNode, Measurable};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::{
    MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities,
};
use std::cell::Cell;
use std::rc::Rc;

/// What provides a field's latest text layout (Compose's `textLayoutResultProvider`).
pub type TextLayoutResultProvider = Rc<dyn Fn() -> Option<Rc<TextLayoutResultProxy>>>;

struct PositionInner {
    offset: MutableState<f32>,
    maximum: MutableState<f32>,
    viewport_size: MutableState<i32>,
    previous_cursor_rect: Cell<Rect>,
    previous_selection: Cell<TextRange>,
    orientation: MutableState<Orientation>,
}

/// How far a value-based field scrolled (Compose's TextFieldScrollerPosition): its offset (px, states), the most it
/// can scroll, the visible size, and which way it scrolls. Cheap to clone: clones are the same position.
#[derive(Clone)]
pub struct TextFieldScrollerPosition(Rc<PositionInner>);

impl PartialEq for TextFieldScrollerPosition {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for TextFieldScrollerPosition {
    /// Compose's `TextFieldScrollerPosition()`: vertical, at the top.
    fn default() -> Self {
        TextFieldScrollerPosition::new(Orientation::Vertical, 0.0)
    }
}

impl TextFieldScrollerPosition {
    /// One scrolling `initial_orientation`'s way, `initial` px scrolled.
    pub fn new(initial_orientation: Orientation, initial: f32) -> TextFieldScrollerPosition {
        TextFieldScrollerPosition(Rc::new(PositionInner {
            offset: mutable_state_of(initial),
            maximum: mutable_state_of(0.0),
            viewport_size: mutable_state_of(0),
            previous_cursor_rect: Cell::new(Rect::Zero),
            previous_selection: Cell::new(TextRange::Zero),
            orientation: mutable_state_of(initial_orientation),
        }))
    }

    /// How far it scrolled, px: the text's first visible pixel (Compose's `offset`).
    pub fn offset(&self) -> f32 {
        self.0.offset.get()
    }

    pub fn set_offset(&self, offset: f32) {
        self.0.offset.set(offset);
    }

    /// The most it can scroll, px: the text's size beyond the visible one (Compose's `maximum`).
    pub fn maximum(&self) -> f32 {
        self.0.maximum.get()
    }

    /// The visible size along the way it scrolls, px (Compose's `viewportSize`).
    pub fn viewport_size(&self) -> i32 {
        self.0.viewport_size.get()
    }

    /// The selection the field was last laid out with (Compose's `previousSelection`).
    pub fn previous_selection(&self) -> TextRange {
        self.0.previous_selection.get()
    }

    pub fn set_previous_selection(&self, selection: TextRange) {
        self.0.previous_selection.set(selection);
    }

    /// Which way it scrolls (Compose's `orientation`).
    pub fn orientation(&self) -> Orientation {
        self.0.orientation.get()
    }

    pub fn set_orientation(&self, orientation: Orientation) {
        self.0.orientation.set(orientation);
    }

    /// The field was laid out: its text `text_field_size` px along the way it scrolls in a container
    /// `container_size` px, the cursor at `cursor_rect` (Compose's `update`): the cursor brought into view when it
    /// moved, the offset kept within what can be scrolled.
    pub fn update(&self, orientation: Orientation, cursor_rect: Rect, container_size: i32, text_field_size: i32) {
        let difference = (text_field_size - container_size) as f32;
        self.0.maximum.set(difference);
        let previous = self.0.previous_cursor_rect.get();
        if cursor_rect.left != previous.left || cursor_rect.top != previous.top {
            let vertical = orientation == Orientation::Vertical;
            let cursor_start = if vertical { cursor_rect.top } else { cursor_rect.left };
            let cursor_end = if vertical { cursor_rect.bottom } else { cursor_rect.right };
            self.coerce_offset(cursor_start, cursor_end, container_size);
            self.0.previous_cursor_rect.set(cursor_rect);
        }
        let offset = self.0.offset.peek();
        // (Kotlin's coerceIn: a negative maximum leaves 0)
        self.0.offset.set(offset.min(difference).max(0.0));
        self.0.viewport_size.set(container_size);
    }

    /// Scrolls the cursor from `cursor_start` to `cursor_end` into a container `container_size` px long (Compose's
    /// `coerceOffset`): its end into view, or its start when it fits and is before the view.
    pub fn coerce_offset(&self, cursor_start: f32, cursor_end: f32, container_size: i32) {
        let start_visible_bound = self.0.offset.peek();
        let end_visible_bound = start_visible_bound + container_size as f32;
        let offset_difference = if cursor_end > end_visible_bound {
            cursor_end - end_visible_bound
        } else if cursor_start < start_visible_bound && cursor_end - cursor_start > container_size as f32 {
            // (not enough room for the whole cursor: its end first)
            cursor_end - end_visible_bound
        } else if cursor_start < start_visible_bound && cursor_end - cursor_start <= container_size as f32 {
            cursor_start - start_visible_bound
        } else {
            0.0
        };
        self.0.offset.set(start_visible_bound + offset_difference);
    }

    /// The selection's end the scroll follows (Compose's `getOffsetToFollow`): the one that moved, else its start.
    pub fn get_offset_to_follow(&self, selection: TextRange) -> usize {
        let previous = self.0.previous_selection.get();
        if selection.start != previous.start {
            selection.start
        } else if selection.end != previous.end {
            selection.end
        } else {
            selection.min()
        }
    }

    /// Saves its offset and orientation (Compose's `TextFieldScrollerPosition.Saver`).
    pub fn saver() -> Saver<TextFieldScrollerPosition> {
        list_saver(
            |p: &TextFieldScrollerPosition| {
                vec![
                    SavedValue::Double(p.0.offset.peek() as f64),
                    SavedValue::Bool(p.0.orientation.peek() == Orientation::Vertical),
                ]
            },
            |restored: &[SavedValue]| {
                let (SavedValue::Double(offset), SavedValue::Bool(vertical)) = (restored.first()?, restored.get(1)?)
                else {
                    return None;
                };
                let orientation = if *vertical { Orientation::Vertical } else { Orientation::Horizontal };
                Some(TextFieldScrollerPosition::new(orientation, *offset as f32))
            },
        )
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Scrollable

/// The scrollable state of a field's position (Compose's `ScrollableState by scrollableState` with its own
/// `canScrollForward`/`canScrollBackward`): it can scroll forward until the maximum, back until 0.
#[derive(Clone)]
struct TextFieldScrollableState {
    state: DefaultScrollableState,
    can_scroll_forward: DerivedState<bool>,
    can_scroll_backward: DerivedState<bool>,
    position: TextFieldScrollerPosition,
}

impl PartialEq for TextFieldScrollableState {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state && self.position == other.position
    }
}

impl ScrollableState for TextFieldScrollableState {
    fn scroll_block(&self, scroll_priority: MutatePriority, block: ScrollBlock) -> LocalFuture<'_, ()> {
        self.state.scroll_block(scroll_priority, block)
    }
    fn dispatch_raw_delta(&self, delta: f32) -> f32 {
        self.state.dispatch_raw_delta(delta)
    }
    fn is_scroll_in_progress(&self) -> bool {
        self.state.is_scroll_in_progress()
    }
    fn can_scroll_forward(&self) -> bool {
        self.can_scroll_forward.get()
    }
    fn can_scroll_backward(&self) -> bool {
        self.can_scroll_backward.get()
    }
    fn last_scrolled_forward(&self) -> bool {
        self.state.last_scrolled_forward()
    }
    fn last_scrolled_backward(&self) -> bool {
        self.state.last_scrolled_backward()
    }
    fn scroll_indicator_state(&self) -> Option<Rc<dyn ScrollIndicatorState>> {
        self.state.scroll_indicator_state()
    }
}

/// The decoration box's scrolling of a field (Compose's `textFieldScrollable`): drags, flings and the wheel move
/// `scroller_position` within 0 and its maximum (vertical and left to right reversed: the text moves with the
/// finger), emitted to `interaction_source`; only while `enabled` and there is something to scroll.
#[track_caller]
pub fn text_field_scrollable(
    modifier: Modifier,
    scroller_position: &TextFieldScrollerPosition,
    interaction_source: Option<MutableInteractionSource>,
    enabled: bool,
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
) -> Modifier {
    kompoz_runtime::group(|| {
        kompoz_runtime::kompozable_block!({
            // Not reversed only right to left, sideways.
            let rtl = LocalLayoutDirection.current() == LayoutDirection::Rtl;
            let reverse_direction = scroller_position.orientation() == Orientation::Vertical || !rtl;
            let position = scroller_position.clone();
            let scrollable_state = remember_scrollable_state(move |delta| {
                let offset = position.offset();
                let new_offset = offset + delta;
                let maximum = position.maximum();
                let consumed_delta = if new_offset > maximum {
                    maximum - offset
                } else if new_offset < 0.0 {
                    -offset
                } else {
                    delta
                };
                position.set_offset(offset + consumed_delta);
                consumed_delta
            });
            let (state, position) = (scrollable_state.clone(), scroller_position.clone());
            let wrapped = remember_keyed((scrollable_state, scroller_position.clone()), move || {
                let (forward, backward) = (position.clone(), position.clone());
                TextFieldScrollableState {
                    state,
                    can_scroll_forward: derived_state_of(move || forward.offset() < forward.maximum()),
                    can_scroll_backward: derived_state_of(move || backward.offset() > 0.0),
                    position,
                }
            });
            let orientation = scroller_position.orientation();
            let has_maximum = scroller_position.maximum() != 0.0;
            modifier.scrollable(
                state = wrapped,
                orientation = orientation,
                overscroll_effect = overscroll_effect,
                reverse_direction = reverse_direction,
                interaction_source = interaction_source,
                enabled = enabled && has_maximum,
            )
        })
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Layout

/// The inner field's scrolling layout (Compose's `textFieldScroll`, its `defaultTextFieldScroll`): the overscroll
/// drawn, the text clipped, and laid out as tall (or single-line, as wide) as it is, scrolled by the position —
/// which follows the selection end that moved (the cursor kept in view).
pub fn text_field_scroll(
    modifier: Modifier,
    scroller_position: &TextFieldScrollerPosition,
    text_field_value: &TextFieldValue,
    visual_transformation: &VisualTransformation,
    overscroll_effect: Option<Rc<dyn OverscrollEffect>>,
    text_layout_result_provider: TextLayoutResultProvider,
) -> Modifier {
    let orientation = scroller_position.orientation();
    let cursor_offset = scroller_position.get_offset_to_follow(text_field_value.selection());
    scroller_position.set_previous_selection(text_field_value.selection());
    let transformed_text = filter_with_validation(visual_transformation, text_field_value.annotated_string());
    let element = ScrollLayoutElement {
        orientation,
        scroller_position: scroller_position.clone(),
        cursor_offset,
        transformed_text,
        text_layout_result_provider,
    };
    let overscrolled = match overscroll_effect {
        Some(effect) => modifier.overscroll(Some(effect)),
        None => modifier,
    };
    overscrolled.clip_to_bounds().then_element(element)
}

/// The vertical and horizontal scroll layouts' element (Compose's VerticalScrollLayoutModifier and
/// HorizontalScrollLayoutModifier, data classes holding the layout provider: a new one each composition).
struct ScrollLayoutElement {
    orientation: Orientation,
    scroller_position: TextFieldScrollerPosition,
    cursor_offset: usize,
    transformed_text: TransformedText,
    text_layout_result_provider: TextLayoutResultProvider,
}

impl PartialEq for ScrollLayoutElement {
    fn eq(&self, other: &Self) -> bool {
        self.orientation == other.orientation
            && self.scroller_position == other.scroller_position
            && self.cursor_offset == other.cursor_offset
            && self.transformed_text == other.transformed_text
            && Rc::ptr_eq(&self.text_layout_result_provider, &other.text_layout_result_provider)
    }
}

impl ModifierNodeElement for ScrollLayoutElement {
    type Node = ScrollLayoutNode;
    fn create(&self) -> ScrollLayoutNode {
        ScrollLayoutNode {
            orientation: self.orientation,
            scroller_position: self.scroller_position.clone(),
            cursor_offset: self.cursor_offset,
            transformed_text: self.transformed_text.clone(),
            text_layout_result_provider: self.text_layout_result_provider.clone(),
            handle: None,
        }
    }
    fn update(&self, node: &mut ScrollLayoutNode) {
        node.orientation = self.orientation;
        node.scroller_position = self.scroller_position.clone();
        node.cursor_offset = self.cursor_offset;
        node.transformed_text = self.transformed_text.clone();
        node.text_layout_result_provider = self.text_layout_result_provider.clone();
        if let Some(h) = &node.handle {
            h.invalidate_measurement();
        }
    }
    fn name(&self) -> &'static str {
        match self.orientation {
            Orientation::Vertical => "VerticalScrollLayoutModifier",
            Orientation::Horizontal => "HorizontalScrollLayoutModifier",
        }
    }
}

struct ScrollLayoutNode {
    orientation: Orientation,
    scroller_position: TextFieldScrollerPosition,
    cursor_offset: usize,
    transformed_text: TransformedText,
    text_layout_result_provider: TextLayoutResultProvider,
    handle: Option<NodeHandle>,
}

impl ModifierNode for ScrollLayoutNode {
    node_capabilities!(layout);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
}

impl LayoutModifierNode for ScrollLayoutNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let density = scope.density();
        let (position, cursor_offset, transformed) =
            (self.scroller_position.clone(), self.cursor_offset, self.transformed_text.clone());
        let provider = self.text_layout_result_provider.clone();
        match self.orientation {
            Orientation::Vertical => {
                let child_constraints = Constraints::new(
                    constraints.min_width,
                    constraints.max_width,
                    constraints.min_height,
                    Constraints::Infinity,
                );
                let placeable = measurable.measure(child_constraints);
                let height = placeable.height().min(constraints.max_height);
                let width = placeable.width();
                MeasureResult::new(width, height, Vec::new(), move |p| {
                    let layout = provider().map(|proxy| proxy.value.clone());
                    let cursor_rect = get_cursor_rect_in_scroller(
                        density,
                        cursor_offset,
                        &transformed,
                        layout.as_ref(),
                        false,
                        placeable.width(),
                    );
                    position.update(Orientation::Vertical, cursor_rect, height, placeable.height());
                    let offset = -position.offset();
                    p.place_relative(&placeable, 0, round_to_int(offset));
                })
            }
            Orientation::Horizontal => {
                // Text narrower than the field: the original constraints, more to size it by.
                let max_intrinsic_width = measurable.max_intrinsic_width(constraints.max_height);
                let child_constraints = if max_intrinsic_width < constraints.max_width {
                    constraints
                } else {
                    Constraints::new(
                        constraints.min_width,
                        Constraints::Infinity,
                        constraints.min_height,
                        constraints.max_height,
                    )
                };
                let placeable = measurable.measure(child_constraints);
                let width = placeable.width().min(constraints.max_width);
                let height = placeable.height();
                let rtl = scope.layout_direction() == LayoutDirection::Rtl;
                MeasureResult::new(width, height, Vec::new(), move |p| {
                    let layout = provider().map(|proxy| proxy.value.clone());
                    let cursor_rect = get_cursor_rect_in_scroller(
                        density,
                        cursor_offset,
                        &transformed,
                        layout.as_ref(),
                        rtl,
                        placeable.width(),
                    );
                    position.update(Orientation::Horizontal, cursor_rect, width, placeable.width());
                    let offset = -position.offset();
                    p.place_relative(&placeable, round_to_int(offset), 0);
                })
            }
        }
    }
}

/// The cursor's rect in the scroller (Compose's `getCursorRectInScroller`): the cursor's thickness wide, mirrored
/// right to left.
fn get_cursor_rect_in_scroller(
    density: Density,
    cursor_offset: usize,
    transformed_text: &TransformedText,
    text_layout_result: Option<&TextLayoutResult>,
    rtl: bool,
    text_field_width: i32,
) -> Rect {
    let cursor_rect = text_layout_result
        .map(|l| l.get_cursor_rect(transformed_text.offset_mapping.original_to_transformed(cursor_offset)))
        .unwrap_or(Rect::Zero);
    let thickness = density.round_to_px(DefaultCursorThickness) as f32;
    let cursor_left = if rtl { text_field_width as f32 - cursor_rect.left - thickness } else { cursor_rect.left };
    let cursor_right = if rtl { text_field_width as f32 - cursor_rect.left } else { cursor_rect.left + thickness };
    Rect::new(cursor_left, cursor_rect.top, cursor_right, cursor_rect.bottom)
}

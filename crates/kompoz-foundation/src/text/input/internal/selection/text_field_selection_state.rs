//! A state-based field's selection (Compose's TextFieldSelectionState and its desktop actuals): where the cursor
//! is drawn and its handle, the selection handles, the text toolbar's showing, and the gestures that select —
//! taps placing the cursor, a finger's long press or double tap selecting words and its drags extending them, a
//! mouse's clicks (one: the cursor, two: a word, three: a paragraph) and drags, shift extending, the handles'
//! drags; and the clipboard actions (cut, copy, paste, select all).

use super::text_field_handle_state::TextFieldHandleState;
use crate::content::internal::ReceiveContentConfiguration;
use crate::content::{TransferableContent, TransferableContentSource};
use crate::gestures::{PointerInputScopeGestures, PressCallback, detect_press_down_gesture, detect_tap_and_press};
use crate::interaction::{MutableInteractionSource, PressInteraction};
use crate::text::input::TextFieldCharSequence;
use crate::text::input::internal::undo::TextFieldEditUndoBehavior;
use crate::text::input::internal::{
    IndexTransformationType, SelectionWedgeAffinity, TextLayoutState, TransformedTextFieldState, WedgeAffinity,
};
use crate::text::long_press_text_drag_observer::{Handle, TextDragObserver};
use crate::text::selection::{
    MouseSelectionObserver, SelectionAdjustment, SelectionLayout, await_selection_gestures,
    get_selection_handle_coordinates, get_text_field_selection_layout, is_mouse_or_touch_pad, visible_bounds,
};
use kompoz_coroutines::{coroutine_scope, snapshot_flow};
use kompoz_graphics::LayoutDirection;
use kompoz_graphics::{Density, Dp, Offset, Rect};
use kompoz_runtime::{DerivedState, MutableState, derived_state_of, mutable_state_of, without_read_observation};
use kompoz_text::{AnnotatedString, ResolvedTextDirection, TextLayoutResult, TextRange};
use kompoz_ui::clipboard::ClipEntry;
use kompoz_ui::input::{PointerEventPass, PointerInputScope};
use kompoz_ui::layout::LayoutCoordinates;
use kompoz_ui::platform::{Clipboard, HapticFeedback, HapticFeedbackType};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// The cursor's width (Compose's DefaultCursorThickness).
pub const DefaultCursorThickness: Dp = Dp(2.0);

/// What started a drag directly on the field (Compose's TextFieldSelectionState.InputType): none while there is
/// no such drag.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum InputType {
    None,
    Touch,
    Mouse,
}

/// When the text toolbar shows (Compose's TextToolbarState): never, over a collapsed selection (the cursor), or
/// over an expanded one.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextToolbarState {
    None,
    Cursor,
    Selection,
}

/// What shows and hides a field's text toolbar (Compose's TextToolbarHandler): BasicTextField's hands it to
/// `LocalTextToolbar`.
pub trait TextToolbarHandler {
    /// Shows the toolbar over `rect` (px in the root) with the actions the field can do now.
    fn show_text_toolbar(&self, selection_state: &Rc<TextFieldSelectionState>, rect: Rect);
    fn hide_text_toolbar(&self);
}

/// What the clipboard has to paste (Compose's ClipboardPasteState): updated before a menu shows.
struct ClipboardPasteState {
    clipboard: Clipboard,
    has_text: bool,
    has_clip: bool,
}

impl ClipboardPasteState {
    fn new(clipboard: Clipboard) -> ClipboardPasteState {
        ClipboardPasteState { clipboard, has_text: false, has_clip: false }
    }

    /// Reads what the clipboard holds again: any entry (a clip), one described as text.
    fn update(&mut self) {
        let entry = self.clipboard.get_clip_entry_now();
        self.has_text = entry.as_ref().is_some_and(ClipEntry::has_text);
        self.has_clip = entry.is_some();
    }
}

/// What gives a field's selection state the configuration of the content receiver around the field, if any
/// (Compose's `receiveContentConfiguration: (() -> ReceiveContentConfiguration?)?`, the decorator's).
pub type ReceiveContentConfigurationProvider = Rc<dyn Fn() -> Option<Rc<dyn ReceiveContentConfiguration>>>;

/// A field's selection behaviours and gestures (Compose's TextFieldSelectionState). Made by BasicTextField
/// (remembered per transformed state), updated as it recomposes, shared by its nodes.
pub struct TextFieldSelectionState {
    this: Weak<TextFieldSelectionState>,
    pub(crate) text_field_state: TransformedTextFieldState,
    pub(crate) text_layout_state: Rc<TextLayoutState>,
    density: Cell<Density>,
    enabled: Cell<bool>,
    read_only: Cell<bool>,
    is_password: Cell<bool>,
    clipboard: RefCell<Clipboard>,
    /// Whether the field and its window have the focus (set by the core and decorator nodes).
    is_window_and_text_field_focused: Cell<bool>,
    haptic_feed_back: RefCell<Option<HapticFeedback>>,
    text_toolbar_handler: RefCell<Option<Rc<dyn TextToolbarHandler>>>,
    is_in_touch_mode: MutableState<bool>,
    request_autofill_action: RefCell<Option<Rc<dyn Fn()>>>,
    /// Where the text layout was in the window when a handle's drag began (unspecified: no handle dragged).
    start_text_layout_position_in_window: MutableState<Offset>,
    raw_handle_drag_position: MutableState<Offset>,
    dragging_handle: MutableState<Option<Handle>>,
    direct_drag_gesture_initiator: MutableState<InputType>,
    show_cursor_handle: MutableState<bool>,
    text_toolbar_state: MutableState<TextToolbarState>,
    text_toolbar_shown: MutableState<bool>,
    previous_selection_layout: RefCell<Option<SelectionLayout>>,
    /// The previous drag offset before adjustments (Compose's -1: none).
    previous_raw_drag_offset: Cell<Option<usize>>,
    press_interaction: RefCell<Option<PressInteraction::Press>>,
    clipboard_paste_state: RefCell<ClipboardPasteState>,
    /// What gives the content receiver's configuration around the field (Compose's `receiveContentConfiguration`).
    receive_content_configuration: RefCell<Option<ReceiveContentConfigurationProvider>>,
    derived_visible_content_bounds: RefCell<Option<DerivedState<Option<Rect>>>>,
}

fn contains_inclusive(rect: Rect, point: Offset) -> bool {
    point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom
}

fn coerce_in(point: Offset, rect: Rect) -> Offset {
    crate::text::input::internal::text_layout_state::coerce_in(point, rect)
}

/// The closer of two rects to a point, by their distance from it: negative for the first (Compose's
/// `findClosestRect`).
fn find_closest_rect(offset: Offset, rect1: Rect, rect2: Rect) -> f32 {
    let distance = |r: Rect| {
        let dx = (r.left.max(offset.x.min(r.right)) - offset.x).abs();
        let dy = (r.top.max(offset.y.min(r.bottom)) - offset.y).abs();
        dx * dx + dy * dy
    };
    distance(rect1) - distance(rect2)
}

impl TextFieldSelectionState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        text_field_state: TransformedTextFieldState,
        text_layout_state: Rc<TextLayoutState>,
        density: Density,
        enabled: bool,
        read_only: bool,
        is_password: bool,
        clipboard: Clipboard,
    ) -> Rc<TextFieldSelectionState> {
        let state = Rc::new_cyclic(|this| TextFieldSelectionState {
            this: this.clone(),
            text_field_state,
            text_layout_state,
            density: Cell::new(density),
            enabled: Cell::new(enabled),
            read_only: Cell::new(read_only),
            is_password: Cell::new(is_password),
            clipboard: RefCell::new(clipboard.clone()),
            is_window_and_text_field_focused: Cell::new(false),
            haptic_feed_back: RefCell::new(None),
            text_toolbar_handler: RefCell::new(None),
            is_in_touch_mode: mutable_state_of(true),
            request_autofill_action: RefCell::new(None),
            start_text_layout_position_in_window: mutable_state_of(Offset::Unspecified),
            raw_handle_drag_position: mutable_state_of(Offset::Unspecified),
            dragging_handle: mutable_state_of(None),
            direct_drag_gesture_initiator: mutable_state_of(InputType::None),
            show_cursor_handle: mutable_state_of(false),
            text_toolbar_state: mutable_state_of(TextToolbarState::None),
            text_toolbar_shown: mutable_state_of(false),
            previous_selection_layout: RefCell::new(None),
            previous_raw_drag_offset: Cell::new(None),
            press_interaction: RefCell::new(None),
            clipboard_paste_state: RefCell::new(ClipboardPasteState::new(clipboard)),
            receive_content_configuration: RefCell::new(None),
            derived_visible_content_bounds: RefCell::new(None),
        });
        let weak = Rc::downgrade(&state);
        *state.derived_visible_content_bounds.borrow_mut() =
            Some(derived_state_of(move || weak.upgrade().and_then(|s| s.compute_visible_content_bounds())));
        state
    }

    fn rc(&self) -> Rc<TextFieldSelectionState> {
        self.this.upgrade().expect("a live selection state")
    }

    /// The transformed state it selects in.
    pub fn text_field_state(&self) -> &TransformedTextFieldState {
        &self.text_field_state
    }

    /// The layout state it hit tests with.
    pub fn text_layout_state(&self) -> &Rc<TextLayoutState> {
        &self.text_layout_state
    }

    pub fn enabled(&self) -> bool {
        self.enabled.get()
    }

    pub fn read_only(&self) -> bool {
        self.read_only.get()
    }

    /// Whether the user can change the text (Compose's `editable`).
    pub fn editable(&self) -> bool {
        self.enabled.get() && !self.read_only.get()
    }

    pub fn is_window_and_text_field_focused(&self) -> bool {
        self.is_window_and_text_field_focused.get()
    }

    pub fn set_is_window_and_text_field_focused(&self, focused: bool) {
        self.is_window_and_text_field_focused.set(focused);
    }

    /// Whether the user is touching rather than using a mouse (read as a state).
    pub fn is_in_touch_mode(&self) -> bool {
        self.is_in_touch_mode.get()
    }

    pub fn set_is_in_touch_mode(&self, touch: bool) {
        self.is_in_touch_mode.set(touch);
    }

    /// What the toolbar's autofill item does (the decorator's request).
    pub fn set_request_autofill_action(&self, action: Option<Rc<dyn Fn()>>) {
        *self.request_autofill_action.borrow_mut() = action;
    }

    /// Which handle is dragged (read as a state).
    pub fn dragging_handle(&self) -> Option<Handle> {
        self.dragging_handle.get()
    }

    /// What drags directly on the field now (read as a state).
    pub fn direct_drag_gesture_initiator(&self) -> InputType {
        self.direct_drag_gesture_initiator.get()
    }

    /// Whether the cursor's handle shows while the field is focused (read as a state).
    pub fn show_cursor_handle(&self) -> bool {
        self.show_cursor_handle.get()
    }

    pub fn set_show_cursor_handle(&self, show: bool) {
        self.show_cursor_handle.set(show);
    }

    /// Whether the text toolbar is shown (read as a state).
    pub fn text_toolbar_shown(&self) -> bool {
        self.text_toolbar_shown.get()
    }

    pub fn set_text_toolbar_shown(&self, shown: bool) {
        self.text_toolbar_shown.set(shown);
    }

    /// When the toolbar shows (read as a state).
    pub fn text_toolbar_state(&self) -> TextToolbarState {
        self.text_toolbar_state.get()
    }

    fn text_layout_coordinates(&self) -> Option<LayoutCoordinates> {
        self.text_layout_state.text_layout_node_coordinates().filter(|c| c.is_attached())
    }

    fn current_text_layout_position_in_window(&self) -> Offset {
        self.text_layout_coordinates().map_or(Offset::Unspecified, |c| c.position_in_window())
    }

    /// Where the dragged handle is, in the text layout node's coordinates (Compose's `handleDragPosition`): what
    /// the magnifier follows and the handles' drags hit test with, auto-scroll included. Read as a state.
    pub fn handle_drag_position(&self) -> Offset {
        let raw = self.raw_handle_drag_position.get();
        let start = self.start_text_layout_position_in_window.get();
        if raw.is_unspecified() {
            Offset::Unspecified
        } else if start.is_unspecified() {
            // A drag on the field (in the decoration box's coordinates).
            self.text_layout_state.from_decoration_to_text_layout(raw)
        } else {
            raw + (start - self.current_text_layout_position_in_window())
        }
    }

    /// The cursor handle's state (Compose's `getCursorHandleState`): shown for a collapsed selection in non-empty
    /// text while the cursor is in view (or its handle dragged), nothing dragging on the field; its position
    /// (the cursor's bottom center) only when `include_position`.
    pub fn get_cursor_handle_state(&self, include_position: bool) -> TextFieldHandleState {
        let text = self.text_field_state.visual_text();
        let show_cursor_handle = self.show_cursor_handle.get();
        let not_being_dragged = self.direct_drag_gesture_initiator.get() == InputType::None;
        let dragging_handle = self.dragging_handle.get();
        let visible = show_cursor_handle
            && not_being_dragged
            && text.selection().collapsed()
            && text.should_show_selection()
            && !text.is_empty()
            && (dragging_handle == Some(Handle::Cursor) || self.is_cursor_handle_in_visible_bounds());
        if !visible {
            return TextFieldHandleState::Hidden;
        }
        TextFieldHandleState {
            visible: true,
            position: if include_position { self.get_cursor_rect().bottom_center() } else { Offset::Unspecified },
            line_height: 0.0,
            direction: ResolvedTextDirection::Ltr,
            handles_crossed: false,
        }
    }

    /// Whether the cursor is in view (read unobserved: the auto-scroll settles it in layout).
    fn is_cursor_handle_in_visible_bounds(&self) -> bool {
        let position = without_read_observation(|| self.get_cursor_rect().bottom_center());
        self.text_layout_coordinates().is_some_and(|c| contains_inclusive(visible_bounds(&c), position))
    }

    /// Where the cursor is drawn, in the text layout node's coordinates (Compose's `getCursorRect`): nothing
    /// before a layout or for a selection.
    pub fn get_cursor_rect(&self) -> Rect {
        let Some(layout) = self.text_layout_state.layout_result() else { return Rect::Zero };
        self.calculate_cursor_rect(&layout, &self.text_field_state.visual_text())
    }

    /// Where the focus is, in the decoration box (Compose's `getFocusRect`): the cursor or the selection while
    /// focused, else unset (the whole field).
    pub fn get_focus_rect(&self) -> Option<Rect> {
        let layout = self.text_layout_state.layout_result()?;
        if !self.is_window_and_text_field_focused.get() {
            return None;
        }
        let value = self.text_field_state.visual_text();
        let rect = if value.selection().collapsed() {
            self.calculate_cursor_rect(&layout, &value)
        } else {
            calculate_selection_rect(&layout, &value)
        };
        Some(self.text_layout_state.from_text_layout_to_decoration(rect))
    }

    /// The cursor's rect: the layout's caret widened to the cursor's thickness, kept within the text layout (so it
    /// isn't clipped), on a pixel's middle for an odd width (Compose's `calculateCursorRect`).
    fn calculate_cursor_rect(&self, layout: &TextLayoutResult, visual_text: &TextFieldCharSequence) -> Rect {
        if !visual_text.selection().collapsed() {
            return Rect::Zero;
        }
        let cursor_rect = layout.get_cursor_rect(visual_text.selection().start);
        let cursor_width = self.density.get().to_px(DefaultCursorThickness).floor().max(1.0);
        let cursor_center_x = if layout.layout_input.layout_direction == LayoutDirection::Ltr {
            cursor_rect.left + cursor_width / 2.0
        } else {
            cursor_rect.right - cursor_width / 2.0
        };
        let coerced = cursor_center_x.min(layout.size.width as f32 - cursor_width / 2.0).max(cursor_width / 2.0);
        let coerced = if (cursor_width as i32) % 2 == 1 { coerced.floor() + 0.5 } else { coerced.round() };
        Rect::new(coerced - cursor_width / 2.0, cursor_rect.top, coerced + cursor_width / 2.0, cursor_rect.bottom)
    }

    /// Updates what changes as BasicTextField recomposes (Compose's `update`): a disabled field's toolbar hides.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &self,
        haptic_feed_back: HapticFeedback,
        clipboard: Clipboard,
        show_text_toolbar: Rc<dyn TextToolbarHandler>,
        density: Density,
        enabled: bool,
        read_only: bool,
        is_password: bool,
    ) {
        if !enabled {
            self.hide_text_toolbar();
        }
        let clipboard_changed = *self.clipboard.borrow() != clipboard;
        *self.haptic_feed_back.borrow_mut() = Some(haptic_feed_back);
        *self.clipboard.borrow_mut() = clipboard.clone();
        *self.text_toolbar_handler.borrow_mut() = Some(show_text_toolbar);
        self.density.set(density);
        self.enabled.set(enabled);
        self.read_only.set(read_only);
        self.is_password.set(is_password);
        if clipboard_changed {
            *self.clipboard_paste_state.borrow_mut() = ClipboardPasteState::new(clipboard);
        }
    }

    fn perform_haptic(&self, kind: HapticFeedbackType) {
        let haptics = self.haptic_feed_back.borrow().clone();
        if let Some(h) = haptics {
            h.perform_haptic_feedback(kind);
        }
    }

    // ---- Gestures ----

    /// The cursor handle's gestures (Compose's `cursorHandleGestures`): its drags move the cursor, a tap toggles
    /// the toolbar.
    pub async fn cursor_handle_gestures(self: Rc<Self>, scope: PointerInputScope) {
        coroutine_scope(move |cs| async move {
            let (a, b, c) = (self.clone(), self.clone(), self.clone());
            let (s1, s2, s3) = (scope.clone(), scope.clone(), scope);
            cs.launch(async move { a.detect_touch_mode(s1).await });
            cs.launch(async move { b.detect_cursor_handle_drag_gestures(s2).await });
            cs.launch(async move {
                let weak = Rc::downgrade(&c);
                drop(c);
                s3.detect_tap_gestures()
                    .on_tap(move |_| {
                        if let Some(s) = weak.upgrade() {
                            let next = if s.text_toolbar_state.peek() == TextToolbarState::Cursor {
                                TextToolbarState::None
                            } else {
                                TextToolbarState::Cursor
                            };
                            s.text_toolbar_state.set(next);
                        }
                    })
                    .await
            });
        })
        .await;
    }

    /// A selection handle's gestures (Compose's `selectionHandleGestures`): a press marks it dragged (for the
    /// magnifier), its drags move its end of the selection.
    pub async fn selection_handle_gestures(self: Rc<Self>, scope: PointerInputScope, is_start_handle: bool) {
        coroutine_scope(move |cs| async move {
            let (a, b, c) = (self.clone(), self.clone(), self.clone());
            let (s1, s2, s3) = (scope.clone(), scope.clone(), scope);
            cs.launch(async move { a.detect_touch_mode(s1).await });
            cs.launch(async move {
                struct Clear(Weak<TextFieldSelectionState>);
                impl Drop for Clear {
                    fn drop(&mut self) {
                        if let Some(s) = self.0.upgrade() {
                            s.clear_handle_dragging();
                        }
                    }
                }
                let _clear = Clear(Rc::downgrade(&b));
                let (down, up) = (Rc::downgrade(&b), Rc::downgrade(&b));
                drop(b);
                detect_press_down_gesture(
                    s2,
                    Rc::new(move |_| {
                        if let Some(s) = down.upgrade() {
                            s.mark_start_content_visible_offset();
                            let handle = if is_start_handle { Handle::SelectionStart } else { Handle::SelectionEnd };
                            let position = crate::text::selection::get_adjusted_coordinates(
                                s.get_handle_position(is_start_handle),
                            );
                            s.update_handle_dragging(handle, position);
                        }
                    }),
                    Some(Rc::new(move || {
                        if let Some(s) = up.upgrade() {
                            s.clear_handle_dragging();
                        }
                    })),
                )
                .await;
            });
            cs.launch(async move { c.detect_selection_handle_drag_gestures(s3, is_start_handle).await });
        })
        .await;
    }

    /// Starts the observers of what shows and hides the cursor handle and the toolbar (Compose's
    /// `startToolbarAndHandlesVisibilityObserver`): until cancelled, after which the handle and toolbar hide.
    pub async fn start_toolbar_and_handles_visibility_observer(self: Rc<Self>) {
        struct Finally(Weak<TextFieldSelectionState>);
        impl Drop for Finally {
            fn drop(&mut self) {
                if let Some(s) = self.0.upgrade() {
                    s.show_cursor_handle.set(false);
                    if s.text_toolbar_state.peek() != TextToolbarState::None {
                        s.hide_text_toolbar();
                    }
                }
            }
        }
        let _finally = Finally(Rc::downgrade(&self));
        coroutine_scope(move |cs| async move {
            let (a, b) = (self.clone(), self);
            cs.launch(async move { a.observe_text_changes().await });
            cs.launch(async move { b.observe_text_toolbar_visibility().await });
        })
        .await;
    }

    /// Sets when the toolbar shows (Compose's `updateTextToolbarState`).
    pub fn update_text_toolbar_state(&self, state: TextToolbarState) {
        self.text_toolbar_state.set(state);
    }

    /// Hides its toolbar and forgets its haptics (Compose's `dispose`).
    pub fn dispose(&self) {
        self.hide_text_toolbar();
        *self.haptic_feed_back.borrow_mut() = None;
    }

    /// Whether the pointer of each event is a finger (Compose's `detectTouchMode`): read in the Initial pass.
    pub async fn detect_touch_mode(self: Rc<Self>, scope: PointerInputScope) {
        let weak = Rc::downgrade(&self);
        drop(self);
        scope
            .await_pointer_event_scope(move |s| async move {
                loop {
                    let event = s.await_pointer_event_with(PointerEventPass::Initial).await;
                    if let Some(state) = weak.upgrade() {
                        state.is_in_touch_mode.set(!is_mouse_or_touch_pad(&event));
                    }
                }
            })
            .await
    }

    /// Taps on the field (Compose's `detectTextFieldTapGestures`, the default one): a tap focuses it and, when
    /// focused, shows the keyboard (editable), hides the toolbar and places the cursor nearest; presses go to
    /// `interaction_source`.
    pub async fn detect_text_field_tap_gestures(
        self: Rc<Self>,
        scope: PointerInputScope,
        interaction_source: Option<MutableInteractionSource>,
        request_focus: Rc<dyn Fn()>,
        show_keyboard: Rc<dyn Fn()>,
    ) {
        let weak = Rc::downgrade(&self);
        drop(self);
        let tap_state = weak.clone();
        let on_tap: Rc<dyn Fn(Offset)> = Rc::new(move |offset: Offset| {
            let Some(s) = tap_state.upgrade() else { return };
            request_focus();
            if s.enabled.get() && s.is_window_and_text_field_focused.get() {
                if !s.read_only.get() {
                    show_keyboard();
                    if !s.text_field_state.visual_text().is_empty() {
                        s.show_cursor_handle.set(true);
                    }
                }
                s.update_text_toolbar_state(TextToolbarState::None);
                let coerced = s.text_layout_state.coerced_in_visible_bounds_of_input_text(offset);
                s.place_cursor_at_nearest_offset(s.text_layout_state.from_decoration_to_text_layout(coerced));
            }
        });
        let on_press: Option<PressCallback> = interaction_source.map(|source| {
            let press_state = weak.clone();
            Rc::new(move |press: crate::gestures::PressGestureScope, offset: Offset| {
                let (source, state) = (source.clone(), press_state.clone());
                Box::pin(async move {
                    // Old interactions not ended properly are cancelled.
                    let old = state.upgrade().and_then(|s| s.press_interaction.borrow_mut().take());
                    if let Some(old) = old {
                        source.emit(PressInteraction::Cancel::new(old)).await;
                    }
                    let interaction = PressInteraction::Press::new(offset);
                    source.emit(interaction).await;
                    if let Some(s) = state.upgrade() {
                        *s.press_interaction.borrow_mut() = Some(interaction);
                    }
                    let success = press.try_await_release().await;
                    let pressed = state.upgrade().and_then(|s| s.press_interaction.borrow_mut().take());
                    if let Some(pressed) = pressed {
                        if success {
                            source.emit(PressInteraction::Release::new(pressed)).await;
                        } else {
                            source.emit(PressInteraction::Cancel::new(pressed)).await;
                        }
                    }
                }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>
            }) as PressCallback
        });
        detect_tap_and_press(scope, on_press, Some(on_tap)).await;
    }

    /// Places the cursor at the valid offset nearest `offset` (in the text layout node's coordinates), out of a
    /// transformation's inserted or replaced text to its nearer edge (Compose's `placeCursorAtNearestOffset`):
    /// whether it moved.
    pub fn place_cursor_at_nearest_offset(&self, offset: Offset) -> bool {
        let Some(layout) = self.text_layout_state.layout_result() else { return false };
        let index = layout.get_offset_for_position(offset);
        let mut new_affinity: Option<SelectionWedgeAffinity> = None;
        let (kind, untransformed, retransformed) = self.text_field_state.get_index_transformation_type(index);
        let untransformed_cursor = match kind {
            IndexTransformationType::Untransformed | IndexTransformationType::Deletion => untransformed.start,
            IndexTransformationType::Insertion => {
                let start_rect = layout.get_cursor_rect(retransformed.start);
                let end_rect = layout.get_cursor_rect(retransformed.end);
                new_affinity = Some(if find_closest_rect(offset, start_rect, end_rect) < 0.0 {
                    SelectionWedgeAffinity::new(WedgeAffinity::Start)
                } else {
                    SelectionWedgeAffinity::new(WedgeAffinity::End)
                });
                untransformed.start
            }
            IndexTransformationType::Replacement => {
                let start_rect = layout.get_cursor_rect(retransformed.start);
                let end_rect = layout.get_cursor_rect(retransformed.end);
                if find_closest_rect(offset, start_rect, end_rect) < 0.0 {
                    untransformed.start
                } else {
                    untransformed.end
                }
            }
        };
        let range = TextRange::at(untransformed_cursor);
        if range == self.text_field_state.untransformed_text().selection()
            && new_affinity.is_none_or(|a| a == self.text_field_state.selection_wedge_affinity())
        {
            return false;
        }
        self.text_field_state.select_untransformed_chars_in(range);
        if let Some(a) = new_affinity {
            self.text_field_state.set_selection_wedge_affinity(a);
        }
        true
    }

    async fn detect_cursor_handle_drag_gestures(self: Rc<Self>, scope: PointerInputScope) {
        struct DragState {
            start: Cell<Offset>,
            delta: Cell<Offset>,
        }
        let drag = Rc::new(DragState { start: Cell::new(Offset::Unspecified), delta: Cell::new(Offset::Unspecified) });
        let weak = Rc::downgrade(&self);
        drop(self);
        let on_drag_stop = {
            let (drag, weak) = (drag.clone(), weak.clone());
            Rc::new(move || {
                if drag.start.get().is_specified() {
                    drag.start.set(Offset::Unspecified);
                    drag.delta.set(Offset::Unspecified);
                    if let Some(s) = weak.upgrade() {
                        s.clear_handle_dragging();
                    }
                }
            })
        };
        // detectDragGestures doesn't call onDragCancel when its composable goes (Compose's finally).
        struct Finally(Rc<dyn Fn()>);
        impl Drop for Finally {
            fn drop(&mut self) {
                (self.0)();
            }
        }
        let _finally = Finally(on_drag_stop.clone());
        let (start_drag, start_weak) = (drag.clone(), weak.clone());
        let (end, cancel) = (on_drag_stop.clone(), on_drag_stop);
        scope
            .detect_drag_gestures(move |change, drag_amount| {
                let Some(s) = weak.upgrade() else { return };
                drag.delta.set(drag.delta.get() + drag_amount);
                s.update_handle_dragging(Handle::Cursor, drag.start.get() + drag.delta.get());
                if s.place_cursor_at_nearest_offset(s.handle_drag_position()) {
                    change.consume();
                    s.perform_haptic(HapticFeedbackType::TextHandleMove);
                }
            })
            .on_drag_start(move |_| {
                let Some(s) = start_weak.upgrade() else { return };
                let start = crate::text::selection::get_adjusted_coordinates(s.get_cursor_rect().bottom_center());
                start_drag.start.set(start);
                start_drag.delta.set(Offset::Zero);
                s.is_in_touch_mode.set(true);
                s.mark_start_content_visible_offset();
                s.update_handle_dragging(Handle::Cursor, start);
            })
            .on_drag_end(move || end())
            .on_drag_cancel(move || cancel())
            .await;
    }

    /// The field's selection gestures (Compose's `textFieldSelectionGestures`, the default one): a mouse's and a
    /// finger's, through the observers below.
    pub async fn text_field_selection_gestures(self: Rc<Self>, scope: PointerInputScope, request_focus: Rc<dyn Fn()>) {
        let mouse: Rc<dyn MouseSelectionObserver> = Rc::new(TextFieldMouseSelectionObserver {
            state: Rc::downgrade(&self),
            request_focus: request_focus.clone(),
            drag_begin_offset_in_text: Cell::new(None),
            drag_begin_position: Cell::new(Offset::Unspecified),
            is_double_or_triple_click_only: Cell::new(true),
        });
        let touch: Rc<dyn TextDragObserver> = Rc::new(TextFieldTextDragObserver {
            state: Rc::downgrade(&self),
            request_focus,
            drag_begin_offset_in_text: Cell::new(None),
            drag_begin_position: Cell::new(Offset::Unspecified),
            drag_total_distance: Cell::new(Offset::Zero),
            acting_handle: Cell::new(Handle::SelectionEnd),
            is_long_press_selection_only: Cell::new(true),
            selection_adjustment_mode: Cell::new(SelectionAdjustment::None),
        });
        drop(self);
        await_selection_gestures(scope, mouse, Some(touch)).await;
    }

    /// Suggests a better selection after a long press or double click (Compose's `maybeSuggestSelectionRange`):
    /// smart selection is Android's (PlatformSelectionBehaviors; none on the desktop).
    fn maybe_suggest_selection_range(&self) {}

    async fn detect_selection_handle_drag_gestures(self: Rc<Self>, scope: PointerInputScope, is_start_handle: bool) {
        struct DragState {
            begin: Cell<Offset>,
            total: Cell<Offset>,
        }
        let drag = Rc::new(DragState { begin: Cell::new(Offset::Unspecified), total: Cell::new(Offset::Zero) });
        let handle = if is_start_handle { Handle::SelectionStart } else { Handle::SelectionEnd };
        let weak = Rc::downgrade(&self);
        drop(self);
        let on_drag_stop = {
            let (drag, weak) = (drag.clone(), weak.clone());
            Rc::new(move || {
                if drag.begin.get().is_specified() {
                    if let Some(s) = weak.upgrade() {
                        s.clear_handle_dragging();
                        s.previous_raw_drag_offset.set(None);
                    }
                    drag.begin.set(Offset::Unspecified);
                    drag.total.set(Offset::Zero);
                }
            })
        };
        struct Finally(Weak<TextFieldSelectionState>, Handle, Rc<dyn Fn()>);
        impl Drop for Finally {
            fn drop(&mut self) {
                if self.0.upgrade().is_some_and(|s| s.dragging_handle.peek() == Some(self.1)) {
                    (self.2)();
                }
            }
        }
        let _finally = Finally(weak.clone(), handle, on_drag_stop.clone());
        let (start_drag, start_weak) = (drag.clone(), weak.clone());
        let (end, cancel) = (on_drag_stop.clone(), on_drag_stop);
        scope
            .detect_drag_gestures(move |_, delta| {
                let Some(s) = weak.upgrade() else { return };
                drag.total.set(drag.total.get() + delta);
                let Some(layout) = s.text_layout_state.layout_result() else { return };
                s.update_handle_dragging(handle, drag.begin.get() + drag.total.get());
                let selection = s.text_field_state.visual_text().selection();
                let at = layout.get_offset_for_position(s.handle_drag_position());
                let (start_offset, end_offset) =
                    if is_start_handle { (at, selection.end) } else { (selection.start, at) };
                let new_selection = s.update_selection(
                    &s.text_field_state.visual_text(),
                    start_offset,
                    end_offset,
                    is_start_handle,
                    SelectionAdjustment::CharacterWithWordAccelerate,
                    false,
                    false,
                    Some(HapticFeedbackType::TextHandleMove),
                );
                // A handle's drag never collapses the selection (it can reverse).
                if selection.collapsed() || !new_selection.collapsed() {
                    s.text_field_state.select_chars_in(new_selection);
                }
            })
            .on_drag_start(move |_| {
                let Some(s) = start_weak.upgrade() else { return };
                let begin = crate::text::selection::get_adjusted_coordinates(s.get_handle_position(is_start_handle));
                start_drag.begin.set(begin);
                s.update_handle_dragging(handle, begin);
                start_drag.total.set(Offset::Zero);
                s.previous_raw_drag_offset.set(None);
            })
            .on_drag_end(move || end())
            .on_drag_cancel(move || cancel())
            .await;
    }

    /// Hides the cursor handle and the toolbar whenever the text changes (Compose's `observeTextChanges`).
    async fn observe_text_changes(self: Rc<Self>) {
        let weak = Rc::downgrade(&self);
        let state = self.text_field_state.clone();
        drop(self);
        let mut first = true;
        snapshot_flow(move || state.visual_text().text().to_string())
            .collect(move |_| {
                if std::mem::take(&mut first) {
                    return;
                }
                if let Some(s) = weak.upgrade() {
                    s.show_cursor_handle.set(false);
                    s.update_text_toolbar_state(TextToolbarState::None);
                }
            })
            .await;
    }

    /// Shows the toolbar over the visible content while it should show, hides it otherwise (Compose's
    /// `observeTextToolbarVisibility`, the old context menu's: shown again as the rect moves).
    async fn observe_text_toolbar_visibility(self: Rc<Self>) {
        let weak = Rc::downgrade(&self);
        let derived = self.derived_visible_content_bounds.borrow().clone().expect("made with the state");
        drop(self);
        snapshot_flow(move || derived.get())
            .collect(move |rect| {
                let Some(s) = weak.upgrade() else { return };
                match rect {
                    Some(rect) => s.show_text_toolbar(rect),
                    None => s.hide_text_toolbar(),
                }
            })
            .await;
    }

    /// The visible part of what the toolbar is over, in the root, while it should show (Compose's
    /// `derivedVisibleContentBounds`, read as a derived state).
    pub fn derived_visible_content_bounds(&self) -> Option<Rect> {
        self.derived_visible_content_bounds.borrow().as_ref().and_then(|d| d.get())
    }

    fn compute_visible_content_bounds(&self) -> Option<Rect> {
        let collapsed = self.text_field_state.visual_text().selection().collapsed();
        let toolbar_state = self.text_toolbar_state.get();
        let state_visible = collapsed && toolbar_state == TextToolbarState::Cursor
            || !collapsed && toolbar_state == TextToolbarState::Selection;
        let visible = state_visible && self.dragging_handle.get().is_none() && self.is_in_touch_mode.get();
        if !visible {
            return None;
        }
        let coordinates = self.text_layout_coordinates()?;
        let bounds = visible_bounds(&coordinates);
        let top_left = coordinates.local_to_root(bounds.top_left());
        let visible_in_root = Rect::from_offset_size(top_left, bounds.size());
        let content = self.get_content_rect()?;
        if !content.overlaps(visible_in_root) {
            return None;
        }
        Some(content.intersect(visible_in_root))
    }

    /// The selected region in the root (Compose's `getContentRect`): the cursor for a collapsed selection; else
    /// from the leftmost handle to the rightmost, the first selected line's top to the last one's bottom.
    fn get_content_rect(&self) -> Option<Rect> {
        let coordinates = self.text_layout_coordinates()?;
        let text = self.text_field_state.visual_text();
        if text.selection().collapsed() {
            let cursor = self.get_cursor_rect();
            let top_left = coordinates.local_to_root(cursor.top_left());
            return Some(Rect::from_offset_size(top_left, cursor.size()));
        }
        let start = coordinates.local_to_root(self.get_handle_position(true));
        let end = coordinates.local_to_root(self.get_handle_position(false));
        let Some(layout) = self.text_layout_state.layout_result() else { return Some(Rect::Zero) };
        let start_top =
            coordinates.local_to_root(Offset::new(0.0, layout.get_cursor_rect(text.selection().start).top)).y;
        let end_top = coordinates.local_to_root(Offset::new(0.0, layout.get_cursor_rect(text.selection().end).top)).y;
        let left = start.x.min(end.x);
        let right = start.x.max(end.x);
        Some(Rect::new(
            left,
            start_top.min(end_top),
            if left == right { right + 1.0 } else { right },
            start.y.max(end.y),
        ))
    }

    /// A selection handle's state (Compose's `getSelectionHandleState`): shown for an expanded selection while
    /// its end is in view (or it is dragged) and nothing drags on the field; its position kept in view, only
    /// when `include_position`.
    pub fn get_selection_handle_state(&self, is_start_handle: bool, include_position: bool) -> TextFieldHandleState {
        let handle = if is_start_handle { Handle::SelectionStart } else { Handle::SelectionEnd };
        let Some(layout) = self.text_layout_state.layout_result() else { return TextFieldHandleState::Hidden };
        let visual = self.text_field_state.visual_text();
        let selection = visual.selection();
        if selection.collapsed() {
            return TextFieldHandleState::Hidden;
        }
        let position = self.get_handle_position(is_start_handle);
        let visible = self.direct_drag_gesture_initiator.get() == InputType::None
            && (self.dragging_handle.get() == Some(handle)
                || self.text_layout_coordinates().is_some_and(|c| contains_inclusive(visible_bounds(&c), position)));
        if !visible || !visual.should_show_selection() {
            return TextFieldHandleState::Hidden;
        }
        let direction_offset =
            if is_start_handle { selection.start } else { previous_char_boundary(visual.text(), selection.end) };
        let direction = layout.get_bidi_run_direction(direction_offset);
        let coerced = if include_position {
            self.text_layout_coordinates().map_or(position, |c| coerce_in(position, visible_bounds(&c)))
        } else {
            Offset::Unspecified
        };
        let handle_offset = if is_start_handle { selection.start } else { selection.end };
        TextFieldHandleState {
            visible: true,
            position: coerced,
            line_height: crate::text::selection::get_line_height(&layout, handle_offset),
            direction,
            handles_crossed: selection.reversed(),
        }
    }

    fn get_handle_position(&self, is_start_handle: bool) -> Offset {
        let Some(layout) = self.text_layout_state.layout_result() else { return Offset::Zero };
        let selection = self.text_field_state.visual_text().selection();
        let offset = if is_start_handle { selection.start } else { selection.end };
        get_selection_handle_coordinates(&layout, offset, is_start_handle, selection.reversed())
    }

    /// Marks `handle` dragged at `position` (Compose's `updateHandleDragging`, the magnifier's source).
    pub fn update_handle_dragging(&self, handle: Handle, position: Offset) {
        self.dragging_handle.set(Some(handle));
        self.raw_handle_drag_position.set(position);
    }

    /// Notes where the text layout is as a handle's drag starts, for auto-scroll (Compose's
    /// `markStartContentVisibleOffset`).
    fn mark_start_content_visible_offset(&self) {
        self.start_text_layout_position_in_window.set(self.current_text_layout_position_in_window());
    }

    /// No handle is dragged any more (Compose's `clearHandleDragging`).
    pub fn clear_handle_dragging(&self) {
        self.dragging_handle.set(None);
        self.raw_handle_drag_position.set(Offset::Unspecified);
        self.start_text_layout_position_in_window.set(Offset::Unspecified);
    }

    // ---- Clipboard and menu actions ----

    /// Whether cut can be offered (Compose's `canShowCutMenuItem`): something selected, editable, no password.
    pub fn can_show_cut_menu_item(&self) -> bool {
        self.is_cut_allowed()
    }

    /// Compose's `isCutAllowed`.
    pub fn is_cut_allowed(&self) -> bool {
        !self.text_field_state.visual_text().selection().collapsed() && self.editable() && !self.is_password.get()
    }

    /// Cuts the selected text to the clipboard (Compose's `cut`).
    pub fn cut(&self) {
        if let Some(value) = self.cut_with_result() {
            self.clipboard.borrow().set_clip_entry_now(Some(ClipEntry::with_annotated_string(value)));
        }
    }

    /// Deletes the selected text, giving it (Compose's `cutWithResult`): none when cutting isn't allowed.
    pub fn cut_with_result(&self) -> Option<AnnotatedString> {
        if !self.is_cut_allowed() {
            return None;
        }
        let selected = self.text_field_state.visual_text().get_selected_text().to_string();
        self.text_field_state.delete_selected_text(false);
        Some(AnnotatedString::new(selected))
    }

    /// Whether copy can be offered (Compose's `canShowCopyMenuItem`): something selected, no password.
    pub fn can_show_copy_menu_item(&self) -> bool {
        self.is_copy_allowed()
    }

    /// Compose's `isCopyAllowed`.
    pub fn is_copy_allowed(&self) -> bool {
        !self.text_field_state.visual_text().selection().collapsed() && !self.is_password.get()
    }

    /// Copies the selected text to the clipboard, collapsing the selection to its end when `cancel_selection`
    /// (Compose's `copy`).
    pub fn copy(&self, cancel_selection: bool) {
        if let Some(value) = self.copy_with_result(cancel_selection) {
            self.clipboard.borrow().set_clip_entry_now(Some(ClipEntry::with_annotated_string(value)));
        }
    }

    /// The selected text to copy (Compose's `copyWithResult`): none when copying isn't allowed.
    pub fn copy_with_result(&self, cancel_selection: bool) -> Option<AnnotatedString> {
        if !self.is_copy_allowed() {
            return None;
        }
        let selected = self.text_field_state.visual_text().get_selected_text().to_string();
        if cancel_selection {
            self.text_field_state.collapse_selection_to_max();
        }
        Some(AnnotatedString::new(selected))
    }

    /// Reads what the clipboard has to paste again (Compose's `updateClipboardEntry`).
    pub fn update_clipboard_entry(&self) {
        self.clipboard_paste_state.borrow_mut().update();
    }

    /// Whether paste can be offered (Compose's `canShowPasteMenuItem`): editable, and text in the clipboard as it
    /// was last read ([`update_clipboard_entry`](Self::update_clipboard_entry)) — or, under a content receiver,
    /// anything (its listener takes what it takes).
    pub fn can_show_paste_menu_item(&self) -> bool {
        if !self.is_paste_allowed() {
            return false;
        }
        let paste_state = self.clipboard_paste_state.borrow();
        if paste_state.has_text {
            return true;
        }
        paste_state.has_clip && self.receive_content_configuration().is_some()
    }

    /// The configuration of the content receiver around the field, if any (the decorator's provider asked).
    fn receive_content_configuration(&self) -> Option<Rc<dyn ReceiveContentConfiguration>> {
        let provider = self.receive_content_configuration.borrow().clone();
        provider.and_then(|p| p())
    }

    /// What gives the content receiver's configuration (Compose's settable `receiveContentConfiguration`: the
    /// decorator's while it is attached).
    pub fn set_receive_content_configuration(&self, provider: Option<ReceiveContentConfigurationProvider>) {
        *self.receive_content_configuration.borrow_mut() = provider;
    }

    /// Compose's `isPasteAllowed`.
    pub fn is_paste_allowed(&self) -> bool {
        self.editable()
    }

    /// Pastes what the clipboard holds, a step of its own in the undo history (Compose's `paste`): under a content
    /// receiver it goes to the receivers first (from the clipboard), what they leave of its plain text in place of
    /// the selection; else its text does.
    pub fn paste(&self, is_from_hardware_source: bool) {
        let Some(receive_content_configuration) = self.receive_content_configuration() else {
            return self.paste_as_plain_text(is_from_hardware_source);
        };
        let clip_entry = self.clipboard.borrow().get_clip_entry_now();
        let Some(clip_entry) = clip_entry else { return self.paste_as_plain_text(is_from_hardware_source) };
        let clip_metadata = clip_entry.clip_metadata().clone();
        let remaining = receive_content_configuration.receive_content_listener().on_receive(TransferableContent::new(
            clip_entry,
            clip_metadata,
            TransferableContentSource::Clipboard,
            None,
        ));
        if let Some(clipboard_text) = remaining.and_then(|r| r.clip_entry().read_plain_text()) {
            self.text_field_state.replace_selected_text(
                &clipboard_text,
                false,
                TextFieldEditUndoBehavior::NeverMerge,
                true,
                is_from_hardware_source,
            );
        }
    }

    /// Pastes the clipboard's text over the selection (Compose's `pasteAsPlainText`: the first item's).
    fn paste_as_plain_text(&self, is_from_hardware_source: bool) {
        let clipboard_text = self.clipboard.borrow().get_clip_entry_now().and_then(|e| e.read_text());
        let Some(clipboard_text) = clipboard_text else { return };
        self.text_field_state.replace_selected_text(
            &clipboard_text,
            false,
            TextFieldEditUndoBehavior::NeverMerge,
            true,
            is_from_hardware_source,
        );
    }

    /// Pastes `value` from a platform's clipboard event (Compose's `onPasteEvent`).
    pub fn on_paste_event(&self, value: &AnnotatedString, is_from_hardware_source: bool) {
        if !self.is_paste_allowed() {
            return;
        }
        self.text_field_state.replace_selected_text(
            value.text(),
            false,
            TextFieldEditUndoBehavior::NeverMerge,
            true,
            is_from_hardware_source,
        );
    }

    /// Whether select all can be offered (Compose's `canShowSelectAllMenuItem`): not all selected yet.
    pub fn can_show_select_all_menu_item(&self) -> bool {
        let visual = self.text_field_state.visual_text();
        visual.selection().length() != visual.len()
    }

    /// Selects all the text (Compose's `selectAll`).
    pub fn select_all(&self) {
        self.text_field_state.select_all();
    }

    /// Whether autofill can be offered (Compose's `canShowAutofillMenuItem`): editable, nothing selected.
    pub fn can_show_autofill_menu_item(&self) -> bool {
        self.editable() && self.text_field_state.visual_text().selection().collapsed()
    }

    /// Asks the platform to autofill the field (Compose's `autofill`).
    pub fn autofill(&self) {
        let action = self.request_autofill_action.borrow().clone();
        if let Some(action) = action {
            action();
        }
    }

    fn show_text_toolbar(&self, content_rect: Rect) {
        let handler = self.text_toolbar_handler.borrow().clone();
        if let Some(h) = handler {
            h.show_text_toolbar(&self.rc(), content_rect);
        }
    }

    /// Collapses the selection to its end and hides the cursor handle and the toolbar (Compose's `deselect`).
    pub fn deselect(&self) {
        if !self.text_field_state.visual_text().selection().collapsed() {
            self.text_field_state.collapse_selection_to_end();
        }
        self.show_cursor_handle.set(false);
        self.update_text_toolbar_state(TextToolbarState::None);
    }

    fn hide_text_toolbar(&self) {
        let handler = self.text_toolbar_handler.borrow().clone();
        if let Some(h) = handler {
            h.hide_text_toolbar();
        }
    }

    /// Selects from `start_offset` to `end_offset`, snapped by `adjustment` (Compose's `updateSelection`): the
    /// selection before is context unless this starts one (or it is collapsed, unless
    /// `allow_previous_selection_collapsed`); a meaningful change is felt (`haptic_feedback_type`).
    #[allow(clippy::too_many_arguments)]
    pub fn update_selection(
        &self,
        text_field_char_sequence: &TextFieldCharSequence,
        start_offset: usize,
        end_offset: usize,
        is_start_handle: bool,
        adjustment: SelectionAdjustment,
        allow_previous_selection_collapsed: bool,
        is_start_of_selection: bool,
        haptic_feedback_type: Option<HapticFeedbackType>,
    ) -> TextRange {
        let previous = text_field_char_sequence.selection();
        let previous_selection = (!is_start_of_selection
            && (allow_previous_selection_collapsed || !previous.collapsed()))
        .then_some(previous);
        let new_selection =
            self.get_text_field_selection(start_offset, end_offset, previous_selection, is_start_handle, adjustment);
        if let Some(kind) = haptic_feedback_type
            && (new_selection.min() != previous.min() || new_selection.max() != previous.max())
        {
            self.perform_haptic(kind);
        }
        new_selection
    }

    fn get_text_field_selection(
        &self,
        raw_start_offset: usize,
        raw_end_offset: usize,
        previous_selection: Option<TextRange>,
        is_start_handle: bool,
        adjustment: SelectionAdjustment,
    ) -> TextRange {
        let Some(layout) = self.text_layout_state.layout_result() else { return TextRange::Zero };
        // Without a selection before, a collapsed one is allowed: Character adjusts nothing.
        if previous_selection.is_none() && adjustment == SelectionAdjustment::Character {
            return TextRange::new(raw_start_offset, raw_end_offset);
        }
        let selection_layout = get_text_field_selection_layout(
            &layout,
            raw_start_offset,
            raw_end_offset,
            self.previous_raw_drag_offset.get(),
            previous_selection.unwrap_or(TextRange::Zero),
            previous_selection.is_none(),
            is_start_handle,
        );
        if let Some(previous) = previous_selection
            && !selection_layout.should_recompute_selection(self.previous_selection_layout.borrow().as_ref())
        {
            return previous;
        }
        let result = adjustment.adjust(&selection_layout).to_text_range();
        *self.previous_selection_layout.borrow_mut() = Some(selection_layout);
        self.previous_raw_drag_offset.set(Some(if is_start_handle { raw_start_offset } else { raw_end_offset }));
        result
    }

    // ---- The desktop text context menu's text manager (Compose desktop's `TextFieldSelectionState.textManager`)

    /// Selects the word at `offset` (in the decoration box) unless it is selected already (Compose desktop's
    /// `selectWordAtPositionIfNotAlreadySelected`: macOS's right click).
    pub fn select_word_at_position_if_not_already_selected(&self, offset: Offset) {
        if !self.text_layout_state.is_position_on_text(offset) {
            return;
        }
        let Some(index) = self.text_layout_state.get_offset_for_position(offset, false) else { return };
        let visual = self.text_field_state.visual_text();
        if !visual.selection().contains_offset(index) {
            let selection =
                self.update_selection(&visual, index, index, false, SelectionAdjustment::Word, false, false, None);
            self.text_field_state.select_chars_in(selection);
        }
    }
}

/// The character boundary before `offset` (Compose's `selection.end - 1`, a character back).
fn previous_char_boundary(text: &str, offset: usize) -> usize {
    if offset == 0 {
        return 0;
    }
    let mut i = offset.min(text.len()) - 1;
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// The minimum rect around the selection, in the text layout (Compose's `calculateSelectionRect`).
fn calculate_selection_rect(layout: &TextLayoutResult, visual_text: &TextFieldCharSequence) -> Rect {
    let selection = visual_text.selection();
    if selection.collapsed() {
        return Rect::Zero;
    }
    let line_start = layout.get_line_for_offset(selection.start);
    let line_end = layout.get_line_for_offset(selection.end);
    if line_start == line_end {
        let start = layout.get_horizontal_position(selection.start, true);
        let end = layout.get_horizontal_position(selection.end, true);
        Rect::new(start.min(end), layout.get_line_top(line_start), start.max(end), layout.get_line_bottom(line_end))
    } else {
        layout.get_path_for_range(selection.min(), selection.max()).get_bounds()
    }
}

/// A mouse's selection in the field (Compose's TextFieldMouseSelectionObserver).
struct TextFieldMouseSelectionObserver {
    state: Weak<TextFieldSelectionState>,
    request_focus: Rc<dyn Fn()>,
    drag_begin_offset_in_text: Cell<Option<usize>>,
    drag_begin_position: Cell<Offset>,
    is_double_or_triple_click_only: Cell<bool>,
}

impl TextFieldMouseSelectionObserver {
    fn usable(&self) -> Option<(Rc<TextFieldSelectionState>, TextLayoutResult)> {
        let s = self.state.upgrade()?;
        let layout = s.text_layout_state.layout_result()?;
        if !s.enabled.get() || s.text_field_state.visual_text().is_empty() {
            return None;
        }
        Some((s, layout))
    }

    fn update_selection(
        &self,
        s: &TextFieldSelectionState,
        drag_position: Offset,
        adjustment: SelectionAdjustment,
        layout: &TextLayoutResult,
        is_start_of_selection: bool,
    ) -> TextRange {
        let text_length = layout.layout_input.text.text().len();
        let start_offset = match self.drag_begin_offset_in_text.get() {
            Some(o) if o <= text_length => o,
            _ => s.text_layout_state.get_offset_for_position(self.drag_begin_position.get(), false).unwrap_or(0),
        };
        let end_offset = s.text_layout_state.get_offset_for_position(drag_position, false).unwrap_or(0);
        let mut new_selection = s.update_selection(
            &s.text_field_state.visual_text(),
            start_offset,
            end_offset,
            false,
            adjustment,
            false,
            is_start_of_selection,
            None,
        );
        // A drag from the end padding starts its selection where it first expands.
        if self.drag_begin_offset_in_text.get().is_none() && !new_selection.collapsed() {
            self.drag_begin_offset_in_text.set(Some(new_selection.start));
        }
        if new_selection.reversed() {
            new_selection = TextRange::new(new_selection.end, new_selection.start);
        }
        s.text_field_state.select_chars_in(new_selection);
        s.update_text_toolbar_state(TextToolbarState::Selection);
        new_selection
    }
}

impl MouseSelectionObserver for TextFieldMouseSelectionObserver {
    fn on_start(&self, down_position: Offset, adjustment: SelectionAdjustment, click_count: u32) -> bool {
        let Some((s, layout)) = self.usable() else { return false };
        self.is_double_or_triple_click_only.set(click_count >= 2);
        s.direct_drag_gesture_initiator.set(InputType::Mouse);
        (self.request_focus)();
        s.previous_raw_drag_offset.set(None);
        self.drag_begin_offset_in_text.set(None);
        self.drag_begin_position.set(down_position);
        let new_selection = self.update_selection(&s, down_position, adjustment, &layout, true);
        self.drag_begin_offset_in_text.set(Some(new_selection.start));
        true
    }

    fn on_drag(&self, drag_position: Offset, adjustment: SelectionAdjustment) -> bool {
        let Some((s, layout)) = self.usable() else { return false };
        let previous = s.text_field_state.visual_text().selection();
        let new_selection = self.update_selection(&s, drag_position, adjustment, &layout, false);
        if previous != new_selection {
            self.is_double_or_triple_click_only.set(false);
        }
        true
    }

    fn on_drag_done(&self) {
        let Some(s) = self.state.upgrade() else { return };
        s.direct_drag_gesture_initiator.set(InputType::None);
        if self.is_double_or_triple_click_only.get() {
            s.maybe_suggest_selection_range();
        }
    }

    fn on_extend(&self, down_position: Offset) -> bool {
        let Some((s, layout)) = self.usable() else { return false };
        self.is_double_or_triple_click_only.set(false);
        (self.request_focus)();
        self.update_selection(&s, down_position, SelectionAdjustment::None, &layout, false);
        true
    }

    fn on_extend_drag(&self, _drag_position: Offset) -> bool {
        true
    }
}

/// A finger's long press and drag, or double tap, selecting in the field (Compose's TextFieldTextDragObserver):
/// it works with what is shown (the last layout's text).
struct TextFieldTextDragObserver {
    state: Weak<TextFieldSelectionState>,
    request_focus: Rc<dyn Fn()>,
    drag_begin_offset_in_text: Cell<Option<usize>>,
    drag_begin_position: Cell<Offset>,
    drag_total_distance: Cell<Offset>,
    acting_handle: Cell<Handle>,
    is_long_press_selection_only: Cell<bool>,
    selection_adjustment_mode: Cell<SelectionAdjustment>,
}

impl TextFieldTextDragObserver {
    fn on_drag_stop(&self) {
        if self.drag_begin_position.get().is_unspecified() {
            return;
        }
        let Some(s) = self.state.upgrade() else { return };
        s.clear_handle_dragging();
        self.drag_begin_offset_in_text.set(None);
        self.drag_begin_position.set(Offset::Unspecified);
        self.drag_total_distance.set(Offset::Zero);
        s.previous_raw_drag_offset.set(None);
        self.selection_adjustment_mode.set(SelectionAdjustment::None);
        s.direct_drag_gesture_initiator.set(InputType::None);
        (self.request_focus)();
        if self.is_long_press_selection_only.get() {
            s.maybe_suggest_selection_range();
        }
    }
}

impl TextDragObserver for TextFieldTextDragObserver {
    fn on_down(&self, _point: Offset) {}

    fn on_up(&self) {}

    fn on_start(&self, start_point: Offset, selection_adjustment: SelectionAdjustment) {
        let Some(s) = self.state.upgrade() else { return };
        if !s.enabled.get() {
            return;
        }
        // On the decoration box: handle_drag_position converts it.
        s.update_handle_dragging(self.acting_handle.get(), start_point);
        s.show_cursor_handle.set(false);
        s.direct_drag_gesture_initiator.set(InputType::Touch);
        self.drag_begin_position.set(start_point);
        self.drag_total_distance.set(Offset::Zero);
        s.previous_raw_drag_offset.set(None);
        self.is_long_press_selection_only.set(true);
        self.selection_adjustment_mode.set(selection_adjustment);
        if s.text_layout_state.layout_result().is_none() {
            return;
        }
        if !s.text_layout_state.is_position_on_text(start_point) {
            // A long press beside the text: the cursor at the line's end.
            let offset = s.text_layout_state.get_offset_for_position(start_point, true).unwrap_or(0);
            s.perform_haptic(HapticFeedbackType::LongPress);
            s.text_field_state.place_cursor_before_char_at(offset);
            s.show_cursor_handle.set(true);
            self.is_long_press_selection_only.set(false);
            s.update_text_toolbar_state(TextToolbarState::Cursor);
        } else {
            let visual = s.text_field_state.visual_text();
            if visual.is_empty() {
                return;
            }
            let offset = s.text_layout_state.get_offset_for_position(start_point, true).unwrap_or(0);
            // The selection before is no context for a new one.
            let reset = TextFieldCharSequence::new(visual.text_arc(), TextRange::Zero, None);
            let new_selection = s.update_selection(
                &reset,
                offset,
                offset,
                false,
                selection_adjustment,
                false,
                false,
                Some(HapticFeedbackType::LongPress),
            );
            s.text_field_state.select_chars_in(new_selection);
            s.update_text_toolbar_state(TextToolbarState::Selection);
            self.drag_begin_offset_in_text.set(Some(new_selection.start));
        }
    }

    fn on_drag(&self, delta: Offset) {
        let Some(s) = self.state.upgrade() else { return };
        if !s.enabled.get()
            || s.text_layout_state.layout_result().is_none()
            || s.text_field_state.visual_text().is_empty()
        {
            return;
        }
        self.drag_total_distance.set(self.drag_total_distance.get() + delta);
        let current_drag_position = self.drag_begin_position.get() + self.drag_total_distance.get();
        let begin = self.drag_begin_offset_in_text.get();
        let (start_offset, end_offset, adjustment);
        if begin.is_none() && !s.text_layout_state.is_position_on_text(current_drag_position) {
            // Started and still in the end padding.
            start_offset =
                s.text_layout_state.get_offset_for_position(self.drag_begin_position.get(), true).unwrap_or(0);
            end_offset = s.text_layout_state.get_offset_for_position(current_drag_position, true).unwrap_or(0);
            adjustment = if start_offset == end_offset {
                SelectionAdjustment::None
            } else {
                self.selection_adjustment_mode.get()
            };
        } else {
            // (ComposeFoundationFlags.isConcurrentTextFieldSelectionFixEnabled, on)
            let text_length = s.text_layout_state.layout_result().map_or(0, |l| l.layout_input.text.text().len());
            start_offset = match begin {
                Some(b) if b <= text_length => b,
                _ => s.text_layout_state.get_offset_for_position(self.drag_begin_position.get(), false).unwrap_or(0),
            };
            end_offset = s.text_layout_state.get_offset_for_position(current_drag_position, false).unwrap_or(0);
            if begin.is_none() && start_offset == end_offset {
                // From the end padding: no selection until it expands.
                return;
            }
            adjustment = self.selection_adjustment_mode.get();
            s.update_text_toolbar_state(TextToolbarState::Selection);
        }
        let previous = s.text_field_state.visual_text().selection();
        let mut new_selection = s.update_selection(
            &s.text_field_state.visual_text(),
            start_offset,
            end_offset,
            false,
            adjustment,
            false,
            false,
            Some(HapticFeedbackType::TextHandleMove),
        );
        if self.drag_begin_offset_in_text.get().is_none() && !new_selection.collapsed() {
            self.drag_begin_offset_in_text.set(Some(new_selection.start));
        }
        if new_selection.reversed() {
            new_selection = TextRange::new(new_selection.end, new_selection.start);
        }
        if new_selection != previous {
            // The growing end is the acting handle (both moved: where the middle went).
            let handle = if new_selection.start != previous.start && new_selection.end == previous.end {
                Handle::SelectionStart
            } else if new_selection.start == previous.start && new_selection.end != previous.end {
                Handle::SelectionEnd
            } else {
                let new_middle = (new_selection.start + new_selection.end) as f32 / 2.0;
                let previous_middle = (previous.start + previous.end) as f32 / 2.0;
                if new_middle > previous_middle { Handle::SelectionEnd } else { Handle::SelectionStart }
            };
            self.acting_handle.set(handle);
            self.is_long_press_selection_only.set(false);
        }
        // The selection never collapses while dragged.
        if previous.collapsed() || !new_selection.collapsed() {
            s.text_field_state.select_chars_in(new_selection);
        }
        s.update_handle_dragging(self.acting_handle.get(), current_drag_position);
    }

    fn on_stop(&self) {
        self.on_drag_stop();
    }

    fn on_cancel(&self) {
        self.on_drag_stop();
    }
}

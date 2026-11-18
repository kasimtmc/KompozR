//! What a value-based field does with its text (Compose's TextFieldDelegate.kt): lays it out, draws it with its
//! selection or an input method's preview highlight, tells its input session where the caret is and how the text
//! is laid out, applies the input method's commands, places the cursor at a tap, starts and ends the session, and
//! underlines what the input method composes.

use super::text_delegate::TextDelegate;
use super::text_field_size::compute_size_for_default_text;
use super::text_layout_result_proxy::TextLayoutResultProxy;
use kompoz_graphics::{Color, Constraints, LayoutDirection, Matrix, Offset, Rect, Size};
use kompoz_text::input::{EditCommand, EditProcessor, OffsetMapping, TextFieldValue, TransformedText};
use kompoz_text::{AnnotatedStringBuilder, SpanStyle, TextDecoration, TextLayoutResult, TextRange};
use kompoz_ui::draw::{DefaultBlendMode, DrawScope, DrawStyle};
use kompoz_ui::layout::LayoutCoordinates;
use kompoz_ui::platform::{ImeAction, ImeOptions, TextEditState};
use kompoz_ui::text_input::{TextInputService, TextInputSession};
use std::cell::RefCell;
use std::rc::Rc;

/// What a field's value changes go to (Compose's `onValueChange: (TextFieldValue) -> Unit`).
pub type OnValueChange = Rc<dyn Fn(TextFieldValue)>;

/// `text_delegate`'s text laid out (Compose's `TextFieldDelegate.layout`): its width, height and layout,
/// `prev_result` reused where it can be.
pub fn layout(
    text_delegate: &TextDelegate,
    constraints: Constraints,
    layout_direction: LayoutDirection,
    prev_result: Option<&TextLayoutResult>,
) -> (i32, i32, TextLayoutResult) {
    let result = text_delegate.layout(constraints, layout_direction, prev_result);
    (result.size.width, result.size.height, result)
}

/// Draws the field's text (Compose's `TextFieldDelegate.draw`): first an input method's selection preview
/// highlight in the selection's color, else its deletion preview in the text's color at a fifth, else the
/// selection; then the text.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    scope: &mut DrawScope,
    value: &TextFieldValue,
    selection_preview_highlight_range: TextRange,
    deletion_preview_highlight_range: TextRange,
    offset_mapping: &OffsetMapping,
    text_layout_result: &TextLayoutResult,
    selection_background_color: Color,
) {
    if !selection_preview_highlight_range.collapsed() {
        draw_highlight(
            scope,
            selection_preview_highlight_range,
            offset_mapping,
            text_layout_result,
            selection_background_color,
        );
    } else if !deletion_preview_highlight_range.collapsed() {
        let color = text_layout_result.layout_input.style.color;
        let text_color = if color.is_unspecified() { Color::Black } else { color };
        let faded = kompoz_runtime::named_args!(text_color.copy(alpha = text_color.alpha() * 0.2));
        draw_highlight(scope, deletion_preview_highlight_range, offset_mapping, text_layout_result, faded);
    } else if !value.selection().collapsed() {
        draw_highlight(scope, value.selection(), offset_mapping, text_layout_result, selection_background_color);
    }
    // (Compose's TextPainter.paint)
    scope.draw_text_with(
        text_layout_result,
        Color::Unspecified,
        Offset::Zero,
        f32::NAN,
        None,
        None,
        None,
        DefaultBlendMode,
    );
}

/// Fills `range` of the shown text (Compose's private `drawHighlight`).
fn draw_highlight(
    scope: &mut DrawScope,
    range: TextRange,
    offset_mapping: &OffsetMapping,
    text_layout_result: &TextLayoutResult,
    color: Color,
) {
    let start = offset_mapping.original_to_transformed(range.min());
    let end = offset_mapping.original_to_transformed(range.max());
    if start != end {
        let path = text_layout_result.get_path_for_range(start, end);
        scope.draw_path_with(&path, color, 1.0, DrawStyle::Fill, None, DefaultBlendMode);
    }
}

/// Tells `text_input_session` where the caret is, in the root (Compose's `TextFieldDelegate.notifyFocusedRect`):
/// only while focused.
#[allow(clippy::too_many_arguments)]
pub fn notify_focused_rect(
    value: &TextFieldValue,
    text_delegate: &TextDelegate,
    text_layout_result: &TextLayoutResult,
    layout_coordinates: &LayoutCoordinates,
    text_input_session: &TextInputSession,
    has_focus: bool,
    offset_mapping: &OffsetMapping,
) {
    if !has_focus {
        return;
    }
    let rect = focused_rect_in_root(
        text_layout_result,
        layout_coordinates,
        offset_mapping.original_to_transformed(value.selection().max()),
        || {
            compute_size_for_default_text(
                text_delegate.style(),
                text_delegate.density(),
                text_delegate.font_family_resolver(),
                1,
            )
            .height as f32
        },
    );
    text_input_session.notify_focused_rect(rect);
}

/// Tells `text_input_session` how the text is laid out and where the field is (Compose's
/// `TextFieldDelegate.updateTextLayoutResult`): once both the inner field and the decoration box are known.
pub fn update_text_layout_result(
    text_input_session: &TextInputSession,
    text_field_value: &TextFieldValue,
    offset_mapping: &OffsetMapping,
    text_layout_result: &TextLayoutResultProxy,
) {
    let Some(inner) = text_layout_result.inner_text_field_coordinates() else { return };
    if !inner.is_attached() {
        return;
    }
    let Some(decoration) = text_layout_result.decoration_box_coordinates() else { return };
    let transform_inner = inner.clone();
    let to_root = move |matrix: &mut Matrix| {
        if transform_inner.is_attached() {
            transform_inner.find_root_coordinates().transform_from(&transform_inner, matrix);
        }
    };
    let visible = super::selection::visible_bounds(&inner);
    let decoration_bounds = inner.local_bounding_box_of(&decoration, false);
    text_input_session.update_text_layout_result(
        text_field_value,
        offset_mapping,
        &text_layout_result.value,
        &to_root,
        visible,
        decoration_bounds,
    );
}

/// Applies the input method's `ops` (Compose's `TextFieldDelegate.onEditCommand`): the processor's value after them
/// told to the session first (the input method reads the text back before the app's next composition), then to
/// the field.
pub fn on_edit_command(
    ops: &[EditCommand],
    edit_processor: &RefCell<EditProcessor>,
    on_value_change: &OnValueChange,
    session: Option<&TextInputSession>,
) {
    let new_value = edit_processor.borrow_mut().apply(ops);
    if let Some(session) = session {
        session.update_state_from(None, &new_value);
    }
    on_value_change(new_value);
}

/// Places the cursor where `position` (on the decoration box) is (Compose's `TextFieldDelegate.setCursorOffset`).
pub fn set_cursor_offset(
    position: Offset,
    text_layout_result: &TextLayoutResultProxy,
    edit_processor: &RefCell<EditProcessor>,
    offset_mapping: &OffsetMapping,
    on_value_change: &OnValueChange,
) {
    let offset = offset_mapping.transformed_to_original(text_layout_result.get_offset_for_position(position, true));
    let value = edit_processor.borrow().to_text_field_value();
    on_value_change(value.copy_with_selection(TextRange::at(offset)));
}

/// Starts the field's input session with `value` (Compose's `TextFieldDelegate.restartInput`): the input
/// method's commands applied through `edit_processor` to `on_value_change`, its action key to
/// `on_ime_action_performed`.
pub fn restart_input(
    text_input_service: &TextInputService,
    value: &TextFieldValue,
    edit_processor: &Rc<RefCell<EditProcessor>>,
    ime_options: &ImeOptions,
    on_value_change: &OnValueChange,
    on_ime_action_performed: Rc<dyn Fn(ImeAction)>,
) -> Rc<TextInputSession> {
    // (the session its commands are applied with, known once it started)
    let slot: Rc<RefCell<Option<std::rc::Weak<TextInputSession>>>> = Rc::new(RefCell::new(None));
    let (processor, change, s) = (edit_processor.clone(), on_value_change.clone(), slot.clone());
    let session = Rc::new(text_input_service.start_input(
        TextEditState::from_value(value),
        ime_options.clone(),
        move |ops: &[EditCommand]| {
            let session = s.borrow().as_ref().and_then(|w| w.upgrade());
            on_edit_command(ops, &processor, &change, session.as_deref());
        },
        move |action| on_ime_action_performed(action),
    ));
    *slot.borrow_mut() = Some(Rc::downgrade(&session));
    session
}

/// The field took the focus: its session starts (Compose's `TextFieldDelegate.onFocus`; the keyboard shows with
/// it).
pub fn on_focus(
    text_input_service: &TextInputService,
    value: &TextFieldValue,
    edit_processor: &Rc<RefCell<EditProcessor>>,
    ime_options: &ImeOptions,
    on_value_change: &OnValueChange,
    on_ime_action_performed: Rc<dyn Fn(ImeAction)>,
) -> Rc<TextInputSession> {
    restart_input(text_input_service, value, edit_processor, ime_options, on_value_change, on_ime_action_performed)
}

/// The field lost the focus (Compose's `TextFieldDelegate.onBlur`): its composition finished, its session ended
/// (the keyboard isn't hidden here).
pub fn on_blur(
    text_input_session: &TextInputSession,
    edit_processor: &RefCell<EditProcessor>,
    on_value_change: &OnValueChange,
) {
    let value = edit_processor.borrow().to_text_field_value();
    on_value_change(TextFieldValue::new(value.annotated_string().clone(), value.selection(), None));
    text_input_session.dispose();
}

/// What the input method composes underlined in the shown text (Compose's
/// `TextFieldDelegate.applyCompositionDecoration`).
pub fn apply_composition_decoration(composition_range: TextRange, transformed: &TransformedText) -> TransformedText {
    let start_transformed = transformed.offset_mapping.original_to_transformed(composition_range.start);
    let end_transformed = transformed.offset_mapping.original_to_transformed(composition_range.end);
    let (start, end) = (start_transformed.min(end_transformed), start_transformed.max(end_transformed));
    let mut builder = AnnotatedStringBuilder::new();
    builder.append(&transformed.text);
    builder.add_style(
        SpanStyle { text_decoration: Some(TextDecoration::Underline), ..SpanStyle::default() },
        start,
        end,
    );
    TransformedText::new(builder.to_annotated_string(), transformed.offset_mapping.clone())
}

/// Where editing happens, in the root (Compose's `focusedRectInRoot`): the box of the character at `focus_offset`
/// (or of the one before it at the end; a pixel wide line of the default text's height in empty text).
pub fn focused_rect_in_root(
    layout_result: &TextLayoutResult,
    layout_coordinates: &LayoutCoordinates,
    focus_offset: usize,
    size_for_default_text_height: impl FnOnce() -> f32,
) -> Rect {
    let text = layout_result.layout_input.text.text();
    let bbox = if focus_offset < text.len() {
        layout_result.get_bounding_box(focus_offset)
    } else if focus_offset != 0 {
        layout_result.get_bounding_box(previous_char(text, focus_offset))
    } else {
        Rect::new(0.0, 0.0, 1.0, size_for_default_text_height())
    };
    let global_lt = layout_coordinates.local_to_root(Offset::new(bbox.left, bbox.top));
    Rect::from_offset_size(global_lt, Size::new(bbox.width(), bbox.height()))
}

/// The start of the character before `offset` (Compose's `offset - 1`).
pub(crate) fn previous_char(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].char_indices().next_back().map_or(0, |(i, _)| i)
}

/// A value with another selection, its composition kept (Compose's `copy(selection = …)`).
pub(crate) trait CopyWithSelection {
    fn copy_with_selection(&self, selection: TextRange) -> TextFieldValue;
}

impl CopyWithSelection for TextFieldValue {
    fn copy_with_selection(&self, selection: TextRange) -> TextFieldValue {
        TextFieldValue::new(self.annotated_string().clone(), selection, self.composition())
    }
}

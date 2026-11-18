//! A value-based field's cursor (Compose's TextFieldCursor.kt, the legacy `Modifier.cursor`): while the field and
//! its window have the focus and the selection is a caret, a line of `cursor_brush` at the caret, shown at once
//! after each edit or move and blinking every half second (unless `LocalCursorBlinkEnabled` says not to).

use super::core_text_field::LegacyTextFieldState;
use crate::text::input::internal::CursorAnimationState;
use crate::text::input::internal::selection::DefaultCursorThickness;
use kompoz_coroutines::LaunchedEffect;
use kompoz_graphics::{Brush, Offset, Rect, StrokeCap};
use kompoz_runtime::remember_keyed;
use kompoz_text::input::{OffsetMapping, TextFieldValue};
use kompoz_ui::Modifier;
use kompoz_ui::draw::DefaultBlendMode;
use kompoz_ui::platform::{LocalCursorBlinkEnabled, LocalWindowInfo};
use std::rc::Rc;

/// The cursor drawn over the inner field's content (Compose's legacy `Modifier.cursor`): nothing when not
/// `enabled`, or while the field or its window hasn't the focus, a selection shows, or the brush is an
/// unspecified color.
#[track_caller]
pub fn cursor(
    modifier: Modifier,
    state: &Rc<LegacyTextFieldState>,
    value: &TextFieldValue,
    offset_mapping: &OffsetMapping,
    cursor_brush: &Brush,
    enabled: bool,
) -> Modifier {
    kompoz_runtime::group(|| {
        kompoz_runtime::kompozable_block!({
            if !enabled {
                return modifier;
            }
            let animate_cursor = LocalCursorBlinkEnabled.current();
            let cursor_animation = remember_keyed(animate_cursor, move || CursorAnimationState::new(animate_cursor));
            // (no animation for a cursor that draws no pixel)
            let is_brush_specified = cursor_brush.solid_color().is_none_or(|c| c.is_specified());
            // (only blinking while its window has the focus: not with the screen off)
            let is_window_focused = LocalWindowInfo.current().is_window_focused();
            if is_window_focused && state.has_focus() && value.selection().collapsed() && is_brush_specified {
                let animation = cursor_animation.clone();
                LaunchedEffect((value.annotated_string().clone(), value.selection()), async move {
                    animation.snap_to_visible_and_animate().await;
                });
                let (state, start, offset_mapping, brush) =
                    (state.clone(), value.selection().start, offset_mapping.clone(), cursor_brush.clone());
                modifier.draw_with_content(move |scope| {
                    scope.draw_content();
                    let cursor_alpha = cursor_animation.cursor_alpha();
                    if cursor_alpha == 0.0 {
                        return;
                    }
                    let transformed_offset = offset_mapping.original_to_transformed(start);
                    let cursor_rect = state
                        .layout_result()
                        .map(|l| l.value.get_cursor_rect(transformed_offset))
                        .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
                    let cursor_width = scope.to_px(DefaultCursorThickness).floor().max(1.0);
                    let width = scope.size().width;
                    // (not coerceIn: the minimum isn't sure to be below the maximum)
                    let x =
                        (cursor_rect.left + cursor_width / 2.0).min(width - cursor_width / 2.0).max(cursor_width / 2.0);
                    // An odd width drawn in the middle of a pixel, not blurred by antialiasing.
                    let cursor_x = if (cursor_width as i32) % 2 == 1 { x.floor() + 0.5 } else { x.round_ties_even() };
                    scope.draw_line_with(
                        brush.clone(),
                        Offset::new(cursor_x, cursor_rect.top),
                        Offset::new(cursor_x, cursor_rect.bottom),
                        cursor_width,
                        StrokeCap::Butt,
                        None,
                        cursor_alpha,
                        None,
                        DefaultBlendMode,
                    );
                })
            } else {
                modifier
            }
        })
    })
}

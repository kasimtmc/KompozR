//! A value-based field's pointer input (Compose's TextFieldPointerModifier and TextFieldPressGestureFilter): whether
//! touch or a mouse is used, a tap's press told to the interaction source and placing the cursor (or ending a
//! selection), the mouse's and touch's selection gestures, the text pointer icon.

use super::core_text_field::{HandleState, LegacyTextFieldState, request_focus_and_show_keyboard_if_needed};
use super::selection::text_field_selection_manager::TextFieldSelectionManager;
use super::text_field_delegate as delegate;
use crate::gestures::{PressCallback, PressGestureScope, detect_tap_and_press};
use crate::interaction::MutableInteractionSource;
use crate::interaction::PressInteraction;
use crate::text::TextDragObserver;
use crate::text::selection::{MouseSelectionObserver, await_selection_gestures, update_selection_touch_mode};
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_graphics::Offset;
use kompoz_runtime::{DisposableEffect, on_dispose, remember, remember_updated_state};
use kompoz_text::input::OffsetMapping;
use kompoz_ui::Modifier;
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::input::PointerInputScope;
use kompoz_ui::platform::PointerIcon;
use std::cell::Cell;
use std::rc::Rc;

/// The press held now, if any.
type PressedInteraction = Rc<Cell<Option<PressInteraction::Press>>>;

/// Ends the press held, if any, as cancelled (Compose's "Remove any old interactions if we didn't fire stop /
/// cancel properly").
async fn cancel_pressed(pressed: &PressedInteraction, source: &Option<MutableInteractionSource>) {
    if let Some(old) = pressed.take()
        && let Some(source) = source
    {
        source.emit(PressInteraction::Cancel::new(old)).await;
    }
}

/// Taps and their presses (Compose's `tapPressTextFieldModifier`): a press emitted to `interaction_source` and
/// released or cancelled with the pointer; each tap's position to `on_tap`. Nothing when not `enabled`.
#[track_caller]
pub fn tap_press_text_field_modifier(
    modifier: Modifier,
    interaction_source: Option<MutableInteractionSource>,
    enabled: bool,
    on_tap: impl Fn(Offset) + 'static,
) -> Modifier {
    kompoz_runtime::group(|| {
        kompoz_runtime::kompozable_block!({
            if !enabled {
                return modifier;
            }
            let scope: CoroutineScope = remember_coroutine_scope();
            let pressed_interaction: PressedInteraction = remember(|| Rc::new(Cell::new(None)));
            let on_tap_state = remember_updated_state!(Rc::new(on_tap) as Rc<dyn Fn(Offset)>);
            {
                let (pressed, source) = (pressed_interaction.clone(), interaction_source.clone());
                DisposableEffect(interaction_source.clone(), move || {
                    on_dispose(move || {
                        if let Some(old) = pressed.take()
                            && let Some(source) = &source
                        {
                            source.try_emit(PressInteraction::Cancel::new(old));
                        }
                    })
                });
            }
            let (source, pressed, tap) = (interaction_source.clone(), pressed_interaction, on_tap_state);
            modifier.pointer_input(interaction_source.clone(), move |pointer: PointerInputScope| {
                let (scope, source, pressed, tap) = (scope.clone(), source.clone(), pressed.clone(), tap.clone());
                let on_press: PressCallback = Rc::new(move |press_scope: PressGestureScope, at: Offset| {
                    let (scope, source, pressed) = (scope.clone(), source.clone(), pressed.clone());
                    Box::pin(async move {
                        {
                            let (source, pressed) = (source.clone(), pressed.clone());
                            scope.launch(async move {
                                cancel_pressed(&pressed, &source).await;
                                let interaction = PressInteraction::Press::new(at);
                                if let Some(source) = &source {
                                    source.emit(interaction).await;
                                }
                                pressed.set(Some(interaction));
                            });
                        }
                        let success = press_scope.try_await_release().await;
                        scope.launch(async move {
                            if let Some(old) = pressed.take()
                                && let Some(source) = &source
                            {
                                if success {
                                    source.emit(PressInteraction::Release::new(old)).await;
                                } else {
                                    source.emit(PressInteraction::Cancel::new(old)).await;
                                }
                            }
                        });
                    })
                });
                let on_tap: Rc<dyn Fn(Offset)> = Rc::new(move |at| (tap.get())(at));
                detect_tap_and_press(pointer, Some(on_press), Some(on_tap))
            })
        })
    })
}

/// The field's pointer input (Compose's `textFieldPointer`, its `defaultTextFieldPointer`): touch mode followed; a
/// tap focuses the field (or shows the keyboard again), then places the cursor (with its handle, on text) or ends
/// a selection there; the mouse's and touch's selection gestures through the manager's observers; the text
/// pointer icon.
#[allow(clippy::too_many_arguments)]
#[track_caller]
pub fn text_field_pointer(
    modifier: Modifier,
    manager: &Rc<TextFieldSelectionManager>,
    enabled: bool,
    interaction_source: Option<MutableInteractionSource>,
    state: &Rc<LegacyTextFieldState>,
    focus_requester: &FocusRequester,
    read_only: bool,
    offset_mapping: &OffsetMapping,
) -> Modifier {
    let touch_state = state.clone();
    let touch_mode = update_selection_touch_mode(modifier, move |it| touch_state.set_is_in_touch_mode(it));
    let (state, manager_for_tap, focus_requester, offset_mapping) =
        (state.clone(), manager.clone(), focus_requester.clone(), offset_mapping.clone());
    let tapped = tap_press_text_field_modifier(touch_mode, interaction_source, enabled, move |offset| {
        request_focus_and_show_keyboard_if_needed(&state, &focus_requester, !read_only);
        if state.has_focus() && enabled {
            if state.peek_handle_state() != HandleState::Selection {
                if let Some(layout_result) = state.peek_layout_result() {
                    delegate::set_cursor_offset(
                        offset,
                        &layout_result,
                        &state.processor,
                        &offset_mapping,
                        &state.on_value_change_callback(),
                    );
                    // No cursor state on empty text.
                    if !state.text_delegate().text().text().is_empty() {
                        state.set_handle_state(HandleState::Cursor);
                    }
                }
            } else {
                manager_for_tap.deselect(Some(offset));
            }
        }
    });
    let mouse: Rc<dyn MouseSelectionObserver> = manager.mouse_selection_observer();
    let touch: Rc<dyn TextDragObserver> = manager.touch_selection_observer();
    let keys = (Rc::as_ptr(&mouse) as *const () as usize, Rc::as_ptr(&touch) as *const () as usize);
    tapped
        .pointer_input(keys, move |scope: PointerInputScope| {
            let (mouse, touch) = (mouse.clone(), touch.clone());
            async move { await_selection_gestures(scope, mouse, Some(touch)).await }
        })
        .pointer_hover_icon(PointerIcon::Text)
}

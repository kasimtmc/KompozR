//! The state-based text field (Compose's BasicTextField(state, …) and its internal overload): a box the user
//! edits a [`TextFieldState`] in through the keyboard, the input method, the mouse and touch — no decorations of
//! its own beyond what its [`TextFieldDecorator`] draws around the inner field. Its parts are the nodes of
//! [`input::internal`](super::input::internal): the decorator node (focus, input session, keys, gestures,
//! semantics) on the decoration box, the core node (scroll, cursor, selection, drawing) on the inner field, the
//! text layout node inside it; and the selection state's handles, magnifier and toolbar.

use super::input::internal::selection::{
    TextFieldMagnifierElement, TextFieldSelectionState, TextToolbarHandler, TextToolbarState,
};
use super::input::internal::{
    CodepointTransformation, OnTextFieldLayout, SingleLineCodepointTransformation, TextFieldCoreModifier,
    TextFieldDecoratorModifier, TextFieldTextLayoutModifier, TextLayoutState, TransformedTextFieldState,
};
use super::input::{
    InputTransformation, KeyboardActionHandler, OutputTransformation, TextFieldLineLimits, TextFieldState,
};
use super::keyboard_options::KeyboardOptions;
use super::selection::{
    HandleHeight, HandlePopup, OffsetProvider, SelectionHandle, SelectionHandleAnchor, SelectionHandleInfo,
    SelectionHandleInfoKey,
};
use super::text_field_size::TextFieldSizeModifiers;
use crate::gestures::{Orientation, ScrollableDefaults, ScrollableModifiers};
use crate::interaction::MutableInteractionSource;
use crate::layout::{Box, SizeModifiers, Spacer};
use crate::scroll::{ScrollState, remember_scroll_state};
use crate::text::long_press_text_drag_observer::Handle;
use crate::text::selection::LocalTextSelectionColors;
use kompoz_graphics::{BlendMode, Brush, Color, Dp, DpSize, Offset, Rect, Size, SolidColor};
use kompoz_runtime::{
    DisposableEffect, SideEffect, derived_state_of, kompozable, on_dispose, remember, remember_keyed,
};
use kompoz_text::TextStyle;
use kompoz_ui::draw::Fill;
use kompoz_ui::input::PointerInputScope;
use kompoz_ui::platform::{
    LocalClipboard, LocalDensity, LocalHapticFeedback, LocalLayoutDirection, LocalTextToolbar, LocalWindowInfo,
    PointerIcon, TextToolbar, TextToolbarAction, TextToolbarStatus,
};
use kompoz_ui::{Alignment, Modifier};
use std::rc::Rc;

/// What a field's decoration is (Compose's TextFieldDecorator): it composes around the inner field, which it
/// must call exactly once (`inner_text_field`), adding decorations (an icon, a placeholder, a border) that
/// widen the field's touch target. Any `Fn(&dyn Fn())` is one (Kotlin's `TextFieldDecorator { innerTextField ->
/// … }`).
pub trait TextFieldDecorator {
    /// Composes the decoration around `inner_text_field` (Compose's `Decoration`).
    fn decoration(&self, inner_text_field: &dyn Fn());
}

impl<F: Fn(&dyn Fn())> TextFieldDecorator for F {
    fn decoration(&self, inner_text_field: &dyn Fn()) {
        self(inner_text_field)
    }
}

/// The defaults of BasicTextField (Compose's private BasicTextFieldDefaults).
pub struct BasicTextFieldDefaults;

impl BasicTextFieldDefaults {
    /// The cursor's brush unless given: black (Compose's `CursorBrush`).
    pub fn cursor_brush() -> Brush {
        SolidColor(Color::Black)
    }
}

/// The least touch target of a field's handles (Compose's MinTouchTargetSizeForHandles: Android TextView's).
const MinTouchTargetSizeForHandles: DpSize = DpSize { width: Dp(40.0), height: Dp(40.0) };

/// A box the user edits `state`'s text in (Compose's `BasicTextField(state, …)`): through the keyboard and the
/// input method (an input session while it has the focus, the keyboard shown as `keyboard_options` say), the
/// mouse (a click places the cursor, a double click selects a word, a triple one a paragraph, drags and shift
/// extend) and touch (a tap places the cursor, with a handle; a long press or double tap selects a word, with
/// handles, a magnifier and the text toolbar); copy, cut, paste and select all by keys, the toolbar and
/// accessibility.
///
/// Not `enabled`: neither editable nor focusable, nothing selected. `read_only`: focusable and selectable, not
/// editable. `input_transformation` changes or refuses the user's edits (not the app's); `output_transformation`
/// changes what is shown. `on_keyboard_action` runs for the input method's action key (and Enter in a single-line
/// field), given the default action (Done hides the keyboard, Next and Previous move the focus); none runs the
/// default. `line_limits`: a single line scrolls sideways, newlines shown as spaces; multiple lines grow from
/// their min height up to their max one, then scroll down. `on_text_layout` hears of each layout. The cursor is
/// drawn with `cursor_brush` (an unspecified color: none). `decorator` composes around the inner field.
/// `scroll_state` holds how far it scrolled (none: one remembered here, Compose's `rememberScrollState()`).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn BasicTextField(
    state: TextFieldState,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(false)] read_only: bool,
    #[default(None)] input_transformation: Option<InputTransformation>,
    #[default(TextStyle::Default)] text_style: TextStyle,
    #[default(KeyboardOptions::Default)] keyboard_options: KeyboardOptions,
    #[default(None)] on_keyboard_action: Option<Rc<dyn KeyboardActionHandler>>,
    #[default(TextFieldLineLimits::Default)] line_limits: TextFieldLineLimits,
    #[default(None)] on_text_layout: Option<OnTextFieldLayout>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(BasicTextFieldDefaults::cursor_brush())] cursor_brush: Brush,
    #[default(None)] output_transformation: Option<Rc<dyn OutputTransformation>>,
    #[default(None)] decorator: Option<Rc<dyn TextFieldDecorator>>,
    #[default(None)] scroll_state: Option<ScrollState>,
) {
    basic_text_field_impl(BasicTextFieldArgs {
        state,
        modifier,
        enabled,
        read_only,
        input_transformation,
        text_style,
        keyboard_options,
        on_keyboard_action,
        line_limits,
        on_text_layout,
        interaction_source,
        cursor_brush,
        codepoint_transformation: None,
        output_transformation,
        decorator,
        scroll_state,
        is_password: false,
    });
}

/// Everything the internal BasicTextField takes (Compose's internal overload's parameters: a code point
/// transformation, a secure field's, and whether it is a password).
#[derive(Clone)]
pub(crate) struct BasicTextFieldArgs {
    pub(crate) state: TextFieldState,
    pub(crate) modifier: Modifier,
    pub(crate) enabled: bool,
    pub(crate) read_only: bool,
    pub(crate) input_transformation: Option<InputTransformation>,
    pub(crate) text_style: TextStyle,
    pub(crate) keyboard_options: KeyboardOptions,
    pub(crate) on_keyboard_action: Option<Rc<dyn KeyboardActionHandler>>,
    pub(crate) line_limits: TextFieldLineLimits,
    pub(crate) on_text_layout: Option<OnTextFieldLayout>,
    pub(crate) interaction_source: Option<MutableInteractionSource>,
    pub(crate) cursor_brush: Brush,
    pub(crate) codepoint_transformation: Option<Rc<dyn CodepointTransformation>>,
    pub(crate) output_transformation: Option<Rc<dyn OutputTransformation>>,
    pub(crate) decorator: Option<Rc<dyn TextFieldDecorator>>,
    pub(crate) scroll_state: Option<ScrollState>,
    pub(crate) is_password: bool,
}

/// The identity of an optional shared object, as a key.
fn identity<T: ?Sized>(rc: &Option<Rc<T>>) -> usize {
    rc.as_ref().map_or(0, |r| Rc::as_ptr(r) as *const () as usize)
}

/// What shows a field's toolbar (Compose's `textToolbarHandler` in BasicTextField): `LocalTextToolbar` with the
/// actions the field can do now, each setting the toolbar's state after it.
struct FieldTextToolbarHandler {
    toolbar: Rc<dyn TextToolbar>,
}

/// A menu item running `operation` then setting the toolbar's `desired_state` (Compose's `menuItem`): none when
/// not `enabled`.
fn menu_item(
    selection_state: &Rc<TextFieldSelectionState>,
    enabled: bool,
    desired_state: TextToolbarState,
    operation: impl Fn(&TextFieldSelectionState) + 'static,
) -> TextToolbarAction {
    if !enabled {
        return None;
    }
    let weak = Rc::downgrade(selection_state);
    Some(Rc::new(move || {
        if let Some(s) = weak.upgrade() {
            operation(&s);
            s.update_text_toolbar_state(desired_state);
        }
    }))
}

impl TextToolbarHandler for FieldTextToolbarHandler {
    fn show_text_toolbar(&self, s: &Rc<TextFieldSelectionState>, rect: Rect) {
        s.update_clipboard_entry();
        self.toolbar.show_menu(
            rect,
            menu_item(s, s.can_show_copy_menu_item(), TextToolbarState::None, |s| s.copy(true)),
            menu_item(s, s.can_show_paste_menu_item(), TextToolbarState::None, |s| s.paste(false)),
            menu_item(s, s.can_show_cut_menu_item(), TextToolbarState::None, |s| s.cut()),
            menu_item(s, s.can_show_select_all_menu_item(), TextToolbarState::Selection, |s| s.select_all()),
            menu_item(s, s.can_show_autofill_menu_item(), TextToolbarState::None, |s| s.autofill()),
        );
    }

    fn hide_text_toolbar(&self) {
        if self.toolbar.status() == TextToolbarStatus::Shown {
            self.toolbar.hide();
        }
    }
}

/// The internal BasicTextField (Compose's internal overload taking a code point transformation).
#[track_caller]
pub(crate) fn basic_text_field_impl(args: BasicTextFieldArgs) {
    kompoz_runtime::kompozable_block!({
        let BasicTextFieldArgs {
            state,
            modifier,
            enabled,
            read_only,
            input_transformation,
            text_style,
            keyboard_options,
            on_keyboard_action,
            line_limits,
            on_text_layout,
            interaction_source,
            cursor_brush,
            codepoint_transformation,
            output_transformation,
            decorator,
            scroll_state,
            is_password,
        } = args;
        let own_scroll_state = remember_scroll_state();
        let scroll_state = scroll_state.unwrap_or(own_scroll_state);
        let density = LocalDensity.current();
        let layout_direction = LocalLayoutDirection.current();
        let single_line = line_limits == TextFieldLineLimits::SingleLine;
        // What tells the cursor the focus, among others.
        let own_interaction_source = remember(MutableInteractionSource::new);
        let interaction_source = interaction_source.unwrap_or(own_interaction_source);
        let orientation = if single_line { Orientation::Horizontal } else { Orientation::Vertical };

        let transformed_keys = (state.clone(), identity(&codepoint_transformation), identity(&output_transformation));
        let (s, ct, ot, it) = (
            state.clone(),
            codepoint_transformation.clone(),
            output_transformation.clone(),
            input_transformation.clone(),
        );
        let transformed_state = remember_keyed(transformed_keys, move || {
            // A secure field's mask first, else a single line's, else none.
            let applied: Option<Rc<dyn CodepointTransformation>> = ct.or_else(|| {
                single_line.then(|| Rc::new(SingleLineCodepointTransformation) as Rc<dyn CodepointTransformation>)
            });
            TransformedTextFieldState::new(s, it, applied, ot)
        });
        // A new transformed state, a new layout state (it held the old one).
        let text_layout_state = remember_keyed(transformed_state.clone(), || Rc::new(TextLayoutState::new()));
        // The transformation's keyboard options can be states: read here.
        let resolved_keyboard_options = keyboard_options
            .fill_unspecified_values_with(input_transformation.as_ref().and_then(|t| t.keyboard_options()).as_ref());

        // (Compose's coroutine scope here serves its clipboard's suspending calls; KompozR's clipboard is at once)
        let current_clipboard = LocalClipboard.current();
        let (ts, ls, cb) = (transformed_state.clone(), text_layout_state.clone(), current_clipboard.clone());
        let text_field_selection_state = remember_keyed(transformed_state.clone(), move || {
            TextFieldSelectionState::new(ts, ls, density, enabled, read_only, is_password, cb)
        });
        let current_haptic_feedback = LocalHapticFeedback.current();
        let current_text_toolbar = LocalTextToolbar.current();
        let toolbar_key = Rc::as_ptr(&current_text_toolbar) as *const () as usize;
        let toolbar = current_text_toolbar.clone();
        let text_toolbar_handler: Rc<dyn TextToolbarHandler> = remember_keyed(toolbar_key, move || {
            Rc::new(FieldTextToolbarHandler { toolbar }) as Rc<dyn TextToolbarHandler>
        });

        {
            // Not snapshot states: updated once the composition is applied.
            let (ts, sel, it, clipboard, haptics, handler) = (
                transformed_state.clone(),
                text_field_selection_state.clone(),
                input_transformation.clone(),
                current_clipboard.clone(),
                current_haptic_feedback.clone(),
                text_toolbar_handler.clone(),
            );
            SideEffect(move || {
                ts.update(it);
                sel.update(haptics, clipboard, handler, density, enabled, read_only, is_password);
            });
        }
        let selection_key = Rc::as_ptr(&text_field_selection_state) as usize;
        let disposed = text_field_selection_state.clone();
        DisposableEffect(selection_key, move || on_dispose(move || disposed.dispose()));

        // (the desktop's text field overscroll: none)
        let drag_input_type = text_field_selection_state.direct_drag_gesture_initiator();
        let is_focused = interaction_source.collect_is_focused_as_state();
        let is_drag_hovered = interaction_source.collect_is_drag_and_drop_hovered_as_state();
        let window_info = LocalWindowInfo.current();
        let focus_keys = (interaction_source.clone(), window_info.clone());
        let (f, w) = (is_focused.clone(), window_info.clone());
        // Derived: not composed again when the window takes the focus after the field.
        let is_window_and_text_field_focused =
            remember_keyed(focus_keys, move || derived_state_of(move || f.get() && w.is_window_focused()));
        let window_and_field_focused = is_window_and_text_field_focused.get();
        let drag_hovered = is_drag_hovered.get();

        let decoration_modifiers = modifier
            .then_element(TextFieldDecoratorModifier {
                text_field_state: transformed_state.clone(),
                text_layout_state: text_layout_state.clone(),
                text_field_selection_state: text_field_selection_state.clone(),
                filter: input_transformation.clone(),
                enabled,
                read_only,
                keyboard_options: resolved_keyboard_options.clone(),
                keyboard_action_handler: on_keyboard_action.clone(),
                single_line,
                interaction_source: interaction_source.clone(),
                is_password,
            })
            .scrollable(
                state = scroll_state.clone(),
                orientation = orientation,
                // Not while disabled, nor while another drag is under way.
                enabled = enabled && drag_input_type == super::input::internal::selection::InputType::None,
                reverse_direction = ScrollableDefaults::reverse_direction(layout_direction, orientation, false),
                interaction_source = Some(interaction_source.clone()),
            )
            .pointer_hover_icon(PointerIcon::Text, false)
            // The magnifier on the decoration box (KompozR draws its loupe in the layout: not cut by the core's clip).
            .then_element(TextFieldMagnifierElement {
                text_field_state: transformed_state.clone(),
                selection_state: text_field_selection_state.clone(),
                text_layout_state: text_layout_state.clone(),
                visible: window_and_field_focused
                    || drag_hovered
                    || drag_input_type == super::input::internal::selection::InputType::Touch,
            });

        let decorator: Rc<dyn TextFieldDecorator> = decorator.unwrap_or_else(|| Rc::new(|inner: &dyn Fn()| inner()));
        let (minimum_lines, maximum_lines) = match line_limits {
            TextFieldLineLimits::MultiLine { min_height_in_lines, max_height_in_lines } => {
                (min_height_in_lines, max_height_in_lines)
            }
            TextFieldLineLimits::SingleLine => (1, 1),
        };
        Box(
            decoration_modifiers,
            propagate_min_constraints = true,
            content = move || {
                // (the desktop's text context menu area: WP6's ContextMenuArea)
                let inner = || {
                    // (Compose desktop's clipboard events handler: none)
                    let text_field_size = Modifier
                        .height_for_single_line_field(&text_layout_state)
                        .height_in_lines(&text_style, minimum_lines, maximum_lines, !single_line)
                        .text_field_min_size(&text_style);
                    let core = TextFieldCoreModifier {
                        is_focused: window_and_field_focused,
                        is_drag_hovered: drag_hovered,
                        text_layout_state: text_layout_state.clone(),
                        text_field_state: transformed_state.clone(),
                        text_field_selection_state: text_field_selection_state.clone(),
                        cursor_brush: cursor_brush.clone(),
                        writeable: enabled && !read_only,
                        scroll_state: scroll_state.clone(),
                        orientation,
                    };
                    let layout = TextFieldTextLayoutModifier {
                        text_layout_state: text_layout_state.clone(),
                        text_field_state: transformed_state.clone(),
                        text_style: text_style.clone(),
                        single_line,
                        on_text_layout: on_text_layout.clone(),
                        keyboard_options: resolved_keyboard_options.clone(),
                    };
                    let selection_state = text_field_selection_state.clone();
                    Box(
                        modifier = text_field_size.clip_to_bounds().then_element(core),
                        propagate_min_constraints = true,
                        content = move || {
                            Box(modifier = Modifier.then_element(layout));
                            if enabled && window_and_field_focused && selection_state.is_in_touch_mode() {
                                TextFieldSelectionHandles(selection_state.clone());
                                if !read_only {
                                    TextFieldCursorHandle(selection_state.clone());
                                }
                            }
                        },
                    );
                };
                decorator.decoration(&inner);
            },
        );
    })
}

/// The cursor's handle while it shows (Compose's TextFieldCursorHandle): composed again only when it shows or
/// hides, not as it moves.
#[kompozable]
pub(crate) fn TextFieldCursorHandle(selection_state: Rc<TextFieldSelectionState>) {
    let key = Rc::as_ptr(&selection_state) as usize;
    let s = selection_state.clone();
    let visible = remember_keyed(key, move || derived_state_of(move || s.get_cursor_handle_state(false).visible));
    if visible.get() {
        let provider_state = selection_state.clone();
        let gestures_state = selection_state.clone();
        CursorHandle(
            offset_provider = OffsetProvider::new(move || provider_state.get_cursor_handle_state(true).position),
            modifier = Modifier.pointer_input(key, move |scope: PointerInputScope| {
                let state = gestures_state.clone();
                async move { state.cursor_handle_gestures(scope).await }
            }),
            min_touch_target_size = MinTouchTargetSizeForHandles,
        );
    }
}

/// The selection's handles while they show (Compose's TextFieldSelectionHandles): each composed again only when
/// it shows, hides, turns or crosses, not as it moves.
#[kompozable]
pub(crate) fn TextFieldSelectionHandles(selection_state: Rc<TextFieldSelectionState>) {
    let key = Rc::as_ptr(&selection_state) as usize;
    for is_start_handle in [true, false] {
        kompoz_runtime::key(&is_start_handle, || {
            let s = selection_state.clone();
            let handle_state = remember_keyed(key, move || {
                derived_state_of(move || s.get_selection_handle_state(is_start_handle, false))
            });
            let handle = handle_state.get();
            if handle.visible {
                let provider_state = selection_state.clone();
                let gestures_state = selection_state.clone();
                SelectionHandle(
                    offset_provider = OffsetProvider::new(move || {
                        provider_state.get_selection_handle_state(is_start_handle, true).position
                    }),
                    is_start_handle = is_start_handle,
                    direction = handle.direction,
                    handles_crossed = handle.handles_crossed,
                    modifier = Modifier.pointer_input((key, is_start_handle), move |scope: PointerInputScope| {
                        let state = gestures_state.clone();
                        async move { state.selection_handle_gestures(scope, is_start_handle).await }
                    }),
                    line_height = handle.line_height,
                    min_touch_target_size = MinTouchTargetSizeForHandles,
                );
            }
        });
    }
}

/// The cursor handle's height (Compose Android's CursorHandleHeight).
pub const CursorHandleHeight: Dp = HandleHeight;
/// The cursor handle's width: a selection handle turned 45° (Compose Android's CursorHandleWidth).
pub const CursorHandleWidth: Dp = Dp(25.0 * 2.0 / (1.0 + std::f32::consts::SQRT_2));

/// The cursor's handle (Compose Android's CursorHandle): in a popup centered under the cursor, a selection
/// handle turned 45° to point up, at least `min_touch_target_size` to touch; it tells its [`SelectionHandleInfo`].
#[kompozable]
pub fn CursorHandle(offset_provider: OffsetProvider, modifier: Modifier, min_touch_target_size: DpSize) {
    let info_provider = offset_provider.clone();
    let final_modifier = modifier.semantics(move |config| {
        config.set(
            SelectionHandleInfoKey,
            SelectionHandleInfo {
                handle: Handle::Cursor,
                position: info_provider.provide(),
                anchor: SelectionHandleAnchor::Middle,
                visible: true,
            },
        );
    });
    let handle_color = LocalTextSelectionColors.current().handle_color;
    HandlePopup(
        position_provider = offset_provider,
        handle_reference_point = Alignment::TopCenter,
        content = move || {
            if min_touch_target_size.is_specified() {
                Box(
                    modifier = final_modifier.clone().required_size_in_with(
                        min_touch_target_size.width,
                        min_touch_target_size.height,
                        Dp::Unspecified,
                        Dp::Unspecified,
                    ),
                    content_alignment = Alignment::TopCenter,
                    content = move || DefaultCursorHandle(Modifier, handle_color),
                );
            } else {
                DefaultCursorHandle(final_modifier.clone(), handle_color);
            }
        },
    );
}

/// The cursor handle's icon (Compose Android's DefaultCursorHandle).
#[kompozable]
fn DefaultCursorHandle(modifier: Modifier, handle_color: Color) {
    Spacer(modifier.size_wh(CursorHandleWidth, CursorHandleHeight).then(draw_cursor_handle(handle_color)));
}

/// Draws the cursor handle (Compose Android's `drawCursorHandle`): a selection handle's drop (a circle with the
/// top left quarter square) turned 45° clockwise about its top left, put at the middle of the top.
fn draw_cursor_handle(handle_color: Color) -> Modifier {
    Modifier.draw_with_content(move |s| {
        s.draw_content();
        let radius = s.size().width / 2.0;
        s.translate_with(radius, 0.0, |s| {
            s.rotate_with(45.0, Offset::Zero, |s| {
                s.draw_rect_with(
                    handle_color,
                    Offset::Zero,
                    Size::new(radius, radius),
                    1.0,
                    Fill,
                    None,
                    BlendMode::SrcOver,
                );
                s.draw_circle_with(
                    handle_color,
                    radius,
                    Offset::new(radius, radius),
                    1.0,
                    Fill,
                    None,
                    BlendMode::SrcOver,
                );
            });
        });
    })
}

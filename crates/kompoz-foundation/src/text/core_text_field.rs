//! The value-based text field's engine (Compose's CoreTextField.kt): [`core_text_field`] (Compose's internal
//! `CoreTextField`, what `BasicTextField(value, onValueChange)` is made of), its state ([`LegacyTextFieldState`]),
//! and what it composes — the decoration box (focus, the input session through its own text input service, keys,
//! scrolling, gestures, semantics, autofill's highlight) around the inner field (its size in lines, scrolling, the
//! cursor, the text and selection drawn, its layout), the selection's handles and toolbar, the cursor's handle.

use super::input::internal::core_text_field_semantics_modifier::CoreTextFieldSemanticsModifier;
use super::input::internal::legacy_platform_text_input_service_adapter::{
    LegacyPlatformTextInputServiceAdapter, legacy_text_input_adapter,
};
use super::selection::text_field_selection_manager::{
    TextFieldSelectionHandle, TextFieldSelectionManager, text_field_magnifier,
};
use super::text_delegate::{TextDelegate, update_text_delegate};
use super::text_field_cursor::cursor;
use super::text_field_delegate::{self as delegate, OnValueChange, apply_composition_decoration};
use super::text_field_key_input::text_field_key_input;
use super::text_field_pointer::text_field_pointer;
use super::text_field_scroll::{TextFieldScrollerPosition, text_field_scroll, text_field_scrollable};
use super::text_field_size::{TextFieldSizeModifiers, compute_size_for_default_text};
use super::text_layout_result_proxy::TextLayoutResultProxy;
use super::undo_manager::UndoManager;
use super::validating_offset_mapping::filter_with_validation;
use crate::focusable::FocusableModifiers;
use crate::gestures::Orientation;
use crate::interaction::MutableInteractionSource;
use crate::layout::{Box, SizeModifiers};
use crate::relocation::{BringIntoViewRequester, RelocationModifiers};
use crate::text::CursorHandle;
use crate::text::autofill_highlight::{
    LocalAutofillHighlightBrush, LocalAutofillHighlightColor, autofill_highlight_color, resolve_autofill_highlight,
};
use crate::text::key_event_helpers::cancels_text_selection;
use crate::text::keyboard_actions::{KeyboardActionRunner, KeyboardActions};
use crate::text::modifiers::text_annotated_string::OnTextLayout;
use crate::text::selection::OffsetProvider;
use crate::text::selection::{LocalTextSelectionColors, SimpleLayout};
use kompoz_coroutines::{LaunchedEffect, remember_coroutine_scope, snapshot_flow};
use kompoz_graphics::{Brush, Color, Constraints, Density, Dp, Offset, Rect};
use kompoz_runtime::{
    DisposableEffect, MutableState, mutable_state_of, on_dispose, remember, remember_keyed,
    remember_saveable_keyed_with, without_read_observation,
};
use kompoz_text::input::{EditProcessor, OffsetMapping, TextFieldValue, VisualTransformation};
use kompoz_text::{AnnotatedString, FontFamilyResolver, TextLayoutResult, TextOverflow, TextRange, TextStyle};
use kompoz_ui::focus::{FocusManager, FocusRequester, FocusState};
use kompoz_ui::input::PointerInputScope;
use kompoz_ui::layout::{
    FirstBaseline, IntrinsicMeasurable, IntrinsicMeasureScope, LastBaseline, LayoutCoordinates, Measurable,
    MeasurePolicy, layout_into,
};
use kompoz_ui::platform::{
    ImeAction, ImeOptions, LocalClipboard, LocalDensity, LocalFocusManager, LocalFontFamilyResolver,
    LocalHapticFeedback, LocalSoftwareKeyboardController, LocalTextToolbar, LocalWindowInfo,
    SoftwareKeyboardController,
};
use kompoz_ui::text_input::{TextInputService, TextInputSession};
use kompoz_ui::{MeasureResult, MeasureScope, Modifier};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

// ---------------------------------------------------------------------------------------------------------------
// A state of a value a snapshot state can't hold

/// A value read as a snapshot state (Compose's `mutableStateOf` of what holds handles KompozR's snapshot states
/// can't): its readers are told when it is set.
pub struct ObservedCell<T> {
    value: RefCell<T>,
    version: MutableState<u64>,
}

impl<T: Clone> ObservedCell<T> {
    pub fn new(value: T) -> ObservedCell<T> {
        ObservedCell { value: RefCell::new(value), version: mutable_state_of(0) }
    }

    /// The value, read as a state.
    pub fn get(&self) -> T {
        self.version.get();
        self.value.borrow().clone()
    }

    /// The value, not observed.
    pub fn peek(&self) -> T {
        self.value.borrow().clone()
    }

    /// Sets it, its readers told.
    pub fn set(&self, value: T) {
        *self.value.borrow_mut() = value;
        self.version.set(self.version.peek().wrapping_add(1));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The state

/// What a field's handles show (Compose's HandleState): none (the initial state, and while typing), the selection's
/// handles (after a long press), or the cursor's (after a tap on text).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum HandleState {
    #[default]
    None,
    Selection,
    Cursor,
}

/// A value-based field's state (Compose's LegacyTextFieldState): its text delegate, input session and edit
/// processor, its focus, layout and where it is, what its handles and toolbar show, its keyboard actions, its
/// autofill highlight and the input method's preview highlights.
pub struct LegacyTextFieldState {
    text_delegate: RefCell<TextDelegate>,
    /// Composed again when bumped (Compose's `recomposeScope.invalidate()`: the composition reads it).
    recompose_scope: MutableState<u64>,
    keyboard_controller: Option<SoftwareKeyboardController>,
    /// Applies the input method's commands (Compose's `processor`).
    pub processor: Rc<RefCell<EditProcessor>>,
    input_session: RefCell<Option<Rc<TextInputSession>>>,
    has_focus: MutableState<bool>,
    /// A single-line field's least height, its laid out line's (0 for others; Compose's
    /// `heightForSingleLineField`).
    height_for_single_line_field: MutableState<Dp>,
    layout_coordinates: RefCell<Option<LayoutCoordinates>>,
    layout_result: ObservedCell<Option<Rc<TextLayoutResultProxy>>>,
    /// The value's text before it was transformed (Compose's `untransformedText`).
    untransformed_text: RefCell<Option<AnnotatedString>>,
    handle_state: MutableState<HandleState>,
    show_floating_toolbar: MutableState<bool>,
    show_selection_handle_start: MutableState<bool>,
    show_selection_handle_end: MutableState<bool>,
    show_cursor_handle: MutableState<bool>,
    is_layout_result_stale: Cell<bool>,
    is_in_touch_mode: MutableState<bool>,
    keyboard_action_runner: RefCell<KeyboardActionRunner>,
    autofill_highlight_on: MutableState<bool>,
    just_autofilled: MutableState<bool>,
    /// The app's callback (Compose's `onValueChangeOriginal`): through [`on_value_change`](Self::on_value_change).
    on_value_change_original: RefCell<OnValueChange>,
    selection_background_color: Cell<Color>,
    selection_preview_highlight_range: MutableState<TextRange>,
    deletion_preview_highlight_range: MutableState<TextRange>,
    this: RefCell<Weak<LegacyTextFieldState>>,
}

impl LegacyTextFieldState {
    /// A state laying out with `text_delegate`, composing again through `recompose_scope`.
    pub fn new(
        text_delegate: TextDelegate,
        recompose_scope: MutableState<u64>,
        keyboard_controller: Option<SoftwareKeyboardController>,
    ) -> Rc<LegacyTextFieldState> {
        let state = Rc::new(LegacyTextFieldState {
            text_delegate: RefCell::new(text_delegate),
            recompose_scope,
            keyboard_controller: keyboard_controller.clone(),
            processor: Rc::new(RefCell::new(EditProcessor::new())),
            input_session: RefCell::new(None),
            has_focus: mutable_state_of(false),
            height_for_single_line_field: mutable_state_of(Dp(0.0)),
            layout_coordinates: RefCell::new(None),
            layout_result: ObservedCell::new(None),
            untransformed_text: RefCell::new(None),
            handle_state: mutable_state_of(HandleState::None),
            show_floating_toolbar: mutable_state_of(false),
            show_selection_handle_start: mutable_state_of(false),
            show_selection_handle_end: mutable_state_of(false),
            show_cursor_handle: mutable_state_of(false),
            is_layout_result_stale: Cell::new(true),
            is_in_touch_mode: mutable_state_of(true),
            keyboard_action_runner: RefCell::new(KeyboardActionRunner::new(
                keyboard_controller,
                KeyboardActions::Default,
                FocusManager::default(),
            )),
            autofill_highlight_on: mutable_state_of(false),
            just_autofilled: mutable_state_of(false),
            on_value_change_original: RefCell::new(Rc::new(|_| {})),
            selection_background_color: Cell::new(Color::Unspecified),
            selection_preview_highlight_range: mutable_state_of(TextRange::Zero),
            deletion_preview_highlight_range: mutable_state_of(TextRange::Zero),
            this: RefCell::new(Weak::new()),
        });
        *state.this.borrow_mut() = Rc::downgrade(&state);
        state
    }

    pub fn text_delegate(&self) -> TextDelegate {
        self.text_delegate.borrow().clone()
    }

    pub fn keyboard_controller(&self) -> Option<&SoftwareKeyboardController> {
        self.keyboard_controller.as_ref()
    }

    /// The open input session, if any (Compose's `inputSession`).
    pub fn input_session(&self) -> Option<Rc<TextInputSession>> {
        self.input_session.borrow().clone()
    }

    pub fn set_input_session(&self, session: Option<Rc<TextInputSession>>) {
        *self.input_session.borrow_mut() = session;
    }

    /// Whether it has the focus (a state: Compose's `hasFocus`).
    pub fn has_focus(&self) -> bool {
        self.has_focus.get()
    }

    pub fn set_has_focus(&self, has_focus: bool) {
        self.has_focus.set(has_focus);
    }

    pub fn height_for_single_line_field(&self) -> Dp {
        self.height_for_single_line_field.get()
    }

    pub fn set_height_for_single_line_field(&self, height: Dp) {
        self.height_for_single_line_field.set(height);
    }

    /// The inner field's coordinates while attached (Compose's `layoutCoordinates`).
    pub fn layout_coordinates(&self) -> Option<LayoutCoordinates> {
        self.layout_coordinates.borrow().clone().filter(|c| c.is_attached())
    }

    pub fn set_layout_coordinates(&self, coordinates: Option<LayoutCoordinates>) {
        *self.layout_coordinates.borrow_mut() = coordinates;
    }

    /// The text's layout with where the field is (a state: Compose's `layoutResult`).
    pub fn layout_result(&self) -> Option<Rc<TextLayoutResultProxy>> {
        self.layout_result.get()
    }

    /// The layout, not observed.
    pub fn peek_layout_result(&self) -> Option<Rc<TextLayoutResultProxy>> {
        self.layout_result.peek()
    }

    /// Sets the layout: no longer stale.
    pub fn set_layout_result(&self, value: Option<Rc<TextLayoutResultProxy>>) {
        self.layout_result.set(value);
        self.is_layout_result_stale.set(false);
    }

    /// Whether the layout lags behind the text delegate (Compose's `isLayoutResultStale`).
    pub fn is_layout_result_stale(&self) -> bool {
        self.is_layout_result_stale.get()
    }

    pub fn untransformed_text(&self) -> Option<AnnotatedString> {
        self.untransformed_text.borrow().clone()
    }

    pub fn handle_state(&self) -> HandleState {
        self.handle_state.get()
    }

    pub fn set_handle_state(&self, handle_state: HandleState) {
        self.handle_state.set(handle_state);
    }

    pub fn peek_handle_state(&self) -> HandleState {
        self.handle_state.peek()
    }

    pub fn show_floating_toolbar(&self) -> bool {
        self.show_floating_toolbar.get()
    }

    pub fn set_show_floating_toolbar(&self, show: bool) {
        self.show_floating_toolbar.set(show);
    }

    pub fn show_selection_handle_start(&self) -> bool {
        self.show_selection_handle_start.get()
    }

    pub fn set_show_selection_handle_start(&self, show: bool) {
        self.show_selection_handle_start.set(show);
    }

    pub fn show_selection_handle_end(&self) -> bool {
        self.show_selection_handle_end.get()
    }

    pub fn set_show_selection_handle_end(&self, show: bool) {
        self.show_selection_handle_end.set(show);
    }

    pub fn show_cursor_handle(&self) -> bool {
        self.show_cursor_handle.get()
    }

    pub fn set_show_cursor_handle(&self, show: bool) {
        self.show_cursor_handle.set(show);
    }

    /// Whether touch (not a mouse) is what used it last (a state: Compose's `isInTouchMode`).
    pub fn is_in_touch_mode(&self) -> bool {
        self.is_in_touch_mode.get()
    }

    pub fn set_is_in_touch_mode(&self, touch: bool) {
        self.is_in_touch_mode.set(touch);
    }

    pub fn autofill_highlight_on(&self) -> bool {
        self.autofill_highlight_on.get()
    }

    pub fn set_autofill_highlight_on(&self, on: bool) {
        self.autofill_highlight_on.set(on);
    }

    pub fn just_autofilled(&self) -> bool {
        self.just_autofilled.get()
    }

    pub fn set_just_autofilled(&self, on: bool) {
        self.just_autofilled.set(on);
    }

    pub fn selection_background_color(&self) -> Color {
        self.selection_background_color.get()
    }

    pub fn selection_preview_highlight_range(&self) -> TextRange {
        self.selection_preview_highlight_range.get()
    }

    pub fn set_selection_preview_highlight_range(&self, range: TextRange) {
        self.selection_preview_highlight_range.set(range);
    }

    pub fn deletion_preview_highlight_range(&self) -> TextRange {
        self.deletion_preview_highlight_range.get()
    }

    pub fn set_deletion_preview_highlight_range(&self, range: TextRange) {
        self.deletion_preview_highlight_range.set(range);
    }

    /// Whether an input method's preview highlight shows (Compose's `hasHighlight`).
    pub fn has_highlight(&self) -> bool {
        !self.selection_preview_highlight_range.get().collapsed()
            || !self.deletion_preview_highlight_range.get().collapsed()
    }

    /// The field's value changed (Compose's `onValueChange`): a new text leaves the handles and the autofill
    /// highlight (but for the autofill's own change), the preview highlights are cleared, the app told, and the
    /// field composed again — the app may keep its value, which the input method must then be told again.
    pub fn on_value_change(&self, value: TextFieldValue) {
        let text_changed = self.untransformed_text.borrow().as_ref().map(|t| t.text()) != Some(value.text());
        if text_changed {
            self.handle_state.set(HandleState::None);
            if self.just_autofilled.peek() {
                self.just_autofilled.set(false);
            } else {
                self.autofill_highlight_on.set(false);
            }
        }
        self.selection_preview_highlight_range.set(TextRange::Zero);
        self.deletion_preview_highlight_range.set(TextRange::Zero);
        let original = self.on_value_change_original.borrow().clone();
        original(value);
        self.recompose_scope.set(self.recompose_scope.peek().wrapping_add(1));
    }

    /// [`on_value_change`](Self::on_value_change) as a callback (holding the state weakly).
    pub fn on_value_change_callback(&self) -> OnValueChange {
        let weak = self.this.borrow().clone();
        Rc::new(move |value| {
            if let Some(s) = weak.upgrade() {
                s.on_value_change(value);
            }
        })
    }

    /// The input method's action key (Compose's `onImeActionPerformed`): the app's action, else the default one.
    pub fn on_ime_action_performed(&self, ime_action: ImeAction) {
        self.on_ime_action_performed_with_result(ime_action);
    }

    /// The same, whether something ran (Compose's `onImeActionPerformedWithResult`).
    pub fn on_ime_action_performed_with_result(&self, ime_action: ImeAction) -> bool {
        // (the runner's actions may compose and update it: not borrowed while they run)
        let runner = {
            let r = self.keyboard_action_runner.borrow();
            KeyboardActionRunner::new(
                self.keyboard_controller.clone(),
                r.keyboard_actions.clone(),
                r.focus_manager.clone(),
            )
        };
        runner.run_action(ime_action)
    }

    /// [`on_ime_action_performed`](Self::on_ime_action_performed) as a callback (holding the state weakly).
    pub fn on_ime_action_performed_callback(&self) -> Rc<dyn Fn(ImeAction)> {
        let weak = self.this.borrow().clone();
        Rc::new(move |action| {
            if let Some(s) = weak.upgrade() {
                s.on_ime_action_performed(action);
            }
        })
    }

    /// Takes what the composition gives (Compose's `update`): the callback, colors, keyboard actions, the value's
    /// text, and a new text delegate if what lays the shown text out changed (the layout then stale).
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &self,
        untransformed_text: &AnnotatedString,
        visual_text: &AnnotatedString,
        text_style: &TextStyle,
        soft_wrap: bool,
        density: Density,
        font_family_resolver: &Rc<dyn FontFamilyResolver>,
        on_value_change: OnValueChange,
        keyboard_actions: KeyboardActions,
        focus_manager: FocusManager,
        selection_background_color: Color,
    ) {
        *self.on_value_change_original.borrow_mut() = on_value_change;
        self.selection_background_color.set(selection_background_color);
        {
            let mut runner = self.keyboard_action_runner.borrow_mut();
            runner.keyboard_actions = keyboard_actions;
            runner.focus_manager = focus_manager;
        }
        *self.untransformed_text.borrow_mut() = Some(untransformed_text.clone());
        let current = self.text_delegate();
        let new_text_delegate = update_text_delegate(
            &current,
            visual_text,
            text_style,
            density,
            font_family_resolver,
            soft_wrap,
            TextOverflow::Clip,
            usize::MAX,
            1,
            &[],
        );
        if !current.same_as(&new_text_delegate) {
            self.is_layout_result_stale.set(true);
        }
        *self.text_delegate.borrow_mut() = new_text_delegate;
    }
}

/// A tap focuses the field, or shows the keyboard again if it has the focus already and `allow_keyboard`
/// (Compose's `requestFocusAndShowKeyboardIfNeeded`).
pub fn request_focus_and_show_keyboard_if_needed(
    state: &LegacyTextFieldState,
    focus_requester: &FocusRequester,
    allow_keyboard: bool,
) {
    if !state.has_focus.peek() {
        focus_requester.request_focus_with(kompoz_ui::focus::FocusDirection::Enter);
    } else if allow_keyboard && let Some(keyboard) = state.keyboard_controller() {
        keyboard.show();
    }
}

/// Starts the field's input session (Compose's private `startInputSession`).
fn start_input_session(
    text_input_service: &TextInputService,
    state: &LegacyTextFieldState,
    value: &TextFieldValue,
    ime_options: &ImeOptions,
    offset_mapping: &OffsetMapping,
) {
    let session = delegate::on_focus(
        text_input_service,
        value,
        &state.processor,
        ime_options,
        &state.on_value_change_callback(),
        state.on_ime_action_performed_callback(),
    );
    state.set_input_session(Some(session));
    notify_focused_rect(state, value, offset_mapping);
}

/// Ends the field's input session (Compose's private `endInputSession`).
fn end_input_session(state: &LegacyTextFieldState) {
    if let Some(session) = state.input_session() {
        delegate::on_blur(&session, &state.processor, &state.on_value_change_callback());
    }
    state.set_input_session(None);
}

/// Brings the selection's end into view (Compose's `bringSelectionEndIntoView`): the box of its character (the one
/// before it at the end, a pixel wide line of the default text's height in empty text).
pub async fn bring_selection_end_into_view(
    requester: &BringIntoViewRequester,
    value: &TextFieldValue,
    text_delegate: &TextDelegate,
    text_layout_result: &TextLayoutResult,
    offset_mapping: &OffsetMapping,
) {
    let end = offset_mapping.original_to_transformed(value.selection().max());
    let text = text_layout_result.layout_input.text.text();
    let bounds = if end < text.len() {
        text_layout_result.get_bounding_box(end)
    } else if end != 0 {
        text_layout_result.get_bounding_box(delegate::previous_char(text, end))
    } else {
        let size = compute_size_for_default_text(
            text_delegate.style(),
            text_delegate.density(),
            text_delegate.font_family_resolver(),
            1,
        );
        Rect::new(0.0, 0.0, 1.0, size.height as f32)
    };
    requester.bring_into_view_rect(Some(bounds)).await;
}

/// Tells the session where the caret is (Compose's private `notifyFocusedRect`): unobserved (it is called after
/// layouts and placements already).
fn notify_focused_rect(state: &LegacyTextFieldState, value: &TextFieldValue, offset_mapping: &OffsetMapping) {
    without_read_observation(|| {
        let Some(layout_result) = state.peek_layout_result() else { return };
        let Some(input_session) = state.input_session() else { return };
        let Some(layout_coordinates) = state.layout_coordinates() else { return };
        delegate::notify_focused_rect(
            value,
            &state.text_delegate(),
            &layout_result.value,
            &layout_coordinates,
            &input_session,
            state.has_focus.peek(),
            offset_mapping,
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// CoreTextField

/// What a field's decoration box is (Compose's `decorationBox: @Composable (innerTextField: @Composable () -> Unit)
/// -> Unit`): it composes around the inner field, which it calls exactly once.
pub type DecorationBox = Rc<dyn Fn(&dyn Fn())>;

/// Everything CoreTextField takes (Compose's internal CoreTextField's parameters).
#[derive(Clone)]
pub struct CoreTextFieldArgs {
    pub value: TextFieldValue,
    pub on_value_change: OnValueChange,
    pub modifier: Modifier,
    pub text_style: TextStyle,
    pub visual_transformation: VisualTransformation,
    pub on_text_layout: Option<OnTextLayout>,
    pub interaction_source: Option<MutableInteractionSource>,
    pub cursor_brush: Brush,
    pub soft_wrap: bool,
    pub max_lines: i32,
    pub min_lines: i32,
    pub ime_options: ImeOptions,
    pub keyboard_actions: KeyboardActions,
    pub enabled: bool,
    pub read_only: bool,
    pub decoration_box: Option<DecorationBox>,
    pub text_scroller_position: Option<TextFieldScrollerPosition>,
}

/// The value-based field (Compose's internal `CoreTextField`): `value` shown through `visual_transformation`,
/// what the user does to it given to `on_value_change` (which the app feeds back as the next `value`).
#[track_caller]
pub fn core_text_field(args: CoreTextFieldArgs) {
    kompoz_runtime::group(|| {
        kompoz_runtime::kompozable_block!({
            let CoreTextFieldArgs {
                value,
                on_value_change,
                modifier,
                text_style,
                visual_transformation,
                on_text_layout,
                interaction_source,
                cursor_brush,
                soft_wrap,
                max_lines,
                min_lines,
                ime_options,
                keyboard_actions,
                enabled,
                read_only,
                decoration_box,
                text_scroller_position,
            } = args;
            let focus_requester = remember(FocusRequester::new);
            let legacy_text_input_service_adapter = remember(LegacyPlatformTextInputServiceAdapter::new);
            let adapter = legacy_text_input_service_adapter.clone();
            let text_input_service: TextInputService = remember(move || {
                TextInputService::with_platform_service(
                    adapter as Rc<dyn kompoz_ui::text_input::PlatformTextInputService>,
                )
            });

            // Composition locals
            let density = LocalDensity.current();
            let font_family_resolver = LocalFontFamilyResolver.current();
            let selection_background_color = LocalTextSelectionColors.current().background_color;
            let focus_manager = LocalFocusManager.current();
            let window_info = LocalWindowInfo.current();
            let keyboard_controller = LocalSoftwareKeyboardController.current();

            // Scroll state
            let single_line = max_lines == 1 && !soft_wrap && ime_options.single_line;
            let orientation = if single_line { Orientation::Horizontal } else { Orientation::Vertical };
            let saver = TextFieldScrollerPosition::saver();
            let own_scroller_position = remember_saveable_keyed_with(orientation, &saver, move || {
                TextFieldScrollerPosition::new(orientation, 0.0)
            });
            let scroller_position = text_scroller_position.unwrap_or(own_scroller_position);
            if scroller_position.orientation() != orientation {
                panic!(
                    "Mismatching scroller orientation; {}",
                    if orientation == Orientation::Vertical {
                        "only single-line, non-wrap text fields can scroll horizontally"
                    } else {
                        "single-line, non-wrap text fields can only scroll horizontally"
                    }
                );
            }

            // State
            let (v, vt) = (value.clone(), visual_transformation.clone());
            let transformed_text = remember_keyed((value.clone(), visual_transformation.clone()), move || {
                let transformed = filter_with_validation(&vt, v.annotated_string());
                match v.composition() {
                    Some(composition) => apply_composition_decoration(composition, &transformed),
                    None => transformed,
                }
            });
            let visual_text = transformed_text.text.clone();
            let offset_mapping = transformed_text.offset_mapping.clone();

            // (Compose's currentRecomposeScope: the field composed again when the app didn't take a value the input
            // method and the processor think it has)
            let recompose_scope = remember(|| mutable_state_of(0u64));
            recompose_scope.get();
            let (vt_text, style, resolver, scope_state, controller) = (
                visual_text.clone(),
                text_style.clone(),
                font_family_resolver.clone(),
                recompose_scope.clone(),
                keyboard_controller.clone(),
            );
            let state = remember_keyed(keyboard_controller.clone(), move || {
                LegacyTextFieldState::new(
                    TextDelegate::with_defaults(vt_text, style, soft_wrap, density, resolver),
                    scope_state,
                    Some(controller),
                )
            });
            state.update(
                value.annotated_string(),
                &visual_text,
                &text_style,
                soft_wrap,
                density,
                &font_family_resolver,
                on_value_change.clone(),
                keyboard_actions.clone(),
                focus_manager.clone(),
                selection_background_color,
            );

            // The processor told the value each composition (and the session, where it differs).
            let (old_value, new_value) = state.processor.borrow_mut().reset(&value);
            if let Some(session) = state.input_session() {
                session.update_state_from(Some(&old_value), &new_value);
            }

            let undo_manager = remember(UndoManager::default);
            undo_manager.snapshot_if_needed(&value);

            let coroutine_scope = remember_coroutine_scope();
            let bring_into_view_requester = remember(BringIntoViewRequester::default);

            let um = undo_manager.clone();
            let manager = remember(move || TextFieldSelectionManager::new(Some(um)));
            manager.set_offset_mapping(offset_mapping.clone());
            manager.set_visual_transformation(visual_transformation.clone());
            manager.set_on_value_change(state.on_value_change_callback());
            manager.set_state(Some(state.clone()));
            manager.set_value(value.clone());
            manager.set_clipboard(Some(LocalClipboard.current()));
            manager.set_coroutine_scope(Some(coroutine_scope.clone()));
            manager.set_text_toolbar(Some(LocalTextToolbar.current()));
            manager.set_haptic_feed_back(Some(LocalHapticFeedback.current()));
            manager.set_focus_requester(Some(focus_requester.clone()));
            manager.set_editable(!read_only);
            manager.set_enabled(enabled);
            // (smart selection's PlatformSelectionBehaviors: Android's, none on the desktop; Compose desktop's clipboard
            // events handler: none)

            // Focus
            let focus_modifier = {
                let (state, manager, service) = (state.clone(), manager.clone(), text_input_service.clone());
                let (value, ime_options, offset_mapping) = (value.clone(), ime_options.clone(), offset_mapping.clone());
                let (scope, requester) = (coroutine_scope.clone(), bring_into_view_requester.clone());
                text_field_focus_modifier(
                    enabled,
                    &focus_requester,
                    interaction_source.clone(),
                    move |it: FocusState| {
                        if state.has_focus.peek() == it.is_focused() {
                            return;
                        }
                        state.set_has_focus(it.is_focused());
                        if state.has_focus.peek() && enabled && !read_only {
                            start_input_session(&service, &state, &value, &ime_options, &offset_mapping);
                        } else {
                            end_input_session(&state);
                        }
                        // The cursor brought into view too (the focusable brings the decoration box).
                        if it.is_focused()
                            && let Some(layout_result) = state.peek_layout_result()
                        {
                            let (requester, value, text_delegate, mapping) =
                                (requester.clone(), value.clone(), state.text_delegate(), offset_mapping.clone());
                            scope.launch(async move {
                                bring_selection_end_into_view(
                                    &requester,
                                    &value,
                                    &text_delegate,
                                    &layout_result.value,
                                    &mapping,
                                )
                                .await;
                            });
                        }
                        if !it.is_focused() {
                            manager.deselect(None);
                        }
                    },
                )
            };

            // The keyboard hidden when made disabled or read-only while focused.
            // (Compose's `rememberUpdatedState`: a snapshot state the flow follows — KompozR's remember_updated_state
            // holds its value outside the snapshot system)
            let writeable = remember(|| mutable_state_of(enabled && !read_only));
            writeable.set(enabled && !read_only);
            {
                let (state, manager, service, ime_options) =
                    (state.clone(), manager.clone(), text_input_service.clone(), ime_options.clone());
                LaunchedEffect((), async move {
                    // (Kotlin's `finally`: the session ended when the field leaves the composition)
                    struct EndOnDrop(Rc<LegacyTextFieldState>);
                    impl Drop for EndOnDrop {
                        fn drop(&mut self) {
                            end_input_session(&self.0);
                        }
                    }
                    let _end = EndOnDrop(state.clone());
                    snapshot_flow(move || writeable.get())
                        .collect(move |writeable| {
                            if writeable && state.has_focus.peek() {
                                let (value, mapping) = (manager.value_peek(), manager.offset_mapping());
                                start_input_session(&service, &state, &value, &ime_options, &mapping);
                            } else {
                                end_input_session(&state);
                            }
                        })
                        .await;
                });
            }

            let pointer_modifier = text_field_pointer(
                Modifier,
                &manager,
                enabled,
                interaction_source.clone(),
                &state,
                &focus_requester,
                read_only,
                &offset_mapping,
            );

            let draw_modifier = text_field_draw(&state, &value, &offset_mapping);

            let on_positioned_modifier = {
                let (state, manager, value, offset_mapping, window_info) =
                    (state.clone(), manager.clone(), value.clone(), offset_mapping.clone(), window_info.clone());
                Modifier.on_globally_positioned(move |it: &LayoutCoordinates| {
                    state.set_layout_coordinates(Some(it.clone()));
                    if let Some(layout_result) = state.peek_layout_result() {
                        layout_result.set_inner_text_field_coordinates(Some(it.clone()));
                    }
                    if enabled {
                        if state.handle_state.peek() == HandleState::Selection {
                            if state.show_floating_toolbar.peek() && window_info.is_window_focused() {
                                manager.show_selection_toolbar();
                            } else {
                                manager.hide_selection_toolbar();
                            }
                            state.set_show_selection_handle_start(manager.is_selection_handle_in_visible_bound(true));
                            state.set_show_selection_handle_end(manager.is_selection_handle_in_visible_bound(false));
                            state.set_show_cursor_handle(value.selection().collapsed());
                        } else if state.handle_state.peek() == HandleState::Cursor {
                            state.set_show_cursor_handle(manager.is_selection_handle_in_visible_bound(true));
                        }
                        notify_focused_rect(&state, &value, &offset_mapping);
                        if let Some(layout_result) = state.peek_layout_result()
                            && let Some(input_session) = state.input_session()
                            && state.has_focus.peek()
                        {
                            delegate::update_text_layout_result(
                                &input_session,
                                &value,
                                &offset_mapping,
                                &layout_result,
                            );
                        }
                    }
                })
            };

            let is_password = visual_transformation.is_password();
            let semantics_modifier = Modifier.then_element(CoreTextFieldSemanticsModifier {
                transformed_text: transformed_text.clone(),
                value: value.clone(),
                state: state.clone(),
                read_only,
                enabled,
                is_password,
                offset_mapping: offset_mapping.clone(),
                manager: manager.clone(),
                ime_options: ime_options.clone(),
                focus_requester: focus_requester.clone(),
            });

            let show_cursor = enabled && !read_only && window_info.is_window_focused() && !state.has_highlight();
            let cursor_modifier = cursor(Modifier, &state, &value, &offset_mapping, &cursor_brush, show_cursor);

            {
                let m = manager.clone();
                DisposableEffect(Rc::as_ptr(&manager) as usize, move || on_dispose(move || m.hide_selection_toolbar()));
            }

            {
                let (state, service, value) = (state.clone(), text_input_service.clone(), value.clone());
                let options = ime_options.clone();
                DisposableEffect(ime_options.clone(), move || {
                    if state.has_focus.peek() {
                        let session = delegate::restart_input(
                            &service,
                            &value,
                            &state.processor,
                            &options,
                            &state.on_value_change_callback(),
                            state.on_ime_action_performed_callback(),
                        );
                        state.set_input_session(Some(session));
                    }
                    on_dispose(|| {})
                });
            }

            let text_key_input_modifier = text_field_key_input(
                Modifier,
                &state,
                &manager,
                &value,
                state.on_value_change_callback(),
                !read_only,
                max_lines == 1,
                &offset_mapping,
                &undo_manager,
                ime_options.ime_action,
            );

            // (stylus handwriting: stage 15's)

            let autofill_highlight_brush = resolve_autofill_highlight(
                LocalAutofillHighlightBrush.current(),
                LocalAutofillHighlightColor.current(),
                autofill_highlight_color(),
            );
            let draw_decoration_modifier = {
                let state = state.clone();
                // The autofill highlight over the content (and over a Material background).
                Modifier.draw_with_content(move |scope| {
                    scope.draw_content();
                    if (state.autofill_highlight_on() || state.just_autofilled())
                        && autofill_highlight_brush.solid_color().is_none_or(|c| c.is_specified())
                    {
                        let size = scope.size();
                        scope.draw_rect_with(
                            autofill_highlight_brush.clone(),
                            Offset::Zero,
                            size,
                            1.0,
                            kompoz_ui::draw::DrawStyle::Fill,
                            None,
                            kompoz_ui::draw::DefaultBlendMode,
                        );
                    }
                })
            };

            // (the desktop's text field overscroll: none)
            let overscroll_effect = None;

            let decoration_box_coordinates = {
                let state = state.clone();
                move |it: &LayoutCoordinates| {
                    if let Some(layout_result) = state.peek_layout_result() {
                        layout_result.set_decoration_box_coordinates(Some(it.clone()));
                    }
                }
            };
            let decoration_box_modifier = text_field_scrollable(
                legacy_text_input_adapter(
                    modifier.then(draw_decoration_modifier),
                    &legacy_text_input_service_adapter,
                    &state,
                    &manager,
                )
                .then(focus_modifier)
                // (the desktop's D-pad interception: none)
                .then(preview_key_event_to_deselect_on_back(&state, &manager))
                .then(text_key_input_modifier),
                &scroller_position,
                interaction_source.clone(),
                enabled,
                overscroll_effect.clone(),
            )
            .then(pointer_modifier)
            .then(semantics_modifier)
            .on_globally_positioned(decoration_box_coordinates);
            // (the new text context menu's components: Compose's flag, off on the desktop; the field's overlay: none)

            let show_handle_and_magnifier =
                enabled && state.has_focus() && state.is_in_touch_mode() && window_info.is_window_focused();
            let magnifier_modifier =
                if show_handle_and_magnifier { text_field_magnifier(Modifier, &manager) } else { Modifier };

            let decoration_box: DecorationBox = decoration_box.unwrap_or_else(|| Rc::new(|inner: &dyn Fn()| inner()));
            let core = CoreInner {
                state: state.clone(),
                manager: manager.clone(),
                text_style: text_style.clone(),
                min_lines,
                max_lines,
                soft_wrap,
                scroller_position: scroller_position.clone(),
                value: value.clone(),
                visual_transformation: visual_transformation.clone(),
                overscroll_effect,
                cursor_modifier,
                draw_modifier,
                on_positioned_modifier,
                magnifier_modifier,
                bring_into_view_requester: bring_into_view_requester.clone(),
                window_info: window_info.clone(),
                coroutine_scope: coroutine_scope.clone(),
                on_text_layout: on_text_layout.clone(),
                offset_mapping: offset_mapping.clone(),
                density,
                show_handle_and_magnifier,
                read_only,
            };
            CoreTextFieldRootBox(decoration_box_modifier, move || {
                let core = core.clone();
                decoration_box(&move || core.compose());
            });
        })
    })
}

/// The decoration box's root (Compose's CoreTextFieldRootBox): a box passing its min constraints on.
#[track_caller]
#[allow(non_snake_case)]
fn CoreTextFieldRootBox(modifier: Modifier, content: impl Fn() + 'static) {
    kompoz_runtime::group(|| {
        kompoz_runtime::kompozable_block!({
            // (the desktop's text context menu area around the content: WP6's ContextMenuArea)
            Box(modifier = modifier, propagate_min_constraints = true, content = content);
        })
    })
}

/// What the inner field is composed from.
#[derive(Clone)]
struct CoreInner {
    state: Rc<LegacyTextFieldState>,
    manager: Rc<TextFieldSelectionManager>,
    text_style: TextStyle,
    min_lines: i32,
    max_lines: i32,
    soft_wrap: bool,
    scroller_position: TextFieldScrollerPosition,
    value: TextFieldValue,
    visual_transformation: VisualTransformation,
    overscroll_effect: Option<Rc<dyn crate::overscroll::OverscrollEffect>>,
    cursor_modifier: Modifier,
    draw_modifier: Modifier,
    on_positioned_modifier: Modifier,
    magnifier_modifier: Modifier,
    bring_into_view_requester: BringIntoViewRequester,
    window_info: kompoz_ui::platform::WindowInfo,
    coroutine_scope: kompoz_coroutines::CoroutineScope,
    on_text_layout: Option<OnTextLayout>,
    offset_mapping: OffsetMapping,
    density: Density,
    show_handle_and_magnifier: bool,
    read_only: bool,
}

impl CoreInner {
    /// The inner field (Compose's `innerTextField`): its size (a single line's least height, its lines), its
    /// scrolling, the cursor, the text drawn, its least size, where it is, the magnifier, the bring-into-view
    /// requester; the text's layout, the handles and the toolbar.
    #[track_caller]
    fn compose(&self) {
        kompoz_runtime::group(|| {
            kompoz_runtime::kompozable_block!({
                let s = self.clone();
                // A single line at least as tall as its laid out line, not to cut a taller character; its lines.
                let height = s.state.height_for_single_line_field();
                let sizing_modifier = Modifier
                    .height_in(height, if height == Dp(0.0) { Dp::Unspecified } else { height })
                    .height_in_lines(&s.text_style, s.min_lines, s.max_lines, s.soft_wrap);
                let layout_state = s.state.clone();
                let provider: super::text_field_scroll::TextLayoutResultProvider =
                    Rc::new(move || layout_state.peek_layout_result());
                let core_text_field_modifier = text_field_scroll(
                    sizing_modifier,
                    &s.scroller_position,
                    &s.value,
                    &s.visual_transformation,
                    s.overscroll_effect.clone(),
                    provider,
                )
                .then(s.cursor_modifier.clone())
                .then(s.draw_modifier.clone())
                .text_field_min_size(&s.text_style)
                .then(s.on_positioned_modifier.clone())
                .then(s.magnifier_modifier.clone())
                .bring_into_view_requester(&s.bring_into_view_requester);

                let inner = s.clone();
                SimpleLayout(core_text_field_modifier, move || {
                    let policy = Rc::new(CoreTextFieldMeasurePolicy(inner.clone())) as Rc<dyn MeasurePolicy>;
                    layout_into(Modifier, policy, false, || {});
                    let state = &inner.state;
                    let show = state.handle_state() != HandleState::None
                        && state.layout_coordinates().is_some()
                        && inner.show_handle_and_magnifier;
                    SelectionToolbarAndHandles(inner.manager.clone(), show);
                    if state.handle_state() == HandleState::Cursor
                        && !inner.read_only
                        && inner.show_handle_and_magnifier
                    {
                        TextFieldCursorHandle(inner.manager.clone());
                    }
                });
            })
        })
    }
}

/// The inner field's text layout (Compose's CoreTextField measure policy): the text laid out by the text delegate
/// (the last layout reused where it can be); a new layout made the state's, told to the app, the cursor brought
/// into view where the text changed while focused; a single line's least height noted.
struct CoreTextFieldMeasurePolicy(CoreInner);

impl MeasurePolicy for CoreTextFieldMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        _measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let c = &self.0;
        let state = &c.state;
        let prev_proxy = without_read_observation(|| state.peek_layout_result());
        let prev_result = prev_proxy.as_ref().map(|p| p.value.clone());
        let (width, height, result) =
            delegate::layout(&state.text_delegate(), constraints, scope.layout_direction(), prev_result.as_ref());
        // (read here: a font resolved later measures it again)
        result.multi_paragraph.intrinsics().has_stale_resolved_fonts();
        if prev_result.as_ref() != Some(&result) {
            state.set_layout_result(Some(Rc::new(TextLayoutResultProxy::new(
                result.clone(),
                prev_proxy.as_ref().and_then(|p| p.decoration_box_coordinates()),
            ))));
            let manager = &c.manager;
            let show_cursor =
                manager.enabled() && manager.editable() && c.window_info.is_window_focused() && !state.has_highlight();
            let text_changed =
                prev_result.as_ref().map(|p| p.layout_input.text.text()) != Some(result.layout_input.text.text());
            if show_cursor && state.has_focus.peek() && text_changed {
                let (manager, requester) = (manager.clone(), c.bring_into_view_requester.clone());
                c.coroutine_scope.launch(async move {
                    let cursor_rect = manager.get_cursor_rect();
                    requester.bring_into_view_rect(Some(cursor_rect)).await;
                });
            }
            if let Some(on_text_layout) = &c.on_text_layout {
                on_text_layout(&result);
            }
            notify_focused_rect(state, &c.value, &c.offset_mapping);
        }
        // A single line at least as tall as its first line (a taller character than the reference ones not cut).
        let single_line_height = if c.max_lines == 1 { (result.get_line_bottom(0)).ceil() as i32 } else { 0 };
        state.set_height_for_single_line_field(c.density.to_dp(single_line_height as f32));
        let first = super::modifiers::layout_utils::round_to_int(result.first_baseline());
        let last = super::modifiers::layout_utils::round_to_int(result.last_baseline());
        MeasureResult::new(width, height, vec![(FirstBaseline.into(), first), (LastBaseline.into(), last)], |_| {})
    }

    fn max_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        _measurables: &[&dyn IntrinsicMeasurable],
        _height: i32,
    ) -> i32 {
        let text_delegate = self.0.state.text_delegate();
        text_delegate.layout_intrinsics(scope.layout_direction());
        text_delegate.max_intrinsic_width()
    }
}

/// The selection's handles and the toolbar (Compose's private SelectionToolbarAndHandles): the handles of a
/// selection while they are in view (the layout not stale); a toolbar a new character took the field out of.
#[track_caller]
#[allow(non_snake_case)]
fn SelectionToolbarAndHandles(manager: Rc<TextFieldSelectionManager>, show: bool) {
    kompoz_runtime::group(|| {
        kompoz_runtime::kompozable_block!({
            if show {
                let state = manager.state();
                let layout = state
                    .as_ref()
                    .filter(|s| !s.is_layout_result_stale())
                    .and_then(|s| s.layout_result())
                    .map(|l| l.value.clone());
                if let (Some(state), Some(layout)) = (state, layout) {
                    let value = manager.value();
                    if !value.selection().collapsed() {
                        let mapping = manager.offset_mapping();
                        let start_offset = mapping.original_to_transformed(value.selection().start);
                        let end_offset = mapping.original_to_transformed(value.selection().end);
                        let start_direction = layout.get_bidi_run_direction(start_offset);
                        let shown = layout.layout_input.text.text();
                        let end_direction = layout.get_bidi_run_direction(delegate::previous_char(shown, end_offset));
                        if state.show_selection_handle_start() {
                            TextFieldSelectionHandle(true, start_direction, manager.clone());
                        }
                        if state.show_selection_handle_end() {
                            TextFieldSelectionHandle(false, end_direction, manager.clone());
                        }
                    }
                    // A new character typed in selection mode (the toolbar shown) takes the field to editing.
                    if manager.is_text_changed() {
                        state.set_show_floating_toolbar(false);
                    }
                    if state.has_focus() {
                        if state.show_floating_toolbar() {
                            manager.show_selection_toolbar();
                        } else {
                            manager.hide_selection_toolbar();
                        }
                    }
                }
            } else {
                manager.hide_selection_toolbar();
            }
        })
    })
}

/// The cursor's handle while it shows on non-empty text (Compose's TextFieldCursorHandle): dragged, it moves the
/// cursor; tapped, it shows the toolbar.
#[track_caller]
#[allow(non_snake_case)]
fn TextFieldCursorHandle(manager: Rc<TextFieldSelectionManager>) {
    kompoz_runtime::group(|| {
        kompoz_runtime::kompozable_block!({
            let show = manager.state().is_some_and(|s| s.show_cursor_handle())
                && manager.transformed_text().is_some_and(|t| !t.text().is_empty());
            if show {
                let key = Rc::as_ptr(&manager) as usize;
                let m = manager.clone();
                let observer = remember_keyed(key, move || m.cursor_drag_observer());
                let position = manager.get_cursor_position(LocalDensity.current());
                let tap_manager = manager.clone();
                CursorHandle(
                    offset_provider = OffsetProvider::new(move || position),
                    modifier = Modifier.pointer_input(key, move |scope: PointerInputScope| {
                        let (observer, tap_manager) = (observer.clone(), tap_manager.clone());
                        async move {
                            kompoz_coroutines::coroutine_scope(move |cs| async move {
                                let (s1, s2) = (scope.clone(), scope);
                                cs.launch(async move {
                                    crate::text::detect_down_and_drag_gestures_with_observer(s1, observer).await
                                });
                                cs.launch(async move {
                                    use crate::gestures::PointerInputScopeGestures;
                                    s2.detect_tap_gestures().on_tap(move |_| tap_manager.show_selection_toolbar()).await
                                });
                            })
                            .await
                        }
                    }),
                    min_touch_target_size = kompoz_graphics::DpSize::Unspecified,
                );
            }
        })
    })
}

/// Back (where the platform has one) deselects a selection (Compose's private `previewKeyEventToDeselectOnBack`).
fn preview_key_event_to_deselect_on_back(
    state: &Rc<LegacyTextFieldState>,
    manager: &Rc<TextFieldSelectionManager>,
) -> Modifier {
    let (state, manager) = (state.clone(), manager.clone());
    Modifier.on_preview_key_event(move |event| {
        if state.handle_state.peek() == HandleState::Selection && cancels_text_selection(event) {
            manager.deselect(None);
            true
        } else {
            false
        }
    })
}

/// The field's focus (Compose's `textFieldFocusModifier`): its requester, its focus told to `on_focus_changed`,
/// focusable while `enabled`.
fn text_field_focus_modifier(
    enabled: bool,
    focus_requester: &FocusRequester,
    interaction_source: Option<MutableInteractionSource>,
    on_focus_changed: impl Fn(FocusState) + 'static,
) -> Modifier {
    Modifier
        .focus_requester(focus_requester)
        .on_focus_changed(on_focus_changed)
        .focusable_with(enabled, interaction_source)
}

/// Draws the text with its selection or highlights behind the content (Compose's `textFieldDraw`, its
/// `defaultTextFieldDraw`).
fn text_field_draw(
    state: &Rc<LegacyTextFieldState>,
    value: &TextFieldValue,
    offset_mapping: &OffsetMapping,
) -> Modifier {
    let (state, value, offset_mapping) = (state.clone(), value.clone(), offset_mapping.clone());
    Modifier.draw_behind(move |scope| {
        if let Some(layout_result) = state.layout_result() {
            delegate::draw(
                scope,
                &value,
                state.selection_preview_highlight_range(),
                state.deletion_preview_highlight_range(),
                &offset_mapping,
                &layout_result.value,
                state.selection_background_color(),
            );
        }
    })
}

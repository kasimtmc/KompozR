//! The password field (Compose's BasicSecureTextField): a single-line BasicTextField that shows its text as
//! obfuscation characters (the last one typed revealed for a moment where the mode says), never copies or cuts it
//! (by keys, the toolbar or accessibility), tells accessibility and autofill it is a password, and hides a revealed
//! character as it loses the focus.

use super::basic_text_field::{BasicTextFieldArgs, BasicTextFieldDefaults, TextFieldDecorator, basic_text_field_impl};
use super::input::internal::{CodepointTransformation, OnTextFieldLayout};
use super::input::{
    ChangeList, InputTransformation, KeyboardActionHandler, TextFieldBuffer, TextFieldLineLimits, TextFieldState,
    TextObfuscationMode, TransformInput,
};
use super::key_mapping::{KeyCommand, platform_default_key_mapping};
use super::keyboard_options::KeyboardOptions;
use crate::interaction::MutableInteractionSource;
use crate::scroll::ScrollState;
use kompoz_coroutines::{Channel, Job, LaunchedEffect, coroutine_scope, delay_ms};
use kompoz_graphics::{Brush, Rect};
use kompoz_runtime::{KompozitionLocalProvider, MutableState, kompozable, mutable_state_of, remember, remember_keyed};
use kompoz_text::TextStyle;
use kompoz_ui::Modifier;
use kompoz_ui::platform::{LocalTextToolbar, TextToolbar, TextToolbarAction, TextToolbarStatus};
use kompoz_ui::semantics::ContentType;
use std::cell::RefCell;
use std::rc::Rc;

/// How long a typed character shows (Compose's LAST_TYPED_CHARACTER_REVEAL_DURATION_MILLIS, Android's
/// PasswordTransformationMethod's).
const LAST_TYPED_CHARACTER_REVEAL_DURATION_MILLIS: u64 = 1500;

/// What a secure field shows for each character unless given (Compose's DefaultObfuscationCharacter: a bullet).
pub const DefaultObfuscationCharacter: char = '\u{2022}';

/// Whether the platform lets a typed character show (Compose's SplitVisibilitySettings): from a touch keyboard,
/// from a physical one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct SplitVisibilitySettings {
    pub(crate) touch: bool,
    pub(crate) physical: bool,
}

/// The platform's password visibility settings (Compose's `rememberPlatformPasswordVisibilitySettingsState`):
/// the desktop shows no typed character in the System mode (Compose desktop's `platformAllowsRevealLastTyped`:
/// false).
fn platform_password_visibility_settings() -> SplitVisibilitySettings {
    SplitVisibilitySettings { touch: false, physical: false }
}

/// A password field (Compose's `BasicSecureTextField`): `state`'s text shown as `text_obfuscation_character`s
/// (`text_obfuscation_mode`: RevealLastTyped shows the character just typed for a moment, System does as the
/// platform says — the desktop doesn't —, Hidden never, Visible shows the text), on a single line; copying and
/// cutting are refused. The other parameters as [`BasicTextField`](super::BasicTextField)'s; its keyboard a
/// password's unless given.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn BasicSecureTextField(
    state: TextFieldState,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(false)] read_only: bool,
    #[default(None)] input_transformation: Option<InputTransformation>,
    #[default(TextStyle::Default)] text_style: TextStyle,
    #[default(KeyboardOptions::SecureTextField)] keyboard_options: KeyboardOptions,
    #[default(None)] on_keyboard_action: Option<Rc<dyn KeyboardActionHandler>>,
    #[default(None)] on_text_layout: Option<OnTextFieldLayout>,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(BasicTextFieldDefaults::cursor_brush())] cursor_brush: Brush,
    #[default(None)] decorator: Option<Rc<dyn TextFieldDecorator>>,
    #[default(TextObfuscationMode::System)] text_obfuscation_mode: TextObfuscationMode,
    #[default(DefaultObfuscationCharacter)] text_obfuscation_character: char,
    #[default(None)] scroll_state: Option<ScrollState>,
) {
    // Compose's `rememberUpdatedState`s: states the transformations read, set at each composition.
    let obfuscation_mask_state = remember(|| mutable_state_of(text_obfuscation_character));
    obfuscation_mask_state.set(text_obfuscation_character);
    let visibility_settings = platform_password_visibility_settings();
    let current_mode = remember(|| mutable_state_of(text_obfuscation_mode));
    current_mode.set(text_obfuscation_mode);
    let current_visibility_settings = remember(|| mutable_state_of(visibility_settings));
    current_visibility_settings.set(visibility_settings);
    let (mask, mode, settings) =
        (obfuscation_mask_state.clone(), current_mode.clone(), current_visibility_settings.clone());
    let controller = remember(move || Rc::new(SecureTextFieldController::new(mask, mode, settings)));
    let controller_key = Rc::as_ptr(&controller) as usize;
    let observer = controller.clone();
    LaunchedEffect(controller_key, async move {
        // Hides what was revealed once its time is up.
        observer.observe_hide_events().await;
    });

    // Revealing is in RevealLastTyped and System (the transformation gates it by the mode).
    let reveal_last_typed_enabled = text_obfuscation_mode == TextObfuscationMode::RevealLastTyped
        || text_obfuscation_mode == TextObfuscationMode::System;
    let resetter = controller.clone();
    LaunchedEffect(reveal_last_typed_enabled, async move {
        // Switched to a mode that doesn't reveal: nothing stays revealed.
        if !reveal_last_typed_enabled {
            resetter.password_input_transformation.hide();
        }
    });

    let (c, m) = (controller.clone(), obfuscation_mask_state.clone());
    let codepoint_transformation: Option<Rc<dyn CodepointTransformation>> =
        remember_keyed((text_obfuscation_mode, controller_key), move || match text_obfuscation_mode {
            TextObfuscationMode::RevealLastTyped | TextObfuscationMode::System => Some(c.codepoint_transformation()),
            TextObfuscationMode::Hidden => {
                let m = m.clone();
                Some(Rc::new(move |_: usize, _: char| m.get()) as Rc<dyn CodepointTransformation>)
            }
            TextObfuscationMode::Visible => None,
        });

    let focus_controller = controller.clone();
    let secure_text_field_modifier = modifier
        .semantics(|config| config.set_content_type(ContentType::Password()))
        .on_preview_key_event(|event| {
            // BasicTextField's own mapping: copy and cut go no further.
            let command = platform_default_key_mapping().map(event);
            command == Some(KeyCommand::COPY) || command == Some(KeyCommand::CUT)
        })
        .on_focus_changed(move |focus| {
            if !focus.is_focused() {
                focus_controller.password_input_transformation.hide();
            }
        });

    let password_transformation = InputTransformation::of(PasswordInputTransformationRef(controller.clone()));
    let input_transformation = match input_transformation {
        None => password_transformation,
        Some(t) => t.then(password_transformation),
    };
    let args = BasicTextFieldArgs {
        state,
        modifier: secure_text_field_modifier,
        enabled,
        read_only,
        input_transformation: Some(input_transformation),
        text_style,
        keyboard_options,
        on_keyboard_action,
        line_limits: TextFieldLineLimits::SingleLine,
        on_text_layout,
        interaction_source,
        cursor_brush,
        codepoint_transformation,
        output_transformation: None,
        decorator,
        scroll_state,
        is_password: true,
    };
    DisableCutCopy(move || basic_text_field_impl(args.clone()));
}

/// Shows and hides a secure field's characters (Compose's SecureTextFieldController).
pub(crate) struct SecureTextFieldController {
    obfuscation_mask: MutableState<char>,
    pub(crate) password_input_transformation: Rc<PasswordInputTransformation>,
    reset_timer_signal: Channel<()>,
}

impl SecureTextFieldController {
    fn new(
        obfuscation_mask: MutableState<char>,
        text_obfuscation_mode: MutableState<TextObfuscationMode>,
        platform_allows_reveal: MutableState<SplitVisibilitySettings>,
    ) -> SecureTextFieldController {
        let reset_timer_signal: Channel<()> = Channel::unlimited();
        let signal = reset_timer_signal.clone();
        let password_input_transformation = Rc::new_cyclic(|me: &std::rc::Weak<PasswordInputTransformation>| {
            let me = me.clone();
            PasswordInputTransformation {
                schedule_hide: Box::new(move || {
                    // A new hide scheduled; the signal refused: hidden at once.
                    if !signal.try_send(()).is_success()
                        && let Some(t) = me.upgrade()
                    {
                        t.hide();
                    }
                }),
                text_obfuscation_mode,
                platform_allows_reveal,
                reveal_codepoint_index: mutable_state_of(-1),
            }
        });
        SecureTextFieldController { obfuscation_mask, password_input_transformation, reset_timer_signal }
    }

    /// What obscures the text (Compose's `codepointTransformation`): the revealed code point as it is, every other
    /// one the mask.
    fn codepoint_transformation(&self) -> Rc<dyn CodepointTransformation> {
        let (reveal, mask) = (self.password_input_transformation.clone(), self.obfuscation_mask.clone());
        Rc::new(
            move |codepoint_index: usize, codepoint: char| {
                if codepoint_index as i64 == reveal.reveal_codepoint_index.get() { codepoint } else { mask.get() }
            },
        )
    }

    /// Hides the revealed character 1.5 s after the last one was (Compose's `observeHideEvents`: a signal
    /// collected latest, a new one restarting the wait).
    async fn observe_hide_events(self: Rc<Self>) {
        let signal = self.reset_timer_signal.clone();
        let transformation = self.password_input_transformation.clone();
        drop(self);
        coroutine_scope(move |scope| async move {
            let latest: RefCell<Option<Job>> = RefCell::new(None);
            loop {
                signal.receive().await;
                if let Some(job) = latest.borrow_mut().take() {
                    job.cancel();
                }
                let t = transformation.clone();
                *latest.borrow_mut() = Some(scope.launch(async move {
                    delay_ms(LAST_TYPED_CHARACTER_REVEAL_DURATION_MILLIS).await;
                    t.hide();
                }));
            }
        })
        .await
    }
}

/// The secure field's input transformation (Compose's PasswordInputTransformation): a single character typed
/// is revealed (where the mode lets it), any other edit hides what was.
pub(crate) struct PasswordInputTransformation {
    schedule_hide: Box<dyn Fn()>,
    text_obfuscation_mode: MutableState<TextObfuscationMode>,
    platform_allows_reveal: MutableState<SplitVisibilitySettings>,
    /// The code point shown as it is (-1: none), read as a state.
    reveal_codepoint_index: MutableState<i64>,
}

impl PasswordInputTransformation {
    /// Everything goes back into hiding (Compose's `hide`).
    pub(crate) fn hide(&self) {
        self.reveal_codepoint_index.set(-1);
    }

    fn transform(&self, buffer: &mut TextFieldBuffer) {
        // Only a change adding one character.
        let changes = buffer.changes();
        let single_character_change = changes.change_count() == 1 && {
            let range = changes.get_range(0);
            buffer.as_char_sequence()[range.min()..range.max()].chars().count() == 1
        };
        // An expanded selection reveals nothing.
        if !single_character_change || buffer.has_selection() {
            self.reveal_codepoint_index.set(-1);
            return;
        }
        let should_reveal = match self.text_obfuscation_mode.get() {
            TextObfuscationMode::RevealLastTyped => true,
            TextObfuscationMode::System => {
                let settings = self.platform_allows_reveal.get();
                if buffer.changes().is_from_hardware_source(0) { settings.physical } else { settings.touch }
            }
            _ => false,
        };
        if !should_reveal {
            self.reveal_codepoint_index.set(-1);
            return;
        }
        // The insertion point in code points (offsets are bytes; the code point transformation counts characters).
        let insertion_point = buffer.changes().get_range(0).min();
        let codepoint_index = buffer.as_char_sequence()[..insertion_point].chars().count() as i64;
        if self.reveal_codepoint_index.peek() != codepoint_index {
            // The time to hide it starts.
            (self.schedule_hide)();
            self.reveal_codepoint_index.set(codepoint_index);
        }
    }
}

/// The controller's transformation as an input transformation of the field.
struct PasswordInputTransformationRef(Rc<SecureTextFieldController>);

impl TransformInput for PasswordInputTransformationRef {
    fn transform_input(&self, buffer: &mut TextFieldBuffer) {
        self.0.password_input_transformation.transform(buffer);
    }
}

/// A text toolbar offering no copy or cut (Compose's DisableCutCopy's toolbar, delegating to the one above).
struct CopyDisabledToolbar(Rc<dyn TextToolbar>);

impl TextToolbar for CopyDisabledToolbar {
    fn show_menu(
        &self,
        rect: Rect,
        _on_copy_requested: TextToolbarAction,
        on_paste_requested: TextToolbarAction,
        _on_cut_requested: TextToolbarAction,
        on_select_all_requested: TextToolbarAction,
        on_autofill_requested: TextToolbarAction,
    ) {
        self.0.show_menu(rect, None, on_paste_requested, None, on_select_all_requested, on_autofill_requested);
    }

    fn hide(&self) {
        self.0.hide();
    }

    fn status(&self) -> TextToolbarStatus {
        self.0.status()
    }
}

/// `content` with a toolbar that offers no copy or cut (Compose's DisableCutCopy).
#[kompozable]
fn DisableCutCopy(content: impl Fn() + 'static) {
    let current_toolbar = LocalTextToolbar.current();
    let key = Rc::as_ptr(&current_toolbar) as *const () as usize;
    let copy_disabled_toolbar =
        remember_keyed(key, move || Rc::new(CopyDisabledToolbar(current_toolbar)) as Rc<dyn TextToolbar>);
    KompozitionLocalProvider(&[LocalTextToolbar.provides_unequal(copy_disabled_toolbar)], || content());
}

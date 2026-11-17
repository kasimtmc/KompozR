//! What a text field does on its keyboard's action key (Compose's KeyboardActions, KeyboardActionScope and the
//! legacy field's KeyboardActionRunner).

use kompoz_runtime::named;
use kompoz_ui::focus::{FocusDirection, FocusManager};
use kompoz_ui::platform::{ImeAction, SoftwareKeyboardController};
use std::rc::Rc;

/// What a keyboard action runs in (Compose's KeyboardActionScope): the field's default action for a key, to run
/// from the app's.
pub trait KeyboardActionScope {
    /// Runs the default action for `ime_action`: Next and Previous move the focus, Done hides the keyboard.
    fn default_keyboard_action(&self, ime_action: ImeAction);
}

/// A keyboard action (an app's `KeyboardActionScope.() -> Unit`).
pub type KeyboardAction = Rc<dyn Fn(&dyn KeyboardActionScope)>;

/// The app's actions for the action keys (Compose's KeyboardActions): a key without one runs its default action.
/// Equal when they hold the same actions.
#[derive(Clone, Default)]
pub struct KeyboardActions {
    pub on_done: Option<KeyboardAction>,
    pub on_go: Option<KeyboardAction>,
    pub on_next: Option<KeyboardAction>,
    pub on_previous: Option<KeyboardAction>,
    pub on_search: Option<KeyboardAction>,
    pub on_send: Option<KeyboardAction>,
}

fn same(a: &Option<KeyboardAction>, b: &Option<KeyboardAction>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

impl PartialEq for KeyboardActions {
    fn eq(&self, other: &Self) -> bool {
        same(&self.on_done, &other.on_done)
            && same(&self.on_go, &other.on_go)
            && same(&self.on_next, &other.on_next)
            && same(&self.on_previous, &other.on_previous)
            && same(&self.on_search, &other.on_search)
            && same(&self.on_send, &other.on_send)
    }
}

impl std::fmt::Debug for KeyboardActions {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let has = |a: &Option<KeyboardAction>| a.is_some();
        f.debug_struct("KeyboardActions")
            .field("on_done", &has(&self.on_done))
            .field("on_go", &has(&self.on_go))
            .field("on_next", &has(&self.on_next))
            .field("on_previous", &has(&self.on_previous))
            .field("on_search", &has(&self.on_search))
            .field("on_send", &has(&self.on_send))
            .finish()
    }
}

impl KeyboardActions {
    /// No action of the app's (Compose's `KeyboardActions.Default`).
    pub const Default: KeyboardActions = KeyboardActions {
        on_done: None,
        on_go: None,
        on_next: None,
        on_previous: None,
        on_search: None,
        on_send: None,
    };

    /// `on_any` for every action key (Compose's `KeyboardActions(onAny)`).
    ///
    /// Kotlin: `KeyboardActions(onAny: KeyboardActionScope.() -> Unit)`
    pub fn on_any(on_any: impl Fn(&dyn KeyboardActionScope) + 'static) -> KeyboardActions {
        let action: KeyboardAction = Rc::new(on_any);
        KeyboardActions {
            on_done: Some(action.clone()),
            on_go: Some(action.clone()),
            on_next: Some(action.clone()),
            on_previous: Some(action.clone()),
            on_search: Some(action.clone()),
            on_send: Some(action),
        }
    }

    /// The app's action for `ime_action`, if it has one.
    pub fn action_for(&self, ime_action: ImeAction) -> Option<&KeyboardAction> {
        match ime_action {
            ImeAction::Done => self.on_done.as_ref(),
            ImeAction::Go => self.on_go.as_ref(),
            ImeAction::Next => self.on_next.as_ref(),
            ImeAction::Previous => self.on_previous.as_ref(),
            ImeAction::Search => self.on_search.as_ref(),
            ImeAction::Send => self.on_send.as_ref(),
            ImeAction::Default | ImeAction::None | ImeAction::Unspecified => None,
        }
    }
}

/// Compose's `KeyboardActions(onDone = …, onGo = …, onNext = …, onPrevious = …, onSearch = …, onSend = …)`.
#[named(type)]
pub fn KeyboardActions(
    #[default(None)] on_done: Option<KeyboardAction>,
    #[default(None)] on_go: Option<KeyboardAction>,
    #[default(None)] on_next: Option<KeyboardAction>,
    #[default(None)] on_previous: Option<KeyboardAction>,
    #[default(None)] on_search: Option<KeyboardAction>,
    #[default(None)] on_send: Option<KeyboardAction>,
) -> KeyboardActions {
    KeyboardActions { on_done, on_go, on_next, on_previous, on_search, on_send }
}

/// Runs a field's keyboard actions (Compose's KeyboardActionRunner, the value-based field's): the app's action for
/// a key if it has one, else the default one — Next and Previous move the focus, Done hides the keyboard.
pub struct KeyboardActionRunner {
    keyboard_controller: Option<SoftwareKeyboardController>,
    pub keyboard_actions: KeyboardActions,
    pub focus_manager: FocusManager,
}

impl KeyboardActionRunner {
    pub fn new(
        keyboard_controller: Option<SoftwareKeyboardController>,
        keyboard_actions: KeyboardActions,
        focus_manager: FocusManager,
    ) -> KeyboardActionRunner {
        KeyboardActionRunner { keyboard_controller, keyboard_actions, focus_manager }
    }

    /// Runs the action for `ime_action`: whether something ran. Panics for `Unspecified` ("invalid ImeAction", as
    /// Compose's).
    pub fn run_action(&self, ime_action: ImeAction) -> bool {
        assert!(ime_action != ImeAction::Unspecified, "invalid ImeAction");
        match self.keyboard_actions.action_for(ime_action).cloned() {
            Some(action) => {
                action(self);
                true
            }
            None => self.default_keyboard_action_with_result(ime_action),
        }
    }

    fn default_keyboard_action_with_result(&self, ime_action: ImeAction) -> bool {
        match ime_action {
            ImeAction::Next => {
                self.focus_manager.move_focus(FocusDirection::Next);
                true
            }
            ImeAction::Previous => {
                self.focus_manager.move_focus(FocusDirection::Previous);
                true
            }
            ImeAction::Done => match &self.keyboard_controller {
                Some(k) => {
                    k.hide();
                    true
                }
                None => false,
            },
            _ => false,
        }
    }
}

impl KeyboardActionScope for KeyboardActionRunner {
    fn default_keyboard_action(&self, ime_action: ImeAction) {
        self.default_keyboard_action_with_result(ime_action);
    }
}

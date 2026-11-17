//! What a state-based field does on its keyboard action (Compose's KeyboardActionHandler).

/// Runs when the input method's action key (or Enter in a single-line field) is pressed (Compose's
/// KeyboardActionHandler, a `fun interface`): given the field's default action for the key (moving the focus on,
/// hiding the keyboard), to run or not. Any `Fn(&dyn Fn())` is one (Kotlin's `{ performDefaultAction -> … }`).
pub trait KeyboardActionHandler {
    fn on_keyboard_action(&self, perform_default_action: &dyn Fn());
}

impl<F: Fn(&dyn Fn())> KeyboardActionHandler for F {
    fn on_keyboard_action(&self, perform_default_action: &dyn Fn()) {
        self(perform_default_action)
    }
}

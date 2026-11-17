//! How a secure text field hides what is typed (Compose's TextObfuscationMode).

/// How a secure text field hides its text (Compose's TextObfuscationMode).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TextObfuscationMode {
    /// Nothing hidden: the text as it is.
    Visible,
    /// Every character masked but the last one typed, shown for a moment (as Android's password fields do).
    RevealLastTyped,
    /// Every character masked.
    Hidden,
    /// As the platform's setting for showing passwords says: like [`RevealLastTyped`](TextObfuscationMode::
    /// RevealLastTyped) where the platform allows the last typed character shown, else [`Hidden`](
    /// TextObfuscationMode::Hidden) (Compose's default for BasicSecureTextField).
    #[default]
    System,
}

impl TextObfuscationMode {
    /// Its value as Compose numbers it (0 to 3).
    pub fn value(&self) -> i32 {
        match self {
            TextObfuscationMode::Visible => 0,
            TextObfuscationMode::RevealLastTyped => 1,
            TextObfuscationMode::Hidden => 2,
            TextObfuscationMode::System => 3,
        }
    }
}

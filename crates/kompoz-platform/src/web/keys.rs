//! The keyboard on the web (C++'s `keyOf`, completed): a `KeyboardEvent` as Compose's key — letters, digits and
//! the layout's punctuation by what the key types (its `key`, as Linux's keysyms and Windows' layouts name them),
//! everything else by the physical key (its `code`: moving and editing, the modifiers left and right, F1 to F24, the
//! numeric keypad and its Enter, media and the browser's keys, Japanese and Korean input keys) —, the character it
//! types (Compose's `utf16CodePoint`, as a Unicode scalar), and the modifiers held (the page's bits: Shift, Ctrl,
//! Alt, Meta; on Apple's systems Command is Ctrl, as the C++ Kompoz's web build had it: what Ctrl does elsewhere).

use crate::keys::{DIGITS, FUNCTION_KEYS, LETTERS};
use kompoz_ui::input::{Key, KeyModifiers};

/// The page's modifier bits (web.js): Shift, Ctrl, Alt, Meta.
pub fn modifiers(bits: u32) -> KeyModifiers {
    let mut m = KeyModifiers::None;
    for (bit, modifier) in
        [(1, KeyModifiers::Shift), (2, KeyModifiers::Ctrl), (4, KeyModifiers::Alt), (8, KeyModifiers::Meta)]
    {
        if bits & bit != 0 {
            m = m | modifier;
        }
    }
    m
}

/// The one character `key` is (a `KeyboardEvent.key` of a key that types), if it is one.
fn single_char(key: &str) -> Option<char> {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

/// The character a key types (`KeyboardEvent.key` when it is one character; 0 for a named key, a dead key, or an
/// input method's).
pub fn code_point(key: &str) -> i32 {
    single_char(key).filter(|c| !c.is_control()).map_or(0, |c| c as i32)
}

/// Compose's key for a `KeyboardEvent`'s `code` (the physical key) and `key` (what it is in the layout).
pub fn key_of(code: &str, key: &str) -> Key {
    // The numeric keypad by its physical key (its digits type digits, which would make them the row's).
    if let Some(k) = numpad_key(code, key) {
        return k;
    }
    if let Some(c) = single_char(key) {
        let lower = c.to_ascii_lowercase();
        if lower.is_ascii_lowercase() {
            return LETTERS[(lower as u8 - b'a') as usize];
        }
        if c.is_ascii_digit() {
            return DIGITS[(c as u8 - b'0') as usize];
        }
        let by_layout = punctuation_key(c);
        if by_layout != Key::Unknown {
            return by_layout;
        }
    }
    // A letter the layout's key types beyond a–z (ş, ü), a shifted key's symbol, a dead key: the physical key, as a
    // US keyboard has it.
    if let Some(letter) = code.strip_prefix("Key").and_then(single_char)
        && letter.is_ascii_uppercase()
    {
        return LETTERS[(letter as u8 - b'A') as usize];
    }
    if let Some(digit) = code.strip_prefix("Digit").and_then(single_char)
        && digit.is_ascii_digit()
    {
        return DIGITS[(digit as u8 - b'0') as usize];
    }
    if let Some(n) = code.strip_prefix('F').and_then(|n| n.parse::<usize>().ok())
        && (1..=24).contains(&n)
    {
        return FUNCTION_KEYS[n - 1];
    }
    named_key(code)
}

/// The numeric keypad's keys (`Numpad…` codes): digits by what Num Lock makes them (a digit, or moving).
fn numpad_key(code: &str, key: &str) -> Option<Key> {
    let rest = code.strip_prefix("Numpad")?;
    if let Some(d) = single_char(rest).filter(char::is_ascii_digit) {
        // With Num Lock off the keypad moves (its `key` says how), as Linux's keysyms have it.
        return Some(match key {
            "ArrowUp" => Key::NumPadDirectionUp,
            "ArrowDown" => Key::NumPadDirectionDown,
            "ArrowLeft" => Key::NumPadDirectionLeft,
            "ArrowRight" => Key::NumPadDirectionRight,
            "Home" => Key::NumPadMoveHome,
            "End" => Key::NumPadMoveEnd,
            "PageUp" => Key::NumPadPageUp,
            "PageDown" => Key::NumPadPageDown,
            "Insert" => Key::NumPadInsert,
            "Clear" => Key::Clear,
            _ => NUMPAD[(d as u8 - b'0') as usize],
        });
    }
    Some(match rest {
        "Decimal" => {
            if key == "Delete" {
                Key::NumPadDelete
            } else {
                Key::NumPadDot
            }
        }
        "Divide" => Key::NumPadDivide,
        "Multiply" => Key::NumPadMultiply,
        "Subtract" => Key::NumPadSubtract,
        "Add" => Key::NumPadAdd,
        "Enter" => Key::NumPadEnter,
        "Equal" => Key::NumPadEquals,
        "Comma" => Key::NumPadComma,
        "ParenLeft" => Key::NumPadLeftParenthesis,
        "ParenRight" => Key::NumPadRightParenthesis,
        _ => return None,
    })
}

const NUMPAD: [Key; 10] = [
    Key::NumPad0,
    Key::NumPad1,
    Key::NumPad2,
    Key::NumPad3,
    Key::NumPad4,
    Key::NumPad5,
    Key::NumPad6,
    Key::NumPad7,
    Key::NumPad8,
    Key::NumPad9,
];

/// The key of a punctuation character (a layout's punctuation key typed alone), as the US keyboard's keys and
/// Linux's keysyms name them; `Unknown` for one without a key of its own.
fn punctuation_key(c: char) -> Key {
    match c {
        ';' => Key::Semicolon,
        '=' => Key::Equals,
        ',' => Key::Comma,
        '-' => Key::Minus,
        '.' => Key::Period,
        '/' => Key::Slash,
        '`' => Key::Grave,
        '[' => Key::LeftBracket,
        '\\' => Key::Backslash,
        ']' => Key::RightBracket,
        '\'' => Key::Apostrophe,
        '@' => Key::At,
        '#' => Key::Pound,
        '+' => Key::Plus,
        '*' => Key::Multiply,
        '¥' => Key::Yen,
        ' ' => Key::Spacebar,
        _ => Key::Unknown,
    }
}

/// The keys named by their physical place (UI Events' `code` values).
fn named_key(code: &str) -> Key {
    match code {
        "Escape" => Key::Escape,
        "Tab" => Key::Tab,
        "Enter" => Key::Enter,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "Insert" => Key::Insert,
        "Space" => Key::Spacebar,
        "Home" => Key::MoveHome,
        "End" => Key::MoveEnd,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "ArrowLeft" => Key::DirectionLeft,
        "ArrowUp" => Key::DirectionUp,
        "ArrowRight" => Key::DirectionRight,
        "ArrowDown" => Key::DirectionDown,
        "ShiftLeft" => Key::ShiftLeft,
        "ShiftRight" => Key::ShiftRight,
        "ControlLeft" => Key::CtrlLeft,
        "ControlRight" => Key::CtrlRight,
        "AltLeft" => Key::AltLeft,
        "AltRight" => Key::AltRight,
        "MetaLeft" | "OSLeft" => Key::MetaLeft,
        "MetaRight" | "OSRight" => Key::MetaRight,
        "CapsLock" => Key::CapsLock,
        "NumLock" => Key::NumLock,
        "ScrollLock" => Key::ScrollLock,
        "PrintScreen" => Key::PrintScreen,
        "Pause" => Key::Break,
        "ContextMenu" => Key::Menu,
        "Help" => Key::Help,
        "Fn" => Key::Function,
        // The US keyboard's punctuation, where the layout's key typed none of it (a dead key, a letter).
        "Semicolon" => Key::Semicolon,
        "Equal" => Key::Equals,
        "Comma" => Key::Comma,
        "Minus" => Key::Minus,
        "Period" => Key::Period,
        "Slash" => Key::Slash,
        "Backquote" => Key::Grave,
        "BracketLeft" => Key::LeftBracket,
        "Backslash" | "IntlBackslash" => Key::Backslash,
        "BracketRight" => Key::RightBracket,
        "Quote" => Key::Apostrophe,
        "IntlRo" => Key::Ro,
        "IntlYen" => Key::Yen,
        // Media, volume, the browser's and the system's keys.
        "BrowserBack" => Key::Back,
        "BrowserForward" => Key::Forward,
        "BrowserRefresh" => Key::Refresh,
        "BrowserStop" => Key::MediaStop,
        "BrowserSearch" => Key::Search,
        "BrowserFavorites" => Key::Bookmark,
        "BrowserHome" => Key::Browser,
        "AudioVolumeMute" | "VolumeMute" => Key::VolumeMute,
        "AudioVolumeDown" | "VolumeDown" => Key::VolumeDown,
        "AudioVolumeUp" | "VolumeUp" => Key::VolumeUp,
        "MediaTrackNext" => Key::MediaNext,
        "MediaTrackPrevious" => Key::MediaPrevious,
        "MediaStop" => Key::MediaStop,
        "MediaPlayPause" => Key::MediaPlayPause,
        "MediaSelect" | "LaunchMediaPlayer" => Key::Music,
        "LaunchMail" => Key::Envelope,
        "LaunchApp1" => Key::Browser,
        "LaunchApp2" => Key::Calculator,
        "Eject" => Key::MediaEject,
        "Power" => Key::Power,
        "Sleep" => Key::Sleep,
        "WakeUp" => Key::WakeUp,
        "Copy" => Key::Copy,
        "Cut" => Key::Cut,
        "Paste" => Key::Paste,
        // Japanese and Korean input keys.
        "KanaMode" => Key::Kana,
        "Lang1" => Key::LanguageSwitch, // (Korean Hangul/English; Japanese Kana on Macs)
        "Lang2" => Key::SwitchCharset,  // (Korean Hanja; Japanese Eisu on Macs)
        "Convert" => Key::Henkan,
        "NonConvert" => Key::Muhenkan,
        "Lang3" => Key::KatakanaHiragana,
        "Lang4" | "Lang5" => Key::ZenkakuHankaru,
        _ => Key::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_by_what_they_type_and_where_they_are() {
        assert_eq!(key_of("KeyA", "a"), Key::A);
        assert_eq!(key_of("KeyA", "A"), Key::A);
        // A French keyboard's A key types "q"; a Turkish one's ş key types no letter a–z: its place.
        assert_eq!(key_of("KeyA", "q"), Key::Q);
        assert_eq!(key_of("Semicolon", "ş"), Key::Semicolon);
        assert_eq!(key_of("Digit7", "7"), Key::Seven);
        assert_eq!(key_of("Digit7", "/"), Key::Slash);
        assert_eq!(key_of("Digit2", "\""), Key::Two);
        assert_eq!(key_of("KeyC", "c"), Key::C);
        assert_eq!(key_of("Backquote", "Dead"), Key::Grave);
        assert_eq!(key_of("F5", "F5"), Key::F5);
        assert_eq!(key_of("F24", "F24"), Key::F24);
        assert_eq!(key_of("ArrowLeft", "ArrowLeft"), Key::DirectionLeft);
        assert_eq!(key_of("ShiftRight", "Shift"), Key::ShiftRight);
        assert_eq!(key_of("ControlLeft", "Control"), Key::CtrlLeft);
        assert_eq!(key_of("Space", " "), Key::Spacebar);
        assert_eq!(key_of("Enter", "Enter"), Key::Enter);
        assert_eq!(key_of("NumpadEnter", "Enter"), Key::NumPadEnter);
        assert_eq!(key_of("Numpad4", "4"), Key::NumPad4);
        assert_eq!(key_of("Numpad4", "ArrowLeft"), Key::NumPadDirectionLeft);
        assert_eq!(key_of("NumpadDecimal", ","), Key::NumPadDot);
        assert_eq!(key_of("Unidentified", "Unidentified"), Key::Unknown);
    }

    #[test]
    fn characters_typed_and_modifiers() {
        assert_eq!(code_point("a"), 'a' as i32);
        assert_eq!(code_point("ş"), 'ş' as i32);
        assert_eq!(code_point("😀"), '😀' as i32);
        assert_eq!(code_point("Enter"), 0);
        assert_eq!(code_point("Dead"), 0);
        assert_eq!(modifiers(1 | 2), KeyModifiers::Shift | KeyModifiers::Ctrl);
        assert_eq!(modifiers(0), KeyModifiers::None);
    }
}

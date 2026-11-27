//! The keyboard on macOS (C++'s `keyOf` and `heldKeys`, completed): a key event's virtual key code (Carbon's
//! `kVK_*`, where the key is on the keyboard) as Compose's key — the letters as the layout maps them (the character
//! the key types without modifiers, as C++ and AWT have them; a layout without Latin letters by where they are),
//! the whole keyboard besides (digits and punctuation by place, F1 to F20, moving and editing, the modifiers left and
//! right, the numeric keypad and its Enter, the JIS keyboard's keys, the media keys AppKit passes) —, the modifiers
//! held (Command is Compose's Meta: foundation's macOS key mapping takes the shortcuts with it; Control is Ctrl,
//! Option Alt), a modifier key's press or release from the flags it changed (left and right told apart by the
//! device's own bits), the character a key types as Compose's key event carries it, and a menu item's shortcut as
//! AppKit's key equivalent.
//!
//! Plain Rust (raw codes and flags in, Compose's terms out): tested on every host.

use crate::keys::LETTERS;
use kompoz_ui::input::{Key, KeyModifiers};

// NSEventModifierFlags (device-independent).
pub const FLAG_CAPS_LOCK: u64 = 1 << 16;
pub const FLAG_SHIFT: u64 = 1 << 17;
pub const FLAG_CONTROL: u64 = 1 << 18;
pub const FLAG_OPTION: u64 = 1 << 19;
pub const FLAG_COMMAND: u64 = 1 << 20;
pub const FLAG_FUNCTION: u64 = 1 << 23;

// The device's own bits in the same flags (IOKit's NX_DEVICE*KEYMASK): which of the two keys is held.
const DEVICE_LEFT_CONTROL: u64 = 0x0001;
const DEVICE_LEFT_SHIFT: u64 = 0x0002;
const DEVICE_RIGHT_SHIFT: u64 = 0x0004;
const DEVICE_LEFT_COMMAND: u64 = 0x0008;
const DEVICE_RIGHT_COMMAND: u64 = 0x0010;
const DEVICE_LEFT_OPTION: u64 = 0x0020;
const DEVICE_RIGHT_OPTION: u64 = 0x0040;
const DEVICE_RIGHT_CONTROL: u64 = 0x2000;

// Carbon's virtual key codes (HIToolbox/Events.h) of the keys whose code says what they are.
pub const KVK_RETURN: u16 = 0x24;
pub const KVK_TAB: u16 = 0x30;
pub const KVK_SPACE: u16 = 0x31;
pub const KVK_DELETE: u16 = 0x33;
pub const KVK_ESCAPE: u16 = 0x35;
pub const KVK_RIGHT_COMMAND: u16 = 0x36;
pub const KVK_COMMAND: u16 = 0x37;
pub const KVK_SHIFT: u16 = 0x38;
pub const KVK_CAPS_LOCK: u16 = 0x39;
pub const KVK_OPTION: u16 = 0x3A;
pub const KVK_CONTROL: u16 = 0x3B;
pub const KVK_RIGHT_SHIFT: u16 = 0x3C;
pub const KVK_RIGHT_OPTION: u16 = 0x3D;
pub const KVK_RIGHT_CONTROL: u16 = 0x3E;
pub const KVK_FUNCTION: u16 = 0x3F;

/// The keys by their virtual key code (ANSI's positions; ISO's and JIS's own keys), as Compose names them.
const KEYS: &[(u16, Key)] = &[
    (0x00, Key::A),
    (0x01, Key::S),
    (0x02, Key::D),
    (0x03, Key::F),
    (0x04, Key::H),
    (0x05, Key::G),
    (0x06, Key::Z),
    (0x07, Key::X),
    (0x08, Key::C),
    (0x09, Key::V),
    (0x0A, Key::Grave), // (ISO's § key, left of 1)
    (0x0B, Key::B),
    (0x0C, Key::Q),
    (0x0D, Key::W),
    (0x0E, Key::E),
    (0x0F, Key::R),
    (0x10, Key::Y),
    (0x11, Key::T),
    (0x12, Key::One),
    (0x13, Key::Two),
    (0x14, Key::Three),
    (0x15, Key::Four),
    (0x16, Key::Six),
    (0x17, Key::Five),
    (0x18, Key::Equals),
    (0x19, Key::Nine),
    (0x1A, Key::Seven),
    (0x1B, Key::Minus),
    (0x1C, Key::Eight),
    (0x1D, Key::Zero),
    (0x1E, Key::RightBracket),
    (0x1F, Key::O),
    (0x20, Key::U),
    (0x21, Key::LeftBracket),
    (0x22, Key::I),
    (0x23, Key::P),
    (KVK_RETURN, Key::Enter),
    (0x25, Key::L),
    (0x26, Key::J),
    (0x27, Key::Apostrophe),
    (0x28, Key::K),
    (0x29, Key::Semicolon),
    (0x2A, Key::Backslash),
    (0x2B, Key::Comma),
    (0x2C, Key::Slash),
    (0x2D, Key::N),
    (0x2E, Key::M),
    (0x2F, Key::Period),
    (KVK_TAB, Key::Tab),
    (KVK_SPACE, Key::Spacebar),
    (0x32, Key::Grave),
    (KVK_DELETE, Key::Backspace),
    (0x34, Key::NumPadEnter), // (a PowerBook's Enter)
    (KVK_ESCAPE, Key::Escape),
    (KVK_RIGHT_COMMAND, Key::MetaRight),
    (KVK_COMMAND, Key::MetaLeft),
    (KVK_SHIFT, Key::ShiftLeft),
    (KVK_CAPS_LOCK, Key::CapsLock),
    (KVK_OPTION, Key::AltLeft),
    (KVK_CONTROL, Key::CtrlLeft),
    (KVK_RIGHT_SHIFT, Key::ShiftRight),
    (KVK_RIGHT_OPTION, Key::AltRight),
    (KVK_RIGHT_CONTROL, Key::CtrlRight),
    (KVK_FUNCTION, Key::Function),
    (0x40, Key::F17),
    (0x41, Key::NumPadDot),
    (0x43, Key::NumPadMultiply),
    (0x45, Key::NumPadAdd),
    (0x47, Key::NumLock), // (the keypad's Clear, where a PC keyboard has Num Lock)
    (0x48, Key::VolumeUp),
    (0x49, Key::VolumeDown),
    (0x4A, Key::VolumeMute),
    (0x4B, Key::NumPadDivide),
    (0x4C, Key::NumPadEnter),
    (0x4E, Key::NumPadSubtract),
    (0x4F, Key::F18),
    (0x50, Key::F19),
    (0x51, Key::NumPadEquals),
    (0x52, Key::NumPad0),
    (0x53, Key::NumPad1),
    (0x54, Key::NumPad2),
    (0x55, Key::NumPad3),
    (0x56, Key::NumPad4),
    (0x57, Key::NumPad5),
    (0x58, Key::NumPad6),
    (0x59, Key::NumPad7),
    (0x5A, Key::F20),
    (0x5B, Key::NumPad8),
    (0x5C, Key::NumPad9),
    (0x5D, Key::Yen),
    (0x5E, Key::Ro), // (JIS's underscore)
    (0x5F, Key::NumPadComma),
    (0x60, Key::F5),
    (0x61, Key::F6),
    (0x62, Key::F7),
    (0x63, Key::F3),
    (0x64, Key::F8),
    (0x65, Key::F9),
    (0x66, Key::Eisu),
    (0x67, Key::F11),
    (0x68, Key::Kana),
    (0x69, Key::F13),
    (0x6A, Key::F16),
    (0x6B, Key::F14),
    (0x6D, Key::F10),
    (0x6E, Key::Menu), // (a PC keyboard's context menu key)
    (0x6F, Key::F12),
    (0x71, Key::F15),
    (0x72, Key::Insert), // (Help, where a PC keyboard has Insert)
    (0x73, Key::MoveHome),
    (0x74, Key::PageUp),
    (0x75, Key::Delete),
    (0x76, Key::F4),
    (0x77, Key::MoveEnd),
    (0x78, Key::F2),
    (0x79, Key::PageDown),
    (0x7A, Key::F1),
    (0x7B, Key::DirectionLeft),
    (0x7C, Key::DirectionRight),
    (0x7D, Key::DirectionDown),
    (0x7E, Key::DirectionUp),
];

/// The key of virtual key code `code`, `unmodified` the characters it types without modifiers (the layout's: a
/// Latin letter there is that letter's key wherever the layout puts it, as C++'s `keyOf` and AWT take it; else the
/// key at that place). Unknown for a code no key has.
pub fn key_of(code: u16, unmodified: &str) -> Key {
    let mut chars = unmodified.chars();
    if let (Some(c), None) = (chars.next(), chars.next())
        && c.is_ascii_alphabetic()
        && !is_keypad(code)
    {
        return LETTERS[(c.to_ascii_lowercase() as u8 - b'a') as usize];
    }
    KEYS.iter().find(|(k, _)| *k == code).map_or(Key::Unknown, |(_, key)| *key)
}

/// Whether the code is the numeric keypad's (whose keys are theirs whatever they type).
fn is_keypad(code: u16) -> bool {
    matches!(code, 0x41 | 0x43 | 0x45 | 0x47 | 0x4B | 0x4C | 0x4E | 0x51..=0x59 | 0x5B | 0x5C | 0x5F)
}

/// The modifiers held, as an event's flags say (Command as Meta).
pub fn modifiers(flags: u64) -> KeyModifiers {
    let mut m = KeyModifiers::None;
    for (bit, modifier) in [
        (FLAG_SHIFT, KeyModifiers::Shift),
        (FLAG_CONTROL, KeyModifiers::Ctrl),
        (FLAG_OPTION, KeyModifiers::Alt),
        (FLAG_COMMAND, KeyModifiers::Meta),
    ] {
        if flags & bit != 0 {
            m = m | modifier;
        }
    }
    m
}

/// A modifier key's change (AppKit's `flagsChanged:`, which says only the flags after it): the key, and whether it
/// went down (its own device bit set, where the flags carry them; else its kind's flag; Caps Lock: down as it locks,
/// up as it unlocks). None for a code that isn't a modifier's.
pub fn modifier_change(code: u16, flags: u64) -> Option<(Key, bool)> {
    let (key, device, flag) = match code {
        KVK_SHIFT => (Key::ShiftLeft, DEVICE_LEFT_SHIFT, FLAG_SHIFT),
        KVK_RIGHT_SHIFT => (Key::ShiftRight, DEVICE_RIGHT_SHIFT, FLAG_SHIFT),
        KVK_CONTROL => (Key::CtrlLeft, DEVICE_LEFT_CONTROL, FLAG_CONTROL),
        KVK_RIGHT_CONTROL => (Key::CtrlRight, DEVICE_RIGHT_CONTROL, FLAG_CONTROL),
        KVK_OPTION => (Key::AltLeft, DEVICE_LEFT_OPTION, FLAG_OPTION),
        KVK_RIGHT_OPTION => (Key::AltRight, DEVICE_RIGHT_OPTION, FLAG_OPTION),
        KVK_COMMAND => (Key::MetaLeft, DEVICE_LEFT_COMMAND, FLAG_COMMAND),
        KVK_RIGHT_COMMAND => (Key::MetaRight, DEVICE_RIGHT_COMMAND, FLAG_COMMAND),
        KVK_FUNCTION => (Key::Function, 0, FLAG_FUNCTION),
        KVK_CAPS_LOCK => return Some((Key::CapsLock, flags & FLAG_CAPS_LOCK != 0)),
        _ => return None,
    };
    let device_bits = DEVICE_LEFT_CONTROL
        | DEVICE_LEFT_SHIFT
        | DEVICE_RIGHT_SHIFT
        | DEVICE_LEFT_COMMAND
        | DEVICE_RIGHT_COMMAND
        | DEVICE_LEFT_OPTION
        | DEVICE_RIGHT_OPTION
        | DEVICE_RIGHT_CONTROL;
    let down = if device != 0 && flags & device_bits != 0 { flags & device != 0 } else { flags & flag != 0 };
    Some((key, down))
}

/// The character a key types as Compose's key event carries it (`utf16CodePoint`, a Unicode scalar here), from the
/// characters AppKit gives with the modifiers applied: Return, Tab, Backspace and Escape as their controls (DEL, which
/// AppKit's Backspace types, as Backspace's 8; the keypad's Enter, ETX, as Return's 13), none for AppKit's function
/// keys' private characters (the arrows, F1…) or for no character.
pub fn code_point(characters: &str) -> i32 {
    let Some(c) = characters.chars().next() else { return 0 };
    match c as u32 {
        0x7F => 8,
        0x03 => 13,
        0xF700..=0xF8FF => 0,
        cp => cp as i32,
    }
}

// The function keys' characters in AppKit (NSEvent.h's NS*FunctionKey).
const UP_ARROW: u32 = 0xF700;
const F1: u32 = 0xF704;
const INSERT: u32 = 0xF727;
const FORWARD_DELETE: u32 = 0xF728;
const HOME: u32 = 0xF729;
const END: u32 = 0xF72B;
const PAGE_UP: u32 = 0xF72C;
const PAGE_DOWN: u32 = 0xF72D;

/// A menu item's shortcut (its modifiers, its key's name as the menus name it: `app::key_name`) as AppKit's key
/// equivalent: the character (a letter lowercase: the shift is in the mask) and the modifier flags it is pressed with
/// (Ctrl Control, Alt Option, Meta Command); none for a key AppKit's menus can't show.
pub fn key_equivalent(modifiers: KeyModifiers, name: &str) -> Option<(String, u64)> {
    let named = |cp: u32| char::from_u32(cp).map(String::from);
    let character = match name {
        "Return" => Some("\r".to_string()),
        "Escape" => Some("\u{1b}".to_string()),
        "Tab" => Some("\t".to_string()),
        "space" => Some(" ".to_string()),
        "BackSpace" => Some("\u{8}".to_string()),
        "Delete" => named(FORWARD_DELETE),
        "Insert" => named(INSERT),
        "Home" => named(HOME),
        "End" => named(END),
        "Page_Up" => named(PAGE_UP),
        "Page_Down" => named(PAGE_DOWN),
        "Up" => named(UP_ARROW),
        "Down" => named(UP_ARROW + 1),
        "Left" => named(UP_ARROW + 2),
        "Right" => named(UP_ARROW + 3),
        "plus" => Some("+".to_string()),
        "minus" => Some("-".to_string()),
        "comma" => Some(",".to_string()),
        "period" => Some(".".to_string()),
        f if f.len() > 1 && f.starts_with('F') && f[1..].chars().all(|c| c.is_ascii_digit()) => {
            let n: u32 = f[1..].parse().ok()?;
            if !(1..=35).contains(&n) {
                return None;
            }
            named(F1 + n - 1)
        }
        other => {
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Some(c.to_lowercase().collect()),
                _ => None,
            }
        }
    }?;
    let mut mask = 0;
    for (modifier, bit) in [
        (KeyModifiers::Shift, FLAG_SHIFT),
        (KeyModifiers::Ctrl, FLAG_CONTROL),
        (KeyModifiers::Alt, FLAG_OPTION),
        (KeyModifiers::Meta, FLAG_COMMAND),
    ] {
        if modifiers.contains(modifier) {
            mask |= bit;
        }
    }
    Some((character, mask))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::DIGITS;

    #[test]
    fn letters_follow_the_layout_and_the_rest_its_place() {
        // QWERTY's A, AZERTY's A (on Q's place), Turkish's ı (no Latin letter: the key at I's place).
        assert_eq!(key_of(0x00, "a"), Key::A);
        assert_eq!(key_of(0x0C, "a"), Key::A);
        assert_eq!(key_of(0x22, "ı"), Key::I);
        // Digits by place, whatever they type (AZERTY's 1 types &).
        assert_eq!(key_of(0x12, "&"), Key::One);
        assert!(DIGITS.contains(&key_of(0x1D, "0")));
        // The keypad's keys are theirs (its Clear Num Lock's place), its Enter its own.
        assert_eq!(key_of(0x53, "1"), Key::NumPad1);
        assert_eq!(key_of(0x4C, "\u{3}"), Key::NumPadEnter);
        // Moving, editing, the function keys, the modifiers; a code no key has.
        for (code, key) in [
            (0x7E, Key::DirectionUp),
            (0x73, Key::MoveHome),
            (0x75, Key::Delete),
            (KVK_DELETE, Key::Backspace),
            (KVK_RETURN, Key::Enter),
            (0x7A, Key::F1),
            (0x5A, Key::F20),
            (KVK_RIGHT_OPTION, Key::AltRight),
            (KVK_COMMAND, Key::MetaLeft),
            (0x66, Key::Eisu),
        ] {
            assert_eq!(key_of(code, ""), key, "{code:#x}");
        }
        assert_eq!(key_of(0x7F, ""), Key::Unknown);
        // Every code once.
        for (i, (a, _)) in KEYS.iter().enumerate() {
            assert!(!KEYS[i + 1..].iter().any(|(b, _)| a == b), "{a:#x} twice");
        }
    }

    #[test]
    fn command_is_meta() {
        assert_eq!(modifiers(FLAG_COMMAND | FLAG_SHIFT), KeyModifiers::Meta | KeyModifiers::Shift);
        assert_eq!(modifiers(FLAG_CONTROL | FLAG_OPTION), KeyModifiers::Ctrl | KeyModifiers::Alt);
        assert_eq!(modifiers(FLAG_CAPS_LOCK | FLAG_FUNCTION), KeyModifiers::None);
    }

    #[test]
    fn modifier_keys_go_down_and_up_by_their_own_bits() {
        // The left Shift down; the right one down with it; the left one up while the right is held.
        assert_eq!(modifier_change(KVK_SHIFT, FLAG_SHIFT | 0x2), Some((Key::ShiftLeft, true)));
        assert_eq!(modifier_change(KVK_RIGHT_SHIFT, FLAG_SHIFT | 0x6), Some((Key::ShiftRight, true)));
        assert_eq!(modifier_change(KVK_SHIFT, FLAG_SHIFT | 0x4), Some((Key::ShiftLeft, false)));
        // Without the device's bits: by the kind's flag.
        assert_eq!(modifier_change(KVK_COMMAND, FLAG_COMMAND), Some((Key::MetaLeft, true)));
        assert_eq!(modifier_change(KVK_RIGHT_CONTROL, 0), Some((Key::CtrlRight, false)));
        assert_eq!(modifier_change(KVK_CAPS_LOCK, FLAG_CAPS_LOCK), Some((Key::CapsLock, true)));
        assert_eq!(modifier_change(0x00, FLAG_SHIFT), None);
    }

    #[test]
    fn typed_characters() {
        assert_eq!(code_point("a"), 'a' as i32);
        assert_eq!(code_point("ğ"), 'ğ' as i32);
        assert_eq!(code_point("\u{7f}"), 8);
        assert_eq!(code_point("\r"), 13);
        assert_eq!(code_point("\u{3}"), 13);
        assert_eq!(code_point("\u{F700}"), 0);
        assert_eq!(code_point(""), 0);
    }

    #[test]
    fn shortcuts_as_key_equivalents() {
        let cmd = KeyModifiers::Meta;
        assert_eq!(key_equivalent(cmd, "q"), Some(("q".into(), FLAG_COMMAND)));
        assert_eq!(key_equivalent(cmd | KeyModifiers::Shift, "S"), Some(("s".into(), FLAG_COMMAND | FLAG_SHIFT)));
        assert_eq!(key_equivalent(KeyModifiers::Ctrl, "F4"), Some(("\u{F707}".into(), FLAG_CONTROL)));
        assert_eq!(key_equivalent(KeyModifiers::Alt, "Delete"), Some(("\u{F728}".into(), FLAG_OPTION)));
        assert_eq!(key_equivalent(cmd, "Right"), Some(("\u{F703}".into(), FLAG_COMMAND)));
        assert_eq!(key_equivalent(cmd, "plus"), Some(("+".into(), FLAG_COMMAND)));
        assert_eq!(key_equivalent(cmd, "Multi_key"), None);
        assert_eq!(key_equivalent(cmd, ""), None);
    }
}

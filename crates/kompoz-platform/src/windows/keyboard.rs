//! The keyboard on Windows (C++'s `keyOf` and `heldKeys`, completed): a key message's virtual key, scan code and
//! extended bit as Compose's key — the whole keyboard (letters and digits as the layout maps them, F1 to F24,
//! moving and editing, the modifiers left and right, the numeric keypad and its Enter, the layout's punctuation by
//! the character its key types, media, browser and the system's keys, Japanese and Korean input keys) as Linux's
//! keysym table has it —, the modifiers held (AltGr, which Windows sends as the right Alt with a left Ctrl it makes
//! up, is neither Ctrl nor Alt: what it types isn't a shortcut), and the character a key types with the modifiers
//! held (Compose's `utf16CodePoint`, `ToUnicodeEx` without changing the keyboard's state).

use super::sys;
use crate::keys::{DIGITS, FUNCTION_KEYS, LETTERS};
use kompoz_ui::input::{Key, KeyModifiers};

// Virtual keys (winuser.h).
const VK_BACK: u16 = 0x08;
const VK_TAB: u16 = 0x09;
const VK_CLEAR: u16 = 0x0C;
const VK_RETURN: u16 = 0x0D;
pub(super) const VK_SHIFT: u16 = 0x10;
pub(super) const VK_CONTROL: u16 = 0x11;
pub(super) const VK_MENU: u16 = 0x12;
const VK_PAUSE: u16 = 0x13;
const VK_CAPITAL: u16 = 0x14;
const VK_KANA: u16 = 0x15;
const VK_IME_ON: u16 = 0x16;
const VK_JUNJA: u16 = 0x17;
const VK_FINAL: u16 = 0x18;
const VK_KANJI: u16 = 0x19;
const VK_IME_OFF: u16 = 0x1A;
const VK_ESCAPE: u16 = 0x1B;
const VK_CONVERT: u16 = 0x1C;
const VK_NONCONVERT: u16 = 0x1D;
const VK_MODECHANGE: u16 = 0x1F;
pub(super) const VK_SPACE: u16 = 0x20;
const VK_PRIOR: u16 = 0x21;
const VK_NEXT: u16 = 0x22;
const VK_END: u16 = 0x23;
const VK_HOME: u16 = 0x24;
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;
const VK_SELECT: u16 = 0x29;
const VK_PRINT: u16 = 0x2A;
const VK_EXECUTE: u16 = 0x2B;
const VK_SNAPSHOT: u16 = 0x2C;
const VK_INSERT: u16 = 0x2D;
const VK_DELETE: u16 = 0x2E;
const VK_HELP: u16 = 0x2F;
const VK_0: u16 = 0x30;
const VK_9: u16 = 0x39;
const VK_A: u16 = 0x41;
const VK_Z: u16 = 0x5A;
const VK_LWIN: u16 = 0x5B;
const VK_RWIN: u16 = 0x5C;
const VK_APPS: u16 = 0x5D;
const VK_SLEEP: u16 = 0x5F;
const VK_NUMPAD0: u16 = 0x60;
const VK_NUMPAD9: u16 = 0x69;
const VK_MULTIPLY: u16 = 0x6A;
const VK_ADD: u16 = 0x6B;
const VK_SEPARATOR: u16 = 0x6C;
const VK_SUBTRACT: u16 = 0x6D;
const VK_DECIMAL: u16 = 0x6E;
const VK_DIVIDE: u16 = 0x6F;
pub(super) const VK_F4: u16 = 0x73;
const VK_F1: u16 = 0x70;
const VK_F24: u16 = 0x87;
const VK_NUMLOCK: u16 = 0x90;
const VK_SCROLL: u16 = 0x91;
const VK_OEM_NEC_EQUAL: u16 = 0x92;
const VK_LSHIFT: u16 = 0xA0;
const VK_RSHIFT: u16 = 0xA1;
const VK_LCONTROL: u16 = 0xA2;
const VK_RCONTROL: u16 = 0xA3;
const VK_LMENU: u16 = 0xA4;
const VK_RMENU: u16 = 0xA5;
const VK_BROWSER_BACK: u16 = 0xA6;
const VK_BROWSER_FORWARD: u16 = 0xA7;
const VK_BROWSER_REFRESH: u16 = 0xA8;
const VK_BROWSER_STOP: u16 = 0xA9;
const VK_BROWSER_SEARCH: u16 = 0xAA;
const VK_BROWSER_FAVORITES: u16 = 0xAB;
const VK_BROWSER_HOME: u16 = 0xAC;
const VK_VOLUME_MUTE: u16 = 0xAD;
const VK_VOLUME_DOWN: u16 = 0xAE;
const VK_VOLUME_UP: u16 = 0xAF;
const VK_MEDIA_NEXT_TRACK: u16 = 0xB0;
const VK_MEDIA_PREV_TRACK: u16 = 0xB1;
const VK_MEDIA_STOP: u16 = 0xB2;
const VK_MEDIA_PLAY_PAUSE: u16 = 0xB3;
const VK_LAUNCH_MAIL: u16 = 0xB4;
const VK_LAUNCH_MEDIA_SELECT: u16 = 0xB5;
const VK_LAUNCH_APP1: u16 = 0xB6;
const VK_LAUNCH_APP2: u16 = 0xB7;
const VK_OEM_1: u16 = 0xBA;
const VK_OEM_PLUS: u16 = 0xBB;
const VK_OEM_COMMA: u16 = 0xBC;
const VK_OEM_MINUS: u16 = 0xBD;
const VK_OEM_PERIOD: u16 = 0xBE;
const VK_OEM_2: u16 = 0xBF;
const VK_OEM_3: u16 = 0xC0;
const VK_ABNT_C1: u16 = 0xC1;
const VK_ABNT_C2: u16 = 0xC2;
const VK_OEM_4: u16 = 0xDB;
const VK_OEM_5: u16 = 0xDC;
const VK_OEM_6: u16 = 0xDD;
const VK_OEM_7: u16 = 0xDE;
const VK_OEM_8: u16 = 0xDF;
const VK_OEM_102: u16 = 0xE2;
const VK_PLAY: u16 = 0xFA;
const VK_ZOOM: u16 = 0xFB;
const VK_OEM_CLEAR: u16 = 0xFE;

/// The scan code of the right Shift (Windows sends both Shifts as VK_SHIFT; the extended bit doesn't tell them).
const SCAN_RIGHT_SHIFT: u16 = 0x36;

/// A key message's key: its virtual key `vk`, scan code `scan` and extended bit (lParam's bit 24: the right Ctrl and
/// Alt, the keypad's Enter, the moving keys outside the keypad); for the layout's punctuation keys (VK_OEM_*), the
/// character `unshifted` gives the key typed alone (`ToUnicodeEx` with no modifiers: what a Turkish or German
/// layout has there), else the US layout's.
pub fn key_of(vk: u16, scan: u16, extended: bool, unshifted: impl Fn() -> Option<char>) -> Key {
    match vk {
        VK_A..=VK_Z => LETTERS[(vk - VK_A) as usize],
        VK_0..=VK_9 => DIGITS[(vk - VK_0) as usize],
        VK_F1..=VK_F24 => FUNCTION_KEYS[(vk - VK_F1) as usize],
        VK_NUMPAD0..=VK_NUMPAD9 => NUMPAD[(vk - VK_NUMPAD0) as usize],
        // The editing and system keys.
        VK_BACK => Key::Backspace,
        VK_TAB => Key::Tab,
        VK_CLEAR | VK_OEM_CLEAR => {
            // (The keypad's 5 without Num Lock: its middle.)
            if extended || vk == VK_OEM_CLEAR { Key::Clear } else { Key::DirectionCenter }
        }
        VK_RETURN => {
            if extended {
                Key::NumPadEnter
            } else {
                Key::Enter
            }
        }
        VK_PAUSE => Key::Break,
        VK_ESCAPE => Key::Escape,
        VK_SPACE => Key::Spacebar,
        VK_SELECT => Key::ButtonSelect,
        VK_PRINT | VK_SNAPSHOT => Key::PrintScreen,
        VK_EXECUTE => Key::Enter,
        VK_INSERT => Key::Insert,
        VK_DELETE => Key::Delete,
        VK_HELP => Key::Help,
        VK_APPS => Key::Menu,
        VK_SLEEP => Key::Sleep,
        // Moving (the keypad's too, with Num Lock off: as Linux has them).
        VK_PRIOR => Key::PageUp,
        VK_NEXT => Key::PageDown,
        VK_END => Key::MoveEnd,
        VK_HOME => Key::MoveHome,
        VK_LEFT => Key::DirectionLeft,
        VK_UP => Key::DirectionUp,
        VK_RIGHT => Key::DirectionRight,
        VK_DOWN => Key::DirectionDown,
        // The modifiers and locks, left and right.
        VK_SHIFT => {
            if scan == SCAN_RIGHT_SHIFT {
                Key::ShiftRight
            } else {
                Key::ShiftLeft
            }
        }
        VK_LSHIFT => Key::ShiftLeft,
        VK_RSHIFT => Key::ShiftRight,
        VK_CONTROL => {
            if extended {
                Key::CtrlRight
            } else {
                Key::CtrlLeft
            }
        }
        VK_LCONTROL => Key::CtrlLeft,
        VK_RCONTROL => Key::CtrlRight,
        VK_MENU => {
            if extended {
                Key::AltRight
            } else {
                Key::AltLeft
            }
        }
        VK_LMENU => Key::AltLeft,
        VK_RMENU => Key::AltRight,
        VK_LWIN => Key::MetaLeft,
        VK_RWIN => Key::MetaRight,
        VK_CAPITAL => Key::CapsLock,
        VK_NUMLOCK => Key::NumLock,
        VK_SCROLL => Key::ScrollLock,
        // The numeric keypad's operators.
        VK_MULTIPLY => Key::NumPadMultiply,
        VK_ADD => Key::NumPadAdd,
        VK_SEPARATOR | VK_ABNT_C2 => Key::NumPadComma,
        VK_SUBTRACT => Key::NumPadSubtract,
        VK_DECIMAL => Key::NumPadDot,
        VK_DIVIDE => Key::NumPadDivide,
        VK_OEM_NEC_EQUAL => Key::NumPadEquals,
        // Punctuation: by what the layout's key types, else as on a US keyboard.
        VK_OEM_1 | VK_OEM_PLUS | VK_OEM_COMMA | VK_OEM_MINUS | VK_OEM_PERIOD | VK_OEM_2 | VK_OEM_3 | VK_OEM_4
        | VK_OEM_5 | VK_OEM_6 | VK_OEM_7 | VK_OEM_8 | VK_OEM_102 | VK_ABNT_C1 => {
            let by_layout = unshifted().map(punctuation_key).unwrap_or(Key::Unknown);
            if by_layout != Key::Unknown {
                return by_layout;
            }
            match vk {
                VK_OEM_1 => Key::Semicolon,
                VK_OEM_PLUS => Key::Equals,
                VK_OEM_COMMA => Key::Comma,
                VK_OEM_MINUS => Key::Minus,
                VK_OEM_PERIOD => Key::Period,
                VK_OEM_2 => Key::Slash,
                VK_OEM_3 => Key::Grave,
                VK_OEM_4 => Key::LeftBracket,
                VK_OEM_5 => Key::Backslash,
                VK_OEM_6 => Key::RightBracket,
                VK_OEM_7 => Key::Apostrophe,
                VK_ABNT_C1 => Key::Ro, // (the Brazilian keyboard's /?, where Japanese keyboards have ろ)
                _ => Key::Unknown,
            }
        }
        // Media, volume, the browser's and the system's keys.
        VK_BROWSER_BACK => Key::Back,
        VK_BROWSER_FORWARD => Key::Forward,
        VK_BROWSER_REFRESH => Key::Refresh,
        VK_BROWSER_STOP => Key::MediaStop,
        VK_BROWSER_SEARCH => Key::Search,
        VK_BROWSER_FAVORITES => Key::Bookmark,
        VK_BROWSER_HOME => Key::Browser,
        VK_VOLUME_MUTE => Key::VolumeMute,
        VK_VOLUME_DOWN => Key::VolumeDown,
        VK_VOLUME_UP => Key::VolumeUp,
        VK_MEDIA_NEXT_TRACK => Key::MediaNext,
        VK_MEDIA_PREV_TRACK => Key::MediaPrevious,
        VK_MEDIA_STOP => Key::MediaStop,
        VK_MEDIA_PLAY_PAUSE => Key::MediaPlayPause,
        VK_PLAY => Key::MediaPlay,
        VK_ZOOM => Key::ZoomIn,
        VK_LAUNCH_MAIL => Key::Envelope,
        VK_LAUNCH_MEDIA_SELECT => Key::Music,
        VK_LAUNCH_APP1 => Key::Browser, // (the "My Computer" key: Explorer)
        VK_LAUNCH_APP2 => Key::Calculator,
        // Japanese and Korean input keys (the same virtual keys on both: Kana/Hangul, Kanji/Hanja).
        VK_KANA => Key::Kana,
        VK_KANJI => Key::KatakanaHiragana,
        VK_CONVERT => Key::Henkan,
        VK_NONCONVERT => Key::Muhenkan,
        VK_MODECHANGE | VK_JUNJA | VK_FINAL => Key::SwitchCharset,
        VK_IME_ON | VK_IME_OFF => Key::LanguageSwitch,
        // (Japanese keyboards' half-width/full-width key: VK_OEM_AUTO and VK_OEM_ENLW, 0xF3/0xF4; Eisu is CapsLock's
        // VK_OEM_ATTN, 0xF0.)
        0xF3 | 0xF4 => Key::ZenkakuHankaru,
        0xF0 => Key::Eisu,
        0xF2 => Key::KatakanaHiragana, // VK_OEM_COPY: Hiragana on Japanese keyboards
        _ => Key::Unknown,
    }
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
/// Linux's keysyms name them; `Unknown` for one without a key of its own (a letter beyond a–z: ş, ü, ö).
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
        _ => Key::Unknown,
    }
}

/// The keys held as of the message handled (C++'s `heldKeys`): Shift; Ctrl and Alt, not where they are AltGr (the
/// right Alt with the left Ctrl Windows makes up for it, without the right Ctrl); the Windows keys as Meta.
pub fn modifiers() -> KeyModifiers {
    modifiers_of(sys::key_down)
}

/// [`modifiers`] as `down` says the keys are.
pub fn modifiers_of(down: impl Fn(u16) -> bool) -> KeyModifiers {
    let alt_gr = down(VK_RMENU) && down(VK_LCONTROL) && !down(VK_RCONTROL);
    let mut m = KeyModifiers::None;
    if down(VK_SHIFT) {
        m = m | KeyModifiers::Shift;
    }
    if down(VK_CONTROL) && !alt_gr {
        m = m | KeyModifiers::Ctrl;
    }
    if down(VK_MENU) && !alt_gr {
        m = m | KeyModifiers::Alt;
    }
    if down(VK_LWIN) || down(VK_RWIN) {
        m = m | KeyModifiers::Meta;
    }
    m
}

/// The character key `vk` (scan code `scan`) types with the keys held now (Compose's `utf16CodePoint`, as a Unicode
/// scalar: a pair of surrogates as the one character): 0 for none, a dead key, or more than one character.
pub fn code_point(vk: u16, scan: u16) -> i32 {
    match sys::key_characters(vk, scan, false) {
        Ok(text) => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => c as i32,
                _ => 0,
            }
        }
        Err(_) => 0,
    }
}

/// The character the layout's key `vk` types alone (no modifiers, no dead key: its own); none if it types none.
pub fn unshifted_character(vk: u16, scan: u16) -> Option<char> {
    let text = sys::key_characters(vk, scan, true).unwrap_or_else(|dead| dead);
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

/// A key message's parts: its scan code (lParam's bits 16-23), whether it is extended (bit 24), whether the key was
/// down before (bit 30: a repeat), whether it is going up (bit 31).
pub fn key_parts(l: isize) -> (u16, bool, bool) {
    let l = l as u32;
    (((l >> 16) & 0xff) as u16, l & (1 << 24) != 0, l & (1 << 30) != 0)
}

/// What a WM_CHAR gives, UTF-16 unit by unit: a character (a high surrogate kept until its low half comes); control
/// characters left out (C++'s: what Ctrl+letters and the editing keys make, which are keys, not text).
#[derive(Default)]
pub struct Characters {
    high: Option<u16>,
}

impl Characters {
    pub fn unit(&mut self, unit: u16) -> Option<String> {
        match unit {
            0xD800..=0xDBFF => {
                self.high = Some(unit);
                None
            }
            0xDC00..=0xDFFF => {
                let high = self.high.take()?;
                String::from_utf16(&[high, unit]).ok()
            }
            _ => {
                self.high = None;
                if unit < 0x20 || unit == 0x7f {
                    return None;
                }
                char::from_u32(unit as u32).map(String::from)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(vk: u16) -> Key {
        key_of(vk, 0, false, || None)
    }

    #[test]
    fn the_whole_keyboard_has_keys() {
        assert_eq!(key(0x41), Key::A);
        assert_eq!(key(0x5A), Key::Z);
        assert_eq!(key(0x30), Key::Zero);
        assert_eq!(key(0x70), Key::F1);
        assert_eq!(key(0x87), Key::F24);
        assert_eq!(key(0x60), Key::NumPad0);
        assert_eq!(key(0x69), Key::NumPad9);
        assert_eq!(key(VK_ESCAPE), Key::Escape);
        assert_eq!(key(VK_APPS), Key::Menu);
        assert_eq!(key(VK_MEDIA_PLAY_PAUSE), Key::MediaPlayPause);
        assert_eq!(key(VK_BROWSER_BACK), Key::Back);
        assert_eq!(key(VK_DIVIDE), Key::NumPadDivide);
        assert_eq!(key(0x07), Key::Unknown);
        // The extended bit and the scan code tell left from right, the keypad's Enter from Enter.
        assert_eq!(key_of(VK_RETURN, 0x1C, false, || None), Key::Enter);
        assert_eq!(key_of(VK_RETURN, 0x1C, true, || None), Key::NumPadEnter);
        assert_eq!(key_of(VK_CONTROL, 0x1D, false, || None), Key::CtrlLeft);
        assert_eq!(key_of(VK_CONTROL, 0x1D, true, || None), Key::CtrlRight);
        assert_eq!(key_of(VK_MENU, 0x38, true, || None), Key::AltRight);
        assert_eq!(key_of(VK_SHIFT, 0x2A, false, || None), Key::ShiftLeft);
        assert_eq!(key_of(VK_SHIFT, 0x36, false, || None), Key::ShiftRight);
        // (The keypad's 5 without Num Lock.)
        assert_eq!(key_of(VK_CLEAR, 0x4C, false, || None), Key::DirectionCenter);
    }

    #[test]
    fn punctuation_keys_are_named_by_what_the_layout_types() {
        // A US keyboard's: as its characters say, and without them.
        assert_eq!(key_of(VK_OEM_1, 0x27, false, || Some(';')), Key::Semicolon);
        assert_eq!(key(VK_OEM_1), Key::Semicolon);
        assert_eq!(key(VK_OEM_7), Key::Apostrophe);
        // A German keyboard's VK_OEM_PLUS types '+'; a Turkish Q's VK_OEM_2 types '.', its VK_OEM_1 'ş' (no key).
        assert_eq!(key_of(VK_OEM_PLUS, 0x1B, false, || Some('+')), Key::Plus);
        assert_eq!(key_of(VK_OEM_2, 0x34, false, || Some('.')), Key::Period);
        assert_eq!(key_of(VK_OEM_1, 0x27, false, || Some('ş')), Key::Semicolon);
    }

    #[test]
    fn alt_gr_is_neither_ctrl_nor_alt() {
        let held = |keys: &'static [u16]| move |vk: u16| keys.contains(&vk);
        let alt_gr = modifiers_of(held(&[VK_RMENU, VK_LCONTROL, VK_MENU, VK_CONTROL]));
        assert_eq!(alt_gr, KeyModifiers::None);
        let ctrl_alt = modifiers_of(held(&[VK_LMENU, VK_LCONTROL, VK_MENU, VK_CONTROL]));
        assert_eq!(ctrl_alt, KeyModifiers::Ctrl | KeyModifiers::Alt);
        let all = modifiers_of(held(&[VK_SHIFT, VK_RCONTROL, VK_CONTROL, VK_LWIN]));
        assert_eq!(all, KeyModifiers::Shift | KeyModifiers::Ctrl | KeyModifiers::Meta);
    }

    #[test]
    fn characters_come_in_utf16_halves() {
        let mut c = Characters::default();
        assert_eq!(c.unit('ş' as u16), Some("ş".into()));
        assert_eq!(c.unit(0xD83D), None);
        assert_eq!(c.unit(0xDE00), Some("😀".into()));
        assert_eq!(c.unit(0xDE00), None); // (a low half alone)
        assert_eq!(c.unit(0x08), None); // (Backspace's)
        assert_eq!(c.unit(0x01), None); // (Ctrl+A's)
        assert_eq!(c.unit(0x7f), None);
    }

    #[test]
    fn key_messages_parts() {
        // Right Ctrl down, repeated: scan 0x1D, extended, previously down.
        let l = (1u32 | (0x1D << 16) | (1 << 24) | (1 << 30)) as i32 as isize;
        assert_eq!(key_parts(l), (0x1D, true, true));
    }
}

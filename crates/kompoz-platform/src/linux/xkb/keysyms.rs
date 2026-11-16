//! X keysyms (xkbcommon's are the same) as Compose's keys: a complete table of what a keyboard has — letters and
//! digits, F1 to F24, moving and editing, the modifiers and locks, the numeric keypad, punctuation, media and the
//! system's keys, Japanese input keys — each keysym's value from xkbcommon-keysyms.h. A keysym with no key of its
//! own (a shifted symbol, a letter beyond a–z) is `Key::Unknown`: the keyboard asks its key's unshifted symbol then
//! (see [`Keyboard`](super::Keyboard)).

use crate::keys::{DIGITS, FUNCTION_KEYS, LETTERS};
use kompoz_ui::input::Key;

// `XKB_KEY_a`…`XKB_KEY_z`, `XKB_KEY_A`…`XKB_KEY_Z`, `XKB_KEY_0`…`XKB_KEY_9`, `XKB_KEY_F1`…`XKB_KEY_F24`.
const LOWER_A: u32 = 0x61;
const LOWER_Z: u32 = 0x7a;
const UPPER_A: u32 = 0x41;
const UPPER_Z: u32 = 0x5a;
const DIGIT_0: u32 = 0x30;
const DIGIT_9: u32 = 0x39;
const F1: u32 = 0xffbe;
const F24: u32 = 0xffd5;

/// The key of `keysym` (C++'s `keyOf`, completed); `Key::Unknown` if it has none.
pub fn key_of(keysym: u32) -> Key {
    match keysym {
        LOWER_A..=LOWER_Z => LETTERS[(keysym - LOWER_A) as usize],
        UPPER_A..=UPPER_Z => LETTERS[(keysym - UPPER_A) as usize],
        DIGIT_0..=DIGIT_9 => DIGITS[(keysym - DIGIT_0) as usize],
        F1..=F24 => FUNCTION_KEYS[(keysym - F1) as usize],
        // The editing and system keys.
        0xff08 => Key::Backspace,   // BackSpace
        0xff09 => Key::Tab,         // Tab
        0xfe20 => Key::Tab,         // ISO_Left_Tab
        0xff0a => Key::Enter,       // Linefeed
        0xff0b => Key::Clear,       // Clear
        0xff0d => Key::Enter,       // Return
        0xff13 => Key::Break,       // Pause
        0xff6b => Key::Break,       // Break
        0xff14 => Key::ScrollLock,  // Scroll_Lock
        0xff15 => Key::PrintScreen, // Sys_Req
        0xff61 => Key::PrintScreen, // Print
        0xff1b => Key::Escape,      // Escape
        0xffff => Key::Delete,      // Delete
        0xff63 => Key::Insert,      // Insert
        0xff67 => Key::Menu,        // Menu
        0xff6a => Key::Help,        // Help
        0xff68 => Key::Search,      // Find
        0xff69 => Key::Escape,      // Cancel
        0xff62 => Key::Enter,       // Execute
        // Moving.
        0xff50 => Key::MoveHome,        // Home
        0xff57 => Key::MoveEnd,         // End
        0xff58 => Key::DirectionCenter, // Begin
        0xff51 => Key::DirectionLeft,   // Left
        0xff52 => Key::DirectionUp,     // Up
        0xff53 => Key::DirectionRight,  // Right
        0xff54 => Key::DirectionDown,   // Down
        0xff55 => Key::PageUp,          // Page_Up
        0xff56 => Key::PageDown,        // Page_Down
        // The modifiers and locks (AltGr is the right Alt, the Super keys Meta).
        0xffe1 => Key::ShiftLeft,      // Shift_L
        0xffe2 => Key::ShiftRight,     // Shift_R
        0xffe3 => Key::CtrlLeft,       // Control_L
        0xffe4 => Key::CtrlRight,      // Control_R
        0xffe5 => Key::CapsLock,       // Caps_Lock
        0xffe6 => Key::CapsLock,       // Shift_Lock
        0xffe7 => Key::MetaLeft,       // Meta_L
        0xffe8 => Key::MetaRight,      // Meta_R
        0xffe9 => Key::AltLeft,        // Alt_L
        0xffea => Key::AltRight,       // Alt_R
        0xffeb => Key::MetaLeft,       // Super_L
        0xffec => Key::MetaRight,      // Super_R
        0xffed => Key::MetaLeft,       // Hyper_L
        0xffee => Key::MetaRight,      // Hyper_R
        0xfe03 => Key::AltRight,       // ISO_Level3_Shift
        0xff7f => Key::NumLock,        // Num_Lock
        0xff7e => Key::SwitchCharset,  // Mode_switch
        0xfe08 => Key::LanguageSwitch, // ISO_Next_Group
        // The numeric keypad (with Num Lock off its keys move).
        0xff80 => Key::Spacebar,        // KP_Space
        0xff89 => Key::Tab,             // KP_Tab
        0xff8d => Key::NumPadEnter,     // KP_Enter
        0xff95 => Key::MoveHome,        // KP_Home
        0xff96 => Key::DirectionLeft,   // KP_Left
        0xff97 => Key::DirectionUp,     // KP_Up
        0xff98 => Key::DirectionRight,  // KP_Right
        0xff99 => Key::DirectionDown,   // KP_Down
        0xff9a => Key::PageUp,          // KP_Page_Up
        0xff9b => Key::PageDown,        // KP_Page_Down
        0xff9c => Key::MoveEnd,         // KP_End
        0xff9d => Key::DirectionCenter, // KP_Begin
        0xff9e => Key::Insert,          // KP_Insert
        0xff9f => Key::Delete,          // KP_Delete
        0xffbd => Key::NumPadEquals,    // KP_Equal
        0xffaa => Key::NumPadMultiply,  // KP_Multiply
        0xffab => Key::NumPadAdd,       // KP_Add
        0xffac => Key::NumPadComma,     // KP_Separator
        0xffad => Key::NumPadSubtract,  // KP_Subtract
        0xffae => Key::NumPadDot,       // KP_Decimal
        0xffaf => Key::NumPadDivide,    // KP_Divide
        0xffb0 => Key::NumPad0,         // KP_0
        0xffb1 => Key::NumPad1,         // KP_1
        0xffb2 => Key::NumPad2,         // KP_2
        0xffb3 => Key::NumPad3,         // KP_3
        0xffb4 => Key::NumPad4,         // KP_4
        0xffb5 => Key::NumPad5,         // KP_5
        0xffb6 => Key::NumPad6,         // KP_6
        0xffb7 => Key::NumPad7,         // KP_7
        0xffb8 => Key::NumPad8,         // KP_8
        0xffb9 => Key::NumPad9,         // KP_9
        // Punctuation, as a US keyboard has it (the shifted ones fall back to their key's unshifted symbol).
        0x0020 => Key::Spacebar,     // space
        0x0027 => Key::Apostrophe,   // apostrophe
        0x002c => Key::Comma,        // comma
        0x002d => Key::Minus,        // minus
        0x002e => Key::Period,       // period
        0x002f => Key::Slash,        // slash
        0x003b => Key::Semicolon,    // semicolon
        0x003d => Key::Equals,       // equal
        0x005b => Key::LeftBracket,  // bracketleft
        0x005c => Key::Backslash,    // backslash
        0x005d => Key::RightBracket, // bracketright
        0x0060 => Key::Grave,        // grave
        0x0040 => Key::At,           // at
        0x0023 => Key::Pound,        // numbersign
        0x002b => Key::Plus,         // plus
        0x002a => Key::Multiply,     // asterisk
        // Media, volume and the system's keys (XFree86's).
        0x1008ff14 => Key::MediaPlayPause,   // XF86AudioPlay
        0x1008ff31 => Key::MediaPause,       // XF86AudioPause
        0x1008ff15 => Key::MediaStop,        // XF86AudioStop
        0x1008ff16 => Key::MediaPrevious,    // XF86AudioPrev
        0x1008ff17 => Key::MediaNext,        // XF86AudioNext
        0x1008ff1c => Key::MediaRecord,      // XF86AudioRecord
        0x1008ff3e => Key::MediaRewind,      // XF86AudioRewind
        0x1008ff97 => Key::MediaFastForward, // XF86AudioForward
        0x1008ff2c => Key::MediaEject,       // XF86Eject
        0x1008ff12 => Key::VolumeMute,       // XF86AudioMute
        0x1008ff11 => Key::VolumeDown,       // XF86AudioLowerVolume
        0x1008ff13 => Key::VolumeUp,         // XF86AudioRaiseVolume
        0x1008ffb2 => Key::MicrophoneMute,   // XF86AudioMicMute
        0x1008ff02 => Key::BrightnessUp,     // XF86MonBrightnessUp
        0x1008ff03 => Key::BrightnessDown,   // XF86MonBrightnessDown
        0x1008ff2f => Key::Sleep,            // XF86Sleep
        0x1008ffa7 => Key::Sleep,            // XF86Suspend
        0x1008ff2b => Key::WakeUp,           // XF86WakeUp
        0x1008ff2a => Key::Power,            // XF86PowerOff
        0x1008ff1b => Key::Search,           // XF86Search
        0x1008ff18 => Key::Browser,          // XF86HomePage
        0x1008ff2e => Key::Browser,          // XF86WWW
        0x1008ff5d => Key::Browser,          // XF86Explorer
        0x1008ff19 => Key::Envelope,         // XF86Mail
        0x1008ff1d => Key::Calculator,       // XF86Calculator
        0x1008ff20 => Key::Calendar,         // XF86Calendar
        0x1008ff92 => Key::Music,            // XF86Music
        0x1008ff32 => Key::Music,            // XF86AudioMedia
        0x1008ff26 => Key::Back,             // XF86Back
        0x1008ff27 => Key::Forward,          // XF86Forward
        0x1008ff29 => Key::Refresh,          // XF86Refresh
        0x1008ff73 => Key::Refresh,          // XF86Reload
        0x1008ff30 => Key::Bookmark,         // XF86Favorites
        0x1008ff58 => Key::Cut,              // XF86Cut
        0x1008ff57 => Key::Copy,             // XF86Copy
        0x1008ff6d => Key::Paste,            // XF86Paste
        0x1008ff8b => Key::ZoomIn,           // XF86ZoomIn
        0x1008ff8c => Key::ZoomOut,          // XF86ZoomOut
        0x1008ff8f => Key::Camera,           // XF86WebCam
        0x1008ff81 => Key::Settings,         // XF86Tools
        0x1008ff56 => Key::MediaClose,       // XF86Close
        0x1008ff6e => Key::Call,             // XF86Phone
        0x1008ff8e => Key::Notification,     // XF86Messenger
        0x1008ff59 => Key::Window,           // XF86Display
        0x1008ffb3 => Key::LanguageSwitch,   // XF86Keyboard
        0x10081166 => Key::Info,             // XF86Info
        0x1008ffa3 => Key::ProgramRed,       // XF86Red
        0x1008ffa4 => Key::ProgramGreen,     // XF86Green
        0x1008ffa5 => Key::ProgramYellow,    // XF86Yellow
        0x1008ffa6 => Key::ProgramBlue,      // XF86Blue
        0x10081192 => Key::ChannelUp,        // XF86ChannelUp
        0x10081193 => Key::ChannelDown,      // XF86ChannelDown
        0x1008ff9a => Key::Captions,         // XF86Subtitle
        0x10081247 => Key::Assist,           // XF86Assistant
        // International: Japanese input keys and the yen sign.
        0xff23 => Key::Henkan,           // Henkan
        0xff22 => Key::Muhenkan,         // Muhenkan
        0xff27 => Key::KatakanaHiragana, // Hiragana_Katakana
        0xff2a => Key::ZenkakuHankaru,   // Zenkaku_Hankaku
        0xff30 => Key::Eisu,             // Eisu_toggle
        0xff2d => Key::Kana,             // Kana_Lock
        0xff26 => Key::KatakanaHiragana, // Katakana
        0xff25 => Key::KatakanaHiragana, // Hiragana
        0xff31 => Key::LanguageSwitch,   // Hangul
        0x00a5 => Key::Yen,              // yen
        _ => Key::Unknown,
    }
}

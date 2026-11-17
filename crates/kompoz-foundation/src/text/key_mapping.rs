//! What a key does in a text field (Compose's KeyCommand and KeyMapping): the commands, the mapping common to the
//! platforms (shortcuts on Ctrl, or Cmd on macOS), the default (Ctrl's words and paragraphs, Linux's and
//! Windows's) and the desktop's per platform (Compose desktop's `DefaultSkikoKeyMapping`, its macOS and Windows
//! mappings); the platform's is [`platform_default_key_mapping`].

use kompoz_ui::input::{Key, KeyEvent, KeyModifiers};
use std::cell::RefCell;
use std::rc::Rc;

/// What a key does in a text field (Compose's KeyCommand).
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum KeyCommand {
    LEFT_CHAR,
    RIGHT_CHAR,
    RIGHT_WORD,
    LEFT_WORD,
    NEXT_PARAGRAPH,
    PREV_PARAGRAPH,
    LINE_START,
    LINE_END,
    LINE_LEFT,
    LINE_RIGHT,
    UP,
    DOWN,
    CENTER,
    PAGE_UP,
    PAGE_DOWN,
    HOME,
    END,
    COPY,
    PASTE,
    CUT,
    DELETE_PREV_CHAR,
    DELETE_NEXT_CHAR,
    DELETE_PREV_WORD,
    DELETE_NEXT_WORD,
    DELETE_FROM_LINE_START,
    DELETE_TO_LINE_END,
    SELECT_ALL,
    SELECT_LEFT_CHAR,
    SELECT_RIGHT_CHAR,
    SELECT_UP,
    SELECT_DOWN,
    SELECT_PAGE_UP,
    SELECT_PAGE_DOWN,
    SELECT_HOME,
    SELECT_END,
    SELECT_LEFT_WORD,
    SELECT_RIGHT_WORD,
    SELECT_NEXT_PARAGRAPH,
    SELECT_PREV_PARAGRAPH,
    SELECT_LINE_START,
    SELECT_LINE_END,
    SELECT_LINE_LEFT,
    SELECT_LINE_RIGHT,
    DESELECT,
    NEW_LINE,
    TAB,
    UNDO,
    REDO,
    CHARACTER_PALETTE,
}

impl KeyCommand {
    /// Whether it changes the text (what a field that can't be edited doesn't do).
    pub fn edits_text(self) -> bool {
        use KeyCommand::*;
        matches!(
            self,
            PASTE
                | CUT
                | DELETE_PREV_CHAR
                | DELETE_NEXT_CHAR
                | DELETE_PREV_WORD
                | DELETE_NEXT_WORD
                | DELETE_FROM_LINE_START
                | DELETE_TO_LINE_END
                | NEW_LINE
                | TAB
                | UNDO
                | REDO
                | CHARACTER_PALETTE
        )
    }
}

/// Maps key events to commands (Compose's KeyMapping).
pub trait KeyMapping {
    fn map(&self, event: &KeyEvent) -> Option<KeyCommand>;
}

impl<F: Fn(&KeyEvent) -> Option<KeyCommand>> KeyMapping for F {
    fn map(&self, event: &KeyEvent) -> Option<KeyCommand> {
        self(event)
    }
}

const NONE: KeyModifiers = KeyModifiers::None;
const SHIFT: KeyModifiers = KeyModifiers::Shift;
const CTRL: KeyModifiers = KeyModifiers::Ctrl;
const ALT: KeyModifiers = KeyModifiers::Alt;
const META: KeyModifiers = KeyModifiers::Meta;

fn ctrl_shift() -> KeyModifiers {
    CTRL | SHIFT
}
fn alt_shift() -> KeyModifiers {
    ALT | SHIFT
}
fn shift_meta() -> KeyModifiers {
    META | SHIFT
}
fn ctrl_alt() -> KeyModifiers {
    CTRL | ALT
}
fn ctrl_meta() -> KeyModifiers {
    CTRL | META
}
fn ctrl_shift_alt() -> KeyModifiers {
    CTRL | SHIFT | ALT
}

/// The modifiers Compose's text key mapping reads of `event`: Alt, Ctrl, Meta and Shift only.
fn modifiers(event: &KeyEvent) -> KeyModifiers {
    KeyModifiers(event.modifiers.0 & (SHIFT | CTRL | ALT | META).0)
}

/// The mapping every platform shares, its shortcuts on `system_shortcut_modifiers` (Compose's
/// `commonKeyMapping`).
pub fn common_key_mapping(system_shortcut_modifiers: KeyModifiers) -> impl KeyMapping {
    move |event: &KeyEvent| -> Option<KeyCommand> {
        use KeyCommand::*;
        let m = modifiers(event);
        let k = event.key;
        if m == system_shortcut_modifiers | SHIFT {
            return (k == Key::Z).then_some(REDO);
        }
        if m == system_shortcut_modifiers {
            return match k {
                Key::C | Key::Insert | Key::NumPadInsert => Some(COPY),
                Key::V => Some(PASTE),
                Key::X => Some(CUT),
                Key::A => Some(SELECT_ALL),
                Key::Y => Some(REDO),
                Key::Z => Some(UNDO),
                _ => None,
            };
        }
        if m == SHIFT {
            return match k {
                Key::DirectionLeft | Key::NumPadDirectionLeft => Some(SELECT_LEFT_CHAR),
                Key::DirectionRight | Key::NumPadDirectionRight => Some(SELECT_RIGHT_CHAR),
                Key::DirectionUp | Key::NumPadDirectionUp => Some(SELECT_UP),
                Key::DirectionDown | Key::NumPadDirectionDown => Some(SELECT_DOWN),
                Key::PageUp | Key::NumPadPageUp => Some(SELECT_PAGE_UP),
                Key::PageDown | Key::NumPadPageDown => Some(SELECT_PAGE_DOWN),
                Key::MoveHome | Key::NumPadMoveHome => Some(SELECT_LINE_START),
                Key::MoveEnd | Key::NumPadMoveEnd => Some(SELECT_LINE_END),
                Key::Insert | Key::NumPadInsert => Some(PASTE),
                _ => None,
            };
        }
        if m == NONE {
            return match k {
                Key::DirectionLeft | Key::NumPadDirectionLeft => Some(LEFT_CHAR),
                Key::DirectionRight | Key::NumPadDirectionRight => Some(RIGHT_CHAR),
                Key::DirectionUp | Key::NumPadDirectionUp => Some(UP),
                Key::DirectionDown | Key::NumPadDirectionDown => Some(DOWN),
                Key::DirectionCenter => Some(CENTER),
                Key::PageUp | Key::NumPadPageUp => Some(PAGE_UP),
                Key::PageDown | Key::NumPadPageDown => Some(PAGE_DOWN),
                Key::MoveHome | Key::NumPadMoveHome => Some(LINE_START),
                Key::MoveEnd | Key::NumPadMoveEnd => Some(LINE_END),
                Key::Enter | Key::NumPadEnter => Some(NEW_LINE),
                Key::Backspace => Some(DELETE_PREV_CHAR),
                Key::Delete => Some(DELETE_NEXT_CHAR),
                Key::Paste => Some(PASTE),
                Key::Cut => Some(CUT),
                Key::Copy => Some(COPY),
                Key::Tab => Some(TAB),
                _ => None,
            };
        }
        None
    }
}

/// The default mapping (Compose's `defaultKeyMapping`): the common one on Ctrl, with Ctrl moving and deleting by
/// words and paragraphs, Ctrl+H a Backspace, Ctrl+\ deselecting, Alt+Delete deleting to the line's end.
pub fn default_key_mapping() -> impl KeyMapping {
    let common = common_key_mapping(CTRL);
    move |event: &KeyEvent| -> Option<KeyCommand> {
        use KeyCommand::*;
        let m = modifiers(event);
        let k = event.key;
        let first = match k {
            Key::Backspace => {
                if m == NONE || m == SHIFT || m == shift_meta() {
                    Some(DELETE_PREV_CHAR)
                } else if m == CTRL || m == ctrl_shift() {
                    Some(DELETE_PREV_WORD)
                } else {
                    None
                }
            }
            Key::Enter | Key::NumPadEnter => {
                (m == NONE || m == SHIFT || m == CTRL || m == ctrl_shift()).then_some(NEW_LINE)
            }
            _ => None,
        };
        if first.is_some() {
            return first;
        }
        let by_modifiers = if m == ctrl_shift() {
            match k {
                Key::DirectionLeft | Key::NumPadDirectionLeft => Some(SELECT_LEFT_WORD),
                Key::DirectionRight | Key::NumPadDirectionRight => Some(SELECT_RIGHT_WORD),
                Key::DirectionUp | Key::NumPadDirectionUp => Some(SELECT_PREV_PARAGRAPH),
                Key::DirectionDown | Key::NumPadDirectionDown => Some(SELECT_NEXT_PARAGRAPH),
                _ => None,
            }
        } else if m == CTRL {
            match k {
                Key::DirectionLeft | Key::NumPadDirectionLeft => Some(LEFT_WORD),
                Key::DirectionRight | Key::NumPadDirectionRight => Some(RIGHT_WORD),
                Key::DirectionUp | Key::NumPadDirectionUp => Some(PREV_PARAGRAPH),
                Key::DirectionDown | Key::NumPadDirectionDown => Some(NEXT_PARAGRAPH),
                Key::H => Some(DELETE_PREV_CHAR),
                Key::Delete => Some(DELETE_NEXT_WORD),
                Key::Backslash => Some(DESELECT),
                _ => None,
            }
        } else if m == SHIFT {
            match k {
                Key::MoveHome | Key::NumPadMoveHome => Some(SELECT_LINE_START),
                Key::MoveEnd | Key::NumPadMoveEnd => Some(SELECT_LINE_END),
                _ => None,
            }
        } else if m == ALT {
            (k == Key::Delete).then_some(DELETE_TO_LINE_END)
        } else {
            None
        };
        by_modifiers.or_else(|| common.map(event))
    }
}

/// The desktop's mapping on Linux and other platforms but macOS and Windows (Compose desktop's
/// `DefaultSkikoKeyMapping`): the default one, Ctrl+Home and Ctrl+End to the text's start and end.
pub fn default_skiko_key_mapping() -> impl KeyMapping {
    let default = default_key_mapping();
    move |event: &KeyEvent| -> Option<KeyCommand> {
        use KeyCommand::*;
        let m = modifiers(event);
        let own = match event.key {
            Key::MoveHome | Key::NumPadMoveHome => {
                if m == CTRL {
                    Some(HOME)
                } else if m == ctrl_shift() {
                    Some(SELECT_HOME)
                } else {
                    None
                }
            }
            Key::MoveEnd | Key::NumPadMoveEnd => {
                if m == CTRL {
                    Some(END)
                } else if m == ctrl_shift() {
                    Some(SELECT_END)
                } else {
                    None
                }
            }
            _ => None,
        };
        own.or_else(|| default.map(event))
    }
}

/// macOS's mapping (Compose desktop's `createMacOsDefaultKeyMapping`): the common one on Cmd, Option moving by
/// words, Cmd to the line's ends and the text's, Emacs' Ctrl keys (as Compose has them: Ctrl+F left and Ctrl+B
/// right), Ctrl+Cmd+Space the character palette.
pub fn mac_os_key_mapping() -> impl KeyMapping {
    let common = common_key_mapping(META);
    move |event: &KeyEvent| -> Option<KeyCommand> {
        use KeyCommand::*;
        let m = modifiers(event);
        let k = event.key;
        let first = match k {
            Key::Delete | Key::NumPadDelete => {
                if m == NONE || m == SHIFT || m == CTRL || m == ctrl_shift() {
                    Some(DELETE_NEXT_CHAR)
                } else if m == ALT || m == alt_shift() {
                    Some(DELETE_NEXT_WORD)
                } else {
                    None
                }
            }
            Key::Backspace => {
                if m == NONE || m == SHIFT || m == CTRL || m == ctrl_shift() {
                    Some(DELETE_PREV_CHAR)
                } else if m == META || m == shift_meta() {
                    Some(DELETE_FROM_LINE_START)
                } else if m == ALT || m == alt_shift() {
                    Some(DELETE_PREV_WORD)
                } else {
                    None
                }
            }
            Key::Enter | Key::NumPadEnter => {
                (m == NONE || m == ALT || m == SHIFT || m == alt_shift()).then_some(NEW_LINE)
            }
            Key::MoveHome | Key::NumPadMoveHome => {
                if m == META {
                    Some(HOME)
                } else if m == shift_meta() {
                    Some(SELECT_HOME)
                } else {
                    None
                }
            }
            Key::MoveEnd | Key::NumPadMoveEnd => {
                if m == META {
                    Some(END)
                } else if m == shift_meta() {
                    Some(SELECT_END)
                } else {
                    None
                }
            }
            _ => None,
        };
        if first.is_some() {
            return first;
        }
        let arrows = |left, right, up, down| match k {
            Key::DirectionLeft | Key::NumPadDirectionLeft => Some(left),
            Key::DirectionRight | Key::NumPadDirectionRight => Some(right),
            Key::DirectionUp | Key::NumPadDirectionUp => Some(up),
            Key::DirectionDown | Key::NumPadDirectionDown => Some(down),
            _ => None,
        };
        let by_modifiers = if m == ctrl_meta() {
            (k == Key::Spacebar).then_some(CHARACTER_PALETTE)
        } else if m == alt_shift() {
            arrows(SELECT_LEFT_WORD, SELECT_RIGHT_WORD, SELECT_PREV_PARAGRAPH, SELECT_NEXT_PARAGRAPH)
        } else if m == shift_meta() {
            arrows(SELECT_LINE_LEFT, SELECT_LINE_RIGHT, SELECT_HOME, SELECT_END)
        } else if m == META {
            arrows(LINE_LEFT, LINE_RIGHT, HOME, END)
        } else if m == ctrl_shift_alt() {
            match k {
                Key::F => Some(SELECT_RIGHT_WORD),
                Key::B => Some(SELECT_LEFT_WORD),
                _ => None,
            }
        } else if m == ctrl_alt() {
            match k {
                Key::F => Some(RIGHT_WORD),
                Key::B => Some(LEFT_WORD),
                _ => None,
            }
        } else if m == ctrl_shift() {
            match k {
                Key::F => Some(SELECT_RIGHT_CHAR),
                Key::B => Some(SELECT_LEFT_CHAR),
                Key::P => Some(SELECT_UP),
                Key::N => Some(SELECT_DOWN),
                Key::A => Some(SELECT_LINE_START),
                Key::E => Some(SELECT_LINE_END),
                _ => None,
            }
        } else if m == CTRL {
            match k {
                Key::F => Some(LEFT_CHAR),
                Key::B => Some(RIGHT_CHAR),
                Key::P => Some(UP),
                Key::N => Some(DOWN),
                Key::A => Some(LINE_START),
                Key::E => Some(LINE_END),
                Key::H => Some(DELETE_PREV_CHAR),
                Key::D => Some(DELETE_NEXT_CHAR),
                Key::K => Some(DELETE_TO_LINE_END),
                Key::O => Some(NEW_LINE),
                _ => None,
            }
        } else if m == SHIFT {
            match k {
                Key::MoveHome | Key::NumPadMoveHome => Some(SELECT_HOME),
                Key::MoveEnd | Key::NumPadMoveEnd => Some(SELECT_END),
                _ => None,
            }
        } else if m == ALT {
            arrows(LEFT_WORD, RIGHT_WORD, PREV_PARAGRAPH, NEXT_PARAGRAPH)
        } else {
            None
        };
        by_modifiers.or_else(|| common.map(event))
    }
}

/// Windows's mapping (Compose desktop's `createWindowsDefaultKeyMapping`): Alt+Backspace undoes, else the
/// desktop's default.
pub fn windows_key_mapping() -> impl KeyMapping {
    let default = default_skiko_key_mapping();
    move |event: &KeyEvent| -> Option<KeyCommand> {
        if modifiers(event) == ALT && event.key == Key::Backspace {
            return Some(KeyCommand::UNDO);
        }
        default.map(event)
    }
}

/// A desktop platform, for its key mapping (Compose desktop's DesktopPlatform).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DesktopPlatform {
    Linux,
    Windows,
    MacOS,
    Unknown,
}

impl DesktopPlatform {
    /// The one running (Compose desktop's `DesktopPlatform.Current`).
    pub fn current() -> DesktopPlatform {
        if cfg!(target_os = "macos") {
            DesktopPlatform::MacOS
        } else if cfg!(target_os = "windows") {
            DesktopPlatform::Windows
        } else if cfg!(target_os = "linux") {
            DesktopPlatform::Linux
        } else {
            DesktopPlatform::Unknown
        }
    }
}

/// `platform`'s mapping (Compose desktop's `createPlatformDefaultKeyMapping`).
pub fn create_platform_default_key_mapping(platform: DesktopPlatform) -> Rc<dyn KeyMapping> {
    match platform {
        DesktopPlatform::MacOS => Rc::new(mac_os_key_mapping()),
        DesktopPlatform::Windows => Rc::new(windows_key_mapping()),
        _ => Rc::new(default_skiko_key_mapping()),
    }
}

thread_local! {
    static KEY_MAPPING_OVERRIDE: RefCell<Option<Rc<dyn KeyMapping>>> = const { RefCell::new(None) };
    static PLATFORM_DEFAULT: Rc<dyn KeyMapping> = create_platform_default_key_mapping(DesktopPlatform::current());
}

/// The mapping text fields use (Compose's `platformDefaultKeyMapping`): the running platform's, unless overridden.
pub fn platform_default_key_mapping() -> Rc<dyn KeyMapping> {
    KEY_MAPPING_OVERRIDE.with(|o| o.borrow().clone()).unwrap_or_else(|| PLATFORM_DEFAULT.with(|m| m.clone()))
}

/// Overrides the mapping text fields use on this thread (Compose desktop's `keyMappingOverride`; none: the
/// platform's). Tests use it to try another platform's.
pub fn set_key_mapping_override(mapping: Option<Rc<dyn KeyMapping>>) {
    KEY_MAPPING_OVERRIDE.with(|o| *o.borrow_mut() = mapping);
}

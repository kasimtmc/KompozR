//! What a text field asks of a key event (Compose's `isTypedEvent`, `isFromSoftKeyboard`, `isFromHardwareSource`,
//! `cancelsTextSelection`) and the platform's character palette (`showCharacterPalette`).
//!
//! Typed characters: Compose desktop types a character from AWT's KEY_TYPED event (a key event of type Unknown
//! there), while key presses (KeyDown) only map to commands. KompozR's desktop platforms hand typed text to the
//! focused field's input session instead (xkb or XIM composing it: `KompozHost::text_input`, a CommitText), so
//! their KeyDowns, which carry the code point for shortcuts, are not typed events either. A key event of type
//! [`KeyEventType::Unknown`] with a printable code point is one (what a test or a platform sends for a KEY_TYPED):
//! a field types it, through its [`DeadKeyCombiner`](super::DeadKeyCombiner).

use kompoz_ui::input::{KeyEvent, KeyEventType};

/// Whether a code point is an ISO control character (Java's `Character.isISOControl`).
fn is_iso_control(code_point: i32) -> bool {
    (0..=0x1F).contains(&code_point) || (0x7F..=0x9F).contains(&code_point)
}

/// Whether `event` types a character (Compose desktop's `isTypedEvent`: a KEY_TYPED event with a printable
/// character, here an Unknown-typed event whose code point isn't a control character — a dead key's flagged code
/// point is one).
pub fn is_typed_event(event: &KeyEvent) -> bool {
    event.r#type == KeyEventType::Unknown && !is_iso_control(event.utf16_code_point)
}

/// Whether `event` came from an on-screen keyboard (Compose's `isFromSoftKeyboard`: never on the desktop).
pub fn is_from_soft_keyboard(event: &KeyEvent) -> bool {
    let _ = event;
    false
}

/// Whether `event` came from a physical keyboard (Compose's `isFromHardwareSource`: the desktop's keys are).
pub fn is_from_hardware_source(event: &KeyEvent) -> bool {
    matches!(event.r#type, KeyEventType::KeyDown | KeyEventType::KeyUp)
}

/// Whether `event` cancels a field's selection before the field sees it (Compose desktop's
/// `cancelsTextSelection`: no key does).
pub fn cancels_text_selection(event: &KeyEvent) -> bool {
    let _ = event;
    false
}

/// Opens the platform's emoji and symbols palette (Compose desktop's `showCharacterPalette`: macOS's
/// `orderFrontCharacterPalette`; Linux and Windows have none a program opens, and their mapping has no key for
/// it).
pub fn show_character_palette() {}

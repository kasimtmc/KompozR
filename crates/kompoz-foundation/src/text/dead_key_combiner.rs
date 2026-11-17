//! Combines a dead key with the key after it (Compose's DeadKeyCombiner, its Android implementation over
//! `KeyCharacterMap`'s accents): a typed event whose code point carries [`COMBINING_ACCENT`] is a dead key, kept
//! until the next one, which then types the accented character (or, if they don't combine, the next key as it is;
//! the same dead key twice or a space after it types the accent itself).
//!
//! KompozR's desktop platforms compose dead keys themselves (xkb's compose tables, the X input method), as Compose
//! desktop's AWT does — what reaches a field is the composed character: no event carries the flag there, and the
//! combiner hands every code point on, as Compose desktop's does.

use kompoz_ui::input::KeyEvent;

/// The flag a dead key's code point carries (Android's `KeyCharacterMap.COMBINING_ACCENT`).
pub const COMBINING_ACCENT: i32 = 0x8000_0000_u32 as i32;
/// What is left of a dead key's code point without the flag: its accent (Android's `COMBINING_ACCENT_MASK`).
pub const COMBINING_ACCENT_MASK: i32 = 0x7FFF_FFFF;

/// Android's accents: (combining mark, accent, fallback typed alone).
const COMBINING: &[(char, char, char)] = &[
    ('\u{0300}', '\u{02CB}', '`'),
    ('\u{0301}', '\u{00B4}', '\''),
    ('\u{0302}', '\u{02C6}', '^'),
    ('\u{0303}', '\u{02DC}', '~'),
    ('\u{0304}', '\u{00AF}', '\u{00AF}'),
    ('\u{0306}', '\u{02D8}', '\u{02D8}'),
    ('\u{0307}', '\u{02D9}', '\u{02D9}'),
    ('\u{0308}', '\u{00A8}', '"'),
    ('\u{0309}', '\u{02C0}', '\u{02C0}'),
    ('\u{030A}', '\u{02DA}', '\u{02DA}'),
    ('\u{030B}', '\u{02DD}', '\u{02DD}'),
    ('\u{030C}', '\u{02C7}', '\u{02C7}'),
    ('\u{0312}', '\u{02BB}', '\u{02BB}'),
    ('\u{0313}', '\u{1FBD}', '\u{1FBD}'),
    ('\u{0314}', '\u{02BD}', '\u{02BD}'),
    ('\u{0315}', '\u{02BC}', '\u{02BC}'),
    ('\u{031B}', '\'', '\''),
    ('\u{0323}', '.', '.'),
    ('\u{0327}', '\u{00B8}', '\u{00B8}'),
    ('\u{0328}', '\u{02DB}', '\u{02DB}'),
    ('\u{0329}', '\u{02CC}', '\u{02CC}'),
    ('\u{0331}', '\u{02CD}', '\u{02CD}'),
    ('\u{0335}', '-', '-'),
];

/// Legacy and one-way accents mapped to their combining marks (Android's extra `sAccentToCombining` entries).
const EXTRA_ACCENTS: &[(char, char)] =
    &[('`', '\u{0300}'), ('^', '\u{0302}'), ('~', '\u{0303}'), ('\'', '\u{0301}'), ('"', '\u{0308}')];

/// The stroke accent's non-standard combinations (Android's `addDeadKey(ACCENT_STROKE, …)`).
const STROKES: &[(char, char)] = &[
    ('D', '\u{0110}'),
    ('G', '\u{01e4}'),
    ('H', '\u{0126}'),
    ('I', '\u{0197}'),
    ('L', '\u{0141}'),
    ('O', '\u{00d8}'),
    ('T', '\u{0166}'),
    ('d', '\u{0111}'),
    ('g', '\u{01e5}'),
    ('h', '\u{0127}'),
    ('i', '\u{0268}'),
    ('l', '\u{0142}'),
    ('o', '\u{00f8}'),
    ('t', '\u{0167}'),
];

/// The combining mark of `accent` (Android's `sAccentToCombining`; the table's last entry for an accent wins, as
/// a SparseIntArray's `append` keeps the last).
fn combining_of(accent: u32) -> Option<char> {
    let extra = EXTRA_ACCENTS.iter().rev().find(|(a, _)| *a as u32 == accent).map(|&(_, c)| c);
    extra.or_else(|| COMBINING.iter().rev().find(|(_, a, _)| *a as u32 == accent).map(|&(c, _, _)| c))
}

/// What `accent` types alone (Android's `sAccentToFallback`, the accent itself if it has none).
fn fallback_of(accent: u32) -> u32 {
    COMBINING.iter().find(|(_, a, _)| *a as u32 == accent).map_or(accent, |&(_, _, f)| f as u32)
}

/// The character `accent` and `c` make (Android's `KeyCharacterMap.getDeadChar`): the accent's fallback for the
/// accent again or a space, a stroke's own combinations, else the NFC of `c` and the accent's combining mark if it
/// is one character; 0 if they don't combine.
pub fn get_dead_char(accent: u32, c: u32) -> u32 {
    if c == accent || c == ' ' as u32 {
        return fallback_of(accent);
    }
    let Some(combining) = combining_of(accent) else { return 0 };
    if combining == '\u{0335}'
        && let Some(&(_, result)) = STROKES.iter().find(|(base, _)| *base as u32 == c)
    {
        return result as u32;
    }
    let Some(base) = char::from_u32(c) else { return 0 };
    let composed = kompoz_text::normalize::to_nfc(&format!("{base}{combining}"));
    let mut chars = composed.chars();
    match (chars.next(), chars.next()) {
        (Some(one), None) => one as u32,
        _ => 0,
    }
}

/// Combines dead keys with the key after them (Compose's DeadKeyCombiner).
#[derive(Clone, Debug, Default)]
pub struct DeadKeyCombiner {
    dead_key_code: Option<i32>,
}

impl DeadKeyCombiner {
    pub fn new() -> DeadKeyCombiner {
        DeadKeyCombiner::default()
    }

    /// What `event` types: none for a dead key (kept for the next one), the combined character after one (or the
    /// key as it is if they don't combine), else the key's code point.
    pub fn consume(&mut self, event: &KeyEvent) -> Option<i32> {
        let code_point = event.utf16_code_point;
        if code_point & COMBINING_ACCENT != 0 {
            self.dead_key_code = Some(code_point & COMBINING_ACCENT_MASK);
            return None;
        }
        if let Some(dead) = self.dead_key_code.take() {
            let combined = get_dead_char(dead as u32, code_point as u32);
            return Some(if combined == 0 { code_point } else { combined as i32 });
        }
        Some(code_point)
    }
}

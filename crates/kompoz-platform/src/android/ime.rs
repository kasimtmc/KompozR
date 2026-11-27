//! What the input method is told and what it does, in KompozR's terms (C++'s `android_ime` mapping and Compose's
//! `EditorInfo.update`, `RecordingInputConnection`): a field's [`ImeOptions`] as `EditorInfo`'s input type, IME
//! options, private options and hint locales ([`EditorConfig`]); an editor action back as Compose's [`ImeAction`];
//! the field's state in Java's UTF-16 offsets and back ([`Utf16State`]); a batch of the input connection's edits
//! ([`EditOp`]: what KompozIme's connection did to its Editable, UTF-16 offsets as Android gives them) as the
//! host's [`EditCommand`]s, each converted against the text as the edits before it left it (UTF-8 bytes, whole
//! characters).
//!
//! KompozIme's Editable is the input method's source of truth (Compose's and C++'s model): it answers the input
//! method at once (the text around the caret, the extracted text), and each batch of edits comes here with what it
//! left. The field takes the edits as commands (its filters and transformations run); where it ends up elsewhere,
//! the Editable is told again. Pure: tested on the host.

use kompoz_ui::platform::{ImeAction, ImeOptions, KeyboardCapitalization, KeyboardType, TextEditState};
use kompoz_ui::text_input::EditCommand;

/// `android.text.InputType`'s bits.
pub mod input_type {
    pub const CLASS_TEXT: i32 = 1;
    pub const CLASS_NUMBER: i32 = 2;
    pub const CLASS_PHONE: i32 = 3;
    pub const CLASS_DATETIME: i32 = 4;
    pub const MASK_CLASS: i32 = 0xf;
    pub const TEXT_VARIATION_URI: i32 = 0x10;
    pub const TEXT_VARIATION_EMAIL_ADDRESS: i32 = 0x20;
    pub const TEXT_VARIATION_EMAIL_SUBJECT: i32 = 0x30;
    pub const TEXT_VARIATION_SHORT_MESSAGE: i32 = 0x40;
    pub const TEXT_VARIATION_LONG_MESSAGE: i32 = 0x50;
    pub const TEXT_VARIATION_PERSON_NAME: i32 = 0x60;
    pub const TEXT_VARIATION_POSTAL_ADDRESS: i32 = 0x70;
    pub const TEXT_VARIATION_PASSWORD: i32 = 0x80;
    pub const TEXT_VARIATION_VISIBLE_PASSWORD: i32 = 0x90;
    pub const TEXT_VARIATION_FILTER: i32 = 0xb0;
    pub const TEXT_VARIATION_PHONETIC: i32 = 0xc0;
    pub const NUMBER_VARIATION_PASSWORD: i32 = 0x10;
    pub const NUMBER_FLAG_SIGNED: i32 = 0x1000;
    pub const NUMBER_FLAG_DECIMAL: i32 = 0x2000;
    pub const DATETIME_VARIATION_DATE: i32 = 0x10;
    pub const DATETIME_VARIATION_TIME: i32 = 0x20;
    pub const TEXT_FLAG_CAP_CHARACTERS: i32 = 0x1000;
    pub const TEXT_FLAG_CAP_WORDS: i32 = 0x2000;
    pub const TEXT_FLAG_CAP_SENTENCES: i32 = 0x4000;
    pub const TEXT_FLAG_AUTO_CORRECT: i32 = 0x8000;
    pub const TEXT_FLAG_MULTI_LINE: i32 = 0x20000;
}

/// `EditorInfo`'s IME options and actions.
pub mod editor {
    pub const ACTION_UNSPECIFIED: i32 = 0;
    pub const ACTION_NONE: i32 = 1;
    pub const ACTION_GO: i32 = 2;
    pub const ACTION_SEARCH: i32 = 3;
    pub const ACTION_SEND: i32 = 4;
    pub const ACTION_NEXT: i32 = 5;
    pub const ACTION_DONE: i32 = 6;
    pub const ACTION_PREVIOUS: i32 = 7;
    pub const FLAG_NO_FULLSCREEN: i32 = 0x0200_0000;
    pub const FLAG_NO_ENTER_ACTION: i32 = 0x4000_0000;
    pub const FLAG_FORCE_ASCII: i32 = i32::MIN; // 0x8000_0000
}

/// What KompozIme puts in the `EditorInfo` of a field's input connection (Compose's `EditorInfo.update`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EditorConfig {
    pub input_type: i32,
    pub ime_options: i32,
    /// `EditorInfo.privateImeOptions` (none: not set).
    pub private_ime_options: Option<String>,
    /// The languages the keyboard is hinted to type in, BCP 47 tags (`EditorInfo.hintLocales`; empty: none).
    pub hint_locales: Vec<String>,
    /// The field takes content besides text (`contentMimeTypes` `*/*`: a content receiver around it).
    pub accepts_content: bool,
    /// The input method may take handwriting into it (`setStylusHandwritingEnabled`): where handwriting is supported,
    /// not for a password.
    pub stylus_handwriting: bool,
}

impl EditorConfig {
    /// The `EditorInfo` of a field with `options` (`accepts_content`: under a content receiver;
    /// `handwriting_supported`: Android 14 and later).
    pub fn of(options: &ImeOptions, accepts_content: bool, handwriting_supported: bool) -> EditorConfig {
        use input_type::*;
        let action = match options.ime_action {
            ImeAction::Default | ImeAction::Unspecified => {
                // (The last resort to keep a single line: Android's keyboards show Enter otherwise.)
                if options.single_line { editor::ACTION_DONE } else { editor::ACTION_UNSPECIFIED }
            }
            ImeAction::None => editor::ACTION_NONE,
            ImeAction::Go => editor::ACTION_GO,
            ImeAction::Next => editor::ACTION_NEXT,
            ImeAction::Previous => editor::ACTION_PREVIOUS,
            ImeAction::Search => editor::ACTION_SEARCH,
            ImeAction::Send => editor::ACTION_SEND,
            ImeAction::Done => editor::ACTION_DONE,
        };
        let mut ime_options = action;
        let mut input_type = match options.keyboard_type {
            KeyboardType::Text | KeyboardType::Unspecified => CLASS_TEXT,
            KeyboardType::Ascii => {
                ime_options |= editor::FLAG_FORCE_ASCII;
                CLASS_TEXT
            }
            KeyboardType::Number => CLASS_NUMBER,
            KeyboardType::Phone => CLASS_PHONE,
            KeyboardType::Uri => CLASS_TEXT | TEXT_VARIATION_URI,
            KeyboardType::Email => CLASS_TEXT | TEXT_VARIATION_EMAIL_ADDRESS,
            KeyboardType::Password => CLASS_TEXT | TEXT_VARIATION_PASSWORD,
            KeyboardType::NumberPassword => CLASS_NUMBER | NUMBER_VARIATION_PASSWORD,
            KeyboardType::Decimal => CLASS_NUMBER | NUMBER_FLAG_DECIMAL,
            KeyboardType::PasswordVisible => CLASS_TEXT | TEXT_VARIATION_VISIBLE_PASSWORD,
            KeyboardType::PostalAddress => CLASS_TEXT | TEXT_VARIATION_POSTAL_ADDRESS,
            KeyboardType::PersonName => CLASS_TEXT | TEXT_VARIATION_PERSON_NAME,
            KeyboardType::EmailSubject => CLASS_TEXT | TEXT_VARIATION_EMAIL_SUBJECT,
            KeyboardType::ShortMessage => CLASS_TEXT | TEXT_VARIATION_SHORT_MESSAGE,
            KeyboardType::LongMessage => CLASS_TEXT | TEXT_VARIATION_LONG_MESSAGE,
            KeyboardType::Filter => CLASS_TEXT | TEXT_VARIATION_FILTER,
            KeyboardType::Phonetic => CLASS_TEXT | TEXT_VARIATION_PHONETIC,
            KeyboardType::DateTime => CLASS_DATETIME,
            KeyboardType::Date => CLASS_DATETIME | DATETIME_VARIATION_DATE,
            KeyboardType::Time => CLASS_DATETIME | DATETIME_VARIATION_TIME,
            KeyboardType::NumberSigned => CLASS_NUMBER | NUMBER_FLAG_SIGNED,
            KeyboardType::DecimalSigned => CLASS_NUMBER | NUMBER_FLAG_DECIMAL | NUMBER_FLAG_SIGNED,
            KeyboardType::DecimalPassword => CLASS_NUMBER | NUMBER_VARIATION_PASSWORD | NUMBER_FLAG_DECIMAL,
            KeyboardType::NumberPasswordSigned => CLASS_NUMBER | NUMBER_VARIATION_PASSWORD | NUMBER_FLAG_SIGNED,
            KeyboardType::DecimalPasswordSigned => {
                CLASS_NUMBER | NUMBER_VARIATION_PASSWORD | NUMBER_FLAG_DECIMAL | NUMBER_FLAG_SIGNED
            }
        };
        let text = input_type & MASK_CLASS == CLASS_TEXT;
        if !options.single_line && text {
            // (TextView's setInputTypeSingleLine.)
            input_type |= TEXT_FLAG_MULTI_LINE;
            if matches!(options.ime_action, ImeAction::Default | ImeAction::Unspecified) {
                ime_options |= editor::FLAG_NO_ENTER_ACTION;
            }
        }
        if text {
            input_type |= match options.capitalization {
                KeyboardCapitalization::Characters => TEXT_FLAG_CAP_CHARACTERS,
                KeyboardCapitalization::Words => TEXT_FLAG_CAP_WORDS,
                KeyboardCapitalization::Sentences => TEXT_FLAG_CAP_SENTENCES,
                KeyboardCapitalization::None | KeyboardCapitalization::Unspecified => 0,
            };
            if options.auto_correct {
                input_type |= TEXT_FLAG_AUTO_CORRECT;
            }
        }
        ime_options |= editor::FLAG_NO_FULLSCREEN;
        let password = matches!(
            options.keyboard_type,
            KeyboardType::Password
                | KeyboardType::PasswordVisible
                | KeyboardType::NumberPassword
                | KeyboardType::DecimalPassword
                | KeyboardType::NumberPasswordSigned
                | KeyboardType::DecimalPasswordSigned
        );
        EditorConfig {
            input_type,
            ime_options,
            private_ime_options: options.platform_ime_options.as_ref().and_then(|o| o.private_ime_options.clone()),
            hint_locales: options.hint_locales.iter().map(|l| l.to_language_tag()).collect(),
            accepts_content,
            stylus_handwriting: handwriting_supported && !password,
        }
    }
}

impl EditorConfig {
    /// KompozIme's `imeShow` flags: content taken (1), handwriting (2), the keyboard shown (4).
    pub fn flags(&self, show_keyboard: bool) -> i32 {
        (self.accepts_content as i32) | (self.stylus_handwriting as i32) << 1 | (show_keyboard as i32) << 2
    }
}

/// The action key the input method pressed (`performEditorAction`'s) as Compose's (RecordingInputConnection's
/// `performEditorAction`: what it doesn't know is the default action).
pub fn ime_action_of(editor_action: i32) -> ImeAction {
    match editor_action {
        editor::ACTION_NONE => ImeAction::None,
        editor::ACTION_GO => ImeAction::Go,
        editor::ACTION_SEARCH => ImeAction::Search,
        editor::ACTION_SEND => ImeAction::Send,
        editor::ACTION_NEXT => ImeAction::Next,
        editor::ACTION_DONE => ImeAction::Done,
        editor::ACTION_PREVIOUS => ImeAction::Previous,
        _ => ImeAction::Default,
    }
}

// ---- UTF-16 offsets ----

/// The byte offset in `text` of UTF-16 offset `u16` (clamped to the text; one inside a surrogate pair: the
/// character's start).
pub fn byte_of_utf16(text: &str, u16: i32) -> usize {
    if u16 <= 0 {
        return 0;
    }
    let mut units = 0;
    for (at, c) in text.char_indices() {
        let next = units + c.len_utf16();
        if next > u16 as usize {
            return at;
        }
        units = next;
    }
    text.len()
}

/// The UTF-16 offset of byte offset `byte` of `text` (a byte inside a character: its start).
pub fn utf16_of_byte(text: &str, byte: usize) -> i32 {
    let mut at = byte.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    text[..at].chars().map(char::len_utf16).sum::<usize>() as i32
}

/// A field's state in UTF-16 offsets, as Java's Editable has it (no composition: -1, -1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Utf16State {
    pub selection_start: i32,
    pub selection_end: i32,
    pub composition_start: i32,
    pub composition_end: i32,
}

impl Utf16State {
    pub fn of(state: &TextEditState) -> Utf16State {
        let (cs, ce) = match state.composition {
            Some((s, e)) => (utf16_of_byte(&state.text, s), utf16_of_byte(&state.text, e)),
            None => (-1, -1),
        };
        Utf16State {
            selection_start: utf16_of_byte(&state.text, state.selection_start),
            selection_end: utf16_of_byte(&state.text, state.selection_end),
            composition_start: cs,
            composition_end: ce,
        }
    }

    /// `text` with these offsets as KompozR's state (a composition empty or negative: none).
    pub fn state(&self, text: String) -> TextEditState {
        let composition = (self.composition_start >= 0 && self.composition_end >= 0)
            .then(|| {
                let (s, e) = (byte_of_utf16(&text, self.composition_start), byte_of_utf16(&text, self.composition_end));
                (s.min(e), s.max(e))
            })
            .filter(|(s, e)| s < e);
        TextEditState {
            selection_start: byte_of_utf16(&text, self.selection_start),
            selection_end: byte_of_utf16(&text, self.selection_end),
            composition,
            text,
        }
    }
}

// ---- A batch of edits ----

/// What KompozIme's input connection did, in order, as it records it (UTF-16 offsets and lengths).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EditOp {
    CommitText { text: String, new_cursor_position: i32 },
    SetComposingText { text: String, new_cursor_position: i32 },
    SetComposingRegion { start: i32, end: i32 },
    FinishComposingText,
    DeleteSurroundingText { before: i32, after: i32 },
    DeleteSurroundingTextInCodePoints { before: i32, after: i32 },
    SetSelection { start: i32, end: i32 },
}

/// The codes of KompozInputConnection's recorded edits (its `OP_*`), four ints each: the code, two arguments, the
/// length in bytes of its text (the texts one after the other in a byte array).
pub mod op {
    pub const COMMIT_TEXT: i32 = 1;
    pub const SET_COMPOSING_TEXT: i32 = 2;
    pub const SET_COMPOSING_REGION: i32 = 3;
    pub const FINISH_COMPOSING_TEXT: i32 = 4;
    pub const DELETE_SURROUNDING_TEXT: i32 = 5;
    pub const DELETE_SURROUNDING_TEXT_IN_CODE_POINTS: i32 = 6;
    pub const SET_SELECTION: i32 = 7;
}

/// The edits KompozInputConnection recorded (`ops`: four ints an edit; `texts`: their texts, UTF-8, one after the
/// other); what doesn't decode (an unknown code, a text past the bytes or not UTF-8) ends them there.
pub fn decode_ops(ops: &[i32], texts: &[u8]) -> Vec<EditOp> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for e in ops.as_chunks::<4>().0 {
        let (code, a, b, len) = (e[0], e[1], e[2], e[3].max(0) as usize);
        let Some(bytes) = texts.get(at..at + len) else { break };
        let Ok(text) = std::str::from_utf8(bytes) else { break };
        at += len;
        out.push(match code {
            op::COMMIT_TEXT => EditOp::CommitText { text: text.to_string(), new_cursor_position: a },
            op::SET_COMPOSING_TEXT => EditOp::SetComposingText { text: text.to_string(), new_cursor_position: a },
            op::SET_COMPOSING_REGION => EditOp::SetComposingRegion { start: a, end: b },
            op::FINISH_COMPOSING_TEXT => EditOp::FinishComposingText,
            op::DELETE_SURROUNDING_TEXT => EditOp::DeleteSurroundingText { before: a, after: b },
            op::DELETE_SURROUNDING_TEXT_IN_CODE_POINTS => {
                EditOp::DeleteSurroundingTextInCodePoints { before: a, after: b }
            }
            op::SET_SELECTION => EditOp::SetSelection { start: a, end: b },
            _ => break,
        });
    }
    out
}

/// How many bytes `units` UTF-16 units before byte `from` of `text` are (whole characters: one cut in half is
/// taken whole).
fn bytes_before(text: &str, from: usize, units: i32) -> usize {
    let mut left = units.max(0) as usize;
    let mut at = from.min(text.len());
    for c in text[..at].chars().rev() {
        if left == 0 {
            break;
        }
        left = left.saturating_sub(c.len_utf16());
        at -= c.len_utf8();
    }
    from.min(text.len()) - at
}

/// How many bytes `units` UTF-16 units after byte `from` of `text` are (whole characters).
fn bytes_after(text: &str, from: usize, units: i32) -> usize {
    let mut left = units.max(0) as usize;
    let start = from.min(text.len());
    let mut at = start;
    for c in text[start..].chars() {
        if left == 0 {
            break;
        }
        left = left.saturating_sub(c.len_utf16());
        at += c.len_utf8();
    }
    at - start
}

/// `ops` as the host's commands, each converted against `state` as the ones before it left it (which it is
/// advanced to).
pub fn commands_of(state: &mut TextEditState, ops: &[EditOp]) -> Vec<EditCommand> {
    let mut out = Vec::with_capacity(ops.len());
    for o in ops {
        let text = &state.text;
        let command = match o {
            EditOp::CommitText { text, new_cursor_position } => {
                EditCommand::CommitText { text: text.clone(), new_cursor_position: *new_cursor_position }
            }
            EditOp::SetComposingText { text, new_cursor_position } => {
                EditCommand::SetComposingText { text: text.clone(), new_cursor_position: *new_cursor_position }
            }
            EditOp::SetComposingRegion { start, end } => {
                EditCommand::SetComposingRegion { start: byte_of_utf16(text, *start), end: byte_of_utf16(text, *end) }
            }
            EditOp::FinishComposingText => EditCommand::FinishComposingText,
            EditOp::DeleteSurroundingText { before, after } => {
                let (lo, hi) =
                    (state.selection_start.min(state.selection_end), state.selection_start.max(state.selection_end));
                EditCommand::DeleteSurroundingText {
                    before: bytes_before(text, lo, *before),
                    after: bytes_after(text, hi, *after),
                }
            }
            EditOp::DeleteSurroundingTextInCodePoints { before, after } => {
                EditCommand::DeleteSurroundingTextInCodePoints {
                    before: (*before).max(0) as usize,
                    after: (*after).max(0) as usize,
                }
            }
            EditOp::SetSelection { start, end } => {
                EditCommand::SetSelection { start: byte_of_utf16(text, *start), end: byte_of_utf16(text, *end) }
            }
        };
        state.apply(&command);
        out.push(command);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_ui::platform::PlatformImeOptions;

    #[test]
    fn ime_options_are_editor_infos() {
        use input_type::*;
        let text = EditorConfig::of(&ImeOptions::default(), false, true);
        assert_eq!(text.input_type, CLASS_TEXT | TEXT_FLAG_AUTO_CORRECT);
        assert_eq!(text.ime_options, editor::ACTION_DONE | editor::FLAG_NO_FULLSCREEN);
        assert!(text.stylus_handwriting && !text.accepts_content);
        // A multi-line field with the default action: Enter makes lines, no action key.
        let multi =
            ImeOptions { single_line: false, capitalization: KeyboardCapitalization::Sentences, ..Default::default() };
        let multi = EditorConfig::of(&multi, true, false);
        assert_eq!(
            multi.input_type,
            CLASS_TEXT | TEXT_FLAG_MULTI_LINE | TEXT_FLAG_CAP_SENTENCES | TEXT_FLAG_AUTO_CORRECT
        );
        assert_eq!(multi.ime_options, editor::FLAG_NO_ENTER_ACTION | editor::FLAG_NO_FULLSCREEN);
        assert!(multi.accepts_content && !multi.stylus_handwriting);
        // A password: no correction, no handwriting; a number: no text flags.
        let password = ImeOptions {
            keyboard_type: KeyboardType::Password,
            ime_action: ImeAction::Go,
            auto_correct: false,
            ..Default::default()
        };
        let password = EditorConfig::of(&password, false, true);
        assert_eq!(password.input_type, CLASS_TEXT | TEXT_VARIATION_PASSWORD);
        assert_eq!(password.ime_options, editor::ACTION_GO | editor::FLAG_NO_FULLSCREEN);
        assert!(!password.stylus_handwriting);
        let number = ImeOptions {
            keyboard_type: KeyboardType::DecimalPasswordSigned,
            capitalization: KeyboardCapitalization::Words,
            single_line: false,
            ..Default::default()
        };
        assert_eq!(
            EditorConfig::of(&number, false, true).input_type,
            CLASS_NUMBER | NUMBER_VARIATION_PASSWORD | NUMBER_FLAG_DECIMAL | NUMBER_FLAG_SIGNED
        );
        // ASCII forces the keyboard's ASCII; private options and hint locales go as they are.
        let ascii = ImeOptions {
            keyboard_type: KeyboardType::Ascii,
            ime_action: ImeAction::Search,
            platform_ime_options: Some(PlatformImeOptions { private_ime_options: Some("nm".into()) }),
            hint_locales: kompoz_text::LocaleList::from_tags("tr-TR,en-US"),
            ..Default::default()
        };
        let ascii = EditorConfig::of(&ascii, false, false);
        assert_eq!(ascii.ime_options, editor::ACTION_SEARCH | editor::FLAG_FORCE_ASCII | editor::FLAG_NO_FULLSCREEN);
        assert_eq!(ascii.private_ime_options.as_deref(), Some("nm"));
        assert_eq!(ascii.hint_locales, vec!["tr-TR".to_string(), "en-US".to_string()]);
        assert_eq!(
            EditorConfig::of(&ImeOptions { keyboard_type: KeyboardType::Date, ..Default::default() }, false, false)
                .input_type,
            CLASS_DATETIME | DATETIME_VARIATION_DATE
        );
        assert_eq!(ime_action_of(editor::ACTION_NEXT), ImeAction::Next);
        assert_eq!(ime_action_of(editor::ACTION_UNSPECIFIED), ImeAction::Default);
        assert_eq!(ime_action_of(99), ImeAction::Default);
    }

    #[test]
    fn utf16_offsets_are_bytes() {
        let s = "aç😀b";
        // a: 1 byte 1 unit; ç: 2 bytes 1 unit; 😀: 4 bytes 2 units; b.
        assert_eq!((0..=5).map(|u| byte_of_utf16(s, u)).collect::<Vec<_>>(), vec![0, 1, 3, 3, 7, 8]);
        assert_eq!(byte_of_utf16(s, 99), 8);
        assert_eq!(byte_of_utf16(s, -1), 0);
        assert_eq!([0, 1, 3, 5, 7, 8].map(|b| utf16_of_byte(s, b)), [0, 1, 2, 2, 4, 5]);
        let state = TextEditState { text: s.into(), selection_start: 7, selection_end: 3, composition: Some((1, 7)) };
        let u = Utf16State::of(&state);
        assert_eq!(u, Utf16State { selection_start: 4, selection_end: 2, composition_start: 1, composition_end: 4 });
        assert_eq!(u.state(s.into()), state);
        let none = Utf16State { selection_start: 1, selection_end: 1, composition_start: -1, composition_end: -1 };
        assert_eq!(none.state("ab".into()).composition, None);
        let empty = Utf16State { composition_start: 1, composition_end: 1, ..none };
        assert_eq!(empty.state("ab".into()).composition, None);
    }

    #[test]
    fn a_batch_of_edits_applies_on_utf16_offsets() {
        // The texts one after the other, four ints an edit.
        let ops = [
            op::SET_COMPOSING_TEXT,
            1,
            0,
            4, // "çay"
            op::FINISH_COMPOSING_TEXT,
            0,
            0,
            0,
            op::COMMIT_TEXT,
            1,
            0,
            5, // " 😀"
            op::SET_SELECTION,
            2,
            2,
            0,
            op::DELETE_SURROUNDING_TEXT,
            1,
            3,
            0,
            op::SET_COMPOSING_REGION,
            0,
            1,
            0,
            op::DELETE_SURROUNDING_TEXT_IN_CODE_POINTS,
            0,
            1,
            0,
        ];
        let texts = "çay 😀".as_bytes();
        let decoded = decode_ops(&ops, texts);
        assert_eq!(decoded.len(), 7);
        assert_eq!(decoded[2], EditOp::CommitText { text: " 😀".into(), new_cursor_position: 1 });
        let mut state = TextEditState::default();
        let commands = commands_of(&mut state, &decoded);
        // "çay 😀", the caret at UTF-16 offset 2 (after "ça": byte 3); one unit before it ("a") and three after
        // it ("y", " ", half the emoji: taken whole, 6 bytes) deleted: "ç", the caret at its end, then composing.
        assert_eq!(commands[3], EditCommand::SetSelection { start: 3, end: 3 });
        assert_eq!(commands[4], EditCommand::DeleteSurroundingText { before: 1, after: 6 });
        assert_eq!(commands[5], EditCommand::SetComposingRegion { start: 0, end: 2 });
        assert_eq!(state.text, "ç");
        assert_eq!((state.selection_start, state.selection_end), (2, 2));
        assert_eq!(state.composition, Some((0, 2)));
        // An unknown code or a text past the bytes ends the batch there.
        assert_eq!(decode_ops(&[op::COMMIT_TEXT, 1, 0, 9], b"ab"), vec![]);
        assert_eq!(decode_ops(&[op::FINISH_COMPOSING_TEXT, 0, 0, 0, 42, 0, 0, 0], b"").len(), 1);
    }
}

//! The editing core both text field engines share (Compose's ui-text `EditingBuffer`, `EditCommand`s and
//! `EditProcessor`), in UTF-8 bytes on whole characters, and NFC (what Android's dead keys compose through).

use kompoz_text::TextRange;
use kompoz_text::input::{EditCommand, EditProcessor, EditingBuffer, TextFieldValue, update_range_after_delete};
use kompoz_text::normalize::to_nfc;

fn buffer(text: &str, selection: (usize, usize), composition: Option<(usize, usize)>) -> EditingBuffer {
    let mut b = EditingBuffer::from_text(text, TextRange::new(selection.0, selection.1));
    if let Some((s, e)) = composition {
        b.set_composition(s, e);
    }
    b
}

fn applied(mut b: EditingBuffer, commands: &[EditCommand]) -> (String, TextRange, Option<TextRange>) {
    for c in commands {
        c.apply_to(&mut b);
    }
    (b.text().to_string(), b.selection(), b.composition())
}

fn commit(text: &str, p: i32) -> EditCommand {
    EditCommand::CommitText { text: text.into(), new_cursor_position: p }
}

fn compose(text: &str, p: i32) -> EditCommand {
    EditCommand::SetComposingText { text: text.into(), new_cursor_position: p }
}

#[test]
fn commits_and_compositions_replace_the_composition_or_the_selection() {
    let r = TextRange::new;
    // A commit replaces the selection; the caret after it (1), before it (0), further on (3).
    assert_eq!(applied(buffer("abc", (1, 2), None), &[commit("XY", 1)]), ("aXYc".into(), r(3, 3), None));
    assert_eq!(applied(buffer("abc", (1, 2), None), &[commit("XY", 0)]), ("aXYc".into(), r(1, 1), None));
    assert_eq!(applied(buffer("abc", (1, 1), None), &[commit("X", 3)]), ("aXbc".into(), r(4, 4), None));
    assert_eq!(applied(buffer("abc", (1, 1), None), &[commit("X", 10)]), ("aXbc".into(), r(4, 4), None));
    // Composing text replaces the composition, then a commit finishes it.
    let b = buffer("ab", (2, 2), None);
    let (text, sel, comp) = applied(b.clone(), &[compose("ş", 1)]);
    assert_eq!((text.as_str(), sel, comp), ("abş", r(4, 4), Some(r(2, 4))));
    assert_eq!(applied(b, &[compose("ş", 1), compose("şu", 1), commit("şuna", 1)]), ("abşuna".into(), r(7, 7), None));
    // Empty composing text: none.
    assert_eq!(applied(buffer("ab", (1, 1), Some((0, 1))), &[compose("", 1)]), ("b".into(), r(0, 0), None));
    // A region composing, in either order; an empty one finishes it.
    assert_eq!(
        applied(buffer("abcd", (0, 0), None), &[EditCommand::SetComposingRegion { start: 3, end: 1 }]).2,
        Some(r(1, 3))
    );
    assert_eq!(
        applied(buffer("abcd", (0, 0), Some((0, 2))), &[EditCommand::SetComposingRegion { start: 2, end: 2 }]).2,
        None
    );
    assert_eq!(
        applied(buffer("abcd", (0, 0), Some((0, 2))), &[EditCommand::FinishComposingText]),
        ("abcd".into(), r(0, 0), None)
    );
}

#[test]
fn deletions_around_the_selection_and_backspace() {
    let r = TextRange::new;
    // Bytes around the selection, whole characters (a two-byte "ğ" half-touched is taken whole).
    assert_eq!(
        applied(buffer("ağbcd", (3, 4), None), &[EditCommand::DeleteSurroundingText { before: 1, after: 1 }]),
        ("abd".into(), r(1, 2), None)
    );
    // Code points.
    assert_eq!(
        applied(
            buffer("ağbcd", (3, 3), None),
            &[EditCommand::DeleteSurroundingTextInCodePoints { before: 2, after: 1 }]
        ),
        ("cd".into(), r(0, 0), None)
    );
    // The composition follows a deletion before it; deleted away, it is finished.
    let (_, _, comp) =
        applied(buffer("abcdef", (2, 2), Some((3, 5))), &[EditCommand::DeleteSurroundingText { before: 1, after: 0 }]);
    assert_eq!(comp, Some(r(2, 4)));
    // Backspace: the composition, else the selection, else the cluster before the caret.
    assert_eq!(applied(buffer("abc", (3, 3), Some((1, 3))), &[EditCommand::Backspace]), ("a".into(), r(1, 1), None));
    assert_eq!(applied(buffer("abc", (1, 3), None), &[EditCommand::Backspace]), ("a".into(), r(1, 1), None));
    let family = "a\u{1F468}\u{200D}\u{1F469}";
    assert_eq!(applied(buffer(family, (family.len(), family.len()), None), &[EditCommand::Backspace]).0, "a");
    assert_eq!(applied(buffer("abc", (0, 0), None), &[EditCommand::Backspace]).0, "abc");
    // The caret moved by clusters; all deleted.
    let flag = "x\u{1F1F9}\u{1F1F7}y";
    assert_eq!(applied(buffer(flag, (1, 1), None), &[EditCommand::MoveCursor { amount: 1 }]).1, r(9, 9));
    assert_eq!(applied(buffer(flag, (9, 10), None), &[EditCommand::MoveCursor { amount: -1 }]).1, r(1, 1));
    assert_eq!(applied(buffer("abc", (1, 1), None), &[EditCommand::DeleteAll]), (String::new(), r(0, 0), None));
    // A selection set in either order, clamped; inside a character, back to its start.
    assert_eq!(applied(buffer("ağb", (0, 0), None), &[EditCommand::SetSelection { start: 9, end: 2 }]).1, r(1, 4));
    assert_eq!(update_range_after_delete(r(2, 6), r(3, 4)), r(2, 5));
    assert_eq!(update_range_after_delete(r(2, 6), r(0, 3)), r(0, 3));
    assert_eq!(update_range_after_delete(r(2, 6), r(5, 9)), r(2, 5));
    assert_eq!(update_range_after_delete(r(2, 6), r(1, 9)), r(1, 1));
}

#[test]
fn edit_processors_keep_a_value_as_commands_change_it() {
    let mut p = EditProcessor::new();
    let value = TextFieldValue::from_text("abc", TextRange::new(3, 1), None);
    let (old, new) = p.reset(&value);
    assert_eq!((old.text(), new.text()), ("", "abc"));
    let after = p.apply(&[commit("X", 1)]);
    // (A reversed selection stays reversed.)
    assert_eq!((after.text(), after.selection()), ("aX", TextRange::new(2, 2)));
    let after = p.apply(&[EditCommand::SetSelection { start: 0, end: 1 }, compose("yz", 1)]);
    assert_eq!((after.text(), after.composition()), ("yzX", Some(TextRange::new(0, 2))));
    // A composition the value brings is dropped (a new text, or one the buffer didn't have); the one the input
    // method's commands made is kept.
    let composing = TextFieldValue::from_text("q", TextRange::at(1), Some(TextRange::new(0, 1)));
    assert_eq!(p.reset(&composing).1.composition(), None);
    assert_eq!(p.reset(&composing).1.composition(), None);
    p.apply(&[EditCommand::SetComposingRegion { start: 0, end: 1 }]);
    assert_eq!(p.reset(&composing).1.composition(), Some(TextRange::new(0, 1)));
    assert_eq!(p.to_text_field_value().text(), "q");
}

#[test]
fn nfc_composes_what_decomposes() {
    assert_eq!(to_nfc("e\u{0301}"), "é");
    assert_eq!(to_nfc("u\u{0308}\u{030C}"), "ǚ");
    assert_eq!(to_nfc("\u{01D6}"), "\u{01D6}");
    assert_eq!(to_nfc("x\u{0301}"), "x\u{0301}");
    assert_eq!(to_nfc("\u{1100}\u{1161}"), "가");
    // Canonical order: a below mark first.
    assert_eq!(to_nfc("a\u{0301}\u{0323}"), "ạ\u{0301}");
}

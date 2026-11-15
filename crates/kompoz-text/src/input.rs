//! A text field's value (Compose's `androidx.compose.ui.text.input.TextFieldValue`): its text, selection and
//! composition. Text fields themselves are stage 10's.

use crate::annotated_string::AnnotatedString;
use crate::range::TextRange;
use kompoz_runtime::named;

/// The nearest character boundary of `text` at or before `offset` (within it).
pub(crate) fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut at = offset.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn coerce(range: TextRange, text: &str) -> TextRange {
    TextRange::new(floor_boundary(text, range.start), floor_boundary(text, range.end))
}

/// A text field's value (Compose's TextFieldValue): its text, its selection (a caret when collapsed) and the
/// part the input method is composing, both kept within the text (on its character boundaries).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TextFieldValue {
    annotated_string: AnnotatedString,
    selection: TextRange,
    composition: Option<TextRange>,
}

impl TextFieldValue {
    /// An annotated text, its selection and composition (kept within it).
    pub fn new(annotated_string: AnnotatedString, selection: TextRange, composition: Option<TextRange>) -> Self {
        let selection = coerce(selection, annotated_string.text());
        let composition = composition.map(|c| coerce(c, annotated_string.text()));
        TextFieldValue { annotated_string, selection, composition }
    }
    /// Plain text, its selection and composition.
    pub fn from_text(text: &str, selection: TextRange, composition: Option<TextRange>) -> Self {
        TextFieldValue::new(AnnotatedString::new(text), selection, composition)
    }

    pub fn text(&self) -> &str {
        self.annotated_string.text()
    }
    pub fn annotated_string(&self) -> &AnnotatedString {
        &self.annotated_string
    }
    pub fn selection(&self) -> TextRange {
        self.selection
    }
    pub fn composition(&self) -> Option<TextRange> {
        self.composition
    }

    /// The same with plain `text` (its selection and composition as they were, kept within it).
    pub fn with_text(&self, text: &str) -> TextFieldValue {
        TextFieldValue::from_text(text, self.selection, self.composition)
    }

    /// Up to `max_chars` characters before the selection (Compose's getTextBeforeSelection).
    pub fn get_text_before_selection(&self, max_chars: usize) -> AnnotatedString {
        let text = self.text();
        let end = self.selection.min();
        let start = text[..end].char_indices().rev().nth(max_chars.saturating_sub(1)).map_or(0, |(i, _)| i);
        let start = if max_chars == 0 { end } else { start };
        self.annotated_string.sub_sequence(start, end)
    }

    /// Up to `max_chars` characters after the selection (Compose's getTextAfterSelection).
    pub fn get_text_after_selection(&self, max_chars: usize) -> AnnotatedString {
        let text = self.text();
        let start = self.selection.max();
        let end = text[start..].char_indices().nth(max_chars).map_or(text.len(), |(i, _)| start + i);
        self.annotated_string.sub_sequence(start, end)
    }

    /// What is selected (Compose's getSelectedText).
    pub fn get_selected_text(&self) -> AnnotatedString {
        self.annotated_string.sub_sequence_range(self.selection)
    }
}

#[named]
impl TextFieldValue {
    /// The same with what is given replaced (Compose's copy).
    pub fn copy(
        &self,
        #[default(self.annotated_string.clone())] annotated_string: AnnotatedString,
        #[default(self.selection)] selection: TextRange,
        #[default(self.composition)] composition: Option<TextRange>,
    ) -> TextFieldValue {
        TextFieldValue::new(annotated_string, selection, composition)
    }
}

/// Compose's `TextFieldValue(text = "", selection = TextRange.Zero, composition = null)`.
#[named]
pub fn TextFieldValue(
    #[default(String::new())] text: String,
    #[default(TextRange::Zero)] selection: TextRange,
    #[default(None)] composition: Option<TextRange>,
) -> TextFieldValue {
    TextFieldValue::from_text(&text, selection, composition)
}

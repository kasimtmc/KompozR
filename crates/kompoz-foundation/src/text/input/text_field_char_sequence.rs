//! An immutable snapshot of a [`TextFieldState`](super::TextFieldState)'s contents (Compose's
//! TextFieldCharSequence): its text, selection, the range an input method composes, a highlighted range, the
//! annotations of the composition and of an output transformation, and its tracked styles.

use super::text_field_text_styles::TextFieldTextStylesImpl;
use kompoz_text::TextRange;
use kompoz_text::annotated_string::{Annotation, Range};
use std::sync::Arc;

/// An annotation placed on a range of a field's text (Compose's PlacedAnnotation).
pub type PlacedAnnotation = Range<Annotation>;

/// A text range highlight type (Compose's TextHighlightType): a handwriting gesture's preview — kompoz-ui's, which
/// the platform's input method gives an open session's editor.
pub use kompoz_ui::text_input::TextHighlightType;

/// A field's contents at a moment (Compose's TextFieldCharSequence): cheap to clone. The selection, composition and
/// highlight are kept within the text.
#[derive(Clone, Debug)]
pub struct TextFieldCharSequence {
    text: Arc<str>,
    selection: TextRange,
    composition: Option<TextRange>,
    highlight: Option<(TextHighlightType, TextRange)>,
    composing_annotations: Option<Arc<Vec<PlacedAnnotation>>>,
    output_annotations: Option<Arc<Vec<PlacedAnnotation>>>,
    text_field_text_styles: Option<TextFieldTextStylesImpl>,
}

impl Default for TextFieldCharSequence {
    fn default() -> Self {
        TextFieldCharSequence::new("", TextRange::Zero, None)
    }
}

fn coerce(range: TextRange, len: usize) -> TextRange {
    range.coerce_in(0, len)
}

impl TextFieldCharSequence {
    /// `text` with `selection` and `composition` (kept within it).
    pub fn new(text: impl Into<Arc<str>>, selection: TextRange, composition: Option<TextRange>) -> Self {
        TextFieldCharSequence::with_all(text, selection, composition, None, None, None, None)
    }

    /// With everything Compose's constructor takes.
    pub fn with_all(
        text: impl Into<Arc<str>>,
        selection: TextRange,
        composition: Option<TextRange>,
        highlight: Option<(TextHighlightType, TextRange)>,
        composing_annotations: Option<Vec<PlacedAnnotation>>,
        output_annotations: Option<Vec<PlacedAnnotation>>,
        text_field_text_styles: Option<TextFieldTextStylesImpl>,
    ) -> Self {
        let text = text.into();
        let len = text.len();
        TextFieldCharSequence {
            selection: coerce(selection, len),
            composition: composition.map(|c| coerce(c, len)),
            highlight: highlight.map(|(t, r)| (t, coerce(r, len))),
            composing_annotations: composing_annotations.map(Arc::new),
            output_annotations: output_annotations.map(Arc::new),
            text_field_text_styles,
            text,
        }
    }

    /// The text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text, shared.
    pub fn text_arc(&self) -> Arc<str> {
        self.text.clone()
    }

    /// Its length (bytes).
    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The selection (a caret when collapsed).
    pub fn selection(&self) -> TextRange {
        self.selection
    }

    /// The range an input method composes, if any.
    pub fn composition(&self) -> Option<TextRange> {
        self.composition
    }

    /// A range highlighted (a handwriting gesture's preview), if any.
    pub fn highlight(&self) -> Option<(TextHighlightType, TextRange)> {
        self.highlight
    }

    /// The annotations of the composition (an input method's styling of it).
    pub fn composing_annotations(&self) -> Option<&[PlacedAnnotation]> {
        self.composing_annotations.as_deref().map(|v| v.as_slice())
    }

    /// The styles an output transformation added.
    pub fn output_annotations(&self) -> Option<&[PlacedAnnotation]> {
        self.output_annotations.as_deref().map(|v| v.as_slice())
    }

    /// Its tracked styles, if any were ever added.
    pub fn text_field_text_styles(&self) -> Option<&TextFieldTextStylesImpl> {
        self.text_field_text_styles.as_ref()
    }

    /// Whether the text is `other` (Compose's `contentEquals`).
    pub fn content_equals(&self, other: &str) -> bool {
        *self.text == *other
    }

    /// Whether to show the cursor or selection (not while a handwriting preview is highlighted).
    pub fn should_show_selection(&self) -> bool {
        self.highlight.is_none()
    }

    /// Up to `max_chars` characters before the selection (Compose's getTextBeforeSelection).
    pub fn get_text_before_selection(&self, max_chars: usize) -> &str {
        let end = self.selection.min();
        let start = if max_chars == 0 {
            end
        } else {
            self.text[..end].char_indices().rev().nth(max_chars - 1).map_or(0, |(i, _)| i)
        };
        &self.text[start..end]
    }

    /// Up to `max_chars` characters after the selection (Compose's getTextAfterSelection).
    pub fn get_text_after_selection(&self, max_chars: usize) -> &str {
        let start = self.selection.max();
        let end = self.text[start..].char_indices().nth(max_chars).map_or(self.text.len(), |(i, _)| start + i);
        &self.text[start..end]
    }

    /// What is selected (Compose's getSelectedText).
    pub fn get_selected_text(&self) -> &str {
        &self.text[self.selection.min()..self.selection.max()]
    }
}

/// The same contents, selection, composition, highlight, composing annotations and styles (Compose's `equals`; the
/// output annotations are not compared). To compare the text alone: [`content_equals`](TextFieldCharSequence::
/// content_equals).
impl PartialEq for TextFieldCharSequence {
    fn eq(&self, other: &Self) -> bool {
        self.selection == other.selection
            && self.composition == other.composition
            && self.highlight == other.highlight
            && self.composing_annotations.as_deref() == other.composing_annotations.as_deref()
            && *self.text == *other.text
            && self.text_field_text_styles == other.text_field_text_styles
    }
}

impl std::fmt::Display for TextFieldCharSequence {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

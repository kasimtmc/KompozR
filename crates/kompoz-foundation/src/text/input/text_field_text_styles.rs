//! The styles of a [`TextFieldState`](super::TextFieldState)'s text, to read (Compose's TextFieldTextStyles): what a
//! formatting toolbar asks. Changing them is [`TextFieldState::edit`](super::TextFieldState::edit)'s, through the
//! buffer.

use super::internal::text_style_buffer::TextStyleBuffer;
use kompoz_text::annotated_string::Range;
use kompoz_text::{ParagraphStyle, SpanStyle, TextRange};
use std::sync::Arc;

/// A field's styles at a moment (Compose's TextFieldTextStyles, its `EmptyTextFieldTextStyles` when it has none).
/// A style meets a range when they overlap; a collapsed range meets the styles around it and those starting at
/// it (not those ending at it). Styles come in the order they were added.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TextFieldTextStyles(pub(crate) Option<TextFieldTextStylesImpl>);

impl TextFieldTextStyles {
    /// The span styles meeting `range`.
    pub fn get_span_styles(&self, range: TextRange) -> Vec<Range<SpanStyle>> {
        self.0.as_ref().map_or_else(Vec::new, |s| s.get_span_styles(range))
    }

    /// The paragraph styles meeting `range`.
    pub fn get_paragraph_styles(&self, range: TextRange) -> Vec<Range<ParagraphStyle>> {
        self.0.as_ref().map_or_else(Vec::new, |s| s.get_paragraph_styles(range))
    }
}

/// A text's styles held immutably with its length (Compose's TextFieldTextStylesImpl).
#[derive(Clone, PartialEq, Debug)]
pub struct TextFieldTextStylesImpl {
    pub(crate) text_style_buffer: Arc<TextStyleBuffer>,
    length: usize,
}

impl TextFieldTextStylesImpl {
    pub(crate) fn new(text_style_buffer: TextStyleBuffer, length: usize) -> TextFieldTextStylesImpl {
        TextFieldTextStylesImpl { text_style_buffer: Arc::new(text_style_buffer), length }
    }

    /// The span styles meeting `range` (clamped to the text).
    pub fn get_span_styles(&self, range: TextRange) -> Vec<Range<SpanStyle>> {
        let (start, end) = (range.min().min(self.length), range.max().min(self.length));
        self.text_style_buffer.get_immutable_styles::<SpanStyle>(start, end)
    }

    /// The paragraph styles meeting `range` (clamped to the text).
    pub fn get_paragraph_styles(&self, range: TextRange) -> Vec<Range<ParagraphStyle>> {
        let (start, end) = (range.min().min(self.length), range.max().min(self.length));
        self.text_style_buffer.get_immutable_styles::<ParagraphStyle>(start, end)
    }

    /// The styles held.
    pub fn text_style_buffer(&self) -> &TextStyleBuffer {
        &self.text_style_buffer
    }
}

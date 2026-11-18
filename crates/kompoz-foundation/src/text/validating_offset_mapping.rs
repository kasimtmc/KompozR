//! A value-based field's visual transformation checked (Compose's ValidatingOffsetMapping.kt): a transformation
//! whose offset mapping gives offsets outside the other text panics with a message naming the offsets, at once for
//! the first hundred characters and both ends, then at each mapping the field asks for.
//!
//! The offsets checked are the texts' character boundaries (KompozR's offsets are UTF-8 bytes on character
//! boundaries; Compose checks each UTF-16 index).

use kompoz_text::AnnotatedString;
use kompoz_text::input::{MapOffsets, OffsetMapping, TransformedText, VisualTransformation};

/// The identity of empty texts, checked (Compose's `ValidatingEmptyOffsetMappingIdentity`): what a field's
/// selection manager maps with before it is given its value.
pub fn validating_empty_offset_mapping_identity() -> OffsetMapping {
    OffsetMapping::of(ValidatingOffsetMapping {
        delegate: OffsetMapping::Identity,
        original_length: 0,
        transformed_length: 0,
    })
}

/// `visual_transformation`'s filter of `text`, its offset mapping checked (Compose's `filterWithValidation`):
/// panics at once if the first offsets map outside the other text, and later at a mapping that does.
pub fn filter_with_validation(visual_transformation: &VisualTransformation, text: &AnnotatedString) -> TransformedText {
    let delegate = visual_transformation.filter(text);
    throw_if_not_valid_transform(&delegate, text.text(), 100);
    let transformed_length = delegate.text.text().len();
    TransformedText::new(
        delegate.text,
        OffsetMapping::of(ValidatingOffsetMapping {
            delegate: delegate.offset_mapping,
            original_length: text.text().len(),
            transformed_length,
        }),
    )
}

/// Panics if `transformed`'s mapping maps any of the first `limit` character boundaries of either text, or its
/// end, outside the other text (Compose's `throwIfNotValidTransform`).
pub fn throw_if_not_valid_transform(transformed: &TransformedText, original_text: &str, limit: usize) {
    let transformed_text = transformed.text.text();
    let (original_length, transformed_length) = (original_text.len(), transformed_text.len());
    let mapping = &transformed.offset_mapping;
    for (offset, _) in original_text.char_indices().take(limit) {
        validate_original_to_transformed(mapping.original_to_transformed(offset), transformed_length, offset);
    }
    validate_original_to_transformed(
        mapping.original_to_transformed(original_length),
        transformed_length,
        original_length,
    );
    for (offset, _) in transformed_text.char_indices().take(limit) {
        validate_transformed_to_original(mapping.transformed_to_original(offset), original_length, offset);
    }
    validate_transformed_to_original(
        mapping.transformed_to_original(transformed_length),
        original_length,
        transformed_length,
    );
}

/// A mapping checking what its delegate gives for offsets within the texts (Compose's ValidatingOffsetMapping).
struct ValidatingOffsetMapping {
    delegate: OffsetMapping,
    original_length: usize,
    transformed_length: usize,
}

impl MapOffsets for ValidatingOffsetMapping {
    fn original_to_transformed(&self, offset: usize) -> usize {
        let transformed = self.delegate.original_to_transformed(offset);
        // (only requests within the text are checked: the field is responsible for asking right)
        if offset <= self.original_length {
            validate_original_to_transformed(transformed, self.transformed_length, offset);
        }
        transformed
    }

    fn transformed_to_original(&self, offset: usize) -> usize {
        let original = self.delegate.transformed_to_original(offset);
        if offset <= self.transformed_length {
            validate_transformed_to_original(original, self.original_length, offset);
        }
        original
    }
}

fn validate_transformed_to_original(original_offset: usize, original_length: usize, offset: usize) {
    assert!(
        original_offset <= original_length,
        "OffsetMapping.transformedToOriginal returned invalid mapping: {offset} -> {original_offset} is not in range \
         of original text [0, {original_length}]"
    );
}

fn validate_original_to_transformed(transformed_offset: usize, transformed_length: usize, offset: usize) {
    assert!(
        transformed_offset <= transformed_length,
        "OffsetMapping.originalToTransformed returned invalid mapping: {offset} -> {transformed_offset} is not in \
         range of transformed text [0, {transformed_length}]"
    );
}

//! How a field's text is shown (Compose's OutputTransformation), a [`TextFieldBuffer`] of the text changed for the
//! display alone (inserting separators, a prefix or a suffix, styles): the state keeps the text as it is, and the
//! selection maps between the two.

use super::text_field_buffer::TextFieldBuffer;
use std::rc::Rc;

/// What changes a field's text for showing it (Compose's OutputTransformation, a `fun interface`): any
/// `Fn(&mut TextFieldBuffer)` is one (Kotlin's `OutputTransformation { … }`).
pub trait OutputTransformation {
    /// Changes `buffer` (the text and selection now) to what is shown (Compose's `TextFieldBuffer.transformOutput()`).
    fn transform_output(&self, buffer: &mut TextFieldBuffer);
}

impl<F: Fn(&mut TextFieldBuffer)> OutputTransformation for F {
    fn transform_output(&self, buffer: &mut TextFieldBuffer) {
        self(buffer)
    }
}

/// An output transformation from `transform` (Kotlin's SAM constructor `OutputTransformation { … }`).
///
/// Kotlin: `OutputTransformation(transform: TextFieldBuffer.() -> Unit)`
pub fn output_transformation(transform: impl Fn(&mut TextFieldBuffer) + 'static) -> Rc<dyn OutputTransformation> {
    Rc::new(transform)
}

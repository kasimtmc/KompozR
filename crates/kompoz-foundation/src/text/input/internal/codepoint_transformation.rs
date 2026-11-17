//! A field's visual transformation of code points, one to one (Compose's CodepointTransformation): a password's
//! mask, a single-line field's line breaks shown as spaces.

use super::offset_mapping_calculator::OffsetMappingCalculator;
use crate::text::input::TextFieldCharSequence;
use std::any::Any;
use std::rc::Rc;

/// Maps each code point of a field's text to the one shown (Compose's CodepointTransformation): given its index
/// (in code points) and itself. Any `Fn(usize, char) -> char` is one.
pub trait CodepointTransformation: Any {
    fn transform(&self, codepoint_index: usize, codepoint: char) -> char;
}

impl<F: Fn(usize, char) -> char + 'static> CodepointTransformation for F {
    fn transform(&self, codepoint_index: usize, codepoint: char) -> char {
        self(codepoint_index, codepoint)
    }
}

/// Whether two transformations are the same (Compose's equality: a mask by its character, the single-line one
/// with itself, others by identity).
pub fn same_codepoint_transformation(a: &dyn CodepointTransformation, b: &dyn CodepointTransformation) -> bool {
    let (a_any, b_any) = (a as &dyn Any, b as &dyn Any);
    if let (Some(x), Some(y)) =
        (a_any.downcast_ref::<MaskCodepointTransformation>(), b_any.downcast_ref::<MaskCodepointTransformation>())
    {
        return x == y;
    }
    if a_any.is::<SingleLineCodepointTransformation>() && b_any.is::<SingleLineCodepointTransformation>() {
        return true;
    }
    std::ptr::addr_eq(a as *const dyn CodepointTransformation, b as *const dyn CodepointTransformation)
}

/// Every code point shown as `character` (Compose's MaskCodepointTransformation).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MaskCodepointTransformation {
    pub character: char,
}

impl CodepointTransformation for MaskCodepointTransformation {
    fn transform(&self, _codepoint_index: usize, _codepoint: char) -> char {
        self.character
    }
}

/// A transformation masking every code point with `character` (Compose's `CodepointTransformation.mask`).
pub fn mask(character: char) -> Rc<dyn CodepointTransformation> {
    Rc::new(MaskCodepointTransformation { character })
}

/// A single-line field's: a line feed shown as a space, a carriage return as a zero-width no-break space
/// (Compose's SingleLineCodepointTransformation): whatever the text holds shows on one line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SingleLineCodepointTransformation;

impl CodepointTransformation for SingleLineCodepointTransformation {
    fn transform(&self, _codepoint_index: usize, codepoint: char) -> char {
        match codepoint {
            '\n' => ' ',
            '\r' => '\u{FEFF}',
            c => c,
        }
    }
}

/// `text` as `transformation` shows it, the code points it changed recorded in `offset_mapping_calculator`
/// (Compose's `toVisualText`): none if it changed nothing.
pub fn to_visual_text(
    text: &TextFieldCharSequence,
    transformation: &dyn CodepointTransformation,
    offset_mapping_calculator: &mut OffsetMappingCalculator,
) -> Option<String> {
    let mut changed = false;
    let mut out = String::with_capacity(text.len());
    for (index, c) in text.text().chars().enumerate() {
        let new = transformation.transform(index, c);
        if new != c {
            changed = true;
            let at = out.len();
            offset_mapping_calculator.record_edit_operation(at, at + c.len_utf8(), new.len_utf8());
        }
        out.push(new);
    }
    changed.then_some(out)
}

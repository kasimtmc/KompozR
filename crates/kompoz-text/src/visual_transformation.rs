//! How a value-based text field shows its text (Compose's `androidx.compose.ui.text.input` VisualTransformation,
//! TransformedText, OffsetMapping and PasswordVisualTransformation): the field keeps its value as it is and shows
//! what its visual transformation makes of it, the two texts' offsets mapped both ways (the caret, the selection,
//! a tap's offset).
//!
//! Offsets are UTF-8 bytes (Compose's UTF-16 units). A password field shows one mask character for each character
//! of its text (Compose's one for each UTF-16 unit: an emoji outside the BMP is one bullet here, two there), its
//! offsets mapped between the two by character — the masked text's bytes aren't the original's when the mask isn't
//! one byte long, as the default bullet isn't.

use crate::annotated_string::AnnotatedString;
use kompoz_runtime::named;
use std::fmt;
use std::rc::Rc;

/// What maps the offsets of a text and of what is shown of it, both ways (Compose's OffsetMapping interface
/// members), for one of the app's own. Each must be monotonically non-decreasing: a caret advancing in one text
/// advances or stays in the other.
pub trait MapOffsets {
    /// The offset in the shown text of `offset` in the original one.
    fn original_to_transformed(&self, offset: usize) -> usize;
    /// The offset in the original text of `offset` in the shown one.
    fn transformed_to_original(&self, offset: usize) -> usize;
}

#[derive(Clone)]
enum MappingKind {
    /// Compose's `OffsetMapping.Identity`.
    Identity,
    /// The app's own, or one of foundation's.
    Custom(Rc<dyn MapOffsets>),
}

/// The offset mapping between a text and what is shown of it (Compose's OffsetMapping): made from an
/// implementation of [`MapOffsets`] ([`OffsetMapping::of`], Kotlin's `object : OffsetMapping`) or two closures
/// ([`OffsetMapping::new`]); the identity is [`OffsetMapping::Identity`]. Cheap to clone; equal when the same (the
/// identity equal to itself).
#[derive(Clone)]
pub struct OffsetMapping {
    kind: MappingKind,
}

impl OffsetMapping {
    /// The identity: both texts the same (Compose's `OffsetMapping.Identity`).
    pub const Identity: OffsetMapping = OffsetMapping { kind: MappingKind::Identity };

    /// One of the app's own (Kotlin's `object : OffsetMapping { … }`).
    pub fn of(mapping: impl MapOffsets + 'static) -> OffsetMapping {
        OffsetMapping { kind: MappingKind::Custom(Rc::new(mapping)) }
    }

    /// One mapping with these two closures.
    pub fn new(
        original_to_transformed: impl Fn(usize) -> usize + 'static,
        transformed_to_original: impl Fn(usize) -> usize + 'static,
    ) -> OffsetMapping {
        struct Closures<A, B>(A, B);
        impl<A: Fn(usize) -> usize, B: Fn(usize) -> usize> MapOffsets for Closures<A, B> {
            fn original_to_transformed(&self, offset: usize) -> usize {
                (self.0)(offset)
            }
            fn transformed_to_original(&self, offset: usize) -> usize {
                (self.1)(offset)
            }
        }
        OffsetMapping::of(Closures(original_to_transformed, transformed_to_original))
    }

    /// The offset in the shown text of `offset` in the original one (Compose's `originalToTransformed`).
    pub fn original_to_transformed(&self, offset: usize) -> usize {
        match &self.kind {
            MappingKind::Identity => offset,
            MappingKind::Custom(m) => m.original_to_transformed(offset),
        }
    }

    /// The offset in the original text of `offset` in the shown one (Compose's `transformedToOriginal`).
    pub fn transformed_to_original(&self, offset: usize) -> usize {
        match &self.kind {
            MappingKind::Identity => offset,
            MappingKind::Custom(m) => m.transformed_to_original(offset),
        }
    }

    /// Whether it is the identity.
    pub fn is_identity(&self) -> bool {
        matches!(self.kind, MappingKind::Identity)
    }
}

impl PartialEq for OffsetMapping {
    fn eq(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (MappingKind::Identity, MappingKind::Identity) => true,
            (MappingKind::Custom(a), MappingKind::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for OffsetMapping {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_identity() { f.write_str("OffsetMapping.Identity") } else { f.write_str("OffsetMapping") }
    }
}

/// What is shown of a text and how the two map (Compose's TransformedText): equal when both texts and mappings
/// are.
#[derive(Clone, PartialEq, Debug)]
pub struct TransformedText {
    /// What is shown.
    pub text: AnnotatedString,
    /// The offsets of the original text and of what is shown, both ways.
    pub offset_mapping: OffsetMapping,
}

impl TransformedText {
    /// What is shown, and how its offsets map to the original text's.
    pub fn new(text: AnnotatedString, offset_mapping: OffsetMapping) -> TransformedText {
        TransformedText { text, offset_mapping }
    }
}

/// Compose's `TransformedText(text, offsetMapping)`.
#[allow(non_snake_case)]
pub fn TransformedText(text: AnnotatedString, offset_mapping: OffsetMapping) -> TransformedText {
    TransformedText::new(text, offset_mapping)
}

/// What a visual transformation does (Compose's VisualTransformation interface member), for one of the app's own.
pub trait FilterText {
    /// What is shown of `text`, and how their offsets map.
    fn filter(&self, text: &AnnotatedString) -> TransformedText;
}

#[derive(Clone)]
enum VisualKind {
    /// Compose's `VisualTransformation.None`.
    None,
    /// Compose's PasswordVisualTransformation.
    Password(char),
    /// The app's own.
    Custom(Rc<dyn FilterText>),
}

/// How a value-based field shows its text (Compose's VisualTransformation, a `fun interface`): none
/// ([`VisualTransformation::None`]), a password's mask ([`PasswordVisualTransformation`]), or the app's own — a
/// closure ([`VisualTransformation::new`], Kotlin's `VisualTransformation { text -> … }`) or an implementation of
/// [`FilterText`] ([`VisualTransformation::of`]). Cheap to clone; equal as Compose's are: none to none, a password's
/// by its mask, the app's when the same.
#[derive(Clone)]
pub struct VisualTransformation {
    kind: VisualKind,
}

impl VisualTransformation {
    /// No transformation: the text shown as it is (Compose's `VisualTransformation.None`).
    pub const None: VisualTransformation = VisualTransformation { kind: VisualKind::None };

    /// One showing what `filter` makes of the text (Kotlin's SAM constructor `VisualTransformation { text -> … }`).
    ///
    /// Kotlin: `VisualTransformation(filter: (AnnotatedString) -> TransformedText)`
    pub fn new(filter: impl Fn(&AnnotatedString) -> TransformedText + 'static) -> VisualTransformation {
        struct Closure<F>(F);
        impl<F: Fn(&AnnotatedString) -> TransformedText> FilterText for Closure<F> {
            fn filter(&self, text: &AnnotatedString) -> TransformedText {
                (self.0)(text)
            }
        }
        VisualTransformation::of(Closure(filter))
    }

    /// One of the app's own (Kotlin's `object : VisualTransformation { … }`).
    pub fn of(transformation: impl FilterText + 'static) -> VisualTransformation {
        VisualTransformation { kind: VisualKind::Custom(Rc::new(transformation)) }
    }

    /// What is shown of `text`, and how their offsets map (Compose's `filter`).
    pub fn filter(&self, text: &AnnotatedString) -> TransformedText {
        match &self.kind {
            VisualKind::None => TransformedText::new(text.clone(), OffsetMapping::Identity),
            VisualKind::Password(mask) => password_filter(*mask, text),
            VisualKind::Custom(f) => f.filter(text),
        }
    }

    /// Its mask, if it is a password's (Kotlin's `is PasswordVisualTransformation` and its `mask`).
    pub fn password_mask(&self) -> Option<char> {
        match self.kind {
            VisualKind::Password(mask) => Some(mask),
            _ => None,
        }
    }

    /// Whether it is a password's (Kotlin's `is PasswordVisualTransformation`).
    pub fn is_password(&self) -> bool {
        self.password_mask().is_some()
    }
}

impl Default for VisualTransformation {
    fn default() -> Self {
        VisualTransformation::None
    }
}

impl PartialEq for VisualTransformation {
    fn eq(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (VisualKind::None, VisualKind::None) => true,
            (VisualKind::Password(a), VisualKind::Password(b)) => a == b,
            (VisualKind::Custom(a), VisualKind::Custom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for VisualTransformation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            VisualKind::None => f.write_str("VisualTransformation.None"),
            VisualKind::Password(mask) => write!(f, "PasswordVisualTransformation({mask:?})"),
            VisualKind::Custom(_) => f.write_str("VisualTransformation"),
        }
    }
}

/// A password's visual transformation (Compose's PasswordVisualTransformation): each character shown as `mask`.
#[named]
pub fn PasswordVisualTransformation(#[default('\u{2022}')] mask: char) -> VisualTransformation {
    VisualTransformation { kind: VisualKind::Password(mask) }
}

/// Each character of `text` as `mask`, the offsets mapped by character.
fn password_filter(mask: char, text: &AnnotatedString) -> TransformedText {
    let boundaries: Vec<usize> = text.text().char_indices().map(|(i, _)| i).chain([text.text().len()]).collect();
    let chars = boundaries.len() - 1;
    let masked = mask.to_string().repeat(chars);
    let mapping = MaskMapping { boundaries, mask_len: mask.len_utf8() };
    TransformedText::new(AnnotatedString::new(masked), OffsetMapping::of(mapping))
}

/// The offsets of a text and of its mask: a character of one is a mask character of the other.
struct MaskMapping {
    /// Where each character of the original text starts, then its length.
    boundaries: Vec<usize>,
    mask_len: usize,
}

impl MapOffsets for MaskMapping {
    fn original_to_transformed(&self, offset: usize) -> usize {
        // The characters starting at or before `offset` (a caret inside one stays before it).
        let chars = self.boundaries.partition_point(|&b| b <= offset).saturating_sub(1);
        chars * self.mask_len
    }

    fn transformed_to_original(&self, offset: usize) -> usize {
        let chars = (offset / self.mask_len).min(self.boundaries.len() - 1);
        self.boundaries[chars]
    }
}

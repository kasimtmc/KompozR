//! What a field does to the user's edits before they reach its state (Compose's InputTransformation): accept,
//! change or refuse them, with the keyboard options and semantics they imply. Made from a closure
//! ([`InputTransformation::new`], Kotlin's `InputTransformation { … }`), an implementation of [`TransformInput`]
//! (Kotlin's `object : InputTransformation`), or Compose's own on the companion value [`InputTransformation`]
//! itself (`InputTransformation.max_length(10)`, `by_value`, `all_caps`), chained with [`then`](
//! InputTransformation::then).

use super::text_field_buffer::TextFieldBuffer;
use crate::text::keyboard_options::KeyboardOptions;
use kompoz_text::Locale;
use kompoz_ui::platform::KeyboardCapitalization;
use kompoz_ui::semantics::SemanticsConfiguration;
use std::rc::Rc;

/// What an input transformation is (Compose's InputTransformation interface members), for one of the app's own.
pub trait TransformInput {
    /// Changes `buffer` (the user's edit applied to the field's value; its changes and original text there to read)
    /// to what the field takes; [`revert_all_changes`](TextFieldBuffer::revert_all_changes) refuses the edit.
    fn transform_input(&self, buffer: &mut TextFieldBuffer);

    /// The keyboard it wants (merged under the field's own options).
    fn keyboard_options(&self) -> Option<KeyboardOptions> {
        None
    }

    /// Semantics it adds to the field's (a maximum length, …).
    fn apply_semantics(&self, receiver: &mut SemanticsConfiguration) {
        let _ = receiver;
    }
}

enum Kind {
    /// The app's own.
    Custom(Rc<dyn TransformInput>),
    /// Compose's FilterChain.
    Chain(InputTransformation, InputTransformation),
    /// Compose's InputTransformationByValue.
    ByValue(Rc<dyn Fn(String, String) -> String>),
    /// Compose's AllCapsTransformation.
    AllCaps(Locale),
    /// Compose's MaxLengthFilter.
    MaxLength(usize),
}

/// A field's input transformation (Compose's InputTransformation): cheap to clone. The value of the same name is
/// Compose's companion object: it changes nothing, and Compose's transformations are made on it.
#[derive(Clone)]
pub struct InputTransformation {
    kind: Option<Rc<Kind>>,
}

/// The companion (Compose's `InputTransformation.Companion`): an input transformation changing nothing, on which
/// `InputTransformation.max_length(…)`, `.by_value(…)`, `.all_caps(…)` are called.
pub const InputTransformation: InputTransformation = InputTransformation { kind: None };

impl InputTransformation {
    fn of_kind(kind: Kind) -> InputTransformation {
        InputTransformation { kind: Some(Rc::new(kind)) }
    }

    /// One transforming with `transform` (Kotlin's `InputTransformation { … }`).
    pub fn new(transform: impl Fn(&mut TextFieldBuffer) + 'static) -> InputTransformation {
        struct Closure<F>(F);
        impl<F: Fn(&mut TextFieldBuffer)> TransformInput for Closure<F> {
            fn transform_input(&self, buffer: &mut TextFieldBuffer) {
                (self.0)(buffer)
            }
        }
        InputTransformation::of(Closure(transform))
    }

    /// One of the app's own (Kotlin's `object : InputTransformation { … }`).
    pub fn of(transformation: impl TransformInput + 'static) -> InputTransformation {
        InputTransformation::of_kind(Kind::Custom(Rc::new(transformation)))
    }

    /// This one, then `next` on what it leaves (Compose's `then`): `next`'s keyboard options over this one's.
    pub fn then(&self, next: InputTransformation) -> InputTransformation {
        InputTransformation::of_kind(Kind::Chain(self.clone(), next))
    }

    /// This one, then one deciding from the text before and the text proposed what the field takes (Compose's
    /// `byValue`): returning the proposed text keeps the edit, the current one refuses it (the selection as it
    /// was), any other replaces the text with it where it differs. (Compose tells the two apart by identity;
    /// Rust's strings have none: by content.)
    pub fn by_value(&self, transformation: impl Fn(String, String) -> String + 'static) -> InputTransformation {
        self.then(InputTransformation::of_kind(Kind::ByValue(Rc::new(transformation))))
    }

    /// This one, then what was typed in capitals as `locale` writes them (Compose's `allCaps`): the keyboard asked
    /// for capitals too. Text set in code stays as it is.
    pub fn all_caps(&self, locale: Locale) -> InputTransformation {
        self.then(InputTransformation::of_kind(Kind::AllCaps(locale)))
    }

    /// This one, then refusing an edit leaving the text longer than `max_length` characters (Compose's
    /// `maxLength`; Compose counts UTF-16 units, KompozR counts characters: code points). Said to accessibility
    /// as the field's maximum length.
    pub fn max_length(&self, max_length: usize) -> InputTransformation {
        self.then(InputTransformation::of_kind(Kind::MaxLength(max_length)))
    }

    /// Transforms `buffer` (Compose's `TextFieldBuffer.transformInput()`).
    pub fn transform_input(&self, buffer: &mut TextFieldBuffer) {
        let Some(kind) = &self.kind else { return };
        match &**kind {
            Kind::Custom(t) => t.transform_input(buffer),
            Kind::Chain(first, second) => {
                first.transform_input(buffer);
                second.transform_input(buffer);
            }
            Kind::ByValue(transformation) => {
                let proposed = buffer.as_char_sequence().to_string();
                let current = buffer.original_text().to_string();
                let accepted = transformation(current.clone(), proposed.clone());
                if accepted == proposed {
                    // (as typed)
                } else if accepted == current {
                    buffer.revert_all_changes();
                } else {
                    buffer.set_text_if_changed(&accepted);
                }
            }
            Kind::AllCaps(locale) => {
                // (Each change read again as the one before is replaced: capitals may be longer.)
                let mut i = 0;
                while i < kompoz_ui_change_count(buffer) {
                    let range = super::change_tracker::ChangeList::get_range(buffer.changes(), i);
                    if !range.collapsed() {
                        let upper =
                            kompoz_text::intl::to_upper_case(range.substring(buffer.as_char_sequence()), locale);
                        buffer.replace(range.min(), range.max(), &upper);
                    }
                    i += 1;
                }
            }
            Kind::MaxLength(max_length) => {
                if buffer.as_char_sequence().chars().count() > *max_length {
                    buffer.revert_all_changes();
                }
            }
        }
    }

    /// The keyboard it wants (Compose's `keyboardOptions`): a chain's second's over its first's; capitals for
    /// `all_caps`.
    pub fn keyboard_options(&self) -> Option<KeyboardOptions> {
        let kind = self.kind.as_ref()?;
        match &**kind {
            Kind::Custom(t) => t.keyboard_options(),
            Kind::Chain(first, second) => match second.keyboard_options() {
                Some(options) => Some(options.fill_unspecified_values_with(first.keyboard_options().as_ref())),
                None => first.keyboard_options(),
            },
            Kind::AllCaps(_) => {
                Some(KeyboardOptions { capitalization: KeyboardCapitalization::Characters, ..KeyboardOptions::Default })
            }
            Kind::ByValue(_) | Kind::MaxLength(_) => None,
        }
    }

    /// Adds its semantics to `receiver` (Compose's `applySemantics`): a chain's both, the maximum length.
    pub fn apply_semantics(&self, receiver: &mut SemanticsConfiguration) {
        let Some(kind) = &self.kind else { return };
        match &**kind {
            Kind::Custom(t) => t.apply_semantics(receiver),
            Kind::Chain(first, second) => {
                first.apply_semantics(receiver);
                second.apply_semantics(receiver);
            }
            Kind::MaxLength(max_length) => receiver.set_max_text_length((*max_length).min(i32::MAX as usize) as i32),
            Kind::ByValue(_) | Kind::AllCaps(_) => {}
        }
    }
}

fn kompoz_ui_change_count(buffer: &TextFieldBuffer) -> usize {
    super::change_tracker::ChangeList::change_count(buffer.changes())
}

/// Compose's equality: the companion with itself, a chain by its parts (and keyboard options), `by_value` by its
/// closure, `all_caps` by its locale, `max_length` by its length, the app's own by identity.
impl PartialEq for InputTransformation {
    fn eq(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (None, None) => true,
            (Some(a), Some(b)) if Rc::ptr_eq(a, b) => true,
            (Some(a), Some(b)) => match (&**a, &**b) {
                (Kind::Custom(x), Kind::Custom(y)) => Rc::ptr_eq(x, y),
                (Kind::Chain(a1, a2), Kind::Chain(b1, b2)) => {
                    a1 == b1 && a2 == b2 && self.keyboard_options() == other.keyboard_options()
                }
                (Kind::ByValue(x), Kind::ByValue(y)) => Rc::ptr_eq(x, y),
                (Kind::AllCaps(x), Kind::AllCaps(y)) => x == y,
                (Kind::MaxLength(x), Kind::MaxLength(y)) => x == y,
                _ => false,
            },
            _ => false,
        }
    }
}

/// As Compose's `toString`: `InputTransformation.maxLength(4)`, `first.then(second)`.
impl std::fmt::Debug for InputTransformation {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let Some(kind) = &self.kind else { return f.write_str("InputTransformation") };
        match &**kind {
            Kind::Custom(_) => f.write_str("InputTransformation(…)"),
            Kind::Chain(first, second) => write!(f, "{first:?}.then({second:?})"),
            Kind::ByValue(_) => f.write_str("InputTransformation.byValue(transformation=…)"),
            Kind::AllCaps(locale) => write!(f, "InputTransformation.allCaps(locale={locale:?})"),
            Kind::MaxLength(n) => write!(f, "InputTransformation.maxLength({n})"),
        }
    }
}

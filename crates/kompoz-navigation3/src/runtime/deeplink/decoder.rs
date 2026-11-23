//! DeepLinkDecoder.kt: a navigation key read from a deep link's arguments (Compose's `DeepLinkDecoder` and
//! `ListDecoder`, kotlinx.serialization `AbstractDecoder`s).
//!
//! kotlinx.serialization is `Saveable` here: the decoder walks the key type's [`SerialDescriptor`] as Kotlin's
//! decoder is walked by the type's serializer, and builds the [`SavedValue`] the type's `restore` reads — a primitive
//! as its saved value (an integer `Int`, a floating point number `Double`, a boolean `Bool`, a text `String`, a char a
//! text of it), a class a map of its elements by name (an object an empty map), a list a list, an enum its entry's
//! index, a nullable value (an `Option`) a list of none or one value — then has the [`Saver`] restore it.

use kompoz_runtime::kt::{self, KtMap, panic_kt};
use kompoz_runtime::{SavedValue, Saver, SerialDescriptor, SerialKind};
use std::collections::BTreeMap;

/// A required argument missing, or one not of its type (Compose's internal `DeepLinkDecoderException`, a
/// `SerializationException`): the request doesn't match.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DeepLinkDecoderException(pub(crate) Option<String>);

impl DeepLinkDecoderException {
    fn new(message: impl Into<String>) -> DeepLinkDecoderException {
        DeepLinkDecoderException(Some(message.into()))
    }
}

type Arguments = KtMap<String, Vec<String>>;

/// kotlinx's `SerialKind`'s text (`toString()`).
fn kind_name(kind: SerialKind) -> &'static str {
    match kind {
        SerialKind::Boolean => "BOOLEAN",
        SerialKind::Byte => "BYTE",
        SerialKind::Short => "SHORT",
        SerialKind::Int => "INT",
        SerialKind::Long => "LONG",
        SerialKind::Float => "FLOAT",
        SerialKind::Double => "DOUBLE",
        SerialKind::Char => "CHAR",
        SerialKind::String => "STRING",
        SerialKind::Enum => "ENUM",
        SerialKind::Class => "CLASS",
        SerialKind::Object => "OBJECT",
        SerialKind::List => "LIST",
        SerialKind::Map => "MAP",
        SerialKind::Contextual => "CONTEXTUAL",
    }
}

/// Decodes arguments from a flat map of texts (a deep link's) into a value (Compose's `DeepLinkDecoder`).
pub(crate) struct DeepLinkDecoder<'a> {
    arguments: &'a Arguments,
}

impl<'a> DeepLinkDecoder<'a> {
    pub(crate) fn new(arguments: &'a Arguments) -> DeepLinkDecoder<'a> {
        DeepLinkDecoder { arguments }
    }

    /// A `T` of the arguments (Kotlin's `decodeSerializableValue(serializer)`): `Err` if a required argument is
    /// missing or one isn't of its type; `Ok(None)` if `serializer` doesn't restore a value of what was decoded
    /// (where Kotlin's serializer would throw).
    ///
    /// # Panics
    /// As Kotlin throws: for a map (IllegalArgumentException), a list of other than primitives or a value of a kind
    /// read only by its own serializer (SerializationException).
    #[track_caller]
    pub(crate) fn decode_serializable_value<T: 'static>(
        &self,
        serializer: &Saver<T>,
    ) -> Result<Option<T>, DeepLinkDecoderException> {
        let saved = self.decode_value(&serializer.descriptor(), "")?;
        Ok(serializer.restore(&saved))
    }

    /// The value `descriptor` describes, of the argument `name` (Kotlin's decoder with its `currentName`).
    #[track_caller]
    fn decode_value(&self, descriptor: &SerialDescriptor, name: &str) -> Result<SavedValue, DeepLinkDecoderException> {
        if descriptor.nullable {
            // (kotlinx's nullable serializer: decodeNotNullMark, else decodeNull)
            if !self.arguments.contains_key(name.to_string()) {
                return Ok(SavedValue::list([]));
            }
            let mut inner = descriptor.clone();
            inner.nullable = false;
            return Ok(SavedValue::list([self.decode_value(&inner, name)?]));
        }
        match descriptor.kind {
            SerialKind::Class | SerialKind::Object => self.decode_structure(descriptor),
            SerialKind::List => {
                // (the element descriptor is the collection's type parameter)
                let element = descriptor.element_descriptor(0);
                if !element.kind.is_primitive() {
                    panic_kt(
                        "SerializationException",
                        &format!(
                            "Cannot decode collection of type with serial name {}. Only collections of primitives \
                             are supported.",
                            element.serial_name
                        ),
                    );
                }
                let values = self.arguments.get_ref(&name.to_string()).ok_or_else(|| {
                    DeepLinkDecoderException::new(format!("Argument value not found for argument name [{name}]"))
                })?;
                ListDecoder { values }.decode(&element)
            }
            SerialKind::Map => panic_kt(
                "IllegalArgumentException",
                &format!(
                    "Cannot decode type with serial name {}. Map decoding is not supported.",
                    descriptor.serial_name
                ),
            ),
            SerialKind::Enum => {
                let value = self.decode_string(name)?;
                match descriptor.element_index(value) {
                    Some(index) => Ok(SavedValue::Int(index as i64)),
                    None => Err(DeepLinkDecoderException::new(format!("Unknown ENUM value: {value}"))),
                }
            }
            SerialKind::Contextual => panic_kt(
                "SerializationException",
                &format!(
                    "Cannot decode field [{name}] for type with serial name {}. Unsupported structure kind: {}",
                    descriptor.serial_name,
                    kind_name(descriptor.kind)
                ),
            ),
            kind => decode_primitive(kind, self.decode_string(name)?),
        }
    }

    /// Kotlin's `decodeString()`: the argument's first value.
    fn decode_string(&self, name: &str) -> Result<&'a str, DeepLinkDecoderException> {
        self.arguments.get_ref(&name.to_string()).and_then(|values| values.first()).map(String::as_str).ok_or_else(
            || DeepLinkDecoderException::new(format!("Argument value not found for argument name [{name}]")),
        )
    }

    /// A class's or an object's elements, each of the argument of its name (Kotlin's `beginStructure` of a class: a
    /// decoder of the same arguments, and its `decodeElementIndex`).
    #[track_caller]
    fn decode_structure(&self, descriptor: &SerialDescriptor) -> Result<SavedValue, DeepLinkDecoderException> {
        let mut elements = BTreeMap::new();
        for index in 0..descriptor.elements.len() {
            let name = descriptor.element_name(index);
            let element = descriptor.element_descriptor(index);
            let kind = element.kind;
            // Unsupported collections.
            if kind == SerialKind::Map {
                panic_kt(
                    "IllegalArgumentException",
                    &format!(
                        "Cannot decode field [{name}] for type with serial name{}. Map decoding is not supported.",
                        descriptor.serial_name
                    ),
                );
            }
            // Non-nested structures: missing, or empty for other than a text, is left to its default if it has one.
            if kind.is_primitive() || kind == SerialKind::Enum || kind == SerialKind::List {
                let values = self.arguments.get_ref(&name.to_string());
                let missing = match values {
                    None => true,
                    // (Kotlin's `first()` of no values: a NoSuchElementException, not a mismatch)
                    Some(values) if kind != SerialKind::String && values.is_empty() => {
                        panic_kt("NoSuchElementException", "List is empty.")
                    }
                    Some(values) => kind != SerialKind::String && values[0].is_empty(),
                };
                if missing {
                    if descriptor.is_element_optional(index) {
                        continue;
                    }
                    return Err(DeepLinkDecoderException::new(format!("Missing argument for required field [{name}]")));
                }
            }
            // Nested structures (classes) always: they look for their own elements in the same arguments.
            let value = self.decode_value(&element, name)?;
            elements.insert(name.to_string(), value);
        }
        Ok(SavedValue::Map(std::sync::Arc::new(elements)))
    }
}

/// Decodes a collection of primitives from its values (Compose's `ListDecoder`): each one an element, `"null"` none
/// for a nullable element.
struct ListDecoder<'a> {
    values: &'a [String],
}

impl ListDecoder<'_> {
    fn decode(&self, element: &SerialDescriptor) -> Result<SavedValue, DeepLinkDecoderException> {
        let mut items = Vec::with_capacity(self.values.len());
        for value in self.values {
            let item = if element.nullable {
                if value == "null" {
                    SavedValue::list([])
                } else {
                    SavedValue::list([decode_primitive(element.kind, value)?])
                }
            } else {
                decode_primitive(element.kind, value)?
            };
            items.push(item);
        }
        Ok(SavedValue::list(items))
    }
}

/// A primitive of `value` as Kotlin's `toInt()`, `toBooleanStrict()`, … read it (Java's `parseInt`, …), its failure a
/// [`DeepLinkDecoderException`] of the exception's message.
fn decode_primitive(kind: SerialKind, value: &str) -> Result<SavedValue, DeepLinkDecoderException> {
    let number_format = || DeepLinkDecoderException::new(format!("For input string: \"{value}\""));
    let integer = |min: i64, max: i64| -> Result<SavedValue, DeepLinkDecoderException> {
        let n = kt::KtStr::to_long_or_null(&value).ok_or_else(number_format)?;
        if n < min || n > max {
            return Err(if kind == SerialKind::Byte || kind == SerialKind::Short {
                DeepLinkDecoderException::new(format!("Value out of range. Value:\"{value}\" Radix:10"))
            } else {
                number_format()
            });
        }
        Ok(SavedValue::Int(n))
    };
    match kind {
        SerialKind::String => Ok(SavedValue::String(value.to_string())),
        SerialKind::Int => integer(i32::MIN as i64, i32::MAX as i64),
        SerialKind::Long => integer(i64::MIN, i64::MAX),
        SerialKind::Short => integer(i16::MIN as i64, i16::MAX as i64),
        SerialKind::Byte => integer(i8::MIN as i64, i8::MAX as i64),
        SerialKind::Boolean => match value {
            "true" => Ok(SavedValue::Bool(true)),
            "false" => Ok(SavedValue::Bool(false)),
            _ => Err(DeepLinkDecoderException::new(format!("The string doesn't represent a boolean value: {value}"))),
        },
        SerialKind::Float => {
            let f = kt::KtStr::to_float_or_null(&value).ok_or_else(|| float_error(value))?;
            Ok(SavedValue::Double(f as f64))
        }
        SerialKind::Double => {
            let d = kt::KtStr::to_double_or_null(&value).ok_or_else(|| float_error(value))?;
            Ok(SavedValue::Double(d))
        }
        SerialKind::Char => match value.chars().next() {
            Some(c) => Ok(SavedValue::String(c.to_string())),
            None => Err(DeepLinkDecoderException::new("Char sequence is empty.")),
        },
        _ => unreachable!("not a primitive kind: {kind:?}"),
    }
}

/// Java's `parseFloat`/`parseDouble`'s message.
fn float_error(value: &str) -> DeepLinkDecoderException {
    if value.trim_matches(|c: char| c <= ' ').is_empty() {
        DeepLinkDecoderException::new("empty String")
    } else {
        DeepLinkDecoderException::new(format!("For input string: \"{value}\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_runtime::{Saveable, SerialElement};

    #[derive(Debug, PartialEq)]
    struct Pair {
        a: i32,
        b: Option<String>,
    }

    impl Saveable for Pair {
        fn descriptor() -> SerialDescriptor {
            SerialDescriptor::class(
                "Pair",
                vec![SerialElement::of::<i32>("a", false), SerialElement::of::<Option<String>>("b", true)],
            )
        }
        fn save(&self) -> SavedValue {
            SavedValue::map([("a", self.a.save()), ("b", self.b.save())])
        }
        fn restore(saved: &SavedValue) -> Option<Self> {
            let b = match saved.as_map()?.get("b") {
                Some(b) => Option::<String>::restore(b)?,
                None => None,
            };
            Some(Pair { a: i32::restore(saved.get("a"))?, b })
        }
    }

    fn args(entries: &[(&str, &[&str])]) -> Arguments {
        entries.iter().map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect())).collect()
    }

    #[test]
    fn primitives_are_read_as_kotlin_reads_them() {
        assert_eq!(decode_primitive(SerialKind::Int, "+12"), Ok(SavedValue::Int(12)));
        assert_eq!(
            decode_primitive(SerialKind::Int, "12x"),
            Err(DeepLinkDecoderException::new("For input string: \"12x\""))
        );
        assert!(decode_primitive(SerialKind::Byte, "300").is_err());
        assert_eq!(decode_primitive(SerialKind::Boolean, "true"), Ok(SavedValue::Bool(true)));
        assert!(decode_primitive(SerialKind::Boolean, "True").is_err());
        assert_eq!(decode_primitive(SerialKind::Float, "4.5"), Ok(SavedValue::Double(4.5)));
        assert_eq!(decode_primitive(SerialKind::Char, "xy"), Ok(SavedValue::String("x".into())));
        assert!(decode_primitive(SerialKind::Char, "").is_err());
    }

    #[test]
    fn classes_lists_and_nullables_are_built_as_their_types_restore_them() {
        let saver = Saver::<Pair>::of();
        let a = args(&[("a", &["1"]), ("b", &["x"])]);
        assert_eq!(
            DeepLinkDecoder::new(&a).decode_serializable_value(&saver),
            Ok(Some(Pair { a: 1, b: Some("x".into()) }))
        );
        let a = args(&[("a", &["1"])]);
        assert_eq!(DeepLinkDecoder::new(&a).decode_serializable_value(&saver), Ok(Some(Pair { a: 1, b: None })));
        let a = args(&[("a", &[""])]);
        assert!(DeepLinkDecoder::new(&a).decode_serializable_value(&saver).is_err());
        let list = Saver::<Vec<Option<i32>>>::of();
        let a = args(&[("", &["1", "null", "3"])]);
        assert_eq!(DeepLinkDecoder::new(&a).decode_serializable_value(&list), Ok(Some(vec![Some(1), None, Some(3)])));
    }
}

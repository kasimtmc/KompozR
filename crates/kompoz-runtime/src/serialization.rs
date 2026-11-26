//! kotlinx.serialization's serializers of one's own (stage 14 FINAL): a class implementing `KSerializer<T>` —
//! its `descriptor`, `serialize(encoder, value)`, `deserialize(decoder)` — as an app writes one for a type that has
//! none (`object UUIDSerializer : KSerializer<UUID>`: a `java.util.UUID` as its text).
//!
//! KompozR's serializers are [`Saver`]s (what `encodeToSavedState(serializer, value)`, `handle.saved(serializer)`,
//! `@Serializable(with = …)` take): a [`KSerializerImpl`] — the Kotlin syntax's class or object deriving from
//! `KSerializer<T>` implements it, as kotlinx's subclasses do the interface — writes a value to an [`Encoder`] (a
//! [`SavedValue`]: a primitive, or another serializer's value) and reads it back from a [`Decoder`]; it is given
//! wherever a serializer is ([`IntoSaver`]), and [`KSerializer`] holds one as a value.
//!
//! ```
//! use kompoz_runtime::kt::UUID;
//! use kompoz_runtime::{Decoder, Encoder, KSerializerImpl, PrimitiveKind, PrimitiveSerialDescriptor, IntoSaver};
//!
//! struct UUIDSerializer;
//!
//! impl KSerializerImpl for UUIDSerializer {
//!     type Value = UUID;
//!     fn descriptor(&self) -> kompoz_runtime::SerialDescriptor {
//!         PrimitiveSerialDescriptor("UUID", PrimitiveKind::STRING)
//!     }
//!     fn serialize(&self, encoder: &Encoder, value: &UUID) {
//!         encoder.encode_string(value.to_string());
//!     }
//!     fn deserialize(&self, decoder: &Decoder) -> UUID {
//!         UUID::from_string(&decoder.decode_string())
//!     }
//! }
//!
//! let saver = UUIDSerializer.into_saver();
//! let id = UUID::random_uuid();
//! let saved = saver.save(&id);
//! assert_eq!(saved.as_str(), Some(id.to_string().as_str()));
//! assert_eq!(saver.restore(&saved), Some(id));
//! assert_eq!(saver.descriptor().serial_name, "UUID");
//! ```

use crate::kt::panic_kt;
use crate::{SavedValue, Saver, SerialDescriptor, SerialKind};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// kotlinx's `PrimitiveKind`: the kinds a primitive descriptor is of (`PrimitiveKind.STRING`).
///
/// Kotlin: `PrimitiveKind`
pub struct PrimitiveKind;

impl PrimitiveKind {
    pub const BOOLEAN: SerialKind = SerialKind::Boolean;
    pub const BYTE: SerialKind = SerialKind::Byte;
    pub const CHAR: SerialKind = SerialKind::Char;
    pub const SHORT: SerialKind = SerialKind::Short;
    pub const INT: SerialKind = SerialKind::Int;
    pub const LONG: SerialKind = SerialKind::Long;
    pub const FLOAT: SerialKind = SerialKind::Float;
    pub const DOUBLE: SerialKind = SerialKind::Double;
    pub const STRING: SerialKind = SerialKind::String;
}

/// A descriptor of a primitive `kind` named `serial_name` (kotlinx's `PrimitiveSerialDescriptor`).
///
/// Kotlin: `PrimitiveSerialDescriptor(serialName: String, kind: PrimitiveKind): SerialDescriptor`
#[allow(non_snake_case)]
pub fn PrimitiveSerialDescriptor(serial_name: &str, kind: SerialKind) -> SerialDescriptor {
    SerialDescriptor::new(serial_name, kind)
}

/// What a serializer writes a value to (kotlinx's `Encoder`): one value, a primitive or another serializer's. A clone
/// writes to the same place.
#[derive(Clone, Default)]
pub struct Encoder {
    out: Rc<RefCell<Option<SavedValue>>>,
}

impl fmt::Debug for Encoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Encoder({:?})", self.out.borrow())
    }
}

impl Encoder {
    /// An encoder of nothing written yet.
    pub fn new() -> Encoder {
        Encoder::default()
    }

    fn put(&self, v: SavedValue) {
        *self.out.borrow_mut() = Some(v);
    }

    /// What was written (none: nothing).
    pub fn written(&self) -> Option<SavedValue> {
        self.out.borrow().clone()
    }

    /// Kotlin: `encodeString(value: String)`
    pub fn encode_string(&self, value: impl Into<String>) {
        self.put(SavedValue::from(value.into()));
    }
    /// Kotlin: `encodeBoolean(value: Boolean)`
    pub fn encode_boolean(&self, value: bool) {
        self.put(SavedValue::from(value));
    }
    /// Kotlin: `encodeInt(value: Int)`
    pub fn encode_int(&self, value: i32) {
        self.put(SavedValue::from(value));
    }
    /// Kotlin: `encodeLong(value: Long)`
    pub fn encode_long(&self, value: i64) {
        self.put(SavedValue::from(value));
    }
    /// Kotlin: `encodeFloat(value: Float)`
    pub fn encode_float(&self, value: f32) {
        self.put(SavedValue::from(value));
    }
    /// Kotlin: `encodeDouble(value: Double)`
    pub fn encode_double(&self, value: f64) {
        self.put(SavedValue::from(value));
    }
    /// Kotlin: `encodeChar(value: Char)`
    pub fn encode_char(&self, value: char) {
        self.put(crate::Saveable::save(&value));
    }
    /// `value` written by `serializer`.
    ///
    /// Kotlin: `encodeSerializableValue(serializer: SerializationStrategy<T>, value: T)`
    pub fn encode_serializable_value<T: 'static>(&self, serializer: impl IntoSaver<T>, value: T) {
        self.put(serializer.into_saver().save(&value));
    }
}

/// What a serializer reads a value from (kotlinx's `Decoder`): what its encoder was given. A value of another kind
/// than read is, as in kotlinx, a `SerializationException` (a panic).
#[derive(Clone, Debug)]
pub struct Decoder {
    value: SavedValue,
}

impl Decoder {
    /// A decoder of `value`.
    pub fn new(value: SavedValue) -> Decoder {
        Decoder { value }
    }

    #[track_caller]
    fn wrong(&self, what: &str) -> ! {
        panic_kt("SerializationException", &format!("expected {what}, found {:?}", self.value))
    }

    /// Kotlin: `decodeString(): String`
    #[track_caller]
    pub fn decode_string(&self) -> String {
        self.value.as_str().map(str::to_string).unwrap_or_else(|| self.wrong("a string"))
    }
    /// Kotlin: `decodeBoolean(): Boolean`
    #[track_caller]
    pub fn decode_boolean(&self) -> bool {
        crate::Saveable::restore(&self.value).unwrap_or_else(|| self.wrong("a boolean"))
    }
    /// Kotlin: `decodeInt(): Int`
    #[track_caller]
    pub fn decode_int(&self) -> i32 {
        crate::Saveable::restore(&self.value).unwrap_or_else(|| self.wrong("an int"))
    }
    /// Kotlin: `decodeLong(): Long`
    #[track_caller]
    pub fn decode_long(&self) -> i64 {
        crate::Saveable::restore(&self.value).unwrap_or_else(|| self.wrong("a long"))
    }
    /// Kotlin: `decodeFloat(): Float`
    #[track_caller]
    pub fn decode_float(&self) -> f32 {
        crate::Saveable::restore(&self.value).unwrap_or_else(|| self.wrong("a float"))
    }
    /// Kotlin: `decodeDouble(): Double`
    #[track_caller]
    pub fn decode_double(&self) -> f64 {
        crate::Saveable::restore(&self.value).unwrap_or_else(|| self.wrong("a double"))
    }
    /// Kotlin: `decodeChar(): Char`
    #[track_caller]
    pub fn decode_char(&self) -> char {
        crate::Saveable::restore(&self.value).unwrap_or_else(|| self.wrong("a char"))
    }
    /// The value `deserializer` reads.
    ///
    /// Kotlin: `decodeSerializableValue(deserializer: DeserializationStrategy<T>): T`
    #[track_caller]
    pub fn decode_serializable_value<T: 'static>(&self, deserializer: impl IntoSaver<T>) -> T {
        deserializer.into_saver().restore(&self.value).unwrap_or_else(|| self.wrong("its serializer's value"))
    }
}

/// A serializer of one's own (kotlinx's `KSerializer<T>` interface, as the class deriving from it implements it):
/// its descriptor, how it writes a value, how it reads one back. Given wherever a serializer is ([`IntoSaver`]).
///
/// Kotlin: `abstract class KSerializer<T>()`
pub trait KSerializerImpl: 'static {
    /// Kotlin's `T`.
    type Value: 'static;

    /// What its values are (a primitive's: [`PrimitiveSerialDescriptor`]).
    /// Kotlin: `descriptor: SerialDescriptor`
    fn descriptor(&self) -> SerialDescriptor;

    /// `value` written to `encoder`.
    /// Kotlin: `serialize(encoder: Encoder, value: T)`
    fn serialize(&self, encoder: &Encoder, value: &Self::Value);

    /// The value `decoder` holds, read back.
    /// Kotlin: `deserialize(decoder: Decoder): T`
    fn deserialize(&self, decoder: &Decoder) -> Self::Value;
}

/// A [`KSerializerImpl`] as a value (kotlinx's `KSerializer<T>`): a clone is the same serializer.
pub struct KSerializer<T: 'static> {
    imp: Rc<dyn KSerializerImpl<Value = T>>,
}

impl<T: 'static> Clone for KSerializer<T> {
    fn clone(&self) -> Self {
        KSerializer { imp: self.imp.clone() }
    }
}

impl<T: 'static> fmt::Debug for KSerializer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KSerializer({})", self.imp.descriptor().serial_name)
    }
}

impl<T: 'static> KSerializer<T> {
    /// The serializer `imp` is.
    pub fn new(imp: impl KSerializerImpl<Value = T>) -> KSerializer<T> {
        KSerializer { imp: Rc::new(imp) }
    }

    /// Kotlin: `descriptor: SerialDescriptor`
    pub fn descriptor(&self) -> SerialDescriptor {
        self.imp.descriptor()
    }

    /// Kotlin: `serialize(encoder: Encoder, value: T)`
    pub fn serialize(&self, encoder: &Encoder, value: &T) {
        self.imp.serialize(encoder, value)
    }

    /// Kotlin: `deserialize(decoder: Decoder): T`
    pub fn deserialize(&self, decoder: &Decoder) -> T {
        self.imp.deserialize(decoder)
    }

    /// It as KompozR's serializer: a value saved as it writes it (nothing written: a null), read back by it (none
    /// where it throws).
    pub fn saver(&self) -> Saver<T> {
        let (save, restore, describe) = (self.imp.clone(), self.imp.clone(), self.imp.clone());
        Saver::with_descriptor_of(
            move |v| {
                let e = Encoder::new();
                save.serialize(&e, v);
                e.written().unwrap_or(SavedValue::Null)
            },
            // (a value it can't read, the `SerializationException` it throws: none, as a saver's restore gives)
            move |s| crate::kt::catching(|| restore.deserialize(&Decoder::new(s.clone()))).ok(),
            move || describe.descriptor(),
        )
    }
}

/// What is given where a serializer is taken (kotlinx's `SerializationStrategy<T>`, `DeserializationStrategy<T>`,
/// `KSerializer<T>` parameters): a [`Saver`], a [`KSerializer`], a [`KSerializerImpl`]'s object (the Kotlin
/// syntax's `UUIDSerializer()`, `UUIDSerializer`).
pub trait IntoSaver<T> {
    /// It as a [`Saver`].
    fn into_saver(self) -> Saver<T>;
}

impl<T: 'static> IntoSaver<T> for Saver<T> {
    fn into_saver(self) -> Saver<T> {
        self
    }
}

impl<T: 'static> IntoSaver<T> for &Saver<T> {
    fn into_saver(self) -> Saver<T> {
        self.clone()
    }
}

impl<T: 'static> IntoSaver<T> for KSerializer<T> {
    fn into_saver(self) -> Saver<T> {
        self.saver()
    }
}

impl<T: 'static> IntoSaver<T> for &KSerializer<T> {
    fn into_saver(self) -> Saver<T> {
        self.saver()
    }
}

impl<S: KSerializerImpl> IntoSaver<S::Value> for S {
    fn into_saver(self) -> Saver<S::Value> {
        KSerializer::new(self).saver()
    }
}

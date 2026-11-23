//! DeepLinkSerializer.kt: a value of a navigation key's argument written as a text ([`DeepLinkSerializer`]).

use kompoz_runtime::{SavedValue, Saver, SerialDescriptor, SerialKind};

/// A serializer writing a value as a text, and reading it back (Compose's abstract class `DeepLinkSerializer<T>`, a
/// `KSerializer<T>` of a `STRING` primitive descriptor): what gives a navigation key's argument of a type of one's
/// own (not a primitive) its value from a deep link's argument, as it does the saved value of a key saved with its
/// back stack.
///
/// Kotlin's serializers of it are `object`s: here a type implementing it (a unit struct), its functions associated.
/// Its [`saver`](Self::saver) is the serializer given where Kotlin gives the object (`UriDeepLinkMatcher(…,
/// serializer = ProductSerializer)`); a field of the type is read through it once the type is `Saveable` through it,
/// Kotlin's `@Serializable(with = ProductSerializer::class) val product: Product` — [`serializable_with!`]:
///
/// ```
/// use kompoz_navigation3::runtime::deeplink::{DeepLinkSerializer, serializable_with};
/// use kompoz_runtime::kt::panic_kt;
///
/// #[derive(Clone, Debug, PartialEq)]
/// struct Product {
///     id: i32,
///     name: String,
/// }
///
/// struct ProductSerializer;
///
/// impl DeepLinkSerializer for ProductSerializer {
///     type Value = Product;
///     fn serial_name() -> String {
///         "androidx.navigation3.ui.samples.Product".into()
///     }
///     fn deserialize(value: &str) -> Product {
///         let (id, name) = value.split_once('-').unwrap_or((value, ""));
///         let Ok(id) = id.parse() else { panic_kt("SerializationException", &format!("Invalid product id: {id}")) };
///         Product { id, name: name.to_string() }
///     }
///     fn serialize(value: &Product) -> String {
///         format!("{}-{}", value.id, value.name)
///     }
/// }
///
/// serializable_with!(Product, ProductSerializer);
/// ```
pub trait DeepLinkSerializer: 'static {
    /// Kotlin's `T`.
    type Value: 'static;

    /// The unique serial name of the type it serializes.
    /// Kotlin: `serialName: String`
    fn serial_name() -> String;

    /// `value` read as a value (what [`serialize`](Self::serialize) writes). A text it can't read is, as in Kotlin,
    /// the serializer's own exception to throw: a panic (`SerializationException`).
    /// Kotlin: `deserialize(value: String): T`
    fn deserialize(value: &str) -> Self::Value;

    /// `value` written as a text (what [`deserialize`](Self::deserialize) reads).
    /// Kotlin: `serialize(value: T): String`
    fn serialize(value: &Self::Value) -> String;

    /// A `STRING` primitive of its serial name.
    /// Kotlin: `descriptor: SerialDescriptor`
    fn descriptor() -> SerialDescriptor {
        SerialDescriptor::new(Self::serial_name(), SerialKind::String)
    }

    /// It as a serializer (Kotlin's object as the `KSerializer<T>` it is): a value saved as the text it writes, read
    /// back by reading that text.
    fn saver() -> Saver<Self::Value>
    where
        Self: Sized,
    {
        Saver::with_descriptor(
            |value| SavedValue::String(Self::serialize(value)),
            |saved| saved.as_str().map(Self::deserialize),
            Self::descriptor,
        )
    }
}

/// Makes a type `Saveable` through a [`DeepLinkSerializer`] of it: `serializable_with!(Product, ProductSerializer)`
/// (Kotlin's `@Serializable(with = ProductSerializer::class)` on the fields of the type, here on the type: every
/// field and list element of it is read and saved through it).
#[macro_export]
#[doc(hidden)]
macro_rules! __serializable_with {
    ($t:ty, $serializer:ty $(,)?) => {
        impl $crate::runtime::deeplink::__private::Saveable for $t {
            fn descriptor() -> $crate::runtime::deeplink::__private::SerialDescriptor {
                <$serializer as $crate::runtime::deeplink::DeepLinkSerializer>::descriptor()
            }
            fn save(&self) -> $crate::runtime::deeplink::__private::SavedValue {
                $crate::runtime::deeplink::__private::SavedValue::String(
                    <$serializer as $crate::runtime::deeplink::DeepLinkSerializer>::serialize(self),
                )
            }
            fn restore(saved: &$crate::runtime::deeplink::__private::SavedValue) -> ::std::option::Option<Self> {
                saved.as_str().map(<$serializer as $crate::runtime::deeplink::DeepLinkSerializer>::deserialize)
            }
        }
    };
}

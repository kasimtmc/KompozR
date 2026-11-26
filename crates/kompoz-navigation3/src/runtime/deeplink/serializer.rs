//! DeepLinkSerializer.kt: a value of a navigation key's argument written as a text ([`DeepLinkSerializerImpl`], a
//! serializer as a value [`DeepLinkSerializer`]).

use kompoz_runtime::{SavedValue, Saver, SerialDescriptor, SerialKind};
use std::fmt;
use std::rc::Rc;

/// A serializer writing a value as a text, and reading it back (Compose's abstract class `DeepLinkSerializer<T>`, a
/// `KSerializer<T>` of a `STRING` primitive descriptor): what gives a navigation key's argument of a type of one's
/// own (not a primitive) its value from a deep link's argument, as it does the saved value of a key saved with its
/// back stack.
///
/// Kotlin's serializers of it are `object`s: here a type implementing it (a unit struct, `Default`: what
/// [`serializable_with!`] makes the one object of); the Kotlin syntax's `object ProductSerializer :
/// DeepLinkSerializer<Product>() { override fun deserialize(…) … }` implements it. Its [`saver`](Self::saver) is the
/// serializer given where Kotlin gives the object (`UriDeepLinkMatcher(…, serializer = ProductSerializer)`); a field
/// of the type is read through it once the type is `Saveable` through it, Kotlin's `@Serializable(with =
/// ProductSerializer::class) val product: Product` — [`serializable_with!`]:
///
/// ```
/// use kompoz_navigation3::runtime::deeplink::{DeepLinkSerializerImpl, serializable_with};
/// use kompoz_runtime::kt::panic_kt;
///
/// #[derive(Clone, Debug, PartialEq)]
/// struct Product {
///     id: i32,
///     name: String,
/// }
///
/// #[derive(Default)]
/// struct ProductSerializer;
///
/// impl DeepLinkSerializerImpl for ProductSerializer {
///     type Value = Product;
///     fn serial_name(&self) -> String {
///         "androidx.navigation3.ui.samples.Product".into()
///     }
///     fn deserialize(&self, value: &str) -> Product {
///         let (id, name) = value.split_once('-').unwrap_or((value, ""));
///         let Ok(id) = id.parse() else { panic_kt("SerializationException", &format!("Invalid product id: {id}")) };
///         Product { id, name: name.to_string() }
///     }
///     fn serialize(&self, value: &Product) -> String {
///         format!("{}-{}", value.id, value.name)
///     }
/// }
///
/// serializable_with!(Product, ProductSerializer);
/// ```
///
/// Kotlin: `abstract class DeepLinkSerializer<T>()`
pub trait DeepLinkSerializerImpl: 'static {
    /// Kotlin's `T`.
    type Value: 'static;

    /// The unique serial name of the type it serializes.
    /// Kotlin: `serialName: String`
    fn serial_name(&self) -> String;

    /// `value` read as a value (what [`serialize`](Self::serialize) writes). A text it can't read is, as in Kotlin,
    /// the serializer's own exception to throw: a panic (`SerializationException`).
    /// Kotlin: `deserialize(value: String): T`
    fn deserialize(&self, value: &str) -> Self::Value;

    /// `value` written as a text (what [`deserialize`](Self::deserialize) reads).
    /// Kotlin: `serialize(value: T): String`
    fn serialize(&self, value: &Self::Value) -> String;

    /// A `STRING` primitive of its serial name.
    /// Kotlin: `descriptor: SerialDescriptor`
    fn descriptor(&self) -> SerialDescriptor {
        SerialDescriptor::new(self.serial_name(), SerialKind::String)
    }

    /// It as a serializer (Kotlin's object as the `KSerializer<T>` it is): a value saved as the text it writes, read
    /// back by reading that text.
    fn saver() -> Saver<Self::Value>
    where
        Self: Default + Sized,
    {
        Saver::with_descriptor(
            |value| SavedValue::String(Self::default().serialize(value)),
            |saved| saved.as_str().map(|s| Self::default().deserialize(s)),
            descriptor_of::<Self>,
        )
    }
}

fn descriptor_of<S: DeepLinkSerializerImpl + Default>() -> SerialDescriptor {
    S::default().descriptor()
}

/// A [`DeepLinkSerializerImpl`] as a value (Kotlin's object of a `DeepLinkSerializer<T>` subclass, as the type
/// `DeepLinkSerializer<T>`): a clone is the same serializer.
pub struct DeepLinkSerializer<T: 'static> {
    imp: Rc<dyn DeepLinkSerializerImpl<Value = T>>,
}

impl<T: 'static> Clone for DeepLinkSerializer<T> {
    fn clone(&self) -> Self {
        DeepLinkSerializer { imp: self.imp.clone() }
    }
}

impl<T: 'static> fmt::Debug for DeepLinkSerializer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DeepLinkSerializer({})", self.imp.serial_name())
    }
}

impl<T: 'static> DeepLinkSerializer<T> {
    /// The serializer `imp` is.
    pub fn new(imp: impl DeepLinkSerializerImpl<Value = T>) -> DeepLinkSerializer<T> {
        DeepLinkSerializer { imp: Rc::new(imp) }
    }

    /// Kotlin: `serialName: String`
    pub fn serial_name(&self) -> String {
        self.imp.serial_name()
    }

    /// Kotlin: `deserialize(value: String): T`
    pub fn deserialize(&self, value: &str) -> T {
        self.imp.deserialize(value)
    }

    /// Kotlin: `serialize(value: T): String`
    pub fn serialize(&self, value: &T) -> String {
        self.imp.serialize(value)
    }

    /// Kotlin: `descriptor: SerialDescriptor`
    pub fn descriptor(&self) -> SerialDescriptor {
        self.imp.descriptor()
    }
}

/// Makes a type `Saveable` through a [`DeepLinkSerializerImpl`] of it: `serializable_with!(Product,
/// ProductSerializer)` (Kotlin's `@Serializable(with = ProductSerializer::class)` on the fields of the type, here on
/// the type: every field and list element of it is read and saved through it; the Kotlin syntax writes it for such a
/// field of a `@Serializable` class).
#[macro_export]
#[doc(hidden)]
macro_rules! __serializable_with {
    ($t:ty, $serializer:ty $(,)?) => {
        impl $crate::runtime::deeplink::__private::Saveable for $t {
            fn descriptor() -> $crate::runtime::deeplink::__private::SerialDescriptor {
                $crate::runtime::deeplink::DeepLinkSerializerImpl::descriptor(
                    &<$serializer as ::core::default::Default>::default(),
                )
            }
            fn save(&self) -> $crate::runtime::deeplink::__private::SavedValue {
                $crate::runtime::deeplink::__private::SavedValue::String(
                    $crate::runtime::deeplink::DeepLinkSerializerImpl::serialize(
                        &<$serializer as ::core::default::Default>::default(),
                        self,
                    ),
                )
            }
            fn restore(saved: &$crate::runtime::deeplink::__private::SavedValue) -> ::std::option::Option<Self> {
                saved.as_str().map(|s| {
                    $crate::runtime::deeplink::DeepLinkSerializerImpl::deserialize(
                        &<$serializer as ::core::default::Default>::default(),
                        s,
                    )
                })
            }
        }
    };
}

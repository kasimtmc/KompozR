//! Typed routes (Compose's `@Serializable` routes and its `androidx.navigation.serialization`: `RouteBuilder`,
//! `RouteEncoder`, `RouteDecoder`, `NavTypeConverter`).
//!
//! A route is a struct with `#[derive(Route)]` (a unit struct is Kotlin's `object`): its base name is the type's
//! name (`#[route(name = "…")]` gives another, Kotlin's `@SerialName`), each field an argument. A field is a path
//! segment (`Detail/{id}`) unless it has a default — an `Option` (default `None`), a `#[route(default)]` or
//! `#[route(default = expr)]` field — or a collection type, which make it a query parameter
//! (`?tab={tab}`; a collection's repeated, `?ids=1&ids=2`): Compose's `computeParamType`. A field's type is a
//! [`RouteValue`]: integers (`i32`, `i64`), floats (`f32`, `f64`), `bool`, `String`, enums with
//! `#[derive(RouteValue)]`, their `Option`s, `Vec`s (lists) and `Box<[…]>`s (arrays) — each with its `NavType` as
//! Compose's (`IntType`, `StringType` …) — or a type of one's own whose `NavType` the `typeMap` gives (keyed by
//! [`type_of`]`::<T>()`, Kotlin's `typeOf<T>()`).
//!
//! A value's route is its pattern filled: `Detail(id = 7, tab = null)` → `Detail/7?tab=null` (a `String` written
//! encoded, a float as Java writes it); its arguments read back give the value ([`decode_arguments`],
//! `entry.to_route::<T>()`).

use crate::argument::{NamedNavArgument, NavArgumentBuilder};
use crate::java::string_hash;
use crate::nav_type::{NavEnum, NavType, enum_list_nav_type, enum_nav_type, enum_nullable_nav_type};
use crate::saved_state::{SavedState, contains};
use kompoz_runtime::kt::KtMap;
pub use kompoz_runtime::kt::{KType, type_of};
use std::any::{Any, TypeId};
use std::fmt;

/// A typed route's `typeMap`: the `NavType`s of its fields' types that aren't built in (Kotlin's
/// `Map<KType, NavType<*>>`; `map_of([(type_of::<T>(), MyType)])`).
pub type TypeMap = KtMap<KType, NavType>;

/// A field of a typed route (what Kotlin's serial descriptor tells of an element).
#[doc(hidden)]
pub struct RouteElement {
    /// Its argument's name (the field's, or its `#[route(name = …)]`).
    pub name: &'static str,
    /// Whether it has a default (Kotlin's `isElementOptional`): an `Option`, a `#[route(default)]`.
    pub optional: bool,
    /// Whether it may be null (an `Option`).
    pub nullable: bool,
    /// Its type.
    pub ktype: fn() -> KType,
    /// Its built-in `NavType` (`None`: the typeMap's only).
    pub nav_type: fn() -> Option<NavType>,
}

/// A typed route's description (Kotlin's serial descriptor): its serial name and fields.
#[doc(hidden)]
pub struct RouteDescriptor {
    pub serial_name: &'static str,
    pub elements: &'static [RouteElement],
}

/// A typed route (Compose's `@Serializable` class or object used as a route): `#[derive(Route)]` writes it (see the
/// module).
pub trait Route: 'static {
    #[doc(hidden)]
    fn descriptor() -> &'static RouteDescriptor
    where
        Self: Sized;
    /// Field `index`'s value as its `NavType`'s value (`None`: null).
    #[doc(hidden)]
    fn element_value(&self, index: usize) -> Option<Box<dyn Any>>;
    /// The value of the fields' values `value(index)` gives: `None` a field not there (its default then),
    /// `Some(None)` a null one.
    #[doc(hidden)]
    fn from_elements(value: &mut dyn FnMut(usize) -> Option<Option<Box<dyn Any>>>) -> Result<Self, String>
    where
        Self: Sized;
    /// Whether `other` is an equal value of this type (Kotlin's data class `equals`).
    #[doc(hidden)]
    fn route_eq(&self, other: &dyn Any) -> bool;
}

/// A typed route's value of any type (Kotlin's `Any` route object): what `navigate(Detail(7))`,
/// `popUpTo(Detail(7))` take.
pub trait AnyRoute: 'static {
    #[doc(hidden)]
    fn route_descriptor(&self) -> &'static RouteDescriptor;
    #[doc(hidden)]
    fn route_element_value(&self, index: usize) -> Option<Box<dyn Any>>;
    #[doc(hidden)]
    fn route_equals(&self, other: &dyn AnyRoute) -> bool;
    #[doc(hidden)]
    fn as_any(&self) -> &dyn Any;
    /// Its type's name.
    fn route_type_name(&self) -> &'static str;
}

impl<T: Route> AnyRoute for T {
    fn route_descriptor(&self) -> &'static RouteDescriptor {
        T::descriptor()
    }
    fn route_element_value(&self, index: usize) -> Option<Box<dyn Any>> {
        self.element_value(index)
    }
    fn route_equals(&self, other: &dyn AnyRoute) -> bool {
        self.route_eq(other.as_any())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn route_type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

impl fmt::Debug for dyn AnyRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.route_descriptor().serial_name)
    }
}

/// A typed route's type (Kotlin's `KClass<T>` of a route: `popUpTo<T>()`, `navigation<T>(startDestination =
/// Start::class)`): `RouteClass::of::<T>()`.
#[derive(Clone, Copy)]
pub struct RouteClass {
    descriptor: fn() -> &'static RouteDescriptor,
    type_id: fn() -> TypeId,
    name: fn() -> &'static str,
}

impl RouteClass {
    /// `T`'s (Kotlin's `T::class`).
    pub fn of<T: Route>() -> RouteClass {
        RouteClass { descriptor: T::descriptor, type_id: TypeId::of::<T>, name: std::any::type_name::<T> }
    }

    pub(crate) fn descriptor(&self) -> &'static RouteDescriptor {
        (self.descriptor)()
    }

    /// Its simple name (Kotlin's `simpleName`).
    pub fn simple_name(&self) -> &'static str {
        let name = (self.name)();
        name.rsplit("::").next().unwrap_or(name)
    }

    /// Its route's hash code: what its destination's id is (Kotlin's `serializer().generateHashCode()`).
    pub fn hash_code(&self) -> i32 {
        hash_code_of(self.descriptor())
    }
}

impl PartialEq for RouteClass {
    fn eq(&self, other: &Self) -> bool {
        (self.type_id)() == (other.type_id)()
    }
}

impl fmt::Debug for RouteClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "class {}", (self.name)())
    }
}

// ---- Field types ----

/// A type a typed route's field can be (Compose: a `@Serializable` type with a `NavType`). The built-in ones are
/// implemented here; an enum gets it from `#[derive(RouteValue)]`; a type of one's own implements it with the
/// defaults (`impl RouteValue for SearchParameters {}`) and gives its `NavType` in the routes' `typeMap`.
pub trait RouteValue: Clone + PartialEq + 'static {
    /// Whether a field of it may be null (an `Option`'s).
    const NULLABLE: bool = false;
    /// Its built-in `NavType` (Kotlin's `getNavType()` of its descriptor; `None`: the typeMap's only).
    fn nav_type() -> Option<NavType> {
        None
    }
    /// Its `Option`'s built-in `NavType`.
    fn nullable_nav_type() -> Option<NavType> {
        None
    }
    /// Its `Vec`'s (Kotlin's `List<T>`).
    fn list_nav_type() -> Option<NavType> {
        None
    }
    /// The `Vec`'s of its `Option`s (Kotlin's `List<T?>`).
    fn nullable_list_nav_type() -> Option<NavType> {
        None
    }
    /// Its `Box<[…]>`'s (Kotlin's arrays).
    fn array_nav_type() -> Option<NavType> {
        None
    }
    /// The `Box<[…]>`'s of its `Option`s.
    fn nullable_array_nav_type() -> Option<NavType> {
        None
    }
    /// It as its `NavType`'s value (`None`: null); by default itself.
    fn to_nav_value(&self) -> Option<Box<dyn Any>> {
        Some(Box::new(self.clone()))
    }
    /// It from its `NavType`'s value (`None` given: null; `None` returned: not a value of it).
    fn from_nav_value(value: Option<Box<dyn Any>>) -> Option<Self> {
        value?.downcast::<Self>().ok().map(|b| *b)
    }
}

macro_rules! route_values {
    ($($t:ty => $one:ident, $nullable:ident, $list:ident, $array:ident;)*) => {$(
        impl RouteValue for $t {
            fn nav_type() -> Option<NavType> {
                Some(NavType::$one)
            }
            fn nullable_nav_type() -> Option<NavType> {
                Some(NavType::$nullable)
            }
            fn list_nav_type() -> Option<NavType> {
                Some(NavType::$list)
            }
            fn array_nav_type() -> Option<NavType> {
                Some(NavType::$array)
            }
        }
    )*};
}

route_values! {
    i32 => IntType, IntNullableType, IntListType, IntArrayType;
    i64 => LongType, LongNullableType, LongListType, LongArrayType;
    f32 => FloatType, FloatNullableType, FloatListType, FloatArrayType;
    f64 => DoubleType, DoubleNullableType, DoubleListType, DoubleArrayType;
    bool => BoolType, BoolNullableType, BoolListType, BoolArrayType;
}

impl RouteValue for String {
    fn nav_type() -> Option<NavType> {
        Some(NavType::StringNonNullableType)
    }
    fn nullable_nav_type() -> Option<NavType> {
        Some(NavType::StringType)
    }
    fn list_nav_type() -> Option<NavType> {
        Some(NavType::StringListType)
    }
    fn nullable_list_nav_type() -> Option<NavType> {
        Some(NavType::StringNullableListType)
    }
    fn array_nav_type() -> Option<NavType> {
        Some(NavType::StringArrayType)
    }
    fn nullable_array_nav_type() -> Option<NavType> {
        Some(NavType::StringNullableArrayType)
    }
}

/// Kotlin's `Byte`, `Short` and `Char`: no built-in `NavType` (the typeMap's).
impl RouteValue for i8 {}
impl RouteValue for i16 {}
impl RouteValue for char {}
/// java.util's `UUID` (a `@Serializable(with = UUIDSerializer::class)` property of NavigationSamples' WrappedUUID):
/// none either.
impl RouteValue for kompoz_runtime::kt::UUID {}

impl<T: RouteValue> RouteValue for Option<T> {
    const NULLABLE: bool = true;
    fn nav_type() -> Option<NavType> {
        T::nullable_nav_type()
    }
    fn list_nav_type() -> Option<NavType> {
        T::nullable_list_nav_type()
    }
    fn array_nav_type() -> Option<NavType> {
        T::nullable_array_nav_type()
    }
    fn to_nav_value(&self) -> Option<Box<dyn Any>> {
        self.as_ref().and_then(T::to_nav_value)
    }
    fn from_nav_value(value: Option<Box<dyn Any>>) -> Option<Self> {
        match value {
            None => Some(None),
            Some(v) => T::from_nav_value(Some(v)).map(Some),
        }
    }
}

impl<T: RouteValue> RouteValue for Vec<T> {
    fn nav_type() -> Option<NavType> {
        T::list_nav_type()
    }
    fn nullable_nav_type() -> Option<NavType> {
        T::list_nav_type()
    }
}

impl<T: RouteValue> RouteValue for Box<[T]> {
    fn nav_type() -> Option<NavType> {
        T::array_nav_type()
    }
    fn nullable_nav_type() -> Option<NavType> {
        T::array_nav_type()
    }
}

/// An enum's `NavType`s (what `#[derive(RouteValue)]` gives an enum).
#[doc(hidden)]
pub fn enum_route_nav_types<E: NavEnum>() -> (NavType, NavType, NavType) {
    (enum_nav_type::<E>(), enum_nullable_nav_type::<E>(), enum_list_nav_type::<E>())
}

// ---- Patterns, arguments, hash codes ----

fn unknown_nav_type_error(field: &str, field_type: &str, class_name: &str, type_map: &TypeMap) -> String {
    format!(
        "Route {class_name} could not find any NavType for argument {field} of type {field_type} - typeMap \
         received was {type_map:?}"
    )
}

/// The `NavType` of `element`: the typeMap's for its type, else its built-in one (Kotlin's `computeNavType`).
fn compute_nav_type(element: &RouteElement, type_map: &TypeMap) -> Option<NavType> {
    let ktype = (element.ktype)();
    type_map.iter().find(|(k, _)| **k == ktype).map(|(_, t)| t.clone()).or_else(|| (element.nav_type)())
}

fn nav_type_of(descriptor: &RouteDescriptor, element: &RouteElement, type_map: &TypeMap) -> NavType {
    compute_nav_type(element, type_map).unwrap_or_else(|| {
        panic!("{}", unknown_nav_type_error(element.name, (element.ktype)().name(), descriptor.serial_name, type_map))
    })
}

/// Where an argument goes: a collection's or one with a default in the query, the others in the path.
fn is_query(element: &RouteElement, type_: &NavType) -> bool {
    type_.is_collection() || element.optional
}

/// A route being built (Kotlin's `RouteBuilder`): its path, path arguments and query.
struct RouteBuilder {
    path: String,
    path_args: String,
    query_args: String,
}

impl RouteBuilder {
    fn new(path: &str) -> RouteBuilder {
        RouteBuilder { path: path.to_string(), path_args: String::new(), query_args: String::new() }
    }
    fn add_path(&mut self, segment: &str) {
        self.path_args.push('/');
        self.path_args.push_str(segment);
    }
    fn add_query(&mut self, name: &str, value: &str) {
        self.query_args.push(if self.query_args.is_empty() { '?' } else { '&' });
        self.query_args.push_str(name);
        self.query_args.push('=');
        self.query_args.push_str(value);
    }
    fn build(self) -> String {
        self.path + &self.path_args + &self.query_args
    }
}

pub(crate) fn route_pattern_of(descriptor: &RouteDescriptor, type_map: &TypeMap, path: Option<&str>) -> String {
    let mut builder = RouteBuilder::new(path.unwrap_or(descriptor.serial_name));
    for element in descriptor.elements {
        let type_ = nav_type_of(descriptor, element, type_map);
        let placeholder = format!("{{{}}}", element.name);
        if is_query(element, &type_) {
            builder.add_query(element.name, &placeholder);
        } else {
            builder.add_path(&placeholder);
        }
    }
    builder.build()
}

/// `T`'s route pattern (Kotlin's `generateRoutePattern`): `Detail/{id}?tab={tab}`; `path` in place of its name
/// (a deep link's base path).
///
/// # Panics
/// A field whose type has no `NavType` (built in or in `type_map`).
pub fn generate_route_pattern<T: Route>(type_map: &TypeMap, path: Option<&str>) -> String {
    route_pattern_of(T::descriptor(), type_map, path)
}

pub(crate) fn nav_arguments_of(descriptor: &RouteDescriptor, type_map: &TypeMap) -> Vec<NamedNavArgument> {
    descriptor
        .elements
        .iter()
        .map(|element| {
            let mut b = NavArgumentBuilder::new();
            b.set_type(nav_type_of(descriptor, element, type_map));
            b.set_nullable(element.nullable);
            if element.optional {
                b.set_unknown_default_value_present(true);
            }
            NamedNavArgument::new(element.name, b.build())
        })
        .collect()
}

/// `T`'s arguments (Kotlin's `generateNavArguments`): each field's, of its `NavType`, nullable as it is, with a
/// default (not known here) when it has one.
pub fn generate_nav_arguments<T: Route>(type_map: &TypeMap) -> Vec<NamedNavArgument> {
    nav_arguments_of(T::descriptor(), type_map)
}

pub(crate) fn hash_code_of(descriptor: &RouteDescriptor) -> i32 {
    descriptor
        .elements
        .iter()
        .fold(string_hash(descriptor.serial_name), |h, e| h.wrapping_mul(31).wrapping_add(string_hash(e.name)))
}

/// `T`'s hash code: its serial name's, then each argument name's (Kotlin's `generateHashCode`); its destination's
/// id.
pub fn generate_hash_code<T: Route>() -> i32 {
    hash_code_of(T::descriptor())
}

/// `route`'s route: its pattern with its values (Kotlin's `generateRouteWithArgs`), each written by `type_map`'s
/// `NavType` of its name (a destination's arguments' types).
///
/// # Panics
/// A field with no `NavType` in `type_map`.
pub fn generate_route_with_args(route: &dyn AnyRoute, type_map: &[(String, NavType)]) -> String {
    let descriptor = route.route_descriptor();
    let type_of_arg = |name: &str| {
        type_map
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())
            .unwrap_or_else(|| panic!("Cannot locate NavType for argument [{name}]"))
    };
    let mut builder = RouteBuilder::new(descriptor.serial_name);
    for (index, element) in descriptor.elements.iter().enumerate() {
        let type_ = type_of_arg(element.name);
        // (Kotlin's RouteEncoder: a collection's values each, another's one)
        let value = route.route_element_value(index);
        let values = match type_.erased().serialize_values_any(value.as_deref()) {
            Some(values) => values,
            None => vec![type_.erased().serialize_any(value.as_deref())],
        };
        if is_query(element, &type_) {
            for v in &values {
                builder.add_query(element.name, v);
            }
        } else {
            assert!(
                values.len() == 1,
                "Expected one value for argument {}, found {}values instead.",
                element.name,
                values.len()
            );
            builder.add_path(&values[0]);
        }
    }
    builder.build()
}

/// A `T` from `bundle`'s arguments (Kotlin's `decodeArguments`): each field's value read by `type_map`'s
/// `NavType` of its name; a field not there takes its default.
///
/// # Panics
/// A field without a default not there, or null where it may not be (Kotlin's serialization exceptions).
pub fn decode_arguments<T: Route>(bundle: &SavedState, type_map: &[(String, NavType)]) -> T {
    let descriptor = T::descriptor();
    let decoded = T::from_elements(&mut |index| {
        let name = descriptor.elements[index].name;
        if !contains(bundle, name) {
            return None;
        }
        let type_ = type_map.iter().find(|(n, _)| n == name).map(|(_, t)| t.clone());
        Some(type_.and_then(|t| t.erased().get_any(bundle, name)))
    });
    decoded.unwrap_or_else(|e| panic!("{e}"))
}

#[doc(hidden)]
pub fn missing_field(field: &str, serial_name: &str) -> String {
    format!("Field '{field}' is required for type with serial name '{serial_name}', but it was missing")
}

#[doc(hidden)]
pub fn unexpected_null(field: &str) -> String {
    format!("Unexpected null value for non-nullable argument {field}")
}

/// A typed route of a [`SavedStateHandle`]'s values (navigation's `SavedStateHandle.toRoute<T>(typeMap)`: a
/// ViewModel's handle holds its entry's arguments).
pub trait SavedStateHandleRoutes {
    /// Its values as a `T`, read by `T`'s fields' `NavType`s (`type_map`'s for types of one's own).
    ///
    /// Kotlin: `toRoute<T>(typeMap: Map<KType, NavType<*>> = emptyMap())`
    ///
    /// # Panics
    /// If they aren't a `T`'s.
    fn to_route<T: Route>(&self, type_map: TypeMap) -> T;
}

impl SavedStateHandleRoutes for kompoz_lifecycle::SavedStateHandle {
    fn to_route<T: Route>(&self, type_map: TypeMap) -> T {
        let descriptor = T::descriptor();
        let types: Vec<(String, NavType)> =
            nav_arguments_of(descriptor, &type_map).iter().map(|a| (a.name(), a.argument().r#type())).collect();
        let decoded = T::from_elements(&mut |index| {
            let name = descriptor.elements[index].name;
            if !self.contains(name) {
                return None;
            }
            let value: kompoz_runtime::SavedValue = self.get(name).unwrap_or_default();
            let state = crate::saved_state::saved_state(|w| w.put_value(name, value));
            let type_ = types.iter().find(|(n, _)| n == name).map(|(_, t)| t.clone());
            Some(type_.and_then(|t| t.erased().get_any(&state, name)))
        });
        decoded.unwrap_or_else(|e| panic!("{e}"))
    }
}

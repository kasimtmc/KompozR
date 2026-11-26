//! `NavType`s: how an argument of a type is put in a saved state, read back, parsed from a route's or link's text and
//! written into one (Compose's `NavType<T>`, `CollectionNavType<T>`, the built-in types and the internal ones typed
//! routes use).
//!
//! A [`NavType`] is a cheap handle (Kotlin's `NavType<*>` object): the built-ins are its constants
//! (`NavType::IntType`, Kotlin's `NavType.IntType`), a type of one's own implements [`NavTypeImpl`] (Kotlin's
//! `object : NavType<T>(isNullableAllowed) { … }`) and is made one with [`NavType::new`]; a collection's implements
//! [`CollectionNavType`] too (its values are a query parameter's repeated values). Values are Kotlin's: `None` is
//! `null` — of a type that allows it ([`NavTypeImpl::is_nullable_allowed`]).

use crate::java;
use crate::saved_state::{SavedState, contains, is_null, put};
use crate::uri;
use kompoz_runtime::SavedValue;
use std::any::{Any, TypeId};
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

/// A value that couldn't be parsed or isn't of a type (Kotlin's `IllegalArgumentException`, the
/// `NumberFormatException` of a number's text, the `UnsupportedOperationException` of a type with no text form).
#[derive(Clone, Debug, PartialEq)]
pub struct NavTypeError(pub String);

impl fmt::Display for NavTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NavTypeError {}

/// What a `NavType` of one's own implements (Kotlin's abstract `NavType<T>`): its values are `Value`s, `None`
/// Kotlin's `null` (of a type that [allows](Self::is_nullable_allowed) it). The Kotlin syntax's `object :
/// NavType<T>(isNullableAllowed = false) { override fun put(…) … }` is a [`NavType`] of an implementation of it.
///
/// Kotlin: `abstract class NavType<T>(isNullableAllowed: Boolean)`
pub trait NavTypeImpl: 'static {
    /// Kotlin's `T`.
    type Value: Clone + PartialEq + 'static;

    /// Whether its arguments may be null (Kotlin's `isNullableAllowed`).
    fn is_nullable_allowed(&self) -> bool;

    /// Its name (Kotlin's `name`: `"integer"`, `"string"`, a class's name).
    fn name(&self) -> String {
        "nav_type".into()
    }

    /// Puts `value` under `key` (Kotlin's `put(bundle, key, value)`).
    fn put(&self, bundle: &SavedState, key: &str, value: Option<&Self::Value>);

    /// The value under `key` (Kotlin's `get(bundle, key)`; `None`: null, or not there).
    fn get(&self, bundle: &SavedState, key: &str) -> Option<Self::Value>;

    /// The value `value`'s text is (Kotlin's `parseValue(value)`).
    fn parse_value(&self, value: &str) -> Result<Option<Self::Value>, NavTypeError>;

    /// The value of `value`'s text given the one before it (a repeated query parameter: a collection adds it to
    /// `previous`); by default as [`parse_value`](Self::parse_value).
    fn parse_value_with(
        &self,
        value: &str,
        previous: Option<&Self::Value>,
    ) -> Result<Option<Self::Value>, NavTypeError> {
        let _ = previous;
        self.parse_value(value)
    }

    /// `value` as a route's text (Kotlin's `serializeAsValue`; Kotlin's default is `toString()`).
    fn serialize_as_value(&self, value: Option<&Self::Value>) -> String;

    /// Whether two values are the same (Kotlin's `valueEquals`; by default `==`).
    fn value_equals(&self, value: Option<&Self::Value>, other: Option<&Self::Value>) -> bool {
        value == other
    }

    /// It as a collection type, if it is one (Kotlin's `is CollectionNavType`).
    fn as_collection(&self) -> Option<&dyn CollectionNavType<Value = Self::Value>> {
        None
    }
}

/// A `NavType` of collections (Kotlin's `CollectionNavType<T>`): a typed route's argument of it is a query
/// parameter, repeated for each element; its [`as_collection`](NavTypeImpl::as_collection) gives itself.
pub trait CollectionNavType: NavTypeImpl {
    /// Each element's text (Kotlin's `serializeAsValues`).
    fn serialize_as_values(&self, value: Option<&Self::Value>) -> Vec<String>;
    /// An empty collection (Kotlin's `emptyCollection`): the value of a parameter given no values.
    fn empty_collection(&self) -> Self::Value;
}

/// A `NavTypeImpl` without its value type (what a `NavType` holds): values as `dyn Any`.
pub(crate) trait ErasedNavType {
    fn name(&self) -> String;
    fn is_nullable_allowed(&self) -> bool;
    fn put_any(&self, bundle: &SavedState, key: &str, value: Option<&dyn Any>);
    fn get_any(&self, bundle: &SavedState, key: &str) -> Option<Box<dyn Any>>;
    fn parse_any(&self, value: &str) -> Result<Option<Box<dyn Any>>, NavTypeError>;
    fn parse_and_put(&self, bundle: &SavedState, key: &str, value: &str) -> Result<(), NavTypeError>;
    fn parse_and_put_repeated(&self, bundle: &SavedState, key: &str, value: &str) -> Result<(), NavTypeError>;
    fn serialize_any(&self, value: Option<&dyn Any>) -> String;
    fn serialize_values_any(&self, value: Option<&dyn Any>) -> Option<Vec<String>>;
    fn is_collection(&self) -> bool;
    fn put_empty_collection(&self, bundle: &SavedState, key: &str);
    fn values_equal_in(&self, a: &SavedState, b: &SavedState, key: &str) -> bool;
    fn value_equals_any(&self, a: Option<&dyn Any>, b: Option<&dyn Any>) -> bool;
    fn value_type(&self) -> (TypeId, &'static str);
    fn impl_type(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
}

fn typed<'a, V: 'static>(value: Option<&'a dyn Any>, name: &str) -> Option<&'a V> {
    value.map(|v| {
        v.downcast_ref::<V>().unwrap_or_else(|| {
            panic!(
                "a value of {} given to the NavType {name}, which takes {}",
                type_name_of(v),
                std::any::type_name::<V>()
            )
        })
    })
}

fn type_name_of(_: &dyn Any) -> &'static str {
    "another type"
}

impl<N: NavTypeImpl> ErasedNavType for N {
    fn name(&self) -> String {
        NavTypeImpl::name(self)
    }
    fn is_nullable_allowed(&self) -> bool {
        NavTypeImpl::is_nullable_allowed(self)
    }
    fn put_any(&self, bundle: &SavedState, key: &str, value: Option<&dyn Any>) {
        self.put(bundle, key, typed::<N::Value>(value, &NavTypeImpl::name(self)));
    }
    fn get_any(&self, bundle: &SavedState, key: &str) -> Option<Box<dyn Any>> {
        self.get(bundle, key).map(|v| Box::new(v) as Box<dyn Any>)
    }
    fn parse_any(&self, value: &str) -> Result<Option<Box<dyn Any>>, NavTypeError> {
        Ok(self.parse_value(value)?.map(|v| Box::new(v) as Box<dyn Any>))
    }
    fn parse_and_put(&self, bundle: &SavedState, key: &str, value: &str) -> Result<(), NavTypeError> {
        let parsed = self.parse_value(value)?;
        self.put(bundle, key, parsed.as_ref());
        Ok(())
    }
    fn parse_and_put_repeated(&self, bundle: &SavedState, key: &str, value: &str) -> Result<(), NavTypeError> {
        if !contains(bundle, key) {
            return Err(NavTypeError("There is no previous value in this savedState.".into()));
        }
        let previous = self.get(bundle, key);
        let parsed = self.parse_value_with(value, previous.as_ref())?;
        self.put(bundle, key, parsed.as_ref());
        Ok(())
    }
    fn serialize_any(&self, value: Option<&dyn Any>) -> String {
        self.serialize_as_value(typed::<N::Value>(value, &NavTypeImpl::name(self)))
    }
    fn serialize_values_any(&self, value: Option<&dyn Any>) -> Option<Vec<String>> {
        let collection = self.as_collection()?;
        Some(collection.serialize_as_values(typed::<N::Value>(value, &NavTypeImpl::name(self))))
    }
    fn is_collection(&self) -> bool {
        self.as_collection().is_some()
    }
    fn put_empty_collection(&self, bundle: &SavedState, key: &str) {
        if let Some(c) = self.as_collection() {
            let empty = c.empty_collection();
            self.put(bundle, key, Some(&empty));
        }
    }
    fn values_equal_in(&self, a: &SavedState, b: &SavedState, key: &str) -> bool {
        self.value_equals(self.get(a, key).as_ref(), self.get(b, key).as_ref())
    }
    fn value_equals_any(&self, a: Option<&dyn Any>, b: Option<&dyn Any>) -> bool {
        let name = NavTypeImpl::name(self);
        self.value_equals(typed::<N::Value>(a, &name), typed::<N::Value>(b, &name))
    }
    fn value_type(&self) -> (TypeId, &'static str) {
        (TypeId::of::<N::Value>(), std::any::type_name::<N::Value>())
    }
    fn impl_type(&self) -> TypeId {
        TypeId::of::<N>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone)]
enum Repr {
    Static(&'static dyn ErasedNavType),
    Shared(Rc<dyn ErasedNavType>),
}

/// A navigation argument's type (Kotlin's `NavType<*>`): see the module. Cheap to clone; two are equal when they
/// are of the same implementation and name (the built-ins are each one object, as Kotlin's).
#[derive(Clone)]
pub struct NavType(Repr);

impl PartialEq for NavType {
    fn eq(&self, other: &Self) -> bool {
        self.erased().impl_type() == other.erased().impl_type() && self.name() == other.name()
    }
}

impl fmt::Debug for NavType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

impl fmt::Display for NavType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

#[allow(non_upper_case_globals)]
impl NavType {
    /// Integers (Kotlin's `Int`): `"42"`, `"0x2A"`.
    pub const IntType: NavType = NavType(Repr::Static(&IntNavType));
    /// Arrays of integers (Kotlin's `IntArray?`), `Box<[i32]>`.
    pub const IntArrayType: NavType = NavType(Repr::Static(&IntArrayNavType));
    /// Lists of integers (Kotlin's `List<Int>?`), `Vec<i32>`.
    pub const IntListType: NavType = NavType(Repr::Static(&IntListNavType));
    /// Longs (Kotlin's `Long`): `"42"`, `"42L"`, `"0x2A"`.
    pub const LongType: NavType = NavType(Repr::Static(&LongNavType));
    pub const LongArrayType: NavType = NavType(Repr::Static(&LongArrayNavType));
    pub const LongListType: NavType = NavType(Repr::Static(&LongListNavType));
    /// Floats (Kotlin's `Float`).
    pub const FloatType: NavType = NavType(Repr::Static(&FloatNavType));
    pub const FloatArrayType: NavType = NavType(Repr::Static(&FloatArrayNavType));
    pub const FloatListType: NavType = NavType(Repr::Static(&FloatListNavType));
    /// Booleans: `"true"`, `"false"`.
    pub const BoolType: NavType = NavType(Repr::Static(&BoolNavType));
    pub const BoolArrayType: NavType = NavType(Repr::Static(&BoolArrayNavType));
    pub const BoolListType: NavType = NavType(Repr::Static(&BoolListNavType));
    /// Strings, nullable (Kotlin's `String?`: `"null"` is null), written encoded.
    pub const StringType: NavType = NavType(Repr::Static(&StringNavType));
    pub const StringArrayType: NavType = NavType(Repr::Static(&StringArrayNavType));
    pub const StringListType: NavType = NavType(Repr::Static(&StringListNavType));

    // Typed routes' own (Kotlin's internal `InternalNavType`).
    pub(crate) const IntNullableType: NavType = NavType(Repr::Static(&IntNullableNavType));
    pub(crate) const BoolNullableType: NavType = NavType(Repr::Static(&BoolNullableNavType));
    pub(crate) const DoubleType: NavType = NavType(Repr::Static(&DoubleNavType));
    pub(crate) const DoubleNullableType: NavType = NavType(Repr::Static(&DoubleNullableNavType));
    pub(crate) const FloatNullableType: NavType = NavType(Repr::Static(&FloatNullableNavType));
    pub(crate) const LongNullableType: NavType = NavType(Repr::Static(&LongNullableNavType));
    pub(crate) const StringNonNullableType: NavType = NavType(Repr::Static(&StringNonNullableNavType));
    pub(crate) const StringNullableArrayType: NavType = NavType(Repr::Static(&StringNullableArrayNavType));
    pub(crate) const StringNullableListType: NavType = NavType(Repr::Static(&StringNullableListNavType));
    pub(crate) const DoubleArrayType: NavType = NavType(Repr::Static(&DoubleArrayNavType));
    pub(crate) const DoubleListType: NavType = NavType(Repr::Static(&DoubleListNavType));

    /// A `NavType` of one's own (Kotlin's `object : NavType<T>(…) { … }`).
    pub fn new(imp: impl NavTypeImpl) -> NavType {
        NavType(Repr::Shared(Rc::new(imp)))
    }

    /// The `NavType` of a static one of one's own (Kotlin's `object MyType : NavType<T>(…) { … }`, named as a
    /// constant: `const MyType: NavType = NavType::of_object(&MyTypeImpl);`).
    pub const fn of_object<N: NavTypeImpl>(imp: &'static N) -> NavType {
        NavType(Repr::Static(imp))
    }

    /// An enum's (Kotlin's `NavType.EnumType(Class)`): its entries by name, any case.
    #[allow(non_snake_case)]
    pub fn EnumType<E: NavEnum>() -> NavType {
        NavType::new(EnumNavType::<E>(PhantomData))
    }

    pub(crate) fn erased(&self) -> &dyn ErasedNavType {
        match &self.0 {
            Repr::Static(s) => *s,
            Repr::Shared(r) => &**r,
        }
    }

    /// Its name (Kotlin's `name`).
    pub fn name(&self) -> String {
        self.erased().name()
    }

    /// Whether its arguments may be null (Kotlin's `isNullableAllowed`).
    pub fn is_nullable_allowed(&self) -> bool {
        self.erased().is_nullable_allowed()
    }

    /// Whether it is a [`CollectionNavType`].
    pub fn is_collection(&self) -> bool {
        self.erased().is_collection()
    }

    /// Its implementation, if it is an `N` (Kotlin's `type as? MyNavType`).
    pub fn downcast_ref<N: NavTypeImpl>(&self) -> Option<&N> {
        self.erased().as_any().downcast_ref::<N>()
    }

    /// Puts `value` (a value of its `Value` type, or `None`: null) under `key`.
    ///
    /// # Panics
    /// If `T` isn't its value type.
    pub fn put<T: 'static>(&self, bundle: &SavedState, key: &str, value: Option<&T>) {
        self.erased().put_any(bundle, key, value.map(|v| v as &dyn Any));
    }

    /// The value under `key` as a `T` (`None`: null, not there, or not a `T`).
    pub fn get<T: 'static>(&self, bundle: &SavedState, key: &str) -> Option<T> {
        self.erased().get_any(bundle, key).and_then(|b| b.downcast::<T>().ok()).map(|b| *b)
    }

    /// `value`'s text parsed (Kotlin's `parseValue`), as a `T`.
    ///
    /// # Panics
    /// If `T` isn't its value type.
    pub fn parse_value<T: 'static>(&self, value: &str) -> Result<Option<T>, NavTypeError> {
        Ok(self.erased().parse_any(value)?.map(|b| {
            *b.downcast::<T>()
                .unwrap_or_else(|_| panic!("the NavType {} doesn't parse {}", self.name(), std::any::type_name::<T>()))
        }))
    }

    /// Parses `value` and puts it under `key` (Kotlin's `parseAndPut`).
    pub fn parse_and_put(&self, bundle: &SavedState, key: &str, value: &str) -> Result<(), NavTypeError> {
        self.erased().parse_and_put(bundle, key, value)
    }

    /// `value` as a route's text (Kotlin's `serializeAsValue`).
    ///
    /// # Panics
    /// If `T` isn't its value type.
    pub fn serialize_as_value<T: 'static>(&self, value: Option<&T>) -> String {
        self.erased().serialize_any(value.map(|v| v as &dyn Any))
    }

    /// A collection's elements' texts (Kotlin's `serializeAsValues`); `None` if it isn't a collection type.
    pub fn serialize_as_values<T: 'static>(&self, value: Option<&T>) -> Option<Vec<String>> {
        self.erased().serialize_values_any(value.map(|v| v as &dyn Any))
    }

    /// Whether two values are the same for it (Kotlin's `valueEquals`).
    pub fn value_equals<T: 'static>(&self, value: Option<&T>, other: Option<&T>) -> bool {
        self.erased().value_equals_any(value.map(|v| v as &dyn Any), other.map(|v| v as &dyn Any))
    }

    /// The built-in type of the name `type_` (an argument's `app:argType`: `"integer"`, `"string[]"`,
    /// `"List<Long>"` …), strings for none (Kotlin's `fromArgType`; class names are Android's).
    ///
    /// # Panics
    /// If `type_` names no built-in type.
    pub fn from_arg_type(type_: Option<&str>, package_name: Option<&str>) -> NavType {
        let _ = package_name;
        match type_ {
            None | Some("") => NavType::StringType,
            Some(t) => nav_type_from_arg_type(t).unwrap_or_else(|| panic!("the NavType {t} isn't known here")),
        }
    }

    /// The type a default value's text is of: an integer, a long, a float, a boolean, else a string (Kotlin's
    /// `inferFromValue`).
    pub fn infer_from_value(value: &str) -> NavType {
        if IntNavType.parse_value(value).is_ok() {
            NavType::IntType
        } else if LongNavType.parse_value(value).is_ok() {
            NavType::LongType
        } else if FloatNavType.parse_value(value).is_ok() {
            NavType::FloatType
        } else if BoolNavType.parse_value(value).is_ok() {
            NavType::BoolType
        } else {
            NavType::StringType
        }
    }

    /// The type of a value (Kotlin's `inferFromValueType`): integers, longs, floats, booleans, their arrays,
    /// strings and their arrays; strings for none.
    ///
    /// # Panics
    /// For a value of another type.
    pub fn infer_from_value_type(value: Option<&dyn Any>) -> NavType {
        let Some(v) = value else { return NavType::StringType };
        let candidates = [
            NavType::IntType,
            NavType::IntArrayType,
            NavType::LongType,
            NavType::LongArrayType,
            NavType::FloatType,
            NavType::FloatArrayType,
            NavType::BoolType,
            NavType::BoolArrayType,
            NavType::StringType,
            NavType::StringArrayType,
        ];
        candidates
            .into_iter()
            .find(|t| t.erased().value_type().0 == v.type_id())
            .unwrap_or_else(|| panic!("Object of this type is not supported for navigation arguments."))
    }
}

fn nav_type_from_arg_type(t: &str) -> Option<NavType> {
    [
        NavType::IntType,
        NavType::IntArrayType,
        NavType::IntListType,
        NavType::LongType,
        NavType::LongArrayType,
        NavType::LongListType,
        NavType::BoolType,
        NavType::BoolArrayType,
        NavType::BoolListType,
        NavType::StringType,
        NavType::StringArrayType,
        NavType::StringListType,
        NavType::FloatType,
        NavType::FloatArrayType,
        NavType::FloatListType,
    ]
    .into_iter()
    .find(|n| n.name() == t)
}

// ---- Parsing as Kotlin's (Java's) ----

fn number_error(value: &str) -> NavTypeError {
    NavTypeError(format!("For input string: \"{value}\""))
}

/// Kotlin's `String.toInt()` (Java's `Integer.parseInt`): a sign and decimal digits.
fn parse_int(value: &str, radix: u32) -> Result<i32, NavTypeError> {
    if value.is_empty() || value.trim() != value {
        return Err(number_error(value));
    }
    i32::from_str_radix(value, radix).map_err(|_| number_error(value))
}

fn parse_long(value: &str, radix: u32) -> Result<i64, NavTypeError> {
    if value.is_empty() || value.trim() != value {
        return Err(number_error(value));
    }
    i64::from_str_radix(value, radix).map_err(|_| number_error(value))
}

/// Java's `Double.parseDouble`: around it whitespace, a trailing `f`/`F`/`d`/`D`, `NaN` and `Infinity`.
pub(crate) fn parse_java_double(value: &str) -> Result<f64, NavTypeError> {
    let t = value.trim_matches(|c: char| c <= ' ');
    let (sign, body) = match t.strip_prefix('-') {
        Some(b) => (-1.0, b),
        None => (1.0, t.strip_prefix('+').unwrap_or(t)),
    };
    match body {
        "NaN" => return Ok(f64::NAN),
        "Infinity" => return Ok(sign * f64::INFINITY),
        _ => {}
    }
    let body = body.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(body);
    let ok = !body.is_empty()
        && body.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
        && body.chars().any(|c| c.is_ascii_digit());
    if !ok {
        return Err(number_error(value));
    }
    body.parse::<f64>().map(|d| sign * d).map_err(|_| number_error(value))
}

fn parse_java_float(value: &str) -> Result<f32, NavTypeError> {
    // (parsed as a float, as Java's Float.parseFloat rounds once)
    let t = value.trim_matches(|c: char| c <= ' ');
    parse_java_double(value)?;
    let body = t.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(t);
    match body.trim_start_matches(['+', '-']) {
        "NaN" => Ok(f32::NAN),
        "Infinity" => Ok(if body.starts_with('-') { f32::NEG_INFINITY } else { f32::INFINITY }),
        _ => body.parse::<f32>().map_err(|_| number_error(value)),
    }
}

fn parse_bool(value: &str) -> Result<bool, NavTypeError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(NavTypeError("A boolean NavType only accepts \"true\" or \"false\" values.".into())),
    }
}

fn int_value(value: &str) -> Result<i32, NavTypeError> {
    match value.strip_prefix("0x") {
        Some(hex) => parse_int(hex, 16),
        None => parse_int(value, 10),
    }
}

fn long_value(value: &str) -> Result<i64, NavTypeError> {
    let local = value.strip_suffix('L').unwrap_or(value);
    if value.starts_with("0x") { parse_long(&local[2..], 16) } else { parse_long(local, 10) }
}

fn null_or<T>(value: &str, parse: impl FnOnce(&str) -> Result<T, NavTypeError>) -> Result<Option<T>, NavTypeError> {
    if value == "null" { Ok(None) } else { parse(value).map(Some) }
}

// ---- Putting and reading values ----

fn put_or_null(bundle: &SavedState, key: &str, value: Option<SavedValue>) {
    put(bundle, key, value.unwrap_or(SavedValue::Null));
}

fn read<T>(
    bundle: &SavedState,
    key: &str,
    f: impl FnOnce(&crate::saved_state::SavedStateReader, &str) -> Option<T>,
) -> Option<T> {
    if !contains(bundle, key) || is_null(bundle, key) { None } else { bundle.read(|r| f(r, key)) }
}

fn list_value<T: Clone>(values: &[T], one: impl Fn(T) -> SavedValue) -> SavedValue {
    SavedValue::list(values.iter().cloned().map(one))
}

fn plus<T: Clone>(previous: Option<&[T]>, more: Vec<T>) -> Vec<T> {
    let mut all = previous.map(<[T]>::to_vec).unwrap_or_default();
    all.extend(more);
    all
}

// ---- The built-in types ----

macro_rules! scalar_type {
    ($ty:ident, $value:ty, $name:literal, $nullable:literal, $put:expr, $get:ident, $parse:expr, $serialize:expr) => {
        #[derive(Clone, Copy, Debug, Default)]
        pub(crate) struct $ty;
        impl NavTypeImpl for $ty {
            type Value = $value;
            fn is_nullable_allowed(&self) -> bool {
                $nullable
            }
            fn name(&self) -> String {
                $name.into()
            }
            fn put(&self, bundle: &SavedState, key: &str, value: Option<&$value>) {
                let to: fn(&$value) -> SavedValue = $put;
                put_or_null(bundle, key, value.map(to));
            }
            fn get(&self, bundle: &SavedState, key: &str) -> Option<$value> {
                read(bundle, key, |b, k| b.$get(k))
            }
            fn parse_value(&self, value: &str) -> Result<Option<$value>, NavTypeError> {
                let parse: fn(&str) -> Result<Option<$value>, NavTypeError> = $parse;
                parse(value)
            }
            fn serialize_as_value(&self, value: Option<&$value>) -> String {
                let s: fn(&$value) -> String = $serialize;
                value.map_or_else(|| "null".to_string(), s)
            }
        }
    };
}

scalar_type!(
    IntNavType,
    i32,
    "integer",
    false,
    |v| SavedValue::Int(*v as i64),
    get_int_or_null,
    |v| int_value(v).map(Some),
    |v| v.to_string()
);
scalar_type!(
    IntNullableNavType,
    i32,
    "integer_nullable",
    true,
    |v| SavedValue::Int(*v as i64),
    get_int_or_null,
    |v| null_or(v, int_value),
    |v| v.to_string()
);
scalar_type!(
    LongNavType,
    i64,
    "long",
    false,
    |v| SavedValue::Int(*v),
    get_long_or_null,
    |v| long_value(v).map(Some),
    |v| v.to_string()
);
scalar_type!(
    LongNullableNavType,
    i64,
    "long_nullable",
    true,
    |v| SavedValue::Int(*v),
    get_long_or_null,
    |v| null_or(v, long_value),
    |v| v.to_string()
);
scalar_type!(
    FloatNavType,
    f32,
    "float",
    false,
    |v| SavedValue::Double(*v as f64),
    get_float_or_null,
    |v| parse_java_float(v).map(Some),
    |v| java::float_to_string(*v)
);
scalar_type!(
    FloatNullableNavType,
    f32,
    "float_nullable",
    true,
    |v| SavedValue::Double(*v as f64),
    get_float_or_null,
    |v| null_or(v, parse_java_float),
    |v| java::float_to_string(*v)
);
scalar_type!(
    DoubleNavType,
    f64,
    "double",
    false,
    |v| SavedValue::Double(*v),
    get_double_or_null,
    |v| parse_java_double(v).map(Some),
    |v| java::double_to_string(*v)
);
scalar_type!(
    DoubleNullableNavType,
    f64,
    "double_nullable",
    true,
    |v| SavedValue::Double(*v),
    get_double_or_null,
    |v| null_or(v, parse_java_double),
    |v| java::double_to_string(*v)
);
scalar_type!(
    BoolNavType,
    bool,
    "boolean",
    false,
    |v| SavedValue::Bool(*v),
    get_boolean_or_null,
    |v| parse_bool(v).map(Some),
    |v| v.to_string()
);
scalar_type!(
    BoolNullableNavType,
    bool,
    "boolean_nullable",
    true,
    |v| SavedValue::Bool(*v),
    get_boolean_or_null,
    |v| null_or(v, parse_bool),
    |v| v.to_string()
);
scalar_type!(
    StringNavType,
    String,
    "string",
    true,
    |v| SavedValue::String(v.clone()),
    get_string_or_null,
    |v| Ok(if v == "null" { None } else { Some(v.to_string()) }),
    |v| uri::encode(v, None)
);

/// Kotlin's internal `StringNonNullableType`: `"null"` is the text "null", and a missing one reads as it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StringNonNullableNavType;

impl NavTypeImpl for StringNonNullableNavType {
    type Value = String;
    fn is_nullable_allowed(&self) -> bool {
        false
    }
    fn name(&self) -> String {
        "string_non_nullable".into()
    }
    fn put(&self, bundle: &SavedState, key: &str, value: Option<&String>) {
        put_or_null(bundle, key, value.map(|v| SavedValue::String(v.clone())));
    }
    fn get(&self, bundle: &SavedState, key: &str) -> Option<String> {
        Some(read(bundle, key, |b, k| b.get_string_or_null(k)).unwrap_or_else(|| "null".into()))
    }
    fn parse_value(&self, value: &str) -> Result<Option<String>, NavTypeError> {
        Ok(Some(value.to_string()))
    }
    fn serialize_as_value(&self, value: Option<&String>) -> String {
        value.map_or_else(|| "null".into(), |v| uri::encode(v, None))
    }
}

/// A collection type: arrays (`Box<[T]>`, Kotlin's `IntArray`, `Array<String>` …) and lists (`Vec<T>`).
macro_rules! collection_type {
    ($ty:ident, $value:ty, $elem:ty, $name:literal, $put:expr, $get:ident, $parse_one:expr, $serialize_one:expr, $wrap:expr, $unwrap:expr) => {
        #[derive(Clone, Copy, Debug, Default)]
        pub(crate) struct $ty;
        impl NavTypeImpl for $ty {
            type Value = $value;
            fn is_nullable_allowed(&self) -> bool {
                true
            }
            fn name(&self) -> String {
                $name.into()
            }
            fn put(&self, bundle: &SavedState, key: &str, value: Option<&$value>) {
                let one: fn($elem) -> SavedValue = $put;
                let unwrap: fn(&$value) -> &[$elem] = $unwrap;
                put_or_null(bundle, key, value.map(|v| list_value(unwrap(v), one)));
            }
            fn get(&self, bundle: &SavedState, key: &str) -> Option<$value> {
                let wrap: fn(Vec<$elem>) -> $value = $wrap;
                read(bundle, key, |b, k| b.$get(k)).map(wrap)
            }
            fn parse_value(&self, value: &str) -> Result<Option<$value>, NavTypeError> {
                let one: fn(&str) -> Result<$elem, NavTypeError> = $parse_one;
                let wrap: fn(Vec<$elem>) -> $value = $wrap;
                Ok(Some(wrap(vec![one(value)?])))
            }
            fn parse_value_with(&self, value: &str, previous: Option<&$value>) -> Result<Option<$value>, NavTypeError> {
                let one: fn(&str) -> Result<$elem, NavTypeError> = $parse_one;
                let wrap: fn(Vec<$elem>) -> $value = $wrap;
                let unwrap: fn(&$value) -> &[$elem] = $unwrap;
                Ok(Some(wrap(plus(previous.map(unwrap), vec![one(value)?]))))
            }
            fn serialize_as_value(&self, value: Option<&$value>) -> String {
                // (Kotlin's default: the collection's toString())
                let s: fn(&$elem) -> String = $serialize_one;
                let unwrap: fn(&$value) -> &[$elem] = $unwrap;
                value.map_or_else(
                    || "null".into(),
                    |v| format!("[{}]", unwrap(v).iter().map(s).collect::<Vec<_>>().join(", ")),
                )
            }
            fn as_collection(&self) -> Option<&dyn CollectionNavType<Value = $value>> {
                Some(self)
            }
        }
        impl CollectionNavType for $ty {
            fn serialize_as_values(&self, value: Option<&$value>) -> Vec<String> {
                let s: fn(&$elem) -> String = $serialize_one;
                let unwrap: fn(&$value) -> &[$elem] = $unwrap;
                value.map(|v| unwrap(v).iter().map(s).collect()).unwrap_or_default()
            }
            fn empty_collection(&self) -> $value {
                let wrap: fn(Vec<$elem>) -> $value = $wrap;
                wrap(Vec::new())
            }
        }
    };
}

fn boxed<T>(v: Vec<T>) -> Box<[T]> {
    v.into_boxed_slice()
}
fn listed<T>(v: Vec<T>) -> Vec<T> {
    v
}

collection_type!(
    IntArrayNavType,
    Box<[i32]>,
    i32,
    "integer[]",
    |v| SavedValue::Int(v as i64),
    get_int_array_or_null,
    int_value,
    |v| v.to_string(),
    boxed,
    |v| v
);
collection_type!(
    IntListNavType,
    Vec<i32>,
    i32,
    "List<Int>",
    |v| SavedValue::Int(v as i64),
    get_int_array_or_null,
    int_value,
    |v| v.to_string(),
    listed,
    |v| v
);
collection_type!(
    LongArrayNavType,
    Box<[i64]>,
    i64,
    "long[]",
    SavedValue::Int,
    get_long_array_or_null,
    long_value,
    |v| v.to_string(),
    boxed,
    |v| v
);
collection_type!(
    LongListNavType,
    Vec<i64>,
    i64,
    "List<Long>",
    SavedValue::Int,
    get_long_array_or_null,
    long_value,
    |v| v.to_string(),
    listed,
    |v| v
);
collection_type!(
    FloatArrayNavType,
    Box<[f32]>,
    f32,
    "float[]",
    |v| SavedValue::Double(v as f64),
    get_float_array_or_null,
    parse_java_float,
    |v| java::float_to_string(*v),
    boxed,
    |v| v
);
collection_type!(
    FloatListNavType,
    Vec<f32>,
    f32,
    "List<Float>",
    |v| SavedValue::Double(v as f64),
    get_float_array_or_null,
    parse_java_float,
    |v| java::float_to_string(*v),
    listed,
    |v| v
);
collection_type!(
    DoubleArrayNavType,
    Box<[f64]>,
    f64,
    "double[]",
    SavedValue::Double,
    get_double_array_or_null,
    parse_java_double,
    |v| java::double_to_string(*v),
    boxed,
    |v| v
);
collection_type!(
    DoubleListNavType,
    Vec<f64>,
    f64,
    "List<Double>",
    SavedValue::Double,
    get_double_array_or_null,
    parse_java_double,
    |v| java::double_to_string(*v),
    listed,
    |v| v
);
collection_type!(
    BoolArrayNavType,
    Box<[bool]>,
    bool,
    "boolean[]",
    SavedValue::Bool,
    get_boolean_array_or_null,
    parse_bool,
    |v| v.to_string(),
    boxed,
    |v| v
);
collection_type!(
    BoolListNavType,
    Vec<bool>,
    bool,
    "List<Boolean>",
    SavedValue::Bool,
    get_boolean_array_or_null,
    parse_bool,
    |v| v.to_string(),
    listed,
    |v| v
);
collection_type!(
    StringArrayNavType,
    Box<[String]>,
    String,
    "string[]",
    SavedValue::String,
    get_string_array_or_null,
    |v| Ok(v.to_string()),
    |v| uri::encode(v, None),
    boxed,
    |v| v
);
collection_type!(
    StringListNavType,
    Vec<String>,
    String,
    "List<String>",
    SavedValue::String,
    get_string_array_or_null,
    |v| Ok(v.to_string()),
    |v| uri::encode(v, None),
    listed,
    |v| v
);

/// Nullable strings in an array or list: null kept as the text "null" (Kotlin's internal `StringNullableArrayType`
/// and `StringNullableListType`).
macro_rules! nullable_strings_type {
    ($ty:ident, $value:ty, $name:literal, $wrap:expr) => {
        #[derive(Clone, Copy, Debug, Default)]
        pub(crate) struct $ty;
        impl NavTypeImpl for $ty {
            type Value = $value;
            fn is_nullable_allowed(&self) -> bool {
                true
            }
            fn name(&self) -> String {
                $name.into()
            }
            fn put(&self, bundle: &SavedState, key: &str, value: Option<&$value>) {
                put_or_null(
                    bundle,
                    key,
                    value.map(|v| {
                        SavedValue::list(
                            v.iter().map(|s| SavedValue::String(s.clone().unwrap_or_else(|| "null".into()))),
                        )
                    }),
                );
            }
            fn get(&self, bundle: &SavedState, key: &str) -> Option<$value> {
                let wrap: fn(Vec<Option<String>>) -> $value = $wrap;
                read(bundle, key, |b, k| b.get_string_array_or_null(k))
                    .map(|v| wrap(v.into_iter().map(|s| if s == "null" { None } else { Some(s) }).collect()))
            }
            fn parse_value(&self, value: &str) -> Result<Option<$value>, NavTypeError> {
                let wrap: fn(Vec<Option<String>>) -> $value = $wrap;
                Ok(Some(wrap(vec![if value == "null" { None } else { Some(value.to_string()) }])))
            }
            fn parse_value_with(&self, value: &str, previous: Option<&$value>) -> Result<Option<$value>, NavTypeError> {
                let wrap: fn(Vec<Option<String>>) -> $value = $wrap;
                let one = if value == "null" { None } else { Some(value.to_string()) };
                Ok(Some(wrap(plus(previous.map(|p| &p[..]), vec![one]))))
            }
            fn serialize_as_value(&self, value: Option<&$value>) -> String {
                value.map_or_else(|| "null".into(), |v| format!("{v:?}"))
            }
            fn as_collection(&self) -> Option<&dyn CollectionNavType<Value = $value>> {
                Some(self)
            }
        }
        impl CollectionNavType for $ty {
            fn serialize_as_values(&self, value: Option<&$value>) -> Vec<String> {
                value
                    .map(|v| {
                        v.iter().map(|s| s.as_ref().map_or_else(|| "null".into(), |s| uri::encode(s, None))).collect()
                    })
                    .unwrap_or_default()
            }
            fn empty_collection(&self) -> $value {
                let wrap: fn(Vec<Option<String>>) -> $value = $wrap;
                wrap(Vec::new())
            }
        }
    };
}

nullable_strings_type!(StringNullableArrayNavType, Box<[Option<String>]>, "string_nullable[]", boxed);
nullable_strings_type!(StringNullableListNavType, Vec<Option<String>>, "List<String?>", listed);

// ---- Enums ----

/// An enum a route's argument can be (what `#[derive(RouteValue)]` gives an enum of unit variants; Kotlin's
/// `Enum`): its entries in order and each one's name (Kotlin's `entries` and `name`).
pub trait NavEnum: Clone + PartialEq + 'static {
    /// Its entries, in order (Kotlin's `entries`).
    fn entries() -> &'static [Self];
    /// This entry's name (Kotlin's `name`): its variant's.
    fn name(&self) -> &'static str;
    /// The type's name (Kotlin's class name, an enum `NavType`'s name).
    fn type_name() -> &'static str {
        std::any::type_name::<Self>()
    }
}

fn enum_by_name<E: NavEnum>(value: &str) -> Result<E, NavTypeError> {
    let wanted = value.to_lowercase();
    E::entries()
        .iter()
        .find(|e| e.name().to_lowercase() == wanted)
        .cloned()
        .ok_or_else(|| NavTypeError(format!("Enum value {value} not found for type {}.", E::type_name())))
}

/// Kotlin's `NavType.EnumType`: an enum's entries by name (any case); a value is kept as its name.
pub(crate) struct EnumNavType<E>(PhantomData<E>);

impl<E: NavEnum> NavTypeImpl for EnumNavType<E> {
    type Value = E;
    fn is_nullable_allowed(&self) -> bool {
        false
    }
    fn name(&self) -> String {
        E::type_name().into()
    }
    fn put(&self, bundle: &SavedState, key: &str, value: Option<&E>) {
        put_or_null(bundle, key, value.map(|v| SavedValue::String(v.name().into())));
    }
    fn get(&self, bundle: &SavedState, key: &str) -> Option<E> {
        read(bundle, key, |b, k| b.get_string_or_null(k)).and_then(|n| enum_by_name(&n).ok())
    }
    fn parse_value(&self, value: &str) -> Result<Option<E>, NavTypeError> {
        enum_by_name(value).map(Some)
    }
    fn serialize_as_value(&self, value: Option<&E>) -> String {
        value.map_or_else(|| "null".into(), |v| v.name().into())
    }
}

/// Kotlin's internal `EnumNullableType`: the same, `"null"` null.
pub(crate) struct EnumNullableNavType<E>(PhantomData<E>);

impl<E: NavEnum> NavTypeImpl for EnumNullableNavType<E> {
    type Value = E;
    fn is_nullable_allowed(&self) -> bool {
        true
    }
    fn name(&self) -> String {
        E::type_name().into()
    }
    fn put(&self, bundle: &SavedState, key: &str, value: Option<&E>) {
        put_or_null(bundle, key, value.map(|v| SavedValue::String(v.name().into())));
    }
    fn get(&self, bundle: &SavedState, key: &str) -> Option<E> {
        read(bundle, key, |b, k| b.get_string_or_null(k)).and_then(|n| enum_by_name(&n).ok())
    }
    fn parse_value(&self, value: &str) -> Result<Option<E>, NavTypeError> {
        null_or(value, enum_by_name)
    }
    fn serialize_as_value(&self, value: Option<&E>) -> String {
        value.map_or_else(|| "null".into(), |v| v.name().into())
    }
}

/// Kotlin's internal `EnumListType`.
pub(crate) struct EnumListNavType<E>(PhantomData<E>);

impl<E: NavEnum> NavTypeImpl for EnumListNavType<E> {
    type Value = Vec<E>;
    fn is_nullable_allowed(&self) -> bool {
        true
    }
    fn name(&self) -> String {
        format!("List<{}}}>", E::type_name())
    }
    fn put(&self, bundle: &SavedState, key: &str, value: Option<&Vec<E>>) {
        put_or_null(
            bundle,
            key,
            value.map(|v| SavedValue::list(v.iter().map(|e| SavedValue::String(e.name().into())))),
        );
    }
    fn get(&self, bundle: &SavedState, key: &str) -> Option<Vec<E>> {
        read(bundle, key, |b, k| b.get_string_array_or_null(k))
            .and_then(|v| v.iter().map(|n| enum_by_name(n).ok()).collect())
    }
    fn parse_value(&self, value: &str) -> Result<Option<Vec<E>>, NavTypeError> {
        Ok(Some(vec![enum_by_name(value)?]))
    }
    fn parse_value_with(&self, value: &str, previous: Option<&Vec<E>>) -> Result<Option<Vec<E>>, NavTypeError> {
        Ok(Some(plus(previous.map(|p| &p[..]), vec![enum_by_name(value)?])))
    }
    fn serialize_as_value(&self, value: Option<&Vec<E>>) -> String {
        value.map_or_else(
            || "null".into(),
            |v| format!("[{}]", v.iter().map(NavEnum::name).collect::<Vec<_>>().join(", ")),
        )
    }
    fn as_collection(&self) -> Option<&dyn CollectionNavType<Value = Vec<E>>> {
        Some(self)
    }
}

impl<E: NavEnum> CollectionNavType for EnumListNavType<E> {
    fn serialize_as_values(&self, value: Option<&Vec<E>>) -> Vec<String> {
        value.map(|v| v.iter().map(|e| e.name().to_string()).collect()).unwrap_or_default()
    }
    fn empty_collection(&self) -> Vec<E> {
        Vec::new()
    }
}

pub(crate) fn enum_nav_type<E: NavEnum>() -> NavType {
    NavType::new(EnumNavType::<E>(PhantomData))
}
pub(crate) fn enum_nullable_nav_type<E: NavEnum>() -> NavType {
    NavType::new(EnumNullableNavType::<E>(PhantomData))
}
pub(crate) fn enum_list_nav_type<E: NavEnum>() -> NavType {
    NavType::new(EnumListNavType::<E>(PhantomData))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_ins_parse_put_and_write_as_kotlins() {
        let b = SavedState::new();
        NavType::IntType.parse_and_put(&b, "n", "0x2A").unwrap();
        assert_eq!(b.read(|r| r.get_int("n")), 42);
        assert!(NavType::IntType.parse_value::<i32>("4.5").is_err());
        assert_eq!(NavType::LongType.parse_value::<i64>("42L").unwrap(), Some(42));
        assert_eq!(NavType::FloatType.parse_value::<f32>(" 1.5f ").unwrap(), Some(1.5));
        assert!(NavType::BoolType.parse_value::<bool>("True").is_err());
        assert_eq!(NavType::StringType.parse_value::<String>("null").unwrap(), None);
        assert_eq!(NavType::StringType.serialize_as_value(Some(&"a b/c".to_string())), "a%20b%2Fc");
        assert_eq!(NavType::FloatType.serialize_as_value(Some(&2.0f32)), "2.0");
        let l = SavedState::new();
        NavType::IntListType.parse_and_put(&l, "ids", "1").unwrap();
        NavType::IntListType.erased().parse_and_put_repeated(&l, "ids", "2").unwrap();
        assert_eq!(NavType::IntListType.get::<Vec<i32>>(&l, "ids"), Some(vec![1, 2]));
        assert_eq!(NavType::IntListType.serialize_as_values(Some(&vec![1, 2])), Some(vec!["1".into(), "2".into()]));
        assert!(NavType::IntListType.is_collection() && !NavType::IntType.is_collection());
        assert_eq!(NavType::from_arg_type(Some("List<Long>"), None), NavType::LongListType);
        assert_eq!(NavType::infer_from_value("12345678901"), NavType::LongType);
        assert_eq!(NavType::infer_from_value("1.5"), NavType::FloatType);
        assert_eq!(NavType::infer_from_value_type(Some(&true)), NavType::BoolType);
        assert_ne!(NavType::IntType, NavType::LongType);
    }
}

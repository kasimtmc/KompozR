//! NavMetadataKey.kt, NavMetadata.kt: an entry's (and a scene's) metadata, Kotlin's `Map<String, Any>`.

use std::any::Any;
use std::fmt;
use std::ops::Add;
use std::rc::Rc;

/// A key of a value of type `T` in a [`Metadata`] (Compose's `NavMetadataKey<T>`): the `metadata { put(key, value) }`
/// DSL takes only these. A key is an object of its own (a unit struct): `impl NavMetadataKey<Dp> for PaneWidth {}`.
/// Its name in the map is its type's (Kotlin's `key.toString()` of an `object`).
pub trait NavMetadataKey<T>: 'static {
    /// Its name in the map.
    fn metadata_key_name(&self) -> String {
        std::any::type_name::<Self>().to_string()
    }
}

/// What an entry or a scene tells its display (Compose's `Map<String, Any>` metadata): values of any type by name,
/// in the order they were put. Two are equal when they have the same names with the same values (each value compared
/// by identity: Kotlin compares them with `equals`, here they can be of any type — closures —; a map built once, as
/// an entry provider's are, is equal to itself).
#[derive(Clone, Default)]
pub struct Metadata {
    entries: Rc<Vec<(String, Rc<dyn Any>)>>,
}

impl Metadata {
    /// An empty one (Kotlin's `emptyMap()`).
    pub fn new() -> Metadata {
        Metadata::default()
    }

    /// One of a value by name (Kotlin's `mapOf(name to value)`).
    pub fn of<V: 'static>(name: impl Into<String>, value: V) -> Metadata {
        Metadata { entries: Rc::new(vec![(name.into(), Rc::new(value) as Rc<dyn Any>)]) }
    }

    /// The value of `key`, if there is one of its type (Kotlin's `metadata[key]`, an operator extension of
    /// `Map<String, Any>`).
    pub fn get<T: Clone + 'static>(&self, key: impl NavMetadataKey<T>) -> Option<T> {
        self.get_by_name(&key.metadata_key_name())?.downcast_ref::<T>().cloned()
    }

    /// Whether it has a value for `key` (Kotlin's `key in metadata`).
    pub fn contains<T>(&self, key: impl NavMetadataKey<T>) -> bool {
        self.contains_name(&key.metadata_key_name())
    }

    /// The value of the name `name`, of any type (Kotlin's `metadata["name"]`).
    pub fn get_by_name(&self, name: &str) -> Option<Rc<dyn Any>> {
        self.entries.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone())
    }

    /// The value of the name `name` if it is a `T` (Kotlin's `metadata["name"] as? T`).
    pub fn get_as<T: Clone + 'static>(&self, name: &str) -> Option<T> {
        self.get_by_name(name)?.downcast_ref::<T>().cloned()
    }

    /// Whether it has a value of the name `name` (Kotlin's `metadata.containsKey(name)`).
    pub fn contains_name(&self, name: &str) -> bool {
        self.entries.iter().any(|(n, _)| n == name)
    }

    /// Its names, in order (Kotlin's `keys`).
    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|(n, _)| n.clone()).collect()
    }

    /// How many values it has (Kotlin's `size`).
    pub fn size(&self) -> i32 {
        self.entries.len() as i32
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// This one with `other`'s values, `other`'s taking the place of this one's of the same names (Kotlin's
    /// `metadata + other`).
    pub fn plus(&self, other: &Metadata) -> Metadata {
        let mut entries: Vec<(String, Rc<dyn Any>)> = (*self.entries).clone();
        for (name, value) in other.entries.iter() {
            match entries.iter_mut().find(|(n, _)| n == name) {
                Some(e) => e.1 = value.clone(),
                None => entries.push((name.clone(), value.clone())),
            }
        }
        Metadata { entries: Rc::new(entries) }
    }

    pub(crate) fn put_by_name(&mut self, name: String, value: Rc<dyn Any>) {
        let entries = Rc::make_mut(&mut self.entries);
        match entries.iter_mut().find(|(n, _)| *n == name) {
            Some(e) => e.1 = value,
            None => entries.push((name, value)),
        }
    }
}

impl Add for Metadata {
    type Output = Metadata;
    fn add(self, other: Metadata) -> Metadata {
        self.plus(&other)
    }
}

impl Add<&Metadata> for &Metadata {
    type Output = Metadata;
    fn add(self, other: &Metadata) -> Metadata {
        self.plus(other)
    }
}

impl PartialEq for Metadata {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.entries, &other.entries)
            || (self.entries.len() == other.entries.len()
                && self.entries.iter().all(|(n, v)| other.entries.iter().any(|(m, w)| n == m && Rc::ptr_eq(v, w))))
    }
}

impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.entries.iter().map(|(n, _)| (n, "…"))).finish()
    }
}

/// What the [`metadata`] DSL's builder is given (Compose's `MetadataScope`): `put(key, value)` adds a value.
#[derive(Default)]
pub struct MetadataScope {
    map: Metadata,
}

impl MetadataScope {
    pub fn new() -> MetadataScope {
        MetadataScope::default()
    }

    /// Adds `value` for `key` (Compose's `put(key: K, value: T)`): a value of the key's type, or what makes one (a
    /// lambda of a transition key, as Kotlin writes it: `put(NavDisplay.TransitionKey) { slideIn… }`).
    pub fn put<T: 'static, M>(&mut self, key: impl NavMetadataKey<T>, value: impl IntoMetadataValue<T, M>) {
        self.map.put_by_name(key.metadata_key_name(), Rc::new(value.into_metadata_value()));
    }

    /// The metadata built (Compose's internal `build()`).
    pub fn build(self) -> Metadata {
        self.map
    }
}

/// What makes a metadata value of type `T` (its `M` tells the ways apart): a `T` itself, or a lambda of a transition
/// key (made into the key's value type).
pub trait IntoMetadataValue<T, M> {
    fn into_metadata_value(self) -> T;
}

/// A value as itself.
#[doc(hidden)]
pub struct Itself;

impl<T> IntoMetadataValue<T, Itself> for T {
    fn into_metadata_value(self) -> T {
        self
    }
}

/// Builds metadata with the DSL (Compose's `metadata { put(key, value) }`).
///
/// Kotlin: `metadata(builder: MetadataScope.() -> Unit): Map<String, Any>`
pub fn metadata(builder: impl FnOnce(&mut MetadataScope)) -> Metadata {
    let mut scope = MetadataScope::new();
    builder(&mut scope);
    scope.build()
}

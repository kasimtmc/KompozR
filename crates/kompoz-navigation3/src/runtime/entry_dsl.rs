//! EntryProvider.kt: the `entryProvider { entry<K> { … } }` DSL.

use super::nav_entry::NavEntry;
use super::nav_key::{NavEntryKey, NavKey, default_content_key};
use super::nav_metadata::Metadata;
use kompoz_runtime::kt::panic_kt;
use kompoz_ui::layout::SlotKey;
use std::any::TypeId;
use std::cell::RefCell;
use std::rc::Rc;

/// What a key of type `K` is among keys of type `T` (Kotlin's `K : T`): `K` itself, or a [`NavKey`] among
/// `Rc<dyn NavKey>`s.
pub trait EntryKeyOf<T>: NavEntryKey {
    /// It as a `T`.
    fn into_entry_key(self) -> T;
    /// `key` as a `K`, if it is one (of `K`'s class exactly: Kotlin's `key::class == K::class`).
    fn from_entry_key(key: &T) -> Option<Self>;
}

impl<K: NavEntryKey> EntryKeyOf<K> for K {
    fn into_entry_key(self) -> K {
        self
    }
    fn from_entry_key(key: &K) -> Option<K> {
        Some(key.clone())
    }
}

impl<K: NavKey + Clone + PartialEq> EntryKeyOf<Rc<dyn NavKey>> for K {
    fn into_entry_key(self) -> Rc<dyn NavKey> {
        Rc::new(self)
    }
    fn from_entry_key(key: &Rc<dyn NavKey>) -> Option<K> {
        key.downcast_ref::<K>().cloned()
    }
}

/// An entry's metadata as an entry provider takes it (Compose's two overloads of each `entry`): a [`Metadata`], or a
/// lambda making it from the key (`metadata = { key -> … }`).
pub trait EntryMetadata<K>: 'static {
    fn metadata_of(&self, key: &K) -> Metadata;
}

impl<K> EntryMetadata<K> for Metadata {
    fn metadata_of(&self, _: &K) -> Metadata {
        self.clone()
    }
}

/// No metadata (Kotlin's default `emptyMap()` of an entry's metadata).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NoMetadata;

impl<K> EntryMetadata<K> for NoMetadata {
    fn metadata_of(&self, _: &K) -> Metadata {
        Metadata::new()
    }
}

impl<K, F: Fn(K) -> Metadata + 'static> EntryMetadata<K> for F
where
    K: Clone,
{
    fn metadata_of(&self, key: &K) -> Metadata {
        self(key.clone())
    }
}

/// An entry provider of a class (Compose's private `EntryClassProvider`), on keys of type `T`.
struct ClassProvider<T> {
    name: &'static str,
    content_key: Rc<dyn Fn(&T) -> SlotKey>,
    metadata: Rc<dyn Fn(&T) -> Metadata>,
    content: Rc<dyn Fn(T)>,
}

/// An entry provider of a key (Compose's private `EntryProvider`).
struct KeyProvider<T> {
    key: T,
    content_key: SlotKey,
    metadata: Rc<dyn Fn(&T) -> Metadata>,
    content: Rc<dyn Fn(T)>,
}

/// Where the [`entry_provider`] DSL adds entries (Compose's `EntryProviderScope`): of a key (`entry(key) { }`) or of a
/// class of keys (`entry<K> { }`); a key none of them is for gets the fallback's entry.
pub struct EntryProviderScope<T> {
    fallback: Rc<dyn Fn(T) -> NavEntry<T>>,
    clazz_providers: Vec<(TypeId, ClassProvider<T>)>,
    providers: Vec<KeyProvider<T>>,
}

#[kompoz_runtime::named]
impl<T: NavEntryKey> EntryProviderScope<T> {
    /// A scope whose unknown keys get `fallback`'s entries (Compose's constructor).
    pub fn new(fallback: impl Fn(T) -> NavEntry<T> + 'static) -> EntryProviderScope<T> {
        EntryProviderScope { fallback: Rc::new(fallback), clazz_providers: Vec::new(), providers: Vec::new() }
    }

    /// Adds the entry of `key` showing `content` (Compose's `addEntryProvider(key, contentKey, metadata, content)`,
    /// both overloads: `metadata` a [`Metadata`] or a lambda of the key). Its content key is
    /// [`default_content_key`]`(key)` unless given. A key given twice panics.
    ///
    /// Kotlin: `addEntryProvider(key: K, contentKey: Any = defaultContentKey(key), metadata: Map<String, Any> = emptyMap(), content: @Composable (K) -> Unit)`
    pub fn add_entry_provider<K: EntryKeyOf<T>>(
        &mut self,
        key: K,
        #[default(None)] content_key: Option<SlotKey>,
        #[default(NoMetadata)] metadata: impl EntryMetadata<K>,
        content: impl Fn(K) + 'static,
    ) {
        self.add_key(key, content_key, metadata, content);
    }

    /// Adds the entry of `key` (Compose's `entry(key, contentKey, metadata, content)`, both overloads): what
    /// [`add_entry_provider`](Self::add_entry_provider) does.
    ///
    /// Kotlin: `entry(key: K, contentKey: Any = defaultContentKey(key), metadata: Map<String, Any> = emptyMap(), content: @Composable (K) -> Unit)`
    pub fn entry_of_key<K: EntryKeyOf<T>>(
        &mut self,
        key: K,
        #[default(None)] content_key: Option<SlotKey>,
        #[default(NoMetadata)] metadata: impl EntryMetadata<K>,
        content: impl Fn(K) + 'static,
    ) {
        self.add_key(key, content_key, metadata, content);
    }

    /// Adds the entries of the keys of class `K` (Compose's `addEntryProvider(clazz, clazzContentKey, metadata,
    /// content)`, both overloads): a key's content key is `clazz_content_key`'s of it ([`default_content_key`] unless
    /// given). A class given twice panics.
    ///
    /// Kotlin: `addEntryProvider(clazz: KClass<out K>, clazzContentKey: (key: K) -> Any = { defaultContentKey(it) }, metadata: Map<String, Any> = emptyMap(), content: @Composable (K) -> Unit)`
    pub fn add_entry_provider_of_class<K: EntryKeyOf<T>>(
        &mut self,
        #[default(None)] clazz_content_key: Option<Rc<dyn Fn(K) -> SlotKey>>,
        #[default(NoMetadata)] metadata: impl EntryMetadata<K>,
        content: impl Fn(K) + 'static,
    ) {
        self.add_class(clazz_content_key, metadata, content);
    }

    /// Adds the entries of the keys of class `K` (Compose's reified `entry<K>(clazzContentKey, metadata, content)`,
    /// both overloads): what [`add_entry_provider_of_class`](Self::add_entry_provider_of_class) does.
    ///
    /// Kotlin: `entry(clazzContentKey: (key: K) -> Any = { defaultContentKey(it) }, metadata: Map<String, Any> = emptyMap(), content: @Composable (K) -> Unit)`
    pub fn entry<K: EntryKeyOf<T>>(
        &mut self,
        #[default(None)] clazz_content_key: Option<Rc<dyn Fn(K) -> SlotKey>>,
        #[default(NoMetadata)] metadata: impl EntryMetadata<K>,
        content: impl Fn(K) + 'static,
    ) {
        self.add_class(clazz_content_key, metadata, content);
    }
}

impl<T: NavEntryKey> EntryProviderScope<T> {
    fn add_key<K: EntryKeyOf<T>>(
        &mut self,
        key: K,
        content_key: Option<SlotKey>,
        metadata: impl EntryMetadata<K>,
        content: impl Fn(K) + 'static,
    ) {
        let content_key = content_key.unwrap_or_else(|| default_content_key(&key));
        let t = key.clone().into_entry_key();
        if self.providers.iter().any(|p| p.key == t) {
            panic_kt(
                "IllegalArgumentException",
                &format!("An `entry` with the key `key` has already been added: {}.", key.key_to_string()),
            );
        }
        self.providers.push(KeyProvider {
            key: t,
            content_key,
            metadata: Rc::new(move |t: &T| match K::from_entry_key(t) {
                Some(k) => metadata.metadata_of(&k),
                None => Metadata::new(),
            }),
            content: Rc::new(move |t: T| {
                if let Some(k) = K::from_entry_key(&t) {
                    content(k)
                }
            }),
        });
    }

    fn add_class<K: EntryKeyOf<T>>(
        &mut self,
        clazz_content_key: Option<Rc<dyn Fn(K) -> SlotKey>>,
        metadata: impl EntryMetadata<K>,
        content: impl Fn(K) + 'static,
    ) {
        let id = TypeId::of::<K>();
        let name = std::any::type_name::<K>();
        if self.clazz_providers.iter().any(|(c, _)| *c == id) {
            let simple = name.rsplit("::").next().unwrap_or(name);
            panic_kt(
                "IllegalArgumentException",
                &format!("An `entry` with the same `clazz` has already been added: {simple}."),
            );
        }
        let content_key: Rc<dyn Fn(&T) -> SlotKey> = Rc::new(move |t: &T| {
            let k = K::from_entry_key(t).expect("a key of its class");
            match &clazz_content_key {
                Some(f) => f(k),
                None => default_content_key(&k),
            }
        });
        let provider = ClassProvider {
            name,
            content_key,
            metadata: Rc::new(move |t: &T| match K::from_entry_key(t) {
                Some(k) => metadata.metadata_of(&k),
                None => Metadata::new(),
            }),
            content: Rc::new(move |t: T| {
                if let Some(k) = K::from_entry_key(&t) {
                    content(k)
                }
            }),
        };
        self.clazz_providers.push((id, provider));
    }

    /// The entry provider of the entries added (Compose's internal `build()`): a key's entry is its key provider's,
    /// else its class's, else the fallback's; each key's metadata is computed once.
    pub fn build(self) -> impl Fn(T) -> NavEntry<T> + Clone + 'static {
        let metadata_cache: Rc<RefCell<Vec<(T, Metadata)>>> = Rc::new(RefCell::new(Vec::new()));
        let scope = Rc::new(self);
        move |key: T| {
            let class = scope.clazz_providers.iter().find(|(id, _)| *id == key.key_value().type_id()).map(|(_, p)| p);
            let provider = scope.providers.iter().find(|p| p.key == key);
            let cached = metadata_cache.borrow().iter().find(|(k, _)| *k == key).map(|(_, m)| m.clone());
            let metadata = match cached {
                Some(m) => m,
                None => {
                    let m = match (class, provider) {
                        (Some(c), _) => (c.metadata)(&key),
                        (None, Some(p)) => (p.metadata)(&key),
                        _ => Metadata::new(),
                    };
                    metadata_cache.borrow_mut().push((key.clone(), m.clone()));
                    m
                }
            };
            if let Some(p) = provider {
                let content = p.content.clone();
                return NavEntry::new(key, p.content_key.clone(), metadata, move |t: T| content(t));
            }
            if let Some(c) = class {
                let content = c.content.clone();
                let content_key = (c.content_key)(&key);
                let _ = c.name;
                return NavEntry::new(key, content_key, metadata, move |t: T| content(t));
            }
            (scope.fallback)(key)
        }
    }
}

/// An entry provider built with the DSL (Compose's `entryProvider(fallback) { entry<K> { … } }`): the entries
/// `builder` adds; a key none of them is for gets `fallback`'s entry (by default it panics: "Unknown screen").
///
/// Kotlin: `entryProvider(fallback: (unknownScreen: T) -> NavEntry<T> = { throw IllegalStateException("Unknown screen $it") }, builder: EntryProviderScope<T>.() -> Unit): (T) -> NavEntry<T>`
#[kompoz_runtime::named]
pub fn entry_provider<T: NavEntryKey>(
    #[default(unknown_screen::<T>)] fallback: impl Fn(T) -> NavEntry<T> + 'static,
    builder: impl FnOnce(&mut EntryProviderScope<T>),
) -> impl Fn(T) -> NavEntry<T> + Clone + 'static {
    let mut scope = EntryProviderScope::new(fallback);
    builder(&mut scope);
    scope.build()
}

/// The default fallback: Kotlin's `throw IllegalStateException("Unknown screen $it")`.
#[doc(hidden)]
pub fn unknown_screen<T: NavEntryKey>(key: T) -> NavEntry<T> {
    panic_kt("IllegalStateException", &format!("Unknown screen {}", key.key_to_string()))
}

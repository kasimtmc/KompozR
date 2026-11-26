//! NavEntry.kt, NavEntryDecorator.kt.

use super::nav_key::{NavEntryKey, default_content_key};
use super::nav_metadata::Metadata;
use kompoz_runtime::restartable;
use kompoz_ui::layout::SlotKey;
use std::fmt;
use std::rc::Rc;

/// A destination of a back stack: its key and the content shown for it (Compose's `NavEntry`), made by an entry
/// provider (the [`entry_provider`](super::entry_provider) DSL) for each key.
///
/// - `content_key` tells its content apart (and its decorators' states: entries of one content key share them);
///   derived from the key, by default [`default_content_key`] (`"$key:${key::class}"`). It must be saveable (a text
///   or a number) where a decorator saves state by it.
/// - `metadata` tells its display how to show it (a scene strategy's, `NavDisplay`'s transitions).
///
/// Two are equal when their keys, content keys and metadata are, and their content is the same closure (Kotlin's
/// `content === other.content`).
pub struct NavEntry<T> {
    key: T,
    content_key: SlotKey,
    metadata: Metadata,
    content: Rc<dyn Fn(T)>,
}

impl<T: Clone> Clone for NavEntry<T> {
    fn clone(&self) -> Self {
        NavEntry {
            key: self.key.clone(),
            content_key: self.content_key.clone(),
            metadata: self.metadata.clone(),
            content: self.content.clone(),
        }
    }
}

impl<T> NavEntry<T> {
    /// What tells its content apart (Kotlin's `contentKey`).
    pub fn content_key(&self) -> SlotKey {
        self.content_key.clone()
    }

    /// What it tells its display (Kotlin's `metadata`).
    pub fn metadata(&self) -> Metadata {
        self.metadata.clone()
    }

    #[allow(dead_code)]
    pub(crate) fn key(&self) -> &T {
        &self.key
    }
}

impl<T: NavEntryKey> NavEntry<T> {
    /// An entry of `key` showing `content` (Compose's primary constructor, every parameter given).
    pub fn new(key: T, content_key: SlotKey, metadata: Metadata, content: impl Fn(T) + 'static) -> NavEntry<T> {
        NavEntry { key, content_key, metadata, content: Rc::new(content) }
    }

    /// An entry of `nav_entry`'s key, content key and metadata showing `content` (Compose's constructor
    /// `NavEntry(navEntry, content)`).
    ///
    /// (Kotlin's `NavEntry(navEntry, content)`: the Kotlin syntax's `NavEntry(x) { … }` is the primary constructor, an
    /// argument's type not told apart there — stage 14.)
    pub fn of_entry(nav_entry: &NavEntry<T>, content: impl Fn(T) + 'static) -> NavEntry<T> {
        NavEntry {
            key: nav_entry.key.clone(),
            content_key: nav_entry.content_key.clone(),
            metadata: nav_entry.metadata.clone(),
            content: Rc::new(content),
        }
    }

    /// Composes its content with its key (Compose's `Content()`). Composable: a restartable scope of its own,
    /// skipped when the entry is the same.
    #[allow(non_snake_case)]
    #[track_caller]
    pub fn Content(&self) {
        restartable(self.clone(), Some(|a: &NavEntry<T>, b: &NavEntry<T>| a == b), |entry: &NavEntry<T>| {
            (entry.content)(entry.key.clone())
        });
    }
}

/// An entry of `key` showing `content` (Compose's `NavEntry(key, contentKey, metadata, content)`): its content key
/// [`default_content_key`] unless given, its metadata none unless given.
///
/// Kotlin: `NavEntry(key: T, contentKey: Any = defaultContentKey(key), metadata: Map<String, Any> = emptyMap(), content: @Composable (T) -> Unit)`
#[kompoz_runtime::named(type)]
pub fn NavEntry<T: NavEntryKey>(
    key: T,
    #[default(None)] content_key: Option<SlotKey>,
    #[default(Metadata::new())] metadata: Metadata,
    content: impl Fn(T) + 'static,
) -> NavEntry<T> {
    let content_key = content_key.unwrap_or_else(|| default_content_key(&key));
    NavEntry::new(key, content_key, metadata, content)
}

impl<T: PartialEq> PartialEq for NavEntry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.content_key == other.content_key
            && self.metadata == other.metadata
            && Rc::ptr_eq(&self.content, &other.content)
    }
}

impl<T: fmt::Debug> fmt::Debug for NavEntry<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "NavEntry(key={:?}, contentKey={:?}, metadata={:?}, content={:p})",
            self.key,
            self.content_key,
            self.metadata,
            Rc::as_ptr(&self.content) as *const ()
        )
    }
}

/// Kotlin's `toString()` of an entry (its `Debug`), what a data class holding one writes of it.
impl<T: fmt::Debug> kompoz_runtime::kt::KtToString for NavEntry<T> {
    fn kt_write(&self, out: &mut String) {
        out.push_str(&format!("{self:?}"));
    }
}

/// Kotlin's `hashCode()` of an entry: of its content key and its content (what its `==` compares too).
impl<T> kompoz_runtime::kt::KtHashCode for NavEntry<T> {
    fn kt_hash_code(&self) -> i32 {
        let content = Rc::as_ptr(&self.content) as *const () as usize;
        kompoz_runtime::kt::hash_code_of(&self.content_key).wrapping_mul(31).wrapping_add(content as i32)
    }
}

/// What decorates the entries [`remember_decorated_nav_entries`](super::remember_decorated_nav_entries) gives
/// (Compose's open class `NavEntryDecorator`): `decorate` composes an entry's content within something (a
/// composition local provided, a wrapper; it calls `entry.Content()`), for the entries actually shown; `on_pop` cleans
/// up the decorator's state of a content key when the last entry of that key has been popped and its content has
/// left the composition. Reused as a function returning one (or a type of one's own made into one: Kotlin's
/// subclasses are such functions here, `SaveableStateHolderNavEntryDecorator(holder)`).
///
/// Decorators are the same when they are one (Kotlin's identity of a decorator object).
pub struct NavEntryDecorator<T> {
    pub(crate) on_pop: Rc<dyn Fn(SlotKey)>,
    pub(crate) decorate: Rc<dyn Fn(NavEntry<T>)>,
}

impl<T> Clone for NavEntryDecorator<T> {
    fn clone(&self) -> Self {
        NavEntryDecorator { on_pop: self.on_pop.clone(), decorate: self.decorate.clone() }
    }
}

impl<T> PartialEq for NavEntryDecorator<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.decorate, &other.decorate) && Rc::ptr_eq(&self.on_pop, &other.on_pop)
    }
}

impl<T> fmt::Debug for NavEntryDecorator<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NavEntryDecorator@{:p}", Rc::as_ptr(&self.decorate) as *const ())
    }
}

impl<T: NavEntryKey> NavEntryDecorator<T> {
    /// One of these callbacks (Compose's constructor).
    pub fn new(on_pop: impl Fn(SlotKey) + 'static, decorate: impl Fn(NavEntry<T>) + 'static) -> NavEntryDecorator<T> {
        NavEntryDecorator { on_pop: Rc::new(on_pop), decorate: Rc::new(decorate) }
    }

    /// Calls its `on_pop` (Compose's internal `onPop`).
    pub fn on_pop(&self, content_key: SlotKey) {
        (self.on_pop)(content_key)
    }

    /// Decorates `entry` (Compose's internal `decorate`). Composable.
    pub fn decorate(&self, entry: NavEntry<T>) {
        (self.decorate)(entry)
    }
}

/// A decorator of these callbacks (Compose's `NavEntryDecorator(onPop, decorate)`): `on_pop` none unless given.
///
/// Kotlin: `NavEntryDecorator(onPop: (key: Any) -> Unit = {}, decorate: @Composable (entry: NavEntry<T>) -> Unit)`
#[kompoz_runtime::named(type)]
pub fn NavEntryDecorator<T: NavEntryKey>(
    #[default(|_: SlotKey| {})] on_pop: impl Fn(SlotKey) + 'static,
    decorate: impl Fn(NavEntry<T>) + 'static,
) -> NavEntryDecorator<T> {
    NavEntryDecorator::new(on_pop, decorate)
}

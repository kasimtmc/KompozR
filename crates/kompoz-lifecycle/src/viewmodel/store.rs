//! lifecycle-viewmodel's ViewModelStore.kt, ViewModelStoreOwner.kt, HasDefaultViewModelProviderFactory.kt,
//! ViewModelStoreOwnerDefaults.kt and the desktop DefaultViewModelProviderFactory.

use super::provider::{CreationExtras, ViewModelProviderFactory};
use super::{KClass, ViewModel};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// A key of a [`ViewModelStore`] or a [`ViewModelStoreProvider`](super::ViewModelStoreProvider) (Kotlin's `Any?`
/// keys): none, a number, or a text — made from what a Kotlin key is (`5`, `"page"`, `None`).
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum AnyKey {
    Null,
    Long(i64),
    String(String),
    /// A Kotlin object used as a key (`ViewModelStoreProvider.ProviderMarkerKey`), by its name.
    Object(&'static str),
}

impl fmt::Display for AnyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnyKey::Null => f.write_str("null"),
            AnyKey::Object(name) => f.write_str(name),
            AnyKey::Long(n) => write!(f, "{n}"),
            AnyKey::String(s) => f.write_str(s),
        }
    }
}

impl From<&str> for AnyKey {
    fn from(s: &str) -> AnyKey {
        AnyKey::String(s.to_string())
    }
}
impl From<String> for AnyKey {
    fn from(s: String) -> AnyKey {
        AnyKey::String(s)
    }
}
impl From<&String> for AnyKey {
    fn from(s: &String) -> AnyKey {
        AnyKey::String(s.clone())
    }
}
impl From<()> for AnyKey {
    fn from(_: ()) -> AnyKey {
        AnyKey::Null
    }
}
macro_rules! long_keys {
    ($($t:ty)*) => {$(
        impl From<$t> for AnyKey {
            fn from(n: $t) -> AnyKey {
                AnyKey::Long(n as i64)
            }
        }
    )*};
}
long_keys!(i8 i16 i32 i64 isize u8 u16 u32 u64 usize);
impl<K: Into<AnyKey>> From<Option<K>> for AnyKey {
    fn from(k: Option<K>) -> AnyKey {
        k.map_or(AnyKey::Null, Into::into)
    }
}

/// The ViewModels of an owner by key (Compose's `ViewModelStore`): an owner keeps its own while it lives and
/// [`clear`](Self::clear)s it as it goes for good (the host as it is destroyed, a navigation entry as it is popped).
/// A clone is the same store.
#[derive(Clone, Default)]
pub struct ViewModelStore(Rc<RefCell<Vec<(AnyKey, Rc<dyn ViewModel>)>>>);

impl PartialEq for ViewModelStore {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl ViewModelStore {
    /// An empty store (Compose's `ViewModelStore()`).
    pub fn new() -> ViewModelStore {
        ViewModelStore::default()
    }

    /// Keeps `view_model` under `key`, the one there before cleared (Compose's `put`, LIBRARY_GROUP).
    pub fn put(&self, key: impl Into<AnyKey>, view_model: Rc<dyn ViewModel>) {
        let key = key.into();
        let old = {
            let mut map = self.0.borrow_mut();
            match map.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => Some(std::mem::replace(v, view_model)),
                None => {
                    map.push((key, view_model));
                    None
                }
            }
        };
        if let Some(old) = old {
            old.clear();
        }
    }

    /// The ViewModel under `key` (Compose's `get`, LIBRARY_GROUP).
    pub fn get(&self, key: impl Into<AnyKey>) -> Option<Rc<dyn ViewModel>> {
        let key = key.into();
        self.0.borrow().iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())
    }

    /// The ViewModel under `key`, `default_value()`'s kept there if there is none (Compose's `getOrPut`).
    ///
    /// # Panics
    /// If the one there isn't a `T` (Kotlin's unchecked cast).
    pub fn get_or_put<T: ViewModel>(&self, key: impl Into<AnyKey>, default_value: impl FnOnce() -> T) -> Rc<T> {
        let key = key.into();
        let view_model = match self.get(key.clone()) {
            Some(v) => v,
            None => {
                let v: Rc<dyn ViewModel> = Rc::new(default_value());
                self.0.borrow_mut().push((key.clone(), v.clone()));
                v
            }
        };
        view_model.downcast_rc::<T>().unwrap_or_else(|| {
            panic!("ClassCastException: the ViewModel under {key} isn't a {}", std::any::type_name::<T>())
        })
    }

    /// Its keys, in the order they were put (Compose's `keys`).
    pub fn keys(&self) -> Vec<AnyKey> {
        self.0.borrow().iter().map(|(k, _)| k.clone()).collect()
    }

    /// Clears its ViewModels (each closed, then told `onCleared`) and forgets them (Compose's `clear`).
    pub fn clear(&self) {
        let snapshot = std::mem::take(&mut *self.0.borrow_mut());
        for (_, view_model) in snapshot {
            view_model.clear();
        }
    }
}

impl fmt::Debug for ViewModelStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys: Vec<String> = self.keys().iter().map(ToString::to_string).collect();
        write!(f, "ViewModelStore@{:x}(keys=[{}])", Rc::as_ptr(&self.0) as *const () as usize, keys.join(", "))
    }
}

/// What has a [`ViewModelStore`] (Compose's `ViewModelStoreOwner`): the host (the app), a navigation entry, what
/// [`remember_view_model_store_owner`](super::remember_view_model_store_owner) makes. An owner that has default
/// factories and extras says so (Kotlin's `is HasDefaultViewModelProviderFactory`). Shared as
/// `Rc<dyn ViewModelStoreOwner>`, told apart by identity.
pub trait ViewModelStoreOwner {
    /// Its store.
    fn view_model_store(&self) -> ViewModelStore;

    /// Its default factory and extras, if it has them.
    fn as_has_default_view_model_provider_factory(&self) -> Option<&dyn HasDefaultViewModelProviderFactory> {
        None
    }
}

impl PartialEq for dyn ViewModelStoreOwner {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl dyn ViewModelStoreOwner + '_ {
    /// Its default factory, else [`DefaultViewModelProviderFactory`] (Compose's `ViewModelStoreOwner?.
    /// defaultViewModelProviderFactory`).
    pub fn default_view_model_provider_factory(&self) -> Rc<dyn ViewModelProviderFactory> {
        match self.as_has_default_view_model_provider_factory() {
            Some(d) => d.default_view_model_provider_factory(),
            None => Rc::new(DefaultViewModelProviderFactory),
        }
    }

    /// Its default extras, else none (Compose's `ViewModelStoreOwner?.defaultViewModelCreationExtras`).
    pub fn default_view_model_creation_extras(&self) -> CreationExtras {
        match self.as_has_default_view_model_provider_factory() {
            Some(d) => d.default_view_model_creation_extras(),
            None => CreationExtras::Empty,
        }
    }
}

/// An owner's default factory and the extras it gives a factory (Compose's `HasDefaultViewModelProviderFactory`):
/// the host's makes ViewModels with a [`SavedStateHandle`](super::SavedStateHandle) of their own.
pub trait HasDefaultViewModelProviderFactory {
    /// The factory a provider of the owner uses when none is given.
    fn default_view_model_provider_factory(&self) -> Rc<dyn ViewModelProviderFactory> {
        Rc::new(DefaultViewModelProviderFactory)
    }

    /// The extras a factory is given (the owner's saved state registry and itself, for a SavedStateHandle).
    fn default_view_model_creation_extras(&self) -> CreationExtras {
        CreationExtras::Empty
    }
}

/// The factory of an owner without one of its own (Compose's internal desktop `DefaultViewModelProviderFactory`):
/// a class's own construction ([`ViewModel::new_instance`], Kotlin's reflection).
pub struct DefaultViewModelProviderFactory;

impl ViewModelProviderFactory for DefaultViewModelProviderFactory {
    fn create(&self, model_class: &KClass, extras: &CreationExtras) -> Rc<dyn ViewModel> {
        model_class.create_instance(extras)
    }
}

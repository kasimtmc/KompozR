//! A lazy layout's item keys (Compose's `Any` keys, held by value): any value with `Hash` and `Eq` that can be
//! written out with the state (`Into<SavedValue>`: an item's `remember_saveable` values are kept by its key, and
//! saved with it), and the keys of items not given one ([`DefaultLazyKey`]: their index).

use kompoz_runtime::SavedValue;
use std::any::{Any, TypeId};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// What a key can be: a value with `Hash`, `Eq`, `Clone` and a saved form (Compose's keys are compared by
/// `equals` and, on Android, must be saveable in a Bundle).
pub trait LazyKeyValue: Any {
    /// Whether `other` is the same key (of the same type, and equal).
    fn key_eq(&self, other: &dyn LazyKeyValue) -> bool;
    /// Hashes it (with its type).
    fn key_hash(&self, state: &mut dyn Hasher);
    fn as_any(&self) -> &dyn Any;
    /// How it is written out with the state.
    fn saved(&self) -> SavedValue;
    /// Its type's name, for what keys the saved state by it.
    fn type_name(&self) -> &'static str;
    /// Whether `other` is a value of its type equal to it.
    fn eq_any(&self, other: &dyn Any) -> bool;
}

impl<T: Any + Hash + Eq + Clone + Into<SavedValue>> LazyKeyValue for T {
    fn key_eq(&self, other: &dyn LazyKeyValue) -> bool {
        other.as_any().downcast_ref::<T>().is_some_and(|o| o == self)
    }
    fn key_hash(&self, mut state: &mut dyn Hasher) {
        TypeId::of::<T>().hash(&mut state);
        self.hash(&mut state);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn saved(&self) -> SavedValue {
        self.clone().into()
    }
    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
    fn eq_any(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| o == self)
    }
}

/// An item's key, held by value (Compose's `Any` key): made from any `Hash + Eq + Clone` value with a saved form
/// ([`LazyKey::new`], or `.into()` from a number, a `String`, a `&'static str`, a `char`, a `bool`). Two are equal
/// when their values are (of the same type). Clones are cheap.
#[derive(Clone)]
pub struct LazyKey(Rc<dyn LazyKeyValue>);

impl LazyKey {
    pub fn new<T: Hash + Eq + Clone + Into<SavedValue> + 'static>(key: T) -> LazyKey {
        LazyKey(Rc::new(key))
    }

    /// Its value, if it is a `T`.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.0.as_any().downcast_ref::<T>()
    }

    /// Whether its value is `key`.
    pub fn is<T: PartialEq + 'static>(&self, key: &T) -> bool {
        self.downcast_ref::<T>().is_some_and(|v| v == key)
    }

    /// Whether it is the key `other`: a `LazyKey` equal to it, or a value equal to its own (what accessibility's
    /// `IndexForKey` and a test's `perform_scroll_to_key` are given).
    pub fn matches(&self, other: &dyn Any) -> bool {
        match other.downcast_ref::<LazyKey>() {
            Some(key) => key == self,
            None => self.0.eq_any(other),
        }
    }

    /// How it is written out with the state.
    pub fn saved_value(&self) -> SavedValue {
        self.0.saved()
    }

    /// What the state of its item is kept under in a saveable state holder: its type and its saved form (equal keys
    /// give equal names, also after the process comes back).
    pub(crate) fn holder_key(&self) -> String {
        format!("{}#{:?}", self.0.type_name(), self.0.saved())
    }
}

impl PartialEq for LazyKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || self.0.key_eq(&*other.0)
    }
}

impl Eq for LazyKey {}

impl Hash for LazyKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.key_hash(state);
    }
}

impl fmt::Debug for LazyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0.saved())
    }
}

macro_rules! keys_from {
    ($($t:ty)*) => {$(
        impl From<$t> for LazyKey {
            fn from(key: $t) -> LazyKey {
                LazyKey::new(key)
            }
        }
    )*};
}
keys_from!(i8 i16 i32 i64 isize u8 u16 u32 u64 usize bool String &'static str);

impl From<char> for LazyKey {
    fn from(key: char) -> LazyKey {
        LazyKey::new(key.to_string())
    }
}

impl From<DefaultLazyKey> for LazyKey {
    fn from(key: DefaultLazyKey) -> LazyKey {
        LazyKey::new(key)
    }
}

/// The key of an item not given one (Compose's Android `DefaultLazyKey`): its index. Not equal to any key given (a
/// key `0` is another key than item 0's default one).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DefaultLazyKey(pub i32);

impl From<DefaultLazyKey> for SavedValue {
    fn from(key: DefaultLazyKey) -> SavedValue {
        SavedValue::map([("DefaultLazyKey", SavedValue::from(key.0))])
    }
}

/// The key of item `index` when its interval gives none (Compose's `getDefaultLazyLayoutKey`).
pub fn get_default_lazy_layout_key(index: i32) -> LazyKey {
    LazyKey::new(DefaultLazyKey(index))
}

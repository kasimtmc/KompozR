//! NavBackStack.kt, RememberNavBackStack.kt (+ RememberNavBackStack.android.kt), serialization/NavBackStackSerializer.kt
//! (+ NavKeySerializer.android.kt).

use super::decorated::BackStackList;
use super::entry_dsl::EntryKeyOf;
use super::nav_key::{NavKey, note_nav_key, restore_any_nav_key, save_any_nav_key};
use kompoz_runtime::kt::{KtIterable, panic_kt};
use kompoz_runtime::{Saveable, SavedValue, Saver, SnapshotStateList, kompozable, remember_saveable_with};
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

/// A mutable back stack of keys that is a snapshot state (Compose's `NavBackStack`, a `MutableList` and a
/// `StateObject` by its `SnapshotStateList`): a change makes its readers compose again. Its list's functions are its
/// own (it derefs to its [`SnapshotStateList`]); those taking a key take a key of any of its types (`back_stack.add(
/// Details(42))` on a back stack of `Rc<dyn NavKey>`s). Clones are the same back stack.
///
/// Usually made by [`remember_nav_back_stack`] (saved and restored), else `NavBackStack(vec![…])`.
pub struct NavBackStack<T> {
    base: SnapshotStateList<T>,
}

impl<T> Clone for NavBackStack<T> {
    fn clone(&self) -> Self {
        NavBackStack { base: self.base.clone() }
    }
}

impl<T> PartialEq for NavBackStack<T> {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
    }
}

impl<T> Deref for NavBackStack<T> {
    type Target = SnapshotStateList<T>;
    fn deref(&self) -> &SnapshotStateList<T> {
        &self.base
    }
}

impl<T: Clone + fmt::Debug + 'static> fmt::Debug for NavBackStack<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.base.with(|v| v.to_vec())).finish()
    }
}

/// A back stack of `elements` (Compose's constructors `NavBackStack()` and `NavBackStack(vararg elements)`).
///
/// Kotlin: `NavBackStack(vararg elements: T)`
pub fn NavBackStack<T: Clone + 'static>(elements: impl IntoIterator<Item = T>) -> NavBackStack<T> {
    NavBackStack::new(elements.into_iter().collect())
}

impl<T: Clone + 'static> NavBackStack<T> {
    /// A back stack of `elements`.
    pub fn new(elements: Vec<T>) -> NavBackStack<T> {
        NavBackStack { base: SnapshotStateList::new_confined(elements) }
    }

    /// A back stack over `base` (Compose's primary constructor: `base` is the list).
    ///
    /// Kotlin: `NavBackStack(base: SnapshotStateList<T>)`
    pub fn of_list(base: SnapshotStateList<T>) -> NavBackStack<T> {
        NavBackStack { base }
    }

    /// Its list (Compose's internal `base`).
    pub fn base(&self) -> &SnapshotStateList<T> {
        &self.base
    }

    /// Adds `key` at its top (Kotlin's `add(element)`, `+=`).
    ///
    /// Kotlin: `add(element: E): Boolean`
    pub fn add<K: EntryKeyOf<T>>(&self, key: K) -> bool {
        self.base.add(key.into_entry_key());
        true
    }

    /// Inserts `key` at `index`.
    ///
    /// Kotlin: `add(index: Int, element: E)`
    pub fn add_at_index<K: EntryKeyOf<T>>(&self, index: i32, key: K) {
        self.base.add_at_index(index, key.into_entry_key());
    }

    /// Kotlin's `addAll(elements)`.
    pub fn add_all<K: EntryKeyOf<T>>(&self, keys: impl IntoIterator<Item = K>) -> bool {
        self.base.add_all(keys.into_iter().map(EntryKeyOf::into_entry_key))
    }

    /// Replaces the key at `index`: the one it was.
    ///
    /// Kotlin: `set(index: Int, element: E): E`
    pub fn set<K: EntryKeyOf<T>>(&self, index: i32, key: K) -> T {
        let old = self.get_at(index);
        self.base.set(index as usize, key.into_entry_key());
        old
    }

    /// Removes the first key equal to `key`: whether there was one.
    ///
    /// Kotlin: `remove(element: E): Boolean`
    pub fn remove<K: EntryKeyOf<T>>(&self, key: K) -> bool
    where
        T: PartialEq,
    {
        self.base.remove(&key.into_entry_key())
    }

    /// Whether it has a key equal to `key`.
    ///
    /// Kotlin: `contains(element: E): Boolean`
    pub fn contains<K: EntryKeyOf<T>>(&self, key: K) -> bool
    where
        T: PartialEq,
    {
        let t = key.into_entry_key();
        self.base.with(|v| v.contains(&t))
    }

    /// The key at `index` (Kotlin's `get(index)`, `backStack[index]`).
    pub fn get_at(&self, index: i32) -> T {
        let n = self.base.len();
        match usize::try_from(index).ok().and_then(|i| self.base.get(i)) {
            Some(k) => k,
            None => panic_kt("IndexOutOfBoundsException", &format!("Index {index} out of bounds for length {n}")),
        }
    }

    /// Removes the key at `index`: the one it was.
    ///
    /// Kotlin: `removeAt(index: Int): E`
    pub fn remove_at(&self, index: i32) -> T {
        let n = self.base.len();
        if index < 0 || index as usize >= n {
            panic_kt("IndexOutOfBoundsException", &format!("Index {index} out of bounds for length {n}"));
        }
        self.base.remove_at(index as usize)
    }

    /// Removes its last key: the one it was, or none when it is empty.
    ///
    /// Kotlin: `removeLastOrNull(): E?`
    pub fn remove_last_or_null(&self) -> Option<T> {
        let n = self.base.len();
        (n > 0).then(|| self.base.remove_at(n - 1))
    }

    /// Removes its last key, which it must have.
    ///
    /// Kotlin: `removeLast(): E`
    pub fn remove_last(&self) -> T {
        self.remove_last_or_null().unwrap_or_else(|| panic_kt("NoSuchElementException", "List is empty."))
    }

    /// Removes its first key: the one it was, or none when it is empty.
    ///
    /// Kotlin: `removeFirstOrNull(): E?`
    pub fn remove_first_or_null(&self) -> Option<T> {
        (!self.base.is_empty()).then(|| self.base.remove_at(0))
    }

    /// The index of its last key (Kotlin's `lastIndex`): -1 when it is empty.
    pub fn last_index(&self) -> i32 {
        self.base.len() as i32 - 1
    }

    /// Its keys (Kotlin's `toList()`): a read that subscribes.
    pub fn to_list(&self) -> Vec<T> {
        self.base.to_vec()
    }
}

impl<T: Clone + 'static> KtIterable for NavBackStack<T> {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.base.to_vec().into_iter()
    }
}

impl<T: Clone + 'static> BackStackList<T> for NavBackStack<T> {
    fn keys(&self) -> Vec<T> {
        self.base.to_vec()
    }
    fn remove_last_key(&self) -> bool {
        self.remove_last_or_null().is_some()
    }
}

/// A back stack of keys of one type is saved as its keys are (Compose's `NavBackStackSerializer(serializer<T>())`:
/// closed polymorphism, a sealed type of keys).
impl<T: NavKey + Saveable + Clone + PartialEq> Saveable for NavBackStack<T> {
    fn descriptor() -> kompoz_runtime::SerialDescriptor {
        let mut d = Vec::<T>::descriptor();
        d.serial_name = "androidx.navigation3.runtime.NavBackStack".into();
        d
    }
    fn save(&self) -> SavedValue {
        self.base.with(|v| SavedValue::list(v.iter().map(T::save)))
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        Vec::<T>::restore(saved).map(NavBackStack::new)
    }
}

/// A back stack of keys of any type is saved with each key's class (Compose's `NavBackStackSerializer` of
/// `NavKeySerializer()`, the Android one: each key as its class's name and its value). Read back, a key's class must
/// be known in this process (Android's reflection finds it; here a key of it was saved or put in a back stack since
/// the process started, or [`register_nav_key`](super::register_nav_key) registered it): else the back stack isn't
/// restored (it is made anew).
impl Saveable for NavBackStack<Rc<dyn NavKey>> {
    fn descriptor() -> kompoz_runtime::SerialDescriptor {
        kompoz_runtime::SerialDescriptor::new(
            "androidx.navigation3.runtime.NavBackStack",
            kompoz_runtime::SerialKind::List,
        )
    }
    fn save(&self) -> SavedValue {
        self.base.with(|v| SavedValue::list(v.iter().map(save_any_nav_key)))
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        let keys: Option<Vec<Rc<dyn NavKey>>> = saved.as_list()?.iter().map(restore_any_nav_key).collect();
        keys.map(NavBackStack::new)
    }
}

/// The saver of a key of any type (Compose's `NavKeySerializer`, Android's): its class's name and its value.
///
/// Kotlin: `NavKeySerializer()`
pub fn NavKeySerializer() -> Saver<Rc<dyn NavKey>> {
    Saver::new(save_any_nav_key, restore_any_nav_key)
}

/// The saver of a back stack whose keys `element_serializer` saves (Compose's `NavBackStackSerializer(
/// elementSerializer)`).
///
/// Kotlin: `NavBackStackSerializer(elementSerializer: KSerializer<T>)`
pub fn NavBackStackSerializer<T: Clone + 'static>(element_serializer: Saver<T>) -> Saver<NavBackStack<T>> {
    let (s, r) = (element_serializer.clone(), element_serializer);
    Saver::new(
        move |stack: &NavBackStack<T>| stack.base.with(|v| SavedValue::list(v.iter().map(|k| s.save(k)))),
        move |saved: &SavedValue| {
            let keys: Option<Vec<T>> = saved.as_list()?.iter().map(|k| r.restore(k)).collect();
            keys.map(NavBackStack::new)
        },
    )
}

/// A back stack of `elements` remembered across recompositions and saved with the composition's state (restored
/// after the process ends: Compose's `rememberNavBackStack(vararg elements)`, Android's, saving each key with its
/// class: see [`NavBackStack`]'s `Saveable`). Keys are [`NavKey`]s of any type.
///
/// Kotlin: `rememberNavBackStack(vararg elements: NavKey): NavBackStack<NavKey>`
#[kompozable]
pub fn remember_nav_back_stack(elements: impl IntoIterator<Item = Rc<dyn NavKey>>) -> NavBackStack<Rc<dyn NavKey>> {
    let elements: Vec<Rc<dyn NavKey>> = elements.into_iter().collect();
    for k in &elements {
        note_nav_key(&**k);
    }
    let saver = Saver::new(
        |s: &NavBackStack<Rc<dyn NavKey>>| s.save(),
        |v: &SavedValue| NavBackStack::<Rc<dyn NavKey>>::restore(v),
    );
    remember_saveable_with(&saver, move || NavBackStack::new(elements))
}

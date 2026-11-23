//! Remembering helpers, as Compose's internal RememberUtils.kt (adaptive-layout 1.2.0): a value remembered (and
//! saved) per key ([`remember_persistently_with_key`], its map and entry savers) and a remembered mutable reference
//! ([`remember_ref`], [`remember_updated_ref`], [`RefHolder`]).

use kompoz_runtime::{SavedValue, Saver, list_saver, remember, remember_saveable_with};
use std::cell::RefCell;
use std::rc::Rc;

/// A map of keys to values that keeps the order they came in (Kotlin's `mutableMapOf`, a LinkedHashMap): what
/// [`remember_persistently_with_key`] remembers and saves. A shared handle.
pub(crate) type PersistentMap<K, V> = Rc<RefCell<Vec<(K, V)>>>;

/// The value of `key`, made by `calculation` the first time the key is asked for, remembered for each key and
/// saved with the UI's state: the map saved by `key_saver` and `value_saver` (Compose's internal inline composable
/// `rememberPersistentlyWithKey`). Compose's defaults for both savers are `autoSaver()`; its only caller
/// (`rememberPaneExpansionState`) gives both, as Rust callers do here.
#[track_caller]
pub(crate) fn remember_persistently_with_key<K: PartialEq + Clone + 'static, V: Clone + 'static>(
    key: K,
    key_saver: Saver<K>,
    value_saver: Saver<V>,
    calculation: impl FnOnce() -> V,
) -> V {
    let map: PersistentMap<K, V> =
        remember_saveable_with(&map_saver(key_saver, value_saver), || Rc::new(RefCell::new(Vec::new())));
    // (Kotlin's `getOrPut`)
    if let Some((_, value)) = map.borrow().iter().find(|(k, _)| *k == key) {
        return value.clone();
    }
    let value = calculation();
    map.borrow_mut().push((key, value.clone()));
    value
}

/// The map saved as a list of its entries (Compose's private `MapSaver`); restored in the same order.
fn map_saver<K: Clone + 'static, V: Clone + 'static>(
    key_saver: Saver<K>,
    value_saver: Saver<V>,
) -> Saver<PersistentMap<K, V>> {
    let entry_saver = map_entry_saver(key_saver, value_saver);
    let restore_entry = entry_saver.clone();
    list_saver(
        move |map: &PersistentMap<K, V>| map.borrow().iter().map(|entry| entry_saver.save(entry)).collect(),
        move |entries| {
            let mut map = Vec::with_capacity(entries.len());
            for entry in entries {
                // (Compose's `!!`: an entry that doesn't restore fails)
                map.push(restore_entry.restore(entry).expect("a saved pane expansion entry"));
            }
            Some(Rc::new(RefCell::new(map)))
        },
    )
}

/// An entry saved as the list of its key and value (Compose's private `MapEntrySaver`).
fn map_entry_saver<K: Clone + 'static, V: Clone + 'static>(
    key_saver: Saver<K>,
    value_saver: Saver<V>,
) -> Saver<(K, V)> {
    let (restore_key, restore_value) = (key_saver.clone(), value_saver.clone());
    list_saver(
        move |(key, value): &(K, V)| vec![key_saver.save(key), value_saver.save(value)],
        move |items: &[SavedValue]| {
            let key = restore_key.restore(items.first()?)?;
            let value = restore_value.restore(items.get(1)?)?;
            Some((key, value))
        },
    )
}

/// A reference that can be read and set anywhere, a remembered one keeping what was last set (Compose's internal
/// `RefHolder`, read and written as a delegated property there). A shared handle: its clones are the same holder.
pub(crate) struct RefHolder<T>(Rc<RefCell<T>>);

impl<T> Clone for RefHolder<T> {
    fn clone(&self) -> Self {
        RefHolder(self.0.clone())
    }
}

impl<T: Clone> RefHolder<T> {
    pub(crate) fn new(value: T) -> RefHolder<T> {
        RefHolder(Rc::new(RefCell::new(value)))
    }

    /// What it holds now (Kotlin's `getValue`).
    pub(crate) fn value(&self) -> T {
        self.0.borrow().clone()
    }

    /// Holds `value` from now on (Kotlin's `setValue`).
    pub(crate) fn set_value(&self, value: T) {
        *self.0.borrow_mut() = value;
    }
}

/// A holder remembered with `initial_value` (Compose's internal composable `rememberRef`).
#[track_caller]
pub(crate) fn remember_ref<T: Clone + 'static>(initial_value: T) -> RefHolder<T> {
    remember(move || RefHolder::new(initial_value))
}

/// A remembered holder set to `value` at each composition (Compose's internal composable `rememberUpdatedRef`): the
/// latest value, read where it is needed later, without a state's reads and writes.
#[track_caller]
pub(crate) fn remember_updated_ref<T: Clone + 'static>(value: T) -> RefHolder<T> {
    let holder = remember_ref(value.clone());
    holder.set_value(value);
    holder
}

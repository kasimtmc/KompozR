//! Observable collections: `mutable_state_list_of`, `mutable_state_map_of`.

use crate::state::{MutableState, never_equal_policy};
use std::collections::BTreeMap;

/// An observable list (Compose's SnapshotStateList): reading it subscribes, changing it recomposes the readers.
pub struct SnapshotStateList<T> {
    state: MutableState<Vec<T>>,
}

impl<T> Clone for SnapshotStateList<T> {
    fn clone(&self) -> Self {
        SnapshotStateList { state: self.state.clone() }
    }
}

impl<T: Clone + Send + Sync + 'static> SnapshotStateList<T> {
    pub fn new(items: Vec<T>) -> Self {
        SnapshotStateList { state: MutableState::new(items, never_equal_policy()) }
    }

    /// The items (a copy).
    pub fn to_vec(&self) -> Vec<T> {
        self.state.get()
    }
    /// Runs `f` with the items, without copying them.
    pub fn with<R>(&self, f: impl FnOnce(&[T]) -> R) -> R {
        self.state.with(|v| f(v))
    }
    pub fn len(&self) -> usize {
        self.with(|v| v.len())
    }
    pub fn is_empty(&self) -> bool {
        self.with(|v| v.is_empty())
    }
    pub fn get(&self, index: usize) -> Option<T> {
        self.with(|v| v.get(index).cloned())
    }

    pub fn add(&self, item: T) {
        self.state.update(|v| v.push(item));
    }
    /// Inserts at `index` (at the end if it is past it).
    pub fn add_at(&self, index: usize, item: T) {
        self.state.update(|v| v.insert(index.min(v.len()), item));
    }
    pub fn set(&self, index: usize, item: T) {
        self.state.update(|v| v[index] = item);
    }
    pub fn remove_at(&self, index: usize) -> T {
        let mut removed = None;
        self.state.update(|v| removed = Some(v.remove(index)));
        removed.unwrap()
    }
    pub fn retain(&self, keep: impl FnMut(&T) -> bool) {
        self.state.update(|v| v.retain(keep));
    }
    pub fn clear(&self) {
        self.state.update(|v| v.clear());
    }
    /// Changes the items in place: `list.update(|v| v.sort())`.
    pub fn update(&self, mutate: impl FnOnce(&mut Vec<T>)) {
        self.state.update(mutate);
    }
}

/// The same list, not the same items.
impl<T> PartialEq for SnapshotStateList<T> {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
    }
}

/// `mutableStateListOf(…)`.
pub fn mutable_state_list_of<T: Clone + Send + Sync + 'static>(
    items: impl IntoIterator<Item = T>,
) -> SnapshotStateList<T> {
    SnapshotStateList::new(items.into_iter().collect())
}

/// An observable map (Compose's SnapshotStateMap): reading it subscribes, changing it recomposes the readers.
/// Its entries go in key order.
pub struct SnapshotStateMap<K, V> {
    state: MutableState<BTreeMap<K, V>>,
}

impl<K, V> Clone for SnapshotStateMap<K, V> {
    fn clone(&self) -> Self {
        SnapshotStateMap { state: self.state.clone() }
    }
}

impl<K: Ord + Clone + Send + Sync + 'static, V: Clone + Send + Sync + 'static> SnapshotStateMap<K, V> {
    pub fn new(entries: BTreeMap<K, V>) -> Self {
        SnapshotStateMap { state: MutableState::new(entries, never_equal_policy()) }
    }

    pub fn with<R>(&self, f: impl FnOnce(&BTreeMap<K, V>) -> R) -> R {
        self.state.with(f)
    }
    /// The entries (a copy).
    pub fn to_map(&self) -> BTreeMap<K, V> {
        self.state.get()
    }
    pub fn len(&self) -> usize {
        self.with(|m| m.len())
    }
    pub fn is_empty(&self) -> bool {
        self.with(|m| m.is_empty())
    }
    pub fn contains_key(&self, key: &K) -> bool {
        self.with(|m| m.contains_key(key))
    }
    /// The value for `key` (Compose's `map[key]`).
    pub fn get(&self, key: &K) -> Option<V> {
        self.with(|m| m.get(key).cloned())
    }
    pub fn get_or_default(&self, key: &K, otherwise: V) -> V {
        self.get(key).unwrap_or(otherwise)
    }

    /// Sets `key` to `value` (Compose's `map[key] = value`); the value it had before. The same value again:
    /// nobody recomposes.
    pub fn put(&self, key: K, value: V) -> Option<V>
    where
        V: PartialEq,
    {
        if let Some(old) = self.state.peek_with(|m| m.get(&key).cloned())
            && old == value
        {
            return Some(old);
        }
        let mut before = None;
        self.state.update(|m| before = m.insert(key, value));
        before
    }
    pub fn put_all(&self, more: impl IntoIterator<Item = (K, V)>) {
        self.state.update(|m| m.extend(more));
    }
    /// Takes `key` out; the value it had. Nothing there: nobody recomposes.
    pub fn remove(&self, key: &K) -> Option<V> {
        if !self.state.peek_with(|m| m.contains_key(key)) {
            return None;
        }
        let mut was = None;
        self.state.update(|m| was = m.remove(key));
        was
    }
    pub fn clear(&self) {
        self.state.update(|m| m.clear());
    }
    /// Changes the entries in place.
    pub fn update(&self, mutate: impl FnOnce(&mut BTreeMap<K, V>)) {
        self.state.update(mutate);
    }
}

/// The same map, not the same entries.
impl<K, V> PartialEq for SnapshotStateMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
    }
}

/// `mutableStateMapOf(…)`.
pub fn mutable_state_map_of<K: Ord + Clone + Send + Sync + 'static, V: Clone + Send + Sync + 'static>(
    entries: impl IntoIterator<Item = (K, V)>,
) -> SnapshotStateMap<K, V> {
    SnapshotStateMap::new(entries.into_iter().collect())
}

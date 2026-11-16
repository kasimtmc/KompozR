//! Kotlin's collections: `listOf`/`setOf`/`mapOf` and their kin, Kotlin's insertion-ordered sets and maps
//! ([`KtSet`], [`KtMap`]), and the functions of `Iterable` ([`KtList`]), `MutableList` ([`KtMutableList`]) and `Map`
//! ([`KtMapExt`]) as extension traits.
//!
//! Lambdas get the elements themselves (clones), as Kotlin's see them, not borrows: `list.filter(|it| it > 2)`.
//! Kotlin's `Int` results (sizes, indices, counts) are `i32`; an index out of range panics as Kotlin throws.

use super::range::{IndexedValue, KtContains, KtProgression, KtStep, kt_compare};
use super::text::{KtToString, to_string};
use super::{KtSum, panic_kt};
use crate::{SnapshotStateList, SnapshotStateMap};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::hash::{BuildHasher, Hash};

// ---- Constructors ----

/// Kotlin's `listOf(a, b, c)` (its varargs as an array, or anything iterable): a `List` is a `Vec`.
/// Kotlin: `listOf(vararg elements: T): List<T>`
pub fn list_of<I: IntoIterator>(elements: I) -> Vec<I::Item> {
    elements.into_iter().collect()
}
/// Kotlin: `mutableListOf(vararg elements: T): MutableList<T>`
pub fn mutable_list_of<I: IntoIterator>(elements: I) -> Vec<I::Item> {
    elements.into_iter().collect()
}
/// Kotlin: `arrayListOf(vararg elements: T): ArrayList<T>`
pub fn array_list_of<I: IntoIterator>(elements: I) -> Vec<I::Item> {
    elements.into_iter().collect()
}
/// Kotlin's `listOfNotNull(a, b)`: the elements that aren't null.
/// Kotlin: `listOfNotNull(vararg elements: T?): List<T>`
pub fn list_of_not_null<T, I: IntoIterator<Item = Option<T>>>(elements: I) -> Vec<T> {
    elements.into_iter().flatten().collect()
}
/// Kotlin: `emptyList(): List<T>`
pub fn empty_list<T>() -> Vec<T> {
    Vec::new()
}
/// Kotlin's `List(size) { i -> … }`: each element from its index.
/// Kotlin: `List(size: Int, init: (Int) -> T): List<T>`
#[allow(non_snake_case)]
pub fn List<T>(size: i32, init: impl FnMut(i32) -> T) -> Vec<T> {
    (0..size.max(0)).map(init).collect()
}
/// Kotlin: `MutableList(size: Int, init: (Int) -> T): MutableList<T>`
#[allow(non_snake_case)]
pub fn MutableList<T>(size: i32, init: impl FnMut(i32) -> T) -> Vec<T> {
    List(size, init)
}
/// Kotlin: `intArrayOf(vararg elements: Int): IntArray`
pub fn int_array_of<I: IntoIterator<Item = i32>>(elements: I) -> Vec<i32> {
    elements.into_iter().collect()
}
/// Kotlin: `floatArrayOf(vararg elements: Float): FloatArray`
pub fn float_array_of<I: IntoIterator<Item = f32>>(elements: I) -> Vec<f32> {
    elements.into_iter().collect()
}
/// Kotlin: `setOf(vararg elements: T): Set<T>`
pub fn set_of<T: PartialEq, I: IntoIterator<Item = T>>(elements: I) -> KtSet<T> {
    elements.into_iter().collect()
}
/// Kotlin: `mutableSetOf(vararg elements: T): MutableSet<T>`
pub fn mutable_set_of<T: PartialEq, I: IntoIterator<Item = T>>(elements: I) -> KtSet<T> {
    elements.into_iter().collect()
}
/// Kotlin: `emptySet(): Set<T>`
pub fn empty_set<T>() -> KtSet<T> {
    KtSet { items: Vec::new() }
}
/// Kotlin's `mapOf(k to v, …)`: its entries in the order given (a later one of a key replaces the earlier's value).
/// Kotlin: `mapOf(vararg pairs: Pair<K, V>): Map<K, V>`
pub fn map_of<K: PartialEq, V, I: IntoIterator<Item = (K, V)>>(pairs: I) -> KtMap<K, V> {
    pairs.into_iter().collect()
}
/// Kotlin: `mutableMapOf(vararg pairs: Pair<K, V>): MutableMap<K, V>`
pub fn mutable_map_of<K: PartialEq, V, I: IntoIterator<Item = (K, V)>>(pairs: I) -> KtMap<K, V> {
    pairs.into_iter().collect()
}
/// Kotlin: `emptyMap(): Map<K, V>`
pub fn empty_map<K, V>() -> KtMap<K, V> {
    KtMap { entries: Vec::new() }
}
/// Kotlin's `a to b`: a pair.
pub fn to<A, B>(first: A, second: B) -> (A, B) {
    (first, second)
}
/// Kotlin's `Pair(a, b)`.
#[allow(non_snake_case)]
pub fn Pair<A, B>(first: A, second: B) -> (A, B) {
    (first, second)
}
/// Kotlin's `Triple(a, b, c)`.
#[allow(non_snake_case)]
pub fn Triple<A, B, C>(first: A, second: B, third: C) -> (A, B, C) {
    (first, second, third)
}

// ---- Destructuring: val (a, b) = pair ----

/// `component1()` … `component5()` of a tuple (Kotlin's `Pair`, `Triple`, a map's entry): what a destructuring
/// declaration reads.
pub trait KtComponents {
    type C1;
    type C2;
    fn component1(&self) -> Self::C1;
    fn component2(&self) -> Self::C2;
}

impl<A: Clone, B: Clone> KtComponents for (A, B) {
    type C1 = A;
    type C2 = B;
    fn component1(&self) -> A {
        self.0.clone()
    }
    fn component2(&self) -> B {
        self.1.clone()
    }
}

impl<A: Clone, B: Clone, C: Clone> KtComponents for (A, B, C) {
    type C1 = A;
    type C2 = B;
    fn component1(&self) -> A {
        self.0.clone()
    }
    fn component2(&self) -> B {
        self.1.clone()
    }
}

/// The third part of a triple.
pub trait KtComponent3 {
    type C3;
    fn component3(&self) -> Self::C3;
}
impl<A, B, C: Clone> KtComponent3 for (A, B, C) {
    type C3 = C;
    fn component3(&self) -> C {
        self.2.clone()
    }
}

/// A pair's parts by Kotlin's names (`pair.first`, `pair.second`; a triple's `third`).
pub trait KtPair {
    type A;
    type B;
    fn first(&self) -> Self::A;
    fn second(&self) -> Self::B;
}
impl<A: Clone, B: Clone> KtPair for (A, B) {
    type A = A;
    type B = B;
    fn first(&self) -> A {
        self.0.clone()
    }
    fn second(&self) -> B {
        self.1.clone()
    }
}
impl<A: Clone, B: Clone, C> KtPair for (A, B, C) {
    type A = A;
    type B = B;
    fn first(&self) -> A {
        self.0.clone()
    }
    fn second(&self) -> B {
        self.1.clone()
    }
}
/// A triple's `third`.
pub trait KtTriple {
    type C;
    fn third(&self) -> Self::C;
}
impl<A, B, C: Clone> KtTriple for (A, B, C) {
    type C = C;
    fn third(&self) -> C {
        self.2.clone()
    }
}

// ---- KtSet, KtMap: Kotlin's LinkedHashSet and LinkedHashMap ----

/// Kotlin's `Set` (`setOf`, a `LinkedHashSet`): unique elements in the order they were added; equal to another
/// with the same elements in any order. Asks only `PartialEq` of them (a set is small in a UI; floats and data
/// classes go in).
#[derive(Clone, Debug)]
pub struct KtSet<T> {
    items: Vec<T>,
}

impl<T> Default for KtSet<T> {
    fn default() -> Self {
        KtSet { items: Vec::new() }
    }
}

impl<T: PartialEq> KtSet<T> {
    pub fn new() -> KtSet<T> {
        KtSet { items: Vec::new() }
    }
    /// Adds it unless it is there: whether it was added.
    pub fn add(&mut self, element: T) -> bool {
        if self.items.as_slice().contains(&element) {
            return false;
        }
        self.items.push(element);
        true
    }
    /// Whether it was there.
    pub fn remove(&mut self, element: T) -> bool {
        match self.items.iter().position(|e| *e == element) {
            Some(i) => {
                self.items.remove(i);
                true
            }
            None => false,
        }
    }
    pub fn contains(&self, element: T) -> bool {
        self.items.as_slice().contains(&element)
    }
}

impl<T> KtSet<T> {
    pub fn size(&self) -> i32 {
        self.items.len() as i32
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn clear(&mut self) {
        self.items.clear();
    }
    pub fn as_slice(&self) -> &[T] {
        &self.items
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }
}

impl<T: PartialEq> PartialEq for KtSet<T> {
    fn eq(&self, other: &Self) -> bool {
        self.items.len() == other.items.len() && self.items.iter().all(|e| other.items.as_slice().contains(e))
    }
}

impl<T: PartialEq> FromIterator<T> for KtSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = KtSet::new();
        for e in iter {
            set.add(e);
        }
        set
    }
}

impl<T: PartialEq> Extend<T> for KtSet<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for e in iter {
            self.add(e);
        }
    }
}

impl<T> IntoIterator for KtSet<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a KtSet<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

/// Kotlin's `Map` (`mapOf`, a `LinkedHashMap`): its entries in the order their keys were first put; equal to
/// another with the same entries in any order. Asks only `PartialEq` of its keys.
#[derive(Clone, Debug)]
pub struct KtMap<K, V> {
    entries: Vec<(K, V)>,
}

impl<K, V> Default for KtMap<K, V> {
    fn default() -> Self {
        KtMap { entries: Vec::new() }
    }
}

impl<K, V> KtMap<K, V> {
    /// Its entries, in order.
    pub fn as_slice(&self) -> &[(K, V)] {
        &self.entries
    }
    pub fn size(&self) -> i32 {
        self.entries.len() as i32
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }
}

impl<K: PartialEq, V> KtMap<K, V> {
    pub fn new() -> KtMap<K, V> {
        KtMap { entries: Vec::new() }
    }
    fn position(&self, key: &K) -> Option<usize> {
        self.entries.iter().position(|(k, _)| k == key)
    }
    /// Sets the key's value (keeping its place if it has one): the value it had.
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        match self.position(&key) {
            Some(i) => Some(std::mem::replace(&mut self.entries[i].1, value)),
            None => {
                self.entries.push((key, value));
                None
            }
        }
    }
    /// Kotlin's `map[key] = value`.
    pub fn set(&mut self, key: K, value: V) {
        self.put(key, value);
    }
    /// The value it had.
    pub fn remove(&mut self, key: K) -> Option<V> {
        self.position(&key).map(|i| self.entries.remove(i).1)
    }
    pub fn contains_key(&self, key: K) -> bool {
        self.position(&key).is_some()
    }
    /// The key's value, borrowed.
    pub fn get_ref(&self, key: &K) -> Option<&V> {
        self.position(key).map(|i| &self.entries[i].1)
    }
    pub fn put_all(&mut self, entries: impl IntoIterator<Item = (K, V)>) {
        for (k, v) in entries {
            self.put(k, v);
        }
    }
}

impl<K: PartialEq, V: Clone> KtMap<K, V> {
    /// Kotlin's `map[key]`: its value, or null.
    pub fn get(&self, key: K) -> Option<V> {
        self.get_ref(&key).cloned()
    }
    /// Kotlin's `getOrPut(key) { … }`: its value, else `default`'s, put first.
    pub fn get_or_put(&mut self, key: K, default: impl FnOnce() -> V) -> V {
        match self.position(&key) {
            Some(i) => self.entries[i].1.clone(),
            None => {
                let v = default();
                self.entries.push((key, v.clone()));
                v
            }
        }
    }
}

impl<K: PartialEq, V: PartialEq> PartialEq for KtMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.entries.len() == other.entries.len()
            && self.entries.iter().all(|(k, v)| other.get_ref(k).is_some_and(|o| o == v))
    }
}

impl<K: PartialEq, V> FromIterator<(K, V)> for KtMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = KtMap::new();
        map.put_all(iter);
        map
    }
}

impl<K: PartialEq, V> Extend<(K, V)> for KtMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        self.put_all(iter);
    }
}

impl<K, V> IntoIterator for KtMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

// ---- Iterable ----

/// What Kotlin's `Iterable` functions ([`KtList`]) go over: its elements as clones, in order.
pub trait KtIterable {
    type Item: Clone;
    fn kt_iter(&self) -> impl Iterator<Item = Self::Item>;
}

impl<T: Clone> KtIterable for [T] {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.iter().cloned()
    }
}
impl<T: Clone, const N: usize> KtIterable for [T; N] {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.iter().cloned()
    }
}
impl<T: Clone> KtIterable for Vec<T> {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.iter().cloned()
    }
}
impl<T: Clone> KtIterable for VecDeque<T> {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.iter().cloned()
    }
}
impl<T: Clone> KtIterable for KtSet<T> {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.items.iter().cloned()
    }
}
impl<T: Clone> KtIterable for BTreeSet<T> {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.iter().cloned()
    }
}
impl<T: Clone, S> KtIterable for HashSet<T, S> {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.iter().cloned()
    }
}
impl<T: KtStep> KtIterable for KtProgression<T> {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.iter()
    }
}
/// Its items as they are when asked (a read that subscribes).
impl<T: Clone + Send + Sync + 'static> KtIterable for SnapshotStateList<T> {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.to_vec().into_iter()
    }
}
impl<C: KtIterable + ?Sized> KtIterable for &C {
    type Item = C::Item;
    fn kt_iter(&self) -> impl Iterator<Item = C::Item> {
        (**self).kt_iter()
    }
}

impl<T: PartialEq> KtContains<T> for [T] {
    fn kt_contains(&self, value: &T) -> bool {
        self.contains(value)
    }
}
impl<T: PartialEq> KtContains<T> for Vec<T> {
    fn kt_contains(&self, value: &T) -> bool {
        self.as_slice().contains(value)
    }
}
impl<T: PartialEq, const N: usize> KtContains<T> for [T; N] {
    fn kt_contains(&self, value: &T) -> bool {
        self.as_slice().contains(value)
    }
}
impl<T: PartialEq> KtContains<T> for KtSet<T> {
    fn kt_contains(&self, value: &T) -> bool {
        self.items.as_slice().contains(value)
    }
}
impl<T: Ord> KtContains<T> for BTreeSet<T> {
    fn kt_contains(&self, value: &T) -> bool {
        self.contains(value)
    }
}
impl<T: Eq + Hash, S: BuildHasher> KtContains<T> for HashSet<T, S> {
    fn kt_contains(&self, value: &T) -> bool {
        self.contains(value)
    }
}
impl<T: PartialEq + Clone + Send + Sync + 'static> KtContains<T> for SnapshotStateList<T> {
    fn kt_contains(&self, value: &T) -> bool {
        self.with(|items| items.contains(value))
    }
}
/// `key in map`.
impl<K: PartialEq, V> KtContains<K> for KtMap<K, V> {
    fn kt_contains(&self, key: &K) -> bool {
        self.position(key).is_some()
    }
}
impl<K: Ord, V> KtContains<K> for BTreeMap<K, V> {
    fn kt_contains(&self, key: &K) -> bool {
        self.contains_key(key)
    }
}
impl<K: Eq + Hash, V, S: BuildHasher> KtContains<K> for HashMap<K, V, S> {
    fn kt_contains(&self, key: &K) -> bool {
        self.contains_key(key)
    }
}

/// An index Kotlin is given (`list[i]`), checked as Kotlin checks it.
#[track_caller]
fn checked_index(index: i32, len: usize) -> usize {
    if index < 0 || index as usize >= len {
        panic_kt("IndexOutOfBoundsException", &format!("Index {index} out of bounds for length {len}"));
    }
    index as usize
}

/// The greater by Kotlin's `compareTo` (a NaN the greatest); on a tie the first, as Kotlin's `maxOf`.
fn greater<T: PartialOrd>(a: T, b: T) -> T {
    if kt_compare(&a, &b) == Ordering::Less { b } else { a }
}
fn lesser<T: PartialOrd>(a: T, b: T) -> T {
    if kt_compare(&b, &a) == Ordering::Less { b } else { a }
}

/// Kotlin's `Iterable` (and `List`) functions over anything [`KtIterable`]. Where Kotlin overloads a name with
/// and without a predicate, the predicate's is `…_that` (`any()`, `any_that { … }`); a Rust method of the same
/// name on the type comes first (`Vec::is_empty`, `Vec::remove`), so a caller that must have Kotlin's calls
/// through the trait (`KtList::contains(&list, x)`).
pub trait KtList: KtIterable {
    /// Kotlin: `size: Int`
    fn size(&self) -> i32 {
        self.kt_iter().count() as i32
    }
    fn is_empty(&self) -> bool {
        self.kt_iter().next().is_none()
    }
    fn is_not_empty(&self) -> bool {
        !KtList::is_empty(self)
    }
    /// Kotlin: `indices: IntRange`
    fn indices(&self) -> KtProgression<i32> {
        super::range::until(0, self.size())
    }
    /// Kotlin: `lastIndex: Int`
    fn last_index(&self) -> i32 {
        self.size() - 1
    }
    /// Kotlin's `list[index]`, `list.get(index)`.
    /// Kotlin: `get(index: Int): T`
    #[track_caller]
    fn get(&self, index: i32) -> Self::Item {
        let n = self.size() as usize;
        let i = checked_index(index, n);
        self.kt_iter().nth(i).expect("within its size")
    }
    /// Kotlin: `getOrNull(index: Int): T?`
    fn get_or_null(&self, index: i32) -> Option<Self::Item> {
        if index < 0 { None } else { self.kt_iter().nth(index as usize) }
    }
    /// Kotlin: `getOrElse(index: Int, defaultValue: (Int) -> T): T`
    fn get_or_else(&self, index: i32, default_value: impl FnOnce(i32) -> Self::Item) -> Self::Item {
        self.get_or_null(index).unwrap_or_else(|| default_value(index))
    }
    /// Kotlin: `elementAt(index: Int): T`
    #[track_caller]
    fn element_at(&self, index: i32) -> Self::Item {
        self.get(index)
    }

    /// Kotlin: `first(): T`
    #[track_caller]
    fn first(&self) -> Self::Item {
        self.kt_iter().next().unwrap_or_else(|| panic_kt("NoSuchElementException", "List is empty."))
    }
    /// Kotlin: `first(predicate: (T) -> Boolean): T`
    #[track_caller]
    fn first_that(&self, mut predicate: impl FnMut(Self::Item) -> bool) -> Self::Item {
        self.kt_iter().find(|v| predicate(v.clone())).unwrap_or_else(|| {
            panic_kt("NoSuchElementException", "Collection contains no element matching the predicate.")
        })
    }
    /// Kotlin: `last(): T`
    #[track_caller]
    fn last(&self) -> Self::Item {
        self.kt_iter().last().unwrap_or_else(|| panic_kt("NoSuchElementException", "List is empty."))
    }
    /// Kotlin: `last(predicate: (T) -> Boolean): T`
    #[track_caller]
    fn last_that(&self, mut predicate: impl FnMut(Self::Item) -> bool) -> Self::Item {
        self.kt_iter()
            .filter(|v| predicate(v.clone()))
            .last()
            .unwrap_or_else(|| panic_kt("NoSuchElementException", "List contains no element matching the predicate."))
    }
    /// Kotlin: `firstOrNull(): T?`
    fn first_or_null(&self) -> Option<Self::Item> {
        self.kt_iter().next()
    }
    /// Kotlin: `firstOrNull(predicate: (T) -> Boolean): T?`
    fn first_or_null_that(&self, mut predicate: impl FnMut(Self::Item) -> bool) -> Option<Self::Item> {
        self.kt_iter().find(|v| predicate(v.clone()))
    }
    /// Kotlin: `lastOrNull(): T?`
    fn last_or_null(&self) -> Option<Self::Item> {
        self.kt_iter().last()
    }
    /// Kotlin: `lastOrNull(predicate: (T) -> Boolean): T?`
    fn last_or_null_that(&self, mut predicate: impl FnMut(Self::Item) -> bool) -> Option<Self::Item> {
        self.kt_iter().filter(|v| predicate(v.clone())).last()
    }
    /// Kotlin: `find(predicate: (T) -> Boolean): T?`
    fn find(&self, predicate: impl FnMut(Self::Item) -> bool) -> Option<Self::Item> {
        self.first_or_null_that(predicate)
    }
    /// Kotlin: `findLast(predicate: (T) -> Boolean): T?`
    fn find_last(&self, predicate: impl FnMut(Self::Item) -> bool) -> Option<Self::Item> {
        self.last_or_null_that(predicate)
    }
    /// Kotlin: `single(): T`
    #[track_caller]
    fn single(&self) -> Self::Item {
        let mut it = self.kt_iter();
        let Some(first) = it.next() else { panic_kt("NoSuchElementException", "List is empty.") };
        if it.next().is_some() {
            panic_kt("IllegalArgumentException", "List has more than one element.");
        }
        first
    }
    /// Kotlin: `singleOrNull(): T?`
    fn single_or_null(&self) -> Option<Self::Item> {
        let mut it = self.kt_iter();
        let first = it.next()?;
        if it.next().is_some() { None } else { Some(first) }
    }

    /// Kotlin: `contains(element: T): Boolean`
    fn contains(&self, element: Self::Item) -> bool
    where
        Self::Item: PartialEq,
    {
        self.kt_iter().any(|v| v == element)
    }
    /// Kotlin: `indexOf(element: T): Int`
    fn index_of(&self, element: Self::Item) -> i32
    where
        Self::Item: PartialEq,
    {
        self.kt_iter().position(|v| v == element).map_or(-1, |i| i as i32)
    }
    /// Kotlin: `lastIndexOf(element: T): Int`
    fn last_index_of(&self, element: Self::Item) -> i32
    where
        Self::Item: PartialEq,
    {
        let mut found = -1;
        for (i, v) in self.kt_iter().enumerate() {
            if v == element {
                found = i as i32;
            }
        }
        found
    }
    /// Kotlin: `indexOfFirst(predicate: (T) -> Boolean): Int`
    fn index_of_first(&self, predicate: impl FnMut(Self::Item) -> bool) -> i32 {
        self.kt_iter().position(predicate).map_or(-1, |i| i as i32)
    }
    /// Kotlin: `indexOfLast(predicate: (T) -> Boolean): Int`
    fn index_of_last(&self, mut predicate: impl FnMut(Self::Item) -> bool) -> i32 {
        let mut found = -1;
        for (i, v) in self.kt_iter().enumerate() {
            if predicate(v) {
                found = i as i32;
            }
        }
        found
    }

    /// Kotlin: `forEach(action: (T) -> Unit)`
    fn for_each(&self, action: impl FnMut(Self::Item)) {
        self.kt_iter().for_each(action);
    }
    /// Kotlin: `forEachIndexed(action: (Int, T) -> Unit)`
    fn for_each_indexed(&self, mut action: impl FnMut(i32, Self::Item)) {
        for (i, v) in self.kt_iter().enumerate() {
            action(i as i32, v);
        }
    }
    /// Kotlin: `onEach(action: (T) -> Unit): List<T>`
    fn on_each(&self, mut action: impl FnMut(Self::Item)) -> Vec<Self::Item> {
        self.kt_iter().inspect(|v| action(v.clone())).collect()
    }

    /// Kotlin: `map(transform: (T) -> R): List<R>`
    fn map<R>(&self, transform: impl FnMut(Self::Item) -> R) -> Vec<R> {
        self.kt_iter().map(transform).collect()
    }
    /// Kotlin: `mapIndexed(transform: (Int, T) -> R): List<R>`
    fn map_indexed<R>(&self, mut transform: impl FnMut(i32, Self::Item) -> R) -> Vec<R> {
        self.kt_iter().enumerate().map(|(i, v)| transform(i as i32, v)).collect()
    }
    /// Kotlin: `mapNotNull(transform: (T) -> R?): List<R>`
    fn map_not_null<R>(&self, transform: impl FnMut(Self::Item) -> Option<R>) -> Vec<R> {
        self.kt_iter().filter_map(transform).collect()
    }
    /// Kotlin: `flatMap(transform: (T) -> Iterable<R>): List<R>`
    fn flat_map<I: IntoIterator>(&self, transform: impl FnMut(Self::Item) -> I) -> Vec<I::Item> {
        self.kt_iter().flat_map(transform).collect()
    }
    /// Kotlin: `filter(predicate: (T) -> Boolean): List<T>`
    fn filter(&self, mut predicate: impl FnMut(Self::Item) -> bool) -> Vec<Self::Item> {
        self.kt_iter().filter(|v| predicate(v.clone())).collect()
    }
    /// Kotlin: `filterNot(predicate: (T) -> Boolean): List<T>`
    fn filter_not(&self, mut predicate: impl FnMut(Self::Item) -> bool) -> Vec<Self::Item> {
        self.kt_iter().filter(|v| !predicate(v.clone())).collect()
    }
    /// Kotlin: `filterIndexed(predicate: (Int, T) -> Boolean): List<T>`
    fn filter_indexed(&self, mut predicate: impl FnMut(i32, Self::Item) -> bool) -> Vec<Self::Item> {
        self.kt_iter().enumerate().filter(|(i, v)| predicate(*i as i32, v.clone())).map(|(_, v)| v).collect()
    }
    /// Kotlin: `withIndex(): Iterable<IndexedValue<T>>`
    fn with_index(&self) -> Vec<IndexedValue<Self::Item>> {
        self.kt_iter().enumerate().map(|(i, value)| IndexedValue { index: i as i32, value }).collect()
    }

    /// Kotlin: `any(): Boolean`
    fn any(&self) -> bool {
        self.kt_iter().next().is_some()
    }
    /// Kotlin: `any(predicate: (T) -> Boolean): Boolean`
    fn any_that(&self, predicate: impl FnMut(Self::Item) -> bool) -> bool {
        self.kt_iter().any(predicate)
    }
    /// Kotlin: `all(predicate: (T) -> Boolean): Boolean`
    fn all(&self, predicate: impl FnMut(Self::Item) -> bool) -> bool {
        self.kt_iter().all(predicate)
    }
    /// Kotlin: `none(): Boolean`
    fn none(&self) -> bool {
        self.kt_iter().next().is_none()
    }
    /// Kotlin: `none(predicate: (T) -> Boolean): Boolean`
    fn none_that(&self, predicate: impl FnMut(Self::Item) -> bool) -> bool {
        !self.kt_iter().any(predicate)
    }
    /// Kotlin: `count(): Int`
    fn count(&self) -> i32 {
        self.size()
    }
    /// Kotlin: `count(predicate: (T) -> Boolean): Int`
    fn count_that(&self, mut predicate: impl FnMut(Self::Item) -> bool) -> i32 {
        self.kt_iter().filter(|v| predicate(v.clone())).count() as i32
    }

    /// Kotlin: `sum(): T`
    fn sum(&self) -> Self::Item
    where
        Self::Item: KtSum,
    {
        self.kt_iter().fold(Self::Item::kt_zero(), KtSum::kt_add)
    }
    /// Kotlin: `sumOf(selector: (T) -> R): R`
    fn sum_of<R: KtSum>(&self, selector: impl FnMut(Self::Item) -> R) -> R {
        self.kt_iter().map(selector).fold(R::kt_zero(), KtSum::kt_add)
    }
    /// Kotlin: `average(): Double`
    fn average(&self) -> f64
    where
        Self::Item: Into<f64>,
    {
        let (mut total, mut n) = (0.0, 0);
        for v in self.kt_iter() {
            total += v.into();
            n += 1;
        }
        if n == 0 { f64::NAN } else { total / n as f64 }
    }
    /// Kotlin: `maxOf(selector: (T) -> R): R`
    #[track_caller]
    fn max_of<R: PartialOrd>(&self, selector: impl FnMut(Self::Item) -> R) -> R {
        self.max_of_or_null(selector).unwrap_or_else(|| panic_kt("NoSuchElementException", ""))
    }
    /// Kotlin: `minOf(selector: (T) -> R): R`
    #[track_caller]
    fn min_of<R: PartialOrd>(&self, selector: impl FnMut(Self::Item) -> R) -> R {
        self.min_of_or_null(selector).unwrap_or_else(|| panic_kt("NoSuchElementException", ""))
    }
    /// Kotlin: `maxOfOrNull(selector: (T) -> R): R?`
    fn max_of_or_null<R: PartialOrd>(&self, selector: impl FnMut(Self::Item) -> R) -> Option<R> {
        self.kt_iter().map(selector).reduce(greater)
    }
    /// Kotlin: `minOfOrNull(selector: (T) -> R): R?`
    fn min_of_or_null<R: PartialOrd>(&self, selector: impl FnMut(Self::Item) -> R) -> Option<R> {
        self.kt_iter().map(selector).reduce(lesser)
    }
    /// Kotlin: `maxOrNull(): T?`
    fn max_or_null(&self) -> Option<Self::Item>
    where
        Self::Item: PartialOrd,
    {
        self.kt_iter().reduce(greater)
    }
    /// Kotlin: `minOrNull(): T?`
    fn min_or_null(&self) -> Option<Self::Item>
    where
        Self::Item: PartialOrd,
    {
        self.kt_iter().reduce(lesser)
    }
    /// Kotlin: `max(): T`
    #[track_caller]
    fn max(&self) -> Self::Item
    where
        Self::Item: PartialOrd,
    {
        self.max_or_null().unwrap_or_else(|| panic_kt("NoSuchElementException", ""))
    }
    /// Kotlin: `min(): T`
    #[track_caller]
    fn min(&self) -> Self::Item
    where
        Self::Item: PartialOrd,
    {
        self.min_or_null().unwrap_or_else(|| panic_kt("NoSuchElementException", ""))
    }
    /// Kotlin: `maxByOrNull(selector: (T) -> R): T?`
    fn max_by_or_null<R: PartialOrd>(&self, mut selector: impl FnMut(Self::Item) -> R) -> Option<Self::Item> {
        let mut best: Option<(Self::Item, R)> = None;
        for v in self.kt_iter() {
            let k = selector(v.clone());
            if best.as_ref().is_none_or(|(_, b)| kt_compare(b, &k) == Ordering::Less) {
                best = Some((v, k));
            }
        }
        best.map(|(v, _)| v)
    }
    /// Kotlin: `minByOrNull(selector: (T) -> R): T?`
    fn min_by_or_null<R: PartialOrd>(&self, mut selector: impl FnMut(Self::Item) -> R) -> Option<Self::Item> {
        let mut best: Option<(Self::Item, R)> = None;
        for v in self.kt_iter() {
            let k = selector(v.clone());
            if best.as_ref().is_none_or(|(_, b)| kt_compare(&k, b) == Ordering::Less) {
                best = Some((v, k));
            }
        }
        best.map(|(v, _)| v)
    }
    /// Kotlin: `maxBy(selector: (T) -> R): T`
    #[track_caller]
    fn max_by<R: PartialOrd>(&self, selector: impl FnMut(Self::Item) -> R) -> Self::Item {
        self.max_by_or_null(selector).unwrap_or_else(|| panic_kt("NoSuchElementException", ""))
    }
    /// Kotlin: `minBy(selector: (T) -> R): T`
    #[track_caller]
    fn min_by<R: PartialOrd>(&self, selector: impl FnMut(Self::Item) -> R) -> Self::Item {
        self.min_by_or_null(selector).unwrap_or_else(|| panic_kt("NoSuchElementException", ""))
    }
    /// Kotlin: `fold(initial: R, operation: (R, T) -> R): R`
    fn fold<R>(&self, initial: R, operation: impl FnMut(R, Self::Item) -> R) -> R {
        self.kt_iter().fold(initial, operation)
    }
    /// Kotlin: `reduce(operation: (T, T) -> T): T`
    #[track_caller]
    fn reduce(&self, operation: impl FnMut(Self::Item, Self::Item) -> Self::Item) -> Self::Item {
        self.kt_iter()
            .reduce(operation)
            .unwrap_or_else(|| panic_kt("UnsupportedOperationException", "Empty collection can't be reduced."))
    }

    /// Kotlin: `sorted(): List<T>`
    fn sorted(&self) -> Vec<Self::Item>
    where
        Self::Item: PartialOrd,
    {
        let mut out: Vec<_> = self.kt_iter().collect();
        out.sort_by(kt_compare);
        out
    }
    /// Kotlin: `sortedDescending(): List<T>`
    fn sorted_descending(&self) -> Vec<Self::Item>
    where
        Self::Item: PartialOrd,
    {
        let mut out: Vec<_> = self.kt_iter().collect();
        out.sort_by(|a, b| kt_compare(b, a));
        out
    }
    /// Kotlin: `sortedBy(selector: (T) -> R?): List<T>`
    fn sorted_by<R: PartialOrd>(&self, mut selector: impl FnMut(Self::Item) -> R) -> Vec<Self::Item> {
        let mut keyed: Vec<(R, Self::Item)> = self.kt_iter().map(|v| (selector(v.clone()), v)).collect();
        keyed.sort_by(|a, b| kt_compare(&a.0, &b.0));
        keyed.into_iter().map(|(_, v)| v).collect()
    }
    /// Kotlin: `sortedByDescending(selector: (T) -> R?): List<T>`
    fn sorted_by_descending<R: PartialOrd>(&self, mut selector: impl FnMut(Self::Item) -> R) -> Vec<Self::Item> {
        let mut keyed: Vec<(R, Self::Item)> = self.kt_iter().map(|v| (selector(v.clone()), v)).collect();
        keyed.sort_by(|a, b| kt_compare(&b.0, &a.0));
        keyed.into_iter().map(|(_, v)| v).collect()
    }
    /// Kotlin: `sortedWith(comparator: Comparator<T>): List<T>`
    fn sorted_with(&self, mut comparator: impl FnMut(&Self::Item, &Self::Item) -> Ordering) -> Vec<Self::Item> {
        let mut out: Vec<_> = self.kt_iter().collect();
        out.sort_by(|a, b| comparator(a, b));
        out
    }
    /// Kotlin: `reversed(): List<T>`
    fn reversed(&self) -> Vec<Self::Item> {
        let mut out: Vec<_> = self.kt_iter().collect();
        out.reverse();
        out
    }
    /// Kotlin: `distinct(): List<T>`
    fn distinct(&self) -> Vec<Self::Item>
    where
        Self::Item: PartialEq,
    {
        let mut out: Vec<Self::Item> = Vec::new();
        for v in self.kt_iter() {
            if !out.as_slice().contains(&v) {
                out.push(v);
            }
        }
        out
    }
    /// Kotlin: `distinctBy(selector: (T) -> K): List<T>`
    fn distinct_by<K: PartialEq>(&self, mut selector: impl FnMut(Self::Item) -> K) -> Vec<Self::Item> {
        let mut keys: Vec<K> = Vec::new();
        let mut out = Vec::new();
        for v in self.kt_iter() {
            let k = selector(v.clone());
            if !keys.as_slice().contains(&k) {
                keys.push(k);
                out.push(v);
            }
        }
        out
    }
    /// Kotlin: `take(n: Int): List<T>`
    #[track_caller]
    fn take(&self, n: i32) -> Vec<Self::Item> {
        require_count(n);
        self.kt_iter().take(n as usize).collect()
    }
    /// Kotlin: `drop(n: Int): List<T>`
    #[track_caller]
    fn drop(&self, n: i32) -> Vec<Self::Item> {
        require_count(n);
        self.kt_iter().skip(n as usize).collect()
    }
    /// Kotlin: `takeLast(n: Int): List<T>`
    #[track_caller]
    fn take_last(&self, n: i32) -> Vec<Self::Item> {
        require_count(n);
        let all: Vec<_> = self.kt_iter().collect();
        let from = all.len().saturating_sub(n as usize);
        all[from..].to_vec()
    }
    /// Kotlin: `dropLast(n: Int): List<T>`
    #[track_caller]
    fn drop_last(&self, n: i32) -> Vec<Self::Item> {
        require_count(n);
        let mut all: Vec<_> = self.kt_iter().collect();
        all.truncate(all.len().saturating_sub(n as usize));
        all
    }
    /// Kotlin: `takeWhile(predicate: (T) -> Boolean): List<T>`
    fn take_while(&self, mut predicate: impl FnMut(Self::Item) -> bool) -> Vec<Self::Item> {
        self.kt_iter().take_while(|v| predicate(v.clone())).collect()
    }
    /// Kotlin: `dropWhile(predicate: (T) -> Boolean): List<T>`
    fn drop_while(&self, mut predicate: impl FnMut(Self::Item) -> bool) -> Vec<Self::Item> {
        self.kt_iter().skip_while(|v| predicate(v.clone())).collect()
    }
    /// Kotlin: `chunked(size: Int): List<List<T>>`
    #[track_caller]
    fn chunked(&self, size: i32) -> Vec<Vec<Self::Item>> {
        if size <= 0 {
            panic_kt("IllegalArgumentException", &format!("size {size} must be greater than zero."));
        }
        let all: Vec<_> = self.kt_iter().collect();
        all.chunks(size as usize).map(<[_]>::to_vec).collect()
    }
    /// Kotlin: `windowed(size: Int, step: Int = 1, partialWindows: Boolean = false): List<List<T>>`
    #[track_caller]
    fn windowed(&self, size: i32, step: i32, partial_windows: bool) -> Vec<Vec<Self::Item>> {
        if size <= 0 || step <= 0 {
            panic_kt(
                "IllegalArgumentException",
                &format!("size {size} and step {step} must both be greater than zero."),
            );
        }
        let all: Vec<_> = self.kt_iter().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < all.len() {
            let end = i + size as usize;
            if end > all.len() && !partial_windows {
                break;
            }
            out.push(all[i..end.min(all.len())].to_vec());
            i += step as usize;
        }
        out
    }
    /// Kotlin: `zip(other: Iterable<R>): List<Pair<T, R>>`
    fn zip<O: KtIterable + ?Sized>(&self, other: &O) -> Vec<(Self::Item, O::Item)> {
        self.kt_iter().zip(other.kt_iter()).collect()
    }
    /// Kotlin: `partition(predicate: (T) -> Boolean): Pair<List<T>, List<T>>`
    fn partition(&self, mut predicate: impl FnMut(Self::Item) -> bool) -> (Vec<Self::Item>, Vec<Self::Item>) {
        self.kt_iter().partition(|v| predicate(v.clone()))
    }
    /// Kotlin: `groupBy(keySelector: (T) -> K): Map<K, List<T>>`
    fn group_by<K: PartialEq>(&self, mut key_selector: impl FnMut(Self::Item) -> K) -> KtMap<K, Vec<Self::Item>> {
        let mut out: KtMap<K, Vec<Self::Item>> = KtMap::new();
        for v in self.kt_iter() {
            let k = key_selector(v.clone());
            match out.position(&k) {
                Some(i) => out.entries[i].1.push(v),
                None => out.entries.push((k, vec![v])),
            }
        }
        out
    }
    /// Kotlin: `associate(transform: (T) -> Pair<K, V>): Map<K, V>`
    fn associate<K: PartialEq, V>(&self, transform: impl FnMut(Self::Item) -> (K, V)) -> KtMap<K, V> {
        self.kt_iter().map(transform).collect()
    }
    /// Kotlin: `associateBy(keySelector: (T) -> K): Map<K, T>`
    fn associate_by<K: PartialEq>(&self, mut key_selector: impl FnMut(Self::Item) -> K) -> KtMap<K, Self::Item> {
        self.kt_iter().map(|v| (key_selector(v.clone()), v)).collect()
    }
    /// Kotlin: `associateWith(valueSelector: (T) -> V): Map<T, V>`
    fn associate_with<V>(&self, mut value_selector: impl FnMut(Self::Item) -> V) -> KtMap<Self::Item, V>
    where
        Self::Item: PartialEq,
    {
        self.kt_iter().map(|v| (v.clone(), value_selector(v))).collect()
    }
    /// Kotlin: `joinToString(separator: CharSequence = ", ", prefix: CharSequence = "", postfix: CharSequence = "", limit: Int = -1, truncated: CharSequence = "...", transform: ((T) -> CharSequence)? = null): String`
    fn join_to_string(
        &self,
        separator: impl AsRef<str>,
        prefix: impl AsRef<str>,
        postfix: impl AsRef<str>,
        limit: i32,
        truncated: impl AsRef<str>,
        transform: Option<&dyn Fn(Self::Item) -> String>,
    ) -> String
    where
        Self::Item: KtToString,
    {
        let mut out = prefix.as_ref().to_string();
        let mut count = 0;
        for v in self.kt_iter() {
            count += 1;
            if count > 1 {
                out.push_str(separator.as_ref());
            }
            if limit >= 0 && count > limit {
                out.push_str(truncated.as_ref());
                break;
            }
            match transform {
                Some(t) => out.push_str(&t(v)),
                None => v.kt_write(&mut out),
            }
        }
        out.push_str(postfix.as_ref());
        out
    }
    /// Kotlin: `ifEmpty(defaultValue: () -> List<T>): List<T>`
    fn if_empty(&self, default_value: impl FnOnce() -> Vec<Self::Item>) -> Vec<Self::Item> {
        let all: Vec<_> = self.kt_iter().collect();
        if all.is_empty() { default_value() } else { all }
    }
    /// Kotlin: `plus(elements: Iterable<T>): List<T>` (`list + other`)
    fn plus<O: KtIterable<Item = Self::Item> + ?Sized>(&self, elements: &O) -> Vec<Self::Item> {
        self.kt_iter().chain(elements.kt_iter()).collect()
    }
    /// Kotlin: `plus(element: T): List<T>` (`list + element`)
    fn plus_element(&self, element: Self::Item) -> Vec<Self::Item> {
        self.kt_iter().chain(std::iter::once(element)).collect()
    }
    /// Kotlin: `minus(element: T): List<T>` (`list - element`: the first equal one left out)
    fn minus_element(&self, element: Self::Item) -> Vec<Self::Item>
    where
        Self::Item: PartialEq,
    {
        let mut out: Vec<_> = self.kt_iter().collect();
        if let Some(i) = out.iter().position(|v| *v == element) {
            out.remove(i);
        }
        out
    }
    /// Kotlin: `minus(elements: Iterable<T>): List<T>` (`list - other`: every element of the other left out)
    fn minus<O: KtIterable<Item = Self::Item> + ?Sized>(&self, elements: &O) -> Vec<Self::Item>
    where
        Self::Item: PartialEq,
    {
        let gone: Vec<_> = elements.kt_iter().collect();
        self.kt_iter().filter(|v| !gone.as_slice().contains(v)).collect()
    }
    /// Kotlin: `toList(): List<T>`
    fn to_list(&self) -> Vec<Self::Item> {
        self.kt_iter().collect()
    }
    /// Kotlin: `toMutableList(): MutableList<T>`
    fn to_mutable_list(&self) -> Vec<Self::Item> {
        self.kt_iter().collect()
    }
    /// Kotlin: `toSet(): Set<T>`
    fn to_set(&self) -> KtSet<Self::Item>
    where
        Self::Item: PartialEq,
    {
        self.kt_iter().collect()
    }
    /// Kotlin: `toMutableSet(): MutableSet<T>`
    fn to_mutable_set(&self) -> KtSet<Self::Item>
    where
        Self::Item: PartialEq,
    {
        self.kt_iter().collect()
    }
}

impl<C: KtIterable + ?Sized> KtList for C {}

#[track_caller]
fn require_count(n: i32) {
    if n < 0 {
        panic_kt("IllegalArgumentException", &format!("Requested element count {n} is less than zero."));
    }
}

/// Kotlin's functions of a list of nullable elements.
pub trait KtListOfNullable<T>: KtIterable<Item = Option<T>> {
    /// Kotlin: `filterNotNull(): List<T>`
    fn filter_not_null(&self) -> Vec<T> {
        self.kt_iter().flatten().collect()
    }
}
impl<T, C: KtIterable<Item = Option<T>> + ?Sized> KtListOfNullable<T> for C {}

/// Kotlin's functions of a list of lists.
pub trait KtListOfLists<I: IntoIterator>: KtIterable<Item = I>
where
    I: Clone,
{
    /// Kotlin: `flatten(): List<T>`
    fn flatten(&self) -> Vec<I::Item> {
        self.kt_iter().flatten().collect()
    }
}
impl<I: IntoIterator + Clone, C: KtIterable<Item = I> + ?Sized> KtListOfLists<I> for C {}

// ---- MutableList ----

/// Kotlin's `MutableList` functions, on a `Vec` (a [`SnapshotStateList`] has them itself, taking `&self`). `Vec`'s
/// own `remove(index)` comes first in a method call: Kotlin's `remove(element)` is `KtMutableList::remove(&mut
/// list, x)`.
pub trait KtMutableList: KtIterable {
    /// Kotlin: `add(element: E): Boolean`
    fn add(&mut self, element: Self::Item) -> bool;
    /// Kotlin: `add(index: Int, element: E)`
    fn add_at(&mut self, index: i32, element: Self::Item);
    /// Kotlin: `addAll(elements: Collection<E>): Boolean`
    fn add_all<I: IntoIterator<Item = Self::Item>>(&mut self, elements: I) -> bool;
    /// Kotlin: `remove(element: E): Boolean`
    fn remove(&mut self, element: Self::Item) -> bool
    where
        Self::Item: PartialEq;
    /// Kotlin: `removeAt(index: Int): E`
    fn remove_at(&mut self, index: i32) -> Self::Item;
    /// Kotlin: `removeAll(predicate: (E) -> Boolean): Boolean`
    fn remove_all(&mut self, predicate: impl FnMut(Self::Item) -> bool) -> bool;
    /// Kotlin: `retainAll(predicate: (E) -> Boolean): Boolean`
    fn retain_all(&mut self, mut predicate: impl FnMut(Self::Item) -> bool) -> bool {
        self.remove_all(|v| !predicate(v))
    }
    /// Kotlin: `removeIf(filter: (E) -> Boolean): Boolean`
    fn remove_if(&mut self, filter: impl FnMut(Self::Item) -> bool) -> bool {
        self.remove_all(filter)
    }
    /// Kotlin's `list[index] = element`: the element it had.
    /// Kotlin: `set(index: Int, element: E): E`
    fn set(&mut self, index: i32, element: Self::Item) -> Self::Item;
    /// Kotlin: `removeFirst(): E`
    fn remove_first(&mut self) -> Self::Item;
    /// Kotlin: `removeLast(): E`
    fn remove_last(&mut self) -> Self::Item;
}

impl<T: Clone> KtMutableList for Vec<T> {
    fn add(&mut self, element: T) -> bool {
        self.push(element);
        true
    }
    #[track_caller]
    fn add_at(&mut self, index: i32, element: T) {
        let i = checked_index(index, self.len() + 1);
        self.insert(i, element);
    }
    fn add_all<I: IntoIterator<Item = T>>(&mut self, elements: I) -> bool {
        let n = self.len();
        self.extend(elements);
        self.len() > n
    }
    fn remove(&mut self, element: T) -> bool
    where
        T: PartialEq,
    {
        match self.iter().position(|v| *v == element) {
            Some(i) => {
                Vec::remove(self, i);
                true
            }
            None => false,
        }
    }
    #[track_caller]
    fn remove_at(&mut self, index: i32) -> T {
        let i = checked_index(index, self.len());
        Vec::remove(self, i)
    }
    fn remove_all(&mut self, mut predicate: impl FnMut(T) -> bool) -> bool {
        let n = self.len();
        self.retain(|v| !predicate(v.clone()));
        self.len() < n
    }
    #[track_caller]
    fn set(&mut self, index: i32, element: T) -> T {
        let i = checked_index(index, self.len());
        std::mem::replace(&mut self[i], element)
    }
    #[track_caller]
    fn remove_first(&mut self) -> T {
        if self.is_empty() {
            panic_kt("NoSuchElementException", "List is empty.");
        }
        Vec::remove(self, 0)
    }
    #[track_caller]
    fn remove_last(&mut self) -> T {
        self.pop().unwrap_or_else(|| panic_kt("NoSuchElementException", "List is empty."))
    }
}

/// Kotlin's `MutableList` functions a state list has (Compose's SnapshotStateList is one): taking `&self`, so a
/// lambda holding the list changes it.
impl<T: Clone + Send + Sync + 'static> SnapshotStateList<T> {
    /// Kotlin: `size: Int`
    pub fn size(&self) -> i32 {
        self.len() as i32
    }
    /// Kotlin: `addAll(elements: Collection<E>): Boolean`
    pub fn add_all(&self, elements: impl IntoIterator<Item = T>) -> bool {
        let more: Vec<T> = elements.into_iter().collect();
        let any = !more.is_empty();
        if any {
            self.update(|v| v.extend(more));
        }
        any
    }
    /// Kotlin: `remove(element: E): Boolean`
    pub fn remove(&self, element: &T) -> bool
    where
        T: PartialEq,
    {
        let at = self.with(|v| v.iter().position(|e| e == element));
        match at {
            Some(i) => {
                self.remove_at(i);
                true
            }
            None => false,
        }
    }
    /// Kotlin: `removeAll(predicate: (E) -> Boolean): Boolean`
    pub fn remove_all(&self, mut predicate: impl FnMut(T) -> bool) -> bool {
        let gone: Vec<bool> = self.with(|v| v.iter().map(|e| predicate(e.clone())).collect());
        if !gone.as_slice().contains(&true) {
            return false;
        }
        self.update(|v| {
            let mut i = 0;
            v.retain(|_| {
                i += 1;
                !gone[i - 1]
            });
        });
        true
    }
    /// Kotlin: `removeIf(filter: (E) -> Boolean): Boolean`
    pub fn remove_if(&self, filter: impl FnMut(T) -> bool) -> bool {
        self.remove_all(filter)
    }
    /// Kotlin: `retainAll(predicate: (E) -> Boolean): Boolean`
    pub fn retain_all(&self, mut predicate: impl FnMut(T) -> bool) -> bool {
        self.remove_all(|v| !predicate(v))
    }
}

// ---- Map ----

/// What Kotlin's `Map` functions ([`KtMapExt`]) read: its entries, in order, as clones.
pub trait KtMapLike {
    type K: Clone;
    type V: Clone;
    fn kt_entries(&self) -> Vec<(Self::K, Self::V)>;
}

impl<K: Clone, V: Clone> KtMapLike for KtMap<K, V> {
    type K = K;
    type V = V;
    fn kt_entries(&self) -> Vec<(K, V)> {
        self.entries.clone()
    }
}
impl<K: Clone, V: Clone> KtMapLike for BTreeMap<K, V> {
    type K = K;
    type V = V;
    fn kt_entries(&self) -> Vec<(K, V)> {
        self.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}
impl<K: Clone, V: Clone, S> KtMapLike for HashMap<K, V, S> {
    type K = K;
    type V = V;
    fn kt_entries(&self) -> Vec<(K, V)> {
        self.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}
/// Its entries as they are when asked (a read that subscribes).
impl<K: Ord + Clone + Send + Sync + 'static, V: Clone + Send + Sync + 'static> KtMapLike for SnapshotStateMap<K, V> {
    type K = K;
    type V = V;
    fn kt_entries(&self) -> Vec<(K, V)> {
        self.with(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }
}
impl<M: KtMapLike + ?Sized> KtMapLike for &M {
    type K = M::K;
    type V = M::V;
    fn kt_entries(&self) -> Vec<(M::K, M::V)> {
        (**self).kt_entries()
    }
}

/// Kotlin's `Map` functions over anything [`KtMapLike`]: entries go to lambdas as pairs (`|(k, v)| …`).
pub trait KtMapExt: KtMapLike {
    /// Kotlin: `size: Int`
    fn size(&self) -> i32 {
        self.kt_entries().len() as i32
    }
    fn is_empty(&self) -> bool {
        self.kt_entries().is_empty()
    }
    fn is_not_empty(&self) -> bool {
        !self.kt_entries().is_empty()
    }
    /// Kotlin: `keys: Set<K>`
    fn keys(&self) -> KtSet<Self::K>
    where
        Self::K: PartialEq,
    {
        KtSet { items: self.kt_entries().into_iter().map(|(k, _)| k).collect() }
    }
    /// Kotlin: `values: Collection<V>`
    fn values(&self) -> Vec<Self::V> {
        self.kt_entries().into_iter().map(|(_, v)| v).collect()
    }
    /// Kotlin: `entries: Set<Map.Entry<K, V>>`
    fn entries(&self) -> Vec<(Self::K, Self::V)> {
        self.kt_entries()
    }
    /// Kotlin's `map[key]`.
    /// Kotlin: `get(key: K): V?`
    fn get(&self, key: Self::K) -> Option<Self::V>
    where
        Self::K: PartialEq,
    {
        self.kt_entries().into_iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }
    /// Kotlin: `getValue(key: K): V`
    #[track_caller]
    fn get_value(&self, key: Self::K) -> Self::V
    where
        Self::K: PartialEq + KtToString,
    {
        let text = to_string(&key);
        KtMapExt::get(self, key)
            .unwrap_or_else(|| panic_kt("NoSuchElementException", &format!("Key {text} is missing in the map.")))
    }
    /// Kotlin: `getOrDefault(key: K, defaultValue: V): V`
    fn get_or_default(&self, key: Self::K, default_value: Self::V) -> Self::V
    where
        Self::K: PartialEq,
    {
        KtMapExt::get(self, key).unwrap_or(default_value)
    }
    /// Kotlin: `getOrElse(key: K, defaultValue: () -> V): V`
    fn get_or_else(&self, key: Self::K, default_value: impl FnOnce() -> Self::V) -> Self::V
    where
        Self::K: PartialEq,
    {
        KtMapExt::get(self, key).unwrap_or_else(default_value)
    }
    /// Kotlin: `containsKey(key: K): Boolean`
    fn contains_key(&self, key: Self::K) -> bool
    where
        Self::K: PartialEq,
    {
        self.kt_entries().iter().any(|(k, _)| *k == key)
    }
    /// Kotlin: `containsValue(value: V): Boolean`
    fn contains_value(&self, value: Self::V) -> bool
    where
        Self::V: PartialEq,
    {
        self.kt_entries().iter().any(|(_, v)| *v == value)
    }
    /// Kotlin: `forEach(action: (Map.Entry<K, V>) -> Unit)`
    fn for_each(&self, action: impl FnMut((Self::K, Self::V))) {
        self.kt_entries().into_iter().for_each(action);
    }
    /// Kotlin: `map(transform: (Map.Entry<K, V>) -> R): List<R>`
    fn map<R>(&self, transform: impl FnMut((Self::K, Self::V)) -> R) -> Vec<R> {
        self.kt_entries().into_iter().map(transform).collect()
    }
    /// Kotlin: `mapValues(transform: (Map.Entry<K, V>) -> R): Map<K, R>`
    fn map_values<R>(&self, mut transform: impl FnMut((Self::K, Self::V)) -> R) -> KtMap<Self::K, R>
    where
        Self::K: PartialEq,
    {
        self.kt_entries().into_iter().map(|e| (e.0.clone(), transform(e))).collect()
    }
    /// Kotlin: `mapKeys(transform: (Map.Entry<K, V>) -> R): Map<R, V>`
    fn map_keys<R: PartialEq>(&self, mut transform: impl FnMut((Self::K, Self::V)) -> R) -> KtMap<R, Self::V> {
        self.kt_entries().into_iter().map(|e| (transform(e.clone()), e.1)).collect()
    }
    /// Kotlin: `filter(predicate: (Map.Entry<K, V>) -> Boolean): Map<K, V>`
    fn filter(&self, mut predicate: impl FnMut((Self::K, Self::V)) -> bool) -> KtMap<Self::K, Self::V>
    where
        Self::K: PartialEq,
    {
        self.kt_entries().into_iter().filter(|e| predicate(e.clone())).collect()
    }
    /// Kotlin: `filterKeys(predicate: (K) -> Boolean): Map<K, V>`
    fn filter_keys(&self, mut predicate: impl FnMut(Self::K) -> bool) -> KtMap<Self::K, Self::V>
    where
        Self::K: PartialEq,
    {
        self.kt_entries().into_iter().filter(|e| predicate(e.0.clone())).collect()
    }
    /// Kotlin: `filterValues(predicate: (V) -> Boolean): Map<K, V>`
    fn filter_values(&self, mut predicate: impl FnMut(Self::V) -> bool) -> KtMap<Self::K, Self::V>
    where
        Self::K: PartialEq,
    {
        self.kt_entries().into_iter().filter(|e| predicate(e.1.clone())).collect()
    }
    /// Kotlin: `any(predicate: (Map.Entry<K, V>) -> Boolean): Boolean`
    fn any_that(&self, predicate: impl FnMut((Self::K, Self::V)) -> bool) -> bool {
        self.kt_entries().into_iter().any(predicate)
    }
    /// Kotlin: `all(predicate: (Map.Entry<K, V>) -> Boolean): Boolean`
    fn all(&self, predicate: impl FnMut((Self::K, Self::V)) -> bool) -> bool {
        self.kt_entries().into_iter().all(predicate)
    }
    /// Kotlin: `count(predicate: (Map.Entry<K, V>) -> Boolean): Int`
    fn count_that(&self, mut predicate: impl FnMut((Self::K, Self::V)) -> bool) -> i32 {
        self.kt_entries().into_iter().filter(|e| predicate(e.clone())).count() as i32
    }
    /// Kotlin: `toList(): List<Pair<K, V>>`
    fn to_list(&self) -> Vec<(Self::K, Self::V)> {
        self.kt_entries()
    }
    /// Kotlin: `toMutableMap(): MutableMap<K, V>`
    fn to_mutable_map(&self) -> KtMap<Self::K, Self::V>
    where
        Self::K: PartialEq,
    {
        self.kt_entries().into_iter().collect()
    }
    /// Kotlin: `plus(pair: Pair<K, V>): Map<K, V>` (`map + (k to v)`)
    fn plus(&self, pair: (Self::K, Self::V)) -> KtMap<Self::K, Self::V>
    where
        Self::K: PartialEq,
    {
        let mut out: KtMap<_, _> = self.kt_entries().into_iter().collect();
        out.put(pair.0, pair.1);
        out
    }
    /// Kotlin: `minus(key: K): Map<K, V>` (`map - key`)
    fn minus(&self, key: Self::K) -> KtMap<Self::K, Self::V>
    where
        Self::K: PartialEq,
    {
        self.kt_entries().into_iter().filter(|(k, _)| *k != key).collect()
    }
}

impl<M: KtMapLike + ?Sized> KtMapExt for M {}

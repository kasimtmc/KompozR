//! Kotlin's mutable collections, references as Kotlin's are: [`MutableList`], [`MutableSet`], [`MutableMap`] are
//! shared cells (a clone is the same object: a function given one and calling `add`/`removeAt`/`put` changes the
//! caller's, two `val`s holding it see one object, `===` is its identity), while Kotlin's read-only `List`, `Set` and
//! `Map` stay values (`Vec`, [`KtSet`], [`KtMap`]): a mutable one given where a read-only one is taken is a snapshot of
//! it (`toList()`), as reading it then would be.
//!
//! A [`MutableList`] may be a [`SnapshotStateList`] (Compose's is a `MutableList`): the same state list, its changes
//! the state's. A [`ListView`] is Kotlin's `List<T>` where a function asks whether it is a `MutableList` (`if
//! (backStack is MutableList) backStack.removeAt(…)`): the caller's list, read as it is when read.
//!
//! No borrow of a cell is held across a call of the caller's code (a predicate, a selector, a default): what such a
//! call reads or changes of the same object is seen as Kotlin's would be.

use super::collections::{KtIterable, KtMap, KtMapLike, KtSet};
use super::object::KtHashCode;
use super::random::{KtShuffle, Random};
use super::range::{KtContains, kt_compare};
use super::text::{KtToString, write_list, write_map};
use super::{KtList, panic_kt};
use crate::SnapshotStateList;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::fmt;
use std::rc::Rc;

/// An index Kotlin is given (`list[i]`), checked as Kotlin checks it.
#[track_caller]
fn checked(index: i32, len: usize) -> usize {
    if index < 0 || index as usize >= len {
        panic_kt("IndexOutOfBoundsException", &format!("Index {index} out of bounds for length {len}"));
    }
    index as usize
}

// ---- MutableList ----

/// What a [`MutableList`] holds its elements in: its own cell, or a state list it is.
enum ListCell<T> {
    Own(Rc<RefCell<Vec<T>>>),
    State(SnapshotStateList<T>),
}

/// Kotlin's `MutableList<T>` (`mutableListOf`, `ArrayList`, `toMutableList()`): one list shared by every clone of it,
/// changed through any (its functions take `&self`); `==` compares the elements, [`same`](Self::same) the object.
pub struct MutableList<T> {
    cell: ListCell<T>,
}

impl<T> Clone for MutableList<T> {
    /// The same list (Kotlin's reference to it).
    fn clone(&self) -> Self {
        MutableList {
            cell: match &self.cell {
                ListCell::Own(c) => ListCell::Own(c.clone()),
                ListCell::State(s) => ListCell::State(s.clone()),
            },
        }
    }
}

impl<T> Default for MutableList<T> {
    fn default() -> Self {
        MutableList::from_vec(Vec::new())
    }
}

impl<T> MutableList<T> {
    /// A new list of `items`.
    pub fn from_vec(items: Vec<T>) -> MutableList<T> {
        MutableList { cell: ListCell::Own(Rc::new(RefCell::new(items))) }
    }
    /// A new empty list.
    pub fn new() -> MutableList<T> {
        MutableList::from_vec(Vec::new())
    }
    /// The state list itself, as a `MutableList` (Compose's `SnapshotStateList` is one): its changes the state's.
    pub fn of_state(list: SnapshotStateList<T>) -> MutableList<T> {
        MutableList { cell: ListCell::State(list) }
    }
    /// Whether it is the same list as `other` (Kotlin's `===`).
    pub fn same(&self, other: &MutableList<T>) -> bool {
        match (&self.cell, &other.cell) {
            (ListCell::Own(a), ListCell::Own(b)) => Rc::ptr_eq(a, b),
            (ListCell::State(a), ListCell::State(b)) => a == b,
            _ => false,
        }
    }
}

impl<T: Clone + 'static> MutableList<T> {
    /// Its elements read (borrowed while `f` runs: `f` mustn't change the list).
    pub fn with<R>(&self, f: impl FnOnce(&[T]) -> R) -> R {
        match &self.cell {
            ListCell::Own(c) => f(&c.borrow()),
            ListCell::State(s) => s.with(f),
        }
    }
    /// Its elements changed in one go.
    pub fn update<R>(&self, f: impl FnOnce(&mut Vec<T>) -> R) -> R {
        match &self.cell {
            ListCell::Own(c) => f(&mut c.borrow_mut()),
            ListCell::State(s) => {
                let mut out = None;
                s.update(|v| out = Some(f(v)));
                out.expect("updated")
            }
        }
    }
    /// Its elements as they are now (a read-only `List` of them: Kotlin's `toList()`).
    pub fn to_vec(&self) -> Vec<T> {
        self.with(<[T]>::to_vec)
    }
    /// Kotlin: `size: Int`
    pub fn size(&self) -> i32 {
        self.with(|v| v.len() as i32)
    }
    /// Kotlin: `isEmpty(): Boolean`
    pub fn is_empty(&self) -> bool {
        self.with(<[T]>::is_empty)
    }
    /// Kotlin's `list[index]`.
    /// Kotlin: `get(index: Int): E`
    #[track_caller]
    pub fn get(&self, index: i32) -> T {
        self.with(|v| v[checked(index, v.len())].clone())
    }
    /// Kotlin's `list[index] = element`: the element it had.
    /// Kotlin: `set(index: Int, element: E): E`
    #[track_caller]
    pub fn set(&self, index: i32, element: T) -> T {
        let i = self.with(|v| checked(index, v.len()));
        self.update(|v| std::mem::replace(&mut v[i], element))
    }
    /// Kotlin: `add(element: E): Boolean`
    pub fn add(&self, element: T) -> bool {
        self.update(|v| v.push(element));
        true
    }
    /// Kotlin: `add(index: Int, element: E)`
    #[track_caller]
    pub fn add_at(&self, index: i32, element: T) {
        let i = self.with(|v| checked(index, v.len() + 1));
        self.update(|v| v.insert(i, element));
    }
    /// Kotlin: `addAll(elements: Collection<E>): Boolean`
    pub fn add_all(&self, elements: impl IntoIterator<Item = T>) -> bool {
        let more: Vec<T> = elements.into_iter().collect();
        if more.is_empty() {
            return false;
        }
        self.update(|v| v.extend(more));
        true
    }
    /// Kotlin: `addAll(index: Int, elements: Collection<E>): Boolean`
    #[track_caller]
    pub fn add_all_at(&self, index: i32, elements: impl IntoIterator<Item = T>) -> bool {
        let i = self.with(|v| checked(index, v.len() + 1));
        let more: Vec<T> = elements.into_iter().collect();
        if more.is_empty() {
            return false;
        }
        self.update(|v| {
            let tail = v.split_off(i);
            v.extend(more);
            v.extend(tail);
        });
        true
    }
    /// Its first element equal to `element` removed: whether there was one.
    /// Kotlin: `remove(element: E): Boolean`
    pub fn remove(&self, element: T) -> bool
    where
        T: PartialEq,
    {
        match self.with(|v| v.iter().position(|e| *e == element)) {
            Some(i) => {
                self.update(|v| v.remove(i));
                true
            }
            None => false,
        }
    }
    /// Kotlin: `removeAt(index: Int): E`
    #[track_caller]
    pub fn remove_at(&self, index: i32) -> T {
        let i = self.with(|v| checked(index, v.len()));
        self.update(|v| v.remove(i))
    }
    /// Kotlin: `removeAll(predicate: (E) -> Boolean): Boolean`
    pub fn remove_all(&self, mut predicate: impl FnMut(T) -> bool) -> bool {
        let gone: Vec<bool> = self.to_vec().into_iter().map(&mut predicate).collect();
        if !gone.as_slice().contains(&true) {
            return false;
        }
        self.update(|v| {
            let mut i = 0;
            v.retain(|_| {
                i += 1;
                !gone.as_slice().get(i - 1).copied().unwrap_or(false)
            });
        });
        true
    }
    /// Kotlin: `retainAll(predicate: (E) -> Boolean): Boolean`
    pub fn retain_all(&self, mut predicate: impl FnMut(T) -> bool) -> bool {
        self.remove_all(|v| !predicate(v))
    }
    /// Kotlin: `removeIf(filter: (E) -> Boolean): Boolean`
    pub fn remove_if(&self, filter: impl FnMut(T) -> bool) -> bool {
        self.remove_all(filter)
    }
    /// Kotlin: `removeFirst(): E`
    #[track_caller]
    pub fn remove_first(&self) -> T {
        self.remove_first_or_null().unwrap_or_else(|| panic_kt("NoSuchElementException", "List is empty."))
    }
    /// Kotlin: `removeLast(): E`
    #[track_caller]
    pub fn remove_last(&self) -> T {
        self.remove_last_or_null().unwrap_or_else(|| panic_kt("NoSuchElementException", "List is empty."))
    }
    /// Its first element removed, none if it is empty.
    /// Kotlin: `removeFirstOrNull(): E?`
    pub fn remove_first_or_null(&self) -> Option<T> {
        (!self.is_empty()).then(|| self.update(|v| v.remove(0)))
    }
    /// Its last element removed, none if it is empty.
    /// Kotlin: `removeLastOrNull(): E?`
    pub fn remove_last_or_null(&self) -> Option<T> {
        (!self.is_empty()).then(|| self.update(|v| v.pop().expect("not empty")))
    }
    /// Kotlin: `clear()`
    pub fn clear(&self) {
        if !self.is_empty() {
            self.update(Vec::clear);
        }
    }
    /// Reverses its elements in place.
    /// Kotlin: `reverse()`
    pub fn reverse(&self) {
        if self.size() > 1 {
            self.update(|v| v.reverse());
        }
    }
    /// Kotlin: `shuffle(random: Random)`
    pub fn shuffle(&self, random: &Random) {
        self.update(|v| KtShuffle::shuffle(v, random));
    }
    /// Sorts it in place by Kotlin's `compareTo` (stable).
    /// Kotlin: `sort()`
    pub fn sort(&self)
    where
        T: PartialOrd,
    {
        self.update(|v| v.sort_by(kt_compare));
    }
    /// Kotlin: `sortDescending()`
    pub fn sort_descending(&self)
    where
        T: PartialOrd,
    {
        self.update(|v| v.sort_by(|a, b| kt_compare(b, a)));
    }
    /// Kotlin: `sortBy(selector: (T) -> R?)`
    pub fn sort_by<R: PartialOrd>(&self, selector: impl FnMut(T) -> R) {
        let sorted = KtList::sorted_by(&self.to_vec(), selector);
        self.update(|v| *v = sorted);
    }
    /// Kotlin: `sortByDescending(selector: (T) -> R?)`
    pub fn sort_by_descending<R: PartialOrd>(&self, selector: impl FnMut(T) -> R) {
        let sorted = KtList::sorted_by_descending(&self.to_vec(), selector);
        self.update(|v| *v = sorted);
    }
    /// Kotlin: `sortWith(comparator: Comparator<in T>)`
    pub fn sort_with(&self, comparator: impl FnMut(&T, &T) -> Ordering) {
        let sorted = KtList::sorted_with(&self.to_vec(), comparator);
        self.update(|v| *v = sorted);
    }
    /// Sets each element to `element`.
    /// Kotlin: `fill(value: T)`
    pub fn fill(&self, value: T) {
        self.update(|v| v.iter_mut().for_each(|e| *e = value.clone()));
    }
}

impl<T> From<Vec<T>> for MutableList<T> {
    fn from(items: Vec<T>) -> Self {
        MutableList::from_vec(items)
    }
}

impl<T> From<SnapshotStateList<T>> for MutableList<T> {
    fn from(list: SnapshotStateList<T>) -> Self {
        MutableList::of_state(list)
    }
}

impl<T> FromIterator<T> for MutableList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        MutableList::from_vec(iter.into_iter().collect())
    }
}

impl<T: Clone + 'static> Extend<T> for MutableList<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.add_all(iter);
    }
}

/// Its elements as they are when asked.
impl<T: Clone + 'static> KtIterable for MutableList<T> {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.to_vec().into_iter()
    }
}

/// `for (x in list)`: its elements as they were when the loop began.
impl<T: Clone + 'static> IntoIterator for MutableList<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.to_vec().into_iter()
    }
}
impl<T: Clone + 'static> IntoIterator for &MutableList<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.to_vec().into_iter()
    }
}

impl<T: PartialEq + Clone + 'static> KtContains<T> for MutableList<T> {
    fn kt_contains(&self, value: &T) -> bool {
        self.with(|v| v.contains(value))
    }
}

/// Kotlin's `==` of lists: the same elements in the same order.
impl<T: PartialEq + Clone + 'static> PartialEq for MutableList<T> {
    fn eq(&self, other: &Self) -> bool {
        self.same(other) || self.to_vec() == other.to_vec()
    }
}
impl<T: PartialEq + Clone + 'static> PartialEq<Vec<T>> for MutableList<T> {
    fn eq(&self, other: &Vec<T>) -> bool {
        self.with(|v| v == other.as_slice())
    }
}
impl<T: PartialEq + Clone + 'static> PartialEq<MutableList<T>> for Vec<T> {
    fn eq(&self, other: &MutableList<T>) -> bool {
        other == self
    }
}

impl<T: KtToString + Clone + 'static> KtToString for MutableList<T> {
    fn kt_write(&self, out: &mut String) {
        self.with(|v| write_list(out, v));
    }
}
impl<T: KtHashCode + Clone + 'static> KtHashCode for MutableList<T> {
    fn kt_hash_code(&self) -> i32 {
        self.to_vec().kt_hash_code()
    }
}
impl<T: fmt::Debug + Clone + 'static> fmt::Debug for MutableList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with(|v| f.debug_list().entries(v).finish())
    }
}
impl<T: KtToString + Clone + 'static> fmt::Display for MutableList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.kt_to_string())
    }
}

// ---- ListView: a List that may be a MutableList ----

/// Kotlin's `List<T>` where the function holding it asks whether it is a `MutableList` (`backStack is MutableList`):
/// the caller's list as it is — a read-only one's elements, or the mutable list itself (read as it is when read,
/// changed through [`as_mutable`](Self::as_mutable)).
#[derive(Clone)]
pub enum ListView<T> {
    /// A read-only list's elements.
    Values(Rc<Vec<T>>),
    /// A mutable list (a state list too).
    Mutable(MutableList<T>),
}

impl<T> ListView<T> {
    /// Kotlin's `list is MutableList`.
    pub fn is_mutable(&self) -> bool {
        matches!(self, ListView::Mutable(_))
    }
    /// Kotlin's `list as? MutableList<T>`: the mutable list it is, if it is one.
    pub fn as_mutable(&self) -> Option<MutableList<T>> {
        match self {
            ListView::Mutable(m) => Some(m.clone()),
            ListView::Values(_) => None,
        }
    }
    /// Whether it is the same list as `other` (Kotlin's `===`).
    pub fn same(&self, other: &ListView<T>) -> bool {
        match (self, other) {
            (ListView::Values(a), ListView::Values(b)) => Rc::ptr_eq(a, b),
            (ListView::Mutable(a), ListView::Mutable(b)) => a.same(b),
            _ => false,
        }
    }
}

impl<T: Clone + 'static> ListView<T> {
    /// Its elements as they are now.
    pub fn to_vec(&self) -> Vec<T> {
        match self {
            ListView::Values(v) => (**v).clone(),
            ListView::Mutable(m) => m.to_vec(),
        }
    }
    /// Kotlin: `size: Int`
    pub fn size(&self) -> i32 {
        match self {
            ListView::Values(v) => v.len() as i32,
            ListView::Mutable(m) => m.size(),
        }
    }
    /// Kotlin's `list[index]`.
    /// Kotlin: `get(index: Int): E`
    #[track_caller]
    pub fn get(&self, index: i32) -> T {
        match self {
            ListView::Values(v) => v[checked(index, v.len())].clone(),
            ListView::Mutable(m) => m.get(index),
        }
    }
}

impl<T> From<Vec<T>> for ListView<T> {
    fn from(items: Vec<T>) -> Self {
        ListView::Values(Rc::new(items))
    }
}
impl<T: Clone> From<&[T]> for ListView<T> {
    fn from(items: &[T]) -> Self {
        ListView::Values(Rc::new(items.to_vec()))
    }
}
impl<T> From<MutableList<T>> for ListView<T> {
    fn from(list: MutableList<T>) -> Self {
        ListView::Mutable(list)
    }
}
impl<T> From<SnapshotStateList<T>> for ListView<T> {
    fn from(list: SnapshotStateList<T>) -> Self {
        ListView::Mutable(MutableList::of_state(list))
    }
}

impl<T: Clone + 'static> KtIterable for ListView<T> {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.to_vec().into_iter()
    }
}
impl<T: Clone + 'static> IntoIterator for ListView<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.to_vec().into_iter()
    }
}
impl<T: Clone + 'static> IntoIterator for &ListView<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.to_vec().into_iter()
    }
}
impl<T: PartialEq + Clone + 'static> KtContains<T> for ListView<T> {
    fn kt_contains(&self, value: &T) -> bool {
        self.to_vec().as_slice().contains(value)
    }
}
impl<T: PartialEq + Clone + 'static> PartialEq for ListView<T> {
    fn eq(&self, other: &Self) -> bool {
        self.to_vec() == other.to_vec()
    }
}
impl<T: KtToString + Clone + 'static> KtToString for ListView<T> {
    fn kt_write(&self, out: &mut String) {
        write_list(out, &self.to_vec());
    }
}
impl<T: KtHashCode + Clone + 'static> KtHashCode for ListView<T> {
    fn kt_hash_code(&self) -> i32 {
        self.to_vec().kt_hash_code()
    }
}
impl<T: fmt::Debug + Clone + 'static> fmt::Debug for ListView<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.to_vec()).finish()
    }
}

// ---- MutableSet ----

/// Kotlin's `MutableSet<T>` (`mutableSetOf`, `HashSet`, `toMutableSet()`): one set (insertion-ordered, as Kotlin's
/// `LinkedHashSet`) shared by every clone of it.
pub struct MutableSet<T> {
    cell: Rc<RefCell<KtSet<T>>>,
}

impl<T> Clone for MutableSet<T> {
    /// The same set.
    fn clone(&self) -> Self {
        MutableSet { cell: self.cell.clone() }
    }
}

impl<T> Default for MutableSet<T> {
    fn default() -> Self {
        MutableSet { cell: Rc::new(RefCell::new(KtSet::default())) }
    }
}

impl<T> MutableSet<T> {
    /// A new set of `items`.
    pub fn of(items: KtSet<T>) -> MutableSet<T> {
        MutableSet { cell: Rc::new(RefCell::new(items)) }
    }
    /// A new empty set.
    pub fn new() -> MutableSet<T> {
        MutableSet::default()
    }
    /// Whether it is the same set as `other` (Kotlin's `===`).
    pub fn same(&self, other: &MutableSet<T>) -> bool {
        Rc::ptr_eq(&self.cell, &other.cell)
    }
}

impl<T: PartialEq + Clone + 'static> MutableSet<T> {
    /// Its elements as they are now (a read-only `Set` of them: Kotlin's `toSet()`).
    pub fn to_set(&self) -> KtSet<T> {
        self.cell.borrow().clone()
    }
    /// Kotlin: `size: Int`
    pub fn size(&self) -> i32 {
        self.cell.borrow().size()
    }
    /// Kotlin: `isEmpty(): Boolean`
    pub fn is_empty(&self) -> bool {
        self.cell.borrow().is_empty()
    }
    /// Kotlin: `contains(element: E): Boolean`
    pub fn contains(&self, element: T) -> bool {
        self.cell.borrow().contains(element)
    }
    /// Adds it unless it is there: whether it was added.
    /// Kotlin: `add(element: E): Boolean`
    pub fn add(&self, element: T) -> bool {
        self.cell.borrow_mut().add(element)
    }
    /// Kotlin: `addAll(elements: Collection<E>): Boolean`
    pub fn add_all(&self, elements: impl IntoIterator<Item = T>) -> bool {
        let more: Vec<T> = elements.into_iter().collect();
        let mut any = false;
        for e in more {
            any |= self.add(e);
        }
        any
    }
    /// Kotlin: `remove(element: E): Boolean`
    pub fn remove(&self, element: T) -> bool {
        self.cell.borrow_mut().remove(element)
    }
    /// Kotlin: `removeAll(predicate: (E) -> Boolean): Boolean`
    pub fn remove_all(&self, mut predicate: impl FnMut(T) -> bool) -> bool {
        let gone: Vec<T> = self.to_set().into_iter().filter(|e| predicate(e.clone())).collect();
        for e in &gone {
            self.remove(e.clone());
        }
        !gone.is_empty()
    }
    /// Kotlin: `retainAll(predicate: (E) -> Boolean): Boolean`
    pub fn retain_all(&self, mut predicate: impl FnMut(T) -> bool) -> bool {
        self.remove_all(|v| !predicate(v))
    }
    /// Kotlin: `removeIf(filter: (E) -> Boolean): Boolean`
    pub fn remove_if(&self, filter: impl FnMut(T) -> bool) -> bool {
        self.remove_all(filter)
    }
    /// Kotlin: `clear()`
    pub fn clear(&self) {
        self.cell.borrow_mut().clear();
    }
}

impl<T: PartialEq + Clone + 'static> From<KtSet<T>> for MutableSet<T> {
    fn from(items: KtSet<T>) -> Self {
        MutableSet::of(items)
    }
}
impl<T: PartialEq + Clone + 'static> FromIterator<T> for MutableSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        MutableSet::of(iter.into_iter().collect())
    }
}
impl<T: PartialEq + Clone + 'static> Extend<T> for MutableSet<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.add_all(iter);
    }
}
impl<T: Clone + 'static> KtIterable for MutableSet<T> {
    type Item = T;
    fn kt_iter(&self) -> impl Iterator<Item = T> {
        self.cell.borrow().as_slice().to_vec().into_iter()
    }
}
impl<T: Clone + 'static> IntoIterator for MutableSet<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.cell.borrow().as_slice().to_vec().into_iter()
    }
}
impl<T: Clone + 'static> IntoIterator for &MutableSet<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.cell.borrow().as_slice().to_vec().into_iter()
    }
}
impl<T: PartialEq> KtContains<T> for MutableSet<T> {
    fn kt_contains(&self, value: &T) -> bool {
        self.cell.borrow().as_slice().contains(value)
    }
}
/// Kotlin's `==` of sets: the same elements, in any order.
impl<T: PartialEq> PartialEq for MutableSet<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.cell, &other.cell) || *self.cell.borrow() == *other.cell.borrow()
    }
}
impl<T: PartialEq> PartialEq<KtSet<T>> for MutableSet<T> {
    fn eq(&self, other: &KtSet<T>) -> bool {
        *self.cell.borrow() == *other
    }
}
impl<T: PartialEq> PartialEq<MutableSet<T>> for KtSet<T> {
    fn eq(&self, other: &MutableSet<T>) -> bool {
        *self == *other.cell.borrow()
    }
}
impl<T: KtToString> KtToString for MutableSet<T> {
    fn kt_write(&self, out: &mut String) {
        write_list(out, self.cell.borrow().as_slice());
    }
}
impl<T: fmt::Debug> fmt::Debug for MutableSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.cell.borrow().as_slice()).finish()
    }
}
impl<T: KtToString> fmt::Display for MutableSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.kt_to_string())
    }
}

// ---- MutableMap ----

/// Kotlin's `MutableMap<K, V>` (`mutableMapOf`, `HashMap`, `toMutableMap()`): one map (insertion-ordered, as
/// Kotlin's `LinkedHashMap`) shared by every clone of it.
pub struct MutableMap<K, V> {
    cell: Rc<RefCell<KtMap<K, V>>>,
}

impl<K, V> Clone for MutableMap<K, V> {
    /// The same map.
    fn clone(&self) -> Self {
        MutableMap { cell: self.cell.clone() }
    }
}

impl<K, V> Default for MutableMap<K, V> {
    fn default() -> Self {
        MutableMap { cell: Rc::new(RefCell::new(KtMap::default())) }
    }
}

impl<K, V> MutableMap<K, V> {
    /// A new map of `entries`.
    pub fn of(entries: KtMap<K, V>) -> MutableMap<K, V> {
        MutableMap { cell: Rc::new(RefCell::new(entries)) }
    }
    /// A new empty map.
    pub fn new() -> MutableMap<K, V> {
        MutableMap::default()
    }
    /// Whether it is the same map as `other` (Kotlin's `===`).
    pub fn same(&self, other: &MutableMap<K, V>) -> bool {
        Rc::ptr_eq(&self.cell, &other.cell)
    }
}

impl<K: PartialEq + Clone + 'static, V: Clone + 'static> MutableMap<K, V> {
    /// Its entries as they are now (a read-only `Map` of them: Kotlin's `toMap()`).
    pub fn to_map(&self) -> KtMap<K, V> {
        self.cell.borrow().clone()
    }
    /// Kotlin: `size: Int`
    pub fn size(&self) -> i32 {
        self.cell.borrow().size()
    }
    /// Kotlin: `isEmpty(): Boolean`
    pub fn is_empty(&self) -> bool {
        self.cell.borrow().is_empty()
    }
    /// Kotlin's `map[key]`: its value, or null.
    /// Kotlin: `get(key: K): V?`
    pub fn get(&self, key: K) -> Option<V> {
        self.cell.borrow().get(key)
    }
    /// Kotlin: `containsKey(key: K): Boolean`
    pub fn contains_key(&self, key: K) -> bool {
        self.cell.borrow().contains_key(key)
    }
    /// Sets the key's value (keeping its place if it has one): the value it had.
    /// Kotlin: `put(key: K, value: V): V?`
    pub fn put(&self, key: K, value: V) -> Option<V> {
        self.cell.borrow_mut().put(key, value)
    }
    /// Kotlin's `map[key] = value`.
    /// Kotlin: `set(key: K, value: V)`
    pub fn set(&self, key: K, value: V) {
        self.put(key, value);
    }
    /// Kotlin: `putAll(from: Map<out K, V>)`
    pub fn put_all(&self, entries: impl IntoIterator<Item = (K, V)>) {
        let entries: Vec<(K, V)> = entries.into_iter().collect();
        self.cell.borrow_mut().put_all(entries);
    }
    /// The value the key had, removed.
    /// Kotlin: `remove(key: K): V?`
    pub fn remove(&self, key: K) -> Option<V> {
        self.cell.borrow_mut().remove(key)
    }
    /// Kotlin: `clear()`
    pub fn clear(&self) {
        self.cell.borrow_mut().clear();
    }
    /// Kotlin's `getOrPut(key) { … }`: its value, else `default`'s (made with the map not borrowed), put first.
    /// Kotlin: `getOrPut(key: K, defaultValue: () -> V): V`
    pub fn get_or_put(&self, key: K, default: impl FnOnce() -> V) -> V {
        if let Some(v) = self.cell.borrow().get_ref(&key) {
            return v.clone();
        }
        let v = default();
        self.put(key, v.clone());
        v
    }
    /// [`get_or_put`](Self::get_or_put)'s suspending twin.
    #[doc(hidden)]
    pub async fn get_or_put_async(&self, key: K, default: impl AsyncFnOnce() -> V) -> V {
        if let Some(v) = self.cell.borrow().get_ref(&key) {
            return v.clone();
        }
        let v = default().await;
        self.put(key, v.clone());
        v
    }
}

impl<K: PartialEq + Clone + 'static, V: Clone + 'static> From<KtMap<K, V>> for MutableMap<K, V> {
    fn from(entries: KtMap<K, V>) -> Self {
        MutableMap::of(entries)
    }
}
impl<K: PartialEq + Clone + 'static, V: Clone + 'static> FromIterator<(K, V)> for MutableMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        MutableMap::of(iter.into_iter().collect())
    }
}
impl<K: PartialEq + Clone + 'static, V: Clone + 'static> Extend<(K, V)> for MutableMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        self.put_all(iter);
    }
}
/// Its entries as they are when asked.
impl<K: Clone, V: Clone> KtMapLike for MutableMap<K, V> {
    type K = K;
    type V = V;
    fn kt_entries(&self) -> Vec<(K, V)> {
        self.cell.borrow().as_slice().to_vec()
    }
}
/// `for ((k, v) in map)`: its entries as they were when the loop began.
impl<K: Clone, V: Clone> IntoIterator for MutableMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.kt_entries().into_iter()
    }
}
impl<K: Clone, V: Clone> IntoIterator for &MutableMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.kt_entries().into_iter()
    }
}
/// `key in map`.
impl<K: PartialEq, V> KtContains<K> for MutableMap<K, V> {
    fn kt_contains(&self, key: &K) -> bool {
        self.cell.borrow().get_ref(key).is_some()
    }
}
/// Kotlin's `==` of maps: the same entries, in any order.
impl<K: PartialEq, V: PartialEq> PartialEq for MutableMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.cell, &other.cell) || *self.cell.borrow() == *other.cell.borrow()
    }
}
impl<K: PartialEq, V: PartialEq> PartialEq<KtMap<K, V>> for MutableMap<K, V> {
    fn eq(&self, other: &KtMap<K, V>) -> bool {
        *self.cell.borrow() == *other
    }
}
impl<K: PartialEq, V: PartialEq> PartialEq<MutableMap<K, V>> for KtMap<K, V> {
    fn eq(&self, other: &MutableMap<K, V>) -> bool {
        *self == *other.cell.borrow()
    }
}
impl<K: KtToString, V: KtToString> KtToString for MutableMap<K, V> {
    fn kt_write(&self, out: &mut String) {
        write_map(out, self.cell.borrow().as_slice().iter().map(|(k, v)| (k, v)));
    }
}
impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for MutableMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.cell.borrow().as_slice().iter().map(|(k, v)| (k, v))).finish()
    }
}
impl<K: KtToString, V: KtToString> fmt::Display for MutableMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.kt_to_string())
    }
}

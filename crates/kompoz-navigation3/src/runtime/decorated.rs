//! DecoratedNavEntries.kt: `rememberDecoratedNavEntries`.

use super::nav_entry::{NavEntry, NavEntryDecorator};
use super::nav_key::NavEntryKey;
use kompoz_runtime::{
    DisposableEffect, SnapshotStateList, group, key, kompozable, on_dispose, remember, remember_keyed,
    remember_updated_state,
};
use kompoz_ui::layout::SlotKey;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// A list of keys as a back stack (Kotlin's `List<T>` of `rememberDecoratedNavEntries(backStack, …)` and
/// `NavDisplay(backStack, …)`): its keys (read where it is given: an observable list — a [`NavBackStack`]
/// (super::NavBackStack), a `SnapshotStateList` — makes the reader compose again when it changes), and whether it is
/// one a display may pop (Kotlin's `backStack is MutableList`).
pub trait BackStackList<T> {
    /// Its keys, in order.
    fn keys(&self) -> Vec<T>;
    /// Removes its last key (Kotlin's `removeLastOrNull()`), if it is mutable: whether it could.
    fn remove_last_key(&self) -> bool {
        false
    }
}

impl<T: Clone> BackStackList<T> for Vec<T> {
    fn keys(&self) -> Vec<T> {
        self.clone()
    }
}

impl<T: Clone> BackStackList<T> for [T] {
    fn keys(&self) -> Vec<T> {
        self.to_vec()
    }
}

impl<T: Clone, const N: usize> BackStackList<T> for [T; N] {
    fn keys(&self) -> Vec<T> {
        self.to_vec()
    }
}

impl<T: Clone + 'static> BackStackList<T> for SnapshotStateList<T> {
    fn keys(&self) -> Vec<T> {
        self.to_vec()
    }
    fn remove_last_key(&self) -> bool {
        let n = self.len();
        if n == 0 {
            return false;
        }
        self.remove_at(n - 1);
        true
    }
}

impl<T, L: BackStackList<T> + ?Sized> BackStackList<T> for &L {
    fn keys(&self) -> Vec<T> {
        (**self).keys()
    }
    fn remove_last_key(&self) -> bool {
        (**self).remove_last_key()
    }
}

impl<T, L: BackStackList<T> + ?Sized> BackStackList<T> for Rc<L> {
    fn keys(&self) -> Vec<T> {
        (**self).keys()
    }
    fn remove_last_key(&self) -> bool {
        (**self).remove_last_key()
    }
}

/// The entries of `back_stack`'s keys, made by `entry_provider` and decorated with `entry_decorators` (Compose's
/// `rememberDecoratedNavEntries(backStack, entryDecorators, entryProvider)`): remembered while the keys are the same
/// (across recompositions, not process death: the back stack and the decorators' states are hoisted and saved by
/// the caller). An observable back stack (a [`NavBackStack`](super::NavBackStack), a `SnapshotStateList`) or an
/// immutable one replaced with another: navigate and pop change it, the entries follow. One call per back stack
/// (several back stacks: one each, their entries concatenated if wanted).
///
/// Kotlin: `rememberDecoratedNavEntries(backStack: List<T>, entryDecorators: List<NavEntryDecorator<T>> = listOf(), entryProvider: (key: T) -> NavEntry<T>): List<NavEntry<T>>`
#[kompozable]
pub fn remember_decorated_nav_entries<T: NavEntryKey>(
    back_stack: impl BackStackList<T>,
    #[default(Vec::new())] entry_decorators: Vec<NavEntryDecorator<T>>,
    entry_provider: impl Fn(T) -> NavEntry<T> + 'static,
) -> Vec<NavEntry<T>> {
    let keys = back_stack.keys();
    let k = keys.clone();
    let entries = remember_keyed(keys, move || k.into_iter().map(&entry_provider).collect::<Vec<_>>());
    remember_decorated_nav_entries_of(entries, entry_decorators)
}

/// `entries` decorated with `entry_decorators` (Compose's `rememberDecoratedNavEntries(entries, entryDecorators)`):
/// entries decorated already are decorated again, these decorators outside theirs. A decorator's `on_pop` of a
/// content key is called once the last entry of that key has left the entries and its content the composition.
///
/// Kotlin: `rememberDecoratedNavEntries(entries: List<NavEntry<T>>, entryDecorators: List<NavEntryDecorator<T>> = listOf()): List<NavEntry<T>>`
#[kompozable]
pub fn remember_decorated_nav_entries_of<T: NavEntryKey>(
    entries: Vec<NavEntry<T>>,
    #[default(Vec::new())] entry_decorators: Vec<NavEntryDecorator<T>>,
) -> Vec<NavEntry<T>> {
    let keys_in_backstack: Rc<RefCell<HashSet<SlotKey>>> = remember(|| Rc::new(RefCell::new(HashSet::new())));
    let keys_in_composition: Rc<RefCell<HashSet<SlotKey>>> = remember(|| Rc::new(RefCell::new(HashSet::new())));
    let decorated: Vec<NavEntry<T>> = entries
        .iter()
        .map(|entry| decorate_entry(entry, &entry_decorators, &keys_in_backstack, &keys_in_composition))
        .collect();
    prepare_back_stack(&decorated, &entry_decorators, &keys_in_backstack, &keys_in_composition);
    decorated
}

/// `decorators` in order without the repeated ones (Kotlin's `distinct()`).
fn distinct<T>(decorators: &[NavEntryDecorator<T>]) -> Vec<NavEntryDecorator<T>> {
    let mut out: Vec<NavEntryDecorator<T>> = Vec::new();
    for d in decorators {
        if !out.contains(d) {
            out.push(d.clone());
        }
    }
    out
}

/// Compose's private `decorateEntry`: `entry` wrapped with `decorators` (the first outermost), its content tracking
/// whether it is composed, and calling the decorators' `on_pop` (in reverse) when it leaves the composition after
/// its key left the back stack (an entry that animated out: `prepare_back_stack` couldn't). A composable call in a
/// loop: a group of its own, keyed by the content key inside (entries swapped with another back stack's).
#[track_caller]
fn decorate_entry<T: NavEntryKey>(
    entry: &NavEntry<T>,
    decorators: &[NavEntryDecorator<T>],
    keys_in_backstack: &Rc<RefCell<HashSet<SlotKey>>>,
    keys_in_composition: &Rc<RefCell<HashSet<SlotKey>>>,
) -> NavEntry<T> {
    group(|| {
        let latest_decorators = remember_updated_state!(decorators.to_vec());
        let content_key = entry.content_key();
        key(&content_key.clone(), || {
            let (entry_inner, decorators) = (entry.clone(), decorators.to_vec());
            let (in_backstack, in_composition) = (keys_in_backstack.clone(), keys_in_composition.clone());
            // (Compose's compiler remembers the lambda while what it captures is the same: the entry stays equal)
            let entry = entry.clone();
            remember_keyed((entry.clone(), decorators.clone()), move || {
                NavEntry::of_entry(&entry, move |_: T| {
                    let (content_key, in_backstack, in_composition) =
                        (content_key.clone(), in_backstack.clone(), in_composition.clone());
                    let latest = latest_decorators.clone();
                    kompoz_runtime::kompozable_block!({
                        DisposableEffect(content_key.clone(), move || {
                            in_composition.borrow_mut().insert(content_key.clone());
                            on_dispose(move || {
                                let not_in_composition = in_composition.borrow_mut().remove(&content_key);
                                let popped = !in_backstack.borrow().contains(&content_key);
                                if popped && not_in_composition {
                                    // (the decorators' clean-ups in reverse, as their own disposals would go)
                                    for d in latest.get().iter().rev() {
                                        d.on_pop(content_key.clone());
                                    }
                                }
                            })
                        });
                    });
                    let mut wrapped = entry_inner.clone();
                    for decorator in distinct(&decorators).into_iter().rev() {
                        let inner = wrapped.clone();
                        wrapped = NavEntry::of_entry(&inner.clone(), move |_: T| decorator.decorate(inner.clone()));
                    }
                    wrapped.Content();
                })
            })
        })
    })
}

/// Compose's private `PrepareBackStack`: notes the back stack's content keys, and calls the decorators' `on_pop` (in
/// reverse) of a key that left it while its content isn't composed (never composed, or not animating out).
#[track_caller]
fn prepare_back_stack<T: NavEntryKey>(
    entries: &[NavEntry<T>],
    decorators: &[NavEntryDecorator<T>],
    keys_in_backstack: &Rc<RefCell<HashSet<SlotKey>>>,
    keys_in_composition: &Rc<RefCell<HashSet<SlotKey>>>,
) {
    group(|| {
        let latest_entries = remember_updated_state!(entries.to_vec());
        let latest_decorators = remember_updated_state!(decorators.to_vec());
        for entry in entries {
            let content_key = entry.content_key();
            keys_in_backstack.borrow_mut().insert(content_key.clone());
            let (in_backstack, in_composition) = (keys_in_backstack.clone(), keys_in_composition.clone());
            let (latest_entries, latest_decorators) = (latest_entries.clone(), latest_decorators.clone());
            group(|| {
                kompoz_runtime::kompozable_block!({
                    DisposableEffect((content_key.clone(), entries.to_vec()), move || {
                        on_dispose(move || {
                            let still = latest_entries.get().iter().any(|e| e.content_key() == content_key);
                            let popped = !still && in_backstack.borrow_mut().remove(&content_key);
                            if popped && !in_composition.borrow().contains(&content_key) {
                                for d in latest_decorators.get().iter().rev() {
                                    d.on_pop(content_key.clone());
                                }
                            }
                        })
                    });
                });
            });
        }
    })
}

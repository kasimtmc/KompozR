//! The index's overlay: the declarations a translation sees besides the API — its own text's classes, objects,
//! enum classes, type aliases, and the module table's (the crate's other Kotlin sources and its Rust items) — as
//! items of the index, so that the generator finds a class's constructor, properties and methods, an object's
//! members, an enum's entries where it finds the API's ([`super::lookup`] gives the overlay's items of a name
//! before the API's: a package's own declarations win over a star import's).
//!
//! The overlay is the thread's while a translation runs ([`install`]); its strings and parameter lists are interned
//! for the process ([`intern`], [`intern_params`]: the index's items are `'static`, and a translation made again
//! — rust-analyzer expanding a block at each edit — finds the same ones again rather than leaking new copies).

use super::{Item, Param};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{LazyLock, Mutex};

/// What a translation adds to the index.
#[derive(Clone, Debug, Default)]
pub struct Overlay {
    /// Items, any order (the translation's own first, then the module table's: the first of a name wins where one
    /// is chosen).
    pub items: Vec<Item>,
    /// Kotlin's qualified names of nested types and the Rust types they are (`Catalog.Entry` → `CatalogEntry`).
    pub types: Vec<(&'static str, &'static str)>,
    /// Type aliases: name → Rust's text of the target.
    pub aliases: Vec<(&'static str, &'static str)>,
    /// The user's interfaces (Rust's names): traits whose values are shared trait objects (`Rc<dyn I>`).
    pub interfaces: Vec<&'static str>,
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<Overlay>>> = const { RefCell::new(None) };
}

/// Puts back the overlay there was when it is dropped.
pub struct Installed(Option<Rc<Overlay>>);

impl Drop for Installed {
    fn drop(&mut self) {
        let previous = self.0.take();
        CURRENT.with(|c| *c.borrow_mut() = previous);
    }
}

/// Makes `overlay` the thread's until the guard is dropped.
pub fn install(overlay: Overlay) -> Installed {
    let previous = CURRENT.with(|c| c.borrow_mut().replace(Rc::new(overlay)));
    Installed(previous)
}

/// The thread's overlay, if a translation installed one.
pub fn current() -> Option<Rc<Overlay>> {
    CURRENT.with(|c| c.borrow().clone())
}

static STRINGS: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Default::default);
static PARAMS: LazyLock<Mutex<HashMap<String, &'static [Param]>>> = LazyLock::new(Default::default);
static MERGED: LazyLock<Mutex<HashMap<String, &'static [Item]>>> = LazyLock::new(Default::default);

/// `s` for the life of the process (one copy of each text).
pub fn intern(s: &str) -> &'static str {
    let mut set = STRINGS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = set.get(s) {
        return found;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(leaked);
    leaked
}

/// `params` for the life of the process (one copy of each list).
pub fn intern_params(params: Vec<Param>) -> &'static [Param] {
    if params.is_empty() {
        return &[];
    }
    let key = format!("{params:?}");
    let mut map = PARAMS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = map.get(&key) {
        return found;
    }
    let leaked: &'static [Param] = Box::leak(params.into_boxed_slice());
    map.insert(key, leaked);
    leaked
}

static STRS: LazyLock<Mutex<HashMap<Vec<&'static str>, &'static [&'static str]>>> = LazyLock::new(Default::default);

/// A list of strings for the life of the process (one copy of each list).
pub fn intern_strs(list: Vec<&'static str>) -> &'static [&'static str] {
    if list.is_empty() {
        return &[];
    }
    let mut map = STRS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = map.get(&list) {
        return found;
    }
    let leaked: &'static [&'static str] = Box::leak(list.clone().into_boxed_slice());
    map.insert(list, leaked);
    leaked
}

/// Whether a Rust type name is one of the user's interfaces.
pub fn is_interface(name: &str) -> bool {
    current().is_some_and(|o| o.interfaces.contains(&name))
}

/// The overlay's items of a Kotlin name followed by the API's (`api`); none when the overlay has none of it.
pub(super) fn merged(kotlin: &str, api: &'static [Item]) -> Option<&'static [Item]> {
    let overlay = current()?;
    let own: Vec<Item> = overlay.items.iter().filter(|i| i.kotlin == kotlin).copied().collect();
    if own.is_empty() {
        return None;
    }
    let all: Vec<Item> = own.into_iter().chain(api.iter().copied()).collect();
    let key = format!("{all:?}");
    let mut map = MERGED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = map.get(&key) {
        return Some(found);
    }
    let leaked: &'static [Item] = Box::leak(all.into_boxed_slice());
    map.insert(key, leaked);
    Some(leaked)
}

pub(super) fn rust_type(kotlin: &str) -> Option<&'static str> {
    current()?.types.iter().find(|(k, _)| *k == kotlin).map(|(_, r)| *r)
}

pub(super) fn kotlin_type(rust: &str) -> Option<&'static str> {
    current()?.types.iter().find(|(_, r)| *r == rust).map(|(k, _)| *k)
}

pub(super) fn alias(name: &str) -> Option<&'static str> {
    current()?.aliases.iter().find(|(a, _)| *a == name).map(|(_, t)| *t)
}

/// The overlay's items (none without one): what a scan of the index's whole list adds.
pub fn items() -> Vec<Item> {
    current().map(|o| o.items.clone()).unwrap_or_default()
}

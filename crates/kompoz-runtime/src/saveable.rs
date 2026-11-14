//! Saveable state: `remember_saveable` keeps its value while the composable is away from the composition, inside
//! a [`SaveableStateHolder`]'s provider (a navigation entry on the back stack, a tab that isn't shown). Values are
//! also written out with the host's state (a [`SaveableStateRegistry`] the host provides), which Android keeps
//! while it ends the app in the background, and come back with it. Outside any provider a value goes when its
//! composable leaves.

use crate::composer::{Composer, GroupId, key_of, mix_key, stable_hash};
use crate::local::{KompozitionLocal, KompozitionLocalProvider};
use crate::remember::remember;
use crate::saved_value::{Saveable, SavedValue};
use crate::state::MutableState;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::Location;
use std::rc::{Rc, Weak};

/// The values `remember_saveable` keeps under one provider, by their position below it.
#[derive(Default)]
struct Registry {
    values: HashMap<u64, (TypeId, Box<dyn Any>)>,
    /// How each value is written out.
    savers: HashMap<u64, Box<dyn Fn() -> SavedValue>>,
    /// From a saved state, for values not made yet.
    restored: HashMap<u64, SavedValue>,
    /// The provider's group: positions are counted from it down.
    boundary: Option<GroupId>,
    /// The host's: a value goes with its composable.
    forgets_leaving: bool,
    /// A holder's: the values whose composables left (kept if the provider left with them, as Compose saves a
    /// provider's values as it goes; else gone once the composition settles), and whether the provider is there.
    leaving: Vec<u64>,
    present: bool,
}

impl Registry {
    fn save(&self) -> SavedValue {
        let mut out: Vec<(String, SavedValue)> =
            self.restored.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(); // not back yet: kept
        out.extend(self.savers.iter().map(|(k, save)| (k.to_string(), save())));
        SavedValue::map(out)
    }
    fn restore(&mut self, saved: &SavedValue) {
        if let Some(map) = saved.as_map() {
            for (name, value) in map {
                if let Ok(key) = name.parse::<u64>() {
                    self.restored.insert(key, value.clone());
                }
            }
        }
    }
    fn drop_value(&mut self, key: u64) -> Option<(TypeId, Box<dyn Any>)> {
        self.savers.remove(&key);
        self.values.remove(&key)
    }
}

#[derive(Clone)]
struct RegistryRef(Rc<RefCell<Registry>>);

impl PartialEq for RegistryRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

static LocalSaveableRegistry: KompozitionLocal<Option<RegistryRef>> = KompozitionLocal::new(|| None);

thread_local! {
    /// Registries with values whose composables left.
    static LEAVING: RefCell<Vec<Weak<RefCell<Registry>>>> = const { RefCell::new(Vec::new()) };
}

fn note_leaving(registry: &Rc<RefCell<Registry>>, key: u64) {
    let mut r = registry.borrow_mut();
    if r.leaving.is_empty() {
        LEAVING.with(|l| l.borrow_mut().push(Rc::downgrade(registry)));
    }
    r.leaving.push(key);
}

/// After a composition, once what left it is forgotten: the values that left with their composables, not their
/// providers, gone.
pub(crate) fn settle_saveables() {
    let leaving = LEAVING.with(|l| std::mem::take(&mut *l.borrow_mut()));
    let mut dropped = Vec::new();
    for weak in leaving {
        if let Some(r) = weak.upgrade() {
            let mut r = r.borrow_mut();
            if r.present {
                // Its provider is still there: they are gone.
                for key in std::mem::take(&mut r.leaving) {
                    dropped.extend(r.drop_value(key));
                }
            }
            r.leaving.clear(); // (else kept for when it comes back, as Compose saves them)
        }
    }
    drop(dropped);
}

/// What `remember_saveable` keeps in its group: its entry in its registry, which goes with the group — at once
/// from the host's, from a holder's unless its provider goes too.
struct Entry {
    registry: Weak<RefCell<Registry>>,
    key: u64,
}

impl Drop for Entry {
    fn drop(&mut self) {
        if let Some(r) = self.registry.upgrade() {
            if r.borrow().forgets_leaving {
                let dropped = r.borrow_mut().drop_value(self.key);
                drop(dropped);
            } else {
                note_leaving(&r, self.key);
            }
        }
    }
}

/// How a value is written out and read back, given to `remember_saveable_with` (Compose's `Saver(save,
/// restore)`): for a type without [`Saveable`], or saved some other way.
pub struct Saver<T> {
    save: Rc<dyn Fn(&T) -> SavedValue>,
    restore: Rc<dyn Fn(&SavedValue) -> Option<T>>,
}

impl<T> Clone for Saver<T> {
    fn clone(&self) -> Self {
        Saver { save: self.save.clone(), restore: self.restore.clone() }
    }
}

impl<T: 'static> Saver<T> {
    pub fn new(
        save: impl Fn(&T) -> SavedValue + 'static,
        restore: impl Fn(&SavedValue) -> Option<T> + 'static,
    ) -> Self {
        Saver { save: Rc::new(save), restore: Rc::new(restore) }
    }

    /// The same saving a state of the value (Compose's stateSaver).
    pub fn for_state(&self) -> Saver<MutableState<T>>
    where
        T: PartialEq + Clone + Send + Sync,
    {
        let (save, restore) = (self.save.clone(), self.restore.clone());
        Saver::new(
            move |s: &MutableState<T>| s.peek_with(|v| save(v)),
            move |v| restore(v).map(crate::mutable_state_of),
        )
    }
}

/// A saver writing a list (Compose's listSaver).
pub fn list_saver<T: 'static>(
    save: impl Fn(&T) -> Vec<SavedValue> + 'static,
    restore: impl Fn(&[SavedValue]) -> Option<T> + 'static,
) -> Saver<T> {
    Saver::new(move |v| SavedValue::list(save(v)), move |s| s.as_list().and_then(&restore))
}

/// A saver writing a map (Compose's mapSaver).
pub fn map_saver<T: 'static>(
    save: impl Fn(&T) -> Vec<(String, SavedValue)> + 'static,
    restore: impl Fn(&std::collections::BTreeMap<String, SavedValue>) -> Option<T> + 'static,
) -> Saver<T> {
    Saver::new(move |v| SavedValue::map(save(v)), move |s| s.as_map().and_then(&restore))
}

/// `remember { … }` that a [`SaveableStateHolder`] keeps while the composable is away, and that comes back after
/// the process ended.
#[track_caller]
pub fn remember_saveable<T: Saveable + Clone + 'static>(init: impl FnOnce() -> T) -> T {
    remember_saveable_at(key_of(Location::caller()), Saver::new(T::save, T::restore), init, false)
}

/// The same, saved by `saver` (Compose's `rememberSaveable(saver = …)`).
#[track_caller]
pub fn remember_saveable_with<T: Clone + 'static>(saver: &Saver<T>, init: impl FnOnce() -> T) -> T {
    remember_saveable_at(key_of(Location::caller()), saver.clone(), init, false)
}

/// `rememberSaveable(inputs) { … }`: made again, the value it had and its saved one dropped, when `inputs`
/// change (several: a tuple).
#[track_caller]
pub fn remember_saveable_keyed<K: PartialEq + Clone + 'static, T: Saveable + Clone + 'static>(
    inputs: K,
    init: impl FnOnce() -> T,
) -> T {
    remember_saveable_keyed_with(inputs, &Saver::new(T::save, T::restore), init)
}

#[track_caller]
pub fn remember_saveable_keyed_with<K: PartialEq + Clone + 'static, T: Clone + 'static>(
    inputs: K,
    saver: &Saver<T>,
    init: impl FnOnce() -> T,
) -> T {
    let c = Composer::require();
    c.start_group(key_of(Location::caller()));
    let last = remember(|| Rc::new(RefCell::new(inputs.clone())));
    let changed = *last.borrow() != inputs;
    if changed {
        *last.borrow_mut() = inputs;
    }
    let v = remember_saveable_at(stable_hash("value"), saver.clone(), init, changed);
    c.end_group();
    v
}

fn remember_saveable_at<T: Clone + 'static>(key: u64, saver: Saver<T>, init: impl FnOnce() -> T, reset: bool) -> T {
    let c = Composer::require();
    let registry = LocalSaveableRegistry.current();
    c.start_group(key);
    if reset {
        // Its registry entry dropped or noted leaving now: made anew below.
        drop(c.take_slot());
    }
    let kept = c.take_slot().and_then(|slot| {
        let v = slot.value.downcast_ref::<(T, Rc<Entry>)>().map(|(v, _)| v.clone());
        c.put_slot(slot);
        v
    });
    let value = match kept {
        Some(v) => v,
        None => {
            let (value, entry) = match &registry {
                None => (init(), Entry { registry: Weak::new(), key: 0 }),
                Some(r) => {
                    let boundary = r.0.borrow().boundary;
                    let key = c.key_below(boundary);
                    let mut reg = r.0.borrow_mut();
                    let mut dropped = None;
                    if let Some(i) = reg.leaving.iter().position(|k| *k == key) {
                        // It left with its composable, its provider still there: a new one.
                        reg.leaving.remove(i);
                        dropped = reg.drop_value(key);
                    }
                    let back = reg
                        .values
                        .get(&key)
                        .filter(|(t, _)| *t == TypeId::of::<T>())
                        .and_then(|(_, v)| v.downcast_ref::<T>().cloned());
                    let restored = reg.restored.remove(&key);
                    drop(reg);
                    drop(dropped);
                    let value = match back {
                        Some(v) => v, // back again: the value it had
                        None => {
                            let v = restored.and_then(|s| (saver.restore)(&s)).unwrap_or_else(init); // after the process ended
                            let save = saver.save.clone();
                            let held = v.clone();
                            let mut reg = r.0.borrow_mut();
                            reg.values.insert(key, (TypeId::of::<T>(), Box::new(v.clone())));
                            reg.savers.insert(key, Box::new(move || save(&held)));
                            v
                        }
                    };
                    (value, Entry { registry: Rc::downgrade(&r.0), key })
                }
            };
            c.set_remembered(Box::new((value.clone(), Rc::new(entry))), None);
            value
        }
    };
    c.end_group();
    value
}

/// The host's registry: the values of the whole composition, written out with the host's state and given back
/// when the host is made again from it. Provide it at the root with [`ProvideSaveableStateRegistry`].
#[derive(Clone)]
pub struct SaveableStateRegistry {
    inner: Rc<RefCell<Registry>>,
}

impl SaveableStateRegistry {
    /// A registry giving back `saved` (what `save` wrote before the process ended).
    pub fn new(saved: Option<&SavedValue>) -> Self {
        let mut r = Registry { forgets_leaving: true, present: true, ..Default::default() };
        if let Some(saved) = saved {
            r.restore(saved);
        }
        SaveableStateRegistry { inner: Rc::new(RefCell::new(r)) }
    }

    pub fn save(&self) -> SavedValue {
        self.inner.borrow().save()
    }
}

/// Composes `content` with its `remember_saveable` values kept in `registry`.
#[allow(non_snake_case)]
#[track_caller]
pub fn ProvideSaveableStateRegistry(registry: &SaveableStateRegistry, content: impl FnOnce()) {
    let c = Composer::require();
    c.start_group(key_of(Location::caller()));
    registry.inner.borrow_mut().boundary = Some(c.current_group());
    KompozitionLocalProvider(&[LocalSaveableRegistry.provides(Some(RegistryRef(registry.inner.clone())))], content);
    c.end_group();
}

/// Keeps the `remember_saveable` values of content shown under different keys (Compose's
/// rememberSaveableStateHolder): `SaveableStateProvider(key) { … }` finds them again when that key comes back;
/// `remove_state(key)` forgets them.
#[derive(Clone, Default)]
pub struct SaveableStateHolder {
    registries: Rc<RefCell<HashMap<String, Rc<RefCell<Registry>>>>>,
    removed: Rc<RefCell<Vec<String>>>,
}

/// Marks a holder's registry as gone with its provider's group.
struct Presence(RefCell<Weak<RefCell<Registry>>>);

impl Drop for Presence {
    fn drop(&mut self) {
        if let Some(r) = self.0.borrow().upgrade() {
            r.borrow_mut().present = false;
        }
    }
}

impl SaveableStateHolder {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(non_snake_case)]
    #[track_caller]
    pub fn SaveableStateProvider(&self, key: &str, content: impl FnOnce()) {
        let removed = self.removed.borrow().iter().any(|k| k == key);
        let registry = if removed {
            // Content still shown under a removed key (leaving with an animation) keeps nothing new.
            Rc::new(RefCell::new(Registry::default()))
        } else {
            self.registries.borrow_mut().entry(key.to_string()).or_default().clone()
        };
        let c = Composer::require();
        c.start_group(mix_key(key_of(Location::caller()), stable_hash(key)));
        {
            let mut r = registry.borrow_mut();
            r.boundary = Some(c.current_group()); // positions are counted from here down
            r.present = true;
        }
        let presence = remember(|| Rc::new(Presence(RefCell::new(Weak::new()))));
        *presence.0.borrow_mut() = Rc::downgrade(&registry);
        KompozitionLocalProvider(&[LocalSaveableRegistry.provides(Some(RegistryRef(registry)))], content);
        c.end_group();
    }

    pub fn remove_state(&self, key: &str) {
        self.registries.borrow_mut().remove(key);
        self.removed.borrow_mut().push(key.to_string());
    }
}

/// The same holder, not the same values.
impl PartialEq for SaveableStateHolder {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.registries, &other.registries)
    }
}

/// Its keys' values, written out and read back.
impl Saveable for SaveableStateHolder {
    fn save(&self) -> SavedValue {
        SavedValue::map(self.registries.borrow().iter().map(|(k, r)| (k.clone(), r.borrow().save())))
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        let holder = SaveableStateHolder::new();
        for (key, values) in saved.as_map()? {
            let mut r = Registry::default();
            r.restore(values);
            holder.registries.borrow_mut().insert(key.clone(), Rc::new(RefCell::new(r)));
        }
        Some(holder)
    }
}

#[track_caller]
pub fn remember_saveable_state_holder() -> SaveableStateHolder {
    remember_saveable(SaveableStateHolder::new)
}

/// A state of a saveable value is saveable.
impl<T: Saveable + PartialEq + Clone + Send + Sync + 'static> Saveable for MutableState<T> {
    fn save(&self) -> SavedValue {
        self.peek_with(T::save)
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        T::restore(saved).map(crate::mutable_state_of)
    }
}

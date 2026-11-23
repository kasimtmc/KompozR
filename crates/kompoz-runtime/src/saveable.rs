//! Saveable state: `remember_saveable` keeps its value while the composable is away from the composition, inside
//! a [`SaveableStateHolder`]'s provider (a navigation entry on the back stack, a tab that isn't shown). Values are
//! also written out with the host's state (a [`SaveableStateRegistry`] the host provides), which Android keeps
//! while it ends the app in the background, and come back with it. Outside any provider a value goes when its
//! composable leaves. What isn't a composable's (a node's) registers its values by key with the registry
//! ([`SaveableStateRegistry::register_provider`], Compose's) and asks for them back by key.

use crate::composer::{Boundary, Composer, GroupId, key_of, mix_key, stable_hash};
use crate::kompozable;
use crate::local::{KompozitionLocal, provide_here};
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
    /// The provider's group: positions are counted from it down (through the child compositions below it too).
    boundary: Option<Boundary>,
    /// The positions of the values whose composables are there now, each with its place and entry: another one at
    /// the same position elsewhere (content of the same shape in two child compositions) takes the next free one, in
    /// the order they come; one at the same place (its content reused) takes it over, starting over.
    claimed: HashMap<u64, Claim>,
    /// The host's: a value goes with its composable.
    forgets_leaving: bool,
    /// A holder's: the values whose composables left (kept if the provider left with them, as Compose saves a
    /// provider's values as it goes; else gone once the composition settles), and whether the provider is there.
    leaving: Vec<u64>,
    present: bool,
    /// Values registered by key (Compose's `registerProvider`), in the order they were, each by its entry's id.
    providers: Vec<(String, u64, Rc<dyn Fn() -> Option<SavedValue>>)>,
    next_provider: u64,
    /// From a saved state, the values registered by key not consumed yet (Compose's `restored`).
    restored_by_key: HashMap<String, Vec<SavedValue>>,
    /// Which values may be saved (Compose's `canBeSaved`; none: any).
    can_be_saved: Option<Rc<dyn Fn(&SavedValue) -> bool>>,
}

/// Where a registry writes the values registered by key, beside those of `remember_saveable` (named by their
/// positions, all numbers).
const PROVIDED: &str = "providers";

impl Registry {
    fn save(&self) -> SavedValue {
        let mut out: Vec<(String, SavedValue)> =
            self.restored.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(); // not back yet: kept
        out.extend(self.savers.iter().map(|(k, save)| (k.to_string(), save())));
        // (Compose's performSave: what was restored and not consumed yet kept, then the providers' values)
        let mut by_key: std::collections::BTreeMap<String, Vec<SavedValue>> =
            self.restored_by_key.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let mut keys: Vec<&String> = Vec::new();
        for (k, _, _) in &self.providers {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        for key in keys {
            let values: Vec<Option<SavedValue>> =
                self.providers.iter().filter(|(k, _, _)| k == key).map(|(_, _, p)| p()).collect();
            for v in values.iter().flatten() {
                assert!(self.can_be_saved(v), "kompoz: a value registered under {key:?} can't be saved");
            }
            if values.len() == 1 {
                if let Some(v) = values.into_iter().next().flatten() {
                    by_key.insert(key.clone(), vec![v]);
                }
            } else {
                by_key.insert(key.clone(), values.into_iter().map(Option::unwrap_or_default).collect());
            }
        }
        if !by_key.is_empty() {
            out.push((
                PROVIDED.to_string(),
                SavedValue::map(by_key.into_iter().map(|(k, v)| (k, SavedValue::list(v)))),
            ));
        }
        SavedValue::map(out)
    }
    fn restore(&mut self, saved: &SavedValue) {
        if let Some(map) = saved.as_map() {
            for (name, value) in map {
                if let Ok(key) = name.parse::<u64>() {
                    self.restored.insert(key, value.clone());
                }
            }
            if let Some(provided) = saved.get(PROVIDED).as_map() {
                for (key, values) in provided {
                    self.restored_by_key.insert(key.clone(), values.as_list().unwrap_or_default().to_vec());
                }
            }
        }
    }
    fn can_be_saved(&self, value: &SavedValue) -> bool {
        self.can_be_saved.as_ref().is_none_or(|f| f(value))
    }
    fn drop_value(&mut self, key: u64) -> Option<(TypeId, Box<dyn Any>)> {
        self.savers.remove(&key);
        self.values.remove(&key)
    }
}

/// The registry `remember_saveable` and what registers values by key keep them in (Compose's
/// LocalSaveableStateRegistry): the host's, or a `SaveableStateHolder` provider's.
pub static LocalSaveableStateRegistry: KompozitionLocal<Option<SaveableStateRegistry>> =
    KompozitionLocal::new_static(|| None);

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

/// Who holds a position: the `remember_saveable` group (its composition's id and the group) and its entry.
#[derive(Clone, Copy, PartialEq)]
struct Claim {
    place: (u64, GroupId),
    entry: u64,
}

/// What `remember_saveable` keeps in its group: its entry in its registry, which goes with the group — at once
/// from the host's, from a holder's unless its provider goes too (unless another entry took its position over: its
/// content reused).
struct Entry {
    registry: Weak<RefCell<Registry>>,
    key: u64,
    id: u64,
}

impl Entry {
    fn next_id() -> u64 {
        thread_local! {
            static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
        }
        NEXT.with(|n| {
            let id = n.get();
            n.set(id + 1);
            id
        })
    }
}

impl Drop for Entry {
    fn drop(&mut self) {
        if let Some(r) = self.registry.upgrade() {
            let owner = r.borrow().claimed.get(&self.key).is_some_and(|c| c.entry == self.id);
            if !owner {
                return;
            }
            r.borrow_mut().claimed.remove(&self.key);
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

    /// Writes `value` out (Compose's `Saver.save`).
    pub fn save(&self, value: &T) -> SavedValue {
        (self.save)(value)
    }

    /// Reads a value back (Compose's `Saver.restore`): none if it can't.
    pub fn restore(&self, saved: &SavedValue) -> Option<T> {
        (self.restore)(saved)
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
///
/// Kotlin: `rememberSaveable(saver: Saver<T>, init: () -> T)`
#[track_caller]
pub fn remember_saveable_with<T: Clone + 'static>(saver: &Saver<T>, init: impl FnOnce() -> T) -> T {
    remember_saveable_at(key_of(Location::caller()), saver.clone(), init, false)
}

/// A state saved by `state_saver`, a saver of its value (Compose's `rememberSaveable(stateSaver = …) {
/// mutableStateOf(…) }`): after the process ends, a state of the value saved comes back.
///
/// Kotlin: `rememberSaveable(stateSaver: Saver<T>, init: () -> MutableState<T>)`
#[track_caller]
pub fn remember_saveable_state<T: PartialEq + Clone + Send + Sync + 'static>(
    state_saver: &Saver<T>,
    init: impl FnOnce() -> MutableState<T>,
) -> MutableState<T> {
    remember_saveable_at(key_of(Location::caller()), state_saver.for_state(), init, false)
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
    let registry = LocalSaveableStateRegistry.current();
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
            let (value, entry) = match registry.as_ref().map(|r| &r.inner) {
                None => (init(), Entry { registry: Weak::new(), key: 0, id: 0 }),
                Some(r) => {
                    let boundary = r.borrow().boundary;
                    let mut key = c.key_below(boundary);
                    let place = (c.id, c.current_group());
                    let entry_id = Entry::next_id();
                    let mut reg = r.borrow_mut();
                    let mut replaced = None;
                    while let Some(claim) = reg.claimed.get(&key).copied() {
                        if claim.place == place {
                            // (its content reused here: the value it had is gone, a new one made)
                            replaced = reg.drop_value(key);
                            break;
                        }
                        key = mix_key(key, 1);
                    }
                    reg.claimed.insert(key, Claim { place, entry: entry_id });
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
                    drop((dropped, replaced));
                    let value = match back {
                        Some(v) => v, // back again: the value it had
                        None => {
                            let v = restored.and_then(|s| (saver.restore)(&s)).unwrap_or_else(init); // after the process ended
                            let save = saver.save.clone();
                            let held = v.clone();
                            let mut reg = r.borrow_mut();
                            reg.values.insert(key, (TypeId::of::<T>(), Box::new(v.clone())));
                            reg.savers.insert(key, Box::new(move || save(&held)));
                            v
                        }
                    };
                    (value, Entry { registry: Rc::downgrade(r), key, id: entry_id })
                }
            };
            c.set_remembered(Box::new((value.clone(), Rc::new(entry))), None);
            value
        }
    };
    c.end_group();
    value
}

/// A registry of saveable values (Compose's SaveableStateRegistry): the host's (the values of the whole composition,
/// written out with the host's state and given back when the host is made again from it; provide it at the root
/// with [`ProvideSaveableStateRegistry`]) or a [`SaveableStateHolder`] provider's. Besides `remember_saveable`'s, it
/// keeps values registered by key ([`register_provider`](SaveableStateRegistry::register_provider)), asked for as
/// it saves, and gives back those it was restored with ([`consume_restored`](SaveableStateRegistry::consume_restored)).
/// Clones are the same registry.
#[derive(Clone)]
pub struct SaveableStateRegistry {
    inner: Rc<RefCell<Registry>>,
}

impl PartialEq for SaveableStateRegistry {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

/// What [`SaveableStateRegistry::register_provider`] gives (Compose's SaveableStateRegistry.Entry): unregister it
/// when the value goes.
pub struct SaveableStateRegistryEntry {
    registry: Weak<RefCell<Registry>>,
    id: u64,
}

impl SaveableStateRegistryEntry {
    /// The value no longer saved (Compose's `unregister`).
    pub fn unregister(self) {
        if let Some(r) = self.registry.upgrade() {
            let removed = {
                let mut r = r.borrow_mut();
                let at = r.providers.iter().position(|(_, id, _)| *id == self.id);
                at.map(|i| r.providers.remove(i))
            };
            drop(removed);
        }
    }
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

    /// The same, saving only the values `can_be_saved` takes (Compose's `SaveableStateRegistry(restored,
    /// canBeSaved)`).
    pub fn with_can_be_saved(saved: Option<&SavedValue>, can_be_saved: impl Fn(&SavedValue) -> bool + 'static) -> Self {
        let registry = SaveableStateRegistry::new(saved);
        registry.inner.borrow_mut().can_be_saved = Some(Rc::new(can_be_saved));
        registry
    }

    /// What it holds, written out (Compose's `performSave`): the values of its `remember_saveable`s and of its
    /// providers, and what it was restored with and wasn't asked for yet.
    pub fn save(&self) -> SavedValue {
        self.inner.borrow().save()
    }

    /// Compose's name of [`save`](SaveableStateRegistry::save).
    pub fn perform_save(&self) -> SavedValue {
        self.save()
    }

    /// Gives back, once, the value saved under `key` (the first of them if several were), if it was restored with
    /// one (Compose's `consumeRestored`).
    pub fn consume_restored(&self, key: &str) -> Option<SavedValue> {
        let mut r = self.inner.borrow_mut();
        let values = r.restored_by_key.get_mut(key)?;
        let value = (!values.is_empty()).then(|| values.remove(0));
        if values.is_empty() {
            r.restored_by_key.remove(key);
        }
        value
    }

    /// Saves what `provider` gives (none: nothing) under `key` whenever the registry is saved, until the entry is
    /// unregistered (Compose's `registerProvider`). Panics on a blank key.
    pub fn register_provider(
        &self,
        key: &str,
        provider: impl Fn() -> Option<SavedValue> + 'static,
    ) -> SaveableStateRegistryEntry {
        assert!(!key.trim().is_empty(), "kompoz: registered key is empty or blank");
        let mut r = self.inner.borrow_mut();
        let id = r.next_provider;
        r.next_provider += 1;
        r.providers.push((key.to_string(), id, Rc::new(provider)));
        SaveableStateRegistryEntry { registry: Rc::downgrade(&self.inner), id }
    }

    /// Whether it can save `value` (Compose's `canBeSaved`).
    pub fn can_be_saved(&self, value: &SavedValue) -> bool {
        self.inner.borrow().can_be_saved(value)
    }

    /// Takes `saved` as what to give back (C++'s `restoreState` of a host made already, before its first frame):
    /// what composes from now on finds it.
    pub fn restore(&self, saved: &SavedValue) {
        self.inner.borrow_mut().restore(saved);
    }
}

/// Composes `content` with its `remember_saveable` values kept in `registry`.
#[kompozable(inline)]
pub fn ProvideSaveableStateRegistry(registry: &SaveableStateRegistry, content: impl FnOnce()) {
    registry.inner.borrow_mut().boundary = Some(Composer::require().boundary_here());
    provide_here(vec![LocalSaveableStateRegistry.provides(Some(registry.clone())).0], content);
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
            r.boundary = Some(c.boundary_here()); // positions are counted from here down
            r.present = true;
        }
        let presence = remember(|| Rc::new(Presence(RefCell::new(Weak::new()))));
        *presence.0.borrow_mut() = Rc::downgrade(&registry);
        let registry = SaveableStateRegistry { inner: registry };
        provide_here(vec![LocalSaveableStateRegistry.provides(Some(registry)).0], content);
        c.end_group();
    }

    /// [`SaveableStateProvider`](Self::SaveableStateProvider) as Compose's is made: in `ReusableContent(key)`, so
    /// content shown under one key and then another at the same place (a lazy layout's item slot reused for
    /// another item) keeps its nodes, its state going with its key and coming back with it.
    #[allow(non_snake_case)]
    #[track_caller]
    pub fn ReusableSaveableStateProvider(&self, key: &str, content: impl FnOnce()) {
        let removed = self.removed.borrow().iter().any(|k| k == key);
        let registry = if removed {
            Rc::new(RefCell::new(Registry::default()))
        } else {
            self.registries.borrow_mut().entry(key.to_string()).or_default().clone()
        };
        let c = Composer::require();
        c.start_group(key_of(Location::caller()));
        // (ReusableContent(key): the key kept in a slot; another one composes what follows as new, nodes reused)
        let changed = match c.take_slot() {
            Some(mut slot) => {
                let changed = match slot.value.downcast_mut::<String>() {
                    Some(old) if old == key => false,
                    Some(old) => {
                        *old = key.to_string();
                        true
                    }
                    None => true,
                };
                c.put_slot(slot);
                changed
            }
            None => {
                c.set_remembered(Box::new(key.to_string()), None);
                false
            }
        };
        if changed {
            c.begin_reuse();
        }
        {
            let mut r = registry.borrow_mut();
            r.boundary = Some(c.boundary_here()); // positions are counted from here down
            r.present = true;
        }
        let presence = remember(|| Rc::new(Presence(RefCell::new(Weak::new()))));
        *presence.0.borrow_mut() = Rc::downgrade(&registry);
        let registry = SaveableStateRegistry { inner: registry };
        provide_here(vec![LocalSaveableStateRegistry.provides(Some(registry)).0], content);
        if changed {
            c.end_reuse();
        }
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

/// A state list of saveable values is saveable (Compose's `rememberSaveable { mutableStateListOf(…) }`: its
/// SnapshotStateList is Parcelable on Android, SnapshotStateList.android.kt), its elements written out.
impl<T: Saveable + Clone + Send + Sync + 'static> Saveable for crate::SnapshotStateList<T> {
    fn save(&self) -> SavedValue {
        self.peek_with(|items| SavedValue::list(items.iter().map(T::save)))
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        Vec::<T>::restore(saved).map(crate::SnapshotStateList::new)
    }
}

/// A state map of saveable keys and values is saveable (savedstate-compose's `SnapshotStateMapSerializer`; Compose's
/// SnapshotStateMap is Parcelable on Android): its entries written out as pairs, in their order.
impl<K: Saveable + Ord + Clone + Send + Sync + 'static, V: Saveable + Clone + Send + Sync + 'static> Saveable
    for crate::SnapshotStateMap<K, V>
{
    fn save(&self) -> SavedValue {
        self.peek_with(|entries| SavedValue::list(entries.iter().map(|(k, v)| SavedValue::list([k.save(), v.save()]))))
    }
    fn restore(saved: &SavedValue) -> Option<Self> {
        let entries = saved
            .as_list()?
            .iter()
            .map(|pair| match pair.as_list()? {
                [k, v] => Some((K::restore(k)?, V::restore(v)?)),
                _ => None,
            })
            .collect::<Option<std::collections::BTreeMap<K, V>>>()?;
        Some(crate::SnapshotStateMap::new(entries))
    }
}

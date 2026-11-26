//! lifecycle-viewmodel-savedstate's SavedStateHandle.kt (SavedStateHandleImpl, its non-Android actual),
//! SavedStateHandleSupport.kt, SavedStateViewModelFactory.kt, SavedStateHandleController.kt and serialization's
//! SavedStateHandleDelegate.kt; viewmodel-compose's SavedStateHandleSaver.kt (`saveable`).

use super::provider::{CreationExtras, CreationExtrasKey, VIEW_MODEL_KEY, ViewModelProvider, ViewModelProviderFactory};
use super::store::{ViewModelStore, ViewModelStoreOwner};
use super::{KClass, ViewModel, ViewModelImpl};
use crate::lifecycle::{LifecycleEvent, LifecycleEventObserver, LifecycleObserver, LifecycleOwner, LifecycleState};
use crate::saved_state::{
    SavedState, SavedStateDelegate, SavedStateProvider, SavedStateRegistry, SavedStateRegistryOwner, saved_state,
    saved_state_of,
};
use kompoz_coroutines::{MutableStateFlow, StateFlow};
use kompoz_runtime::{MutableState, Saveable, SavedValue, Saver};
use std::any::Any;
use std::cell::{Cell, OnceCell, RefCell};
use std::rc::{Rc, Weak};

// ---- SavedStateHandle ----

/// A state flow the handle keeps for a key: the flow, how a value set is given to it, what it holds now.
struct FlowEntry {
    flow: Rc<dyn Any>,
    set: Box<dyn Fn(&SavedValue)>,
    get: Box<dyn Fn() -> SavedValue>,
}

#[derive(Default)]
struct HandleImpl {
    /// The values by key, in the order first set (Kotlin's `regular`).
    regular: Vec<(String, SavedValue)>,
    providers: Vec<(String, Rc<dyn SavedStateProvider>)>,
    flows: Vec<(String, Rc<FlowEntry>)>,
    mutable_flows: Vec<(String, Rc<FlowEntry>)>,
}

fn find<'a, V>(entries: &'a [(String, V)], key: &str) -> Option<&'a V> {
    entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn put<V>(entries: &mut Vec<(String, V)>, key: &str, value: V) {
    match entries.iter_mut().find(|(k, _)| k == key) {
        Some((_, v)) => *v = value,
        None => entries.push((key.to_string(), value)),
    }
}

fn take<V>(entries: &mut Vec<(String, V)>, key: &str) -> Option<V> {
    entries.iter().position(|(k, _)| k == key).map(|i| entries.remove(i).1)
}

/// A ViewModel's values that outlive the process (Compose's `SavedStateHandle`): by key, saved with its owner's
/// state (the app's, a navigation entry's) and given back to the ViewModel made again after the platform ended the
/// app; a navigation entry's starts with its arguments. Values are of [`Saveable`] types (Kotlin's types a Bundle
/// holds); each key can be watched as a [`StateFlow`] ([`get_state_flow`](Self::get_state_flow)). A clone is the
/// same handle.
///
/// ```
/// # use kompoz_lifecycle::*;
/// let state = SavedStateHandle::new();
/// let query = state.get_state_flow("query", String::new());
/// state.set("query", "kedi".to_string());
/// assert_eq!(query.value(), "kedi");
/// assert_eq!(state.get::<String>("query").as_deref(), Some("kedi"));
/// ```
#[derive(Clone, Default)]
pub struct SavedStateHandle(Rc<RefCell<HandleImpl>>);

impl PartialEq for SavedStateHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for SavedStateHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SavedStateHandle(keys={:?})", self.keys())
    }
}

impl SavedStateHandle {
    /// An empty handle (Compose's `SavedStateHandle()`, VisibleForTesting: a ViewModel gets its own from its
    /// factory).
    pub fn new() -> SavedStateHandle {
        SavedStateHandle::default()
    }

    /// A handle of `initial_state`'s values (Compose's `SavedStateHandle(initialState)`).
    ///
    /// Kotlin: `SavedStateHandle(initialState: Map<String, Any?>)`
    pub fn new_with<K: Into<String>>(initial_state: impl IntoIterator<Item = (K, SavedValue)>) -> SavedStateHandle {
        let handle = SavedStateHandle::new();
        handle.0.borrow_mut().regular = initial_state.into_iter().map(|(k, v)| (k.into(), v)).collect();
        handle
    }

    /// A handle of `restored_state` (what one saved before the process ended), else of `default_state` (a
    /// navigation entry's arguments) (Compose's `createHandle`, LIBRARY_GROUP).
    pub fn create_handle(restored_state: Option<&SavedState>, default_state: Option<&SavedState>) -> SavedStateHandle {
        match restored_state.or(default_state) {
            None => SavedStateHandle::new(),
            Some(initial) => SavedStateHandle::new_with(initial.read(|r| r.to_map())),
        }
    }

    /// Whether a value of `T`'s type can be kept (Compose's `validateValue`): every [`Saveable`] can.
    pub fn validate_value(value: &SavedValue) -> bool {
        let _ = value;
        true
    }

    /// What saves its values (Compose's `savedStateProvider()`, LIBRARY_GROUP): the state flows' values written into
    /// them first, and the states of the providers set on it.
    pub fn saved_state_provider(&self) -> Rc<dyn SavedStateProvider> {
        let me = self.clone();
        Rc::new(move || me.save())
    }

    fn save(&self) -> SavedState {
        let mutable_flows: Vec<(String, Rc<FlowEntry>)> = self.0.borrow().mutable_flows.clone();
        for (key, flow) in mutable_flows {
            let value = (flow.get)();
            self.set_value(&key, value);
        }
        let providers: Vec<(String, Rc<dyn SavedStateProvider>)> = self.0.borrow().providers.clone();
        for (key, provider) in providers {
            let state = provider.save_state();
            self.set_value(&key, state.to_saved_value());
        }
        let regular = self.0.borrow().regular.clone();
        saved_state_of(regular, |_| {})
    }

    /// Whether it has a value of `key` (Compose's `contains` operator).
    pub fn contains(&self, key: &str) -> bool {
        find(&self.0.borrow().regular, key).is_some()
    }

    /// Its value of `key` as a [`StateFlow`], the value given whenever it is set (Compose's `getStateFlow`):
    /// `initial_value` (set as its value) if it has none. Asked again, the same flow.
    ///
    /// # Panics
    /// If the key's flow is of another type (Kotlin's unchecked cast).
    pub fn get_state_flow<T: Saveable + Clone + PartialEq + Send + 'static>(
        &self,
        key: &str,
        initial_value: T,
    ) -> StateFlow<T> {
        self.flow(key, initial_value, false).as_state_flow()
    }

    /// Its value of `key` as a [`MutableStateFlow`] (Compose's `getMutableStateFlow`): what is set to the flow is
    /// its value (saved, and read by [`get`](Self::get)), what is set to it the flow's.
    pub fn get_mutable_state_flow<T: Saveable + Clone + PartialEq + Send + 'static>(
        &self,
        key: &str,
        initial_value: T,
    ) -> MutableStateFlow<T> {
        self.flow(key, initial_value, true)
    }

    fn flow<T: Saveable + Clone + PartialEq + Send + 'static>(
        &self,
        key: &str,
        initial_value: T,
        mutable: bool,
    ) -> MutableStateFlow<T> {
        let existing = {
            let h = self.0.borrow();
            find(if mutable { &h.mutable_flows } else { &h.flows }, key).cloned()
        };
        if let Some(entry) = existing {
            return entry.flow.clone().downcast::<MutableStateFlow<T>>().map(|f| (*f).clone()).unwrap_or_else(|_| {
                panic!("ClassCastException: the flow of {key:?} isn't a flow of {}", std::any::type_name::<T>())
            });
        }
        let mut h = self.0.borrow_mut();
        if find(&h.regular, key).is_none() {
            put(&mut h.regular, key, initial_value.save());
        }
        let value = find(&h.regular, key).and_then(T::restore).unwrap_or(initial_value);
        let flow = MutableStateFlow::new(value);
        let (setter, getter) = (flow.clone(), flow.clone());
        let entry = Rc::new(FlowEntry {
            flow: Rc::new(flow.clone()),
            set: Box::new(move |v| {
                if let Some(v) = T::restore(v) {
                    setter.set_value(v);
                }
            }),
            get: Box::new(move || getter.value().save()),
        });
        put(if mutable { &mut h.mutable_flows } else { &mut h.flows }, key, entry);
        flow
    }

    /// Its keys, in the order first set, then those of its providers (Compose's `keys()`).
    pub fn keys(&self) -> Vec<String> {
        let h = self.0.borrow();
        let mut keys: Vec<String> = h.regular.iter().map(|(k, _)| k.clone()).collect();
        for (k, _) in &h.providers {
            if !keys.contains(k) {
                keys.push(k.clone());
            }
        }
        keys
    }

    /// Its value of `key`, if it has one that is a `T` (Compose's `get` operator; one of another type is kept, as
    /// Kotlin's unchecked cast keeps it).
    pub fn get<T: Saveable>(&self, key: &str) -> Option<T> {
        let mutable = find(&self.0.borrow().mutable_flows, key).cloned();
        let value = match mutable {
            Some(flow) => (flow.get)(),
            None => find(&self.0.borrow().regular, key).cloned()?,
        };
        T::restore(&value)
    }

    /// `value` as its value of `key`, given to the key's state flows (Compose's `set` operator, `state[key] = v`).
    pub fn set<T: Saveable>(&self, key: &str, value: T) {
        self.set_value(key, value.save());
    }

    /// The same of a value as it is held.
    pub fn set_value(&self, key: &str, value: SavedValue) {
        let (flow, mutable_flow) = {
            let mut h = self.0.borrow_mut();
            put(&mut h.regular, key, value.clone());
            (find(&h.flows, key).cloned(), find(&h.mutable_flows, key).cloned())
        };
        if let Some(f) = flow {
            (f.set)(&value);
        }
        if let Some(f) = mutable_flow {
            (f.set)(&value);
        }
    }

    /// Forgets the value of `key` and its flows, giving the value if it was a `T` (Compose's `remove`).
    pub fn remove<T: Saveable>(&self, key: &str) -> Option<T> {
        let latest = {
            let mut h = self.0.borrow_mut();
            take(&mut h.flows, key);
            take(&mut h.mutable_flows, key);
            take(&mut h.regular, key)
        };
        latest.as_ref().and_then(T::restore)
    }

    /// Saves under `key` what `provider` gives (Compose's `setSavedStateProvider`), replacing the one there.
    pub fn set_saved_state_provider(&self, key: &str, provider: Rc<dyn SavedStateProvider>) {
        put(&mut self.0.borrow_mut().providers, key, provider);
    }

    /// No longer saves what the provider of `key` gives (Compose's `clearSavedStateProvider`).
    pub fn clear_saved_state_provider(&self, key: &str) {
        let removed = take(&mut self.0.borrow_mut().providers, key);
        drop(removed);
    }

    /// A value kept in it under `key` (lifecycle-viewmodel-savedstate's `val user by handle.saved(key) { … }`):
    /// read, it is what was saved before the process ended, else `init()`'s. (Kotlin's key defaults to the
    /// property's qualified name, which Rust can't name: it is given.)
    ///
    /// Kotlin: `saved(key: String, init: () -> T)`
    pub fn saved<T: Saveable + Clone + 'static>(
        &self,
        key: &str,
        init: impl FnOnce() -> T + 'static,
    ) -> SavedStateDelegate<T> {
        self.saved_with(Saver::new(|v: &T| v.save(), T::restore), key, init)
    }

    /// The same, written by `serializer`.
    ///
    /// Kotlin: `saved(serializer: KSerializer<T>, key: String, init: () -> T)`
    pub fn saved_with<T: Clone + 'static>(
        &self,
        serializer: impl kompoz_runtime::IntoSaver<T>,
        key: &str,
        init: impl FnOnce() -> T + 'static,
    ) -> SavedStateDelegate<T> {
        let (register, load) = (self.clone(), self.clone());
        SavedStateDelegate::new(
            key.to_string(),
            serializer.into_saver(),
            init,
            move |key, provider| register.set_saved_state_provider(key, provider),
            move |key| load.get::<SavedState>(key),
        )
    }

    /// A value saved in it under `key` (viewmodel-compose's `handle.saveable(key) { … }`, SavedStateHandleSaveableApi):
    /// what was saved before the process ended, else `init()`'s; saved, as it then is, with the handle.
    ///
    /// Kotlin: `saveable(key: String, saver: Saver<T, out Any> = autoSaver(), init: () -> T)`
    pub fn saveable<T: Saveable + Clone + 'static>(&self, key: &str, init: impl FnOnce() -> T) -> T {
        self.saveable_with(key, Saver::new(|v: &T| v.save(), T::restore), init)
    }

    /// The same, written by `saver`.
    ///
    /// Kotlin: `saveable(key: String, saver: Saver<T, out Any>, init: () -> T)`
    pub fn saveable_with<T: Clone + 'static>(&self, key: &str, saver: Saver<T>, init: impl FnOnce() -> T) -> T {
        let restored =
            self.get::<SavedState>(key).and_then(|s| s.read(|r| r.get_value("value"))).and_then(|v| saver.restore(&v));
        let value = restored.unwrap_or_else(init);
        let saved = value.clone();
        self.set_saved_state_provider(key, Rc::new(move || saved_state_of([("value", saver.save(&saved))], |_| {})));
        value
    }

    /// A state saved in it under `key`, its value written by `state_saver` (viewmodel-compose's
    /// `handle.saveable(key, stateSaver) { mutableStateOf(…) }`).
    ///
    /// Kotlin: `saveable(key: String, stateSaver: Saver<T, out Any>, init: () -> MutableState<T>)`
    pub fn saveable_state<T: PartialEq + Clone + Send + Sync + 'static>(
        &self,
        key: &str,
        state_saver: Saver<T>,
        init: impl FnOnce() -> MutableState<T>,
    ) -> MutableState<T> {
        self.saveable_with(key, state_saver.for_state(), init)
    }
}

// ---- SavedStateHandleSupport ----

/// Where the SavedStateHandles' ViewModel is kept in its owner's store (Compose's `VIEWMODEL_KEY`).
const VIEWMODEL_KEY: &str = "androidx.lifecycle.internal.SavedStateHandlesVM";
/// Where the handles' provider is registered in the owner's registry (Compose's `SAVED_STATE_KEY`).
pub(crate) const SAVED_STATE_KEY: &str = "androidx.lifecycle.internal.SavedStateHandlesProvider";

/// The [`SavedStateRegistryOwner`] of the ViewModel being made (Compose's `SAVED_STATE_REGISTRY_OWNER_KEY`).
pub const SAVED_STATE_REGISTRY_OWNER_KEY: CreationExtrasKey<Rc<dyn SavedStateRegistryOwner>> =
    CreationExtrasKey::named("SAVED_STATE_REGISTRY_OWNER_KEY");

/// The [`ViewModelStoreOwner`] of the ViewModel being made (Compose's `VIEW_MODEL_STORE_OWNER_KEY`).
pub const VIEW_MODEL_STORE_OWNER_KEY: CreationExtrasKey<Rc<dyn ViewModelStoreOwner>> =
    CreationExtrasKey::named("VIEW_MODEL_STORE_OWNER_KEY");

/// The default values of the ViewModel's [`SavedStateHandle`] (a navigation entry's arguments) (Compose's
/// `DEFAULT_ARGS_KEY`).
pub const DEFAULT_ARGS_KEY: CreationExtrasKey<SavedState> = CreationExtrasKey::named("DEFAULT_ARGS_KEY");

/// The handles of an owner's ViewModels, by their keys (Compose's internal `SavedStateHandlesVM`).
#[derive(Default)]
struct SavedStateHandlesVM {
    base: ViewModelImpl,
    handles: RefCell<Vec<(String, SavedStateHandle)>>,
}

impl ViewModel for SavedStateHandlesVM {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
}

/// The factory of [`SavedStateHandlesVM`].
struct HandlesVmFactory;

impl ViewModelProviderFactory for HandlesVmFactory {
    fn create(&self, _model_class: &KClass, _extras: &CreationExtras) -> Rc<dyn ViewModel> {
        Rc::new(SavedStateHandlesVM::default())
    }
}

fn saved_state_handles_vm(store: &ViewModelStore) -> Rc<SavedStateHandlesVM> {
    ViewModelProvider::create_from_store(store.clone(), Some(Rc::new(HandlesVmFactory)), None)
        .get_keyed::<SavedStateHandlesVM>(VIEWMODEL_KEY)
}

/// Saves the handles of an owner's ViewModels (Compose's internal `SavedStateHandlesProvider`): what was restored
/// and not claimed yet, then each handle's values.
struct SavedStateHandlesProvider {
    saved_state_registry: SavedStateRegistry,
    store: ViewModelStore,
    restored: Cell<bool>,
    restored_state: RefCell<Option<SavedState>>,
    view_model: OnceCell<Rc<SavedStateHandlesVM>>,
}

impl SavedStateHandlesProvider {
    fn view_model(&self) -> Rc<SavedStateHandlesVM> {
        self.view_model.get_or_init(|| saved_state_handles_vm(&self.store)).clone()
    }

    fn perform_restore(&self) {
        if !self.restored.get() {
            let new_state = self.saved_state_registry.consume_restored_state_for_key(SAVED_STATE_KEY);
            let old = self.restored_state.borrow().clone();
            *self.restored_state.borrow_mut() = Some(saved_state(|w| {
                if let Some(old) = &old {
                    w.put_all(old);
                }
                if let Some(new_state) = &new_state {
                    w.put_all(new_state);
                }
            }));
            self.restored.set(true);
            // (the ViewModel got now, so that the handles are saved after the lifecycle is destroyed too)
            self.view_model();
        }
    }

    fn consume_restored_state_for_key(&self, key: &str) -> Option<SavedState> {
        self.perform_restore();
        let state = self.restored_state.borrow().clone()?;
        if !state.read(|r| r.contains(key)) {
            return None;
        }
        let result = state.read(|r| r.get_saved_state_or_null(key)).unwrap_or_default();
        state.write(|w| w.remove(key));
        if state.read(|r| r.is_empty()) {
            *self.restored_state.borrow_mut() = None;
        }
        Some(result)
    }
}

impl SavedStateProvider for SavedStateHandlesProvider {
    fn save_state(&self) -> SavedState {
        let restored = self.restored_state.borrow().clone();
        let handles = self.view_model().handles.borrow().clone();
        let out = saved_state(|w| {
            if let Some(r) = &restored {
                w.put_all(r);
            }
            for (key, handle) in &handles {
                let state = handle.saved_state_provider().save_state();
                if !state.read(|r| r.is_empty()) {
                    w.put_saved_state(key, &state);
                }
            }
        });
        self.restored.set(false);
        out
    }
}

/// Restores the handles' provider as the owner is created (Compose's internal `SavedStateHandleAttacher`).
struct SavedStateHandleAttacher {
    me: Weak<SavedStateHandleAttacher>,
    provider: Rc<SavedStateHandlesProvider>,
}

impl LifecycleObserver for SavedStateHandleAttacher {
    fn as_lifecycle_event_observer(&self) -> Option<&dyn LifecycleEventObserver> {
        Some(self)
    }
}

impl LifecycleEventObserver for SavedStateHandleAttacher {
    fn on_state_changed(&self, source: &dyn LifecycleOwner, event: LifecycleEvent) {
        assert!(
            event == LifecycleEvent::OnCreate,
            "IllegalStateException: Next event must be ON_CREATE, it was {event}"
        );
        if let Some(me) = self.me.upgrade() {
            source.lifecycle().remove_observer(me);
        }
        self.provider.perform_restore();
    }
}

/// lifecycle-viewmodel-savedstate's SavedStateHandleSupport.kt `enableSavedStateHandles()`, for an owner of both a
/// saved state registry and a ViewModel store.
pub trait SavedStateHandleSupport: SavedStateRegistryOwner + ViewModelStoreOwner {
    /// Lets the owner's ViewModels have [`SavedStateHandle`]s ([`CreationExtras::create_saved_state_handle`]): their
    /// handles saved with the owner's state (Compose's `enableSavedStateHandles()`); called once, while the owner
    /// is `INITIALIZED` or `CREATED` (again: nothing).
    ///
    /// # Panics
    /// When the owner is started.
    #[track_caller]
    fn enable_saved_state_handles(&self) {
        enable_saved_state_handles(self);
    }
}

impl<T: SavedStateRegistryOwner + ViewModelStoreOwner + ?Sized> SavedStateHandleSupport for T {}

#[track_caller]
fn enable_saved_state_handles<T: SavedStateRegistryOwner + ViewModelStoreOwner + ?Sized>(owner: &T) {
    let current_state = owner.lifecycle().current_state();
    assert!(
        current_state == LifecycleState::Initialized || current_state == LifecycleState::Created,
        "IllegalArgumentException: Failed to enable `SavedStateHandle`. The `Lifecycle.State` must be `INITIALIZED` \
         or `CREATED`, but was `{current_state}`. You must call `enableSavedStateHandles()` before the \
         `Lifecycle.State` moves to `STARTED`."
    );
    let registry = owner.saved_state_registry();
    if registry.get_saved_state_provider(SAVED_STATE_KEY).is_none() {
        let provider = Rc::new(SavedStateHandlesProvider {
            saved_state_registry: registry.clone(),
            store: owner.view_model_store(),
            restored: Cell::new(false),
            restored_state: RefCell::new(None),
            view_model: OnceCell::new(),
        });
        registry.register_saved_state_provider(SAVED_STATE_KEY, provider.clone());
        let attacher: Rc<dyn LifecycleObserver> =
            Rc::new_cyclic(|me: &Weak<SavedStateHandleAttacher>| SavedStateHandleAttacher { me: me.clone(), provider });
        owner.lifecycle().add_observer(attacher);
    }
}

fn saved_state_handles_provider(owner: &dyn SavedStateRegistryOwner) -> Rc<SavedStateHandlesProvider> {
    let provider = owner.saved_state_registry().get_saved_state_provider(SAVED_STATE_KEY);
    let any: Option<Rc<dyn Any>> = provider.map(|p| p as Rc<dyn Any>);
    any.and_then(|p| p.downcast::<SavedStateHandlesProvider>().ok()).unwrap_or_else(|| {
        panic!("IllegalStateException: enableSavedStateHandles() wasn't called prior to createSavedStateHandle() call")
    })
}

impl CreationExtras {
    /// The [`SavedStateHandle`] of the ViewModel being made (Compose's `CreationExtras.createSavedStateHandle()`): the
    /// one its owner kept for its key, else one of what was saved under that key before the process ended, else of
    /// the default arguments. Needs the owner's [`enable_saved_state_handles`](SavedStateHandleSupport::
    /// enable_saved_state_handles) (the host and navigation entries do it).
    ///
    /// # Panics
    /// When the extras haven't the saved state registry owner, the ViewModel store owner or the ViewModel's key
    /// (Kotlin's IllegalArgumentException), or the owner didn't enable handles.
    #[track_caller]
    pub fn create_saved_state_handle(&self) -> SavedStateHandle {
        let registry_owner = self.get(&SAVED_STATE_REGISTRY_OWNER_KEY).unwrap_or_else(|| {
            panic!("IllegalArgumentException: CreationExtras must have a value by `SAVED_STATE_REGISTRY_OWNER_KEY`")
        });
        let store_owner = self.get(&VIEW_MODEL_STORE_OWNER_KEY).unwrap_or_else(|| {
            panic!("IllegalArgumentException: CreationExtras must have a value by `VIEW_MODEL_STORE_OWNER_KEY`")
        });
        let default_args = self.get(&DEFAULT_ARGS_KEY);
        let key = self.get(&VIEW_MODEL_KEY).unwrap_or_else(|| {
            panic!("IllegalArgumentException: CreationExtras must have a value by `VIEW_MODEL_KEY`")
        });
        let provider = saved_state_handles_provider(&*registry_owner);
        let view_model = saved_state_handles_vm(&store_owner.view_model_store());
        if let Some(handle) = find(&view_model.handles.borrow(), &key) {
            return handle.clone();
        }
        let handle = SavedStateHandle::create_handle(
            provider.consume_restored_state_for_key(&key).as_ref(),
            default_args.as_ref(),
        );
        view_model.handles.borrow_mut().push((key, handle.clone()));
        handle
    }
}

// ---- SavedStateViewModelFactory ----

/// The factory of an owner with saved state (Compose's `SavedStateViewModelFactory`, the host's and a navigation
/// entry's default): a class made by its own construction ([`ViewModel::new_instance`]), which may take a
/// [`SavedStateHandle`] (`extras.create_saved_state_handle()`) — Android's actual, which finds a constructor taking
/// one; the desktop one can't make a class at all (UnsupportedOperationException: only factories and initializers).
#[derive(Clone, Copy, Default)]
pub struct SavedStateViewModelFactory;

impl SavedStateViewModelFactory {
    /// Kotlin: `SavedStateViewModelFactory()`
    pub fn new() -> SavedStateViewModelFactory {
        SavedStateViewModelFactory
    }
}

impl ViewModelProviderFactory for SavedStateViewModelFactory {
    fn create(&self, model_class: &KClass, extras: &CreationExtras) -> Rc<dyn ViewModel> {
        model_class.create_instance(extras)
    }
}

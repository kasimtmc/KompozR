//! Back stack entries (Compose's `NavBackStackEntry` and `NavBackStackEntryState`): a destination on the back
//! stack with its arguments — a `LifecycleOwner` whose lifecycle is the host's at most and its own maximum (resumed
//! while it is the destination shown, started while it is going or coming, created below), and what it saves.

use crate::destination::NavDestination;
use crate::saved_state::{SavedState, copy};
use crate::serialization::{Route, decode_arguments};
use kompoz_lifecycle::{
    AnyKey, CreationExtras, DEFAULT_ARGS_KEY, HasDefaultViewModelProviderFactory, Lifecycle, LifecycleEvent,
    LifecycleOwner, LifecycleRegistry, LifecycleState, MutableCreationExtras, SAVED_STATE_REGISTRY_OWNER_KEY,
    SavedStateHandle, SavedStateHandleSupport, SavedStateRegistry, SavedStateRegistryController,
    SavedStateRegistryOwner, SavedStateViewModelFactory, VIEW_MODEL_STORE_OWNER_KEY, ViewModel, ViewModelImpl,
    ViewModelProvider, ViewModelProviderFactory, ViewModelStore, ViewModelStoreOwner, ViewModelStoreProvider,
    view_model_factory,
};
use std::any::Any;
use std::cell::{Cell, OnceCell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

/// Where an entry's ViewModels are kept (Compose's `NavViewModelStoreProvider`, the controller's, by entry id): what
/// is cleared when an entry goes for good.
pub trait NavViewModelStoreProvider: Any {
    /// The store of the entry of id `key`, made the first time (Kotlin's `get(key)`).
    fn get(&self, key: &str) -> ViewModelStore;
    /// Clears and forgets the store of the entry of id `key` (Kotlin's `clear(key)`).
    fn clear(&self, key: &str);
}

/// The controller's stores of its entries' ViewModels, kept in `view_model_store` (Compose's
/// `NavViewModelStoreProvider(viewModelStore)`: a `ViewModelStoreProvider` under the key
/// "androidx.navigation.NavControllerViewModel").
#[allow(non_snake_case)]
pub fn NavViewModelStoreProvider(view_model_store: Option<ViewModelStore>) -> Rc<dyn NavViewModelStoreProvider> {
    struct Stores(ViewModelStoreProvider);
    impl NavViewModelStoreProvider for Stores {
        fn get(&self, key: &str) -> ViewModelStore {
            self.0.get_or_create(AnyKey::from(key.to_string()))
        }
        fn clear(&self, key: &str) {
            self.0.clear_key(AnyKey::from(key.to_string()));
        }
    }
    Rc::new(Stores(ViewModelStoreProvider::new(
        view_model_store,
        AnyKey::from("androidx.navigation.NavControllerViewModel".to_string()),
        SavedState::new(),
        None,
        None,
    )))
}

/// The ViewModel holding an entry's SavedStateHandle (Kotlin's private `SavedStateViewModel`).
struct SavedStateViewModel {
    base: ViewModelImpl,
    handle: SavedStateHandle,
}

impl ViewModel for SavedStateViewModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
}

pub(crate) struct EntryData {
    destination: RefCell<NavDestination>,
    immutable_args: Option<SavedState>,
    host_lifecycle_state: Cell<LifecycleState>,
    max_lifecycle: Cell<LifecycleState>,
    view_model_store_provider: Option<Rc<dyn NavViewModelStoreProvider>>,
    id: String,
    saved_state: Option<SavedState>,
    lifecycle_registry: LifecycleRegistry,
    saved_state_registry_controller: SavedStateRegistryController,
    saved_state_registry_attached: Cell<bool>,
    saved_state_handle: OnceCell<SavedStateHandle>,
    me: Weak<EntryData>,
}

impl LifecycleOwner for EntryData {
    fn lifecycle(&self) -> Lifecycle {
        self.lifecycle_registry.lifecycle()
    }
}

impl SavedStateRegistryOwner for EntryData {
    fn saved_state_registry(&self) -> SavedStateRegistry {
        self.saved_state_registry_controller.saved_state_registry()
    }
}

impl ViewModelStoreOwner for EntryData {
    /// # Panics
    /// Before it is on the back stack, once destroyed, or without the controller's ViewModel store.
    fn view_model_store(&self) -> ViewModelStore {
        assert!(
            self.saved_state_registry_attached.get(),
            "You cannot access the NavBackStackEntry's ViewModels until it is added to the NavController's back stack \
             (i.e., the Lifecycle of the NavBackStackEntry reaches the CREATED state)."
        );
        assert!(
            self.lifecycle_registry.lifecycle().current_state() != LifecycleState::Destroyed,
            "You cannot access the NavBackStackEntry's ViewModels after the NavBackStackEntry is destroyed."
        );
        let provider = self.view_model_store_provider.as_ref().expect(
            "You must call setViewModelStore() on your NavHostController before accessing the ViewModelStore of a \
             navigation graph.",
        );
        provider.get(&self.id)
    }

    fn as_has_default_view_model_provider_factory(&self) -> Option<&dyn HasDefaultViewModelProviderFactory> {
        Some(self)
    }
}

impl HasDefaultViewModelProviderFactory for EntryData {
    fn default_view_model_provider_factory(&self) -> Rc<dyn ViewModelProviderFactory> {
        Rc::new(SavedStateViewModelFactory)
    }

    /// Its saved state registry and itself as the owners, its arguments as the default arguments (a ViewModel's
    /// SavedStateHandle starts with the navigation arguments).
    fn default_view_model_creation_extras(&self) -> CreationExtras {
        let empty = CreationExtras::Empty;
        let extras = MutableCreationExtras::new(&empty);
        let me = self.me.upgrade().expect("the entry");
        let registry_owner: Rc<dyn SavedStateRegistryOwner> = me.clone();
        let store_owner: Rc<dyn ViewModelStoreOwner> = me;
        extras.set(&SAVED_STATE_REGISTRY_OWNER_KEY, registry_owner);
        extras.set(&VIEW_MODEL_STORE_OWNER_KEY, store_owner);
        if let Some(args) = &self.immutable_args {
            extras.set(&DEFAULT_ARGS_KEY, copy(args));
        }
        extras.into()
    }
}

/// An entry of the back stack (Compose's `NavBackStackEntry`): see the module. A cheap handle; two are equal when
/// they are the same entry.
#[derive(Clone)]
pub struct NavBackStackEntry(pub(crate) Rc<EntryData>);

impl PartialEq for NavBackStackEntry {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for NavBackStackEntry {}

impl std::hash::Hash for NavBackStackEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state);
    }
}

impl fmt::Debug for NavBackStackEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NavBackStackEntry({}) destination={:?}", self.0.id, self.destination())
    }
}

impl LifecycleOwner for NavBackStackEntry {
    fn lifecycle(&self) -> Lifecycle {
        self.0.lifecycle_registry.lifecycle()
    }
}

/// A random UUID's text (Kotlin's `UUID.randomUUID().toString()`): version 4.
fn random_uuid() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    thread_local! {
        static COUNTER: Cell<u64> = const { Cell::new(0) };
    }
    let mut bytes = [0u8; 16];
    for (i, chunk) in bytes.chunks_mut(8).enumerate() {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(COUNTER.with(|c| {
            c.set(c.get() + 1);
            c.get()
        }));
        h.write_usize(i);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        h.write_u128(nanos);
        chunk.copy_from_slice(&h.finish().to_le_bytes());
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

impl NavBackStackEntry {
    /// An entry of `destination` (Kotlin's `NavBackStackEntry.create(context, destination, arguments,
    /// hostLifecycleState, viewModelStoreProvider, id, savedState)`): `id` a new UUID when none.
    pub fn create(
        destination: NavDestination,
        arguments: Option<SavedState>,
        host_lifecycle_state: LifecycleState,
        view_model_store_provider: Option<Rc<dyn NavViewModelStoreProvider>>,
        id: Option<String>,
        saved_state: Option<SavedState>,
    ) -> NavBackStackEntry {
        let data = Rc::new_cyclic(|me: &Weak<EntryData>| {
            let owner: Weak<dyn LifecycleOwner> = me.clone();
            let registry_owner: Weak<dyn SavedStateRegistryOwner> = me.clone();
            EntryData {
                destination: RefCell::new(destination),
                immutable_args: arguments,
                host_lifecycle_state: Cell::new(host_lifecycle_state),
                max_lifecycle: Cell::new(LifecycleState::Initialized),
                view_model_store_provider,
                id: id.unwrap_or_else(random_uuid),
                saved_state,
                lifecycle_registry: LifecycleRegistry::new(owner),
                saved_state_registry_controller: SavedStateRegistryController::create(registry_owner),
                saved_state_registry_attached: Cell::new(false),
                saved_state_handle: OnceCell::new(),
                me: me.clone(),
            }
        });
        NavBackStackEntry(data)
    }

    /// A new entry for `entry`'s place with `arguments` (Kotlin's `NavBackStackEntry.create(entry, arguments)`: the
    /// same id, destination and saved state; its lifecycle's states).
    pub fn create_from(entry: &NavBackStackEntry, arguments: Option<SavedState>) -> NavBackStackEntry {
        let e = NavBackStackEntry::create(
            entry.destination(),
            arguments,
            entry.0.host_lifecycle_state.get(),
            entry.0.view_model_store_provider.clone(),
            Some(entry.0.id.clone()),
            entry.0.saved_state.clone(),
        );
        e.0.host_lifecycle_state.set(entry.0.host_lifecycle_state.get());
        e.set_max_lifecycle(entry.max_lifecycle());
        e
    }

    /// Its id (Kotlin's `id`): what its ViewModels and saved states are kept by.
    pub fn id(&self) -> String {
        self.0.id.clone()
    }

    /// Its destination (Kotlin's `destination`).
    pub fn destination(&self) -> NavDestination {
        self.0.destination.borrow().clone()
    }

    pub(crate) fn set_destination(&self, destination: NavDestination) {
        *self.0.destination.borrow_mut() = destination;
    }

    /// Its arguments (Kotlin's `arguments`): a copy.
    pub fn arguments(&self) -> Option<SavedState> {
        self.0.immutable_args.as_ref().map(copy)
    }

    pub(crate) fn immutable_args(&self) -> Option<&SavedState> {
        self.0.immutable_args.as_ref()
    }

    /// It as the `LifecycleOwner` its content is given (`LocalLifecycleOwner provides entry`).
    pub fn as_lifecycle_owner(&self) -> Rc<dyn LifecycleOwner> {
        self.0.clone()
    }

    /// It as the `ViewModelStoreOwner` its content is given (`LocalViewModelStoreOwner provides entry`).
    pub fn as_view_model_store_owner(&self) -> Rc<dyn ViewModelStoreOwner> {
        self.0.clone()
    }

    /// It as the `SavedStateRegistryOwner` its content is given (`LocalSavedStateRegistryOwner provides entry`).
    pub fn as_saved_state_registry_owner(&self) -> Rc<dyn SavedStateRegistryOwner> {
        self.0.clone()
    }

    /// Its ViewModels' store (Kotlin's `viewModelStore`).
    ///
    /// # Panics
    /// Before it is on the back stack, once destroyed, or without the controller's ViewModel store.
    pub fn view_model_store(&self) -> ViewModelStore {
        self.0.view_model_store()
    }

    /// Its saved state registry (Kotlin's `savedStateRegistry`).
    pub fn saved_state_registry(&self) -> SavedStateRegistry {
        self.0.saved_state_registry()
    }

    /// The factory its ViewModels are made by by default (Kotlin's `defaultViewModelProviderFactory`).
    pub fn default_view_model_provider_factory(&self) -> Rc<dyn ViewModelProviderFactory> {
        self.0.default_view_model_provider_factory()
    }

    /// The extras its ViewModels are made with by default (Kotlin's `defaultViewModelCreationExtras`).
    pub fn default_view_model_creation_extras(&self) -> CreationExtras {
        self.0.default_view_model_creation_extras()
    }

    /// Its own SavedStateHandle (Kotlin's `savedStateHandle`: results for the screen below; its arguments in it).
    ///
    /// # Panics
    /// Before it is on the back stack, or once destroyed.
    pub fn saved_state_handle(&self) -> SavedStateHandle {
        self.0
            .saved_state_handle
            .get_or_init(|| {
                assert!(
                    self.0.saved_state_registry_attached.get(),
                    "You cannot access the NavBackStackEntry's SavedStateHandle until it is added to the \
                     NavController's back stack (i.e., the Lifecycle of the NavBackStackEntry reaches the CREATED state)."
                );
                assert!(
                    self.lifecycle().current_state() != LifecycleState::Destroyed,
                    "You cannot access the NavBackStackEntry's SavedStateHandle after the NavBackStackEntry is destroyed."
                );
                let factory = view_model_factory(|b| {
                    b.initializer(|extras: &CreationExtras| SavedStateViewModel {
                        base: ViewModelImpl::new(),
                        handle: extras.create_saved_state_handle(),
                    })
                });
                let owner: &dyn ViewModelStoreOwner = &*self.0;
                ViewModelProvider::create(owner, Some(factory), None).get::<SavedStateViewModel>().handle.clone()
            })
            .clone()
    }

    /// Where its ViewModels are kept (Kotlin's `viewModelStoreProvider`).
    pub fn view_model_store_provider(&self) -> Option<Rc<dyn NavViewModelStoreProvider>> {
        self.0.view_model_store_provider.clone()
    }

    /// The saved state it was made with (restored: what its saved state registry restores).
    pub fn restored_state(&self) -> Option<SavedState> {
        self.0.saved_state.clone()
    }

    /// Whether its saved state registry is attached (it has been on a back stack: Kotlin checks it before
    /// giving its ViewModels and SavedStateHandle).
    pub fn is_saved_state_registry_attached(&self) -> bool {
        self.0.saved_state_registry_attached.get()
    }

    /// The highest state its lifecycle may be in (Kotlin's `maxLifecycle`).
    pub fn max_lifecycle(&self) -> LifecycleState {
        self.0.max_lifecycle.get()
    }

    /// Sets it, moving its lifecycle (Kotlin's `maxLifecycle = …`).
    pub fn set_max_lifecycle(&self, state: LifecycleState) {
        self.0.max_lifecycle.set(state);
        self.update_state();
    }

    /// The host's state (what its lifecycle follows up to its maximum).
    pub fn host_lifecycle_state(&self) -> LifecycleState {
        self.0.host_lifecycle_state.get()
    }

    /// The host's lifecycle moved by `event` (Kotlin's `handleLifecycleEvent`).
    pub fn handle_lifecycle_event(&self, event: LifecycleEvent) {
        self.0.host_lifecycle_state.set(event.target_state());
        self.update_state();
    }

    /// Moves its lifecycle to the host's state or its maximum, the lower (Kotlin's `updateState`); the first time,
    /// its saved state registry is attached (and what it saved restored) before.
    pub fn update_state(&self) {
        if !self.0.saved_state_registry_attached.get() {
            // (the first time, before its lifecycle moves up: its registry attached and what it saved restored)
            self.0.saved_state_registry_controller.perform_attach();
            self.0.saved_state_registry_attached.set(true);
            if self.0.view_model_store_provider.is_some() {
                self.0.enable_saved_state_handles();
            }
            self.0.saved_state_registry_controller.perform_restore(self.0.saved_state.as_ref());
        }
        let host = self.0.host_lifecycle_state.get();
        let max = self.0.max_lifecycle.get();
        let target = if host.ordinal() < max.ordinal() { host } else { max };
        self.0.lifecycle_registry.set_current_state(target);
    }

    /// Writes what its saved state registry saves into `out_bundle` (Kotlin's `saveState`).
    pub fn save_state(&self, out_bundle: &SavedState) {
        self.0.saved_state_registry_controller.perform_save(out_bundle);
    }

    /// Its arguments as a `T` (Compose's `NavBackStackEntry.toRoute<T>()`), read by its destination's arguments'
    /// types.
    ///
    /// # Panics
    /// If they aren't a `T`'s.
    pub fn to_route<T: Route>(&self) -> T {
        let args = self.arguments().unwrap_or_default();
        decode_arguments::<T>(&args, &self.destination().arguments().types())
    }
}

// ---- NavBackStackEntryState ----

const KEY_ID: &str = "nav-entry-state:id";
const KEY_DESTINATION_ID: &str = "nav-entry-state:destination-id";
const KEY_ARGS: &str = "nav-entry-state:args";
const KEY_SAVED_STATE: &str = "nav-entry-state:saved-state";

/// What is kept of an entry when its stack is saved (Compose's internal `NavBackStackEntryState`): its id, its
/// destination's id, its arguments and saved state.
#[derive(Clone, Debug, PartialEq)]
pub struct NavBackStackEntryState {
    id: String,
    destination_id: i32,
    args: Option<SavedState>,
    saved_state: SavedState,
}

impl NavBackStackEntryState {
    /// `entry`'s (its saved state written now).
    pub fn new(entry: &NavBackStackEntry) -> NavBackStackEntryState {
        let saved_state = SavedState::new();
        entry.save_state(&saved_state);
        NavBackStackEntryState {
            id: entry.id(),
            destination_id: entry.destination().id(),
            args: entry.arguments(),
            saved_state,
        }
    }

    /// One read back from what [`write_to_state`](Self::write_to_state) wrote.
    ///
    /// # Panics
    /// A state without its keys.
    pub fn from_state(state: &SavedState) -> NavBackStackEntryState {
        state.read(|r| NavBackStackEntryState {
            id: r.get_string(KEY_ID),
            destination_id: r.get_int(KEY_DESTINATION_ID),
            args: Some(r.get_saved_state(KEY_ARGS)),
            saved_state: r.get_saved_state(KEY_SAVED_STATE),
        })
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn destination_id(&self) -> i32 {
        self.destination_id
    }

    pub fn args(&self) -> Option<SavedState> {
        self.args.clone()
    }

    pub fn saved_state(&self) -> SavedState {
        self.saved_state.clone()
    }

    /// It written into a saved state.
    pub fn write_to_state(&self) -> SavedState {
        crate::saved_state::saved_state(|w| {
            w.put_string(KEY_ID, self.id.clone());
            w.put_int(KEY_DESTINATION_ID, self.destination_id);
            w.put_saved_state(KEY_ARGS, &self.args.clone().unwrap_or_default());
            w.put_saved_state(KEY_SAVED_STATE, &self.saved_state);
        })
    }

    /// The entry it was, of `destination` again (its id, arguments, saved state).
    pub fn instantiate(
        &self,
        destination: NavDestination,
        host_lifecycle_state: LifecycleState,
        view_model_store_provider: Option<Rc<dyn NavViewModelStoreProvider>>,
    ) -> NavBackStackEntry {
        NavBackStackEntry::create(
            destination,
            self.args.as_ref().map(copy),
            host_lifecycle_state,
            view_model_store_provider,
            Some(self.id.clone()),
            Some(self.saved_state.clone()),
        )
    }
}

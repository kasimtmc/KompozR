//! lifecycle-viewmodel-savedstate's ViewModelStoreProvider.kt and ViewModelStoreOwnerFactory.kt (the
//! `ViewModelStoreOwner(…)` factories).

use super::handle::{
    DEFAULT_ARGS_KEY, SAVED_STATE_KEY, SAVED_STATE_REGISTRY_OWNER_KEY, SavedStateHandleSupport,
    SavedStateViewModelFactory, VIEW_MODEL_STORE_OWNER_KEY,
};
use super::provider::{CreationExtras, MutableCreationExtras, ViewModelProviderFactory};
use super::store::{AnyKey, HasDefaultViewModelProviderFactory, ViewModelStore, ViewModelStoreOwner};
use super::{ViewModel, ViewModelImpl};
use crate::lifecycle::{Lifecycle, LifecycleOwner};
use crate::saved_state::{SavedState, SavedStateRegistry, SavedStateRegistryOwner, saved_state};
use kompoz_runtime::kt::AutoCloseable;
use kompoz_runtime::named;
use std::cell::{Cell, OnceCell, RefCell};
use std::rc::{Rc, Weak};

// ---- ViewModelStoreOwner(…) ----

/// The extras an owner made by a factory gives (Compose's `defaultViewModelCreationExtras` getters of
/// ViewModelStoreOwnerFactory.kt): `default_creation_extras`'s, the default arguments added, itself as the owner.
fn owner_extras(
    default_creation_extras: &CreationExtras,
    default_args: &SavedState,
    store_owner: Rc<dyn ViewModelStoreOwner>,
    registry_owner: Option<Rc<dyn SavedStateRegistryOwner>>,
) -> CreationExtras {
    let extras = MutableCreationExtras::new(default_creation_extras);
    let existing = extras.get(&DEFAULT_ARGS_KEY);
    extras.set(
        &DEFAULT_ARGS_KEY,
        saved_state(|w| {
            if let Some(existing) = &existing {
                w.put_all(existing);
            }
            w.put_all(default_args);
        }),
    );
    if let Some(registry_owner) = registry_owner {
        extras.set(&SAVED_STATE_REGISTRY_OWNER_KEY, registry_owner);
    }
    extras.set(&VIEW_MODEL_STORE_OWNER_KEY, store_owner);
    extras.into()
}

/// The owner [`ViewModelStoreOwner()`](ViewModelStoreOwner()) makes.
struct StoreOwner {
    me: Weak<StoreOwner>,
    view_model_store: ViewModelStore,
    default_args: SavedState,
    default_creation_extras: CreationExtras,
    default_factory: Rc<dyn ViewModelProviderFactory>,
}

impl ViewModelStoreOwner for StoreOwner {
    fn view_model_store(&self) -> ViewModelStore {
        self.view_model_store.clone()
    }
    fn as_has_default_view_model_provider_factory(&self) -> Option<&dyn HasDefaultViewModelProviderFactory> {
        Some(self)
    }
}

impl HasDefaultViewModelProviderFactory for StoreOwner {
    fn default_view_model_provider_factory(&self) -> Rc<dyn ViewModelProviderFactory> {
        self.default_factory.clone()
    }
    fn default_view_model_creation_extras(&self) -> CreationExtras {
        let me: Rc<dyn ViewModelStoreOwner> = self.me.upgrade().expect("kompoz: the owner is there");
        owner_extras(&self.default_creation_extras, &self.default_args, me, None)
    }
}

/// An owner of `view_model_store` (Compose's `ViewModelStoreOwner(viewModelStore, defaultArgs, defaultCreationExtras,
/// defaultFactory)`): its ViewModels made by `default_factory`, given `default_creation_extras` with
/// `default_args` and the owner.
///
/// Kotlin: `ViewModelStoreOwner(viewModelStore: ViewModelStore, defaultArgs: SavedState = savedState(), defaultCreationExtras: CreationExtras? = null, defaultFactory: ViewModelProvider.Factory? = null)`
#[allow(non_snake_case)]
pub fn ViewModelStoreOwner(
    view_model_store: ViewModelStore,
    default_args: SavedState,
    default_creation_extras: Option<CreationExtras>,
    default_factory: Option<Rc<dyn ViewModelProviderFactory>>,
) -> Rc<dyn ViewModelStoreOwner> {
    let default_creation_extras = default_creation_extras.unwrap_or(CreationExtras::Empty);
    let default_factory: Rc<dyn ViewModelProviderFactory> =
        default_factory.unwrap_or_else(|| Rc::new(SavedStateViewModelFactory));
    Rc::new_cyclic(|me: &Weak<StoreOwner>| StoreOwner {
        me: me.clone(),
        view_model_store,
        default_args,
        default_creation_extras,
        default_factory,
    })
}

/// The owner the two other factories make: a saved state registry owner too.
struct SavedStateStoreOwner {
    me: Weak<SavedStateStoreOwner>,
    view_model_store: ViewModelStore,
    saved_state_registry: SavedStateRegistry,
    lifecycle: Lifecycle,
    default_args: SavedState,
    default_creation_extras: CreationExtras,
    default_factory: Rc<dyn ViewModelProviderFactory>,
}

impl ViewModelStoreOwner for SavedStateStoreOwner {
    fn view_model_store(&self) -> ViewModelStore {
        self.view_model_store.clone()
    }
    fn as_has_default_view_model_provider_factory(&self) -> Option<&dyn HasDefaultViewModelProviderFactory> {
        Some(self)
    }
}

impl LifecycleOwner for SavedStateStoreOwner {
    fn lifecycle(&self) -> Lifecycle {
        self.lifecycle.clone()
    }
}

impl SavedStateRegistryOwner for SavedStateStoreOwner {
    fn saved_state_registry(&self) -> SavedStateRegistry {
        self.saved_state_registry.clone()
    }
}

impl HasDefaultViewModelProviderFactory for SavedStateStoreOwner {
    fn default_view_model_provider_factory(&self) -> Rc<dyn ViewModelProviderFactory> {
        self.default_factory.clone()
    }
    fn default_view_model_creation_extras(&self) -> CreationExtras {
        let me = self.me.upgrade().expect("kompoz: the owner is there");
        owner_extras(&self.default_creation_extras, &self.default_args, me.clone(), Some(me))
    }
}

/// An owner of `view_model_store` saving its ViewModels' handles with `saved_state_registry_owner`'s state (Compose's
/// `ViewModelStoreOwner(viewModelStore, savedStateRegistryOwner, …)`).
///
/// Kotlin: `ViewModelStoreOwner(viewModelStore: ViewModelStore, savedStateRegistryOwner: SavedStateRegistryOwner, defaultArgs: SavedState = savedState(), defaultCreationExtras: CreationExtras? = null, defaultFactory: ViewModelProvider.Factory? = null)`
#[allow(non_snake_case)]
pub fn ViewModelStoreOwner_of(
    view_model_store: ViewModelStore,
    saved_state_registry_owner: &dyn SavedStateRegistryOwner,
    default_args: SavedState,
    default_creation_extras: Option<CreationExtras>,
    default_factory: Option<Rc<dyn ViewModelProviderFactory>>,
) -> Rc<dyn ViewModelStoreOwner> {
    ViewModelStoreOwner_with(
        view_model_store,
        saved_state_registry_owner.saved_state_registry(),
        saved_state_registry_owner.lifecycle(),
        default_args,
        default_creation_extras,
        default_factory,
    )
}

/// An owner of `view_model_store` saving its ViewModels' handles in `saved_state_registry`, `lifecycle` its owner's
/// (Compose's `ViewModelStoreOwner(viewModelStore, savedStateRegistry, lifecycle, …)`): handles enabled in the
/// registry if they aren't yet.
///
/// Kotlin: `ViewModelStoreOwner(viewModelStore: ViewModelStore, savedStateRegistry: SavedStateRegistry, lifecycle: Lifecycle, defaultArgs: SavedState = savedState(), defaultCreationExtras: CreationExtras? = null, defaultFactory: ViewModelProvider.Factory? = null)`
#[allow(non_snake_case)]
pub fn ViewModelStoreOwner_with(
    view_model_store: ViewModelStore,
    saved_state_registry: SavedStateRegistry,
    lifecycle: Lifecycle,
    default_args: SavedState,
    default_creation_extras: Option<CreationExtras>,
    default_factory: Option<Rc<dyn ViewModelProviderFactory>>,
) -> Rc<dyn ViewModelStoreOwner> {
    let default_creation_extras = default_creation_extras.unwrap_or(CreationExtras::Empty);
    let default_factory: Rc<dyn ViewModelProviderFactory> =
        default_factory.unwrap_or_else(|| Rc::new(SavedStateViewModelFactory));
    let owner = Rc::new_cyclic(|me: &Weak<SavedStateStoreOwner>| SavedStateStoreOwner {
        me: me.clone(),
        view_model_store,
        saved_state_registry,
        lifecycle,
        default_args,
        default_creation_extras,
        default_factory,
    });
    if owner.saved_state_registry.get_saved_state_provider(SAVED_STATE_KEY).is_none() {
        owner.enable_saved_state_handles();
    }
    owner
}

// ---- ViewModelStoreProvider ----

/// What [`ViewModelStoreProvider::acquire_token`] gives (Compose's fun interface `ReferenceToken`, an
/// AutoCloseable): close it as what uses the store goes.
#[derive(Clone)]
pub struct ReferenceToken(Rc<dyn Fn()>);

impl ReferenceToken {
    /// Gives the reference back (Compose's `close()`).
    pub fn close(&self) {
        (self.0)()
    }
}

impl AutoCloseable for ReferenceToken {
    fn close(&self) {
        (self.0)()
    }
}

/// A store of a provider, with who uses it.
struct Entry {
    key: AnyKey,
    store: ViewModelStore,
    ref_count: Cell<i32>,
    is_disposable: Cell<bool>,
}

/// A provider's stores, kept in its parent's store as a ViewModel (Compose's private `StateHolder`): cleared with
/// it.
#[derive(Default)]
struct StateHolder {
    base: ViewModelImpl,
    entries: RefCell<Vec<Rc<Entry>>>,
}

impl ViewModel for StateHolder {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }

    fn on_cleared(&self) {
        let entries = self.entries.borrow().clone();
        for entry in entries {
            entry.is_disposable.set(true);
            if entry.ref_count.get() <= 0 {
                self.remove(&entry.key);
            }
        }
    }
}

impl StateHolder {
    fn get_or_create(&self, key: &AnyKey) -> Rc<Entry> {
        if let Some(e) = self.entries.borrow().iter().find(|e| e.key == *key) {
            return e.clone();
        }
        let e = Rc::new(Entry {
            key: key.clone(),
            store: ViewModelStore::new(),
            ref_count: Cell::new(0),
            is_disposable: Cell::new(false),
        });
        self.entries.borrow_mut().push(e.clone());
        e
    }

    fn remove(&self, key: &AnyKey) {
        let removed = {
            let mut entries = self.entries.borrow_mut();
            entries.iter().position(|e| e.key == *key).map(|i| entries.remove(i))
        };
        if let Some(e) = removed {
            e.store.clear();
        }
    }

    fn clear_key(&self, key: &AnyKey) {
        let Some(entry) = self.entries.borrow().iter().find(|e| e.key == *key).cloned() else { return };
        entry.is_disposable.set(true);
        if entry.ref_count.get() <= 0 {
            self.remove(key);
        }
    }
}

struct ProviderInner {
    parent_store: Option<ViewModelStore>,
    parent_key: AnyKey,
    default_args: SavedState,
    default_creation_extras: CreationExtras,
    default_factory: Rc<dyn ViewModelProviderFactory>,
    state_holder: OnceCell<Rc<StateHolder>>,
}

/// ViewModel stores by key, kept in a parent store (Compose's `ViewModelStoreProvider`): each of its keys' stores
/// lives while something holds a reference token to it, and goes once its key is cleared and nothing does; all go
/// when the parent's store is cleared. What `remember_view_model_store_owner` scopes ViewModels to a part of a
/// screen with (a pager's page, a feature). A clone is the same provider.
#[derive(Clone)]
pub struct ViewModelStoreProvider(Rc<ProviderInner>);

impl PartialEq for ViewModelStoreProvider {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The key of the token a provider's composable holds while it is there (Compose's `ProviderMarkerKey` object).
const PROVIDER_MARKER_KEY: AnyKey = AnyKey::Object("ViewModelStoreProvider.ProviderMarkerKey");

impl ViewModelStoreProvider {
    /// A provider keeping its stores in `parent_store` under `parent_key` (none: its own) (Compose's
    /// `ViewModelStoreProvider(parentStore, parentKey, defaultArgs, defaultCreationExtras, defaultFactory)`).
    ///
    /// Kotlin: `ViewModelStoreProvider(parentStore: ViewModelStore?, parentKey: Any? = AnyKey.Null, defaultArgs: SavedState = savedState(), defaultCreationExtras: CreationExtras? = null, defaultFactory: ViewModelProvider.Factory? = null)`
    pub fn new(
        parent_store: Option<ViewModelStore>,
        parent_key: impl Into<AnyKey>,
        default_args: SavedState,
        default_creation_extras: Option<CreationExtras>,
        default_factory: Option<Rc<dyn ViewModelProviderFactory>>,
    ) -> ViewModelStoreProvider {
        ViewModelStoreProvider(Rc::new(ProviderInner {
            parent_store,
            parent_key: parent_key.into(),
            default_args,
            default_creation_extras: default_creation_extras.unwrap_or(CreationExtras::Empty),
            default_factory: default_factory.unwrap_or_else(|| Rc::new(SavedStateViewModelFactory)),
            state_holder: OnceCell::new(),
        }))
    }

    /// A provider keeping its stores in `parent_owner`'s store, its defaults the owner's (Compose's
    /// `ViewModelStoreProvider(parentOwner, …)`).
    ///
    /// Kotlin: `ViewModelStoreProvider(parentOwner: ViewModelStoreOwner?, parentKey: Any? = AnyKey.Null, defaultArgs: SavedState = savedState(), defaultCreationExtras: CreationExtras? = null, defaultFactory: ViewModelProvider.Factory? = null)`
    pub fn of_owner(
        parent_owner: Option<Rc<dyn ViewModelStoreOwner>>,
        parent_key: impl Into<AnyKey>,
        default_args: SavedState,
        default_creation_extras: Option<CreationExtras>,
        default_factory: Option<Rc<dyn ViewModelProviderFactory>>,
    ) -> ViewModelStoreProvider {
        let extras = default_creation_extras.unwrap_or_else(|| match &parent_owner {
            Some(o) => o.default_view_model_creation_extras(),
            None => CreationExtras::Empty,
        });
        let factory = default_factory.unwrap_or_else(|| match &parent_owner {
            Some(o) => o.default_view_model_provider_factory(),
            None => Rc::new(super::store::DefaultViewModelProviderFactory),
        });
        ViewModelStoreProvider(Rc::new(ProviderInner {
            parent_store: parent_owner.map(|o| o.view_model_store()),
            parent_key: parent_key.into(),
            default_args,
            default_creation_extras: extras,
            default_factory: factory,
            state_holder: OnceCell::new(),
        }))
    }
}

#[named]
impl ViewModelStoreProvider {
    /// An owner of `key`'s store (Compose's `getOrCreateOwner`), its handles saved with `saved_state_registry_owner`'s
    /// state if one is given.
    ///
    /// Kotlin: `getOrCreateOwner(key: Any?, savedStateRegistryOwner: SavedStateRegistryOwner? = null, defaultArgs: SavedState = this.defaultArgs, defaultCreationExtras: CreationExtras = this.defaultCreationExtras, defaultFactory: ViewModelProvider.Factory = this.defaultFactory)`
    pub fn get_or_create_owner(
        &self,
        key: impl Into<AnyKey>,
        #[default(None)] saved_state_registry_owner: Option<Rc<dyn SavedStateRegistryOwner>>,
        #[default(None)] default_args: Option<SavedState>,
        #[default(None)] default_creation_extras: Option<CreationExtras>,
        #[default(None)] default_factory: Option<Rc<dyn ViewModelProviderFactory>>,
    ) -> Rc<dyn ViewModelStoreOwner> {
        let view_model_store = self.get_or_create(key);
        let default_args = default_args.unwrap_or_else(|| self.0.default_args.clone());
        let default_creation_extras = default_creation_extras.unwrap_or_else(|| self.0.default_creation_extras.clone());
        let default_factory = default_factory.unwrap_or_else(|| self.0.default_factory.clone());
        match saved_state_registry_owner {
            None => ViewModelStoreOwner(
                view_model_store,
                default_args,
                Some(default_creation_extras),
                Some(default_factory),
            ),
            Some(owner) => ViewModelStoreOwner_with(
                view_model_store,
                owner.saved_state_registry(),
                owner.lifecycle(),
                default_args,
                Some(default_creation_extras),
                Some(default_factory),
            ),
        }
    }
}

impl ViewModelStoreProvider {
    fn state_holder(&self) -> Rc<StateHolder> {
        self.0
            .state_holder
            .get_or_init(|| match &self.0.parent_store {
                Some(parent) => parent.get_or_put(self.0.parent_key.clone(), StateHolder::default),
                None => Rc::new(StateHolder::default()),
            })
            .clone()
    }

    /// A reference to `key`'s store (made if there is none): it isn't cleared while the token is open (Compose's
    /// `acquireToken`).
    pub fn acquire_token(&self, key: impl Into<AnyKey>) -> ReferenceToken {
        let key = key.into();
        let holder = self.state_holder();
        let entry = holder.get_or_create(&key);
        entry.ref_count.set(entry.ref_count.get() + 1);
        let weak = Rc::downgrade(&holder);
        ReferenceToken(Rc::new(move || {
            entry.ref_count.set(entry.ref_count.get() - 1);
            if entry.is_disposable.get()
                && entry.ref_count.get() <= 0
                && let Some(holder) = weak.upgrade()
            {
                holder.remove(&key);
            }
        }))
    }

    /// `key`'s store, made if there is none (Compose's `getOrCreate`).
    pub fn get_or_create(&self, key: impl Into<AnyKey>) -> ViewModelStore {
        self.state_holder().get_or_create(&key.into()).store.clone()
    }

    /// `key`'s store goes as soon as no token references it (at once if none does) (Compose's `clearKey`).
    pub fn clear_key(&self, key: impl Into<AnyKey>) {
        self.state_holder().clear_key(&key.into());
    }

    /// Every store goes as its tokens close, unless the provider's own composable still holds its token (Compose's
    /// `clearAllKeys`).
    pub fn clear_all_keys(&self) {
        let holder = self.state_holder();
        let marked = holder.entries.borrow().iter().find(|e| e.key == PROVIDER_MARKER_KEY).cloned();
        if marked.is_some_and(|e| e.ref_count.get() > 0) {
            return;
        }
        holder.on_cleared();
    }

    /// The key of the token its composable holds (Compose's `ViewModelStoreProvider.ProviderMarkerKey`).
    pub fn provider_marker_key() -> AnyKey {
        PROVIDER_MARKER_KEY
    }
}

//! lifecycle-viewmodel-compose: LocalViewModelStoreOwner.kt, ViewModel.kt (`viewModel`),
//! RememberViewModelStoreOwner.kt and RememberViewModelStoreProvider.kt.

use super::provider::{
    CreationExtras, ViewModelProvider, ViewModelProviderFactory, default_key, downcast, view_model_factory,
};
use super::store::{AnyKey, ViewModelStoreOwner};
use super::store_provider::ViewModelStoreProvider;
use super::{KClass, ViewModel};
use crate::compose::LocalLifecycleOwner;
use crate::lifecycle::LifecycleState;
use crate::saved_state::{LocalSavedStateRegistryOwner, SavedState, SavedStateRegistryOwner};
use kompoz_runtime::{
    DisposableEffect, KompozitionLocal, ProvidedValue, current_composite_key_hash, kompozable, named_args, on_dispose,
    remember_keyed,
};
use std::rc::Rc;

static LOCAL_VIEW_MODEL_STORE_OWNER: KompozitionLocal<Option<Rc<dyn ViewModelStoreOwner>>> =
    KompozitionLocal::new(|| None);

/// The type of [`LocalViewModelStoreOwner`] (Compose's `object LocalViewModelStoreOwner`).
pub struct LocalViewModelStoreOwnerLocal(());

/// The nearest ViewModel store owner (Compose's `LocalViewModelStoreOwner`): the host's (the app's, its host
/// default) unless a navigation entry or [`remember_view_model_store_owner`] gives another below.
pub static LocalViewModelStoreOwner: LocalViewModelStoreOwnerLocal = LocalViewModelStoreOwnerLocal(());

impl LocalViewModelStoreOwnerLocal {
    /// The nearest owner, none if nothing gives one (Compose's `current`).
    pub fn current(&self) -> Option<Rc<dyn ViewModelStoreOwner>> {
        LOCAL_VIEW_MODEL_STORE_OWNER.current()
    }

    /// `view_model_store_owner` as the owner of the content below (Compose's `provides`).
    pub fn provides(&self, view_model_store_owner: Rc<dyn ViewModelStoreOwner>) -> ProvidedValue {
        LOCAL_VIEW_MODEL_STORE_OWNER.provides(Some(view_model_store_owner))
    }
}

/// The nearest owner, which must be there (Compose's `checkNotNull(LocalViewModelStoreOwner.current) { … }`).
#[track_caller]
fn required_owner(message: &str) -> Rc<dyn ViewModelStoreOwner> {
    LocalViewModelStoreOwner.current().unwrap_or_else(|| panic!("IllegalStateException: {message}"))
}

const NO_OWNER: &str = "No ViewModelStoreOwner was provided via LocalViewModelStoreOwner";
const NOT_PRESENT: &str = "CompositionLocal LocalViewModelStoreOwner not present";

/// The `VM` of `view_model_store_owner` (the nearest by default) for `key` (none: the class's), made by `factory`
/// (none: the owner's default) the first time, with `extras` (none: the owner's defaults) (Compose's
/// `viewModel<VM>()`): the same one while the owner lives. The type is the one the result goes to (Kotlin's type
/// argument), a reference to it: `let vm: Rc<TickViewModel> = view_model();` (or a type naming one, a Kotlin class's
/// Rust twin `type TickViewModel = Rc<TickViewModelClass>`: [`ViewModelRef`]).
///
/// ```no_run
/// # use kompoz_lifecycle::*;
/// # use kompoz_runtime::kompozable;
/// # use std::rc::Rc;
/// # struct Counter { base: ViewModelImpl }
/// # impl ViewModel for Counter {
/// #     fn view_model_impl(&self) -> &ViewModelImpl { &self.base }
/// #     fn new_instance(_: &CreationExtras) -> Option<Self> { Some(Counter { base: ViewModelImpl::new() }) }
/// # }
/// #[kompozable]
/// fn Screen() {
///     let counter: Rc<Counter> = view_model();
///     # let _ = counter;
/// }
/// ```
///
/// Kotlin: `viewModel(viewModelStoreOwner: ViewModelStoreOwner = …, key: String? = null, factory: ViewModelProvider.Factory? = null, extras: CreationExtras = …)`
#[kompozable]
pub fn view_model<T: ViewModelRef>(
    #[default(required_owner(NO_OWNER))] view_model_store_owner: Rc<dyn ViewModelStoreOwner>,
    #[default(None)] key: Option<String>,
    #[default(None)] factory: Option<Rc<dyn ViewModelProviderFactory>>,
    #[default(None)] extras: Option<CreationExtras>,
) -> T {
    T::from_rc(downcast(get(&KClass::of::<T::Model>(), &*view_model_store_owner, key, factory, extras)))
}

/// What [`view_model`] gives: a shared reference to a ViewModel (Kotlin's object reference) — an `Rc<V>`, or a
/// type naming one (a Kotlin class's Rust twin, `type QueryViewModel = Rc<QueryViewModelClass>`, which a Kotlin
/// parameter `vm: QueryViewModel` takes).
pub trait ViewModelRef: Clone + 'static {
    /// The ViewModel it refers to.
    type Model: ViewModel;
    /// It, of the ViewModel the store gave.
    fn from_rc(model: Rc<Self::Model>) -> Self;
}

impl<V: ViewModel> ViewModelRef for Rc<V> {
    type Model = V;
    fn from_rc(model: Rc<V>) -> Rc<V> {
        model
    }
}

/// The ViewModel of `model_class` (Compose's `viewModel(modelClass, …)`).
///
/// Kotlin: `viewModel(modelClass: KClass<VM>, viewModelStoreOwner: ViewModelStoreOwner = …, key: String? = null, factory: ViewModelProvider.Factory? = null, extras: CreationExtras = …)`
#[kompozable]
pub fn view_model_of(
    model_class: KClass,
    #[default(required_owner(NO_OWNER))] view_model_store_owner: Rc<dyn ViewModelStoreOwner>,
    #[default(None)] key: Option<String>,
    #[default(None)] factory: Option<Rc<dyn ViewModelProviderFactory>>,
    #[default(None)] extras: Option<CreationExtras>,
) -> Rc<dyn ViewModel> {
    get(&model_class, &*view_model_store_owner, key, factory, extras)
}

/// The `VM` made by `initializer` the first time (its receiver the extras: `extras.create_saved_state_handle()`)
/// (Compose's `viewModel { … }`).
///
/// ```no_run
/// # use kompoz_lifecycle::*;
/// # use kompoz_runtime::kompozable;
/// # use std::rc::Rc;
/// # struct Search { base: ViewModelImpl, handle: SavedStateHandle }
/// # impl ViewModel for Search { fn view_model_impl(&self) -> &ViewModelImpl { &self.base } }
/// #[kompozable]
/// fn Screen() {
///     let search: Rc<Search> =
///         view_model_with(|extras| Search { base: ViewModelImpl::new(), handle: extras.create_saved_state_handle() });
///     # let _ = search;
/// }
/// ```
///
/// Kotlin: `viewModel(viewModelStoreOwner: ViewModelStoreOwner = …, key: String? = null, initializer: CreationExtras.() -> VM)`
#[kompozable]
pub fn view_model_with<VM: ViewModel>(
    #[default(required_owner(NO_OWNER))] view_model_store_owner: Rc<dyn ViewModelStoreOwner>,
    #[default(None)] key: Option<String>,
    initializer: impl Fn(&CreationExtras) -> VM + 'static,
) -> Rc<VM> {
    let factory = view_model_factory(move |b| b.initializer(initializer));
    let extras = view_model_store_owner.default_view_model_creation_extras();
    downcast(get(&KClass::of::<VM>(), &*view_model_store_owner, key, Some(factory), Some(extras)))
}

/// Compose's `viewModel(modelClass, …)` body.
fn get(
    model_class: &KClass,
    owner: &dyn ViewModelStoreOwner,
    key: Option<String>,
    factory: Option<Rc<dyn ViewModelProviderFactory>>,
    extras: Option<CreationExtras>,
) -> Rc<dyn ViewModel> {
    let factory = factory.unwrap_or_else(|| owner.default_view_model_provider_factory());
    let extras = extras.unwrap_or_else(|| owner.default_view_model_creation_extras());
    let provider = ViewModelProvider::create(owner, Some(factory), Some(extras));
    match key {
        Some(key) => provider.get_class_keyed(&key, model_class),
        None => provider.get_class_keyed(&default_key(model_class), model_class),
    }
}

/// A ViewModel store owner of the caller's own (Compose's `rememberViewModelStoreOwner(parent, …)`): its store kept
/// in `parent`'s (the nearest owner's) under the caller's position, cleared as the caller leaves the composition for
/// good; its ViewModels' handles saved with `saved_state_registry_owner`'s state. Provide it as
/// [`LocalViewModelStoreOwner`] to the content its ViewModels are scoped to.
///
/// Kotlin: `rememberViewModelStoreOwner(parent: ViewModelStoreOwner? = …, savedStateRegistryOwner: SavedStateRegistryOwner? = LocalSavedStateRegistryOwner.current, defaultArgs: SavedState = savedState(), defaultCreationExtras: CreationExtras = …, defaultFactory: ViewModelProvider.Factory = …)`
#[kompozable]
pub fn remember_view_model_store_owner(
    #[default(Some(required_owner(NOT_PRESENT)))] parent: Option<Rc<dyn ViewModelStoreOwner>>,
    #[default(Some(LocalSavedStateRegistryOwner.current()))] saved_state_registry_owner: Option<
        Rc<dyn SavedStateRegistryOwner>,
    >,
    #[default(SavedState::new())] default_args: SavedState,
    #[default(None)] default_creation_extras: Option<CreationExtras>,
    #[default(None)] default_factory: Option<Rc<dyn ViewModelProviderFactory>>,
) -> Rc<dyn ViewModelStoreOwner> {
    let key = AnyKey::Long(current_composite_key_hash() as i64);
    let provider = provider_at(key.clone(), parent, default_args, default_creation_extras, default_factory);
    owner_at(key, provider, saved_state_registry_owner)
}

/// An owner of the caller's own of `provider`'s stores, under the caller's position (Compose's
/// `rememberViewModelStoreOwner(provider, savedStateRegistryOwner)`).
///
/// Kotlin: `rememberViewModelStoreOwner(provider: ViewModelStoreProvider, savedStateRegistryOwner: SavedStateRegistryOwner? = LocalSavedStateRegistryOwner.current)`
#[kompozable]
pub fn remember_view_model_store_owner_of(
    provider: ViewModelStoreProvider,
    #[default(Some(LocalSavedStateRegistryOwner.current()))] saved_state_registry_owner: Option<
        Rc<dyn SavedStateRegistryOwner>,
    >,
) -> Rc<dyn ViewModelStoreOwner> {
    let key = AnyKey::Long(current_composite_key_hash() as i64);
    owner_at(key, provider, saved_state_registry_owner)
}

/// An owner of `provider`'s store of `key` (a pager's page) (Compose's `rememberViewModelStoreOwner(key, provider,
/// savedStateRegistryOwner)`): the store referenced while the caller is in the composition.
///
/// Kotlin: `rememberViewModelStoreOwner(key: Any?, provider: ViewModelStoreProvider, savedStateRegistryOwner: SavedStateRegistryOwner? = LocalSavedStateRegistryOwner.current)`
#[kompozable]
pub fn remember_view_model_store_owner_keyed(
    key: impl Into<AnyKey>,
    provider: ViewModelStoreProvider,
    #[default(Some(LocalSavedStateRegistryOwner.current()))] saved_state_registry_owner: Option<
        Rc<dyn SavedStateRegistryOwner>,
    >,
) -> Rc<dyn ViewModelStoreOwner> {
    owner_at(key.into(), provider, saved_state_registry_owner)
}

/// Compose's `rememberViewModelStoreOwner(key, provider, savedStateRegistryOwner)` body.
#[kompozable]
fn owner_at(
    key: AnyKey,
    provider: ViewModelStoreProvider,
    saved_state_registry_owner: Option<Rc<dyn SavedStateRegistryOwner>>,
) -> Rc<dyn ViewModelStoreOwner> {
    let (p, k, s) = (provider.clone(), key.clone(), saved_state_registry_owner.clone());
    let owner = remember_keyed((provider.clone(), key.clone(), saved_state_registry_owner), move || {
        named_args!(p.get_or_create_owner(k, saved_state_registry_owner = s))
    });
    DisposableEffect(owner.clone(), move || {
        let token = provider.acquire_token(key.clone());
        on_dispose(move || token.close())
    });
    owner
}

/// A provider of ViewModel stores of the caller's own (Compose's `rememberViewModelStoreProvider(parent, …)`), kept
/// in `parent`'s store under the caller's position: its stores cleared as the caller leaves the composition for
/// good (not as the app is destroyed: they go with the parent's store then).
///
/// Kotlin: `rememberViewModelStoreProvider(parent: ViewModelStoreOwner? = …, defaultArgs: SavedState = savedState(), defaultCreationExtras: CreationExtras = …, defaultFactory: ViewModelProvider.Factory = …)`
#[kompozable]
pub fn remember_view_model_store_provider(
    #[default(Some(required_owner(NOT_PRESENT)))] parent: Option<Rc<dyn ViewModelStoreOwner>>,
    #[default(SavedState::new())] default_args: SavedState,
    #[default(None)] default_creation_extras: Option<CreationExtras>,
    #[default(None)] default_factory: Option<Rc<dyn ViewModelProviderFactory>>,
) -> ViewModelStoreProvider {
    let key = AnyKey::Long(current_composite_key_hash() as i64);
    provider_at(key, parent, default_args, default_creation_extras, default_factory)
}

/// The same, kept under `key` in `parent`'s store (Compose's `rememberViewModelStoreProvider(key, parent, …)`): parts
/// of the screen with the same key share it.
///
/// Kotlin: `rememberViewModelStoreProvider(key: Any?, parent: ViewModelStoreOwner? = …, defaultArgs: SavedState = savedState(), defaultCreationExtras: CreationExtras = …, defaultFactory: ViewModelProvider.Factory = …)`
#[kompozable]
pub fn remember_view_model_store_provider_keyed(
    key: impl Into<AnyKey>,
    #[default(Some(required_owner(NOT_PRESENT)))] parent: Option<Rc<dyn ViewModelStoreOwner>>,
    #[default(SavedState::new())] default_args: SavedState,
    #[default(None)] default_creation_extras: Option<CreationExtras>,
    #[default(None)] default_factory: Option<Rc<dyn ViewModelProviderFactory>>,
) -> ViewModelStoreProvider {
    provider_at(key.into(), parent, default_args, default_creation_extras, default_factory)
}

/// Compose's `rememberViewModelStoreProvider(key, parent, …)` body.
#[kompozable]
fn provider_at(
    key: AnyKey,
    parent: Option<Rc<dyn ViewModelStoreOwner>>,
    default_args: SavedState,
    default_creation_extras: Option<CreationExtras>,
    default_factory: Option<Rc<dyn ViewModelProviderFactory>>,
) -> ViewModelStoreProvider {
    let (p, k) = (parent.clone(), key.clone());
    let provider = remember_keyed((parent, key), move || {
        ViewModelStoreProvider::of_owner(p, k, default_args, default_creation_extras, default_factory)
    });
    let lifecycle = LocalLifecycleOwner.current().lifecycle();
    let pr = provider.clone();
    DisposableEffect((provider.clone(), lifecycle.clone()), move || {
        let token = pr.acquire_token(ViewModelStoreProvider::provider_marker_key());
        let (pr, lifecycle) = (pr.clone(), lifecycle.clone());
        on_dispose(move || {
            token.close();
            if lifecycle.current_state().is_at_least(LifecycleState::Created) {
                pr.clear_all_keys();
            }
        })
    });
    provider
}

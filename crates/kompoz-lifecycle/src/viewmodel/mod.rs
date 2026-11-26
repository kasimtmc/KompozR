//! androidx.lifecycle's ViewModels (lifecycle-viewmodel 2.11, lifecycle-viewmodel-savedstate, lifecycle-viewmodel-
//! compose): state and work that outlive the composables showing them —
//!
//! - [`ViewModel`] (its [`ViewModelImpl`]: closeables, [`view_model_scope`](ViewModelExtensions::view_model_scope),
//!   `onCleared`), [`ViewModelStore`] / [`ViewModelStoreOwner`], [`ViewModelProvider`] with its
//!   [`ViewModelProviderFactory`]s ([`NewInstanceFactory`], [`view_model_factory`]'s initializers),
//!   [`CreationExtras`];
//! - [`SavedStateHandle`] (a ViewModel's values saved with its owner's state), `enableSavedStateHandles`
//!   ([`SavedStateHandleSupport`]), [`CreationExtras::create_saved_state_handle`], [`SavedStateViewModelFactory`],
//!   [`ViewModelStoreProvider`] and the `ViewModelStoreOwner(…)` factories;
//! - viewmodel-compose: [`LocalViewModelStoreOwner`], `viewModel` ([`view_model`], [`view_model_with`],
//!   [`view_model_of`]), [`remember_view_model_store_owner`], [`remember_view_model_store_provider`],
//!   `SavedStateHandle.saveable` ([`SavedStateHandle::saveable`]).

mod compose;
mod handle;
mod provider;
mod store;
mod store_provider;

pub use compose::{
    LocalViewModelStoreOwner, LocalViewModelStoreOwnerLocal, ViewModelRef, remember_view_model_store_owner,
    remember_view_model_store_owner_keyed, remember_view_model_store_owner_of, remember_view_model_store_provider,
    remember_view_model_store_provider_keyed, view_model, view_model_of, view_model_with,
};
pub use handle::{
    DEFAULT_ARGS_KEY, SAVED_STATE_REGISTRY_OWNER_KEY, SavedStateHandle, SavedStateHandleSupport,
    SavedStateViewModelFactory, VIEW_MODEL_STORE_OWNER_KEY,
};
pub use provider::{
    CreationExtras, CreationExtrasKey, InitializerViewModelFactoryBuilder, MutableCreationExtras, NewInstanceFactory,
    OnRequeryFactory, VIEW_MODEL_KEY, ViewModelInitializer, ViewModelLazy, ViewModelProvider, ViewModelProviderFactory,
    view_model_factory,
};
pub use store::{
    AnyKey, DefaultViewModelProviderFactory, HasDefaultViewModelProviderFactory, ViewModelStore, ViewModelStoreOwner,
};
pub use store_provider::{
    ReferenceToken, ViewModelStoreOwner, ViewModelStoreOwner_of, ViewModelStoreOwner_with, ViewModelStoreProvider,
};

use kompoz_coroutines::{CoroutineContext, CoroutineScope, Dispatcher};
use kompoz_runtime::kt::AutoCloseable;
use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

/// What a screen's state and work live in, beyond the composables showing it (Compose's abstract class
/// `ViewModel`): made by a [`ViewModelProviderFactory`] the first time its owner (the app, a navigation entry) is
/// asked for it ([`viewModel`]), kept in its owner's [`ViewModelStore`] while the owner lives, cleared as it goes —
/// its resources closed (its [`view_model_scope`](ViewModelExtensions::view_model_scope) cancelled), then
/// [`on_cleared`](Self::on_cleared).
///
/// A ViewModel is a type of one's own holding a [`ViewModelImpl`] (what Kotlin's base class holds: made by the
/// constructor, as Kotlin's `ViewModel()` / `ViewModel(viewModelScope)` / `ViewModel(closeables)` are); the factories
/// make it from its [`new_instance`](Self::new_instance) (Kotlin finds the class's constructor by reflection):
///
/// ```
/// # use kompoz_lifecycle::*;
/// # use kompoz_coroutines::*;
/// struct CounterViewModel {
///     base: ViewModelImpl,
///     count: MutableStateFlow<i32>,
/// }
/// impl ViewModel for CounterViewModel {
///     fn view_model_impl(&self) -> &ViewModelImpl { &self.base }
///     fn new_instance(_extras: &CreationExtras) -> Option<Self> {
///         Some(CounterViewModel { base: ViewModelImpl::new(), count: MutableStateFlow::new(0) })
///     }
/// }
/// ```
///
/// A Kotlin class `class X : ViewModel() { … }` in the Kotlin syntax is such a type: its handle holding the
/// [`ViewModelImpl`], its `onCleared` override the trait's, its constructor (none, or a [`SavedStateHandle`]) its
/// [`new_instance`](Self::new_instance).
///
/// Kotlin: `abstract class ViewModel`
pub trait ViewModel: Any {
    /// Its base class's part (Kotlin's private `impl`): the closeables it closes as it is cleared.
    fn view_model_impl(&self) -> &ViewModelImpl;

    /// Called once as it is cleared, after its resources are closed (Compose's protected `onCleared()`).
    fn on_cleared(&self) {}

    /// It made by a factory that has only its class (Kotlin's reflective constructor: a public one taking nothing —
    /// `NewInstanceFactory` —, or, as Android's `SavedStateViewModelFactory` finds, one taking a
    /// [`SavedStateHandle`]: `extras.create_saved_state_handle()`). None (the default): such a factory can't make
    /// it ("Cannot create an instance of class …"): give it an initializer or a factory of its own.
    fn new_instance(extras: &CreationExtras) -> Option<Self>
    where
        Self: Sized,
    {
        let _ = extras;
        None
    }
}

impl dyn ViewModel {
    /// Clears it: its resources closed, then `on_cleared` (Compose's internal `clear()`, what its store does).
    pub(crate) fn clear(&self) {
        self.view_model_impl().clear();
        self.on_cleared();
    }

    /// It as its type, if it is a `T`.
    pub(crate) fn downcast_rc<T: ViewModel>(self: Rc<Self>) -> Option<Rc<T>> {
        let any: Rc<dyn Any> = self;
        any.downcast::<T>().ok()
    }
}

/// Kotlin's `x as T` / `x as? T` of a ViewModel (a factory's `create(T::class, …)`, typed `T` in Kotlin): the app's
/// own class, if it is one.
impl<T: Any + Clone> kompoz_runtime::kt::KtAs<T> for dyn ViewModel {
    fn kt_as_safe(&self) -> Option<T> {
        let any: &dyn Any = self;
        any.downcast_ref::<T>().cloned()
    }
}

impl fmt::Debug for dyn ViewModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ViewModel@{:p}", self as *const dyn ViewModel as *const ())
    }
}

/// Where a ViewModel's `viewModelScope` is kept among its closeables (Compose's `VIEW_MODEL_SCOPE_KEY`).
const VIEW_MODEL_SCOPE_KEY: &str = "androidx.lifecycle.viewmodel.internal.ViewModelCoroutineScope.JOB_KEY";

/// The part of a [`ViewModel`] its base class holds (Compose's internal `ViewModelImpl`): the resources closed as it
/// is cleared — those with a key first, then the constructor's, then those added without one —, and whether it
/// was. A resource added after it was cleared is closed at once.
#[derive(Default)]
pub struct ViewModelImpl {
    key_to_closeables: RefCell<Vec<(String, Rc<dyn AutoCloseable>)>>,
    closeables: RefCell<Vec<Rc<dyn AutoCloseable>>>,
    is_cleared: Cell<bool>,
}

impl ViewModelImpl {
    /// Kotlin's `ViewModel()`.
    pub fn new() -> ViewModelImpl {
        ViewModelImpl::default()
    }

    /// Kotlin's `ViewModel(viewModelScope)`: `view_model_scope` is its `viewModelScope`, cancelled as it is cleared.
    pub fn with_scope(view_model_scope: CoroutineScope) -> ViewModelImpl {
        let me = ViewModelImpl::new();
        me.add_closeable_with_key(VIEW_MODEL_SCOPE_KEY, Rc::new(CloseableCoroutineScope(view_model_scope)));
        me
    }

    /// Kotlin's `ViewModel(vararg closeables)`: closed as it is cleared.
    pub fn with_closeables(closeables: impl IntoIterator<Item = Rc<dyn AutoCloseable>>) -> ViewModelImpl {
        let me = ViewModelImpl::new();
        me.closeables.borrow_mut().extend(closeables);
        me
    }

    /// Kotlin's `ViewModel(viewModelScope, vararg closeables)`.
    pub fn with_scope_and_closeables(
        view_model_scope: CoroutineScope,
        closeables: impl IntoIterator<Item = Rc<dyn AutoCloseable>>,
    ) -> ViewModelImpl {
        let me = ViewModelImpl::with_scope(view_model_scope);
        me.closeables.borrow_mut().extend(closeables);
        me
    }

    fn clear(&self) {
        if self.is_cleared.get() {
            return;
        }
        self.is_cleared.set(true);
        let keyed: Vec<Rc<dyn AutoCloseable>> =
            self.key_to_closeables.borrow().iter().map(|(_, c)| c.clone()).collect();
        for c in keyed {
            c.close();
        }
        let plain = std::mem::take(&mut *self.closeables.borrow_mut());
        for c in plain {
            c.close();
        }
    }

    /// Whether it was cleared.
    pub fn is_cleared(&self) -> bool {
        self.is_cleared.get()
    }

    /// Closes `closeable` as it is cleared, under `key` (the one there before closed at once); cleared already: closes
    /// it at once (Compose's `addCloseable(key, closeable)`).
    ///
    /// Kotlin: `addCloseable(key: String, closeable: AutoCloseable)`
    pub fn add_closeable_with_key(&self, key: &str, closeable: Rc<dyn AutoCloseable>) {
        if self.is_cleared.get() {
            closeable.close();
            return;
        }
        let old = {
            let mut keyed = self.key_to_closeables.borrow_mut();
            match keyed.iter_mut().find(|(k, _)| k == key) {
                Some((_, c)) => Some(std::mem::replace(c, closeable)),
                None => {
                    keyed.push((key.to_string(), closeable));
                    None
                }
            }
        };
        if let Some(old) = old {
            old.close();
        }
    }

    /// Closes `closeable` as it is cleared; cleared already: at once (Compose's `addCloseable(closeable)`).
    pub fn add_closeable(&self, closeable: Rc<dyn AutoCloseable>) {
        if self.is_cleared.get() {
            closeable.close();
            return;
        }
        let mut closeables = self.closeables.borrow_mut();
        if !closeables.iter().any(|c| Rc::ptr_eq(c, &closeable)) {
            closeables.push(closeable);
        }
    }

    /// The resource added under `key`, if it is a `T` (Compose's `getCloseable<T>(key)`).
    pub fn get_closeable<T: AutoCloseable>(&self, key: &str) -> Option<Rc<T>> {
        let c = self.key_to_closeables.borrow().iter().find(|(k, _)| k == key).map(|(_, c)| c.clone())?;
        let any: Rc<dyn Any> = c;
        any.downcast::<T>().ok()
    }

    /// Its `viewModelScope` (Compose's `ViewModel.viewModelScope`): made the first time, on the main dispatcher with
    /// a supervisor job, cancelled as it is cleared (right before `on_cleared`). What a ViewModel's constructor
    /// launches on (`flow.stateIn(viewModelScope, …)`). Made outside a host (no main dispatcher: where Kotlin falls
    /// back to `EmptyCoroutineContext`), on a dispatcher of its own, which runs what is launched when it is driven.
    pub fn view_model_scope(&self) -> CoroutineScope {
        if let Some(scope) = self.get_closeable::<CloseableCoroutineScope>(VIEW_MODEL_SCOPE_KEY) {
            return scope.0.clone();
        }
        // (Kotlin's `createViewModelScope`: Dispatchers.Main.immediate + SupervisorJob())
        let dispatcher = Dispatcher::try_main().unwrap_or_default();
        let scope = CoroutineScope::new_supervisor(&dispatcher, CoroutineContext::new());
        self.add_closeable_with_key(VIEW_MODEL_SCOPE_KEY, Rc::new(CloseableCoroutineScope(scope.clone())));
        scope
    }
}

/// A coroutine scope closed by cancelling it (Compose's internal `CloseableCoroutineScope`).
pub struct CloseableCoroutineScope(pub CoroutineScope);

impl AutoCloseable for CloseableCoroutineScope {
    fn close(&self) {
        self.0.cancel();
    }
}

/// A ViewModel's members of its base class, for every ViewModel (Compose's `ViewModel` methods and its
/// `viewModelScope` extension): through its [`ViewModelImpl`].
pub trait ViewModelExtensions: ViewModel {
    /// Its coroutine scope (Compose's `viewModelScope`): cancelled as it is cleared.
    fn view_model_scope(&self) -> CoroutineScope {
        self.view_model_impl().view_model_scope()
    }

    /// Kotlin: `addCloseable(key: String, closeable: AutoCloseable)`
    fn add_closeable_with_key(&self, key: &str, closeable: Rc<dyn AutoCloseable>) {
        self.view_model_impl().add_closeable_with_key(key, closeable);
    }

    /// Kotlin: `addCloseable(closeable: AutoCloseable)`
    fn add_closeable(&self, closeable: Rc<dyn AutoCloseable>) {
        self.view_model_impl().add_closeable(closeable);
    }

    /// Kotlin: `getCloseable(key: String)`
    fn get_closeable<T: AutoCloseable>(&self, key: &str) -> Option<Rc<T>> {
        self.view_model_impl().get_closeable(key)
    }
}

impl<V: ViewModel + ?Sized> ViewModelExtensions for V {}

/// A ViewModel class, as a factory is given it (Kotlin's `KClass<T>` of a ViewModel): its type, its name, and how
/// it is made by a factory that has only its class ([`ViewModel::new_instance`]). `KClass::of::<T>()` is Kotlin's
/// `T::class`.
#[derive(Clone, Copy)]
pub struct KClass {
    type_id: TypeId,
    qualified_name: &'static str,
    new_instance: fn(&CreationExtras) -> Option<Rc<dyn ViewModel>>,
}

fn new_instance_of<T: ViewModel>(extras: &CreationExtras) -> Option<Rc<dyn ViewModel>> {
    T::new_instance(extras).map(|vm| Rc::new(vm) as Rc<dyn ViewModel>)
}

impl KClass {
    /// `T`'s class (Kotlin's `T::class`).
    pub fn of<T: ViewModel>() -> KClass {
        KClass {
            type_id: TypeId::of::<T>(),
            qualified_name: std::any::type_name::<T>(),
            new_instance: new_instance_of::<T>,
        }
    }

    /// Its JVM class (Kotlin's `T::class.java`, what Android's factories are given): the class itself.
    pub fn java(&self) -> KClass {
        *self
    }

    /// Its full name (Kotlin's `qualifiedName`; Rust's type name, with its path).
    pub fn qualified_name(&self) -> &'static str {
        self.qualified_name
    }

    /// Kotlin's `canonicalName` (what a provider's default key is made of): its full name.
    pub fn canonical_name(&self) -> &'static str {
        self.qualified_name
    }

    /// Its name without its path (Kotlin's `simpleName`).
    pub fn simple_name(&self) -> &'static str {
        let base = self.qualified_name.split('<').next().unwrap_or(self.qualified_name);
        base.rsplit("::").next().unwrap_or(base)
    }

    /// Whether `value` is one (Kotlin's `isInstance`).
    pub fn is_instance(&self, value: &dyn ViewModel) -> bool {
        let any: &dyn Any = value;
        any.type_id() == self.type_id
    }

    /// One made as a factory with only its class makes it (Kotlin's reflective construction): panics if it can't be
    /// ("Cannot create an instance of class …", Kotlin's RuntimeException).
    #[track_caller]
    pub fn create_instance(&self, extras: &CreationExtras) -> Rc<dyn ViewModel> {
        (self.new_instance)(extras)
            .unwrap_or_else(|| panic!("RuntimeException: Cannot create an instance of class {}", self.qualified_name))
    }
}

impl PartialEq for KClass {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}

impl Eq for KClass {}

impl std::hash::Hash for KClass {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
    }
}

impl fmt::Debug for KClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "class {}", self.qualified_name)
    }
}

//! lifecycle-viewmodel's CreationExtras.kt, ViewModelProvider.kt (its desktop actual, ViewModelProviderImpl,
//! ViewModelProviders, ViewModelProviderGet.kt), InitializerViewModelFactory.kt and ViewModelLazy.kt.

use super::store::{DefaultViewModelProviderFactory, ViewModelStore, ViewModelStoreOwner};
use super::{KClass, ViewModel};
use kompoz_runtime::kt::KtDelegate;
use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

// ---- CreationExtras ----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum KeyId {
    Named(&'static str),
    Made(u64),
}

/// A key of [`CreationExtras`], of the type of its value (Compose's `CreationExtras.Key<T>`): two keys are the same
/// when they are the same key (Kotlin's objects), made by [`new`](Self::new) (Kotlin's `CreationExtras.Key<T>()`)
/// or, for a library's, [`named`](Self::named).
///
/// Kotlin: `CreationExtras.Key`
pub struct CreationExtrasKey<T> {
    id: KeyId,
    _value: PhantomData<fn() -> T>,
}

impl<T> Clone for CreationExtrasKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for CreationExtrasKey<T> {}
impl<T> PartialEq for CreationExtrasKey<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> fmt::Debug for CreationExtrasKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = std::any::type_name::<T>();
        match self.id {
            KeyId::Named(n) => write!(f, "CreationExtras.Key({n})<{name}>"),
            KeyId::Made(n) => write!(f, "CreationExtras.Key@{n:x}<{name}>"),
        }
    }
}

impl<T> CreationExtrasKey<T> {
    /// A key of its own (Compose's `CreationExtras.Key<T>()`): no other is the same.
    #[allow(clippy::new_without_default)]
    pub fn new() -> CreationExtrasKey<T> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        CreationExtrasKey { id: KeyId::Made(NEXT.fetch_add(1, Ordering::Relaxed)), _value: PhantomData }
    }

    /// The key named `name` (a constant: the library's keys, `VIEW_MODEL_KEY`…): keys of one name are the same.
    pub const fn named(name: &'static str) -> CreationExtrasKey<T> {
        CreationExtrasKey { id: KeyId::Named(name), _value: PhantomData }
    }
}

/// What a [`ViewModelProviderFactory`] is given beside the class to make a ViewModel with (Compose's
/// `CreationExtras`): values by [`CreationExtrasKey`] — the ViewModel's key ([`ViewModelProvider::VIEW_MODEL_KEY`]),
/// its owner and saved state registry owner (for a [`SavedStateHandle`](super::SavedStateHandle)), default
/// arguments. A clone holds the same values. Equal when they hold the same values under the same keys.
#[derive(Clone, Default)]
pub struct CreationExtras {
    extras: RefCell<Vec<(KeyId, Rc<dyn Any>)>>,
}

impl PartialEq for CreationExtras {
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = (self.extras.borrow(), other.extras.borrow());
        a.len() == b.len()
            && a.iter()
                .all(|(k, v)| b.iter().any(|(k2, v2)| k == k2 && std::ptr::addr_eq(Rc::as_ptr(v), Rc::as_ptr(v2))))
    }
}

impl fmt::Debug for CreationExtras {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys: Vec<String> = self.extras.borrow().iter().map(|(k, _)| format!("{k:?}")).collect();
        write!(f, "CreationExtras(extras=[{}])", keys.join(", "))
    }
}

#[allow(non_upper_case_globals)]
impl CreationExtras {
    /// None (Compose's `CreationExtras.Empty`).
    #[allow(clippy::declare_interior_mutable_const)] // (each use a fresh empty value: nothing is shared)
    pub const Empty: CreationExtras = CreationExtras { extras: RefCell::new(Vec::new()) };

    /// The value of `key`, if it has one (Compose's `get` operator).
    pub fn get<T: Clone + 'static>(&self, key: &CreationExtrasKey<T>) -> Option<T> {
        let extras = self.extras.borrow();
        let (_, v) = extras.iter().find(|(k, _)| *k == key.id)?;
        v.downcast_ref::<T>().cloned()
    }

    /// Whether it has a value of `key` (Compose's `contains` operator, `key in extras`).
    pub fn contains<T>(&self, key: &CreationExtrasKey<T>) -> bool {
        self.extras.borrow().iter().any(|(k, _)| *k == key.id)
    }

    fn put(&self, id: KeyId, value: Rc<dyn Any>) {
        let mut extras = self.extras.borrow_mut();
        match extras.iter_mut().find(|(k, _)| *k == id) {
            Some((_, v)) => *v = value,
            None => extras.push((id, value)),
        }
    }

    fn put_all(&self, from: &CreationExtras) {
        if std::ptr::eq(self, from) {
            return;
        }
        let entries = from.extras.borrow().clone();
        for (k, v) in entries {
            self.put(k, v);
        }
    }
}

/// `self` and `creation_extras`'s values, `creation_extras`'s winning (Compose's `plus` operator).
impl std::ops::Add<&CreationExtras> for &CreationExtras {
    type Output = MutableCreationExtras;
    fn add(self, creation_extras: &CreationExtras) -> MutableCreationExtras {
        let out = MutableCreationExtras::new(self);
        out.0.put_all(creation_extras);
        out
    }
}

/// Extras a factory's caller fills (Compose's `MutableCreationExtras`): it is the [`CreationExtras`] it holds
/// (Deref), and [`set`](Self::set)s values.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct MutableCreationExtras(CreationExtras);

impl MutableCreationExtras {
    /// Extras with `initial_extras`'s values (a copy) (Compose's `MutableCreationExtras(initialExtras = Empty)`).
    ///
    /// Kotlin: `MutableCreationExtras(initialExtras: CreationExtras = CreationExtras.Empty)`
    pub fn new(initial_extras: &CreationExtras) -> MutableCreationExtras {
        let me = MutableCreationExtras::default();
        me.0.put_all(initial_extras);
        me
    }

    /// `t` as the value of `key` (Compose's `set` operator, `extras[key] = t`).
    pub fn set<T: 'static>(&self, key: &CreationExtrasKey<T>, t: T) {
        self.0.put(key.id, Rc::new(t));
    }
}

impl std::ops::Deref for MutableCreationExtras {
    type Target = CreationExtras;
    fn deref(&self) -> &CreationExtras {
        &self.0
    }
}

impl From<MutableCreationExtras> for CreationExtras {
    fn from(m: MutableCreationExtras) -> CreationExtras {
        m.0
    }
}

/// `creation_extras`'s values added (Compose's `plusAssign` operator).
impl std::ops::AddAssign<&CreationExtras> for MutableCreationExtras {
    fn add_assign(&mut self, creation_extras: &CreationExtras) {
        self.0.put_all(creation_extras);
    }
}

/// Extras of `initial_extras`'s values, `builder_action` setting more (Compose's `CreationExtras(initialExtras) {
/// … }`).
///
/// Kotlin: `CreationExtras(initialExtras: CreationExtras = CreationExtras.Empty, builderAction: MutableCreationExtras.() -> Unit = {})`
#[allow(non_snake_case)]
pub fn CreationExtras(
    initial_extras: &CreationExtras,
    builder_action: impl FnOnce(&MutableCreationExtras),
) -> CreationExtras {
    let extras = MutableCreationExtras::new(initial_extras);
    builder_action(&extras);
    extras.0
}

// ---- Factories ----

/// What makes the ViewModels a [`ViewModelProvider`] doesn't have yet (Compose's `ViewModelProvider.Factory`):
/// given the class and the [`CreationExtras`], it gives a ViewModel of that class.
///
/// Kotlin: `ViewModelProvider.Factory`
pub trait ViewModelProviderFactory: Any {
    /// A new ViewModel of `model_class` (Compose's `create(modelClass, extras)`). The default can't: it panics
    /// (Kotlin's UnsupportedOperationException).
    #[track_caller]
    fn create(&self, model_class: &KClass, extras: &CreationExtras) -> Rc<dyn ViewModel> {
        let _ = (model_class, extras);
        panic!(
            "UnsupportedOperationException: `Factory.create(String, CreationExtras)` is not implemented. You may need \
             to override the method and provide a custom implementation. Note that using `Factory.create(String)` is \
             not supported and considered an error."
        )
    }

    /// The factory as one told each time a provider gives a ViewModel it had (Kotlin's `is OnRequeryFactory`).
    fn as_on_requery_factory(&self) -> Option<&dyn OnRequeryFactory> {
        None
    }
}

/// A factory told of each ViewModel a provider gives that it had already (Compose's LIBRARY_GROUP
/// `ViewModelProvider.OnRequeryFactory`).
///
/// Kotlin: `ViewModelProvider.OnRequeryFactory`
pub trait OnRequeryFactory {
    fn on_requery(&self, view_model: &Rc<dyn ViewModel>) {
        let _ = view_model;
    }
}

/// The factory making a class by its own construction (Compose's desktop `ViewModelProvider.NewInstanceFactory`, a
/// public constructor taking nothing found by reflection: here [`ViewModel::new_instance`]).
///
/// Kotlin: `ViewModelProvider.NewInstanceFactory`
#[derive(Clone, Copy, Default)]
pub struct NewInstanceFactory;

impl NewInstanceFactory {
    /// The one (Compose's `NewInstanceFactory.instance`, LIBRARY_GROUP).
    pub fn instance() -> Rc<NewInstanceFactory> {
        Rc::new(NewInstanceFactory)
    }
}

impl ViewModelProviderFactory for NewInstanceFactory {
    fn create(&self, model_class: &KClass, extras: &CreationExtras) -> Rc<dyn ViewModel> {
        model_class.create_instance(extras)
    }
}

// ---- ViewModelProvider ----

/// Gives the ViewModels of a store, made by its factory the first time each is asked for (Compose's
/// `ViewModelProvider`): by class (under the class's default key) or by key.
pub struct ViewModelProvider {
    store: ViewModelStore,
    factory: Rc<dyn ViewModelProviderFactory>,
    default_extras: CreationExtras,
}

/// The key a provider keeps a class's ViewModel under when it is asked without one (Compose's
/// `VIEW_MODEL_PROVIDER_DEFAULT_KEY`).
const VIEW_MODEL_PROVIDER_DEFAULT_KEY: &str = "androidx.lifecycle.ViewModelProvider.DefaultKey";

/// The key of the ViewModel being made, among the extras its factory is given (Compose's
/// `ViewModelProvider.VIEW_MODEL_KEY`).
pub const VIEW_MODEL_KEY: CreationExtrasKey<String> = CreationExtrasKey::named("ViewModelProvider.VIEW_MODEL_KEY");

impl ViewModelProvider {
    /// A provider of `owner`'s store, its ViewModels made by `factory` (none: the owner's default) with `extras`
    /// (none: the owner's default) (Compose's `ViewModelProvider.create(owner, factory, extras)`).
    ///
    /// Kotlin: `create(owner: ViewModelStoreOwner, factory: ViewModelProvider.Factory? = null, extras: CreationExtras? = null)`
    pub fn create(
        owner: &dyn ViewModelStoreOwner,
        factory: Option<Rc<dyn ViewModelProviderFactory>>,
        extras: Option<CreationExtras>,
    ) -> ViewModelProvider {
        ViewModelProvider {
            store: owner.view_model_store(),
            factory: factory.unwrap_or_else(|| owner.default_view_model_provider_factory()),
            default_extras: extras.unwrap_or_else(|| owner.default_view_model_creation_extras()),
        }
    }

    /// A provider of `store`, its ViewModels made by `factory` (none: the default factory, a class's own
    /// construction) with `extras` (none: none) (Compose's `ViewModelProvider.create(store, factory, extras)`).
    ///
    /// Kotlin: `create(store: ViewModelStore, factory: ViewModelProvider.Factory? = null, extras: CreationExtras? = null)`
    pub fn create_from_store(
        store: ViewModelStore,
        factory: Option<Rc<dyn ViewModelProviderFactory>>,
        extras: Option<CreationExtras>,
    ) -> ViewModelProvider {
        ViewModelProvider {
            store,
            factory: factory.unwrap_or_else(|| Rc::new(DefaultViewModelProviderFactory)),
            default_extras: extras.unwrap_or(CreationExtras::Empty),
        }
    }
}

impl ViewModelProvider {
    /// The key of the ViewModel being made (Compose's `ViewModelProvider.VIEW_MODEL_KEY`).
    pub const VIEW_MODEL_KEY: CreationExtrasKey<String> = VIEW_MODEL_KEY;

    /// The `T` of its store, made the first time (Compose's `get<VM>()`, `get(VM::class)`).
    #[track_caller]
    pub fn get<T: ViewModel>(&self) -> Rc<T> {
        let class = KClass::of::<T>();
        let key = default_key(&class);
        downcast(self.get_view_model(&class, &key))
    }

    /// The `T` under `key` (Compose's `get<VM>(key)`, `get(key, VM::class)`).
    ///
    /// Kotlin: `get(key: String)`
    #[track_caller]
    pub fn get_keyed<T: ViewModel>(&self, key: &str) -> Rc<T> {
        downcast(self.get_view_model(&KClass::of::<T>(), key))
    }

    /// The ViewModel of `model_class` (Compose's `get(modelClass)`).
    ///
    /// Kotlin: `get(modelClass: KClass<T>)`
    pub fn get_class(&self, model_class: &KClass) -> Rc<dyn ViewModel> {
        self.get_view_model(model_class, &default_key(model_class))
    }

    /// The ViewModel of `model_class` under `key` (Compose's `get(key, modelClass)`).
    ///
    /// Kotlin: `get(key: String, modelClass: KClass<T>)`
    pub fn get_class_keyed(&self, key: &str, model_class: &KClass) -> Rc<dyn ViewModel> {
        self.get_view_model(model_class, key)
    }

    /// Compose's `ViewModelProviderImpl.getViewModel`: the one under `key` if it is of the class (the factory told
    /// if it is an OnRequeryFactory), else one made now (the extras given its key), kept under `key` (the one there
    /// before cleared).
    fn get_view_model(&self, model_class: &KClass, key: &str) -> Rc<dyn ViewModel> {
        if let Some(view_model) = self.store.get(key)
            && model_class.is_instance(&*view_model)
        {
            if let Some(requery) = self.factory.as_on_requery_factory() {
                requery.on_requery(&view_model);
            }
            return view_model;
        }
        let model_extras = MutableCreationExtras::new(&self.default_extras);
        model_extras.set(&VIEW_MODEL_KEY, key.to_string());
        let view_model = self.factory.create(model_class, &model_extras);
        self.store.put(key, view_model.clone());
        view_model
    }
}

/// A class's default key (Compose's `ViewModelProviders.getDefaultKey`).
pub(crate) fn default_key(model_class: &KClass) -> String {
    format!("{VIEW_MODEL_PROVIDER_DEFAULT_KEY}:{}", model_class.canonical_name())
}

#[track_caller]
pub(crate) fn downcast<T: ViewModel>(view_model: Rc<dyn ViewModel>) -> Rc<T> {
    view_model.downcast_rc::<T>().unwrap_or_else(|| {
        panic!("ClassCastException: the factory made a ViewModel that isn't a {}", std::any::type_name::<T>())
    })
}

// ---- Initializers ----

/// How a ViewModel class is made by a [`view_model_factory`] (Compose's `ViewModelInitializer`).
#[derive(Clone)]
pub struct ViewModelInitializer {
    clazz: KClass,
    initializer: Rc<dyn Fn(&CreationExtras) -> Rc<dyn ViewModel>>,
}

impl ViewModelInitializer {
    /// `initializer` makes the `clazz`es (Compose's `ViewModelInitializer(clazz, initializer)`).
    pub fn new<T: ViewModel>(
        clazz: KClass,
        initializer: impl Fn(&CreationExtras) -> T + 'static,
    ) -> ViewModelInitializer {
        ViewModelInitializer {
            clazz,
            initializer: Rc::new(move |extras| Rc::new(initializer(extras)) as Rc<dyn ViewModel>),
        }
    }
}

/// The receiver of [`view_model_factory`]'s block (Compose's `InitializerViewModelFactoryBuilder`): an initializer
/// per class.
#[derive(Default)]
pub struct InitializerViewModelFactoryBuilder {
    initializers: RefCell<Vec<ViewModelInitializer>>,
}

impl InitializerViewModelFactoryBuilder {
    pub fn new() -> InitializerViewModelFactoryBuilder {
        InitializerViewModelFactoryBuilder::default()
    }

    /// `initializer` makes the `clazz`es (Compose's `addInitializer`).
    ///
    /// # Panics
    /// If `clazz` has one already ("A `initializer` with the same `clazz` has already been added: …").
    #[track_caller]
    pub fn add_initializer<T: ViewModel>(&self, clazz: KClass, initializer: impl Fn(&CreationExtras) -> T + 'static) {
        assert!(
            !self.initializers.borrow().iter().any(|i| i.clazz == clazz),
            "IllegalArgumentException: A `initializer` with the same `clazz` has already been added: {}.",
            clazz.canonical_name()
        );
        self.initializers.borrow_mut().push(ViewModelInitializer::new(clazz, initializer));
    }

    /// `initializer` makes the `VM`s (Compose's `initializer { … }`, the block's receiver the extras).
    #[track_caller]
    pub fn initializer<VM: ViewModel>(&self, initializer: impl Fn(&CreationExtras) -> VM + 'static) {
        self.add_initializer(KClass::of::<VM>(), initializer);
    }

    /// The factory of its initializers (Compose's `build()`).
    pub fn build(&self) -> Rc<dyn ViewModelProviderFactory> {
        Rc::new(InitializerViewModelFactory { initializers: self.initializers.borrow().clone() })
    }
}

/// A factory of initializers, `builder` adding them (Compose's `viewModelFactory { initializer { … } }`).
///
/// ```
/// # use kompoz_lifecycle::*;
/// struct Search { base: ViewModelImpl, handle: SavedStateHandle }
/// impl ViewModel for Search { fn view_model_impl(&self) -> &ViewModelImpl { &self.base } }
/// let factory = view_model_factory(|this| {
///     this.initializer(|extras| Search { base: ViewModelImpl::new(), handle: extras.create_saved_state_handle() })
/// });
/// # let _ = factory;
/// ```
pub fn view_model_factory(
    builder: impl FnOnce(&InitializerViewModelFactoryBuilder),
) -> Rc<dyn ViewModelProviderFactory> {
    let b = InitializerViewModelFactoryBuilder::new();
    builder(&b);
    b.build()
}

/// Compose's internal `InitializerViewModelFactory`.
struct InitializerViewModelFactory {
    initializers: Vec<ViewModelInitializer>,
}

impl ViewModelProviderFactory for InitializerViewModelFactory {
    fn create(&self, model_class: &KClass, extras: &CreationExtras) -> Rc<dyn ViewModel> {
        let initializer = self.initializers.iter().find(|i| i.clazz == *model_class).unwrap_or_else(|| {
            panic!("IllegalArgumentException: No initializer set for given class {}", model_class.canonical_name())
        });
        (initializer.initializer)(extras)
    }
}

// ---- ViewModelLazy ----

/// A ViewModel got the first time it is read (Compose's `ViewModelLazy`, a `Lazy<VM>`: `val vm by
/// ViewModelLazy(…)`): from a provider of `store_producer()`'s store with `factory_producer()`'s factory and
/// `extras_producer()`'s extras.
pub struct ViewModelLazy<VM: ViewModel> {
    store_producer: Box<dyn Fn() -> ViewModelStore>,
    factory_producer: Box<dyn Fn() -> Rc<dyn ViewModelProviderFactory>>,
    extras_producer: Box<dyn Fn() -> CreationExtras>,
    cached: RefCell<Option<Rc<VM>>>,
}

impl<VM: ViewModel> ViewModelLazy<VM> {
    /// Kotlin: `ViewModelLazy(viewModelClass: KClass<VM>, storeProducer: () -> ViewModelStore, factoryProducer: () -> ViewModelProvider.Factory, extrasProducer: () -> CreationExtras = { CreationExtras.Empty })`
    pub fn new(
        store_producer: impl Fn() -> ViewModelStore + 'static,
        factory_producer: impl Fn() -> Rc<dyn ViewModelProviderFactory> + 'static,
        extras_producer: impl Fn() -> CreationExtras + 'static,
    ) -> ViewModelLazy<VM> {
        ViewModelLazy {
            store_producer: Box::new(store_producer),
            factory_producer: Box::new(factory_producer),
            extras_producer: Box::new(extras_producer),
            cached: RefCell::new(None),
        }
    }
}

impl<VM: ViewModel> ViewModelLazy<VM> {
    /// The ViewModel (got the first time).
    pub fn value(&self) -> Rc<VM> {
        if let Some(vm) = self.cached.borrow().clone() {
            return vm;
        }
        let provider = ViewModelProvider {
            store: (self.store_producer)(),
            factory: (self.factory_producer)(),
            default_extras: (self.extras_producer)(),
        };
        let vm = provider.get::<VM>();
        *self.cached.borrow_mut() = Some(vm.clone());
        vm
    }

    /// Whether it was got already (Compose's `isInitialized()`).
    pub fn is_initialized(&self) -> bool {
        self.cached.borrow().is_some()
    }
}

impl<VM: ViewModel> KtDelegate for ViewModelLazy<VM> {
    type Value = Rc<VM>;
    fn get_value(&self) -> Rc<VM> {
        self.value()
    }
}

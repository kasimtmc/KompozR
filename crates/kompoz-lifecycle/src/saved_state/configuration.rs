//! savedstate's SavedStateConfiguration (SavedStateConfiguration.kt) and the part of kotlinx.serialization's
//! `SerializersModule` it carries that KompozR's savers use: the subclasses of an open (polymorphic) base, each with its
//! serializer, by which a value saved with its class's name is read back (kotlinx's open polymorphism; Android finds
//! the class by reflection without one). A configuration given to `encodeToSavedState`/`decodeFromSavedState`/
//! `rememberSerializable`/`rememberNavBackStack` is in effect while they save and restore: what a polymorphic value's
//! reader asks [`polymorphic_restorer`] for.

use kompoz_runtime::{Saveable, SavedValue, Saver};
use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

/// A class, as a value (Kotlin's `KClass<T>` of `Home::class`, `NavKey::class`): what `SerializersModule`'s
/// `polymorphic(baseClass)` and `subclass(clazz)` take. An interface's is the trait object's (`SerialClass<dyn NavKey>`).
/// (A ViewModel's class, what a factory is given, is [`KClass`](crate::KClass).)
pub struct SerialClass<T: ?Sized + 'static>(PhantomData<fn() -> Box<T>>);

impl<T: ?Sized + 'static> SerialClass<T> {
    /// `T`'s class (Kotlin's `T::class`).
    pub fn of() -> SerialClass<T> {
        SerialClass(PhantomData)
    }

    /// Its qualified name (Kotlin's `qualifiedName`: Rust's name of the type).
    pub fn qualified_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

impl<T: ?Sized + 'static> Clone for SerialClass<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized + 'static> Copy for SerialClass<T> {}

impl<T: ?Sized + 'static> fmt::Debug for SerialClass<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "class {}", std::any::type_name::<T>())
    }
}

/// An open base whose values a subclass `K`'s are (kotlinx's `polymorphic(Base::class) { subclass(K::class) }`):
/// implemented by the base's trait object for its subclasses (`dyn NavKey` for every `NavKey`), the value made one.
pub trait PolymorphicBase<K>: 'static {
    /// `value` as a value of the base (Kotlin's upcast).
    fn upcast(value: K) -> Rc<Self>;
}

/// One subclass of an open base: its class's name (what a value saved with its class says, Rust's name of the type)
/// and what reads a value of it back as the base's (an `Rc<dyn Fn(&SavedValue) -> Option<Rc<B>>>`, held as `Any`).
#[derive(Clone)]
struct Subclass {
    base: std::any::TypeId,
    name: &'static str,
    restore: Rc<dyn Any>,
}

type Restore<B> = Rc<dyn Fn(&SavedValue) -> Option<Rc<B>>>;

/// The serializers a configuration adds (kotlinx.serialization's `SerializersModule`): the subclasses of open bases,
/// each with its serializer (`polymorphic(NavKey::class) { subclass(Home::class) }`). Empty by default
/// (`EmptySerializersModule()`).
#[derive(Clone, Default)]
pub struct SerializersModule {
    subclasses: Rc<Vec<Subclass>>,
}

impl SerializersModule {
    /// What reads back a value of the subclass of `B` named `name`, if this module has it.
    fn restorer<B: ?Sized + 'static>(&self, name: &str) -> Option<Restore<B>> {
        let base = std::any::TypeId::of::<B>();
        self.subclasses
            .iter()
            .rev()
            .find(|s| s.base == base && s.name == name)
            .and_then(|s| s.restore.downcast_ref::<Restore<B>>().cloned())
    }

    /// The names of the subclasses of `B` it has, in the order they were given.
    pub fn subclass_names<B: ?Sized + 'static>(&self) -> Vec<&'static str> {
        let base = std::any::TypeId::of::<B>();
        self.subclasses.iter().filter(|s| s.base == base).map(|s| s.name).collect()
    }

    /// This module and `other`'s, `other`'s taking precedence (kotlinx's `overwriteWith`, `+`).
    pub fn overwrite_with(&self, other: &SerializersModule) -> SerializersModule {
        let mut all: Vec<Subclass> = (*self.subclasses).clone();
        all.extend(other.subclasses.iter().cloned());
        SerializersModule { subclasses: Rc::new(all) }
    }
}

impl fmt::Debug for SerializersModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.subclasses.iter().map(|s| s.name)).finish()
    }
}

/// A module of what `builder_action` registers (kotlinx's `SerializersModule { … }`).
///
/// Kotlin: `SerializersModule(builderAction: SerializersModuleBuilder.() -> Unit)`
pub fn SerializersModule(builder_action: impl FnOnce(&mut SerializersModuleBuilder)) -> SerializersModule {
    let mut b = SerializersModuleBuilder { subclasses: Vec::new() };
    builder_action(&mut b);
    SerializersModule { subclasses: Rc::new(b.subclasses) }
}

/// What `SerializersModule { … }`'s block registers with (kotlinx's `SerializersModuleBuilder`).
pub struct SerializersModuleBuilder {
    subclasses: Vec<Subclass>,
}

impl SerializersModuleBuilder {
    /// The subclasses of the open base `base_class` that `builder_action` registers (kotlinx's `polymorphic(baseClass)
    /// { subclass(…) }`).
    ///
    /// Kotlin: `polymorphic(baseClass: KClass<Base>, builderAction: PolymorphicModuleBuilder<Base>.() -> Unit = {})`
    pub fn polymorphic<B: ?Sized + 'static>(
        &mut self,
        base_class: SerialClass<B>,
        builder_action: impl FnOnce(&mut PolymorphicModuleBuilder<B>),
    ) {
        let _ = base_class;
        let mut b = PolymorphicModuleBuilder { subclasses: Vec::new(), base: PhantomData };
        builder_action(&mut b);
        self.subclasses.extend(b.subclasses);
    }

    /// The module `other`'s registrations too (kotlinx's `include(module)`).
    pub fn include(&mut self, module: &SerializersModule) {
        self.subclasses.extend(module.subclasses.iter().cloned());
    }
}

/// What `polymorphic(Base::class) { … }`'s block registers the base's subclasses with (kotlinx's
/// `PolymorphicModuleBuilder<Base>`).
pub struct PolymorphicModuleBuilder<B: ?Sized + 'static> {
    subclasses: Vec<Subclass>,
    base: PhantomData<fn() -> Box<B>>,
}

impl<B: ?Sized + 'static> PolymorphicModuleBuilder<B> {
    /// The subclass `clazz`, its serializer its type's (kotlinx's reified `subclass(K::class)`).
    ///
    /// Kotlin: `subclass(clazz: KClass<T>)`
    pub fn subclass<K: Saveable + 'static>(&mut self, clazz: SerialClass<K>)
    where
        B: PolymorphicBase<K>,
    {
        let _ = clazz;
        self.subclass_serializer(Saver::<K>::of());
    }

    /// The subclass `serializer` writes and reads (kotlinx's `subclass(K.serializer())`).
    ///
    /// Kotlin: `subclass(serializer: KSerializer<T>)`
    pub fn subclass_serializer<K: 'static>(&mut self, serializer: Saver<K>)
    where
        B: PolymorphicBase<K>,
    {
        let restore: Restore<B> = Rc::new(move |v: &SavedValue| serializer.restore(v).map(B::upcast));
        self.subclasses.push(Subclass {
            base: std::any::TypeId::of::<B>(),
            name: std::any::type_name::<K>(),
            restore: Rc::new(restore),
        });
    }
}

/// How savedstate encodes and decodes (savedstate's `SavedStateConfiguration`): the serializers module its
/// polymorphic values' subclasses are read back by. (KompozR's savers write every property and a polymorphic value's
/// class whatever is configured: kotlinx's `encodeDefaults` and `classDiscriminatorMode` have no part in them.)
#[derive(Clone, Default, Debug)]
pub struct SavedStateConfiguration {
    serializers_module: SerializersModule,
}

impl SavedStateConfiguration {
    /// The default configuration (Kotlin's `SavedStateConfiguration.DEFAULT`): no module of its own.
    ///
    /// Kotlin: `DEFAULT`
    pub fn default_configuration() -> SavedStateConfiguration {
        SavedStateConfiguration::default()
    }

    /// Its serializers module (Kotlin's `serializersModule`).
    pub fn serializers_module(&self) -> SerializersModule {
        self.serializers_module.clone()
    }

    /// `block` with this configuration in effect: what a polymorphic value's reader finds its subclasses by.
    pub fn in_effect<R>(&self, block: impl FnOnce() -> R) -> R {
        MODULES.with(|m| m.borrow_mut().push(self.serializers_module.clone()));
        struct Pop;
        impl Drop for Pop {
            fn drop(&mut self) {
                MODULES.with(|m| m.borrow_mut().pop());
            }
        }
        let _pop = Pop;
        block()
    }
}

/// A configuration made by `builder_action` from `from`'s (savedstate's `SavedStateConfiguration { serializersModule
/// = … }`).
///
/// Kotlin: `SavedStateConfiguration(from: SavedStateConfiguration = SavedStateConfiguration.DEFAULT, builderAction: SavedStateConfiguration.Builder.() -> Unit)`
#[kompoz_runtime::named(type)]
pub fn SavedStateConfiguration(
    #[default(SavedStateConfiguration::default())] from: SavedStateConfiguration,
    builder_action: impl FnOnce(&mut SavedStateConfigurationBuilder),
) -> SavedStateConfiguration {
    let mut b = SavedStateConfigurationBuilder { serializers_module: from.serializers_module.clone() };
    builder_action(&mut b);
    SavedStateConfiguration { serializers_module: b.serializers_module }
}

/// What `SavedStateConfiguration { … }`'s block sets (savedstate's `SavedStateConfiguration.Builder`).
///
/// Kotlin: `SavedStateConfiguration.Builder`
pub struct SavedStateConfigurationBuilder {
    serializers_module: SerializersModule,
}

impl SavedStateConfigurationBuilder {
    /// The module of the configuration made (Kotlin's `serializersModule`).
    pub fn serializers_module(&self) -> SerializersModule {
        self.serializers_module.clone()
    }

    /// Kotlin's `serializersModule = module`.
    pub fn set_serializers_module(&mut self, module: SerializersModule) {
        self.serializers_module = module;
    }
}

thread_local! {
    /// The modules of the configurations in effect, innermost last.
    static MODULES: RefCell<Vec<SerializersModule>> = const { RefCell::new(Vec::new()) };
}

/// What reads back a value of the subclass of `B` named `name` (Rust's name of its type), from the configurations in
/// effect (the innermost first): what a polymorphic value's reader asks for a class it doesn't know otherwise.
pub fn polymorphic_restorer<B: ?Sized + 'static>(name: &str) -> Option<Rc<dyn Fn(&SavedValue) -> Option<Rc<B>>>> {
    MODULES.with(|m| m.borrow().iter().rev().find_map(|module| module.restorer::<B>(name)))
}

/// `serializer` with `configuration` in effect while it saves and restores (what a configured saver is).
pub fn configured_saver<T: 'static>(serializer: Saver<T>, configuration: &SavedStateConfiguration) -> Saver<T> {
    let (s, r) = (serializer.clone(), serializer);
    let (c1, c2) = (configuration.clone(), configuration.clone());
    Saver::new(move |v: &T| c1.in_effect(|| s.save(v)), move |v: &SavedValue| c2.in_effect(|| r.restore(v)))
}

/// `remember { init() }` saved with the composition's state by `serializer` (savedstate-compose's
/// `rememberSerializable(serializer) { … }`).
///
/// Kotlin: `rememberSerializable(serializer: KSerializer<T>, init: () -> T)`
#[track_caller]
pub fn remember_serializable<T: Clone + 'static>(serializer: Saver<T>, init: impl FnOnce() -> T) -> T {
    kompoz_runtime::remember_saveable_with(&serializer, init)
}

/// [`remember_serializable`] with `configuration` in effect while it saves and restores (savedstate-compose's
/// `rememberSerializable(serializer, configuration) { … }`).
///
/// Kotlin: `rememberSerializable(serializer: KSerializer<T>, configuration: SavedStateConfiguration, init: () -> T)`
#[track_caller]
pub fn remember_serializable_configured<T: Clone + 'static>(
    serializer: Saver<T>,
    configuration: SavedStateConfiguration,
    init: impl FnOnce() -> T,
) -> T {
    kompoz_runtime::remember_saveable_with(&configured_saver(serializer, &configuration), init)
}

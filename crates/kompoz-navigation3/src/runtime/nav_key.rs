//! NavKey.kt, and what Kotlin's `T : Any` keys are here.

use kompoz_navigationevent::DynEq;
use kompoz_runtime::{Saveable, SavedValue};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// A key of a back stack's destination (Compose's marker interface `NavKey`): a data holder compared structurally,
/// as Kotlin's `data class` and `object` keys are (`PartialEq`), written out with the back stack (Kotlin: keys "must
/// be marked with the `@Serializable` annotation in order to be saved"; here a key is [`Saveable`], a compile-time
/// rule: `#[derive(Saveable)]`). A type says it is one: `impl NavKey for Profile {}`.
///
/// Kotlin's type `NavKey` (a back stack of keys of any type, `NavBackStack<NavKey>`) is `Rc<dyn NavKey>`: what
/// `rememberNavBackStack(Profile)` holds, what `entry<Profile> { … }` finds its `Profile` in.
///
/// ```
/// # use kompoz_navigation3::runtime::NavKey;
/// # use kompoz_runtime::{Saveable, SavedValue};
/// #[derive(Clone, PartialEq, Debug)]
/// struct Profile;
/// impl NavKey for Profile {}
/// # impl Saveable for Profile {
/// #     fn save(&self) -> SavedValue { SavedValue::map::<&str>([]) }
/// #     fn restore(_: &SavedValue) -> Option<Self> { Some(Profile) }
/// # }
/// ```
pub trait NavKey: DynEq + SaveNavKey + fmt::Debug + 'static {}

/// What saves a [`NavKey`] of any type (kotlinx's polymorphic serializer of `NavKey`, NavKeySerializer.android.kt: its
/// class's name and its value): every [`Saveable`] key has it.
#[doc(hidden)]
pub trait SaveNavKey {
    /// Its class's name (Kotlin's `value::class.java.name`).
    fn nav_key_class_name(&self) -> &'static str;
    /// Its value written out.
    fn save_nav_key(&self) -> SavedValue;
    /// What reads its class's keys back.
    fn nav_key_restorer(&self) -> fn(&SavedValue) -> Option<Rc<dyn NavKey>>;
}

fn restore_nav_key<K: NavKey + Saveable>(saved: &SavedValue) -> Option<Rc<dyn NavKey>> {
    K::restore(saved).map(|k| Rc::new(k) as Rc<dyn NavKey>)
}

impl<K: NavKey + Saveable> SaveNavKey for K {
    fn nav_key_class_name(&self) -> &'static str {
        std::any::type_name::<K>()
    }
    fn save_nav_key(&self) -> SavedValue {
        self.save()
    }
    fn nav_key_restorer(&self) -> fn(&SavedValue) -> Option<Rc<dyn NavKey>> {
        restore_nav_key::<K>
    }
}

impl dyn NavKey {
    /// Whether it is a `K` (Kotlin's `key is K`).
    pub fn is<K: 'static>(&self) -> bool {
        self.as_any().is::<K>()
    }

    /// It as a `K`, if it is one (Kotlin's `key as? K`).
    pub fn downcast_ref<K: 'static>(&self) -> Option<&K> {
        self.as_any().downcast_ref::<K>()
    }
}

impl PartialEq for dyn NavKey {
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other.as_any())
    }
}

thread_local! {
    /// The classes of the keys seen (their readers by name): what restores a back stack of keys of any type
    /// (Android's reflection, `Class.forName(className)`, in NavKeySerializer.android.kt). A class is known once a
    /// key of it was put in a back stack, or registered ([`register_nav_key`]).
    static CLASSES: RefCell<HashMap<&'static str, fn(&SavedValue) -> Option<Rc<dyn NavKey>>>> =
        RefCell::new(HashMap::new());
}

/// Makes `K`'s keys restorable by their class's name before any is seen in this process (what Android's reflection
/// finds by itself; kotlinx's `polymorphic(NavKey::class) { subclass(K::class) }`).
pub fn register_nav_key<K: NavKey + Saveable>() {
    CLASSES.with(|c| c.borrow_mut().insert(std::any::type_name::<K>(), restore_nav_key::<K> as _));
}

pub(crate) fn note_nav_key(key: &dyn NavKey) {
    let (name, restore) = (key.nav_key_class_name(), key.nav_key_restorer());
    CLASSES.with(|c| {
        c.borrow_mut().entry(name).or_insert(restore);
    });
}

/// A key of any type written out (NavKeySerializer's structure: its class's name, then its value).
pub(crate) fn save_any_nav_key(key: &Rc<dyn NavKey>) -> SavedValue {
    note_nav_key(&**key);
    SavedValue::map([("type", SavedValue::from(key.nav_key_class_name())), ("value", key.save_nav_key())])
}

/// A key of any type read back: `None` if its class isn't known (seen in this process, registered, or a subclass of
/// `NavKey` in the serializers module of a configuration in effect: kotlinx's open polymorphism) or its value isn't
/// one of it.
pub(crate) fn restore_any_nav_key(saved: &SavedValue) -> Option<Rc<dyn NavKey>> {
    let name = saved.get("type").as_str()?;
    if let Some(restore) = kompoz_lifecycle::polymorphic_restorer::<dyn NavKey>(name) {
        return restore(saved.get("value"));
    }
    let restore = CLASSES.with(|c| c.borrow().get(name).copied())?;
    restore(saved.get("value"))
}

/// A key is a `NavKey`'s value (kotlinx's `polymorphic(NavKey::class) { subclass(Home::class) }`).
impl<K: NavKey + Saveable> kompoz_lifecycle::PolymorphicBase<K> for dyn NavKey {
    fn upcast(value: K) -> Rc<dyn NavKey> {
        Rc::new(value)
    }
}

/// What a back stack's keys can be (Kotlin's `T : Any` of `NavEntry<T>`, `NavDisplay<T>`, …): compared by `==`,
/// written by `Debug` (Kotlin's `toString`), each of a class (Kotlin's `key::class`: what `entry<K> { }` finds it by).
/// [`NavKey`] types are, and `Rc<dyn NavKey>` (its class the key's), texts and numbers.
pub trait NavEntryKey: Clone + PartialEq + fmt::Debug + 'static {
    /// The key as a value of its class (an `Rc<dyn NavKey>`'s key itself).
    fn key_value(&self) -> &dyn Any;
    /// Its class's name (Kotlin's `key::class`'s).
    fn key_class_name(&self) -> &'static str;
    /// Kotlin's `toString()` of it.
    fn key_to_string(&self) -> String {
        format!("{self:?}")
    }
}

impl<K: NavKey + Clone + PartialEq> NavEntryKey for K {
    fn key_value(&self) -> &dyn Any {
        self
    }
    fn key_class_name(&self) -> &'static str {
        std::any::type_name::<K>()
    }
}

impl NavEntryKey for Rc<dyn NavKey> {
    fn key_value(&self) -> &dyn Any {
        (**self).as_any()
    }
    fn key_class_name(&self) -> &'static str {
        self.nav_key_class_name()
    }
}

macro_rules! plain_keys {
    ($($t:ty => $name:literal),*) => {$(
        impl NavEntryKey for $t {
            fn key_value(&self) -> &dyn Any {
                self
            }
            fn key_class_name(&self) -> &'static str {
                $name
            }
            fn key_to_string(&self) -> String {
                self.to_string()
            }
        }
    )*};
}
plain_keys!(
    String => "kotlin.String", &'static str => "kotlin.String", i32 => "kotlin.Int", i64 => "kotlin.Long",
    u32 => "kotlin.UInt", u64 => "kotlin.ULong", usize => "kotlin.Long", char => "kotlin.Char", bool => "kotlin.Boolean"
);

/// Kotlin's `defaultContentKey(key)` (`"$key:${key::class}"`): what tells an entry's content apart by default.
pub fn default_content_key<T: NavEntryKey>(key: &T) -> kompoz_ui::layout::SlotKey {
    kompoz_ui::layout::SlotKey::new(format!("{}:class {}", key.key_to_string(), key.key_class_name()))
}

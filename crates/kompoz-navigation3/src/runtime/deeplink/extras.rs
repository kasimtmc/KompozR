//! RequestExtras.kt: [`RequestExtras`], its keys ([`RequestExtrasKey`]) and the [`request_extras`] DSL.

use std::any::Any;
use std::fmt;
use std::rc::Rc;

/// A key of [`RequestExtras`], of values of type [`Value`](Self::Value) (Compose's `RequestExtrasKey<T>`): typically
/// a unit struct (Kotlin's `object MyKey : RequestExtrasKey<String>`). Keys are told apart by their type and `==`.
pub trait RequestExtrasKey: PartialEq + fmt::Debug + 'static {
    /// Kotlin's `T`.
    type Value: PartialEq + fmt::Debug + 'static;
}

/// A key of any type, as the map keeps it.
#[doc(hidden)]
pub trait AnyKey {
    fn as_any(&self) -> &dyn Any;
    fn same(&self, other: &dyn AnyKey) -> bool;
    fn write(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

impl<K: RequestExtrasKey> AnyKey for K {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn same(&self, other: &dyn AnyKey) -> bool {
        other.as_any().downcast_ref::<K>().is_some_and(|o| o == self)
    }
    fn write(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A value of any type, as the map keeps it.
#[doc(hidden)]
pub trait AnyValue {
    fn as_any(&self) -> &dyn Any;
    fn same(&self, other: &dyn AnyValue) -> bool;
    fn write(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

impl<V: PartialEq + fmt::Debug + 'static> AnyValue for V {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn same(&self, other: &dyn AnyValue) -> bool {
        other.as_any().downcast_ref::<V>().is_some_and(|o| o == self)
    }
    fn write(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // (a text as Kotlin writes it, any other value by its Debug)
        match (self as &dyn Any).downcast_ref::<String>() {
            Some(s) => f.write_str(s),
            None => write!(f, "{self:?}"),
        }
    }
}

type Entry = (Rc<dyn AnyKey>, Rc<dyn AnyValue>);

/// Builds [`RequestExtras`] with [`put`](Self::put): the scope of [`request_extras`] (Compose's
/// `RequestExtrasScope`).
#[derive(Default)]
pub struct RequestExtrasScope {
    map: Vec<Entry>,
}

fn put_entry(map: &mut Vec<Entry>, entry: Entry) {
    match map.iter().position(|(k, _)| k.same(&*entry.0)) {
        Some(i) => map[i].1 = entry.1,
        None => map.push(entry),
    }
}

impl RequestExtrasScope {
    /// Stores `value` with `key`.
    /// Kotlin: `put(key: RequestExtrasKey<T>, value: T)`
    pub fn put<K: RequestExtrasKey>(&mut self, key: K, value: K::Value) {
        put_entry(&mut self.map, (Rc::new(key), Rc::new(value)));
    }

    fn build(self) -> RequestExtras {
        RequestExtras { map: self.map }
    }
}

/// [`RequestExtras`] made by `builder` (Compose's `requestExtras { put(Key, value) }`).
/// Kotlin: `requestExtras(builder: RequestExtrasScope.() -> Unit): RequestExtras`
pub fn request_extras(builder: impl FnOnce(&mut RequestExtrasScope)) -> RequestExtras {
    let mut scope = RequestExtrasScope::default();
    builder(&mut scope);
    scope.build()
}

/// [`request_extras`]'s suspending twin (an inline function's lambda: see `kompoz_runtime::kt`).
#[doc(hidden)]
pub async fn request_extras_async(builder: impl AsyncFnOnce(&mut RequestExtrasScope)) -> RequestExtras {
    let mut scope = RequestExtrasScope::default();
    builder(&mut scope).await;
    scope.build()
}

/// An empty [`RequestExtras`].
/// Kotlin: `emptyRequestExtras(): RequestExtras`
pub fn empty_request_extras() -> RequestExtras {
    RequestExtras::default()
}

/// A map of [`RequestExtrasKey`]s to their values, a [`DeepLinkRequest`](super::DeepLinkRequest)'s extras (Compose's
/// `RequestExtras`): made by [`request_extras`] or [`empty_request_extras`]. Its entries keep the order they were
/// put in (Kotlin's `LinkedHashMap`); two are equal with the same entries in any order. Its text writes each key by
/// its `Debug` (Kotlin's objects' `toString()` is their class's name and a hash).
#[derive(Clone, Default)]
pub struct RequestExtras {
    map: Vec<Entry>,
}

/// Its entries, a map's (Kotlin's `isEmpty()` and `isNotEmpty()` through `KtMapExt`, as a `Map`'s).
impl kompoz_runtime::kt::KtMapLike for RequestExtras {
    type K = Rc<dyn AnyKey>;
    type V = Rc<dyn AnyValue>;
    fn kt_entries(&self) -> Vec<(Rc<dyn AnyKey>, Rc<dyn AnyValue>)> {
        self.map.clone()
    }
}

impl RequestExtras {
    pub(crate) fn map_is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The number of entries.
    /// Kotlin: `size: Int`
    pub fn size(&self) -> i32 {
        self.map.len() as i32
    }

    /// `key`'s value, if it has one (Kotlin's `extras[key]`).
    /// Kotlin: `get(key: RequestExtrasKey<T>): T?`
    pub fn get<K: RequestExtrasKey>(&self, key: &K) -> Option<&K::Value> {
        self.map
            .iter()
            .find(|(k, _)| k.as_any().downcast_ref::<K>() == Some(key))
            .and_then(|(_, v)| v.as_any().downcast_ref::<K::Value>())
    }

    /// Whether `key` has a value (Kotlin's `key in extras`).
    /// Kotlin: `contains(key: RequestExtrasKey<*>): Boolean`
    pub fn contains<K: RequestExtrasKey>(&self, key: &K) -> bool {
        self.map.iter().any(|(k, _)| k.as_any().downcast_ref::<K>() == Some(key))
    }

    /// Its entries and `other`'s, `other`'s values taking the place of its own for the same keys (Kotlin's `+`).
    /// Kotlin: `plus(other: RequestExtras): RequestExtras`
    pub fn plus(&self, other: &RequestExtras) -> RequestExtras {
        let mut map = self.map.clone();
        for entry in &other.map {
            put_entry(&mut map, entry.clone());
        }
        RequestExtras { map }
    }

    /// Its entries but those of `other`'s keys (Kotlin's `-`).
    /// Kotlin: `minus(other: RequestExtras): RequestExtras`
    pub fn minus(&self, other: &RequestExtras) -> RequestExtras {
        let map = self.map.iter().filter(|(k, _)| !other.map.iter().any(|(o, _)| o.same(&**k))).cloned().collect();
        RequestExtras { map }
    }
}

impl std::ops::Add for &RequestExtras {
    type Output = RequestExtras;
    fn add(self, other: &RequestExtras) -> RequestExtras {
        self.plus(other)
    }
}

impl std::ops::Sub for &RequestExtras {
    type Output = RequestExtras;
    fn sub(self, other: &RequestExtras) -> RequestExtras {
        self.minus(other)
    }
}

impl PartialEq for RequestExtras {
    fn eq(&self, other: &Self) -> bool {
        self.map.len() == other.map.len()
            && self.map.iter().all(|(k, v)| other.map.iter().any(|(ok, ov)| ok.same(&**k) && ov.same(&**v)))
    }
}

/// `RequestExtras({key=value, …})` (Kotlin's `toString()`).
impl fmt::Display for RequestExtras {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RequestExtras({")?;
        for (i, (k, v)) in self.map.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            k.write(f)?;
            f.write_str("=")?;
            v.write(f)?;
        }
        f.write_str("})")
    }
}

impl fmt::Debug for RequestExtras {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

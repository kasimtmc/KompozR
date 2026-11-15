//! Semantics property keys and the configuration they are set into (Compose's SemanticsPropertyKey and
//! SemanticsConfiguration).

use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

/// A semantics property (Compose's SemanticsPropertyKey): its name (what it is known by), how a merging parent
/// takes a child's value (`merge_policy(parent's, child's)`; none: the parent's left as it is), whether it tells
/// accessibility something, and for an action, how two values for one node are made one (the label and the
/// action of one, what it lacks taken from the other: Compose's AccessibilityAction rule).
///
/// A value is `Clone + PartialEq + Debug` (closures are wrapped in types comparing by identity).
pub struct SemanticsPropertyKey<T> {
    name: &'static str,
    merge_policy: fn(Option<T>, T) -> Option<T>,
    /// Whether it tells accessibility something (Compose's `isImportantForAccessibility`).
    important_for_accessibility: bool,
    /// An action's: two values for one configuration made one (`combine(first, second)`: the first's label and
    /// action, what it lacks from the second).
    combine: Option<fn(&T, &T) -> T>,
    _t: PhantomData<fn() -> T>,
}

impl<T> Clone for SemanticsPropertyKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for SemanticsPropertyKey<T> {}

/// The default merge policy: the parent's value if it has one, else the child's.
pub(crate) fn keep_parent<T>(parent: Option<T>, child: T) -> Option<T> {
    Some(parent.unwrap_or(child))
}

/// A merge policy keeping the parent's value only (a test tag, a role, a traversal index).
pub(crate) fn keep_parent_only<T>(parent: Option<T>, _child: T) -> Option<T> {
    parent
}

impl<T: Clone + PartialEq + fmt::Debug + 'static> SemanticsPropertyKey<T> {
    /// A key telling accessibility nothing, merged by its default policy: the parent's value if it has one, else
    /// the child's (Compose's public constructor).
    pub const fn new(name: &'static str) -> SemanticsPropertyKey<T> {
        SemanticsPropertyKey {
            name,
            merge_policy: keep_parent::<T>,
            important_for_accessibility: false,
            combine: None,
            _t: PhantomData,
        }
    }

    /// A key telling accessibility nothing, merged by `merge_policy`.
    pub const fn with_merge_policy(name: &'static str, merge_policy: fn(Option<T>, T) -> Option<T>) -> Self {
        SemanticsPropertyKey { name, merge_policy, important_for_accessibility: false, combine: None, _t: PhantomData }
    }

    /// A key that tells accessibility something (Compose's `AccessibilityKey`).
    pub const fn accessibility(name: &'static str) -> Self {
        SemanticsPropertyKey {
            name,
            merge_policy: keep_parent::<T>,
            important_for_accessibility: true,
            combine: None,
            _t: PhantomData,
        }
    }

    /// A key that tells accessibility something, merged by `merge_policy`.
    pub const fn accessibility_with_merge_policy(
        name: &'static str,
        merge_policy: fn(Option<T>, T) -> Option<T>,
    ) -> Self {
        SemanticsPropertyKey { name, merge_policy, important_for_accessibility: true, combine: None, _t: PhantomData }
    }

    /// The same key, telling accessibility nothing.
    pub const fn not_important_for_accessibility(self) -> Self {
        SemanticsPropertyKey { important_for_accessibility: false, ..self }
    }

    /// The same key, two values for one configuration made one by `combine(first, second)`.
    pub(crate) const fn combining(self, combine: fn(&T, &T) -> T) -> Self {
        SemanticsPropertyKey { combine: Some(combine), ..self }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Whether it tells accessibility something.
    pub fn is_important_for_accessibility(&self) -> bool {
        self.important_for_accessibility
    }

    /// The value a merging parent ends up with (Compose's `merge`).
    pub fn merge(&self, parent_value: Option<T>, child_value: T) -> Option<T> {
        (self.merge_policy)(parent_value, child_value)
    }
}

impl<T> fmt::Debug for SemanticsPropertyKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AccessibilityKey: {}", self.name)
    }
}

/// What a configuration knows of a key without its type: merging, combining, comparing and printing its values.
trait ErasedKey {
    fn name(&self) -> &'static str;
    fn important(&self) -> bool;
    fn merge(&self, parent: Option<&Rc<dyn Any>>, child: &Rc<dyn Any>) -> Option<Rc<dyn Any>>;
    fn combine(&self, first: &Rc<dyn Any>, second: &Rc<dyn Any>) -> Option<Rc<dyn Any>>;
    fn eq(&self, a: &dyn Any, b: &dyn Any) -> bool;
    fn debug(&self, value: &dyn Any, f: &mut fmt::Formatter) -> fmt::Result;
}

impl<T: Clone + PartialEq + fmt::Debug + 'static> ErasedKey for SemanticsPropertyKey<T> {
    fn name(&self) -> &'static str {
        self.name
    }
    fn important(&self) -> bool {
        self.important_for_accessibility
    }
    fn merge(&self, parent: Option<&Rc<dyn Any>>, child: &Rc<dyn Any>) -> Option<Rc<dyn Any>> {
        let parent = parent.and_then(|p| p.downcast_ref::<T>().cloned());
        let child = child.downcast_ref::<T>()?.clone();
        (self.merge_policy)(parent, child).map(|v| Rc::new(v) as Rc<dyn Any>)
    }
    fn combine(&self, first: &Rc<dyn Any>, second: &Rc<dyn Any>) -> Option<Rc<dyn Any>> {
        let combine = self.combine?;
        Some(Rc::new(combine(first.downcast_ref::<T>()?, second.downcast_ref::<T>()?)))
    }
    fn eq(&self, a: &dyn Any, b: &dyn Any) -> bool {
        matches!((a.downcast_ref::<T>(), b.downcast_ref::<T>()), (Some(a), Some(b)) if a == b)
    }
    /// As Kotlin prints it: texts bare, lists of texts as `[a, b]`, the rest as `Debug` does.
    fn debug(&self, value: &dyn Any, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(s) = value.downcast_ref::<String>() {
            return f.write_str(s);
        }
        if let Some(v) = value.downcast_ref::<Vec<String>>() {
            return write!(f, "[{}]", v.join(", "));
        }
        match value.downcast_ref::<T>() {
            Some(v) => write!(f, "{v:?}"),
            None => write!(f, "?"),
        }
    }
}

/// A property set: its key (erased) and its value.
#[derive(Clone)]
struct Entry {
    key: Rc<dyn ErasedKey>,
    value: Rc<dyn Any>,
}

/// The semantics properties of a layout, and whether it merges its descendants' or clears them (Compose's
/// SemanticsConfiguration). Semantics modifier nodes set into it (it is Compose's SemanticsPropertyReceiver: the
/// setters of `SemanticsProperties` and `SemanticsActions` are its methods). Properties keep the order they were
/// first set in.
#[derive(Clone, Default)]
pub struct SemanticsConfiguration {
    entries: Vec<Entry>,
    /// Its descendants' semantics are its own (a button's label).
    pub is_merging_semantics_of_descendants: bool,
    /// Its descendants' semantics are dropped.
    pub is_clearing_semantics: bool,
}

impl SemanticsConfiguration {
    pub fn new() -> SemanticsConfiguration {
        SemanticsConfiguration::default()
    }

    /// Sets `key` to `value` (Compose's `set`); an action set again takes what the new one lacks (its label or
    /// its action) from the one it replaces.
    pub fn set<T: Clone + PartialEq + fmt::Debug + 'static>(&mut self, key: SemanticsPropertyKey<T>, value: T) {
        let value: Rc<dyn Any> = Rc::new(value);
        match self.entries.iter_mut().find(|e| e.key.name() == key.name) {
            Some(e) => {
                e.value = key.combine(&value, &e.value).unwrap_or(value);
                e.key = Rc::new(key);
            }
            None => self.entries.push(Entry { key: Rc::new(key), value }),
        }
    }

    /// `key`'s value, if it is set (Compose's `getOrNull`).
    pub fn get<T: Clone + 'static>(&self, key: SemanticsPropertyKey<T>) -> Option<T> {
        self.entries.iter().find(|e| e.key.name() == key.name).and_then(|e| e.value.downcast_ref::<T>().cloned())
    }

    /// `key`'s value; panics if it isn't set, as Compose's `get` throws.
    #[track_caller]
    pub fn require<T: Clone + 'static>(&self, key: SemanticsPropertyKey<T>) -> T {
        match self.get(key) {
            Some(v) => v,
            None => panic!("Key not present: {key:?} - consider getOrElse or getOrNull"),
        }
    }

    pub fn get_or_else<T: Clone + 'static>(&self, key: SemanticsPropertyKey<T>, default: impl FnOnce() -> T) -> T {
        self.get(key).unwrap_or_else(default)
    }

    pub fn contains<T>(&self, key: SemanticsPropertyKey<T>) -> bool {
        self.entries.iter().any(|e| e.key.name() == key.name)
    }

    /// Whether the property named `name` is set.
    pub fn contains_name(&self, name: &str) -> bool {
        self.entries.iter().any(|e| e.key.name() == name)
    }

    /// The names of the properties set, in the order they were first set.
    pub fn names(&self) -> Vec<&'static str> {
        self.entries.iter().map(|e| e.key.name()).collect()
    }

    /// How many properties are set.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether a property tells accessibility something (Compose's `containsImportantForAccessibility`).
    pub fn contains_important_for_accessibility(&self) -> bool {
        self.entries.iter().any(|e| e.key.important())
    }

    /// A child's properties taken in by a merging parent (Compose's `mergeChild`).
    pub fn merge_child(&mut self, child: &SemanticsConfiguration) {
        for c in &child.entries {
            let existing = self.entries.iter().position(|e| e.key.name() == c.key.name());
            let parent_value = existing.map(|i| self.entries[i].value.clone());
            if let Some(merged) = c.key.merge(parent_value.as_ref(), &c.value) {
                match existing {
                    Some(i) => self.entries[i].value = merged,
                    None => self.entries.push(Entry { key: c.key.clone(), value: merged }),
                }
            }
        }
    }

    /// Another node's properties taken in as if set here (Compose's `collapsePeer`): those not set here yet, and
    /// for an action set on both, what this one lacks.
    pub fn collapse_peer(&mut self, peer: &SemanticsConfiguration) {
        if peer.is_merging_semantics_of_descendants {
            self.is_merging_semantics_of_descendants = true;
        }
        if peer.is_clearing_semantics {
            self.is_clearing_semantics = true;
        }
        for e in &peer.entries {
            match self.entries.iter_mut().find(|x| x.key.name() == e.key.name()) {
                None => self.entries.push(e.clone()),
                Some(x) => {
                    if let Some(v) = x.key.combine(&x.value, &e.value) {
                        x.value = v;
                    }
                }
            }
        }
    }

    /// A copy (Compose's `copy`).
    pub fn copy(&self) -> SemanticsConfiguration {
        self.clone()
    }

    /// Visits each property, its name and its value printed.
    pub fn for_each_printed(&self, mut f: impl FnMut(&'static str, String)) {
        for e in &self.entries {
            f(e.key.name(), format!("{:?}", Printed(e)));
        }
    }
}

/// A property's value as `Debug` prints it.
struct Printed<'a>(&'a Entry);

impl fmt::Debug for Printed<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.key.debug(&*self.0.value, f)
    }
}

impl PartialEq for SemanticsConfiguration {
    /// The same properties set to equal values (in any order), merging and clearing alike (Compose's `equals`).
    fn eq(&self, other: &Self) -> bool {
        self.is_merging_semantics_of_descendants == other.is_merging_semantics_of_descendants
            && self.is_clearing_semantics == other.is_clearing_semantics
            && self.entries.len() == other.entries.len()
            && self.entries.iter().all(|e| {
                other
                    .entries
                    .iter()
                    .find(|o| o.key.name() == e.key.name())
                    .is_some_and(|o| e.key.eq(&*e.value, &*o.value))
            })
    }
}

impl fmt::Debug for SemanticsConfiguration {
    /// As Compose's `toString`: `mergeDescendants=true, isClearingSemantics=true, Name : value, …`.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut sep = "";
        if self.is_merging_semantics_of_descendants {
            write!(f, "mergeDescendants=true")?;
            sep = ", ";
        }
        if self.is_clearing_semantics {
            write!(f, "{sep}isClearingSemantics=true")?;
            sep = ", ";
        }
        for e in &self.entries {
            write!(f, "{sep}{} : {:?}", e.key.name(), Printed(e))?;
            sep = ", ";
        }
        Ok(())
    }
}

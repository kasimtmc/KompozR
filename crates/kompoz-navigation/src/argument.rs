//! A destination's arguments: their types, whether they may be null, their default values (Compose's
//! `NavArgument`, its builder, `NamedNavArgument`, `navArgument(name) { … }`).

use crate::nav_type::NavType;
use crate::saved_state::{SavedState, contains, is_null};
use std::any::Any;
use std::fmt;
use std::rc::Rc;

/// An argument of a destination (Compose's `NavArgument`): its [`NavType`], whether it may be null, its default
/// value. Made with [`NavArgumentBuilder`] (or [`nav_argument`]).
#[derive(Clone)]
pub struct NavArgument {
    type_: NavType,
    is_nullable: bool,
    default_value: Option<Rc<dyn Any>>,
    is_default_value_present: bool,
    is_default_value_unknown: bool,
}

impl NavArgument {
    fn new(
        type_: NavType,
        is_nullable: bool,
        default_value: Option<Rc<dyn Any>>,
        default_value_present: bool,
        unknown_default_value_present: bool,
    ) -> NavArgument {
        assert!(type_.is_nullable_allowed() || !is_nullable, "{} does not allow nullable values", type_.name());
        assert!(
            is_nullable || !default_value_present || default_value.is_some(),
            "Argument with type {} has null value but is not nullable.",
            type_.name()
        );
        NavArgument {
            type_,
            is_nullable,
            default_value,
            is_default_value_present: default_value_present || unknown_default_value_present,
            is_default_value_unknown: unknown_default_value_present,
        }
    }

    /// Its type (Kotlin's `type`).
    pub fn r#type(&self) -> NavType {
        self.type_.clone()
    }

    /// Whether it may be null (Kotlin's `isNullable`).
    pub fn is_nullable(&self) -> bool {
        self.is_nullable
    }

    /// Whether it has a default value (Kotlin's `isDefaultValuePresent`; a typed route's argument with a default
    /// has one whose value isn't known here).
    pub fn is_default_value_present(&self) -> bool {
        self.is_default_value_present
    }

    pub(crate) fn is_default_value_unknown(&self) -> bool {
        self.is_default_value_unknown
    }

    /// Its default value (Kotlin's `defaultValue`), of its type's value type; `None` for none or null.
    pub fn default_value(&self) -> Option<Rc<dyn Any>> {
        self.default_value.clone()
    }

    /// Puts its default value under `name` in `bundle`, if it has one known (Kotlin's `putDefaultValue`).
    pub fn put_default_value(&self, name: &str, bundle: &SavedState) {
        if self.is_default_value_present
            && let Some(v) = &self.default_value
        {
            self.type_.erased().put_any(bundle, name, Some(&**v));
        }
    }

    /// Whether `bundle`'s value of `name` fits it: not null unless it may be, of its type (Kotlin's `verify`).
    pub fn verify(&self, name: &str, bundle: &SavedState) -> bool {
        if !self.is_nullable && contains(bundle, name) && is_null(bundle, name) {
            return false;
        }
        // (Kotlin's IllegalStateException of a value of another type: a value there that its type can't read)
        !(contains(bundle, name) && !is_null(bundle, name) && self.type_.erased().get_any(bundle, name).is_none())
    }
}

impl PartialEq for NavArgument {
    fn eq(&self, other: &Self) -> bool {
        if self.is_nullable != other.is_nullable
            || self.is_default_value_present != other.is_default_value_present
            || self.type_ != other.type_
        {
            return false;
        }
        match (&self.default_value, &other.default_value) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                (**a).type_id() == (**b).type_id() && self.type_.erased().value_equals_any(Some(&**a), Some(&**b))
            }
            _ => false,
        }
    }
}

impl fmt::Debug for NavArgument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NavArgument Type: {} Nullable: {}", self.type_, self.is_nullable)?;
        if self.is_default_value_present {
            match &self.default_value {
                Some(v) => write!(f, " DefaultValue: {}", self.type_.erased().serialize_any(Some(&**v)))?,
                None => write!(f, " DefaultValue: null")?,
            }
        }
        Ok(())
    }
}

/// Kotlin's `NavArgument.Builder`.
#[derive(Clone, Default)]
pub struct NavArgumentBuilderBase {
    type_: Option<NavType>,
    is_nullable: bool,
    default_value: Option<Rc<dyn Any>>,
    default_value_present: bool,
    unknown_default_value_present: bool,
}

impl NavArgumentBuilderBase {
    pub fn new() -> Self {
        Self::default()
    }

    /// Kotlin's `setType`.
    pub fn set_type(&mut self, type_: NavType) -> &mut Self {
        self.type_ = Some(type_);
        self
    }

    /// Kotlin's `setIsNullable`.
    pub fn set_is_nullable(&mut self, is_nullable: bool) -> &mut Self {
        self.is_nullable = is_nullable;
        self
    }

    /// Kotlin's `setDefaultValue(value)`: a value of the type's value type.
    pub fn set_default_value<T: 'static>(&mut self, value: T) -> &mut Self {
        self.default_value = Some(Rc::new(value));
        self.default_value_present = true;
        self
    }

    /// Kotlin's `setDefaultValue(null)`.
    pub fn set_default_value_null(&mut self) -> &mut Self {
        self.default_value = None;
        self.default_value_present = true;
        self
    }

    pub(crate) fn set_unknown_default_value_present(&mut self, present: bool) -> &mut Self {
        self.unknown_default_value_present = present;
        self
    }

    /// The argument: of the type set, else of its default value's (Kotlin's `build`).
    ///
    /// # Panics
    /// A type that doesn't allow null for a nullable argument, a null default of an argument that isn't nullable,
    /// a default value of a type no `NavType` is inferred for.
    pub fn build(&self) -> NavArgument {
        let type_ = self.type_.clone().unwrap_or_else(|| NavType::infer_from_value_type(self.default_value.as_deref()));
        NavArgument::new(
            type_,
            self.is_nullable,
            self.default_value.clone(),
            self.default_value_present,
            self.unknown_default_value_present,
        )
    }
}

/// The builder `nav_argument`'s block configures (Compose's `NavArgumentBuilder`): `type`, `nullable`,
/// `defaultValue`.
#[derive(Clone, Default)]
pub struct NavArgumentBuilder {
    builder: NavArgumentBuilderBase,
    type_: Option<NavType>,
    nullable: bool,
    default_value: Option<Rc<dyn Any>>,
}

impl NavArgumentBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Its type (Kotlin's `type`).
    ///
    /// # Panics
    /// If none was set (Kotlin's IllegalStateException).
    pub fn r#type(&self) -> NavType {
        self.type_.clone().expect("NavType has not been set on this builder.")
    }

    /// Kotlin's `type = …`.
    pub fn set_type(&mut self, value: NavType) {
        self.type_ = Some(value.clone());
        self.builder.set_type(value);
    }

    /// Kotlin's `nullable`.
    pub fn nullable(&self) -> bool {
        self.nullable
    }

    /// Kotlin's `nullable = …`.
    pub fn set_nullable(&mut self, value: bool) {
        self.nullable = value;
        self.builder.set_is_nullable(value);
    }

    /// Kotlin's `defaultValue`.
    pub fn default_value(&self) -> Option<Rc<dyn Any>> {
        self.default_value.clone()
    }

    /// Kotlin's `defaultValue = value` (of the type's value type).
    pub fn set_default_value<T: 'static>(&mut self, value: T) {
        let v: Rc<dyn Any> = Rc::new(value);
        self.default_value = Some(v.clone());
        self.builder.default_value = Some(v);
        self.builder.default_value_present = true;
    }

    /// Kotlin's `defaultValue = null`.
    pub fn set_default_value_null(&mut self) {
        self.default_value = None;
        self.builder.set_default_value_null();
    }

    pub(crate) fn set_unknown_default_value_present(&mut self, value: bool) {
        self.builder.set_unknown_default_value_present(value);
    }

    /// The argument (Kotlin's `build`).
    pub fn build(&self) -> NavArgument {
        self.builder.build()
    }
}

/// An argument with its name (Compose's `NamedNavArgument`; Kotlin's `val (name, argument) = …`).
#[derive(Clone, Debug, PartialEq)]
pub struct NamedNavArgument {
    name: String,
    argument: NavArgument,
}

impl NamedNavArgument {
    pub(crate) fn new(name: impl Into<String>, argument: NavArgument) -> NamedNavArgument {
        NamedNavArgument { name: name.into(), argument }
    }
    pub fn name(&self) -> String {
        self.name.clone()
    }
    pub fn argument(&self) -> NavArgument {
        self.argument.clone()
    }
    /// Kotlin's `component1()`.
    pub fn component1(&self) -> String {
        self.name.clone()
    }
    /// Kotlin's `component2()`.
    pub fn component2(&self) -> NavArgument {
        self.argument.clone()
    }
}

/// An argument `name` as `builder` configures it (Compose's `navArgument(name) { type = …; nullable = …;
/// defaultValue = … }`).
pub fn nav_argument(name: impl Into<String>, builder: impl FnOnce(&mut NavArgumentBuilder)) -> NamedNavArgument {
    let mut b = NavArgumentBuilder::new();
    builder(&mut b);
    NamedNavArgument::new(name, b.build())
}

/// A destination's arguments by name, in the order they were added (Kotlin's `Map<String, NavArgument>`, a
/// `LinkedHashMap`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NavArguments(Vec<(String, NavArgument)>);

impl NavArguments {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, name: &str) -> Option<&NavArgument> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, a)| a)
    }
    pub fn contains_key(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
    /// Sets `name`'s argument (in its place when it is there).
    pub fn insert(&mut self, name: impl Into<String>, argument: NavArgument) {
        let name = name.into();
        match self.0.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = argument,
            None => self.0.push((name, argument)),
        }
    }
    pub fn remove(&mut self, name: &str) -> Option<NavArgument> {
        let i = self.0.iter().position(|(n, _)| n == name)?;
        Some(self.0.remove(i).1)
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &NavArgument)> {
        self.0.iter().map(|(n, a)| (n, a))
    }
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.iter().map(|(n, _)| n)
    }
    /// Each argument's type by name (Kotlin's `arguments.mapValues { it.value.type }`).
    pub fn types(&self) -> Vec<(String, NavType)> {
        self.0.iter().map(|(n, a)| (n.clone(), a.r#type())).collect()
    }
}

impl<'a> IntoIterator for &'a NavArguments {
    type Item = (&'a String, &'a NavArgument);
    type IntoIter = Box<dyn Iterator<Item = (&'a String, &'a NavArgument)> + 'a>;
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

/// The names of the arguments that are required — not nullable, without a default — and that
/// `is_argument_missing` says are missing (Kotlin's `missingRequiredArguments`).
pub(crate) fn missing_required_arguments(
    arguments: &NavArguments,
    mut is_argument_missing: impl FnMut(&str) -> bool,
) -> Vec<String> {
    arguments
        .iter()
        .filter(|(_, a)| !a.is_nullable() && !a.is_default_value_present())
        .map(|(n, _)| n.clone())
        .filter(|n| is_argument_missing(n))
        .collect()
}

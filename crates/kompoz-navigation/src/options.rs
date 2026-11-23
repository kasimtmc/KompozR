//! How a navigation goes (Compose's `NavOptions`, `NavOptions.Builder`, `navOptions { … }`, `NavOptionsBuilder`,
//! `PopUpToBuilder`): popping the back stack down to a destination first (and it too, saving what is popped),
//! not stacking a second copy of the top destination, bringing back a saved stack.

use crate::destination::create_route;
use crate::java::string_hash;
use crate::serialization::{AnyRoute, Route, RouteClass, hash_code_of};
use std::fmt;
use std::rc::Rc;

/// Options of a navigation (Compose's `NavOptions`).
#[derive(Clone)]
pub struct NavOptions {
    single_top: bool,
    restore_state: bool,
    pop_up_to_id: i32,
    pop_up_to_route: Option<String>,
    pop_up_to_route_class: Option<RouteClass>,
    pop_up_to_route_object: Option<Rc<dyn AnyRoute>>,
    pop_up_to_inclusive: bool,
    pop_up_to_save_state: bool,
}

impl NavOptions {
    /// Whether the destination, already on top, isn't stacked again (Kotlin's `shouldLaunchSingleTop`).
    pub fn should_launch_single_top(&self) -> bool {
        self.single_top
    }
    /// Whether a stack saved for the destination comes back (Kotlin's `shouldRestoreState`).
    pub fn should_restore_state(&self) -> bool {
        self.restore_state
    }
    /// Whether the destination popped up to goes too (Kotlin's `isPopUpToInclusive`).
    pub fn is_pop_up_to_inclusive(&self) -> bool {
        self.pop_up_to_inclusive
    }
    /// Whether what is popped is saved (Kotlin's `shouldPopUpToSaveState`).
    pub fn should_pop_up_to_save_state(&self) -> bool {
        self.pop_up_to_save_state
    }
    /// The id popped up to (-1: none) (Kotlin's `popUpToId`).
    pub fn pop_up_to_id(&self) -> i32 {
        self.pop_up_to_id
    }
    /// The route popped up to (Kotlin's `popUpToRoute`).
    pub fn pop_up_to_route(&self) -> Option<String> {
        self.pop_up_to_route.clone()
    }
    /// The typed route class popped up to (Kotlin's `popUpToRouteClass`).
    pub fn pop_up_to_route_class(&self) -> Option<RouteClass> {
        self.pop_up_to_route_class
    }
    /// The typed route value popped up to (Kotlin's `popUpToRouteObject`).
    pub fn pop_up_to_route_object(&self) -> Option<Rc<dyn AnyRoute>> {
        self.pop_up_to_route_object.clone()
    }

    /// A builder (Kotlin's `NavOptions.Builder()`).
    pub fn builder() -> NavOptionsBuilderBase {
        NavOptionsBuilderBase::default()
    }
}

impl PartialEq for NavOptions {
    fn eq(&self, other: &Self) -> bool {
        let objects = match (&self.pop_up_to_route_object, &other.pop_up_to_route_object) {
            (None, None) => true,
            (Some(a), Some(b)) => a.route_equals(&**b),
            _ => false,
        };
        self.single_top == other.single_top
            && self.restore_state == other.restore_state
            && self.pop_up_to_id == other.pop_up_to_id
            && self.pop_up_to_route == other.pop_up_to_route
            && self.pop_up_to_route_class == other.pop_up_to_route_class
            && objects
            && self.pop_up_to_inclusive == other.pop_up_to_inclusive
            && self.pop_up_to_save_state == other.pop_up_to_save_state
    }
}

impl fmt::Debug for NavOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NavOptions(")?;
        if self.single_top {
            write!(f, "launchSingleTop ")?;
        }
        if self.restore_state {
            write!(f, "restoreState ")?;
        }
        if self.pop_up_to_route.is_some() || self.pop_up_to_id != -1 {
            write!(f, "popUpTo(")?;
            if let Some(r) = &self.pop_up_to_route {
                write!(f, "{r}")?;
            } else if let Some(c) = &self.pop_up_to_route_class {
                write!(f, "{c:?}")?;
            } else if let Some(o) = &self.pop_up_to_route_object {
                write!(f, "{o:?}")?;
            } else {
                write!(f, "0x{:x}", self.pop_up_to_id)?;
            }
            if self.pop_up_to_inclusive {
                write!(f, " inclusive")?;
            }
            if self.pop_up_to_save_state {
                write!(f, " saveState")?;
            }
            write!(f, ")")?;
        }
        write!(f, ")")
    }
}

/// Kotlin's `NavOptions.Builder`.
///
/// Kotlin: `NavOptions.Builder`
#[derive(Clone)]
pub struct NavOptionsBuilderBase {
    single_top: bool,
    restore_state: bool,
    pop_up_to_id: i32,
    pop_up_to_route: Option<String>,
    pop_up_to_route_class: Option<RouteClass>,
    pop_up_to_route_object: Option<Rc<dyn AnyRoute>>,
    pop_up_to_inclusive: bool,
    pop_up_to_save_state: bool,
}

impl Default for NavOptionsBuilderBase {
    fn default() -> Self {
        NavOptionsBuilderBase {
            single_top: false,
            restore_state: false,
            pop_up_to_id: -1,
            pop_up_to_route: None,
            pop_up_to_route_class: None,
            pop_up_to_route_object: None,
            pop_up_to_inclusive: false,
            pop_up_to_save_state: false,
        }
    }
}

impl NavOptionsBuilderBase {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_launch_single_top(&mut self, single_top: bool) -> &mut Self {
        self.single_top = single_top;
        self
    }
    pub fn set_restore_state(&mut self, restore_state: bool) -> &mut Self {
        self.restore_state = restore_state;
        self
    }
    /// Pops up to the destination of id `destination_id` (Kotlin's `setPopUpTo(destinationId, inclusive,
    /// saveState)`).
    pub fn set_pop_up_to_id(&mut self, destination_id: i32, inclusive: bool, save_state: bool) -> &mut Self {
        self.pop_up_to_id = destination_id;
        self.pop_up_to_route = None;
        self.pop_up_to_inclusive = inclusive;
        self.pop_up_to_save_state = save_state;
        self
    }
    /// Pops up to `route`'s destination (Kotlin's `setPopUpTo(route, inclusive, saveState)`).
    pub fn set_pop_up_to(&mut self, route: Option<String>, inclusive: bool, save_state: bool) -> &mut Self {
        self.pop_up_to_id = -1;
        self.pop_up_to_route = route;
        self.pop_up_to_inclusive = inclusive;
        self.pop_up_to_save_state = save_state;
        self
    }
    /// Pops up to `T`'s destination (Kotlin's `setPopUpTo<T>(inclusive, saveState)`).
    pub fn set_pop_up_to_of<T: Route>(&mut self, inclusive: bool, save_state: bool) -> &mut Self {
        self.set_pop_up_to_class(RouteClass::of::<T>(), inclusive, save_state)
    }
    /// Kotlin's `setPopUpTo(route: KClass<T>, inclusive, saveState)`.
    pub fn set_pop_up_to_class(&mut self, route: RouteClass, inclusive: bool, save_state: bool) -> &mut Self {
        self.pop_up_to_route_class = Some(route);
        self.pop_up_to_inclusive = inclusive;
        self.pop_up_to_save_state = save_state;
        self
    }
    /// Kotlin's `setPopUpTo(route: T, inclusive, saveState)`.
    pub fn set_pop_up_to_object(&mut self, route: Rc<dyn AnyRoute>, inclusive: bool, save_state: bool) -> &mut Self {
        self.pop_up_to_route_object = Some(route);
        self.pop_up_to_inclusive = inclusive;
        self.pop_up_to_save_state = save_state;
        self
    }

    pub fn build(&self) -> NavOptions {
        let mut o = NavOptions {
            single_top: self.single_top,
            restore_state: self.restore_state,
            pop_up_to_id: self.pop_up_to_id,
            pop_up_to_route: None,
            pop_up_to_route_class: None,
            pop_up_to_route_object: None,
            pop_up_to_inclusive: self.pop_up_to_inclusive,
            pop_up_to_save_state: self.pop_up_to_save_state,
        };
        // (Kotlin's constructors: the id is the route's, the class's or the object's class's hash code)
        if let Some(route) = &self.pop_up_to_route {
            o.pop_up_to_id = string_hash(&create_route(Some(route)));
            o.pop_up_to_route = Some(route.clone());
        } else if let Some(class) = self.pop_up_to_route_class {
            o.pop_up_to_id = class.hash_code();
            o.pop_up_to_route_class = Some(class);
        } else if let Some(object) = &self.pop_up_to_route_object {
            o.pop_up_to_id = hash_code_of(object.route_descriptor());
            o.pop_up_to_route_object = Some(object.clone());
        }
        o
    }
}

/// What `pop_up_to`'s block sets (Compose's `PopUpToBuilder`): `inclusive`, `saveState`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PopUpToBuilder {
    inclusive: bool,
    save_state: bool,
}

impl PopUpToBuilder {
    /// Kotlin's `inclusive`.
    pub fn inclusive(&self) -> bool {
        self.inclusive
    }
    /// Kotlin's `inclusive = …`: the destination popped up to goes too.
    pub fn set_inclusive(&mut self, value: bool) {
        self.inclusive = value;
    }
    /// Kotlin's `saveState`.
    pub fn save_state(&self) -> bool {
        self.save_state
    }
    /// Kotlin's `saveState = …`: what is popped is saved, for a later `restoreState`.
    pub fn set_save_state(&mut self, value: bool) {
        self.save_state = value;
    }
}

/// What `nav_options`' block sets (Compose's `NavOptionsBuilder`): `launchSingleTop`, `restoreState`, `popUpTo(…)`.
pub struct NavOptionsBuilder {
    launch_single_top: bool,
    restore_state: bool,
    pop_up_to_id: i32,
    pop_up_to_route: Option<String>,
    pop_up_to_route_class: Option<RouteClass>,
    pop_up_to_route_object: Option<Rc<dyn AnyRoute>>,
    inclusive: bool,
    save_state: bool,
}

impl Default for NavOptionsBuilder {
    fn default() -> Self {
        NavOptionsBuilder {
            launch_single_top: false,
            restore_state: false,
            pop_up_to_id: -1,
            pop_up_to_route: None,
            pop_up_to_route_class: None,
            pop_up_to_route_object: None,
            inclusive: false,
            save_state: false,
        }
    }
}

impl NavOptionsBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    /// Kotlin's `launchSingleTop`.
    pub fn launch_single_top(&self) -> bool {
        self.launch_single_top
    }
    /// Kotlin's `launchSingleTop = …`: the destination, already on top, isn't stacked again.
    pub fn set_launch_single_top(&mut self, value: bool) {
        self.launch_single_top = value;
    }
    /// Kotlin's `restoreState`.
    pub fn restore_state(&self) -> bool {
        self.restore_state
    }
    /// Kotlin's `restoreState = …`: a stack saved for the destination (`popUpTo { saveState = true }`) comes
    /// back.
    pub fn set_restore_state(&mut self, value: bool) {
        self.restore_state = value;
    }
    /// Kotlin's `popUpToId`.
    pub fn pop_up_to_id(&self) -> i32 {
        self.pop_up_to_id
    }
    /// Kotlin's `popUpToRoute`.
    pub fn pop_up_to_route(&self) -> Option<String> {
        self.pop_up_to_route.clone()
    }
    /// Kotlin's `popUpToRouteClass`.
    pub fn pop_up_to_route_class(&self) -> Option<RouteClass> {
        self.pop_up_to_route_class
    }
    /// Kotlin's `popUpToRouteObject`.
    pub fn pop_up_to_route_object(&self) -> Option<Rc<dyn AnyRoute>> {
        self.pop_up_to_route_object.clone()
    }

    fn apply(&mut self, pop_up_to_builder: impl FnOnce(&mut PopUpToBuilder)) {
        let mut b = PopUpToBuilder::default();
        pop_up_to_builder(&mut b);
        self.inclusive = b.inclusive;
        self.save_state = b.save_state;
    }

    /// Pops up to the destination of id `id` first (Kotlin's `popUpTo(id) { … }`).
    pub fn pop_up_to_id_with(&mut self, id: i32, pop_up_to_builder: impl FnOnce(&mut PopUpToBuilder)) {
        self.pop_up_to_id = id;
        self.pop_up_to_route = None;
        self.apply(pop_up_to_builder);
    }

    /// Pops up to `route`'s destination first (Kotlin's `popUpTo(route) { inclusive = …; saveState = … }`).
    ///
    /// Kotlin: `popUpTo(route: String, popUpToBuilder: PopUpToBuilder.() -> Unit = {})`
    ///
    /// # Panics
    /// A blank route.
    pub fn pop_up_to(&mut self, route: &str, pop_up_to_builder: impl FnOnce(&mut PopUpToBuilder)) {
        assert!(!route.trim().is_empty(), "Cannot pop up to an empty route");
        self.pop_up_to_route = Some(route.to_string());
        self.pop_up_to_id = -1;
        self.apply(pop_up_to_builder);
    }

    /// Pops up to `T`'s destination first (Compose's `popUpTo<T> { … }`).
    ///
    /// Kotlin: `popUpTo<T>(popUpToBuilder: PopUpToBuilder.() -> Unit = {})`
    pub fn pop_up_to_of<T: Route>(&mut self, pop_up_to_builder: impl FnOnce(&mut PopUpToBuilder)) {
        self.pop_up_to_class(RouteClass::of::<T>(), pop_up_to_builder);
    }

    /// Kotlin's `popUpTo(route: KClass<T>) { … }`.
    pub fn pop_up_to_class(&mut self, route: RouteClass, pop_up_to_builder: impl FnOnce(&mut PopUpToBuilder)) {
        self.pop_up_to_route_class = Some(route);
        self.pop_up_to_id = -1;
        self.pop_up_to_route = None;
        self.apply(pop_up_to_builder);
    }

    /// Pops up to `route`'s destination (with its values) first (Kotlin's `popUpTo(route: T) { … }`).
    ///
    /// Kotlin: `popUpTo(route: T, popUpToBuilder: PopUpToBuilder.() -> Unit = {})`
    pub fn pop_up_to_object<T: Route>(&mut self, route: T, pop_up_to_builder: impl FnOnce(&mut PopUpToBuilder)) {
        self.pop_up_to_route_object = Some(Rc::new(route));
        self.pop_up_to_id = -1;
        self.pop_up_to_route = None;
        self.apply(pop_up_to_builder);
    }

    pub(crate) fn build(&self) -> NavOptions {
        let mut b = NavOptionsBuilderBase::new();
        b.set_launch_single_top(self.launch_single_top);
        b.set_restore_state(self.restore_state);
        if let Some(route) = &self.pop_up_to_route {
            b.set_pop_up_to(Some(route.clone()), self.inclusive, self.save_state);
        } else if let Some(class) = self.pop_up_to_route_class {
            b.set_pop_up_to_class(class, self.inclusive, self.save_state);
        } else if let Some(object) = &self.pop_up_to_route_object {
            b.set_pop_up_to_object(object.clone(), self.inclusive, self.save_state);
        } else {
            b.set_pop_up_to_id(self.pop_up_to_id, self.inclusive, self.save_state);
        }
        b.build()
    }
}

/// The options `options_builder` sets (Compose's `navOptions { popUpTo<Home> { saveState = true };
/// launchSingleTop = true; restoreState = true }`).
pub fn nav_options(options_builder: impl FnOnce(&mut NavOptionsBuilder)) -> NavOptions {
    let mut b = NavOptionsBuilder::new();
    options_builder(&mut b);
    b.build()
}

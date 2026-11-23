//! Building a NavHost's graph (navigation-compose's `ComposeNavigatorDestinationBuilder`,
//! `DialogNavigatorDestinationBuilder` and NavGraphBuilder's `composable`, `navigation` and `dialog`): KompozR's
//! builder of a composable destination is `kompozable` (C++ Kompoz's name), `composable` Compose's (both here, the
//! same). Rust has no overloads: a route's form is `kompozable(route, …)`, a typed route's `kompozable_of::<T>(…)`, a
//! route class's `kompozable_class(route, …)` (Kotlin's `composable(route: KClass<T>, …)`); the same of `navigation`
//! and `dialog`.
//!
//! A pop transition left out is the enter (exit) transition given (Kotlin's default `popEnterTransition =
//! enterTransition`); Kotlin's explicit `null` there (the graph's or the NavHost's then) is a transition giving none,
//! `Some(Rc::new(|_| None))`.

use super::dialog::{DialogNavigator, DialogNavigatorDestination};
use super::navigator::{
    ComposeNavigator, ComposeNavigatorDestination, DestinationEnterTransition, DestinationExitTransition,
    DestinationSizeTransform, NavTransitions,
};
use crate::argument::NamedNavArgument;
use crate::builder::{DestinationBuilder, IntoStartDestination, NavDestinationBuilder, NavGraphBuilder};
use crate::deep_link::NavDeepLink;
use crate::destination::NavDestination;
use crate::entry::NavBackStackEntry;
use crate::serialization::{Route, RouteClass, TypeMap};
use kompoz_animation::AnimatedContentScope;
use kompoz_runtime::named;
use kompoz_ui::window::DialogProperties;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

/// The builder of a composable destination (Compose's `ComposeNavigatorDestinationBuilder`): a
/// [`NavDestinationBuilder`] (it derefs to it: arguments, deep links) of a [`ComposeNavigator`]'s destination with
/// its content and transitions.
pub struct ComposeNavigatorDestinationBuilder {
    base: NavDestinationBuilder,
    compose_navigator: Rc<ComposeNavigator>,
    content: Rc<dyn Fn(&AnimatedContentScope, NavBackStackEntry)>,
    /// Kotlin's `enterTransition`.
    pub enter_transition: Option<DestinationEnterTransition>,
    /// Kotlin's `exitTransition`.
    pub exit_transition: Option<DestinationExitTransition>,
    /// Kotlin's `popEnterTransition`.
    pub pop_enter_transition: Option<DestinationEnterTransition>,
    /// Kotlin's `popExitTransition`.
    pub pop_exit_transition: Option<DestinationExitTransition>,
    /// Kotlin's `sizeTransform`.
    pub size_transform: Option<DestinationSizeTransform>,
}

impl ComposeNavigatorDestinationBuilder {
    fn make(
        base: NavDestinationBuilder,
        navigator: Rc<ComposeNavigator>,
        content: impl Fn(&AnimatedContentScope, NavBackStackEntry) + 'static,
    ) -> Self {
        ComposeNavigatorDestinationBuilder {
            base,
            compose_navigator: navigator,
            content: Rc::new(content),
            enter_transition: None,
            exit_transition: None,
            pop_enter_transition: None,
            pop_exit_transition: None,
            size_transform: None,
        }
    }

    /// A destination of `navigator` with `route` (Kotlin's `ComposeNavigatorDestinationBuilder(navigator, route,
    /// content)`).
    pub fn new(
        navigator: Rc<ComposeNavigator>,
        route: &str,
        content: impl Fn(&AnimatedContentScope, NavBackStackEntry) + 'static,
    ) -> Self {
        let base = NavDestinationBuilder::new(navigator.clone(), Some(route.to_string()));
        ComposeNavigatorDestinationBuilder::make(base, navigator, content)
    }

    /// A destination of the typed route `route` (Kotlin's `ComposeNavigatorDestinationBuilder(navigator, route:
    /// KClass<*>, typeMap, content)`).
    pub fn new_class(
        navigator: Rc<ComposeNavigator>,
        route: RouteClass,
        type_map: TypeMap,
        content: impl Fn(&AnimatedContentScope, NavBackStackEntry) + 'static,
    ) -> Self {
        let base = NavDestinationBuilder::new_class(navigator.clone(), Some(route), type_map);
        ComposeNavigatorDestinationBuilder::make(base, navigator, content)
    }

    /// The destination (Kotlin's `build()`: its `instantiateDestination` filled, the transitions set).
    pub fn build(self) -> ComposeNavigatorDestination {
        let ComposeNavigatorDestinationBuilder {
            base,
            compose_navigator,
            content,
            enter_transition,
            exit_transition,
            pop_enter_transition,
            pop_exit_transition,
            size_transform,
        } = self;
        let destination =
            ComposeNavigatorDestination::new(&compose_navigator, move |scope, entry| content(scope, entry));
        base.build_with(destination.as_destination());
        let t = destination.transitions();
        *t.enter_transition.borrow_mut() = enter_transition;
        *t.exit_transition.borrow_mut() = exit_transition;
        *t.pop_enter_transition.borrow_mut() = pop_enter_transition;
        *t.pop_exit_transition.borrow_mut() = pop_exit_transition;
        *t.size_transform.borrow_mut() = size_transform;
        destination
    }
}

impl Deref for ComposeNavigatorDestinationBuilder {
    type Target = NavDestinationBuilder;
    fn deref(&self) -> &NavDestinationBuilder {
        &self.base
    }
}

impl DerefMut for ComposeNavigatorDestinationBuilder {
    fn deref_mut(&mut self) -> &mut NavDestinationBuilder {
        &mut self.base
    }
}

impl DestinationBuilder for ComposeNavigatorDestinationBuilder {
    fn build_destination(self) -> NavDestination {
        self.build().as_destination()
    }
}

/// The builder of a dialog destination (Compose's `DialogNavigatorDestinationBuilder`): a [`NavDestinationBuilder`]
/// (it derefs to it) of a [`DialogNavigator`]'s destination with its dialog's properties and content.
pub struct DialogNavigatorDestinationBuilder {
    base: NavDestinationBuilder,
    dialog_navigator: Rc<DialogNavigator>,
    dialog_properties: DialogProperties,
    content: Rc<dyn Fn(NavBackStackEntry)>,
}

impl DialogNavigatorDestinationBuilder {
    /// A dialog destination of `navigator` with `route` (Kotlin's `DialogNavigatorDestinationBuilder(navigator,
    /// route, dialogProperties, content)`).
    pub fn new(
        navigator: Rc<DialogNavigator>,
        route: &str,
        dialog_properties: DialogProperties,
        content: impl Fn(NavBackStackEntry) + 'static,
    ) -> Self {
        let base = NavDestinationBuilder::new(navigator.clone(), Some(route.to_string()));
        DialogNavigatorDestinationBuilder {
            base,
            dialog_navigator: navigator,
            dialog_properties,
            content: Rc::new(content),
        }
    }

    /// A dialog destination of the typed route `route` (Kotlin's `DialogNavigatorDestinationBuilder(navigator,
    /// route: KClass<*>, typeMap, dialogProperties, content)`).
    pub fn new_class(
        navigator: Rc<DialogNavigator>,
        route: RouteClass,
        type_map: TypeMap,
        dialog_properties: DialogProperties,
        content: impl Fn(NavBackStackEntry) + 'static,
    ) -> Self {
        let base = NavDestinationBuilder::new_class(navigator.clone(), Some(route), type_map);
        DialogNavigatorDestinationBuilder {
            base,
            dialog_navigator: navigator,
            dialog_properties,
            content: Rc::new(content),
        }
    }

    /// The destination (Kotlin's `build()` of its `instantiateDestination`).
    pub fn build(self) -> DialogNavigatorDestination {
        let content = self.content;
        let destination =
            DialogNavigatorDestination::new(&self.dialog_navigator, self.dialog_properties, move |e| content(e));
        self.base.build_with(destination.clone().into());
        destination
    }
}

impl Deref for DialogNavigatorDestinationBuilder {
    type Target = NavDestinationBuilder;
    fn deref(&self) -> &NavDestinationBuilder {
        &self.base
    }
}

impl DerefMut for DialogNavigatorDestinationBuilder {
    fn deref_mut(&mut self) -> &mut NavDestinationBuilder {
        &mut self.base
    }
}

impl DestinationBuilder for DialogNavigatorDestinationBuilder {
    fn build_destination(self) -> NavDestination {
        self.build().into()
    }
}

/// The transitions of a composable builder, the pop ones the enter/exit ones when left out.
fn set_transitions(
    b: &mut ComposeNavigatorDestinationBuilder,
    enter_transition: Option<DestinationEnterTransition>,
    exit_transition: Option<DestinationExitTransition>,
    pop_enter_transition: Option<DestinationEnterTransition>,
    pop_exit_transition: Option<DestinationExitTransition>,
    size_transform: Option<DestinationSizeTransform>,
) {
    b.pop_enter_transition = pop_enter_transition.or_else(|| enter_transition.clone());
    b.pop_exit_transition = pop_exit_transition.or_else(|| exit_transition.clone());
    b.enter_transition = enter_transition;
    b.exit_transition = exit_transition;
    b.size_transform = size_transform;
}

/// The same of a nested graph, if it is a NavHost's (Kotlin's `if (this is ComposeNavGraph)`).
fn set_graph_transitions(
    graph: &NavDestination,
    enter_transition: Option<DestinationEnterTransition>,
    exit_transition: Option<DestinationExitTransition>,
    pop_enter_transition: Option<DestinationEnterTransition>,
    pop_exit_transition: Option<DestinationExitTransition>,
    size_transform: Option<DestinationSizeTransform>,
) {
    if graph.extra::<super::navigator::ComposeNavGraphData>().is_none() {
        return;
    }
    let t: Rc<NavTransitions> = NavTransitions::of(graph).expect("a NavHost's graph");
    *t.pop_enter_transition.borrow_mut() = pop_enter_transition.or_else(|| enter_transition.clone());
    *t.pop_exit_transition.borrow_mut() = pop_exit_transition.or_else(|| exit_transition.clone());
    *t.enter_transition.borrow_mut() = enter_transition;
    *t.exit_transition.borrow_mut() = exit_transition;
    *t.size_transform.borrow_mut() = size_transform;
}

#[named]
impl NavGraphBuilder {
    /// Adds a composable destination of `route` showing `content` (Compose's `composable(route, …) { entry -> … }`;
    /// KompozR's name, Compose's is [`composable`](Self::composable)): its arguments, deep links and transitions
    /// (none: its graph's, then the NavHost's).
    ///
    /// Kotlin: `kompozable(route: String, arguments: List<NamedNavArgument> = emptyList(), deepLinks: List<NavDeepLink> = emptyList(), enterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = null, exitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = null, popEnterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = enterTransition, popExitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = exitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = null, content: @Composable AnimatedContentScope.(NavBackStackEntry) -> Unit)`
    #[allow(clippy::too_many_arguments)]
    pub fn kompozable(
        &mut self,
        route: &str,
        #[default(Vec::new())] arguments: Vec<NamedNavArgument>,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(None)] enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] exit_transition: Option<DestinationExitTransition>,
        #[default(None)] pop_enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] pop_exit_transition: Option<DestinationExitTransition>,
        #[default(None)] size_transform: Option<DestinationSizeTransform>,
        content: impl Fn(&AnimatedContentScope, NavBackStackEntry) + 'static,
    ) {
        self.kompozable_impl_route(
            route,
            arguments,
            deep_links,
            [enter_transition, pop_enter_transition],
            [exit_transition, pop_exit_transition],
            size_transform,
            Rc::new(content),
        );
    }

    /// Compose's name of [`kompozable`](Self::kompozable).
    ///
    /// Kotlin: `composable(route: String, arguments: List<NamedNavArgument> = emptyList(), deepLinks: List<NavDeepLink> = emptyList(), enterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = null, exitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = null, popEnterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = enterTransition, popExitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = exitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = null, content: @Composable AnimatedContentScope.(NavBackStackEntry) -> Unit)`
    #[allow(clippy::too_many_arguments)]
    pub fn composable(
        &mut self,
        route: &str,
        #[default(Vec::new())] arguments: Vec<NamedNavArgument>,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(None)] enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] exit_transition: Option<DestinationExitTransition>,
        #[default(None)] pop_enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] pop_exit_transition: Option<DestinationExitTransition>,
        #[default(None)] size_transform: Option<DestinationSizeTransform>,
        content: impl Fn(&AnimatedContentScope, NavBackStackEntry) + 'static,
    ) {
        self.kompozable_impl_route(
            route,
            arguments,
            deep_links,
            [enter_transition, pop_enter_transition],
            [exit_transition, pop_exit_transition],
            size_transform,
            Rc::new(content),
        );
    }

    /// Adds a composable destination of the typed route `T` (Compose's `composable<T> { entry -> … }`; KompozR's
    /// name): its arguments `T`'s fields (of the types `type_map` adds), its deep links and transitions.
    ///
    /// Kotlin: `kompozable<T>(typeMap: Map<KType, NavType<*>> = emptyMap(), deepLinks: List<NavDeepLink> = emptyList(), enterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = null, exitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = null, popEnterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = enterTransition, popExitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = exitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = null, content: @Composable AnimatedContentScope.(NavBackStackEntry) -> Unit)`
    #[allow(clippy::too_many_arguments)]
    pub fn kompozable_of<T: Route>(
        &mut self,
        #[default(TypeMap::default())] type_map: TypeMap,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(None)] enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] exit_transition: Option<DestinationExitTransition>,
        #[default(None)] pop_enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] pop_exit_transition: Option<DestinationExitTransition>,
        #[default(None)] size_transform: Option<DestinationSizeTransform>,
        content: impl Fn(&AnimatedContentScope, NavBackStackEntry) + 'static,
    ) {
        self.kompozable_impl_class(
            RouteClass::of::<T>(),
            type_map,
            deep_links,
            [enter_transition, pop_enter_transition],
            [exit_transition, pop_exit_transition],
            size_transform,
            Rc::new(content),
        );
    }

    /// Compose's name of [`kompozable_of`](Self::kompozable_of).
    ///
    /// Kotlin: `composable<T>(typeMap: Map<KType, NavType<*>> = emptyMap(), deepLinks: List<NavDeepLink> = emptyList(), enterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = null, exitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = null, popEnterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = enterTransition, popExitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = exitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = null, content: @Composable AnimatedContentScope.(NavBackStackEntry) -> Unit)`
    #[allow(clippy::too_many_arguments)]
    pub fn composable_of<T: Route>(
        &mut self,
        #[default(TypeMap::default())] type_map: TypeMap,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(None)] enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] exit_transition: Option<DestinationExitTransition>,
        #[default(None)] pop_enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] pop_exit_transition: Option<DestinationExitTransition>,
        #[default(None)] size_transform: Option<DestinationSizeTransform>,
        content: impl Fn(&AnimatedContentScope, NavBackStackEntry) + 'static,
    ) {
        self.kompozable_impl_class(
            RouteClass::of::<T>(),
            type_map,
            deep_links,
            [enter_transition, pop_enter_transition],
            [exit_transition, pop_exit_transition],
            size_transform,
            Rc::new(content),
        );
    }

    /// Adds a composable destination of the route class `route` (Compose's `composable(route: KClass<T>, …)`;
    /// KompozR's name).
    ///
    /// Kotlin: `kompozable(route: KClass<T>, typeMap: Map<KType, NavType<*>> = emptyMap(), deepLinks: List<NavDeepLink> = emptyList(), enterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = null, exitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = null, popEnterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = enterTransition, popExitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = exitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = null, content: @Composable AnimatedContentScope.(NavBackStackEntry) -> Unit)`
    #[allow(clippy::too_many_arguments)]
    pub fn kompozable_class(
        &mut self,
        route: RouteClass,
        #[default(TypeMap::default())] type_map: TypeMap,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(None)] enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] exit_transition: Option<DestinationExitTransition>,
        #[default(None)] pop_enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] pop_exit_transition: Option<DestinationExitTransition>,
        #[default(None)] size_transform: Option<DestinationSizeTransform>,
        content: impl Fn(&AnimatedContentScope, NavBackStackEntry) + 'static,
    ) {
        self.kompozable_impl_class(
            route,
            type_map,
            deep_links,
            [enter_transition, pop_enter_transition],
            [exit_transition, pop_exit_transition],
            size_transform,
            Rc::new(content),
        );
    }

    /// Compose's name of [`kompozable_class`](Self::kompozable_class).
    ///
    /// Kotlin: `composable(route: KClass<T>, typeMap: Map<KType, NavType<*>> = emptyMap(), deepLinks: List<NavDeepLink> = emptyList(), enterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = null, exitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = null, popEnterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = enterTransition, popExitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = exitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = null, content: @Composable AnimatedContentScope.(NavBackStackEntry) -> Unit)`
    #[allow(clippy::too_many_arguments)]
    pub fn composable_class(
        &mut self,
        route: RouteClass,
        #[default(TypeMap::default())] type_map: TypeMap,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(None)] enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] exit_transition: Option<DestinationExitTransition>,
        #[default(None)] pop_enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] pop_exit_transition: Option<DestinationExitTransition>,
        #[default(None)] size_transform: Option<DestinationSizeTransform>,
        content: impl Fn(&AnimatedContentScope, NavBackStackEntry) + 'static,
    ) {
        self.kompozable_impl_class(
            route,
            type_map,
            deep_links,
            [enter_transition, pop_enter_transition],
            [exit_transition, pop_exit_transition],
            size_transform,
            Rc::new(content),
        );
    }

    /// Adds a nested graph of `route` starting at `start_destination`, `builder` adding its destinations (Compose's
    /// `navigation(startDestination, route) { … }`): its arguments, deep links and the transitions of the
    /// destinations in it (theirs first).
    ///
    /// Kotlin: `navigation(startDestination: String, route: String, arguments: List<NamedNavArgument> = emptyList(), deepLinks: List<NavDeepLink> = emptyList(), enterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = null, exitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = null, popEnterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = enterTransition, popExitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = exitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = null, builder: NavGraphBuilder.() -> Unit)`
    #[allow(clippy::too_many_arguments)]
    pub fn navigation(
        &mut self,
        start_destination: &str,
        route: &str,
        #[default(Vec::new())] arguments: Vec<NamedNavArgument>,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(None)] enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] exit_transition: Option<DestinationExitTransition>,
        #[default(None)] pop_enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] pop_exit_transition: Option<DestinationExitTransition>,
        #[default(None)] size_transform: Option<DestinationSizeTransform>,
        builder: impl FnOnce(&mut NavGraphBuilder),
    ) {
        let mut nested = NavGraphBuilder::new(&self.provider(), start_destination, Some(route));
        builder(&mut nested);
        let graph = nested.build().as_destination();
        for a in arguments {
            graph.add_argument(a.name(), a.argument());
        }
        for d in deep_links {
            graph.add_nav_deep_link(d);
        }
        set_graph_transitions(
            &graph,
            enter_transition,
            exit_transition,
            pop_enter_transition,
            pop_exit_transition,
            size_transform,
        );
        self.add_destination(graph);
    }

    /// Adds a nested graph of the typed route `T` starting at `start_destination` — a typed route's value or a
    /// [`RouteClass`] — (Compose's `navigation<T>(startDestination = Start) { … }`).
    ///
    /// Kotlin: `navigation<T>(startDestination: Any, typeMap: Map<KType, NavType<*>> = emptyMap(), deepLinks: List<NavDeepLink> = emptyList(), enterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = null, exitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = null, popEnterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = enterTransition, popExitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = exitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = null, builder: NavGraphBuilder.() -> Unit)`
    #[allow(clippy::too_many_arguments)]
    pub fn navigation_of<T: Route>(
        &mut self,
        start_destination: impl IntoStartDestination,
        #[default(TypeMap::default())] type_map: TypeMap,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(None)] enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] exit_transition: Option<DestinationExitTransition>,
        #[default(None)] pop_enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] pop_exit_transition: Option<DestinationExitTransition>,
        #[default(None)] size_transform: Option<DestinationSizeTransform>,
        builder: impl FnOnce(&mut NavGraphBuilder),
    ) {
        self.navigation_class_impl(
            start_destination,
            RouteClass::of::<T>(),
            type_map,
            deep_links,
            [enter_transition, pop_enter_transition],
            [exit_transition, pop_exit_transition],
            size_transform,
            builder,
        );
    }

    /// Adds a nested graph of the route class `route` (Compose's `navigation(startDestination, route: KClass<T>,
    /// …) { … }`).
    ///
    /// Kotlin: `navigation(startDestination: Any, route: KClass<T>, typeMap: Map<KType, NavType<*>> = emptyMap(), deepLinks: List<NavDeepLink> = emptyList(), enterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = null, exitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = null, popEnterTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition?)? = enterTransition, popExitTransition: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition?)? = exitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = null, builder: NavGraphBuilder.() -> Unit)`
    #[allow(clippy::too_many_arguments)]
    pub fn navigation_class(
        &mut self,
        start_destination: impl IntoStartDestination,
        route: RouteClass,
        #[default(TypeMap::default())] type_map: TypeMap,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(None)] enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] exit_transition: Option<DestinationExitTransition>,
        #[default(None)] pop_enter_transition: Option<DestinationEnterTransition>,
        #[default(None)] pop_exit_transition: Option<DestinationExitTransition>,
        #[default(None)] size_transform: Option<DestinationSizeTransform>,
        builder: impl FnOnce(&mut NavGraphBuilder),
    ) {
        self.navigation_class_impl(
            start_destination,
            route,
            type_map,
            deep_links,
            [enter_transition, pop_enter_transition],
            [exit_transition, pop_exit_transition],
            size_transform,
            builder,
        );
    }

    /// Adds a dialog destination of `route` showing `content` in a `Dialog` of `dialog_properties` (Compose's
    /// `dialog(route) { entry -> … }`).
    ///
    /// Kotlin: `dialog(route: String, arguments: List<NamedNavArgument> = emptyList(), deepLinks: List<NavDeepLink> = emptyList(), dialogProperties: DialogProperties = DialogProperties(), content: @Composable (NavBackStackEntry) -> Unit)`
    pub fn dialog(
        &mut self,
        route: &str,
        #[default(Vec::new())] arguments: Vec<NamedNavArgument>,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(DialogProperties::default())] dialog_properties: DialogProperties,
        content: impl Fn(NavBackStackEntry) + 'static,
    ) {
        let navigator = self.provider().get_navigator_of::<DialogNavigator>();
        let mut b = DialogNavigatorDestinationBuilder::new(navigator, route, dialog_properties, content);
        for a in arguments {
            b.argument_value(&a.name(), a.argument());
        }
        for d in deep_links {
            b.deep_link_value(d);
        }
        self.destination(b);
    }

    /// Adds a dialog destination of the typed route `T` (Compose's `dialog<T> { entry -> … }`).
    ///
    /// Kotlin: `dialog<T>(typeMap: Map<KType, NavType<*>> = emptyMap(), deepLinks: List<NavDeepLink> = emptyList(), dialogProperties: DialogProperties = DialogProperties(), content: @Composable (NavBackStackEntry) -> Unit)`
    pub fn dialog_of<T: Route>(
        &mut self,
        #[default(TypeMap::default())] type_map: TypeMap,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(DialogProperties::default())] dialog_properties: DialogProperties,
        content: impl Fn(NavBackStackEntry) + 'static,
    ) {
        self.dialog_class_impl(RouteClass::of::<T>(), type_map, deep_links, dialog_properties, Rc::new(content));
    }

    /// Adds a dialog destination of the route class `route` (Compose's `dialog(route: KClass<T>, …)`).
    ///
    /// Kotlin: `dialog(route: KClass<T>, typeMap: Map<KType, NavType<*>> = emptyMap(), deepLinks: List<NavDeepLink> = emptyList(), dialogProperties: DialogProperties = DialogProperties(), content: @Composable (NavBackStackEntry) -> Unit)`
    pub fn dialog_class(
        &mut self,
        route: RouteClass,
        #[default(TypeMap::default())] type_map: TypeMap,
        #[default(Vec::new())] deep_links: Vec<NavDeepLink>,
        #[default(DialogProperties::default())] dialog_properties: DialogProperties,
        content: impl Fn(NavBackStackEntry) + 'static,
    ) {
        self.dialog_class_impl(route, type_map, deep_links, dialog_properties, Rc::new(content));
    }
}

type Content = Rc<dyn Fn(&AnimatedContentScope, NavBackStackEntry)>;

impl NavGraphBuilder {
    fn dialog_class_impl(
        &mut self,
        route: RouteClass,
        type_map: TypeMap,
        deep_links: Vec<NavDeepLink>,
        dialog_properties: DialogProperties,
        content: Rc<dyn Fn(NavBackStackEntry)>,
    ) {
        let navigator = self.provider().get_navigator_of::<DialogNavigator>();
        let mut b =
            DialogNavigatorDestinationBuilder::new_class(navigator, route, type_map, dialog_properties, move |e| {
                content(e)
            });
        for d in deep_links {
            b.deep_link_value(d);
        }
        self.destination(b);
    }

    #[allow(clippy::too_many_arguments)]
    fn navigation_class_impl(
        &mut self,
        start_destination: impl IntoStartDestination,
        route: RouteClass,
        type_map: TypeMap,
        deep_links: Vec<NavDeepLink>,
        [enter, pop_enter]: [Option<DestinationEnterTransition>; 2],
        [exit, pop_exit]: [Option<DestinationExitTransition>; 2],
        size_transform: Option<DestinationSizeTransform>,
        builder: impl FnOnce(&mut NavGraphBuilder),
    ) {
        let mut nested = NavGraphBuilder::new_typed(&self.provider(), start_destination, Some(route), type_map);
        builder(&mut nested);
        let graph = nested.build().as_destination();
        for d in deep_links {
            graph.add_nav_deep_link(d);
        }
        set_graph_transitions(&graph, enter, exit, pop_enter, pop_exit, size_transform);
        self.add_destination(graph);
    }

    #[allow(clippy::too_many_arguments)]
    fn kompozable_impl_route(
        &mut self,
        route: &str,
        arguments: Vec<NamedNavArgument>,
        deep_links: Vec<NavDeepLink>,
        [enter, pop_enter]: [Option<DestinationEnterTransition>; 2],
        [exit, pop_exit]: [Option<DestinationExitTransition>; 2],
        size_transform: Option<DestinationSizeTransform>,
        content: Content,
    ) {
        let navigator = self.provider().get_navigator_of::<ComposeNavigator>();
        let mut b = ComposeNavigatorDestinationBuilder::new(navigator, route, move |s, e| content(s, e));
        for a in arguments {
            b.argument_value(&a.name(), a.argument());
        }
        for d in deep_links {
            b.deep_link_value(d);
        }
        set_transitions(&mut b, enter, exit, pop_enter, pop_exit, size_transform);
        self.destination(b);
    }

    #[allow(clippy::too_many_arguments)]
    fn kompozable_impl_class(
        &mut self,
        route: RouteClass,
        type_map: TypeMap,
        deep_links: Vec<NavDeepLink>,
        [enter, pop_enter]: [Option<DestinationEnterTransition>; 2],
        [exit, pop_exit]: [Option<DestinationExitTransition>; 2],
        size_transform: Option<DestinationSizeTransform>,
        content: Content,
    ) {
        let navigator = self.provider().get_navigator_of::<ComposeNavigator>();
        let mut b =
            ComposeNavigatorDestinationBuilder::new_class(navigator, route, type_map, move |s, e| content(s, e));
        for d in deep_links {
            b.deep_link_value(d);
        }
        set_transitions(&mut b, enter, exit, pop_enter, pop_exit, size_transform);
        self.destination(b);
    }
}

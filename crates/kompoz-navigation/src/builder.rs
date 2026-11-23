//! Building graphs (Compose's `NavDestinationBuilder`, `NavGraphBuilder`, `navigation(…) { … }`,
//! `NavigatorProvider.navigation`, `NavController.createGraph`): a destination's route (or typed route), label,
//! arguments and deep links; a graph's destinations and start destination. A navigator's own builder (navigation-
//! compose's `composable`) makes its destination and has [`NavDestinationBuilder::build_with`] fill it.

use crate::argument::{NavArgument, NavArgumentBuilder, NavArguments};
use crate::controller::NavController;
use crate::deep_link::{NavDeepLink, NavDeepLinkBuilder, NavDeepLinkDslBuilder, nav_deep_link, nav_deep_link_of_with};
use crate::destination::{NavDestination, NavGraph};
use crate::navigator::{NavGraphNavigator, Navigator, NavigatorProvider};
use crate::serialization::{AnyRoute, Route, RouteClass, TypeMap, nav_arguments_of, route_pattern_of};
use std::rc::Rc;

/// What a graph's [`destination`](NavGraphBuilder::destination) takes: a builder of a destination (Kotlin's
/// `NavDestinationBuilder<D>` whose `build()` its subclasses override).
pub trait DestinationBuilder {
    /// The destination it builds.
    fn build_destination(self) -> NavDestination;
}

/// The builder of a destination (Compose's `NavDestinationBuilder`): see the module.
pub struct NavDestinationBuilder {
    navigator: Rc<dyn Navigator>,
    id: i32,
    route: Option<String>,
    type_map: Option<TypeMap>,
    label: Option<String>,
    arguments: NavArguments,
    deep_links: Vec<NavDeepLink>,
}

impl NavDestinationBuilder {
    /// A destination of `navigator` with `route` (Kotlin's `NavDestinationBuilder(navigator, route)`).
    pub fn new(navigator: Rc<dyn Navigator>, route: Option<String>) -> NavDestinationBuilder {
        NavDestinationBuilder {
            navigator,
            id: -1,
            route,
            type_map: None,
            label: None,
            arguments: NavArguments::new(),
            deep_links: Vec::new(),
        }
    }

    /// A destination of `navigator` with the id `id` (Kotlin's deprecated `NavDestinationBuilder(navigator, id)`).
    pub fn new_with_id(navigator: Rc<dyn Navigator>, id: i32) -> NavDestinationBuilder {
        let mut b = NavDestinationBuilder::new(navigator, None);
        b.id = id;
        b
    }

    /// `T`'s destination (Kotlin's `NavDestinationBuilder(navigator, T::class, typeMap)`): its route `T`'s
    /// pattern, its id `T`'s hash code, its arguments `T`'s fields.
    pub fn new_of<T: Route>(navigator: Rc<dyn Navigator>, type_map: TypeMap) -> NavDestinationBuilder {
        NavDestinationBuilder::new_class(navigator, Some(RouteClass::of::<T>()), type_map)
    }

    /// The same of a route class (none: no route, Kotlin's `null` KClass).
    pub fn new_class(
        navigator: Rc<dyn Navigator>,
        route: Option<RouteClass>,
        type_map: TypeMap,
    ) -> NavDestinationBuilder {
        let mut b =
            NavDestinationBuilder::new(navigator, route.map(|r| route_pattern_of(r.descriptor(), &type_map, None)));
        if let Some(r) = route {
            b.id = r.hash_code();
            for a in nav_arguments_of(r.descriptor(), &type_map) {
                b.arguments.insert(a.name(), a.argument());
            }
        }
        b.type_map = Some(type_map);
        b
    }

    /// Its navigator (Kotlin's `navigator`).
    pub fn navigator(&self) -> Rc<dyn Navigator> {
        self.navigator.clone()
    }

    /// Its route (Kotlin's `route`).
    pub fn route(&self) -> Option<String> {
        self.route.clone()
    }

    /// Its id (Kotlin's `id`; -1 for none).
    pub fn id(&self) -> i32 {
        self.id
    }

    /// Kotlin's `label`.
    pub fn label(&self) -> Option<String> {
        self.label.clone()
    }

    /// Kotlin's `label = …`.
    pub fn set_label(&mut self, label: Option<String>) {
        self.label = label;
    }

    /// Adds the argument `name` as `argument_builder` sets it (Kotlin's `argument(name) { … }`).
    pub fn argument(&mut self, name: &str, argument_builder: impl FnOnce(&mut NavArgumentBuilder)) {
        let mut b = NavArgumentBuilder::new();
        argument_builder(&mut b);
        self.arguments.insert(name, b.build());
    }

    /// Adds the argument `name` (Kotlin's `argument(name, argument)`).
    pub fn argument_value(&mut self, name: &str, argument: NavArgument) {
        self.arguments.insert(name, argument);
    }

    /// Adds a deep link of `uri_pattern` (Kotlin's `deepLink(uriPattern)`).
    pub fn deep_link(&mut self, uri_pattern: &str) {
        self.deep_links.push(NavDeepLinkBuilder::from_uri_pattern(uri_pattern).build());
    }

    /// Adds the deep link `nav_deep_link` sets (Kotlin's `deepLink { … }`).
    pub fn deep_link_with(&mut self, nav_deep_link_builder: impl FnOnce(&mut NavDeepLinkDslBuilder)) {
        self.deep_links.push(nav_deep_link(nav_deep_link_builder));
    }

    /// Adds `T`'s deep link under `base_path` (Kotlin's `deepLink<T>(basePath)`).
    ///
    /// Kotlin: `deepLink<T>(basePath: String)`
    pub fn deep_link_of<T: Route>(&mut self, base_path: &str) {
        self.deep_link_of_with::<T>(base_path, |_| {});
    }

    /// The same with what `nav_deep_link_builder` sets (Kotlin's `deepLink<T>(basePath) { … }`).
    ///
    /// Kotlin: `deepLink<T>(basePath: String, navDeepLink: NavDeepLinkDslBuilder.() -> Unit)`
    ///
    /// # Panics
    /// If the builder wasn't made for a typed route, or `T` has arguments the destination hasn't (of the same
    /// types).
    pub fn deep_link_of_with<T: Route>(
        &mut self,
        base_path: &str,
        nav_deep_link_builder: impl FnOnce(&mut NavDeepLinkDslBuilder),
    ) {
        let type_map = self.type_map.clone().unwrap_or_else(|| {
            panic!(
                "Cannot add deeplink from KClass [{}]. Use the NavDestinationBuilder constructor that takes a KClass \
                 with the same arguments.",
                std::any::type_name::<T>()
            )
        });
        for a in nav_arguments_of(T::descriptor(), &type_map) {
            let ok = self.arguments.get(&a.name()).is_some_and(|arg| arg.r#type() == a.argument().r#type());
            assert!(
                ok,
                "Cannot add deeplink from KClass [{}]. DeepLink contains unknown argument [{}]. Ensure deeplink \
                 arguments matches the destination's route from KClass",
                std::any::type_name::<T>(),
                a.name()
            );
        }
        self.deep_links.push(nav_deep_link_of_with::<T>(base_path, type_map, nav_deep_link_builder));
    }

    /// Adds `nav_deep_link` (Kotlin's `deepLink(navDeepLink)`).
    pub fn deep_link_value(&mut self, nav_deep_link: NavDeepLink) {
        self.deep_links.push(nav_deep_link);
    }

    /// The destination: its navigator's new one, filled (Kotlin's `build()`).
    pub fn build(self) -> NavDestination {
        let destination = self.navigator.create_destination();
        self.build_with(destination)
    }

    /// `destination` (a navigator's own, Kotlin's overridden `instantiateDestination`) filled: its label,
    /// arguments, deep links, route, id.
    pub fn build_with(self, destination: NavDestination) -> NavDestination {
        destination.set_label(self.label);
        for (name, argument) in self.arguments.iter() {
            destination.add_argument(name.clone(), argument.clone());
        }
        for deep_link in self.deep_links {
            destination.add_nav_deep_link(deep_link);
        }
        if let Some(route) = self.route {
            destination.set_route(Some(route));
        }
        if self.id != -1 {
            destination.set_id(self.id);
        }
        destination
    }
}

impl DestinationBuilder for NavDestinationBuilder {
    fn build_destination(self) -> NavDestination {
        self.build()
    }
}

/// A graph's start destination given as a typed route (Kotlin's `startDestination: KClass<*>` or `Any`): a
/// [`RouteClass`], or a typed route's value.
pub enum StartDestination {
    Class(RouteClass),
    Object(Rc<dyn AnyRoute>),
}

/// What converts to a [`StartDestination`]: a [`RouteClass`], a typed route's value.
pub trait IntoStartDestination {
    fn into_start_destination(self) -> StartDestination;
}

impl IntoStartDestination for RouteClass {
    fn into_start_destination(self) -> StartDestination {
        StartDestination::Class(self)
    }
}

impl<T: Route> IntoStartDestination for T {
    fn into_start_destination(self) -> StartDestination {
        StartDestination::Object(Rc::new(self))
    }
}

/// The builder of a graph (Compose's `NavGraphBuilder`): its destinations (added with [`destination`](Self::destination)
/// and a navigator's builders, nested graphs with [`navigation`](Self::navigation)) and its start destination.
pub struct NavGraphBuilder {
    base: NavDestinationBuilder,
    provider: NavigatorProvider,
    start_destination_id: i32,
    start_destination_route: Option<String>,
    start_destination: Option<StartDestination>,
    destinations: Vec<NavDestination>,
}

impl NavGraphBuilder {
    fn make(provider: &NavigatorProvider, base: NavDestinationBuilder) -> NavGraphBuilder {
        NavGraphBuilder {
            base,
            provider: provider.clone(),
            start_destination_id: 0,
            start_destination_route: None,
            start_destination: None,
            destinations: Vec::new(),
        }
    }

    fn graph_navigator(provider: &NavigatorProvider) -> Rc<dyn Navigator> {
        provider.get_navigator_of::<NavGraphNavigator>()
    }

    /// A graph of `route` starting at `start_destination` (Kotlin's `NavGraphBuilder(provider, startDestination,
    /// route)`).
    pub fn new(provider: &NavigatorProvider, start_destination: &str, route: Option<&str>) -> NavGraphBuilder {
        let base = NavDestinationBuilder::new(NavGraphBuilder::graph_navigator(provider), route.map(str::to_string));
        let mut b = NavGraphBuilder::make(provider, base);
        b.start_destination_route = Some(start_destination.to_string());
        b
    }

    /// A graph of the id `id` starting at the id `start_destination` (Kotlin's deprecated id form).
    pub fn new_with_ids(provider: &NavigatorProvider, id: i32, start_destination: i32) -> NavGraphBuilder {
        let base = NavDestinationBuilder::new_with_id(NavGraphBuilder::graph_navigator(provider), id);
        let mut b = NavGraphBuilder::make(provider, base);
        b.start_destination_id = start_destination;
        b
    }

    /// A graph of the typed route `route` starting at a typed route's destination (Kotlin's
    /// `NavGraphBuilder(provider, startDestination: KClass<*> or Any, route: KClass<*>?, typeMap)`).
    pub fn new_typed(
        provider: &NavigatorProvider,
        start_destination: impl IntoStartDestination,
        route: Option<RouteClass>,
        type_map: TypeMap,
    ) -> NavGraphBuilder {
        let base = NavDestinationBuilder::new_class(NavGraphBuilder::graph_navigator(provider), route, type_map);
        let mut b = NavGraphBuilder::make(provider, base);
        b.start_destination = Some(start_destination.into_start_destination());
        b
    }

    /// Its navigators (Kotlin's `provider`).
    pub fn provider(&self) -> NavigatorProvider {
        self.provider.clone()
    }

    /// Its own destination's builder (its label, arguments, deep links: Kotlin's inherited members).
    pub fn destination_builder(&mut self) -> &mut NavDestinationBuilder {
        &mut self.base
    }

    /// Adds the destination `navigator_destination` builds (Kotlin's `destination(navDestination)`).
    pub fn destination(&mut self, navigator_destination: impl DestinationBuilder) {
        self.destinations.push(navigator_destination.build_destination());
    }

    /// Adds `destination` (Kotlin's `addDestination`, `+destination`).
    pub fn add_destination(&mut self, destination: NavDestination) {
        self.destinations.push(destination);
    }

    /// Adds a nested graph of `route` starting at `start_destination` (Compose's `navigation(startDestination,
    /// route) { … }`).
    pub fn navigation(&mut self, start_destination: &str, route: &str, builder: impl FnOnce(&mut NavGraphBuilder)) {
        let mut nested = NavGraphBuilder::new(&self.provider, start_destination, Some(route));
        builder(&mut nested);
        self.destination(nested);
    }

    /// Adds a nested graph of `T`'s route starting at a typed route's destination (Compose's
    /// `navigation<T>(startDestination = Start) { … }`).
    ///
    /// Kotlin: `navigation<T>(startDestination: Any, typeMap: Map<KType, NavType<*>> = emptyMap(), builder: NavGraphBuilder.() -> Unit)`
    pub fn navigation_of<T: Route>(
        &mut self,
        start_destination: impl IntoStartDestination,
        type_map: TypeMap,
        builder: impl FnOnce(&mut NavGraphBuilder),
    ) {
        let mut nested =
            NavGraphBuilder::new_typed(&self.provider, start_destination, Some(RouteClass::of::<T>()), type_map);
        builder(&mut nested);
        self.destination(nested);
    }

    /// The graph (Kotlin's `build()`).
    ///
    /// # Panics
    /// Without a start destination.
    pub fn build(self) -> NavGraph {
        let graph = self.base.build().as_graph().expect("the graph navigator's destination is a graph");
        graph.add_destinations(self.destinations);
        if self.start_destination_id == 0 && self.start_destination_route.is_none() && self.start_destination.is_none()
        {
            if graph.route().is_some() {
                panic!("You must set a start destination route");
            } else {
                panic!("You must set a start destination id");
            }
        }
        if let Some(route) = self.start_destination_route {
            graph.set_start_destination(route);
        } else if let Some(start) = self.start_destination {
            match start {
                StartDestination::Class(c) => graph.set_start_destination_class(c),
                StartDestination::Object(o) => graph.set_start_destination_object(&*o),
            }
        } else {
            graph.set_start_destination_id(self.start_destination_id);
        }
        graph
    }
}

impl DestinationBuilder for NavGraphBuilder {
    fn build_destination(self) -> NavDestination {
        self.build().as_destination()
    }
}

impl NavigatorProvider {
    /// A graph of `route` starting at `start_destination`, `builder` adding its destinations (Compose's
    /// `NavigatorProvider.navigation(startDestination, route) { … }`).
    pub fn navigation(
        &self,
        start_destination: &str,
        route: Option<&str>,
        builder: impl FnOnce(&mut NavGraphBuilder),
    ) -> NavGraph {
        let mut b = NavGraphBuilder::new(self, start_destination, route);
        builder(&mut b);
        b.build()
    }

    /// The same of typed routes (Compose's `navigation(startDestination: KClass or Any, route: KClass?, typeMap)`).
    pub fn navigation_typed(
        &self,
        start_destination: impl IntoStartDestination,
        route: Option<RouteClass>,
        type_map: TypeMap,
        builder: impl FnOnce(&mut NavGraphBuilder),
    ) -> NavGraph {
        let mut b = NavGraphBuilder::new_typed(self, start_destination, route, type_map);
        builder(&mut b);
        b.build()
    }
}

impl NavController {
    /// A graph of its navigators (Compose's `NavController.createGraph(startDestination, route) { … }`).
    pub fn create_graph(
        &self,
        start_destination: &str,
        route: Option<&str>,
        builder: impl FnOnce(&mut NavGraphBuilder),
    ) -> NavGraph {
        self.navigator_provider().navigation(start_destination, route, builder)
    }

    /// The same of typed routes (Compose's `createGraph(startDestination: KClass or Any, route: KClass?,
    /// typeMap)`).
    pub fn create_graph_typed(
        &self,
        start_destination: impl IntoStartDestination,
        route: Option<RouteClass>,
        type_map: TypeMap,
        builder: impl FnOnce(&mut NavGraphBuilder),
    ) -> NavGraph {
        self.navigator_provider().navigation_typed(start_destination, route, type_map, builder)
    }
}

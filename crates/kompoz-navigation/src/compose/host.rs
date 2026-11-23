//! The NavHost (navigation-compose's `NavHost` overloads, `DefaultNavTransitions`, `NavHostEventHandler` and
//! `NavBackStackEntryInfo`, `rememberNavController`, `currentBackStackEntryAsState`): the top composable entry of
//! the controller's back stack shown in an `AnimatedContent` of a `SeekableTransitionState`, entries coming in over
//! the one before going forward and under it going back (Compose's z-indices), back popping it, a predictive back
//! gesture seeking the pop transition; the dialog destinations over it in a [`DialogHost`].
//!
//! Rust has no overloads: Kotlin's `NavHost(navController, startDestination: String, …)` is [`NavHost`], its
//! `startDestination: Any` / `KClass<*>` (a typed route) forms [`NavHost_of`], its `graph: NavGraph` form
//! [`NavHost_graph`]. The pop transitions left out are the enter (exit) one given, as Kotlin's defaults.

use super::dialog::{DialogHost, DialogNavigator};
use super::navigator::{ComposeNavGraphNavigator, ComposeNavigator, ComposeNavigatorDestination, NavTransitions};
use crate::builder::{IntoStartDestination, NavGraphBuilder};
use crate::controller::{NavController, NavHostController};
use crate::destination::{NavDestination, NavGraph};
use crate::entry::NavBackStackEntry;
use crate::navigator::Navigator;
use crate::saved_state::SavedState;
use crate::serialization::{RouteClass, TypeMap};
use kompoz_animation::{
    AnimatedContentScope, AnimatedContentTransitionScope, ContentTransform, EnterTransition, ExitTransition,
    SeekableTransitionState, SizeTransform, TweenSpec, animate, fade_in, fade_out, remember_transition, scale_out,
    spring, tween,
};
use kompoz_coroutines::{AsFlow, LaunchedEffect, coroutine_scope};
use kompoz_lifecycle::{LifecycleStartEffect, LocalLifecycleOwner, LocalViewModelStoreOwner};
use kompoz_navigationevent::{
    LocalNavigationEventDispatcherOwner, NavigationEvent, NavigationEventHandler, NavigationEventHandlerCallbacks,
    NavigationEventInfo,
};
use kompoz_runtime::{
    DisposableEffect, MutableState, SavedValue, Saver, SideEffect, State, derived_state_of, kompozable,
    mutable_state_of, named_args, on_dispose, remember, remember_keyed, remember_saveable_keyed_with,
    remember_saveable_state_holder,
};
use kompoz_ui::layout::SlotKey;
use kompoz_ui::platform::{LocalDeepLinks, LocalPlatform};
use kompoz_ui::{Alignment, Modifier};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// A NavHost's enter transition (Kotlin's `AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition`).
pub type NavHostEnterTransition = Rc<dyn Fn(&AnimatedContentTransitionScope<NavBackStackEntry>) -> EnterTransition>;
/// A NavHost's exit transition (Kotlin's `AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition`).
pub type NavHostExitTransition = Rc<dyn Fn(&AnimatedContentTransitionScope<NavBackStackEntry>) -> ExitTransition>;
/// A NavHost's size transform (Kotlin's `AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?`).
pub type NavHostSizeTransform =
    Rc<dyn Fn(&AnimatedContentTransitionScope<NavBackStackEntry>) -> Option<Rc<dyn SizeTransform>>>;

// ---- DefaultNavTransitions ----

/// A NavHost's transitions unless told otherwise (Compose's `DefaultNavTransitions`, its Android actual — the
/// desktop's is the same —): a 700 ms cross-fade, the pop ones the same; during a predictive back gesture the one
/// coming in fades in (Material's default effects spring), the one leaving scales out to 0.7; no size transform.
pub struct DefaultNavTransitions;

impl DefaultNavTransitions {
    /// Kotlin: `enterTransition`
    pub fn enter_transition(_: &AnimatedContentTransitionScope<NavBackStackEntry>) -> EnterTransition {
        let spec: TweenSpec<f32> = named_args!(tween(700));
        named_args!(fade_in(animation_spec = Arc::new(spec)))
    }

    /// Kotlin: `exitTransition`
    pub fn exit_transition(_: &AnimatedContentTransitionScope<NavBackStackEntry>) -> ExitTransition {
        let spec: TweenSpec<f32> = named_args!(tween(700));
        named_args!(fade_out(animation_spec = Arc::new(spec)))
    }

    /// The pop enter transition of `enter_transition`: itself (Kotlin's `popEnterTransition(enterTransition)`).
    pub fn pop_enter_transition(enter_transition: NavHostEnterTransition) -> NavHostEnterTransition {
        enter_transition
    }

    /// The pop exit transition of `exit_transition`: itself (Kotlin's `popExitTransition(exitTransition)`).
    pub fn pop_exit_transition(exit_transition: NavHostExitTransition) -> NavHostExitTransition {
        exit_transition
    }

    /// Kotlin: `predictivePopEnterTransition`
    pub fn predictive_pop_enter_transition(
        _: &AnimatedContentTransitionScope<NavBackStackEntry>,
        _swipe_edge: i32,
    ) -> EnterTransition {
        // (material3's motionScheme.defaultEffectsSpec())
        let spec = {
            let s: kompoz_animation::SpringSpec<f32> = named_args!(spring(damping_ratio = 1.0, stiffness = 1600.0));
            s
        };
        named_args!(fade_in(animation_spec = Arc::new(spec)))
    }

    /// Kotlin: `predictivePopExitTransition`
    pub fn predictive_pop_exit_transition(
        _: &AnimatedContentTransitionScope<NavBackStackEntry>,
        _swipe_edge: i32,
    ) -> ExitTransition {
        named_args!(scale_out(target_scale = 0.7))
    }

    /// None (Kotlin's `sizeTransform`, null).
    pub fn size_transform() -> Option<NavHostSizeTransform> {
        None
    }
}

/// The transitions a NavHost was given (its defaults filled).
struct HostTransitions {
    enter: NavHostEnterTransition,
    exit: NavHostExitTransition,
    pop_enter: NavHostEnterTransition,
    pop_exit: NavHostExitTransition,
    predictive_pop_enter: Rc<dyn Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> EnterTransition>,
    predictive_pop_exit: Rc<dyn Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> ExitTransition>,
    size_transform: Option<NavHostSizeTransform>,
}

#[allow(clippy::too_many_arguments)]
fn host_transitions(
    enter_transition: impl Fn(&AnimatedContentTransitionScope<NavBackStackEntry>) -> EnterTransition + 'static,
    exit_transition: impl Fn(&AnimatedContentTransitionScope<NavBackStackEntry>) -> ExitTransition + 'static,
    pop_enter_transition: Option<NavHostEnterTransition>,
    pop_exit_transition: Option<NavHostExitTransition>,
    predictive_pop_enter_transition: impl Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> EnterTransition
    + 'static,
    predictive_pop_exit_transition: impl Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> ExitTransition
    + 'static,
    size_transform: Option<NavHostSizeTransform>,
) -> Rc<HostTransitions> {
    let enter: NavHostEnterTransition = Rc::new(enter_transition);
    let exit: NavHostExitTransition = Rc::new(exit_transition);
    Rc::new(HostTransitions {
        pop_enter: pop_enter_transition.unwrap_or_else(|| DefaultNavTransitions::pop_enter_transition(enter.clone())),
        pop_exit: pop_exit_transition.unwrap_or_else(|| DefaultNavTransitions::pop_exit_transition(exit.clone())),
        enter,
        exit,
        predictive_pop_enter: Rc::new(predictive_pop_enter_transition),
        predictive_pop_exit: Rc::new(predictive_pop_exit_transition),
        size_transform,
    })
}

// ---- NavBackStackEntryInfo, NavHostEventHandler ----

/// The navigation event info of a NavHost's entry (Compose's `NavBackStackEntryInfo`): what its back handler tells
/// the dispatcher's history.
#[derive(Clone, PartialEq)]
pub struct NavBackStackEntryInfo {
    /// The entry (Kotlin's `visibleEntry`).
    pub visible_entry: Option<NavBackStackEntry>,
}

/// Kotlin's `NavBackStackEntryInfo(visibleEntry = null)`.
#[allow(non_snake_case)]
pub fn NavBackStackEntryInfo(visible_entry: Option<NavBackStackEntry>) -> NavBackStackEntryInfo {
    NavBackStackEntryInfo { visible_entry }
}

impl fmt::Debug for NavBackStackEntryInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NavBackStackEntryInfo(visibleEntry={:?})", self.visible_entry)
    }
}

impl NavigationEventInfo for NavBackStackEntryInfo {}

/// The states of a NavHost's back handler (Kotlin's `NavHostEventHandler`'s fields).
struct HandlerStates {
    navigator: Rc<ComposeNavigator>,
    current_back_stack_entry: RefCell<Option<NavBackStackEntry>>,
    in_predictive_back: MutableState<bool>,
    progress: MutableState<f32>,
    swipe_edge: MutableState<i32>,
}

impl HandlerStates {
    fn prepare_transition(&self) {
        let back_stack = self.navigator.back_stack().value();
        if back_stack.len() < 2 {
            return;
        }
        let current_entry = back_stack[back_stack.len() - 1].clone();
        let previous_entry = back_stack[back_stack.len() - 2].clone();
        self.navigator.prepare_for_transition(&current_entry);
        self.navigator.prepare_for_transition(&previous_entry);
        *self.current_back_stack_entry.borrow_mut() = Some(current_entry);
    }

    fn reset_state(&self) {
        self.in_predictive_back.set(false);
        self.progress.set(0.0);
        self.swipe_edge.set(NavigationEvent::EDGE_NONE);
        *self.current_back_stack_entry.borrow_mut() = None;
    }
}

/// The callbacks (Kotlin's overrides).
struct HandlerCallbacks(Rc<HandlerStates>);

impl NavigationEventHandlerCallbacks<NavBackStackEntryInfo> for HandlerCallbacks {
    fn on_back_started(&self, _: &NavigationEventHandler<NavBackStackEntryInfo>, event: NavigationEvent) {
        self.0.prepare_transition();
        self.0.in_predictive_back.set(true);
        self.0.progress.set(event.progress);
        self.0.swipe_edge.set(event.swipe_edge);
    }

    fn on_back_progressed(&self, _: &NavigationEventHandler<NavBackStackEntryInfo>, event: NavigationEvent) {
        self.0.in_predictive_back.set(true);
        self.0.progress.set(event.progress);
        self.0.swipe_edge.set(event.swipe_edge);
    }

    fn on_back_completed(&self, _: &NavigationEventHandler<NavBackStackEntryInfo>) {
        if !self.0.in_predictive_back.peek() {
            self.0.prepare_transition();
        }
        let entry = self.0.current_back_stack_entry.borrow().clone();
        if let Some(entry) = entry {
            self.0.navigator.pop_back_stack_to(&entry, false);
        }
        self.0.reset_state();
    }

    fn on_back_cancelled(&self, _: &NavigationEventHandler<NavBackStackEntryInfo>) {
        self.0.reset_state();
    }
}

/// A NavHost's back handler (Compose's internal `NavHostEventHandler`): back pops its navigator's top entry; a
/// predictive back gesture's progress and swipe edge are states the NavHost seeks its transition by.
#[derive(Clone)]
struct NavHostEventHandler {
    handler: NavigationEventHandler<NavBackStackEntryInfo>,
    states: Rc<HandlerStates>,
}

impl PartialEq for NavHostEventHandler {
    fn eq(&self, other: &Self) -> bool {
        self.handler == other.handler
    }
}

impl NavHostEventHandler {
    fn new(navigator: Rc<ComposeNavigator>) -> NavHostEventHandler {
        let initial = NavBackStackEntryInfo(navigator.back_stack().value().last().cloned());
        let states = Rc::new(HandlerStates {
            navigator,
            current_back_stack_entry: RefCell::new(None),
            in_predictive_back: mutable_state_of(false),
            progress: mutable_state_of(0.0),
            swipe_edge: mutable_state_of(NavigationEvent::EDGE_NONE),
        });
        let handler = NavigationEventHandler::with_forward(initial, true, false, HandlerCallbacks(states.clone()));
        NavHostEventHandler { handler, states }
    }
}

/// The same navigator (a key by identity).
#[derive(Clone)]
struct SameNavigator(Rc<ComposeNavigator>);

impl PartialEq for SameNavigator {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Compose's internal `rememberNavHostEventHandler`: the handler of `navigator`, added to the nearest dispatcher
/// while composed, back enabled while `enabled` and the lifecycle started.
#[kompozable]
fn remember_nav_host_event_handler(navigator: Rc<ComposeNavigator>, enabled: bool) -> NavHostEventHandler {
    let dispatcher = LocalNavigationEventDispatcherOwner
        .current()
        .unwrap_or_else(|| panic!("No NavigationEventDispatcher was provided via LocalNavigationEventDispatcherOwner"))
        .navigation_event_dispatcher();
    let n = navigator.clone();
    let handler = remember_keyed(SameNavigator(navigator), move || NavHostEventHandler::new(n));
    let h = handler.clone();
    LifecycleStartEffect((enabled, handler.clone()), move |scope| {
        h.handler.set_is_back_enabled(enabled);
        let h = h.clone();
        scope.on_stop_or_dispose(move |_| h.handler.set_is_back_enabled(false))
    });
    let h = handler.clone();
    DisposableEffect((dispatcher.clone(), handler.clone()), move || {
        dispatcher.add_handler(&h.handler);
        on_dispose(move || h.handler.remove())
    });
    handler
}

// ---- NavHost ----

/// The first of `destination`'s hierarchy (it, then the graphs it is in) giving a transition, by `pick`.
fn first_in_hierarchy<R>(destination: &NavDestination, pick: impl Fn(&NavTransitions) -> Option<R>) -> Option<R> {
    destination.hierarchy().iter().find_map(|d| NavTransitions::of(d).and_then(|t| pick(&t)))
}

/// A composable entry's path: its route with its arguments' values in (what C++ Kompoz's NavHost told the platform
/// as the screen shown changed — the web's address; a query parameter of no value is left out).
fn path_of(entry: &NavBackStackEntry) -> String {
    let route = entry.destination().route().unwrap_or_default();
    let args = entry.arguments().map(|a| a.to_saved_value());
    let value_of = |name: &str| -> Option<String> {
        match args.as_ref()?.as_map()?.get(name)? {
            SavedValue::String(s) => Some(s.clone()),
            SavedValue::Int(n) => Some(n.to_string()),
            SavedValue::Double(d) => Some(d.to_string()),
            SavedValue::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    };
    let fill = |part: &str| -> Option<String> {
        let mut out = String::new();
        let mut rest = part;
        while let Some(open) = rest.find('{') {
            let close = rest[open..].find('}')? + open;
            out.push_str(&rest[..open]);
            out.push_str(&crate::uri::encode(&value_of(&rest[open + 1..close])?, None));
            rest = &rest[close + 1..];
        }
        out.push_str(rest);
        Some(out)
    };
    let (path, query) = match route.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (route.as_str(), None),
    };
    let mut out = fill(path).unwrap_or_else(|| path.to_string());
    let params: Vec<String> = query.map_or(Vec::new(), |q| q.split('&').filter_map(fill).collect());
    if !params.is_empty() {
        out.push('?');
        out.push_str(&params.join("&"));
    }
    out
}

/// Shows the destinations of `graph` (a place in the composition for self-contained navigation; Compose's
/// `NavHost(navController, graph, …)`): the top composable entry of `nav_controller`'s back stack in an
/// `AnimatedContent` aligned by `content_alignment`, entries coming in by `enter_transition` and the one before
/// leaving by `exit_transition` (popping: `pop_enter_transition` and `pop_exit_transition`, by default the enter
/// and exit ones; during a predictive back gesture the predictive ones, seeked as far as the gesture has gone), a
/// destination's own transitions (or its graphs') first; its size by `size_transform`; dialog destinations over it.
/// Back pops while there is an entry to pop. The entries are the content's lifecycle, ViewModel store and saved
/// state registry owners, capped by the NavHost's lifecycle; their `rememberSaveable` values are kept while they
/// are on the back stack (or saved).
///
/// Kotlin: `NavHost(navController: NavHostController, graph: NavGraph, modifier: Modifier = Modifier, contentAlignment: Alignment = Alignment.TopStart, enterTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition = DefaultNavTransitions.enterTransition, exitTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition = DefaultNavTransitions.exitTransition, popEnterTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition = DefaultNavTransitions.popEnterTransition(enterTransition), popExitTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition = DefaultNavTransitions.popExitTransition(exitTransition), predictivePopEnterTransition: AnimatedContentTransitionScope<NavBackStackEntry>.(Int) -> EnterTransition = DefaultNavTransitions.predictivePopEnterTransition, predictivePopExitTransition: AnimatedContentTransitionScope<NavBackStackEntry>.(Int) -> ExitTransition = DefaultNavTransitions.predictivePopExitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = DefaultNavTransitions.sizeTransform)`
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub fn NavHost_graph(
    nav_controller: NavController,
    graph: NavGraph,
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::TopStart)] content_alignment: Alignment,
    #[default(DefaultNavTransitions::enter_transition)] enter_transition: impl Fn(
        &AnimatedContentTransitionScope<NavBackStackEntry>,
    ) -> EnterTransition
    + 'static,
    #[default(DefaultNavTransitions::exit_transition)] exit_transition: impl Fn(
        &AnimatedContentTransitionScope<NavBackStackEntry>,
    ) -> ExitTransition
    + 'static,
    #[default(None)] pop_enter_transition: Option<NavHostEnterTransition>,
    #[default(None)] pop_exit_transition: Option<NavHostExitTransition>,
    #[default(DefaultNavTransitions::predictive_pop_enter_transition)]
    predictive_pop_enter_transition: impl Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> EnterTransition
    + 'static,
    #[default(DefaultNavTransitions::predictive_pop_exit_transition)]
    predictive_pop_exit_transition: impl Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> ExitTransition
    + 'static,
    #[default(DefaultNavTransitions::size_transform())] size_transform: Option<NavHostSizeTransform>,
) {
    let transitions = host_transitions(
        enter_transition,
        exit_transition,
        pop_enter_transition,
        pop_exit_transition,
        predictive_pop_enter_transition,
        predictive_pop_exit_transition,
        size_transform,
    );
    nav_host_impl(nav_controller, graph, modifier, content_alignment, transitions);
}

/// Shows a graph of the destinations `builder` adds, starting at the route `start_destination` (Compose's
/// `NavHost(navController, startDestination, …) { … }`; the graph's route `route`): see [`NavHost_graph`].
///
/// Kotlin: `NavHost(navController: NavHostController, startDestination: String, modifier: Modifier = Modifier, contentAlignment: Alignment = Alignment.TopStart, route: String? = null, enterTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition = DefaultNavTransitions.enterTransition, exitTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition = DefaultNavTransitions.exitTransition, popEnterTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition = DefaultNavTransitions.popEnterTransition(enterTransition), popExitTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition = DefaultNavTransitions.popExitTransition(exitTransition), predictivePopEnterTransition: AnimatedContentTransitionScope<NavBackStackEntry>.(Int) -> EnterTransition = DefaultNavTransitions.predictivePopEnterTransition, predictivePopExitTransition: AnimatedContentTransitionScope<NavBackStackEntry>.(Int) -> ExitTransition = DefaultNavTransitions.predictivePopExitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = DefaultNavTransitions.sizeTransform, builder: NavGraphBuilder.() -> Unit)`
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub fn NavHost(
    nav_controller: NavController,
    start_destination: &str,
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::TopStart)] content_alignment: Alignment,
    #[default(None)] route: Option<String>,
    #[default(DefaultNavTransitions::enter_transition)] enter_transition: impl Fn(
        &AnimatedContentTransitionScope<NavBackStackEntry>,
    ) -> EnterTransition
    + 'static,
    #[default(DefaultNavTransitions::exit_transition)] exit_transition: impl Fn(
        &AnimatedContentTransitionScope<NavBackStackEntry>,
    ) -> ExitTransition
    + 'static,
    #[default(None)] pop_enter_transition: Option<NavHostEnterTransition>,
    #[default(None)] pop_exit_transition: Option<NavHostExitTransition>,
    #[default(DefaultNavTransitions::predictive_pop_enter_transition)]
    predictive_pop_enter_transition: impl Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> EnterTransition
    + 'static,
    #[default(DefaultNavTransitions::predictive_pop_exit_transition)]
    predictive_pop_exit_transition: impl Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> ExitTransition
    + 'static,
    #[default(DefaultNavTransitions::size_transform())] size_transform: Option<NavHostSizeTransform>,
    builder: impl Fn(&mut NavGraphBuilder),
) {
    // (Kotlin's `remember(route, startDestination, builder)`: a builder lambda is a new one at each composition
    // here — closures don't compare —, the graph made again, which the controller takes as the same graph)
    let graph = nav_controller.create_graph(start_destination, route.as_deref(), builder);
    let transitions = host_transitions(
        enter_transition,
        exit_transition,
        pop_enter_transition,
        pop_exit_transition,
        predictive_pop_enter_transition,
        predictive_pop_exit_transition,
        size_transform,
    );
    nav_host_impl(nav_controller, graph, modifier, content_alignment, transitions);
}

/// Shows a graph of the destinations `builder` adds, starting at a typed route's destination — its value, or a
/// [`RouteClass`] — (Compose's `NavHost(navController, startDestination = Home, …) { … }`; the graph's typed route
/// `route`, of the types `type_map` adds): see [`NavHost_graph`].
///
/// Kotlin: `NavHost(navController: NavHostController, startDestination: Any, modifier: Modifier = Modifier, contentAlignment: Alignment = Alignment.TopStart, route: KClass<*>? = null, typeMap: Map<KType, NavType<*>> = emptyMap(), enterTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition = DefaultNavTransitions.enterTransition, exitTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition = DefaultNavTransitions.exitTransition, popEnterTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> EnterTransition = DefaultNavTransitions.popEnterTransition(enterTransition), popExitTransition: AnimatedContentTransitionScope<NavBackStackEntry>.() -> ExitTransition = DefaultNavTransitions.popExitTransition(exitTransition), predictivePopEnterTransition: AnimatedContentTransitionScope<NavBackStackEntry>.(Int) -> EnterTransition = DefaultNavTransitions.predictivePopEnterTransition, predictivePopExitTransition: AnimatedContentTransitionScope<NavBackStackEntry>.(Int) -> ExitTransition = DefaultNavTransitions.predictivePopExitTransition, sizeTransform: (AnimatedContentTransitionScope<NavBackStackEntry>.() -> SizeTransform?)? = DefaultNavTransitions.sizeTransform, builder: NavGraphBuilder.() -> Unit)`
#[kompozable(inline)]
#[allow(clippy::too_many_arguments)]
pub fn NavHost_of(
    nav_controller: NavController,
    start_destination: impl IntoStartDestination,
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::TopStart)] content_alignment: Alignment,
    #[default(None)] route: Option<RouteClass>,
    #[default(TypeMap::default())] type_map: TypeMap,
    #[default(DefaultNavTransitions::enter_transition)] enter_transition: impl Fn(
        &AnimatedContentTransitionScope<NavBackStackEntry>,
    ) -> EnterTransition
    + 'static,
    #[default(DefaultNavTransitions::exit_transition)] exit_transition: impl Fn(
        &AnimatedContentTransitionScope<NavBackStackEntry>,
    ) -> ExitTransition
    + 'static,
    #[default(None)] pop_enter_transition: Option<NavHostEnterTransition>,
    #[default(None)] pop_exit_transition: Option<NavHostExitTransition>,
    #[default(DefaultNavTransitions::predictive_pop_enter_transition)]
    predictive_pop_enter_transition: impl Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> EnterTransition
    + 'static,
    #[default(DefaultNavTransitions::predictive_pop_exit_transition)]
    predictive_pop_exit_transition: impl Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> ExitTransition
    + 'static,
    #[default(DefaultNavTransitions::size_transform())] size_transform: Option<NavHostSizeTransform>,
    builder: impl Fn(&mut NavGraphBuilder),
) {
    let graph = nav_controller.create_graph_typed(start_destination, route, type_map, builder);
    let transitions = host_transitions(
        enter_transition,
        exit_transition,
        pop_enter_transition,
        pop_exit_transition,
        predictive_pop_enter_transition,
        predictive_pop_exit_transition,
        size_transform,
    );
    nav_host_impl(nav_controller, graph, modifier, content_alignment, transitions);
}

/// `navigator` as a navigator of type `N`.
fn downcast_navigator<N: Navigator>(navigator: Rc<dyn Navigator>) -> Option<Rc<N>> {
    let any: Rc<dyn Any> = navigator;
    any.downcast::<N>().ok()
}

/// NavHost's body (Kotlin's last `NavHost(navController, graph, …)`).
#[kompozable]
fn nav_host_impl(
    nav_controller: NavController,
    graph: NavGraph,
    modifier: Modifier,
    content_alignment: Alignment,
    transitions: Rc<HostTransitions>,
) {
    let lifecycle_owner = LocalLifecycleOwner.current();
    let view_model_store_owner = LocalViewModelStoreOwner.current().unwrap_or_else(|| {
        panic!("NavHost requires a ViewModelStoreOwner to be provided via LocalViewModelStoreOwner")
    });

    nav_controller.set_view_model_store(view_model_store_owner.view_model_store());

    // Then set the graph
    nav_controller.set_graph(graph);

    // Find the ComposeNavigator, returning early if it isn't found (such as is the case when using
    // TestNavHostController)
    let navigator = nav_controller.navigator_provider().get_navigator(ComposeNavigator::NAME);
    if let Some(compose_navigator) = downcast_navigator::<ComposeNavigator>(navigator) {
        nav_host_content(
            nav_controller.clone(),
            compose_navigator,
            lifecycle_owner,
            modifier,
            content_alignment,
            transitions,
        );
        let dialog = nav_controller.navigator_provider().get_navigator(DialogNavigator::NAME);
        if let Some(dialog_navigator) = downcast_navigator::<DialogNavigator>(dialog) {
            // Show any dialog destinations
            DialogHost(dialog_navigator);
        }
    }
}

#[kompozable]
fn nav_host_content(
    nav_controller: NavController,
    compose_navigator: Rc<ComposeNavigator>,
    lifecycle_owner: Rc<dyn kompoz_lifecycle::LifecycleOwner>,
    modifier: Modifier,
    content_alignment: Alignment,
    transitions: Rc<HostTransitions>,
) {
    let current_back_stack = compose_navigator.back_stack().collect_as_confined_state().get();

    let back_handler = remember_nav_host_event_handler(compose_navigator.clone(), current_back_stack.len() > 1);
    let in_predictive_back = back_handler.states.in_predictive_back.get();
    let progress = back_handler.states.progress.get();
    let swipe_edge = back_handler.states.swipe_edge.get();

    let nav = nav_controller.clone();
    DisposableEffect(lifecycle_owner.clone(), move || {
        // Setup the navController with proper owners
        nav.set_lifecycle_owner(lifecycle_owner);
        on_dispose(|| {})
    });

    let (h, stack) = (back_handler.clone(), current_back_stack.clone());
    SideEffect(move || {
        h.handler.set_is_back_enabled(stack.len() > 1);
        let back_info: Vec<NavBackStackEntryInfo> =
            stack.iter().rev().skip(1).map(|e| NavBackStackEntryInfo(Some(e.clone()))).collect();
        h.handler.set_info(NavBackStackEntryInfo(stack.last().cloned()), back_info, Vec::new());
    });

    let saveable_state_holder = remember_saveable_state_holder();

    let all_visible_entries: State<Vec<NavBackStackEntry>> =
        nav_controller.visible_entries().collect_as_confined_state();

    // Intercept back only when there's a destination to pop
    let all = all_visible_entries.clone();
    let visible_entries = remember(move || {
        derived_state_of(move || {
            all.get()
                .into_iter()
                .filter(|entry| entry.destination().navigator_name() == ComposeNavigator::NAME)
                .collect::<Vec<_>>()
        })
    });

    let back_stack_entry: Option<NavBackStackEntry> = visible_entries.get().last().cloned();

    // (C++ Kompoz's: the platform told the screen shown, as its path — the web's address)
    if let Some(top) = current_back_stack.last() {
        let path = path_of(top);
        let platform = LocalPlatform.current();
        let reported = remember(|| Rc::new(RefCell::new(None::<String>)));
        SideEffect(move || {
            if reported.borrow().as_deref() != Some(path.as_str()) {
                *reported.borrow_mut() = Some(path.clone());
                platform.get().location_changed(&path);
            }
        });
    }

    let z_indices = remember(|| Rc::new(RefCell::new(HashMap::<String, f32>::new())));

    if let Some(back_stack_entry) = back_stack_entry {
        let (t, nav) = (transitions.clone(), compose_navigator.clone());
        let final_enter =
            Rc::new(move |scope: &AnimatedContentTransitionScope<NavBackStackEntry>| -> EnterTransition {
                let target_destination = scope.target_state().destination();
                if in_predictive_back {
                    first_in_hierarchy(&target_destination, |d| {
                        d.predictive_pop_enter_transition.borrow().clone().and_then(|f| f(scope, swipe_edge))
                    })
                    .unwrap_or_else(|| (t.predictive_pop_enter)(scope, swipe_edge))
                } else if nav.is_pop() {
                    first_in_hierarchy(&target_destination, |d| {
                        d.pop_enter_transition.borrow().clone().and_then(|f| f(scope))
                    })
                    .unwrap_or_else(|| (t.pop_enter)(scope))
                } else {
                    first_in_hierarchy(&target_destination, |d| {
                        d.enter_transition.borrow().clone().and_then(|f| f(scope))
                    })
                    .unwrap_or_else(|| (t.enter)(scope))
                }
            });

        let (t, nav) = (transitions.clone(), compose_navigator.clone());
        let final_exit = Rc::new(move |scope: &AnimatedContentTransitionScope<NavBackStackEntry>| -> ExitTransition {
            let initial_destination = scope.initial_state().destination();
            if in_predictive_back {
                first_in_hierarchy(&initial_destination, |d| {
                    d.predictive_pop_exit_transition.borrow().clone().and_then(|f| f(scope, swipe_edge))
                })
                .unwrap_or_else(|| (t.predictive_pop_exit)(scope, swipe_edge))
            } else if nav.is_pop() {
                first_in_hierarchy(&initial_destination, |d| {
                    d.pop_exit_transition.borrow().clone().and_then(|f| f(scope))
                })
                .unwrap_or_else(|| (t.pop_exit)(scope))
            } else {
                first_in_hierarchy(&initial_destination, |d| d.exit_transition.borrow().clone().and_then(|f| f(scope)))
                    .unwrap_or_else(|| (t.exit)(scope))
            }
        });

        let t = transitions.clone();
        let final_size_transform = Rc::new(
            move |scope: &AnimatedContentTransitionScope<NavBackStackEntry>| -> Option<Rc<dyn SizeTransform>> {
                let target_destination = scope.target_state().destination();
                first_in_hierarchy(&target_destination, |d| d.size_transform.borrow().clone().and_then(|f| f(scope)))
                    .or_else(|| t.size_transform.as_ref().and_then(|f| f(scope)))
            },
        );

        let (visible, nav) = (visible_entries.clone(), compose_navigator.clone());
        DisposableEffect(true, move || {
            on_dispose(move || {
                for entry in visible.get() {
                    nav.on_transition_complete(&entry);
                }
            })
        });

        // The state returned here cannot be nullable cause it produces the input of the transitionSpec passed into
        // the AnimatedContent and that must match the non-nullable scope exposed by the transitions on the NavHost
        // and composable APIs.
        let first = back_stack_entry.clone();
        let transition_state = remember(move || SeekableTransitionState::new(first));

        let transition = remember_transition(transition_state.clone(), label = Some("entry".to_string()));

        if in_predictive_back {
            let (ts, stack) = (transition_state.clone(), current_back_stack.clone());
            LaunchedEffect(progress, async move {
                // Update transition progress safely (same guard against stale enabled state).
                if stack.len() > 1 {
                    let previous_entry = stack[stack.len() - 2].clone();
                    named_args!(ts.seek_to(progress, target_state = previous_entry)).await;
                }
            });
        } else {
            let (ts, entry, tr) = (transition_state.clone(), back_stack_entry.clone(), transition.clone());
            LaunchedEffect(back_stack_entry.clone(), async move {
                // This ensures we don't animate after the back gesture is cancelled and we are already on the
                // current state
                if ts.current_state() != entry {
                    named_args!(ts.animate_to(target_state = entry)).await;
                } else {
                    // convert from nanoseconds to milliseconds
                    let total_duration = tr.total_duration_nanos() / 1_000_000;
                    // When the predictive back gesture is cancel, we need to manually animate the
                    // SeekableTransitionState from where it left off, to zero and then snapTo the final position.
                    let fraction = ts.fraction();
                    let spec: TweenSpec<f32> = named_args!(tween((fraction * total_duration as f32) as i32));
                    coroutine_scope(move |scope| async move {
                        named_args!(animate(
                            fraction,
                            0.0,
                            animation_spec = Arc::new(spec),
                            block = move |value: f32, _: f32| {
                                let (ts, entry) = (ts.clone(), entry.clone());
                                scope.launch(async move {
                                    if value > 0.0 {
                                        // Seek the original transition back to the currentState
                                        ts.seek_to(value).await;
                                    }
                                    if value == 0.0 {
                                        // Once we animate to the start, we need to snap to the right state.
                                        ts.snap_to(entry).await;
                                    }
                                });
                            }
                        ))
                        .await;
                    })
                    .await;
                }
            });
        }

        let (visible, nav, z) = (visible_entries.clone(), compose_navigator.clone(), z_indices.clone());
        let transition_spec = move |scope: &AnimatedContentTransitionScope<NavBackStackEntry>| -> ContentTransform {
            // If the initialState of the AnimatedContent is not in visibleEntries, we are in a case where visible
            // has cleared the old state for some reason, so instead of attempting to animate away from the
            // initialState, we skip the animation.
            let (initial, target) = (scope.initial_state(), scope.target_state());
            if visible.get().contains(&initial) {
                let initial_z_index = *z.borrow_mut().entry(initial.id()).or_insert(0.0);
                let target_z_index = if target.id() == initial.id() {
                    initial_z_index
                } else if nav.is_pop() || in_predictive_back {
                    initial_z_index - 1.0
                } else {
                    initial_z_index + 1.0
                };
                z.borrow_mut().insert(target.id(), target_z_index);
                named_args!(ContentTransform(
                    final_enter(scope),
                    final_exit(scope),
                    target_content_z_index = target_z_index,
                    size_transform = final_size_transform(scope)
                ))
            } else {
                named_args!(ContentTransform(
                    EnterTransition::None,
                    ExitTransition::None,
                    size_transform = final_size_transform(scope)
                ))
            }
        };
        let content_key: Rc<dyn Fn(&NavBackStackEntry) -> SlotKey> =
            Rc::new(|e: &NavBackStackEntry| SlotKey::new(e.id()));
        let (ts, visible, current, holder) =
            (transition_state.clone(), visible_entries.clone(), back_stack_entry.clone(), saveable_state_holder);
        transition.AnimatedContent(
            modifier = modifier,
            transition_spec = transition_spec,
            content_alignment = content_alignment,
            content_key = Some(content_key),
            content = move |scope: &AnimatedContentScope, it: NavBackStackEntry| {
                // In some specific cases, such as clearing your back stack by changing your start destination,
                // AnimatedContent can contain an entry that is no longer part of visible entries since it was
                // cleared from the back stack and is not animating. In these cases the currentEntry will be null,
                // and in those cases, AnimatedContent will just skip attempting to transition the old entry.
                let is_predictive_back_cancel_animation = ts.current_state() == current;
                let current_entry = if in_predictive_back || is_predictive_back_cancel_animation {
                    // We have to do this because the previous entry does not show up in visibleEntries even if we
                    // prepare it above as part of onBackStackChangeStarted
                    Some(it)
                } else {
                    visible.get().into_iter().rev().find(|entry| *entry == it)
                };
                // while in the scope of the composable, we provide the navBackStackEntry as the
                // ViewModelStoreOwner and LifecycleOwner
                if let Some(entry) = current_entry {
                    let destination = ComposeNavigatorDestination::from_destination(&entry.destination())
                        .expect("a composable destination");
                    let content = destination.content();
                    let (scope, e) = (scope.clone(), entry.clone());
                    entry.LocalOwnersProvider(&holder, move || content(&scope, e));
                }
            },
        );

        let (current, target) = (transition.current_state(), transition.target_state());
        let (nav, navigator, visible, z, entry) = (
            nav_controller.clone(),
            compose_navigator.clone(),
            visible_entries.clone(),
            z_indices.clone(),
            back_stack_entry,
        );
        LaunchedEffect((current.clone(), target.clone()), async move {
            // There is a race condition where previous animation has completed the new animation has yet to start
            // and there is a navigate call before this effect. We need to make sure we are completing only when
            // the start is settled on the actual entry.
            if current == target && (nav.current_back_stack_entry().is_none() || target == entry) {
                for e in visible.get() {
                    navigator.on_transition_complete(&e);
                }
                z.borrow_mut().retain(|key, _| *key == target.id());
            }
        });
    }
}

// ---- rememberNavController, currentBackStackEntryAsState ----

/// A controller with the NavHost's navigators — graphs, composables, dialogs — (Compose's private
/// `createNavController(context)`): the host's deep links its activity's intents.
fn create_nav_controller(deep_links: &kompoz_ui::platform::DeepLinks) -> NavHostController {
    let controller = NavController::new();
    let provider = controller.navigator_provider();
    provider.add_navigator(Rc::new(ComposeNavGraphNavigator::new(&provider)));
    provider.add_navigator(Rc::new(ComposeNavigator::new()));
    provider.add_navigator(Rc::new(DialogNavigator::new()));
    controller.set_deep_links(Some(deep_links.clone()));
    controller
}

/// Compose's private `NavControllerSaver`: the controller's saved state, a new controller taking it back.
fn nav_controller_saver(deep_links: kompoz_ui::platform::DeepLinks) -> Saver<NavHostController> {
    Saver::new(
        |c: &NavHostController| c.save_state().map_or(SavedValue::Null, |s| s.to_saved_value()),
        move |saved: &SavedValue| {
            let c = create_nav_controller(&deep_links);
            c.restore_state(Some(SavedState::from_saved_value(saved)));
            Some(c)
        },
    )
}

/// The navigators given (by identity: Kotlin's `inputs`).
#[derive(Clone)]
struct SameNavigators(Vec<Rc<dyn Navigator>>);

impl PartialEq for SameNavigators {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self.0.iter().zip(&other.0).all(|(a, b)| std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b)))
    }
}

/// A NavHostController for a NavHost, saved with the composition (Compose's `rememberNavController(vararg
/// navigators)`): its navigators the NavHost's — graphs, composables, dialogs — and `navigators`; the host's deep
/// links (Android's activity intents: the link the app was opened with, those sent since) go to it.
///
/// Kotlin: `rememberNavController(vararg navigators: Navigator<out NavDestination>)`
#[kompozable]
pub fn remember_nav_controller(#[default(&[])] navigators: &[Rc<dyn Navigator>]) -> NavController {
    let deep_links = LocalDeepLinks.current();
    let saver = nav_controller_saver(deep_links.clone());
    let controller = remember_saveable_keyed_with(SameNavigators(navigators.to_vec()), &saver, || {
        create_nav_controller(&deep_links)
    });
    for navigator in navigators {
        controller.navigator_provider().add_navigator_named(&navigator.name(), navigator.clone());
    }
    // (an activity's intents go to its controller while it is there)
    let c = controller.clone();
    DisposableEffect(controller.clone(), move || on_dispose(move || c.set_deep_links(None)));
    controller
}

impl NavController {
    /// Its current back stack entry as state (Compose's `currentBackStackEntryAsState()`): what reads it
    /// recomposes as it changes; none until the first.
    ///
    /// Kotlin: `currentBackStackEntryAsState(): State<NavBackStackEntry?>`
    #[track_caller]
    pub fn current_back_stack_entry_as_state(&self) -> State<Option<NavBackStackEntry>> {
        let flow = kompoz_runtime::group(|| {
            remember_keyed(self.clone(), || self.current_back_stack_entry_flow().as_flow().map(Some))
        });
        flow.collect_as_confined_state(None)
    }
}

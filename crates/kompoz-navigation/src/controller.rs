//! The navigation controller (Compose's `NavController` — its common code, `NavControllerImpl`, and what its
//! platforms add: deep link handling, `navigateUp`, its saved state): it keeps the back stack of entries — each
//! destination navigated to, the graphs it is in below it —, navigates (by route, typed route, deep link, request)
//! with options, pops, saves and restores stacks, and moves its entries' lifecycles.

use crate::deep_link::{NavDeepLinkRequest, NavDeepLinkRequestBuilder};
use crate::destination::{NavDestination, NavGraph, create_route};
use crate::entry::{NavBackStackEntry, NavBackStackEntryState, NavViewModelStoreProvider};
use crate::java::string_hash;
use crate::navigator::{
    NavGraphNavigator, Navigator, NavigatorExtras, NavigatorProvider, NavigatorState, NavigatorStateImpl,
    same_navigator,
};
use crate::options::{NavOptions, NavOptionsBuilder, nav_options};
use crate::saved_state::{SavedState, saved_state};
use crate::serialization::{AnyRoute, Route, RouteClass, generate_route_with_args, hash_code_of};
use crate::uri::NavUri;
use kompoz_coroutines::{MutableSharedFlow, MutableStateFlow, SharedFlow, StateFlow};
use kompoz_lifecycle::{LifecycleEvent, LifecycleObserver, LifecycleOwner, LifecycleState};
use kompoz_ui::platform::DeepLinks;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

/// Told each time the destination shown changes (Compose's `NavController.OnDestinationChangedListener`): a
/// closure `|controller, destination, arguments| …` is one.
pub trait OnDestinationChangedListener: 'static {
    fn on_destination_changed(
        &self,
        controller: &NavController,
        destination: &NavDestination,
        arguments: Option<&SavedState>,
    );
}

impl<F: Fn(&NavController, &NavDestination, Option<&SavedState>) + 'static> OnDestinationChangedListener for F {
    fn on_destination_changed(
        &self,
        controller: &NavController,
        destination: &NavDestination,
        arguments: Option<&SavedState>,
    ) {
        self(controller, destination, arguments)
    }
}

/// Where a navigation goes (what Kotlin's `navigate` overloads take): a route (`"detail/42"`), a typed route's value
/// (`Detail { id: 42 }`), a deep link's URI, a request.
pub enum NavTarget {
    Route(String),
    Object(Rc<dyn AnyRoute>),
    Uri(NavUri),
    Request(NavDeepLinkRequest),
}

/// What converts to a [`NavTarget`]: `&str`/`String` (a route), a typed route's value, a [`NavUri`], a
/// [`NavDeepLinkRequest`].
pub trait IntoNavTarget {
    fn into_nav_target(self) -> NavTarget;
}

impl IntoNavTarget for &str {
    fn into_nav_target(self) -> NavTarget {
        NavTarget::Route(self.to_string())
    }
}
impl IntoNavTarget for String {
    fn into_nav_target(self) -> NavTarget {
        NavTarget::Route(self)
    }
}
impl IntoNavTarget for &String {
    fn into_nav_target(self) -> NavTarget {
        NavTarget::Route(self.clone())
    }
}
impl IntoNavTarget for NavUri {
    fn into_nav_target(self) -> NavTarget {
        NavTarget::Uri(self)
    }
}
impl IntoNavTarget for NavDeepLinkRequest {
    fn into_nav_target(self) -> NavTarget {
        NavTarget::Request(self)
    }
}
impl<T: Route> IntoNavTarget for T {
    fn into_nav_target(self) -> NavTarget {
        NavTarget::Object(Rc::new(self))
    }
}
impl IntoNavTarget for NavTarget {
    fn into_nav_target(self) -> NavTarget {
        self
    }
}

/// A destination on the back stack as Kotlin's `popBackStack`, `getBackStackEntry`, `clearBackStack` name it: its
/// id, a route (pattern or path), a typed route's value.
pub enum DestinationTarget {
    Id(i32),
    Route(String),
    Object(Rc<dyn AnyRoute>),
}

/// What converts to a [`DestinationTarget`]: an `i32` (an id), `&str`/`String` (a route), a typed route's value.
pub trait IntoDestinationTarget {
    fn into_destination_target(self) -> DestinationTarget;
}

impl IntoDestinationTarget for i32 {
    fn into_destination_target(self) -> DestinationTarget {
        DestinationTarget::Id(self)
    }
}
impl IntoDestinationTarget for &str {
    fn into_destination_target(self) -> DestinationTarget {
        DestinationTarget::Route(self.to_string())
    }
}
impl IntoDestinationTarget for String {
    fn into_destination_target(self) -> DestinationTarget {
        DestinationTarget::Route(self)
    }
}
impl IntoDestinationTarget for &String {
    fn into_destination_target(self) -> DestinationTarget {
        DestinationTarget::Route(self.clone())
    }
}
impl<T: Route> IntoDestinationTarget for T {
    fn into_destination_target(self) -> DestinationTarget {
        DestinationTarget::Object(Rc::new(self))
    }
}

const KEY_NAVIGATOR_STATE: &str = "android-support-nav:controller:navigatorState";
const KEY_NAVIGATOR_STATE_NAMES: &str = "android-support-nav:controller:navigatorState:names";
const KEY_BACK_STACK: &str = "android-support-nav:controller:backStack";
const KEY_BACK_STACK_DEST_IDS: &str = "android-support-nav:controller:backStackDestIds";
const KEY_BACK_STACK_IDS: &str = "android-support-nav:controller:backStackIds";
const KEY_BACK_STACK_STATES_IDS: &str = "android-support-nav:controller:backStackStates";
const KEY_BACK_STACK_STATES_PREFIX: &str = "android-support-nav:controller:backStackStates:";
const KEY_DEEP_LINK_HANDLED: &str = "android-support-nav:controller:deepLinkHandled";

thread_local! {
    static DEEP_LINK_SAVE_STATE: Cell<bool> = const { Cell::new(true) };
}

type EntryHandler = Rc<dyn Fn(&NavBackStackEntry)>;

pub(crate) struct ControllerData {
    me: Weak<ControllerData>,
    graph: RefCell<Option<NavGraph>>,
    navigator_state_to_restore: RefCell<Option<SavedState>>,
    back_stack_to_restore: RefCell<Option<Vec<SavedState>>>,
    back_queue: RefCell<Vec<NavBackStackEntry>>,
    current_back_stack: MutableStateFlow<Vec<NavBackStackEntry>>,
    visible_entries: MutableStateFlow<Vec<NavBackStackEntry>>,
    child_to_parent_entries: RefCell<Vec<(NavBackStackEntry, NavBackStackEntry)>>,
    parent_to_child_count: RefCell<Vec<(NavBackStackEntry, i32)>>,
    /// Destination id → the id of the saved stack it leads to (Kotlin's `backStackMap`, a LinkedHashMap).
    back_stack_map: RefCell<Vec<(i32, Option<String>)>>,
    back_stack_states: RefCell<Vec<(String, Vec<NavBackStackEntryState>)>>,
    lifecycle_owner: RefCell<Option<Rc<dyn LifecycleOwner>>>,
    lifecycle_observer: RefCell<Option<Rc<dyn LifecycleObserver>>>,
    view_model_store_provider: RefCell<Option<Rc<dyn NavViewModelStoreProvider>>>,
    on_destination_changed_listeners: RefCell<Vec<Rc<dyn OnDestinationChangedListener>>>,
    host_lifecycle_state: Cell<LifecycleState>,
    navigator_provider: RefCell<NavigatorProvider>,
    navigator_state: RefCell<Vec<(Rc<dyn Navigator>, NavigatorState)>>,
    add_to_back_stack_handler: RefCell<Option<EntryHandler>>,
    pop_from_back_stack_handler: RefCell<Option<EntryHandler>>,
    entry_saved_state: RefCell<Vec<(NavBackStackEntry, bool)>>,
    dispatch_reentrant_count: Cell<i32>,
    back_stack_entries_to_dispatch: RefCell<Vec<NavBackStackEntry>>,
    current_back_stack_entry_flow: MutableSharedFlow<NavBackStackEntry>,
    deep_link_handled: Cell<bool>,
    deep_links: RefCell<Option<DeepLinks>>,
}

/// The navigation controller (Compose's `NavController`, `NavHostController`): see the module. A cheap handle:
/// clones are the same controller.
#[derive(Clone)]
pub struct NavController(Rc<ControllerData>);

/// Compose's `NavHostController` (the controller a `NavHost` is given): the same type here.
pub type NavHostController = NavController;

impl PartialEq for NavController {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for NavController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NavController({:?})", self.0.back_queue.borrow())
    }
}

impl Default for NavController {
    fn default() -> Self {
        NavController::new()
    }
}

/// The controller's own `NavigatorState` (Kotlin's inner `NavControllerNavigatorState`).
struct ControllerNavigatorState {
    controller: Weak<ControllerData>,
    navigator: Rc<dyn Navigator>,
}

impl ControllerNavigatorState {
    fn controller(&self) -> NavController {
        NavController(self.controller.upgrade().expect("the navigator's controller"))
    }
}

impl NavigatorStateImpl for ControllerNavigatorState {
    fn create_back_stack_entry(
        &self,
        _: &NavigatorState,
        destination: &NavDestination,
        arguments: Option<SavedState>,
    ) -> NavBackStackEntry {
        self.controller().create_back_stack_entry(destination, arguments)
    }

    fn push(&self, state: &NavigatorState, entry: &NavBackStackEntry) {
        self.controller().push_entry(state, &self.navigator, entry);
    }

    fn pop(&self, state: &NavigatorState, pop_up_to: &NavBackStackEntry, save_state: bool) {
        self.controller().pop_entry(state, &self.navigator, pop_up_to, save_state);
    }

    fn mark_transition_complete(&self, state: &NavigatorState, entry: &NavBackStackEntry) {
        self.controller().mark_transition_complete(state, entry);
    }

    fn prepare_for_transition(&self, state: &NavigatorState, entry: &NavBackStackEntry) {
        state.base_prepare_for_transition(entry);
        self.controller().prepare_for_transition(entry);
    }
}

fn index_of(list: &[NavBackStackEntry], entry: &NavBackStackEntry) -> Option<usize> {
    list.iter().position(|e| e == entry)
}

impl NavController {
    /// A controller with the graph navigator (Kotlin's `NavController(context)`; a `NavHost`'s adds its own
    /// navigators).
    pub fn new() -> NavController {
        let data = Rc::new_cyclic(|me: &Weak<ControllerData>| ControllerData {
            me: me.clone(),
            graph: RefCell::new(None),
            navigator_state_to_restore: RefCell::new(None),
            back_stack_to_restore: RefCell::new(None),
            back_queue: RefCell::new(Vec::new()),
            current_back_stack: MutableStateFlow::new(Vec::new()),
            visible_entries: MutableStateFlow::new(Vec::new()),
            child_to_parent_entries: RefCell::new(Vec::new()),
            parent_to_child_count: RefCell::new(Vec::new()),
            back_stack_map: RefCell::new(Vec::new()),
            back_stack_states: RefCell::new(Vec::new()),
            lifecycle_owner: RefCell::new(None),
            lifecycle_observer: RefCell::new(None),
            view_model_store_provider: RefCell::new(None),
            on_destination_changed_listeners: RefCell::new(Vec::new()),
            host_lifecycle_state: Cell::new(LifecycleState::Initialized),
            navigator_provider: RefCell::new(NavigatorProvider::new()),
            navigator_state: RefCell::new(Vec::new()),
            add_to_back_stack_handler: RefCell::new(None),
            pop_from_back_stack_handler: RefCell::new(None),
            entry_saved_state: RefCell::new(Vec::new()),
            dispatch_reentrant_count: Cell::new(0),
            back_stack_entries_to_dispatch: RefCell::new(Vec::new()),
            current_back_stack_entry_flow: MutableSharedFlow::new(1),
            deep_link_handled: Cell::new(false),
            deep_links: RefCell::new(None),
        });
        let controller = NavController(data);
        let provider = controller.navigator_provider();
        provider.add_navigator(Rc::new(NavGraphNavigator::new(&provider)));
        controller
    }

    fn from_weak(me: &Weak<ControllerData>) -> Option<NavController> {
        me.upgrade().map(NavController)
    }

    // ---- The graph ----

    /// Its graph (Kotlin's `graph`).
    ///
    /// # Panics
    /// Before one is set.
    pub fn graph(&self) -> NavGraph {
        self.0.graph.borrow().clone().expect("You must call setGraph() before calling getGraph()")
    }

    /// Sets its graph (Kotlin's `graph = …`): see [`set_graph_with_args`](Self::set_graph_with_args).
    pub fn set_graph(&self, graph: NavGraph) {
        self.set_graph_with_args(graph, None);
    }

    /// Sets its graph (Kotlin's `setGraph(graph, startDestinationArgs)`): a new one replaces the stack (saved
    /// stacks cleared) with its start destination (or a deep link's, or the stack restored); the same one again
    /// takes the new graph's nodes for the entries' destinations.
    ///
    /// # Panics
    /// With entries on the stack after the controller's host was destroyed.
    pub fn set_graph_with_args(&self, graph: NavGraph, start_destination_args: Option<SavedState>) {
        assert!(
            self.0.back_queue.borrow().is_empty() || self.host_lifecycle_state() != LifecycleState::Destroyed,
            "You cannot set a new graph on a NavController with entries on the back stack after the NavController \
             has been destroyed. Please ensure that your NavHost has the same lifetime as your NavController."
        );
        let previous = self.0.graph.borrow().clone();
        if previous.as_ref() != Some(&graph) {
            if let Some(previous_graph) = previous {
                let saved_ids: Vec<i32> = self.0.back_stack_map.borrow().iter().map(|(id, _)| *id).collect();
                for id in saved_ids {
                    self.clear_back_stack_internal_id(id);
                }
                self.pop_back_stack_internal_id(previous_graph.id(), true, false);
            }
            *self.0.graph.borrow_mut() = Some(graph);
            self.on_graph_created(start_destination_args);
        } else {
            let current = previous.expect("the same graph");
            // (each entry's hierarchy taken before the nodes are replaced and any entry is given its new destination:
            // a destination's parent is a weak reference here — Kotlin's a strong one —, the old graphs kept by these
            // lists meanwhile)
            let entries = self.0.back_queue.borrow().clone();
            let hierarchies: Vec<Vec<NavDestination>> =
                entries.iter().map(|e| e.destination().hierarchy().into_iter().rev().collect()).collect();
            for node in graph.nodes() {
                // (Kotlin's `nodes.replace(key, newDestination)`: the new graph's nodes in the old one, their parent
                // this one, equal to the new graph, see replace_node)
                current.replace_node(node.id(), node);
            }
            for (entry, hierarchy) in entries.iter().zip(hierarchies) {
                let mut new_dest: NavDestination = current.as_destination();
                for old in hierarchy {
                    if old == current.as_destination() && new_dest == graph.as_destination() {
                        continue;
                    }
                    if let Some(g) = new_dest.as_graph() {
                        new_dest = g.find_node_id(old.id()).expect("the destination in the new graph");
                    }
                }
                entry.set_destination(new_dest);
            }
        }
    }

    fn on_graph_created(&self, start_destination_args: Option<SavedState>) {
        let to_restore = self.0.navigator_state_to_restore.borrow().clone();
        if let Some(state) = to_restore {
            let names = state.read(|r| r.get_string_list_or_null(KEY_NAVIGATOR_STATE_NAMES));
            for name in names.unwrap_or_default() {
                let navigator = self.navigator_provider().get_navigator(&name);
                if let Some(saved) = state.read(|r| r.get_saved_state_or_null(&name)) {
                    navigator.on_restore_state(saved);
                }
            }
        }
        let back_stack_to_restore = self.0.back_stack_to_restore.borrow_mut().take();
        if let Some(back_stack_to_restore) = back_stack_to_restore {
            for saved in back_stack_to_restore {
                let state = NavBackStackEntryState::from_state(&saved);
                let node = self.find_destination_id(state.destination_id(), None).unwrap_or_else(|| {
                    panic!(
                        "Restoring the Navigation back stack failed: destination {} cannot be found from the current \
                         destination {:?}",
                        state.destination_id(),
                        self.current_destination()
                    )
                });
                let entry =
                    state.instantiate(node.clone(), self.host_lifecycle_state(), self.view_model_store_provider());
                let navigator = self.navigator_provider().get_navigator(&node.navigator_name());
                let navigator_state = self.navigator_state_of(&navigator);
                self.0.back_queue.borrow_mut().push(entry.clone());
                navigator_state.base_push(&entry);
                if let Some(parent) = entry.destination().parent() {
                    let parent_entry = self.get_back_stack_entry_id(parent.id());
                    self.link_child_to_parent(&entry, &parent_entry);
                }
            }
        }
        for (_, navigator) in self.navigator_provider().navigators() {
            if !navigator.is_attached() {
                let state = self.navigator_state_of(&navigator);
                navigator.on_attach(state);
            }
        }
        let graph = self.0.graph.borrow().clone();
        if let Some(graph) = graph.filter(|_| self.0.back_queue.borrow().is_empty()) {
            if !self.check_deep_link_handled() {
                self.navigate_node(&graph.as_destination(), start_destination_args, None, None);
            }
        } else {
            self.dispatch_on_destination_changed();
        }
    }

    /// The navigator state of `navigator`, made the first time (Kotlin's `navigatorState.getOrPut`).
    fn navigator_state_of(&self, navigator: &Rc<dyn Navigator>) -> NavigatorState {
        if let Some((_, s)) = self.0.navigator_state.borrow().iter().find(|(n, _)| same_navigator(n, navigator)) {
            return s.clone();
        }
        let state = NavigatorState::from_rc(Rc::new(ControllerNavigatorState {
            controller: self.0.me.clone(),
            navigator: navigator.clone(),
        }));
        self.0.navigator_state.borrow_mut().push((navigator.clone(), state.clone()));
        state
    }

    fn existing_navigator_state(&self, navigator: &Rc<dyn Navigator>) -> Option<NavigatorState> {
        self.0.navigator_state.borrow().iter().find(|(n, _)| same_navigator(n, navigator)).map(|(_, s)| s.clone())
    }

    fn all_navigator_states(&self) -> Vec<NavigatorState> {
        self.0.navigator_state.borrow().iter().map(|(_, s)| s.clone()).collect()
    }

    /// Its navigators (Kotlin's `navigatorProvider`).
    pub fn navigator_provider(&self) -> NavigatorProvider {
        self.0.navigator_provider.borrow().clone()
    }

    /// Sets its navigators (Kotlin's `navigatorProvider = …`).
    ///
    /// # Panics
    /// After its graph was set.
    pub fn set_navigator_provider(&self, navigator_provider: NavigatorProvider) {
        assert!(self.0.back_queue.borrow().is_empty(), "NavigatorProvider must be set before setGraph call");
        *self.0.navigator_provider.borrow_mut() = navigator_provider;
    }

    // ---- The back stack ----

    /// Its back stack, graphs' entries too (Kotlin's `currentBackStack`).
    pub fn current_back_stack(&self) -> StateFlow<Vec<NavBackStackEntry>> {
        self.0.current_back_stack.as_state_flow()
    }

    /// The entries to show (Kotlin's `visibleEntries`): those at least started and those in a transition, without
    /// the graphs'.
    pub fn visible_entries(&self) -> StateFlow<Vec<NavBackStackEntry>> {
        self.0.visible_entries.as_state_flow()
    }

    /// Its back stack now (graphs' entries too).
    pub fn back_queue(&self) -> Vec<NavBackStackEntry> {
        self.0.back_queue.borrow().clone()
    }

    /// The top entry (Kotlin's `currentBackStackEntry`).
    pub fn current_back_stack_entry(&self) -> Option<NavBackStackEntry> {
        self.0.back_queue.borrow().last().cloned()
    }

    /// Each entry that becomes the top one (Kotlin's `currentBackStackEntryFlow`; the last one replayed).
    pub fn current_back_stack_entry_flow(&self) -> SharedFlow<NavBackStackEntry> {
        self.0.current_back_stack_entry_flow.as_shared_flow()
    }

    /// The entry below the top one that isn't a graph's (Kotlin's `previousBackStackEntry`).
    pub fn previous_back_stack_entry(&self) -> Option<NavBackStackEntry> {
        self.0.back_queue.borrow().iter().rev().skip(1).find(|e| !e.destination().is_graph()).cloned()
    }

    /// The top entry's destination (Kotlin's `currentDestination`).
    pub fn current_destination(&self) -> Option<NavDestination> {
        self.current_back_stack_entry().map(|e| e.destination())
    }

    /// The topmost entry of `route` (an id, a route, a typed route's value; Kotlin's `getBackStackEntry`).
    ///
    /// # Panics
    /// If there is none on the back stack (Kotlin's IllegalArgumentException).
    pub fn get_back_stack_entry(&self, route: impl IntoDestinationTarget) -> NavBackStackEntry {
        match route.into_destination_target() {
            DestinationTarget::Id(id) => self.get_back_stack_entry_id(id),
            DestinationTarget::Route(r) => self.get_back_stack_entry_route(&r),
            DestinationTarget::Object(o) => {
                let route = self.generate_route_filled(&*o);
                self.get_back_stack_entry_route(&route)
            }
        }
    }

    /// The topmost entry of `T`'s destination (Compose's `getBackStackEntry<T>()`).
    ///
    /// Kotlin: `getBackStackEntry<T>()`
    pub fn get_back_stack_entry_of<T: Route>(&self) -> NavBackStackEntry {
        self.get_back_stack_entry_class(RouteClass::of::<T>())
    }

    /// The topmost entry of `route`'s destination (Kotlin's `getBackStackEntry(route: KClass<T>)`).
    pub fn get_back_stack_entry_class(&self, route: RouteClass) -> NavBackStackEntry {
        let id = route.hash_code();
        assert!(
            self.find_destination_comprehensive(&self.graph().as_destination(), id, true, None).is_some(),
            "Destination with route {} cannot be found in navigation graph {:?}",
            route.simple_name(),
            self.graph()
        );
        self.current_back_stack().value().into_iter().rev().find(|e| e.destination().id() == id).unwrap_or_else(|| {
            panic!(
                "No destination with route {} is on the NavController's back stack. The current destination is {:?}",
                route.simple_name(),
                self.current_destination()
            )
        })
    }

    fn get_back_stack_entry_id(&self, destination_id: i32) -> NavBackStackEntry {
        self.0.back_queue.borrow().iter().rev().find(|e| e.destination().id() == destination_id).cloned().unwrap_or_else(|| {
            panic!(
                "No destination with ID {destination_id} is on the NavController's back stack. The current destination \
                 is {:?}",
                self.current_destination()
            )
        })
    }

    fn get_back_stack_entry_route(&self, route: &str) -> NavBackStackEntry {
        let queue = self.0.back_queue.borrow().clone();
        queue.into_iter().rev().find(|e| e.destination().has_route_with(route, e.immutable_args())).unwrap_or_else(|| {
            panic!(
                "No destination with route {route} is on the NavController's back stack. The current destination is \
                 {:?}",
                self.current_destination()
            )
        })
    }

    fn link_child_to_parent(&self, child: &NavBackStackEntry, parent: &NavBackStackEntry) {
        let mut links = self.0.child_to_parent_entries.borrow_mut();
        match links.iter_mut().find(|(c, _)| c == child) {
            Some(link) => link.1 = parent.clone(),
            None => links.push((child.clone(), parent.clone())),
        }
        drop(links);
        let mut counts = self.0.parent_to_child_count.borrow_mut();
        match counts.iter_mut().find(|(p, _)| p == parent) {
            Some(c) => c.1 += 1,
            None => counts.push((parent.clone(), 1)),
        }
    }

    fn unlink_child_from_parent(&self, child: &NavBackStackEntry) -> Option<NavBackStackEntry> {
        let parent = {
            let mut links = self.0.child_to_parent_entries.borrow_mut();
            let i = links.iter().position(|(c, _)| c == child)?;
            links.remove(i).1
        };
        let count = {
            let mut counts = self.0.parent_to_child_count.borrow_mut();
            counts.iter_mut().find(|(p, _)| *p == parent).map(|c| {
                c.1 -= 1;
                c.1
            })
        };
        if count == Some(0) {
            let navigator = self.navigator_provider().get_navigator(&parent.destination().navigator_name());
            if let Some(state) = self.existing_navigator_state(&navigator) {
                state.mark_transition_complete(&parent);
            }
            self.0.parent_to_child_count.borrow_mut().retain(|(p, _)| *p != parent);
        }
        Some(parent)
    }

    fn parent_child_count(&self, parent: &NavBackStackEntry) -> Option<i32> {
        self.0.parent_to_child_count.borrow().iter().find(|(p, _)| p == parent).map(|(_, c)| *c)
    }

    // ---- What the navigator states call ----

    fn push_entry(&self, state: &NavigatorState, state_navigator: &Rc<dyn Navigator>, entry: &NavBackStackEntry) {
        let destination_navigator = self.navigator_provider().get_navigator(&entry.destination().navigator_name());
        if same_navigator(&destination_navigator, state_navigator) {
            let handler = self.0.add_to_back_stack_handler.borrow().clone();
            match handler {
                Some(h) => {
                    h(entry);
                    state.base_push(entry);
                }
                None => {
                    // (Kotlin logs "Ignoring add of destination … outside of the call to navigate()")
                }
            }
        } else {
            let other = self.existing_navigator_state(&destination_navigator).unwrap_or_else(|| {
                panic!("NavigatorBackStack for {} should already be created", entry.destination().navigator_name())
            });
            other.push(entry);
        }
    }

    fn create_back_stack_entry(
        &self,
        destination: &NavDestination,
        arguments: Option<SavedState>,
    ) -> NavBackStackEntry {
        NavBackStackEntry::create(
            destination.clone(),
            arguments,
            self.host_lifecycle_state(),
            self.view_model_store_provider(),
            None,
            None,
        )
    }

    fn pop_entry(
        &self,
        state: &NavigatorState,
        state_navigator: &Rc<dyn Navigator>,
        pop_up_to: &NavBackStackEntry,
        save_state: bool,
    ) {
        let destination_navigator = self.navigator_provider().get_navigator(&pop_up_to.destination().navigator_name());
        {
            let mut saved = self.0.entry_saved_state.borrow_mut();
            match saved.iter_mut().find(|(e, _)| e == pop_up_to) {
                Some(s) => s.1 = save_state,
                None => saved.push((pop_up_to.clone(), save_state)),
            }
        }
        if same_navigator(&destination_navigator, state_navigator) {
            let handler = self.0.pop_from_back_stack_handler.borrow().clone();
            match handler {
                Some(h) => {
                    h(pop_up_to);
                    state.base_pop(pop_up_to, save_state);
                }
                None => {
                    let state = state.clone();
                    let entry = pop_up_to.clone();
                    self.pop_back_stack_from_navigator(pop_up_to, move || state.base_pop(&entry, save_state));
                }
            }
        } else {
            let other = self.existing_navigator_state(&destination_navigator).expect("the navigator's state");
            other.pop(pop_up_to, save_state);
        }
    }

    fn mark_transition_complete(&self, state: &NavigatorState, entry: &NavBackStackEntry) {
        let saved = self.0.entry_saved_state.borrow().iter().find(|(e, _)| e == entry).is_some_and(|(_, s)| *s);
        state.base_mark_transition_complete(entry);
        self.0.entry_saved_state.borrow_mut().retain(|(e, _)| e != entry);
        let in_queue = self.0.back_queue.borrow().contains(entry);
        if !in_queue {
            self.unlink_child_from_parent(entry);
            if entry.lifecycle().current_state().is_at_least(LifecycleState::Created) {
                entry.set_max_lifecycle(LifecycleState::Destroyed);
            }
            let id_in_queue = self.0.back_queue.borrow().iter().any(|e| e.id() == entry.id());
            if !id_in_queue
                && !saved
                && let Some(p) = self.view_model_store_provider()
            {
                p.clear(&entry.id());
            }
            self.update_back_stack_lifecycle();
            self.0.visible_entries.set_value(self.populate_visible_entries());
        } else if !state.is_navigating() {
            self.update_back_stack_lifecycle();
            self.0.current_back_stack.set_value(self.back_queue());
            self.0.visible_entries.set_value(self.populate_visible_entries());
        }
    }

    fn prepare_for_transition(&self, entry: &NavBackStackEntry) {
        assert!(self.0.back_queue.borrow().contains(entry), "Cannot transition entry that is not in the back stack");
        entry.set_max_lifecycle(LifecycleState::Started);
    }

    // ---- Listeners ----

    /// Adds `listener`, told now of the current destination if there is one (Kotlin's
    /// `addOnDestinationChangedListener`).
    pub fn add_on_destination_changed_listener(&self, listener: Rc<dyn OnDestinationChangedListener>) {
        self.0.on_destination_changed_listeners.borrow_mut().push(listener.clone());
        if let Some(entry) = self.current_back_stack_entry() {
            listener.on_destination_changed(self, &entry.destination(), entry.arguments().as_ref());
        }
    }

    /// Removes `listener` (Kotlin's `removeOnDestinationChangedListener`).
    pub fn remove_on_destination_changed_listener(&self, listener: &Rc<dyn OnDestinationChangedListener>) {
        self.0
            .on_destination_changed_listeners
            .borrow_mut()
            .retain(|l| !std::ptr::addr_eq(Rc::as_ptr(l), Rc::as_ptr(listener)));
    }

    // ---- Popping ----

    /// Pops the top destination (and the graphs it leaves; Kotlin's `popBackStack()`): whether a destination is
    /// left to show (popping the last one empties the stack: false).
    pub fn pop_back_stack(&self) -> bool {
        match self.current_destination() {
            None => false,
            Some(d) => self.pop_back_stack_id(d.id(), true, false),
        }
    }

    /// Pops down to the topmost entry of `route` — an id, a route (pattern or path), a typed route's value —, it too
    /// when `inclusive`, saving what is popped for a later `restoreState` when `save_state` (Kotlin's
    /// `popBackStack(route, inclusive, saveState)`): whether something was popped and a destination is left.
    ///
    /// Kotlin: `popBackStack(route: String, inclusive: Boolean, saveState: Boolean = false)`
    pub fn pop_back_stack_to(&self, route: impl IntoDestinationTarget, inclusive: bool, save_state: bool) -> bool {
        match route.into_destination_target() {
            DestinationTarget::Id(id) => self.pop_back_stack_id(id, inclusive, save_state),
            DestinationTarget::Route(r) => {
                let popped = self.pop_back_stack_internal_route(&r, inclusive, save_state);
                popped && self.dispatch_on_destination_changed()
            }
            DestinationTarget::Object(o) => {
                let popped = self.pop_back_stack_internal_object(&*o, inclusive, save_state);
                popped && self.dispatch_on_destination_changed()
            }
        }
    }

    /// Pops down to `T`'s topmost entry (Compose's `popBackStack<T>(inclusive, saveState)`).
    ///
    /// Kotlin: `popBackStack<T>(inclusive: Boolean, saveState: Boolean = false)`
    pub fn pop_back_stack_of<T: Route>(&self, inclusive: bool, save_state: bool) -> bool {
        self.pop_back_stack_class(RouteClass::of::<T>(), inclusive, save_state)
    }

    /// Kotlin's `popBackStack(route: KClass<T>, inclusive, saveState)`.
    ///
    /// Kotlin: `popBackStack(route: KClass<T>, inclusive: Boolean, saveState: Boolean = false)`
    ///
    /// # Panics
    /// If the graph has no destination of `route`.
    pub fn pop_back_stack_class(&self, route: RouteClass, inclusive: bool, save_state: bool) -> bool {
        let id = route.hash_code();
        assert!(
            self.find_destination_comprehensive(&self.graph().as_destination(), id, true, None).is_some(),
            "Destination with route {} cannot be found in navigation graph {:?}",
            route.simple_name(),
            self.graph()
        );
        self.pop_back_stack_id(id, inclusive, save_state)
    }

    fn pop_back_stack_id(&self, destination_id: i32, inclusive: bool, save_state: bool) -> bool {
        let popped = self.pop_back_stack_internal_id(destination_id, inclusive, save_state);
        popped && self.dispatch_on_destination_changed()
    }

    fn pop_back_stack_internal_id(&self, destination_id: i32, inclusive: bool, save_state: bool) -> bool {
        let queue = self.back_queue();
        if queue.is_empty() {
            return false;
        }
        let mut pop_operations: Vec<Rc<dyn Navigator>> = Vec::new();
        let mut found: Option<NavDestination> = None;
        for entry in queue.iter().rev() {
            let destination = entry.destination();
            let navigator = self.navigator_provider().get_navigator(&destination.navigator_name());
            if inclusive || destination.id() != destination_id {
                pop_operations.push(navigator);
            }
            if destination.id() == destination_id {
                found = Some(destination);
                break;
            }
        }
        match found {
            None => false,
            Some(f) => self.execute_pop_operations(pop_operations, &f, inclusive, save_state),
        }
    }

    fn pop_back_stack_internal_object(&self, route: &dyn AnyRoute, inclusive: bool, save_state: bool) -> bool {
        let final_route = self.generate_route_filled(route);
        self.pop_back_stack_internal_route(&final_route, inclusive, save_state)
    }

    fn pop_back_stack_internal_route(&self, route: &str, inclusive: bool, save_state: bool) -> bool {
        let queue = self.back_queue();
        if queue.is_empty() {
            return false;
        }
        let mut pop_operations: Vec<Rc<dyn Navigator>> = Vec::new();
        let mut found: Option<NavDestination> = None;
        for entry in queue.iter().rev() {
            let destination = entry.destination();
            let has_route = destination.has_route_with(route, entry.immutable_args());
            if inclusive || !has_route {
                pop_operations.push(self.navigator_provider().get_navigator(&destination.navigator_name()));
            }
            if has_route {
                found = Some(destination);
                break;
            }
        }
        match found {
            None => false,
            Some(f) => self.execute_pop_operations(pop_operations, &f, inclusive, save_state),
        }
    }

    /// The destination, then the graphs it starts (each its parent's start destination).
    fn start_chain(destination: &NavDestination) -> Vec<NavDestination> {
        let mut out = vec![destination.clone()];
        loop {
            let last = out.last().expect("one").clone();
            match last.parent() {
                Some(p) if p.start_destination_id() == last.id() => out.push(p.as_destination()),
                _ => break,
            }
        }
        out
    }

    fn back_stack_map_contains(&self, id: i32) -> bool {
        self.0.back_stack_map.borrow().iter().any(|(k, _)| *k == id)
    }

    fn back_stack_map_put(&self, id: i32, value: Option<String>) {
        let mut map = self.0.back_stack_map.borrow_mut();
        match map.iter_mut().find(|(k, _)| *k == id) {
            Some(slot) => slot.1 = value,
            None => map.push((id, value)),
        }
    }

    fn execute_pop_operations(
        &self,
        pop_operations: Vec<Rc<dyn Navigator>>,
        found_destination: &NavDestination,
        inclusive: bool,
        save_state: bool,
    ) -> bool {
        let popped = Rc::new(Cell::new(false));
        let saved: Rc<RefCell<Vec<NavBackStackEntryState>>> = Rc::new(RefCell::new(Vec::new()));
        for navigator in pop_operations {
            let received = Rc::new(Cell::new(false));
            let top = self.0.back_queue.borrow().last().cloned().expect("an entry to pop");
            let (r, p, s, me) = (received.clone(), popped.clone(), saved.clone(), self.0.me.clone());
            self.pop_back_stack_internal_with(
                &navigator,
                &top,
                save_state,
                Rc::new(move |entry: &NavBackStackEntry| {
                    r.set(true);
                    p.set(true);
                    if let Some(c) = NavController::from_weak(&me) {
                        c.pop_entry_from_back_stack(entry, save_state, &mut s.borrow_mut());
                    }
                }),
            );
            if !received.get() {
                break;
            }
        }
        if save_state {
            let saved = saved.borrow();
            if !inclusive {
                for destination in NavController::start_chain(found_destination) {
                    if self.back_stack_map_contains(destination.id()) {
                        break;
                    }
                    self.back_stack_map_put(destination.id(), saved.first().map(NavBackStackEntryState::id));
                }
            }
            if let Some(first) = saved.first() {
                if let Some(first_destination) = self.find_destination_id(first.destination_id(), None) {
                    for destination in NavController::start_chain(&first_destination) {
                        if self.back_stack_map_contains(destination.id()) {
                            break;
                        }
                        self.back_stack_map_put(destination.id(), Some(first.id()));
                    }
                }
                let first_id = first.id();
                if self.0.back_stack_map.borrow().iter().any(|(_, v)| v.as_deref() == Some(first_id.as_str())) {
                    let mut states = self.0.back_stack_states.borrow_mut();
                    match states.iter_mut().find(|(k, _)| *k == first_id) {
                        Some(slot) => slot.1 = saved.clone(),
                        None => states.push((first_id, saved.clone())),
                    }
                }
            }
        }
        popped.get()
    }

    fn pop_back_stack_internal_with(
        &self,
        navigator: &Rc<dyn Navigator>,
        pop_up_to: &NavBackStackEntry,
        save_state: bool,
        handler: EntryHandler,
    ) {
        *self.0.pop_from_back_stack_handler.borrow_mut() = Some(handler);
        navigator.pop_back_stack_to(pop_up_to, save_state);
        *self.0.pop_from_back_stack_handler.borrow_mut() = None;
    }

    fn pop_back_stack_from_navigator(&self, pop_up_to: &NavBackStackEntry, on_complete: impl FnOnce()) {
        let queue = self.back_queue();
        let Some(pop_index) = index_of(&queue, pop_up_to) else { return };
        if pop_index + 1 != queue.len() {
            self.pop_back_stack_internal_id(queue[pop_index + 1].destination().id(), true, false);
        }
        self.pop_entry_from_back_stack(pop_up_to, false, &mut Vec::new());
        on_complete();
        self.dispatch_on_destination_changed();
    }

    fn pop_entry_from_back_stack(
        &self,
        pop_up_to: &NavBackStackEntry,
        save_state: bool,
        saved_state: &mut Vec<NavBackStackEntryState>,
    ) {
        let entry = self.0.back_queue.borrow().last().cloned().expect("an entry");
        assert!(
            entry == *pop_up_to,
            "Attempted to pop {:?}, which is not the top of the back stack ({:?})",
            pop_up_to.destination(),
            entry.destination()
        );
        self.0.back_queue.borrow_mut().pop();
        let navigator = self.navigator_provider().get_navigator(&entry.destination().navigator_name());
        let state = self.existing_navigator_state(&navigator);
        let transitioning = state.is_some_and(|s| s.transitions_in_progress().value().contains(&entry))
            || self.parent_child_count(&entry).is_some();
        if entry.lifecycle().current_state().is_at_least(LifecycleState::Created) {
            if save_state {
                entry.set_max_lifecycle(LifecycleState::Created);
                saved_state.insert(0, NavBackStackEntryState::new(&entry));
            }
            if !transitioning {
                entry.set_max_lifecycle(LifecycleState::Destroyed);
                self.unlink_child_from_parent(&entry);
            } else {
                entry.set_max_lifecycle(LifecycleState::Created);
            }
        }
        if !save_state
            && !transitioning
            && let Some(p) = self.view_model_store_provider()
        {
            p.clear(&entry.id());
        }
    }

    // ---- Clearing saved stacks ----

    /// Clears the stack saved for `route` (an id, a route, a typed route's value; Kotlin's `clearBackStack`): its
    /// entries' saved states and ViewModels go. Whether one was cleared.
    pub fn clear_back_stack(&self, route: impl IntoDestinationTarget) -> bool {
        let cleared = match route.into_destination_target() {
            DestinationTarget::Id(id) => self.clear_back_stack_internal_id(id),
            DestinationTarget::Route(r) => self.clear_back_stack_internal_route(&r),
            DestinationTarget::Object(o) => {
                let route = self.generate_route_filled(&*o);
                self.clear_back_stack_internal_route(&route)
            }
        };
        cleared && self.dispatch_on_destination_changed()
    }

    /// Clears the stack saved for `T` (Compose's `clearBackStack<T>()`).
    ///
    /// Kotlin: `clearBackStack<T>()`
    pub fn clear_back_stack_of<T: Route>(&self) -> bool {
        self.clear_back_stack_class(RouteClass::of::<T>())
    }

    /// Kotlin's `clearBackStack(route: KClass<T>)`.
    pub fn clear_back_stack_class(&self, route: RouteClass) -> bool {
        self.clear_back_stack(route.hash_code())
    }

    fn set_navigating(&self, value: bool) {
        for s in self.all_navigator_states() {
            s.set_is_navigating(value);
        }
    }

    fn clear_back_stack_internal_id(&self, destination_id: i32) -> bool {
        self.set_navigating(true);
        let restored = self.restore_state_internal_id(
            destination_id,
            None,
            Some(&nav_options(|b| b.set_restore_state(true))),
            None,
        );
        self.set_navigating(false);
        restored && self.pop_back_stack_internal_id(destination_id, true, false)
    }

    fn clear_back_stack_internal_route(&self, route: &str) -> bool {
        self.set_navigating(true);
        let restored = self.restore_state_internal_route(route);
        self.set_navigating(false);
        restored && self.pop_back_stack_internal_route(route, true, false)
    }

    // ---- Dispatching ----

    /// Pops the graphs' entries left on top, moves the entries' lifecycles, tells the listeners and the flows
    /// (Kotlin's `dispatchOnDestinationChanged`): whether there is an entry.
    fn dispatch_on_destination_changed(&self) -> bool {
        loop {
            let last = self.0.back_queue.borrow().last().cloned();
            match last {
                Some(e) if e.destination().is_graph() => self.pop_entry_from_back_stack(&e, false, &mut Vec::new()),
                _ => break,
            }
        }
        let last = self.current_back_stack_entry();
        if let Some(l) = &last {
            self.0.back_stack_entries_to_dispatch.borrow_mut().push(l.clone());
        }
        self.0.dispatch_reentrant_count.set(self.0.dispatch_reentrant_count.get() + 1);
        self.update_back_stack_lifecycle();
        self.0.dispatch_reentrant_count.set(self.0.dispatch_reentrant_count.get() - 1);
        if self.0.dispatch_reentrant_count.get() == 0 {
            let dispatch: Vec<NavBackStackEntry> =
                std::mem::take(&mut *self.0.back_stack_entries_to_dispatch.borrow_mut());
            for entry in dispatch {
                let listeners = self.0.on_destination_changed_listeners.borrow().clone();
                for listener in listeners {
                    listener.on_destination_changed(self, &entry.destination(), entry.arguments().as_ref());
                }
                self.0.current_back_stack_entry_flow.try_emit(entry);
            }
            self.0.current_back_stack.set_value(self.back_queue());
            self.0.visible_entries.set_value(self.populate_visible_entries());
        }
        last.is_some()
    }

    fn is_transitioning(&self, entry: &NavBackStackEntry) -> bool {
        let navigator = self.navigator_provider().get_navigator(&entry.destination().navigator_name());
        self.existing_navigator_state(&navigator).is_some_and(|s| s.transitions_in_progress().value().contains(entry))
    }

    /// Moves each entry's maximum lifecycle (Kotlin's `updateBackStackLifecycle`): the top destination and the
    /// graphs it is in resumed (supporting panes' below too), floating windows' below started, the others created.
    fn update_back_stack_lifecycle(&self) {
        let back_stack = self.back_queue();
        let Some(top) = back_stack.last() else { return };
        let topmost = top.destination();
        let mut next_resumed: Vec<NavDestination> = vec![topmost.clone()];
        if topmost.is_supporting_pane() {
            let only_floating = topmost.is_floating_window();
            for entry in back_stack.iter().rev().skip(1) {
                let d = entry.destination();
                if only_floating && !d.is_floating_window() && !d.is_graph() {
                    break;
                }
                next_resumed.push(d.clone());
                if !d.is_supporting_pane() && !d.is_graph() {
                    break;
                }
            }
        }
        let mut next_started: Vec<NavDestination> = Vec::new();
        if next_resumed.last().is_some_and(NavDestination::is_floating_window) {
            for entry in back_stack.iter().rev() {
                let d = entry.destination();
                next_started.push(d.clone());
                if !d.is_floating_window() && !d.is_supporting_pane() && !d.is_graph() {
                    break;
                }
            }
        }
        let mut upward: Vec<(NavBackStackEntry, LifecycleState)> = Vec::new();
        for entry in back_stack.iter().rev() {
            let current_max = entry.max_lifecycle();
            let destination = entry.destination();
            if next_resumed.first().is_some_and(|d| d.id() == destination.id()) {
                if current_max != LifecycleState::Resumed {
                    let transitioning = self.is_transitioning(entry);
                    if !transitioning && self.parent_child_count(entry) != Some(0) {
                        upward.push((entry.clone(), LifecycleState::Resumed));
                    } else {
                        upward.push((entry.clone(), LifecycleState::Started));
                    }
                }
                if next_started.first().is_some_and(|d| d.id() == destination.id()) {
                    next_started.remove(0);
                }
                next_resumed.remove(0);
                if let Some(p) = destination.parent() {
                    next_resumed.push(p.as_destination());
                }
            } else if next_started.first().is_some_and(|d| d.id() == destination.id()) {
                let started = next_started.remove(0);
                if current_max == LifecycleState::Resumed {
                    entry.set_max_lifecycle(LifecycleState::Started);
                } else if current_max != LifecycleState::Started {
                    upward.push((entry.clone(), LifecycleState::Started));
                }
                if let Some(p) = started.parent() {
                    let p = p.as_destination();
                    if !next_started.contains(&p) {
                        next_started.push(p);
                    }
                }
            } else {
                entry.set_max_lifecycle(LifecycleState::Created);
            }
        }
        for entry in &back_stack {
            match upward.iter().find(|(e, _)| e == entry) {
                Some((_, state)) => entry.set_max_lifecycle(*state),
                None => entry.update_state(),
            }
        }
    }

    fn populate_visible_entries(&self) -> Vec<NavBackStackEntry> {
        let mut entries: Vec<NavBackStackEntry> = Vec::new();
        for state in self.all_navigator_states() {
            for entry in state.transitions_in_progress().value() {
                if !entries.contains(&entry) && !entry.max_lifecycle().is_at_least(LifecycleState::Started) {
                    entries.push(entry);
                }
            }
        }
        for entry in self.back_queue() {
            if !entries.contains(&entry) && entry.max_lifecycle().is_at_least(LifecycleState::Started) {
                entries.push(entry);
            }
        }
        entries.retain(|e| !e.destination().is_graph());
        entries
    }

    // ---- Finding destinations ----

    /// The destination of id `destination_id` from the current one's graph (Kotlin's `findDestination(id)`).
    pub fn find_destination_id(
        &self,
        destination_id: i32,
        matching_dest: Option<&NavDestination>,
    ) -> Option<NavDestination> {
        let graph = self.0.graph.borrow().clone()?;
        if graph.id() == destination_id {
            match matching_dest {
                Some(m) => {
                    if graph.as_destination() == *m && m.parent().is_none() {
                        return Some(graph.as_destination());
                    }
                }
                None => return Some(graph.as_destination()),
            }
        }
        let current = self.current_destination().unwrap_or_else(|| graph.as_destination());
        self.find_destination_comprehensive(&current, destination_id, false, matching_dest)
    }

    fn find_destination_comprehensive(
        &self,
        destination: &NavDestination,
        destination_id: i32,
        search_children: bool,
        matching_dest: Option<&NavDestination>,
    ) -> Option<NavDestination> {
        if destination.id() == destination_id {
            match matching_dest {
                Some(m) => {
                    if destination == m && destination.parent() == m.parent() {
                        return Some(destination.clone());
                    }
                }
                None => return Some(destination.clone()),
            }
        }
        let current_graph = destination.as_graph().or_else(|| destination.parent()).expect("a graph");
        current_graph.find_node_comprehensive(
            destination_id,
            Some(&current_graph.as_destination()),
            search_children,
            matching_dest,
        )
    }

    /// The destination of `route` (a pattern or a path; Kotlin's `findDestination(route)`).
    pub fn find_destination(&self, route: &str) -> Option<NavDestination> {
        let graph = self.0.graph.borrow().clone()?;
        if graph.route().as_deref() == Some(route) || graph.match_route(route).is_some() {
            return Some(graph.as_destination());
        }
        self.get_top_graph().find_node(Some(route))
    }

    fn get_top_graph(&self) -> NavGraph {
        let current = self.current_destination().unwrap_or_else(|| self.graph().as_destination());
        current.as_graph().or_else(|| current.parent()).expect("a graph")
    }

    fn generate_route_filled(&self, route: &dyn AnyRoute) -> String {
        let id = hash_code_of(route.route_descriptor());
        let destination =
            self.find_destination_comprehensive(&self.graph().as_destination(), id, true, None).unwrap_or_else(|| {
                panic!(
                    "Destination with route {} cannot be found in navigation graph {:?}",
                    route.route_type_name(),
                    self.0.graph.borrow()
                )
            });
        generate_route_with_args(route, &destination.arguments().types())
    }

    // ---- Navigating ----

    /// Navigates to `route` — a route (`"detail/42"`, a pattern's path), a typed route's value, a deep link's
    /// [`NavUri`], a [`NavDeepLinkRequest`] — (Kotlin's `navigate(route)`).
    ///
    /// Kotlin: `navigate(route: String)`
    ///
    /// # Panics
    /// If no destination of the graph matches it, or before a graph is set (Kotlin's IllegalArgumentException).
    pub fn navigate(&self, route: impl IntoNavTarget) {
        self.navigate_options(route, None, None);
    }

    /// The same with `nav_options` and `navigator_extras` (Kotlin's `navigate(route, navOptions,
    /// navigatorExtras)`).
    ///
    /// Kotlin: `navigate(route: String, navOptions: NavOptions? = null, navigatorExtras: Navigator.Extras? = null)`
    pub fn navigate_options(
        &self,
        route: impl IntoNavTarget,
        nav_options: Option<NavOptions>,
        navigator_extras: Option<Rc<dyn NavigatorExtras>>,
    ) {
        let extras = navigator_extras.as_deref();
        match route.into_nav_target() {
            NavTarget::Route(r) => self.navigate_route(&r, nav_options.as_ref(), extras),
            NavTarget::Object(o) => {
                let route = self.generate_route_filled(&*o);
                self.navigate_route(&route, nav_options.as_ref(), extras);
            }
            NavTarget::Uri(u) => {
                self.navigate_request(&NavDeepLinkRequest::new(Some(u), None, None), nav_options.as_ref(), extras)
            }
            NavTarget::Request(r) => self.navigate_request(&r, nav_options.as_ref(), extras),
        }
    }

    /// Navigates to `route` with the options `builder` sets (Kotlin's `navigate(route) { popUpTo(…) {…};
    /// launchSingleTop = true }`).
    ///
    /// Kotlin: `navigate(route: String, builder: NavOptionsBuilder.() -> Unit)`
    pub fn navigate_with(&self, route: impl IntoNavTarget, builder: impl FnOnce(&mut NavOptionsBuilder)) {
        let options = nav_options(builder);
        match route.into_nav_target() {
            NavTarget::Route(r) => self.navigate_route(&r, Some(&options), None),
            NavTarget::Object(o) => {
                let route = self.generate_route_filled(&*o);
                self.navigate_route(&route, Some(&options), None);
            }
            NavTarget::Uri(u) => {
                self.navigate_request(&NavDeepLinkRequest::new(Some(u), None, None), Some(&options), None)
            }
            NavTarget::Request(r) => self.navigate_request(&r, Some(&options), None),
        }
    }

    fn navigate_route(&self, route: &str, nav_options: Option<&NavOptions>, extras: Option<&dyn NavigatorExtras>) {
        assert!(
            self.0.graph.borrow().is_some(),
            "Cannot navigate to {route}. Navigation graph has not been set for NavController {self:?}."
        );
        let top = self.get_top_graph();
        let matched = top.match_route_comprehensive(route, true, true, &top.as_destination());
        match matched {
            Some(m) => {
                let destination = m.destination();
                let args = destination.add_in_default_args(m.matching_args().as_ref()).unwrap_or_default();
                self.navigate_node(&destination, Some(args), nav_options, extras);
            }
            None => panic!(
                "Navigation destination that matches route {route} cannot be found in the navigation graph {:?}",
                self.0.graph.borrow()
            ),
        }
    }

    fn navigate_request(
        &self,
        request: &NavDeepLinkRequest,
        nav_options: Option<&NavOptions>,
        extras: Option<&dyn NavigatorExtras>,
    ) {
        assert!(
            self.0.graph.borrow().is_some(),
            "Cannot navigate to {request}. Navigation graph has not been set for NavController {self:?}."
        );
        let top = self.get_top_graph();
        let matched = top.match_deep_link_comprehensive(request, true, true, &top.as_destination());
        match matched {
            Some(m) => {
                let destination = m.destination();
                let args = destination.add_in_default_args(m.matching_args().as_ref()).unwrap_or_default();
                self.navigate_node(&destination, Some(args), nav_options, extras);
            }
            None => panic!(
                "Navigation destination that matches request {request} cannot be found in the navigation graph {:?}",
                self.0.graph.borrow()
            ),
        }
    }

    fn navigate_node(
        &self,
        node: &NavDestination,
        args: Option<SavedState>,
        nav_options: Option<&NavOptions>,
        extras: Option<&dyn NavigatorExtras>,
    ) {
        self.set_navigating(true);
        let mut popped = false;
        let mut launch_single_top = false;
        let navigated = Rc::new(Cell::new(false));
        if let Some(o) = nav_options {
            if let Some(r) = o.pop_up_to_route() {
                popped =
                    self.pop_back_stack_internal_route(&r, o.is_pop_up_to_inclusive(), o.should_pop_up_to_save_state());
            } else if let Some(c) = o.pop_up_to_route_class() {
                popped = self.pop_back_stack_internal_id(
                    c.hash_code(),
                    o.is_pop_up_to_inclusive(),
                    o.should_pop_up_to_save_state(),
                );
            } else if let Some(obj) = o.pop_up_to_route_object() {
                popped = self.pop_back_stack_internal_object(
                    &*obj,
                    o.is_pop_up_to_inclusive(),
                    o.should_pop_up_to_save_state(),
                );
            } else if o.pop_up_to_id() != -1 {
                popped = self.pop_back_stack_internal_id(
                    o.pop_up_to_id(),
                    o.is_pop_up_to_inclusive(),
                    o.should_pop_up_to_save_state(),
                );
            }
        }
        let final_args = node.add_in_default_args(args.as_ref());
        if nav_options.is_some_and(NavOptions::should_restore_state) && self.back_stack_map_contains(node.id()) {
            navigated.set(self.restore_state_internal_id(node.id(), final_args.clone(), nav_options, extras));
        } else {
            launch_single_top = nav_options.is_some_and(NavOptions::should_launch_single_top)
                && self.launch_single_top_internal(node, args.as_ref());
            if !launch_single_top {
                let entry = NavBackStackEntry::create(
                    node.clone(),
                    final_args.clone(),
                    self.host_lifecycle_state(),
                    self.view_model_store_provider(),
                    None,
                    None,
                );
                let navigator = self.navigator_provider().get_navigator(&node.navigator_name());
                let (n, me, node2, fa) = (navigated.clone(), self.0.me.clone(), node.clone(), final_args.clone());
                self.navigate_internal(
                    &navigator,
                    vec![entry],
                    nav_options,
                    extras,
                    Rc::new(move |e: &NavBackStackEntry| {
                        n.set(true);
                        if let Some(c) = NavController::from_weak(&me) {
                            c.add_entry_to_back_stack(&node2, fa.as_ref(), e, &[]);
                        }
                    }),
                );
            }
        }
        self.set_navigating(false);
        if popped || navigated.get() || launch_single_top {
            self.dispatch_on_destination_changed();
        } else {
            self.update_back_stack_lifecycle();
        }
    }

    fn navigate_internal(
        &self,
        navigator: &Rc<dyn Navigator>,
        entries: Vec<NavBackStackEntry>,
        nav_options: Option<&NavOptions>,
        extras: Option<&dyn NavigatorExtras>,
        handler: EntryHandler,
    ) {
        *self.0.add_to_back_stack_handler.borrow_mut() = Some(handler);
        navigator.navigate(entries, nav_options, extras);
        *self.0.add_to_back_stack_handler.borrow_mut() = None;
    }

    fn launch_single_top_internal(&self, node: &NavDestination, args: Option<&SavedState>) -> bool {
        let current = self.current_back_stack_entry();
        let queue = self.back_queue();
        let Some(node_index) = queue.iter().rposition(|e| e.destination().same(node)) else { return false };
        if let Some(graph) = node.as_graph() {
            let child_ids: Vec<i32> = graph.child_hierarchy().iter().map(NavDestination::id).collect();
            if queue.len() - node_index != child_ids.len() {
                return false;
            }
            let queue_ids: Vec<i32> = queue[node_index..].iter().map(|e| e.destination().id()).collect();
            if queue_ids != child_ids {
                return false;
            }
        } else if current.is_none_or(|c| c.destination().id() != node.id()) {
            return false;
        }
        let mut temp: Vec<NavBackStackEntry> = Vec::new();
        while self.0.back_queue.borrow().len() > node_index {
            let old = self.0.back_queue.borrow_mut().pop().expect("an entry");
            self.unlink_child_from_parent(&old);
            let new_entry = NavBackStackEntry::create_from(&old, old.destination().add_in_default_args(args));
            temp.insert(0, new_entry);
        }
        for new_entry in &temp {
            if let Some(parent) = new_entry.destination().parent() {
                let new_parent = self.get_back_stack_entry_id(parent.id());
                self.link_child_to_parent(new_entry, &new_parent);
            }
            self.0.back_queue.borrow_mut().push(new_entry.clone());
        }
        for new_entry in &temp {
            let navigator = self.navigator_provider().get_navigator(&new_entry.destination().navigator_name());
            navigator.on_launch_single_top(new_entry);
        }
        true
    }

    fn take_saved_stack(&self, id: i32) -> Option<Vec<NavBackStackEntryState>> {
        let back_stack_id = self.0.back_stack_map.borrow().iter().find(|(k, _)| *k == id).and_then(|(_, v)| v.clone());
        self.0.back_stack_map.borrow_mut().retain(|(_, v)| *v != back_stack_id);
        let mut states = self.0.back_stack_states.borrow_mut();
        let i = states.iter().position(|(k, _)| Some(k) == back_stack_id.as_ref())?;
        Some(states.remove(i).1)
    }

    fn restore_state_internal_id(
        &self,
        id: i32,
        args: Option<SavedState>,
        nav_options: Option<&NavOptions>,
        extras: Option<&dyn NavigatorExtras>,
    ) -> bool {
        if !self.back_stack_map_contains(id) {
            return false;
        }
        let state = self.take_saved_stack(id);
        let entries = self.instantiate_back_stack(state);
        self.execute_restore_state(entries, args, nav_options, extras)
    }

    fn restore_state_internal_route(&self, route: &str) -> bool {
        let id = string_hash(&create_route(Some(route)));
        if self.back_stack_map_contains(id) {
            return self.restore_state_internal_id(id, None, None, None);
        }
        let matching = self.find_destination(route).unwrap_or_else(|| {
            panic!(
                "Restore State failed: route {route} cannot be found from the current destination {:?}",
                self.current_destination()
            )
        });
        let state = self.take_saved_stack(matching.id());
        let matching_deep_link = matching.match_route(route).expect("the route matches its destination");
        let first_args = state.as_ref().and_then(|s| s.first()).and_then(NavBackStackEntryState::args);
        if !matching_deep_link.has_matching_args(first_args.as_ref()) {
            return false;
        }
        let entries = self.instantiate_back_stack(state);
        self.execute_restore_state(entries, None, None, None)
    }

    fn execute_restore_state(
        &self,
        entries: Vec<NavBackStackEntry>,
        args: Option<SavedState>,
        nav_options: Option<&NavOptions>,
        extras: Option<&dyn NavigatorExtras>,
    ) -> bool {
        let mut grouped: Vec<Vec<NavBackStackEntry>> = Vec::new();
        for entry in entries.iter().filter(|e| !e.destination().is_graph()) {
            match grouped.last_mut() {
                Some(g)
                    if g.last()
                        .is_some_and(|l| l.destination().navigator_name() == entry.destination().navigator_name()) =>
                {
                    g.push(entry.clone())
                }
                _ => grouped.push(vec![entry.clone()]),
            }
        }
        let navigated = Rc::new(Cell::new(false));
        let entries = Rc::new(entries);
        for group in grouped {
            let navigator = self.navigator_provider().get_navigator(&group[0].destination().navigator_name());
            let last_navigated_index = Rc::new(Cell::new(0usize));
            let (n, me, all, lni, a) =
                (navigated.clone(), self.0.me.clone(), entries.clone(), last_navigated_index.clone(), args.clone());
            self.navigate_internal(
                &navigator,
                group,
                nav_options,
                extras,
                Rc::new(move |entry: &NavBackStackEntry| {
                    n.set(true);
                    let restored: Vec<NavBackStackEntry> = match index_of(&all, entry) {
                        Some(i) => {
                            let slice = all[lni.get()..=i].to_vec();
                            lni.set(i + 1);
                            slice
                        }
                        None => Vec::new(),
                    };
                    if let Some(c) = NavController::from_weak(&me) {
                        c.add_entry_to_back_stack(&entry.destination(), a.as_ref(), entry, &restored);
                    }
                }),
            );
        }
        navigated.get()
    }

    fn instantiate_back_stack(&self, state: Option<Vec<NavBackStackEntryState>>) -> Vec<NavBackStackEntry> {
        let mut back_stack = Vec::new();
        let mut current = self.current_destination().unwrap_or_else(|| self.graph().as_destination());
        for s in state.unwrap_or_default() {
            let node =
                self.find_destination_comprehensive(&current, s.destination_id(), true, None).unwrap_or_else(|| {
                    panic!(
                        "Restore State failed: destination {} cannot be found from the current destination {current:?}",
                        s.destination_id()
                    )
                });
            back_stack.push(s.instantiate(node.clone(), self.host_lifecycle_state(), self.view_model_store_provider()));
            current = node;
        }
        back_stack
    }

    /// Adds `entry` with the entries of the graphs on the way that aren't there (Kotlin's `addEntryToBackStack`).
    fn add_entry_to_back_stack(
        &self,
        node: &NavDestination,
        final_args: Option<&SavedState>,
        entry: &NavBackStackEntry,
        restored: &[NavBackStackEntry],
    ) {
        let new_dest = entry.destination();
        if !new_dest.is_floating_window() {
            loop {
                let last = self.0.back_queue.borrow().last().cloned();
                match last {
                    Some(l)
                        if l.destination().is_floating_window()
                            && self.pop_back_stack_internal_id(l.destination().id(), true, false) => {}
                    _ => break,
                }
            }
        }
        let mut hierarchy: Vec<NavBackStackEntry> = Vec::new();
        let mut destination: Option<NavDestination> = Some(new_dest.clone());
        if node.is_graph() {
            loop {
                let parent = destination.as_ref().expect("a destination").parent();
                if let Some(parent) = &parent {
                    let parent_dest = parent.as_destination();
                    let e =
                        restored.iter().rev().find(|r| r.destination() == parent_dest).cloned().unwrap_or_else(|| {
                            NavBackStackEntry::create(
                                parent_dest.clone(),
                                final_args.cloned(),
                                self.host_lifecycle_state(),
                                self.view_model_store_provider(),
                                None,
                                None,
                            )
                        });
                    hierarchy.insert(0, e);
                    let last = self.0.back_queue.borrow().last().cloned();
                    if let Some(l) = last.filter(|l| l.destination().same(&parent_dest)) {
                        self.pop_entry_from_back_stack(&l, false, &mut Vec::new());
                    }
                }
                destination = parent.map(|p| p.as_destination());
                match &destination {
                    Some(d) if !d.same(node) => {}
                    _ => break,
                }
            }
        }
        destination = Some(hierarchy.first().map_or_else(|| new_dest.clone(), |h| h.destination()));
        while let Some(d) = destination.clone() {
            if self.find_destination_id(d.id(), Some(&d)).is_some_and(|f| f.same(&d)) {
                break;
            }
            let parent = d.parent();
            if let Some(parent) = &parent {
                let args = final_args.filter(|a| !a.read(|r| r.is_empty()));
                let parent_dest = parent.as_destination();
                let e = restored.iter().rev().find(|r| r.destination() == parent_dest).cloned().unwrap_or_else(|| {
                    NavBackStackEntry::create(
                        parent_dest.clone(),
                        parent_dest.add_in_default_args(args),
                        self.host_lifecycle_state(),
                        self.view_model_store_provider(),
                        None,
                        None,
                    )
                });
                hierarchy.insert(0, e);
            }
            destination = parent.map(|p| p.as_destination());
        }
        let overlapping = hierarchy.first().map_or_else(|| new_dest.clone(), |h| h.destination());
        loop {
            let last = self.0.back_queue.borrow().last().cloned();
            match last {
                Some(l) if l.destination().as_graph().is_some_and(|g| g.node(overlapping.id()).is_none()) => {
                    self.pop_entry_from_back_stack(&l, false, &mut Vec::new());
                }
                _ => break,
            }
        }
        let graph = self.graph();
        let first = self.0.back_queue.borrow().first().cloned().or_else(|| hierarchy.first().cloned());
        if first.is_none_or(|f| f.destination() != graph.as_destination()) {
            let graph_dest = graph.as_destination();
            let e = restored.iter().rev().find(|r| r.destination() == graph_dest).cloned().unwrap_or_else(|| {
                NavBackStackEntry::create(
                    graph_dest.clone(),
                    graph_dest.add_in_default_args(final_args),
                    self.host_lifecycle_state(),
                    self.view_model_store_provider(),
                    None,
                    None,
                )
            });
            hierarchy.insert(0, e);
        }
        for e in &hierarchy {
            let navigator = self.navigator_provider().get_navigator(&e.destination().navigator_name());
            let state = self.existing_navigator_state(&navigator).unwrap_or_else(|| {
                panic!("NavigatorBackStack for {} should already be created", node.navigator_name())
            });
            state.base_push(e);
        }
        {
            let mut queue = self.0.back_queue.borrow_mut();
            queue.extend(hierarchy.iter().cloned());
            queue.push(entry.clone());
        }
        for e in hierarchy.iter().chain(std::iter::once(entry)) {
            if let Some(parent) = e.destination().parent() {
                let parent_entry = self.get_back_stack_entry_id(parent.id());
                self.link_child_to_parent(e, &parent_entry);
            }
        }
    }

    /// Goes up (Kotlin's `navigateUp`): pops when there is more than one destination on the stack; with one
    /// there is nothing to go up to on the desktop (Android starts the parent's activity): false.
    pub fn navigate_up(&self) -> bool {
        if self.destination_count_on_back_stack() == 1 { false } else { self.pop_back_stack() }
    }

    /// How many entries of destinations that aren't graphs it has (what a NavHost's back handler is enabled by).
    pub fn destination_count_on_back_stack(&self) -> usize {
        self.0.back_queue.borrow().iter().filter(|e| !e.destination().is_graph()).count()
    }

    // ---- Deep links ----

    /// Opens the deep link `request` as an app opened with it (Kotlin's `handleDeepLink(request)`): the stack starts
    /// over with the graphs on the way to its destination and their start destinations below it, so that going
    /// back leads up. Whether a destination answers it.
    pub fn handle_deep_link(&self, request: &NavDeepLinkRequest) -> bool {
        let top = self.get_top_graph();
        let Some(m) = top.match_deep_link_comprehensive(request, true, true, &top.as_destination()) else {
            return false;
        };
        self.handle_deep_link_match(&m.destination(), m.matching_args())
    }

    /// Opens `link` as [`handle_deep_link`](Self::handle_deep_link) does: a deep link's URI, or — the web's address
    /// fragment, C++ Kompoz's — a route's path (`item/42`, a graph's route its start destination).
    pub fn handle_deep_link_uri(&self, link: &str) -> bool {
        if self.0.graph.borrow().is_none() {
            return false;
        }
        if self.handle_deep_link(&NavDeepLinkRequestBuilder::from_uri(NavUri::new(link)).build()) {
            return true;
        }
        let top = self.get_top_graph();
        match top.match_route_comprehensive(link, true, true, &top.as_destination()) {
            Some(m) => self.handle_deep_link_match(&m.destination(), m.matching_args()),
            None => false,
        }
    }

    /// Whether `link` would be handled (a deep link's URI or a route's path of its graph).
    pub fn answers_deep_link(&self, link: &str) -> bool {
        if self.0.graph.borrow().is_none() {
            return false;
        }
        let top = self.get_top_graph();
        let request = NavDeepLinkRequestBuilder::from_uri(NavUri::new(link)).build();
        top.match_deep_link_comprehensive(&request, true, true, &top.as_destination()).is_some()
            || top.match_route_comprehensive(link, true, true, &top.as_destination()).is_some()
    }

    fn handle_deep_link_match(&self, destination: &NavDestination, matching_args: Option<SavedState>) -> bool {
        let deep_link = destination.build_deep_link_ids(None);
        let global_args = saved_state(|w| {
            if let Some(a) = destination.add_in_default_args(matching_args.as_ref()) {
                w.put_all(&a);
            }
        });
        let args: Vec<SavedState> = deep_link.iter().map(|_| crate::saved_state::copy(&global_args)).collect();
        self.handle_deep_link_ids(&deep_link, &args, true)
    }

    fn handle_deep_link_ids(&self, deep_link: &[i32], args: &[SavedState], new_task: bool) -> bool {
        if new_task {
            if !self.0.back_queue.borrow().is_empty() {
                self.pop_back_stack_internal_id(self.graph().id(), true, false);
            }
            for (index, destination_id) in deep_link.iter().enumerate() {
                let node = self.find_destination_id(*destination_id, None).unwrap_or_else(|| {
                    panic!(
                        "Deep Linking failed: destination {destination_id} cannot be found from the current destination \
                         {:?}",
                        self.current_destination()
                    )
                });
                let current_parent = self.current_destination().and_then(|d| d.parent()).map(|p| p.as_destination());
                let changing_graphs =
                    node.is_graph() && !node.hierarchy().iter().any(|h| Some(h) == current_parent.as_ref());
                let start_id = self.graph().find_start_destination().id();
                let options = nav_options(|b| {
                    if changing_graphs && DEEP_LINK_SAVE_STATE.with(Cell::get) {
                        b.pop_up_to_id_with(start_id, |p| p.set_save_state(true));
                    }
                });
                self.navigate_node(&node, Some(args[index].clone()), Some(&options), None);
            }
            self.0.deep_link_handled.set(true);
            return true;
        }
        let mut graph = self.0.graph.borrow().clone();
        for (i, destination_id) in deep_link.iter().enumerate() {
            let node = if i == 0 {
                graph.as_ref().map(NavGraph::as_destination)
            } else {
                graph.as_ref().and_then(|g| g.find_node_id(*destination_id))
            };
            let node = node.unwrap_or_else(|| {
                panic!("Deep Linking failed: destination {destination_id} cannot be found in graph {graph:?}")
            });
            if i != deep_link.len() - 1 {
                if let Some(g) = node.as_graph() {
                    let mut g = g;
                    while let Some(next) = g.find_node_id(g.start_destination_id()).and_then(|n| n.as_graph()) {
                        g = next;
                    }
                    graph = Some(g);
                }
            } else {
                let options = NavOptions::builder().set_pop_up_to_id(self.graph().id(), true, false).build();
                self.navigate_node(&node, Some(args[i].clone()), Some(&options), None);
            }
        }
        self.0.deep_link_handled.set(true);
        true
    }

    /// Whether a deep link it was given has been handled (Kotlin's `deepLinkHandled`).
    pub fn deep_link_handled(&self) -> bool {
        self.0.deep_link_handled.get()
    }

    /// Whether deep links into another graph save the stack they replace (Kotlin's `enableDeepLinkSaveState`;
    /// true by default).
    pub fn enable_deep_link_save_state(save_state: bool) {
        DEEP_LINK_SAVE_STATE.with(|c| c.set(save_state));
    }

    /// The links the app is opened with and sent (kompoz-ui's [`DeepLinks`], the host's — Android's activity
    /// intents): one waiting when its graph is set is handled then (Kotlin's `checkDeepLinkHandled` of the
    /// activity's intent), and those sent later as they come (what an activity's `onNewIntent` does with
    /// `handleDeepLink`); a link it doesn't answer is left for another.
    pub fn set_deep_links(&self, deep_links: Option<DeepLinks>) {
        let owner = Rc::as_ptr(&self.0) as usize;
        if let Some(previous) = self.0.deep_links.borrow_mut().take() {
            previous.remove_handler(owner);
        }
        if let Some(links) = &deep_links {
            let me = self.0.me.clone();
            links.add_handler(owner, move |link: &str| match NavController::from_weak(&me) {
                Some(c) if c.answers_deep_link(link) => c.handle_deep_link_uri(link),
                _ => false,
            });
        }
        *self.0.deep_links.borrow_mut() = deep_links;
    }

    fn check_deep_link_handled(&self) -> bool {
        if self.0.deep_link_handled.get() {
            return false;
        }
        let links = self.0.deep_links.borrow().clone();
        let Some(links) = links else { return false };
        match links.take(|link| self.answers_deep_link(link)) {
            Some(link) => self.handle_deep_link_uri(&link),
            None => false,
        }
    }

    // ---- Saving ----

    /// Its state (Kotlin's `saveState`): its navigators', its back stack's entries', the stacks saved, whether a
    /// deep link was handled; `None` when there is nothing.
    pub fn save_state(&self) -> Option<SavedState> {
        let mut b: Option<SavedState> = None;
        let mut navigator_names: Vec<String> = Vec::new();
        let navigator_state = SavedState::new();
        if let Some(to_restore) = self.0.navigator_state_to_restore.borrow().as_ref() {
            for name in to_restore.read(|r| r.get_string_list_or_null(KEY_NAVIGATOR_STATE_NAMES)).unwrap_or_default() {
                if let Some(state) = to_restore.read(|r| r.get_saved_state_or_null(&name)) {
                    navigator_names.push(name.clone());
                    navigator_state.write(|w| w.put_saved_state(&name, &state));
                }
            }
        }
        for (name, navigator) in self.navigator_provider().navigators() {
            if let Some(saved) = navigator.on_save_state() {
                if !navigator_names.contains(&name) {
                    navigator_names.push(name.clone());
                }
                navigator_state.write(|w| w.put_saved_state(&name, &saved));
            }
        }
        if !navigator_names.is_empty() {
            navigator_state.write(|w| w.put_string_list(KEY_NAVIGATOR_STATE_NAMES, navigator_names.clone()));
            b = Some(saved_state(|w| w.put_saved_state(KEY_NAVIGATOR_STATE, &navigator_state)));
        }
        let queue = self.back_queue();
        if !queue.is_empty() {
            let back_stack: Vec<SavedState> =
                queue.iter().map(|e| NavBackStackEntryState::new(e).write_to_state()).collect();
            b.get_or_insert_with(SavedState::new).write(|w| w.put_saved_state_list(KEY_BACK_STACK, back_stack));
        } else if let Some(to_restore) = self.0.back_stack_to_restore.borrow().clone() {
            b.get_or_insert_with(SavedState::new).write(|w| w.put_saved_state_list(KEY_BACK_STACK, to_restore));
        }
        let map = self.0.back_stack_map.borrow().clone();
        if !map.is_empty() {
            let ids: Vec<i32> = map.iter().map(|(id, _)| *id).collect();
            let stack_ids: Vec<String> = map.iter().map(|(_, s)| s.clone().unwrap_or_default()).collect();
            b.get_or_insert_with(SavedState::new).write(|w| {
                w.put_int_array(KEY_BACK_STACK_DEST_IDS, ids);
                w.put_string_list(KEY_BACK_STACK_IDS, stack_ids);
            });
        }
        let states = self.0.back_stack_states.borrow().clone();
        if !states.is_empty() {
            let out = b.get_or_insert_with(SavedState::new);
            let mut ids: Vec<String> = Vec::new();
            for (id, entries) in &states {
                ids.push(id.clone());
                let list: Vec<SavedState> = entries.iter().map(NavBackStackEntryState::write_to_state).collect();
                out.write(|w| w.put_saved_state_list(&format!("{KEY_BACK_STACK_STATES_PREFIX}{id}"), list));
            }
            out.write(|w| w.put_string_list(KEY_BACK_STACK_STATES_IDS, ids));
        }
        if self.0.deep_link_handled.get() {
            b.get_or_insert_with(SavedState::new).write(|w| w.put_boolean(KEY_DEEP_LINK_HANDLED, true));
        }
        b
    }

    /// Takes back what [`save_state`](Self::save_state) gave (Kotlin's `restoreState`), before its graph is set:
    /// setting it restores the back stack.
    pub fn restore_state(&self, nav_state: Option<SavedState>) {
        let Some(nav_state) = nav_state else { return };
        nav_state.read(|r| {
            *self.0.navigator_state_to_restore.borrow_mut() = r.get_saved_state_or_null(KEY_NAVIGATOR_STATE);
            *self.0.back_stack_to_restore.borrow_mut() = r.get_saved_state_list_or_null(KEY_BACK_STACK);
            self.0.back_stack_states.borrow_mut().clear();
            if let (Some(dest_ids), Some(ids)) =
                (r.get_int_array_or_null(KEY_BACK_STACK_DEST_IDS), r.get_string_list_or_null(KEY_BACK_STACK_IDS))
            {
                for (index, id) in dest_ids.iter().enumerate() {
                    let stack_id = ids.get(index).filter(|s| !s.is_empty()).cloned();
                    self.back_stack_map_put(*id, stack_id);
                }
            }
            if let Some(state_ids) = r.get_string_list_or_null(KEY_BACK_STACK_STATES_IDS) {
                for id in state_ids {
                    if let Some(list) = r.get_saved_state_list_or_null(&format!("{KEY_BACK_STACK_STATES_PREFIX}{id}")) {
                        let entries: Vec<NavBackStackEntryState> =
                            list.iter().map(NavBackStackEntryState::from_state).collect();
                        self.0.back_stack_states.borrow_mut().push((id, entries));
                    }
                }
            }
            self.0.deep_link_handled.set(r.get_boolean_or_null(KEY_DEEP_LINK_HANDLED).unwrap_or(false));
        });
    }

    // ---- Owners ----

    fn host_lifecycle_state(&self) -> LifecycleState {
        if self.0.lifecycle_owner.borrow().is_none() {
            LifecycleState::Created
        } else {
            self.0.host_lifecycle_state.get()
        }
    }

    /// The owner whose lifecycle caps its entries' (Kotlin's `setLifecycleOwner`: a NavHost's
    /// `LocalLifecycleOwner`).
    pub fn set_lifecycle_owner(&self, owner: Rc<dyn LifecycleOwner>) {
        if self.0.lifecycle_owner.borrow().as_ref().is_some_and(|o| Rc::ptr_eq(o, &owner)) {
            return;
        }
        let observer = self.lifecycle_observer();
        if let Some(previous) = self.0.lifecycle_owner.borrow_mut().take() {
            previous.lifecycle().remove_observer(observer.clone());
        }
        *self.0.lifecycle_owner.borrow_mut() = Some(owner.clone());
        owner.lifecycle().add_observer(observer);
    }

    fn lifecycle_observer(&self) -> Rc<dyn LifecycleObserver> {
        if let Some(o) = self.0.lifecycle_observer.borrow().clone() {
            return o;
        }
        let me = self.0.me.clone();
        let observer: Rc<dyn LifecycleObserver> = Rc::new(move |_: &dyn LifecycleOwner, event: LifecycleEvent| {
            if let Some(c) = NavController::from_weak(&me) {
                c.0.host_lifecycle_state.set(event.target_state());
                if c.0.graph.borrow().is_some() {
                    for entry in c.back_queue() {
                        entry.handle_lifecycle_event(event);
                    }
                }
            }
        });
        *self.0.lifecycle_observer.borrow_mut() = Some(observer.clone());
        observer
    }

    /// The store its entries' ViewModels are kept in (Kotlin's `setViewModelStore`: a NavHost's
    /// `LocalViewModelStoreOwner`'s), under the key "androidx.navigation.NavControllerViewModel".
    ///
    /// # Panics
    /// After its graph was set.
    pub fn set_view_model_store(&self, view_model_store: kompoz_lifecycle::ViewModelStore) {
        self.set_view_model_store_provider(crate::entry::NavViewModelStoreProvider(Some(view_model_store)));
    }

    /// Where its entries' ViewModels are kept (what `set_view_model_store` sets).
    ///
    /// # Panics
    /// After its graph was set.
    pub fn set_view_model_store_provider(&self, provider: Rc<dyn NavViewModelStoreProvider>) {
        if self.0.view_model_store_provider.borrow().is_some() {
            return;
        }
        assert!(self.0.back_queue.borrow().is_empty(), "ViewModelStore should be set before setGraph call");
        *self.0.view_model_store_provider.borrow_mut() = Some(provider);
    }

    /// Where its entries' ViewModels are kept.
    pub fn view_model_store_provider(&self) -> Option<Rc<dyn NavViewModelStoreProvider>> {
        self.0.view_model_store_provider.borrow().clone()
    }

    /// The topmost entry of the graph of id `nav_graph_id` (Kotlin's `getViewModelStoreOwner`: its ViewModels the
    /// graph's).
    ///
    /// # Panics
    /// Without a ViewModel store, or with no such graph on the stack.
    pub fn get_view_model_store_owner(&self, nav_graph_id: i32) -> NavBackStackEntry {
        assert!(
            self.view_model_store_provider().is_some(),
            "You must call setViewModelStore() before calling getViewModelStoreOwner()."
        );
        let entry = self.get_back_stack_entry_id(nav_graph_id);
        assert!(
            entry.destination().is_graph(),
            "No NavGraph with ID {nav_graph_id} is on the NavController's back stack"
        );
        entry
    }
}

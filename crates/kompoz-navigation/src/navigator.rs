//! Navigators (Compose's `Navigator`, `NavigatorState`, `NavigatorProvider`, `NavGraphNavigator`,
//! `NoOpNavigator`): a navigator shows the destinations of its kind — composables, dialogs, graphs — and keeps
//! their part of the back stack in the [`NavigatorState`] its controller gives it.

use crate::destination::{NavDestination, NavGraph};
use crate::entry::NavBackStackEntry;
use crate::options::{NavOptions, nav_options};
use crate::saved_state::{SavedState, contains};
use kompoz_coroutines::{MutableStateFlow, StateFlow};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

/// What a navigator is given beyond the options (Compose's `Navigator.Extras`: shared elements, an activity's
/// options…), each navigator its own kind.
pub trait NavigatorExtras: Any {}

/// The base of every navigator (Kotlin's `Navigator`'s own fields): the state it is attached to.
#[derive(Default)]
pub struct NavigatorBase {
    state: RefCell<Option<NavigatorState>>,
    is_attached: Cell<bool>,
}

impl NavigatorBase {
    pub fn new() -> Self {
        Self::default()
    }

    /// The state it is attached to (Kotlin's `state`).
    ///
    /// # Panics
    /// Before it is attached.
    pub fn state(&self) -> NavigatorState {
        self.state.borrow().clone().expect("You cannot access the Navigator's state until the Navigator is attached")
    }

    /// Whether it is attached (Kotlin's `isAttached`).
    pub fn is_attached(&self) -> bool {
        self.is_attached.get()
    }

    /// Attaches it to `state` (what Kotlin's `onAttach` does).
    pub fn attach(&self, state: NavigatorState) {
        *self.state.borrow_mut() = Some(state);
        self.is_attached.set(true);
    }
}

/// A navigator (Compose's abstract `Navigator<D>`): its [`name`](Self::name) (Kotlin's `@Navigator.Name`), the
/// destinations it makes, and what it does to navigate and pop; the defaults are Kotlin's.
pub trait Navigator: Any {
    /// Its name: what its destinations' `navigator_name` is (Kotlin's `@Navigator.Name`: `"composable"`,
    /// `"dialog"`, `"navigation"`).
    fn name(&self) -> String;

    /// Its base (the state it is attached to).
    fn base(&self) -> &NavigatorBase;

    /// A new destination of it (Kotlin's `createDestination`).
    fn create_destination(&self) -> NavDestination;

    /// The state it is attached to (Kotlin's `state`).
    ///
    /// # Panics
    /// Before it is attached.
    fn state(&self) -> NavigatorState {
        self.base().state()
    }

    /// Whether it is attached (Kotlin's `isAttached`).
    fn is_attached(&self) -> bool {
        self.base().is_attached()
    }

    /// Attaches it to its controller's `state` (Kotlin's `onAttach`; an override calls `self.base().attach(state)`).
    fn on_attach(&self, state: NavigatorState) {
        self.base().attach(state);
    }

    /// Navigates to each of `entries`, pushing each onto its state (Kotlin's `navigate(entries, navOptions,
    /// navigatorExtras)`): by default each through [`navigate_to`](Self::navigate_to).
    fn navigate(
        &self,
        entries: Vec<NavBackStackEntry>,
        nav_options: Option<&NavOptions>,
        navigator_extras: Option<&dyn NavigatorExtras>,
    ) {
        for entry in entries {
            let destination = entry.destination();
            if destination.navigator_name() != self.name() {
                continue;
            }
            let args = entry.arguments();
            let navigated = self.navigate_to(&destination, args.as_ref(), nav_options, navigator_extras);
            let pushed = match navigated {
                None => continue,
                Some(d) if d.same(&destination) => entry,
                Some(d) => {
                    let args = d.add_in_default_args(entry.arguments().as_ref());
                    self.state().create_back_stack_entry(&d, args)
                }
            };
            self.state().push(&pushed);
        }
    }

    /// Navigates to `destination` (Kotlin's older `navigate(destination, args, navOptions, navigatorExtras)`): the
    /// destination navigated to, `None` for none; by default `destination` itself.
    fn navigate_to(
        &self,
        destination: &NavDestination,
        args: Option<&SavedState>,
        nav_options: Option<&NavOptions>,
        navigator_extras: Option<&dyn NavigatorExtras>,
    ) -> Option<NavDestination> {
        let _ = (args, nav_options, navigator_extras);
        Some(destination.clone())
    }

    /// `entry` replaces the top entry of its destination (`launchSingleTop`; Kotlin's `onLaunchSingleTop`).
    fn on_launch_single_top(&self, entry: &NavBackStackEntry) {
        let destination = entry.destination();
        if destination.navigator_name() != self.name() {
            return;
        }
        let options = nav_options(|b| b.set_launch_single_top(true));
        self.navigate_to(&destination, None, Some(&options), None);
        self.state().on_launch_single_top(entry);
    }

    /// Pops its back stack down to and with `pop_up_to` (Kotlin's `popBackStack(popUpTo, savedState)`): by default
    /// one [`pop_back_stack`](Self::pop_back_stack) at a time while it says yes, then the state popped.
    ///
    /// # Panics
    /// If `pop_up_to` isn't on its back stack.
    fn pop_back_stack_to(&self, pop_up_to: &NavBackStackEntry, saved_state: bool) {
        let back_stack = self.state().back_stack().value();
        assert!(
            back_stack.contains(pop_up_to),
            "popBackStack was called with {pop_up_to:?} which does not exist in back stack {back_stack:?}"
        );
        let mut last_popped: Option<NavBackStackEntry> = None;
        for entry in back_stack.iter().rev() {
            if !self.pop_back_stack() {
                break;
            }
            last_popped = Some(entry.clone());
            if entry == pop_up_to {
                break;
            }
        }
        if let Some(last) = last_popped {
            self.state().pop(&last, saved_state);
        }
    }

    /// Pops one entry (Kotlin's older `popBackStack()`): whether it did; by default yes.
    fn pop_back_stack(&self) -> bool {
        true
    }

    /// What it saves of its own (Kotlin's `onSaveState`).
    fn on_save_state(&self) -> Option<SavedState> {
        None
    }

    /// What it saved, given back (Kotlin's `onRestoreState`).
    fn on_restore_state(&self, saved_state: SavedState) {
        let _ = saved_state;
    }
}

/// Whether two navigators are the same object.
pub(crate) fn same_navigator(a: &Rc<dyn Navigator>, b: &Rc<dyn Navigator>) -> bool {
    std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b))
}

// ---- NavigatorState ----

/// What a [`NavigatorState`] of a kind does differently (Kotlin's subclasses' overrides of `NavigatorState`, the
/// controller's `NavControllerNavigatorState`): each default is the state's own (`state.base_…`, Kotlin's `super`).
pub trait NavigatorStateImpl: 'static {
    /// A new entry of `destination` (Kotlin's abstract `createBackStackEntry`).
    fn create_back_stack_entry(
        &self,
        state: &NavigatorState,
        destination: &NavDestination,
        arguments: Option<SavedState>,
    ) -> NavBackStackEntry;

    fn push(&self, state: &NavigatorState, entry: &NavBackStackEntry) {
        state.base_push(entry);
    }
    fn push_with_transition(&self, state: &NavigatorState, entry: &NavBackStackEntry) {
        state.base_push_with_transition(entry);
    }
    fn pop(&self, state: &NavigatorState, pop_up_to: &NavBackStackEntry, save_state: bool) {
        state.base_pop(pop_up_to, save_state);
    }
    fn pop_with_transition(&self, state: &NavigatorState, pop_up_to: &NavBackStackEntry, save_state: bool) {
        state.base_pop_with_transition(pop_up_to, save_state);
    }
    fn on_launch_single_top(&self, state: &NavigatorState, entry: &NavBackStackEntry) {
        state.base_on_launch_single_top(entry);
    }
    fn on_launch_single_top_with_transition(&self, state: &NavigatorState, entry: &NavBackStackEntry) {
        state.base_on_launch_single_top_with_transition(entry);
    }
    fn mark_transition_complete(&self, state: &NavigatorState, entry: &NavBackStackEntry) {
        state.base_mark_transition_complete(entry);
    }
    fn prepare_for_transition(&self, state: &NavigatorState, entry: &NavBackStackEntry) {
        state.base_prepare_for_transition(entry);
    }
}

struct StateData {
    back_stack: MutableStateFlow<Vec<NavBackStackEntry>>,
    transitions_in_progress: MutableStateFlow<Vec<NavBackStackEntry>>,
    is_navigating: Cell<bool>,
    imp: Rc<dyn NavigatorStateImpl>,
}

/// A navigator's part of the back stack (Compose's `NavigatorState`): its entries and those in a transition, as
/// state flows; what the navigator pushes and pops goes through it to the controller. A cheap handle.
#[derive(Clone)]
pub struct NavigatorState(Rc<StateData>);

impl PartialEq for NavigatorState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for NavigatorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NavigatorState({:?})", self.0.back_stack.value())
    }
}

fn plus(set: &[NavBackStackEntry], entry: &NavBackStackEntry) -> Vec<NavBackStackEntry> {
    let mut out = set.to_vec();
    if !out.contains(entry) {
        out.push(entry.clone());
    }
    out
}

impl NavigatorState {
    /// A state whose overrides are `imp`'s.
    pub fn new(imp: impl NavigatorStateImpl) -> NavigatorState {
        NavigatorState::from_rc(Rc::new(imp))
    }

    pub(crate) fn from_rc(imp: Rc<dyn NavigatorStateImpl>) -> NavigatorState {
        NavigatorState(Rc::new(StateData {
            back_stack: MutableStateFlow::new(Vec::new()),
            transitions_in_progress: MutableStateFlow::new(Vec::new()),
            is_navigating: Cell::new(false),
            imp,
        }))
    }

    /// Its overrides.
    pub fn imp(&self) -> Rc<dyn NavigatorStateImpl> {
        self.0.imp.clone()
    }

    /// Its entries (Kotlin's `backStack`).
    pub fn back_stack(&self) -> StateFlow<Vec<NavBackStackEntry>> {
        self.0.back_stack.as_state_flow()
    }

    /// Its entries in a transition (Kotlin's `transitionsInProgress`, a set).
    pub fn transitions_in_progress(&self) -> StateFlow<Vec<NavBackStackEntry>> {
        self.0.transitions_in_progress.as_state_flow()
    }

    /// Whether its controller is navigating (Kotlin's `isNavigating`).
    pub fn is_navigating(&self) -> bool {
        self.0.is_navigating.get()
    }

    pub fn set_is_navigating(&self, value: bool) {
        self.0.is_navigating.set(value);
    }

    /// Pushes `entry` (Kotlin's `push`).
    pub fn push(&self, entry: &NavBackStackEntry) {
        self.0.imp.push(self, entry);
    }

    /// Pushes `entry`, it and the one below in a transition (Kotlin's `pushWithTransition`).
    pub fn push_with_transition(&self, entry: &NavBackStackEntry) {
        self.0.imp.push_with_transition(self, entry);
    }

    /// A new entry of `destination` (Kotlin's `createBackStackEntry`).
    pub fn create_back_stack_entry(
        &self,
        destination: &NavDestination,
        arguments: Option<SavedState>,
    ) -> NavBackStackEntry {
        self.0.imp.create_back_stack_entry(self, destination, arguments)
    }

    /// Pops `pop_up_to` and the entries above it (Kotlin's `pop`).
    pub fn pop(&self, pop_up_to: &NavBackStackEntry, save_state: bool) {
        self.0.imp.pop(self, pop_up_to, save_state);
    }

    /// Pops, it and the one coming back in a transition (Kotlin's `popWithTransition`).
    pub fn pop_with_transition(&self, pop_up_to: &NavBackStackEntry, save_state: bool) {
        self.0.imp.pop_with_transition(self, pop_up_to, save_state);
    }

    /// `entry` in place of the top one of its id (Kotlin's `onLaunchSingleTop`).
    pub fn on_launch_single_top(&self, entry: &NavBackStackEntry) {
        self.0.imp.on_launch_single_top(self, entry);
    }

    /// The same, in a transition (Kotlin's `onLaunchSingleTopWithTransition`).
    pub fn on_launch_single_top_with_transition(&self, entry: &NavBackStackEntry) {
        self.0.imp.on_launch_single_top_with_transition(self, entry);
    }

    /// `entry`'s transition is over (Kotlin's `markTransitionComplete`).
    pub fn mark_transition_complete(&self, entry: &NavBackStackEntry) {
        self.0.imp.mark_transition_complete(self, entry);
    }

    /// `entry` is about to be in a transition (Kotlin's `prepareForTransition`).
    pub fn prepare_for_transition(&self, entry: &NavBackStackEntry) {
        self.0.imp.prepare_for_transition(self, entry);
    }

    // ---- The state's own (Kotlin's `super.…`) ----

    pub fn base_push(&self, entry: &NavBackStackEntry) {
        let mut stack = self.0.back_stack.value();
        stack.push(entry.clone());
        self.0.back_stack.set_value(stack);
    }

    pub fn base_push_with_transition(&self, entry: &NavBackStackEntry) {
        let transitions = self.0.transitions_in_progress.value();
        let stack = self.0.back_stack.value();
        if transitions.contains(entry) && stack.contains(entry) {
            return;
        }
        let mut transitions = transitions;
        if let Some(previous) = stack.last() {
            transitions = plus(&transitions, previous);
        }
        transitions = plus(&transitions, entry);
        self.0.transitions_in_progress.set_value(transitions);
        self.push(entry);
    }

    pub fn base_pop(&self, pop_up_to: &NavBackStackEntry, save_state: bool) {
        let _ = save_state;
        let stack: Vec<NavBackStackEntry> =
            self.0.back_stack.value().into_iter().take_while(|e| e != pop_up_to).collect();
        self.0.back_stack.set_value(stack);
    }

    pub fn base_pop_with_transition(&self, pop_up_to: &NavBackStackEntry, save_state: bool) {
        let transitions = self.0.transitions_in_progress.value();
        let stack = self.0.back_stack.value();
        if transitions.contains(pop_up_to) && !stack.contains(pop_up_to) {
            return;
        }
        let mut transitions = plus(&transitions, pop_up_to);
        let index_of = |e: &NavBackStackEntry| stack.iter().rposition(|x| x == e);
        let pop_index = index_of(pop_up_to);
        let incoming = stack.iter().rev().find(|e| *e != pop_up_to && index_of(e) < pop_index).cloned();
        if let Some(incoming) = incoming {
            transitions = plus(&transitions, &incoming);
        }
        self.0.transitions_in_progress.set_value(transitions);
        self.pop(pop_up_to, save_state);
    }

    pub fn base_on_launch_single_top(&self, entry: &NavBackStackEntry) {
        let mut stack = self.0.back_stack.value();
        if let Some(i) = stack.iter().rposition(|e| e.id() == entry.id()) {
            stack[i] = entry.clone();
        }
        self.0.back_stack.set_value(stack);
    }

    pub fn base_on_launch_single_top_with_transition(&self, entry: &NavBackStackEntry) {
        let stack = self.0.back_stack.value();
        let old =
            stack.iter().rev().find(|e| e.id() == entry.id()).cloned().expect("an entry of the id on the back stack");
        let transitions = plus(&plus(&self.0.transitions_in_progress.value(), &old), entry);
        self.0.transitions_in_progress.set_value(transitions);
        self.on_launch_single_top(entry);
    }

    pub fn base_mark_transition_complete(&self, entry: &NavBackStackEntry) {
        let transitions: Vec<NavBackStackEntry> =
            self.0.transitions_in_progress.value().into_iter().filter(|e| e != entry).collect();
        self.0.transitions_in_progress.set_value(transitions);
    }

    pub fn base_prepare_for_transition(&self, entry: &NavBackStackEntry) {
        let transitions = plus(&self.0.transitions_in_progress.value(), entry);
        self.0.transitions_in_progress.set_value(transitions);
    }
}

// ---- NavigatorProvider ----

pub(crate) struct ProviderEntry {
    name: String,
    navigator: Rc<dyn Navigator>,
    any: Option<Rc<dyn Any>>,
}

/// The navigators of a controller, by name (Compose's `NavigatorProvider`). A cheap handle.
#[derive(Clone, Default)]
pub struct NavigatorProvider(Rc<RefCell<Vec<ProviderEntry>>>);

impl PartialEq for NavigatorProvider {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl NavigatorProvider {
    pub fn new() -> NavigatorProvider {
        NavigatorProvider::default()
    }

    /// Its navigators by name, in the order added (Kotlin's `navigators`).
    pub fn navigators(&self) -> Vec<(String, Rc<dyn Navigator>)> {
        self.0.borrow().iter().map(|e| (e.name.clone(), e.navigator.clone())).collect()
    }

    /// The navigator named `name` (Kotlin's `getNavigator(name)`).
    ///
    /// # Panics
    /// An empty name, or none of it.
    pub fn get_navigator(&self, name: &str) -> Rc<dyn Navigator> {
        assert!(!name.is_empty(), "navigator name cannot be an empty string");
        self.0.borrow().iter().find(|e| e.name == name).map(|e| e.navigator.clone()).unwrap_or_else(|| {
            panic!("Could not find Navigator with name \"{name}\". You must call NavController.addNavigator() for each navigation type.")
        })
    }

    /// Its navigator of type `N` (Kotlin's `provider[N::class]`).
    ///
    /// # Panics
    /// If it has none.
    pub fn get_navigator_of<N: Navigator>(&self) -> Rc<N> {
        self.0
            .borrow()
            .iter()
            .find_map(|e| e.any.clone().and_then(|a| a.downcast::<N>().ok()))
            .unwrap_or_else(|| panic!("Could not find Navigator {}", std::any::type_name::<N>()))
    }

    /// Adds `navigator` under its name (Kotlin's `addNavigator(navigator)`): the one it replaces, if any.
    pub fn add_navigator<N: Navigator>(&self, navigator: Rc<N>) -> Option<Rc<dyn Navigator>> {
        let name = navigator.name();
        let any: Rc<dyn Any> = navigator.clone();
        self.add(name, navigator, Some(any))
    }

    /// Adds `navigator` under `name` (Kotlin's `addNavigator(name, navigator)`).
    ///
    /// # Panics
    /// An empty name; a navigator attached to another controller, or replacing an attached one.
    pub fn add_navigator_named(&self, name: &str, navigator: Rc<dyn Navigator>) -> Option<Rc<dyn Navigator>> {
        self.add(name.to_string(), navigator, None)
    }

    fn add(&self, name: String, navigator: Rc<dyn Navigator>, any: Option<Rc<dyn Any>>) -> Option<Rc<dyn Navigator>> {
        assert!(!name.is_empty(), "navigator name cannot be an empty string");
        let previous = self.0.borrow().iter().find(|e| e.name == name).map(|e| e.navigator.clone());
        if let Some(p) = &previous {
            if same_navigator(p, &navigator) {
                return Some(navigator);
            }
            assert!(!p.is_attached(), "Navigator {name} is replacing an already attached navigator");
        }
        assert!(!navigator.is_attached(), "Navigator {name} is already attached to another NavController");
        let mut entries = self.0.borrow_mut();
        match entries.iter_mut().find(|e| e.name == name) {
            Some(e) => {
                e.navigator = navigator;
                e.any = any;
            }
            None => entries.push(ProviderEntry { name, navigator, any }),
        }
        previous
    }

    pub(crate) fn downgrade(&self) -> Weak<RefCell<Vec<ProviderEntry>>> {
        Rc::downgrade(&self.0)
    }
}

// ---- NavGraphNavigator, NoOpNavigator ----

/// The navigator of graphs (Compose's `NavGraphNavigator`, named `"navigation"`): navigating to a graph navigates
/// to its start destination (with the start route's arguments), the graph's entry below it.
pub struct NavGraphNavigator {
    base: NavigatorBase,
    provider: Weak<RefCell<Vec<ProviderEntry>>>,
}

impl NavGraphNavigator {
    /// Its name (Kotlin's `NavGraphNavigator.NAME`).
    pub const NAME: &'static str = "navigation";

    /// The graph navigator of `navigator_provider`'s navigators.
    pub fn new(navigator_provider: &NavigatorProvider) -> NavGraphNavigator {
        NavGraphNavigator { base: NavigatorBase::new(), provider: navigator_provider.downgrade() }
    }

    fn provider(&self) -> NavigatorProvider {
        NavigatorProvider(self.provider.upgrade().expect("its navigator provider"))
    }

    /// Its entries (Kotlin's `backStack`).
    pub fn back_stack(&self) -> StateFlow<Vec<NavBackStackEntry>> {
        self.state().back_stack()
    }

    fn navigate_entry(
        &self,
        entry: &NavBackStackEntry,
        nav_options: Option<&NavOptions>,
        navigator_extras: Option<&dyn NavigatorExtras>,
    ) {
        let destination = entry.destination().as_graph().expect("a graph's entry");
        let mut args = entry.arguments();
        let start_id = destination.start_destination_id();
        let start_route = destination.start_destination_route();
        assert!(
            start_id != 0 || start_route.is_some(),
            "no start destination defined via app:startDestination for {}",
            destination.display_name()
        );
        let start = match &start_route {
            Some(r) => destination.find_node_searching(r, false),
            None => destination.node(start_id),
        };
        let start = start.unwrap_or_else(|| {
            panic!(
                "navigation destination {} is not a direct child of this NavGraph",
                destination.start_dest_display_name()
            )
        });
        if let Some(start_route) = &start_route {
            if Some(start_route) != start.route().as_ref() {
                let matching = start.match_route(start_route).and_then(|m| m.matching_args());
                if let Some(matching) = matching.filter(|m| !m.read(|r| r.is_empty())) {
                    let merged = crate::saved_state::saved_state(|w| {
                        w.put_all(&matching);
                        if let Some(a) = &args {
                            w.put_all(a);
                        }
                    });
                    args = Some(merged);
                }
            }
            let arguments = start.arguments();
            if !arguments.is_empty() {
                let missing = crate::argument::missing_required_arguments(&arguments, |key| {
                    args.as_ref().is_none_or(|a| !contains(a, key))
                });
                assert!(
                    missing.is_empty(),
                    "Cannot navigate to startDestination {start:?}. Missing required arguments [{missing:?}]"
                );
            }
        }
        let navigator = self.provider().get_navigator(&start.navigator_name());
        let start_entry = self.state().create_back_stack_entry(&start, start.add_in_default_args(args.as_ref()));
        navigator.navigate(vec![start_entry], nav_options, navigator_extras);
    }
}

impl Navigator for NavGraphNavigator {
    fn name(&self) -> String {
        NavGraphNavigator::NAME.into()
    }

    fn base(&self) -> &NavigatorBase {
        &self.base
    }

    fn create_destination(&self) -> NavDestination {
        NavGraph::new(NavGraphNavigator::NAME).as_destination()
    }

    fn navigate(
        &self,
        entries: Vec<NavBackStackEntry>,
        nav_options: Option<&NavOptions>,
        navigator_extras: Option<&dyn NavigatorExtras>,
    ) {
        for entry in &entries {
            self.navigate_entry(entry, nav_options, navigator_extras);
        }
    }
}

/// A navigator that shows nothing (Compose's `NoOpNavigator`, named `"NoOp"`).
#[derive(Default)]
pub struct NoOpNavigator {
    base: NavigatorBase,
}

impl NoOpNavigator {
    pub fn new() -> NoOpNavigator {
        NoOpNavigator::default()
    }
}

impl Navigator for NoOpNavigator {
    fn name(&self) -> String {
        "NoOp".into()
    }
    fn base(&self) -> &NavigatorBase {
        &self.base
    }
    fn create_destination(&self) -> NavDestination {
        NavDestination::new("NoOp")
    }
}

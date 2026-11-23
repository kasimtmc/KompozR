//! The navigators of composables and nested graphs (navigation-compose's `ComposeNavigator` with its `Destination`,
//! `ComposeNavGraphNavigator` with its `ComposeNavGraph`): a composable destination holds its content and the
//! transitions it enters and leaves with; a graph of the NavHost holds the transitions of the destinations in it.

use crate::destination::{NavDestination, NavDestinationKind, NavGraph};
use crate::entry::NavBackStackEntry;
use crate::navigator::{NavGraphNavigator, Navigator, NavigatorBase, NavigatorExtras, NavigatorProvider};
use crate::options::NavOptions;
use kompoz_animation::{
    AnimatedContentScope, AnimatedContentTransitionScope, EnterTransition, ExitTransition, SizeTransform,
};
use kompoz_coroutines::StateFlow;
use kompoz_runtime::{MutableState, mutable_state_of};
use std::cell::RefCell;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

/// The scope a NavHost's transitions are picked in: its change from one entry to another.
pub type NavTransitionScope = AnimatedContentTransitionScope<NavBackStackEntry>;

/// A destination's enter transition (Kotlin's `AnimatedContentTransitionScope<NavBackStackEntry>.() ->
/// EnterTransition?`): none (null) leaves it to the graph above it, then to the NavHost.
pub type DestinationEnterTransition =
    Rc<dyn Fn(&AnimatedContentTransitionScope<NavBackStackEntry>) -> Option<EnterTransition>>;
/// A destination's exit transition (Kotlin's `AnimatedContentTransitionScope<NavBackStackEntry>.() ->
/// ExitTransition?`).
pub type DestinationExitTransition =
    Rc<dyn Fn(&AnimatedContentTransitionScope<NavBackStackEntry>) -> Option<ExitTransition>>;
/// A destination's enter transition during a predictive back gesture, of the gesture's swipe edge (Kotlin's
/// `AnimatedContentTransitionScope<NavBackStackEntry>.(Int) -> EnterTransition?`).
pub type DestinationPredictiveEnterTransition =
    Rc<dyn Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> Option<EnterTransition>>;
/// A destination's exit transition during a predictive back gesture (Kotlin's
/// `AnimatedContentTransitionScope<NavBackStackEntry>.(Int) -> ExitTransition?`).
pub type DestinationPredictiveExitTransition =
    Rc<dyn Fn(&AnimatedContentTransitionScope<NavBackStackEntry>, i32) -> Option<ExitTransition>>;
/// A destination's size transform (Kotlin's `AnimatedContentTransitionScope<NavBackStackEntry>.() ->
/// SizeTransform?`).
pub type DestinationSizeTransform =
    Rc<dyn Fn(&AnimatedContentTransitionScope<NavBackStackEntry>) -> Option<Rc<dyn SizeTransform>>>;

/// The content of a composable destination (Kotlin's `@Composable AnimatedContentScope.(NavBackStackEntry) ->
/// Unit`).
pub type DestinationContent = Rc<dyn Fn(&AnimatedContentScope, NavBackStackEntry)>;

/// The transitions a destination or a graph of a NavHost sets (Kotlin's `internal var enterTransition` … of
/// `ComposeNavigator.Destination` and `ComposeNavGraph`).
#[derive(Default)]
pub(crate) struct NavTransitions {
    pub(crate) enter_transition: RefCell<Option<DestinationEnterTransition>>,
    pub(crate) exit_transition: RefCell<Option<DestinationExitTransition>>,
    pub(crate) pop_enter_transition: RefCell<Option<DestinationEnterTransition>>,
    pub(crate) pop_exit_transition: RefCell<Option<DestinationExitTransition>>,
    pub(crate) predictive_pop_enter_transition: RefCell<Option<DestinationPredictiveEnterTransition>>,
    pub(crate) predictive_pop_exit_transition: RefCell<Option<DestinationPredictiveExitTransition>>,
    pub(crate) size_transform: RefCell<Option<DestinationSizeTransform>>,
}

impl NavTransitions {
    /// The transitions of `destination`, if it is a composable destination or a NavHost's graph (Kotlin's `when
    /// (this) { is ComposeNavigator.Destination -> …; is ComposeNavGraph -> …; else -> null }`).
    pub(crate) fn of(destination: &NavDestination) -> Option<Rc<NavTransitions>> {
        if let Some(d) = destination.extra::<ComposeDestinationData>() {
            return Some(d.transitions.clone());
        }
        destination.extra::<ComposeNavGraphData>().map(|g| g.transitions.clone())
    }
}

// ---- ComposeNavigator ----

/// The navigator of composable destinations (Compose's `ComposeNavigator`, named `"composable"`): a NavHost shows
/// the top of its back stack, entries coming and going through transitions.
pub struct ComposeNavigator {
    base: NavigatorBase,
    is_pop: MutableState<bool>,
}

impl Default for ComposeNavigator {
    fn default() -> Self {
        ComposeNavigator::new()
    }
}

impl ComposeNavigator {
    /// Its name (Kotlin's `ComposeNavigator.NAME`).
    pub const NAME: &'static str = "composable";

    /// Kotlin's `ComposeNavigator()`.
    pub fn new() -> ComposeNavigator {
        ComposeNavigator { base: NavigatorBase::new(), is_pop: mutable_state_of(false) }
    }

    /// Its entries in a transition (Kotlin's internal `transitionsInProgress`).
    pub fn transitions_in_progress(&self) -> StateFlow<Vec<NavBackStackEntry>> {
        self.state().transitions_in_progress()
    }

    /// Its back stack (Kotlin's `backStack`).
    pub fn back_stack(&self) -> StateFlow<Vec<NavBackStackEntry>> {
        self.state().back_stack()
    }

    /// Whether the last change of its back stack was a pop (Kotlin's internal `isPop`, a state).
    pub(crate) fn is_pop(&self) -> bool {
        self.is_pop.get()
    }

    /// Puts `entry` in a transition (a predictive back gesture's) before it happens (Kotlin's
    /// `prepareForTransition`).
    pub fn prepare_for_transition(&self, entry: &NavBackStackEntry) {
        self.state().prepare_for_transition(entry);
    }

    /// `entry`'s transition is done (Kotlin's `onTransitionComplete`).
    pub fn on_transition_complete(&self, entry: &NavBackStackEntry) {
        self.state().mark_transition_complete(entry);
    }
}

impl Navigator for ComposeNavigator {
    fn name(&self) -> String {
        ComposeNavigator::NAME.into()
    }

    fn base(&self) -> &NavigatorBase {
        &self.base
    }

    fn create_destination(&self) -> NavDestination {
        ComposeNavigatorDestination::new(self, |_: &AnimatedContentScope, _: NavBackStackEntry| {}).into()
    }

    fn navigate(
        &self,
        entries: Vec<NavBackStackEntry>,
        _nav_options: Option<&NavOptions>,
        _navigator_extras: Option<&dyn NavigatorExtras>,
    ) {
        for entry in &entries {
            self.state().push_with_transition(entry);
        }
        self.is_pop.set(false);
    }

    fn pop_back_stack_to(&self, pop_up_to: &NavBackStackEntry, saved_state: bool) {
        self.state().pop_with_transition(pop_up_to, saved_state);
        self.is_pop.set(true);
    }
}

/// What a composable destination keeps (Kotlin's `ComposeNavigator.Destination`'s fields).
pub(crate) struct ComposeDestinationData {
    pub(crate) content: DestinationContent,
    pub(crate) transitions: Rc<NavTransitions>,
}

/// A composable destination (Compose's `ComposeNavigator.Destination`): its content, given its back stack entry in
/// the NavHost's `AnimatedContentScope`, and its transitions. A [`NavDestination`] it derefs to.
///
/// Kotlin: `ComposeNavigator.Destination`
#[derive(Clone)]
pub struct ComposeNavigatorDestination(NavDestination);

impl ComposeNavigatorDestination {
    /// A destination of `navigator` showing `content` (Kotlin's `Destination(navigator, content)`).
    ///
    /// Kotlin: `ComposeNavigator.Destination(navigator: ComposeNavigator, content: @Composable AnimatedContentScope.(NavBackStackEntry) -> Unit)`
    pub fn new(
        navigator: &ComposeNavigator,
        content: impl Fn(&AnimatedContentScope, NavBackStackEntry) + 'static,
    ) -> ComposeNavigatorDestination {
        let data =
            ComposeDestinationData { content: Rc::new(content), transitions: Rc::new(NavTransitions::default()) };
        ComposeNavigatorDestination(NavDestination::of_kind(
            navigator.name(),
            NavDestinationKind { class_name: "Destination", extra: Some(Rc::new(data)), ..Default::default() },
        ))
    }

    /// `destination` as a composable destination, if it is one (Kotlin's `destination as? ComposeNavigator.Destination`).
    pub fn from_destination(destination: &NavDestination) -> Option<ComposeNavigatorDestination> {
        destination.extra::<ComposeDestinationData>().map(|_| ComposeNavigatorDestination(destination.clone()))
    }

    /// It as a destination.
    pub fn as_destination(&self) -> NavDestination {
        self.0.clone()
    }

    fn data(&self) -> Rc<ComposeDestinationData> {
        self.0.extra::<ComposeDestinationData>().expect("a composable destination")
    }

    /// Its content (Kotlin's internal `content`).
    pub(crate) fn content(&self) -> DestinationContent {
        self.data().content.clone()
    }

    /// Its transitions.
    pub(crate) fn transitions(&self) -> Rc<NavTransitions> {
        self.data().transitions.clone()
    }
}

impl Deref for ComposeNavigatorDestination {
    type Target = NavDestination;
    fn deref(&self) -> &NavDestination {
        &self.0
    }
}

impl From<ComposeNavigatorDestination> for NavDestination {
    fn from(d: ComposeNavigatorDestination) -> NavDestination {
        d.0
    }
}

impl fmt::Debug for ComposeNavigatorDestination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

// ---- ComposeNavGraphNavigator ----

/// What a NavHost's graph keeps (Kotlin's `ComposeNavGraph`'s fields).
pub(crate) struct ComposeNavGraphData {
    pub(crate) transitions: Rc<NavTransitions>,
}

/// The navigator of a NavHost's graphs (Compose's internal `ComposeNavGraphNavigator`, named `"navigation"`): a
/// [`NavGraphNavigator`] whose graphs (Kotlin's `ComposeNavGraph`) hold the transitions of the destinations in them.
pub(crate) struct ComposeNavGraphNavigator {
    graph_navigator: NavGraphNavigator,
}

impl ComposeNavGraphNavigator {
    pub(crate) fn new(navigator_provider: &NavigatorProvider) -> ComposeNavGraphNavigator {
        ComposeNavGraphNavigator { graph_navigator: NavGraphNavigator::new(navigator_provider) }
    }
}

impl Navigator for ComposeNavGraphNavigator {
    fn name(&self) -> String {
        NavGraphNavigator::NAME.into()
    }

    fn base(&self) -> &NavigatorBase {
        self.graph_navigator.base()
    }

    fn create_destination(&self) -> NavDestination {
        let data = ComposeNavGraphData { transitions: Rc::new(NavTransitions::default()) };
        NavGraph::of_kind(
            NavGraphNavigator::NAME,
            NavDestinationKind { class_name: "ComposeNavGraph", extra: Some(Rc::new(data)), ..Default::default() },
        )
        .as_destination()
    }

    fn navigate(
        &self,
        entries: Vec<NavBackStackEntry>,
        nav_options: Option<&NavOptions>,
        navigator_extras: Option<&dyn NavigatorExtras>,
    ) {
        self.graph_navigator.navigate(entries, nav_options, navigator_extras);
    }
}

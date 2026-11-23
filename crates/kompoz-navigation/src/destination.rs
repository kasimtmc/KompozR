//! Destinations and graphs (Compose's `NavDestination`, `NavGraph`, `FloatingWindow`, `SupportingPane`): a
//! destination is a node of the navigation graph — a route, arguments, deep links — of a navigator; a graph is a
//! destination holding others, one of them its start.

use crate::argument::{NavArgument, NavArguments, missing_required_arguments};
use crate::deep_link::{NavDeepLink, NavDeepLinkBuilder, NavDeepLinkRequest};
use crate::java::string_hash;
use crate::saved_state::{SavedState, contains, put_all, size};
use crate::serialization::{AnyRoute, Route, RouteClass, generate_route_with_args, hash_code_of};
use crate::uri::NavUri;
use std::any::Any;
use std::cell::{Cell, OnceCell, RefCell};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Deref;
use std::rc::{Rc, Weak};

/// What a destination is beyond its navigator's name (Kotlin's subclass of `NavDestination` and the interfaces it
/// implements): a navigator's own destination class (`ComposeNavigator.Destination` with its content) gives it.
#[derive(Clone, Default)]
pub struct NavDestinationKind {
    /// Kotlin's simple class name (what it prints as; `NavDestination` by default).
    pub class_name: &'static str,
    /// Kotlin's `FloatingWindow`: shown over the destination below it (a dialog), which stays started.
    pub floating_window: bool,
    /// Kotlin's `SupportingPane`: shown beside the destination below it, which stays resumed.
    pub supporting_pane: bool,
    /// What the navigator keeps with it (a composable destination's content); [`NavDestination::extra`] gives it.
    pub extra: Option<Rc<dyn Any>>,
}

pub(crate) struct GraphData {
    /// Its nodes by id (Kotlin's `SparseArrayCompat`, iterated in the ids' order).
    nodes: RefCell<BTreeMap<i32, NavDestination>>,
    start_dest_id: Cell<i32>,
    start_dest_id_name: RefCell<Option<String>>,
    start_destination_route: RefCell<Option<String>>,
}

pub(crate) struct DestinationData {
    navigator_name: String,
    kind: NavDestinationKind,
    parent: RefCell<Weak<DestinationData>>,
    label: RefCell<Option<String>>,
    id: Cell<i32>,
    id_name: RefCell<Option<String>>,
    route: RefCell<Option<String>>,
    route_deep_link: RefCell<Option<OnceCell<NavDeepLink>>>,
    route_pattern: RefCell<Option<String>>,
    arguments: RefCell<NavArguments>,
    deep_links: RefCell<Vec<NavDeepLink>>,
    graph: Option<GraphData>,
}

/// A destination (Compose's `NavDestination`): see the module. A cheap handle: clones are the same destination.
/// Equal destinations have the same id, route, deep links and arguments (a graph's, the same nodes and start too:
/// Kotlin's `equals`); [`same`](Self::same) tells the same one apart.
#[derive(Clone)]
pub struct NavDestination(pub(crate) Rc<DestinationData>);

/// `route` as the URI its deep link matches (Kotlin's `createRoute`: `android-app://androidx.navigation/route`).
pub fn create_route(route: Option<&str>) -> String {
    match route {
        Some(r) => format!("android-app://androidx.navigation/{r}"),
        None => String::new(),
    }
}

/// A match of a route or deep link to a destination (Kotlin's `NavDestination.DeepLinkMatch`), better ones greater:
/// an exact one, then more path segments matched, then with arguments, more arguments, the action matched, the MIME
/// type better matched.
#[derive(Clone)]
pub struct DeepLinkMatch {
    destination: NavDestination,
    matching_args: Option<SavedState>,
    is_exact_deep_link: bool,
    matching_path_segments: usize,
    has_matching_action: bool,
    mime_type_match_level: i32,
}

impl DeepLinkMatch {
    /// The destination matched.
    pub fn destination(&self) -> NavDestination {
        self.destination.clone()
    }

    /// The arguments the route or link gave.
    pub fn matching_args(&self) -> Option<SavedState> {
        self.matching_args.clone()
    }

    /// Whether `arguments` has each argument the match gave, with an equal value (Kotlin's `hasMatchingArgs`).
    pub fn has_matching_args(&self, arguments: Option<&SavedState>) -> bool {
        let (Some(arguments), Some(matching)) = (arguments, &self.matching_args) else { return false };
        let destination_args = self.destination.arguments();
        for key in crate::saved_state::keys(matching) {
            if !contains(arguments, &key) {
                return false;
            }
            if let Some(argument) = destination_args.get(&key)
                && !argument.r#type().erased().values_equal_in(matching, arguments, &key)
            {
                return false;
            }
        }
        true
    }

    fn compare(&self, other: &DeepLinkMatch) -> Ordering {
        if self.is_exact_deep_link != other.is_exact_deep_link {
            return if self.is_exact_deep_link { Ordering::Greater } else { Ordering::Less };
        }
        match self.matching_path_segments.cmp(&other.matching_path_segments) {
            Ordering::Equal => {}
            o => return o,
        }
        match (&self.matching_args, &other.matching_args) {
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (Some(a), Some(b)) => match size(a).cmp(&size(b)) {
                Ordering::Equal => {}
                o => return o,
            },
            (None, None) => {}
        }
        if self.has_matching_action != other.has_matching_action {
            return if self.has_matching_action { Ordering::Greater } else { Ordering::Less };
        }
        self.mime_type_match_level.cmp(&other.mime_type_match_level)
    }
}

impl PartialEq for DeepLinkMatch {
    fn eq(&self, other: &Self) -> bool {
        self.compare(other) == Ordering::Equal
    }
}

impl PartialOrd for DeepLinkMatch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.compare(other))
    }
}

/// The greatest of `matches` (Kotlin's `maxOrNull`: the first of equal ones).
pub(crate) fn best_match(matches: impl IntoIterator<Item = DeepLinkMatch>) -> Option<DeepLinkMatch> {
    let mut best: Option<DeepLinkMatch> = None;
    for m in matches {
        if best.as_ref().is_none_or(|b| m.compare(b) == Ordering::Greater) {
            best = Some(m);
        }
    }
    best
}

impl NavDestination {
    fn make(navigator_name: String, kind: NavDestinationKind, graph: Option<GraphData>) -> NavDestination {
        NavDestination(Rc::new(DestinationData {
            navigator_name,
            kind,
            parent: RefCell::new(Weak::new()),
            label: RefCell::new(None),
            id: Cell::new(0),
            id_name: RefCell::new(None),
            route: RefCell::new(None),
            route_deep_link: RefCell::new(None),
            route_pattern: RefCell::new(None),
            arguments: RefCell::new(NavArguments::new()),
            deep_links: RefCell::new(Vec::new()),
            graph,
        }))
    }

    /// A destination of the navigator named `navigator_name` (Kotlin's `NavDestination(navigatorName)`).
    pub fn new(navigator_name: impl Into<String>) -> NavDestination {
        NavDestination::make(navigator_name.into(), NavDestinationKind::default(), None)
    }

    /// A destination of `navigator_name` of the kind a navigator's destinations are (a subclass of Kotlin's).
    pub fn of_kind(navigator_name: impl Into<String>, kind: NavDestinationKind) -> NavDestination {
        NavDestination::make(navigator_name.into(), kind, None)
    }

    /// Whether `other` is this very destination (Kotlin's `===`).
    pub fn same(&self, other: &NavDestination) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Its navigator's name (Kotlin's `navigatorName`).
    pub fn navigator_name(&self) -> String {
        self.0.navigator_name.clone()
    }

    /// What its navigator keeps with it, if it is a `T`.
    pub fn extra<T: 'static>(&self) -> Option<Rc<T>> {
        self.0.kind.extra.clone()?.downcast::<T>().ok()
    }

    /// Whether it is a `FloatingWindow` (a dialog's).
    pub fn is_floating_window(&self) -> bool {
        self.0.kind.floating_window
    }

    /// Whether it is a `SupportingPane`.
    pub fn is_supporting_pane(&self) -> bool {
        self.0.kind.supporting_pane
    }

    /// It as a graph, if it is one (Kotlin's `as? NavGraph`).
    pub fn as_graph(&self) -> Option<NavGraph> {
        self.0.graph.as_ref().map(|_| NavGraph(self.clone()))
    }

    /// Whether it is a graph (Kotlin's `is NavGraph`).
    pub fn is_graph(&self) -> bool {
        self.0.graph.is_some()
    }

    /// The graph it is in (Kotlin's `parent`).
    pub fn parent(&self) -> Option<NavGraph> {
        self.0.parent.borrow().upgrade().map(|d| NavGraph(NavDestination(d)))
    }

    pub(crate) fn set_parent(&self, parent: Option<&NavGraph>) {
        *self.0.parent.borrow_mut() = parent.map_or_else(Weak::new, |p| Rc::downgrade(&p.0.0));
    }

    /// Its label (Kotlin's `label`).
    pub fn label(&self) -> Option<String> {
        self.0.label.borrow().clone()
    }

    pub fn set_label(&self, label: Option<String>) {
        *self.0.label.borrow_mut() = label;
    }

    /// Its arguments (Kotlin's `arguments`).
    pub fn arguments(&self) -> NavArguments {
        self.0.arguments.borrow().clone()
    }

    /// Its id (Kotlin's `id`): its route's hash code (its typed route's), or one given.
    pub fn id(&self) -> i32 {
        self.0.id.get()
    }

    /// Sets its id (Kotlin's `id = …`).
    pub fn set_id(&self, id: i32) {
        self.0.id.set(id);
        *self.0.id_name.borrow_mut() = None;
    }

    /// Its route (Kotlin's `route`): a pattern (`detail/{id}?tab={tab}`).
    pub fn route(&self) -> Option<String> {
        self.0.route.borrow().clone()
    }

    /// Sets its route (Kotlin's `route = …`): its id becomes the route's hash code.
    ///
    /// # Panics
    /// A blank route, or one without the arguments it requires (Kotlin's IllegalArgumentException).
    pub fn set_route(&self, route: Option<String>) {
        match &route {
            None => {
                self.set_id(0);
                *self.0.route_deep_link.borrow_mut() = None;
            }
            Some(r) => {
                assert!(!r.trim().is_empty(), "Cannot have an empty route");
                let temp_route = create_route(Some(r));
                let temp_deep_link = NavDeepLinkBuilder::from_uri_pattern(temp_route.clone()).build();
                let names = temp_deep_link.argument_names();
                let missing = missing_required_arguments(&self.0.arguments.borrow(), |k| !names.iter().any(|n| n == k));
                assert!(
                    missing.is_empty(),
                    "Cannot set route \"{r}\" for destination {self:?}. Following required arguments are missing: \
                     {missing:?}"
                );
                *self.0.route_pattern.borrow_mut() = Some(temp_route.clone());
                *self.0.route_deep_link.borrow_mut() = Some(OnceCell::new());
                self.set_id(string_hash(&temp_route));
            }
        }
        *self.0.route.borrow_mut() = route;
    }

    fn route_deep_link(&self) -> Option<NavDeepLink> {
        let cell = self.0.route_deep_link.borrow();
        let cell = cell.as_ref()?;
        Some(
            cell.get_or_init(|| {
                NavDeepLinkBuilder::from_uri_pattern(self.0.route_pattern.borrow().clone().expect("a route")).build()
            })
            .clone(),
        )
    }

    /// The name it is shown by (Kotlin's `displayName`): its id's.
    pub fn display_name(&self) -> String {
        let name = self.0.id_name.borrow().clone().unwrap_or_else(|| self.id().to_string());
        match &self.0.graph {
            Some(_) if self.id() == 0 => "the root navigation".into(),
            _ => name,
        }
    }

    /// Its deep links.
    pub fn deep_links(&self) -> Vec<NavDeepLink> {
        self.0.deep_links.borrow().clone()
    }

    /// Whether it (a graph's nodes too) answers `deep_link` (Kotlin's `hasDeepLink(Uri)`).
    pub fn has_deep_link(&self, deep_link: &NavUri) -> bool {
        self.has_deep_link_request(&NavDeepLinkRequest::new(Some(deep_link.clone()), None, None))
    }

    /// Whether it answers `request` (Kotlin's `hasDeepLink(NavDeepLinkRequest)`).
    pub fn has_deep_link_request(&self, request: &NavDeepLinkRequest) -> bool {
        self.match_deep_link(request).is_some()
    }

    /// Adds a deep link of `uri_pattern` (Kotlin's `addDeepLink(uriPattern)`).
    pub fn add_deep_link(&self, uri_pattern: impl Into<String>) {
        self.add_nav_deep_link(NavDeepLinkBuilder::from_uri_pattern(uri_pattern).build());
    }

    /// Adds `deep_link` (Kotlin's `addDeepLink(navDeepLink)`).
    ///
    /// # Panics
    /// A link without the arguments it requires.
    pub fn add_nav_deep_link(&self, deep_link: NavDeepLink) {
        let names = deep_link.argument_names();
        let missing = missing_required_arguments(&self.0.arguments.borrow(), |k| !names.iter().any(|n| n == k));
        assert!(
            missing.is_empty(),
            "Deep link {:?} can't be used to open destination {self:?}.\nFollowing required arguments are missing: \
             {missing:?}",
            deep_link.uri_pattern()
        );
        self.0.deep_links.borrow_mut().push(deep_link);
    }

    /// The match of `route` (a pattern or a path: `detail/42`) to it, with the arguments it gives (Kotlin's
    /// `matchRoute`).
    pub fn match_route(&self, route: &str) -> Option<DeepLinkMatch> {
        let deep_link = self.route_deep_link()?;
        let uri = NavUri::new(create_route(Some(route)));
        let args = self.0.arguments.borrow().clone();
        let matching = deep_link.get_matching_arguments(&uri, &args)?;
        let segments = deep_link.calculate_matching_path_segments(Some(&uri));
        Some(DeepLinkMatch {
            destination: self.clone(),
            matching_args: Some(matching),
            is_exact_deep_link: deep_link.is_exact_deep_link(),
            matching_path_segments: segments,
            has_matching_action: false,
            mime_type_match_level: -1,
        })
    }

    /// The best match of `request` to its own deep links.
    fn match_own_deep_link(&self, request: &NavDeepLinkRequest) -> Option<DeepLinkMatch> {
        let deep_links = self.deep_links();
        if deep_links.is_empty() {
            return None;
        }
        let arguments = self.arguments();
        let uri = request.uri();
        let mut best: Option<DeepLinkMatch> = None;
        for deep_link in deep_links {
            if !deep_link.matches(request) {
                continue;
            }
            let matching_args = uri.as_ref().and_then(|u| deep_link.get_matching_arguments(u, &arguments));
            let segments = deep_link.calculate_matching_path_segments(uri.as_ref());
            let matching_action = request.action().is_some() && request.action() == deep_link.action();
            let mime_level = request.mime_type().map_or(-1, |m| deep_link.get_mime_type_match_rating(&m));
            let has_required = || {
                let args = deep_link.get_matching_path_and_query_args(uri.as_ref(), &arguments);
                missing_required_arguments(&arguments, |k| !contains(&args, k)).is_empty()
            };
            if matching_args.is_some() || ((matching_action || mime_level > -1) && has_required()) {
                let m = DeepLinkMatch {
                    destination: self.clone(),
                    matching_args,
                    is_exact_deep_link: deep_link.is_exact_deep_link(),
                    matching_path_segments: segments,
                    has_matching_action: matching_action,
                    mime_type_match_level: mime_level,
                };
                if best.as_ref().is_none_or(|b| m.compare(b) == Ordering::Greater) {
                    best = Some(m);
                }
            }
        }
        best
    }

    /// The best match of `request` (Kotlin's `matchDeepLink`): to its deep links, a graph's to its nodes' too.
    pub fn match_deep_link(&self, request: &NavDeepLinkRequest) -> Option<DeepLinkMatch> {
        match self.as_graph() {
            Some(graph) => graph.match_deep_link_comprehensive_from(
                self.match_own_deep_link(request),
                request,
                true,
                false,
                &graph.0,
            ),
            None => self.match_own_deep_link(request),
        }
    }

    /// Whether `route` (a pattern or a path) with `arguments` is it (Kotlin's `hasRoute(route, arguments)`).
    pub fn has_route_with(&self, route: &str, arguments: Option<&SavedState>) -> bool {
        if self.route().as_deref() == Some(route) {
            return true;
        }
        match self.match_route(route) {
            Some(m) if m.destination.same(self) => m.has_matching_args(arguments),
            _ => false,
        }
    }

    /// Whether it is `T`'s destination (Compose's `NavDestination.hasRoute<T>()`).
    pub fn has_route<T: Route>(&self) -> bool {
        self.has_route_class(RouteClass::of::<T>())
    }

    /// Whether it is `route`'s destination (Kotlin's `hasRoute(KClass)`).
    pub fn has_route_class(&self, route: RouteClass) -> bool {
        route.hash_code() == self.id()
    }

    /// Adds (or replaces) the argument `name`.
    pub fn add_argument(&self, name: impl Into<String>, argument: NavArgument) {
        self.0.arguments.borrow_mut().insert(name, argument);
    }

    /// Removes the argument `name`.
    pub fn remove_argument(&self, name: &str) {
        self.0.arguments.borrow_mut().remove(name);
    }

    /// `args` with its arguments' default values where `args` has none (Kotlin's `addInDefaultArgs`).
    ///
    /// # Panics
    /// A value of an argument not of its type.
    pub fn add_in_default_args(&self, args: Option<&SavedState>) -> Option<SavedState> {
        let arguments = self.arguments();
        if args.is_none() && arguments.is_empty() {
            return None;
        }
        let defaults = SavedState::new();
        for (key, value) in arguments.iter() {
            value.put_default_value(key, &defaults);
        }
        if let Some(args) = args {
            put_all(&defaults, args);
            for (key, value) in arguments.iter() {
                if !value.is_default_value_unknown() {
                    assert!(
                        value.verify(key, &defaults),
                        "Wrong argument type for '{key}' in argument savedState. {} expected.",
                        value.r#type().name()
                    );
                }
            }
        }
        Some(defaults)
    }

    /// It and the graphs it is in, innermost first (Compose's `NavDestination.hierarchy`).
    pub fn hierarchy(&self) -> Vec<NavDestination> {
        let mut out = vec![self.clone()];
        while let Some(p) = out.last().and_then(NavDestination::parent) {
            out.push(p.0.clone());
        }
        out
    }

    /// The ids of the destinations a deep link to it goes through, outermost first: the graphs, without those it
    /// (or a graph on the way) is the start destination of, then it (Kotlin's `buildDeepLinkIds`).
    pub fn build_deep_link_ids(&self, previous_destination: Option<&NavDestination>) -> Vec<i32> {
        let mut hierarchy: Vec<NavDestination> = Vec::new();
        let mut current = Some(self.clone());
        while let Some(c) = current {
            let parent = c.parent();
            if let Some(prev_parent) = previous_destination.and_then(NavDestination::parent)
                && prev_parent.find_node_id(c.id()).is_some_and(|n| n.same(&c))
            {
                hierarchy.insert(0, c);
                break;
            }
            if parent.as_ref().is_none_or(|p| p.start_destination_id() != c.id()) {
                hierarchy.insert(0, c.clone());
            }
            if parent.as_ref().map(|p| &p.0) == previous_destination {
                break;
            }
            current = parent.map(|p| p.0);
        }
        hierarchy.iter().map(NavDestination::id).collect()
    }
}

impl PartialEq for NavDestination {
    fn eq(&self, other: &Self) -> bool {
        if self.same(other) {
            return true;
        }
        let base = self.id() == other.id()
            && self.route() == other.route()
            && *self.0.deep_links.borrow() == *other.0.deep_links.borrow()
            && *self.0.arguments.borrow() == *other.0.arguments.borrow()
            && self.is_graph() == other.is_graph();
        if !base {
            return false;
        }
        match (self.as_graph(), other.as_graph()) {
            (Some(a), Some(b)) => {
                let (an, bn) = (a.nodes_map(), b.nodes_map());
                an.len() == bn.len()
                    && a.start_destination_id() == b.start_destination_id()
                    && an.iter().all(|(id, n)| bn.get(id).is_some_and(|m| n == m))
            }
            _ => true,
        }
    }
}

impl fmt::Debug for NavDestination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let class = if self.is_graph() {
            "NavGraph"
        } else if self.0.kind.class_name.is_empty() {
            "NavDestination"
        } else {
            self.0.kind.class_name
        };
        write!(f, "{class}(")?;
        match &*self.0.id_name.borrow() {
            Some(n) => write!(f, "{n}")?,
            None => write!(f, "0x{:x}", self.id())?,
        }
        write!(f, ")")?;
        if let Some(r) = self.route().filter(|r| !r.trim().is_empty()) {
            write!(f, " route={r}")?;
        }
        if let Some(l) = self.label() {
            write!(f, " label={l}")?;
        }
        if let Some(g) = self.as_graph() {
            write!(f, " startDestination=")?;
            let start = g
                .find_node(g.start_destination_route().as_deref())
                .or_else(|| g.find_node_id(g.start_destination_id()));
            match start {
                Some(s) => write!(f, "{{{s:?}}}")?,
                None => match g.start_destination_route() {
                    Some(r) => write!(f, "{r}")?,
                    None => match &*g.data().start_dest_id_name.borrow() {
                        Some(n) => write!(f, "{n}")?,
                        None => write!(f, "0x{:x}", g.data().start_dest_id.get())?,
                    },
                },
            }
        }
        Ok(())
    }
}

impl fmt::Display for NavDestination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Compose's `NavDestination.hierarchy` and `hasRoute<T>()` as extension properties (Kotlin's
/// `destination.hierarchy.any { it.hasRoute<T>() }`).
pub trait NavDestinationExt {
    /// It and the graphs it is in, innermost first.
    fn hierarchy(&self) -> Vec<NavDestination>;
}

impl NavDestinationExt for Option<NavDestination> {
    fn hierarchy(&self) -> Vec<NavDestination> {
        self.as_ref().map(NavDestination::hierarchy).unwrap_or_default()
    }
}

// ---- NavGraph ----

/// A graph of destinations (Compose's `NavGraph`): a destination holding others, by id, one of them its start
/// destination (what navigating to the graph shows). It is a [`NavDestination`] (it derefs to it).
#[derive(Clone, PartialEq)]
pub struct NavGraph(pub(crate) NavDestination);

impl Deref for NavGraph {
    type Target = NavDestination;
    fn deref(&self) -> &NavDestination {
        &self.0
    }
}

impl fmt::Debug for NavGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl From<NavGraph> for NavDestination {
    fn from(g: NavGraph) -> NavDestination {
        g.0
    }
}

impl NavGraph {
    /// A graph of the navigator named `navigator_name` (Kotlin's `NavGraph(navGraphNavigator)`: its name is
    /// `"navigation"`).
    pub fn new(navigator_name: impl Into<String>) -> NavGraph {
        NavGraph::of_kind(navigator_name, NavDestinationKind { class_name: "NavGraph", ..Default::default() })
    }

    /// A graph of the kind a navigator's graphs are (a subclass of Kotlin's: navigation-compose's `ComposeNavGraph`
    /// with its transitions).
    pub fn of_kind(navigator_name: impl Into<String>, kind: NavDestinationKind) -> NavGraph {
        NavGraph(NavDestination::make(
            navigator_name.into(),
            kind,
            Some(GraphData {
                nodes: RefCell::new(BTreeMap::new()),
                start_dest_id: Cell::new(0),
                start_dest_id_name: RefCell::new(None),
                start_destination_route: RefCell::new(None),
            }),
        ))
    }

    fn data(&self) -> &GraphData {
        self.0.0.graph.as_ref().expect("a graph")
    }

    /// It as a destination.
    pub fn as_destination(&self) -> NavDestination {
        self.0.clone()
    }

    fn nodes_map(&self) -> BTreeMap<i32, NavDestination> {
        self.data().nodes.borrow().clone()
    }

    /// Its nodes, in their ids' order (Kotlin's `nodes`, iterating the graph).
    pub fn nodes(&self) -> Vec<NavDestination> {
        self.data().nodes.borrow().values().cloned().collect()
    }

    /// Its node of id `id` (Kotlin's `nodes[id]`).
    pub fn node(&self, id: i32) -> Option<NavDestination> {
        self.data().nodes.borrow().get(&id).cloned()
    }

    /// Puts `node` in the place of id `id`, its parent as it is (Kotlin's `nodes.replace(key, node)`).
    pub(crate) fn replace_node(&self, id: i32, node: NavDestination) {
        if let Some(slot) = self.data().nodes.borrow_mut().get_mut(&id) {
            // (its parent this graph: Kotlin's stays the new graph — kept by the node there, a weak reference here
            // that would end with it —, which equals this one)
            node.set_parent(Some(self));
            *slot = node;
        }
    }

    /// Adds `node` (Kotlin's `addDestination`): replaces a node of the same id.
    ///
    /// # Panics
    /// A node without id or route, of the graph's route or id, or already in another graph.
    pub fn add_destination(&self, node: NavDestination) {
        let id = node.id();
        let inner_route = node.route();
        assert!(
            id != 0 || inner_route.is_some(),
            "Destinations must have an id or route. Call setId(), setRoute(), or include an android:id or app:route \
             in your navigation XML."
        );
        if let Some(route) = self.route() {
            assert!(
                inner_route.as_deref() != Some(route.as_str()),
                "Destination {node:?} cannot have the same route as graph {self:?}"
            );
        }
        assert!(id != self.id(), "Destination {node:?} cannot have the same id as graph {self:?}");
        let existing = self.node(id);
        if existing.as_ref().is_some_and(|e| e.same(&node)) {
            return;
        }
        assert!(
            node.parent().is_none(),
            "Destination already has a parent set. Call NavGraph.remove() to remove the previous parent."
        );
        if let Some(e) = existing {
            e.set_parent(None);
        }
        node.set_parent(Some(self));
        self.data().nodes.borrow_mut().insert(id, node);
    }

    /// Adds each of `nodes` (Kotlin's `addDestinations`).
    pub fn add_destinations(&self, nodes: impl IntoIterator<Item = NavDestination>) {
        for n in nodes {
            self.add_destination(n);
        }
    }

    /// Its node (or a node of a graph in it, or of the graphs it is in) of id `id` (Kotlin's `findNode(resId)`).
    pub fn find_node_id(&self, id: i32) -> Option<NavDestination> {
        self.find_node_comprehensive(id, Some(&self.0), false, None)
    }

    /// Kotlin's `findNodeComprehensive`: its node of id `res_id`, then (searching children) its graphs', then its
    /// parents'; with `matching_dest`, only that one where it is.
    pub fn find_node_comprehensive(
        &self,
        res_id: i32,
        last_visited: Option<&NavDestination>,
        search_children: bool,
        matching_dest: Option<&NavDestination>,
    ) -> Option<NavDestination> {
        let mut destination = self.node(res_id);
        match matching_dest {
            Some(m) => {
                let ok = destination.as_ref().is_some_and(|d| d == m && d.parent() == m.parent());
                if ok {
                    return destination;
                }
                destination = None;
            }
            None => {
                if destination.is_some() {
                    return destination;
                }
            }
        }
        if search_children {
            destination = self.nodes().into_iter().find_map(|child| {
                let graph = child.as_graph()?;
                if last_visited.is_some_and(|l| *l == child) {
                    return None;
                }
                graph.find_node_comprehensive(res_id, Some(&self.0), true, matching_dest)
            });
        }
        destination.or_else(|| {
            let parent = self.parent()?;
            if last_visited.is_some_and(|l| *l == parent.0) {
                return None;
            }
            parent.find_node_comprehensive(res_id, Some(&self.0), search_children, matching_dest)
        })
    }

    /// Its node of `route` (a pattern or a path), or its parents' (Kotlin's `findNode(route)`).
    pub fn find_node(&self, route: Option<&str>) -> Option<NavDestination> {
        let route = route.filter(|r| !r.trim().is_empty())?;
        self.find_node_searching(route, true)
    }

    /// The node of `T`'s route (Kotlin's `findNode<T>()`).
    pub fn find_node_of<T: Route>(&self) -> Option<NavDestination> {
        self.find_node_class(RouteClass::of::<T>())
    }

    /// The node of `route`'s class (Kotlin's `findNode(KClass)`).
    pub fn find_node_class(&self, route: RouteClass) -> Option<NavDestination> {
        self.find_node_id(route.hash_code())
    }

    /// The node of `route`'s type (Kotlin's `findNode(route: T)`).
    pub fn find_node_object(&self, route: &dyn AnyRoute) -> Option<NavDestination> {
        self.find_node_id(hash_code_of(route.route_descriptor()))
    }

    /// Its node of `route`, matching patterns, then paths; its parents' when `search_parents` (Kotlin's
    /// `findNode(route, searchParents)`).
    pub fn find_node_searching(&self, route: &str, search_parents: bool) -> Option<NavDestination> {
        let found =
            self.nodes().into_iter().find(|n| n.route().as_deref() == Some(route) || n.match_route(route).is_some());
        found.or_else(|| if search_parents { self.parent()?.find_node(Some(route)) } else { None })
    }

    /// Moves all of `other`'s nodes into it (Kotlin's `addAll`).
    pub fn add_all(&self, other: &NavGraph) {
        for n in other.nodes() {
            other.remove(&n);
            self.add_destination(n);
        }
    }

    /// Removes `node` (Kotlin's `remove`).
    pub fn remove(&self, node: &NavDestination) {
        let removed = self.data().nodes.borrow_mut().remove(&node.id());
        if let Some(r) = removed {
            r.set_parent(None);
        }
    }

    /// Removes all its nodes (Kotlin's `clear`).
    pub fn clear(&self) {
        for n in self.nodes() {
            self.remove(&n);
        }
    }

    /// Its start destination's id (Kotlin's `startDestinationId`).
    pub fn start_destination_id(&self) -> i32 {
        self.data().start_dest_id.get()
    }

    /// Its start destination's route (Kotlin's `startDestinationRoute`).
    pub fn start_destination_route(&self) -> Option<String> {
        self.data().start_destination_route.borrow().clone()
    }

    /// Sets its start destination by id (Kotlin's `setStartDestination(startDestId)`).
    ///
    /// # Panics
    /// The graph's own id.
    pub fn set_start_destination_id(&self, start_dest_id: i32) {
        assert!(
            start_dest_id != self.id(),
            "Start destination {start_dest_id} cannot use the same id as the graph {self:?}"
        );
        *self.data().start_destination_route.borrow_mut() = None;
        self.data().start_dest_id.set(start_dest_id);
        *self.data().start_dest_id_name.borrow_mut() = None;
    }

    /// Sets its start destination by route (Kotlin's `setStartDestination(startDestRoute)`); its id is the route's
    /// hash code.
    ///
    /// # Panics
    /// The graph's own route, or a blank one.
    pub fn set_start_destination(&self, start_dest_route: impl Into<String>) {
        let route = start_dest_route.into();
        assert!(
            Some(&route) != self.route().as_ref(),
            "Start destination {route} cannot use the same route as the graph {self:?}"
        );
        assert!(!route.trim().is_empty(), "Cannot have an empty start destination route");
        self.data().start_dest_id.set(string_hash(&create_route(Some(&route))));
        *self.data().start_destination_route.borrow_mut() = Some(route);
    }

    /// Sets `T`'s destination as its start (Kotlin's `setStartDestination<T>()`).
    pub fn set_start_destination_of<T: Route>(&self) {
        self.set_start_destination_class(RouteClass::of::<T>());
    }

    /// Sets `route`'s class's destination as its start (Kotlin's `setStartDestination(KClass)`).
    ///
    /// # Panics
    /// If it has no such node.
    pub fn set_start_destination_class(&self, route: RouteClass) {
        self.set_start_destination_serializer(route.hash_code(), route.simple_name(), |d| d.route().expect("a route"));
    }

    /// Sets `route`'s destination as its start, with `route`'s values (Kotlin's `setStartDestination(route: T)`).
    pub fn set_start_destination_object(&self, route: &dyn AnyRoute) {
        let id = hash_code_of(route.route_descriptor());
        self.set_start_destination_serializer(id, route.route_descriptor().serial_name, |d| {
            generate_route_with_args(route, &d.arguments().types())
        });
    }

    fn set_start_destination_serializer(
        &self,
        id: i32,
        name: &str,
        parse_route: impl FnOnce(&NavDestination) -> String,
    ) {
        let start = self.find_node_id(id).unwrap_or_else(|| {
            panic!(
                "Cannot find startDestination {name} from NavGraph. Ensure the starting NavDestination was added \
                     with route from KClass."
            )
        });
        let route = parse_route(&start);
        // (the route set first, its id the class's after: Kotlin's bypass of the id's setter)
        self.set_start_destination(route);
        self.data().start_dest_id.set(id);
    }

    /// Its start destination's name (Kotlin's `startDestDisplayName`).
    pub fn start_dest_display_name(&self) -> String {
        let mut name = self.data().start_dest_id_name.borrow_mut();
        if name.is_none() {
            *name = Some(self.start_destination_route().unwrap_or_else(|| self.start_destination_id().to_string()));
        }
        name.clone().expect("set above")
    }

    /// The best match of `route` (Kotlin's `matchRouteComprehensive`): the graph's own, its nodes' (searching
    /// children), its parents' (searching parents), not again through `last_visited`.
    pub fn match_route_comprehensive(
        &self,
        route: &str,
        search_children: bool,
        search_parent: bool,
        last_visited: &NavDestination,
    ) -> Option<DeepLinkMatch> {
        let best = self.match_route(route);
        let best_child = if search_children {
            best_match(self.nodes().into_iter().filter_map(|child| {
                if child == *last_visited {
                    None
                } else if let Some(g) = child.as_graph() {
                    g.match_route_comprehensive(route, true, false, &self.0)
                } else {
                    child.match_route(route)
                }
            }))
        } else {
            None
        };
        let best_parent = self.parent().and_then(|p| {
            if search_parent && p.0 != *last_visited {
                p.match_route_comprehensive(route, search_children, true, &self.0)
            } else {
                None
            }
        });
        best_match([best, best_child, best_parent].into_iter().flatten())
    }

    /// The best match of `request` (Kotlin's `matchDeepLinkComprehensive`): the graph's own deep links', its nodes'
    /// (searching children), its parents' (searching parents).
    pub fn match_deep_link_comprehensive(
        &self,
        request: &NavDeepLinkRequest,
        search_children: bool,
        search_parent: bool,
        last_visited: &NavDestination,
    ) -> Option<DeepLinkMatch> {
        let own = self.0.match_own_deep_link(request);
        self.match_deep_link_comprehensive_from(own, request, search_children, search_parent, last_visited)
    }

    fn match_deep_link_comprehensive_from(
        &self,
        best: Option<DeepLinkMatch>,
        request: &NavDeepLinkRequest,
        search_children: bool,
        search_parent: bool,
        last_visited: &NavDestination,
    ) -> Option<DeepLinkMatch> {
        let best_child = if search_children {
            best_match(
                self.nodes()
                    .into_iter()
                    .filter_map(|child| if child != *last_visited { child.match_deep_link(request) } else { None }),
            )
        } else {
            None
        };
        let best_parent = self.parent().and_then(|p| {
            if search_parent && p.0 != *last_visited {
                p.match_deep_link_comprehensive(request, search_children, true, &self.0)
            } else {
                None
            }
        });
        best_match([best, best_child, best_parent].into_iter().flatten())
    }

    /// Its start destination, through the graphs that start it (Compose's `NavGraph.findStartDestination()`).
    pub fn find_start_destination(&self) -> NavDestination {
        self.child_hierarchy().pop().expect("the graph itself")
    }

    /// It, its start destination, that one's if it is a graph… (Kotlin's `childHierarchy()`).
    pub fn child_hierarchy(&self) -> Vec<NavDestination> {
        let mut out = vec![self.0.clone()];
        while let Some(g) = out.last().and_then(NavDestination::as_graph) {
            match g.node(g.start_destination_id()) {
                Some(n) => out.push(n),
                None => break,
            }
        }
        out
    }
}

impl NavGraph {
    /// Kotlin's `NavGraph.iterator()` (its nodes).
    pub fn iter(&self) -> std::vec::IntoIter<NavDestination> {
        self.nodes().into_iter()
    }
}

impl NavDestination {
    /// Compose's `hasRoute(T::class)` for a value's type.
    pub fn has_route_of(&self, route: &dyn AnyRoute) -> bool {
        hash_code_of(route.route_descriptor()) == self.id()
    }
}

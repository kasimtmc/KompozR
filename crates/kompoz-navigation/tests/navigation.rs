//! Navigation's core as Compose's behaves (navigation-common and navigation-runtime 2.10): routes and deep links
//! matched, arguments typed, the controller's back stack (nested graphs, popUpTo, launchSingleTop, saved and
//! restored stacks, deep links), entries' lifecycles, the controller's saved state, typed routes. The non-UI parts of
//! the C++ Kompoz's navigation tests (kompoz_tests.cpp) by name; their NavHost halves are WP3's (navigation-compose).
//!
//! The destinations here are a test navigator's (navigation-testing's `TestNavigator`): navigating pushes,
//! popping pops.

use kompoz_lifecycle::{
    CreationExtras, LifecycleOwner, LifecycleRegistry, LifecycleState, SavedStateHandle, ViewModel, ViewModelImpl,
    ViewModelProvider, ViewModelStore,
};
use kompoz_navigation::*;
use kompoz_runtime::SavedValue;
use kompoz_runtime::kt::{map_of, type_of};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

// ---- Test navigator and helpers ----

#[derive(Default)]
struct TestNavigator {
    base: NavigatorBase,
}

impl Navigator for TestNavigator {
    fn name(&self) -> String {
        "test".into()
    }
    fn base(&self) -> &NavigatorBase {
        &self.base
    }
    fn create_destination(&self) -> NavDestination {
        NavDestination::new("test")
    }
}

/// A controller with the test navigator.
fn controller() -> NavController {
    let nav = NavController::new();
    nav.navigator_provider().add_navigator(Rc::new(TestNavigator::default()));
    nav
}

fn test_navigator(b: &NavGraphBuilder) -> Rc<dyn Navigator> {
    b.provider().get_navigator("test")
}

/// A test destination of `route`.
fn screen(b: &mut NavGraphBuilder, route: &str) {
    let d = NavDestinationBuilder::new(test_navigator(b), Some(route.into()));
    b.destination(d);
}

/// A test destination of `route` with deep links.
fn screen_with_links(b: &mut NavGraphBuilder, route: &str, links: &[&str]) {
    let mut d = NavDestinationBuilder::new(test_navigator(b), Some(route.into()));
    for l in links {
        d.deep_link(l);
    }
    b.destination(d);
}

/// `T`'s test destination.
fn typed_screen<T: Route>(b: &mut NavGraphBuilder, deep_link_base: Option<&str>) {
    let mut d = NavDestinationBuilder::new_of::<T>(test_navigator(b), TypeMap::default());
    if let Some(base) = deep_link_base {
        d.deep_link_of::<T>(base);
    }
    b.destination(d);
}

/// The back stack as the C++ tests write it: routes, a graph's in brackets, the root graph left out.
fn stack_of(nav: &NavController) -> String {
    let root = nav.graph().as_destination();
    nav.back_queue()
        .iter()
        .filter(|e| !e.destination().same(&root))
        .map(|e| {
            let d = e.destination();
            let r = d.route().unwrap_or_default();
            if d.is_graph() { format!("[{r}]") } else { r }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn routes(destinations: &[NavDestination]) -> Vec<String> {
    destinations.iter().map(|d| d.route().unwrap_or_default()).collect()
}

struct Host {
    registry: LifecycleRegistry,
}

impl LifecycleOwner for Host {
    fn lifecycle(&self) -> kompoz_lifecycle::Lifecycle {
        self.registry.lifecycle()
    }
}

fn host(state: LifecycleState) -> Rc<Host> {
    let h = Rc::new_cyclic(|w: &Weak<Host>| Host { registry: LifecycleRegistry::new(w.clone()) });
    h.registry.set_current_state(state);
    h
}

// ---- Routes and deep links ----

#[test]
fn routes_match_their_patterns_and_give_their_arguments() {
    let d = NavDestination::new("test");
    d.add_argument("id", nav_argument("id", |a| a.set_type(NavType::IntType)).argument());
    d.set_route(Some("item/{id}?tab={tab}".into()));
    let m = d.match_route("item/42?tab=b").expect("a match");
    let args = m.matching_args().expect("arguments");
    assert_eq!(args.get_int("id"), 42);
    assert_eq!(args.get_string("tab").as_deref(), Some("b"));
    // A value its type can't parse isn't this destination's.
    assert!(d.match_route("item/x").is_none());
    assert!(d.has_route_with("item/{id}?tab={tab}", None));
    let entry_args = args.clone();
    assert!(d.has_route_with("item/42", Some(&entry_args)) && !d.has_route_with("item/43", Some(&entry_args)));
    assert_eq!(d.id(), string_hash_of("android-app://androidx.navigation/item/{id}?tab={tab}"));
}

fn string_hash_of(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

#[test]
#[should_panic(expected = "Following required arguments are missing")]
fn a_route_without_a_required_argument_is_refused() {
    let d = NavDestination::new("test");
    d.add_argument("id", nav_argument("id", |a| a.set_type(NavType::IntType)).argument());
    d.set_route(Some("item".into()));
}

#[test]
fn deep_links_match_paths_queries_fragments_actions_and_mime_types() {
    let link = NavDeepLink::from_uri_pattern("kompoz://item/{n}");
    let mut args = NavArguments::new();
    args.insert("n", nav_argument("n", |a| a.set_type(NavType::IntType)).argument());
    // The scheme in any case; a query or fragment after the path.
    let m = link.get_matching_arguments(&NavUri("KOMPOZ://item/7?x=1#top"), &args).expect("a match");
    assert_eq!(m.get_int("n"), 7);
    assert!(link.get_matching_arguments(&NavUri("kompoz://item/x"), &args).is_none());
    assert!(link.get_matching_arguments(&NavUri("kompoz://item/7/more"), &args).is_none());
    // Without a scheme, http and https.
    let web = NavDeepLink::from_uri_pattern("example.com/{a}");
    assert!(web.get_matching_arguments(&NavUri("https://example.com/b"), &NavArguments::new()).is_some());
    assert!(web.get_matching_arguments(&NavUri("ftp://example.com/b"), &NavArguments::new()).is_none());
    // A query parameter's pattern: several arguments in one, values decoded, '+' a space.
    let q = NavDeepLink::from_uri_pattern("kompoz://user?name={first}_{last}&q={q}");
    let m = q
        .get_matching_arguments(&NavUri("kompoz://user?name=Ada_Lovelace&q=a+b%26c"), &NavArguments::new())
        .expect("a match");
    assert_eq!(m.get_string("first").as_deref(), Some("Ada"));
    assert_eq!(m.get_string("last").as_deref(), Some("Lovelace"));
    assert_eq!(m.get_string("q").as_deref(), Some("a b&c"));
    // A list argument: repeated parameters; none: an empty list.
    let mut list_args = NavArguments::new();
    list_args.insert("ids", nav_argument("ids", |a| a.set_type(NavType::IntListType)).argument());
    let l = NavDeepLink::from_uri_pattern("kompoz://list?ids={ids}");
    let m = l.get_matching_arguments(&NavUri("kompoz://list?ids=1&ids=2&ids=3"), &list_args).expect("a match");
    assert_eq!(NavType::IntListType.get::<Vec<i32>>(&m, "ids"), Some(vec![1, 2, 3]));
    let m = l.get_matching_arguments(&NavUri("kompoz://list"), &list_args).expect("a match");
    assert_eq!(NavType::IntListType.get::<Vec<i32>>(&m, "ids"), Some(vec![]));
    // A fragment's arguments.
    let f = NavDeepLink::from_uri_pattern("kompoz://doc#{section}");
    let m = f.get_matching_arguments(&NavUri("kompoz://doc#intro"), &NavArguments::new()).expect("a match");
    assert_eq!(m.get_string("section").as_deref(), Some("intro"));
    // Wildcards.
    let w = NavDeepLink::from_uri_pattern("kompoz://files/.*/end");
    assert!(w.get_matching_arguments(&NavUri("kompoz://files/a/b/end"), &NavArguments::new()).is_some());
    // Actions and MIME types.
    let dest = NavDestination::new("test");
    dest.set_route(Some("share".into()));
    dest.add_nav_deep_link(nav_deep_link(|b| {
        b.set_action(Some("SEND".into()));
        b.set_mime_type(Some("image/*".into()));
    }));
    let request = NavDeepLinkRequestBuilder::from_action("SEND").set_mime_type("image/png").build();
    assert!(dest.has_deep_link_request(&request));
    let wrong = NavDeepLinkRequestBuilder::from_action("VIEW").set_mime_type("image/png").build();
    assert!(!dest.has_deep_link_request(&wrong));
    assert_eq!(NavDeepLinkBuilder::new().set_mime_type("text/*").build().get_mime_type_match_rating("text/plain"), 2);
}

// ---- NavTypes and arguments ----

#[test]
fn arguments_take_their_defaults_and_refuse_values_of_other_types() {
    let d = NavDestination::new("test");
    d.add_argument(
        "n",
        nav_argument("n", |a| {
            a.set_type(NavType::IntType);
            a.set_default_value(5);
        })
        .argument(),
    );
    d.add_argument(
        "s",
        nav_argument("s", |a| {
            a.set_type(NavType::StringType);
            a.set_nullable(true);
            a.set_default_value_null();
        })
        .argument(),
    );
    let args = d.add_in_default_args(None).expect("defaults");
    assert_eq!(args.get_int("n"), 5);
    assert!(!args.contains_key("s"));
    let given = saved_state(|w| w.put_int("n", 9));
    assert_eq!(d.add_in_default_args(Some(&given)).expect("args").get_int("n"), 9);
    let wrong = saved_state(|w| w.put_string("n", "x"));
    let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| d.add_in_default_args(Some(&wrong))));
    assert!(refused.is_err());
    // A type inferred from the default.
    let inferred = nav_argument("x", |a| a.set_default_value(1.5f32)).argument();
    assert_eq!(inferred.r#type(), NavType::FloatType);
}

/// A type of one's own and its NavType (Compose's custom NavType through a typeMap).
#[derive(Clone, PartialEq, Debug, RouteValue)]
struct SearchParameters {
    text: String,
    count: i32,
}

struct SearchParametersType;

impl NavTypeImpl for SearchParametersType {
    type Value = SearchParameters;
    fn is_nullable_allowed(&self) -> bool {
        false
    }
    fn name(&self) -> String {
        "SearchParameters".into()
    }
    fn put(&self, bundle: &SavedState, key: &str, value: Option<&SearchParameters>) {
        if let Some(v) = value {
            bundle.write(|w| w.put_string(key, format!("{}:{}", v.text, v.count)));
        }
    }
    fn get(&self, bundle: &SavedState, key: &str) -> Option<SearchParameters> {
        bundle.get_string(key).and_then(|s| self.parse_value(&s).ok().flatten())
    }
    fn parse_value(&self, value: &str) -> Result<Option<SearchParameters>, NavTypeError> {
        let (t, c) = value.split_once(':').ok_or_else(|| NavTypeError("no ':'".into()))?;
        Ok(Some(SearchParameters { text: t.into(), count: c.parse().map_err(|_| NavTypeError("a count".into()))? }))
    }
    fn serialize_as_value(&self, value: Option<&SearchParameters>) -> String {
        value.map_or("null".into(), |v| format!("{}:{}", v.text, v.count))
    }
}

#[derive(Route, Clone, PartialEq, Debug)]
struct Search {
    parameters: SearchParameters,
    #[route(default = 3)]
    page: i32,
    #[route(name = "filters")]
    tags: Vec<String>,
}

#[test]
fn typed_routes_take_custom_types_defaults_lists_and_serial_names() {
    let types = map_of([(type_of::<SearchParameters>(), NavType::new(SearchParametersType))]);
    assert_eq!(generate_route_pattern::<Search>(&types, None), "Search/{parameters}?page={page}&filters={filters}");
    let args = generate_nav_arguments::<Search>(&types);
    assert_eq!(args.iter().map(NamedNavArgument::name).collect::<Vec<_>>(), ["parameters", "page", "filters"]);
    assert!(args[1].argument().is_default_value_present() && !args[0].argument().is_default_value_present());
    assert!(args[2].argument().r#type().is_collection());
    // Without the typeMap the custom type has no NavType.
    let missing = std::panic::catch_unwind(|| generate_route_pattern::<Search>(&TypeMap::default(), None));
    assert!(missing.is_err());
    let nav = controller();
    let graph = nav.create_graph("home", None, |b| {
        screen(b, "home");
        let d = NavDestinationBuilder::new_of::<Search>(test_navigator(b), types.clone());
        b.destination(d);
    });
    nav.set_graph(graph);
    nav.navigate(Search {
        parameters: SearchParameters { text: "a b".into(), count: 2 },
        page: 7,
        tags: vec!["x".into(), "y z".into()],
    });
    let entry = nav.current_back_stack_entry().expect("an entry");
    assert!(entry.destination().has_route::<Search>());
    let s = entry.to_route::<Search>();
    assert_eq!(s.parameters, SearchParameters { text: "a b".into(), count: 2 });
    assert_eq!((s.page, s.tags), (7, vec!["x".to_string(), "y z".to_string()]));
    // The query parameters left out: the default, an empty list.
    nav.navigate("Search/q:1");
    let s = nav.current_back_stack_entry().expect("an entry").to_route::<Search>();
    assert_eq!((s.parameters.count, s.page, s.tags.len()), (1, 3, 0));
}

// ---- C++ Kompoz's type_safe_routes_carry_their_arguments (its controller part) ----

#[derive(Route, Clone, PartialEq, Debug)]
struct HomeRoute;

#[derive(RouteValue, Clone, Copy, PartialEq, Debug)]
enum ProfileTab {
    Posts,
    Likes,
}

#[derive(Route, Clone, PartialEq, Debug)]
struct ProfileRoute {
    id: String,
    pro: bool,
    tab: Option<ProfileTab>,
    zoom: Option<f64>,
}

#[derive(Route, Clone, PartialEq, Debug)]
#[route(name = "login")]
struct LoginGraph;

#[derive(Route, Clone, PartialEq, Debug)]
#[route(name = "login/username")]
struct UsernameRoute {
    attempt: i32,
}

#[test]
fn type_safe_routes_carry_their_arguments() {
    assert_eq!(generate_route_pattern::<HomeRoute>(&TypeMap::default(), None), "HomeRoute");
    let pattern = "ProfileRoute/{id}/{pro}?tab={tab}&zoom={zoom}";
    assert_eq!(generate_route_pattern::<ProfileRoute>(&TypeMap::default(), None), pattern);
    // (Compose writes a null argument "null"; the C++ left it out)
    let types: Vec<(String, NavType)> = generate_nav_arguments::<ProfileRoute>(&TypeMap::default())
        .iter()
        .map(|a| (a.name(), a.argument().r#type()))
        .collect();
    let path =
        generate_route_with_args(&ProfileRoute { id: "a b/c".into(), pro: true, tab: None, zoom: Some(1.5) }, &types);
    assert_eq!(path, "ProfileRoute/a%20b%2Fc/true?tab=null&zoom=1.5");
    assert_eq!(
        nav_deep_link_of::<ProfileRoute>("kompoz://profile").uri_pattern().as_deref(),
        Some("kompoz://profile/{id}/{pro}?tab={tab}&zoom={zoom}")
    );
    let nav = controller();
    let graph = nav.create_graph_typed(HomeRoute, None, TypeMap::default(), |b| {
        typed_screen::<HomeRoute>(b, None);
        typed_screen::<ProfileRoute>(b, Some("kompoz://profile"));
        b.navigation_of::<LoginGraph>(UsernameRoute { attempt: 0 }, TypeMap::default(), |login| {
            typed_screen::<UsernameRoute>(login, None);
        });
    });
    nav.set_graph(graph);
    assert_eq!(stack_of(&nav), "HomeRoute");
    assert!(nav.current_destination().expect("one").has_route::<HomeRoute>());
    nav.navigate(ProfileRoute { id: "a b/c".into(), pro: true, tab: Some(ProfileTab::Likes), zoom: Some(2.25) });
    let seen = nav.current_back_stack_entry().expect("an entry").to_route::<ProfileRoute>();
    assert_eq!(seen, ProfileRoute { id: "a b/c".into(), pro: true, tab: Some(ProfileTab::Likes), zoom: Some(2.25) });
    nav.navigate(ProfileRoute { id: "x".into(), pro: false, tab: None, zoom: None });
    let seen = nav.current_back_stack_entry().expect("an entry").to_route::<ProfileRoute>();
    assert_eq!(seen, ProfileRoute { id: "x".into(), pro: false, tab: None, zoom: None });
    nav.navigate(LoginGraph); // a graph: its start destination
    assert_eq!(stack_of(&nav), format!("HomeRoute {pattern} {pattern} [login] login/username/{{attempt}}"));
    assert_eq!(nav.current_back_stack_entry().expect("one").to_route::<UsernameRoute>().attempt, 0);
    assert!(nav.get_back_stack_entry_of::<LoginGraph>().destination().is_graph());
    nav.navigate(UsernameRoute { attempt: 2 });
    assert_eq!(nav.current_back_stack_entry().expect("one").to_route::<UsernameRoute>().attempt, 2);
    assert_eq!(nav.get_back_stack_entry(UsernameRoute { attempt: 2 }), nav.current_back_stack_entry().expect("one"));
    assert!(nav.pop_back_stack_of::<LoginGraph>(true, false));
    assert!(nav.current_destination().expect("one").has_route::<ProfileRoute>());
    // (Compose's enum arguments are by name)
    assert!(nav.handle_deep_link_uri("kompoz://profile/derin/false?tab=posts"));
    assert_eq!(stack_of(&nav), format!("HomeRoute {pattern}"));
    let seen = nav.current_back_stack_entry().expect("one").to_route::<ProfileRoute>();
    assert_eq!((seen.id.as_str(), seen.tab, seen.pro), ("derin", Some(ProfileTab::Posts), false));
    // popUpTo<T> { inclusive } and the hierarchy's hasRoute<T>.
    nav.navigate(LoginGraph);
    let in_login =
        |nav: &NavController| nav.current_destination().hierarchy().iter().any(|d| d.has_route::<LoginGraph>());
    assert!(in_login(&nav));
    nav.navigate_with(HomeRoute, |o| o.pop_up_to_of::<HomeRoute>(|p| p.set_inclusive(true)));
    assert_eq!(stack_of(&nav), "HomeRoute");
    assert!(!in_login(&nav));
}

// ---- C++ Kompoz's nested_navigation_graphs_are_entered_by_route_and_left_with_their_screens ----

fn nested_graph(nav: &NavController) -> NavGraph {
    nav.create_graph("home", None, |g| {
        screen(g, "home");
        g.navigation("username", "login", |login| {
            screen(login, "username");
            screen(login, "password");
            login.navigation("code", "verify", |verify| screen(verify, "code"));
        });
        screen(g, "profile");
    })
}

#[test]
fn nested_navigation_graphs_are_entered_by_route_and_left_with_their_screens() {
    let nav = controller();
    nav.set_graph(nested_graph(&nav));
    nav.navigate("login"); // its start destination, in an entry of the graph
    assert_eq!(stack_of(&nav), "home [login] username");
    assert_eq!(routes(&nav.current_destination().expect("one").hierarchy()), ["username", "login", ""]);
    assert_eq!(nav.previous_back_stack_entry().expect("one").destination().route().as_deref(), Some("home"));
    nav.navigate("password"); // in the same graph: in the same entry
    assert_eq!(stack_of(&nav), "home [login] username password");
    assert_eq!(nav.previous_back_stack_entry().expect("one").destination().route().as_deref(), Some("username"));
    assert!(nav.get_back_stack_entry("login").destination().is_graph());
    nav.navigate("verify");
    assert_eq!(stack_of(&nav), "home [login] username password [verify] code");
    assert_eq!(routes(&nav.current_destination().expect("one").hierarchy()), ["code", "verify", "login", ""]);
    nav.pop_back_stack(); // a graph's last screen: the graph goes with it
    assert_eq!(stack_of(&nav), "home [login] username password");
    nav.navigate_with("profile", |o| o.pop_up_to("login", |p| p.set_inclusive(true))); // the whole flow off
    assert_eq!(stack_of(&nav), "home profile");
    nav.navigate("password"); // straight to a graph's screen: the graph comes with it
    assert_eq!(stack_of(&nav), "home profile [login] password");
    nav.navigate("login"); // the graph's route from inside it: another one
    assert_eq!(stack_of(&nav), "home profile [login] password [login] username");
    let stack = nav.back_queue();
    assert!(stack.len() == 7 && stack[3] != stack[5]);
    assert!(nav.pop_back_stack_to("password", false, false) && stack_of(&nav) == "home profile [login] password");
    assert!(nav.pop_back_stack() && stack_of(&nav) == "home profile");
    assert!(nav.pop_back_stack() && stack_of(&nav) == "home");
    // (Compose pops the last destination too: the stack is left empty, false — the C++ refused)
    assert!(!nav.pop_back_stack() && nav.back_queue().is_empty());
    nav.navigate("home");
    nav.navigate("login");
    assert!(nav.pop_back_stack_to("username", true, false) && stack_of(&nav) == "home"); // the graph left without screens goes
    nav.navigate("login");
    nav.navigate_with("password", |o| o.pop_up_to("login", |_| {})); // down to the graph's entry, and on in it
    assert_eq!(stack_of(&nav), "home [login] password");
    nav.navigate_with("profile", |o| o.pop_up_to("login", |_| {})); // a graph left without screens isn't kept
    assert_eq!(stack_of(&nav), "home profile");
    assert_eq!(nav.previous_back_stack_entry().expect("one").destination().route().as_deref(), Some("home"));
    assert!(nav.navigate_up() && stack_of(&nav) == "home" && !nav.navigate_up());
}

// ---- C++ Kompoz's deep_links_open_their_destination_with_the_screens_above_it (its controller part) ----

fn deep_link_graph(nav: &NavController) -> NavGraph {
    nav.create_graph("home", None, |g| {
        screen(g, "home");
        g.navigation("list/all", "list", |list| {
            screen(list, "list/all");
            screen_with_links(list, "item/{n}", &["kompoz://item/{n}", "https://example.com/items/{n}"]);
        });
        screen_with_links(g, "search?q={q}", &["kompoz://search?q={q}"]);
        screen_with_links(g, "tag/{name}", &["kompoz://tags/{name}"]);
    })
}

#[test]
fn deep_links_open_their_destination_with_the_screens_above_it() {
    let links = kompoz_ui::platform::DeepLinks::new();
    links.open("kompoz://item/42"); // the app is opened with it
    let nav = controller();
    let destinations = Rc::new(RefCell::new(Vec::new()));
    let d = destinations.clone();
    nav.add_on_destination_changed_listener(Rc::new(
        move |_: &NavController, dest: &NavDestination, args: Option<&SavedState>| {
            let n = args.map(|a| a.get_string("n").unwrap_or_default()).unwrap_or_default();
            d.borrow_mut().push(format!("{} {n}", dest.route().unwrap_or_default()));
        },
    ));
    nav.set_deep_links(Some(links.clone()));
    nav.set_graph(deep_link_graph(&nav));
    assert_eq!(stack_of(&nav), "home [list] list/all item/{n}");
    assert_eq!(
        nav.current_back_stack_entry().expect("one").arguments().expect("args").get_string("n").as_deref(),
        Some("42")
    );
    assert!(links.pending().is_none() && nav.deep_link_handled());
    assert_eq!(destinations.borrow().last().map(String::as_str), Some("item/{n} 42"));
    assert!(nav.pop_back_stack()); // back leads up
    assert_eq!(nav.current_destination().expect("one").route().as_deref(), Some("list/all"));
    assert!(nav.pop_back_stack() && stack_of(&nav) == "home");
    links.open("HTTPS://example.com/items/7#top"); // sent while it runs; the scheme in any case
    assert_eq!(stack_of(&nav), "home [list] list/all item/{n}");
    assert_eq!(
        nav.current_back_stack_entry().expect("one").arguments().expect("args").get_string("n").as_deref(),
        Some("7")
    );
    nav.navigate(NavUri("kompoz://search?q=a%20b%26c")); // from inside the app: on top
    assert_eq!(stack_of(&nav), "home [list] list/all item/{n} search?q={q}");
    assert_eq!(
        nav.current_back_stack_entry().expect("one").arguments().expect("args").get_string("q").as_deref(),
        Some("a b&c")
    );
    assert!(!nav.handle_deep_link_uri("kompoz://nothing") && !nav.handle_deep_link_uri("other://item/3"));
    links.open("kompoz://nothing"); // left for another
    assert_eq!(stack_of(&nav), "home [list] list/all item/{n} search?q={q}");
    assert_eq!(links.take(|_| true).as_deref(), Some("kompoz://nothing"));
    assert!(nav.handle_deep_link_uri("item/5")); // a route's path (the web's #fragment)
    assert_eq!(stack_of(&nav), "home [list] list/all item/{n}");
    assert!(nav.handle_deep_link_uri("list") && stack_of(&nav) == "home [list] list/all"); // a graph: its start
    assert!(nav.handle_deep_link_uri("home") && stack_of(&nav) == "home");
    assert!(nav.handle_deep_link_uri("kompoz://tags/a%2Fb%20c%3F")); // decoded once
    assert_eq!(stack_of(&nav), "home tag/{name}");
    assert_eq!(
        nav.current_back_stack_entry().expect("one").arguments().expect("args").get_string("name").as_deref(),
        Some("a/b c?")
    );
    // Its handler goes with it.
    nav.set_deep_links(None);
    links.open("kompoz://item/1");
    assert_eq!(links.pending().as_deref(), Some("kompoz://item/1"));
}

// ---- C++ Kompoz's a_nested_graph_is_saved_and_restored_as_a_tab (its controller part) ----

fn tab_graph(nav: &NavController) -> NavGraph {
    nav.create_graph("home", None, |g| {
        screen(g, "home");
        g.navigation("list/all", "list", |list| {
            screen(list, "list/all");
            screen(list, "item/{n}");
        });
        screen(g, "settings");
    })
}

#[test]
fn a_nested_graph_is_saved_and_restored_as_a_tab() {
    let nav = controller();
    nav.set_graph(tab_graph(&nav));
    let tab = |route: &str| {
        nav.navigate_with(route, |o| {
            o.pop_up_to("home", |p| p.set_save_state(true));
            o.set_launch_single_top(true);
            o.set_restore_state(true);
        })
    };
    let in_list = |nav: &NavController| routes(&nav.current_destination().hierarchy()).iter().any(|r| r == "list");
    tab("list");
    nav.navigate("item/7");
    assert!(in_list(&nav));
    let graph_id = nav.get_back_stack_entry("list").id();
    let item_id = nav.current_back_stack_entry().expect("one").id();
    tab("settings");
    assert_eq!(stack_of(&nav), "home settings"); // saved, the graph with it
    assert!(!in_list(&nav));
    tab("list");
    assert_eq!(stack_of(&nav), "home [list] list/all item/{n}");
    assert_eq!(nav.get_back_stack_entry("list").id(), graph_id); // the same entries, by id
    assert_eq!(nav.current_back_stack_entry().expect("one").id(), item_id);
    assert_eq!(
        nav.current_back_stack_entry().expect("one").arguments().expect("args").get_string("n").as_deref(),
        Some("7")
    );
    tab("home");
    tab("home"); // launchSingleTop: still one
    assert_eq!(stack_of(&nav), "home");
    nav.navigate("list"); // without restoreState: a new graph
    nav.navigate("item/8");
    assert_eq!(stack_of(&nav), "home [list] list/all item/{n}");
    assert_ne!(nav.get_back_stack_entry("list").id(), graph_id);
    tab("settings"); // saved in place of the first one
    assert_eq!(stack_of(&nav), "home settings");
    // The saved stack, cleared.
    assert!(nav.clear_back_stack("list"));
    tab("list");
    assert_eq!(stack_of(&nav), "home [list] list/all");
}

// ---- Compose's controller behaviour ----

#[test]
fn launch_single_top_replaces_the_top_entry_keeping_its_id() {
    let nav = controller();
    nav.set_graph(nav.create_graph("home", None, |g| {
        screen(g, "home");
        screen(g, "item/{n}");
    }));
    nav.navigate("item/1");
    let first = nav.current_back_stack_entry().expect("one");
    nav.navigate_with("item/2", |o| o.set_launch_single_top(true));
    let second = nav.current_back_stack_entry().expect("one");
    assert_eq!(stack_of(&nav), "home item/{n}");
    assert!(first != second && first.id() == second.id());
    assert_eq!(second.arguments().expect("args").get_string("n").as_deref(), Some("2"));
    nav.navigate("item/3");
    assert_eq!(stack_of(&nav), "home item/{n} item/{n}");
}

#[test]
fn entries_lifecycles_follow_their_place_capped_by_the_host() {
    let nav = controller();
    let h = host(LifecycleState::Resumed);
    nav.set_lifecycle_owner(h.clone());
    nav.set_graph(nav.create_graph("home", None, |g| {
        screen(g, "home");
        screen(g, "detail");
    }));
    let home = nav.current_back_stack_entry().expect("one");
    assert_eq!(home.lifecycle().current_state(), LifecycleState::Resumed);
    nav.navigate("detail");
    let detail = nav.current_back_stack_entry().expect("one");
    assert_eq!(home.lifecycle().current_state(), LifecycleState::Created);
    assert_eq!(detail.lifecycle().current_state(), LifecycleState::Resumed);
    let graph = nav.back_queue()[0].clone();
    assert_eq!(graph.lifecycle().current_state(), LifecycleState::Resumed); // the graph of the top destination
    h.registry.set_current_state(LifecycleState::Started); // the host's state caps them
    assert_eq!(detail.lifecycle().current_state(), LifecycleState::Started);
    h.registry.set_current_state(LifecycleState::Resumed);
    nav.pop_back_stack();
    assert_eq!(detail.lifecycle().current_state(), LifecycleState::Destroyed);
    assert_eq!(home.lifecycle().current_state(), LifecycleState::Resumed);
    // The visible entries: those at least started.
    assert_eq!(nav.visible_entries().value(), vec![home.clone()]);
    // Without a lifecycle owner, entries are created at most.
    let other = controller();
    other.set_graph(other.create_graph("home", None, |g| screen(g, "home")));
    assert_eq!(other.current_back_stack_entry().expect("one").lifecycle().current_state(), LifecycleState::Created);
}

#[test]
fn listeners_and_flows_follow_the_destination() {
    let nav = controller();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let listener: Rc<dyn OnDestinationChangedListener> =
        Rc::new(move |_: &NavController, d: &NavDestination, _: Option<&SavedState>| {
            s.borrow_mut().push(d.route().unwrap_or_default())
        });
    nav.set_graph(nav.create_graph("home", None, |g| {
        screen(g, "home");
        screen(g, "a");
    }));
    nav.add_on_destination_changed_listener(listener.clone()); // told of the current one at once
    nav.navigate("a");
    nav.pop_back_stack();
    nav.remove_on_destination_changed_listener(&listener);
    nav.navigate("a");
    assert_eq!(*seen.borrow(), ["home", "a", "home"]);
    assert_eq!(nav.current_back_stack().value().len(), 3); // the graph's entry too
    assert_eq!(
        nav.current_back_stack_entry_flow().replay_cache().last().and_then(|e| e.destination().route()).as_deref(),
        Some("a")
    );
}

#[test]
fn the_controller_saves_its_stacks_and_restores_them() {
    let nav = controller();
    nav.set_graph(tab_graph(&nav));
    nav.navigate("list");
    nav.navigate("item/3");
    let ids: Vec<String> = nav.back_queue().iter().map(NavBackStackEntry::id).collect();
    nav.navigate_with("settings", |o| o.pop_up_to("home", |p| p.set_save_state(true)));
    let saved = nav.save_state().expect("a state");
    // Written out as bytes and read back, as the host writes it.
    let bytes = SavedValue::encode(&saved.to_saved_value());
    let back = SavedState::from_saved_value(&SavedValue::decode(&bytes).expect("decoded"));
    let restored = controller();
    restored.restore_state(Some(back));
    restored.set_graph(tab_graph(&restored));
    assert_eq!(stack_of(&restored), "home settings");
    restored.navigate_with("list", |o| {
        o.pop_up_to("home", |p| p.set_save_state(true));
        o.set_restore_state(true);
    });
    assert_eq!(stack_of(&restored), "home [list] list/all item/{n}");
    let restored_ids: Vec<String> = restored.back_queue().iter().skip(2).map(NavBackStackEntry::id).collect();
    assert_eq!(restored_ids, ids[2..]);
    assert_eq!(
        restored.current_back_stack_entry().expect("one").arguments().expect("args").get_string("n").as_deref(),
        Some("3")
    );
}

#[test]
fn nav_options_are_built_as_composes() {
    let o = nav_options(|b| {
        b.pop_up_to_of::<HomeRoute>(|p| {
            p.set_inclusive(true);
            p.set_save_state(true);
        });
        b.set_launch_single_top(true);
        b.set_restore_state(true);
    });
    assert!(
        o.should_launch_single_top()
            && o.should_restore_state()
            && o.is_pop_up_to_inclusive()
            && o.should_pop_up_to_save_state()
    );
    assert_eq!(o.pop_up_to_route_class(), Some(RouteClass::of::<HomeRoute>()));
    assert_eq!(o.pop_up_to_id(), generate_hash_code::<HomeRoute>());
    let p = nav_options(|b| b.pop_up_to("home", |_| {}));
    assert_eq!(p.pop_up_to_route().as_deref(), Some("home"));
    assert!(!p.is_pop_up_to_inclusive());
    assert_eq!(
        format!("{o:?}"),
        "NavOptions(launchSingleTop restoreState popUpTo(class navigation::HomeRoute inclusive saveState))"
    );
    assert_eq!(
        nav_options(|b| b.pop_up_to("home", |p| p.set_inclusive(true))),
        NavOptions::builder().set_pop_up_to(Some("home".into()), true, false).build()
    );
}

// ---- Entries' ViewModels and saved states (WP1's) ----

thread_local! {
    static CLEARED: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

struct NoteViewModel {
    base: ViewModelImpl,
    handle: SavedStateHandle,
}

impl ViewModel for NoteViewModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
    fn on_cleared(&self) {
        CLEARED.with(|c| c.set(c.get() + 1));
    }
    fn new_instance(extras: &CreationExtras) -> Option<Self> {
        Some(NoteViewModel { base: ViewModelImpl::new(), handle: extras.create_saved_state_handle() })
    }
}

/// The detail destination's arguments as a typed route (`SavedStateHandle.toRoute<T>()`).
#[derive(Route, Clone, PartialEq, Debug)]
struct DetailArgs {
    id: String,
}

fn note_view_model(entry: &NavBackStackEntry) -> Rc<NoteViewModel> {
    let owner = entry.as_view_model_store_owner();
    ViewModelProvider::create(&*owner, None, None).get::<NoteViewModel>()
}

fn detail_graph(nav: &NavController) -> NavGraph {
    nav.create_graph("home", None, |g| {
        screen(g, "home");
        screen(g, "detail/{id}");
    })
}

fn view_model_controller(h: &Rc<Host>) -> NavController {
    let nav = controller();
    nav.set_lifecycle_owner(h.clone());
    nav.set_view_model_store(ViewModelStore::new());
    nav.set_graph(detail_graph(&nav));
    nav
}

#[test]
fn entries_have_view_models_with_their_arguments_saved_and_cleared_with_them() {
    CLEARED.with(|c| c.set(0));
    let h = host(LifecycleState::Resumed);
    let nav = view_model_controller(&h);
    nav.navigate("detail/7");
    let entry = nav.current_back_stack_entry().expect("one");
    let vm = note_view_model(&entry);
    // The navigation arguments are the handle's defaults; the entry's own handle has them too.
    assert_eq!(vm.handle.get::<String>("id").as_deref(), Some("7"));
    assert_eq!(entry.saved_state_handle().get::<String>("id").as_deref(), Some("7"));
    assert!(Rc::ptr_eq(&vm, &note_view_model(&entry))); // the same while the entry is there
    assert_eq!(vm.handle.to_route::<DetailArgs>(TypeMap::default()), DetailArgs { id: "7".into() });
    vm.handle.set("note", 3i32);
    // Saved with the controller, restored with the entry: a new ViewModel, its handle's values back.
    let saved = nav.save_state().expect("a state");
    let restored = controller();
    restored.set_lifecycle_owner(h.clone());
    restored.set_view_model_store(ViewModelStore::new());
    restored.restore_state(Some(SavedState::from_saved_value(&saved.to_saved_value())));
    restored.set_graph(detail_graph(&restored));
    let back = restored.current_back_stack_entry().expect("one");
    assert_eq!(back.id(), entry.id());
    let again = note_view_model(&back);
    assert!(!Rc::ptr_eq(&again, &vm));
    assert_eq!(again.handle.get::<i32>("note"), Some(3));
    // Popped for good: its ViewModels cleared.
    assert_eq!(CLEARED.with(|c| c.get()), 0);
    nav.pop_back_stack();
    assert_eq!(CLEARED.with(|c| c.get()), 1);
    // Popped with its state saved: kept.
    let other = view_model_controller(&h);
    other.navigate("detail/1");
    note_view_model(&other.current_back_stack_entry().expect("one"));
    other.navigate_with("home", |o| {
        o.pop_up_to("home", |p| {
            p.set_inclusive(true);
            p.set_save_state(true);
        })
    });
    assert_eq!(CLEARED.with(|c| c.get()), 1);
}

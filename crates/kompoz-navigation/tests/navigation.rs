//! Navigation's core as Compose's behaves (navigation-common and navigation-runtime 2.10): routes and deep links
//! matched, arguments typed, the controller's back stack (nested graphs, popUpTo, launchSingleTop, saved and
//! restored stacks, deep links), entries' lifecycles, the controller's saved state, typed routes. The C++ Kompoz's
//! navigation tests (kompoz_tests.cpp) are whole in nav_host.rs, with the NavHost showing their screens.
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

#[derive(Route, Clone, PartialEq, Debug)]
struct HomeRoute;

/// A graph of a home, a list tab (a nested graph) and settings (the C++ tab test's).
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

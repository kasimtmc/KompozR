//! (stage 13 WP3) The NavHost in Kotlin's syntax, as a Compose user writes it: the C++ Kompoz's Navigator
//! (kompozable_tests.cpp `navigation_as_compose_writes_it`) and AppShellForms (`app_shell_as_compose_writes_it`,
//! whole but its desktop MenuBar — stage 15/16's, D7), and navigation-compose 2.10.2's NavigationSamples.kt, from
//! kt/nav_host.kt (kompoz_file!). The same behaviour crates/kompoz-navigation/tests/nav_host.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::platform::LocalInspectionMode;
use std::cell::RefCell;
use std::rc::Rc;

// ---- Rust twins of the Kotlin classes (stage 14, D6) ----------------------------------------------------------------

/// `@Serializable data class Detail(val id: Int, val tab: String? = null)`
#[derive(Route, KotlinProperties, Clone, PartialEq, Debug)]
pub struct Detail {
    pub id: i32,
    pub tab: Option<String>,
}

/// Kotlin's `Detail(id, tab = null)`.
#[named(type)]
pub fn Detail(id: i32, #[default(None)] tab: Option<String>) -> Detail {
    Detail { id, tab }
}

/// `@Serializable object Start`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct Start;

/// `@Serializable object Shell`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct Shell;

/// `@Serializable object AccountGraph`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct AccountGraph;

/// `@Serializable data class Member(val n: Int)`
#[derive(Route, KotlinProperties, Clone, PartialEq, Debug)]
pub struct Member {
    pub n: i32,
}

/// Kotlin's `Member(n)`.
pub fn Member(n: i32) -> Member {
    Member { n }
}

/// NavigationSamples' `@Serializable object Profile { val resourceId: Int = R.string.profile }`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct Profile;

/// NavigationSamples' `@Serializable object Scrollable { val resourceId: Int = R.string.scrollable }`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct Scrollable;

/// NavigationSamples' `@Serializable object Dialog { val resourceId: Int = R.string.dialog }`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct Dialog;

/// NavigationSamples' `@Serializable data class Dashboard(val userId: String? = "no value given") { companion object {
/// val resourceId: Int = R.string.dashboard } }`
#[derive(Route, KotlinProperties, Clone, PartialEq, Debug)]
pub struct Dashboard {
    #[route(default = Some("no value given".to_string()))]
    pub user_id: Option<String>,
}

/// Kotlin's `Dashboard(userId = "no value given")`.
#[named(type)]
pub fn Dashboard(#[default(Some("no value given".to_string()))] user_id: Option<String>) -> Dashboard {
    Dashboard { user_id }
}

/// NavigationSamples' `@Serializable object Nested`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct Nested;

/// NavigationSamples' `@Serializable data class NestedWithArg(val userId: String? = "default nested arg")`
#[derive(Route, KotlinProperties, Clone, PartialEq, Debug)]
pub struct NestedWithArg {
    #[route(default = Some("default nested arg".to_string()))]
    pub user_id: Option<String>,
}

/// java.util.UUID (what the samples' third-party type wraps): its text.
#[derive(Clone, PartialEq, Debug)]
pub struct UUID(String);

impl UUID {
    /// Kotlin's `UUID.randomUUID()`: a version-4 UUID (made from a counter and the time here).
    pub fn random_uuid() -> UUID {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(1);
        let n = N.fetch_add(1, Ordering::Relaxed);
        UUID(format!("00000000-0000-4000-8000-{n:012x}"))
    }

    /// Kotlin's `UUID.fromString(s)`.
    pub fn from_string(s: &str) -> UUID {
        UUID(s.to_string())
    }
}

impl std::fmt::Display for UUID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// NavigationSamples' `@Serializable class WrappedUUID(@Serializable(with = UUIDSerializer::class) val uuid: UUID)`:
/// a route value of the samples' own NavType (WrappedUUIDNavType through the typeMap).
#[derive(KotlinProperties, RouteValue, Clone, PartialEq, Debug)]
pub struct WrappedUUID {
    pub uuid: UUID,
}

/// Kotlin's `WrappedUUID(uuid)`.
pub fn WrappedUUID(uuid: UUID) -> WrappedUUID {
    WrappedUUID { uuid }
}

/// NavigationSamples' `@Serializable class User(val id: WrappedUUID)`
#[derive(Route, KotlinProperties, Clone, PartialEq, Debug)]
pub struct User {
    pub id: WrappedUUID,
}

/// Kotlin's `User(id)`.
pub fn User(id: WrappedUUID) -> User {
    User { id }
}

/// NavigationSamples' `object WrappedUUIDNavType : NavType<WrappedUUID>(isNullableAllowed = false)`: put/get as a
/// string, parsed from and written as the UUID's text (`Uri.encode`).
struct WrappedUUIDNavTypeImpl;

impl NavTypeImpl for WrappedUUIDNavTypeImpl {
    type Value = WrappedUUID;
    fn is_nullable_allowed(&self) -> bool {
        false
    }
    fn put(&self, bundle: &SavedState, key: &str, value: Option<&WrappedUUID>) {
        if let Some(v) = value {
            bundle.write(|w| w.put_string(key, v.uuid.to_string()));
        }
    }
    fn get(&self, bundle: &SavedState, key: &str) -> Option<WrappedUUID> {
        Some(WrappedUUID(UUID::from_string(&bundle.get_string(key)?)))
    }
    fn parse_value(&self, value: &str) -> Result<Option<WrappedUUID>, NavTypeError> {
        Ok(Some(WrappedUUID(UUID::from_string(value))))
    }
    fn serialize_as_value(&self, value: Option<&WrappedUUID>) -> String {
        value.map_or("null".into(), |v| v.uuid.to_string())
    }
}

/// The samples' `WrappedUUIDNavType` object.
pub const WrappedUUIDNavType: NavType = NavType::of_object(&WrappedUUIDNavTypeImpl);

// The samples' string resources (NavigationSamples' `R.string.*`): tests/navResources.
compose_resources!(dir = "tests/navResources");

kompoz_file!("tests/kt/nav_host.kt");

// ---- the tests ----------------------------------------------------------------------------------------------------

type Held = Rc<RefCell<Option<NavHostController>>>;

fn held() -> Held {
    Rc::new(RefCell::new(None))
}

fn nav_of(h: &Held) -> NavHostController {
    h.borrow().clone().expect("a controller")
}

/// C++ `navigation_as_compose_writes_it`.
fn navigation_as_compose_writes_it() {
    let h = held();
    let hc = h.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let hc = hc.clone();
        NavigatorApp(move |nav: NavHostController| *hc.borrow_mut() = Some(nav));
    }));
    rule.wait_for_idle(); // (the entry read above the NavHost arrives with its first composition)
    let nav = nav_of(&h);
    rule.on_node_with_text("Aç").assert_exists();
    rule.on_node_with_text("rota Start").assert_exists();
    rule.on_node_with_text("Aç").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("ayrıntı 7 b").assert_exists();
    rule.on_node_with_text("rota Detail/{id}?tab={tab}").assert_exists();
    rule.on_node_with_text("Geri").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Aç").assert_exists();
    nav.navigate(NavUri("kompoz://a/4"));
    rule.wait_for_idle();
    rule.on_node_with_text("a 4").assert_exists();
}

/// C++ `app_shell_as_compose_writes_it` (whole but its MenuBar: stage 15/16's).
fn app_shell_as_compose_writes_it() {
    let h = held();
    let hc = h.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let hc = hc.clone();
            AppShellApp(move |nav: NavHostController| *hc.borrow_mut() = Some(nav));
        }),
        IntSize::new(600, 800),
    );
    rule.wait_for_idle();
    let nav = nav_of(&h);
    rule.on_node_with_text("kabuk ekranı").assert_exists();
    rule.on_node_with_text("sekme kabuk").assert_exists();
    rule.on_node_with_text("eylem 7").assert_exists();
    rule.on_node_with_text("Hesap").perform_click();
    rule.wait_for_idle();
    rule.advance_time_by(1000);
    rule.wait_for_idle();
    rule.on_node_with_text("hesap 0").assert_exists();
    rule.on_node_with_text("sekme hesap").assert_exists();
    // (a rail: its items one under another at the start)
    let at = |t: &str| rule.on_node_with_text(t).get_unclipped_bounds_in_root();
    let (home, account) = (at("Kabuk"), at("Hesap"));
    assert!(account.top > home.top && home.left < 100.0);
    assert!(nav.handle_deep_link_uri("kompoz://member/5"));
    rule.wait_for_idle();
    rule.advance_time_by(1000);
    rule.wait_for_idle();
    rule.on_node_with_text("hesap 5").assert_exists();
    // The shell's tab again: the account's stack saved.
    rule.on_node_with_text("Kabuk").perform_click();
    rule.wait_for_idle();
    rule.advance_time_by(1000);
    rule.wait_for_idle();
    rule.on_node_with_text("kabuk ekranı").assert_exists();
    rule.on_node_with_text("sekme kabuk").assert_exists();
}

fn click(rule: &KompozTestRule, text: &str) {
    rule.on_node_with_text(text).perform_click();
    rule.wait_for_idle();
}

/// NavigationSamples' BasicNav: the profile's buttons to the dashboard, the scrollable list and the dialog; back.
fn basic_nav_sample() {
    let rule = KompozTestRule::new(BasicNav);
    rule.wait_for_idle();
    rule.on_node_with_text("Profile").assert_exists();
    assert!(!rule.on_node_with_text("Go to Previous screen").exists()); // nothing below it
    click(&rule, "Navigate to Dashboard");
    rule.on_node_with_text("Dashboard").assert_exists();
    click(&rule, "Go to Previous screen");
    rule.on_node_with_text("Profile").assert_exists();
    click(&rule, "Navigate to Scrollable");
    rule.on_node_with_text("Easy As Pie").assert_exists();
    click(&rule, "Navigate to Dashboard"); // (from the scrollable screen: Dashboard slides in)
    rule.on_node_with_text("Dashboard").assert_exists();
    click(&rule, "Go to Previous screen");
    click(&rule, "Go to Previous screen");
    // The dialog over the profile, which stays; its own back button pops it.
    click(&rule, "Navigate to Dialog");
    rule.on_node_with_text("Easy As Pie").assert_exists();
    rule.on_node_with_text("Profile").assert_exists();
    click(&rule, "Go to Previous screen");
    assert!(!rule.on_node_with_text("Easy As Pie").exists());
    // Back dismisses the dialog too.
    click(&rule, "Navigate to Dialog");
    assert!(rule.host().key_down(kompoz::ui::input::Key::Escape, kompoz::ui::input::KeyModifiers::default()));
    rule.wait_for_idle();
    assert!(!rule.on_node_with_text("Easy As Pie").exists());
}

/// NavigationSamples' NestedNavStartDestination, NestedNavInGraph and NavScaffold: a graph as the start destination,
/// a graph of a typed route with a start destination of its own, the NavHost in a scaffold.
fn nested_nav_samples() {
    let rule = KompozTestRule::new(NestedNavStartDestination);
    rule.wait_for_idle();
    rule.on_node_with_text("Profile").assert_exists();
    click(&rule, "Navigate to Dashboard");
    rule.on_node_with_text("Dashboard").assert_exists();
    let rule = KompozTestRule::new(NestedNavInGraph);
    rule.wait_for_idle();
    click(&rule, "Navigate to Dashboard"); // the graph: its start destination, Nested's screen
    rule.on_node_with_text("Dashboard").assert_exists();
    click(&rule, "Go to Previous screen");
    rule.on_node_with_text("Profile").assert_exists();
    let rule = KompozTestRule::new(NavScaffold);
    rule.wait_for_idle();
    click(&rule, "Navigate to Scrollable");
    rule.on_node_with_text("Easy As Pie").assert_exists();
}

/// NavigationSamples' NavWithArgsInNestedGraph: the graph's argument goes to its start destination's.
fn nav_with_args_in_nested_graph_sample() {
    let rule = KompozTestRule::new(NavWithArgsInNestedGraph);
    rule.wait_for_idle();
    rule.on_node(kompoz::test::has_set_text_action()).perform_text_input("kasım");
    rule.wait_for_idle();
    click(&rule, "Navigate to Dashboard with userId");
    rule.on_node_with_text("kasım").assert_exists();
}

/// NavigationSamples' NavHostPreview and ThirdPartySerializableType: a NavHost while inspecting; a typed route of a
/// type of the app's own with its NavType through the typeMap.
fn preview_and_third_party_type_samples() {
    let rule = KompozTestRule::new(NavHostPreview);
    rule.wait_for_idle();
    rule.on_node_with_tag("text").assert_exists();
    let rule = KompozTestRule::new(ThirdPartySerializableType);
    rule.wait_for_idle();
    named_args!(rule.on_node_with_text("uuid: 00000000-0000-4000-8000-", substring = true)).assert_exists();
}

fn main() {
    common::run(&[
        ("navigation_as_compose_writes_it", navigation_as_compose_writes_it),
        ("app_shell_as_compose_writes_it", app_shell_as_compose_writes_it),
        ("basic_nav_sample", basic_nav_sample),
        ("nested_nav_samples", nested_nav_samples),
        ("nav_with_args_in_nested_graph_sample", nav_with_args_in_nested_graph_sample),
        ("preview_and_third_party_type_samples", preview_and_third_party_type_samples),
    ]);
}

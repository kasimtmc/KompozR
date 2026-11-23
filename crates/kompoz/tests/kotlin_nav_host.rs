//! (stage 13 WP3) The NavHost in Kotlin's syntax, as a Compose user writes it: the C++ Kompoz's Navigator
//! (kompozable_tests.cpp `navigation_as_compose_writes_it`) and AppShellForms (`app_shell_as_compose_writes_it`,
//! whole but its desktop MenuBar — stage 15/16's, D7), and navigation-compose 2.10.2's NavigationSamples.kt,
//! SizeTransformSample.kt and SharedElementSample.kt, from kt/nav_host.kt (kompoz_file!). The same behaviour
//! crates/kompoz-navigation/tests/nav_host.rs checks in Rust.

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

/// SizeTransformSample's `@Serializable object Collapsed`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct Collapsed;

/// SizeTransformSample's `@Serializable object Expanded`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct Expanded;

/// SizeTransformSample's `@Serializable object NestedGraph`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct NestedGraph;

/// SizeTransformSample's `@Serializable object InnerCollapsed`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct InnerCollapsed;

/// SharedElementSample's `@Serializable object Select`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct Select;

/// SharedElementSample's `@Serializable object SharedElement`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct SharedElement;

/// SharedElementSample's `@Serializable object TopAppBarShared`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct TopAppBarShared;

/// SharedElementSample's `@Serializable object RedBox`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct RedBox;

/// SharedElementSample's `@Serializable object BlueBox`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct BlueBox;

/// SharedElementSample's `@Serializable object First`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct First;

/// SharedElementSample's `@Serializable object Second`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct Second;

/// SharedElementSample's `@Serializable object Third`
#[derive(Route, Clone, PartialEq, Debug)]
pub struct Third;

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

// ---- SizeTransformSample.kt ---------------------------------------------------------------------------------------

kompoz! {
    // The test's app: the sample in a Box tagged to measure (its size the NavHost's: the sample's own Box wraps it).
    @Composable
    fun SizeTransformMeasured() {
        Box(modifier = Modifier.testTag("host")) { SizeTransformComposable() }
    }
}

fn host_size(rule: &KompozTestRule) -> (f32, f32) {
    let b = rule.on_node_with_tag("host").get_unclipped_bounds_in_root();
    (b.width(), b.height())
}

/// The size `keyframes { durationMillis = 500; at150 at 150 }` gives `elapsed` ms in, from `from` to `to` (linear
/// between keyframes: keyframes' default easing).
fn keyframed(elapsed: i64, from: (f32, f32), at150: (f32, f32), to: (f32, f32)) -> (f32, f32) {
    let lerp = |a: f32, b: f32, f: f32| a + (b - a) * f;
    let (a, b, f) = if elapsed <= 150 {
        (from, at150, elapsed as f32 / 150.0)
    } else {
        (at150, to, ((elapsed - 150) as f32 / 350.0).min(1.0))
    };
    (lerp(a.0, b.0, f), lerp(a.1, b.1, f))
}

/// Clicks the screen shown (the one clickable box) and follows the NavHost's size frame by frame along the
/// destination's SizeTransform: `from` on the transition's first frame, the keyframe's size at 150 ms, `to` at 500.
fn follow_size_transform(rule: &KompozTestRule, from: (f32, f32), at150: (f32, f32), to: (f32, f32)) {
    rule.main_clock().set_auto_advance(false);
    rule.on_node(kompoz::test::has_click_action()).perform_click();
    rule.main_clock().advance_time_by_frame();
    let start = rule.main_clock().current_time();
    assert_eq!(host_size(rule), from);
    let mut by_keyframe = false;
    while rule.main_clock().current_time() - start < 528 {
        rule.main_clock().advance_time_by_frame();
        let elapsed = rule.main_clock().current_time() - start;
        let (w, h) = host_size(rule);
        let (ew, eh) = keyframed(elapsed, from, at150, to);
        // (within a few px: a frame's play time is the frame's, the keyframes' points whole ms)
        assert!((w - ew).abs() <= 4.0 && (h - eh).abs() <= 4.0, "{elapsed} ms: {w}x{h}, not {ew}x{eh}");
        by_keyframe |= (150..=160).contains(&elapsed);
    }
    assert!(by_keyframe);
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    assert_eq!(host_size(rule), to);
}

/// SizeTransformSample's SizeTransformComposable: Collapsed (40 dp) to Expanded (500 dp) along Expanded's
/// SizeTransform (its keyframe at 150 ms: the target's width, the initial height + 400), back along Collapsed's (the
/// initial width, the heights' mean).
fn size_transform_composable_sample() {
    let rule = KompozTestRule::with_viewport(SizeTransformMeasured, IntSize::new(800, 800));
    assert_eq!(host_size(&rule), (40.0, 40.0));
    follow_size_transform(&rule, (40.0, 40.0), (500.0, 440.0), (500.0, 500.0));
    assert_eq!(rule.on_all_nodes(kompoz::test::has_click_action()).fetch_semantics_nodes().len(), 1);
    follow_size_transform(&rule, (500.0, 500.0), (500.0, 270.0), (40.0, 40.0));
    assert_eq!(rule.on_all_nodes(kompoz::test::has_click_action()).fetch_semantics_nodes().len(), 1);
}

/// SizeTransformSample's SizeTransformNav: its NavHost's start destination (Collapsed) is no destination of its
/// graph (only NestedGraph is), so building the graph throws as Compose's does (NavGraph's `setStartDestination`:
/// IllegalStateException) — the sample shows `navigation<T>(sizeTransform = …)`'s form, it doesn't run.
fn size_transform_nav_sample() {
    let thrown = common::thrown(|| KompozTestRule::new(SizeTransformNav)).expect("SizeTransformNav throws");
    assert!(thrown.starts_with("Cannot find startDestination Collapsed from NavGraph."), "{thrown}");
}

// ---- SharedElementSample.kt ---------------------------------------------------------------------------------------

fn all_bounds(rule: &KompozTestRule, text: &str) -> Vec<Rect> {
    rule.on_all_nodes_with_text(text).fetch_semantics_nodes().iter().map(|n| n.bounds_in_root()).collect()
}

fn escape(rule: &KompozTestRule) -> bool {
    use kompoz::ui::input::{Key, KeyModifiers};
    let taken = rule.host().key_down(Key::Escape, KeyModifiers::default());
    rule.host().key_up(Key::Escape, KeyModifiers::default());
    rule.wait_for_idle();
    taken
}

/// SharedElementSample's SharedElementAnimationNav: the red box (100 dp at the top left) to the blue one (180 dp at
/// 180, 180) and back, their shared bounds between the two on the way (both boxes in them), each at its place after.
fn shared_element_animation_nav_sample() {
    let rule = KompozTestRule::new(SharedElementAnimationNav);
    let red = Rect::new(0.0, 0.0, 100.0, 100.0);
    let blue = Rect::new(180.0, 180.0, 360.0, 360.0);
    assert_eq!(all_bounds(&rule, "start"), vec![red]);
    for (from, to, from_bounds, to_bounds) in [("start", "finish", red, blue), ("finish", "start", blue, red)] {
        rule.main_clock().set_auto_advance(false);
        rule.on_node_with_text(from).perform_click();
        rule.main_clock().advance_time_by(96);
        let (shared, other) = (all_bounds(&rule, to), all_bounds(&rule, from));
        assert_eq!(shared.len(), 1);
        assert_eq!(shared, other); // (sharedBounds: both shown in the shared bounds)
        let b = shared[0];
        let between = |v: f32, a: f32, z: f32| v > a.min(z) && v < a.max(z);
        assert!(
            between(b.left, from_bounds.left, to_bounds.left)
                && between(b.top, from_bounds.top, to_bounds.top)
                && between(b.width(), from_bounds.width(), to_bounds.width()),
            "{b:?}"
        );
        rule.main_clock().set_auto_advance(true);
        rule.wait_for_idle();
        assert_eq!(all_bounds(&rule, to), vec![to_bounds]);
        assert!(all_bounds(&rule, from).is_empty());
    }
}

/// SharedElementSample's TopAppBarElement: First to Second, the shared top app bar staying put (its lookahead place
/// the same in both) while the rest slides in from the start (SlideDirection.Right) and out towards it (Left);
/// Second to Third, which has no app bar (Second's slides away with its screen); popBackStack<First>(false) back to
/// First (Third out towards the end, its popExitTransition), Second popped too.
fn top_app_bar_element_sample() {
    let rule = KompozTestRule::new(TopAppBarElement);
    let left = |t: &str| rule.on_node_with_text(t).get_bounds_in_root().left;
    // (the app bar's title: the upper of the two texts saying the screen's name)
    let app_bar_title = |t: &str| all_bounds(&rule, t).into_iter().min_by(|a, b| a.top.total_cmp(&b.top)).unwrap();
    let title_at = app_bar_title("first");
    rule.main_clock().set_auto_advance(false);
    rule.on_node_with_text("Navigate to Second").perform_click();
    rule.main_clock().advance_time_by(48);
    // (both titles where the app bar is: its shared bounds, which don't slide)
    assert_eq!(app_bar_title("second").top_left(), title_at.top_left());
    assert_eq!(app_bar_title("first"), title_at);
    assert!(left("Navigate to Third") < 0.0 && left("Navigate to Second") < 0.0);
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    assert_eq!(app_bar_title("second").left, title_at.left);
    assert_eq!(left("Navigate to Third"), 0.0);
    assert!(all_bounds(&rule, "first").is_empty());
    rule.main_clock().set_auto_advance(false);
    rule.on_node_with_text("Navigate to Third").perform_click();
    rule.main_clock().advance_time_by(48);
    assert!(app_bar_title("second").left < title_at.left); // (no match: it slides with its screen)
    assert!(left("Pop back to First") < 0.0);
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    rule.on_node_with_text("third").assert_exists();
    assert!(all_bounds(&rule, "second").is_empty());
    rule.main_clock().set_auto_advance(false);
    rule.on_node_with_text("Pop back to First").perform_click();
    rule.main_clock().advance_time_by(48);
    assert!(left("Pop back to First") > 0.0 && left("Navigate to Second") < 0.0);
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    assert_eq!(app_bar_title("first"), title_at);
    assert_eq!(left("Navigate to Second"), 0.0);
    assert!(all_bounds(&rule, "third").is_empty());
    assert!(!escape(&rule)); // (Second popped too: First alone on the stack)
}

/// SharedElementSample's AnimatedNav: its two rows open the two samples (in a NavHost of their own), back returns.
fn animated_nav_sample() {
    let rule = KompozTestRule::new(AnimatedNav);
    click(&rule, "AnimationNav");
    rule.on_node_with_text("start").assert_exists();
    click(&rule, "start");
    rule.on_node_with_text("finish").assert_exists();
    assert!(escape(&rule)); // (the inner NavHost's back: the red box again)
    rule.on_node_with_text("start").assert_exists();
    assert!(escape(&rule)); // (the outer's: the list)
    rule.on_node_with_text("Top Bar Shared Element").assert_exists();
    assert!(all_bounds(&rule, "start").is_empty());
    click(&rule, "Top Bar Shared Element");
    rule.on_node_with_text("Navigate to Second").assert_exists();
    assert!(escape(&rule));
    rule.on_node_with_text("AnimationNav").assert_exists();
}

fn main() {
    common::run(&[
        ("navigation_as_compose_writes_it", navigation_as_compose_writes_it),
        ("app_shell_as_compose_writes_it", app_shell_as_compose_writes_it),
        ("basic_nav_sample", basic_nav_sample),
        ("nested_nav_samples", nested_nav_samples),
        ("nav_with_args_in_nested_graph_sample", nav_with_args_in_nested_graph_sample),
        ("preview_and_third_party_type_samples", preview_and_third_party_type_samples),
        ("size_transform_composable_sample", size_transform_composable_sample),
        ("size_transform_nav_sample", size_transform_nav_sample),
        ("shared_element_animation_nav_sample", shared_element_animation_nav_sample),
        ("top_app_bar_element_sample", top_app_bar_element_sample),
        ("animated_nav_sample", animated_nav_sample),
    ]);
}

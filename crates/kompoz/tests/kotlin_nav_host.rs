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
/// type of the app's own with its NavType through the typeMap (a random UUID's text; the NavType's object writing and
/// reading it).
fn preview_and_third_party_type_samples() {
    let rule = KompozTestRule::new(NavHostPreview);
    rule.wait_for_idle();
    rule.on_node_with_tag("text").assert_exists();
    let rule = KompozTestRule::new(ThirdPartySerializableType);
    rule.wait_for_idle();
    named_args!(rule.on_node_with_text("uuid: ", substring = true)).assert_exists();
    let text = "0f0e0d0c-0b0a-4908-8706-050403020100";
    let t: NavType = WrappedUUIDNavType {}.into();
    let parsed: WrappedUUID = t.parse_value(text).unwrap().unwrap();
    assert_eq!(parsed.uuid().to_string(), text);
    assert_eq!(t.serialize_as_value(Some(&parsed)), text);
    let bundle = SavedState::new();
    t.put(&bundle, "id", Some(&parsed));
    // (a plain class: another object of the same UUID)
    assert_eq!(t.get::<WrappedUUID>(&bundle, "id").map(|w| w.uuid()), Some(parsed.uuid()));
    // (`@Serializable(with = UUIDSerializer::class) val uuid`: saved as the object UUIDSerializer writes it, its text)
    let user = User(parsed.clone());
    let saved = Saveable::save(&user);
    assert_eq!(saved.get("id").get("uuid").as_str(), Some(text));
    let back = <User as Saveable>::restore(&saved).expect("a user");
    assert_eq!(back.id().uuid(), parsed.uuid());
    assert_eq!(UUIDSerializer {}.descriptor().serial_name, "UUID");
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

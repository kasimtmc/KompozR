//! (stage 13 WP4, stage 14 WPD) Navigation 3 in Kotlin's syntax, as a Compose app writes it: navigation3-ui's and
//! navigation3-runtime's samples, their classes Kotlin's (kt/navigation3.kt and kt/navigation3_runtime.kt,
//! kompoz_file!) — NavDisplay with a remembered back stack and the entry provider DSL, the dialog scene strategy, a
//! hoisted scene state with its navigation back handler, scenes and strategies of their own giving transitions, back
//! stacks switched and concatenated, entries with their ViewModels, back stacks saved by kotlinx's serializers (open
//! and closed polymorphism, a serializers module, Android's reflection), deep links matched to keys and back stacks,
//! (stage 14 LISTS) the runtime samples' CommonUiNavDisplay (kt/navigation3_common_ui.kt) popping its caller's list.
//! crates/kompoz-navigation3/tests/navigation3.rs and deeplink.rs check the same in Rust's syntax.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::navigation3::runtime::deeplink::*;
use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, has_text};
use kompoz::ui::input::KeyModifiers;
use kompoz::ui::layout::SlotKey;
use kompoz::ui::window::{Dialog, DialogProperties};
use std::rc::Rc;

// The samples' string resources (NavigationSamples.kt's `R.string.*`): tests/navResources.
compose_resources!(dir = "tests/navResources");

kompoz_file!("tests/kt/navigation3.kt");

/// kt/navigation3_runtime.kt: the runtime samples (their package's own names: `HomeKey`, `Product`, … are the ui
/// samples' too).
mod runtime_samples {
    #![allow(non_snake_case, non_upper_case_globals, unused_variables)]
    use kompoz::navigation3::runtime::deeplink::*;
    use kompoz::prelude::*;
    kompoz_file!("tests/kt/navigation3_runtime.kt");
}

/// kt/navigation3_common_ui.kt: CommonUiNavDisplaySample.kt (its object and composable of one name, its
/// `TopLevelRoute`), and the test's caller.
mod common_ui {
    #![allow(non_snake_case, non_upper_case_globals)]
    use kompoz::prelude::*;
    kompoz_file!("tests/kt/navigation3_common_ui.kt");
}

// ---- runtime samples --------------------------------------------------------------------------------------------

/// The back stack the composable `$sample` gives (shown), `$change`d; then the process ended (the host's state saved,
/// a new rule restoring it): the back stack it gives then.
macro_rules! saved_and_restored {
    ($sample:ident, $change:expr) => {{
        let held: Rc<std::cell::RefCell<Option<_>>> = Rc::default();
        let show = mutable_state_of(false);
        let (h, s) = (held.clone(), show.clone());
        let ui = kompozable_lambda!(move || {
            if s.get() {
                let stack = $sample();
                *h.borrow_mut() = Some(stack.clone());
                BasicText(format!("{:?}", stack.to_list()));
            }
        });
        restore_after(held, show, ui, $change)
    }};
}

fn restore_after<T: Clone + 'static>(
    held: Rc<std::cell::RefCell<Option<NavBackStack<T>>>>,
    show: MutableState<bool>,
    ui: impl Content + Clone + 'static,
    change: impl FnOnce(&NavBackStack<T>),
) -> (NavBackStack<T>, NavBackStack<T>) {
    let rule = KompozTestRule::new(ui.clone());
    show.set(true);
    rule.wait_for_idle();
    let before = held.borrow_mut().take().expect("composed");
    change(&before);
    rule.wait_for_idle();
    let bytes = rule.host().save_state();
    drop(rule);
    show.set(false);
    let rule = KompozTestRule::new(ui);
    rule.host().restore_state(&bytes);
    show.set(true);
    rule.wait_for_idle();
    let after = held.borrow_mut().take().expect("composed again");
    (before, after)
}

/// NavBackStackSamples.kt: a back stack of NavKeys remembered by kotlinx's serializer with its subclasses registered
/// (open polymorphism), one of a sealed class's keys (closed): both back after the process ended.
fn nav_back_stack_samples_as_compose_writes_them() {
    use runtime_samples::*;
    let (before, after) = saved_and_restored!(NavBackStack_OpenPolymorphism, |s: &NavBackStack<_>| {
        s.add(NavBackStackSamplesHome());
        s.add(NavBackStackSamplesChat());
        s.add(NavBackStackSamplesSpaces());
    });
    assert_eq!(before.size(), 3);
    let keys = after.to_list();
    assert_eq!(keys.len(), 3);
    assert!(keys[0].is::<NavBackStackSamplesHome>() && keys[1].is::<NavBackStackSamplesChat>());
    assert!(keys[2].is::<NavBackStackSamplesSpaces>());
    let (_, after) = saved_and_restored!(NavBackStack_ClosedPolymorphism, |s: &NavBackStack<_>| {
        s.add(NavBackStackSamplesSealedKey::from(NavBackStackSamplesSealedKeyInbox()));
        s.add(NavBackStackSamplesSealedKey::from(NavBackStackSamplesSealedKeySettings()));
    });
    let keys = after.to_list();
    assert!(matches!(
        keys.as_slice(),
        [NavBackStackSamplesSealedKey::Inbox(_), NavBackStackSamplesSealedKey::Settings(_)]
    ));
}

/// NavBackStackSerializerSamples.kt: a back stack encoded and decoded (by Android's reflection — KompozR's classes of
/// the keys seen —, by a serializers module's subclasses); RememberNavBackStackSamples.kt: remembered back stacks of
/// such keys, back after the process ended.
fn nav_back_stack_serializer_samples_as_compose_writes_them() {
    use runtime_samples::*;
    let decoded: Rc<std::cell::RefCell<Vec<NavBackStack<Rc<dyn NavKey>>>>> = Rc::default();
    let d = decoded.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let a = NavBackStackSerializer_withReflection();
        let b = NavBackStackSerializer_withSerializersModule();
        d.borrow_mut().extend([a, b]);
    }));
    for stack in decoded.borrow().iter().take(2) {
        let keys = stack.to_list();
        assert_eq!(keys[0].downcast_ref::<NavBackStackSerializerSamplesHome>().map(|h| h.id()), Some("abc".into()));
        assert_eq!(keys[1].downcast_ref::<NavBackStackSerializerSamplesDetails>().map(|d| d.item_id()), Some(42));
    }
    let (_, after) = saved_and_restored!(rememberNavBackStack_withReflection, |s: &NavBackStack<_>| {
        s.add(RememberNavBackStackSamplesDetails(7));
    });
    let keys = after.to_list();
    assert_eq!(keys.len(), 3);
    assert_eq!(keys[0].downcast_ref::<RememberNavBackStackSamplesHome>().map(|h| h.id()), Some("start".into()));
    assert_eq!(keys[2].downcast_ref::<RememberNavBackStackSamplesDetails>().map(|d| d.item_id()), Some(7));
    let (_, after) = saved_and_restored!(rememberNavBackStack_withSerializersModule, |s: &NavBackStack<_>| {
        s.add(RememberNavBackStackSamplesDetails(8));
    });
    let keys = after.to_list();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[1].downcast_ref::<RememberNavBackStackSamplesDetails>().map(|d| d.item_id()), Some(8));
}

/// DeepLinkMatcherSample.kt and DeepLinkNonPrimitiveArgSample.kt: the keys and back stacks their matchers make.
fn deep_link_samples_as_compose_writes_them() {
    use kompoz::navigation3::runtime::deeplink::MatchResultOf;
    use runtime_samples::*;
    assert_eq!(static_key_deep_link_matcher_sample().map(|r| *r.key()), Some(ImageKey {}));
    // (the sample's request has a trailing '/' its pattern hasn't: as in Kotlin, no match)
    assert!(deep_link_matcher_with_back_stack_sample("https://www.nav3deeplinksample.com/userlist/").is_none());
    let result = deep_link_matcher_with_back_stack_sample("https://www.nav3deeplinksample.com/userlist").unwrap();
    // (Kotlin's objects written as theirs are, `HomeKey@1`)
    let keys = result.back_stack();
    assert!(keys.len() == 2 && keys[0].is::<HomeKey>() && keys[1].is::<UserListKey>());
    let at = |n: i32| {
        let uri = format!("https://www.nav3deeplinksample.com/articles/{n}");
        format!("{:?}", deep_link_matcher_with_conditional_back_stack_sample(&uri).map(|r| r.back_stack().to_vec()))
    };
    assert_eq!(at(12), "Some([ArticleSection(category=Shipping), Article(id=12)])");
    assert_eq!(at(3), "Some([ArticleSection(category=Returns), Article(id=3)])");
    let key =
        |q: &str| format!("{:?}", deep_link_serializer_sample(format!("http://www.nav3example.com/products?{q}")));
    assert_eq!(
        key("product=123-productA&count=12"),
        "ProductInventoryKey(product=Product(id=123, name=productA), count=12)"
    );
    // (a required argument missing: HomeKey; the serializer's own exception thrown)
    assert!(key("product=123-productA").starts_with("HomeKey@"));
    assert_eq!(
        common::thrown(|| key("product=abc-x&count=1")).as_deref(),
        Some("SerializationException: Invalid product id: abc")
    );
}

// ---- ui samples -------------------------------------------------------------------------------------------------

fn escape(rule: &KompozTestRule) -> bool {
    let taken = rule.host().key_down(Key::Escape, KeyModifiers::None);
    rule.host().key_up(Key::Escape, KeyModifiers::None);
    rule.wait_for_idle();
    taken
}

/// Whether `text` is drawn: a text of its own, or lines of one wrapped (`Overlai` `d Entry` of the samples' 50 dp
/// boxes; broken at a space, the space left out).
fn shown(rule: &KompozTestRule, text: &str) -> bool {
    let lines = rule.drawn_texts();
    (0..lines.len()).any(|i| {
        let mut joined = String::new();
        lines[i..].iter().any(|line| {
            if !joined.is_empty() && !text.get(joined.len()..).is_some_and(|rest| rest.starts_with(line.as_str())) {
                joined.push(' ');
            }
            joined.push_str(line);
            joined == text
        })
    })
}

/// Clicks the node of the text `text`.
fn click(rule: &KompozTestRule, text: &str) {
    rule.on_node_with(has_text(text, false, false), false).perform_click();
    rule.wait_for_idle();
}

/// OverlaySamples.kt's DialogSample: the dialog entry over the overlaid one, back closing it.
fn dialog_sample_as_compose_writes_it() {
    let rule = KompozTestRule::new(DialogSample);
    assert!(shown(&rule, "Overlaid Entry") && !shown(&rule, "Dialog"));
    click(&rule, "Open Dialog");
    assert!(shown(&rule, "Overlaid Entry") && shown(&rule, "Dialog"));
    assert!(escape(&rule));
    assert!(!shown(&rule, "Dialog"));
}

/// SceneStateSamples.kt's SceneStateSample: back pops through the hoisted handler.
fn scene_state_sample_as_compose_writes_it() {
    let rule = KompozTestRule::new(SceneStateSample);
    assert!(shown(&rule, "Key = b"));
    assert!(escape(&rule));
    assert!(shown(&rule, "Key = a") && !shown(&rule, "Key = b"));
    assert!(!escape(&rule));
}

/// CustomSceneSamples.kt's SceneDefaultTransitionsSample: the scene's horizontal slides, not NavDisplay's vertical
/// ones (both 5 s: mid-way both contents shown).
fn scene_default_transitions_sample_as_compose_writes_it() {
    let rule = KompozTestRule::new(SceneDefaultTransitionsSample);
    assert!(shown(&rule, "A"));
    rule.main_clock().set_auto_advance(false);
    rule.on_node_with(has_text("Click to navigate", false, false), false).perform_click();
    rule.advance_time_by(2500);
    assert!(shown(&rule, "A") && shown(&rule, "B"));
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    assert!(shown(&rule, "B") && !shown(&rule, "A"));
    assert!(escape(&rule));
    assert!(shown(&rule, "A") && !shown(&rule, "B"));
}

/// CustomSceneSamples.kt's SceneOverrideEntryTransitionsSample (a generic data class of the crate implementing the
/// API's `Scene<T>`, `super.metadata` its default, the strategy a generic class of no property of its `T`): the
/// scene's horizontal slide over the entry's vertical one (5 s), B mid-way to the right of where it ends, not above.
fn scene_override_entry_transitions_sample_as_compose_writes_it() {
    let rule = KompozTestRule::new(SceneOverrideEntryTransitionsSample);
    assert!(shown(&rule, "A"));
    rule.main_clock().set_auto_advance(false);
    rule.on_node_with(has_text("Click to navigate", false, false), false).perform_click();
    rule.advance_time_by(2500);
    assert!(shown(&rule, "A") && shown(&rule, "B"));
    let mid = rule.on_node_with_text("B").position_in_root();
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    let end = rule.on_node_with_text("B").position_in_root();
    assert!(mid.x > end.x + 1.0 && (mid.y - end.y).abs() < 1.0, "mid {mid:?}, end {end:?}");
    assert!(shown(&rule, "B") && !shown(&rule, "A"));
}

/// SceneStrategySamples.kt's SceneStrategyOnBackSample (its local generic classes: an `OverlayScene<T>` with
/// `equals`/`hashCode` of its own, its strategy): a dialog scene for every entry above the first, its dialog's
/// dismissal the scope's onBack (the display's).
fn scene_strategy_on_back_sample_as_compose_writes_it() {
    let stack = NavBackStack(vec!["ana".to_string(), "pencere".to_string()]);
    let s = stack.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let strategy: Rc<dyn SceneStrategy<String>> = Rc::new(SceneStrategyOnBackSampleSimpleDialogSceneStrategy());
        NavDisplay(
            s.clone(),
            scene_strategies = vec![strategy],
            entry_provider = |k: String| NavEntry(k, content = |k: String| BasicText(k)),
        );
    }));
    assert!(shown(&rule, "ana") && shown(&rule, "pencere"));
    // (the scenes of one entry and its previous ones equal, their hashes too; another's not)
    let entry = |k: &str| NavEntry::new(k.to_string(), SlotKey::new(k.to_string()), Metadata::new(), |_| {});
    let scene = |e: &NavEntry<String>, p: Vec<NavEntry<String>>| {
        SceneStrategyOnBackSampleSimpleDialogScene(e.clone(), p, || {})
    };
    let (a, b) = (entry("a"), entry("b"));
    let (x, y, z) = (scene(&b, vec![a.clone()]), scene(&b, vec![a.clone()]), scene(&a, vec![]));
    assert!(x == y && x.hash_code_kt() == y.hash_code_kt() && x != z && x == x.clone());
    // (the dialog's own back: its dismissal, the scope's onBack, the display's)
    assert!(escape(&rule));
    assert_eq!(stack.size(), 1);
}

/// ComplexBackStackSamples.kt's MultipleBackStackSample and ConcatenatedBackStackSample (the crate's interfaces of
/// `NavKey` — `(it as SampleKey).name` of a back stack's keys, `listOf(Home, User)` of their `SampleTabKey`): a back
/// stack per tab, switched with their decorators' states, or concatenated.
fn complex_back_stack_samples_as_compose_writes_them() {
    use kompoz::test::is_selectable;
    let tab = |rule: &KompozTestRule, k: usize| {
        rule.on_all_nodes(is_selectable()).get(k).perform_click();
        rule.wait_for_idle();
    };
    let rule = KompozTestRule::new(MultipleBackStackSample);
    assert!(shown(&rule, "Home Page") && shown(&rule, "current backStack:[Home]"));
    click(&rule, "Navigate to Detail");
    assert!(shown(&rule, "Detail Page in Home Tab") && shown(&rule, "current backStack:[Home, Detail]"));
    tab(&rule, 1);
    assert!(shown(&rule, "User Page") && shown(&rule, "current backStack:[User]"));
    tab(&rule, 0);
    assert!(shown(&rule, "Detail Page in Home Tab"));
    assert!(escape(&rule));
    assert!(shown(&rule, "Home Page"));
    drop(rule);
    let rule = KompozTestRule::new(ConcatenatedBackStackSample);
    tab(&rule, 1);
    assert!(shown(&rule, "User Page") && shown(&rule, "current backStack:[Home, User]"));
    click(&rule, "Navigate to Detail");
    assert!(shown(&rule, "Detail Page in User Tab") && shown(&rule, "current backStack:[Home, User, Detail]"));
    // (back to the user's tab's first page, then out of it: the home tab's)
    assert!(escape(&rule));
    assert!(shown(&rule, "User Page"));
    assert!(escape(&rule));
    assert!(shown(&rule, "Home Page"));
}

/// AnimatedOverlaySamples.kt's AnimatedBottomSheetSample (its `object : OverlayScene<…>`'s `lateinit var sheetState`
/// set by its content, read by its suspending `onRemove`: the object's one variable): the entry in a modal bottom
/// sheet, its pop hiding the sheet (the scene's `onRemove`) before it leaves.
fn animated_bottom_sheet_sample_as_compose_writes_it() {
    let rule = KompozTestRule::new(AnimatedBottomSheetSample);
    assert!(shown(&rule, "Click to open bottom sheet"));
    click(&rule, "Click to open bottom sheet");
    assert!(shown(&rule, "BottomSheet") && shown(&rule, "Close BottomSheet"));
    rule.main_clock().set_auto_advance(false);
    rule.on_node_with(has_text("Close BottomSheet", false, false), false).perform_click();
    rule.advance_time_by(16);
    // (popped: the sheet still shown while `onRemove` hides it)
    assert!(shown(&rule, "BottomSheet"));
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    assert!(!shown(&rule, "BottomSheet") && shown(&rule, "Click to open bottom sheet"));
}

/// BasicSamples.kt's SceneNav (NavigationSamples.kt's screens): Profile's ViewModel scoped to its entry; Dashboard
/// navigated to and back (its predictive pop by the swipe's edge), Scrollable's list, DialogBase's dialog.
fn scene_nav_as_compose_writes_it() {
    let rule = KompozTestRule::new(SceneNav);
    assert!(shown(&rule, "no user Profile"));
    click(&rule, "Navigate to Dashboard");
    assert!(shown(&rule, "no value given"));
    let host = rule.host();
    let step = |progress: f32| kompoz::ui::back::BackEventCompat {
        progress,
        swipe_edge: kompoz::ui::back::SwipeEdge::Right,
        ..Default::default()
    };
    host.back_started(step(0.0));
    host.back_progressed(step(0.5));
    rule.wait_for_idle();
    assert!(shown(&rule, "no user Profile") && shown(&rule, "no value given"));
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert!(shown(&rule, "no user Profile") && !shown(&rule, "no value given"));
    click(&rule, "Navigate to Scrollable");
    assert!(shown(&rule, "Easy As Pie"));
    click(&rule, "Go to Previous screen");
    click(&rule, "Navigate to Dialog");
    click(&rule, "Show Dialog");
    assert!(shown(&rule, "Easy As Pie"));
    assert!(escape(&rule));
    assert!(!shown(&rule, "Easy As Pie") && shown(&rule, "Dialog"));
}

/// DeepLinkNonPrimitiveArgSamples.kt: the start key a deep link's arguments make (a class of flattened arguments, a
/// serializer's text, a list of them) shown (their failures, HomeKey's: kt/navigation3_runtime.kt's samples).
fn deep_link_ui_samples_as_compose_writes_them() {
    let rule = KompozTestRule::new(NonPrimitiveArgFlattenedSample);
    assert!(shown(&rule, "ProfileKey(name=Name(firstName=john, lastName=doe), age=25)"));
    drop(rule);
    let rule = KompozTestRule::new(DeepLinkSerializerSample);
    assert!(shown(&rule, "ProductInventoryKey(product=Product(id=123, name=productA), count=12)"));
    drop(rule);
    let rule = KompozTestRule::new(ListTypeDeepLinkSerializerSample);
    assert!(shown(&rule, "HatListKey(hats=[Hat(id=2342, weight=4.5), Hat(id=2643, weight=5.6)])"));
}

/// CommonUiNavDisplaySample.kt's CommonUiNavDisplay, given the caller's state list (a `MutableList`): a navigation
/// fades the new entry in (700 ms: mid-way both shown), the display's default back — `onBack: () -> Unit = { if
/// (backstack is MutableList) backstack.removeAt(backstack.size - 1) }`, a default reading the earlier parameter —
/// pops the caller's list itself, and the back handler is off with one entry left.
fn common_ui_nav_display_sample_as_compose_writes_it() {
    let rule = KompozTestRule::new(common_ui::CommonUiNavDisplayDemo);
    assert!(shown(&rule, "ekran Ana") && shown(&rule, "yığın Ana"));
    rule.main_clock().set_auto_advance(false);
    rule.on_node_with(has_text("git", false, false), false).perform_click();
    rule.advance_time_by(350);
    assert!(shown(&rule, "ekran Ana") && shown(&rule, "ekran Ayrıntı") && shown(&rule, "yığın Ana/Ayrıntı"));
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    assert!(shown(&rule, "ekran Ayrıntı") && !shown(&rule, "ekran Ana"));
    assert!(escape(&rule));
    assert!(shown(&rule, "yığın Ana") && shown(&rule, "ekran Ana") && !shown(&rule, "ekran Ayrıntı"));
    assert!(!escape(&rule));
    assert!(shown(&rule, "yığın Ana"));
}

fn main() {
    common::run(&[
        ("common_ui_nav_display_sample_as_compose_writes_it", common_ui_nav_display_sample_as_compose_writes_it),
        ("dialog_sample_as_compose_writes_it", dialog_sample_as_compose_writes_it),
        ("scene_state_sample_as_compose_writes_it", scene_state_sample_as_compose_writes_it),
        (
            "scene_default_transitions_sample_as_compose_writes_it",
            scene_default_transitions_sample_as_compose_writes_it,
        ),
        (
            "scene_override_entry_transitions_sample_as_compose_writes_it",
            scene_override_entry_transitions_sample_as_compose_writes_it,
        ),
        ("scene_strategy_on_back_sample_as_compose_writes_it", scene_strategy_on_back_sample_as_compose_writes_it),
        ("complex_back_stack_samples_as_compose_writes_them", complex_back_stack_samples_as_compose_writes_them),
        ("animated_bottom_sheet_sample_as_compose_writes_it", animated_bottom_sheet_sample_as_compose_writes_it),
        ("scene_nav_as_compose_writes_it", scene_nav_as_compose_writes_it),
        ("deep_link_ui_samples_as_compose_writes_them", deep_link_ui_samples_as_compose_writes_them),
        ("nav_back_stack_samples_as_compose_writes_them", nav_back_stack_samples_as_compose_writes_them),
        (
            "nav_back_stack_serializer_samples_as_compose_writes_them",
            nav_back_stack_serializer_samples_as_compose_writes_them,
        ),
        ("deep_link_samples_as_compose_writes_them", deep_link_samples_as_compose_writes_them),
    ]);
}

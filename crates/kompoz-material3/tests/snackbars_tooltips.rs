//! Material's snackbars and plain tooltips as Compose's material3 has them: the C++ Kompoz's snackbar and tooltip
//! tests by name (kompoz_tests.cpp; M15's values where the C++ differed, said where), Compose's behaviour (the
//! host's queue, durations, results and cancellation; the layouts; the tooltip box's transition, caret, state and
//! position providers) and M15's snackbar samples (ScaffoldSamples.kt's, over a Scaffold) and
//! TooltipSamples.kt's plain tooltips.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::{MutatePriority, MutatorMutex};
use kompoz_coroutines::{Channel, CoroutineScope, LaunchedEffect};
use kompoz_foundation::interaction::{HoverInteraction, Interaction};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_text::{TextOverflow, TextStyle};
use kompoz_ui::input::PointerType;
use kompoz_ui::platform::{
    AccessibilityManager, LocalAccessibilityManager, LocalLayoutDirection, LocalViewConfiguration, ViewConfiguration,
};
use kompoz_ui::semantics::{LiveRegionMode, SemanticsActions, SemanticsProperties};
use kompoz_ui::window::PopupPositionProvider;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

tests!(
    // snackbars
    snackbar_queue_action_and_timeout,
    snackbar_results_come_back_to_the_coroutine,
    snackbar_hosts_hand_their_data_to_a_snackbar_of_your_own,
    snackbars_lay_out_as_compose_and_host_your_own,
    snackbars_with_the_styling_fix_lay_out_as_its_layouts,
    a_cancelled_caller_takes_its_snackbar_off_the_queue,
    snackbar_durations_follow_the_accessibility_manager,
    the_host_fades_and_scales_snackbars_and_tells_accessibility,
    snackbar_defaults_are_the_tokens,
    scaffold_with_simple_snackbar_sample,
    scaffold_with_indefinite_snackbar_sample,
    scaffold_with_custom_snackbar_sample,
    scaffold_with_coroutines_snackbar_sample,
    scaffold_with_multiline_snackbar_sample,
    // tooltips
    tooltip_box_goes_on_the_side_its_position_says,
    tooltip_box_shows_on_long_press_and_hover,
    tooltip_caret_points_at_the_anchor,
    a_lifted_finger_hovers_nothing,
    view_configuration_sets_the_gesture_thresholds,
    tooltip_states_show_for_a_while_or_until_dismissed,
    the_tooltip_box_grows_and_fades_its_tooltip,
    plain_tooltips_are_sized_padded_and_styled_as_the_tokens,
    tooltip_defaults_and_caret_shapes,
    plain_tooltip_samples,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// The bounds of a node of `tag` (popups' too), px (the rule's density is 1).
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with_text(text).get_unclipped_bounds_in_root()
}

/// Where the first line of `text` was drawn: its start and baseline.
fn baseline_of(canvas: &RecordingCanvas, text: &str) -> Offset {
    let i = canvas.texts.iter().position(|t| text.starts_with(t.as_str()) && !t.is_empty());
    canvas.baselines[i.unwrap_or_else(|| panic!("{text:?} not drawn: {:?}", canvas.texts))]
}

/// A coroutine scope of the rule's host (what `rememberCoroutineScope()` gives a composable).
fn scope_of(rule: &KompozTestRule) -> CoroutineScope {
    CoroutineScope::new(rule.host().dispatcher())
}

/// A slot (`action = { … }`) for a nullable composable parameter.
fn slot(content: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(content))
}

/// Launches a snackbar's show, its result dropped (Kotlin's `scope.launch { state.showSnackbar(…) }`).
fn launch_show(scope: &CoroutineScope, shown: impl std::future::Future<Output = SnackbarResult> + 'static) {
    scope.launch(async move {
        shown.await;
    });
}

/// A mouse over `at` (moving there, no button).
fn mouse_to(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_move(at, -1, MOUSE);
}

/// The mouse's pointer id.
const MOUSE: i64 = 0;

type Results = Rc<RefCell<Vec<SnackbarResult>>>;

/// Shows a snackbar from `scope`, its result pushed to `results`.
fn show(
    scope: &CoroutineScope,
    state: &SnackbarHostState,
    results: &Results,
    message: &str,
    action_label: Option<&str>,
    duration: Option<SnackbarDuration>,
    with_dismiss_action: bool,
) {
    let shown = named_args!(state.show_snackbar(
        message.to_string(),
        action_label = action_label.map(str::to_string),
        with_dismiss_action = with_dismiss_action,
        duration = duration,
    ));
    let results = results.clone();
    scope.launch(async move {
        let result = shown.await;
        results.borrow_mut().push(result);
    });
}

// ==== snackbars ======================================================================================================

fn snackbar_queue_action_and_timeout() {
    // C++'s: its non-suspending showSnackbar(message, actionLabel, onAction) queued and called back; here Compose's
    // suspending showSnackbar from coroutines, the action's result coming back to the first's.
    let state = SnackbarHostState::new();
    let s = state.clone();
    let rule =
        KompozTestRule::with_viewport(kompozable_lambda!(move || SnackbarHost(s.clone())), IntSize::new(400, 800));
    let (scope, results) = (scope_of(&rule), Results::default());
    show(&scope, &state, &results, "birinci", Some("Geri al"), None, false);
    show(&scope, &state, &results, "ikinci", None, None, false);
    rule.wait_for_idle();
    rule.on_node_with_text("birinci").assert_exists();
    rule.on_node_with_text("ikinci").assert_does_not_exist();
    rule.on_node_with_text("Geri al").perform_click();
    rule.wait_for_idle();
    assert_eq!(*results.borrow(), [SnackbarResult::ActionPerformed]);
    rule.on_node_with_text("ikinci").assert_exists(); // the queued one follows
    rule.advance_time_by(3000);
    rule.on_node_with_text("ikinci").assert_exists();
    rule.advance_time_by(1200); // short snackbars last 4 s...
    rule.wait_for_idle(); // ...then fade out
    rule.on_node_with_text("ikinci").assert_does_not_exist();
    assert!(state.current_snackbar_data().is_none());
    assert_eq!(*results.borrow(), [SnackbarResult::ActionPerformed, SnackbarResult::Dismissed]);
}

fn snackbar_results_come_back_to_the_coroutine() {
    let state = SnackbarHostState::new();
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || SnackbarHost(s.clone(), modifier = Modifier)),
        IntSize::new(400, 800),
    );
    let (scope, results) = (scope_of(&rule), Results::default());
    let (st, r) = (state.clone(), results.clone());
    scope.launch(async move {
        // with an action: until answered
        let first = named_args!(st.show_snackbar("Silindi".to_string(), action_label = Some("Geri al".to_string())));
        let result = first.await;
        r.borrow_mut().push(result);
        // without: Short
        let second = st.show_snackbar("Kısa".to_string());
        let result = second.await;
        r.borrow_mut().push(result);
        let third = named_args!(st.show_snackbar(
            "Kapatılabilir".to_string(),
            duration = Some(SnackbarDuration::Indefinite),
            with_dismiss_action = true,
        ));
        let result = third.await;
        r.borrow_mut().push(result);
    });
    use SnackbarResult as R;
    rule.wait_for_idle();
    rule.on_node_with_text("Silindi").assert_exists();
    assert!(results.borrow().is_empty());
    rule.advance_time_by(12000);
    rule.on_node_with_text("Silindi").assert_exists(); // no time limit
    assert!(results.borrow().is_empty());
    rule.on_node_with_text("Geri al").perform_click();
    rule.wait_for_idle();
    assert_eq!(*results.borrow(), [R::ActionPerformed]);
    rule.on_node_with_text("Kısa").assert_exists();
    rule.advance_time_by(4500); // timed out
    assert_eq!(*results.borrow(), [R::ActionPerformed, R::Dismissed]);
    rule.wait_for_idle();
    rule.on_node_with_text("Kapatılabilir").assert_exists();
    rule.on_node_with_content_description("Dismiss").perform_click(); // its close button
    rule.wait_for_idle();
    assert_eq!(*results.borrow(), [R::ActionPerformed, R::Dismissed, R::Dismissed]);
    let cancellable = scope_of(&rule);
    show(&cancellable, &state, &results, "Sonra", Some("Tamam"), None, false);
    rule.wait_for_idle();
    rule.on_node_with_text("Sonra").assert_exists();
    cancellable.cancel(); // the coroutine went: its snackbar goes too
    rule.wait_for_idle();
    assert!(state.current_snackbar_data().is_none() && results.borrow().len() == 3);
    rule.on_node_with_text("Sonra").assert_does_not_exist();
    // (C++'s "plain one", a showSnackbar without a coroutine, is a launched one here: Compose's only suspends)
    show(&scope, &state, &results, "Eski usul", Some("Tamam"), None, false);
    rule.advance_frame();
    let current = state.current_snackbar_data().expect("shown");
    assert_eq!(current.visuals().message(), "Eski usul");
    assert_eq!(current.visuals().action_label().as_deref(), Some("Tamam"));
    assert_eq!(current.visuals().duration(), SnackbarDuration::Indefinite);
    assert!(!current.visuals().with_dismiss_action());
}

fn snackbar_hosts_hand_their_data_to_a_snackbar_of_your_own() {
    let state = SnackbarHostState::new();
    let s = state.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            SnackbarHost(
                s.clone(),
                modifier = Modifier,
                snackbar = |data: Rc<dyn SnackbarData>| {
                    let (act, dismiss) = (data.clone(), data.clone());
                    let label = data.visuals().action_label().unwrap_or_default();
                    Row(
                        modifier = Modifier.test_tag("kendi"),
                        content = || {
                            Text(format!("» {}", data.visuals().message()));
                            TextButton(on_click = move || act.perform_action(), content = || Text(label.clone()));
                            TextButton(on_click = move || dismiss.dismiss(), content = || Text("Kapat"));
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let (scope, results) = (scope_of(&rule), Results::default());
    let (st, r) = (state.clone(), results.clone());
    scope.launch(async move {
        let first = named_args!(st.show_snackbar("Silindi".to_string(), action_label = Some("Geri al".to_string())));
        let result = first.await;
        r.borrow_mut().push(result);
        let second = named_args!(st.show_snackbar("Taşındı".to_string(), action_label = Some("Tamam".to_string())));
        let result = second.await;
        r.borrow_mut().push(result);
    });
    rule.wait_for_idle();
    rule.on_node_with_text("» Silindi").assert_exists();
    rule.on_node_with_text("Geri al").perform_click();
    rule.wait_for_idle();
    assert_eq!(*results.borrow(), [SnackbarResult::ActionPerformed]);
    rule.on_node_with_text("» Taşındı").assert_exists();
    rule.on_node_with_text("Kapat").perform_click();
    rule.wait_for_idle();
    assert_eq!(*results.borrow(), [SnackbarResult::ActionPerformed, SnackbarResult::Dismissed]);
    // (C++'s callback of a plain showSnackbar: the launched one's result here) performAction answers it once.
    show(&scope, &state, &results, "Eski usul", Some("Geri al"), None, false);
    rule.wait_for_idle();
    let data = state.current_snackbar_data().expect("shown");
    rule.on_node_with_text("Geri al").perform_click();
    data.dismiss(); // (answered already: nothing)
    rule.wait_for_idle();
    assert_eq!(results.borrow().last(), Some(&SnackbarResult::ActionPerformed));
    assert!(state.current_snackbar_data().is_none() && results.borrow().len() == 3);
}

#[kompozable]
fn ActionButton(label: &'static str) {
    TextButton(on_click = || {}, content = || Text(label));
}

fn snackbars_lay_out_as_compose_and_host_your_own() {
    // Compose's default layouts (ComposeMaterial3Flags' snackbar styling fix off: its legacy ones).
    let container = Color::new(0.1, 0.2, 0.3, 1.0);
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            Column(
                content = || {
                    Snackbar(
                        modifier = Modifier.test_tag("tek"),
                        action = slot(|| ActionButton("Geri al")),
                        content = || Text("Kaydedildi"),
                    );
                    Snackbar(
                        modifier = Modifier.width(220.dp()).test_tag("çift"),
                        action = slot(|| ActionButton("Tamam")),
                        content = || Text("Uzun bir ileti iki satıra iner çünkü sığmaz bu genişliğe"),
                    );
                    Snackbar(modifier = Modifier.test_tag("yalnız"), content = || Text("Yalnız")); // nothing else: still 48
                    Snackbar(
                        modifier = Modifier.test_tag("büyük"),
                        action = slot(|| Text("EYLEM", font_size = 28.sp())),
                        content = || Text("Büyük eylemli"),
                    );
                    Snackbar(
                        modifier = Modifier.width(160.dp()).test_tag("iki"),
                        action = slot(|| ActionButton("Tamam")),
                        content = || Text("İki satıra inen bir ileti"),
                    );
                    Snackbar(
                        modifier = Modifier.test_tag("alt"),
                        action = slot(|| ActionButton("Uzun bir eylem adı")),
                        action_on_new_line = true,
                        container_color = container,
                        content = || Text("Alt satırda eylem"),
                    );
                },
            );
        }),
        IntSize::new(800, 1000),
    );
    let canvas = rule.draw();
    let (one, two, below) = (bounds(&rule, "tek"), bounds(&rule, "çift"), bounds(&rule, "alt"));
    // One line: 48 dp, 600 wide inside its 16 and 8 dp sides (624, as Compose's), the action on the text's baseline
    // at the end (the button's 12 dp padding around its label).
    let (saved, undo) = (baseline_of(&canvas, "Kaydedildi"), baseline_of(&canvas, "Geri al"));
    assert!(near(one.width(), 624.0, 0.5) && near(one.height(), 48.0, 0.5), "{one:?}");
    assert!(near(saved.y, undo.y, 0.5) && near(saved.x, one.left + 16.0, 0.5), "{saved:?} {undo:?}");
    let button = text_bounds(&rule, "Geri al"); // (the button's merged node)
    assert!(near(button.right, one.right - 8.0, 0.5), "{button:?} {one:?}");
    // More lines: the first baseline 30 down, 68 dp at least; more than two: the text's 6 dp below it.
    let long = "Uzun bir ileti iki satıra iner çünkü sığmaz bu genişliğe";
    assert!(near(baseline_of(&canvas, long).y, two.top + 30.0, 0.5) && two.height() > 69.0, "{two:?}");
    assert!(near(text_bounds(&rule, long).bottom + 6.0, two.bottom, 0.5));
    assert!(near(bounds(&rule, "iki").height(), 68.0, 0.5)); // two lines: 68 dp
    assert!(near(bounds(&rule, "yalnız").height(), 48.0, 0.5));
    // a taller action still on the text's baseline (each text at whole px)
    assert!(near(baseline_of(&canvas, "Büyük eylemli").y, baseline_of(&canvas, "EYLEM").y, 1.0));
    // The action on a line of its own, at the end; its own container color.
    let action = text_bounds(&rule, "Uzun bir eylem adı");
    let text = baseline_of(&canvas, "Alt satırda eylem");
    assert!(action.top > text.y && near(action.right, below.right - 8.0, 0.5) && near(text.y, below.top + 30.0, 0.5));
    assert_eq!(canvas.rect_colors.iter().filter(|c| **c == container).count(), 1);
}

fn snackbars_with_the_styling_fix_lay_out_as_its_layouts() {
    // ComposeMaterial3Flags.isSnackbarStylingFixEnabled: OneRowSnackbar (the text 14 dp above and below, everything
    // centered in 48 dp at least) and NewLineButtonSnackbar (the action row at the end, 4 dp off the bottom).
    ComposeMaterial3Flags::set_is_snackbar_styling_fix_enabled(true);
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Column(
                content = || {
                    Snackbar(
                        modifier = Modifier.test_tag("tek"),
                        action = slot(|| ActionButton("Geri al")),
                        dismiss_action = slot(|| Box(Modifier.size(30.dp()).test_tag("kapat"))),
                        content = || Text("Kaydedildi", modifier = Modifier.test_tag("metin")),
                    );
                    Snackbar(
                        modifier = Modifier.test_tag("alt"),
                        action = slot(|| ActionButton("Uzun bir eylem adı")),
                        action_on_new_line = true,
                        content = || Text("Alt satırda eylem", modifier = Modifier.test_tag("alt metin")),
                    );
                },
            );
        }),
        IntSize::new(800, 1000),
    );
    let (one, text, close) = (bounds(&rule, "tek"), bounds(&rule, "metin"), bounds(&rule, "kapat"));
    let button = rule.on_node_with_text("Geri al").get_unclipped_bounds_in_root();
    // (a dismiss action: no 8 dp at the end; the button's 48 dp of room the tallest)
    assert!(near(one.width(), 616.0, 0.5) && one.height() >= 48.0, "{one:?}");
    assert!(near(text.center().y, one.center().y, 0.5) && near(close.center().y, one.center().y, 0.5));
    assert!(near(close.right, one.right, 0.5) && near(text.left, one.left + 16.0, 0.5));
    assert!(button.right <= close.left);
    let (below, below_text) = (bounds(&rule, "alt"), bounds(&rule, "alt metin"));
    let action = text_bounds(&rule, "Uzun bir eylem adı");
    assert!(near(below_text.top, below.top + 14.0, 0.5) && action.top >= below_text.bottom + 14.0);
    assert!(near(action.right, below.right - 8.0, 0.5), "{action:?} {below:?}");
    ComposeMaterial3Flags::set_is_snackbar_styling_fix_enabled(false);
}

fn a_cancelled_caller_takes_its_snackbar_off_the_queue() {
    // Compose's queue is a fair mutex: a caller cancelled while queued leaves it; shown, its snackbar goes.
    let state = SnackbarHostState::new();
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || SnackbarHost(s.clone())));
    let (scope, results) = (scope_of(&rule), Results::default());
    show(&scope, &state, &results, "bir", Some("A"), None, false);
    let queued = scope_of(&rule);
    show(&queued, &state, &results, "iki", Some("B"), None, false);
    show(&scope, &state, &results, "üç", Some("C"), None, false);
    rule.wait_for_idle();
    rule.on_node_with_text("bir").assert_exists();
    queued.cancel();
    state.current_snackbar_data().unwrap().dismiss();
    rule.wait_for_idle();
    rule.on_node_with_text("iki").assert_does_not_exist();
    rule.on_node_with_text("üç").assert_exists();
    assert_eq!(*results.borrow(), [SnackbarResult::Dismissed]);
}

/// An accessibility manager asking for twice the time.
struct Slower;

impl AccessibilityManager for Slower {
    fn calculate_recommended_timeout_millis(&self, original: i64, icons: bool, text: bool, controls: bool) -> i64 {
        assert!(icons && text);
        if original == i64::MAX { original } else { original * 2 + if controls { 1000 } else { 0 } }
    }
}

fn snackbar_durations_follow_the_accessibility_manager() {
    // Compose's SnackbarDuration.toMillis: 4 s short, 10 s long, indefinite; what LocalAccessibilityManager
    // recommends for icons, text and (with an action) controls.
    let state = SnackbarHostState::new();
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        KompozitionLocalProvider(
            &[LocalAccessibilityManager.provides_unequal(Some(Rc::new(Slower) as Rc<dyn AccessibilityManager>))],
            move || SnackbarHost(s.clone()),
        );
    }));
    let (scope, results) = (scope_of(&rule), Results::default());
    show(&scope, &state, &results, "kısa", None, None, false);
    rule.wait_for_idle();
    rule.advance_time_by(7800);
    assert!(results.borrow().is_empty());
    rule.advance_time_by(400);
    assert_eq!(results.borrow().len(), 1);
    // A long one with an action: (10 s twice and 1 s for its control)
    show(&scope, &state, &results, "uzun", Some("Tamam"), Some(SnackbarDuration::Long), false);
    rule.wait_for_idle();
    rule.advance_time_by(20800);
    assert_eq!(results.borrow().len(), 1);
    rule.advance_time_by(400);
    assert_eq!(*results.borrow(), [SnackbarResult::Dismissed, SnackbarResult::Dismissed]);
    // Indefinite: no timer at all (the rule idles without jumping to the end of time).
    show(&scope, &state, &results, "sürekli", None, Some(SnackbarDuration::Indefinite), false);
    rule.wait_for_idle();
    rule.advance_time_by(60000);
    rule.on_node_with_text("sürekli").assert_exists();
}

fn the_host_fades_and_scales_snackbars_and_tells_accessibility() {
    // FadeInFadeOutWithScale: in from 80% and transparent, out to 80% and transparent (the motion scheme's fast
    // spatial and effects specs), the one going drawn until it is out; a polite live region titled "Alert",
    // dismissed by accessibility's dismiss action.
    let state = SnackbarHostState::new();
    let s = state.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || SnackbarHost(s.clone())));
    let (scope, results) = (scope_of(&rule), Results::default());
    show(&scope, &state, &results, "ileti", Some("Tamam"), None, false);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let coming = rule.draw();
    let growing = coming.transforms.iter().any(|m| m.values[0] > 0.8 && m.values[0] < 0.99);
    let fading = coming.layer_alphas.iter().any(|a| *a > 0.01 && *a < 0.99);
    assert!(growing && fading, "{:?} {:?}", coming.transforms, coming.layer_alphas);
    rule.wait_for_idle();
    let node = rule.on_node_with_text("ileti").fetch_semantics_node();
    let mut pane = None;
    let mut live = None;
    let mut dismiss = None;
    let mut n = Some(node);
    while let Some(node) = n {
        let config = node.config();
        pane = pane.or_else(|| config.get(SemanticsProperties::PaneTitle));
        live = live.or_else(|| config.get(SemanticsProperties::LiveRegion));
        dismiss = dismiss.or_else(|| config.get(SemanticsActions::Dismiss));
        n = node.parent();
    }
    assert_eq!(pane.as_deref(), Some("Alert"));
    assert_eq!(live, Some(LiveRegionMode::Polite));
    (dismiss.expect("a dismiss action").action.expect("its action"))();
    rule.advance_frame();
    rule.advance_frame();
    assert_eq!(*results.borrow(), [SnackbarResult::Dismissed]);
    rule.on_node_with_text("ileti").assert_exists(); // still going
    rule.wait_for_idle();
    rule.on_node_with_text("ileti").assert_does_not_exist();
}

fn snackbar_defaults_are_the_tokens() {
    let seen = Rc::new(RefCell::new(None));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = MaterialTheme::color_scheme();
        *s.borrow_mut() = Some((
            c,
            SnackbarDefaults::shape(),
            SnackbarDefaults::color(),
            SnackbarDefaults::content_color(),
            SnackbarDefaults::action_color(),
            SnackbarDefaults::action_content_color(),
            SnackbarDefaults::dismiss_action_content_color(),
            MaterialTheme::shapes().extra_small,
        ));
    }));
    rule.wait_for_idle();
    let (c, shape, color, content, action, action_content, dismiss, extra_small) = seen.borrow().clone().unwrap();
    let extra_small: Arc<dyn Shape> = Arc::new(extra_small);
    assert!(*shape == *extra_small);
    assert_eq!((color, content), (c.inverse_surface, c.inverse_on_surface));
    assert_eq!((action, action_content, dismiss), (c.inverse_primary, c.inverse_primary, c.inverse_on_surface));
}

// ---- M15's snackbar samples (ScaffoldSamples.kt), over a Scaffold as written

/// `ScaffoldWithSimpleSnackbar`.
#[kompozable]
fn ScaffoldWithSimpleSnackbar() {
    let snackbar_host_state = remember(SnackbarHostState::new);
    let scope = kompoz_coroutines::remember_coroutine_scope();
    let host = snackbar_host_state.clone();
    Scaffold(
        snackbar_host = move || SnackbarHost(host.clone()),
        floating_action_button = move || {
            let click_count = remember(|| mutable_state_of(0));
            let (scope, state) = (scope.clone(), snackbar_host_state.clone());
            ExtendedFloatingActionButton(
                on_click = move || {
                    // show snackbar as a suspend function
                    click_count.set(click_count.get() + 1);
                    let shown = state.show_snackbar(format!("Snackbar # {}", click_count.get()));
                    launch_show(&scope, shown);
                },
                content = || Text("Show snackbar"),
            );
        },
        content = |inner_padding: PaddingValues| {
            Text(
                text = "Body content",
                modifier = Modifier.padding_values(inner_padding).fill_max_size().wrap_content_size(),
            );
        },
    );
}

fn scaffold_with_simple_snackbar_sample() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| ScaffoldWithSimpleSnackbar()));
    rule.on_node_with_text("Body content").assert_exists();
    rule.on_node_with_text("Show snackbar").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Snackbar # 1").assert_exists();
    rule.on_node_with_text("Show snackbar").perform_click(); // queued
    rule.advance_time_by(4500);
    rule.wait_for_idle();
    rule.on_node_with_text("Snackbar # 1").assert_does_not_exist();
    rule.on_node_with_text("Snackbar # 2").assert_exists();
    // (the scaffold puts the host right above the 56 dp button, itself 16 dp above the bottom: the snackbar in it,
    // padded 12 dp, centered)
    let snackbar = text_bounds(&rule, "Snackbar # 2");
    assert!(snackbar.bottom > 600.0 && snackbar.bottom < 800.0 - 16.0 - 56.0 - 12.0, "{snackbar:?}");
    let host = rule.on_node_with_text("Show snackbar").get_unclipped_bounds_in_root();
    assert!(host.right == 400.0 - 16.0 && host.bottom == 800.0 - 16.0, "{host:?}");
}

/// `ScaffoldWithIndefiniteSnackbar`.
#[kompozable]
fn ScaffoldWithIndefiniteSnackbar() {
    let snackbar_host_state = remember(SnackbarHostState::new);
    let scope = kompoz_coroutines::remember_coroutine_scope();
    let host = snackbar_host_state.clone();
    Scaffold(
        snackbar_host = move || SnackbarHost(host.clone()),
        floating_action_button = move || {
            let click_count = remember(|| mutable_state_of(0));
            let (scope, state) = (scope.clone(), snackbar_host_state.clone());
            ExtendedFloatingActionButton(
                on_click = move || {
                    // show snackbar as a suspend function
                    click_count.set(click_count.get() + 1);
                    let shown = named_args!(state.show_snackbar(
                        format!("Snackbar # {}", click_count.get()),
                        action_label = Some("Action".to_string()),
                        with_dismiss_action = true,
                        duration = Some(SnackbarDuration::Indefinite),
                    ));
                    launch_show(&scope, shown);
                },
                content = || Text("Show snackbar"),
            );
        },
        content = |inner_padding: PaddingValues| {
            Text(
                text = "Body content",
                modifier = Modifier.padding_values(inner_padding).fill_max_size().wrap_content_size(),
            );
        },
    );
}

fn scaffold_with_indefinite_snackbar_sample() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| ScaffoldWithIndefiniteSnackbar()));
    rule.on_node_with_text("Show snackbar").perform_click();
    rule.wait_for_idle();
    rule.advance_time_by(15000);
    rule.on_node_with_text("Snackbar # 1").assert_exists();
    rule.on_node_with_text("Action").assert_exists();
    // its dismiss button, a tooltip telling it on a long press
    let close = rule.on_node_with_content_description("Dismiss").get_unclipped_bounds_in_root();
    rule.host().pointer_down(close.center(), -1, 0, PointerType::Touch);
    rule.advance_time_by(600);
    rule.on_node_with_text("Dismiss").assert_exists();
    rule.host().pointer_up(close.center(), -1, 0);
    rule.wait_for_idle();
    rule.on_node_with_text("Snackbar # 1").assert_exists(); // (the long press didn't click)
    rule.on_node_with_content_description("Dismiss").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Snackbar # 1").assert_does_not_exist();
}

/// `ScaffoldWithCustomSnackbar`'s `class SnackbarVisualsWithError`.
struct SnackbarVisualsWithError {
    message: String,
    is_error: bool,
}

impl SnackbarVisuals for SnackbarVisualsWithError {
    fn message(&self) -> String {
        self.message.clone()
    }
    fn action_label(&self) -> Option<String> {
        Some(if self.is_error { "Error" } else { "OK" }.to_string())
    }
    fn with_dismiss_action(&self) -> bool {
        false
    }
    fn duration(&self) -> SnackbarDuration {
        SnackbarDuration::Indefinite
    }
}

thread_local! {
    /// What the custom sample's snackbars were shown with (Kotlin's `data.visuals as? SnackbarVisualsWithError`:
    /// a trait object can't be cast back, so the sample's visuals are found by their message).
    static ERRORS: RefCell<Vec<(String, bool)>> = const { RefCell::new(Vec::new()) };
}

/// `ScaffoldWithCustomSnackbar`.
#[kompozable]
fn ScaffoldWithCustomSnackbar() {
    let snackbar_host_state = remember(SnackbarHostState::new);
    let scope = kompoz_coroutines::remember_coroutine_scope();
    let host = snackbar_host_state.clone();
    Scaffold(
        snackbar_host = move || {
            // reuse default SnackbarHost to have default animation and timing handling
            SnackbarHost(
                host.clone(),
                snackbar = |data: Rc<dyn SnackbarData>| {
                    // custom snackbar with the custom action button color and border
                    let message = data.visuals().message();
                    let is_error = ERRORS.with(|e| e.borrow().iter().any(|(m, error)| *m == message && *error));
                    let button_color = if is_error {
                        named_args!(ButtonDefaults.text_button_colors(
                            container_color = MaterialTheme::color_scheme().error_container,
                            content_color = MaterialTheme::color_scheme().error,
                        ))
                    } else {
                        named_args!(
                            ButtonDefaults
                                .text_button_colors(content_color = MaterialTheme::color_scheme().inverse_primary)
                        )
                    };
                    let d = data.clone();
                    Snackbar(
                        modifier = Modifier
                            .border(2.dp(), MaterialTheme::color_scheme().secondary)
                            .padding(12.dp())
                            .test_tag("özel"),
                        action = slot(move || {
                            let d = d.clone();
                            let label = d.visuals().action_label().unwrap_or_default();
                            TextButton(
                                on_click = move || if is_error { d.dismiss() } else { d.perform_action() },
                                colors = button_color,
                                content = move || Text(label.clone()),
                            );
                        }),
                        content = move || Text(message.clone()),
                    );
                },
            );
        },
        floating_action_button = move || {
            let click_count = remember(|| mutable_state_of(0));
            let (scope, state) = (scope.clone(), snackbar_host_state.clone());
            ExtendedFloatingActionButton(
                on_click = move || {
                    click_count.set(click_count.get() + 1);
                    let visuals = SnackbarVisualsWithError {
                        message: format!("Snackbar # {}", click_count.get()),
                        is_error: click_count.get() % 2 != 0,
                    };
                    ERRORS.with(|e| e.borrow_mut().push((visuals.message.clone(), visuals.is_error)));
                    let shown = state.show_snackbar_visuals(Rc::new(visuals));
                    launch_show(&scope, shown);
                },
                content = || Text("Show snackbar"),
            );
        },
        content = |inner_padding: PaddingValues| {
            Text(
                text = "Custom Snackbar Demo",
                modifier = Modifier.padding_values(inner_padding).fill_max_size().wrap_content_size(),
            );
        },
    );
}

fn scaffold_with_custom_snackbar_sample() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| ScaffoldWithCustomSnackbar()));
    let error_container = Rc::new(RefCell::new(Color::Unspecified));
    rule.on_node_with_text("Show snackbar").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Snackbar # 1").assert_exists();
    rule.on_node_with_text("Error").assert_exists(); // the first is an error
    let canvas = rule.draw();
    let _ = error_container;
    assert!(!canvas.strokes.is_empty()); // its border
    rule.on_node_with_text("Error").perform_click(); // dismisses it
    rule.on_node_with_text("Show snackbar").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Snackbar # 2").assert_exists();
    rule.on_node_with_text("OK").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Snackbar # 2").assert_does_not_exist();
}

/// `ScaffoldWithCoroutinesSnackbar`.
#[kompozable]
fn ScaffoldWithCoroutinesSnackbar(results: Results) {
    // decouple snackbar host state from scaffold state for demo purposes
    // this state, channel and flow is for demo purposes to demonstrate business logic layer
    let snackbar_host_state = remember(SnackbarHostState::new);
    // we allow only one snackbar to be in the queue here, hence conflated
    let channel = remember(Channel::<i32>::conflated);
    let (state, ch) = (snackbar_host_state.clone(), channel.clone());
    LaunchedEffect(ChannelKey(channel.clone()), async move {
        ch.receive_as_flow()
            .collect_async(move |index| {
                let (state, results) = (state.clone(), results.clone());
                async move {
                    let result = named_args!(state.show_snackbar(
                        format!("Snackbar # {index}"),
                        action_label = Some(format!("Action on {index}")),
                    ))
                    .await;
                    match result {
                        SnackbarResult::ActionPerformed => { /* action has been performed */ }
                        SnackbarResult::Dismissed => { /* dismissed, no action needed */ }
                    }
                    results.borrow_mut().push(result);
                }
            })
            .await;
    });
    let host = snackbar_host_state.clone();
    Scaffold(
        snackbar_host = move || SnackbarHost(host.clone()),
        floating_action_button = move || {
            let click_count = remember(|| mutable_state_of(0));
            let channel = channel.clone();
            ExtendedFloatingActionButton(
                on_click = move || {
                    // offset snackbar data to the business logic
                    click_count.set(click_count.get() + 1);
                    channel.try_send(click_count.get());
                },
                content = || Text("Show snackbar"),
            );
        },
        content = |inner_padding: PaddingValues| {
            Text(
                text = "Snackbar demo",
                modifier = Modifier.padding_values(inner_padding).fill_max_size().wrap_content_size(),
            );
        },
    );
}

/// A channel as an effect's key (the same channel).
#[derive(Clone)]
struct ChannelKey(Channel<i32>);

impl PartialEq for ChannelKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

fn scaffold_with_coroutines_snackbar_sample() {
    let results = Results::default();
    let r = results.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || ScaffoldWithCoroutinesSnackbar(r.clone())));
    rule.on_node_with_text("Show snackbar").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Snackbar # 1").assert_exists();
    // two more clicks while it shows: the conflated channel keeps the last
    rule.on_node_with_text("Show snackbar").perform_click();
    rule.on_node_with_text("Show snackbar").perform_click();
    rule.on_node_with_text("Action on 1").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Snackbar # 3").assert_exists();
    rule.on_node_with_text("Snackbar # 2").assert_does_not_exist();
    assert_eq!(*results.borrow(), [SnackbarResult::ActionPerformed]);
}

/// `ScaffoldWithMultilineSnackbar`.
#[kompozable]
fn ScaffoldWithMultilineSnackbar() {
    let snackbar_host_state = remember(SnackbarHostState::new);
    let scope = kompoz_coroutines::remember_coroutine_scope();
    let host = snackbar_host_state.clone();
    Scaffold(
        snackbar_host = move || {
            SnackbarHost(
                host.clone(),
                snackbar = |data: Rc<dyn SnackbarData>| {
                    Snackbar(
                        modifier = Modifier.test_tag("çok satır"),
                        content = move || {
                            // The Material spec recommends a maximum of 2 lines of text.
                            Text(data.visuals().message(), max_lines = 2, overflow = TextOverflow::Ellipsis);
                        },
                    );
                },
            );
        },
        floating_action_button = move || {
            let (scope, state) = (scope.clone(), snackbar_host_state.clone());
            ExtendedFloatingActionButton(
                on_click = move || {
                    let long_message = "Very very very very very very very very very very very very very \
                                        very very very very very very very very very very very very \
                                        very very very very very very very very very very long message";
                    let shown = state.show_snackbar(long_message.to_string());
                    launch_show(&scope, shown);
                },
                content = || Text("Show snackbar"),
            );
        },
        content = |inner_padding: PaddingValues| {
            Text(
                text = "Multiline Snackbar Demo",
                modifier = Modifier.padding_values(inner_padding).fill_max_size().wrap_content_size(),
            );
        },
    );
}

fn scaffold_with_multiline_snackbar_sample() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| ScaffoldWithMultilineSnackbar()));
    rule.on_node_with_text("Show snackbar").perform_click();
    rule.wait_for_idle();
    // two lines at most: 68 dp
    assert!(near(bounds(&rule, "çok satır").height(), 68.0, 0.5), "{:?}", bounds(&rule, "çok satır"));
}

// ==== tooltips =======================================================================================================

/// A tooltip's state held for a test.
type Held = Rc<RefCell<Option<Rc<dyn TooltipState>>>>;

fn held(h: &Held) -> Rc<dyn TooltipState> {
    h.borrow().clone().expect("a tooltip state")
}

/// The path drawn in `color` (a tooltip's container with its caret) and where its own coordinates are in the root.
fn path_of(canvas: &RecordingCanvas, color: Color) -> Option<(Rect, Path, Offset)> {
    (0..canvas.paths.len()).find(|i| canvas.path_colors[*i] == color && canvas.path_strokes[*i] == 0.0).map(|i| {
        let shape = canvas.path_shapes[i].clone();
        let origin =
            Offset::new(canvas.paths[i].left - shape.get_bounds().left, canvas.paths[i].top - shape.get_bounds().top);
        (canvas.paths[i], shape, origin)
    })
}

fn tooltip_box_goes_on_the_side_its_position_says() {
    // M15's spacing: 4 dp off the anchor, the caret over it (C++'s tooltip moved away by its caret's 8 dp more too:
    // 12); centered on the anchor across or along.
    let side = mutable_state_of(TooltipAnchorPosition::Above);
    let rtl = mutable_state_of(false);
    let anchor_x = mutable_state_of(300.0f32);
    let spacing = mutable_state_of(4.0f32);
    let h: Held = Rc::default();
    let hh = h.clone();
    let (side_set, rtl_set, anchor_x_set, spacing_set) = (side.clone(), rtl.clone(), anchor_x.clone(), spacing.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let direction = if rtl.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
            let (hh, side, spacing, anchor_x) = (hh.clone(), side.clone(), spacing.clone(), anchor_x.clone());
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
                let (hh, side, spacing, anchor_x) = (hh.clone(), side.clone(), spacing.clone(), anchor_x.clone());
                Box(
                    modifier = Modifier.fill_max_size(),
                    content = move || {
                        // from the start edge
                        let (hh, side, spacing) = (hh.clone(), side.clone(), spacing.clone());
                        // (a box of its own for each side and place: Compose's tooltip scope keeps the first
                        // position provider, and the caret is laid out with the tooltip, which the anchor moving
                        // alone doesn't lay out again — a box changed in place would keep its caret)
                        let place = (side.get().to_string(), anchor_x.get().to_bits(), spacing.get().to_bits());
                        Box(
                            modifier = Modifier.offset(anchor_x.get().dp(), 300.dp()),
                            content = move || {
                                let (hh, side, spacing) = (hh.clone(), side.clone(), spacing.clone());
                                key(&place, move || {
                                    let s = remember_tooltip_state(initial_is_visible = true, is_persistent = true);
                                    *hh.borrow_mut() = Some(s.clone());
                                    TooltipBox(
                                        position_provider =
                                            named_args!(TooltipDefaults.remember_tooltip_position_provider(
                                                side.get(),
                                                spacing_between_tooltip_and_anchor = spacing.get().dp()
                                            )),
                                        tooltip = |scope: &dyn TooltipScope| {
                                            // three lines tall
                                            scope.PlainTooltip(
                                                modifier = Modifier.test_tag("ipucu"),
                                                caret_shape = Some(TooltipDefaults::caret_shape()),
                                                content =
                                                    || Text("Bu ipucu üç satıra yayılacak kadar uzun bir açıklama"),
                                            );
                                        },
                                        state = s,
                                        content = || Box(Modifier.size(48.dp()).test_tag("çapa")),
                                    );
                                });
                            },
                        );
                    },
                );
            });
        }),
        IntSize::new(800, 800),
    );
    let inverse = Rc::new(RefCell::new(Color::Unspecified));
    let at = |p: TooltipAnchorPosition| {
        side_set.set(p);
        rule.wait_for_idle();
        (bounds(&rule, "ipucu"), bounds(&rule, "çapa"))
    };
    let caret = || {
        let canvas = rule.draw();
        // (the tooltip's container and its caret, one path)
        let color = canvas.path_colors.iter().copied().find(|c| c.alpha() > 0.9).unwrap_or(*inverse.borrow());
        path_of(&canvas, color).map(|(b, _, _)| b).expect("the tooltip's path")
    };
    let (above, a) = at(TooltipAnchorPosition::Above);
    assert!(near(above.bottom, a.top - 4.0, 0.5) && near(above.center().x, a.center().x, 0.5), "{above:?} {a:?}");
    let c = caret();
    // (the caret under the tooltip, 8 dp, pointing down at the anchor)
    assert!(near(c.bottom, above.bottom + 8.0, 0.5) && near(c.top, above.top, 0.5), "{c:?} {above:?}");
    let (below, b) = at(TooltipAnchorPosition::Below);
    assert!(near(below.top, b.bottom + 4.0, 0.5) && near(below.center().x, b.center().x, 0.5));
    let c = caret();
    assert!(near(c.top, below.top - 8.0, 0.5), "{c:?} {below:?}");
    let (left, l) = at(TooltipAnchorPosition::Left);
    let c = caret();
    assert!(near(left.right, l.left - 4.0, 0.5) && near(left.center().y, l.center().y, 0.5));
    assert!(near(c.right, left.right + 8.0, 0.5) && left.height() >= 40.0); // the caret at its middle
    let (right, r) = at(TooltipAnchorPosition::Right);
    let c = caret();
    assert!(near(right.left, r.right + 4.0, 0.5) && near(right.center().y, r.center().y, 0.5));
    assert!(near(c.left, right.left - 8.0, 0.5));
    // Start and End as the layout goes.
    assert!(near(at(TooltipAnchorPosition::Start).0.right, l.left - 4.0, 0.5));
    assert!(near(at(TooltipAnchorPosition::End).0.left, r.right + 4.0, 0.5));
    rtl_set.set(true); // (the anchor now 300 dp from the right)
    let (start, s1) = at(TooltipAnchorPosition::Start);
    let (end, e1) = at(TooltipAnchorPosition::End);
    assert!(near(start.left, s1.right + 4.0, 0.5) && near(end.right, e1.left - 4.0, 0.5) && near(s1.right, 500.0, 0.5));
    rtl_set.set(false);
    // No room on its side: the other one.
    anchor_x_set.set(8.0);
    let (flipped, f) = at(TooltipAnchorPosition::Left);
    assert!(near(flipped.left, f.right + 4.0, 0.5));
    anchor_x_set.set(792.0 - 48.0);
    let (back, g) = at(TooltipAnchorPosition::Right);
    assert!(near(back.right, g.left - 4.0, 0.5));
    // Farther off, as asked.
    anchor_x_set.set(300.0);
    spacing_set.set(10.0);
    let (far, k) = at(TooltipAnchorPosition::Above);
    assert!(near(far.bottom, k.top - 10.0, 0.5));
    let _ = held(&h);
}

/// A row 150 dp in and 300 dp down holding a favorite icon button with a plain tooltip; its clicks counted.
fn favorite(h: Held, clicks: Rc<RefCell<i32>>, source: Option<MutableInteractionSource>) -> impl Content + 'static {
    kompozable_lambda!(move || {
        let (h, clicks, source) = (h.clone(), clicks.clone(), source.clone());
        Row(
            modifier = Modifier.padding_sides(start = 150.dp(), top = 300.dp()),
            content = move || {
                let s = remember_tooltip_state();
                *h.borrow_mut() = Some(s.clone());
                let (clicks, source) = (clicks.clone(), source.clone());
                TooltipBox(
                    position_provider =
                        TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                    tooltip = |scope: &dyn TooltipScope| scope.PlainTooltip(content = || Text("Favorilere ekle")),
                    state = s,
                    content = move || {
                        let clicks = clicks.clone();
                        IconButton(
                            on_click = move || *clicks.borrow_mut() += 1,
                            interaction_source = source.clone(),
                            content =
                                || VectorIcon(Icons::Filled::Favorite(), content_description = Some("Favori".into())),
                        );
                    },
                );
            },
        );
    })
}

fn tooltip_box_shows_on_long_press_and_hover() {
    let (h, clicks): (Held, _) = (Rc::default(), Rc::new(RefCell::new(0)));
    let rule = KompozTestRule::with_viewport(favorite(h.clone(), clicks.clone(), None), IntSize::new(400, 800));
    let host = rule.host();
    let anchor = rule.on_node_with_content_description("Favori").get_unclipped_bounds_in_root();
    let touch = |hold_millis: i64| {
        // a finger: it doesn't hover
        host.pointer_down(anchor.center(), -1, 0, PointerType::Touch);
        rule.advance_time_by(hold_millis);
        host.pointer_up(anchor.center(), -1, 0);
        host.pointer_exit();
        rule.advance_frame();
    };
    touch(100); // a tap still clicks
    rule.advance_time_by(1000);
    assert!(*clicks.borrow() == 1 && !held(&h).is_visible());
    // A long press shows it above the anchor, centered, and doesn't click.
    touch(600);
    assert!(held(&h).is_visible() && host.overlay_count() == 1 && *clicks.borrow() == 1);
    let tip = text_bounds(&rule, "Favorilere ekle");
    assert!(tip.bottom < anchor.top && (tip.center().x - anchor.center().x).abs() < 2.0, "{tip:?} {anchor:?}");
    rule.advance_time_by(1600); // goes away by itself (1.5 s from the long press's show)
    rule.wait_for_idle();
    assert!(!held(&h).is_visible());
    assert_eq!(host.overlay_count(), 0);
    // The mouse entering shows it at once (Compose's BasicTooltipBox: on Enter; C++'s after a moment), while it
    // stays (shown for user input: no time limit); leaving hides it.
    mouse_to(&rule, anchor.center());
    rule.advance_frame();
    assert!(held(&h).is_visible());
    rule.advance_time_by(2000);
    assert!(held(&h).is_visible());
    mouse_to(&rule, Offset::new(20.0, 20.0));
    rule.advance_frame();
    assert!(!held(&h).transition().target_state()); // (visible until it has faded out)
    rule.wait_for_idle();
    assert!(!held(&h).is_visible());
}

fn tooltip_caret_points_at_the_anchor() {
    // C++'s, the plain tooltip's half (the rich tooltip's is WP7's): M15's 4 dp spacing, the caret over it.
    let top = mutable_state_of(300.0f32);
    let left = mutable_state_of(150.0f32);
    let side = mutable_state_of(40.0f32);
    let with_caret = mutable_state_of(true);
    let h: Held = Rc::default();
    let inverse = Rc::new(RefCell::new(Color::Unspecified));
    let (hh, inv) = (h.clone(), inverse.clone());
    let (top_get, left_get, side_get, with_caret_get) = (top.clone(), left.clone(), side.clone(), with_caret.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *inv.borrow_mut() = MaterialTheme::color_scheme().inverse_surface;
            let (hh, with_caret, side) = (hh.clone(), with_caret_get.clone(), side_get.clone());
            let place = (left_get.get().to_bits(), top_get.get().to_bits(), side.get().to_bits());
            Row(
                modifier = Modifier.padding_sides(start = left_get.get().dp(), top = top_get.get().dp()),
                content = move || {
                    // (a box of its own for each place: the caret is laid out with the tooltip, which the anchor
                    // moving alone doesn't lay out again, in Compose too)
                    let (hh, with_caret, side) = (hh.clone(), with_caret.clone(), side.clone());
                    key(&place, move || {
                        let s = remember_tooltip_state(initial_is_visible = true, is_persistent = true);
                        *hh.borrow_mut() = Some(s.clone());
                        let caret = if with_caret.get() { Some(TooltipDefaults::caret_shape()) } else { None };
                        let side = side.clone();
                        TooltipBox(
                            position_provider =
                                TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                            tooltip = move |scope: &dyn TooltipScope| {
                                scope.PlainTooltip(
                                    modifier = Modifier.test_tag("tip"),
                                    caret_shape = caret.clone(),
                                    content = || Text("Favorilere ekle"),
                                );
                            },
                            state = s,
                            content = move || Box(Modifier.size(side.get().dp()).test_tag("anchor")),
                        );
                    });
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let inverse = *inverse.borrow();
    // The caret's tip (8 dp under the tooltip) at `x`: inside the path there, not 6 dp to either side.
    let tip_at = |x: f32, y: f32| {
        let canvas = rule.draw();
        let (_, path, origin) = path_of(&canvas, inverse).expect("the tooltip's path");
        path.contains(Offset::new(x, y) - origin)
    };
    let anchor = bounds(&rule, "anchor");
    // Above: 4 dp off the anchor, the caret under it pointing down at the anchor's middle, over the spacing.
    let tip = bounds(&rule, "tip");
    assert!(near(tip.bottom, anchor.top - 4.0, 0.5), "{tip:?} {anchor:?}");
    let x = anchor.center().x;
    assert!(tip_at(x, tip.bottom + 2.0) && !tip_at(x - 7.0, tip.bottom + 2.0) && !tip_at(x + 7.0, tip.bottom + 2.0));
    assert!(!tip_at(x, tip.bottom + 8.5));
    // No room above: under the anchor, the caret on top pointing up.
    top.set(10.0);
    rule.wait_for_idle();
    let (anchor, tip) = (bounds(&rule, "anchor"), bounds(&rule, "tip"));
    assert!(near(tip.top, anchor.bottom + 4.0, 0.5));
    assert!(tip_at(anchor.center().x, tip.top - 2.0) && !tip_at(anchor.center().x + 7.0, tip.top - 2.0));
    // Near the window's edge the tooltip is pushed in (start aligned); the caret still points at the anchor.
    left.set(0.0);
    rule.wait_for_idle();
    let (anchor, tip) = (bounds(&rule, "anchor"), bounds(&rule, "tip"));
    assert!(near(tip.left, anchor.left, 0.5) && tip.center().x > anchor.center().x + 10.0, "{tip:?} {anchor:?}");
    assert!(tip_at(anchor.center().x, tip.top - 2.0));
    // None asked for: no caret, 4 dp off.
    with_caret.set(false);
    top.set(300.0);
    left.set(150.0);
    rule.wait_for_idle();
    let canvas = rule.draw();
    assert!(path_of(&canvas, inverse).is_none()); // (a plain rounded rect)
    let (anchor, tip) = (bounds(&rule, "anchor"), bounds(&rule, "tip"));
    assert!(near(tip.bottom, anchor.top - 4.0, 0.5));
    let _ = held(&h);
}

fn a_lifted_finger_hovers_nothing() {
    // A platform that doesn't say the finger left (no exit after it): what it lifted over isn't hovered, however
    // many frames are drawn after; the tooltip's mouse handling doesn't show it. (Hovering told by the icon
    // button's interaction source, as Compose's `collectIsHoveredAsState`.)
    let (h, clicks): (Held, _) = (Rc::default(), Rc::new(RefCell::new(0)));
    let source = MutableInteractionSource::new();
    let hovered = Rc::new(RefCell::new(0i32));
    let rule = KompozTestRule::with_viewport(
        favorite(h.clone(), clicks.clone(), Some(source.clone())),
        IntSize::new(400, 800),
    );
    let (hv, s) = (hovered.clone(), source.clone());
    scope_of(&rule).launch(async move {
        s.interactions()
            .collect(move |it: Arc<dyn Interaction>| {
                if it.is::<HoverInteraction::Enter>() {
                    *hv.borrow_mut() += 1;
                } else if it.is::<HoverInteraction::Exit>() {
                    *hv.borrow_mut() -= 1;
                }
            })
            .await
    });
    let host = rule.host();
    let anchor = rule.on_node_with_content_description("Favori").get_unclipped_bounds_in_root();
    mouse_to(&rule, anchor.center()); // a mouse rests on it: it is hovered
    rule.advance_frame();
    assert_eq!(*hovered.borrow(), 1);
    host.pointer_exit();
    rule.advance_frame();
    held(&h).dismiss();
    rule.advance_frame();
    host.pointer_down(anchor.center(), -1, 0, PointerType::Touch); // then a finger taps it
    rule.advance_time_by(100);
    host.pointer_up(anchor.center(), -1, 0);
    rule.advance_frame();
    assert_eq!(*hovered.borrow(), 0);
    rule.advance_time_by(1500); // the ripple drawn all along
    assert!(!held(&h).is_visible() && *hovered.borrow() == 0 && *clicks.borrow() == 1);
}

fn view_configuration_sets_the_gesture_thresholds() {
    // C++'s, the tooltip's half (the rest is foundation's, tests/gestures.rs): a tooltip's long press takes the
    // host's long press timeout.
    let (h, clicks): (Held, _) = (Rc::default(), Rc::new(RefCell::new(0)));
    let rule = KompozTestRule::with_viewport(favorite(h.clone(), clicks, None), IntSize::new(400, 800));
    let host = rule.host();
    host.set_view_configuration(ViewConfiguration {
        long_press_timeout: Duration::from_millis(600),
        double_tap_timeout: Duration::from_millis(200),
        touch_slop: 20.0,
        maximum_fling_velocity: 1000.0,
        ..ViewConfiguration::default()
    });
    rule.wait_for_idle();
    let anchor = rule.on_node_with_content_description("Favori").get_unclipped_bounds_in_root().center();
    host.pointer_down(anchor, -1, 0, PointerType::Touch);
    rule.advance_time_by(500);
    assert!(!held(&h).is_visible());
    rule.advance_time_by(150);
    assert!(held(&h).is_visible());
    host.pointer_up(anchor, -1, 0);
    host.pointer_exit();
    rule.wait_for_idle();
    let _ = LocalViewConfiguration;
}

fn tooltip_states_show_for_a_while_or_until_dismissed() {
    // rememberTooltipState: not persistent by default — 1.5 s shown from code; persistent: until dismissed or a
    // press outside. TooltipState(): persistent by default. One shown at a time through the global mutex; a mutex
    // of their own shows both.
    let states: Rc<RefCell<Vec<Rc<dyn TooltipState>>>> = Rc::default();
    let own = MutatorMutex::new();
    let st = states.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (st, own) = (st.clone(), own.clone());
            Column(
                modifier = Modifier.padding_sides(top = 300.dp()),
                content = move || {
                    let made = vec![
                        remember_tooltip_state(),
                        remember_tooltip_state(is_persistent = true),
                        remember(|| TooltipState(false, true, MutatorMutex::new())),
                        remember_tooltip_state(mutator_mutex = own.clone()),
                    ];
                    *st.borrow_mut() = made.clone();
                    for (i, s) in made.into_iter().enumerate() {
                        TooltipBox(
                            position_provider =
                                TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Below),
                            tooltip = move |scope: &dyn TooltipScope| {
                                scope.PlainTooltip(content = move || Text(format!("ipucu {i}")))
                            },
                            state = s,
                            content = move || Box(Modifier.size(40.dp()).test_tag(format!("çapa {i}"))),
                        );
                    }
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let state = |i: usize| states.borrow()[i].clone();
    let scope = scope_of(&rule);
    assert!(!state(0).is_persistent() && state(1).is_persistent() && state(2).is_persistent());
    scope.launch(state(0).show(MutatePriority::Default));
    rule.advance_frame();
    assert!(state(0).is_visible());
    rule.advance_time_by(1400);
    assert!(state(0).is_visible());
    rule.advance_time_by(200);
    rule.wait_for_idle();
    assert!(!state(0).is_visible());
    // Persistent: stays; another of the global mutex dismisses it.
    scope.launch(state(1).show(MutatePriority::Default));
    rule.advance_time_by(3000);
    assert!(state(1).is_visible());
    scope.launch(state(0).show(MutatePriority::Default));
    rule.wait_for_idle();
    assert!(!state(1).is_visible());
    rule.advance_time_by(2000);
    rule.wait_for_idle();
    // A mutex of their own: both shown.
    scope.launch(state(2).show(MutatePriority::Default));
    scope.launch(state(3).show(MutatePriority::Default));
    rule.advance_frame();
    rule.on_node_with_text("ipucu 2").assert_exists();
    rule.on_node_with_text("ipucu 3").assert_exists();
    rule.wait_for_idle();
    // Dismissed from code; a press outside dismisses a persistent one too.
    state(2).dismiss();
    rule.wait_for_idle();
    rule.on_node_with_text("ipucu 2").assert_does_not_exist();
    scope.launch(state(1).show(MutatePriority::Default));
    rule.wait_for_idle();
    rule.host().pointer_down(Offset::new(380.0, 780.0), -1, 0, PointerType::Touch); // outside
    rule.host().pointer_up(Offset::new(380.0, 780.0), -1, 0);
    rule.wait_for_idle();
    assert!(!state(1).is_visible());
    // Shown with PreventUserInput, its end (timed out) doesn't dismiss it: Compose's `finally`.
    scope.launch(state(0).show(MutatePriority::PreventUserInput));
    rule.advance_time_by(1600);
    rule.wait_for_idle();
    assert!(state(0).is_visible());
    state(0).dismiss();
    rule.wait_for_idle();
}

fn the_tooltip_box_grows_and_fades_its_tooltip() {
    // TooltipBox's transition: from 80% and transparent to whole (the motion scheme's fast spatial and fast
    // effects specs), back out as it is dismissed, drawn until it is out.
    let h: Held = Rc::default();
    let hh = h.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let hh = hh.clone();
            Column(
                modifier = Modifier.padding_sides(top = 300.dp()),
                content = move || {
                    let s = remember_tooltip_state(is_persistent = true);
                    *hh.borrow_mut() = Some(s.clone());
                    TooltipBox(
                        position_provider =
                            TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                        tooltip = |scope: &dyn TooltipScope| {
                            scope.PlainTooltip(modifier = Modifier.test_tag("kart"), content = || Text("İpucu"))
                        },
                        state = s,
                        content = || VectorIcon(Icons::Filled::Info(), content_description = Some("Bilgi".into())),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    scope_of(&rule).launch(held(&h).show(MutatePriority::Default));
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let coming = rule.draw();
    let growing = coming.transforms.iter().any(|m| m.values[0] > 0.8 && m.values[0] < 0.99);
    let fading = coming.layer_alphas.iter().any(|a| *a > 0.01 && *a < 0.99);
    assert!(growing && fading, "{:?} {:?}", coming.transforms, coming.layer_alphas);
    rule.wait_for_idle();
    let shown = rule.draw();
    assert!(
        shown.layer_alphas.iter().all(|a| *a >= 0.999) && shown.layer_transforms.iter().all(|t| t.scale_x >= 0.999)
    );
    // Out: still there while it fades, then gone.
    held(&h).dismiss();
    rule.advance_frame();
    rule.advance_frame();
    rule.on_node_with_tag("kart").assert_exists();
    assert!(held(&h).is_visible()); // (its transition still going there)
    rule.wait_for_idle();
    rule.on_node_with_tag("kart").assert_does_not_exist();
    assert!(!held(&h).is_visible());
}

fn plain_tooltips_are_sized_padded_and_styled_as_the_tokens() {
    // PlainTooltip: 40 × 24 dp at least, 200 dp wide at most (its max_width), padded 8 dp at the sides and 4 above
    // and below; the body small style in the inverse on-surface on the inverse surface, extra small corners.
    let seen = Rc::new(RefCell::new(None));
    let s = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *s.borrow_mut() = Some(MaterialTheme::color_scheme());
            Column(
                modifier = Modifier.padding_sides(top = 300.dp()),
                content = || {
                    for (tag, text, width) in [
                        ("küçük", "a", None),
                        ("uzun", "Çok uzun bir ipucu metni ki iki yüz dp'yi aşar gider", None),
                        ("dar", "Dar bir ipucu metni", Some(80.dp())),
                    ] {
                        let s = remember_tooltip_state(initial_is_visible = true, is_persistent = true);
                        TooltipBox(
                            position_provider =
                                TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Below),
                            tooltip = move |scope: &dyn TooltipScope| match width {
                                Some(w) => scope.PlainTooltip(
                                    modifier = Modifier.test_tag(tag),
                                    max_width = w,
                                    content = move || Text(text, modifier = Modifier.test_tag(format!("{tag} metin"))),
                                ),
                                None => scope.PlainTooltip(
                                    modifier = Modifier.test_tag(tag),
                                    content = move || Text(text, modifier = Modifier.test_tag(format!("{tag} metin"))),
                                ),
                            },
                            state = s,
                            content = || Box(Modifier.size(40.dp())),
                        );
                        Spacer(Modifier.height(150.dp()));
                    }
                },
            );
        }),
        IntSize::new(800, 1200),
    );
    rule.wait_for_idle();
    let colors = seen.borrow().unwrap();
    let small = bounds(&rule, "küçük");
    assert!(small.width() >= 40.0 && near(small.height(), 24.0_f32.max(small.height()), 0.0) && small.height() >= 24.0);
    let (long, long_text) = (bounds(&rule, "uzun"), bounds(&rule, "uzun metin"));
    assert!(near(long.width(), 200.0, 0.5) && near(long_text.left, long.left + 8.0, 0.5));
    assert!(near(long_text.top, long.top + 4.0, 0.5) && near(long_text.bottom, long.bottom - 4.0, 0.5));
    assert!(near(bounds(&rule, "dar").width(), 80.0, 0.5));
    let canvas = rule.draw();
    let i = canvas.texts.iter().position(|t| t == "a").expect("drawn");
    assert_eq!(canvas.colors[i], colors.inverse_on_surface);
    let typography = Typography::default();
    assert_eq!(canvas.text_styles[i].font_size, typography.body_small.font_size.value());
    let container = canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .position(|(r, c)| *c == colors.inverse_surface && near(r.left, small.left, 0.5));
    assert!(container.is_some_and(|k| near(canvas.rect_radii[k], 4.0, 0.5)));
    let _ = TextStyle::default();
}

fn tooltip_defaults_and_caret_shapes() {
    // TooltipDefaults' plain tooltip members, the caret shapes, the deprecated position providers.
    assert_eq!(TooltipDefaults::CaretSize, DpSize(16.dp(), 8.dp()));
    assert_eq!(TooltipDefaults::PlainTooltipMaxWidth, 200.dp());
    let density = Density::new(2.0, 1.0);
    let outline = TooltipDefaults::caret_shape().create_outline(Size::new(100.0, 40.0), LayoutDirection::Ltr, &density);
    // (a triangle from its top edge's middle at the origin, 16 dp wide, pointing 8 dp down)
    assert_eq!(outline.bounds(), Rect::new(-16.0, 0.0, 16.0, 16.0));
    let Outline::Generic(path) = &outline else { panic!("{outline:?}") };
    assert!(path.contains(Offset::new(0.0, 8.0)) && !path.contains(Offset::new(10.0, 12.0)));
    let wide = TooltipDefaults::caret_shape_of_size(DpSize(24.dp(), 12.dp()));
    assert_eq!(
        wide.create_outline(Size::new(1.0, 1.0), LayoutDirection::Ltr, &density).bounds(),
        Rect::new(-24.0, 0.0, 24.0, 24.0)
    );
    assert!(Arc::ptr_eq(&TooltipDefaults::caret_shape(), &TooltipDefaults::caret_shape()));
    assert!(*TooltipDefaults::caret_shape() == *(Arc::new(DefaultTooltipCaretShape::default()) as Arc<dyn Shape>));
    assert_eq!(DefaultTooltipCaretShape::new(DpSize(1.dp(), 2.dp())).caret_size, DpSize(1.dp(), 2.dp()));
    let seen = Rc::new(RefCell::new(None));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        #[allow(deprecated)]
        let plain = TooltipDefaults.remember_plain_tooltip_position_provider();
        *s.borrow_mut() = Some((
            MaterialTheme::color_scheme(),
            TooltipDefaults::plain_tooltip_container_shape(),
            TooltipDefaults::plain_tooltip_container_color(),
            TooltipDefaults::plain_tooltip_content_color(),
            MaterialTheme::shapes().extra_small,
            plain,
        ));
    }));
    rule.wait_for_idle();
    let (c, shape, container, content, extra_small, plain) = seen.borrow().clone().unwrap();
    let extra_small: Arc<dyn Shape> = Arc::new(extra_small);
    assert!(*shape == *extra_small);
    assert_eq!((container, content), (c.inverse_surface, c.inverse_on_surface));
    // The deprecated plain one: above, centered even past the window's sides; below without room above.
    let place = |provider: &Rc<dyn PopupPositionProvider>, anchor: IntRect| {
        provider.calculate_position(anchor, IntSize::new(400, 800), LayoutDirection::Ltr, IntSize::new(100, 30))
    };
    assert_eq!(place(&plain, IntRect::new(0, 300, 40, 340)), IntOffset::new(-30, 266));
    assert_eq!(place(&plain, IntRect::new(100, 10, 140, 50)), IntOffset::new(70, 54));
}

/// One of the plain tooltip samples: a favorite icon button with a plain tooltip at `position`, its caret
/// `caret` (`PlainTooltipSample`: none).
#[kompozable]
fn PlainTooltipSampleOf(position: TooltipAnchorPosition, caret: Option<Arc<dyn Shape>>) {
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(position),
        tooltip = move |scope: &dyn TooltipScope| {
            scope.PlainTooltip(caret_shape = caret.clone(), content = || Text("Add to favorites"))
        },
        state = remember_tooltip_state(),
        content = || {
            IconButton(
                on_click = || { /* Icon button's click event */ },
                content = || {
                    VectorIcon(Icons::Filled::Favorite(), content_description = Some("Localized Description".into()))
                },
            );
        },
    );
}

/// `PlainTooltipWithManualInvocationSample`.
#[kompozable]
fn PlainTooltipWithManualInvocationSample() {
    let tooltip_state = remember_tooltip_state();
    let scope = kompoz_coroutines::remember_coroutine_scope();
    let state = tooltip_state.clone();
    Column(
        horizontal_alignment = Alignment::CenterHorizontally,
        content = || {
            TooltipBox(
                position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                tooltip = |scope: &dyn TooltipScope| scope.PlainTooltip(content = || Text("Add to list")),
                state = tooltip_state.clone(),
                content = || {
                    IconButton(
                        on_click = || { /* Icon button's click event */ },
                        content = || {
                            VectorIcon(
                                Icons::Filled::AddCircle(),
                                content_description = Some("Localized Description".into()),
                            )
                        },
                    );
                },
            );
            Spacer(Modifier.required_height(30.dp()));
            OutlinedButton(
                on_click = move || {
                    scope.launch(state.show(MutatePriority::Default));
                },
                content = || Text("Display tooltip"),
            );
        },
    );
}

fn plain_tooltip_samples() {
    // M15's TooltipSamples.kt, the plain tooltips: each anchor a 48 dp icon button in the middle of the window, its
    // tooltip shown on a long press on the side its sample says, the caret (where it has one) pointing at it.
    let sample = mutable_state_of(0usize);
    let chosen = sample.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let sample = chosen.clone();
            Box(
                modifier = Modifier.fill_max_size(),
                content_alignment = Alignment::Center,
                content = move || {
                    let caret = Some(TooltipDefaults::caret_shape());
                    match sample.get() {
                        0 => PlainTooltipSampleOf(TooltipAnchorPosition::Above, None), // PlainTooltipSample
                        1 => PlainTooltipSampleOf(TooltipAnchorPosition::Above, caret), // PlainTooltipWithCaret
                        2 => PlainTooltipSampleOf(TooltipAnchorPosition::Below, caret), // …BelowAnchor
                        3 => PlainTooltipSampleOf(TooltipAnchorPosition::Left, caret), // …LeftOfAnchor
                        4 => PlainTooltipSampleOf(TooltipAnchorPosition::Right, caret), // …RightOfAnchor
                        5 => PlainTooltipSampleOf(TooltipAnchorPosition::Start, caret), // …StartOfAnchor
                        6 => PlainTooltipSampleOf(TooltipAnchorPosition::End, caret),  // …EndOfAnchor
                        7 => PlainTooltipSampleOf(
                            TooltipAnchorPosition::Above,
                            Some(TooltipDefaults::caret_shape_of_size(DpSize(24.dp(), 12.dp()))),
                        ), // PlainTooltipWithCustomCaret
                        _ => PlainTooltipWithManualInvocationSample(),
                    }
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let tooltip_path = |rule: &KompozTestRule| {
        let canvas = rule.draw();
        let color = canvas.path_colors.iter().copied().find(|c| c.alpha() > 0.9).expect("a path");
        path_of(&canvas, color).map(|(b, _, _)| b).unwrap()
    };
    // (where the tooltip went and its path's bounds, caret included)
    let shown = |i: usize| {
        sample.set(i);
        rule.wait_for_idle();
        let anchor = rule.on_node_with_content_description("Localized Description").get_unclipped_bounds_in_root();
        rule.host().pointer_down(anchor.center(), -1, 0, PointerType::Touch);
        rule.advance_time_by(600);
        rule.host().pointer_up(anchor.center(), -1, 0);
        rule.advance_time_by(300);
        let tip = text_bounds(&rule, "Add to favorites");
        (anchor, tip)
    };
    let (anchor, tip) = shown(0);
    assert!(tip.bottom < anchor.top && near(tip.center().x, anchor.center().x, 1.0));
    rule.wait_for_idle();
    let (anchor, tip) = shown(1);
    let path = tooltip_path(&rule);
    assert!(tip.bottom < anchor.top && path.bottom > tip.bottom + 4.0 && near(path.center().x, anchor.center().x, 1.0));
    rule.wait_for_idle();
    let (anchor, tip) = shown(2);
    assert!(tip.top > anchor.bottom && tooltip_path(&rule).top < tip.top - 4.0);
    rule.wait_for_idle();
    for (i, left) in [(3, true), (4, false), (5, true), (6, false)] {
        let (anchor, tip) = shown(i);
        let path = tooltip_path(&rule);
        if left {
            assert!(tip.right < anchor.left && path.right > tip.right + 4.0, "{i}: {tip:?} {path:?} {anchor:?}");
        } else {
            assert!(tip.left > anchor.right && path.left < tip.left - 4.0, "{i}: {tip:?} {path:?} {anchor:?}");
        }
        rule.wait_for_idle();
    }
    let (_, tip) = shown(7);
    assert!(tooltip_path(&rule).bottom > tip.bottom + 10.0); // (a 12 dp caret)
    rule.wait_for_idle();
    // The manual one: its button shows it.
    sample.set(8);
    rule.wait_for_idle();
    rule.on_node_with_text("Add to list").assert_does_not_exist();
    // (a click of the pointer: `perform_click` waits until idle, the tooltip's 1.5 s gone with it)
    let button = text_bounds(&rule, "Display tooltip");
    rule.host().pointer_down(button.center(), -1, 0, PointerType::Touch);
    rule.host().pointer_up(button.center(), -1, 0);
    rule.advance_time_by(100);
    rule.on_node_with_text("Add to list").assert_exists();
    rule.advance_time_by(1600);
    rule.wait_for_idle();
    rule.on_node_with_text("Add to list").assert_does_not_exist();
}

//! navigation-compose's NavHost as Compose's behaves (2.10): the C++ Kompoz's NavHost, navigation and ViewModel tests
//! by name (kompoz_tests.cpp), each whole — their controller parts with the NavHost showing the screens —, where
//! Compose's behaviour differs from the C++'s said at each; and Compose's own: dialog destinations, transitions of
//! destinations and graphs, the entry's owners, the controller saved with the composition.

#![allow(non_snake_case)]

use kompoz_animation::*;
use kompoz_coroutines::{AsFlow, CoroutineScope, MutableStateFlow, delay_ms};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_lifecycle::*;
use kompoz_navigation::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::input::{Key, KeyModifiers};
use kompoz_ui::platform::PlatformCallbacks;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

// ---- helpers ------------------------------------------------------------------------------------------------------

/// The controller a test's NavHost was given (`controller = nav` in the C++ tests).
type Held = Rc<RefCell<Option<NavController>>>;

/// Waits for idle with the clock advancing on its own (Compose's `mainClock.autoAdvance = true; waitForIdle()`):
/// what a test stepping the clock by hand does to let the animations run their course; its `auto_advance` put
/// back after.
fn finish_animations(rule: &KompozTestRule) {
    let clock = rule.main_clock();
    let was = clock.auto_advance();
    clock.set_auto_advance(true);
    rule.wait_for_idle();
    clock.set_auto_advance(was);
}

fn held() -> Held {
    Rc::new(RefCell::new(None))
}

fn nav_of(h: &Held) -> NavController {
    h.borrow().clone().expect("a NavHost composed")
}

/// The back stack as the C++ tests write it: routes, a graph's in brackets, the root graph left out.
fn stack_of(nav: &NavController) -> String {
    let root = nav.graph().as_destination();
    nav.back_queue()
        .iter()
        .filter(|e| !e.destination().same(&root))
        .map(|e| {
            let d = e.destination();
            let route = d.route().unwrap_or_default();
            if d.is_graph() { format!("[{route}]") } else { route }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn route_of(entry: Option<NavBackStackEntry>) -> String {
    entry.and_then(|e| e.destination().route()).unwrap_or_default()
}

fn string_arg(entry: &NavBackStackEntry, name: &str) -> Option<String> {
    entry.arguments().and_then(|a| a.get_string(name))
}

fn int_arg(name: &str) -> NamedNavArgument {
    nav_argument(name, |a| a.set_type(NavType::IntType))
}

fn escape(rule: &KompozTestRule) -> bool {
    rule.host().key_down(Key::Escape, KeyModifiers::default())
}

fn snap_fade() -> (Rc<dyn Fn(&NavTransitionScope) -> EnterTransition>, Rc<dyn Fn(&NavTransitionScope) -> ExitTransition>)
{
    (
        Rc::new(|_: &NavTransitionScope| named_args!(fade_in(animation_spec = Arc::new(snap::<f32>())))),
        Rc::new(|_: &NavTransitionScope| named_args!(fade_out(animation_spec = Arc::new(snap::<f32>())))),
    )
}

/// What `f` panics with.
fn panics(f: impl FnOnce()) -> String {
    let e = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).expect_err("it panics");
    e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
}

// ---- C++ nav_host_goes_to_routes_with_arguments_and_back ----------------------------------------------------------

/// Routes with arguments (an Int one declared so: Compose's arguments are strings unless typed), the optional query
/// parameter, back by Escape, `launchSingleTop`, `popUpTo`. Compose's: navigating to a route no destination answers
/// panics (IllegalArgumentException; the C++ ignored it); popping the last entry empties the stack (the C++ refused).
fn nav_host_goes_to_routes_with_arguments_and_back() {
    let h = held();
    let hc = h.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let nav = remember_nav_controller();
        *hc.borrow_mut() = Some(nav.clone());
        NavHost(
            nav,
            "home",
            modifier = Modifier.fill_max_size(),
            builder = |graph: &mut NavGraphBuilder| {
                graph.kompozable("home", |_, _| Text("ana sayfa"));
                graph.kompozable(
                    "detail/{id}?tab={tab}",
                    arguments = vec![int_arg("id")],
                    content = |_, e: NavBackStackEntry| {
                        let id = e.arguments().map_or(-1, |a| a.get_int("id"));
                        Text(format!("öğe {id} sekme {}", string_arg(&e, "tab").unwrap_or_else(|| "yok".into())));
                    },
                );
                graph.kompozable("settings", |_, _| Text("ayarlar"));
            },
        );
    }));
    let nav = nav_of(&h);
    assert!(rule.on_node_with_text("ana sayfa").exists() && nav.back_queue().len() == 2); // (the graph's entry too)
    assert!(nav.previous_back_stack_entry().is_none());
    nav.navigate("detail/42?tab=bilgi");
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("öğe 42 sekme bilgi").exists() && !rule.on_node_with_text("ana sayfa").exists());
    assert_eq!(route_of(nav.current_back_stack_entry()), "detail/{id}?tab={tab}");
    assert_eq!(route_of(nav.previous_back_stack_entry()), "home");
    nav.navigate("detail/7"); // the query parameter is optional
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("öğe 7 sekme yok").exists() && stack_of(&nav).split(' ').count() == 3);
    // (no such route: Compose throws)
    assert!(panics(|| nav.navigate("nowhere/1")).contains("cannot be found in the navigation graph"));
    // Back (Escape on the desktop) pops; at the start destination it is the platform's.
    assert!(escape(&rule));
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("öğe 42 sekme bilgi").exists());
    assert!(nav.pop_back_stack());
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("ana sayfa").exists());
    assert!(!escape(&rule)); // nothing to go back to: unused
    // launchSingleTop doesn't stack a second copy; popUpTo clears down to a route.
    nav.navigate("settings");
    nav.navigate_with("settings", |o| o.set_launch_single_top(true));
    assert_eq!(stack_of(&nav), "home settings");
    nav.navigate("detail/1");
    nav.navigate("detail/2");
    nav.navigate_with("settings", |o| o.pop_up_to("home", |_| {}));
    rule.wait_for_idle();
    assert!(stack_of(&nav) == "home settings" && rule.on_node_with_text("ayarlar").exists());
    nav.navigate_with("detail/3", |o| o.pop_up_to("home", |p| p.set_inclusive(true)));
    assert_eq!(stack_of(&nav), "detail/{id}?tab={tab}");
    // (down to the top entry itself: nothing popped — Compose says false; the C++ said true)
    assert!(!nav.pop_back_stack_to("detail/{id}?tab={tab}", false, false) && stack_of(&nav) == "detail/{id}?tab={tab}");
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("öğe 3 sekme yok").exists());
}

// ---- C++ nav_host_brings_entries_in_over_going_forward_and_under_going_back ---------------------------------------

/// The screen drawn last among `ekran …`, while both cross-fade.
fn top_screen(rule: &KompozTestRule) -> String {
    rule.draw().texts.into_iter().rfind(|t| t.starts_with("ekran")).unwrap_or_default()
}

fn abc(graph: &mut NavGraphBuilder) {
    for route in ["a", "b", "c"] {
        kompozable_block!({
            graph.kompozable(route, move |_, _| Text(format!("ekran {route}")));
        });
    }
}

/// Compose's z-indices: forward, the new entry one above the one before; back, one below.
fn nav_host_brings_entries_in_over_going_forward_and_under_going_back() {
    let h = held();
    let hc = h.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let nav = remember_nav_controller();
        *hc.borrow_mut() = Some(nav.clone());
        NavHost(nav, "a", modifier = Modifier.fill_max_size(), builder = abc);
    }));
    let nav = nav_of(&h);
    // (the clock by hand: the checks on the way see the screens crossing)
    rule.main_clock().set_auto_advance(false);
    nav.navigate("b");
    rule.advance_time_by(300);
    assert_eq!(top_screen(&rule), "ekran b"); // forward: over the one before
    finish_animations(&rule);
    nav.navigate("c");
    finish_animations(&rule);
    nav.pop_back_stack();
    rule.advance_time_by(300);
    assert!(rule.on_node_with_text("ekran b").exists() && top_screen(&rule) == "ekran c"); // back: b comes in under c
    finish_animations(&rule);
    nav.pop_back_stack();
    rule.advance_time_by(300);
    assert_eq!(top_screen(&rule), "ekran b"); // and a under b (one less again, not level with it)
}

// ---- C++ nav_host_seeks_its_pop_transition_with_the_back_gesture --------------------------------------------------

/// What is drawn a frame on: the screens bottom to top, the faded layers' alphas sorted.
fn drawn(rule: &KompozTestRule) -> (Vec<String>, Vec<f32>) {
    rule.advance_frame();
    let canvas = rule.draw();
    let screens = canvas.texts.iter().filter(|t| t.starts_with("ekran")).cloned().collect();
    let mut alphas: Vec<f32> = canvas.layer_alphas.iter().copied().filter(|a| *a < 1.0).collect();
    alphas.sort_by(f32::total_cmp);
    (screens, alphas)
}

fn same(a: &[f32], b: &[f32]) -> bool {
    let mut b = b.to_vec();
    b.sort_by(f32::total_cmp);
    a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 0.01)
}

fn gesture(progress: f32) -> kompoz_ui::back::BackEventCompat {
    kompoz_ui::back::BackEventCompat { touch_x: 60.0, touch_y: 400.0, progress, ..Default::default() }
}

fn fosi(f: f32) -> f32 {
    FastOutSlowInEasing.transform(f)
}

/// The predictive back gesture seeks the pop transition (Compose's NavHost: its predictive pop transitions, given
/// here as the pop ones, the C++'s single pop transition) as far as it has gone, the one coming back under the one
/// leaving; called off, the top one alone again; done, it pops and the transition goes on from there; a press pops
/// from the transition's start; gone elsewhere during a gesture, the gesture goes with the screen it showed.
fn nav_host_seeks_its_pop_transition_with_the_back_gesture() {
    let h = held();
    let hc = h.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let nav = remember_nav_controller();
        *hc.borrow_mut() = Some(nav.clone());
        // Coming back: in over 300 ms (eased), the one leaving out in the first 150. Going on: at once.
        let pop_enter = |_: &NavTransitionScope| {
            let spec: TweenSpec<f32> = named_args!(tween(300));
            named_args!(fade_in(animation_spec = Arc::new(spec)))
        };
        let pop_exit = |_: &NavTransitionScope| {
            let spec: TweenSpec<f32> = named_args!(tween(150, easing = Arc::new(LinearEasing)));
            named_args!(fade_out(animation_spec = Arc::new(spec)))
        };
        NavHost(
            nav,
            "a",
            modifier = Modifier.fill_max_size(),
            enter_transition = |_: &NavTransitionScope| EnterTransition::None,
            exit_transition = |_: &NavTransitionScope| ExitTransition::None,
            pop_enter_transition = Some(Rc::new(pop_enter)),
            pop_exit_transition = Some(Rc::new(pop_exit)),
            predictive_pop_enter_transition = move |s: &NavTransitionScope, _: i32| pop_enter(s),
            predictive_pop_exit_transition = move |s: &NavTransitionScope, _: i32| pop_exit(s),
            builder = abc,
        );
    }));
    let nav = nav_of(&h);
    let host = rule.host();
    assert!(!host.wants_back()); // at the start: back leaves the app
    nav.navigate("b");
    rule.wait_for_idle();
    // A quarter of the way: 75 ms into the pop transition — a in as its easing has it at 75 of 300, b half out —, a
    // under b.
    host.back_started(gesture(0.0));
    host.back_progressed(gesture(0.25));
    // (Compose's: the seek waits for the composition of its target — a frame later than the C++'s)
    drawn(&rule);
    let (screens, alphas) = drawn(&rule);
    assert_eq!(screens, ["ekran a", "ekran b"]);
    assert!(same(&alphas, &[fosi(0.25), 0.5]), "{alphas:?}");
    assert_eq!(route_of(nav.current_back_stack_entry()), "b");
    // Called off: b alone again, whole.
    host.back_cancelled();
    rule.wait_for_idle();
    let (screens, alphas) = drawn(&rule);
    assert!(screens == ["ekran b"] && alphas.is_empty(), "{screens:?} {alphas:?}");
    assert!(!rule.on_node_with_text("ekran a").exists());
    // Done: it pops, and the transition goes on from 75 ms: 75 more (and a frame) and b is gone. (Compose's: it goes
    // on two frames later — the popped stack composed, then its first frame —, still from 75 ms.)
    host.back_started(gesture(0.0));
    host.back_progressed(gesture(0.25));
    drawn(&rule);
    drawn(&rule);
    assert!(host.back_invoked());
    drawn(&rule);
    let (_, alphas) = drawn(&rule);
    assert!(alphas.contains(&fosi(0.25)) || alphas.iter().any(|a| (a - fosi(0.25)).abs() < 0.01), "{alphas:?}");
    rule.advance_time_by(75);
    let (screens, alphas) = drawn(&rule);
    assert!(route_of(nav.current_back_stack_entry()) == "a" && screens == ["ekran a"], "{screens:?}");
    assert!(
        alphas.len() == 1 && alphas[0] > fosi(150.0 / 300.0) && alphas[0] < fosi(185.0 / 300.0),
        "{alphas:?} {}",
        fosi(155.0 / 300.0)
    );
    rule.wait_for_idle();
    assert!(drawn(&rule).1.is_empty());
    // A press pops as before, from the start of the transition (a frame on: 80 ms).
    nav.navigate("b");
    rule.wait_for_idle();
    assert!(escape(&rule));
    rule.advance_time_by(75);
    let (screens, alphas) = drawn(&rule);
    // (the same time in both: b's linear fade says how far — Compose's starts a frame or two after the press,
    // the composition of the popped stack first: about 64 ms here, the C++'s 80)
    let leaving = alphas.iter().copied().fold(0.0, f32::max);
    let t = (1.0 - leaving) * 150.0;
    assert!(screens.len() == 2 && (55.0..=85.0).contains(&t), "{screens:?} {alphas:?}");
    assert!(same(&alphas, &[fosi(t / 300.0), leaving]), "{alphas:?} at {t} ms");
    rule.wait_for_idle();
    // Gone somewhere else during a gesture: the screen it showed goes with it.
    nav.navigate("b");
    rule.wait_for_idle();
    host.back_started(gesture(0.0));
    host.back_progressed(gesture(0.25));
    drawn(&rule);
    nav.navigate("c");
    rule.wait_for_idle();
    host.back_cancelled();
    rule.wait_for_idle();
    assert_eq!(drawn(&rule).0, ["ekran c"]);
    // From c, done: b, and no gesture left to show a below it.
    host.back_started(gesture(0.0));
    host.back_progressed(gesture(0.25));
    drawn(&rule);
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert_eq!(drawn(&rule).0, ["ekran b"]);
    // The top one replaced during a gesture, the one below the same.
    host.back_started(gesture(0.0));
    host.back_progressed(gesture(0.25));
    drawn(&rule);
    nav.navigate_with("c", |o| o.pop_up_to("b", |p| p.set_inclusive(true)));
    rule.wait_for_idle();
    host.back_progressed(gesture(0.5));
    drawn(&rule);
    assert!(host.back_invoked());
    rule.wait_for_idle();
    // (Compose's handler pops the entry its gesture started on — b, gone: the controller ignores it —; the C++ popped
    // the one on top then)
    assert!(stack_of(&nav) == "a c" && drawn(&rule).0 == ["ekran c"]);
    assert!(escape(&rule)); // the next back pops the one on top
    rule.wait_for_idle();
    assert!(route_of(nav.current_back_stack_entry()) == "a" && drawn(&rule).0 == ["ekran a"]);
}

// ---- C++ nav_host_keeps_saveable_state_on_the_back_stack_and_in_saved_tabs ----------------------------------------

/// A screen with state that survives (rememberSaveable, the scroll position) and state that doesn't.
#[kompozable]
fn SaveableScreen(name: &'static str) {
    let saved = remember_saveable(|| mutable_state_of(0));
    let plain = remember(|| mutable_state_of(0));
    let scroll = remember_scroll_state();
    let (s, p) = (saved.clone(), plain.clone());
    Column(
        modifier = Modifier.fill_max_size().vertical_scroll(&scroll).test_tag(name),
        content = move || {
            Text(format!("{name} {} {}", s.get(), p.get()));
            let (s, p) = (s.clone(), p.clone());
            Box(
                modifier = Modifier
                    .clickable(move || {
                        s.set(s.peek() + 1);
                        p.set(p.peek() + 1);
                    })
                    .test_tag(format!("{name}+")),
                content = || Text("artır"),
            );
            Box(modifier = Modifier.height(3000.dp()));
        },
    );
}

/// rememberSaveable comes back with its entry, remember doesn't; tabs keep their stacks, saved while another is
/// shown (Compose: the saved entries destroyed, their state saved, entries of the same ids made again — D10 —); an
/// entry popped for good starts afresh.
fn nav_host_keeps_saveable_state_on_the_back_stack_and_in_saved_tabs() {
    let h = held();
    let hc = h.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let nav = remember_nav_controller();
        *hc.borrow_mut() = Some(nav.clone());
        let (enter, exit) = snap_fade();
        let (e, x) = (enter.clone(), exit.clone());
        NavHost(
            nav,
            "a",
            modifier = Modifier.fill_max_size(),
            enter_transition = move |s: &NavTransitionScope| e(s),
            exit_transition = move |s: &NavTransitionScope| x(s),
            pop_enter_transition = Some(enter),
            pop_exit_transition = Some(exit),
            builder = |graph: &mut NavGraphBuilder| {
                graph.kompozable("a", |_, _| SaveableScreen("a"));
                graph.kompozable("b", |_, _| SaveableScreen("b"));
                graph.kompozable("b/detail/{n}", |_, _| SaveableScreen("detay"));
            },
        );
    }));
    let nav = nav_of(&h);
    let tap = |tag: &str, times: usize| {
        for _ in 0..times {
            rule.on_node_with_tag(tag).perform_click();
        }
    };
    tap("a+", 2);
    assert!(rule.on_node_with_text("a 2 2").exists());
    nav.navigate("b/detail/1");
    rule.wait_for_idle();
    nav.pop_back_stack();
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("a 2 0").exists()); // rememberSaveable came back, remember didn't
    // Tabs: each keeps its own stack, saved while another is shown.
    let tab = |route: &str| {
        nav.navigate_with(route, |o| {
            o.pop_up_to("a", |p| p.set_save_state(true));
            o.set_launch_single_top(true);
            o.set_restore_state(true);
        });
        rule.wait_for_idle();
    };
    tab("b");
    nav.navigate("b/detail/7");
    rule.wait_for_idle();
    tap("detay+", 3);
    assert!(rule.on_node_with_text("detay 3 3").exists() && stack_of(&nav) == "a b b/detail/{n}");
    let detail_id = nav.current_back_stack_entry().unwrap().id();
    tab("a");
    assert!(stack_of(&nav) == "a" && rule.on_node_with_text("a 2 0").exists());
    tab("b"); // back where it was: the detail, its count
    assert!(stack_of(&nav) == "a b b/detail/{n}" && rule.on_node_with_text("detay 3 0").exists());
    assert_eq!(nav.current_back_stack_entry().unwrap().id(), detail_id); // (an entry of the same id)
    nav.pop_back_stack();
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("b 0 0").exists());
    // Popped for good (not saved): a fresh start next time.
    nav.navigate("b/detail/8");
    rule.wait_for_idle();
    tap("detay+", 1);
    nav.pop_back_stack();
    nav.navigate("b/detail/8");
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("detay 0 0").exists());
}

// ---- C++ saveable_scroll_position_comes_back_with_its_entry -------------------------------------------------------

fn saveable_scroll_position_comes_back_with_its_entry() {
    let nav = Rc::new(RefCell::new(None::<NavController>));
    let seen = Rc::new(RefCell::new(None::<ScrollState>));
    let (n, s) = (nav.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let controller = remember_nav_controller();
        *n.borrow_mut() = Some(controller.clone());
        let (enter, exit) = snap_fade();
        let (e, x) = (enter.clone(), exit.clone());
        let s = s.clone();
        NavHost(
            controller,
            "list",
            modifier = Modifier.fill_max_size(),
            enter_transition = move |t: &NavTransitionScope| e(t),
            exit_transition = move |t: &NavTransitionScope| x(t),
            pop_enter_transition = Some(enter),
            pop_exit_transition = Some(exit),
            builder = move |graph: &mut NavGraphBuilder| {
                let s = s.clone();
                graph.kompozable("list", move |_, _| {
                    let scroll = remember_scroll_state();
                    *s.borrow_mut() = Some(scroll.clone());
                    Column(
                        modifier = Modifier.fill_max_size().vertical_scroll(&scroll),
                        content = || Box(modifier = Modifier.height(5000.dp())),
                    );
                });
                graph.kompozable("other", |_, _| Text("öteki"));
            },
        );
    }));
    let scroll = seen.borrow().clone().unwrap();
    CoroutineScope::new(rule.host().dispatcher()).launch(async move {
        scroll.scroll_to(1234).await;
    });
    rule.wait_for_idle();
    let controller = nav.borrow().clone().unwrap();
    controller.navigate("other");
    rule.wait_for_idle();
    controller.pop_back_stack();
    rule.wait_for_idle();
    assert_eq!(seen.borrow().as_ref().unwrap().value(), 1234);
}

// ---- The C++ ViewModels ----------------------------------------------------------------------------------------------

thread_local! {
    static MADE: Cell<i32> = const { Cell::new(0) };
    static CLEARED: Cell<i32> = const { Cell::new(0) };
    static TICKS: Cell<i32> = const { Cell::new(0) };
}

fn made() -> i32 {
    MADE.with(Cell::get)
}
fn cleared() -> i32 {
    CLEARED.with(Cell::get)
}
fn ticks() -> i32 {
    TICKS.with(Cell::get)
}
fn reset_counts() {
    MADE.with(|c| c.set(0));
    CLEARED.with(|c| c.set(0));
    TICKS.with(|c| c.set(0));
}

/// A ViewModel that counts how many were made and cleared, and runs a coroutine in its scope (C++ `TestViewModel`).
struct TestViewModel {
    base: ViewModelImpl,
    count: MutableStateFlow<i32>,
}

impl TestViewModel {
    fn start(&self) {
        self.view_model_scope().launch(async {
            loop {
                delay_ms(100).await;
                TICKS.with(|c| c.set(c.get() + 1));
            }
        });
    }
}

impl ViewModel for TestViewModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
    fn on_cleared(&self) {
        CLEARED.with(|c| c.set(c.get() + 1));
    }
    fn new_instance(_: &CreationExtras) -> Option<Self> {
        MADE.with(|c| c.set(c.get() + 1));
        Some(TestViewModel { base: ViewModelImpl::new(), count: MutableStateFlow::new(0) })
    }
}

/// A ViewModel sharing a flow in its scope while collected (C++ `SharingModel`).
struct SharingModel {
    base: ViewModelImpl,
    shared: kompoz_coroutines::StateFlow<i32>,
}

impl ViewModel for SharingModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }
    fn new_instance(_: &CreationExtras) -> Option<Self> {
        let base = ViewModelImpl::new();
        let source = MutableStateFlow::new(0);
        let started = kompoz_coroutines::SharingStarted::WhileSubscribed {
            stop_timeout: std::time::Duration::from_millis(5000),
            replay_expiration: std::time::Duration::MAX,
        };
        let shared = source.as_flow().state_in(&base.view_model_scope(), started, 0);
        Some(SharingModel { base, shared })
    }
}

#[kompozable]
fn CounterView(count: MutableState<i32>) {
    Text(format!("count={}", count.get()));
}

// ---- C++ view_models_of_navigation_entries_go_before_their_host ---------------------------------------------------

/// An entry's ViewModel (and the coroutines of its scope) go as the host goes, while its dispatcher is still there.
fn view_models_of_navigation_entries_go_before_their_host() {
    let count = mutable_state_of(0);
    let weak = Rc::new(RefCell::new(std::rc::Weak::<SharingModel>::new()));
    {
        let (c, w) = (count.clone(), weak.clone());
        let rule = KompozTestRule::new(kompozable_lambda!(move || {
            let nav = remember_nav_controller();
            let (c, w) = (c.clone(), w.clone());
            NavHost(
                nav,
                "a",
                modifier = Modifier.fill_max_size(),
                builder = move |graph: &mut NavGraphBuilder| {
                    let (c, w) = (c.clone(), w.clone());
                    graph.kompozable("a", move |_, _| {
                        let vm: Rc<SharingModel> = view_model();
                        *w.borrow_mut() = Rc::downgrade(&vm);
                        Text(format!("a {}", vm.shared.collect_as_state().get()));
                        CounterView(c.clone());
                    });
                },
            );
        }));
        count.set(1);
        rule.wait_for_idle();
        assert!(rule.on_node_with_text("count=1").exists() && weak.borrow().upgrade().is_some());
    }
    assert!(weak.borrow().upgrade().is_none());
}

// ---- C++ view_models_live_as_long_as_their_navigation_entry -------------------------------------------------------

fn fade_100() -> (Rc<dyn Fn(&NavTransitionScope) -> EnterTransition>, Rc<dyn Fn(&NavTransitionScope) -> ExitTransition>)
{
    (
        Rc::new(|_: &NavTransitionScope| {
            let spec: TweenSpec<f32> = named_args!(tween(100));
            named_args!(fade_in(animation_spec = Arc::new(spec)))
        }),
        Rc::new(|_: &NavTransitionScope| {
            let spec: TweenSpec<f32> = named_args!(tween(100));
            named_args!(fade_out(animation_spec = Arc::new(spec)))
        }),
    )
}

/// An entry's ViewModel lives while the entry is on the back stack, cleared (its scope cancelled) once the entry
/// is popped and gone from the screen; one outside the NavHost is the app's.
fn view_models_live_as_long_as_their_navigation_entry() {
    reset_counts();
    let h = held();
    let app_level = Rc::new(RefCell::new(None::<Rc<TestViewModel>>));
    let seen = Rc::new(RefCell::new(None::<Rc<TestViewModel>>));
    let (hc, a, s) = (h.clone(), app_level.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *a.borrow_mut() = Some(view_model(key = Some("uygulama".to_string()))); // outside the NavHost: the app's
        let nav = remember_nav_controller();
        *hc.borrow_mut() = Some(nav.clone());
        let (enter, exit) = fade_100();
        let (e, x) = (enter.clone(), exit.clone());
        let s = s.clone();
        NavHost(
            nav,
            "a",
            modifier = Modifier.fill_max_size(),
            enter_transition = move |t: &NavTransitionScope| e(t),
            exit_transition = move |t: &NavTransitionScope| x(t),
            pop_enter_transition = Some(enter),
            pop_exit_transition = Some(exit),
            builder = move |graph: &mut NavGraphBuilder| {
                graph.kompozable("a", |_, _| Text("a"));
                let s = s.clone();
                graph.kompozable("b/{n}", move |_, _| {
                    let vm: Rc<TestViewModel> = view_model();
                    *s.borrow_mut() = Some(vm.clone());
                    Text(format!("b {}", vm.count.collect_as_state().get()));
                });
            },
        );
    }));
    let nav = nav_of(&h);
    let first = app_level.borrow().clone().unwrap();
    rule.wait_for_idle();
    assert!(Rc::ptr_eq(app_level.borrow().as_ref().unwrap(), &first) && made() == 1); // the same one, composition after composition
    nav.navigate("b/1");
    rule.wait_for_idle();
    let b1 = seen.borrow().clone().unwrap();
    b1.count.set_value(5);
    b1.start();
    rule.advance_time_by(350);
    assert!(rule.on_node_with_text("b 5").exists() && made() == 2 && ticks() >= 3);
    nav.navigate("b/2"); // another entry, another ViewModel; the first one waits on the back stack
    rule.wait_for_idle();
    assert!(!Rc::ptr_eq(seen.borrow().as_ref().unwrap(), &b1) && made() == 3 && cleared() == 0);
    assert!(rule.on_node_with_text("b 0").exists());
    nav.pop_back_stack();
    rule.advance_frame();
    assert_eq!(cleared(), 0); // still on the screen, leaving
    rule.wait_for_idle();
    assert!(
        Rc::ptr_eq(seen.borrow().as_ref().unwrap(), &b1) && cleared() == 1 && rule.on_node_with_text("b 5").exists()
    );
    nav.pop_back_stack();
    rule.wait_for_idle();
    let t = ticks();
    rule.advance_time_by(500);
    assert!(cleared() == 2 && ticks() == t); // its scope was cancelled with it
    assert_eq!(made(), 3); // the app's one is still the first
}

// ---- C++ nested_navigation_graphs_are_entered_by_route_and_left_with_their_screens ---------------------------------

fn hierarchy_routes(entry: &NavBackStackEntry, root: &NavDestination) -> Vec<String> {
    entry.destination().hierarchy().iter().filter(|d| !d.same(root)).map(|d| d.route().unwrap_or_default()).collect()
}

/// Nested graphs entered by their route (their start destination, below it the graph's entry), left with their last
/// screen; `popUpTo` a graph; a graph's route from inside it is another one. (Compose's: popping the last destination
/// empties the stack, as above.)
fn nested_navigation_graphs_are_entered_by_route_and_left_with_their_screens() {
    let h = held();
    let hc = h.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let nav = remember_nav_controller();
        *hc.borrow_mut() = Some(nav.clone());
        NavHost(
            nav,
            "home",
            modifier = Modifier.fill_max_size(),
            builder = |graph: &mut NavGraphBuilder| {
                graph.kompozable("home", |_, _| Text("ana"));
                graph.navigation("username", "login", |login: &mut NavGraphBuilder| {
                    login.kompozable("username", |_, _| Text("kullanıcı"));
                    login.kompozable("password", |_, _| Text("parola"));
                    // a graph in a graph
                    login.navigation("code", "verify", |verify: &mut NavGraphBuilder| {
                        verify.kompozable("code", |_, _| Text("kod"));
                    });
                });
                graph.kompozable("profile", |_, _| Text("profil"));
            },
        );
    }));
    let nav = nav_of(&h);
    let root = nav.graph().as_destination();
    nav.navigate("login"); // its start destination, in an entry of the graph
    rule.wait_for_idle();
    assert!(stack_of(&nav) == "home [login] username" && rule.on_node_with_text("kullanıcı").exists());
    assert_eq!(hierarchy_routes(&nav.current_back_stack_entry().unwrap(), &root), ["username", "login"]);
    assert_eq!(route_of(nav.previous_back_stack_entry()), "home"); // the screen below, not the graph
    nav.navigate("password"); // in the same graph: in the same entry
    rule.wait_for_idle();
    assert!(stack_of(&nav) == "home [login] username password" && rule.on_node_with_text("parola").exists());
    assert!(
        route_of(nav.previous_back_stack_entry()) == "username"
            && nav.get_back_stack_entry("login").destination().is_graph()
    );
    nav.navigate("verify");
    rule.wait_for_idle();
    assert_eq!(stack_of(&nav), "home [login] username password [verify] code");
    assert_eq!(hierarchy_routes(&nav.current_back_stack_entry().unwrap(), &root), ["code", "verify", "login"]);
    assert!(escape(&rule)); // a graph's last screen: the graph goes with it
    rule.wait_for_idle();
    assert!(stack_of(&nav) == "home [login] username password" && rule.on_node_with_text("parola").exists());
    nav.navigate_with("profile", |o| o.pop_up_to("login", |p| p.set_inclusive(true))); // the whole flow off
    rule.wait_for_idle();
    assert!(stack_of(&nav) == "home profile" && rule.on_node_with_text("profil").exists());
    nav.navigate("password"); // straight to a graph's screen: the graph comes with it
    rule.wait_for_idle();
    assert_eq!(stack_of(&nav), "home profile [login] password");
    nav.navigate("login"); // the graph's route from inside it: another one
    rule.wait_for_idle();
    assert_eq!(stack_of(&nav), "home profile [login] password [login] username");
    let stack = nav.back_queue();
    assert!(stack.len() == 7 && stack[3] != stack[5]);
    assert!(nav.pop_back_stack_to("password", false, false) && stack_of(&nav) == "home profile [login] password");
    assert!(nav.pop_back_stack() && stack_of(&nav) == "home profile");
    assert!(nav.pop_back_stack() && stack_of(&nav) == "home");
    rule.wait_for_idle();
    assert!(!escape(&rule) && rule.on_node_with_text("ana").exists()); // one screen: back is the platform's
    nav.navigate("login");
    assert!(nav.pop_back_stack_to("username", true, false) && stack_of(&nav) == "home"); // the graph left without screens goes
    nav.navigate("login");
    nav.navigate_with("password", |o| o.pop_up_to("login", |_| {})); // down to the graph's entry, and on in it
    assert_eq!(stack_of(&nav), "home [login] password");
    nav.navigate_with("profile", |o| o.pop_up_to("login", |_| {})); // a graph left without screens isn't kept
    assert_eq!(stack_of(&nav), "home profile");
    rule.wait_for_idle();
    assert!(route_of(nav.previous_back_stack_entry()) == "home" && rule.on_node_with_text("profil").exists());
    assert!(nav.navigate_up() && stack_of(&nav) == "home" && !nav.navigate_up());
}

// ---- C++ a_nested_graphs_view_models_are_shared_by_its_screens -----------------------------------------------------

/// A nested graph's screens share its entry's ViewModels (`viewModel(viewModelStoreOwner = the graph's entry)`),
/// cleared as the graph's entry goes.
fn a_nested_graphs_view_models_are_shared_by_its_screens() {
    reset_counts();
    let h = held();
    let in_username = Rc::new(RefCell::new(None::<Rc<TestViewModel>>));
    let in_password = Rc::new(RefCell::new(None::<Rc<TestViewModel>>));
    let (hc, u, p) = (h.clone(), in_username.clone(), in_password.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let nav = remember_nav_controller();
        *hc.borrow_mut() = Some(nav.clone());
        let (enter, exit) = fade_100();
        let (e, x) = (enter.clone(), exit.clone());
        let (u, p, n) = (u.clone(), p.clone(), nav.clone());
        NavHost(
            nav,
            "login", // a graph to start with
            modifier = Modifier.fill_max_size(),
            enter_transition = move |t: &NavTransitionScope| e(t),
            exit_transition = move |t: &NavTransitionScope| x(t),
            pop_enter_transition = Some(enter),
            pop_exit_transition = Some(exit),
            builder = move |graph: &mut NavGraphBuilder| {
                let (u, p, n) = (u.clone(), p.clone(), n.clone());
                graph.navigation("username", "login", move |login: &mut NavGraphBuilder| {
                    let (u, n2) = (u.clone(), n.clone());
                    login.kompozable("username", move |_, e: NavBackStackEntry| {
                        let n = n2.clone();
                        let parent = remember_keyed(e.clone(), move || n.get_back_stack_entry("login"));
                        let vm: Rc<TestViewModel> =
                            view_model(view_model_store_owner = parent.as_view_model_store_owner());
                        *u.borrow_mut() = Some(vm.clone());
                        Text(format!("kullanıcı {}", vm.count.collect_as_state().get()));
                    });
                    let (p, n2) = (p.clone(), n.clone());
                    login.kompozable("password", move |_, e: NavBackStackEntry| {
                        let n = n2.clone();
                        let parent = remember_keyed(e.clone(), move || n.get_back_stack_entry("login"));
                        let vm: Rc<TestViewModel> =
                            view_model(view_model_store_owner = parent.as_view_model_store_owner());
                        *p.borrow_mut() = Some(vm.clone());
                        Text(format!("parola {}", vm.count.collect_as_state().get()));
                    });
                });
                graph.kompozable("home", |_, _| Text("ana"));
            },
        );
    }));
    let nav = nav_of(&h);
    assert!(stack_of(&nav) == "[login] username" && rule.on_node_with_text("kullanıcı 0").exists());
    assert!(!escape(&rule)); // one screen: back leaves the app
    in_username.borrow().as_ref().unwrap().count.set_value(3);
    nav.navigate("password");
    rule.wait_for_idle();
    let same_vm = Rc::ptr_eq(in_password.borrow().as_ref().unwrap(), in_username.borrow().as_ref().unwrap());
    assert!(same_vm && made() == 1 && rule.on_node_with_text("parola 3").exists());
    assert!(escape(&rule)); // the back button
    rule.wait_for_idle();
    assert!(cleared() == 0 && rule.on_node_with_text("kullanıcı 3").exists()); // the graph is still there
    nav.navigate("password");
    rule.wait_for_idle();
    in_password.borrow().as_ref().unwrap().start(); // in the graph's viewModelScope
    nav.navigate_with("home", |o| o.pop_up_to("login", |p| p.set_inclusive(true)));
    rule.advance_frame();
    assert_eq!(cleared(), 0); // the password screen is leaving, with the graph's ViewModel
    rule.advance_time_by(300);
    assert!(cleared() == 1 && stack_of(&nav) == "home" && rule.on_node_with_text("ana").exists());
    let t = ticks();
    rule.advance_time_by(500);
    assert!(ticks() == t && made() == 1);
    nav.navigate("login");
    rule.wait_for_idle();
    let held_entry = nav.get_back_stack_entry("login"); // kept here: its ViewModels are cleared all the same
    let store = held_entry.view_model_store();
    nav.navigate_with("home", |o| o.pop_up_to("login", |_| {})); // down to the graph's entry, then out of it: it goes
    rule.advance_time_by(300);
    assert!(stack_of(&nav) == "home home" && made() == 2 && cleared() == 2 && store.keys().is_empty());
}

// ---- C++ deep_links_open_their_destination_with_the_screens_above_it ----------------------------------------------

/// What the platform was asked: the locations told, the links opened.
#[derive(Default)]
struct Recorded {
    locations: RefCell<Vec<String>>,
    opened: RefCell<Vec<String>>,
}

impl PlatformCallbacks for Recorded {
    fn location_changed(&self, path: &str) {
        self.locations.borrow_mut().push(path.to_string());
    }
    fn open_uri(&self, uri: &str) {
        self.opened.borrow_mut().push(uri.to_string());
    }
}

/// The link the app is opened with goes to its destination, the graphs on the way and their start destinations
/// below it (back leads up); links sent while it runs too; `navigate(uri)` from inside the app goes on top; a link
/// nothing answers is left for another; a route's path is a link (the web's address); the screen shown is told to
/// the platform as it changes (C++ Kompoz's, the web's address: Compose has no such report).
fn deep_links_open_their_destination_with_the_screens_above_it() {
    let h = held();
    let shown = mutable_state_of(false);
    let tick = mutable_state_of(0);
    let (hc, s, t) = (h.clone(), shown.clone(), tick.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        if !s.get() {
            return;
        }
        t.get(); // a change here composes the NavHost again, the stack as it was
        let nav = remember_nav_controller();
        *hc.borrow_mut() = Some(nav.clone());
        NavHost(
            nav,
            "home",
            modifier = Modifier.fill_max_size(),
            builder = |graph: &mut NavGraphBuilder| {
                graph.kompozable("home", |_, _| Text("ana"));
                graph.navigation("list/all", "list", |list: &mut NavGraphBuilder| {
                    list.kompozable("list/all", |_, _| Text("liste"));
                    list.kompozable(
                        "item/{n}",
                        deep_links = vec![
                            nav_deep_link(|d| d.set_uri_pattern(Some("kompoz://item/{n}".into()))),
                            nav_deep_link(|d| d.set_uri_pattern(Some("https://example.com/items/{n}".into()))),
                        ],
                        content = |_, e: NavBackStackEntry| {
                            Text(format!("öğe {}", string_arg(&e, "n").unwrap_or_else(|| "-1".into())))
                        },
                    );
                });
                graph.kompozable(
                    "search?q={q}",
                    deep_links = vec![nav_deep_link(|d| d.set_uri_pattern(Some("kompoz://search?q={q}".into())))],
                    content = |_, e: NavBackStackEntry| {
                        Text(format!("arama: {}", string_arg(&e, "q").unwrap_or_else(|| "-".into())))
                    },
                );
                graph.kompozable(
                    "tag/{name}",
                    deep_links = vec![nav_deep_link(|d| d.set_uri_pattern(Some("kompoz://tags/{name}".into())))],
                    content = |_, e: NavBackStackEntry| {
                        Text(format!("etiket: {}", string_arg(&e, "name").unwrap_or_else(|| "-".into())))
                    },
                );
            },
        );
    }));
    let platform = Rc::new(Recorded::default());
    rule.host().set_callbacks(platform.clone());
    rule.host().open_deep_link("kompoz://item/42"); // the app is opened with it
    shown.set(true);
    rule.wait_for_idle();
    let nav = nav_of(&h);
    assert!(stack_of(&nav) == "home [list] list/all item/{n}" && rule.on_node_with_text("öğe 42").exists());
    assert_eq!(*platform.locations.borrow(), ["item/42"]);
    tick.set(1);
    rule.wait_for_idle();
    assert_eq!(platform.locations.borrow().len(), 1); // told when it changes only
    assert!(escape(&rule)); // back leads up
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("liste").exists());
    assert_eq!(platform.locations.borrow().last().unwrap(), "list/all");
    assert!(escape(&rule));
    rule.wait_for_idle();
    assert!(stack_of(&nav) == "home" && rule.on_node_with_text("ana").exists());
    rule.host().open_deep_link("HTTPS://example.com/items/7#top"); // sent while it runs; the scheme in any case
    rule.wait_for_idle();
    assert!(stack_of(&nav) == "home [list] list/all item/{n}" && rule.on_node_with_text("öğe 7").exists());
    nav.navigate(NavUri("kompoz://search?q=a%20b%26c")); // from inside the app: on top
    rule.wait_for_idle();
    assert_eq!(stack_of(&nav), "home [list] list/all item/{n} search?q={q}");
    assert!(rule.on_node_with_text("arama: a b&c").exists());
    assert_eq!(platform.locations.borrow().last().unwrap(), "search?q=a%20b%26c");
    assert!(!nav.handle_deep_link_uri("kompoz://nothing") && !nav.handle_deep_link_uri("other://item/3")); // nothing answers them
    rule.host().open_deep_link("kompoz://nothing");
    rule.wait_for_idle();
    assert_eq!(stack_of(&nav), "home [list] list/all item/{n} search?q={q}"); // left for another NavHost
    assert_eq!(rule.host().deep_links().take(|_| true).as_deref(), Some("kompoz://nothing"));
    assert!(nav.handle_deep_link_uri("item/5")); // a route's path (the web's #fragment)
    rule.wait_for_idle();
    assert!(stack_of(&nav) == "home [list] list/all item/{n}" && rule.on_node_with_text("öğe 5").exists());
    assert!(nav.handle_deep_link_uri("list") && stack_of(&nav) == "home [list] list/all"); // a graph: its start
    assert!(nav.handle_deep_link_uri("home") && stack_of(&nav) == "home");
    // what the route's path can't hold as it is stays encoded
    assert!(nav.handle_deep_link_uri("kompoz://tags/a%2Fb%20c%3F"));
    rule.wait_for_idle();
    assert!(stack_of(&nav) == "home tag/{name}" && rule.on_node_with_text("etiket: a/b c?").exists());
}

// ---- C++ type_safe_routes_carry_their_arguments -------------------------------------------------------------------

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

/// Typed routes (`#[derive(Route)]`, Compose's `@Serializable`): their patterns (a field with a default or of a
/// collection a query parameter, the others path segments — the C++'s `pro = false` is a path segment here: no
/// default), values written into routes (Compose writes a null "null"; the C++ left it out), destinations of them,
/// a typed graph, `popBackStack<T>`, deep links of them (enum arguments by name: Compose's; the C++'s by number).
fn type_safe_routes_carry_their_arguments() {
    assert_eq!(generate_route_pattern::<HomeRoute>(&TypeMap::default(), None), "HomeRoute");
    let pattern = "ProfileRoute/{id}/{pro}?tab={tab}&zoom={zoom}";
    assert_eq!(generate_route_pattern::<ProfileRoute>(&TypeMap::default(), None), pattern);
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
    let h = held();
    let seen = Rc::new(RefCell::new(None::<ProfileRoute>));
    let (hc, s) = (h.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let nav = remember_nav_controller();
        *hc.borrow_mut() = Some(nav.clone());
        let s = s.clone();
        NavHost_of(
            nav,
            HomeRoute,
            modifier = Modifier.fill_max_size(),
            builder = move |graph: &mut NavGraphBuilder| {
                graph.kompozable_of::<HomeRoute>(|_, _| Text("ana"));
                let s = s.clone();
                graph.kompozable_of::<ProfileRoute>(
                    deep_links = vec![nav_deep_link_of::<ProfileRoute>("kompoz://profile")],
                    content = move |_: &AnimatedContentScope, e: NavBackStackEntry| {
                        let route = e.to_route::<ProfileRoute>();
                        Text(format!("profil {}", route.id));
                        *s.borrow_mut() = Some(route);
                    },
                );
                graph.navigation_of::<LoginGraph>(UsernameRoute { attempt: 0 }, |login: &mut NavGraphBuilder| {
                    login.kompozable_of::<UsernameRoute>(|_, e: NavBackStackEntry| {
                        Text(format!("deneme {}", e.to_route::<UsernameRoute>().attempt))
                    });
                });
            },
        );
    }));
    let nav = nav_of(&h);
    assert!(
        stack_of(&nav) == "HomeRoute" && nav.current_back_stack_entry().unwrap().destination().has_route::<HomeRoute>()
    );
    nav.navigate(ProfileRoute { id: "a b/c".into(), pro: true, tab: Some(ProfileTab::Likes), zoom: Some(2.25) });
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("profil a b/c").exists());
    assert!(nav.current_back_stack_entry().unwrap().destination().has_route::<ProfileRoute>());
    assert_eq!(
        seen.borrow().clone(),
        Some(ProfileRoute { id: "a b/c".into(), pro: true, tab: Some(ProfileTab::Likes), zoom: Some(2.25) })
    );
    nav.navigate(ProfileRoute { id: "x".into(), pro: false, tab: None, zoom: None }); // the optional ones left out
    rule.wait_for_idle();
    assert_eq!(seen.borrow().clone(), Some(ProfileRoute { id: "x".into(), pro: false, tab: None, zoom: None }));
    nav.navigate(LoginGraph); // a graph: its start destination
    rule.wait_for_idle();
    assert_eq!(stack_of(&nav), format!("HomeRoute {pattern} {pattern} [login] login/username/{{attempt}}"));
    assert!(
        rule.on_node_with_text("deneme 0").exists()
            && nav.get_back_stack_entry_of::<LoginGraph>().destination().is_graph()
    );
    nav.navigate(UsernameRoute { attempt: 2 });
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("deneme 2").exists());
    let in_login =
        |nav: &NavController| nav.current_destination().hierarchy().iter().any(|d| d.has_route::<LoginGraph>());
    assert!(in_login(&nav));
    assert!(nav.pop_back_stack_of::<LoginGraph>(true, false));
    assert!(nav.current_back_stack_entry().unwrap().destination().has_route::<ProfileRoute>());
    assert!(nav.handle_deep_link_uri("kompoz://profile/derin/false?tab=posts"));
    rule.wait_for_idle();
    assert_eq!(stack_of(&nav), format!("HomeRoute {pattern}"));
    let s = seen.borrow().clone().unwrap();
    assert!(s.id == "derin" && s.tab == Some(ProfileTab::Posts) && !s.pro);
    // popUpTo<T> { inclusive } and the hierarchy's hasRoute<T>.
    nav.navigate(LoginGraph);
    rule.wait_for_idle();
    assert!(in_login(&nav));
    nav.navigate_with(HomeRoute, |o| o.pop_up_to_of::<HomeRoute>(|p| p.set_inclusive(true)));
    rule.wait_for_idle();
    assert!(stack_of(&nav) == "HomeRoute" && !in_login(&nav) && rule.on_node_with_text("ana").exists());
}

// ---- C++ a_nested_graph_is_saved_and_restored_as_a_tab ------------------------------------------------------------

/// A nested graph saved as a tab (`popUpTo(home) { saveState }`, `restoreState`) with its screens' saveable values and
/// its ViewModels; navigated to without `restoreState`, a new one; saved in place of the first, whose ViewModels go;
/// the saved stack cleared. (Compose's: the saved entries are destroyed, their state saved, entries of the same ids
/// made again on restoring — D10; their ViewModels kept meanwhile.)
fn a_nested_graph_is_saved_and_restored_as_a_tab() {
    reset_counts();
    let h = held();
    let list_model = Rc::new(RefCell::new(None::<Rc<TestViewModel>>));
    let (hc, lm) = (h.clone(), list_model.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let nav = remember_nav_controller();
        *hc.borrow_mut() = Some(nav.clone());
        let (lm, n) = (lm.clone(), nav.clone());
        NavHost(
            nav,
            "home",
            modifier = Modifier.fill_max_size(),
            builder = move |graph: &mut NavGraphBuilder| {
                graph.kompozable("home", |_, _| Text("ana"));
                let (lm, n) = (lm.clone(), n.clone());
                graph.navigation("list/all", "list", move |list: &mut NavGraphBuilder| {
                    list.kompozable("list/all", |_, _| Text("liste"));
                    let (lm, n) = (lm.clone(), n.clone());
                    list.kompozable("item/{n}", move |_, e: NavBackStackEntry| {
                        let nav = n.clone();
                        let parent = remember_keyed(e.clone(), move || nav.get_back_stack_entry("list"));
                        let vm: Rc<TestViewModel> =
                            view_model(view_model_store_owner = parent.as_view_model_store_owner());
                        *lm.borrow_mut() = Some(vm);
                        let likes = remember_saveable(|| mutable_state_of(0));
                        let (l, n) = (likes.clone(), string_arg(&e, "n").unwrap_or_default());
                        Box(
                            modifier = Modifier.clickable(move || l.set(l.peek() + 1)),
                            content = move || Text(format!("öğe {n} · {}", likes.get())),
                        );
                    });
                });
                graph.kompozable("settings", |_, _| Text("ayarlar"));
            },
        );
    }));
    let nav = nav_of(&h);
    let tab = |route: &str| {
        nav.navigate_with(route, |o| {
            o.pop_up_to("home", |p| p.set_save_state(true));
            o.set_launch_single_top(true);
            o.set_restore_state(true);
        });
        rule.wait_for_idle();
    };
    let in_list = |nav: &NavController| {
        nav.current_destination().hierarchy().iter().any(|d| d.route().as_deref() == Some("list"))
    };
    tab("list");
    nav.navigate("item/7");
    rule.wait_for_idle();
    rule.on_node_with_text("öğe 7 · 0").perform_click();
    assert!(rule.on_node_with_text("öğe 7 · 1").exists() && in_list(&nav));
    let graph_id = nav.get_back_stack_entry("list").id();
    tab("settings");
    assert!(stack_of(&nav) == "home settings" && !in_list(&nav) && cleared() == 0); // saved, the graph with it
    tab("list");
    assert!(stack_of(&nav) == "home [list] list/all item/{n}" && nav.get_back_stack_entry("list").id() == graph_id);
    assert!(in_list(&nav) && rule.on_node_with_text("öğe 7 · 1").exists() && made() == 1);
    tab("home");
    tab("home"); // launchSingleTop: still one
    assert!(stack_of(&nav) == "home" && cleared() == 0); // the list tab is saved
    nav.navigate("list"); // without restoreState: a new graph
    nav.navigate("item/8");
    rule.wait_for_idle();
    assert!(stack_of(&nav) == "home [list] list/all item/{n}" && nav.get_back_stack_entry("list").id() != graph_id);
    assert!(made() == 2 && rule.on_node_with_text("öğe 8 · 0").exists());
    // Saved again as a tab: Compose keeps the first saved stack (the list's route is mapped to it already) — the
    // C++ saved this one in its place, clearing the first one's ViewModels; this one's stay in the store, as
    // Compose's do.
    tab("settings");
    assert!(cleared() == 0 && stack_of(&nav) == "home settings");
    tab("list"); // the first one back, as it was
    assert!(stack_of(&nav) == "home [list] list/all item/{n}" && nav.get_back_stack_entry("list").id() == graph_id);
    assert!(rule.on_node_with_text("öğe 7 · 1").exists() && made() == 2);
    // Saved again, then cleared: its ViewModels go; the tab starts afresh.
    tab("settings");
    assert!(nav.clear_back_stack("list"));
    rule.wait_for_idle();
    // (Compose's: the entries it brought back to pop for good are in a transition the NavHost completes when its own
    // next one settles — the screen shown didn't change —: their ViewModels go then)
    assert_eq!(cleared(), 0);
    tab("list");
    assert!(stack_of(&nav) == "home [list] list/all" && cleared() == 1);
}

// ---- C++ uri_handler_opens_the_apps_own_links_in_it (its NavHost half) ---------------------------------------------

/// The app's own links (those a NavHost here answers) open in it, not by the platform; with no NavHost for them,
/// the platform gets them. (Its clickable texts' half is kompoz-foundation's clickable.rs.)
fn uri_handler_opens_the_apps_own_links_in_it() {
    let hosted = mutable_state_of(true);
    let hs = hosted.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let links = kompoz_ui::platform::LocalUriHandler.current();
        let hs = hs.clone();
        Column(
            content = move || {
                let l = links.clone();
                Box(modifier = Modifier.clickable(move || l.open_uri("kompoz://item/7")), content = || Text("kendi"));
                let l = links.clone();
                Box(
                    modifier = Modifier.clickable(move || l.open_uri("https://kotlinlang.org")),
                    content = || Text("dış"),
                );
                if !hs.get() {
                    return;
                }
                let nav = remember_nav_controller();
                NavHost(
                    nav,
                    "home",
                    modifier = Modifier.fill_max_width().height(200.dp()),
                    builder = |graph: &mut NavGraphBuilder| {
                        graph.kompozable("home", |_, _| Text("ev"));
                        graph.kompozable(
                            "item/{n}",
                            deep_links = vec![nav_deep_link(|d| d.set_uri_pattern(Some("kompoz://item/{n}".into())))],
                            content = |_, e: NavBackStackEntry| {
                                Text(format!("öğe {}", string_arg(&e, "n").unwrap_or_default()))
                            },
                        );
                    },
                );
            },
        );
    }));
    let platform = Rc::new(Recorded::default());
    rule.host().set_callbacks(platform.clone());
    rule.on_node_with_text("kendi").perform_click();
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("öğe 7").exists() && platform.opened.borrow().is_empty()); // a NavHost here answers it
    rule.on_node_with_text("dış").perform_click();
    assert_eq!(*platform.opened.borrow(), ["https://kotlinlang.org"]);
    hosted.set(false); // no NavHost for it any more: the system gets it
    rule.wait_for_idle();
    rule.on_node_with_text("kendi").perform_click();
    assert_eq!(*platform.opened.borrow(), ["https://kotlinlang.org", "kompoz://item/7"]);
}

// ---- C++ saved_state_outlives_the_host (its NavHost half) ----------------------------------------------------------

#[derive(Default)]
struct Run {
    nav: RefCell<Option<NavController>>,
    list: RefCell<Option<LazyListState>>,
    count: RefCell<Option<MutableState<i32>>>,
    entry_count: RefCell<Option<MutableState<i32>>>,
}

fn saving_app(r: &Rc<Run>, shown: &MutableState<bool>) -> impl Content + 'static {
    let (r, shown) = (r.clone(), shown.clone());
    kompozable_lambda!(move || {
        if !shown.get() {
            return;
        }
        *r.count.borrow_mut() = Some(remember_saveable(|| mutable_state_of(0)));
        let nav = remember_nav_controller();
        *r.nav.borrow_mut() = Some(nav.clone());
        let r = r.clone();
        NavHost(
            nav,
            "home",
            modifier = Modifier.fill_max_size(),
            builder = move |graph: &mut NavGraphBuilder| {
                let r1 = r.clone();
                graph.kompozable("home", move |_, _| {
                    let state = remember_lazy_list_state();
                    *r1.list.borrow_mut() = Some(state.clone());
                    LazyColumn(
                        modifier = Modifier.fill_max_size(),
                        state = Some(state),
                        content = |this: &mut LazyListScope| {
                            this.items(100, |i| {
                                let liked = remember_saveable(|| mutable_state_of(false));
                                let l = liked.clone();
                                Box(
                                    modifier = Modifier
                                        .height(50.dp())
                                        .clickable(move || l.set(true))
                                        .test_tag(format!("satır{i}")),
                                    content = move || Text(format!("satır {i}{}", if liked.get() { " ♥" } else { "" })),
                                );
                            });
                        },
                    );
                });
                let r2 = r.clone();
                graph.kompozable(
                    "detail/{id}",
                    arguments = vec![int_arg("id")],
                    content = move |_, e: NavBackStackEntry| {
                        let n = remember_saveable(|| mutable_state_of(0));
                        *r2.entry_count.borrow_mut() = Some(n.clone());
                        Text(format!("ayrıntı {} · {}", e.arguments().map_or(-1, |a| a.get_int("id")), n.get()));
                    },
                );
            },
        );
    })
}

/// The NavHost's state written out with the host's (the controller's back stack, the entries' saveable values and
/// their saved state) and back in a host made again from it: the stack as it was, the screen below as it was once
/// back goes to it. (The composition's own values: the host half in kompoz-lifecycle's viewmodel.rs.)
fn saved_state_outlives_the_host() {
    let bytes = {
        let (r, shown) = (Rc::new(Run::default()), mutable_state_of(false));
        let rule = KompozTestRule::new(saving_app(&r, &shown));
        shown.set(true);
        rule.wait_for_idle();
        let list = r.list.borrow().clone().unwrap();
        CoroutineScope::new(rule.host().dispatcher()).launch(async move { list.scroll_to_item(40).await });
        rule.wait_for_idle();
        rule.on_node_with_tag("satır41").perform_click();
        assert!(rule.on_node_with_text("satır 41 ♥").exists());
        r.count.borrow().as_ref().unwrap().set(5);
        r.nav.borrow().as_ref().unwrap().navigate("detail/7");
        rule.wait_for_idle();
        r.entry_count.borrow().as_ref().unwrap().set(3);
        rule.wait_for_idle();
        let bytes = rule.host().save_state();
        let saved = SavedValue::decode(&bytes).unwrap();
        let composition = saved.get("composition").as_map().cloned().unwrap_or_default();
        // (count, the nav controller, the NavHost's holder and its DialogHost's — Compose's: the C++'s had a field more, no dialogs)
        assert_eq!(composition.len(), 4, "{composition:?}");
        bytes
    };
    let (r, shown) = (Rc::new(Run::default()), mutable_state_of(false));
    let rule = KompozTestRule::new(saving_app(&r, &shown));
    rule.host().restore_state(&bytes);
    shown.set(true);
    rule.wait_for_idle();
    let nav = r.nav.borrow().clone().unwrap();
    assert_eq!(r.count.borrow().as_ref().unwrap().peek(), 5);
    assert!(stack_of(&nav) == "home detail/{id}" && rule.on_node_with_text("ayrıntı 7 · 3").exists());
    // Saved again before the screen below was shown: what it had is still there, not yet claimed.
    assert_eq!(SavedValue::decode(&rule.host().save_state()), SavedValue::decode(&bytes));
    assert!(escape(&rule)); // back: the screen below, as it was
    rule.wait_for_idle();
    assert_eq!(r.list.borrow().as_ref().unwrap().first_visible_item_index(), 40);
    assert!(rule.on_node_with_text("satır 41 ♥").exists() && rule.on_node_with_text("satır 42").exists());
}

// ---- C++ nav_entries_have_lifecycles_capped_by_the_nav_host --------------------------------------------------------

/// An entry's lifecycle: resumed on top, started while a transition runs, created on the back stack, destroyed
/// popped; capped by the NavHost's (the app's); the entry is its content's lifecycle owner. (Compose's: a saved tab's
/// entries are destroyed, their state saved, an entry of the same id made on restoring — D10, the C++ kept the same
/// one created —; the NavHost leaving the composition doesn't move its entries — their effects end with it —, the
/// C++ took them to created.)
fn nav_entries_have_lifecycles_capped_by_the_nav_host() {
    use LifecycleState as S;
    let h = held();
    let hosted = mutable_state_of(true);
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let owner_was_entry = Rc::new(Cell::new(false));
    let (a, b);
    {
        let (hc, ho, lg, ow) = (h.clone(), hosted.clone(), log.clone(), owner_was_entry.clone());
        let rule = KompozTestRule::new(kompozable_lambda!(move || {
            let nav = remember_nav_controller();
            *hc.borrow_mut() = Some(nav.clone());
            if !ho.get() {
                return;
            }
            let (lg, ow) = (lg.clone(), ow.clone());
            NavHost(
                nav,
                "a",
                modifier = Modifier.fill_max_size(),
                builder = move |graph: &mut NavGraphBuilder| {
                    let (lg1, ow) = (lg.clone(), ow.clone());
                    graph.kompozable("a", move |_, e: NavBackStackEntry| {
                        ow.set(LocalLifecycleOwner.current() == e.as_lifecycle_owner()); // the entry's
                        let lg = lg1.clone();
                        LifecycleResumeEffect(
                            0,
                            effects = move |scope: &LifecycleResumePauseEffectScope| {
                                lg.borrow_mut().push("a resume".into());
                                let lg = lg.clone();
                                scope.on_pause_or_dispose(move |_| lg.borrow_mut().push("a pause".into()))
                            },
                        );
                        Text("ekran a");
                    });
                    let lg2 = lg.clone();
                    graph.kompozable("b", move |_, _| {
                        let lg = lg2.clone();
                        LifecycleStartEffect(
                            0,
                            effects = move |scope: &LifecycleStartStopEffectScope| {
                                lg.borrow_mut().push("b start".into());
                                let lg = lg.clone();
                                scope.on_stop_or_dispose(move |_| lg.borrow_mut().push("b stop".into()))
                            },
                        );
                        Text("ekran b");
                    });
                    graph.kompozable("c", |_, _| Text("ekran c"));
                },
            );
        }));
        let nav = nav_of(&h);
        let entry_a = nav.current_back_stack_entry().unwrap();
        let state = |e: &NavBackStackEntry| e.lifecycle().current_state();
        assert!(owner_was_entry.get() && state(&entry_a) == S::Resumed && *log.borrow() == ["a resume"]);
        nav.navigate("b");
        // The transition runs: b started, a still shown (Compose 2.10's: the entry going is created at once, shown as
        // it is in a transition; the C++ kept it started).
        let entry_b = nav.current_back_stack_entry().unwrap();
        assert!(state(&entry_a) == S::Created && state(&entry_b) == S::Started);
        assert!(nav.visible_entries().value().contains(&entry_a));
        rule.advance_time_by(32); // (b composed a frame or two later: its start effect then)
        assert!(state(&entry_a) == S::Created && state(&entry_b) == S::Started);
        assert_eq!(*log.borrow(), ["a resume", "a pause", "b start"]);
        rule.wait_for_idle(); // done: b resumed, a waits on the back stack
        assert!(state(&entry_a) == S::Created && state(&entry_b) == S::Resumed);
        // The app going to the background takes the entries with it, at once; back, only up to theirs.
        rule.set_lifecycle_state(S::Created);
        assert!(state(&entry_b) == S::Created && log.borrow().last().unwrap() == "b stop");
        rule.set_lifecycle_state(S::Resumed);
        assert!(
            state(&entry_b) == S::Resumed && state(&entry_a) == S::Created && log.borrow().last().unwrap() == "b start"
        );
        nav.pop_back_stack();
        rule.wait_for_idle(); // popped and gone: destroyed
        assert!(state(&entry_b) == S::Destroyed && state(&entry_a) == S::Resumed);
        assert!(
            log.borrow().last().unwrap() == "a resume" && log.borrow().iter().filter(|l| *l == "b stop").count() == 2
        );
        // A stack saved (tabs): its entries destroyed, their state saved; restored, entries of the same ids resume.
        nav.navigate("b");
        rule.wait_for_idle();
        let saved = nav.current_back_stack_entry().unwrap();
        nav.navigate_with("c", |o| o.pop_up_to("a", |p| p.set_save_state(true)));
        rule.wait_for_idle();
        let c = nav.current_back_stack_entry().unwrap();
        assert!(state(&saved) == S::Destroyed && state(&c) == S::Resumed);
        nav.navigate_with("b", |o| {
            o.pop_up_to("a", |p| p.set_save_state(true));
            o.set_restore_state(true);
        });
        rule.wait_for_idle();
        let restored = nav.current_back_stack_entry().unwrap();
        assert!(restored.id() == saved.id() && state(&restored) == S::Resumed && state(&c) == S::Destroyed);
        nav.pop_back_stack();
        rule.wait_for_idle();
        hosted.set(false); // the NavHost leaves: its screens' effects end with them
        rule.wait_for_idle();
        assert!(state(&entry_a) == S::Resumed && log.borrow().last().unwrap() == "a pause");
        hosted.set(true);
        rule.wait_for_idle();
        assert!(state(&entry_a) == S::Resumed && log.borrow().last().unwrap() == "a resume");
        (a, b) = (entry_a, entry_b);
    }
    assert_eq!(a.lifecycle().current_state(), S::Destroyed); // with the app
    assert_eq!(b.lifecycle().current_state(), S::Destroyed);
}

fn main() {
    let tests: &[(&str, fn())] = &[
        ("nav_host_goes_to_routes_with_arguments_and_back", nav_host_goes_to_routes_with_arguments_and_back),
        (
            "nav_host_brings_entries_in_over_going_forward_and_under_going_back",
            nav_host_brings_entries_in_over_going_forward_and_under_going_back,
        ),
        (
            "nav_host_seeks_its_pop_transition_with_the_back_gesture",
            nav_host_seeks_its_pop_transition_with_the_back_gesture,
        ),
        (
            "nav_host_keeps_saveable_state_on_the_back_stack_and_in_saved_tabs",
            nav_host_keeps_saveable_state_on_the_back_stack_and_in_saved_tabs,
        ),
        ("saveable_scroll_position_comes_back_with_its_entry", saveable_scroll_position_comes_back_with_its_entry),
        (
            "view_models_of_navigation_entries_go_before_their_host",
            view_models_of_navigation_entries_go_before_their_host,
        ),
        ("view_models_live_as_long_as_their_navigation_entry", view_models_live_as_long_as_their_navigation_entry),
        (
            "nested_navigation_graphs_are_entered_by_route_and_left_with_their_screens",
            nested_navigation_graphs_are_entered_by_route_and_left_with_their_screens,
        ),
        (
            "a_nested_graphs_view_models_are_shared_by_its_screens",
            a_nested_graphs_view_models_are_shared_by_its_screens,
        ),
        (
            "deep_links_open_their_destination_with_the_screens_above_it",
            deep_links_open_their_destination_with_the_screens_above_it,
        ),
        ("type_safe_routes_carry_their_arguments", type_safe_routes_carry_their_arguments),
        ("a_nested_graph_is_saved_and_restored_as_a_tab", a_nested_graph_is_saved_and_restored_as_a_tab),
        ("uri_handler_opens_the_apps_own_links_in_it", uri_handler_opens_the_apps_own_links_in_it),
        ("saved_state_outlives_the_host", saved_state_outlives_the_host),
        ("nav_entries_have_lifecycles_capped_by_the_nav_host", nav_entries_have_lifecycles_capped_by_the_nav_host),
    ];
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    let mut failed = Vec::new();
    for (name, test) in tests {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        if std::panic::catch_unwind(test).is_err() {
            failed.push(*name);
            println!("FAILED {name}");
        } else {
            println!("ok     {name}");
        }
    }
    println!("\n{} run, {} failed", tests.len(), failed.len());
    if !failed.is_empty() {
        std::process::exit(1);
    }
}

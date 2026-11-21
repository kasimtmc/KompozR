//! foundation's clickables: `clickable`, `combined_clickable`, `selectable`, `toggleable`, `tri_state_toggleable`,
//! `focusable`, `hoverable`, interaction sources, indications and `magnifier` — taps, mouse clicks and keys, long
//! and double clicks, presses shown and cancelled, focus and hover, what accessibility is told — and the C++
//! Kompoz's tests that needed them (without Material, scrolling, text fields or animation; Compose's behaviour where
//! the two differ). Numbers in px.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::input::*;
use kompoz_ui::platform::{
    HapticFeedbackType, LocalHapticFeedback, LocalUriHandler, LocalViewConfiguration, PlatformCallbacks,
    ViewConfiguration,
};
use kompoz_ui::semantics::*;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

type Log = Rc<RefCell<Vec<String>>>;

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn push(log: &Log, s: impl Into<String>) {
    log.borrow_mut().push(s.into());
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn down(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
}

fn down_id(rule: &KompozTestRule, p: Offset, id: i64) {
    rule.host().pointer_down(p, -1, id, PointerType::Touch);
}

fn move_to(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_move(p, -1, 0);
}

fn up(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_up(p, -1, 0);
}

fn up_id(rule: &KompozTestRule, p: Offset, id: i64) {
    rule.host().pointer_up(p, -1, id);
}

/// C++'s `tap`: down, a frame, up, a frame.
fn tap(rule: &KompozTestRule, p: Offset) {
    down(rule, p);
    rule.advance_frame();
    up(rule, p);
    rule.advance_frame();
}

/// A mouse click: the primary button down, a frame, up.
fn mouse_click(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Mouse);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_frame();
}

/// A key pressed and let go.
fn key(rule: &KompozTestRule, k: Key) -> bool {
    let taken = rule.host().key_down(k, KeyModifiers::None);
    rule.host().key_up(k, KeyModifiers::None);
    taken
}

fn center(rule: &KompozTestRule, tag: &str) -> Offset {
    rule.on_node_with_tag_unmerged(tag).get_bounds_in_root().center()
}

/// Whether the node tagged `tag` has the focus.
fn focused(rule: &KompozTestRule, tag: &str) -> bool {
    let node = rule.on_node_with_tag_unmerged(tag).layout_node();
    rule.host().focused_node().is_some_and(|f| Rc::ptr_eq(&f, &node))
}

/// The name of an interaction (C++'s Interaction::Type).
fn interaction_name(i: &dyn Interaction) -> &'static str {
    if i.is::<PressInteraction::Press>() {
        "Press"
    } else if i.is::<PressInteraction::Release>() {
        "Release"
    } else if i.is::<PressInteraction::Cancel>() {
        "Cancel"
    } else if i.is::<HoverInteraction::Enter>() {
        "HoverEnter"
    } else if i.is::<HoverInteraction::Exit>() {
        "HoverExit"
    } else if i.is::<FocusInteraction::Focus>() {
        "Focus"
    } else if i.is::<FocusInteraction::Unfocus>() {
        "Unfocus"
    } else if i.is::<DragInteraction::Start>() {
        "DragStart"
    } else if i.is::<DragInteraction::Stop>() {
        "DragStop"
    } else if i.is::<DragInteraction::Cancel>() {
        "DragCancel"
    } else {
        "?"
    }
}

/// C++'s LogInteractions: what `source` emits, logged while it is composed.
#[kompozable(inline)]
fn LogInteractions(source: MutableInteractionSource, log: Log) {
    let flow = source.interactions();
    LaunchedEffect(source.clone(), async move {
        flow.collect(move |i| push(&log, interaction_name(&*i))).await;
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Clicks

fn counter_increments_on_click() {
    // C++'s counter_increments_on_click: a clickable text (its role a button) for its Material Button.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let count = remember(|| mutable_state_of(0));
        Column(|| {
            Text(format!("Count: {}", count.get()));
            Text(
                "+",
                modifier = Modifier.clickable(role = Some(Role::Button), on_click = move || count.set(count.get() + 1)),
            );
        });
    }));
    rule.on_node_with_text("Count: 0").assert_exists();
    rule.on_node_with_text("+").assert_has_click_action().perform_click();
    rule.on_node_with_text("+").perform_click();
    rule.on_node_with_text("Count: 2").assert_exists();
    rule.on_node_with_text("Count: 0").assert_does_not_exist();
}

fn clicks_come_by_touch_mouse_keys_and_accessibility() {
    let clicks = Rc::new(Cell::new(0));
    let c = clicks.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        Box(modifier = Modifier.size(100.dp()).clickable(move || c.set(c.get() + 1)).test_tag("kutu"));
    }));
    tap(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1);
    mouse_click(&rule, at(50.0, 50.0));
    assert_eq!(clicks.get(), 2);
    // Keys: focused with Tab (keyboard mode), then Enter, NumPadEnter, Space and DPad center click it on their
    // release; others don't.
    assert!(key(&rule, Key::Tab) && focused(&rule, "kutu"));
    assert!(rule.host().key_down(Key::Enter, KeyModifiers::None));
    assert_eq!(clicks.get(), 2); // pressed, not yet clicked
    assert!(rule.host().key_up(Key::Enter, KeyModifiers::None));
    assert_eq!(clicks.get(), 3);
    for k in [Key::NumPadEnter, Key::Spacebar, Key::DirectionCenter] {
        assert!(key(&rule, k));
    }
    assert_eq!(clicks.get(), 6);
    assert!(!key(&rule, Key::A));
    assert_eq!(clicks.get(), 6);
    // A key held repeating presses once and clicks once.
    rule.host().key_down(Key::Enter, KeyModifiers::None);
    rule.host().key_down(Key::Enter, KeyModifiers::None);
    rule.host().key_up(Key::Enter, KeyModifiers::None);
    assert_eq!(clicks.get(), 7);
    // A release that wasn't pressed here clicks nothing (and isn't taken).
    assert!(!rule.host().key_up(Key::Enter, KeyModifiers::None));
    assert_eq!(clicks.get(), 7);
    // Accessibility's click.
    rule.on_node_with_tag("kutu").perform_semantics_action(SemanticsActions::OnClick);
    assert_eq!(clicks.get(), 8);
}

/// A tap detector logging `name` on each tap.
fn tapper(
    log: &Log,
    name: &'static str,
) -> impl Fn(PointerInputScope) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + 'static {
    let log = log.clone();
    move |s: PointerInputScope| {
        let log = log.clone();
        Box::pin(async move { s.detect_tap_gestures().on_tap(move |_| push(&log, name)).await })
    }
}

fn tap_handlers_and_clicks_respect_consumption() {
    // C++'s tap_handlers_and_clicks_respect_consumption: a press taken by the handler inside is no click, and the
    // other way round.
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Column(move || {
            let (a, b) = (l.clone(), l.clone());
            Box(
                modifier = Modifier.size(100.dp()).clickable(move || push(&a, "parent click")),
                content = move || Box(modifier = Modifier.size(50.dp()).pointer_input(0, tapper(&b, "child tap"))),
            );
            let (c, d) = (l.clone(), l.clone());
            Box(
                modifier = Modifier.size(100.dp()).pointer_input(0, tapper(&c, "parent tap")),
                content = move || {
                    let d = d.clone();
                    Box(modifier = Modifier.size(50.dp()).clickable(move || push(&d, "child click")));
                },
            );
        });
    }));
    for p in [at(25.0, 25.0), at(75.0, 75.0), at(25.0, 125.0), at(75.0, 175.0)] {
        tap(&rule, p);
        rule.wait_for_idle();
    }
    assert_eq!(taken(&log), ["child tap", "parent click", "child click", "parent tap"]);
}

fn presses_move_out_cancel_and_disabled_takes_none() {
    // A press moved out of the clickable (and its touch target) is cancelled; one moved within it still clicks; a
    // press taken in the Final pass by another (a scroll) is cancelled; disabled: no click, no press.
    let clicks = Rc::new(Cell::new(0));
    let enabled = mutable_state_of(true);
    let (c, e) = (clicks.clone(), enabled.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, e) = (c.clone(), e.clone());
        Column(move || {
            let c = c.clone();
            Box(modifier = Modifier
                .size(100.dp())
                .clickable(enabled = e.get(), on_click = move || c.set(c.get() + 1))
                .test_tag("kutu"));
            Box(modifier = Modifier.size(100.dp()).test_tag("boş"));
        });
    }));
    down(&rule, at(50.0, 50.0));
    rule.advance_frame();
    move_to(&rule, at(90.0, 90.0));
    rule.advance_frame();
    up(&rule, at(90.0, 90.0));
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1);
    down(&rule, at(50.0, 50.0));
    rule.advance_frame();
    move_to(&rule, at(50.0, 150.0));
    rule.advance_frame();
    move_to(&rule, at(50.0, 50.0)); // (back over it: still cancelled)
    up(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1);
    enabled.set(false);
    rule.wait_for_idle();
    rule.on_node_with_tag("kutu").assert_is_not_enabled().assert_has_click_action();
    tap(&rule, at(50.0, 50.0));
    key(&rule, Key::Tab);
    key(&rule, Key::Enter);
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1);
    assert!(rule.host().focused_node().is_none()); // not focusable while disabled
    enabled.set(true);
    rule.wait_for_idle();
    rule.on_node_with_tag("kutu").assert_is_enabled();
    tap(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 2);
}

fn a_press_taken_in_the_final_pass_is_cancelled() {
    // An outer handler consuming the move in the Final pass (what a scroll does once it starts) cancels the press.
    let source = MutableInteractionSource::new();
    let log = new_log();
    let clicks = Rc::new(Cell::new(0));
    let (s, l, c) = (source.clone(), log.clone(), clicks.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        LogInteractions(s.clone(), l.clone());
        let (s, c) = (s.clone(), c.clone());
        Box(
            modifier = Modifier.size(200.dp()).pointer_input(0, |s: PointerInputScope| async move {
                s.await_pointer_event_scope(|s| async move {
                    loop {
                        let e = s.await_pointer_event_with(PointerEventPass::Final).await;
                        if e.r#type == PointerEventType::Move {
                            e.changes.iter().for_each(|c| c.consume());
                        }
                    }
                })
                .await
            }),
            content = move || {
                let c = c.clone();
                Box(modifier = Modifier
                    .size(100.dp())
                    .clickable(interaction_source = Some(s.clone()), on_click = move || c.set(c.get() + 1)));
            },
        );
    }));
    down(&rule, at(50.0, 50.0));
    rule.advance_frame();
    move_to(&rule, at(52.0, 50.0));
    rule.advance_frame();
    up(&rule, at(52.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 0);
    assert_eq!(taken(&log), ["Press", "Cancel"]);
    drop(source);
}

fn combined_clickable_long_and_double_clicks() {
    // C++'s combined_clickable_long_and_double_clicks: a click waits for a second; two are a double click (the
    // second after Compose's 40 ms double tap minimum: C++ had none); a long press is a long click only.
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (a, b, c) = (l.clone(), l.clone(), l.clone());
        let long: Callback = Rc::new(move || push(&b, "long"));
        let double: Callback = Rc::new(move || push(&c, "double"));
        Box(modifier = Modifier.size(100.dp()).combined_clickable(
            on_long_click = Some(long),
            on_double_click = Some(double),
            on_click = move || push(&a, "click"),
        ));
    }));
    tap(&rule, at(50.0, 50.0));
    assert!(log.borrow().is_empty()); // waits for a second click
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["click"]);

    tap(&rule, at(50.0, 50.0));
    rule.main_clock().advance_time_by(48);
    tap(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["double"]);

    // Too soon after the first: the second press doesn't count, the first is a click.
    tap(&rule, at(50.0, 50.0));
    tap(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["click"]);

    down(&rule, at(50.0, 50.0));
    rule.main_clock().advance_time_by(500);
    assert_eq!(taken(&log), ["long"]);
    up(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());
}

fn combined_clickable_without_a_double_click_clicks_at_once_and_keys_long_click() {
    // No double click: a click comes with the release. A key held for the long press timeout is a long click (no
    // click); two key clicks in time are a double click; accessibility has a long click action with its label.
    let log = new_log();
    let double = mutable_state_of(false);
    let (l, d) = (log.clone(), double.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (a, b, c) = (l.clone(), l.clone(), l.clone());
        let long: Callback = Rc::new(move || push(&b, "long"));
        let double: Option<Callback> =
            if d.get() { Some(Rc::new(move || push(&c, "double")) as Callback) } else { None };
        Box(modifier = Modifier
            .size(100.dp())
            .combined_clickable(
                on_long_click_label = Some("uzun".to_string()),
                on_long_click = Some(long),
                on_double_click = double,
                on_click = move || push(&a, "click"),
            )
            .test_tag("kutu"));
    }));
    tap(&rule, at(50.0, 50.0));
    assert_eq!(taken(&log), ["click"]);
    let node = rule.on_node_with_tag("kutu").fetch_semantics_node();
    let long = node.config().get(SemanticsActions::OnLongClick).expect("a long click action");
    assert_eq!(long.label.as_deref(), Some("uzun"));
    rule.on_node_with_tag("kutu").perform_semantics_action(SemanticsActions::OnLongClick);
    assert_eq!(taken(&log), ["long"]);
    // Keys.
    key(&rule, Key::Tab);
    rule.host().key_down(Key::Enter, KeyModifiers::None);
    rule.main_clock().advance_time_by(500);
    assert_eq!(taken(&log), ["long"]);
    rule.host().key_up(Key::Enter, KeyModifiers::None);
    rule.wait_for_idle();
    assert!(taken(&log).is_empty()); // the long click was it
    key(&rule, Key::Enter);
    assert_eq!(taken(&log), ["click"]);
    double.set(true);
    rule.wait_for_idle();
    key(&rule, Key::Enter);
    rule.main_clock().advance_time_by(100);
    key(&rule, Key::Enter);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["double"]);
    key(&rule, Key::Enter);
    assert!(log.borrow().is_empty());
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["click"]); // no second in time
}

/// Platform callbacks of the test's own: the links opened and the haptics felt.
#[derive(Default)]
struct Recorded {
    opened: RefCell<Vec<String>>,
    felt: RefCell<Vec<HapticFeedbackType>>,
    clipboard: RefCell<Option<String>>,
}

impl PlatformCallbacks for Recorded {
    fn open_uri(&self, uri: &str) {
        self.opened.borrow_mut().push(uri.to_string());
    }
    fn haptic_feedback(&self, kind: HapticFeedbackType) {
        self.felt.borrow_mut().push(kind);
    }
    fn get_clip_entry(&self) -> Option<kompoz_ui::clipboard::ClipEntry> {
        self.clipboard.borrow().clone().map(kompoz_ui::clipboard::ClipEntry::with_plain_text)
    }
    fn set_clip_entry(&self, entry: Option<kompoz_ui::clipboard::ClipEntry>) {
        *self.clipboard.borrow_mut() = entry.and_then(|e| e.read_plain_text());
    }
}

fn platform_services_open_links_and_long_clicks_are_felt() {
    // The parts of C++'s platform_services_open_links_vibrate_and_hide_the_keyboard and
    // uri_handler_opens_the_apps_own_links_in_it without text fields and NavHost: clickable texts open a link,
    // feel, copy and paste; a combined clickable's long click is felt (its haptic feedback on), not when off.
    let value = mutable_state_of(String::new());
    let long_clicks = Rc::new(Cell::new(0));
    let (v, lc) = (value.clone(), long_clicks.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let links = LocalUriHandler.current();
        let haptics = LocalHapticFeedback.current();
        let clip = kompoz_ui::platform::LocalClipboardManager.current();
        let (v, lc) = (v.clone(), lc.clone());
        Column(move || {
            let links = links.clone();
            Text("aç", modifier = Modifier.clickable(move || links.open_uri("https://kotlinlang.org")));
            let haptics = haptics.clone();
            Text(
                "onay",
                modifier = Modifier.clickable(move || haptics.perform_haptic_feedback(HapticFeedbackType::Confirm)),
            );
            let (clip, v) = (clip.clone(), v.clone());
            Text(
                "pano",
                modifier = Modifier.clickable(move || {
                    v.set(clip.get_text().map(|t| t.text().to_string()).unwrap_or_default());
                    clip.set_text(kompoz_text::AnnotatedString::new("kopya"));
                }),
            );
            let (a, b) = (lc.clone(), lc.clone());
            let long: Callback = Rc::new(move || a.set(a.get() + 1));
            let quiet: Callback = Rc::new(move || b.set(b.get() + 1));
            Box(modifier = Modifier
                .size_wh(100.dp(), 40.dp())
                .combined_clickable(on_long_click = Some(long), on_click = || {})
                .test_tag("uzun"));
            Box(modifier = Modifier
                .size_wh(100.dp(), 40.dp())
                .combined_clickable(on_long_click = Some(quiet), haptic_feedback_enabled = false, on_click = || {})
                .test_tag("sessiz"));
        });
    }));
    let platform = Rc::new(Recorded::default());
    *platform.clipboard.borrow_mut() = Some("önceki".into());
    rule.host().set_callbacks(platform.clone());
    rule.on_node_with_text("aç").perform_click();
    assert_eq!(*platform.opened.borrow(), ["https://kotlinlang.org"]);
    rule.on_node_with_text("onay").perform_click();
    assert_eq!(*platform.felt.borrow(), [HapticFeedbackType::Confirm]);
    rule.on_node_with_text("pano").perform_click();
    assert!(value.peek() == "önceki" && platform.clipboard.borrow().as_deref() == Some("kopya"));
    let c = center(&rule, "uzun");
    down(&rule, c);
    rule.main_clock().advance_time_by(500);
    up(&rule, c);
    rule.wait_for_idle();
    assert_eq!(long_clicks.get(), 1);
    assert_eq!(*platform.felt.borrow(), [HapticFeedbackType::Confirm, HapticFeedbackType::LongPress]);
    let c = center(&rule, "sessiz");
    down(&rule, c);
    rule.main_clock().advance_time_by(500);
    up(&rule, c);
    rule.wait_for_idle();
    assert_eq!(long_clicks.get(), 2);
    assert_eq!(platform.felt.borrow().len(), 2); // not felt
}

fn the_view_configuration_times_a_long_click() {
    // The combined clickable part of C++'s view_configuration_sets_the_gesture_thresholds: the host's long press
    // timeout (600 ms) is the long click's.
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let _seen = LocalViewConfiguration.current(); // (read here, as C++'s)
        let l = l.clone();
        let long: Callback = Rc::new(move || push(&l, "uzun"));
        Box(modifier = Modifier.size(100.dp()).combined_clickable(on_long_click = Some(long), on_click = || {}));
    }));
    rule.host().set_view_configuration(ViewConfiguration {
        long_press_timeout: Duration::from_millis(600),
        ..ViewConfiguration::default()
    });
    rule.wait_for_idle();
    down(&rule, at(50.0, 50.0));
    rule.main_clock().advance_time_by(500);
    assert!(log.borrow().is_empty());
    rule.main_clock().advance_time_by(150);
    assert_eq!(taken(&log), ["uzun"]);
    up(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
}

fn leaving_content_takes_no_taps() {
    // C++'s leaving_content_takes_no_taps without its animations (their half is kompoz-animation's tests/animated.rs):
    // content gone takes no taps — the cards replaced by the detail, the button hidden.
    let selected = mutable_state_of(-1);
    let shown = mutable_state_of(true);
    let button_clicks = Rc::new(Cell::new(0));
    let (se, sh, bc) = (selected.clone(), shown.clone(), button_clicks.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (se, sh, bc) = (se.clone(), sh.clone(), bc.clone());
        Column(move || {
            let item = se.get();
            let se = se.clone();
            Box(
                modifier = Modifier.height(300.dp()),
                content = move || {
                    if item >= 0 {
                        Text(
                            format!("ayrıntı {item}"),
                            modifier = Modifier.padding_sides_with(0.dp(), 200.dp(), 0.dp(), 0.dp()),
                        );
                    } else {
                        let se = se.clone();
                        Column(move || {
                            for i in 0..3 {
                                let se = se.clone();
                                Box(modifier = Modifier
                                    .size_wh(100.dp(), 40.dp())
                                    .test_tag(format!("card{i}"))
                                    .clickable(move || se.set(i)));
                            }
                        });
                    }
                },
            );
            if sh.get() {
                let bc = bc.clone();
                Box(modifier =
                    Modifier.size_wh(100.dp(), 40.dp()).test_tag("button").clickable(move || bc.set(bc.get() + 1)));
            }
        });
    }));
    let (first, second) = (center(&rule, "card0"), center(&rule, "card1"));
    down(&rule, first);
    up(&rule, first);
    rule.advance_frame();
    assert_eq!(selected.peek(), 0);
    rule.on_node_with_tag("card1").assert_does_not_exist();
    down(&rule, second);
    up(&rule, second);
    rule.wait_for_idle();
    assert_eq!(selected.peek(), 0);
    rule.on_node_with_text("ayrıntı 0").assert_exists();
    let button = center(&rule, "button");
    shown.set(false);
    rule.advance_frame();
    down(&rule, button);
    up(&rule, button);
    rule.wait_for_idle();
    assert_eq!(button_clicks.get(), 0);
}

fn small_touch_targets_take_touches_near_them() {
    // C++'s small_touch_targets_take_touches_near_them with its clickables: a 20 px one takes a touch 5 px off it,
    // the nearer of two, not past its 48 px target, a small button in a card first, not from outside a clip; a
    // finger let go off it but in its target still clicks; a mouse's is just its bounds.
    let taps: Log = new_log();
    let t = taps.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let t = t.clone();
        Box(
            modifier = Modifier.fill_max_size(),
            content = move || {
                let target = |name: &'static str, x: i32, y: i32, w: i32, h: i32, t: &Log| {
                    let t = t.clone();
                    Modifier
                        .absolute_offset_with(x.dp(), y.dp())
                        .size_wh(w.dp(), h.dp())
                        .clickable(move || push(&t, name))
                };
                Box(modifier = target("a", 100, 100, 20, 20, &t)); // its minimum touch target: 86..134
                Box(modifier = target("b", 140, 100, 20, 20, &t)); // 126..174: overlapping a's
                Box(modifier = target("geniş", 0, 300, 300, 20, &t)); // wide enough: grown only up and down
                let (card, inner) = (t.clone(), t.clone());
                Box(
                    modifier = Modifier
                        .absolute_offset_with(0.dp(), 400.dp())
                        .size_wh(200.dp(), 100.dp())
                        .clickable(move || push(&card, "kart")),
                    content = move || Box(modifier = target("iç", 100, 40, 20, 20, &inner)),
                );
                let detector = t.clone();
                Box(modifier = Modifier
                    .absolute_offset_with(300.dp(), 100.dp())
                    .size(20.dp())
                    .pointer_input(0, tapper(&detector, "dokunuş")));
                Box(modifier = target("büyük", 100, 600, 100, 100, &t));
                Box(modifier = target("yanında", 205, 640, 20, 20, &t));
                let corner = t.clone();
                Box(
                    modifier = Modifier.absolute_offset_with(250.dp(), 400.dp()).size(100.dp()).clip_to_bounds(),
                    content = move || Box(modifier = target("köşe", 0, 0, 20, 20, &corner)),
                );
            },
        );
    }));
    let touch = |p: Offset| {
        tap(&rule, p);
        rule.wait_for_idle();
        taken(&taps)
    };
    assert_eq!(touch(at(95.0, 110.0)), ["a"]); // 5 px off it
    assert!(touch(at(80.0, 110.0)).is_empty()); // past its minimum touch target
    assert_eq!(touch(at(128.0, 110.0)), ["a"]); // in both: the nearer
    assert_eq!(touch(at(132.0, 110.0)), ["b"]);
    assert_eq!(touch(at(150.0, 330.0)), ["geniş"]); // 10 px under it
    assert!(touch(at(310.0, 310.0)).is_empty()); // past its end: not grown that way
    assert_eq!(touch(at(125.0, 450.0)), ["iç"]); // by the small button in the card: the button
    assert_eq!(touch(at(325.0, 110.0)), ["dokunuş"]); // a tap detector's too
    assert_eq!(touch(at(198.0, 650.0)), ["büyük"]); // touched directly: that one
    assert_eq!(touch(at(255.0, 425.0)), ["köşe"]);
    assert!(touch(at(255.0, 395.0)).is_empty()); // not from outside the box clipping it
    // Let go off it, but in its minimum touch target: still a click.
    down(&rule, at(110.0, 110.0));
    rule.advance_frame();
    move_to(&rule, at(95.0, 110.0));
    rule.advance_frame();
    up(&rule, at(95.0, 110.0));
    rule.wait_for_idle();
    assert_eq!(taken(&taps), ["a"]);
    // A mouse's is just its bounds: pressed on a, let go off it, no click.
    let h = rule.host();
    h.pointer_down(at(95.0, 110.0), -1, 0, PointerType::Mouse);
    rule.advance_frame();
    h.pointer_up(at(95.0, 110.0), -1, 0);
    rule.wait_for_idle();
    h.pointer_down(at(110.0, 110.0), -1, 0, PointerType::Mouse);
    rule.advance_frame();
    h.pointer_move(at(95.0, 110.0), -1, 0);
    rule.advance_frame();
    h.pointer_up(at(95.0, 110.0), -1, 0);
    rule.wait_for_idle();
    assert!(taken(&taps).is_empty());
}

fn z_index_puts_a_sibling_above_the_others() {
    // C++'s z_index_puts_a_sibling_above_the_others: drawn and touched in z order.
    let z = mutable_state_of(1.0f32);
    let clicked = new_log();
    let (zz, c) = (z.clone(), clicked.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (zz, c) = (zz.clone(), c.clone());
        Column(move || {
            let (a, b, cc) = (c.clone(), c.clone(), c.clone());
            let z = zz.get();
            Box(
                modifier = Modifier.size_wh(200.dp(), 120.dp()),
                content = move || {
                    let (a, b, cc) = (a.clone(), b.clone(), cc.clone());
                    Box(
                        modifier = Modifier.size(100.dp()).z_index(z).clickable(move || push(&a, "a")),
                        content = || Text("a"),
                    );
                    Box(modifier = Modifier.size(100.dp()).clickable(move || push(&b, "b")), content = || Text("b"));
                    Box(
                        modifier = Modifier.size(100.dp()).z_index(-1.0).clickable(move || push(&cc, "c")),
                        content = || Text("c"),
                    );
                },
            );
            // A layout's own placement order: y placed under x.
            Layout(
                content = || {
                    Text("x");
                    Text("y");
                },
                measure_policy = measure_policy(|scope, ms, c| {
                    let loose = Constraints::new(0, c.max_width, 0, c.max_height);
                    let (x, y) = (ms[0].measure(loose), ms[1].measure(loose));
                    let (w, h) = (x.width().max(y.width()), x.height().max(y.height()));
                    scope.layout(w, h, move |p| {
                        p.place(&x, 0, 0);
                        p.place_at(&y, IntOffset::Zero, -1.0);
                    })
                }),
            );
        });
    }));
    assert_eq!(rule.drawn_texts(), ["c", "b", "a", "y", "x"]);
    down(&rule, at(50.0, 50.0));
    up(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(taken(&clicked), ["a"]); // the one on top takes the touch
    z.set(0.0); // back among the others in composition order
    rule.wait_for_idle();
    assert_eq!(rule.drawn_texts(), ["c", "a", "b", "y", "x"]);
    down(&rule, at(50.0, 50.0));
    up(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(taken(&clicked), ["b"]);
}

fn children_measured_but_not_placed_are_not_drawn_hit_or_told() {
    // C++'s children_measured_but_not_placed_are_not_drawn_hit_or_told.
    let taps = Rc::new(Cell::new(0));
    let positioned = Rc::new(Cell::new(0));
    let (t, p) = (taps.clone(), positioned.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (t, p) = (t.clone(), p.clone());
        Layout(
            modifier = Modifier.size_wh(200.dp(), 100.dp()),
            content = move || {
                Box(modifier = Modifier.size(50.dp()).background(Color::new(1.0, 0.0, 0.0, 1.0)).test_tag("yerleşen"));
                let (t, p) = (t.clone(), p.clone());
                Box(modifier = Modifier
                    .size(80.dp())
                    .background(Color::new(0.0, 0.0, 1.0, 1.0))
                    .clickable(move || t.set(t.get() + 1))
                    .semantics(|c| c.set_content_description("gizli"))
                    .on_globally_positioned(move |_| p.set(p.get() + 1))
                    .test_tag("bekleyen"));
            },
            measure_policy = measure_policy(|scope, ms, c| {
                let loose = Constraints::new(0, c.max_width, 0, c.max_height);
                let placed = ms[0].measure(loose);
                let _measured_only = ms[1].measure(loose);
                scope.layout(200, 100, move |pl| pl.place(&placed, 0, 0))
            }),
        );
    }));
    let node = |tag: &str| rule.on_node_with_tag_unmerged(tag).layout_node();
    assert!(node("yerleşen").is_placed() && !node("bekleyen").is_placed());
    let canvas = rule.draw();
    let red = canvas.rect_colors.contains(&Color::new(1.0, 0.0, 0.0, 1.0));
    let blue = canvas.rect_colors.contains(&Color::new(0.0, 0.0, 1.0, 1.0));
    assert!(red && !blue); // not drawn
    tap(&rule, at(60.0, 60.0)); // where it would be
    rule.wait_for_idle();
    assert_eq!(taps.get(), 0); // not hit
    assert!(!rule.host().accessibility_tree().iter().any(|n| n.label.contains("gizli"))); // not told
    assert_eq!(positioned.get(), 0); // nor told where it is
}

fn a_two_finger_tap_is_a_click() {
    // The click part of C++'s two_finger_tap_is_no_click_and_the_next_finger_leads: Compose's clickable clicks
    // once all the pointers are up, however many there were (C++ took a second finger as no tap).
    let clicks = Rc::new(Cell::new(0));
    let c = clicks.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        Box(modifier = Modifier.size(200.dp()).clickable(move || c.set(c.get() + 1)).test_tag("button"));
    }));
    down_id(&rule, at(50.0, 50.0), 1);
    down_id(&rule, at(150.0, 50.0), 2);
    up_id(&rule, at(50.0, 50.0), 1);
    up_id(&rule, at(150.0, 50.0), 2);
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1);
    rule.on_node_with_tag("button").perform_click();
    assert_eq!(clicks.get(), 2);
}

// ---------------------------------------------------------------------------------------------------------------
// Interactions

fn interaction_source_hears_presses_releases_cancels_and_keyboard_clicks() {
    // C++'s interaction_source_hears_presses_releases_cancels_and_keyboard_clicks, a clickable box for its Material
    // Button: a press and its release; slid off, a cancel; the keyboard's focus and Enter; gone while focused or
    // pressed; one's own interactions.
    let source = MutableInteractionSource::new();
    let log = new_log();
    let clicks = Rc::new(Cell::new(0));
    let shown = mutable_state_of(true);
    let (s, l, c, sh) = (source.clone(), log.clone(), clicks.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        LogInteractions(s.clone(), l.clone());
        let (s, c, sh) = (s.clone(), c.clone(), sh.clone());
        Column(move || {
            if sh.get() {
                let (s, c) = (s.clone(), c.clone());
                let pressed = s.collect_is_pressed_as_state().get();
                Box(
                    modifier = Modifier
                        .clickable(
                            role = Some(Role::Button),
                            interaction_source = Some(s.clone()),
                            on_click = move || c.set(c.get() + 1),
                        )
                        .test_tag("düğme"),
                    content = move || Text(if pressed { "basılı" } else { "serbest" }),
                );
            }
            Box(modifier = Modifier.size(100.dp()).test_tag("boş"));
        });
    }));
    let pressed = || rule.on_node_with_text("basılı").exists();
    let c = center(&rule, "düğme");
    down(&rule, c);
    rule.wait_for_idle();
    assert!(pressed() && taken(&log) == ["Press"]);
    up(&rule, c);
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("serbest").exists() && taken(&log) == ["Release"] && clicks.get() == 1);
    let away = center(&rule, "boş");
    down(&rule, c); // slid off before letting go: a cancel, no click
    move_to(&rule, away);
    up(&rule, away);
    rule.wait_for_idle();
    assert!(taken(&log) == ["Press", "Cancel"] && clicks.get() == 1 && !pressed());
    // The keyboard: focused, then Enter is a press and its release.
    key(&rule, Key::Tab);
    key(&rule, Key::Enter);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["Focus", "Press", "Release"]);
    assert_eq!(clicks.get(), 2);
    // Pointing elsewhere ends the keyboard focus (touch mode: a clickable isn't focusable then).
    tap(&rule, away);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["Unfocus"]);
    // Gone while focused: an unfocus.
    key(&rule, Key::Tab);
    rule.wait_for_idle();
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["Focus", "Unfocus"]);
    shown.set(true);
    rule.wait_for_idle();
    // Gone while pressed: a cancel.
    let c = center(&rule, "düğme");
    down(&rule, c);
    rule.wait_for_idle();
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["Press", "Cancel"]);
    up(&rule, c);
    rule.wait_for_idle();
    // One's own: a press, then its release, by the same press; another press goes on.
    let press = PressInteraction::Press::new(at(3.0, 4.0));
    source.try_emit(press);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["Press"]);
    let other = PressInteraction::Press::new(Offset::Zero);
    assert!(other != press && PressInteraction::Release::new(press).press == press);
    source.try_emit(other);
    source.try_emit(PressInteraction::Release::new(press));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["Press", "Release"]);
}

fn collected_states_follow_their_interactions() {
    // collect_is_pressed/hovered/focused/dragged_as_state: true while a start isn't ended, each by the one it names.
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (p, h, f, d) = (
            s.collect_is_pressed_as_state().get(),
            s.collect_is_hovered_as_state().get(),
            s.collect_is_focused_as_state().get(),
            s.collect_is_dragged_as_state().get(),
        );
        Text(format!("{p} {h} {f} {d}"));
    }));
    let shows = |text: &str| rule.on_node_with_text(text).exists();
    assert!(shows("false false false false"));
    let (press, enter, focus, start) = (
        PressInteraction::Press::new(Offset::Zero),
        HoverInteraction::Enter::new(),
        FocusInteraction::Focus::new(),
        DragInteraction::Start::new(),
    );
    source.try_emit(press);
    source.try_emit(enter);
    source.try_emit(focus);
    source.try_emit(start);
    rule.wait_for_idle();
    assert!(shows("true true true true"));
    // An end naming another start ends nothing.
    source.try_emit(PressInteraction::Cancel::new(PressInteraction::Press::new(Offset::Zero)));
    rule.wait_for_idle();
    assert!(shows("true true true true"));
    source.try_emit(PressInteraction::Cancel::new(press));
    source.try_emit(HoverInteraction::Exit::new(enter));
    source.try_emit(FocusInteraction::Unfocus::new(focus));
    source.try_emit(DragInteraction::Stop::new(start));
    rule.wait_for_idle();
    assert!(shows("false false false false"));
    let again = DragInteraction::Start::new();
    source.try_emit(again);
    rule.wait_for_idle();
    assert!(shows("false false false true"));
    source.try_emit(DragInteraction::Cancel::new(again));
    rule.wait_for_idle();
    assert!(shows("false false false false"));
}

fn hover_interactions_follow_the_mouse_only() {
    // C++'s hover_interactions_follow_the_mouse_only: a hoverable box and a clickable in it.
    let (bx, button) = (MutableInteractionSource::new(), MutableInteractionSource::new());
    let (b1, b2) = (bx.clone(), button.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (b1, b2) = (b1.clone(), b2.clone());
        Column(move || {
            let b2 = b2.clone();
            Box(
                modifier = Modifier.size(200.dp()).hoverable(b1.clone()).test_tag("kutu"),
                content = move || {
                    Box(
                        modifier = Modifier
                            .clickable(interaction_source = Some(b2.clone()), on_click = || {})
                            .test_tag("düğme"),
                        content = || Text("bas"),
                    );
                },
            );
            Box(modifier = Modifier.size(100.dp()).test_tag("dışarı"));
            Text(format!(
                "kutu {} düğme {}",
                b1.collect_is_hovered_as_state().get(),
                b2.collect_is_hovered_as_state().get()
            ));
        });
    }));
    let shows = |text: &str| rule.on_node_with_text(text).exists();
    let (on_button, in_box, out) = (center(&rule, "düğme"), at(190.0, 190.0), center(&rule, "dışarı"));
    move_to(&rule, in_box);
    rule.wait_for_idle();
    assert!(shows("kutu true düğme false"));
    move_to(&rule, on_button); // over both
    rule.wait_for_idle();
    assert!(shows("kutu true düğme true"));
    move_to(&rule, out);
    rule.wait_for_idle();
    assert!(shows("kutu false düğme false"));
    move_to(&rule, on_button);
    rule.host().pointer_exit(); // the mouse left the window
    rule.wait_for_idle();
    assert!(shows("kutu false düğme false"));
    move_to(&rule, on_button);
    rule.wait_for_idle();
    assert!(shows("kutu true düğme true"));
    tap(&rule, on_button); // a finger: no hover (and the mouse's ends)
    rule.wait_for_idle();
    assert!(shows("kutu false düğme false"));
}

fn focusables_tell_their_focus_and_take_accessibilitys_request() {
    // `focusable`: its focus interactions, `Focused` and the `RequestFocus` action; not enabled, no part of the
    // focus; another source ends the focus told to the old one; a tap doesn't focus it.
    let (first, second) = (MutableInteractionSource::new(), MutableInteractionSource::new());
    let use_second = mutable_state_of(false);
    let enabled = mutable_state_of(true);
    let (l1, l2) = (new_log(), new_log());
    let (f, s, us, en, a, b) =
        (first.clone(), second.clone(), use_second.clone(), enabled.clone(), l1.clone(), l2.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        LogInteractions(f.clone(), a.clone());
        LogInteractions(s.clone(), b.clone());
        let source = if us.get() { s.clone() } else { f.clone() };
        let enabled = en.get();
        Column(move || {
            Box(modifier = Modifier
                .size(40.dp())
                .focusable(enabled = enabled, interaction_source = Some(source.clone()))
                .test_tag("odak"));
            Box(modifier = Modifier.size(40.dp()).focusable().test_tag("öbür"));
        });
    }));
    rule.on_node_with_tag("odak").assert(has_request_focus_action()).assert_is_not_focused();
    rule.on_node_with_tag("odak").request_focus();
    assert!(focused(&rule, "odak"));
    rule.on_node_with_tag("odak").assert_is_focused();
    assert_eq!(taken(&l1), ["Focus"]);
    use_second.set(true); // another source: the old one hears the focus end
    rule.wait_for_idle();
    assert_eq!(taken(&l1), ["Unfocus"]);
    key(&rule, Key::Tab);
    rule.wait_for_idle();
    assert!(focused(&rule, "öbür"));
    assert!(taken(&l2).is_empty()); // (the focus came while its old source had it)
    rule.on_node_with_tag("odak").request_focus();
    assert_eq!(taken(&l2), ["Focus"]);
    // A tap focuses nothing; the focus stays (it is always focusable).
    tap(&rule, center(&rule, "öbür"));
    rule.wait_for_idle();
    assert!(focused(&rule, "odak"));
    enabled.set(false);
    rule.wait_for_idle();
    assert!(rule.host().focused_node().is_none());
    assert_eq!(taken(&l2), ["Unfocus"]);
    rule.on_node_with_tag("odak").assert(SemanticsMatcher::key_not_defined(SemanticsActions::RequestFocus));
}

fn focused_bounds_observers_hear_where_the_focus_is() {
    // `on_focused_bounds_changed`: told the focused focusable's coordinates as it takes the focus, none as it
    // loses it, the nearest observer first.
    let log = new_log();
    let requester = kompoz_ui::focus::FocusRequester::new();
    let (l, r) = (log.clone(), requester.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (outer, inner, r) = (l.clone(), l.clone(), r.clone());
        Box(
            modifier =
                Modifier.on_focused_bounds_changed(move |c| push(&outer, format!("dış {:?}", c.map(|c| c.size())))),
            content = move || {
                let (inner, r) = (inner.clone(), r.clone());
                Box(
                    modifier = Modifier
                        .padding(10.dp())
                        .on_focused_bounds_changed(move |c| push(&inner, format!("iç {:?}", c.map(|c| c.size())))),
                    content = move || Box(modifier = Modifier.size(30.dp()).focus_requester(&r).focusable()),
                );
            },
        );
    }));
    requester.request_focus();
    rule.wait_for_idle();
    let some = format!("{:?}", Some(IntSize::new(30, 30)));
    assert_eq!(taken(&log)[..2], [format!("iç {some}"), format!("dış {some}")]);
    rule.host().clear_focus(true);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["iç None", "dış None"]);
}

fn a_lifted_finger_leaves_no_hover_highlight() {
    // C++'s a_lifted_finger_leaves_no_hover_highlight: the debug indication's hover rect only for a mouse.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(modifier = Modifier.size(100.dp()).clickable(|| {}).test_tag("kutu"));
    }));
    let b = rule.on_node_with_tag("kutu").get_bounds_in_root();
    let highlighted = || {
        let canvas = rule.draw();
        canvas.rects.iter().any(|r| near(r.left, b.left) && near(r.top, b.top) && near(r.width(), b.width()))
    };
    tap(&rule, b.center());
    rule.wait_for_idle();
    assert!(!highlighted());
    move_to(&rule, b.center()); // the mouse over it
    rule.wait_for_idle();
    assert!(highlighted());
}

fn the_default_indication_shows_presses_hovers_and_focus() {
    // Compose's DefaultDebugIndication: black at 0.3 over the content while pressed, 0.1 while hovered or focused,
    // nothing otherwise; made when first needed (a clickable without a source of its own).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(modifier =
            Modifier.size(100.dp()).background(Color::new(1.0, 0.0, 0.0, 1.0)).clickable(|| {}).test_tag("kutu"));
    }));
    let overlay = || {
        let canvas = rule.draw();
        canvas
            .rect_colors
            .iter()
            .filter(|c| c.red() == 0.0 && c.alpha() < 1.0)
            .map(|c| (c.alpha() * 10.0).round() as i32)
            .collect::<Vec<_>>()
    };
    assert!(overlay().is_empty());
    down(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(overlay(), [3]);
    up(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert!(overlay().is_empty());
    move_to(&rule, at(50.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(overlay(), [1]);
    rule.host().pointer_exit();
    key(&rule, Key::Tab);
    rule.wait_for_idle();
    assert!(focused(&rule, "kutu"));
    assert_eq!(overlay(), [1]);
    rule.host().key_down(Key::Enter, KeyModifiers::None);
    rule.wait_for_idle();
    assert_eq!(overlay(), [3]);
    rule.host().key_up(Key::Enter, KeyModifiers::None);
    rule.wait_for_idle();
    assert_eq!(overlay(), [1]);
}

fn a_clickable_can_leave_out_the_indication() {
    // The clickable part of C++ 23625 (draggable_tells_its_drags_and_clickable_can_leave_out_the_indication; its
    // draggable part is gestures.rs' draggable_tells_its_drags): no indication, nothing drawn over it while pressed.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(modifier = Modifier.size(60.dp()).clickable_indication(None, None, on_click = || {}).test_tag("sade"));
            Box(modifier = Modifier.size(60.dp()).clickable(|| {}).test_tag("normal"));
        });
    }));
    let overlaid = |tag: &str| {
        let c = center(&rule, tag);
        let before = rule.draw().rects.len();
        down(&rule, c);
        rule.main_clock().advance_time_by(100);
        let pressed = rule.draw().rects.len();
        up(&rule, c);
        rule.wait_for_idle();
        pressed > before
    };
    assert!(!overlaid("sade") && overlaid("normal"));
}

/// A custom indication (C++'s HeardFactory): its nodes log what they hear and count frames while attached, and
/// draw a mark of its color over the content.
#[derive(Clone)]
struct HeardFactory {
    color: Color,
    made: Rc<RefCell<Vec<Rc<HeardState>>>>,
}

impl PartialEq for HeardFactory {
    fn eq(&self, other: &Self) -> bool {
        self.color == other.color && Rc::ptr_eq(&self.made, &other.made)
    }
}

/// What a heard node heard, whether it is attached, and whether the coroutine it runs while attached is on.
#[derive(Default)]
struct HeardState {
    heard: RefCell<Vec<String>>,
    attached: Cell<bool>,
    running: Cell<bool>,
}

/// Marks a coroutine gone when it is dropped (cancelled).
struct Running(Rc<HeardState>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.running.set(false);
    }
}

impl Indication for HeardFactory {
    fn as_node_factory(&self) -> Option<&dyn IndicationNodeFactory> {
        Some(self)
    }
}

impl IndicationNodeFactory for HeardFactory {
    fn create(&self, interaction_source: InteractionSource) -> std::boxed::Box<dyn ModifierNode> {
        let state = Rc::new(HeardState::default());
        self.made.borrow_mut().push(state.clone());
        std::boxed::Box::new(HeardNode { color: self.color, source: interaction_source, state })
    }
}

struct HeardNode {
    color: Color,
    source: InteractionSource,
    state: Rc<HeardState>,
}

impl ModifierNode for HeardNode {
    node_capabilities!(draw);
    fn on_attach(&mut self, cx: &NodeCx) {
        self.state.attached.set(true);
        let (flow, state) = (self.source.interactions(), self.state.clone());
        let scope = cx.coroutine_scope();
        scope.launch(flow.collect(move |i| state.heard.borrow_mut().push(interaction_name(&*i).to_string())));
        self.state.running.set(true);
        let running = Running(self.state.clone());
        scope.launch(async move {
            let _running = running;
            std::future::pending::<()>().await;
        });
    }
    fn on_detach(&mut self, _cx: &NodeCx) {
        self.state.attached.set(false);
    }
}

impl kompoz_ui::draw::DrawModifierNode for HeardNode {
    fn draw(&mut self, scope: &mut kompoz_ui::draw::ContentDrawScope) {
        scope.draw_content();
        scope.draw_rect(self.color);
    }
}

fn custom_indication_hears_its_component_and_draws_around_it() {
    // C++'s custom_indication_hears_its_component_and_draws_around_it, with Compose's lazily made indication (a
    // clickable without a source of its own makes its node at its first input or focus) and a clickable box for
    // its Material Button.
    let red = HeardFactory { color: Color::new(1.0, 0.0, 0.0, 1.0), made: Default::default() };
    let blue = HeardFactory { color: Color::new(0.0, 0.0, 1.0, 1.0), made: Default::default() };
    let use_blue = mutable_state_of(false);
    let enabled = mutable_state_of(true);
    let tick = mutable_state_of(0);
    let own = MutableInteractionSource::new();
    let (r, bl, ub, en, ti, ow) =
        (red.clone(), blue.clone(), use_blue.clone(), enabled.clone(), tick.clone(), own.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let _ = ti.get(); // recomposed: the same factory keeps its node
        let (r, bl, ub, en, ow) = (r.clone(), bl.clone(), ub.clone(), en.clone(), ow.clone());
        Column(move || {
            let indication: Rc<dyn Indication> = if ub.get() { Rc::new(bl.clone()) } else { Rc::new(r.clone()) };
            Box(modifier = Modifier
                .size_wh(100.dp(), 50.dp())
                .clickable_indication(None, Some(indication), enabled = en.get(), on_click = || {})
                .test_tag("tık"));
            // A component of one's own: its source, drawn by Modifier.indication.
            let blue: Rc<dyn Indication> = Rc::new(bl.clone());
            Box(modifier = Modifier.size_wh(100.dp(), 50.dp()).indication(&ow, Some(blue)).test_tag("kendi"));
            let red: Rc<dyn Indication> = Rc::new(r.clone());
            KompozitionLocalProvider(&[LocalIndication.provides(red)], || {
                Box(modifier = Modifier.size_wh(100.dp(), 50.dp()).clickable(|| {}), content = || Text("Düğme"));
            });
        });
    }));
    // The component of one's own made its node at once; the clickables wait for their first input.
    assert!(red.made.borrow().is_empty() && blue.made.borrow().len() == 1);
    let c = center(&rule, "tık");
    tap(&rule, c);
    rule.wait_for_idle();
    assert_eq!(red.made.borrow().len(), 1);
    let bx = red.made.borrow()[0].clone();
    // (made as the press came: it hears it from its first frame on)
    assert_eq!(*bx.heard.borrow(), ["Press", "Release"]);
    // A mouse going over it and away.
    move_to(&rule, c);
    rule.wait_for_idle();
    rule.host().pointer_exit();
    rule.wait_for_idle();
    assert_eq!(bx.heard.borrow()[2..], ["HoverEnter", "HoverExit"]);
    // The keyboard: focused, then Enter presses it.
    key(&rule, Key::Tab);
    rule.wait_for_idle();
    key(&rule, Key::Enter);
    rule.wait_for_idle();
    assert_eq!(bx.heard.borrow()[4..], ["Focus", "Press", "Release"]);
    key(&rule, Key::Escape); // (the focus off it)
    rule.wait_for_idle();
    // Drawn around the content: its mark over the box (and the button, once it made its node), the other's over
    // the component.
    tap(&rule, rule.on_node_with_text("Düğme").get_bounds_in_root().center());
    rule.wait_for_idle();
    assert_eq!(red.made.borrow().len(), 2);
    let canvas = rule.draw();
    let marks = |c: Color| canvas.rect_colors.iter().filter(|k| **k == c).count();
    assert!(marks(red.color) == 2 && marks(blue.color) == 1);
    // The component's own source: what is emitted to it.
    let press = PressInteraction::Press::new(at(10.0, 10.0));
    own.try_emit(press);
    own.try_emit(PressInteraction::Cancel::new(press));
    rule.wait_for_idle();
    assert_eq!(*blue.made.borrow()[0].heard.borrow(), ["Press", "Cancel"]);
    // Recomposed: the same node. Another factory: a new node, the old one gone with its coroutines.
    tick.set(1);
    rule.wait_for_idle();
    assert!(red.made.borrow().len() == 2 && bx.attached.get());
    use_blue.set(true);
    rule.wait_for_idle();
    assert!(blue.made.borrow().len() == 2 && !bx.attached.get());
    assert!(!bx.running.get()); // its coroutines went with it
    // Disabled: Compose keeps the indication (C++ dropped it); nothing to show, the mark is only drawn.
    enabled.set(false);
    rule.wait_for_idle();
    assert!(blue.made.borrow()[1].attached.get());
}

fn indication_shows_a_sources_presses_on_another_node() {
    // C++'s indication_shows_a_sources_ripple_on_another_node with the debug indication for its ripple: a press on
    // the area, no indication of its own, shows on the icon (its bounds); so does the mouse over it.
    let source = MutableInteractionSource::new();
    let s = source.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Box(
            modifier = Modifier
                .size_wh(200.dp(), 100.dp())
                .clickable_indication(Some(s.clone()), None, on_click = || {})
                .test_tag("alan"),
            content_alignment = Alignment::Center,
            content = move || {
                let debug: Rc<dyn Indication> = Rc::new(DefaultDebugIndication);
                Box(modifier = Modifier.size(48.dp()).indication(&s, Some(debug)).test_tag("simge"));
            },
        );
    }));
    let (area, icon) = (
        rule.on_node_with_tag("alan").get_bounds_in_root(),
        rule.on_node_with_tag_unmerged("simge").get_bounds_in_root(),
    );
    let drawn = || {
        let canvas = rule.draw();
        canvas.rects.iter().zip(&canvas.rect_colors).map(|(r, c)| (*r, c.alpha())).collect::<Vec<_>>()
    };
    down(&rule, at(area.left + 10.0, area.top + 10.0));
    rule.wait_for_idle();
    let pressed = drawn();
    assert!(pressed.len() == 1 && pressed[0].0 == icon && near(pressed[0].1, 0.3), "{pressed:?}");
    up(&rule, at(area.left + 10.0, area.top + 10.0));
    rule.wait_for_idle();
    assert!(drawn().is_empty());
    move_to(&rule, area.center());
    rule.wait_for_idle();
    let hovered = drawn();
    assert!(hovered.len() == 1 && hovered[0].0 == icon && near(hovered[0].1, 0.1));
}

fn a_source_of_a_flow_of_its_own_is_what_an_indication_and_a_state_follow() {
    // Compose's InteractionSource is an interface: any flow can be a source's interactions (Material's navigation
    // items give their indicator ripple one whose presses are moved). Here a source of only the area's presses: the
    // icon's indication shows the press, not the mouse over the area; the pressed state follows it too.
    let source = MutableInteractionSource::new();
    let presses_only = InteractionSource::from_flow(source.interactions().filter(|interaction| {
        interaction.is::<PressInteraction::Press>()
            || interaction.is::<PressInteraction::Release>()
            || interaction.is::<PressInteraction::Cancel>()
    }));
    assert!(presses_only == presses_only.clone() && presses_only != *source);
    let pressed_log = new_log();
    let (s, p, log) = (source.clone(), presses_only.clone(), pressed_log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, p, log) = (s.clone(), p.clone(), log.clone());
        Box(
            modifier = Modifier
                .size_wh(200.dp(), 100.dp())
                .clickable_indication(Some(s.clone()), None, on_click = || {})
                .test_tag("alan"),
            content_alignment = Alignment::Center,
            content = move || {
                let debug: Rc<dyn Indication> = Rc::new(DefaultDebugIndication);
                push(&log, format!("pressed {}", p.collect_is_pressed_as_state().get()));
                Box(modifier = Modifier.size(48.dp()).indication(&p, Some(debug)).test_tag("simge"));
            },
        );
    }));
    let (area, icon) = (
        rule.on_node_with_tag("alan").get_bounds_in_root(),
        rule.on_node_with_tag_unmerged("simge").get_bounds_in_root(),
    );
    let drawn = || {
        let canvas = rule.draw();
        canvas.rects.iter().zip(&canvas.rect_colors).map(|(r, c)| (*r, c.alpha())).collect::<Vec<_>>()
    };
    assert_eq!(taken(&pressed_log), ["pressed false"]);
    move_to(&rule, area.center());
    rule.wait_for_idle();
    assert!(drawn().is_empty(), "the hover isn't in the flow");
    down(&rule, at(area.left + 10.0, area.top + 10.0));
    rule.wait_for_idle();
    let pressed = drawn();
    assert!(pressed.len() == 1 && pressed[0].0 == icon && near(pressed[0].1, 0.3), "{pressed:?}");
    assert_eq!(taken(&pressed_log), ["pressed true"]);
    up(&rule, at(area.left + 10.0, area.top + 10.0));
    rule.wait_for_idle();
    assert!(drawn().is_empty());
    assert_eq!(taken(&pressed_log), ["pressed false"]);
}

// ---------------------------------------------------------------------------------------------------------------
// Focus and keys

fn tab_reaches_buttons_and_enter_or_space_clicks_them() {
    // C++'s tab_reaches_buttons_and_enter_or_space_clicks_them without its text field (stage 6), clickable boxes
    // for its Material Buttons: Tab reaches them (keyboard mode), Enter and Space click them (on their release, as
    // Compose's), a tap ends the keyboard focus and focuses neither a clickable nor a focusable.
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Column(move || {
            let (a, b) = (l.clone(), l.clone());
            Box(
                modifier =
                    Modifier.clickable(role = Some(Role::Button), on_click = move || push(&a, "bir")).test_tag("one"),
                content = || Text("Bir"),
            );
            Box(
                modifier =
                    Modifier.clickable(role = Some(Role::Button), on_click = move || push(&b, "iki")).test_tag("two"),
                content = || Text("İki"),
            );
            Box(modifier = Modifier.size(40.dp()).focusable().test_tag("box"));
        });
    }));
    assert!(key(&rule, Key::Tab) && focused(&rule, "one"));
    assert!(key(&rule, Key::Enter));
    assert_eq!(taken(&log), ["bir"]);
    key(&rule, Key::Tab);
    assert!(focused(&rule, "two") && key(&rule, Key::Spacebar));
    assert_eq!(taken(&log), ["iki"]);
    key(&rule, Key::Tab);
    assert!(focused(&rule, "box"));
    rule.host().key_down(Key::Tab, KeyModifiers::Shift);
    rule.host().key_up(Key::Tab, KeyModifiers::Shift);
    assert!(focused(&rule, "two"));
    rule.wait_for_idle();
    rule.on_node_with_tag("two").assert_is_focused();
    // Pointing ends the keyboard focus; a tap doesn't focus a button or a focusable.
    rule.on_node_with_tag("one").perform_click();
    assert!(rule.host().focused_node().is_none());
    assert_eq!(taken(&log), ["bir"]);
    rule.on_node_with_tag("box").perform_click();
    assert!(rule.host().focused_node().is_none());
}

fn a_clickable_gets_the_keys_a_focused_child_leaves_and_cancels_presses_when_unfocused() {
    // A key a focused child doesn't take bubbles up to the clickable around it; the focus leaving while a key is
    // held cancels that press.
    let source = MutableInteractionSource::new();
    let log = new_log();
    let clicks = Rc::new(Cell::new(0));
    let (s, l, c) = (source.clone(), log.clone(), clicks.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        LogInteractions(s.clone(), l.clone());
        let (s, c) = (s.clone(), c.clone());
        Column(move || {
            let c = c.clone();
            Box(
                modifier = Modifier
                    .clickable(interaction_source = Some(s.clone()), on_click = move || c.set(c.get() + 1))
                    .test_tag("dış"),
                content = || Box(modifier = Modifier.size(40.dp()).focusable().test_tag("iç")),
            );
            Box(modifier = Modifier.size(40.dp()).focusable().test_tag("sonraki"));
        });
    }));
    rule.on_node_with_tag_unmerged("iç").request_focus();
    assert!(focused(&rule, "iç"));
    key(&rule, Key::Enter);
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1);
    assert_eq!(taken(&log), ["Press", "Release"]); // (the clickable itself isn't focused)
    // Back to the clickable around it (in keyboard mode: Shift+Tab).
    rule.host().key_down(Key::Tab, KeyModifiers::Shift);
    rule.host().key_up(Key::Tab, KeyModifiers::Shift);
    rule.wait_for_idle();
    assert!(focused(&rule, "dış"), "{:?}", rule.host().focused_node().map(|n| n.id()));
    assert_eq!(taken(&log), ["Focus"]);
    rule.host().key_down(Key::Spacebar, KeyModifiers::None);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["Press"]);
    rule.on_node_with_tag("sonraki").request_focus(); // the focus leaves while the key is held
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["Cancel", "Unfocus"]); // (the clickable told first, as Compose's)
    rule.host().key_up(Key::Spacebar, KeyModifiers::None);
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 1);
}

// ---------------------------------------------------------------------------------------------------------------
// Semantics

fn clickables_tell_accessibility_their_click_role_and_state() {
    // The merged node: its click action (labelled), role, enabled or disabled, focusable; selectable and toggleable
    // states; a combined clickable's long click.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(
                modifier = Modifier
                    .clickable(on_click_label = Some("aç".to_string()), role = Some(Role::Button), on_click = || {})
                    .test_tag("düğme"),
                content = || Text("Aç"),
            );
            Box(
                modifier = Modifier.clickable(enabled = false, on_click = || {}).test_tag("kapalı"),
                content = || Text("Kapalı"),
            );
            Box(modifier = Modifier
                .size(20.dp())
                .selectable(true, role = Some(Role::RadioButton), on_click = || {})
                .test_tag("seçili"));
            Box(modifier = Modifier.size(20.dp()).selectable(false, on_click = || {}).test_tag("seçilmemiş"));
            Box(modifier = Modifier
                .size(20.dp())
                .toggleable(true, role = Some(Role::Switch), on_value_change = |_| {})
                .test_tag("açık"));
            Box(modifier = Modifier
                .size(20.dp())
                .tri_state_toggleable(ToggleableState::Indeterminate, role = Some(Role::Checkbox), on_click = || {})
                .test_tag("kısmen"));
        });
    }));
    let button = rule.on_node_with_tag("düğme");
    button.assert_has_click_action().assert_is_enabled().assert(has_role(Role::Button)).assert(is_focusable());
    button.assert_text_equals(&["Aç"]); // merged
    let config = button.fetch_semantics_node().config();
    assert_eq!(config.get(SemanticsActions::OnClick).and_then(|a| a.label).as_deref(), Some("aç"));
    rule.on_node_with(has_text("Aç", false, false), true).assert(has_no_click_action());
    rule.on_node_with_tag("kapalı").assert_is_not_enabled().assert_has_click_action().assert(is_not_focusable());
    rule.on_node_with_tag("seçili").assert_is_selectable().assert_is_selected().assert(has_role(Role::RadioButton));
    rule.on_node_with_tag("seçilmemiş").assert_is_not_selected();
    rule.on_node_with_tag("açık").assert_is_toggleable().assert_is_on().assert(has_role(Role::Switch));
    let tri = rule.on_node_with_tag("kısmen").fetch_semantics_node().config();
    assert_eq!(tri.get(SemanticsProperties::ToggleableState), Some(ToggleableState::Indeterminate));
    assert_eq!(tri.get(SemanticsProperties::FillableData), Some(FillableData::Boolean(false)));
    assert_eq!(tri.get(SemanticsProperties::ContentDataType), Some(ContentDataType::Toggle));
}

fn selectable_and_toggleable_rows_take_the_tap_for_their_controls() {
    // C++'s selectable_and_toggleable_rows_take_the_tap_for_their_controls with plain boxes for its Material
    // controls: a tap anywhere in the row (on its control too, which has no callback) is the row's.
    let choice = mutable_state_of(0);
    let on = mutable_state_of(false);
    let tri = mutable_state_of(ToggleableState::Off);
    let (ch, o, t) = (choice.clone(), on.clone(), tri.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (ch, o, t) = (ch.clone(), o.clone(), t.clone());
        Column(
            modifier = Modifier.selectable_group().test_tag("grup"),
            content = move || {
                for i in 0..3 {
                    let ch = ch.clone();
                    let selected = ch.get() == i;
                    Row(
                        modifier = Modifier
                            .fill_max_width()
                            .height(56.dp())
                            .selectable(selected, role = Some(Role::RadioButton), on_click = move || ch.set(i))
                            .test_tag(format!("satır {i}")),
                        vertical_alignment = Alignment::CenterVertically,
                        content = move || {
                            Box(modifier = Modifier.size(20.dp()).test_tag(format!("düğme {i}")));
                            Text(format!("seçenek {i}"));
                        },
                    );
                }
                let o = o.clone();
                let value = o.get();
                Row(
                    modifier = Modifier
                        .fill_max_width()
                        .height(56.dp())
                        .toggleable(value, role = Some(Role::Switch), on_value_change = move |v| o.set(v))
                        .test_tag("anahtar satırı"),
                    content = || {
                        Text("Bildirimler");
                        Box(modifier = Modifier.size(20.dp()).test_tag("anahtar"));
                    },
                );
                let t = t.clone();
                let state = t.get();
                let next = move || {
                    t.set(match t.peek() {
                        ToggleableState::Off => ToggleableState::Indeterminate,
                        ToggleableState::Indeterminate => ToggleableState::On,
                        ToggleableState::On => ToggleableState::Off,
                    })
                };
                Row(
                    modifier = Modifier
                        .fill_max_width()
                        .height(56.dp())
                        .tri_state_toggleable(state, role = Some(Role::Checkbox), on_click = next)
                        .test_tag("üç"),
                    content = || Box(modifier = Modifier.size(20.dp()).test_tag("kutu")),
                );
            },
        );
    }));
    rule.on_node_with_tag("satır 0").assert_is_selected();
    rule.on_node_with_tag_unmerged("düğme 2").perform_click(); // the row takes the tap
    assert_eq!(choice.peek(), 2);
    rule.on_node_with_tag("satır 2").assert_is_selected();
    rule.on_node_with_tag("satır 0").assert_is_not_selected();
    rule.on_node_with_text("seçenek 1").perform_click();
    assert_eq!(choice.peek(), 1);
    rule.on_node_with_tag_unmerged("anahtar").perform_click();
    assert!(on.peek());
    rule.on_node_with_tag("anahtar satırı").assert_is_on();
    rule.on_node_with_tag("anahtar satırı").perform_click();
    assert!(!on.peek());
    rule.on_node_with_tag("anahtar satırı").assert_is_off();
    rule.on_node_with_tag_unmerged("kutu").perform_click();
    assert_eq!(tri.peek(), ToggleableState::Indeterminate);
    let s = rule.on_node_with_tag("üç").fetch_semantics_node().config();
    assert!(
        s.get(SemanticsProperties::ToggleableState) == Some(ToggleableState::Indeterminate)
            && s.get(SemanticsProperties::Role) == Some(Role::Checkbox)
    );
    assert!(
        rule.on_node_with_tag("grup").fetch_semantics_node().config().contains(SemanticsProperties::SelectableGroup)
    );
    rule.on_node_with_tag_unmerged("düğme 0").assert(has_no_click_action()); // no callback, not clickable
}

fn selectable_groups_are_told_to_accessibility() {
    // C++'s selectable_groups_are_told_to_accessibility with its selectables.
    let chosen = mutable_state_of(1);
    let ch = chosen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let ch = ch.clone();
        Row(
            modifier = Modifier.selectable_group().test_tag("grup"),
            content = move || {
                for i in 0..3 {
                    let ch = ch.clone();
                    let selected = ch.get() == i;
                    Box(modifier = Modifier.size(40.dp()).selectable(
                        selected,
                        role = Some(Role::RadioButton),
                        on_click = move || ch.set(i),
                    ));
                }
            },
        );
        Row(modifier = Modifier.test_tag("düz"), content = || Text("grup değil"));
    }));
    let tree = rule.host().accessibility_tree();
    let groups: Vec<_> = tree.iter().filter(|n| n.selectable_group).collect();
    assert_eq!(groups.len(), 1);
    let items: Vec<_> = tree.iter().filter(|n| n.parent == Some(groups[0].id)).collect();
    assert!(items.len() == 3 && items.iter().filter(|n| n.checked == Some(true)).count() == 1);
    assert!(rule.host().perform_semantics_action(items[2].id, SemanticsAction::Click));
    rule.wait_for_idle();
    assert_eq!(chosen.peek(), 2);
}

// ---------------------------------------------------------------------------------------------------------------
// The magnifier

fn magnifier_draws_the_content_zoomed_over_it() {
    // C++'s magnifier_draws_the_scene_zoomed_over_everything: 100 x 48 at 1.25 times, the 80 x 38.4 around the
    // source (70, 80 in the window: past the padding before the magnifier), moved 42 up; unspecified: none.
    let source = mutable_state_of(Offset::new(50.0, 60.0));
    let sizes = Rc::new(RefCell::new(Vec::new()));
    let (s, sz) = (source.clone(), sizes.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, sz) = (s.clone(), sz.clone());
        let told: Rc<dyn Fn(DpSize)> = Rc::new(move |size| sz.borrow_mut().push(size));
        Box(
            modifier =
                Modifier.padding(20.dp()).size(300.dp()).magnifier(move |_| s.get(), on_size_changed = Some(told)),
            content = || Text("büyüt"),
        );
    }));
    let zoomed = || {
        let canvas = rule.draw();
        canvas
            .layer_transforms
            .iter()
            .position(|t| (t.scale_x - 1.25).abs() < 0.01)
            .map(|i| (canvas.layer_bounds[i], canvas.layer_transforms[i], canvas.texts.clone()))
    };
    let (bounds, transform, texts) = zoomed().expect("a zoomed layer");
    assert!(near(bounds.left, 70.0 - 40.0) && near(bounds.top, 80.0 - 19.2) && near(bounds.width(), 80.0));
    assert!(near(bounds.height(), 38.4) && near(transform.translation_x, 0.0) && near(transform.translation_y, -42.0));
    assert_eq!(texts, ["büyüt", "büyüt"]); // the content, then the content zoomed
    assert_eq!(*sizes.borrow(), [DpSize::new(100.dp(), 48.dp())]);
    let position = rule
        .on_root()
        .on_children()
        .on_first()
        .fetch_semantics_node()
        .config()
        .get(kompoz_foundation::magnifier::MagnifierPositionInRoot);
    assert_eq!(position.map(|p| (p.0)()), Some(at(70.0, 80.0)));
    source.set(Offset::new(f32::NAN, 0.0)); // NaN: none
    rule.wait_for_idle();
    assert!(zoomed().is_none());
    // Kept in the window: a source by its top edge puts the loupe below the edge, not past it.
    source.set(Offset::new(50.0, 0.0));
    rule.wait_for_idle();
    let (bounds, transform, _) = zoomed().expect("a zoomed layer");
    assert!(near(bounds.top, 20.0 - 19.2) && near(transform.translation_y, 24.0 - 20.0), "{bounds:?} {transform:?}");
}

/// An indication marking its component while pressed (C++'s PressMarkFactory).
#[derive(PartialEq)]
struct PressMark;

impl Indication for PressMark {
    fn as_node_factory(&self) -> Option<&dyn IndicationNodeFactory> {
        Some(self)
    }
}

impl IndicationNodeFactory for PressMark {
    fn create(&self, interaction_source: InteractionSource) -> std::boxed::Box<dyn ModifierNode> {
        std::boxed::Box::new(PressMarkNode { source: interaction_source, pressed: Rc::new(Cell::new(false)) })
    }
}

struct PressMarkNode {
    source: InteractionSource,
    pressed: Rc<Cell<bool>>,
}

impl ModifierNode for PressMarkNode {
    node_capabilities!(draw);
    fn on_attach(&mut self, cx: &NodeCx) {
        let (flow, pressed, handle) = (self.source.interactions(), self.pressed.clone(), cx.handle());
        let mut presses = Vec::new();
        cx.coroutine_scope().launch(flow.collect(move |i| {
            if let Some(p) = i.downcast_ref::<PressInteraction::Press>() {
                presses.push(*p);
            } else if let Some(r) = i.downcast_ref::<PressInteraction::Release>() {
                presses.retain(|p| *p != r.press);
            } else if let Some(c) = i.downcast_ref::<PressInteraction::Cancel>() {
                presses.retain(|p| *p != c.press);
            }
            pressed.set(!presses.is_empty());
            handle.invalidate_draw();
        }));
    }
}

impl kompoz_ui::draw::DrawModifierNode for PressMarkNode {
    fn draw(&mut self, scope: &mut kompoz_ui::draw::ContentDrawScope) {
        scope.draw_content();
        if self.pressed.get() {
            scope.draw_circle(Color::new(1.0, 0.0, 1.0, 1.0));
        }
    }
}

fn nodes_are_drawn_again_when_what_they_draw_from_changes() {
    // C++'s nodes_are_drawn_again_when_what_they_draw_from_changes: the rule's keep check (a display list keeping
    // what didn't change, against one drawing everything, every frame) finds nothing to tell as presses show and
    // go — the debug indication, a mark through another's source — and as what is drawn into a layer changes.
    let shared = MutableInteractionSource::new();
    let tint = mutable_state_of(Color::new(1.0, 0.0, 0.0, 1.0));
    let (sh, ti) = (shared.clone(), tint.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let layer = kompoz_ui::draw::remember_graphics_layer();
        let (sh, ti) = (sh.clone(), ti.clone());
        Column(move || {
            Box(modifier = Modifier.size_wh(100.dp(), 50.dp()).clickable(|| {}).test_tag("düz"));
            // Pressed through another's source: only its indication hears of it.
            let mark: Rc<dyn Indication> = Rc::new(PressMark);
            Box(modifier = Modifier.size_wh(100.dp(), 50.dp()).clickable_indication(
                Some(sh.clone()),
                Some(mark),
                on_click = || {},
            ));
            Box(modifier = Modifier
                .size_wh(100.dp(), 50.dp())
                .clickable_indication(Some(sh.clone()), None, on_click = || {})
                .test_tag("kaynak"));
            // Drawn into a layer of its own and that drawn: the nodes in it are drawn there.
            let (layer, ti) = (layer.clone(), ti.clone());
            Box(
                modifier = Modifier.size_wh(100.dp(), 50.dp()).draw_with_content(move |d| {
                    d.record_with(&layer, IntSize::new(100, 50), |c| c.draw_content());
                    d.draw_layer(&layer);
                }),
                content = move || {
                    let ti = ti.clone();
                    Box(
                        modifier = Modifier.padding(4.dp()),
                        content = move || {
                            let ti = ti.clone();
                            Box(modifier = Modifier.size(20.dp()).draw_behind(move |d| d.draw_rect(ti.get())));
                        },
                    );
                },
            );
        });
    }));
    // A press, and the screen drawn before the next frame: the press still shows next frame; let go, it goes.
    let flat = center(&rule, "düz");
    down(&rule, flat);
    let _ = rule.drawn_texts();
    rule.advance_frame();
    rule.main_clock().advance_time_by(100);
    up(&rule, flat);
    rule.wait_for_idle();
    // Pressed again, a second finger down too: the press goes on (Compose's), then both let go.
    down_id(&rule, flat, 0);
    rule.main_clock().advance_time_by(100);
    down_id(&rule, at(flat.x + 30.0, flat.y), 1);
    rule.main_clock().advance_time_by(100);
    up_id(&rule, at(flat.x + 30.0, flat.y), 1);
    up_id(&rule, flat, 0);
    rule.wait_for_idle();
    // Its source pressed elsewhere: the mark shows, then goes.
    let other = center(&rule, "kaynak");
    down(&rule, other);
    rule.main_clock().advance_time_by(100);
    let marked = || {
        let c = rule.draw();
        let magenta = Color::new(1.0, 0.0, 1.0, 1.0);
        c.rect_colors.contains(&magenta) || c.path_colors.contains(&magenta)
    };
    assert!(marked());
    up(&rule, other);
    rule.wait_for_idle();
    assert!(!marked());
    // What is drawn into the layer changes, deep in it.
    tint.set(Color::new(0.0, 0.0, 1.0, 1.0));
    rule.wait_for_idle();
    assert!(rule.draw().rect_colors.contains(&Color::new(0.0, 0.0, 1.0, 1.0)));
    drop(shared);
}

// ---------------------------------------------------------------------------------------------------------------
// State that goes with what is clicked

/// C++'s Item: a row that remembers its own click count, clicked through the UI.
#[kompozable]
fn Item(id: i32) {
    let clicks = remember(|| mutable_state_of(0));
    let c = clicks.clone();
    Text(format!("item{id}:{}", clicks.get()), modifier = Modifier.clickable(move || c.set(c.get() + 1)));
}

fn key_keeps_state_with_its_item() {
    // C++'s key_keeps_state_with_its_item, its items clicked (kompoz-runtime's port clicks their states).
    let ids = mutable_state_of(vec![1, 2, 3]);
    let i = ids.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let i = i.clone();
        Column(move || {
            for id in i.get() {
                kompoz_runtime::key(&id, || Item(id));
            }
        });
    }));
    rule.on_node_with_text("item2:0").perform_click();
    rule.on_node_with_text("item2:1").assert_exists();
    ids.update(|v| {
        v.remove(0);
    }); // item 1 removed
    rule.wait_for_idle();
    rule.on_node_with_text("item2:1").assert_exists(); // the state moved with item 2
    rule.on_node_with_text("item3:0").assert_exists();
}

fn without_key_state_is_positional() {
    // C++'s without_key_state_is_positional, its items clicked.
    let ids = mutable_state_of(vec![1, 2]);
    let i = ids.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let i = i.clone();
        Column(move || {
            for id in i.get() {
                Item(id);
            }
        });
    }));
    rule.on_node_with_text("item1:0").perform_click();
    ids.update(|v| {
        v.remove(0);
    });
    rule.wait_for_idle();
    rule.on_node_with_text("item2:1").assert_exists(); // as in Compose without key()
}

fn movable_content_with_parameters_keeps_its_state_where_it_goes() {
    // C++'s movable_content_with_parameters_keeps_its_state_where_it_goes (its receiver part is kompoz-runtime's):
    // moved into a Row with new parameters, its clicks came along.
    let wide = mutable_state_of(false);
    let title = mutable_state_of("bir".to_string());
    let (w, t) = (wide.clone(), title.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let card = remember(|| {
            movable_content_with_param_of(|p: &(String, i32)| {
                let (name, n) = p.clone();
                let clicks = remember(|| mutable_state_of(0));
                let c = clicks.clone();
                Text(
                    format!("{name} {n} {}", clicks.get()),
                    modifier = Modifier.test_tag("kart").clickable(move || c.set(c.peek() + 1)),
                );
            })
        });
        let name = t.get();
        if w.get() {
            Row(move || card.call((name.clone(), 2)));
        } else {
            Column(move || card.call((name.clone(), 1)));
        }
    }));
    rule.on_node_with_tag("kart").perform_click();
    rule.on_node_with_tag("kart").assert_text_equals(&["bir 1 1"]);
    wide.set(true);
    title.set("iki".into());
    rule.wait_for_idle();
    rule.on_node_with_tag("kart").assert_text_equals(&["iki 2 1"]);
}

fn movable_content_leaves_a_container_from_two_groups_down() {
    // C++'s movable_content_leaves_a_container_from_two_groups_down: the container goes while the content moves out
    // of a group inside it to elsewhere; its node is taken from the old parent before it joins the new one.
    let inside = mutable_state_of(true);
    let i = inside.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let tile = remember(|| {
            movable_content_of(move || {
                let clicks = remember(|| mutable_state_of(0));
                let c = clicks.clone();
                Text(
                    format!("kutu {}", clicks.get()),
                    modifier = Modifier.test_tag("kutu").clickable(move || c.set(c.peek() + 1)),
                );
            })
        });
        let inside = i.get();
        let (a, b) = (tile.clone(), tile.clone());
        Column(move || {
            let a = a.clone();
            if inside {
                Box(
                    modifier = Modifier.test_tag("dış"),
                    content = move || {
                        let a = a.clone();
                        Box(move || {
                            Text("önce"); // siblings: the empty place is sorted among them
                            a.call();
                            Text("sonra");
                        });
                    },
                );
            }
        });
        Row(move || {
            if !inside {
                b.call();
            }
        });
    }));
    rule.on_node_with_tag("kutu").perform_click();
    rule.wait_for_idle();
    inside.set(false);
    rule.wait_for_idle();
    rule.on_node_with_tag("dış").assert_does_not_exist();
    rule.on_node_with_tag("kutu").assert_text_equals(&["kutu 1"]); // moved, its state with it
}

fn movable_content_keeps_its_state_wherever_it_goes() {
    // C++'s movable_content_keeps_its_state_wherever_it_goes: clicked counts, nodes and running effects go along
    // from a Row to a Column and from one box to another; called twice, the second is a new instance; no longer
    // called, it leaves once; back, from scratch.
    let wide = mutable_state_of(true);
    let in_first = mutable_state_of(true);
    let shown = mutable_state_of(true);
    let twice = mutable_state_of(false);
    let counts: Rc<RefCell<std::collections::HashMap<String, i32>>> = Default::default();
    let (w, f, s, tw, co) = (wide.clone(), in_first.clone(), shown.clone(), twice.clone(), counts.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let counter = |name: &'static str, counts: Rc<RefCell<std::collections::HashMap<String, i32>>>| {
            movable_content_of(move || {
                let n = remember(|| mutable_state_of(0));
                let (started, ended) = (counts.clone(), counts.clone());
                LaunchedEffect(0, async move {
                    *started.borrow_mut().entry(format!("{name} başladı")).or_default() += 1;
                });
                DisposableEffect(0, move || {
                    let ended = ended.clone();
                    on_dispose(move || *ended.borrow_mut().entry(format!("{name} bitti")).or_default() += 1)
                });
                let m = n.clone();
                Text(
                    format!("{name} {}", n.get()),
                    modifier = Modifier.clickable(move || m.set(m.peek() + 1)).test_tag(name),
                );
            })
        };
        let card = remember(|| counter("kart", co.clone()));
        let chip = remember(|| counter("çip", co.clone()));
        let (wide, first, shown, twice) = (w.get(), f.get(), s.get(), tw.get());
        Column(move || {
            let card = card.clone();
            if wide {
                Row(
                    modifier = Modifier.test_tag("satır"),
                    content = move || {
                        if shown {
                            card.call();
                        }
                    },
                );
            } else {
                Column(
                    modifier = Modifier.test_tag("sütun"),
                    content = move || {
                        if shown {
                            card.call();
                        }
                        if twice {
                            card.call(); // a second instance
                        }
                    },
                );
            }
            let (a, b) = (chip.clone(), chip.clone());
            Box(
                modifier = Modifier.test_tag("bir"),
                content = move || {
                    if first {
                        a.call();
                    }
                },
            );
            Box(
                modifier = Modifier.test_tag("iki"),
                content = move || {
                    if !first {
                        b.call();
                    }
                },
            );
        });
    }));
    let count = |key: &str| counts.borrow().get(key).copied().unwrap_or(0);
    let parent_of = |tag: &str| rule.on_node_with_tag(tag).layout_node().parent();
    rule.on_node_with_tag("kart").perform_click();
    rule.on_node_with_tag("kart").perform_click();
    rule.on_node_with_tag("çip").perform_click();
    rule.on_node_with_text("kart 2").assert_exists();
    rule.on_node_with_text("çip 1").assert_exists();
    // The Row becomes a Column: the card's count, its node and its running effects go along.
    wide.set(false);
    rule.wait_for_idle();
    rule.on_node_with_text("kart 2").assert_exists();
    assert!(count("kart başladı") == 1 && count("kart bitti") == 0);
    let column = rule.on_node_with_tag("sütun").layout_node();
    assert!(parent_of("kart").is_some_and(|p| Rc::ptr_eq(&p, &column)));
    // The first box lets go of the chip and the second, composed after it, takes it.
    in_first.set(false);
    rule.wait_for_idle();
    rule.on_node_with_text("çip 1").assert_exists();
    assert!(count("çip başladı") == 1 && count("çip bitti") == 0);
    let second = rule.on_node_with_tag("iki").layout_node();
    assert!(parent_of("çip").is_some_and(|p| Rc::ptr_eq(&p, &second)));
    // Called twice: the second is a new instance, starting over.
    twice.set(true);
    rule.wait_for_idle();
    rule.on_node_with_text("kart 2").assert_exists();
    rule.on_node_with_text("kart 0").assert_exists();
    assert_eq!(count("kart başladı"), 2);
    twice.set(false);
    rule.wait_for_idle();
    assert_eq!(count("kart bitti"), 1);
    rule.on_node_with_text("kart 2").assert_exists();
    // No longer called: it leaves the composition once.
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(count("kart bitti"), 2);
    rule.on_node_with_text("kart 2").assert_does_not_exist();
    shown.set(true); // back: from scratch
    rule.wait_for_idle();
    rule.on_node_with_text("kart 0").assert_exists();
    assert_eq!(count("kart başladı"), 3);
}

tests!(
    counter_increments_on_click,
    clicks_come_by_touch_mouse_keys_and_accessibility,
    tap_handlers_and_clicks_respect_consumption,
    presses_move_out_cancel_and_disabled_takes_none,
    a_press_taken_in_the_final_pass_is_cancelled,
    combined_clickable_long_and_double_clicks,
    combined_clickable_without_a_double_click_clicks_at_once_and_keys_long_click,
    platform_services_open_links_and_long_clicks_are_felt,
    the_view_configuration_times_a_long_click,
    leaving_content_takes_no_taps,
    small_touch_targets_take_touches_near_them,
    z_index_puts_a_sibling_above_the_others,
    children_measured_but_not_placed_are_not_drawn_hit_or_told,
    a_two_finger_tap_is_a_click,
    interaction_source_hears_presses_releases_cancels_and_keyboard_clicks,
    collected_states_follow_their_interactions,
    hover_interactions_follow_the_mouse_only,
    focusables_tell_their_focus_and_take_accessibilitys_request,
    focused_bounds_observers_hear_where_the_focus_is,
    a_lifted_finger_leaves_no_hover_highlight,
    the_default_indication_shows_presses_hovers_and_focus,
    a_clickable_can_leave_out_the_indication,
    custom_indication_hears_its_component_and_draws_around_it,
    indication_shows_a_sources_presses_on_another_node,
    a_source_of_a_flow_of_its_own_is_what_an_indication_and_a_state_follow,
    tab_reaches_buttons_and_enter_or_space_clicks_them,
    a_clickable_gets_the_keys_a_focused_child_leaves_and_cancels_presses_when_unfocused,
    clickables_tell_accessibility_their_click_role_and_state,
    selectable_and_toggleable_rows_take_the_tap_for_their_controls,
    selectable_groups_are_told_to_accessibility,
    magnifier_draws_the_content_zoomed_over_it,
    nodes_are_drawn_again_when_what_they_draw_from_changes,
    key_keeps_state_with_its_item,
    without_key_state_is_positional,
    movable_content_with_parameters_keeps_its_state_where_it_goes,
    movable_content_keeps_its_state_wherever_it_goes,
    movable_content_leaves_a_container_from_two_groups_down,
);

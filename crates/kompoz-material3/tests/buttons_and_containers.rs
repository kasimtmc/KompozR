//! Buttons and containers together, and the Material halves of earlier tests about buttons and clickables: the
//! C++ Kompoz's tests by name whose foundation or ui half is in a lower crate (a clickable box standing in for the
//! Material Button there), here with Material's buttons as the C++ test has them. Parts that need components of
//! later packages say which. Numbers in px at density 1.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::lazy::index_key;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::Text;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::input::*;
use kompoz_ui::semantics::*;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn center(rule: &KompozTestRule, tag: &str) -> Offset {
    rule.on_node_with_tag_unmerged(tag).get_bounds_in_root().center()
}

fn down(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
}

fn move_to(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_move(p, -1, 0);
}

fn up(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_up(p, -1, 0);
}

/// C++'s `tap`: down, a frame, up, a frame.
fn tap(rule: &KompozTestRule, p: Offset) {
    down(rule, p);
    rule.advance_frame();
    up(rule, p);
    rule.advance_frame();
}

/// A key pressed and let go.
fn key(rule: &KompozTestRule, k: Key) -> bool {
    let taken = rule.host().key_down(k, KeyModifiers::None);
    rule.host().key_up(k, KeyModifiers::None);
    taken
}

/// Shift and a key.
fn shift_key(rule: &KompozTestRule, k: Key) -> bool {
    let taken = rule.host().key_down(k, KeyModifiers::Shift);
    rule.host().key_up(k, KeyModifiers::Shift);
    taken
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
    } else {
        "?"
    }
}

/// C++'s LogInteractions: what `source` emits, logged while it is composed.
#[kompozable(inline)]
fn LogInteractions(source: MutableInteractionSource, log: Log) {
    let flow = source.interactions();
    LaunchedEffect(source.clone(), async move {
        flow.collect(move |i| log.borrow_mut().push(interaction_name(&*i).to_string())).await;
    });
}

// ---- The Material halves of earlier tests ------------------------------------------------------------------------

fn counter_increments_on_click() {
    // C++'s counter_increments_on_click with its Material Button (foundation's clickable.rs has it with a clickable
    // text).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let count = remember(|| mutable_state_of(0));
        Column(move || {
            Text(format!("Count: {}", count.get()));
            let count = count.clone();
            Button(move || count.set(count.get() + 1), content = || Text("+"));
        });
    }));
    rule.on_node_with_text("Count: 0").assert_exists();
    rule.on_node_with_text("+").perform_click();
    rule.on_node_with_text("+").perform_click();
    rule.on_node_with_text("Count: 2").assert_exists();
    rule.on_node_with_text("Count: 0").assert_does_not_exist();
}

fn interaction_source_hears_presses_releases_cancels_and_keyboard_clicks() {
    // C++'s test with its Material Button given the source: a press and its release; slid off, a cancel; the
    // keyboard's focus and Enter; gone while focused or pressed (foundation's clickable.rs has the clickable's and
    // one's own interactions).
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
                let s2 = s.clone();
                Button(
                    move || c.set(c.get() + 1),
                    modifier = Modifier.test_tag("düğme"),
                    interaction_source = Some(s.clone()),
                    content = move || {
                        Text(if s2.collect_is_pressed_as_state().get() { "basılı" } else { "serbest" });
                    },
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
    // Pointing elsewhere ends the keyboard focus.
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
}

fn custom_indication_hears_its_component_and_draws_around_it() {
    // C++'s test's Button: it took the LocalIndication around it; Compose's Button (its Surface) shows its
    // interactions by `ripple()` whatever LocalIndication is — so a debug indication provided around it draws
    // nothing over it, and the ripple shows its press (foundation's clickable.rs has the clickables' part).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let debug: Rc<dyn Indication> = Rc::new(DefaultDebugIndication);
        KompozitionLocalProvider(&[LocalIndication.provides(debug)], || {
            Column(|| {
                Box(modifier = Modifier.size_wh(100.dp(), 50.dp()).clickable(|| {}).test_tag("kutu"));
                Button(|| {}, modifier = Modifier.test_tag("düğme"), content = || Text("Düğme"));
            });
        });
    }));
    rule.main_clock().set_auto_advance(false);
    // (the debug indication's press: black at 0.3 over the whole node)
    let dimmed = |r: Rect| {
        let canvas = rule.draw();
        canvas
            .rects
            .iter()
            .zip(&canvas.rect_colors)
            .any(|(x, c)| *x == r && c.red() == 0.0 && near(c.alpha(), 0.3, 0.01))
    };
    for (tag, takes) in [("kutu", true), ("düğme", false)] {
        let c = center(&rule, tag);
        down(&rule, c);
        rule.main_clock().advance_time_by(200);
        assert_eq!(
            dimmed(bounds(&rule, tag)),
            takes,
            "{tag}: the box takes the indication around it, the button its ripple"
        );
        up(&rule, c);
        rule.main_clock().advance_time_by(500);
    }
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
}

fn tab_reaches_buttons_and_enter_or_space_clicks_them() {
    // C++'s test with its Material Buttons (its TextField: WP3's text fields; foundation's clickable.rs has it with
    // clickable boxes): Tab reaches them, Enter and Space click them, Shift+Tab back; a tap doesn't focus them.
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Column(move || {
            let (a, b) = (l.clone(), l.clone());
            Button(
                move || a.borrow_mut().push("bir".into()),
                modifier = Modifier.test_tag("one"),
                content = || Text("Bir"),
            );
            Button(
                move || b.borrow_mut().push("iki".into()),
                modifier = Modifier.test_tag("two"),
                content = || Text("İki"),
            );
            Box(modifier = Modifier.size(40.dp()).focusable().test_tag("box"));
        });
    }));
    assert!(key(&rule, Key::Tab) && focused(&rule, "one"));
    assert!(key(&rule, Key::Enter));
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["bir"]);
    key(&rule, Key::Tab);
    assert!(focused(&rule, "two"));
    assert!(key(&rule, Key::Spacebar));
    rule.wait_for_idle();
    assert_eq!(log.borrow().last().map(String::as_str), Some("iki"));
    key(&rule, Key::Tab);
    assert!(focused(&rule, "box"));
    shift_key(&rule, Key::Tab);
    assert!(focused(&rule, "two"));
    // Pointing ends keyboard focus; a tap doesn't focus a button.
    rule.on_node_with_tag("one").perform_click();
    rule.wait_for_idle();
    assert!(rule.host().focused_node().is_none() && log.borrow().last().map(String::as_str) == Some("bir"));
}

fn a_focus_request_brings_the_node_into_view() {
    // C++'s test with its Material Button above (its TextField below: WP3's; a focusable box here, foundation's
    // scroll.rs has it with a BasicTextField): a focus request far below scrolls it into view, the button's back up.
    // (A clickable takes the focus only out of touch mode, as Compose's: the keyboard's mode asked for, as a Compose
    // test does through LocalInputModeManager.)
    let scroll = ScrollState::new(0);
    let (below, above) = (FocusRequester::new(), FocusRequester::new());
    let modes: Rc<RefCell<Option<kompoz_ui::platform::InputModeManager>>> = Rc::new(RefCell::new(None));
    let (s, b, a, m) = (scroll.clone(), below.clone(), above.clone(), modes.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *m.borrow_mut() = Some(kompoz_ui::platform::LocalInputModeManager.current());
            let (b, a) = (b.clone(), a.clone());
            Column(
                modifier = Modifier.fill_max_size().vertical_scroll(&s),
                content = move || {
                    Button(
                        || {},
                        modifier = Modifier.height(60.dp()).focus_requester(&a).test_tag("üst"),
                        content = || Text("Üst"),
                    );
                    Box(modifier = Modifier.height(1500.dp()));
                    Box(modifier = Modifier.size(56.dp()).focus_requester(&b).focusable().test_tag("alan"));
                    Box(modifier = Modifier.height(1500.dp()));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.run_on_idle(|| modes.borrow().as_ref().unwrap().request_input_mode(kompoz_ui::platform::InputMode::Keyboard));
    below.request_focus();
    rule.wait_for_idle();
    let field = bounds(&rule, "alan");
    assert!(field.top >= 0.0 && near(field.bottom, 800.0, 1.0), "{field:?}");
    above.request_focus();
    rule.wait_for_idle();
    assert!(
        near(bounds(&rule, "üst").top, 0.0, 0.5) && scroll.value() == 0,
        "{:?} {}",
        bounds(&rule, "üst"),
        scroll.value()
    );
}

fn focus_moved_by_keys_scrolls_into_view_and_so_does_the_keyboard() {
    // C++'s test's Material Buttons (its TextField and the keyboard part: WP3's; foundation's scroll.rs has it with
    // focusable boxes and a BasicTextField): Tab brings the focused button into view, Shift+Tab scrolls back up.
    let scroll = ScrollState::new(0);
    let s = scroll.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            Column(
                modifier = Modifier.fill_max_size().vertical_scroll(&s),
                content = || {
                    for i in 0..20 {
                        Button(
                            || {},
                            modifier = Modifier.height(60.dp()).test_tag(format!("b{i}")),
                            content = move || Text(format!("Düğme {i}")),
                        );
                    }
                },
            );
        }),
        IntSize::new(400, 800),
    );
    for _ in 0..15 {
        key(&rule, Key::Tab);
    }
    rule.wait_for_idle();
    let b14 = bounds(&rule, "b14");
    assert!(focused(&rule, "b14") && b14.top >= 0.0 && b14.bottom <= 800.0 && scroll.value() > 0, "{b14:?}");
    for _ in 0..14 {
        shift_key(&rule, Key::Tab);
    }
    rule.wait_for_idle();
    assert!(focused(&rule, "b0") && near(bounds(&rule, "b0").top, 0.0, 0.5));
}

fn scrollable_states_hear_the_users_drags() {
    // C++'s test's Material Button in the scrolled list (its LazyColumn and pager parts are the lazy layouts' and
    // the pager's; foundation's scroll.rs has the scroll container's with a clickable box): dragged before the tap
    // timeout, the button's source never hears the press.
    let item = MutableInteractionSource::new();
    let list = ScrollState::new(0);
    let (list_log, item_log) = (new_log(), new_log());
    let (it, l, il, ll) = (item.clone(), list.clone(), item_log.clone(), list_log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let flow = l.interaction_source().interactions();
        let ll = ll.clone();
        LaunchedEffect((), async move {
            flow.collect(move |i| ll.borrow_mut().push(interaction_name(&*i).to_string())).await;
        });
        LogInteractions(it.clone(), il.clone());
        let (it, l) = (it.clone(), l.clone());
        Column(
            modifier = Modifier.fill_max_width().height(300.dp()).vertical_scroll(&l).test_tag("liste"),
            content = move || {
                Button(
                    || {},
                    modifier = Modifier.test_tag("öğe"),
                    interaction_source = Some(it.clone()),
                    content = || Text("öğe"),
                );
                for i in 0..50 {
                    Text(format!("satır {i}"), modifier = Modifier.height(40.dp()));
                }
            },
        );
    }));
    let start = center(&rule, "öğe");
    down(&rule, start);
    for i in 1..=6 {
        move_to(&rule, Offset::new(start.x, start.y - 10.0 * i as f32));
    }
    rule.wait_for_idle_within(0);
    assert!(*list_log.borrow() == ["DragStart"] && item_log.borrow().is_empty(), "{:?} {:?}", list_log, item_log);
    up(&rule, Offset::new(start.x, start.y - 60.0));
    rule.wait_for_idle();
    assert!(*list_log.borrow() == ["DragStart", "DragStop"] && item_log.borrow().is_empty());
    assert!(list.value() > 0);
}

#[kompozable]
fn LikedRow(i: i32) {
    let liked = remember_saveable(|| mutable_state_of(false));
    let plain = remember(|| mutable_state_of(false));
    let (l, p) = (liked.clone(), plain.clone());
    Row(
        modifier = Modifier.fill_max_width().height(50.dp()),
        content = move || {
            Text(
                format!(
                    "satır {i} {} {}",
                    if l.get() { "beğenildi" } else { "-" },
                    if p.get() { "işaretli" } else { "-" }
                ),
                modifier = Modifier.weight(1.0),
            );
            let (l, p) = (l.clone(), p.clone());
            Button(
                move || {
                    l.set(true);
                    p.set(true);
                },
                modifier = Modifier.test_tag(format!("like{i}")),
                content = || Text("beğen"),
            );
        },
    );
}

fn lazy_column_items_keep_saveable_state_when_scrolled_away() {
    // C++'s test with its Material Buttons (foundation's lazy.rs has it with clickable boxes): an item's saveable
    // state comes back with it, what it only remembered doesn't.
    let list = LazyListState::new(0, 0);
    let scope: Rc<RefCell<Option<kompoz_coroutines::CoroutineScope>>> = Rc::new(RefCell::new(None));
    let (l, sc) = (list.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            *sc.borrow_mut() = Some(kompoz_coroutines::remember_coroutine_scope());
            LazyColumn(
                modifier = Modifier.fill_max_size(),
                state = Some(l.clone()),
                content = |this: &mut LazyListScope| {
                    this.items(200, key = index_key(|i| i), item_content = |i| LikedRow(i));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.on_node_with_tag("like3").perform_click();
    rule.on_node_with_text("satır 3 beğenildi işaretli").assert_exists();
    let l = list.clone();
    scope.borrow().as_ref().unwrap().launch(async move { l.scroll_to_item(150).await });
    rule.wait_for_idle();
    rule.on_node_with_tag("like3").assert_does_not_exist();
    let l = list.clone();
    scope.borrow().as_ref().unwrap().launch(async move { l.scroll_to_item(0).await });
    rule.wait_for_idle();
    rule.on_node_with_text("satır 3 beğenildi -").assert_exists();
}

fn subkompoz_layout_composes_slots_from_what_it_measured() {
    // C++'s test's TextButton in the slot picked by the constraints (foundation's subcompose.rs has it with a
    // clickable box): the slot keeps the button's state while it is the one asked for; the other slot is another.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            SubcomposeLayout(
                modifier = Modifier.fill_max_width(),
                measure_policy = |scope, constraints: Constraints| {
                    let wide = constraints.max_width >= 500;
                    let key = if wide { "geniş" } else { "dar" };
                    let measurables = scope.subcompose(key, move || {
                        let taps = remember(|| mutable_state_of(0));
                        let t = taps.clone();
                        TextButton(
                            move || t.set(t.peek() + 1),
                            content = move || Text(format!("{key} düzen {}", taps.get())),
                        );
                    });
                    let loose = constraints.copy_max_dimensions();
                    let placeables: Vec<Placeable> = measurables.iter().map(|m| m.measure(loose)).collect();
                    let height = placeables.iter().map(|p| p.height()).max().unwrap_or(0);
                    scope.layout(constraints.max_width, height, move |p| {
                        for placeable in &placeables {
                            p.place(placeable, 0, 0);
                        }
                    })
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.on_node_with_text("dar düzen 0").perform_click();
    rule.host().set_viewport_size(IntSize::new(450, 800));
    rule.wait_for_idle();
    rule.on_node_with_text("dar düzen 1").assert_exists();
    rule.host().set_viewport_size(IntSize::new(600, 800));
    rule.wait_for_idle();
    rule.on_node_with_text("geniş düzen 0").assert_exists();
    rule.on_node_with_text("dar düzen 1").assert_does_not_exist();
}

fn semantics_tree_merges_buttons_and_performs_actions() {
    // C++'s test's Material Button with an Icon and a Text (its Checkbox: WP2's; its TextField: WP3's; its
    // AlertDialog: WP5's; kompoz-ui's semantics.rs has the rest): one node to accessibility, a button, its icon's
    // description and its text read together, clicked by the action.
    let clicks = Rc::new(Cell::new(0));
    let c = clicks.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let c = c.clone();
        Column(move || {
            let c = c.clone();
            Button(
                move || c.set(c.get() + 1),
                content = || {
                    VectorIcon(Icons::Filled::Add(), Some("Artı".into()));
                    Text("Ekle");
                },
            );
        });
    }));
    let tree = rule.host().accessibility_tree();
    let button = tree.iter().find(|n| n.role == Some(Role::Button)).cloned().expect("a button");
    assert!(button.label == "Artı Ekle" && button.clickable && button.enabled, "{button:?}");
    assert!(!tree.iter().any(|n| n.label == "Ekle")); // merged into the button
    assert!(rule.host().perform_semantics_action(button.id, SemanticsAction::Click) && clicks.get() == 1);
}

fn semantics_merge_clear_and_hide() {
    // C++'s test's Material Buttons (kompoz-ui's semantics.rs has the rest with clickables): one in a merging row
    // stays a node of its own; one inside cleared or hidden semantics isn't told.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Row(
                modifier = Modifier.semantics_with(true, |_| {}),
                content = || {
                    VectorIcon(Icons::Filled::Star(), Some("Yıldızlı".into()));
                    Text("Favoriler");
                    Button(|| {}, content = || Text("Aç")); // interactive: on its own
                },
            );
            Box(
                modifier = Modifier.clear_and_set_semantics(|c| c.set_content_description("Puan: 4 / 5")),
                content = || {
                    Button(|| {}, content = || Text("İç düğme"));
                },
            );
            Box(
                modifier = Modifier.semantics(|c| c.hide_from_accessibility()),
                content = || {
                    Button(|| {}, content = || Text("Gizli düğme"));
                },
            );
        });
    }));
    let tree = rule.host().accessibility_tree();
    let row = tree.iter().find(|n| n.label == "Yıldızlı Favoriler").expect("the row");
    let open = tree.iter().find(|n| n.label == "Aç").expect("the button");
    assert!(open.parent == Some(row.id) && open.clickable && open.role == Some(Role::Button));
    assert!(!tree.iter().any(|n| n.label.contains("İç düğme") || n.label.contains("Gizli düğme")));
}

fn test_rule_assertions() {
    // C++'s test's Material Buttons and Icon (its Checkbox: WP2's; kompoz-ui's test_rule.rs has the rest with
    // semantics boxes): a disabled button has its click action but isn't enabled; an icon has none.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Button(|| {}, content = || Text("Tamam"));
            Button(|| {}, enabled = false, content = || Text("Kapalı"));
            VectorIcon(Icons::Filled::Info(), Some("Bilgi".into()));
        });
    }));
    rule.on_node_with_text("Tamam")
        .assert_is_displayed()
        .assert_has_click_action()
        .assert_is_enabled()
        .assert_text_equals(&["Tamam"]);
    rule.on_node_with_text("Kapalı").assert_has_click_action().assert_is_not_enabled();
    rule.on_node_with_content_description("Bilgi").assert_has_no_click_action();
}

// ---- Buttons and containers together (area A) ----------------------------------------------------------------------

fn buttons_and_chips_keep_48_dp_of_room_to_touch_them() {
    // C++'s test (its ToggleButton: WP1b's): each 48 dp tall to touch where it looks smaller (the button 40, the
    // chip 32, the small FAB 40 square), drawn in the middle; a tap 2 dp from the top lands; no room asked for, as
    // big as they look. (Compose's semantics bounds of a clickable component are its clickable's — the 40 dp
    // container —, so the room is measured on a Box around each, as big as what it holds.)
    let taps = new_log();
    let room = mutable_state_of(48.0f32);
    let (t, r) = (taps.clone(), room.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let t = t.clone();
            KompozitionLocalProvider(&[LocalMinimumInteractiveComponentSize.provides(Dp(r.get()))], move || {
                let t = t.clone();
                Column(move || {
                    let push = |t: &Log, s: &'static str| {
                        let t = t.clone();
                        move || t.borrow_mut().push(s.to_string())
                    };
                    let (a, b, c, d) = (push(&t, "düğme"), push(&t, "metin"), push(&t, "çip"), push(&t, "fab"));
                    Box(
                        modifier = Modifier.test_tag("düğme"),
                        content = move || {
                            Button(a, content = || Text("Kaydet"));
                        },
                    );
                    Box(
                        modifier = Modifier.test_tag("metin"),
                        content = move || {
                            TextButton(b, content = || Text("İptal"));
                        },
                    );
                    Box(
                        modifier = Modifier.test_tag("çip"),
                        content = move || {
                            AssistChip(c, label = || Text("Yol"));
                        },
                    );
                    Box(
                        modifier = Modifier.test_tag("fab"),
                        content = move || {
                            SmallFloatingActionButton(
                                d,
                                content = || VectorIcon(Icons::Filled::Add(), Some("Ekle".into())),
                            );
                        },
                    );
                });
            });
        }),
        IntSize::new(400, 800),
    );
    let (b, m, c, f) = (bounds(&rule, "düğme"), bounds(&rule, "metin"), bounds(&rule, "çip"), bounds(&rule, "fab"));
    assert!(b.height() == 48.0 && m.height() == 48.0 && c.height() == 48.0, "{b:?} {m:?} {c:?}");
    assert!(f.width() == 48.0 && f.height() == 48.0, "{f:?}");
    // The button's container is 40 dp in the middle.
    let primary = light_color_scheme().primary;
    let canvas = rule.draw();
    let container = canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .any(|(r, k)| *k == primary && near(r.top, b.top + 4.0, 0.01) && near(r.height(), 40.0, 0.01))
        || canvas
            .paths
            .iter()
            .zip(&canvas.path_colors)
            .any(|(r, k)| *k == primary && near(r.top, b.top + 4.0, 0.01) && near(r.height(), 40.0, 0.01));
    assert!(container, "the container drawn 40 dp tall, 4 dp down");
    for tag in ["düğme", "metin", "çip", "fab"] {
        let r = bounds(&rule, tag);
        tap(&rule, Offset::new(r.center().x, r.top + 2.0));
    }
    rule.wait_for_idle();
    assert_eq!(*taps.borrow(), ["düğme", "metin", "çip", "fab"]);
    // No room asked for: as big as they look.
    room.set(0.0);
    rule.wait_for_idle();
    assert!(bounds(&rule, "düğme").height() == 40.0 && bounds(&rule, "çip").height() == 32.0);
    assert_eq!(bounds(&rule, "fab").width(), 40.0);
}

fn buttons_animate_composing_around_them_only_as_they_turn() {
    // C++'s test's Button and small extended FAB (its ToggleButton and ButtonGroup: WP1b's): hovered, their
    // elevation rises without composing what is around them again; collapsed, the FAB narrows as it is laid out,
    // what is around it composed once as it turns.
    let expanded = mutable_state_of(true);
    let composed = Rc::new(Cell::new(0));
    let (e, c) = (expanded.clone(), composed.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            c.set(c.get() + 1);
            let open = e.get();
            Column(move || {
                Button(|| {}, modifier = Modifier.test_tag("düğme"), content = || Text("Gönder"));
                TextIconSmallExtendedFloatingActionButton(
                    || Text("Ekle"),
                    || VectorIcon(Icons::Filled::Add(), None),
                    on_click = || {},
                    modifier = Modifier.test_tag("fab"),
                    expanded = open,
                );
            });
        }),
        IntSize::new(400, 800),
    );
    for tag in ["düğme", "fab"] {
        rule.host().pointer_move(center(&rule, tag), -1, 0);
        rule.advance_frame();
        let before = composed.get();
        rule.wait_for_idle();
        assert_eq!(composed.get(), before, "{tag}: hovered, composed again");
    }
    rule.host().pointer_exit();
    rule.wait_for_idle();
    let full = bounds(&rule, "fab").width();
    expanded.set(false);
    rule.advance_frame();
    let before = composed.get();
    rule.advance_time_by(64);
    let narrowing = bounds(&rule, "fab").width();
    assert!(composed.get() == before && narrowing < full - 5.0, "{full} {narrowing}");
    rule.wait_for_idle();
    let narrow = bounds(&rule, "fab").width();
    // (collapsed to its icon: the small FAB's 56 dp square container)
    assert!(narrow < narrowing && composed.get() == before, "{narrow}");
    assert_eq!(narrow, 56.0);
}

fn rows_line_children_up_by_their_baselines() {
    // C++'s test's Button lined up by its label's baseline, passed on (foundation's tests have the texts' part).
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Row(|| {
                Button(|| {}, modifier = Modifier.align_by_baseline(), content = || Text("Tamam"));
                Text("etiket", modifier = Modifier.align_by(LastBaseline), font_size = 24.sp());
            });
        }),
        IntSize::new(400, 800),
    );
    let canvas = rule.draw();
    let at = |t: &str| canvas.texts.iter().position(|x| x == t).map(|i| canvas.baselines[i].y).expect(t);
    // (alignment lines are whole px, as Compose's: the label's baseline rounded where it is passed on)
    assert!(near(at("Tamam"), at("etiket"), 0.5), "{} {}", at("Tamam"), at("etiket"));
}

fn center_aligned_bar_clickable_cards_fab_sizes_and_positions() {
    // C++'s test's clickable Card and FAB sizes (its Scaffold, CenterAlignedTopAppBar and FAB positions: WP4a's):
    // the card clicks; a small FAB is 40 dp in 48 of room, a large one 96, the FAB 56.
    let clicks = Rc::new(Cell::new(0));
    let c = clicks.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let c = c.clone();
            Column(move || {
                let c = c.clone();
                ClickableCard(
                    move || c.set(c.get() + 1),
                    modifier = Modifier.test_tag("kart"),
                    content = || {
                        Text("Kart", modifier = Modifier.padding(16.dp()));
                    },
                );
                Row(|| {
                    SmallFloatingActionButton(
                        || {},
                        modifier = Modifier.test_tag("küçük"),
                        content = || {
                            VectorIcon(Icons::Filled::Add(), Some("Küçük".into()));
                        },
                    );
                    LargeFloatingActionButton(
                        || {},
                        modifier = Modifier.test_tag("büyük"),
                        content = || {
                            VectorIcon(Icons::Filled::Add(), Some("Büyük".into()));
                        },
                    );
                });
                FloatingActionButton(
                    || {},
                    modifier = Modifier.test_tag("fab"),
                    content = || {
                        VectorIcon(Icons::Filled::Add(), Some("Ekle".into()));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.on_node_with_tag("kart").perform_click();
    assert_eq!(clicks.get(), 1);
    // (the small FAB's node is its 40 dp container, the large one starting 48 dp along: its room)
    let (small, large) = (bounds(&rule, "küçük"), bounds(&rule, "büyük"));
    assert!(
        small.width() == 40.0 && small.left == 4.0 && large.left == 48.0 && large.width() == 96.0,
        "{small:?} {large:?}"
    );
    assert_eq!(bounds(&rule, "fab").width(), 56.0);
}

tests!(
    buttons_and_chips_keep_48_dp_of_room_to_touch_them,
    buttons_animate_composing_around_them_only_as_they_turn,
    rows_line_children_up_by_their_baselines,
    center_aligned_bar_clickable_cards_fab_sizes_and_positions,
    counter_increments_on_click,
    interaction_source_hears_presses_releases_cancels_and_keyboard_clicks,
    custom_indication_hears_its_component_and_draws_around_it,
    tab_reaches_buttons_and_enter_or_space_clicks_them,
    a_focus_request_brings_the_node_into_view,
    focus_moved_by_keys_scrolls_into_view_and_so_does_the_keyboard,
    scrollable_states_hear_the_users_drags,
    lazy_column_items_keep_saveable_state_when_scrolled_away,
    subkompoz_layout_composes_slots_from_what_it_measured,
    semantics_tree_merges_buttons_and_performs_actions,
    semantics_merge_clear_and_hide,
    test_rule_assertions,
);

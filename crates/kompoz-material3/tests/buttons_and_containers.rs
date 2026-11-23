//! Buttons and containers together, and the Material halves of earlier tests about buttons and clickables: the
//! C++ Kompoz's tests by name whose foundation or ui half is in a lower crate (a clickable box standing in for the
//! Material Button there), here with Material's buttons as the C++ test has them. Parts that need components of
//! later packages say which. Numbers in px at density 1.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_coroutines::{CoroutineScope, LaunchedEffect};
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
    // C++'s test with its Material Buttons and the TextField between them (foundation's clickable.rs has it with
    // clickable boxes): Tab reaches them, Enter and Space click them, the second Tab the field, Shift+Tab back; a tap
    // focuses neither a button nor a focusable. (In the field, a multi-line one, Tab types a tab, as Compose's: the
    // focus moves on with Ctrl+Tab, which Compose's focus system takes as Tab; the C++ moved on with Tab.)
    let log = new_log();
    let text = mutable_state_of(String::new());
    let (l, t) = (log.clone(), text.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, t) = (l.clone(), t.clone());
        Column(move || {
            let (a, b, t) = (l.clone(), l.clone(), t.clone());
            Button(
                move || a.borrow_mut().push("bir".into()),
                modifier = Modifier.test_tag("one"),
                content = || Text("Bir"),
            );
            StringTextField(t.get(), move |v| t.set(v), modifier = Modifier.test_tag("field"));
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
    assert!(focused(&rule, "field"));
    rule.wait_for_idle();
    rule.on_node_with_tag("field").assert_is_focused();
    assert!(key(&rule, Key::Tab));
    rule.wait_for_idle();
    assert!(focused(&rule, "field") && text.peek() == "\t", "{:?}", text.peek());
    rule.host().key_down(Key::Tab, KeyModifiers::Ctrl);
    rule.host().key_up(Key::Tab, KeyModifiers::Ctrl);
    assert!(focused(&rule, "two"));
    assert!(key(&rule, Key::Spacebar));
    rule.wait_for_idle();
    assert_eq!(log.borrow().last().map(String::as_str), Some("iki"));
    key(&rule, Key::Tab);
    assert!(focused(&rule, "box"));
    shift_key(&rule, Key::Tab);
    assert!(focused(&rule, "two"));
    rule.wait_for_idle();
    rule.on_node_with_tag("two").assert_is_focused(); // the focused button shows it
    // Pointing ends keyboard focus; a tap doesn't focus a button or a focusable().
    rule.on_node_with_tag("one").perform_click();
    rule.wait_for_idle();
    assert!(rule.host().focused_node().is_none() && log.borrow().last().map(String::as_str) == Some("bir"));
    rule.on_node_with_tag("box").perform_click();
    rule.wait_for_idle();
    assert!(rule.host().focused_node().is_none());
}

fn a_focus_request_brings_the_node_into_view() {
    // C++'s test with its Material Button above and its TextField below (foundation's scroll.rs has it with a
    // focusable box and a BasicTextField): a focus request far below, with no keyboard to make room for, scrolls
    // the field into view; the button's back up. (A clickable takes the focus only out of touch mode, as Compose's:
    // the keyboard's mode asked for, as a Compose test does through LocalInputModeManager.)
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
                    StringTextField(
                        "",
                        |_| {},
                        modifier = Modifier.focus_requester(&b).test_tag("alan"),
                        single_line = true,
                        keyboard_options = KeyboardOptions(show_keyboard_on_focus = false),
                    );
                    Box(modifier = Modifier.height(1500.dp()));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.run_on_idle(|| modes.borrow().as_ref().unwrap().request_input_mode(kompoz_ui::platform::InputMode::Keyboard));
    below.request_focus(); // far below, and no keyboard to make room for: scrolled to anyway
    rule.wait_for_idle();
    let field = bounds(&rule, "alan");
    assert!(focused(&rule, "alan") && field.height() == 56.0, "{field:?}");
    assert!(field.top >= 0.0 && field.bottom <= 800.5 && near(field.bottom, 800.0, 1.0), "{field:?}"); // at the bottom
    above.request_focus(); // and back up
    rule.wait_for_idle();
    assert!(
        near(bounds(&rule, "üst").top, 0.0, 0.5) && scroll.value() == 0,
        "{:?} {}",
        bounds(&rule, "üst"),
        scroll.value()
    );
}

/// C++'s `state.scrollTo(value)`: scrolled from code, run until idle.
fn scroll_to(rule: &KompozTestRule, state: &ScrollState, value: i32) {
    let s = state.clone();
    CoroutineScope::new(rule.host().dispatcher()).launch(async move {
        s.scroll_to(value).await;
    });
    rule.wait_for_idle();
}

fn focus_moved_by_keys_scrolls_into_view_and_so_does_the_keyboard() {
    // C++'s test with its Material Buttons and TextField (foundation's scroll.rs has it with focusable boxes and a
    // BasicTextField): Tab brings the focused button into view, Shift+Tab scrolls back up; the tapped field a
    // keyboard comes up over (the scroll's viewport shrinking by its padding) has its cursor kept above it; a window
    // that gets shorter too; not once scrolled away from it.
    let scroll = ScrollState::new(0);
    let text = mutable_state_of(String::new());
    let (s, t) = (scroll.clone(), text.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (s, t) = (s.clone(), t.clone());
            MaterialTheme(move || {
                let t = t.clone();
                Column(
                    modifier = Modifier.fill_max_size().ime_padding().vertical_scroll(&s),
                    content = move || {
                        for i in 0..20 {
                            Button(
                                || {},
                                modifier = Modifier.height(60.dp()).test_tag(format!("b{i}")),
                                content = move || Text(format!("Düğme {i}")),
                            );
                        }
                        let t = t.clone();
                        StringTextField(
                            t.get(),
                            move |v| t.set(v),
                            modifier = Modifier.height(60.dp()).test_tag("field"),
                        );
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    for _ in 0..15 {
        key(&rule, Key::Tab);
    }
    rule.wait_for_idle();
    let b14 = bounds(&rule, "b14");
    assert!(focused(&rule, "b14") && b14.top >= 0.0 && b14.bottom <= 800.0 && scroll.value() > 0, "{b14:?}");
    // Back up: it scrolls back as far as needed.
    for _ in 0..14 {
        shift_key(&rule, Key::Tab);
    }
    rule.wait_for_idle();
    assert!(focused(&rule, "b0") && near(bounds(&rule, "b0").top, 0.0, 0.5));
    // A tapped field near the bottom, then the keyboard comes up over it: it scrolls above it.
    scroll_to(&rule, &scroll, scroll.max_value());
    let field = bounds(&rule, "field");
    tap(&rule, field.center());
    rule.wait_for_idle();
    assert!(focused(&rule, "field") && bounds(&rule, "field").bottom > 700.0, "{:?}", bounds(&rule, "field"));
    let insets = kompoz_ui::platform::SystemInsets {
        ime: kompoz_ui::platform::Insets { left: 0, top: 0, right: 0, bottom: 300 },
        ..Default::default()
    };
    rule.host().set_system_insets(insets);
    rule.wait_for_idle();
    // Compose keeps the focused node's focus rect in view: a field over a String (the C++'s) gives none of its
    // own, so all of it (one over a TextFieldState gives its cursor's: foundation's scroll.rs).
    let editing = bounds(&rule, "field");
    assert!(near(editing.bottom, 500.0, 0.5) && editing.height() == 60.0, "{editing:?}");
    // A window that isn't edge to edge gets shorter instead (the keyboard is no inset there).
    rule.host().set_system_insets(kompoz_ui::platform::SystemInsets::default());
    rule.wait_for_idle();
    scroll_to(&rule, &scroll, scroll.max_value());
    rule.host().set_viewport_size(IntSize::new(400, 500));
    rule.wait_for_idle();
    let editing = bounds(&rule, "field");
    assert!(editing.bottom <= 500.5 && editing.bottom > 450.0, "{editing:?}");
    scroll_to(&rule, &scroll, 0); // scrolled away from it, then taller again: it stays where it is
    rule.host().set_viewport_size(IntSize::new(400, 800));
    rule.wait_for_idle();
    assert_eq!(scroll.value(), 0);
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
    rule.wait_for_idle();
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
    // C++'s test's heading, Material Button with an Icon and a Text, Checkbox, TextField, scrolled column and
    // AlertDialog (kompoz-ui's semantics.rs has the rest): one node to accessibility for the button, its icon's
    // description and its text read together, clicked by the action; the checkbox a node of its role, checked, no
    // label, unchecked by its click action; the field an editable node with its value, unfocused until its click
    // focuses it, its id kept; the column scrolled forward by its action; a dialog on top.
    let clicks = Rc::new(Cell::new(0));
    let checked = mutable_state_of(true);
    let scroll = ScrollState::new(0);
    let dialog = mutable_state_of(false);
    let (c, k, s, d) = (clicks.clone(), checked.clone(), scroll.clone(), dialog.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, k, s, d) = (c.clone(), k.clone(), s.clone(), d.clone());
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&s),
            content = move || {
                let (c, k) = (c.clone(), k.clone());
                Text("Başlık", modifier = Modifier.semantics(|p| p.heading()));
                Button(
                    move || c.set(c.get() + 1),
                    content = || {
                        VectorIcon(Icons::Filled::Add(), Some("Artı".into()));
                        Text("Ekle");
                    },
                );
                Checkbox(k.get(), Some(Rc::new(move |v| k.set(v))));
                StringTextField("merhaba", |_| {}, placeholder = Some(Rc::new(|| Text("yaz"))));
                Box(modifier = Modifier.height(2000.dp()));
                if d.get() {
                    // (Compose's AlertDialog requires its confirm button: none here, as the C++'s had)
                    AlertDialog(|| {}, || {}, title = Some(Rc::new(|| Text("Uyarı"))));
                }
            },
        );
    }));
    let tree = rule.host().accessibility_tree();
    let button = tree.iter().find(|n| n.role == Some(Role::Button)).cloned().expect("a button");
    let field = tree.iter().find(|n| n.editable).cloned().expect("a field");
    let check = tree.iter().find(|n| n.role == Some(Role::Checkbox)).cloned().expect("a checkbox");
    assert!(button.label == "Artı Ekle" && button.clickable && button.enabled, "{button:?}");
    assert!(!tree.iter().any(|n| n.label == "Ekle")); // merged into the button
    assert!(check.checked == Some(true) && check.label.is_empty(), "{check:?}");
    assert!(field.value == "merhaba" && !field.focused, "{field:?}");
    assert!(tree.iter().any(|n| n.heading && n.label == "Başlık"));
    let list = tree.iter().find(|n| n.scrollable).cloned().expect("the scrolled column");
    assert!(rule.host().perform_semantics_action(button.id, SemanticsAction::Click) && clicks.get() == 1);
    assert!(rule.host().perform_semantics_action(check.id, SemanticsAction::Click));
    rule.wait_for_idle();
    assert!(!checked.peek());
    assert!(rule.host().perform_semantics_action(field.id, SemanticsAction::Click));
    rule.wait_for_idle();
    let again = rule.host().accessibility_tree();
    let f = again.iter().find(|n| n.editable).expect("the field still");
    assert!(f.focused && f.id == field.id, "{f:?}"); // ids stay
    assert!(rule.host().perform_semantics_action(list.id, SemanticsAction::ScrollForward));
    rule.wait_for_idle();
    assert!(scroll.value() > 100, "{}", scroll.value());
    // A dialog on top: the tree is the dialog's (its title), what is under it isn't told.
    dialog.set(true);
    rule.wait_for_idle();
    let modal = rule.host().accessibility_tree();
    assert!(modal.iter().any(|n| n.label == "Uyarı"), "{modal:?}");
    assert!(!modal.iter().any(|n| n.role == Some(Role::Button)), "{modal:?}");
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
    // C++'s test's Material Buttons, Checkbox and Icon (kompoz-ui's test_rule.rs has the rest with semantics boxes):
    // a disabled button has its click action but isn't enabled; the checkbox off, on once clicked, and asserting it
    // off then fails as Compose's message says; an icon has no click action.
    let ticked = mutable_state_of(false);
    let t = ticked.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let t = t.clone();
        Column(move || {
            Button(|| {}, content = || Text("Tamam"));
            Button(|| {}, enabled = false, content = || Text("Kapalı"));
            let set = t.clone();
            Checkbox(t.get(), Some(Rc::new(move |v| set.set(v))), modifier = Modifier.test_tag("box"));
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
    rule.on_node_with_tag("box").assert_is_off();
    rule.on_node_with_tag("box").perform_click();
    rule.on_node_with_tag("box").assert_is_on();
    assert!(ticked.peek());
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rule.on_node_with_tag("box").assert_is_off();
    }));
    std::panic::set_hook(previous);
    let message = failed.err().and_then(|e| e.downcast_ref::<String>().cloned()).unwrap_or_default();
    assert!(message.contains("Failed to assert the following: (ToggleableState = 'Off')"), "{message}");
}

// ---- Buttons and containers together (area A) ----------------------------------------------------------------------

fn buttons_and_chips_keep_48_dp_of_room_to_touch_them() {
    // C++'s test (its ToggleButton: WP1b's, in place): each 48 dp tall to touch where it looks smaller (the button
    // and the toggle button 40, the chip 32, the small FAB 40 square), drawn in the middle; a tap 2 dp from the top
    // lands (the toggle button checked); no room asked for, as big as they look. (Compose's semantics bounds of a clickable component are its clickable's — the 40 dp
    // container —, so the room is measured on a Box around each, as big as what it holds.)
    let taps = new_log();
    let room = mutable_state_of(48.0f32);
    let on = mutable_state_of(false); // (WP1b: toggle_split)
    let (t, r, o) = (taps.clone(), room.clone(), on.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (t, o) = (t.clone(), o.clone());
            KompozitionLocalProvider(&[LocalMinimumInteractiveComponentSize.provides(Dp(r.get()))], move || {
                let (t, o) = (t.clone(), o.clone());
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
                    // (WP1b: toggle_split)
                    let o = o.clone();
                    Box(
                        modifier = Modifier.test_tag("aç"),
                        content = move || {
                            let o2 = o.clone();
                            ToggleButton(o.get(), move |v| o2.set(v), content = || Text("Aç"));
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
    assert_eq!(bounds(&rule, "aç").height(), 48.0); // (WP1b: toggle_split)
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
    for tag in ["düğme", "metin", "aç", "çip", "fab"] {
        let r = bounds(&rule, tag);
        tap(&rule, Offset::new(r.center().x, r.top + 2.0));
    }
    rule.wait_for_idle();
    assert_eq!(*taps.borrow(), ["düğme", "metin", "çip", "fab"]);
    assert!(on.get(), "the toggle button checked by a tap 2 dp from the top"); // (WP1b: toggle_split)
    // No room asked for: as big as they look.
    room.set(0.0);
    rule.wait_for_idle();
    assert!(bounds(&rule, "düğme").height() == 40.0 && bounds(&rule, "çip").height() == 32.0);
    assert_eq!(bounds(&rule, "fab").width(), 40.0);
}

#[allow(deprecated)] // (WP1b: group — ButtonGroup's deprecated content overload, as the C++ composed it)
fn buttons_animate_composing_around_them_only_as_they_turn() {
    // C++'s test's Button, ToggleButton (WP1b: toggle_split), small extended FAB and ButtonGroup (WP1b: group; M15's
    // deprecated content overload, as the C++ composed it): hovered, their elevation rises without composing what is
    // around them again; a held button in the group grows without composing it again; checked and collapsed, the toggle
    // button's corners morph from round towards 12 dp as it is drawn and the FAB narrows as it is laid out, what is
    // around them composed once as they turn.
    let expanded = mutable_state_of(true);
    let checked = mutable_state_of(false); // (WP1b: toggle_split)
    let held = MutableInteractionSource::new(); // (WP1b: group)
    let composed = Rc::new(Cell::new(0));
    let (e, c, k, h) = (expanded.clone(), composed.clone(), checked.clone(), held.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            c.set(c.get() + 1);
            let open = e.get();
            let (k, h) = (k.clone(), h.clone());
            Column(move || {
                Button(|| {}, modifier = Modifier.test_tag("düğme"), content = || Text("Gönder"));
                // (WP1b: toggle_split)
                let k2 = k.clone();
                ToggleButton(
                    k.get(),
                    move |v| k2.set(v),
                    modifier = Modifier.test_tag("seçim"),
                    content = || Text("Seç"),
                );
                TextIconSmallExtendedFloatingActionButton(
                    || Text("Ekle"),
                    || VectorIcon(Icons::Filled::Add(), None),
                    on_click = || {},
                    modifier = Modifier.test_tag("fab"),
                    expanded = open,
                );
                // (WP1b: group)
                let h = h.clone();
                ButtonGroupWithoutOverflowIndicator(
                    content = move |group: &ButtonGroupScope| {
                        let other = remember(MutableInteractionSource::new);
                        Button(|| {}, modifier = group.animate_width(Modifier, &other), content = || Text("Önceki"));
                        Button(
                            || {},
                            modifier = group.animate_width(Modifier, &h).test_tag("tutulan"),
                            interaction_source = Some(h.clone()),
                            content = || Text("Oynat"),
                        );
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    let natural = bounds(&rule, "tutulan").width(); // (WP1b: group)
    for tag in ["düğme", "fab"] {
        rule.host().pointer_move(center(&rule, tag), -1, 0);
        rule.advance_frame();
        let before = composed.get();
        rule.wait_for_idle();
        assert_eq!(composed.get(), before, "{tag}: hovered, composed again");
    }
    rule.host().pointer_exit();
    rule.wait_for_idle();
    // (WP1b: group) pressed: the button grows as it is laid out, nothing composed again
    rule.host().pointer_down(center(&rule, "tutulan"), -1, 0, PointerType::Touch);
    rule.advance_frame();
    let before = composed.get();
    rule.advance_time_by(500);
    assert!(composed.get() == before && bounds(&rule, "tutulan").width() > natural + 3.0, "{natural}");
    rule.host().pointer_up(center(&rule, "tutulan"), -1, 0);
    rule.wait_for_idle();
    // (WP1b: toggle_split) the toggle button's container as drawn: its corner radius
    let corner = |rule: &KompozTestRule| {
        let b = bounds(rule, "seçim");
        let canvas = rule.draw();
        canvas
            .rects
            .iter()
            .zip(&canvas.rect_radii)
            .find(|(r, _)| b.contains(r.center()) && near(r.height(), 40.0, 0.01))
            .map_or(-1.0, |(_, radius)| *radius)
    };
    let full = bounds(&rule, "fab").width();
    let round = corner(&rule);
    // (the clock by hand: the checks on the way see the buttons turning)
    rule.main_clock().set_auto_advance(false);
    checked.set(true);
    expanded.set(false);
    rule.advance_frame();
    let before = composed.get();
    rule.advance_time_by(64);
    let narrowing = bounds(&rule, "fab").width();
    let morphing = corner(&rule);
    assert!(composed.get() == before && narrowing < full - 5.0, "{full} {narrowing}");
    // (from round towards the checked shape's 12 dp corners, as it is drawn)
    assert!(near(round, 20.0, 0.01) && morphing < 19.0 && morphing > 12.5, "{round} {morphing}");
    finish_animations(&rule);
    let narrow = bounds(&rule, "fab").width();
    // (collapsed to its icon: the small FAB's 56 dp square container)
    assert!(narrow < narrowing && composed.get() == before, "{narrow}");
    assert_eq!(narrow, 56.0);
    assert!(near(corner(&rule), 12.0, 0.01)); // (WP1b: toggle_split)
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
    // C++'s test whole: a Scaffold's CenterAlignedTopAppBar keeps its title in the middle of the bar, between its
    // icons; the card clicks; a small FAB is 40 dp in 48 of room, a large one 96; the scaffold's FAB at each position.
    let title = mutable_state_of("Başlık".to_string());
    let position = mutable_state_of(FabPosition::End);
    let clicks = Rc::new(Cell::new(0));
    let rule = {
        let (title, position, c) = (title.clone(), position.clone(), clicks.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (title, c) = (title.clone(), c.clone());
                Scaffold(
                    modifier = Modifier.fill_max_size(),
                    top_bar = move || {
                        let title = title.clone();
                        CenterAlignedTopAppBar(
                            title = move || {
                                Text(
                                    title.get(),
                                    modifier = Modifier.test_tag("başlık"),
                                    max_lines = 1,
                                    overflow = kompoz_text::style::TextOverflow::Ellipsis,
                                );
                            },
                            navigation_icon = || {
                                IconButton(|| {}, content = || VectorIcon(Icons::Filled::Menu(), Some("Menü".into())));
                            },
                            actions = || {
                                IconButton(|| {}, content = || VectorIcon(Icons::Filled::Search(), Some("Ara".into())));
                                IconButton(
                                    || {},
                                    content = || VectorIcon(Icons::Filled::MoreVert(), Some("Diğer".into())),
                                );
                            },
                        );
                    },
                    bottom_bar = || Box(modifier = Modifier.fill_max_width().height(80.dp())),
                    floating_action_button = || {
                        FloatingActionButton(
                            || {},
                            modifier = Modifier.test_tag("fab"),
                            content = || VectorIcon(Icons::Filled::Add(), Some("Ekle".into())),
                        );
                    },
                    floating_action_button_position = position.get(),
                    content = move |padding: PaddingValues| {
                        let c = c.clone();
                        Column(
                            modifier = Modifier.padding_values(padding),
                            content = move || {
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
                            },
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let t = rule.on_node_with_tag_unmerged("başlık").get_unclipped_bounds_in_root();
    assert!((t.center().x - 200.0).abs() <= 1.0, "{t:?}"); // in the middle of the bar, not of what the icons leave
    title.set("Çok çok uzun bir başlık, simgelerin arasına sığmayacak kadar uzun".to_string());
    rule.wait_for_idle();
    let t = rule.on_node_with_tag_unmerged("başlık").get_unclipped_bounds_in_root();
    // Kept between them. M15 (TopAppBarMeasurePolicy) gives the title the room the icons leave — 400 less the
    // navigation icon's 4 + 48 and the actions' 96 + 4, the title padded 4 at each side: 240 for the text — and,
    // centered, it would run under the wider actions, so it moves towards the start until it ends where they begin
    // (the C++ kept it out of the wider side's room on both sides, centered: 84 to 316).
    assert!(near(t.left, 4.0 + 48.0 + 4.0, 0.5) && near(t.right, 400.0 - 4.0 - 96.0 - 4.0, 0.5), "{t:?}");
    rule.on_node_with_tag("kart").perform_click();
    assert_eq!(clicks.get(), 1);
    // (the small FAB's node is its 40 dp container, the large one starting 48 dp along: its room)
    let (small, large) = (bounds(&rule, "küçük"), bounds(&rule, "büyük"));
    assert!(
        small.width() == 40.0 && small.left == 4.0 && large.left == 48.0 && large.width() == 96.0,
        "{small:?} {large:?}"
    );
    let fab = bounds(&rule, "fab");
    assert_eq!((fab.left, fab.top, fab.width()), (400.0 - 56.0 - 16.0, 800.0 - 80.0 - 16.0 - 56.0, 56.0)); // at the end, above the bottom bar
    position.set(FabPosition::Center);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "fab").left, 172.0);
    position.set(FabPosition::Start);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "fab").left, 16.0);
    position.set(FabPosition::EndOverlay); // over the bottom bar
    rule.wait_for_idle();
    let fab = bounds(&rule, "fab");
    assert_eq!((fab.left, fab.top), (400.0 - 56.0 - 16.0, 800.0 - 16.0 - 56.0));
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

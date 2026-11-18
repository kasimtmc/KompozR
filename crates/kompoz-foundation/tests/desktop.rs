//! Compose for desktop's foundation extras: `Modifier.on_click` and `Modifier.on_drag` (pointer matchers, keyboard
//! modifiers, double and long clicks), `TooltipArea`, `ContextMenuArea` (its default representation in a popup, the
//! text context menus of selection containers and text fields), `VerticalScrollbar`/`HorizontalScrollbar` with their
//! adapters — the C++ Kompoz's tests of them (survey N, P: 29071, 29131, 20659, 20537) and Compose's behaviour where
//! the two differ. Numbers in px (density 1).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
#[allow(unused_imports)]
use kompoz_ui::alignment::Alignment;
use kompoz_ui::input::{KeyModifiers, PointerButton, PointerKeyboardModifiers, PointerType};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

/// C++'s `click`: the mouse's primary button down, a frame, up, 400 ms.
fn click(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Mouse);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_time_by(400);
}

/// C++'s `tap`: a finger down, a frame, up, a frame.
fn tap(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(p, -1, 0);
    rule.advance_frame();
}

fn mouse_button(rule: &KompozTestRule, p: Offset, button: PointerButton, pressed: bool) {
    rule.host().mouse_button(p, -1, button, pressed);
}

fn count(c: &Rc<Cell<i32>>) -> impl Fn() + 'static {
    let c = c.clone();
    move || c.set(c.get() + 1)
}

fn on_click_matches_buttons_pointers_and_keys() {
    // C++ 29071: the primary matcher takes the mouse's primary button and fingers; a secondary button's matcher
    // only a mouse's secondary press and release over the box (pressed or let go elsewhere, or another button:
    // nothing); a keyboard modifiers test; a mouse-only matcher; double and long clicks.
    let [primary, secondary, with_ctrl, mouse_only, doubles, longs] =
        std::array::from_fn::<_, 6, _>(|_| Rc::new(Cell::new(0)));
    let (p, s, w, m, d, l) =
        (primary.clone(), secondary.clone(), with_ctrl.clone(), mouse_only.clone(), doubles.clone(), longs.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (p, s, w, m, d, l) = (p.clone(), s.clone(), w.clone(), m.clone(), d.clone(), l.clone());
        Column(
            content = move || {
                Box(modifier = Modifier.size_wh(100.dp(), 40.dp()).on_click(on_click = count(&p)));
                Box(modifier = Modifier
                    .size_wh(100.dp(), 40.dp())
                    .on_click(matcher = PointerMatcher::mouse(PointerButton::Secondary), on_click = count(&s)));
                Box(modifier = Modifier.size_wh(100.dp(), 40.dp()).on_click(
                    keyboard_modifiers = Some(Rc::new(|m: &PointerKeyboardModifiers| m.is_ctrl_pressed())
                        as kompoz_foundation::gestures::KeyboardModifiersTest),
                    on_click = count(&w),
                ));
                Box(modifier = Modifier
                    .size_wh(100.dp(), 40.dp())
                    .on_click(matcher = PointerMatcher::mouse(PointerButton::Primary), on_click = count(&m)));
                let (dd, ll) = (count(&d), count(&l));
                Box(modifier = Modifier.size_wh(100.dp(), 40.dp()).on_click(
                    on_double_click = Some(Rc::new(dd) as Rc<dyn Fn()>),
                    on_long_click = Some(Rc::new(ll) as Rc<dyn Fn()>),
                    on_click = || {},
                ));
            },
        );
    }));
    click(&rule, at(50.0, 20.0));
    tap(&rule, at(50.0, 20.0)); // a finger too
    rule.advance_time_by(400);
    assert_eq!(primary.get(), 2);
    click(&rule, at(50.0, 60.0)); // the primary button: not this one's
    mouse_button(&rule, at(50.0, 60.0), PointerButton::Secondary, true);
    mouse_button(&rule, at(50.0, 60.0), PointerButton::Secondary, false);
    mouse_button(&rule, at(50.0, 60.0), PointerButton::Secondary, true);
    mouse_button(&rule, at(50.0, 150.0), PointerButton::Secondary, false); // let go elsewhere: no click
    mouse_button(&rule, at(50.0, 60.0), PointerButton::Tertiary, false);
    assert_eq!(secondary.get(), 1);
    // Pressed elsewhere (after one let go elsewhere), let go over it; another button pressed, this one let go.
    mouse_button(&rule, at(50.0, 150.0), PointerButton::Secondary, true);
    mouse_button(&rule, at(50.0, 60.0), PointerButton::Secondary, false);
    mouse_button(&rule, at(50.0, 60.0), PointerButton::Tertiary, true);
    mouse_button(&rule, at(50.0, 60.0), PointerButton::Secondary, false);
    assert_eq!(secondary.get(), 1); // no clicks
    // (the tertiary button let go first, unlike the C++: a finger's event while it is held drops the mouse's hit
    // path, as Compose's HitPathTracker drops the pointers an event doesn't carry — its next press, of a mouse
    // already down, isn't hit-tested again and goes nowhere)
    mouse_button(&rule, at(50.0, 60.0), PointerButton::Tertiary, false);
    tap(&rule, at(50.0, 60.0)); // after a finger's tap: only a mouse has that button anyway
    mouse_button(&rule, at(50.0, 60.0), PointerButton::Secondary, true);
    mouse_button(&rule, at(50.0, 60.0), PointerButton::Secondary, false);
    assert_eq!(secondary.get(), 2);
    click(&rule, at(50.0, 100.0));
    rule.host().set_keyboard_modifiers(KeyModifiers::Ctrl);
    click(&rule, at(50.0, 100.0));
    rule.host().set_keyboard_modifiers(KeyModifiers::None);
    assert_eq!(with_ctrl.get(), 1);
    tap(&rule, at(50.0, 140.0));
    rule.advance_time_by(400);
    click(&rule, at(50.0, 140.0));
    assert_eq!(mouse_only.get(), 1);
    for _ in 0..2 {
        // a double click
        rule.host().pointer_down(at(50.0, 180.0), -1, 0, PointerType::Mouse);
        rule.advance_frame();
        rule.host().pointer_up(at(50.0, 180.0), -1, 0);
        rule.advance_time_by(48);
    }
    rule.advance_time_by(400);
    rule.host().pointer_down(at(50.0, 180.0), -1, 0, PointerType::Mouse); // a long press
    rule.advance_time_by(800);
    rule.host().pointer_up(at(50.0, 180.0), -1, 0);
    rule.advance_time_by(400);
    assert_eq!((doubles.get(), longs.get()), (1, 1));
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).fetch_semantics_node().unclipped_bounds_in_root()
}

fn exists(rule: &KompozTestRule, tag: &str) -> bool {
    rule.on_node_with_tag(tag).exists()
}

fn mouse_to(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_move(p, -1, 0);
}

fn tooltip_area_shows_by_the_mouse_after_a_moment() {
    // C++ 29131: after the delay (500 ms by default) the tooltip shows 16 dp below the cursor, following it; a press
    // hides it, the mouse resting again shows it, leaving hides it; one placed by the area's bounds goes below it,
    // centered.
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Box(
            modifier = Modifier.fill_max_size(),
            content = || {
                Box(
                    modifier = Modifier.padding(40.dp()),
                    content = || {
                        TooltipArea(
                            tooltip = || Box(modifier = Modifier.size_wh(40.dp(), 20.dp()).test_tag("ipucu")),
                            modifier = Modifier.test_tag("alan"),
                            content = || Box(modifier = Modifier.size_wh(120.dp(), 60.dp())),
                        );
                    },
                );
                Box(
                    modifier = Modifier.padding_sides(start = 200.dp(), top = 400.dp()),
                    content = || {
                        TooltipArea(
                            tooltip = || Box(modifier = Modifier.size_wh(30.dp(), 10.dp()).test_tag("altında")),
                            delay_millis = 100,
                            tooltip_placement = TooltipPlacement::component_rect(
                                Alignment::BottomCenter,
                                Alignment::BottomCenter,
                                DpOffset::Zero,
                            ),
                            content = || Box(modifier = Modifier.size_wh(80.dp(), 40.dp())),
                        );
                    },
                );
            },
        );
    }));
    mouse_to(&rule, at(60.0, 50.0));
    rule.advance_time_by(300);
    assert!(!exists(&rule, "ipucu")); // not yet
    mouse_to(&rule, at(70.0, 55.0));
    rule.advance_time_by(300);
    assert!(exists(&rule, "ipucu"));
    let tip = bounds(&rule, "ipucu");
    assert_eq!((tip.left, tip.top), (70.0, 55.0 + 16.0)); // at the pointer, 16 dp below
    mouse_to(&rule, at(90.0, 60.0)); // it follows the mouse
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "ipucu").left, 90.0);
    rule.host().pointer_down(at(90.0, 60.0), -1, 0, PointerType::Mouse); // a press hides it
    rule.host().pointer_up(at(90.0, 60.0), -1, 0);
    rule.wait_for_idle();
    assert!(!exists(&rule, "ipucu"));
    mouse_to(&rule, at(91.0, 60.0));
    rule.advance_time_by(600);
    assert!(exists(&rule, "ipucu"));
    mouse_to(&rule, at(300.0, 300.0)); // leaving hides it
    rule.wait_for_idle();
    assert!(!exists(&rule, "ipucu"));
    // By the area: below it, centered.
    mouse_to(&rule, at(220.0, 420.0));
    rule.advance_time_by(200);
    assert!(exists(&rule, "altında"));
    let below = bounds(&rule, "altında");
    assert_eq!((below.left, below.top), (200.0 + 40.0 - 15.0, 440.0));
}

/// A platform keeping its clipboard.
#[derive(Default)]
struct ClipboardPlatform {
    clipboard: RefCell<Option<String>>,
}

impl kompoz_ui::platform::PlatformCallbacks for ClipboardPlatform {
    fn get_clip_entry(&self) -> Option<kompoz_ui::clipboard::ClipEntry> {
        self.clipboard.borrow().clone().map(kompoz_ui::clipboard::ClipEntry::with_plain_text)
    }
    fn set_clip_entry(&self, entry: Option<kompoz_ui::clipboard::ClipEntry>) {
        *self.clipboard.borrow_mut() = entry.and_then(|e| e.read_plain_text());
    }
}

/// A right click (the mouse's secondary button pressed and let go) at `p`.
fn secondary_click(rule: &KompozTestRule, p: Offset) {
    mouse_button(rule, p, PointerButton::Secondary, true);
    mouse_button(rule, p, PointerButton::Secondary, false);
    rule.wait_for_idle();
}

/// A menu row's bounds: the clickable row its label is merged into.
fn row(rule: &KompozTestRule, label: &str) -> Rect {
    rule.on_node_with_text(label).fetch_semantics_node().unclipped_bounds_in_root()
}

fn right_click_opens_context_menus() {
    // C++ 20659: an area's menu opens where it was clicked, its items as rows (Compose's: 112 to 280 dp wide, at
    // least 32 high, the column 4 dp in); a disabled item does nothing; an item runs and closes it; Escape and a
    // click outside close it; near the window's corner it opens towards the inside; a disabled area opens nothing; a
    // text field's own menu (cut, copy, paste, select all) and a selection container's (copy, select all).
    let picked: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let field = TextFieldState::new("merhaba dünya");
    let (p, f) = (picked.clone(), field.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (p, f) = (p.clone(), f.clone());
            Column(
                content = move || {
                    BasicTextField(
                        f.clone(),
                        modifier = Modifier.width(300.dp()).test_tag("alan"),
                        line_limits = TextFieldLineLimits::SingleLine,
                    );
                    let p = p.clone();
                    ContextMenuArea(
                        items = move || {
                            let (a, b) = (p.clone(), p.clone());
                            vec![
                                ContextMenuItem::new("Yenile", move || a.borrow_mut().push("Yenile".into())),
                                ContextMenuItem::with_enabled("Sil", false, move || b.borrow_mut().push("Sil".into())),
                            ]
                        },
                        content = || Box(modifier = Modifier.size_wh(400.dp(), 300.dp()).test_tag("kart")),
                    );
                    ContextMenuArea(
                        items = || vec![ContextMenuItem::new("Gizli", || {})],
                        enabled = false,
                        content = || Box(modifier = Modifier.size_wh(100.dp(), 20.dp()).test_tag("kapalı")),
                    );
                    Text("Sayfa", modifier = Modifier.test_tag("sayfa"));
                    ContextMenuArea(
                        items = || {
                            vec![ContextMenuItem::new(
                                "Bu menü öğesinin adı bir menünün en geniş halinden de uzun",
                                || {},
                            )]
                        },
                        content = || Box(modifier = Modifier.size_wh(400.dp(), 100.dp()).test_tag("uzun")),
                    );
                    SelectionContainer(content = || Text("seçilir metin", modifier = Modifier.test_tag("metin")));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let platform = Rc::new(ClipboardPlatform::default());
    *platform.clipboard.borrow_mut() = Some("pano".into());
    rule.host().set_callbacks(platform.clone());
    let host = rule.host();
    // An area's menu opens where it was clicked, its items as rows.
    let card = bounds(&rule, "kart");
    let at_ = at(50.0, card.top + 30.0);
    secondary_click(&rule, at_);
    assert_eq!(host.overlay_count(), 1);
    let first = row(&rule, "Yenile");
    assert_eq!((first.left, first.top), (at_.x, at_.y + 4.0), "{first:?}");
    assert!(first.height() >= 32.0 && first.width() >= 112.0, "{first:?}");
    assert_eq!(row(&rule, "Sil").width(), first.width()); // as wide as the widest
    rule.on_node_with_text("Sil").assert_is_not_enabled();
    rule.on_node_with_text("Sil").perform_click(); // disabled: nothing
    rule.wait_for_idle();
    assert!(picked.borrow().is_empty() && host.overlay_count() == 1);
    rule.on_node_with_text("Yenile").perform_click(); // runs and closes
    rule.wait_for_idle();
    assert_eq!(*picked.borrow(), ["Yenile"]);
    assert_eq!(host.overlay_count(), 0);
    secondary_click(&rule, at_);
    host.key_down(kompoz_ui::input::Key::Escape, KeyModifiers::None);
    host.key_up(kompoz_ui::input::Key::Escape, KeyModifiers::None);
    rule.wait_for_idle();
    assert_eq!(host.overlay_count(), 0);
    // Near the window's corner it opens towards the inside.
    let corner = at(390.0, card.bottom - 4.0);
    secondary_click(&rule, corner);
    let flipped = row(&rule, "Yenile");
    assert!(flipped.right <= corner.x + 0.5 && flipped.left > 0.0, "{flipped:?}");
    // A click outside closes it.
    host.pointer_down(at(10.0, 790.0), -1, 0, PointerType::Mouse);
    host.pointer_up(at(10.0, 790.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(host.overlay_count(), 0);
    secondary_click(&rule, bounds(&rule, "sayfa").center()); // nothing there
    secondary_click(&rule, bounds(&rule, "kapalı").center()); // disabled
    assert_eq!(host.overlay_count(), 0);
    // A text field's own: its caret at the end (Compose's, unlike the C++'s focus and caret at the click: only
    // macOS selects the word clicked), so nothing to cut or copy yet.
    let alan = bounds(&rule, "alan");
    secondary_click(&rule, alan.center());
    assert_eq!(host.overlay_count(), 1);
    rule.on_node_with_text("Cut").assert_is_not_enabled();
    rule.on_node_with_text("Copy").assert_is_not_enabled();
    rule.on_node_with_text("Paste").assert_is_enabled();
    rule.on_node_with_text("Select all").perform_click();
    rule.wait_for_idle();
    assert_eq!(field.selection(), kompoz_text::TextRange::new(0, "merhaba dünya".len()));
    // Copy (the selection collapsed to its end, Compose's `copy()`), cut, paste.
    secondary_click(&rule, alan.center());
    rule.on_node_with_text("Copy").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    assert_eq!(platform.clipboard.borrow().as_deref(), Some("merhaba dünya"));
    assert_eq!(field.text(), "merhaba dünya");
    assert!(field.selection().collapsed());
    secondary_click(&rule, alan.center());
    rule.on_node_with_text("Select all").perform_click();
    rule.wait_for_idle();
    secondary_click(&rule, alan.center());
    rule.on_node_with_text("Cut").perform_click();
    rule.wait_for_idle();
    assert!(field.text().is_empty() && platform.clipboard.borrow().as_deref() == Some("merhaba dünya"));
    secondary_click(&rule, alan.center());
    rule.on_node_with_text("Paste").perform_click();
    rule.wait_for_idle();
    assert_eq!(field.text(), "merhaba dünya");
    assert_eq!(host.overlay_count(), 0);
    // A long item: the menu 280 dp wide at most (Material's menu).
    secondary_click(&rule, at(10.0, bounds(&rule, "uzun").top + 10.0));
    assert_eq!(row(&rule, "Bu menü öğesinin adı bir menünün en geniş halinden de uzun").width(), 280.0);
    host.key_down(kompoz_ui::input::Key::Escape, KeyModifiers::None);
    host.key_up(kompoz_ui::input::Key::Escape, KeyModifiers::None);
    rule.wait_for_idle();
    // A selection container's: copy only (Compose desktop's SelectionManager text manager), enabled once text is
    // selected (a double click selects a word).
    let metin = bounds(&rule, "metin");
    secondary_click(&rule, metin.center());
    assert!(!rule.on_node_with_text("Cut").exists() && !rule.on_node_with_text("Select all").exists());
    rule.on_node_with_text("Copy").assert_is_not_enabled();
    host.key_down(kompoz_ui::input::Key::Escape, KeyModifiers::None);
    host.key_up(kompoz_ui::input::Key::Escape, KeyModifiers::None);
    rule.wait_for_idle();
    assert_eq!(host.overlay_count(), 0);
    let word = at(metin.left + 4.0, metin.center().y);
    for _ in 0..2 {
        host.pointer_down(word, -1, 0, PointerType::Mouse);
        rule.advance_frame();
        host.pointer_up(word, -1, 0);
        rule.advance_frame();
    }
    rule.wait_for_idle();
    secondary_click(&rule, word);
    rule.on_node_with_text("Copy").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    assert_eq!(platform.clipboard.borrow().as_deref(), Some("seçilir"));
    assert_eq!(host.overlay_count(), 0);
}

/// The thumb drawn in the bar `tag` (a filled rect there of the thumb's black), and its color.
fn thumb_in(rule: &KompozTestRule, tag: &str) -> Option<(Rect, Color)> {
    let mut canvas = RecordingCanvas::new();
    rule.host().draw(&mut canvas);
    let bar = bounds(rule, tag);
    canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .find(|(r, c)| bar.contains(r.center()) && c.alpha() > 0.0 && c.red() == 0.0)
        .map(|(r, c)| (*r, *c))
}

fn near(a: f32, b: f32, by: f32) -> bool {
    (a - b).abs() <= by
}

fn scrollbars_show_where_the_content_is_and_move_it() {
    // C++ 20537: a vertical scrollbar of a scroll state (its thumb a fifth of the track for 200 of 1000 px in view,
    // where the content is; dragged, it scrolls by as much as its travel stands for — Compose's from the press, no
    // touch slop: the C++'s took it off —; a press on the track pages towards it, on the thumb does nothing; hovered
    // darker; at least 16 px; none with nothing to scroll), a lazy list's (estimated from the items in view) and a
    // horizontal one.
    let scroll: Rc<RefCell<Option<(ScrollState, ScrollState, LazyListState)>>> = Rc::new(RefCell::new(None));
    let content_height = mutable_state_of(1000);
    let (sc, ch) = (scroll.clone(), content_height.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (scroll, wide, list) = (remember_scroll_state(), remember_scroll_state(), remember_lazy_list_state());
            *sc.borrow_mut() = Some((scroll.clone(), wide.clone(), list.clone()));
            let ch = ch.clone();
            Column(
                content = move || {
                    let (scroll, ch) = (scroll.clone(), ch.clone());
                    Box(
                        modifier = Modifier.size_wh(300.dp(), 200.dp()),
                        content = move || {
                            let h = ch.get();
                            Column(
                                modifier = Modifier.fill_max_size().vertical_scroll(&scroll),
                                content = move || Box(modifier = Modifier.fill_max_width().height((h as f32).dp())),
                            );
                            VerticalScrollbar(
                                remember_scrollbar_adapter(scroll.clone()),
                                modifier = Modifier.align(Alignment::CenterEnd).fill_max_height().test_tag("dikey"),
                            );
                        },
                    );
                    let list = list.clone();
                    Box(
                        modifier = Modifier.size_wh(300.dp(), 300.dp()),
                        content = move || {
                            LazyColumn(
                                modifier = Modifier.fill_max_size(),
                                state = Some(list.clone()),
                                content = |this: &mut LazyListScope| {
                                    this.items(100, |i| {
                                        Text(format!("Satır {i}"), modifier = Modifier.height(50.dp()));
                                    });
                                },
                            );
                            VerticalScrollbar(
                                remember_scrollbar_adapter(list.clone()),
                                modifier = Modifier.align(Alignment::CenterEnd).fill_max_height().test_tag("liste"),
                            );
                        },
                    );
                    let wide = wide.clone();
                    Box(
                        modifier = Modifier.size_wh(200.dp(), 60.dp()),
                        content = move || {
                            Row(
                                modifier = Modifier.fill_max_size().horizontal_scroll(&wide),
                                content = || Box(modifier = Modifier.width(800.dp()).fill_max_height()),
                            );
                            HorizontalScrollbar(
                                remember_scrollbar_adapter(wide.clone()),
                                modifier = Modifier.align(Alignment::BottomCenter).fill_max_width().test_tag("yatay"),
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let (scroll, wide, list) = scroll.borrow().clone().unwrap();
    let style = default_scrollbar_style();
    let host = rule.host();
    let wait = |rule: &KompozTestRule| rule.wait_for_idle();
    // 8 px at the end; the thumb a fifth of the track (200 of 1000 in view), at the start.
    let bar = bounds(&rule, "dikey");
    assert!(bar.width() == 8.0 && bar.height() == 200.0 && bar.right == 300.0 && scroll.viewport_size() == 200);
    let t = thumb_in(&rule, "dikey").expect("a thumb");
    assert!(t.0.top == bar.top && t.0.height() == 40.0 && t.1 == style.unhover_color, "{t:?}");
    // Scrolled half way, the thumb is half way down its travel.
    let coroutines = kompoz_coroutines::CoroutineScope::new(host.dispatcher());
    let launch = |f: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>| {
        coroutines.launch(f);
    };
    let s2 = scroll.clone();
    launch(Box::pin(async move {
        s2.scroll_to(400).await;
    }));
    wait(&rule);
    assert_eq!(thumb_in(&rule, "dikey").unwrap().0.top, bar.top + 80.0);
    // Dragged, the thumb moves the content by as much as its travel stands for (800 / 160 per px).
    let grab = at(bar.center().x, bar.top + 100.0);
    host.pointer_down(grab, -1, 0, PointerType::Mouse);
    for d in [4.0, 12.0, 24.0] {
        rule.advance_frame();
        host.pointer_move(at(grab.x, grab.y + d), -1, 0);
    }
    rule.advance_time_by(200);
    assert!(near(scroll.value() as f32, 400.0 + 24.0 * 5.0, 1.0), "{}", scroll.value());
    assert_ne!(thumb_in(&rule, "dikey").unwrap().1, style.unhover_color); // darker while dragged
    host.pointer_up(at(grab.x, grab.y + 24.0), -1, 0);
    wait(&rule);
    // A press on the track pages towards it.
    let before = scroll.value();
    click(&rule, at(bar.center().x, bar.top + 195.0));
    wait(&rule);
    assert_eq!(scroll.value(), before + 200);
    click(&rule, at(bar.center().x, bar.top + 2.0));
    wait(&rule);
    assert_eq!(scroll.value(), before);
    let t = thumb_in(&rule, "dikey").unwrap();
    click(&rule, t.0.center()); // a press on the thumb itself: nothing
    wait(&rule);
    assert_eq!(scroll.value(), before);
    // The mouse over it darkens it.
    // (from outside it: the clicks, teleported there, entered it pressed, which isn't a hover)
    host.pointer_move(at(100.0, 700.0), -1, 0);
    rule.advance_time_by(400);
    host.pointer_move(bar.center(), -1, 0);
    rule.advance_time_by(400);
    assert_eq!(thumb_in(&rule, "dikey").unwrap().1, style.hover_color);
    host.pointer_move(at(100.0, 700.0), -1, 0);
    rule.advance_time_by(400);
    // A very long content: the thumb keeps 16 px. Nothing to scroll: no thumb.
    content_height.set(20000);
    wait(&rule);
    assert_eq!(thumb_in(&rule, "dikey").unwrap().0.height(), 16.0);
    content_height.set(100);
    wait(&rule);
    assert!(thumb_in(&rule, "dikey").is_none());
    // A lazy list's: 100 items of 50 in 300 px, the thumb 18 px; at the 47th item and 25 px, where that is.
    let list_bar = bounds(&rule, "liste");
    let t = thumb_in(&rule, "liste").unwrap();
    assert!(t.0.height() == 18.0 && t.0.top == list_bar.top, "{t:?}");
    let l = list.clone();
    launch(Box::pin(async move { kompoz_runtime::named_args!(l.scroll_to_item(47, scroll_offset = 25)).await }));
    wait(&rule);
    let t = thumb_in(&rule, "liste").unwrap();
    assert!(near(t.0.top, list_bar.top + (300.0 - 18.0) * 2375.0 / 4700.0, 1.0), "{t:?}");
    // A press on its track pages by the 300 px in view, keeping the offset into the item.
    click(&rule, at(list_bar.center().x, list_bar.bottom - 3.0));
    wait(&rule);
    assert_eq!((list.first_visible_item_index(), list.first_visible_item_scroll_offset()), (53, 25));
    // Dragging its thumb to the end shows the last items.
    let t = thumb_in(&rule, "liste").unwrap();
    let g = at(list_bar.center().x, t.0.center().y);
    host.pointer_down(g, -1, 0, PointerType::Mouse);
    for d in [10.0, 100.0, 200.0] {
        rule.advance_frame();
        host.pointer_move(at(g.x, g.y + d), -1, 0);
    }
    rule.advance_time_by(200);
    host.pointer_up(at(g.x, g.y + 200.0), -1, 0);
    wait(&rule);
    assert!(list.first_visible_item_index() >= 93, "{}", list.first_visible_item_index());
    // Across: the thumb a quarter of the track, at the bottom edge.
    let across = bounds(&rule, "yatay");
    let t = thumb_in(&rule, "yatay").unwrap();
    assert!(across.height() == 8.0 && t.0.width() == 50.0 && t.0.left == across.left, "{across:?} {t:?}");
    let w = wide.clone();
    launch(Box::pin(async move {
        w.scroll_to(600).await;
    }));
    wait(&rule);
    assert_eq!(thumb_in(&rule, "yatay").unwrap().0.right, across.right);
}

fn value_based_field_has_its_context_menu_and_scrollbar() {
    // A value-based field (CoreTextField's root box) has the desktop's text context menu too — select all, copy
    // (the selection kept: Compose desktop's legacy `copy(false)`), cut, paste —; its scroll state an app holds
    // (Compose for desktop's TextFieldScrollState) moves with a scrollbar beside it.
    let value = mutable_state_of(kompoz_text::TextFieldValue::from_text(
        "bir\niki\nüç\ndört\nbeş\naltı\nyedi\nsekiz",
        kompoz_text::TextRange::Zero,
        None,
    ));
    let held: Rc<RefCell<Option<TextFieldScrollState>>> = Rc::new(RefCell::new(None));
    let (v, h) = (value.clone(), held.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let scroll_state = remember_text_field_vertical_scroll_state();
            *h.borrow_mut() = Some(scroll_state.clone());
            let v = v.clone();
            Box(
                modifier = Modifier.size_wh(200.dp(), 60.dp()),
                content = move || {
                    let v2 = v.clone();
                    BasicTextFieldValue(
                        v.get(),
                        move |it: kompoz_text::TextFieldValue| v2.set(it),
                        modifier = Modifier.fill_max_size().test_tag("eski"),
                        scroll_state = Some(scroll_state.clone()),
                    );
                    VerticalScrollbar(
                        remember_scrollbar_adapter(scroll_state.clone()),
                        modifier = Modifier.align(Alignment::CenterEnd).fill_max_height().test_tag("çubuk"),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let platform = Rc::new(ClipboardPlatform::default());
    *platform.clipboard.borrow_mut() = Some("pano".into());
    rule.host().set_callbacks(platform.clone());
    let scroll_state = held.borrow().clone().unwrap();
    // More lines than fit: a thumb shorter than the track; a press on the track below it scrolls a viewport.
    assert!(scroll_state.max_offset() > 0.0 && scroll_state.viewport_size() == 60);
    let bar = bounds(&rule, "çubuk");
    let thumb = thumb_in(&rule, "çubuk").expect("a thumb");
    assert!(thumb.0.height() < bar.height() && thumb.0.top == bar.top, "{thumb:?}");
    click(&rule, at(bar.center().x, bar.bottom - 2.0));
    rule.wait_for_idle();
    assert_eq!(scroll_state.offset(), 60.0f32.min(scroll_state.max_offset()));
    assert!(thumb_in(&rule, "çubuk").unwrap().0.top > bar.top);
    // Its menu.
    let field = bounds(&rule, "eski");
    let all = "bir\niki\nüç\ndört\nbeş\naltı\nyedi\nsekiz";
    secondary_click(&rule, at(field.left + 10.0, field.top + 10.0));
    assert_eq!(rule.host().overlay_count(), 1);
    rule.on_node_with_text("Cut").assert_is_not_enabled();
    rule.on_node_with_text("Copy").assert_is_not_enabled();
    rule.on_node_with_text("Paste").assert_is_enabled();
    rule.on_node_with_text("Select all").perform_click();
    rule.wait_for_idle();
    assert_eq!(value.peek().selection(), kompoz_text::TextRange::new(0, all.len()));
    secondary_click(&rule, at(field.left + 10.0, field.top + 10.0));
    rule.on_node_with_text("Copy").assert_is_enabled().perform_click();
    rule.wait_for_idle();
    assert_eq!(platform.clipboard.borrow().as_deref(), Some(all));
    assert_eq!(value.peek().selection(), kompoz_text::TextRange::new(0, all.len()));
    secondary_click(&rule, at(field.left + 10.0, field.top + 10.0));
    rule.on_node_with_text("Cut").perform_click();
    rule.wait_for_idle();
    assert!(value.peek().text().is_empty());
    secondary_click(&rule, at(field.left + 10.0, field.top + 10.0));
    rule.on_node_with_text("Paste").perform_click();
    rule.wait_for_idle();
    assert_eq!(value.peek().text(), all);
    assert_eq!(rule.host().overlay_count(), 0);
}

fn on_drag_follows_the_matched_pointer() {
    // Modifier.on_drag: a press the matcher takes dragged past the touch slop starts it (where the press was), each
    // move told, the matched release ends it; a finger, which the mouse-only matcher doesn't take, drags nothing;
    // matchers combine (`+`).
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let l = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (a, b, c) = (l.clone(), l.clone(), l.clone());
        Box(modifier = Modifier.size_wh(200.dp(), 100.dp()).on_drag(
            matcher = PointerMatcher::mouse(PointerButton::Primary),
            on_drag_start = move |at: Offset| a.borrow_mut().push(format!("start {}", at.x)),
            on_drag_end = move || b.borrow_mut().push("end".into()),
            on_drag = move |delta: Offset| c.borrow_mut().push(format!("{}", delta.x)),
        ));
    }));
    let slop = kompoz_ui::platform::ViewConfiguration::default().touch_slop;
    rule.host().pointer_down(at(20.0, 50.0), -1, 0, PointerType::Mouse);
    for x in [30.0, 60.0] {
        rule.advance_frame();
        rule.host().pointer_move(at(x, 50.0), -1, 0);
    }
    rule.host().pointer_up(at(60.0, 50.0), -1, 0);
    rule.wait_for_idle();
    // (the first move less the mouse's slop — a fraction of the touch slop —, then the second whole)
    let got = log.borrow().clone();
    assert!(got.len() == 4 && got[0] == "start 20" && got[2] == "30" && got[3] == "end", "{got:?}");
    let first: f32 = got[1].parse().unwrap();
    assert!(first > 10.0 - slop && first < 10.0, "{got:?}");
    log.borrow_mut().clear();
    rule.host().pointer_down(at(20.0, 50.0), -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_move(at(80.0, 50.0), -1, 0);
    rule.host().pointer_up(at(80.0, 50.0), -1, 0);
    rule.wait_for_idle();
    assert!(log.borrow().is_empty());
    let both = PointerMatcher::mouse(PointerButton::Primary) + PointerMatcher::Touch;
    assert_eq!(both, PointerMatcher::mouse(PointerButton::Primary).plus(&PointerMatcher::Touch));
    assert_ne!(both, PointerMatcher::Primary);
}

tests!(
    on_drag_follows_the_matched_pointer,
    value_based_field_has_its_context_menu_and_scrollbar,
    scrollbars_show_where_the_content_is_and_move_it,
    on_click_matches_buttons_pointers_and_keys,
    tooltip_area_shows_by_the_mouse_after_a_moment,
    right_click_opens_context_menus,
);

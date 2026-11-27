//! Compose for desktop's foundation extras in Kotlin's syntax, as a Compose user writes them: kompozable_tests.cpp
//! 3044's `InputForms` (`ContextMenuArea(items = { listOf(ContextMenuItem("…") { }) }) { }`, `TooltipArea(tooltip =,
//! delayMillis =, tooltipPlacement = TooltipPlacement.ComponentRect(…))`, `Modifier.onClick(matcher =,
//! keyboardModifiers = { })`, `onPointerEvent`, `transformable(state, canPan = { })`, `clickable(interactionSource =
//! null, indication = null)`, `Popup(popupPositionProvider = object : PopupPositionProvider { … }, properties =
//! PopupProperties(focusable = false))`) through `kompoz!`; and from a `.kt` file
//! (kompoz_file!) scrollbars of a scroll state and a lazy list (`rememberScrollbarAdapter`), `Modifier.onDrag`, a
//! `ContextMenuDataProvider`, a text field's own menu and a value-based field's scroll state. The same components the
//! Rust syntax's tests check (crates/kompoz-foundation/tests/desktop.rs). And Compose Desktop's windows and tray as
//! an app writes them (`Window(onCloseRequest =, state = rememberWindowState(…), …) { … window.title … }`,
//! `WindowPosition(Alignment.Center)`, `Tray(icon =, state = rememberTrayState(), …) { Item(…); CheckboxItem(…);
//! Separator(); Menu(…) { … } }`, `trayState.sendNotification(Notification(…, Notification.Type.Info))`,
//! `exitApplication()`), the requests they make checked (what the platform's app core opens: its own tests run them).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, RecordingCanvas};
use kompoz::ui::input::{PointerButton, PointerEventType, PointerKeyboardModifiers, PointerType};
use kompoz::ui::window::{
    ApplicationScope, ComposeWindow, FrameWindowScope, KeyShortcut, MenuItemKind, Notification, NotificationType,
    Popup, PopupPositionProvider, PopupProperties, WindowPosition,
};
use std::cell::Cell;
use std::rc::Rc;

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

// ---- input_forms_as_compose_writes_them (kompozable_tests.cpp 3044) --------------------------------------------

kompoz! {
    @Composable
    fun InputForms() {
        var picked by remember { mutableStateOf("yok") }
        var clicks by remember { mutableStateOf(0) }
        var moved by remember { mutableStateOf(Offset.Zero) }
        var zoom by remember { mutableStateOf(1f) }
        val transform = rememberTransformableState { _, z, pan, _ ->
            zoom *= z
            moved += pan
        }
        Column {
            ContextMenuArea(items = { listOf(ContextMenuItem("Yenile") { picked = "Yenile" }, ContextMenuItem("Sil", false) {}) }) {
                Box(modifier = Modifier.size(100.dp, 40.dp).testTag("alan"))
            }
            Text("seçim: $picked")
            TooltipArea(tooltip = { Text("ipucu", modifier = Modifier.testTag("ipucu")) }, delayMillis = 100,
                        tooltipPlacement = TooltipPlacement.ComponentRect(Alignment.BottomCenter, Alignment.BottomCenter, DpOffset(0.dp, 4.dp))) {
                Box(modifier = Modifier.size(100.dp, 40.dp).testTag("ipuçlu"))
            }
            Box(modifier = Modifier.size(100.dp, 40.dp).testTag("tıklanır")
                    .onClick(matcher = PointerMatcher.mouse(PointerButton.Secondary)) { clicks++ }
                    .onClick(keyboardModifiers = { it.isCtrlPressed }) { clicks += 10 }
                    .onPointerEvent(PointerEventType.Move) { moved = it.changes.first().position }
                    .transformable(transform, canPan = { zoom > 1f })
                    .clickable(interactionSource = null, indication = null) { clicks += 100 })
            Text("tık: $clicks · ${moved.x.roundToInt()}")
            var open by remember { mutableStateOf(true) }
            if (open) {
                Popup(popupPositionProvider = object : PopupPositionProvider {
                    override fun calculatePosition(anchorBounds: IntRect, windowSize: IntSize, layoutDirection: LayoutDirection, popupContentSize: IntSize): IntOffset {
                        return IntOffset(anchorBounds.left + 10, anchorBounds.bottom + popupContentSize.height)
                    }
                }, onDismissRequest = { open = false }, properties = PopupProperties(focusable = false)) {
                    Text("açılır", modifier = Modifier.testTag("açılır"))
                }
            }
        }
    }
}

fn input_forms_as_compose_writes_them() {
    let rule = KompozTestRule::with_viewport(InputForms, IntSize::new(400, 800));
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("seçim: yok").exists() && rule.on_node_with_tag("açılır").exists());
    assert_eq!(rule.host().overlay_count(), 1);
    assert!(near(bounds(&rule, "açılır").left, 10.0));
    let area = bounds(&rule, "alan");
    let at = Offset::new(area.left + 50.0, area.top + 20.0);
    rule.host().mouse_button(at, -1, PointerButton::Secondary, true);
    rule.host().mouse_button(at, -1, PointerButton::Secondary, false);
    rule.wait_for_idle();
    rule.on_node_with_text("Sil").assert_is_not_enabled();
    rule.on_node_with_text("Yenile").perform_click();
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("seçim: Yenile").exists());
    let tipped = bounds(&rule, "ipuçlu");
    rule.host().pointer_move(Offset::new(tipped.left + 50.0, tipped.top + 20.0), -1, 0);
    rule.advance_time_by(300);
    let tip = bounds(&rule, "ipucu");
    assert!(near(tip.top, tipped.bottom + 4.0), "{tip:?} {tipped:?}");
    let clickable = bounds(&rule, "tıklanır");
    let c = Offset::new(clickable.left + 50.0, clickable.top + 20.0);
    // (the tooltip lies over the box, shown under the mouse: the mouse's first move there enters it, which keeps it
    // — Compose's TooltipArea —, its next, out of the area, hides it; then the box: entered, then moved to 50)
    for x in [c.x + 1.0, c.x + 2.0, c.x + 3.0, c.x] {
        rule.host().pointer_move(Offset::new(x, c.y), -1, 0);
        rule.wait_for_idle();
    }
    assert!(!rule.on_node_with_tag("ipucu").exists());
    rule.host().mouse_button(c, -1, PointerButton::Secondary, true);
    rule.host().mouse_button(c, -1, PointerButton::Secondary, false);
    rule.advance_time_by(400);
    rule.on_node_with_tag("tıklanır").perform_click();
    rule.wait_for_idle();
    // (the secondary click 1, the primary one: the first onClick isn't its, the second's keyboard test false, the
    // clickable 100; the move at 50)
    assert!(rule.on_node_with_text("tık: 101 · 50").exists());
}

// ---- From a .kt file: scrollbars, onDrag, a context menu's data provider, the text fields' menus ----------------

kompoz_file!("tests/kt/desktop.kt");

/// The thumb drawn in the bar `tag`: a filled rect there of black.
fn thumb_in(rule: &KompozTestRule, tag: &str) -> Option<Rect> {
    let mut canvas = RecordingCanvas::new();
    rule.host().draw(&mut canvas);
    let bar = bounds(rule, tag);
    canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .find(|(r, c)| bar.contains(r.center()) && c.alpha() > 0.0 && c.red() == 0.0)
        .map(|(r, _)| *r)
}

fn scrollbars_on_drag_and_menus_from_a_kt_file() {
    let rule = KompozTestRule::with_viewport(DesktopForms, IntSize::new(400, 800));
    rule.wait_for_idle();
    // A scroll state's scrollbar: 200 of 1000 in view, the thumb a fifth of the track; a press below it pages.
    let bar = bounds(&rule, "dikey");
    assert!(near(thumb_in(&rule, "dikey").expect("a thumb").height(), 40.0));
    let below = Offset::new(bar.center().x, bar.bottom - 3.0);
    rule.host().pointer_down(below, -1, 0, PointerType::Mouse);
    // (the press launches the track's scrolling, dispatched as Compose for desktop's TrackPressScroller launches it:
    // it runs before the release comes, which cancels it — a release in the same turn would cancel it unstarted)
    rule.main_clock().run_current();
    rule.host().pointer_up(below, -1, 0);
    rule.wait_for_idle();
    rule.on_node_with_text("kaydırma: 200").assert_exists();
    // A lazy list's: 100 rows of 50 in 300, an 18 px thumb.
    assert!(near(thumb_in(&rule, "liste").expect("a thumb").height(), 18.0));
    // onDrag with the mouse's secondary button: its moves summed, ended by its release.
    let drag = bounds(&rule, "sürükle");
    let start = drag.center();
    // (the mouse comes over it first, an event of its own: Compose for desktop's matcher-taking detectors launch their
    // coroutines (plain `launch`, dispatched) from the first event their pointer input sees, so a press that is that
    // first event comes before they wait for it — as it would in Compose)
    rule.host().pointer_move(start, -1, 0);
    rule.main_clock().run_current();
    rule.host().mouse_button(start, -1, PointerButton::Secondary, true);
    for dx in [10.0, 20.0, 40.0] {
        rule.advance_frame();
        rule.host().pointer_move(Offset::new(start.x + dx, start.y), -1, 0);
    }
    rule.host().mouse_button(Offset::new(start.x + 40.0, start.y), -1, PointerButton::Secondary, false);
    rule.wait_for_idle();
    rule.on_node_with_text("sürüklendi: 40 bitti").assert_exists();
    // A data provider's items after the area's own.
    let area = bounds(&rule, "menü");
    rule.host().mouse_button(area.center(), -1, PointerButton::Secondary, true);
    rule.host().mouse_button(area.center(), -1, PointerButton::Secondary, false);
    rule.wait_for_idle();
    assert!(rule.on_node_with_text("Eklenen").exists() && rule.on_node_with_text("Kendi").exists());
    let own_top = rule.on_node_with_text("Kendi").fetch_semantics_node().unclipped_bounds_in_root().top;
    let provided_top = rule.on_node_with_text("Eklenen").fetch_semantics_node().unclipped_bounds_in_root().top;
    assert!(own_top < provided_top);
    rule.on_node_with_text("Eklenen").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("seçilen: Eklenen").assert_exists();
    // The state field's menu: select all, then copy shows enabled.
    let field = bounds(&rule, "alan");
    rule.host().mouse_button(field.center(), -1, PointerButton::Secondary, true);
    rule.host().mouse_button(field.center(), -1, PointerButton::Secondary, false);
    rule.wait_for_idle();
    rule.on_node_with_text("Select all").perform_click();
    rule.wait_for_idle();
    rule.host().mouse_button(field.center(), -1, PointerButton::Secondary, true);
    rule.host().mouse_button(field.center(), -1, PointerButton::Secondary, false);
    rule.wait_for_idle();
    rule.on_node_with_text("Copy").assert_is_enabled();
}

// ---- windows_and_tray_as_compose_writes_them (Compose Desktop's Window, Tray, notifications) ---------------------

kompoz! {
    @Composable
    fun Windows(app: ApplicationScope) {
        var open by remember { mutableStateOf(true) }
        var count by remember { mutableStateOf(0) }
        val state = rememberWindowState(position = WindowPosition(Alignment.Center), size = DpSize(400.dp, 300.dp))
        val trayState = rememberTrayState()
        Tray(icon = ColorPainter(Color.Red), state = trayState, tooltip = "Pencereler", onAction = { open = true }) {
            Item("Aç", mnemonic = 'A', shortcut = KeyShortcut(Key.O, ctrl = true), onClick = { open = true })
            CheckboxItem("Sayaç", checked = count > 0, onCheckedChange = { count = if (it) 1 else 0 })
            Separator()
            Menu("Daha") {
                Item("Çık", onClick = { app.exitApplication() })
            }
        }
        if (open) {
            Window(onCloseRequest = { open = false }, state = state, title = "Mesajlar $count", resizable = false,
                   alwaysOnTop = true, onKeyEvent = { false }) {
                Column {
                    Text("pencere: ${window.title}")
                    Button(onClick = { trayState.sendNotification(Notification("Başlık", "İleti", Notification.Type.Info)) }) {
                        Text("bildir")
                    }
                }
            }
        }
    }
}

kompoz! {
    @Composable
    fun ApplicationScope.Single() {
        Window(onCloseRequest = ::exitApplication, title = "Tek") {
            Text("tek")
        }
    }
}

/// The single-window app as Compose Desktop's samples write it: an extension of the application's scope, its close
/// request `::exitApplication`.
fn a_single_window_app_exits_on_close() {
    let exited = Rc::new(Cell::new(false));
    let scope = {
        let exited = exited.clone();
        ApplicationScope::new(Rc::new(move || exited.set(true)))
    };
    let rule = KompozTestRule::with_viewport(kompozable_lambda!(move || scope.Single()), IntSize::new(100, 100));
    rule.wait_for_idle();
    let windows = rule.host().window_registry().windows();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].title(), "Tek");
    windows[0].close_requested();
    assert!(exited.get(), "::exitApplication");
}

fn windows_and_tray_as_compose_writes_them() {
    let exited = Rc::new(Cell::new(false));
    let scope = {
        let exited = exited.clone();
        ApplicationScope::new(Rc::new(move || exited.set(true)))
    };
    let rule =
        KompozTestRule::with_viewport(kompozable_lambda!(move || Windows(scope.clone())), IntSize::new(400, 400));
    rule.wait_for_idle();
    let registry = rule.host().window_registry().clone();
    let windows = registry.windows();
    assert_eq!(windows.len(), 1);
    let w = windows[0].clone();
    assert_eq!(w.title(), "Mesajlar 0");
    let p = w.properties();
    assert!(!p.resizable && p.always_on_top && p.visible && !p.undecorated);
    assert_eq!(w.state().size(), DpSize(400.dp(), 300.dp()));
    assert_eq!(w.state().position(), WindowPosition::Aligned(Alignment::Center));
    // The tray icon's menu, as the platform shows it.
    let trays = registry.trays();
    assert_eq!(trays.len(), 1);
    let t = trays[0].clone();
    assert_eq!(t.tooltip().as_deref(), Some("Pencereler"));
    let menu = t.menu();
    let texts: Vec<&str> = menu.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(texts, ["Aç", "Sayaç", "", "Daha"]);
    assert_eq!(menu[0].mnemonic, Some('A'));
    assert_eq!(menu[0].shortcut, Some(KeyShortcut { key: Key::O, ctrl: true, meta: false, alt: false, shift: false }));
    assert_eq!(menu[1].kind, MenuItemKind::Checkbox { checked: false });
    assert_eq!(menu[2].kind, MenuItemKind::Separator);
    assert_eq!(menu[3].items[0].text, "Çık");
    // The checkbox toggled: the window's title follows (the request kept up to date).
    menu[1].click();
    rule.wait_for_idle();
    assert_eq!(w.title(), "Mesajlar 1");
    assert_eq!(registry.trays()[0].menu()[1].kind, MenuItemKind::Checkbox { checked: true });
    // The window's content in a host of its own, as the platform's app core composes it: its scope's window.
    let r = w.clone();
    let host = KompozHost::new_window(
        kompozable_lambda!(move || {
            let content = r.content();
            content(&FrameWindowScope::new(ComposeWindow::new(&r)));
        }),
        w.locals(),
    );
    host.set_viewport_size(IntSize::new(400, 300));
    host.frame(0);
    let tree = host.accessibility_tree();
    assert!(tree.iter().any(|n| n.label == "pencere: Mesajlar 1"), "{tree:?}");
    let button = tree.iter().find(|n| n.clickable).expect("the button");
    assert!(host.perform_semantics_action(button.id, kompoz::ui::semantics::SemanticsAction::Click));
    host.frame(16_000_000);
    assert_eq!(
        t.take_notifications(),
        [Notification::new("Başlık", "İleti", NotificationType::Info)],
        "sent through the tray icon's state"
    );
    drop(host);
    // The user closing it: its onCloseRequest takes it out; the icon's click brings it back.
    w.close_requested();
    rule.wait_for_idle();
    assert!(registry.windows().is_empty());
    t.action();
    rule.wait_for_idle();
    assert_eq!(registry.windows().len(), 1);
    // The submenu's item ends the app.
    registry.trays()[0].menu()[3].items[0].click();
    assert!(exited.get());
}

tests![
    input_forms_as_compose_writes_them,
    scrollbars_on_drag_and_menus_from_a_kt_file,
    windows_and_tray_as_compose_writes_them,
    a_single_window_app_exits_on_close,
];

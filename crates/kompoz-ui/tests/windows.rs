//! The popup, dialog and back tests: popups laid out against the window by their providers, drawn over the main
//! tree and hit first, dismissed by presses outside and by back; dialogs; focus, keys and accessibility scoped to
//! the window on top; back handlers (plain and predictive, in popups first). The C++ Kompoz's overlay and back
//! tests without Material (a dialog and a popup of the test Text for its AlertDialog and DropdownMenu, tap
//! detectors for its buttons). Numbers in px.

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_coroutines::{Flow, delay_ms};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::back::*;
use kompoz_ui::focus::FocusRequester;
use kompoz_ui::graphics_context::*;
use kompoz_ui::input::*;
use kompoz_ui::window::*;
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

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

fn tap(rule: &KompozTestRule, at: Offset) {
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(at, -1, 0);
    rule.advance_frame();
}

fn escape(rule: &KompozTestRule) -> bool {
    let taken = rule.host().key_down(Key::Escape, KeyModifiers::None);
    rule.host().key_up(Key::Escape, KeyModifiers::None);
    taken
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// Something tapped logs `name` (a Material button's click, by foundation's tap detector).
#[kompozable(inline)]
fn Tappable(log: Log, name: &'static str, modifier: Modifier, content: impl FnOnce()) {
    Box(
        modifier.pointer_input(name, move |s: PointerInputScope| {
            let log = log.clone();
            async move { s.detect_tap_gestures().on_tap(move |_| push(&log, name)).await }
        }),
        content = content,
    );
}

/// Places a popup under its anchor, from its start (what a dropdown menu's provider does, without its margins).
struct Below;

impl PopupPositionProvider for Below {
    fn calculate_position(&self, anchor: IntRect, _: IntSize, _: LayoutDirection, _: IntSize) -> IntOffset {
        IntOffset::new(anchor.left, anchor.bottom)
    }
}

/// What a recording canvas needs: the texts drawn, in order.
#[derive(Default)]
struct Texts {
    texts: Vec<String>,
}

impl GraphicsContext for Texts {
    fn fill_round_rect(&mut self, _: Rect, _: CornerRadii, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: CornerRadii, _: f32, _: Color) {}
    fn draw_line(&mut self, _: Offset, _: Offset, _: f32, _: Color) {}
    fn draw_text(&mut self, text: &TextDraw, _: Color) {
        self.texts.push(text.text.to_string());
    }
    fn draw_arc(&mut self, _: Rect, _: f32, _: f32, _: f32, _: Color, _: bool) {}
    fn draw_image(&mut self, _: &ImageBitmap, _: Rect, _: Rect, _: f32, _: FilterQuality) {}
    fn push_layer(&mut self, _: Rect, _: CornerRadii, _: f32, _: LayerTransform) {}
    fn pop_layer(&mut self) {}
    fn push_clip(&mut self, _: Rect) {}
    fn push_clip_round_rect(&mut self, _: Rect, _: CornerRadii, _: ClipOp) {}
    fn push_clip_path(&mut self, _: &Path, _: Offset, _: ClipOp) {}
    fn pop_clip(&mut self) {}
    fn push_translation(&mut self, _: Offset) {}
    fn pop_translation(&mut self) {}
    fn push_transform(&mut self, _: &Matrix) {}
    fn pop_transform(&mut self) {}
    fn push_color_filter(&mut self, _: &ColorFilter) {}
    fn pop_color_filter(&mut self) {}
    fn draw_vertices(&mut self, _: &Vertices, _: Offset, _: BlendMode, _: &Brush, _: Rect, _: f32) {}
    fn draw_path(&mut self, _: &Path, _: Offset, _: &Brush, _: Rect, _: &PathStroke) {}
}

fn drawn_texts(rule: &KompozTestRule) -> Vec<String> {
    let mut canvas = Texts::default();
    rule.host().draw(&mut canvas);
    canvas.texts
}

// ---------------------------------------------------------------------------------------------------------------
// Ported from C++

fn dialog_draws_on_top_takes_input_and_dismisses() {
    // C++'s dialog_draws_on_top_takes_input_and_dismisses: a dialog of the test Text for its AlertDialog.
    let open = mutable_state_of(false);
    let log = new_log();
    let (o, l) = (open.clone(), log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, l) = (o.clone(), l.clone());
        Column(
            content = move || {
                Tappable(l.clone(), "below", Modifier.test_tag("below"), || Text("Alttaki"));
                if o.get() {
                    let (o2, l2, l3) = (o.clone(), l.clone(), l.clone());
                    Dialog(
                        on_dismiss_request = move || {
                            push(&l2, "dismiss");
                            o2.set(false);
                        },
                        content = move || {
                            let l3 = l3.clone();
                            Column(
                                content = move || {
                                    Text("Başlık");
                                    Text("Gövde metni");
                                    Box(Modifier.height(60.dp())); // (clear of the minimum touch target of the button)
                                    Tappable(l3.clone(), "ok", Modifier, || Text("Tamam"));
                                },
                            );
                        },
                    );
                }
            },
        );
    }));
    open.set(true);
    rule.wait_for_idle();
    let texts = drawn_texts(&rule);
    assert!(texts.len() == 4 && texts[0] == "Alttaki" && texts[1] == "Başlık", "{texts:?}"); // the dialog after the window
    assert_eq!(rule.host().overlay_count(), 1);
    // Centered: its content (as wide as its longest line) in the window less the platform's margins.
    let title = rule.on_node_with_text("Başlık").get_unclipped_bounds_in_root();
    let body = rule.on_node_with_text("Gövde metni").get_unclipped_bounds_in_root();
    assert!(
        title.left == body.left && (body.left - (400.0 - body.width()) / 2.0).abs() <= 0.5 && title.top > 300.0,
        "{title:?} {body:?}"
    );
    rule.on_node_with_text("Gövde metni").perform_click(); // on the dialog: nothing happens
    assert!(taken(&log).is_empty());
    tap(&rule, bounds(&rule, "below").center()); // outside its content: it closes, nothing below is tapped
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["dismiss"]);
    rule.on_node_with_text("Başlık").assert_does_not_exist();
    assert_eq!(rule.host().overlay_count(), 0);
    open.set(true);
    rule.wait_for_idle();
    rule.on_node_with_text("Tamam").perform_click();
    assert_eq!(taken(&log), ["ok"]);
    assert!(escape(&rule)); // back closes it
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["dismiss"]);
    assert!(!open.peek());
    assert!(!escape(&rule)); // nothing left to go back from
}

fn dropdown_menu_opens_below_its_anchor_and_closes_outside() {
    // C++'s dropdown_menu_opens_below_its_anchor_and_closes_outside: a focusable popup under its anchor for its
    // DropdownMenu (no menu margins), tap detectors for its buttons and items.
    let expanded = mutable_state_of(false);
    let log = new_log();
    let (e, l) = (expanded.clone(), log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (e, l) = (e.clone(), l.clone());
        Column(
            Modifier.padding(20.dp()),
            content = move || {
                let (e1, l1) = (e.clone(), l.clone());
                Box(
                    Modifier.test_tag("anchor"),
                    content = move || {
                        let e2 = e1.clone();
                        Box(
                            Modifier.pointer_input(0, move |s: PointerInputScope| {
                                let e2 = e2.clone();
                                async move { s.detect_tap_gestures().on_tap(move |_| e2.set(true)).await }
                            }),
                            content = || Text("Menü"),
                        );
                        if e1.get() {
                            let (e3, e4, l2) = (e1.clone(), e1.clone(), l1.clone());
                            Popup(
                                popup_position_provider = Some(Rc::new(Below) as Rc<dyn PopupPositionProvider>),
                                on_dismiss_request = move || e3.set(false),
                                properties = PopupProperties { focusable: true, ..PopupProperties::default() },
                                content = move || {
                                    let (e4, l2) = (e4.clone(), l2.clone());
                                    Column(
                                        content = move || {
                                            let (e5, l3) = (e4.clone(), l2.clone());
                                            Box(
                                                Modifier.pointer_input(1, move |s: PointerInputScope| {
                                                    let (e5, l3) = (e5.clone(), l3.clone());
                                                    async move {
                                                        s.detect_tap_gestures()
                                                            .on_tap(move |_| {
                                                                push(&l3, "copy");
                                                                e5.set(false);
                                                            })
                                                            .await
                                                    }
                                                }),
                                                content = || Text("Kopyala"),
                                            );
                                            Tappable(l2.clone(), "paste", Modifier, || Text("Yapıştır"));
                                        },
                                    );
                                },
                            );
                        }
                    },
                );
                Tappable(
                    l.clone(),
                    "other",
                    Modifier.padding_sides_with(0.dp(), 300.dp(), 0.dp(), 0.dp()).test_tag("other"),
                    || Text("Diğer"),
                );
            },
        );
    }));
    rule.on_node_with_text("Kopyala").assert_does_not_exist();
    rule.on_node_with_text("Menü").perform_click();
    let anchor = bounds(&rule, "anchor");
    let copy = rule.on_node_with_text("Kopyala").get_unclipped_bounds_in_root();
    assert!(copy.left == anchor.left && copy.top >= anchor.bottom, "{anchor:?} {copy:?}");
    rule.on_node_with(is_popup(), false).assert_exists();
    tap(&rule, rule.on_node_with_text("Diğer").get_unclipped_bounds_in_root().center()); // outside: closes, nothing else
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());
    rule.on_node_with_text("Diğer").perform_click(); // (it does once the menu is closed)
    assert_eq!(taken(&log), ["other"]);
    rule.on_node_with_text("Menü").perform_click();
    tap(&rule, Offset::new(380.0, 780.0)); // empty space closes it too
    rule.wait_for_idle();
    assert!(taken(&log).is_empty() && !expanded.peek());
    rule.on_node_with_text("Kopyala").assert_does_not_exist();
    rule.on_node_with_text("Menü").perform_click();
    rule.on_node_with_text("Kopyala").perform_click();
    assert_eq!(taken(&log), ["copy"]);
    rule.on_node_with_text("Kopyala").assert_does_not_exist();
}

fn back_handlers_most_recent_enabled_wins() {
    // C++'s back_handlers_most_recent_enabled_wins.
    let log = new_log();
    let inner = mutable_state_of(true);
    let (l, i) = (log.clone(), inner.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l1, l2, i) = (l.clone(), l.clone(), i.clone());
        BackHandler(true, move || push(&l1, "outer"));
        Column(
            content = move || {
                let l2 = l2.clone();
                BackHandler(i.get(), move || push(&l2, "inner"));
            },
        );
    }));
    assert!(escape(&rule));
    inner.set(false);
    rule.wait_for_idle();
    assert!(escape(&rule));
    assert_eq!(taken(&log), ["inner", "outer"]);
    assert!(rule.host().back_invoked()); // the platform's back too
    assert_eq!(taken(&log), ["outer"]);
}

fn back(progress: f32, edge: SwipeEdge) -> BackEventCompat {
    BackEventCompat { touch_x: 0.0, touch_y: 0.0, progress, swipe_edge: edge }
}

fn predictive_back_handler_follows_the_gesture() {
    // C++'s predictive_back_handler_follows_the_gesture.
    let log = new_log();
    let enabled = mutable_state_of(true);
    let (l, en) = (log.clone(), enabled.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l1, l2, en) = (l.clone(), l.clone(), en.clone());
        BackHandler(true, move || push(&l1, "outer"));
        PredictiveBackHandler(en.get(), move |progress: Flow<BackEventCompat>| {
            let log = l2.clone();
            async move {
                struct Called(Log);
                impl Drop for Called {
                    fn drop(&mut self) {
                        push(&self.0, "end"); // runs when the gesture is called off too
                    }
                }
                let _called = Called(log.clone());
                let l = log.clone();
                progress
                    .collect(move |e| {
                        let edge = if e.swipe_edge == SwipeEdge::Right { "R" } else { "L" };
                        push(&l, format!("{:.1}{edge}", e.progress));
                    })
                    .await;
                delay_ms(100).await; // closing, say
                push(&log, "done");
            }
        });
    }));
    let host = rule.host();
    assert!(host.wants_back());
    // A gesture: its steps (not the start) go to the flow; done, the code after it runs.
    host.back_started(BackEventCompat { touch_x: 10.0, touch_y: 300.0, progress: 0.0, swipe_edge: SwipeEdge::Right });
    host.back_progressed(BackEventCompat {
        touch_x: 20.0,
        touch_y: 300.0,
        progress: 0.2,
        swipe_edge: SwipeEdge::Right,
    });
    rule.wait_for_idle();
    host.back_progressed(BackEventCompat {
        touch_x: 60.0,
        touch_y: 300.0,
        progress: 0.6,
        swipe_edge: SwipeEdge::Right,
    });
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["0.2R", "0.6R"]);
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["0.2R", "0.6R", "done", "end"]);
    // A press: the flow ends at once.
    assert!(escape(&rule));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["done", "end"]);
    // Called off: the coroutine is cancelled, its guard runs, the rest doesn't.
    host.back_started(BackEventCompat::default());
    host.back_progressed(back(0.3, SwipeEdge::Left));
    rule.wait_for_idle();
    host.back_cancelled();
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["0.3L", "end"]);
    // A gesture while the run before is still closing: that one is cancelled.
    assert!(escape(&rule));
    host.back_started(BackEventCompat::default());
    host.back_progressed(back(0.4, SwipeEdge::Left));
    rule.wait_for_idle();
    host.back_cancelled();
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["end", "0.4L", "end"]);
    // Disabled during a gesture: it is called off, and back goes to the handler under it.
    host.back_started(BackEventCompat::default());
    rule.wait_for_idle();
    enabled.set(false);
    rule.wait_for_idle();
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["end", "outer"]);
    // A plain BackHandler takes the gesture only once it is done.
    host.back_started(BackEventCompat::default());
    host.back_progressed(back(0.5, SwipeEdge::Left));
    host.back_cancelled();
    assert!(taken(&log).is_empty());
    host.back_started(BackEventCompat::default());
    assert!(host.back_invoked());
    assert_eq!(taken(&log), ["outer"]);
}

fn back_gestures_wait_for_popups() {
    // C++'s back_gestures_wait_for_popups_and_the_focus with a focusable popup (C++'s OverlaySpec default); the
    // focus doesn't take back first (Compose's: back goes to the dispatcher; C++ cleared the focus).
    let log = new_log();
    let open = mutable_state_of(false);
    let handler = mutable_state_of(true);
    let field = FocusRequester::new();
    let (l, o, h, f) = (log.clone(), open.clone(), handler.clone(), field.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, o, f) = (l.clone(), o.clone(), f.clone());
        if h.get() {
            let l1 = l.clone();
            PredictiveBackHandler(true, move |progress: Flow<BackEventCompat>| {
                let log = l1.clone();
                async move {
                    struct Called(Log);
                    impl Drop for Called {
                        fn drop(&mut self) {
                            push(&self.0, "end");
                        }
                    }
                    let _called = Called(log.clone());
                    let l = log.clone();
                    progress.collect(move |_| push(&l, "step")).await;
                    push(&log, "back");
                }
            });
        }
        Box(Modifier.size(40.dp()).focus_requester(&f).focus_target().test_tag("field"));
        if o.get() {
            let (o2, l2) = (o.clone(), l.clone());
            Popup(
                on_dismiss_request = move || {
                    push(&l2, "dismiss");
                    o2.set(false);
                },
                properties = PopupProperties { focusable: true, ..PopupProperties::default() },
                content = || Text("menü"),
            );
        }
    }));
    let host = rule.host();
    open.set(true);
    rule.wait_for_idle();
    // A popup on top: it closes once the gesture is done; the handler hears nothing of it.
    host.back_started(BackEventCompat::default());
    host.back_progressed(back(0.5, SwipeEdge::Left));
    rule.wait_for_idle();
    assert!(log.borrow().is_empty() && host.wants_back());
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["dismiss"]);
    // With the focus somewhere: the handler takes the gesture (Compose's; the focus stays).
    field.request_focus();
    rule.wait_for_idle();
    host.back_started(BackEventCompat::default());
    host.back_progressed(back(0.5, SwipeEdge::Left));
    rule.wait_for_idle();
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["step", "back", "end"]);
    assert!(host.focused_node().is_some());
    // A popup that opens during a gesture: the next gesture is the popup's (the first one is called off).
    host.back_started(BackEventCompat::default());
    host.back_progressed(back(0.5, SwipeEdge::Left));
    rule.wait_for_idle();
    open.set(true);
    rule.wait_for_idle();
    host.back_started(BackEventCompat::default());
    rule.wait_for_idle();
    assert!(host.back_invoked());
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["step", "end", "dismiss"]);
    // Leaving the composition during a gesture calls it off; nothing then wants back, and the gesture's end goes
    // on to the platform (which leaves the app).
    host.back_started(BackEventCompat::default());
    host.back_progressed(back(0.5, SwipeEdge::Left));
    rule.wait_for_idle();
    handler.set(false);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["step", "end"]);
    assert!(!host.wants_back() && !host.back_invoked());
    // A popup alone: back is wanted.
    open.set(true);
    rule.wait_for_idle();
    assert!(host.wants_back());
}

fn back_stops_at_a_dialog_that_cannot_be_dismissed() {
    // C++'s back_stops_at_a_dialog_that_cannot_be_dismissed.
    let backs = Rc::new(std::cell::Cell::new(0));
    let b = backs.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let b = b.clone();
        BackHandler(true, move || b.set(b.get() + 1));
        Dialog(
            on_dismiss_request = || {},
            properties = DialogProperties { dismiss_on_back_press: false, ..DialogProperties::default() },
            content = || Text("Kalıcı"),
        );
    }));
    assert!(escape(&rule));
    rule.wait_for_idle();
    assert_eq!(backs.get(), 0);
    rule.on_node_with_text("Kalıcı").assert_exists();
}

fn a_dialog_without_the_platform_width_gives_its_content_the_window() {
    // C++'s a_full_screen_popup_gives_its_content_the_window as Compose's Dialog without the platform's width:
    // its content gets the window (Compose's: within it, centered; a content filling it fills the window).
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Dialog(
                on_dismiss_request = || {},
                properties = DialogProperties { use_platform_default_width: false, ..DialogProperties::default() },
                content = || {
                    Box(
                        Modifier.fill_max_size().test_tag("icerik"),
                        content = || Box(Modifier.size_wh(60.dp(), 40.dp()).test_tag("küçük")),
                    )
                },
            );
        }),
        IntSize::new(400, 800),
    );
    assert_eq!(bounds(&rule, "icerik"), Rect::new(0.0, 0.0, 400.0, 800.0));
    assert_eq!(bounds(&rule, "küçük"), Rect::new(0.0, 0.0, 60.0, 40.0));
}

fn popups_come_and_go_with_their_anchors() {
    // C++'s nodes_holding_popups_are_counted: a popup shown is an overlay of the host; hidden, or its node gone,
    // it no longer is.
    let shown = mutable_state_of(true);
    let s = shown.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            Box(content = move || {
                if s.get() {
                    Popup(content = || Box(Modifier.size(10.dp()).test_tag("p")));
                }
            });
        }),
        IntSize::new(200, 200),
    );
    assert_eq!(rule.host().overlay_count(), 1);
    rule.on_node_with_tag("p").assert_exists();
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(rule.host().overlay_count(), 0);
    rule.on_node_with_tag("p").assert_does_not_exist();
    shown.set(true);
    rule.wait_for_idle();
    assert_eq!(rule.host().overlay_count(), 1);
    assert!(rule.on_node_with_tag("p").layout_node().window_root().anchor().is_some());
}

fn back_handlers_in_a_popup_come_before_it_closes() {
    // C++'s back_handlers_in_a_popup_come_before_it_closes.
    let log = new_log();
    let open = mutable_state_of(true);
    let inner = mutable_state_of(true);
    let (l, o, i) = (log.clone(), open.clone(), inner.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, o, i) = (l.clone(), o.clone(), i.clone());
        let l1 = l.clone();
        BackHandler(true, move || push(&l1, "pencere"));
        if o.get() {
            let (o2, l2, l3, i2) = (o.clone(), l.clone(), l.clone(), i.clone());
            Dialog(
                on_dismiss_request = move || {
                    push(&l2, "kapat");
                    o2.set(false);
                },
                content = move || {
                    let l3 = l3.clone();
                    BackHandler(i2.get(), move || push(&l3, "içeride"));
                    Text("diyalog");
                },
            );
        }
        let l4 = l.clone();
        BackHandler(true, move || push(&l4, "sonra")); // composed later, but the window's: not while the dialog is on top
    }));
    let host = rule.host();
    assert!(escape(&rule));
    host.back_started(BackEventCompat::default());
    assert!(host.back_invoked()); // a gesture's end too
    inner.set(false);
    rule.wait_for_idle();
    assert!(escape(&rule)); // none in it: the dialog closes
    rule.wait_for_idle();
    assert!(escape(&rule));
    assert_eq!(taken(&log), ["içeride", "içeride", "kapat", "sonra"]);
}

// ---------------------------------------------------------------------------------------------------------------
// Compose's behaviour

fn popups_are_placed_by_their_providers_and_follow_their_anchors() {
    let x = mutable_state_of(10);
    let rtl = mutable_state_of(false);
    let (xx, r) = (x.clone(), rtl.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (xx, r) = (xx.clone(), r.clone());
        let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        let values = [kompoz_ui::platform::LocalLayoutDirection.provides(direction)];
        KompozitionLocalProvider(&values, move || {
            let xx = xx.clone();
            Box(
                Modifier.fill_max_size(),
                content = move || {
                    Box(
                        Modifier.offset_with(xx.get().dp(), 20.dp()).size_wh(100.dp(), 40.dp()).test_tag("çapa"),
                        content = || {
                            // At the anchor's bottom end, its own bottom end there, moved 5 towards the end and 3 down.
                            Popup(
                                alignment = Alignment::BottomEnd,
                                offset = IntOffset::new(5, 3),
                                content = || Box(Modifier.size_wh(30.dp(), 10.dp()).test_tag("pop")),
                            );
                            Popup(
                                alignment = Alignment::TopStart,
                                content = || Box(Modifier.size_wh(500.dp(), 10.dp()).test_tag("geniş")),
                            );
                        },
                    );
                },
            );
        });
    }));
    // Compose's AlignmentOffsetPositionProvider: the anchor's BottomEnd point, less the popup's own BottomEnd; the
    // anchor's bounds its content's (where its offset put it: Compose's parent layout coordinates).
    assert_eq!(bounds(&rule, "pop"), Rect::new(10.0 + 100.0 - 30.0 + 5.0, 20.0 + 40.0 - 10.0 + 3.0, 115.0, 63.0));
    // Kept in the window (clipping enabled): moved back to its left edge... and as wide as the window lets it be.
    assert_eq!(bounds(&rule, "geniş").left, 0.0);
    assert_eq!(bounds(&rule, "geniş").width(), 400.0);
    x.set(50); // its anchor moves: it follows
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "pop").left, 50.0 + 100.0 - 30.0 + 5.0);
    rtl.set(true); // right to left: the end is the left, the offset goes left
    rule.wait_for_idle();
    let anchor = bounds(&rule, "çapa");
    assert_eq!(bounds(&rule, "pop").left, anchor.left - 5.0, "{anchor:?}");
}

fn input_outside_a_non_focusable_popup_passes_through_and_dismisses_it() {
    let log = new_log();
    let open = mutable_state_of(true);
    let (l, o) = (log.clone(), open.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, o) = (l.clone(), o.clone());
        Column(
            content = move || {
                let (l1, o1) = (l.clone(), o.clone());
                Tappable(l.clone(), "alttaki", Modifier.size(100.dp()).test_tag("alttaki"), move || {
                    if o1.get() {
                        let (o2, l2, l3) = (o1.clone(), l1.clone(), l1.clone());
                        Popup(
                            alignment = Alignment::TopStart,
                            offset = IntOffset::new(0, 200),
                            on_dismiss_request = move || {
                                push(&l2, "kapan");
                                o2.set(false);
                            },
                            content =
                                move || Tappable(l3.clone(), "popup", Modifier.size(50.dp()).test_tag("popup"), || {}),
                        );
                    }
                });
            },
        );
    }));
    // On the popup: it takes it (hit first).
    rule.on_node_with_tag("popup").perform_click();
    assert_eq!(taken(&log), ["popup"]);
    // Outside it, on what is below: that is tapped, and the popup is dismissed (Compose's non-focusable popup).
    tap(&rule, Offset::new(50.0, 50.0));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["kapan", "alttaki"]);
    rule.on_node_with_tag("popup").assert_does_not_exist();
    // A non-focusable popup takes neither keys nor back.
    open.set(true);
    rule.wait_for_idle();
    assert!(!escape(&rule));
    assert!(!rule.host().wants_back());
    // A mouse press outside dismisses it too.
    rule.host().mouse_button(Offset::new(300.0, 700.0), -1, PointerButton::Primary, true);
    rule.host().mouse_button(Offset::new(300.0, 700.0), -1, PointerButton::Primary, false);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["kapan"]);
}

fn a_focusable_popup_scopes_the_focus_keys_and_accessibility() {
    let log = new_log();
    let open = mutable_state_of(false);
    let (l, o) = (log.clone(), open.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, o) = (l.clone(), o.clone());
        Column(
            content = move || {
                Box(Modifier.size(20.dp()).focus_target().test_tag("ana1"));
                Box(Modifier.size(20.dp()).focus_target().test_tag("ana2"));
                if o.get() {
                    let l = l.clone();
                    Popup(
                        properties = PopupProperties { focusable: true, ..PopupProperties::default() },
                        content = move || {
                            let l = l.clone();
                            Column(
                                Modifier.on_key_event(move |e| {
                                    if e.r#type == KeyEventType::KeyDown {
                                        push(&l, format!("popup {}", e.key.native_key_code()));
                                    }
                                    false
                                }),
                                content = || {
                                    Box(Modifier.size(20.dp()).focus_target().test_tag("p1"));
                                    Box(Modifier.size(20.dp()).focus_target().test_tag("p2"));
                                    Text("menü");
                                },
                            );
                        },
                    );
                }
            },
        );
    }));
    let at = |tag: &str| {
        let node = rule.on_node_with_tag(tag).layout_node();
        rule.host().focused_node().is_some_and(|f| Rc::ptr_eq(&f, &node))
    };
    rule.host().key_down(Key::Tab, KeyModifiers::None);
    assert!(at("ana1"));
    open.set(true);
    rule.wait_for_idle();
    // Tab goes in the popup's window, round and round in it; the main window's focus moves into it.
    for expected in ["p1", "p2", "p1"] {
        rule.host().key_down(Key::Tab, KeyModifiers::None);
        assert!(at(expected), "{expected}");
    }
    // Its key nodes see the keys once the focus is in it (before, they go to its root's).
    assert_eq!(taken(&log).len(), 2);
    // Accessibility sees the popup's window only.
    let tree = rule.host().accessibility_tree();
    assert!(tree.iter().any(|n| n.label == "menü") && !tree.iter().any(|n| n.test_tag.as_deref() == Some("ana1")));
    // Two roots now: the host's window's and the popup's.
    rule.on_all_nodes(is_root()).assert_count_equals(2);
    open.set(false); // closed with the focus in it: the focus is cleared
    rule.wait_for_idle();
    assert!(rule.host().focused_node().is_none());
    rule.host().key_down(Key::Tab, KeyModifiers::None);
    assert!(at("ana1"));
}

fn a_dialog_dismissed_only_as_it_says() {
    let log = new_log();
    let (l1, l2) = (log.clone(), log.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l1, l2) = (l1.clone(), l2.clone());
        Tappable(l1.clone(), "altta", Modifier.fill_max_size(), || {});
        Dialog(
            on_dismiss_request = move || push(&l2, "kapat"),
            properties = DialogProperties { dismiss_on_click_outside: false, ..DialogProperties::default() },
            content = || Box(Modifier.size(50.dp()).test_tag("içerik")),
        );
    }));
    tap(&rule, Offset::new(10.0, 10.0)); // outside its content: not dismissed, nor through
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());
    rule.on_node_with_tag("içerik").assert_is_displayed();
    rule.on_node_with(is_dialog(), false).assert_exists();
    assert!(escape(&rule)); // back still does
    assert_eq!(taken(&log), ["kapat"]);
}

fn popups_nest_above_the_one_they_were_opened_from() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            Modifier.size(50.dp()),
            content = || {
                Popup(
                    content = || {
                        Box(
                            Modifier.size(80.dp()).test_tag("ilk"),
                            content = || {
                                Popup(
                                    offset = IntOffset::new(10, 10),
                                    content = || Box(Modifier.size(20.dp()).test_tag("iç")),
                                );
                            },
                        );
                    },
                );
            },
        );
    }));
    let roots = rule.host().overlays.roots();
    assert_eq!(roots.len(), 2);
    // The inner one above (drawn after, hit first); its anchor the outer's content, its place from there.
    assert!(roots[1].anchor().is_some_and(|a| a.window_root().is_popup_root()));
    assert_eq!(bounds(&rule, "iç").top_left(), Offset::new(10.0, 10.0));
    rule.on_all_nodes(is_popup()).assert_count_equals(2);
}

// ---------------------------------------------------------------------------------------------------------------
// The host

fn frame_times_fall_on_the_displays_beat() {
    // C++'s frame_times_fall_on_the_displays_beat.
    use kompoz_ui::host::frame_time_on_beat;
    assert_eq!(frame_time_on_beat(None, 1000, 100), 1000); // the first: now
    assert_eq!(frame_time_on_beat(Some(1000), 1090, 100), 1100); // started a little early in its period
    assert_eq!(frame_time_on_beat(Some(1100), 1149, 100), 1100); // within half a period: the same frame time
    assert_eq!(frame_time_on_beat(Some(1100), 1240, 100), 1200); // late in its period
    assert_eq!(frame_time_on_beat(Some(1200), 1460, 100), 1500); // two refreshes missed: three periods on
    assert_eq!(frame_time_on_beat(Some(1500), 1300, 100), 1500); // (a clock behind it: never back)
    assert_eq!(frame_time_on_beat(Some(1500), 1600, 0), 1600); // no period known: now
}

fn run_on_ui_thread_from_background() {
    // C++'s run_on_ui_thread_from_background, through the host's handle.
    let value = mutable_state_of(0);
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || Text(format!("v={}", v.get()))));
    let handle = rule.host().handle();
    let worker = std::thread::spawn(move || handle.post(move || value.set(42)));
    worker.join().unwrap();
    rule.wait_for_idle();
    rule.on_node_with_text("v=42").assert_exists();
}

tests!(
    dialog_draws_on_top_takes_input_and_dismisses,
    dropdown_menu_opens_below_its_anchor_and_closes_outside,
    back_handlers_most_recent_enabled_wins,
    predictive_back_handler_follows_the_gesture,
    back_gestures_wait_for_popups,
    back_stops_at_a_dialog_that_cannot_be_dismissed,
    a_dialog_without_the_platform_width_gives_its_content_the_window,
    popups_come_and_go_with_their_anchors,
    back_handlers_in_a_popup_come_before_it_closes,
    popups_are_placed_by_their_providers_and_follow_their_anchors,
    input_outside_a_non_focusable_popup_passes_through_and_dismisses_it,
    a_focusable_popup_scopes_the_focus_keys_and_accessibility,
    a_dialog_dismissed_only_as_it_says,
    popups_nest_above_the_one_they_were_opened_from,
    frame_times_fall_on_the_displays_beat,
    run_on_ui_thread_from_background,
);

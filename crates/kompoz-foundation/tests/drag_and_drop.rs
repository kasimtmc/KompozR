//! foundation.draganddrop: `drag_and_drop_source` and `drag_and_drop_target` with the host's drags within the app
//! (the pointer taken, the decoration drawn under it, the drop, Escape), drops from outside, and a drag handed to
//! the platform as it leaves the window (the C++ Kompoz's tests; Compose's behaviour where they differ). Numbers in
//! px (density 1).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_coroutines::CoroutineScope;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::clipboard::ClipEntry;
use kompoz_ui::draganddrop::PlatformDragAndDrop;
use kompoz_ui::draw::DrawScope;
use kompoz_ui::input::*;
use kompoz_ui::platform::PlatformCallbacks;
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

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

/// A target logging what it is told: `drop <text or file> at x,y` (px in the window), and its ending's result.
struct Logging {
    log: Log,
    on_drop: Box<dyn Fn(&DragAndDropEvent) -> bool>,
}

impl DragAndDropTarget for Logging {
    fn on_drop(&self, event: &DragAndDropEvent) -> bool {
        (self.on_drop)(event)
    }
    fn on_started(&self, _: &DragAndDropEvent) {
        push(&self.log, "started");
    }
    fn on_entered(&self, _: &DragAndDropEvent) {
        push(&self.log, "entered");
    }
    fn on_exited(&self, _: &DragAndDropEvent) {
        push(&self.log, "exited");
    }
    fn on_ended(&self, event: &DragAndDropEvent) {
        push(&self.log, if event.result { "ended taken" } else { "ended" });
    }
}

/// A target that only says it never starts.
struct Never(Log);

impl DragAndDropTarget for Never {
    fn on_drop(&self, _: &DragAndDropEvent) -> bool {
        false
    }
    fn on_started(&self, _: &DragAndDropEvent) {
        push(&self.0, "never");
    }
}

fn fruit_data(name: &str) -> DragAndDropTransferData {
    DragAndDropTransferData {
        clip_entry: ClipEntry::with_plain_text(name),
        local_state: Some(Rc::new(name.to_string())),
        ..Default::default()
    }
}

/// Moves the pointer `id` (a finger's, or the mouse's) from `from` to `to` in 8 frames.
fn move_to(rule: &KompozTestRule, from: Offset, to: Offset, id: i64) {
    for i in 1..=8 {
        rule.advance_frame();
        rule.host().pointer_move(from + (to - from) * (i as f32 / 8.0), -1, id);
    }
}

fn drag_and_drop_carries_data_from_a_source_to_a_target() {
    // C++ 13414, as Compose does it: the event's position is the window's (Compose's `positionInRoot`: C++ gave the
    // target's); no haptic feedback as a drag starts (Compose's source doesn't ask for one); a clickable inside a
    // source takes a finger's press for itself (it consumes the down a long press would start from: Compose's
    // detectors want it unconsumed), so its taps click and the mouse drags it.
    let log = new_log();
    let fruits = mutable_state_of(vec!["elma".to_string(), "armut".to_string()]);
    let basket = mutable_state_of(Vec::<String>::new());
    let (l, f, b) = (log.clone(), fruits.clone(), basket.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (l, f, b) = (l.clone(), f.clone(), b.clone());
            Column(move || {
                let (l, f, b) = (l.clone(), f.clone(), b.clone());
                let names = f.get();
                Row(move || {
                    for name in names.clone() {
                        let n = name.clone();
                        Box(
                            modifier =
                                Modifier.size_wh(80.dp(), 40.dp()).drag_and_drop_source(move |_| Some(fruit_data(&n))),
                            content = move || Text(name.clone()),
                        );
                    }
                });
                let lc = l.clone();
                Box(modifier = Modifier
                    .size_wh(80.dp(), 40.dp())
                    .drag_and_drop_source(|_| Some(fruit_data("tık")))
                    .clickable(move || push(&lc, "click"))
                    .test_tag("tıkla"));
                Box(modifier = Modifier.fill_max_width().height(60.dp()));
                let should = l.clone();
                let target: Rc<dyn DragAndDropTarget> = remember(|| {
                    let (l, f, b) = (l.clone(), f.clone(), b.clone());
                    Rc::new(Logging {
                        log: l.clone(),
                        on_drop: Box::new(move |e: &DragAndDropEvent| {
                            let name = e.local_state::<String>().cloned().unwrap_or_default();
                            let p = e.position_in_root;
                            push(&l, format!("drop {name} at {},{}", p.x as i32, p.y as i32));
                            if name == "tık" {
                                return true;
                            }
                            let mut left = f.peek();
                            left.retain(|n| *n != e.text());
                            f.set(left);
                            let mut inn = b.peek();
                            inn.push(e.text().to_string());
                            b.set(inn);
                            true
                        }),
                    }) as Rc<dyn DragAndDropTarget>
                });
                Box(modifier = Modifier.fill_max_width().height(150.dp()).drag_and_drop_target(
                    move |e: &DragAndDropEvent| {
                        push(&should, format!("should {}", e.text()));
                        e.has_mime_type("text/plain")
                    },
                    target,
                ));
                let never: Rc<dyn DragAndDropTarget> =
                    remember(|| Rc::new(Never(l.clone())) as Rc<dyn DragAndDropTarget>);
                Box(modifier = Modifier
                    .fill_max_width()
                    .height(100.dp())
                    .drag_and_drop_target(|_: &DragAndDropEvent| false, never));
            });
        }),
        IntSize::new(400, 800),
    );
    let h = rule.host();
    // (the fruits 0..40 down, the clickable source 40..80, the basket 140..290)
    // A tap on the source with a clickable inside clicks.
    h.pointer_down(at(40.0, 60.0), -1, 0, PointerType::Touch);
    h.pointer_up(at(40.0, 60.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["click"]);
    // A finger holds the apple, then takes it to the basket.
    h.pointer_down(at(40.0, 20.0), -1, 0, PointerType::Touch);
    rule.advance_time_by(500);
    assert_eq!(taken(&log), ["should elma", "started"]);
    assert!(h.is_dragging_and_dropping());
    move_to(&rule, at(40.0, 20.0), at(200.0, 215.0), 0);
    assert_eq!(taken(&log), ["entered"]);
    let canvas = rule.draw();
    // (the apple, and its faded copy under the finger)
    assert_eq!(canvas.texts.iter().filter(|t| t.as_str() == "elma").count(), 2, "{:?}", canvas.texts);
    assert!(canvas.layer_alphas.contains(&0.75), "{:?}", canvas.layer_alphas);
    h.pointer_up(at(200.0, 215.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["drop elma at 200,215", "ended taken"]);
    assert!(basket.peek() == ["elma"] && fruits.peek() == ["armut"] && !h.is_dragging_and_dropping());
    // Taken out again and let go elsewhere: no drop.
    h.pointer_down(at(40.0, 20.0), -1, 0, PointerType::Touch); // (the pear is first now)
    rule.advance_time_by(500);
    move_to(&rule, at(40.0, 20.0), at(200.0, 215.0), 0);
    move_to(&rule, at(200.0, 215.0), at(200.0, 100.0), 0);
    h.pointer_up(at(200.0, 100.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["should armut", "started", "entered", "exited", "ended"]);
    // The mouse just drags (past the touch slop); Escape calls it off.
    h.pointer_down(at(40.0, 20.0), -1, 0, PointerType::Mouse);
    move_to(&rule, at(40.0, 20.0), at(200.0, 215.0), 0);
    assert_eq!(taken(&log), ["should armut", "started", "entered"]);
    h.key_down(Key::Escape, KeyModifiers::None);
    h.pointer_up(at(200.0, 215.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["exited", "ended"]);
    assert_eq!(fruits.peek().len(), 1);
    // The mouse drags the source with a clickable inside, dropped on the basket: no click.
    h.pointer_down(at(40.0, 60.0), -1, 0, PointerType::Mouse);
    move_to(&rule, at(40.0, 60.0), at(200.0, 215.0), 0);
    h.pointer_up(at(200.0, 215.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["should tık", "started", "entered", "drop tık at 200,215", "ended taken"]);
    // A finger that moves off before holding long enough doesn't drag.
    h.pointer_down(at(40.0, 20.0), -1, 0, PointerType::Touch);
    move_to(&rule, at(40.0, 20.0), at(40.0, 120.0), 0);
    rule.advance_time_by(500);
    h.pointer_up(at(40.0, 120.0), -1, 0);
    rule.wait_for_idle();
    assert!(taken(&log).is_empty());
}

fn drops_from_outside_the_app_reach_the_targets() {
    // C++ 13510 (the host's path since stage 8, here through foundation's modifier): told what it may be as it
    // comes, what it is at the drop; a target for drags within the app only (they carry a local state) takes no
    // part.
    let log = new_log();
    let l = log.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let l = l.clone();
            Column(move || {
                let should = l.clone();
                let target: Rc<dyn DragAndDropTarget> = remember(|| {
                    let l = l.clone();
                    Rc::new(Logging {
                        log: l.clone(),
                        on_drop: Box::new(move |e: &DragAndDropEvent| {
                            let data = e.transfer_data.clone().unwrap_or_default();
                            let what = data.files().first().cloned().unwrap_or(data.text());
                            let p = e.position_in_root;
                            push(&l, format!("drop {what} at {},{}", p.x as i32, p.y as i32));
                            !data.files().is_empty()
                        }),
                    }) as Rc<dyn DragAndDropTarget>
                });
                Box(modifier = Modifier.fill_max_width().height(200.dp()).drag_and_drop_target(
                    move |e: &DragAndDropEvent| {
                        push(&should, format!("should {}", e.has_mime_type("text/uri-list")));
                        e.has_mime_type("text/uri-list")
                    },
                    target,
                ));
                let never: Rc<dyn DragAndDropTarget> =
                    remember(|| Rc::new(Never(l.clone())) as Rc<dyn DragAndDropTarget>);
                Box(modifier = Modifier
                    .fill_max_width()
                    .height(200.dp())
                    .drag_and_drop_target(|e: &DragAndDropEvent| e.local_state::<String>().is_some(), never));
            });
        }),
        IntSize::new(400, 800),
    );
    let h = rule.host();
    h.external_drag_entered(vec!["text/uri-list".into(), "text/plain".into()]);
    h.external_drag_moved(at(100.0, 300.0));
    h.external_drag_moved(at(100.0, 50.0));
    let data = DragAndDropTransferData {
        clip_entry: ClipEntry::with_files(vec!["/tmp/a b.png".into()]),
        ..Default::default()
    };
    assert!(h.external_drop(data, at(100.0, 60.0)));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["should true", "started", "entered", "drop /tmp/a b.png at 100,60", "ended taken"]);
    // Brought over and taken away again.
    h.external_drag_entered(vec!["text/uri-list".into(), "text/plain".into()]);
    h.external_drag_moved(at(100.0, 50.0));
    h.external_drag_left();
    assert_eq!(taken(&log), ["should true", "started", "entered", "exited", "ended"]);
    // Text dropped where nothing said where it was: the target asks again, by what it is.
    let text = DragAndDropTransferData { clip_entry: ClipEntry::with_plain_text("merhaba"), ..Default::default() };
    assert!(!h.external_drop(text, at(100.0, 50.0)));
    assert_eq!(taken(&log), ["should false"]);
}

fn a_dragged_item_of_a_scrolled_list_is_drawn_from_where_it_is() {
    // C++ 22221: an item nested in a list that scrolls, dragged while the list scrolls under it (from code): what
    // is kept of the frames stays right (the keep check), and the item is drawn where it is and as the decoration
    // under the finger (Compose's shadow: its content recorded as it drew).
    let scroll = ScrollState::new(0);
    let green = Color::from_argb(0xFF00A000);
    let sc = scroll.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            Column(
                modifier = Modifier.fill_max_size().vertical_scroll(&sc),
                content = move || {
                    Box(modifier = Modifier.height(200.dp()));
                    Box(
                        modifier = Modifier.padding(2.dp()),
                        content = move || {
                            Box(
                                modifier = Modifier.padding(2.dp()),
                                content = move || {
                                    // (the source before the background: its shadow is what is drawn after it, Compose's)
                                    Box(modifier = Modifier
                                        .size_wh(80.dp(), 40.dp())
                                        .drag_and_drop_source(|_| Some(fruit_data("elma")))
                                        .background(green));
                                },
                            );
                        },
                    );
                    Box(modifier = Modifier.height(2000.dp()));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.set_keep_check(true);
    let scroll_to = |value: i32| {
        let s = scroll.clone();
        CoroutineScope::new(rule.host().dispatcher()).launch(async move {
            s.scroll_to(value).await;
        });
    };
    scroll_to(50);
    rule.wait_for_idle();
    let h = rule.host();
    h.pointer_down(at(40.0, 174.0), -1, 0, PointerType::Touch); // (204 - 50 + 20)
    rule.advance_time_by(500);
    assert!(h.is_dragging_and_dropping());
    for i in 1..=8 {
        rule.advance_frame();
        h.pointer_move(at(40.0 + 25.0 * i as f32, 174.0 + 25.0 * i as f32), -1, 0);
    }
    scroll_to(80); // (and the list scrolls under it meanwhile)
    rule.advance_frame();
    rule.advance_frame();
    assert!(h.take_keep_check_mismatches().is_empty());
    let canvas = rule.draw();
    let greens = canvas.rect_colors.iter().filter(|c| **c == green).count();
    assert_eq!(greens, 2, "the item where it is and its copy under the finger");
    h.pointer_up(at(240.0, 374.0), -1, 0);
    rule.wait_for_idle();
    assert!(!h.is_dragging_and_dropping());
}

/// A platform taking the app's drags as they leave the window (WP5b's outgoing drags): what it was given.
#[derive(Default)]
struct Outgoing {
    drags: RefCell<Vec<(String, Size, Offset)>>,
    decorations: RefCell<Vec<RecordingCanvas>>,
}

impl PlatformCallbacks for Outgoing {
    fn start_drag_and_drop(&self, drag: &PlatformDragAndDrop) -> bool {
        self.drags.borrow_mut().push((drag.transfer_data.text(), drag.decoration_size, drag.decoration_offset));
        let mut canvas = RecordingCanvas::new();
        drag.draw(&mut canvas);
        self.decorations.borrow_mut().push(canvas);
        true
    }
}

fn a_drag_leaving_the_window_goes_to_the_platform() {
    // The hook WP5b's outgoing drags plug into (Compose's drags are the platform's: AWT's or Android's): a drag
    // within the app that leaves the window is handed to the platform with what it carries and its decoration (its
    // targets told it left and ended); back over the window it carries its local state again; its end tells the
    // source the action it was taken with (the desktop's onTransferCompleted). Within the app, a drop tells it too:
    // Move, Ctrl Copy (AWT's user action). A source drawing its own decoration.
    let log = new_log();
    let completed = Rc::new(RefCell::new(Vec::<Option<DragAndDropTransferAction>>::new()));
    let (l, c) = (log.clone(), completed.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (l, c) = (l.clone(), c.clone());
            Column(move || {
                let c = c.clone();
                Box(modifier = Modifier.size_wh(80.dp(), 40.dp()).drag_and_drop_source_with_decoration(
                    |scope: &mut DrawScope<'_>| scope.draw_rect(Color::Red, Offset::Zero, Size::new(20.0, 10.0)),
                    move |_| {
                        let c = c.clone();
                        Some(DragAndDropTransferData {
                            on_transfer_completed: Some(Rc::new(move |a: Option<DragAndDropTransferAction>| {
                                c.borrow_mut().push(a)
                            })
                                as Rc<dyn Fn(Option<DragAndDropTransferAction>)>),
                            ..fruit_data("elma")
                        })
                    },
                ));
                let l2 = l.clone();
                let target: Rc<dyn DragAndDropTarget> = remember(|| {
                    let l = l.clone();
                    Rc::new(Logging {
                        log: l.clone(),
                        on_drop: Box::new(move |e: &DragAndDropEvent| {
                            push(&l, format!("drop {:?} {:?}", e.local_state::<String>(), e.action));
                            true
                        }),
                    }) as Rc<dyn DragAndDropTarget>
                });
                Box(modifier = Modifier.fill_max_width().height(200.dp()).drag_and_drop_target(
                    move |_: &DragAndDropEvent| {
                        push(&l2, "should");
                        true
                    },
                    target,
                ));
            });
        }),
        IntSize::new(400, 800),
    );
    let h = rule.host();
    // (the mouse comes over it first, an event of its own: Compose for desktop's matcher-taking detectors launch their
    // coroutines (plain `launch`, dispatched) from the first event their pointer input sees, so a press that is that
    // first event comes before they wait for it — as it would in Compose)
    h.pointer_move(at(40.0, 20.0), -1, 0);
    rule.main_clock().run_current();
    // Within the app, Ctrl held: copied.
    h.pointer_down(at(40.0, 20.0), -1, 0, PointerType::Mouse);
    move_to(&rule, at(40.0, 20.0), at(100.0, 100.0), 0);
    h.key_down(Key::CtrlLeft, KeyModifiers::Ctrl);
    h.pointer_up(at(100.0, 100.0), -1, 0);
    h.key_up(Key::CtrlLeft, KeyModifiers::None);
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["should", "started", "entered", "drop Some(\"elma\") Some(Copy)", "ended taken"]);
    assert_eq!(*completed.borrow(), [Some(DragAndDropTransferAction::Copy)]);
    // Without a platform that takes it, out of the window it stays the app's.
    h.pointer_down(at(40.0, 20.0), -1, 0, PointerType::Mouse);
    move_to(&rule, at(40.0, 20.0), at(100.0, 900.0), 0);
    assert!(h.is_dragging_and_dropping());
    h.key_down(Key::Escape, KeyModifiers::None);
    rule.wait_for_idle();
    completed.borrow_mut().clear();
    taken(&log);
    // With one, it is handed over as it leaves.
    let platform = Rc::new(Outgoing::default());
    h.set_callbacks(platform.clone());
    h.pointer_down(at(40.0, 20.0), -1, 0, PointerType::Mouse);
    move_to(&rule, at(40.0, 20.0), at(100.0, 100.0), 0);
    move_to(&rule, at(100.0, 100.0), at(100.0, 900.0), 0);
    assert!(!h.is_dragging_and_dropping());
    assert_eq!(*platform.drags.borrow(), [("elma".to_string(), Size::new(80.0, 40.0), at(40.0, 20.0))]);
    assert_eq!(platform.decorations.borrow()[0].rect_colors, [Color::Red]);
    assert_eq!(taken(&log), ["should", "started", "entered", "exited", "ended"]);
    // The platform brings it back over the window and it is dropped there: with its local state.
    h.external_drag_entered(vec!["text/plain".into()]);
    h.external_drag_moved(at(100.0, 100.0));
    let platform_data =
        DragAndDropTransferData { clip_entry: ClipEntry::with_plain_text("elma"), ..Default::default() };
    assert!(h.external_drop(platform_data, at(100.0, 100.0)));
    h.platform_drag_ended(Some(DragAndDropTransferAction::Move));
    rule.wait_for_idle();
    assert_eq!(taken(&log), ["should", "started", "entered", "drop Some(\"elma\") None", "ended taken"]);
    assert_eq!(*completed.borrow(), [Some(DragAndDropTransferAction::Move)]);
}

tests!(
    drag_and_drop_carries_data_from_a_source_to_a_target,
    drops_from_outside_the_app_reach_the_targets,
    a_dragged_item_of_a_scrolled_list_is_drawn_from_where_it_is,
    a_drag_leaving_the_window_goes_to_the_platform,
);

//! Gestures in Kotlin's syntax, as a Compose user writes them (kompozable_tests.cpp 2525-2551's Gestures, whole: its
//! `enum class DragValue`, `draggable` by name, `anchoredDraggable` of a state with `DraggableAnchors { … at … }`,
//! `pointerInput { detectTapGestures(onTap = { }) }`, a drag's points in a state list, `dragAndDropTarget` by name of
//! an `object : DragAndDropTarget`; `remember { AnchoredDraggableState(…) }` for its
//! `rememberAnchoredDraggableState`, which Compose 1.12 hasn't), through `kompoz!`; and in a `.kt` file (kompoz_file!)
//! 2976-3020's `draggable2D`/`scrollable2D` of their remembered states, a `transformable` with `canPan`, and a
//! `dragAndDropSource` carrying text. The same behaviour crates/kompoz-foundation/tests/gestures2d.rs and
//! drag_and_drop.rs check in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::clipboard::ClipEntry;
use kompoz::ui::draganddrop::{DragAndDropEvent, DragAndDropTarget};
use kompoz::ui::input::PointerType;
use std::cell::RefCell;
use std::rc::Rc;

/// A finger from `from` to `to`, a move a frame, held `hold` frames, let go.
fn drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32, hold: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / frames as f32), -1, 0);
    }
    for _ in 0..hold {
        rule.advance_frame();
    }
    h.pointer_up(to, -1, 0);
    rule.wait_for_idle();
}

/// The text of the node whose text has `part`.
fn text_with(rule: &KompozTestRule, part: &str) -> String {
    let node = named_args!(rule.on_node_with_text(part, substring = true)).fetch_semantics_node();
    let texts = node.config().get(kompoz::ui::semantics::SemanticsProperties::Text);
    texts.and_then(|t| t.first().map(|t| t.text().to_string())).unwrap_or_default()
}

/// A target noting the texts dropped on it (kt/gestures.kt's `Fruit` takes it from Rust).
struct Dropped(Rc<RefCell<Vec<String>>>);

impl DragAndDropTarget for Dropped {
    fn on_drop(&self, event: &DragAndDropEvent) -> bool {
        self.0.borrow_mut().push(event.text().to_string());
        true
    }
}

kompoz! {
    enum class DragValue { Start, End }

    @Composable
    fun Gestures(onEvent: (String) -> Unit) {
        var x by remember { mutableStateOf(0f) }
        val state = remember { AnchoredDraggableState(DragValue.Start) }
        state.updateAnchors(DraggableAnchors {
            DragValue.Start at 0f
            DragValue.End at 120f
        })
        val points = remember { mutableStateListOf<Offset>() }
        Column {
            Box(modifier = Modifier.size(56.dp).testTag("tutamaç")
                    .draggable(orientation = Orientation.Horizontal, state = rememberDraggableState { delta -> x = maxOf(0f, x + delta) }))
            Box(modifier = Modifier.size(56.dp).anchoredDraggable(state, Orientation.Horizontal).testTag("geçiş"))
            Box(modifier = Modifier.size(100.dp).testTag("dokun").pointerInput(Unit) {
                detectTapGestures(onTap = { onEvent("tap") }, onLongPress = { onEvent("long") })
            })
            Box(modifier = Modifier.size(100.dp).testTag("çiz").pointerInput(Unit) {
                detectDragGestures(onDragStart = { at -> points.clear(); points.add(at) }, onDrag = { change, _ -> points.add(change.position) })
            })
            Text("x ${x.roundToInt()} noktalar ${points.size} konum ${state.requireOffset().roundToInt()}")
            Box(modifier = Modifier.size(40.dp).dragAndDropTarget(shouldStartDragAndDrop = { it.hasMimeType("text/plain") },
                                                                  target = remember {
                                                                      object : DragAndDropTarget {
                                                                          override fun onDrop(event: DragAndDropEvent): Boolean {
                                                                              onEvent(event.text())
                                                                              return true
                                                                          }
                                                                      }
                                                                  }))
        }
    }
}

fn gestures_as_compose_writes_them() {
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let seen = log.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let seen = seen.clone();
        Gestures(move |event: String| seen.borrow_mut().push(event));
    }));
    rule.on_node_with_text("x 0 noktalar 0 konum 0").assert_exists();
    drag(&rule, Offset::new(10.0, 28.0), Offset::new(40.0, 28.0), 3, 2);
    let slop = kompoz::ui::platform::ViewConfiguration::default().touch_slop;
    let x = (30.0 - slop).round() as i32;
    rule.on_node_with_text(&format!("x {x} noktalar 0 konum 0")).assert_exists();
    // The anchored one: dragged most of the way, settled on the end.
    drag(&rule, Offset::new(10.0, 84.0), Offset::new(110.0, 84.0), 5, 10);
    rule.on_node_with_text(&format!("x {x} noktalar 0 konum 120")).assert_exists();
    // A tap, then a long press.
    rule.on_node_with_tag("dokun").perform_touch_input(|t| t.click());
    rule.wait_for_idle();
    rule.advance_time_by(400); // (onTap waits for no double tap when there is no onDoubleTap: at once)
    assert_eq!(*log.borrow(), ["tap"]);
    rule.on_node_with_tag("dokun").perform_touch_input(|t| t.long_click());
    rule.wait_for_idle();
    assert_eq!(*log.borrow(), ["tap", "long"]);
    // A drag's points: its start and its moves.
    rule.on_node_with_tag("çiz").perform_touch_input(|t| t.swipe(Offset(10.0, 50.0), Offset(90.0, 50.0)));
    rule.wait_for_idle();
    let shown = text_with(&rule, "noktalar");
    let n: i32 = shown.split(' ').nth(3).and_then(|n| n.parse().ok()).unwrap_or(0);
    assert!(n >= 2, "the drag's start and moves: {shown}");
    // A drop from outside on the target (below the text), by its MIME type.
    let below = named_args!(rule.on_node_with_text("noktalar", substring = true))
        .fetch_semantics_node()
        .bounds_in_root()
        .bottom;
    let h = rule.host();
    assert!(h.external_drag_entered(vec!["text/plain".into()]));
    let data = DragAndDropTransferData { clip_entry: ClipEntry::with_plain_text("elma"), ..Default::default() };
    assert!(h.external_drop(data, Offset::new(20.0, below + 20.0)));
    assert_eq!(*log.borrow(), ["tap", "long", "elma"]);
}

kompoz_file!("tests/kt/gestures.kt");

fn draggable2d_and_scrollable2d_from_a_kt_file() {
    let rule = KompozTestRule::new(Pulled);
    // Caught at once: a move under the touch slop counts.
    drag(&rule, Offset::new(50.0, 50.0), Offset::new(53.0, 52.0), 1, 2);
    rule.on_node_with_text("çekilen 3 2 kaydırılan 0 0").assert_exists();
    // The 2D scrollable takes what goes left or down of each delta.
    drag(&rule, Offset::new(80.0, 120.0), Offset::new(20.0, 180.0), 6, 6);
    let texts = text_with(&rule, "kaydırılan");
    let slop = kompoz::ui::platform::ViewConfiguration::default().touch_slop;
    let keep = (60.0f32 * 2.0f32.sqrt() - slop) / (60.0 * 2.0f32.sqrt());
    let expected = format!("kaydırılan {} {}", (-60.0 * keep).round() as i32, (60.0 * keep).round() as i32);
    assert!(texts.ends_with(&expected), "{texts} {expected}");
}

fn a_transformable_from_a_kt_file() {
    let rule = KompozTestRule::new(Zoom);
    // One finger, not zoomed: no pan.
    drag(&rule, Offset::new(150.0, 200.0), Offset::new(150.0, 100.0), 10, 10);
    rule.on_node_with_text("ölçek 10 kayma 0").assert_exists();
    // Two fingers spreading: zoomed.
    let h = rule.host();
    h.pointer_down(Offset::new(100.0, 150.0), -1, 1, PointerType::Touch);
    h.pointer_down(Offset::new(200.0, 150.0), -1, 2, PointerType::Touch);
    for i in 1..=8 {
        rule.advance_frame();
        h.pointer_move(Offset::new(100.0 - 5.0 * i as f32, 150.0), -1, 1);
        h.pointer_move(Offset::new(200.0 + 5.0 * i as f32, 150.0), -1, 2);
    }
    h.pointer_up(Offset::new(60.0, 150.0), -1, 1);
    h.pointer_up(Offset::new(240.0, 150.0), -1, 2);
    rule.wait_for_idle();
    // (spread from 100 to 180 px apart: what is past the touch slop zooms)
    let shown = text_with(&rule, "ölçek");
    let scale: i32 = shown.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    assert!(scale > 13 && shown.ends_with("kayma 0"), "{shown}");
    // Zoomed: one finger pans.
    drag(&rule, Offset::new(150.0, 200.0), Offset::new(150.0, 100.0), 10, 10);
    let shown = text_with(&rule, "kayma");
    assert!(!shown.ends_with("kayma 0"), "{shown}");
}

fn a_drag_and_drop_source_from_a_kt_file() {
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let target: Rc<dyn DragAndDropTarget> = Rc::new(Dropped(dropped.clone()));
    let rule = KompozTestRule::new(kompozable_lambda!(move || Fruit("armut".to_string(), target.clone())));
    // The mouse drags it past the slop onto the target.
    let h = rule.host();
    // (the mouse comes over it first, an event of its own: Compose for desktop's matcher-taking detectors launch their
    // coroutines (plain `launch`, dispatched) from the first event their pointer input sees, so a press that is that
    // first event comes before they wait for it — as it would in Compose)
    h.pointer_move(Offset::new(40.0, 20.0), -1, 0);
    rule.main_clock().run_current();
    h.pointer_down(Offset::new(40.0, 20.0), -1, 0, PointerType::Mouse);
    for i in 1..=8 {
        rule.advance_frame();
        h.pointer_move(Offset::new(40.0 + 10.0 * i as f32, 20.0 + 15.0 * i as f32), -1, 0);
    }
    h.pointer_up(Offset::new(120.0, 140.0), -1, 0);
    rule.wait_for_idle();
    assert_eq!(*dropped.borrow(), ["armut"]);
}

tests![
    gestures_as_compose_writes_them,
    draggable2d_and_scrollable2d_from_a_kt_file,
    a_transformable_from_a_kt_file,
    a_drag_and_drop_source_from_a_kt_file,
];

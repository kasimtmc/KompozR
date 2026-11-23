//! Lazy lists in Kotlin's syntax, as a Compose user writes them: `LazyColumn { item { … }; items(n) { i -> … } }`,
//! `items(list, key = { it.id })`, `itemsIndexed`, `stickyHeader`, a list over a `mutableStateListOf`, and
//! `rememberLazyListState()` scrolled by `animateScrollToItem` in a coroutine (kompozable_tests.cpp 630, 682, 2581,
//! 2773), through `kompoz!`; and in a `.kt` file (kompoz_file!) a snapping LazyRow, items that animate and fill a
//! fraction of the list, a state read through `layoutInfo`. The same lists crates/kompoz-foundation/tests/lazy.rs
//! checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::input::PointerType;

/// A node's bounds in the root, unclipped (px; the rule's density is 1).
fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A mail of the inbox.
#[derive(Clone, KotlinProperties)]
pub struct Mail {
    pub from: String,
}

/// A row of a keyed list (Kotlin's `data class Row2(val id: Int, val label: String)`).
#[derive(Clone, PartialEq, Debug, KotlinProperties)]
pub struct Row2 {
    pub id: i32,
    pub label: String,
}

/// `Row2(id, label)`.
pub fn Row2(id: i32, label: impl Into<String>) -> Row2 {
    Row2 { id, label: label.into() }
}

/// A thing to do (Kotlin's `data class Todo(val id: Int, val title: String, val done: Boolean)`).
#[derive(Clone, PartialEq, Debug, KotlinProperties)]
pub struct Todo {
    pub id: i32,
    pub title: String,
    pub done: bool,
}

/// `Todo(id, title, done)`.
pub fn Todo(id: i32, title: impl Into<String>, done: bool) -> Todo {
    Todo { id, title: title.into(), done }
}

kompoz! {
    // kompozable_tests.cpp 630: a header item and the mails with their indices.
    @Kompozable
    fun Inbox(mails: List<Mail>) {
        LazyColumn(modifier = Modifier.height(300.dp)) {
            item { Text("Gelen kutusu") }
            itemsIndexed(mails) { index, mail -> Text("$index: ${mail.from}") }
        }
    }

    @Kompozable
    fun KeyedRow(row: Row2) {
        var taps by remember { mutableStateOf(0) }
        Text("${row.label}:$taps", modifier = Modifier.clickable { taps++ })
    }

    // kompozable_tests.cpp 682: an item's state follows its key when the list changes.
    @Kompozable
    fun KeyedList(rows: List<Row2>) {
        LazyColumn(modifier = Modifier.height(400.dp)) {
            item(key = "başlık") { Text("Satırlar") }
            items(rows, key = { it.id }) { row -> KeyedRow(row) }
            item { Text(if (rows.size > 1) "çok" else "${Int.MAX_VALUE > 0}") }
        }
    }

    // kompozable_tests.cpp 2581: a list of things to do over a state list, toggled by a click.
    @Kompozable
    fun TodoScreen() {
        val todos = remember { mutableStateListOf(Todo(1, "Yaz", false), Todo(2, "Oku", false)) }
        LazyColumn {
            items(todos, key = { it.id }) { t ->
                Text(
                    if (t.done) "✓ ${t.title}" else t.title,
                    modifier = Modifier.clickable {
                        val at = todos.indexOfFirst { it.id == t.id }
                        todos[at] = Todo(t.id, t.title, !t.done)
                    },
                )
            }
        }
    }

    // kompozable_tests.cpp 2773: a lazy list over a state list, shuffled and refilled.
    @Kompozable
    fun Tasks() {
        val tasks = remember { mutableStateListOf(1, 2, 3) }
        Column {
            Text("Ters", modifier = Modifier.clickable { tasks.reverse() })
            Text("Doldur", modifier = Modifier.clickable { tasks.clear(); tasks.addAll(listOf(4, 5)) })
            LazyColumn(modifier = Modifier.height(200.dp)) {
                items(tasks, key = { it }) { Text("görev $it") }
            }
            Text("sıra: ${tasks.joinToString(",")}")
        }
    }

    // Sections under sticky headers, a remembered state scrolled by animateScrollToItem from a coroutine.
    @Kompozable
    fun Sections() {
        val state = rememberLazyListState()
        val scope = rememberCoroutineScope()
        Column {
            Text("ilk ${state.firstVisibleItemIndex}", modifier = Modifier.clickable {
                scope.launch { state.animateScrollToItem(30) }
            })
            LazyColumn(state = state, modifier = Modifier.height(300.dp).testTag("bölümler")) {
                for (section in 0 until 4) {
                    stickyHeader(key = "h$section") { Text("Bölüm $section", modifier = Modifier.height(40.dp).testTag("h$section")) }
                    items(10) { i -> Text("$section.$i", modifier = Modifier.height(50.dp)) }
                }
            }
        }
    }
}

fn a_lazy_column_of_an_item_and_indexed_mails() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Inbox(vec![Mail { from: "ayşe".into() }, Mail { from: "ali".into() }])
    }));
    rule.on_node_with_text("Gelen kutusu").assert_exists();
    rule.on_node_with_text("0: ayşe").assert_exists();
    rule.on_node_with_text("1: ali").assert_exists();
}

fn keys_keep_an_items_state() {
    let rows = mutable_state_of(vec![Row2(1, "a"), Row2(2, "b")]);
    let r = rows.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || KeyedList(r.value())));
    rule.on_node_with_text("Satırlar").assert_exists();
    rule.on_node_with_text("çok").assert_exists();
    rule.on_node_with_text("b:0").perform_click();
    rule.on_node_with_text("b:1").assert_exists();
    rows.set(vec![Row2(0, "z"), Row2(1, "a"), Row2(2, "b")]);
    rule.wait_for_idle();
    rule.on_node_with_text("b:1").assert_exists();
    rule.on_node_with_text("z:0").assert_exists();
    rows.set(vec![Row2(1, "a")]);
    rule.wait_for_idle();
    rule.on_node_with_text("true").assert_exists();
}

fn a_todo_list_over_a_state_list() {
    let rule = KompozTestRule::new(TodoScreen);
    rule.on_node_with_text("Yaz").perform_click();
    rule.on_node_with_text("✓ Yaz").assert_exists();
    rule.on_node_with_text("Oku").assert_exists();
}

fn a_lazy_list_over_a_state_list_follows_it() {
    let rule = KompozTestRule::new(Tasks);
    rule.on_node_with_text("görev 2").assert_exists();
    rule.on_node_with_text("Ters").perform_click();
    rule.on_node_with_text("sıra: 3,2,1").assert_exists();
    let top = |text: &str| rule.on_node_with_text(text).get_unclipped_bounds_in_root().top;
    assert!(top("görev 3") < top("görev 2") && top("görev 2") < top("görev 1"));
    rule.on_node_with_text("Doldur").perform_click();
    rule.on_node_with_text("görev 4").assert_exists();
    rule.on_node_with_text("görev 5").assert_exists();
    rule.on_all_nodes_with_text("görev 2").assert_count_equals(0);
}

fn sticky_headers_and_an_animated_scroll() {
    let rule = KompozTestRule::with_viewport(Sections, IntSize::new(400, 800));
    rule.on_node_with_text("ilk 0").assert_exists();
    rule.on_node_with_text("ilk 0").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("ilk 30").assert_exists();
    // (item 30: section 2's header is 22, its items 23..32; the header sticks at the top)
    let list = bounds(&rule, "bölümler");
    assert!((bounds(&rule, "h2").top - list.top).abs() < 0.5, "{:?} {:?}", bounds(&rule, "h2"), list);
}

// ---- kompoz_file! -------------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/lazy.kt");

/// A quick finger from `from` to `to`, a move a frame, let go (a fling).
fn swipe(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / frames as f32), -1, 0);
    }
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

/// The drawn text starting with `prefix`.
fn drawn(rule: &KompozTestRule, prefix: &str) -> String {
    let texts = rule.drawn_texts();
    texts.iter().find(|t| t.starts_with(prefix)).cloned().unwrap_or_else(|| panic!("no `{prefix}…` in {texts:?}"))
}

fn a_snapping_carousel_from_a_kt_file() {
    let rule = KompozTestRule::with_viewport(Carousel, IntSize::new(400, 400));
    assert_eq!(drawn(&rule, "konum "), "konum 0:0");
    let c = bounds(&rule, "carousel");
    let y = c.top + 50.0;
    // A fling to the left: it settles with an item in the middle (300 px wide, items of 100: the one after the first
    // in view, which is then fully in it).
    swipe(&rule, Offset::new(c.left + 280.0, y), Offset::new(c.left + 160.0, y), 4);
    rule.wait_for_idle();
    let at = drawn(&rule, "konum ");
    let (index, offset) = at["konum ".len()..].split_once(':').expect("index:offset");
    let index: i32 = index.parse().unwrap();
    assert!(index >= 1 && offset == "0", "{at}");
    let middle = bounds(&rule, &format!("kart {}", index + 1));
    assert!((middle.left - c.left - 100.0).abs() < 0.5, "{middle:?}");
    // A slow drag by 70 px back, held: the drag alone, then the snap to the item nearest the middle (the first in
    // view before, 30 px short of it).
    let h = rule.host();
    h.pointer_down(Offset::new(c.left + 100.0, y), -1, 0, PointerType::Touch);
    for i in 1..=7 {
        rule.advance_frame();
        h.pointer_move(Offset::new(c.left + 100.0 + 10.0 * i as f32, y), -1, 0);
    }
    rule.advance_time_by(200);
    h.pointer_up(Offset::new(c.left + 170.0, y), -1, 0);
    rule.wait_for_idle();
    assert_eq!(drawn(&rule, "konum "), format!("konum {}:0", index - 1));
}

fn items_filling_half_the_list_glide_when_reordered_from_a_kt_file() {
    let order = mutable_state_of(vec![1, 2, 3]);
    let o = order.clone();
    let rule = KompozTestRule::with_viewport(kompozable_lambda!(move || Shelf(o.value())), IntSize::new(400, 800));
    let first = bounds(&rule, "raf 1");
    assert!((first.width() - 100.0).abs() < 0.5 && first.top.abs() < 0.5, "{first:?}");
    assert!((bounds(&rule, "raf 3").top - 100.0).abs() < 0.5);
    // (the clock by hand: the check on the way sees the items gliding)
    rule.main_clock().set_auto_advance(false);
    order.set(vec![3, 1, 2]);
    rule.advance_time_by(100);
    let mid = bounds(&rule, "raf 3").top;
    assert!(mid > 10.0 && mid < 90.0, "{mid}");
    common::settle(&rule);
    assert!(bounds(&rule, "raf 3").top.abs() < 0.5);
    assert!((bounds(&rule, "raf 2").top - 100.0).abs() < 0.5);
}

fn layout_info_in_a_derived_state_and_a_scroll_from_a_launched_effect_from_a_kt_file() {
    let target = mutable_state_of(0);
    let t = target.clone();
    let rule = KompozTestRule::with_viewport(kompozable_lambda!(move || Jump(t.value())), IntSize::new(400, 800));
    rule.on_node_with_text("son 3").assert_exists();
    target.set(50);
    rule.wait_for_idle();
    rule.on_node_with_text("son 53").assert_exists();
    rule.on_node_with_text("satır 50").assert_exists();
    rule.on_all_nodes_with_text("satır 49").assert_count_equals(0);
}

tests!(
    a_lazy_column_of_an_item_and_indexed_mails,
    keys_keep_an_items_state,
    a_todo_list_over_a_state_list,
    a_lazy_list_over_a_state_list_follows_it,
    sticky_headers_and_an_animated_scroll,
    a_snapping_carousel_from_a_kt_file,
    items_filling_half_the_list_glide_when_reordered_from_a_kt_file,
    layout_info_in_a_derived_state_and_a_scroll_from_a_launched_effect_from_a_kt_file,
);

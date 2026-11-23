//! Material's top app bars as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's tests by name
//! (kompoz_tests.cpp; M15's values and API where the C++ differed, said at each), Compose's behaviour (the state, its
//! fractions and saver; the scroll behaviors' connections, their remembering, the lazy-list and scroll-state forms,
//! the deprecated reversed one; the bar's drags and settling; the colors and their copy; the overrides; the
//! defaults), and the top-bar samples of M15's AppBarSamples.kt, each checked as used (SimpleTopAppBarWithAdaptiveActions
//! waits for WP8's currentWindowAdaptiveInfo). Density 1: dp are px; the viewport is 400 × 800 unless said.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::Easing;
use kompoz_foundation::gestures::ScrollableState;
use kompoz_foundation::layout::{PaddingValues, WindowInsets};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material_icons_extended::Icons as ExtendedIcons;
use kompoz_material3::prelude::*;
use kompoz_material3_adaptive::{WindowSizeClass, current_window_adaptive_info};
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas, has_text};
use kompoz_text::{FontWeight, TextOverflow, TextStyle};
use kompoz_ui::input::{NestedScrollSource, PointerType};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::tooling::preview::LoremIpsum;
use kompoz_ui::unit::Velocity;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    // (the C++ Kompoz's)
    top_app_bar_enter_always_hides_before_the_list_scrolls,
    a_collapsing_top_bar_moves_the_content_without_recomposing_it,
    large_top_app_bar_exit_until_collapsed,
    two_rows_top_app_bars_let_their_titles_style_themselves,
    pinned_top_app_bar_knows_when_content_is_under_it,
    a_top_app_bar_takes_its_scrolled_color,
    flexible_top_app_bars_are_bigger_and_place_their_title_across,
    top_app_bars_show_a_subtitle_under_the_title,
    top_app_bar_colors_heights_and_insets_can_be_set,
    center_aligned_top_app_bar_mirrors_right_to_left,
    // (Compose's behaviour)
    top_app_bar_state_keeps_its_offsets_in_their_limits_and_is_saved,
    the_scroll_behaviors_move_their_state_by_what_the_content_scrolls,
    behaviors_of_a_list_or_a_scroll_know_whether_it_is_at_its_start,
    the_deprecated_reversed_enter_always_leaves_a_reversed_content_its_scrolls,
    dragging_a_top_app_bar_moves_it_and_it_settles,
    a_two_rows_top_app_bar_takes_its_scrolled_color_and_title_as_it_collapses,
    top_app_bar_colors_copy_and_their_deprecated_forms,
    top_app_bars_are_made_by_the_overrides_provided,
    top_app_bar_defaults_as_compose,
    // (M15's AppBarSamples)
    simple_top_app_bar,
    simple_top_app_bar_with_adaptive_actions,
    simple_top_app_bar_with_subtitle,
    simple_center_aligned_top_app_bar,
    simple_center_aligned_top_app_bar_with_subtitle,
    pinned_top_app_bar,
    pinned_top_app_bar_with_pre_scrolled_lazy_column,
    pinned_top_app_bar_with_reversed_lazy_grid,
    enter_always_top_app_bar,
    enter_always_top_app_bar_with_reverse_scrolling,
    exit_until_collapsed_medium_top_app_bar,
    exit_until_collapsed_center_aligned_medium_flexible_top_app_bar,
    exit_until_collapsed_large_top_app_bar,
    exit_until_collapsed_center_aligned_large_flexible_top_app_bar,
    custom_two_rows_top_app_bar,
);

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn near_within(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A text's own bounds (the unmerged tree's).
fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with(has_text(text, false, false), true).get_unclipped_bounds_in_root()
}

/// Where a node's content is in the root (its inner coordinates: C++'s `contentBounds`).
fn content_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    let inner = rule.on_node_with_tag(tag).layout_node().inner_coordinates();
    inner.find_root_coordinates().local_bounding_box_of(&inner, false)
}

type Slot<T> = Rc<RefCell<Option<T>>>;

fn slot<T>() -> Slot<T> {
    Rc::new(RefCell::new(None))
}

fn get<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

fn at(x: f32, y: f32) -> Offset {
    Offset::new(x, y)
}

// A note on the drags below: a bar collapsing under a finger moves what the finger is on (the list goes up with
// it); Compose doesn't dispatch a move whose position in a pointer input node is where the last one was, so the
// drags go by the C++'s steps, which aren't whole pixels of what moves under them.

/// The C++ tests' `slowDrag`: down, moved over `frames` frames, held 200 ms, up.
fn slow_drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        let t = i as f32 / frames as f32;
        h.pointer_move(from + (to - from) * t, -1, 0);
    }
    rule.advance_time_by(200);
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

/// The C++ tests' `rule.swipe(from, to, frames)`: a move each frame, up at once (a fling).
fn swipe(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        let t = i as f32 / frames as f32;
        h.pointer_move(from + (to - from) * t, -1, 0);
    }
    h.pointer_up(to, -1, 0);
    rule.advance_frame();
}

/// The color `text` was drawn in.
fn color_of(canvas: &RecordingCanvas, text: &str) -> Option<Color> {
    canvas.texts.iter().position(|t| t == text).map(|i| canvas.colors[i])
}

/// The largest size `text` was drawn at.
fn size_of(canvas: &RecordingCanvas, text: &str) -> f32 {
    canvas
        .texts
        .iter()
        .zip(&canvas.text_styles)
        .filter(|(t, _)| *t == text)
        .map(|(_, s)| s.font_size)
        .fold(0.0, f32::max)
}

/// The C++ tests' list of 100 rows, 50 dp tall, tagged "row<i>", under `padding`.
#[kompozable]
fn Rows(list: LazyListState, padding: PaddingValues) {
    LazyColumn(
        modifier = Modifier.fill_max_size(),
        state = Some(list),
        content_padding = padding,
        content = |this: &mut LazyListScope| {
            this.items(100, |i| {
                Text(format!("Satır {i}"), modifier = Modifier.height(50.dp()).test_tag(format!("row{i}")))
            });
        },
    );
}

// ---- The C++ Kompoz's tests --------------------------------------------------------------------------------------

/// A small bar under an entering-always behavior collapses before the list scrolls up, comes back before it scrolls
/// down, and settles after a fling.
fn top_app_bar_enter_always_hides_before_the_list_scrolls() {
    let behavior = slot::<Rc<dyn TopAppBarScrollBehavior>>();
    let list = LazyListState::new(0, 0);
    let rule = {
        let (behavior, list) = (behavior.clone(), list.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let scroll = TopAppBarDefaults.enter_always_scroll_behavior();
                *behavior.borrow_mut() = Some(scroll.clone());
                let (bar, list) = (scroll.clone(), list.clone());
                Scaffold(
                    modifier = Modifier.fill_max_size().nested_scroll(scroll.nested_scroll_connection()),
                    top_bar = move || TopAppBar(title = || Text("Başlık"), scroll_behavior = bar.clone()),
                    content = move |padding| Rows(list.clone(), padding),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let state = get(&behavior).state();
    assert!(near(state.height_offset_limit(), -64.0) && near(bounds(&rule, "row0").top, 64.0));
    // 2 dp per move: 40 less the touch slop. (Compose's slop is 8 dp and its drag goes on from it: 32 dp; the C++'s
    // went on 30.)
    slow_drag(&rule, at(200.0, 400.0), at(200.0, 360.0), 20);
    assert!(
        near(state.height_offset(), -32.0)
            && list.first_visible_item_index() == 0
            && list.first_visible_item_scroll_offset() == 0,
        "{state:?}"
    );
    assert!(near(bounds(&rule, "row0").top, 64.0 - 32.0)); // the content moved up with the bar
    // the title in the middle of what shows
    assert!(near_within(text_bounds(&rule, "Başlık").center().y, (64.0 - 32.0) / 2.0, 1.0));
    slow_drag(&rule, at(200.0, 400.0), at(200.0, 200.0), 20);
    assert!(near(state.height_offset(), -64.0) && near(state.collapsed_fraction(), 1.0));
    assert!(list.first_visible_item_index() > 0 || list.first_visible_item_scroll_offset() > 100);
    let index = list.first_visible_item_index();
    let offset = list.first_visible_item_scroll_offset();
    slow_drag(&rule, at(200.0, 300.0), at(200.0, 340.0), 20); // down: the bar comes back first
    assert!(
        near(state.height_offset(), -64.0 + 32.0)
            && list.first_visible_item_index() == index
            && list.first_visible_item_scroll_offset() == offset,
        "{state:?}"
    );
    // Let go half way after a fling: it settles all the way.
    swipe(&rule, at(200.0, 300.0), at(200.0, 320.0), 2);
    rule.wait_for_idle();
    assert!(near(state.height_offset(), 0.0) || near(state.height_offset(), -64.0), "{state:?}");
}

/// As the bar collapses, what lays out by the scaffold's padding (the list's content padding, `Modifier.padding`,
/// consumed insets) follows it, the content not composed again.
fn a_collapsing_top_bar_moves_the_content_without_recomposing_it() {
    let behavior = slot::<Rc<dyn TopAppBarScrollBehavior>>();
    let list = LazyListState::new(0, 0);
    let composed = Rc::new(Cell::new(0));
    let seen = slot::<PaddingValues>();
    let rule = {
        let (behavior, list, composed, seen) = (behavior.clone(), list.clone(), composed.clone(), seen.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let scroll = TopAppBarDefaults.enter_always_scroll_behavior();
                *behavior.borrow_mut() = Some(scroll.clone());
                let (bar, list, composed, seen) = (scroll.clone(), list.clone(), composed.clone(), seen.clone());
                Scaffold(
                    modifier = Modifier.fill_max_size().nested_scroll(scroll.nested_scroll_connection()),
                    top_bar = move || TopAppBar(title = || Text("Başlık"), scroll_behavior = bar.clone()),
                    content = move |padding: PaddingValues| {
                        composed.set(composed.get() + 1);
                        *seen.borrow_mut() = Some(padding.clone());
                        let list = list.clone();
                        Box(
                            modifier = Modifier.fill_max_size(),
                            content = move || {
                                Rows(list.clone(), padding.clone());
                                // What is left of 120 at the top once the bar's padding is kept clear of.
                                Box(modifier = Modifier
                                    .fill_max_width()
                                    .padding_values(padding.clone())
                                    .consume_window_insets_padding_values(padding.clone())
                                    .window_insets_top_height(WindowInsets::new(0, 120, 0, 0))
                                    .test_tag("kenar"));
                            },
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let state = get(&behavior).state();
    let edge = || content_bounds(&rule, "kenar");
    let before = composed.get();
    assert!(near(get(&seen).calculate_top_padding().value(), 64.0) && near(bounds(&rule, "row0").top, 64.0));
    assert!(near(edge().top, 64.0) && near(edge().height(), 120.0 - 64.0));
    // 2 dp per move: 40 less Compose's 8 dp touch slop (the C++'s went on 30).
    slow_drag(&rule, at(200.0, 400.0), at(200.0, 360.0), 20);
    assert!(near(state.height_offset(), -32.0) && near(get(&seen).calculate_top_padding().value(), 64.0 - 32.0));
    assert!(near(bounds(&rule, "row0").top, 64.0 - 32.0)); // the list's content padding
    assert!(near(edge().top, 64.0 - 32.0) && near(edge().height(), 120.0 - 32.0)); // Modifier.padding, consumed insets
    assert_eq!(composed.get(), before); // laid out again, not composed again
}

/// A large bar under an exiting-until-collapsed behavior collapses before the list scrolls up, and expands only once
/// the list is back at its top.
fn large_top_app_bar_exit_until_collapsed() {
    let behavior = slot::<Rc<dyn TopAppBarScrollBehavior>>();
    let list = LazyListState::new(0, 0);
    let rule = {
        let (behavior, list) = (behavior.clone(), list.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let scroll = TopAppBarDefaults.exit_until_collapsed_scroll_behavior();
                *behavior.borrow_mut() = Some(scroll.clone());
                let (bar, list) = (scroll.clone(), list.clone());
                Scaffold(
                    modifier = Modifier.fill_max_size().nested_scroll(scroll.nested_scroll_connection()),
                    top_bar = move || LargeTopAppBar(title = || Text("Büyük başlık"), scroll_behavior = bar.clone()),
                    content = move |padding| Rows(list.clone(), padding),
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let state = get(&behavior).state();
    assert!(near(state.height_offset_limit(), -88.0) && near(bounds(&rule, "row0").top, 152.0));
    // (each drag's fling settles the bar by an animation: the C++'s settled at once; it is let end before the next)
    // up 52 after Compose's 8 dp slop (the C++'s 51): collapses, the list stays
    slow_drag(&rule, at(200.0, 500.0), at(200.0, 440.0), 20);
    assert!(
        near(state.height_offset(), -52.0)
            && list.first_visible_item_scroll_offset() == 0
            && list.first_visible_item_index() == 0,
        "{state:?}"
    );
    rule.wait_for_idle();
    slow_drag(&rule, at(200.0, 500.0), at(200.0, 200.0), 20);
    rule.wait_for_idle();
    assert!(
        near(state.height_offset(), -88.0) && list.first_visible_item_index() >= 4,
        "{state:?} {} {}",
        list.first_visible_item_index(),
        list.first_visible_item_scroll_offset()
    );
    // Down: the list scrolls back first; the bar expands only once the list is at its top.
    slow_drag(&rule, at(200.0, 300.0), at(200.0, 400.0), 20);
    rule.wait_for_idle();
    assert!(near(state.height_offset(), -88.0) && list.first_visible_item_index() >= 2);
    slow_drag(&rule, at(200.0, 200.0), at(200.0, 700.0), 20);
    assert!(
        list.first_visible_item_index() == 0
            && list.first_visible_item_scroll_offset() == 0
            && near(state.height_offset(), 0.0),
        "{state:?}"
    );
}

/// A two-rows bar asks for each row's title and subtitle (expanded or not), which take the style around them (the
/// bar's fonts merge into it: the fake measurer's widths are of the style around, 20 sp: 10 dp a letter).
fn two_rows_top_app_bars_let_their_titles_style_themselves() {
    let asked = Rc::new(RefCell::new(Vec::<bool>::new()));
    let behavior = slot::<Rc<dyn TopAppBarScrollBehavior>>();
    let list = LazyListState::new(0, 0);
    let rule = {
        let (asked, behavior, list) = (asked.clone(), behavior.clone(), list.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let scroll = TopAppBarDefaults.exit_until_collapsed_scroll_behavior();
                *behavior.borrow_mut() = Some(scroll.clone());
                let (asked, bar, list) = (asked.clone(), scroll.clone(), list.clone());
                let around = named_args!(TextStyle(font_size = 20.sp()));
                // the style around the bar: 10 dp a letter
                KompozitionLocalProvider(&[LocalTextStyle.provides(around)], move || {
                    let (asked, bar, list) = (asked.clone(), bar.clone(), list.clone());
                    Scaffold(
                        modifier = Modifier.fill_max_size().nested_scroll(bar.nested_scroll_connection()),
                        top_bar = move || {
                            let asked = asked.clone();
                            TwoRowsTopAppBar(
                                title = move |expanded: bool| {
                                    asked.borrow_mut().push(expanded);
                                    Text(
                                        if expanded { "Büyük başlık" } else { "Küçük" },
                                        modifier = Modifier.test_tag(if expanded { "büyük" } else { "küçük" }),
                                    );
                                },
                                scroll_behavior = bar.clone(),
                                modifier = Modifier.test_tag("çubuk"),
                                subtitle = Some(Rc::new(|expanded: bool| {
                                    Text(
                                        if expanded { "uzun alt başlık" } else { "alt" },
                                        modifier =
                                            Modifier.test_tag(if expanded { "alt büyük" } else { "alt küçük" }),
                                    );
                                }) as Rc<dyn Fn(bool)>),
                            );
                        },
                        content = move |padding| Rows(list.clone(), padding),
                    );
                });
            }),
            IntSize::new(400, 800),
        )
    };
    // (the row hidden from accessibility clears its titles' semantics: they are found in the unmerged tree)
    let at =
        |tag: &str| named_args!(rule.on_node_with_tag(tag, use_unmerged_tree = true)).get_unclipped_bounds_in_root();
    // Asked for both rows, each its own text.
    assert!(asked.borrow().contains(&true) && asked.borrow().contains(&false));
    assert!(
        near(at("çubuk").height(), 112.0 + 24.0)
            && at("büyük").top > 63.0
            && at("küçük").top < 40.0
            && at("alt büyük").top > at("büyük").top
    );
    // (M15 merges the bar's fonts into the style around: the sizes are the bar's — headline medium 28 for the large
    // title, label large 14 (0.1 sp letter spacing) for its subtitle, title large 22 for the small row's title — where
    // the C++ left the style around: the fake glyphs are half an em wide; and each text is as tall as its style's
    // line: 36, 20, 28 and label medium's 16 for the small row's subtitle.)
    assert!(
        near(at("büyük").width(), 12.0 * 14.0)
            && near(at("küçük").width(), 5.0 * 11.0)
            && near_within(at("alt büyük").width(), 15.0 * 7.1, 1.0),
        "{:?} {:?} {:?}",
        at("büyük"),
        at("küçük"),
        at("alt büyük")
    );
    assert!(
        near(at("büyük").height(), 36.0)
            && near(at("alt büyük").height(), 20.0)
            && near(at("küçük").height(), 28.0)
            && near(at("alt küçük").height(), 16.0)
    );
    // It collapses to 64 dp as the list scrolls.
    slow_drag(&rule, Offset::new(200.0, 500.0), Offset::new(200.0, 200.0), 20);
    assert!(near(get(&behavior).state().height_offset(), 64.0 - 136.0) && near(at("çubuk").height(), 64.0));
}

/// A pinned bar stays; it notes that content is under it, and that it isn't once the content is back at its top.
fn pinned_top_app_bar_knows_when_content_is_under_it() {
    let behavior = slot::<Rc<dyn TopAppBarScrollBehavior>>();
    let scroll = ScrollState::new(0);
    let rule = {
        let (behavior, scroll) = (behavior.clone(), scroll.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let pinned = TopAppBarDefaults.pinned_scroll_behavior();
                *behavior.borrow_mut() = Some(pinned.clone());
                let (bar, scroll) = (pinned.clone(), scroll.clone());
                Scaffold(
                    modifier = Modifier.fill_max_size().nested_scroll(pinned.nested_scroll_connection()),
                    top_bar = move || TopAppBar(title = || Text("Başlık"), scroll_behavior = bar.clone()),
                    content = move |padding: PaddingValues| {
                        Column(
                            modifier = Modifier.fill_max_size().vertical_scroll(&scroll).padding_values(padding),
                            content = || Box(modifier = Modifier.height(2000.dp())),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let state = get(&behavior).state();
    slow_drag(&rule, at(200.0, 500.0), at(200.0, 300.0), 20);
    assert!(near(state.height_offset(), 0.0) && state.overlapped_fraction() > 0.5 && scroll.value() > 150);
    slow_drag(&rule, at(200.0, 200.0), at(200.0, 700.0), 20);
    assert!(scroll.value() == 0 && near(state.overlapped_fraction(), 0.0), "{state:?}");
}

/// A pinned bar's container turns to the scrolled color as content comes under it, the bar alone composing again.
fn a_top_app_bar_takes_its_scrolled_color() {
    let behavior = slot::<Rc<dyn TopAppBarScrollBehavior>>();
    let scroll = ScrollState::new(0);
    let cs = light_color_scheme();
    let composed = Rc::new(Cell::new(0));
    let rule = {
        let (behavior, scroll, composed) = (behavior.clone(), scroll.clone(), composed.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                composed.set(composed.get() + 1);
                let pinned = TopAppBarDefaults.pinned_scroll_behavior();
                *behavior.borrow_mut() = Some(pinned.clone());
                let (bar, scroll) = (pinned.clone(), scroll.clone());
                Scaffold(
                    modifier = Modifier.fill_max_size().nested_scroll(pinned.nested_scroll_connection()),
                    top_bar = move || TopAppBar(title = || Text("Başlık"), scroll_behavior = bar.clone()),
                    content = move |padding: PaddingValues| {
                        Column(
                            modifier = Modifier.fill_max_size().vertical_scroll(&scroll).padding_values(padding),
                            content = || Box(modifier = Modifier.height(2000.dp())),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let bar_color = || {
        let c = rule.draw();
        c.rects
            .iter()
            .zip(&c.rect_colors)
            .find(|(r, _)| near(r.left, 0.0) && near(r.top, 0.0) && near(r.width(), 400.0) && near(r.height(), 64.0))
            .map(|(_, c)| *c)
    };
    let before = composed.get();
    assert_eq!(bar_color(), Some(cs.surface));
    slow_drag(&rule, at(200.0, 500.0), at(200.0, 300.0), 20);
    rule.wait_for_idle();
    // Its color went over as content came under it. (The C++ composed its root again once; in Compose the bar's
    // derived target color recomposes the bar alone: the root isn't composed again.)
    assert!(get(&behavior).state().overlapped_fraction() > 0.5 && composed.get() == before);
    assert_eq!(bar_color(), Some(cs.surface_container));
}

/// The flexible bars are taller (more with a subtitle), their titles in larger styles, placed at the start,
/// centered or at the end of the whole bar as their alignment says.
fn flexible_top_app_bars_are_bigger_and_place_their_title_across() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Column(
                content = || {
                    MediumFlexibleTopAppBar(
                        title = || Text("Notlar"),
                        window_insets = WindowInsets::Zero,
                        modifier = Modifier.test_tag("orta-çubuk"),
                    );
                    MediumFlexibleTopAppBar(
                        title = || Text("Notlar"),
                        subtitle = Some(Rc::new(|| Text("12 not")) as Rc<dyn Fn()>),
                        window_insets = WindowInsets::Zero,
                        modifier = Modifier.test_tag("alt-yazılı"),
                    );
                    LargeFlexibleTopAppBar(
                        title = || Text("Gelen"),
                        window_insets = WindowInsets::Zero,
                        title_horizontal_alignment = Alignment::CenterHorizontally,
                        modifier = Modifier.test_tag("büyük-çubuk"),
                    );
                    LargeFlexibleTopAppBar(
                        title = || Text("Giden"),
                        subtitle = Some(Rc::new(|| Text("2 ileti")) as Rc<dyn Fn()>),
                        window_insets = WindowInsets::Zero,
                        title_horizontal_alignment = Alignment::End,
                        modifier = Modifier.test_tag("son-çubuk"),
                    );
                },
            );
        }),
        IntSize::new(400, 1000),
    );
    let at = |tag: &str| bounds(&rule, tag);
    assert!(near(at("orta-çubuk").height(), 112.0) && near(at("alt-yazılı").height(), 136.0));
    assert!(near(at("büyük-çubuk").height(), 120.0) && near(at("son-çubuk").height(), 152.0));
    let canvas = rule.draw();
    // (M15's tokens: the large flexible title is display small, 36 sp; its subtitle title medium, 16 sp.)
    assert!(
        near(size_of(&canvas, "Notlar"), 28.0)
            && near(size_of(&canvas, "12 not"), 14.0)
            && near(size_of(&canvas, "Gelen"), 36.0)
            && near(size_of(&canvas, "2 ileti"), 16.0)
    );
    // The large titles (fake glyphs half an em wide): Start's 16 dp in, centered, and at the end 8 dp from it (M15
    // places an end-aligned title by the whole width less the actions' 4 dp and its own 4 dp padding: the C++ put it
    // 16 dp from the end).
    let large_at = |text: &str, size: f32| {
        canvas
            .texts
            .iter()
            .enumerate()
            .find(|(i, t)| *t == text && near(canvas.text_styles[*i].font_size, size))
            .map_or(-1.0, |(i, _)| canvas.baselines[i].x)
    };
    assert!(
        near(large_at("Notlar", 28.0), 16.0)
            && near(large_at("Gelen", 36.0), 200.0 - 45.0)
            && near(large_at("Giden", 36.0), 400.0 - 8.0 - 90.0),
        "{} {} {}",
        large_at("Notlar", 28.0),
        large_at("Gelen", 36.0),
        large_at("Giden", 36.0)
    );
    // its subtitle at the end too (7 letters of 8.15 — title medium's 0.15 sp letter spacing —, lined up at the end of
    // the column the title makes as wide)
    assert!(near_within(large_at("2 ileti", 16.0), 400.0 - 8.0 - 57.05, 1.0), "{}", large_at("2 ileti", 16.0));
}

/// The subtitle sits under the title in the subtitle's color and style; centered, both are in the middle; a medium
/// bar with a subtitle is taller, its subtitle 24 dp above the bottom.
fn top_app_bars_show_a_subtitle_under_the_title() {
    let cs = light_color_scheme();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Column(
                content = || {
                    TopAppBarWithSubtitle(
                        title = || Text("Gelen kutusu", modifier = Modifier.test_tag("başlık")),
                        subtitle = || Text("3 okunmamış", modifier = Modifier.test_tag("alt")),
                    );
                    // (The C++'s CenterAlignedTopAppBar(title, subtitle): M15's centered bar has no subtitle; its
                    // subtitle form is TopAppBar(title, subtitle, titleHorizontalAlignment = CenterHorizontally).)
                    TopAppBarWithSubtitle(
                        title = || Text("Takvim", modifier = Modifier.test_tag("orta-başlık")),
                        subtitle = || Text("Ekim", modifier = Modifier.test_tag("orta-alt")),
                        title_horizontal_alignment = Alignment::CenterHorizontally,
                    );
                    // (The C++'s MediumTopAppBar(title, subtitle): M15's medium bar has no subtitle; its subtitle form is
                    // MediumFlexibleTopAppBar.)
                    MediumFlexibleTopAppBar(
                        title = || Text("Notlar", modifier = Modifier.test_tag("m-başlık")),
                        subtitle =
                            Some(Rc::new(|| Text("12 not", modifier = Modifier.test_tag("m-alt"))) as Rc<dyn Fn()>),
                        modifier = Modifier.test_tag("orta-çubuk"),
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let at = |tag: &str| bounds(&rule, tag);
    // Small: the subtitle (12 sp) right under the title (22 sp), the two in the middle of the 64 dp bar.
    let (title, sub) = (at("başlık"), at("alt"));
    assert!(near(sub.top, title.bottom) && near(sub.left, title.left) && near(title.left, 16.0));
    assert!(near(title.top, (64.0 - 28.0 - 16.0) / 2.0) && near(sub.height(), 16.0)); // titleLarge's and labelMedium's lines
    let c = rule.draw();
    assert_eq!(color_of(&c, "3 okunmamış"), Some(cs.on_surface_variant));
    assert!(near(size_of(&c, "3 okunmamış"), 12.0));
    // the title in titleLarge, at the regular weight
    let i = c.texts.iter().position(|t| t == "Gelen kutusu").expect("the title drawn");
    assert!(c.text_styles[i].font_weight == FontWeight::Normal && near(c.text_styles[i].font_size, 22.0));
    // Centered: both in the middle.
    assert!(
        near(at("orta-başlık").center().x, 200.0)
            && near(at("orta-alt").center().x, 200.0)
            && near(at("orta-alt").top, at("orta-başlık").bottom)
    );
    // Medium: 24 dp taller for it, the subtitle under the large title, its baseline 24 dp above the bottom (M15 keeps
    // the title column's last baseline 24 dp up; the C++ kept the subtitle's bottom there).
    let bar = at("orta-çubuk");
    assert!(near(bar.height(), 112.0 + 24.0));
    // the small one in the top row (faded in as it collapses) and the large one's
    let subs = named_args!(rule.on_all_nodes_with_tag("m-alt", use_unmerged_tree = true)).fetch_semantics_nodes();
    assert_eq!(subs.len(), 2);
    let lowest = subs.iter().map(|n| n.bounds_in_root().bottom).fold(0.0, f32::max);
    let large = (0..c.texts.len()).find(|&i| c.texts[i] == "12 not" && near(c.text_styles[i].font_size, 14.0));
    let baseline = c.baselines[large.expect("the large subtitle drawn")].y;
    assert!(near(baseline, bar.bottom - 24.0) && lowest > baseline, "{baseline} {lowest} {bar:?}");
}

/// A bar's colors (each part its own), heights and insets can be set; a medium bar collapses from its expanded
/// height to its collapsed one.
fn top_app_bar_colors_heights_and_insets_can_be_set() {
    let cs = light_color_scheme();
    let (red, green, blue, magenta) = (
        Color::from_argb(0xFFFF0000),
        Color::from_argb(0xFF00FF00),
        Color::from_argb(0xFF0000FF),
        Color::from_argb(0xFFFF00FF),
    );
    let custom = mutable_state_of(false);
    let centered = mutable_state_of(false);
    let state = slot::<TopAppBarState>();
    let navigation_color = Rc::new(Cell::new(Color::Unspecified));
    let action_color = Rc::new(Cell::new(Color::Unspecified));
    let rule = {
        let (custom, centered, state) = (custom.clone(), centered.clone(), state.clone());
        let (navigation_color, action_color) = (navigation_color.clone(), action_color.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let behavior = TopAppBarDefaults.exit_until_collapsed_scroll_behavior();
                *state.borrow_mut() = Some(behavior.state());
                let own = custom.get();
                // (The C++ took each color, height and insets as an argument of its own; M15 takes them as colors,
                // expandedHeight and windowInsets.)
                let colors = if own {
                    named_args!(TopAppBarDefaults.top_app_bar_colors(
                        navigation_icon_content_color = red,
                        title_content_color = green,
                        action_icon_content_color = blue,
                        subtitle_content_color = magenta,
                    ))
                } else {
                    TopAppBarDefaults.top_app_bar_colors()
                };
                let height = if own { 80.dp() } else { TopAppBarDefaults::TopAppBarExpandedHeight };
                let insets = if own { WindowInsets::new(0, 10, 0, 0) } else { TopAppBarDefaults.window_insets() };
                let (navigation_color, action_color) = (navigation_color.clone(), action_color.clone());
                let navigation_icon = move || {
                    navigation_color.set(LocalContentColor.current());
                    VectorIcon(Icons::Filled::Menu(), Some("Menü".to_string()));
                };
                let actions = move || {
                    action_color.set(LocalContentColor.current());
                    VectorIcon(Icons::Filled::Search(), Some("Ara".to_string()));
                };
                let centered = centered.get();
                Column(
                    content = move || {
                        let (navigation_icon, actions) = (navigation_icon.clone(), actions.clone());
                        // (The C++'s CenterAlignedTopAppBar(title, subtitle): M15's subtitle form centered.)
                        TopAppBarWithSubtitle(
                            title = || Text("Başlık"),
                            modifier = Modifier.test_tag("tek"),
                            subtitle = || Text("Alt"),
                            navigation_icon = navigation_icon,
                            actions = actions,
                            title_horizontal_alignment =
                                if centered { Alignment::CenterHorizontally } else { Alignment::Start },
                            colors = colors,
                            expanded_height = height,
                            window_insets = insets.clone(),
                        );
                        MediumTopAppBar(
                            title = || Text("Orta"),
                            expanded_height =
                                if own { 150.dp() } else { TopAppBarDefaults::MediumAppBarExpandedHeight },
                            collapsed_height =
                                if own { 56.dp() } else { TopAppBarDefaults::MediumAppBarCollapsedHeight },
                            scroll_behavior = behavior.clone(),
                            modifier = Modifier.test_tag("orta"),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    // The theme's: the navigation icon and title in onSurface, actions and subtitle in onSurfaceVariant.
    let plain = rule.draw();
    assert!(navigation_color.get() == cs.on_surface && color_of(&plain, "Başlık") == Some(cs.on_surface));
    assert!(action_color.get() == cs.on_surface_variant && color_of(&plain, "Alt") == Some(cs.on_surface_variant));
    assert!(near(bounds(&rule, "tek").height(), 64.0) && near(bounds(&rule, "orta").height(), 112.0));
    assert!(near(get(&state).height_offset_limit(), 64.0 - 112.0));
    // Each part in its own color; 80 dp tall under 10 dp of insets; the medium one from 150 down to 56.
    custom.set(true);
    rule.wait_for_idle();
    let set = rule.draw();
    assert!(navigation_color.get() == red && color_of(&set, "Başlık") == Some(green));
    assert!(action_color.get() == blue && color_of(&set, "Alt") == Some(magenta));
    assert!(
        near(bounds(&rule, "tek").height(), 90.0)
            && near_within(text_bounds(&rule, "Başlık").center().y, 10.0 + 40.0, 12.0)
    );
    assert!(near(bounds(&rule, "orta").height(), 150.0) && near(get(&state).height_offset_limit(), 56.0 - 150.0));
    get(&state).set_height_offset(-94.0); // collapsed
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "orta").height(), 56.0));
    // The centered bar takes them all the same.
    centered.set(true);
    rule.wait_for_idle();
    let middle = rule.draw();
    assert!(navigation_color.get() == red && color_of(&middle, "Başlık") == Some(green));
    assert!(action_color.get() == blue && color_of(&middle, "Alt") == Some(magenta));
    assert!(near(bounds(&rule, "tek").height(), 90.0));
}

/// Right to left, the navigation icon is at the start (the right), the actions at the end (the left), the title in
/// the middle.
fn center_aligned_top_app_bar_mirrors_right_to_left() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                CenterAlignedTopAppBar(
                    title = || Text("Başlık"),
                    navigation_icon = || VectorIcon(Icons::Filled::Menu(), Some("Menü".to_string())),
                    actions = || VectorIcon(Icons::Filled::Search(), Some("Ara".to_string())),
                    window_insets = WindowInsets::Zero,
                );
            });
        }),
        IntSize::new(400, 800),
    );
    // The navigation icon at the start (the right), the actions at the end (the left), 4 dp in.
    let menu = rule.on_node_with_content_description("Menü").get_unclipped_bounds_in_root();
    let search = rule.on_node_with_content_description("Ara").get_unclipped_bounds_in_root();
    assert!(near(menu.right, 400.0 - 4.0) && near(search.left, 4.0), "{menu:?} {search:?}");
    assert!(near_within(text_bounds(&rule, "Başlık").center().x, 200.0, 1.0));
}

// ---- Compose's behaviour -------------------------------------------------------------------------------------------

/// The state keeps its height offset between its limit and 0; its collapsed fraction is the offset over the limit,
/// its overlapped fraction how far the content scrolled under it over the limit (both 0 with a limit of 0); its saver
/// writes the limit and the offsets and reads them back; remembered, it starts at Compose's defaults or those given.
fn top_app_bar_state_keeps_its_offsets_in_their_limits_and_is_saved() {
    let state = TopAppBarState::new(-100.0, -20.0, -30.0);
    assert_eq!((state.height_offset_limit(), state.height_offset(), state.content_offset()), (-100.0, -20.0, -30.0));
    assert!(near_within(state.collapsed_fraction(), 0.2, 1e-4));
    assert!(near_within(state.overlapped_fraction(), 0.3, 1e-4));
    state.set_content_offset(-250.0); // scrolled by more than the bar: wholly under it
    assert_eq!(state.overlapped_fraction(), 1.0);
    state.set_height_offset(-150.0);
    assert_eq!((state.height_offset(), state.collapsed_fraction()), (-100.0, 1.0));
    state.set_height_offset(10.0);
    assert_eq!((state.height_offset(), state.collapsed_fraction()), (0.0, 0.0));
    state.set_height_offset(-50.0);
    let saver = TopAppBarState::Saver();
    let restored = saver.restore(&saver.save(&state)).expect("restored");
    assert_eq!(
        (restored.height_offset_limit(), restored.height_offset(), restored.content_offset()),
        (-100.0, -50.0, -250.0)
    );
    assert!(restored != state && state.clone() == state);
    assert!(format!("{restored:?}").starts_with("TopAppBarState(heightOffsetLimit=-100"));
    state.set_height_offset_limit(0.0);
    assert_eq!((state.collapsed_fraction(), state.overlapped_fraction()), (0.0, 0.0));

    let remembered: Slot<(TopAppBarState, TopAppBarState)> = slot();
    let _rule = {
        let remembered = remembered.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let given = remember_top_app_bar_state(-64.0, -10.0, 5.0);
            *remembered.borrow_mut() = Some((remember_top_app_bar_state(), given));
        }))
    };
    let (default, given) = get(&remembered);
    assert_eq!(
        (default.height_offset_limit(), default.height_offset(), default.content_offset()),
        (-f32::MAX, 0.0, 0.0)
    );
    assert_eq!((given.height_offset_limit(), given.height_offset(), given.content_offset()), (-64.0, -10.0, 5.0));
}

/// The behaviors' connections: the pinned one notes what the content scrolled (the bar stays), its offset reset by a
/// fling down; entering always, the bar takes the scrolls before the content while it moves, either way; exiting
/// until collapsed, it takes the scrolls up before the content and the scrolls down only after it — each while
/// `can_scroll` says so. Remembered while their state and arguments stay (another state: another behavior).
fn the_scroll_behaviors_move_their_state_by_what_the_content_scrolls() {
    let can = Rc::new(Cell::new(true));
    let at_start = Rc::new(Cell::new(true));
    let other_state = mutable_state_of(false);
    let recompose = mutable_state_of(0);
    let behaviors: Slot<Vec<Rc<dyn TopAppBarScrollBehavior>>> = slot();
    let rule = {
        let (can, at_start, other_state, recompose, behaviors) =
            (can.clone(), at_start.clone(), other_state.clone(), recompose.clone(), behaviors.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            recompose.get();
            let own = remember(|| TopAppBarState::new(-64.0, 0.0, 0.0));
            let other = remember(|| TopAppBarState::new(-64.0, 0.0, 0.0));
            let state = if other_state.get() { other } else { own };
            let (c1, c2, c3, s) = (can.clone(), can.clone(), can.clone(), at_start.clone());
            let pinned = named_args!(TopAppBarDefaults.pinned_scroll_behavior(
                state = state,
                can_scroll = move || c1.get(),
                is_scrolling_content_at_start = move || s.get(),
            ));
            let enter = named_args!(TopAppBarDefaults.enter_always_scroll_behavior(
                state = remember(|| TopAppBarState::new(-64.0, 0.0, 0.0)),
                can_scroll = move || c2.get(),
            ));
            let exit = named_args!(TopAppBarDefaults.exit_until_collapsed_scroll_behavior(
                state = remember(|| TopAppBarState::new(-64.0, 0.0, 0.0)),
                can_scroll = move || c3.get(),
            ));
            *behaviors.borrow_mut() = Some(vec![pinned, enter, exit]);
        }))
    };
    let first = get(&behaviors);
    let [pinned, enter, exit] = [first[0].clone(), first[1].clone(), first[2].clone()];
    let source = NestedScrollSource::UserInput;
    let y = |v: f32| at(0.0, v);
    // Pinned: no specs; the content's scrolls noted, the bar staying.
    assert!(pinned.is_pinned() && pinned.snap_animation_spec().is_none() && pinned.fling_animation_spec().is_none());
    let p = pinned.nested_scroll_connection();
    assert_eq!(p.on_pre_scroll(y(-30.0), source), Offset::Zero);
    assert_eq!(p.on_post_scroll(y(-30.0), y(0.0), source), Offset::Zero);
    assert_eq!((pinned.state().height_offset(), pinned.state().content_offset()), (0.0, -30.0));
    assert!(pinned.state().overlapped_fraction() > 0.4);
    can.set(false);
    p.on_post_scroll(y(-30.0), y(0.0), source);
    assert_eq!(pinned.state().content_offset(), -30.0);
    can.set(true);
    drop(p.on_post_fling(Velocity::Zero, Velocity::new(0.0, 100.0))); // a fling down that ended: back at the top
    assert_eq!(pinned.state().content_offset(), 0.0);
    assert_eq!(pinned.state().overlapped_fraction(), 0.0);
    at_start.set(false); // told the content isn't at its start (a list pre-scrolled): under the bar
    assert_eq!(pinned.state().overlapped_fraction(), 1.0);
    at_start.set(true);
    // Entering always: the bar first, either way.
    assert!(!enter.is_pinned() && enter.snap_animation_spec().is_some() && enter.fling_animation_spec().is_some());
    let e = enter.nested_scroll_connection();
    assert_eq!(e.on_pre_scroll(y(-20.0), source), y(-20.0));
    assert_eq!(e.on_pre_scroll(y(-60.0), source), y(-60.0)); // (moved: all of it taken)
    assert_eq!(enter.state().height_offset(), -64.0);
    assert_eq!(e.on_pre_scroll(y(-10.0), source), Offset::Zero); // collapsed: the content's
    assert_eq!(e.on_post_scroll(y(-10.0), y(0.0), source), Offset::Zero);
    assert_eq!(enter.state().content_offset(), -10.0);
    assert_eq!(e.on_pre_scroll(y(5.0), source), y(5.0)); // down: the bar comes back before the content moves
    assert_eq!(enter.state().height_offset(), -59.0);
    can.set(false);
    assert_eq!(e.on_pre_scroll(y(5.0), source), Offset::Zero);
    can.set(true);
    // Exiting until collapsed: up before the content, down after it.
    assert!(!exit.is_pinned() && exit.snap_animation_spec().is_some() && exit.fling_animation_spec().is_some());
    let x = exit.nested_scroll_connection();
    assert_eq!(x.on_pre_scroll(y(10.0), source), Offset::Zero);
    assert_eq!(x.on_pre_scroll(y(-30.0), source), y(-30.0));
    assert_eq!(x.on_post_scroll(y(-5.0), y(0.0), source), y(-5.0)); // the content scrolled up: more
    assert_eq!(exit.state().height_offset(), -35.0);
    assert_eq!(x.on_post_scroll(y(0.0), y(20.0), source), y(20.0)); // what the content left of a scroll down
    assert_eq!((exit.state().height_offset(), exit.state().content_offset()), (-15.0, -5.0));
    // Composed again with the same state and arguments: the same behaviors.
    recompose.set(1);
    rule.wait_for_idle();
    let again = get(&behaviors);
    assert!(first.iter().zip(&again).all(|(a, b)| Rc::ptr_eq(a, b)));
    // Another state: another pinned behavior.
    other_state.set(true);
    rule.wait_for_idle();
    let second = get(&behaviors);
    assert!(!Rc::ptr_eq(&first[0], &second[0]) && *first[0] != *second[0] && Rc::ptr_eq(&first[1], &second[1]));
    assert!(format!("{:?}", &*second[0]).contains("isPinned=true"));
}

/// The behaviors made of a lazy list's or a scroll's state tell their state whether the content is at its start: a
/// list scrolled down (or a reversed scroll at its bottom) is under the bar though no scroll was noted, until it is
/// back at its start.
fn behaviors_of_a_list_or_a_scroll_know_whether_it_is_at_its_start() {
    let list = LazyListState::new(30, 0);
    let scroll = ScrollState::new(0);
    let behaviors: Slot<Vec<Rc<dyn TopAppBarScrollBehavior>>> = slot();
    let rule = {
        let (list, scroll, behaviors) = (list.clone(), scroll.clone(), behaviors.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let pinned = TopAppBarDefaults.pinned_scroll_behavior_lazy_list(list.clone());
                let enter = TopAppBarDefaults.enter_always_scroll_behavior_lazy_list(list.clone());
                let reversed = named_args!(
                    TopAppBarDefaults.pinned_scroll_behavior_scroll_state(scroll.clone(), reverse_scrolling = true)
                );
                let entering_reversed = named_args!(
                    TopAppBarDefaults
                        .enter_always_scroll_behavior_scroll_state(scroll.clone(), reverse_scrolling = true,)
                );
                *behaviors.borrow_mut() = Some(vec![pinned, enter, reversed, entering_reversed]);
                let (list, scroll) = (list.clone(), scroll.clone());
                Column(
                    content = move || {
                        let list = list.clone();
                        Box(
                            modifier = Modifier.height(300.dp()),
                            content = move || Rows(list.clone(), PaddingValues::all(Dp(0.0))),
                        );
                        Column(
                            modifier = named_args!(
                                Modifier.height(300.dp()).vertical_scroll(&scroll, reverse_scrolling = true)
                            ),
                            content = || Box(modifier = Modifier.height(2000.dp())),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let overlapped = || get(&behaviors).iter().map(|b| b.state().overlapped_fraction()).collect::<Vec<_>>();
    // The list scrolled down to its row 30; the reversed scroll at its start, its bottom: both under the bar.
    assert_eq!(overlapped(), vec![1.0, 1.0, 1.0, 1.0]);
    list.request_scroll_to_item(0);
    scroll.dispatch_raw_delta(scroll.max_value() as f32);
    rule.wait_for_idle();
    assert_eq!(overlapped(), vec![0.0, 0.0, 0.0, 0.0]);
}

/// The deprecated entering-always behavior over a reversed content takes nothing of the scrolls before the content
/// (though it moves the bar) and doesn't move the bar by what the content scrolled; not reversed, it takes them as
/// entering always does and moves the bar by what the content scrolled too.
fn the_deprecated_reversed_enter_always_leaves_a_reversed_content_its_scrolls() {
    let behaviors: Slot<Vec<Rc<dyn TopAppBarScrollBehavior>>> = slot();
    let _rule = {
        let behaviors = behaviors.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let reversed = named_args!(TopAppBarDefaults.enter_always_scroll_behavior_reverse_layout(
                state = remember(|| TopAppBarState::new(-64.0, 0.0, 0.0)),
                reverse_layout = true,
            ));
            let forward = named_args!(TopAppBarDefaults.enter_always_scroll_behavior_reverse_layout(
                state = remember(|| TopAppBarState::new(-64.0, 0.0, 0.0)),
            ));
            *behaviors.borrow_mut() = Some(vec![reversed, forward]);
        }))
    };
    let b = get(&behaviors);
    let source = NestedScrollSource::UserInput;
    let (reversed, forward) = (b[0].clone(), b[1].clone());
    assert!(!reversed.is_pinned() && reversed.snap_animation_spec().is_some());
    let r = reversed.nested_scroll_connection();
    assert_eq!(r.on_pre_scroll(at(0.0, -20.0), source), Offset::Zero);
    assert_eq!(reversed.state().height_offset(), -20.0);
    r.on_post_scroll(at(0.0, -10.0), Offset::Zero, source);
    assert_eq!((reversed.state().height_offset(), reversed.state().content_offset()), (-20.0, -10.0));
    let f = forward.nested_scroll_connection();
    assert_eq!(f.on_pre_scroll(at(0.0, -20.0), source), at(0.0, -20.0));
    f.on_post_scroll(at(0.0, -10.0), Offset::Zero, source);
    assert_eq!((forward.state().height_offset(), forward.state().content_offset()), (-30.0, -10.0));
}

/// A bar that isn't pinned is dragged itself (its state's offset moved by the drag); let go, it settles: back when
/// less than half collapsed, collapsed when more. A pinned bar takes no drags.
fn dragging_a_top_app_bar_moves_it_and_it_settles() {
    let pinned = mutable_state_of(false);
    let behavior = slot::<Rc<dyn TopAppBarScrollBehavior>>();
    let rule = {
        let (pinned, behavior) = (pinned.clone(), behavior.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let b = if pinned.get() {
                    TopAppBarDefaults.pinned_scroll_behavior()
                } else {
                    TopAppBarDefaults.exit_until_collapsed_scroll_behavior()
                };
                *behavior.borrow_mut() = Some(b.clone());
                MediumTopAppBar(
                    title = || Text("Başlık"),
                    modifier = Modifier.test_tag("çubuk"),
                    window_insets = WindowInsets::Zero,
                    scroll_behavior = b,
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let state = || get(&behavior).state();
    let drag = |from: f32, to: f32| {
        let h = rule.host();
        h.pointer_down(at(200.0, from), -1, 0, PointerType::Touch);
        for i in 1..=10 {
            rule.advance_frame();
            h.pointer_move(at(200.0, from + (to - from) * i as f32 / 10.0), -1, 0);
        }
        rule.advance_time_by(200);
        let moved = state().height_offset();
        h.pointer_up(at(200.0, to), -1, 0);
        rule.wait_for_idle();
        moved
    };
    assert!(near(state().height_offset_limit(), 64.0 - 112.0));
    // up 18 less the 8 dp touch slop: 10 of 48, then back
    let moved = drag(100.0, 82.0);
    assert!(near(moved, -10.0), "{moved}");
    assert!(near(state().height_offset(), 0.0) && near(bounds(&rule, "çubuk").height(), 112.0));
    // up 38 less the slop: 30 of 48, then collapsed
    let moved = drag(100.0, 62.0);
    assert!(near(moved, -30.0), "{moved}");
    assert!(near(state().height_offset(), -48.0) && near(bounds(&rule, "çubuk").height(), 64.0));
    // down again: expanded
    drag(30.0, 80.0);
    assert!(near(state().height_offset(), 0.0));
    // Pinned: it stays.
    pinned.set(true);
    rule.wait_for_idle();
    drag(100.0, 40.0);
    assert!(near(state().height_offset(), 0.0) && near(bounds(&rule, "çubuk").height(), 112.0));
}

/// A two-rows bar's container goes over to its scrolled color as it collapses (by how far, not animated); its large
/// row's title is what accessibility finds expanded, the small row's once it is more than half collapsed.
fn a_two_rows_top_app_bar_takes_its_scrolled_color_and_title_as_it_collapses() {
    let cs = light_color_scheme();
    let state = TopAppBarState::new(-f32::MAX, 0.0, 0.0);
    let rule = {
        let state = state.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let behavior =
                    named_args!(TopAppBarDefaults.exit_until_collapsed_scroll_behavior(state = state.clone()));
                LargeTopAppBar(
                    title = || Text("Başlık"),
                    window_insets = WindowInsets::Zero,
                    scroll_behavior = behavior,
                );
            }),
            IntSize::new(400, 800),
        )
    };
    let container = || {
        let c = rule.draw();
        c.rects.iter().zip(&c.rect_colors).find(|(r, _)| near(r.top, 0.0) && near(r.width(), 400.0)).map(|(_, c)| *c)
    };
    let title_top = || rule.on_node_with_text("Başlık").get_unclipped_bounds_in_root().top;
    assert_eq!(container(), Some(cs.surface));
    assert_eq!(rule.on_all_nodes_with_text("Başlık").fetch_semantics_nodes().len(), 1);
    assert!(title_top() > 64.0, "the large row's: {}", title_top());
    state.set_height_offset(-44.0); // half of 88
    rule.wait_for_idle();
    let half = lerp_color(cs.surface, cs.surface_container, kompoz_animation::FastOutLinearInEasing.transform(0.5));
    assert_eq!(container(), Some(half));
    state.set_height_offset(-88.0);
    rule.wait_for_idle();
    assert_eq!(container(), Some(cs.surface_container));
    assert_eq!(rule.on_all_nodes_with_text("Başlık").fetch_semantics_nodes().len(), 1);
    assert!(title_top() < 64.0, "the small row's: {}", title_top());
}

/// Colors copy with those given (an unspecified one keeps its own); the deprecated constructor colors the subtitle
/// as the title; the deprecated center aligned, medium and large colors are the top app bar's.
fn top_app_bar_colors_copy_and_their_deprecated_forms() {
    let (red, green) = (Color::from_argb(0xFFFF0000), Color::from_argb(0xFF00FF00));
    let c = TopAppBarColors::new(Color::White, Color::Black, red, green, Color::Gray, Color::Blue);
    let copied = named_args!(c.copy(title_content_color = red, subtitle_content_color = Color::Unspecified));
    assert_eq!(copied, TopAppBarColors { title_content_color: red, ..c });
    assert_eq!(c.copy(), c);
    let old = TopAppBarColors::without_subtitle_content_color(Color::White, Color::Black, red, green, Color::Gray);
    assert_eq!(old.subtitle_content_color, green);
    let colors: Slot<Vec<TopAppBarColors>> = slot();
    let _rule = {
        let colors = colors.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            *colors.borrow_mut() = Some(vec![
                TopAppBarDefaults.top_app_bar_colors(),
                TopAppBarDefaults.center_aligned_top_app_bar_colors(),
                TopAppBarDefaults.medium_top_app_bar_colors(),
                TopAppBarDefaults.large_top_app_bar_colors(),
                named_args!(TopAppBarDefaults.large_top_app_bar_colors(container_color = red)),
            ]);
        }))
    };
    let colors = get(&colors);
    assert!(colors[1..4].iter().all(|c| *c == colors[0]));
    assert_eq!(colors[4], TopAppBarColors { container_color: red, ..colors[0] });
}

/// A single-row override noting what it is given.
struct OwnSingleRow(Rc<RefCell<Vec<String>>>);

impl SingleRowTopAppBarOverride for OwnSingleRow {
    #[track_caller]
    fn SingleRowTopAppBar(&self, scope: &SingleRowTopAppBarOverrideScope) {
        let given = self.0.clone();
        let scope = scope.clone();
        kompozable_block!({
            let alignment =
                if scope.title_horizontal_alignment() == Alignment::CenterHorizontally { "center" } else { "start" };
            given.borrow_mut().push(format!(
                "{alignment} {} {} {} {} {} {}",
                scope.expanded_height().value(),
                scope.subtitle().is_some(),
                scope.title_text_style().font_size.value(),
                scope.colors() == TopAppBarDefaults.top_app_bar_colors(),
                scope.content_padding() == TopAppBarDefaults::ContentPadding(),
                scope.scroll_behavior().is_some(),
            ));
            let (title, actions, navigation_icon) = (scope.title(), scope.actions(), scope.navigation_icon());
            let _ = (scope.window_insets(), scope.subtitle_text_style());
            Row(
                modifier = scope.modifier().test_tag("kendi"),
                content = move || {
                    navigation_icon();
                    title();
                    actions();
                },
            );
        })
    }
}

/// A two-rows override noting what it is given.
struct OwnTwoRows(Rc<RefCell<Vec<String>>>);

impl TwoRowsTopAppBarOverride for OwnTwoRows {
    #[track_caller]
    fn TwoRowsTopAppBar(&self, scope: &TwoRowsTopAppBarOverrideScope) {
        self.0.borrow_mut().push(format!(
            "{} {} {} {} {} {}",
            scope.collapsed_height().value(),
            scope.expanded_height().value(),
            scope.title_bottom_padding().value(),
            scope.title_text_style().font_size.value(),
            scope.small_title_text_style().font_size.value(),
            scope.subtitle().is_some() && scope.small_subtitle().is_some(),
        ));
        let (title, small_title) = (scope.title(), scope.small_title());
        let _ = (
            scope.subtitle_text_style(),
            scope.small_subtitle_text_style(),
            scope.title_horizontal_alignment(),
            scope.navigation_icon(),
            scope.actions(),
            scope.window_insets(),
            scope.colors(),
            scope.scroll_behavior(),
        );
        let modifier = scope.modifier();
        kompozable_block!({
            Column(
                modifier = modifier.test_tag("kendi iki"),
                content = move || {
                    title();
                    small_title();
                },
            );
        })
    }
}

/// The bars are made by the overrides their locals provide, given their arguments (the defaults filled in); the
/// default overrides make Material's bars.
fn top_app_bars_are_made_by_the_overrides_provided() {
    let given = Rc::new(RefCell::new(Vec::<String>::new()));
    let own = mutable_state_of(true);
    let rule = {
        let (given, own) = (given.clone(), own.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let single: Rc<dyn SingleRowTopAppBarOverride> = if own.get() {
                    Rc::new(OwnSingleRow(given.clone()))
                } else {
                    Rc::new(DefaultSingleRowTopAppBarOverride)
                };
                let two: Rc<dyn TwoRowsTopAppBarOverride> = if own.get() {
                    Rc::new(OwnTwoRows(given.clone()))
                } else {
                    Rc::new(DefaultTwoRowsTopAppBarOverride)
                };
                KompozitionLocalProvider(
                    &[LocalSingleRowTopAppBarOverride.provides(single), LocalTwoRowsTopAppBarOverride.provides(two)],
                    || {
                        Column(
                            content = || {
                                TopAppBar(title = || Text("Küçük"), actions = || Text("Eylem"));
                                TopAppBarWithSubtitle(
                                    title = || Text("Alt yazılı"),
                                    subtitle = || Text("alt"),
                                    title_horizontal_alignment = Alignment::CenterHorizontally,
                                    expanded_height = Dp::Unspecified,
                                );
                                CenterAlignedTopAppBar(
                                    title = || Text("Orta"),
                                    expanded_height = 80.dp(),
                                    scroll_behavior = TopAppBarDefaults.pinned_scroll_behavior(),
                                );
                                MediumTopAppBar(title = || Text("Orta boy"));
                                LargeTopAppBar(title = || Text("Büyük"), expanded_height = Dp::Infinity);
                                TwoRowsTopAppBar(
                                    title = |expanded: bool| Text(if expanded { "Açık" } else { "Kapalı" }),
                                    subtitle = Some(Rc::new(|_: bool| Text("alt")) as Rc<dyn Fn(bool)>),
                                );
                            },
                        );
                    },
                );
            }),
            IntSize::new(400, 1000),
        )
    };
    assert_eq!(
        given.borrow().clone(),
        vec![
            "start 64 false 22 true true false",
            "center 64 true 22 true true false",
            "center 80 false 22 true true true",
            "64 112 24 24 22 false",
            "64 152 28 28 22 false",
            "64 136 0 28 22 true",
        ]
    );
    assert_eq!(rule.on_all_nodes_with_tag("kendi").fetch_semantics_nodes().len(), 3);
    rule.on_node_with_text("Eylem").assert_exists();
    // the two-rows bar's own titles, asked for each row
    rule.on_node_with_text("Açık").assert_exists();
    rule.on_node_with_text("Kapalı").assert_exists();
    // The default overrides: Material's bars.
    own.set(false);
    rule.wait_for_idle();
    rule.on_node_with_tag("kendi").assert_does_not_exist();
    assert!(near(text_bounds(&rule, "Küçük").left, 16.0));
}

/// The defaults: Compose's heights, no content padding, the system bars' insets at the sides and the top, the theme's
/// colors (the surface, its high container scrolled under, the navigation icon and title on the surface, actions and
/// subtitle on its variant); an unspecified or infinite height is the default.
fn top_app_bar_defaults_as_compose() {
    assert_eq!(
        [
            TopAppBarDefaults::TopAppBarExpandedHeight,
            TopAppBarDefaults::MediumAppBarCollapsedHeight,
            TopAppBarDefaults::MediumAppBarExpandedHeight,
            TopAppBarDefaults::MediumFlexibleAppBarWithoutSubtitleExpandedHeight,
            TopAppBarDefaults::MediumFlexibleAppBarWithSubtitleExpandedHeight,
            TopAppBarDefaults::LargeAppBarCollapsedHeight,
            TopAppBarDefaults::LargeAppBarExpandedHeight,
            TopAppBarDefaults::LargeFlexibleAppBarWithoutSubtitleExpandedHeight,
            TopAppBarDefaults::LargeFlexibleAppBarWithSubtitleExpandedHeight,
        ]
        .map(|d| d.value()),
        [64.0, 64.0, 112.0, 112.0, 136.0, 64.0, 152.0, 120.0, 152.0]
    );
    assert_eq!(TopAppBarDefaults::ContentPadding(), PaddingValues::all(Dp(0.0)));
    let cs = light_color_scheme();
    let colors = slot::<TopAppBarColors>();
    let rule = {
        let colors = colors.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                *colors.borrow_mut() = Some(TopAppBarDefaults.top_app_bar_colors());
                Column(
                    content = || {
                        TopAppBar(title = || Text("Başlık"), modifier = Modifier.test_tag("çubuk"));
                        TopAppBar(
                            title = || Text("Sınırsız"),
                            expanded_height = Dp::Infinity,
                            window_insets = WindowInsets::Zero,
                            modifier = Modifier.test_tag("sınırsız"),
                        );
                        MediumTopAppBar(
                            title = || Text("Orta"),
                            collapsed_height = Dp::Unspecified,
                            expanded_height = Dp::Unspecified,
                            window_insets = WindowInsets::Zero,
                            modifier = Modifier.test_tag("orta"),
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        )
    };
    assert_eq!(
        get(&colors),
        TopAppBarColors::new(
            cs.surface,
            cs.surface_container,
            cs.on_surface,
            cs.on_surface,
            cs.on_surface_variant,
            cs.on_surface_variant,
        )
    );
    assert!(near(bounds(&rule, "sınırsız").height(), 64.0) && near(bounds(&rule, "orta").height(), 112.0));
    // The system bars' insets: the bar keeps clear of the status bar at the top and the sides (and not of the
    // navigation bar at the bottom).
    assert!(near(bounds(&rule, "çubuk").height(), 64.0));
    rule.host().set_system_insets(kompoz_ui::platform::SystemInsets {
        status_bars: kompoz_ui::platform::Insets::new(10, 24, 12, 0),
        navigation_bars: kompoz_ui::platform::Insets::new(0, 0, 0, 48),
        ..Default::default()
    });
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "çubuk").height(), 64.0 + 24.0));
    let title = text_bounds(&rule, "Başlık");
    assert!(near(title.left, 10.0 + 16.0) && title.top > 24.0, "{title:?}");
}

// ---- M15's samples ---------------------------------------------------------------------------------------------
// (as written, in Rust; Kotlin's twins in crates/kompoz/tests/kt/top_app_bars.kt. SimpleTopAppBarWithAdaptiveActions
// reads material3-adaptive's currentWindowAdaptiveInfo: kompoz-material3-adaptive, a dev-dependency here.)

/// The samples' icon buttons, each in a plain tooltip above it (their `TooltipBox(positionProvider =
/// TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above), tooltip = { PlainTooltip {
/// Text(tooltip) } }, state = rememberTooltipState()) { IconButton(onClick = { /* doSomething() */ }) { Icon(…) } }`).
#[kompozable]
fn TooltipIconButton(tooltip: String, image_vector: ImageVector, content_description: String) {
    TooltipBox(
        position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
        tooltip = move |scope: &dyn TooltipScope| {
            let tooltip = tooltip.clone();
            scope.PlainTooltip(content = move || Text(tooltip.clone()));
        },
        state = remember_tooltip_state(),
        content = move || {
            let (image_vector, content_description) = (image_vector.clone(), content_description.clone());
            IconButton(
                on_click = || { /* doSomething() */ },
                content = move || {
                    VectorIcon(
                        image_vector = image_vector.clone(),
                        content_description = Some(content_description.clone()),
                    )
                },
            );
        },
    );
}

/// The samples' navigation icon.
#[kompozable]
fn MenuButton() {
    TooltipIconButton("Menu".into(), Icons::Filled::Menu(), "Menu".into());
}

/// The samples' action.
#[kompozable]
fn FavoriteButton() {
    TooltipIconButton("Add to favorites".into(), Icons::Filled::Favorite(), "Add to favorites".into());
}

/// The samples' `Text(…, maxLines = 1, overflow = TextOverflow.Ellipsis)` titles.
#[kompozable]
fn OneLine(text: &'static str) {
    Text(text, max_lines = 1, overflow = TextOverflow::Ellipsis);
}

/// The samples' content: `LazyColumn(contentPadding = innerPadding, verticalArrangement = Arrangement.spacedBy(8.dp))`
/// of the 76 numbered rows (`state` the one given).
#[kompozable]
fn SampleList(inner_padding: PaddingValues, state: Option<LazyListState>) {
    LazyColumn(
        state = state,
        content_padding = inner_padding,
        vertical_arrangement = Arrangement::spaced_by(8.dp()),
        content = |scope: &mut LazyListScope| {
            let list: Rc<Vec<String>> = Rc::new((0..=75).map(|it| it.to_string()).collect());
            let count = list.len() as i32;
            scope.items(count, move |it| {
                let list = list.clone();
                kompozable_block!({
                    Text(
                        text = list[it as usize].clone(),
                        style = MaterialTheme::typography().body_large,
                        modifier = Modifier.fill_max_width().padding_hv(16.dp(), 0.dp()),
                    )
                })
            });
        },
    );
}

#[kompozable]
fn SimpleTopAppBar() {
    Scaffold(
        top_bar = || {
            TopAppBar(
                title = || OneLine("Simple TopAppBar"),
                navigation_icon = || MenuButton(),
                actions = || FavoriteButton(),
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding, None),
    );
}

#[kompozable]
fn SimpleTopAppBarWithAdaptiveActions() {
    let size_class = current_window_adaptive_info().window_size_class;
    // Material guidelines state 3 items max in compact, and 5 items max elsewhere.
    // To test this, try a resizable emulator, or a phone in landscape and portrait orientation.
    let max_item_count = if size_class.min_width_dp() >= WindowSizeClass::WIDTH_DP_MEDIUM_LOWER_BOUND { 5 } else { 3 };
    let icons = vec![
        ExtendedIcons::Filled::Attachment(),
        ExtendedIcons::Filled::Edit(),
        ExtendedIcons::Outlined::Star(),
        ExtendedIcons::Filled::Snooze(),
        ExtendedIcons::Outlined::MarkEmailUnread(),
    ];
    let items = vec!["Attachment", "Edit", "Star", "Snooze", "Mark unread"];

    Scaffold(
        top_bar = move || {
            let (icons, items) = (icons.clone(), items.clone());
            TopAppBar(
                title = || OneLine("Simple TopAppBar"),
                navigation_icon = || MenuButton(),
                actions = move || {
                    let (icons, items) = (icons.clone(), items.clone());
                    AppBarRow(
                        max_item_count = max_item_count,
                        overflow_indicator = |it: AppBarMenuState| {
                            TooltipBox(
                                position_provider =
                                    TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                                tooltip = |scope: &dyn TooltipScope| scope.PlainTooltip(content = || Text("Overflow")),
                                state = remember_tooltip_state(),
                                content = move || {
                                    let it = it.clone();
                                    IconButton(
                                        on_click = move || it.show(),
                                        content = || {
                                            VectorIcon(
                                                image_vector = Icons::Filled::MoreVert(),
                                                content_description = Some("Overflow".into()),
                                            )
                                        },
                                    );
                                },
                            );
                        },
                        content = move |scope: &dyn AppBarRowScope| {
                            for (index, item) in items.iter().enumerate() {
                                let icon = icons[index].clone();
                                let item = item.to_string();
                                let description = item.clone();
                                scope.clickable_item(
                                    || {},
                                    move || {
                                        VectorIcon(
                                            image_vector = icon.clone(),
                                            content_description = Some(description.clone()),
                                        )
                                    },
                                    item,
                                );
                            }
                        },
                    );
                },
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding, None),
    );
}

#[kompozable]
fn SimpleTopAppBarWithSubtitle() {
    let scroll_behavior = TopAppBarDefaults.pinned_scroll_behavior();
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            TopAppBarWithSubtitle(
                title = || OneLine("Simple TopAppBar"),
                subtitle = || OneLine("Subtitle"),
                navigation_icon = || MenuButton(),
                actions = || FavoriteButton(),
                scroll_behavior = bar.clone(),
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding, None),
    );
}

#[kompozable]
fn SimpleCenterAlignedTopAppBar() {
    Scaffold(
        top_bar = || {
            CenterAlignedTopAppBar(
                title = || OneLine("Centered TopAppBar"),
                navigation_icon = || MenuButton(),
                actions = || FavoriteButton(),
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding, None),
    );
}

#[kompozable]
fn SimpleCenterAlignedTopAppBarWithSubtitle() {
    let scroll_behavior = TopAppBarDefaults.pinned_scroll_behavior();
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            TopAppBarWithSubtitle(
                title = || OneLine("Simple TopAppBar"),
                subtitle = || OneLine("Subtitle"),
                title_horizontal_alignment = Alignment::CenterHorizontally,
                navigation_icon = || MenuButton(),
                actions = || FavoriteButton(),
                scroll_behavior = bar.clone(),
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding, None),
    );
}

#[kompozable]
fn PinnedTopAppBar() {
    let scroll_behavior = TopAppBarDefaults.pinned_scroll_behavior();
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            TopAppBar(
                title = || OneLine("TopAppBar"),
                navigation_icon = || MenuButton(),
                actions = || {
                    // RowScope here, so these icons will be placed horizontally
                    FavoriteButton();
                    TooltipIconButton("Add to starred".into(), Icons::Filled::Star(), "Star".into());
                },
                scroll_behavior = bar.clone(),
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding, None),
    );
}

#[kompozable]
fn PinnedTopAppBarWithPreScrolledLazyColumn() {
    let lazy_list_state = remember_lazy_list_state(30);
    // Pass the state to ensure the top app bar color updates correctly when content is reversed or
    // pre-scrolled.
    let scroll_behavior = TopAppBarDefaults.pinned_scroll_behavior_lazy_list(lazy_list_state.clone());
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            TopAppBar(
                title = || OneLine("TopAppBar"),
                navigation_icon = || MenuButton(),
                scroll_behavior = bar.clone(),
            );
        },
        content = move |inner_padding: PaddingValues| SampleList(inner_padding, Some(lazy_list_state.clone())),
    );
}

#[kompozable]
fn PinnedTopAppBarWithReversedLazyGrid() {
    let lazy_grid_state = remember_lazy_grid_state();
    // In a reversed grid, we need to provide a custom `isScrollingContentAtStart` to the scroll
    // behavior to ensure the top app bar's color updates correctly.
    let is_scrolling_content_at_start = {
        let state = lazy_grid_state.clone();
        remember_keyed(lazy_grid_state.clone(), move || {
            derived_state_of(move || {
                if state.layout_info().reverse_layout() {
                    !state.can_scroll_forward()
                } else {
                    !state.can_scroll_backward()
                }
            })
        })
    };
    let scroll_behavior = named_args!(
        TopAppBarDefaults
            .pinned_scroll_behavior(is_scrolling_content_at_start = move || is_scrolling_content_at_start.value())
    );
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            TopAppBar(
                title = || OneLine("TopAppBar"),
                navigation_icon = || MenuButton(),
                scroll_behavior = bar.clone(),
            );
        },
        content = move |inner_padding: PaddingValues| {
            LazyVerticalGrid(
                columns = GridCells::Adaptive(100.dp()),
                reverse_layout = true,
                content_padding = inner_padding,
                state = Some(lazy_grid_state.clone()),
                content = |scope: &mut LazyGridScope| {
                    let list: Rc<Vec<String>> = Rc::new((0..=75).map(|it| it.to_string()).collect());
                    let count = list.len() as i32;
                    scope.items(count, move |it| {
                        let text = list[it as usize].clone();
                        kompozable_block!({
                            Box(
                                modifier = Modifier.fill_max_width().height(50.dp()),
                                content = move || {
                                    Text(
                                        text = text.clone(),
                                        style = MaterialTheme::typography().body_large,
                                        modifier = Modifier.fill_max_width().padding_hv(16.dp(), 0.dp()),
                                    )
                                },
                            )
                        })
                    });
                },
            );
        },
    );
}

#[kompozable]
fn EnterAlwaysTopAppBar() {
    let scroll_behavior = TopAppBarDefaults.enter_always_scroll_behavior();
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            TopAppBarWithSubtitle(
                title = || OneLine("TopAppBar"),
                subtitle = || OneLine("Subtitle"),
                navigation_icon = || MenuButton(),
                actions = || FavoriteButton(),
                scroll_behavior = bar.clone(),
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding, None),
    );
}

#[kompozable]
fn EnterAlwaysTopAppBarWithReverseScrolling() {
    let scroll_state = remember_scroll_state();
    let scroll_behavior =
        // Pass these parameters to ensure the top app bar color updates correctly when content has
        // reverse scrolling.
        named_args!(TopAppBarDefaults.enter_always_scroll_behavior_scroll_state(
            scroll_state = scroll_state.clone(),
            reverse_scrolling = true,
        ));
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            TopAppBar(
                title = || OneLine("TopAppBar"),
                navigation_icon = || MenuButton(),
                scroll_behavior = bar.clone(),
            );
        },
        content = move |inner_padding: PaddingValues| {
            Column(
                modifier = named_args!(
                    Modifier
                        .fill_max_size()
                        .padding_values(inner_padding)
                        .vertical_scroll(&scroll_state, reverse_scrolling = true)
                ),
                vertical_arrangement = Arrangement::Bottom,
                content = || {
                    for it in 0..=75 {
                        Box(
                            modifier = Modifier.fill_max_size().padding_hv(16.dp(), 0.dp()),
                            content =
                                move || Text(text = it.to_string(), style = MaterialTheme::typography().body_large),
                        );
                    }
                },
            );
        },
    );
}

#[kompozable]
fn ExitUntilCollapsedMediumTopAppBar() {
    let scroll_behavior = TopAppBarDefaults.exit_until_collapsed_scroll_behavior();
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            MediumTopAppBar(
                title = || OneLine("Medium TopAppBar"),
                navigation_icon = || MenuButton(),
                actions = || FavoriteButton(),
                scroll_behavior = bar.clone(),
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding, None),
    );
}

#[kompozable]
fn ExitUntilCollapsedCenterAlignedMediumFlexibleTopAppBar() {
    let scroll_behavior = TopAppBarDefaults.exit_until_collapsed_scroll_behavior();
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            MediumFlexibleTopAppBar(
                title = || OneLine("Medium TopAppBar"),
                subtitle = Some(Rc::new(|| OneLine("Subtitle")) as Rc<dyn Fn()>),
                title_horizontal_alignment = Alignment::CenterHorizontally,
                navigation_icon = || MenuButton(),
                actions = || FavoriteButton(),
                scroll_behavior = bar.clone(),
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding, None),
    );
}

#[kompozable]
fn ExitUntilCollapsedLargeTopAppBar() {
    let scroll_behavior = TopAppBarDefaults.exit_until_collapsed_scroll_behavior();
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            LargeTopAppBar(
                title = || OneLine("Large TopAppBar"),
                navigation_icon = || MenuButton(),
                actions = || FavoriteButton(),
                scroll_behavior = bar.clone(),
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding, None),
    );
}

#[kompozable]
fn ExitUntilCollapsedCenterAlignedLargeFlexibleTopAppBar() {
    let scroll_behavior = TopAppBarDefaults.exit_until_collapsed_scroll_behavior();
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            LargeFlexibleTopAppBar(
                title = || OneLine("Large TopAppBar"),
                subtitle = Some(Rc::new(|| OneLine("Subtitle")) as Rc<dyn Fn()>),
                title_horizontal_alignment = Alignment::CenterHorizontally,
                navigation_icon = || MenuButton(),
                actions = || FavoriteButton(),
                scroll_behavior = bar.clone(),
            );
        },
        content = |inner_padding: PaddingValues| SampleList(inner_padding, None),
    );
}

#[kompozable]
fn CustomTwoRowsTopAppBar() {
    let scroll_behavior = TopAppBarDefaults.exit_until_collapsed_scroll_behavior();
    let bar = scroll_behavior.clone();
    Scaffold(
        modifier = Modifier.nested_scroll(scroll_behavior.nested_scroll_connection()),
        top_bar = move || {
            TwoRowsTopAppBar(
                title = |expanded: bool| {
                    if expanded {
                        Text("Expanded TopAppBar", max_lines = 1, overflow = TextOverflow::Ellipsis);
                    } else {
                        Text("Collapsed TopAppBar", max_lines = 1, overflow = TextOverflow::Ellipsis);
                    }
                },
                subtitle = Some(Rc::new(|expanded: bool| {
                    if expanded {
                        Text(
                            "Expanded Subtitle",
                            max_lines = 1,
                            overflow = TextOverflow::Ellipsis,
                            modifier = Modifier.padding_sides(bottom = 24.dp()),
                        );
                    } else {
                        Text("Collapsed Subtitle", max_lines = 1, overflow = TextOverflow::Ellipsis);
                    }
                }) as Rc<dyn Fn(bool)>),
                collapsed_height = 64.dp(),
                expanded_height = 156.dp(),
                navigation_icon = || MenuButton(),
                scroll_behavior = bar.clone(),
            );
        },
        content = |inner_padding: PaddingValues| {
            Column(
                modifier =
                    Modifier.fill_max_width().padding_values(inner_padding).vertical_scroll(&remember_scroll_state()),
                content = || {
                    KompozitionLocalProvider(
                        &[LocalTextStyle.provides(MaterialTheme::typography().body_large)],
                        || Text(text = remember(|| LoremIpsum().values().first().cloned().unwrap_or_default())),
                    );
                },
            );
        },
    );
}

/// The bar the sample's scaffold drew: its container's rect (at the top, as wide as the window) and color.
fn bar(rule: &KompozTestRule) -> (Rect, Color) {
    let c = rule.draw();
    c.rects
        .iter()
        .zip(&c.rect_colors)
        .find(|(r, _)| near(r.left, 0.0) && near(r.top, 0.0) && near(r.width(), 400.0) && r.height() < 400.0)
        .map(|(r, c)| (*r, *c))
        .unwrap_or((Rect::Zero, Color::Unspecified))
}

/// Where the sample's button of this content description is.
fn button(rule: &KompozTestRule, description: &str) -> Rect {
    rule.on_node_with_content_description(description).get_unclipped_bounds_in_root()
}

/// The sample's text (in the merged tree: a two-rows bar's row hidden from accessibility not found).
fn shown(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with_text(text).get_unclipped_bounds_in_root()
}

/// A small bar's parts: the navigation icon at the start, `actions` at the end (the last one 4 dp in), the title
/// 16 dp in (or centered) in the middle of its 64 dp.
fn assert_small_bar(rule: &KompozTestRule, title: &str, centered: bool, actions: &[&str]) {
    let (rect, color) = bar(rule);
    assert!(near(rect.height(), 64.0) && color == light_color_scheme().surface, "{rect:?} {color:?}");
    assert!(button(rule, "Menu").center().x < 52.0);
    if let Some(last) = actions.last() {
        assert!(near_within(button(rule, last).center().x, 400.0 - 4.0 - 24.0, 1.0), "{:?}", button(rule, last));
    }
    for pair in actions.windows(2) {
        assert!(button(rule, pair[0]).right <= button(rule, pair[1]).left + 0.5);
    }
    let t = shown(rule, title);
    if centered {
        assert!(near_within(t.center().x, 200.0, 1.0), "{t:?}");
    } else {
        assert!(t.left >= button(rule, "Menu").right - 0.5 && t.left < 80.0, "{t:?}");
    }
}

/// Scrolls the sample's content up by `by` (a slow drag in the window's middle) and lets it settle.
fn scroll_up(rule: &KompozTestRule, by: f32) {
    slow_drag(rule, at(200.0, 600.0), at(200.0, 600.0 - by), 20);
    rule.wait_for_idle();
}

fn simple_top_app_bar() {
    let rule = KompozTestRule::new(SimpleTopAppBar);
    assert_small_bar(&rule, "Simple TopAppBar", false, &["Add to favorites"]);
    assert!(near(shown(&rule, "0").top, 64.0)); // the list under the bar (the scaffold's padding)
    scroll_up(&rule, 300.0);
    // No scroll behavior: the bar stays as it is.
    assert_eq!(bar(&rule), (Rect::new(0.0, 0.0, 400.0, 64.0), light_color_scheme().surface));
    rule.on_node_with_text("0").assert_does_not_exist();
}

/// Whether a node of `description` is placed (AppBarRow composes every item and its overflow indicator, placing
/// only those that show).
fn placed_with_description(rule: &KompozTestRule, description: &str) -> bool {
    rule.on_all_nodes_with_content_description(description).fetch_semantics_nodes().iter().any(|n| n.is_placed())
}

/// 400 dp wide (compact): three slots — M15's AppBarRow keeps the last of `maxItemCount` for the overflow
/// indicator while items are left over: Attachment, Edit and the indicator, whose menu holds Star, Snooze and Mark
/// unread. 700 dp (medium): all five, no indicator.
fn simple_top_app_bar_with_adaptive_actions() {
    let rule = KompozTestRule::new(SimpleTopAppBarWithAdaptiveActions);
    assert_small_bar(&rule, "Simple TopAppBar", false, &["Attachment", "Edit", "Overflow"]);
    for item in ["Star", "Snooze", "Mark unread"] {
        assert!(!placed_with_description(&rule, item), "{item}");
    }
    rule.on_node_with_content_description("Overflow").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Star").assert_exists();
    rule.on_node_with_text("Snooze").assert_exists();
    rule.on_node_with_text("Mark unread").assert_exists();
    let rule = KompozTestRule::with_viewport(SimpleTopAppBarWithAdaptiveActions, IntSize::new(700, 800));
    for item in ["Attachment", "Edit", "Star", "Snooze", "Mark unread"] {
        assert!(placed_with_description(&rule, item), "{item}");
    }
    assert!(!placed_with_description(&rule, "Overflow"));
}

fn simple_top_app_bar_with_subtitle() {
    let rule = KompozTestRule::new(SimpleTopAppBarWithSubtitle);
    assert_small_bar(&rule, "Simple TopAppBar", false, &["Add to favorites"]);
    let (title, subtitle) = (shown(&rule, "Simple TopAppBar"), shown(&rule, "Subtitle"));
    assert!(near(subtitle.top, title.bottom) && near(subtitle.left, title.left));
    // Pinned: it stays, turning to its scrolled color.
    scroll_up(&rule, 300.0);
    assert_eq!(bar(&rule), (Rect::new(0.0, 0.0, 400.0, 64.0), light_color_scheme().surface_container));
}

fn simple_center_aligned_top_app_bar() {
    let rule = KompozTestRule::new(SimpleCenterAlignedTopAppBar);
    assert_small_bar(&rule, "Centered TopAppBar", true, &["Add to favorites"]);
    scroll_up(&rule, 300.0);
    assert_eq!(bar(&rule).1, light_color_scheme().surface);
}

fn simple_center_aligned_top_app_bar_with_subtitle() {
    let rule = KompozTestRule::new(SimpleCenterAlignedTopAppBarWithSubtitle);
    assert_small_bar(&rule, "Simple TopAppBar", true, &["Add to favorites"]);
    let subtitle = shown(&rule, "Subtitle");
    assert!(
        near_within(subtitle.center().x, 200.0, 1.0) && near(subtitle.top, shown(&rule, "Simple TopAppBar").bottom)
    );
    scroll_up(&rule, 300.0);
    assert_eq!(bar(&rule).1, light_color_scheme().surface_container);
}

fn pinned_top_app_bar() {
    let rule = KompozTestRule::new(PinnedTopAppBar);
    assert_small_bar(&rule, "TopAppBar", false, &["Add to favorites", "Star"]);
    scroll_up(&rule, 300.0);
    assert_eq!(bar(&rule), (Rect::new(0.0, 0.0, 400.0, 64.0), light_color_scheme().surface_container));
    // Back at the top of the list: the container's color again.
    slow_drag(&rule, at(200.0, 200.0), at(200.0, 700.0), 20);
    rule.wait_for_idle();
    assert!(near(shown(&rule, "0").top, 64.0));
    assert_eq!(bar(&rule).1, light_color_scheme().surface);
}

fn pinned_top_app_bar_with_pre_scrolled_lazy_column() {
    let rule = KompozTestRule::new(PinnedTopAppBarWithPreScrolledLazyColumn);
    // Scrolled to its row 30 to begin with: the content is under the bar, which is in its scrolled color.
    rule.on_node_with_text("30").assert_exists();
    rule.on_node_with_text("0").assert_does_not_exist();
    assert_eq!(bar(&rule), (Rect::new(0.0, 0.0, 400.0, 64.0), light_color_scheme().surface_container));
}

fn pinned_top_app_bar_with_reversed_lazy_grid() {
    let rule = KompozTestRule::new(PinnedTopAppBarWithReversedLazyGrid);
    // Reversed: the first items at the bottom, four to a line (100 dp at the least), the rest above, under the bar:
    // its scrolled color.
    let first = shown(&rule, "0");
    assert!(near(first.top, 800.0 - 50.0) && near(shown(&rule, "3").left - first.left, 300.0), "{first:?}");
    assert_eq!(bar(&rule).1, light_color_scheme().surface_container);
    // Flung on to its end (down, as it is reversed): its top below the bar, which takes its container's color (the
    // fling left at the end resets what the behavior noted of the scrolls).
    swipe(&rule, at(200.0, 200.0), at(200.0, 700.0), 5);
    rule.wait_for_idle();
    rule.on_node_with_text("0").assert_does_not_exist();
    assert!(near(shown(&rule, "75").top, 64.0), "{:?}", shown(&rule, "75"));
    assert_eq!(bar(&rule).1, light_color_scheme().surface);
}

fn enter_always_top_app_bar() {
    let rule = KompozTestRule::new(EnterAlwaysTopAppBar);
    assert_small_bar(&rule, "TopAppBar", false, &["Add to favorites"]);
    // Up: the bar goes first, then the list.
    scroll_up(&rule, 300.0);
    assert!(near(bar(&rule).0.height(), 0.0), "{:?}", bar(&rule));
    rule.on_node_with_text("0").assert_does_not_exist();
    // Down a little: the bar comes back before the list moves.
    slow_drag(&rule, at(200.0, 300.0), at(200.0, 340.0), 20);
    assert!(near(bar(&rule).0.height(), 32.0), "{:?}", bar(&rule));
    rule.on_node_with_text("0").assert_does_not_exist();
}

fn enter_always_top_app_bar_with_reverse_scrolling() {
    let rule = KompozTestRule::new(EnterAlwaysTopAppBarWithReverseScrolling);
    // Scrolled reversed: its end shows at the bottom, its start above, under the bar: the bar's scrolled color.
    assert!(shown(&rule, "75").bottom <= 800.5 && shown(&rule, "75").bottom > 700.0);
    assert!(shown(&rule, "0").bottom < 0.0);
    assert_eq!(bar(&rule), (Rect::new(0.0, 0.0, 400.0, 64.0), light_color_scheme().surface_container));
    // Up: the bar leaves first.
    scroll_up(&rule, 300.0);
    assert!(near(bar(&rule).0.height(), 0.0), "{:?}", bar(&rule));
}

/// A two-rows sample: `expanded` tall with its title (`centered`), collapsing to 64 dp as the list scrolls up, and
/// expanding again once it is back at its top.
fn assert_two_rows(rule: &KompozTestRule, title: &str, expanded: f32, centered: bool) {
    let (rect, color) = bar(rule);
    assert!(near(rect.height(), expanded) && color == light_color_scheme().surface, "{rect:?}");
    let t = shown(rule, title);
    assert!(t.top >= 64.0 - 0.5, "the large title: {t:?}");
    if centered {
        assert!(near_within(t.center().x, 200.0, 1.0), "{t:?}");
    } else {
        assert!(near(t.left, 16.0), "{t:?}");
    }
    scroll_up(rule, 400.0);
    let (rect, color) = bar(rule);
    assert!(near(rect.height(), 64.0) && color == light_color_scheme().surface_container, "{rect:?}");
    assert!(shown(rule, title).bottom <= 64.5, "the small one's: {:?}", shown(rule, title));
    slow_drag(rule, at(200.0, 200.0), at(200.0, 700.0), 20);
    slow_drag(rule, at(200.0, 200.0), at(200.0, 700.0), 20);
    rule.wait_for_idle();
    assert!(near(bar(rule).0.height(), expanded), "{:?}", bar(rule));
}

fn exit_until_collapsed_medium_top_app_bar() {
    let rule = KompozTestRule::new(ExitUntilCollapsedMediumTopAppBar);
    assert_two_rows(&rule, "Medium TopAppBar", 112.0, false);
}

fn exit_until_collapsed_center_aligned_medium_flexible_top_app_bar() {
    let rule = KompozTestRule::new(ExitUntilCollapsedCenterAlignedMediumFlexibleTopAppBar);
    let subtitle = shown(&rule, "Subtitle");
    assert!(near_within(subtitle.center().x, 200.0, 1.0) && subtitle.top > shown(&rule, "Medium TopAppBar").top);
    assert_two_rows(&rule, "Medium TopAppBar", 136.0, true);
}

fn exit_until_collapsed_large_top_app_bar() {
    let rule = KompozTestRule::new(ExitUntilCollapsedLargeTopAppBar);
    assert_two_rows(&rule, "Large TopAppBar", 152.0, false);
}

fn exit_until_collapsed_center_aligned_large_flexible_top_app_bar() {
    let rule = KompozTestRule::new(ExitUntilCollapsedCenterAlignedLargeFlexibleTopAppBar);
    assert!(near_within(shown(&rule, "Subtitle").center().x, 200.0, 1.0));
    assert_two_rows(&rule, "Large TopAppBar", 152.0, true);
}

fn custom_two_rows_top_app_bar() {
    let rule = KompozTestRule::new(CustomTwoRowsTopAppBar);
    // 156 dp expanded: its own large title and subtitle (24 dp under it), the collapsed ones hidden.
    assert!(near(bar(&rule).0.height(), 156.0), "{:?}", bar(&rule));
    let subtitle = shown(&rule, "Expanded Subtitle");
    assert!(subtitle.top >= shown(&rule, "Expanded TopAppBar").bottom - 0.5 && subtitle.bottom <= 156.0 - 24.0 + 0.5);
    rule.on_node_with_text("Collapsed TopAppBar").assert_does_not_exist();
    let lorem = named_args!(rule.on_node_with_text("Lorem ipsum", substring = true)).get_unclipped_bounds_in_root();
    assert!(lorem.top >= 156.0 - 0.5, "{lorem:?}");
    // Collapsed to 64 dp as the text scrolls: the collapsed title and subtitle.
    scroll_up(&rule, 300.0);
    assert!(near(bar(&rule).0.height(), 64.0), "{:?}", bar(&rule));
    rule.on_node_with_text("Collapsed TopAppBar").assert_exists();
    rule.on_node_with_text("Collapsed Subtitle").assert_exists();
    rule.on_node_with_text("Expanded TopAppBar").assert_does_not_exist();
}

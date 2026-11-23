//! Material's carousels as Compose's material3 has them (package `androidx.compose.material3.carousel`): the C++
//! Kompoz's carousel tests by name (kompoz_tests.cpp; M15's values where the C++ differed, said where), Compose's
//! behaviour (the state and its saver, animated and immediate scrolls, the flings, the item scope's masks and draw
//! info, multi-aspect carousels of lazy rows and grids) and all M15 CarouselSamples. The keyline math (arrangements,
//! keyline lists, strategies, snap offsets) is checked by the crate's unit tests (src/carousel/tests.rs), as
//! Compose's own KeylineListTest and StrategyTest check its internals.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_coroutines::{CoroutineScope, remember_coroutine_scope};
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material3::prelude::*;
use kompoz_resources::{DrawableResource, StringResource, compose_resources, painter_resource, string_resource};
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_ui::back::BackHandler;
use kompoz_ui::input::PointerType;
use kompoz_ui::layout::GraphicsLayerScope;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::Role;
use kompoz_ui::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

tests!(
    // the C++ Kompoz's
    multi_browse_carousel_grows_and_shrinks_its_items_as_they_move,
    carousel_items_clip_and_border_their_masks,
    centered_hero_carousel_keeps_a_large_item_between_two_small_ones,
    carousel_items_know_their_size_while_they_are_drawn,
    uncontained_carousel_keeps_its_item_width_and_scrolls_freely,
    // Compose's behaviour
    carousel_state_is_saved_with_its_item_and_count,
    remember_carousel_state_takes_the_latest_item_count,
    animate_scroll_to_item_lands_where_scroll_to_item_puts_the_item,
    carousel_flings_advance_one_item_go_on_or_stop_anywhere,
    carousels_tell_accessibility_they_are_carousels,
    remember_mask_shape_follows_the_items_mask,
    carousels_mirror_right_to_left,
    multi_aspect_carousels_mask_and_move_lazy_row_items,
    multi_aspect_carousels_mask_lazy_grid_items,
    // M15's CarouselSamples
    horizontal_multi_browse_carousel_sample,
    horizontal_uncontained_carousel_sample,
    horizontal_centered_hero_carousel_sample,
    fading_horizontal_multi_browse_carousel_sample,
    carousel_with_show_all_button_sample,
    multi_aspect_carousel_lazy_row_sample,
);

const TouchSlop: f32 = 8.0;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn near_by(a: f32, b: f32, by: f32) -> bool {
    (a - b).abs() <= by
}

type Slot<T> = Rc<RefCell<Option<T>>>;

fn slot<T>() -> Slot<T> {
    Rc::new(RefCell::new(None))
}

fn get<T: Clone>(s: &Slot<T>) -> T {
    s.borrow().clone().expect("set by the composition")
}

fn launch(scope: &Slot<CoroutineScope>, f: impl std::future::Future<Output = ()> + 'static) {
    named_args!(get(scope).launch(f));
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// What each item's draw info said as it was last drawn: its mask (in the item), size, least and most size.
type Seen = Rc<RefCell<BTreeMap<i32, (Rect, f32, f32, f32)>>>;

/// An item filling its room, tagged `{prefix}{index}`, telling `seen` its draw info as it draws.
fn seen_item(prefix: &'static str, seen: Seen) -> impl Fn(&CarouselItemScope, i32) + 'static {
    move |scope: &CarouselItemScope, i: i32| {
        let info = scope.carousel_item_draw_info();
        let seen = seen.clone();
        kompozable_block!({
            Box(modifier = Modifier.fill_max_size().test_tag(format!("{prefix}{i}")).draw_behind(move |_| {
                seen.borrow_mut().insert(i, (info.mask_rect(), info.size(), info.min_size(), info.max_size()));
            }));
        });
    }
}

/// Each item's mask on the screen as `{x, width}`, left to right (the C++ tests' `masks()`, whose recording canvas
/// had its clips on the screen; KompozR's records a clip where its layer's transform doesn't reach: the item's
/// place on the screen, its layer's translation in, plus its mask in it): the ones in view, narrower than the window.
fn masks(rule: &KompozTestRule, prefix: &str, seen: &Seen) -> Vec<(f32, f32)> {
    seen.borrow_mut().clear();
    let _: RecordingCanvas = rule.draw();
    let mut out: Vec<(f32, f32)> = seen
        .borrow()
        .iter()
        .map(|(i, (mask, ..))| (bounds(rule, &format!("{prefix}{i}")).left + mask.left, mask.width()))
        .filter(|&(x, w)| w < 399.0 && x + w > 0.5 && x < 399.5)
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

fn is(got: &[(f32, f32)], want: &[(f32, f32)]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(g, w)| near(g.0, w.0) && near(g.1, w.1))
}

fn drag(rule: &KompozTestRule, from: Offset, to: Offset, frames: i32) {
    let h = rule.host();
    h.pointer_down(from, -1, 0, PointerType::Touch);
    for i in 1..=frames {
        rule.advance_frame();
        h.pointer_move(from + (to - from) * (i as f32 / frames as f32), -1, 0);
    }
    rule.advance_frame();
}

// ---- The C++ Kompoz's carousel tests ------------------------------------------------------------------------------

/// M15's multi-browse arrangement of 186 dp items in 400 is 187.2 large, 140.8 medium, 56 small (the C++'s 186, 142,
/// 56: Compose's arrangement flexes the medium item back towards the large size's target, 1.2 short here); after
/// `scrollToItem(9)` the current item is 9 (each item has a snap position of its own: the last ones snap where the end
/// steps put them; the C++ said 7).
fn multi_browse_carousel_grows_and_shrinks_its_items_as_they_move() {
    let (carousel, scope): (Slot<CarouselState>, Slot<CoroutineScope>) = (slot(), slot());
    let seen: Seen = Default::default();
    let (c, s, k) = (carousel.clone(), scope.clone(), seen.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_carousel_state(item_count = || 10);
            *c.borrow_mut() = Some(state.clone());
            *s.borrow_mut() = Some(remember_coroutine_scope());
            HorizontalMultiBrowseCarousel(
                state,
                186.dp(),
                modifier = Modifier.fill_max_width().height(200.dp()),
                item_spacing = 8.dp(),
                content = seen_item("c", k.clone()),
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    // 400 wide: a large one, a medium, a small, 8 apart.
    let start = masks(&rule, "c", &seen);
    assert!(is(&start, &[(0.0, 187.2), (195.2, 140.8), (344.0, 56.0)]), "{start:?}");
    // Its content keeps the large size (the item rounded to 187), cut around its middle.
    let medium = bounds(&rule, "c1");
    assert!(near(medium.width(), 187.0) && near(medium.left + medium.width() / 2.0, 195.2 + 70.4), "{medium:?}");
    // Dragged half an item along: the first one shrinks off the start, the medium grows.
    let to = Offset::new(300.0 - 97.0 - TouchSlop, 100.0);
    drag(&rule, Offset::new(300.0, 100.0), to, 10);
    let mid = masks(&rule, "c", &seen);
    assert!(mid.len() == 4 && near_by(mid[1].0, 97.0, 3.5) && near_by(mid[1].1, 164.0, 3.5), "{mid:?}");
    assert!(mid[0].0 < 0.0 && mid[0].1 < 186.0 && near(mid[0].0 + mid[0].1 + 8.0, mid[1].0), "side by side: {mid:?}");
    rule.host().pointer_up(to, -1, 0);
    rule.wait_for_idle(); // let go: it settles on an item
    let settled = masks(&rule, "c", &seen);
    assert_eq!(get(&carousel).current_item(), 1);
    assert!(is(&settled, &start), "{settled:?}");
    // At the end of the list it turns around: the last item large too.
    launch(&scope, get(&carousel).scroll_to_item(9));
    rule.wait_for_idle();
    let end = masks(&rule, "c", &seen);
    assert_eq!(get(&carousel).current_item(), 9);
    assert!(is(&end, &[(0.0, 56.0), (64.0, 140.8), (212.8, 187.2)]), "{end:?}");
}

/// The clip and the border follow the mask (a rounded rect 140.8 wide in the medium item, where the canvas records
/// them: the item's place before its layer's translation).
fn carousel_items_clip_and_border_their_masks() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let state = remember_carousel_state(item_count = || 10);
            HorizontalMultiBrowseCarousel(
                state,
                186.dp(),
                modifier = Modifier.fill_max_width().height(200.dp()),
                item_spacing = 8.dp(),
                content = |scope: &CarouselItemScope, i: i32| {
                    let shape: Arc<dyn Shape> = Arc::new(RoundedCornerShape(20.dp()));
                    let m = scope.mask_clip(Modifier.fill_max_size(), shape.clone());
                    let m = scope.mask_border(m, BorderStroke(2.dp(), Color::Red), shape);
                    Box(modifier = m.test_tag(format!("m{i}")));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let canvas = rule.draw();
    // The medium item (140.8 of its content's 187): clipped and bordered there, not at its content's bounds.
    let content = bounds(&rule, "m1");
    assert!(near(content.width(), 187.0), "{content:?}");
    let rounded = |p: &Path, b: Rect| p.contains(b.center()) && !p.contains(Offset::new(b.left + 2.0, b.top + 2.0));
    let clipped = canvas.clip_shapes.iter().any(|k| {
        let b = k.bounds;
        near(b.width(), 140.8) && near(b.height(), 200.0) && k.path.as_ref().is_some_and(|p| rounded(p, p.get_bounds()))
    });
    // (a generic outline's border is drawn into a bitmap of its bounds tinted the border's color, as Compose's)
    let bordered = canvas.mask_colors.contains(&Color::Red)
        && canvas.image_destinations.iter().any(|d| near_by(d.width(), 140.8, 2.0) && near_by(d.height(), 200.0, 2.0));
    assert!(clipped && bordered, "{clipped} {bordered} {:?} {:?}", canvas.mask_colors, canvas.image_destinations);
}

/// M15's hero arrangement with no item width cap is a large item the whole width less 40 dp small items (its large
/// target is the carousel's width, so the small items shrink to their least first): 304 and 40 where the C++ had 272
/// and 56. Capped at 200, M15 fits two large items of 148 between the small ones (Compose's TODO(b/420998605): its
/// arrangement has no most large size yet), where the C++ made one of 200. A page is 312 (the large item and the
/// spacing): dragged 0.6 of it back, held still and let go, it settles on the previous item. Two items: 352 and 40.
fn centered_hero_carousel_keeps_a_large_item_between_two_small_ones() {
    let (carousel, scope): (Slot<CarouselState>, Slot<CoroutineScope>) = (slot(), slot());
    let seen: Seen = Default::default();
    let cap = mutable_state_of(Dp::Unspecified);
    let (c, s, k, cp) = (carousel.clone(), scope.clone(), seen.clone(), cap.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_carousel_state(item_count = || 6);
            *c.borrow_mut() = Some(state.clone());
            *s.borrow_mut() = Some(remember_coroutine_scope());
            HorizontalCenteredHeroCarousel(
                state,
                modifier = Modifier.fill_max_width().height(200.dp()),
                max_item_width = cp.get(),
                item_spacing = 8.dp(),
                content = seen_item("h", k.clone()),
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    // At the start the first item is large at the start: 400 - 2 small (40) - 2 gaps = 304.
    let start = masks(&rule, "h", &seen);
    assert!(is(&start, &[(0.0, 304.0), (312.0, 40.0), (360.0, 40.0)]), "{start:?}");
    let small_one = bounds(&rule, "h1"); // its content keeps the large size, cut around its middle
    assert!(near(small_one.width(), 304.0) && near(small_one.left + small_one.width() / 2.0, 312.0 + 20.0));
    launch(&scope, get(&carousel).scroll_to_item(2)); // in the middle: large in the middle
    rule.wait_for_idle();
    let middle = masks(&rule, "h", &seen);
    assert_eq!(get(&carousel).current_item(), 2);
    assert!(is(&middle, &[(0.0, 40.0), (48.0, 304.0), (360.0, 40.0)]), "{middle:?}");
    // Dragged a little over half a page back: let go, it settles on the previous one.
    let to = Offset::new(100.0 + 312.0 * 0.6 + TouchSlop, 100.0);
    drag(&rule, Offset::new(100.0, 100.0), to, 10);
    let mid = masks(&rule, "h", &seen);
    assert!(
        mid.len() == 4 && mid[2].1 < 304.0 && mid[2].1 > 40.0 && mid[1].1 < 304.0 && mid[1].1 > 40.0,
        "two growing and shrinking: {mid:?}"
    );
    for _ in 0..10 {
        // held still: let go without a fling
        rule.advance_frame();
        rule.host().pointer_move(to, -1, 0);
    }
    rule.host().pointer_up(to, -1, 0);
    rule.wait_for_idle();
    let back = masks(&rule, "h", &seen);
    assert_eq!(get(&carousel).current_item(), 1);
    assert!(is(&back, &middle), "{back:?}");
    launch(&scope, get(&carousel).scroll_to_item(5)); // at the end the last one is large at the end
    rule.wait_for_idle();
    let end = masks(&rule, "h", &seen);
    assert_eq!(get(&carousel).current_item(), 5);
    assert!(is(&end, &[(0.0, 40.0), (48.0, 40.0), (96.0, 304.0)]), "{end:?}");
    cap.set(200.dp()); // capped: two large ones of what is left
    launch(&scope, get(&carousel).scroll_to_item(3));
    rule.wait_for_idle();
    let capped = masks(&rule, "h", &seen);
    assert!(is(&capped, &[(0.0, 40.0), (48.0, 148.0), (204.0, 148.0), (360.0, 40.0)]), "{capped:?}");

    // Two items: the first large at the start, then the second at the end.
    let seen2: Seen = Default::default();
    let k2 = seen2.clone();
    let two = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_carousel_state(item_count = || 2);
            HorizontalCenteredHeroCarousel(
                state,
                modifier = Modifier.fill_max_width().height(200.dp()),
                item_spacing = 8.dp(),
                content = seen_item("p", k2.clone()),
            );
        }),
        IntSize::new(400, 800),
    );
    two.wait_for_idle();
    let pair = masks(&two, "p", &seen2);
    assert!(is(&pair, &[(0.0, 352.0), (360.0, 40.0)]), "{pair:?}");
    // Dragged all the way: the second one goes smoothly to the end.
    let h = two.host();
    h.pointer_down(Offset::new(380.0, 100.0), -1, 0, PointerType::Touch);
    let (mut leap, mut last_x) = (0.0f32, 360.0f32);
    for i in 1..=30 {
        two.advance_frame();
        h.pointer_move(Offset::new(380.0 - 12.0 * i as f32, 100.0), -1, 0);
        let now = masks(&two, "p", &seen2);
        if let Some(&(x, _)) = now.last() {
            leap = leap.max((x - last_x).abs());
            last_x = x;
        }
    }
    h.pointer_up(Offset::new(20.0, 100.0), -1, 0);
    two.wait_for_idle();
    let pair_end = masks(&two, "p", &seen2);
    assert!(leap < 30.0 && is(&pair_end, &[(0.0, 40.0), (48.0, 352.0)]), "{leap} {pair_end:?}");
}

/// M15's least size is of the rounded keylines, the anchors' 10 dp in (the C++ left them out: 56); the hero's sizes
/// as M15 arranges them (see above).
fn carousel_items_know_their_size_while_they_are_drawn() {
    let seen: Seen = Default::default();
    let k = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_carousel_state(item_count = || 6);
            HorizontalCenteredHeroCarousel(
                state,
                modifier = Modifier.fill_max_width().height(200.dp()),
                item_spacing = 8.dp(),
                content = seen_item("h", k.clone()),
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let drawn = |rule: &KompozTestRule| {
        seen.borrow_mut().clear();
        rule.draw();
        seen.borrow().clone()
    };
    let items = drawn(&rule);
    let (i0, i1) = (items[&0], items[&1]);
    assert!(near(i0.1, 304.0) && near(i0.2, 10.0) && near(i0.3, 304.0), "large, of 10 .. 304: {i0:?}");
    assert!(near(i1.1, 40.0) && near(i1.3, 304.0), "{i1:?}");
    // Dragged a little: the sizes follow without recomposing.
    let to = Offset::new(300.0 - 40.0 - TouchSlop, 100.0);
    drag(&rule, Offset::new(300.0, 100.0), to, 5);
    let items = drawn(&rule);
    let (i0, i1) = (items[&0], items[&1]);
    assert!(i0.1 < 303.0 && i0.1 > 41.0 && i1.1 > 41.0 && i1.1 < 303.0, "{i0:?} {i1:?}");
    rule.host().pointer_up(to, -1, 0);
    rule.wait_for_idle();
    // The others report too (multi-browse: 187.2 large, 56 small).
    let seen2: Seen = Default::default();
    let k2 = seen2.clone();
    let browse = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_carousel_state(item_count = || 10);
            HorizontalMultiBrowseCarousel(
                state,
                186.dp(),
                modifier = Modifier.fill_max_width().height(200.dp()),
                item_spacing = 8.dp(),
                content = seen_item("b", k2.clone()),
            );
        }),
        IntSize::new(400, 800),
    );
    browse.wait_for_idle();
    browse.draw();
    let b = seen2.borrow();
    assert!(
        near(b[&0].1, 187.2)
            && near(b[&1].1, 140.8)
            && near(b[&2].1, 56.0)
            && near(b[&2].2, 10.0)
            && near(b[&2].3, 187.2),
        "{b:?}"
    );
}

/// M15's uncontained items are the item width and the spacing wide (Compose's `largeItemSize = itemSize +
/// itemSpacing`, the pager's spacing besides: 158 here, the C++ 150), the focal ones shrunk by the start padding's
/// share at the start (152.67 from 16); no snapping: where it was let go it stays; at its end the last mask's end is
/// the padding's from the window's end.
fn uncontained_carousel_keeps_its_item_width_and_scrolls_freely() {
    let (carousel, scope): (Slot<CarouselState>, Slot<CoroutineScope>) = (slot(), slot());
    let seen: Seen = Default::default();
    let (c, s, k) = (carousel.clone(), scope.clone(), seen.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_carousel_state(item_count = || 10);
            *c.borrow_mut() = Some(state.clone());
            *s.borrow_mut() = Some(remember_coroutine_scope());
            HorizontalUncontainedCarousel(
                state,
                150.dp(),
                modifier = Modifier.fill_max_width().height(120.dp()),
                item_spacing = 8.dp(),
                content_padding = PaddingValues::new(16.dp(), 0.dp(), 16.dp(), 0.dp()),
                content = seen_item("u", k.clone()),
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "u0").width(), 158.0) && near(bounds(&rule, "u1").width(), 158.0));
    let start = masks(&rule, "u", &seen);
    assert!(is(&start, &[(16.0, 152.67), (176.67, 152.67), (337.33, 120.67)]), "{start:?}");
    let h = rule.host();
    h.pointer_down(Offset::new(300.0, 60.0), -1, 0, PointerType::Touch);
    for i in 1..=30 {
        rule.advance_frame();
        h.pointer_move(Offset::new(300.0 - 5.0 * i as f32, 60.0), -1, 0);
    }
    for _ in 0..20 {
        rule.advance_frame(); // held still: no fling
    }
    let let_go = masks(&rule, "u", &seen);
    h.pointer_up(Offset::new(150.0, 60.0), -1, 0);
    rule.wait_for_idle();
    let after = masks(&rule, "u", &seen);
    assert!(is(&after, &let_go) && get(&carousel).current_item() == 1, "no snapping: {let_go:?} {after:?}");
    launch(&scope, get(&carousel).scroll_to_item(100)); // to its end
    rule.wait_for_idle();
    let end = masks(&rule, "u", &seen);
    let last = end.last().copied().unwrap();
    assert!(near_by(last.0 + last.1, 400.0 - 16.0, 1.0), "{end:?}");
}

// ---- Compose's behaviour --------------------------------------------------------------------------------------------

/// A multi-browse carousel of 10 items 186 dp wide in 400, tagged `{prefix}{i}`.
fn browse_rule(
    prefix: &'static str,
    carousel: &Slot<CarouselState>,
    scope: &Slot<CoroutineScope>,
    seen: &Seen,
    fling: Option<fn(&CarouselState) -> Rc<dyn TargetedFlingBehavior>>,
) -> KompozTestRule {
    let (c, s, k) = (carousel.clone(), scope.clone(), seen.clone());
    KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let state = remember_carousel_state(item_count = || 10);
            *c.borrow_mut() = Some(state.clone());
            *s.borrow_mut() = Some(remember_coroutine_scope());
            let fling_behavior = fling.map(|f| f(&state));
            HorizontalMultiBrowseCarousel(
                state,
                186.dp(),
                modifier = Modifier.fill_max_width().height(200.dp()),
                item_spacing = 8.dp(),
                fling_behavior = fling_behavior,
                content = seen_item(prefix, k.clone()),
            );
        }),
        IntSize::new(400, 800),
    )
}

/// `CarouselState.Saver` keeps the current item, its offset fraction and the item count; a state made by
/// `CarouselState(…)` is a scrollable state of its pager (not scrolling at rest).
fn carousel_state_is_saved_with_its_item_and_count() {
    let state = CarouselState(3, 0.25, || 7);
    assert!(!state.is_scroll_in_progress() && state.current_item() == 3);
    let saver = CarouselState::saver();
    let restored = saver.restore(&saver.save(&state)).expect("restored");
    assert_eq!(restored.current_item(), 3);
    assert_ne!(restored, state);
    assert_eq!(restored, restored.clone());
    let again = saver.save(&restored);
    assert_eq!(again, saver.save(&state));
    // The defaults (Kotlin's `CarouselState(itemCount = { 4 })`).
    let plain = CarouselState::new(0, 0.0, Rc::new(|| 4));
    assert_eq!(plain.current_item(), 0);
}

/// `rememberCarouselState` takes the latest item count function: more items come in.
fn remember_carousel_state_takes_the_latest_item_count() {
    let count = mutable_state_of(2);
    let seen: Seen = Default::default();
    let (n, k) = (count.clone(), seen.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let n = n.get();
            let state = remember_carousel_state(item_count = move || n);
            HorizontalMultiBrowseCarousel(
                state,
                186.dp(),
                modifier = Modifier.fill_max_width().height(200.dp()),
                item_spacing = 8.dp(),
                content = seen_item("n", k.clone()),
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    assert!(!rule.on_node_with_tag("n2").exists());
    count.set(5);
    rule.wait_for_idle();
    assert!(rule.on_node_with_tag("n2").exists());
    let now = masks(&rule, "n", &seen);
    assert!(is(&now, &[(0.0, 187.2), (195.2, 140.8), (344.0, 56.0)]), "{now:?}");
}

/// `animateScrollToItem` glides to an item's own snap position (far: jumping close first), as `scrollToItem` puts it;
/// to where it is, nothing.
fn animate_scroll_to_item_lands_where_scroll_to_item_puts_the_item() {
    let (carousel, scope): (Slot<CarouselState>, Slot<CoroutineScope>) = (slot(), slot());
    let seen: Seen = Default::default();
    let rule = browse_rule("a", &carousel, &scope, &seen, None);
    rule.wait_for_idle();
    let start = masks(&rule, "a", &seen);
    launch(&scope, named_args!(get(&carousel).animate_scroll_to_item(9)));
    rule.advance_frame();
    rule.advance_frame();
    assert!(get(&carousel).is_scroll_in_progress());
    rule.wait_for_idle();
    // (at the end as `scrollToItem(9)` puts it, within a pixel: the distance is of whole-pixel snap offsets and a
    // 187-pixel page where the keylines' item is 187.2, as Compose's; items 8 and 9 are then both within a pixel of
    // their own snap positions, and the one closer by that fraction is the current one — 8 here)
    let end = masks(&rule, "a", &seen);
    assert!(get(&carousel).current_item() >= 8, "{}", get(&carousel).current_item());
    assert!(is(&end, &[(0.0, 56.0), (64.0, 140.8), (212.8, 187.2)]), "{end:?}");
    let current = get(&carousel).current_item();
    launch(&scope, named_args!(get(&carousel).animate_scroll_to_item(current)));
    rule.advance_frame();
    assert!(!get(&carousel).is_scroll_in_progress());
    let spec: std::sync::Arc<dyn kompoz_animation::AnimationSpec<f32>> =
        std::sync::Arc::new(kompoz_animation::TweenSpec::new(300, 0, kompoz_animation::LinearEasing));
    launch(&scope, named_args!(get(&carousel).animate_scroll_to_item(0, animation_spec = spec)));
    rule.wait_for_idle();
    let back = masks(&rule, "a", &seen);
    assert!(get(&carousel).current_item() == 0 && is(&back, &start), "{back:?}");
}

/// A fast fling moves one item at most with the default single-advance fling; the multi-browse fling goes as far as
/// the velocity carries and snaps; the no-snap fling (the uncontained carousel's default) stops where it ends.
fn carousel_flings_advance_one_item_go_on_or_stop_anywhere() {
    let fast = |rule: &KompozTestRule| {
        let h = rule.host();
        // (fast, but short of half a page: the fling starts on the first item)
        h.pointer_down(Offset::new(350.0, 100.0), -1, 0, PointerType::Touch);
        for i in 1..=3 {
            rule.advance_frame();
            h.pointer_move(Offset::new(350.0 - 40.0 * i as f32, 100.0), -1, 0);
        }
        h.pointer_up(Offset::new(230.0, 100.0), -1, 0);
        rule.wait_for_idle();
    };
    let (carousel, scope): (Slot<CarouselState>, Slot<CoroutineScope>) = (slot(), slot());
    let seen: Seen = Default::default();
    let single = browse_rule("s", &carousel, &scope, &seen, None);
    single.wait_for_idle();
    let start = masks(&single, "s", &seen);
    fast(&single);
    assert_eq!(get(&carousel).current_item(), 1);
    assert!(is(&masks(&single, "s", &seen), &start));

    let multi = browse_rule(
        "m",
        &carousel,
        &scope,
        &seen,
        Some(|s: &CarouselState| CarouselDefaults.multi_browse_fling_behavior(s)),
    );
    multi.wait_for_idle();
    fast(&multi);
    let item = get(&carousel).current_item();
    assert!(item > 1, "{item}");
    // (settled on an item: its masks are some item's resting ones)
    let rest = masks(&multi, "m", &seen);
    assert!(rest.iter().any(|m| near(m.1, 187.2)), "{rest:?}");

    let free =
        browse_rule("f", &carousel, &scope, &seen, Some(|_: &CarouselState| CarouselDefaults.no_snap_fling_behavior()));
    free.wait_for_idle();
    fast(&free);
    let stopped = masks(&free, "f", &seen);
    assert!(!stopped.iter().any(|m| near(m.1, 187.2)) || get(&carousel).current_item() > 1, "{stopped:?}");
}

/// A carousel tells accessibility it is one (Compose's `Role.Carousel` on the pager).
fn carousels_tell_accessibility_they_are_carousels() {
    let (carousel, scope): (Slot<CarouselState>, Slot<CoroutineScope>) = (slot(), slot());
    let seen: Seen = Default::default();
    let rule = browse_rule("r", &carousel, &scope, &seen, None);
    rule.wait_for_idle();
    rule.on_node(kompoz_test::has_role(kompoz_ui::semantics::Role::Carousel)).assert_exists();
}

/// `rememberMaskShape` gives a shape following the item's mask, for modifiers of one's own (a background here).
fn remember_mask_shape_follows_the_items_mask() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let state = remember_carousel_state(item_count = || 10);
            HorizontalMultiBrowseCarousel(
                state,
                186.dp(),
                modifier = Modifier.fill_max_width().height(200.dp()),
                item_spacing = 8.dp(),
                content = |scope: &CarouselItemScope, i: i32| {
                    let shape = scope.remember_mask_shape(Arc::new(RoundedCornerShape(12.dp())));
                    let color = if i == 1 { Color::Blue } else { Color::Green };
                    Box(modifier = Modifier.fill_max_size().background(color, Arc::new(shape)));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let canvas = rule.draw();
    let blue: Vec<Rect> = canvas
        .path_colors
        .iter()
        .zip(&canvas.path_shapes)
        .filter(|(c, _)| **c == Color::Blue)
        .map(|(_, p)| p.get_bounds())
        .collect();
    assert!(blue.len() == 1 && near(blue[0].width(), 140.8) && near(blue[0].left, 23.1), "{blue:?}");
}

/// Right to left the carousel starts at the right: the first item large there, the others leftwards.
fn carousels_mirror_right_to_left() {
    let seen: Seen = Default::default();
    let k = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let k = k.clone();
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
                let state = remember_carousel_state(item_count = || 10);
                HorizontalMultiBrowseCarousel(
                    state,
                    186.dp(),
                    modifier = Modifier.fill_max_width().height(200.dp()),
                    item_spacing = 8.dp(),
                    content = seen_item("l", k.clone()),
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    // (the items 187 wide, mirrored: their masks 0.2 to 0.6 from the left-to-right ones)
    let now = masks(&rule, "l", &seen);
    let want = [(0.0, 56.0), (64.0, 140.8), (212.8, 187.2)];
    assert!(now.len() == 3 && now.iter().zip(want).all(|(g, w)| near_by(g.0, w.0, 1.0) && near(g.1, w.1)), "{now:?}");
    let first = bounds(&rule, "l0");
    assert!(near(first.right, 400.0), "{first:?}");
}

/// A multi-aspect carousel of a lazy row: items keep their sizes; one leaving the start is masked from its start and
/// its content moved towards the end (up to its size × its mask intensity: ⅓ to ½ for aspect ratios from 1 to 16:9,
/// ¼ to ⅓ from 9:16 to 1), one entering at the end masked from its end and moved towards the start; the draw infos
/// say so, and the clip, the parallax and the border follow.
fn multi_aspect_carousels_mask_and_move_lazy_row_items() {
    let (list, infos): (Slot<LazyListState>, Rc<RefCell<Vec<MultiAspectCarouselItemDrawInfo>>>) =
        (slot(), Default::default());
    let scope: Slot<CoroutineScope> = slot();
    let (l, inf, s) = (list.clone(), infos.clone(), scope.clone());
    let widths = [300.0f32, 200.0, 100.0, 300.0];
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (l, inf, s) = (l.clone(), inf.clone(), s.clone());
            MultiAspectCarouselScope(move |carousel: &MultiAspectCarouselScope| {
                let state = remember_lazy_list_state();
                *l.borrow_mut() = Some(state.clone());
                *s.borrow_mut() = Some(remember_coroutine_scope());
                let (carousel, inf) = (carousel.clone(), inf.clone());
                LazyRow(
                    state = state.clone(),
                    horizontal_arrangement = Arrangement::spaced_by(8.dp()),
                    modifier = Modifier.fill_max_width().height(200.dp()),
                    content = move |this: &mut LazyListScope| {
                        let (carousel, state, inf) = (carousel.clone(), state.clone(), inf.clone());
                        this.items(
                            4,
                            item_content = move |i| {
                                let draw_info = remember(|| MultiAspectCarouselItemDrawInfo(i, state.clone()));
                                if i < 2 {
                                    let mut v = inf.borrow_mut();
                                    if v.len() <= i as usize {
                                        v.push(draw_info.clone());
                                    }
                                }
                                let shape: Arc<dyn Shape> = Arc::new(RoundedCornerShape(16.dp()));
                                let m = carousel.mask_border(
                                    Modifier.width(widths[i as usize].dp()).height(200.dp()),
                                    BorderStroke(2.dp(), Color::Red),
                                    shape.clone(),
                                    draw_info.clone(),
                                );
                                let m = carousel.mask_clip(m, shape, draw_info);
                                Box(modifier = m.background(Color::Blue).test_tag(format!("ma{i}")));
                            },
                        );
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let (first, second) = (infos.borrow()[0].clone(), infos.borrow()[1].clone());
    // At rest: the first in full; the second entering at the end (308 + 200 past 400: 108 out).
    assert!(first.index() == 0 && first.is_horizontal() && near(first.size(), 300.0) && near(first.parallax(), 0.0));
    let intensity = 1.0 / 3.0 + (1.5 - 1.0) / (16.0 / 9.0 - 1.0) * (1.0 / 2.0 - 1.0 / 3.0);
    assert!(near(first.min_size(), 300.0 * (1.0 - intensity)) && near(first.max_size(), 300.0), "{first:?}");
    assert!(near(second.mask_start(), 0.0) && near(second.mask_end(), 200.0 - 150.0 * 108.0 / 200.0), "{second:?}");
    assert!(near(second.parallax(), -50.0 * 108.0 / 200.0), "{second:?}");
    // Scrolled 150 along: the first half out at the start.
    launch(&scope, named_args!(get(&list).scroll_to_item(0, scroll_offset = 150)));
    rule.wait_for_idle();
    let mask_start = 300.0 * (1.0 - intensity) * 0.5;
    assert!(near(first.mask_start(), mask_start) && near(first.mask_end(), 300.0), "{first:?}");
    assert!(near(first.size(), 300.0 - mask_start) && near(first.parallax(), 300.0 * intensity * 0.5), "{first:?}");
    let canvas = rule.draw();
    let clipped = canvas.clip_shapes.iter().any(|k| near(k.bounds.width(), 300.0 - mask_start));
    let moved = canvas.translations.iter().any(|t| near(t.x, 300.0 * intensity * 0.5));
    let bordered = canvas.image_destinations.iter().any(|d| near_by(d.width(), 300.0 - mask_start, 2.0));
    assert!(clipped && moved && bordered, "{clipped} {moved} {bordered}");
    // Equal by what they say now; their Debug as Compose's toString.
    assert_eq!(first, first.clone());
    assert_ne!(first, second);
    assert!(format!("{first:?}").starts_with("MultiAspectCarouselItemDrawInfoImpl(size="));
}

/// A multi-aspect carousel of a lazy grid: its main axis the grid's (vertical here), an item leaving the top masked
/// from its top; an item out of view says nothing (0).
fn multi_aspect_carousels_mask_lazy_grid_items() {
    let (grid, info, far): (
        Slot<LazyGridState>,
        Slot<MultiAspectCarouselItemDrawInfo>,
        Slot<MultiAspectCarouselItemDrawInfo>,
    ) = (slot(), slot(), slot());
    let scope: Slot<CoroutineScope> = slot();
    let (g, inf, fr, s) = (grid.clone(), info.clone(), far.clone(), scope.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (g, inf, fr, s) = (g.clone(), inf.clone(), fr.clone(), s.clone());
            MultiAspectCarouselScope(move |_: &MultiAspectCarouselScope| {
                let state = remember_lazy_grid_state();
                *g.borrow_mut() = Some(state.clone());
                *s.borrow_mut() = Some(remember_coroutine_scope());
                *inf.borrow_mut() = Some(GridMultiAspectCarouselItemDrawInfo(0, state.clone()));
                *fr.borrow_mut() = Some(GridMultiAspectCarouselItemDrawInfo(40, state.clone()));
                LazyVerticalGrid(
                    GridCells::Fixed(2),
                    state = state,
                    modifier = Modifier.size_wh(400.dp(), 400.dp()),
                    content = |this: &mut LazyGridScope| {
                        this.items(50, item_content = |_| Box(modifier = Modifier.height(100.dp())));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let (first, far) = (get(&info), get(&far));
    assert!(!first.is_horizontal() && near(first.max_size(), 100.0) && near(first.mask_end(), 100.0));
    assert!(near(far.size(), 0.0) && near(far.min_size(), 0.0) && near(far.parallax(), 0.0));
    launch(&scope, named_args!(get(&grid).scroll_to_item(0, scroll_offset = 50)));
    rule.wait_for_idle();
    // (100 along, 200 across: under 9:16, a quarter intensity: masked up to 75, moved up to 25, when out by its
    // height; half out here)
    assert!(near(first.mask_start(), 37.5) && near(first.parallax(), 12.5), "{first:?}");
}

// ---- M15's CarouselSamples.kt ---------------------------------------------------------------------------------------
// Their images are Android resources (`painterResource(R.drawable.carousel_image_N)`, `stringResource(…)`): KompozR's
// (Compose Multiplatform's) here, tests/composeResources' `Res` (its images 3:2 ones of a color of their own, as the
// samples' photos are images the items crop; its descriptions "Carousel image N"). Their local `data class
// CarouselItem` is a struct here.

compose_resources!(dir = "tests/composeResources");

/// The samples' `R.drawable.carousel_image_1` … `_5`, `R.string.carousel_image_1_description` … `_5`.
fn carousel_images() -> [(DrawableResource, StringResource); 5] {
    [
        (Res::drawable::carousel_image_1(), Res::string::carousel_image_1_description()),
        (Res::drawable::carousel_image_2(), Res::string::carousel_image_2_description()),
        (Res::drawable::carousel_image_3(), Res::string::carousel_image_3_description()),
        (Res::drawable::carousel_image_4(), Res::string::carousel_image_4_description()),
        (Res::drawable::carousel_image_5(), Res::string::carousel_image_5_description()),
    ]
}

/// Image `i`'s description, as its string resource says it.
fn description(i: i32) -> String {
    format!("Carousel image {i}")
}

#[derive(Clone, PartialEq, Debug)]
struct CarouselItem {
    id: i32,
    image_res_id: DrawableResource,
    content_description_res_id: StringResource,
}

fn sample_items() -> Vec<CarouselItem> {
    carousel_images()
        .into_iter()
        .enumerate()
        .map(|(i, (image, text))| CarouselItem { id: i as i32, image_res_id: image, content_description_res_id: text })
        .collect()
}

/// The samples' `MaterialTheme.shapes.extraLarge`.
fn extra_large() -> Arc<dyn Shape> {
    Arc::new(MaterialTheme::shapes().extra_large.clone())
}

fn description_bounds(rule: &KompozTestRule, i: i32) -> Rect {
    rule.on_node_with_content_description(&description(i)).get_unclipped_bounds_in_root()
}

#[kompozable]
fn HorizontalMultiBrowseCarouselSample() {
    let items = sample_items();
    let state = remember_carousel_state(item_count = move || 5);
    let animation_scope = remember_coroutine_scope();
    let s = state.clone();
    HorizontalMultiBrowseCarousel(
        state,
        186.dp(),
        modifier = Modifier.fill_max_width().height(221.dp()),
        item_spacing = 8.dp(),
        content_padding = PaddingValues::hv(16.dp(), 0.dp()),
        content = move |scope: &CarouselItemScope, i: i32| {
            let item = items[i as usize].clone();
            let (s, animation_scope) = (s.clone(), animation_scope.clone());
            let modifier = Modifier.height(205.dp()).fill_max_width().clickable(
                true,
                Some("Tap to focus".into()),
                Some(Role::Image),
                None,
                move || {
                    named_args!(animation_scope.launch(named_args!(s.animate_scroll_to_item(i))));
                },
            );
            kompozable_block!({
                Image(
                    modifier = scope.mask_clip(modifier, extra_large()),
                    painter = painter_resource(item.image_res_id.clone()),
                    content_description = Some(string_resource(item.content_description_res_id.clone())),
                    content_scale = ContentScale::Crop,
                );
            });
        },
    );
}

#[kompozable]
fn HorizontalUncontainedCarouselSample() {
    let items = sample_items();
    HorizontalUncontainedCarousel(
        remember_carousel_state(item_count = move || 5),
        186.dp(),
        modifier = Modifier.width(412.dp()).height(221.dp()),
        item_spacing = 8.dp(),
        content_padding = PaddingValues::hv(16.dp(), 0.dp()),
        content = move |scope: &CarouselItemScope, i: i32| {
            let item = items[i as usize].clone();
            kompozable_block!({
                Image(
                    modifier = scope.mask_clip(Modifier.height(205.dp()), extra_large()),
                    painter = painter_resource(item.image_res_id.clone()),
                    content_description = Some(string_resource(item.content_description_res_id.clone())),
                    content_scale = ContentScale::Crop,
                );
            });
        },
    );
}

#[kompozable]
fn HorizontalCenteredHeroCarouselSample() {
    let items = sample_items();
    let state = remember_carousel_state(item_count = move || 5);
    let animation_scope = remember_coroutine_scope();
    let s = state.clone();
    HorizontalCenteredHeroCarousel(
        state,
        modifier = Modifier.fill_max_width().height(221.dp()).padding_hv(horizontal = 24.dp()),
        item_spacing = 8.dp(),
        content_padding = PaddingValues::hv(16.dp(), 0.dp()),
        content = move |scope: &CarouselItemScope, i: i32| {
            let item = items[i as usize].clone();
            let (s, animation_scope) = (s.clone(), animation_scope.clone());
            let clipped = scope.mask_clip(Modifier.fill_max_width().height(205.dp()), extra_large());
            kompozable_block!({
                Image(
                    modifier =
                        clipped.clickable(true, Some("Tap to focus".into()), Some(Role::Image), None, move || {
                            named_args!(animation_scope.launch(named_args!(s.animate_scroll_to_item(i))));
                        }),
                    painter = painter_resource(item.image_res_id.clone()),
                    content_description = Some(string_resource(item.content_description_res_id.clone())),
                    content_scale = ContentScale::Crop,
                );
            });
        },
    );
}

#[kompozable]
// (its item content an `if` of two layouts in a composable block, which clippy takes for a value left unused)
#[allow(clippy::unnecessary_operation)]
fn FadingHorizontalMultiBrowseCarouselSample() {
    let items = sample_items();
    let state = remember_carousel_state(item_count = move || 5);
    HorizontalMultiBrowseCarousel(
        state,
        186.dp(),
        modifier = Modifier.width(412.dp()).height(221.dp()),
        item_spacing = 8.dp(),
        content_padding = PaddingValues::hv(16.dp(), 0.dp()),
        content = move |scope: &CarouselItemScope, i: i32| {
            let item = items[i as usize].clone();
            let scope = scope.clone();
            kompozable_block!({
                // For item 1 and 4, a stacked item layout clipping two images on their own to the item's mask.
                if i == 1 || i == 4 {
                    let scope = scope.clone();
                    Column(
                        modifier = Modifier.height(205.dp()),
                        vertical_arrangement = Arrangement::spaced_by(8.dp()),
                        content = move || {
                            let large = extra_large();
                            let m = scope.mask_clip(Modifier.fill_max_width().fill_max_height(0.5), large.clone());
                            let m = scope.mask_border(m, BorderStroke(3.dp(), Color::Magenta), large);
                            Image(
                                modifier = m,
                                painter = painter_resource(item.image_res_id.clone()),
                                content_description = Some(string_resource(item.content_description_res_id.clone())),
                                content_scale = ContentScale::Crop,
                            );
                            let small: Arc<dyn Shape> = Arc::new(RoundedCornerShape(8.dp()));
                            let m = scope.mask_clip(Modifier.fill_max_size(), small.clone());
                            let m = scope.mask_border(m, BorderStroke(5.dp(), Color::Green), small);
                            Image(
                                modifier = m,
                                painter = painter_resource(item.image_res_id.clone()),
                                content_description = Some(string_resource(item.content_description_res_id.clone())),
                                content_scale = ContentScale::Crop,
                            );
                        },
                    );
                } else {
                    // Masked by a generic path's shape.
                    let path_shape: Arc<dyn Shape> = remember(|| -> Arc<dyn Shape> {
                        Arc::new(OutlineShape(|size: Size, _: LayoutDirection, _: &Density| {
                            let round_rect = RoundRect::from_rect_radius(
                                Rect::new(0.0, 0.0, size.width, size.height),
                                CornerRadius::new(30.0, 30.0),
                            );
                            let mut shape_path = Path::new();
                            shape_path.add_round_rect(&round_rect, PathDirection::CounterClockwise);
                            Outline::Generic(shape_path)
                        }))
                    });
                    let m = scope.mask_clip(Modifier.height(205.dp()), path_shape.clone());
                    let m = scope.mask_border(m, BorderStroke(5.dp(), Color::Red), path_shape);
                    let info = scope.carousel_item_draw_info();
                    Box(
                        modifier = m,
                        content = move || {
                            Image(
                                painter = painter_resource(item.image_res_id.clone()),
                                content_description = Some(string_resource(item.content_description_res_id.clone())),
                                modifier = Modifier.fill_max_size(),
                                content_scale = ContentScale::Crop,
                            );
                            let info = info.clone();
                            ElevatedAssistChip(
                                on_click = || { /* Do something! */ },
                                label = move || Text(format!("Image {i}")),
                                modifier = Modifier.graphics_layer_block(move |layer: &mut GraphicsLayerScope| {
                                    // Faded in once the item is large enough to show the whole chip.
                                    let width = layer.size().width;
                                    layer.alpha =
                                        lerp(0.0, 1.0, (width - info.max_size() + info.size()).max(0.0) / width);
                                    // Pinned to the start of the item's mask.
                                    layer.translation_x = info.mask_rect().left + layer.density().to_px(8.dp());
                                }),
                                leading_icon = Some(Rc::new(|| {
                                    VectorIcon(
                                        kompoz_material_icons_extended::Icons::Filled::Image(),
                                        content_description = Some("Localized description".into()),
                                        modifier = Modifier.size(AssistChipDefaults::IconSize),
                                    );
                                })),
                            );
                        },
                    );
                }
            });
        },
    );
}

#[kompozable]
fn CarouselWithShowAllButtonSample() {
    let items = sample_items();
    let show_all_items = remember(|| mutable_state_of(false));
    let s = show_all_items.clone();
    BackHandler(enabled = show_all_items.get(), on_back = move || s.set(false));
    if show_all_items.get() {
        // The grid page itself (a dedicated screen would be better navigation).
        LazyVerticalGrid(
            GridCells::Fixed(2),
            content_padding = PaddingValues::all(16.dp()),
            vertical_arrangement = Arrangement::spaced_by(8.dp()),
            horizontal_arrangement = Arrangement::spaced_by(8.dp()),
            modifier = Modifier.fill_max_size().padding_sides(top = 8.dp()),
            content = move |this: &mut LazyGridScope| {
                let items = items.clone();
                this.items(
                    items.len() as i32,
                    item_content = move |i| {
                        let item = items[i as usize].clone();
                        Image(
                            painter = painter_resource(item.image_res_id.clone()),
                            content_description = Some(string_resource(item.content_description_res_id.clone())),
                            modifier = Modifier.height(205.dp()).clip(MaterialTheme::shapes().extra_large.clone()),
                            content_scale = ContentScale::Crop,
                        );
                    },
                );
            },
        );
    } else {
        let scroll = remember_scroll_state();
        let s = show_all_items.clone();
        Column(
            modifier = Modifier.fill_max_size().vertical_scroll(&scroll).padding_hv(vertical = 16.dp()),
            content = move || {
                let items = items.clone();
                HorizontalMultiBrowseCarousel(
                    remember_carousel_state(item_count = || 5),
                    186.dp(),
                    modifier = Modifier.width(412.dp()).height(221.dp()),
                    item_spacing = 8.dp(),
                    content_padding = PaddingValues::hv(16.dp(), 0.dp()),
                    content = move |scope: &CarouselItemScope, i: i32| {
                        let item = items[i as usize].clone();
                        kompozable_block!({
                            Image(
                                modifier = scope.mask_clip(Modifier.height(205.dp()), extra_large()),
                                painter = painter_resource(item.image_res_id.clone()),
                                content_description = Some(string_resource(item.content_description_res_id.clone())),
                                content_scale = ContentScale::Crop,
                            );
                        });
                    },
                );
                let s = s.clone();
                TextButton(
                    on_click = move || s.set(true),
                    modifier = Modifier.align(Alignment::End).padding_hv(vertical = 4.dp(), horizontal = 16.dp()),
                    content = || Text("Show all"),
                );
                // Content of the scrolling page, for example.
                for _ in 0..=4 {
                    Spacer(modifier = Modifier.height(16.dp()));
                    Box(modifier =
                        Modifier.fill_max_width().height(140.dp()).padding_hv(horizontal = 16.dp()).background(
                            MaterialTheme::color_scheme().surface_variant,
                            Arc::new(RoundedCornerShape(16.dp())),
                        ));
                }
            },
        );
    }
}

#[kompozable]
fn MultiAspectCarouselLazyRowSample() {
    let sizes = [305.dp(), 205.dp(), 275.dp(), 350.dp(), 100.dp()];
    let items: Vec<(CarouselItem, Dp)> = sample_items().into_iter().zip(sizes).collect();
    MultiAspectCarouselScope(move |carousel: &MultiAspectCarouselScope| {
        let state = remember_lazy_list_state();
        let (items, carousel) = (items.clone(), carousel.clone());
        LazyRow(
            state = state.clone(),
            content_padding = PaddingValues::all(16.dp()),
            horizontal_arrangement = Arrangement::spaced_by(8.dp()),
            modifier = Modifier.fill_max_width().height(221.dp()),
            content = move |this: &mut LazyListScope| {
                let (items, carousel, state) = (items.clone(), carousel.clone(), state.clone());
                this.items_indexed(
                    items,
                    item_content = move |i: i32, (item, main_axis_size): &(CarouselItem, Dp)| {
                        let state = state.clone();
                        let draw_info = remember(move || MultiAspectCarouselItemDrawInfo(i, state));
                        Image(
                            painter = painter_resource(item.image_res_id.clone()),
                            content_description = Some(string_resource(item.content_description_res_id.clone())),
                            modifier = carousel.mask_clip(
                                Modifier.width(*main_axis_size).height(205.dp()),
                                extra_large(),
                                draw_info,
                            ),
                            content_scale = ContentScale::Crop,
                        );
                    },
                );
            },
        );
    });
}

/// HorizontalMultiBrowseCarouselSample: its images masked from 16 dp in, tapping one glides it into focus.
fn horizontal_multi_browse_carousel_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| HorizontalMultiBrowseCarouselSample()),
        IntSize::new(412, 800),
    );
    rule.wait_for_idle();
    for i in 1..=3 {
        rule.on_node_with_content_description(&description(i)).assert_exists();
    }
    let before = description_bounds(&rule, 3);
    rule.on_node_with_content_description(&description(3)).perform_click();
    rule.wait_for_idle();
    assert!(description_bounds(&rule, 3).left < before.left - 100.0, "{before:?}");
    assert_eq!(sample_items()[2].id, 2);
}

/// HorizontalUncontainedCarouselSample: items 186 + 8 wide from the start padding.
fn horizontal_uncontained_carousel_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| HorizontalUncontainedCarouselSample()),
        IntSize::new(500, 800),
    );
    rule.wait_for_idle();
    let (first, second) = (description_bounds(&rule, 1), description_bounds(&rule, 2));
    assert!(near(first.width(), 194.0) && near(first.height(), 205.0), "{first:?}");
    assert!(near(second.left - first.left, 194.0 + 8.0 - 16.0 / 3.0), "{first:?} {second:?}");
}

/// HorizontalCenteredHeroCarouselSample: the first image large at the start; tapping the third (its small mask at
/// the end: a click at its node's center would land on the second's, as Compose's — the center of its bounds cut to
/// the carousel) centers it.
fn horizontal_centered_hero_carousel_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| HorizontalCenteredHeroCarouselSample()),
        IntSize::new(412, 800),
    );
    rule.wait_for_idle();
    let before = description_bounds(&rule, 3);
    let at = Offset::new(before.center().x, 100.0);
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.advance_frame();
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
    let after = description_bounds(&rule, 3);
    assert!(near_by(after.center().x, 412.0 / 2.0, 1.0), "{before:?} {after:?}");
}

/// FadingHorizontalMultiBrowseCarouselSample: stacked items of two images bordered magenta and green, the others
/// bordered red with a chip pinned to the mask's start.
fn fading_horizontal_multi_browse_carousel_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| FadingHorizontalMultiBrowseCarouselSample()),
        IntSize::new(412, 800),
    );
    rule.wait_for_idle();
    assert_eq!(rule.on_all_nodes_with_content_description(&description(2)).fetch_semantics_nodes().len(), 2);
    rule.on_node_with_text("Image 0").assert_exists();
    let canvas = rule.draw();
    for color in [Color::Red, Color::Magenta, Color::Green] {
        assert!(canvas.mask_colors.contains(&color), "{:?}", canvas.mask_colors);
    }
}

/// CarouselWithShowAllButtonSample: "Show all" shows the grid of all the images; back, the carousel again.
fn carousel_with_show_all_button_sample() {
    let rule =
        KompozTestRule::with_viewport(kompozable_lambda!(|| CarouselWithShowAllButtonSample()), IntSize::new(412, 800));
    rule.wait_for_idle();
    assert!(!rule.host().back_invoked()); // (nothing to go back from)
    rule.on_node_with_text("Show all").perform_click();
    rule.wait_for_idle();
    assert!(!rule.on_node_with_text("Show all").exists());
    for i in 1..=5 {
        rule.on_node_with_content_description(&description(i)).assert_exists();
    }
    assert!(rule.host().back_invoked());
    rule.wait_for_idle();
    rule.on_node_with_text("Show all").assert_exists();
}

/// MultiAspectCarouselLazyRowSample: images of their own widths; the one entering at the end masked.
fn multi_aspect_carousel_lazy_row_sample() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| MultiAspectCarouselLazyRowSample()),
        IntSize::new(412, 800),
    );
    rule.wait_for_idle();
    let (first, second) = (description_bounds(&rule, 1), description_bounds(&rule, 2));
    assert!(near(first.width(), 305.0) && near(second.width(), 205.0) && near(second.left, 16.0 + 305.0 + 8.0));
    // (the second enters at the end: masked from its end, its content moved towards the start)
    let canvas = rule.draw();
    assert!(
        canvas.clip_shapes.iter().any(|k| k.bounds.width() < 204.0 && k.bounds.width() > 50.0),
        "{:?}",
        canvas.clip_shapes
    );
}

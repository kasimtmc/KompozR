//! Shared elements (Compose's SharedTransitionScope.kt, SharedContentNode.kt, SharedTransitionStateMachine.kt,
//! SharedElement.kt, SharedElementEntry.kt, RenderInTransitionOverlayNodeElement.kt, BoundsAnimation.kt): the C++
//! Kompoz's tests of SharedTransitionLayout (shared_transition_overlay_draws_in_its_order_and_nodes_above,
//! shared_bounds_resize_mode_scales_or_remeasures, shared_element_moves_from_its_match_and_hides_it,
//! a_scrolled_lists_places_are_worked_out_where_they_are_asked_for, skip_to_lookahead_size_lays_out_where_it_goes),
//! by name, adapted where Compose's behaviour differs (each says how); and Compose's behaviour: caller-managed
//! visibility, overlay clips, placeholder sizes, skipToLookaheadPosition, SharedContentConfig, an interruption mid-way,
//! nested AnimatedContent, the visual debugging of shared elements.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_animation::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::layout::SlotKey;
use kompoz_ui::unit::Velocity;
use kompoz_ui::window::Popup;
use kompoz_ui::{Alignment, Modifier};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// Where `color` is first drawn as a filled rect (none: not at all).
fn order(c: &RecordingCanvas, color: Color) -> Option<usize> {
    c.rect_colors.iter().position(|&x| x == color)
}

/// The rects drawn in `color`.
fn rects_of(c: &RecordingCanvas, color: Color) -> Vec<Rect> {
    c.rects.iter().zip(&c.rect_colors).filter(|(_, x)| **x == color).map(|(r, _)| *r).collect()
}

fn same_rect(a: Rect, b: Rect) -> bool {
    near(a.left, b.left, 0.01)
        && near(a.top, b.top, 0.01)
        && near(a.width(), b.width(), 0.01)
        && near(a.height(), b.height(), 0.01)
}

/// The content's scope as the AnimatedVisibilityScope it is (Kotlin's `this@AnimatedContent`).
fn visibility_scope(s: &AnimatedContentScope) -> AnimatedVisibilityScope {
    AnimatedVisibilityScope::from(s.clone())
}

const RED: Color = Color::hex(0xC03030);
const GREEN: Color = Color::hex(0x30A030);
const BLUE: Color = Color::hex(0x3050C0);
const GRAY: Color = Color::hex(0x808080);
const YELLOW: Color = Color::hex(0xE0C020);

/// 160 ms into a change of `details` (each frame drawn, as a host draws them): the last frame's drawing.
fn middle(rule: &KompozTestRule, details: &MutableState<bool>) -> RecordingCanvas {
    let _ = rule.draw();
    details.set(!details.peek());
    let mut canvas = rule.draw();
    for _ in 0..10 {
        rule.advance_frame();
        canvas = rule.draw();
    }
    canvas
}

// ---- C++ shared_transition_overlay_draws_in_its_order_and_nodes_above ---------------------------------------------

fn shared_transition_overlay_draws_in_its_order_and_nodes_above() {
    let details = mutable_state_of(false);
    let in_overlay = mutable_state_of(true);
    let (d, o) = (details.clone(), in_overlay.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (d, o) = (d.clone(), o.clone());
            SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
                let (d, o, shared) = (d.clone(), o.clone(), shared.clone());
                Box(
                    modifier = Modifier.fill_max_size(),
                    content = move || {
                        let (o, sh) = (o.clone(), shared.clone());
                        AnimatedContent(
                            d.get(),
                            content = move |s: &AnimatedContentScope, open: bool| {
                                let scope = visibility_scope(s);
                                let a = sh.remember_shared_content_state(SlotKey::new("a"));
                                let b = sh.remember_shared_content_state(SlotKey::new("b"));
                                let (sh, o) = (sh.clone(), o.clone());
                                Column(move || {
                                    let size = if open { (200.0, 100.0) } else { (40.0, 40.0) };
                                    let m = sh.shared_element(
                                        Modifier.size_wh(size.0.dp(), size.1.dp()),
                                        a.clone(),
                                        &scope,
                                        render_in_overlay_during_transition = o.get(),
                                        z_index_in_overlay = 2.0,
                                    );
                                    Box(modifier = m.background(RED));
                                    let w = if open { 150.0 } else { 30.0 };
                                    let m = sh.shared_element(Modifier.size_wh(w.dp(), 30.dp()), b.clone(), &scope);
                                    Box(modifier = m.background(GREEN));
                                });
                            },
                        );
                        // Always in the overlay (its own condition), though before the gray box here.
                        let m = shared.render_in_shared_transition_scope_overlay(
                            Modifier.size(20.dp()),
                            render_in_overlay = Rc::new(|| true),
                        );
                        Box(modifier = m.background(YELLOW));
                        Box(modifier = Modifier.fill_max_width().height(300.dp()).background(GRAY)); // over the content
                        // A bar the moving elements pass under: in the overlay at 1, between them.
                        let m = shared.render_in_shared_transition_scope_overlay(
                            Modifier.align(Alignment::BottomCenter).fill_max_width().height(60.dp()),
                            z_index_in_overlay = 1.0,
                        );
                        Box(modifier = m.background(BLUE));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    // At rest: all where they are, the bar last but the yellow box (in the overlay always: after the content).
    let still = rule.draw();
    let at = |c: &RecordingCanvas, color| order(c, color).unwrap_or(usize::MAX);
    assert!(at(&still, RED) < at(&still, GRAY), "{:?}", still.rect_colors);
    assert!(at(&still, GRAY) < at(&still, BLUE) && at(&still, BLUE) < at(&still, YELLOW));
    // Moving: above the content, by z: green (0), the bar (1), red (2).
    let c = middle(&rule, &details);
    assert!(at(&c, GRAY) < at(&c, GREEN) && at(&c, GREEN) < at(&c, BLUE) && at(&c, BLUE) < at(&c, RED));
    // (a top-level shared element at 0 before other content of the overlay at 0: the yellow box)
    assert!(at(&c, GREEN) < at(&c, YELLOW) && at(&c, YELLOW) < at(&c, BLUE));
    rule.wait_for_idle();
    // Red moving where it is: under the gray box that comes after it.
    in_overlay.set(false);
    rule.wait_for_idle();
    let c = middle(&rule, &details);
    assert!(order(&c, RED).is_some() && at(&c, RED) < at(&c, GRAY) && at(&c, GREEN) < at(&c, BLUE));
    assert_eq!(rects_of(&c, RED).len(), 1); // not in the overlay too
    rule.wait_for_idle();
}

// ---- C++ shared_bounds_resize_mode_scales_or_remeasures -----------------------------------------------------------

fn shared_bounds_resize_mode_scales_or_remeasures() {
    let details = mutable_state_of(false);
    let mode = mutable_state_of(0); // 0: RemeasureToBounds, 1: scale_to_bounds(FillBounds)
    let (d, md) = (details.clone(), mode.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (d, md) = (d.clone(), md.clone());
            SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
                let (shared, md) = (shared.clone(), md.clone());
                AnimatedContent(
                    d.get(),
                    content = move |s: &AnimatedContentScope, open: bool| {
                        let card = shared.remember_shared_content_state(SlotKey::new("kart"));
                        let resize_mode = if md.get() == 0 {
                            ResizeMode::RemeasureToBounds
                        } else {
                            ResizeMode::scale_to_bounds(ContentScale::FillBounds, Alignment::Center)
                        };
                        let size = if open { (200.0, 100.0) } else { (40.0, 40.0) };
                        let m = shared.shared_bounds(
                            Modifier.size_wh(size.0.dp(), size.1.dp()),
                            card,
                            &visibility_scope(s),
                            resize_mode = resize_mode,
                        );
                        Box(modifier = m.background(if open { BLUE } else { RED }));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    // Remeasured: both contents drawn at the in-between size, not scaled.
    let c = middle(&rule, &details);
    let drawn: Vec<Rect> = rects_of(&c, BLUE).into_iter().chain(rects_of(&c, RED)).collect();
    assert_eq!(drawn.len(), 2, "{drawn:?}");
    let w = drawn[0].width();
    assert!(drawn.iter().all(|r| r.width() > 45.0 && r.width() < 195.0 && (r.width() - w).abs() < 0.5), "{drawn:?}");
    // (a layer scaling from its top left is drawn through its matrix)
    // (AnimatedContent's default scaleIn scales the content coming in evenly; nothing is scaled unevenly)
    let uneven = |m: &&Matrix| (m.values[0] - m.values[5]).abs() > 0.001;
    assert!(!c.transforms.iter().any(|m| uneven(&m)));
    rule.wait_for_idle();
    // FillBounds: each scaled into the bounds both ways, unevenly (200 x 100 and 40 x 40 into the bounds on the way).
    mode.set(1);
    rule.wait_for_idle();
    let c = middle(&rule, &details);
    let unevenly: Vec<&Matrix> = c.transforms.iter().filter(|m| (m.values[0] - m.values[5]).abs() > 0.02).collect();
    assert_eq!(unevenly.len(), 2, "{:?}", c.transforms);
    // (laid out at their own sizes where they are going, scaled into the bounds)
    assert!(has_size(&c, BLUE, 200.0, 100.0) && has_size(&c, RED, 40.0, 40.0));
    rule.wait_for_idle();
}

/// Whether a rect of `color` of `w` by `h` is drawn.
fn has_size(c: &RecordingCanvas, color: Color, w: f32, h: f32) -> bool {
    rects_of(c, color).iter().any(|r| near(r.width(), w, 0.01) && near(r.height(), h, 0.01))
}

// ---- C++ shared_element_moves_from_its_match_and_hides_it ---------------------------------------------------------

fn shared_element_moves_from_its_match_and_hides_it() {
    // Compose's drawing: the moving element is laid out at the bounds (measured at their size, placed where they are)
    // and drawn in the overlay from there, its content recorded in a layer; C++ drew it where it is laid out and moved
    // its layer. So the rects land where the bounds are.
    let details = mutable_state_of(false);
    let bounds_mode = mutable_state_of(false); // shared_bounds instead of shared_element
    let (d, b) = (details.clone(), bounds_mode.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (d, b) = (d.clone(), b.clone());
            SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
                let (shared, b) = (shared.clone(), b.clone());
                AnimatedContent(
                    d.get(),
                    content = move |s: &AnimatedContentScope, open: bool| {
                        let photo = shared.remember_shared_content_state(SlotKey::new("photo"));
                        let m = if open {
                            Modifier.offset(100.dp(), 300.dp()).size(200.dp())
                        } else {
                            Modifier.size(40.dp())
                        };
                        let scope = visibility_scope(s);
                        let m = if b.get() {
                            shared.shared_bounds(m, photo, &scope)
                        } else {
                            shared.shared_element(m, photo, &scope)
                        };
                        Box(modifier = m.background(if open { BLUE } else { RED }));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    let frame = || {
        rule.advance_frame();
        rule.draw()
    };
    rule.wait_for_idle();
    let still = frame();
    assert!(rects_of(&still, RED) == vec![Rect::new(0.0, 0.0, 40.0, 40.0)] && rects_of(&still, BLUE).is_empty());
    details.set(true);
    // It starts where the small one was, at its size; the small one isn't drawn (a shared element going isn't).
    let first = frame();
    let first = if rects_of(&first, BLUE).is_empty() { frame() } else { first };
    assert!(rects_of(&first, RED).is_empty(), "{:?}", first.rect_colors);
    assert_eq!(rects_of(&first, BLUE), vec![Rect::new(0.0, 0.0, 40.0, 40.0)]);
    let mut mid = frame();
    for _ in 0..9 {
        mid = frame(); // 160 ms in
    }
    let moving = rects_of(&mid, BLUE);
    assert_eq!(moving.len(), 1);
    let (size, x) = (moving[0].width(), moving[0].left);
    assert!(size > 60.0 && size < 190.0 && x > 5.0 && x < 95.0, "{moving:?}");
    // (laid out at the in-between size, whole px)
    assert!(moving[0].width() == moving[0].width().round() && rects_of(&mid, RED).is_empty());
    for _ in 0..80 {
        frame();
    }
    let settled = frame();
    assert!(
        rects_of(&settled, BLUE) == vec![Rect::new(100.0, 300.0, 300.0, 500.0)] && rects_of(&settled, RED).is_empty()
    );
    // Back, with shared_bounds: both are drawn in the moving bounds (scaled into them), the leaving one fading out.
    bounds_mode.set(true);
    rule.wait_for_idle();
    frame();
    details.set(false);
    frame();
    for _ in 0..6 {
        mid = frame();
    }
    let (blue, red) = (rects_of(&mid, BLUE), rects_of(&mid, RED));
    assert!(blue.len() == 1 && red.len() == 1, "{blue:?} {red:?}");
    // (each laid out at its own size where it goes and scaled from its top left into the bounds, FillWidth: placed
    // alike, the leaving one 200 wide, the coming one 40, by scales of one width; a layer scaling from its top left is
    // drawn through its matrix)
    assert!(near(blue[0].left, red[0].left, 0.01) && near(blue[0].top, red[0].top, 0.01), "{blue:?} {red:?}");
    assert!(near(blue[0].width(), 200.0, 0.01) && near(red[0].width(), 40.0, 0.01));
    let scales: Vec<f32> = mid.transforms.iter().filter(|m| m.values[0] == m.values[5]).map(|m| m.values[0]).collect();
    let width =
        scales.iter().find_map(|&b| scales.iter().find(|&&r| (200.0 * b - 40.0 * r).abs() < 0.5).map(|_| 200.0 * b));
    assert!(width.is_some_and(|w| w > 40.0 && w < 200.0), "{scales:?}");
    // (the leaving one fading out as the coming one fades in: their alphas add up to one)
    let fading: Vec<f32> = mid.layer_alphas.iter().copied().filter(|a| *a > 0.05 && *a < 0.95).collect();
    assert!(fading.iter().any(|a| fading.iter().any(|b| (a + b - 1.0).abs() < 0.02)), "{:?}", mid.layer_alphas);
    for _ in 0..80 {
        frame();
    }
    let settled = frame();
    assert!(rects_of(&settled, RED) == vec![Rect::new(0.0, 0.0, 40.0, 40.0)] && rects_of(&settled, BLUE).is_empty());
}

// ---- C++ skip_to_lookahead_size_lays_out_where_it_goes ------------------------------------------------------------

fn skip_to_lookahead_size_lays_out_where_it_goes() {
    // (C++'s `animateBounds()` with its own LookaheadScope Box: Compose's `animateBounds(lookaheadScope)` in a
    // `LookaheadScope { }` (D9); `skipToLookaheadSize()` is the shared transition scope's: the first two boxes are in
    // the SharedTransitionLayout's scope here, their skipping enabled always)
    let wide = mutable_state_of(false);
    let details = mutable_state_of(false);
    let (w, d) = (wide.clone(), details.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (w, d) = (w.clone(), d.clone());
            SharedTransitionLayout(
                Modifier.fill_max_width().height(400.dp()),
                move |shared: &SharedTransitionScope| {
                    let (w, d, shared) = (w.clone(), d.clone(), shared.clone());
                    Column(
                        modifier = Modifier.fill_max_size(),
                        content = move || {
                            for skip in [true, false] {
                                let lookahead: Rc<dyn kompoz_ui::LookaheadScope> = shared.clone().into();
                                let width = if w.get() { 300.0 } else { 100.0 };
                                let inner = if skip {
                                    shared.skip_to_lookahead_size(Modifier, enabled = Rc::new(|| true))
                                } else {
                                    Modifier
                                };
                                let tag = if skip { 1 } else { 0 };
                                Box(
                                    modifier = Modifier.animate_bounds(lookahead).width(width.dp()).height(40.dp()),
                                    content = move || {
                                        // (the tag before the skipping: Compose's semantics bounds are those of the
                                        // layout the tag wraps — after it, the content's)
                                        Box(
                                            modifier = Modifier
                                                .test_tag(format!("dış{tag}"))
                                                .then(inner.clone())
                                                .fill_max_size(),
                                            content = move || {
                                                Box(modifier = Modifier.fill_max_size().test_tag(format!("iç{tag}")));
                                            },
                                        );
                                    },
                                );
                            }
                            let shared = shared.clone();
                            // (faded only: Compose's default also scales the content coming in, from 92 %)
                            AnimatedContent(
                                d.get(),
                                transition_spec = |_| fade_in().together_with(fade_out()),
                                content = move |s: &AnimatedContentScope, open: bool| {
                                    let card = shared.remember_shared_content_state(SlotKey::new("kart"));
                                    let m = shared.shared_element(
                                        Modifier.size_wh(if open { 300.dp() } else { 60.dp() }, 60.dp()),
                                        card,
                                        &visibility_scope(s),
                                    );
                                    Box(
                                        modifier = shared.skip_to_lookahead_size(m),
                                        content = move || {
                                            let tag = if open { "kart açık" } else { "kart kapalı" };
                                            Box(modifier = Modifier.fill_max_size().test_tag(tag));
                                        },
                                    );
                                },
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.main_clock().set_auto_advance(false);
    settle(&rule);
    let width = |tag: &str| bounds(&rule, tag).width();
    wide.set(true);
    for _ in 0..6 {
        rule.advance_frame();
    }
    let along = width("dış0");
    assert!(along > 120.0 && along < 280.0 && near(width("iç0"), along, 0.5), "{along}"); // its content along the way
    // (laid out where it goes, sized along the way: the semantics bounds are the layout's, the skipping one's
    // content at its lookahead size)
    assert!(near(width("dış1"), along, 0.5) && near(width("iç1"), 300.0, 0.5), "{} {}", width("dış1"), width("iç1"));
    settle(&rule);
    assert!(near(width("iç0"), 300.0, 0.5) && near(width("iç1"), 300.0, 0.5));
    wide.set(false); // back: laid out at 100 from the start
    for _ in 0..6 {
        rule.advance_frame();
    }
    assert!(near(width("iç1"), 100.0, 0.5) && width("iç0") > 120.0);
    settle(&rule);
    // A moving shared element: the entering content at its own size.
    let _ = rule.draw();
    details.set(true);
    for _ in 0..6 {
        rule.advance_frame();
        let _ = rule.draw();
    }
    assert!(near(width("kart açık"), 300.0, 0.5), "{}", width("kart açık"));
    settle(&rule);
}

// ---- C++ a_scrolled_lists_places_are_worked_out_where_they_are_asked_for -------------------------------------------

fn a_scrolled_lists_places_are_worked_out_where_they_are_asked_for() {
    const POPUP_YELLOW: Color = Color::hex(0xFFFF00);
    struct BesideAnchor;
    impl kompoz_ui::window::PopupPositionProvider for BesideAnchor {
        fn calculate_position(&self, anchor: IntRect, _: IntSize, _: LayoutDirection, _: IntSize) -> IntOffset {
            IntOffset::new(anchor.right, anchor.top)
        }
    }
    let scroll = ScrollState(0);
    let s = scroll.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
                let shared = shared.clone();
                Column(
                    modifier = Modifier.fill_max_size().vertical_scroll(&s),
                    content = move || {
                        Box(modifier = Modifier.height(100.dp()));
                        let shared = shared.clone();
                        // (nested: what is in it is placed from it)
                        Box(
                            modifier = Modifier.padding(10.dp()),
                            content = move || {
                                let m = shared.render_in_shared_transition_scope_overlay(
                                    Modifier.size(20.dp()),
                                    render_in_overlay = Rc::new(|| true),
                                );
                                Box(modifier = m.background(POPUP_YELLOW).test_tag("üstte"));
                            },
                        );
                        Box(
                            modifier = Modifier.padding(5.dp()).size(30.dp()).test_tag("çapa"),
                            content = || {
                                Popup(
                                    popup_position_provider =
                                        Some(Rc::new(BesideAnchor) as Rc<dyn kompoz_ui::window::PopupPositionProvider>),
                                    content = || Box(modifier = Modifier.size(10.dp()).test_tag("açılır")),
                                );
                            },
                        );
                        Box(modifier = Modifier.height(2000.dp()));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    let check = |scrolled: f32| {
        let (top, anchor, popup) = (bounds(&rule, "üstte"), bounds(&rule, "çapa"), bounds(&rule, "açılır"));
        let c = rule.draw();
        let at = rects_of(&c, POPUP_YELLOW).first().copied().unwrap_or(Rect::Zero);
        // (the anchor's tag after its padding: Compose's semantics bounds, inside it; the popup's anchor is that too)
        assert!(near(top.top, 110.0 - scrolled, 0.5) && near(anchor.top, 145.0 - scrolled, 0.5), "{top:?} {anchor:?}");
        assert!(near(popup.left, anchor.right, 0.5) && near(popup.top, anchor.top, 0.5), "{popup:?} {anchor:?}");
        assert!(near(at.top, top.top, 0.5) && near(at.left, top.left, 0.5), "{at:?} {top:?}");
    };
    check(0.0);
    for to in [37, 12, 80] {
        let s = scroll.clone();
        kompoz_coroutines::CoroutineScope::new(rule.host().dispatcher()).launch(async move {
            s.scroll_to(to).await;
        });
        rule.wait_for_idle();
        check(to as f32);
    }
}

// ---- Compose's behaviour ------------------------------------------------------------------------------------------

/// Two shared elements of "kutu" whose visibility the caller manages: `a` (40, at the top left, red) shown while
/// `first`, `b` (100, at (200, 400), blue) otherwise; `b` only while `with_b`; `config` theirs.
#[kompozable]
fn CallerManaged(first: bool, with_b: bool, config: Rc<dyn SharedContentConfig>, placeholder_size: PlaceholderSize) {
    SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
        let (shared, config, placeholder_size) = (shared.clone(), config.clone(), placeholder_size.clone());
        Box(
            modifier = Modifier.fill_max_size(),
            content = move || {
                let a = shared.remember_shared_content_state(SlotKey::new("kutu"), config = config.clone());
                Column(move || {
                    // (the shared element outermost: its placeholder is what the Column lays out)
                    let m = shared.shared_element_with_caller_managed_visibility(
                        Modifier,
                        a.clone(),
                        first,
                        placeholder_size = placeholder_size.clone(),
                    );
                    Box(modifier = m.size(40.dp()).background(RED).test_tag("a"));
                    Box(modifier = Modifier.size(10.dp()).test_tag("altında"));
                });
                if with_b {
                    let b = shared.remember_shared_content_state(SlotKey::new("kutu"), config = config.clone());
                    let m = shared.shared_element_with_caller_managed_visibility(
                        Modifier.offset(200.dp(), 400.dp()).size(100.dp()),
                        b,
                        !first,
                    );
                    Box(modifier = m.background(BLUE).test_tag("b"));
                }
            },
        );
    });
}

fn caller_managed_visibility_moves_the_visible_one_and_hides_the_other() {
    // Compose's sharedElementWithCallerManagedVisibility: both stay in the tree; the one shown moves from the other's
    // bounds to its own, the other is drawn nowhere and, the match outliving the transition, laid out where the
    // shown one is.
    let first = mutable_state_of(true);
    let f = first.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            CallerManaged(f.get(), true, SharedTransitionDefaults::SharedContentConfig(), PlaceholderSize::ContentSize);
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let c = rule.draw();
    assert_eq!(rects_of(&c, RED), vec![Rect::new(0.0, 0.0, 40.0, 40.0)]);
    // (the one not shown laid out where it is in lookahead: what it wraps measured with its lookahead constraints
    // once no approach is needed, as Compose's coordinators do)
    assert!(rects_of(&c, BLUE).is_empty() && same_rect(bounds(&rule, "b"), Rect::new(200.0, 400.0, 300.0, 500.0)));
    first.set(false);
    let mut c = rule.draw();
    for _ in 0..10 {
        rule.advance_frame();
        c = rule.draw();
    }
    let moving = rects_of(&c, BLUE);
    assert!(moving.len() == 1 && moving[0].left > 5.0 && moving[0].left < 195.0, "{moving:?}");
    assert!(moving[0].width() > 40.0 && moving[0].width() < 100.0 && rects_of(&c, RED).is_empty());
    rule.wait_for_idle();
    let c = rule.draw();
    assert_eq!(rects_of(&c, BLUE), vec![Rect::new(200.0, 400.0, 300.0, 500.0)]);
    assert!(rects_of(&c, RED).is_empty() && same_rect(bounds(&rule, "a"), Rect::new(0.0, 0.0, 40.0, 40.0)));
}

fn placeholder_size_is_what_the_parent_lays_out() {
    // ContentSize: the shared element's layout takes its lookahead size at once (what comes after it in a Column is
    // where it goes); AnimatedSize: the size along the animation (what comes after follows it).
    for (placeholder_size, at_once) in [(PlaceholderSize::ContentSize, true), (PlaceholderSize::AnimatedSize, false)] {
        let first = mutable_state_of(false);
        let f = first.clone();
        let ps = placeholder_size.clone();
        let rule = KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                CallerManaged(f.get(), true, SharedTransitionDefaults::SharedContentConfig(), ps.clone());
            }),
            IntSize::new(400, 800),
        );
        rule.main_clock().set_auto_advance(false);
        rule.wait_for_idle();
        // (at rest, `a` not shown: laid out at its lookahead size, no approach needed)
        let rest = bounds(&rule, "altında").top;
        assert!(near(rest, 40.0, 0.5), "{placeholder_size:?}: {rest}");
        first.set(true);
        for _ in 0..8 {
            rule.advance_frame();
        }
        let (a, below) = (bounds(&rule, "a"), bounds(&rule, "altında").top);
        assert!(a.width() > 45.0 && a.width() < 95.0, "{placeholder_size:?}: {a:?}");
        if at_once {
            assert!(near(below, 40.0, 0.5), "{below}");
        } else {
            assert!(near(below, a.height(), 0.5), "{below} {a:?}");
        }
        settle(&rule);
        assert!(near(bounds(&rule, "altında").top, 40.0, 0.5));
    }
    // (a placeholder size of a closure)
    let half = PlaceholderSize(|content: IntSize, animated: IntSize| {
        IntSize::new((content.width + animated.width) / 2, (content.height + animated.height) / 2)
    });
    assert_eq!(half.calculate_size(IntSize::new(40, 40), IntSize::new(100, 60)), IntSize::new(70, 50));
    assert!(half != PlaceholderSize::ContentSize && half == half.clone());
}

/// A shared content configuration of the tests: enabled while `enabled`, going to `alternative` once its target is
/// removed.
struct TestConfig {
    enabled: Rc<std::cell::Cell<bool>>,
    alternative: Option<Rect>,
    asked: Rc<RefCell<Vec<(Rect, Size)>>>,
}

impl SharedContentConfig for TestConfig {
    fn is_enabled(&self, _: &SharedContentState) -> bool {
        self.enabled.get()
    }
    fn alternative_target_bounds_in_transition_scope_after_removal(
        &self,
        _: &SharedContentState,
        target_bounds_before_removal: Rect,
        shared_transition_layout_size: Size,
    ) -> Option<Rect> {
        self.asked.borrow_mut().push((target_bounds_before_removal, shared_transition_layout_size));
        self.alternative
    }
}

fn shared_content_config_enables_and_gives_targets_after_removal() {
    // Disabled: no match, the two shown and hidden as they are (each where it is laid out, as if not shared).
    let enabled = Rc::new(std::cell::Cell::new(false));
    let asked = Rc::new(RefCell::new(Vec::new()));
    let config: Rc<dyn SharedContentConfig> =
        Rc::new(TestConfig { enabled: enabled.clone(), alternative: None, asked: asked.clone() });
    let (first, with_b) = (mutable_state_of(true), mutable_state_of(true));
    let (f, w, cfg) = (first.clone(), with_b.clone(), config.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            CallerManaged(f.get(), w.get(), cfg.clone(), PlaceholderSize::ContentSize);
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    first.set(false);
    rule.advance_frame();
    rule.advance_frame();
    let c = rule.draw();
    assert_eq!(rects_of(&c, BLUE), vec![Rect::new(200.0, 400.0, 300.0, 500.0)]);
    assert_eq!(rects_of(&c, RED), vec![Rect::new(0.0, 0.0, 40.0, 40.0)]);
    rule.wait_for_idle();
    // The target removed during the transition: none to go to by default, the other drawn where it is laid out.
    enabled.set(true);
    first.set(true);
    rule.wait_for_idle();
    first.set(false);
    for _ in 0..4 {
        rule.advance_frame();
    }
    with_b.set(false);
    rule.advance_frame();
    rule.advance_frame();
    let c = rule.draw();
    assert_eq!(rects_of(&c, RED), vec![Rect::new(0.0, 0.0, 40.0, 40.0)], "{:?}", c.rect_colors);
    let (before, layout) = asked.borrow().last().copied().expect("the configuration asked");
    assert!(before.width() > 40.0 && before.width() <= 100.0 && layout == Size::new(400.0, 800.0), "{before:?}");
    rule.wait_for_idle();
    // With an alternative target: the one left goes there (drawn, though not shown, along its bounds).
    let alternative = Rect::new(300.0, 700.0, 350.0, 750.0);
    let config: Rc<dyn SharedContentConfig> =
        Rc::new(TestConfig { enabled: enabled.clone(), alternative: Some(alternative), asked: asked.clone() });
    let (first, with_b) = (mutable_state_of(true), mutable_state_of(true));
    let (f, w, cfg) = (first.clone(), with_b.clone(), config.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            CallerManaged(f.get(), w.get(), cfg.clone(), PlaceholderSize::ContentSize);
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    first.set(false);
    for _ in 0..4 {
        rule.advance_frame();
    }
    with_b.set(false);
    rule.advance_frame();
    rule.advance_frame();
    let x = rects_of(&rule.draw(), RED).first().map_or(-1.0, |r| r.left);
    for _ in 0..6 {
        rule.advance_frame();
    }
    let later = rects_of(&rule.draw(), RED);
    assert!(later.len() == 1 && later[0].left > x && x > 0.0, "{x} {later:?}");
    // (the defaults' and the scope's configurations: all defaults; one not transformed during a deferred transition)
    let defaults = SharedTransitionDefaults::SharedContentConfig();
    assert!(Rc::ptr_eq(&defaults, &SharedTransitionDefaults::SharedContentConfig()));
    assert!(
        defaults.should_keep_enabled_for_ongoing_animation() && defaults.permit_transform_during_deferred_transition()
    );
}

fn shared_content_states_tell_their_match_and_the_scope_its_transition() {
    // isMatchFound as soon as a match is found (a deferred one too), isTransitionActive while a matched element
    // animates; the scope's configurations; a state's key; the overlay clip of a shape and the parent's clip.
    let details = mutable_state_of(false);
    let seen = Rc::new(RefCell::new(Vec::<(bool, bool)>::new()));
    let states = Rc::new(RefCell::new(Vec::<SharedContentState>::new()));
    let scope_cell: Rc<RefCell<Option<SharedTransitionScope>>> = Rc::new(RefCell::new(None));
    let (d, s, st, sc) = (details.clone(), seen.clone(), states.clone(), scope_cell.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (d, s, st, sc) = (d.clone(), s.clone(), st.clone(), sc.clone());
            SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
                *sc.borrow_mut() = Some(shared.clone());
                let (s, st, shared) = (s.clone(), st.clone(), shared.clone());
                AnimatedContent(
                    d.get(),
                    content = move |c: &AnimatedContentScope, open: bool| {
                        let photo = shared.remember_shared_content_state(SlotKey::new("photo"));
                        if open {
                            s.borrow_mut().push((photo.is_match_found(), shared.is_transition_active()));
                            st.borrow_mut().push(photo.clone());
                        }
                        let clip = shared.OverlayClip(RoundedCornerShape(8.dp()));
                        let m = if open {
                            Modifier.offset(100.dp(), 300.dp()).size(200.dp())
                        } else {
                            Modifier.size(40.dp())
                        };
                        let m = shared.shared_bounds(
                            m,
                            photo,
                            &visibility_scope(c),
                            clip_in_overlay_during_transition = clip,
                        );
                        Box(
                            modifier = m.background(if open { BLUE } else { RED }),
                            content = move || {
                                // (nested: clipped by its parent's clip by default)
                                let inner = shared.remember_shared_content_state(SlotKey::new("iç"));
                                let m = shared.shared_element(Modifier.size(10.dp()), inner, &visibility_scope(c));
                                Box(modifier = m.background(GREEN));
                            },
                        );
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let shared = scope_cell.borrow().clone().expect("the scope");
    assert!(!shared.is_transition_active());
    details.set(true);
    rule.advance_frame();
    for _ in 0..6 {
        rule.advance_frame();
    }
    let c = rule.draw();
    assert!(shared.is_transition_active());
    // (the open content's first composition reads a match deferred; its later ones the transition active)
    assert!(seen.borrow().iter().any(|(found, _)| *found), "{:?}", seen.borrow());
    assert!(seen.borrow().iter().any(|(_, active)| *active), "{:?}", seen.borrow());
    let photo = states.borrow().last().cloned().expect("the open content's state");
    assert!(photo.key().is(&"photo") && photo.is_match_found());
    // (the clip path in the overlay: the rounded shape at the bounds; the nested element's parent and clip)
    // (the content laid out at its lookahead size, 200, scaled into the bounds from their top left)
    let clip = photo.clip_path_in_overlay().expect("a clip in the overlay");
    let blue = rects_of(&c, BLUE)[0];
    let clip_bounds = clip.get_bounds();
    assert!(near(clip_bounds.left, blue.left, 0.5) && near(clip_bounds.top, blue.top, 0.5), "{clip_bounds:?} {blue:?}");
    assert!(clip_bounds.width() > 40.0 && clip_bounds.width() < 200.0, "{clip_bounds:?}");
    assert!(c.clip_shapes.iter().any(|s| s.path.is_some() && same_rect(s.bounds, clip_bounds)));
    let inner = states.borrow().last().cloned().expect("a state");
    assert!(inner.parent_shared_content_state().is_none());
    rule.wait_for_idle();
    assert!(!shared.is_transition_active() && photo.is_match_found() == photo.is_match_found());
    let c = rule.draw();
    assert!(c.clip_shapes.iter().all(|s| s.path.is_none())); // at rest: no clip of the overlay
    // (the scope's configuration factories)
    assert!(shared.SharedContentConfig().permit_transform_during_deferred_transition());
    let kept = named_args!(shared.SharedContentConfig(permit_transform_during_deferred_transition = false));
    assert!(!kept.permit_transform_during_deferred_transition() && kept.is_enabled(&photo));
}

fn skip_to_lookahead_position_places_at_once_where_it_goes() {
    // In a parent gliding to the other side (animate_bounds), a child skipping to its lookahead position is there at
    // once; one that doesn't follows its parent.
    let right = mutable_state_of(false);
    let r = right.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let r = r.clone();
            SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
                let (r, shared) = (r.clone(), shared.clone());
                let alignment = if r.get() { Alignment::TopEnd } else { Alignment::TopStart };
                Box(
                    modifier = Modifier.size_wh(400.dp(), 100.dp()),
                    content_alignment = alignment,
                    content = move || {
                        let lookahead: Rc<dyn kompoz_ui::LookaheadScope> = shared.clone().into();
                        let linear: Rc<dyn BoundsTransform> =
                            BoundsTransform(|_, _| TweenSpec::<Rect>::new(400, 0, LinearEasing));
                        let shared = shared.clone();
                        Row(
                            modifier = Modifier
                                .animate_bounds(lookahead, bounds_transform = linear)
                                .size_wh(100.dp(), 50.dp()),
                            content = move || {
                                let m = shared.skip_to_lookahead_position(Modifier, enabled = Rc::new(|| true));
                                Box(modifier = m.size(20.dp()).test_tag("atlayan"));
                                Box(modifier = Modifier.size(20.dp()).test_tag("izleyen"));
                            },
                        );
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.main_clock().set_auto_advance(false);
    settle(&rule);
    assert!(near(bounds(&rule, "atlayan").left, 0.0, 0.5) && near(bounds(&rule, "izleyen").left, 20.0, 0.5));
    right.set(true);
    rule.advance_frame();
    rule.advance_time_by(200);
    let (skipping, following) = (bounds(&rule, "atlayan").left, bounds(&rule, "izleyen").left);
    assert!(near(skipping, 300.0, 0.5), "{skipping}");
    assert!(following > 60.0 && following < 280.0, "{following}");
    settle(&rule);
    assert!(near(bounds(&rule, "atlayan").left, 300.0, 0.5) && near(bounds(&rule, "izleyen").left, 320.0, 0.5));
}

fn an_interrupted_shared_transition_goes_back_from_where_it_is() {
    let details = mutable_state_of(false);
    let d = details.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let d = d.clone();
            SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
                let shared = shared.clone();
                AnimatedContent(
                    d.get(),
                    content = move |s: &AnimatedContentScope, open: bool| {
                        let photo = shared.remember_shared_content_state(SlotKey::new("photo"));
                        let m = if open {
                            Modifier.offset(200.dp(), 400.dp()).size(100.dp())
                        } else {
                            Modifier.size(40.dp())
                        };
                        let m = shared.shared_element(m, photo, &visibility_scope(s));
                        Box(modifier = m.background(if open { BLUE } else { RED }));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let shown = |c: &RecordingCanvas| {
        let all: Vec<Rect> = rects_of(c, BLUE).into_iter().chain(rects_of(c, RED)).collect();
        assert_eq!(all.len(), 1, "{all:?}");
        all[0]
    };
    details.set(true);
    let mut last = shown(&rule.draw());
    for _ in 0..8 {
        rule.advance_frame();
        last = shown(&rule.draw());
    }
    assert!(last.left > 20.0 && last.left < 180.0, "{last:?}");
    // Back mid-way: no jump, from where it is to the small one's place.
    details.set(false);
    for _ in 0..40 {
        rule.advance_frame();
        let now = shown(&rule.draw());
        assert!((now.left - last.left).abs() < 40.0 && (now.width() - last.width()).abs() < 30.0, "{last:?} → {now:?}");
        last = now;
    }
    rule.wait_for_idle();
    assert_eq!(shown(&rule.draw()), Rect::new(0.0, 0.0, 40.0, 40.0));
}

fn shared_elements_follow_nested_animated_contents() {
    // A shared element in an inner AnimatedContent follows the inner one's transition (its scope's), another the
    // outer one's: its bounds animation runs while the root of its transitions does.
    let (page, sub) = (mutable_state_of(0), mutable_state_of(0));
    let (p, s) = (page.clone(), sub.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (p, s) = (p.clone(), s.clone());
            SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
                let (s, shared) = (s.clone(), shared.clone());
                AnimatedContent(
                    p.get(),
                    content = move |outer: &AnimatedContentScope, page: i32| {
                        let outer = visibility_scope(outer);
                        let (s, shared) = (s.clone(), shared.clone());
                        Box(
                            modifier = Modifier.fill_max_size(),
                            content = move || {
                                let (outer, shared) = (outer.clone(), shared.clone());
                                AnimatedContent(
                                    s.get(),
                                    content = move |inner: &AnimatedContentScope, sub: i32| {
                                        // (one of the inner scope, its key of the page; one of the outer)
                                        let key = format!("iç {page}");
                                        let state = shared.remember_shared_content_state(SlotKey::new(key));
                                        let m = Modifier.offset((sub * 200).dp(), 0.dp()).size(40.dp());
                                        let m = shared.shared_element(m, state, &visibility_scope(inner));
                                        Box(modifier = m.background(RED));
                                        let state = shared.remember_shared_content_state(SlotKey::new("dış"));
                                        let m = Modifier.offset(0.dp(), (page * 400 + 100).dp()).size(40.dp());
                                        let m = shared.shared_element(m, state, &outer);
                                        Box(modifier = m.background(BLUE));
                                    },
                                );
                            },
                        );
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    sub.set(1);
    for _ in 0..8 {
        rule.advance_frame();
    }
    let reds = rects_of(&rule.draw(), RED);
    assert!(reds.len() == 1 && reds[0].left > 20.0 && reds[0].left < 180.0, "{reds:?}");
    rule.wait_for_idle();
    assert_eq!(rects_of(&rule.draw(), RED), vec![Rect::new(200.0, 0.0, 240.0, 40.0)]);
    page.set(1);
    for _ in 0..8 {
        rule.advance_frame();
    }
    let blues = rects_of(&rule.draw(), BLUE);
    assert!(blues.len() == 1 && blues[0].top > 120.0 && blues[0].top < 480.0, "{blues:?}");
    rule.wait_for_idle();
    assert_eq!(rects_of(&rule.draw(), BLUE), vec![Rect::new(0.0, 500.0, 40.0, 540.0)]);
}

fn shared_elements_are_marked_by_the_visual_debugging() {
    // Compose's LookaheadAnimationVisualDebugging of shared elements: at rest each outlined in the inactive color;
    // during a transition the overlay's film, the scope's perimeter, the moving one's path and target (in its key's
    // color), one without a match outlined and hatched in the unmatched color.
    let inactive = Color::hex(0x102030);
    let unmatched = Color::hex(0x405060);
    let film = Color::from_argb(0x80708090);
    let details = mutable_state_of(false);
    let d = details.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let d = d.clone();
            LookaheadAnimationVisualDebugging(
                overlay_color = film,
                unmatched_element_color = unmatched,
                inactive_element_color = inactive,
                content = move || {
                    let d = d.clone();
                    SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
                        let shared = shared.clone();
                        AnimatedContent(
                            d.get(),
                            content = move |s: &AnimatedContentScope, open: bool| {
                                let scope = visibility_scope(s);
                                let photo = shared.remember_shared_content_state(SlotKey::new("photo"));
                                let m = if open {
                                    Modifier.offset(100.dp(), 300.dp()).size(200.dp())
                                } else {
                                    Modifier.size(40.dp())
                                };
                                Box(modifier = shared.shared_element(m, photo, &scope).background(BLUE));
                                if open {
                                    let alone = shared.remember_shared_content_state(SlotKey::new("yalnız"));
                                    let m = shared.shared_element(
                                        Modifier.offset(0.dp(), 600.dp()).size(50.dp()),
                                        alone,
                                        &scope,
                                    );
                                    Box(modifier = m.background(GREEN));
                                }
                            },
                        );
                    });
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(c.stroke_colors.contains(&inactive) && !c.stroke_colors.contains(&unmatched));
    assert!(!c.rect_colors.contains(&film));
    details.set(true);
    for _ in 0..8 {
        rule.advance_frame();
    }
    let c = rule.draw();
    assert!(c.rect_colors.contains(&film), "{:?}", c.rect_colors);
    assert!(c.stroke_colors.contains(&unmatched), "{:?}", c.stroke_colors);
    // (the moving one: its path dashed, its target outlined, in its key's color over white)
    assert!(c.stroke_colors.contains(&Color::White) && !c.paths.is_empty());
    rule.wait_for_idle();
}

fn deferred_transitions_move_shared_elements_by_hand_then_hand_over() {
    // A deferred phase (a predictive back's) moving the leaving content by hand: its shared element, the transition
    // active while it is moved (Compose's `isMutating`), is drawn in the overlay moved with it; the transition then
    // takes over from where it was shown. A configuration not permitting it leaves the element where its content
    // draws it (moved with its container, in place).
    for permit in [true, false] {
        let state = DeferredTransitionState(0);
        let progress = mutable_state_of(0);
        let (st, p) = (state.clone(), progress.clone());
        let rule = KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (st, p) = (st.clone(), p.clone());
                SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
                    let (st, p, shared) = (st.clone(), p.clone(), shared.clone());
                    Box(
                        modifier = Modifier.fill_max_size(),
                        content = move || {
                            let t = remember_deferred_transition(st.clone());
                            let (p, shared) = (p.clone(), shared.clone());
                            t.DeferredAnimatedContent(
                                transition_spec = |_| {
                                    fade_in(Arc::new(TweenSpec::<f32>::new(200, 0, LinearEasing)))
                                        .together_with(fade_out(Arc::new(TweenSpec::<f32>::new(200, 0, LinearEasing))))
                                },
                                mutable_transform_spec = move |_| {
                                    let (p1, p2) = (p.clone(), p.clone());
                                    Some(named_args!(MutableContentTransform(
                                        block = move |m: &MutableContentTransform| {
                                            let (p1, p2) = (p1.clone(), p2.clone());
                                            m.initial_content_transform(move |s: &mut dyn MutableTransformScope, _| {
                                                s.set_offset(IntOffset::new(-p1.get(), 0));
                                            });
                                            m.target_content_transform(
                                                move |s: &mut dyn MutableTransformScope, full: IntSize| {
                                                    s.set_offset(IntOffset::new(full.width - p2.get(), 0));
                                                },
                                            );
                                        }
                                    )))
                                },
                                content = move |c: &AnimatedContentScope, n: i32| {
                                    let config = named_args!(
                                        shared
                                            .SharedContentConfig(permit_transform_during_deferred_transition = permit)
                                    );
                                    let photo =
                                        shared.remember_shared_content_state(SlotKey::new("photo"), config = config);
                                    let m = if n == 1 {
                                        Modifier.offset(200.dp(), 400.dp()).size(100.dp())
                                    } else {
                                        Modifier.size(40.dp())
                                    };
                                    let m = shared.shared_element(m, photo, &visibility_scope(c));
                                    Box(modifier = m.background(if n == 1 { BLUE } else { RED }));
                                },
                            );
                            Box(modifier = Modifier.size(300.dp()).background(GRAY)); // over the content
                        },
                    );
                });
            }),
            IntSize::new(400, 800),
        );
        rule.wait_for_idle();
        state.defer(1);
        progress.set(30);
        rule.advance_frame();
        rule.advance_frame();
        // (in the overlay while its container is moved by hand — Compose's entry `isMutating` —: moved with it when
        // permitted, else where it was)
        let c = rule.draw();
        let red = rects_of(&c, RED);
        assert!(rects_of(&c, BLUE).is_empty(), "{permit}"); // (the one coming in not drawn: not shown yet)
        let (at, gray) = (order(&c, RED).expect("drawn"), order(&c, GRAY).expect("drawn"));
        assert!(at > gray, "{permit}: in the overlay");
        let x = if permit { -30.0 } else { 0.0 };
        assert_eq!(red, vec![Rect::new(x, 0.0, x + 40.0, 40.0)], "{permit}");
        progress.set(50);
        rule.advance_frame();
        let x = if permit { -50.0 } else { 0.0 };
        assert_eq!(rects_of(&rule.draw(), RED), vec![Rect::new(x, 0.0, x + 40.0, 40.0)], "{permit}");
        // The transition takes over: the one coming in from where the leaving one was shown.
        state.animate_to(1);
        let mut first = None;
        for _ in 0..4 {
            rule.advance_frame();
            if first.is_none() {
                first = rects_of(&rule.draw(), BLUE).first().copied();
            }
        }
        let first = first.expect("the one coming in drawn");
        assert!(
            if permit { first.left < -20.0 } else { first.left > -1.0 && first.left < 20.0 },
            "{permit}: {first:?}"
        );
        rule.wait_for_idle();
        assert_eq!(rects_of(&rule.draw(), BLUE), vec![Rect::new(200.0, 400.0, 300.0, 500.0)]);
    }
}

fn a_fling_velocity_carries_the_shared_element_on() {
    // prepareTransitionWithInitialVelocity: the transition starts with the fling's momentum, which dies down along
    // the bounds animation's spring: early on the element is further along the fling than without it.
    let mut lefts = Vec::new();
    for velocity in [Velocity::Zero, Velocity::new(3000.0, 0.0)] {
        let details = mutable_state_of(false);
        let states = Rc::new(RefCell::new(Vec::<SharedContentState>::new()));
        let (d, st) = (details.clone(), states.clone());
        let rule = KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (d, st) = (d.clone(), st.clone());
                SharedTransitionLayout(Modifier.fill_max_size(), move |shared: &SharedTransitionScope| {
                    let (shared, st) = (shared.clone(), st.clone());
                    AnimatedContent(
                        d.get(),
                        content = move |s: &AnimatedContentScope, open: bool| {
                            let photo = shared.remember_shared_content_state(SlotKey::new("photo"));
                            if !open {
                                st.borrow_mut().push(photo.clone());
                            }
                            let m = if open {
                                Modifier.offset(0.dp(), 400.dp()).size(40.dp())
                            } else {
                                Modifier.size(40.dp())
                            };
                            let m = shared.shared_element(m, photo, &visibility_scope(s));
                            Box(modifier = m.background(if open { BLUE } else { RED }));
                        },
                    );
                });
            }),
            IntSize::new(400, 800),
        );
        rule.wait_for_idle();
        let photo = states.borrow().last().cloned().expect("the state");
        photo.prepare_transition_with_initial_velocity(velocity);
        details.set(true);
        for _ in 0..5 {
            rule.advance_frame();
        }
        let blue = rects_of(&rule.draw(), BLUE);
        assert_eq!(blue.len(), 1);
        lefts.push(blue[0].left);
        rule.wait_for_idle();
        assert_eq!(rects_of(&rule.draw(), BLUE), vec![Rect::new(0.0, 400.0, 40.0, 440.0)]);
    }
    assert!(near(lefts[0], 0.0, 0.5) && lefts[1] > 10.0, "{lefts:?}");
}

fn a_shared_transition_scope_gives_its_root_modifier() {
    // SharedTransitionScope { modifier -> }: the content lays itself out, its root taking the modifier (the overlay
    // drawn over it, the scope's coordinates its).
    let details = mutable_state_of(false);
    let d = details.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let d = d.clone();
            SharedTransitionScope(move |shared: &SharedTransitionScope, root: Modifier| {
                let (shared, d) = (shared.clone(), d.clone());
                Column(
                    modifier = root.padding(20.dp()).fill_max_size(),
                    content = move || {
                        let shared = shared.clone();
                        AnimatedContent(
                            d.get(),
                            content = move |s: &AnimatedContentScope, open: bool| {
                                let photo = shared.remember_shared_content_state(SlotKey::new("photo"));
                                let m = if open {
                                    Modifier.offset(100.dp(), 100.dp()).size(100.dp())
                                } else {
                                    Modifier.size(40.dp())
                                };
                                let m = shared.shared_element(m, photo, &visibility_scope(s));
                                Box(modifier = m.background(if open { BLUE } else { RED }));
                            },
                        );
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    details.set(true);
    for _ in 0..6 {
        rule.advance_frame();
    }
    let blue = rects_of(&rule.draw(), BLUE);
    assert!(blue.len() == 1 && blue[0].left > 20.0 && blue[0].left < 120.0, "{blue:?}");
    rule.wait_for_idle();
    assert_eq!(rects_of(&rule.draw(), BLUE), vec![Rect::new(120.0, 120.0, 220.0, 220.0)]);
}

tests![
    shared_transition_overlay_draws_in_its_order_and_nodes_above,
    shared_bounds_resize_mode_scales_or_remeasures,
    shared_element_moves_from_its_match_and_hides_it,
    skip_to_lookahead_size_lays_out_where_it_goes,
    a_scrolled_lists_places_are_worked_out_where_they_are_asked_for,
    caller_managed_visibility_moves_the_visible_one_and_hides_the_other,
    placeholder_size_is_what_the_parent_lays_out,
    shared_content_config_enables_and_gives_targets_after_removal,
    shared_content_states_tell_their_match_and_the_scope_its_transition,
    skip_to_lookahead_position_places_at_once_where_it_goes,
    an_interrupted_shared_transition_goes_back_from_where_it_is,
    shared_elements_follow_nested_animated_contents,
    shared_elements_are_marked_by_the_visual_debugging,
    deferred_transitions_move_shared_elements_by_hand_then_hand_over,
    a_fling_velocity_carries_the_shared_element_on,
    a_shared_transition_scope_gives_its_root_modifier,
];

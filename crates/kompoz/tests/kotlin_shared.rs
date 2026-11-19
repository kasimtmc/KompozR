//! Shared elements in Kotlin's syntax, as a Compose user writes them: the C++ Kompoz's kompozable_tests.cpp
//! 2466-2470 (`SharedTransitionLayout(modifier = …) { AnimatedContent(…) { o -> Box(modifier = Modifier…
//! .sharedElement(rememberSharedContentState(key = "photo"), animatedVisibilityScope = this@AnimatedContent)) } }`) and
//! Compose's samples: shared bounds with a resize mode and a placeholder size (SharedElementInAnimatedContentSample),
//! a scope handed down and its member extensions reached by `with(sharedTransitionScope) { }` (the navigation
//! pattern), a FAB in the overlay with `renderInSharedTransitionScopeOverlay(zIndexInOverlay =)` and
//! `animateEnterExit` (SharedElementWithFABInOverlaySample), a clip in the overlay of a shape (`OverlayClip(…)`),
//! `sharedElementWithCallerManagedVisibility`; and in a `.kt` file (kompoz_file!) `SharedTransitionScope { modifier ->
//! }`, `skipToLookaheadSize()`, `isTransitionActive`, `isMatchFound`,
//! `SharedContentConfig(permitTransformDuringDeferredTransition =)`, `animateBounds(this@SharedTransitionScope)`. The
//! same behaviour crates/kompoz-animation/tests/shared.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, RecordingCanvas};

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn exists(rule: &KompozTestRule, tag: &str) -> bool {
    !rule.on_all_nodes_with_tag(tag).fetch_semantics_nodes().is_empty()
}

/// The rects drawn in `color`.
fn rects_of(c: &RecordingCanvas, color: Color) -> Vec<Rect> {
    c.rects.iter().zip(&c.rect_colors).filter(|(_, x)| **x == color).map(|(r, _)| *r).collect()
}

/// Where `color` is first drawn (none: not at all).
fn order(c: &RecordingCanvas, color: Color) -> Option<usize> {
    c.rect_colors.iter().position(|&x| x == color)
}

/// Frames of a change, each drawn (as a host draws them): the last one's drawing.
fn frames(rule: &KompozTestRule, n: usize) -> RecordingCanvas {
    let mut c = rule.draw();
    for _ in 0..n {
        rule.advance_frame();
        c = rule.draw();
    }
    c
}

// ---- kompozable_tests.cpp 2466-2470 -----------------------------------------------------------------------------

kompoz! {
    @Composable
    fun Photo(open: Boolean) {
        SharedTransitionLayout(modifier = Modifier.fillMaxWidth()) {
            AnimatedContent(targetState = open, label = "photo") { o ->
                Box(modifier = Modifier.size(if (o) 80.dp else 40.dp).testTag(if (o) "büyük" else "küçük")
                        .sharedElement(rememberSharedContentState(key = "photo"), animatedVisibilityScope = this@AnimatedContent)
                        .background(if (o) Color.Blue else Color.Red))
            }
        }
    }
}

fn a_shared_element_as_compose_writes_it() {
    let open = mutable_state_of(false);
    let o = open.clone();
    let rule = KompozTestRule::with_viewport(kompozable_lambda!(move || Photo(o.get())), IntSize::new(400, 800));
    rule.wait_for_idle();
    assert!(exists(&rule, "küçük") && !exists(&rule, "büyük"));
    open.set(true);
    let c = frames(&rule, 8);
    // (moving: the big one drawn in the overlay at a size between, the small one not drawn)
    let blue = rects_of(&c, Color::Blue);
    assert!(blue.len() == 1 && blue[0].width() > 40.0 && blue[0].width() < 80.0, "{blue:?}");
    assert!(rects_of(&c, Color::Red).is_empty());
    rule.wait_for_idle();
    assert!(exists(&rule, "büyük") && !exists(&rule, "küçük"));
    assert_eq!(bounds(&rule, "büyük").width(), 80.0);
}

// ---- Compose's samples ------------------------------------------------------------------------------------------

kompoz! {
    // (SharedElementInAnimatedContentSample's shape: bounds shared between a thumbnail and a details screen, their
    // contents remeasured to the bounds; an element inside the details' with a placeholder of the animated size)
    @Composable
    fun Thumbnail(showThumbnail: Boolean) {
        SharedTransitionLayout(modifier = Modifier.fillMaxSize()) {
            AnimatedContent(showThumbnail, transitionSpec = { fadeIn() togetherWith fadeOut() }) {
                if (it) {
                    Box(
                        modifier = Modifier
                            .sharedBounds(
                                rememberSharedContentState(key = "bounds"),
                                this,
                                enter = fadeIn(),
                                exit = fadeOut(),
                                resizeMode = SharedTransitionScope.ResizeMode.RemeasureToBounds,
                            )
                            .size(100.dp)
                            .background(Color.Blue)
                            .testTag("küçük resim")
                    )
                } else {
                    Column(
                        modifier = Modifier
                            .fillMaxSize()
                            .sharedBounds(
                                rememberSharedContentState(key = "bounds"),
                                this@AnimatedContent,
                                resizeMode = SharedTransitionScope.ResizeMode.scaleToBounds(ContentScale.Fit),
                            )
                            .background(Color.Gray)
                            .testTag("ayrıntılar")
                    ) {
                        Box(
                            modifier = Modifier
                                .sharedElement(
                                    rememberSharedContentState(key = "kedi"),
                                    animatedVisibilityScope = this@AnimatedContent,
                                    placeholderSize = SharedTransitionScope.PlaceholderSize.AnimatedSize,
                                )
                                .size(200.dp)
                                .background(Color.Red)
                        )
                    }
                }
            }
        }
    }
}

fn shared_bounds_as_compose_writes_them() {
    let thumbnail = mutable_state_of(true);
    let t = thumbnail.clone();
    let rule = KompozTestRule::with_viewport(kompozable_lambda!(move || Thumbnail(t.get())), IntSize::new(400, 800));
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "küçük resim"), Rect::new(0.0, 0.0, 100.0, 100.0));
    thumbnail.set(false);
    let c = frames(&rule, 6);
    // (both in the moving bounds, in the overlay: the gray details scaled into them from its full size; the blue
    // thumbnail remeasured to them)
    let blue = rects_of(&c, Color::Blue);
    assert!(blue.len() == 1 && blue[0].width() > 100.0 && blue[0].width() < 400.0, "{blue:?}");
    // (laid out at its full size, scaled to fit the bounds, centered in them)
    let gray = rects_of(&c, Color::Gray);
    assert!(gray.len() == 1 && gray[0].width() == 400.0 && gray[0].height() == 800.0, "{gray:?}");
    rule.wait_for_idle();
    assert!(exists(&rule, "ayrıntılar") && !exists(&rule, "küçük resim"));
    assert_eq!(bounds(&rule, "ayrıntılar"), Rect::new(0.0, 0.0, 400.0, 800.0));
}

kompoz! {
    // (the navigation pattern: the scopes handed down, the shared transition scope's member extensions reached by
    // `with(sharedTransitionScope) { }`)
    @Composable
    fun Item(id: Int, big: Boolean, sharedTransitionScope: SharedTransitionScope, animatedVisibilityScope: AnimatedVisibilityScope) {
        with(sharedTransitionScope) {
            Box(
                modifier = Modifier
                    .offset(if (big) 0.dp else (id * 50).dp, if (big) 300.dp else 0.dp)
                    .sharedElement(rememberSharedContentState(key = "öğe $id"), animatedVisibilityScope)
                    .size(if (big) 200.dp else 40.dp)
                    .background(Color.Green)
                    .testTag(if (big) "ayrıntı $id" else "öğe $id")
            )
        }
    }

    @Composable
    fun ListToDetails(selected: Int) {
        SharedTransitionLayout {
            AnimatedContent(selected, label = "sayfa") { s ->
                if (s < 0) {
                    Box { for (id in 0 until 3) Item(id, false, this@SharedTransitionLayout, this@AnimatedContent) }
                } else {
                    Item(s, true, this@SharedTransitionLayout, this@AnimatedContent)
                }
            }
        }
    }
}

fn a_scope_handed_down_as_compose_writes_it() {
    let selected = mutable_state_of(-1);
    let s = selected.clone();
    let rule =
        KompozTestRule::with_viewport(kompozable_lambda!(move || ListToDetails(s.get())), IntSize::new(400, 800));
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "öğe 2"), Rect::new(100.0, 0.0, 140.0, 40.0));
    selected.set(2);
    let c = frames(&rule, 8);
    // (the selected one moves from its place in the list; the others fade out where they are)
    let moving: Vec<Rect> = rects_of(&c, Color::Green).into_iter().filter(|r| r.top > 1.0).collect();
    assert!(moving.len() == 1 && moving[0].left < 100.0 && moving[0].top < 300.0, "{moving:?}");
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "ayrıntı 2"), Rect::new(0.0, 300.0, 200.0, 500.0));
}

kompoz! {
    // (SharedElementWithFABInOverlaySample's shape: a FAB drawn in the overlay over the moving elements, coming in
    // and going by its own enter/exit; a clip of a shape in the overlay)
    @Composable
    fun WithFab(details: Boolean) {
        SharedTransitionLayout(modifier = Modifier.fillMaxSize()) {
            AnimatedContent(targetState = details) { d ->
                Box(modifier = Modifier.fillMaxSize()) {
                    Box(
                        modifier = Modifier
                            .sharedElement(
                                rememberSharedContentState(key = "kart"),
                                animatedVisibilityScope = this@AnimatedContent,
                                clipInOverlayDuringTransition = OverlayClip(RoundedCornerShape(16.dp)),
                            )
                            .size(if (d) 300.dp else 100.dp)
                            .background(Color.Red)
                    )
                    if (!d) {
                        Box(
                            modifier = Modifier
                                .renderInSharedTransitionScopeOverlay(zIndexInOverlay = 1f)
                                .animateEnterExit(enter = fadeIn(), exit = fadeOut())
                                .offset(300.dp, 700.dp)
                                .size(56.dp)
                                .background(Color.Yellow)
                                .testTag("fab")
                        )
                    }
                }
            }
        }
    }
}

fn an_overlay_fab_as_compose_writes_it() {
    let details = mutable_state_of(false);
    let d = details.clone();
    let rule = KompozTestRule::with_viewport(kompozable_lambda!(move || WithFab(d.get())), IntSize::new(400, 800));
    rule.wait_for_idle();
    let c = rule.draw();
    assert!(order(&c, Color::Red) < order(&c, Color::Yellow));
    details.set(true);
    let c = frames(&rule, 6);
    // (moving: the card in the overlay clipped by its rounded shape, the FAB (z 1) over it)
    assert!(order(&c, Color::Red).is_some() && order(&c, Color::Red) < order(&c, Color::Yellow));
    assert!(c.clip_shapes.iter().any(|s| s.path.is_some()));
    rule.wait_for_idle();
    assert!(!exists(&rule, "fab"));
}

kompoz! {
    @Composable
    fun Tabs(first: Boolean) {
        SharedTransitionLayout {
            Row {
                Box(
                    modifier = Modifier
                        .sharedElementWithCallerManagedVisibility(rememberSharedContentState(key = "çizgi"), visible = first)
                        .size(100.dp, 4.dp)
                        .background(Color.Black)
                        .testTag("birinci")
                )
                Box(
                    modifier = Modifier
                        .sharedElementWithCallerManagedVisibility(rememberSharedContentState(key = "çizgi"), visible = !first)
                        .size(100.dp, 4.dp)
                        .background(Color.Black)
                        .testTag("ikinci")
                )
            }
        }
    }
}

fn caller_managed_visibility_as_compose_writes_it() {
    let first = mutable_state_of(true);
    let f = first.clone();
    let rule = KompozTestRule::with_viewport(kompozable_lambda!(move || Tabs(f.get())), IntSize::new(400, 800));
    rule.wait_for_idle();
    assert_eq!(rects_of(&rule.draw(), Color::Black), vec![Rect::new(0.0, 0.0, 100.0, 4.0)]);
    first.set(false);
    let c = frames(&rule, 6);
    let line = rects_of(&c, Color::Black);
    assert!(line.len() == 1 && line[0].left > 5.0 && line[0].left < 95.0, "{line:?}");
    rule.wait_for_idle();
    assert_eq!(rects_of(&rule.draw(), Color::Black), vec![Rect::new(100.0, 0.0, 200.0, 4.0)]);
}

// ---- kompoz_file! ------------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/shared.kt");

fn a_kt_files_shared_transitions() {
    let (open, log) = (mutable_state_of(false), std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new())));
    let (o, l) = (open.clone(), log.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let l = l.clone();
            SharedFile(o.get(), move |line: String| l.borrow_mut().push(line));
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    assert!(log.borrow().iter().all(|l| l == "dinlenme false false"), "{:?}", log.borrow());
    open.set(true);
    let c = frames(&rule, 6);
    // (the transition active and the match found, read in composition)
    assert!(log.borrow().iter().any(|l| l == "açık true true"), "{:?}", log.borrow());
    // (skipToLookaheadSize: the content laid out at its size where it goes; animateBounds in the scope)
    assert_eq!(bounds(&rule, "atlayan iç").width(), 300.0);
    assert!(rects_of(&c, Color::Magenta).len() == 1);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "atlayan iç").width(), 300.0);
    assert_eq!(bounds(&rule, "kayan").left, 200.0);
}

tests![
    a_shared_element_as_compose_writes_it,
    shared_bounds_as_compose_writes_them,
    a_scope_handed_down_as_compose_writes_it,
    an_overlay_fab_as_compose_writes_it,
    caller_managed_visibility_as_compose_writes_it,
    a_kt_files_shared_transitions,
];

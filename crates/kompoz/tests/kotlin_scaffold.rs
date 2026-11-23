//! Material's Scaffold in Kotlin's syntax: the C++ Kompoz's ScaffoldExample (kompozable_tests.cpp's
//! `composes_scaffold_sample`) and material3 1.5's ScaffoldSamples (its snackbar ones in kotlin_lists_sheets.rs) from a
//! `.kt` file (kompoz_file!) (kompozable_tests.cpp's ScrollForms, its TopAppBar and AppBarRow, is whole in
//! kotlin_material_sources.rs) and the C++ Kompoz's Scaffold tests
//! (kompoz_tests.cpp; kompozable_tests.cpp has no source for them) written as a Compose user writes them through
//! `kompoz!`, each checked as crates/kompoz-material3/tests/scaffold.rs and buttons_and_containers.rs check them (M15's
//! values; the differences from the C++'s said there). Density 1: dp are px.

#![allow(non_snake_case, non_upper_case_globals, unused_variables)]

mod common;

#[allow(unused_imports)]
use kompoz::prelude::*;
#[allow(unused_imports)]
use kompoz::test::KompozTestRule;
use kompoz::ui::platform::{Insets, SystemInsets};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ==== scaffold ===================================================================================================

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// Where a node's content is in the root (its inner coordinates).
fn content_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    let inner = rule.on_node_with_tag(tag).layout_node().inner_coordinates();
    inner.find_root_coordinates().local_bounding_box_of(&inner, false)
}

fn insets(left: i32, top: i32, right: i32, bottom: i32) -> Insets {
    Insets::new(left, top, right, bottom)
}

kompoz! {
    // (scaffold_pads_content_by_its_bars_and_insets)
    @Composable
    fun PaddedScaffold(seen: (PaddingValues) -> Unit) {
        Scaffold(
            modifier = Modifier.fillMaxSize(),
            topBar = { TopAppBar(title = { Text("Başlık") }) },
            floatingActionButton = { FloatingActionButton(onClick = {}) { Icon(Icons.Filled.Add, contentDescription = "Ekle") } },
        ) { padding ->
            seen(padding)
            Box(modifier = Modifier.fillMaxSize().padding(padding).testTag("content"))
        }
    }

    // (what_lays_out_by_a_scaffolds_padding_follows_it_without_recomposing: a bar as tall as `bar`, read as it is
    // measured, made taller by a click; a list, a carousel and a button laid out by the padding, a text reading it while
    // composing)
    @Composable
    fun TopPaddingText(padding: PaddingValues) {
        Text("Üst ${padding.calculateTopPadding().value.toInt()}")
    }

    @Composable
    fun FollowingScaffold(onComposed: () -> Unit) {
        var bar by remember { mutableStateOf(50) }
        Scaffold(
            modifier = Modifier.fillMaxSize(),
            topBar = {
                Box(
                    Modifier.fillMaxWidth().layout { measurable, constraints ->
                        val placeable = measurable.measure(Constraints.fixed(constraints.maxWidth, bar))
                        layout(constraints.maxWidth, bar) { placeable.place(0, 0) }
                    }.clickable { bar = 80 }.testTag("çubuk")
                )
            },
        ) { padding ->
            onComposed()
            Column(Modifier.fillMaxSize()) {
                LazyColumn(modifier = Modifier.fillMaxWidth().height(150.dp).testTag("liste"), contentPadding = padding) {
                    items(10) { Box(Modifier.fillMaxWidth().height(40.dp).testTag("l$it")) }
                }
                HorizontalUncontainedCarousel(
                    state = rememberCarouselState { 5 },
                    itemWidth = 100.dp,
                    modifier = Modifier.fillMaxWidth().height(150.dp).testTag("karusel"),
                    contentPadding = padding,
                ) { i -> Box(Modifier.fillMaxSize().testTag("k$i")) }
                Button(onClick = {}, modifier = Modifier.testTag("düğme"), contentPadding = padding) {
                    Text("Düğme", modifier = Modifier.testTag("etiket"))
                }
                TopPaddingText(padding)
            }
        }
    }

    // (a_scaffolds_content_is_measured_with_the_padding_its_bars_give_it_then: the bar and the content's box change
    // together, by one click)
    @Composable
    fun MeasuredScaffold(told: (Float, Int) -> Unit) {
        var bar by remember { mutableStateOf(50) }
        var tall by remember { mutableStateOf(100) }
        Scaffold(
            modifier = Modifier.fillMaxSize(),
            topBar = {
                Box(
                    Modifier.fillMaxWidth().layout { measurable, constraints ->
                        val placeable = measurable.measure(Constraints.fixed(constraints.maxWidth, bar))
                        layout(constraints.maxWidth, bar) { placeable.place(0, 0) }
                    }.clickable { bar = 80; tall = 130 }.testTag("çubuk")
                )
            },
        ) { padding ->
            Box(Modifier.fillMaxSize().padding(padding)) {
                Box(
                    Modifier.fillMaxWidth().layout { measurable, constraints ->
                        val placeable = measurable.measure(Constraints.fixed(constraints.maxWidth, tall))
                        layout(constraints.maxWidth, tall) { placeable.place(0, 0) }
                    }.onGloballyPositioned { told(it.positionInRoot().y, it.size.height) }
                )
            }
        }
    }

    // (scaffold_and_navigation_bar_leave_the_keyboard_to_ime_padding)
    @Composable
    fun KeyboardScaffold(seen: (PaddingValues) -> Unit) {
        Scaffold(
            modifier = Modifier.fillMaxSize(),
            bottomBar = {
                NavigationBar(modifier = Modifier.testTag("bar")) {
                    NavigationBarItem(selected = true, onClick = {}, icon = { Icon(Icons.Filled.Home, contentDescription = null) }, label = { Text("Bir") })
                }
            },
        ) { padding ->
            seen(padding)
            Box(modifier = Modifier.fillMaxSize().padding(padding).consumeWindowInsets(padding).imePadding().testTag("content"))
        }
    }

    // (scaffold_bodies_and_carousel_items_leave_content_its_own_size_as_compose: the Scaffold, the BottomSheetScaffold
    // and the carousel cases)
    @Composable
    fun OwnSizeBodies(case: Int) {
        if (case == 2) {
            HorizontalMultiBrowseCarousel(
                state = rememberCarouselState { 10 },
                preferredItemWidth = 186.dp,
                modifier = Modifier.fillMaxWidth().height(200.dp),
                itemSpacing = 8.dp,
            ) { i ->
                if (i == 0) Box(modifier = Modifier.size(60.dp, 40.dp).testTag("içerik"))
            }
        } else if (case == 1) {
            BottomSheetScaffold(sheetContent = { Text("Ayrıntılar") }, modifier = Modifier.fillMaxSize()) {
                Box(modifier = Modifier.size(60.dp, 40.dp).testTag("içerik"))
            }
        } else {
            Scaffold(modifier = Modifier.fillMaxSize()) {
                Box(modifier = Modifier.size(60.dp, 40.dp).testTag("içerik"))
            }
        }
    }

    // (center_aligned_bar_clickable_cards_fab_sizes_and_positions)
    @Composable
    fun CenterAlignedScaffold(longTitle: Boolean, position: FabPosition, onCard: () -> Unit) {
        Scaffold(
            modifier = Modifier.fillMaxSize(),
            topBar = {
                CenterAlignedTopAppBar(
                    title = {
                        Text(
                            if (longTitle) "Çok çok uzun bir başlık, simgelerin arasına sığmayacak kadar uzun" else "Başlık",
                            modifier = Modifier.testTag("başlık"),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    },
                    navigationIcon = { IconButton(onClick = {}) { Icon(Icons.Filled.Menu, contentDescription = "Menü") } },
                    actions = {
                        IconButton(onClick = {}) { Icon(Icons.Filled.Search, contentDescription = "Ara") }
                        IconButton(onClick = {}) { Icon(Icons.Filled.MoreVert, contentDescription = "Diğer") }
                    },
                )
            },
            bottomBar = { Box(Modifier.fillMaxWidth().height(80.dp)) },
            floatingActionButton = {
                FloatingActionButton(onClick = {}, modifier = Modifier.testTag("fab")) { Icon(Icons.Filled.Add, contentDescription = "Ekle") }
            },
            floatingActionButtonPosition = position,
        ) { padding ->
            Column(Modifier.padding(padding)) {
                Card(onClick = { onCard() }, modifier = Modifier.testTag("kart")) { Text("Kart", modifier = Modifier.padding(16.dp)) }
                Row {
                    SmallFloatingActionButton(onClick = {}, modifier = Modifier.testTag("küçük")) { Icon(Icons.Filled.Add, contentDescription = "Küçük") }
                    LargeFloatingActionButton(onClick = {}, modifier = Modifier.testTag("büyük")) { Icon(Icons.Filled.Add, contentDescription = "Büyük") }
                }
            }
        }
    }
}

type Seen = Rc<RefCell<Option<PaddingValues>>>;

fn seen_padding(seen: &Seen) -> PaddingValues {
    seen.borrow().clone().expect("the content was composed")
}

fn scaffold_pads_content_by_its_bars_and_insets() {
    let seen: Seen = Rc::default();
    let rule = {
        let seen = seen.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let seen = seen.clone();
                PaddedScaffold(move |p: PaddingValues| *seen.borrow_mut() = Some(p));
            }),
            IntSize::new(400, 800),
        )
    };
    rule.host().set_system_insets(SystemInsets {
        status_bars: insets(0, 24, 0, 0),
        navigation_bars: insets(0, 0, 0, 16),
        ..Default::default()
    });
    rule.wait_for_idle();
    let padding = seen_padding(&seen);
    assert!(near(padding.calculate_top_padding().0, 64.0 + 24.0)); // the top bar includes the status bar
    assert!(near(padding.calculate_bottom_padding().0, 16.0)); // no bottom bar: the navigation bar inset
    assert!(near(content_bounds(&rule, "content").top, 88.0));
    // (the icon's own node: the FAB merges its description)
    let icon = kompoz::runtime::named_args!(rule.on_node_with_content_description("Ekle", use_unmerged_tree = true))
        .get_unclipped_bounds_in_root();
    assert!(near(icon.right, 400.0 - 16.0 - 16.0) && near(icon.bottom, 800.0 - 16.0 - 16.0 - 16.0), "{icon:?}");
}

fn what_lays_out_by_a_scaffolds_padding_follows_it_without_recomposing() {
    let composed = Rc::new(Cell::new(0));
    let rule = {
        let composed = composed.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let composed = composed.clone();
                FollowingScaffold(move || composed.set(composed.get() + 1));
            }),
            IntSize::new(400, 1000),
        )
    };
    // How far below its container's top each one's first item (the button's label) is.
    let inset = || {
        let label = rule.on_node_with_tag_unmerged("etiket").get_unclipped_bounds_in_root().top;
        [
            bounds(&rule, "l0").top - bounds(&rule, "liste").top,
            bounds(&rule, "k0").top - bounds(&rule, "karusel").top,
            label - bounds(&rule, "düğme").top,
        ]
    };
    let before = composed.get();
    let was = inset();
    assert!(near(was[0], 50.0) && near(was[1], 50.0), "{was:?}");
    rule.on_node_with_text("Üst 50").assert_exists();
    rule.on_node_with_tag("çubuk").perform_click();
    let now = inset();
    for (now, was) in now.iter().zip(&was) {
        assert!(near(now - was, 30.0), "{now} {was}");
    }
    rule.on_node_with_text("Üst 80").assert_exists();
    assert_eq!(composed.get(), before);
}

fn a_scaffolds_content_is_measured_with_the_padding_its_bars_give_it_then() {
    let told: Rc<RefCell<Vec<(f32, i32)>>> = Rc::default();
    let rule = {
        let told = told.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let told = told.clone();
                MeasuredScaffold(move |y: f32, h: i32| told.borrow_mut().push((y, h)));
            }),
            IntSize::new(400, 800),
        )
    };
    // (each layout tells where the box is: never with the bar's old padding and its own new height, or the other way
    // round — see the Rust test)
    let all = |told: &[(f32, i32)], top: f32, height: i32| told.iter().all(|(y, h)| near(*y, top) && *h == height);
    let first = told.borrow().len();
    assert!(first >= 1 && all(&told.borrow(), 50.0, 100), "{:?}", told.borrow());
    rule.on_node_with_tag("çubuk").perform_click();
    let told = told.borrow();
    assert!(told.len() > first && all(&told[first..], 80.0, 130), "{told:?}");
}

fn scaffold_and_navigation_bar_leave_the_keyboard_to_ime_padding() {
    let seen: Seen = Rc::default();
    let rule = {
        let seen = seen.clone();
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let seen = seen.clone();
                KeyboardScaffold(move |p: PaddingValues| *seen.borrow_mut() = Some(p));
            }),
            IntSize::new(400, 800),
        )
    };
    rule.host().set_system_insets(SystemInsets { navigation_bars: insets(0, 0, 0, 48), ..Default::default() });
    rule.wait_for_idle();
    let bar_height = bounds(&rule, "bar").height();
    assert_eq!(bar_height, 80.0 + 48.0); // (80 dp over the navigation bar's 48)
    assert!(near(seen_padding(&seen).calculate_bottom_padding().0, bar_height));
    assert!(near(content_bounds(&rule, "content").bottom, 800.0 - bar_height));
    rule.host().set_system_insets(SystemInsets {
        navigation_bars: insets(0, 0, 0, 48),
        ime: insets(0, 0, 0, 320),
        ..Default::default()
    });
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "bar").height(), bar_height)); // the keyboard covers it, doesn't lift it
    assert!(near(content_bounds(&rule, "content").bottom, 800.0 - 320.0)); // above the keyboard
}

fn scaffold_bodies_and_carousel_items_leave_content_its_own_size_as_compose() {
    for case in 0..3 {
        let rule =
            KompozTestRule::with_viewport(kompozable_lambda!(move || OwnSizeBodies(case)), IntSize::new(400, 800));
        let b = bounds(&rule, "içerik");
        assert!(near(b.width(), 60.0) && near(b.height(), 40.0), "{case}: {b:?}");
    }
}

fn center_aligned_bar_clickable_cards_fab_sizes_and_positions() {
    let long_title = mutable_state_of(false);
    let position = mutable_state_of(FabPosition::End);
    let clicks = Rc::new(Cell::new(0));
    let rule = {
        let (long_title, position, clicks) = (long_title.clone(), position.clone(), clicks.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let clicks = clicks.clone();
                CenterAlignedScaffold(long_title.get(), position.get(), move || clicks.set(clicks.get() + 1));
            }),
            IntSize::new(400, 800),
        )
    };
    let t = rule.on_node_with_tag_unmerged("başlık").get_unclipped_bounds_in_root();
    assert!((t.center().x - 200.0).abs() <= 1.0, "{t:?}"); // in the middle of the bar, not of what the icons leave
    long_title.set(true);
    rule.wait_for_idle();
    // (M15: in the room the icons leave, moved off the center to end where the wider actions begin)
    let t = rule.on_node_with_tag_unmerged("başlık").get_unclipped_bounds_in_root();
    assert!(near(t.left, 4.0 + 48.0 + 4.0) && near(t.right, 400.0 - 4.0 - 96.0 - 4.0), "{t:?}");
    rule.on_node_with_tag("kart").perform_click();
    assert_eq!(clicks.get(), 1);
    // (a small FAB 40 dp in 48 of room, a large one 96)
    assert!(bounds(&rule, "küçük").width() == 40.0 && bounds(&rule, "büyük").width() == 96.0);
    let fab = bounds(&rule, "fab");
    assert_eq!((fab.left, fab.top), (400.0 - 56.0 - 16.0, 800.0 - 80.0 - 16.0 - 56.0)); // at the end, above the bar
    position.set(FabPosition::Center);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "fab").left, 172.0);
    position.set(FabPosition::Start);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "fab").left, 16.0);
    position.set(FabPosition::EndOverlay); // over the bottom bar
    rule.wait_for_idle();
    let fab = bounds(&rule, "fab");
    assert_eq!((fab.left, fab.top), (400.0 - 56.0 - 16.0, 800.0 - 16.0 - 56.0));
}

// ---- kt/scaffold.kt ----------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/scaffold.kt");

fn composes_scaffold_sample() {
    let rule = KompozTestRule::with_viewport(ScaffoldExample, IntSize::new(400, 800));
    rule.on_node_with_text("Top app bar").assert_exists();
    rule.on_node_with_text("Bottom app bar").assert_exists();
    kompoz::runtime::named_args!(
        rule.on_node_with_text("You have pressed the floating action button 0 times.", substring = true)
    )
    .assert_exists();
    rule.on_node_with_content_description("Add").perform_click();
    rule.on_node_with_content_description("Add").perform_click();
    let node = kompoz::runtime::named_args!(
        rule.on_node_with_text("You have pressed the floating action button 2 times.", substring = true)
    )
    .fetch_semantics_node();
    let text =
        node.config().get(kompoz::ui::semantics::SemanticsProperties::Text).expect("a text")[0].text().to_string();
    // (trimIndent: no indent)
    assert!(text.starts_with("This is an example"), "{text}");
    // The bars' colors: the top one's container the primary container (topAppBarColors imported), the bottom one's
    // too, and the FAB's (its default).
    let cs = light_color_scheme();
    assert_eq!(rule.draw().rect_colors.iter().filter(|c| **c == cs.primary_container).count(), 3);
    // The content under the 64 dp bar, the FAB above the 80 dp bottom bar.
    let text = kompoz::runtime::named_args!(rule.on_node_with_text("This is an example", substring = true))
        .get_unclipped_bounds_in_root();
    assert!(near(text.top, 64.0 + 8.0), "{text:?}");
    let fab = rule.on_node_with_content_description("Add").get_unclipped_bounds_in_root();
    assert!(near(fab.bottom, 800.0 - 80.0 - 16.0) && near(fab.right, 400.0 - 16.0), "{fab:?}");
}

fn simple_scaffold_with_top_bar_sample() {
    let rule = KompozTestRule::with_viewport(SimpleScaffoldWithTopBar, IntSize::new(400, 800));
    rule.on_node_with_text("Simple Scaffold Screen").assert_exists();
    // The FAB at the bottom end; the list's boxes from under the 64 dp bar (its content padding), the first's color
    // the first of the sample's.
    let fab = rule.on_node_with_text("Inc").get_unclipped_bounds_in_root();
    assert!(near(fab.right, 400.0 - 16.0) && near(fab.bottom, 800.0 - 16.0), "{fab:?}");
    let canvas = rule.draw();
    assert!(canvas.rect_colors.contains(&Color::from_argb(0xFFFFD7D7)));
    assert!(canvas.rects.iter().any(|r| near(r.top, 64.0) && near(r.height(), 50.0)), "{:?}", canvas.rects);
}

fn main() {
    common::run(&[
        // ==== scaffold ====
        ("scaffold_pads_content_by_its_bars_and_insets", scaffold_pads_content_by_its_bars_and_insets),
        (
            "what_lays_out_by_a_scaffolds_padding_follows_it_without_recomposing",
            what_lays_out_by_a_scaffolds_padding_follows_it_without_recomposing,
        ),
        (
            "a_scaffolds_content_is_measured_with_the_padding_its_bars_give_it_then",
            a_scaffolds_content_is_measured_with_the_padding_its_bars_give_it_then,
        ),
        (
            "scaffold_and_navigation_bar_leave_the_keyboard_to_ime_padding",
            scaffold_and_navigation_bar_leave_the_keyboard_to_ime_padding,
        ),
        (
            "scaffold_bodies_and_carousel_items_leave_content_its_own_size_as_compose",
            scaffold_bodies_and_carousel_items_leave_content_its_own_size_as_compose,
        ),
        (
            "center_aligned_bar_clickable_cards_fab_sizes_and_positions",
            center_aligned_bar_clickable_cards_fab_sizes_and_positions,
        ),
        ("composes_scaffold_sample", composes_scaffold_sample),
        ("simple_scaffold_with_top_bar_sample", simple_scaffold_with_top_bar_sample),
    ]);
}

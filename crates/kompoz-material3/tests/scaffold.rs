//! Material's scaffold as Compose's material3 (1.5.0-alpha17) has it: the C++ Kompoz's tests by name
//! (kompoz_tests.cpp; M15's behaviour where the C++ differed, said at each), Compose's behaviour (the slots' places,
//! the floating action button's positions, the snackbar above it and the bottom bar, the content's padding from
//! the bars and the window insets, the insets consumed around, the colors, the defaults), M15's ScaffoldSamples
//! (its snackbar ones are in snackbars_tooltips.rs) and the animated FloatingActionButtonSamples, which compose a
//! Scaffold. Density 1: dp are px; the viewport is 400 × 800.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_material3_adaptive_navigation_suite::{NavigationSuiteScaffoldLayout, NavigationSuiteType};
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_ui::layout::LayoutCoordinates;
use kompoz_ui::platform::{Insets, LocalLayoutDirection, SystemInsets};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    // (the C++ Kompoz's)
    scaffold_pads_content_by_its_bars_and_insets,
    what_lays_out_by_a_scaffolds_padding_follows_it_without_recomposing,
    a_scaffolds_content_is_measured_with_the_padding_its_bars_give_it_then,
    scaffold_and_navigation_bar_leave_the_keyboard_to_ime_padding,
    scaffold_bodies_and_carousel_items_leave_content_its_own_size_as_compose,
    // (Compose's behaviour)
    scaffold_places_the_fab_where_its_position_says,
    scaffold_places_the_fab_and_the_snackbar_clear_of_the_insets_at_the_bottom_and_sides,
    scaffold_places_the_snackbar_above_the_fab_or_the_bottom_bar,
    scaffold_mirrors_the_fab_right_to_left,
    scaffold_leaves_out_what_was_consumed_around_it,
    scaffold_takes_the_insets_where_there_is_no_bar,
    scaffold_draws_its_container_and_gives_its_content_color,
    scaffold_slots_compose_again_only_when_one_changes,
    fab_positions_and_scaffold_defaults,
    // (M15's samples)
    simple_scaffold_with_top_bar_sample,
    animated_floating_action_button_sample,
    animated_extended_floating_action_button_samples,
);

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// Where a node's content is in the root (its inner coordinates: C++'s `contentBounds`).
fn content_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    let inner = rule.on_node_with_tag(tag).layout_node().inner_coordinates();
    inner.find_root_coordinates().local_bounding_box_of(&inner, false)
}

fn insets(left: i32, top: i32, right: i32, bottom: i32) -> Insets {
    Insets::new(left, top, right, bottom)
}

/// A box as tall as `height`, read as it is measured (the C++ tests' `sized`): what it lays out by changes without
/// anything composing again.
fn sized(height: MutableState<f32>) -> Modifier {
    Modifier.fill_max_width().layout(move |scope, measurable, c| {
        let h = height.get() as i32;
        let placeable = measurable.measure(Constraints::fixed(c.max_width, h));
        scope.layout(c.max_width, h, move |p| p.place(&placeable, 0, 0))
    })
}

type Seen = Rc<RefCell<Option<PaddingValues>>>;

fn seen_padding(seen: &Seen) -> PaddingValues {
    seen.borrow().clone().expect("the content was composed")
}

// ---- Ported from C++ -------------------------------------------------------------------------------------------

/// Reads the padding while composing (the C++'s `kompozable(CallSite::current(), …)`): composed again when it
/// changes.
#[kompozable]
fn TopPaddingText(padding: PaddingValues) {
    Text(format!("Üst {}", padding.calculate_top_padding().0 as i32));
}

fn scaffold_pads_content_by_its_bars_and_insets() {
    let seen: Seen = Rc::default();
    let rule = {
        let seen = seen.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let seen = seen.clone();
            Scaffold(
                modifier = Modifier.fill_max_size(),
                top_bar = || TopAppBar(title = || Text("Başlık")),
                floating_action_button = || {
                    FloatingActionButton(|| {}, content = || VectorIcon(Icons::Filled::Add(), Some("Ekle".into())));
                },
                content = move |padding: PaddingValues| {
                    *seen.borrow_mut() = Some(padding.clone());
                    Box(modifier = Modifier.fill_max_size().padding_values(padding).test_tag("content"));
                },
            );
        }))
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
    // The icon inside a 56 dp FAB, 16 dp from the end, 16 dp above the navigation bar. (The FAB merges its icon's
    // description: the icon's own node is the unmerged tree's, the C++'s node.)
    let icon = named_args!(rule.on_node_with_content_description("Ekle", use_unmerged_tree = true))
        .get_unclipped_bounds_in_root();
    assert!(near(icon.right, 400.0 - 16.0 - 16.0) && near(icon.bottom, 800.0 - 16.0 - 16.0 - 16.0), "{icon:?}");
    let fab = rule.on_node_with_content_description("Ekle").get_unclipped_bounds_in_root();
    assert!(near(fab.width(), 56.0) && near(fab.right, 400.0 - 16.0) && near(fab.bottom, 800.0 - 16.0 - 16.0));
}

fn what_lays_out_by_a_scaffolds_padding_follows_it_without_recomposing() {
    let bar = mutable_state_of(50.0f32);
    let composed = Rc::new(Cell::new(0));
    let rule = {
        let (bar, composed) = (bar.clone(), composed.clone());
        KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let (bar, composed) = (bar.clone(), composed.clone());
                Scaffold(
                    modifier = Modifier.fill_max_size(),
                    // As tall as `bar`, read as it is measured: the content's padding changes, nothing is recomposed.
                    top_bar = move || Box(modifier = sized(bar.clone())),
                    content = move |padding: PaddingValues| {
                        composed.set(composed.get() + 1);
                        let pager = remember_pager_state(0, || 3);
                        let carousel = remember_carousel_state(item_count = || 5);
                        Column(
                            modifier = Modifier.fill_max_size(),
                            content = move || {
                                LazyVerticalStaggeredGrid(
                                    StaggeredGridCells::Fixed(2),
                                    modifier = Modifier.fill_max_width().height(150.dp()).test_tag("grid"),
                                    content_padding = padding.clone(),
                                    content = |scope: &mut LazyStaggeredGridScope| {
                                        scope.items(10, |i| {
                                            Box(modifier =
                                                Modifier.fill_max_width().height(40.dp()).test_tag(format!("g{i}")));
                                        });
                                    },
                                );
                                HorizontalPager(
                                    pager.clone(),
                                    modifier = Modifier.fill_max_width().height(150.dp()).test_tag("pager"),
                                    content_padding = padding.clone(),
                                    page_content =
                                        |p| Box(modifier = Modifier.fill_max_size().test_tag(format!("p{p}"))),
                                );
                                HorizontalUncontainedCarousel(
                                    carousel.clone(),
                                    100.dp(),
                                    modifier = Modifier.fill_max_width().height(150.dp()).test_tag("carousel"),
                                    content_padding = padding.clone(),
                                    content = |_: &CarouselItemScope, i: i32| {
                                        Box(modifier = Modifier.fill_max_size().test_tag(format!("c{i}")));
                                    },
                                );
                                Button(
                                    || {},
                                    modifier = Modifier.test_tag("button"),
                                    content_padding = padding.clone(),
                                    content = || Text("Düğme", modifier = Modifier.test_tag("label")),
                                );
                                // Read while composing: this one is composed again.
                                TopPaddingText(padding.clone());
                            },
                        );
                    },
                );
            }),
            IntSize::new(400, 1000),
        )
    };
    // How far below its container's top each one's first item (the button's label) is.
    let inset = || {
        let below = |container: &str, item: &str| bounds(&rule, item).top - bounds(&rule, container).top;
        let label = rule.on_node_with_tag_unmerged("label").get_unclipped_bounds_in_root().top;
        [below("grid", "g0"), below("pager", "p0"), below("carousel", "c0"), label - bounds(&rule, "button").top]
    };
    let before = composed.get();
    let was = inset();
    assert!(near(was[0], 50.0) && near(was[1], 50.0) && near(was[2], 50.0), "{was:?}");
    rule.on_node_with_text("Üst 50").assert_exists();
    bar.set(80.0);
    rule.wait_for_idle();
    let now = inset();
    for (now, was) in now.iter().zip(&was) {
        assert!(near(now - was, 30.0), "{now} {was}");
    }
    rule.on_node_with_text("Üst 80").assert_exists();
    assert_eq!(composed.get(), before);
}

fn a_scaffolds_content_is_measured_with_the_padding_its_bars_give_it_then() {
    let bar = mutable_state_of(50.0f32);
    let tall = mutable_state_of(100.0f32);
    let told: Rc<RefCell<Vec<Rect>>> = Rc::default();
    let rule = {
        let (bar, tall, told) = (bar.clone(), tall.clone(), told.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let (bar, tall, told) = (bar.clone(), tall.clone(), told.clone());
            Scaffold(
                modifier = Modifier.fill_max_size(),
                top_bar = move || Box(modifier = sized(bar.clone())),
                content = move |padding: PaddingValues| {
                    let (tall, told) = (tall.clone(), told.clone());
                    Box(
                        modifier = Modifier.fill_max_size().padding_values(padding),
                        content = move || {
                            let told = told.clone();
                            Box(modifier =
                                sized(tall.clone()).on_globally_positioned(move |at: &LayoutCoordinates| {
                                    let p = at.position_in_root();
                                    let s = at.size();
                                    told.borrow_mut().push(Rect::new(
                                        p.x,
                                        p.y,
                                        p.x + s.width as f32,
                                        p.y + s.height as f32,
                                    ));
                                }));
                        },
                    );
                },
            );
        }))
    };
    // Each layout tells where the box is: the scaffold's write of the content's padding as it measures (a state,
    // as Compose's `paddingHolder`) is applied after the frame's layout and lays out what read it once more, with
    // the same values, telling them again (the C++ counted one layout). What matters is the same: the content is
    // never laid out with the bar's old padding and its own new height, or the other way round.
    let all = |told: &[Rect], top: f32, height: f32| told.iter().all(|r| near(r.top, top) && near(r.height(), height));
    let first = told.borrow().len();
    assert!(first >= 1 && all(&told.borrow(), 50.0, 100.0), "{:?}", told.borrow());
    // The bar and the content change together: the content is laid out under the bar as it is now.
    bar.set(80.0);
    tall.set(130.0);
    rule.wait_for_idle();
    let told = told.borrow();
    assert!(told.len() > first && all(&told[first..], 80.0, 130.0), "{told:?}");
}

fn scaffold_and_navigation_bar_leave_the_keyboard_to_ime_padding() {
    let seen: Seen = Rc::default();
    let rule = {
        let seen = seen.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let seen = seen.clone();
            Scaffold(
                modifier = Modifier.fill_max_size(),
                bottom_bar = || {
                    NavigationBar(
                        modifier = Modifier.test_tag("bar"),
                        content = || {
                            NavigationBarItem(
                                true,
                                || {},
                                || VectorIcon(Icons::Filled::Home(), None),
                                label = Some(Rc::new(|| Text("Bir"))),
                            );
                        },
                    );
                },
                content = move |padding: PaddingValues| {
                    *seen.borrow_mut() = Some(padding.clone());
                    Box(modifier = Modifier
                        .fill_max_size()
                        .padding_values(padding.clone())
                        .consume_window_insets_padding_values(padding)
                        .ime_padding()
                        .test_tag("content"));
                },
            );
        }))
    };
    rule.host().set_system_insets(SystemInsets { navigation_bars: insets(0, 0, 0, 48), ..Default::default() });
    rule.wait_for_idle();
    let bar_height = bounds(&rule, "bar").height();
    // (80 dp over the navigation bar's 48)
    assert_eq!(bar_height, 80.0 + 48.0);
    assert!(near(seen_padding(&seen).calculate_bottom_padding().0, bar_height));
    assert!(near(content_bounds(&rule, "content").bottom, 800.0 - bar_height));
    rule.host().set_system_insets(SystemInsets {
        navigation_bars: insets(0, 0, 0, 48),
        ime: insets(0, 0, 0, 320),
        ..Default::default()
    });
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "bar").height(), bar_height)); // the keyboard covers it, doesn't lift it
    assert!(near(seen_padding(&seen).calculate_bottom_padding().0, bar_height));
    assert!(near(content_bounds(&rule, "content").bottom, 800.0 - 320.0)); // above the keyboard
    rule.host().set_system_insets(SystemInsets {
        navigation_bars: insets(0, 0, 0, 48),
        display_cutout: insets(30, 0, 0, 0), // a camera at the side
        ..Default::default()
    });
    rule.wait_for_idle();
    // Compose for desktop's `systemBarsForVisualComponents` is the system bars alone (Android's adds the display
    // cutout, which the C++ followed: a start padding of 30): the content isn't padded for the camera.
    let padding = seen_padding(&seen);
    assert!(near(padding.calculate_start_padding(LayoutDirection::Ltr).0, 0.0));
    assert!(near(bounds(&rule, "bar").height(), bar_height));
}

fn scaffold_bodies_and_carousel_items_leave_content_its_own_size_as_compose() {
    // The Scaffold, BottomSheetScaffold and NavigationSuiteScaffoldLayout cases of the C++ test (its carousel case is in
    // carousel.rs). Compose measures a scaffold's body with no minimum size (the navigation suite's in a Box of its
    // own): content that doesn't fill keeps its size.
    let which = mutable_state_of(0);
    let rule = {
        let which = which.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let body = || Box(modifier = Modifier.size_wh(60.dp(), 40.dp()).test_tag("icerik"));
            let bar = || Box(modifier = Modifier.size_wh(80.dp(), 80.dp())); // a bar or a rail
            match which.get() {
                0 => Scaffold(modifier = Modifier.fill_max_size(), content = move |_: PaddingValues| body()),
                1 => BottomSheetScaffold(
                    sheet_content = || Text("Ayrıntılar"),
                    modifier = Modifier.fill_max_size(),
                    content = move |_: PaddingValues| body(),
                ),
                2 => NavigationSuiteScaffoldLayout(bar, NavigationSuiteType::NavigationBar, content = body),
                3 => NavigationSuiteScaffoldLayout(bar, NavigationSuiteType::NavigationRail, content = body),
                _ => NavigationSuiteScaffoldLayout(bar, NavigationSuiteType::None, content = body),
            }
        }))
    };
    for i in 0..=4 {
        which.set(i);
        rule.wait_for_idle();
        let b = bounds(&rule, "icerik");
        assert!(near(b.width(), 60.0) && near(b.height(), 40.0), "{i}: {b:?}");
    }
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

/// A 56 dp box standing for a floating action button.
#[kompozable]
fn Fab() {
    Box(modifier = Modifier.size(56.dp()).test_tag("fab"));
}

fn scaffold_places_the_fab_where_its_position_says() {
    let position = mutable_state_of(FabPosition::End);
    let with_bar = mutable_state_of(true);
    let rule = {
        let (position, with_bar) = (position.clone(), with_bar.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let with_bar = with_bar.clone();
            Scaffold(
                modifier = Modifier.fill_max_size(),
                bottom_bar = move || {
                    if with_bar.get() {
                        Box(modifier = Modifier.fill_max_width().height(80.dp()));
                    }
                },
                floating_action_button = || Fab(),
                floating_action_button_position = position.get(),
                content = |_: PaddingValues| {},
            );
        }))
    };
    let at = |p: FabPosition| {
        position.set(p);
        rule.wait_for_idle();
        let b = bounds(&rule, "fab");
        (b.left, b.top)
    };
    // 16 dp from the edge, 16 dp above the bottom bar; over it at the end with EndOverlay.
    assert_eq!(at(FabPosition::End), (400.0 - 16.0 - 56.0, 800.0 - 80.0 - 16.0 - 56.0));
    assert_eq!(at(FabPosition::Center), (172.0, 800.0 - 80.0 - 16.0 - 56.0));
    assert_eq!(at(FabPosition::Start), (16.0, 800.0 - 80.0 - 16.0 - 56.0));
    assert_eq!(at(FabPosition::EndOverlay), (400.0 - 16.0 - 56.0, 800.0 - 16.0 - 56.0));
    // No bottom bar: 16 dp above the bottom whatever the position.
    with_bar.set(false);
    assert_eq!(at(FabPosition::End), (400.0 - 16.0 - 56.0, 800.0 - 16.0 - 56.0));
    assert_eq!(at(FabPosition::Center).1, 800.0 - 16.0 - 56.0);
}

fn scaffold_places_the_fab_and_the_snackbar_clear_of_the_insets_at_the_bottom_and_sides() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Scaffold(
            modifier = Modifier.fill_max_size(),
            snackbar_host = || Box(modifier = Modifier.fill_max_width().height(48.dp()).test_tag("snackbar")),
            floating_action_button = || Fab(),
            content_window_insets = WindowInsets::new(10, 20, 30, 40),
            content = |_: PaddingValues| {},
        );
    }));
    let fab = bounds(&rule, "fab");
    // (the right inset and 16 dp from the right; the bottom inset and 16 dp from the bottom)
    assert_eq!((fab.left, fab.top), (400.0 - 30.0 - 16.0 - 56.0, 800.0 - 40.0 - 16.0 - 56.0));
    let snackbar = bounds(&rule, "snackbar");
    // As wide as the room between the side insets, centered between them, above the FAB.
    assert_eq!((snackbar.left, snackbar.width()), (10.0, 400.0 - 10.0 - 30.0));
    assert_eq!(snackbar.bottom, fab.top);
}

fn scaffold_places_the_snackbar_above_the_fab_or_the_bottom_bar() {
    let fab = mutable_state_of(true);
    let bar = mutable_state_of(true);
    let rule = {
        let (fab, bar) = (fab.clone(), bar.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let (fab, bar) = (fab.clone(), bar.clone());
            Scaffold(
                modifier = Modifier.fill_max_size(),
                bottom_bar = move || {
                    if bar.get() {
                        Box(modifier = Modifier.fill_max_width().height(80.dp()));
                    }
                },
                snackbar_host = || Box(modifier = Modifier.size_wh(300.dp(), 48.dp()).test_tag("snackbar")),
                floating_action_button = move || {
                    if fab.get() {
                        Fab();
                    }
                },
                content_window_insets = WindowInsets::new(0, 0, 0, 24),
                content = |_: PaddingValues| {},
            );
        }))
    };
    let snackbar = || bounds(&rule, "snackbar");
    // Above the FAB (whose own top is 16 dp above the bar): centered, its bottom at the FAB's top.
    assert_eq!((snackbar().left, snackbar().bottom), (50.0, 800.0 - 80.0 - 16.0 - 56.0));
    // No FAB: right above the bottom bar.
    fab.set(false);
    rule.wait_for_idle();
    assert_eq!(snackbar().bottom, 800.0 - 80.0);
    // Neither: above the insets at the bottom.
    bar.set(false);
    rule.wait_for_idle();
    assert_eq!(snackbar().bottom, 800.0 - 24.0);
    // A FAB without a bar: the FAB above the insets, the snackbar above it.
    fab.set(true);
    rule.wait_for_idle();
    assert_eq!(snackbar().bottom, 800.0 - 24.0 - 16.0 - 56.0);
}

fn scaffold_mirrors_the_fab_right_to_left() {
    let position = mutable_state_of(FabPosition::End);
    let rule = {
        let position = position.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let position = position.clone();
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
                Scaffold(
                    modifier = Modifier.fill_max_size(),
                    floating_action_button = || Fab(),
                    floating_action_button_position = position.get(),
                    content_window_insets = WindowInsets::new(10, 0, 30, 0),
                    content = |_: PaddingValues| {},
                );
            });
        }))
    };
    // At the end: the left right to left, past the left inset.
    assert_eq!(bounds(&rule, "fab").left, 10.0 + 16.0);
    position.set(FabPosition::Start);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "fab").left, 400.0 - 30.0 - 16.0 - 56.0);
    position.set(FabPosition::Center);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "fab").left, ((400 - 56 + 10 - 30) / 2) as f32);
}

fn scaffold_leaves_out_what_was_consumed_around_it() {
    let seen: Seen = Rc::default();
    let rule = {
        let seen = seen.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let seen = seen.clone();
            Box(
                modifier = Modifier.fill_max_size().status_bars_padding(),
                content = move || {
                    let seen = seen.clone();
                    Scaffold(
                        modifier = Modifier.fill_max_size(),
                        content = move |padding: PaddingValues| {
                            *seen.borrow_mut() = Some(padding.clone());
                            Box(modifier = Modifier.fill_max_size().padding_values(padding).test_tag("content"));
                        },
                    );
                },
            );
        }))
    };
    rule.host().set_system_insets(SystemInsets {
        status_bars: insets(0, 24, 0, 0),
        navigation_bars: insets(0, 0, 0, 16),
        ..Default::default()
    });
    rule.wait_for_idle();
    // The status bar was taken around the scaffold: the content's padding is the navigation bar's at the bottom.
    let padding = seen_padding(&seen);
    assert!(near(padding.calculate_top_padding().0, 0.0) && near(padding.calculate_bottom_padding().0, 16.0));
    let content = content_bounds(&rule, "content");
    assert!(near(content.top, 24.0) && near(content.bottom, 800.0 - 16.0), "{content:?}");
}

fn scaffold_takes_the_insets_where_there_is_no_bar() {
    let with_bars = mutable_state_of(false);
    let seen: Seen = Rc::default();
    let rule = {
        let (with_bars, seen) = (with_bars.clone(), seen.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let (w1, w2, seen) = (with_bars.clone(), with_bars.clone(), seen.clone());
            Scaffold(
                top_bar = move || {
                    if w1.get() {
                        Box(modifier = Modifier.fill_max_width().height(64.dp()));
                    }
                },
                bottom_bar = move || {
                    if w2.get() {
                        Box(modifier = Modifier.fill_max_width().height(80.dp()));
                    }
                },
                content_window_insets = WindowInsets::new(10, 20, 30, 40),
                content = move |padding: PaddingValues| *seen.borrow_mut() = Some(padding),
            );
        }))
    };
    let sides = || {
        let p = seen_padding(&seen);
        let ltr = LayoutDirection::Ltr;
        [
            p.calculate_start_padding(ltr).0,
            p.calculate_top_padding().0,
            p.calculate_end_padding(ltr).0,
            p.calculate_bottom_padding().0,
        ]
    };
    assert_eq!(sides(), [10.0, 20.0, 30.0, 40.0]);
    // The bars take care of the insets at the top and the bottom: the padding is their heights there.
    with_bars.set(true);
    rule.wait_for_idle();
    assert_eq!(sides(), [10.0, 64.0, 30.0, 80.0]);
}

fn scaffold_draws_its_container_and_gives_its_content_color() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let rule = {
        let seen = seen.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            let seen = seen.clone();
            MaterialTheme(
                color_scheme = light_color_scheme(),
                content = move || {
                    let (s1, s2) = (seen.clone(), seen.clone());
                    Column(
                        content = move || {
                            let (s1, s2) = (s1.clone(), s2.clone());
                            Box(
                                modifier = Modifier.size(100.dp()),
                                content = move || {
                                    let s1 = s1.clone();
                                    Scaffold(
                                        content =
                                            move |_: PaddingValues| s1.borrow_mut().push(LocalContentColor.current()),
                                    );
                                },
                            );
                            Box(
                                modifier = Modifier.size(100.dp()),
                                content = move || {
                                    let s2 = s2.clone();
                                    Scaffold(
                                        container_color = MaterialTheme::color_scheme().primary_container,
                                        content =
                                            move |_: PaddingValues| s2.borrow_mut().push(LocalContentColor.current()),
                                    );
                                },
                            );
                        },
                    );
                },
            );
        }))
    };
    let canvas = rule.draw();
    let cs = light_color_scheme();
    // (its background, then the content colors of each container: contentColorFor)
    assert_eq!(canvas.rect_colors, vec![cs.background, cs.primary_container]);
    assert_eq!(*seen.borrow(), vec![cs.on_background, cs.on_primary_container]);
}

fn scaffold_slots_compose_again_only_when_one_changes() {
    let title = mutable_state_of("bir".to_string());
    let tick = mutable_state_of(0);
    let counts = Rc::new([Cell::new(0), Cell::new(0), Cell::new(0)]);
    let rule = {
        let (title, tick, counts) = (title.clone(), tick.clone(), counts.clone());
        KompozTestRule::new(kompozable_lambda!(move || {
            let _ = tick.get();
            let text = title.get();
            let (c1, c2, c3) = (counts.clone(), counts.clone(), counts.clone());
            Scaffold(
                top_bar = move || {
                    c1[0].set(c1[0].get() + 1);
                    Text(text.clone());
                },
                floating_action_button = move || {
                    c2[1].set(c2[1].get() + 1);
                    Fab();
                },
                content = move |_: PaddingValues| c3[2].set(c3[2].get() + 1),
            );
        }))
    };
    let now = || counts.iter().map(Cell::get).collect::<Vec<_>>();
    assert_eq!(now(), vec![1, 1, 1]);
    // Composed again with the same lambdas (each the same memoized closure): the scaffold is skipped.
    tick.set(1);
    rule.wait_for_idle();
    assert_eq!(now(), vec![1, 1, 1]);
    // A new top bar (its closure captures another title): the scaffold's layout composes again, and with it its
    // subcomposing layout, which composes all its slots again (Compose's SubcomposeLayout forces its children to
    // recompose when it isn't skipped: `forceRecomposeChildren`).
    title.set("iki".to_string());
    rule.wait_for_idle();
    rule.on_node_with_text("iki").assert_exists();
    assert_eq!(now(), vec![2, 2, 2]);
}

fn fab_positions_and_scaffold_defaults() {
    assert_eq!(format!("{}", FabPosition::Start), "FabPosition.Start");
    assert_eq!(format!("{:?}", FabPosition::Center), "FabPosition.Center");
    assert_eq!(FabPosition::End.to_string(), "FabPosition.End");
    assert_eq!(FabPosition::EndOverlay.to_string(), "FabPosition.EndOverlay");
    assert_ne!(FabPosition::End, FabPosition::EndOverlay);
    // The default content insets: the system bars (Compose for desktop's `systemBarsForVisualComponents`), read
    // live from the platform's.
    let seen: Rc<RefCell<Option<WindowInsets>>> = Rc::default();
    let rule = {
        let seen = seen.clone();
        KompozTestRule::new(kompozable_lambda!(move || {
            *seen.borrow_mut() = Some(ScaffoldDefaults.content_window_insets());
        }))
    };
    let sides = || {
        let insets = seen.borrow().clone().expect("composed");
        let (d, ltr) = (Density::new(1.0, 1.0), LayoutDirection::Ltr);
        [insets.get_left(&d, ltr), insets.get_top(&d), insets.get_right(&d, ltr), insets.get_bottom(&d)]
    };
    assert_eq!(sides(), [0, 0, 0, 0]);
    rule.host().set_system_insets(SystemInsets {
        status_bars: insets(0, 24, 0, 0),
        navigation_bars: insets(0, 0, 0, 16),
        ime: insets(0, 0, 0, 300),
        caption_bar: insets(0, 30, 0, 0),
        display_cutout: insets(20, 0, 0, 0),
        ..Default::default()
    });
    rule.wait_for_idle();
    // (the status bar's and the caption bar's union at the top, the navigation bar at the bottom; not the keyboard,
    // nor the display cutout — Android's actual has it)
    assert_eq!(sides(), [0, 30, 0, 16]);
}

// ---- M15's ScaffoldSamples.kt (its snackbar samples: snackbars_tooltips.rs) ------------------------------------

/// `SimpleScaffoldWithTopBar`.
#[kompozable]
#[allow(clippy::mixed_case_hex_literals)] // (as the sample writes them)
fn SimpleScaffoldWithTopBar() {
    let colors = vec![
        Color::from_argb(0xFFffd7d7),
        Color::from_argb(0xFFffe9d6),
        Color::from_argb(0xFFfffbd0),
        Color::from_argb(0xFFe3ffd9),
        Color::from_argb(0xFFd0fff8),
    ];

    Scaffold(
        top_bar = || {
            TopAppBar(
                title = || Text("Simple Scaffold Screen"),
                navigation_icon = || {
                    IconButton(
                        on_click = || { /* "Open nav drawer" */ },
                        content = || {
                            VectorIcon(
                                Icons::Filled::Menu(),
                                content_description = Some("Localized description".into()),
                            );
                        },
                    );
                },
            );
        },
        floating_action_button_position = FabPosition::End,
        floating_action_button = || {
            ExtendedFloatingActionButton(on_click = || { /* fab click handler */ }, content = || Text("Inc"));
        },
        content = move |inner_padding: PaddingValues| {
            let colors = colors.clone();
            LazyColumn(
                // consume insets as scaffold doesn't do it by default
                modifier = Modifier.consume_window_insets_padding_values(inner_padding.clone()).test_tag("liste"),
                content_padding = inner_padding,
                content = move |scope: &mut LazyListScope| {
                    let colors = colors.clone();
                    scope.items(100, move |it| {
                        let color = colors[it as usize % colors.len()];
                        Box(modifier =
                            Modifier.fill_max_width().height(50.dp()).background(color).test_tag(format!("kutu{it}")));
                    });
                },
            );
        },
    );
}

fn simple_scaffold_with_top_bar_sample() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| SimpleScaffoldWithTopBar()));
    rule.on_node_with_text("Simple Scaffold Screen").assert_exists();
    // The list's first box right under the 64 dp bar (the content padding), the FAB at the bottom end.
    assert_eq!(bounds(&rule, "kutu0").top, 64.0);
    let fab = rule.on_node_with_text("Inc").get_unclipped_bounds_in_root();
    assert!(fab.right == 400.0 - 16.0 && fab.bottom == 800.0 - 16.0, "{fab:?}");
    // Scrolled, the boxes pass under the bar.
    rule.on_node_with_tag("liste").perform_scroll_to_index(10);
    rule.wait_for_idle();
    assert!(bounds(&rule, "kutu10").top <= 64.0);
}

// ---- M15's samples (FloatingActionButtonSamples.kt: the animated ones compose a Scaffold) ----------------------

/// The samples' list: 100 items, its first visible item telling the FAB to show or expand.
#[kompozable]
fn SampleList(list_state: LazyListState) {
    LazyColumn(
        state = list_state,
        modifier = Modifier.fill_max_size().test_tag("liste"),
        content = |scope: &mut LazyListScope| {
            for index in 0..100 {
                scope.item(move || Text(format!("List item - {index}"), modifier = Modifier.padding(24.dp())));
            }
        },
    );
}

/// `AnimatedFloatingActionButtonSample`.
#[kompozable]
fn AnimatedFloatingActionButtonSample() {
    let list_state = remember_lazy_list_state();
    // The FAB is initially shown. Upon scrolling past the first item we hide the FAB by using a
    // remembered derived state to minimize unnecessary compositions.
    let s = list_state.clone();
    let fab_visible = remember(move || derived_state_of(move || s.first_visible_item_index() == 0));
    Scaffold(
        floating_action_button = move || {
            let fab_visible = fab_visible.clone();
            // A FAB should have a tooltip associated with it.
            TooltipBox(
                position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                tooltip = |scope: &dyn TooltipScope| scope.PlainTooltip(content = || Text("Localized description")),
                state = remember_tooltip_state(),
                content = move || {
                    MediumFloatingActionButton(
                        modifier = named_args!(Modifier.test_tag("fab").animate_floating_action_button(
                            visible = fab_visible.value(),
                            alignment = Alignment::BottomEnd,
                        )),
                        on_click = || { /* do something */ },
                        content = || {
                            VectorIcon(
                                Icons::Filled::Add(),
                                Some("Localized description".to_string()),
                                modifier = Modifier.size(FloatingActionButtonDefaults::MediumIconSize),
                            );
                        },
                    );
                },
            );
        },
        floating_action_button_position = FabPosition::End,
        content = move |_: PaddingValues| SampleList(list_state.clone()),
    );
}

fn animated_floating_action_button_sample() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| AnimatedFloatingActionButtonSample()));
    // At the bottom end, 16 dp in: 80 dp (the medium FAB).
    let fab = bounds(&rule, "fab");
    assert_eq!((fab.right, fab.bottom), (400.0 - 16.0, 800.0 - 16.0));
    assert_eq!(fab.width(), 80.0);
    rule.on_node_with_tag("liste").perform_scroll_to_index(10);
    rule.advance_time_by(2000);
    // Hidden: scaled to nothing towards its bottom end, faded out, taking no room (so the scaffold has no FAB to
    // place).
    assert_eq!(rule.on_node_with_tag("fab").layout_node().size(), IntSize::new(0, 0));
    rule.on_node_with_tag("liste").perform_scroll_to_index(0);
    rule.advance_time_by(2000);
    let back = bounds(&rule, "fab");
    assert_eq!((back.width(), back.right, back.bottom), (80.0, 400.0 - 16.0, 800.0 - 16.0));
}

/// `AnimatedExtendedFloatingActionButtonSample` and its Small / Medium / Large twins (`size` 0 to 3): expanded while
/// the list's first item is the first visible one, a tooltip telling it once collapsed.
#[kompozable]
fn AnimatedExtendedFloatingActionButtonSampleOf(size: i32) {
    let list_state = remember_lazy_list_state();
    // The FAB is initially expanded. Once the first visible item is past the first item we
    // collapse the FAB. We use a remembered derived state to minimize unnecessary compositions.
    let s = list_state.clone();
    let expanded_fab = remember(move || derived_state_of(move || s.first_visible_item_index() == 0));
    Scaffold(
        floating_action_button = move || {
            let (e1, e2) = (expanded_fab.clone(), expanded_fab.clone());
            // A collapsed FAB should have a tooltip associated with it.
            TooltipBox(
                position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                tooltip = move |scope: &dyn TooltipScope| {
                    if !e1.value() {
                        scope.PlainTooltip(content = || Text("Localized description"));
                    }
                },
                state = remember_tooltip_state(),
                content = move || {
                    let expanded = e2.value();
                    let modifier = Modifier.test_tag("fab");
                    match size {
                        0 => TextIconExtendedFloatingActionButton(
                            on_click = || { /* do something */ },
                            expanded = expanded,
                            icon = || VectorIcon(Icons::Filled::Add(), Some("Localized Description".to_string())),
                            text = || Text(text = "Extended FAB"),
                            modifier = modifier.clone(),
                        ),
                        1 => TextIconSmallExtendedFloatingActionButton(
                            on_click = || { /* do something */ },
                            expanded = expanded,
                            icon = || VectorIcon(Icons::Filled::Add(), Some("Localized Description".to_string())),
                            text = || Text(text = "Small Extended FAB"),
                            modifier = modifier.clone(),
                        ),
                        2 => TextIconMediumExtendedFloatingActionButton(
                            on_click = || { /* do something */ },
                            expanded = expanded,
                            icon = || {
                                VectorIcon(
                                    Icons::Filled::Add(),
                                    Some("Localized Description".to_string()),
                                    modifier = Modifier.size(FloatingActionButtonDefaults::MediumIconSize),
                                )
                            },
                            text = || Text(text = "Medium Extended FAB"),
                            modifier = modifier.clone(),
                        ),
                        _ => TextIconLargeExtendedFloatingActionButton(
                            on_click = || { /* do something */ },
                            expanded = expanded,
                            icon = || {
                                VectorIcon(
                                    Icons::Filled::Add(),
                                    Some("Localized Description".to_string()),
                                    modifier = Modifier.size(FloatingActionButtonDefaults::LargeIconSize),
                                )
                            },
                            text = || Text(text = "Large Extended FAB"),
                            modifier = modifier.clone(),
                        ),
                    }
                },
            );
        },
        floating_action_button_position = FabPosition::End,
        content = move |_: PaddingValues| SampleList(list_state.clone()),
    );
}

fn animated_extended_floating_action_button_samples() {
    // (collapsed: the size of the container's height, square)
    for (size, height) in [(0, 56.0), (1, 56.0), (2, 80.0), (3, 96.0)] {
        let rule = KompozTestRule::new(kompozable_lambda!(move || AnimatedExtendedFloatingActionButtonSampleOf(size)));
        let fab = bounds(&rule, "fab");
        assert!(fab.width() > height && fab.height() == height, "{size}: {fab:?}");
        assert_eq!((fab.right, fab.bottom), (400.0 - 16.0, 800.0 - 16.0));
        rule.on_node_with_tag("liste").perform_scroll_to_index(10);
        rule.advance_time_by(2000);
        let collapsed = bounds(&rule, "fab");
        assert!(near(collapsed.width(), height) && collapsed.right == 400.0 - 16.0, "{size}: {collapsed:?}");
        rule.on_node_with_tag("liste").perform_scroll_to_index(0);
        rule.advance_time_by(2000);
        assert!(near(bounds(&rule, "fab").width(), fab.width()), "{size}");
    }
}

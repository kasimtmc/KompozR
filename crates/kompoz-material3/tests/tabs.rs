//! Material's tabs and tab rows as Compose's material3 (1.5.0-alpha17) has them: the C++ Kompoz's tests by name
//! (kompoz_tests.cpp), Compose's behaviour (the tab heights and baselines, the indicator scope's layout and offset,
//! the positions, the deprecated rows and indicator, disabled tabs, the defaults), and the M15 samples of
//! TabSamples.kt.

#![allow(non_snake_case, non_upper_case_globals, deprecated)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_animation::{Animatable, AnimationVector1D, VectorConverter, animate_color_as_state, spring};
use kompoz_coroutines::remember_coroutine_scope;
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_foundation::prelude::*;
use kompoz_foundation::scroll::ScrollState;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::{KompozTestRule, RecordingCanvas};
use kompoz_text::TextOverflow;
use kompoz_ui::input::PointerType;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::Role;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

tests!(
    tab_row_indicator_slides_to_the_selected_tab,
    tabs_as_compose_1_4,
    secondary_and_scrollable_tab_rows,
    rtl_tab_rows_keep_their_indicators_under_the_selected_tab,
    double_measured_children_settle,
    tabs_are_48_or_72_dp_by_their_text_and_icon,
    tab_indicator_layout_is_given_the_tabs_positions,
    tab_positions_tell_their_edges,
    scrollable_rows_take_their_paddings_and_least_tab_width,
    disabled_tabs_take_no_clicks_and_tell_their_interactions,
    tab_row_defaults_follow_the_tokens,
    deprecated_tab_rows_give_their_indicator_the_positions,
    tab_samples,
    fancy_tab_samples,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// A node's bounds in the unmerged tree (a tab's parts: merged into the tab).
fn part_bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn text_bounds(rule: &KompozTestRule, text: &str) -> Rect {
    rule.on_node_with_text(text).get_unclipped_bounds_in_root()
}

/// A nullable composable slot (Kotlin's `(@Composable () -> Unit)?`).
fn slot(f: impl Fn() + 'static) -> Option<Rc<dyn Fn()>> {
    Some(Rc::new(f))
}

/// A tab row's indicator (Kotlin's `indicator: @Composable TabIndicatorScope.() -> Unit`).
fn indicator(f: impl Fn(&TabIndicatorScope) + 'static) -> Option<Rc<dyn Fn(&TabIndicatorScope)>> {
    Some(Rc::new(f))
}

/// A deprecated tab row's indicator (Kotlin's `indicator: @Composable (tabPositions: List<TabPosition>) -> Unit`).
fn positions_indicator(f: impl Fn(Vec<TabPosition>) + 'static) -> Option<Rc<dyn Fn(Vec<TabPosition>)>> {
    Some(Rc::new(f))
}

/// The rects drawn `height` tall (the 3 dp indicators).
fn rects_of_height(canvas: &RecordingCanvas, height: f32) -> Vec<Rect> {
    canvas.rects.iter().copied().filter(|r| near(r.height(), height, 0.01)).collect()
}

/// The color `text` was drawn in.
fn color_of(canvas: &RecordingCanvas, text: &str) -> Option<Color> {
    canvas.texts.iter().position(|t| t == text).map(|i| canvas.colors[i])
}

// ---- The C++ Kompoz's tests ------------------------------------------------------------------------------------

fn tab_row_indicator_slides_to_the_selected_tab() {
    let selected = mutable_state_of(0);
    let s = selected.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            PrimaryTabRow(
                s.get(),
                modifier = Modifier.test_tag("tabs"),
                tabs = move || {
                    for (i, title) in ["Bir", "İkinci", "Üç"].into_iter().enumerate() {
                        let s2 = s.clone();
                        TextIconTab(s.get() == i as i32, move || s2.set(i as i32), text = slot(move || Text(title)));
                    }
                },
            );
        }),
        IntSize::new(300, 800),
    );
    let indicator = || rects_of_height(&rule.draw(), 3.0).first().copied().expect("the indicator");
    // Tabs are 100 wide; the indicator is as wide as the text (FakeTextMeasurer: 7px per char at 14, and
    // titleSmall's 0.1 sp after each).
    let first = indicator();
    assert!(near(first.width(), 24.0, 0.01) && near(first.left, 50.0 - 12.0, 0.01), "{first:?}"); // "Bir" is 21.3 wide: at least 24
    rule.on_node_with_text("İkinci").perform_click();
    rule.wait_for_idle();
    let second = indicator();
    // (the indicator measured at whole pixels, as Compose rounds its width: 43, centered in the tab)
    assert!(near(second.width(), 42.6, 0.5) && near(second.left, 150.0 - 21.3, 1.0), "{second:?}");
    assert!(near(bounds(&rule, "tabs").height(), 48.0, 0.01));
}

fn tabs_as_compose_1_4() {
    let selected = mutable_state_of(0);
    let scheme = Rc::new(Cell::new(light_color_scheme()));
    let (s, sc) = (selected.clone(), scheme.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        sc.set(MaterialTheme::color_scheme());
        let (s, cs) = (s.clone(), MaterialTheme::color_scheme());
        Column(move || {
            let s = s.clone();
            PrimaryTabRow(
                s.get(),
                modifier = Modifier.test_tag("üst"),
                tabs = move || {
                    let (s0, s1) = (s.clone(), s.clone());
                    TextIconTab(
                        s.get() == 0,
                        move || s0.set(0),
                        text = slot(|| Text("Bir")),
                        icon = slot(|| Box(Modifier.size(24.dp()).test_tag("simge"))),
                        modifier = Modifier.test_tag("ilk"),
                    );
                    TextIconTab(
                        s.get() == 1,
                        move || s1.set(1),
                        text = slot(|| Text("İki")),
                        selected_content_color = cs.tertiary,
                        unselected_content_color = Some(cs.on_surface_variant),
                    );
                },
            );
            PrimaryTabRow(0, tabs = || TextIconTab(false, || {}, text = slot(|| Text("Yalnız"))));
        });
    }));
    let cs = scheme.get();
    // With an icon: 72 dp, the text's baseline 17 dp above the bottom, the icon 20 dp above it.
    let (tab, icon) = (bounds(&rule, "ilk"), part_bounds(&rule, "simge"));
    let canvas = rule.draw();
    let baseline = canvas.baselines[canvas.texts.iter().position(|t| t == "Bir").expect("Bir drawn")].y;
    assert!(near(tab.height(), 72.0, 0.01), "{tab:?}");
    assert!(
        near(baseline, tab.top + 72.0 - 17.0, 0.5) && near(icon.top + 24.0 + 20.0, baseline, 0.5),
        "{baseline} {icon:?}"
    );
    // Colors: the row's (primary), unselected ones too unless told; eased.
    assert_eq!(color_of(&canvas, "Bir"), Some(cs.primary));
    assert_eq!(color_of(&canvas, "Yalnız"), Some(cs.primary));
    assert_eq!(color_of(&canvas, "İki"), Some(cs.on_surface_variant));
    let indicator_x =
        || rects_of_height(&rule.draw(), 3.0).into_iter().find(|r| r.top < 80.0).map_or(0.0, |r| r.center().x);
    let from = indicator_x();
    selected.set(1);
    for _ in 0..3 {
        rule.advance_frame();
    }
    let easing = color_of(&rule.draw(), "İki");
    assert!(easing != Some(cs.tertiary) && easing != Some(cs.on_surface_variant), "{easing:?}");
    // The indicator slides on the default spatial spring.
    for _ in 0..4 {
        rule.advance_frame();
    }
    let at = indicator_x();
    rule.wait_for_idle();
    let to = indicator_x();
    let progress = (at - from) / (to - from); // the default spatial spring's two thirds by then
    assert!(progress > 0.6 && progress < 0.85, "{progress}");
    // The ripple in the selected color, though it isn't selected.
    let second = text_bounds(&rule, "İki");
    selected.set(0);
    rule.wait_for_idle();
    rule.host().pointer_down(second.center(), -1, 0, PointerType::Touch);
    rule.advance_frame();
    for _ in 0..4 {
        rule.advance_frame(); // (the ripple fades in)
    }
    let pressed = rule.draw();
    // (the ripple's circle, clipped to the tab as Compose's bounded ripple: a rect or a path)
    let tertiary_ripple = pressed.rect_colors.iter().chain(&pressed.path_colors).any(|c| {
        c.alpha() < 0.2
            && c.alpha() > 0.0
            && c.red() == cs.tertiary.red()
            && c.green() == cs.tertiary.green()
            && c.blue() == cs.tertiary.blue()
    });
    assert!(
        tertiary_ripple,
        "{:?} {:?}",
        pressed.rect_colors.iter().chain(&pressed.path_colors).filter(|c| c.alpha() < 1.0).collect::<Vec<_>>(),
        cs.tertiary
    );
    rule.host().pointer_up(second.center(), -1, 0);
    rule.wait_for_idle();
    let tabs = rule.host().accessibility_tree().iter().filter(|n| n.role == Some(Role::Tab)).count();
    assert_eq!(tabs, 3);
}

fn secondary_and_scrollable_tab_rows() {
    let selected = mutable_state_of(0);
    let chosen = mutable_state_of(0);
    let scroll = ScrollState::new(0);
    let (s, c, sc) = (selected.clone(), chosen.clone(), scroll.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (s, c, sc) = (s.clone(), c.clone(), sc.clone());
            Column(move || {
                let s = s.clone();
                SecondaryTabRow(
                    s.get(),
                    tabs = move || {
                        for (i, title) in ["Bir", "İkinci", "Üç"].into_iter().enumerate() {
                            let s2 = s.clone();
                            TextIconTab(
                                s.get() == i as i32,
                                move || s2.set(i as i32),
                                text = slot(move || Text(title)),
                            );
                        }
                    },
                );
                let c = c.clone();
                PrimaryScrollableTabRow(
                    c.get(),
                    modifier = Modifier.test_tag("scrolling"),
                    scroll_state = sc.clone(),
                    tabs = move || {
                        for i in 0..10 {
                            let c2 = c.clone();
                            TextIconTab(
                                c.get() == i,
                                move || c2.set(i),
                                text = slot(move || Text(format!("Sekme {i}"))),
                            );
                        }
                    },
                );
            });
        }),
        IntSize::new(300, 800),
    );
    let cs = light_color_scheme();
    // Secondary: a 3 dp indicator under the whole tab; the tabs in onSurface (unselected ones too, as Compose's).
    let under = rects_of_height(&rule.draw(), 3.0)[0];
    assert!(near(under.left, 0.0, 0.01) && near(under.width(), 100.0, 0.01), "{under:?}");
    rule.on_node_with_text("İkinci").perform_click();
    rule.wait_for_idle();
    let canvas = rule.draw();
    let under = rects_of_height(&canvas, 3.0)[0];
    assert!(near(under.left, 100.0, 0.01) && near(under.width(), 100.0, 0.01), "{under:?}");
    assert!(color_of(&canvas, "İkinci") == Some(cs.on_surface) && color_of(&canvas, "Bir") == Some(cs.on_surface));
    // Scrollable: each tab as wide as its text and padding (at least 90 dp), after 52 dp.
    assert!(near(text_bounds(&rule, "Sekme 0").center().x, 52.0 + 45.0, 0.5));
    assert!(near(text_bounds(&rule, "Sekme 1").center().x, 52.0 + 90.0 + 45.0, 0.5));
    assert_eq!(color_of(&canvas, "Sekme 0"), Some(cs.primary));
    // A tab selected from code scrolls into the middle of the view.
    chosen.set(5);
    rule.wait_for_idle();
    assert!(near(scroll.value() as f32, 52.0 + 5.0 * 90.0 + 45.0 - 150.0, 0.5), "{}", scroll.value());
    assert!(near(text_bounds(&rule, "Sekme 5").center().x, 150.0, 0.5));
    chosen.set(9); // ...or as far as it goes
    rule.wait_for_idle();
    assert!(scroll.value() == scroll.max_value() && scroll.max_value() == 52 * 2 + 10 * 90 - 300, "{}", scroll.value());
    let height = bounds(&rule, "scrolling").height();
    // and it scrolls by hand
    rule.on_node_with_tag("scrolling").perform_touch_input(move |t| {
        t.swipe_with(Offset::new(50.0, height / 2.0), Offset::new(250.0, height / 2.0), 200)
    });
    rule.wait_for_idle();
    assert!(scroll.value() < scroll.max_value() - 150, "{}", scroll.value());
}

fn rtl_tab_rows_keep_their_indicators_under_the_selected_tab() {
    let selected = mutable_state_of(0);
    let s = selected.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let s = s.clone();
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], move || {
                let s = s.clone();
                Column(move || {
                    let s = s.clone();
                    // (Compose's row takes the selected tab's position: a row of three can't be told 7 — Compose
                    // throws —, so this row stays at its last tab where the C++ shared the 7)
                    PrimaryTabRow(
                        s.get().min(2),
                        modifier = Modifier.test_tag("sekmeler"),
                        tabs = move || {
                            for i in 0..3 {
                                let s2 = s.clone();
                                TextIconTab(
                                    s.get() == i,
                                    move || s2.set(i),
                                    text = slot(move || Text(format!("S{i}"))),
                                    modifier = Modifier.test_tag(format!("s{i}")),
                                );
                            }
                        },
                    );
                    let s = s.clone();
                    PrimaryScrollableTabRow(
                        s.get(),
                        tabs = move || {
                            for i in 0..12 {
                                let s2 = s.clone();
                                TextIconTab(
                                    s.get() == i,
                                    move || s2.set(i),
                                    text = slot(move || Text(format!("Şehir {i}"))),
                                    modifier = Modifier.test_tag(format!("k{i}")),
                                );
                            }
                        },
                    );
                });
            });
        }),
        IntSize::new(300, 600),
    );
    let indicators = || rects_of_height(&rule.draw(), 3.0); // the 3 dp tall rounded bars
    let centered = |bar: Rect, tab: Rect| near(bar.center().x, tab.center().x, 1.0);
    let bars = indicators();
    assert!(bars.len() == 2 && centered(bars[0], bounds(&rule, "s0")), "{bars:?}");
    assert!(bounds(&rule, "s0").left > 150.0); // the first tab at the right
    selected.set(2);
    rule.wait_for_idle();
    let bars = indicators();
    assert!(
        bars.len() == 2 && centered(bars[0], bounds(&rule, "s2")) && centered(bars[1], bounds(&rule, "k2")),
        "{bars:?}"
    );
    selected.set(7); // a scrollable row brings the tab into the middle
    rule.wait_for_idle();
    let k7 = bounds(&rule, "k7");
    let bars = indicators();
    // (the first row keeps its indicator under its last tab; the C++'s, told of a tab it hadn't, drew none)
    assert!(near(k7.center().x, 150.0, 2.0) && bars.len() == 2 && centered(bars[1], k7), "{k7:?} {bars:?}");
}

fn double_measured_children_settle() {
    // C++'s double_measured_children_settle, its Material half (kompoz-animation's bounds.rs has the rest: an
    // animated size in a row of IntrinsicSize.Min): a PrimaryTabRow measures its tabs twice (asked their intrinsic
    // height, then at an equal share of the width). The C++'s tab was a BoxWithConstraints choosing its text by the
    // width given; Compose's row asks its tabs' intrinsic sizes, which a BoxWithConstraints can't give (Compose
    // throws), so the tab here is a layout choosing what it shows by its constraints as it measures. Ten frames later
    // nothing composes again or waits.
    let widths = Rc::new(RefCell::new(Vec::<i32>::new()));
    let w = widths.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let w = w.clone();
        Column(move || {
            let w = w.clone();
            PrimaryTabRow(
                0,
                tabs = move || {
                    let w = w.clone();
                    Tab(
                        true,
                        || {},
                        content = move || {
                            let w = w.clone();
                            Text(
                                "geniş",
                                modifier = Modifier.layout(move |scope, measurable, constraints| {
                                    w.borrow_mut().push(constraints.max_width);
                                    let placeable = measurable.measure(constraints);
                                    scope.layout(placeable.width(), placeable.height(), move |p| {
                                        p.place(&placeable, 0, 0)
                                    })
                                }),
                            );
                        },
                    );
                    TextIconTab(false, || {}, text = slot(|| Text("İki")));
                },
            );
        });
    }));
    rule.wait_for_idle();
    let before = rule.host().stats().recompositions;
    let measured = widths.borrow().len();
    for _ in 0..10 {
        rule.advance_frame();
    }
    assert_eq!(rule.host().stats().recompositions, before);
    assert!(!rule.host().has_pending_work());
    assert_eq!(widths.borrow().len(), measured); // nor measures again
    assert!(widths.borrow().iter().any(|w| *w > 50)); // measured at its share of the width
}

// ---- Compose's behaviour ---------------------------------------------------------------------------------------

fn tabs_are_48_or_72_dp_by_their_text_and_icon() {
    // TabBaselineLayout: a text or an icon alone in 48 dp, centered; both in 72 dp, the text's last baseline 14 dp
    // above the indicator's top (6 dp for two lines), the icon 20 sp above the first baseline.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Column(|| {
                PrimaryTabRow(
                    0,
                    tabs = || {
                        TextIconTab(true, || {}, text = slot(|| Text("Yazı")), modifier = Modifier.test_tag("yazı"));
                        TextIconTab(
                            false,
                            || {},
                            icon = slot(|| Box(Modifier.size(24.dp()).test_tag("simge"))),
                            modifier = Modifier.test_tag("simgeli"),
                        );
                    },
                );
                PrimaryTabRow(
                    0,
                    tabs = || {
                        TextIconTab(
                            true,
                            || {},
                            text = slot(|| Text("İki\nsatır")),
                            icon = slot(|| Box(Modifier.size(24.dp()).test_tag("üst simge"))),
                            modifier = Modifier.test_tag("iki"),
                        );
                    },
                );
            });
        }),
        IntSize::new(300, 800),
    );
    let (text_tab, icon_tab) = (bounds(&rule, "yazı"), bounds(&rule, "simgeli"));
    assert!(near(text_tab.height(), 48.0, 0.01) && near(icon_tab.height(), 48.0, 0.01));
    let icon = part_bounds(&rule, "simge");
    assert!(near(icon.center().y, icon_tab.center().y, 0.5) && near(icon.center().x, icon_tab.center().x, 0.5));
    let text = text_bounds(&rule, "Yazı");
    assert!(near(text.center().y, text_tab.center().y, 0.5) && near(text.center().x, text_tab.center().x, 0.5));
    // Two lines and an icon: taller than 72 dp if the parts need it; the last baseline 6 + 3 dp above the bottom.
    let two = bounds(&rule, "iki");
    let canvas = rule.draw();
    let last_line = canvas.texts.iter().rposition(|t| t.contains("satır")).expect("the second line");
    let last_baseline = canvas.baselines[last_line].y;
    assert!(two.height() >= 72.0 && near(last_baseline, two.bottom - 9.0, 0.5), "{two:?} {last_baseline}");
    let above = part_bounds(&rule, "üst simge");
    assert!(near(above.center().x, two.center().x, 0.5) && above.top >= two.top - 0.5);
}

fn tab_indicator_layout_is_given_the_tabs_positions() {
    // A custom indicator laid out by `tab_indicator_layout`: it is given the tabs' positions (left, width, content
    // width; the content less 16 dp a side, at least 24) and the constraints of the row's indicator.
    let seen = Rc::new(RefCell::new(Vec::<TabPosition>::new()));
    let selected = mutable_state_of(1);
    let (sn, s) = (seen.clone(), selected.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (sn, s) = (sn.clone(), s.clone());
            let index = s.get();
            SecondaryTabRow(
                index,
                indicator = indicator(move |scope: &TabIndicatorScope| {
                    let sn = sn.clone();
                    Box(scope
                        .tab_indicator_layout(Modifier, move |m, measurable, constraints, positions| {
                            *sn.borrow_mut() = positions.clone();
                            let tab = positions[index as usize];
                            let width = m.round_to_px(tab.content_width());
                            let placeable = measurable.measure(Constraints::fixed(width, 6));
                            let x = m.round_to_px(tab.left()) + (m.round_to_px(tab.width()) - width) / 2;
                            m.layout(constraints.max_width, 6, move |p| p.place(&placeable, x, 0))
                        })
                        .background(Color::Red, std::sync::Arc::new(RectangleShape))
                        .test_tag("gösterge"));
                }),
                tabs = move || {
                    for (i, title) in ["A", "Uzun bir başlık"].into_iter().enumerate() {
                        let s2 = s.clone();
                        TextIconTab(s.get() == i as i32, move || s2.set(i as i32), text = slot(move || Text(title)));
                    }
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let positions = seen.borrow().clone();
    assert_eq!(positions.len(), 2);
    assert!(near(positions[0].left().0, 0.0, 0.01) && near(positions[0].width().0, 200.0, 0.01));
    assert!(near(positions[0].content_width().0, 24.0, 0.01)); // "A" is 7.1 wide: at least 24
    assert!(near(positions[1].left().0, 200.0, 0.01) && near(positions[1].right().0, 400.0, 0.01));
    assert!(near(positions[1].content_width().0, 15.0 * 7.0 + 14.0 * 0.1, 1.0), "{:?}", positions[1]);
    let indicator = rects_of_height(&rule.draw(), 6.0)[0];
    assert!(near(indicator.center().x, 300.0, 0.5) && near(indicator.width(), positions[1].content_width().0, 0.5));
    selected.set(0);
    rule.wait_for_idle();
    let indicator = rects_of_height(&rule.draw(), 6.0)[0];
    assert!(near(indicator.center().x, 100.0, 0.5) && near(indicator.width(), 24.0, 0.5), "{indicator:?}");
}

fn tab_positions_tell_their_edges() {
    let seen = Rc::new(RefCell::new(Vec::<TabPosition>::new()));
    let sn = seen.clone();
    let _rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let sn = sn.clone();
            PrimaryTabRow(
                0,
                indicator = indicator(move |scope: &TabIndicatorScope| {
                    let sn = sn.clone();
                    Box(scope.tab_indicator_layout(Modifier, move |m, measurable, constraints, positions| {
                        *sn.borrow_mut() = positions;
                        let placeable = measurable.measure(constraints);
                        m.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, 0, 0))
                    }));
                }),
                tabs = || {
                    TextIconTab(true, || {}, text = slot(|| Text("Bir")));
                    TextIconTab(false, || {}, text = slot(|| Text("İki")));
                },
            );
        }),
        IntSize::new(200, 400),
    );
    let positions = seen.borrow().clone();
    let second = positions[1];
    assert_eq!(second.right(), second.left() + second.width());
    assert_eq!(second, positions[1]);
    assert_ne!(positions[0], positions[1]);
    assert_eq!(
        format!("{second:?}"),
        format!(
            "TabPosition(left={:?}, right={:?}, width={:?}, contentWidth={:?})",
            Dp(100.0),
            Dp(200.0),
            Dp(100.0),
            Dp(24.0)
        )
    );
}

fn scrollable_rows_take_their_paddings_and_least_tab_width() {
    // `edge_padding` before the first and after the last, `min_tab_width` the least a tab takes; the secondary row
    // in the secondary colors with the line under the whole tab.
    let scroll = ScrollState::new(0);
    let sc = scroll.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            SecondaryScrollableTabRow(
                0,
                scroll_state = sc.clone(),
                edge_padding = 10.dp(),
                min_tab_width = 120.dp(),
                modifier = Modifier.test_tag("satır"),
                tabs = || {
                    for i in 0..5 {
                        TextIconTab(
                            i == 0,
                            || {},
                            text = slot(move || Text(format!("T{i}"))),
                            modifier = Modifier.test_tag(format!("t{i}")),
                        );
                    }
                },
            );
        }),
        IntSize::new(300, 400),
    );
    let (t0, t1) = (bounds(&rule, "t0"), bounds(&rule, "t1"));
    assert!(near(t0.left, 10.0, 0.01) && near(t0.width(), 120.0, 0.01) && near(t1.left, 130.0, 0.01), "{t0:?} {t1:?}");
    assert_eq!(scroll.max_value(), 10 * 2 + 5 * 120 - 300);
    let canvas = rule.draw();
    let cs = light_color_scheme();
    let line = rects_of_height(&canvas, 3.0)[0];
    assert!(near(line.left, 10.0, 0.01) && near(line.width(), 120.0, 0.01), "{line:?}");
    assert_eq!(color_of(&canvas, "T1"), Some(cs.on_surface));
}

fn disabled_tabs_take_no_clicks_and_tell_their_interactions() {
    let clicks = Rc::new(Cell::new(0));
    let enabled = mutable_state_of(true);
    let source = MutableInteractionSource::new();
    let pressed = Rc::new(Cell::new(0));
    let (c, e, src, p) = (clicks.clone(), enabled.clone(), source.clone(), pressed.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, e, src) = (c.clone(), e.clone(), src.clone());
        if src.as_interaction_source().collect_is_pressed_as_state().value() {
            p.set(p.get() + 1);
        }
        PrimaryTabRow(
            0,
            tabs = move || {
                let c = c.clone();
                Tab(
                    false,
                    move || c.set(c.get() + 1),
                    enabled = e.get(),
                    interaction_source = Some(src.clone()),
                    modifier = Modifier.test_tag("sekme"),
                    content = || Text("Sekme"),
                );
                let c = c.clone();
                LeadingIconTab(
                    false,
                    move || c.set(c.get() + 10),
                    text = || Text("Öncü"),
                    icon = || Box(Modifier.size(24.dp())),
                    enabled = e.get(),
                );
            },
        );
    }));
    rule.on_node_with_tag("sekme").perform_click();
    rule.on_node_with_text("Öncü").perform_click();
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 11);
    assert!(pressed.get() >= 1);
    enabled.set(false);
    rule.wait_for_idle();
    rule.on_node_with_tag("sekme").perform_click();
    rule.on_node_with_text("Öncü").perform_click();
    rule.wait_for_idle();
    assert_eq!(clicks.get(), 11);
    rule.on_node_with_tag("sekme").assert_is_not_enabled();
}

fn tab_row_defaults_follow_the_tokens() {
    let seen = Rc::new(RefCell::new(Vec::<Color>::new()));
    let sn = seen.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            sn.borrow_mut().extend([
                TabRowDefaults.primary_container_color(),
                TabRowDefaults.secondary_container_color(),
                TabRowDefaults.primary_content_color(),
                TabRowDefaults.secondary_content_color(),
                TabRowDefaults.container_color(),
                TabRowDefaults.content_color(),
            ]);
            Column(|| {
                Box(
                    Modifier.size_wh(100.dp(), 10.dp()),
                    content = || {
                        TabRowDefaults.PrimaryIndicator(modifier = Modifier.test_tag("birincil"));
                    },
                );
                Box(
                    Modifier.size_wh(100.dp(), 10.dp()),
                    content = || {
                        TabRowDefaults.SecondaryIndicator(modifier = Modifier.test_tag("ikincil"));
                    },
                );
                Box(
                    Modifier.size_wh(100.dp(), 10.dp()),
                    content = || {
                        TabRowDefaults.Indicator(
                            modifier = Modifier.test_tag("eski"),
                            height = 4.dp(),
                            color = Color::Red,
                        );
                    },
                );
            });
        }),
        IntSize::new(300, 300),
    );
    let cs = light_color_scheme();
    assert_eq!(*seen.borrow(), [cs.surface, cs.surface, cs.primary, cs.on_surface, cs.surface, cs.primary]);
    assert_eq!(TabRowDefaults::ScrollableTabRowMinTabWidth, Dp(90.0));
    assert_eq!(TabRowDefaults::ScrollableTabRowEdgeStartPadding, Dp(52.0));
    // The primary indicator: 24 × 3 dp, rounded, primary; the secondary one across; the old one as asked.
    assert_eq!(bounds(&rule, "birincil").size(), Size::new(24.0, 3.0));
    assert_eq!(bounds(&rule, "ikincil").size(), Size::new(100.0, 3.0));
    assert_eq!(bounds(&rule, "eski").size(), Size::new(100.0, 4.0));
    let canvas = rule.draw();
    let bar =
        canvas.rects.iter().position(|r| near(r.width(), 24.0, 0.01) && near(r.height(), 3.0, 0.01)).expect("bar");
    assert!(
        canvas.rect_colors[bar] == cs.primary && near(canvas.rect_radii[bar], 1.5, 0.01),
        "{}",
        canvas.rect_radii[bar]
    );
    let line =
        canvas.rects.iter().position(|r| near(r.width(), 100.0, 0.01) && near(r.height(), 3.0, 0.01)).expect("line");
    assert_eq!(canvas.rect_colors[line], cs.primary);
    assert!(canvas.rects.iter().zip(&canvas.rect_colors).any(|(r, c)| near(r.height(), 4.0, 0.01) && *c == Color::Red));
}

fn deprecated_tab_rows_give_their_indicator_the_positions() {
    // Compose's deprecated TabRow and ScrollableTabRow: their indicator over the whole row, given the positions; the
    // default one a SecondaryIndicator offset to the selected tab by the deprecated `tab_indicator_offset`, sliding.
    let selected = mutable_state_of(0);
    let given = Rc::new(RefCell::new(Vec::<TabPosition>::new()));
    let (s, g) = (selected.clone(), given.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (s, g) = (s.clone(), g.clone());
            Column(move || {
                let s0 = s.clone();
                TabRow(
                    s.get(),
                    modifier = Modifier.test_tag("sabit"),
                    tabs = move || {
                        for i in 0..3 {
                            let s2 = s0.clone();
                            TextIconTab(
                                s0.get() == i,
                                move || s2.set(i),
                                text = slot(move || Text(format!("Sabit {i}"))),
                            );
                        }
                    },
                );
                let (s1, g) = (s.clone(), g.clone());
                ScrollableTabRow(
                    s.get(),
                    modifier = Modifier.test_tag("kayan"),
                    indicator = positions_indicator(move |positions: Vec<TabPosition>| {
                        *g.borrow_mut() = positions.clone();
                        TabRowDefaults.Indicator(
                            modifier = TabRowDefaults.tab_indicator_offset(Modifier, positions[0]),
                            height = 5.dp(),
                        );
                    }),
                    tabs = move || {
                        for i in 0..3 {
                            let s2 = s1.clone();
                            TextIconTab(
                                s1.get() == i,
                                move || s2.set(i),
                                text = slot(move || Text(format!("Kayan {i}"))),
                            );
                        }
                    },
                );
            });
        }),
        IntSize::new(300, 600),
    );
    let fixed = bounds(&rule, "sabit");
    assert!(near(fixed.height(), 48.0, 0.01));
    let line = rects_of_height(&rule.draw(), 3.0)[0];
    assert!(near(line.left, 0.0, 0.01) && near(line.width(), 100.0, 0.01), "{line:?}");
    // The scrolling one: tabs at least 90 dp after 52, the indicator given where they are.
    let positions = given.borrow().clone();
    assert!(
        positions.len() == 3 && near(positions[0].left().0, 52.0, 0.01) && near(positions[0].width().0, 90.0, 0.01)
    );
    let custom = rects_of_height(&rule.draw(), 5.0)[0];
    assert!(near(custom.left, 52.0, 0.01) && near(custom.width(), 90.0, 0.01), "{custom:?}");
    selected.set(2);
    rule.advance_frame();
    rule.advance_frame();
    rule.advance_frame();
    let moving = rects_of_height(&rule.draw(), 3.0)[0];
    assert!(moving.left > 0.0 && moving.left < 200.0, "{moving:?}"); // on its way
    rule.wait_for_idle();
    let line = rects_of_height(&rule.draw(), 3.0)[0];
    assert!(near(line.left, 200.0, 0.01) && near(line.width(), 100.0, 0.01), "{line:?}");
}

// ---- The M15 samples (TabSamples.kt) ---------------------------------------------------------------------------

/// The titles of the text samples.
const Titles: [&str; 3] = ["Tab 1", "Tab 2", "Tab 3 with lots of text"];

/// The titles of the scrolling samples.
const ScrollingTitles: [&str; 10] = [
    "Tab 1",
    "Tab 2",
    "Tab 3 with lots of text",
    "Tab 4",
    "Tab 5",
    "Tab 6 with lots of text",
    "Tab 7",
    "Tab 8",
    "Tab 9 with lots of text",
    "Tab 10",
];

#[kompozable]
fn SelectedText(text: String) {
    Text(
        text,
        modifier = Modifier.align(Alignment::CenterHorizontally),
        style = MaterialTheme::typography().body_large,
    );
}

#[kompozable]
fn PrimaryTextTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        PrimaryTabRow(
            s.get(),
            tabs = move || {
                for (index, title) in Titles.into_iter().enumerate() {
                    let s2 = s.clone();
                    TextIconTab(
                        s.get() == index as i32,
                        move || s2.set(index as i32),
                        text = slot(move || Text(title, max_lines = 2, overflow = TextOverflow::Ellipsis)),
                    );
                }
            },
        );
        SelectedText(format!("Primary tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn PrimaryIconTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        PrimaryTabRow(
            s.get(),
            tabs = move || {
                for index in 0..3 {
                    let s2 = s.clone();
                    let selected = s.get() == index;
                    // Icon-only tab should have a tooltip associated with it.
                    TooltipBox(
                        TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                        |scope: &DynTooltipScope| scope.PlainTooltip(content = || Text("Favorite")),
                        remember_tooltip_state(),
                        content = move || {
                            let s2 = s2.clone();
                            TextIconTab(
                                selected,
                                move || s2.set(index),
                                icon = slot(|| VectorIcon(Icons::Filled::Favorite(), Some("Favorite".into()))),
                            );
                        },
                    );
                }
            },
        );
        SelectedText(format!("Icon tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn SecondaryTextTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        SecondaryTabRow(
            s.get(),
            tabs = move || {
                for (index, title) in Titles.into_iter().enumerate() {
                    let s2 = s.clone();
                    TextIconTab(
                        s.get() == index as i32,
                        move || s2.set(index as i32),
                        text = slot(move || Text(title, max_lines = 2, overflow = TextOverflow::Ellipsis)),
                    );
                }
            },
        );
        SelectedText(format!("Secondary tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn TextTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        PrimaryTabRow(
            s.get(),
            tabs = move || {
                for (index, title) in Titles.into_iter().enumerate() {
                    let s2 = s.clone();
                    TextIconTab(
                        s.get() == index as i32,
                        move || s2.set(index as i32),
                        text = slot(move || Text(title, max_lines = 2, overflow = TextOverflow::Ellipsis)),
                    );
                }
            },
        );
        SelectedText(format!("Text tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn SecondaryIconTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        SecondaryTabRow(
            s.get(),
            tabs = move || {
                for index in 0..3 {
                    let s2 = s.clone();
                    let selected = s.get() == index;
                    // Icon-only tab should have a tooltip associated with it.
                    TooltipBox(
                        TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                        |scope: &DynTooltipScope| scope.PlainTooltip(content = || Text("Favorite")),
                        remember_tooltip_state(),
                        content = move || {
                            let s2 = s2.clone();
                            TextIconTab(
                                selected,
                                move || s2.set(index),
                                icon = slot(|| VectorIcon(Icons::Filled::Favorite(), Some("Favorite".into()))),
                            );
                        },
                    );
                }
            },
        );
        SelectedText(format!("Icon tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn TextAndIconTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        PrimaryTabRow(
            s.get(),
            tabs = move || {
                for (index, title) in Titles.into_iter().enumerate() {
                    let s2 = s.clone();
                    TextIconTab(
                        s.get() == index as i32,
                        move || s2.set(index as i32),
                        text = slot(move || Text(title, max_lines = 2, overflow = TextOverflow::Ellipsis)),
                        icon = slot(|| VectorIcon(Icons::Filled::Favorite(), None)),
                    );
                }
            },
        );
        SelectedText(format!("Text and icon tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn LeadingIconTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        PrimaryTabRow(
            s.get(),
            tabs = move || {
                for (index, title) in ["Tab", "Tab & icon", "Tab 3 with lots of text"].into_iter().enumerate() {
                    let s2 = s.clone();
                    LeadingIconTab(
                        s.get() == index as i32,
                        move || s2.set(index as i32),
                        text = move || {
                            BadgedBox(
                                || Badge(modifier = Modifier, content = slot(|| Text("999+"))),
                                content = move || Text(title),
                            );
                        },
                        icon = || VectorIcon(Icons::Filled::Favorite(), None),
                    );
                }
            },
        );
        SelectedText(format!("Leading icon tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn ScrollingPrimaryTextTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        PrimaryScrollableTabRow(
            s.get(),
            tabs = move || {
                for (index, title) in ScrollingTitles.into_iter().enumerate() {
                    let s2 = s.clone();
                    TextIconTab(
                        s.get() == index as i32,
                        move || s2.set(index as i32),
                        text = slot(move || Text(title)),
                    );
                }
            },
        );
        SelectedText(format!("Scrolling primary tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn ScrollingSecondaryTextTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        SecondaryScrollableTabRow(
            s.get(),
            tabs = move || {
                for (index, title) in ScrollingTitles.into_iter().enumerate() {
                    let s2 = s.clone();
                    TextIconTab(
                        s.get() == index as i32,
                        move || s2.set(index as i32),
                        text = slot(move || Text(title)),
                    );
                }
            },
        );
        SelectedText(format!("Scrolling secondary tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn ScrollingTextTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        PrimaryScrollableTabRow(
            s.get(),
            tabs = move || {
                for (index, title) in ScrollingTitles.into_iter().enumerate() {
                    let s2 = s.clone();
                    TextIconTab(
                        s.get() == index as i32,
                        move || s2.set(index as i32),
                        text = slot(move || Text(title)),
                    );
                }
            },
        );
        SelectedText(format!("Scrolling text tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn FancyTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        SecondaryTabRow(
            s.get(),
            tabs = move || {
                for (index, title) in ["Tab 1", "Tab 2", "Tab 3"].into_iter().enumerate() {
                    let s2 = s.clone();
                    FancyTab(title.to_string(), move || s2.set(index as i32), index as i32 == s.get());
                }
            },
        );
        SelectedText(format!("Fancy tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn FancyIndicatorTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        let index = s.get();
        SecondaryTabRow(
            index,
            indicator = indicator(move |scope: &TabIndicatorScope| {
                FancyIndicator(
                    MaterialTheme::color_scheme().primary,
                    modifier = scope.tab_indicator_offset(Modifier, index),
                );
            }),
            tabs = move || {
                for (index, title) in ["Tab 1", "Tab 2", "Tab 3"].into_iter().enumerate() {
                    let s2 = s.clone();
                    TextIconTab(
                        s.get() == index as i32,
                        move || s2.set(index as i32),
                        text = slot(move || Text(title)),
                    );
                }
            },
        );
        SelectedText(format!("Fancy indicator tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn FancyIndicatorContainerTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        let index = s.get();
        SecondaryTabRow(
            index,
            indicator =
                indicator(move |scope: &TabIndicatorScope| FancyAnimatedIndicatorWithModifier(scope.clone(), index)),
            tabs = move || {
                for (index, title) in ["Tab 1", "Tab 2", "Tab 3"].into_iter().enumerate() {
                    let s2 = s.clone();
                    TextIconTab(
                        s.get() == index as i32,
                        move || s2.set(index as i32),
                        text = slot(move || Text(title)),
                    );
                }
            },
        );
        SelectedText(format!("Fancy transition tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn FancyIndicator(color: Color, #[default(Modifier)] modifier: Modifier) {
    // Draws a rounded rectangular with border around the Tab, with a 5.dp padding from the edges
    // Color is passed in as a parameter [color]
    Box(modifier
        .padding(5.dp())
        .fill_max_size()
        .border_stroke(BorderStroke(2.dp(), color), std::sync::Arc::new(RoundedCornerShape(5.dp()))));
}

/// Kotlin's `TabIndicatorScope.FancyAnimatedIndicatorWithModifier(index, isScrollable)`: the scope (a value) first.
#[kompozable]
fn FancyAnimatedIndicatorWithModifier(scope: TabIndicatorScope, index: i32, #[default(false)] is_scrollable: bool) {
    let colors = [
        MaterialTheme::color_scheme().primary,
        MaterialTheme::color_scheme().secondary,
        MaterialTheme::color_scheme().tertiary,
    ];
    let start_animatable = remember(|| Rc::new(RefCell::new(None::<Animatable<Dp, AnimationVector1D>>)));
    let end_animatable = remember(|| Rc::new(RefCell::new(None::<Animatable<Dp, AnimationVector1D>>)));
    let coroutine_scope = remember_coroutine_scope();
    let indicator_color = animate_color_as_state(colors[index as usize % colors.len()], label = String::new());

    Box(scope
        .tab_indicator_layout(Modifier, move |m, measurable, constraints, tab_positions| {
            let new_start = tab_positions[index as usize].left();
            let new_end = tab_positions[index as usize].right();
            let start_anim = start_animatable
                .borrow_mut()
                .get_or_insert_with(|| Animatable((new_start, Dp::VectorConverter)))
                .clone();
            let end_anim =
                end_animatable.borrow_mut().get_or_insert_with(|| Animatable((new_end, Dp::VectorConverter))).clone();

            if end_anim.target_value() != new_end {
                let (anim, spec) = (
                    end_anim.clone(),
                    if end_anim.target_value() < new_end {
                        named_args!(spring(damping_ratio = 1.0, stiffness = 1000.0))
                    } else {
                        named_args!(spring(damping_ratio = 1.0, stiffness = 50.0))
                    },
                );
                coroutine_scope.launch(async move {
                    named_args!(anim.animate_to(new_end, animation_spec = std::sync::Arc::new(spec))).await;
                });
            }

            if start_anim.target_value() != new_start {
                let (anim, spec) = (
                    start_anim.clone(),
                    // Handle directionality here, if we are moving to the right, we
                    // want the right side of the indicator to move faster, if we are
                    // moving to the left, we want the left side to move faster.
                    if start_anim.target_value() < new_start {
                        named_args!(spring(damping_ratio = 1.0, stiffness = 50.0))
                    } else {
                        named_args!(spring(damping_ratio = 1.0, stiffness = 1000.0))
                    },
                );
                coroutine_scope.launch(async move {
                    named_args!(anim.animate_to(new_start, animation_spec = std::sync::Arc::new(spec))).await;
                });
            }

            let indicator_end = m.round_to_px(end_anim.value());
            let indicator_start = m.round_to_px(start_anim.value());

            // Apply an offset from the start to correctly position the indicator around the tab
            let placeable = measurable.measure(named_args!(
                constraints
                    .copy(max_width = indicator_end - indicator_start, min_width = indicator_end - indicator_start,)
            ));
            let content_width = m.round_to_px(tab_positions[index as usize].content_width());
            let tab_width = m.round_to_px(tab_positions[index as usize].width());
            m.layout(constraints.max_width, constraints.max_height, move |p| {
                let relative_offset = if is_scrollable { (tab_width - content_width) / 2 } else { 0 };
                p.place(&placeable, indicator_start - relative_offset, 0);
            })
        })
        .padding(5.dp())
        .fill_max_size()
        .draw_with_content(move |d| {
            let (radius, width) = (d.to_px(5.dp()), d.to_px(2.dp()));
            d.draw_round_rect(
                indicator_color.value(),
                corner_radius = CornerRadius(radius),
                style = DrawStyle::Stroke(Stroke(width = width)),
            );
        }));
}

#[kompozable]
fn ScrollingFancyIndicatorContainerTabs() {
    let state = remember(|| mutable_state_of(0));
    let s = state.clone();
    Column(move || {
        let s = s.clone();
        let index = s.get();
        SecondaryScrollableTabRow(
            index,
            indicator = indicator(move |scope: &TabIndicatorScope| {
                FancyAnimatedIndicatorWithModifier(scope.clone(), index, is_scrollable = true)
            }),
            tabs = move || {
                for (index, title) in ScrollingTitles.into_iter().enumerate() {
                    let s2 = s.clone();
                    TextIconTab(
                        s.get() == index as i32,
                        move || s2.set(index as i32),
                        text = slot(move || Text(title)),
                    );
                }
            },
        );
        SelectedText(format!("Scrolling fancy transition tab {} selected", s.get() + 1));
    });
}

#[kompozable]
fn FancyTab(title: String, on_click: impl Fn() + 'static, selected: bool) {
    Tab(
        selected,
        on_click,
        content = move || {
            let title = title.clone();
            Column(
                modifier = Modifier.padding(10.dp()).height(50.dp()).fill_max_width(),
                vertical_arrangement = Arrangement::SpaceBetween,
                content = move || {
                    Box(Modifier.size(10.dp()).align(Alignment::CenterHorizontally).background(
                        if selected {
                            MaterialTheme::color_scheme().primary
                        } else {
                            MaterialTheme::color_scheme().background
                        },
                        std::sync::Arc::new(RectangleShape),
                    ));
                    Text(
                        title.clone(),
                        style = MaterialTheme::typography().body_large,
                        modifier = Modifier.align(Alignment::CenterHorizontally),
                    );
                },
            );
        },
    );
}

/// Composes `sample`, clicks the tab `click` and checks the text `expect` then shows.
fn check_sample(name: &str, sample: impl Content + 'static, viewport: IntSize, click: &str, expect: &str) {
    let rule = KompozTestRule::with_viewport(sample, viewport);
    rule.on_node_with_text(&expect.replace(" 2 ", " 1 ")).assert_exists();
    let tab = rule.on_node_with_text(click);
    tab.perform_click();
    rule.wait_for_idle();
    assert!(rule.on_node_with_text(expect).exists(), "{name}: {expect}");
}

fn tab_samples() {
    let size = IntSize::new(400, 800);
    check_sample("PrimaryTextTabs", PrimaryTextTabs, size, "Tab 2", "Primary tab 2 selected");
    check_sample("SecondaryTextTabs", SecondaryTextTabs, size, "Tab 2", "Secondary tab 2 selected");
    check_sample("TextTabs", TextTabs, size, "Tab 2", "Text tab 2 selected");
    check_sample("TextAndIconTabs", TextAndIconTabs, size, "Tab 2", "Text and icon tab 2 selected");
    check_sample("LeadingIconTabs", LeadingIconTabs, size, "Tab & icon", "Leading icon tab 2 selected");
    check_sample(
        "ScrollingPrimaryTextTabs",
        ScrollingPrimaryTextTabs,
        size,
        "Tab 2",
        "Scrolling primary tab 2 selected",
    );
    check_sample(
        "ScrollingSecondaryTextTabs",
        ScrollingSecondaryTextTabs,
        size,
        "Tab 2",
        "Scrolling secondary tab 2 selected",
    );
    check_sample("ScrollingTextTabs", ScrollingTextTabs, size, "Tab 2", "Scrolling text tab 2 selected");
    // The icon samples: three Favorite icons, each in a tooltip box.
    let icon_samples: [fn() -> KompozTestRule; 2] = [
        || KompozTestRule::with_viewport(PrimaryIconTabs, IntSize::new(400, 800)),
        || KompozTestRule::with_viewport(SecondaryIconTabs, IntSize::new(400, 800)),
    ];
    for make in icon_samples {
        let text = "Icon tab";
        let rule = make();
        rule.on_node_with_text(&format!("{text} 1 selected")).assert_exists();
        let icons = rule.on_all_nodes_with_content_description("Favorite").fetch_semantics_nodes();
        assert_eq!(icons.len(), 3);
        let third = icons[2].bounds_in_root().center();
        rule.host().pointer_down(third, -1, 0, PointerType::Touch);
        rule.host().pointer_up(third, -1, 0);
        rule.wait_for_idle();
        rule.on_node_with_text(&format!("{text} 3 selected")).assert_exists();
    }
    // The leading icon tabs' badges stay within their tabs.
    let rule = KompozTestRule::with_viewport(LeadingIconTabs, size);
    let badges = rule.on_all_nodes_with_text("999+").fetch_semantics_nodes();
    assert_eq!(badges.len(), 3);
    assert!(badges.iter().all(|b| b.bounds_in_root().top >= 0.0), "a badge above its tab");
}

fn fancy_tab_samples() {
    let size = IntSize::new(400, 800);
    check_sample("FancyTabs", FancyTabs, size, "Tab 2", "Fancy tab 2 selected");
    check_sample("FancyIndicatorTabs", FancyIndicatorTabs, size, "Tab 2", "Fancy indicator tab 2 selected");
    check_sample(
        "FancyIndicatorContainerTabs",
        FancyIndicatorContainerTabs,
        size,
        "Tab 2",
        "Fancy transition tab 2 selected",
    );
    check_sample(
        "ScrollingFancyIndicatorContainerTabs",
        ScrollingFancyIndicatorContainerTabs,
        size,
        "Tab 2",
        "Scrolling fancy transition tab 2 selected",
    );
    // FancyIndicator: a 2 dp primary frame 5 dp in from the tab (a third of the row).
    let rule = KompozTestRule::with_viewport(FancyIndicatorTabs, size);
    let cs = light_color_scheme();
    let canvas = rule.draw();
    let frame = canvas.strokes.iter().zip(&canvas.stroke_colors).find(|(_, c)| **c == cs.primary).map(|(r, _)| *r);
    let frame = frame.expect("the fancy indicator's frame");
    assert!(near(frame.left, 5.0, 0.6) && near(frame.width(), 400.0 / 3.0 - 10.0, 1.0), "{frame:?}");
    // The animated container: its frame around the selected tab moves there, its color to the next of three.
    let rule = KompozTestRule::with_viewport(FancyIndicatorContainerTabs, size);
    rule.on_node_with_text("Tab 3").perform_click();
    rule.wait_for_idle();
    let canvas = rule.draw();
    let frame = canvas.strokes.iter().zip(&canvas.stroke_colors).find(|(_, c)| **c == cs.tertiary).map(|(r, _)| *r);
    let frame = frame.expect("the animated indicator's frame, in the tertiary color");
    assert!(frame.left > 400.0 * 2.0 / 3.0 && frame.right < 400.0, "{frame:?}");
}

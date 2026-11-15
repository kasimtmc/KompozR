//! foundation.layout's window insets: the kinds the platform reports, their arithmetic, padding by them,
//! consuming them down the tree, sizes from them, right to left.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::platform::{Insets, LocalLayoutDirection, SystemInsets};
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

// ---- Helpers ---------------------------------------------------------------------------------------------------

/// A node's outer bounds in the root (its outer coordinates): x, y, width, height.
fn bounds(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    let outer = rule.on_node_with_tag(tag).layout_node().coordinates();
    let b = outer.find_root_coordinates().local_bounding_box_of(&outer, false);
    [b.left, b.top, b.width(), b.height()]
}

/// Where a node's content is in the root (its inner coordinates: C++'s `contentBounds`): x, y, width, height.
fn content_bounds(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    let inner = rule.on_node_with_tag(tag).layout_node().inner_coordinates();
    let b = inner.find_root_coordinates().local_bounding_box_of(&inner, false);
    [b.left, b.top, b.width(), b.height()]
}

/// A node's measured size (for one of no area, whose bounds are zero).
fn size(rule: &KompozTestRule, tag: &str) -> IntSize {
    rule.on_node_with_tag(tag).layout_node().size()
}

const D: Density = Density::new(1.0, 1.0);

/// Insets' sides at density 1, left to right: left, top, right, bottom.
fn sides(insets: &WindowInsets) -> [i32; 4] {
    sides_in(insets, LayoutDirection::Ltr)
}

fn sides_in(insets: &WindowInsets, direction: LayoutDirection) -> [i32; 4] {
    [insets.get_left(&D, direction), insets.get_top(&D), insets.get_right(&D, direction), insets.get_bottom(&D)]
}

fn insets(left: i32, top: i32, right: i32, bottom: i32) -> Insets {
    Insets::new(left, top, right, bottom)
}

type Log = Rc<RefCell<Vec<[i32; 4]>>>;

/// Equal arguments, always: a scope never run again by its parent.
fn always(_: &(), _: &()) -> bool {
    true
}

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn last(log: &Log) -> Option<[i32; 4]> {
    log.borrow().last().copied()
}

// ---- Ported from C++ -------------------------------------------------------------------------------------------

fn window_insets_paddings_consume_what_the_outer_ones_took() {
    // C++'s window_insets_paddings_consume_what_the_outer_ones_took.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Row(Modifier.fill_max_size(), || {
                // The bars outside, the keyboard inside: only the part of it below the navigation bar.
                Box(
                    Modifier.weight(1.0).fill_max_height().system_bars_padding().test_tag("bars"),
                    content = || {
                        Box(Modifier.fill_max_size().ime_padding().test_tag("ime"));
                    },
                );
                // Padding the app applied itself, declared as consumed.
                Box(
                    Modifier
                        .weight(1.0)
                        .fill_max_height()
                        .padding_sides(bottom = 100.dp())
                        .consume_window_insets_padding_values(PaddingValues::new(0.dp(), 0.dp(), 0.dp(), 100.dp())),
                    content = || Box(Modifier.fill_max_size().ime_padding().test_tag("consumed")),
                );
            });
        }),
        IntSize::new(400, 800),
    );
    let bottom = |tag: &str| {
        let c = content_bounds(&rule, tag);
        c[1] + c[3]
    };
    assert_eq!(bottom("ime"), 800.0); // no insets yet
    rule.host().set_system_insets(SystemInsets {
        status_bars: insets(0, 24, 0, 0),
        navigation_bars: insets(0, 0, 0, 48),
        ime: insets(0, 0, 0, 300),
        ..Default::default()
    });
    rule.wait_for_idle();
    assert_eq!(content_bounds(&rule, "bars")[1], 24.0);
    assert_eq!(bottom("ime"), 800.0 - 48.0 - 252.0);
    assert_eq!(bottom("consumed"), 800.0 - 100.0 - 200.0);
}

fn consumed_window_insets_are_told() {
    // C++'s consumed_window_insets_are_told.
    let (outer, inner, first) = (log(), log(), log());
    let padded = mutable_state_of(true);
    // Read during layout only: laid out again, not recomposed.
    let relaid = mutable_state_of(0);
    let (o, i, f, p, r) = (outer.clone(), inner.clone(), first.clone(), padded.clone(), relaid.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, f) = (o.clone(), f.clone());
        let (i, r) = (i.clone(), r.clone());
        Box(
            Modifier
                .on_consumed_window_insets_changed(move |w| f.borrow_mut().push(sides(w)))
                .then(if p.get() { Modifier.status_bars_padding() } else { Modifier })
                .on_consumed_window_insets_changed(move |w| o.borrow_mut().push(sides(w))),
            content = || {
                // Skipped when its parent composes again: the same element.
                restartable((), Some(always as fn(&(), &()) -> bool), move |_| {
                    let (i, r) = (i.clone(), r.clone());
                    Box(Modifier
                        .layout(move |scope, m, c| {
                            r.get();
                            let placeable = m.measure(c);
                            scope.layout(placeable.width(), placeable.height(), move |s| s.place(&placeable, 0, 0))
                        })
                        .consume_window_insets(WindowInsets::new(0, 0, 0, 40))
                        .on_consumed_window_insets_changed(move |w| i.borrow_mut().push(sides(w))));
                });
            },
        );
    }));
    rule.host().set_system_insets(SystemInsets { status_bars: insets(0, 24, 0, 0), ..Default::default() });
    rule.wait_for_idle();
    // Before the padding nothing is taken; after it, the status bar; inside, also what it consumed.
    assert_eq!(last(&first), Some([0, 0, 0, 0]));
    assert_eq!(last(&outer), Some([0, 24, 0, 0]));
    assert_eq!(last(&inner), Some([0, 24, 0, 40]));
    // Told again when it changes; not on a layout that leaves it as it was.
    let told = inner.borrow().len();
    rule.host().set_system_insets(SystemInsets { status_bars: insets(0, 30, 0, 0), ..Default::default() });
    rule.wait_for_idle();
    assert_eq!(inner.borrow().len(), told + 1);
    assert_eq!(last(&inner), Some([0, 30, 0, 40]));
    relaid.set(1);
    rule.wait_for_idle();
    assert_eq!(inner.borrow().len(), told + 1); // laid out again, the same: not told
    padded.set(false);
    rule.wait_for_idle();
    assert_eq!(last(&inner), Some([0, 0, 0, 40]));
}

fn window_insets_kinds_and_arithmetic() {
    // C++'s window_insets_kinds_and_arithmetic.
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        *s.borrow_mut() = vec![
            sides(&WindowInsets::system_bars()),
            sides(&WindowInsets::safe_drawing()),
            sides(&WindowInsets::safe_content()),
        ];
    }));
    rule.host().set_system_insets(SystemInsets {
        status_bars: insets(0, 24, 0, 0),
        navigation_bars: insets(0, 0, 0, 48),
        ime: insets(0, 0, 0, 300),
        display_cutout: insets(30, 0, 0, 0),
        system_gestures: insets(20, 0, 20, 0),
        ..Default::default()
    });
    rule.wait_for_idle();
    assert_eq!(*seen.borrow(), [[0, 24, 0, 48], [30, 24, 0, 300], [30, 24, 20, 300]]);
    // The caption bar is a system bar; safe gestures gather the gesture edges, the tappable elements and the
    // waterfall, and safe content takes them in.
    let more = Rc::new(RefCell::new(Vec::new()));
    let m = more.clone();
    let more_rule = KompozTestRule::new(kompozable_lambda!(move || {
        *m.borrow_mut() = [
            WindowInsets::system_bars(),
            WindowInsets::caption_bar(),
            WindowInsets::mandatory_system_gestures(),
            WindowInsets::tappable_element(),
            WindowInsets::waterfall(),
            WindowInsets::safe_gestures(),
            WindowInsets::safe_drawing(),
            WindowInsets::safe_content(),
        ]
        .iter()
        .map(sides)
        .collect::<Vec<_>>();
    }));
    more_rule.host().set_system_insets(SystemInsets {
        status_bars: insets(0, 24, 0, 0),
        navigation_bars: insets(0, 0, 0, 20),
        system_gestures: insets(16, 0, 16, 0),
        caption_bar: insets(0, 32, 0, 0),
        mandatory_system_gestures: insets(0, 0, 0, 30),
        tappable_element: insets(0, 0, 0, 40),
        waterfall: insets(6, 0, 7, 0),
        ..Default::default()
    });
    more_rule.wait_for_idle();
    assert_eq!(
        *more.borrow(),
        [
            [0, 32, 0, 20],
            [0, 32, 0, 0],
            [0, 0, 0, 30],
            [0, 0, 0, 40],
            [6, 0, 7, 0],
            [16, 0, 16, 40],
            [0, 32, 0, 20],
            [16, 32, 16, 40]
        ]
    );
    // Their paddings.
    let padded = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Column(|| {
                Box(
                    Modifier.caption_bar_padding().width(100.dp()),
                    content = || Box(Modifier.size(10.dp()).test_tag("başlık")),
                );
                Box(
                    Modifier.waterfall_padding().width(100.dp()),
                    content = || Box(Modifier.size(10.dp()).test_tag("şelale")),
                );
                Box(
                    Modifier.system_gestures_padding().width(100.dp()),
                    content = || {
                        Box(Modifier.size(10.dp()).test_tag("hareket"));
                    },
                );
                Box(
                    Modifier.mandatory_system_gestures_padding().width(100.dp()),
                    content = || {
                        Box(Modifier.size(10.dp()).test_tag("zorunlu"));
                    },
                );
                Box(
                    Modifier.safe_gestures_padding().width(100.dp()),
                    content = || {
                        Box(Modifier.size(10.dp()).test_tag("güvenli"));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    padded.host().set_system_insets(SystemInsets {
        system_gestures: insets(16, 0, 16, 0),
        caption_bar: insets(0, 32, 0, 0),
        mandatory_system_gestures: insets(0, 0, 0, 30),
        tappable_element: insets(0, 0, 0, 40),
        waterfall: insets(6, 0, 7, 0),
        ..Default::default()
    });
    padded.wait_for_idle();
    let at = |tag: &str| bounds(&padded, tag);
    assert_eq!(at("başlık")[1], 32.0);
    assert_eq!(at("şelale")[0], 6.0);
    assert_eq!(at("hareket")[0], 16.0);
    assert_eq!(at("güvenli")[0], 16.0);
    assert_eq!(at("güvenli")[1] - at("zorunlu")[1], 40.0); // its 30 below
    let (a, b) = (WindowInsets::new(10, 20, 30, 40), WindowInsets::new(15, 5, 30, 50));
    assert_eq!(sides(&a.union(&b)), [15, 20, 30, 50]);
    assert_eq!(sides(&a.exclude(&b)), [0, 15, 0, 0]);
    assert_eq!(sides(&a.only(WindowInsetsSides::Horizontal + WindowInsetsSides::Bottom)), [10, 0, 30, 40]);
}

fn window_inset_sizes_make_spacers_under_the_bars() {
    // C++'s window_inset_sizes_make_spacers_under_the_bars.
    let is_rtl = mutable_state_of(false);
    let r = is_rtl.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], || {
                Column(|| {
                    Spacer(Modifier.window_insets_top_height(WindowInsets::status_bars()).test_tag("üst"));
                    Spacer(Modifier.window_insets_bottom_height(WindowInsets::navigation_bars()).test_tag("alt"));
                    Row(|| {
                        Spacer(
                            Modifier
                                .height(10.dp())
                                .window_insets_start_width(WindowInsets::display_cutout())
                                .test_tag("baş"),
                        );
                        Spacer(
                            Modifier
                                .height(10.dp())
                                .window_insets_end_width(WindowInsets::display_cutout())
                                .test_tag("son"),
                        );
                    });
                    // Taken above it: nothing left.
                    Column(Modifier.status_bars_padding(), || {
                        Spacer(Modifier.window_insets_top_height(WindowInsets::status_bars()).test_tag("alınmış"));
                    });
                });
            });
        }),
        IntSize::new(400, 800),
    );
    rule.host().set_system_insets(SystemInsets {
        status_bars: insets(0, 24, 0, 0),
        navigation_bars: insets(0, 0, 0, 48),
        display_cutout: insets(30, 0, 12, 0),
        ..Default::default()
    });
    rule.wait_for_idle();
    assert_eq!(size(&rule, "üst").height, 24);
    assert_eq!(size(&rule, "alt").height, 48);
    assert_eq!(size(&rule, "alınmış"), IntSize::Zero);
    assert_eq!(size(&rule, "baş").width, 30);
    assert_eq!(size(&rule, "son").width, 12);
    // Right to left: the start is the right.
    is_rtl.set(true);
    rule.wait_for_idle();
    assert_eq!(size(&rule, "baş").width, 12);
    assert_eq!(size(&rule, "son").width, 30);
}

fn rtl_keeps_unplaced_children_and_the_insets_where_they_are() {
    // C++'s rtl_keeps_unplaced_children_and_the_insets_where_they_are.
    let tick = mutable_state_of(0);
    let t = tick.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let t = t.clone();
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
                Column(Modifier.fill_max_size(), || {
                    // A recomposition: measured and placed again.
                    t.get();
                    Layout(
                        content = || {
                            Box(Modifier.size(20.dp()).test_tag("yerleşen"));
                            Box(Modifier.size(30.dp()).test_tag("bekleyen"));
                        },
                        modifier = Modifier.size_wh(100.dp(), 50.dp()).test_tag("düzen"),
                        measure_policy = measure_policy(|scope, ms, c| {
                            let loose = Constraints::new(0, c.max_width, 0, c.max_height);
                            let placed = ms[0].measure(loose);
                            // Measured, never placed.
                            let _waiting = ms[1].measure(loose);
                            scope.layout(100, 50, move |p| p.place_relative(&placed, 0, 0))
                        }),
                    );
                    // The window's left and right stay the screen's.
                    Box(
                        Modifier.fill_max_width().window_insets_padding(WindowInsets::system_bars()),
                        content = || {
                            Box(Modifier.fill_max_width().height(10.dp()).test_tag("içeride"));
                        },
                    );
                });
            });
        }),
        IntSize::new(300, 600),
    );
    rule.host().set_system_insets(SystemInsets { navigation_bars: insets(40, 0, 0, 0), ..Default::default() });
    rule.wait_for_idle();
    let waiting = bounds(&rule, "bekleyen")[0];
    let layout_x = bounds(&rule, "düzen")[0];
    assert_eq!(layout_x, 200.0);
    assert_eq!(bounds(&rule, "yerleşen")[0], layout_x + 80.0);
    // Laid out again and again: what wasn't placed doesn't flip.
    for _ in 0..3 {
        tick.set(tick.peek() + 1);
        rule.wait_for_idle();
        assert_eq!(bounds(&rule, "bekleyen")[0], waiting);
    }
    // The left inset, at the left.
    assert_eq!(bounds(&rule, "içeride")[0], 40.0);
}

fn keyboard_insets_follow_its_slide() {
    // C++'s keyboard_insets_follow_its_slide.
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            let (ime, from, to) =
                (WindowInsets::ime(), WindowInsets::ime_animation_source(), WindowInsets::ime_animation_target());
            Column(Modifier.fill_max_size().ime_padding(), || {
                Text(format!(
                    "{} {} {} {}",
                    ime.get_bottom(&D),
                    from.get_bottom(&D),
                    to.get_bottom(&D),
                    WindowInsets::is_ime_visible()
                ));
                Box(Modifier.fill_max_width().weight(1.0).test_tag("içerik"));
            });
        }),
        IntSize::new(400, 800),
    );
    let shows = |text: &str, height: f32| {
        rule.wait_for_idle();
        let b = bounds(&rule, "içerik");
        rule.on_node_with_text(text).exists() && b[1] + b[3] == height
    };
    assert!(shows("0 0 0 false", 800.0));
    // On its way up, a third of the way: the padding follows it; where it goes is known.
    rule.host().set_system_insets(SystemInsets {
        ime: insets(0, 0, 0, 100),
        ime_animating: true,
        ime_visible: true,
        ime_animation_target: insets(0, 0, 0, 300),
        ..Default::default()
    });
    assert!(shows("100 0 300 true", 700.0));
    // Up and still: where it started and where it went are where it is (the target a stale one).
    rule.host().set_system_insets(SystemInsets {
        ime: insets(0, 0, 0, 300),
        ime_visible: true,
        ime_animation_target: insets(0, 0, 0, 123),
        ..Default::default()
    });
    assert!(shows("300 300 300 true", 500.0));
    // On its way down: not up any more.
    rule.host().set_system_insets(SystemInsets {
        ime: insets(0, 0, 0, 200),
        ime_animating: true,
        ime_animation_source: insets(0, 0, 0, 300),
        ..Default::default()
    });
    assert!(shows("200 300 0 false", 600.0));
}

// ---- Window insets ---------------------------------------------------------------------------------------------

fn window_insets_as_compose() {
    use LayoutDirection::{Ltr, Rtl};
    // In px and in dp.
    let dp = WindowInsets::of_dp(1.5.dp(), 2.dp(), 3.dp(), 4.dp());
    let dense = Density::new(2.0, 1.0);
    assert_eq!(
        [dp.get_left(&dense, Ltr), dp.get_top(&dense), dp.get_right(&dense, Ltr), dp.get_bottom(&dense)],
        [3, 4, 6, 8]
    );
    assert_eq!(sides(&WindowInsets::Zero), [0, 0, 0, 0]);
    assert_eq!(sides(&WindowInsets::default()), [0, 0, 0, 0]);
    // Added, and `+`.
    let (a, b) = (WindowInsets::new(1, 2, 3, 4), WindowInsets::new(10, 20, 30, 40));
    assert_eq!(sides(&a.add(&b)), [11, 22, 33, 44]);
    assert_eq!(sides(&(a.clone() + b.clone())), [11, 22, 33, 44]);
    // Only some sides: start and end follow the direction, left and right don't.
    let start = a.only(WindowInsetsSides::Start + WindowInsetsSides::Top);
    assert_eq!(sides_in(&start, Ltr), [1, 2, 0, 0]);
    assert_eq!(sides_in(&start, Rtl), [0, 2, 3, 0]);
    let right = a.only(WindowInsetsSides::Right);
    assert_eq!(sides_in(&right, Rtl), [0, 0, 3, 0]);
    assert_eq!(sides_in(&a.only(WindowInsetsSides::End + WindowInsetsSides::Vertical), Rtl), [1, 2, 0, 4]);
    // As padding values, in a density: px over the density.
    let p = WindowInsets::new(10, 20, 30, 40).as_padding_values_with(dense);
    assert_eq!(p.calculate_left_padding(Ltr), 5.dp());
    assert_eq!(p.calculate_top_padding(), 10.dp());
    assert_eq!(p.calculate_end_padding(Rtl), 5.dp());
    assert_eq!(p, WindowInsets::new(10, 20, 30, 40).as_padding_values_with(dense));
    assert_ne!(p, WindowInsets::new(10, 20, 30, 40).as_padding_values_with(D));
    // Compared by how they were made, parts by parts.
    assert_eq!(a.union(&b), WindowInsets::new(1, 2, 3, 4).union(&WindowInsets::new(10, 20, 30, 40)));
    assert_ne!(a.union(&b), b.union(&a));
    assert_ne!(a.union(&b), a.add(&b));
    // Named in a body.
    let named = Rc::new(RefCell::new(None));
    let n = named.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        *n.borrow_mut() = Some(WindowInsets(top = 2.dp()));
    }));
    assert_eq!(*named.borrow(), Some(WindowInsets::of_dp(0.dp(), 2.dp(), 0.dp(), 0.dp())));
    assert_ne!(WindowInsets::new(0, 2, 0, 0), WindowInsets::of_dp(0.dp(), 2.dp(), 0.dp(), 0.dp()));
    // Printed as Compose's.
    assert_eq!(format!("{a:?}"), "Insets(left=1, top=2, right=3, bottom=4)");
    assert_eq!(format!("{:?}", a.union(&b)), format!("({a:?} ∪ {b:?})"));
    assert_eq!(format!("{:?}", WindowInsetsSides::Start + WindowInsetsSides::Top), "WindowInsetsSides(Start+Top)");
    assert_eq!(format!("{:?}", WindowInsetsSides::Horizontal), "WindowInsetsSides(Start+Left+End+Right)");
    // One's own.
    struct Half;
    impl CustomWindowInsets for Half {
        fn get_left(&self, density: &Density, _: LayoutDirection) -> i32 {
            (density.density * 5.0) as i32
        }
        fn get_top(&self, _: &Density) -> i32 {
            1
        }
        fn get_right(&self, _: &Density, direction: LayoutDirection) -> i32 {
            if direction == LayoutDirection::Rtl { 7 } else { 0 }
        }
        fn get_bottom(&self, _: &Density) -> i32 {
            2
        }
    }
    let custom = WindowInsets::custom(Half);
    assert_eq!(sides_in(&custom, Rtl), [5, 1, 7, 2]);
    assert_eq!(custom, custom.clone());
    assert_ne!(custom, WindowInsets::custom(Half));
}

fn window_insets_padding_follows_mutable_insets_and_padding_values() {
    let mutable = MutableWindowInsets::default();
    let m = mutable.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let m = m.clone();
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
            Column(Modifier.width(200.dp()), || {
                Box(
                    Modifier.window_insets_padding(m.as_window_insets()),
                    content = || {
                        Box(Modifier.size(10.dp()).test_tag("mutable"));
                    },
                );
                // Padding values as insets: their start is the right here.
                Box(
                    Modifier.window_insets_padding(WindowInsets::new(0, 0, 0, 0)),
                    content = || {
                        Box(
                            Modifier
                                .consume_window_insets_padding_values(PaddingValues::new(
                                    10.dp(),
                                    0.dp(),
                                    0.dp(),
                                    0.dp(),
                                ))
                                .window_insets_padding(WindowInsets::new(25, 0, 25, 0)),
                            content = || Box(Modifier.size(10.dp()).test_tag("after values")),
                        );
                    },
                );
                // Only the start, the right here.
                Box(
                    Modifier
                        .fill_max_width()
                        .window_insets_padding(WindowInsets::new(5, 0, 9, 0).only(WindowInsetsSides::Start)),
                    content = || {
                        Box(Modifier.fill_max_width().height(10.dp()).test_tag("start"));
                    },
                );
            });
        });
    }));
    rule.set_layout_check(false);
    assert_eq!(content_bounds(&rule, "mutable")[0], 190.0);
    // Set: measured again with them.
    mutable.set_insets(WindowInsets::new(0, 7, 12, 0));
    rule.wait_for_idle();
    let b = bounds(&rule, "mutable");
    assert_eq!((b[0], b[1]), (178.0, 7.0));
    // The 10 consumed at the start (the right): 25 - 10 left there, 25 on the left.
    let v = bounds(&rule, "after values");
    assert_eq!(v[0], 200.0 - 15.0 - 10.0);
    assert_eq!(bounds(&rule, "start"), [0.0, 7.0 + 10.0 + 10.0, 191.0, 10.0]);
}

fn consuming_window_insets_as_compose() {
    let (seen, inner_seen) = (log(), log());
    let shown = mutable_state_of(true);
    let (s, i, sh) = (seen.clone(), inner_seen.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, i) = (s.clone(), i.clone());
        Column(|| {
            Box(
                Modifier
                    .consume_window_insets(WindowInsets::new(5, 10, 0, 0))
                    .consume_window_insets(WindowInsets::new(0, 4, 0, 6))
                    .on_consumed_window_insets_changed(move |w| s.borrow_mut().push(sides(w))),
                content = || {
                    // What the padding pads by: past what is consumed above only.
                    Box(
                        Modifier.window_insets_padding(WindowInsets::new(8, 8, 8, 8)).test_tag("padded"),
                        content = || {
                            Box(Modifier.size(10.dp()));
                        },
                    );
                    let padding = if sh.get() {
                        Modifier.window_insets_padding(WindowInsets::new(0, 20, 0, 0))
                    } else {
                        Modifier
                    };
                    Box(
                        padding,
                        content = || {
                            // (kept as it is when its parent composes again)
                            restartable((), Some(always as fn(&(), &()) -> bool), move |_| {
                                let i = i.clone();
                                Box(Modifier.on_consumed_window_insets_changed(move |w| i.borrow_mut().push(sides(w))));
                            });
                        },
                    );
                },
            );
        });
    }));
    // Consuming is the union of what is consumed.
    assert_eq!(last(&seen), Some([5, 10, 0, 6]));
    let p = bounds(&rule, "padded");
    let c = content_bounds(&rule, "padded");
    assert_eq!((c[0] - p[0], c[1] - p[1], p[2], p[3]), (3.0, 0.0, 10.0 + 3.0 + 8.0, 10.0 + 2.0));
    assert_eq!(last(&inner_seen), Some([5, 20, 0, 6]));
    // A node that leaves hands what was consumed above it down.
    let told = inner_seen.borrow().len();
    shown.set(false);
    rule.wait_for_idle();
    assert_eq!(inner_seen.borrow().len(), told + 1);
    assert_eq!(last(&inner_seen), Some([5, 10, 0, 6]));
    assert_eq!(
        Modifier.consume_window_insets(WindowInsets::new(1, 2, 3, 4)),
        Modifier.consume_window_insets(WindowInsets::new(1, 2, 3, 4))
    );
}

fn recalculate_window_insets_takes_what_is_past_the_bounds() {
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Column(|| {
                Spacer(Modifier.height(50.dp()));
                // At 50, 200 by 200: everything past it on each side is consumed for what is inside.
                Box(
                    Modifier.size(200.dp()).recalculate_window_insets(),
                    content = || {
                        Box(Modifier.fill_max_size().system_bars_padding().test_tag("recalculated"));
                    },
                );
                // Not fixed: what is inside sees what is consumed above.
                Box(
                    Modifier.wrap_content_size().recalculate_window_insets(),
                    content = || {
                        Box(Modifier.system_bars_padding().size(20.dp()).test_tag("as above"));
                    },
                );
            });
        }),
        IntSize::new(400, 800),
    );
    rule.host().set_system_insets(SystemInsets {
        status_bars: insets(0, 24, 0, 0),
        navigation_bars: insets(10, 0, 0, 30),
        ..Default::default()
    });
    rule.wait_for_idle();
    // Its top 50 is past the status bar, its left 0 isn't past the 10 at the left, its right and bottom are.
    let (b, c) = (bounds(&rule, "recalculated"), content_bounds(&rule, "recalculated"));
    assert_eq!((c[0] - b[0], c[1] - b[1], c[2], c[3]), (10.0, 0.0, 190.0, 200.0));
    let (b, c) = (bounds(&rule, "as above"), content_bounds(&rule, "as above"));
    assert_eq!((c[0] - b[0], c[1] - b[1]), (10.0, 24.0));
}

tests!(
    window_insets_paddings_consume_what_the_outer_ones_took,
    consumed_window_insets_are_told,
    window_insets_kinds_and_arithmetic,
    window_inset_sizes_make_spacers_under_the_bars,
    rtl_keeps_unplaced_children_and_the_insets_where_they_are,
    keyboard_insets_follow_its_slide,
    window_insets_as_compose,
    window_insets_padding_follows_mutable_insets_and_padding_values,
    consuming_window_insets_as_compose,
    recalculate_window_insets_takes_what_is_past_the_bounds,
);

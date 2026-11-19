//! Lookahead, from Compose's semantics (its LookaheadScope, ApproachLayoutModifierNode, lookahead coordinates): a
//! scope's layouts measured and placed in lookahead first, then by their approach; approach measure and placement;
//! intrinsics in lookahead; the motion frame of reference; subcompose's approach; nested scopes; what a read state
//! measures again. Numbers in px (density 1).

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::layout::{IntrinsicMeasurable, LayoutModifierNode};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

type Scope = Rc<RefCell<Option<Rc<dyn LookaheadScope>>>>;

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn coordinates(rule: &KompozTestRule, tag: &str) -> LayoutCoordinates {
    rule.on_node_with_tag(tag).layout_node().coordinates()
}

fn near(a: Offset, b: Offset) -> bool {
    (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01
}

/// A layout modifier noting whether it measures in lookahead.
fn noting_passes(passes: Rc<RefCell<Vec<bool>>>) -> Modifier {
    Modifier.layout(move |s, m, c| {
        passes.borrow_mut().push(s.is_looking_ahead());
        let p = m.measure(c);
        s.layout(p.width(), p.height(), move |pl| pl.place(&p, 0, 0))
    })
}

fn an_approach_layout_measures_its_way_to_the_lookahead_size() {
    let wide = mutable_state_of(false);
    let approaching = mutable_state_of(true);
    let lookaheads = Rc::new(RefCell::new(Vec::<IntSize>::new()));
    let passes = Rc::new(RefCell::new(Vec::new()));
    let (w, a, l, p) = (wide.clone(), approaching.clone(), lookaheads.clone(), passes.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (w, a, l, p) = (w.clone(), a.clone(), l.clone(), p.clone());
        Column(|| {
            let (w, a, l, p) = (w.clone(), a.clone(), l.clone(), p.clone());
            LookaheadScope(move |_| {
                let (w, a, l, p) = (w.clone(), a.clone(), l.clone(), p.clone());
                Box(
                    modifier = Modifier
                        .approach_layout(
                            is_measurement_approach_in_progress = move |size| {
                                l.borrow_mut().push(size);
                                a.get()
                            },
                            approach_measure = |s: &ApproachMeasureScope, m: &dyn Measurable, _| {
                                let target = s.lookahead_size();
                                let placeable = m.measure(Constraints::fixed(target.width / 2, target.height));
                                s.layout(placeable.width(), placeable.height(), move |pl| pl.place(&placeable, 0, 0))
                            },
                        )
                        .test_tag("yaklaşan"),
                    content = move || {
                        let width = if w.get() { 200 } else { 100 };
                        Box(modifier = noting_passes(p.clone()).size_wh(width.dp(), 50.dp()).test_tag("içerik"));
                    },
                );
            });
        });
    }));
    // In lookahead 100 wide; on its way, half that: what it wraps measured so.
    assert_eq!(lookaheads.borrow().last(), Some(&IntSize::new(100, 50)));
    assert_eq!(bounds(&rule, "yaklaşan").width(), 50.0);
    assert_eq!(bounds(&rule, "içerik").width(), 50.0);
    // What it wraps measured in both passes, in lookahead first.
    assert!(passes.borrow().contains(&true) && passes.borrow().contains(&false));
    assert!(passes.borrow().first() == Some(&true));
    // The approach done: what it wraps measured with its lookahead constraints, as in lookahead.
    approaching.set(false);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "yaklaşan").width(), 100.0);
    assert_eq!(bounds(&rule, "içerik").width(), 100.0);
    // A new destination, already there.
    wide.set(true);
    rule.wait_for_idle();
    assert_eq!(lookaheads.borrow().last(), Some(&IntSize::new(200, 50)));
    assert_eq!(bounds(&rule, "yaklaşan").width(), 200.0);
}

fn the_lookahead_places_where_the_approach_goes() {
    // A layout placing its child 60 right in lookahead, 10 right in the main pass.
    let scope: Scope = Rc::new(RefCell::new(None));
    let placed = Rc::new(RefCell::new(Vec::<LayoutCoordinates>::new()));
    let positioned = Rc::new(RefCell::new(Vec::<LayoutCoordinates>::new()));
    let (s, pl, po) = (scope.clone(), placed.clone(), positioned.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, pl, po) = (s.clone(), pl.clone(), po.clone());
        Box(
            modifier = Modifier.padding(10.dp()),
            content = move || {
                let (s, pl, po) = (s.clone(), pl.clone(), po.clone());
                LookaheadScope(move |scope| {
                    *s.borrow_mut() = Some(scope.clone());
                    let (pl, po) = (pl.clone(), po.clone());
                    Layout(
                        content = move || {
                            let (pl, po) = (pl.clone(), po.clone());
                            Box(modifier = Modifier
                                .size(20.dp())
                                .on_placed(move |c| pl.borrow_mut().push(c.clone()))
                                .on_globally_positioned(move |c| po.borrow_mut().push(c.clone()))
                                .test_tag("çocuk"));
                        },
                        modifier = Modifier.test_tag("yerleştiren"),
                        measure_policy = measure_policy(|s, ms, _| {
                            let x = if s.is_looking_ahead() { 60 } else { 10 };
                            let p = ms[0].measure(Constraints::default());
                            s.layout(100, 100, move |pl| pl.place(&p, x, 5))
                        }),
                    );
                });
            },
        );
    }));
    let scope = scope.borrow().clone().unwrap();
    let parent = coordinates(&rule, "yerleştiren");
    let child = coordinates(&rule, "çocuk");
    // The main pass: where the main policy put it.
    assert_eq!(bounds(&rule, "çocuk").left, 20.0);
    // Its lookahead coordinates: where the lookahead put it.
    let lookahead_child = scope.to_lookahead_coordinates(&child);
    assert!(lookahead_child.is_lookahead() && !child.is_lookahead());
    assert_eq!(lookahead_child.size(), IntSize::new(20, 20));
    assert!(scope.to_lookahead_coordinates(&lookahead_child) == lookahead_child);
    let at = scope.local_lookahead_position_of(&parent, &child, Offset::Zero, true);
    assert!(near(at, Offset::new(60.0, 5.0)), "{at:?}");
    assert!(near(lookahead_child.position_in_root(), Offset::new(70.0, 15.0)));
    let lookahead_parent = lookahead_child.parent_layout_coordinates().unwrap();
    assert!(lookahead_parent.is_lookahead());
    assert!(near(lookahead_parent.local_position_of(&lookahead_child, Offset::Zero), Offset::new(60.0, 5.0)));
    // Mixed: the root's top left in its lookahead coordinates; the window and back.
    let root = child.find_root_coordinates();
    assert!(near(lookahead_child.local_position_of(&root, Offset::Zero), Offset::new(-70.0, -15.0)));
    assert!(near(root.local_position_of(&lookahead_child, Offset::Zero), Offset::new(70.0, 15.0)));
    let p = Offset::new(3.0, 4.0);
    assert!(near(lookahead_child.window_to_local(lookahead_child.local_to_window(p)), p));
    // On placed and globally positioned: told by the main pass alone, its coordinates.
    assert!(!placed.borrow().is_empty() && placed.borrow().iter().all(|c| !c.is_lookahead()));
    assert!(positioned.borrow().iter().all(|c| !c.is_lookahead()));
    assert_eq!(positioned.borrow().last().unwrap().position_in_root(), Offset::new(20.0, 15.0));
}

fn a_placement_approach_places_at_the_lookahead_position_once_done() {
    let moving = mutable_state_of(true);
    let lookahead_coordinates = Rc::new(RefCell::new(Vec::<bool>::new()));
    let (m, lc) = (moving.clone(), lookahead_coordinates.clone());
    let in_progress: Rc<dyn Fn(&PlacementScope, &LayoutCoordinates) -> bool> = Rc::new(move |_, c| {
        lc.borrow_mut().push(c.is_lookahead());
        m.get()
    });
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let in_progress = in_progress.clone();
        LookaheadScope(move |_| {
            let in_progress = in_progress.clone();
            Box(
                modifier = Modifier.size(100.dp()).approach_layout(
                    is_measurement_approach_in_progress = |_| false,
                    is_placement_approach_in_progress = Some(in_progress),
                    approach_measure = |s: &ApproachMeasureScope, m: &dyn Measurable, c| {
                        let p = m.measure(c);
                        // (on its way: 30 to the right of where it is going)
                        s.layout(p.width(), p.height(), move |pl| pl.place(&p, 30, 0))
                    },
                ),
                content = || Box(modifier = Modifier.size(20.dp()).test_tag("taşınan")),
            );
        });
    }));
    // On its way: where the approach places it.
    assert_eq!(bounds(&rule, "taşınan").left, 30.0);
    assert!(!lookahead_coordinates.borrow().is_empty() && lookahead_coordinates.borrow().iter().all(|l| *l));
    // There: placed where it is in lookahead, whatever the approach's block says.
    moving.set(false);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "taşınan").left, 0.0);
    moving.set(true);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "taşınan").left, 30.0);
}

/// An approach node halving: its approach measures what it wraps at half its lookahead width, its approach
/// intrinsics half what it wraps says; in lookahead it is a plain layout. It notes which intrinsics were asked.
struct Halving {
    asked: Rc<RefCell<Vec<&'static str>>>,
}

impl ModifierNode for Halving {
    node_capabilities!(layout, approach_layout);
}

impl LayoutModifierNode for Halving {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        layout::approach_lookahead_measure(scope, measurable, constraints)
    }
    fn max_intrinsic_width(&mut self, scope: &IntrinsicMeasureScope, m: &dyn IntrinsicMeasurable, height: i32) -> i32 {
        self.asked.borrow_mut().push(if scope.is_looking_ahead() { "lookahead" } else { "main" });
        m.max_intrinsic_width(height)
    }
}

impl ApproachLayoutModifierNode for Halving {
    fn is_measurement_approach_in_progress(&mut self, _: IntSize) -> bool {
        true
    }
    fn approach_measure(&mut self, s: &ApproachMeasureScope, m: &dyn Measurable, _: Constraints) -> MeasureResult {
        let target = s.lookahead_size();
        let p = m.measure(Constraints::fixed(target.width / 2, target.height));
        s.layout(p.width(), p.height(), move |pl| pl.place(&p, 0, 0))
    }
    fn max_approach_intrinsic_width(
        &mut self,
        _: &ApproachIntrinsicMeasureScope,
        m: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        self.asked.borrow_mut().push("approach");
        m.max_intrinsic_width(height) / 2
    }
}

#[derive(Clone)]
struct HalvingElement(Rc<RefCell<Vec<&'static str>>>);

impl PartialEq for HalvingElement {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl ModifierNodeElement for HalvingElement {
    type Node = Halving;
    fn create(&self) -> Halving {
        Halving { asked: self.0.clone() }
    }
    fn update(&self, node: &mut Halving) {
        node.asked = self.0.clone();
    }
}

fn intrinsics_are_asked_in_lookahead_and_in_the_approach() {
    let scope: Scope = Rc::new(RefCell::new(None));
    let asked = Rc::new(RefCell::new(Vec::new()));
    let (s, a) = (scope.clone(), asked.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, a) = (s.clone(), a.clone());
        LookaheadScope(move |scope| {
            *s.borrow_mut() = Some(scope.clone());
            let a = a.clone();
            Box(
                modifier = Modifier.width_intrinsic(IntrinsicSize::Max).test_tag("sarmal"),
                content = move || {
                    let a = a.clone();
                    Box(
                        modifier = Modifier::element(HalvingElement(a)).test_tag("yarı"),
                        content = || {
                            Box(modifier = Modifier.size_wh(100.dp(), 20.dp()));
                        },
                    );
                },
            );
        });
    }));
    // In lookahead its plain intrinsics (in a lookahead scope), in the main pass its approach ones.
    assert!(asked.borrow().contains(&"lookahead") && asked.borrow().contains(&"approach"));
    assert!(!asked.borrow().contains(&"main"));
    let scope = scope.borrow().clone().unwrap();
    let wrapper = coordinates(&rule, "sarmal");
    assert_eq!(scope.to_lookahead_coordinates(&wrapper).size().width, 100);
    assert_eq!(wrapper.size().width, 50);
    assert_eq!(bounds(&rule, "yarı").width(), 50.0);
}

fn a_motion_frame_of_reference_is_left_out_when_asked() {
    let offset = mutable_state_of(40);
    let scope: Scope = Rc::new(RefCell::new(None));
    let (o, s) = (offset.clone(), scope.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (o, s) = (o.clone(), s.clone());
        Column(|| {
            let (o, s) = (o.clone(), s.clone());
            // (a scroller, as Compose's: its content on its own frame of reference)
            let o2 = o.clone();
            Layout(
                content = || {
                    Box(modifier = Modifier.size(20.dp()).test_tag("kayan"));
                    Box(modifier = Modifier.size(20.dp()).test_tag("yanında"));
                },
                modifier = Modifier.size(100.dp()).test_tag("kaydıran"),
                measure_policy = measure_policy(move |s, ms, _| {
                    let first = ms[0].measure(Constraints::default());
                    let second = ms[1].measure(Constraints::default());
                    let o = o2.clone();
                    s.layout(100, 100, move |pl| {
                        let y = o.get();
                        pl.with_motion_frame_of_reference_placement(|pl| pl.place(&first, 0, y));
                        pl.place(&second, 50, y);
                    })
                }),
            );
            LookaheadScope(move |scope| {
                *s.borrow_mut() = Some(scope.clone());
                let o = o.clone();
                Layout(
                    content = move || {
                        let o = o.clone();
                        Layout(
                            content = || Box(modifier = Modifier.size(20.dp()).test_tag("öngörülen")),
                            modifier = Modifier.size(100.dp()).test_tag("öngören kaydıran"),
                            measure_policy = measure_policy(move |s, ms, _| {
                                let p = ms[0].measure(Constraints::default());
                                let o = o.clone();
                                s.layout(100, 100, move |pl| {
                                    let y = o.get();
                                    pl.with_motion_frame_of_reference_placement(|pl| pl.place(&p, 0, y));
                                })
                            }),
                        );
                    },
                    measure_policy = measure_policy(|s, ms, c| {
                        let p = ms[0].measure(c);
                        s.layout(p.width(), p.height(), move |pl| pl.place(&p, 0, 0))
                    }),
                );
            });
        });
    }));
    let parent = coordinates(&rule, "kaydıran");
    let moved = coordinates(&rule, "kayan");
    let beside = coordinates(&rule, "yanında");
    assert!(moved.introduces_motion_frame_of_reference() && !beside.introduces_motion_frame_of_reference());
    assert!(near(parent.local_position_of(&moved, Offset::Zero), Offset::new(0.0, 40.0)));
    assert!(near(parent.local_position_of_with(&moved, Offset::Zero, false), Offset::Zero));
    assert!(near(parent.local_position_of_with(&beside, Offset::Zero, false), Offset::new(50.0, 40.0)));
    // In lookahead too.
    let scope = scope.borrow().clone().unwrap();
    let lookahead_parent = coordinates(&rule, "öngören kaydıran");
    let lookahead_moved = coordinates(&rule, "öngörülen");
    assert!(scope.to_lookahead_coordinates(&lookahead_moved).introduces_motion_frame_of_reference());
    let with = scope.local_lookahead_position_of(&lookahead_parent, &lookahead_moved, Offset::Zero, true);
    let without = scope.local_lookahead_position_of(&lookahead_parent, &lookahead_moved, Offset::Zero, false);
    assert!(near(with, Offset::new(0.0, 40.0)) && near(without, Offset::Zero), "{with:?} {without:?}");
    // It moves: placed again where it is.
    offset.set(10);
    rule.wait_for_idle();
    assert!(near(parent.local_position_of(&moved, Offset::Zero), Offset::new(0.0, 10.0)));
    assert!(near(parent.local_position_of_with(&moved, Offset::Zero, false), Offset::Zero));
}

fn a_subcomposing_layout_composes_in_lookahead_and_in_its_approach() {
    let scope: Scope = Rc::new(RefCell::new(None));
    let passes = Rc::new(RefCell::new(Vec::new()));
    let extra = mutable_state_of(false);
    let (s, p, e) = (scope.clone(), passes.clone(), extra.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, p, e) = (s.clone(), p.clone(), e.clone());
        LookaheadScope(move |scope| {
            *s.borrow_mut() = Some(scope.clone());
            let (p, e) = (p.clone(), e.clone());
            SubcomposeLayout(
                modifier = Modifier.test_tag("alt"),
                measure_policy = move |s, c: Constraints| {
                    p.borrow_mut().push(s.is_looking_ahead());
                    let loose = c.copy_max_dimensions();
                    let mut placeables: Vec<Placeable> = s
                        .subcompose("ana", || Box(modifier = Modifier.size_wh(30.dp(), 10.dp()).test_tag("ana")))
                        .iter()
                        .map(|m| m.measure(loose))
                        .collect();
                    // (only the approach asks for it: composed for the approach, laid out in lookahead on its own)
                    if !s.is_looking_ahead() && e.get() {
                        placeables.extend(
                            s.subcompose("yaklaşımda", || {
                                Box(modifier = Modifier.size_wh(15.dp(), 10.dp()).test_tag("yaklaşımda"))
                            })
                            .iter()
                            .map(|m| m.measure(loose)),
                        );
                    }
                    s.layout(100, 20, move |pl| {
                        let mut x = 0;
                        for placeable in &placeables {
                            pl.place(placeable, x, 0);
                            x += placeable.width();
                        }
                    })
                },
            );
        });
    }));
    assert!(passes.borrow().contains(&true) && passes.borrow().contains(&false));
    rule.on_node_with_tag("ana").assert_exists();
    rule.on_node_with_tag("yaklaşımda").assert_does_not_exist();
    extra.set(true);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "yaklaşımda").left, 30.0);
    let scope = scope.borrow().clone().unwrap();
    let approach_only = scope.to_lookahead_coordinates(&coordinates(&rule, "yaklaşımda"));
    assert!(approach_only.is_lookahead() && approach_only.size() == IntSize::new(15, 10));
    extra.set(false);
    rule.wait_for_idle();
    rule.on_node_with_tag("yaklaşımda").assert_does_not_exist();
    rule.on_node_with_tag("ana").assert_exists();
}

fn nested_lookahead_scopes_share_one_lookahead() {
    let scope: Scope = Rc::new(RefCell::new(None));
    let inner_scope: Scope = Rc::new(RefCell::new(None));
    let passes = Rc::new(RefCell::new(Vec::new()));
    let (s, i, p) = (scope.clone(), inner_scope.clone(), passes.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, i, p) = (s.clone(), i.clone(), p.clone());
        LookaheadScope(move |scope| {
            *s.borrow_mut() = Some(scope.clone());
            let (i, p) = (i.clone(), p.clone());
            Layout(
                content = move || {
                    let (i, p) = (i.clone(), p.clone());
                    LookaheadScope(move |inner| {
                        *i.borrow_mut() = Some(inner.clone());
                        Box(modifier = noting_passes(p.clone()).size(20.dp()).test_tag("iç"));
                    });
                },
                modifier = Modifier.padding(5.dp()).test_tag("dış"),
                measure_policy = measure_policy(|s, ms, _| {
                    let x = if s.is_looking_ahead() { 60 } else { 10 };
                    let p = ms[0].measure(Constraints::default());
                    s.layout(100, 100, move |pl| pl.place(&p, x, 0))
                }),
            );
        });
    }));
    // The inner scope's layouts are in the outer one's lookahead: where the outer lookahead put them.
    let scope = scope.borrow().clone().unwrap();
    let inner = coordinates(&rule, "iç");
    assert!(near(scope.to_lookahead_coordinates(&inner).position_in_root(), Offset::new(65.0, 5.0)));
    assert_eq!(bounds(&rule, "iç").left, 15.0);
    // One lookahead measure a measure.
    let lookahead = passes.borrow().iter().filter(|p| **p).count();
    let main = passes.borrow().iter().filter(|p| !**p).count();
    assert_eq!(lookahead, main);
    // Each scope's coordinates: its parent's.
    let inner_scope = inner_scope.borrow().clone().unwrap();
    let outer_coordinates = scope.lookahead_scope_coordinates_of(&inner);
    let inner_coordinates = inner_scope.lookahead_scope_coordinates_of(&inner);
    assert!(near(outer_coordinates.position_in_root(), Offset::Zero));
    assert!(near(inner_coordinates.position_in_root(), Offset::new(5.0, 5.0)));
}

fn a_read_state_measures_again_only_the_pass_that_read_it() {
    let in_lookahead = mutable_state_of(10);
    let in_main = mutable_state_of(10);
    let counts = Rc::new(Cell::new((0, 0)));
    let (l, m, c) = (in_lookahead.clone(), in_main.clone(), counts.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, m, c) = (l.clone(), m.clone(), c.clone());
        LookaheadScope(move |_| {
            let (l, m, c) = (l.clone(), m.clone(), c.clone());
            Layout(
                content = || Box(modifier = Modifier.size(10.dp())),
                modifier = Modifier.test_tag("okuyan"),
                measure_policy = measure_policy(move |s, ms, _| {
                    let (looking, main) = c.get();
                    let width = if s.is_looking_ahead() {
                        c.set((looking + 1, main));
                        l.get()
                    } else {
                        c.set((looking, main + 1));
                        m.get()
                    };
                    let p = ms[0].measure(Constraints::default());
                    s.layout(width, 10, move |pl| pl.place(&p, 0, 0))
                }),
            );
        });
    }));
    // (the layout check measures everything again)
    rule.set_layout_check(false);
    counts.set((0, 0));
    // What only the main pass read: the main pass alone.
    in_main.set(30);
    rule.wait_for_idle();
    assert_eq!(counts.get(), (0, 1));
    assert_eq!(bounds(&rule, "okuyan").width(), 30.0);
    // What the lookahead read: the lookahead, then the main pass after it.
    counts.set((0, 0));
    in_lookahead.set(50);
    rule.wait_for_idle();
    assert_eq!(counts.get(), (1, 1));
}

fn outside_a_lookahead_scope_nothing_looks_ahead() {
    let passes = Rc::new(RefCell::new(Vec::new()));
    let p = passes.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Box(modifier = noting_passes(p.clone()).size(20.dp()).test_tag("düz"));
    }));
    assert!(!passes.borrow().is_empty() && passes.borrow().iter().all(|p| !p));
    let node = rule.on_node_with_tag("düz").layout_node();
    assert!(!node.is_in_lookahead_scope());
}

fn an_approach_scope_knows_its_lookahead_scope_coordinates() {
    let seen = Rc::new(RefCell::new(None::<(Offset, Constraints)>));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Box(
            modifier = Modifier.padding(15.dp()),
            content = move || {
                let s = s.clone();
                LookaheadScope(move |_| {
                    let s = s.clone();
                    Box(modifier = Modifier
                        .approach_layout(
                            is_measurement_approach_in_progress = |_| true,
                            approach_measure = move |scope: &ApproachMeasureScope, m: &dyn Measurable, c| {
                                let p = m.measure(c);
                                let (scope2, s) = (scope.clone(), s.clone());
                                let lookahead_constraints = scope.lookahead_constraints();
                                scope.layout(p.width(), p.height(), move |pl| {
                                    let at = scope2.lookahead_scope_coordinates(pl).position_in_root();
                                    *s.borrow_mut() = Some((at, lookahead_constraints));
                                    pl.place(&p, 0, 0);
                                })
                            },
                        )
                        .size(10.dp()));
                });
            },
        );
    }));
    let (at, constraints) = seen.borrow().unwrap();
    assert!(near(at, Offset::new(15.0, 15.0)));
    assert_eq!(constraints, Constraints::new(0, 370, 0, 770));
}

tests!(
    an_approach_layout_measures_its_way_to_the_lookahead_size,
    the_lookahead_places_where_the_approach_goes,
    a_placement_approach_places_at_the_lookahead_position_once_done,
    intrinsics_are_asked_in_lookahead_and_in_the_approach,
    a_motion_frame_of_reference_is_left_out_when_asked,
    a_subcomposing_layout_composes_in_lookahead_and_in_its_approach,
    nested_lookahead_scopes_share_one_lookahead,
    a_read_state_measures_again_only_the_pass_that_read_it,
    outside_a_lookahead_scope_nothing_looks_ahead,
    an_approach_scope_knows_its_lookahead_scope_coordinates,
);

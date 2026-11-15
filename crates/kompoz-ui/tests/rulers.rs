//! Rulers (Compose's Ruler, RulerScope, RectRulers): provided by a layout's measure result, read while placing in
//! the reader's coordinates, derived, provided one at a time; a reader placed again, nothing measured, as a value it
//! read changes. The window insets rulers the host's root provides. Numbers in px.

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::layout::{
    HorizontalRuler, RectRulers, Ruler, RulerScope, VerticalRuler, WindowInsetsAnimation, WindowInsetsRulers,
};
use kompoz_ui::platform::{Insets, InsetsAnimation, InsetsKind, LocalLayoutDirection, SystemInsets};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A layout `size` square providing what `rulers` gives, its content placed at its top left.
fn providing(size: i32, rulers: impl Fn(&RulerScope) + 'static) -> Modifier {
    let rulers: Rc<dyn Fn(&RulerScope)> = Rc::new(rulers);
    Modifier.layout(move |scope, m, _| {
        let p = m.measure(Constraints::fixed(size, size));
        let rulers = rulers.clone();
        scope.layout_with_rulers(size, size, move |s| rulers(s), move |pl| pl.place(&p, 0, 0))
    })
}

/// A layout reading, as it places, what `read` gives (into `out`), counting its measurements in `measures`.
fn reading(
    read: impl Fn(&PlacementScope) -> Vec<f32> + 'static,
    out: Rc<RefCell<Vec<f32>>>,
    measures: Rc<Cell<u32>>,
) -> Modifier {
    let read = Rc::new(read);
    Modifier.layout(move |scope, m, c| {
        measures.set(measures.get() + 1);
        let p = m.measure(c);
        let (read, out) = (read.clone(), out.clone());
        scope.layout(p.width(), p.height(), move |pl| {
            *out.borrow_mut() = read(pl);
            pl.place(&p, 0, 0);
        })
    })
}

fn rulers_are_read_where_they_are_placed_and_place_their_readers_again() {
    let (left, top, unused) = (VerticalRuler::new(), HorizontalRuler::new(), VerticalRuler::new());
    let value = mutable_state_of(30.0f32);
    let (out, measures) = (Rc::new(RefCell::new(Vec::new())), Rc::new(Cell::new(0)));
    let (l, t, u, v, o, m) = (left.clone(), top.clone(), unused.clone(), value.clone(), out.clone(), measures.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, t, u, v, o, m) = (l.clone(), t.clone(), u.clone(), v.clone(), o.clone(), m.clone());
        let (l2, t2) = (l.clone(), t.clone());
        Box(
            providing(200, move |s| {
                s.provides(&l2, v.get());
                s.provides(&t2, 10.0);
            }),
            content = move || {
                let (l, t, u) = (l.clone(), t.clone(), u.clone());
                Box(Modifier
                    .offset_with(20.dp(), 5.dp())
                    .then(reading(
                        move |p| vec![p.current(&l, -1.0), p.current(&t, -1.0), p.current(&u, -1.0)],
                        o.clone(),
                        m.clone(),
                    ))
                    .size(50.dp()));
            },
        );
    }));
    // (the layout check measures everything once more)
    rule.set_layout_check(false);
    // In the reader's coordinates (it is 20 right and 5 down); one nothing provides: the default.
    assert_eq!(*out.borrow(), [10.0, 5.0, -1.0]);
    let measured = measures.get();
    // The value changes: the reader is placed again, nothing measured.
    value.set(50.0);
    rule.wait_for_idle();
    assert_eq!(*out.borrow(), [30.0, 5.0, -1.0]);
    assert_eq!(measures.get(), measured);
}

fn derived_rulers_and_rect_rulers_merge_what_is_provided() {
    let (a, b) = (VerticalRuler::new(), VerticalRuler::new());
    let (outer, inner) = (RectRulers::new(), RectRulers::new());
    let max = VerticalRuler::max_of(&[a.clone(), b.clone()]);
    let min = VerticalRuler::min_of(&[a.clone(), b.clone()]);
    let half = VerticalRuler::derived({
        let a = a.clone();
        move |scope, default| scope.current(&a, default * 2.0) / 2.0
    });
    let innermost = RectRulers::innermost_of(&[outer.clone(), inner.clone()]);
    let outermost = RectRulers::outermost_of(&[outer.clone(), inner.clone()]);
    let out = Rc::new(RefCell::new(Vec::new()));
    let o = out.clone();
    let (pa, pb, po, pi) = (a.clone(), b.clone(), outer.clone(), inner.clone());
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (pa, pb, po, pi) = (pa.clone(), pb.clone(), po.clone(), pi.clone());
        let (max, min, half, innermost, outermost, o) =
            (max.clone(), min.clone(), half.clone(), innermost.clone(), outermost.clone(), o.clone());
        Box(
            providing(100, move |s| {
                s.provides(&pa, 40.0);
                s.provides(&pb, 70.0);
                for (r, v) in [(&po, 10.0), (&pi, 20.0)] {
                    s.provides(&r.left, v);
                    s.provides(&r.top, v);
                    s.provides(&r.right, 100.0 - v);
                    s.provides(&r.bottom, 100.0 - v);
                }
            }),
            content = move || {
                let (max, min, half, innermost, outermost) =
                    (max.clone(), min.clone(), half.clone(), innermost.clone(), outermost.clone());
                Box(reading(
                    move |p| {
                        vec![
                            p.current(&max, -1.0),
                            p.current(&min, -1.0),
                            half.current(p, -1.0),
                            p.current(&innermost.left, -1.0),
                            p.current(&innermost.right, -1.0),
                            p.current(&outermost.top, -1.0),
                            p.current(&outermost.bottom, -1.0),
                        ]
                    },
                    o.clone(),
                    Rc::new(Cell::new(0)),
                )
                .fill_max_size());
            },
        );
    }));
    assert_eq!(*out.borrow(), [70.0, 40.0, 20.0, 20.0, 80.0, 10.0, 90.0]);
    // Rulers are known by identity: clones are the same ruler, a new one another.
    assert!(a == a.clone() && a != VerticalRuler::new() && Ruler::from(&a) == Ruler::Vertical(a.clone()));
    assert_eq!(format!("{:?}", RectRulers::named("pencere")), "RectRulers(pencere)");
}

fn a_nearer_provider_shadows_and_relative_rulers_mirror_right_to_left() {
    let ruler = VerticalRuler::new();
    let near = mutable_state_of(false);
    let rtl = mutable_state_of(false);
    let out = Rc::new(RefCell::new(Vec::new()));
    let (r, n, d, o) = (ruler.clone(), near.clone(), rtl.clone(), out.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let direction = if d.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        let (r, n, o) = (r.clone(), n.clone(), o.clone());
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], move || {
            let (r, n, o) = (r.clone(), n.clone(), o.clone());
            let r2 = r.clone();
            Box(
                providing(200, move |s| s.provides_relative(&r2, 30.0)),
                content = move || {
                    let (r, o) = (r.clone(), o.clone());
                    let r3 = r.clone();
                    let inner = if n.get() { providing(100, move |s| s.provides(&r3, 7.0)) } else { Modifier };
                    Box(
                        inner,
                        content = move || {
                            let r = r.clone();
                            Box(reading(move |p| vec![p.current(&r, -1.0)], o.clone(), Rc::new(Cell::new(0)))
                                .size(10.dp()));
                        },
                    );
                },
            );
        });
    }));
    assert_eq!(*out.borrow(), [30.0]);
    // A layout nearer the reader providing it: its value from now on (the reader placed again).
    near.set(true);
    rule.wait_for_idle();
    assert_eq!(*out.borrow(), [7.0]);
    near.set(false);
    rule.wait_for_idle();
    assert_eq!(*out.borrow(), [30.0]);
    // Right to left, a relative ruler is from the right: at 170, 20 left of the reader (placed at the right, 190).
    rtl.set(true);
    rule.wait_for_idle();
    assert_eq!(*out.borrow(), [-20.0]);
}

fn a_ruler_provider_provides_one_ruler_when_it_is_read() {
    let (asked, spare) = (VerticalRuler::new(), VerticalRuler::new());
    let calls = Rc::new(RefCell::new(Vec::<String>::new()));
    let source = mutable_state_of(5.0f32);
    let out = Rc::new(RefCell::new(Vec::new()));
    let (a, sp, c, so, o) = (asked.clone(), spare.clone(), calls.clone(), source.clone(), out.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (a, sp, c, so, o) = (a.clone(), sp.clone(), c.clone(), so.clone(), o.clone());
        let (a2, sp2) = (a.clone(), sp.clone());
        let provider = Modifier.layout(move |scope, m, _| {
            let p = m.measure(Constraints::fixed(100, 100));
            let (a2, sp2, c, so) = (a2.clone(), sp2.clone(), c.clone(), so.clone());
            let a3 = a2.clone();
            scope.layout_with_ruler_provider(
                100,
                100,
                move |r| *r == Ruler::from(&a3),
                move |s, r| {
                    c.borrow_mut().push("provider".into());
                    if *r == Ruler::from(&a2) {
                        s.provides(&a2, so.get());
                    }
                    s.provides(&sp2, 99.0); // (another one, while it is at it)
                },
                move |pl| pl.place(&p, 0, 0),
            )
        });
        Box(
            provider,
            content = move || {
                let (a, sp) = (a.clone(), sp.clone());
                Box(reading(
                    move |p| vec![p.current(&a, -1.0), p.current(&a, -1.0), p.current(&sp, -1.0)],
                    o.clone(),
                    Rc::new(Cell::new(0)),
                )
                .size(10.dp()));
            },
        );
    }));
    rule.set_layout_check(false);
    calls.borrow_mut().clear();
    source.set(6.0);
    rule.wait_for_idle();
    // Run once for the ruler read (the second read finds the value), the other ruler it gave read too.
    assert_eq!(*out.borrow(), [6.0, 6.0, 99.0]);
    assert_eq!(calls.borrow().len(), 1);
    // What it read changes: the value dropped, the reader placed again, the provider run again.
    source.set(8.0);
    rule.wait_for_idle();
    assert_eq!(out.borrow()[0], 8.0);
    assert_eq!(calls.borrow().len(), 2);
}

// ---- The window insets rulers ----------------------------------------------------------------------------------

fn insets(left: i32, top: i32, right: i32, bottom: i32) -> Insets {
    Insets::new(left, top, right, bottom)
}

fn rects(p: &PlacementScope, r: &RectRulers) -> Vec<f32> {
    vec![p.current(&r.left, -1.0), p.current(&r.top, -1.0), p.current(&r.right, -1.0), p.current(&r.bottom, -1.0)]
}

fn the_root_provides_the_window_insets_rulers() {
    let out = Rc::new(RefCell::new(Vec::new()));
    let animation = Rc::new(RefCell::new(None::<WindowInsetsAnimation>));
    let cutouts = Rc::new(RefCell::new(Vec::<Vec<f32>>::new()));
    let measures = Rc::new(Cell::new(0));
    let (o, a, c, m) = (out.clone(), animation.clone(), cutouts.clone(), measures.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (a, c) = (a.clone(), c.clone());
        Box(Modifier
            .padding(10.dp())
            .then(reading(
                move |p| {
                    *a.borrow_mut() = Some(WindowInsetsRulers::ime().get_animation(p));
                    *c.borrow_mut() = p.get_display_cutout_bounds().iter().map(|r| rects(p, r)).collect();
                    let mut v = rects(p, &WindowInsetsRulers::status_bars().current());
                    v.extend(rects(p, &WindowInsetsRulers::status_bars().maximum()));
                    v.extend(rects(p, &WindowInsetsRulers::safe_drawing().current()));
                    v.push(p.current(&WindowInsetsRulers::ime().maximum().bottom, -1.0));
                    v.push(p.current(&WindowInsetsRulers::ime().get_animation(p).target.bottom, -1.0));
                    v
                },
                o.clone(),
                m.clone(),
            ))
            .fill_max_size());
    }));
    rule.set_layout_check(false);
    // Nothing reported yet: the window's edges, in the reader's coordinates (10 in from them).
    assert_eq!(out.borrow()[..4], [-10.0, -10.0, 390.0, 790.0]);
    let measured = measures.get();
    rule.host().set_system_insets(SystemInsets {
        status_bars: insets(0, 24, 0, 0),
        status_bars_ignoring_visibility: Some(insets(0, 30, 0, 0)),
        navigation_bars: insets(0, 0, 0, 48),
        ime: insets(0, 0, 0, 150),
        ime_visible: true,
        display_cutout: insets(0, 40, 0, 0),
        cutout_rects: vec![IntRect::new(180, 0, 220, 40)],
        animations: vec![InsetsAnimation {
            kind: InsetsKind::Ime,
            source: Insets::Zero,
            target: insets(0, 0, 0, 300),
            fraction: 0.5,
            alpha: 1.0,
            duration_millis: 250,
        }],
        ..Default::default()
    });
    rule.wait_for_idle();
    let v = out.borrow().clone();
    // The status bars now, and what they would cover shown; the innermost of the bars, the keyboard and the cutout; the keyboard has no
    // maximum; where it is sliding to.
    assert_eq!(v[..4], [-10.0, 14.0, 390.0, 790.0]);
    assert_eq!(v[4..8], [-10.0, 20.0, 390.0, 790.0]);
    assert_eq!(v[8..12], [-10.0, 30.0, 390.0, 640.0]);
    assert_eq!(v[12], -1.0);
    assert_eq!(v[13], 800.0 - 300.0 - 10.0);
    let a = animation.borrow().clone().unwrap();
    assert!(a.is_animating && a.is_visible && a.fraction == 0.5 && a.duration_millis == 250);
    assert_eq!(*cutouts.borrow(), [vec![170.0, -10.0, 210.0, 30.0]]);
    // Only placed again: nothing measured for the insets.
    assert_eq!(measures.get(), measured);
    // The innermost rulers' animation: the system bars, as their kinds say.
    let bars = WindowInsetsRulers::system_bars();
    assert!(bars == WindowInsetsRulers::system_bars() && bars != WindowInsetsRulers::status_bars());
    assert_eq!(format!("{bars:?}"), "system bars");
}

tests!(
    rulers_are_read_where_they_are_placed_and_place_their_readers_again,
    derived_rulers_and_rect_rulers_merge_what_is_provided,
    a_nearer_provider_shadows_and_relative_rulers_mirror_right_to_left,
    a_ruler_provider_provides_one_ruler_when_it_is_read,
    the_root_provides_the_window_insets_rulers,
);

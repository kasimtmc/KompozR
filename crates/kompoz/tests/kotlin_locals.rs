//! Composition locals computed where they are read (Compose 1.7's), in Kotlin's syntax as a Compose user writes them:
//! `compositionLocalWithComputedDefaultOf { LocalX.currentValue … }`, `LocalX providesComputed { … }` reading the other
//! locals as the reader sees them (a provider below the computed one's counts), several values given to one
//! `CompositionLocalProvider`; and in a `.kt` file (kompoz_file!) the C++ Kompoz's
//! a_local_provided_from_an_animated_state_is_followed_by_its_readers_alone in Compose's form (`LocalTint
//! providesComputed { color.value }`) and a computed default reading `LocalDensity.currentValue` through `with`. The
//! same behaviour crates/kompoz-runtime/tests/runtime.rs and crates/kompoz-animation/tests/as_state.rs check in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

kompoz! {
    val LocalBase = compositionLocalOf { 1 }
    val LocalTwice = compositionLocalOf { 0 }
    val LocalThrice = compositionLocalWithComputedDefaultOf { LocalBase.currentValue * 3 }

    @Composable
    fun Computed(seen: (String) -> Unit) {
        seen("${LocalTwice.current}/${LocalThrice.current}")
        CompositionLocalProvider(LocalTwice providesComputed { LocalBase.currentValue * 2 }) {
            seen("${LocalTwice.current}/${LocalThrice.current}")
            CompositionLocalProvider(LocalBase provides 10) {
                seen("${LocalTwice.current}/${LocalThrice.current}")
                CompositionLocalProvider(LocalThrice provides 7, LocalTwice provides 5) {
                    seen("${LocalTwice.current}/${LocalThrice.current}")
                }
            }
        }
    }
}

fn computed_locals_as_compose_writes_them() {
    let seen = Rc::new(RefCell::new(Vec::<String>::new()));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Computed(move |v| s.borrow_mut().push(v));
    }));
    // (nothing provided; computed above, reading the base's default; and below a base given; values given)
    assert_eq!(*seen.borrow(), ["0/3", "2/3", "20/30", "5/7"]);
}

// ---- kompoz_file! ------------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/locals.kt");

fn a_computed_local_follows_an_animated_state_in_its_readers_alone() {
    // The C++ Kompoz's a_local_provided_from_an_animated_state_is_followed_by_its_readers_alone, as Compose writes it.
    let (on, tick) = (mutable_state_of(false), mutable_state_of(0));
    let (composed, reads) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let seen = Rc::new(Cell::new(Color::Transparent));
    let (o, t, c, r, s) = (on.clone(), tick.clone(), composed.clone(), reads.clone(), seen.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, r, s) = (c.clone(), r.clone(), s.clone());
        Tinted(
            o.get(),
            t.get(),
            move |_| c.set(c.get() + 1),
            move |color| {
                r.set(r.get() + 1);
                s.set(color);
            },
        );
    }));
    assert_eq!(seen.get(), Color::Black);
    on.set(true);
    rule.advance_frame(); // the provider composed again with the new target...
    let (before, reads_before) = (composed.get(), reads.get());
    rule.advance_time_by(80);
    assert!(composed.get() == before && reads.get() > reads_before); // ...and from then on only the reader
    assert!(seen.get() != Color::Black && seen.get() != Color::White);
    rule.wait_for_idle();
    assert!(seen.get() == Color::White && composed.get() == before && !rule.host().has_pending_work());
    // The provider composed again for something else, its computation capturing the same state (Compose's memoized
    // lambda): its reader isn't told.
    let reads_now = reads.get();
    tick.set(1);
    rule.wait_for_idle();
    assert!(composed.get() == before + 1 && reads.get() == reads_now, "{} {}", composed.get(), reads.get());
}

fn a_computed_default_reads_the_density_where_it_is_read() {
    let seen = Rc::new(RefCell::new(Vec::<f32>::new()));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Hairlines(move |v| s.borrow_mut().push(v));
    }));
    let density = seen.borrow()[0] / 10.0;
    assert_eq!(*seen.borrow(), [10.0 * density, 20.0]);
}

tests!(
    computed_locals_as_compose_writes_them,
    a_computed_local_follows_an_animated_state_in_its_readers_alone,
    a_computed_default_reads_the_density_where_it_is_read,
);

//! The focus and key tests: focus targets and their state, requesters, properties (redirects, cancels, enter and
//! exit), restorers, groups, events, one- and two-dimensional search, the keys along the focused path, and the C++
//! Kompoz's focus and key tests that need no text field (focus targets for those), with Compose's behaviour where
//! the two differ. Numbers in px.

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::focus::*;
use kompoz_ui::input::*;
use kompoz_ui::platform::{InputMode, LocalInputModeManager};
use kompoz_ui::*;
use std::cell::RefCell;
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

fn new_log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn taken(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.borrow_mut())
}

fn key(rule: &KompozTestRule, k: Key) -> bool {
    let taken = rule.host().key_down(k, KeyModifiers::None);
    rule.host().key_up(k, KeyModifiers::None);
    taken
}

fn shift_tab(rule: &KompozTestRule) -> bool {
    let taken = rule.host().key_down(Key::Tab, KeyModifiers::Shift);
    rule.host().key_up(Key::Tab, KeyModifiers::Shift);
    taken
}

/// Whether the node tagged `tag` has the focus.
fn at(rule: &KompozTestRule, tag: &str) -> bool {
    let node = rule.on_node_with_tag(tag).layout_node();
    rule.host().focused_node().is_some_and(|f| Rc::ptr_eq(&f, &node))
}

fn focused_tag(rule: &KompozTestRule) -> String {
    let Some(f) = rule.host().focused_node() else { return "none".into() };
    rule.all_nodes(true)
        .into_iter()
        .find(|n| Rc::ptr_eq(n.layout_node(), &f))
        .and_then(|n| n.config().get(kompoz_ui::semantics::SemanticsProperties::TestTag))
        .unwrap_or_else(|| "?".into())
}

fn name(s: FocusState) -> &'static str {
    if s.is_focused() {
        "focused"
    } else if s.has_focus() {
        "inside"
    } else {
        "none"
    }
}

/// A clickable `size` px, its top left at (`x`, `y`) of the box it is in (C++'s tests' buttons): focusable in
/// keyboard mode, as the keys these tests press put the UI in.
#[kompozable(inline)]
fn Target(x: i32, y: i32, size: i32, tag: &'static str) {
    Box(
        Modifier.padding_sides_with(x.dp(), y.dp(), 0.dp(), 0.dp()),
        content = move || {
            Box(Modifier.size(size.dp()).clickable(|| {}).test_tag(tag));
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Ported from C++

fn arrow_keys_move_focus_to_the_nearest_one_that_way() {
    // C++'s arrow_keys_move_focus_to_the_nearest_one_that_way.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                for row in 0..3 {
                    Row(content = move || {
                        for col in 0..3 {
                            Box(Modifier
                                .size(50.dp())
                                .padding(5.dp())
                                .clickable(|| {})
                                .test_tag(format!("{row}{col}")));
                        }
                    });
                }
            },
        );
    }));
    assert!(key(&rule, Key::Tab) && at(&rule, "00"));
    key(&rule, Key::DirectionRight);
    key(&rule, Key::DirectionDown);
    assert!(at(&rule, "11"));
    key(&rule, Key::DirectionDown);
    key(&rule, Key::DirectionLeft);
    assert!(at(&rule, "20"));
    assert!(!key(&rule, Key::DirectionLeft) && at(&rule, "20")); // nothing further that way
    key(&rule, Key::DirectionUp);
    assert!(at(&rule, "10"));
}

fn focus_groups_keep_arrow_keys_inside_first() {
    // C++'s focus_groups_keep_arrow_keys_inside_first: a1 and a2 in a 240 x 340 box (a group or not); b, d and c
    // around it.
    let screen = |grouped: bool| {
        kompozable_lambda!(move || {
            Box(
                Modifier.size_wh(400.dp(), 600.dp()),
                content = move || {
                    let group = if grouped {
                        Modifier.size_wh(240.dp(), 340.dp()).focus_group()
                    } else {
                        Modifier.size_wh(240.dp(), 340.dp())
                    };
                    Box(
                        group,
                        content = || {
                            Target(0, 0, 40, "a1");
                            Target(200, 150, 40, "a2");
                        },
                    );
                    Target(100, 0, 40, "b");
                    Target(150, 300, 40, "d");
                    Target(0, 500, 40, "c");
                },
            );
        })
    };
    {
        let rule = KompozTestRule::new(screen(false)); // no group: the nearest of all
        key(&rule, Key::Tab);
        key(&rule, Key::DirectionRight);
        assert!(at(&rule, "b"));
        shift_tab(&rule); // a2
        assert!(at(&rule, "a2"));
        key(&rule, Key::DirectionLeft);
        assert!(at(&rule, "d"), "{}", focused_tag(&rule));
        key(&rule, Key::Tab); // c
        assert!(at(&rule, "c"));
        key(&rule, Key::DirectionUp);
        assert!(at(&rule, "d"), "{}", focused_tag(&rule));
    }
    let rule = KompozTestRule::new(screen(true));
    key(&rule, Key::Tab);
    key(&rule, Key::DirectionRight);
    assert!(at(&rule, "a2")); // inside the group first, though b is nearer
    assert!(!key(&rule, Key::DirectionRight) && at(&rule, "a2")); // nothing that way, in it or around it
    key(&rule, Key::DirectionLeft);
    assert!(at(&rule, "a1")); // not d
    key(&rule, Key::Tab); // a2
    key(&rule, Key::DirectionDown);
    assert!(at(&rule, "d")); // nothing below it in the group: out of it
    key(&rule, Key::Tab); // Tab still goes in order: d, c
    assert!(at(&rule, "c"));
    // From outside, the group is one target (in c's beam, nearer than d), entered at its best candidate that way:
    // a2 (a1's beam doesn't beat a2, which is nearer).
    key(&rule, Key::DirectionUp);
    assert!(at(&rule, "a2"), "{}", focused_tag(&rule));
}

fn nested_focus_groups_count_as_their_outermost_from_outside() {
    // C++'s nested_focus_groups_count_as_their_outermost_from_outside: G holds e and the group H (holding h1) at
    // its bottom right; c is under it all.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            Modifier.size_wh(400.dp(), 600.dp()),
            content = || {
                Box(
                    Modifier.size_wh(400.dp(), 340.dp()).focus_group(),
                    content = || {
                        Box(
                            Modifier.padding_sides_with(300.dp(), 240.dp(), 0.dp(), 0.dp()),
                            content = || {
                                Box(Modifier.size(100.dp()).focus_group(), content = || Target(60, 0, 40, "h1"));
                            },
                        );
                        Target(310, 300, 40, "e");
                    },
                );
                Target(330, 500, 40, "c");
            },
        );
    }));
    for _ in 0..3 {
        key(&rule, Key::Tab); // h1, e, c
    }
    assert!(at(&rule, "c"));
    // Up: G is the target (the outermost group), entered at its best candidate: H (as near as e, nearer across),
    // then H's h1 (Compose's search goes into the groups it lands on; C++ stopped at e).
    key(&rule, Key::DirectionUp);
    assert!(at(&rule, "h1"), "{}", focused_tag(&rule));
}

fn focus_properties_skip_and_redirect_the_focus() {
    // C++'s focus_properties_skip_and_redirect_the_focus, a target that can't be focused for its text field; the
    // properties before the clickables, as Compose's apply to the targets after them (C++ put them after).
    let (first, last, field) = (FocusRequester::new(), FocusRequester::new(), FocusRequester::new());
    let (f1, l1, d1) = (first.clone(), last.clone(), field.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (f, l, d) = (f1.clone(), l1.clone(), d1.clone());
        Column(
            content = move || {
                let (f2, l2) = (f.clone(), l.clone());
                Row(content = move || {
                    let (next, right) = (l2.clone(), FocusRequester::Cancel);
                    Box(Modifier
                        .size(50.dp())
                        .focus_requester(&f2)
                        .focus_properties(move |p| {
                            p.next = next.clone();
                            p.right = right.clone();
                        })
                        .clickable(|| {})
                        .test_tag("bir"));
                    Box(Modifier
                        .size(50.dp())
                        .focus_properties(|p| p.can_focus = false)
                        .clickable(|| {})
                        .test_tag("iki"));
                    Box(Modifier.size(50.dp()).clickable(|| {}).test_tag("üç"));
                });
                Box(Modifier
                    .size_wh(280.dp(), 56.dp())
                    .focus_requester(&d)
                    .focus_properties(|p| p.can_focus = false)
                    .focus_target()
                    .test_tag("alan"));
                let (previous, down) = (f.clone(), d.clone());
                Box(Modifier
                    .size(50.dp())
                    .focus_requester(&l)
                    .focus_properties(move |p| {
                        p.previous = previous.clone();
                        p.down = down.clone();
                    })
                    .clickable(|| {})
                    .test_tag("son"));
                // A target's properties are its own: the clickable inside one that can't be focused still can.
                Box(
                    Modifier.size(80.dp()).focus_properties(|p| p.can_focus = false).clickable(|| {}).test_tag("dış"),
                    content = || Box(Modifier.size(40.dp()).clickable(|| {}).test_tag("iç")),
                );
            },
        );
    }));
    key(&rule, Key::Tab);
    assert!(at(&rule, "bir"));
    // Cancel: it stays (the nearest that way would be "iki"'s neighbour "üç"); Compose's root takes the key.
    assert!(key(&rule, Key::DirectionRight) && at(&rule, "bir"));
    key(&rule, Key::Tab); // next: straight to "son", past "üç" and the rest
    assert!(at(&rule, "son"));
    shift_tab(&rule); // previous: back to "bir"
    assert!(at(&rule, "bir"));
    key(&rule, Key::DirectionDown); // no say for Down: the nearest below that can take it (past the field)
    assert!(at(&rule, "son"));
    // To one that can't take it: the move fails there (Compose's; C++ went the usual way instead).
    assert!(!rule.host().move_focus(FocusDirection::Down));
    assert!(at(&rule, "son"));
    key(&rule, Key::Tab); // a target's properties are its own: the clickable inside "dış" can take it
    assert!(at(&rule, "iç"));
    key(&rule, Key::DirectionUp);
    key(&rule, Key::DirectionUp);
    assert!(at(&rule, "bir"));
    assert!(!rule.host().move_focus(FocusDirection::Right)); // cancelled
    key(&rule, Key::DirectionDown);
    assert!(at(&rule, "son"));
    // Can't be focused: not by a requester.
    assert!(!field.request_focus());
    rule.wait_for_idle();
    assert!(at(&rule, "son"));
    let _ = (first, last);
}

fn on_focus_event_and_progress_semantics() {
    // C++'s on_focus_event_and_progress_semantics, the column a focus group
    // (Compose's event nodes see the nearest target below them: the group's state); Compose tells the first state
    // too, and nothing for a move inside the group.
    let events = new_log();
    let changes = new_log();
    let [a, b, far] = FocusRequester::create_refs();
    let (e, c, a1, b1, far1) = (events.clone(), changes.clone(), a.clone(), b.clone(), far.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (e, c, a, b, far) = (e.clone(), c.clone(), a1.clone(), b1.clone(), far1.clone());
        Column(
            content = move || {
                let (e, c, a, b) = (e.clone(), c.clone(), a.clone(), b.clone());
                Column(
                    Modifier
                        .on_focus_event(move |s| e.borrow_mut().push(name(s).to_string()))
                        .on_focus_changed(move |s| c.borrow_mut().push(name(s).to_string()))
                        .focus_group(),
                    content = move || {
                        Box(Modifier.size(20.dp()).focus_requester(&a).focusable());
                        Box(Modifier.size(20.dp()).focus_requester(&b).focusable());
                    },
                );
                Box(Modifier.size(20.dp()).focus_requester(&far).focusable());
                Box(Modifier.size(50.dp()).test_tag("ilerleme").progress_semantics_with(15.0, 0.0..=10.0, 4));
                Box(Modifier.size(50.dp()).test_tag("belirsiz").progress_semantics_indeterminate());
            },
        );
    }));
    a.request_focus();
    rule.wait_for_idle();
    b.request_focus(); // the focus moves inside: nothing for the group (C++ told an event)
    rule.wait_for_idle();
    far.request_focus(); // it leaves
    rule.wait_for_idle();
    a.request_focus();
    rule.wait_for_idle();
    rule.host().clear_focus(false);
    rule.wait_for_idle();
    assert_eq!(taken(&events), ["none", "inside", "none", "inside", "none"]);
    assert_eq!(taken(&changes), ["none", "inside", "none", "inside", "none"]);
    // ...and nothing while the focus moves elsewhere.
    far.request_focus();
    rule.wait_for_idle();
    rule.host().clear_focus(false);
    rule.wait_for_idle();
    assert!(taken(&events).is_empty());
    rule.on_node_with_tag("ilerleme").assert_range_info_equals(semantics::ProgressBarRangeInfo::new(
        10.0,
        0.0..=10.0,
        4,
    ));
    rule.on_node_with_tag("belirsiz").assert_range_info_equals(semantics::ProgressBarRangeInfo::Indeterminate);
}

fn key_events_preview_down_then_focused_then_bubble_up() {
    // C++'s key_events_preview_down_then_field_then_bubble_up, a focus target for its text field (the typing part
    // waits for the text stage).
    let log = new_log();
    let r = FocusRequester::new();
    let handler = |log: Log, name: &'static str, stop: bool| {
        move |e: &KeyEvent| {
            let kind = if e.r#type == KeyEventType::KeyDown { "down" } else { "up" };
            log.borrow_mut().push(format!("{name} {kind}"));
            stop && e.key == Key::F2
        }
    };
    let (l, r1) = (log.clone(), r.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, r) = (l.clone(), r1.clone());
        Box(
            Modifier.on_preview_key_event(handler(l.clone(), "outer.preview", false)).on_key_event(handler(
                l.clone(),
                "outer",
                false,
            )),
            content = move || {
                let (l, r) = (l.clone(), r.clone());
                Box(
                    Modifier.on_preview_key_event(handler(l.clone(), "inner.preview", true)).on_key_event(handler(
                        l.clone(),
                        "inner",
                        false,
                    )),
                    content = move || Box(Modifier.size(10.dp()).focus_requester(&r).focus_target()),
                );
            },
        );
    }));
    r.request_focus();
    rule.wait_for_idle();
    rule.host().key_down(Key::F1, KeyModifiers::None);
    assert_eq!(taken(&log), ["outer.preview down", "inner.preview down", "inner down", "outer down"]);
    assert!(rule.host().key_down(Key::F2, KeyModifiers::None)); // the inner preview takes it: nothing below sees it
    assert_eq!(taken(&log), ["outer.preview down", "inner.preview down"]);
    rule.host().key_up(Key::F1, KeyModifiers::None);
    let l = taken(&log);
    assert!(l.len() == 4 && l.last().unwrap() == "outer up", "{l:?}");
    // A key up without its down is dropped (Compose's validateKeyEvent).
    assert!(!rule.host().key_up(Key::F3, KeyModifiers::None));
    assert!(taken(&log).is_empty());
}

fn focus_restorers_bring_the_focus_back_where_it_left() {
    // C++'s focus_restorers_bring_the_focus_back_where_it_left, focus targets for its buttons and the rows focus
    // groups (how Compose's focusRestorer is used).
    let (fallback, whole_row) = (FocusRequester::new(), FocusRequester::new());
    let (fb, wr) = (fallback.clone(), whole_row.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (fb, wr) = (fb.clone(), wr.clone());
        Column(
            content = move || {
                Box(Modifier.size(50.dp()).focus_target().test_tag("x"));
                Row(
                    Modifier.focus_restorer_with(FocusRequester::Default).focus_requester(&wr).focus_group(),
                    content = || {
                        for t in ["a", "b", "c"] {
                            Box(Modifier.size(50.dp()).focus_target().test_tag(t));
                        }
                    },
                );
                let fb2 = fb.clone();
                Row(
                    Modifier.focus_restorer_with(fb.clone()).focus_group(),
                    content = move || {
                        Box(Modifier.size(50.dp()).focus_target().test_tag("d"));
                        Box(Modifier.size(50.dp()).focus_requester(&fb2).focus_target().test_tag("e"));
                    },
                );
            },
        );
    }));
    for _ in 0..3 {
        key(&rule, Key::Tab); // x, a, b
    }
    assert!(at(&rule, "b"));
    key(&rule, Key::DirectionUp); // out of the row, up to x
    assert!(at(&rule, "x"));
    key(&rule, Key::DirectionDown); // back in: at b, where it left, not a under x
    assert!(at(&rule, "b"), "{}", focused_tag(&rule));
    key(&rule, Key::Tab);
    key(&rule, Key::Tab); // c, then into the second row: nothing kept there yet, its fallback
    assert!(at(&rule, "e"), "{}", focused_tag(&rule));
    whole_row.request_focus(); // the first row asked for: back at c, where it left it
    rule.wait_for_idle();
    assert!(at(&rule, "c"), "{}", focused_tag(&rule));
}

#[kompozable]
fn RestorableRow(shown: MutableState<bool>) {
    if shown.get() {
        Column(
            content = || {
                Box(Modifier.size(50.dp()).focus_target().test_tag("x"));
                Row(
                    Modifier.focus_restorer().focus_group(),
                    content = || {
                        for t in ["a", "b", "c"] {
                            Box(Modifier.size(50.dp()).focus_target().test_tag(t));
                        }
                    },
                );
            },
        );
    }
}

/// The same content for both hosts (what a restorer saves is found by where it is composed).
fn restorable(shown: &MutableState<bool>) -> impl Content + 'static {
    let s = shown.clone();
    kompozable_lambda!(move || RestorableRow(s.clone()))
}

fn focus_restoration_outlives_the_process() {
    // Compose's FocusRestorer: the child a target noted is registered with the saveable state registry where it is
    // (the host's), saved with the host's state and consumed back where the target, made anew, restores.
    let bytes = {
        let shown = mutable_state_of(true);
        let rule = KompozTestRule::new(restorable(&shown));
        for _ in 0..3 {
            key(&rule, Key::Tab); // x, a, b
        }
        key(&rule, Key::DirectionUp); // out of the row: it notes b
        assert!(at(&rule, "x"), "{}", focused_tag(&rule));
        rule.host().save_state()
    };
    let shown = mutable_state_of(false);
    let rule = KompozTestRule::new(restorable(&shown));
    rule.host().restore_state(&bytes);
    shown.set(true);
    rule.wait_for_idle();
    key(&rule, Key::Tab); // x
    key(&rule, Key::DirectionDown); // into the row: back at b, as the first host left it
    assert!(at(&rule, "b"), "{}", focused_tag(&rule));
    // Consumed: what the registry was given back is gone; what is noted now is saved instead.
    let saved = SavedValue::decode(&rule.host().save_state()).unwrap();
    let provided = saved.get("composition").get("providers");
    assert_eq!(provided.as_map().map(|m| m.len()), Some(1));
}

// ---------------------------------------------------------------------------------------------------------------
// Compose's behaviour

fn arrow_keys_find_focusables_where_an_offset_put_them() {
    // C++'s arrow_keys_find_focusables_where_an_offset_put_them (its scrolling part waits for the scroll
    // containers).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            Modifier.size(400.dp()),
            content = || {
                Box(Modifier.size(40.dp()).clickable(|| {}).test_tag("a"));
                Box(Modifier.offset_with(200.dp(), 0.dp()).size(40.dp()).clickable(|| {}).test_tag("b")); // moved right
                Box(Modifier.offset_with(0.dp(), 150.dp()).size(40.dp()).clickable(|| {}).test_tag("c")); // and down
            },
        );
    }));
    key(&rule, Key::Tab);
    key(&rule, Key::DirectionRight);
    assert!(at(&rule, "b")); // where it is drawn and tapped, not where the node would be without the offset
    key(&rule, Key::DirectionLeft);
    key(&rule, Key::DirectionDown);
    assert!(at(&rule, "c"));
}

fn tab_goes_in_placement_order_and_wraps_around() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Row(content = || {
                    Box(Modifier.size(20.dp()).focus_target().test_tag("1"));
                    Box(Modifier.size(20.dp()).focus_target().test_tag("2"));
                });
                Box(Modifier.size(20.dp()).focus_target().test_tag("3"));
            },
        );
    }));
    assert_eq!(rule.host().focused_node().map(|n| n.id()), None);
    let order: Vec<String> = (0..4)
        .map(|_| {
            key(&rule, Key::Tab);
            focused_tag(&rule)
        })
        .collect();
    assert_eq!(order, ["1", "2", "3", "1"]); // wraps around (Compose's moveFocus)
    let back: Vec<String> = (0..2)
        .map(|_| {
            shift_tab(&rule);
            focused_tag(&rule)
        })
        .collect();
    assert_eq!(back, ["3", "2"]);
    // Escape (Exit) with nothing around the focused target that can take it: not taken, back goes nowhere.
    assert!(!key(&rule, Key::Escape));
    assert!(at(&rule, "2"));
}

fn requests_states_capture_and_clear() {
    let states = new_log();
    let requester = FocusRequester::new();
    let shown = mutable_state_of(true);
    let (s, r, sh) = (states.clone(), requester.clone(), shown.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, r, sh) = (s.clone(), r.clone(), sh.clone());
        Box(
            Modifier.size(60.dp()).on_focus_changed(move |st| s.borrow_mut().push(format!("{st:?}"))),
            content = move || {
                if sh.get() {
                    Box(Modifier.size(50.dp()).focus_requester(&r).focus_target());
                }
            },
        );
    }));
    assert_eq!(taken(&states), ["Inactive"]); // its first state, as Compose's
    assert!(requester.request_focus());
    assert_eq!(taken(&states), ["Active"]);
    assert!(requester.capture_focus());
    assert_eq!(taken(&states), ["Captured"]);
    assert!(!rule.host().clear_focus(false)); // captured: not without force
    assert!(rule.host().focused_node().is_some());
    assert!(requester.free_focus());
    assert_eq!(taken(&states), ["Active"]);
    assert!(requester.capture_focus());
    assert!(rule.host().clear_focus(true)); // forced
    assert_eq!(taken(&states), ["Captured", "Inactive"]);
    assert!(!requester.free_focus());
    // Requested again, then the node goes: the focus goes with it, its node told.
    requester.request_focus();
    assert!(rule.host().focused_node().is_some());
    taken(&states);
    shown.set(false);
    rule.wait_for_idle();
    assert!(rule.host().focused_node().is_none());
    assert_eq!(taken(&states), ["Inactive"]);
}

fn a_requester_works_from_the_first_composition_and_without_layout() {
    // Compose's: a request from a LaunchedEffect (the nodes attached, nothing laid out yet) takes the focus.
    let requester = FocusRequester::new();
    let r = requester.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let r2 = r.clone();
        LaunchedEffect((), async move {
            r2.request_focus();
        });
        Box(Modifier.size(30.dp()).focus_requester(&r).focus_target().test_tag("t"));
    }));
    assert!(at(&rule, "t"));
}

fn custom_enter_and_exit_redirect_or_cancel() {
    let (inner, outside) = (FocusRequester::new(), FocusRequester::new());
    let block_exit = mutable_state_of(false);
    let (i, o, be) = (inner.clone(), outside.clone(), block_exit.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (i, o, be) = (i.clone(), o.clone(), be.clone());
        Column(
            content = move || {
                Box(Modifier.size(20.dp()).focus_requester(&o).focus_target().test_tag("dışarı"));
                let (i2, be2) = (i.clone(), be.clone());
                Column(
                    Modifier
                        .focus_properties(move |p| {
                            let i3 = i2.clone();
                            p.set_on_enter(move |_| {
                                i3.request_focus();
                            });
                            let be3 = be2.clone();
                            p.set_on_exit(move |scope| {
                                if be3.peek() {
                                    scope.cancel_focus_change();
                                }
                            });
                        })
                        .focus_group(),
                    content = move || {
                        Box(Modifier.size(20.dp()).focus_target().test_tag("ilk"));
                        Box(Modifier.size(20.dp()).focus_requester(&i).focus_target().test_tag("hedef"));
                    },
                );
            },
        );
    }));
    key(&rule, Key::Tab);
    assert!(at(&rule, "dışarı"));
    key(&rule, Key::Tab); // entering the group: its on_enter sends the focus to "hedef", not its first
    assert!(at(&rule, "hedef"), "{}", focused_tag(&rule));
    block_exit.set(true);
    rule.wait_for_idle();
    assert!(!outside.request_focus()); // leaving is cancelled
    assert!(at(&rule, "hedef"));
    block_exit.set(false);
    rule.wait_for_idle();
    assert!(outside.request_focus());
    assert!(at(&rule, "dışarı"));
}

fn properties_read_state_and_lose_the_focus_when_they_say_so() {
    let can = mutable_state_of(true);
    let c = can.clone();
    let requester = FocusRequester::new();
    let r = requester.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, r) = (c.clone(), r.clone());
        Box(Modifier
            .size(20.dp())
            .focus_requester(&r)
            .focus_properties(move |p| p.can_focus = c.get())
            .focus_target()
            .test_tag("t"));
    }));
    requester.request_focus();
    assert!(at(&rule, "t"));
    can.set(false); // what the properties read changed: the focus is cleared
    rule.wait_for_idle();
    assert!(rule.host().focused_node().is_none());
    assert!(!requester.request_focus());
}

fn system_defined_focusability_follows_the_input_mode() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(Modifier.size(20.dp()).then_element(SystemDefinedTarget).test_tag("t"));
    }));
    let mode = rule.on_node_with_tag("t").layout_node().current_value_of(&LocalInputModeManager);
    // Touch mode (the start): it can't take the focus.
    assert_eq!(mode.input_mode(), InputMode::Touch);
    assert!(!rule.host().move_focus(FocusDirection::Next));
    // A key puts the UI in keyboard mode: then it can.
    assert!(key(&rule, Key::Tab));
    assert_eq!(mode.input_mode(), InputMode::Keyboard);
    assert!(at(&rule, "t"));
    // A finger puts it back in touch mode, and it can no longer keep the focus.
    rule.on_node_with_tag("t").perform_touch_input(|s| s.click());
    assert_eq!(mode.input_mode(), InputMode::Touch);
    assert!(!rule.host().move_focus(FocusDirection::Next) || !at(&rule, "t"));
}

#[derive(PartialEq)]
struct SystemDefinedTarget;

impl ModifierNodeElement for SystemDefinedTarget {
    type Node = FocusTargetNode;
    fn create(&self) -> FocusTargetNode {
        FocusTargetNode::new(Focusability::SystemDefined, None)
    }
    fn update(&self, _node: &mut FocusTargetNode) {}
}

fn enter_goes_into_the_focused_group_and_exit_out() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Box(
            Modifier.size(100.dp()).focus_target().test_tag("kap"),
            content = || {
                Row(content = || {
                    Box(Modifier.size(20.dp()).focus_target().test_tag("sol"));
                    Box(Modifier.size(20.dp()).focus_target().test_tag("sağ"));
                });
            },
        );
    }));
    key(&rule, Key::Tab);
    assert!(at(&rule, "kap"));
    key(&rule, Key::Enter); // into it: its first from the start
    assert!(at(&rule, "sol"), "{}", focused_tag(&rule));
    key(&rule, Key::Escape); // out: the target around it that can take it
    assert!(at(&rule, "kap"));
}

fn the_focused_rect_and_focusable_content() {
    let requester = FocusRequester::new();
    let r = requester.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let r = r.clone();
        Box(
            Modifier.padding(10.dp()),
            content = move || {
                Box(Modifier
                    .size(30.dp())
                    .focus_requester(&r)
                    .focus_properties(|p| p.focus_rect = Some(Rect::new(1.0, 2.0, 3.0, 4.0)))
                    .focus_target());
            },
        );
    }));
    assert!(rule.host().focus.has_focusable_content());
    requester.request_focus();
    // Its rect in the root: the one its properties give, moved to where it is.
    assert_eq!(rule.host().focus.focus_rect(), Some(Rect::new(11.0, 12.0, 13.0, 14.0)));
}

fn rotary_and_soft_keyboard_events_go_along_the_focused_path() {
    let log = new_log();
    let r = FocusRequester::new();
    let (l, r1) = (log.clone(), r.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, r) = (l.clone(), r1.clone());
        let (l1, l2, l3) = (l.clone(), l.clone(), l.clone());
        Box(
            Modifier
                .on_pre_rotary_scroll_event(move |e| {
                    l1.borrow_mut().push(format!("pre {}", e.vertical_scroll_pixels));
                    false
                })
                .on_rotary_scroll_event(move |e| {
                    l2.borrow_mut().push(format!("rotary {}", e.vertical_scroll_pixels));
                    true
                })
                .on_intercept_key_before_soft_keyboard(move |e| {
                    l3.borrow_mut().push(format!("soft {}", e.key.native_key_code()));
                    e.key == Key::A
                }),
            content = move || Box(Modifier.size(10.dp()).focus_requester(&r).focus_target()),
        );
    }));
    let event = RotaryScrollEvent {
        vertical_scroll_pixels: 5.0,
        horizontal_scroll_pixels: 0.0,
        uptime_millis: 0,
        input_device_id: 0,
    };
    assert!(!rule.host().rotary_scroll(event)); // nothing focused
    r.request_focus();
    assert!(rule.host().rotary_scroll(event));
    assert_eq!(taken(&log), ["pre 5", "rotary 5"]);
    assert!(rule.host().key_down(Key::A, KeyModifiers::None)); // taken before the keys
    assert_eq!(taken(&log), [format!("soft {}", Key::A.native_key_code())]);
}

fn key_input_injected_on_a_node() {
    let log = new_log();
    let r = FocusRequester::new();
    let (l, r1) = (log.clone(), r.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (l, r) = (l.clone(), r1.clone());
        Box(Modifier
            .on_key_event(move |e| {
                let mods = if e.is_shift_pressed() { "+shift" } else { "" };
                let kind = if e.r#type == KeyEventType::KeyDown { "down" } else { "up" };
                l.borrow_mut()
                    .push(format!("{kind} {}{mods}", char::from_u32(e.utf16_code_point as u32).unwrap_or('?')));
                false
            })
            .size(10.dp())
            .focus_requester(&r)
            .focus_target()
            .test_tag("t"));
    }));
    r.request_focus();
    rule.on_node_with_tag("t").perform_key_input(|k| {
        k.press_key(Key::A);
        k.with_key_down(Key::ShiftLeft, |k| k.press_key(Key::B));
        k.with_key_toggled(Key::CapsLock, |k| k.press_key(Key::C));
    });
    let l = taken(&log);
    assert!(l.contains(&"down a".to_string()) && l.contains(&"up a".to_string()), "{l:?}");
    assert!(l.contains(&"down B+shift".to_string()), "{l:?}");
    assert!(l.contains(&"down C".to_string()), "{l:?}");
    let taken_key = rule.on_node_with_tag("t").perform_key_press(KeyEvent::new(
        Key::Tab,
        KeyEventType::KeyDown,
        KeyModifiers::None,
    ));
    assert!(taken_key);
}

tests!(
    focus_restoration_outlives_the_process,
    arrow_keys_move_focus_to_the_nearest_one_that_way,
    focus_groups_keep_arrow_keys_inside_first,
    nested_focus_groups_count_as_their_outermost_from_outside,
    focus_properties_skip_and_redirect_the_focus,
    on_focus_event_and_progress_semantics,
    key_events_preview_down_then_focused_then_bubble_up,
    focus_restorers_bring_the_focus_back_where_it_left,
    arrow_keys_find_focusables_where_an_offset_put_them,
    tab_goes_in_placement_order_and_wraps_around,
    requests_states_capture_and_clear,
    a_requester_works_from_the_first_composition_and_without_layout,
    custom_enter_and_exit_redirect_or_cancel,
    properties_read_state_and_lose_the_focus_when_they_say_so,
    system_defined_focusability_follows_the_input_mode,
    enter_goes_into_the_focused_group_and_exit_out,
    the_focused_rect_and_focusable_content,
    rotary_and_soft_keyboard_events_go_along_the_focused_path,
    key_input_injected_on_a_node,
);

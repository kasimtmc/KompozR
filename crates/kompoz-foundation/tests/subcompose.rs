//! SubcomposeLayout and BoxWithConstraints: slots composed as the layout measures (the C++ Kompoz's
//! SubkompozLayout and BoxWithConstraints tests), slots kept for reuse, precomposed (at once and a piece at a
//! time), and the panic of asking a subcomposing layout for an intrinsic size.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::layout::{
    NoIntrinsicsMessage, SlotIdsSet, SlotKey, SubcomposeLayout, SubcomposeLayoutState, SubcomposeSlotReusePolicy,
};
use kompoz_ui::modifier::{TraversableNode, TraversalDecision};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---- Helpers ---------------------------------------------------------------------------------------------------

/// A node's bounds in the root: x, y, width, height (px).
fn bounds(rule: &KompozTestRule, text_or_tag: &str, by_text: bool) -> [f32; 4] {
    let node = if by_text { rule.on_node_with_text(text_or_tag) } else { rule.on_node_with_tag(text_or_tag) };
    let b = node.get_unclipped_bounds_in_root();
    [b.left, b.top, b.width(), b.height()]
}

fn tag(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    bounds(rule, tag, false)
}

fn text(rule: &KompozTestRule, text: &str) -> [f32; 4] {
    bounds(rule, text, true)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn exists(rule: &KompozTestRule, text: &str) -> bool {
    rule.on_node_with_text(text).exists()
}

/// Runs `f`, which is expected to panic, without the panic printed; its message.
fn panic_message(f: impl FnOnce()) -> String {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(hook);
    let payload = r.expect_err("it panics");
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default()
}

// ---- C++ 10768: box_with_constraints_and_on_size_changed ---------------------------------------------------------

fn box_with_constraints_and_on_size_changed() {
    let reported = Rc::new(Cell::new(IntSize::Zero));
    let r = reported.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let r = r.clone();
            BoxWithConstraints(
                modifier = Modifier.fill_max_width().on_size_changed(move |s| r.set(s)),
                content = |scope: &BoxWithConstraintsScope| {
                    Text(if scope.max_width() < 420.dp() { "dar" } else { "geniş" });
                },
            );
        }),
        IntSize::new(400, 800),
    );
    assert!(exists(&rule, "dar"));
    assert_eq!(reported.get().width, 400);
    rule.host().set_viewport_size(IntSize::new(600, 800));
    rule.wait_for_idle();
    assert!(exists(&rule, "geniş") && !exists(&rule, "dar"));
    assert_eq!(reported.get().width, 600);
}

// ---- C++ 12047: subkompoz_layout_composes_a_slot_as_it_measures --------------------------------------------------

/// A slot asked for the first time is composed there and then: its measurables come back on the first pass; one
/// whose content changed is composed again; one no longer asked for leaves.
fn subcompose_layout_composes_a_slot_as_it_measures() {
    let first_pass = Rc::new(RefCell::new(Vec::new()));
    let width = mutable_state_of(10.0f32);
    let show_badge = mutable_state_of(true);
    let (f, w, b) = (first_pass.clone(), width.clone(), show_badge.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (f, w, b) = (f.clone(), w.clone(), b.clone());
        SubcomposeLayout(
            modifier = Modifier.test_tag("alt"),
            measure_policy = move |s, c: Constraints| {
                let width = w.get();
                let main = s.subcompose("ana", move || {
                    Box(modifier = Modifier.size_wh(width.dp(), 20.dp()).test_tag("ana"));
                });
                f.borrow_mut().push(main.len());
                let loose = c.copy_max_dimensions();
                let main: Vec<Placeable> = main.iter().map(|m| m.measure(loose)).collect();
                let (right, height) = main.last().map_or((0, 0), |p| (p.width(), p.height()));
                let badges: Vec<Placeable> = if b.get() {
                    s.subcompose("rozet", || {
                        Box(modifier = Modifier.size(5.dp()).test_tag("rozet"));
                    })
                    .iter()
                    .map(|m| m.measure(loose))
                    .collect()
                } else {
                    Vec::new()
                };
                s.layout(right + 5, height, move |p| {
                    for m in &main {
                        p.place(m, 0, 0);
                    }
                    for badge in &badges {
                        p.place(badge, right, 0);
                    }
                })
            },
        );
    }));
    let d = rule.density().density;
    assert_eq!(first_pass.borrow().first(), Some(&1));
    assert!(near(tag(&rule, "ana")[2], 10.0 * d) && near(tag(&rule, "rozet")[0], 10.0 * d));
    width.set(30.0);
    rule.wait_for_idle();
    assert!(near(tag(&rule, "ana")[2], 30.0 * d) && near(tag(&rule, "rozet")[0], 30.0 * d));
    show_badge.set(false);
    rule.wait_for_idle();
    assert!(!rule.on_node_with_tag("rozet").exists() && rule.on_node_with_tag("ana").exists());
}

// ---- C++ 12013: box_with_constraints_composes_with_its_constraints_the_first_time ------------------------------

/// The content's first composition already sees the width it is laid out in, and it is composed again when that or
/// what it shows changes.
fn box_with_constraints_composes_with_its_constraints_the_first_time() {
    let widths = Rc::new(RefCell::new(Vec::new()));
    let label = mutable_state_of("bir".to_string());
    let (w, l) = (widths.clone(), label.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (w, l) = (w.clone(), l.clone());
            BoxWithConstraints(
                modifier = Modifier.fill_max_width(),
                content_alignment = Alignment::Center,
                content = move |scope: &BoxWithConstraintsScope| {
                    w.borrow_mut().push(scope.max_width());
                    let size = if scope.max_width() < 420.dp() { "dar" } else { "geniş" };
                    Text(format!("{} {size}", l.get()), modifier = Modifier.test_tag("metin"));
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let d = rule.density().density;
    assert_eq!(widths.borrow().first().copied(), Some(Dp(400.0 / d)));
    assert!(exists(&rule, "bir dar"));
    let t = tag(&rule, "metin");
    assert!((t[0] + t[2] / 2.0 - 200.0).abs() <= 1.0, "{t:?}"); // centered in it (to the pixel)
    label.set("iki".into());
    rule.wait_for_idle();
    assert!(exists(&rule, "iki dar"));
    rule.host().set_viewport_size(IntSize::new(600, 800));
    rule.wait_for_idle();
    assert!(exists(&rule, "iki geniş") && !exists(&rule, "iki dar"));
}

// ---- C++ 14074: custom_layouts_take_compose_forms -----------------------------------------------------------------

/// Layout's measure policy with layout(w, h) { … }, SubcomposeLayout's layout(w, h), Constraints.copy by name,
/// placeRelative(x = …, y = …); the divider is a Box of its thickness (Material's HorizontalDivider is stage 12's).
fn custom_layouts_take_compose_forms() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Layout(
                    content = || {
                        Box(modifier = Modifier.size(20.dp()).test_tag("bir"));
                        Box(modifier = Modifier.size(20.dp()).test_tag("iki"));
                    },
                    modifier = Modifier.test_tag("merdiven"),
                    measure_policy = |scope: &MeasureScope, children: &[&dyn Measurable], c: Constraints| {
                        let placed: Vec<Placeable> =
                            children.iter().map(|m| m.measure(c.copy(min_width = 0, min_height = 0))).collect();
                        let step = scope.round_to_px(30.dp());
                        let down = scope.round_to_px(20.dp());
                        scope.layout(scope.round_to_px(60.dp()), scope.round_to_px(40.dp()), move |p| {
                            p.place_relative(&placed[0], x = 0, y = 0);
                            p.place_relative(&placed[1], x = step, y = down);
                        })
                    },
                );
                SubcomposeLayout(
                    modifier = Modifier.test_tag("alt"),
                    measure_policy = |s, c: Constraints| {
                        // (composed now, as Compose's subcompose does: measurable on the first pass)
                        let parts = s.subcompose("kutu", || {
                            Box(modifier = Modifier.size_wh(10.dp(), 30.dp()));
                        });
                        let p = parts[0].measure(c.copy_max_dimensions());
                        let (w, h) = (p.width(), p.height());
                        s.layout(w, h, move |scope| scope.place_relative(&p, 0, 0))
                    },
                );
                Box(modifier = Modifier.fill_max_width().height(3.dp()).test_tag("çizgi"));
            },
        );
    }));
    let d = rule.density().density;
    let (stairs, two) = (tag(&rule, "merdiven"), tag(&rule, "iki"));
    assert!(near(stairs[2], 60.0 * d) && near(stairs[3], 40.0 * d));
    assert!(near(two[0] - stairs[0], 30.0 * d) && near(two[1] - stairs[1], 20.0 * d));
    assert!(near(tag(&rule, "alt")[3], 30.0 * d));
    assert!(near(tag(&rule, "çizgi")[3], 3.0 * d));
    let c = Constraints::new(0, 100, 0, 50);
    assert_eq!(c.offset(-20, -10), Constraints::new(0, 80, 0, 40));
    assert_eq!(c.constrain_width(150), 100);
}

// ---- C++ 14132 (adapted): subkompoz_layout_is_not_composed_again_for_a_probe_measure ------------------------------

/// C++'s SubkompozLayout answered a probe measure (`width(IntrinsicSize::Max)`) without composing for it; Compose's
/// SubcomposeLayout throws when asked for an intrinsic size, and so does this one. Measured for real, the slot is
/// composed once and stays.
fn subcompose_layout_panics_when_asked_for_an_intrinsic_size() {
    let message = panic_message(|| {
        let _rule = KompozTestRule::with_viewport(
            kompozable_lambda!(|| {
                Column(
                    modifier = Modifier.width_intrinsic(IntrinsicSize::Max),
                    content = || {
                        SubcomposeLayout(
                            measure_policy = |s, c: Constraints| {
                                let wide = c.max_width >= 500;
                                let measured: Vec<Placeable> = s
                                    .subcompose(if wide { "geniş" } else { "dar" }, move || {
                                        Text(if wide { "geniş" } else { "dar" });
                                    })
                                    .iter()
                                    .map(|m| m.measure(c.copy_max_dimensions()))
                                    .collect();
                                let height = measured.iter().map(|p| p.height()).max().unwrap_or(0);
                                s.layout(if c.has_bounded_width() { c.max_width } else { 100 }, height, move |p| {
                                    for m in &measured {
                                        p.place(m, 0, 0);
                                    }
                                })
                            },
                        );
                    },
                );
            }),
            IntSize::new(400, 800),
        );
    });
    assert!(message.contains(NoIntrinsicsMessage), "{message}");

    let runs = Rc::new(Cell::new(0));
    let r = runs.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let r = r.clone();
            Column(
                content = move || {
                    let r = r.clone();
                    SubcomposeLayout(
                        measure_policy = move |s, c: Constraints| {
                            let wide = c.max_width >= 500;
                            let r = r.clone();
                            let measured: Vec<Placeable> = s
                                .subcompose(if wide { "geniş" } else { "dar" }, move || {
                                    r.set(r.get() + 1);
                                    Text(if wide { "geniş" } else { "dar" });
                                })
                                .iter()
                                .map(|m| m.measure(c.copy_max_dimensions()))
                                .collect();
                            let height = measured.iter().map(|p| p.height()).max().unwrap_or(0);
                            s.layout(c.max_width, height, move |p| {
                                for m in &measured {
                                    p.place(m, 0, 0);
                                }
                            })
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    rule.wait_for_idle();
    let settled = runs.get();
    for _ in 0..5 {
        rule.advance_frame();
    }
    assert!(exists(&rule, "dar") && !exists(&rule, "geniş"));
    assert_eq!(runs.get(), settled);
    assert!(!rule.host().has_pending_work());
}

// ---- C++ 11931: subkompoz_layout_composes_slots_from_what_it_measured ---------------------------------------------

/// A slot that depends on another's size (a line as wide as the title); slots picked by the constraints, as
/// BoxWithConstraints does, each keeping its state while it is the one asked for. (C++'s TextButton is a clickable
/// Box here: Material is stage 12's.)
fn subcompose_layout_composes_slots_from_what_it_measured() {
    let title = mutable_state_of("Başlık metni".to_string());
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let (t, l) = (title.clone(), log.clone());
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let (t, l) = (t.clone(), l.clone());
            Column(
                content = move || {
                    let t = t.clone();
                    SubcomposeLayout(
                        measure_policy = move |s, c: Constraints| {
                            let loose = c.copy_max_dimensions();
                            let t = t.clone();
                            let main: Vec<Placeable> = s
                                .subcompose("ana", move || {
                                    Text(t.get(), font_size = 16.sp());
                                })
                                .iter()
                                .map(|m| m.measure(loose))
                                .collect();
                            let w = main.iter().map(|p| p.width()).max().unwrap_or(0);
                            let h = main.iter().map(|p| p.height()).max().unwrap_or(0);
                            let line_text = format!("genişlik {:.0}", w as f32 / s.density().density);
                            let line: Vec<Placeable> = s
                                .subcompose("rozet", move || {
                                    Text(line_text.clone(), font_size = 16.sp());
                                })
                                .iter()
                                .map(|m| m.measure(loose))
                                .collect();
                            let total = h + line.iter().map(|p| p.height()).sum::<i32>();
                            s.layout(w, total, move |p| {
                                for m in &main {
                                    p.place(m, 0, 0);
                                }
                                let mut y = h;
                                for m in &line {
                                    p.place(m, 0, y);
                                    y += m.height();
                                }
                            })
                        },
                    );
                    let l = l.clone();
                    SubcomposeLayout(
                        modifier = Modifier.fill_max_width(),
                        measure_policy = move |s, c: Constraints| {
                            let wide = c.max_width >= s.round_to_px(500.dp());
                            let l = l.clone();
                            let measured: Vec<Placeable> = s
                                .subcompose(if wide { "geniş" } else { "dar" }, move || {
                                    let taps = remember(|| mutable_state_of(0));
                                    let l = l.clone();
                                    DisposableEffect(0, move || {
                                        l.borrow_mut().push(if wide { "+geniş" } else { "+dar" }.to_string());
                                        on_dispose(move || {
                                            l.borrow_mut().push(if wide { "-geniş" } else { "-dar" }.to_string())
                                        })
                                    });
                                    let t = taps.clone();
                                    Box(
                                        modifier = Modifier.clickable(move || t.set(t.peek() + 1)),
                                        content = move || {
                                            Text(
                                                format!("{} düzen {}", if wide { "geniş" } else { "dar" }, taps.get()),
                                                font_size = 16.sp(),
                                            );
                                        },
                                    );
                                })
                                .iter()
                                .map(|m| m.measure(c.copy_max_dimensions()))
                                .collect();
                            let height = measured.iter().map(|p| p.height()).max().unwrap_or(0);
                            s.layout(c.max_width, height, move |p| {
                                for m in &measured {
                                    p.place(m, 0, 0);
                                }
                            })
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 800),
    );
    let d = rule.density().density;
    assert!(exists(&rule, "Başlık metni") && exists(&rule, "genişlik 96"));
    assert!(near(text(&rule, "genişlik 96")[1], 20.0 * d)); // under the title
    assert!(exists(&rule, "dar düzen 0") && !exists(&rule, "geniş düzen 0"));
    title.set("Kısa".into()); // the title is narrower: the line that depends on it is composed again
    rule.wait_for_idle();
    assert!(exists(&rule, "genişlik 32") && !exists(&rule, "genişlik 96"));
    rule.on_node_with_text("dar düzen 0").perform_click();
    rule.host().set_viewport_size(IntSize::new(450, 800)); // measured again, the same slot: its state stays
    rule.wait_for_idle();
    assert!(exists(&rule, "dar düzen 1"));
    rule.host().set_viewport_size(IntSize::new(600, 800)); // wide: the other slot, the first one goes
    rule.wait_for_idle();
    assert!(exists(&rule, "geniş düzen 0") && !exists(&rule, "dar düzen 1"));
    // (the new slot composed as it is asked for, as Compose's; the old one leaves after the measure)
    assert_eq!(*log.borrow(), ["+dar", "+geniş", "-dar"]);
}

// ---- Beyond the C++ tests: reuse, precomposition, locals ---------------------------------------------------------

static LocalShade: KompozitionLocal<String> = KompozitionLocal::new(|| "none".into());

/// A slot sees the locals provided where its layout is composed, and is told when they change.
fn slots_see_the_locals_where_the_layout_is() {
    let shade = mutable_state_of("açık".to_string());
    let s = shade.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        KompozitionLocalProvider(&[LocalShade.provides(s.get())], || {
            BoxWithConstraints(
                content = |_: &BoxWithConstraintsScope| {
                    Text(format!("renk {}", LocalShade.current()));
                },
            );
        });
    }));
    assert!(exists(&rule, "renk açık"));
    shade.set("koyu".into());
    rule.wait_for_idle();
    assert!(exists(&rule, "renk koyu") && !exists(&rule, "renk açık"));
}

/// A layout keeping one slot for reuse: a slot no longer asked for keeps its layouts, deactivated, and the next new
/// slot is composed into them (its state starting over). (A slot replacing another in the same measure is a new one:
/// the old one is let go only once the measure placed what it used, as Compose's.)
fn a_slot_kept_for_reuse_is_composed_into_by_the_next() {
    let which = mutable_state_of(Some(0));
    let made = Rc::new(Cell::new(0));
    let state = SubcomposeLayoutState::new(SubcomposeSlotReusePolicy(1));
    let (w, m, st) = (which.clone(), made.clone(), state.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (w, m) = (w.clone(), m.clone());
        SubcomposeLayout(
            state = Some(st.clone()),
            measure_policy = move |s, c: Constraints| {
                let Some(id) = w.get() else { return s.layout(c.max_width, c.max_height, |_| {}) };
                let m = m.clone();
                let measured: Vec<Placeable> = s
                    .subcompose(id, move || {
                        let m = m.clone();
                        let n = remember(move || {
                            m.set(m.get() + 1);
                            m.get()
                        });
                        Box(
                            modifier = Modifier.size(10.dp()).test_tag("öğe"),
                            content = move || {
                                Text(format!("öğe {id} ({n})"));
                            },
                        );
                    })
                    .iter()
                    .map(|m| m.measure(c))
                    .collect();
                s.layout(c.max_width, c.max_height, move |p| {
                    for m in &measured {
                        p.place(m, 0, 0);
                    }
                })
            },
        );
    }));
    assert!(exists(&rule, "öğe 0 (1)"));
    let first = rule.on_node_with_tag("öğe").layout_node();
    let first_id = first.semantics_id();
    which.set(None);
    rule.wait_for_idle();
    // Kept: there, deactivated, out of the semantics.
    assert!(first.is_attached() && first.is_deactivated());
    assert!(!rule.on_node_with_tag("öğe").exists());
    which.set(Some(1));
    rule.wait_for_idle();
    assert!(exists(&rule, "öğe 1 (2)") && !exists(&rule, "öğe 0 (1)"));
    let second = rule.on_node_with_tag("öğe").layout_node();
    assert!(Rc::ptr_eq(&first, &second), "the slot's layout is reused");
    assert_ne!(second.semantics_id(), first_id, "a reused layout is a new node to accessibility");
    assert!(!second.is_deactivated());
}

/// A policy keeping the even slots and reusing them only for even ones.
struct EvenSlots;

impl kompoz_ui::layout::SubcomposeSlotReusePolicy for EvenSlots {
    fn get_slots_to_retain(&self, slot_ids: &mut SlotIdsSet) {
        slot_ids.retain_if(|id| id.downcast_ref::<i32>().is_some_and(|i| i % 2 == 0));
    }
    fn are_compatible(&self, slot_id: &SlotKey, reusable_slot_id: &SlotKey) -> bool {
        let even = |k: &SlotKey| k.downcast_ref::<i32>().is_some_and(|i| i % 2 == 0);
        even(slot_id) && even(reusable_slot_id)
    }
}

fn a_reuse_policy_chooses_what_is_kept_and_for_what() {
    let ids = mutable_state_of(vec![0, 1]);
    let state = SubcomposeLayoutState::new(Rc::new(EvenSlots));
    let (i, st) = (ids.clone(), state.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let i = i.clone();
        SubcomposeLayout(
            state = Some(st.clone()),
            measure_policy = move |s, c: Constraints| {
                let mut placed = Vec::new();
                for id in i.get() {
                    for m in s.subcompose(id, move || Text(format!("slot {id}"), modifier = Modifier.test_tag("slot")))
                    {
                        placed.push(m.measure(c.copy_max_dimensions()));
                    }
                }
                s.layout(c.max_width, c.max_height, move |p| {
                    for (k, m) in placed.iter().enumerate() {
                        p.place(m, 0, 20 * k as i32);
                    }
                })
            },
        );
    }));
    let node_of = |rule: &KompozTestRule, text: &str| rule.on_node_with_text(text).layout_node();
    let zero = node_of(&rule, "slot 0");
    let one = node_of(&rule, "slot 1");
    ids.set(vec![]); // 0 kept (even), 1 disposed of
    rule.wait_for_idle();
    assert!(!one.is_attached());
    assert!(zero.is_attached() && zero.is_deactivated());
    ids.set(vec![3]); // odd: not compatible with 0's, a new slot
    rule.wait_for_idle();
    assert!(!Rc::ptr_eq(&node_of(&rule, "slot 3"), &zero));
    ids.set(vec![3, 2]); // even: 0's reused
    rule.wait_for_idle();
    assert!(Rc::ptr_eq(&node_of(&rule, "slot 2"), &zero) && !zero.is_deactivated());
}

/// A node found by its key below a precomposed slot's handle.
#[derive(PartialEq)]
struct Group(&'static str);

struct GroupNode(&'static str);

impl ModifierNodeElement for Group {
    type Node = GroupNode;
    fn create(&self) -> GroupNode {
        GroupNode(self.0)
    }
    fn update(&self, node: &mut GroupNode) {
        node.0 = self.0;
    }
}

impl ModifierNode for GroupNode {
    node_capabilities!(traversable);
}

impl TraversableNode for GroupNode {
    fn traverse_key(&self) -> &dyn std::any::Any {
        &"group"
    }
}

/// A slot precomposed before the measure asks for it: composed (its layouts there, not placed), premeasured, then
/// taken by the measure without composing again; one disposed of before it is asked for leaves.
fn a_precomposed_slot_is_taken_by_the_measure() {
    let shown = mutable_state_of(false);
    let runs = Rc::new(Cell::new(0));
    let state = SubcomposeLayoutState::default();
    let (sh, st, r) = (shown.clone(), state.clone(), runs.clone());
    let content: Rc<dyn Fn()> = Rc::new(move || {
        r.set(r.get() + 1);
        kompozable_block!({
            Box(modifier = Modifier.size_wh(30.dp(), 12.dp()).test_tag("önceden"));
        });
    });
    let c2 = content.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (sh, c2) = (sh.clone(), c2.clone());
        SubcomposeLayout(
            state = Some(st.clone()),
            measure_policy = move |s, c: Constraints| {
                let measured: Vec<Placeable> = if sh.get() {
                    s.subcompose_shared("önceden", c2.clone())
                        .iter()
                        .map(|m| m.measure(c.copy_max_dimensions()))
                        .collect()
                } else {
                    Vec::new()
                };
                s.layout(c.max_width, c.max_height, move |p| {
                    for m in &measured {
                        p.place(m, 0, 0);
                    }
                })
            },
        );
    }));
    let handle = state.precompose_shared("önceden", content.clone());
    assert_eq!(runs.get(), 1);
    assert_eq!(handle.placeables_count(), 1);
    assert!(rule.on_node_with_tag("önceden").exists()); // there, not placed
    assert!(!rule.on_node_with_tag("önceden").layout_node().is_placed());
    assert_eq!(handle.get_size(0), IntSize::Zero);
    handle.premeasure(0, Constraints::new(0, 400, 0, 800));
    let d = rule.density().density;
    assert_eq!(handle.get_size(0), IntSize::new((30.0 * d).round() as i32, (12.0 * d).round() as i32));
    shown.set(true);
    rule.wait_for_idle();
    assert_eq!(runs.get(), 1, "taken as it was composed");
    assert!(rule.on_node_with_tag("önceden").layout_node().is_placed());
    handle.dispose(); // used by the measure since: nothing to do
    assert!(rule.on_node_with_tag("önceden").exists());

    // Precomposed and let go before it is asked for: it leaves. Its traversable nodes are found through the handle.
    let other = state.precompose("başka", || {
        kompozable_block!({
            Box(
                modifier = Modifier.then_element(Group("dış")),
                content = || {
                    Box(modifier = Modifier.then_element(Group("iç")));
                    Text("başka");
                },
            );
        });
    });
    assert!(exists(&rule, "başka"));
    let mut found = Vec::new();
    other.traverse_descendants(&"group", &mut |cx| {
        cx.with::<GroupNode, _>(|g| found.push(g.0));
        TraversalDecision::ContinueTraversal
    });
    assert_eq!(found, ["dış", "iç"]);
    let mut found = Vec::new();
    other.traverse_descendants(&"group", &mut |cx| {
        cx.with::<GroupNode, _>(|g| found.push(g.0));
        TraversalDecision::SkipSubtreeAndContinueTraversal
    });
    assert_eq!(found, ["dış"]);
    other.dispose();
    rule.wait_for_idle();
    assert!(!exists(&rule, "başka"));
}

/// A slot precomposed a piece at a time: resumed (pausing at each restartable call) until complete, applied, then
/// taken by the measure; one the measure asks for before it is complete is completed by it.
fn a_paused_precomposition_is_composed_a_piece_at_a_time() {
    let shown = mutable_state_of(Vec::<i32>::new());
    let state = SubcomposeLayoutState::default();
    let content: Rc<dyn Fn()> = Rc::new(|| {
        kompozable_block!({
            Column(
                content = || {
                    Text("bir");
                    Text("iki");
                    Text("üç");
                },
            );
        });
    });
    let (sh, st, c2) = (shown.clone(), state.clone(), content.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (sh, c2) = (sh.clone(), c2.clone());
        SubcomposeLayout(
            state = Some(st.clone()),
            measure_policy = move |s, c: Constraints| {
                let mut measured = Vec::new();
                for slot in sh.get() {
                    for m in s.subcompose_shared(slot, c2.clone()) {
                        measured.push(m.measure(c.copy_max_dimensions()));
                    }
                }
                s.layout(c.max_width, c.max_height, move |p| {
                    for m in &measured {
                        p.place(m, 0, 0);
                    }
                })
            },
        );
    }));
    let _ = shown;
    let paused = state.create_paused_precomposition_shared(1, content.clone());
    assert!(!paused.is_complete());
    let mut resumes = 0;
    while !paused.resume(|| true) {
        resumes += 1;
        assert!(!exists(&rule, "bir"), "nothing joins the layout before it is applied");
    }
    assert!(resumes >= 3, "{resumes}");
    let handle = paused.apply();
    assert!(exists(&rule, "bir") && exists(&rule, "üç"));
    assert_eq!(handle.placeables_count(), 1);
    handle.dispose();
    rule.wait_for_idle();
    assert!(!exists(&rule, "bir"));

    // Asked for by the measure while it is paused: completed there.
    let paused = state.create_paused_precomposition_shared(2, content.clone());
    paused.resume(|| true);
    let measure_now = mutable_state_of(vec![2]);
    let (st, c3, mn) = (state.clone(), content.clone(), measure_now.clone());
    rule.set_content(kompozable_lambda!(move || {
        let (c3, mn) = (c3.clone(), mn.clone());
        SubcomposeLayout(
            state = Some(st.clone()),
            measure_policy = move |s, c: Constraints| {
                let mut measured = Vec::new();
                for slot in mn.get() {
                    for m in s.subcompose_shared(slot, c3.clone()) {
                        measured.push(m.measure(c.copy_max_dimensions()));
                    }
                }
                s.layout(c.max_width, c.max_height, move |p| {
                    for m in &measured {
                        p.place(m, 0, 0);
                    }
                })
            },
        );
    }));
    assert!(exists(&rule, "üç"));
}

/// Asking for a slot twice in one measure panics, as Compose's does.
fn a_slot_asked_for_twice_panics() {
    let message = panic_message(|| {
        let _rule = KompozTestRule::new(kompozable_lambda!(|| {
            SubcomposeLayout(
                measure_policy = |s, c: Constraints| {
                    s.subcompose("aynı", || Text("a"));
                    s.subcompose("aynı", || Text("b"));
                    s.layout(c.min_width, c.min_height, |_| {})
                },
            );
        }));
    });
    assert!(message.contains("Key \"aynı\" was already used"), "{message}");
}

tests!(
    box_with_constraints_and_on_size_changed,
    subcompose_layout_composes_a_slot_as_it_measures,
    box_with_constraints_composes_with_its_constraints_the_first_time,
    custom_layouts_take_compose_forms,
    subcompose_layout_panics_when_asked_for_an_intrinsic_size,
    subcompose_layout_composes_slots_from_what_it_measured,
    slots_see_the_locals_where_the_layout_is,
    a_slot_kept_for_reuse_is_composed_into_by_the_next,
    a_reuse_policy_chooses_what_is_kept_and_for_what,
    a_precomposed_slot_is_taken_by_the_measure,
    a_paused_precomposition_is_composed_a_piece_at_a_time,
    a_slot_asked_for_twice_panics,
);

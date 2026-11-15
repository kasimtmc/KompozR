//! The UI core: layout nodes measured and placed, the modifier node chain, invalidation and read observation,
//! intrinsics, alignment lines, right to left, coordinates, semantics, the host.

#![allow(non_snake_case)]

mod common;

use common::*;
use kompoz_coroutines::{Job, delay_ms};
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::layout::LayoutModifierNode;
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---- Helpers ---------------------------------------------------------------------------------------------------

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static MEASURES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log(s: impl Into<String>) {
    LOG.with(|l| l.borrow_mut().push(s.into()));
}

fn take_log() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

fn measured(name: &str) {
    MEASURES.with(|m| m.borrow_mut().push(name.to_string()));
}

fn take_measures() -> Vec<String> {
    MEASURES.with(|m| std::mem::take(&mut *m.borrow_mut()))
}

/// A leaf `width` by `height`, its measurements noted under `name`.
#[kompozable]
fn Leaf(name: &'static str, width: i32, height: i32, #[default(Modifier)] modifier: Modifier) {
    Layout(
        modifier = modifier,
        measure_policy = measure_policy(move |s, _, c| {
            measured(name);
            s.layout(c.constrain_width(width), c.constrain_height(height), |_| {})
        }),
    );
}

/// Its children one under the other, from the start.
#[kompozable]
fn Stack(#[default(Modifier)] modifier: Modifier, content: impl Fn()) {
    Layout(
        content = content,
        modifier = modifier,
        measure_policy = measure_policy(|s, ms, c| {
            let loose = Constraints::new(0, c.max_width, 0, c.max_height);
            let placeables: Vec<Placeable> = ms.iter().map(|m| m.measure(loose)).collect();
            let width = placeables.iter().map(|p| p.width()).max().unwrap_or(0);
            let height = placeables.iter().map(|p| p.height()).sum();
            s.layout(c.constrain_width(width), c.constrain_height(height), move |p| {
                let mut y = 0;
                for placeable in &placeables {
                    p.place_relative(placeable, 0, y);
                    y += placeable.height();
                }
            })
        }),
    );
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

// ---- Constraints -----------------------------------------------------------------------------------------------

fn constraints_as_compose() {
    let c = Constraints::new(10, 100, 20, Constraints::Infinity);
    assert!(c.has_bounded_width() && !c.has_bounded_height());
    assert!(!c.has_fixed_width() && !c.is_zero());
    assert_eq!(Constraints::fixed(5, 6), Constraints::new(5, 5, 6, 6));
    assert!(Constraints::fixed(5, 6).has_fixed_width() && Constraints::fixed(5, 6).has_fixed_height());
    assert_eq!(Constraints::fixed_width(7), Constraints::new(7, 7, 0, Constraints::Infinity));
    assert_eq!(Constraints::fixed_height(7), Constraints::new(0, Constraints::Infinity, 7, 7));
    assert!(Constraints::fixed(0, 3).is_zero());
    assert_eq!(c.constrain_width(5), 10);
    assert_eq!(c.constrain_height(1000), 1000);
    assert_eq!(c.constrain_size(IntSize::new(200, 5)), IntSize::new(100, 20));
    assert_eq!(c.constrain(Constraints::new(0, 50, 30, 40)), Constraints::new(10, 50, 30, 40));
    assert_eq!(c.offset(-20, 5), Constraints::new(0, 80, 25, Constraints::Infinity));
    assert_eq!(c.offset(5, -30), Constraints::new(15, 105, 0, Constraints::Infinity));
    assert_eq!(c.copy(), c);
    assert_eq!(Constraints::default(), Constraints::new(0, Constraints::Infinity, 0, Constraints::Infinity));
    assert!(c.is_satisfied_by(IntSize::new(50, 30)) && !c.is_satisfied_by(IntSize::new(5, 30)));
    assert_eq!(format!("{c:?}"), "Constraints(minWidth = 10, maxWidth = 100, minHeight = 20, maxHeight = Infinity)");
    let caught = std::panic::catch_unwind(|| Constraints::new(10, 5, 0, 0));
    assert!(caught.is_err());
    // Named, in a body.
    let seen = Rc::new(Cell::new(Constraints::default()));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        s.set(Constraints(max_width = 30, min_height = 4).copy(min_width = 2).offset(0, 6));
    }));
    assert_eq!(seen.get(), Constraints::new(2, 30, 10, Constraints::Infinity));
}

// ---- Layout ----------------------------------------------------------------------------------------------------

fn a_custom_layout_measures_and_places_its_children() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Stack(modifier = Modifier.test_tag("stack"), || {
            Leaf("a", 30, 10, modifier = Modifier.test_tag("a"));
            Leaf("b", 50, 20, modifier = Modifier.test_tag("b"));
        });
    }));
    assert_eq!(bounds(&rule, "stack"), rect(0.0, 0.0, 50.0, 30.0));
    assert_eq!(bounds(&rule, "a"), rect(0.0, 0.0, 30.0, 10.0));
    assert_eq!(bounds(&rule, "b"), rect(0.0, 10.0, 50.0, 20.0));
}

fn a_layout_measures_again_only_what_changed() {
    take_measures();
    let width = mutable_state_of(10);
    let w = width.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Stack(|| {
            Leaf("a", 10, 10);
            Leaf("b", w.get(), 10, modifier = Modifier.test_tag("b"));
            Leaf("c", 10, 10);
        });
    }));
    rule.set_layout_check(false);
    take_measures();
    width.set(40);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "b").width(), 40.0);
    // b measured again (its policy is a new closure); the stack's other children as they were.
    assert_eq!(take_measures(), ["b"]);
}

fn state_read_while_placing_places_again_without_measuring_or_composing() {
    take_measures();
    let shift = mutable_state_of(0);
    let s = shift.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        let moved = Modifier.test_tag("moved").layout(move |scope, measurable, c| {
            let placeable = measurable.measure(c);
            let s = s.clone();
            scope.layout(placeable.width(), placeable.height(), move |p| p.place(&placeable, s.get(), 0))
        });
        Leaf("moved", 20, 20, modifier = moved);
    }));
    rule.set_layout_check(false);
    take_measures();
    let compositions = rule.host().stats().recompositions;
    shift.set(15);
    rule.wait_for_idle();
    assert_eq!(rule.on_node_with_tag("moved").layout_node().inner_coordinates().position_in_root().x, 15.0);
    assert_eq!(rule.host().stats().recompositions, compositions);
    assert!(take_measures().is_empty());
}

fn state_read_while_measuring_measures_again_without_composing() {
    take_measures();
    let size = mutable_state_of(10);
    let s = size.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Layout(
            modifier = Modifier.test_tag("sized"),
            measure_policy = measure_policy(move |scope, _, _| {
                measured("sized");
                let v = s.get();
                scope.layout(v, v, |_| {})
            }),
        );
    }));
    rule.set_layout_check(false);
    take_measures();
    let compositions = rule.host().stats().recompositions;
    size.set(25);
    rule.wait_for_idle();
    assert_eq!(bounds(&rule, "sized"), rect(0.0, 0.0, 25.0, 25.0));
    assert_eq!(take_measures(), ["sized"]);
    assert_eq!(rule.host().stats().recompositions, compositions);
}

fn the_layout_check_finds_nothing_amiss() {
    let n = mutable_state_of(1);
    let m = n.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Stack(|| {
            for i in 0..m.get() {
                key(&i, || Leaf("item", 10 * (i + 1), 10));
            }
        });
    }));
    n.set(3);
    rule.wait_for_idle();
    n.set(0);
    rule.wait_for_idle();
}

// ---- The modifier node chain -----------------------------------------------------------------------------------

thread_local! {
    static NEXT_NODE: Cell<u32> = const { Cell::new(0) };
    static JOBS: RefCell<Vec<Job>> = const { RefCell::new(Vec::new()) };
}

/// An element whose node logs its life: `name` (the element's type is the same whatever the name: reused and
/// updated), its node numbered as made.
#[derive(PartialEq)]
struct Logged {
    name: &'static str,
}

struct LoggedNode {
    name: &'static str,
    number: u32,
}

impl ModifierNodeElement for Logged {
    type Node = LoggedNode;
    fn create(&self) -> LoggedNode {
        let number = NEXT_NODE.with(|n| {
            n.set(n.get() + 1);
            n.get()
        });
        log(format!("create {} #{number}", self.name));
        LoggedNode { name: self.name, number }
    }
    fn update(&self, node: &mut LoggedNode) {
        log(format!("update #{} {} -> {}", node.number, node.name, self.name));
        node.name = self.name;
    }
}

impl ModifierNode for LoggedNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        log(format!("attach {} #{}", self.name, self.number));
        let name = self.name;
        let job = cx.coroutine_scope().launch(async move {
            loop {
                delay_ms(3000).await;
                log(format!("tick {name}"));
            }
        });
        JOBS.with(|j| j.borrow_mut().push(job));
    }
    fn on_detach(&mut self, _: &NodeCx) {
        log(format!("detach {} #{}", self.name, self.number));
    }
    fn on_reset(&mut self) {
        log(format!("reset {} #{}", self.name, self.number));
    }
}

/// Another type of element (its place can't take a Logged's node).
#[derive(PartialEq)]
struct Other;

struct OtherNode;

impl ModifierNodeElement for Other {
    type Node = OtherNode;
    fn create(&self) -> OtherNode {
        log("create other");
        OtherNode
    }
    fn update(&self, _: &mut OtherNode) {}
}

impl ModifierNode for OtherNode {
    fn on_attach(&mut self, _: &NodeCx) {
        log("attach other");
    }
    fn on_detach(&mut self, _: &NodeCx) {
        log("detach other");
    }
}

fn chain_of(names: &[&'static str]) -> Modifier {
    names
        .iter()
        .fold(Modifier, |m, n| if *n == "other" { m.then_element(Other) } else { m.then_element(Logged { name: n }) })
}

fn the_node_chain_reuses_updates_and_diffs_as_compose() {
    take_log();
    NEXT_NODE.with(|n| n.set(0));
    JOBS.with(|j| j.borrow_mut().clear());
    let names = mutable_state_of(vec!["a", "b"]);
    let n = names.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Leaf("chained", 10, 10, modifier = chain_of(&n.get()));
    }));
    // Made head first, attached head first once the layout is.
    assert_eq!(take_log(), ["create a #1", "create b #2", "attach a #1", "attach b #2"]);
    // The same elements: nothing.
    names.set(vec!["a", "b"]);
    rule.wait_for_idle();
    assert!(take_log().is_empty());
    // The same type at a place: kept and updated.
    names.set(vec!["a", "c"]);
    rule.wait_for_idle();
    assert_eq!(take_log(), ["update #2 b -> c"]);
    // One inserted between: only it made; the others matched by the diff and kept (equal: not updated).
    names.set(vec!["a", "other", "c"]);
    rule.wait_for_idle();
    assert_eq!(take_log(), ["create other", "attach other"]);
    // The first removed: the rest kept, the node of c now a's (updated).
    names.set(vec!["other", "a"]);
    rule.wait_for_idle();
    assert_eq!(take_log(), ["detach a #1", "update #2 c -> a"]);
    // A kept node's coroutine runs on (it was launched as "b"); the removed one's was cancelled.
    rule.main_clock().advance_time_by(5000);
    let ticks = take_log();
    assert!(!ticks.is_empty() && ticks.iter().all(|t| t == "tick b"), "{ticks:?}");
    let jobs = JOBS.with(|j| j.borrow().clone());
    assert!(!jobs[0].is_active() && jobs[1].is_active());
    // All removed: detached head first, their coroutines cancelled.
    names.set(vec![]);
    rule.wait_for_idle();
    assert_eq!(take_log(), ["detach other", "detach a #2"]);
    assert!(jobs.iter().all(|j| !j.is_active()));
}

fn attach_and_detach_follow_composes_order() {
    take_log();
    NEXT_NODE.with(|n| n.set(0));
    let names = mutable_state_of(vec!["a", "b", "c"]);
    let n = names.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Leaf("chained", 10, 10, modifier = chain_of(&n.get()));
    }));
    take_log();
    // Replaced in the middle: removed, then inserted, at its place in the diff.
    names.set(vec!["a", "other", "c"]);
    rule.wait_for_idle();
    assert_eq!(take_log(), ["detach b #2", "create other", "attach other"]);
    // The layout node leaves: its nodes detached tail first.
    let shown = mutable_state_of(true);
    let s = shown.clone();
    let rule2 = KompozTestRule::new(kompozable_lambda!(move || {
        if s.get() {
            Leaf("chained", 10, 10, modifier = chain_of(&["x", "y"]));
        }
    }));
    take_log();
    shown.set(false);
    rule2.wait_for_idle();
    assert_eq!(take_log(), ["detach y #5", "detach x #4"]);
    drop(rule);
}

fn a_reused_layout_resets_and_attaches_its_nodes_again() {
    take_log();
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Leaf("reused", 10, 10, modifier = Modifier.test_tag("reused").then(chain_of(&["r"])));
    }));
    take_log();
    rule.on_node_with_tag("reused").layout_node().on_reuse();
    let l = take_log();
    assert!(l[0].starts_with("reset r") && l[1].starts_with("detach r") && l[2].starts_with("attach r"), "{l:?}");
}

/// A node that delegates to a layout node and a semantics one when attached.
#[derive(PartialEq)]
struct Delegating;

struct DelegatingNode {
    padding: Option<kompoz_ui::modifier::DelegatedNode<Pad>>,
}

struct Pad(i32);

impl ModifierNode for Pad {
    kompoz_ui::node_capabilities!(layout);
    fn on_attach(&mut self, _: &NodeCx) {
        log("pad attached");
    }
}

impl LayoutModifierNode for Pad {
    fn measure(&mut self, scope: &MeasureScope, measurable: &dyn Measurable, c: Constraints) -> MeasureResult {
        let p = self.0;
        let placeable = measurable.measure(c.offset(-2 * p, -2 * p));
        scope.layout(placeable.width() + 2 * p, placeable.height() + 2 * p, move |s| s.place(&placeable, p, p))
    }
}

impl ModifierNodeElement for Delegating {
    type Node = DelegatingNode;
    fn create(&self) -> DelegatingNode {
        DelegatingNode { padding: None }
    }
    fn update(&self, _: &mut DelegatingNode) {}
}

impl ModifierNode for DelegatingNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        log("delegating attached");
        self.padding = Some(cx.delegate(Pad(5)));
    }
}

fn a_delegating_node_has_its_delegates_kinds_and_lifecycle() {
    take_log();
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Leaf("inner", 10, 10, modifier = Modifier.test_tag("outer").then_element(Delegating));
    }));
    assert_eq!(take_log(), ["delegating attached", "pad attached"]);
    // The delegate is a layout node: the content inside its padding.
    let node = rule.on_node_with_tag("outer").layout_node();
    assert_eq!(node.size(), IntSize::new(20, 20));
    assert_eq!(node.inner_coordinates().position_in_root(), Offset::new(5.0, 5.0));
}

// ---- Intrinsics ------------------------------------------------------------------------------------------------

fn intrinsics_come_from_the_policy_and_the_layout_modifiers() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Layout(
            content = || {
                // A text: min = its widest word, max = all of it.
                Text("ab cdef g", font_size = 20.sp());
                // A modifier's default intrinsics measure what it wraps answering with its own: 10 more.
                Text(
                    "abcd",
                    font_size = 20.sp(),
                    modifier = Modifier.layout(|scope, m, c| {
                        let p = m.measure(c.offset(-10, 0));
                        scope.layout(p.width() + 10, p.height(), move |pl| pl.place(&p, 10, 0))
                    }),
                );
                // A custom policy's default intrinsics: measured with its children answering with theirs.
                Stack(|| {
                    Text("xy", font_size = 20.sp());
                    Text("wxyz", font_size = 20.sp());
                });
            },
            measure_policy = measure_policy(move |scope, ms, c| {
                let mut out = s.borrow_mut();
                for m in ms {
                    out.push((m.min_intrinsic_width(100), m.max_intrinsic_width(100), m.min_intrinsic_height(100)));
                }
                let placeables: Vec<Placeable> = ms.iter().map(|m| m.measure(c)).collect();
                scope.layout(10, 10, move |p| {
                    for pl in &placeables {
                        p.place(pl, 0, 0);
                    }
                })
            }),
        );
    }));
    let seen = seen.borrow();
    assert_eq!(seen[0], (40, 90, 25));
    assert_eq!(seen[1], (50, 50, 25));
    assert_eq!(seen[2], (40, 40, 50));
}

// ---- Alignment lines -------------------------------------------------------------------------------------------

fn alignment_lines_pass_up_moved_and_merged() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Layout(
            content = || {
                // Two texts in a stack: its first baseline the top one's, its last the bottom one's.
                Stack(modifier = Modifier.test_tag("stack"), || {
                    Text("small", font_size = 10.sp());
                    Text("big", font_size = 40.sp());
                });
                // A layout modifier moving its content moves the lines.
                Text(
                    "moved",
                    font_size = 10.sp(),
                    modifier = Modifier.layout(|scope, m, c| {
                        let p = m.measure(c);
                        scope.layout(p.width(), p.height() + 7, move |pl| pl.place(&p, 0, 7))
                    }),
                );
                // No text: no line.
                Leaf("empty", 5, 5);
            },
            measure_policy = measure_policy(move |scope, ms, c| {
                let placeables: Vec<Placeable> = ms.iter().map(|m| m.measure(c)).collect();
                let mut out = s.borrow_mut();
                for p in &placeables {
                    out.push((p.get(FirstBaseline), p.get(LastBaseline)));
                }
                scope.layout(200, 200, move |pl| {
                    let mut y = 0;
                    for p in &placeables {
                        pl.place(p, 0, y);
                        y += p.height();
                    }
                })
            }),
        );
    }));
    let seen = seen.borrow();
    // small: 12.5 high (13), baseline 8; big at 13: baseline 13 + 32.
    assert_eq!(seen[0], (8, 13 + 32));
    assert_eq!(seen[1], (15, 15));
    assert_eq!(seen[2], (AlignmentLine::Unspecified, AlignmentLine::Unspecified));
    // As the coordinates tell them.
    let stack = rule.on_node_with_tag("stack").layout_node();
    assert_eq!(stack.coordinates().get(FirstBaseline), 8);
}

// ---- Right to left ---------------------------------------------------------------------------------------------

fn place_relative_mirrors_right_to_left() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        KompozitionLocalProvider(&[kompoz_ui::platform::LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
            Layout(
                content = || {
                    Leaf("relative", 10, 10, modifier = Modifier.test_tag("relative"));
                    Leaf("absolute", 10, 10, modifier = Modifier.test_tag("absolute"));
                },
                measure_policy = measure_policy(|scope, ms, c| {
                    let a = ms[0].measure(c);
                    let b = ms[1].measure(c);
                    scope.layout(100, 50, move |p| {
                        p.place_relative(&a, 5, 0);
                        p.place(&b, 5, 20);
                    })
                }),
            );
        });
    }));
    assert_eq!(bounds(&rule, "relative"), rect(85.0, 0.0, 10.0, 10.0));
    assert_eq!(bounds(&rule, "absolute"), rect(5.0, 20.0, 10.0, 10.0));
    assert_eq!(rule.on_node_with_tag("relative").layout_node().layout_direction(), LayoutDirection::Rtl);
}

thread_local! {
    static RESOLVERS: RefCell<Vec<Rc<dyn kompoz_text::FontFamilyResolver>>> = const { RefCell::new(Vec::new()) };
}

fn the_host_gives_text_its_font_family_resolver() {
    use kompoz_text::{FakeFontFamilyResolver, FontFamily, FontFamilyResolver, FontStyle, FontSynthesis, FontWeight};
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        let resolver = kompoz_ui::platform::LocalFontFamilyResolver.current();
        RESOLVERS.with(|r| r.borrow_mut().push(resolver));
    }));
    let last = || RESOLVERS.with(|r| r.borrow().last().cloned().expect("read"));
    // The host's: one over the fake backend until another is installed.
    assert!(Rc::ptr_eq(&last(), &rule.host().font_family_resolver()));
    let typeface =
        last().resolve(Some(&FontFamily::Serif), FontWeight::Bold, FontStyle::Italic, FontSynthesis::All).get();
    assert_eq!((typeface.weight, typeface.style), (FontWeight::Bold, FontStyle::Italic));
    assert_eq!(typeface.font, kompoz_text::FakeBackend::font_id());
    // Another installed: what reads it composes again and gets it.
    let other: Rc<dyn FontFamilyResolver> = Rc::new(FakeFontFamilyResolver::new());
    rule.host().set_font_family_resolver(other.clone());
    rule.wait_for_idle();
    assert!(Rc::ptr_eq(&last(), &other) && Rc::ptr_eq(&rule.host().font_family_resolver(), &other));
    // Everything composed again keeps it.
    let reads = RESOLVERS.with(|r| r.borrow().len());
    rule.host().invalidate();
    rule.wait_for_idle();
    assert!(RESOLVERS.with(|r| r.borrow().len()) > reads && Rc::ptr_eq(&last(), &other));
}

tests!(
    constraints_as_compose,
    the_node_chain_reuses_updates_and_diffs_as_compose,
    attach_and_detach_follow_composes_order,
    a_reused_layout_resets_and_attaches_its_nodes_again,
    a_delegating_node_has_its_delegates_kinds_and_lifecycle,
    intrinsics_come_from_the_policy_and_the_layout_modifiers,
    alignment_lines_pass_up_moved_and_merged,
    place_relative_mirrors_right_to_left,
    a_custom_layout_measures_and_places_its_children,
    a_layout_measures_again_only_what_changed,
    state_read_while_placing_places_again_without_measuring_or_composing,
    state_read_while_measuring_measures_again_without_composing,
    the_layout_check_finds_nothing_amiss,
    the_host_gives_text_its_font_family_resolver,
);

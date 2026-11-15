//! The tree as the host keeps it: coordinates and layers, placement and position callbacks, z order, parent data
//! and layout ids, semantics, drawing and its observed reads, node traversal and observation, the host's
//! environment and window modifiers.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode};
use kompoz_ui::graphics_context::{ClipOp, CornerRadii, GraphicsContext, LayerTransform, NodeRecording, TextDraw};
use kompoz_ui::layout::LayoutIdKey;
use kompoz_ui::modifier::{ObserverModifierNode, TraversableNode, TraversalDecision};
use kompoz_ui::platform::{
    FrameRateCategory, LocalLayoutDirection, LocalWindowInfo, SystemInsets, is_system_in_dark_theme,
};
use kompoz_ui::semantics::SemanticsProperties;
use kompoz_ui::*;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ---- Helpers ---------------------------------------------------------------------------------------------------

#[kompozable]
fn Leaf(width: i32, height: i32, #[default(Modifier)] modifier: Modifier) {
    Layout(
        modifier = modifier,
        measure_policy =
            measure_policy(move |s, _, c| s.layout(c.constrain_width(width), c.constrain_height(height), |_| {})),
    );
}

/// Its children one under the other.
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

/// What it wraps `all` px in from each side.
fn padded(m: Modifier, all: i32) -> Modifier {
    m.layout(move |scope, measurable, c| {
        let p = measurable.measure(c.offset(-2 * all, -2 * all));
        scope.layout(p.width() + 2 * all, p.height() + 2 * all, move |pl| pl.place_relative(&p, all, all))
    })
}

/// What it wraps, `x` px from the left, through a layer `layer` sets up.
fn layered(m: Modifier, x: i32, layer: impl Fn(&mut layout::GraphicsLayerScope) + 'static) -> Modifier {
    let layer: layout::LayerBlock = Rc::new(layer);
    m.layout(move |scope, measurable, c| {
        let p = measurable.measure(c);
        let layer = layer.clone();
        scope.layout(p.width() + x, p.height(), move |pl| {
            pl.place_with_layer_block(&p, IntOffset::new(x, 0), layer.clone(), 0.0)
        })
    })
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x, y, x + w, y + h)
}

fn near(a: Offset, b: Offset) -> bool {
    (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01
}

/// Records what a test looks at: each text and where it lands (translations applied), the node hooks, transforms.
#[derive(Default)]
struct Recorder {
    texts: Vec<(String, Offset)>,
    translation: Offset,
    stack: Vec<Offset>,
    nodes: Vec<u64>,
    transforms: usize,
    layers: usize,
}

impl GraphicsContext for Recorder {
    fn fill_round_rect(&mut self, _: Rect, _: CornerRadii, _: Color) {}
    fn stroke_round_rect(&mut self, _: Rect, _: CornerRadii, _: f32, _: Color) {}
    fn draw_line(&mut self, _: Offset, _: Offset, _: f32, _: Color) {}
    fn draw_text(&mut self, text: &TextDraw, _: Color) {
        self.texts.push((text.text.to_string(), text.baseline + self.translation));
    }
    fn draw_arc(&mut self, _: Rect, _: f32, _: f32, _: f32, _: Color, _: bool) {}
    fn draw_image(&mut self, _: &ImageBitmap, _: Rect, _: Rect, _: f32, _: FilterQuality) {}
    fn push_layer(&mut self, _: Rect, _: CornerRadii, _: f32, _: LayerTransform) {
        self.layers += 1;
    }
    fn pop_layer(&mut self) {}
    fn push_clip(&mut self, _: Rect) {}
    fn push_clip_round_rect(&mut self, _: Rect, _: CornerRadii, _: ClipOp) {}
    fn push_clip_path(&mut self, _: &Path, _: Offset, _: ClipOp) {}
    fn pop_clip(&mut self) {}
    fn push_translation(&mut self, by: Offset) {
        self.stack.push(self.translation);
        self.translation = self.translation + by;
    }
    fn pop_translation(&mut self) {
        self.translation = self.stack.pop().unwrap();
    }
    fn push_transform(&mut self, _: &Matrix) {
        self.transforms += 1;
    }
    fn pop_transform(&mut self) {}
    fn push_color_filter(&mut self, _: &ColorFilter) {}
    fn pop_color_filter(&mut self) {}
    fn draw_vertices(&mut self, _: &Vertices, _: Offset, _: BlendMode, _: &Brush, _: Rect, _: f32) {}
    fn draw_path(&mut self, _: &Path, _: Offset, _: &Brush, _: Rect, _: &PathStroke) {}
    fn begin_node(&mut self, node: u64) {
        self.nodes.push(node);
    }
    fn record_node(&mut self, _: u64, _: Offset, _: bool) -> NodeRecording {
        NodeRecording::Other
    }
}

fn draw(rule: &KompozTestRule) -> Recorder {
    let mut r = Recorder::default();
    rule.host().draw(&mut r);
    r
}

// ---- Coordinates -----------------------------------------------------------------------------------------------

fn coordinates_tell_where_layouts_are_and_map_between_them() {
    let inner = Rc::new(RefCell::new(None::<LayoutCoordinates>));
    let other = Rc::new(RefCell::new(None::<LayoutCoordinates>));
    let (i, o) = (inner.clone(), other.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (i, o) = (i.clone(), o.clone());
        Stack(modifier = padded(Modifier, 10), || {
            Leaf(100, 50);
            Leaf(
                40,
                40,
                modifier = padded(Modifier, 5).on_globally_positioned(move |c| *i.borrow_mut() = Some(c.clone())),
            );
            Leaf(30, 30, modifier = Modifier.on_globally_positioned(move |c| *o.borrow_mut() = Some(c.clone())));
        });
    }));
    let inner = inner.borrow().clone().unwrap();
    let other = other.borrow().clone().unwrap();
    // inner: inside its 5 px padding, under the 50 px leaf, inside the stack's 10 px: at (15, 65).
    assert_eq!(inner.position_in_root(), Offset::new(15.0, 65.0));
    assert_eq!(inner.size(), IntSize::new(40, 40));
    // Its layout node's parent is the stack (its inner coordinator, past the padding): at (10, 10) in the root.
    let parent = inner.parent_layout_coordinates().unwrap();
    assert_eq!(parent.position_in_root(), Offset::new(10.0, 10.0));
    assert_eq!(inner.position_in_parent(), Offset::new(5.0, 55.0));
    assert_eq!(inner.bounds_in_parent(), rect(5.0, 55.0, 40.0, 40.0));
    // The coordinates wrapping inner's: the padding's, at (0, 50) in the stack.
    assert_eq!(inner.parent_coordinates().unwrap().size(), IntSize::new(50, 50));
    // other is at (10, 110): a point of inner's, and inner's bounds, in other's coordinates.
    assert_eq!(other.local_position_of(&inner, Offset::new(5.0, 5.0)), Offset::new(10.0, -40.0));
    assert_eq!(other.local_bounding_box_of(&inner, true), rect(5.0, -45.0, 40.0, 40.0));
    assert_eq!(inner.find_root_coordinates().size(), IntSize::new(120, 150));
    assert!(inner.is_attached());
    // The window: the root moved by where the host's window puts it.
    rule.host().set_window_offset(Offset::new(100.0, 200.0));
    rule.wait_for_idle();
    assert_eq!(inner.position_in_window(), Offset::new(115.0, 265.0));
    assert_eq!(inner.window_to_local(Offset::new(115.0, 265.0)), Offset::Zero);
}

fn a_layer_moves_and_clips_what_coordinates_tell() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Stack(|| {
            // Scaled twice about its top left: a point of its child's at (5, 5) lands at (10, 10) past its 20 px.
            Leaf(
                10,
                10,
                modifier = layered(Modifier, 20, |l| {
                    l.scale_x = 2.0;
                    l.scale_y = 2.0;
                    l.transform_origin = layout::TransformOrigin::new(0.0, 0.0);
                })
                .test_tag("scaled"),
            );
            // Clipped: a child bigger than it is cut in bounds_in_root, not in the unclipped bounds.
            Stack(
                modifier = layered(Modifier, 0, |l| l.clip = true).layout(|scope, m, c| {
                    let p = m.measure(c);
                    scope.layout(20, 20, move |pl| pl.place(&p, 0, 0))
                }),
                || Leaf(50, 50, modifier = Modifier.test_tag("big")),
            );
        });
    }));
    let scaled = rule.on_node_with_tag("scaled").layout_node().inner_coordinates();
    assert!(near(scaled.local_to_root(Offset::new(5.0, 5.0)), Offset::new(30.0, 10.0)));
    assert_eq!(bounds(&rule, "scaled"), rect(20.0, 0.0, 20.0, 20.0));
    let mut m = Matrix::new();
    scaled.transform_to_root(&mut m);
    assert!(near(m.map(Offset::new(5.0, 5.0)), Offset::new(30.0, 10.0)));
    assert_eq!(rule.on_node_with_tag("big").get_bounds_in_root(), rect(0.0, 10.0, 20.0, 20.0));
    assert_eq!(bounds(&rule, "big"), rect(0.0, 10.0, 50.0, 50.0));
    // Drawn through its matrix.
    assert_eq!(draw(&rule).transforms, 1);
}

// ---- Callbacks -------------------------------------------------------------------------------------------------

fn size_placement_and_position_callbacks_are_told_as_compose() {
    let sizes = Rc::new(RefCell::new(Vec::new()));
    let placed = Rc::new(Cell::new(0));
    let positions = Rc::new(RefCell::new(Vec::new()));
    let width = mutable_state_of(10);
    let top = mutable_state_of(0);
    let (s, p, g, w, t) = (sizes.clone(), placed.clone(), positions.clone(), width.clone(), top.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (s, p, g) = (s.clone(), p.clone(), g.clone());
        let top_now = t.get();
        Stack(|| {
            Leaf(10, top_now);
            Leaf(
                w.get(),
                10,
                modifier = Modifier
                    .on_size_changed(move |size| s.borrow_mut().push(size))
                    .on_placed(move |_| p.set(p.get() + 1))
                    .on_globally_positioned(move |c| g.borrow_mut().push(c.position_in_root())),
            );
        });
    }));
    rule.set_layout_check(false);
    assert_eq!(*sizes.borrow(), [IntSize::new(10, 10)]);
    assert!(placed.get() >= 1);
    assert_eq!(positions.borrow().last().copied(), Some(Offset::Zero));
    // Measured again, the same size: not told.
    let told = sizes.borrow().len();
    width.set(10);
    top.set(0);
    rule.wait_for_idle();
    assert_eq!(sizes.borrow().len(), told);
    // Resized: told; moved by its sibling: told where.
    width.set(20);
    top.set(30);
    rule.wait_for_idle();
    assert_eq!(sizes.borrow().last().copied(), Some(IntSize::new(20, 10)));
    assert_eq!(positions.borrow().last().copied(), Some(Offset::new(0.0, 30.0)));
}

// ---- Z order ---------------------------------------------------------------------------------------------------

fn a_child_placed_higher_is_drawn_above() {
    // C++'s a_child_placed_higher_is_drawn_above: the z-index read while placing places again, nothing more.
    let front = mutable_state_of(false);
    let f = front.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let f = f.clone();
        Layout(
            content = || {
                Leaf(10, 10, modifier = Modifier.test_tag("a"));
                Leaf(10, 10, modifier = Modifier.test_tag("b"));
            },
            modifier = Modifier.test_tag("parent"),
            measure_policy = measure_policy(move |scope, ms, _| {
                let a = ms[0].measure(Constraints::fixed(10, 10));
                let b = ms[1].measure(Constraints::fixed(10, 10));
                let f = f.clone();
                scope.layout(10, 10, move |p| {
                    p.place_at(&a, IntOffset::Zero, if f.get() { 1.0 } else { 0.0 });
                    p.place(&b, 0, 0);
                })
            }),
        );
    }));
    let parent = rule.on_node_with_tag("parent").layout_node();
    let bottom = || parent.z_sorted_children()[0].id();
    assert_eq!(bottom(), rule.on_node_with_tag("a").layout_node().id());
    let compositions = rule.host().stats().recompositions;
    front.set(true);
    rule.wait_for_idle();
    assert_eq!(bottom(), rule.on_node_with_tag("b").layout_node().id());
    assert_eq!(rule.host().stats().recompositions, compositions);
    // Modifier.z_index does it too: the first drawn last.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Stack(|| {
            Text("first", modifier = Modifier.z_index(2.0));
            Text("second");
        });
    }));
    let texts: Vec<String> = draw(&rule).texts.into_iter().map(|t| t.0).collect();
    assert_eq!(texts, ["second", "first"]);
}

// ---- Parent data and layout ids --------------------------------------------------------------------------------

fn custom_layout_layout_modifier_and_layout_id() {
    // C++'s custom_layout_layout_modifier_and_layout_id.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Layout(
            content = || {
                Leaf(20, 20, modifier = Modifier.layout_id("last").test_tag("c"));
                Leaf(30, 30, modifier = Modifier.test_tag("a"));
                Leaf(40, 40, modifier = Modifier.test_tag("b"));
            },
            modifier = Modifier.test_tag("diag"),
            measure_policy = measure_policy(|scope, ms, c| {
                let is_last = |m: &&dyn Measurable| m.layout_id().is_some_and(|id: Rc<dyn LayoutIdKey>| id.is(&"last"));
                let ordered: Vec<&&dyn Measurable> =
                    ms.iter().filter(|m| !is_last(m)).chain(ms.iter().filter(|m| is_last(m))).collect();
                let loose = Constraints::new(0, c.max_width, 0, c.max_height);
                let placeables: Vec<Placeable> = ordered.iter().map(|m| m.measure(loose)).collect();
                let (w, h) = placeables.iter().fold((0, 0), |(x, y), p| (x + p.width(), y + p.height()));
                scope.layout(w, h, move |pl| {
                    let (mut x, mut y) = (0, 0);
                    for p in &placeables {
                        pl.place(p, x, y);
                        x += p.width();
                        y += p.height();
                    }
                })
            }),
        );
    }));
    assert_eq!(bounds(&rule, "diag"), rect(0.0, 0.0, 90.0, 90.0));
    assert_eq!(bounds(&rule, "a"), rect(0.0, 0.0, 30.0, 30.0));
    assert_eq!(bounds(&rule, "b"), rect(30.0, 30.0, 40.0, 40.0));
    assert_eq!(bounds(&rule, "c"), rect(70.0, 70.0, 20.0, 20.0));
    // Modifier.layout: a node 8 px taller than its content, the content at the bottom.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Leaf(
            50,
            50,
            modifier = Modifier.test_tag("box").layout(|scope, m, c| {
                let p = m.measure(c);
                scope.layout(p.width(), p.height() + 8, move |pl| pl.place(&p, 0, 8))
            }),
        );
    }));
    let node = rule.on_node_with_tag("box").layout_node();
    assert_eq!(bounds(&rule, "box"), rect(0.0, 0.0, 50.0, 58.0));
    let inner = node.inner_coordinates();
    assert_eq!(inner.find_root_coordinates().local_bounding_box_of(&inner, false), rect(0.0, 8.0, 50.0, 50.0));
}

fn measuring_a_child_twice_needs_a_multi_measure_layout() {
    let twice = |multi: bool| {
        std::panic::catch_unwind(move || {
            let policy = measure_policy(|scope, ms, c| {
                let _first = ms[0].measure(c);
                let second = ms[0].measure(c);
                scope.layout(second.width(), second.height(), move |p| p.place(&second, 0, 0))
            });
            let _rule = KompozTestRule::new(kompozable_lambda!(move || {
                if multi {
                    MultiMeasureLayout(content = || Leaf(10, 10), measure_policy = policy);
                } else {
                    Layout(content = || Leaf(10, 10), measure_policy = policy);
                }
            }));
        })
        .is_ok()
    };
    assert!(twice(true));
    assert!(!twice(false));
}

// ---- Semantics -------------------------------------------------------------------------------------------------

fn the_semantics_tree_merges_as_compose() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Stack(modifier = Modifier.test_tag("button").semantics(merge_descendants = true, |_| {}), || {
            Text("Tamam");
            Text("İptal", modifier = Modifier.test_tag("inner"));
            // Merging its own: kept apart.
            Stack(
                modifier = Modifier
                    .test_tag("own")
                    .semantics(merge_descendants = true, |c| c.set_content_description("kendi")),
                || {
                    Text("ayrı");
                },
            );
        });
        Stack(modifier = Modifier.test_tag("cleared").clear_and_set_semantics(|c| c.set_text("yerine")), || {
            Text("gizli")
        });
    }));
    // Merged: the button says both texts (its texts aren't nodes of their own: what says "Tamam" is the button).
    let button = rule.on_node_with_tag("button").fetch_semantics_node();
    assert_eq!(button.config().get(SemanticsProperties::Text), Some(vec!["Tamam".to_string(), "İptal".to_string()]));
    let tag = |n: kompoz_ui::semantics::SemanticsNode| n.config().get(SemanticsProperties::TestTag);
    assert_eq!(tag(rule.on_node_with_text("Tamam").fetch_semantics_node()), Some("button".to_string()));
    assert_eq!(button.children().len(), 1);
    // The test tag merges as Compose's: the parent's kept.
    assert_eq!(button.config().get(SemanticsProperties::TestTag), Some("button".to_string()));
    // A node merging its own descendants is a node of its own.
    assert_eq!(tag(rule.on_node_with_text("ayrı").fetch_semantics_node()), Some("own".to_string()));
    // Unmerged: every node.
    rule.on_node_with_tag_unmerged("inner").assert_exists();
    assert_eq!(rule.all_nodes(true).len(), 1 + 1 + 2 + 1 + 1 + 1 + 1);
    // Cleared: what is below it gone, its own text instead.
    rule.on_node_with_text("yerine").assert_exists();
    rule.on_node_with_text("gizli").assert_does_not_exist();
    // The outer test tag wins over an inner one (applied tail first).
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Leaf(10, 10, modifier = Modifier.test_tag("dış").semantics(|c| c.set_test_tag("iç")));
    }));
    rule.on_node_with_tag("dış").assert_exists();
    rule.on_node_with_tag("iç").assert_does_not_exist();
    // Ids are the layout nodes': stable as the tree changes around them.
    let id = rule.on_node_with_tag("dış").fetch_semantics_node().id();
    rule.wait_for_idle();
    assert_eq!(rule.on_node_with_tag("dış").fetch_semantics_node().id(), id);
    assert!(rule.on_root().fetch_semantics_node().is_root());
}

fn semantics_read_state_and_follow_it() {
    let label = mutable_state_of("bir".to_string());
    let l = label.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let l = l.clone();
        Leaf(10, 10, modifier = Modifier.semantics(move |c| c.set_text(l.get())));
    }));
    rule.on_node_with_text("bir").assert_exists();
    let compositions = rule.host().stats().recompositions;
    label.set("iki".into());
    rule.wait_for_idle();
    rule.on_node_with_text("iki").assert_exists();
    assert_eq!(rule.host().stats().recompositions, compositions);
}

// ---- Drawing ---------------------------------------------------------------------------------------------------

/// A draw node drawing a text of what its state says (read while drawing).
#[derive(PartialEq)]
struct Label(MutableState<String>);

struct LabelNode(MutableState<String>);

impl ModifierNodeElement for Label {
    type Node = LabelNode;
    fn create(&self) -> LabelNode {
        LabelNode(self.0.clone())
    }
    fn update(&self, node: &mut LabelNode) {
        node.0 = self.0.clone();
    }
}

impl ModifierNode for LabelNode {
    node_capabilities!(draw);
}

impl DrawModifierNode for LabelNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        scope.draw_content();
        let text = TextDraw::new(self.0.get(), Offset::new(1.0, 2.0), 10.0, Size::new(10.0, 10.0));
        scope.context().draw_text(&text, Color::Black);
    }
}

fn drawing_goes_through_coordinators_and_observes_its_reads() {
    let label = mutable_state_of("önce".to_string());
    let l = label.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Stack(modifier = padded(Modifier, 5), || {
            Text("üst", font_size = 10.sp());
            Leaf(10, 10, modifier = padded(Modifier, 3).then_element(Label(l.clone())));
        });
    }));
    let r = draw(&rule);
    // Each layout node between begin_node and end_node: the root, the stack, the text, the leaf.
    assert_eq!(r.nodes.len(), 4);
    // The text inside the stack's padding; the label in its leaf's coordinates (past the padding layout node's 3
    // px, under the 13 px text).
    assert_eq!(r.texts[0], ("üst".to_string(), Offset::new(5.0, 13.0)));
    assert_eq!(r.texts[1], ("önce".to_string(), Offset::new(5.0 + 3.0 + 1.0, 5.0 + 13.0 + 3.0 + 2.0)));
    // A state read while drawing: drawn again, neither composed nor laid out again.
    let (compositions, relaid) = (rule.host().stats().recompositions, rule.host().relaid_out_count());
    label.set("sonra".into());
    assert!(rule.host().frame(rule.main_clock().current_time() * 1_000_000 + 16_000_000));
    assert_eq!(draw(&rule).texts[1].0, "sonra");
    assert_eq!(rule.host().stats().recompositions, compositions);
    assert_eq!(rule.host().relaid_out_count(), relaid);
}

// ---- Traversal and observation ---------------------------------------------------------------------------------

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
    fn traverse_key(&self) -> &dyn Any {
        &"group"
    }
}

thread_local! {
    static FOUND: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Walks the groups around it when attached.
#[derive(PartialEq)]
struct Walker;

struct WalkerNode;

impl ModifierNodeElement for Walker {
    type Node = WalkerNode;
    fn create(&self) -> WalkerNode {
        WalkerNode
    }
    fn update(&self, _: &mut WalkerNode) {}
}

impl ModifierNode for WalkerNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        let mut found = Vec::new();
        cx.traverse_ancestors(&"group", &mut |g| {
            g.with::<GroupNode, _>(|n| found.push(format!("up {}", n.0)));
            true
        });
        cx.traverse_descendants(&"group", &mut |g| {
            g.with::<GroupNode, _>(|n| found.push(format!("down {}", n.0)));
            TraversalDecision::ContinueTraversal
        });
        if let Some(n) = cx.find_nearest_ancestor(&"group") {
            n.with::<GroupNode, _>(|n| found.push(format!("nearest {}", n.0)));
        }
        FOUND.with(|f| *f.borrow_mut() = found);
    }
}

fn nodes_find_each_other_up_and_down_the_tree() {
    let _rule = KompozTestRule::new(kompozable_lambda!(|| {
        Stack(modifier = Modifier.then_element(Group("outer")), || {
            Stack(
                modifier = Modifier.then_element(Group("middle")).then_element(Walker).then_element(Group("same")),
                || {
                    Leaf(1, 1, modifier = Modifier.then_element(Group("child")));
                    Stack(|| Leaf(1, 1, modifier = Modifier.then_element(Group("grandchild"))));
                },
            );
        });
    }));
    let found = FOUND.with(|f| f.borrow().clone());
    assert_eq!(found, ["up middle", "up outer", "down same", "down child", "down grandchild", "nearest middle"]);
}

/// Reads a state in `observe_reads` when attached and counts the changes it is told of.
#[derive(PartialEq)]
struct Watcher(MutableState<i32>);

struct WatcherNode(MutableState<i32>, Rc<Cell<i32>>, Option<NodeHandle>);

thread_local! {
    static WATCHED: Rc<Cell<i32>> = Rc::new(Cell::new(0));
}

impl ModifierNodeElement for Watcher {
    type Node = WatcherNode;
    fn create(&self) -> WatcherNode {
        WatcherNode(self.0.clone(), WATCHED.with(|w| w.clone()), None)
    }
    fn update(&self, node: &mut WatcherNode) {
        node.0 = self.0.clone();
    }
}

impl ModifierNode for WatcherNode {
    node_capabilities!(observer);
    fn on_attach(&mut self, cx: &NodeCx) {
        let state = self.0.clone();
        cx.observe_reads(|| state.get());
        self.2 = Some(cx.handle());
    }
}

impl ObserverModifierNode for WatcherNode {
    fn on_observed_reads_changed(&mut self) {
        self.1.set(self.1.get() + 1);
        let state = self.0.clone();
        if let Some(h) = &self.2 {
            h.observe_reads(|| state.get());
        }
    }
}

fn a_node_is_told_when_what_it_observed_changes() {
    WATCHED.with(|w| w.set(0));
    let value = mutable_state_of(0);
    let v = value.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Leaf(1, 1, modifier = Modifier.then_element(Watcher(v.clone())));
    }));
    value.set(1);
    rule.wait_for_idle();
    value.set(2);
    rule.wait_for_idle();
    assert_eq!(WATCHED.with(|w| w.get()), 2);
}

// ---- The host --------------------------------------------------------------------------------------------------

fn host_environment_changes_recompose_and_measure_only_their_readers() {
    // C++'s host_environment_changes_recompose_and_measure_only_their_readers.
    let composed = Rc::new(Cell::new(0));
    let dark_runs = Rc::new(Cell::new(0));
    let measured = Rc::new(Cell::new(0));
    let (c, d, m) = (composed.clone(), dark_runs.clone(), measured.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        c.set(c.get() + 1);
        let (d, m) = (d.clone(), m.clone());
        Stack(move || {
            let m = m.clone();
            Layout(
                measure_policy = measure_policy(move |s, _, _| {
                    m.set(m.get() + 1);
                    s.layout(10, 10, |_| {})
                }),
            );
            let d = d.clone();
            scoped(move || {
                d.set(d.get() + 1);
                Text(if is_system_in_dark_theme() { "koyu" } else { "açık" });
            });
        });
    }));
    rule.set_layout_check(false);
    let measures = measured.get();
    rule.host().set_system_dark_theme(true);
    rule.wait_for_idle();
    rule.on_node_with_text("koyu").assert_exists();
    assert_eq!(dark_runs.get(), 2);
    // The insets and the window's focus: what reads them again, nothing else.
    rule.host().set_system_insets(SystemInsets { ime_visible: true, ..Default::default() });
    rule.host().set_window_focused(false);
    rule.wait_for_idle();
    assert_eq!((composed.get(), dark_runs.get(), measured.get()), (1, 2, measures));
    // invalidate(): all of it again.
    rule.host().invalidate();
    rule.wait_for_idle();
    assert!(composed.get() == 2 && measured.get() > measures);
}

fn set_content_replaces_the_content() {
    // The content's scope skips while its generation is the same: set_content gives the new content one of its
    // own, so it is composed though nothing it reads changed.
    let rule = KompozTestRule::new(kompozable_lambda!(|| Text("ilk")));
    rule.on_node_with_text("ilk").assert_exists();
    rule.set_content(kompozable_lambda!(|| Text("ikinci")));
    rule.on_node_with_text("ikinci").assert_exists();
    rule.on_node_with_text("ilk").assert_does_not_exist();
    // Set again to content that shows the same: composed all the same.
    let runs = Rc::new(Cell::new(0));
    let r = runs.clone();
    rule.set_content(kompozable_lambda!(move || {
        r.set(r.get() + 1);
        Text("ikinci");
    }));
    assert_eq!(runs.get(), 1);
}

fn window_info_and_layout_direction_reach_their_readers() {
    let focused = Rc::new(Cell::new(true));
    let f = focused.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        f.set(LocalWindowInfo.current().is_window_focused());
        Text(format!("{:?}", LocalLayoutDirection.current()));
    }));
    rule.host().set_window_focused(false);
    rule.wait_for_idle();
    assert!(!focused.get());
    rule.host().set_layout_direction(LayoutDirection::Rtl);
    rule.wait_for_idle();
    rule.on_node_with_text("Rtl").assert_exists();
}

fn keep_screen_on_while_laid_out() {
    // C++'s keep_screen_on_while_laid_out (without its popup: the windows package's).
    let video = mutable_state_of(false);
    let v = video.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Stack(|| {
            Text("Tarif");
            if v.get() {
                Leaf(100, 100, modifier = Modifier.keep_screen_on());
            }
        });
    }));
    assert!(!rule.host().keep_screen_on());
    video.set(true);
    rule.wait_for_idle();
    assert!(rule.host().keep_screen_on());
    video.set(false);
    rule.wait_for_idle();
    assert!(!rule.host().keep_screen_on());
}

fn sensitive_content_while_laid_out() {
    // C++'s sensitive_content_while_laid_out (a Text in place of its TextField; without its popup).
    let card = mutable_state_of(false);
    let taken = mutable_state_of(false);
    let (c, t) = (card.clone(), taken.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Stack(|| {
            Text("Ödeme");
            if c.get() {
                Text("4111", modifier = Modifier.sensitive_content(true));
            }
            if t.get() {
                Leaf(10, 10, modifier = Modifier.sensitive_content(false));
            }
        });
    }));
    assert!(!rule.host().sensitive_content());
    card.set(true);
    rule.wait_for_idle();
    assert!(rule.host().sensitive_content());
    card.set(false);
    taken.set(true);
    rule.wait_for_idle();
    assert!(!rule.host().sensitive_content());
    // The same text taking the hint back.
    let hint = mutable_state_of(true);
    let h = hint.clone();
    let same = KompozTestRule::new(kompozable_lambda!(move || {
        Text("4111", modifier = Modifier.sensitive_content(h.get()));
    }));
    assert!(same.host().sensitive_content());
    hint.set(false);
    same.wait_for_idle();
    assert!(!same.host().sensitive_content());
}

fn preferred_frame_rates_gather_the_fastest() {
    // C++'s preferred_frame_rates_gather_the_fastest.
    let fast = mutable_state_of(true);
    let high = mutable_state_of(false);
    let (f, h) = (fast.clone(), high.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Stack(|| {
            Leaf(10, 10, modifier = Modifier.preferred_frame_rate(90.0));
            Leaf(10, 10, modifier = Modifier.preferred_frame_rate(FrameRateCategory::Normal));
            if f.get() {
                Leaf(10, 10, modifier = Modifier.preferred_frame_rate(120.0));
            }
            if h.get() {
                Leaf(10, 10, modifier = Modifier.preferred_frame_rate(FrameRateCategory::High));
            }
        });
    }));
    assert_eq!(rule.host().preferred_frame_rate(), 120.0);
    assert_eq!(rule.host().preferred_frame_rate_category(), FrameRateCategory::Normal);
    fast.set(false);
    high.set(true);
    rule.wait_for_idle();
    assert_eq!(rule.host().preferred_frame_rate(), 90.0);
    assert_eq!(rule.host().preferred_frame_rate_category(), FrameRateCategory::High);
    let none = KompozTestRule::new(kompozable_lambda!(|| Leaf(10, 10)));
    assert_eq!(none.host().preferred_frame_rate(), 0.0);
    assert_eq!(none.host().preferred_frame_rate_category(), FrameRateCategory::Default);
}

// ---- Composition locals read by nodes ----------------------------------------------------------------------------

static LocalGap: KompozitionLocal<i32> = kompozition_local_of(|| 0);
static LocalUnit: KompozitionLocal<i32> = static_kompozition_local_of(|| 1);

thread_local! {
    static GAP_MEASURES: Cell<u32> = const { Cell::new(0) };
    static GAP_COMPOSED: Cell<u32> = const { Cell::new(0) };
}

/// A layout modifier node as wide as `LocalGap` times `LocalUnit`, read as it measures.
struct GapNode {
    handle: Option<NodeHandle>,
}

impl ModifierNode for GapNode {
    node_capabilities!(layout);
    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }
}

impl kompoz_ui::layout::LayoutModifierNode for GapNode {
    fn measure(&mut self, scope: &MeasureScope, m: &dyn Measurable, _: Constraints) -> MeasureResult {
        GAP_MEASURES.with(|c| c.set(c.get() + 1));
        let h = self.handle.as_ref().unwrap();
        let width = h.current_value_of(&LocalGap) * h.current_value_of(&LocalUnit);
        let p = m.measure(Constraints::fixed(width, 10));
        scope.layout(width, 10, move |pl| pl.place(&p, 0, 0))
    }
}

#[derive(PartialEq)]
struct GapElement;

impl ModifierNodeElement for GapElement {
    type Node = GapNode;
    fn create(&self) -> GapNode {
        GapNode { handle: None }
    }
    fn update(&self, _: &mut GapNode) {}
}

#[kompozable]
fn GapLeaf() {
    GAP_COMPOSED.with(|c| c.set(c.get() + 1));
    Leaf(0, 0, modifier = Modifier.then_element(GapElement).test_tag("aralık"));
}

fn a_dynamic_local_read_while_measuring_measures_again_without_composing() {
    // Compose's DynamicValueHolder: a node reading a dynamic local while it measures is measured again when the
    // provider gives another value, nothing composed; a static local's new value composes the layout again, and
    // its nodes are invalidated (Compose's compositionLocalMap setter).
    GAP_MEASURES.with(|c| c.set(0));
    GAP_COMPOSED.with(|c| c.set(0));
    let (gap, unit) = (mutable_state_of(10), mutable_state_of(1));
    let (g, u) = (gap.clone(), unit.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        KompozitionLocalProvider(&[LocalGap.provides(g.get()), LocalUnit.provides(u.get())], || GapLeaf());
    }));
    rule.set_layout_check(false);
    let width = |rule: &KompozTestRule| rule.on_node_with_tag("aralık").layout_node().size().width;
    assert_eq!(width(&rule), 10);
    let measured = GAP_MEASURES.with(|c| c.get());
    let leaf = rule.on_node_with_tag("aralık").layout_node().compositions();
    gap.set(25);
    rule.wait_for_idle();
    assert_eq!(width(&rule), 25);
    assert_eq!(GAP_MEASURES.with(|c| c.get()), measured + 1);
    assert_eq!(rule.on_node_with_tag("aralık").layout_node().compositions(), leaf); // (not composed)
    unit.set(2);
    rule.wait_for_idle();
    assert_eq!(width(&rule), 50);
    assert_eq!(GAP_COMPOSED.with(|c| c.get()), 1); // (what doesn't read it skipped)
    assert!(rule.on_node_with_tag("aralık").layout_node().compositions() > leaf);
}

// ---- The host's saved state --------------------------------------------------------------------------------------

#[kompozable]
fn Counted(shown: MutableState<bool>, handle: Rc<RefCell<Option<MutableState<i32>>>>) {
    if shown.get() {
        let count = remember_saveable(|| mutable_state_of(0));
        *handle.borrow_mut() = Some(count.clone());
        Text(format!("sayı {}", count.get()));
    }
}

/// The same content for both hosts (its values are found by where they are composed).
fn counted(shown: &MutableState<bool>, handle: &Rc<RefCell<Option<MutableState<i32>>>>) -> impl Content + 'static {
    let (s, h) = (shown.clone(), handle.clone());
    kompozable_lambda!(move || Counted(s.clone(), h.clone()))
}

fn the_host_saves_and_restores_its_saveable_state() {
    // C++'s saveState / restoreState: the composition's `remember_saveable` values written out with the host's
    // state (the host provides the registry at its root), given back to a host made from it before what keeps them
    // is composed.
    let bytes = {
        let (shown, handle) = (mutable_state_of(true), Rc::new(RefCell::new(None)));
        let rule = KompozTestRule::new(counted(&shown, &handle));
        handle.borrow().as_ref().unwrap().set(5);
        rule.wait_for_idle();
        rule.on_node_with_text("sayı 5").assert_exists();
        rule.host().save_state()
    };
    let (shown, handle) = (mutable_state_of(false), Rc::new(RefCell::new(None)));
    let rule = KompozTestRule::new(counted(&shown, &handle));
    rule.host().restore_state(&bytes);
    shown.set(true);
    rule.wait_for_idle();
    rule.on_node_with_text("sayı 5").assert_exists();
    // Bytes that aren't a saved state change nothing.
    rule.host().restore_state(b"kompoz");
    assert_eq!(SavedValue::decode(&rule.host().save_state()), SavedValue::decode(&bytes));
}

// ---- System gesture exclusion ------------------------------------------------------------------------------------

/// The platform told the system gesture exclusion rects.
#[derive(Default)]
struct Exclusions(RefCell<Vec<Vec<IntRect>>>);

impl kompoz_ui::platform::PlatformCallbacks for Exclusions {
    fn system_gesture_exclusion(&self, rects: &[IntRect]) {
        self.0.borrow_mut().push(rects.to_vec());
    }
}

fn system_gesture_exclusion_tells_the_platform_where_swipes_are_the_apps() {
    // C++'s system_gesture_exclusion_keeps_the_rects_in_view without its scrolled part (scroll containers are a
    // later stage's), with Compose's rects: each in the root, all of it or the part given, as nodes come and go.
    let shown = mutable_state_of(true);
    let s = shown.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let s = s.clone();
        Stack(move || {
            if s.get() {
                Leaf(100, 50, modifier = Modifier.system_gesture_exclusion()); // 0..50, all of it
            }
            // Only its first 30 across (a slider's edge, say).
            Leaf(
                200,
                60,
                modifier =
                    Modifier.system_gesture_exclusion_with(|c| Rect::new(0.0, 0.0, 30.0, c.size().height as f32)),
            );
            Leaf(50, 80, modifier = padded(Modifier.system_gesture_exclusion(), 10));
        });
    }));
    let exclusions = Rc::new(Exclusions::default());
    rule.host().set_callbacks(exclusions.clone());
    assert_eq!(
        rule.host().system_gesture_exclusion(),
        [IntRect::new(0, 0, 100, 50), IntRect::new(0, 50, 30, 110), IntRect::new(0, 110, 70, 210)]
    );
    // Gone, and the others up 50: the platform told.
    shown.set(false);
    rule.wait_for_idle();
    let expected = vec![IntRect::new(0, 0, 30, 60), IntRect::new(0, 60, 70, 160)];
    assert_eq!(rule.host().system_gesture_exclusion(), expected);
    assert_eq!(exclusions.0.borrow().last(), Some(&expected));
    // A layout pass that moves nothing tells nothing again.
    let told = exclusions.0.borrow().len();
    rule.host().invalidate();
    rule.wait_for_idle();
    assert_eq!(exclusions.0.borrow().len(), told);
}

tests!(
    coordinates_tell_where_layouts_are_and_map_between_them,
    a_layer_moves_and_clips_what_coordinates_tell,
    size_placement_and_position_callbacks_are_told_as_compose,
    a_child_placed_higher_is_drawn_above,
    custom_layout_layout_modifier_and_layout_id,
    measuring_a_child_twice_needs_a_multi_measure_layout,
    the_semantics_tree_merges_as_compose,
    semantics_read_state_and_follow_it,
    drawing_goes_through_coordinators_and_observes_its_reads,
    nodes_find_each_other_up_and_down_the_tree,
    a_node_is_told_when_what_it_observed_changes,
    host_environment_changes_recompose_and_measure_only_their_readers,
    set_content_replaces_the_content,
    window_info_and_layout_direction_reach_their_readers,
    keep_screen_on_while_laid_out,
    sensitive_content_while_laid_out,
    preferred_frame_rates_gather_the_fastest,
    a_dynamic_local_read_while_measuring_measures_again_without_composing,
    the_host_saves_and_restores_its_saveable_state,
    system_gesture_exclusion_tells_the_platform_where_swipes_are_the_apps,
);

//! KompozR's headless UI testing, as Compose's `androidx.compose.ui.test`: a [`KompozTestRule`] composes content in
//! a host of a fixed viewport, runs its frames on a virtual clock (C++'s 16 ms frames), and finds nodes in the
//! semantics tree to assert about.
//!
//! ```no_run
//! # use kompoz_test::*;
//! # use kompoz_runtime::kompozable_lambda;
//! let rule = KompozTestRule::new(kompozable_lambda!(|| Text("Merhaba")));
//! rule.on_node_with_text("Merhaba").assert_exists();
//! ```

#![allow(non_snake_case)]

mod input;
mod recording;
mod text;

pub use input::{EVENT_PERIOD_MILLIS, InjectionScope, MouseInjectionScope, ScrollWheel, TouchInjectionScope};
pub use recording::{Arc, Backdrop, ClipShape, Elevated, Mesh, PathShadow, RecordingCanvas, ShapeShadow};
pub use text::{FakeTextMeasurer, FontMetrics, Text};

use kompoz_graphics::{Dp, IntSize, Offset, Rect};
use kompoz_runtime::Content;
use kompoz_ui::KompozHost;
use kompoz_ui::semantics::{SemanticsNode, SemanticsProperties};
use std::cell::Cell;
use std::fmt;

/// One frame of virtual time (C++'s and Compose's test clock: 16 ms).
pub const FRAME_NANOS: i64 = 16_000_000;

/// The rule's virtual clock (Compose's MainTestClock).
pub struct MainClock<'a> {
    rule: &'a KompozTestRule,
}

impl MainClock<'_> {
    /// The virtual time, in milliseconds.
    pub fn current_time(&self) -> i64 {
        self.rule.clock.get() / 1_000_000
    }
    /// Runs frames for `millis` of virtual time (coroutine delays, animations).
    pub fn advance_time_by(&self, millis: i64) {
        self.rule.advance_time_by(millis);
    }
    /// Advances by exactly one frame.
    pub fn advance_time_by_frame(&self) {
        self.rule.advance_frame();
    }
}

/// Composes content in a headless host and drives it (Compose's ComposeContentTestRule, C++'s KompozTestRule): its
/// viewport (C++'s default 400 by 800 px), a virtual clock of 16 ms frames, finders over the semantics tree. The
/// layout check (every settled layout measured again from scratch, which must come out the same) and the keep check
/// (every frame drawn into a display list keeping the nodes that didn't change and into one drawing them all, which
/// must record the same) are on, as in C++'s test binary: a difference fails the frame.
pub struct KompozTestRule {
    host: KompozHost,
    clock: Cell<i64>,
    /// What injected input left (`perform_touch_input`, `perform_mouse_input`).
    injection: std::cell::RefCell<input::InjectionState>,
}

impl KompozTestRule {
    /// A rule composing `content` in a 400 by 800 viewport, run until idle.
    pub fn new(content: impl Content + 'static) -> KompozTestRule {
        KompozTestRule::with_viewport(content, IntSize::new(400, 800))
    }

    /// The same in a viewport of `viewport` px.
    pub fn with_viewport(content: impl Content + 'static, viewport: IntSize) -> KompozTestRule {
        let host = KompozHost::new(content);
        host.set_viewport_size(viewport);
        host.set_layout_check(true);
        host.set_keep_check(true);
        let rule = KompozTestRule { host, clock: Cell::new(0), injection: Default::default() };
        rule.wait_for_idle_within(0);
        rule
    }

    pub fn host(&self) -> &KompozHost {
        &self.host
    }

    /// Composes `content` instead, run until idle.
    pub fn set_content(&self, content: impl Content + 'static) {
        self.host.set_content(content);
        self.wait_for_idle();
    }

    /// Turns the layout check off (a test that counts measurements: the check measures everything once more) or
    /// on.
    pub fn set_layout_check(&self, on: bool) {
        self.host.set_layout_check(on);
    }

    /// Turns the keep check off (a test that counts draws: the check draws everything twice more each frame) or on.
    pub fn set_keep_check(&self, on: bool) {
        self.host.set_keep_check(on);
    }

    pub fn main_clock(&self) -> MainClock<'_> {
        MainClock { rule: self }
    }

    /// Runs frames (16 ms of virtual time each) until nothing is pending, coroutine delays due within 2 s included
    /// (C++'s `waitForIdle`).
    pub fn wait_for_idle(&self) {
        self.wait_for_idle_within(2000);
    }

    /// The same, delays due within `timer_horizon_millis` waited for.
    pub fn wait_for_idle_within(&self, timer_horizon_millis: i64) {
        let horizon = self.clock.get() + timer_horizon_millis * 1_000_000;
        for _ in 0..2000 {
            // Work on worker threads takes real time: give it some.
            if kompoz_coroutines::background_work_in_flight() > 0 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            self.advance_frame();
            let next = self.host.next_deadline();
            if !self.host.has_pending_work() && next.is_none_or(|n| n > horizon) {
                return;
            }
        }
        panic!("kompoz: wait_for_idle: never idle");
    }

    /// Runs frames for `millis` of virtual time.
    pub fn advance_time_by(&self, millis: i64) {
        let mut t = 0;
        while t < millis {
            self.advance_frame();
            t += 16;
        }
    }

    /// Advances virtual time by one frame and runs it; a difference the layout check or the keep check found
    /// fails.
    pub fn advance_frame(&self) {
        self.clock.set(self.clock.get() + FRAME_NANOS);
        self.host.frame(self.clock.get());
        let mut mismatches = self.host.take_layout_check_mismatches();
        mismatches.extend(self.host.take_keep_check_mismatches());
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    }

    // ---- Finders ----

    /// The node whose `TestTag` is `tag` (Compose's `onNodeWithTag`), in the merged tree.
    pub fn on_node_with_tag(&self, tag: &str) -> SemanticsNodeInteraction<'_> {
        let wanted = tag.to_string();
        self.on_node(
            move |n| n.config().get(SemanticsProperties::TestTag).is_some_and(|t| t == wanted),
            format!("TestTag = '{tag}'"),
            false,
        )
    }

    /// The node whose `TestTag` is `tag` in the unmerged tree.
    pub fn on_node_with_tag_unmerged(&self, tag: &str) -> SemanticsNodeInteraction<'_> {
        let wanted = tag.to_string();
        self.on_node(
            move |n| n.config().get(SemanticsProperties::TestTag).is_some_and(|t| t == wanted),
            format!("TestTag = '{tag}'"),
            true,
        )
    }

    /// The node one of whose content descriptions is `description` (Compose's `onNodeWithContentDescription`), in
    /// the merged tree.
    pub fn on_node_with_content_description(&self, description: &str) -> SemanticsNodeInteraction<'_> {
        let wanted = description.to_string();
        self.on_node(
            move |n| n.config().get(SemanticsProperties::ContentDescription).is_some_and(|ds| ds.contains(&wanted)),
            format!("ContentDescription = '{description}'"),
            false,
        )
    }

    /// The node whose text (or a part of it, with `substring`) is `text` (Compose's `onNodeWithText`), in the
    /// merged tree.
    pub fn on_node_with_text(&self, text: &str) -> SemanticsNodeInteraction<'_> {
        let wanted = text.to_string();
        self.on_node(
            move |n| n.config().get(SemanticsProperties::Text).is_some_and(|ts| ts.contains(&wanted)),
            format!("Text + EditableText contains '{text}'"),
            false,
        )
    }

    /// The node whose text contains `text`.
    pub fn on_node_with_text_containing(&self, text: &str) -> SemanticsNodeInteraction<'_> {
        let wanted = text.to_string();
        self.on_node(
            move |n| n.config().get(SemanticsProperties::Text).is_some_and(|ts| ts.iter().any(|t| t.contains(&wanted))),
            format!("Text contains substring '{text}'"),
            false,
        )
    }

    /// The root (Compose's `onRoot`).
    pub fn on_root(&self) -> SemanticsNodeInteraction<'_> {
        self.on_node(|n| n.is_root(), "isRoot".to_string(), false)
    }

    /// The one node `matcher` finds (Compose's `onNode`), described as `description`.
    pub fn on_node(
        &self,
        matcher: impl Fn(&SemanticsNode) -> bool + 'static,
        description: String,
        use_unmerged_tree: bool,
    ) -> SemanticsNodeInteraction<'_> {
        SemanticsNodeInteraction { rule: self, matcher: Box::new(matcher), description, use_unmerged_tree }
    }

    /// The nodes of the tree, merged or not.
    pub fn all_nodes(&self, use_unmerged_tree: bool) -> Vec<SemanticsNode> {
        self.host.semantics_owner().all_semantics_nodes(!use_unmerged_tree)
    }

    /// The host's density.
    pub fn density(&self) -> kompoz_graphics::Density {
        self.host.density()
    }

    /// What the host draws now, recorded.
    pub fn draw(&self) -> RecordingCanvas {
        let mut canvas = RecordingCanvas::new();
        self.host.draw(&mut canvas);
        canvas
    }

    /// The texts the host draws now, in order (C++'s `drawnTexts`).
    pub fn drawn_texts(&self) -> Vec<String> {
        self.draw().texts
    }
}

/// A node a finder found, to assert about (Compose's SemanticsNodeInteraction): looked up anew each time, so it
/// follows the tree as it changes.
pub struct SemanticsNodeInteraction<'a> {
    rule: &'a KompozTestRule,
    matcher: Box<dyn Fn(&SemanticsNode) -> bool>,
    description: String,
    use_unmerged_tree: bool,
}

impl fmt::Debug for SemanticsNodeInteraction<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SemanticsNodeInteraction({})", self.description)
    }
}

impl SemanticsNodeInteraction<'_> {
    fn find(&self) -> Vec<SemanticsNode> {
        self.rule.all_nodes(self.use_unmerged_tree).into_iter().filter(|n| (self.matcher)(n)).collect()
    }

    /// The node, or why there isn't exactly one (Compose's `fetchSemanticsNode`, panicking as its assertions do).
    #[track_caller]
    pub fn fetch_semantics_node(&self) -> SemanticsNode {
        let found = self.find();
        match found.len() {
            1 => found.into_iter().next().unwrap(),
            0 => panic!(
                "Failed: assertExists.\nReason: Expected exactly '1' node but could not find any node that satisfies: ({})",
                self.description
            ),
            n => panic!(
                "Failed: Expected exactly '1' node but found '{n}' nodes that satisfy: ({})\nNodes found: {found:?}",
                self.description
            ),
        }
    }

    /// Whether exactly one node matches (C++'s `exists`).
    pub fn exists(&self) -> bool {
        self.find().len() == 1
    }

    #[track_caller]
    pub fn assert_exists(&self) -> &Self {
        self.fetch_semantics_node();
        self
    }

    #[track_caller]
    pub fn assert_does_not_exist(&self) -> &Self {
        let found = self.find();
        assert!(
            found.is_empty(),
            "Failed: assertDoesNotExist.\nReason: Did not expect any node but found '{}' nodes that satisfy: ({})",
            found.len(),
            self.description
        );
        self
    }

    /// Its bounds in the root, not cut by clipping layers (Compose's `getUnclippedBoundsInRoot`), in px.
    #[track_caller]
    pub fn get_unclipped_bounds_in_root(&self) -> Rect {
        self.fetch_semantics_node().unclipped_bounds_in_root()
    }

    /// Its bounds in the root, cut by clipping layers (Compose's `getBoundsInRoot`), in px.
    #[track_caller]
    pub fn get_bounds_in_root(&self) -> Rect {
        self.fetch_semantics_node().bounds_in_root()
    }

    /// The layout node it is.
    #[track_caller]
    pub fn layout_node(&self) -> std::rc::Rc<kompoz_ui::LayoutNode> {
        self.fetch_semantics_node().layout_node().clone()
    }

    fn density(&self) -> f32 {
        self.rule.density().density
    }

    #[track_caller]
    fn assert_close(&self, what: &str, actual_px: f32, expected: Dp) {
        let actual = actual_px / self.density();
        assert!(
            (actual - expected.0).abs() <= 0.5,
            "Actual {what} is {actual}.dp, expected {}.dp (tolerance: 0.5.dp) for ({})",
            expected.0,
            self.description
        );
    }

    /// Its width is `expected` (dp), within half a dp (Compose's `assertWidthIsEqualTo`).
    #[track_caller]
    pub fn assert_width_is_equal_to(&self, expected: Dp) -> &Self {
        let b = self.get_unclipped_bounds_in_root();
        self.assert_close("width", b.width(), expected);
        self
    }

    #[track_caller]
    pub fn assert_height_is_equal_to(&self, expected: Dp) -> &Self {
        let b = self.get_unclipped_bounds_in_root();
        self.assert_close("height", b.height(), expected);
        self
    }

    /// Its top left in the root is (`left`, `top`) dp (Compose's `assertPositionInRootIsEqualTo`).
    #[track_caller]
    pub fn assert_position_in_root_is_equal_to(&self, left: Dp, top: Dp) -> &Self {
        let b = self.get_unclipped_bounds_in_root();
        self.assert_close("left", b.left, left);
        self.assert_close("top", b.top, top);
        self
    }

    #[track_caller]
    pub fn assert_left_position_in_root_is_equal_to(&self, left: Dp) -> &Self {
        self.assert_close("left", self.get_unclipped_bounds_in_root().left, left);
        self
    }

    #[track_caller]
    pub fn assert_top_position_in_root_is_equal_to(&self, top: Dp) -> &Self {
        self.assert_close("top", self.get_unclipped_bounds_in_root().top, top);
        self
    }

    /// Its position in the root, px.
    #[track_caller]
    pub fn position_in_root(&self) -> Offset {
        self.fetch_semantics_node().position_in_root()
    }
}

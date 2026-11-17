//! KompozR's headless UI testing, as Compose's `androidx.compose.ui.test`: a [`KompozTestRule`] composes content in
//! a host of a fixed viewport, runs its frames on a virtual clock (C++'s 16 ms frames), finds nodes in the semantics
//! trees of its window and popups ([`SemanticsMatcher`]s, finders), asserts about them and acts on them
//! ([`SemanticsNodeInteraction`]), and injects touch, mouse and key input.
//!
//! ```no_run
//! # use kompoz_test::*;
//! # use kompoz_runtime::kompozable_lambda;
//! let rule = KompozTestRule::new(kompozable_lambda!(|| Text("Merhaba")));
//! rule.on_node_with_text("Merhaba").assert_exists();
//! ```

#![allow(non_snake_case)]

mod input;
mod interaction;
mod matcher;
mod recording;
mod text;
mod text_actions;

pub use input::{
    EVENT_PERIOD_MILLIS, InjectionScope, KeyInjectionScope, MouseInjectionScope, ScrollWheel, TouchInjectionScope,
};
pub use interaction::{
    SelectionResult, SemanticsNodeInteraction, SemanticsNodeInteractionCollection, SemanticsSelector, print_node,
    print_nodes,
};
pub use matcher::*;
pub use recording::{
    RecordedArc, RecordedBackdrop, RecordedClipShape, RecordedElevationShadow, RecordedMesh, RecordedPathShadow,
    RecordedShapeShadow, RecordingCanvas,
};
pub use text::Text;

use kompoz_graphics::IntSize;
use kompoz_runtime::{Content, named};
use kompoz_ui::KompozHost;
use kompoz_ui::semantics::SemanticsNode;
use std::cell::Cell;

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

    /// The same in a viewport of `viewport` px. Text is measured by kompoz-text's fake backend (as the C++ Kompoz's
    /// tests measured it) unless the test installs another resolver (`host().set_font_family_resolver`).
    pub fn with_viewport(content: impl Content + 'static, viewport: IntSize) -> KompozTestRule {
        let host = KompozHost::new(content);
        host.set_font_family_resolver(kompoz_ui::platform::fake_resolver());
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

    // ---- Finders (Compose's Finders.kt) ----

    /// The one node `matcher` matches (Compose's `onNode`), in the merged tree (or the unmerged one).
    pub fn on_node_with(&self, matcher: SemanticsMatcher, use_unmerged_tree: bool) -> SemanticsNodeInteraction<'_> {
        SemanticsNodeInteraction::new(self, SemanticsSelector::of(matcher), use_unmerged_tree)
    }

    /// The nodes `matcher` matches (Compose's `onAllNodes`).
    pub fn on_all_nodes_with(
        &self,
        matcher: SemanticsMatcher,
        use_unmerged_tree: bool,
    ) -> SemanticsNodeInteractionCollection<'_> {
        SemanticsNodeInteractionCollection::new(self, SemanticsSelector::of(matcher), use_unmerged_tree)
    }

    /// The node whose test tag is `tag`, in the unmerged tree.
    pub fn on_node_with_tag_unmerged(&self, tag: &str) -> SemanticsNodeInteraction<'_> {
        self.on_node_with(has_test_tag(tag), true)
    }

    /// The root of the host's window (Compose's `onRoot`; with a popup shown there are several roots: use
    /// `on_all_nodes(is_root())`).
    pub fn on_root(&self) -> SemanticsNodeInteraction<'_> {
        self.on_node_with(is_root(), false)
    }

    /// The nodes of every window's tree (the host's, its popups'), merged or not.
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

#[named]
impl KompozTestRule {
    /// The one node `matcher` matches (Compose's `onNode(matcher, useUnmergedTree)`).
    pub fn on_node(
        &self,
        matcher: SemanticsMatcher,
        #[default(false)] use_unmerged_tree: bool,
    ) -> SemanticsNodeInteraction<'_> {
        self.on_node_with(matcher, use_unmerged_tree)
    }

    /// The nodes `matcher` matches (Compose's `onAllNodes(matcher, useUnmergedTree)`).
    pub fn on_all_nodes(
        &self,
        matcher: SemanticsMatcher,
        #[default(false)] use_unmerged_tree: bool,
    ) -> SemanticsNodeInteractionCollection<'_> {
        self.on_all_nodes_with(matcher, use_unmerged_tree)
    }

    /// The node whose test tag is `tag` (Compose's `onNodeWithTag`).
    pub fn on_node_with_tag(
        &self,
        tag: &str,
        #[default(false)] use_unmerged_tree: bool,
    ) -> SemanticsNodeInteraction<'_> {
        self.on_node_with(has_test_tag(tag), use_unmerged_tree)
    }

    /// The nodes whose test tag is `tag` (Compose's `onAllNodesWithTag`).
    pub fn on_all_nodes_with_tag(
        &self,
        tag: &str,
        #[default(false)] use_unmerged_tree: bool,
    ) -> SemanticsNodeInteractionCollection<'_> {
        self.on_all_nodes_with(has_test_tag(tag), use_unmerged_tree)
    }

    /// The node with a text `text` (or containing it, with `substring`) (Compose's `onNodeWithText`).
    pub fn on_node_with_text(
        &self,
        text: &str,
        #[default(false)] substring: bool,
        #[default(false)] ignore_case: bool,
        #[default(false)] use_unmerged_tree: bool,
    ) -> SemanticsNodeInteraction<'_> {
        self.on_node_with(has_text(text, substring, ignore_case), use_unmerged_tree)
    }

    /// The nodes with a text `text` (Compose's `onAllNodesWithText`).
    pub fn on_all_nodes_with_text(
        &self,
        text: &str,
        #[default(false)] substring: bool,
        #[default(false)] ignore_case: bool,
        #[default(false)] use_unmerged_tree: bool,
    ) -> SemanticsNodeInteractionCollection<'_> {
        self.on_all_nodes_with(has_text(text, substring, ignore_case), use_unmerged_tree)
    }

    /// The node with a content description `label` (Compose's `onNodeWithContentDescription`).
    pub fn on_node_with_content_description(
        &self,
        label: &str,
        #[default(false)] substring: bool,
        #[default(false)] ignore_case: bool,
        #[default(false)] use_unmerged_tree: bool,
    ) -> SemanticsNodeInteraction<'_> {
        self.on_node_with(has_content_description(label, substring, ignore_case), use_unmerged_tree)
    }

    /// The nodes with a content description `label` (Compose's `onAllNodesWithContentDescription`).
    pub fn on_all_nodes_with_content_description(
        &self,
        label: &str,
        #[default(false)] substring: bool,
        #[default(false)] ignore_case: bool,
        #[default(false)] use_unmerged_tree: bool,
    ) -> SemanticsNodeInteractionCollection<'_> {
        self.on_all_nodes_with(has_content_description(label, substring, ignore_case), use_unmerged_tree)
    }
}

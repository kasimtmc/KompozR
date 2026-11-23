//! KompozR's headless UI testing, as Compose's `androidx.compose.ui.test`: a [`KompozTestRule`] composes content in
//! a host of a fixed viewport, runs its frames on a virtual clock of 16 ms frames ([`MainClock`]: Compose's
//! MainTestClock, advancing on its own while waiting for idle unless `auto_advance` is off, infinite animations
//! cancelled while it does), finds nodes in the semantics trees of its window and popups ([`SemanticsMatcher`]s,
//! finders), asserts about them and acts on them ([`SemanticsNodeInteraction`]), and injects touch, mouse and key
//! input.
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

use kompoz_coroutines::{CoroutineContext, cancel_current};
use kompoz_graphics::IntSize;
use kompoz_runtime::{Content, kompozable_lambda, named};
use kompoz_ui::semantics::SemanticsNode;
use kompoz_ui::{InfiniteAnimationPolicy, KompozHost};
use std::cell::Cell;
use std::rc::Rc;

/// One frame of virtual time (C++'s and Compose's test clock: 16 ms).
pub const FRAME_NANOS: i64 = 16_000_000;

/// One frame, in milliseconds (Compose's test clock's `frameDelayMillis`).
const FRAME_MILLIS: i64 = FRAME_NANOS / 1_000_000;

/// What `wait_until` and `advance_time_until` panic with when their time runs out (Compose's
/// ComposeTimeoutException).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeTimeoutException(pub String);

impl std::fmt::Display for ComposeTimeoutException {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ComposeTimeoutException {}

/// Panics with a [`ComposeTimeoutException`] (Kotlin's `throw`), its message written out first (a panic of a value
/// that isn't text shows none).
fn timed_out(message: String) -> ! {
    eprintln!("kompoz: ComposeTimeoutException: {message}");
    std::panic::panic_any(ComposeTimeoutException(message))
}

/// The rule's virtual clock (Compose's MainTestClock): frames of 16 ms. While [`auto_advance`](Self::auto_advance)
/// is on (the default), waiting for idle advances it frame by frame until nothing is left to do, and the rule's
/// `InfiniteAnimationPolicy` cancels infinite animations (they would never let it be idle); off, it moves only when
/// the test advances it, and infinite animations run.
pub struct MainClock<'a> {
    rule: &'a KompozTestRule,
}

impl MainClock<'_> {
    /// The virtual time, in milliseconds (Compose's `currentTime`).
    pub fn current_time(&self) -> i64 {
        self.rule.clock.get() / 1_000_000
    }

    /// Whether waiting for idle advances it (Compose's `autoAdvance`, true by default).
    pub fn auto_advance(&self) -> bool {
        self.rule.auto_advance.get()
    }

    /// Lets waiting for idle advance it, or not (Compose's `autoAdvance =`): off, infinite animations run, and
    /// waiting for idle runs only what is due at the current time.
    pub fn set_auto_advance(&self, auto_advance: bool) {
        self.rule.auto_advance.set(auto_advance);
    }

    /// Advances by exactly one frame (Compose's `advanceTimeByFrame`).
    pub fn advance_time_by_frame(&self) {
        self.rule.advance_frame();
    }

    /// Runs what is due at the current time, the clock not moving (Compose's `runCurrent`, its test scheduler's
    /// `mainClock.scheduler.runCurrent()`): the coroutines launched or resumed since (launched by the test: queued
    /// on the dispatcher, as Kotlin's `launch` dispatches them), what they change composed and laid out.
    pub fn run_current(&self) {
        self.rule.run_current();
    }
}

#[named]
impl<'a> MainClock<'a> {
    /// Advances by `milliseconds` (Compose's `advanceTimeBy`): rounded up to whole frames, each run; with
    /// `ignore_frame_duration`, exactly by `milliseconds` — the frames due on the way run, then the coroutine timers
    /// due by its end without a frame.
    pub fn advance_time_by(&self, milliseconds: i64, #[default(false)] ignore_frame_duration: bool) {
        self.rule.advance_time_by_millis(milliseconds, ignore_frame_duration);
    }

    /// Advances frame by frame until `condition` holds (Compose's `advanceTimeUntil`); panics with a
    /// [`ComposeTimeoutException`] after `timeout_millis` of virtual time.
    pub fn advance_time_until(&self, #[default(1_000)] timeout_millis: i64, condition: impl Fn() -> bool) {
        let start = self.current_time();
        while !condition() {
            self.rule.advance_frame();
            if self.current_time() - start > timeout_millis {
                timed_out(format!("Condition still not satisfied after {timeout_millis} ms"));
            }
        }
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
    /// The clock's `auto_advance`, which the rule's infinite animation policy reads too.
    auto_advance: Rc<Cell<bool>>,
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
        let rule = KompozTestRule::host_with(content, viewport);
        rule.wait_for_idle_within(0);
        rule
    }

    /// A rule with nothing composed yet, in a 400 by 800 viewport (Compose's `createComposeRule()`): its clock can be
    /// set up first (`main_clock().set_auto_advance(false)`), then [`set_content`](Self::set_content) composes.
    pub fn empty() -> KompozTestRule {
        KompozTestRule::empty_with_viewport(IntSize::new(400, 800))
    }

    /// The same in a viewport of `viewport` px.
    pub fn empty_with_viewport(viewport: IntSize) -> KompozTestRule {
        KompozTestRule::host_with(kompozable_lambda!(|| {}), viewport)
    }

    /// The host composing `content`, its effects running with the rule's infinite animation policy (Compose's test
    /// rule's: infinite animations cancelled while the clock advances on its own).
    fn host_with(content: impl Content + 'static, viewport: IntSize) -> KompozTestRule {
        let auto_advance = Rc::new(Cell::new(true));
        let a = auto_advance.clone();
        let policy = InfiniteAnimationPolicy::new(move |operation| {
            if a.get() {
                // (Compose: `throw CancellationException("Infinite animations are disabled on tests")`)
                Box::pin(cancel_current::<()>())
            } else {
                operation
            }
        });
        let host = KompozHost::with_effect_coroutine_context(content, CoroutineContext::of(policy));
        host.set_font_family_resolver(kompoz_ui::platform::fake_resolver());
        host.set_viewport_size(viewport);
        host.set_layout_check(true);
        host.set_keep_check(true);
        KompozTestRule { host, clock: Cell::new(0), auto_advance, injection: Default::default() }
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

    /// Waits until the host is idle (Compose's `waitForIdle`). With the clock's `auto_advance` on: frames of 16 ms
    /// run until nothing is pending, the coroutine delays due within 2 s waited for too (C++'s `waitForIdle`; Compose
    /// doesn't wait for delays); infinite animations are cancelled meanwhile, as the rule's policy says. Off: what is
    /// due at the current time runs — applied changes, composition, layout, effects and the coroutines they wake —
    /// the clock left where it is (frames come when the test advances it).
    pub fn wait_for_idle(&self) {
        if self.auto_advance.get() {
            self.wait_for_idle_within(2000);
        } else {
            self.run_current();
        }
    }

    /// The same advancing, delays due within `timer_horizon_millis` waited for.
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

    /// Runs what is due at the current time, the clock not moving (Compose's `runCurrent`, and what its layout passes
    /// do): passes without a frame until nothing they do is left.
    fn run_current(&self) {
        for _ in 0..100 {
            self.host.run_due(self.clock.get());
            self.check_frame();
            // (what the keep check's drawing set going is the next frame's, as a drawing is a frame's)
            if !self.host.has_pending_work_before_drawing() {
                return;
            }
        }
        panic!("kompoz: run_current: never idle at the current time");
    }

    /// Runs `action` once the host is idle, giving its value (Compose's `runOnIdle`).
    pub fn run_on_idle<R>(&self, action: impl FnOnce() -> R) -> R {
        self.wait_for_idle();
        action()
    }

    /// Runs frames for `millis` of virtual time, rounded up to whole frames (the clock's `advance_time_by`).
    pub fn advance_time_by(&self, millis: i64) {
        self.advance_time_by_millis(millis, false);
    }

    fn advance_time_by_millis(&self, millis: i64, ignore_frame_duration: bool) {
        if !ignore_frame_duration {
            let mut t = 0;
            while t < millis {
                self.advance_frame();
                t += FRAME_MILLIS;
            }
            return;
        }
        let target = self.clock.get() + millis * 1_000_000;
        while self.clock.get() + FRAME_NANOS <= target {
            self.advance_frame();
        }
        if self.clock.get() < target {
            self.clock.set(target);
            self.run_current();
        }
    }

    /// Advances virtual time by one frame and runs it; a difference the layout check or the keep check found
    /// fails.
    pub fn advance_frame(&self) {
        self.clock.set(self.clock.get() + FRAME_NANOS);
        self.host.frame(self.clock.get());
        self.check_frame();
    }

    fn check_frame(&self) {
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
    /// Waits until `condition` holds (Compose's `waitUntil`): frame by frame while the clock's `auto_advance` is on;
    /// else what is due runs, the clock still, while the work of other threads (real time) goes on. Panics with a
    /// [`ComposeTimeoutException`] after `timeout_millis` of real time, `condition_description` in its message.
    pub fn wait_until(
        &self,
        #[default(None)] condition_description: Option<&str>,
        #[default(1_000)] timeout_millis: i64,
        condition: impl Fn() -> bool,
    ) {
        let start = std::time::Instant::now();
        while !condition() {
            if self.auto_advance.get() {
                self.advance_frame();
            } else {
                self.run_current();
            }
            // (Compose's Thread.sleep(10): what other threads do takes real time)
            std::thread::sleep(std::time::Duration::from_millis(10));
            if start.elapsed().as_millis() as i64 > timeout_millis {
                let what = condition_description.map_or(String::from("Condition"), |d| format!("Condition ({d})"));
                timed_out(format!("{what} still not satisfied after {timeout_millis} ms"));
            }
        }
    }

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

//! KompozR's headless UI testing, as Compose's `androidx.compose.ui.test`: a [`KompozTestRule`] composes content in
//! a host of a fixed viewport, runs its frames on a virtual clock of 16 ms frames ([`MainClock`]: Compose's
//! MainTestClock, advancing on its own while waiting for idle unless `auto_advance` is off, infinite animations
//! cancelled while it does), finds nodes in the semantics trees of its window and popups ([`SemanticsMatcher`]s,
//! finders), asserts about them and acts on them ([`SemanticsNodeInteraction`]), and injects touch, mouse and key
//! input.
//!
//! As Compose's, every finder and assertion waits for idle first ([`KompozTestRule::wait_for_idle`]: frames while
//! the host has frame work — animations, frame awaiters, recomposition, layout — with `auto_advance` on; coroutine
//! delays aren't waited for): what a test checks is the UI settled. To check a state part way through an animation,
//! turn the clock's `auto_advance` off and step it (`advance_time_by`, `advance_time_by_frame`); to let a delay
//! elapse, advance the clock past it. Finders called from inside a frame (a composition, a layout, an effect) panic,
//! as Compose's synchronization refuses to run on the UI thread.
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
    shortcut_modifier,
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

/// How long [`KompozTestRule::wait_for_idle`] waits, in real time, before it gives up (Espresso's master idling
/// policy's timeout, Compose's on Robolectric).
const IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

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
/// is on (the default), waiting for idle (and so every finder) advances it frame by frame while the host has frame
/// work — not to a coroutine's delay —, and the rule's `InfiniteAnimationPolicy` cancels infinite animations (they
/// would never let it be idle); off, it moves only when the test advances it, and infinite animations run.
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
    /// Whether the rule is running a frame or a pass of the host (synchronizing from inside one panics).
    in_frame: Cell<bool>,
    /// Whether the finders skip waiting for idle (Compose's test owner's `isImplicitWaitSuppressed`).
    implicit_wait_suppressed: Cell<bool>,
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
        rule.wait_for_idle();
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
        KompozTestRule {
            host,
            clock: Cell::new(0),
            auto_advance,
            in_frame: Cell::new(false),
            implicit_wait_suppressed: Cell::new(false),
            injection: Default::default(),
        }
    }

    pub fn host(&self) -> &KompozHost {
        &self.host
    }

    /// Moves the app's lifecycle (the host's, LocalLifecycleOwner's outside a navigation entry) to `state`, its
    /// observers told at once (C++'s `rule.host().setLifecycleState`): what the platform does as the window goes to
    /// the background and back. The rule's host starts resumed.
    pub fn set_lifecycle_state(&self, state: kompoz_lifecycle::LifecycleState) {
        self.host.set_lifecycle_state(state);
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

    /// Waits until the host is idle (Compose's `waitForIdle`: its idling strategy's `runUntilIdle` over the
    /// ComposeIdlingResource's `isIdleNow`). Each round runs what is due at the current time (Compose's
    /// `runCurrent`: applied changes, composition, layout, effects and the coroutines they wake), then — with the
    /// clock's `auto_advance` on — advances it frame by frame, up to 100 frames, while the host has work only a
    /// frame does (coroutines awaiting a frame, animations, scopes asking for one: the frame clock's awaiters;
    /// changes not yet composed or laid out); it is idle after a round that advanced no frame and left nothing to
    /// lay out, twice in a row. Coroutine timers (`delay`) aren't waited for, as Compose's aren't: a test waiting
    /// for one advances the clock to it (`main_clock().advance_time_by`). Infinite animations are cancelled while
    /// `auto_advance` is on (the rule's `InfiniteAnimationPolicy`), so they never keep it busy. With `auto_advance`
    /// off, a round runs what is due now and lays it out, the clock left where it is.
    ///
    /// Work on worker threads ([`kompoz_coroutines::run_in_background`]) is waited for too, in real time: KompozR's
    /// own idling resource for its background pool (Compose's tests register theirs). Panics when the host isn't
    /// idle after 60 s of real time (Espresso's master idling policy), and when called from inside a frame (a
    /// composition, a layout, an effect, a frame callback): Compose's synchronization can't run on the UI thread.
    #[track_caller]
    pub fn wait_for_idle(&self) {
        self.check_not_in_frame();
        let start = kompoz_runtime::clock::Instant::now();
        let mut attempts = 0u32;
        let mut was_idle = false;
        loop {
            if start.elapsed() >= IDLE_TIMEOUT {
                panic!(
                    "kompoz: Compose did not get idle after {attempts} attempts in {} seconds (pending: {}).",
                    IDLE_TIMEOUT.as_secs(),
                    self.busy_reasons()
                );
            }
            attempts += 1;
            let idle = self.is_idle_now() && kompoz_coroutines::background_work_in_flight() == 0;
            if idle && was_idle {
                return;
            }
            if kompoz_coroutines::background_work_in_flight() > 0 {
                // (what worker threads do takes real time: give it some; on the web there are none, and it runs as
                // the dispatcher runs)
                #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            was_idle = idle;
        }
    }

    /// One round of [`wait_for_idle`](Self::wait_for_idle) (Compose's ComposeIdlingResource `isIdleNow`): what is
    /// due runs, then frames while the clock advances on its own and the host has frame work (at most 100); idle if
    /// no frame was needed and nothing but recomposition (a frame's work) is left.
    fn is_idle_now(&self) -> bool {
        self.run_current();
        let mut frames = 0;
        while frames < 100 && self.auto_advance.get() && self.host.has_pending_work() {
            self.advance_frame();
            frames += 1;
        }
        // (recomposition left is a frame's: with `auto_advance` off it waits for the test's frames, as Compose's
        // idling resource's `checkLayoutBusy` alone decides then)
        frames == 0 && !self.host.has_pending_work_besides_recomposition()
    }

    /// What keeps the host busy, for the timeout's message (Compose's `getDiagnosticMessageIfBusy`).
    fn busy_reasons(&self) -> &'static str {
        if self.host.has_pending_work_without_frame() {
            "pending recompositions or measure/layout"
        } else if self.host.has_pending_work() {
            "pending recompositions (frame awaiters): most likely endless frame work that isn't an infinite animation"
        } else {
            "work on worker threads"
        }
    }

    /// Panics when the rule is running a frame or a pass of the host (a callback of it is calling): Compose's
    /// synchronization can't be called from the UI thread (its EspressoLink's `runUntilIdle` check).
    #[track_caller]
    fn check_not_in_frame(&self) {
        assert!(
            !self.in_frame.get(),
            "Functions that involve synchronization (Assertions, Actions, Synchronization; e.g. assertIsSelected(), \
             doClick(), runOnIdle()) cannot be run from the main thread. Did you nest such a function inside \
             runOnIdle {{}}, runOnUiThread {{}} or setContent {{}}?"
        );
    }

    /// Runs `block` with the finders and assertions not waiting for idle (Compose's `runWithoutImplicitWait`): they
    /// see the tree as it is now.
    pub fn run_without_implicit_wait<R>(&self, block: impl FnOnce() -> R) -> R {
        struct Restore<'a>(&'a Cell<bool>, bool);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }
        let _restore = Restore(&self.implicit_wait_suppressed, self.implicit_wait_suppressed.replace(true));
        block()
    }

    /// Runs what is due at the current time, the clock not moving (Compose's `runCurrent`, and what its layout passes
    /// do): passes without a frame until nothing they do is left. A composition that keeps invalidating itself (it
    /// writes what it read before: an AnimatedContent's visible list while one content replaces another of its key)
    /// is left to the frames after two passes that left only that: Compose recomposes at frames only, so what a
    /// composition writes back is the next frame's there.
    #[track_caller]
    fn run_current(&self) {
        self.check_not_in_frame();
        let mut only_recomposition = 0;
        for _ in 0..100 {
            self.as_frame(|| self.host.run_due(self.clock.get()));
            self.check_frame();
            // (what the keep check's drawing set going is the next frame's, as a drawing is a frame's)
            if !self.host.has_pending_work_before_drawing() {
                return;
            }
            if self.host.has_pending_work_besides_recomposition_before_drawing() {
                only_recomposition = 0;
            } else {
                only_recomposition += 1;
                if only_recomposition == 2 {
                    return;
                }
            }
        }
        panic!("kompoz: run_current: never idle at the current time");
    }

    /// Runs `pass` (a frame or a pass of the host) marked as running one.
    fn as_frame<R>(&self, pass: impl FnOnce() -> R) -> R {
        struct Restore<'a>(&'a Cell<bool>);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        self.in_frame.set(true);
        let _restore = Restore(&self.in_frame);
        pass()
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
    #[track_caller]
    pub fn advance_frame(&self) {
        self.check_not_in_frame();
        self.clock.set(self.clock.get() + FRAME_NANOS);
        self.as_frame(|| self.host.frame(self.clock.get()));
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

    /// The nodes of every window's tree (the host's, its popups'), merged or not, once the host is idle (Compose's
    /// test owner's `getAllSemanticsNodes`: its `getRoots` waits for idle first, unless in
    /// [`run_without_implicit_wait`](Self::run_without_implicit_wait)). Every finder's fetch and every assertion
    /// goes through it: what they see is the UI settled as [`wait_for_idle`](Self::wait_for_idle) leaves it — to
    /// check a state mid-animation, turn the clock's `auto_advance` off and step it.
    #[track_caller]
    pub fn all_nodes(&self, use_unmerged_tree: bool) -> Vec<SemanticsNode> {
        if !self.implicit_wait_suppressed.get() {
            self.wait_for_idle();
        }
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
        let start = kompoz_runtime::clock::Instant::now();
        while !condition() {
            if self.auto_advance.get() {
                self.advance_frame();
            } else {
                self.run_current();
            }
            // (Compose's Thread.sleep(10): what other threads do takes real time; none on the web, which can't sleep)
            #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
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

//! Bringing what asks into a scrollable's view, as Compose's ContentInViewNode.kt,
//! BringIntoViewRequestPriorityQueue.kt and UpdatableAnimationState.kt: requests queued by containment, the
//! smallest one that fits brought in by a spring animation whose target is worked out again each frame (things
//! move, requests come and go); a focused child the viewport shrank over kept in view.

use super::Orientation;
use super::bring_into_view_spec::{BringIntoViewSpec, LocalBringIntoViewSpec};
use super::scrollable::ScrollingLogic;
use crate::relocation::BringIntoViewResponder;
use kompoz_animation::{AnimationSpec, AnimationVector, AnimationVector1D, VectorConverter, VectorizedAnimationSpec};
use kompoz_coroutines::{CoroutineStart, await_frame_end, current_coroutine_context, current_job, with_frame_nanos};
use kompoz_graphics::{IntOffset, IntSize, Offset, Rect, Size};
use kompoz_runtime::named_args;
use kompoz_ui::MotionDurationScale;
use kompoz_ui::focus::BringIntoViewFuture;
use kompoz_ui::input::NestedScrollSource;
use kompoz_ui::layout::LayoutAwareModifierNode;
use kompoz_ui::{ModifierNode, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::task::{Poll, Waker};

/// Below this a rect is as good as in view (Compose's MinScrollThreshold).
const MinScrollThreshold: f32 = 0.5;

/// What gives the focused child's rect in the scrollable's coordinates (Compose's `getFocusedRect`).
pub(crate) type FocusedRect = Rc<dyn Fn() -> Option<Rect>>;

/// A scrollable's bringing into view (Compose's ContentInViewNode): delegated to by the scrollable, with a
/// [`BringIntoViewResponderNode`](crate::relocation::BringIntoViewResponderNode) of it.
pub(crate) struct ContentInViewNode {
    pub(crate) core: Rc<ContentInViewCore>,
}

pub(crate) struct ContentInViewCore {
    me: Weak<ContentInViewCore>,
    orientation: Cell<Orientation>,
    scrolling_logic: Rc<ScrollingLogic>,
    reverse_direction: Cell<bool>,
    bring_into_view_spec: RefCell<Option<Rc<dyn BringIntoViewSpec>>>,
    get_focused_rect: FocusedRect,
    bring_into_view_requests: Rc<BringIntoViewRequestPriorityQueue>,
    tracking_focused_child: Cell<bool>,
    /// Its size, none before its first measure (Compose's UnspecifiedIntSize).
    viewport_size: Cell<Option<IntSize>>,
    is_animation_running: Cell<bool>,
    handle: RefCell<Option<NodeHandle>>,
}

impl ContentInViewNode {
    pub(crate) fn new(
        orientation: Orientation,
        scrolling_logic: Rc<ScrollingLogic>,
        reverse_direction: bool,
        bring_into_view_spec: Option<Rc<dyn BringIntoViewSpec>>,
        get_focused_rect: FocusedRect,
    ) -> ContentInViewNode {
        let core = Rc::new_cyclic(|me| ContentInViewCore {
            me: me.clone(),
            orientation: Cell::new(orientation),
            scrolling_logic,
            reverse_direction: Cell::new(reverse_direction),
            bring_into_view_spec: RefCell::new(bring_into_view_spec),
            get_focused_rect,
            bring_into_view_requests: Rc::new(BringIntoViewRequestPriorityQueue::default()),
            tracking_focused_child: Cell::new(false),
            viewport_size: Cell::new(None),
            is_animation_running: Cell::new(false),
            handle: RefCell::new(None),
        });
        ContentInViewNode { core }
    }
}

impl ModifierNode for ContentInViewNode {
    node_capabilities!(layout_aware);

    fn should_auto_invalidate(&self) -> bool {
        false
    }

    fn on_attach(&mut self, cx: &NodeCx) {
        *self.core.handle.borrow_mut() = Some(cx.handle());
    }
}

impl LayoutAwareModifierNode for ContentInViewNode {
    fn on_remeasured(&mut self, size: IntSize) {
        self.core.on_remeasured(size);
    }
}

impl ContentInViewCore {
    fn rc(&self) -> Rc<ContentInViewCore> {
        self.me.upgrade().expect("kompoz: a scrollable's bring into view outlived it")
    }

    /// Its viewport's size, nothing before its first measure (Compose's `viewportSizeOrZero`).
    pub(crate) fn viewport_size_or_zero(&self) -> IntSize {
        self.viewport_size.get().unwrap_or(IntSize::Zero)
    }

    fn require_bring_into_view_spec(&self) -> Rc<dyn BringIntoViewSpec> {
        if let Some(spec) = self.bring_into_view_spec.borrow().clone() {
            return spec;
        }
        match self.handle.borrow().as_ref() {
            Some(h) => h.current_value_of(&LocalBringIntoViewSpec),
            None => LocalBringIntoViewSpec.default_value(),
        }
    }

    /// Compose's `update`.
    pub(crate) fn update(
        &self,
        orientation: Orientation,
        reverse_direction: bool,
        bring_into_view_spec: Option<Rc<dyn BringIntoViewSpec>>,
    ) {
        self.orientation.set(orientation);
        self.reverse_direction.set(reverse_direction);
        *self.bring_into_view_spec.borrow_mut() = bring_into_view_spec;
    }

    /// Compose's `onRemeasured`: a viewport shrunk over the focused child brings it back in.
    fn on_remeasured(&self, size: IntSize) {
        let previous_viewport_size = self.viewport_size_or_zero();
        self.viewport_size.set(Some(size));
        // Grown: nothing to do.
        if self.axis_cmp_int(size, previous_viewport_size) >= 0 {
            return;
        }
        // (reverseDirection is mostly the opposite of a reversed scroll: the viewport's adjustment uses its
        // opposite)
        let viewport_adjustment_for_reverse_scroll = if !self.reverse_direction.get() {
            if self.orientation.get() == Orientation::Vertical {
                IntOffset::new(0, previous_viewport_size.height - size.height)
            } else {
                IntOffset::new(previous_viewport_size.width - size.width, 0)
            }
        } else {
            IntOffset::Zero
        };
        if let Some(focused_child_bounds) = (self.get_focused_rect)()
            && !self.is_animation_running.get()
            && !self.tracking_focused_child.get()
            // (the bounds are from before the frame placed the children: a reversed one is adjusted by what that
            // placement will move them)
            && self.is_max_visible(focused_child_bounds, previous_viewport_size, IntOffset::Zero)
            && !self.is_max_visible(
                focused_child_bounds,
                self.viewport_size_or_zero(),
                viewport_adjustment_for_reverse_scroll,
            )
        {
            self.tracking_focused_child.set(true);
            self.launch_animation(viewport_adjustment_for_reverse_scroll);
        }
    }

    /// Compose's `launchAnimation`: a spring towards the scroll that brings the requests (and the tracked focused
    /// child) into view, its target worked out again after each frame.
    fn launch_animation(&self, viewport_adjustment_for_reverse_scroll: IntOffset) {
        let bring_into_view_spec = self.require_bring_into_view_spec();
        assert!(!self.is_animation_running.get(), "launchAnimation called when previous animation was running");
        let Some(scope) = self.handle.borrow().as_ref().and_then(|h| h.coroutine_scope()) else { return };
        let animation_state = Rc::new(UpdatableAnimationState::new(&*bring_into_view_spec.scroll_animation_spec()));
        let me = self.rc();
        let animation = async move {
            let animation_job = current_job();
            /// Compose's `finally`: the requests not satisfied are cancelled.
            struct Finally(Rc<ContentInViewCore>);
            impl Drop for Finally {
                fn drop(&mut self) {
                    self.0.is_animation_running.set(false);
                    self.0.bring_into_view_requests.cancel_and_remove_all();
                    self.0.tracking_focused_child.set(false);
                }
            }
            let _finally = Finally(me.clone());
            me.is_animation_running.set(true);
            let logic = me.scrolling_logic.clone();
            let (core, spec, state) = (me.clone(), bring_into_view_spec.clone(), animation_state.clone());
            logic
                .scroll(
                    kompoz_animation::MutatePriority::Default,
                    Box::new(move |scope| {
                        Box::pin(async move {
                            state
                                .value
                                .set(core.calculate_scroll_delta(&*spec, viewport_adjustment_for_reverse_scroll));
                            let (before_core, after_core, after_spec, after_state) =
                                (core.clone(), core.clone(), spec.clone(), state.clone());
                            state
                                .animate_to_zero(
                                    // Each frame, in the frame's callback.
                                    move |delta| {
                                        let logic = &before_core.scrolling_logic;
                                        // (reverseDirection is the opposite of what the scroll modifiers are given)
                                        let scroll_multiplier =
                                            if before_core.reverse_direction.get() { 1.0 } else { -1.0 };
                                        let adjusted_delta = scroll_multiplier * delta;
                                        let consumed_scroll = scroll_multiplier
                                            * logic.offset_to_float(logic.reverse_offset_if_needed(scope.scroll_by(
                                                logic.reverse_offset_if_needed(logic.float_to_offset(adjusted_delta)),
                                                NestedScrollSource::UserInput,
                                            )));
                                        if consumed_scroll.abs() < delta.abs() {
                                            // Not all taken this frame: it won't be later either (an end). Its
                                            // requests are cancelled, or it would ask forever.
                                            animation_job.cancel();
                                        }
                                    },
                                    // Each frame, after the frame's composition and layout (the scroll applied).
                                    move || {
                                        // The requests this scroll satisfied are done.
                                        after_core.bring_into_view_requests.resume_and_remove_while(|bounds| {
                                            match bounds {
                                                // (gone: removed)
                                                None => true,
                                                Some(b) => after_core.is_max_visible(
                                                    b,
                                                    after_core.viewport_size_or_zero(),
                                                    IntOffset::Zero,
                                                ),
                                            }
                                        });
                                        // The focused child tracked shows: no longer tracked.
                                        if after_core.tracking_focused_child.get()
                                            && (after_core.get_focused_rect)().is_some_and(|r| {
                                                after_core.is_max_visible(
                                                    r,
                                                    after_core.viewport_size_or_zero(),
                                                    IntOffset::Zero,
                                                )
                                            })
                                        {
                                            after_core.tracking_focused_child.set(false);
                                        }
                                        // The target again: things moved, requests came and went.
                                        after_state
                                            .value
                                            .set(after_core.calculate_scroll_delta(&*after_spec, IntOffset::Zero));
                                    },
                                )
                                .await;
                        })
                    }),
                )
                .await;
            // Done (or needing no animation): what is left (too big to satisfy) completes.
            me.bring_into_view_requests.resume_and_remove_all();
        };
        // (Compose: `coroutineScope.launch(start = CoroutineStart.UNDISPATCHED)`)
        named_args!(scope.launch(start = CoroutineStart::UNDISPATCHED, coroutine = animation));
    }

    /// The scroll the requests (or the tracked focused child) need now (Compose's `calculateScrollDelta`).
    fn calculate_scroll_delta(
        &self,
        bring_into_view_spec: &dyn BringIntoViewSpec,
        viewport_adjustment_for_reverse_scroll: IntOffset,
    ) -> f32 {
        let Some(viewport_size) = self.viewport_size.get() else { return 0.0 };
        let rectangle_to_make_visible = match self.find_bring_into_view_request() {
            Some(r) => r,
            None if self.tracking_focused_child.get() => match (self.get_focused_rect)() {
                Some(r) => r,
                None => return 0.0,
            },
            None => return 0.0,
        };
        let size = viewport_size.to_size();
        match self.orientation.get() {
            Orientation::Vertical => bring_into_view_spec.calculate_scroll_distance(
                rectangle_to_make_visible.top - viewport_adjustment_for_reverse_scroll.y as f32,
                rectangle_to_make_visible.bottom - rectangle_to_make_visible.top,
                size.height,
            ),
            Orientation::Horizontal => bring_into_view_spec.calculate_scroll_distance(
                rectangle_to_make_visible.left - viewport_adjustment_for_reverse_scroll.x as f32,
                rectangle_to_make_visible.right - rectangle_to_make_visible.left,
                size.width,
            ),
        }
    }

    /// The largest request that fits, from the smallest up; the smallest if none does (Compose's
    /// `findBringIntoViewRequest`).
    fn find_bring_into_view_request(&self) -> Option<Rect> {
        let mut rectangle_to_make_visible: Option<Rect> = None;
        let viewport = self.viewport_size_or_zero().to_size();
        let mut found = None;
        self.bring_into_view_requests.for_each_from_smallest(&mut |bounds| {
            if found.is_some() {
                return;
            }
            // (gone ones are removed later)
            let Some(bounds) = bounds else { return };
            if self.axis_cmp_size(bounds.size(), viewport) <= 0 {
                rectangle_to_make_visible = Some(bounds);
            } else {
                // One that doesn't fit: the next smaller (or this one, oversized, if none).
                found = Some(rectangle_to_make_visible.unwrap_or(bounds));
            }
        });
        found.or(rectangle_to_make_visible)
    }

    /// Where `child_bounds` will be once brought in (Compose's `computeDestination`).
    fn compute_destination(&self, child_bounds: Rect) -> Rect {
        let offset = self.relocation_offset(child_bounds, self.viewport_size_or_zero(), IntOffset::Zero);
        child_bounds.translate(-offset)
    }

    /// Whether `rect` shows (within the threshold) in a viewport of `size` (Compose's `isMaxVisible`).
    fn is_max_visible(&self, rect: Rect, size: IntSize, container_offset: IntOffset) -> bool {
        let relocation_offset = self.relocation_offset(rect, size, container_offset);
        relocation_offset.x.abs() <= MinScrollThreshold && relocation_offset.y.abs() <= MinScrollThreshold
    }

    /// Compose's `relocationOffset`.
    fn relocation_offset(&self, child_bounds: Rect, container_size: IntSize, container_offset: IntOffset) -> Offset {
        let size = container_size.to_size();
        let spec = self.require_bring_into_view_spec();
        match self.orientation.get() {
            Orientation::Vertical => Offset::new(
                0.0,
                spec.calculate_scroll_distance(
                    child_bounds.top - container_offset.y as f32,
                    child_bounds.bottom - child_bounds.top,
                    size.height,
                ),
            ),
            Orientation::Horizontal => Offset::new(
                spec.calculate_scroll_distance(
                    child_bounds.left - container_offset.x as f32,
                    child_bounds.right - child_bounds.left,
                    size.width,
                ),
                0.0,
            ),
        }
    }

    /// Sizes compared along the axis (Compose's `IntSize.compareTo`).
    fn axis_cmp_int(&self, a: IntSize, b: IntSize) -> i32 {
        let (a, b) = match self.orientation.get() {
            Orientation::Horizontal => (a.width, b.width),
            Orientation::Vertical => (a.height, b.height),
        };
        a.cmp(&b) as i32
    }

    /// The same of `Size`s (Compose's `Size.compareTo`).
    fn axis_cmp_size(&self, a: Size, b: Size) -> i32 {
        let (a, b) = match self.orientation.get() {
            Orientation::Horizontal => (a.width, b.width),
            Orientation::Vertical => (a.height, b.height),
        };
        a.partial_cmp(&b).map_or(0, |o| o as i32)
    }
}

impl BringIntoViewResponder for ContentInViewCore {
    /// Compose's `calculateRectForParent`.
    fn calculate_rect_for_parent(&self, local_rect: Rect) -> Rect {
        assert!(
            self.viewport_size.get().is_some(),
            "Expected BringIntoViewRequester to not be used before parents are placed."
        );
        self.compute_destination(local_rect)
    }

    /// Compose's `bringChildIntoView`: nothing for a rect that shows (or is gone); else a request, the animation
    /// launched if it isn't running.
    fn bring_child_into_view(self: Rc<Self>, local_rect: Rc<dyn Fn() -> Option<Rect>>) -> BringIntoViewFuture {
        Box::pin(async move {
            match local_rect() {
                None => return,
                Some(r) if self.is_max_visible(r, self.viewport_size_or_zero(), IntOffset::Zero) => return,
                Some(_) => {}
            }
            let request = Rc::new(Request::new(local_rect));
            let waiting = WaitRequest { request: request.clone(), queue: self.bring_into_view_requests.clone() };
            if self.bring_into_view_requests.enqueue(&request) && !self.is_animation_running.get() {
                self.launch_animation(IntOffset::Zero);
            }
            waiting.await;
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The requests

/// How a request ended: resumed (satisfied) or cancelled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outcome {
    Resumed,
    Cancelled,
}

/// A bring-into-view request (Compose's ContentInViewNode.Request): its bounds as they are now, and the coroutine
/// waiting on it.
struct Request {
    current_bounds: Rc<dyn Fn() -> Option<Rect>>,
    outcome: Cell<Option<Outcome>>,
    waker: RefCell<Option<Waker>>,
}

impl Request {
    fn new(current_bounds: Rc<dyn Fn() -> Option<Rect>>) -> Request {
        Request { current_bounds, outcome: Cell::new(None), waker: RefCell::new(None) }
    }

    fn complete(&self, outcome: Outcome) {
        if self.outcome.get().is_none() {
            self.outcome.set(Some(outcome));
            let waker = self.waker.borrow_mut().take();
            if let Some(w) = waker {
                w.wake();
            }
        }
    }
}

/// A request waited on: done once it is resumed or cancelled (the caller goes on either way: a cancelled one's
/// coroutine was a child of the responder's scope, as Compose's); dropped while waiting, it leaves the queue
/// (Compose's `invokeOnCancellation`).
struct WaitRequest {
    request: Rc<Request>,
    queue: Rc<BringIntoViewRequestPriorityQueue>,
}

impl std::future::Future for WaitRequest {
    type Output = ();
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<()> {
        if self.request.outcome.get().is_some() {
            Poll::Ready(())
        } else {
            *self.request.waker.borrow_mut() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl Drop for WaitRequest {
    fn drop(&mut self) {
        if self.request.outcome.get().is_none() {
            self.queue.remove(&self.request);
        }
    }
}

/// The requests, ordered by containment: each contains those after it (Compose's
/// BringIntoViewRequestPriorityQueue).
#[derive(Default)]
struct BringIntoViewRequestPriorityQueue {
    requests: RefCell<Vec<Rc<Request>>>,
}

impl BringIntoViewRequestPriorityQueue {
    fn remove(&self, request: &Rc<Request>) {
        self.requests.borrow_mut().retain(|r| !Rc::ptr_eq(r, request));
    }

    /// Cancels it, out of the queue (Compose's `continuation.cancel` and its cancellation handler).
    fn cancel(&self, request: &Rc<Request>) {
        self.remove(request);
        request.complete(Outcome::Cancelled);
    }

    /// Puts `request` where its bounds go (Compose's `enqueue`): after the last one containing it; one neither
    /// containing it nor contained by it (and those after it) is cancelled. False (and resumed at once) if it is
    /// gone already.
    fn enqueue(&self, request: &Rc<Request>) -> bool {
        let Some(request_bounds) = (request.current_bounds)() else {
            request.complete(Outcome::Resumed);
            return false;
        };
        let mut i = self.requests.borrow().len();
        while i > 0 {
            i -= 1;
            let r = self.requests.borrow()[i].clone();
            let Some(r_bounds) = (r.current_bounds)() else { continue };
            let intersection = request_bounds.intersect(r_bounds);
            if intersection == request_bounds {
                // This one contains the new one: after it.
                self.requests.borrow_mut().insert(i + 1, request.clone());
                return true;
            } else if intersection != r_bounds {
                // Not overlapping wholly: it and those after it are cancelled ("interrupted by a newer,
                // non-overlapping call"), the search goes on up. (Compose's loop `for (j in requests.size - 1..i)`
                // runs once only when this is the last one, and then cancels `requests[i]`.)
                let size = self.requests.borrow().len();
                if size - 1 <= i {
                    for _ in (size - 1)..=i {
                        let at = self.requests.borrow().get(i).cloned();
                        if let Some(at) = at {
                            self.cancel(&at);
                        }
                    }
                }
            }
            // Else the new one contains it: on up the queue.
        }
        // No request contains it: it contains them all (or there are none left), the head of the queue.
        self.requests.borrow_mut().insert(0, request.clone());
        true
    }

    /// Calls `block` with each request's bounds now, the smallest first (Compose's `forEachFromSmallest`).
    fn for_each_from_smallest(&self, block: &mut dyn FnMut(Option<Rect>)) {
        let requests = self.requests.borrow().clone();
        for r in requests.iter().rev() {
            block((r.current_bounds)());
        }
    }

    /// Resumes them all (Compose's `resumeAndRemoveAll`).
    fn resume_and_remove_all(&self) {
        let requests = std::mem::take(&mut *self.requests.borrow_mut());
        for r in requests {
            r.complete(Outcome::Resumed);
        }
    }

    /// Resumes the smallest ones while `block` says (Compose's `resumeAndRemoveWhile`).
    fn resume_and_remove_while(&self, mut block: impl FnMut(Option<Rect>) -> bool) {
        loop {
            let last = self.requests.borrow().last().cloned();
            let Some(last) = last else { return };
            if block((last.current_bounds)()) {
                self.requests.borrow_mut().pop();
                last.complete(Outcome::Resumed);
            } else {
                return;
            }
        }
    }

    /// Cancels them all (Compose's `cancelAndRemoveAll`).
    fn cancel_and_remove_all(&self) {
        let requests = self.requests.borrow().clone();
        for r in requests {
            self.cancel(&r);
        }
        assert!(self.requests.borrow().is_empty(), "uncancelled requests present");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The animation

/// Below this the value is zero (Compose's UpdatableAnimationState VisibilityThreshold).
const VisibilityThreshold: f32 = 0.01;

/// A float animated towards zero by a spec whose value can be changed between frames, the animation going on
/// from where it is with its velocity (Compose's UpdatableAnimationState).
struct UpdatableAnimationState {
    vectorized_spec: Box<dyn VectorizedAnimationSpec<AnimationVector1D>>,
    last_frame_time: Cell<Option<i64>>,
    last_velocity: Cell<AnimationVector1D>,
    is_running: Cell<bool>,
    value: Cell<f32>,
}

impl UpdatableAnimationState {
    fn new(animation_spec: &dyn AnimationSpec<f32>) -> UpdatableAnimationState {
        UpdatableAnimationState {
            vectorized_spec: <AnimationVector1D as AnimationVector>::vectorize(animation_spec, &f32::VectorConverter),
            last_frame_time: Cell::new(None),
            last_velocity: Cell::new(AnimationVector1D(0.0)),
            is_running: Cell::new(false),
            value: Cell::new(0.0),
        }
    }

    /// Animates the value to zero, `before_frame` given each frame's change of it (in the frame's callback),
    /// `after_frame` called after it (Compose's `animateToZero`).
    async fn animate_to_zero(&self, mut before_frame: impl FnMut(f32), mut after_frame: impl FnMut()) {
        assert!(!self.is_running.get(), "animateToZero called while previous animation is running");
        let duration_scale = current_coroutine_context().get::<MotionDurationScale>().map_or(1.0, |s| s.scale_factor());
        self.is_running.set(true);
        /// Compose's `finally`.
        struct Reset<'a>(&'a UpdatableAnimationState);
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.0.last_frame_time.set(None);
                self.0.last_velocity.set(AnimationVector1D(0.0));
                self.0.is_running.set(false);
            }
        }
        let _reset = Reset(self);
        let zero = AnimationVector1D(0.0);
        // (the value may change after each frame: the animation goes on until it is zero, whatever its duration)
        while self.value.get().abs() >= VisibilityThreshold {
            with_frame_nanos(|frame_time| {
                if self.last_frame_time.get().is_none() {
                    self.last_frame_time.set(Some(frame_time));
                }
                let vectorized_current_value = AnimationVector1D(self.value.get());
                let play_time = if duration_scale == 0.0 {
                    // Animations off (a11y or developer settings).
                    self.vectorized_spec.get_duration_nanos(
                        AnimationVector1D(self.value.get()),
                        zero,
                        self.last_velocity.get(),
                    )
                } else {
                    ((frame_time - self.last_frame_time.get().unwrap_or(frame_time)) as f64 / duration_scale as f64)
                        .round() as i64
                };
                let new_value = self
                    .vectorized_spec
                    .get_value_from_nanos(play_time, vectorized_current_value, zero, self.last_velocity.get())
                    .value;
                self.last_velocity.set(self.vectorized_spec.get_velocity_from_nanos(
                    play_time,
                    vectorized_current_value,
                    zero,
                    self.last_velocity.get(),
                ));
                self.last_frame_time.set(Some(frame_time));
                let delta = self.value.get() - new_value;
                self.value.set(new_value);
                before_frame(delta);
            })
            .await;
            // (Compose's continuation runs after the frame's layout: where the scroll put things is known)
            await_frame_end().await;
            after_frame();
            if duration_scale == 0.0 {
                // Never more than one loop with animations off.
                break;
            }
        }
        // The last loop may have left a value under the threshold: once more with exactly zero.
        if self.value.get().abs() != 0.0 {
            with_frame_nanos(|_| {
                let delta = self.value.get();
                self.value.set(0.0);
                before_frame(delta);
            })
            .await;
            // (Compose's continuation runs after the frame's layout: where the scroll put things is known)
            await_frame_end().await;
            after_frame();
        }
    }
}

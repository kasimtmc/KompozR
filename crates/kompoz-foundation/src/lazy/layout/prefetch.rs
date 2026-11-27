//! Prefetching (Compose's LazyLayoutPrefetchState.kt, PrefetchScheduler.kt and Android's AndroidPrefetchScheduler):
//! items composed (a piece at a time: pausable precomposition), applied and premeasured in the pauses between
//! frames before they scroll in, so the frame that shows them only places them. A request runs while the pause has
//! time for its next step, judged by what that step took before for items of its content type (moving averages);
//! an urgent one (it will show next frame) runs whenever there is any time. Nested lazy layouts in a prefetched
//! item prefetch their first items with it.

use super::lazy_layout::LazyLayoutItemContentFactory;
use kompoz_graphics::IntSize;
use kompoz_runtime::clock::Instant;
use kompoz_runtime::{kompozable, remember_keyed};
use kompoz_ui::layout::{PausedPrecomposition, PrecomposedSlotHandle, SlotKey, SubcomposeLayoutState};
use kompoz_ui::modifier::{TraversableNode, TraversalDecision};
use kompoz_ui::{
    Constraints, IdleScope, IdleWorkQueue, LocalIdleWorkQueue, Modifier, ModifierNode, ModifierNodeElement,
    node_capabilities,
};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

// ---------------------------------------------------------------------------------------------------------------
// Requests and the scheduler

/// What a prefetch request is told when it runs (Compose's PrefetchRequestScope): how long it has before the next
/// frame is due.
pub trait PrefetchRequestScope {
    fn available_time_nanos(&self) -> i64;
}

impl PrefetchRequestScope for IdleScope {
    fn available_time_nanos(&self) -> i64 {
        IdleScope::available_time_nanos(self)
    }
}

/// A piece of prefetch work (Compose's PrefetchRequest): run while there is time, whether it has more to do.
pub(crate) trait PrefetchRequest: Any {
    fn execute(&self, scope: &dyn PrefetchRequestScope) -> bool;
}

/// Where prefetch requests wait to run (Compose's PriorityPrefetchScheduler): high priority ones before low
/// priority ones.
pub(crate) trait PrefetchScheduler {
    fn schedule_high_priority_prefetch(&self, request: Rc<dyn PrefetchRequest>);
    fn schedule_low_priority_prefetch(&self, request: Rc<dyn PrefetchRequest>);
}

/// The scheduler of a host (Compose's AndroidPrefetchScheduler, the View's): its requests run in the host's pauses
/// between frames ([`IdleWorkQueue`]), as long as there is time, the first with more to do going on in the next
/// pause. One for each host, shared by its lazy layouts.
struct HostPrefetchScheduler {
    queue: IdleWorkQueue,
    /// The requests waiting, high priority first (in the order they came within a priority).
    requests: RefCell<Vec<(bool, Rc<dyn PrefetchRequest>)>>,
    me: Weak<HostPrefetchScheduler>,
}

impl HostPrefetchScheduler {
    fn owner(&self) -> usize {
        self as *const Self as usize
    }

    fn schedule(&self, high: bool, request: Rc<dyn PrefetchRequest>) {
        {
            let mut requests = self.requests.borrow_mut();
            let at =
                if high { requests.iter().position(|(h, _)| !h).unwrap_or(requests.len()) } else { requests.len() };
            requests.insert(at, (high, request));
        }
        self.start_execution();
    }

    /// Asks the host for its pauses, if it isn't asked already.
    fn start_execution(&self) {
        if self.queue.has_work_of(self.owner()) {
            return;
        }
        let me = self.me.clone();
        self.queue.request(self.owner(), move |scope| me.upgrade().is_some_and(|s| s.run(scope)));
    }

    /// Runs the requests while there is time (Compose's `run`/`runRequest`): whether there is more to do.
    fn run(&self, scope: &IdleScope) -> bool {
        loop {
            let Some(request) = self.requests.borrow().first().map(|(_, r)| r.clone()) else { return false };
            if scope.available_time_nanos() <= 0 {
                return true;
            }
            if request.execute(scope) {
                return true;
            }
            self.requests.borrow_mut().retain(|(_, r)| !Rc::ptr_eq(r, &request));
        }
    }
}

impl PrefetchScheduler for HostPrefetchScheduler {
    fn schedule_high_priority_prefetch(&self, request: Rc<dyn PrefetchRequest>) {
        self.schedule(true, request);
    }
    fn schedule_low_priority_prefetch(&self, request: Rc<dyn PrefetchRequest>) {
        self.schedule(false, request);
    }
}

/// Where there is no host to run requests (Compose's no-op scheduler): they are dropped.
struct NoopPrefetchScheduler;

impl PrefetchScheduler for NoopPrefetchScheduler {
    fn schedule_high_priority_prefetch(&self, _: Rc<dyn PrefetchRequest>) {}
    fn schedule_low_priority_prefetch(&self, _: Rc<dyn PrefetchRequest>) {}
}

thread_local! {
    /// The schedulers of the hosts (Compose keeps one in a tag of the View).
    static SCHEDULERS: RefCell<Vec<Weak<HostPrefetchScheduler>>> = const { RefCell::new(Vec::new()) };
}

fn scheduler_of(queue: &IdleWorkQueue) -> Rc<HostPrefetchScheduler> {
    SCHEDULERS.with(|s| {
        let mut schedulers = s.borrow_mut();
        schedulers.retain(|w| w.strong_count() > 0);
        if let Some(existing) = schedulers.iter().filter_map(Weak::upgrade).find(|s| s.queue == *queue) {
            return existing;
        }
        let scheduler = Rc::new_cyclic(|me| HostPrefetchScheduler {
            queue: queue.clone(),
            requests: RefCell::new(Vec::new()),
            me: me.clone(),
        });
        schedulers.push(Rc::downgrade(&scheduler));
        scheduler
    })
}

/// The scheduler of the host the composition is in (Compose's `rememberDefaultPrefetchScheduler`): requests run in
/// its pauses between frames; none outside a host.
#[kompozable]
pub(crate) fn remember_default_prefetch_scheduler() -> Rc<dyn PrefetchScheduler> {
    let queue = LocalIdleWorkQueue.current();
    remember_keyed(queue.clone(), move || match &queue {
        Some(queue) => scheduler_of(queue) as Rc<dyn PrefetchScheduler>,
        None => Rc::new(NoopPrefetchScheduler) as Rc<dyn PrefetchScheduler>,
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The state

/// How many items a nested lazy layout prefetches when no number is known yet.
pub(crate) const UnspecifiedNestedPrefetchCount: i32 = -1;

/// What a [`LazyLayout`](super::LazyLayout)'s items are prefetched through (Compose's LazyLayoutPrefetchState):
/// precomposition (and premeasure) of an item ahead of the frame that shows it, run in the host's pauses; with
/// `on_nested_prefetch`, what this layout prefetches when it is itself in an item another lazy layout prefetches.
/// Clones are the same state.
#[derive(Clone)]
pub struct LazyLayoutPrefetchState {
    inner: Rc<PrefetchStateInner>,
}

pub(crate) struct PrefetchStateInner {
    on_nested_prefetch: Option<Rc<dyn Fn(&dyn NestedPrefetchScope)>>,
    prefetch_metrics: Rc<PrefetchMetrics>,
    pub(crate) prefetch_handle_provider: RefCell<Option<Rc<PrefetchHandleProvider>>>,
    pub(crate) realized_nested_prefetch_count: Cell<i32>,
    pub(crate) ideal_nested_prefetch_count: Cell<i32>,
    pub(crate) last_number_of_nested_prefetch_items: Cell<i32>,
}

impl PartialEq for LazyLayoutPrefetchState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Default for LazyLayoutPrefetchState {
    fn default() -> Self {
        LazyLayoutPrefetchState::new(None)
    }
}

impl LazyLayoutPrefetchState {
    /// A state; `on_nested_prefetch` schedules what this layout prefetches when an item it is in is prefetched
    /// (Compose's `LazyLayoutPrefetchState(onNestedPrefetch)`).
    pub fn new(on_nested_prefetch: Option<Rc<dyn Fn(&dyn NestedPrefetchScope)>>) -> LazyLayoutPrefetchState {
        LazyLayoutPrefetchState {
            inner: Rc::new(PrefetchStateInner {
                on_nested_prefetch,
                prefetch_metrics: Rc::new(PrefetchMetrics::default()),
                prefetch_handle_provider: RefCell::new(None),
                realized_nested_prefetch_count: Cell::new(UnspecifiedNestedPrefetchCount),
                ideal_nested_prefetch_count: Cell::new(UnspecifiedNestedPrefetchCount),
                last_number_of_nested_prefetch_items: Cell::new(0),
            }),
        }
    }

    pub(crate) fn inner(&self) -> &PrefetchStateInner {
        &self.inner
    }

    /// Schedules item `index` to be composed ahead (Compose's `schedulePrecomposition`): the handle cancels it.
    pub fn schedule_precomposition(&self, index: i32) -> PrefetchHandle {
        self.schedule_precomposition_with_priority(index, true)
    }

    pub(crate) fn schedule_precomposition_with_priority(&self, index: i32, is_high_priority: bool) -> PrefetchHandle {
        let provider = self.inner.prefetch_handle_provider.borrow().clone();
        match provider {
            Some(p) => p.schedule_precomposition(index, is_high_priority, self.inner.prefetch_metrics.clone()),
            None => PrefetchHandle(None),
        }
    }

    /// Schedules item `index` to be composed and measured in `constraints` ahead (Compose's
    /// `schedulePrecompositionAndPremeasure`); `on_item_premeasured` is told its sizes once it is.
    pub fn schedule_precomposition_and_premeasure(
        &self,
        index: i32,
        constraints: Constraints,
        on_item_premeasured: Option<Rc<dyn Fn(&dyn PrefetchResultScope)>>,
    ) -> PrefetchHandle {
        self.schedule_precomposition_and_premeasure_with_priority(index, constraints, true, on_item_premeasured)
    }

    pub(crate) fn schedule_precomposition_and_premeasure_with_priority(
        &self,
        index: i32,
        constraints: Constraints,
        is_high_priority: bool,
        on_item_premeasured: Option<Rc<dyn Fn(&dyn PrefetchResultScope)>>,
    ) -> PrefetchHandle {
        let provider = self.inner.prefetch_handle_provider.borrow().clone();
        match provider {
            Some(p) => p.schedule_premeasure(
                index,
                constraints,
                self.inner.prefetch_metrics.clone(),
                is_high_priority,
                on_item_premeasured,
            ),
            None => PrefetchHandle(None),
        }
    }

    /// What this layout prefetches as a nested one (Compose's `collectNestedPrefetchRequests`).
    pub(crate) fn collect_nested_prefetch_requests(&self) -> Vec<Rc<dyn PrefetchRequest>> {
        let Some(on_nested_prefetch) = self.inner.on_nested_prefetch.clone() else { return Vec::new() };
        let scope = NestedPrefetchScopeImpl {
            state: self.clone(),
            nested_prefetch_item_count: self.inner.realized_nested_prefetch_count.get(),
            requests: RefCell::new(Vec::new()),
        };
        on_nested_prefetch(&scope);
        let requests = scope.requests.into_inner();
        self.inner.last_number_of_nested_prefetch_items.set(requests.len() as i32);
        requests
    }

    /// Lets this layout's items be prefetched by `provider` (none: the layout left).
    pub(crate) fn set_prefetch_handle_provider(&self, provider: Option<Rc<PrefetchHandleProvider>>) {
        *self.inner.prefetch_handle_provider.borrow_mut() = provider;
    }

    pub(crate) fn set_ideal_nested_prefetch_count(&self, count: i32) {
        self.inner.ideal_nested_prefetch_count.set(count);
    }
}

/// A prefetch scheduled (Compose's `LazyLayoutPrefetchState.PrefetchHandle`): it can be cancelled (what it
/// composed is disposed of), or marked urgent (the item shows next frame: it runs whenever there is any time).
#[derive(Clone)]
pub struct PrefetchHandle(Option<Rc<HandleAndRequest>>);

impl PrefetchHandle {
    pub fn cancel(&self) {
        if let Some(r) = &self.0 {
            r.cancel();
        }
    }

    pub fn mark_as_urgent(&self) {
        if let Some(r) = &self.0 {
            r.is_urgent.set(true);
        }
    }
}

/// What a premeasured prefetch tells (Compose's `PrefetchResultScope`): the item and the sizes of its layouts.
pub trait PrefetchResultScope {
    /// How many layouts the item's content emitted.
    fn placeables_count(&self) -> i32;
    /// The item.
    fn index(&self) -> i32;
    /// The size layout `placeable_index` was premeasured to.
    fn get_size(&self, placeable_index: i32) -> IntSize;
}

/// What a nested lazy layout schedules its prefetches in (Compose's NestedPrefetchScope): the number of items the
/// outer one would like it to (its last count of items in view; -1 until known), and the scheduling.
pub trait NestedPrefetchScope {
    fn nested_prefetch_item_count(&self) -> i32 {
        UnspecifiedNestedPrefetchCount
    }
    fn schedule_precomposition(&self, index: i32);
    fn schedule_precomposition_and_premeasure(&self, index: i32, constraints: Constraints);
}

struct NestedPrefetchScopeImpl {
    state: LazyLayoutPrefetchState,
    nested_prefetch_item_count: i32,
    requests: RefCell<Vec<Rc<dyn PrefetchRequest>>>,
}

impl NestedPrefetchScope for NestedPrefetchScopeImpl {
    fn nested_prefetch_item_count(&self) -> i32 {
        self.nested_prefetch_item_count
    }

    fn schedule_precomposition(&self, index: i32) {
        let Some(provider) = self.state.inner.prefetch_handle_provider.borrow().clone() else { return };
        let request = provider.create_nested_prefetch_request(index, None, self.state.inner.prefetch_metrics.clone());
        self.requests.borrow_mut().push(request);
    }

    fn schedule_precomposition_and_premeasure(&self, index: i32, constraints: Constraints) {
        let Some(provider) = self.state.inner.prefetch_handle_provider.borrow().clone() else { return };
        let request = provider.create_nested_prefetch_request(
            index,
            Some(constraints),
            self.state.inner.prefetch_metrics.clone(),
        );
        self.requests.borrow_mut().push(request);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Metrics

/// What prefetching items took, by content type (Compose's PrefetchMetrics).
#[derive(Default)]
pub(crate) struct PrefetchMetrics {
    averages_by_content_type: RefCell<HashMap<Option<SlotKey>, Rc<Averages>>>,
}

impl PrefetchMetrics {
    fn get_average(&self, content_type: &Option<SlotKey>) -> Rc<Averages> {
        self.averages_by_content_type.borrow_mut().entry(content_type.clone()).or_default().clone()
    }
}

/// Moving averages of what each step took for items of a content type (Compose's Averages).
pub(crate) struct Averages {
    composition_time_nanos: Cell<i64>,
    resume_time_nanos: Cell<i64>,
    pause_time_nanos: Cell<i64>,
    apply_time_nanos: Cell<i64>,
    measure_time_nanos: Cell<i64>,
    nested_prefetch_count: Cell<i32>,
}

impl Default for Averages {
    fn default() -> Self {
        Averages {
            composition_time_nanos: Cell::new(0),
            resume_time_nanos: Cell::new(0),
            pause_time_nanos: Cell::new(0),
            apply_time_nanos: Cell::new(0),
            measure_time_nanos: Cell::new(0),
            nested_prefetch_count: Cell::new(UnspecifiedNestedPrefetchCount),
        }
    }
}

impl Averages {
    /// A weighted moving average, biased to recent times (the idea is RecycledViewPool's).
    fn average_time(new: i64, current: i64) -> i64 {
        if current == 0 { new } else { current / 4 * 3 + new / 4 }
    }

    fn save(cell: &Cell<i64>, time_nanos: i64) {
        cell.set(Self::average_time(time_nanos, cell.get()));
    }

    fn save_nested_prefetch_count(&self, count: i32) {
        let current = self.nested_prefetch_count.get();
        self.nested_prefetch_count.set(if current == UnspecifiedNestedPrefetchCount {
            count
        } else {
            (current * 3 + count) / 4
        });
    }

    fn clear_measure_time(&self) {
        self.measure_time_nanos.set(0);
    }

    #[allow(dead_code)]
    pub(crate) fn composition_time_nanos(&self) -> i64 {
        self.composition_time_nanos.get()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Handles and requests

/// What schedules a layout's prefetches (Compose's PrefetchHandleProvider): made by the layout for its item
/// content factory, its subcompose state and its host's scheduler.
pub(crate) struct PrefetchHandleProvider {
    item_content_factory: Rc<LazyLayoutItemContentFactory>,
    subcompose_layout_state: SubcomposeLayoutState,
    executor: Rc<dyn PrefetchScheduler>,
    /// Cleared once the layout left (requests then do nothing).
    is_state_active: Cell<bool>,
    /// Whether a request that composed in a run pauses before premeasuring (Compose's testing switch).
    should_pause_between_precomposition_and_premeasure: Cell<bool>,
    me: Weak<PrefetchHandleProvider>,
}

impl PrefetchHandleProvider {
    pub(crate) fn new(
        item_content_factory: Rc<LazyLayoutItemContentFactory>,
        subcompose_layout_state: SubcomposeLayoutState,
        executor: Rc<dyn PrefetchScheduler>,
    ) -> Rc<PrefetchHandleProvider> {
        Rc::new_cyclic(|me| PrefetchHandleProvider {
            item_content_factory,
            subcompose_layout_state,
            executor,
            is_state_active: Cell::new(true),
            should_pause_between_precomposition_and_premeasure: Cell::new(false),
            me: me.clone(),
        })
    }

    pub(crate) fn on_disposed(&self) {
        self.is_state_active.set(false);
    }

    fn new_request(
        &self,
        index: i32,
        constraints: Option<Constraints>,
        prefetch_metrics: Rc<PrefetchMetrics>,
        on_item_premeasured: Option<Rc<dyn Fn(&dyn PrefetchResultScope)>>,
    ) -> Rc<HandleAndRequest> {
        Rc::new_cyclic(|me| HandleAndRequest {
            index,
            provider: self.me.upgrade().expect("a prefetch handle provider"),
            prefetch_metrics,
            on_item_premeasured,
            premeasure_constraints: constraints,
            precompose_handle: RefCell::new(None),
            paused_precomposition: RefCell::new(None),
            is_measured: Cell::new(false),
            is_canceled: Cell::new(false),
            is_applied: Cell::new(false),
            key_used_for_composition: RefCell::new(None),
            has_resolved_nested_prefetches: Cell::new(false),
            nested_prefetch_controller: RefCell::new(None),
            is_urgent: Cell::new(false),
            available_time_nanos: Cell::new(0),
            elapsed_time_nanos: Cell::new(0),
            start_time: Cell::new(Instant::now()),
            pause_requested: Cell::new(false),
            me: me.clone(),
        })
    }

    fn execute_with_priority(&self, request: Rc<dyn PrefetchRequest>, is_high_priority: bool) {
        if is_high_priority {
            self.executor.schedule_high_priority_prefetch(request);
        } else {
            self.executor.schedule_low_priority_prefetch(request);
        }
    }

    pub(crate) fn schedule_precomposition(
        &self,
        index: i32,
        is_high_priority: bool,
        prefetch_metrics: Rc<PrefetchMetrics>,
    ) -> PrefetchHandle {
        let request = self.new_request(index, None, prefetch_metrics, None);
        self.execute_with_priority(request.clone(), is_high_priority);
        PrefetchHandle(Some(request))
    }

    pub(crate) fn schedule_premeasure(
        &self,
        index: i32,
        constraints: Constraints,
        prefetch_metrics: Rc<PrefetchMetrics>,
        is_high_priority: bool,
        on_item_premeasured: Option<Rc<dyn Fn(&dyn PrefetchResultScope)>>,
    ) -> PrefetchHandle {
        let request = self.new_request(index, Some(constraints), prefetch_metrics, on_item_premeasured);
        self.execute_with_priority(request.clone(), is_high_priority);
        PrefetchHandle(Some(request))
    }

    pub(crate) fn create_nested_prefetch_request(
        &self,
        index: i32,
        constraints: Option<Constraints>,
        prefetch_metrics: Rc<PrefetchMetrics>,
    ) -> Rc<dyn PrefetchRequest> {
        self.new_request(index, constraints, prefetch_metrics, None)
    }
}

/// A prefetch of one item, its handle and its request at once (Compose's HandleAndRequestImpl).
pub(crate) struct HandleAndRequest {
    index: i32,
    provider: Rc<PrefetchHandleProvider>,
    prefetch_metrics: Rc<PrefetchMetrics>,
    on_item_premeasured: Option<Rc<dyn Fn(&dyn PrefetchResultScope)>>,
    premeasure_constraints: Option<Constraints>,
    precompose_handle: RefCell<Option<PrecomposedSlotHandle>>,
    paused_precomposition: RefCell<Option<PausedPrecomposition>>,
    is_measured: Cell<bool>,
    is_canceled: Cell<bool>,
    is_applied: Cell<bool>,
    key_used_for_composition: RefCell<Option<super::LazyKey>>,
    has_resolved_nested_prefetches: Cell<bool>,
    nested_prefetch_controller: RefCell<Option<Rc<NestedPrefetchController>>>,
    is_urgent: Cell<bool>,
    available_time_nanos: Cell<i64>,
    elapsed_time_nanos: Cell<i64>,
    start_time: Cell<Instant>,
    pause_requested: Cell<bool>,
    me: Weak<HandleAndRequest>,
}

impl HandleAndRequest {
    fn is_composed(&self) -> bool {
        self.is_applied.get() || self.paused_precomposition.borrow().as_ref().is_some_and(|p| p.is_complete())
    }

    fn cancel(&self) {
        if !self.is_canceled.get() {
            self.is_canceled.set(true);
            self.clean_up();
        }
    }

    /// Whether a step that took `average` before fits in `available` (Compose's `shouldExecute`): an urgent
    /// request's whenever there is any time.
    fn should_execute(&self, available: i64, average: i64) -> bool {
        let required = if self.is_urgent.get() { 0 } else { average };
        available > required
    }

    fn reset_available_time_to(&self, available_time_nanos: i64) {
        self.available_time_nanos.set(available_time_nanos);
        self.start_time.set(Instant::now());
        self.elapsed_time_nanos.set(0);
    }

    fn update_elapsed_and_available_time(&self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.start_time.get()).as_nanos() as i64;
        self.elapsed_time_nanos.set(elapsed);
        self.available_time_nanos.set(self.available_time_nanos.get() - elapsed);
        self.start_time.set(now);
    }

    fn clean_up(&self) {
        if let Some(p) = self.paused_precomposition.borrow_mut().take() {
            p.cancel();
        }
        if let Some(h) = self.precompose_handle.borrow_mut().take() {
            h.dispose();
        }
        *self.nested_prefetch_controller.borrow_mut() = None;
    }

    /// Compose's `executeRequest`: compose (a piece at a time), apply, nested prefetches, premeasure, each when
    /// there is time for it; whether there is more to do.
    fn execute_request(&self, scope: &dyn PrefetchRequestScope) -> bool {
        let factory = &self.provider.item_content_factory;
        let item_provider = factory.item_provider();
        let is_valid = !self.is_canceled.get() && self.index >= 0 && self.index < item_provider.item_count();
        if !is_valid {
            self.clean_up();
            return false;
        }
        let key = item_provider.get_key(self.index);
        if self.key_used_for_composition.borrow().as_ref().is_some_and(|k| *k != key) {
            // (the item at the index changed: the request is no longer valid)
            self.clean_up();
            return false;
        }
        let content_type = item_provider.get_content_type(self.index);
        let average = self.prefetch_metrics.get_average(&content_type);
        let was_composed_at_start = self.is_composed();

        self.reset_available_time_to(scope.available_time_nanos());
        if !self.is_composed() {
            if self.should_execute(
                self.available_time_nanos.get(),
                average.resume_time_nanos.get() + average.pause_time_nanos.get(),
            ) {
                self.perform_pausable_composition(&key, &content_type, &average);
            }
            if !self.is_composed() {
                return true;
            }
        }

        if self.paused_precomposition.borrow().is_some() {
            if self.should_execute(self.available_time_nanos.get(), average.apply_time_nanos.get()) {
                self.perform_apply();
                self.update_elapsed_and_available_time();
                Averages::save(&average.apply_time_nanos, self.elapsed_time_nanos.get());
            } else {
                return true;
            }
        }

        // (nested prefetching is best effort: what the item's nested layouts become later isn't followed)
        if !self.has_resolved_nested_prefetches.get() {
            if self.available_time_nanos.get() > 0 {
                *self.nested_prefetch_controller.borrow_mut() = self.resolve_nested_prefetch_states();
                self.has_resolved_nested_prefetches.set(true);
            } else {
                return true;
            }
        }
        let controller = self.nested_prefetch_controller.borrow().clone();
        let has_more_work = controller.as_ref().is_some_and(|c| {
            c.execute_nested_prefetches(scope, average.nested_prefetch_count.get(), self.is_urgent.get())
        });
        if has_more_work {
            return true;
        }
        if let Some(c) = &controller
            && c.executed_nested_prefetch.get()
        {
            self.update_elapsed_and_available_time();
            c.executed_nested_prefetch.set(false);
        }

        if let Some(constraints) = self.premeasure_constraints
            && !self.is_measured.get()
        {
            if self.provider.should_pause_between_precomposition_and_premeasure.get() && !was_composed_at_start {
                return true;
            }
            if self.should_execute(self.available_time_nanos.get(), average.measure_time_nanos.get()) {
                self.perform_measure(constraints);
                self.update_elapsed_and_available_time();
                Averages::save(&average.measure_time_nanos, self.elapsed_time_nanos.get());
                if let Some(on_premeasured) = &self.on_item_premeasured {
                    on_premeasured(self);
                }
            } else {
                return true;
            }
        }

        // Measured: the "ideal" number of nested prefetches is known, saved to the average.
        if self.is_measured.get()
            && self.has_resolved_nested_prefetches.get()
            && let Some(controller) = controller
        {
            let ideal = controller.collect_ideal_nested_prefetch_count();
            average.save_nested_prefetch_count(ideal);
            let last = controller.collect_nested_prefetched_items_count();
            // (fewer prefetched last time than next: the measure time measured is wrong, measured again)
            if last < ideal {
                average.clear_measure_time();
            }
        }
        false
    }

    /// Composes the item a piece at a time until it is complete or time runs out (Compose's
    /// `performPausableComposition`).
    fn perform_pausable_composition(
        &self,
        key: &super::LazyKey,
        content_type: &Option<SlotKey>,
        averages: &Rc<Averages>,
    ) {
        let composition = {
            let mut paused = self.paused_precomposition.borrow_mut();
            if paused.is_none() {
                let factory = &self.provider.item_content_factory;
                let content = factory.get_content(self.index, key, content_type);
                *paused = Some(
                    self.provider.subcompose_layout_state.create_paused_precomposition_shared(key.clone(), content),
                );
                *self.key_used_for_composition.borrow_mut() = Some(key.clone());
            }
            // (a handle of the same slot, so composing doesn't hold this borrow)
            PausedHandle(self.me.clone())
        };
        self.pause_requested.set(false);
        while !composition.is_complete() && !self.pause_requested.get() {
            let me = self.me.clone();
            let averages = averages.clone();
            composition.resume(move || {
                let Some(me) = me.upgrade() else { return true };
                if !me.pause_requested.get() {
                    me.update_elapsed_and_available_time();
                    Averages::save(&averages.resume_time_nanos, me.elapsed_time_nanos.get());
                    let pause = !me.should_execute(
                        me.available_time_nanos.get(),
                        averages.resume_time_nanos.get() + averages.pause_time_nanos.get(),
                    );
                    me.pause_requested.set(pause);
                }
                me.pause_requested.get()
            });
        }
        self.update_elapsed_and_available_time();
        if self.pause_requested.get() {
            Averages::save(&averages.pause_time_nanos, self.elapsed_time_nanos.get());
        } else {
            Averages::save(&averages.resume_time_nanos, self.elapsed_time_nanos.get());
        }
    }

    fn perform_apply(&self) {
        let precomposition = self.paused_precomposition.borrow_mut().take().expect("Nothing to apply!");
        *self.precompose_handle.borrow_mut() = Some(precomposition.apply());
        self.is_applied.set(true);
    }

    fn perform_measure(&self, constraints: Constraints) {
        assert!(
            !self.is_canceled.get(),
            "Callers should check whether the request is still valid before calling performMeasure()"
        );
        assert!(!self.is_measured.get(), "Request was already measured!");
        self.is_measured.set(true);
        let handle = self.precompose_handle.borrow();
        let handle = handle.as_ref().expect("performComposition() must be called before performMeasure()");
        for placeable_index in 0..handle.placeables_count() {
            handle.premeasure(placeable_index, constraints);
        }
    }

    /// The prefetch states of the lazy layouts in the item (Compose's `resolveNestedPrefetchStates`).
    fn resolve_nested_prefetch_states(&self) -> Option<Rc<NestedPrefetchController>> {
        let handle = self.precompose_handle.borrow();
        let handle = handle.as_ref().expect("Should precompose before resolving nested prefetch states");
        let mut nested_states: Vec<LazyLayoutPrefetchState> = Vec::new();
        handle.traverse_descendants(&TraversablePrefetchStateNodeKey, &mut |cx| {
            if let Some(state) = cx.with(|n: &mut TraversablePrefetchStateNode| n.prefetch_state.clone()) {
                nested_states.push(state);
            }
            TraversalDecision::SkipSubtreeAndContinueTraversal
        });
        if nested_states.is_empty() {
            None
        } else {
            Some(Rc::new(NestedPrefetchController {
                states: nested_states.clone(),
                requests_by_state: RefCell::new(vec![None; nested_states.len()]),
                state_index: Cell::new(0),
                request_index: Cell::new(0),
                executed_nested_prefetch: Cell::new(false),
                parent_canceled: self.me.clone(),
            }))
        }
    }
}

/// The paused precomposition of a request, reached through it (so resuming doesn't borrow the request).
struct PausedHandle(Weak<HandleAndRequest>);

impl PausedHandle {
    fn is_complete(&self) -> bool {
        self.0.upgrade().is_none_or(|r| r.paused_precomposition.borrow().as_ref().is_none_or(|p| p.is_complete()))
    }

    fn resume(&self, should_pause: impl Fn() -> bool + 'static) -> bool {
        let Some(r) = self.0.upgrade() else { return true };
        // (the precomposition object is a handle of the slot: cloned out of the borrow by its slot id)
        let paused = r.paused_precomposition.borrow();
        match paused.as_ref() {
            Some(p) => p.resume(should_pause),
            None => true,
        }
    }
}

impl PrefetchRequest for HandleAndRequest {
    fn execute(&self, scope: &dyn PrefetchRequestScope) -> bool {
        // (the layout that made it left)
        if !self.provider.is_state_active.get() {
            return false;
        }
        self.execute_request(scope)
    }
}

impl PrefetchResultScope for HandleAndRequest {
    fn placeables_count(&self) -> i32 {
        self.precompose_handle.borrow().as_ref().map_or(0, |h| h.placeables_count() as i32)
    }
    fn index(&self) -> i32 {
        self.index
    }
    fn get_size(&self, placeable_index: i32) -> IntSize {
        self.precompose_handle.borrow().as_ref().map_or(IntSize::Zero, |h| h.get_size(placeable_index as usize))
    }
}

/// The nested prefetches of a prefetched item (Compose's NestedPrefetchController): each nested layout's requests,
/// run in order.
struct NestedPrefetchController {
    states: Vec<LazyLayoutPrefetchState>,
    requests_by_state: RefCell<Vec<Option<Vec<Rc<dyn PrefetchRequest>>>>>,
    state_index: Cell<usize>,
    request_index: Cell<usize>,
    executed_nested_prefetch: Cell<bool>,
    parent_canceled: Weak<HandleAndRequest>,
}

impl NestedPrefetchController {
    fn execute_nested_prefetches(
        &self,
        scope: &dyn PrefetchRequestScope,
        nested_prefetch_count: i32,
        is_urgent: bool,
    ) -> bool {
        if self.state_index.get() >= self.states.len() {
            return false;
        }
        assert!(
            self.parent_canceled.upgrade().is_none_or(|p| !p.is_canceled.get()),
            "Should not execute nested prefetch on canceled request"
        );
        // (the nested count known: the nested layouts prefetch that many)
        for state in &self.states {
            state.inner.realized_nested_prefetch_count.set(nested_prefetch_count);
        }
        while self.state_index.get() < self.states.len() {
            let state_index = self.state_index.get();
            if self.requests_by_state.borrow()[state_index].is_none() {
                if scope.available_time_nanos() <= 0 {
                    // (resolved when there is time again)
                    return true;
                }
                let requests = self.states[state_index].collect_nested_prefetch_requests();
                self.requests_by_state.borrow_mut()[state_index] = Some(requests);
            }
            let requests = self.requests_by_state.borrow()[state_index].clone().unwrap_or_default();
            while self.request_index.get() < requests.len() {
                let request = &requests[self.request_index.get()];
                if is_urgent && let Some(r) = as_handle(request) {
                    // (urgent because its parent is)
                    r.is_urgent.set(true);
                }
                self.executed_nested_prefetch.set(true);
                if request.execute(scope) {
                    return true;
                }
                self.request_index.set(self.request_index.get() + 1);
            }
            self.request_index.set(0);
            self.state_index.set(state_index + 1);
        }
        false
    }

    fn collect_ideal_nested_prefetch_count(&self) -> i32 {
        let count = self.states.iter().map(|s| s.inner.ideal_nested_prefetch_count.get()).min().unwrap_or(i32::MAX);
        if count == i32::MAX { 0 } else { count }
    }

    fn collect_nested_prefetched_items_count(&self) -> i32 {
        let count =
            self.states.iter().map(|s| s.inner.last_number_of_nested_prefetch_items.get()).min().unwrap_or(i32::MAX);
        if count == i32::MAX { 0 } else { count }
    }
}

/// A request as the prefetch it is (nested requests are made by providers: always one).
fn as_handle(request: &Rc<dyn PrefetchRequest>) -> Option<&HandleAndRequest> {
    let any: &dyn Any = request.as_ref();
    any.downcast_ref::<HandleAndRequest>()
}

// ---------------------------------------------------------------------------------------------------------------
// Finding the nested layouts' states

/// The key of the nodes telling a lazy layout's prefetch state to the prefetches of the items it is in.
const TraversablePrefetchStateNodeKey: &str = "androidx.compose.foundation.lazy.layout.TraversablePrefetchStateNode";

/// Tells the prefetch state of the layout it is on to the prefetch of an item it is in (Compose's
/// `traversablePrefetchState`).
pub(crate) fn traversable_prefetch_state(modifier: Modifier, state: Option<LazyLayoutPrefetchState>) -> Modifier {
    match state {
        Some(state) => modifier.then_element(TraversablePrefetchStateModifierElement { prefetch_state: state }),
        None => modifier,
    }
}

#[derive(PartialEq)]
struct TraversablePrefetchStateModifierElement {
    prefetch_state: LazyLayoutPrefetchState,
}

impl ModifierNodeElement for TraversablePrefetchStateModifierElement {
    type Node = TraversablePrefetchStateNode;
    fn create(&self) -> TraversablePrefetchStateNode {
        TraversablePrefetchStateNode { prefetch_state: self.prefetch_state.clone() }
    }
    fn update(&self, node: &mut TraversablePrefetchStateNode) {
        node.prefetch_state = self.prefetch_state.clone();
    }
    fn name(&self) -> &'static str {
        "traversablePrefetchState"
    }
}

struct TraversablePrefetchStateNode {
    prefetch_state: LazyLayoutPrefetchState,
}

impl ModifierNode for TraversablePrefetchStateNode {
    node_capabilities!(traversable);
}

impl TraversableNode for TraversablePrefetchStateNode {
    fn traverse_key(&self) -> &dyn Any {
        &TraversablePrefetchStateNodeKey
    }
}

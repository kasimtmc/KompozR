//! ui's spatial layout modifiers, as Compose's: `on_layout_rect_changed` (OnLayoutRectChangedModifier),
//! `on_visibility_changed` (OnVisibilityChangedModifier), `on_first_visible` (OnFirstVisibleModifier) and
//! `layout_bounds` with its [`LayoutBoundsHolder`].

use super::{GlobalPositionAwareModifierNode, LayoutCoordinates, UnplacedAwareModifierNode};
use crate::modifier::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, ObserverModifierNode};
use crate::node_capabilities;
use crate::spatial::{
    OnLayoutRectChangedNode, RectCallback, RelativeLayoutBounds, read_window_size, relative_layout_bounds,
};
use kompoz_coroutines::{Job, delay_ms};
use kompoz_runtime::{MutableState, mutable_state_of, named};
use std::cell::RefCell;
use std::rc::Rc;

/// The spatial modifiers (Compose's `onLayoutRectChanged`, `onVisibilityChanged`, `onFirstVisible` and
/// `layoutBounds`): told where a layout is in the window after the layout passes that move it, and when the
/// window resizes.
#[named]
impl Modifier {
    /// Calls `callback` with the layout's bounds when they change (Compose's `onLayoutRectChanged`): at once when
    /// the last call is `throttle_millis` or more ago, else once they have been still for `debounce_millis`
    /// (the throttle's when 0); with both 0, at once each time. A layout taken out of the tree isn't told
    /// anything more.
    pub fn on_layout_rect_changed(
        self,
        #[default(0)] throttle_millis: i64,
        #[default(64)] debounce_millis: i64,
        callback: impl Fn(&RelativeLayoutBounds) + 'static,
    ) -> Modifier {
        self.then_element(OnLayoutRectChangedElement { throttle_millis, debounce_millis, callback: Rc::new(callback) })
    }

    /// [`on_layout_rect_changed`](Modifier::on_layout_rect_changed) with all its parameters, for plain code.
    pub fn on_layout_rect_changed_with(
        self,
        throttle_millis: i64,
        debounce_millis: i64,
        callback: impl Fn(&RelativeLayoutBounds) + 'static,
    ) -> Modifier {
        self.then_element(OnLayoutRectChangedElement { throttle_millis, debounce_millis, callback: Rc::new(callback) })
    }

    /// Calls `callback` with true when more than `min_fraction_visible` of the layout (or all of it) is in the
    /// window — or in `viewport_bounds`' layout, when given — and with false when it no longer is, or leaves the
    /// layout or stops being placed (Compose's `onVisibilityChanged`). Becoming visible is told once it has
    /// lasted `min_duration_ms`; becoming invisible at once.
    pub fn on_visibility_changed(
        self,
        #[default(0)] min_duration_ms: i64,
        #[default(1.0)] min_fraction_visible: f32,
        #[default(None)] viewport_bounds: Option<LayoutBoundsHolder>,
        callback: impl Fn(bool) + 'static,
    ) -> Modifier {
        let callback: Rc<dyn Fn(bool)> = Rc::new(callback);
        self.then_element(OnVisibilityChangedElement {
            min_duration_ms,
            min_fraction_visible,
            viewport_bounds,
            callback,
        })
    }

    /// [`on_visibility_changed`](Modifier::on_visibility_changed) with all its parameters, for plain code.
    pub fn on_visibility_changed_with(
        self,
        min_duration_ms: i64,
        min_fraction_visible: f32,
        viewport_bounds: Option<LayoutBoundsHolder>,
        callback: impl Fn(bool) + 'static,
    ) -> Modifier {
        let callback: Rc<dyn Fn(bool)> = Rc::new(callback);
        self.then_element(OnVisibilityChangedElement {
            min_duration_ms,
            min_fraction_visible,
            viewport_bounds,
            callback,
        })
    }

    /// Calls `callback` the first time the layout is visible (as `on_visibility_changed` says) and has stayed so
    /// for `min_duration_ms`, then never again (Compose's deprecated `onFirstVisible`: a layout that leaves and
    /// comes back, a lazy list's item, is a new one to it).
    pub fn on_first_visible(
        self,
        #[default(0)] min_duration_ms: i64,
        #[default(1.0)] min_fraction_visible: f32,
        #[default(None)] viewport_bounds: Option<LayoutBoundsHolder>,
        callback: impl Fn() + 'static,
    ) -> Modifier {
        let callback: Rc<dyn Fn()> = Rc::new(callback);
        self.then_element(OnFirstVisibleElement { min_duration_ms, min_fraction_visible, viewport_bounds, callback })
    }

    /// [`on_first_visible`](Modifier::on_first_visible) with all its parameters, for plain code.
    pub fn on_first_visible_with(
        self,
        min_duration_ms: i64,
        min_fraction_visible: f32,
        viewport_bounds: Option<LayoutBoundsHolder>,
        callback: impl Fn() + 'static,
    ) -> Modifier {
        let callback: Rc<dyn Fn()> = Rc::new(callback);
        self.then_element(OnFirstVisibleElement { min_duration_ms, min_fraction_visible, viewport_bounds, callback })
    }

    /// Keeps the layout's bounds in `holder` while it is in the tree (Compose's `layoutBounds`): a viewport for
    /// `on_visibility_changed`, or anything that reads them.
    pub fn layout_bounds(self, holder: LayoutBoundsHolder) -> Modifier {
        self.then_element(LayoutBoundsElement { holder })
    }
}

// ---- on_layout_rect_changed ----

struct OnLayoutRectChangedElement {
    throttle_millis: i64,
    debounce_millis: i64,
    callback: RectCallback,
}

impl PartialEq for OnLayoutRectChangedElement {
    fn eq(&self, other: &Self) -> bool {
        self.throttle_millis == other.throttle_millis
            && self.debounce_millis == other.debounce_millis
            && Rc::ptr_eq(&self.callback, &other.callback)
    }
}

impl ModifierNodeElement for OnLayoutRectChangedElement {
    type Node = OnLayoutRectChangedNode;
    fn create(&self) -> OnLayoutRectChangedNode {
        OnLayoutRectChangedNode::new(self.throttle_millis, self.debounce_millis, self.callback.clone())
    }
    fn update(&self, node: &mut OnLayoutRectChangedNode) {
        node.update(self.throttle_millis, self.debounce_millis, self.callback.clone());
    }
    fn name(&self) -> &'static str {
        "onRectChanged"
    }
}

// ---- layout_bounds ----

/// Where a layout with `Modifier.layout_bounds` is (Compose's LayoutBoundsHolder): none while it isn't in the
/// tree. Reading it subscribes the reader, which is told when the bounds change. Equal only to itself (its
/// clones).
#[derive(Clone)]
pub struct LayoutBoundsHolder {
    /// Counted up as the bounds change (bounds aren't snapshot values: they know their node).
    version: MutableState<u64>,
    bounds: Rc<RefCell<Option<RelativeLayoutBounds>>>,
}

impl LayoutBoundsHolder {
    pub fn new() -> LayoutBoundsHolder {
        LayoutBoundsHolder { version: mutable_state_of(0), bounds: Rc::new(RefCell::new(None)) }
    }

    /// The layout's bounds when it was last placed (Compose's `bounds`).
    pub fn bounds(&self) -> Option<RelativeLayoutBounds> {
        self.version.get();
        self.bounds.borrow().clone()
    }

    /// The same without subscribing the reader.
    fn peek(&self) -> Option<RelativeLayoutBounds> {
        self.bounds.borrow().clone()
    }

    fn set(&self, bounds: Option<RelativeLayoutBounds>) {
        if *self.bounds.borrow() == bounds {
            return;
        }
        *self.bounds.borrow_mut() = bounds;
        self.version.set(self.version.peek() + 1);
    }
}

impl Default for LayoutBoundsHolder {
    fn default() -> Self {
        LayoutBoundsHolder::new()
    }
}

impl PartialEq for LayoutBoundsHolder {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.bounds, &other.bounds)
    }
}

impl std::fmt::Debug for LayoutBoundsHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "LayoutBoundsHolder({:?})", self.peek())
    }
}

#[derive(PartialEq)]
struct LayoutBoundsElement {
    holder: LayoutBoundsHolder,
}

impl ModifierNodeElement for LayoutBoundsElement {
    type Node = LayoutBoundsNode;
    fn create(&self) -> LayoutBoundsNode {
        LayoutBoundsNode { holder: self.holder.clone(), handle: None, last_bounds: None }
    }
    fn update(&self, node: &mut LayoutBoundsNode) {
        node.holder = self.holder.clone();
        node.holder.set(node.last_bounds.clone());
    }
    fn name(&self) -> &'static str {
        "layoutBounds"
    }
}

/// The node of `layout_bounds` (Compose's LayoutBoundsNode): its layout's bounds put in the holder as they
/// change, none once it leaves.
struct LayoutBoundsNode {
    holder: LayoutBoundsHolder,
    handle: Option<NodeHandle>,
    last_bounds: Option<RelativeLayoutBounds>,
}

impl LayoutBoundsNode {
    fn check(&mut self) {
        let Some(bounds) = self.handle.as_ref().and_then(relative_layout_bounds) else { return };
        if self.last_bounds.as_ref() != Some(&bounds) {
            self.last_bounds = Some(bounds.clone());
            self.holder.set(Some(bounds));
        }
    }

    fn observe_window(&self) {
        if let Some(cx) = self.handle.as_ref().and_then(|h| h.cx()) {
            cx.observe_reads(|| read_window_size(&cx));
        }
    }
}

impl ModifierNode for LayoutBoundsNode {
    node_capabilities!(global_position_aware, unplaced_aware, observer);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.observe_window();
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.handle = None;
        self.last_bounds = None;
        self.holder.set(None);
    }
}

impl GlobalPositionAwareModifierNode for LayoutBoundsNode {
    fn on_globally_positioned(&mut self, _: &LayoutCoordinates) {
        self.check();
    }
}

impl UnplacedAwareModifierNode for LayoutBoundsNode {
    fn on_unplaced(&mut self) {
        self.last_bounds = None;
    }
}

impl ObserverModifierNode for LayoutBoundsNode {
    fn on_observed_reads_changed(&mut self) {
        self.observe_window();
        self.check();
    }
}

// ---- Visibility ----

/// What the visibility nodes share: their settings, the bounds last seen, the viewport's, and reading them.
struct Visibility {
    min_duration_ms: i64,
    min_fraction_visible: f32,
    viewport_bounds: Option<LayoutBoundsHolder>,
    handle: Option<NodeHandle>,
    job: Option<Job>,
    last_bounds: Option<RelativeLayoutBounds>,
    last_viewport: Option<RelativeLayoutBounds>,
}

impl Visibility {
    fn new(min_duration_ms: i64, min_fraction_visible: f32, viewport_bounds: Option<LayoutBoundsHolder>) -> Self {
        Visibility {
            min_duration_ms,
            min_fraction_visible,
            viewport_bounds,
            handle: None,
            job: None,
            last_bounds: None,
            last_viewport: None,
        }
    }

    /// Whether `bounds` is visible enough: none while a viewport is asked for and not laid out yet (Compose's
    /// `checkVisibility` up to its result).
    fn is_visible(&self, bounds: &RelativeLayoutBounds) -> Option<bool> {
        if self.last_viewport.is_none() && self.viewport_bounds.is_some() {
            return None;
        }
        let fraction = match &self.last_viewport {
            Some(viewport) => bounds.fraction_visible_in(viewport),
            None => bounds.fraction_visible_in_window(),
        };
        Some(fraction > self.min_fraction_visible || fraction == 1.0)
    }

    /// The layout's bounds now, if they aren't the last ones seen.
    fn moved(&self) -> Option<RelativeLayoutBounds> {
        let bounds = relative_layout_bounds(self.handle.as_ref()?)?;
        if self.last_bounds.as_ref() == Some(&bounds) {
            return None;
        }
        Some(bounds)
    }

    /// Reads the window's size and the viewport's bounds, observed; whether the viewport changed (Compose's
    /// `updateViewport`).
    fn observe(&mut self) -> bool {
        let Some(cx) = self.handle.as_ref().and_then(|h| h.cx()) else { return false };
        let viewport = self.viewport_bounds.clone();
        let now = cx.observe_reads(|| {
            read_window_size(&cx);
            viewport.as_ref().and_then(|v| v.bounds())
        });
        if self.last_viewport != now {
            self.last_viewport = now;
            true
        } else {
            false
        }
    }

    fn cancel_job(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel();
        }
    }

    /// Launches what runs `on_due` with its node after `min_duration_ms`.
    fn launch_timer<N: ModifierNode>(&mut self, on_due: fn(&mut N) -> Option<Box<dyn FnOnce()>>) {
        self.cancel_job();
        let Some(handle) = self.handle.clone() else { return };
        let Some(cx) = handle.cx().filter(|cx| cx.is_attached()) else { return };
        let millis = self.min_duration_ms.max(0) as u64;
        self.job = Some(cx.coroutine_scope().launch(async move {
            delay_ms(millis).await;
            // (the callback is called with the node let go)
            let call = handle.cx().and_then(|cx| cx.with::<N, _>(on_due)).flatten();
            if let Some(call) = call {
                call();
            }
        }));
    }
}

// ---- on_visibility_changed ----

struct OnVisibilityChangedElement {
    min_duration_ms: i64,
    min_fraction_visible: f32,
    viewport_bounds: Option<LayoutBoundsHolder>,
    callback: Rc<dyn Fn(bool)>,
}

impl PartialEq for OnVisibilityChangedElement {
    fn eq(&self, other: &Self) -> bool {
        self.min_duration_ms == other.min_duration_ms
            && self.min_fraction_visible == other.min_fraction_visible
            && self.viewport_bounds == other.viewport_bounds
            && Rc::ptr_eq(&self.callback, &other.callback)
    }
}

impl ModifierNodeElement for OnVisibilityChangedElement {
    type Node = OnVisibilityChangedNode;
    fn create(&self) -> OnVisibilityChangedNode {
        OnVisibilityChangedNode {
            visibility: Visibility::new(self.min_duration_ms, self.min_fraction_visible, self.viewport_bounds.clone()),
            callback: self.callback.clone(),
            last_result: false,
            last_reported_result: false,
        }
    }
    fn update(&self, node: &mut OnVisibilityChangedNode) {
        node.visibility.min_duration_ms = self.min_duration_ms;
        node.visibility.min_fraction_visible = self.min_fraction_visible;
        node.callback = self.callback.clone();
        node.visibility.viewport_bounds = self.viewport_bounds.clone();
        node.update_viewport();
        node.force_update();
    }
    fn name(&self) -> &'static str {
        "onViewportVisibilityChanged"
    }
}

/// Makes the node of `on_visibility_changed`, to delegate to (Compose's `onVisibilityChangedNode`).
pub fn on_visibility_changed_node(
    min_duration_ms: i64,
    min_fraction_visible: f32,
    viewport_bounds: Option<LayoutBoundsHolder>,
    callback: impl Fn(bool) + 'static,
) -> OnVisibilityChangedNode {
    OnVisibilityChangedNode {
        visibility: Visibility::new(min_duration_ms, min_fraction_visible, viewport_bounds),
        callback: Rc::new(callback),
        last_result: false,
        last_reported_result: false,
    }
}

/// The node of `on_visibility_changed` (Compose's OnVisibilityChangedNode).
pub struct OnVisibilityChangedNode {
    visibility: Visibility,
    callback: Rc<dyn Fn(bool)>,
    last_result: bool,
    last_reported_result: bool,
}

impl OnVisibilityChangedNode {
    /// The layout moved to `bounds` (Compose's `rectChanged`).
    fn rect_changed(&mut self, bounds: RelativeLayoutBounds) {
        // (the viewport may have moved in the same pass: read it again, not observed)
        if let Some(v) = &self.visibility.viewport_bounds {
            self.visibility.last_viewport = v.peek();
        }
        self.check_visibility(bounds);
    }

    /// Compose's `checkVisibility`.
    fn check_visibility(&mut self, bounds: RelativeLayoutBounds) {
        let visible = self.visibility.is_visible(&bounds);
        self.visibility.last_bounds = Some(bounds);
        let Some(new_result) = visible else { return };
        if new_result != self.last_result {
            self.last_result = new_result;
            self.visibility.cancel_job();
            if new_result != self.last_reported_result {
                // Becoming visible waits its duration; becoming invisible is told at once.
                if new_result && self.visibility.min_duration_ms > 0 {
                    self.visibility.launch_timer::<OnVisibilityChangedNode>(|n| n.take_trigger());
                } else {
                    self.trigger_callback();
                }
            }
        }
    }

    fn trigger_callback(&mut self) {
        if let Some(call) = self.take_trigger() {
            call();
        }
    }

    /// Compose's `triggerCallback`, the call itself handed back.
    fn take_trigger(&mut self) -> Option<Box<dyn FnOnce()>> {
        self.visibility.cancel_job();
        self.last_reported_result = self.last_result;
        let (callback, result) = (self.callback.clone(), self.last_result);
        Some(Box::new(move || callback(result)))
    }

    fn force_update(&mut self) {
        if let Some(bounds) = self.visibility.last_bounds.clone() {
            self.check_visibility(bounds);
        }
    }

    /// Compose's `fireExitIfNeeded`: told it is invisible, if it was told it is visible.
    fn fire_exit_if_needed(&mut self) {
        self.visibility.cancel_job();
        self.last_result = false;
        if self.last_reported_result {
            self.trigger_callback();
        }
    }

    fn update_viewport(&mut self) {
        if self.visibility.observe() {
            self.force_update();
        }
    }
}

impl ModifierNode for OnVisibilityChangedNode {
    node_capabilities!(global_position_aware, unplaced_aware, observer);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.visibility.handle = Some(cx.handle());
        self.update_viewport();
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.fire_exit_if_needed();
        self.visibility.handle = None;
    }

    fn on_reset(&mut self) {
        self.fire_exit_if_needed();
        self.last_result = false;
        self.visibility.last_bounds = None;
        self.visibility.last_viewport = None;
    }
}

impl GlobalPositionAwareModifierNode for OnVisibilityChangedNode {
    fn on_globally_positioned(&mut self, _: &LayoutCoordinates) {
        if let Some(bounds) = self.visibility.moved() {
            self.rect_changed(bounds);
        }
    }
}

impl UnplacedAwareModifierNode for OnVisibilityChangedNode {
    fn on_unplaced(&mut self) {
        self.fire_exit_if_needed();
        // (placed again, it is told where, as a layout put back in Compose's rect list is)
        self.visibility.last_bounds = None;
    }
}

impl ObserverModifierNode for OnVisibilityChangedNode {
    fn on_observed_reads_changed(&mut self) {
        self.update_viewport();
        // (the window resized)
        if let Some(bounds) = self.visibility.moved() {
            self.rect_changed(bounds);
        }
    }
}

// ---- on_first_visible ----

struct OnFirstVisibleElement {
    min_duration_ms: i64,
    min_fraction_visible: f32,
    viewport_bounds: Option<LayoutBoundsHolder>,
    callback: Rc<dyn Fn()>,
}

impl PartialEq for OnFirstVisibleElement {
    fn eq(&self, other: &Self) -> bool {
        self.min_duration_ms == other.min_duration_ms
            && self.min_fraction_visible == other.min_fraction_visible
            && self.viewport_bounds == other.viewport_bounds
            && Rc::ptr_eq(&self.callback, &other.callback)
    }
}

impl ModifierNodeElement for OnFirstVisibleElement {
    type Node = OnFirstVisibleNode;
    fn create(&self) -> OnFirstVisibleNode {
        OnFirstVisibleNode {
            visibility: Visibility::new(self.min_duration_ms, self.min_fraction_visible, self.viewport_bounds.clone()),
            callback: self.callback.clone(),
            last_result: false,
            done: false,
        }
    }
    fn update(&self, node: &mut OnFirstVisibleNode) {
        node.visibility.min_duration_ms = self.min_duration_ms;
        node.visibility.min_fraction_visible = self.min_fraction_visible;
        node.callback = self.callback.clone();
        node.visibility.viewport_bounds = self.viewport_bounds.clone();
        node.update_viewport();
        node.force_update();
    }
    fn name(&self) -> &'static str {
        "onFirstVisible"
    }
}

/// The node of `on_first_visible` (Compose's OnFirstVisibleNode).
struct OnFirstVisibleNode {
    visibility: Visibility,
    callback: Rc<dyn Fn()>,
    last_result: bool,
    /// Told: it listens no more (Compose's unregistered handle).
    done: bool,
}

impl OnFirstVisibleNode {
    /// Compose's `checkVisibility`.
    fn check_visibility(&mut self, bounds: RelativeLayoutBounds) {
        let visible = self.visibility.is_visible(&bounds);
        self.visibility.last_bounds = Some(bounds);
        let Some(new_result) = visible else { return };
        if new_result && !self.last_result {
            self.start_timer();
        } else if !new_result && self.last_result {
            self.visibility.cancel_job();
        }
        self.last_result = new_result;
    }

    fn start_timer(&mut self) {
        if self.visibility.min_duration_ms == 0 {
            if let Some(call) = self.take_trigger() {
                call();
            }
        } else {
            self.visibility.launch_timer::<OnFirstVisibleNode>(|n| n.take_trigger());
        }
    }

    /// Compose's `triggerCallback`, the call handed back: told once, then deaf.
    fn take_trigger(&mut self) -> Option<Box<dyn FnOnce()>> {
        if self.done {
            return None;
        }
        self.done = true;
        self.visibility.cancel_job();
        let callback = self.callback.clone();
        Some(Box::new(move || callback()))
    }

    fn force_update(&mut self) {
        if !self.done
            && let Some(bounds) = self.visibility.last_bounds.clone()
        {
            self.check_visibility(bounds);
        }
    }

    fn update_viewport(&mut self) {
        if self.visibility.observe() {
            self.force_update();
        }
    }
}

impl ModifierNode for OnFirstVisibleNode {
    node_capabilities!(global_position_aware, observer);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.visibility.handle = Some(cx.handle());
        self.update_viewport();
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.visibility.cancel_job();
        self.visibility.handle = None;
    }

    fn on_reset(&mut self) {
        self.visibility.cancel_job();
        self.last_result = false;
        self.visibility.last_bounds = None;
        self.visibility.last_viewport = None;
    }
}

impl GlobalPositionAwareModifierNode for OnFirstVisibleNode {
    fn on_globally_positioned(&mut self, _: &LayoutCoordinates) {
        if self.done {
            return;
        }
        if let Some(bounds) = self.visibility.moved() {
            self.check_visibility(bounds);
        }
    }
}

impl ObserverModifierNode for OnFirstVisibleNode {
    fn on_observed_reads_changed(&mut self) {
        self.update_viewport();
        if !self.done
            && let Some(bounds) = self.visibility.moved()
        {
            self.check_visibility(bounds);
        }
    }
}
